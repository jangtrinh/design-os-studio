//! Which server requests become Approve/Deny cards, and what the card says. Everything else stays in the terminal.

use super::support::{fixture, with, without, THREAD, TURN2};
use crate::codex_remote::card_mapping::{card_key, map_request, needs_item_lookup, CodexCard, MapContext, Mapped, OpenTabReason};
use crate::codex_remote::item_index::ItemIndex;
use crate::codex_remote::protocol::CardKind;
use crate::codex_remote::session_folder::SessionFolder;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::Path;

/// An index that has seen these `item/started` frames.
pub(super) fn index(frames: &[Value]) -> ItemIndex {
    let mut items = ItemIndex::default();
    for frame in frames {
        let params = &frame["params"];
        items.record(params["threadId"].as_str().unwrap(), params["turnId"].as_str().unwrap(), &params["item"]);
    }
    items
}

/// The session folder the recorded requests were made in.
pub(super) fn session_folder() -> SessionFolder {
    SessionFolder::new(Path::new("/session"))
}

fn map_in(request: &Value, items: &ItemIndex, flags: (bool, bool, bool), session: &SessionFolder) -> Mapped {
    let (ask, joined, seen) = flags;
    let roots: HashSet<String> = joined.then(|| THREAD.to_string()).into_iter().collect();
    let already: HashSet<String> = seen.then(|| card_key(7, &request["id"])).into_iter().collect();
    let ctx = MapContext { generation: 7, tab_title: "Codex CLI", ask_mode: ask, joined_roots: &roots, seen: &already, items, session };
    map_request(&ctx, &request["id"], request["method"].as_str().unwrap(), &request["params"])
}

pub(super) fn run(request: &Value, items: &ItemIndex, ask: bool, joined: bool, seen: bool) -> Mapped {
    map_in(request, items, (ask, joined, seen), &session_folder())
}

/// In Ask mode, from a joined thread, for a tab whose session folder is `session`.
pub(super) fn run_in(request: &Value, session: &SessionFolder) -> Mapped {
    map_in(request, &ItemIndex::default(), (true, true, false), session)
}

pub(super) fn map(request: &Value, items: &ItemIndex) -> Mapped {
    run(request, items, true, true, false)
}

pub(super) fn card(mapped: Mapped) -> CodexCard {
    match mapped {
        Mapped::Card(card) => card,
        Mapped::OpenTab(why) => panic!("expected a card, got Open tab ({why:?})"),
    }
}

pub(super) fn open_tab(mapped: Mapped) -> OpenTabReason {
    match mapped {
        Mapped::OpenTab(why) => why,
        Mapped::Card(card) => panic!("expected Open tab, got a card: {card:?}"),
    }
}

pub(super) fn mcp_items() -> ItemIndex {
    index(&[fixture("item-started-mcp")])
}

#[test]
fn an_mcp_tool_approval_is_titled_by_the_tool_of_the_matching_item() {
    let card = card(map(&fixture("mcp-elicitation-request"), &mcp_items()));
    assert_eq!((card.kind, card.key.as_str(), card.raw_id.clone()), (CardKind::McpTool, "cx-7-2", json!(2)));
    assert_eq!(card.title, "Codex CLI wants to use design:os designos_new_document");
    assert_eq!(card.summary, "designos · name: \"spike930b\"");
    assert!(!card.truncated);
    assert_eq!((card.thread_id.as_str(), card.turn_id.as_str()), (THREAD, TURN2));
    assert_eq!(card_key(3, &json!("abc")), "cx-3-\"abc\"", "a string id keeps its quotes, so 1 and \"1\" are different keys");
}

#[test]
fn an_mcp_request_that_is_not_a_plain_tool_approval_stays_in_the_terminal() {
    let base = fixture("mcp-elicitation-request");
    for request in [
        with(base.clone(), "/params/mode", json!("url")),
        with(base.clone(), "/params/_meta/codex_approval_kind", json!("browser_auth")),
        with(base.clone(), "/params/requestedSchema/properties", json!({"name": {"type": "string"}})),
        without(base.clone(), "/params/requestedSchema"),
        without(base.clone(), "/params/_meta"),
    ] {
        assert_eq!(open_tab(map(&request, &mcp_items())), OpenTabReason::Disqualified, "{request}");
    }
    assert_eq!(open_tab(map(&without(base.clone(), "/params/turnId"), &mcp_items())), OpenTabReason::Incomplete);
    assert_eq!(open_tab(map(&without(base, "/params/serverName"), &mcp_items())), OpenTabReason::Incomplete);
}

#[test]
fn the_tool_needs_exactly_one_in_progress_call_of_that_server_in_that_turn() {
    let request = fixture("mcp-elicitation-request");
    let started = fixture("item-started-mcp");
    let second = with(started.clone(), "/params/item/id", json!("call_second"));
    let other_server = with(started.clone(), "/params/item/server", json!("elsewhere"));
    let other_turn = with(started.clone(), "/params/turnId", json!("another-turn"));
    let done = with(started.clone(), "/params/item/status", json!("completed"));
    for (what, items) in [
        ("none seen", ItemIndex::default()),
        ("two calls", index(&[started.clone(), second])),
        ("other server", index(&[other_server])),
        ("other turn", index(&[other_turn])),
        ("completed", index(&[done])),
    ] {
        assert_eq!(open_tab(map(&request, &items)), OpenTabReason::ItemUnknown, "{what}");
    }
    let mut items = index(&[started]);
    assert_eq!(items.mcp_calls(THREAD, TURN2, "designos"), 1);
    items.finish_turn(THREAD, TURN2);
    assert_eq!(items.mcp_calls(THREAD, TURN2, "designos"), 0, "a finished turn's items are dropped");
    assert_eq!(open_tab(map(&request, &items)), OpenTabReason::ItemUnknown);
}

#[test]
fn only_a_plain_tool_approval_with_no_known_call_asks_for_a_lookup() {
    let roots: HashSet<String> = [THREAD.to_string()].into();
    let seen = HashSet::new();
    let lookup = |request: &Value, items: &ItemIndex, ask_mode: bool| {
        let session = session_folder();
        let ctx = MapContext { generation: 7, tab_title: "Codex CLI", ask_mode, joined_roots: &roots, seen: &seen, items, session: &session };
        needs_item_lookup(&ctx, &request["id"], request["method"].as_str().unwrap(), &request["params"])
    };
    let (request, none) = (fixture("mcp-elicitation-request"), ItemIndex::default());
    assert_eq!(lookup(&request, &none, true), Some((THREAD.to_string(), TURN2.to_string())));
    assert_eq!(lookup(&request, &mcp_items(), true), None, "the call is known");
    assert_eq!(lookup(&request, &none, false), None, "no buttons outside Ask mode");
    assert_eq!(lookup(&with(request.clone(), "/params/mode", json!("url")), &none, true), None, "not a tool approval");
    assert_eq!(lookup(&with(request, "/params/threadId", json!("other")), &none, true), None, "not a joined thread");
    assert_eq!(lookup(&fixture("command-approval-request"), &none, true), None, "a command names itself");
}

#[test]
fn a_command_approval_shows_the_command_exactly_as_sent() {
    let none = ItemIndex::default();
    let card = card(map(&fixture("command-approval-request"), &none));
    assert_eq!((card.kind, card.title.as_str(), card.summary.as_str()), (CardKind::Command, "Codex CLI wants to run a command", "/bin/zsh -lc 'echo hi'"));
    assert_eq!(card.key, "cx-7-1");
    let other = json!([{"type": "unknown", "command": "cd x"}, {"type": "unknown", "command": "ls"}]);
    assert_eq!(card_summary(with(fixture("command-approval-request"), "/params/commandActions", other)), "/bin/zsh -lc 'echo hi'");
    for ok in [json!([]), json!(null)] {
        assert!(
            matches!(map(&with(fixture("command-approval-request"), "/params/networkApprovalContext", ok), &none), Mapped::Card(_)),
            "empty network fields do not disqualify"
        );
    }
}

fn card_summary(request: Value) -> String {
    card(map(&request, &ItemIndex::default())).summary
}

#[test]
fn a_command_approval_that_is_more_than_a_plain_command_stays_in_the_terminal() {
    let base = fixture("command-approval-request");
    for request in [
        with(base.clone(), "/params/kind", json!("writeStdin")),
        with(base.clone(), "/params/networkApprovalContext", json!({"host": "example.com", "protocol": "https"})),
        with(base.clone(), "/params/proposedNetworkPolicyAmendments", json!([{"action": "allow", "host": "example.com"}])),
        with(base.clone(), "/params/availableDecisions", json!(["cancel"])),
    ] {
        assert_eq!(open_tab(map(&request, &ItemIndex::default())), OpenTabReason::Disqualified, "{request}");
    }
    let bare = without(without(base, "/params/commandActions"), "/params/command");
    assert_eq!(open_tab(map(&bare, &ItemIndex::default())), OpenTabReason::Incomplete, "no command text, no buttons");
}

#[test]
fn a_file_change_needs_no_extra_root_and_a_known_item_with_its_paths() {
    let items = index(&[fixture("item-started-file-change.synthetic")]);
    let card = card(map(&fixture("file-change-request.synthetic"), &items));
    assert_eq!((card.kind, card.title.as_str()), (CardKind::FileChange, "Codex CLI wants to change 2 files"));
    assert_eq!(card.summary, "add /session/notes/todo.md, edit /session/README.md");
    let grant = with(fixture("file-change-request.synthetic"), "/params/grantRoot", json!("/session"));
    assert_eq!(open_tab(map(&grant, &items)), OpenTabReason::Disqualified);
    assert_eq!(open_tab(map(&fixture("file-change-request.synthetic"), &ItemIndex::default())), OpenTabReason::ItemUnknown);
}

#[test]
fn a_request_from_an_unjoined_thread_a_tab_out_of_ask_mode_a_replay_or_another_method_gets_no_buttons() {
    let request = fixture("command-approval-request");
    let none = ItemIndex::default();
    assert_eq!(open_tab(run(&request, &none, true, false, false)), OpenTabReason::UnknownThread);
    assert_eq!(open_tab(run(&request, &none, false, true, false)), OpenTabReason::NotAsk);
    assert_eq!(open_tab(run(&request, &none, true, true, true)), OpenTabReason::Duplicate);
    for method in ["item/permissions/requestApproval", "item/tool/requestUserInput", "account/chatgptAuthTokens/refresh", "something/new"] {
        assert_eq!(open_tab(map(&with(request.clone(), "/method", json!(method)), &none)), OpenTabReason::Unsupported, "{method}");
    }
}
