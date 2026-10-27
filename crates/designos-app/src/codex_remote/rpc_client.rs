//! The client thread of one tab (#930): connects to the app-server's socket, exchanges JSON-RPC frames over WebSocket,
//! and carries the state machine's actions out (socket writes, UI events, join retries).
//!
//! One thread owns the connection. A second thread only reads the socket and hands frames over, so everything the
//! machine sees (server frames, button clicks, shutdown) arrives on one queue and the thread never polls.

use super::protocol::Decision;
use super::rpc_session::{connect, session};
use super::rpc_state::Machine;
use super::session_folder::SessionFolder;
use super::ws::Message;
use super::{CodexEvent, UnavailableReason};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

/// Called after every event, so an idle window repaints without a timer.
pub type Wake = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone, Copy)]
pub struct Timing {
    /// How long to wait after the socket appears before connecting, so the TUI initializes the server first.
    pub grace: Duration,
    pub connect_retry: Duration,
    /// How long, from the start, to wait for the socket. Longer than the TUI's own wait, so the client never gives up on
    /// a server the TUI is using.
    pub connect_timeout: Duration,
    pub resume_retry: Duration,
    pub max_resume_attempts: u32,
}

impl Timing {
    pub const PRODUCTION: Timing = Timing {
        grace: Duration::from_millis(1000),
        connect_retry: Duration::from_millis(100),
        connect_timeout: Duration::from_secs(10),
        resume_retry: Duration::from_millis(250),
        max_resume_attempts: 20,
    };
}

pub struct ClientConfig {
    /// The rendezvous path (a symlink to Codex's socket).
    pub socket: PathBuf,
    pub generation: u64,
    pub tab_title: String,
    pub ask_mode: Arc<AtomicBool>,
    /// The tab's session folder, fixed when the server started.
    pub session_folder: SessionFolder,
    pub timing: Timing,
    pub wake: Wake,
}

pub enum ClientCommand {
    /// A click on a card.
    Answer {
        key: String,
        decision: Decision,
    },
    Shutdown,
}

pub(crate) enum Input {
    Frame(Message),
    Command(ClientCommand),
    /// The reader hit the end of the stream or a frame it could not accept.
    ReaderEnded,
}

/// How the UI side talks to the client thread.
#[derive(Clone)]
pub struct ClientHandle {
    inbox: Sender<Input>,
}

impl ClientHandle {
    /// A handle whose commands land in the returned queue, with no client thread behind it.
    #[cfg(test)]
    pub(super) fn detached() -> (ClientHandle, Receiver<Input>) {
        let (inbox, queue) = channel();
        (ClientHandle { inbox }, queue)
    }

    pub fn send(&self, command: ClientCommand) {
        // A client that already ended has nothing left to answer.
        let _ = self.inbox.send(Input::Command(command));
    }
}

/// How the session ended: the connection broke (the UI is told), or the client stopped on its own account.
#[derive(PartialEq)]
pub(super) enum End {
    Lost,
    Quiet,
}

pub(super) struct Out {
    events: Sender<CodexEvent>,
    wake: Wake,
}

impl Out {
    /// `false` when nobody listens any more.
    pub(super) fn emit(&self, event: CodexEvent) -> bool {
        let sent = self.events.send(event).is_ok();
        (self.wake)();
        sent
    }
}

pub fn spawn(config: ClientConfig, events: Sender<CodexEvent>) -> std::io::Result<ClientHandle> {
    let (inbox, queue) = channel();
    let handle = ClientHandle { inbox: inbox.clone() };
    std::thread::Builder::new().name("codex-rpc-client".into()).spawn(move || run(config, inbox, queue, events))?;
    Ok(handle)
}

fn run(config: ClientConfig, inbox: Sender<Input>, queue: Receiver<Input>, events: Sender<CodexEvent>) {
    let out = Out { events, wake: config.wake.clone() };
    let stream = match connect(&config, &queue) {
        Ok(stream) => stream,
        Err(End::Lost) => {
            out.emit(CodexEvent::Unavailable(UnavailableReason::SocketTimeout));
            return;
        }
        Err(End::Quiet) => return,
    };
    let mut machine =
        Machine::new(config.generation, config.tab_title.clone(), config.ask_mode.clone(), config.session_folder.clone(), config.timing.max_resume_attempts);
    if session(&config, &out, stream, inbox, &queue, &mut machine) == End::Lost {
        out.emit(CodexEvent::Disconnected);
    }
}
