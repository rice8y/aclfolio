//! Stored publications metadata.

use super::{
    Attachment, Award, Erratum, Event, Mrf, NameSpecification, Notice, PdfReference, Revision,
    UrlReference, Video,
};
use crate::{
    ids::{CollectionId, PaperId, VolumeId},
    text::RichText,
};
use serde::{Deserialize, Serialize};

/// Volume metadata including publication year (not ingestion year).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumeMetadata {
    /// Full volume title, preserving inline markup.
    pub booktitle: RichText,
    /// Abbreviated volume title, when present.
    pub shortbooktitle: Option<RichText>,
    /// Editor credits in source order.
    pub editors: Vec<NameSpecification>,
    /// Publisher as recorded, including a present empty value.
    pub publisher: Option<String>,
    /// Publication location as recorded.
    pub address: Option<String>,
    /// Month expression, possibly a range or a nonstandard spelling.
    pub month: Option<String>,
    /// Publication year as a string; never inferred from ingestion time.
    pub year: String,
    /// ISBN as recorded; not a paper-level DOI.
    pub isbn: Option<String>,
    /// Stored PDF reference; absence does not imply an available PDF.
    pub pdf: Option<PdfReference>,
    /// Links in source order, including repeated roles.
    pub urls: Vec<UrlReference>,
    /// Explicit DOI as recorded, including a present empty value.
    pub doi: Option<String>,
    /// Explicit SIG IDs; venue associations are not inherited here.
    pub sigs: Vec<String>,
    /// Explicit venue IDs in source order.
    pub venues: Vec<String>,
    /// Journal volume number as recorded.
    pub journal_volume: Option<String>,
    /// Journal issue number as recorded.
    pub journal_issue: Option<String>,
    /// Explicit journal title, before venue-based fallback.
    pub journal_title: Option<String>,
}

/// One paper or front matter record. Front matter has no stored title.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Paper {
    /// Full validated paper identifier.
    pub id: PaperId,
    /// Unpadded identifier within the parent, preserving source spelling.
    pub local_id: String,
    /// Whether this is the volume front matter with local ID zero.
    pub frontmatter: bool,
    /// Optional source type annotation.
    pub kind: Option<String>,
    /// Explicit ingestion date; inheritance is available through a bound view.
    pub ingest_date: Option<String>,
    /// Stored title; front matter instead uses the volume title through `PaperRef`.
    pub title: Option<RichText>,
    /// Abstract with inline markup; absence and empty content are distinct.
    pub abstract_text: Option<RichText>,
    /// Stored citation key, without generation or collision resolution.
    pub bibkey: String,
    /// Author credits in source order.
    pub authors: Vec<NameSpecification>,
    /// Editor credits in source order.
    pub editors: Vec<NameSpecification>,
    /// Explicit DOI as recorded, including a present empty value.
    pub doi: Option<String>,
    /// Page expression as recorded, before range normalization.
    pub pages: Option<String>,
    /// Paper-level journal issue override.
    pub issue: Option<String>,
    /// Paper-level journal title override.
    pub journal: Option<String>,
    /// Publication language code as recorded.
    pub language: Option<String>,
    /// Month expression, possibly a range or a nonstandard spelling.
    pub month: Option<String>,
    /// Optional bibliographic note.
    pub note: Option<String>,
    /// Historical machine-readable-format reference.
    pub mrf: Option<Mrf>,
    /// Stored PDF reference; absence does not imply an available PDF.
    pub pdf: Option<PdfReference>,
    /// Links in source order, including repeated roles.
    pub urls: Vec<UrlReference>,
    /// Ancillary files in source order.
    pub attachments: Vec<Attachment>,
    /// Revised PDFs in source order.
    pub revisions: Vec<Revision>,
    /// Erratum PDFs in source order.
    pub errata: Vec<Erratum>,
    /// Retraction notice, retained independently of removal.
    pub retracted: Option<Notice>,
    /// Removal notice, retained independently of retraction.
    pub removed: Option<Notice>,
    /// Kind of the last deletion notice in XML order; determines the effective
    /// notice.
    pub last_deletion: Option<crate::types::DeletionKind>,
    /// Video references in source order.
    pub videos: Vec<Video>,
    /// Awards in source order.
    pub awards: Vec<Award>,
}

/// A proceedings or journal volume. Paper order follows XML source order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Volume {
    /// Full validated volume identifier.
    pub id: VolumeId,
    /// Unpadded identifier within the parent, preserving source spelling.
    pub local_id: String,
    /// Volume type, usually `proceedings` or `journal`.
    pub kind: String,
    /// Explicit ingestion date; inheritance is available through a bound view.
    pub ingest_date: Option<String>,
    /// Metadata inherited by contained papers through their bound views.
    pub metadata: VolumeMetadata,
    /// Papers and front matter in XML order.
    pub papers: Vec<Paper>,
}

/// One XML collection. Child records retain source order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collection {
    /// Full validated collection identifier.
    pub id: CollectionId,
    /// Volumes in XML order.
    pub volumes: Vec<Volume>,
    /// Explicit event definition, if the collection contains one.
    pub event: Option<Event>,
}
