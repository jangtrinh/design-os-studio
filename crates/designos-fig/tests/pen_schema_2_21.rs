//! Comprehensive integration tests for `.pen` JSON scene format schema `2.21` and the
//! `1.0 -> 2.8 -> 2.13 -> 2.17 -> 2.21` version-migration chain (`#570`).

use designos_doc::*;
use designos_fig::{export_pen_json, import_pen_json, import_pen_json_report, PenImportOptions};
use serde_json::{json, Value};
use std::path::PathBuf;

fn fixture_str(name: &str) -> String {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests");
    p.push("fixtures");
    p.push("pen");
    p.push(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", p.display()))
}

fn find_by_source_id<'a>(doc: &'a Document, sid: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.source_id.as_deref() == Some(sid)).unwrap_or_else(|| panic!("no node with source_id '{sid}'"))
}

fn assert_nodes_pen_field_equal(a: &Document, na_id: NodeId, b: &Document, nb_id: NodeId, ctx: &str) {
    let na = a.get(na_id).expect("node a");
    let nb = b.get(nb_id).expect("node b");
    let path = format!("{ctx}/{:?}", na.source_id.as_deref().unwrap_or(&na.name));
    assert_eq!(na.source_id, nb.source_id, "{path}: source_id");
    assert_eq!(na.name, nb.name, "{path}: name");
    assert_eq!(na.visible, nb.visible, "{path}: visible");
    assert!((na.opacity - nb.opacity).abs() < 1e-4, "{path}: opacity {} vs {}", na.opacity, nb.opacity);
    assert_eq!(na.blend, nb.blend, "{path}: blend");
    assert_eq!(na.hyperlink, nb.hyperlink, "{path}: hyperlink");
    assert!((na.size.x - nb.size.x).abs() < 1e-2 && (na.size.y - nb.size.y).abs() < 1e-2, "{path}: size {:?} vs {:?}", na.size, nb.size);
    assert!(
        (na.transform.a - nb.transform.a).abs() < 1e-3
            && (na.transform.b - nb.transform.b).abs() < 1e-3
            && (na.transform.c - nb.transform.c).abs() < 1e-3
            && (na.transform.d - nb.transform.d).abs() < 1e-3
            && (na.transform.tx - nb.transform.tx).abs() < 1e-3
            && (na.transform.ty - nb.transform.ty).abs() < 1e-3,
        "{path}: transform {:?} vs {:?}",
        na.transform,
        nb.transform
    );
    assert_eq!(na.layout_child, nb.layout_child, "{path}: layout_child");
    assert_eq!(na.fills, nb.fills, "{path}: fills");
    assert_eq!(na.stroke, nb.stroke, "{path}: stroke");
    assert_eq!(na.corner_radii, nb.corner_radii, "{path}: corner_radii");
    assert_eq!(na.effects, nb.effects, "{path}: effects");
    assert_eq!(na.variables, nb.variables, "{path}: variables");
    assert_eq!(na.component.overrides, nb.component.overrides, "{path}: component.overrides");
    assert_eq!(na.component.is_slot, nb.component.is_slot, "{path}: component.is_slot");
    assert_eq!(na.component.is_slot_content, nb.component.is_slot_content, "{path}: component.is_slot_content");
    assert_eq!(na.component.props.len(), nb.component.props.len(), "{path}: component.props.len()");
    match (&na.kind, &nb.kind) {
        (NodeKind::Frame(fa), NodeKind::Frame(fb))
        | (NodeKind::Component(fa), NodeKind::Component(fb))
        | (NodeKind::ComponentSet(fa), NodeKind::ComponentSet(fb)) => {
            assert_eq!(fa, fb, "{path}: frame_props");
        }
        (NodeKind::Instance { frame: fa, component: ca, component_key: ka }, NodeKind::Instance { frame: fb, component: cb, component_key: kb }) => {
            assert_eq!(fa, fb, "{path}: instance frame_props");
            assert_eq!(ka, kb, "{path}: instance component_key");
            let ca_sid = ca.and_then(|id| a.get(id)).and_then(|n| n.source_id.clone());
            let cb_sid = cb.and_then(|id| b.get(id)).and_then(|n| n.source_id.clone());
            assert_eq!(ca_sid, cb_sid, "{path}: instance target component source_id");
        }
        (NodeKind::Text(ta), NodeKind::Text(tb)) => {
            assert_eq!(ta.content, tb.content, "{path}: text content");
            assert_eq!(ta.style, tb.style, "{path}: text style");
            assert_eq!(ta.runs, tb.runs, "{path}: text runs");
        }
        (NodeKind::Ellipse(ea), NodeKind::Ellipse(eb)) => {
            assert!(
                (ea.start - eb.start).abs() < 1e-3 && (ea.end - eb.end).abs() < 1e-3 && (ea.inner_radius - eb.inner_radius).abs() < 1e-3,
                "{path}: ellipse arc"
            );
        }
        (NodeKind::Vector { paths: pa, .. }, NodeKind::Vector { paths: pb, .. }) => {
            assert_eq!(pa, pb, "{path}: vector paths");
        }
        (NodeKind::Connector(ca), NodeKind::Connector(cb)) => {
            let sa_sid = ca.start.node.and_then(|id| a.get(id)).and_then(|n| n.source_id.clone());
            let sb_sid = cb.start.node.and_then(|id| b.get(id)).and_then(|n| n.source_id.clone());
            let ea_sid = ca.end.node.and_then(|id| a.get(id)).and_then(|n| n.source_id.clone());
            let eb_sid = cb.end.node.and_then(|id| b.get(id)).and_then(|n| n.source_id.clone());
            assert_eq!((sa_sid, ca.start.magnet, ca.start.point), (sb_sid, cb.start.magnet, cb.start.point), "{path}: connector start");
            assert_eq!((ea_sid, ca.end.magnet, ca.end.point), (eb_sid, cb.end.magnet, cb.end.point), "{path}: connector end");
            assert_eq!(ca.label.as_ref().map(|l| &l.content), cb.label.as_ref().map(|l| &l.content), "{path}: connector label");
        }
        (ka, kb) => assert_eq!(ka, kb, "{path}: kind"),
    }
    assert_eq!(na.children.len(), nb.children.len(), "{path}: children count");
    for (&ca, &cb) in na.children.iter().zip(nb.children.iter()) {
        assert_nodes_pen_field_equal(a, ca, b, cb, &path);
    }
}

fn assert_documents_pen_field_equal(a: &Document, b: &Document, ctx: &str) {
    assert_eq!(a.name, b.name, "{ctx}: doc.name");
    assert_eq!(a.variable_collections, b.variable_collections, "{ctx}: variable_collections");
    assert_eq!(a.variables, b.variables, "{ctx}: variables");
    assert_eq!(a.pages.len(), b.pages.len(), "{ctx}: pages.len()");
    for (idx, (&pa_id, &pb_id)) in a.pages.iter().zip(b.pages.iter()).enumerate() {
        let pa = a.get(pa_id).expect("page a");
        let pb = b.get(pb_id).expect("page b");
        assert_eq!(pa.name, pb.name, "{ctx}: page[{idx}].name");
        assert_eq!(pa.kind, pb.kind, "{ctx}: page[{idx}].kind");
        assert_eq!(pa.children.len(), pb.children.len(), "{ctx}: page[{idx}].children.len()");
        for (&ca, &cb) in pa.children.iter().zip(pb.children.iter()) {
            assert_nodes_pen_field_equal(a, ca, b, cb, &format!("{ctx}/page[{idx}]"));
        }
    }
}

// 1. migrates_1_0_through_2_21_fixtures
#[test]
fn migrates_1_0_through_2_21_fixtures() {
    for (file, expected_ver) in [("v1_0.pen", "1.0"), ("v2_8.pen", "2.8"), ("v2_13.pen", "2.13"), ("v2_17.pen", "2.17"), ("v2_21.pen", "2.21")] {
        let raw = fixture_str(file);
        let report = import_pen_json_report(&raw, &PenImportOptions::default()).unwrap_or_else(|e| panic!("failed to import {file}: {e}"));
        assert_eq!(report.source_version, expected_ver, "fixture {file}");

        let exported = export_pen_json(&report.doc).expect("export_pen_json");
        let exported_val: Value = serde_json::from_str(&exported).expect("valid exported JSON");
        assert_eq!(exported_val.get("version").and_then(Value::as_str), Some("2.21"), "exported version for {file} must be 2.21");
        validate_pen_2_21_document_shape(&exported_val, file).unwrap_or_else(|e| panic!("exported {file} failed 2.21 schema shape validation: {e}"));

        let reimported = import_pen_json_report(&exported, &PenImportOptions::default()).unwrap_or_else(|e| panic!("failed to reimport {file}: {e}"));
        assert_eq!(reimported.source_version, "2.21");
        assert_documents_pen_field_equal(&report.doc, &reimported.doc, file);
    }

    // Check specific migration outcomes on v1_0, v2_8, v2_13, v2_17:
    let d1 = import_pen_json(&fixture_str("v1_0.pen")).unwrap();
    let card1 = find_by_source_id(&d1, "card-1");
    assert_eq!(card1.corner_radii, [8.0, 8.0, 8.0, 8.0]);
    assert_eq!(card1.kind.frame_props().unwrap().auto_layout.as_ref().unwrap().padding, [12.0, 20.0, 12.0, 20.0]);
    assert_eq!(card1.stroke.weight, 2.0);

    let d28 = import_pen_json(&fixture_str("v2_8.pen")).unwrap();
    let comp28 = find_by_source_id(&d28, "btn-comp");
    assert!(matches!(comp28.kind, NodeKind::Component(_)));
    let inst28 = find_by_source_id(&d28, "btn-inst");
    assert!(matches!(inst28.kind, NodeKind::Instance { component: Some(cid), .. } if cid == comp28.id));

    let d213 = import_pen_json(&fixture_str("v2_13.pen")).unwrap();
    let hero = find_by_source_id(&d213, "hero");
    assert!(matches!(hero.fills.first().map(|p| &p.kind), Some(PaintKind::LinearGradient { .. })));
    let spark = find_by_source_id(&d213, "spark-svg");
    assert!(matches!(spark.kind, NodeKind::Vector { .. }));

    let d217 = import_pen_json(&fixture_str("v2_17.pen")).unwrap();
    let slot217 = find_by_source_id(&d217, "body-slot");
    assert!(slot217.component.is_slot);
    assert_eq!(slot217.plugin_data.get("pen:placeholder").map(String::as_str), Some("true"));
    let badge217 = find_by_source_id(&d217, "badge-abs");
    assert!(badge217.layout_child.absolute);
    assert!(badge217.transform.a < 0.0);
}

// 2. rejects_unsupported_future_version_2_22
#[test]
fn rejects_unsupported_future_version_2_22() {
    let raw = r#"{"version": "2.22", "children": []}"#;
    let err = import_pen_json_report(raw, &PenImportOptions::default()).unwrap_err();
    assert!(err.contains("2.22"), "error should mention seen version 2.22: {err}");
    assert!(err.contains("2.21"), "error should mention max supported version 2.21: {err}");

    let raw_3_0 = r#"{"version": "3.0", "children": []}"#;
    let err_3_0 = import_pen_json_report(raw_3_0, &PenImportOptions::default()).unwrap_err();
    assert!(err_3_0.contains("3.0") && err_3_0.contains("2.21"), "{err_3_0}");
}

// 3. defaults_missing_version_to_1_0_and_migrates_to_2_21
#[test]
fn defaults_missing_version_to_1_0_and_migrates_to_2_21() {
    let raw = r##"{
        "name": "Versionless Scene",
        "children": [
            {
                "id": "legacy-box",
                "type": "frame",
                "padding": { "top": 4, "right": 8, "bottom": 4, "left": 8 },
                "fillColor": "#FF0000"
            }
        ]
    }"##;
    let report = import_pen_json_report(raw, &PenImportOptions::default()).expect("missing version defaults to 1.0");
    assert_eq!(report.source_version, "1.0");
    let box_node = find_by_source_id(&report.doc, "legacy-box");
    assert_eq!(box_node.kind.frame_props().unwrap().auto_layout.as_ref().unwrap().padding, [4.0, 8.0, 4.0, 8.0]);
    let exported: Value = serde_json::from_str(&export_pen_json(&report.doc).unwrap()).unwrap();
    assert_eq!(exported.get("version").and_then(Value::as_str), Some("2.21"));
}

// 4. themes_and_variables_multi_axis_and_order
#[test]
fn themes_and_variables_multi_axis_and_order() {
    let raw = r##"{
        "version": "2.21",
        "themes": {
            "ZetaTheme": ["Light", "Dark", "HighContrast"],
            "AlphaDensity": ["Cozy", "Compact"]
        },
        "variables": {
            "z-first": {
                "type": "color",
                "value": [
                    { "theme": { "ZetaTheme": "Light" }, "value": "#FFFFFF" },
                    { "theme": { "ZetaTheme": "Dark" }, "value": "#000000" },
                    { "theme": { "ZetaTheme": "HighContrast" }, "value": "#FFFF00" }
                ]
            },
            "a-second": {
                "type": "color",
                "value": "#3B82F6"
            },
            "m-third": {
                "type": "number",
                "value": [
                    { "theme": { "AlphaDensity": "Cozy" }, "value": 16 },
                    { "theme": { "AlphaDensity": "Compact" }, "value": 8 }
                ]
            }
        },
        "children": []
    }"##;

    let doc = import_pen_json(raw).expect("import multi-axis themes and variables");
    let collections: Vec<&VariableCollection> = doc.variable_collections.values().collect();
    assert_eq!(collections.len(), 2, "two axes in themes must produce two VariableCollections");
    assert_eq!(collections[0].name, "ZetaTheme", "first collection matches first key in themes");
    assert_eq!(collections[1].name, "AlphaDensity", "second collection matches second key in themes");

    let zeta_modes: Vec<&str> = collections[0].modes.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(zeta_modes, vec!["Light", "Dark", "HighContrast"]);
    assert_eq!(collections[0].default_mode, "ZetaTheme:Light");

    let alpha_modes: Vec<&str> = collections[1].modes.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(alpha_modes, vec!["Cozy", "Compact"]);
    assert_eq!(collections[1].default_mode, "AlphaDensity:Cozy");

    // `z-first` and `a-second` belong to the first collection in file order (`z-first` before `a-second`, NOT alphabetical!).
    assert_eq!(collections[0].variables, vec!["z-first".to_string(), "a-second".to_string()]);
    assert_eq!(collections[1].variables, vec!["m-third".to_string()]);
}

// 5. variable_bindings_on_text_and_font_family_warning
#[test]
fn variable_bindings_on_text_and_font_family_warning() {
    let raw = r##"{
        "version": "2.21",
        "variables": {
            "font-sans": { "type": "string", "value": "Space Grotesk" },
            "size-lg": { "type": "number", "value": 24 },
            "weight-bold": { "type": "number", "value": 700 },
            "lh-tight": { "type": "number", "value": 28 },
            "ls-wide": { "type": "number", "value": 1.5 }
        },
        "children": [
            {
                "id": "txt-bound",
                "type": "text",
                "content": "Headline",
                "fontFamily": "$font-sans",
                "fontSize": "$size-lg",
                "fontWeight": "$weight-bold",
                "lineHeight": "$lh-tight",
                "letterSpacing": "$ls-wide"
            }
        ]
    }"##;

    let report = import_pen_json_report(raw, &PenImportOptions::default()).expect("import text variable bindings");
    assert!(
        report.warnings.iter().any(|w| w.code == "unsupported_font_family_variable" && w.node_id.as_deref() == Some("txt-bound")),
        "expected unsupported_font_family_variable warning, got {:?}",
        report.warnings
    );

    let txt = find_by_source_id(&report.doc, "txt-bound");
    let tp = txt.kind.any_text().unwrap();
    assert_eq!(tp.style.font_family, "Space Grotesk");
    assert_eq!(tp.style.font_size, 24.0);
    assert_eq!(tp.style.font_weight, 700);
    assert_eq!(tp.style.line_height, LineHeight::Pixels(28.0));
    assert_eq!(tp.style.letter_spacing, 1.5);

    assert_eq!(txt.variables.bindings.get("fontSize").map(String::as_str), Some("size-lg"));
    assert_eq!(txt.variables.bindings.get("fontWeight").map(String::as_str), Some("weight-bold"));
    assert_eq!(txt.variables.bindings.get("lineHeight").map(String::as_str), Some("lh-tight"));
    assert_eq!(txt.variables.bindings.get("letterSpacing").map(String::as_str), Some("ls-wide"));
    assert!(txt.plugin_data.get("pen:bindings").unwrap().contains("$font-sans"));

    // Round-trip preserves the variable references in exported JSON.
    let exported: Value = serde_json::from_str(&export_pen_json(&report.doc).unwrap()).unwrap();
    let exp_txt = &exported["children"][0];
    assert_eq!(exp_txt["fontFamily"].as_str(), Some("$font-sans"));
    assert_eq!(exp_txt["fontSize"].as_str(), Some("$size-lg"));
    assert_eq!(exp_txt["fontWeight"].as_str(), Some("$weight-bold"));
    assert_eq!(exp_txt["lineHeight"].as_str(), Some("$lh-tight"));
    assert_eq!(exp_txt["letterSpacing"].as_str(), Some("$ls-wide"));
}

// 6. instance_inherits_unset_component_properties_and_overrides_explicit_ones
#[test]
fn instance_inherits_unset_component_properties_and_overrides_explicit_ones() {
    let raw = r##"{
        "version": "2.21",
        "children": [
            {
                "id": "card-comp",
                "type": "frame",
                "reusable": true,
                "name": "Card",
                "width": 260,
                "height": 140,
                "fill": "#1E293B",
                "stroke": { "fill": "#38BDF8", "thickness": 2 },
                "cornerRadius": 16
            },
            {
                "id": "inst-inherit",
                "type": "ref",
                "ref": "card-comp",
                "name": "Inherited Card"
            },
            {
                "id": "inst-override",
                "type": "ref",
                "ref": "card-comp",
                "name": "Overridden Card",
                "width": 320,
                "fill": "#EF4444",
                "cornerRadius": 4
            }
        ]
    }"##;

    let doc = import_pen_json(raw).expect("import instance inheritance");
    let inh = find_by_source_id(&doc, "inst-inherit");
    assert_eq!(inh.size, Vec2::new(260.0, 140.0), "unset width/height must inherit from component");
    assert_eq!(inh.fills, vec![Paint::solid(Color::from_hex("#1E293B").unwrap())]);
    assert_eq!(inh.stroke.weight, 2.0);
    assert_eq!(inh.corner_radii, [16.0; 4]);
    assert!(!inh.component.overrides.contains(&OverrideField::Size));
    assert!(!inh.component.overrides.contains(&OverrideField::Fills));
    assert!(!inh.component.overrides.contains(&OverrideField::Stroke));
    assert!(!inh.component.overrides.contains(&OverrideField::CornerRadii));

    let ov = find_by_source_id(&doc, "inst-override");
    assert_eq!(ov.size, Vec2::new(320.0, 140.0), "explicit width overrides, unset height inherits");
    assert_eq!(ov.fills, vec![Paint::solid(Color::from_hex("#EF4444").unwrap())]);
    assert_eq!(ov.corner_radii, [4.0; 4]);
    assert!(ov.component.overrides.contains(&OverrideField::Size));
    assert!(ov.component.overrides.contains(&OverrideField::Fills));
    assert!(ov.component.overrides.contains(&OverrideField::CornerRadii));
    assert!(!ov.component.overrides.contains(&OverrideField::Stroke));
}

// 7. slot_declaration_and_instance_children_replacement
#[test]
fn slot_declaration_and_instance_children_replacement() {
    let raw = r##"{
        "version": "2.21",
        "children": [
            {
                "id": "pref-chip",
                "type": "frame",
                "reusable": true,
                "name": "PreferredChip",
                "width": 60,
                "height": 24,
                "fill": "#E2E8F0"
            },
            {
                "id": "dialog-comp",
                "type": "frame",
                "reusable": true,
                "name": "Dialog",
                "width": 300,
                "height": 200,
                "layout": "vertical",
                "children": [
                    {
                        "id": "dialog-body",
                        "type": "frame",
                        "name": "BodySlot",
                        "slot": ["pref-chip"],
                        "children": [
                            {
                                "id": "default-placeholder",
                                "type": "text",
                                "content": "Default Body"
                            }
                        ]
                    }
                ]
            },
            {
                "id": "dialog-inst",
                "type": "ref",
                "ref": "dialog-comp",
                "name": "Custom Dialog",
                "descendants": {
                    "dialog-body": {
                        "children": [
                            {
                                "id": "custom-row-1",
                                "type": "text",
                                "name": "Row 1",
                                "content": "Custom Row A"
                            },
                            {
                                "id": "custom-row-2",
                                "type": "text",
                                "name": "Row 2",
                                "content": "Custom Row B"
                            }
                        ]
                    }
                }
            }
        ]
    }"##;

    let mut doc = import_pen_json(raw).expect("import slot replacement");
    let chip_id = find_by_source_id(&doc, "pref-chip").id;
    let comp = find_by_source_id(&doc, "dialog-comp");
    let comp_id = comp.id;
    assert!(comp.component.props.iter().any(|p| p.name == "BodySlot" && p.kind == PropKind::Slot));
    assert_eq!(comp.component.slot_configs.get("BodySlot").map(|c| c.preferred.clone()), Some(vec![chip_id]));

    let inst = find_by_source_id(&doc, "dialog-inst");
    let inst_slot_id = inst.children[0];
    let inst_slot = doc.get(inst_slot_id).unwrap();
    assert!(inst_slot.component.overrides.contains(&OverrideField::SlotContent));
    assert_eq!(inst_slot.children.len(), 2);
    for &cid in &inst_slot.children {
        assert!(doc.get(cid).unwrap().component.is_slot_content);
    }

    // Mutate main component and sync instances: custom slot children must survive!
    doc.get_mut(comp_id).unwrap().size.x = 360.0;
    sync_instances(&mut doc, comp_id).apply(&mut doc).expect("sync_instances");

    let inst_slot_after = doc.get(inst_slot_id).unwrap();
    assert_eq!(inst_slot_after.children.len(), 2, "custom slot children must survive sync_instances");
    let contents: Vec<&str> = inst_slot_after.children.iter().map(|&cid| doc.get(cid).unwrap().kind.any_text().unwrap().content.as_str()).collect();
    assert_eq!(contents, vec!["Custom Row A", "Custom Row B"]);
}

// 8. descendants_overrides_stroke_padding_font_and_icon
#[test]
fn descendants_overrides_stroke_padding_font_and_icon() {
    let raw = r##"{
        "version": "2.21",
        "children": [
            {
                "id": "item-comp",
                "type": "frame",
                "reusable": true,
                "width": 200,
                "height": 60,
                "children": [
                    {
                        "id": "item-box",
                        "type": "frame",
                        "width": 180,
                        "height": 40,
                        "padding": 4,
                        "stroke": { "fill": "#000000", "thickness": 1 }
                    },
                    {
                        "id": "item-label",
                        "type": "text",
                        "content": "Base",
                        "fontFamily": "Inter",
                        "fontSize": 12,
                        "fontWeight": 400
                    },
                    {
                        "id": "item-icon",
                        "type": "icon_font",
                        "iconFontFamily": "lucide",
                        "iconFontName": "search",
                        "width": 16,
                        "height": 16
                    }
                ]
            },
            {
                "id": "item-inst",
                "type": "ref",
                "ref": "item-comp",
                "descendants": {
                    "item-box": {
                        "padding": [8, 16, 8, 16],
                        "stroke": { "fill": "#FF0000", "thickness": 3 }
                    },
                    "item-label": {
                        "content": "Overridden",
                        "fontFamily": "Roboto",
                        "fontSize": 18,
                        "fontWeight": 700,
                        "fontStyle": "italic"
                    },
                    "item-icon": {
                        "iconFontName": "check"
                    }
                }
            }
        ]
    }"##;

    let doc = import_pen_json(raw).expect("import descendant overrides");
    let inst = find_by_source_id(&doc, "item-inst");
    let box_node = doc.get(inst.children[0]).unwrap();
    assert_eq!(box_node.stroke.weight, 3.0);
    assert_eq!(box_node.kind.frame_props().unwrap().auto_layout.as_ref().unwrap().padding, [8.0, 16.0, 8.0, 16.0]);

    let label_node = doc.get(inst.children[1]).unwrap();
    let tp = label_node.kind.any_text().unwrap();
    assert_eq!(tp.content, "Overridden");
    assert_eq!(tp.style.font_family, "Roboto");
    assert_eq!(tp.style.font_size, 18.0);
    assert_eq!(tp.style.font_weight, 700);
    assert!(tp.style.italic);

    let icon_node = doc.get(inst.children[2]).unwrap();
    assert!(icon_node.plugin_data.get("pen:icon").unwrap().contains("\"check\""));

    // Round-trip through export -> import
    let exported = export_pen_json(&doc).unwrap();
    let rt = import_pen_json(&exported).unwrap();
    let rt_inst = find_by_source_id(&rt, "item-inst");
    let rt_label = rt.get(rt_inst.children[1]).unwrap().kind.any_text().unwrap();
    assert_eq!(rt_label.style.font_family, "Roboto");
    assert_eq!(rt_label.style.font_size, 18.0);
    assert!(rt.get(rt_inst.children[2]).unwrap().plugin_data.get("pen:icon").unwrap().contains("\"check\""));
}

// 9. frame_defaults_horizontal_and_fit_content_vs_layout_none
#[test]
fn frame_defaults_horizontal_and_fit_content_vs_layout_none() {
    let raw = r##"{
        "version": "2.21",
        "children": [
            {
                "id": "default-frame",
                "type": "frame"
            },
            {
                "id": "freeform-frame",
                "type": "frame",
                "layout": "none",
                "width": 300,
                "height": 200
            }
        ]
    }"##;

    let doc = import_pen_json(raw).expect("import frame defaults");
    let def_f = find_by_source_id(&doc, "default-frame");
    let al = def_f.kind.frame_props().unwrap().auto_layout.as_ref().expect("frame without layout key must default to horizontal AutoLayout");
    assert_eq!(al.direction, LayoutDirection::Horizontal);
    assert_eq!(def_f.layout_child.horizontal, Sizing::Hug);
    assert_eq!(def_f.layout_child.vertical, Sizing::Hug);
    assert_eq!(al.primary_sizing, Sizing::Hug);
    assert_eq!(al.counter_sizing, Sizing::Hug);

    let free_f = find_by_source_id(&doc, "freeform-frame");
    assert!(free_f.kind.frame_props().unwrap().auto_layout.is_none(), "frame with layout: 'none' must not have auto_layout");
    assert_eq!(free_f.layout_child.horizontal, Sizing::Fixed);
    assert_eq!(free_f.layout_child.vertical, Sizing::Fixed);
    assert_eq!(free_f.size, Vec2::new(300.0, 200.0));
}

// 10. reusable_non_frame_nodes_and_placeholder_roundtrip
#[test]
fn reusable_non_frame_nodes_and_placeholder_roundtrip() {
    let raw = r##"{
        "version": "2.21",
        "children": [
            {
                "id": "pill-text",
                "type": "text",
                "reusable": true,
                "name": "ReusablePillText",
                "content": "Reusable Pill",
                "fontSize": 14,
                "fontWeight": 600,
                "fill": "#10B981"
            },
            {
                "id": "pill-inst",
                "type": "ref",
                "ref": "pill-text",
                "name": "PillCopy",
                "x": 120,
                "y": 20
            },
            {
                "id": "ph-frame",
                "type": "frame",
                "placeholder": true,
                "width": 100,
                "height": 50
            }
        ]
    }"##;

    let doc = import_pen_json(raw).expect("import reusable non-frame and placeholder");
    let comp = find_by_source_id(&doc, "pill-text");
    assert!(matches!(comp.kind, NodeKind::Component(_)));
    let inst = find_by_source_id(&doc, "pill-inst");
    assert!(matches!(inst.kind, NodeKind::Instance { component: Some(cid), .. } if cid == comp.id));
    assert_eq!(inst.children.len(), 1);
    assert_eq!(doc.get(inst.children[0]).unwrap().kind.any_text().unwrap().content, "Reusable Pill");

    let ph = find_by_source_id(&doc, "ph-frame");
    assert_eq!(ph.plugin_data.get("pen:placeholder").map(String::as_str), Some("true"));

    let exported: Value = serde_json::from_str(&export_pen_json(&doc).unwrap()).unwrap();
    let exp_pill = &exported["children"][0];
    assert_eq!(exp_pill["id"].as_str(), Some("pill-text"));
    assert_eq!(exp_pill["type"].as_str(), Some("text"));
    assert_eq!(exp_pill["reusable"].as_bool(), Some(true));
    assert_eq!(exp_pill["content"].as_str(), Some("Reusable Pill"));

    let exp_ph = &exported["children"][2];
    assert_eq!(exp_ph["placeholder"].as_bool(), Some(true));
}

// 11. gradient_linear_radial_angular_import_and_export
#[test]
fn gradient_linear_radial_angular_import_and_export() {
    let raw = r##"{
        "version": "2.21",
        "children": [
            {
                "id": "g-lin",
                "type": "rectangle",
                "width": 100,
                "height": 100,
                "fill": {
                    "type": "gradient",
                    "gradientType": "linear",
                    "rotation": 30,
                    "colors": [
                        { "color": "#FF0000", "position": 0.0 },
                        { "color": "#0000FF", "position": 1.0 }
                    ]
                }
            },
            {
                "id": "g-rad",
                "type": "rectangle",
                "width": 100,
                "height": 100,
                "fill": {
                    "type": "gradient",
                    "gradientType": "radial",
                    "rotation": 90,
                    "colors": [
                        { "color": "#00FF00", "position": 0.0 },
                        { "color": "#000000", "position": 1.0 }
                    ]
                }
            },
            {
                "id": "g-ang",
                "type": "rectangle",
                "width": 100,
                "height": 100,
                "fill": {
                    "type": "gradient",
                    "gradientType": "angular",
                    "rotation": 180,
                    "colors": [
                        { "color": "#FFFF00", "position": 0.0 },
                        { "color": "#FF00FF", "position": 0.5 },
                        { "color": "#00FFFF", "position": 1.0 }
                    ]
                }
            }
        ]
    }"##;

    let doc = import_pen_json(raw).expect("import gradients");
    assert!(matches!(find_by_source_id(&doc, "g-lin").fills[0].kind, PaintKind::LinearGradient { .. }));
    assert!(matches!(find_by_source_id(&doc, "g-rad").fills[0].kind, PaintKind::RadialGradient { .. }));
    assert!(matches!(find_by_source_id(&doc, "g-ang").fills[0].kind, PaintKind::AngularGradient { .. }));

    let exported = export_pen_json(&doc).expect("export gradients");
    let rt = import_pen_json(&exported).expect("reimport gradients");
    match &find_by_source_id(&rt, "g-lin").fills[0].kind {
        PaintKind::LinearGradient { stops, .. } => assert_eq!(stops.len(), 2),
        other => panic!("expected LinearGradient, got {other:?}"),
    }
    match &find_by_source_id(&rt, "g-rad").fills[0].kind {
        PaintKind::RadialGradient { stops, .. } => assert_eq!(stops.len(), 2),
        other => panic!("expected RadialGradient, got {other:?}"),
    }
    match &find_by_source_id(&rt, "g-ang").fills[0].kind {
        PaintKind::AngularGradient { stops, .. } => assert_eq!(stops.len(), 3),
        other => panic!("expected AngularGradient, got {other:?}"),
    }
}

// 12. stroke_per_side_weights_alignment_cap_join_and_fill_list
#[test]
fn stroke_per_side_weights_alignment_cap_join_and_fill_list() {
    let raw = r##"{
        "version": "2.21",
        "children": [
            {
                "id": "styled-stroke",
                "type": "frame",
                "width": 200,
                "height": 100,
                "stroke": {
                    "fill": [
                        { "type": "color", "color": "#112233" },
                        { "type": "color", "color": "#445566", "opacity": 0.5 }
                    ],
                    "thickness": { "top": 1, "right": 2, "bottom": 3, "left": 4 },
                    "strokeAlignment": "outside",
                    "cap": "round",
                    "join": "bevel"
                }
            }
        ]
    }"##;

    let doc = import_pen_json(raw).expect("import rich stroke");
    let n = find_by_source_id(&doc, "styled-stroke");
    assert_eq!(n.stroke.side_weights, Some([1.0, 2.0, 3.0, 4.0]));
    assert_eq!(n.stroke.align, StrokeAlign::Outside);
    assert_eq!(n.stroke.cap, StrokeCap::Round);
    assert_eq!(n.stroke.join, StrokeJoin::Bevel);
    assert_eq!(n.stroke.paints.len(), 2);

    let exported = export_pen_json(&doc).expect("export rich stroke");
    let rt = import_pen_json(&exported).expect("reimport rich stroke");
    let rn = find_by_source_id(&rt, "styled-stroke");
    assert_eq!(rn.stroke.side_weights, Some([1.0, 2.0, 3.0, 4.0]));
    assert_eq!(rn.stroke.align, StrokeAlign::Outside);
    assert_eq!(rn.stroke.cap, StrokeCap::Round);
    assert_eq!(rn.stroke.join, StrokeJoin::Bevel);
    assert_eq!(rn.stroke.paints.len(), 2);
}

// 13. icon_font_never_renders_raw_identifier_as_text
#[test]
fn icon_font_never_renders_raw_identifier_as_text() {
    let raw = r##"{
        "version": "2.21",
        "children": [
            {
                "id": "ic-known",
                "type": "icon_font",
                "iconFontFamily": "lucide",
                "iconFontName": "search",
                "width": 24,
                "height": 24,
                "fill": "#111827"
            },
            {
                "id": "ic-unknown",
                "type": "icon_font",
                "iconFontFamily": "custom-pack",
                "iconFontName": "nonexistent-glyph-42",
                "width": 20,
                "height": 20
            }
        ]
    }"##;

    let report = import_pen_json_report(raw, &PenImportOptions::default()).expect("import icon_font");
    for n in report.doc.nodes.values() {
        assert!(!matches!(n.kind, NodeKind::Text(_)), "icon_font must NEVER produce a NodeKind::Text node (found {:?})", n);
    }

    let known = find_by_source_id(&report.doc, "ic-known");
    assert!(matches!(known.kind, NodeKind::Vector { .. }));
    assert!(known.plugin_data.get("pen:icon").unwrap().contains("\"search\""));
    assert!(!known.stroke.paints.is_empty(), "lucide icon fill must map to stroke paints");
    assert_eq!(known.stroke.weight, 2.0);
    assert_eq!(known.stroke.cap, StrokeCap::Round);
    assert_eq!(known.stroke.join, StrokeJoin::Round);

    let unknown = find_by_source_id(&report.doc, "ic-unknown");
    assert!(matches!(unknown.kind, NodeKind::Vector { .. }), "unknown icon must render a fallback vector glyph, not an empty Frame");
    assert!(unknown.plugin_data.get("pen:icon").unwrap().contains("\"nonexistent-glyph-42\""));
    assert!(report.warnings.iter().any(|w| w.code == "unresolved_icon_glyph" && w.node_id.as_deref() == Some("ic-unknown")));

    let exported: Value = serde_json::from_str(&export_pen_json(&report.doc).unwrap()).unwrap();
    assert_eq!(exported["children"][0]["type"].as_str(), Some("icon_font"));
    assert_eq!(exported["children"][0]["iconFontFamily"].as_str(), Some("lucide"));
    assert_eq!(exported["children"][0]["iconFontName"].as_str(), Some("search"));
    assert_eq!(exported["children"][0]["fill"].as_str(), Some("#111827"));
    assert!(exported["children"][0].get("stroke").is_none(), "synthesized icon stroke must export back as fill, not stroke");
    assert_eq!(exported["children"][1]["type"].as_str(), Some("icon_font"));
    assert_eq!(exported["children"][1]["iconFontName"].as_str(), Some("nonexistent-glyph-42"));
}

// 14. ellipse_arc_and_path_viewbox_scaling
#[test]
fn ellipse_arc_and_path_viewbox_scaling() {
    let raw = r##"{
        "version": "2.21",
        "children": [
            {
                "id": "donut-arc",
                "type": "ellipse",
                "width": 80,
                "height": 80,
                "startAngle": 90,
                "sweepAngle": 180,
                "innerRadius": 0.6
            },
            {
                "id": "scaled-path",
                "type": "path",
                "width": 48,
                "height": 48,
                "viewBox": "0 0 24 24",
                "geometry": "M 0 0 L 24 12 L 12 24 Z"
            }
        ]
    }"##;

    let doc = import_pen_json(raw).expect("import ellipse arc and path viewBox");
    let arc_node = find_by_source_id(&doc, "donut-arc");
    match arc_node.kind {
        NodeKind::Ellipse(arc) => {
            assert!((arc.start - 90.0_f32.to_radians()).abs() < 1e-4);
            assert!((arc.end - 270.0_f32.to_radians()).abs() < 1e-4);
            assert!((arc.inner_radius - 0.6).abs() < 1e-4);
        }
        ref other => panic!("expected Ellipse, got {other:?}"),
    }

    let path_node = find_by_source_id(&doc, "scaled-path");
    match &path_node.kind {
        NodeKind::Vector { paths, .. } => {
            assert_eq!(paths.len(), 1);
            assert!(paths[0].cmds.contains(&PathCmd::LineTo(Vec2::new(48.0, 24.0))));
            assert!(paths[0].cmds.contains(&PathCmd::LineTo(Vec2::new(24.0, 48.0))));
        }
        other => panic!("expected Vector, got {other:?}"),
    }

    // Round-trip arc parameters
    let exported = export_pen_json(&doc).unwrap();
    let rt = import_pen_json(&exported).unwrap();
    if let NodeKind::Ellipse(arc) = find_by_source_id(&rt, "donut-arc").kind {
        assert!((arc.start.to_degrees() - 90.0).abs() < 1e-2);
        assert!(((arc.end - arc.start).to_degrees() - 180.0).abs() < 1e-2);
        assert!((arc.inner_radius - 0.6).abs() < 1e-4);
    } else {
        panic!("expected Ellipse on reimport");
    }
}

// 15. layout_position_absolute_and_flips_and_text_decorations
#[test]
fn layout_position_absolute_and_flips_and_text_decorations() {
    let raw = r##"{
        "version": "2.21",
        "children": [
            {
                "id": "parent-stack",
                "type": "frame",
                "width": 200,
                "height": 200,
                "children": [
                    {
                        "id": "abs-pin",
                        "type": "rectangle",
                        "x": 150,
                        "y": 10,
                        "width": 32,
                        "height": 32,
                        "layoutPosition": "absolute",
                        "flipX": true,
                        "flipY": true
                    },
                    {
                        "id": "u-text",
                        "type": "text",
                        "content": "Underlined",
                        "underline": true
                    },
                    {
                        "id": "s-text",
                        "type": "text",
                        "content": "Struck",
                        "strikethrough": true
                    }
                ]
            }
        ]
    }"##;

    let doc = import_pen_json(raw).expect("import absolute, flips, decorations");
    let pin = find_by_source_id(&doc, "abs-pin");
    assert!(pin.layout_child.absolute);
    assert!(pin.transform.a < 0.0 && pin.transform.d < 0.0);

    let ut = find_by_source_id(&doc, "u-text").kind.any_text().unwrap();
    assert_eq!(ut.style.decoration, TextDecoration::Underline);
    let st = find_by_source_id(&doc, "s-text").kind.any_text().unwrap();
    assert_eq!(st.style.decoration, TextDecoration::Strikethrough);

    let exported = export_pen_json(&doc).unwrap();
    let rt = import_pen_json(&exported).unwrap();
    let rt_pin = find_by_source_id(&rt, "abs-pin");
    assert!(rt_pin.layout_child.absolute);
    assert!(rt_pin.transform.a < 0.0 && rt_pin.transform.d < 0.0);
    assert_eq!(find_by_source_id(&rt, "u-text").kind.any_text().unwrap().style.decoration, TextDecoration::Underline);
    assert_eq!(find_by_source_id(&rt, "s-text").kind.any_text().unwrap().style.decoration, TextDecoration::Strikethrough);
}

// 16. href_and_blend_mode_roundtrip
#[test]
fn href_and_blend_mode_roundtrip() {
    let raw = r##"{
        "version": "2.21",
        "children": [
            {
                "id": "link-node",
                "type": "text",
                "content": "Visit Website",
                "href": "https://example.org/docs",
                "blendMode": "multiply"
            }
        ]
    }"##;

    let doc = import_pen_json(raw).expect("import href and blendMode");
    let n = find_by_source_id(&doc, "link-node");
    assert_eq!(n.hyperlink.as_deref(), Some("https://example.org/docs"));
    assert_eq!(n.blend, BlendMode::Multiply);

    let exported = export_pen_json(&doc).unwrap();
    let rt = import_pen_json(&exported).unwrap();
    let rn = find_by_source_id(&rt, "link-node");
    assert_eq!(rn.hyperlink.as_deref(), Some("https://example.org/docs"));
    assert_eq!(rn.blend, BlendMode::Multiply);
}

// 17. image_fill_relative_path_scale_modes_and_traversal_refusal
#[test]
fn image_fill_relative_path_scale_modes_and_traversal_refusal() {
    let temp_dir = std::env::temp_dir().join(format!("pen_img_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    let sample_bytes = b"\x89PNG\r\n\x1a\nfake_png_payload";
    std::fs::write(temp_dir.join("hero.png"), sample_bytes).unwrap();

    let raw = r##"{
        "version": "2.21",
        "children": [
            {
                "id": "img-ok",
                "type": "rectangle",
                "width": 200,
                "height": 120,
                "fill": { "type": "image", "url": "hero.png", "mode": "fit" }
            },
            {
                "id": "img-escape",
                "type": "rectangle",
                "width": 100,
                "height": 100,
                "fill": { "type": "image", "url": "../outside.png", "mode": "tile" }
            }
        ]
    }"##;

    let opts = PenImportOptions { base_dir: Some(temp_dir.clone()) };
    let report = import_pen_json_report(raw, &opts).expect("import image fills");

    let ok_node = find_by_source_id(&report.doc, "img-ok");
    match &ok_node.fills[0].kind {
        PaintKind::Image { image_ref, scale_mode } => {
            assert_eq!(*scale_mode, ImageScaleMode::Fit);
            assert_eq!(report.doc.images.get(image_ref).map(Vec::as_slice), Some(sample_bytes.as_slice()));
        }
        other => panic!("expected PaintKind::Image, got {other:?}"),
    }

    let esc_node = find_by_source_id(&report.doc, "img-escape");
    assert_eq!(esc_node.plugin_data.get("pen:image-url").map(String::as_str), Some("../outside.png"));
    assert!(
        report.warnings.iter().any(|w| w.code == "image_path_refused" && w.node_id.as_deref() == Some("img-escape")),
        "expected image_path_refused warning, got {:?}",
        report.warnings
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

// 18. library_imports_resolution_cycle_detection_and_traversal_refusal
#[test]
fn library_imports_resolution_cycle_detection_and_traversal_refusal() {
    let temp_dir = std::env::temp_dir().join(format!("pen_lib_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    // Cyclic libraries: a.pen imports b.pen, b.pen imports a.pen
    let a_pen = r##"{
        "version": "2.21",
        "imports": { "b": "b.pen" },
        "children": [
            {
                "id": "shared-btn",
                "type": "frame",
                "reusable": true,
                "name": "SharedButton",
                "width": 140,
                "height": 40,
                "fill": "#2563EB"
            }
        ]
    }"##;
    let b_pen = r##"{
        "version": "2.21",
        "imports": { "a": "a.pen" },
        "children": []
    }"##;
    std::fs::write(temp_dir.join("a.pen"), a_pen).unwrap();
    std::fs::write(temp_dir.join("b.pen"), b_pen).unwrap();

    let main_pen = r##"{
        "version": "2.21",
        "imports": {
            "ui": "a.pen",
            "evil": "../escape.pen"
        },
        "children": [
            {
                "id": "local-use",
                "type": "ref",
                "ref": "ui:shared-btn",
                "name": "UsedFromLib"
            }
        ]
    }"##;

    let opts = PenImportOptions { base_dir: Some(temp_dir.clone()) };
    let report = import_pen_json_report(main_pen, &opts).expect("library imports with cycle and traversal must not fail");

    let inst = find_by_source_id(&report.doc, "local-use");
    assert_eq!(inst.size, Vec2::new(140.0, 40.0), "resolved component from ui:shared-btn");
    assert!(report.warnings.iter().any(|w| w.code == "import_cycle"), "expected import_cycle warning");
    assert!(report.warnings.iter().any(|w| w.code == "import_path_refused"), "expected import_path_refused warning");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

// 19. unknown_node_types_preserved_as_unknown_with_raw_payload_roundtrip
#[test]
fn unknown_node_types_preserved_as_unknown_with_raw_payload_roundtrip() {
    let raw = r##"{
        "version": "2.21",
        "children": [
            {
                "id": "ai-prompt-1",
                "type": "prompt",
                "name": "Generate Hero",
                "model": "claude-sonnet",
                "promptText": "Design a dark dashboard header",
                "children": [
                    {
                        "id": "prompt-child",
                        "type": "text",
                        "content": "Nested inside unknown"
                    }
                ]
            }
        ]
    }"##;

    let report = import_pen_json_report(raw, &PenImportOptions::default()).expect("import unknown node type");
    assert!(report.warnings.iter().any(|w| w.code == "unknown_node_type" && w.node_id.as_deref() == Some("ai-prompt-1")), "expected unknown_node_type warning");

    let unk = find_by_source_id(&report.doc, "ai-prompt-1");
    assert_eq!(unk.kind, NodeKind::Unknown { type_name: "prompt".into() });
    assert!(unk.fills.is_empty(), "unknown node must not be given a white frame fill");
    assert_eq!(unk.children.len(), 1);

    let exported: Value = serde_json::from_str(&export_pen_json(&report.doc).unwrap()).unwrap();
    let exp_unk = &exported["children"][0];
    assert_eq!(exp_unk["type"].as_str(), Some("prompt"));
    assert_eq!(exp_unk["model"].as_str(), Some("claude-sonnet"));
    assert_eq!(exp_unk["promptText"].as_str(), Some("Design a dark dashboard header"));
    assert_eq!(exp_unk["children"][0]["id"].as_str(), Some("prompt-child"));
}

// 20. snaps_intermediate_version_2_15_and_migrates
#[test]
fn snaps_intermediate_version_2_15_and_migrates() {
    let raw = r##"{
        "version": "2.15",
        "children": [
            {
                "id": "svg-node",
                "type": "svg",
                "d": "M 0 0 L 10 10",
                "horizontalFlip": true
            }
        ]
    }"##;
    let report = import_pen_json_report(raw, &PenImportOptions::default()).expect("import 2.15");
    assert_eq!(report.source_version, "2.15");
    let n = find_by_source_id(&report.doc, "svg-node");
    assert!(matches!(n.kind, NodeKind::Vector { .. }));
    assert!(n.transform.a < 0.0);
}

// 21. multi_axis_variable_entry_roundtrips_verbatim
#[test]
fn multi_axis_variable_entry_roundtrips_verbatim() {
    let raw = r##"{
        "version": "2.21",
        "themes": {
            "Theme": ["Light", "Dark"],
            "Density": ["Cozy", "Compact"]
        },
        "variables": {
            "combo-gap": {
                "type": "number",
                "value": [
                    { "theme": { "Theme": "Light", "Density": "Cozy" }, "value": 24 },
                    { "theme": { "Theme": "Dark", "Density": "Compact" }, "value": 12 }
                ]
            }
        },
        "children": []
    }"##;

    let doc = import_pen_json(raw).expect("import multi-axis variable");
    assert_eq!(doc.variables["combo-gap"].description, "", "Variable::description must stay untouched");

    let exported: Value = serde_json::from_str(&export_pen_json(&doc).unwrap()).unwrap();
    let arr = exported["variables"]["combo-gap"]["value"].as_array().expect("multi-axis value array");
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["theme"]["Theme"].as_str(), Some("Light"));
    assert_eq!(arr[0]["theme"]["Density"].as_str(), Some("Cozy"));
    assert_eq!(arr[1]["theme"]["Theme"].as_str(), Some("Dark"));
    assert_eq!(arr[1]["theme"]["Density"].as_str(), Some("Compact"));
}

// 22. nested_slash_path_in_descendants_resolves
#[test]
fn nested_slash_path_in_descendants_resolves() {
    let raw = r##"{
        "version": "2.21",
        "children": [
            {
                "id": "leaf-comp",
                "type": "frame",
                "reusable": true,
                "children": [
                    { "id": "deep-txt", "type": "text", "content": "Initial" }
                ]
            },
            {
                "id": "outer-comp",
                "type": "frame",
                "reusable": true,
                "children": [
                    { "id": "inner-ref", "type": "ref", "ref": "leaf-comp" }
                ]
            },
            {
                "id": "top-inst",
                "type": "ref",
                "ref": "outer-comp",
                "descendants": {
                    "inner-ref/deep-txt": {
                        "content": "Deeply Overridden"
                    }
                }
            }
        ]
    }"##;

    let doc = import_pen_json(raw).expect("import nested slash path in descendants");
    let top = find_by_source_id(&doc, "top-inst");
    let inner = doc.get(top.children[0]).unwrap();
    let deep = doc.get(inner.children[0]).unwrap();
    assert_eq!(deep.kind.any_text().unwrap().content, "Deeply Overridden");
}

// 23. remote_image_url_records_warning_and_plugin_data
#[test]
fn remote_image_url_records_warning_and_plugin_data() {
    let raw = r##"{
        "version": "2.21",
        "children": [
            {
                "id": "remote-banner",
                "type": "rectangle",
                "width": 400,
                "height": 200,
                "fill": {
                    "type": "image",
                    "url": "https://cdn.example.com/photo.jpg",
                    "mode": "crop"
                }
            }
        ]
    }"##;

    let report = import_pen_json_report(raw, &PenImportOptions::default()).expect("import remote image");
    assert!(report.warnings.iter().any(|w| w.code == "external_image_url"));
    let banner = find_by_source_id(&report.doc, "remote-banner");
    assert_eq!(banner.plugin_data.get("pen:image-url").map(String::as_str), Some("https://cdn.example.com/photo.jpg"));
    match &banner.fills[0].kind {
        PaintKind::Image { scale_mode, .. } => assert_eq!(*scale_mode, ImageScaleMode::Crop),
        other => panic!("expected Image paint, got {other:?}"),
    }

    let exported: Value = serde_json::from_str(&export_pen_json(&report.doc).unwrap()).unwrap();
    assert_eq!(exported["children"][0]["fill"]["url"].as_str(), Some("https://cdn.example.com/photo.jpg"));
    assert_eq!(exported["children"][0]["fill"]["mode"].as_str(), Some("crop"));
}

// 24. library_import_without_base_dir_records_warning
#[test]
fn library_import_without_base_dir_records_warning() {
    let raw = r##"{
        "version": "2.21",
        "imports": { "tokens": "./tokens.pen" },
        "children": [
            { "id": "missing-ref", "type": "ref", "ref": "tokens:btn" }
        ]
    }"##;

    let report = import_pen_json_report(raw, &PenImportOptions::default()).expect("missing base_dir must warn, not fail");
    assert!(report.warnings.iter().any(|w| w.code == "unresolved_import"));
    assert!(report.warnings.iter().any(|w| w.code == "unresolved_ref"));
}

// 25. export_always_writes_explicit_layout_and_dimensions
#[test]
fn export_always_writes_explicit_layout_and_dimensions() {
    let raw = r##"{
        "version": "2.21",
        "children": [
            {
                "id": "implicit-frame",
                "type": "frame"
            }
        ]
    }"##;

    let doc = import_pen_json(raw).expect("import implicit frame");
    let exported: Value = serde_json::from_str(&export_pen_json(&doc).unwrap()).unwrap();
    let f = &exported["children"][0];
    assert_eq!(f["layout"].as_str(), Some("horizontal"));
    assert_eq!(f["width"].as_str(), Some("fit_content"));
    assert_eq!(f["height"].as_str(), Some("fit_content"));
    assert_eq!(json!(exported["version"]), json!("2.21"));
}

// 26. instance_single_axis_width_override_preserves_inherited_height_across_roundtrip_and_sync
#[test]
fn instance_single_axis_width_override_preserves_inherited_height_across_roundtrip_and_sync() {
    let raw = r##"{
        "version": "2.21",
        "children": [
            {
                "id": "card-comp",
                "type": "frame",
                "reusable": true,
                "name": "Card",
                "width": 260,
                "height": 140
            },
            {
                "id": "inst-width-only",
                "type": "ref",
                "ref": "card-comp",
                "name": "Width Only Instance",
                "width": 320
            }
        ]
    }"##;

    let doc = import_pen_json(raw).expect("import single-axis width override");
    let exported_str = export_pen_json(&doc).expect("export single-axis width override");
    let exported: Value = serde_json::from_str(&exported_str).unwrap();
    let exp_inst = &exported["children"][1];
    assert_eq!(exp_inst.get("width").and_then(Value::as_f64), Some(320.0), "explicit width must be exported");
    assert!(exp_inst.get("height").is_none(), "omitted height on instance must NOT be exported as explicit: {exp_inst}");

    // Re-import and change component height: instance must still inherit the new height!
    let mut rt = import_pen_json(&exported_str).expect("reimport single-axis width override");
    let comp_id = find_by_source_id(&rt, "card-comp").id;
    let inst_id = find_by_source_id(&rt, "inst-width-only").id;
    rt.get_mut(comp_id).unwrap().size.y = 220.0;
    sync_instances(&mut rt, comp_id).apply(&mut rt).expect("sync_instances");
    assert_eq!(
        rt.get(inst_id).unwrap().size,
        Vec2::new(320.0, 220.0),
        "after round-trip, component height change must reach instance that only overrode width"
    );
}

// 27. instance_root_partial_stroke_override_preserves_component_stroke_paints
#[test]
fn instance_root_partial_stroke_override_preserves_component_stroke_paints() {
    let raw = r##"{
        "version": "2.21",
        "children": [
            {
                "id": "stroke-comp",
                "type": "frame",
                "reusable": true,
                "name": "StrokedBox",
                "width": 200,
                "height": 100,
                "stroke": {
                    "fill": "#38BDF8",
                    "thickness": 1,
                    "align": "outside",
                    "cap": "round",
                    "join": "bevel"
                }
            },
            {
                "id": "stroke-inst",
                "type": "ref",
                "ref": "stroke-comp",
                "name": "WiderStrokeInstance",
                "strokeWidth": 4
            }
        ]
    }"##;

    let doc = import_pen_json(raw).expect("import partial stroke override on instance root");
    let inst = find_by_source_id(&doc, "stroke-inst");
    assert_eq!(inst.stroke.weight, 4.0, "strokeWidth override on instance root must apply");
    assert_eq!(
        inst.stroke.paints,
        vec![Paint::solid(Color::from_hex("#38BDF8").unwrap())],
        "instance setting only strokeWidth must inherit stroke paints from component"
    );
    assert_eq!(inst.stroke.align, StrokeAlign::Outside, "instance setting only strokeWidth must inherit stroke align");
    assert_eq!(inst.stroke.cap, StrokeCap::Round, "instance setting only strokeWidth must inherit stroke cap");
    assert_eq!(inst.stroke.join, StrokeJoin::Bevel, "instance setting only strokeWidth must inherit stroke join");

    let exported_str = export_pen_json(&doc).expect("export partial stroke override");
    let exported: Value = serde_json::from_str(&exported_str).unwrap();
    let exp_inst_stroke = &exported["children"][1]["stroke"];
    assert_eq!(exp_inst_stroke.get("thickness").and_then(Value::as_f64), Some(4.0), "explicit stroke thickness must be exported");
    assert!(exp_inst_stroke.get("fill").is_none(), "inherited stroke fill must not be exported as an explicit override: {exp_inst_stroke}");

    let mut rt = import_pen_json(&exported_str).expect("reimport partial stroke override");
    let comp_id = find_by_source_id(&rt, "stroke-comp").id;
    let inst_id = find_by_source_id(&rt, "stroke-inst").id;
    rt.get_mut(comp_id).unwrap().stroke.paints = vec![Paint::solid(Color::from_hex("#EF4444").unwrap())];
    sync_instances(&mut rt, comp_id).apply(&mut rt).expect("sync_instances on stroke-comp");
    let synced_inst = rt.get(inst_id).unwrap();
    assert_eq!(synced_inst.stroke.weight, 4.0, "explicit stroke weight override must survive sync_instances");
    assert_eq!(
        synced_inst.stroke.paints,
        vec![Paint::solid(Color::from_hex("#EF4444").unwrap())],
        "after round-trip, component stroke paint change must propagate to instance that only overrode strokeWidth"
    );
}

// 28. multi_axis_variable_stores_all_combinations_outside_key_and_resolves_with_node_theme
#[test]
fn multi_axis_variable_stores_all_combinations_outside_key_and_resolves_with_node_theme() {
    let raw = r##"{
        "version": "2.21",
        "themes": {
            "ZetaTheme": ["Light", "Dark"],
            "AlphaDensity": ["Cozy", "Compact"]
        },
        "variables": {
            "combo-pad": {
                "type": "number",
                "value": [
                    { "theme": { "ZetaTheme": "Light", "AlphaDensity": "Cozy" }, "value": 24 },
                    { "theme": { "ZetaTheme": "Light", "AlphaDensity": "Compact" }, "value": 12 },
                    { "theme": { "ZetaTheme": "Dark", "AlphaDensity": "Cozy" }, "value": 32 },
                    { "theme": { "ZetaTheme": "Dark", "AlphaDensity": "Compact" }, "value": 16 }
                ]
            }
        },
        "children": [
            {
                "id": "box-default",
                "type": "rectangle",
                "width": "$combo-pad",
                "height": 40
            },
            {
                "id": "box-compact",
                "type": "rectangle",
                "theme": { "AlphaDensity": "Compact" },
                "width": "$combo-pad",
                "height": 40
            },
            {
                "id": "box-dark-compact",
                "type": "rectangle",
                "theme": { "ZetaTheme": "Dark", "AlphaDensity": "Compact" },
                "width": "$combo-pad",
                "height": 40
            }
        ]
    }"##;

    let mut doc = import_pen_json(raw).expect("import multi-axis matrix variable");
    let var = &doc.variables["combo-pad"];
    assert_eq!(var.key, None, "Variable.key is the Figma library key and must not hold pen:multi data");
    assert_eq!(var.values.len(), 4, "all 4 multi-axis combinations must be stored in Variable.values");

    let def_node = find_by_source_id(&doc, "box-default");
    assert_eq!(def_node.size.x, 24.0, "default theme (Light + Cozy) must resolve to 24");

    let compact_node = find_by_source_id(&doc, "box-compact");
    assert_eq!(compact_node.size.x, 12.0, "node with AlphaDensity=Compact (and default ZetaTheme=Light) must resolve to 12");

    let dark_compact_node = find_by_source_id(&doc, "box-dark-compact");
    assert_eq!(dark_compact_node.size.x, 16.0, "node with ZetaTheme=Dark + AlphaDensity=Compact must resolve to 16");

    // Switching the second axis on `box-default` via Document variable modes must change the resolved number!
    let def_id = def_node.id;
    let alpha_col_id = doc.variable_collections.values().find(|c| c.name == "AlphaDensity").unwrap().id.clone();
    doc.get_mut(def_id).unwrap().variables.modes.insert(alpha_col_id, "AlphaDensity:Compact".into());
    assert_eq!(
        doc.resolve_variable(def_id, "combo-pad"),
        Some(VariableValue::Number(12.0)),
        "switching the second theme axis on a node must update resolve_variable"
    );
}

// 29. multi_page_pen_migrates_imports_and_exports_all_pages
#[test]
fn multi_page_pen_migrates_imports_and_exports_all_pages() {
    let raw = r##"{
        "version": "1.0",
        "name": "MultiPage Book",
        "children": [
            {
                "id": "p1-card",
                "type": "frame",
                "name": "Page 1 Card",
                "fillColor": "#112233"
            }
        ],
        "pages": [
            {
                "id": "page-1",
                "name": "Cover",
                "background": "#1E1E1E",
                "children": [
                    {
                        "id": "p1-card",
                        "type": "frame",
                        "name": "Page 1 Card",
                        "fillColor": "#112233"
                    }
                ]
            },
            {
                "id": "page-2",
                "name": "Details",
                "background": "#F8FAFC",
                "children": [
                    {
                        "id": "p2-box",
                        "type": "frame",
                        "name": "Page 2 Box",
                        "padding": { "top": 6, "right": 12, "bottom": 6, "left": 12 },
                        "fillColor": "#445566"
                    }
                ]
            }
        ]
    }"##;

    let doc = import_pen_json(raw).expect("import multi-page .pen");
    assert_eq!(doc.pages.len(), 2, "both pages must be imported");
    assert_eq!(doc.get(doc.pages[0]).unwrap().name, "Cover");
    assert_eq!(doc.get(doc.pages[1]).unwrap().name, "Details");

    let p2_box = find_by_source_id(&doc, "p2-box");
    assert_eq!(
        p2_box.kind.frame_props().unwrap().auto_layout.as_ref().unwrap().padding,
        [6.0, 12.0, 6.0, 12.0],
        "migration chain must walk nodes inside pages[]"
    );

    let exported = export_pen_json(&doc).expect("export multi-page .pen");
    let rt = import_pen_json(&exported).expect("reimport multi-page .pen");
    assert_eq!(rt.pages.len(), 2, "exported multi-page document must preserve page boundaries on re-import");
    assert_eq!(rt.get(rt.pages[0]).unwrap().name, "Cover");
    assert_eq!(rt.get(rt.pages[1]).unwrap().name, "Details");
    assert_eq!(rt.get(rt.pages[0]).unwrap().children.len(), 1);
    assert_eq!(rt.get(rt.pages[1]).unwrap().children.len(), 1);
    assert_documents_pen_field_equal(&doc, &rt, "multi_page_roundtrip");
}

// 30. connection_node_roundtrips_endpoints_and_label
#[test]
fn connection_node_roundtrips_endpoints_and_label() {
    let raw = r##"{
        "version": "2.21",
        "children": [
            {
                "id": "node-a",
                "type": "rectangle",
                "x": 0,
                "y": 0,
                "width": 100,
                "height": 60
            },
            {
                "id": "node-b",
                "type": "rectangle",
                "x": 240,
                "y": 0,
                "width": 100,
                "height": 60
            },
            {
                "id": "conn-1",
                "type": "connection",
                "name": "Flow Arrow",
                "from": { "id": "node-a", "magnet": "right" },
                "to": { "id": "node-b", "magnet": "left" },
                "content": "approves"
            }
        ]
    }"##;

    let doc = import_pen_json(raw).expect("import connection node");
    let a_id = find_by_source_id(&doc, "node-a").id;
    let b_id = find_by_source_id(&doc, "node-b").id;
    let conn = find_by_source_id(&doc, "conn-1");
    match &conn.kind {
        NodeKind::Connector(cp) => {
            assert_eq!(cp.start.node, Some(a_id));
            assert_eq!(cp.start.magnet, Magnet::Right);
            assert_eq!(cp.end.node, Some(b_id));
            assert_eq!(cp.end.magnet, Magnet::Left);
            assert_eq!(cp.label.as_ref().map(|l| l.content.as_str()), Some("approves"));
        }
        other => panic!("expected Connector, got {other:?}"),
    }

    let exported = export_pen_json(&doc).expect("export connection node");
    let rt = import_pen_json(&exported).expect("reimport connection node");
    assert_documents_pen_field_equal(&doc, &rt, "connection_roundtrip");
}

// 31. multi_page_library_import_resolves_components_from_all_pages
#[test]
fn multi_page_library_import_resolves_components_from_all_pages() {
    let tmp_dir = std::env::temp_dir().join(format!("designos_pen_multipage_lib_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).unwrap();

    let lib_pen = r##"{
        "version": "2.21",
        "pages": [
            {
                "id": "lib-page-1",
                "name": "Tokens",
                "children": [
                    {
                        "id": "cover-comp",
                        "type": "frame",
                        "reusable": true,
                        "name": "CoverComp",
                        "width": 120,
                        "height": 40
                    }
                ]
            },
            {
                "id": "lib-page-2",
                "name": "Components",
                "children": [
                    {
                        "id": "page2-btn",
                        "type": "frame",
                        "reusable": true,
                        "name": "Page2Button",
                        "width": 180,
                        "height": 52,
                        "fill": "#10B981"
                    }
                ]
            }
        ]
    }"##;
    std::fs::write(tmp_dir.join("multipage_lib.pen"), lib_pen).unwrap();

    let main_pen = r##"{
        "version": "2.21",
        "imports": {
            "ui": "multipage_lib.pen"
        },
        "children": [
            {
                "id": "use-page2-btn",
                "type": "ref",
                "ref": "ui:page2-btn"
            }
        ]
    }"##;

    let report = import_pen_json_report(main_pen, &PenImportOptions { base_dir: Some(tmp_dir.clone()) }).expect("import main with multi-page library");
    let _ = std::fs::remove_dir_all(&tmp_dir);

    assert!(
        !report.warnings.iter().any(|w| w.code == "unresolved_ref"),
        "expected 0 unresolved_ref warnings when referencing component on page 2 of library, got: {:?}",
        report.warnings
    );
    let inst = find_by_source_id(&report.doc, "use-page2-btn");
    assert_eq!(inst.size, Vec2::new(180.0, 52.0), "instance must inherit dimensions from page-2 library component");
    assert_eq!(inst.fills, vec![Paint::solid(Color::from_hex("#10B981").unwrap())]);
}

// 32. user_stroke_and_size_edits_on_pen_instance_survive_sync_and_pen_round_trip
#[test]
fn user_stroke_and_size_edits_on_pen_instance_survive_sync_and_pen_round_trip() {
    let raw = r##"{
        "version": "2.21",
        "children": [
            {
                "id": "btn-comp",
                "type": "frame",
                "reusable": true,
                "name": "Button",
                "width": 120,
                "height": 40,
                "stroke": { "fill": "#EF4444", "thickness": 1 }
            },
            {
                "id": "btn-inst",
                "type": "ref",
                "ref": "btn-comp",
                "width": 200,
                "stroke": { "thickness": 4 }
            },
            {
                "id": "btn-empty-stroke",
                "type": "ref",
                "ref": "btn-comp",
                "stroke": {}
            }
        ]
    }"##;

    let mut doc = import_pen_json(raw).expect("import partial stroke/size instance");
    let comp_id = find_by_source_id(&doc, "btn-comp").id;
    let inst_id = find_by_source_id(&doc, "btn-inst").id;
    let empty_id = find_by_source_id(&doc, "btn-empty-stroke").id;

    // Empty `stroke: {}` must not write an empty `pen:explicit_stroke_fields` marker or mark Stroke overridden.
    let empty_inst = doc.get(empty_id).unwrap();
    assert!(!empty_inst.plugin_data.contains_key("pen:explicit_stroke_fields"), "empty stroke object must not record pen:explicit_stroke_fields");
    assert!(!empty_inst.component.overrides.contains(&OverrideField::Stroke), "empty stroke object must not mark OverrideField::Stroke");

    // Before user edits, `btn-inst` has partial `.pen` markers from import:
    assert_eq!(doc.get(inst_id).unwrap().plugin_data.get("pen:explicit_stroke_fields").map(String::as_str), Some("width"));
    assert_eq!(doc.get(inst_id).unwrap().plugin_data.get("pen:explicit_width").map(String::as_str), Some("true"));
    assert!(!doc.get(inst_id).unwrap().plugin_data.contains_key("pen:explicit_height"));

    // Simulate a Studio user edit changing the instance's stroke color to blue (#3B82F6) and height to 64.
    let before = doc.get(inst_id).unwrap().clone();
    let blue = Paint::solid(Color::from_hex("#3B82F6").unwrap());
    {
        let inst_mut = doc.get_mut(inst_id).unwrap();
        inst_mut.stroke.paints = vec![blue.clone()];
        inst_mut.size.y = 64.0;
    }
    let after_snapshot = doc.get(inst_id).unwrap().clone();
    let recorded = record_override(&before, &after_snapshot);
    assert!(recorded.contains(&OverrideField::Stroke));
    assert!(recorded.contains(&OverrideField::Size));
    {
        let inst_mut = doc.get_mut(inst_id).unwrap();
        clear_stale_pen_override_markers(inst_mut, &recorded);
        inst_mut.component.overrides.extend(recorded);
    }

    // Syncing instances against `btn-comp` must preserve BOTH the user's blue stroke color + 64 height
    // AND the previously overridden thickness 4 + width 200.
    sync_instances(&mut doc, comp_id).apply(&mut doc).expect("sync_instances after user edit");
    let synced = doc.get(inst_id).unwrap();
    assert_eq!(synced.stroke.paints, vec![blue.clone()], "user-edited stroke color must survive sync_instances");
    assert_eq!(synced.stroke.weight, 4.0, "instance stroke thickness must survive sync_instances");
    assert_eq!(synced.size, Vec2::new(200.0, 64.0), "user-edited height and instance width must survive sync_instances");

    // Exporting to `.pen` and re-importing must preserve both the blue stroke color, thickness 4, and 200x64 size.
    let exported = export_pen_json(&doc).expect("export after user edit");
    let rt = import_pen_json(&exported).expect("reimport after user edit");
    let rt_inst = find_by_source_id(&rt, "btn-inst");
    assert_eq!(rt_inst.stroke.paints, vec![blue], "user-edited stroke color must survive .pen export/import round trip");
    assert_eq!(rt_inst.stroke.weight, 4.0, "instance stroke thickness must survive .pen export/import round trip");
    assert_eq!(rt_inst.size, Vec2::new(200.0, 64.0), "user-edited size must survive .pen export/import round trip");
}

const FORBIDDEN_LEGACY_PEN_KEYS: &[&str] = &[
    "fillColor",
    "strokeColor",
    "strokeThickness",
    "componentId",
    "overrides",
    "slots",
    "isPlaceholder",
    "horizontalFlip",
    "verticalFlip",
    "svgPath",
    "stops",
    "position",
];

const COMMON_PEN_2_21_NODE_KEYS: &[&str] = &[
    "id",
    "type",
    "name",
    "reusable",
    "enabled",
    "opacity",
    "blendMode",
    "href",
    "layoutPosition",
    "placeholder",
    "slot",
    "x",
    "y",
    "rotation",
    "flipX",
    "flipY",
    "width",
    "height",
    "theme",
    "fill",
    "stroke",
    "cornerRadius",
    "effect",
    "children",
];

fn allowed_type_specific_pen_2_21_keys(node_type: &str) -> Option<&'static [&'static str]> {
    match node_type {
        "frame" | "group" | "section" => Some(&["layout", "wrap", "gap", "padding", "justifyContent", "alignItems", "clip"]),
        "rectangle" | "line" => Some(&[]),
        "ellipse" => Some(&["startAngle", "sweepAngle", "innerRadius"]),
        "polygon" => Some(&["polygonCount", "innerRadius"]),
        "path" => Some(&["geometry", "fillRule", "viewBox"]),
        "text" => Some(&[
            "content",
            "fontFamily",
            "fontSize",
            "fontWeight",
            "fontStyle",
            "letterSpacing",
            "lineHeight",
            "textAlign",
            "underline",
            "strikethrough",
            "textDecoration",
        ]),
        "icon_font" => Some(&["iconFontFamily", "iconFontName", "weight"]),
        "note" => Some(&["content", "author"]),
        "ref" => Some(&["ref", "descendants", "layout", "wrap", "gap", "padding", "justifyContent", "alignItems", "clip"]),
        "connection" => Some(&["from", "to", "content"]),
        "boolean" => Some(&["operation"]),
        _ => None,
    }
}

fn is_valid_pen_dimension_value(v: &Value) -> bool {
    match v {
        Value::Number(n) => n.as_f64().is_some_and(|x| x.is_finite() && x >= 0.0),
        Value::String(s) => {
            s == "fit_content"
                || s == "fill_container"
                || (s.starts_with("fit_content(") && s.ends_with(')'))
                || (s.starts_with("fill_container(") && s.ends_with(')'))
                || s.starts_with('$')
        }
        _ => false,
    }
}

fn is_valid_pen_num_or_var(v: &Value) -> bool {
    v.as_f64().is_some_and(|x| x.is_finite()) || v.as_str().is_some_and(|s| s.starts_with('$'))
}

fn validate_pen_2_21_fill_shape(v: &Value, path: &str) -> Result<(), String> {
    match v {
        Value::String(s) => {
            if !(s.starts_with('#') || s.starts_with('$')) {
                return Err(format!("{path}: fill string '{s}' must be a hex color or $variable"));
            }
            Ok(())
        }
        Value::Object(obj) => {
            for k in obj.keys() {
                if FORBIDDEN_LEGACY_PEN_KEYS.contains(&k.as_str()) {
                    return Err(format!("{path}: forbidden legacy key '{k}' in fill object"));
                }
            }
            match obj.get("type").and_then(Value::as_str) {
                Some("color") => {
                    if !obj.get("color").and_then(Value::as_str).is_some_and(|s| s.starts_with('#') || s.starts_with('$')) {
                        return Err(format!("{path}: color fill object must have hex/$ 'color'"));
                    }
                    Ok(())
                }
                Some("gradient") => {
                    if obj.contains_key("stops") {
                        return Err(format!("{path}: gradient fill must use 2.21 'colors', not legacy 'stops'"));
                    }
                    let colors = obj.get("colors").and_then(Value::as_array).ok_or_else(|| format!("{path}: gradient fill missing 'colors' array"))?;
                    for (idx, stop) in colors.iter().enumerate() {
                        let sobj = stop.as_object().ok_or_else(|| format!("{path}.colors[{idx}]: must be object"))?;
                        for sk in sobj.keys() {
                            if !["color", "position"].contains(&sk.as_str()) {
                                return Err(format!("{path}.colors[{idx}]: unexpected key '{sk}' in 2.21 gradient stop"));
                            }
                        }
                        if !sobj.get("color").and_then(Value::as_str).is_some_and(|s| s.starts_with('#') || s.starts_with('$')) {
                            return Err(format!("{path}.colors[{idx}]: missing valid 'color'"));
                        }
                        if !sobj.get("position").is_some_and(is_valid_pen_num_or_var) {
                            return Err(format!("{path}.colors[{idx}]: missing valid 'position'"));
                        }
                    }
                    Ok(())
                }
                Some("image") => {
                    if obj.get("url").and_then(Value::as_str).is_none() {
                        return Err(format!("{path}: image fill missing string 'url'"));
                    }
                    Ok(())
                }
                other => Err(format!("{path}: invalid fill object type {other:?}")),
            }
        }
        Value::Array(arr) => {
            for (i, item) in arr.iter().enumerate() {
                validate_pen_2_21_fill_shape(item, &format!("{path}[{i}]"))?;
            }
            Ok(())
        }
        _ => Err(format!("{path}: invalid fill value shape {v}")),
    }
}

fn validate_pen_2_21_stroke_shape(v: &Value, path: &str) -> Result<(), String> {
    let obj = v.as_object().ok_or_else(|| format!("{path}: 2.21 stroke must be an object, got {v}"))?;
    for k in obj.keys() {
        if !["fill", "thickness", "align", "strokeAlignment", "cap", "join", "dashPattern"].contains(&k.as_str()) {
            return Err(format!("{path}: unexpected key '{k}' in 2.21 stroke object"));
        }
    }
    if let Some(f) = obj.get("fill") {
        validate_pen_2_21_fill_shape(f, &format!("{path}.fill"))?;
    }
    if let Some(th) = obj.get("thickness") {
        let valid_th = is_valid_pen_num_or_var(th)
            || th.as_object().is_some_and(|m| {
                !m.is_empty() && m.iter().all(|(side, val)| ["top", "right", "bottom", "left"].contains(&side.as_str()) && is_valid_pen_num_or_var(val))
            });
        if !valid_th {
            return Err(format!("{path}.thickness: invalid 2.21 stroke thickness shape {th}"));
        }
    }
    for align_key in ["align", "strokeAlignment"] {
        if let Some(al) = obj.get(align_key) {
            if !al.as_str().is_some_and(|s| ["inside", "center", "outside"].contains(&s)) {
                return Err(format!("{path}.{align_key}: invalid stroke align {al}"));
            }
        }
    }
    if let Some(cap) = obj.get("cap") {
        if !cap.as_str().is_some_and(|s| ["none", "round", "square"].contains(&s)) {
            return Err(format!("{path}.cap: invalid stroke cap {cap}"));
        }
    }
    if let Some(join) = obj.get("join") {
        if !join.as_str().is_some_and(|s| ["miter", "round", "bevel"].contains(&s)) {
            return Err(format!("{path}.join: invalid stroke join {join}"));
        }
    }
    Ok(())
}

fn validate_pen_2_21_node_shape(node: &Value, path: &str) -> Result<(), String> {
    let obj = node.as_object().ok_or_else(|| format!("{path}: node must be a JSON object"))?;
    let id = obj.get("id").and_then(Value::as_str).ok_or_else(|| format!("{path}: node missing string 'id'"))?;
    let npath = format!("{path}({id})");
    let node_type = obj.get("type").and_then(Value::as_str).ok_or_else(|| format!("{npath}: node missing string 'type'"))?;

    for k in obj.keys() {
        if FORBIDDEN_LEGACY_PEN_KEYS.contains(&k.as_str()) {
            return Err(format!("{npath}: contains forbidden legacy key '{k}'"));
        }
    }

    if let Some(type_keys) = allowed_type_specific_pen_2_21_keys(node_type) {
        for k in obj.keys() {
            if !COMMON_PEN_2_21_NODE_KEYS.contains(&k.as_str()) && !type_keys.contains(&k.as_str()) {
                return Err(format!("{npath}: key '{k}' is not in the allowed 2.21 key set for type '{node_type}'"));
            }
        }
    }

    if let Some(w) = obj.get("width") {
        if !is_valid_pen_dimension_value(w) {
            return Err(format!("{npath}.width: invalid 2.21 dimension shape {w}"));
        }
    }
    if let Some(h) = obj.get("height") {
        if !is_valid_pen_dimension_value(h) {
            return Err(format!("{npath}.height: invalid 2.21 dimension shape {h}"));
        }
    }
    if let Some(layout) = obj.get("layout") {
        if !layout.as_str().is_some_and(|s| ["horizontal", "vertical", "grid", "none"].contains(&s)) {
            return Err(format!("{npath}.layout: invalid 2.21 layout value {layout}"));
        }
    }
    if let Some(pad) = obj.get("padding") {
        let valid_pad =
            is_valid_pen_num_or_var(pad) || pad.as_array().is_some_and(|arr| (arr.len() == 2 || arr.len() == 4) && arr.iter().all(is_valid_pen_num_or_var));
        if !valid_pad {
            return Err(format!("{npath}.padding: invalid 2.21 padding shape {pad}"));
        }
    }
    if let Some(fill) = obj.get("fill") {
        validate_pen_2_21_fill_shape(fill, &format!("{npath}.fill"))?;
    }
    if let Some(stroke) = obj.get("stroke") {
        validate_pen_2_21_stroke_shape(stroke, &format!("{npath}.stroke"))?;
    }
    if let Some(cr) = obj.get("cornerRadius") {
        let valid_cr = is_valid_pen_num_or_var(cr) || cr.as_array().is_some_and(|arr| arr.len() == 4 && arr.iter().all(is_valid_pen_num_or_var));
        if !valid_cr {
            return Err(format!("{npath}.cornerRadius: invalid cornerRadius shape {cr}"));
        }
    }
    if let Some(slot) = obj.get("slot") {
        if !slot.as_array().is_some_and(|arr| arr.iter().all(|x| x.is_string())) {
            return Err(format!("{npath}.slot: 2.21 slot must be an array of strings, got {slot}"));
        }
    }
    if let Some(ph) = obj.get("placeholder") {
        if !ph.is_boolean() {
            return Err(format!("{npath}.placeholder: must be boolean"));
        }
    }
    if let Some(desc) = obj.get("descendants") {
        let dobj = desc.as_object().ok_or_else(|| format!("{npath}.descendants: must be an object"))?;
        for (dk, dv) in dobj {
            let ov = dv.as_object().ok_or_else(|| format!("{npath}.descendants[{dk}]: must be an object"))?;
            for ok in ov.keys() {
                if FORBIDDEN_LEGACY_PEN_KEYS.contains(&ok.as_str()) {
                    return Err(format!("{npath}.descendants[{dk}]: contains forbidden legacy key '{ok}'"));
                }
            }
            if let Some(f) = ov.get("fill") {
                validate_pen_2_21_fill_shape(f, &format!("{npath}.descendants[{dk}].fill"))?;
            }
            if let Some(s) = ov.get("stroke") {
                validate_pen_2_21_stroke_shape(s, &format!("{npath}.descendants[{dk}].stroke"))?;
            }
            if let Some(ch) = ov.get("children") {
                let carr = ch.as_array().ok_or_else(|| format!("{npath}.descendants[{dk}].children: must be array"))?;
                for (ci, cn) in carr.iter().enumerate() {
                    validate_pen_2_21_node_shape(cn, &format!("{npath}.descendants[{dk}].children[{ci}]"))?;
                }
            }
        }
    }
    if let Some(children) = obj.get("children") {
        let carr = children.as_array().ok_or_else(|| format!("{npath}.children: must be an array"))?;
        for (i, ch) in carr.iter().enumerate() {
            validate_pen_2_21_node_shape(ch, &format!("{npath}.children[{i}]"))?;
        }
    }
    Ok(())
}

fn validate_pen_2_21_document_shape(root: &Value, ctx: &str) -> Result<(), String> {
    let obj = root.as_object().ok_or_else(|| format!("{ctx}: root must be an object"))?;
    for k in obj.keys() {
        if !["version", "name", "imports", "themes", "variables", "children", "pages"].contains(&k.as_str()) {
            return Err(format!("{ctx}: unexpected top-level key '{k}'"));
        }
    }
    if obj.get("version").and_then(Value::as_str) != Some("2.21") {
        return Err(format!("{ctx}: root version must be '2.21'"));
    }
    let children = obj.get("children").and_then(Value::as_array).ok_or_else(|| format!("{ctx}: root missing 'children' array"))?;
    for (i, ch) in children.iter().enumerate() {
        validate_pen_2_21_node_shape(ch, &format!("{ctx}/children[{i}]"))?;
    }
    if let Some(pages) = obj.get("pages") {
        let parr = pages.as_array().ok_or_else(|| format!("{ctx}: 'pages' must be an array"))?;
        for (pi, page) in parr.iter().enumerate() {
            let pobj = page.as_object().ok_or_else(|| format!("{ctx}/pages[{pi}]: must be an object"))?;
            for pk in pobj.keys() {
                if !["id", "name", "background", "children"].contains(&pk.as_str()) {
                    return Err(format!("{ctx}/pages[{pi}]: unexpected page key '{pk}'"));
                }
            }
            let pchildren = pobj.get("children").and_then(Value::as_array).ok_or_else(|| format!("{ctx}/pages[{pi}]: missing 'children'"))?;
            for (ci, ch) in pchildren.iter().enumerate() {
                validate_pen_2_21_node_shape(ch, &format!("{ctx}/pages[{pi}]/children[{ci}]"))?;
            }
        }
    }
    Ok(())
}

// 33. exported_fixtures_validate_against_2_21_schema_shape (AC2)
#[test]
fn exported_fixtures_validate_against_2_21_schema_shape() {
    for file in ["v1_0.pen", "v2_8.pen", "v2_13.pen", "v2_17.pen", "v2_21.pen"] {
        let raw = fixture_str(file);
        let doc = import_pen_json(&raw).unwrap_or_else(|e| panic!("import {file}: {e}"));
        let exported = export_pen_json(&doc).unwrap_or_else(|e| panic!("export {file}: {e}"));
        let val: Value = serde_json::from_str(&exported).expect("valid exported JSON");
        validate_pen_2_21_document_shape(&val, file).unwrap_or_else(|e| panic!("{file} failed 2.21 schema validation: {e}"));
    }

    // Verify the validator rejects legacy keys and invalid shapes.
    let bad_legacy = json!({
        "version": "2.21",
        "children": [{ "id": "r1", "type": "rectangle", "fillColor": "#FF0000" }]
    });
    assert!(validate_pen_2_21_document_shape(&bad_legacy, "bad_legacy").is_err());

    let bad_type_key = json!({
        "version": "2.21",
        "children": [{ "id": "r2", "type": "rectangle", "polygonCount": 5 }]
    });
    assert!(validate_pen_2_21_document_shape(&bad_type_key, "bad_type_key").is_err());

    let bad_stroke_shape = json!({
        "version": "2.21",
        "children": [{ "id": "r3", "type": "rectangle", "stroke": 2 }]
    });
    assert!(validate_pen_2_21_document_shape(&bad_stroke_shape, "bad_stroke_shape").is_err());
}

// 34. icon_nodes_render_non_zero_pixels_for_plus_check_trash_2_and_unknown
#[test]
fn icon_nodes_render_non_zero_pixels_for_plus_check_trash_2_and_unknown() {
    let raw = r##"{
        "version": "2.21",
        "children": [
            {
                "id": "ic-plus",
                "type": "icon_font",
                "iconFontFamily": "lucide",
                "iconFontName": "plus",
                "width": 24,
                "height": 24,
                "fill": "#111827"
            },
            {
                "id": "ic-check",
                "type": "icon_font",
                "iconFontFamily": "lucide",
                "iconFontName": "check",
                "width": 24,
                "height": 24,
                "fill": "#111827"
            },
            {
                "id": "ic-trash-2",
                "type": "icon_font",
                "iconFontFamily": "lucide",
                "iconFontName": "trash-2",
                "width": 24,
                "height": 24,
                "fill": "#111827"
            },
            {
                "id": "ic-fallback",
                "type": "icon_font",
                "iconFontFamily": "lucide",
                "iconFontName": "custom-unlisted-icon-99",
                "width": 24,
                "height": 24,
                "fill": "#111827"
            }
        ]
    }"##;

    let doc = import_pen_json(raw).expect("import icon nodes");
    for sid in ["ic-plus", "ic-check", "ic-trash-2", "ic-fallback"] {
        let node = find_by_source_id(&doc, sid);
        assert!(matches!(node.kind, NodeKind::Vector { .. }), "{sid} must be a Vector node, got {:?}", node.kind);
        let pm = designos_render::render_node(&doc, node.id, 1.0).unwrap_or_else(|| panic!("render_node for {sid}"));
        let drawn_pixels = pm.pixels().iter().filter(|px| px.alpha() > 0).count();
        assert!(drawn_pixels > 0, "icon '{sid}' must draw > 0 visible pixels when rendered, got 0");
    }
}
