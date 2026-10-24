//! Publication kinds and reference sentinels.

/// Local ID reserved for frontmatter.
pub const FRONTMATTER_ID: &str = "0";

/// Missing citation-key sentinel.
pub const NO_BIBKEY: &str = "__NO_BIBKEY__";

/// Sentinel that deliberately disconnects a name specification from an explicit
/// identity.
pub const NO_PERSON_ID: &str = "__NO_PERSON_ID__";

/// Default ingestion date.
pub const UNKNOWN_INGEST_DATE: &str = "1900-01-01";

/// The kind of a publication record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaperType {
    /// Regular paper.
    Paper,
    /// Volume frontmatter.
    Frontmatter,
    /// Backmatter.
    Backmatter,
}

/// The kind of a volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VolumeType {
    /// Journal issue.
    Journal,
    /// Conference or workshop proceedings.
    Proceedings,
}

impl std::str::FromStr for PaperType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "paper" => Ok(Self::Paper),
            "frontmatter" => Ok(Self::Frontmatter),
            "backmatter" => Ok(Self::Backmatter),
            _ => Err(format!("unknown paper type {s}")),
        }
    }
}

impl std::str::FromStr for VolumeType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "journal" => Ok(Self::Journal),
            "proceedings" => Ok(Self::Proceedings),
            _ => Err(format!("unknown volume type {s}")),
        }
    }
}

/// Kind of deletion represented by a notice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeletionKind {
    /// Retraction.
    Retracted,
    /// Removal.
    Removed,
}
