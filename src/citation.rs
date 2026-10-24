//! Bibliographic data, deterministic BibTeX and built-in ACL citations.

use crate::{
    NameSpecification, PaperRef,
    latex::{BibtexValue, make_bibtex_entry},
};
use serde_json::{Value, json};

impl NameSpecification {
    /// CiteProcJSON name representation (an empty given name is omitted).
    pub fn citeproc_dict(&self) -> Value {
        if let Some(first) = self.name.first.as_deref().filter(|s| !s.is_empty()) {
            json!({"family":self.name.last,"given":first})
        } else {
            json!({"family":self.name.last})
        }
    }
}

impl PaperRef<'_> {
    /// BibTeX entry type, accounting for journal/proceedings and frontmatter.
    pub fn bibtype(self) -> Result<&'static str, String> {
        match (self.is_frontmatter(), self.volume().kind.as_str()) {
            (true, "journal") => Ok("book"),
            (false, "journal") => Ok("article"),
            (true, "proceedings") => Ok("proceedings"),
            (false, "proceedings") => Ok("inproceedings"),
            (_, kind) => Err(format!("unknown volume type {kind}")),
        }
    }

    /// CSL item type.
    pub fn csltype(self) -> &'static str {
        if self.is_frontmatter() {
            "book"
        } else if self.volume().kind == "journal" {
            "article-journal"
        } else {
            "paper-conference"
        }
    }

    /// CiteProcJSON bibliographic data. The issued year remains a string, matching
    /// Python.
    pub fn citeproc_dict(self) -> Result<Value, String> {
        let editors: Vec<_> = self
            .editors()
            .iter()
            .map(NameSpecification::citeproc_dict)
            .collect();
        let authors: Vec<_> = if self.is_frontmatter() {
            editors.clone()
        } else {
            self.authors
                .iter()
                .map(NameSpecification::citeproc_dict)
                .collect()
        };
        let mut value = json!({
            "id": self.bibkey,
            "title": self.title().as_text()?,
            "type": self.csltype(),
            "author": authors,
            "editor": editors,
            "publisher": self.publisher(),
            "publisher-place": self.address(),
            "issued": {"date-parts": [[self.year()]]},
            "URL": self.web_url(),
            "DOI": self.doi.as_deref().filter(|s| !s.is_empty()),
            "ISBN": self.isbn(),
            "page": self.pages.as_deref().filter(|s| !s.is_empty()),
        });
        let map = value.as_object_mut().unwrap();
        if self.volume().kind == "journal" {
            map.insert("container-title".into(), json!(self.journal_title()));
            map.insert("volume".into(), json!(self.journal_volume()));
            map.insert("issue".into(), json!(self.journal_issue()));
        } else if self.volume().kind == "proceedings" {
            map.insert(
                "container-title".into(),
                json!(self.volume().metadata.booktitle.as_text()?),
            );
        }
        map.retain(|_, v| !v.is_null());
        Ok(value)
    }

    /// Exact BibTeX formatting with optional abstract inclusion.
    pub fn to_bibtex(self, with_abstract: bool) -> Result<String, String> {
        if self.bibkey == crate::types::NO_BIBKEY {
            return Err("cannot generate BibTeX without a bibkey".into());
        }
        let title = self.title();
        let editors = self.editors();
        let stored = self.stored();
        let url = self.web_url();
        let mut fields = vec![
            ("title", BibtexValue::Markup(title)),
            ("author", BibtexValue::Names(&stored.authors)),
            ("editor", BibtexValue::Names(editors)),
        ];
        if !self.is_frontmatter() {
            if self.volume().kind == "journal" {
                for (key, val) in [
                    ("journal", self.journal_title()),
                    ("volume", self.journal_volume()),
                    ("number", self.journal_issue()),
                ] {
                    if let Some(v) = val {
                        fields.push((key, BibtexValue::Text(v)));
                    }
                }
            } else if self.volume().kind == "proceedings" {
                fields.push((
                    "booktitle",
                    BibtexValue::Markup(&self.volume().metadata.booktitle),
                ));
            }
        }
        for (key, val) in [
            ("month", self.month()),
            ("year", Some(self.year())),
            ("address", self.address()),
            ("publisher", self.publisher()),
            ("note", stored.note.as_deref()),
            ("url", Some(url.as_str())),
            ("doi", stored.doi.as_deref()),
            ("pages", stored.pages.as_deref()),
            ("language", stored.language.as_deref()),
            ("ISBN", self.isbn()),
        ] {
            if let Some(v) = val {
                fields.push((key, BibtexValue::Text(v)));
            }
        }
        if with_abstract {
            if let Some(a) = &stored.abstract_text {
                fields.push(("abstract", BibtexValue::Markup(a)));
            }
        }
        make_bibtex_entry(self.bibtype()?, &stored.bibkey, &fields)
    }

    /// HTML citation in the official built-in ACL style.
    pub fn to_acl_citation(self) -> Result<String, String> {
        render_acl_citation(self)
    }

    /// Brief Markdown citation with names, venue acronym and year.
    pub fn to_markdown_citation(self) -> Result<String, String> {
        let names = if self.is_frontmatter() {
            self.editors()
        } else {
            &self.stored().authors
        };
        let name = match names {
            [] => String::new(),
            [a] => a.name.last.clone(),
            [a, b] => format!("{} & {}", a.name.last, b.name.last),
            [a, ..] => format!("{} et al.", a.name.last),
        };
        let venue = self.volume_ref().venue_acronym()?;
        let venue_year = if venue == "WS" {
            self.year().to_owned()
        } else {
            format!("{venue} {}", self.year())
        };
        let suffix = if name.is_empty() {
            venue_year
        } else {
            format!("{name}, {venue_year}")
        };
        Ok(format!(
            "[{}]({}) ({suffix})",
            self.title().as_text()?,
            self.web_url()
        ))
    }
}

fn names(names: &[NameSpecification]) -> String {
    match names {
        [] => "N.N.".into(),
        [a] => a.name.as_first_last(),
        [a, b] => format!("{} and {}", a.name.as_first_last(), b.name.as_first_last()),
        _ => format!(
            "{}, and {}",
            names[..names.len() - 1]
                .iter()
                .map(|n| n.name.as_first_last())
                .collect::<Vec<_>>()
                .join(", "),
            names.last().unwrap().name.as_first_last()
        ),
    }
}

fn pages(p: &str) -> String {
    p.replace("--", "–").replace('-', "–")
}

/// Render an ACL-style citation using the same string formatting as Python.
pub fn render_acl_citation(p: PaperRef<'_>) -> Result<String, String> {
    let authors = if !p.authors.is_empty() {
        names(&p.authors)
    } else {
        let e = p.editors();
        if e.is_empty() {
            String::new()
        } else {
            let n = names(e);
            if p.is_frontmatter() {
                n
            } else {
                format!("{n} ({})", if e.len() > 1 { "eds." } else { "ed." })
            }
        }
    };
    let mut title = format!("<a href=\"{}\">{}</a>", p.web_url(), p.title().as_text()?);
    if p.is_frontmatter() {
        title = format!("<i>{title}</i>");
    }
    let mut parent = String::new();
    match p.bibtype()? {
        "inproceedings" => {
            parent = format!("In <i>{}</i>", p.volume().metadata.booktitle.as_text()?);
            if let Some(s) = p.pages.as_deref().filter(|s| !s.is_empty()) {
                let s = pages(s);
                parent.push_str(&format!(
                    ", {} {s}",
                    if s.contains('–') { "pages" } else { "page" }
                ));
            }
            if let Some(s) = p.address().filter(|s| !s.is_empty()) {
                parent.push_str(&format!(", {s}"));
            }
            if let Some(s) = p.publisher().filter(|s| !s.is_empty()) {
                parent.push_str(&format!(". {s}"));
            }
        }
        "article" => {
            parent = format!("<i>{}</i>", p.journal_title().unwrap_or("None"));
            if let Some(s) = p.journal_volume().filter(|s| !s.is_empty()) {
                parent.push_str(&format!(", {s}"));
                if let Some(s) = p.journal_issue() {
                    parent.push_str(&format!("({s})"));
                }
                if let Some(s) = p.pages.as_deref().filter(|s| !s.is_empty()) {
                    parent.push_str(&format!(":{}", pages(s)));
                }
            } else if let Some(s) = p.pages.as_deref().filter(|s| !s.is_empty()) {
                parent.push_str(&format!(", {}", pages(s)));
            }
        }
        _ => {
            if let Some(s) = p.publisher().filter(|s| !s.is_empty()) {
                parent = s.into();
                if let Some(s) = p.address().filter(|s| !s.is_empty()) {
                    parent.push_str(&format!(", {s}"));
                }
            }
        }
    }
    let mut result = if authors.is_empty() {
        format!("{title}. {}.", p.year())
    } else {
        format!("{authors}. {}. {title}.", p.year())
    };
    if !parent.is_empty() {
        result.push_str(&format!(" {parent}."));
    }
    Ok(result)
}

/// Render CiteProcJSON with an explicit CSL file, using a native Rust CSL
/// processor.
/// Style names are rejected, matching the reference API's file-only behavior.
pub fn citeproc_render_html(
    data: &Value,
    style: impl AsRef<std::path::Path>,
    link_title: bool,
) -> Result<String, String> {
    let path = style.as_ref();
    if !path.is_file() {
        return Err("citation styles must be supplied as a CSL file path".into());
    }
    let xml = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    crate::csl::render(data, &xml, link_title)
}

impl PaperRef<'_> {
    /// Render the built-in ACL citation, or a bibliography entry using a CSL file.
    pub fn to_citation(self, style: Option<&std::path::Path>) -> Result<String, String> {
        match style {
            None => self.to_acl_citation(),
            Some(path) => citeproc_render_html(&self.citeproc_dict()?, path, true),
        }
    }
}
