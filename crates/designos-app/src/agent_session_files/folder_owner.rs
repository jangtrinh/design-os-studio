//! Whose a session folder is: the id of the user Studio runs as, and the check of a folder's owner against it.

use super::folder::refused;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

/// macOS shows user id 99 for every object on a volume mounted with "ignore ownership" and means the current user by it
/// (`mount(8)`), so a library on such a drive keeps its session folders.
const OWNERSHIP_IGNORED_UID: u32 = 99;

/// Accepts a folder whose owner is `own_uid`; `owner_uid` is the owner the folder reports.
pub(super) fn check_owner(dir: &Path, owner_uid: u32, own_uid: u32) -> io::Result<()> {
    if owner_uid == own_uid || (cfg!(target_os = "macos") && owner_uid == OWNERSHIP_IGNORED_UID) {
        return Ok(());
    }
    Err(refused(format!("{} belongs to another user. Move or delete it and try again", dir.display())))
}

/// The id of the user Studio runs as, read once from a file Studio creates in the folder `probe_dir` names: a new file is
/// owned by the creating user, and `create_new` never follows a link. The caller names Studio's own sessions folder, so the
/// id comes from the volume that holds the user's data, not from a library on a drive that may not keep ownership.
pub(super) fn own_uid(probe_dir: impl FnOnce() -> io::Result<PathBuf>) -> io::Result<u32> {
    use std::os::unix::fs::MetadataExt;
    static UID: OnceLock<u32> = OnceLock::new();
    static SEQ: AtomicU64 = AtomicU64::new(0);
    if let Some(uid) = UID.get() {
        return Ok(*uid);
    }
    let dir = probe_dir()?;
    for _ in 0..4 {
        let probe = dir.join(format!(".owner-probe-{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)));
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&probe) {
            Ok(file) => {
                let uid = file.metadata().map(|m| m.uid());
                let _ = std::fs::remove_file(&probe);
                return uid.map(|uid| *UID.get_or_init(|| uid));
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::other("could not tell which user owns the session folders"))
}
