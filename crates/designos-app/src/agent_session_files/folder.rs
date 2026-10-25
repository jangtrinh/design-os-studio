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
//!
//! What Studio writes below a session folder is a third rule, kept by [`super::no_follow`].

#[cfg(unix)]
use super::folder_owner::{check_owner, own_uid};
use super::refusal::{cannot, failed, refused, Fix, Refusal};
use std::io;
use std::path::{Path, PathBuf};

/// Folder name of the ephemeral sessions under Studio's data folder.
const SESSIONS_DIR_NAME: &str = "sessions";

/// Parent of a session path when Studio has no private data folder: [`create_session_folder`] refuses everything below
/// it, never falling back to a folder an agent can write. It is absolute on every platform, so no call on it ever resolves
/// against Studio's working folder: on Unix nothing can be made below it (`/dev/null` is a file), and on Windows no path
/// below the `NUL` device exists.
#[cfg(unix)]
pub(super) const UNAVAILABLE_ROOT: &str = "/dev/null/designos-no-private-data-folder";
#[cfg(not(unix))]
pub(super) const UNAVAILABLE_ROOT: &str = r"\\.\NUL\designos-no-private-data-folder";

/// The error for a session path below [`UNAVAILABLE_ROOT`].
pub(super) fn no_private_data_folder() -> io::Error {
    let message = "Studio has no private data folder for agent sessions (its user data folder is missing or inside a temporary folder)";
    failed(io::ErrorKind::Other, message.to_string(), Fix::HomeFolder)
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

/// `root` spelled without the links above it. It is looked up once it exists and kept: the sessions root of a process never changes.
fn real_sessions_root(root: Option<&Path>) -> Option<&'static Path> {
    static REAL_ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    if let Some(real) = REAL_ROOT.get() {
        return Some(real);
    }
    let real = root?.canonicalize().ok()?;
    Some(REAL_ROOT.get_or_init(|| real))
}

/// The private channel root, spelled without the links above it. Looked up once it exists and kept, like [`real_sessions_root`].
fn real_private_root(root: Option<&Path>) -> Option<&'static Path> {
    static REAL_ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    if let Some(real) = REAL_ROOT.get() {
        return Some(real);
    }
    let real = root?.canonicalize().ok()?;
    Some(REAL_ROOT.get_or_init(|| real))
}

/// A path below the root no session can use, named `name`: what a folder Studio cannot make private resolves to, so every read of it
/// fails and every write is refused, and nothing ever lands in a folder an agent can write.
pub fn unavailable_path(name: &str) -> PathBuf {
    Path::new(UNAVAILABLE_ROOT).join(name)
}

/// What a folder is to the routed helpers, when they anchor at it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Anchor {
    /// A session folder: a direct child of the ephemeral sessions folder (or of the root no session can use), or of a library's
    /// `agent-sessions` folder. Everything below it is the agent's to write, and Studio's helpers never follow a link in it.
    Session,
    /// The folder of a session's approval channel, a direct child of Studio's private channel root (`<data dir>/designos/ap`). No agent
    /// grant reaches it, and only its hook folder (`agent_hook_outputs`) is a place for the routed helpers: the rest of it is what
    /// decides which commands a CLI runs and which request Studio answers, and a helper that wrote there would be a way to change it.
    Channel,
}

/// The folders the routed helpers can anchor at, found once and asked of any number of folders.
fn anchors() -> impl Fn(&Path) -> Option<Anchor> {
    let root = ephemeral_sessions_root();
    // The same folder spelled through the links above it (`/private/var` for `/var` on macOS) is Studio's own folder too.
    let real_root = real_sessions_root(root.as_deref());
    let private = crate::agent_approval_channel::private_root();
    let real_private = real_private_root(private.as_deref());
    move |dir: &Path| {
        let up = dir.parent()?;
        if root.as_deref() == Some(up)
            || real_root == Some(up)
            || up == Path::new(UNAVAILABLE_ROOT)
            || up.file_name().is_some_and(|name| name == "agent-sessions")
        {
            Some(Anchor::Session)
        } else if private.as_deref() == Some(up) || real_private == Some(up) {
            Some(Anchor::Channel)
        } else {
            None
        }
    }
}

/// The folder `path` lies in that the routed helpers anchor at, when it lies in one: the closest folder above `path` that is a session
/// folder, or the channel folder of a session when `path` is in its hook folder (`hook-out`, where the hooks report to). Each is
/// opened from its own handle and never through a link.
pub(super) fn session_folder_of(path: &Path) -> Option<PathBuf> {
    let anchor_of = anchors();
    path.ancestors()
        .skip(1)
        .find(|dir| match anchor_of(dir) {
            Some(Anchor::Session) => true,
            Some(Anchor::Channel) => path
                .strip_prefix(dir)
                .ok()
                .and_then(|below| below.components().next())
                .is_some_and(|first| first.as_os_str() == crate::agent_hook_outputs::DIR_NAME),
            None => false,
        })
        .map(Path::to_path_buf)
}

/// Whether `dir` is a folder the routed helpers anchor at, of either kind. The folder of a place that [`session_folder_of`] found is.
pub(super) fn is_anchor_folder(dir: &Path) -> bool {
    anchors()(dir).is_some()
}

/// Whether `path` lies in the private channel root, however it is spelled.
pub(super) fn lies_in_private_root(path: &Path) -> bool {
    let private = crate::agent_approval_channel::private_root();
    let real_private = real_private_root(private.as_deref());
    private.as_deref().is_some_and(|root| path.starts_with(root)) || real_private.is_some_and(|root| path.starts_with(root))
}

/// Makes the session folder `dir` the way rule 2 says; a re-sync of a folder Studio made earlier succeeds. Parent folders
/// are made owner-only, and the sessions folder of an ephemeral session must be a real folder of the user.
pub(super) fn create_session_folder(dir: &Path) -> io::Result<()> {
    create_session_folder_in(ephemeral_sessions_root().as_deref(), dir)
}

pub(super) fn create_session_folder_in(ephemeral_root: Option<&Path>, dir: &Path) -> io::Result<()> {
    if dir.starts_with(UNAVAILABLE_ROOT) {
        return Err(no_private_data_folder());
    }
    let parent = dir.parent().ok_or_else(|| io::Error::other(format!("{} has no parent folder", dir.display())))?;
    match std::fs::symlink_metadata(dir) {
        Ok(_) => return require_own_real_folder(ephemeral_root, dir, parent).map(|_| ()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(cannot("created", dir, e, Fix::CheckPermissions)),
    }
    if ephemeral_root == Some(parent) {
        make_private_root(parent)?;
    } else {
        crate::agent_approval_channel::create_private_dir_all(parent).map_err(|e| cannot("created", parent, e, Fix::CheckPermissions))?;
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
        Err(e) => Err(cannot("created", dir, e, Fix::CheckPermissions)),
    }
}

/// Creates the sessions folder (and what is above it) owner-only and requires it to be a real folder of the user, with
/// no access for anyone else.
fn make_private_root(root: &Path) -> io::Result<()> {
    crate::agent_approval_channel::create_private_dir_all(root).map_err(|e| cannot("created", root, e, Fix::CheckPermissions))?;
    let meta = require_own_real_folder(Some(root), root, root)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700)).map_err(|e| cannot("made private", root, e, Fix::CheckPermissions))?;
        }
    }
    #[cfg(not(unix))]
    let _ = meta;
    Ok(())
}

/// Requires `dir` to be a real folder, not a link, that the user owns. The user's id is read in the sessions folder
/// (`ephemeral_root`), and only without one next to the folder checked (`near`).
pub(super) fn require_own_real_folder(ephemeral_root: Option<&Path>, dir: &Path, near: &Path) -> io::Result<std::fs::Metadata> {
    let meta = std::fs::symlink_metadata(dir).map_err(|e| cannot("checked", dir, e, Fix::CheckPermissions))?;
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
    crate::agent_approval_channel::create_private_dir_all(root).map_err(|e| cannot("created", root, e, Fix::CheckPermissions))?;
    check_real_folder(root, &std::fs::symlink_metadata(root).map_err(|e| cannot("checked", root, e, Fix::CheckPermissions))?)?;
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
