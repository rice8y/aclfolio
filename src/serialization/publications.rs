//! Collection/volume/paper XML conversion and snapshot-bound convenience
//! methods.

use super::validation::validate_publication_xml;
use super::{attr, element, optional, ref_xml};
use crate::{Collection, Event, Paper, PaperRef, Volume, config::Config};
use crate::{
    types::DeletionKind,
    utils::{xml_attribute, xml_escape},
};

impl Paper {
    /// Serialize a stored paper/frontmatter record without modifying it.
    pub fn to_xml(&self, config: &Config) -> Result<String, String> {
        if self.bibkey == crate::types::NO_BIBKEY {
            return Err("cannot serialize a paper without a bibkey".into());
        }
        let tag = if self.frontmatter {
            "frontmatter"
        } else {
            "paper"
        };
        let mut out = format!("<{tag}");
        if !self.frontmatter {
            attr(&mut out, "id", Some(self.local_id.as_str()));
        }
        attr(&mut out, "ingest-date", self.ingest_date.as_deref());
        if self.kind.as_deref() == Some("backmatter") {
            attr(&mut out, "type", Some("backmatter"));
        }
        out.push('>');
        out.push_str(&format!(
            "<!--{}-->",
            config.format(&config.paper_page_template, self.id.as_str())
        ));
        if !self.frontmatter {
            if let Some(t) = &self.title {
                out.push_str(&t.to_xml("title"));
            }
            for n in &self.authors {
                out.push_str(&n.to_xml("author"));
            }
            for n in &self.editors {
                out.push_str(&n.to_xml("editor"));
            }
        }
        optional(&mut out, "pages", self.pages.as_deref());
        if let Some(a) = &self.abstract_text {
            out.push_str(&a.to_xml("abstract"));
        }
        if let Some(pdf) = &self.pdf {
            out.push_str(&format!(
                "<pdf hash=\"{}\"/>",
                xml_attribute(if pdf.hash.is_empty() {
                    "None"
                } else {
                    &pdf.hash
                })
            ));
        }
        for u in &self.urls {
            out.push_str(&ref_xml(
                "url",
                &u.url,
                u.checksum.as_deref(),
                u.kind.as_deref(),
            ));
        }
        for e in &self.errata {
            out.push_str(&e.to_xml());
        }
        for r in &self.revisions {
            out.push_str(&r.to_xml());
        }
        for (tag, v) in [
            ("doi", self.doi.as_deref()),
            ("issue", self.issue.as_deref()),
            ("journal", self.journal.as_deref()),
            ("language", self.language.as_deref()),
            ("month", self.month.as_deref()),
            ("note", self.note.as_deref()),
        ] {
            optional(&mut out, tag, v);
        }
        for a in &self.attachments {
            if a.kind.as_deref() == Some("mrf") {
                out.push_str(&format!(
                    "<mrf hash=\"{}\" src=\"latexml\">{}</mrf>",
                    xml_attribute(&a.hash),
                    xml_escape(&a.filename)
                ));
            } else {
                out.push_str(&ref_xml(
                    "attachment",
                    &a.filename,
                    (!a.hash.is_empty()).then_some(a.hash.as_str()),
                    a.kind.as_deref(),
                ));
            }
        }
        if let Some(m) = &self.mrf {
            out.push_str(&format!(
                "<mrf src=\"latexml\">{}</mrf>",
                xml_escape(&m.value)
            ));
        }
        for v in &self.videos {
            out.push_str(&v.to_xml());
        }
        for award in &self.awards {
            out.push_str(&award.to_xml());
        }
        match self.last_deletion {
            Some(DeletionKind::Retracted) => {
                if let Some(n) = &self.retracted {
                    out.push_str(&n.to_xml(DeletionKind::Retracted));
                }
            }
            Some(DeletionKind::Removed) => {
                if let Some(n) = &self.removed {
                    out.push_str(&n.to_xml(DeletionKind::Removed));
                }
            }
            None => {}
        }
        out.push_str(&element("bibkey", &self.bibkey));
        out.push_str(&format!("</{tag}>"));
        Ok(out)
    }
}

impl Volume {
    /// Serialize metadata and optionally contained papers. XML output is in-memory
    /// only.
    pub fn to_xml(&self, with_papers: bool, config: &Config) -> Result<String, String> {
        let id = &self.local_id;
        let m = &self.metadata;
        let mut out = format!(
            "<volume id=\"{}\" type=\"{}\"",
            xml_attribute(id),
            xml_attribute(&self.kind)
        );
        attr(&mut out, "ingest-date", self.ingest_date.as_deref());
        out.push_str(&format!(
            "><!--{}--><meta>",
            config.format(&config.volume_page_template, self.id.as_str())
        ));
        out.push_str(&m.booktitle.to_xml("booktitle"));
        if let Some(s) = &m.shortbooktitle {
            out.push_str(&s.to_xml("shortbooktitle"));
        }
        for n in &m.editors {
            out.push_str(&n.to_xml("editor"));
        }
        for (tag, v) in [
            ("publisher", m.publisher.as_deref()),
            ("address", m.address.as_deref()),
            ("doi", m.doi.as_deref()),
            ("isbn", m.isbn.as_deref()),
            ("month", m.month.as_deref()),
            ("year", Some(m.year.as_str())),
        ] {
            optional(&mut out, tag, v);
        }
        if let Some(pdf) = &m.pdf {
            out.push_str(&format!(
                "<pdf hash=\"{}\"/>",
                xml_attribute(if pdf.hash.is_empty() {
                    "None"
                } else {
                    &pdf.hash
                })
            ));
        }
        for u in &m.urls {
            out.push_str(&ref_xml(
                "url",
                &u.url,
                u.checksum.as_deref(),
                u.kind.as_deref(),
            ));
        }
        for sig in &m.sigs {
            out.push_str(&element("sig", sig));
        }
        for venue in &m.venues {
            out.push_str(&element("venue", venue));
        }
        for (tag, v) in [
            ("journal-volume", m.journal_volume.as_deref()),
            ("journal-issue", m.journal_issue.as_deref()),
            ("journal-title", m.journal_title.as_deref()),
        ] {
            optional(&mut out, tag, v);
        }
        out.push_str("</meta>");
        if with_papers {
            for p in &self.papers {
                out.push_str(&p.to_xml(config)?);
            }
        }
        out.push_str("</volume>");
        Ok(out)
    }

    /// Parse a standalone volume with an explicit parent collection ID.
    pub fn from_xml(collection: &crate::CollectionId, xml: &str) -> Result<Self, crate::Error> {
        validate_publication_xml(xml).map_err(crate::Error::Limit)?;
        let input = format!("<collection id=\"{collection}\">{xml}</collection>");
        let limits = crate::Limits::default();
        let (mut c, _) = crate::parser::parse_detached(
            std::path::Path::new("<memory>"),
            &input,
            limits.xml_nodes,
            limits.xml_depth,
        )?;
        c.volumes
            .pop()
            .ok_or_else(|| crate::Error::Limit("XML contains no volume".into()))
    }
}

impl Collection {
    /// Serialize a detached collection, including its volumes and explicit event.
    pub fn to_xml(&self, config: &Config) -> Result<String, String> {
        let mut out = format!("<collection id=\"{}\">", self.id);
        for v in &self.volumes {
            out.push_str(&v.to_xml(true, config)?);
        }
        if let Some(e) = &self.event {
            out.push_str(&e.to_xml(config));
        }
        out.push_str("</collection>");
        Ok(out)
    }

    /// Parse a complete collection in memory with default resource bounds and
    /// diagnostics.
    pub fn from_xml(xml: &str) -> Result<(Self, Vec<crate::Diagnostic>), crate::Error> {
        let limits = crate::Limits::default();
        crate::parser::parse(
            std::path::Path::new("<memory>"),
            xml,
            limits.xml_nodes,
            limits.xml_depth,
        )
    }
}

impl Paper {
    /// Parse a paper/frontmatter element using its detached parent volume's
    /// metadata.
    pub fn from_xml(volume: &Volume, xml: &str) -> Result<Self, crate::Error> {
        let v = volume
            .to_xml(false, &Config::default())
            .map_err(crate::Error::Limit)?;
        let v = v.replace("</volume>", &format!("{xml}</volume>"));
        let mut v = Volume::from_xml(&volume.id.collection_id(), &v)?;
        v.papers
            .pop()
            .ok_or_else(|| crate::Error::Limit("XML contains no paper".into()))
    }
}

impl Event {
    /// Parse a detached event with an explicit parent collection identifier.
    pub fn from_xml(collection: &crate::CollectionId, xml: &str) -> Result<Self, crate::Error> {
        let input = format!("<collection id=\"{collection}\">{xml}</collection>");
        let (c, _) = Collection::from_xml(&input)?;
        c.event
            .ok_or_else(|| crate::Error::Limit("XML contains no event".into()))
    }
}

impl PaperRef<'_> {
    /// Serialize this paper with its snapshot's URL configuration.
    pub fn to_xml(self) -> Result<String, String> {
        self.stored().to_xml(self.root().config())
    }
}

impl crate::navigation::VolumeRef<'_> {
    /// Serialize this volume with its snapshot's URL configuration.
    pub fn to_xml(self, with_papers: bool) -> Result<String, String> {
        self.stored().to_xml(with_papers, self.root().config())
    }
}
