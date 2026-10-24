//! Whose a session folder is: the id of the user Studio runs as, and the check of a folder's owner against it.

use super::refusal::{refused, Refusal};
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
    Err(refused(Refusal::OtherOwner, dir))
}

/// The id of the user Studio runs as, read once from a file Studio creates in the folder `probe_dir` names: a new file is
/// owned by the creating user, and `create_new` never follows a link. The caller names Studio's own sessions folder, so the
/// id comes from the volume that holds the user's data, not from a library on a drive that may not keep ownership.
pub(super) fn own_uid(probe_dir: impl FnOnce() -> io::Result<PathBuf>) -> io::Result<u32> {
    static UID: OnceLock<u32> = OnceLock::new();
    if let Some(uid) = UID.get() {
        return Ok(*uid);
    }
    let uid = probe_uid(&probe_dir()?)?;
    Ok(*UID.get_or_init(|| uid))
}

/// The id of the user who owns a file just created in `dir`.
pub(super) fn probe_uid(dir: &Path) -> io::Result<u32> {
    use std::os::unix::fs::MetadataExt;
    static SEQ: AtomicU64 = AtomicU64::new(0);
    for _ in 0..4 {
        let probe = dir.join(format!(".owner-probe-{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)));
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&probe) {
            Ok(file) => {
                let uid = file.metadata().map(|m| m.uid());
                let _ = std::fs::remove_file(&probe);
                return uid;
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                let message = format!("a file can't be created in {}: {e}. Check its permissions and free space, then try again", dir.display());
                return Err(io::Error::new(e.kind(), message));
            }
        }
    }
    Err(io::Error::other(format!("could not tell which user owns {}. Delete the .owner-probe files in it and try again", dir.display())))
}
