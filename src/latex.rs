//! LaTeX and BibTeX formatting compatible with the pinned Python
//! implementation.

use crate::{model::NameSpecification, text::RichText};
use std::{collections::BTreeMap, sync::LazyLock};
use unicode_normalization::UnicodeNormalization;

static ENCODE: LazyLock<BTreeMap<String, String>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../data/latex-encode.json")).expect("bundled LaTeX map")
});

/// Encode Unicode for BibTeX, including brace protection of substitutions.
pub fn latex_encode(text: &str) -> String {
    let mut result = String::new();
    for c in text.nfc() {
        let mut buf = [0u8; 4];
        if let Some(s) = ENCODE.get(c.encode_utf8(&mut buf)) {
            result.push_str(s);
        } else {
            result.push(c);
        }
    }
    result
}

/// Test literal brace balance, including early closing braces.
pub fn has_unbalanced_braces(text: &str) -> bool {
    let mut depth = 0;
    for c in text.chars() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ => {}
        }
        if depth < 0 {
            return true;
        }
    }
    depth != 0
}

/// Convert English month specifications into BibTeX macros or concatenations.
pub fn bibtex_convert_month(spec: &str) -> String {
    let months = [
        ("january", "jan"),
        ("february", "feb"),
        ("march", "mar"),
        ("april", "apr"),
        ("may", "may"),
        ("june", "jun"),
        ("july", "jul"),
        ("august", "aug"),
        ("september", "sep"),
        ("october", "oct"),
        ("november", "nov"),
        ("december", "dec"),
    ];
    let mut s = spec.to_lowercase();
    for (name, short) in months {
        if s == name || s == short {
            return short.into();
        }
    }
    s = format!("\"{s}\"");
    for (name, short) in months {
        if s.contains(name) {
            s = s.replace(name, &format!("\" # {short} # \""));
            s = s
                .split(" # ")
                .filter(|s| *s != "\"\"")
                .collect::<Vec<_>>()
                .join(" # ");
        }
    }
    s
}

fn word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Normalize opening/closing quotation marks after LaTeX encoding.
pub fn latex_convert_quotes(text: &str) -> String {
    fn pass(
        text: &str,
        patterns: &[&str],
        replacement: &str,
        opening: bool,
        single: bool,
    ) -> String {
        let mut out = String::new();
        let mut i = 0;
        while i < text.len() {
            let prev = text[..i].chars().next_back();
            let pattern = patterns.iter().find(|p| text[i..].starts_with(**p));
            if let Some(p) = pattern {
                let next = text[i + p.len()..].chars().next();
                let valid = if opening {
                    prev.is_none_or(crate::utils::python_whitespace)
                        && next.is_none_or(|c| {
                            c != '}'
                                && !crate::utils::python_whitespace(c)
                                && (!single || c != '\'')
                        })
                } else {
                    prev != Some('\\') && next.is_none_or(|c| !word(c))
                };
                if valid {
                    out.push_str(replacement);
                    i += p.len();
                    continue;
                }
            }
            let c = text[i..].chars().next().unwrap();
            out.push(c);
            i += c.len_utf8();
        }
        out
    }
    let s = pass(text, &["{``}", "{''}", "''"], "``", true, false);
    let s = pass(&s, &["{`}", "{'}", "'"], "`", true, true);
    let s = pass(&s, &["{''}"], "''", false, false);
    pass(&s, &["{'}"], "'", false, true)
}

/// A typed bibliographic field; formatting depends on the source value's type.
pub enum BibtexValue<'a> {
    /// Already textual metadata (field-specific conversion applies).
    Text(&'a str),
    /// Rich title or abstract.
    Markup(&'a RichText),
    /// Ordered author/editor names.
    Names(&'a [NameSpecification]),
}

/// Convert credits into BibTeX's multi-line author/editor representation.
pub fn namespecs_to_bibtex(names: &[NameSpecification]) -> String {
    names
        .iter()
        .map(|n| n.name.as_bibtex())
        .collect::<Vec<_>>()
        .join("  and\n      ")
}

/// Produce an exact, deterministic BibTeX entry, rejecting unbalanced braces.
pub fn make_bibtex_entry(
    bibtype: &str,
    bibkey: &str,
    fields: &[(&str, BibtexValue<'_>)],
) -> Result<String, String> {
    let mut lines = vec![format!("@{bibtype}{{{bibkey},")];
    for (key, value) in fields {
        let value = match value {
            BibtexValue::Text(s) => {
                if s.is_empty() {
                    continue;
                }
                match *key {
                    "journal" | "address" | "publisher" | "note" => latex_encode(s),
                    "month" => bibtex_convert_month(s),
                    "pages" => {
                        let chars: Vec<_> = s.chars().collect();
                        let mut out = String::new();
                        for (i, c) in chars.iter().enumerate() {
                            if ['-', '–', '—'].contains(c)
                                && i > 0
                                && i + 1 < chars.len()
                                && chars[i - 1].is_ascii_digit()
                                && chars[i + 1].is_ascii_digit()
                            {
                                out.push_str("--");
                            } else {
                                out.push(*c);
                            }
                        }
                        out
                    }
                    _ => s.to_string(),
                }
            }
            BibtexValue::Markup(t) => {
                if t.as_text()?.is_empty() {
                    continue;
                }
                t.as_latex()
            }
            BibtexValue::Names(n) => {
                if n.is_empty() {
                    continue;
                }
                namespecs_to_bibtex(n)
            }
        };
        if has_unbalanced_braces(&value) {
            return Err(format!(
                "BibTeX field {key:?} has unbalanced braces: {value}"
            ));
        }
        let quoted = if *key == "month" {
            value
        } else if value.contains('"') {
            format!("{{{value}}}")
        } else {
            format!("\"{value}\"")
        };
        lines.push(format!("    {key} = {quoted},"));
    }
    lines.last_mut().unwrap().pop();
    lines.push("}".into());
    Ok(lines.join("\n"))
}
