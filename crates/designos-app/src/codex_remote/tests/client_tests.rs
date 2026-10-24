//! The client against the fake app-server: connect, initialize, discover the TUI's thread, join it.

use super::fake_server::{FakeServer, QUIET, WAIT};
use super::harness::{test_config, Harness};
use super::support::{fixture, with, THREAD};
use crate::codex_remote::launch::SOCKET_WAIT_TICKS;
use crate::codex_remote::rpc_client::{spawn, Timing};
use crate::codex_remote::ws::{self, Message};
use crate::codex_remote::{CodexEvent, Originator, UnavailableReason};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::sync::Arc;
use std::time::Duration;

fn the_thread() -> Value {
    fixture("thread-read-response")["result"].clone()
}

/// Joins the fake thread: it starts, goes active, and the third `thread/resume` succeeds after two `-32600`.
pub fn join(h: &mut Harness) {
    h.conn.send(&fixture("thread-started"));
    h.conn.send(&fixture("thread-status-active"));
    for attempt in 0..3 {
        let resume = h.conn.expect("thread/resume");
        assert_eq!(resume["params"], json!({"threadId": THREAD, "excludeTurns": true}), "only these keys, every attempt");
        if attempt < 2 {
            h.conn.send(&with(fixture("resume-error-32600"), "/id", resume["id"].clone()));
        } else {
            let thread = with(the_thread()["thread"].clone(), "/status", json!({"type": "active", "activeFlags": []}));
            h.conn.reply(&resume, json!({"thread": thread}));
        }
    }
    assert_eq!(h.until(|e| matches!(e, CodexEvent::Joined { .. }).then_some(e)), CodexEvent::Joined { thread_id: THREAD.into() });
    assert_eq!(h.next(), CodexEvent::Waiting { approval: false, active: true }, "the state at join is reported even though it was seen before");
}

#[test]
fn the_client_says_who_initialized_the_server_first_and_wakes_the_ui_for_it() {
    let h = Harness::start();
    assert_eq!(h.next(), CodexEvent::Originator(Originator::Tui));
    assert!(h.woken.load(Ordering::Relaxed) >= 1, "every event wakes an idle window");
    let h = Harness::start_with("designos-studio", "0.164.9", &[]);
    assert_eq!(h.next(), CodexEvent::Originator(Originator::Studio));
}

#[test]
fn a_server_of_another_release_line_or_an_unreadable_one_makes_the_client_passive() {
    for (version, shown) in [("0.165.0", "0.165.0"), ("0.163.7", "0.163.7"), ("garbage", "garbage")] {
        let mut h = Harness::start_with("codex-tui", version, &[]);
        assert_eq!(h.until(|e| matches!(e, CodexEvent::Unavailable(_)).then_some(e)), CodexEvent::Unavailable(UnavailableReason::ServerVersion(shown.into())));
        assert!(h.conn.silence().is_empty(), "nothing is written to a server Studio cannot vouch for");
        assert!(h.conn.hung_up());
    }
}

#[test]
fn without_a_socket_the_client_gives_up_after_the_connect_timeout() {
    let (tx, events) = channel();
    let missing = std::env::temp_dir().join("cxs-never-there.sock");
    spawn(test_config(&missing, Arc::new(AtomicBool::new(true)), Arc::new(|| {})), tx).unwrap();
    assert_eq!(events.recv_timeout(WAIT), Ok(CodexEvent::Unavailable(UnavailableReason::SocketTimeout)));
    assert_eq!(events.recv_timeout(WAIT), Err(RecvTimeoutError::Disconnected), "the client thread has ended");
}

#[test]
fn a_socket_that_shows_up_late_is_connected_after_the_grace() {
    let (tx, events) = channel();
    let path = std::env::temp_dir().join(format!("cxs-late-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&path);
    spawn(test_config(&path, Arc::new(AtomicBool::new(true)), Arc::new(|| {})), tx).unwrap();
    std::thread::sleep(QUIET / 2);
    let server = FakeServer::bind_at(&path);
    let mut conn = server.accept();
    let init = conn.expect("initialize");
    conn.reply(&init, fixture("initialize-response")["result"].clone());
    assert!(matches!(events.recv_timeout(WAIT), Ok(CodexEvent::Originator(_))));
}

#[test]
fn threads_are_found_in_the_loaded_list_and_in_thread_started_and_only_roots_count() {
    let mut h = Harness::start_with("codex-tui", "0.164.0", &[THREAD]);
    let read = h.conn.expect("thread/read");
    assert_eq!(read["params"], json!({"threadId": THREAD}));
    h.conn.reply(&read, the_thread());
    assert_eq!(h.until(|e| matches!(e, CodexEvent::ThreadReady).then_some(e)), CodexEvent::ThreadReady);

    let mut h = Harness::start();
    h.conn.send(&fixture("thread-started"));
    assert_eq!(h.until(|e| matches!(e, CodexEvent::ThreadReady).then_some(e)), CodexEvent::ThreadReady);

    let mut h = Harness::start_with("codex-tui", "0.164.0", &[THREAD]);
    let read = h.conn.expect("thread/read");
    let sub_agent = with(the_thread(), "/thread/parentThreadId", json!("some-parent-thread"));
    h.conn.reply(&read, sub_agent);
    h.conn.send(&fixture("thread-status-active"));
    assert!(h.conn.silence().is_empty(), "a sub-agent thread is never joined");
    assert!(!std::iter::from_fn(|| h.events.try_recv().ok()).any(|e| e == CodexEvent::ThreadReady), "and does not make the TUI's thread ready");
}

#[test]
fn the_client_joins_after_the_first_prompt_and_retries_while_the_thread_has_no_rollout() {
    let mut h = Harness::start();
    join(&mut h);
    assert!(h.conn.silence().is_empty(), "once joined it does not resume again");
}

#[test]
fn the_retries_stop_at_the_cap_and_the_next_active_status_starts_them_again() {
    let mut h = Harness::start();
    h.conn.send(&fixture("thread-started"));
    h.conn.send(&fixture("thread-status-active"));
    for _ in 0..20 {
        let resume = h.conn.expect("thread/resume");
        h.conn.send(&with(fixture("resume-error-32600"), "/id", resume["id"].clone()));
    }
    assert!(h.conn.silence().is_empty(), "no 21st attempt");
    h.conn.send(&fixture("thread-status-waiting"));
    assert_eq!(h.conn.expect("thread/resume")["params"]["threadId"], THREAD, "a new active status re-arms the join");
}

#[test]
fn a_ping_is_answered_with_the_same_payload_and_unreadable_frames_are_ignored() {
    let mut h = Harness::start();
    h.conn.send_raw(&ws::encode_frame(ws::OP_PING, b"are-you-there", None));
    assert_eq!(h.conn.recv_message(WAIT), Some(Message::Pong(b"are-you-there".to_vec())));
    h.conn.send_raw(&ws::encode_frame(ws::OP_TEXT, b"this is not json", None));
    h.conn.send(&fixture("thread-started"));
    assert_eq!(h.until(|e| matches!(e, CodexEvent::ThreadReady).then_some(e)), CodexEvent::ThreadReady);
    assert!(h.conn.silence().is_empty(), "garbage gets no reply");
}

#[test]
fn production_timing_connects_after_the_tui_and_outlasts_its_socket_wait() {
    let timing = Timing::PRODUCTION;
    assert_eq!((timing.grace, timing.connect_retry, timing.connect_timeout), (Duration::from_secs(1), Duration::from_millis(100), Duration::from_secs(10)));
    assert_eq!((timing.resume_retry, timing.max_resume_attempts), (Duration::from_millis(250), 20));
    let tui_waits = Duration::from_millis(50 * u64::from(SOCKET_WAIT_TICKS));
    assert!(timing.connect_timeout > tui_waits, "the client must not give up on a server the TUI is still waiting for");
}
