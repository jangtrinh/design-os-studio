//! Errors. Every message says what is wrong and, where the caller can act on it, the fix.

use crate::hash::ContentHash;

/// A manifest problem: the JSON path of the offending field, what is wrong, and how to repair it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManifestError {
    pub field: String,
    pub problem: String,
    pub fix: String,
}

impl ManifestError {
    pub(crate) fn error(field: impl Into<String>, problem: impl Into<String>, fix: impl Into<String>) -> DocsError {
        DocsError::Manifest(Self { field: field.into(), problem: problem.into(), fix: fix.into() })
    }
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "docs-manifest.json: `{}` {}; fix: {}", self.field, self.problem, self.fix)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DocsError {
    #[error("{path}: {source}")]
    Io { path: String, source: std::io::Error },

    #[error("{path} does not exist; fix: create it or refresh the page list")]
    NotFound { path: String },

    /// The file is not what the caller last read (or, for `Expect::Absent`, it already exists). Nothing was written.
    #[error("{}", conflict_message(.path, .expected, .found))]
    Conflict { path: String, expected: Option<ContentHash>, found: Option<ContentHash> },

    #[error("{0}")]
    Manifest(ManifestError),

    /// A page or manifest larger than the size the store reads. Nothing was read.
    #[error("{path} is over the {limit} byte limit; fix: split the page")]
    TooLarge { path: String, limit: u64 },

    /// The path leads, through a symlink, to a place outside the docs folder. Nothing was read or written.
    #[error("{path} resolves to a place outside the docs folder, so it was not touched; fix: remove the symlink, or replace it with a real file inside the docs folder")]
    OutsideDocs { path: String },

    /// A page or manifest with more than one hard link. A hard link to a file outside the docs folder looks like an
    /// ordinary file inside it, so the store cannot tell it from one and does not read it.
    #[error("{path} has more than one hard link, so it may be another name for a file outside the docs folder and was not read; fix: replace it with a copy of the file (a hard link cannot be told apart from the file it points to)")]
    HardLinked { path: String },

    /// The page was replaced (an atomic save) every time it was opened, so no read could be matched to one file. The page is
    /// fine; reading it again works. Nothing was read.
    #[error("{path} changed while it was read; fix: read it again")]
    ChangedDuringRead { path: String },

    #[error("block {index} does not exist; the page has {len} blocks; fix: read the page again and use an index below {len}")]
    BlockIndex { index: usize, len: usize },

    /// Another program holds the docs folder's write lock and did not let go in time. Nothing was written.
    #[error("{path} is locked by another program, so nothing was written; fix: close the other Studio window or agent that is writing this project's docs, then retry (the lock frees itself when that program exits)")]
    Busy { path: String },

    #[error("invalid path {path:?}: {problem}; fix: {fix}")]
    InvalidPath { path: String, problem: String, fix: String },
}

fn conflict_message(path: &str, expected: &Option<ContentHash>, found: &Option<ContentHash>) -> String {
    match (expected, found) {
        (None, Some(_)) => format!("{path} already exists, so it was not overwritten; fix: reload it and edit the existing page, or pick another name"),
        (Some(_), None) => format!("{path} was deleted since it was read, so it was not written; fix: recreate the page deliberately or drop the edit"),
        _ => format!("{path} changed on disk since it was read, so it was not overwritten; fix: reload the page and apply your edit to the new version"),
    }
}

impl DocsError {
    pub(crate) fn io(path: impl Into<String>, source: std::io::Error) -> Self {
        if source.kind() == std::io::ErrorKind::NotFound {
            return Self::NotFound { path: path.into() };
        }
        Self::Io { path: path.into(), source }
    }

    pub(crate) fn invalid_path(path: &str, problem: impl Into<String>) -> Self {
        Self::InvalidPath {
            path: path.to_string(),
            problem: problem.into(),
            fix: "use a relative Markdown path such as `spec/checkout.md` (no `..`, no leading `/` or `.`)".into(),
        }
    }
}

pub type Result<T> = std::result::Result<T, DocsError>;
