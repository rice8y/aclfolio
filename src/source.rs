//! Offline source manifests and immutable snapshot loading options.

use crate::{Anthology, error::*};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

/// Upstream revision used for the 0.1 compatibility contract.
pub const SUPPORTED_REVISION: &str = "e9b28ecee27696ac39bf02882b4c552488cee8fe";

/// SHA-256 of the authoritative schema at [`SUPPORTED_REVISION`].
pub const SUPPORTED_SCHEMA_SHA256: &str =
    "5992354d467a9a4bc5de6f9b8d58ea3308e6ef249b12e0ef5ea161ca741ec35c";

/// Handling of structural, extension and reference diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadPolicy {
    /// Return all diagnostics alongside usable records.
    Compatible,
    /// Reject snapshots containing any diagnostic.
    Strict,
}

/// Resource bounds, applied before allocating or recursing through inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// Maximum bytes in one XML or JSON file (default 64 MiB).
    pub file_bytes: u64,
    /// Maximum bytes across all sources (default 2 GiB).
    pub total_bytes: u64,
    /// Maximum source files (default 20,000).
    pub files: usize,
    /// XML nodes per collection (default 2,000,000).
    pub xml_nodes: u32,
    /// Maximum XML element nesting (default 64).
    pub xml_depth: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            file_bytes: 64 * 1024 * 1024,
            total_bytes: 2 * 1024 * 1024 * 1024,
            files: 20_000,
            xml_nodes: 2_000_000,
            xml_depth: 64,
        }
    }
}

/// Local loading configuration. No option enables network access.
#[derive(Debug, Clone)]
pub struct LoadOptions {
    /// URL templates used by effective metadata and file references.
    pub config: crate::config::Config,
    /// Default is compatible, retaining all diagnostics.
    pub policy: LoadPolicy,
    /// Caller-supplied commit identity. This is a claim, not Git verification.
    pub revision: Option<String>,
    /// Allocation and nesting limits.
    pub limits: Limits,
}

impl Default for LoadOptions {
    fn default() -> Self {
        Self {
            config: crate::config::Config::default(),
            policy: LoadPolicy::Compatible,
            revision: None,
            limits: Limits::default(),
        }
    }
}

impl LoadOptions {
    /// Load with these options.
    pub fn open(&self, path: impl AsRef<Path>) -> Result<Anthology, Error> {
        Anthology::load(path.as_ref(), self, None)
    }
}

/// Actual source content fingerprints and optional caller-provided revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// Caller-provided revision, never interpreted as a timestamp.
    pub declared_revision: Option<String>,
    /// Relative paths to lowercase SHA-256 content digests.
    pub files: BTreeMap<PathBuf, String>,
}

/// Inputs are regular files only; symlinks are rejected.
pub(crate) fn read(root: &Path, relative: &Path, limits: &Limits) -> Result<Vec<u8>, Error> {
    let path = root.join(relative);
    let metadata = fs::symlink_metadata(&path).map_err(|e| io(&path, e))?;
    if !metadata.is_file() {
        return Err(Error::Limit(format!(
            "{} is not a regular file",
            path.display()
        )));
    }
    if metadata.len() > limits.file_bytes {
        return Err(Error::Limit(format!(
            "{} exceeds file byte limit",
            path.display()
        )));
    }
    let mut bytes = Vec::new();
    fs::File::open(&path)
        .map_err(|e| io(&path, e))?
        .take(limits.file_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| io(&path, e))?;
    if bytes.len() as u64 > limits.file_bytes {
        return Err(Error::Limit(format!(
            "{} grew past file byte limit",
            path.display()
        )));
    }
    Ok(bytes)
}

pub(crate) fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn manifest(root: &Path, limits: &Limits) -> Result<BTreeMap<PathBuf, String>, Error> {
    let mut paths = Vec::new();
    for dir in ["xml", "json"] {
        let directory = root.join(dir);
        if !fs::symlink_metadata(&directory)
            .map_err(|e| io(&directory, e))?
            .is_dir()
        {
            return Err(Error::Limit(format!(
                "{} must be a real directory",
                directory.display()
            )));
        }
        for entry in fs::read_dir(&directory).map_err(|e| io(&directory, e))? {
            let entry = entry.map_err(|e| io(&directory, e))?;
            let name = entry.file_name();
            let p = Path::new(dir).join(name);
            if p.extension().is_some_and(|x| x == dir) || p == Path::new("xml/schema.rnc") {
                paths.push(p);
                if paths.len() > limits.files {
                    return Err(Error::Limit("source file count".into()));
                }
            }
        }
    }
    paths.sort();
    let mut total = 0u64;
    paths
        .into_iter()
        .map(|p| {
            let bytes = read(root, &p, limits)?;
            total = total.saturating_add(bytes.len() as u64);
            if total > limits.total_bytes {
                return Err(Error::Limit("total source bytes".into()));
            }
            Ok((p, hash(&bytes)))
        })
        .collect()
}
