//! Stored files metadata.

use serde::{Deserialize, Serialize};

/// A stored PDF checksum. Absence of this record means no stored PDF reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfReference {
    /// Stored CRC32 checksum, distinct from source provenance hashes.
    pub hash: String,
}

/// A URL and its optional role, in source order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UrlReference {
    /// Optional source type annotation.
    pub kind: Option<String>,
    /// Web address as recorded in the source.
    pub url: String,
    /// Optional CRC32 checksum of the referenced resource.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
}

/// A local attachment reference; the library never opens it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    /// Optional source type annotation.
    pub kind: Option<String>,
    /// Stored CRC32 checksum, distinct from source provenance hashes.
    pub hash: String,
    /// Attachment filename, without resolving a URL template.
    pub filename: String,
}

/// A paper revision, retaining its original identifier, date and description.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revision {
    /// Revision suffix used to derive the revised PDF filename.
    pub id: String,
    /// Date as recorded in the source.
    pub date: Option<String>,
    /// Stored CRC32 checksum, distinct from source provenance hashes.
    pub hash: String,
    /// Revision note; an empty string represents no explanatory text.
    pub text: String,
}

/// A separately published erratum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Erratum {
    /// Erratum suffix used to derive the PDF filename.
    pub id: String,
    /// Date as recorded in the source.
    pub date: Option<String>,
    /// Stored CRC32 checksum, distinct from source provenance hashes.
    pub hash: Option<String>,
}

/// A retraction or removal notice; both can be present independently.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notice {
    /// Date as recorded in the source.
    pub date: String,
    /// Explanation of the removal or retraction.
    pub text: String,
}

/// A video link with an optional explicit permission flag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Video {
    /// Video URL or event-local reference.
    pub href: String,
    /// Explicit distribution permission; absence leaves the default to the view.
    pub permission: Option<bool>,
}

/// A named award and its description.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Award {
    /// Award category or display name.
    pub name: String,
    /// Award reasoning; an empty string represents absent reasoning.
    pub text: String,
}

/// Historical machine-readable-format reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mrf {
    /// Source location of the historical machine-readable resource.
    pub src: String,
    /// Text content of the machine-readable-format element.
    pub value: String,
}

impl Award {
    /// Optional award reasoning as interpreted by the Python XML constructor.
    pub fn reasoning(&self) -> Option<&str> {
        (!self.text.is_empty()).then_some(&self.text)
    }
}

impl Revision {
    /// Optional revision note.
    pub fn note(&self) -> Option<&str> {
        (!self.text.is_empty()).then_some(&self.text)
    }
}

impl Notice {
    /// Optional explanatory note.
    pub fn note(&self) -> Option<&str> {
        (!self.text.is_empty()).then_some(&self.text)
    }
}
