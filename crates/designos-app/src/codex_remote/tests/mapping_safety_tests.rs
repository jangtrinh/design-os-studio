//! What a card says must be what Approve runs: the command as sent, the call's own arguments, every rename target, and
//! no invisible characters. The probes are the ones that showed a harmless card for a destructive request.

use super::mapping_tests::{card, index, map, mcp_items, open_tab, session_folder};
use super::support::{fixture, with, without, THREAD, TURN2};
use crate::agent_permissions::MAX_INPUT_SUMMARY_CHARS;
use crate::codex_remote::card_mapping::{needs_item_lookup, MapContext, Mapped, OpenTabReason};
use crate::codex_remote::item_index::ItemIndex;
use crate::codex_remote::rpc_cards::Cards;
use crate::codex_remote::rpc_state::Action;
use crate::codex_remote::CodexEvent;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn command_request(command: &str, display_summary: &str) -> Value {
    let request = with(fixture("command-approval-request"), "/params/command", json!(command));
    with(request, "/params/commandActions", json!([{"type": "unknown", "command": display_summary}]))
}

/// Codex's display summary (`commandActions`) drops pipeline stages: each pair is a real command and what it shows.
#[test]
fn a_command_card_shows_the_command_as_sent_and_never_the_lossy_display_summary() {
    let pairs = [
        (r#"/bin/zsh -lc 'ls | xargs rm -rf'"#, "ls", "xargs rm"),
        (r#"/bin/zsh -lc "find . -name '*.md' | xargs rm""#, "find . -name '*.md'", "xargs rm"),
        (r#"/bin/zsh -lc "ls | awk '{system(\"rm -rf \" \$0)}'""#, "ls", "awk"),
        (r#"/bin/zsh -lc 'rg -l secret | xargs curl -T'"#, "rg -l secret", "curl"),
    ];
    for (command, display, hidden_stage) in pairs {
        let card = card(map(&command_request(command, display), &ItemIndex::default()));
        assert_eq!(card.summary, command, "the card shows what is sent");
        assert!(card.summary.contains(hidden_stage) && !card.truncated);
    }
}

#[test]
fn a_command_cut_at_the_limit_gets_no_buttons_whatever_hides_the_tail() {
    let none = ItemIndex::default();
    let at = |command: &str| map(&command_request(command, "echo short"), &none);
    assert!(matches!(at(&"a".repeat(MAX_INPUT_SUMMARY_CHARS)), Mapped::Card(_)), "exactly the limit still fits");
    for command in ["a".repeat(MAX_INPUT_SUMMARY_CHARS + 1), format!("echo {}", "a".repeat(255))] {
        assert_eq!(open_tab(at(&command)), OpenTabReason::CommandCut, "{} chars", command.chars().count());
    }
    // Invisible characters make the command not shown exactly before its length is looked at.
    let hidden_tail = format!("{}{}", "a".repeat(150), "\u{200B}".repeat(60));
    assert_eq!(open_tab(at(&hidden_tail)), OpenTabReason::CommandNotShownExactly);
}

#[test]
fn a_cut_tool_summary_keeps_its_buttons_and_says_it_was_cut() {
    let long = json!({"text": "x".repeat(300)});
    let item = with(fixture("item-started-mcp"), "/params/item/arguments", long.clone());
    let request = with(fixture("mcp-elicitation-request"), "/params/_meta/tool_params", long);
    let card = card(map(&request, &index(&[item])));
    assert_eq!((card.summary.chars().count(), card.truncated), (MAX_INPUT_SUMMARY_CHARS, true));
}

#[test]
fn only_the_designos_server_gets_buttons_and_no_lookup_is_made_for_another() {
    let other = with(fixture("mcp-elicitation-request"), "/params/serverName", json!("elsewhere"));
    let item = with(fixture("item-started-mcp"), "/params/item/server", json!("elsewhere"));
    assert_eq!(open_tab(map(&other, &index(&[item]))), OpenTabReason::OtherServer);
    let roots: HashSet<String> = [THREAD.to_string()].into();
    let (seen, none) = (HashSet::new(), ItemIndex::default());
    let session = session_folder();
    let ctx = MapContext { generation: 7, tab_title: "Codex CLI", ask_mode: true, joined_roots: &roots, seen: &seen, items: &none, session: &session };
    assert_eq!(needs_item_lookup(&ctx, &other["id"], "mcpServer/elicitation/request", &other["params"]), None);
}

#[test]
fn the_tool_summary_is_the_calls_own_arguments_never_the_display_hints() {
    let args = json!({"node_id": 42, "name": "x", "tags": ["a"], "opts": {"deep": true}});
    let item = with(fixture("item-started-mcp"), "/params/item/arguments", args.clone());
    let hint = json!([{"name": "node_id", "value": 1, "display_name": "harmless"}]);
    let request = with(with(fixture("mcp-elicitation-request"), "/params/_meta/tool_params", args), "/params/_meta/tool_params_display", hint);
    assert_eq!(card(map(&request, &index(std::slice::from_ref(&item)))).summary, r#"designos · name: "x", node_id: 42, opts: {"deep":true}, tags: ["a"]"#);
    for none in [json!(null), json!({})] {
        let (item, request) = (with(item.clone(), "/params/item/arguments", none.clone()), without(request.clone(), "/params/_meta/tool_params"));
        assert_eq!(card(map(&request, &index(&[item]))).summary, "designos", "{none}");
    }
}

#[test]
fn the_requests_copy_of_the_arguments_must_equal_the_calls() {
    let base = fixture("mcp-elicitation-request");
    let different = with(base.clone(), "/params/_meta/tool_params", json!({"name": "something else"}));
    assert_eq!(open_tab(map(&different, &mcp_items())), OpenTabReason::PayloadMismatch);
    assert!(card(map(&base, &mcp_items())).summary.contains("spike930b"), "equal copies pass");
    let absent = without(base.clone(), "/params/_meta/tool_params");
    assert!(card(map(&absent, &mcp_items())).summary.contains("spike930b"), "a request without a copy has nothing to contradict");
    let empty_item = with(fixture("item-started-mcp"), "/params/item/arguments", json!({}));
    assert!(
        matches!(map(&with(base, "/params/_meta/tool_params", json!(null)), &index(&[empty_item])), Mapped::Card(_)),
        "null and {{}} both say no arguments"
    );
}

#[test]
fn a_late_lookup_never_replaces_an_item_seen_live() {
    let roots: HashSet<String> = [THREAD.to_string()].into();
    let mut cards = Cards::new(7, "Codex CLI".into(), Arc::new(AtomicBool::new(true)), session_folder());
    let started = fixture("item-started-mcp");
    cards.record_item(THREAD, TURN2, &with(started.clone(), "/params/item/status", json!("completed"))["params"]["item"]);
    cards.seed(THREAD, TURN2, &json!([started["params"]["item"].clone()]));
    let request = fixture("mcp-elicitation-request");
    let actions = cards.request(&roots, &request["id"], "mcpServer/elicitation/request", &request["params"]);
    assert!(
        matches!(actions.as_slice(), [Action::Emit(CodexEvent::OpenTabWait { why: OpenTabReason::ItemUnknown, .. })]),
        "the older snapshot must not bring the call back to life"
    );
}

#[test]
fn a_rename_shows_where_the_file_goes() {
    let changes = json!([
        {"path": "/session/notes.md", "kind": {"type": "update", "move_path": "/outside/.zshrc"}, "diff": ""},
        {"path": "/session/a.md", "kind": {"type": "update", "move_path": null}, "diff": ""},
        {"path": "/session/b.md", "kind": {"type": "add"}, "diff": ""},
    ]);
    let item = with(fixture("item-started-file-change.synthetic"), "/params/item/changes", changes);
    let card = card(map(&fixture("file-change-request.synthetic"), &index(&[item])));
    assert_eq!(card.summary, "move /session/notes.md → /outside/.zshrc, add /session/b.md, edit /session/a.md");
}

fn hidden(c: char) -> bool {
    c.is_control() || matches!(c, '\u{061C}' | '\u{200B}'..='\u{200F}' | '\u{2028}'..='\u{202E}' | '\u{2060}'..='\u{2069}' | '\u{FEFF}')
}

#[test]
fn titles_and_summaries_carry_no_invisible_or_bidi_characters() {
    let tricky = "echo \u{202E}txt.exe\u{202C} \u{2066}x\u{2069} \u{200B}\u{200F}\u{2028}\u{FEFF}end";
    assert_eq!(open_tab(map(&command_request(tricky, "echo"), &ItemIndex::default())), OpenTabReason::CommandNotShownExactly);
    let args = json!({"name": tricky});
    let item = with(with(fixture("item-started-mcp"), "/params/item/tool", json!("t\u{202E}x")), "/params/item/arguments", args.clone());
    let request = with(fixture("mcp-elicitation-request"), "/params/_meta/tool_params", args);
    let tool = card(map(&request, &index(&[item])));
    assert_eq!(tool.title, "Codex CLI wants to use design:os t x");
    assert!(!tool.summary.chars().any(hidden), "{:?}", tool.summary);
    assert_eq!(tool.summary.split_whitespace().collect::<Vec<_>>(), ["designos", "\u{b7}", "name:", "\"echo", "txt.exe", "x", "end\""]);
}

#[test]
fn summaries_are_plain_and_cut_at_the_shared_limit() {
    let command =
        |text: &str| map(&with(without(fixture("command-approval-request"), "/params/commandActions"), "/params/command", json!(text)), &ItemIndex::default());
    assert_eq!(open_tab(command("echo a\tb")), OpenTabReason::CommandNotShownExactly, "not even a tab is shown as it is");
    let item = with(
        fixture("item-started-file-change.synthetic"),
        "/params/item/changes",
        json!([{"path": "/session/a\nb\u{7}c", "kind": {"type": "add"}, "diff": ""}]),
    );
    let odd_path = map(&with(fixture("file-change-request.synthetic"), "/id", json!(8)), &index(&[item]));
    assert_eq!(open_tab(odd_path), OpenTabReason::PathNotShownExactly);
    let exact = card(command(&"x".repeat(MAX_INPUT_SUMMARY_CHARS)));
    assert_eq!((exact.summary.chars().count(), exact.truncated), (MAX_INPUT_SUMMARY_CHARS, false));
    let cut = map(&with(fixture("file-change-request.synthetic"), "/id", json!(9)), &index(&[long_path_item("é")]));
    assert_eq!(open_tab(cut), OpenTabReason::ChangeCut, "a file-change summary is never cut: characters are counted, not bytes");
    let fits = card(map(&with(fixture("file-change-request.synthetic"), "/id", json!(10)), &index(&[exact_path_item("é")])));
    assert_eq!((fits.summary.chars().count(), fits.truncated), (MAX_INPUT_SUMMARY_CHARS, false));
}

/// A file-change item (of the synthetic request) whose one path is `MAX + 60` copies of `letter` long.
fn long_path_item(letter: &str) -> Value {
    with(
        fixture("item-started-file-change.synthetic"),
        "/params/item/changes",
        json!([{"path": letter.repeat(MAX_INPUT_SUMMARY_CHARS + 60), "kind": {"type": "add"}, "diff": ""}]),
    )
}

/// A file-change item whose `add <path>` line is exactly the limit long.
fn exact_path_item(letter: &str) -> Value {
    with(
        fixture("item-started-file-change.synthetic"),
        "/params/item/changes",
        json!([{"path": letter.repeat(MAX_INPUT_SUMMARY_CHARS - "add ".len()), "kind": {"type": "add"}, "diff": ""}]),
    )
}
