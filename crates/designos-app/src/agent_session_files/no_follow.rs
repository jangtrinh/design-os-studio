//! Writes below a session folder that no link, and no folder swapped for a link, can carry out of it.
//!
//! The agent of a tab can create, rename and swap anything inside its session folder, in a loop. A path checked a moment ago
//! may be a link now, so nothing here checks a path and then uses it. The session folder is opened once, with `O_NOFOLLOW`
//! and an owner check; every subfolder is opened relative to its parent's descriptor with `O_NOFOLLOW` (`openat`, or
//! `mkdirat` first); every file is made, renamed and removed relative to the descriptor of the folder that holds it
//! (`openat`, `renameat`, `unlinkat`). A link at any step is refused, and a folder that is swapped for a link after it was
//! opened changes nothing: the descriptor still names the folder Studio opened.

mod files;
mod remove;
mod set_aside;
#[cfg(test)]
pub(super) mod test_hooks;

pub(super) use files::open_plain_path;

use super::folder_owner::check_owner;
use super::refusal::{cannot, failed, refused, Fix, Refusal};
use std::ffi::{CStr, CString, OsStr};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

const FOLDER_FLAGS: libc::c_int = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
/// How often a name somebody else took, or a folder that vanishes again, is tried before Studio gives up.
const ATTEMPTS: usize = 6;

fn c_name(name: &OsStr) -> io::Result<CString> {
    CString::new(name.as_bytes()).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "a name holds a NUL byte"))
}

/// Runs a system call again for as long as a signal interrupts it.
fn retry(mut call: impl FnMut() -> libc::c_int) -> io::Result<libc::c_int> {
    loop {
        let rc = call();
        if rc >= 0 {
            return Ok(rc);
        }
        let e = io::Error::last_os_error();
        if e.kind() != io::ErrorKind::Interrupted {
            return Err(e);
        }
    }
}

/// `e` for a file that could not be made in the folder `folder`, which the line names: a folder that refuses was made read-only.
fn cannot_write_to(folder: &Path, e: io::Error) -> io::Error {
    let fix = if e.kind() == io::ErrorKind::PermissionDenied { Fix::MakeWritable } else { Fix::CheckPermissions };
    cannot("written to", folder, e, fix)
}

/// A folder Studio holds open. Every call acts on the folder the descriptor names, whatever its path became since.
pub(super) struct Dir {
    fd: OwnedFd,
    path: PathBuf,
}

impl Dir {
    fn adopt(fd: libc::c_int, path: PathBuf) -> Dir {
        // SAFETY: `fd` was just returned by a successful `open`/`openat`/`fcntl` and nothing else owns it.
        Dir { fd: unsafe { OwnedFd::from_raw_fd(fd) }, path }
    }

    fn raw(&self) -> libc::c_int {
        self.fd.as_raw_fd()
    }

    /// Opens the session folder at `path`, which must be a real folder of the user. A link there is refused, so is a file or
    /// a folder of another user; a folder that does not exist is a `NotFound` error. Folders above `path` are Studio's own.
    pub(super) fn open_session_folder(path: &Path) -> io::Result<Dir> {
        let c = c_name(path.as_os_str())?;
        for _ in 0..ATTEMPTS {
            #[cfg(test)]
            test_hooks::run(path, test_hooks::Step::BeforeOpen);
            // SAFETY: `c` is a NUL-terminated path that outlives the call.
            match retry(|| unsafe { libc::open(c.as_ptr(), FOLDER_FLAGS) }) {
                Ok(fd) => {
                    let dir = Dir::adopt(fd, path.to_path_buf());
                    dir.require_owner()?;
                    return Ok(dir);
                }
                Err(e) => {
                    #[cfg(test)]
                    test_hooks::run(path, test_hooks::Step::AfterFailedOpen);
                    let entry = std::fs::symlink_metadata(path).map(|meta| (meta.file_type().is_symlink(), meta.is_dir()));
                    if let Some(error) = explain_open(path, e, entry) {
                        return Err(error);
                    }
                }
            }
        }
        Err(refused(Refusal::Link, path))
    }

    fn require_owner(&self) -> io::Result<()> {
        // SAFETY: an all-zero `stat` is a valid value, and `fstat` fills it for the descriptor this folder owns.
        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        retry(|| unsafe { libc::fstat(self.raw(), &mut st) }).map_err(|e| cannot("checked", &self.path, e, Fix::CheckPermissions))?;
        // SAFETY: `geteuid` takes no arguments and cannot fail.
        check_owner(&self.path, st.st_uid, unsafe { libc::geteuid() })
    }

    /// `(is a link, is a folder)` of the entry `name`, without following it.
    fn kind_of(&self, name: &CStr) -> io::Result<(bool, bool)> {
        // SAFETY: an all-zero `stat` is a valid value, and `fstatat` fills it; `name` is NUL-terminated.
        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        retry(|| unsafe { libc::fstatat(self.raw(), name.as_ptr(), &mut st, libc::AT_SYMLINK_NOFOLLOW) })?;
        let kind = st.st_mode & libc::S_IFMT;
        Ok((kind == libc::S_IFLNK, kind == libc::S_IFDIR))
    }

    /// Opens the subfolder `name`; with `create`, a missing one is made first (mode 0700). A link or a file is refused. A link that
    /// is a real folder again by the time it is looked at (its owner took it back) is tried again, up to `ATTEMPTS` times, and is
    /// then refused as a link: that is what the open met each time.
    pub(super) fn open_child(&self, name: &OsStr, create: bool) -> io::Result<Dir> {
        let (c, path) = (c_name(name)?, self.path.join(name));
        let mut met_a_link = false;
        for _ in 0..ATTEMPTS {
            #[cfg(test)]
            test_hooks::run(&path, test_hooks::Step::BeforeOpen);
            // SAFETY: `c` is a NUL-terminated name that outlives the call; the descriptor is this folder's.
            match retry(|| unsafe { libc::openat(self.raw(), c.as_ptr(), FOLDER_FLAGS) }) {
                Ok(fd) => return Ok(Dir::adopt(fd, path)),
                Err(e) if create && e.kind() == io::ErrorKind::NotFound => {
                    // SAFETY: as above.
                    match retry(|| unsafe { libc::mkdirat(self.raw(), c.as_ptr(), 0o700) }) {
                        // Somebody made it first: the next `openat` looks at what is there, which may be a link.
                        Ok(_) => {}
                        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                        Err(e) => return Err(cannot("created", &path, e, Fix::CheckPermissions)),
                    }
                }
                Err(e) => {
                    #[cfg(test)]
                    test_hooks::run(&path, test_hooks::Step::AfterFailedOpen);
                    if let Some(error) = explain_open(&path, e, self.kind_of(&c)) {
                        return Err(error);
                    }
                    met_a_link = true;
                }
            }
        }
        if met_a_link {
            return Err(refused(Refusal::Link, &path));
        }
        Err(cannot("opened", &path, io::Error::other("it was removed again each time"), Fix::CheckPermissions))
    }
}

/// Why opening the folder `path` failed, as the error to return. `None` when what the open met, a link or a file, has gone since
/// or is a real folder now (`entry` is `(is a link, is a folder)` of what sits there now, when that can be told): its owner took it
/// back or removed it, and the open is worth another try.
fn explain_open(path: &Path, e: io::Error, entry: io::Result<(bool, bool)>) -> Option<io::Error> {
    let not_a_folder = matches!(e.raw_os_error(), Some(libc::ELOOP | libc::ENOTDIR));
    match entry {
        Ok((true, _)) if not_a_folder => Some(refused(Refusal::Link, path)),
        Ok((false, false)) if not_a_folder => Some(refused(Refusal::File, path)),
        Ok((false, true)) if not_a_folder => None,
        Err(gone) if not_a_folder && gone.kind() == io::ErrorKind::NotFound => None,
        _ if e.kind() == io::ErrorKind::NotFound => Some(e),
        _ => Some(cannot("opened", path, e, Fix::CheckPermissions)),
    }
}
