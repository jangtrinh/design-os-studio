//! The WebSocket codec: the subset Codex's app-server speaks on its unix socket.

use crate::codex_remote::ws::{
    encode_frame, handshake_request, random_key, random_mask, read_handshake, Decoder, Message, WsError, MAX_MESSAGE_BYTES, OP_CLOSE, OP_PING, OP_PONG, OP_TEXT,
};
use std::io::Read;

/// An unmasked server frame, any FIN bit, with a payload under 126 bytes.
fn raw_frame(fin: bool, opcode: u8, payload: &[u8]) -> Vec<u8> {
    assert!(payload.len() < 126);
    let mut out = vec![if fin { 0x80 } else { 0 } | opcode, payload.len() as u8];
    out.extend_from_slice(payload);
    out
}

fn decode_all(decoder: &mut Decoder) -> Vec<Message> {
    let mut out = Vec::new();
    while let Some(message) = decoder.next_message().expect("a valid stream") {
        out.push(message);
    }
    out
}

/// Gives out one byte per `read`, the worst case for a parser that assumes a whole header arrives at once.
struct Trickle(Vec<u8>);

impl Read for Trickle {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.0.is_empty() || buf.is_empty() {
            return Ok(0);
        }
        buf[0] = self.0.remove(0);
        Ok(1)
    }
}

#[test]
fn client_frames_are_masked_and_use_the_shortest_length_form() {
    let mask = [0x11, 0x22, 0x33, 0x44];
    for (len, header) in [(5usize, vec![0x81, 0x80 | 5]), (200, vec![0x81, 0x80 | 126, 0, 200]), (70_000, vec![0x81, 0x80 | 127, 0, 0, 0, 0, 0, 1, 0x11, 0x70])]
    {
        let payload: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
        let frame = encode_frame(OP_TEXT, &payload, Some(mask));
        assert_eq!(&frame[..header.len()], header.as_slice(), "length form for {len} bytes");
        assert_eq!(&frame[header.len()..header.len() + 4], &mask, "the mask key follows the length");
        let body = &frame[header.len() + 4..];
        assert_eq!(body.len(), len);
        assert!(body.iter().zip(&payload).enumerate().all(|(i, (b, p))| *b == p ^ mask[i % 4]), "payload is XORed with the mask");
    }
}

#[test]
fn a_masked_frame_round_trips_through_the_decoder() {
    let mut decoder = Decoder::new();
    decoder.push(&encode_frame(OP_TEXT, br#"{"id":1}"#, Some(random_mask())));
    assert_eq!(decode_all(&mut decoder), vec![Message::Text(r#"{"id":1}"#.into())]);
    decoder.push(&encode_frame(OP_TEXT, &vec![b'x'; 70_000], None));
    assert_eq!(decode_all(&mut decoder), vec![Message::Text("x".repeat(70_000))]);
}

#[test]
fn the_decoder_reassembles_a_message_fed_one_byte_at_a_time() {
    let mut stream = raw_frame(true, OP_TEXT, b"first");
    stream.extend(raw_frame(true, OP_TEXT, b"second"));
    let mut decoder = Decoder::new();
    let mut got = Vec::new();
    for byte in stream {
        decoder.push(&[byte]);
        got.extend(decode_all(&mut decoder));
    }
    assert_eq!(got, vec![Message::Text("first".into()), Message::Text("second".into())]);
}

#[test]
fn fragments_are_joined_and_a_ping_between_them_is_surfaced() {
    let mut decoder = Decoder::new();
    decoder.push(&raw_frame(false, OP_TEXT, b"hel"));
    decoder.push(&raw_frame(true, OP_PING, b"are-you-there"));
    decoder.push(&raw_frame(false, 0, b"lo "));
    decoder.push(&raw_frame(true, 0, b"world"));
    assert_eq!(decode_all(&mut decoder), vec![Message::Ping(b"are-you-there".to_vec()), Message::Text("hello world".into())]);
}

#[test]
fn pong_and_close_frames_decode() {
    let mut decoder = Decoder::new();
    decoder.push(&raw_frame(true, OP_PONG, b""));
    decoder.push(&raw_frame(true, OP_CLOSE, &[0x03, 0xE8]));
    assert_eq!(decode_all(&mut decoder), vec![Message::Pong(Vec::new()), Message::Close]);
}

#[test]
fn binary_invalid_utf8_and_stray_continuation_frames_are_rejected() {
    for (frame, want) in [
        (raw_frame(true, 2, b"\x00\x01"), WsError::Binary),
        (raw_frame(true, OP_TEXT, &[0xFF, 0xFE]), WsError::BadUtf8),
        (raw_frame(true, 0, b"orphan"), WsError::Protocol("continuation without a message")),
    ] {
        let mut decoder = Decoder::new();
        decoder.push(&frame);
        assert_eq!(decoder.next_message(), Err(want));
    }
}

#[test]
fn a_frame_over_the_cap_is_rejected_before_it_is_buffered() {
    let mut decoder = Decoder::new();
    let mut header = vec![0x81, 127];
    header.extend_from_slice(&((MAX_MESSAGE_BYTES as u64) + 1).to_be_bytes());
    decoder.push(&header);
    assert_eq!(decoder.next_message(), Err(WsError::TooLarge), "the header alone is enough to refuse it");
}

#[test]
fn fragments_over_the_cap_in_total_are_rejected() {
    let half = vec![b'a'; MAX_MESSAGE_BYTES / 2 + 1];
    let mut decoder = Decoder::new();
    let mut first = vec![0x01, 127];
    first.extend_from_slice(&(half.len() as u64).to_be_bytes());
    first.extend_from_slice(&half);
    let mut second = vec![0x80, 127];
    second.extend_from_slice(&(half.len() as u64).to_be_bytes());
    second.extend_from_slice(&half);
    decoder.push(&first);
    assert_eq!(decoder.next_message(), Ok(None), "a first fragment under the cap is held");
    decoder.push(&second);
    assert_eq!(decoder.next_message(), Err(WsError::TooLarge));
}

#[test]
fn the_handshake_accepts_only_101_and_keeps_bytes_after_the_header() {
    let ok = b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n\r\n".iter().copied().chain(raw_frame(true, OP_TEXT, b"early")).collect::<Vec<u8>>();
    let rest = read_handshake(&mut ok.as_slice()).expect("101 is accepted");
    assert_eq!(rest, raw_frame(true, OP_TEXT, b"early"));
    let mut trickle = Trickle(ok);
    assert_eq!(read_handshake(&mut trickle).expect("a header that arrives byte by byte"), Vec::<u8>::new(), "nothing past the header was read");
    assert_eq!(trickle.0, raw_frame(true, OP_TEXT, b"early"), "the early frame is still in the stream");
    for bad in ["HTTP/1.1 400 Bad Request\r\n\r\n", "HTTP/1.1 200 OK\r\n\r\n", "garbage\r\n\r\n"] {
        assert!(matches!(read_handshake(&mut bad.as_bytes()), Err(WsError::Handshake(_))), "{bad:?}");
    }
    assert!(matches!(read_handshake(&mut &b"HTTP/1.1 101 "[..]), Err(WsError::Handshake(_))), "a stream that ends inside the header");
}

#[test]
fn the_handshake_request_names_the_rpc_path_and_the_key() {
    let text = String::from_utf8(handshake_request("abc==")).unwrap();
    assert!(text.starts_with("GET /rpc HTTP/1.1\r\n"), "{text}");
    for line in ["Upgrade: websocket", "Connection: Upgrade", "Sec-WebSocket-Key: abc==", "Sec-WebSocket-Version: 13"] {
        assert!(text.contains(&format!("{line}\r\n")), "{line}");
    }
    assert!(text.ends_with("\r\n\r\n"));
}

#[test]
fn keys_and_masks_are_random() {
    use base64::Engine;
    let (a, b) = (random_key(), random_key());
    assert_ne!(a, b);
    assert_eq!(base64::engine::general_purpose::STANDARD.decode(&a).expect("base64").len(), 16);
    assert_ne!(random_mask(), random_mask());
}

#[test]
fn a_client_refuses_a_masked_frame_from_its_server_and_a_server_accepts_one_from_its_client() {
    let masked = encode_frame(OP_TEXT, b"{}", Some(random_mask()));
    let mut client = Decoder::client();
    client.push(&masked);
    assert_eq!(client.next_message(), Err(WsError::Protocol("masked server frame")));
    let mut server = Decoder::new();
    server.push(&masked);
    assert_eq!(server.next_message(), Ok(Some(Message::Text("{}".into()))));
    client = Decoder::client();
    client.push(&encode_frame(OP_TEXT, b"{}", None));
    assert_eq!(client.next_message(), Ok(Some(Message::Text("{}".into()))), "unmasked frames are what a server sends");
}
