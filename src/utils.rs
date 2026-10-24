//! Pure normalization and validation utilities matching the Python reference.

use unicode_normalization::UnicodeNormalization;

/// Split a page range using the reference's ordered separator rules.
pub fn interpret_pages(text: &str) -> (&str, &str) {
    for separator in ["--", "-", "–"] {
        if text.matches(separator).count() == 1 {
            return text.split_once(separator).expect("one occurrence");
        }
    }
    (text, text)
}

/// English full month name to month number; no abbreviation or whitespace
/// trimming.
pub fn month_str2num(text: &str) -> Option<u8> {
    [
        "january",
        "february",
        "march",
        "april",
        "may",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
    ]
    .iter()
    .position(|m| *m == text.to_lowercase())
    .map(|i| i as u8 + 1)
}

/// Remove LF characters, trim Unicode whitespace, and collapse runs of ASCII
/// spaces.
/// Interior tabs and other Unicode whitespace are deliberately retained.
pub fn remove_extra_whitespace(text: &str) -> String {
    let without_lf = text.replace('\n', "");
    let mut previous_space = false;
    without_lf
        .trim_matches(python_whitespace)
        .chars()
        .filter(|&c| {
            let keep = c != ' ' || !previous_space;
            previous_space = c == ' ';
            keep
        })
        .collect()
}

pub(crate) fn python_whitespace(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// Apply Anthology's selective Unicode decomposition and NFC normalization.
pub fn clean_unicode(text: &str) -> String {
    let mut value = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        let c = match c {
            '\u{ad}' => continue,
            '\u{2010}' | '\u{2011}' => '-',
            'ı' if chars.peek().is_some_and(|c| is_nonspacing_mark(*c)) => 'i',
            c => c,
        };
        if let Some(decomposed) = unicode_decomposition(c) {
            value.push_str(decomposed);
        } else {
            value.push(c);
        }
    }
    value.nfc().collect()
}
include!("unicode_tables.rs");

/// Parse the four XML Schema boolean spellings, rejecting all others.
pub fn xsd_boolean(value: &str) -> Result<bool, crate::ids::IdError> {
    match value {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(crate::ids::IdError(value.into())),
    }
}

/// Escape XML character data.
pub fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Escape a double-quoted XML attribute.
pub fn xml_attribute(text: &str) -> String {
    xml_escape(text)
        .replace('"', "&quot;")
        .replace('\t', "&#9;")
        .replace('\n', "&#10;")
        .replace('\r', "&#13;")
}

/// Validate the reference API's lexical ISO date format (calendar validity is
/// separate).
pub fn validate_iso_date(value: &str) -> Result<(), String> {
    let bytes = value.as_bytes();
    if bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
    {
        Ok(())
    } else {
        Err(format!("invalid ISO date {value:?}"))
    }
}

/// Container spelling for the reference's compact publication-ID
/// representation.
#[derive(Debug, Clone, Copy)]
pub enum IdContainer {
    /// Python list representation.
    List,
    /// Python tuple representation.
    Tuple,
    /// Python set representation; callers supply their chosen iteration order.
    Set,
}

/// Format at most five identifiers with the total count, without loading
/// records.
pub fn repr_item_ids(
    ids: &[crate::ids::IdParts],
    container: IdContainer,
) -> Result<String, crate::IdError> {
    let (name, left, right, empty) = match container {
        IdContainer::List => ("list", "[", "]", "[]"),
        IdContainer::Tuple => ("tuple", "(", ")", "()"),
        IdContainer::Set => ("set", "{", "}", "set()"),
    };
    if ids.is_empty() {
        return Ok(empty.into());
    }
    let mut shown = ids
        .iter()
        .take(5)
        .map(|id| {
            crate::ids::build_id(&id.0, id.1.as_deref(), id.2.as_deref()).map(|s| format!("'{s}'"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if ids.len() > 5 {
        shown.push("...".into());
    }
    Ok(format!(
        "<{name}[AnthologyIDTuple] with {} item{} {left}{}{right}>",
        ids.len(),
        if ids.len() == 1 { "" } else { "s" },
        shown.join(", ")
    ))
}

pub(crate) fn increment_decimal(value: &str) -> String {
    let mut digits = value.trim_start_matches('0').as_bytes().to_vec();
    for digit in digits.iter_mut().rev() {
        if *digit < b'9' {
            *digit += 1;
            return String::from_utf8(digits).unwrap();
        }
        *digit = b'0';
    }
    digits.insert(0, b'1');
    String::from_utf8(digits).unwrap()
}
