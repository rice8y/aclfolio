//! Directory JSON decoding rejects duplicate keys before deserialization loses
//! them.

use crate::{
    Error,
    source::{LoadOptions, hash, read},
};
use serde::{
    Deserialize,
    de::{self, MapAccess, Visitor},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

// Detect duplicate JSON keys recursively before serde can silently overwrite
// them.
struct UniqueValue;

impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(UniqueKeysVisitor)
    }
}

struct UniqueKeysVisitor;

impl<'de> Visitor<'de> for UniqueKeysVisitor {
    type Value = UniqueValue;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("JSON with unique object keys")
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<UniqueValue, M::Error> {
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(de::Error::custom(format!("duplicate key {key}")));
            }
            map.next_value::<UniqueValue>()?;
        }
        Ok(UniqueValue)
    }

    fn visit_seq<A: de::SeqAccess<'de>>(self, mut sequence: A) -> Result<UniqueValue, A::Error> {
        while sequence.next_element::<UniqueValue>()?.is_some() {}
        Ok(UniqueValue)
    }

    fn visit_bool<E: de::Error>(self, _: bool) -> Result<UniqueValue, E> {
        Ok(UniqueValue)
    }

    fn visit_i64<E: de::Error>(self, _: i64) -> Result<UniqueValue, E> {
        Ok(UniqueValue)
    }

    fn visit_u64<E: de::Error>(self, _: u64) -> Result<UniqueValue, E> {
        Ok(UniqueValue)
    }

    fn visit_f64<E: de::Error>(self, _: f64) -> Result<UniqueValue, E> {
        Ok(UniqueValue)
    }

    fn visit_str<E: de::Error>(self, _: &str) -> Result<UniqueValue, E> {
        Ok(UniqueValue)
    }

    fn visit_unit<E: de::Error>(self) -> Result<UniqueValue, E> {
        Ok(UniqueValue)
    }
}

pub(super) fn directory<T: serde::de::DeserializeOwned>(
    root: &Path,
    path: &str,
    options: &LoadOptions,
    manifest: &BTreeMap<PathBuf, String>,
) -> Result<BTreeMap<String, T>, Error> {
    let path = Path::new(path);
    let data = read(root, path, &options.limits)?;
    if manifest.get(path) != Some(&hash(&data)) {
        return Err(Error::SourceChanged(path.into()));
    }
    let error = |e: serde_json::Error| Error::Parse {
        file: path.into(),
        entity: None,
        line: e.line() as u32,
        column: e.column() as u32,
        message: e.to_string(),
    };
    serde_json::from_slice::<UniqueValue>(&data).map_err(error)?;
    serde_json::from_slice(&data).map_err(error)
}
