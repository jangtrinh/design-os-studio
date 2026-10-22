//! A literal colour written through `update_node` or `apply_batch` replaces what the node was bound or
//! linked to, so the canvas shows the colour that was sent, and one undo brings both back.
#![allow(clippy::result_large_err)]

mod support;

use designos_doc::live_patch::{replay_live_patch, LiveLogLine, LivePatchState};
use designos_doc::{Document, NodeId, NodeKind, Paint};
use designos_mcp::engine::Engine;
use designos_mcp::server::Session;
use serde_json::{json, Value};
use std::{fs, path::PathBuf, time::SystemTime};
use support::*;

// ---- a literal colour replaces the binding -------------------------------------------------------

#[test]
fn hex_fill_on_a_bound_frame_drops_the_binding_and_draws_the_new_colour() {
    let mut e = palette();
    let id = bound_frame(&mut e);
    assert_bound_to(&e, id, "color/primary", PRIMARY);

    let reply = e.call("designos_update_node", json!({"node_id": id.0, "fill": RED})).unwrap();

    assert_eq!(reply["changed"], true, "{reply}");
    assert_plain(&e, id, RED);
    let (r, g, b) = centre_pixel(&e, id);
    assert!(r > 240 && g < 15 && b < 15, "the frame renders red, got {r},{g},{b}");
}

#[test]
fn hex_fill_on_bound_text_drops_the_binding_and_draws_the_new_colour() {
    let mut e = palette();
    let id = bound_text(&mut e);
    assert_bound_to(&e, id, "color/primary", PRIMARY);

    e.call("designos_update_node", json!({"node_id": id.0, "fill": RED})).unwrap();

    assert_plain(&e, id, RED);
    assert!(red_pixels(&e, id) > 20, "the rendered text is red");
}

#[test]
fn fills_array_on_a_bound_node_drops_the_binding_too() {
    let mut e = palette();
    let frame = bound_frame(&mut e);
    let text = bound_text(&mut e);
    let paint = serde_json::to_value(Paint::solid(colour(RED))).unwrap();

    e.call("designos_update_node", json!({"node_id": frame.0, "fills": [paint.clone()]})).unwrap();
    e.call("designos_update_node", json!({"node_id": text.0, "fills": [paint]})).unwrap();

    assert_plain(&e, frame, RED);
    assert_plain(&e, text, RED);
}

#[test]
fn hex_equal_to_the_variable_value_still_detaches_and_reports_a_change() {
    let mut e = palette();
    let id = bound_frame(&mut e);

    let reply = e.call("designos_update_node", json!({"node_id": id.0, "fill": PRIMARY})).unwrap();

    assert_eq!(reply["changed"], true, "the binding went away, so the node changed: {reply}");
    assert_plain(&e, id, PRIMARY);
}

#[test]
fn every_fill_binding_and_the_fill_style_link_go_with_the_replaced_paints() {
    let mut e = palette();
    let id = bound_frame(&mut e);
    {
        let n = e.doc.get_mut(id).unwrap();
        n.variables.bindings.insert("fills/0/opacity".into(), "color/gap".into());
        n.variables.bindings.insert("width".into(), "color/gap".into());
        n.style_refs.insert("fill".into(), "S:surface".into());
    }

    e.call("designos_update_node", json!({"node_id": id.0, "fill": RED})).unwrap();

    assert_plain(&e, id, RED);
    assert_eq!(node(&e, id).variables.bindings.get("width").map(String::as_str), Some("color/gap"), "bindings on other properties stay");
}

#[test]
fn a_hex_fill_on_a_style_linked_node_drops_the_style_link() {
    let mut e = palette();
    let id = apply(&mut e, json!({"kind": "frame", "name": "Card", "width": 80, "height": 40, "fill": "#18181B"}));
    e.call(
        "designos_apply_batch",
        json!({"ops": [{
            "op": "set_style",
            "upsert": [{"key": "surface", "name": "Surface", "property": "fill", "fill": "#18181B"}],
            "apply": [{"node": id.0, "style": "Surface"}]
        }]}),
    )
    .unwrap();
    assert!(node(&e, id).style_refs.contains_key("fill"), "the style is linked");

    e.call("designos_update_node", json!({"node_id": id.0, "fill": RED})).unwrap();

    assert_plain(&e, id, RED);
}

#[test]
fn batch_update_with_a_hex_fill_drops_the_binding() {
    let mut e = palette();
    let frame = bound_frame(&mut e);
    let text = bound_text(&mut e);

    e.call("designos_apply_batch", json!({"ops": [{"op": "update", "node": frame.0, "fill": RED}, {"op": "update", "node": text.0, "fill": RED}]})).unwrap();

    assert_plain(&e, frame, RED);
    assert_plain(&e, text, RED);
    assert!(red_pixels(&e, text) > 20);
}

#[test]
fn one_undo_restores_the_colour_and_the_binding() {
    let mut e = palette();
    let frame = bound_frame(&mut e);
    let text = bound_text(&mut e);
    let before_frame = node(&e, frame).clone();
    let before_text = node(&e, text).clone();

    e.call("designos_update_node", json!({"node_id": frame.0, "fill": RED})).unwrap();
    e.call("designos_apply_batch", json!({"ops": [{"op": "update", "node": text.0, "fill": RED}]})).unwrap();
    assert_plain(&e, frame, RED);
    assert_plain(&e, text, RED);

    e.call("designos_undo", json!({})).unwrap();
    assert_eq!(node(&e, text), &before_text, "one undo restores the text node");
    assert_bound_to(&e, text, "color/primary", PRIMARY);
    e.call("designos_undo", json!({})).unwrap();
    assert_eq!(node(&e, frame), &before_frame, "one undo restores the frame");
    assert_bound_to(&e, frame, "color/primary", PRIMARY);

    e.call("designos_redo", json!({})).unwrap();
    assert_plain(&e, frame, RED);
}

fn apply_red_fill_style(e: &mut Engine, id: NodeId) {
    let op =
        json!({"op": "set_style", "upsert": [{"key": "s_red", "name": "Red", "property": "fill", "fill": RED}], "apply": [{"node": id.0, "style": "Red"}]});
    e.call("designos_apply_batch", json!({ "ops": [op] })).expect("set_style");
}

#[test]
fn linking_a_fill_style_drops_the_variable_binding_and_one_undo_restores_it() {
    let mut e = palette();
    let frame = bound_frame(&mut e);
    let text = bound_text(&mut e);
    let before_frame = node(&e, frame).clone();
    let before_text = node(&e, text).clone();

    apply_red_fill_style(&mut e, frame);
    apply_red_fill_style(&mut e, text);

    for id in [frame, text] {
        assert_eq!(fill_bindings(&e, id), Vec::<(String, String)>::new(), "no fill binding left on {id:?}");
        assert_eq!(node(&e, id).style_refs.get("fill").map(String::as_str), Some("s_red"), "the style is linked to {id:?}");
        let drawn = drawn_paints(&e, id);
        assert!(drawn.len() == 1 && solid_hex(&drawn[0]) == RED, "{id:?} draws the style colour: {drawn:?}");
    }
    assert!(red_pixels(&e, text) > 20, "the rendered text is red");
    let (r, g, b) = centre_pixel(&e, frame);
    assert!(r > 240 && g < 15 && b < 15, "the frame renders red, got {r},{g},{b}");

    e.call("designos_undo", json!({})).unwrap();
    assert_eq!(node(&e, text), &before_text, "one undo restores the text node");
    assert_bound_to(&e, text, "color/primary", PRIMARY);
    e.call("designos_undo", json!({})).unwrap();
    assert_eq!(node(&e, frame), &before_frame, "one undo restores the frame");
    assert_bound_to(&e, frame, "color/primary", PRIMARY);
}

#[test]
fn linking_a_stroke_style_leaves_the_fill_binding_alone() {
    let mut e = palette();
    let frame = bound_frame(&mut e);

    e.call(
        "designos_apply_batch",
        json!({"ops": [{"op": "set_style", "upsert": [{"key": "s_edge", "name": "Edge", "property": "stroke", "fill": "#00FF00"}],
                        "apply": [{"node": frame.0, "style": "Edge"}]}]}),
    )
    .expect("set_style");

    assert_bound_to(&e, frame, "color/primary", PRIMARY);
    assert_eq!(node(&e, frame).style_refs.get("stroke").map(String::as_str), Some("s_edge"));
}

/// A session bound to an open editor: its edits go to `doc-patch.jsonl`, and the editor replays them.
struct EditorSession {
    base: PathBuf,
    session: Session,
    mirror: Document,
    state: LivePatchState,
    replayed: usize,
}

impl EditorSession {
    fn new() -> Self {
        let nonce = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
        let base = std::env::temp_dir().join(format!("designos-mcp-colour-{}-{nonce}", std::process::id()));
        let (workspace, session_dir) = (base.join("ws"), base.join("session"));
        fs::create_dir_all(&workspace).unwrap();
        fs::create_dir_all(&session_dir).unwrap();
        let mut doc = Document::new("Canvas");
        doc.add_page("Page");
        fs::write(session_dir.join("canvas.designos"), doc.to_json().unwrap()).unwrap();
        let session = Session::with_session_dir(&workspace, &session_dir).unwrap();
        Self { base, session, mirror: doc, state: LivePatchState::default(), replayed: 0 }
    }

    fn call(&mut self, tool: &str, args: Value) -> Value {
        self.session.call(tool, args).unwrap_or_else(|e| panic!("{tool}: {e:?}"))
    }

    /// The editor replays every patch the agent has logged since it last looked.
    fn editor_catches_up(&mut self) {
        let raw = fs::read_to_string(self.base.join("session").join("doc-patch.jsonl")).unwrap_or_default();
        for line in raw.lines().skip(self.replayed) {
            self.replayed += 1;
            if let LiveLogLine::Patch(patch) = LiveLogLine::parse(line.as_bytes()).expect("log line") {
                let report = replay_live_patch(&mut self.mirror, &patch, &mut self.state, &mut |d, c| c.apply(d).map(|_| ())).unwrap();
                assert!(report.skipped.is_empty(), "{:?}", report.skipped);
            }
        }
    }
}

impl Drop for EditorSession {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

#[test]
fn the_detach_reaches_the_open_editor_through_the_edit_log() {
    let mut s = EditorSession::new();
    s.call("designos_apply_batch", palette_ops());
    let page = s.mirror.pages[0].0;
    let frame = s.call(
        "designos_apply_spec",
        json!({"parent_id": page, "spec": {"kind": "frame", "name": "Button", "width": 120, "height": 40, "fill": "var:Color/Primary"}}),
    )["root_node_id"]
        .as_u64()
        .map(NodeId)
        .unwrap();
    let text = s.call(
        "designos_apply_spec",
        json!({"parent_id": page, "spec": {"kind": "text", "name": "Label", "text": "Verify", "width": 160, "height": 40, "text_color": "var:Color/Primary"}}),
    )["root_node_id"]
        .as_u64()
        .map(NodeId)
        .unwrap();
    s.editor_catches_up();
    for id in [frame, text] {
        assert_eq!(
            s.mirror.get(id).unwrap().variables.bindings.get("fills/0").map(String::as_str),
            Some("color/primary"),
            "the editor holds the binding first"
        );
    }

    s.call("designos_update_node", json!({"node_id": frame.0, "fill": RED}));
    s.call("designos_apply_batch", json!({"ops": [{"op": "update", "node": text.0, "fill": RED}]}));
    s.editor_catches_up();

    for id in [frame, text] {
        let n = s.mirror.get(id).unwrap();
        assert!(n.variables.bindings.keys().all(|k| !k.starts_with("fills/")), "the editor copy of {id:?} lost the binding: {:?}", n.variables.bindings);
        let drawn = designos_doc::resolved_node(&s.mirror, id).unwrap();
        let paints = match &drawn.kind {
            NodeKind::Text(t) => t.style.fills.clone(),
            _ => drawn.fills.clone(),
        };
        assert!(paints.len() == 1 && solid_hex(&paints[0]) == RED, "the editor copy of {id:?} draws red: {paints:?}");
    }
    let agent = s.session.engine.doc.get(frame).unwrap();
    assert_eq!(s.mirror.get(frame).unwrap().variables, agent.variables, "agent and editor agree");
}
