//! Codex approvals in the Agents panel (#930).
//!
//! In Ask mode on macOS with a tested Codex release, a Codex tab runs `codex app-server` as a child of Studio and the
//! TUI as `codex --remote unix://<rendezvous>`. Studio is a second client of that server: it reads the structured
//! approval requests, shows Approve and Deny cards, and answers a request only after a click on a card it showed. A tab
//! that cannot take this path starts exactly as it does today. The terminal's own menu keeps working either way.
//!
//! `shell_env` captures the tab's environment, `gate` decides the path with it, `remote` starts the tab, `server_process`
//! and `stale_socket` own the child, `launch` builds the TUI command, `ws`, `rpc_client` and `rpc_session` are the
//! connection, `rpc_state` is its state machine, `card_mapping` makes the cards (the text through `approval_text`, the check
//! that a command runs in the tab's folder through `session_folder`).
pub mod card_mapping;
pub mod exec_cards;
pub mod file_changes;
pub mod gate;
pub mod item_index;
pub mod launch;
pub mod protocol;
pub mod remote;
pub mod rpc_cards;
pub mod rpc_client;
pub mod rpc_session;
pub mod rpc_state;
pub mod rpc_threads;
pub mod server_process;
pub mod session_folder;
pub mod shell_capture;
pub mod shell_env;
pub mod stale_socket;
pub mod ws;

use card_mapping::{CodexCard, OpenTabReason};
use protocol::Decision;
use rpc_client::{ClientCommand, ClientHandle};
use server_process::ServerHandle;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
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
    /// Not macOS: the card path is built and tested there only. The tab starts as today, with a note saying so.
    PlatformUnsupported,
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
    _server: Option<ServerHandle>,
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

#[cfg(test)]
impl CodexLink {
    /// A link with no server behind it: the test feeds `events` and reads the answers the link sends.
    pub(crate) fn for_test() -> (CodexLink, std::sync::mpsc::Sender<CodexEvent>, Receiver<rpc_client::Input>) {
        let (tx, events) = std::sync::mpsc::channel();
        let (client, sent) = ClientHandle::detached();
        (CodexLink { events, client, ask_mode: Arc::new(AtomicBool::new(true)), _server: None }, tx, sent)
    }
}

impl Drop for CodexLink {
    fn drop(&mut self) {
        self.client.send(ClientCommand::Shutdown);
    }
}

#[cfg(test)]
mod tests;
