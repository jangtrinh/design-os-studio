//! Writing, reading, renaming and removing files below a session folder, by path. A path is split into the session folder it
//! lies in and the names below it, and the work goes through [`Dir`], which follows no link below the session folder: a link
//! planted by the agent at any of those names is refused (or, as the name of a file, replaced), and never carries a write out.
//!
//! A path outside every session folder (a test's scratch folder) is anchored at its own folder instead: nothing above it is
//! checked, and the name is handled the same way.

use super::folder::{create_session_folder_in, ephemeral_sessions_root, no_private_data_folder, session_folder_of, UNAVAILABLE_ROOT};
use super::no_follow::Dir;
use super::refusal::{cannot, failed, Fix};
use std::ffi::OsString;
use std::io;
use std::path::{Component, Path, PathBuf};

/// Where a path lies: the folder that anchors it, the subfolders between that folder and the entry, and the entry's name.
struct Place {
    anchor: PathBuf,
    folders: Vec<OsString>,
    name: OsString,
}

fn place_of(path: &Path) -> io::Result<Place> {
    let invalid = |why: &str| io::Error::new(io::ErrorKind::InvalidInput, format!("{} {why}", path.display()));
    let name = path.file_name().ok_or_else(|| invalid("has no name"))?.to_os_string();
    let parent = path.parent().ok_or_else(|| invalid("has no folder"))?;
    let anchor = session_folder_of(path).unwrap_or_else(|| parent.to_path_buf());
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

/// Atomically writes `bytes` to `path` via a temporary file of the same folder followed by a rename, so concurrent MCP readers
/// never observe a half-written file. The session folder, and the folders below it, are made when they are missing.
pub fn atomic_write_session_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_session_file_unless(path, bytes, || false).map(|_| ())
}

/// Like [`atomic_write_session_file`], but `hold` is asked after the temporary file is complete and just before it replaces
/// `path`. When it says yes the temporary file is removed, `path` is left as it was and `Ok(true)` comes back.
pub fn write_session_file_unless(path: &Path, bytes: &[u8], hold: impl FnOnce() -> bool) -> io::Result<bool> {
    let place = place_of(path)?;
    open_holder(&place, true)?.write_atomic(&place.name, bytes, hold)
}

/// [`atomic_write_session_file`], then reads the file back through the same open folder and fails when it does not hold what
/// was written: another agent tab of the folder replaced it in between.
pub fn write_session_file_verified(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let place = place_of(path)?;
    let dir = open_holder(&place, true)?;
    dir.write_atomic(&place.name, bytes, || false)?;
    #[cfg(test)]
    run_before_read_back_test_hook(path);
    if dir.read(&place.name)? != bytes {
        return Err(failed(io::ErrorKind::InvalidData, format!("{} changed while Studio wrote it", path.display()), Fix::CloseOtherTabs));
    }
    Ok(())
}

/// Makes the new file `path` holding `bytes`; an existing entry, a link included, is an `AlreadyExists` error.
pub fn create_session_file_exclusive(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let place = place_of(path)?;
    open_holder(&place, true)?.create_new(&place.name, bytes)
}

/// The content of the plain file `path`; a link, or anything but a plain file, is an error.
pub fn read_session_file(path: &Path) -> io::Result<Vec<u8>> {
    let place = place_of(path)?;
    open_holder(&place, false)?.read(&place.name)
}

/// Removes the file or link `path` (a link is removed itself).
pub fn remove_session_file(path: &Path) -> io::Result<()> {
    let place = place_of(path)?;
    open_holder(&place, false)?.remove_file(&place.name)
}

/// Removes the folder, file or link `path`, and everything in a folder, without following a link below the session folder.
/// What is not there is already removed.
pub fn remove_session_dir_all(path: &Path) -> io::Result<()> {
    let place = place_of(path)?;
    match open_holder(&place, false) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        holder => holder?.remove_all(&place.name),
    }
}

/// Renames the entry `from` to `to`, which must lie in the same folder.
pub fn rename_session_file(from: &Path, to: &Path) -> io::Result<()> {
    let (place, target) = (place_of(from)?, to.file_name().filter(|_| from.parent() == to.parent()));
    let target = target.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("{} is not in the folder of {}", to.display(), from.display())))?;
    open_holder(&place, false)?.rename_entry(&place.name, target)
}

/// Makes the folder `path` and the folders above it that are missing, below the session folder it lies in (which is made too,
/// when it is a session folder that is missing). A link or a file at any of them is refused.
pub fn create_session_dir_all(path: &Path) -> io::Result<()> {
    let place = place_of(path)?;
    open_holder(&place, true)?.open_child(&place.name, true).map(|_| ())
}

#[cfg(test)]
type ReadBackHook = Box<dyn FnOnce() + Send>;

#[cfg(test)]
static READ_BACK_TEST_HOOKS: std::sync::Mutex<Vec<(PathBuf, ReadBackHook)>> = std::sync::Mutex::new(Vec::new());

/// Runs `hook` once, on the thread that writes `path` with [`write_session_file_verified`], between the write and the read
/// back, so a test can replace the file in that window. Keyed by path so parallel tests never run each other's hook.
#[cfg(test)]
pub(super) fn set_before_read_back_test_hook(path: &Path, hook: ReadBackHook) {
    READ_BACK_TEST_HOOKS.lock().unwrap_or_else(|e| e.into_inner()).push((path.to_path_buf(), hook));
}

#[cfg(test)]
fn run_before_read_back_test_hook(path: &Path) {
    let hook = {
        let mut hooks = READ_BACK_TEST_HOOKS.lock().unwrap_or_else(|e| e.into_inner());
        hooks.iter().position(|(p, _)| p == path).map(|i| hooks.remove(i).1)
    };
    if let Some(hook) = hook {
        hook();
    }
}
