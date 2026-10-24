//! One connection of the client thread (#930): finding the socket, the WebSocket upgrade, the reader thread, and the
//! loop that feeds the state machine and carries out what it asks for.

use super::rpc_client::{ClientCommand, ClientConfig, End, Input, Out};
use super::rpc_state::{Action, Machine};
use super::ws::{self, Decoder, Message};
use super::{CodexEvent, Originator};
use serde_json::Value;
use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

/// Waits `span`; `true` when a shutdown was asked for meanwhile.
fn pause(queue: &Receiver<Input>, span: Duration) -> bool {
    let deadline = Instant::now() + span;
    loop {
        match queue.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Input::Command(ClientCommand::Shutdown)) | Err(RecvTimeoutError::Disconnected) => return true,
            Ok(_) => {}
            Err(RecvTimeoutError::Timeout) => return false,
        }
    }
}

/// Waits for the socket, gives the TUI the grace to initialize first, and connects. `Lost` here means the timeout.
pub(super) fn connect(config: &ClientConfig, queue: &Receiver<Input>) -> Result<UnixStream, End> {
    let started = Instant::now();
    let mut graced = false;
    loop {
        if config.socket.exists() {
            if !graced && pause(queue, config.timing.grace) {
                return Err(End::Quiet);
            }
            graced = true;
            if let Ok(stream) = UnixStream::connect(&config.socket) {
                return Ok(stream);
            }
        }
        if started.elapsed() >= config.timing.connect_timeout {
            return Err(End::Lost);
        }
        if pause(queue, config.timing.connect_retry) {
            return Err(End::Quiet);
        }
    }
}

/// Reads the socket until it ends, handing every complete message to the client thread.
fn read_frames(mut stream: UnixStream, early: Vec<u8>, inbox: Sender<Input>) {
    let mut decoder = Decoder::client();
    decoder.push(&early);
    let mut buf = [0u8; 16 * 1024];
    loop {
        loop {
            match decoder.next_message() {
                Ok(Some(message)) => {
                    if inbox.send(Input::Frame(message)).is_err() {
                        return;
                    }
                }
                Ok(None) => break,
                Err(_) => {
                    let _ = inbox.send(Input::ReaderEnded);
                    return;
                }
            }
        }
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => decoder.push(&buf[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
    let _ = inbox.send(Input::ReaderEnded);
}

struct Session<'a> {
    writer: UnixStream,
    out: &'a Out,
    retry_after: Duration,
    /// Join retries waiting for their time.
    retries: Vec<(Instant, String)>,
}

impl Session<'_> {
    fn write(&mut self, opcode: u8, payload: &[u8]) -> std::io::Result<()> {
        self.writer.write_all(&ws::encode_frame(opcode, payload, Some(ws::random_mask())))
    }

    /// Carries out what the machine asked for; `Some` when the session must end.
    fn apply(&mut self, actions: Vec<Action>) -> Option<End> {
        for action in actions {
            match action {
                Action::Send(frame) => {
                    if self.write(ws::OP_TEXT, frame.to_string().as_bytes()).is_err() {
                        return Some(End::Lost);
                    }
                }
                Action::Emit(event) => {
                    if let CodexEvent::Originator(who) = &event {
                        // Once per link: whether the TUI or Studio named the server first (no other field).
                        eprintln!("codex originator: {}", if *who == Originator::Studio { "studio" } else { "tui" });
                    }
                    if !self.out.emit(event) {
                        return Some(End::Quiet);
                    }
                }
                Action::Retry { thread_id } => self.retries.push((Instant::now() + self.retry_after, thread_id)),
                Action::Close => return Some(End::Quiet),
            }
        }
        None
    }

    fn until_next_retry(&self) -> Duration {
        let now = Instant::now();
        self.retries.iter().map(|(at, _)| at.saturating_duration_since(now)).min().unwrap_or(Duration::from_secs(3600))
    }

    fn due_retries(&mut self) -> Vec<String> {
        let now = Instant::now();
        let (due, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.retries).into_iter().partition(|(at, _)| *at <= now);
        self.retries = later;
        due.into_iter().map(|(_, thread)| thread).collect()
    }
}

pub(super) fn session(config: &ClientConfig, out: &Out, mut stream: UnixStream, inbox: Sender<Input>, queue: &Receiver<Input>, machine: &mut Machine) -> End {
    if stream.write_all(&ws::handshake_request(&ws::random_key())).is_err() || stream.set_read_timeout(Some(Duration::from_secs(5))).is_err() {
        return End::Lost;
    }
    let Ok(early) = ws::read_handshake(&mut stream) else { return End::Lost };
    let Ok(reader) = stream.try_clone() else { return End::Lost };
    if stream.set_read_timeout(None).is_err() {
        return End::Lost;
    }
    let reader_inbox = inbox.clone();
    if std::thread::Builder::new().name("codex-rpc-reader".into()).spawn(move || read_frames(reader, early, reader_inbox)).is_err() {
        return End::Lost;
    }
    let mut session = Session { writer: stream, out, retry_after: config.timing.resume_retry, retries: Vec::new() };
    let mut end = session.apply(machine.start());
    while end.is_none() {
        end = match queue.recv_timeout(session.until_next_retry()) {
            Ok(Input::Frame(Message::Text(text))) => serde_json::from_str::<Value>(&text).ok().and_then(|frame| session.apply(machine.on_frame(&frame))),
            Ok(Input::Frame(Message::Ping(data))) => session.write(ws::OP_PONG, &data).err().map(|_| End::Lost),
            Ok(Input::Frame(Message::Pong(_))) => None,
            Ok(Input::Frame(Message::Close)) | Ok(Input::ReaderEnded) | Err(RecvTimeoutError::Disconnected) => Some(End::Lost),
            Ok(Input::Command(ClientCommand::Answer { key, decision })) => session.apply(machine.answer(&key, decision)),
            Ok(Input::Command(ClientCommand::Shutdown)) => Some(End::Quiet),
            Err(RecvTimeoutError::Timeout) => None,
        };
        for thread_id in session.due_retries() {
            end = end.or_else(|| session.apply(machine.on_retry(&thread_id)));
        }
    }
    let _ = session.write(ws::OP_CLOSE, &[]);
    let _ = session.writer.shutdown(Shutdown::Both);
    end.unwrap_or(End::Quiet)
}
