//! REST and plugin JSON names a shader paint or effect `SHADER` and keeps the reference under `shader`; the
//! flattened `CUSTOM` form is the shape the `.fig` reader hands over and still imports.
use designos_doc::*;
use designos_fig::{decode_fig_to_json, export_fig, export_rest_json, import_rest_json};
use serde_json::{json, Value};

fn reference() -> Value {
    json!({
        "customEffectId": {"guid": {"sessionID": 10, "localID": 25}, "assetRef": {"key": "shader_key_abc", "version": "1"}},
        "componentPropAssignments": [{"defID": {"sessionID": 1, "localID": 2}, "value": {"floatValue": 0.5}}]
    })
}

fn rest_file(fills: Value, effects: Value) -> String {
    json!({
        "name": "ShaderRest",
        "document": {"id": "0:0", "name": "Document", "type": "DOCUMENT", "children": [{
            "id": "0:1", "name": "Page 1", "type": "CANVAS", "children": [{
                "id": "1:2", "name": "ShaderLayer", "type": "RECTANGLE",
                "absoluteBoundingBox": {"x": 0.0, "y": 0.0, "width": 100.0, "height": 80.0},
                "fills": fills, "effects": effects
            }]
        }]}
    })
    .to_string()
}

fn shader_type_file() -> String {
    rest_file(
        json!([
            {"type": "SOLID", "color": {"r": 1.0, "g": 0.0, "b": 0.0, "a": 1.0}},
            {"type": "SHADER", "opacity": 0.5, "visible": true, "blendMode": "OVERLAY", "shader": reference()}
        ]),
        json!([{"type": "SHADER", "visible": false, "shader": reference()}]),
    )
}

fn custom_alias_file() -> String {
    let mut fill = json!({"type": "CUSTOM", "opacity": 0.5, "visible": true, "blendMode": "OVERLAY"});
    let mut effect = json!({"type": "CUSTOM", "visible": false});
    for (k, v) in reference().as_object().unwrap() {
        fill[k] = v.clone();
        effect[k] = v.clone();
    }
    rest_file(json!([{"type": "SOLID", "color": {"r": 1.0, "g": 0.0, "b": 0.0, "a": 1.0}}, fill]), json!([effect]))
}

fn layer(doc: &Document) -> &Node {
    doc.nodes.values().find(|n| n.name == "ShaderLayer").expect("ShaderLayer")
}

fn exported_layer(doc: &Document) -> Value {
    export_rest_json(doc)["document"]["children"][0]["children"][0].clone()
}

#[test]
fn the_shader_type_imports_a_shader_paint_and_effect_with_their_own_fields() {
    let doc = import_rest_json(&shader_type_file()).expect("import_rest_json");
    let node = layer(&doc);
    assert_eq!(node.fills.len(), 2, "the shader fill keeps its slot after the solid");
    let fill = &node.fills[1];
    assert!((fill.opacity - 0.5).abs() < 1e-6);
    assert!(fill.visible);
    assert_eq!(fill.blend, BlendMode::Overlay);
    assert!(matches!(&fill.kind, PaintKind::Shader(s) if s.raw == reference()), "the object under shader is the raw message: {:?}", fill.kind);
    assert_eq!(node.effects.len(), 1);
    assert!(matches!(&node.effects[0], Effect::Shader { shader, visible: false } if shader.raw == reference()), "{:?}", node.effects[0]);
}

#[test]
fn the_flattened_custom_form_keeps_importing() {
    let doc = import_rest_json(&custom_alias_file()).expect("import the CUSTOM form");
    let node = layer(&doc);
    assert!(matches!(&node.fills[1].kind, PaintKind::Shader(s) if s.raw == reference()), "{:?}", node.fills);
    assert!(matches!(&node.effects[0], Effect::Shader { shader, visible: false } if shader.raw == reference()), "{:?}", node.effects);
}

#[test]
fn the_flattened_custom_form_imports_the_same_shader() {
    let alias = import_rest_json(&custom_alias_file()).expect("import the CUSTOM form");
    let shader = import_rest_json(&shader_type_file()).expect("import the SHADER form");
    assert_eq!(layer(&alias).fills, layer(&shader).fills);
    assert_eq!(layer(&alias).effects, layer(&shader).effects);
}

#[test]
fn rest_export_writes_the_shader_type_with_the_raw_object_under_shader() {
    let doc = import_rest_json(&custom_alias_file()).expect("import the CUSTOM form");
    let out = exported_layer(&doc);

    let fill = &out["fills"][1];
    assert_eq!(fill["type"], json!("SHADER"));
    assert_eq!(fill["shader"], reference());
    assert_eq!(fill["opacity"], json!(0.5));
    assert_eq!(fill["visible"], json!(true));
    assert_eq!(fill["blendMode"], json!("OVERLAY"));
    assert!(fill.get("customEffectId").is_none(), "the reference is not flattened onto the paint: {fill}");

    let effect = &out["effects"][0];
    assert_eq!(effect["type"], json!("SHADER"));
    assert_eq!(effect["shader"], reference());
    assert_eq!(effect["visible"], json!(false));
    assert!(effect.get("customEffectId").is_none(), "the reference is not flattened onto the effect: {effect}");
}

#[test]
fn the_exported_shader_shape_imports_back_unchanged() {
    let first = import_rest_json(&shader_type_file()).expect("import_rest_json");
    assert_eq!((layer(&first).fills.len(), layer(&first).effects.len()), (2, 1), "the shaders are imported before they are exported");
    let exported = export_rest_json(&first);
    let second = import_rest_json(&exported.to_string()).expect("import the exported json");
    assert_eq!(layer(&second).fills, layer(&first).fills);
    assert_eq!(layer(&second).effects, layer(&first).effects);
}

#[test]
fn an_effect_field_beside_the_shader_object_is_kept() {
    let file = rest_file(json!([]), json!([{"type": "SHADER", "visible": true, "blendMode": "MULTIPLY", "shader": reference()}]));
    let doc = import_rest_json(&file).expect("import_rest_json");
    let Effect::Shader { shader, .. } = &layer(&doc).effects[0] else { panic!("expected a shader effect") };
    assert_eq!(shader.raw["blendMode"], json!("MULTIPLY"), "a field the model does not hold stays in the raw message");
    assert_eq!(shader.raw["customEffectId"], reference()["customEffectId"]);
}

#[test]
fn a_shader_imported_from_rest_reaches_the_fig_export() {
    let doc = import_rest_json(&shader_type_file()).expect("import_rest_json");
    let fig = export_fig(&doc).expect("export_fig");
    let decoded = decode_fig_to_json(&fig).expect("decode the exported fig");
    let changes = decoded["message"]["nodeChanges"].as_array().expect("nodeChanges");
    let nc = changes.iter().find(|nc| nc["name"] == "ShaderLayer").expect("ShaderLayer is exported");
    assert_eq!(nc["effects"][0]["type"], json!("CUSTOM"));
    assert_eq!(nc["effects"][0]["customEffectId"], reference()["customEffectId"]);
    assert_eq!(nc["fillPaints"][1]["type"], json!("CUSTOM"));
    assert_eq!(nc["fillPaints"][1]["customEffectId"], reference()["customEffectId"]);
}
