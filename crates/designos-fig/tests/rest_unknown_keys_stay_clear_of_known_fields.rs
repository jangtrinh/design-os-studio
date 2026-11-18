//! Unknown REST keys are kept on `Node::extra`, but a key the importer reads or the exporter writes is
//! never one of them: a copy kept beside the real field would come back stale after the user edits it.

mod common;

use common::{find, rich_document};
use designos_doc::*;
use designos_fig::{export_rest_json, import_rest_json};
use serde_json::{json, Value};

fn page_payload(children: Value) -> String {
    json!({"name":"x","editorType":"figma","document":{"id":"0:0","name":"Document","type":"DOCUMENT","children":[
        {"id":"0:1","name":"Page","type":"CANVAS","children":children}]}})
    .to_string()
}

fn exported_children(doc: &Document) -> Vec<Value> {
    export_rest_json(doc)["document"]["children"][0]["children"].as_array().cloned().unwrap_or_default()
}

#[test]
fn nothing_the_exporter_writes_comes_back_as_an_unknown_key() {
    let doc = rich_document();
    let reimported = import_rest_json(&export_rest_json(&doc).to_string()).unwrap();
    let leaked: Vec<(String, Vec<String>)> =
        reimported.nodes.values().filter(|n| !n.extra.is_empty()).map(|n| (n.name.clone(), n.extra.keys().cloned().collect())).collect();
    assert!(leaked.is_empty(), "known keys captured as unknown: {leaked:?}");
}

#[test]
fn keys_the_importer_reads_are_not_kept_as_unknown() {
    let payload = page_payload(json!([
        {"id":"1:1","name":"Frame","type":"FRAME","clipsContent":true,"layoutMode":"HORIZONTAL","counterAxisAlignContent":"SPACE_BETWEEN",
         "size":{"x":100.0,"y":100.0},"relativeTransform":[[1,0,0],[0,1,0]],"futureFrameKey":1,"children":[
            {"id":"1:2","name":"Label","type":"TEXT","characters":"hi","textAutoResize":"HEIGHT","textTruncation":"ENDING","maxLines":2,
             "lineTypes":["NONE"],"size":{"x":50.0,"y":20.0},"relativeTransform":[[1,0,0],[0,1,0]]}]}
    ]));
    let doc = import_rest_json(&payload).unwrap();
    assert_eq!(find(&doc, "Frame").extra.keys().collect::<Vec<_>>(), ["futureFrameKey"]);
    assert!(find(&doc, "Label").extra.is_empty(), "{:?}", find(&doc, "Label").extra);
}

#[test]
fn a_frame_that_stops_clipping_does_not_get_its_old_clip_back_on_export() {
    let payload = page_payload(json!([
        {"id":"1:1","name":"Frame","type":"FRAME","clipsContent":true,"size":{"x":100.0,"y":100.0},"relativeTransform":[[1,0,0],[0,1,0]]}
    ]));
    let mut doc = import_rest_json(&payload).unwrap();
    let id = find(&doc, "Frame").id;
    let NodeKind::Frame(fp) = &mut doc.get_mut(id).unwrap().kind else { panic!("frame") };
    assert!(fp.clips_content);
    fp.clips_content = false;
    let out = exported_children(&doc);
    assert_ne!(out[0]["clipsContent"], json!(true), "the stale imported value came back: {}", out[0]);
}

#[test]
fn an_extra_key_cannot_replace_a_key_the_exporter_writes() {
    let mut doc = import_rest_json(&page_payload(json!([
        {"id":"1:1","name":"Rect","type":"RECTANGLE","size":{"x":10.0,"y":10.0},"relativeTransform":[[1,0,0],[0,1,0]]}
    ])))
    .unwrap();
    let id = find(&doc, "Rect").id;
    let n = doc.get_mut(id).unwrap();
    for k in ["id", "name", "visible", "type", "opacity", "size"] {
        n.extra.insert(k.into(), json!("shadow"));
    }
    let out = exported_children(&doc);
    assert_eq!(out[0]["name"], "Rect");
    assert_eq!(out[0]["type"], "RECTANGLE");
    assert_eq!(out[0]["visible"], true);
    assert_ne!(out[0]["id"], "shadow");
    assert_ne!(out[0]["size"], "shadow");
    assert_ne!(out[0]["opacity"], "shadow");
}

#[test]
fn unknown_keys_survive_rest_import_native_save_native_load_and_rest_export() {
    let payload = page_payload(json!([
        {"id":"1:1","name":"Rect","type":"RECTANGLE","futureNodeKey":[1,2,3],"size":{"x":10.0,"y":10.0},"relativeTransform":[[1,0,0],[0,1,0]]},
        {"id":"1:2","name":"Stamp","type":"STAMP","stampType":"heart","futureStampKey":{"n":2},"size":{"x":10.0,"y":10.0},"relativeTransform":[[1,0,0],[0,1,0]]}
    ]));
    let doc = import_rest_json(&payload).unwrap();
    let reopened = Document::from_json(&doc.to_json().unwrap()).unwrap();
    let out = exported_children(&reopened);
    assert_eq!(out[0]["futureNodeKey"], json!([1, 2, 3]));
    assert_eq!(out[1]["futureStampKey"], json!({"n": 2}));
}

#[test]
fn unknown_connector_keys_survive_a_native_save() {
    let payload = json!({"name":"x","editorType":"figjam","document":{"id":"0:0","name":"Document","type":"DOCUMENT","children":[
        {"id":"0:1","name":"Page","type":"CANVAS","children":[
            {"id":"1:1","name":"Connector","type":"CONNECTOR","size":{"x":100.0,"y":100.0},"relativeTransform":[[1,0,0],[0,1,0]],
             "connectorStart":{"position":{"x":0,"y":0}},"connectorEnd":{"position":{"x":100,"y":100}},"futureKey":{"n":1}}]}]}})
    .to_string();
    let doc = import_rest_json(&payload).unwrap();
    let reopened = Document::from_json(&doc.to_json().unwrap()).unwrap();
    assert_eq!(exported_children(&reopened)[0]["futureKey"], json!({"n": 1}));
}

#[test]
fn text_and_board_keys_the_exporter_writes_do_not_come_back_as_unknown() {
    let mut doc = Document::new("text");
    let page = doc.add_page("Page");
    let id = doc.alloc_id();
    let mut text = Node::text(id, Rect::new(0.0, 0.0, 120.0, 40.0), "Hello world", TextStyle::default());
    if let NodeKind::Text(t) = &mut text.kind {
        t.max_lines = Some(2);
        t.text_wrap = TextWrap::Balance;
        t.runs.push(TextRun { start: 0, end: 5, style: t.style.clone() });
    }
    doc.insert(page, text).unwrap();
    let sticky = doc.alloc_id();
    doc.insert(page, Node::sticky(sticky, Rect::new(200.0, 0.0, 160.0, 160.0), "note")).unwrap();
    let reimported = import_rest_json(&export_rest_json(&doc).to_string()).unwrap();
    let leaked: Vec<_> = reimported.nodes.values().filter(|n| !n.extra.is_empty()).map(|n| (&n.name, &n.extra)).collect();
    assert!(leaked.is_empty(), "{leaked:?}");
}

/// The saved form of a board item writes its kept keys beside its own fields, so a kept key spelled like one
/// of those fields would be written twice and the file would no longer open.
#[test]
fn an_unknown_key_spelled_like_a_saved_board_field_does_not_corrupt_a_native_save() {
    fn field_names<T: serde::Serialize + Default>() -> Vec<String> {
        serde_json::to_value(T::default()).unwrap().as_object().unwrap().keys().cloned().collect()
    }
    let mut collide = |ty: &str, names: Vec<String>| {
        let mut node = json!({"id":"1:1","name":"Item","type":ty,"size":{"x":100.0,"y":100.0},"relativeTransform":[[1,0,0],[0,1,0]]});
        if ty == "CONNECTOR" {
            node["connectorStart"] = json!({"position":{"x":0,"y":0}});
            node["connectorEnd"] = json!({"position":{"x":100,"y":100}});
        }
        for name in &names {
            node[name] = json!({"future": name});
        }
        let doc = import_rest_json(&page_payload(json!([node]))).unwrap();
        let saved = doc.to_json().unwrap();
        let reopened = Document::from_json(&saved).unwrap_or_else(|e| panic!("{ty} with keys {names:?} no longer opens: {e}"));
        let item = |d: &Document| d.nodes.values().find(|n| n.name == "Item").map(|n| n.kind.clone()).unwrap();
        assert_eq!(item(&reopened), item(&doc), "{ty}: the item changed through a save and a load");
    };
    collide("STICKY", field_names::<StickyProps>());
    collide("SHAPE_WITH_TEXT", field_names::<ShapeWithTextProps>());
    collide("CONNECTOR", field_names::<ConnectorProps>());
}

#[test]
fn prototype_dev_and_grid_keys_other_import_passes_read_are_not_kept_as_unknown() {
    let payload = page_payload(json!([
        {"id":"1:1","name":"Frame","type":"FRAME","size":{"x":100.0,"y":100.0},"relativeTransform":[[1,0,0],[0,1,0]],
         "reactions":[],"transitionNodeID":null,"transitionDuration":300.0,"transitionEasing":{"type":"EASE_IN"},
         "scrollBehavior":"SCROLLS","overflowDirection":"NONE","isFixed":false,"devStatus":{"type":"READY_FOR_DEV"},
         "textDecorationSkipInk":true,"futureFrameKey":1}
    ]));
    let doc = import_rest_json(&payload).unwrap();
    assert_eq!(find(&doc, "Frame").extra.keys().collect::<Vec<_>>(), ["futureFrameKey"]);
}

#[test]
fn keys_that_only_repeat_modelled_geometry_are_not_kept_but_interactions_are() {
    let payload = page_payload(json!([
        {"id":"1:1","name":"Rect","type":"RECTANGLE","size":{"x":10.0,"y":10.0},"relativeTransform":[[0.866,-0.5,0],[0.5,0.866,0]],
         "rotation":30.0,"absoluteRenderBounds":{"x":0.0,"y":0.0,"width":12.0,"height":12.0},"background":[],
         "interactions":[],"futureKey":1}
    ]));
    let doc = import_rest_json(&payload).unwrap();
    assert_eq!(find(&doc, "Rect").extra.keys().collect::<Vec<_>>(), ["futureKey", "interactions"]);
}
