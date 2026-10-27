//! Studio's narrowest panel, where Gemini cuts its option labels to fit, and the cost of watching a Gemini tab's screen: a label that was
//! cut is still the dialog, and a tab that printed nothing is not read again.

use super::gemini_test_support::*;
use crate::gemini_screen::DIALOG_MARKER;
use crate::gemini_status_hooks::{self as hooks, SYSTEM_DEFAULTS_ENV};

/// The exec dialog of Gemini 0.65.0 in a main area `width` columns wide: a rounded box of that width, and in it rows of a 2-cell
/// indicator, a 2-cell number, a 1-cell gap and the label. The label gets `width - 9` cells and is cut with an ellipsis when it does not
/// fit, never wrapped (`BaseSelectionList.tsx:122-144`, `RadioButtonSelect.tsx:103`, `ToolConfirmationQueue.tsx:79`). The terminal is
/// `width` columns wide, or `width + 1` with the alternate screen.
fn dialog_at(width: usize) -> String {
    let cut = |text: &str, room: usize| -> String {
        if text.chars().count() <= room {
            text.to_string()
        } else {
            text.chars().take(room - 1).chain(std::iter::once('…')).collect()
        }
    };
    let row = |text: String| format!("│ {:<inner$} │", cut(&text, width - 4), inner = width - 4);
    let option = |index: usize, label: &str| row(format!("{}{index}. {}", if index == 1 { "● " } else { "  " }, cut(label, width - 9)));
    let lines = [
        format!("╭{}╮", "─".repeat(width - 2)),
        row("Action Required".to_string()),
        row("? Shell rm -rf build".to_string()),
        row("Allow execution of: 'rm'?".to_string()),
        option(1, "Allow once"),
        option(2, "Allow for this session"),
        option(3, DIALOG_MARKER),
        format!("╰{}╯", "─".repeat(width - 2)),
    ];
    format!("{CLEAR}{}", lines.join("\r\n"))
}

/// The panel widths in columns Studio reaches, and whether Gemini runs with its alternate screen (which takes a column off its width).
/// 30 columns is the 260 px minimum. The ellipsis cuts the label to "No, suggest changes …" at 30 (21 cells for the label) and to
/// "No, suggest changes…" at 29 (20 cells), the narrowest the line is found at without the alternate screen.
const WIDTHS: [(usize, bool); 6] = [(34, false), (33, false), (30, false), (30, true), (29, false), (60, false)];

#[test]
fn a_label_cut_to_fit_is_still_the_dialog_down_to_the_minimum_panel_width() {
    let (dir, mut st) = start_gemini("cut", ASK, |_| {});
    let mut failures = Vec::new();
    for (columns, alternate_screen) in WIDTHS {
        st.tabs[0].resize(20, columns as u16);
        show(&mut st, 0, &dialog_at(columns - usize::from(alternate_screen)));
        if !st.tabs[0].gemini_dialog_open() {
            failures.push(format!("{columns} columns, alternate screen {alternate_screen}: the dialog was missed"));
        } else if send(&mut st) != REFUSED {
            failures.push(format!("{columns} columns, alternate screen {alternate_screen}: the composer sent into the dialog"));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
    show(&mut st, 0, CLEAR);
    assert!(!st.tabs[0].gemini_dialog_open());
    finish(dir, st);
}

#[test]
fn a_hook_wait_on_a_cut_label_stands_while_the_dialog_is_on_screen_and_ends_when_it_is_gone() {
    let (dir, mut st) = start_gemini("cut-wait", ASK, |_| {});
    st.tabs[0].resize(20, 30);
    show(&mut st, 0, &dialog_at(30));
    ask_permission(&mut st);
    std::thread::sleep(QUIET);
    poll(&mut st);
    assert!(waiting(&st, 0) && st.needs_approval(), "Needs approval ended with the cut dialog on screen");
    assert_eq!(send(&mut st), REFUSED);
    dialog_goes(&mut st, 0);
    assert!(!waiting(&st, 0), "the wait outlived the dialog: the cut label was never seen");
    finish(dir, st);
}

#[test]
fn a_tab_with_no_hooks_file_starts_its_wait_from_a_cut_label() {
    let none_such = std::env::temp_dir().join(format!("designos-gemini-cut-admin-{}", std::process::id()));
    let (dir, mut st) = hooks::with_system_config(&[(SYSTEM_DEFAULTS_ENV, "/admin/defaults.json")], &none_such, || start_gemini("cut-nohooks", ASK, |_| {}));
    assert!(!st.tabs[0].gemini.hooks);
    st.tabs[0].resize(20, 33);
    show(&mut st, 0, &dialog_at(33));
    poll(&mut st);
    assert!(waiting(&st, 0) && st.needs_approval(), "a cut label started no wait");
    assert_eq!(send(&mut st), REFUSED);
    finish(dir, st);
}

#[test]
fn a_tab_that_printed_nothing_is_not_read_again() {
    let (dir, mut st) = start_gemini("reads", ASK, |_| {});
    poll(&mut st);
    let first = st.tabs[0].gemini.reads;
    for _ in 0..5 {
        poll(&mut st);
    }
    assert_eq!(st.tabs[0].gemini.reads, first, "a tab that printed nothing was read again");
    show(&mut st, 0, &dialog_screen(1, 1));
    poll(&mut st);
    assert_eq!(st.tabs[0].gemini.reads, first + 1, "new output was not read");
    poll(&mut st);
    assert_eq!(st.tabs[0].gemini.reads, first + 1, "the same output was read twice");
    st.tabs[0].resize(30, 100);
    poll(&mut st);
    assert_eq!(st.tabs[0].gemini.reads, first + 2, "a resize was not read");
    finish(dir, st);
}

#[test]
fn output_the_cli_prints_through_its_own_terminal_is_read() {
    let none_such = std::env::temp_dir().join(format!("designos-gemini-printed-admin-{}", std::process::id()));
    let (dir, mut st) =
        hooks::with_system_config(&[(SYSTEM_DEFAULTS_ENV, "/admin/defaults.json")], &none_such, || start_stub("printed", ASK, STUB_DIALOG, |_| {}));
    let printed = st.session_dir.join("printed.txt");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !printed.is_file() {
        assert!(std::time::Instant::now() < deadline, "the stub never printed");
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    std::thread::sleep(std::time::Duration::from_millis(300));
    poll(&mut st);
    assert!(waiting(&st, 0), "the dialog the CLI printed was not read: the reader thread did not announce its output");
    finish(dir, st);
}
