//! Variable code syntax (`codeSyntax`) and composed opacity bindings (`opacityVar`) through
//! the REST JSON, the kiwi-shaped `.fig` node changes, and an export/import round trip.

use designos_doc::*;
use designos_fig::{export_fig, export_rest_json, export_variables_json, import_fig, import_fig_node_changes, import_rest_json, import_variables_json};
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

const WEB: &str = "var(--brand-primary, #336699)";

fn variables_payload() -> String {
    json!({"status": 200, "error": false, "meta": {
        "variableCollections": {"VariableCollectionId:1:1": {
            "id": "VariableCollectionId:1:1", "name": "Theme", "defaultModeId": "1:0",
            "modes": [{"modeId": "1:0", "name": "Light"}], "variableIds": ["VariableID:1:2", "VariableID:1:3"]}},
        "variables": {
            "VariableID:1:2": {"id": "VariableID:1:2", "name": "Brand/Primary", "variableCollectionId": "VariableCollectionId:1:1",
                "resolvedType": "COLOR", "valuesByMode": {"1:0": {"r": 0.2, "g": 0.4, "b": 0.6, "a": 1.0}}, "scopes": ["ALL_SCOPES"],
                "codeSyntax": {"WEB": WEB, "iOS": "Color.brandPrimary"}},
            "VariableID:1:3": {"id": "VariableID:1:3", "name": "Fade", "variableCollectionId": "VariableCollectionId:1:1",
                "resolvedType": "FLOAT", "valuesByMode": {"1:0": 50.0}, "scopes": ["COLOR_OPACITY"], "codeSyntax": {}}}}})
    .to_string()
}

#[test]
fn rest_code_syntax_is_stored_and_written_back() {
    let mut doc = Document::new("d");
    assert_eq!(import_variables_json(&mut doc, &variables_payload()).unwrap(), 2);
    let brand = &doc.variables["VariableID:1:2"];
    assert_eq!(brand.code_syntax_for(CodeSyntaxPlatform::Web), Some(WEB));
    assert_eq!(brand.code_syntax_for(CodeSyntaxPlatform::Ios), Some("Color.brandPrimary"));
    assert_eq!(brand.code_syntax_for(CodeSyntaxPlatform::Android), None);
    assert!(doc.variables["VariableID:1:3"].code_syntax.is_empty());

    let out = export_variables_json(&doc);
    assert_eq!(out["meta"]["variables"]["VariableID:1:2"]["codeSyntax"], json!({"WEB": WEB, "iOS": "Color.brandPrimary"}));
    assert_eq!(out["meta"]["variables"]["VariableID:1:3"]["codeSyntax"], json!({}));
    let mut back = Document::new("b");
    import_variables_json(&mut back, &out.to_string()).unwrap();
    assert_eq!(back.variables, doc.variables);
}

#[test]
fn native_json_round_trip_keeps_code_syntax_with_figma_labels() {
    let mut doc = Document::new("d");
    import_variables_json(&mut doc, &variables_payload()).unwrap();
    let json = doc.to_json().unwrap();
    assert!(json.contains(r#""code_syntax":{"WEB":"var(--brand-primary, #336699)","iOS":"Color.brandPrimary"}"#), "{json}");
    let back = Document::from_json(&json).unwrap();
    assert_eq!(back.variables, doc.variables);
    // old files without the field still load
    assert!(back.variables["VariableID:1:3"].code_syntax.is_empty());
}

fn g(s: u64, l: u64) -> Value {
    json!({"sessionID": s, "localID": l})
}

fn alias(s: u64, l: u64) -> Value {
    json!({"dataType": "ALIAS", "resolvedDataType": "COLOR", "value": {"alias": {"guid": g(s, l)}}})
}

fn node(id: (u64, u64), parent: (u64, u64), ty: &str, name: &str, extra: Value) -> Value {
    let mut v = json!({"guid": g(id.0, id.1), "parentIndex": {"guid": g(parent.0, parent.1), "position": "!"}, "type": ty, "name": name,
        "phase": "CREATED", "visible": true, "opacity": 1.0, "size": {"x": 100.0, "y": 50.0},
        "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}});
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    v
}

fn figma_shaped_changes() -> Value {
    let color = json!({"r": 1.0, "g": 1.0, "b": 1.0, "a": 1.0});
    json!([
        node((0, 0), (0, 0), "DOCUMENT", "Document", json!({})),
        node((0, 1), (0, 0), "CANVAS", "Page 1", json!({})),
        node(
            (1, 5),
            (0, 1),
            "VARIABLE_SET",
            "Theme",
            json!({
            "variableSetModes": [{"id": g(1, 6), "name": "Light", "sortPosition": "!"}]})
        ),
        node(
            (1, 9),
            (0, 1),
            "VARIABLE",
            "Brand/Primary",
            json!({
            "variableSetID": {"guid": g(1, 5)}, "variableResolvedType": "COLOR",
            "variableDataValues": {"entries": [{"modeID": g(1, 6), "variableData": {"dataType": "COLOR", "value": {"colorValue": color}}}]},
            "codeSyntax": {"entries": [{"platform": "WEB", "value": "var(--brand)"}, {"platform": "ANDROID", "value": "R.color.brand"}, {"platform": "iOS", "value": "Color.brand"}]}})
        ),
        node(
            (1, 10),
            (0, 1),
            "VARIABLE",
            "Fade",
            json!({
            "variableSetID": {"guid": g(1, 5)}, "variableResolvedType": "FLOAT", "variableScopes": ["COLOR_OPACITY"],
            "variableDataValues": {"entries": [{"modeID": g(1, 6), "variableData": {"dataType": "FLOAT", "value": {"floatValue": 40.0}}}]}})
        ),
        node(
            (1, 20),
            (0, 1),
            "FRAME",
            "Card",
            json!({
            "fillPaints": [{"type": "SOLID", "color": color, "opacity": 0.5, "visible": true, "blendMode": "NORMAL",
                "colorVar": alias(1, 9), "opacityVar": alias(1, 10)}],
            "strokePaints": [{"type": "SOLID", "color": color, "opacity": 1.0, "visible": true, "blendMode": "NORMAL", "opacityVar": alias(1, 10)}],
            "strokeWeight": 1.0,
            "effects": [{"type": "DROP_SHADOW", "color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 0.5}, "offset": {"x": 0.0, "y": 2.0},
                "radius": 4.0, "visible": true, "effectOpacityVar": alias(1, 10)}]})
        ),
    ])
}

#[test]
fn fig_import_reads_code_syntax_and_opacity_bindings() {
    let doc = import_fig_node_changes(&figma_shaped_changes(), "d").unwrap();
    let brand = doc.variables.values().find(|v| v.name == "Brand/Primary").unwrap();
    assert_eq!(brand.code_syntax_for(CodeSyntaxPlatform::Web), Some("var(--brand)"));
    assert_eq!(brand.code_syntax_for(CodeSyntaxPlatform::Android), Some("R.color.brand"));
    assert_eq!(brand.code_syntax_for(CodeSyntaxPlatform::Ios), Some("Color.brand"));
    let fade = doc.variables.values().find(|v| v.name == "Fade").unwrap();
    assert!(fade.code_syntax.is_empty());
    assert_eq!(fade.scopes, ["COLOR_OPACITY"]);

    let card = find(&doc, "Card");
    let b = &card.variables.bindings;
    assert_eq!(b["fills/0"], brand.id);
    assert_eq!(b["fills/0/opacity"], fade.id);
    assert_eq!(b["strokes/0/opacity"], fade.id);
    assert_eq!(b["effects/0/opacity"], fade.id);
    // raw values stay; the bound variable IS the opacity (40 %), it does not multiply the 0.5 / 1.0 / 0.5 raw ones
    assert_eq!(card.fills[0].opacity, 0.5);
    let r = resolved_node(&doc, card.id).unwrap();
    assert!((r.fills[0].opacity - 0.4).abs() < 1e-6);
    assert!((r.stroke.paints[0].opacity - 0.4).abs() < 1e-6);
    let Effect::DropShadow { color, .. } = &r.effects[0] else { panic!() };
    assert!((color.a - 0.4).abs() < 1e-6);
}

#[test]
fn fig_export_then_import_keeps_code_syntax_and_opacity_bindings() {
    let doc = import_fig_node_changes(&figma_shaped_changes(), "d").unwrap();
    let bytes = export_fig(&doc).expect("export_fig");
    let back = import_fig(&bytes).expect("import_fig");
    let var = |d: &Document, name: &str| d.variables.values().find(|v| v.name == name).unwrap().clone();
    let expected: BTreeMap<CodeSyntaxPlatform, String> = [
        (CodeSyntaxPlatform::Web, "var(--brand)".to_string()),
        (CodeSyntaxPlatform::Android, "R.color.brand".to_string()),
        (CodeSyntaxPlatform::Ios, "Color.brand".to_string()),
    ]
    .into_iter()
    .collect();
    assert_eq!(var(&back, "Brand/Primary").code_syntax, expected);
    assert!(var(&back, "Fade").code_syntax.is_empty());
    let (a, b) = (find(&doc, "Card"), find(&back, "Card"));
    for key in ["fills/0", "fills/0/opacity", "strokes/0/opacity", "effects/0/opacity"] {
        assert_eq!(doc.variables[&a.variables.bindings[key]].name, back.variables[&b.variables.bindings[key]].name, "{key}");
    }
    assert_eq!(b.variables.bindings.len(), a.variables.bindings.len());
    assert_eq!(b.fills[0].opacity, 0.5);
}

#[test]
fn rest_bound_variables_carry_opacity_bindings_both_ways() {
    let doc = import_fig_node_changes(&figma_shaped_changes(), "d").unwrap();
    let card = find(&doc, "Card");
    let json = export_rest_json(&doc);
    let page = &json["document"]["children"][0]["children"];
    let card_json = page.as_array().unwrap().iter().find(|c| c["name"] == "Card").unwrap();
    let bv = &card_json["boundVariables"];
    assert_eq!(bv["fills"][0]["id"], card.variables.bindings["fills/0"].as_str());
    assert_eq!(bv["fillOpacities"][0]["id"], card.variables.bindings["fills/0/opacity"].as_str());
    assert_eq!(bv["strokeOpacities"][0]["id"], card.variables.bindings["strokes/0/opacity"].as_str());
    assert_eq!(bv["effects"][0]["opacity"]["id"], card.variables.bindings["effects/0/opacity"].as_str());
    let back = import_rest_json(&json.to_string()).unwrap();
    assert_eq!(find(&back, "Card").variables, card.variables);
}
