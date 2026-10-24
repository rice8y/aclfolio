//! Optional Git acquisition. Uses the Git executable, never Python.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

/// Repository synchronization failure.
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    /// Could not execute Git or access a directory.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Git returned an unsuccessful status.
    #[error("git failed: {0}")]
    Git(String),
    /// Metadata load failure.
    #[error(transparent)]
    Metadata(#[from] crate::Error),
}

fn run(command: &mut Command) -> Result<String, RepositoryError> {
    let output = command.output()?;
    if !output.status.success() {
        return Err(RepositoryError::Git(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().into())
}

/// Clone a shallow single-branch repository or pull its existing origin.
/// Returns an origin mismatch as a diagnostic, matching the reference's
/// non-fatal behavior.
/// Git controls merge/conflict behavior; no reset or metadata editing is
/// performed.
pub fn clone_or_pull_from_repo(
    url: &str,
    path: impl AsRef<Path>,
) -> Result<Option<String>, RepositoryError> {
    let path = path.as_ref();
    if path.join(".git").exists() {
        let origin = run(Command::new("git")
            .arg("-C")
            .arg(path)
            .args(["remote", "get-url", "origin"]))?;
        run(Command::new("git")
            .arg("-C")
            .arg(path)
            .args(["pull", "--force", "origin"]))?;
        Ok((origin != url).then(|| format!("origin {origin} does not match requested {url}")))
    } else {
        run(Command::new("git")
            .args(["clone", "--single-branch", "--depth=1", "--", url])
            .arg(path))?;
        Ok(None)
    }
}

/// Find the current repository's metadata directory.
pub fn data_directory_within_repo(start: impl AsRef<Path>) -> Result<PathBuf, RepositoryError> {
    let root = run(Command::new("git")
        .arg("-C")
        .arg(start.as_ref())
        .args(["rev-parse", "--show-toplevel"]))?;
    Ok(PathBuf::from(root).join("data"))
}

impl crate::Anthology {
    /// Acquire/update a repository at an explicit location, then load its metadata.
    pub fn from_repo(
        url: &str,
        path: impl AsRef<Path>,
    ) -> Result<(Self, Option<String>), RepositoryError> {
        let warning = clone_or_pull_from_repo(url, &path)?;
        Ok((Self::open(path.as_ref().join("data"))?, warning))
    }

    /// Load metadata from the Git repository containing the supplied directory.
    pub fn from_within_repo(start: impl AsRef<Path>) -> Result<Self, RepositoryError> {
        Ok(Self::open(data_directory_within_repo(start)?)?)
    }
}
