//! The session-file calls on a platform without `O_NOFOLLOW`: every step checks the entry and then acts on its path, so a link
//! swapped in between the check and the open is not caught here. A link or a FIFO that is already in place is refused.
use super::{no_unused_temp_name, stat, temp_name};
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::Path;

const TEMP_TRIES: usize = 8;

/// Refuses an entry that is not a plain file; one that is not there yet is fine when `missing_is_fine`.
fn check(path: &Path, missing_is_fine: bool) -> io::Result<()> {
    match stat(path) {
        Err(e) if missing_is_fine && e.kind() == io::ErrorKind::NotFound => Ok(()),
        checked => checked.map(|_| ()),
    }
}

fn finish(file: &std::fs::File, durable: bool) -> io::Result<()> {
    if durable {
        file.sync_all()
    } else {
        Ok(())
    }
}

pub fn read(path: &Path) -> io::Result<Vec<u8>> {
    check(path, false)?;
    std::fs::read(path)
}

pub fn append(path: &Path, bytes: &[u8], durable: bool) -> io::Result<()> {
    check(path, true)?;
    let mut file = OpenOptions::new().append(true).create(true).open(path)?;
    file.write_all(bytes)?;
    finish(&file, durable)
}

pub fn truncate(path: &Path, durable: bool) -> io::Result<()> {
    match check(path, false) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        checked => checked?,
    }
    let file = OpenOptions::new().write(true).truncate(true).open(path)?;
    finish(&file, durable)
}

pub fn write_atomic(path: &Path, bytes: &[u8], durable: bool) -> io::Result<()> {
    let name = path.file_name().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("{} has no file name", path.display())))?;
    let folder = path.parent().unwrap_or_else(|| Path::new(""));
    for _ in 0..TEMP_TRIES {
        let temp = folder.join(temp_name(name));
        let mut file = match OpenOptions::new().write(true).create_new(true).open(&temp) {
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
