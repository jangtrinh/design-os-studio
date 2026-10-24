//! A client thread connected to the fake app-server, with the recorded 0.164 `initialize` exchange already played.

use super::fake_server::{Conn, FakeServer, QUIET, WAIT};
use super::support::{fixture, with};
use crate::codex_remote::protocol::{initialize_params, CLIENT_NAME};
use crate::codex_remote::rpc_client::{spawn, ClientConfig, ClientHandle, Timing, Wake};
use crate::codex_remote::session_folder::SessionFolder;
use crate::codex_remote::CodexEvent;
use serde_json::json;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::Duration;

/// Milliseconds instead of the production seconds.
pub const FAST: Timing = Timing {
    grace: Duration::from_millis(20),
    connect_retry: Duration::from_millis(10),
    connect_timeout: Duration::from_millis(400),
    resume_retry: Duration::from_millis(5),
    max_resume_attempts: 20,
};
pub fn test_config(socket: &Path, ask_mode: Arc<AtomicBool>, wake: Wake) -> ClientConfig {
    ClientConfig {
        socket: socket.to_path_buf(),
        generation: 7,
        tab_title: "Codex CLI".into(),
        ask_mode,
        session_folder: SessionFolder::new(Path::new("/session")),
        timing: FAST,
        wake,
    }
}

/// A client thread connected to a fake server.
pub struct Harness {
    pub conn: Conn,
    pub events: Receiver<CodexEvent>,
    pub client: ClientHandle,
    pub ask_mode: Arc<AtomicBool>,
    pub woken: Arc<AtomicUsize>,
    _server: FakeServer,
}

impl Harness {
    /// Connects, completes `initialize` as `originator/version`, and answers `thread/loaded/list` with `threads`.
    pub fn start_with(originator: &str, version: &str, threads: &[&str]) -> Self {
        let server = FakeServer::bind();
        let (tx, events) = channel();
        let (ask_mode, woken) = (Arc::new(AtomicBool::new(true)), Arc::new(AtomicUsize::new(0)));
        let counter = woken.clone();
        let wake: Wake = Arc::new(move || {
            counter.fetch_add(1, Ordering::Relaxed);
        });
        let client = spawn(test_config(&server.path, ask_mode.clone(), wake), tx).expect("the client thread");
        let mut conn = server.accept();
        let init = conn.expect("initialize");
        assert_eq!((init["id"].clone(), init["params"].clone()), (json!(1), initialize_params()), "{}", CLIENT_NAME);
        let agent = format!("{originator}/{version} (OS; arch) xterm-256color ({CLIENT_NAME}; 0.6.1)");
        conn.reply(&init, with(fixture("initialize-response"), "/result/userAgent", json!(agent))["result"].clone());
        let mut harness = Self { conn, events, client, ask_mode, woken, _server: server };
        if version.starts_with("0.164") {
            assert_eq!(harness.conn.recv()["method"], "initialized");
            let list = harness.conn.expect("thread/loaded/list");
            harness.conn.reply(&list, json!({"data": threads, "nextCursor": null}));
        }
        harness
    }

    pub fn start() -> Self {
        Self::start_with("codex-tui", "0.164.0", &[])
    }

    pub fn next(&self) -> CodexEvent {
        self.events.recv_timeout(WAIT).expect("an event")
    }

    /// Events until `pick` returns something; the others are skipped.
    pub fn until<T>(&self, mut pick: impl FnMut(CodexEvent) -> Option<T>) -> T {
        loop {
            if let Some(found) = pick(self.next()) {
                return found;
            }
        }
    }

    /// No event arrives within `QUIET`.
    pub fn no_event(&self) -> bool {
        self.events.recv_timeout(QUIET).is_err()
    }
}
