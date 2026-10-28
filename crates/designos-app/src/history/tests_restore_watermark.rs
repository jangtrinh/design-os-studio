//! The load watermark of the document on screen across a version restore and its undo and redo.

use super::tests::{add_rect, app_with_file, Tmp};

#[test]
fn restoring_a_version_then_undoing_and_redoing_it_swaps_each_documents_watermark() {
    let t = Tmp::new("restore_watermark");
    let mut app = app_with_file(&t, 2);
    let opened = app.load_next_id;
    assert!(app.save_named_version("two", ""));
    let id = app.lib_file.as_ref().unwrap().id.clone();
    let n = app.lib.as_ref().unwrap().list_versions(&id)[0].n;
    let version = app.lib.as_ref().unwrap().load_version(&id, n).unwrap().peek_next_id();
    add_rect(&mut app);
    add_rect(&mut app);
    let live_next = app.doc.peek_next_id();
    assert!(opened != version && version != live_next && opened != live_next, "three different values: {opened:?} {version:?} {live_next:?}");

    app.restore_version(n);
    assert_eq!(app.load_next_id, version, "the restored version is a document of its own, with its own watermark");

    app.undo();
    assert_eq!(app.load_next_id, opened, "undoing the restore brings back the watermark the live document was opened with");
    assert_ne!(app.load_next_id, live_next, "not the id its edits had reached");

    app.redo();
    assert_eq!(app.load_next_id, version, "redoing the restore brings the version's watermark back");
}
