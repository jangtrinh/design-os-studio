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

    #[error("front matter: `{field}` {problem}; fix: {fix}")]
    FrontMatter { field: String, problem: String, fix: String },

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
