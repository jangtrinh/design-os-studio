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
