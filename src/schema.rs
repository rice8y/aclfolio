//! Validation against the pinned ACL Anthology Relax NG grammar, entirely in
//! Rust.

use roxmltree::Node;
use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock},
};

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(tag = "op")]
enum Pattern {
    #[serde(rename = "empty")]
    Empty,
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "ref")]
    Ref {
        name: String,
    },
    #[serde(rename = "element")]
    Element {
        name: String,
        child: Arc<Pattern>,
    },
    #[serde(rename = "attribute")]
    Attribute {
        name: String,
        child: Arc<Pattern>,
    },
    #[serde(rename = "data")]
    Data {
        name: String,
        params: BTreeMap<String, String>,
    },
    #[serde(rename = "value")]
    Value {
        value: String,
    },
    #[serde(rename = "group")]
    Group {
        left: Arc<Pattern>,
        right: Arc<Pattern>,
    },
    #[serde(rename = "interleave")]
    Interleave {
        left: Arc<Pattern>,
        right: Arc<Pattern>,
    },
    #[serde(rename = "choice")]
    Choice {
        left: Arc<Pattern>,
        right: Arc<Pattern>,
    },
    #[serde(rename = "optional")]
    Optional {
        child: Arc<Pattern>,
    },
    #[serde(rename = "zeroOrMore")]
    Zero {
        child: Arc<Pattern>,
    },
    #[serde(rename = "oneOrMore")]
    One {
        child: Arc<Pattern>,
    },
    Fail,
}

fn grammar() -> &'static BTreeMap<String, Arc<Pattern>> {
    static GRAMMAR: OnceLock<BTreeMap<String, Arc<Pattern>>> = OnceLock::new();
    GRAMMAR.get_or_init(|| {
        serde_json::from_str(include_str!("../data/schema.json")).expect("bundled schema")
    })
}

fn empty() -> Arc<Pattern> {
    Arc::new(Pattern::Empty)
}

fn fail() -> Arc<Pattern> {
    Arc::new(Pattern::Fail)
}

fn choice(left: Arc<Pattern>, right: Arc<Pattern>) -> Arc<Pattern> {
    if matches!(*left, Pattern::Fail) {
        right
    } else if matches!(*right, Pattern::Fail) || left == right {
        left
    } else {
        Arc::new(Pattern::Choice { left, right })
    }
}

fn group(left: Arc<Pattern>, right: Arc<Pattern>, interleave: bool) -> Arc<Pattern> {
    if matches!(*left, Pattern::Fail) || matches!(*right, Pattern::Fail) {
        fail()
    } else if matches!(*left, Pattern::Empty) {
        right
    } else if matches!(*right, Pattern::Empty) {
        left
    } else if interleave {
        Arc::new(Pattern::Interleave { left, right })
    } else {
        Arc::new(Pattern::Group { left, right })
    }
}

fn nullable(pattern: &Pattern) -> bool {
    match pattern {
        Pattern::Empty | Pattern::Text | Pattern::Optional { .. } | Pattern::Zero { .. } => true,
        Pattern::Group { left, right } | Pattern::Interleave { left, right } => {
            nullable(left) && nullable(right)
        }
        Pattern::Choice { left, right } => nullable(left) || nullable(right),
        Pattern::One { child } => nullable(child),
        Pattern::Ref { name } => nullable(&grammar()[name]),
        Pattern::Data { name, params } => datatype(name, "", params),
        Pattern::Value { value } => value.is_empty(),
        _ => false,
    }
}

#[derive(Clone, Copy)]
enum Token<'a, 'input> {
    Attribute(&'a str, &'a str),
    Text(&'a str),
    Element(Node<'a, 'input>),
}

fn derivative(pattern: &Pattern, token: Token<'_, '_>) -> Arc<Pattern> {
    match pattern {
        Pattern::Ref { name } => derivative(&grammar()[name], token),
        Pattern::Element { name, child } => match token {
            Token::Element(n)
                if n.tag_name().name() == name
                    && n.tag_name().namespace().is_none()
                    && valid_element(child, n) =>
            {
                empty()
            }
            _ => fail(),
        },
        Pattern::Attribute { name, child } => match token {
            Token::Attribute(n, v) if n == name && nullable(&derivative(child, Token::Text(v))) => {
                empty()
            }
            _ => fail(),
        },
        Pattern::Text => {
            if matches!(token, Token::Text(_)) {
                Arc::new(Pattern::Text)
            } else {
                fail()
            }
        }
        Pattern::Data { name, params } => match token {
            Token::Text(text) if datatype(name, text, params) => empty(),
            _ => fail(),
        },
        Pattern::Value { value } => match token {
            Token::Text(text)
                if text.split_whitespace().collect::<Vec<_>>().join(" ") == *value =>
            {
                empty()
            }
            _ => fail(),
        },
        Pattern::Choice { left, right } => {
            choice(derivative(left, token), derivative(right, token))
        }
        Pattern::Group { left, right } | Pattern::Interleave { left, right } => {
            let interleave = matches!(pattern, Pattern::Interleave { .. });
            let a = group(derivative(left, token), right.clone(), interleave);
            if interleave || matches!(token, Token::Attribute(..)) {
                choice(a, group(left.clone(), derivative(right, token), interleave))
            } else if nullable(left) {
                choice(a, derivative(right, token))
            } else {
                a
            }
        }
        Pattern::Optional { child } => derivative(child, token),
        Pattern::Zero { child } | Pattern::One { child } => group(
            derivative(child, token),
            Arc::new(Pattern::Zero {
                child: child.clone(),
            }),
            false,
        ),
        _ => fail(),
    }
}

fn valid_element(pattern: &Arc<Pattern>, node: Node<'_, '_>) -> bool {
    let mut state = pattern.clone();
    for attribute in node.attributes() {
        if attribute.namespace().is_some() {
            return false;
        }
        state = derivative(
            &state,
            Token::Attribute(attribute.name(), attribute.value()),
        );
        if matches!(*state, Pattern::Fail) {
            return false;
        }
    }
    let mut saw_content = false;
    for child in node.children() {
        if child.is_element() {
            saw_content = true;
            state = derivative(&state, Token::Element(child));
        } else if child.is_text() {
            let text = child.text().unwrap();
            let next = derivative(&state, Token::Text(text));
            if !text.trim_matches([' ', '\t', '\r', '\n']).is_empty()
                || !matches!(*next, Pattern::Fail)
            {
                state = next;
                saw_content = true;
            }
        }
        if matches!(*state, Pattern::Fail) {
            return false;
        }
    }
    if !saw_content && !nullable(&state) {
        state = derivative(&state, Token::Text(""));
    }
    nullable(&state)
}

fn regex_match(pattern: &str, value: &str) -> bool {
    static CACHE: OnceLock<std::sync::Mutex<BTreeMap<String, regex::Regex>>> = OnceLock::new();
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap();
    if !cache.contains_key(pattern) {
        let Ok(re) = regex::Regex::new(&format!("^(?:{pattern})$")) else {
            return false;
        };
        cache.insert(pattern.into(), re);
    }
    cache[pattern].is_match(value)
}

fn ncname(value: &str) -> bool {
    let mut chars = value.chars();
    chars.next().is_some_and(|c| c == '_' || c.is_alphabetic())
        && chars.all(|c| {
            c.is_alphanumeric()
                || matches!(c, '_' | '-' | '.' | '\u{b7}')
                || unicode_normalization::char::canonical_combining_class(c) != 0
        })
}

/// Check an XML Schema date, including leap years and optional timezone
/// offsets.
pub fn is_xsd_date(value: &str) -> bool {
    static DATE_PATTERN: OnceLock<regex::Regex> = OnceLock::new();
    let date_pattern = DATE_PATTERN.get_or_init(|| {
        regex::Regex::new(r"^(-?[0-9]{4,})-([0-9]{2})-([0-9]{2})(Z|[+-]([0-9]{2}):([0-9]{2}))?$")
            .unwrap()
    });
    let Some(captures) = date_pattern.captures(value) else {
        return false;
    };
    let Ok(year) = captures[1].parse::<i64>() else {
        return false;
    };
    let month = captures[2].parse::<usize>().unwrap();
    let day = captures[3].parse::<usize>().unwrap();
    if year == 0 || month == 0 || month > 12 || day == 0 {
        return false;
    }
    let days_in_month = [
        31,
        if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            29
        } else {
            28
        },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ][month - 1];
    if day > days_in_month {
        return false;
    }
    if let (Some(hours), Some(minutes)) = (captures.get(5), captures.get(6)) {
        let hours = hours.as_str().parse::<u8>().unwrap();
        let minutes = minutes.as_str().parse::<u8>().unwrap();
        if hours > 14 || minutes > 59 || (hours == 14 && minutes != 0) {
            return false;
        }
    }
    true
}

fn datatype(name: &str, value: &str, params: &BTreeMap<String, String>) -> bool {
    let collapsed = value
        .split([' ', '\t', '\r', '\n'])
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let normalized = if name == "string" { value } else { &collapsed };
    let valid = match name {
        "string" | "anyURI" => true,
        "NCName" => ncname(normalized),
        "NMTOKEN" => {
            !normalized.is_empty()
                && normalized.chars().all(|c| {
                    c.is_alphanumeric()
                        || matches!(c, '_' | '-' | '.' | ':' | '\u{b7}')
                        || unicode_normalization::char::canonical_combining_class(c) != 0
                })
        }
        "positiveInteger" | "nonNegativeInteger" => {
            regex_match(r"[+-]?[0-9]+", normalized)
                && !normalized.starts_with('-')
                && (name == "nonNegativeInteger"
                    || normalized.chars().any(|c| ('1'..='9').contains(&c)))
        }
        "date" => is_xsd_date(normalized),
        "gYear" => {
            regex_match(
                r"-?[0-9]{4,}(?:Z|[+-](?:0[0-9]|1[0-4]):[0-5][0-9])?",
                normalized,
            ) && !normalized.starts_with("0000")
        }
        "boolean" => ["true", "false", "1", "0"].contains(&normalized),
        "language" => regex_match(r"[A-Za-z]{1,8}(?:-[A-Za-z0-9]{1,8})*", normalized),
        _ => false,
    };
    valid
        && params.iter().all(|(key, p)| match key.as_str() {
            "pattern" => regex_match(p, normalized),
            "minLength" => normalized.chars().count() >= p.parse().unwrap(),
            "maxLength" => normalized.chars().count() <= p.parse().unwrap(),
            _ => false,
        })
}

/// A document that fails the pinned Relax NG schema.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct SchemaError {
    /// Validation or XML parser detail.
    pub message: String,
}

/// Validate original XML bytes without modifying metadata or requiring external
/// libraries.
pub fn validate_xml(xml: &str) -> Result<(), SchemaError> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| SchemaError {
        message: e.to_string(),
    })?;
    if nullable(&derivative(
        &grammar()["#start"],
        Token::Element(doc.root_element()),
    )) {
        Ok(())
    } else {
        Err(SchemaError {
            message: format!(
                "document <{}> does not match the pinned ACL Anthology Relax NG schema",
                doc.root_element().tag_name().name()
            ),
        })
    }
}

/// Read and validate an original XML file.
pub fn validate_file(path: impl AsRef<std::path::Path>) -> Result<(), SchemaError> {
    let xml = std::fs::read_to_string(path).map_err(|e| SchemaError {
        message: e.to_string(),
    })?;
    validate_xml(&xml)
}
