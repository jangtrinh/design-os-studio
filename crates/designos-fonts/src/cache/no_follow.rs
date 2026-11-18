//! The no-follow twin of [`write_atomic_with`](super::write_atomic_with), for a path somebody else named: an agent's output
//! path, a headless command's `--out`. The ordinary writer saves *through* a link, which is right for an editor save and wrong
//! when the name is not trusted: a planted link would steer the write onto any file the process can write.
//!
//! What this writer adds to the ordinary one, and nothing else:
//! - a link at the final path component is refused (`O_NOFOLLOW` on the open that also keeps the handle for the metadata copy,
//!   so there is no check-then-use gap); the rename that installs the new file replaces whatever is at the path and follows
//!   nothing, so a link swapped in after the open is replaced, never written through;
//! - a file with a second name (`nlink > 1`) is refused: the new file would leave the other name on the old bytes;
//! - it makes no folder: the caller owns the parent.
//!
//! Everything else is shared with the ordinary writer: the content is streamed into a temporary file beside the target, the
//! mode, and on macOS the ACL and extended attributes, are copied from the file that was opened, a failed `fill` leaves the old
//! file untouched, a named pipe or a folder is refused without blocking.
//!
//! Known limit: a platform without `O_NOFOLLOW` (not unix) looks at the entry first and opens it by path afterwards, so a link
//! swapped in between the two is not caught there. A link that is already in place is still refused.

use super::{sweep_stale_temps, tmp_prefix, write_via_tmp, ExistingTarget};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// Why a no-follow write refused its target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoFollowRefusal {
    /// A link at the final component.
    Link,
    /// A plain file with a second name (a hard link).
    SecondName,
    /// A folder.
    Directory,
    /// A named pipe, a socket or a device.
    NotRegularFile,
    /// A file this process may not write (a read-only mode, or an ACL or flag that denies it).
    NotWritable,
}

/// The error a refused target carries inside its [`io::Error`].
#[derive(Debug)]
struct Refused {
    kind: NoFollowRefusal,
    path: PathBuf,
    /// What the operating system said, for [`NoFollowRefusal::NotWritable`].
    cause: Option<String>,
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let path = self.path.display();
        match (self.kind, &self.cause) {
            (NoFollowRefusal::Link, _) => write!(f, "{path} is a link, not a file"),
            (NoFollowRefusal::SecondName, _) => write!(f, "{path} has a second name (a hard link)"),
            (NoFollowRefusal::Directory, _) => write!(f, "{path} is a directory"),
            (NoFollowRefusal::NotRegularFile, _) => write!(f, "{path} is not a regular file"),
            (NoFollowRefusal::NotWritable, Some(cause)) => write!(f, "{path} is read-only or not writable: {cause}"),
            (NoFollowRefusal::NotWritable, None) => write!(f, "{path} is read-only or not writable"),
        }
    }
}

impl std::error::Error for Refused {}

/// A refusal of `path` as an [`io::Error`]. The error kind matches what the ordinary writer reports for the same case.
pub(super) fn refused(kind: NoFollowRefusal, path: &Path, cause: Option<String>) -> io::Error {
    let io_kind = match kind {
        NoFollowRefusal::Directory | NoFollowRefusal::NotRegularFile => io::ErrorKind::InvalidInput,
        NoFollowRefusal::Link | NoFollowRefusal::SecondName | NoFollowRefusal::NotWritable => io::ErrorKind::PermissionDenied,
    };
    io::Error::new(io_kind, Refused { kind, path: path.to_path_buf(), cause })
}

/// Why `e` refused a path, when it did. Any other error (a full disk, a missing folder) is `None`.
pub fn no_follow_refusal(e: &io::Error) -> Option<NoFollowRefusal> {
    e.get_ref()?.downcast_ref::<Refused>().map(|refused| refused.kind)
}

/// [`write_atomic_with`](super::write_atomic_with) that never follows a link at the final path component. See the module
/// comment for what it refuses and keeps. A refusal carries a [`NoFollowRefusal`] ([`no_follow_refusal`]).
pub fn write_atomic_with_no_follow(path: &Path, fill: impl FnOnce(&mut dyn io::Write) -> io::Result<()>) -> io::Result<()> {
    let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("{} has no file name", path.display())));
    };
    let existing = open_existing(path)?;
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let prefix = tmp_prefix(&name);
    sweep_stale_temps(dir, prefix);
    write_via_tmp(path, dir, prefix, fill, existing)
}

/// [`write_atomic_with_no_follow`] for content that is already in memory.
pub fn write_atomic_no_follow(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_atomic_with_no_follow(path, |out| out.write_all(bytes))
}

/// The file at `path` opened without following a link, or `None` when nothing is there.
fn open_existing(path: &Path) -> io::Result<Option<ExistingTarget>> {
    match super::open_writable_regular(path, true) {
        Ok((file, meta)) => {
            #[cfg(not(target_os = "macos"))]
            {
                // Only macOS lets an ACL grant write where the mode does not (the open above already said yes).
                if meta.permissions().readonly() {
                    return Err(refused(NoFollowRefusal::NotWritable, path, Some("permission denied".into())));
                }
                drop(file);
            }
            Ok(Some(ExistingTarget {
                #[cfg(target_os = "macos")]
                file,
                meta,
            }))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
