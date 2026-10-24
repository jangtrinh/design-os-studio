//! A file-change card says what each change does, the destructive ones first and never cut, or is not a card.

use super::mapping_tests::{card, index, map, open_tab};
use super::support::{fixture, with};
use crate::agent_permissions::MAX_INPUT_SUMMARY_CHARS;
use crate::codex_remote::card_mapping::{Mapped, OpenTabReason};
use serde_json::{json, Value};

fn changes(list: Value) -> Mapped {
    let item = with(fixture("item-started-file-change.synthetic"), "/params/item/changes", list);
    map(&fixture("file-change-request.synthetic"), &index(&[item]))
}

fn add(path: &str) -> Value {
    json!({"path": path, "kind": {"type": "add"}, "diff": ""})
}

fn edit(path: &str) -> Value {
    json!({"path": path, "kind": {"type": "update", "move_path": null}, "diff": ""})
}

fn delete(path: &str) -> Value {
    json!({"path": path, "kind": {"type": "delete"}, "diff": ""})
}

fn rename(path: &str, to: &str) -> Value {
    json!({"path": path, "kind": {"type": "update", "move_path": to}, "diff": ""})
}

/// A path of exactly `len` characters.
fn path_of(len: usize) -> String {
    format!("/{}", "p".repeat(len - 1))
}

#[test]
fn every_change_says_what_it_does_and_the_destructive_ones_come_first() {
    let all = json!([
        edit("/session/b.md"),
        add("/session/a.md"),
        delete("/session/c.md"),
        rename("/session/d.md", "/session/e.md"),
        edit("/session/f.md"),
        add("/session/g.md")
    ]);
    let want = "delete /session/c.md, move /session/d.md \u{2192} /session/e.md, add /session/a.md, add /session/g.md, edit /session/b.md, edit /session/f.md";
    assert_eq!(card(changes(all)).summary, want);
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
        assert_eq!(open_tab(changes(json!([add("/session/ok"), odd]))), OpenTabReason::Disqualified, "{odd}");
    }
}

#[test]
fn a_path_that_would_not_read_as_it_is_gets_no_buttons_wherever_it_stands() {
    let paths = [
        "/session/notes\nmd",
        "/session/a\rb",
        "/session/a\tb",
        "/session/a\u{A0}b",
        "/session/a\u{2003}b",
        "/session/a\u{2028}b",
        "/session/a\u{AD}b",
        "/session/a\u{200B}b",
        "/session/a\u{202E}b",
        "/session/a\u{FE0F}b",
        "/session/a\u{600}b",
        "/session/a\u{FFF9}b",
        "/session/a\u{13430}b",
        "/session/e\u{323}\u{302}",
        "/session/a\u{301}",
    ];
    for path in paths {
        assert_eq!(open_tab(changes(json!([add(path)]))), OpenTabReason::PathNotShownExactly, "add {path:?}");
        assert_eq!(open_tab(changes(json!([add("/session/ok"), delete(path)]))), OpenTabReason::PathNotShownExactly, "delete {path:?}");
        assert_eq!(open_tab(changes(json!([rename("/session/a", path)]))), OpenTabReason::PathNotShownExactly, "move target {path:?}");
        assert_eq!(open_tab(changes(json!([rename(path, "/session/b")]))), OpenTabReason::PathNotShownExactly, "move source {path:?}");
    }
}

#[test]
fn a_precomposed_vietnamese_path_is_a_card_and_the_same_name_decomposed_is_not() {
    let nfc = "/session/T\u{e0}i li\u{1ec7}u/\u{111}\u{1a1}n h\u{e0}ng.md";
    assert_eq!(card(changes(json!([add(nfc)]))).summary, format!("add {nfc}"));
    let nfd = "/session/Te\u{323}\u{302}.md";
    assert_eq!(open_tab(changes(json!([add(nfd)]))), OpenTabReason::PathNotShownExactly);
}

#[test]
fn a_summary_that_fits_exactly_is_a_card_and_one_more_character_is_not() {
    let fits = path_of(MAX_INPUT_SUMMARY_CHARS - "delete ".len());
    let shown = card(changes(json!([delete(&fits)])));
    assert_eq!((shown.summary.chars().count(), shown.truncated), (MAX_INPUT_SUMMARY_CHARS, false));
    let one_more = path_of(MAX_INPUT_SUMMARY_CHARS - "delete ".len() + 1);
    assert_eq!(open_tab(changes(json!([delete(&one_more)]))), OpenTabReason::ChangeCut);
    let add_fits = path_of(MAX_INPUT_SUMMARY_CHARS - "add ".len());
    assert!(!card(changes(json!([add(&add_fits)]))).truncated);
    assert_eq!(open_tab(changes(json!([add(&path_of(MAX_INPUT_SUMMARY_CHARS - "add ".len() + 1))]))), OpenTabReason::ChangeCut);
}

/// An edit or an add can be as dangerous as a delete (a shell startup file, a login item), and the cut drops what comes last.
#[test]
fn any_cut_of_the_summary_gets_no_buttons_whatever_the_cut_part_is() {
    let four_long_adds = json!([add(&path_of(60)), add(&path_of(61)), add(&path_of(62)), add(&path_of(63)), edit("/Users/me/.zshrc")]);
    assert_eq!(open_tab(changes(four_long_adds)), OpenTabReason::ChangeCut);
    assert_eq!(open_tab(changes(json!([edit(&path_of(190)), delete("/outside/project.designos")]))), OpenTabReason::ChangeCut);
    let two = path_of(93);
    assert_eq!(open_tab(changes(json!([delete(&two), delete(&two)]))), OpenTabReason::ChangeCut);
    assert_eq!(open_tab(changes(json!([rename("/session/a", &path_of(300))]))), OpenTabReason::ChangeCut);
    assert_eq!(open_tab(changes(json!([add(&path_of(300))]))), OpenTabReason::ChangeCut);
}

/// A path that contains the card's own separators can make one change read as two.
#[test]
fn a_path_that_contains_the_cards_own_separators_gets_no_buttons() {
    let probe = rename("/session/n.md, edit /session/x.md", "/Users/me/.zshrc");
    assert_eq!(open_tab(changes(json!([probe]))), OpenTabReason::PathNotShownExactly);
    for path in ["/session/a, b.md", "/session/a \u{2192} /Users/me/.zshrc", "/session/a, ", "/session/\u{2192}"] {
        assert_eq!(open_tab(changes(json!([add(path)]))), OpenTabReason::PathNotShownExactly, "{path:?}");
        assert_eq!(open_tab(changes(json!([rename("/session/a", path)]))), OpenTabReason::PathNotShownExactly, "target {path:?}");
        assert_eq!(open_tab(changes(json!([delete(path)]))), OpenTabReason::PathNotShownExactly, "{path:?}");
    }
    assert_eq!(
        card(changes(json!([add("/session/a,b.md"), add("/session/a ,b")]))).summary,
        "add /session/a,b.md, add /session/a ,b",
        "a comma alone is a character"
    );
}
