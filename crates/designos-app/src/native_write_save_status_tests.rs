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
