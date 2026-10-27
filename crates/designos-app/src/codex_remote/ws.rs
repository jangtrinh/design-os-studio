//! The WebSocket (RFC 6455) subset Codex's app-server speaks on its unix socket (`GET /rpc`).
//!
//! Out: the handshake, masked text, pong and close frames. In: unmasked text, ping, pong, close and continuation frames
//! (binary is refused). `Sec-WebSocket-Accept` is not checked: it needs SHA-1, which is not in the dependency tree, and
//! the peer is a Codex process on a private path, so the digest proves nothing.

use std::io::Read;

/// The largest message (and the largest single frame) accepted. Anything bigger ends the link.
pub const MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024;
pub const OP_TEXT: u8 = 0x1;
pub const OP_CLOSE: u8 = 0x8;
pub const OP_PING: u8 = 0x9;
pub const OP_PONG: u8 = 0xA;
const OP_CONTINUATION: u8 = 0x0;
const OP_BINARY: u8 = 0x2;
/// A handshake reply is a status line and a few headers; more than this is not one.
const MAX_HANDSHAKE_BYTES: usize = 16 * 1024;

#[derive(Debug, PartialEq)]
pub enum Message {
    Text(String),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    Close,
}

#[derive(Debug, PartialEq)]
pub enum WsError {
    Handshake(String),
    Binary,
    BadUtf8,
    TooLarge,
    Protocol(&'static str),
}

pub fn handshake_request(key: &str) -> Vec<u8> {
    format!(
        "GET /rpc HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n"
    )
    .into_bytes()
}

/// 16 random bytes, base64 (the `Sec-WebSocket-Key` form). The bytes of a v4 UUID come from the OS random source.
pub fn random_key() -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(uuid::Uuid::new_v4().as_bytes())
}

pub fn random_mask() -> [u8; 4] {
    let [a, b, c, d, ..] = uuid::Uuid::new_v4().into_bytes();
    [a, b, c, d]
}

/// One final frame. A client passes `Some(mask)` (RFC 6455 requires masking client frames); a server side passes `None`.
pub fn encode_frame(opcode: u8, payload: &[u8], mask: Option<[u8; 4]>) -> Vec<u8> {
    let mask_bit = if mask.is_some() { 0x80 } else { 0 };
    let mut out = vec![0x80 | opcode];
    match payload.len() {
        n if n < 126 => out.push(mask_bit | n as u8),
        n if n <= u16::MAX as usize => out.extend([mask_bit | 126, (n >> 8) as u8, n as u8]),
        n => {
            out.push(mask_bit | 127);
            out.extend((n as u64).to_be_bytes());
        }
    }
    let key = mask.unwrap_or_default();
    out.extend(mask.iter().flatten());
    out.extend(payload.iter().enumerate().map(|(i, b)| b ^ key[i % 4]));
    out
}

/// Reads the HTTP reply to the upgrade request and requires status 101. Returns the bytes that followed the header in
/// the same read(s): the first frames can arrive with it.
pub fn read_handshake<R: Read>(reader: &mut R) -> Result<Vec<u8>, WsError> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    let end = loop {
        if let Some(at) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break at + 4;
        }
        if buf.len() > MAX_HANDSHAKE_BYTES {
            return Err(WsError::Handshake("reply header too long".into()));
        }
        match reader.read(&mut chunk) {
            Ok(0) => return Err(WsError::Handshake("connection closed inside the header".into())),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) => return Err(WsError::Handshake(e.to_string())),
        }
    };
    let status = buf[..end].split(|&b| b == b'\r').next().map(String::from_utf8_lossy).unwrap_or_default();
    if status.split_whitespace().nth(1) != Some("101") {
        return Err(WsError::Handshake(format!("expected 101, got {status:?}")));
    }
    Ok(buf.split_off(end))
}

struct Header {
    fin: bool,
    opcode: u8,
    mask: Option<[u8; 4]>,
    len: usize,
    size: usize,
}

/// Incremental frame parser: `push` whatever the socket gave, then `next_message` until it returns `None`. A frame cut
/// in two by a read (or a read timeout) stays buffered, so nothing is lost between calls.
#[derive(Default)]
pub struct Decoder {
    buf: Vec<u8>,
    /// Opcode and data of a fragmented message still waiting for its final frame.
    partial: Option<(u8, Vec<u8>)>,
    /// A server never masks its frames (RFC 6455, 5.1): a masked one ends the link.
    reject_masked: bool,
}

impl Decoder {
    #[cfg(test)]
    pub fn new() -> Self {
        Self::default()
    }

    /// A decoder for what a client reads from its server.
    pub fn client() -> Self {
        Decoder { reject_masked: true, ..Self::default() }
    }

    pub fn push(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    fn header(&self) -> Result<Option<Header>, WsError> {
        let b = &self.buf;
        if b.len() < 2 {
            return Ok(None);
        }
        if b[0] & 0x70 != 0 {
            return Err(WsError::Protocol("reserved bits set"));
        }
        let (fin, opcode, masked) = (b[0] & 0x80 != 0, b[0] & 0x0F, b[1] & 0x80 != 0);
        if masked && self.reject_masked {
            return Err(WsError::Protocol("masked server frame"));
        }
        let (len, mut size) = match b[1] & 0x7F {
            126 if b.len() >= 4 => (u16::from_be_bytes([b[2], b[3]]) as u64, 4),
            127 if b.len() >= 10 => (u64::from_be_bytes([b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9]]), 10),
            126 | 127 => return Ok(None),
            n => (n as u64, 2),
        };
        if len > MAX_MESSAGE_BYTES as u64 {
            return Err(WsError::TooLarge);
        }
        if opcode >= 8 && (!fin || len > 125) {
            return Err(WsError::Protocol("fragmented or long control frame"));
        }
        let mask = if masked {
            let Some(key) = b.get(size..size + 4) else { return Ok(None) };
            size += 4;
            Some([key[0], key[1], key[2], key[3]])
        } else {
            None
        };
        Ok(Some(Header { fin, opcode, mask, len: len as usize, size }))
    }

    /// The next complete message, `None` when more bytes are needed.
    pub fn next_message(&mut self) -> Result<Option<Message>, WsError> {
        loop {
            let Some(head) = self.header()? else { return Ok(None) };
            if self.buf.len() < head.size + head.len {
                return Ok(None);
            }
            let mut data: Vec<u8> = self.buf.drain(..head.size + head.len).skip(head.size).collect();
            if let Some(key) = head.mask {
                data.iter_mut().enumerate().for_each(|(i, b)| *b ^= key[i % 4]);
            }
            match head.opcode {
                OP_CLOSE => return Ok(Some(Message::Close)),
                OP_PING => return Ok(Some(Message::Ping(data))),
                OP_PONG => return Ok(Some(Message::Pong(data))),
                OP_BINARY => return Err(WsError::Binary),
                OP_TEXT if self.partial.is_some() => return Err(WsError::Protocol("new message inside a fragmented one")),
                OP_TEXT | OP_CONTINUATION => {
                    let (opcode, mut whole) = self.partial.take().unwrap_or((head.opcode, Vec::new()));
                    if opcode == OP_CONTINUATION {
                        return Err(WsError::Protocol("continuation without a message"));
                    }
                    if whole.len() + data.len() > MAX_MESSAGE_BYTES {
                        return Err(WsError::TooLarge);
                    }
                    whole.extend_from_slice(&data);
                    if head.fin {
                        return String::from_utf8(whole).map(|text| Some(Message::Text(text))).map_err(|_| WsError::BadUtf8);
                    }
                    self.partial = Some((opcode, whole));
                }
                _ => return Err(WsError::Protocol("unknown opcode")),
            }
        }
    }
}
