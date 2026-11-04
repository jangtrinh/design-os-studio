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
/// How many times a read starts over when the page is replaced while it is being opened.
const READ_RERUNS: usize = 3;

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
        let (text, hash) = self.read_text(&self.checked_page_path(rel)?, rel)?;
        Ok(Versioned { value: Page::parse(&text), hash })
    }

    /// Writes `page` (as `Page::render` gives it). Returns the hash of the bytes now on disk; identical content is
    /// not rewritten, so it raises no change event.
    pub fn write_page(&self, rel: &str, page: &Page, expect: Expect) -> Result<ContentHash> {
        self.write_bytes(&self.checked_page_path(rel)?, rel, page.render().as_bytes(), expect)
    }

    pub fn read_manifest(&self) -> Result<Versioned<Manifest>> {
        let path = self.dir.join(MANIFEST_FILE);
        self.confine(&path, MANIFEST_FILE)?;
        let (text, hash) = self.read_text(&path, MANIFEST_FILE)?;
        Ok(Versioned { value: Manifest::parse(&text)?, hash })
    }

    /// Validates the manifest, then writes it like a page.
    pub fn write_manifest(&self, manifest: &Manifest, expect: Expect) -> Result<ContentHash> {
        let json = manifest.to_json_pretty()?;
        let path = self.dir.join(MANIFEST_FILE);
        self.confine(&path, MANIFEST_FILE)?;
        self.write_bytes(&path, MANIFEST_FILE, json.as_bytes(), expect)
    }

    /// Every `.md` file under the docs folder (relative, `/`-separated, sorted); empty when the folder is missing.
    /// Hidden names and temp files are skipped.
    pub fn list_page_files(&self) -> Result<Vec<String>> {
        let mut out = Vec::new();
        collect_pages(&self.dir, "", 0, &mut out)?;
        out.sort();
        Ok(out)
    }

    pub(crate) fn lock_timeout(&self) -> Duration {
        self.lock_timeout
    }

    pub(crate) fn checked_page_path(&self, rel: &str) -> Result<PathBuf> {
        if let Some(problem) = page_path_problem(rel) {
            return Err(DocsError::invalid_path(rel, problem));
        }
        let path = self.dir.join(rel);
        self.confine(&path, rel)?;
        Ok(path)
    }

    /// Refuses a path that, after following every symlink, is outside the docs folder (`OutsideDocs`), and returns
    /// where it really is (`path` itself when it does not exist yet). The deepest part of `path` that exists decides;
    /// names below it are new and cannot point anywhere. A symlink whose target is inside the docs folder is fine.
    /// Without a docs folder there is nothing to escape into.
    fn confine(&self, path: &Path, shown: &str) -> Result<PathBuf> {
        let Ok(dir) = self.dir.canonicalize() else { return Ok(path.to_path_buf()) };
        let mut probe = path;
        let (real, exists) = loop {
            match probe.canonicalize() {
                Ok(real) => break (real, probe == path),
                Err(_) => match probe.parent() {
                    Some(parent) => probe = parent,
                    None => return Ok(path.to_path_buf()),
                },
            }
        };
        if !real.starts_with(&dir) {
            return Err(DocsError::OutsideDocs { path: shown.to_string() });
        }
        Ok(if exists { real } else { path.to_path_buf() })
    }

    /// Reads a page or the manifest. The file that is read is the one `confine` found, opened by its real path and
    /// then checked as an open handle: the name must still resolve to itself and to the same file (device and inode),
    /// so a link swapped in after the check is refused instead of followed. A file with more than one hard link is
    /// refused too, because a hard link to a file outside the folder cannot be told apart from an inside file.
    ///
    /// A save by rename (Studio's own, or an editor's) changes the inode under the name while the path stays inside the
    /// folder. That is a change, not an escape: the read starts over, up to `READ_RERUNS` times, and then fails with
    /// `ChangedDuringRead`, which is worth retrying. `OutsideDocs` is only for a path that really leads out.
    pub(crate) fn read_confined(&self, path: &Path, shown: &str) -> Result<Vec<u8>> {
        for _ in 0..=READ_RERUNS {
            let real = self.confine(path, shown)?;
            #[cfg(test)]
            tests::before_open();
            let file = File::open(&real).map_err(|e| DocsError::io(shown, e))?;
            #[cfg(test)]
            tests::after_open();
            let meta = file.metadata().map_err(|e| DocsError::io(shown, e))?;
            if !meta.is_file() {
                return Err(DocsError::io(shown, std::io::Error::new(std::io::ErrorKind::InvalidInput, "is not a file")));
            }
            if verify_handle(&self.dir, &real, &meta, shown)? == Handle::Changed {
                continue;
            }
            return match read_capped_file(file).map_err(|e| DocsError::io(shown, e))? {
                Some(bytes) => Ok(bytes),
                None => Err(DocsError::TooLarge { path: shown.to_string(), limit: MAX_FILE_BYTES }),
            };
        }
        Err(DocsError::ChangedDuringRead { path: shown.to_string() })
    }

    fn read_text(&self, path: &Path, shown: &str) -> Result<(String, ContentHash)> {
        let bytes = self.read_confined(path, shown)?;
        let hash = ContentHash::of(&bytes);
        let text = String::from_utf8(bytes)
            .map_err(|_| DocsError::io(shown, std::io::Error::new(std::io::ErrorKind::InvalidData, "is not UTF-8 text; fix: re-save it as UTF-8 Markdown")))?;
        Ok((text, hash))
    }

    fn write_bytes(&self, path: &Path, shown: &str, bytes: &[u8], expect: Expect) -> Result<ContentHash> {
        std::fs::create_dir_all(&self.dir).map_err(|e| DocsError::io(self.dir.display().to_string(), e))?;
        let _lock = DirLock::acquire(&self.dir, self.lock_timeout)?;
        let found = match self.read_confined(path, shown) {
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

/// What the name is now, compared with the file that was opened.
#[derive(Debug, PartialEq, Eq)]
enum Handle {
    /// The name still resolves to `real`, which is the open file.
    Same,
    /// The name moved or was replaced, but still lies inside the docs folder (an atomic save, a link to another page).
    Changed,
}

/// The open file is the file `real` names right now: `real` still resolves to itself (no link was put in its way), is the
/// same device and inode as the handle, and has no second name. A path that now leads outside the docs folder is an
/// `OutsideDocs` error; one that only changed inside it is `Handle::Changed`.
fn verify_handle(docs_dir: &Path, real: &Path, meta: &std::fs::Metadata, shown: &str) -> Result<Handle> {
    let outside = || DocsError::OutsideDocs { path: shown.to_string() };
    let again = match real.canonicalize() {
        Ok(again) => again,
        // Removed between the open and now (the middle of a replace): nothing to compare, so look again.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Handle::Changed),
        Err(e) => return Err(DocsError::io(shown, e)),
    };
    let dir = docs_dir.canonicalize().map_err(|e| DocsError::io(shown, e))?;
    if !again.starts_with(&dir) {
        return Err(outside());
    }
    if again != real {
        return Ok(Handle::Changed);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if meta.nlink() > 1 {
            return Err(DocsError::HardLinked { path: shown.to_string() });
        }
        let now = match std::fs::metadata(&again) {
            Ok(now) => now,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Handle::Changed),
            Err(e) => return Err(DocsError::io(shown, e)),
        };
        if (now.dev(), now.ino()) != (meta.dev(), meta.ino()) {
            return Ok(Handle::Changed);
        }
    }
    #[cfg(not(unix))]
    let _ = meta;
    Ok(Handle::Same)
}

/// The file's bytes, or `None` when it holds more than `MAX_FILE_BYTES`. The cap is applied while reading, so a file
/// that grows after it was measured (or was measured before it was fully written) is never read whole into memory.
pub(crate) fn read_capped(path: &Path) -> std::io::Result<Option<Vec<u8>>> {
    read_capped_file(File::open(path)?)
}

fn read_capped_file(file: File) -> std::io::Result<Option<Vec<u8>>> {
    use std::io::Read;
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes)?;
    Ok((bytes.len() as u64 <= MAX_FILE_BYTES).then_some(bytes))
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
pub(crate) struct DirLock(File);

impl DirLock {
    pub(crate) fn acquire(dir: &Path, timeout: Duration) -> Result<Self> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    type Hook = Box<dyn Fn()>;

    thread_local! {
        /// Runs between the confinement check and the open, where a concurrent process could swap a link.
        static BEFORE_OPEN: RefCell<Option<Hook>> = const { RefCell::new(None) };
    }

    pub(super) fn before_open() {
        BEFORE_OPEN.with(|hook| {
            if let Some(f) = hook.borrow().as_ref() {
                f();
            }
        });
    }

    thread_local! {
        /// Runs after the file is open and before it is checked: where a save by rename changes what the name points at.
        static AFTER_OPEN: RefCell<Option<Hook>> = const { RefCell::new(None) };
    }

    pub(super) fn after_open() {
        AFTER_OPEN.with(|hook| {
            if let Some(f) = hook.borrow().as_ref() {
                f();
            }
        });
    }

    #[test]
    fn the_read_cap_is_applied_while_reading_not_from_a_size_measured_earlier() {
        let tmp = tempfile::tempdir().unwrap();
        let exactly = tmp.path().join("exactly.md");
        std::fs::write(&exactly, vec![b'a'; MAX_FILE_BYTES as usize]).unwrap();
        assert_eq!(read_capped(&exactly).unwrap().map(|b| b.len() as u64), Some(MAX_FILE_BYTES));

        let over = tmp.path().join("over.md");
        std::fs::write(&over, vec![b'a'; MAX_FILE_BYTES as usize + 1]).unwrap();
        assert!(read_capped(&over).unwrap().is_none());
        let store = DocsStore::at(tmp.path());
        assert!(matches!(store.read_confined(&over, "over.md"), Err(DocsError::TooLarge { limit, .. }) if limit == MAX_FILE_BYTES));
    }

    #[cfg(unix)]
    mod swapped_between_check_and_open {
        use super::*;
        use std::os::unix::fs::symlink;

        fn docs() -> (tempfile::TempDir, DocsStore) {
            let tmp = tempfile::tempdir().unwrap();
            let dir = tmp.path().join("docs");
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("inside.md"), "# inside\n").unwrap();
            std::fs::write(tmp.path().join("secret.md"), "# SECRET\n").unwrap();
            (tmp, DocsStore::at(dir))
        }

        fn set_hook(f: impl Fn() + 'static) {
            BEFORE_OPEN.with(|h| *h.borrow_mut() = Some(Box::new(f)));
        }

        fn clear_hook() {
            BEFORE_OPEN.with(|h| *h.borrow_mut() = None);
        }

        #[test]
        fn a_link_name_swapped_to_an_outside_file_after_the_check_is_not_followed() {
            let (tmp, store) = docs();
            let (link, secret) = (store.dir().join("x.md"), tmp.path().join("secret.md"));
            symlink(store.dir().join("inside.md"), &link).unwrap();
            let swap = link.clone();
            set_hook(move || {
                std::fs::remove_file(&swap).unwrap();
                symlink(&secret, &swap).unwrap();
            });
            let read = store.read_page("x.md");
            clear_hook();
            let page = read.expect("the file the check saw (inside.md) is still what is read");
            assert_eq!(page.value.render(), "# inside\n", "the real path was opened, so the swapped name made no difference");
        }

        #[test]
        fn the_real_file_replaced_by_a_link_to_an_outside_file_after_the_check_is_refused() {
            let (tmp, store) = docs();
            let (real, secret) = (store.dir().join("inside.md"), tmp.path().join("secret.md"));
            set_hook(move || {
                std::fs::remove_file(&real).unwrap();
                symlink(&secret, &real).unwrap();
            });
            let read = store.read_page("inside.md");
            clear_hook();
            assert!(matches!(read, Err(DocsError::OutsideDocs { .. })), "{read:?}");
        }

        #[test]
        fn a_folder_swapped_for_a_link_after_the_check_is_refused() {
            let (tmp, store) = docs();
            std::fs::create_dir_all(store.dir().join("sub")).unwrap();
            std::fs::write(store.dir().join("sub/a.md"), "# a\n").unwrap();
            let outside = tmp.path().join("other");
            std::fs::create_dir_all(&outside).unwrap();
            std::fs::write(outside.join("a.md"), "# SECRET\n").unwrap();
            let sub = store.dir().join("sub");
            set_hook(move || {
                std::fs::rename(&sub, sub.with_file_name("sub-moved")).unwrap();
                symlink(&outside, &sub).unwrap();
            });
            let read = store.read_page("sub/a.md");
            clear_hook();
            assert!(matches!(read, Err(DocsError::OutsideDocs { .. })), "{read:?}");
        }
    }

    mod saved_by_rename_during_read {
        use super::*;
        use std::cell::Cell;
        use std::rc::Rc;

        fn docs() -> (tempfile::TempDir, DocsStore) {
            let tmp = tempfile::tempdir().unwrap();
            let dir = tmp.path().join("docs");
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("a.md"), "# old\n").unwrap();
            (tmp, DocsStore::at(dir))
        }

        /// Saves `a.md` the way Studio does: a temporary file, then a rename over the page.
        fn save_by_rename(dir: &Path, text: &str) {
            std::fs::write(dir.join(".a.tmp"), text).unwrap();
            std::fs::rename(dir.join(".a.tmp"), dir.join("a.md")).unwrap();
        }

        fn set_after(f: impl Fn() + 'static) {
            AFTER_OPEN.with(|h| *h.borrow_mut() = Some(Box::new(f)));
        }

        fn clear_after() {
            AFTER_OPEN.with(|h| *h.borrow_mut() = None);
        }

        #[test]
        fn a_save_by_rename_between_open_and_check_is_read_again_and_returns_the_new_page() {
            let (_tmp, store) = docs();
            let (dir, calls) = (store.dir().to_path_buf(), Rc::new(Cell::new(0)));
            let seen = calls.clone();
            set_after(move || {
                seen.set(seen.get() + 1);
                if seen.get() == 1 {
                    save_by_rename(&dir, "# new\n");
                }
            });
            let read = store.read_page("a.md");
            clear_after();
            let page = read.expect("a page replaced by an atomic save is not an escape");
            assert_eq!(page.value.render(), "# new\n", "the retry reads the file that is there now");
            assert_eq!(calls.get(), 2);
        }

        #[test]
        fn a_page_replaced_on_every_attempt_is_a_retryable_change_not_an_escape() {
            let (_tmp, store) = docs();
            let (dir, calls) = (store.dir().to_path_buf(), Rc::new(Cell::new(0)));
            let seen = calls.clone();
            set_after(move || {
                seen.set(seen.get() + 1);
                save_by_rename(&dir, &format!("# v{}\n", seen.get()));
            });
            let read = store.read_page("a.md");
            clear_after();
            let err = read.unwrap_err();
            assert!(matches!(err, DocsError::ChangedDuringRead { .. }), "{err:?}");
            assert!(err.to_string().contains("changed while it was read") && err.to_string().contains("fix: read it again"), "{err}");
            assert_eq!(calls.get(), 4, "one read and three reruns");
        }

        #[cfg(unix)]
        #[test]
        fn a_link_to_a_file_inside_the_folder_put_in_the_way_is_followed_by_the_retry() {
            let (_tmp, store) = docs();
            std::fs::write(store.dir().join("b.md"), "# b\n").unwrap();
            let (dir, calls) = (store.dir().to_path_buf(), Rc::new(Cell::new(0)));
            let seen = calls.clone();
            set_after(move || {
                seen.set(seen.get() + 1);
                if seen.get() == 1 {
                    std::fs::remove_file(dir.join("a.md")).unwrap();
                    std::os::unix::fs::symlink(dir.join("b.md"), dir.join("a.md")).unwrap();
                }
            });
            let read = store.read_page("a.md");
            clear_after();
            assert_eq!(read.expect("inside the folder").value.render(), "# b\n");
        }
    }
}
