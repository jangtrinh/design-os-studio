//! Figma clipboard paste through the window: the HTML flavour is read through `App::system_html`, never
//! the OS pasteboard, from Cmd+V (with or without a text paste event) and from Edit > Paste.
//! Cmd+V sequences follow what egui-winit really emits, see the comment above `paste_chord`.

use super::*;
use designos_fig::container::Compression;
use std::sync::atomic::{AtomicUsize, Ordering};

fn figma_payload() -> Option<String> {
    Some(crate::clipboard::tests::figma_html(&designos_fig::testkit::sample_fig(Compression::Zstd, true)))
}

type Ids = std::collections::BTreeSet<NodeId>;

fn ids(s: &Sim) -> Ids {
    s.app().doc.nodes.keys().copied().collect()
}

/// Sorted names of the nodes that appeared since `before`.
fn names_added(s: &Sim, before: &Ids) -> Vec<String> {
    let mut v: Vec<String> = s.app().doc.nodes.iter().filter(|(k, _)| !before.contains(k)).map(|(_, n)| n.name.to_string()).collect();
    v.sort();
    v
}

fn key_v(pressed: bool, modifiers: Modifiers) -> Event {
    Event::Key { key: Key::V, physical_key: Some(Key::V), pressed, repeat: false, modifiers }
}

#[test]
fn a_paste_event_pastes_the_scripted_figma_html_as_layers() {
    let mut s = Sim::new();
    s.shell.app.system_html = figma_payload;
    let before = ids(&s);
    s.frame(vec![Event::Paste("plain text beside the figma html".into())]);
    let names = names_added(&s, &before);
    assert!(names.iter().any(|n| n == "Button") && names.iter().any(|n| n == "Button copy"), "{names:?}");
}

#[test]
fn a_paste_event_without_figma_html_pastes_only_its_text() {
    let mut s = Sim::new();
    let before = ids(&s);
    s.frame(vec![Event::Paste("just some words".into())]);
    let names = names_added(&s, &before);
    assert_eq!(names, vec!["just some words"], "one text layer and no Figma layers");
}

// The key sequences below are what egui-winit 0.35 really emits for a paste chord:
// - `src/lib.rs:1000-1015`: a V *press* with `modifiers.command` becomes `Event::Paste(text)` when the clipboard
//   text is non-empty, and nothing at all otherwise; no `Event::Key { pressed: true }` follows either way.
// - `src/lib.rs:1018`: the V *release* is an ordinary `Event::Key { pressed: false }`, with the modifiers held at
//   that moment (winit 0.30 `platform_impl/macos/app.rs:25-34` still delivers a keyUp while Cmd is down).
// So a Figma payload that arrives without plain text is only visible to the app on the release.

/// Runs one real paste chord: the press frame carries the text paste (when the clipboard has plain text), the
/// release frame the V release. `held_at_release` are the modifiers still down when V goes up.
fn paste_chord(s: &mut Sim, text: Option<&str>, held: Modifiers, held_at_release: Modifiers) {
    s.mods = held;
    s.frame(text.map(|t| vec![Event::Paste(t.into())]).unwrap_or_default());
    s.mods = held_at_release;
    s.frame(vec![key_v(false, held_at_release)]);
}

macro_rules! counted_figma_clipboard {
    ($reads:ident) => {
        static $reads: AtomicUsize = AtomicUsize::new(0);
        fn counted() -> Option<String> {
            $reads.fetch_add(1, Ordering::SeqCst);
            figma_payload()
        }
    };
}

fn button_count(s: &Sim, before: &Ids) -> usize {
    names_added(s, before).iter().filter(|n| *n == "Button").count()
}

#[test]
fn cmd_v_with_only_figma_html_pastes_when_v_is_released() {
    let mut s = Sim::new();
    s.shell.app.system_html = figma_payload;
    let before = ids(&s);
    s.mods = Modifiers::COMMAND;
    s.frame(vec![]); // the press: egui-winit pushes nothing without plain text
    assert_eq!(ids(&s), before, "nothing to act on before the release");
    s.frame(vec![key_v(false, Modifiers::COMMAND)]);
    assert_eq!(button_count(&s, &before), 1, "{:?}", names_added(&s, &before));
}

#[test]
fn cmd_v_with_neither_plain_text_nor_figma_html_does_nothing() {
    let mut s = Sim::new();
    let before = ids(&s);
    paste_chord(&mut s, None, Modifiers::COMMAND, Modifiers::COMMAND);
    assert_eq!(ids(&s), before);
}

#[test]
fn cmd_v_with_plain_text_pastes_once_across_press_and_release() {
    counted_figma_clipboard!(READS);
    let mut s = Sim::new();
    s.shell.app.system_html = counted;
    let before = ids(&s);
    paste_chord(&mut s, Some("text"), Modifiers::COMMAND, Modifiers::COMMAND);
    assert_eq!(button_count(&s, &before), 1, "one Button, not two: {:?}", names_added(&s, &before));
    assert_eq!(READS.load(Ordering::SeqCst), 1, "one clipboard read for one paste");
}

#[test]
fn a_text_paste_and_the_v_release_in_one_frame_paste_once() {
    counted_figma_clipboard!(READS);
    let mut s = Sim::new();
    s.shell.app.system_html = counted;
    let before = ids(&s);
    s.mods = Modifiers::COMMAND;
    s.frame(vec![Event::Paste("text".into()), key_v(false, Modifiers::COMMAND)]);
    assert_eq!(button_count(&s, &before), 1);
    assert_eq!(READS.load(Ordering::SeqCst), 1);
}

#[test]
fn releasing_cmd_before_v_still_pastes_a_figma_html_only_clipboard_once() {
    counted_figma_clipboard!(READS);
    let mut s = Sim::new();
    s.shell.app.system_html = counted;
    let before = ids(&s);
    s.mods = Modifiers::COMMAND;
    s.frame(vec![]); // press
    paste_chord_release_after_cmd_up(&mut s);
    assert_eq!(button_count(&s, &before), 1);
    assert_eq!(READS.load(Ordering::SeqCst), 1);

    // the same order with a plain-text paste does not paste a second time on the late release
    let before = ids(&s);
    paste_chord(&mut s, Some("text"), Modifiers::COMMAND, Modifiers::NONE);
    assert_eq!(button_count(&s, &before), 1, "{:?}", names_added(&s, &before));
    assert_eq!(READS.load(Ordering::SeqCst), 2, "one more read, for one more paste");
}

fn paste_chord_release_after_cmd_up(s: &mut Sim) {
    s.mods = Modifiers::NONE;
    s.frame(vec![key_v(false, Modifiers::NONE)]);
}

#[test]
fn a_plain_v_tap_never_reads_the_clipboard() {
    counted_figma_clipboard!(READS);
    let mut s = Sim::new();
    s.shell.app.system_html = counted;
    let before = ids(&s);
    s.frame(vec![key_v(true, Modifiers::NONE)]);
    s.frame(vec![key_v(false, Modifiers::NONE)]);
    s.frame(vec![key_v(false, Modifiers::NONE)]); // a stray release
    assert_eq!(READS.load(Ordering::SeqCst), 0);
    assert_eq!(ids(&s), before);
}

#[test]
fn pressing_cmd_after_a_plain_v_press_does_not_turn_its_release_into_a_paste() {
    counted_figma_clipboard!(READS);
    let mut s = Sim::new();
    s.shell.app.system_html = counted;
    s.frame(vec![key_v(true, Modifiers::NONE)]);
    s.mods = Modifiers::COMMAND;
    s.frame(vec![]);
    s.frame(vec![key_v(false, Modifiers::COMMAND)]);
    assert_eq!(READS.load(Ordering::SeqCst), 0);
}

#[test]
fn losing_window_focus_forgets_a_plain_v_press_whose_release_never_came() {
    let mut s = Sim::new();
    s.shell.app.system_html = figma_payload;
    let before = ids(&s);
    s.frame(vec![key_v(true, Modifiers::NONE)]);
    s.frame(vec![Event::WindowFocused(false)]);
    s.frame(vec![Event::WindowFocused(true)]);
    paste_chord(&mut s, None, Modifiers::COMMAND, Modifiers::COMMAND);
    assert_eq!(button_count(&s, &before), 1);
}

#[test]
fn shift_cmd_v_uses_paste_over_selection_and_does_not_read_system_clipboard() {
    // Real sequences: with text, Event::Paste on the press; without, only the release. Both events carry no
    // modifiers, so the shell uses the frame modifiers to route Shift+Cmd+V to the in-app paste-over command.
    // That command intentionally ignores system text/HTML and does nothing when no in-app copy exists.
    counted_figma_clipboard!(READS);
    let shift_cmd = Modifiers { shift: true, ..Modifiers::COMMAND };
    let mut s = Sim::new();
    s.shell.app.system_html = counted;
    let before = ids(&s);
    paste_chord(&mut s, Some("text"), shift_cmd, shift_cmd);
    assert_eq!(button_count(&s, &before), 0, "Shift+Cmd+V must not import system text");
    assert_eq!(READS.load(Ordering::SeqCst), 0, "Shift+Cmd+V must not read system HTML");

    let before = ids(&s);
    paste_chord(&mut s, None, shift_cmd, shift_cmd);
    assert_eq!(button_count(&s, &before), 0, "Shift+Cmd+V must not import Figma HTML: {:?}", names_added(&s, &before));
    assert_eq!(READS.load(Ordering::SeqCst), 0, "Shift+Cmd+V must not read system HTML");
}

#[test]
fn shift_cmd_v_with_html_only_uses_the_in_app_copy_over_selection() {
    counted_figma_clipboard!(READS);
    let mut s = Sim::new();
    let source = s.by_name("Button");
    let target = s.by_name("Card");
    s.shell.app.selection = vec![source];
    s.shell.app.copy_selection().expect("copy source layers");
    s.shell.app.system_html = counted;
    s.shell.app.selection = vec![target];
    let before = s.app().doc.nodes.len();

    let shift_cmd = Modifiers { shift: true, ..Modifiers::COMMAND };
    s.mods = shift_cmd;
    s.frame(vec![]);
    s.frame(vec![key_v(false, shift_cmd)]);
    s.mods = Modifiers::NONE;

    assert!(s.app().doc.nodes.len() > before, "Shift+Cmd+V uses the in-app copy");
    assert_eq!(s.app().selection.len(), 1);
    assert_eq!(s.app().doc.get(s.app().selection[0]).unwrap().parent, Some(target));
    assert_eq!(READS.load(Ordering::SeqCst), 0, "Shift+Cmd+V must not read system HTML");
}

#[test]
fn shift_let_go_before_v_on_a_figma_html_only_clipboard_is_a_plain_paste() {
    // egui-winit never forwards the V press of a paste chord and pushes no text paste for an HTML-only clipboard, so
    // "Cmd+Shift down, V down, Shift up, V up" reaches the app exactly as "Cmd+Shift down, Shift up, V down, V up" does:
    // the release carries Cmd alone. That is a plain Cmd+V, and it must stay one: a later change that told the two
    // apart would also have to leave a plain Cmd+V after a let-go Shift alone.
    counted_figma_clipboard!(READS);
    let mut s = Sim::new();
    let source = s.by_name("Button");
    let target = s.by_name("Card");
    s.shell.app.selection = vec![source];
    s.shell.app.copy_selection().expect("copy source layers");
    s.shell.app.system_html = counted;
    s.shell.app.selection = vec![target];
    let before = ids(&s);

    let shift_cmd = Modifiers { shift: true, ..Modifiers::COMMAND };
    s.mods = shift_cmd;
    s.frame(vec![]);
    s.mods = Modifiers::COMMAND;
    // winit's macOS backend queues a modifier's key event before the modifier change, so it is still stamped with it
    let shift_up = Event::Key { key: Key::ShiftLeft, physical_key: Some(Key::ShiftLeft), pressed: false, repeat: false, modifiers: shift_cmd };
    s.frame(vec![shift_up, key_v(false, Modifiers::COMMAND)]);
    s.mods = Modifiers::NONE;

    assert_eq!(READS.load(Ordering::SeqCst), 1, "a plain paste reads the system clipboard once");
    let names = names_added(&s, &before);
    assert!(names.iter().any(|n| n == "Button copy"), "the Figma layers were imported, not the in-app copy: {names:?}");
}

#[test]
fn edit_paste_reads_the_scripted_figma_html() {
    let mut s = Sim::new();
    s.shell.app.system_html = figma_payload;
    let before = ids(&s);
    let ctx = s.ctx.clone();
    crate::panels::menus::run(&mut s.shell.app, &ctx, crate::panels::menus::Action::Paste);
    let names = names_added(&s, &before);
    assert!(names.iter().any(|n| n == "Button"), "{names:?}");
}
