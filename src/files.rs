//! File references, CRC32 checksums and optional HTTP downloads.

use crate::{
    config::Config,
    utils::{xml_attribute, xml_escape},
};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Reference category determines its URL template and expected media type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileKind {
    /// PDF document.
    Pdf,
    /// PDF thumbnail JPEG.
    Thumbnail,
    /// Ancillary file.
    Attachment,
    /// External link or event file.
    Url,
    /// Video recording.
    Video,
}

/// A file reference with an optional CRC32 checksum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileReference {
    /// Local name or full URL.
    pub name: String,
    /// Expected eight-digit hexadecimal CRC32, when known.
    pub checksum: Option<String>,
    /// Reference category.
    pub kind: FileKind,
    /// Video distribution permission (defaults to true).
    pub permission: bool,
}

/// Compute the lowercase eight-digit CRC32 used by Anthology metadata.
pub fn compute_checksum(bytes: &[u8]) -> String {
    format!("{:08x}", crc32fast::hash(bytes))
}

/// Compute a file's CRC32, returning filesystem errors unchanged.
pub fn compute_checksum_from_file(path: impl AsRef<Path>) -> std::io::Result<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = crc32fast::Hasher::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(format!("{:08x}", hasher.finalize()))
}

impl FileReference {
    /// Create a detached reference.
    pub fn new(kind: FileKind, name: impl Into<String>, checksum: Option<String>) -> Self {
        Self {
            kind,
            name: name.into(),
            checksum,
            permission: true,
        }
    }

    /// Whether the reference lacks a URI scheme separator.
    pub fn is_local(&self) -> bool {
        !self.name.contains("://")
    }

    /// Expected HTTP content type, if enforced by this reference kind.
    pub fn content_type(&self) -> Option<&'static str> {
        (self.kind == FileKind::Pdf).then_some("application/pdf")
    }

    /// Configuration field used to resolve a local reference.
    pub fn template_field(&self) -> &'static str {
        match self.kind {
            FileKind::Pdf => "pdf_location_template",
            FileKind::Thumbnail => "pdf_thumbnail_location_template",
            FileKind::Attachment => "attachment_location_template",
            FileKind::Url => "event_location_template",
            FileKind::Video => "video_location_template",
        }
    }

    fn template<'a>(&self, config: &'a Config) -> &'a str {
        match self.kind {
            FileKind::Pdf => &config.pdf_location_template,
            FileKind::Thumbnail => &config.pdf_thumbnail_location_template,
            FileKind::Attachment => &config.attachment_location_template,
            FileKind::Url => &config.event_location_template,
            FileKind::Video => &config.video_location_template,
        }
    }

    /// Resolve this reference with explicit URL configuration.
    pub fn url(&self, config: &Config) -> String {
        if self.is_local() {
            config.format(self.template(config), &self.name)
        } else {
            self.name.clone()
        }
    }

    /// Compute a checksum and infer the stored basename using the URL template
    /// suffix.
    pub fn from_file(
        kind: FileKind,
        path: impl AsRef<Path>,
        config: &Config,
    ) -> std::io::Result<Self> {
        let path = path.as_ref();
        if !path.is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "not a file",
            ));
        }
        let mut result = Self::new(kind, "", None);
        let suffix = path
            .extension()
            .map(|s| format!(".{}", s.to_string_lossy()))
            .unwrap_or_default();
        let name = if result.template(config).ends_with(&suffix) {
            path.file_stem()
        } else {
            path.file_name()
        };
        result.name = name.unwrap_or_default().to_string_lossy().into_owned();
        result.checksum = Some(compute_checksum_from_file(path)?);
        Ok(result)
    }

    /// Parse a detached reference from an XML element.
    pub fn from_xml(kind: FileKind, xml: &str) -> Result<Self, String> {
        let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
        let e = doc.root_element();
        let mut r = Self::new(
            kind,
            if kind == FileKind::Video {
                e.attribute("href").unwrap_or("None")
            } else {
                e.text().unwrap_or("")
            },
            e.attribute("hash")
                .filter(|s| !s.is_empty())
                .map(str::to_owned),
        );
        if kind == FileKind::Video {
            r.permission = e
                .attribute("permission")
                .map(crate::utils::xsd_boolean)
                .transpose()
                .map_err(|e| e.to_string())?
                .unwrap_or(true);
            r.checksum = None;
        }
        Ok(r)
    }

    /// Serialize a detached reference; PDF/video use their schema-mandated tags.
    pub fn to_xml(&self, tag: &str) -> String {
        if self.kind == FileKind::Pdf {
            return format!(
                "<pdf hash=\"{}\"/>",
                xml_attribute(self.checksum.as_deref().unwrap_or("None"))
            );
        }
        if self.kind == FileKind::Video {
            return format!(
                "<video href=\"{}\"{}/>",
                xml_attribute(&self.name),
                if self.permission {
                    ""
                } else {
                    " permission=\"false\""
                }
            );
        }
        let hash = if self.is_local() {
            self.checksum
                .as_ref()
                .map(|s| format!(" hash=\"{}\"", xml_attribute(s)))
                .unwrap_or_default()
        } else {
            String::new()
        };
        format!("<{tag}{hash}>{}</{tag}>", xml_escape(&self.name))
    }

    /// Reject local references where the schema requires an external URL.
    pub fn validate_external(&self) -> Result<(), String> {
        if self.kind == FileKind::Url && !self.is_local() {
            Ok(())
        } else {
            Err("expected non-local URL reference".into())
        }
    }
}

/// Download result preserves Python's non-fatal checksum warning explicitly.
#[cfg(feature = "network")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadOutcome {
    /// Existing file already matched the expected checksum.
    AlreadyPresent,
    /// Downloaded successfully, with no checksum mismatch.
    Downloaded,
    /// Downloaded and saved, but the expected checksum did not match.
    ChecksumMismatch,
}

/// Errors downloading a referenced file.
#[cfg(feature = "network")]
#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    /// HTTP, TLS, status or timeout error.
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    /// Filesystem error.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Unexpected content type; the destination was not overwritten.
    #[error("expected {expected}, got {actual:?}")]
    ContentType {
        /// Expected media type.
        expected: String,
        /// Actual response header.
        actual: Option<String>,
    },
}

#[cfg(feature = "network")]
impl FileReference {
    /// Download to an explicit path, honoring timeouts, content type and CRC32.
    /// An existing matching file is reused; otherwise the path is overwritten.
    pub fn download(
        &self,
        path: impl AsRef<Path>,
        config: &Config,
        timeout: std::time::Duration,
    ) -> Result<DownloadOutcome, DownloadError> {
        let path = path.as_ref();
        if path.exists()
            && self.checksum.is_some()
            && Some(compute_checksum_from_file(path)?) == self.checksum
        {
            return Ok(DownloadOutcome::AlreadyPresent);
        }
        let response = reqwest::blocking::Client::builder()
            .timeout(timeout)
            .build()?
            .get(self.url(config))
            .send()?
            .error_for_status()?;
        if let Some(expected) = self.content_type() {
            let actual = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|x| x.to_str().ok())
                .map(str::to_owned);
            if actual.as_deref() != Some(expected) {
                return Err(DownloadError::ContentType {
                    expected: expected.into(),
                    actual,
                });
            }
        }
        let bytes = response.bytes()?;
        let mismatch = self
            .checksum
            .as_ref()
            .is_some_and(|h| *h != compute_checksum(&bytes));
        std::fs::write(path, &bytes)?;
        Ok(if mismatch {
            DownloadOutcome::ChecksumMismatch
        } else {
            DownloadOutcome::Downloaded
        })
    }
}

impl crate::UrlReference {
    /// Convert stored URL metadata to a downloadable reference.
    pub fn file_reference(&self) -> FileReference {
        FileReference::new(FileKind::Url, &self.url, self.checksum.clone())
    }
}

impl crate::Attachment {
    /// Convert a stored attachment to a downloadable reference.
    pub fn file_reference(&self) -> FileReference {
        FileReference::new(
            FileKind::Attachment,
            &self.filename,
            (!self.hash.is_empty()).then(|| self.hash.clone()),
        )
    }
}

impl crate::Video {
    /// Convert a video to a downloadable reference, with default permission true.
    pub fn file_reference(&self) -> FileReference {
        let mut r = FileReference::new(FileKind::Video, &self.href, None);
        r.permission = self.permission.unwrap_or(true);
        r
    }
}
