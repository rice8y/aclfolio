//! Source acquisition, consistent-read checks and immutable snapshot
//! construction.

use super::{Anthology, Indices, ReloadStats, directory::directory};
use crate::{
    Collection, CollectionId, Diagnostic, DiagnosticKind, Error,
    error::{diagnostic, parse_error},
    parser,
    source::{
        LoadOptions, LoadPolicy, Provenance, SUPPORTED_REVISION, SUPPORTED_SCHEMA_SHA256, hash,
        manifest, read,
    },
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};

#[derive(Debug, Clone)]
pub(super) struct CollectionSource {
    pub(super) id: CollectionId,
    pub(super) diagnostics: Vec<Diagnostic>,
}

/// Records under construction, before publication indices make them navigable.
#[derive(Default)]
struct LoadedCollections {
    records: BTreeMap<CollectionId, Arc<Collection>>,
    sources: BTreeMap<PathBuf, CollectionSource>,
    diagnostics: Vec<Diagnostic>,
    stats: ReloadStats,
}

impl LoadedCollections {
    fn read(
        root: &Path,
        options: &LoadOptions,
        files: &BTreeMap<PathBuf, String>,
        previous: Option<&Anthology>,
    ) -> Result<Self, Error> {
        let mut loaded = Self::default();
        for (file, digest) in files {
            if file.extension().is_some_and(|e| e == "xml") {
                let reusable = previous
                    .filter(|snapshot| {
                        snapshot.limits == options.limits
                            && snapshot.provenance.files.get(file) == Some(digest)
                    })
                    .and_then(|snapshot| {
                        snapshot.sources.get(file).map(|source| {
                            (
                                snapshot.collections[&source.id].clone(),
                                source.diagnostics.clone(),
                            )
                        })
                    });
                let (collection, diagnostics) = if let Some(reused) = reusable {
                    loaded.stats.reused_collections += 1;
                    reused
                } else {
                    loaded.stats.parsed_collections += 1;
                    read_collection(root, file, digest, options)?
                };
                let id = collection.id.clone();
                loaded.diagnostics.extend(diagnostics.clone());
                if loaded.records.insert(id.clone(), collection).is_some() {
                    return Err(parse_error(file, format!("duplicate collection {id}")));
                }
                loaded
                    .sources
                    .insert(file.clone(), CollectionSource { id, diagnostics });
            } else if file.extension().is_some_and(|e| e == "json")
                && !matches!(
                    file.to_str(),
                    Some("json/people.json" | "json/venues.json" | "json/sigs.json")
                )
            {
                loaded.diagnostics.push(diagnostic(
                    file.clone(),
                    None,
                    DiagnosticKind::UnknownField,
                    "unknown JSON source file".into(),
                ));
            }
        }
        if files
            .get(Path::new("xml/schema.rnc"))
            .is_some_and(|schema| schema != SUPPORTED_SCHEMA_SHA256)
        {
            loaded.diagnostics.push(diagnostic(
                "xml/schema.rnc",
                None,
                DiagnosticKind::UnknownField,
                format!("schema fingerprint differs from supported revision {SUPPORTED_REVISION}"),
            ));
        }
        Ok(loaded)
    }
}

fn read_collection(
    root: &Path,
    file: &Path,
    digest: &str,
    options: &LoadOptions,
) -> Result<(Arc<Collection>, Vec<Diagnostic>), Error> {
    let data = read(root, file, &options.limits)?;
    if hash(&data) != digest {
        return Err(Error::SourceChanged(file.into()));
    }
    let input = std::str::from_utf8(&data).map_err(|e| parse_error(file, e))?;
    let (collection, diagnostics) = parser::parse(
        file,
        input,
        options.limits.xml_nodes,
        options.limits.xml_depth,
    )?;
    Ok((Arc::new(collection), diagnostics))
}

impl Anthology {
    /// Open local `data/` using compatible validation and default resource limits.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        LoadOptions::default().open(path)
    }

    /// Reconcile this snapshot with a new immutable directory. Failure leaves self
    /// intact.
    /// Every source is hashed; unchanged collections reuse their existing
    /// allocations.
    pub fn reload(&self, path: impl AsRef<Path>, options: &LoadOptions) -> Result<Self, Error> {
        Self::load(path.as_ref(), options, Some(self))
    }

    pub(crate) fn load(
        root: &Path,
        options: &LoadOptions,
        previous: Option<&Self>,
    ) -> Result<Self, Error> {
        let files = manifest(root, &options.limits)?;
        let people = directory(root, "json/people.json", options, &files)?;
        let venues = directory(root, "json/venues.json", options, &files)?;
        let sigs = directory(root, "json/sigs.json", options, &files)?;
        let loaded = LoadedCollections::read(root, options, &files, previous)?;
        let indices = Indices::build(&loaded.records, &loaded.sources, &people)?;
        let mut snapshot = Self {
            collections: loaded.records,
            indices,
            config: options.config.clone(),
            datadir: root.to_path_buf(),
            bibkeys: OnceLock::new(),
            identities: OnceLock::new(),
            people,
            venues,
            sigs,
            provenance: Provenance {
                declared_revision: options.revision.clone(),
                files,
            },
            diagnostics: loaded.diagnostics,
            sources: loaded.sources,
            stats: loaded.stats,
            limits: options.limits.clone(),
        };
        snapshot.diagnostics.extend(snapshot.validate());
        if options.policy == LoadPolicy::Strict && !snapshot.diagnostics.is_empty() {
            return Err(Error::Validation(snapshot.diagnostics));
        }
        // Recheck even reused files: a snapshot must describe one consistent source
        // tree.
        if manifest(root, &options.limits)? != snapshot.provenance.files {
            return Err(Error::SourceChanged(root.into()));
        }
        Ok(snapshot)
    }
}
