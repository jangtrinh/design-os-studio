use designos_mcp::server::Session;
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static WORKSPACE_SEQ: AtomicU64 = AtomicU64::new(0);

struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let seq = WORKSPACE_SEQ.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("designos-mcp-{}-{nonce}-{seq}", std::process::id()));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn saved_file_roundtrip_and_unsaved_changes_guard() {
    let dir = Workspace::new();
    let mut s = Session::new(&dir.0).unwrap();
    let d = s.call("designos_get_document", json!({})).unwrap();
    let page = d["pages"][0]["node_id"].clone();
    let node = s.call("designos_create_node", json!({"parent_id":page,"kind":"text","name":"Heading","text":"Hello MCP"})).unwrap();
    assert!(s.call("designos_new_document", json!({"name":"Oops"})).is_err());
    s.call("designos_save_file", json!({"path":"first.designos"})).unwrap();
    s.call("designos_update_node", json!({"node_id":node["node"]["id"],"text":"Changed"})).unwrap();
    assert!(s.call("designos_open_file", json!({"path":"first.designos"})).is_err());
    s.call("designos_open_file", json!({"path":"first.designos","discard_changes":true})).unwrap();
    assert_eq!(s.call("designos_get_node", json!({"node_id":node["node"]["id"]})).unwrap()["node"], node["node"]);
    let original = fs::read(dir.0.join("first.designos")).unwrap();
    assert!(s.call("designos_save_file", json!({"path":"first.designos"})).is_err());
    assert_eq!(fs::read(dir.0.join("first.designos")).unwrap(), original);
    s.call("designos_save_file", json!({"path":"second.designos"})).unwrap();
    assert!(dir.0.join("second.designos").exists());
    let rendered = s.call("designos_render_png", json!({"path":"preview.png","node_id":node["node"]["id"],"scale":2.0})).unwrap();
    assert_eq!(rendered["rendered"], true);
    assert!(dir.0.join("preview.png").exists());
    let fig_exported = s.call("designos_export_fig_file", json!({"output_path":"exported.fig"})).unwrap();
    assert_eq!(fig_exported["saved"], true);
    assert!(fig_exported["bytes"].as_u64().unwrap() > 16);
    assert!(dir.0.join("exported.fig").exists());
}

#[test]
fn exporting_a_fig_reports_the_comment_threads_it_leaves_out() {
    let dir = Workspace::new();
    let mut s = Session::new(&dir.0).unwrap();
    let none = s.call("designos_export_fig_file", json!({"output_path":"none.fig"})).unwrap();
    assert_eq!(none["omitted_comment_threads"], 0);
    s.call("designos_add_comment", json!({"text":"Make this bigger"})).unwrap();
    s.call("designos_add_comment", json!({"text":"And this one"})).unwrap();
    let two = s.call("designos_export_fig_file", json!({"output_path":"two.fig"})).unwrap();
    assert_eq!(two["saved"], true);
    assert_eq!(two["omitted_comment_threads"], 2);
    // the native format keeps what the .fig could not
    s.call("designos_save_file", json!({"path":"kept.designos"})).unwrap();
    assert!(fs::read_to_string(dir.0.join("kept.designos")).unwrap().contains("Make this bigger"));
}

#[test]
fn paths_and_bad_imports_cannot_replace_the_session() {
    let dir = Workspace::new();
    let outside = Workspace::new();
    fs::write(outside.0.join("outside.designos"), "{}").unwrap();
    let mut s = Session::new(&dir.0).unwrap();
    let before = s.call("designos_get_document", json!({})).unwrap();
    assert!(s.call("designos_open_file", json!({"path":outside.0.join("outside.designos")})).is_err());
    assert!(s.call("designos_save_file", json!({"path":outside.0.join("escape.designos")})).is_err());
    assert!(s.call("designos_save_file", json!({"path":"../escape.designos"})).is_err());
    assert!(s.call("designos_save_file", json!({"path":"bad.json"})).is_err());
    fs::write(dir.0.join("bad.designos"), "not JSON").unwrap();
    assert!(s.call("designos_open_file", json!({"path":"bad.designos"})).is_err());
    let mut broken = s.engine.doc.clone();
    let id = broken.pages[0];
    broken.get_mut(id).unwrap().parent = Some(id);
    fs::write(dir.0.join("cycle.designos"), broken.to_json().unwrap()).unwrap();
    assert!(s.call("designos_open_file", json!({"path":"cycle.designos"})).is_err());
    assert_eq!(s.call("designos_get_document", json!({})).unwrap(), before);
    assert!(!outside.0.join("escape.designos").exists());
}

#[test]
fn imported_stale_id_counter_does_not_overwrite_existing_nodes() {
    let dir = Workspace::new();
    let mut s = Session::new(&dir.0).unwrap();
    let mut native = serde_json::to_value(&s.engine.doc).unwrap();
    native["next_id"] = json!(0);
    fs::write(dir.0.join("stale.designos"), native.to_string()).unwrap();
    let doc = s.call("designos_open_file", json!({"path":"stale.designos"})).unwrap();
    let page = doc["pages"][0]["node_id"].clone();
    let created = s.call("designos_create_node", json!({"parent_id":page,"kind":"rectangle"})).unwrap();
    assert_ne!(created["node"]["id"], page);
    assert_eq!(s.call("designos_get_document", json!({})).unwrap()["node_count"], 2);
    s.call("designos_delete_node", json!({"node_id":created["node"]["id"]})).unwrap();
    let next = s.call("designos_create_node", json!({"parent_id":page,"kind":"rectangle"})).unwrap();
    assert!(next["node"]["id"].as_u64().unwrap() > created["node"]["id"].as_u64().unwrap());

    let mut exhausted = serde_json::to_value(&s.engine.doc).unwrap();
    exhausted["next_id"] = json!(u64::MAX);
    fs::write(dir.0.join("exhausted.designos"), exhausted.to_string()).unwrap();
    s.call("designos_open_file", json!({"path":"exhausted.designos","discard_changes":true})).unwrap();
    let before = s.engine.doc.to_json().unwrap();
    assert!(s.call("designos_create_node", json!({"parent_id":page,"kind":"rectangle"})).unwrap_err().contains("exhausted"));
    assert_eq!(s.engine.doc.to_json().unwrap(), before);
}

#[test]
fn node_id_without_allocation_headroom_is_refused_on_open() {
    use designos_doc::{Node, NodeId, Rect};
    let dir = Workspace::new();
    let mut s = Session::new(&dir.0).unwrap();
    let before = s.call("designos_get_document", json!({})).unwrap();
    let mut doc = s.engine.doc.clone();
    let page = doc.pages[0];
    doc.insert(page, Node::rectangle(NodeId(u64::MAX), Rect::new(0.0, 0.0, 1.0, 1.0))).unwrap();
    fs::write(dir.0.join("maxid.designos"), doc.to_json().unwrap()).unwrap();

    let err = s.call("designos_open_file", json!({"path":"maxid.designos","discard_changes":true})).unwrap_err();

    assert!(err.contains("malformed") && err.contains("too close to the maximum"), "{err}");
    assert_eq!(s.call("designos_get_document", json!({})).unwrap(), before, "the session keeps its document");
}

/// A `null` number is what older versions saved for a NaN. It carries no data to lose, so the file
/// opens, but the tool result says how many numbers were read as their neutral value.
#[test]
fn a_native_file_with_null_numbers_opens_and_the_result_reports_the_repair() {
    use designos_doc::{Node, Rect};
    let dir = Workspace::new();
    let mut s = Session::new(&dir.0).unwrap();
    let mut doc = s.engine.doc.clone();
    let page = doc.pages[0];
    let id = doc.alloc_id();
    let rect = doc.insert(page, Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
    doc.get_mut(rect).unwrap().size.x = f32::NAN;
    doc.get_mut(rect).unwrap().opacity = f32::NAN;
    // The text the previous release wrote: serde_json's own `null` for each NaN.
    let legacy = serde_json::to_string(&doc).unwrap();
    assert!(serde_json::from_str::<designos_doc::Document>(&legacy).is_err(), "the strict parse must refuse the legacy text");
    fs::write(dir.0.join("legacy.designos"), legacy).unwrap();

    let opened = s.call("designos_open_file", json!({"path":"legacy.designos"})).unwrap();
    assert_eq!(opened["repaired_non_finite_numbers"], 2);
    assert_eq!(opened["node_count"], 2);
    let node = s.engine.doc.get(rect).unwrap();
    assert_eq!((node.size.x, node.opacity), (0.0, 1.0));

    // A file that needed no repair carries no such field.
    fs::write(dir.0.join("clean.designos"), s.engine.doc.to_json().unwrap()).unwrap();
    let opened = s.call("designos_open_file", json!({"path":"clean.designos"})).unwrap();
    assert!(opened.get("repaired_non_finite_numbers").is_none(), "{opened}");
}

/// 40 nested Fill frames, 200 wide, with a 10 px leaf hugging the right edge of the innermost
/// one. Widening the outermost frame to 600 moves the leaf to x = 590, but one relayout stops 32
/// hops down and queues the rest for the lazy pass.
fn queued_chain(s: &mut Session) -> designos_doc::NodeId {
    use designos_doc::{AutoLayout, LayoutAlign, LayoutChild, LayoutDirection, Node, NodeKind, Rect, Sizing};
    let doc = &mut s.engine.doc;
    let mut cur = doc.pages[0];
    let mut outermost = None;
    for i in 0..40 {
        let id = doc.alloc_id();
        let mut f = Node::frame(id, Rect::new(0.0, 0.0, 200.0, 40.0));
        if let NodeKind::Frame(fp) = &mut f.kind {
            fp.auto_layout = Some(AutoLayout {
                direction: LayoutDirection::Horizontal,
                primary_sizing: Sizing::Fixed,
                counter_sizing: Sizing::Fixed,
                primary_align: LayoutAlign::End,
                ..AutoLayout::default()
            });
        }
        if i > 0 {
            f.layout_child = LayoutChild { horizontal: Sizing::Fill, vertical: Sizing::Fill, ..LayoutChild::default() };
        }
        doc.insert(cur, f).unwrap();
        outermost.get_or_insert(id);
        cur = id;
    }
    let leaf = doc.alloc_id();
    doc.insert(cur, Node::rectangle(leaf, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
    let outermost = outermost.unwrap();
    doc.relayout_subtree(outermost);
    assert_eq!(doc.get(leaf).unwrap().transform.tx, 190.0, "precondition: the leaf hugs the 200 px frame");
    doc.get_mut(outermost).unwrap().size.x = 600.0;
    doc.relayout_subtree(outermost);
    assert!(doc.relayout_skipped_count() > 0, "precondition: the nest is deeper than one relayout reaches");
    assert_ne!(doc.get(leaf).unwrap().transform.tx, 590.0, "precondition: the leaf is not moved yet");
    leaf
}

#[test]
fn session_writes_and_exports_only_a_finished_layout() {
    let dir = Workspace::new();
    for (tool, args) in [
        ("designos_save_file", json!({"path":"queued.designos"})),
        ("designos_export_fig_file", json!({"output_path":"queued.fig"})),
        ("designos_render_png", json!({"path":"queued.png"})),
    ] {
        let mut s = Session::new(&dir.0).unwrap();
        let leaf = queued_chain(&mut s);
        let mut args = args;
        if tool == "designos_render_png" {
            args["node_id"] = json!(leaf.0);
        }
        s.call(tool, args).unwrap();
        assert_eq!(s.engine.doc.relayout_skipped_count(), 0, "{tool} must finish the queued layout first");
        assert_eq!(s.engine.doc.get(leaf).unwrap().transform.tx, 590.0, "{tool}");
        if tool == "designos_save_file" {
            let saved = designos_doc::Document::from_json(&fs::read_to_string(dir.0.join("queued.designos")).unwrap()).unwrap();
            assert_eq!(saved.get(leaf).unwrap().transform.tx, 590.0, "the saved file holds the finished layout, not the queued one");
        }
    }
}

fn unsaved_session(dir: &Workspace) -> Session {
    let mut s = Session::new(&dir.0).unwrap();
    s.call("designos_save_file", json!({"path":"first.designos"})).unwrap();
    let page = s.call("designos_get_document", json!({})).unwrap()["pages"][0]["node_id"].clone();
    s.call("designos_create_node", json!({"parent_id":page,"kind":"frame","name":"Unsaved"})).unwrap();
    s
}

#[test]
fn open_file_refuses_a_path_outside_the_workspace_before_it_looks_at_unsaved_changes() {
    let dir = Workspace::new();
    let mut s = unsaved_session(&dir);

    for outside in ["/etc/passwd", "../escape.designos"] {
        let err = s.call("designos_open_file", json!({"path": outside})).unwrap_err().to_json_value();
        assert_eq!(err["code"], "PATH_OUTSIDE_WORKSPACE", "{outside}: {err}");
        assert_eq!(err["path"], "/path", "{outside}: {err}");
    }
    let err = s.call("designos_open_file", json!({"path": "/etc/passwd", "discard_changes": true})).unwrap_err().to_json_value();
    assert_eq!(err["code"], "PATH_OUTSIDE_WORKSPACE", "{err}");
}

#[test]
fn the_unsaved_changes_hint_names_the_real_next_step_and_only_a_workspace_path() {
    let dir = Workspace::new();
    let mut s = unsaved_session(&dir);

    let err = s.call("designos_open_file", json!({"path": "first.designos"})).unwrap_err().to_json_value();

    assert_eq!(err["code"], "UNSAVED_CHANGES", "{err}");
    assert_eq!(err["path"], "/discard_changes", "{err}");
    let message = err["message"].as_str().unwrap();
    let next_step = err["next_step"].as_str().unwrap();
    for text in [message, next_step] {
        assert!(text.contains("designos_save_file"), "names the tool that saves: {text}");
        assert!(text.contains("discard_changes=true"), "names the parameter that exists: {text}");
        assert!(!text.contains("force"), "no parameter that does not exist: {text}");
        assert!(!text.contains(".designos/.designos"), "no doubled extension: {text}");
    }
    assert_eq!(err["corrected_example"], json!({"path": "first.designos", "discard_changes": true}), "{err}");

    let new_doc = s.call("designos_new_document", json!({"name": "Fresh"})).unwrap_err().to_json_value();
    assert_eq!(new_doc["code"], "UNSAVED_CHANGES", "{new_doc}");
    assert!(!new_doc["next_step"].as_str().unwrap().contains("force"), "{new_doc}");
    assert!(new_doc["next_step"].as_str().unwrap().contains("discard_changes=true"), "{new_doc}");
}

#[test]
fn a_missing_file_inside_the_workspace_is_reported_as_missing_not_as_unsaved_changes() {
    let dir = Workspace::new();
    let mut s = unsaved_session(&dir);

    let err = s.call("designos_open_file", json!({"path": "nothing-here.designos"})).unwrap_err().to_json_value();

    assert_eq!(err["code"], "INVALID_ARGUMENTS", "{err}");
}

#[test]
fn web_capture_between_64_and_512_mib_passes_the_size_gate() {
    let dir = Workspace::new();
    let mut s = Session::new(&dir.0).unwrap();

    let cap_65 = dir.0.join("capture_65mib.json");
    let f = fs::File::create(&cap_65).unwrap();
    f.set_len(65 * 1024 * 1024).unwrap();
    drop(f);

    let err = s.call("designos_import_web_capture", json!({"path": "capture_65mib.json"})).unwrap_err().to_json_value();
    let _ = fs::remove_file(&cap_65);

    assert_eq!(err["code"], "INVALID_ARGUMENTS", "{err}");
    let msg = err["message"].as_str().unwrap();
    assert!(msg.contains("invalid capture JSON"), "expected JSON parse error past size gate, got: {msg}");
    assert!(!msg.contains("import limit"), "65 MiB capture must not hit an import limit: {msg}");
}

#[test]
fn web_capture_over_512_mib_is_file_too_large() {
    let dir = Workspace::new();
    let mut s = Session::new(&dir.0).unwrap();

    let cap_513 = dir.0.join("capture_513mib.json");
    let len: u64 = 513 * 1024 * 1024;
    let f = fs::File::create(&cap_513).unwrap();
    f.set_len(len).unwrap();
    drop(f);

    let err = s.call("designos_import_web_capture", json!({"path": "capture_513mib.json"})).unwrap_err().to_json_value();
    let _ = fs::remove_file(&cap_513);

    assert_eq!(err["code"], "FILE_TOO_LARGE", "{err}");
    assert_eq!(err["path"], "/path", "{err}");
    assert_eq!(err["received"], json!(len), "{err}");
    assert_eq!(err["expected"], "<= 536870912 bytes", "{err}");
    assert_eq!(err["message"], "Input file exceeds the 512 MiB import limit", "{err}");
    assert_eq!(err["safe_to_retry"], false, "{err}");
    assert!(err["next_step"].as_str().unwrap().contains("Studio"), "{err}");
}

#[test]
fn open_file_between_64_and_512_mib_passes_the_size_gate() {
    let dir = Workspace::new();
    let mut s = Session::new(&dir.0).unwrap();

    let doc_65 = dir.0.join("sparse_65mib.designos");
    let f = fs::File::create(&doc_65).unwrap();
    f.set_len(65 * 1024 * 1024).unwrap();
    drop(f);

    let err = s.call("designos_open_file", json!({"path": "sparse_65mib.designos", "discard_changes": true})).unwrap_err().to_json_value();
    let _ = fs::remove_file(&doc_65);

    assert_eq!(err["code"], "DOCUMENT_INVALID", "{err}");
    assert_ne!(err["code"], "FILE_TOO_LARGE", "{err}");
}

#[test]
fn import_size_gate_accepts_exact_512_mib_and_never_partially_opens_oversized_valid_designos() {
    use std::io::Write;

    let dir = Workspace::new();
    let mut s = Session::new(&dir.0).unwrap();
    let before = s.call("designos_get_document", json!({})).unwrap();

    // 1. Exactly 536,870,912 bytes (512 MiB) is accepted by the size gate on web capture.
    let exact_cap = dir.0.join("exact_512mib.json");
    let f = fs::File::create(&exact_cap).unwrap();
    f.set_len(512 * 1024 * 1024).unwrap();
    drop(f);
    let cap_err = s.call("designos_import_web_capture", json!({"path": "exact_512mib.json"})).unwrap_err().to_json_value();
    let _ = fs::remove_file(&exact_cap);
    assert_ne!(cap_err["code"], "FILE_TOO_LARGE", "exact 512 MiB capture must pass the size gate: {cap_err}");
    assert!(cap_err["message"].as_str().unwrap().contains("invalid capture JSON"), "{cap_err}");

    // 2. A valid .designos file whose size exceeds 512 MiB is refused with FILE_TOO_LARGE and never opened partially.
    let mut valid_doc = designos_doc::Document::new("OversizedSentinelDoc");
    let page = valid_doc.add_page("SentinelPage");
    let id = valid_doc.alloc_id();
    valid_doc.insert(page, designos_doc::Node::frame(id, designos_doc::Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
    let oversized_path = dir.0.join("oversized_valid.designos");
    let mut f = fs::File::create(&oversized_path).unwrap();
    f.write_all(valid_doc.to_json().unwrap().as_bytes()).unwrap();
    let over_len: u64 = 512 * 1024 * 1024 + 1;
    f.set_len(over_len).unwrap();
    drop(f);

    let open_err = s.call("designos_open_file", json!({"path": "oversized_valid.designos", "discard_changes": true})).unwrap_err().to_json_value();
    let _ = fs::remove_file(&oversized_path);

    assert_eq!(open_err["code"], "FILE_TOO_LARGE", "{open_err}");
    assert_eq!(open_err["path"], "/path", "{open_err}");
    assert_eq!(open_err["received"], json!(over_len), "{open_err}");
    assert_eq!(open_err["expected"], "<= 536870912 bytes", "{open_err}");
    assert_eq!(open_err["safe_to_retry"], false, "{open_err}");
    assert!(open_err["next_step"].as_str().unwrap().contains("Studio"), "{open_err}");
    assert_eq!(s.call("designos_get_document", json!({})).unwrap(), before, "session document must not be partially replaced");
}
