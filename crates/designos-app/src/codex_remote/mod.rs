//! Codex approvals in the Agents panel (#930).
//!
//! In Ask mode on macOS with a tested Codex release, a Codex tab runs `codex app-server` as a child of Studio and the
//! TUI as `codex --remote unix://<rendezvous>`. Studio is a second client of that server: it reads the structured
//! approval requests, shows Approve and Deny cards, and answers a request only after a click on a card it showed. A tab
//! that cannot take this path starts exactly as it does today. The terminal's own menu keeps working either way.
//!
//! `gate` decides the path, `server_process` and `stale_socket` own the child, `launch` builds the TUI command, `ws`,
//! `rpc_client` and `rpc_session` are the connection, `rpc_state` is its state machine, `card_mapping` makes the cards.
// The integration that calls these items is a later change: remove this allow when it lands (#930).
#![cfg_attr(not(test), allow(dead_code))]

pub mod card_mapping;
pub mod gate;
pub mod item_index;
pub mod launch;
pub mod protocol;
pub mod rpc_cards;
pub mod rpc_client;
pub mod rpc_session;
pub mod rpc_state;
pub mod rpc_threads;
pub mod server_process;
pub mod stale_socket;
pub mod ws;

use card_mapping::{CodexCard, OpenTabReason};
use portable_pty::CommandBuilder;
use protocol::Decision;
use rpc_client::{ClientCommand, ClientConfig, ClientHandle, Timing, Wake};
use server_process::{ServerHandle, ServerLaunch};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

/// Why a Codex tab starts the way it does today instead of on the card path.
#[derive(Debug, Clone, PartialEq)]
pub enum FallbackReason {
    /// The version of the binary the tab would run is not known yet (or could not be read). Today's launch, no note.
    VersionUnknown,
    /// A version outside the tested release line. Today's launch, with a note naming the tested line.
    VersionOutsideRange(String),
    /// Studio has no private folder for this session.
    NoPrivateFolder,
    /// The rendezvous path would not fit a unix socket address.
    PathTooLong,
    /// A server still answers on the rendezvous path, or its leftovers could not be cleared.
    SocketInUse,
    /// The server could not be started.
    SpawnFailed(String),
}

/// Which client initialized the server first. The first one names the process, and Studio wants the TUI to be it.
#[derive(Debug, Clone, PartialEq)]
pub enum Originator {
    Tui,
    Studio,
}

/// Why the client stopped without ever serving cards.
#[derive(Debug, Clone, PartialEq)]
pub enum UnavailableReason {
    /// The socket did not appear (or refused connections) in time.
    SocketTimeout,
    /// The server is not the tested release line. The client closed its connection.
    ServerVersion(String),
}

/// What the client tells the UI thread. Read them with [`CodexLink::next_event`].
#[derive(Debug, Clone, PartialEq)]
pub enum CodexEvent {
    Originator(Originator),
    /// Studio has seen the TUI's thread: its composer is up, so a queued prompt can go in.
    ThreadReady,
    /// Studio joined a thread: from now on its approvals reach Studio too.
    Joined {
        thread_id: String,
    },
    /// A request with Approve and Deny buttons.
    Card(CodexCard),
    /// A request that waits for the terminal's menu: it gets no buttons and no reply.
    OpenTabWait {
        key: String,
        why: OpenTabReason,
    },
    /// The request is answered, withdrawn or its turn ended: remove its card.
    Resolved {
        key: String,
    },
    /// The state over all threads (including sub-agents): one waits on an approval, or one is working.
    Waiting {
        approval: bool,
        active: bool,
    },
    Unavailable(UnavailableReason),
    /// The connection ended. Cards are void.
    Disconnected,
}

/// The live link of one tab: the server, the client thread and its event queue. Dropping it ends them all.
pub struct CodexLink {
    events: Receiver<CodexEvent>,
    client: ClientHandle,
    ask_mode: Arc<AtomicBool>,
    /// Dropping the server closes the watchdog's pipe, which ends the server's whole process group.
    _server: ServerHandle,
}

impl CodexLink {
    /// The next event, without waiting.
    pub fn next_event(&self) -> Option<CodexEvent> {
        self.events.try_recv().ok()
    }

    /// Answers the card `key` with one click. A card that is gone, was answered or never had buttons gets no reply.
    pub fn answer(&self, key: &str, decision: Decision) {
        self.client.send(ClientCommand::Answer { key: key.to_string(), decision });
    }

    /// Whether requests may get buttons. A tab that leaves Ask mode turns them off.
    pub fn set_ask_mode(&self, ask: bool) {
        self.ask_mode.store(ask, Ordering::Relaxed);
    }
}

impl Drop for CodexLink {
    fn drop(&mut self) {
        self.client.send(ClientCommand::Shutdown);
    }
}

/// What a tab on the card path needs from its caller.
pub struct StartRequest<'a> {
    /// The absolute path of the `codex` the tab will run, as the PTY's `PATH` resolves it.
    pub codex_bin: &'a Path,
    /// The session's private folder, or `None` when there is none.
    pub channel_dir: Option<&'a Path>,
    pub tab_id: usize,
    /// The builder the tab's PTY is made from: the server gets its environment and working folder.
    pub pty: &'a CommandBuilder,
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
        gate::check_binary(request.codex_bin)?;
        let channel_dir = request.channel_dir.ok_or(FallbackReason::NoPrivateFolder)?;
        let rendezvous = gate::rendezvous_path(channel_dir, request.tab_id)?;
        stale_socket::clear_stale_rendezvous(&rendezvous)?;
        let spawn_failed = |e: std::io::Error| FallbackReason::SpawnFailed(e.to_string());
        let launch = ServerLaunch::from_command_builder(request.pty, request.codex_bin, &rendezvous, &channel_dir.join(format!("cx-{}.log", request.tab_id)))
            .map_err(spawn_failed)?;
        let server = server_process::start(&launch).map_err(spawn_failed)?;
        let (tx, events) = channel();
        let ask_mode = Arc::new(AtomicBool::new(true));
        let config = ClientConfig {
            socket: rendezvous.clone(),
            generation: NEXT_GENERATION.fetch_add(1, Ordering::Relaxed),
            tab_title: crate::terminal::AgentCliPreset::Codex.tab_title().to_string(),
            ask_mode: ask_mode.clone(),
            timing: request.timing,
            wake: request.wake,
        };
        let client = rpc_client::spawn(config, tx).map_err(spawn_failed)?;
        Ok(CodexRemote { link: CodexLink { events, client, ask_mode, _server: server }, rendezvous })
    }
}

#[cfg(test)]
mod tests {
    mod card_flow_tests;
    mod client_tests;
    mod fake_server;
    mod gate_tests;
    mod harness;
    mod launch_tests;
    mod lifecycle_tests;
    mod mapping_tests;
    mod remote_tests;
    mod schema_tests;
    mod snapshot_tests;
    mod stale_socket_tests;
    mod support;
    mod ws_tests;
}
