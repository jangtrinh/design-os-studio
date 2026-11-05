//! UI tests for version history: the panel, saving a named version, viewing, restoring.

use super::*;
use crate::home::Act;
use crate::panels::menus::Action;
use designos_library::Library;
use std::path::PathBuf;

struct TmpDir(PathBuf);

impl TmpDir {
    fn new() -> Self {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("designos_history_ui_{}_{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        Self(p)
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// An editor with a library demo fixture file open.
fn file_sim() -> (Sim, TmpDir) {
    let dir = TmpDir::new();
    let mut s = Sim::with(Options { home: true, library: Some(dir.0.clone()), ..Default::default() });
    let l = lib(&s);
    let p = l.list_projects()[0].id.clone();
    let id = l.create_file(&p, None, "Demo fixture", &crate::demo::demo_document()).expect("create demo fixture file").id;
    crate::home::apply(&mut s.shell.app, Act::Open(id));
    s.frame(vec![]);
    s.frame(vec![]);
    (s, dir)
}

fn lib(s: &Sim) -> Library {
    s.app().lib.clone().expect("a library")
}

fn file_id(s: &Sim) -> String {
    s.app().lib_file.as_ref().expect("a library file is open").id.clone()
}

fn run(s: &mut Sim, a: Action) {
    let ctx = s.ctx.clone();
    crate::panels::menus::run(&mut s.shell.app, &ctx, a);
    s.frame(vec![]);
    s.frame(vec![]);
}

fn nodes(s: &Sim) -> usize {
    s.app().doc.nodes.len()
}

/// Ctrl+Alt+S, type a name, Ctrl+Enter.
fn save_version(s: &mut Sim, name: &str) {
    s.mods = Modifiers::COMMAND | Modifiers::ALT;
    s.key(Key::S);
    s.mods = Modifiers::NONE;
    assert!(s.app().history.save_modal.is_some(), "Ctrl+Alt+S opened the dialog");
    s.rect_of("version-name");
    s.type_text(name);
    s.frame(vec![]);
    s.mods = Modifiers::COMMAND;
    s.key(Key::Enter);
    s.mods = Modifiers::NONE;
    s.frame(vec![]);
}

fn delete_node(s: &mut Sim, name: &str) {
    let id = s.by_name(name);
    s.shell.app.select_only(id);
    s.shell.app.delete_selection();
    s.frame(vec![]);
}

#[test]
fn save_a_named_version_view_it_restore_it_and_undo() {
    let (mut s, _d) = file_sim();
    let full = nodes(&s);
    run(&mut s, Action::ToggleVersionHistory);
    assert!(s.app().history.open);
    s.rect_of("history-panel");
    s.rect_of("history-current");

    save_version(&mut s, "Before cleanup");
    assert!(s.app().history.save_modal.is_none());
    let versions = lib(&s).list_versions(&file_id(&s));
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].label.as_deref(), Some("Before cleanup"));
    assert!(!versions[0].auto);
    s.frame(vec![]);
    s.rect_of("history-version-1");

    // edit, then look at the old version
    delete_node(&mut s, "Hero");
    assert_eq!(nodes(&s), full - 1);
    let row = s.rect_of("history-version-1");
    s.click(row.center());
    assert!(s.app().history.viewing.is_some());
    assert_eq!(nodes(&s), full, "the old version is on the canvas");
    s.rect_of("history-banner");
    let (restore, back) = (s.rect_of("history-restore"), s.rect_of("history-back"));
    assert!(s.app().history.viewing.as_ref().unwrap().title.contains("Before cleanup"));

    // read-only: shortcuts and the canvas cannot change it, and nothing is autosaved over the file
    s.shell.app.select_only(s.by_name("Card"));
    s.key(Key::Delete);
    assert_eq!(nodes(&s), full);
    for _ in 0..40 {
        s.frame(vec![]);
    }
    assert_eq!(lib(&s).load_file(&file_id(&s)).unwrap().nodes.len(), full - 1, "the file still holds the edited document");

    // Back to current
    s.click(back.center());
    assert!(s.app().history.viewing.is_none());
    assert_eq!(nodes(&s), full - 1);

    // view again and restore
    let row = s.rect_of("history-version-1");
    s.click(row.center());
    assert!(s.app().history.viewing.is_some());
    let restore2 = s.rect_of("history-restore");
    assert_eq!(restore.size(), restore2.size());
    s.click(restore2.center());
    assert!(s.app().history.viewing.is_none());
    assert_eq!(nodes(&s), full, "restored");
    let list = lib(&s).list_versions(&file_id(&s));
    assert!(list.iter().any(|v| v.auto), "the pre-restore state was kept as an auto version");
    assert_eq!(list.len(), 2);

    // one undo brings the pre-restore document back
    s.cmd_key(Key::Z);
    assert_eq!(nodes(&s), full - 1);
    s.mods = Modifiers::COMMAND | Modifiers::SHIFT;
    s.key(Key::Z);
    s.mods = Modifiers::NONE;
    assert_eq!(nodes(&s), full);
}

#[test]
fn the_panel_groups_versions_by_day() {
    let (mut s, _d) = file_sim();
    let l = lib(&s);
    let id = file_id(&s);
    let now = crate::comments::time::now_ms();
    let doc = s.app().doc.clone();
    l.save_version_at(&id, &doc, None, None, true, now - 3 * 86_400_000).unwrap();
    l.save_version_at(&id, &doc, Some("Named".into()), None, false, now - 100).unwrap();
    run(&mut s, Action::ToggleVersionHistory);
    s.rect_of("history-version-1");
    s.rect_of("history-version-2");
    let top = s.rect_of("history-current");
    assert!(
        s.rect_of("history-version-2").top() > top.top() && s.rect_of("history-version-1").top() > s.rect_of("history-version-2").top(),
        "newest first, under Current version"
    );
    assert_eq!(crate::history::day_label(now, now - 100), "Today");
}

#[test]
fn files_outside_the_library_show_a_hint_instead() {
    let mut s = Sim::new();
    run(&mut s, Action::ToggleVersionHistory);
    s.rect_of("history-hint");
    assert!(s.app().history.open);
    s.mods = Modifiers::COMMAND | Modifiers::ALT;
    s.key(Key::S);
    s.mods = Modifiers::NONE;
    assert!(s.app().history.save_modal.is_none(), "no save dialog without a library file");
    assert!(s.app().toast.as_ref().is_some_and(|t| t.text == "Save to library to keep version history"));
    // closing the panel again
    let close = close_button(&s);
    s.click(close.center());
    assert!(!s.app().history.open);
}

/// The close button of the right panel header.
fn close_button(s: &Sim) -> Rect {
    let r = s.rect_of("history-panel");
    Rect::from_min_size(Pos2::new(r.right() - 8.0 - 24.0, r.top() + theme::COMMENT_PANEL_HEADER_H / 2.0 - 12.0), Vec2::splat(24.0))
}

fn open_menu(s: &mut Sim, n: u32) {
    let row = s.rect_of(&format!("history-version-{n}"));
    s.right_click(row.center());
    assert_eq!(s.app().history.menu.map(|m| m.0), Some(n));
}

#[test]
fn the_context_menu_names_duplicates_and_deletes() {
    let (mut s, _d) = file_sim();
    let l = lib(&s);
    let id = file_id(&s);
    let doc = s.app().doc.clone();
    l.save_version(&id, &doc, None, true).unwrap();
    run(&mut s, Action::ToggleVersionHistory);

    // Name this version (second row)
    open_menu(&mut s, 1);
    s.key(Key::ArrowDown);
    s.key(Key::ArrowDown);
    s.key(Key::Enter);
    assert!(s.app().dialog.is_some(), "a name prompt opened");
    s.frame(vec![]);
    s.type_text("Handoff");
    s.frame(vec![]);
    s.key(Key::Enter);
    s.frame(vec![]);
    assert!(s.app().dialog.is_none());
    assert_eq!(l.list_versions(&id)[0].label.as_deref(), Some("Handoff"));
    s.frame(vec![]);

    // Duplicate as new file (third row)
    let files = l.all_files().len();
    open_menu(&mut s, 1);
    for _ in 0..3 {
        s.key(Key::ArrowDown);
    }
    s.key(Key::Enter);
    assert_eq!(l.all_files().len(), files + 1);
    assert!(l.all_files().iter().any(|f| f.name.contains("Handoff")));

    // Delete (fourth row)
    open_menu(&mut s, 1);
    for _ in 0..4 {
        s.key(Key::ArrowDown);
    }
    s.key(Key::Enter);
    assert!(l.list_versions(&id).is_empty());
}

#[test]
fn closing_the_window_saves_an_automatic_version() {
    let (mut s, _d) = file_sim();
    let id = file_id(&s);
    delete_node(&mut s, "Hero");
    assert!(lib(&s).list_versions(&id).is_empty());
    s.shell.app.save_close_version();
    let list = lib(&s).list_versions(&id);
    assert_eq!(list.len(), 1);
    assert!(list[0].auto);
    assert_eq!(lib(&s).load_version(&id, list[0].n).unwrap().nodes.len(), nodes(&s));
}

#[test]
fn labeled_live_patch_shows_step_label_in_history_panel() {
    let (mut s, _d) = file_sim();
    let hero = s.by_name("Hero");
    let patch = designos_doc::live_patch::LivePatch::new(vec![designos_doc::live_patch::LiveOp::Remove { id: hero }]).with_label(Some("Agent: run-7".into()));
    crate::live_patch_apply::apply_live_patches(&mut s.shell.app, &[patch]);
    assert_eq!(s.app().hist.peek_undo_label(), Some("Agent: run-7"));

    run(&mut s, Action::ToggleVersionHistory);
    let _ = s.rect_of("history-undo-0");

    // Undo moves the label to redo; redo restores it to undo.
    s.cmd_key(Key::Z);
    assert_eq!(s.app().hist.peek_undo_label(), None);
    assert_eq!(s.app().hist.peek_redo_label(), Some("Agent: run-7"));
}

#[test]
fn banner_paints_back_to_current_in_the_frame_restore_is_clicked() {
    let (mut s, _d) = file_sim();
    run(&mut s, Action::ToggleVersionHistory);
    save_version(&mut s, "Before cleanup");
    s.frame(vec![]);
    let row = s.rect_of("history-version-1");
    s.click(row.center());
    assert!(s.app().history.viewing.is_some());
    let restore = s.rect_of("history-restore");
    s.mv(restore.center());
    s.button(true);
    s.button(false);
    let back_painted = s.shapes.iter().any(|clipped| matches!(&clipped.shape, egui::epaint::Shape::Text(text) if text.galley.text() == "Back to current"));
    assert!(back_painted, "the click frame still paints the second button");
    assert!(s.app().history.viewing.is_none(), "the click restored the version");
}
