//! Name selection, particles, initialization, labels and et-al truncation.

use super::text::{MISSING_NAME, case, format, wrap};
use super::{Formatting, RenderResult, Renderer, StyleNode, Text};
use serde_json::Value;

impl Renderer<'_> {
    pub(super) fn names(&mut self, node: &StyleNode, context: &StyleNode) -> RenderResult {
        let mut output = Vec::new();
        let mut count = 0;
        let mut found = false;
        let default_name = StyleNode {
            tag: "name".into(),
            ..StyleNode::default()
        };
        let name = context.child("name").unwrap_or(&default_name);
        let roles = node
            .attr("variable", "")
            .split_whitespace()
            .collect::<Vec<_>>();
        let merged = roles.len() == 2
            && roles.contains(&"editor")
            && roles.contains(&"translator")
            && self.data.get("editor").is_some()
            && self.data.get("editor") == self.data.get("translator")
            && !self.term("editortranslator", "long", false).is_empty();
        for role in roles {
            if merged && role == "translator" {
                continue;
            }
            if let Some(names) = self.data.get(role) {
                found = true;
                let names = names.as_array().ok_or("CSL names must be an array")?;
                if self.name_option(name, "form") == Some("count") {
                    count += names.len().min(
                        self.name_option(name, "et-al-use-first")
                            .unwrap_or("1")
                            .parse()
                            .unwrap_or(1),
                    );
                    continue;
                }
                let name_text = self.name(name, context, names)?;
                let name_missing = name_text.is_none();
                let mut text = name_text.unwrap_or_default();
                if let Some(label) = context.child("label") {
                    if let Some(label_text) = self.label(
                        label,
                        if merged { "editortranslator" } else { role },
                        Some(names.len() > 1),
                    ) {
                        if name_missing {
                            text = Text::segment(MISSING_NAME);
                        }
                        if context
                            .children
                            .first()
                            .is_some_and(|child| child.tag == "label")
                        {
                            let mut label_text = label_text;
                            label_text.append(text);
                            text = label_text;
                        } else {
                            text.append(label_text);
                        }
                    }
                }
                output.push(text);
            }
        }
        let text = if self.name_option(name, "form") == Some("count") && found {
            if count == 0 {
                None
            } else {
                Some(Text::segment(count.to_string()))
            }
        } else if found {
            Text::join(
                output,
                node.attr(
                    "delimiter",
                    self.bibliography
                        .attr("names-delimiter", self.root.attr("names-delimiter", "")),
                ),
            )
        } else if let Some(substitute) = node.child("substitute") {
            self.render(substitute, "", Some(context))?
        } else {
            None
        };
        Ok(text.and_then(|text| self.format_output(node, text, Formatting::Affixes)))
    }

    fn name(&self, node: &StyleNode, context: &StyleNode, names: &[Value]) -> RenderResult {
        let option = |key| self.name_option(node, key);
        let integer_option = |key| option(key).unwrap_or("0").parse::<usize>().unwrap_or(0);
        let minimum_names = integer_option("et-al-min");
        let first_names = integer_option("et-al-use-first");
        let truncate = names.len() > 1 && minimum_names > 0 && names.len() >= minimum_names;
        let include_last =
            option("et-al-use-last") == Some("true") && first_names + 2 <= minimum_names;
        let selected: Vec<_> = if truncate {
            names
                .iter()
                .take(first_names)
                .chain(if include_last { names.last() } else { None })
                .collect()
        } else {
            names.iter().collect()
        };
        let delimiter = option("delimiter").unwrap_or(", ");
        let mut output = Vec::new();
        for (index, value) in selected.iter().enumerate() {
            let get = |key| {
                value
                    .get(key)
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned()
            };
            let mut given = get("given");
            let mut family = if value.get("literal").is_some() {
                get("literal")
            } else {
                get("family")
            };
            let dropping_particle = get("dropping-particle");
            let non_dropping_particle = get("non-dropping-particle");
            let suffix = get("suffix");
            if let Some(mark) = option("initialize-with").filter(|_| value.get("given").is_some()) {
                given = initialize(
                    &given,
                    mark,
                    option("initialize-with-hyphen") != Some("false"),
                );
            }
            let sorted = option("name-as-sort-order") == Some("all")
                || (option("name-as-sort-order") == Some("first") && index == 0);
            let short = option("form") == Some("short");
            if short {
                family = space(&[&non_dropping_particle, &family]);
            } else if sorted {
                if matches!(
                    option("demote-non-dropping-particle"),
                    Some("never" | "sort-only")
                ) {
                    family = space(&[&non_dropping_particle, &family]);
                    given = space(&[&given, &dropping_particle]);
                } else {
                    given = space(&[&given, &dropping_particle, &non_dropping_particle]);
                }
            } else {
                family = space(&[&dropping_particle, &non_dropping_particle, &family]);
            }
            for part in node.children.iter().filter(|part| part.tag == "name-part") {
                if matches!(
                    part.get("text-case"),
                    Some("capitalize-first" | "capitalize-all" | "sentence")
                ) {
                    return Err(
                        "name-part case operation is unsupported by the reference processor".into(),
                    );
                }
                let target = if part.get("name") == Some("given") {
                    &mut given
                } else {
                    &mut family
                };
                let text = case(part, Text::segment(target.clone()), "");
                let text = format(part, text);
                *target = wrap(part, text).string();
            }
            let text = if short {
                family
            } else if sorted {
                [family, given, suffix]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(option("sort-separator").unwrap_or(", "))
            } else {
                space(&[&given, &family, &suffix])
            };
            output.push(Text::plain(text));
        }
        let et_al = if let Some(et_al_node) = context.child("et-al") {
            self.format_output(
                et_al_node,
                Text::segment(self.term(et_al_node.attr("term", "et-al"), "long", false)),
                Formatting::Affixes,
            )
            .unwrap_or_default()
        } else {
            Text::segment(self.term("et-al", "long", false))
        };
        let text = if truncate && !et_al.empty() {
            if include_last {
                if let Some(text) = output.last_mut() {
                    let mut ellipsis = Text::segment("… ");
                    ellipsis.append(text.clone());
                    *text = ellipsis;
                }
                Text::join(output, delimiter)
            } else if option("delimiter-precedes-et-al") == Some("always")
                || (option("delimiter-precedes-et-al") == Some("contextual") && output.len() >= 2)
            {
                output.push(et_al);
                Text::join(output, delimiter)
            } else {
                let mut text = Text::join(output, delimiter).unwrap_or_default();
                text.append(Text::segment(" "));
                text.append(et_al);
                Some(text)
            }
        } else if let Some(conjunction) = option("and").filter(|_| output.len() > 1) {
            let last_name = output.pop().unwrap();
            let len = output.len() + 1;
            let mut text = Text::join(output, node.attr("delimiter", ", ")).unwrap_or_default();
            text.append(Text::segment(
                if option("delimiter-precedes-last") == Some("always")
                    || (option("delimiter-precedes-last") == Some("contextual") && len > 2)
                {
                    node.attr("delimiter", ", ")
                } else {
                    " "
                },
            ));
            text.append(Text::segment(format!(
                "{} ",
                if conjunction == "symbol" {
                    "&amp;".into()
                } else {
                    self.term("and", "long", false)
                }
            )));
            text.append(last_name);
            Some(text)
        } else {
            Text::join(output, delimiter)
        };
        Ok(text.and_then(|text| self.format_output(node, text, Formatting::Affixes)))
    }
}

fn space(parts: &[&str]) -> String {
    parts
        .iter()
        .filter(|s| !s.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" ")
}

fn initialize(given: &str, mark: &str, hyphen: bool) -> String {
    let given = if hyphen {
        given.to_owned()
    } else {
        given.replace('-', " ")
    };
    given
        .split('-')
        .map(|p| {
            let mut text = String::new();
            let mut group = Vec::new();
            for part in p.replace('.', " ").split_whitespace() {
                let c = part.chars().next().unwrap();
                if c.is_uppercase() {
                    group.push(c.to_string());
                } else {
                    text.push_str(&group.join(mark));
                    text.push_str(mark);
                    text.push(' ');
                    text.push_str(part);
                    text.push(' ');
                    group.clear();
                }
            }
            text.push_str(&group.join(mark));
            text.push_str(mark);
            text.split_whitespace().collect::<Vec<_>>().join(" ")
        })
        .collect::<Vec<_>>()
        .join("-")
}
