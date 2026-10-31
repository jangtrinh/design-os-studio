//! Every read and write of a session file goes through here. A session file is a direct child of `DESIGNOS_SESSION_DIR`, the
//! folder Studio makes for one agent tab and the agent can write in. The agent can plant a link or a FIFO at any of those
//! names, and a plain open by path would carry an append, an overwrite or a truncate to any file the user can write, or wait
//! for a FIFO for ever.
//!
//! On unix an open here follows no link at the last component (`O_NOFOLLOW`), waits for nobody (`O_NONBLOCK`), and then checks
//! the file it opened: only a plain file with one name is read or written through. A file that is
//! replaced as a whole is written to a temporary file with a random name first (`O_CREAT|O_EXCL|O_NOFOLLOW`, owner only) and
//! renamed over the target: a rename replaces whatever sits at the target, a link included, and follows nothing. Nothing here
//! makes the session folder. It is Studio's, and a folder that is missing is an error the caller reports.
//!
//! Known limit: a platform without `O_NOFOLLOW` (`path_based.rs`) checks the entry and then acts on its path, so a link swapped
//! in between the two is not caught there. A link that is already in place is still refused.

use std::collections::hash_map::RandomState;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::Metadata;
use std::hash::{BuildHasher, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(test)]
mod atomic_tests;
/// The fallback is compiled on unix too, in tests, so its checks run where the guards of `platform` would otherwise hide them.
#[cfg(all(test, unix))]
#[path = "session_file_io/path_based.rs"]
mod path_based;
#[cfg_attr(unix, path = "session_file_io/no_follow.rs")]
#[cfg_attr(not(unix), path = "session_file_io/path_based.rs")]
mod platform;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

/// `append` adds `bytes` to a plain file (made when missing). `truncate` empties a plain file; one that is not there is already
/// empty. `write_atomic` replaces the file as a whole or not at all. `durable` also syncs the file to disk. `read` returns a plain file's
/// bytes; a file that is not there is a `NotFound` error.
pub use platform::{append, read, truncate, write_atomic};

/// Why a path was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// A symbolic link: it reaches a file that is not the session's.
    Link,
    /// A FIFO, a folder, a device or a socket.
    NotPlainFile,
    /// A plain file with a second name (a hard link): the other name reaches a file that is not the session's.
    SecondName,
}

/// The error a refused path carries inside its [`io::Error`].
#[derive(Debug)]
struct Refused {
    kind: Refusal,
    path: PathBuf,
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let path = self.path.display();
        match self.kind {
            Refusal::Link => write!(f, "{path} is a link, not a file"),
            Refusal::NotPlainFile => write!(f, "{path} is not a plain file"),
            Refusal::SecondName => write!(f, "{path} has a second name (a hard link)"),
        }
    }
}

impl std::error::Error for Refused {}

fn refused(kind: Refusal, path: &Path) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, Refused { kind, path: path.to_path_buf() })
}

/// Why `e` refused a path, when it did (any other error, such as a full disk, is `None`).
pub fn refusal_of(e: &io::Error) -> Option<Refusal> {
    e.get_ref()?.downcast_ref::<Refused>().map(|refused| refused.kind)
}

/// What went wrong with `path` as a sentence for the agent: the cause, then what to do next (the cause first, then ", then try
/// again", as Studio words its own). `verb` is `read` or `write`; a failure that is not a refusal names the path and the cause.
pub fn explain(verb: &str, path: &Path, e: &io::Error) -> String {
    match refusal_of(e) {
        Some(Refusal::Link) => format!("{e}. Delete the link, then try again."),
        Some(Refusal::NotPlainFile | Refusal::SecondName) => format!("{e}. Delete it, then try again."),
        None => {
            let done = if verb == "write" { "written" } else { "read" };
            format!("{} can't be {done}: {e}. Check its permissions and free space, then try again.", path.display())
        }
    }
}

/// Whether `meta` is a file that has another name besides this one (a hard link).
#[cfg(unix)]
fn has_second_name(meta: &Metadata) -> bool {
    std::os::unix::fs::MetadataExt::nlink(meta) > 1
}

/// Whether `meta` is a file that has another name besides this one: not asked on a platform without link counts.
#[cfg(not(unix))]
fn has_second_name(_: &Metadata) -> bool {
    false
}

/// The metadata of the plain file with one name at `path`, without following a link. Used to learn a size or a modification
/// time; reading or writing the file opens it again and checks it again.
pub fn stat(path: &Path) -> io::Result<Metadata> {
    let meta = std::fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() {
        return Err(refused(Refusal::Link, path));
    }
    if !meta.file_type().is_file() {
        return Err(refused(Refusal::NotPlainFile, path));
    }
    if has_second_name(&meta) {
        return Err(refused(Refusal::SecondName, path));
    }
    Ok(meta)
}

/// Deletes the session file at `path`; one that is not there is already gone. Removing follows no link: a planted link is
/// unlinked itself and nothing it points at is touched.
pub fn remove(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// The text of the plain file at `path`; a file that is not there is a `NotFound` error.
pub fn read_to_string(path: &Path) -> io::Result<String> {
    String::from_utf8(read(path)?).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "stream did not contain valid UTF-8"))
}

/// A name for a temporary file beside `name`: hidden, with 64 random bits (the keys of `RandomState` come from the operating
/// system's random generator), so no one can plant a link there beforehand.
fn temp_name(name: &OsStr) -> OsString {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
    hasher.write_u32(std::process::id());
    let mut temp = OsString::from(".");
    temp.push(name);
    temp.push(format!(".{:016x}.tmp", hasher.finish()));
    temp
}

fn no_unused_temp_name(path: &Path) -> io::Error {
    io::Error::new(io::ErrorKind::AlreadyExists, format!("no unused temporary name beside {}", path.display()))
}
