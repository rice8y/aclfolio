//! Pure XML formatting and semantic comparison utilities.

use crate::utils::{xml_attribute, xml_escape};

/// Tags whose inner XML has markup semantics.
pub const TAGS_WITH_MARKUP: &[&str] = &[
    "b",
    "i",
    "u",
    "sc",
    "tt",
    "a",
    "fixed-case",
    "title",
    "abstract",
    "booktitle",
    "shortbooktitle",
];

/// Parents whose child tag order is insignificant.
pub const TAGS_WITH_UNORDERED_CHILDREN: &[&str] = &[
    "talk",
    "paper",
    "meta",
    "frontmatter",
    "event",
    "colocated",
    "author",
    "editor",
    "speaker",
    "variant",
];

/// Repeated tags whose relative order remains significant.
pub const TAGS_WITH_ORDER_SEMANTICS: &[&str] = &[
    "author",
    "editor",
    "speaker",
    "erratum",
    "revision",
    "talk",
    "venue",
    "attachment",
    "award",
    "video",
];

/// Elements kept on one line by canonical indentation.
pub const TAGS_WITHOUT_LINEBREAKS: &[&str] = &[
    "author",
    "editor",
    "speaker",
    "title",
    "booktitle",
    "variant",
];

/// Detached XML value for pure conversions. It is never connected to
/// authoritative records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlElement {
    /// Element name, or `#comment` for an XML comment.
    pub tag: String,
    /// Attributes in serialized order.
    pub attributes: Vec<(String, String)>,
    /// Text preceding the first child.
    pub text: Option<String>,
    /// Child elements/comments in order.
    pub children: Vec<XmlElement>,
    /// Text following this element in its parent.
    pub tail: Option<String>,
}

impl XmlElement {
    /// Parse a detached XML document, retaining comments, mixed content and
    /// attribute order.
    pub fn parse(xml: &str) -> Result<Self, String> {
        fn walk(n: roxmltree::Node<'_, '_>) -> Result<XmlElement, String> {
            if n.tag_name().namespace().is_some() {
                return Err("namespaced elements are not Anthology XML".into());
            }
            let mut result = XmlElement {
                tag: if n.is_comment() {
                    "#comment".into()
                } else {
                    n.tag_name().name().into()
                },
                attributes: n
                    .attributes()
                    .map(|a| (a.name().into(), a.value().into()))
                    .collect(),
                text: None,
                children: vec![],
                tail: None,
            };
            if n.is_comment() {
                result.text = n.text().map(str::to_owned);
                return Ok(result);
            }
            for child in n.children() {
                if child.is_text() {
                    let text = child.text().unwrap();
                    let dst = if let Some(c) = result.children.last_mut() {
                        &mut c.tail
                    } else {
                        &mut result.text
                    };
                    dst.get_or_insert_with(String::new).push_str(text);
                } else if child.is_element() || child.is_comment() {
                    result.children.push(walk(child)?);
                }
            }
            Ok(result)
        }
        let d = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
        let mut root = walk(d.root_element())?;
        root.tail = d
            .root_element()
            .next_siblings()
            .skip(1)
            .filter(|n| n.is_text())
            .filter_map(|n| n.text())
            .reduce(|_, b| b)
            .map(str::to_owned);
        Ok(root)
    }

    /// Serialize an element, optionally including its tail.
    pub fn serialize(&self, with_tail: bool) -> String {
        let mut out = if self.tag == "#comment" {
            format!("<!--{}-->", self.text.as_deref().unwrap_or(""))
        } else {
            let mut out = format!("<{}", self.tag);
            for (k, v) in &self.attributes {
                out.push_str(&format!(" {k}=\"{}\"", xml_attribute(v)));
            }
            if self.children.is_empty() && self.text.is_none() {
                out.push_str("/>");
            } else {
                out.push('>');
                if let Some(t) = &self.text {
                    out.push_str(&xml_escape(t).replace('\r', "&#13;"));
                }
                for c in &self.children {
                    out.push_str(&c.serialize(true));
                }
                out.push_str(&format!("</{}>", self.tag));
            }
            out
        };
        if with_tail {
            if let Some(t) = &self.tail {
                out.push_str(&xml_escape(t).replace('\r', "&#13;"));
            }
        }
        out
    }

    /// Append character data to a detached element's final text position.
    pub fn append_text(&mut self, text: &str) {
        let dst = if let Some(child) = self.children.last_mut() {
            &mut child.tail
        } else {
            &mut self.text
        };
        dst.get_or_insert_with(String::new).push_str(text);
    }

    /// Serialize inner content and the outer tail, matching `stringify_children`.
    pub fn stringify_children(&self) -> String {
        let mut out = self.text.as_deref().map(xml_escape).unwrap_or_default();
        for c in &self.children {
            out.push_str(&c.serialize(false));
            if let Some(t) = &c.tail {
                out.push_str(&xml_escape(t));
            }
        }
        if let Some(t) = &self.tail {
            out.push_str(&xml_escape(t));
        }
        out.trim_matches(crate::utils::python_whitespace).into()
    }

    /// Canonically indent a detached value using two-space nesting and inline-tag
    /// rules.
    pub fn indent(&mut self, level: usize, internal: bool) {
        let oneline = TAGS_WITHOUT_LINEBREAKS.contains(&self.tag.as_str());
        let markup = TAGS_WITH_MARKUP.contains(&self.tag.as_str());
        self.text = self.text.as_deref().map(|s| {
            clean_whitespace(s)
                .trim_start_matches(crate::utils::python_whitespace)
                .into()
        });
        let blank = |s: &Option<String>| {
            s.as_deref()
                .is_none_or(|s| s.trim_matches(crate::utils::python_whitespace).is_empty())
        };
        if !self.children.is_empty() {
            if !oneline && !markup && blank(&self.text) {
                self.text = Some(format!("\n{}", "  ".repeat(level + 1)));
            }
            if blank(&self.tail) {
                self.tail = Some(format!("\n{}", "  ".repeat(level)));
            }
            if !markup {
                for c in &mut self.children {
                    c.indent(level + 1, internal || oneline);
                }
            }
            let last = self.children.last_mut().unwrap();
            if oneline {
                last.tail = last.tail.as_deref().map(|s| {
                    clean_whitespace(s)
                        .trim_end_matches(crate::utils::python_whitespace)
                        .into()
                });
            } else if !markup && !internal && blank(&last.tail) {
                last.tail = Some(format!("\n{}", "  ".repeat(level)));
            }
        } else {
            self.text = self.text.as_deref().map(|s| {
                clean_whitespace(s)
                    .trim_matches(crate::utils::python_whitespace)
                    .into()
            });
            if self.tag == "#comment" {
                self.text = Some(format!(" {} ", self.text.as_deref().unwrap_or("")));
            }
            if internal {
                self.tail = self.tail.as_deref().map(clean_whitespace);
            } else if blank(&self.tail) {
                self.tail = Some(format!("\n{}", "  ".repeat(level)));
            }
        }
    }

    /// Logical equality with the pinned Python comparator's ordering and
    /// empty-element rules.
    pub fn logically_equals(&self, other: &Self) -> bool {
        if self.tag != other.tag
            || self.text.as_deref().unwrap_or("") != other.text.as_deref().unwrap_or("")
        {
            return false;
        }
        let attributes = |e: &Self| {
            e.attributes
                .iter()
                .cloned()
                .collect::<std::collections::BTreeMap<_, _>>()
        };
        if attributes(self) != attributes(other) {
            return false;
        }
        if TAGS_WITH_MARKUP.contains(&self.tag.as_str()) {
            return self
                .serialize(true)
                .trim_end_matches(crate::utils::python_whitespace)
                == other
                    .serialize(true)
                    .trim_end_matches(crate::utils::python_whitespace);
        }

        fn children(e: &XmlElement) -> Vec<&XmlElement> {
            let empty = [
                "abstract",
                "address",
                "affiliation",
                "colocated",
                "dates",
                "first",
                "pages",
                "publisher",
            ];
            let mut children: Vec<_> = e
                .children
                .iter()
                .filter(|c| {
                    c.tag != "#comment"
                        && !(empty.contains(&c.tag.as_str())
                            && c.text.as_deref().is_none_or(str::is_empty))
                })
                .collect();
            if TAGS_WITH_UNORDERED_CHILDREN.contains(&e.tag.as_str()) {
                children.sort_by_cached_key(|c| {
                    (
                        c.tag.clone(),
                        if TAGS_WITH_ORDER_SEMANTICS.contains(&c.tag.as_str()) {
                            String::new()
                        } else {
                            c.serialize(true)
                        },
                    )
                });
            }
            children
        }
        let a = children(self);
        let b = children(other);
        a.len() == b.len() && a.into_iter().zip(b).all(|(a, b)| a.logically_equals(b))
    }
}

/// Collapse runs of ASCII spaces, leaving other whitespace intact.
pub fn clean_whitespace(text: &str) -> String {
    let mut out = String::new();
    let mut space = false;
    for c in text.chars() {
        if c != ' ' || !space {
            out.push(c);
        }
        space = c == ' ';
    }
    out
}

/// Compare two XML document strings for logical Anthology equivalence.
pub fn assert_equals(a: &str, b: &str) -> Result<(), String> {
    if XmlElement::parse(a)?.logically_equals(&XmlElement::parse(b)?) {
        Ok(())
    } else {
        Err("XML elements are not logically equivalent".into())
    }
}

/// Escape optional XML character data.
pub fn xml_escape_or_none(text: Option<&str>) -> Option<String> {
    text.map(xml_escape)
}
