//! Publication annotations and their detached XML representation.

use super::attr;
use crate::utils::{xml_attribute, xml_escape};
use crate::{Award, Erratum, Notice, Revision, Video, types::DeletionKind};

impl Award {
    /// Serialize this award.
    pub fn to_xml(&self) -> String {
        format!(
            "<award name=\"{}\">{}</award>",
            xml_attribute(&self.name),
            xml_escape(&self.text)
        )
    }

    /// Parse a detached award, requiring its name attribute.
    pub fn from_xml(xml: &str) -> Result<Self, String> {
        let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
        let n = doc.root_element();
        Ok(Self {
            name: n.attribute("name").ok_or("award must have a name")?.into(),
            text: n.text().unwrap_or("").into(),
        })
    }
}

impl Revision {
    /// Serialize a revision's ID, checksum, date and note.
    pub fn to_xml(&self) -> String {
        let mut out = format!(
            "<revision id=\"{}\" hash=\"{}\"",
            xml_attribute(&self.id),
            xml_attribute(&self.hash)
        );
        attr(&mut out, "date", self.date.as_deref());
        if self.text.is_empty() {
            out.push_str("/>");
        } else {
            out.push_str(&format!(">{}</revision>", xml_escape(&self.text)));
        }
        out
    }

    /// Parse a detached revision.
    pub fn from_xml(xml: &str) -> Result<Self, String> {
        let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
        let n = doc.root_element();
        let id = n.attribute("id").unwrap_or("None");
        validate_revision_id(id)?;
        if let Some(date) = n.attribute("date") {
            crate::utils::validate_iso_date(date)?;
        }
        Ok(Self {
            id: id.into(),
            date: n.attribute("date").map(str::to_owned),
            hash: n.attribute("hash").unwrap_or("None").into(),
            text: n.text().unwrap_or("").into(),
        })
    }
}

fn validate_revision_id(id: &str) -> Result<(), String> {
    if id.len() > 2
        || id.is_empty()
        || id.starts_with('0')
        || !id.bytes().all(|c| c.is_ascii_digit())
    {
        Err("revision/erratum ID must be between 1 and 99".into())
    } else {
        Ok(())
    }
}

impl Erratum {
    /// Serialize an erratum's ID, checksum and optional date.
    pub fn to_xml(&self) -> String {
        let mut out = format!(
            "<erratum id=\"{}\" hash=\"{}\"",
            xml_attribute(&self.id),
            xml_attribute(self.hash.as_deref().unwrap_or("None"))
        );
        attr(&mut out, "date", self.date.as_deref());
        out.push_str("/>");
        out
    }

    /// Parse a detached erratum.
    pub fn from_xml(xml: &str) -> Result<Self, String> {
        let r = Revision::from_xml(xml)?;
        Ok(Self {
            id: r.id,
            date: r.date,
            hash: {
                let d = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
                d.root_element()
                    .attribute("hash")
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
            },
        })
    }
}

impl Notice {
    /// Serialize a deletion notice with an explicit kind.
    pub fn to_xml(&self, kind: DeletionKind) -> String {
        let tag = if kind == DeletionKind::Retracted {
            "retracted"
        } else {
            "removed"
        };
        format!(
            "<{tag} date=\"{}\">{}</{tag}>",
            xml_attribute(&self.date),
            xml_escape(&self.text)
        )
    }

    /// Parse a deletion notice and return its kind.
    pub fn from_xml(xml: &str) -> Result<(DeletionKind, Self), String> {
        let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
        let n = doc.root_element();
        let kind = match n.tag_name().name() {
            "retracted" => DeletionKind::Retracted,
            "removed" => DeletionKind::Removed,
            _ => return Err("unknown deletion type".into()),
        };
        crate::utils::validate_iso_date(n.attribute("date").unwrap_or("None"))?;
        Ok((
            kind,
            Self {
                date: n
                    .attribute("date")
                    .ok_or("deletion date is required")?
                    .into(),
                text: n.text().unwrap_or("").into(),
            },
        ))
    }
}

impl Video {
    /// Serialize a video link; true permission is implicit in the schema.
    pub fn to_xml(&self) -> String {
        let mut out = format!("<video href=\"{}\"", xml_attribute(&self.href));
        if self.permission == Some(false) {
            out.push_str(" permission=\"false\"");
        }
        out.push_str("/>");
        out
    }
}
