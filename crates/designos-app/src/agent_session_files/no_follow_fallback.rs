//! The calls of `no_follow` for a platform without `openat`: every step checks the entry and then acts on its path, so a link
//! swapped in between the check and the write is not caught here. A link that is already there is refused at any step.

use super::refusal::{cannot, failed, refused, Fix, Refusal};
use std::ffi::OsStr;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// A folder below a session folder, held by its path.
pub(super) struct Dir {
    path: PathBuf,
}

/// Refuses a link or a file where a folder should be.
fn require_real_folder(path: &Path) -> io::Result<()> {
    let meta = std::fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() {
        return Err(refused(Refusal::Link, path));
    }
    if !meta.is_dir() {
        return Err(refused(Refusal::File, path));
    }
    Ok(())
}

fn cannot_io(what: &str, path: &Path, e: io::Error) -> io::Error {
    cannot(what, path, e, Fix::CheckPermissions)
}

impl Dir {
    /// The session folder at `path`, which must be a real folder; a folder that does not exist is a `NotFound` error.
    pub(super) fn open_session_folder(path: &Path) -> io::Result<Dir> {
        require_real_folder(path)?;
        Ok(Dir { path: path.to_path_buf() })
    }

    /// The subfolder `name`; with `create`, a missing one is made first. A link or a file is refused.
    pub(super) fn open_child(&self, name: &OsStr, create: bool) -> io::Result<Dir> {
        let path = self.path.join(name);
        match std::fs::symlink_metadata(&path) {
            Ok(_) => {}
            Err(e) if create && e.kind() == io::ErrorKind::NotFound => {
                std::fs::create_dir(&path).or_else(|e| if e.kind() == io::ErrorKind::AlreadyExists { Ok(()) } else { Err(cannot_io("created", &path, e)) })?
            }
            Err(e) => return Err(e),
        }
        require_real_folder(&path)?;
        Ok(Dir { path })
    }

    /// Writes `bytes` to a new file `name`, which must not exist yet, and makes them durable.
    pub(in crate::agent_session_files) fn create_new(&self, name: &OsStr, bytes: &[u8]) -> io::Result<()> {
        let path = self.path.join(name);
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path).map_err(|e| cannot_io("created", &path, e))?;
        file.write_all(bytes).and_then(|()| file.sync_all()).map_err(|e| cannot_io("written", &path, e))
    }

    /// Writes `bytes` to `name` as a whole or not at all (a temporary file of this folder, then a rename over `name`).
    /// `hold` is asked once the temporary file is complete; when it says yes the file is removed and `Ok(true)` comes back.
    pub(in crate::agent_session_files) fn write_atomic(&self, name: &OsStr, bytes: &[u8], hold: impl FnOnce() -> bool) -> io::Result<bool> {
        let path = self.path.join(name);
        let tmp = self.path.join(format!(".{}.{}.tmp", name.to_string_lossy(), &uuid::Uuid::new_v4().simple().to_string()[..16]));
        let written =
            std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp).and_then(|mut file| file.write_all(bytes).and_then(|()| file.sync_all()));
        if let Err(e) = written {
            let _ = std::fs::remove_file(&tmp);
            return Err(cannot_io("written", &path, e));
        }
        if hold() {
            let _ = std::fs::remove_file(&tmp);
            return Ok(true);
        }
        std::fs::rename(&tmp, &path).map(|()| false).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            cannot_io("written", &path, e)
        })
    }

    /// Renames the entry `from` of this folder to `to`, another entry of the same folder.
    pub(in crate::agent_session_files) fn rename_entry(&self, from: &OsStr, to: &OsStr) -> io::Result<()> {
        std::fs::rename(self.path.join(from), self.path.join(to)).map_err(|e| cannot_io("renamed", &self.path.join(from), e))
    }

    /// Removes the file or link `name`.
    pub(in crate::agent_session_files) fn remove_file(&self, name: &OsStr) -> io::Result<()> {
        std::fs::remove_file(self.path.join(name)).map_err(|e| cannot_io("removed", &self.path.join(name), e))
    }

    /// Opens the plain file `name` for reading. A link, or anything but a plain file, is refused.
    pub(in crate::agent_session_files) fn open_plain(&self, name: &OsStr) -> io::Result<std::fs::File> {
        let path = self.path.join(name);
        let meta = std::fs::symlink_metadata(&path).map_err(|e| cannot_io("read", &path, e))?;
        if !meta.is_file() {
            return Err(failed(io::ErrorKind::InvalidInput, format!("{} is not a plain file", path.display()), Fix::DeleteIt));
        }
        std::fs::File::open(&path).map_err(|e| cannot_io("read", &path, e))
    }

    /// The whole content of the file `name`. A link, or anything but a plain file, is refused.
    pub(in crate::agent_session_files) fn read(&self, name: &OsStr) -> io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        io::Read::read_to_end(&mut self.open_plain(name)?, &mut bytes).map_err(|e| cannot_io("read", &self.path.join(name), e))?;
        Ok(bytes)
    }

    /// Removes the entry `name`: a file or link itself, a folder with everything in it.
    pub(in crate::agent_session_files) fn remove_all(&self, name: &OsStr) -> io::Result<()> {
        let path = self.path.join(name);
        let removed = match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => std::fs::remove_dir_all(&path),
            Ok(_) => std::fs::remove_file(&path).or_else(|_| std::fs::remove_dir(&path)),
            Err(e) => Err(e),
        };
        match removed {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other.map_err(|e| cannot_io("removed", &path, e)),
        }
    }
}

/// Opens `path`, which lies outside every session folder (the transcript a CLI names), for reading by offset: a plain file, or an
/// error.
pub(super) fn open_plain_path(path: &Path) -> io::Result<std::fs::File> {
    let meta = std::fs::metadata(path).map_err(|e| cannot_io("read", path, e))?;
    if !meta.is_file() {
        return Err(failed(io::ErrorKind::InvalidInput, format!("{} is not a plain file", path.display()), Fix::DeleteIt));
    }
    std::fs::File::open(path).map_err(|e| cannot_io("read", path, e))
}
