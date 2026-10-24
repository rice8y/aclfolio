//! Detached name values, normalization, validation and serialization.

use crate::{
    model::Name,
    utils::{xml_attribute, xml_escape},
};

impl Name {
    /// Construct a name with a non-empty last component.
    pub fn new(
        first: Option<String>,
        last: String,
        script: Option<String>,
    ) -> Result<Self, String> {
        if last.is_empty() {
            return Err("last name must not be empty".into());
        }
        Ok(Self {
            first,
            last,
            script,
            extensions: Default::default(),
        })
    }

    /// Full name in first-last order.
    pub fn as_first_last(&self) -> String {
        match self.first.as_deref().filter(|s| !s.is_empty()) {
            Some(f) => format!("{f} {}", self.last),
            None => self.last.clone(),
        }
    }

    /// Full name in last-first order.
    pub fn as_last_first(&self) -> String {
        match self.first.as_deref().filter(|s| !s.is_empty()) {
            Some(f) => format!("{}, {f}", self.last),
            None => self.last.clone(),
        }
    }

    /// Script-aware full name (Han uses last-first with no space).
    pub fn as_full(&self) -> String {
        if self.script.as_deref() == Some("hani") {
            format!("{}{}", self.last, self.first.as_deref().unwrap_or(""))
        } else {
            self.as_first_last()
        }
    }

    /// BibTeX representation of this name.
    pub fn as_bibtex(&self) -> String {
        crate::latex::latex_encode(&self.as_last_first())
    }

    /// Parse an unambiguous first-last or last-comma-first string.
    pub fn from_string(name: &str) -> Result<Self, String> {
        let name = name.trim_matches(crate::utils::python_whitespace);
        let parts: Vec<_> = if name.contains(", ") {
            {
                let mut p: Vec<_> = name.split(", ").collect();
                p.reverse();
                p
            }
        } else {
            name.split(' ').collect()
        };
        match parts.as_slice() {
            [last] => Self::new(None, (*last).into(), None),
            [first, last] => Self::new(Some((*first).into()), (*last).into(), None),
            _ => Err(format!("name cannot be unambiguously split: {name}")),
        }
    }

    /// Serialize using Python's name dictionary convention, omitting absent fields.
    pub fn to_dict(&self) -> serde_json::Value {
        let mut out = serde_json::Map::new();
        if let Some(s) = &self.first {
            out.insert("first".into(), s.clone().into());
        }
        out.insert("last".into(), self.last.clone().into());
        if let Some(s) = &self.script {
            out.insert("script".into(), s.clone().into());
        }
        out.into()
    }

    /// Serialize a detached name to an XML wrapper.
    pub fn to_xml(&self, tag: &str) -> String {
        let attr = self
            .script
            .as_ref()
            .map(|s| format!(" script=\"{}\"", xml_attribute(s)))
            .unwrap_or_default();
        let first = match self.first.as_deref().filter(|s| !s.is_empty()) {
            Some(s) => format!("<first>{}</first>", xml_escape(s)),
            None => "<first/>".into(),
        };
        format!(
            "<{tag}{attr}>{first}<last>{}</last></{tag}>",
            xml_escape(&self.last)
        )
    }
}

use std::{collections::BTreeMap, sync::LazyLock};
use unicode_normalization::UnicodeNormalization;

static TRANSLITERATION: LazyLock<BTreeMap<u32, String>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../data/unidecode.json"))
        .expect("bundled Unidecode 1.4.0 table")
});

/// Slugify a string using the reference's default Latin transliteration rules.
pub fn slugify(text: &str, name_replacements: bool) -> String {
    let text = if name_replacements {
        text.replace(['ʼ', '’'], "-")
    } else {
        text.into()
    };
    let text = text.replace('\'', "-");
    let mut transliterated = String::new();
    for c in text.nfkd() {
        if c.is_ascii() {
            transliterated.push(c);
        } else if let Some(s) = TRANSLITERATION.get(&(c as u32)) {
            transliterated.push_str(s);
        }
    }
    let text = decode_entities(&transliterated)
        .nfkd()
        .collect::<String>()
        .to_lowercase()
        .replace('\'', "");
    let chars: Vec<_> = text.chars().collect();
    let mut out = String::new();
    for (i, c) in chars.iter().enumerate() {
        if *c == ','
            && i > 0
            && i + 1 < chars.len()
            && chars[i - 1].is_ascii_digit()
            && chars[i + 1].is_ascii_digit()
        {
            continue;
        }
        if c.is_ascii_alphanumeric() {
            out.push(*c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').into()
}

fn decode_entities(text: &str) -> String {
    static ENTITIES: LazyLock<BTreeMap<String, u32>> =
        LazyLock::new(|| serde_json::from_str(include_str!("../data/html-entities.json")).unwrap());

    static RE: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"&([A-Za-z][A-Za-z0-9]*|#[0-9]+|#x[0-9a-fA-F]+);").unwrap()
    });
    RE.replace_all(text, |c: &regex::Captures<'_>| {
        let v = &c[1];
        let n = if let Some(x) = v.strip_prefix("#x") {
            u32::from_str_radix(x, 16).ok()
        } else if let Some(x) = v.strip_prefix('#') {
            x.parse().ok()
        } else {
            ENTITIES.get(v).copied()
        };
        n.and_then(char::from_u32)
            .map(|c| c.to_string())
            .unwrap_or_else(|| c[0].into())
    })
    .into_owned()
}

fn python_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

impl Name {
    /// Python name equality ignores the script and treats absent/empty first names
    /// alike.
    pub fn equivalent(&self, other: &Self) -> bool {
        self.first.as_deref().unwrap_or("") == other.first.as_deref().unwrap_or("")
            && self.last == other.last
    }

    /// Name slug used by the official identity-resolution algorithm.
    pub fn slugify(&self) -> String {
        slugify(&self.as_first_last(), true)
    }

    /// Heuristic canonical-name score from the Python reference.
    pub fn score(&self) -> f64 {
        let name = self.as_first_last();
        let mut score = name.chars().count() as f64;
        score += name.chars().filter(|c| !c.is_ascii() || *c == '-').count() as f64;
        for word in name.split(|c| !python_word(c)) {
            if word.chars().skip(1).any(char::is_uppercase) {
                score -= 1.0;
            }
            if word.chars().next().is_some_and(char::is_lowercase) {
                score -= 1.0;
            }
        }
        if name.chars().next().is_some_and(char::is_lowercase) {
            score -= 1.0;
        }
        if self
            .first
            .as_ref()
            .is_some_and(|s| s.chars().count() > self.last.chars().count())
        {
            score += 0.5;
        }
        score
    }

    /// Validate name punctuation, spacing, digits and capitalization of initials.
    pub fn is_valid(&self) -> bool {
        valid_part(self.first.as_deref().unwrap_or(""), false) && valid_part(&self.last, true)
    }

    /// Return an explicit validation error instead of a boolean.
    pub fn validate(&self) -> Result<(), String> {
        if self.is_valid() {
            Ok(())
        } else {
            Err(format!("invalid name: {}", self.as_first_last()))
        }
    }

    /// Normalize wholly upper/lowercase names and lowercase initials without
    /// changing this value.
    pub fn case_normalize(&self, force: bool) -> Self {
        if self.script.is_some() {
            return self.clone();
        }
        let value = self.as_first_last();
        let cased: Vec<_> = value
            .chars()
            .filter(|c| c.is_lowercase() || c.is_uppercase())
            .collect();
        let title = force
            || (!cased.is_empty()
                && (cased.iter().all(|c| c.is_lowercase())
                    || cased.iter().all(|c| c.is_uppercase())));
        let mut first = self.first.as_deref().map(normalize_initials);
        let mut last = normalize_initials(&self.last);
        if title {
            first = first.map(|s| title_case(&s));
            last = title_case(&last);
            let prefixes = [
                "van den", "van der", "von der", "de la", "della", "dela", "bin", "bint", "del",
                "dos", "van", "von", "al", "da", "de", "di", "du", "el", "la", "le",
            ];
            for p in prefixes {
                let prefix = format!("{p} ");
                if last.to_lowercase().starts_with(&prefix) {
                    last = format!("{prefix}{}", &last[prefix.len()..]);
                    break;
                }
            }
            if last.starts_with("Mc") && last.as_bytes().get(2).is_some_and(u8::is_ascii_lowercase)
            {
                last.replace_range(2..3, &last[2..3].to_uppercase());
            }
        }
        Self {
            first,
            last,
            script: None,
            extensions: self.extensions.clone(),
        }
    }
}

fn title_case(text: &str) -> String {
    let mut previous_cased = false;
    let mut out = String::new();
    for c in text.chars() {
        if previous_cased {
            out.extend(c.to_lowercase());
        } else {
            out.extend(c.to_uppercase());
        }
        previous_cased = c.is_lowercase() || c.is_uppercase();
    }
    out
}

fn normalize_initials(text: &str) -> String {
    let mut chars: Vec<_> = text.chars().collect();
    for i in 0..chars.len() {
        let c = chars[i];
        if c.is_ascii_lowercase() {
            let before_word = i == 0 || !python_word(chars[i - 1]);
            let after_dot = chars.get(i + 1) == Some(&'.');
            if (before_word && c != 'v' && after_dot)
                || (i > 0 && chars[i - 1] == '.' && (after_dot || i + 1 == chars.len()))
            {
                chars[i] = c.to_ascii_uppercase();
            }
        }
    }
    let text: String = chars.into_iter().collect();

    static TOKEN: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"(^|\s)([A-Za-z](?:\.[A-Za-z]+)+\.?)(\s|$)").unwrap());
    // Preserve whitespace and run twice to cover adjacent tokens sharing a
    // boundary.
    let pass = |s: &str| {
        TOKEN
            .replace_all(s, |c: &regex::Captures<'_>| {
                format!("{}{}{}", &c[1], title_case(&c[2]), &c[3])
            })
            .into_owned()
    };
    pass(&pass(&text))
}

fn valid_part(value: &str, last: bool) -> bool {
    static VALID: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r#"^[^ '\",.:;!@#$%^&*()=+/?<>\[\]`\\–-](\S*( \S+)* ?[^ ,-])?$"#).unwrap()
    });

    static UNDER: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"\.[a-z]|\. [a-z]\b|\b[a-uw-z]\.").unwrap());

    static OVER: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"( |^)[a-z] [a-z]( |$)| -|- ").unwrap());

    static BAD: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"[\p{P}\p{S}]").unwrap());

    static ALPHA: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"^\p{L}*$").unwrap());

    static DIGIT: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"^\p{Nd}$").unwrap());
    if ALPHA.is_match(value) {
        return true;
    }
    if !VALID.is_match(value) || UNDER.is_match(value) || OVER.is_match(value) {
        return false;
    }
    for c in value.chars() {
        if c == ' ' {
            continue;
        }
        let digit = DIGIT.is_match(&c.to_string()) || ['²', '³', '¹'].contains(&c);
        if digit {
            if last && value.contains(" 3rd") {
                continue;
            }
            return false;
        }
        if !"'’.,‘\"“”„-–—&/()".contains(c) && BAD.is_match(&c.to_string()) {
            return false;
        }
    }
    true
}

impl Name {
    /// Decode LaTeX in each name component without modifying the source value.
    pub fn latex_normalize(&self) -> Result<Self, String> {
        Ok(Self {
            first: self
                .first
                .as_deref()
                .map(|s| crate::text::RichText::from_latex_maybe(s, true)?.as_text())
                .transpose()?,
            last: crate::text::RichText::from_latex_maybe(&self.last, true)?.as_text()?,
            script: self.script.clone(),
            extensions: self.extensions.clone(),
        })
    }
}
