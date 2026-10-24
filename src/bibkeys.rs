//! Pure citation-key generation and the pinned English stop-word list.

use crate::{Anthology, PaperRef};
use std::{collections::BTreeSet, sync::OnceLock};

/// Maximum author names before using `etal` in a generated key.
pub const BIBKEY_MAX_NAMES: usize = 2;

/// Stop words used by the reference library's citation-key generator.
pub fn stopwords() -> &'static BTreeSet<String> {
    static WORDS: OnceLock<BTreeSet<String>> = OnceLock::new();
    WORDS.get_or_init(|| {
        serde_json::from_str(include_str!("../data/stopwords.json")).expect("bundled stopwords")
    })
}

/// Case-sensitive lookup of an already slugified word.
pub fn is_stopword(word: &str) -> bool {
    stopwords().contains(word)
}

/// Stop words for volume titles, including conference words and ordinal
/// expressions.
pub fn is_volume_stopword(word: &str) -> bool {
    const VOLUME_WORDS: &[&str] = &[
        "proceedings",
        "volume",
        "conference",
        "workshop",
        "annual",
        "meeting",
        "computational",
    ];

    static ORDINAL: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(concat!(
            r"^(?:[0-9]+(?:st|nd|rd|th)|[a-z]+(?:ieth|enth)|",
            r"first|second|third|fourth|fifth|sixth|eighth|ninth|twelfth)$"
        ))
        .expect("volume-title ordinal pattern")
    });
    is_stopword(word) || VOLUME_WORDS.contains(&word) || ORDINAL.is_match(word)
}

impl Anthology {
    /// Generate an unused key without modifying the paper or reserving the key.
    pub fn generate_bibkey(&self, paper: PaperRef<'_>) -> Result<String, String> {
        let mut words = std::collections::VecDeque::new();
        let mut key = if paper.is_frontmatter() {
            format!(
                "{}-{}-{}",
                crate::names::slugify(&paper.volume_ref().venue_acronym()?, false),
                paper.year(),
                paper.volume_ref().local_id()
            )
        } else {
            let specs = if paper.authors.is_empty() {
                paper.editors()
            } else {
                &paper.stored().authors
            };
            let names = if specs.is_empty() {
                "nn".into()
            } else if specs.len() > BIBKEY_MAX_NAMES {
                format!("{}-etal", crate::names::slugify(&specs[0].name.last, false))
            } else {
                specs
                    .iter()
                    .map(|n| crate::names::slugify(&n.name.last, false))
                    .collect::<Vec<_>>()
                    .join("-")
            };
            let title = crate::names::slugify(&paper.title().as_text()?, false);
            words.extend(
                title
                    .split('-')
                    .filter(|w| !is_stopword(w))
                    .map(str::to_owned),
            );
            if words.is_empty() {
                words.extend(title.split('-').map(str::to_owned));
                words.push_back("0".into());
            }
            format!("{names}-{}-{}", paper.year(), words.pop_front().unwrap())
        };
        let index = self.bibkey_index()?;
        while index.contains_key(&key) {
            if let Some(word) = words.pop_front() {
                key.push('-');
                key.push_str(&word);
            } else if let Some((prefix, n)) = key
                .rsplit_once('-')
                .filter(|(_, s)| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            {
                key = format!("{prefix}-{}", crate::utils::increment_decimal(n));
            } else {
                key.push_str("-2");
            }
        }
        Ok(key)
    }

    /// Build the citation-key index once, rejecting duplicate keys.
    pub fn bibkey_index(
        &self,
    ) -> Result<&std::collections::BTreeMap<String, crate::PaperId>, String> {
        self.bibkeys
            .get_or_init(|| {
                let mut result = std::collections::BTreeMap::new();
                for p in self.papers() {
                    if result.insert(p.bibkey.clone(), p.id().clone()).is_some() {
                        return Err(format!("duplicate bibkey {}", p.bibkey));
                    }
                }
                Ok(result)
            })
            .as_ref()
            .map_err(Clone::clone)
    }
}
