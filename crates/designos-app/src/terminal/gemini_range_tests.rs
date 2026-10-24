//! Outside the tested Gemini range (0.35.x) nothing reads the dialog from the screen: the hooks alone drive the status, a tab with
//! no hooks file starts no wait, and the composer is not closed by a marker. The version is the one the tests name; none runs a CLI.

use super::gemini_test_support::*;
use super::*;
use crate::agent_composer::SendOutcome;
use crate::gemini_dialog::{version_in_range, with_gemini_version, DIALOG_MARKER, GONE_DEBOUNCE};
use crate::gemini_status_hooks::{self as hooks, SYSTEM_DEFAULTS_ENV};

#[test]
fn only_gemini_0_35_is_in_the_tested_range() {
    let table = [
        ("0.35.0", true),
        ("0.35.3", true),
        (" 0.35.9\n", true),
        ("0.34.9", false),
        ("0.36.0", false),
        ("0.40.1", false),
        ("1.35.0", false),
        ("", false),
        ("garbage", false),
    ];
    for (version, expected) in table {
        assert_eq!(version_in_range(Some(version)), expected, "{version:?}");
    }
    assert!(!version_in_range(None), "an unknown version is outside the range");
}

#[test]
fn the_marker_is_the_line_gemini_0_35_draws_last_in_every_dialog_with_options() {
    // The last option of the edit, exec, info and MCP dialogs (`ToolConfirmationMessage.tsx:270, 296, 321, 352` in 0.35.3).
    assert_eq!(DIALOG_MARKER, "No, suggest changes (esc)");
    assert!(dialog_screen(1, 1).contains(DIALOG_MARKER) && dialog_screen(2, 2).contains(DIALOG_MARKER));
}

#[test]
fn outside_the_tested_range_the_hooks_alone_drive_the_status() {
    for version in [Some("0.40.1"), None] {
        with_gemini_version(version, || {
            let (dir, mut st) = start_gemini("range", ASK, |_| {});
            let id = st.tabs[0].id;
            show(&mut st, 0, &dialog_screen(1, 1));
            ask_permission(&mut st);
            // The dialog goes and nothing ends the wait: the marker is not used.
            show(&mut st, 0, CLEAR);
            poll(&mut st);
            std::thread::sleep(GONE_DEBOUNCE * 2);
            poll(&mut st);
            assert!(waiting(&st, 0), "{version:?}: the wait ended on the screen outside the tested range");
            assert_eq!(send(&mut st), REFUSED);
            send_record(&mut st, id, "PreToolUse", r#"{"session_id":"g"}"#);
            assert!(matches!(st.tabs[0].effective_status(), AgentTabStatus::Working(_)), "{version:?}");
            // An open dialog does not close the composer either: only the status says so.
            show(&mut st, 0, &dialog_screen(1, 1));
            assert!(!st.tabs[0].gemini_dialog_open(), "{version:?}: the marker was used");
            assert!(!matches!(send(&mut st), SendOutcome::Refused(_)), "{version:?}: the composer was closed by the marker");
            finish(dir, st);
        });
    }
}

#[test]
fn outside_the_tested_range_a_tab_with_no_hooks_file_starts_no_wait() {
    let none_such = std::env::temp_dir().join(format!("designos-gemini-range-admin-{}", std::process::id()));
    let (dir, mut st) = hooks::with_system_config(&[(SYSTEM_DEFAULTS_ENV, "/admin/defaults.json")], &none_such, || start_gemini("range-admin", ASK, |_| {}));
    assert!(!st.tabs[0].gemini.hooks);
    with_gemini_version(Some("0.36.0"), || {
        show(&mut st, 0, &dialog_screen(1, 1));
        poll(&mut st);
        assert!(!waiting(&st, 0), "a dialog on screen started a wait outside the tested range");
    });
    // In range, the same screen does.
    poll(&mut st);
    assert!(waiting(&st, 0));
    finish(dir, st);
}
