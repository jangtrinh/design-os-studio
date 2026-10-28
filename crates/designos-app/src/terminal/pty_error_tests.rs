//! A tab whose process cannot start after Studio moved entries aside for it says so about both: the entries stay moved.

use super::pty_error_line;

#[test]
fn a_pty_error_after_a_move_names_what_was_moved() {
    let note = "Moved .mcp.json to .designos-set-aside/20261025-101500-a1b2c3 in this file's agent folder, so Grok won't run it.";
    assert_eq!(pty_error_line("openpty failed: no more ptys", Some(note)), format!("PTY error: openpty failed: no more ptys. {note}"));
}

#[test]
fn a_pty_error_with_nothing_moved_is_the_error_alone() {
    assert_eq!(pty_error_line("openpty failed: no more ptys", None), "PTY error: openpty failed: no more ptys");
}
