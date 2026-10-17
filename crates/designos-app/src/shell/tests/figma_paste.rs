//! Figma clipboard paste through the window: the HTML flavour is read through `App::system_html`, never
//! the OS pasteboard, from Cmd+V (with or without a text paste event) and from Edit > Paste.

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
fn a_paste_event_without_figma_html_leaves_the_document_alone() {
    let mut s = Sim::new();
    let before = ids(&s);
    s.frame(vec![Event::Paste("just some words".into())]);
    assert_eq!(ids(&s), before);
}

#[test]
fn cmd_v_without_a_paste_event_pastes_figma_html() {
    let mut s = Sim::new();
    s.shell.app.system_html = figma_payload;
    let before = ids(&s);
    s.frame(vec![key_v(true, Modifiers::COMMAND)]);
    let names = names_added(&s, &before);
    assert!(names.iter().any(|n| n == "Button"), "{names:?}");
}

#[test]
fn cmd_v_without_a_paste_event_or_figma_html_does_nothing() {
    let mut s = Sim::new();
    let before = ids(&s);
    s.frame(vec![key_v(true, Modifiers::COMMAND)]);
    assert_eq!(ids(&s), before);
}

#[test]
fn cmd_v_together_with_a_paste_event_pastes_once() {
    static READS: AtomicUsize = AtomicUsize::new(0);
    fn counted() -> Option<String> {
        READS.fetch_add(1, Ordering::SeqCst);
        figma_payload()
    }
    let mut s = Sim::new();
    s.shell.app.system_html = counted;
    let before = ids(&s);
    s.frame(vec![key_v(true, Modifiers::COMMAND), Event::Paste("text".into())]);
    let names = names_added(&s, &before);
    assert_eq!(names.iter().filter(|n| *n == "Button").count(), 1, "one Button, not two: {names:?}");
    assert_eq!(READS.load(Ordering::SeqCst), 1, "one clipboard read for one paste");
}

#[test]
fn shift_cmd_v_does_not_read_the_clipboard_html() {
    static READS: AtomicUsize = AtomicUsize::new(0);
    fn counted() -> Option<String> {
        READS.fetch_add(1, Ordering::SeqCst);
        figma_payload()
    }
    let mut s = Sim::new();
    s.shell.app.system_html = counted;
    let before = ids(&s);
    s.frame(vec![key_v(true, Modifiers { shift: true, ..Modifiers::COMMAND })]);
    assert_eq!(READS.load(Ordering::SeqCst), 0);
    assert_eq!(ids(&s), before, "paste over selection is not a Figma paste");
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
