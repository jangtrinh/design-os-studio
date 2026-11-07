//! A copy of a document that still has pages to load shares the page loader of the original. Whichever side
//! loads a page first leaves the other with empty instances, so a copy must never be the one that loads, and a
//! document whose pages a copy used up must refuse to write instead of reporting a save.

use crate::app::App;
use designos_doc::Document;
use designos_fig::testkit::{deferred_pages_fig, deferred_pages_shape, DeferredPagesShape};
use std::path::{Path, PathBuf};

pub(crate) fn scratch(tag: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!("designos_native_copies_{tag}_{}_{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// An editor with the deferred-page `.fig` open in its one tab.
pub(crate) fn editor_with_fig_in_a_tab(dir: &Path) -> App {
    let fig = dir.join("kit.fig");
    std::fs::write(&fig, deferred_pages_fig()).unwrap();
    let mut app = App::new(Document::new("Init"));
    app.open_path(fig);
    if app.tabs.entries.is_empty() {
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.sync_from_app(&app);
        app.tabs = tabs;
    }
    assert!(app.doc.page_hydrator.is_some(), "precondition: the importer deferred a page");
    assert_eq!(deferred_pages_shape(&app.doc), DeferredPagesShape::deferred(), "precondition: the instances on the deferred page are empty");
    app
}

pub(crate) fn shape_on_disk(path: &Path) -> DeferredPagesShape {
    deferred_pages_shape(&crate::io::load_path(path).unwrap_or_else(|e| panic!("reopen {}: {e}", path.display())))
}

#[test]
fn a_duplicated_tab_saved_first_leaves_the_original_tab_saving_its_content() {
    let dir = scratch("dup_save");
    let mut app = editor_with_fig_in_a_tab(&dir);
    let original = app.tabs.active_idx;
    app.duplicate_tab(original).expect("the tab is duplicated");

    app.persist_document_to(&dir.join("copy.designos")).expect("the copy saves");
    app.switch_tab(original);
    app.persist_document_to(&dir.join("orig.designos")).expect("the original saves");

    assert_eq!(shape_on_disk(&dir.join("copy.designos")), DeferredPagesShape::loaded(), "the copy keeps the children and the overrides");
    assert_eq!(shape_on_disk(&dir.join("orig.designos")), DeferredPagesShape::loaded(), "the original keeps the children and the overrides");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_duplicated_tab_whose_pages_are_visited_leaves_the_original_tab_saving_its_content() {
    let dir = scratch("dup_visit");
    let mut app = editor_with_fig_in_a_tab(&dir);
    let original = app.tabs.active_idx;
    app.duplicate_tab(original).expect("the tab is duplicated");
    for page in app.doc.pages.clone() {
        app.switch_page(page);
    }

    app.switch_tab(original);
    app.persist_document_to(&dir.join("orig.designos")).expect("the original saves");

    assert_eq!(shape_on_disk(&dir.join("orig.designos")), DeferredPagesShape::loaded(), "the original keeps the children and the overrides");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn duplicating_a_tab_with_pages_to_load_loads_them_into_the_source_first() {
    let dir = scratch("dup_loads_source");
    let mut app = editor_with_fig_in_a_tab(&dir);
    let original = app.tabs.active_idx;

    let copy = app.duplicate_tab(original).expect("the tab is duplicated");

    assert_ne!(copy, original);
    assert_eq!(deferred_pages_shape(&app.doc), DeferredPagesShape::loaded(), "the copy on screen starts complete");
    assert!(!app.doc.has_pending_pages());
    assert_eq!(deferred_pages_shape(&app.tabs.entries[original].doc), DeferredPagesShape::loaded(), "the source starts complete too");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_tab_with_pages_to_load_says_so_before_it_is_duplicated() {
    let dir = scratch("dup_pending");
    let mut app = editor_with_fig_in_a_tab(&dir);
    let tabs = std::mem::take(&mut app.tabs);
    assert!(tabs.tab_has_pending_pages(tabs.active_idx, &app), "the loader still waits for a page");
    app.tabs = tabs;
    app.doc.load_pending_pages().expect("loads");
    let tabs = std::mem::take(&mut app.tabs);
    assert!(!tabs.tab_has_pending_pages(tabs.active_idx, &app));
    app.tabs = tabs;
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_clone_put_back_as_the_open_document_still_saves_its_content() {
    let dir = scratch("clone_back");
    let mut app = editor_with_fig_in_a_tab(&dir);
    // The same shape as the preview frames: the open document is replaced by a clone of itself.
    let snapshot = app.doc.clone();
    app.doc = snapshot;

    app.persist_document_to(&dir.join("kit.designos")).expect("the restored document saves");

    assert_eq!(shape_on_disk(&dir.join("kit.designos")), DeferredPagesShape::loaded());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_preview_frame_built_from_a_clone_leaves_the_open_document_saving_its_content() {
    let dir = scratch("preview_frame");
    let mut app = editor_with_fig_in_a_tab(&dir);
    let base = app.doc.clone();

    // Frames of a preview sequence replace the open document by a clone of the base on every step.
    crate::shell::apply_micro_interaction_continuous_frame(&mut app, &base, "none", 0, 10);
    app.persist_document_to(&dir.join("kit.designos")).expect("the document saves");

    assert_eq!(shape_on_disk(&dir.join("kit.designos")), DeferredPagesShape::loaded());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_document_whose_pages_a_copy_loaded_refuses_to_save_and_says_why() {
    let dir = scratch("refuses");
    let mut app = editor_with_fig_in_a_tab(&dir);
    let mut copy = app.doc.clone();
    copy.load_pending_pages().expect("the copy loads");
    let out = dir.join("kit.designos");

    let res = app.persist_document_to(&out);

    assert!(res.is_err() && !out.exists(), "no file is written from empty instances");
    let toast = app.toast.as_ref().map(|t| t.text.clone()).unwrap_or_default();
    assert!(toast.starts_with("Cannot save") && toast.contains("Another copy loaded these pages"), "the toast names the cause: {toast}");
    let _ = std::fs::remove_dir_all(&dir);
}
