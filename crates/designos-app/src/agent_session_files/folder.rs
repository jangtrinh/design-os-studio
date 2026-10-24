//! Where agent session folders live, and how Studio makes them.
//!
//! A session folder is the working folder of an agent, which a sandboxed agent may write without asking. If an agent
//! could choose what that folder is, it could point another tab's folder at any folder the user owns. Two rules close that:
//!
//! 1. Ephemeral sessions (unsaved files, window fallbacks) live in `<data dir>/designos/sessions/p<pid>-tab-<n>`, beside
//!    the approval channel in `<data dir>/designos/ap`. They do not live in `$TMPDIR`: Codex's `workspace-write` sandbox
//!    can write it, and the names are predictable, so an agent could plant a link at the name a later tab will get.
//! 2. Studio makes each session folder itself, with an exclusive `mkdir` (mode 0700). A path that exists already is
//!    accepted only when it is a real folder (not a link) owned by the user. Anything else is an error, and nothing is
//!    written there.

#[cfg(unix)]
use super::folder_owner::{check_owner, own_uid};
use super::refusal::{refused, Refusal};
use std::io;
use std::path::{Path, PathBuf};

/// Folder name of the ephemeral sessions under Studio's data folder.
const SESSIONS_DIR_NAME: &str = "sessions";

/// Parent of a session path when Studio has no private data folder: [`create_session_folder`] refuses everything below
/// it, never falling back to a folder an agent can write. On Unix nothing can be made below it (`/dev/null` is a file).
#[cfg(unix)]
pub(super) const UNAVAILABLE_ROOT: &str = "/dev/null/designos-no-private-data-folder";
#[cfg(not(unix))]
pub(super) const UNAVAILABLE_ROOT: &str = "designos-no-private-data-folder";

/// `e` with the folder it happened at and the next step, for the line a tab shows when it cannot start.
fn cannot_make(dir: &Path, e: io::Error) -> io::Error {
    io::Error::new(e.kind(), format!("{} can't be created: {e}. Check its permissions and free space, then try again", dir.display()))
}

/// `<data dir>/designos/sessions` of this machine, or `None` when the approval channel has no private root either (no
/// data folder, or one inside a folder an agent can write).
pub fn default_sessions_root() -> Option<PathBuf> {
    sessions_root_beside(&crate::agent_approval_channel::default_private_root()?)
}

/// The sessions folder that sits next to the approval channel root `<data dir>/designos/ap`.
pub fn sessions_root_beside(private_root: &Path) -> Option<PathBuf> {
    Some(private_root.parent()?.join(SESSIONS_DIR_NAME))
}

/// The folder every ephemeral session folder is a direct child of. Each unit-test process gets a root of its own under the
/// temp folder (see `test_root`), so tests never write into the real data folder; [`default_sessions_root`] is what the
/// tests of the production location check.
pub fn ephemeral_sessions_root() -> Option<PathBuf> {
    #[cfg(test)]
    {
        Some(super::test_root::test_sessions_root())
    }
    #[cfg(not(test))]
    {
        static ROOT: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
        ROOT.get_or_init(default_sessions_root).clone()
    }
}

/// The ephemeral session folder for `session_id`: always a direct child of [`ephemeral_sessions_root`], whatever the id
/// holds. Without a private data folder the path is one [`create_session_folder`] refuses.
pub fn ephemeral_session_dir(session_id: &str) -> PathBuf {
    let name: String = session_id.chars().map(|c| if matches!(c, '/' | '\\') { '_' } else { c }).collect();
    let name = if name.is_empty() || name == "." || name == ".." { "_".to_string() } else { name };
    ephemeral_sessions_root().unwrap_or_else(|| PathBuf::from(UNAVAILABLE_ROOT)).join(name)
}

/// Makes the session folder `dir` the way rule 2 says; a re-sync of a folder Studio made earlier succeeds. Parent folders
/// are made owner-only, and the sessions folder of an ephemeral session must be a real folder of the user.
pub(super) fn create_session_folder(dir: &Path) -> io::Result<()> {
    create_session_folder_in(ephemeral_sessions_root().as_deref(), dir)
}

pub(super) fn create_session_folder_in(ephemeral_root: Option<&Path>, dir: &Path) -> io::Result<()> {
    if dir.starts_with(UNAVAILABLE_ROOT) {
        return Err(io::Error::other(
            "Studio has no private data folder for agent sessions (its user data folder is missing or inside a temporary folder). Make sure your home folder exists, then restart Studio",
        ));
    }
    let parent = dir.parent().ok_or_else(|| io::Error::other(format!("{} has no parent folder", dir.display())))?;
    match std::fs::symlink_metadata(dir) {
        Ok(_) => return require_own_real_folder(ephemeral_root, dir, parent).map(|_| ()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(cannot_make(dir, e)),
    }
    if ephemeral_root == Some(parent) {
        make_private_root(parent)?;
    } else {
        crate::agent_approval_channel::create_private_dir_all(parent).map_err(|e| cannot_make(parent, e))?;
    }
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    match builder.create(dir) {
        Ok(()) => Ok(()),
        // Someone made it between the check and the `mkdir`: it must still be ours and a real folder.
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => require_own_real_folder(ephemeral_root, dir, parent).map(|_| ()),
        Err(e) => Err(cannot_make(dir, e)),
    }
}

/// Creates the sessions folder (and what is above it) owner-only and requires it to be a real folder of the user, with
/// no access for anyone else.
fn make_private_root(root: &Path) -> io::Result<()> {
    crate::agent_approval_channel::create_private_dir_all(root).map_err(|e| cannot_make(root, e))?;
    let meta = require_own_real_folder(Some(root), root, root)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    #[cfg(not(unix))]
    let _ = meta;
    Ok(())
}

/// Requires `dir` to be a real folder, not a link, that the user owns. The user's id is read in the sessions folder
/// (`ephemeral_root`), and only without one next to the folder checked (`near`).
pub(super) fn require_own_real_folder(ephemeral_root: Option<&Path>, dir: &Path, near: &Path) -> io::Result<std::fs::Metadata> {
    let meta = std::fs::symlink_metadata(dir)?;
    check_real_folder(dir, &meta)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        check_owner(dir, meta.uid(), own_uid(|| probe_dir_for(ephemeral_root, near))?)?;
    }
    #[cfg(not(unix))]
    let _ = (ephemeral_root, near);
    Ok(meta)
}

/// The folder the user's id is read in: Studio's sessions folder, and only without one the folder next to the one checked.
#[cfg(unix)]
pub(super) fn probe_dir_for(ephemeral_root: Option<&Path>, near: &Path) -> io::Result<PathBuf> {
    let Some(root) = ephemeral_root else {
        return Ok(near.to_path_buf());
    };
    crate::agent_approval_channel::create_private_dir_all(root).map_err(|e| cannot_make(root, e))?;
    check_real_folder(root, &std::fs::symlink_metadata(root)?)?;
    Ok(root.to_path_buf())
}

fn check_real_folder(dir: &Path, meta: &std::fs::Metadata) -> io::Result<()> {
    if meta.file_type().is_symlink() {
        return Err(refused(Refusal::Link, dir));
    }
    if !meta.is_dir() {
        return Err(refused(Refusal::File, dir));
    }
    Ok(())
}

/// Makes the folder a session file is written into (`parent`). When `parent` is a session folder, or lies inside one,
/// that folder gets the checks of [`create_session_folder`], so a write never follows a link planted at the folder.
pub(super) fn prepare_write_parent(parent: &Path) -> io::Result<()> {
    let root = ephemeral_sessions_root();
    let is_session_folder = |dir: &Path| {
        dir.parent()
            .is_some_and(|up| root.as_deref() == Some(up) || up == Path::new(UNAVAILABLE_ROOT) || up.file_name().is_some_and(|name| name == "agent-sessions"))
    };
    match parent.ancestors().find(|dir| is_session_folder(dir)) {
        Some(folder) => {
            create_session_folder_in(root.as_deref(), folder)?;
            if folder == parent {
                Ok(())
            } else {
                std::fs::create_dir_all(parent)
            }
        }
        None => std::fs::create_dir_all(parent),
    }
}
