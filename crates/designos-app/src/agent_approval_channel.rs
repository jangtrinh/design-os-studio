//! The Studio-private folder behind Ask-mode approvals.
//!
//! Claude's `PermissionRequest` hook raises a request and waits for Studio's answer. Whatever can write that answer,
//! or the hook script that decides what to print, approves the agent's next action. So none of it may sit where an
//! agent can write: not in its working folder (the session folder), not in an `--add-dir`, and not in `/tmp` or
//! `$TMPDIR` (the writable roots of Codex's `workspace-write` sandbox).
//!
//! One folder per session, named by a hash of the session folder's path so a restarted Studio finds it again:
//!
//! ```text
//! <data dir>/designos/ap/        0700  Studio-private root (macOS: ~/Library/Application Support/designos/ap)
//!   <sid>/                       0700  16 hex digits: FNV-1a 64 of the session folder's path
//!     approve-hook.sh            0700  the hook; everything it decides with is in this folder or in the script itself
//!     tab-<id>/                  0700  `<request id>.request.json` and `<request id>.answer` of one tab
//! ```
//!
//! The path is short on purpose: the Codex app-server socket is meant to sit next to `approve-hook.sh`, and a unix
//! socket path is limited to 104 bytes on macOS. The folder is not secret (the agent can read `hooks.json`); the
//! protection is that no agent write grant reaches it. A process that runs unsandboxed as the same user can already
//! rewrite the agent's own settings, so no folder or socket stops that case.

use std::io;
use std::path::{Path, PathBuf};

/// Name of the private root under Studio's data folder.
const ROOT_DIR_NAME: &str = "ap";

/// File name of the approval hook inside a session's channel folder.
pub const HOOK_SCRIPT_NAME: &str = "approve-hook.sh";

/// Folders any agent can write without a prompt: `/tmp` and `$TMPDIR` (Codex's sandbox grants both), in the spelling
/// given and as the file system resolves it (macOS: `/tmp` is `/private/tmp`).
fn temp_roots() -> Vec<PathBuf> {
    let mut roots = vec![std::env::temp_dir(), PathBuf::from("/tmp"), PathBuf::from("/var/tmp")];
    let resolved: Vec<PathBuf> = roots.iter().filter_map(|root| root.canonicalize().ok()).collect();
    roots.extend(resolved);
    roots
}

/// `path` with its longest existing ancestor resolved through the file system (symlinks, `/private` on macOS).
fn resolve_existing_prefix(path: &Path) -> PathBuf {
    let mut tail = Vec::new();
    let mut current = path;
    loop {
        if let Ok(real) = current.canonicalize() {
            return tail.iter().rev().fold(real, |acc, part| acc.join(part));
        }
        match (current.parent(), current.file_name()) {
            (Some(parent), Some(name)) => {
                tail.push(name);
                current = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// `<data dir>/designos/ap`, the production location of every channel folder, or `None` when the platform has no
/// data folder or it lies inside a temp folder an agent can write. `None` means no approval hook is installed and
/// Claude asks in its own terminal prompt, which fails closed. Never falls back to `.` or a temp folder.
pub fn default_private_root() -> Option<PathBuf> {
    let root = designos_library::app_data_dir()?.join(ROOT_DIR_NAME);
    let resolved = resolve_existing_prefix(&root);
    let agent_writable = temp_roots().iter().any(|temp| root.starts_with(temp) || resolved.starts_with(temp));
    (!agent_writable).then_some(root)
}

/// The private root in force. Unit tests get a root of their own under the temp folder, so they never write into the
/// real data folder; [`default_private_root`] is what the tests of the production location check.
pub fn private_root() -> Option<PathBuf> {
    #[cfg(test)]
    {
        Some(std::env::temp_dir().join("designos-approval-channels-test"))
    }
    #[cfg(not(test))]
    {
        static ROOT: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
        ROOT.get_or_init(default_private_root).clone()
    }
}

/// Short, stable name of the channel folder for `session_dir`: 16 hex digits of the FNV-1a 64 hash of its path.
pub fn session_id(session_dir: &Path) -> String {
    format!("{:016x}", crate::agent_sessions::fnv1a_64(session_dir.as_os_str().as_encoded_bytes()))
}

/// `<root>/<sid>` for `session_dir`; `None` when that would land inside the session folder itself.
pub fn channel_dir_in(root: &Path, session_dir: &Path) -> Option<PathBuf> {
    let dir = root.join(session_id(session_dir));
    (!dir.starts_with(session_dir)).then_some(dir)
}

/// The private channel folder of `session_dir`, or `None` without a private root.
pub fn channel_dir(session_dir: &Path) -> Option<PathBuf> {
    channel_dir_in(&private_root()?, session_dir)
}

/// `<channel>/approve-hook.sh` of `session_dir`.
pub fn hook_script_path(session_dir: &Path) -> Option<PathBuf> {
    channel_dir(session_dir).map(|dir| dir.join(HOOK_SCRIPT_NAME))
}

/// `<channel>/tab-<tab_id>`, where the hook of that tab raises requests and Studio answers them.
pub fn tab_approvals_dir(session_dir: &Path, tab_id: usize) -> Option<PathBuf> {
    channel_dir(session_dir).map(|dir| dir.join(format!("tab-{tab_id}")))
}

/// `create_dir_all` with mode 0700 for every folder it creates (Unix; elsewhere the per-user default applies).
pub fn create_private_dir_all(path: &Path) -> io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}

/// Requires `path` to be a real folder (not a link) that only its owner can use, tightening a looser mode.
fn require_private_dir(path: &Path) -> io::Result<()> {
    let meta = std::fs::symlink_metadata(path)?;
    if !meta.is_dir() {
        return Err(io::Error::other(format!("{} is not a folder", path.display())));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    Ok(())
}

/// Creates the private root and `session_dir`'s channel folder under `root`, both owner-only, and returns the latter.
pub fn prepare_channel_dir_in(root: &Path, session_dir: &Path) -> io::Result<PathBuf> {
    let dir = channel_dir_in(root, session_dir).ok_or_else(|| io::Error::other("the approval channel would sit inside the session folder"))?;
    create_private_dir_all(root)?;
    require_private_dir(root)?;
    create_private_dir_all(&dir)?;
    require_private_dir(&dir)?;
    Ok(dir)
}

/// [`prepare_channel_dir_in`] under the private root; an error when there is none.
pub fn prepare_channel_dir(session_dir: &Path) -> io::Result<PathBuf> {
    let root = private_root().ok_or_else(|| io::Error::other("no Studio-private data folder outside the folders agents can write"))?;
    prepare_channel_dir_in(&root, session_dir)
}

/// Removes the whole channel folder of `session_dir` (script, requests, answers). Called when its last tab closes and
/// when the session ends; a missing folder is fine.
pub fn remove_channel(session_dir: &Path) {
    if let Some(dir) = channel_dir(session_dir) {
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// Removes one tab's request/answer folder: a tab that closes, or a new tab that must not inherit the requests of an
/// earlier run with the same id.
pub fn remove_tab_dir(session_dir: &Path, tab_id: usize) {
    if let Some(dir) = tab_approvals_dir(session_dir, tab_id) {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(tag: &str) -> PathBuf {
        std::env::temp_dir().join("designos-sessions").join(format!("channel-{tag}-{}", std::process::id()))
    }

    /// The production root is neither inside the session folder, nor `/tmp`, nor `$TMPDIR`, and is short enough for
    /// the Codex socket that goes next to the hook script.
    #[test]
    fn the_production_root_sits_outside_every_agent_writable_root() {
        let Some(root) = default_private_root() else {
            // Only a data folder inside a temp folder (a test harness with HOME under /tmp) gets here.
            let data = designos_library::app_data_dir().expect("a platform without a data folder has no root");
            assert!(temp_roots().iter().any(|temp| data.starts_with(temp) || resolve_existing_prefix(&data).starts_with(temp)));
            return;
        };
        let session_dir = session("prod");
        let channel = channel_dir_in(&root, &session_dir).expect("a channel folder");
        let script = channel.join(HOOK_SCRIPT_NAME);
        let tab = channel.join("tab-12");
        let mut roots = temp_roots();
        roots.push(session_dir.clone());
        for path in [&channel, &script, &tab] {
            let resolved = resolve_existing_prefix(path);
            for forbidden in &roots {
                assert!(!path.starts_with(forbidden) && !resolved.starts_with(forbidden), "{} is inside {}", path.display(), forbidden.display());
            }
        }
        assert!(root.ends_with(Path::new("designos").join(ROOT_DIR_NAME)), "{}", root.display());
        // macOS limits a unix socket path to 104 bytes (NUL included). Studio's own part below `<data dir>/designos` stays
        // at 30 bytes, and the whole path fits for the user running the test.
        let socket = channel.join("codex.sock");
        let below_app_dir = socket.strip_prefix(root.parent().expect("the app data folder")).unwrap();
        assert!(below_app_dir.as_os_str().len() <= 30, "{} is {} bytes", below_app_dir.display(), below_app_dir.as_os_str().len());
        assert!(socket.as_os_str().len() <= 103, "{} is {} bytes", socket.display(), socket.as_os_str().len());
    }

    #[test]
    fn the_channel_name_is_short_stable_and_per_session() {
        let a = session("a");
        let b = session("b");
        assert_eq!(session_id(&a), session_id(&a));
        assert_ne!(session_id(&a), session_id(&b));
        let sid = session_id(&a);
        assert_eq!(sid.len(), 16);
        assert!(sid.bytes().all(|c| c.is_ascii_hexdigit()));
        // The name is part of an on-disk path: the hash is pinned so an upgrade keeps finding the folder.
        assert_eq!(session_id(Path::new("/x")), format!("{:016x}", crate::agent_sessions::fnv1a_64(b"/x")));
    }

    #[test]
    fn a_root_inside_the_session_folder_gets_no_channel() {
        let session_dir = session("inside");
        assert_eq!(channel_dir_in(&session_dir.join("ap"), &session_dir), None);
        assert!(prepare_channel_dir_in(&session_dir.join("ap"), &session_dir).is_err());
        assert!(!session_dir.exists(), "nothing is created for a refused channel");
    }

    #[cfg(unix)]
    #[test]
    fn prepared_folders_are_owner_only_even_when_they_existed_with_a_looser_mode() {
        use std::os::unix::fs::PermissionsExt;
        let base = std::env::temp_dir().join(format!("designos-channel-modes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("designos").join("ap");
        let session_dir = session("modes");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
        let channel = prepare_channel_dir_in(&root, &session_dir).unwrap();
        for dir in [&root, &channel] {
            let mode = std::fs::metadata(dir).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o700, "{} must be 0700", dir.display());
        }
        // A tab folder made through the helper is private too, parents included.
        let tab = channel.join("tab-3");
        create_private_dir_all(&tab).unwrap();
        assert_eq!(std::fs::metadata(&tab).unwrap().permissions().mode() & 0o777, 0o700);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[cfg(unix)]
    #[test]
    fn a_link_in_place_of_the_root_is_refused() {
        let base = std::env::temp_dir().join(format!("designos-channel-link-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let elsewhere = base.join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        let root = base.join("ap");
        std::os::unix::fs::symlink(&elsewhere, &root).unwrap();
        let err = prepare_channel_dir_in(&root, &session("link")).expect_err("a link must not serve as the private root");
        assert!(err.to_string().contains("not a folder"), "{err}");
        let _ = std::fs::remove_dir_all(&base);
    }
}
