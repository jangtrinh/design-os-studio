//! Writing, reading, renaming and removing files below a session folder, by path. A path is split into the session folder it
//! lies in and the names below it, and the work goes through [`Dir`], which follows no link below the session folder: a link
//! planted by the agent at any of those names is refused (or, as the name of a file, replaced), and never carries a write out.
//!
//! A path in no session folder is refused. Only a test build anchors such a path (a scratch folder) at its own folder instead:
//! nothing above it is checked, and the name is handled the same way.

use super::folder::{create_session_folder_in, ephemeral_sessions_root, no_private_data_folder, session_folder_of, UNAVAILABLE_ROOT};
use super::no_follow::Dir;
use super::refusal::{cannot, failed, Fix};
use std::ffi::OsString;
use std::io;
use std::path::{Component, Path, PathBuf};

/// Where a path lies: the folder that anchors it, the subfolders between that folder and the entry, and the entry's name.
pub(super) struct Place {
    pub(super) anchor: PathBuf,
    pub(super) folders: Vec<OsString>,
    pub(super) name: OsString,
}

fn place_of(path: &Path) -> io::Result<Place> {
    place_of_with(path, cfg!(test))
}

/// [`place_of`]; `allow_unanchored` lets a path in no session folder lie in its own folder instead of being refused.
pub(super) fn place_of_with(path: &Path, allow_unanchored: bool) -> io::Result<Place> {
    let invalid = |why: &str| io::Error::new(io::ErrorKind::InvalidInput, format!("{} {why}", path.display()));
    let name = path.file_name().ok_or_else(|| invalid("has no name"))?.to_os_string();
    let parent = path.parent().ok_or_else(|| invalid("has no folder"))?;
    let anchor = match session_folder_of(path) {
        Some(folder) => folder,
        None if allow_unanchored => parent.to_path_buf(),
        None => {
            return Err(failed(
                io::ErrorKind::InvalidInput,
                format!("{} is outside this file's agent folder, so Studio doesn't use it", path.display()),
                Fix::ReopenFile,
            ))
        }
    };
    if anchor.starts_with(UNAVAILABLE_ROOT) {
        return Err(no_private_data_folder());
    }
    let mut folders = Vec::new();
    for part in parent.strip_prefix(&anchor).map_err(|_| invalid("is not below its folder"))?.components() {
        match part {
            Component::Normal(folder) => folders.push(folder.to_os_string()),
            _ => return Err(invalid("holds a part that is not a name")),
        }
    }
    Ok(Place { anchor, folders, name })
}

/// Opens the folder that holds the entry of `place`. With `create`, the session folder is made the way Studio makes any (see
/// [`create_session_folder_in`]) and the subfolders below it are made owner-only; without it, a missing folder is `NotFound`.
fn open_holder(place: &Place, create: bool) -> io::Result<Dir> {
    let mut dir = match Dir::open_session_folder(&place.anchor) {
        Err(e) if create && e.kind() == io::ErrorKind::NotFound => {
            if session_folder_of(&place.anchor.join("_")).is_some() {
                create_session_folder_in(ephemeral_sessions_root().as_deref(), &place.anchor)?;
            } else {
                std::fs::create_dir_all(&place.anchor).map_err(|e| cannot("created", &place.anchor, e, Fix::CheckPermissions))?;
            }
            Dir::open_session_folder(&place.anchor)?
        }
        other => other?,
    };
    for folder in &place.folders {
        dir = dir.open_child(folder, create)?;
    }
    Ok(dir)
}

/// The place of `path` and the open folder that holds it. Everything after this goes through that folder, whatever its path
/// holds by then.
fn open_place(path: &Path, create: bool) -> io::Result<(Place, Dir)> {
    let place = place_of(path)?;
    let dir = open_holder(&place, create)?;
    #[cfg(test)]
    test_hooks::run_after_open(path);
    Ok((place, dir))
}

/// Atomically writes `bytes` to `path` via a temporary file of the same folder followed by a rename, so concurrent MCP readers
/// never observe a half-written file. The session folder, and the folders below it, are made when they are missing.
pub fn atomic_write_session_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_session_file_unless(path, bytes, || false).map(|_| ())
}

/// Like [`atomic_write_session_file`], but `hold` is asked after the temporary file is complete and just before it replaces
/// `path`. When it says yes the temporary file is removed, `path` is left as it was and `Ok(true)` comes back.
pub fn write_session_file_unless(path: &Path, bytes: &[u8], hold: impl FnOnce() -> bool) -> io::Result<bool> {
    let (place, dir) = open_place(path, true)?;
    dir.write_atomic(&place.name, bytes, hold)
}

/// [`atomic_write_session_file`], then reads the file back through the same open folder and fails when it does not hold what
/// was written: another agent tab of the folder replaced it in between.
pub fn write_session_file_verified(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let (place, dir) = open_place(path, true)?;
    dir.write_atomic(&place.name, bytes, || false)?;
    #[cfg(test)]
    test_hooks::run_before_read_back(path);
    if dir.read(&place.name)? != bytes {
        return Err(failed(io::ErrorKind::InvalidData, format!("{} changed while Studio wrote it", path.display()), Fix::CloseOtherTabs));
    }
    Ok(())
}

/// Makes the new file `path` holding `bytes`; an existing entry, a link included, is an `AlreadyExists` error.
pub fn create_session_file_exclusive(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let (place, dir) = open_place(path, true)?;
    dir.create_new(&place.name, bytes)
}

/// The content of the plain file `path`; a link, or anything but a plain file, is an error.
pub fn read_session_file(path: &Path) -> io::Result<Vec<u8>> {
    let (place, dir) = open_place(path, false)?;
    dir.read(&place.name)
}

/// The content of the plain file `path` as text. A file that cannot be read, or is not text, is `None`: a poll with no data.
pub fn read_session_text(path: &Path) -> Option<String> {
    String::from_utf8(read_session_file(path).ok()?).ok()
}

/// The plain file `path`, open for reading by offset: relative to its session folder's handle, so never through a link, and
/// refused at once when it is a FIFO. For the readers that tail a file the agent writes.
pub fn open_session_file(path: &Path) -> io::Result<std::fs::File> {
    let (place, dir) = open_place(path, false)?;
    dir.open_plain(&place.name)
}

/// The metadata of the plain file `path`, taken from the file as it was opened, never from the target of a link. For the readers
/// that decide from a file's length, identity or modification time what to read. A link, a FIFO and a missing file are an error.
pub fn session_file_metadata(path: &Path) -> io::Result<std::fs::Metadata> {
    open_session_file(path)?.metadata()
}

/// The plain file `path` outside every session folder (the transcript a CLI names), open for reading by offset. It has no folder
/// to be held to, so a link is followed, but a FIFO is refused at once.
pub fn open_plain_file(path: &Path) -> io::Result<std::fs::File> {
    super::no_follow::open_plain_path(path)
}

/// Removes the file or link `path` (a link is removed itself).
pub fn remove_session_file(path: &Path) -> io::Result<()> {
    let (place, dir) = open_place(path, false)?;
    dir.remove_file(&place.name)
}

/// Removes the folder, file or link `path`, and everything in a folder, without following a link below the session folder.
/// What is not there is already removed.
pub fn remove_session_dir_all(path: &Path) -> io::Result<()> {
    match open_place(path, false) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        opened => {
            let (place, dir) = opened?;
            dir.remove_all(&place.name)
        }
    }
}

/// Renames the entry `from` to `to`, which must lie in the same folder.
pub fn rename_session_file(from: &Path, to: &Path) -> io::Result<()> {
    let target = to.file_name().filter(|_| from.parent() == to.parent());
    let target = target.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("{} is not in the folder of {}", to.display(), from.display())))?;
    let (place, dir) = open_place(from, false)?;
    dir.rename_entry(&place.name, target)
}

/// Makes the folder `path` and the folders above it that are missing, below the session folder it lies in (which is made too,
/// when it is a session folder that is missing). A link or a file at any of them is refused.
pub fn create_session_dir_all(path: &Path) -> io::Result<()> {
    let (place, dir) = open_place(path, true)?;
    dir.open_child(&place.name, true).map(|_| ())
}

/// Test-only hooks that put a swap at an exact point of a call, so a race is not left to chance. Each runs once, on the thread
/// of the call, and is keyed by the path the call was given, so parallel tests never run each other's.
#[cfg(test)]
pub(super) mod test_hooks {
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    type Hook = Box<dyn FnOnce() + Send>;
    type Hooks = Mutex<Vec<(PathBuf, Hook)>>;

    static AFTER_OPEN: Hooks = Mutex::new(Vec::new());
    static BEFORE_READ_BACK: Hooks = Mutex::new(Vec::new());

    fn run(hooks: &Hooks, path: &Path) {
        let hook = {
            let mut hooks = hooks.lock().unwrap_or_else(|e| e.into_inner());
            hooks.iter().position(|(p, _)| p == path).map(|i| hooks.remove(i).1)
        };
        if let Some(hook) = hook {
            hook();
        }
    }

    /// Runs `hook` once the folder that holds `path` is open and before the call does its work.
    pub(in crate::agent_session_files) fn set_after_open(path: &Path, hook: Hook) {
        AFTER_OPEN.lock().unwrap_or_else(|e| e.into_inner()).push((path.to_path_buf(), hook));
    }

    pub(super) fn run_after_open(path: &Path) {
        run(&AFTER_OPEN, path);
    }

    /// Runs `hook` between the write and the read back of [`super::write_session_file_verified`].
    pub(in crate::agent_session_files) fn set_before_read_back(path: &Path, hook: Hook) {
        BEFORE_READ_BACK.lock().unwrap_or_else(|e| e.into_inner()).push((path.to_path_buf(), hook));
    }

    pub(super) fn run_before_read_back(path: &Path) {
        run(&BEFORE_READ_BACK, path);
    }
}
