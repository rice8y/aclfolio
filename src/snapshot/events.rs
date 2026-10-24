//! Event views preserve explicit definitions and expose derived associations.

use super::Anthology;
use crate::{Collection, CollectionId, Event, Volume, VolumeId, source::hash};
use serde::Serialize;

/// All explicit definitions of an event, including historical split
/// collections.
/// Definitions are preserved separately; conflicting metadata is never
/// overwritten.
#[derive(Debug, Clone, Copy)]
pub struct EventGroup<'a> {
    pub(crate) anthology: &'a Anthology,
    pub(super) id: &'a str,
    pub(super) collections: &'a [CollectionId],
}

impl<'a> EventGroup<'a> {
    /// Event ID shared by all definitions.
    pub fn id(self) -> &'a str {
        self.id
    }

    /// Stored definitions in collection-ID order.
    pub fn definitions(self) -> impl ExactSizeIterator<Item = &'a Event> {
        self.collections.iter().map(|c| {
            self.anthology.collections[c]
                .event
                .as_ref()
                .expect("indexed event")
        })
    }

    /// Owning collections in ID order.
    pub fn collections(self) -> impl ExactSizeIterator<Item = &'a Collection> {
        self.collections
            .iter()
            .map(|c| self.anthology.collections[c].as_ref())
    }

    /// Distinct direct, colocated and venue/year-derived volumes in ID order.
    pub fn volumes(self) -> impl Iterator<Item = &'a Volume> {
        self.associations()
            .filter_map(move |(id, _)| self.anthology.volume_by_id(id))
    }

    /// Volume associations and their strongest source. Explicit colocation wins.
    pub fn associations(self) -> impl Iterator<Item = (&'a VolumeId, EventAssociation)> {
        self.anthology
            .indices
            .event_volumes
            .get(self.id)
            .into_iter()
            .flat_map(|m| m.iter())
            .map(|(id, source)| (id, *source))
    }

    /// Whether at least one XML event block explicitly defines this event.
    pub fn is_explicit(self) -> bool {
        !self.collections.is_empty()
    }

    /// Derived display title for an implicit event, if the venue is in the
    /// directory.
    /// Explicit definitions' titles remain accessible without overwriting
    /// conflicts.
    pub fn inferred_title(self) -> Option<String> {
        if self.is_explicit() {
            return None;
        }
        let (venue, year) = self.id.rsplit_once('-')?;
        Some(
            format!("{} ({year})", self.anthology.venue(venue)?.name)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
        )
    }

    pub(super) fn signature(self) -> String {
        hash(
            &serde_json::to_vec(&(
                self.definitions().collect::<Vec<_>>(),
                self.associations().collect::<Vec<_>>(),
                self.inferred_title(),
            ))
            .expect("serializable event"),
        )
    }
}

/// Provenance of an event-to-volume association, strongest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum EventAssociation {
    /// Explicit `colocated/volume-id` reference.
    Explicit,
    /// Volume in a collection declaring this event.
    Collection,
    /// Derived from volume venue and publication year.
    VenueYear,
}
