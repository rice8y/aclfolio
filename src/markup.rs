//! Complete detached MarkupText conversions, preserving XML wrapper attributes
//! and tails.

use crate::{
    latex::latex_encode,
    text::{RichText, normalize},
    utils::{remove_extra_whitespace, xml_escape},
    xml::XmlElement,
};
use std::sync::OnceLock;

/// Text or an XML wrapper containing inline Anthology markup.
/// The first HTML conversion is cached, including its `allow_url` option, as in
/// Python.
#[derive(Debug, Clone)]
pub struct MarkupText {
    content: Content,
    html: OnceLock<String>,
}

#[derive(Debug, Clone)]
enum Content {
    Text(String),
    Element(XmlElement),
}

impl MarkupText {
    /// Construct literal text, optionally applying Unicode normalization.
    pub fn from_string(text: &str, clean: bool) -> Self {
        Self {
            content: Content::Text(if clean {
                crate::utils::clean_unicode(text)
            } else {
                text.into()
            }),
            html: OnceLock::new(),
        }
    }

    /// Construct from a detached element. Childless wrappers become plain text.
    pub fn from_element(element: &XmlElement) -> Self {
        Self {
            content: if element.children.is_empty() {
                Content::Text(element.text.clone().unwrap_or_default())
            } else {
                Content::Element(element.clone())
            },
            html: OnceLock::new(),
        }
    }

    /// Parse a complete XML wrapper.
    pub fn from_xml(xml: &str) -> Result<Self, String> {
        Ok(Self::from_element(&XmlElement::parse(xml)?))
    }

    /// Parse LaTeX into Anthology markup.
    pub fn from_latex(text: &str, clean: bool) -> Result<Self, String> {
        Self::from_rich_text(&RichText::from_latex(text, clean)?)
    }

    /// Heuristically interpret strings that may mix literal punctuation and LaTeX.
    pub fn from_latex_maybe(text: &str, clean: bool) -> Result<Self, String> {
        Self::from_rich_text(&RichText::from_latex_maybe(text, clean)?)
    }

    /// Convert snapshot rich text into a detached MarkupText value.
    pub fn from_rich_text(text: &RichText) -> Result<Self, String> {
        Self::from_xml(&text.to_xml("root"))
    }

    /// Whether the stored value contains XML children, including comments.
    pub fn contains_markup(&self) -> bool {
        matches!(self.content, Content::Element(_))
    }

    /// Inner XML, including the outer tail for a markup wrapper.
    pub fn as_xml(&self) -> String {
        match &self.content {
            Content::Text(text) => xml_escape(text),
            Content::Element(element) => element.stringify_children(),
        }
    }

    /// Serialize under a different outer tag, preserving wrapper attributes and
    /// tail.
    pub fn to_element(&self, tag: &str) -> XmlElement {
        match &self.content {
            Content::Text(text) => XmlElement {
                tag: tag.into(),
                attributes: vec![],
                text: (!text.is_empty()).then(|| text.clone()),
                children: vec![],
                tail: None,
            },
            Content::Element(element) => {
                let mut element = element.clone();
                element.tag = tag.into();
                element
            }
        }
    }

    /// Serialize under a named XML wrapper without writing metadata.
    pub fn to_xml(&self, tag: &str) -> String {
        self.to_element(tag).serialize(true)
    }

    /// Interpreted text, with math and paragraph breaks.
    pub fn as_text(&self) -> Result<String, String> {
        match &self.content {
            Content::Text(text) => Ok(remove_extra_whitespace(text)),
            Content::Element(element) => {
                let text = remove_extra_whitespace(&element_text(element)?);
                Ok(if contains_par(element) {
                    unprotect_par(&text, "\n\n")
                } else {
                    text
                })
            }
        }
    }

    /// HTML with URL handling and the reference's first-call cache behavior.
    pub fn as_html(&self, allow_url: bool) -> Result<String, String> {
        if let Content::Text(text) = &self.content {
            return Ok(xml_escape(&remove_extra_whitespace(text)));
        }
        if let Some(text) = self.html.get() {
            return Ok(text.clone());
        }

        let Content::Element(original) = &self.content else {
            unreachable!()
        };
        let mut element = original.clone();
        convert_html(&mut element, allow_url)?;
        let html = normalize::html(&element.stringify_children());
        let _ = self.html.set(html.clone());
        Ok(html)
    }

    /// LaTeX with the outer wrapper's markup semantics.
    pub fn as_latex(&self) -> String {
        let raw = match &self.content {
            Content::Text(text) => latex_encode(text),
            Content::Element(element) => markup_to_latex(element),
        };
        normalize::latex(&raw)
    }

    /// Test containment in plain text or serialized XML.
    pub fn contains(&self, needle: &str) -> Result<bool, String> {
        Ok(self.as_text()?.contains(needle) || self.as_xml().contains(needle))
    }

    /// Unicode character count of the interpreted text.
    pub fn text_len(&self) -> Result<usize, String> {
        Ok(self.as_text()?.chars().count())
    }

    /// XML-based prefix comparison with Python's optional slice indices.
    pub fn starts_with(&self, values: &[&str], start: Option<isize>, end: Option<isize>) -> bool {
        normalize::slice(&self.as_xml(), start, end)
            .is_some_and(|s| values.iter().any(|v| s.starts_with(v)))
    }

    /// XML-based suffix comparison with Python's optional slice indices.
    pub fn ends_with(&self, values: &[&str], start: Option<isize>, end: Option<isize>) -> bool {
        normalize::slice(&self.as_xml(), start, end)
            .is_some_and(|s| values.iter().any(|v| s.ends_with(v)))
    }
}

fn set_attr(element: &mut XmlElement, key: &str, value: String) {
    if let Some((_, v)) = element.attributes.iter_mut().find(|(k, _)| k == key) {
        *v = value;
    } else {
        element.attributes.push((key.into(), value));
    }
}

fn contains_par(element: &XmlElement) -> bool {
    element
        .children
        .iter()
        .any(|child| child.tag == "par" || contains_par(child))
}

/// Mark paragraph elements with a sentinel in a detached XML value.
pub fn protect_par(element: &mut XmlElement) -> bool {
    let mut changed = false;
    for child in &mut element.children {
        if child.tag == "par" {
            child.text = Some("\u{e000}".into());
            changed = true;
        }
        changed |= protect_par(child);
    }
    changed
}

/// Restore paragraph sentinels after whitespace normalization.
pub fn unprotect_par(text: &str, replacement: &str) -> String {
    normalize::paragraphs(text, replacement)
}

/// Convert an XML element recursively to LaTeX before outer whitespace
/// normalization.
pub fn markup_to_latex(element: &XmlElement) -> String {
    if element.tag == "tex-math" {
        return format!("${}$", element.text.as_deref().unwrap_or("None"));
    }
    if element.tag == "url" {
        return format!("\\url{{{}}}", element.text.as_deref().unwrap_or("None"));
    }
    let mut text = latex_encode(element.text.as_deref().unwrap_or(""));
    for child in &element.children {
        text.push_str(&markup_to_latex(child));
        if let Some(tail) = &child.tail {
            text.push_str(&latex_encode(tail));
        }
    }
    match element.tag.as_str() {
        "fixed-case" => format!("{{{text}}}"),
        "b" => format!("\\textbf{{{text}}}"),
        "i" => format!("\\textit{{{text}}}"),
        "u" => format!("\\underline{{{text}}}"),
        "sc" => format!("\\textsc{{{text}}}"),
        "tt" => format!("\\texttt{{{text}}}"),
        "par" => "\\par ".into(),
        "a" => format!(
            "\\href{{{}}}{{{text}}}",
            element
                .attributes
                .iter()
                .find(|(k, _)| k == "href")
                .map(|(_, v)| v.as_str())
                .unwrap_or("")
        ),
        _ => text,
    }
}

impl PartialEq for MarkupText {
    fn eq(&self, other: &Self) -> bool {
        self.as_xml() == other.as_xml()
    }
}

impl Eq for MarkupText {}

impl PartialOrd for MarkupText {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MarkupText {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_xml().cmp(&other.as_xml())
    }
}

impl PartialEq<str> for MarkupText {
    fn eq(&self, other: &str) -> bool {
        self.as_xml() == other
    }
}

impl std::str::FromStr for MarkupText {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        Self::from_latex_maybe(s, true)
    }
}

impl std::fmt::Display for MarkupText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.as_xml())
    }
}

impl std::hash::Hash for MarkupText {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_xml().hash(state);
    }
}

fn element_text(element: &XmlElement) -> Result<String, String> {
    let mut text = if element.tag == "#comment" {
        String::new()
    } else if element.tag == "tex-math" {
        crate::math::to_unicode(element.text.as_deref().ok_or("math element has no text")?)?
    } else if element.tag == "par" {
        "\u{e000}".into()
    } else {
        element.text.clone().unwrap_or_default()
    };
    for child in &element.children {
        text.push_str(&element_text(child)?);
    }
    if let Some(tail) = &element.tail {
        text.push_str(tail);
    }
    Ok(text)
}

fn convert_html(element: &mut XmlElement, allow_url: bool) -> Result<(), String> {
    match element.tag.as_str() {
        "url" => {
            element.tag = if allow_url { "a" } else { "span" }.into();
            if allow_url {
                set_attr(
                    element,
                    "href",
                    element.text.clone().unwrap_or_else(|| "None".into()),
                );
            }
            set_attr(element, "class", "acl-markup-url".into());
        }
        "a" if !allow_url => {
            element.tag = "span".into();
            element.attributes.retain(|(k, _)| k != "href");
        }
        "sc" => {
            element.tag = "span".into();
            set_attr(element, "style", "font-variant: small-caps".into());
        }
        "fixed-case" => {
            element.tag = "span".into();
            set_attr(element, "class", "acl-fixed-case".into());
        }
        "tex-math" => {
            let mut new = XmlElement::parse(&crate::math::to_html(
                element.text.as_deref().ok_or("math element has no text")?,
            )?)?;
            new.tail = element.tail.take();
            *element = new;
            return Ok(());
        }
        _ => {
            if element.children.is_empty() && element.text.is_none() {
                element.text = Some(String::new());
            }
        }
    }
    let mut children = vec![];
    for mut child in std::mem::take(&mut element.children) {
        if child.tag == "par" {
            child.tag = "br".into();
            let tail = child.tail.take();
            children.push(child);
            children.push(XmlElement {
                tag: "br".into(),
                attributes: vec![],
                text: None,
                children: vec![],
                tail,
            });
        } else {
            convert_html(&mut child, allow_url)?;
            children.push(child);
        }
    }
    element.children = children;
    Ok(())
}
