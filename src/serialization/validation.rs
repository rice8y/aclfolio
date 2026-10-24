//! Constructor-level checks for detached publications.

// These reproduce the reference constructors, which accept incomplete
// fragments.
// Snapshot structural diagnostics and whole-document Relax NG checks are
// separate.
use crate::{Award, Erratum, NameSpecification, Notice, Revision};

pub(super) fn validate_publication_xml(xml: &str) -> Result<(), String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
    for node in doc.descendants().filter(|n| n.is_element()) {
        let last = |tag: &str| node.children().rfind(|n| n.has_tag_name(tag));
        let require = |tag: &str| {
            if last(tag).is_some() {
                Ok(())
            } else {
                Err(format!("missing required {tag}"))
            }
        };
        let allowed = match node.tag_name().name() {
            "volume" => {
                node.attribute("type")
                    .ok_or("missing volume type")?
                    .parse::<crate::types::VolumeType>()?;
                if !crate::ids::is_valid_item_id(node.attribute("id").unwrap_or("None")) {
                    return Err("invalid volume ID".into());
                }
                "meta paper frontmatter"
            }
            "meta" if node.parent().is_some_and(|n| n.has_tag_name("volume")) => {
                require("booktitle")?;
                require("year")?;
                if last("year").and_then(|n| n.text()).is_none() {
                    return Err("year must be a string".into());
                }
                "address doi isbn month publisher year journal-issue journal-volume journal-title booktitle shortbooktitle editor pdf url sig venue"
            }
            "paper" | "frontmatter" => {
                let front = node.has_tag_name("frontmatter");
                if !front {
                    require("title")?;
                    node.attribute("type")
                        .unwrap_or("paper")
                        .parse::<crate::types::PaperType>()?;
                    if !crate::ids::is_valid_item_id(node.attribute("id").unwrap_or("None")) {
                        return Err("invalid paper ID".into());
                    }
                }
                let bib = last("bibkey")
                    .and_then(|n| n.text())
                    .ok_or("missing bibliography key")?;
                if !crate::ids::is_valid_bibkey(bib) {
                    return Err("invalid bibliography key".into());
                }
                if front {
                    "bibkey doi pages attachment revision pdf url"
                } else {
                    "bibkey doi issue journal language month note pages author editor abstract title attachment award erratum removed retracted revision pdf url video mrf"
                }
            }
            _ => "",
        };
        if !allowed.is_empty() {
            for child in node.children().filter(|n| n.is_element()) {
                if !allowed
                    .split_whitespace()
                    .any(|tag| child.has_tag_name(tag))
                {
                    return Err(format!(
                        "unsupported {} child {}",
                        node.tag_name().name(),
                        child.tag_name().name()
                    ));
                }
            }
        }
        if let Some(date) = node.attribute("ingest-date") {
            crate::utils::validate_iso_date(date)?;
        }
        let fragment = &xml[node.range()];
        match node.tag_name().name() {
            "author" | "editor" | "speaker" => {
                NameSpecification::from_xml(fragment)?;
            }
            "revision" => {
                Revision::from_xml(fragment)?;
            }
            "erratum" => {
                Erratum::from_xml(fragment)?;
            }
            "removed" | "retracted" => {
                Notice::from_xml(fragment)?;
            }
            "award" => {
                Award::from_xml(fragment)?;
            }
            "video" => {
                crate::files::FileReference::from_xml(crate::files::FileKind::Video, fragment)?;
            }
            "url"
                if node.parent().is_some_and(|p| {
                    p.has_tag_name("paper")
                        || p.has_tag_name("frontmatter")
                        || p.has_tag_name("meta")
                }) =>
            {
                crate::files::FileReference::from_xml(crate::files::FileKind::Url, fragment)?
                    .validate_external()?;
            }
            _ => {}
        }
    }
    Ok(())
}
