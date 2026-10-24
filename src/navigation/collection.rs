//! Collection views, source provenance and contained publications.

use super::{PaperRef, VolumeRef};
use crate::{Anthology, Collection};

/// A collection bound to its snapshot, source path and derived navigation.
#[derive(Debug, Clone, Copy)]
pub struct CollectionRef<'a> {
    pub(crate) anthology: &'a Anthology,
    pub(crate) collection: &'a Collection,
}

impl std::ops::Deref for CollectionRef<'_> {
    type Target = Collection;

    fn deref(&self) -> &Collection {
        self.collection
    }
}

impl<'a> CollectionRef<'a> {
    /// Containing snapshot.
    pub fn root(self) -> &'a Anthology {
        self.anthology
    }

    /// Stored metadata.
    pub fn stored(self) -> &'a Collection {
        self.collection
    }

    /// Original collection source path.
    pub fn path(self) -> std::path::PathBuf {
        self.anthology
            .collection_source_path(&self.collection.id)
            .expect("bound collection source")
    }

    /// All volumes in XML order.
    pub fn volumes(self) -> impl ExactSizeIterator<Item = VolumeRef<'a>> {
        self.collection.volumes.iter().map(move |volume| VolumeRef {
            anthology: self.anthology,
            volume,
        })
    }

    /// All papers in XML order.
    pub fn papers(self) -> impl Iterator<Item = PaperRef<'a>> {
        self.volumes().flat_map(VolumeRef::papers)
    }

    /// Lookup a volume by its local, source-preserved identifier.
    pub fn get(self, id: &str) -> Option<VolumeRef<'a>> {
        self.volumes().find(|v| v.local_id == id)
    }

    /// The explicitly stored event definition, if any.
    pub fn event(self) -> Option<&'a crate::Event> {
        self.collection.event.as_ref()
    }

    /// Validate the original source file against the pinned Relax NG schema.
    pub fn validate_schema(self) -> Result<(), crate::schema::SchemaError> {
        crate::schema::validate_file(self.path())
    }

    /// Successful immutable snapshots contain fully loaded, unmodified collections.
    pub fn is_data_loaded(self) -> bool {
        true
    }

    /// Records are never edited through this reference.
    pub fn is_modified(self) -> bool {
        false
    }

    /// A source error prevents construction of a snapshot.
    pub fn raised_error_on_load(self) -> bool {
        false
    }
}
