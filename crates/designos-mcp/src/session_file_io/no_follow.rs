//! The session-file calls on a platform with `O_NOFOLLOW`: the flags are the guard, and the opened file is checked, not the path.
use super::{no_unused_temp_name, refused, temp_name, Refusal};
use std::fs::{File, Metadata, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

/// A link at the last component is an error instead of being followed; a FIFO opens at once instead of waiting for the other end.
const NO_LINK_NO_WAIT: i32 = libc::O_NOFOLLOW | libc::O_NONBLOCK;
const TEMP_TRIES: usize = 8;
const OWNER_ONLY: u32 = 0o600;

/// What the failed open of `path` says about it: a refusal when `e` means a link, a FIFO nobody reads, or a folder.
pub(super) fn open_failure(path: &Path, e: io::Error) -> io::Error {
    match e.raw_os_error() {
        Some(libc::ELOOP | libc::EMLINK) if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) => refused(Refusal::Link, path),
        Some(libc::ENXIO | libc::EISDIR) => refused(Refusal::NotPlainFile, path),
        _ => e,
    }
}

/// Opens `path` and refuses what is not a plain file. A file that is written through must also have no other name: a hard link
/// planted in the folder is the same attack as a symbolic one.
pub(super) fn open_plain(path: &Path, options: &mut OpenOptions, writes: bool) -> io::Result<(File, Metadata)> {
    let file = options.custom_flags(NO_LINK_NO_WAIT).open(path).map_err(|e| open_failure(path, e))?;
    let meta = file.metadata()?;
    if !meta.file_type().is_file() {
        return Err(refused(Refusal::NotPlainFile, path));
    }
    if writes && meta.nlink() > 1 {
        return Err(refused(Refusal::Link, path));
    }
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
    let (mut file, meta) = open_plain(path, OpenOptions::new().read(true), false)?;
    let mut bytes = Vec::with_capacity(usize::try_from(meta.len()).unwrap_or(0));
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

pub fn append(path: &Path, bytes: &[u8], durable: bool) -> io::Result<()> {
    let (mut file, _) = open_plain(path, OpenOptions::new().append(true).create(true).mode(OWNER_ONLY), true)?;
    file.write_all(bytes)?;
    finish(&file, durable)
}

/// Empties the file only after the opened file proved to be plain, so the open itself never truncates (`O_TRUNC` would, on a
/// file that turns out not to be ours).
pub fn truncate(path: &Path, durable: bool) -> io::Result<()> {
    let (file, _) = match open_plain(path, OpenOptions::new().write(true), true) {
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
        let written = file.write_all(bytes).and_then(|()| finish(&file, durable)).and_then(|()| std::fs::rename(&temp, path));
        if written.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        return written;
    }
    Err(no_unused_temp_name(path))
}
