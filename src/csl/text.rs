//! Segmented bibliography output and inline formatting.

use super::style::StyleNode;

// The reference processor distinguishes a missing name from the literal "None"
// until the final link/formatting pass. Keep that marker inside the renderer.
pub(super) const MISSING_NAME: &str = "\0None\0";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Boundaries {
    #[default]
    Merge,
    Preserve,
}

/// Segment boundaries determine whether a title can be linked as a whole.
/// Styling flattens its input; concatenating variable output preserves
/// boundaries.
#[derive(Clone, Debug, Default)]
pub(super) struct Text {
    segments: Vec<String>,
    boundaries: Boundaries,
}

impl Text {
    pub(super) fn segment(text: impl Into<String>) -> Self {
        Self {
            segments: vec![text.into()],
            boundaries: Boundaries::Preserve,
        }
    }

    pub(super) fn plain(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            segments: if text.is_empty() { vec![] } else { vec![text] },
            boundaries: Boundaries::Merge,
        }
    }

    pub(super) fn string(&self) -> String {
        self.segments.concat()
    }

    pub(super) fn empty(&self) -> bool {
        self.segments.iter().all(String::is_empty)
    }

    pub(super) fn into_segments(self) -> impl Iterator<Item = String> {
        self.segments.into_iter()
    }

    pub(super) fn map_segments(mut self, map: impl Fn(String) -> String) -> Self {
        self.segments = self.segments.into_iter().map(map).collect();
        self
    }

    pub(super) fn append(&mut self, mut other: Self) {
        let last = self.segments.iter().rev().find_map(|s| s.chars().last());
        let first = other.segments.iter().find_map(|s| s.chars().next());
        // Adjacent affixes must not duplicate punctuation at their boundary.
        if last == first && last.is_some_and(|c| " .,;:!?".contains(c)) {
            if let Some(segment) = other.segments.iter_mut().find(|s| !s.is_empty()) {
                segment.remove(0);
            }
        }
        if self.boundaries == Boundaries::Merge && other.boundaries == Boundaries::Merge {
            self.segments = vec![self.string() + &other.string()];
        } else {
            self.boundaries = Boundaries::Preserve;
            self.segments
                .extend(other.segments.into_iter().filter(|s| !s.is_empty()));
        }
    }

    pub(super) fn join(items: Vec<Self>, delimiter: &str) -> Option<Self> {
        let mut items = items.into_iter();
        let mut out = items.next()?;
        for item in items {
            out.append(Self::plain(delimiter));
            out.append(item);
        }
        Some(out)
    }
}

pub(super) fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub(super) fn wrap(node: &StyleNode, text: Text) -> Text {
    let mut output = Text::plain(node.attr("prefix", ""));
    output.append(text);
    output.append(Text::plain(node.attr("suffix", "")));
    output
}

pub(super) fn format(node: &StyleNode, mut text: Text) -> Text {
    for (key, val, tag) in [
        ("font-style", "italic", "i"),
        ("font-style", "oblique", "i"),
        (
            "font-variant",
            "small-caps",
            "span style=\"font-variant:small-caps;\"",
        ),
        ("font-weight", "bold", "b"),
        ("font-weight", "light", "l"),
        ("text-decoration", "underline", "u"),
        ("vertical-align", "sup", "sup"),
        ("vertical-align", "sub", "sub"),
    ] {
        if node.get(key) == Some(val) {
            let end = tag.split_whitespace().next().unwrap();
            text = Text::plain(format!(
                "<{tag}>{}</{end}>",
                text.string().replace(MISSING_NAME, "None")
            ));
        }
    }
    text
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

pub(super) fn case(node: &StyleNode, text: Text, language: &str) -> Text {
    let Some(mode) = node.get("text-case") else {
        return text;
    };
    if mode == "title" && language != "en" {
        return text;
    }
    let content = text.string();
    match mode {
        "lowercase" => text.map_segments(|content| content.to_lowercase()),
        "uppercase" => text.map_segments(|content| content.to_uppercase()),
        "capitalize-first" => {
            let mut text = text;
            if let Some(content) = text.segments.first_mut() {
                *content = capitalize(content);
            }
            text
        }
        "capitalize-all" => Text::plain(
            content
                .split_whitespace()
                .map(capitalize)
                .collect::<Vec<_>>()
                .join(" "),
        ),
        "title" | "sentence" => {
            let stop_words = [
                "a", "an", "and", "as", "at", "but", "by", "down", "for", "from", "in", "into",
                "nor", "of", "on", "onto", "or", "over", "so", "the", "till", "to", "up", "via",
                "with", "yet",
            ];
            let mut previous = ':';
            let all_uppercase = content
                .chars()
                .filter(|c| c.is_alphabetic())
                .all(|c| c.is_uppercase());
            Text::plain(
                content
                    .split_whitespace()
                    .enumerate()
                    .map(|(index, word)| {
                        let all_lowercase = word
                            .chars()
                            .filter(|c| c.is_alphabetic())
                            .all(|c| c.is_lowercase());
                        let result = if mode == "title" {
                            if all_lowercase
                                && (!stop_words.contains(&word) || ":.".contains(previous))
                            {
                                capitalize(word)
                            } else {
                                word.to_owned()
                            }
                        } else {
                            let mut word = if !all_uppercase
                                && !word
                                    .chars()
                                    .filter(|c| c.is_alphabetic())
                                    .all(|c| c.is_uppercase())
                            {
                                word.to_lowercase()
                            } else {
                                word.into()
                            };
                            if index == 0 {
                                word = capitalize(&word);
                            }
                            word
                        };
                        previous = word.chars().last().unwrap_or(' ');
                        result
                    })
                    .collect::<Vec<_>>()
                    .join(" "),
            )
        }
        _ => text,
    }
}
