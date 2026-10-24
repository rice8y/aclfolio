//! CSL syntax and locale resolution, independent of rendering state.

use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

#[derive(Clone, Debug, Default, Deserialize)]
pub(super) struct StyleNode {
    pub(super) tag: String,
    pub(super) attrs: BTreeMap<String, String>,
    pub(super) text: String,
    pub(super) children: Vec<StyleNode>,
}

impl StyleNode {
    pub(super) fn get(&self, key: &str) -> Option<&str> {
        self.attrs.get(key).map(String::as_str)
    }

    pub(super) fn attr<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        self.get(key).unwrap_or(default)
    }

    pub(super) fn child(&self, tag: &str) -> Option<&StyleNode> {
        self.children.iter().find(|node| node.tag == tag)
    }

    pub(super) fn parse(node: roxmltree::Node<'_, '_>) -> Self {
        Self {
            tag: node.tag_name().name().into(),
            attrs: node
                .attributes()
                .map(|attribute| (attribute.name().into(), attribute.value().into()))
                .collect(),
            text: node.text().unwrap_or("").into(),
            children: node
                .children()
                .filter(|node| node.is_element())
                .map(Self::parse)
                .collect(),
        }
    }
}

#[derive(Deserialize)]
struct Locales {
    primary: BTreeMap<String, String>,
    locales: BTreeMap<String, StyleNode>,
}

fn locales() -> &'static Locales {
    static DATA: OnceLock<Locales> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../../data/csl-locales.json"))
            .expect("bundled CSL locales")
    })
}

/// Inline locale overrides precede bundled language fallbacks, in CSL lookup
/// order.
pub(super) fn select_locales(root: &StyleNode) -> Result<Vec<&StyleNode>, String> {
    let output = root.attr("default-locale", "en-US");
    let language = output.split('-').next().unwrap_or("en");
    let mut selected = Vec::new();
    for lang in [Some(output), Some(language), None] {
        let matches = root
            .children
            .iter()
            .filter(|node| node.tag == "locale" && node.get("lang") == lang)
            .collect::<Vec<_>>();
        if matches.len() > 1 {
            return Err("multiple CSL locales with the same language".into());
        }
        selected.extend(matches);
    }
    let mut added = BTreeSet::new();
    for key in [
        Some(output),
        locales().primary.get(language).map(String::as_str),
        Some("en-US"),
    ]
    .into_iter()
    .flatten()
    {
        if added.insert(key) {
            if let Some(node) = locales().locales.get(key) {
                selected.push(node);
            }
        }
    }
    Ok(selected)
}
