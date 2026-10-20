//! Opening or creating a document in a session bound to an open editor is announced in the edit
//! log, so the editor can tell the edits made before it from the edits made after it.
#![allow(clippy::result_large_err)]

use designos_doc::live_patch::{LiveLogLine, LiveOp};
use designos_doc::Document;
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
    fn new() -> Self {
        let nonce = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
        let base = std::env::temp_dir().join(format!("designos-mcp-epoch-{}-{nonce}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)));
        let (workspace, session) = (base.join("ws"), base.join("session"));
        fs::create_dir_all(&workspace).unwrap();
        fs::create_dir_all(&session).unwrap();
        let mut doc = Document::new("Canvas");
        doc.add_page("Page");
        fs::write(session.join("canvas.designos"), doc.to_json().unwrap()).unwrap();
        let mut other = Document::new("Other");
        other.add_page("Page");
        fs::write(workspace.join("other.designos"), other.to_json().unwrap()).unwrap();
        Self { workspace, session }
    }

    fn log(&self) -> Vec<LiveLogLine> {
        let raw = fs::read_to_string(self.session.join("doc-patch.jsonl")).unwrap_or_default();
        raw.lines().map(|l| LiveLogLine::parse(l.as_bytes()).expect("log line")).collect()
    }
}

impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.workspace.parent().unwrap());
    }
}

fn create(session: &mut Session, name: &str) -> Result<serde_json::Value, designos_mcp::tools::errors::ToolError> {
    session.call("designos_create_node", json!({ "kind": "frame", "name": name, "width": 10.0, "height": 10.0 }))
}

#[test]
fn opening_a_file_marks_the_switch_in_the_log_between_the_edits_before_and_after_it() {
    let dirs = Dirs::new();
    let mut session = Session::with_session_dir(&dirs.workspace, &dirs.session).unwrap();
    create(&mut session, "Before").unwrap();
    session.call("designos_open_file", json!({ "path": "other.designos", "discard_changes": true })).unwrap();
    create(&mut session, "After").unwrap();

    let log = dirs.log();
    assert_eq!(log.len(), 3, "{log:?}");
    assert!(matches!(&log[0], LiveLogLine::Patch(p) if matches!(p.ops[0], LiveOp::Insert { .. })));
    let LiveLogLine::DocEpoch(mark) = &log[1] else { panic!("the switch is the second line: {log:?}") };
    assert!(matches!(&log[2], LiveLogLine::Patch(_)));
    let published = fs::read(dirs.session.join("doc-sync.designos")).unwrap();
    assert!(mark.matches(&published), "the mark describes the document published for the switch");
    assert_eq!(Document::from_json(std::str::from_utf8(&published).unwrap()).unwrap().name, "Other");
}

#[test]
fn creating_a_document_marks_the_switch_and_each_switch_is_counted() {
    let dirs = Dirs::new();
    let mut session = Session::with_session_dir(&dirs.workspace, &dirs.session).unwrap();
    session.call("designos_new_document", json!({ "name": "Fresh", "discard_changes": true })).unwrap();
    session.call("designos_open_file", json!({ "path": "other.designos", "discard_changes": true })).unwrap();
    let epochs: Vec<u64> = dirs.log().iter().filter_map(|l| if let LiveLogLine::DocEpoch(e) = l { Some(e.doc_epoch) } else { None }).collect();
    assert_eq!(epochs, [1, 2]);
}

#[test]
fn a_switch_the_editor_was_not_told_about_is_reported_and_no_edit_reaches_the_log_before_it_is() {
    let dirs = Dirs::new();
    let mut session = Session::with_session_dir(&dirs.workspace, &dirs.session).unwrap();
    let log = dirs.session.join("doc-patch.jsonl");
    // The log cannot be appended to (something else sits at its path).
    fs::create_dir(&log).unwrap();
    let err = session.call("designos_open_file", json!({ "path": "other.designos", "discard_changes": true })).unwrap_err();
    assert!(err.contains("editor was not told") && err.contains("doc-patch.jsonl"), "{err}");

    let err = create(&mut session, "After").unwrap_err();
    assert!(err.contains("previous document"), "{err}");
    assert_eq!(session.engine.doc.nodes.values().filter(|n| n.name == "After").count(), 0, "the refused edit is rolled back");

    // Once the log works the switch is announced with `After` already inside `doc-sync.designos`
    // (not re-logged as a duplicate patch), and subsequent edits follow as patches.
    fs::remove_dir(&log).unwrap();
    create(&mut session, "After").unwrap();
    let lines = dirs.log();
    assert_eq!(lines.len(), 1, "{lines:?}");
    let LiveLogLine::DocEpoch(mark) = &lines[0] else { panic!("expected DocEpoch: {lines:?}") };
    let published = fs::read(dirs.session.join("doc-sync.designos")).unwrap();
    assert!(mark.matches(&published));
    let published_doc = Document::from_json(std::str::from_utf8(&published).unwrap()).unwrap();
    assert_eq!(published_doc.nodes.values().filter(|n| n.name == "After").count(), 1);

    create(&mut session, "Second").unwrap();
    let lines = dirs.log();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(matches!(lines[1], LiveLogLine::Patch(_)), "{lines:?}");
}
