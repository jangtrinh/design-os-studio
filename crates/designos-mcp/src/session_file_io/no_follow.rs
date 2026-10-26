//! The session-file calls on a platform with `O_NOFOLLOW`: the flags are the guard, and the opened file is checked, not the path.
use super::{has_second_name, no_unused_temp_name, refused, temp_name, Refusal};
use std::fs::{File, Metadata, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

/// A link at the last component is an error instead of being followed; a FIFO opens at once instead of waiting for the other end.
const NO_LINK_NO_WAIT: i32 = libc::O_NOFOLLOW | libc::O_NONBLOCK;
const TEMP_TRIES: usize = 8;
const OWNER_ONLY: u32 = 0o600;

/// What the failed open or rename of `path` says about it: a refusal when `e` means a link (under `O_NOFOLLOW`, `ELOOP`, or
/// `EMLINK` on the BSDs, whatever the entry has turned into since), a FIFO nobody reads or a socket, or a folder.
pub(super) fn open_failure(path: &Path, e: io::Error) -> io::Error {
    match e.raw_os_error() {
        Some(libc::ELOOP | libc::EMLINK) => refused(Refusal::Link, path),
        Some(libc::ENXIO | libc::EISDIR | libc::EOPNOTSUPP) => refused(Refusal::NotPlainFile, path),
        _ => e,
    }
}

/// Back to blocking mode: `O_NONBLOCK` only kept the open from waiting, and the file is known to be a plain one.
fn blocking(file: &File) -> io::Result<()> {
    let fd = file.as_raw_fd();
    // SAFETY: `fd` belongs to `file`, which stays open across both calls; F_GETFL and F_SETFL only read and set its status flags.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    // SAFETY: as above.
    if flags == -1 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags & !libc::O_NONBLOCK) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Opens `path` and refuses what is not a plain file with one name: a link planted in the folder is the same attack as a
/// second name for a file somewhere else, and a read through either hands the agent's choice of file to Studio.
pub(super) fn open_plain(path: &Path, options: &mut OpenOptions) -> io::Result<(File, Metadata)> {
    let file = options.custom_flags(NO_LINK_NO_WAIT).open(path).map_err(|e| open_failure(path, e))?;
    let meta = file.metadata()?;
    if !meta.file_type().is_file() {
        return Err(refused(Refusal::NotPlainFile, path));
    }
    if has_second_name(&meta) {
        return Err(refused(Refusal::SecondName, path));
    }
    blocking(&file)?;
    Ok((file, meta))
}

fn finish(file: &File, durable: bool) -> io::Result<()> {
    if durable {
        file.sync_all()
    } else {
        Ok(())
    }
}

pub fn read(path: &Path) -> io::Result<Vec<u8>> {
    let (mut file, meta) = open_plain(path, OpenOptions::new().read(true))?;
    let mut bytes = Vec::with_capacity(usize::try_from(meta.len()).unwrap_or(0));
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

pub fn append(path: &Path, bytes: &[u8], durable: bool) -> io::Result<()> {
    let (mut file, _) = open_plain(path, OpenOptions::new().append(true).create(true).mode(OWNER_ONLY))?;
    file.write_all(bytes)?;
    finish(&file, durable)
}

/// Empties the file only after the opened file proved to be plain, so the open itself never truncates (`O_TRUNC` would, on a
/// file that turns out not to be ours).
pub fn truncate(path: &Path, durable: bool) -> io::Result<()> {
    let (file, _) = match open_plain(path, OpenOptions::new().write(true)) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        opened => opened?,
    };
    file.set_len(0)?;
    finish(&file, durable)
}

pub fn write_atomic(path: &Path, bytes: &[u8], durable: bool) -> io::Result<()> {
    let name = path.file_name().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("{} has no file name", path.display())))?;
    let folder = path.parent().unwrap_or_else(|| Path::new(""));
    for _ in 0..TEMP_TRIES {
        let temp = folder.join(temp_name(name));
        let mut file = match OpenOptions::new().write(true).create_new(true).mode(OWNER_ONLY).custom_flags(libc::O_NOFOLLOW).open(&temp) {
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            opened => opened?,
        };
        // A folder at `path` makes the rename fail (`EISDIR`): that is a refusal, not an unexplained error.
        let written =
            file.write_all(bytes).and_then(|()| finish(&file, durable)).and_then(|()| std::fs::rename(&temp, path).map_err(|e| open_failure(path, e)));
        if written.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        return written;
    }
    Err(no_unused_temp_name(path))
}
