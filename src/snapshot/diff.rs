//! Semantic snapshot comparison, including inherited publication metadata.

use super::Anthology;
use crate::PaperRef;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Write},
    path::PathBuf,
};

/// Canonical entity changes, independent of source filenames.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RecordChanges {
    /// Identifiers newly present.
    pub added: Vec<String>,
    /// Identifiers no longer present.
    pub removed: Vec<String>,
    /// Identifiers whose stored or effective metadata changed.
    pub modified: Vec<String>,
}

/// Deterministic semantic and source changes from an older to a newer snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SnapshotDiff {
    /// All changed input paths (added, modified, removed).
    pub changed_sources: Vec<PathBuf>,
    /// True when source bytes changed but no modeled entity changed.
    pub source_only: bool,
    /// Paper changes include inherited volume metadata.
    pub papers: RecordChanges,
    /// Volume changes include their paper records.
    pub volumes: RecordChanges,
    /// Collection changes include their descendants.
    pub collections: RecordChanges,
    /// Explicit event changes.
    pub events: RecordChanges,
    /// People directory changes.
    pub people: RecordChanges,
    /// Venue directory changes.
    pub venues: RecordChanges,
    /// SIG directory changes.
    pub sigs: RecordChanges,
}

impl Anthology {
    /// Compare this older snapshot with `newer`. Inheritance changes count as paper
    /// modifications. JSON identity changes appear in their own directory change
    /// set.
    pub fn diff(&self, newer: &Self) -> SnapshotDiff {
        // Compare stored paper plus inherited metadata, without copying corpus records.
        let paper_map = |snapshot: &Self| {
            snapshot
                .papers()
                .map(|paper| (paper.id().to_string(), paper_fingerprint(paper)))
                .collect()
        };
        let mut diff = SnapshotDiff {
            changed_sources: self
                .provenance
                .files
                .keys()
                .chain(newer.provenance.files.keys())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .filter(|k| self.provenance.files.get(*k) != newer.provenance.files.get(*k))
                .cloned()
                .collect(),
            papers: changes(paper_map(self), paper_map(newer)),
            volumes: changes(
                self.volumes()
                    .map(|volume| (volume.id.to_string(), Stored(volume)))
                    .collect(),
                newer
                    .volumes()
                    .map(|volume| (volume.id.to_string(), Stored(volume)))
                    .collect(),
            ),
            collections: changes(
                self.collections()
                    .map(|collection| (collection.id.to_string(), Stored(collection)))
                    .collect(),
                newer
                    .collections()
                    .map(|collection| (collection.id.to_string(), Stored(collection)))
                    .collect(),
            ),
            events: changes(
                self.events()
                    .map(|event| (event.id().to_owned(), event.signature()))
                    .collect(),
                newer
                    .events()
                    .map(|event| (event.id().to_owned(), event.signature()))
                    .collect(),
            ),
            people: changes(
                self.people
                    .iter()
                    .map(|(key, person)| (key.clone(), Stored(person)))
                    .collect(),
                newer
                    .people
                    .iter()
                    .map(|(key, person)| (key.clone(), Stored(person)))
                    .collect(),
            ),
            venues: changes(
                self.venues
                    .iter()
                    .map(|(key, venue)| (key.clone(), venue))
                    .collect(),
                newer
                    .venues
                    .iter()
                    .map(|(key, venue)| (key.clone(), venue))
                    .collect(),
            ),
            sigs: changes(
                self.sigs
                    .iter()
                    .map(|(key, sig)| (key.clone(), sig))
                    .collect(),
                newer
                    .sigs
                    .iter()
                    .map(|(key, sig)| (key.clone(), sig))
                    .collect(),
            ),
            source_only: false,
        };
        diff.source_only = !diff.changed_sources.is_empty()
            && [
                &diff.papers,
                &diff.volumes,
                &diff.collections,
                &diff.events,
                &diff.people,
                &diff.venues,
                &diff.sigs,
            ]
            .iter()
            .all(|changes| {
                changes.added.is_empty()
                    && changes.removed.is_empty()
                    && changes.modified.is_empty()
            });
        diff
    }
}

fn paper_fingerprint(paper: PaperRef<'_>) -> String {
    // Serde to a digest writer avoids retaining a second full corpus
    // representation.
    let mut writer = HashWriter(Sha256::new());
    serde_json::to_writer(
        &mut writer,
        &(
            paper.stored(),
            &paper.volume.metadata,
            &paper.volume.ingest_date,
            &paper.volume.kind,
        ),
    )
    .expect("infallible record serialization");
    format!("{:x}", writer.0.finalize())
}

fn changes<T: PartialEq>(older: BTreeMap<String, T>, newer: BTreeMap<String, T>) -> RecordChanges {
    RecordChanges {
        added: newer
            .keys()
            .filter(|key| !older.contains_key(*key))
            .cloned()
            .collect(),
        removed: older
            .keys()
            .filter(|key| !newer.contains_key(*key))
            .cloned()
            .collect(),
        modified: older
            .iter()
            .filter(|(key, value)| newer.get(*key).is_some_and(|other| other != *value))
            .map(|(key, _)| key.clone())
            .collect(),
    }
}

// Name equality ignores script and empty/absent given names. Snapshot changes
// must still distinguish their stored representations.
struct Stored<'a, T>(&'a T);

impl<T: Serialize> PartialEq for Stored<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.0, other.0)
            || serde_json::to_vec(self.0).expect("serializable metadata")
                == serde_json::to_vec(other.0).expect("serializable metadata")
    }
}

struct HashWriter(Sha256);

impl Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
