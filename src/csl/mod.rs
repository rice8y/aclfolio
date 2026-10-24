//! Single-item bibliography rendering following citeproc-py's CSL semantics.

// Port of citeproc-py (BSD-2-Clause); see NOTICE and data/licenses.
// Syntax/locale lookup and output formatting are separate from per-item
// evaluation.

use regex::Regex;
use serde_json::Value;
use std::{collections::BTreeSet, sync::LazyLock};
use style::StyleNode;
use text::{MISSING_NAME, Text, case, escape, format, wrap};

mod dates;
mod names;
mod numbers;
mod style;
#[cfg(test)]
mod tests;
mod text;

static NUMERIC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^[A-Z]*\d+[A-Z]*$").expect("CSL numeric condition"));
static PLURAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\d+[^\d.]+\d+").expect("CSL plural label"));

/// Which CSL element-specific formatting stages apply after evaluation.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Formatting {
    Affixes,
    Number,
    Text,
}

struct Renderer<'a> {
    root: &'a StyleNode,
    bibliography: &'a StyleNode,
    data: &'a Value,
    locales: Vec<&'a StyleNode>,
    suppressed_variables: BTreeSet<String>,
    depth: usize,
    macro_depth: usize,
}

type RenderResult = Result<Option<Text>, String>;

impl<'a> Renderer<'a> {
    fn name_option<'b>(&'b self, node: &'b StyleNode, key: &str) -> Option<&'b str> {
        let parent_key = match key {
            "form" => "name-form",
            "delimiter" => "name-delimiter",
            _ => key,
        };
        let default = match parent_key {
            "delimiter-precedes-et-al" | "delimiter-precedes-last" => Some("contextual"),
            "et-al-min" | "et-al-subsequent-min" => Some("0"),
            "et-al-use-first" | "et-al-subsequent-use-first" => Some("1"),
            "et-al-use-last" => Some("false"),
            "initialize-with-hyphen" => Some("true"),
            "sort-separator" | "name-delimiter" => Some(", "),
            "name-form" => Some("long"),
            "names-delimiter" => Some(""),
            "demote-non-dropping-particle" => Some("display-and-sort"),
            _ => None,
        };

        node.get(key)
            .or_else(|| self.bibliography.get(parent_key))
            .or_else(|| self.root.get(parent_key))
            .or(default)
    }

    fn term(&self, name: &str, form: &str, plural: bool) -> String {
        let form = if form == "long" { None } else { Some(form) };
        let number = if plural { "multiple" } else { "single" };

        for locale in &self.locales {
            let Some(terms) = locale.child("terms") else {
                continue;
            };
            let Some(term) = terms
                .children
                .iter()
                .find(|term| term.get("name") == Some(name) && term.get("form") == form)
            else {
                continue;
            };

            let text = term.child(number).unwrap_or(term).text.as_str();
            return escape(text);
        }
        String::new()
    }

    fn macro_node(&self, name: &str) -> Result<&'a StyleNode, String> {
        self.root
            .children
            .iter()
            .find(|node| node.tag == "macro" && node.get("name") == Some(name))
            .ok_or_else(|| format!("undefined CSL macro {name}"))
    }

    fn references_variable(&self, node: &StyleNode, depth: usize) -> bool {
        if depth > 128 {
            return false;
        }
        match node.tag.as_str() {
            "text" => {
                if let Some(variable) = node.get("variable") {
                    variable != "year-suffix"
                } else if let Some(macro_name) = node.get("macro") {
                    self.macro_node(macro_name)
                        .is_ok_and(|node| self.references_variable(node, depth + 1))
                } else {
                    false
                }
            }
            "names" | "date" | "number" | "group" => true,
            "label" => node.get("variable") == Some("locator"),
            _ => node
                .children
                .iter()
                .any(|child| self.references_variable(child, depth + 1)),
        }
    }

    fn children(&mut self, node: &StyleNode, delimiter: &str) -> RenderResult {
        let mut output = Vec::new();
        for child in &node.children {
            if let Some(text) = self.render(
                child,
                if child.tag == "choose" { delimiter } else { "" },
                None,
            )? {
                output.push(text);
            }
        }
        Ok(Text::join(output, delimiter))
    }

    fn format_output(
        &self,
        node: &StyleNode,
        mut text: Text,
        formatting: Formatting,
    ) -> Option<Text> {
        if text.empty() {
            return None;
        }
        if node.attr("strip-periods", "false") == "true" {
            text = text.map_segments(|s| s.replace('.', ""));
        }
        if formatting != Formatting::Affixes {
            let language = if formatting == Formatting::Text {
                self.data
                    .get("language")
                    .and_then(Value::as_str)
                    .unwrap_or(self.root.attr("default-locale", "en"))
                    .split('-')
                    .next()
                    .unwrap_or("en")
            } else {
                ""
            };
            text = case(node, text, language);
        }
        text = format(node, text);
        if formatting == Formatting::Text && node.attr("quotes", "false") == "true" {
            let mut quoted = Text::segment(self.term("open-quote", "long", false));
            quoted.append(text);
            quoted.append(Text::segment(self.term("close-quote", "long", false)));
            text = quoted;
        }
        Some(wrap(node, text))
    }

    fn render(
        &mut self,
        node: &StyleNode,
        delimiter: &str,
        names_context: Option<&StyleNode>,
    ) -> RenderResult {
        if self.depth >= 128 {
            return Err("CSL recursion limit exceeded".into());
        }
        self.depth += 1;
        let out = self.render_inner(node, delimiter, names_context);
        self.depth -= 1;
        out
    }

    fn render_inner(
        &mut self,
        node: &StyleNode,
        delimiter: &str,
        names_context: Option<&StyleNode>,
    ) -> RenderResult {
        match node.tag.as_str() {
            "layout" => Ok(self
                .children(node, "")?
                .map(|t| format(node, wrap(node, t)))),
            "macro" | "else" | "if" | "else-if" => self.children(node, delimiter),
            "choose" => {
                for branch in &node.children {
                    if branch.tag == "else" || self.condition(branch) {
                        return self.children(branch, delimiter);
                    }
                }
                Ok(None)
            }
            "group" => self.render_group(node),
            "text" => self.render_text(node),
            "number" => self.render_number(node),
            "names" => self.names(node, names_context.unwrap_or(node)),
            "label" => Ok(self.label(node, node.attr("variable", ""), None)),
            "date" => self.date(node),
            "substitute" => self.render_substitute(node, names_context),
            _ => Err(format!("unsupported CSL rendering element {}", node.tag)),
        }
    }

    fn render_group(&mut self, node: &StyleNode) -> RenderResult {
        let mut output = Vec::new();
        let mut references_variables = false;
        let mut rendered_variable = false;
        let delimiter = node.attr("delimiter", "");
        for child in &node.children {
            let references_variable = self.references_variable(child, 0);
            references_variables |= references_variable;
            if let Some(text) = self.render(
                child,
                if child.tag == "choose" { delimiter } else { "" },
                None,
            )? {
                output.push(text);
                rendered_variable |= references_variable;
            }
        }
        // A group containing only missing variables suppresses its literal labels too.
        if references_variables && !rendered_variable {
            return Ok(None);
        }
        Ok(Text::join(output, delimiter)
            .and_then(|text| self.format_output(node, text, Formatting::Affixes)))
    }

    fn render_text(&mut self, node: &StyleNode) -> RenderResult {
        let text = if let Some(variable) = node.get("variable") {
            if self.suppressed_variables.contains(variable) {
                return Ok(None);
            }
            let key = if node.get("form") == Some("short")
                && self.data.get(format!("{variable}-short")).is_some()
            {
                format!("{variable}-short")
            } else {
                variable.into()
            };
            if variable == "citation-number" {
                Some(Text::segment("1"))
            } else if variable.starts_with("page") {
                self.data
                    .get("page")
                    .map(|pages| self.numbers(node, &reference_string(pages), variable, false))
                    .transpose()?
                    .map(Text::segment)
            } else {
                self.data
                    .get(&key)
                    .map(|value| parse_text(&reference_string(value)))
            }
        } else if let Some(macro_name) = node.get("macro") {
            let node = self.macro_node(macro_name)?;
            self.macro_depth += 1;
            let result = self.children(node, "");
            self.macro_depth -= 1;
            result?
        } else if let Some(term) = node.get("term") {
            Some(Text::segment(self.term(
                term,
                node.attr("form", "long"),
                node.attr("plural", "false") == "true",
            )))
        } else {
            node.get("value").map(|s| Text::segment(escape(s)))
        };
        Ok(text.and_then(|text| self.format_output(node, text, Formatting::Text)))
    }

    fn render_number(&mut self, node: &StyleNode) -> RenderResult {
        let variable = node.attr("variable", "");
        let value = self.data.get(if variable == "page-first" {
            "page"
        } else {
            variable
        });
        let text = value
            .map(|value| self.numbers(node, &reference_string(value), variable, true))
            .transpose()?;
        Ok(text.and_then(|text| self.format_output(node, Text::segment(text), Formatting::Number)))
    }

    fn render_substitute(
        &mut self,
        node: &StyleNode,
        names_context: Option<&StyleNode>,
    ) -> RenderResult {
        let mut last = None;
        for child in &node.children {
            let text = self.render(
                child,
                "",
                if child.tag == "names" && child.child("name").is_none() {
                    names_context
                } else {
                    None
                },
            )?;
            let has_output = text.as_ref().is_some_and(|text| !text.empty());
            last = text;
            if has_output {
                if child.tag == "text" {
                    if let Some(variable) = child.get("variable") {
                        // Substitution consumes this variable so it cannot appear again.
                        self.suppressed_variables.insert(variable.into());
                    }
                }
                break;
            }
        }
        Ok(last)
    }

    fn condition(&self, node: &StyleNode) -> bool {
        let mut results = Vec::new();

        for (key, values) in &node.attrs {
            for value in values.split_whitespace() {
                let matched = match key.as_str() {
                    "type" => self.data["type"].as_str() == Some(value),
                    "variable" => value != "locator" && self.data.get(value).is_some(),
                    "is-numeric" => self
                        .data
                        .get(value)
                        .is_some_and(|value| NUMERIC.is_match(&reference_string(value))),
                    "is-uncertain-date" => self
                        .data
                        .get(value)
                        .and_then(|date| date.get("circa"))
                        .is_some_and(|circa| circa.as_i64().unwrap_or(0) != 0 || circa == true),
                    "locator" | "position" => false,
                    _ => continue,
                };
                results.push(matched);
            }
        }
        match node.get("match") {
            Some("any") => results.iter().any(|x| *x),
            Some("none") => !results.iter().any(|x| *x),
            _ => results.iter().all(|x| *x),
        }
    }

    fn label(&self, node: &StyleNode, variable: &str, plural: Option<bool>) -> Option<Text> {
        let plural = match node.attr("plural", "contextual") {
            "always" => true,
            "never" => false,
            _ => plural.unwrap_or_else(|| {
                self.data
                    .get(variable)
                    .is_some_and(|value| PLURAL.is_match(&reference_string(value)))
            }),
        };
        self.format_output(
            node,
            Text::segment(self.term(variable, node.attr("form", "long"), plural)),
            Formatting::Number,
        )
    }
}

fn reference_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        _ => value.to_string(),
    }
}

fn parse_text(text: &str) -> Text {
    Text::segment(
        text.replace("<span class=\"nocase\">", "")
            .replace("</span>", ""),
    )
}

pub(crate) fn render(data: &Value, xml: &str, link: bool) -> Result<String, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
    if doc.root_element().tag_name().namespace() != Some("http://purl.org/net/xbiblio/csl") {
        return Err("missing CSL namespace".into());
    }
    let root = StyleNode::parse(doc.root_element());
    let bibliography = root
        .child("bibliography")
        .ok_or("CSL style has no bibliography")?;
    let layout = bibliography
        .child("layout")
        .ok_or("CSL bibliography has no layout")?;
    let locales = style::select_locales(&root)?;
    if data.get("id").is_none() || data.get("type").is_none() {
        return Err("CSL item requires id and type".into());
    }
    let mut renderer = Renderer {
        root: &root,
        bibliography,
        data,
        locales,
        suppressed_variables: BTreeSet::new(),
        depth: 0,
        macro_depth: 0,
    };
    let text = renderer
        .render(layout, "", None)?
        .ok_or("CSL bibliography produced no entry")?;
    if !link && text.string().contains(MISSING_NAME) {
        return Err("null name segment in bibliography output".into());
    }
    if link {
        let title = data["title"]
            .as_str()
            .ok_or("missing title for citation link")?;
        let url = data["URL"]
            .as_str()
            .ok_or("missing URL for citation link")?;
        Ok(text
            .into_segments()
            .map(|s| {
                if s == title {
                    format!("<a href=\"{url}\">{title}</a>")
                } else {
                    s.replace(MISSING_NAME, "None")
                }
            })
            .collect())
    } else {
        Ok(text.string())
    }
}
