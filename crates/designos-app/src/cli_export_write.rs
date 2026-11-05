//! Where a headless export (`--export-fig`) may write. An agent session can reach the command, so the path it names
//! is not trusted: it must carry the format's extension, and it must be a new file or a plain file the caller asked
//! to replace with `--overwrite`. A symbolic link is never written through, dangling or not, so a link cannot steer
//! an export onto another file.
//!
//! Replacing installs a new file next to the old one and renames it over, so the old file's ACLs, extended
//! attributes, ownership and hard links are not carried over (the permission mode is).

use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Distinguishes the temporary files of exports started by one process.
static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Refuses `path` unless its extension is `ext`, in any letter case.
fn require_extension(path: &Path, ext: &str) -> Result<(), String> {
    if path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case(ext)) {
        return Ok(());
    }
    Err(format!("Refusing to write {}: the output path must end in .{ext}. Name it with a .{ext} extension.", path.display()))
}

fn exists_message(path: &Path) -> String {
    format!("Refusing to replace {}: it already exists. Pass --overwrite to replace it, or choose a new path.", path.display())
}

/// What stands at `path` right now, looked at without following a symbolic link. `Ok(None)` means nothing is there,
/// so a new file may be created; `Ok(Some(_))` is a plain file the caller allowed to be replaced.
fn inspect_target(path: &Path, overwrite: bool) -> Result<Option<Metadata>, String> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("Cannot inspect {}: {e}", path.display())),
    };
    if meta.file_type().is_symlink() {
        return Err(format!("Refusing to write {}: it is a symbolic link. Export to a path that is not a link.", path.display()));
    }
    if !meta.is_file() {
        return Err(format!("Refusing to write {}: it is not a regular file.", path.display()));
    }
    if !overwrite {
        return Err(exists_message(path));
    }
    // A rename needs only write access to the folder, so a file its owner made read-only would be replaced where a plain
    // write is refused. The workspace's other file writers refuse it, so this one does too.
    if meta.permissions().readonly() {
        return Err(format!("Refusing to replace {}: it is read-only. Make it writable first, or choose a new path.", path.display()));
    }
    Ok(Some(meta))
}

/// Checks `path` the way [`write_export`] will, without touching it, so a command that also changes the document can
/// stop before it changes anything.
pub fn check_target(path: &Path, ext: &str, overwrite: bool) -> Result<(), String> {
    require_extension(path, ext)?;
    inspect_target(path, overwrite).map(|_| ())
}

/// Writes `bytes` to `path`, creating missing folders. See the module comment for what is refused. A failed write
/// leaves no partial file behind.
pub fn write_export(path: &Path, ext: &str, bytes: &[u8], overwrite: bool) -> Result<(), String> {
    require_extension(path, ext)?;
    let existing = inspect_target(path, overwrite)?;
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|e| format!("Cannot create {}: {e}", parent.display()))?;
    }
    match &existing {
        None => create_new_file(path, bytes),
        Some(old) => replace_file(path, bytes, old).map_err(|e| write_error(path, &e)),
    }
}

/// Writes a generated JSON export without following a path planted by another process.
/// Spec exports replace the previous file, matching the old `std::fs::write` behavior.
pub(crate) fn write_json_export(path: &Path, json: &str) -> Result<(), String> {
    write_export(path, "json", json.as_bytes(), true)
}

fn write_error(path: &Path, e: &io::Error) -> String {
    format!("Cannot write {}: {e}", path.display())
}

/// `create_new` is `O_CREAT | O_EXCL`: it fails on anything already at `path`, a symbolic link included, so a link
/// planted after the check is not followed.
fn create_new_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path).map_err(|e| {
        if e.kind() == ErrorKind::AlreadyExists {
            exists_message(path)
        } else {
            write_error(path, &e)
        }
    })?;
    if let Err(e) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(write_error(path, &e));
    }
    Ok(())
}

/// Writes `bytes` to a new file next to `path` and renames it over `path`. The rename replaces whatever entry is at
/// `path` and does not follow it, so a link swapped in after the check is replaced, never written through.
fn replace_file(path: &Path, bytes: &[u8], old: &Metadata) -> io::Result<()> {
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let (tmp, mut file) = create_temp_beside(dir)?;
    let installed = (|| {
        file.set_permissions(old.permissions())?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, path)
    })();
    if installed.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    installed
}

/// A new, empty, owner-only file in `dir`. Its name is short and independent of the export's name, so a file name
/// near the file system limit still has a temporary sibling.
fn create_temp_beside(dir: &Path) -> io::Result<(PathBuf, File)> {
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    for _ in 0..16 {
        let tmp = dir.join(format!(".designos-export-{}-{}.tmp", std::process::id(), TMP_SEQ.fetch_add(1, Ordering::Relaxed)));
        match opts.open(&tmp) {
            Ok(file) => return Ok((tmp, file)),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(ErrorKind::AlreadyExists, "no free temporary file name"))
}
