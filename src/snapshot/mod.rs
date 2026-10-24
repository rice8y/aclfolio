//! Immutable corpus storage and indexed read access.
//!
//! Loading stages source records before constructing indices. Validation and
//! diffing
//! read the resulting snapshot; neither can leave it partially initialized.

use crate::{
    config::Config,
    error::Diagnostic,
    ids::{AnthologyId, CollectionId, IdError, PaperId, VolumeId},
    model::{Collection, Name, NameSpecification, PaperRef, Person, Sig, Venue, Volume},
    people::{IdentityError, PersonIndex, PersonRef, ResolvedPerson},
    source::{Limits, Provenance},
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};

pub use diff::{RecordChanges, SnapshotDiff};
pub use events::{EventAssociation, EventGroup};
use index::Indices;
use load::CollectionSource;

mod diff;
mod directory;
mod events;
mod index;
mod load;
mod validation;

/// Complete immutable metadata snapshot with ordered, borrowed lookups.
#[derive(Debug, Clone)]
pub struct Anthology {
    collections: BTreeMap<CollectionId, Arc<Collection>>,
    config: Config,
    datadir: PathBuf,
    pub(crate) bibkeys: OnceLock<Result<BTreeMap<String, PaperId>, String>>,
    identities: OnceLock<Result<PersonIndex, IdentityError>>,
    indices: Indices,
    people: BTreeMap<String, Person>,
    venues: BTreeMap<String, Venue>,
    sigs: BTreeMap<String, Sig>,
    provenance: Provenance,
    diagnostics: Vec<Diagnostic>,
    sources: BTreeMap<PathBuf, CollectionSource>,
    stats: ReloadStats,
    limits: Limits,
}

/// Reuse accounting for the most recent load.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ReloadStats {
    /// Collections parsed from XML.
    pub parsed_collections: usize,
    /// Unchanged immutable collection allocations reused.
    pub reused_collections: usize,
}

impl Anthology {
    /// Collections in canonical ID order.
    pub fn collections(&self) -> impl ExactSizeIterator<Item = &Collection> {
        self.collections.values().map(Arc::as_ref)
    }

    /// Volumes in canonical ID order.
    pub fn volumes(&self) -> impl ExactSizeIterator<Item = &Volume> {
        self.indices
            .volumes
            .keys()
            .map(|id| self.volume_by_id(id).expect("indexed volume"))
    }

    /// Papers and front matter in canonical ID order.
    pub fn papers(&self) -> impl ExactSizeIterator<Item = PaperRef<'_>> {
        self.indices
            .papers
            .keys()
            .map(|id| self.paper_by_id(id).expect("indexed paper"))
    }

    /// Explicit and venue/year-derived events in ID order, with origin exposed.
    pub fn events(&self) -> impl ExactSizeIterator<Item = EventGroup<'_>> {
        self.indices
            .events
            .iter()
            .map(|(id, collections)| EventGroup {
                anthology: self,
                id,
                collections,
            })
    }

    /// Collection lookup; malformed and missing IDs have different results.
    pub fn collection(&self, id: &str) -> Result<Option<&Collection>, IdError> {
        Ok(self
            .collections
            .get(&id.parse::<CollectionId>()?)
            .map(Arc::as_ref))
    }

    /// Volume lookup; malformed and missing IDs have different results.
    pub fn volume(&self, id: &str) -> Result<Option<&Volume>, IdError> {
        Ok(self.volume_by_id(&id.parse()?))
    }

    /// Lookup without allocating/parsing an already validated ID.
    pub fn volume_by_id(&self, id: &VolumeId) -> Option<&Volume> {
        let (collection_id, volume_index) = self.indices.volumes.get(id)?;
        Some(&self.collections[collection_id].volumes[*volume_index])
    }

    /// Paper or front matter lookup; malformed and missing IDs have different
    /// results.
    pub fn paper(&self, id: &str) -> Result<Option<PaperRef<'_>>, IdError> {
        Ok(self.paper_by_id(&id.parse()?))
    }

    /// Lookup without allocating/parsing an already validated ID.
    pub fn paper_by_id(&self, id: &PaperId) -> Option<PaperRef<'_>> {
        let (volume_index, paper_index) = self.indices.papers.get(id)?;
        let volume = self.volume_by_id(volume_index)?;
        Some(PaperRef {
            paper: &volume.papers[*paper_index],
            volume,
            anthology: self,
        })
    }

    /// Event lookup, including venue/year-derived events.
    pub fn event(&self, id: &str) -> Option<EventGroup<'_>> {
        let (id, collections) = self.indices.events.get_key_value(id)?;
        Some(EventGroup {
            anthology: self,
            id,
            collections,
        })
    }

    /// Directory identity lookup; names alone never establish identity.
    pub fn person(&self, id: &str) -> Option<&Person> {
        self.people.get(id)
    }

    /// Resolve an explicit credit ID. Sentinel or unresolved IDs return None.
    pub fn resolve_person(&self, name: &NameSpecification) -> Option<&Person> {
        self.person(name.id.as_deref()?)
    }

    /// People in directory key order, without synthesized people.
    pub fn people(&self) -> impl ExactSizeIterator<Item = (&str, &Person)> {
        self.people.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Venue lookup.
    pub fn venue(&self, id: &str) -> Option<&Venue> {
        self.venues.get(id)
    }

    /// Venues in directory key order.
    pub fn venues(&self) -> impl ExactSizeIterator<Item = (&str, &Venue)> {
        self.venues.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// SIG lookup.
    pub fn sig(&self, id: &str) -> Option<&Sig> {
        self.sigs.get(id)
    }

    /// SIGs in directory key order.
    pub fn sigs(&self) -> impl ExactSizeIterator<Item = (&str, &Sig)> {
        self.sigs.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Explicitly credited paper and volume IDs. No name matching is performed.
    pub fn works(&self, person: &str) -> impl Iterator<Item = &AnthologyId> {
        self.indices.works.get(person).into_iter().flatten()
    }

    /// Volumes with an explicit venue association.
    pub fn venue_volumes(&self, id: &str) -> impl Iterator<Item = &Volume> {
        self.indices
            .venue_volumes
            .get(id)
            .into_iter()
            .flatten()
            .filter_map(|id| self.volume_by_id(id))
    }

    /// Volumes with an explicit SIG association (not inherited from venue hints).
    pub fn sig_volumes(&self, id: &str) -> impl Iterator<Item = &Volume> {
        self.indices
            .sig_volumes
            .get(id)
            .into_iter()
            .flatten()
            .filter_map(|id| self.volume_by_id(id))
    }

    /// Provenance fingerprints for every XML, directory JSON and schema file.
    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    /// Structured compatibility diagnostics in deterministic order.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Reparse/reuse counts.
    pub fn reload_stats(&self) -> &ReloadStats {
        &self.stats
    }

    /// Events connected to a volume, without scanning all collections.
    pub fn volume_events(&self, id: &VolumeId) -> impl Iterator<Item = EventGroup<'_>> {
        self.indices
            .volume_events
            .get(id)
            .into_iter()
            .flatten()
            .filter_map(|event| self.event(event))
    }

    /// Symmetric venue relationships, including reverse-only links.
    pub fn related_venues<'a>(&'a self, id: &'a str) -> impl Iterator<Item = (&'a str, &'a Venue)> {
        self.venues().filter(move |(other, v)| {
            v.related_ids.iter().any(|x| x == id)
                || self
                    .venue(id)
                    .is_some_and(|v| v.related_ids.iter().any(|x| x == other))
        })
    }

    /// Symmetric SIG relationships, including reverse-only links.
    pub fn related_sigs<'a>(&'a self, id: &'a str) -> impl Iterator<Item = (&'a str, &'a Sig)> {
        self.sigs().filter(move |(other, s)| {
            s.related_ids.iter().any(|x| x == id)
                || self
                    .sig(id)
                    .is_some_and(|s| s.related_ids.iter().any(|x| x == other))
        })
    }

    /// Complete official person resolution, including inferred identities and
    /// reverse credits.
    pub fn person_index(&self) -> Result<&PersonIndex, IdentityError> {
        self.identities
            .get_or_init(|| PersonIndex::build(self))
            .as_ref()
            .map_err(Clone::clone)
    }

    /// Look up an explicit or inferred person with publication navigation.
    pub fn get_person(&self, id: &str) -> Result<Option<PersonRef<'_>>, IdentityError> {
        Ok(self.person_index()?.get(id).map(|person| PersonRef {
            anthology: self,
            person,
        }))
    }

    /// Find every identity having this exact first/last name.
    pub fn find_people(&self, name: &Name) -> Result<Vec<PersonRef<'_>>, IdentityError> {
        Ok(self
            .person_index()?
            .get_by_name(name)
            .into_iter()
            .map(|person| PersonRef {
                anthology: self,
                person,
            })
            .collect())
    }

    /// Resolve a publication's credit using IDs, ORCIDs and official name
    /// heuristics.
    pub fn resolve(&self, name: &NameSpecification) -> Result<PersonRef<'_>, IdentityError> {
        Ok(PersonRef {
            anthology: self,
            person: self.person_index()?.resolve(name)?,
        })
    }

    /// URL configuration captured when the snapshot was loaded.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Directory from which this snapshot was read.
    pub fn datadir(&self) -> &Path {
        &self.datadir
    }

    /// Original source path of a stored collection.
    pub fn collection_source_path(&self, id: &CollectionId) -> Option<PathBuf> {
        self.sources
            .iter()
            .find(|(_, source)| &source.id == id)
            .map(|(path, _)| self.datadir.join(path))
    }

    /// Clear derived identity and citation-key caches. Stored records remain
    /// immutable.
    pub fn reset_indices(&mut self) {
        self.identities.take();
        self.bibkeys.take();
    }

    /// Whether the complete identity index has already been built.
    pub fn people_index_loaded(&self) -> bool {
        self.identities.get().is_some()
    }

    /// Whether the complete citation-key index has already been built.
    pub fn bibkey_index_loaded(&self) -> bool {
        self.bibkeys.get().is_some()
    }

    /// Resolve a detached credit, returning an owned identity including newly
    /// inferred variants.
    /// Use a cloned `PersonIndex` for a sequence of hypothetical resolutions
    /// sharing new identities.
    pub fn resolve_credit(
        &self,
        credit: &NameSpecification,
    ) -> Result<ResolvedPerson, IdentityError> {
        let mut index = self.person_index()?.clone();
        Ok(index.get_by_namespec(credit)?.clone())
    }

    /// Reverse event index, keyed by a volume's complete identifier.
    pub fn event_reverse_index(&self) -> &BTreeMap<VolumeId, BTreeSet<String>> {
        &self.indices.volume_events
    }
}
