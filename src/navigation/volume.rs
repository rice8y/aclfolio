//! Volume views and volume-level inheritance, files and relationships.

use super::{CollectionRef, PaperRef, parse_ingest_date};
use crate::{
    Anthology, Collection, EventGroup, NameSpecification, Volume,
    files::{FileKind, FileReference},
    ids::IdParts,
};

/// A volume bound to the snapshot used to resolve its venues, events and
/// papers.
#[derive(Debug, Clone, Copy)]
pub struct VolumeRef<'a> {
    pub(crate) anthology: &'a Anthology,
    pub(crate) volume: &'a Volume,
}

impl std::ops::Deref for VolumeRef<'_> {
    type Target = Volume;

    fn deref(&self) -> &Volume {
        self.volume
    }
}

impl<'a> VolumeRef<'a> {
    /// Original stored volume metadata.
    pub fn stored(self) -> &'a Volume {
        self.volume
    }

    /// Snapshot containing this volume.
    pub fn root(self) -> &'a Anthology {
        self.anthology
    }

    /// Parent collection.
    pub fn collection(self) -> &'a Collection {
        self.anthology
            .collection(self.volume.id.collection_id().as_str())
            .unwrap()
            .unwrap()
    }

    /// Local, unpadded volume ID.
    pub fn local_id(self) -> String {
        self.volume.local_id.clone()
    }

    /// ID tuple with an absent paper component.
    pub fn full_id_tuple(self) -> IdParts {
        IdParts(
            self.volume.id.collection_id().to_string(),
            Some(self.volume.local_id.clone()),
            None,
        )
    }

    /// Effective journal title; absent for proceedings.
    pub fn journal_title(self) -> Option<&'a str> {
        if self.volume.kind != "journal" {
            return None;
        }
        self.volume.metadata.journal_title.as_deref().or_else(|| {
            self.volume
                .metadata
                .venues
                .first()
                .and_then(|id| self.anthology.venue(id))
                .map(|v| v.name.as_str())
        })
    }

    /// Explicit ingestion date or the Python sentinel `1900-01-01`.
    pub fn ingest_date(self) -> &'a str {
        self.volume
            .ingest_date
            .as_deref()
            .unwrap_or(crate::types::UNKNOWN_INGEST_DATE)
    }

    /// All contained papers in XML order.
    pub fn papers(self) -> impl ExactSizeIterator<Item = PaperRef<'a>> {
        self.volume
            .papers
            .iter()
            .map(move |p| self.anthology.paper_by_id(&p.id).unwrap())
    }

    /// Look up a local paper ID without treating it as a full Anthology ID.
    pub fn get(self, local_id: &str) -> Option<PaperRef<'a>> {
        self.papers().find(|p| p.local_id() == local_id)
    }

    /// Front matter identified by local ID zero.
    pub fn frontmatter(self) -> Option<PaperRef<'a>> {
        self.get("0")
    }

    /// Whether the volume contains local paper ID zero.
    pub fn has_frontmatter(self) -> bool {
        self.frontmatter().is_some()
    }

    /// Whether any contained paper has an abstract, including an empty one.
    pub fn has_abstracts(self) -> bool {
        self.volume.papers.iter().any(|p| p.abstract_text.is_some())
    }

    /// Workshop detection uses both venue tags and the interpreted title.
    pub fn is_workshop(self) -> Result<bool, String> {
        Ok(self.volume.metadata.venues.iter().any(|id| id == "ws")
            || self
                .volume
                .metadata
                .booktitle
                .as_text()?
                .to_lowercase()
                .contains("workshop"))
    }

    /// Hyphen-joined venue acronyms, excluding the generic `ws` venue.
    pub fn venue_acronym(self) -> Result<String, String> {
        self.volume
            .metadata
            .venues
            .iter()
            .filter(|id| id.as_str() != "ws")
            .map(|id| {
                self.anthology
                    .venue(id)
                    .map(|v| v.acronym.as_str())
                    .ok_or_else(|| format!("unknown venue {id}"))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|v| v.join("-"))
    }

    /// Configured landing page URL.
    pub fn web_url(self) -> String {
        let config = self.anthology.config();
        config.format(&config.volume_page_template, self.volume.id.as_str())
    }

    /// Stored PDF with its derived filename.
    pub fn pdf(self) -> Option<FileReference> {
        self.volume.metadata.pdf.as_ref().map(|p| {
            FileReference::new(
                FileKind::Pdf,
                self.volume.id.as_str(),
                (!p.hash.is_empty()).then(|| p.hash.clone()),
            )
        })
    }

    /// Events associated with this volume.
    pub fn events(self) -> impl Iterator<Item = EventGroup<'a>> {
        self.anthology.volume_events(&self.volume.id)
    }

    /// Editor credits attached to this volume.
    pub fn namespecs(self) -> &'a [NameSpecification] {
        &self.volume.metadata.editors
    }

    /// Generate the next unused numeric local paper ID without reserving it.
    pub fn generate_paper_id(self) -> String {
        let highest = self
            .volume
            .papers
            .iter()
            .map(|p| p.local_id.as_str())
            .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .map(|s| s.trim_start_matches('0'))
            .max_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)))
            .unwrap_or("");
        crate::utils::increment_decimal(highest)
    }

    /// Generate this volume's frontmatter BibTeX, or an error if none exists.
    pub fn to_bibtex(self) -> Result<String, String> {
        self.frontmatter()
            .ok_or_else(|| "cannot generate BibTeX for a volume without frontmatter".to_owned())?
            .to_bibtex(false)
    }
}

impl<'a> VolumeRef<'a> {
    /// Parent collection with source navigation.
    pub fn collection_ref(self) -> CollectionRef<'a> {
        CollectionRef {
            anthology: self.anthology,
            collection: self.collection(),
        }
    }
}

impl<'a> VolumeRef<'a> {
    /// Effective title, stored as the volume's booktitle.
    pub fn title(self) -> &'a crate::text::RichText {
        &self.volume.metadata.booktitle
    }

    /// Optional short title.
    pub fn shorttitle(self) -> Option<&'a crate::text::RichText> {
        self.volume.metadata.shortbooktitle.as_ref()
    }

    /// Publication year as stored.
    pub fn year(self) -> &'a str {
        &self.volume.metadata.year
    }

    /// Volume kind as an enum.
    pub fn publication_type(self) -> Result<crate::types::VolumeType, String> {
        self.volume.kind.parse()
    }

    /// Publication month.
    pub fn month(self) -> Option<&'a str> {
        self.volume
            .metadata
            .month
            .as_deref()
            .filter(|s| !s.is_empty())
    }

    /// Publisher.
    pub fn publisher(self) -> Option<&'a str> {
        self.volume
            .metadata
            .publisher
            .as_deref()
            .filter(|s| !s.is_empty())
    }

    /// Publication location.
    pub fn address(self) -> Option<&'a str> {
        self.volume
            .metadata
            .address
            .as_deref()
            .filter(|s| !s.is_empty())
    }

    /// ISBN.
    pub fn isbn(self) -> Option<&'a str> {
        self.volume
            .metadata
            .isbn
            .as_deref()
            .filter(|s| !s.is_empty())
    }

    /// DOI.
    pub fn doi(self) -> Option<&'a str> {
        self.volume
            .metadata
            .doi
            .as_deref()
            .filter(|s| !s.is_empty())
    }

    /// Journal issue number.
    pub fn journal_issue(self) -> Option<&'a str> {
        self.volume
            .metadata
            .journal_issue
            .as_deref()
            .filter(|s| !s.is_empty())
    }

    /// Journal volume number.
    pub fn journal_volume(self) -> Option<&'a str> {
        self.volume
            .metadata
            .journal_volume
            .as_deref()
            .filter(|s| !s.is_empty())
    }

    /// Parse the effective ingestion date, rejecting invalid Gregorian dates.
    pub fn ingest_date_value(self) -> Result<chrono::NaiveDate, String> {
        parse_ingest_date(self.ingest_date())
    }
}
