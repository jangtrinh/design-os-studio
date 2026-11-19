//! A `.fig` opened in a session keeps the content of the pages that were not loaded when it is written
//! natively: by `designos_save_file`, and by the whole-document sync that hands an editor the opened file.
#![allow(clippy::result_large_err)]

use designos_doc::Document;
use designos_fig::testkit::{deferred_pages_fig, deferred_pages_shape, DeferredPagesShape};
use designos_mcp::server::Session;
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::SystemTime,
};

static SEQ: AtomicU64 = AtomicU64::new(0);

struct Dirs {
    workspace: PathBuf,
    session: PathBuf,
}

impl Dirs {
    /// A workspace holding the deferred-page `.fig`, and a session folder next to it.
    fn new() -> Self {
        let nonce = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
        let base = std::env::temp_dir().join(format!("designos-mcp-pending-pages-{}-{nonce}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)));
        let (workspace, session) = (base.join("ws"), base.join("session"));
        fs::create_dir_all(&workspace).unwrap();
        fs::create_dir_all(&session).unwrap();
        fs::write(workspace.join("kit.fig"), deferred_pages_fig()).unwrap();
        let mut canvas = Document::new("Canvas");
        canvas.add_page("Page");
        fs::write(session.join("canvas.designos"), canvas.to_json().unwrap()).unwrap();
        assert_deferred_on_import(&workspace.join("kit.fig"));
        Self { workspace, session }
    }
}

impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.workspace.parent().unwrap());
    }
}

/// The importer defers the third page of the fixture: without this the tests below would pass for no reason.
fn assert_deferred_on_import(fig: &std::path::Path) {
    let doc = designos_library::load_external(fig).unwrap();
    assert!(doc.page_hydrator.is_some(), "precondition: the importer deferred a page");
    assert_eq!(deferred_pages_shape(&doc), DeferredPagesShape::deferred(), "precondition: the instances on the deferred page are empty");
}

fn read_document(path: &std::path::Path) -> Document {
    Document::from_json(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn saving_a_large_fig_opened_in_a_session_keeps_the_instance_content_of_pages_that_were_not_loaded() {
    let dirs = Dirs::new();
    let mut session = Session::new(&dirs.workspace).unwrap();
    session.call("designos_open_file", json!({ "path": "kit.fig" })).unwrap();

    session.call("designos_save_file", json!({ "path": "kit.designos" })).unwrap();

    let saved = read_document(&dirs.workspace.join("kit.designos"));
    assert_eq!(deferred_pages_shape(&saved), DeferredPagesShape::loaded(), "the saved file keeps the children and the overrides");
}

#[test]
fn opening_a_large_fig_in_an_editor_session_hands_the_editor_the_instance_content_of_pages_that_were_not_loaded() {
    let dirs = Dirs::new();
    let mut session = Session::with_session_dir(&dirs.workspace, &dirs.session).unwrap();

    session.call("designos_open_file", json!({ "path": "kit.fig", "discard_changes": true })).unwrap();

    for file in ["canvas.designos", "doc-sync.designos"] {
        let published = read_document(&dirs.session.join(file));
        assert_eq!(deferred_pages_shape(&published), DeferredPagesShape::loaded(), "{file} keeps the children and the overrides");
    }
}

#[test]
fn a_saved_file_holds_the_bytes_of_the_documents_text() {
    let dirs = Dirs::new();
    let mut session = Session::new(&dirs.workspace).unwrap();
    session.call("designos_open_file", json!({ "path": "kit.fig" })).unwrap();

    session.call("designos_save_file", json!({ "path": "kit.designos" })).unwrap();

    let written = fs::read(dirs.workspace.join("kit.designos")).unwrap();
    assert_eq!(written, session.engine.doc.to_json().unwrap().into_bytes(), "the streamed file is the text the document gives");
    let temporary: Vec<_> =
        fs::read_dir(&dirs.workspace).unwrap().filter_map(|e| e.ok()).filter(|e| e.file_name().to_string_lossy().ends_with(".tmp")).collect();
    assert!(temporary.is_empty(), "no temporary file stays behind: {temporary:?}");
}

#[test]
fn overwriting_a_saved_file_replaces_it_whole() {
    let dirs = Dirs::new();
    let mut session = Session::new(&dirs.workspace).unwrap();
    session.call("designos_open_file", json!({ "path": "kit.fig" })).unwrap();
    fs::write(dirs.workspace.join("kit.designos"), b"old").unwrap();

    session.call("designos_save_file", json!({ "path": "kit.designos", "overwrite": true })).unwrap();

    assert_eq!(deferred_pages_shape(&read_document(&dirs.workspace.join("kit.designos"))), DeferredPagesShape::loaded());
}

#[test]
fn an_edit_the_editor_never_received_is_rolled_back_and_the_document_still_saves_every_page() {
    let dirs = Dirs::new();
    let mut session = Session::with_session_dir(&dirs.workspace, &dirs.session).unwrap();
    session.call("designos_open_file", json!({ "path": "kit.fig", "discard_changes": true })).unwrap();
    // The edit log cannot be appended to (here: something else sits at its path), so the next edit is rolled back
    // to the clone taken before it.
    let log = dirs.session.join("doc-patch.jsonl");
    let _ = fs::remove_file(&log);
    fs::create_dir(&log).unwrap();
    let page = session.engine.doc.pages[0];
    let err =
        session.call("designos_create_node", json!({ "kind": "frame", "parent_id": page.0, "name": "Rejected", "width": 10.0, "height": 10.0 })).unwrap_err();
    assert!(err.contains("doc-patch.jsonl"), "{err}");
    assert!(!session.engine.doc.nodes.values().any(|n| n.name == "Rejected"), "the edit is rolled back");
    fs::remove_dir(&log).unwrap();

    session.call("designos_save_file", json!({ "path": "after-rollback.designos" })).unwrap();

    assert_eq!(deferred_pages_shape(&read_document(&dirs.workspace.join("after-rollback.designos"))), DeferredPagesShape::loaded());
}

#[test]
fn a_canvas_the_editor_rewrote_replaces_the_document_and_it_still_saves_every_page() {
    let dirs = Dirs::new();
    let mut session = Session::with_session_dir(&dirs.workspace, &dirs.session).unwrap();
    session.call("designos_open_file", json!({ "path": "kit.fig", "discard_changes": true })).unwrap();
    // The editor rewrites its canvas with what the session handed it, with a newer modification time.
    let canvas = dirs.session.join("canvas.designos");
    let text = fs::read_to_string(&canvas).unwrap();
    fs::write(&canvas, text).unwrap();
    let file = fs::OpenOptions::new().write(true).open(&canvas).unwrap();
    file.set_modified(SystemTime::now() + std::time::Duration::from_secs(30)).unwrap();
    drop(file);
    session.sync_from_live_canvas_if_newer();

    session.call("designos_save_file", json!({ "path": "after-reload.designos" })).unwrap();

    assert_eq!(deferred_pages_shape(&read_document(&dirs.workspace.join("after-reload.designos"))), DeferredPagesShape::loaded());
}

/// A loader with nothing left to load that had to leave `skipped` instances empty (the copy budget).
#[derive(Debug)]
struct OverBudget {
    skipped: usize,
}

impl designos_doc::PageHydrator for OverBudget {
    fn hydrate_page(&mut self, _: &mut Document, _: designos_doc::NodeId) -> bool {
        false
    }
    fn hydrate_all(&mut self, _: &mut Document) -> bool {
        false
    }
    fn is_hydrated(&self, _: designos_doc::NodeId) -> bool {
        true
    }
    fn unexpanded_instances(&self) -> usize {
        self.skipped
    }
    fn take_unexpanded_instances(&mut self) -> usize {
        std::mem::take(&mut self.skipped)
    }
}

#[test]
fn a_save_says_how_many_instances_the_file_keeps_empty_and_says_it_once() {
    let dirs = Dirs::new();
    let mut session = Session::new(&dirs.workspace).unwrap();
    session.call("designos_new_document", json!({ "name": "Big" })).unwrap();
    session.engine.doc.page_hydrator = Some(std::sync::Arc::new(std::sync::Mutex::new(OverBudget { skipped: 3 })));

    let reply = session.call("designos_save_file", json!({ "path": "big.designos" })).unwrap();

    let warnings = reply["warnings"].as_array().expect("the reply carries a warning");
    assert!(warnings.iter().any(|w| w.as_str().is_some_and(|w| w.starts_with("3 instances are saved empty"))), "{warnings:?}");

    let again = session.call("designos_save_file", json!({ "path": "big.designos", "overwrite": true })).unwrap();
    assert!(again.get("warnings").is_none(), "each instance is reported once: {again}");
}

#[test]
fn a_save_of_a_normal_file_carries_no_warning() {
    let dirs = Dirs::new();
    let mut session = Session::new(&dirs.workspace).unwrap();
    session.call("designos_open_file", json!({ "path": "kit.fig" })).unwrap();

    let reply = session.call("designos_save_file", json!({ "path": "kit.designos" })).unwrap();

    assert!(reply.get("warnings").is_none(), "{reply}");
}
