//! Immutable person identity resolution and reverse publication indices.

pub use navigation::PersonRef;

use crate::{
    Anthology, AnthologyId,
    ids::{convert_orcid, is_valid_orcid, is_verified_person_id},
    model::{Name, NameSpecification, Person},
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

mod navigation;

/// Name lookup key: Python compares first/last components, ignoring script.
pub type NameKey = (String, String);

fn key(name: &Name) -> NameKey {
    (name.first.clone().unwrap_or_default(), name.last.clone())
}

/// Whether a name was recorded explicitly or inferred while resolving credits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NameLink {
    /// Listed in people.json.
    Explicit,
    /// Inferred from publication credits.
    Inferred,
}

/// Failure resolving person metadata or a publication credit.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{kind}: {message}")]
pub struct IdentityError {
    /// Python-equivalent exception category.
    pub kind: String,
    /// Context and reason.
    pub message: String,
}

fn error(kind: &str, message: impl Into<String>) -> IdentityError {
    IdentityError {
        kind: kind.into(),
        message: message.into(),
    }
}

/// Disjoint sets of similar identifiers. Detached values may be freely
/// combined.
#[derive(Debug, Clone, Default)]
pub struct SimilarityGroups {
    membership: BTreeMap<String, usize>,
    groups: Vec<BTreeSet<String>>,
}

impl SimilarityGroups {
    /// Combine two sets; unseen identifiers begin in singleton sets.
    pub fn merge(&mut self, a: &str, b: &str) {
        let a = self.ensure(a);
        let b = self.ensure(b);
        if a == b {
            return;
        }
        let (large, small) = if self.groups[a].len() >= self.groups[b].len() {
            (a, b)
        } else {
            (b, a)
        };
        let members = std::mem::take(&mut self.groups[small]);
        for item in members {
            self.membership.insert(item.clone(), large);
            self.groups[large].insert(item);
        }
    }

    fn ensure(&mut self, id: &str) -> usize {
        if let Some(i) = self.membership.get(id) {
            return *i;
        }
        let i = self.groups.len();
        self.groups.push(BTreeSet::from([id.into()]));
        self.membership.insert(id.into(), i);
        i
    }

    /// Whether two identifiers are in the same set (including unseen self-pairs).
    pub fn connected(&self, a: &str, b: &str) -> bool {
        a == b
            || self
                .membership
                .get(a)
                .is_some_and(|i| Some(i) == self.membership.get(b))
    }

    /// Copy all members of an identifier's set, including the identifier itself.
    pub fn subset(&self, id: &str) -> BTreeSet<String> {
        self.membership
            .get(id)
            .map(|i| self.groups[*i].clone())
            .unwrap_or_else(|| BTreeSet::from([id.into()]))
    }

    /// Whether this identifier was explicitly merged into the structure.
    pub fn contains(&self, id: &str) -> bool {
        self.membership.contains_key(id)
    }

    /// Identifiers which have been explicitly merged.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &str> {
        self.membership.keys().map(String::as_str)
    }

    /// Count explicitly merged identifiers.
    pub fn len(&self) -> usize {
        self.membership.len()
    }

    /// Whether no identifiers have been merged.
    pub fn is_empty(&self) -> bool {
        self.membership.is_empty()
    }

    /// Remove an identifier from a detached grouping.
    pub fn remove(&mut self, id: &str) {
        if let Some(i) = self.membership.remove(id) {
            self.groups[i].remove(id);
        }
    }
}

/// A person after applying the official name-resolution heuristics.
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedPerson {
    /// Verified or `/unverified` identity key.
    pub id: String,
    /// Metadata with explicit and inferred names in canonical order.
    pub metadata: Person,
    /// Whether the identity is present in people.json.
    pub is_explicit: bool,
    /// Provenance parallel to `metadata.names`.
    pub name_links: Vec<NameLink>,
    /// Authored papers, edited volumes and their frontmatter.
    pub item_ids: BTreeSet<AnthologyId>,
}

impl std::ops::Deref for ResolvedPerson {
    type Target = Person;

    fn deref(&self) -> &Person {
        &self.metadata
    }
}

impl ResolvedPerson {
    /// Canonical name, or an error for an invalid empty name list.
    pub fn canonical_name(&self) -> Result<&Name, IdentityError> {
        self.names
            .first()
            .ok_or_else(|| error("ValueError", format!("no names defined for {}", self.id)))
    }

    /// Whether this person has an equivalent first/last name.
    pub fn has_name(&self, name: &Name) -> bool {
        self.names.iter().any(|n| n.equivalent(name))
    }
}

/// Complete resolved people index; built lazily once per immutable snapshot.
#[derive(Debug, Clone, Default)]
pub struct PersonIndex {
    people: BTreeMap<String, ResolvedPerson>,
    by_name: BTreeMap<NameKey, Vec<String>>,
    by_orcid: BTreeMap<String, String>,
    slugs: BTreeMap<String, BTreeSet<String>>,
    similar: SimilarityGroups,
    warnings: Vec<String>,
}

impl PersonIndex {
    pub(crate) fn build(a: &Anthology) -> Result<Self, IdentityError> {
        let mut index = Self::default();
        for (id, p) in a.people() {
            if !is_verified_person_id(id) {
                return Err(error(
                    "AnthologyInvalidIDError",
                    format!("invalid person ID {id}"),
                ));
            }
            let mut p = p.clone();
            p.orcid = p.orcid.as_deref().map(convert_orcid);
            if p.orcid.as_ref().is_some_and(|s| !is_valid_orcid(s)) {
                return Err(error("ValueError", format!("invalid ORCID on {id}")));
            }
            if let Some(orcid) = &p.orcid {
                if index.by_orcid.insert(orcid.clone(), id.into()).is_some() {
                    return Err(error("ValueError", format!("duplicate ORCID {orcid}")));
                }
            }
            for other in &p.similar {
                index.similar.merge(id, other);
            }
            let names = p.names.clone();
            index.people.insert(
                id.into(),
                ResolvedPerson {
                    id: id.into(),
                    name_links: vec![NameLink::Explicit; names.len()],
                    metadata: p,
                    is_explicit: true,
                    item_ids: Default::default(),
                },
            );
            for name in names {
                index.add_name_index(id, &name);
            }
        }
        let mut errors = vec![];
        for c in a.collections() {
            for v in &c.volumes {
                let result = (|| {
                    index.add_item(v.metadata.editors.iter(), AnthologyId::Volume(v.id.clone()))?;
                    for p in &v.papers {
                        let names: Vec<_> = if p.frontmatter {
                            if p.editors.is_empty() {
                                v.metadata.editors.iter().collect()
                            } else {
                                p.editors.iter().collect()
                            }
                        } else {
                            p.authors.iter().chain(&p.editors).collect()
                        };
                        index.add_item(names.into_iter(), AnthologyId::Paper(p.id.clone()))?;
                    }
                    Ok::<(), IdentityError>(())
                })();
                if let Err(e) = result {
                    errors.push(format!("{}: {e}", v.id));
                }
            }
        }
        if !errors.is_empty() {
            return Err(error("ExceptionGroup", errors.join("\n")));
        }
        Ok(index)
    }

    fn add_name_index(&mut self, id: &str, name: &Name) {
        let names = self.by_name.entry(key(name)).or_default();
        if names.iter().any(|s| s == id) {
            return;
        }
        if let Some(other) = names.last() {
            self.similar.merge(id, other);
        }
        names.push(id.into());
        if is_verified_person_id(id) {
            let ids = self.slugs.entry(name.slugify()).or_default();
            if !ids.contains(id) {
                if let Some(other) = ids.first() {
                    self.similar.merge(id, other);
                }
                ids.insert(id.into());
            }
        }
    }

    fn inferred_name(&mut self, id: &str, name: &Name, canonical: bool, indexed: bool) {
        let person = self.people.get_mut(id).unwrap();
        if !person.has_name(name) {
            if canonical {
                person.metadata.names.insert(0, name.clone());
                person.name_links.insert(0, NameLink::Inferred);
            } else {
                person.metadata.names.push(name.clone());
                person.name_links.push(NameLink::Inferred);
            }
            if indexed {
                self.add_name_index(id, name);
            }
        }
    }

    fn resolve_build(&mut self, ns: &NameSpecification) -> Result<String, IdentityError> {
        let mut normalized = ns.clone();
        normalized.name.first = normalized.name.first.filter(|s| !s.is_empty());
        if let Some(n) = &mut normalized.variant {
            if n.first.as_deref() == Some("") {
                n.first = None;
            }
        }
        for n in &mut normalized.additional_variants {
            if n.first.as_deref() == Some("") {
                n.first = None;
            }
        }
        let ns = &normalized;
        let id = self.resolution_id(ns)?;
        if self.people.contains_key(&id) {
            let person = &self.people[&id];
            let canonical = !person.is_explicit
                && !person.has_name(&ns.name)
                && ns.name.score() > person.canonical_name()?.score();
            self.inferred_name(&id, &ns.name, canonical, true);
        } else {
            self.people.insert(
                id.clone(),
                ResolvedPerson {
                    id: id.clone(),
                    metadata: Person {
                        names: vec![ns.name.clone()],
                        orcid: None,
                        comment: None,
                        degree: None,
                        similar: vec![],
                        disable_name_matching: Some(false),
                        extensions: Default::default(),
                    },
                    is_explicit: false,
                    name_links: vec![NameLink::Inferred],
                    item_ids: Default::default(),
                },
            );
            self.add_name_index(&id, &ns.name);
        }
        for variant in ns.variants() {
            self.inferred_name(&id, variant, false, false);
            self.by_name
                .entry(key(variant))
                .or_insert_with(|| vec![id.clone()]);
        }
        Ok(id)
    }

    fn resolution_id(&self, ns: &NameSpecification) -> Result<String, IdentityError> {
        if let Some(id) = ns
            .id
            .as_deref()
            .filter(|id| *id != crate::types::NO_PERSON_ID)
        {
            let p = self
                .people
                .get(id)
                .filter(|p| p.is_explicit)
                .ok_or_else(|| {
                    error(
                        "PersonDefinitionError",
                        format!("ID {id} was not defined in people.json"),
                    )
                })?;
            if !p.has_name(&ns.name) {
                return Err(error(
                    "PersonDefinitionError",
                    format!("ID {id} has no name {}", ns.name.as_first_last()),
                ));
            }
            if let Some(orcid) = &ns.orcid {
                if Some(convert_orcid(orcid)) != p.orcid {
                    return Err(error(
                        "PersonDefinitionError",
                        format!("ORCID differs for {id}"),
                    ));
                }
            }
            return Ok(id.into());
        }
        if ns.orcid.is_some() && ns.id.as_deref() != Some(crate::types::NO_PERSON_ID) {
            return Err(error(
                "NameSpecResolutionError",
                "ORCID without an explicit ID",
            ));
        }
        let slug = ns.name.slugify();
        if let Some(ids) = self.slugs.get(&slug) {
            if ids.len() == 1 {
                let id = ids.first().unwrap();
                if !self.people[id].disable_name_matching.unwrap_or(false) {
                    return Ok(id.clone());
                }
            }
        }
        Ok(format!("{slug}/unverified"))
    }

    fn add_item<'a>(
        &mut self,
        names: impl Iterator<Item = &'a NameSpecification>,
        item: AnthologyId,
    ) -> Result<(), IdentityError> {
        let mut seen = BTreeSet::new();
        for ns in names {
            let id = self.resolve_build(ns)?;
            let p = self.people.get_mut(&id).unwrap();
            p.item_ids.insert(item.clone());
            if !seen.insert(id.clone()) {
                let message = format!("more than one credit resolves to {id} on {item}");
                if p.is_explicit {
                    return Err(error("NameSpecResolutionError", message));
                }
                self.warnings.push(message);
            }
        }
        Ok(())
    }

    /// All resolved people, including automatically inferred identities.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&str, &ResolvedPerson)> {
        self.people.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Number of explicit and inferred identities.
    pub fn len(&self) -> usize {
        self.people.len()
    }

    /// Whether no identities are indexed.
    pub fn is_empty(&self) -> bool {
        self.people.is_empty()
    }

    /// Resolve a verified or inferred identity key.
    pub fn get(&self, id: &str) -> Option<&ResolvedPerson> {
        self.people.get(id)
    }

    /// Resolve a name specification using the complete snapshot's identity index.
    pub fn resolve(&self, ns: &NameSpecification) -> Result<&ResolvedPerson, IdentityError> {
        let id = self.resolution_id(ns)?;
        self.people.get(&id).ok_or_else(|| {
            error(
                "NameSpecResolutionError",
                format!("resolved identity {id} does not exist"),
            )
        })
    }

    /// Exact-name lookup (script is not part of the comparison).
    pub fn get_by_name(&self, name: &Name) -> Vec<&ResolvedPerson> {
        self.by_name
            .get(&key(name))
            .into_iter()
            .flatten()
            .filter_map(|id| self.people.get(id))
            .collect()
    }

    /// ORCID lookup without silently normalizing the supplied lookup key.
    pub fn get_by_orcid(&self, orcid: &str) -> Option<&ResolvedPerson> {
        self.by_orcid.get(orcid).and_then(|id| self.people.get(id))
    }

    /// Complete exact-name reverse index.
    pub fn by_name(&self) -> &BTreeMap<NameKey, Vec<String>> {
        &self.by_name
    }

    /// Complete ORCID reverse index.
    pub fn by_orcid(&self) -> &BTreeMap<String, String> {
        &self.by_orcid
    }

    /// Name slugs and corresponding verified IDs.
    pub fn slugs_to_verified_ids(&self) -> &BTreeMap<String, BTreeSet<String>> {
        &self.slugs
    }

    /// Transitive groups inferred from names and explicit similarity links.
    pub fn similarity_groups(&self) -> &SimilarityGroups {
        &self.similar
    }

    /// Nonfatal duplicate-credit diagnostics encountered during index construction.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Similar people, reporting dangling explicit similarity references.
    pub fn similar(&self, id: &str) -> Result<Vec<&ResolvedPerson>, IdentityError> {
        self.similar
            .subset(id)
            .into_iter()
            .filter(|s| s != id)
            .map(|id| {
                self.get(&id)
                    .ok_or_else(|| error("KeyError", format!("similar person {id} does not exist")))
            })
            .collect()
    }

    /// Generate an unused verified ID without modifying records or reserving the
    /// ID.
    pub fn generate_person_id(
        &self,
        name: &Name,
        suffix: Option<&str>,
        orcid: Option<&str>,
    ) -> Result<String, IdentityError> {
        let slug = name.slugify();
        if !self.people.contains_key(&slug) {
            return Ok(slug);
        }
        if let Some(s) = suffix {
            let id = format!("{slug}-{}", s.to_lowercase());
            if !self.people.contains_key(&id) {
                return Ok(id);
            }
        }
        if let Some(o) = orcid {
            let end = o
                .chars()
                .rev()
                .take(4)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<String>()
                .to_lowercase();
            let id = format!("{slug}-{end}");
            if !self.people.contains_key(&id) {
                return Ok(id);
            }
        }
        Err(error(
            "AnthologyException",
            "could not generate an unused person ID",
        ))
    }
}

impl PersonIndex {
    /// Resolve a hypothetical credit in a detached index, retaining inferred names.
    /// Clone a snapshot's index to explore credits without modifying its records.
    pub fn get_by_namespec(
        &mut self,
        credit: &NameSpecification,
    ) -> Result<&ResolvedPerson, IdentityError> {
        let id = self.resolve_build(credit)?;
        Ok(&self.people[&id])
    }
}
