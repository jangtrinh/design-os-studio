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
