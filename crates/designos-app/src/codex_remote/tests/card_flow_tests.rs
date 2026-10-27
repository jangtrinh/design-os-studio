//! Requests become cards, a click answers exactly once, and anything without buttons is never answered.

use super::client_tests::join;
use super::fake_server::WAIT;
use super::harness::Harness;
use super::support::{fixture, with, THREAD, TURN1};
use crate::codex_remote::card_mapping::{CodexCard, OpenTabReason};
use crate::codex_remote::protocol::{CardKind, Decision};
use crate::codex_remote::rpc_client::ClientCommand;
use crate::codex_remote::CodexEvent;
use serde_json::{json, Value};
use std::sync::atomic::Ordering;

fn joined() -> Harness {
    let mut h = Harness::start();
    join(&mut h);
    h
}

fn card(h: &Harness) -> CodexCard {
    h.until(|e| if let CodexEvent::Card(card) = e { Some(card) } else { None })
}

fn click(h: &Harness, key: &str, decision: Decision) {
    h.client.send(ClientCommand::Answer { key: key.into(), decision });
}

/// The next frame the client wrote, byte for byte.
fn written(h: &mut Harness) -> String {
    h.conn.recv_text(WAIT).expect("a reply")
}

fn request_with_id(name: &str, id: u64) -> Value {
    with(fixture(name), "/id", json!(id))
}

#[test]
fn approve_and_deny_on_an_mcp_card_send_the_exact_one_shot_replies_once() {
    let mut h = joined();
    h.conn.send(&fixture("item-started-mcp"));
    h.conn.send(&fixture("thread-status-waiting"));
    h.conn.send(&fixture("mcp-elicitation-request"));
    let first = card(&h);
    assert_eq!((first.kind, first.key.as_str(), first.title.as_str()), (CardKind::McpTool, "cx-7-2", "Codex CLI wants to use design:os designos_new_document"));
    assert_eq!(first.summary, "designos · name: \"spike930b\"");
    click(&h, &first.key, Decision::Approve);
    assert_eq!(written(&mut h), r#"{"id":2,"result":{"action":"accept","content":null}}"#);
    click(&h, &first.key, Decision::Approve);
    click(&h, &first.key, Decision::Deny);
    assert!(h.conn.silence().is_empty(), "a card is answered once; later clicks write nothing");
    h.conn.send(&request_with_id("mcp-elicitation-request", 3));
    click(&h, &card(&h).key, Decision::Deny);
    assert_eq!(written(&mut h), r#"{"id":3,"result":{"action":"decline","content":null}}"#);
}

#[test]
fn command_and_file_change_cards_reply_with_a_decision() {
    let mut h = joined();
    h.conn.send(&fixture("command-approval-request"));
    let command = card(&h);
    assert_eq!((command.kind, command.summary.as_str()), (CardKind::Command, "/bin/zsh -lc 'echo hi'"));
    click(&h, &command.key, Decision::Approve);
    let mut replies = vec![written(&mut h)];
    assert_eq!(replies[0], r#"{"id":1,"result":{"decision":"accept"}}"#);
    h.conn.send(&request_with_id("command-approval-request", 11));
    click(&h, &card(&h).key, Decision::Deny);
    replies.push(written(&mut h));
    assert_eq!(replies[1], r#"{"id":11,"result":{"decision":"decline"}}"#);
    h.conn.send(&fixture("item-started-file-change.synthetic"));
    h.conn.send(&fixture("file-change-request.synthetic"));
    let files = card(&h);
    assert_eq!((files.kind, files.title.as_str()), (CardKind::FileChange, "Codex CLI wants to change 2 files"));
    click(&h, &files.key, Decision::Approve);
    replies.push(written(&mut h));
    assert_eq!(replies[2], r#"{"id":5,"result":{"decision":"accept"}}"#);
    for reply in &replies {
        for forbidden in ["acceptForSession", "Amendment", "cancel", "_meta", "persist"] {
            assert!(!reply.contains(forbidden), "{reply}");
        }
    }
}

#[test]
fn a_request_whose_item_was_not_seen_asks_for_the_turns_items_once() {
    let mut h = joined();
    h.conn.send(&fixture("mcp-elicitation-request-late"));
    let list = h.conn.expect("thread/items/list");
    assert_eq!(list["params"], json!({"threadId": THREAD, "turnId": TURN1, "sortDirection": "desc", "limit": 50}));
    h.conn.reply(&list, fixture("items-list-response.synthetic")["result"].clone());
    let late = card(&h);
    assert_eq!(
        (late.key.as_str(), late.title.as_str(), late.summary.as_str()),
        ("cx-7-0", "Codex CLI wants to use design:os designos_new_document", "designos · name: \"spike930\"")
    );
    assert!(h.conn.silence().is_empty(), "one lookup, and no reply before a click");
    h.conn.send(&with(request_with_id("mcp-elicitation-request-late", 20), "/params/turnId", json!("another-turn")));
    let list = h.conn.expect("thread/items/list");
    h.conn.send(&json!({"id": list["id"], "error": {"code": -32603, "message": "boom"}}));
    let event = h.until(|e| matches!(e, CodexEvent::OpenTabWait { .. }).then_some(e));
    assert_eq!(event, CodexEvent::OpenTabWait { key: "cx-7-20".into(), why: OpenTabReason::ItemUnknown }, "a failed lookup leaves the request to the terminal");
    assert!(h.conn.silence().is_empty());
}

#[test]
fn a_request_resolved_or_ended_while_its_items_are_looked_up_never_gets_a_card() {
    for ending in [fixture("request-resolved"), fixture("turn-completed")] {
        let mut h = joined();
        h.conn.send(&fixture("mcp-elicitation-request-late"));
        let list = h.conn.expect("thread/items/list");
        h.conn.send(&ending);
        h.conn.reply(&list, fixture("items-list-response.synthetic")["result"].clone());
        assert!(h.conn.silence().is_empty(), "the reply to the lookup is taken in; no card means no write");
        click(&h, "cx-7-0", Decision::Approve);
        assert!(h.conn.silence().is_empty(), "a click on a request that was already over writes nothing");
        assert!(!std::iter::from_fn(|| h.events.try_recv().ok()).any(|event| matches!(event, CodexEvent::Card(_))), "no card for it either");
    }
}

#[test]
fn a_resolved_request_and_a_finished_turn_remove_their_cards_and_a_late_click_sends_nothing() {
    let mut h = joined();
    h.conn.send(&fixture("item-started-mcp"));
    h.conn.send(&fixture("mcp-elicitation-request"));
    h.conn.send(&fixture("command-approval-request"));
    let (mcp, command) = (card(&h), card(&h));
    h.conn.send(&with(fixture("request-resolved"), "/params/requestId", json!(2)));
    assert_eq!(h.until(|e| matches!(e, CodexEvent::Resolved { .. }).then_some(e)), CodexEvent::Resolved { key: mcp.key.clone() });
    click(&h, &mcp.key, Decision::Approve);
    h.conn.send(&fixture("turn-completed"));
    assert_eq!(
        h.until(|e| matches!(e, CodexEvent::Resolved { .. }).then_some(e)),
        CodexEvent::Resolved { key: command.key.clone() },
        "the turn's other card goes with it"
    );
    click(&h, &command.key, Decision::Deny);
    assert!(h.conn.silence().is_empty());
}

#[test]
fn the_rpc_client_writes_nothing_for_requests_it_does_not_card() {
    let mut h = joined();
    h.conn.send(&fixture("item-started-mcp"));
    let command = fixture("command-approval-request");
    let sent = [
        (100, with(command.clone(), "/method", json!("item/permissions/requestApproval")), OpenTabReason::Unsupported),
        (101, with(command.clone(), "/method", json!("item/tool/requestUserInput")), OpenTabReason::Unsupported),
        (102, json!({"method": "something/new", "params": {}}), OpenTabReason::Unsupported),
        (103, with(fixture("mcp-elicitation-request"), "/params/mode", json!("url")), OpenTabReason::Disqualified),
        (104, with(command.clone(), "/params/networkApprovalContext", json!({"host": "example.com"})), OpenTabReason::Disqualified),
        (105, with(command.clone(), "/params/kind", json!("writeStdin")), OpenTabReason::Disqualified),
        (106, with(fixture("file-change-request.synthetic"), "/params/grantRoot", json!("/session")), OpenTabReason::Disqualified),
        (107, with(command.clone(), "/params/threadId", json!("another-thread")), OpenTabReason::UnknownThread),
    ];
    for (id, request, _) in &sent {
        h.conn.send(&with(request.clone(), "/id", json!(id)));
    }
    let writes = h.conn.silence();
    assert!(writes.is_empty(), "no reply at all, not even an error, for a request without buttons: {writes:?}");
    for (id, _, why) in sent {
        let event = h.until(|e| matches!(e, CodexEvent::OpenTabWait { .. }).then_some(e));
        assert_eq!(event, CodexEvent::OpenTabWait { key: format!("cx-7-{id}"), why });
    }
    h.conn.send(&with(command, "/id", json!(104)));
    assert!(h.no_event(), "a replayed request raises no second event");
    h.ask_mode.store(false, Ordering::Relaxed);
    h.conn.send(&request_with_id("command-approval-request", 108));
    assert_eq!(
        h.until(|e| matches!(e, CodexEvent::OpenTabWait { .. }).then_some(e)),
        CodexEvent::OpenTabWait { key: "cx-7-108".into(), why: OpenTabReason::NotAsk }
    );
    for key in ["cx-7-104", "cx-7-108", "cx-7-nope"] {
        click(&h, key, Decision::Approve);
    }
    assert!(h.conn.silence().is_empty(), "no reply at all, not even an error, for any of them");
}

#[test]
fn waiting_follows_the_thread_status_and_a_lost_connection_is_reported() {
    let mut h = joined();
    for (frame, want) in [("thread-status-waiting", (true, true)), ("thread-status-active", (false, true))] {
        h.conn.send(&fixture(frame));
        assert_eq!(h.next(), CodexEvent::Waiting { approval: want.0, active: want.1 });
    }
    h.conn.send(&fixture("thread-status-active"));
    h.conn.send(&with(fixture("thread-status-active"), "/params/status", json!({"type": "idle"})));
    assert_eq!(h.next(), CodexEvent::Waiting { approval: false, active: false }, "an unchanged state is not repeated");
    h.conn.close();
    assert_eq!(h.until(|e| matches!(e, CodexEvent::Disconnected).then_some(e)), CodexEvent::Disconnected);
    click(&h, "cx-7-1", Decision::Approve);
    h.client.send(ClientCommand::Shutdown);
}

#[test]
fn shutdown_closes_the_connection() {
    let mut h = joined();
    h.client.send(ClientCommand::Shutdown);
    assert!(h.conn.hung_up());
}
