//! Final text normalization shared by stored and detached markup projections.

use crate::{latex::latex_convert_quotes, utils::remove_extra_whitespace};
use regex::Regex;
use std::sync::LazyLock;

// Paragraph boundaries survive trimming by using a private-use sentinel.
// Restore
// them only after whitespace normalization, including at the beginning or end.
pub(crate) fn paragraphs(text: &str, replacement: &str) -> String {
    static PARAGRAPH: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"[\s\x1c-\x1f]*\x{e000}[\s\x1c-\x1f]*").expect("paragraph pattern")
    });
    PARAGRAPH
        .replace_all(text, regex::NoExpand(replacement))
        .into_owned()
}

pub(crate) fn html(text: &str) -> String {
    static BREAKS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"[\s\x1c-\x1f]*((?:<br/>)+)[\s\x1c-\x1f]*").expect("HTML break pattern")
    });
    BREAKS
        .replace_all(&remove_extra_whitespace(text), "$1")
        .into_owned()
}

pub(crate) fn latex(text: &str) -> String {
    static PARAGRAPH: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"[\s\x1c-\x1f]*\\par").expect("LaTeX paragraph pattern"));
    PARAGRAPH
        .replace_all(
            &remove_extra_whitespace(&latex_convert_quotes(text)),
            "\\par",
        )
        .into_owned()
}

/// Python string-protocol bounds count Unicode scalars, not UTF-8 bytes.
/// A start beyond the end remains invalid even for an empty prefix/suffix.
pub(crate) fn slice(text: &str, start: Option<isize>, end: Option<isize>) -> Option<String> {
    let chars: Vec<_> = text.chars().collect();
    let len = chars.len() as isize;
    let relative = |index: isize| {
        if index < 0 {
            (len + index).max(0)
        } else {
            index
        }
    };
    let start = relative(start.unwrap_or(0));
    let end = relative(end.unwrap_or(len)).min(len);
    (start <= end).then(|| chars[start as usize..end as usize].iter().collect())
}
