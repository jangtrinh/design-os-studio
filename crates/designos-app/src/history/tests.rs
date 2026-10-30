//! Version history logic without a window: activity timing, saving, viewing, restoring.

use super::*;
use designos_doc::{Node, Rect};

pub(super) struct Tmp(std::path::PathBuf);
impl Tmp {
    pub(super) fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!("designos_history_{}_{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        Self(p)
    }
}
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// An app with a library file open, holding `n` rectangles.
pub(super) fn app_with_file(t: &Tmp, rects: usize) -> App {
    let mut app = App::new(Document::new("t"));
    app.attach_library(t.0.clone());
    app.new_library_file(None);
    assert!(app.lib_file.is_some());
    for _ in 0..rects {
        add_rect(&mut app);
    }
    app
}

pub(super) fn add_rect(app: &mut App) {
    let id = app.doc.alloc_id();
    let page = app.page;
    let cmd = designos_doc::Command::Insert { parent: page, index: 0, nodes: vec![Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0))] };
    app.hist.exec(&mut app.doc, cmd).unwrap();
    app.changed();
}

fn count(app: &App) -> usize {
    app.doc.get(app.page).map(|p| p.children.len()).unwrap_or(0)
}

#[test]
fn ten_minutes_of_activity_make_an_auto_version() {
    let mut a = Activity::default();
    assert!(!a.due());
    // an edit every 30 seconds for ten minutes
    for i in 0..=20 {
        a.note_edit(i as f64 * 30.0);
    }
    assert!(a.due(), "{a:?}");
    a.reset();
    assert!(!a.due());
}

#[test]
fn pauses_do_not_count_as_activity() {
    let mut a = Activity::default();
    a.note_edit(0.0);
    a.note_edit(3600.0); // an hour away from the keyboard counts for at most a minute
    a.note_edit(7200.0);
    assert!(!a.due());
}

#[test]
fn labels_for_the_panel() {
    let day = 86_400_000u64;
    let now = 10 * day + 5_000;
    assert_eq!(day_label(now, now - 1000), "Today");
    assert_eq!(day_label(now, now - day - 1), "Yesterday");
    assert_eq!(day_label(now, now - 3 * day), "3 days ago");
    assert_eq!(day_label(now, 0), "1970-01-01");
    assert_eq!(stamp(1_000_000_000_123), "2001-09-09 01:46 UTC");
}

#[test]
fn files_outside_the_library_have_no_history() {
    let mut app = App::new(Document::new("t"));
    assert!(!app.has_history());
    app.begin_save_version();
    assert!(app.history.save_modal.is_none());
    assert!(app.toast.as_ref().is_some_and(|t| t.text == "Save to library to keep version history" && t.error));
    assert!(app.save_auto_version().is_none());
    assert!(!app.save_named_version("x", ""));
}

#[test]
fn auto_versions_only_when_something_changed() {
    let t = Tmp::new("auto");
    let mut app = app_with_file(&t, 0);
    assert!(app.save_auto_version().is_none(), "nothing changed since opening");
    add_rect(&mut app);
    let v = app.save_auto_version().expect("a version");
    assert!(v.auto && v.label.is_none());
    assert!(app.save_auto_version().is_none(), "unchanged since the last version");
    add_rect(&mut app);
    app.save_close_version();
    let list = app.lib.as_ref().unwrap().list_versions(&app.lib_file.as_ref().unwrap().id);
    assert_eq!(list.len(), 2);
}

#[test]
fn the_tick_saves_after_ten_minutes_of_edits() {
    let t = Tmp::new("tick");
    let mut app = app_with_file(&t, 1);
    for i in 0..=21 {
        app.now = i as f64 * 30.0;
        app.autosave.changed_at = Some(app.now);
        app.autosave.confirmed = true;
        app.history_tick();
        app.autosave.changed_at = None;
        app.autosave.confirmed = false;
        app.history_tick();
        add_rect(&mut app);
    }
    let id = app.lib_file.as_ref().unwrap().id.clone();
    assert_eq!(app.lib.as_ref().unwrap().list_versions(&id).len(), 1);
}

#[test]
fn save_view_and_restore_round_trip_with_one_undo() {
    let t = Tmp::new("restore");
    let mut app = app_with_file(&t, 2);
    assert!(app.save_named_version("Two rects", "before the redesign"));
    let id = app.lib_file.as_ref().unwrap().id.clone();
    let lib = app.lib.clone().unwrap();
    let named = lib.list_versions(&id);
    assert_eq!(named.len(), 1);
    assert_eq!(named[0].label.as_deref(), Some("Two rects"));
    assert_eq!(named[0].description.as_deref(), Some("before the redesign"));
    assert!(!named[0].auto);

    add_rect(&mut app);
    add_rect(&mut app);
    assert_eq!(count(&app), 4);
    app.save_library_file().unwrap();

    // viewing shows the old document and leaves the live one alone
    app.view_version(named[0].n);
    assert_eq!(count(&app), 2);
    assert!(app.history.viewing.is_some());
    add_rect_via_tx(&mut app);
    assert_eq!(count(&app), 2, "the view is read-only");
    app.flush_autosave();
    assert_eq!(lib.load_file(&id).unwrap().get(app.page).unwrap().children.len(), 4, "the file on disk keeps the live document");
    app.exit_version_view();
    assert_eq!(count(&app), 4);
    assert!(app.hist.can_undo(), "the live undo stack survived the view");

    // restore: a new auto version of the 4-rect state, then the 2-rect state is current
    let before = lib.list_versions(&id).len();
    app.view_version(named[0].n);
    app.restore_version(named[0].n);
    assert!(app.history.viewing.is_none());
    assert_eq!(count(&app), 2);
    let after = lib.list_versions(&id);
    assert_eq!(after.len(), before + 1);
    let kept = after.iter().find(|v| v.auto).expect("pre-restore auto version");
    assert_eq!(lib.load_version(&id, kept.n).unwrap().get(app.page).unwrap().children.len(), 4);
    assert_eq!(lib.load_file(&id).unwrap().get(app.page).unwrap().children.len(), 2, "the file on disk was restored too");

    // one undo returns to the pre-restore state, redo restores again
    app.undo();
    assert_eq!(count(&app), 4);
    assert_eq!(lib.load_file(&id).unwrap().get(app.page).unwrap().children.len(), 4);
    app.redo();
    assert_eq!(count(&app), 2);
    app.undo();
    assert_eq!(count(&app), 4);
    // ... and the older undo history is there again
    app.undo();
    assert_eq!(count(&app), 3);
}

#[test]
fn viewing_a_version_and_leaving_it_restores_the_load_watermark() {
    let t = Tmp::new("watermark");
    let mut app = app_with_file(&t, 2);
    let original = app.load_next_id;
    assert!(app.save_named_version("two", ""));
    let id = app.lib_file.as_ref().unwrap().id.clone();
    let n = app.lib.as_ref().unwrap().list_versions(&id)[0].n;
    let version_peek = app.lib.as_ref().unwrap().load_version(&id, n).unwrap().peek_next_id();
    assert_ne!(version_peek, original, "the saved version's next id is not the watermark recorded when the file was opened");

    app.view_version(n);
    assert_eq!(app.load_next_id, version_peek, "the version on the canvas uses its own watermark");
    app.exit_version_view();
    assert_eq!(app.load_next_id, original, "leaving the version restores the live document's watermark");
}

fn add_rect_via_tx(app: &mut App) {
    let mut tx = crate::edit::Tx::new();
    let id = app.doc.alloc_id();
    let page = app.page;
    tx.cmd(&mut app.doc, designos_doc::Command::Insert { parent: page, index: 0, nodes: vec![Node::rectangle(id, Rect::new(0.0, 0.0, 5.0, 5.0))] }).unwrap();
    app.run_tx(tx);
}

#[test]
fn an_edit_after_a_restore_keeps_the_restore_undoable_last() {
    let t = Tmp::new("restore_edit");
    let mut app = app_with_file(&t, 1);
    assert!(app.save_named_version("one", ""));
    let n = app.lib.as_ref().unwrap().list_versions(&app.lib_file.as_ref().unwrap().id)[0].n;
    add_rect(&mut app);
    add_rect(&mut app);
    app.restore_version(n);
    assert_eq!(count(&app), 1);
    add_rect(&mut app);
    assert_eq!(count(&app), 2);
    app.undo();
    assert_eq!(count(&app), 1, "first the edit");
    app.undo();
    assert_eq!(count(&app), 3, "then the restore");
}

#[test]
fn a_stale_redo_after_undoing_a_restore_is_dropped() {
    let t = Tmp::new("stale_redo");
    let mut app = app_with_file(&t, 1);
    assert!(app.save_named_version("one", ""));
    let n = app.lib.as_ref().unwrap().list_versions(&app.lib_file.as_ref().unwrap().id)[0].n;
    add_rect(&mut app);
    app.restore_version(n);
    app.undo();
    assert_eq!(count(&app), 2);
    add_rect(&mut app); // a new edit invalidates the redo of the restore
    app.redo();
    assert_eq!(count(&app), 3, "redo did not jump back to the restored document");
}

#[test]
fn naming_duplicating_and_deleting_versions() {
    let t = Tmp::new("manage");
    let mut app = app_with_file(&t, 1);
    let lib = app.lib.clone().unwrap();
    let id = app.lib_file.as_ref().unwrap().id.clone();
    add_rect(&mut app);
    let v = app.save_auto_version().unwrap();
    app.name_version(v.n, " Milestone ");
    let named = lib.list_versions(&id);
    assert_eq!(named[0].label.as_deref(), Some("Milestone"));
    assert!(named[0].is_named());

    let files_before = lib.all_files().len();
    app.duplicate_version(v.n);
    let files = lib.all_files();
    assert_eq!(files.len(), files_before + 1);
    let copy = files.iter().find(|f| f.name.contains("Milestone")).expect("duplicate named after the version");
    assert_eq!(lib.load_file(&copy.id).unwrap().pages.len(), 1);
    assert_eq!(copy.project, app.lib_file.as_ref().unwrap().project_id);

    app.delete_version(v.n);
    assert!(lib.list_versions(&id).is_empty());
    assert!(t.0.join("trash").read_dir().unwrap().any(|e| e.unwrap().file_name().to_string_lossy().contains("-v")), "moved to the trash");
}

#[test]
fn leaving_the_file_saves_a_closing_version_and_drops_the_view() {
    let t = Tmp::new("leave");
    let mut app = app_with_file(&t, 1);
    let id = app.lib_file.as_ref().unwrap().id.clone();
    let lib = app.lib.clone().unwrap();
    assert!(app.save_named_version("v", ""));
    add_rect(&mut app);
    let n = lib.list_versions(&id)[0].n;
    app.view_version(n);
    app.go_home();
    assert!(app.history.viewing.is_none());
    let list = lib.list_versions(&id);
    assert_eq!(list.len(), 2, "a closing auto version was added");
    assert_eq!(lib.load_file(&id).unwrap().get(app.page).map(|p| p.children.len()), Some(2), "the live document, not the viewed one, is on disk");
}

/// The `--panel version-history` preset saves nine versions at fixed ages. The library thins automatic versions to one per
/// clock hour once they are an hour old, so two of them must never land in the same clock hour, whatever minute the
/// capture runs in.
#[test]
fn the_version_history_preset_keeps_all_nine_versions_at_every_minute_of_the_hour() {
    let t = Tmp::new("preset_every_minute");
    let hour_ms = 3_600_000u64;
    let hour_start = 1_790_000_000_000 / hour_ms * hour_ms;
    for minute in 0..60u64 {
        let mut app = App::new(Document::new("t"));
        super::preset::apply_at(&mut app, Some(t.0.clone()), hour_start + minute * 60_000 + 30_000);
        let file = app.lib_file.as_ref().expect("the preset opens a library file").id.clone();
        let versions = app.lib.as_ref().expect("the preset attaches a library").list_versions(&file).len();
        assert_eq!(versions, 9, "minute {minute} of the hour kept {versions} of 9 versions");
    }
}
