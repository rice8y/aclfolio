//! Validated canonical identifiers. Syntax validity does not imply corpus
//! membership.

use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

/// An identifier that does not match the Anthology's canonical syntax.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid Anthology identifier: {0}")]
pub struct IdError(pub String);

fn digits(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|c| c.is_ascii_digit())
}

fn word(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

fn collection(value: &str) -> bool {
    if let Some((year, venue)) = value.split_once('.') {
        year.len() == 4 && digits(year) && word(venue)
    } else {
        value.len() == 3 && value.as_bytes()[0].is_ascii_uppercase() && digits(&value[1..])
    }
}

fn width(collection: &str, volume: &str) -> usize {
    if collection.starts_with('W')
        || collection == "C69"
        || (collection == "D19" && volume.len() >= 2 && &volume[..2] >= "50")
    {
        2
    } else {
        1
    }
}

fn volume(value: &str) -> bool {
    let Some((collection_id, volume_id)) = value.split_once('-') else {
        return false;
    };
    collection(collection_id)
        && if collection_id.contains('.') {
            word(volume_id)
        } else {
            digits(volume_id)
                && (volume_id.len() == width(collection_id, volume_id)
                    || (collection_id == "D19" && volume_id.len() == 2))
        }
}

fn paper(value: &str) -> bool {
    if value.contains('.') && value.split('.').count() >= 3 {
        let Some((volume_id, paper_id)) = value.rsplit_once('.') else {
            return false;
        };
        volume(volume_id) && word(paper_id)
    } else if let Some((collection_id, paper_id)) = value.split_once('-') {
        collection(collection_id)
            && !collection_id.contains('.')
            && digits(paper_id)
            && paper_id.len() >= 4
    } else {
        false
    }
}

macro_rules! id {
    ($name:ident, $validate:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            /// Parse a canonical identifier.
            pub fn new(value: impl Into<String>) -> Result<Self, IdError> {
                let value = value.into();
                if $validate(&value) {
                    Ok(Self(value))
                } else {
                    Err(IdError(value))
                }
            }

            /// Borrow the canonical spelling.
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Canonical landing page, with no implication that the record exists.
            pub fn canonical_url(&self) -> String {
                format!("https://aclanthology.org/{}/", self.0)
            }
        }

        impl FromStr for $name {
            type Err = IdError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::new(s)
            }
        }

        impl TryFrom<String> for $name {
            type Error = IdError;

            fn try_from(s: String) -> Result<Self, Self::Error> {
                Self::new(s)
            }
        }

        impl From<$name> for String {
            fn from(id: $name) -> String {
                id.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}
id!(
    CollectionId,
    collection,
    "A collection ID, e.g. `2022.acl` or `P18`."
);
id!(
    VolumeId,
    volume,
    "A volume ID, e.g. `2022.acl-long`, `W18-63`, or `D19-50`."
);
id!(
    PaperId,
    paper,
    "A paper or front matter ID, e.g. `2022.acl-long.42` or `P18-1001`."
);

impl VolumeId {
    /// Parent collection.
    pub fn collection_id(&self) -> CollectionId {
        CollectionId(
            self.0
                .split_once('-')
                .expect("validated volume")
                .0
                .to_owned(),
        )
    }

    /// Construct a paper ID from an unpadded local number.
    pub fn paper_id(&self, number: &str) -> Result<PaperId, IdError> {
        if !word(number) || (!self.0.contains('.') && !digits(number)) {
            return Err(IdError(number.into()));
        }
        if self.0.contains('.') {
            PaperId::new(format!("{}.{number}", self.0))
        } else {
            let c = self.collection_id();
            let v = self.0.split_once('-').expect("validated volume").1;
            let n = 4 - width(c.as_str(), v);
            PaperId::new(format!("{}{number:0>n$}", self.0))
        }
    }
}

impl PaperId {
    /// Parent volume, including C69 and D19 workshop exceptions.
    pub fn volume_id(&self) -> VolumeId {
        if self.0.contains('.') {
            VolumeId(self.0.rsplit_once('.').expect("validated paper").0.into())
        } else {
            let (c, n) = self.0.split_once('-').expect("validated paper");
            VolumeId(format!("{c}-{}", &n[..width(c, n)]))
        }
    }

    /// Parent collection.
    pub fn collection_id(&self) -> CollectionId {
        self.volume_id().collection_id()
    }

    /// Whether this ID denotes front matter.
    pub fn is_frontmatter(&self) -> bool {
        if self.0.contains('.') {
            self.0.ends_with(".0")
        } else {
            self.0[self.volume_id().as_str().len()..]
                .bytes()
                .all(|b| b == b'0')
        }
    }
}

/// Any syntactically valid Anthology collection, volume or paper identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AnthologyId {
    /// Collection identifier.
    Collection(CollectionId),
    /// Volume identifier.
    Volume(VolumeId),
    /// Paper or front matter identifier.
    Paper(PaperId),
}

impl FromStr for AnthologyId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if collection(s) {
            Ok(Self::Collection(s.parse()?))
        } else if volume(s) {
            Ok(Self::Volume(s.parse()?))
        } else {
            Ok(Self::Paper(s.parse()?))
        }
    }
}

impl fmt::Display for AnthologyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Collection(x) => x.fmt(f),
            Self::Volume(x) => x.fmt(f),
            Self::Paper(x) => x.fmt(f),
        }
    }
}

/// Components accepted by the Python-compatible, non-validating ID utilities.
/// Strong identifier types remain available when canonical syntax is required.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdParts(pub String, pub Option<String>, pub Option<String>);

/// Parse an identifier into collection, local volume and local paper
/// components.
/// Like Python, accepts unvalidated collection-only IDs and unpadded volume
/// IDs.
pub fn parse_id(id: &str) -> Result<IdParts, IdError> {
    let Some((collection, rest)) = id.split_once('-') else {
        return Ok(IdParts(id.into(), None, None));
    };
    if rest.contains('-') || collection.is_empty() {
        return Err(IdError(id.into()));
    }
    if collection.starts_with(|c: char| c.is_ascii_digit()) {
        let mut parts = rest.split('.');
        let v = parts.next().unwrap().into();
        let p = parts.next().map(str::to_owned);
        if parts.next().is_some() {
            return Err(IdError(id.into()));
        }
        return Ok(IdParts(collection.into(), Some(v), p));
    }
    if rest.chars().count() < 4 {
        return Ok(IdParts(
            collection.into(),
            Some(rest.trim_start_matches('0').into()),
            None,
        ));
    }
    let wide = collection.starts_with('W')
        || collection == "C69"
        || (collection == "D19"
            && rest
                .chars()
                .next()
                .and_then(|c| c.to_digit(10))
                .ok_or_else(|| IdError(id.into()))?
                >= 5);
    let boundary = rest
        .char_indices()
        .nth(if wide { 2 } else { 1 })
        .map(|(i, _)| i)
        .unwrap_or(rest.len());
    let (v, p) = rest.split_at(boundary);
    let p = p.trim_start_matches('0');
    Ok(IdParts(
        collection.into(),
        Some(if wide { v.trim_start_matches('0') } else { v }.into()),
        Some(if p.is_empty() { "0" } else { p }.into()),
    ))
}

/// Build an identifier using Python's historical minimum-width padding rules.
/// This intentionally performs no collection/item syntax validation.
pub fn build_id(
    collection: &str,
    volume: Option<&str>,
    paper: Option<&str>,
) -> Result<String, IdError> {
    let Some(volume) = volume else {
        return Ok(collection.into());
    };
    if collection.is_empty() {
        return Err(IdError(collection.into()));
    }
    if collection.starts_with(|c: char| c.is_ascii_digit()) {
        return Ok(match paper {
            Some(p) => format!("{collection}-{volume}.{p}"),
            None => format!("{collection}-{volume}"),
        });
    }
    let v: i128 = volume.trim().parse().map_err(|_| IdError(volume.into()))?;
    let wide =
        collection.starts_with('W') || collection == "C69" || (collection == "D19" && v >= 5);
    let mut result = format!("{collection}-{v:0width$}", width = if wide { 2 } else { 1 });
    if let Some(p) = paper {
        let p: i128 = p.trim().parse().map_err(|_| IdError(p.into()))?;
        result.push_str(&format!("{p:0width$}", width = if wide { 2 } else { 3 }));
    }
    Ok(result)
}

impl IdParts {
    /// Build a full identifier from these components.
    pub fn build(&self) -> Result<String, IdError> {
        build_id(&self.0, self.1.as_deref(), self.2.as_deref())
    }
}

/// Check the official collection-ID syntax.
pub fn is_valid_collection_id(id: &str) -> bool {
    collection(id)
}

/// Check a local volume or paper ID (lowercase ASCII letters and digits).
pub fn is_valid_item_id(id: &str) -> bool {
    word(id)
}

/// Check the official verified-person-ID syntax, including the unlink sentinel.
pub fn is_verified_person_id(id: &str) -> bool {
    id == crate::types::NO_PERSON_ID
        || (id.len() >= 2
            && id.as_bytes()[0].is_ascii_lowercase()
            && id
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-'))
}

/// Validate ORCID formatting and ISO 7064 checksum.
pub fn is_valid_orcid(id: &str) -> bool {
    if !orcid_shape(id) {
        return false;
    }
    let total = id[..18]
        .bytes()
        .filter(|&c| c != b'-')
        .fold(0u32, |t, c| (t + u32::from(c - b'0')) * 2);
    let check = (12 - total % 11) % 11;
    id.as_bytes()[18]
        == if check == 10 {
            b'X'
        } else {
            b'0' + check as u8
        }
}

fn orcid_shape(id: &str) -> bool {
    id.len() == 19
        && id.bytes().enumerate().all(|(i, c)| match i {
            4 | 9 | 14 => c == b'-',
            18 => c.is_ascii_digit() || c == b'X',
            _ => c.is_ascii_digit(),
        })
}

/// Uppercase and extract an ORCID-shaped substring (without checksum
/// validation).
pub fn convert_orcid(value: &str) -> String {
    let value = value.to_uppercase();
    if value.len() > 19 {
        for (i, _) in value.char_indices() {
            if let Some(id) = value.get(i..i + 19) {
                if orcid_shape(id) {
                    return id.into();
                }
            }
        }
    }
    value
}

/// Infer a publication year from an ID using the 1960/2060 historical boundary.
pub fn infer_year(id: &str) -> Result<String, IdError> {
    let IdParts(c, _, _) = parse_id(id)?;
    if c.starts_with(|c: char| c.is_ascii_digit()) {
        return Ok(c.split('.').next().unwrap().into());
    }
    let digits = c.get(1..).ok_or_else(|| IdError(id.into()))?;
    let n: u32 = digits.parse().map_err(|_| IdError(id.into()))?;
    Ok(format!("{}{digits}", if n >= 60 { "19" } else { "20" }))
}

/// Whether a citation key satisfies the reference constructor's complete regex
/// match.
pub fn is_valid_bibkey(value: &str) -> bool {
    value == crate::types::NO_BIBKEY
        || value.len() >= 2
            && value.as_bytes()[0].is_ascii_lowercase()
            && value
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Whether a venue identifier satisfies the reference format.
pub fn is_valid_venue_id(value: &str) -> bool {
    value.len() >= 2
        && value.as_bytes()[0].is_ascii_lowercase()
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

/// Whether an event identifier has an alphanumeric venue and four-digit year.
pub fn is_valid_event_id(value: &str) -> bool {
    value.rsplit_once('-').is_some_and(|(venue, year)| {
        !venue.is_empty()
            && venue
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
            && year.len() == 4
            && year.bytes().all(|b| b.is_ascii_digit())
    })
}
