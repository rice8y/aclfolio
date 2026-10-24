//! Identity-bound publication traversal and coauthor queries.

use super::{IdentityError, ResolvedPerson, error};
use crate::{Anthology, AnthologyId, NameSpecification, PaperRef, Volume};
use std::collections::BTreeMap;

/// Resolved person bound to its immutable snapshot for publication navigation.
#[derive(Debug, Clone, Copy)]
pub struct PersonRef<'a> {
    pub(crate) anthology: &'a Anthology,
    pub(crate) person: &'a ResolvedPerson,
}

impl std::ops::Deref for PersonRef<'_> {
    type Target = ResolvedPerson;

    fn deref(&self) -> &ResolvedPerson {
        self.person
    }
}

impl<'a> PersonRef<'a> {
    /// Snapshot containing this person.
    pub fn root(self) -> &'a Anthology {
        self.anthology
    }

    /// Papers authored/edited, including frontmatter of edited volumes.
    pub fn papers(self) -> impl Iterator<Item = PaperRef<'a>> {
        self.person.item_ids.iter().filter_map(move |id| {
            if let AnthologyId::Paper(p) = id {
                self.anthology.paper_by_id(p)
            } else {
                None
            }
        })
    }

    /// Volumes edited by this person.
    pub fn volumes(self) -> impl Iterator<Item = &'a Volume> {
        self.person.item_ids.iter().filter_map(move |id| {
            if let AnthologyId::Volume(v) = id {
                self.anthology.volume_by_id(v)
            } else {
                None
            }
        })
    }

    /// Counts of coauthored papers and coedited volumes, matching the reference's
    /// rules.
    pub fn find_coauthors_counter(
        self,
        include_volumes: bool,
    ) -> Result<BTreeMap<String, usize>, IdentityError> {
        let index = self.anthology.person_index()?;
        let mut counts = BTreeMap::new();
        for id in &self.person.item_ids {
            let names: Vec<_> = match id {
                AnthologyId::Volume(v) => {
                    let v = self.anthology.volume_by_id(v).unwrap();
                    if !include_volumes && !v.papers.iter().any(|p| p.frontmatter) {
                        continue;
                    }
                    v.metadata.editors.iter().collect()
                }
                AnthologyId::Paper(p) => {
                    let p = self.anthology.paper_by_id(p).unwrap();
                    p.stored()
                        .authors
                        .iter()
                        .chain(&p.stored().editors)
                        .collect()
                }
                _ => continue,
            };
            for ns in names {
                let p = index.resolve(ns)?;
                *counts.entry(p.id.clone()).or_insert(0) += 1;
            }
        }
        counts.remove(&self.person.id);
        Ok(counts)
    }

    /// Similar identities, with errors for dangling metadata links.
    pub fn similar(self) -> Result<Vec<Self>, IdentityError> {
        Ok(self
            .anthology
            .person_index()?
            .similar(&self.person.id)?
            .into_iter()
            .map(|person| Self {
                anthology: self.anthology,
                person,
            })
            .collect())
    }
}

impl<'a> PersonRef<'a> {
    /// Every authored or edited publication, retaining its entity type.
    pub fn anthology_items(self) -> impl Iterator<Item = crate::navigation::AnthologyItem<'a>> {
        self.person
            .item_ids
            .iter()
            .filter_map(move |id| self.anthology.get(&id.to_string()).ok().flatten())
    }

    /// The first matching credit on each associated item, including inherited
    /// frontmatter editors.
    pub fn namespecs(self) -> Result<Vec<&'a NameSpecification>, IdentityError> {
        self.anthology_items()
            .map(|item| match item {
                crate::navigation::AnthologyItem::Paper(p) => p.get_namespec_for(&self.id),
                crate::navigation::AnthologyItem::Volume(v) => v.get_namespec_for(&self.id),
                _ => Err(error("ValueError", "collection has no credits")),
            })
            .collect()
    }
}

impl<'a> PaperRef<'a> {
    /// First author/editor credit resolving to the given person ID.
    pub fn get_namespec_for(self, id: &str) -> Result<&'a NameSpecification, IdentityError> {
        let index = self.anthology.person_index()?;
        for ns in self.namespecs().chain(
            if self.is_frontmatter() {
                self.editors()
            } else {
                &[]
            }
            .iter(),
        ) {
            if index.resolve(ns)?.id == id {
                return Ok(ns);
            }
        }
        Err(error(
            "ValueError",
            format!("no credit on {} resolves to {id}", self.id()),
        ))
    }
}

impl<'a> crate::navigation::VolumeRef<'a> {
    /// First editor credit resolving to the given person ID.
    pub fn get_namespec_for(self, id: &str) -> Result<&'a NameSpecification, IdentityError> {
        let index = self.root().person_index()?;
        for ns in self.namespecs() {
            if index.resolve(ns)?.id == id {
                return Ok(ns);
            }
        }
        Err(error(
            "ValueError",
            format!("no credit on {} resolves to {id}", self.id),
        ))
    }
}

impl<'a> PersonRef<'a> {
    /// Unique coauthors/coeditors under the reference's frontmatter rules.
    pub fn find_coauthors(
        self,
        include_volumes: bool,
    ) -> Result<Vec<PersonRef<'a>>, IdentityError> {
        self.find_coauthors_counter(include_volumes)?
            .keys()
            .map(|id| {
                self.anthology
                    .get_person(id)?
                    .ok_or_else(|| error("KeyError", id))
            })
            .collect()
    }
}
