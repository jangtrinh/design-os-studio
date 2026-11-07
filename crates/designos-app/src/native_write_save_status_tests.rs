//! Save and Save As say that the pages of a large `.fig` are being loaded before the window waits, and write on the
//! next frame. A caller that continues with the result gets it before the call returns.

use super::native_write_copies_tests::{editor_with_fig_in_a_tab, scratch, shape_on_disk};
use crate::app::App;
use designos_doc::Document;
use designos_fig::testkit::DeferredPagesShape;

#[test]
fn save_announces_the_page_load_and_writes_on_the_next_frame() {
    let dir = scratch("save_announced");
    let mut app = editor_with_fig_in_a_tab(&dir);
    let out = dir.join("kit.designos");
    app.file_path = Some(out.clone());

    app.save(false);

    assert!(app.has_queued_work() && !out.exists(), "the write waits for the frame that paints the status");
    assert!(app.toast.as_ref().is_some_and(|t| t.text.starts_with("Saving")), "the status says what is going on: {:?}", app.toast.as_ref().map(|t| &t.text));

    app.run_queued_work();

    assert_eq!(shape_on_disk(&out), DeferredPagesShape::loaded(), "the file keeps the children and the overrides");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_save_that_a_caller_chains_on_writes_at_once() {
    let dir = scratch("save_direct");
    let mut app = editor_with_fig_in_a_tab(&dir);
    let out = dir.join("kit.designos");
    app.file_path = Some(out.clone());

    app.save_with(false, &crate::io::SystemDialogs);

    assert!(!app.has_queued_work(), "a caller that continues with the result gets it before the call returns");
    assert_eq!(shape_on_disk(&out), DeferredPagesShape::loaded());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_save_of_a_document_with_every_page_loaded_writes_at_once() {
    let dir = scratch("save_plain");
    let mut app = App::new(Document::new("Plain"));
    app.doc.add_page("Page 1");
    let out = dir.join("plain.designos");
    app.file_path = Some(out.clone());

    app.save(false);

    assert!(!app.has_queued_work() && out.exists(), "nothing to wait for, so the save ran");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_queued_save_keeps_the_document_that_was_active_when_requested() {
    let dir = scratch("save_active_tab");
    let mut app = editor_with_fig_in_a_tab(&dir);
    let original = app.tabs.active_idx;
    let out = dir.join("original.designos");
    app.file_path = Some(out.clone());

    // Opening another tab parks the deferred document. Switching back restores it, then Save
    // queues the expensive page load. A tab switch before the next frame must not retarget it.
    app.open_new_design_tab();
    app.switch_tab(original);
    app.save(false);
    assert!(app.has_queued_work(), "the deferred document save is queued");
    app.switch_tab(1);
    app.run_queued_work();

    assert_eq!(shape_on_disk(&out), DeferredPagesShape::loaded(), "the queued save writes the requested tab");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_queued_save_refuses_when_the_requested_tab_was_closed() {
    let dir = scratch("save_closed_tab");
    let mut app = editor_with_fig_in_a_tab(&dir);
    let original = app.tabs.active_idx;
    let out = dir.join("closed.designos");
    app.file_path = Some(out.clone());

    app.open_new_design_tab();
    app.switch_tab(original);
    app.save(false);
    assert!(app.has_queued_work(), "the deferred document save is queued");
    app.switch_tab(1);
    assert!(app.close_tab(original), "the requested tab can be closed before the next frame");
    app.run_queued_work();

    assert!(!out.exists(), "a closed requested tab is never replaced by another document");
    assert_eq!(app.toast.as_ref().map(|toast| toast.text.as_str()), Some("Cannot save: the tab is no longer open"));
    let _ = std::fs::remove_dir_all(&dir);
}
