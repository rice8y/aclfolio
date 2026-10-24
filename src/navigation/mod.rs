//! Snapshot-bound navigation. Views resolve metadata without changing stored
//! records.

pub use collection::CollectionRef;
pub use paper::{DateOrigin, EffectiveDate, PaperRef};
pub use volume::VolumeRef;

use crate::{
    Anthology, Collection, EventGroup,
    ids::{IdError, IdParts, parse_id},
};

mod collection;
mod paper;
mod volume;

/// Result of a polymorphic collection/volume/paper lookup.
#[derive(Debug, Clone, Copy)]
pub enum AnthologyItem<'a> {
    /// Collection record.
    Collection(&'a Collection),
    /// Snapshot-bound volume.
    Volume(VolumeRef<'a>),
    /// Snapshot-bound paper.
    Paper(PaperRef<'a>),
}

impl Anthology {
    /// Lookup a collection using any of its collection/volume/paper identifiers.
    pub fn get_collection(&self, id: &str) -> Result<Option<&Collection>, IdError> {
        let parts = parse_id(id)?;
        Ok(self.collection(&parts.0).ok().flatten())
    }

    /// Lookup a volume using a volume or contained-paper identifier.
    pub fn get_volume(&self, id: &str) -> Result<Option<VolumeRef<'_>>, IdError> {
        self.get_volume_parts(&parse_id(id)?)
    }

    /// Lookup a volume from already separated local components.
    pub fn get_volume_parts(&self, parts: &IdParts) -> Result<Option<VolumeRef<'_>>, IdError> {
        let Some(volume_id) = &parts.1 else {
            return Ok(None);
        };
        let Some(collection) = self.collection(&parts.0).ok().flatten() else {
            return Ok(None);
        };
        Ok(collection
            .volumes
            .iter()
            .find(|volume| &volume.local_id == volume_id)
            .map(|volume| VolumeRef {
                anthology: self,
                volume,
            }))
    }

    /// Lookup a paper using official unpadded local-component semantics.
    pub fn get_paper(&self, id: &str) -> Result<Option<PaperRef<'_>>, IdError> {
        self.get_paper_parts(&parse_id(id)?)
    }

    /// Lookup a paper from already separated local components.
    pub fn get_paper_parts(&self, p: &IdParts) -> Result<Option<PaperRef<'_>>, IdError> {
        Ok(self
            .get_volume_parts(p)?
            .and_then(|v| p.2.as_deref().and_then(|p| v.get(p))))
    }

    /// Lookup any collection, volume or paper using a full identifier.
    pub fn get(&self, id: &str) -> Result<Option<AnthologyItem<'_>>, IdError> {
        self.get_parts(&parse_id(id)?)
    }

    /// Lookup any collection, volume or paper using local components.
    pub fn get_parts(&self, p: &IdParts) -> Result<Option<AnthologyItem<'_>>, IdError> {
        if p.1.is_none() {
            return Ok(self
                .collection(&p.0)
                .ok()
                .flatten()
                .map(AnthologyItem::Collection));
        }
        if p.2.is_none() {
            return Ok(self.get_volume_parts(p)?.map(AnthologyItem::Volume));
        }
        Ok(self.get_paper_parts(p)?.map(AnthologyItem::Paper))
    }

    /// Volumes in collection/XML order, optionally restricted to one collection.
    pub fn volume_refs<'a>(
        &'a self,
        collection: Option<&'a str>,
    ) -> impl Iterator<Item = VolumeRef<'a>> {
        self.collections()
            .filter(move |c| collection.is_none_or(|id| id == c.id.as_str()))
            .flat_map(|c| c.volumes.iter())
            .map(|volume| VolumeRef {
                anthology: self,
                volume,
            })
    }

    /// Papers in collection/XML order, optionally restricted by any entity ID.
    pub fn papers_in(&self, id: Option<&str>) -> Result<Vec<PaperRef<'_>>, IdError> {
        Ok(match id {
            None => self.volume_refs(None).flat_map(VolumeRef::papers).collect(),
            Some(id) => match self.get(id)? {
                None => vec![],
                Some(AnthologyItem::Paper(p)) => vec![p],
                Some(AnthologyItem::Volume(v)) => v.papers().collect(),
                Some(AnthologyItem::Collection(c)) => c
                    .volumes
                    .iter()
                    .flat_map(|v| v.papers.iter())
                    .filter_map(|p| self.paper_by_id(&p.id))
                    .collect(),
            },
        })
    }

    /// Find a paper by its bibliographic citation key.
    pub fn get_paper_by_bibkey(&self, bibkey: &str) -> Result<Option<PaperRef<'_>>, String> {
        Ok(self
            .bibkey_index()?
            .get(bibkey)
            .and_then(|id| self.paper_by_id(id)))
    }
}

impl Anthology {
    /// Lookup a collection with source and snapshot navigation.
    pub fn collection_ref(&self, id: &str) -> Result<Option<CollectionRef<'_>>, IdError> {
        Ok(self.get_collection(id)?.map(|collection| CollectionRef {
            anthology: self,
            collection,
        }))
    }

    /// All collections with snapshot navigation.
    pub fn collection_refs(&self) -> impl ExactSizeIterator<Item = CollectionRef<'_>> {
        self.collections().map(|collection| CollectionRef {
            anthology: self,
            collection,
        })
    }

    /// Official event-index lookup, including implicit venue/year events.
    pub fn get_event(&self, id: &str) -> Option<EventGroup<'_>> {
        self.event(id)
    }

    /// Eagerly build every derived index, reporting identity and duplicate-key
    /// failures.
    pub fn load_all(&self) -> Result<(), String> {
        self.person_index().map_err(|e| e.to_string())?;
        self.bibkey_index()?;
        Ok(())
    }
}

pub(super) fn parse_ingest_date(value: &str) -> Result<chrono::NaiveDate, String> {
    if value.len() != 10
        || !value
            .get(..4)
            .is_some_and(|s| s.bytes().all(|c| c.is_ascii_digit()))
    {
        return Err(format!("invalid ingestion date {value}"));
    }
    let date = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|e| e.to_string())?;
    if chrono::Datelike::year(&date) < 1 {
        return Err("date year must be positive".into());
    }
    Ok(date)
}
