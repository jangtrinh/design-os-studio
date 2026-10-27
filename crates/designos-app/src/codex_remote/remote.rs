//! Starting the card path for one tab (#930): the gate rows in order, then the server and the client.

use super::rpc_client::{self, ClientConfig, Timing, Wake};
use super::server_process::{self, ServerLaunch};
use super::session_folder::SessionFolder;
use super::shell_env::TabEnv;
use super::{gate, stale_socket, CodexLink, FallbackReason};
use portable_pty::CommandBuilder;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::channel;
use std::sync::Arc;

/// What a tab on the card path needs from its caller.
pub struct StartRequest<'a> {
    /// The absolute path of the `codex` the tab will run, as the PTY's `PATH` resolves it.
    pub codex_bin: &'a Path,
    /// The session's private folder, or `None` when there is none.
    pub channel_dir: Option<&'a Path>,
    pub tab_id: usize,
    /// The builder the tab's PTY is made from: the server gets its working folder.
    pub pty: &'a CommandBuilder,
    /// The tab's environment (login-shell environment, captured once per session): the version probe and the server
    /// both run with it.
    pub env: &'a TabEnv,
    /// Called after every event so an idle window repaints.
    pub wake: Wake,
    pub timing: Timing,
}

/// A started server and client for one tab.
pub struct CodexRemote {
    pub link: CodexLink,
    /// The path the TUI connects to (`codex --remote unix://<this>`).
    pub rendezvous: PathBuf,
}

/// Incremented per server start: request ids restart at 0 with every server, and a card key must not repeat.
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

impl CodexRemote {
    /// Runs the launch gate and, when every row holds, starts the server and the client. Any `Err` means: start the
    /// tab as today.
    pub fn start(request: StartRequest) -> Result<CodexRemote, FallbackReason> {
        gate::check_binary(request.codex_bin, request.env)?;
        let channel_dir = request.channel_dir.ok_or(FallbackReason::NoPrivateFolder)?;
        let rendezvous = gate::rendezvous_path(channel_dir, request.tab_id)?;
        stale_socket::clear_stale_rendezvous(&rendezvous)?;
        let spawn_failed = |e: std::io::Error| FallbackReason::SpawnFailed(e.to_string());
        let launch = ServerLaunch::from_command_builder(
            request.pty,
            request.env,
            request.codex_bin,
            &rendezvous,
            &channel_dir.join(format!("cx-{}.log", request.tab_id)),
        )
        .map_err(spawn_failed)?;
        let server = server_process::start(&launch).map_err(spawn_failed)?;
        let (tx, events) = channel();
        let ask_mode = Arc::new(AtomicBool::new(true));
        let config = ClientConfig {
            socket: rendezvous.clone(),
            generation: NEXT_GENERATION.fetch_add(1, Ordering::Relaxed),
            tab_title: crate::terminal::AgentCliPreset::Codex.short_agent_name().to_string(),
            ask_mode: ask_mode.clone(),
            session_folder: SessionFolder::new(launch.cwd()),
            timing: request.timing,
            wake: request.wake,
        };
        let client = rpc_client::spawn(config, tx).map_err(spawn_failed)?;
        Ok(CodexRemote { link: CodexLink { events, client, ask_mode, _server: Some(server) }, rendezvous })
    }
}
