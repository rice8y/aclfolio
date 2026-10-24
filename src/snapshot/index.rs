//! Eager publication and relationship indices derived from stored records.

use super::{CollectionSource, EventAssociation};
use crate::error::parse_error;
use crate::{AnthologyId, Collection, CollectionId, Error, PaperId, Person, VolumeId};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::Arc,
};

#[derive(Debug, Clone, Default)]
pub(super) struct Indices {
    pub(super) volumes: BTreeMap<VolumeId, (CollectionId, usize)>,
    pub(super) papers: BTreeMap<PaperId, (VolumeId, usize)>,
    pub(super) events: BTreeMap<String, Vec<CollectionId>>,
    pub(super) event_volumes: BTreeMap<String, BTreeMap<VolumeId, EventAssociation>>,
    pub(super) volume_events: BTreeMap<VolumeId, BTreeSet<String>>,
    pub(super) works: BTreeMap<String, BTreeSet<AnthologyId>>,
    pub(super) venue_volumes: BTreeMap<String, BTreeSet<VolumeId>>,
    pub(super) sig_volumes: BTreeMap<String, BTreeSet<VolumeId>>,
}

impl Indices {
    pub(super) fn build(
        collections: &BTreeMap<CollectionId, Arc<Collection>>,
        sources: &BTreeMap<PathBuf, CollectionSource>,
        people: &BTreeMap<String, Person>,
    ) -> Result<Self, Error> {
        let mut indices = Self::default();
        for (file, source) in sources {
            let cid = &source.id;
            let c = &collections[cid];
            if let Some(e) = &c.event {
                indices
                    .events
                    .entry(e.id.clone())
                    .or_default()
                    .push(cid.clone());
                let associations = indices.event_volumes.entry(e.id.clone()).or_default();
                for v in &c.volumes {
                    associations
                        .entry(v.id.clone())
                        .and_modify(|origin| {
                            if *origin == EventAssociation::VenueYear {
                                *origin = EventAssociation::Collection;
                            }
                        })
                        .or_insert(EventAssociation::Collection);
                }
                for id in e.colocated.iter().flatten() {
                    associations.insert(id.clone(), EventAssociation::Explicit);
                }
            }
            for (vi, v) in c.volumes.iter().enumerate() {
                if indices
                    .volumes
                    .insert(v.id.clone(), (cid.clone(), vi))
                    .is_some()
                {
                    return Err(parse_error(file, format!("duplicate volume {}", v.id)));
                }
                for venue in &v.metadata.venues {
                    let event_id = format!("{venue}-{}", v.metadata.year);
                    indices.events.entry(event_id.clone()).or_default();
                    indices
                        .event_volumes
                        .entry(event_id)
                        .or_default()
                        .entry(v.id.clone())
                        .or_insert(EventAssociation::VenueYear);
                    indices
                        .venue_volumes
                        .entry(venue.clone())
                        .or_default()
                        .insert(v.id.clone());
                }
                for sig in &v.metadata.sigs {
                    indices
                        .sig_volumes
                        .entry(sig.clone())
                        .or_default()
                        .insert(v.id.clone());
                }
                for credit in &v.metadata.editors {
                    if let Some(id) = &credit.id {
                        if people.contains_key(id) {
                            indices
                                .works
                                .entry(id.clone())
                                .or_default()
                                .insert(AnthologyId::Volume(v.id.clone()));
                        }
                    }
                }
                for (pi, p) in v.papers.iter().enumerate() {
                    if indices
                        .papers
                        .insert(p.id.clone(), (v.id.clone(), pi))
                        .is_some()
                    {
                        return Err(parse_error(file, format!("duplicate paper {}", p.id)));
                    }
                    for credit in p.authors.iter().chain(&p.editors) {
                        if let Some(id) = &credit.id {
                            if people.contains_key(id) {
                                indices
                                    .works
                                    .entry(id.clone())
                                    .or_default()
                                    .insert(AnthologyId::Paper(p.id.clone()));
                            }
                        }
                    }
                }
            }
        }
        for (event, volumes) in &indices.event_volumes {
            for id in volumes.keys() {
                indices
                    .volume_events
                    .entry(id.clone())
                    .or_default()
                    .insert(event.clone());
            }
        }
        for collections in indices.events.values_mut() {
            collections.sort();
        }
        Ok(indices)
    }
}
