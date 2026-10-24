//! Gemini versions whose text and payloads were read from source (0.35.x and 0.65.x) get the screen reading of `gemini_dialog`. For any
//! other version, a missing one included, the hooks alone drive the status and a tab with no hooks file starts no wait, but the composer
//! still refuses while a dialog is on the screen. The version is the one the tests name; none runs a CLI.

use super::gemini_test_support::*;
use super::*;
use crate::agent_composer::SendOutcome;
use crate::gemini_dialog::{version_in_range, with_gemini_version, GONE_DEBOUNCE};
use crate::gemini_screen::{DIALOG_MARKER, DIALOG_PREFIX};
use crate::gemini_status_hooks::{self as hooks, SYSTEM_DEFAULTS_ENV};

#[test]
fn only_gemini_0_35_and_0_65_are_in_the_tested_range() {
    let table = [
        ("0.35.0", true),
        ("0.35.3", true),
        (" 0.35.9\n", true),
        ("0.65.0", true),
        ("0.65.7", true),
        ("0.34.9", false),
        ("0.36.0", false),
        ("0.50.2", false),
        ("0.64.9", false),
        ("0.66.0-preview.0", false),
        ("0.67.0-nightly.1", false),
        ("1.35.0", false),
        ("1.65.0", false),
        ("", false),
        ("garbage", false),
    ];
    for (version, expected) in table {
        assert_eq!(version_in_range(Some(version)), expected, "{version:?}");
    }
    assert!(!version_in_range(None), "an unknown version is outside the range");
}

#[test]
fn the_marker_is_the_line_gemini_draws_last_in_every_dialog_with_options() {
    // The last option of the edit, exec, info and MCP dialogs (`ToolConfirmationMessage.tsx:270, 296, 321, 352` in 0.35.3) and of the same
    // four and the sandbox expansion dialog (321, 347, 378, 403, 433 in 0.65.0).
    assert_eq!(DIALOG_MARKER, "No, suggest changes (esc)");
    assert!(dialog_screen(1, 1).contains(DIALOG_MARKER) && dialog_screen(2, 2).contains(DIALOG_MARKER));
    // What the screen is searched for is the marker's first three whole words.
    assert!(DIALOG_MARKER.starts_with(DIALOG_PREFIX) && DIALOG_MARKER[DIALOG_PREFIX.len()..].starts_with(' '));
    assert_eq!(DIALOG_PREFIX.split(' ').count(), 3);
}

#[test]
fn outside_the_tested_range_the_hooks_alone_drive_the_status() {
    for version in [Some("0.40.1"), Some("0.64.9"), None] {
        with_gemini_version(version, || {
            let (dir, mut st) = start_gemini("range", ASK, |_| {});
            let id = st.tabs[0].id;
            show(&mut st, 0, &dialog_screen(1, 1));
            ask_permission(&mut st);
            // The dialog goes and nothing ends the wait: the wait logic does not read the screen.
            show(&mut st, 0, CLEAR);
            poll(&mut st);
            std::thread::sleep(GONE_DEBOUNCE * 2);
            poll(&mut st);
            assert!(waiting(&st, 0), "{version:?}: the wait ended on the screen outside the tested range");
            assert_eq!(send(&mut st), REFUSED);
            send_record(&mut st, id, "PreToolUse", r#"{"session_id":"g"}"#);
            assert!(matches!(st.tabs[0].effective_status(), AgentTabStatus::Working(_)), "{version:?}");
            finish(dir, st);
        });
    }
}

#[test]
fn the_composer_refuses_a_dialog_on_screen_whatever_the_gemini_version() {
    for version in [None, Some(""), Some("garbage"), Some("0.36.0"), Some("0.64.9"), Some("0.66.0"), Some("0.35.3"), Some("0.65.0")] {
        with_gemini_version(version, || {
            let (dir, mut st) = start_gemini("any-version", ASK, |_| {});
            show(&mut st, 0, &dialog_screen(1, 1));
            assert_eq!(st.tabs[0].effective_status(), AgentTabStatus::Idle, "{version:?}");
            assert_eq!(send(&mut st), REFUSED, "{version:?}: the composer sent into an open dialog");
            show(&mut st, 0, CLEAR);
            assert!(matches!(send(&mut st), SendOutcome::Written), "{version:?}: an empty screen refused a send");
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
        assert_eq!(send(&mut st), REFUSED, "and the composer still refuses it");
    });
    // In range, the same screen starts one.
    poll(&mut st);
    assert!(waiting(&st, 0));
    finish(dir, st);
}
