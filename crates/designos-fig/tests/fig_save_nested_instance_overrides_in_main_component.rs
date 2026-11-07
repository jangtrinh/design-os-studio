use std::collections::HashSet;

use designos_doc::*;
use designos_fig::container::{self, Compression};
use designos_fig::kiwi::KiwiValue as V;
use designos_fig::testkit::{en, figma_like_schema, fl, guid, msg, st};
use designos_fig::{decode_fig_to_json, export_fig, import_fig, import_fig_node_changes};
use serde_json::{json, Value};

fn color_v(r: f32, g: f32, b: f32, a: f32) -> V {
    st(vec![("r", fl(r)), ("g", fl(g)), ("b", fl(b)), ("a", fl(a))])
}

fn solid_paint_v(r: f32, g: f32, b: f32) -> V {
    msg(vec![("type", en("SOLID")), ("color", color_v(r, g, b, 1.0)), ("opacity", fl(1.0)), ("visible", V::Bool(true)), ("blendMode", en("NORMAL"))])
}

fn bound_solid_paint_v(r: f32, g: f32, b: f32, var_guid: (u32, u32)) -> V {
    msg(vec![
        ("type", en("SOLID")),
        ("color", color_v(r, g, b, 1.0)),
        ("opacity", fl(1.0)),
        ("visible", V::Bool(true)),
        ("blendMode", en("NORMAL")),
        ("colorVar", msg(vec![("dataType", en("ALIAS")), ("value", msg(vec![("alias", msg(vec![("guid", guid(var_guid.0, var_guid.1))]))]))])),
    ])
}

fn identity_matrix() -> V {
    st(vec![("m00", fl(1.0)), ("m01", fl(0.0)), ("m02", fl(0.0)), ("m10", fl(0.0)), ("m11", fl(1.0)), ("m12", fl(0.0))])
}

fn base_nc(g: (u32, u32), parent: (u32, u32), pos: &str, ty: &str, name: &str, w: f32, h: f32) -> Vec<(&'static str, V)> {
    vec![
        ("guid", guid(g.0, g.1)),
        ("parentIndex", st(vec![("guid", guid(parent.0, parent.1)), ("position", V::String(pos.into()))])),
        ("type", en(ty)),
        ("name", V::String(name.into())),
        ("visible", V::Bool(true)),
        ("opacity", fl(1.0)),
        ("phase", en("CREATED")),
        ("size", st(vec![("x", fl(w)), ("y", fl(h))])),
        ("transform", identity_matrix()),
        ("blendMode", en("PASS_THROUGH")),
    ]
}

fn make_fig_bytes(changes: Vec<V>) -> Vec<u8> {
    let schema = figma_like_schema();
    let root = schema.find("Message").unwrap();
    let body = schema.encode_message(root, &msg(vec![("nodeChanges", V::Array(changes))])).unwrap();
    container::write_container(&schema.encode(), &body, Compression::Zstd, 106)
}

fn jguid(s: u64, l: u64) -> Value {
    json!({"sessionID": s, "localID": l})
}

fn var_alias(s: u64, l: u64, resolved: &str) -> Value {
    json!({"dataType": "ALIAS", "resolvedDataType": resolved, "value": {"alias": {"guid": jguid(s, l)}}})
}

fn fig_node(id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str, extra: Value) -> Value {
    let mut v = json!({
        "guid": jguid(id.0, id.1),
        "parentIndex": {"guid": jguid(parent.0, parent.1), "position": pos},
        "type": ty,
        "name": name,
        "phase": "CREATED",
        "visible": true,
        "opacity": 1.0,
        "size": {"x": 120.0, "y": 48.0},
        "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
    });
    for (k, x) in extra.as_object().expect("extra object") {
        v[k] = x.clone();
    }
    v
}

fn guid_pair(v: &Value) -> (u32, u32) {
    (v.get("sessionID").and_then(Value::as_u64).unwrap_or(0) as u32, v.get("localID").and_then(Value::as_u64).unwrap_or(0) as u32)
}

/// Figma stores an instance without baked child `NodeChange` records so readers
/// reconstruct the instance subtree from its main component and `symbolOverrides`.
fn node_changes_without_instance_descendants(decoded: &Value) -> Value {
    let list = decoded["message"]["nodeChanges"].as_array().expect("nodeChanges array");
    let mut parent_of = std::collections::HashMap::new();
    let mut instances = HashSet::new();
    for nc in list {
        let Some(g) = nc.get("guid").map(guid_pair) else { continue };
        if let Some(p) = nc.get("parentIndex").and_then(|pi| pi.get("guid")).map(guid_pair) {
            parent_of.insert(g, p);
        }
        if nc.get("type").and_then(Value::as_str) == Some("INSTANCE") {
            instances.insert(g);
        }
    }
    let under_instance = |mut g: (u32, u32)| -> bool {
        while let Some(&p) = parent_of.get(&g) {
            if p == g {
                break;
            }
            if instances.contains(&p) {
                return true;
            }
            g = p;
        }
        false
    };
    Value::Array(list.iter().filter(|nc| !nc.get("guid").map(guid_pair).is_some_and(under_instance)).cloned().collect())
}

#[test]
fn fig_round_trip_of_main_component_containing_instance_with_text_and_variable_bound_stroke_overrides_keeps_both() {
    let mut set_f = base_nc((1, 2), (0, 0), "$", "VARIABLE_SET", "Colors", 0.0, 0.0);
    set_f.push((
        "variableSetModes",
        V::Array(vec![msg(vec![("id", guid(1, 100)), ("name", V::String("Light".into())), ("sortPosition", V::String("!".into()))])]),
    ));

    let mut var_f = base_nc((1, 3), (1, 2), "!", "VARIABLE", "fg-quaternary", 0.0, 0.0);
    var_f.push(("variableSetID", msg(vec![("guid", guid(1, 2))])));
    var_f.push(("variableResolvedType", en("COLOR")));
    var_f.push((
        "variableDataValues",
        msg(vec![(
            "entries",
            V::Array(vec![msg(vec![
                ("modeID", guid(1, 100)),
                ("variableData", msg(vec![("dataType", en("COLOR")), ("value", msg(vec![("colorValue", color_v(0.64, 0.66, 0.68, 1.0))]))])),
            ])]),
        )]),
    ));

    // Main component `Buttons/Button` (1:10) with a label `Text` (1:11) and an `Icon` (1:12).
    let comp_button = base_nc((1, 10), (0, 3), "!", "SYMBOL", "Buttons/Button", 130.0, 48.0);
    let mut text_cta = base_nc((1, 11), (1, 10), "!", "TEXT", "Text", 80.0, 20.0);
    text_cta.push(("textData", msg(vec![("characters", V::String("Button CTA".into()))])));
    text_cta.push(("fontSize", fl(14.0)));
    let mut icon_shape = base_nc((1, 12), (1, 10), "\"", "ROUNDED_RECTANGLE", "Icon", 20.0, 20.0);
    icon_shape.push(("strokePaints", V::Array(vec![solid_paint_v(0.0, 0.0, 0.0)])));
    icon_shape.push(("strokeWeight", fl(1.5)));

    // Main component `Type=Simple 03, Breakpoint=Desktop` (1:20) on inactive page 3 (0:3)
    // containing an instance (1:21) of `Buttons/Button` (1:10) with a text override
    // and a variable-bound stroke override.
    let comp_variant = base_nc((1, 20), (0, 3), "\"", "SYMBOL", "Type=Simple 03, Breakpoint=Desktop", 1440.0, 960.0);
    let mut inst_in_variant = base_nc((1, 21), (1, 20), "!", "INSTANCE", "Buttons/Button", 130.0, 48.0);
    inst_in_variant.push((
        "symbolData",
        msg(vec![
            ("symbolID", guid(1, 10)),
            (
                "symbolOverrides",
                V::Array(vec![
                    msg(vec![
                        ("guidPath", msg(vec![("guids", V::Array(vec![guid(1, 11)]))])),
                        ("textData", msg(vec![("characters", V::String("Go back".into()))])),
                    ]),
                    msg(vec![
                        ("guidPath", msg(vec![("guids", V::Array(vec![guid(1, 12)]))])),
                        ("strokePaints", V::Array(vec![bound_solid_paint_v(0.64, 0.66, 0.68, (1, 3))])),
                        ("strokeWeight", fl(1.6666666)),
                    ]),
                ]),
            ),
        ]),
    ));

    let mut changes = vec![
        msg(base_nc((0, 0), (0, 0), "", "DOCUMENT", "Document", 0.0, 0.0)),
        msg(base_nc((0, 1), (0, 0), "!", "CANVAS", "Cover", 0.0, 0.0)),
        msg(base_nc((0, 2), (0, 0), "\"", "CANVAS", "Page 2", 0.0, 0.0)),
        msg(base_nc((0, 3), (0, 0), "#", "CANVAS", "404 pages", 0.0, 0.0)),
        msg(set_f),
        msg(var_f),
        msg(comp_button),
        msg(text_cta),
        msg(icon_shape),
        msg(comp_variant),
        msg(inst_in_variant),
    ];
    for i in 0..25_000u32 {
        changes.push(msg(base_nc((9, i + 1), (0, 1), "!", "ROUNDED_RECTANGLE", "F", 4.0, 4.0)));
    }

    let fig = make_fig_bytes(changes);
    let doc = import_fig(&fig).expect("import multi-page .fig");
    assert!(doc.page_hydrator.is_some(), "fixture must defer inactive page 3 before export_fig");

    let saved = export_fig(&doc).expect("export .fig");
    let mut reopened = import_fig(&saved).expect("reimport saved .fig");
    reopened.hydrate_all_pages();

    let variant_id = reopened
        .nodes
        .values()
        .find(|n| matches!(n.kind, NodeKind::Component(_)) && n.name == "Type=Simple 03, Breakpoint=Desktop")
        .map(|n| n.id)
        .expect("variant main component");
    let button_inst_id = reopened.get(variant_id).unwrap().children[0];
    let button_children = &reopened.get(button_inst_id).unwrap().children;

    let label = button_children
        .iter()
        .filter_map(|&cid| reopened.get(cid))
        .find_map(|n| match &n.kind {
            NodeKind::Text(tp) => Some(tp.content.as_str()),
            _ => None,
        })
        .expect("label inside nested instance");
    assert_eq!(label, "Go back", "text override on instance inside main component must survive .fig save");

    let icon = button_children.iter().filter_map(|&cid| reopened.get(cid)).find(|n| n.name == "Icon").expect("icon inside nested instance");
    assert_eq!(
        icon.variables.bindings.get("strokes/0").map(String::as_str),
        Some("VariableID:1:3"),
        "variable-bound stroke override on instance inside main component must keep its variable binding"
    );
    let PaintKind::Solid(stroke_col) = icon.stroke.paints[0].kind else {
        panic!("expected solid stroke paint");
    };
    assert!((stroke_col.r - 0.64).abs() < 1e-3 && (stroke_col.g - 0.66).abs() < 1e-3 && (stroke_col.b - 0.68).abs() < 1e-3);
}

#[test]
fn fig_round_trip_of_main_component_with_two_levels_of_nested_instances_keeps_text_and_variable_bound_stroke_overrides() {
    let fg_quaternary = json!({"r": 0.64, "g": 0.66, "b": 0.68, "a": 1.0});
    let changes = Value::Array(vec![
        fig_node((0, 0), (0, 0), "", "DOCUMENT", "Document", json!({})),
        fig_node((0, 1), (0, 0), "!", "CANVAS", "404 pages", json!({})),
        fig_node(
            (1, 2),
            (0, 0),
            "\"",
            "VARIABLE_SET",
            "Colors",
            json!({
                "defaultModeID": jguid(1, 100),
                "variableSetModes": [{"id": jguid(1, 100), "name": "Light", "sortPosition": "!"}]
            }),
        ),
        fig_node(
            (1, 3),
            (1, 2),
            "!",
            "VARIABLE",
            "fg-quaternary",
            json!({
                "variableSetID": jguid(1, 2),
                "variableResolvedType": "COLOR",
                "variableDataValues": {
                    "entries": [{
                        "modeID": jguid(1, 100),
                        "variableData": {"dataType": "COLOR", "resolvedDataType": "COLOR", "value": {"colorValue": fg_quaternary}}
                    }]
                }
            }),
        ),
        // Innermost main component `arrow-left` (1:4) with an `Icon` shape (1:5) and a `Label` text (1:6).
        fig_node((1, 4), (0, 1), "!", "SYMBOL", "arrow-left", json!({})),
        fig_node(
            (1, 5),
            (1, 4),
            "!",
            "ROUNDED_RECTANGLE",
            "Icon",
            json!({
                "strokePaints": [{"type": "SOLID", "visible": true, "opacity": 1.0, "color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 1.0}}],
                "strokeWeight": 1.6666666
            }),
        ),
        fig_node(
            (1, 6),
            (1, 4),
            "\"",
            "TEXT",
            "Label",
            json!({
                "textData": {"characters": "Button CTA"},
                "fontSize": 14.0
            }),
        ),
        // Middle main component `Buttons/Button` (1:7) containing an instance `Icon leading` (1:8) of `arrow-left` (1:4).
        fig_node((1, 7), (0, 1), "\"", "SYMBOL", "Buttons/Button", json!({})),
        fig_node((1, 8), (1, 7), "!", "INSTANCE", "Icon leading", json!({"symbolData": {"symbolID": jguid(1, 4)}})),
        // Outer main component `Type=Simple 03, Breakpoint=Desktop` (1:10) containing an instance
        // `Buttons/Button` (1:11) of `1:7`, with two-level override paths `1:8 -> 1:6` (text)
        // and `1:8 -> 1:5` (variable-bound stroke).
        fig_node((1, 10), (0, 1), "#", "SYMBOL", "Type=Simple 03, Breakpoint=Desktop", json!({})),
        fig_node(
            (1, 11),
            (1, 10),
            "!",
            "INSTANCE",
            "Buttons/Button",
            json!({
                "symbolData": {
                    "symbolID": jguid(1, 7),
                    "symbolOverrides": [
                        {
                            "guidPath": {"guids": [jguid(1, 8), jguid(1, 6)]},
                            "textData": {"characters": "Go back"}
                        },
                        {
                            "guidPath": {"guids": [jguid(1, 8), jguid(1, 5)]},
                            "overrideLevel": 1,
                            "strokePaints": [{
                                "type": "SOLID",
                                "visible": true,
                                "opacity": 1.0,
                                "color": fg_quaternary,
                                "colorVar": var_alias(1, 3, "COLOR")
                            }],
                            "strokeWeight": 1.6666666
                        }
                    ]
                }
            }),
        ),
    ]);

    let doc = import_fig_node_changes(&changes, "Document").expect("import initial nodeChanges");
    let saved = export_fig(&doc).expect("export .fig");
    let decoded = decode_fig_to_json(&saved).expect("decode exported .fig");

    // Reopen from the exported nodeChanges without baked instance descendants so the reader
    // must resolve the two-level `guidPath` through both nested instances from `symbolOverrides`.
    let stripped = node_changes_without_instance_descendants(&decoded);
    let reopened = import_fig_node_changes(&stripped, "Reopened").expect("reopen from symbolOverrides");

    let simple03_id = reopened
        .nodes
        .values()
        .find(|n| matches!(n.kind, NodeKind::Component(_)) && n.name == "Type=Simple 03, Breakpoint=Desktop")
        .map(|n| n.id)
        .expect("outer main component");
    let btn_inst_id = reopened.get(simple03_id).unwrap().children[0];
    let inner_icon_inst_id = reopened.get(btn_inst_id).unwrap().children[0];
    let inner_children = &reopened.get(inner_icon_inst_id).unwrap().children;

    let label = inner_children
        .iter()
        .filter_map(|&cid| reopened.get(cid))
        .find_map(|n| match &n.kind {
            NodeKind::Text(tp) => Some(tp.content.as_str()),
            _ => None,
        })
        .expect("two-level nested label");
    assert_eq!(label, "Go back", "two-level nested text override inside main component must survive .fig save");

    let icon = inner_children.iter().filter_map(|&cid| reopened.get(cid)).find(|n| n.name == "Icon").expect("two-level nested icon");
    assert_eq!(
        icon.variables.bindings.get("strokes/0").map(String::as_str),
        Some("VariableID:1:3"),
        "two-level nested stroke override inside main component must keep its variable binding"
    );
    let PaintKind::Solid(stroke_col) = icon.stroke.paints[0].kind else {
        panic!("expected solid stroke paint");
    };
    assert!((stroke_col.r - 0.64).abs() < 1e-3 && (stroke_col.g - 0.66).abs() < 1e-3 && (stroke_col.b - 0.68).abs() < 1e-3);
}
