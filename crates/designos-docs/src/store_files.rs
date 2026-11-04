//! File moves and manifest edits on a [`DocsStore`]. Like a write, each takes the folder's advisory lock and checks
//! the file's hash while holding it, so a page cannot be moved, removed or renamed out from under another writer.

use crate::error::{DocsError, Result};
use crate::hash::ContentHash;
use crate::manifest::Manifest;
use crate::store::{DirLock, DocsStore, Expect};
use std::path::Path;

/// How often a manifest edit starts again from a fresh read before it gives up.
const MANIFEST_ATTEMPTS: usize = 3;

impl DocsStore {
    /// Runs `edit` on the manifest as it is on disk now (an empty one when there is none) and writes it back against
    /// that read. When another writer got in between, `edit` runs again on the fresh manifest, so an edit is always
    /// a change to the current state and never a stale full copy.
    pub fn update_manifest(&self, mut edit: impl FnMut(&mut Manifest)) -> Result<()> {
        let mut last = None;
        for _ in 0..MANIFEST_ATTEMPTS {
            let (mut manifest, expect) = match self.read_manifest() {
                Ok(v) => (v.value, Expect::Hash(v.hash)),
                Err(DocsError::NotFound { .. }) => (Manifest::new(), Expect::Absent),
                Err(e) => return Err(e),
            };
            edit(&mut manifest);
            match self.write_manifest(&manifest, expect) {
                Ok(_) => return Ok(()),
                Err(e @ DocsError::Conflict { .. }) => last = Some(e),
                Err(e) => return Err(e),
            }
        }
        Err(last.expect("at least one attempt ran"))
    }

    /// Moves the page `rel` to `dest` (a path anywhere on the same volume, for example a trash folder) if it still
    /// has the content `expect`.
    pub fn move_page_out(&self, rel: &str, dest: &Path, expect: ContentHash) -> Result<()> {
        let path = self.checked_page_path(rel)?;
        let _lock = self.lock()?;
        check_hash(self, &path, rel, expect)?;
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| DocsError::io(parent.display().to_string(), e))?;
        }
        std::fs::rename(&path, dest).map_err(|e| DocsError::io(rel, e))
    }

    /// Deletes the page `rel` if it still has the content `expect`.
    pub fn remove_page(&self, rel: &str, expect: ContentHash) -> Result<()> {
        let path = self.checked_page_path(rel)?;
        let _lock = self.lock()?;
        check_hash(self, &path, rel, expect)?;
        std::fs::remove_file(&path).map_err(|e| DocsError::io(rel, e))
    }

    /// Renames the page `from` to `to` if it still has the content `expect`, and never replaces another file. A
    /// rename that only changes the case of the name works on a case-insensitive file system too, where the two
    /// names are one file: it goes through a temporary name.
    pub fn rename_page(&self, from: &str, to: &str, expect: ContentHash) -> Result<()> {
        let src = self.checked_page_path(from)?;
        let dst = self.checked_page_path(to)?;
        let _lock = self.lock()?;
        check_hash(self, &src, from, expect)?;
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(|e| DocsError::io(parent.display().to_string(), e))?;
        }
        if dst.symlink_metadata().is_ok() {
            if !(from.eq_ignore_ascii_case(to) && same_file(&src, &dst)) {
                return Err(DocsError::Conflict { path: to.to_string(), expected: None, found: Some(ContentHash::of(b"")) });
            }
            let temp = src.with_file_name(format!(".rename-{}.tmp", std::process::id()));
            std::fs::rename(&src, &temp).map_err(|e| DocsError::io(from, e))?;
            return std::fs::rename(&temp, &dst).map_err(|e| DocsError::io(to, e));
        }
        std::fs::rename(&src, &dst).map_err(|e| DocsError::io(from, e))
    }

    /// Moves the file `src` (on the same volume) into the docs folder as `rel`, and never replaces a page.
    pub fn adopt_file(&self, src: &Path, rel: &str) -> Result<()> {
        let dst = self.checked_page_path(rel)?;
        let _lock = self.lock()?;
        if dst.symlink_metadata().is_ok() {
            return Err(DocsError::Conflict { path: rel.to_string(), expected: None, found: Some(ContentHash::of(b"")) });
        }
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(|e| DocsError::io(parent.display().to_string(), e))?;
        }
        std::fs::rename(src, &dst).map_err(|e| DocsError::io(rel, e))
    }

    fn lock(&self) -> Result<DirLock> {
        std::fs::create_dir_all(self.dir()).map_err(|e| DocsError::io(self.dir().display().to_string(), e))?;
        DirLock::acquire(self.dir(), self.lock_timeout())
    }
}

fn check_hash(store: &DocsStore, path: &Path, rel: &str, expect: ContentHash) -> Result<()> {
    let found = match store.read_confined(path, rel) {
        Ok(bytes) => Some(ContentHash::of(&bytes)),
        Err(DocsError::NotFound { .. }) => None,
        Err(e) => return Err(e),
    };
    if found != Some(expect) {
        return Err(DocsError::Conflict { path: rel.to_string(), expected: Some(expect), found });
    }
    Ok(())
}

#[cfg(unix)]
fn same_file(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (std::fs::metadata(a), std::fs::metadata(b)) {
        (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
        _ => false,
    }
}

/// Windows and others: two names that differ only in case are taken as one file (their file systems fold case).
#[cfg(not(unix))]
fn same_file(_: &Path, _: &Path) -> bool {
    true
}
