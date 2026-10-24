//! Structured loading errors and nonfatal compatibility diagnostics.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Distinguishes structural checks from cross-reference checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticKind {
    /// Unknown upstream element, attribute or JSON field.
    UnknownField,
    /// Structural or value constraint violation.
    Structure,
    /// A referenced directory or publication record is absent.
    Reference,
}

/// One actionable compatibility problem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Relative input path.
    pub file: PathBuf,
    /// Closest known entity identifier.
    pub entity: Option<String>,
    /// One-based line where available.
    pub line: Option<u32>,
    /// One-based column where available.
    pub column: Option<u32>,
    /// Validation layer.
    pub kind: DiagnosticKind,
    /// Human-readable description.
    pub message: String,
}

/// Errors never mean “record not found”; lookups use `Ok(None)` for that case.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Local I/O failed.
    #[error("{path}: {source}")]
    Io {
        /// Path that failed.
        path: PathBuf,
        /// Operating system error.
        #[source]
        source: std::io::Error,
    },
    /// Malformed XML/JSON, invalid source ID, duplicate key, or unsupported
    /// encoding.
    #[error("{file}:{line}:{column} ({entity:?}): {message}")]
    Parse {
        /// Source path.
        file: PathBuf,
        /// Closest known entity.
        entity: Option<String>,
        /// One-based line.
        line: u32,
        /// One-based column.
        column: u32,
        /// Parse failure details.
        message: String,
    },
    /// A strict load rejected one or more diagnostics.
    #[error("snapshot validation failed with {} diagnostic(s)", .0.len())]
    Validation(Vec<Diagnostic>),
    /// A caller resource limit was exceeded.
    #[error("resource limit: {0}")]
    Limit(String),
    /// Inputs changed during snapshot construction.
    #[error("source changed while loading: {0}")]
    SourceChanged(PathBuf),
}

pub(crate) fn io(path: &std::path::Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.into(),
        source,
    }
}

pub(crate) fn parse_error(file: &std::path::Path, message: impl ToString) -> Error {
    Error::Parse {
        file: file.into(),
        entity: None,
        line: 1,
        column: 1,
        message: message.to_string(),
    }
}

pub(crate) fn diagnostic(
    file: impl Into<PathBuf>,
    entity: Option<String>,
    kind: DiagnosticKind,
    message: String,
) -> Diagnostic {
    Diagnostic {
        file: file.into(),
        entity,
        line: None,
        column: None,
        kind,
        message,
    }
}
