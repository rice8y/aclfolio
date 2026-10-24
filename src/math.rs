//! Anthology's deliberately limited TeX math rendering, with unknown commands
//! retained.

use crate::text::{TextNode, serialize_nodes};
use std::{collections::BTreeMap, sync::LazyLock};

static SYMBOLS: LazyLock<BTreeMap<String, String>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../data/math-symbols.json")).expect("bundled math symbols")
});

fn text(out: &mut Vec<TextNode>, s: &str) {
    if s.is_empty() {
        return;
    }
    if let Some(TextNode::Text(t)) = out.last_mut() {
        t.push_str(s);
    } else {
        out.push(TextNode::Text(s.into()));
    }
}

fn elem(name: &str, attr: Option<(&str, &str)>, children: Vec<TextNode>) -> TextNode {
    TextNode::Element {
        name: name.into(),
        attributes: attr
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect(),
        children,
    }
}

struct Parser<'a> {
    s: &'a str,
    i: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.s[self.i..].chars().next()
    }

    fn take(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.i += c.len_utf8();
        Some(c)
    }

    fn group(&mut self, closing: Option<char>) -> Result<Vec<TextNode>, String> {
        let mut out = vec![];
        let mut script = None;
        while let Some(c) = self.peek() {
            if Some(c) == closing {
                self.take();
                return Ok(out);
            }
            match c {
                '{' => {
                    self.take();
                    let children = self.group(Some('}'))?;
                    if let Some(tag) = script.take() {
                        out.push(elem(tag, None, children));
                    } else {
                        out.extend(children);
                    }
                }
                '}' => return Err("unmatched TeX closing brace".into()),
                '$' => {
                    self.take();
                    out.extend(self.group(Some('$'))?);
                }
                '%' => {
                    let start = self.i;
                    while self.peek().is_some_and(|c| c != '\n') {
                        self.take();
                    }
                    text(&mut out, &self.s[start..self.i]);
                    script = None;
                }
                '^' | '_' => {
                    self.take();
                    script = Some(if c == '^' { "sup" } else { "sub" });
                }
                '\\' => {
                    self.take();
                    let start = self.i;
                    script = None;
                    while self.peek().is_some_and(|c| c.is_alphabetic()) {
                        self.take();
                    }
                    if self.i == start {
                        if let Some(c) = self.take() {
                            let key = c.to_string();
                            text(
                                &mut out,
                                SYMBOLS
                                    .get(&key)
                                    .map(String::as_str)
                                    .unwrap_or(&self.s[start - 1..self.i]),
                            );
                        }
                        script = None;
                        continue;
                    }
                    let name = &self.s[start..self.i];
                    // TexSoup combines left/right followed by an escaped delimiter into one
                    // command.
                    if name == "left" || name == "right" {
                        continue;
                    }
                    let mut args = vec![];
                    let mut rawargs = String::new();
                    let maxargs = if name == "textbf" || name == "label" {
                        1
                    } else if ["cap", "cup", "in", "notin", "infty", "noindent"].contains(&name) {
                        0
                    } else {
                        usize::MAX
                    };
                    while args.len() < maxargs {
                        let before = self.i;
                        while self.peek().is_some_and(char::is_whitespace) {
                            self.take();
                        }
                        let closing = match self.peek() {
                            Some('{') => '}',
                            Some('[') => ']',
                            _ => {
                                self.i = before;
                                break;
                            }
                        };
                        let start = self.i;
                        self.take();
                        args.push(self.group(Some(closing))?);
                        rawargs.push_str(&self.s[start..self.i]);
                    }
                    let raw = format!("{name}{rawargs}");
                    if let Some(symbol) = SYMBOLS.get(name) {
                        text(&mut out, symbol);
                        for arg in args {
                            out.extend(arg);
                        }
                    } else if let Some(symbol) = SYMBOLS.get(&raw) {
                        text(&mut out, symbol);
                    } else if ["lim", "log", "min", "max"].contains(&name) {
                        out.push(elem(
                            "span",
                            Some(("class", "tex-math-function")),
                            vec![TextNode::Text(name.into())],
                        ));
                        for arg in args {
                            out.extend(arg);
                        }
                    } else if name == "frac" && args.len() == 2 {
                        out.push(elem("sup", None, args.remove(0)));
                        text(&mut out, "⁄");
                        out.push(elem("sub", None, args.remove(0)));
                    } else if let Some((tag, attr)) = formatting(name) {
                        if !args.is_empty() {
                            out.push(elem(tag, attr, args.into_iter().flatten().collect()));
                        }
                    } else if ["bigl", "bigr", "mathsf", "sf", "normalsize"].contains(&name) {
                        out.extend(args.into_iter().flatten());
                    } else {
                        out.push(elem(
                            "span",
                            Some(("class", "tex-math-unparsed")),
                            vec![TextNode::Text(format!("\\{name}{}", rawargs))],
                        ));
                    }
                }
                _ => {
                    if closing.is_some()
                        && c.is_whitespace()
                        && (out.is_empty() || matches!(out.last(), Some(TextNode::Element { .. })))
                    {
                        let after = self.s[self.i..]
                            .trim_start_matches(char::is_whitespace)
                            .chars()
                            .next();
                        if after.is_none() || matches!(after, Some('(' | '\\' | '}' | ']')) {
                            while self.peek().is_some_and(char::is_whitespace) {
                                self.take();
                            }
                            continue;
                        }
                    }
                    self.take();
                    if let Some(tag) = script.take() {
                        out.push(elem(tag, None, vec![TextNode::Text(c.to_string())]));
                    } else {
                        text(&mut out, &c.to_string());
                    }
                }
            }
        }
        if closing.is_some() {
            return Err("unclosed TeX group".into());
        }
        Ok(out)
    }
}

fn formatting(name: &str) -> Option<(&'static str, Option<(&'static str, &'static str)>)> {
    Some(match name {
        "mathrm" | "textrm" | "rm" | "text" => ("span", Some(("class", "fw-normal"))),
        "mathbf" | "textbf" | "bf" | "bm" | "boldsymbol" | "pmb" => ("strong", None),
        "mathit" | "textit" | "mathcal" | "it" => ("span", Some(("class", "fst-italic"))),
        "emph" => ("em", None),
        "textsc" | "sc" => ("span", Some(("style", "font-variant: small-caps;"))),
        "texttt" | "mathtt" | "tt" => ("span", Some(("class", "font-monospace"))),
        "textsubscript" => ("sub", None),
        "textsuperscript" => ("sup", None),
        "small" => ("span", Some(("class", "small"))),
        _ => return None,
    })
}

/// Render an inline TeX expression to Anthology HTML. Unbalanced groups are
/// errors.
pub fn to_html(tex: &str) -> Result<String, String> {
    let nodes = Parser { s: tex, i: 0 }.group(None)?;
    Ok(serialize_nodes(&[elem(
        "span",
        Some(("class", "tex-math")),
        nodes,
    )]))
}

/// Render math to Unicode, dropping HTML formatting but retaining unknown
/// commands.
pub fn to_unicode(tex: &str) -> Result<String, String> {
    fn visit(nodes: &[TextNode], out: &mut String) {
        for n in nodes {
            match n {
                TextNode::Text(t) => out.push_str(t),
                TextNode::Element { children, .. } => visit(children, out),
            }
        }
    }
    let nodes = Parser { s: tex, i: 0 }.group(None)?;
    let mut result = String::new();
    visit(&nodes, &mut result);
    Ok(result)
}

/// Loaded mapping of TeX command names (without the backslash) to Unicode text.
pub fn command_map() -> &'static BTreeMap<String, String> {
    &SYMBOLS
}

/// HTML tag and attributes used for a supported formatting command.
pub fn html_formatting(
    command: &str,
) -> Option<(&'static str, Option<(&'static str, &'static str)>)> {
    formatting(command)
}

/// Convert an XML math element, retaining its tail and consuming its direct
/// text.
pub fn element_to_html(element: &crate::xml::XmlElement) -> Result<crate::xml::XmlElement, String> {
    let text = element.text.as_deref().ok_or("math element has no text")?;
    let mut result = crate::xml::XmlElement::parse(&to_html(text)?)?;
    result.tail = element.tail.clone();
    Ok(result)
}

/// Match the reference string overload, which first parses the expression as
/// XML content.
pub fn xml_string_to_html(text: &str) -> Result<String, String> {
    let e = crate::xml::XmlElement::parse(&format!("<span>{text}</span>"))?;
    Ok(element_to_html(&e)?.serialize(true))
}

/// Convert the direct text of a math XML element to Unicode, excluding its
/// tail.
pub fn element_to_unicode(element: &crate::xml::XmlElement) -> Result<String, String> {
    to_unicode(element.text.as_deref().ok_or("math element has no text")?)
}
