//! English language display names from the pinned language-data/CLDR tables.

use std::{collections::BTreeMap, sync::OnceLock};

#[derive(serde::Deserialize)]
struct Tables {
    names: BTreeMap<String, String>,
    languages: BTreeMap<String, String>,
    territories: BTreeMap<String, String>,
    scripts: BTreeMap<String, String>,
    exceptions: Vec<String>,
}

fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(include_str!("../data/languages.json")).expect("language data")
    })
}

#[derive(Default)]
struct Tag {
    language: String,
    script: Option<String>,
    territory: Option<String>,
}

fn parse(tag: &str, depth: usize) -> Result<Tag, String> {
    if depth > 16 {
        return Err("recursive language alias".into());
    }
    if !tag.is_ascii() {
        return Err("language tags must contain ASCII characters".into());
    }
    let normalized = tag.to_lowercase().replace('_', "-");
    if let Some(alias) = tables().languages.get(&normalized) {
        if alias.to_lowercase() != normalized {
            return parse(alias, depth + 1);
        }
    }
    if tables().exceptions.contains(&normalized) {
        return Ok(Tag {
            language: normalized,
            ..Default::default()
        });
    }
    let parts: Vec<_> = normalized.split('-').collect();
    if parts
        .iter()
        .any(|s| s.is_empty() || s.len() > 8 || !s.bytes().all(|c| c.is_ascii_alphanumeric()))
    {
        return Err("expected 1–8 alphanumeric characters per language subtag".into());
    }
    if parts[0] == "x" {
        return if parts.len() > 1 {
            Ok(Tag {
                language: normalized,
                ..Default::default()
            })
        } else {
            Err("x alone is not a language tag".into())
        };
    }
    if !(2..=4).contains(&parts[0].len()) {
        return Err("expected a language code".into());
    }
    let mut result = if let Some(alias) = tables().languages.get(parts[0]) {
        parse(alias, depth + 1)?
    } else {
        Tag {
            language: parts[0].into(),
            ..Default::default()
        }
    };
    let mut stage = 0;
    let mut i = 1;
    while i < parts.len() {
        let p = parts[i];
        if p.len() == 1 {
            if i + 1 == parts.len() {
                return Err("extension requires a value".into());
            }
            if p == "x" {
                break;
            }
            i += 1;
            if parts[i].len() < 2 {
                return Err("extension requires a value".into());
            }
            while i < parts.len() && parts[i].len() > 1 {
                i += 1;
            }
            stage = 4;
            continue;
        }
        let kind = if (p.len() == 2 && p.bytes().all(|c| c.is_ascii_alphabetic()))
            || (p.len() == 3 && p.bytes().all(|c| c.is_ascii_digit()))
        {
            2
        } else if p.len() == 3 && p.bytes().all(|c| c.is_ascii_alphabetic()) {
            0
        } else if p.len() == 4 && p.bytes().all(|c| c.is_ascii_alphabetic()) {
            1
        } else if p.len() >= 5 || (p.len() == 4 && p.as_bytes()[0].is_ascii_digit()) {
            3
        } else {
            return Err("invalid language subtag".into());
        };
        if kind < stage {
            return Err("language subtags are out of order".into());
        }
        if kind == 0 {
            let mut count = 0;
            while i < parts.len() && parts[i].len() == 3 && count < 3 {
                let minitag = format!("{}-{}", result.language, parts[i]);
                if let Some(alias) = tables().languages.get(&minitag) {
                    let replacement = parse(alias, depth + 1)?;
                    result.language = replacement.language;
                    if replacement.script.is_some() {
                        result.script = replacement.script;
                    }
                    if replacement.territory.is_some() {
                        result.territory = replacement.territory;
                    }
                }
                i += 1;
                count += 1;
            }
            stage = 1;
            continue;
        }
        if kind == 1 {
            result.script = Some(format!("{}{}", p[..1].to_uppercase(), &p[1..]));
            stage = 2;
        } else if kind == 2 {
            result.territory = Some(
                tables()
                    .territories
                    .get(p)
                    .cloned()
                    .unwrap_or_else(|| p.to_uppercase()),
            );
            stage = 3;
        } else {
            stage = 3;
        }
        i += 1;
    }
    Ok(result)
}

fn component(code: &str, fallback: &str) -> String {
    tables().names.get(code).cloned().unwrap_or_else(|| {
        format!(
            "{} [{code}]",
            tables()
                .names
                .get(fallback)
                .map(String::as_str)
                .unwrap_or("Unknown language subtag")
        )
    })
}

/// Resolve a BCP 47 tag to its English display name, suppressing the default
/// script.
/// Unknown but syntactically valid codes receive the reference's descriptive
/// placeholder.
pub fn display_name(tag: &str) -> Result<String, String> {
    let mut t = parse(tag, 0)?;
    if t.script.as_ref() == tables().scripts.get(&t.language) {
        t.script = None;
    }
    let name = component(&t.language, "und");
    let mut extras = vec![];
    if let Some(script) = t.script {
        extras.push(component(&script, "Zzzz"));
    }
    if let Some(territory) = t.territory {
        extras.push(component(&territory, "ZZ"));
    }
    Ok(if extras.is_empty() {
        name
    } else {
        format!("{name} ({})", extras.join(", "))
    })
}

impl crate::PaperRef<'_> {
    /// English name of the paper's language, when specified.
    pub fn language_name(self) -> Result<Option<String>, String> {
        self.language
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(display_name)
            .transpose()
    }
}
