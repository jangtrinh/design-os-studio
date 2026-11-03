//! Reading and writing a project's docs folder: `<library>/projects/<project-id>/docs/`.
//!
//! Every write states what it expects the file to be (`Expect`): the hash the caller read, or "absent" when
//! creating. A mismatch is a `Conflict` and nothing is written. The check and the write happen under an advisory
//! lock on `docs/.docs.lock`, so Studio and `designos-mcp` (both go through this crate) cannot interleave between
//! the check and the rename. A program that edits the Markdown without this crate takes no lock; the window it can
//! hit is the few microseconds between the final hash check and the rename.

use crate::error::{DocsError, Result};
use crate::hash::ContentHash;
use crate::manifest::{Manifest, MANIFEST_FILE};
use crate::model::Page;
use crate::paths::{page_path_problem, project_id_problem};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// A page or manifest larger than this is refused instead of read into memory.
pub(crate) const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;
const LOCK_TIMEOUT: Duration = Duration::from_secs(2);
const LOCK_FILE: &str = ".docs.lock";
const MAX_DEPTH: usize = 8;

/// What a write requires of the file currently on disk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expect {
    /// The file must not exist (creating a page).
    Absent,
    /// The file must have exactly this content (the hash from the read the edit is based on).
    Hash(ContentHash),
}

/// A value read from disk together with the hash of the bytes it came from.
#[derive(Clone, Debug, PartialEq)]
pub struct Versioned<T> {
    pub value: T,
    pub hash: ContentHash,
}

#[derive(Clone, Debug)]
pub struct DocsStore {
    dir: PathBuf,
    lock_timeout: Duration,
}

impl DocsStore {
    /// A store over an explicit docs folder (it need not exist yet).
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into(), lock_timeout: LOCK_TIMEOUT }
    }

    /// How long a write waits for another program's hold on `.docs.lock` before failing with `Busy`.
    pub fn with_lock_timeout(mut self, timeout: Duration) -> Self {
        self.lock_timeout = timeout;
        self
    }

    /// The docs folder of a library project: `<library_root>/projects/<project_id>/docs`. This is the layout
    /// `designos-library` gives a project folder; a test in this crate keeps the two in step.
    pub fn for_project(library_root: &Path, project_id: &str) -> Result<Self> {
        if let Some(problem) = project_id_problem(project_id) {
            return Err(DocsError::InvalidPath { path: project_id.to_string(), problem, fix: "pass the project id from the library, not a path".into() });
        }
        Ok(Self::at(library_root.join("projects").join(project_id).join("docs")))
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn read_page(&self, rel: &str) -> Result<Versioned<Page>> {
        let (text, hash) = self.read_text(&self.page_path(rel)?, rel)?;
        Ok(Versioned { value: Page::parse(&text), hash })
    }

    /// Writes `page` (as `Page::render` gives it). Returns the hash of the bytes now on disk; identical content is
    /// not rewritten, so it raises no change event.
    pub fn write_page(&self, rel: &str, page: &Page, expect: Expect) -> Result<ContentHash> {
        self.write_bytes(&self.page_path(rel)?, rel, page.render().as_bytes(), expect)
    }

    pub fn read_manifest(&self) -> Result<Versioned<Manifest>> {
        let (text, hash) = self.read_text(&self.dir.join(MANIFEST_FILE), MANIFEST_FILE)?;
        Ok(Versioned { value: Manifest::parse(&text)?, hash })
    }

    /// Validates the manifest, then writes it like a page.
    pub fn write_manifest(&self, manifest: &Manifest, expect: Expect) -> Result<ContentHash> {
        let json = manifest.to_json_pretty()?;
        self.write_bytes(&self.dir.join(MANIFEST_FILE), MANIFEST_FILE, json.as_bytes(), expect)
    }

    /// Every `.md` file under the docs folder (relative, `/`-separated, sorted); empty when the folder is missing.
    /// Hidden names and temp files are skipped.
    pub fn list_page_files(&self) -> Result<Vec<String>> {
        let mut out = Vec::new();
        collect_pages(&self.dir, "", 0, &mut out)?;
        out.sort();
        Ok(out)
    }

    fn page_path(&self, rel: &str) -> Result<PathBuf> {
        match page_path_problem(rel) {
            Some(problem) => Err(DocsError::invalid_path(rel, problem)),
            None => Ok(self.dir.join(rel)),
        }
    }

    fn read_text(&self, path: &Path, shown: &str) -> Result<(String, ContentHash)> {
        let bytes = read_limited(path, shown)?;
        let hash = ContentHash::of(&bytes);
        let text = String::from_utf8(bytes)
            .map_err(|_| DocsError::io(shown, std::io::Error::new(std::io::ErrorKind::InvalidData, "is not UTF-8 text; fix: re-save it as UTF-8 Markdown")))?;
        Ok((text, hash))
    }

    fn write_bytes(&self, path: &Path, shown: &str, bytes: &[u8], expect: Expect) -> Result<ContentHash> {
        std::fs::create_dir_all(&self.dir).map_err(|e| DocsError::io(self.dir.display().to_string(), e))?;
        let _lock = DirLock::acquire(&self.dir, self.lock_timeout)?;
        let found = match read_limited(path, shown) {
            Ok(current) => Some(current),
            Err(DocsError::NotFound { .. }) => None,
            Err(e) => return Err(e),
        };
        let found_hash = found.as_deref().map(ContentHash::of);
        let new_hash = ContentHash::of(bytes);
        let expected = match expect {
            Expect::Absent => None,
            Expect::Hash(h) => Some(h),
        };
        if found_hash != expected {
            return Err(DocsError::Conflict { path: shown.to_string(), expected, found: found_hash });
        }
        if found_hash == Some(new_hash) {
            return Ok(new_hash);
        }
        designos_fonts::write_atomic(path, bytes).map_err(|e| DocsError::io(shown, e))?;
        Ok(new_hash)
    }
}

pub(crate) fn read_limited(path: &Path, shown: &str) -> Result<Vec<u8>> {
    let meta = std::fs::metadata(path).map_err(|e| DocsError::io(shown, e))?;
    if !meta.is_file() {
        return Err(DocsError::io(shown, std::io::Error::new(std::io::ErrorKind::InvalidInput, "is not a file")));
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(DocsError::io(
            shown,
            std::io::Error::new(std::io::ErrorKind::InvalidData, format!("is {} bytes, over the {MAX_FILE_BYTES} byte limit; fix: split the page", meta.len())),
        ));
    }
    std::fs::read(path).map_err(|e| DocsError::io(shown, e))
}

fn collect_pages(dir: &Path, prefix: &str, depth: usize, out: &mut Vec<String>) -> Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(DocsError::io(dir.display().to_string(), e)),
    };
    for entry in entries {
        let entry = entry.map_err(|e| DocsError::io(dir.display().to_string(), e))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let rel = format!("{prefix}{name}");
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() && depth < MAX_DEPTH {
            collect_pages(&entry.path(), &format!("{rel}/"), depth + 1, out)?;
        } else if kind.is_file() && page_path_problem(&rel).is_none() {
            out.push(rel);
        }
    }
    Ok(())
}

/// Exclusive advisory lock on the docs folder's lock file, released on drop.
struct DirLock(File);

impl DirLock {
    fn acquire(dir: &Path, timeout: Duration) -> Result<Self> {
        let path = dir.join(LOCK_FILE);
        let shown = path.display().to_string();
        let file = File::options().create(true).truncate(false).write(true).open(&path).map_err(|e| DocsError::io(&shown, e))?;
        let deadline = Instant::now() + timeout;
        loop {
            match file.try_lock() {
                Ok(()) => break,
                // A filesystem without locks still gets the hash check, just not the cross-process exclusion.
                Err(std::fs::TryLockError::Error(e)) if e.kind() == std::io::ErrorKind::Unsupported => break,
                Err(std::fs::TryLockError::Error(e)) => return Err(DocsError::io(&shown, e)),
                Err(std::fs::TryLockError::WouldBlock) if Instant::now() >= deadline => return Err(DocsError::Busy { path: shown }),
                Err(std::fs::TryLockError::WouldBlock) => std::thread::sleep(Duration::from_millis(10)),
            }
        }
        Ok(Self(file))
    }
}

impl Drop for DirLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}
