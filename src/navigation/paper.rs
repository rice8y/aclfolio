//! Borrowed paper views and inherited publication metadata.

use super::{VolumeRef, parse_ingest_date};
use crate::{
    Anthology, Collection, EventGroup,
    files::{FileKind, FileReference},
    ids::IdParts,
};
use crate::{NameSpecification, Paper, PaperId, Volume, text::RichText};
use serde::Serialize;

/// The source of an effective ingestion date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DateOrigin {
    /// Explicit on the paper.
    Paper,
    /// Inherited from volume.
    Volume,
    /// Neither record stores a date.
    Absent,
}

/// A borrowed effective date with its provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EffectiveDate<'a> {
    /// Date as stored in XML; absent dates are never guessed.
    pub value: Option<&'a str>,
    /// Where this value was stored.
    pub origin: DateOrigin,
}

/// A paper bound to its parent in a snapshot, for effective metadata access.
#[derive(Debug, Clone, Copy)]
pub struct PaperRef<'a> {
    pub(crate) paper: &'a Paper,
    pub(crate) volume: &'a Volume,
    pub(crate) anthology: &'a crate::Anthology,
}

impl<'a> PaperRef<'a> {
    /// Stored metadata, with no inheritance applied.
    pub fn stored(self) -> &'a Paper {
        self.paper
    }

    /// Parent volume.
    pub fn volume(self) -> &'a Volume {
        self.volume
    }

    /// Canonical identifier.
    pub fn id(self) -> &'a PaperId {
        &self.paper.id
    }

    /// Title, inherited from the booktitle for front matter.
    pub fn title(self) -> &'a RichText {
        self.paper
            .title
            .as_ref()
            .unwrap_or(&self.volume.metadata.booktitle)
    }

    /// Publication year, inherited from volume metadata.
    pub fn year(self) -> &'a str {
        &self.volume.metadata.year
    }

    /// Effective publication month.
    pub fn month(self) -> Option<&'a str> {
        self.paper
            .month
            .as_deref()
            .filter(|s| !s.is_empty())
            .or(self
                .volume
                .metadata
                .month
                .as_deref()
                .filter(|s| !s.is_empty()))
    }

    /// Effective editor credits, preserving order.
    pub fn editors(self) -> &'a [NameSpecification] {
        if self.paper.editors.is_empty() {
            &self.volume.metadata.editors
        } else {
            &self.paper.editors
        }
    }

    /// Effective ingestion date, never derived from publication or Git time.
    pub fn effective_ingest_date(self) -> EffectiveDate<'a> {
        if let Some(s) = self.paper.ingest_date.as_deref() {
            EffectiveDate {
                value: Some(s),
                origin: DateOrigin::Paper,
            }
        } else if let Some(s) = self.volume.ingest_date.as_deref() {
            EffectiveDate {
                value: Some(s),
                origin: DateOrigin::Volume,
            }
        } else {
            EffectiveDate {
                value: None,
                origin: DateOrigin::Absent,
            }
        }
    }

    /// Canonical landing page; a PDF is not implied.
    pub fn canonical_url(self) -> String {
        self.paper.id.canonical_url()
    }
}

impl std::ops::Deref for PaperRef<'_> {
    type Target = Paper;

    fn deref(&self) -> &Paper {
        self.paper
    }
}

impl<'a> PaperRef<'a> {
    /// Effective journal issue, inheriting when the XML element is empty.
    pub fn journal_issue(self) -> Option<&'a str> {
        self.paper
            .issue
            .as_deref()
            .filter(|s| !s.is_empty())
            .or(self
                .volume
                .metadata
                .journal_issue
                .as_deref()
                .filter(|s| !s.is_empty()))
    }

    /// Effective journal title.
    pub fn journal_title(self) -> Option<&'a str> {
        self.paper
            .journal
            .as_deref()
            .filter(|s| !s.is_empty())
            .or(self.volume_ref().journal_title())
    }

    /// Journal volume inherited from the parent volume.
    pub fn journal_volume(self) -> Option<&'a str> {
        self.volume
            .metadata
            .journal_volume
            .as_deref()
            .filter(|s| !s.is_empty())
    }

    /// Publisher inherited from the parent volume.
    pub fn publisher(self) -> Option<&'a str> {
        self.volume
            .metadata
            .publisher
            .as_deref()
            .filter(|s| !s.is_empty())
    }

    /// Publisher's address inherited from the parent volume.
    pub fn address(self) -> Option<&'a str> {
        self.volume
            .metadata
            .address
            .as_deref()
            .filter(|s| !s.is_empty())
    }

    /// Inherited ISBN, distinct from a paper's DOI.
    pub fn isbn(self) -> Option<&'a str> {
        self.volume
            .metadata
            .isbn
            .as_deref()
            .filter(|s| !s.is_empty())
    }

    /// URL for a stored PDF reference; no link is returned when `<pdf>` is absent.
    pub fn pdf_url(self) -> Option<String> {
        self.paper
            .pdf
            .as_ref()
            .map(|_| format!("https://aclanthology.org/{}.pdf", self.paper.id))
    }
}

impl<'a> PaperRef<'a> {
    /// Snapshot containing this paper.
    pub fn root(self) -> &'a Anthology {
        self.anthology
    }

    /// Parent volume with derived-property navigation.
    pub fn volume_ref(self) -> VolumeRef<'a> {
        VolumeRef {
            anthology: self.anthology,
            volume: self.volume,
        }
    }

    /// Parent collection.
    pub fn collection(self) -> &'a Collection {
        self.volume_ref().collection()
    }

    /// Local, unpadded paper ID.
    pub fn local_id(self) -> String {
        self.paper.local_id.clone()
    }

    /// Full ID decomposed into local components.
    pub fn full_id_tuple(self) -> IdParts {
        IdParts(
            self.volume.id.collection_id().to_string(),
            Some(self.volume.local_id.clone()),
            Some(self.paper.local_id.clone()),
        )
    }

    /// Whether a deletion notice is attached.
    pub fn is_deleted(self) -> bool {
        self.paper.removed.is_some() || self.paper.retracted.is_some()
    }

    /// Whether this record is frontmatter.
    pub fn is_frontmatter(self) -> bool {
        self.paper.frontmatter
    }

    /// Effective ingestion date, including Python's default sentinel.
    pub fn ingest_date(self) -> &'a str {
        self.paper
            .ingest_date
            .as_deref()
            .unwrap_or_else(|| self.volume_ref().ingest_date())
    }

    /// Configured landing page URL.
    pub fn web_url(self) -> String {
        let c = self.anthology.config();
        c.format(&c.paper_page_template, self.id().as_str())
    }

    /// PDF reference with the paper's derived filename.
    pub fn pdf(self) -> Option<FileReference> {
        self.paper.pdf.as_ref().map(|p| {
            FileReference::new(
                FileKind::Pdf,
                self.id().as_str(),
                (!p.hash.is_empty()).then(|| p.hash.clone()),
            )
        })
    }

    /// Thumbnail reference when a PDF is present.
    pub fn thumbnail(self) -> Option<FileReference> {
        self.paper
            .pdf
            .as_ref()
            .map(|_| FileReference::new(FileKind::Thumbnail, self.id().as_str(), None))
    }

    /// Derived erratum PDF references in source order.
    pub fn erratum_files(self) -> Vec<FileReference> {
        self.paper
            .errata
            .iter()
            .map(|e| {
                FileReference::new(
                    FileKind::Pdf,
                    format!("{}e{}", self.id(), e.id),
                    e.hash.clone(),
                )
            })
            .collect()
    }

    /// Derived revision PDF references in source order.
    pub fn revision_files(self) -> Vec<FileReference> {
        self.paper
            .revisions
            .iter()
            .map(|e| {
                FileReference::new(
                    FileKind::Pdf,
                    format!("{}v{}", self.id(), e.id),
                    Some(e.hash.clone()),
                )
            })
            .collect()
    }

    /// Explicit authors and editors; inherited editors are not included.
    pub fn namespecs(self) -> impl Iterator<Item = &'a NameSpecification> {
        self.paper.authors.iter().chain(&self.paper.editors)
    }

    /// Venue IDs inherited from the parent volume.
    pub fn venue_ids(self) -> &'a [String] {
        &self.volume.metadata.venues
    }

    /// Associated events inherited from the parent volume.
    pub fn events(self) -> impl Iterator<Item = EventGroup<'a>> {
        self.volume_ref().events()
    }
}

impl PaperRef<'_> {
    /// Publication kind, including explicit backmatter annotations.
    pub fn publication_type(self) -> Result<crate::types::PaperType, String> {
        if self.frontmatter {
            Ok(crate::types::PaperType::Frontmatter)
        } else {
            self.kind.as_deref().unwrap_or("paper").parse()
        }
    }

    /// Parse the effective ingestion date, rejecting invalid Gregorian dates.
    pub fn ingest_date_value(self) -> Result<chrono::NaiveDate, String> {
        parse_ingest_date(self.ingest_date())
    }
}

impl<'a> PaperRef<'a> {
    /// Effective deletion notice: the last XML notice wins, as in the reference
    /// parser.
    pub fn deletion(self) -> Option<(crate::types::DeletionKind, &'a crate::Notice)> {
        use crate::types::DeletionKind;
        match self.last_deletion {
            Some(DeletionKind::Removed) => self
                .stored()
                .removed
                .as_ref()
                .map(|n| (DeletionKind::Removed, n)),
            Some(DeletionKind::Retracted) => self
                .stored()
                .retracted
                .as_ref()
                .map(|n| (DeletionKind::Retracted, n)),
            None => None,
        }
    }
}
