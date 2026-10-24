//! The files of an open folder: made, written, read, renamed and removed relative to its descriptor.

use super::{c_name, cannot, cannot_write_to, failed, retry, Dir, Fix, ATTEMPTS};
use std::ffi::{CStr, CString, OsStr};
use std::io::{self, Read, Write};
use std::os::fd::{FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

const NEW_FILE_FLAGS: libc::c_int = libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC;

/// `fsync` of `file`. `File::sync_all` is `F_FULLFSYNC` on macOS, which also empties the drive's own cache: 22 small files took
/// 305 to 380 ms through it and 4 to 7 ms through `fsync` (`evidence/perf-22-files-*.log` of the change that added this), and a tab's
/// start writes that many on the UI thread. `fsync` is what the rename needs: a complete file under the new name.
fn flush_to_disk(file: &std::fs::File) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    // SAFETY: the descriptor belongs to `file`, which outlives the call.
    retry(|| unsafe { libc::fsync(file.as_raw_fd()) }).map(|_| ())
}

impl Dir {
    /// Makes a new file `name` (mode 0600) that is not there yet and returns it open for writing. A link at `name` is not followed.
    fn create_file(&self, name: &[u8]) -> io::Result<std::fs::File> {
        let c = CString::new(name).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "a name holds a NUL byte"))?;
        // SAFETY: `c` is NUL-terminated and outlives the call; the mode is promoted to the `int` a variadic call takes.
        let fd = retry(|| unsafe { libc::openat(self.raw(), c.as_ptr(), NEW_FILE_FLAGS, 0o600 as libc::c_uint) })?;
        // SAFETY: `fd` was just returned by a successful `openat` and nothing else owns it.
        Ok(std::fs::File::from(unsafe { OwnedFd::from_raw_fd(fd) }))
    }

    /// Writes `bytes` to a new file `name`, which must not exist yet, and makes them durable.
    pub(in crate::agent_session_files) fn create_new(&self, name: &OsStr, bytes: &[u8]) -> io::Result<()> {
        let path = self.path.join(name);
        let mut file = self.create_file(name.as_bytes()).map_err(|e| cannot_write_to(&self.path, e))?;
        file.write_all(bytes).and_then(|()| flush_to_disk(&file)).map_err(|e| cannot("written", &path, e, Fix::CheckPermissions))
    }

    /// Writes `bytes` to `name` as a whole or not at all: to a temporary file of this folder, then `renameat` over `name`.
    /// `hold` is asked once the temporary file is complete; when it says yes the file is removed and `Ok(true)` comes back.
    /// A link at `name` is replaced, never followed.
    pub(in crate::agent_session_files) fn write_atomic(&self, name: &OsStr, bytes: &[u8], hold: impl FnOnce() -> bool) -> io::Result<bool> {
        let path = self.path.join(name);
        let (tmp, mut file) = self.create_temp(name).map_err(|e| cannot_write_to(&self.path, e))?;
        let drop_tmp = |e: io::Error| {
            let _ = self.unlink(&tmp, 0);
            cannot("written", &path, e, Fix::CheckPermissions)
        };
        file.write_all(bytes).and_then(|()| flush_to_disk(&file)).map_err(drop_tmp)?;
        drop(file);
        if hold() {
            let _ = self.unlink(&tmp, 0);
            return Ok(true);
        }
        self.rename(&tmp, &c_name(name)?).map_err(drop_tmp)?;
        Ok(false)
    }

    /// A new file with a name nobody could have guessed (`.<name>.<random>.tmp`), made with `O_EXCL`.
    fn create_temp(&self, name: &OsStr) -> io::Result<(CString, std::fs::File)> {
        for _ in 0..ATTEMPTS {
            let mut tmp = vec![b'.'];
            tmp.extend_from_slice(name.as_bytes());
            tmp.extend_from_slice(format!(".{}.tmp", &uuid::Uuid::new_v4().simple().to_string()[..16]).as_bytes());
            match self.create_file(&tmp) {
                Ok(file) => return Ok((CString::new(tmp).map_err(io::Error::other)?, file)),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(io::Error::new(io::ErrorKind::AlreadyExists, "no free name for a temporary file"))
    }

    fn rename(&self, from: &CStr, to: &CStr) -> io::Result<()> {
        // SAFETY: both names are NUL-terminated and outlive the call; both are relative to this folder's descriptor.
        retry(|| unsafe { libc::renameat(self.raw(), from.as_ptr(), self.raw(), to.as_ptr()) }).map(|_| ())
    }

    /// Renames the entry `from` of this folder to `to`, another entry of the same folder.
    pub(in crate::agent_session_files) fn rename_entry(&self, from: &OsStr, to: &OsStr) -> io::Result<()> {
        self.rename(&c_name(from)?, &c_name(to)?).map_err(|e| cannot("renamed", &self.path.join(from), e, Fix::CheckPermissions))
    }

    pub(super) fn unlink(&self, name: &CStr, flags: libc::c_int) -> io::Result<()> {
        // SAFETY: `name` is NUL-terminated and outlives the call; it is relative to this folder's descriptor.
        retry(|| unsafe { libc::unlinkat(self.raw(), name.as_ptr(), flags) }).map(|_| ())
    }

    /// Removes the file or link `name`; a link is removed itself.
    pub(in crate::agent_session_files) fn remove_file(&self, name: &OsStr) -> io::Result<()> {
        self.unlink(&c_name(name)?, 0).map_err(|e| cannot("removed", &self.path.join(name), e, Fix::CheckPermissions))
    }

    /// Opens the plain file `name` for reading, to read it by offset: relative to this folder, so a link at `name` is refused
    /// (`O_NOFOLLOW`), and without blocking, so a FIFO is refused at once. Anything but a plain file is an error. The file comes
    /// back blocking again, which for a plain file only matters to what the caller does with it next.
    pub(in crate::agent_session_files) fn open_plain(&self, name: &OsStr) -> io::Result<std::fs::File> {
        let (c, path) = (c_name(name)?, self.path.join(name));
        let flags = libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK;
        // SAFETY: `c` is NUL-terminated and outlives the call; the descriptor is this folder's.
        let fd = retry(|| unsafe { libc::openat(self.raw(), c.as_ptr(), flags) }).map_err(|e| unreadable(&path, e))?;
        // SAFETY: `fd` was just returned by a successful `openat` and nothing else owns it.
        into_plain_reader(std::fs::File::from(unsafe { OwnedFd::from_raw_fd(fd) }), &path)
    }

    /// The whole content of the file `name`. A link, or anything but a plain file, is refused.
    pub(in crate::agent_session_files) fn read(&self, name: &OsStr) -> io::Result<Vec<u8>> {
        let mut file = self.open_plain(name)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(|e| unreadable(&self.path.join(name), e))?;
        Ok(bytes)
    }
}

/// Why `path` could not be read: a link says so, anything else is the error of the open.
fn unreadable(path: &Path, e: io::Error) -> io::Error {
    if e.raw_os_error() == Some(libc::ELOOP) {
        return failed(io::ErrorKind::InvalidInput, format!("{} is a link, not a file", path.display()), Fix::DeleteLink);
    }
    cannot("read", path, e, Fix::CheckPermissions)
}

/// `path` is something else than a plain file: a FIFO, a folder, a device.
fn not_a_plain_file(path: &Path) -> io::Error {
    failed(io::ErrorKind::InvalidInput, format!("{} is not a plain file", path.display()), Fix::DeleteIt)
}

/// `file` (just opened with `O_NONBLOCK`) when it is a plain file, made blocking again; anything else is an error.
fn into_plain_reader(file: std::fs::File, path: &Path) -> io::Result<std::fs::File> {
    use std::os::fd::AsRawFd;
    if !file.metadata().map_err(|e| unreadable(path, e))?.is_file() {
        return Err(not_a_plain_file(path));
    }
    // SAFETY: the descriptor belongs to `file`, which outlives both calls.
    let flags = retry(|| unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFL) }).map_err(|e| unreadable(path, e))?;
    retry(|| unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFL, flags & !libc::O_NONBLOCK) }).map_err(|e| unreadable(path, e))?;
    Ok(file)
}

/// Opens `path`, which lies outside every session folder (the transcript a CLI names), for reading by offset. Links are followed,
/// there is no folder to hold them to, but a FIFO is refused at once: only a plain file comes back.
pub(in crate::agent_session_files) fn open_plain_path(path: &Path) -> io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    let file = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK).open(path).map_err(|e| unreadable(path, e))?;
    into_plain_reader(file, path)
}
