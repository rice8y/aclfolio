//! Stored events metadata.

use super::{NameSpecification, UrlReference};
use crate::{ids::VolumeId, text::RichText};
use serde::{Deserialize, Serialize};

/// Event metadata; an absent meta block is distinct from an empty one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventMetadata {
    /// Explicit event title, when present.
    pub title: Option<RichText>,
    /// Event location as recorded.
    pub location: Option<String>,
    /// Event date expression as recorded, without calendar coercion.
    pub dates: Option<String>,
}

/// An invited talk, which need not have a paper record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Talk {
    /// Optional source type annotation.
    pub kind: Option<String>,
    /// Talk title with inline markup.
    pub title: RichText,
    /// Speaker credits in source order.
    pub speakers: Vec<NameSpecification>,
    /// First talk link; additional links follow in `additional_urls`.
    pub url: Option<UrlReference>,
    /// Links after the first, preserving source order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_urls: Vec<UrlReference>,
}

/// An explicitly stored event, including colocation references.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// Identifier as recorded in the source.
    pub id: String,
    /// Optional meta block; absence differs from a present empty block.
    pub metadata: Option<EventMetadata>,
    /// Event links; absence differs from an empty XML wrapper.
    pub links: Option<Vec<UrlReference>>,
    /// Invited talks in source order.
    pub talks: Vec<Talk>,
    /// Colocated volume IDs; absence differs from an empty XML wrapper.
    pub colocated: Option<Vec<VolumeId>>,
}
