//! The Studio-private folder that holds what the hooks of every agent CLI write, and the replies they read.
//!
//! A hook is a shell command that the agent's CLI runs as the user, in its own process, with no sandbox: Claude runs its hooks outside
//! its sandbox (its docs say so), and Gemini and Grok run theirs unconfined unless the user turns their own sandbox on, which Studio
//! never does. A hook that wrote into the session folder, which an agent can write, wrote through a link, a hard link or a name the
//! agent had swapped there, and so could overwrite or extend any file the user owns. Redirection (`>`, `>>`) cannot refuse a link.
//! So every file a hook writes, and every file it reads back into the agent's context, lies where no agent write grant reaches.
//!
//! ```text
//! <data dir>/designos/ap/<sid>/hook-out/       0700  inside the approval channel folder of the same session (`agent_approval_channel`)
//!   agent-status.json                                 the last state word of any hook
//!   agent-events.jsonl  agent-events.jsonl.lock/      one wrapped record per hook event, and the lock an append takes
//!   turn-marks.jsonl                                  the turn boundaries, with the byte length of `doc-patch.jsonl` at each
//!   claude-transcript.json  .lock/  .tmp.<pid>        which transcript file belongs to which Claude tab
//!   bootstrap.delivered.<run>                         the marker of a first turn whose bootstrap block was printed
//!   page-context-reply.json  bootstrap-context-reply.json  bootstrap-compact-reply.json  skills-context-reply-<tab>-<slot>.json
//!                                                     what the prompt hook of Claude prints into the agent's context
//! ```
//!
//! The folder lies inside the approval channel folder because it has the same lifetime: it is made when a session's agent files are
//! written, and goes with the channel when the last tab of the session closes or the session ends. It is a folder of its own, apart
//! from the files the channel holds for a CLI to read or run (the approval hook script, the hooks, MCP and plugin files), so `hook-out`
//! holds nothing a CLI is told to run.
//!
//! What no agent grant reaches. Studio passes these grants and no others (the private root is outside every one of them):
//!
//! | CLI | what Studio passes | what the CLI's own sandbox, when the user turns it on, grants |
//! |---|---|---|
//! | Claude Code | its working folder, the session folder; `--mcp-config`, `--settings`, `--plugin-dir` name files of the channel to read | none turned on by Studio |
//! | Codex | its working folder; no `-s`, no `--add-dir`; its `workspace-write` sandbox adds `$TMPDIR` and `/tmp` | cwd, `$TMPDIR`, `/tmp` |
//! | Gemini CLI | its working folder and `--include-directories "$DESIGNOS_SESSION_DIR"`, both the session folder | the session folder, `$TMPDIR`, its cache and `~/.gemini`, `~/.npm`, `~/.cache` |
//! | Grok Build | `--cwd "$DESIGNOS_SESSION_DIR"` | cwd, `~/.grok`, temp (profile `workspace`) |
//! | Antigravity | `--add-dir "$DESIGNOS_SESSION_DIR"` | the workspace and temp |
//! | OpenCode | `opencode "$DESIGNOS_SESSION_DIR"` | none |
//! | every tab | `designos-mcp --workspace <the open file's folder>` (Studio's working folder for a file with no path), which writes `.designos`, `.png` and `.fig` names only | |
//!
//! So the folders to stay out of are the session folder, `$TMPDIR`, `/tmp` and `/var/tmp`. The private root is checked against the temp
//! folders (`agent_approval_channel::default_private_root`); a folder Studio cannot keep private means no hook reports there at all,
//! and the tab shows no status (the same as a tab of a CLI with no hooks).
//!
//! The open file's folder is the one grant that can contain the data folder (a file kept in the home folder, or in a folder with no
//! path at all, which Studio serves from its working folder). It is not checked, because `designos-mcp` creates only the names that end
//! in `designos_mcp::engine::WORKSPACE_WRITE_EXTENSIONS` (`.designos`, `.png`, `.fig`, any case) and, for an overwrite, a temporary
//! name that starts with `OUTPUT_TEMP_PREFIX` (made with `create_new`, so never over an existing file), and no name in the hook folder
//! does either: a test lists every name this module hands out and every name a hook command derives from one, and fails on the first
//! that does.
//!
//! `doc-patch.jsonl` stays in the session folder, because `designos-mcp` writes it there. A hook only reads its length, with `stat`,
//! which neither opens it (a FIFO planted there cannot block the hook) nor writes anything.
//!
//! Studio reads these files through the routed helpers of `agent_session_files`, which anchor at the private root as they do at a
//! session folder: from an open handle, and never through a link.

use std::io;
use std::path::{Path, PathBuf};

/// Name of the folder inside a session's channel folder.
pub const DIR_NAME: &str = "hook-out";

/// The last state word of any hook, as `{"state":…,"run_id":…}`.
pub const STATUS_FILE_NAME: &str = "agent-status.json";

/// The wrapped hook records, one per line.
pub const EVENTS_FILE_NAME: &str = "agent-events.jsonl";

/// The turn boundary marks, one per line.
pub const TURN_MARKS_FILE_NAME: &str = "turn-marks.jsonl";

/// What the prompt hook of Claude prints on the turns after the first.
pub const PAGE_REPLY_FILE_NAME: &str = "page-context-reply.json";

/// What the prompt hook of Claude prints on the first turn of a run, once the full bootstrap block is ready.
pub const BOOTSTRAP_REPLY_FILE_NAME: &str = "bootstrap-context-reply.json";

/// What the prompt hook of Claude prints on the first turn of a run when the full block is late.
pub const COMPACT_REPLY_FILE_NAME: &str = "bootstrap-compact-reply.json";

/// The patch log `designos-mcp` writes in the session folder; the one file of that folder a hook touches, and only to read its length.
pub const PATCH_LOG_FILE_NAME: &str = "doc-patch.jsonl";

/// Where the hooks of one session report to, and the session folder they belong to.
#[derive(Clone, Debug)]
pub struct HookPaths {
    session_dir: PathBuf,
    dir: PathBuf,
}

impl HookPaths {
    /// The paths of `session_dir`'s hook folder, wherever they are; nothing is made. Without a private data folder they lie below the
    /// root no session can use, so a read of one fails and a write is refused.
    pub fn locate(session_dir: &Path) -> HookPaths {
        let dir = match crate::agent_approval_channel::channel_dir(session_dir) {
            Some(channel) => channel.join(DIR_NAME),
            None => crate::agent_session_files::unavailable_path(&crate::agent_approval_channel::session_id(session_dir)).join(DIR_NAME),
        };
        HookPaths { session_dir: session_dir.to_path_buf(), dir }
    }

    /// Paths in `dir` for the session folder `session_dir`, for a test that checks the commands built from them.
    #[cfg(test)]
    pub fn at(session_dir: &Path, dir: &Path) -> HookPaths {
        HookPaths { session_dir: session_dir.to_path_buf(), dir: dir.to_path_buf() }
    }

    /// Makes the folder (owner-only, never through a link, with the private channel folder above it) and returns its paths. Fails when
    /// Studio has no private data folder or when the folder cannot be made private: no hook may then report anywhere, and the caller
    /// installs none that writes or reads a file.
    ///
    /// The open file's folder is not checked. It can contain the data folder, and `designos-mcp` serves it, but that server writes only
    /// the names that end in `.designos`, `.png` or `.fig` (see the module doc), and no name in this folder does.
    pub fn prepare(session_dir: &Path) -> io::Result<HookPaths> {
        let channel = crate::agent_approval_channel::prepare_channel_dir(session_dir)?;
        let paths = HookPaths { session_dir: session_dir.to_path_buf(), dir: channel.join(DIR_NAME) };
        crate::agent_session_files::create_session_dir_all(&paths.dir)?;
        Ok(paths)
    }

    /// Whether the hook folder is there: made by [`HookPaths::prepare`], and not gone since. A session with none has no hook installed
    /// that writes or reads a file.
    pub fn is_made(&self) -> bool {
        std::fs::symlink_metadata(&self.dir).is_ok_and(|meta| meta.is_dir())
    }

    /// The folder itself.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The session folder these hooks belong to.
    pub fn session_dir(&self) -> &Path {
        &self.session_dir
    }

    pub fn status(&self) -> PathBuf {
        self.dir.join(STATUS_FILE_NAME)
    }

    pub fn events(&self) -> PathBuf {
        self.dir.join(EVENTS_FILE_NAME)
    }

    pub fn turn_marks(&self) -> PathBuf {
        self.dir.join(TURN_MARKS_FILE_NAME)
    }

    /// The file the Claude hooks record the transcript of each tab in.
    pub fn transcript(&self) -> PathBuf {
        self.dir.join(crate::agent_usage::CLAUDE_TRANSCRIPT_META_FILENAME)
    }

    /// The marker the prompt hook leaves when it has printed the bootstrap block of run `run_id`.
    pub fn delivered_marker(&self, run_id: u64) -> PathBuf {
        self.dir.join(format!("bootstrap.delivered.{run_id}"))
    }

    pub fn page_reply(&self) -> PathBuf {
        self.dir.join(PAGE_REPLY_FILE_NAME)
    }

    pub fn bootstrap_reply(&self) -> PathBuf {
        self.dir.join(BOOTSTRAP_REPLY_FILE_NAME)
    }

    pub fn compact_reply(&self) -> PathBuf {
        self.dir.join(COMPACT_REPLY_FILE_NAME)
    }

    /// The patch log of the session, in the session folder.
    pub fn patch_log(&self) -> PathBuf {
        self.session_dir.join(PATCH_LOG_FILE_NAME)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(tag: &str) -> PathBuf {
        std::env::temp_dir().join("designos-sessions").join(format!("hook-out-{tag}-{}", std::process::id()))
    }

    /// Whether `inner` lies inside `folder`, as either is spelled or resolved through the links above it.
    fn lies_in(inner: &Path, folder: &Path) -> bool {
        let resolve = crate::agent_approval_channel::resolve_existing_prefix;
        inner.starts_with(folder) || resolve(inner).starts_with(resolve(folder))
    }

    /// The production hook folder sits outside the folders Studio grants a CLI and the folders its own sandbox adds: not in the session
    /// folder, not in `$TMPDIR`, `/tmp` or `/var/tmp`, whichever way each is spelled. The open file's folder is the one grant it can
    /// lie in; `no_file_a_hook_writes_or_reads_has_a_name_designos_mcp_can_write` is what makes that harmless.
    #[test]
    fn the_production_hook_folder_lies_outside_every_folder_an_agent_can_write() {
        // A test harness with HOME under a temp folder has no production root (`default_private_root` refuses it, which the test of the
        // approval channel checks); the shape of the folder is the same under a root of the usual place.
        let root = crate::agent_approval_channel::default_private_root()
            .unwrap_or_else(|| PathBuf::from("/Users/someone/Library/Application Support/designos").join("ap"));
        let session_dir = session("prod");
        let channel = crate::agent_approval_channel::channel_dir_in(&root, &session_dir).expect("a channel folder");
        let paths = HookPaths { session_dir: session_dir.clone(), dir: channel.join(DIR_NAME) };
        // Every grant of the table in the module doc: the session folder (the working folder and what `--add-dir`,
        // `--include-directories` and `--cwd` name) and the temp folders. A folder of an open file is a grant too, whichever folder the
        // file is in, so a few of them that do not hold the data folder are checked as well.
        let open_files_folders = [
            std::env::temp_dir().join("a-designs-folder"),
            PathBuf::from("/tmp/designs"),
            std::env::var_os("HOME").map(PathBuf::from).map(|home| home.join("Documents")).unwrap_or_else(|| PathBuf::from("/Users/someone/Documents")),
        ];
        let mut grants = vec![session_dir.clone(), std::env::temp_dir(), PathBuf::from("/tmp"), PathBuf::from("/var/tmp")];
        grants.extend(open_files_folders);
        for file in
            [paths.status(), paths.events(), paths.turn_marks(), paths.transcript(), paths.delivered_marker(3), paths.page_reply(), paths.bootstrap_reply()]
        {
            for grant in &grants {
                assert!(!lies_in(paths.dir(), grant), "the hook folder {} lies in {}", paths.dir().display(), grant.display());
                assert!(!file.starts_with(grant), "{} lies in {}", file.display(), grant.display());
            }
        }
        // The one thing that does lie in the session folder is the patch log a hook only measures.
        assert!(paths.patch_log().starts_with(&session_dir));
    }

    #[test]
    fn without_a_private_data_folder_every_path_is_one_no_read_or_write_can_use() {
        crate::agent_approval_channel::without_private_root(|| {
            let paths = HookPaths::locate(&session("none"));
            assert!(paths.status().starts_with(crate::agent_session_files::unavailable_path("")), "{}", paths.status().display());
            assert!(HookPaths::prepare(&session("none")).is_err());
            assert!(crate::agent_session_files::read_session_text(&paths.status()).is_none());
            assert!(crate::agent_session_files::atomic_write_session_file(&paths.status(), b"{}").is_err());
        });
    }

    #[cfg(unix)]
    #[test]
    fn prepare_makes_an_owner_only_folder_inside_the_approval_channel_and_the_routed_helpers_use_it() {
        use std::os::unix::fs::PermissionsExt;
        let session_dir = session("made");
        crate::agent_approval_channel::remove_channel(&session_dir);
        let paths = HookPaths::prepare(&session_dir).expect("the hook folder is made");
        assert_eq!(std::fs::metadata(paths.dir()).expect("a folder").permissions().mode() & 0o777, 0o700);
        assert_eq!(paths.dir().parent(), crate::agent_approval_channel::channel_dir(&session_dir).as_deref());
        crate::agent_session_files::atomic_write_session_file(&paths.events(), b"one\n").expect("a routed write into the hook folder");
        assert_eq!(crate::agent_session_files::read_session_text(&paths.events()).as_deref(), Some("one\n"));
        // A link planted where a file goes is replaced by the write, never followed.
        let victim = std::env::temp_dir().join(format!("designos-hook-out-victim-{}", std::process::id()));
        std::fs::write(&victim, "keep").expect("a victim");
        std::fs::remove_file(paths.status()).ok();
        std::os::unix::fs::symlink(&victim, paths.status()).expect("a link");
        crate::agent_session_files::atomic_write_session_file(&paths.status(), b"{}").expect("the name is replaced");
        assert_eq!(std::fs::read_to_string(&victim).expect("the victim"), "keep");
        assert!(crate::agent_session_files::read_session_text(&paths.status()).is_some());
        let _ = std::fs::remove_file(&victim);
        crate::agent_approval_channel::remove_channel(&session_dir);
    }
}
