//! The structured fields of a card (#930): what the panel paints is read from these, never from terminal text.

use super::mapping_tests::{card, index, map, mcp_items, run_in, session_folder};
use super::support::{fixture, with, without};
use crate::codex_remote::file_changes::{count_diff_lines, FileLine};
use serde_json::json;

#[test]
fn a_command_card_carries_the_folder_it_runs_in() {
    let request = with(with(fixture("command-approval-request"), "/params/command", json!("rm -rf build")), "/params/cwd", json!("/session"));
    let shown = card(run_in(&request, &session_folder()));
    assert_eq!((shown.summary.as_str(), shown.detail.cwd.as_deref()), ("rm -rf build", Some("/session")));
    assert!(shown.detail.tool.is_none() && shown.detail.input.is_none() && shown.detail.files.is_empty());
}

#[test]
fn an_mcp_card_carries_the_tool_and_the_arguments_as_compact_json() {
    let shown = card(map(&fixture("mcp-elicitation-request"), &mcp_items()));
    assert_eq!(shown.title, "Codex CLI wants to use design:os designos_new_document");
    assert_eq!((shown.detail.tool.as_deref(), shown.detail.input.as_deref()), (Some("designos_new_document"), Some(r#"{"name":"spike930b"}"#)));
    assert!(shown.detail.cwd.is_none());
}

#[test]
fn a_tool_call_without_arguments_has_no_input_line() {
    let mut item = fixture("item-started-mcp");
    item["params"]["item"]["arguments"] = json!({});
    let request = without(fixture("mcp-elicitation-request"), "/params/_meta/tool_params");
    let shown = card(map(&request, &index(&[item])));
    assert_eq!(shown.detail.input, None);
}

#[test]
fn a_file_change_card_lists_each_path_with_counts_from_its_own_diff() {
    let items = index(&[fixture("item-started-file-change.synthetic")]);
    let shown = card(map(&fixture("file-change-request.synthetic"), &items));
    assert_eq!(shown.title, "Codex CLI wants to change 2 files");
    assert_eq!(
        shown.detail.files,
        vec![
            FileLine { verb: "add", path: "/session/notes/todo.md".into(), counts: Some((1, 0)) },
            FileLine { verb: "edit", path: "/session/README.md".into(), counts: Some((1, 1)) },
        ]
    );
}

#[test]
fn one_changed_file_is_named_in_the_singular() {
    let item = with(fixture("item-started-file-change.synthetic"), "/params/item/changes", json!([{"path": "/session/a.md", "kind": {"type": "add"}}]));
    let shown = card(map(&fixture("file-change-request.synthetic"), &index(&[item])));
    assert_eq!(shown.title, "Codex CLI wants to change 1 file");
    assert_eq!(shown.detail.files[0].counts, None, "a change without a diff has no counts to show");
}

#[test]
fn diff_counts_skip_a_file_header_and_count_hunks() {
    assert_eq!(count_diff_lines("+hello\n"), (1, 0));
    assert_eq!(count_diff_lines("@@ -1 +1 @@\n-old\n+new\n"), (1, 1));
    assert_eq!(count_diff_lines("--- a/x\n+++ b/x\n@@ -1,2 +1,3 @@\n a\n-b\n+c\n+d\n"), (2, 1));
    assert_eq!(count_diff_lines(""), (0, 0));
}
