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

        let reimported = import_pen_json_report(&exported, &PenImportOptions::default()).unwrap_or_else(|e| panic!("failed to reimport {file}: {e}"));
        assert_eq!(reimported.source_version, "2.21");
        assert_eq!(reimported.doc.name, report.doc.name);
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

    let unknown = find_by_source_id(&report.doc, "ic-unknown");
    assert!(matches!(unknown.kind, NodeKind::Frame(_)));
    assert!(unknown.plugin_data.get("pen:icon").unwrap().contains("\"nonexistent-glyph-42\""));
    assert!(report.warnings.iter().any(|w| w.code == "unresolved_icon_glyph" && w.node_id.as_deref() == Some("ic-unknown")));

    let exported: Value = serde_json::from_str(&export_pen_json(&report.doc).unwrap()).unwrap();
    assert_eq!(exported["children"][0]["type"].as_str(), Some("icon_font"));
    assert_eq!(exported["children"][0]["iconFontFamily"].as_str(), Some("lucide"));
    assert_eq!(exported["children"][0]["iconFontName"].as_str(), Some("search"));
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
