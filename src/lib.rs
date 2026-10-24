//! Offline, unofficial, read-only access to authoritative ACL Anthology
//! metadata.
//!
//! Snapshots are eager, immutable, `Send + Sync`, and deterministic. See
//! [`LoadOptions`] for resource limits and strict/compatible validation. No
//! Git,
//! Python, database, or network connection is needed at runtime.
//!
//! ```no_run
//! use aclfolio::Anthology;
//! let corpus = Anthology::open("acl-anthology/data")?;
//! if let Some(paper) = corpus.paper("C92-1025")? {
//!     println!("{} ({})", paper.title().plain_text(), paper.year());
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub use error::{Diagnostic, DiagnosticKind, Error};
pub use ids::{AnthologyId, CollectionId, IdError, PaperId, VolumeId};
pub use markup::MarkupText;
pub use model::*;
pub use snapshot::{
    Anthology, EventAssociation, EventGroup, RecordChanges, ReloadStats, SnapshotDiff,
};
pub use source::{Limits, LoadOptions, LoadPolicy, Provenance};

pub mod bibkeys;
pub mod citation;
pub mod config;
pub mod credits;
mod csl;
pub mod directories;
pub mod error;
pub mod files;
pub mod ids;
pub mod json;
pub mod languages;
pub mod latex;
pub mod latex_parse;
pub mod markup;
pub mod math;
pub mod model;
mod names;
pub mod navigation;
mod parser;
pub mod people;
#[cfg(feature = "git")]
pub mod repository;
pub mod schema;
pub mod serialization;
mod snapshot;
pub mod source;
pub mod text;
pub mod types;
pub mod utils;
pub mod xml;
