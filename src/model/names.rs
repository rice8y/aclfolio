//! Stored names metadata.

use super::Extensions;
use serde::{Deserialize, Serialize};

/// A recorded name. Equality compares name components, ignoring script
/// metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Name {
    /// Optional given name, preserving its stored spelling.
    pub first: Option<String>,
    /// Family name.
    pub last: String,
    /// Writing system metadata.
    pub script: Option<String>,
    /// Additional source metadata.
    #[serde(flatten, default)]
    pub extensions: Extensions,
}

impl PartialEq for Name {
    fn eq(&self, other: &Self) -> bool {
        self.first.as_deref().filter(|s| !s.is_empty())
            == other.first.as_deref().filter(|s| !s.is_empty())
            && self.last == other.last
    }
}

impl Eq for Name {}

impl std::hash::Hash for Name {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.first.as_deref().filter(|s| !s.is_empty()).hash(state);
        self.last.hash(state);
    }
}

/// An ordered authorship/editor/speaker credit, independent of directory
/// identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NameSpecification {
    /// Explicit person ID; the no-person sentinel disables ID association.
    pub id: Option<String>,
    /// ORCID identifier, when explicitly supplied.
    pub orcid: Option<String>,
    /// OpenReview profile identifier.
    pub openreview: Option<String>,
    /// Name printed on this publication or talk.
    pub name: Name,
    /// First alternate-script name, retained for direct access.
    pub variant: Option<Name>,
    /// Remaining alternate-script names in source order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_variants: Vec<Name>,
    /// Affiliation attached to this credit.
    pub affiliation: Option<String>,
}

impl NameSpecification {
    /// All non-Latin variants in XML order.
    pub fn variants(&self) -> impl Iterator<Item = &Name> {
        self.variant.iter().chain(&self.additional_variants)
    }
}
