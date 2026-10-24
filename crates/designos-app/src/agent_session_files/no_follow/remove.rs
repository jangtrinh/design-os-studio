//! Removing a folder with everything in it through descriptors, so nothing a link points at is touched.

use super::{c_name, cannot, retry, Dir, Fix};
use std::ffi::{CStr, OsStr, OsString};
use std::io;
use std::os::unix::ffi::OsStringExt;

/// The deepest folder tree that is removed: an agent could make one deep enough to exhaust the stack.
const MAX_DEPTH: usize = 128;

impl Dir {
    /// Removes the entry `name` of this folder. A file or a link goes itself (a link is never followed); a folder is emptied
    /// through its own descriptor, entry by entry, and then removed. A folder swapped for a link meanwhile is refused.
    pub(in crate::agent_session_files) fn remove_all(&self, name: &OsStr) -> io::Result<()> {
        self.remove_all_at(name, 0)
    }

    fn remove_all_at(&self, name: &OsStr, depth: usize) -> io::Result<()> {
        let (c, path) = (c_name(name)?, self.path.join(name));
        let unremovable = |e: io::Error| cannot("removed", &path, e, Fix::CheckPermissions);
        let is_folder = match self.kind_of(&c) {
            Ok((_, is_folder)) => is_folder,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(unremovable(e)),
        };
        if !is_folder {
            return self.unlink(&c, 0).map_err(unremovable);
        }
        if depth >= MAX_DEPTH {
            return Err(unremovable(io::Error::other("it is nested too deeply")));
        }
        let folder = self.open_child(name, false)?;
        for entry in folder.entries().map_err(unremovable)? {
            folder.remove_all_at(&entry, depth + 1)?;
        }
        drop(folder);
        self.unlink(&c, libc::AT_REMOVEDIR).map_err(unremovable)
    }

    /// The names in this folder, without `.` and `..`.
    pub(super) fn entries(&self) -> io::Result<Vec<OsString>> {
        // `fdopendir` takes over the descriptor it is given, so it gets a duplicate. A duplicate shares the read position of its
        // original, which is at the top of the folder only because every `Dir` is opened fresh and nothing else lists it.
        // SAFETY: `F_DUPFD_CLOEXEC` takes the lowest free descriptor number at or above its argument.
        let dup = retry(|| unsafe { libc::fcntl(self.raw(), libc::F_DUPFD_CLOEXEC, 0) })?;
        // SAFETY: `dup` is a descriptor this function owns; on success the stream owns it.
        let stream = unsafe { libc::fdopendir(dup) };
        if stream.is_null() {
            let e = io::Error::last_os_error();
            // SAFETY: the stream was not made, so `dup` is still ours to close.
            unsafe { libc::close(dup) };
            return Err(e);
        }
        let mut names = Vec::new();
        loop {
            // SAFETY: `stream` is an open directory stream that nothing else reads.
            let entry = unsafe { libc::readdir(stream) };
            if entry.is_null() {
                break;
            }
            // SAFETY: `readdir` returned a valid entry whose `d_name` is NUL-terminated.
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
            if name != b"." && name != b".." {
                names.push(OsString::from_vec(name.to_vec()));
            }
        }
        // SAFETY: `stream` is open and closed exactly once, which also closes `dup`.
        unsafe { libc::closedir(stream) };
        Ok(names)
    }
}
