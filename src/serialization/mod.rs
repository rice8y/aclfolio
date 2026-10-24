//! Detached XML conversions. These operations never write authoritative files.

use crate::UrlReference;
pub use crate::types::DeletionKind;
use crate::utils::{xml_attribute, xml_escape};

mod annotations;
mod events;
mod names;
mod publications;
mod validation;

pub(super) fn element(tag: &str, text: &str) -> String {
    if text.is_empty() {
        format!("<{tag}/>")
    } else {
        format!("<{tag}>{}</{tag}>", xml_escape(text))
    }
}

pub(super) fn optional(out: &mut String, tag: &str, value: Option<&str>) {
    if let Some(s) = value.filter(|s| !s.is_empty()) {
        out.push_str(&element(tag, s));
    }
}

pub(super) fn attr(out: &mut String, key: &str, value: Option<&str>) {
    if let Some(s) = value {
        out.push_str(&format!(" {key}=\"{}\"", xml_attribute(s)));
    }
}

pub(super) fn ref_xml(tag: &str, name: &str, hash: Option<&str>, kind: Option<&str>) -> String {
    let mut out = format!("<{tag}");
    if !name.contains("://") {
        attr(&mut out, "hash", hash);
    }
    attr(&mut out, "type", kind.filter(|s| !s.is_empty()));
    out.push('>');
    out.push_str(&xml_escape(name));
    out.push_str(&format!("</{tag}>"));
    out
}

pub(super) fn ordered_links<'a>(
    links: impl Iterator<Item = &'a UrlReference>,
) -> Vec<(&'a str, &'a UrlReference)> {
    let mut result: Vec<(&str, &UrlReference)> = vec![];
    for u in links {
        let kind = u.kind.as_deref().unwrap_or("attachment");
        if let Some(old) = result.iter_mut().find(|(k, _)| *k == kind) {
            old.1 = u;
        } else {
            result.push((kind, u));
        }
    }
    result
}
