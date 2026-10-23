//! Clearing what a killed Codex app-server leaves behind (#930).
//!
//! `codex app-server --listen unix://<rendezvous>` makes the rendezvous path a symlink to a socket named by the hash of
//! that path in `/private/tmp/codex-daemon-<uid>/`, next to a `<hash>.lock`. A server that is killed leaves all three.
//! Codex would rebind over them, but clearing them first keeps two live servers from sharing a path and answers
//! "does anything still serve here" before a tab commits to the card path.

use super::FallbackReason;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

/// [`clear_stale_rendezvous_in`] with the daemon folder of the user who owns the rendezvous folder.
pub fn clear_stale_rendezvous(rendezvous: &Path) -> Result<(), FallbackReason> {
    let daemon_dir =
        rendezvous.parent().and_then(|dir| std::fs::metadata(dir).ok()).map(|meta| PathBuf::from(format!("/private/tmp/codex-daemon-{}", meta.uid())));
    clear_stale_rendezvous_in(rendezvous, daemon_dir.as_deref())
}

/// Removes the leftovers of a server that no longer answers on `rendezvous`.
/// - Nothing there: nothing to do.
/// - Anything that is not a link to a Codex socket (a plain file, a link elsewhere, a name that is not a 64-digit hex
///   hash, a target outside `daemon_dir`): only `rendezvous` itself goes. What it pointed to is never touched.
/// - A link to a socket in `daemon_dir`: if the socket answers, nothing is removed and the result is `SocketInUse`;
///   otherwise the socket, its `.lock` and the link go.
pub fn clear_stale_rendezvous_in(rendezvous: &Path, daemon_dir: Option<&Path>) -> Result<(), FallbackReason> {
    let meta = match std::fs::symlink_metadata(rendezvous) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(FallbackReason::SocketInUse),
    };
    if !meta.file_type().is_symlink() {
        if meta.file_type().is_socket() && UnixStream::connect(rendezvous).is_ok() {
            return Err(FallbackReason::SocketInUse);
        }
        return std::fs::remove_file(rendezvous).map_err(|_| FallbackReason::SocketInUse);
    }
    if let Some(target) = std::fs::read_link(rendezvous).ok().filter(|target| leads_to_codex_socket(target, daemon_dir)) {
        if UnixStream::connect(&target).is_ok() {
            return Err(FallbackReason::SocketInUse);
        }
        if std::fs::symlink_metadata(&target).is_ok_and(|meta| meta.file_type().is_socket()) {
            let _ = std::fs::remove_file(&target);
        }
        let mut lock = target.into_os_string();
        lock.push(".lock");
        let _ = std::fs::remove_file(lock);
    }
    std::fs::remove_file(rendezvous).map_err(|_| FallbackReason::SocketInUse)
}

/// The target lies directly in `daemon_dir` (compared after resolving both) and is named by a 64-digit lowercase hex hash.
fn leads_to_codex_socket(target: &Path, daemon_dir: Option<&Path>) -> bool {
    let named_by_hash =
        target.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.len() == 64 && name.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')));
    let parent = target.parent().and_then(|dir| dir.canonicalize().ok());
    named_by_hash && parent.is_some() && parent == daemon_dir.and_then(|dir| dir.canonicalize().ok())
}
