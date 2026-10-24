//! Stored metadata records, with source presence and ordering preserved.
//!
//! Snapshot-bound views apply inheritance and resolve relationships in
//! `navigation`.

pub use directories::{ExternalMeeting, Person, Sig, Venue};
pub use events::{Event, EventMetadata, Talk};
pub use files::{
    Attachment, Award, Erratum, Mrf, Notice, PdfReference, Revision, UrlReference, Video,
};
pub use names::{Name, NameSpecification};
pub use publications::{Collection, Paper, Volume, VolumeMetadata};

// Preserve the original public paths while navigation owns effective metadata.
pub use crate::navigation::{DateOrigin, EffectiveDate, PaperRef};

mod directories;
mod events;
mod files;
mod names;
mod publications;

/// Extra JSON fields, retained and diagnosed for forward compatibility.
pub type Extensions = std::collections::BTreeMap<String, serde_json::Value>;
