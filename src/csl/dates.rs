//! Date parts and ranges, including locale-specific formats.

use super::text::wrap;
use super::{Formatting, RenderResult, Renderer, StyleNode, Text};
use serde_json::Value;

impl Renderer<'_> {
    pub(super) fn date(&mut self, node: &StyleNode) -> RenderResult {
        let Some(value) = self.data.get(node.attr("variable", "")) else {
            return Ok(None);
        };
        if let Some(literal) = value.get("literal").and_then(Value::as_str) {
            return Ok(Some(wrap(node, Text::segment(literal))));
        }
        let Some(dates) = value.get("date-parts").and_then(Value::as_array) else {
            return Ok(None);
        };
        if dates.is_empty() {
            return Ok(None);
        }
        let format_node = if let Some(form) = node.get("form") {
            self.locales
                .iter()
                .find_map(|locale| {
                    locale
                        .children
                        .iter()
                        .find(|child| child.tag == "date" && child.get("form") == Some(form))
                })
                .ok_or("missing locale date format")?
        } else {
            node
        };
        let parts = node
            .attr("date-parts", "year-month-day")
            .split('-')
            .collect::<Vec<_>>();
        if dates.len() > 1 {
            let begin = &dates[0];
            let end = &dates[1];
            let mut shared_parts = Vec::new();
            let mut differing_parts = parts.clone();
            for (index, part) in ["year", "month", "day"].iter().enumerate() {
                if begin.get(index).is_some() && begin.get(index) == end.get(index) {
                    shared_parts.push(*part);
                    differing_parts.retain(|candidate| candidate != part);
                } else {
                    break;
                }
            }
            let begin_text = self.date_single(format_node, node, begin, &differing_parts)?;
            let end_text = self.date_single(format_node, node, end, &differing_parts)?;
            let shared_text = self.date_single(format_node, node, end, &shared_parts)?;
            let Some(begin_text) = begin_text else {
                return Ok(None);
            };
            let mut text = Text::segment(begin_text.string().trim_end().to_owned());
            text.append(Text::segment("–"));
            if let Some(end_text) = end_text {
                text.append(end_text);
            }
            if let Some(shared_text) = shared_text {
                if node.get("form").is_none() && self.macro_depth > 0 {
                    return Err("date range context has no join operation".into());
                }
                text.append(Text::segment(node.attr("delimiter", "")));
                text.append(Text::segment(shared_text.string().trim_end()));
            }
            return Ok(Some(wrap(node, text)));
        }
        Ok(self
            .date_single(format_node, node, &dates[0], &parts)?
            .map(|text| wrap(node, text)))
    }

    fn date_single(
        &self,
        node: &StyleNode,
        context: &StyleNode,
        date: &Value,
        parts: &[&str],
    ) -> RenderResult {
        let mut output = Vec::new();
        for original in &node.children {
            let mut part = original.clone();
            let name = original.attr("name", "");
            if !parts.contains(&name) {
                continue;
            }
            if let Some(part_override) = context
                .children
                .iter()
                .find(|c| c.get("name") == Some(name))
            {
                part.attrs.extend(part_override.attrs.clone());
            }
            let index = match name {
                "year" => 0,
                "month" => 1,
                "day" => 2,
                _ => continue,
            };
            let Some(value) = date.get(index) else {
                continue;
            };
            let number = value
                .as_i64()
                .or_else(|| value.as_str().and_then(|digits| digits.parse().ok()))
                .ok_or("invalid date part")?;
            let form = part.attr("form", if name == "day" { "numeric" } else { "long" });
            let text = match (name, form) {
                ("year", "short") => {
                    let digits = number.to_string();
                    digits
                        .chars()
                        .skip(digits.len().saturating_sub(2))
                        .collect()
                }
                ("year", _) => format!(
                    "{}{}",
                    number.abs(),
                    if number < 0 {
                        self.term("bc", "long", false)
                    } else if number < 1000 {
                        self.term("ad", "long", false)
                    } else {
                        String::new()
                    }
                ),
                ("month", "long" | "short") => {
                    self.term(&format!("month-{number:02}"), form, false)
                }
                (_, "numeric-leading-zeros") => format!("{number:02}"),
                (_, "ordinal") => self.ordinal(number),
                _ => number.to_string(),
            };
            if let Some(formatted) =
                self.format_output(&part, Text::segment(text), Formatting::Number)
            {
                output.push(formatted);
            }
        }
        Ok(Text::join(
            output,
            if node.tag == "date" && node.get("variable").is_none() {
                context.attr("delimiter", "")
            } else {
                node.attr("delimiter", "")
            },
        ))
    }
}
