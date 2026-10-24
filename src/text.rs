//! Loss-aware mixed XML text, independent of the XML parser.

use crate::utils::{xml_attribute, xml_escape};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(crate) mod normalize;

/// One mixed-content node. Unknown markup is retained in compatible mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextNode {
    /// Entity-decoded Unicode text.
    Text(String),
    /// Markup, including formatting, links, fixed case, math and paragraph breaks.
    Element {
        /// Original element local name.
        name: String,
        /// Attribute values in deterministic key order.
        attributes: BTreeMap<String, String>,
        /// Ordered mixed content.
        children: Vec<TextNode>,
    },
}

/// Ordered mixed content, preserving absence/empty distinctions in its parent.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RichText(pub Vec<TextNode>);

impl RichText {
    /// Collapse Unicode whitespace, recursively unwrap markup, preserve TeX source,
    /// and emit a blank line for `<par/>`. No case folding or TeX interpretation.
    pub fn plain_text(&self) -> String {
        let mut paragraphs = vec![String::new()];
        collect_paragraphs(&self.0, &mut paragraphs);
        paragraphs
            .iter()
            .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>()
            .join("\n\n")
            .trim()
            .to_owned()
    }

    /// Construct a plain text value, optionally applying Anthology normalization.
    pub fn from_string(text: &str, clean: bool) -> Self {
        Self(vec![TextNode::Text(if clean {
            crate::utils::clean_unicode(text)
        } else {
            text.into()
        })])
    }

    /// Parse the children of an XML wrapper without interpreting LaTeX.
    pub fn from_xml(xml: &str) -> Result<Self, String> {
        let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
        Ok(Self::from_node(doc.root_element()))
    }

    pub(crate) fn from_node(node: roxmltree::Node<'_, '_>) -> Self {
        Self(node.children().filter_map(parse_mixed_node).collect())
    }

    /// Whether any markup element is present.
    pub fn contains_markup(&self) -> bool {
        self.0.iter().any(|n| matches!(n, TextNode::Element { .. }))
    }

    /// Inner XML, preserving whitespace and escaping literal text.
    pub fn as_xml(&self) -> String {
        let xml = serialize_nodes(&self.0);
        if self.contains_markup() {
            xml.trim_matches(crate::utils::python_whitespace).into()
        } else {
            xml
        }
    }

    /// Serialize inside a named XML wrapper without writing a file.
    pub fn to_xml(&self, tag: &str) -> String {
        let xml = serialize_nodes(&self.0);
        if xml.is_empty() {
            format!("<{tag}/>")
        } else {
            format!("<{tag}>{xml}</{tag}>")
        }
    }

    /// Plain text with TeX math interpreted as Unicode and paragraph breaks
    /// preserved.
    pub fn as_text(&self) -> Result<String, String> {
        let mut text = String::new();
        write_text(&self.0, &mut text)?;
        let text = crate::utils::remove_extra_whitespace(&text);
        Ok(normalize::paragraphs(&text, "\n\n"))
    }

    /// HTML representation, optionally disabling hyperlink targets.
    pub fn as_html(&self, allow_url: bool) -> Result<String, String> {
        Ok(normalize::html(&render_html(&self.0, allow_url)?))
    }

    /// LaTeX representation with Unicode encoding and preserved math source.
    pub fn as_latex(&self) -> String {
        normalize::latex(&render_latex(&self.0))
    }

    /// Test containment in plain text or XML, as in MarkupText's string protocol.
    pub fn contains(&self, needle: &str) -> Result<bool, String> {
        Ok(self.as_text()?.contains(needle) || self.as_xml().contains(needle))
    }

    /// Unicode scalar count of the interpreted plain text.
    pub fn text_len(&self) -> Result<usize, String> {
        Ok(self.as_text()?.chars().count())
    }

    /// Test a prefix on an XML substring using Python-style character indices.
    pub fn starts_with(&self, prefixes: &[&str], start: Option<isize>, end: Option<isize>) -> bool {
        self.xml_slice(start, end)
            .is_some_and(|s| prefixes.iter().any(|p| s.starts_with(p)))
    }

    /// Test a suffix on an XML substring using Python-style character indices.
    pub fn ends_with(&self, suffixes: &[&str], start: Option<isize>, end: Option<isize>) -> bool {
        self.xml_slice(start, end)
            .is_some_and(|s| suffixes.iter().any(|p| s.ends_with(p)))
    }

    fn xml_slice(&self, start: Option<isize>, end: Option<isize>) -> Option<String> {
        normalize::slice(&self.as_xml(), start, end)
    }
}

fn collect_paragraphs(nodes: &[TextNode], paragraphs: &mut Vec<String>) {
    for node in nodes {
        match node {
            TextNode::Text(text) => paragraphs
                .last_mut()
                .expect("initial paragraph")
                .push_str(text),
            TextNode::Element { name, children, .. } if name == "par" => {
                paragraphs.push(String::new());
                collect_paragraphs(children, paragraphs);
            }
            TextNode::Element { children, .. } => collect_paragraphs(children, paragraphs),
        }
    }
}

fn parse_mixed_node(node: roxmltree::Node<'_, '_>) -> Option<TextNode> {
    if node.is_text() {
        return Some(TextNode::Text(node.text().unwrap_or_default().into()));
    }
    node.is_element().then(|| TextNode::Element {
        name: node.tag_name().name().into(),
        attributes: node
            .attributes()
            .map(|a| (a.name().into(), a.value().into()))
            .collect(),
        children: node.children().filter_map(parse_mixed_node).collect(),
    })
}

fn write_text(nodes: &[TextNode], output: &mut String) -> Result<(), String> {
    for node in nodes {
        match node {
            TextNode::Text(text) => output.push_str(text),
            TextNode::Element { name, children, .. } => match name.as_str() {
                "par" => output.push('\u{e000}'),
                "tex-math" => {
                    let tex = children
                        .iter()
                        .filter_map(|node| {
                            if let TextNode::Text(text) = node {
                                Some(text.as_str())
                            } else {
                                None
                            }
                        })
                        .collect::<String>();
                    output.push_str(&crate::math::to_unicode(&tex)?);
                }
                _ => write_text(children, output)?,
            },
        }
    }
    Ok(())
}

fn render_html(nodes: &[TextNode], allow_url: bool) -> Result<String, String> {
    let mut output = String::new();
    for node in nodes {
        match node {
            TextNode::Text(text) => output.push_str(&xml_escape(text)),
            TextNode::Element {
                name,
                attributes,
                children,
            } => {
                let inner = render_html(children, allow_url)?;
                match name.as_str() {
                    "tex-math" => {
                        let tex = children
                            .iter()
                            .filter_map(|node| {
                                if let TextNode::Text(text) = node {
                                    Some(text.as_str())
                                } else {
                                    None
                                }
                            })
                            .collect::<String>();
                        output.push_str(&crate::math::to_html(&tex)?);
                    }
                    "par" => output.push_str("<br/><br/>"),
                    "url" => {
                        if allow_url {
                            output.push_str(&format!(
                                "<a href=\"{}\" class=\"acl-markup-url\">{inner}</a>",
                                xml_attribute(
                                    &children
                                        .iter()
                                        .filter_map(|node| if let TextNode::Text(text) = node {
                                            Some(text.as_str())
                                        } else {
                                            None
                                        })
                                        .collect::<String>()
                                )
                            ));
                        } else {
                            output.push_str(&format!(
                                "<span class=\"acl-markup-url\">{inner}</span>"
                            ));
                        }
                    }
                    "sc" => output.push_str(&format!(
                        "<span style=\"font-variant: small-caps\">{inner}</span>"
                    )),
                    "fixed-case" => {
                        output.push_str(&format!("<span class=\"acl-fixed-case\">{inner}</span>"));
                    }
                    _ => {
                        let tag = if name == "a" && !allow_url {
                            "span"
                        } else {
                            name
                        };
                        output.push_str(&format!("<{tag}"));
                        for (k, v) in attributes {
                            if !(name == "a" && !allow_url && k == "href") {
                                output.push_str(&format!(" {k}=\"{}\"", xml_attribute(v)));
                            }
                        }
                        output.push_str(&format!(">{inner}</{tag}>"));
                    }
                }
            }
        }
    }
    Ok(output)
}

fn render_latex(nodes: &[TextNode]) -> String {
    let mut output = String::new();
    for node in nodes {
        match node {
            TextNode::Text(text) => output.push_str(&crate::latex::latex_encode(text)),
            TextNode::Element {
                name,
                attributes,
                children,
            } => {
                if name == "tex-math" || name == "url" {
                    let raw = children
                        .first()
                        .and_then(|node| {
                            if let TextNode::Text(text) = node {
                                Some(text.as_str())
                            } else {
                                None
                            }
                        })
                        .unwrap_or("None");
                    output.push_str(&if name == "tex-math" {
                        format!("${raw}$")
                    } else {
                        format!("\\url{{{raw}}}")
                    });
                    continue;
                }
                let inner = render_latex(children);
                let text = match name.as_str() {
                    "fixed-case" => format!("{{{inner}}}"),
                    "b" => format!("\\textbf{{{inner}}}"),
                    "i" => format!("\\textit{{{inner}}}"),
                    "u" => format!("\\underline{{{inner}}}"),
                    "sc" => format!("\\textsc{{{inner}}}"),
                    "tt" => format!("\\texttt{{{inner}}}"),
                    "par" => "\\par ".into(),
                    "a" => format!(
                        "\\href{{{}}}{{{inner}}}",
                        attributes.get("href").map(String::as_str).unwrap_or("")
                    ),
                    _ => inner,
                };
                output.push_str(&text);
            }
        }
    }
    output
}

/// Serialize mixed nodes as XML character data and elements.
pub(crate) fn serialize_nodes(nodes: &[TextNode]) -> String {
    let mut out = String::new();
    for node in nodes {
        match node {
            TextNode::Text(s) => out.push_str(&xml_escape(s).replace('\r', "&#13;")),
            TextNode::Element {
                name,
                attributes,
                children,
            } => {
                out.push('<');
                out.push_str(name);
                for (k, v) in attributes {
                    out.push_str(&format!(" {k}=\"{}\"", xml_attribute(v)));
                }
                if children.is_empty() {
                    out.push_str("/>");
                } else {
                    out.push('>');
                    out.push_str(&serialize_nodes(children));
                    out.push_str(&format!("</{name}>"));
                }
            }
        }
    }
    out
}
