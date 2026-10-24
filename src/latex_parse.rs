//! Parser for the LaTeX subset used by MarkupText constructors.

use crate::text::{RichText, TextNode};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::LazyLock};
use unicode_normalization::UnicodeNormalization;

#[derive(Deserialize)]
struct Macro {
    args: String,
    discard: bool,
    text: Option<String>,
    function: bool,
}

static MACROS: LazyLock<BTreeMap<String, Macro>> =
    LazyLock::new(|| serde_json::from_str(include_str!("../data/latex-macros.json")).unwrap());

fn text(out: &mut Vec<TextNode>, input: &str) {
    if input.is_empty() {
        return;
    }
    if let Some(TextNode::Text(t)) = out.last_mut() {
        t.push_str(input);
    } else {
        out.push(TextNode::Text(input.into()));
    }
}

fn element(name: &str, children: Vec<TextNode>) -> TextNode {
    TextNode::Element {
        name: name.into(),
        attributes: Default::default(),
        children,
    }
}

fn plain(nodes: &[TextNode]) -> String {
    let mut output = String::new();
    for node in nodes {
        match node {
            TextNode::Text(text) => output.push_str(text),
            TextNode::Element { children, .. } => output.push_str(&plain(children)),
        }
    }
    output
}

fn normalize_text(out: &mut Vec<TextNode>, input: &str, root: bool) {
    static PAR: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"[^\S\r\n]*(?:(?:\r\n?|\n)[^\S\r\n]*){2,}").unwrap());

    static LINE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"[^\S\r\n]*(?:(?:\r\n?|\n)[^\S\r\n]*)+").unwrap());
    if root {
        for (offset, p) in PAR.split(input).enumerate() {
            if offset > 0 {
                out.push(element("par", vec![]));
            }
            text(out, &LINE.replace_all(p, " "));
        }
    } else {
        text(out, &LINE.replace_all(input, " "));
    }
}

/// Reusable conversion context for LaTeX document title, author and date
/// macros.
/// Ordinary title/abstract conversions can use `RichText::from_latex` directly.
#[derive(Debug, Clone)]
pub struct LatexContext {
    title: String,
    author: String,
    date: String,
}

impl Default for LatexContext {
    fn default() -> Self {
        Self {
            title: r"[NO \title GIVEN]".into(),
            author: r"[NO \author GIVEN]".into(),
            date: today(),
        }
    }
}

fn today() -> String {
    chrono::Local::now().format("%B %-d, %Y").to_string()
}

struct Parser<'a> {
    input: &'a str,
    offset: usize,
    depth: usize,
    context: &'a mut LatexContext,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.input[self.offset..].chars().next()
    }

    fn take(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.offset += c.len_utf8();
        Some(c)
    }

    fn skip_space(&mut self) {
        while self.peek().is_some_and(crate::utils::python_whitespace) {
            self.take();
        }
    }

    fn nodes(&mut self, closing: Option<char>, root: bool) -> Result<Vec<TextNode>, String> {
        self.depth += 1;
        if self.depth > 256 {
            return Err("LaTeX nesting exceeds 256".into());
        }
        let mut out = vec![];
        while let Some(c) = self.peek() {
            if Some(c) == closing {
                self.take();
                break;
            }
            match c {
                '{' => {
                    self.take();
                    let start = self.offset;
                    let protect = !matches!(
                        self.peek(),
                        None | Some('}' | '\\' | '$' | '`' | '\'' | '~')
                    ) && !self.input[start..].starts_with("--");
                    let children = self.nodes(Some('}'), if protect { false } else { root })?;
                    if protect {
                        out.push(element("fixed-case", children));
                    } else {
                        out.extend(children);
                    }
                }
                '}' => {
                    self.take();
                }
                '\\' => out.extend(self.command(root)?),
                '$' => {
                    let start = self.offset;
                    self.take();
                    let double = self.peek() == Some('$');
                    if double {
                        self.take();
                    }
                    let content_start = self.offset;
                    let mut end = self.input.len();
                    while let Some(c) = self.take() {
                        if c == '\\' {
                            self.take();
                            continue;
                        }
                        if c == '$' {
                            end = self.offset - 1;
                            if double && self.peek() == Some('$') {
                                self.take();
                            }
                            break;
                        }
                    }
                    let content = &self.input[content_start..end];
                    let trivial = content.replace("\\%", "%");
                    if trivial.chars().all(|c| {
                        crate::utils::python_whitespace(c) || c.is_numeric() || ".,@%~".contains(c)
                    }) {
                        text(&mut out, &trivial.replace('~', "\u{a0}"));
                    } else {
                        let raw = self.input[start..self.offset].trim_matches('$');
                        out.push(element(
                            "tex-math",
                            if raw.is_empty() {
                                vec![]
                            } else {
                                vec![TextNode::Text(raw.into())]
                            },
                        ));
                    }
                }
                '%' => {
                    while self.peek().is_some_and(|c| c != '\n' && c != '\r') {
                        self.take();
                    }
                    if self.peek() == Some('\r') {
                        self.take();
                    }
                    if self.peek() == Some('\n') {
                        self.take();
                    }
                    while matches!(self.peek(), Some(' ' | '\t')) {
                        self.take();
                    }
                }
                '~' => {
                    self.take();
                    text(&mut out, "\u{a0}");
                }
                '`' | '\'' => {
                    self.take();
                    if self.peek() == Some(c) {
                        self.take();
                        text(&mut out, if c == '`' { "“" } else { "”" });
                    } else {
                        text(&mut out, if c == '`' { "‘" } else { "’" });
                    }
                }
                '-' => {
                    self.take();
                    if self.peek() == Some('-') {
                        self.take();
                        if self.peek() == Some('-') {
                            self.take();
                            text(&mut out, "—");
                        } else {
                            text(&mut out, "–");
                        }
                    } else {
                        text(&mut out, "-");
                    }
                }
                _ => {
                    let start = self.offset;
                    while self
                        .peek()
                        .is_some_and(|c| !"{}\\$%~`'-".contains(c) && Some(c) != closing)
                    {
                        self.take();
                    }
                    normalize_text(&mut out, &self.input[start..self.offset], root);
                }
            }
        }
        self.depth -= 1;
        Ok(out)
    }

    fn argument(&mut self, optional: bool) -> Result<Option<(Vec<TextNode>, String)>, String> {
        let before = self.offset;
        self.skip_space();
        let start = self.offset;
        if optional && self.peek() != Some('[') {
            self.offset = before;
            return Ok(None);
        }
        if matches!(self.peek(), Some('{')) || (optional && self.peek() == Some('[')) {
            let open = self.take().unwrap();
            let nodes = self.nodes(Some(if open == '{' { '}' } else { ']' }), false)?;
            let close = if open == '{' { '}' } else { ']' };
            let end = if self.input[..self.offset].ends_with(close) {
                self.offset - 1
            } else {
                self.offset
            };
            return Ok(Some((
                nodes,
                self.input[start + 1..end.max(start + 1)].into(),
            )));
        }
        if optional {
            return Ok(None);
        }
        if self.peek() == Some('\\') {
            let nodes = self.command(false)?;
            let raw = self.input[start..self.offset].into();
            return Ok(Some((nodes, raw)));
        }
        Ok(self
            .take()
            .map(|c| (vec![TextNode::Text(c.to_string())], c.to_string())))
    }

    fn command(&mut self, _root: bool) -> Result<Vec<TextNode>, String> {
        self.take();
        let start = self.offset;
        while self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
            self.take();
        }
        let alpha = self.offset > start;
        if !alpha {
            self.take();
        }
        let name = &self.input[start..self.offset];
        if alpha {
            self.skip_space();
        }
        if name == "\\" {
            self.argument(true)?;
            return Ok(vec![TextNode::Text("\n".into())]);
        }
        if name == "(" || name == "[" {
            let end = if name == "(" { "\\)" } else { "\\]" };
            let start = start - 1;
            if let Some(pos) = self.input[self.offset..].find(end) {
                self.offset += pos + end.len();
            } else {
                self.offset = self.input.len();
            }
            return Ok(vec![element(
                "tex-math",
                vec![TextNode::Text(self.input[start..self.offset].into())],
            )]);
        }
        if name == "begin" {
            if let Some((_, env)) = self.argument(false)? {
                let end = format!("\\end{{{env}}}");
                if let Some(pos) = self.input[self.offset..].find(&end) {
                    self.offset += pos + end.len();
                } else {
                    self.offset = self.input.len();
                }
            }
            return Ok(vec![]);
        }
        if name == "verb" {
            let before = self.offset;
            if let Some(delimiter) = self.take() {
                if let Some(end) = self.input[self.offset..].find(delimiter) {
                    self.offset += end + delimiter.len_utf8();
                } else {
                    self.offset = before;
                }
            }
            return Ok(vec![]);
        }
        let spec = MACROS.get(name);
        let mut args: Vec<Option<(Vec<TextNode>, String)>> = vec![];
        if let Some(spec) = spec {
            for c in spec.args.chars() {
                match c {
                    '*' => {
                        if self.peek() == Some('*') {
                            self.take();
                            args.push(Some((vec![TextNode::Text("*".into())], "*".into())));
                        } else {
                            args.push(None);
                        }
                    }
                    '[' => args.push(self.argument(true)?),
                    '{' => args.push(self.argument(false)?),
                    _ => {}
                }
            }
        }
        let tag = match name {
            "emph" | "em" | "textit" | "it" | "textsl" | "sl" => Some("i"),
            "textbf" | "bf" => Some("b"),
            "underline" | "uline" => Some("u"),
            "textsc" => Some("sc"),
            "texttt" => Some("tt"),
            "url" => Some("url"),
            _ => None,
        };
        if let Some(tag) = tag {
            return Ok(vec![element(
                tag,
                args.into_iter().flatten().flat_map(|(n, _)| n).collect(),
            )]);
        }
        if name == "href" {
            let href = args
                .first()
                .and_then(|a| a.as_ref())
                .map(|(_, input)| input.clone())
                .unwrap_or_default();
            let children = args
                .get_mut(1)
                .and_then(Option::take)
                .map(|(n, _)| n)
                .unwrap_or_default();
            return Ok(vec![TextNode::Element {
                name: "a".into(),
                attributes: BTreeMap::from([("href".into(), href)]),
                children,
            }]);
        }
        let argtext: Vec<String> = args
            .iter()
            .map(|a| a.as_ref().map(|(n, _)| plain(n)).unwrap_or_default())
            .collect();
        match name {
            "title" => {
                self.context.title = argtext.first().cloned().unwrap_or_default();
                return Ok(vec![]);
            }
            "author" => {
                self.context.author = argtext.first().cloned().unwrap_or_default();
                return Ok(vec![]);
            }
            "date" => {
                self.context.date = argtext.first().cloned().unwrap_or_default();
                return Ok(vec![]);
            }
            "today" => return Ok(vec![TextNode::Text(today())]),
            "maketitle" => {
                let c = &self.context;
                let len = c
                    .title
                    .chars()
                    .count()
                    .max(4 + c.author.chars().count())
                    .max(4 + c.date.chars().count());
                return Ok(vec![TextNode::Text(format!(
                    "{}\n    {}\n    {}\n{}\n\n",
                    c.title,
                    c.author,
                    c.date,
                    "=".repeat(len)
                ))]);
            }
            _ => {}
        }
        let mut output = String::new();
        if let Some(spec) = spec {
            if let Some(t) = &spec.text {
                output = t.clone();
                if let Some(offset) = output.find("%s") {
                    let v = argtext.first().map(String::as_str).unwrap_or("");
                    output.replace_range(offset..offset + 2, v);
                }
                if t.matches("%s").count() > 1 {
                    let mut value = t.clone();
                    for a in &argtext {
                        value = value.replacen("%s", a, 1);
                    }
                    output = value;
                }
                for (offset, a) in argtext.iter().enumerate() {
                    output = output.replace(&format!("%({})s", offset + 1), a);
                }
            } else if spec.function {
                output = function(name, &argtext);
            } else if !spec.discard {
                output = argtext.join("");
            }
        }
        let mut nodes = vec![];
        text(&mut nodes, &output);
        Ok(nodes)
    }
}

fn function(name: &str, args: &[String]) -> String {
    let arg = args.first().map(String::as_str).unwrap_or("");
    let accent = match name {
        "'" | "acute" => Some('\u{301}'),
        "`" | "grave" => Some('\u{300}'),
        "\"" | "ddot" => Some('\u{308}'),
        "^" | "hat" => Some('\u{302}'),
        "~" | "tilde" => Some('\u{303}'),
        "=" => Some('\u{304}'),
        "." | "dot" => Some('\u{307}'),
        "u" | "breve" => Some('\u{306}'),
        "v" | "check" => Some('\u{30c}'),
        "H" => Some('\u{30b}'),
        "c" => Some('\u{327}'),
        "k" => Some('\u{328}'),
        "b" => Some('\u{331}'),
        "d" => Some('\u{323}'),
        "r" => Some('\u{30a}'),
        "bar" => Some('\u{305}'),
        "vec" => Some('\u{20d7}'),
        "not" => Some('\u{338}'),
        _ => None,
    };
    if let Some(accent) = accent {
        return arg
            .trim_matches(crate::utils::python_whitespace)
            .chars()
            .flat_map(|c| {
                [
                    if c == 'ı' {
                        'i'
                    } else if c == 'ȷ' {
                        'j'
                    } else {
                        c
                    },
                    accent,
                ]
            })
            .collect::<String>()
            .nfc()
            .collect();
    }

    static FONTS: LazyLock<BTreeMap<String, BTreeMap<String, String>>> =
        LazyLock::new(|| serde_json::from_str(include_str!("../data/math-fonts.json")).unwrap());
    if let Some(font) = FONTS.get(name) {
        return arg
            .chars()
            .map(|c| {
                font.get(&c.to_string())
                    .cloned()
                    .unwrap_or_else(|| c.to_string())
            })
            .collect();
    }
    let heading = args.get(2).map(String::as_str).unwrap_or("");
    match name {
        "%" => "%".into(),
        "texorpdfstring" => args.get(1).cloned().unwrap_or_default(),
        "includegraphics" => "\n    < g r a p h i c s >\n".into(),
        "chapter" => format!("\n\nCHAPTER: {}\n", heading.to_uppercase()),
        "section" => format!("\n\n§ {}\n", heading.to_uppercase()),
        "subsection" => format!("\n\n §.§ {heading}\n"),
        "subsubsection" => format!("\n\n  §.§.§ {heading}\n"),
        "paragraph" => format!("\n\n  {heading}\n"),
        "subparagraph" => format!("\n\n    {heading}\n"),
        "part" => format!("\n\nPART: {}\n", heading.to_uppercase()),
        "item" => format!("\n  {}", if arg.is_empty() { "* " } else { arg }),
        "exercise" | "uebung" => format!(
            "\n{arg}\n{}",
            args.get(1)
                .filter(|input| !input.is_empty())
                .map(|input| format!("[{input}]\n"))
                .unwrap_or_default()
        ),
        "input" | "include" | "title" | "author" | "date" | "newcommand" => String::new(),
        _ => arg.into(),
    }
}

/// Parse LaTeX into detached Anthology markup; applies optional plain-text
/// heuristics.
pub fn parse_latex_to_xml(
    input: &str,
    use_fixed_case: bool,
    use_heuristics: bool,
) -> Result<RichText, String> {
    LatexContext::default().parse(input, use_fixed_case, use_heuristics)
}

impl LatexContext {
    /// Convert LaTeX while retaining document title/author/date declarations
    /// between calls.
    pub fn parse(
        &mut self,
        input: &str,
        _use_fixed_case: bool,
        use_heuristics: bool,
    ) -> Result<RichText, String> {
        let mut prepared = String::new();
        let chars: Vec<_> = input.chars().collect();
        for (offset, c) in chars.iter().enumerate() {
            if use_heuristics && *c == '%' && (offset == 0 || chars[offset - 1] != '\\') {
                prepared.push_str("\\%");
            } else if use_heuristics
                && *c == '~'
                && (offset == 0 || [' ', '('].contains(&chars[offset - 1]))
                && chars.get(offset + 1).is_some_and(|c| c.is_numeric())
            {
                prepared.push_str("\\textasciitilde ");
            } else {
                prepared.push(*c);
            }
        }
        Ok(RichText(
            Parser {
                input: &prepared,
                offset: 0,
                depth: 0,
                context: self,
            }
            .nodes(None, true)?,
        ))
    }
}

impl RichText {
    /// Construct markup from LaTeX with optional Unicode normalization.
    pub fn from_latex(input: &str, clean: bool) -> Result<Self, String> {
        let input = if clean {
            crate::utils::clean_unicode(input)
        } else {
            input.into()
        };
        parse_latex_to_xml(&input, true, false)
    }

    /// Construct markup from a string that may contain literal percent signs or
    /// tildes.
    pub fn from_latex_maybe(input: &str, clean: bool) -> Result<Self, String> {
        let input = if clean {
            crate::utils::clean_unicode(input)
        } else {
            input.into()
        };
        parse_latex_to_xml(&input, true, true)
    }
}
