//! A `.fig` save either writes a shader message whole or fails. A key the schema does not name, at any depth of
//! the message, would be dropped by the encoder, so the export refuses it and says where it is and what to do.
use designos_doc::*;
use designos_fig::{export_fig, FigError};
use serde_json::{json, Value};

const NEXT_ACTION: &str = "save as .designos to keep it, or remove the shader";

fn effect_doc(layer: &str, raw: Value) -> Document {
    let mut doc = Document::new("ShaderUnknownKeys");
    let page = doc.add_page("Page 1");
    let id = doc.alloc_id();
    let mut node = Node::rectangle(id, Rect::new(0.0, 0.0, 100.0, 80.0));
    node.name = layer.into();
    node.effects = vec![Effect::Shader { shader: ShaderRef { raw }, visible: true }];
    doc.insert(page, node).unwrap();
    doc
}

fn fill_doc(layer: &str, raw: Value) -> Document {
    let mut doc = Document::new("ShaderUnknownKeys");
    let page = doc.add_page("Page 1");
    let id = doc.alloc_id();
    let mut node = Node::rectangle(id, Rect::new(0.0, 0.0, 100.0, 80.0));
    node.name = layer.into();
    node.fills = vec![Paint { kind: PaintKind::Shader(ShaderRef { raw }), opacity: 1.0, visible: true, blend: BlendMode::Normal }];
    doc.insert(page, node).unwrap();
    doc
}

fn refusal(doc: &Document) -> String {
    match export_fig(doc) {
        Err(FigError::Format(m)) => m,
        Err(other) => panic!("expected FigError::Format, got {other:?}"),
        Ok(_) => panic!("the export must refuse a shader key the schema does not name"),
    }
}

fn assert_refused(doc: &Document, layer: &str, path: &str) {
    let m = refusal(doc);
    assert!(m.contains(&format!("{layer:?}")), "the error names the layer {layer}: {m}");
    assert!(m.contains(&format!("{path:?}")), "the error names the key path {path}: {m}");
    assert!(m.contains(NEXT_ACTION), "the error names the next action: {m}");
}

fn reference() -> Value {
    json!({"guid": {"sessionID": 10, "localID": 25}, "assetRef": {"key": "k", "version": "1"}})
}

#[test]
fn an_unknown_key_on_a_nested_message_fails_the_export() {
    let mut id = reference();
    id["futureFlag"] = json!(true);
    assert_refused(&effect_doc("Glow", json!({"customEffectId": id})), "Glow", "customEffectId.futureFlag");
}

#[test]
fn an_unknown_key_on_a_nested_struct_fails_the_export() {
    let doc = fill_doc("Pattern", json!({"patternSpacing": {"x": 4.0, "y": 8.0, "future": 1}, "customEffectId": reference()}));
    assert_refused(&doc, "Pattern", "patternSpacing.future");
}

#[test]
fn an_unknown_key_two_levels_down_names_the_whole_path() {
    let mut id = reference();
    id["guid"]["epoch"] = json!(7);
    assert_refused(&effect_doc("Deep", json!({"customEffectId": id})), "Deep", "customEffectId.guid.epoch");
}

#[test]
fn an_unknown_key_inside_an_array_of_messages_names_the_index() {
    let raw = json!({
        "customEffectId": reference(),
        "componentPropAssignments": [
            {"defID": {"sessionID": 1, "localID": 2}, "value": {"floatValue": 0.5}},
            {"defID": {"sessionID": 1, "localID": 3}, "value": {"boolValue": true, "future": 1}}
        ]
    });
    assert_refused(&effect_doc("Props", raw), "Props", "componentPropAssignments[1].value.future");
}

#[test]
fn an_unknown_top_level_key_names_the_layer_and_the_next_action() {
    assert_refused(&effect_doc("Top", json!({"customEffectId": reference(), "futureTop": 1})), "Top", "futureTop");
}

#[test]
fn an_unknown_key_in_an_effect_style_names_the_style() {
    let mut doc = Document::new("ShaderStyle");
    doc.add_page("Page 1");
    let mut id = reference();
    id["futureFlag"] = json!(true);
    doc.styles.insert("Shadow/Shader".into(), Style::Effect(vec![Effect::Shader { shader: ShaderRef { raw: json!({"customEffectId": id}) }, visible: true }]));
    doc.style_meta.insert("Shadow/Shader".into(), StyleMeta { name: "Shadow/Shader".into(), property: StyleProperty::Effect, ..Default::default() });
    assert_refused(&doc, "Shadow/Shader", "customEffectId.futureFlag");
}

#[test]
fn a_nested_value_that_cannot_be_written_fails_the_export_instead_of_being_left_out() {
    // `assetRef.key` is a string in the schema; the sibling `version` converts, so a lenient conversion would keep it and drop the key
    let mut id = reference();
    id["assetRef"]["key"] = json!(5);
    let m = refusal(&effect_doc("Cut", json!({"customEffectId": id})));
    assert!(m.contains("\"Cut\"") && m.contains("customEffectId") && m.contains(NEXT_ACTION), "{m}");

    // one element of an array of messages cannot be written: the others are not written without it
    let raw = json!({
        "customEffectId": reference(),
        "componentPropAssignments": [
            {"defID": {"sessionID": 1, "localID": 2}, "value": {"floatValue": 0.5}},
            {"defID": {"sessionID": 1, "localID": 3}, "value": {"floatValue": "high"}}
        ]
    });
    let m = refusal(&effect_doc("Short", raw));
    assert!(m.contains("\"Short\"") && m.contains("componentPropAssignments") && m.contains(NEXT_ACTION), "{m}");
}

#[test]
fn a_shader_whose_every_key_the_schema_names_exports() {
    let raw = json!({
        "customEffectId": reference(),
        "radius": 12.0,
        "count": 3,
        "startOffset": {"x": 2.0, "y": 6.0},
        "componentPropAssignments": [{"defID": {"sessionID": 1, "localID": 2}, "value": {"floatValue": 0.5}}]
    });
    export_fig(&effect_doc("Whole", raw)).expect("every key is a field of the schema");
    let pattern = json!({"patternSpacing": {"x": 4.0, "y": 8.0}, "sourceNodeId": "3:44", "customEffectId": reference()});
    export_fig(&fill_doc("WholePattern", pattern)).expect("a GUID written as session:local is not an unknown key");
}
