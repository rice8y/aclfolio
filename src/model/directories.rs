//! Stored directories metadata.

use super::{Extensions, Name};
use serde::{Deserialize, Serialize};

/// A people-directory record. Its map key is the person identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Person {
    /// Names in directory order; the first is canonical.
    pub names: Vec<Name>,
    /// ORCID identifier, when explicitly supplied.
    pub orcid: Option<String>,
    /// Free-form directory annotation.
    pub comment: Option<String>,
    /// Academic degree annotation.
    pub degree: Option<String>,
    /// IDs of people the directory marks as potentially confusable.
    #[serde(default)]
    pub similar: Vec<String>,
    /// Whether automatic name-based identity matching is disabled.
    pub disable_name_matching: Option<bool>,
    /// Unrecognized JSON properties preserved during loading.
    #[serde(flatten, default)]
    pub extensions: Extensions,
}

/// A venue-directory record; related IDs imply a symmetric relationship.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Venue {
    /// Short display name.
    pub acronym: String,
    /// Display name as recorded in the source.
    pub name: String,
    /// Whether this venue is associated with ACL.
    pub is_acl: Option<bool>,
    /// Whether this venue is a top-level directory entry.
    pub is_toplevel: Option<bool>,
    /// Historical single-letter publication prefix.
    pub oldstyle_letter: Option<String>,
    /// Web address as recorded in the source.
    pub url: Option<String>,
    /// Directory description.
    pub description: Option<String>,
    /// Related directory IDs; navigation also follows reverse links.
    #[serde(default)]
    pub related_ids: Vec<String>,
    /// Optional source type annotation.
    #[serde(rename = "type")]
    pub kind: Option<String>,
    /// Unrecognized JSON properties preserved during loading.
    #[serde(flatten, default)]
    pub extensions: Extensions,
}

/// A meeting held outside the Anthology.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalMeeting {
    /// Publication year as a string; never inferred from ingestion time.
    pub year: String,
    /// Display name as recorded in the source.
    pub name: String,
    /// Web address as recorded in the source.
    pub url: Option<String>,
    /// Unrecognized JSON properties preserved during loading.
    #[serde(flatten, default)]
    pub extensions: Extensions,
}

/// A special interest group; venue associations are hints, not volume
/// membership.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sig {
    /// Short display name.
    pub acronym: String,
    /// Display name as recorded in the source.
    pub name: String,
    /// Web address as recorded in the source.
    pub url: Option<String>,
    /// Directory description.
    pub description: Option<String>,
    /// Related directory IDs; navigation also follows reverse links.
    #[serde(default)]
    pub related_ids: Vec<String>,
    /// Associated venue IDs, which do not imply volume membership.
    #[serde(default)]
    pub venue_ids: Vec<String>,
    /// Meetings listed outside the Anthology.
    #[serde(default)]
    pub external_meetings: Vec<ExternalMeeting>,
    /// Unrecognized JSON properties preserved during loading.
    #[serde(flatten, default)]
    pub extensions: Extensions,
}
