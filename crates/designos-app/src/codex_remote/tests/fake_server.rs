//! An in-process stand-in for `codex app-server --listen unix://...`: it speaks the WebSocket subset over a unix
//! socket, so the client runs against real framing without a Codex or a model.

use super::support::wait_until;
use crate::codex_remote::ws::{self, Decoder, Message};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

pub const WAIT: Duration = Duration::from_secs(5);
/// How long "nothing arrives" is watched for.
pub const QUIET: Duration = Duration::from_millis(250);

pub struct FakeServer {
    listener: UnixListener,
    pub path: PathBuf,
}

impl FakeServer {
    pub fn bind() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        Self::bind_at(&std::env::temp_dir().join(format!("cxs-{}-{}.sock", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))))
    }

    pub fn bind_at(path: &Path) -> Self {
        let _ = std::fs::remove_file(path);
        let listener = UnixListener::bind(path).expect("a socket path under 104 bytes");
        listener.set_nonblocking(true).unwrap();
        Self { listener, path: path.to_path_buf() }
    }

    /// Accepts one client and completes the WebSocket upgrade.
    pub fn accept(&self) -> Conn {
        let mut stream = None;
        assert!(wait_until(WAIT, || self.listener.accept().map(|(s, _)| stream = Some(s)).is_ok()), "no client connected");
        let mut stream = stream.unwrap();
        stream.set_nonblocking(false).unwrap();
        let mut request = Vec::new();
        let mut byte = [0u8; 1];
        while !request.ends_with(b"\r\n\r\n") {
            assert_eq!(stream.read(&mut byte).expect("handshake read"), 1, "client hung up during the handshake");
            request.push(byte[0]);
        }
        assert!(request.starts_with(b"GET /rpc HTTP/1.1\r\n"), "{}", String::from_utf8_lossy(&request));
        stream.write_all(b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: x\r\n\r\n").unwrap();
        Conn { stream, decoder: Decoder::new() }
    }
}

impl Drop for FakeServer {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub struct Conn {
    stream: UnixStream,
    decoder: Decoder,
}

impl Conn {
    /// macOS refuses to set a timeout on a socket whose peer has closed; a read then reports the end of the stream.
    fn timeout(&self, wait: Duration) {
        let _ = self.stream.set_read_timeout(Some(wait));
    }

    pub fn send(&mut self, frame: &Value) {
        self.send_raw(&ws::encode_frame(ws::OP_TEXT, frame.to_string().as_bytes(), None));
    }

    pub fn send_raw(&mut self, bytes: &[u8]) {
        self.stream.write_all(bytes).expect("the client is still connected");
    }

    /// The next message the client wrote, or `None` when nothing came within `wait` or the client hung up.
    pub fn recv_message(&mut self, wait: Duration) -> Option<Message> {
        self.timeout(wait);
        let mut buf = [0u8; 4096];
        loop {
            if let Some(message) = self.decoder.next_message().expect("the client sends valid frames") {
                return Some(message);
            }
            match self.stream.read(&mut buf) {
                Ok(0) | Err(_) => return None,
                Ok(n) => self.decoder.push(&buf[..n]),
            }
        }
    }

    /// The text of the next frame, exactly as it was written.
    pub fn recv_text(&mut self, wait: Duration) -> Option<String> {
        match self.recv_message(wait)? {
            Message::Text(text) => Some(text),
            other => panic!("expected a text frame, got {other:?}"),
        }
    }

    pub fn recv(&mut self) -> Value {
        serde_json::from_str(&self.recv_text(WAIT).expect("the client wrote nothing")).expect("JSON")
    }

    /// The next frame, which must be `method`.
    pub fn expect(&mut self, method: &str) -> Value {
        let frame = self.recv();
        assert_eq!(frame["method"], method, "{frame}");
        frame
    }

    /// Every text frame the client writes in the next `QUIET`. A close frame is not a reply: it ends the list.
    pub fn silence(&mut self) -> Vec<String> {
        let mut texts = Vec::new();
        while let Some(Message::Text(text)) = self.recv_message(QUIET) {
            texts.push(text);
        }
        texts
    }

    /// Answers a request of the client with `result`.
    pub fn reply(&mut self, request: &Value, result: Value) {
        self.send(&json!({"id": request["id"], "result": result}));
    }

    /// The client closed the connection (end of stream), rather than the wait running out.
    pub fn close(&mut self) {
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }

    pub fn hung_up(&mut self) -> bool {
        self.timeout(WAIT);
        let mut buf = [0u8; 256];
        loop {
            match self.stream.read(&mut buf) {
                Ok(0) => return true,
                Ok(_) => continue,
                Err(_) => return false,
            }
        }
    }
}
