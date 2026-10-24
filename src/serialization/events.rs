//! Events and talks, preserving source link and speaker order.

use super::{attr, element, optional, ordered_links, ref_xml};
use crate::{
    Event, NameSpecification, Talk, UrlReference, config::Config, text::RichText,
    utils::xml_attribute,
};

impl Talk {
    /// All stored attachment links in source order.
    pub fn urls(&self) -> impl Iterator<Item = &UrlReference> {
        self.url.iter().chain(&self.additional_urls)
    }

    /// Effective attachment dictionary: missing types mean `attachment`, later
    /// duplicates replace earlier values.
    pub fn attachments(&self) -> std::collections::BTreeMap<&str, &UrlReference> {
        self.urls()
            .map(|u| (u.kind.as_deref().unwrap_or("attachment"), u))
            .collect()
    }

    /// Serialize a talk and its speaker/attachment metadata.
    pub fn to_xml(&self) -> String {
        let mut out = "<talk".to_owned();
        attr(&mut out, "type", self.kind.as_deref());
        out.push('>');
        out.push_str(&self.title.to_xml("title"));
        for n in &self.speakers {
            out.push_str(&n.to_xml("speaker"));
        }
        for (kind, url) in ordered_links(self.urls()) {
            out.push_str(&ref_xml(
                "url",
                &url.url,
                url.checksum.as_deref(),
                Some(kind),
            ));
        }
        out.push_str("</talk>");
        out
    }

    /// Parse a detached talk.
    pub fn from_xml(xml: &str) -> Result<Self, String> {
        let d = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
        let n = d.root_element();
        let mut title = None;
        let mut speakers = vec![];
        let mut urls = vec![];
        for c in n.children().filter(|c| c.is_element()) {
            let child = &xml[c.range()];
            match c.tag_name().name() {
                "title" => title = Some(RichText::from_xml(child)?),
                "speaker" => speakers.push(NameSpecification::from_xml(child)?),
                "url" => urls.push(UrlReference {
                    kind: c.attribute("type").map(str::to_owned),
                    url: c.text().unwrap_or("").into(),
                    checksum: c.attribute("hash").map(str::to_owned),
                }),
                tag => return Err(format!("unsupported talk element {tag}")),
            }
        }
        Ok(Self {
            kind: n.attribute("type").map(str::to_owned),
            title: title.ok_or("talk requires a title")?,
            speakers,
            url: if urls.is_empty() {
                None
            } else {
                Some(urls.remove(0))
            },
            additional_urls: urls,
        })
    }
}

impl Event {
    /// Serialize stored event metadata with configured landing-page comment.
    pub fn to_xml(&self, config: &Config) -> String {
        let mut out = format!(
            "<event id=\"{}\"><!--{}-->",
            xml_attribute(&self.id),
            config.format(&config.event_page_template, &self.id)
        );
        if let Some(m) = &self.metadata {
            let mut meta = String::new();
            if let Some(t) = &m.title {
                meta.push_str(&t.to_xml("title"));
            }
            optional(
                &mut meta,
                "location",
                m.location.as_deref().filter(|s| !s.is_empty()),
            );
            optional(
                &mut meta,
                "dates",
                m.dates.as_deref().filter(|s| !s.is_empty()),
            );
            if !meta.is_empty() {
                out.push_str(&format!("<meta>{meta}</meta>"));
            }
        }
        if let Some(links) = &self.links {
            if !links.is_empty() {
                out.push_str("<links>");
                let links = ordered_links(links.iter());
                for (kind, u) in links {
                    out.push_str(&ref_xml("url", &u.url, u.checksum.as_deref(), Some(kind)));
                }
                out.push_str("</links>");
            }
        }
        for talk in &self.talks {
            out.push_str(&talk.to_xml());
        }
        if let Some(ids) = &self.colocated {
            if !ids.is_empty() {
                out.push_str("<colocated>");
                for id in ids {
                    out.push_str(&element("volume-id", id.as_str()));
                }
                out.push_str("</colocated>");
            }
        }
        out.push_str("</event>");
        out
    }
}
