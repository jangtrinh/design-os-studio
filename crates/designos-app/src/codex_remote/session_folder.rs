//! The tab's session folder, as a command request may name it (#930).
//!
//! Codex lets the model choose the folder a command runs in, and the request carries it as `cwd`; the card shows only the
//! command. Studio owns the session folder, so a request that names exactly it can be read as "in the session". Any other
//! name does not count, a subfolder included: an agent can plant a symlink there. The request's own `cwd` is never
//! resolved, because a link behind it could be swapped after the approval.

use std::path::Path;

/// The names a request may use for the session folder: Studio's own path and its canonical form (`/var/...` and
/// `/private/var/...`), both fixed when the server starts.
#[derive(Clone, Debug)]
pub struct SessionFolder {
    names: Vec<String>,
}

impl SessionFolder {
    /// `path` as Studio holds it and as the file system resolves it right now. A name that is not UTF-8 is left out (no
    /// request can equal it).
    pub fn new(path: &Path) -> Self {
        let canonical = std::fs::canonicalize(path).ok();
        let names = [Some(path.to_path_buf()), canonical]
            .into_iter()
            .flatten()
            .filter_map(|p| p.to_str().map(|name| without_trailing_slash(name).to_string()))
            .collect();
        SessionFolder { names }
    }

    /// `cwd` is one of the names, a trailing slash ignored. A plain string comparison: nothing is resolved.
    pub fn is(&self, cwd: &str) -> bool {
        let cwd = without_trailing_slash(cwd);
        self.names.iter().any(|name| name == cwd)
    }
}

/// `path` without trailing slashes (the root stays `/`).
fn without_trailing_slash(path: &str) -> &str {
    match path.trim_end_matches('/') {
        "" if !path.is_empty() => "/",
        trimmed => trimmed,
    }
}
