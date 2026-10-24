//! A command or file-change card shows what Approve will do exactly, or is not a card.

use super::mapping_tests::{card, index, map, open_tab};
use super::support::{fixture, with};
use crate::codex_remote::card_mapping::OpenTabReason;
use crate::codex_remote::item_index::ItemIndex;
use serde_json::{json, Value};

fn command(text: &str) -> Value {
    with(fixture("command-approval-request"), "/params/command", json!(text))
}

#[test]
fn a_command_the_sanitizer_would_change_gets_no_buttons_because_it_would_not_read_the_same() {
    let none = ItemIndex::default();
    let probes = [
        "/bin/zsh -lc 'echo cleaning up\nrm -rf ..'",
        "/bin/zsh -lc 'echo cleaning up\rrm -rf ..'",
        "echo a\u{2028}rm -rf ..",
        "echo \u{202E}txt.exe",
        "echo \u{7}bell",
        "echo a\u{200B}b",
    ];
    for text in probes {
        assert_eq!(open_tab(map(&command(text), &none)), OpenTabReason::CommandNotShownExactly, "{text:?}");
    }
}

#[test]
fn a_tab_is_shown_as_a_space_and_the_command_still_gets_a_card() {
    let shown = card(map(&command("echo\ta\t\tb"), &ItemIndex::default()));
    assert_eq!(shown.summary, "echo a  b");
}

#[test]
fn a_command_that_cleans_to_nothing_is_incomplete() {
    for text in ["\u{200B}\u{200B}\u{2060}", "   ", "\n\n", "\t \u{FEFF}"] {
        assert_eq!(open_tab(map(&command(text), &ItemIndex::default())), OpenTabReason::Incomplete, "{text:?}");
    }
}

fn changes(list: Value) -> crate::codex_remote::card_mapping::Mapped {
    let item = with(fixture("item-started-file-change.synthetic"), "/params/item/changes", list);
    map(&fixture("file-change-request.synthetic"), &index(&[item]))
}

#[test]
fn every_change_says_what_it_does() {
    let all = json!([
        {"path": "/session/a.md", "kind": {"type": "add"}, "diff": ""},
        {"path": "/session/b.md", "kind": {"type": "update", "move_path": null}, "diff": ""},
        {"path": "/session/c.md", "kind": {"type": "delete"}, "diff": ""},
        {"path": "/session/d.md", "kind": {"type": "update", "move_path": "/session/e.md"}, "diff": ""},
    ]);
    assert_eq!(card(changes(all)).summary, "add /session/a.md, edit /session/b.md, delete /session/c.md, move /session/d.md → /session/e.md");
    let probe = json!([{"path": "/outside/project.designos", "kind": {"type": "delete"}}]);
    assert_eq!(card(changes(probe)).summary, "delete /outside/project.designos", "a delete no longer reads like an edit");
}

#[test]
fn a_change_of_a_kind_studio_cannot_describe_gets_no_buttons() {
    for odd in [
        json!({"path": "/session/a", "kind": {"type": "chmod"}}),
        json!({"path": "/session/a"}),
        json!({"path": "/session/a", "kind": "add"}),
        json!({"path": "/session/a", "kind": {"type": "delete", "move_path": "/elsewhere"}}),
        json!({"kind": {"type": "add"}}),
    ] {
        let list = json!([{"path": "/session/ok", "kind": {"type": "add"}}, odd]);
        assert_eq!(open_tab(changes(list)), OpenTabReason::Disqualified, "{odd}");
    }
}
