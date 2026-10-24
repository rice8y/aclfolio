//! Numeric fields, localized ordinals and page-range shortening.

use super::text::escape;
use super::{Renderer, StyleNode};

impl Renderer<'_> {
    pub(super) fn numbers(
        &self,
        node: &StyleNode,
        value: &str,
        variable: &str,
        number: bool,
    ) -> Result<String, String> {
        let delimiter = if variable.starts_with("page") {
            let term = self.term("page-range-delimiter", "long", false);
            if term.is_empty() { "–".into() } else { term }
        } else {
            "–".into()
        };
        let mut groups = Vec::new();
        for group in value.split(',') {
            let mut items = Vec::new();
            for item in group.split('&') {
                let item = item.replace('–', "-");
                let parts = item.split('-').collect::<Vec<_>>();
                let result = if parts.len() == 2 {
                    let first = parts[0].trim();
                    let last = parts[1].trim();
                    let first = self.number(node, first, number)?;
                    if variable == "page-first" {
                        first
                    } else {
                        let last = if variable == "page" {
                            self.last_page(&first, last)
                        } else {
                            last.into()
                        };
                        format!("{first}{delimiter}{}", self.number(node, &last, number)?)
                    }
                } else {
                    self.number(node, item.trim(), number)?
                };
                items.push(result);
            }
            groups.push(items.join(" &amp; "));
        }
        Ok(groups.join(", "))
    }

    fn last_page(&self, first: &str, last: &str) -> String {
        let common = first
            .chars()
            .zip(last.chars())
            .take_while(|(a, b)| a == b)
            .count();
        let mut mode = self.root.attr("page-range-format", "");
        if matches!(mode, "chicago" | "chicago-15" | "chicago-16") {
            let number = regex::Regex::new(r"\d+")
                .unwrap()
                .find(first)
                .and_then(|m| m.as_str().parse::<u64>().ok());
            if let Some(number) = number {
                mode = if number < 100 || number % 100 == 0 || (first.len() >= 4 && common < 2) {
                    "expanded"
                } else if number % 100 < 10 {
                    "minimal"
                } else {
                    "minimal-two"
                };
            }
        }
        let skip = match mode {
            "minimal" => common,
            "minimal-two" => common.min(first.chars().count().saturating_sub(2)),
            _ => 0,
        };
        last.chars().skip(skip).collect()
    }

    fn number(&self, node: &StyleNode, text: &str, convert: bool) -> Result<String, String> {
        if !convert {
            return Ok(text.into());
        }
        let Ok(number) = text.parse::<i64>() else {
            return Ok(text.into());
        };
        Ok(match node.attr("form", "numeric") {
            "ordinal" => self.ordinal(number),
            "long-ordinal" if number > 10 => self.ordinal(number),
            "long-ordinal" => self.term(&format!("long-ordinal-{number:02}"), "long", false),
            "roman" => {
                if !(0..100000).contains(&number) {
                    return Err("Roman numeral outside supported range".into());
                }
                let mut remaining = number;
                let mut output = String::new();
                for (value, symbol) in [
                    (1000, "m"),
                    (900, "cm"),
                    (500, "d"),
                    (400, "cd"),
                    (100, "c"),
                    (90, "xc"),
                    (50, "l"),
                    (40, "xl"),
                    (10, "x"),
                    (9, "ix"),
                    (5, "v"),
                    (4, "iv"),
                    (1, "i"),
                ] {
                    while remaining >= value {
                        output.push_str(symbol);
                        remaining -= value;
                    }
                }
                output
            }
            _ => number.to_string(),
        })
    }

    pub(super) fn ordinal(&self, number: i64) -> String {
        let exact = self.term(&format!("ordinal-{number:02}"), "long", false);
        let mut term = exact;
        if term.is_empty() {
            let suffix = format!("ordinal-{:02}", number % 10);
            for locale in &self.locales {
                let Some(terms) = locale.child("terms") else {
                    continue;
                };
                let Some(localized) = terms.children.iter().find(|term| {
                    term.get("name") == Some(suffix.as_str())
                        && !matches!(term.get("match"), Some("whole-number" | "last-two-digits"))
                }) else {
                    continue;
                };
                term = escape(&localized.text);
                break;
            }
        }
        if term.is_empty() {
            term = self.term("ordinal", "long", false);
        }
        format!("{number}{term}")
    }
}
