//! A `.fig` save writes every variable referenced by any layer, style, or variable alias,
//! and keeps inherited variable-bound instance paints free of literal paint overrides.

use designos_doc::*;
use designos_fig::{export_fig, import_fig, import_fig_node_changes, import_fig_with_report};
use serde_json::{json, Value};

fn guid(s: u64, l: u64) -> Value {
    json!({"sessionID": s, "localID": l})
}

fn var_alias(s: u64, l: u64, resolved: &str) -> Value {
    json!({"dataType": "ALIAS", "resolvedDataType": resolved, "value": {"alias": {"guid": guid(s, l)}}})
}

fn fig_node(id: (u64, u64), parent: (u64, u64), ty: &str, name: &str, extra: Value) -> Value {
    let mut v = json!({
        "guid": guid(id.0, id.1),
        "parentIndex": {"guid": guid(parent.0, parent.1), "position": "!"},
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

#[test]
fn fig_round_trip_keeps_variables_used_only_by_styles_and_aliases() {
    let base_teal = json!({"r": 0.08, "g": 0.52, "b": 0.45, "a": 1.0});
    let base_slate = json!({"r": 0.12, "g": 0.16, "b": 0.22, "a": 1.0});

    // Build a `.fig` message where:
    // - Variable (1, 10) ("Surface/StyleOnly") is referenced ONLY by a shared FILL style (2, 100),
    //   and aliases Variable (1, 11) ("Primitive/Teal700"), which is used ONLY through that alias.
    // - Variable (1, 12) ("Text/Muted") is marked `isSoftDeleted: true` in the `.fig` container
    //   yet still referenced by a layer's fill binding, and aliases another `isSoftDeleted: true`
    //   primitive Variable (1, 13) ("Primitive/Slate800").
    // - Variable (1, 14) ("Unused/Deleted") is `isSoftDeleted: true` and unreferenced anywhere.
    // - VariableSet (1, 1) uses mode ID (1, 1), sharing `(sessionID, localID)` with the collection GUID.
    let changes = json!([
        fig_node((0, 0), (0, 0), "DOCUMENT", "Document", json!({})),
        fig_node((0, 1), (0, 0), "CANVAS", "Page 1", json!({})),
        fig_node((0, 2), (0, 0), "CANVAS", "Internal Only Canvas", json!({"internalOnly": true})),
        fig_node(
            (1, 1),
            (0, 2),
            "VARIABLE_SET",
            "Tokens",
            json!({
                "key": "tokens-set",
                "variableSetModes": [{"id": guid(1, 1), "name": "Default", "sortPosition": "!"}]
            })
        ),
        fig_node(
            (1, 11),
            (0, 2),
            "VARIABLE",
            "Primitive/Teal700",
            json!({
                "key": "prim-teal-700",
                "variableSetID": {"guid": guid(1, 1)},
                "variableResolvedType": "COLOR",
                "variableDataValues": {
                    "entries": [{
                        "modeID": guid(1, 1),
                        "variableData": {"dataType": "COLOR", "value": {"colorValue": base_teal}}
                    }]
                }
            })
        ),
        fig_node(
            (1, 10),
            (0, 2),
            "VARIABLE",
            "Surface/StyleOnly",
            json!({
                "key": "surface-style-only",
                "variableSetID": {"guid": guid(1, 1)},
                "variableResolvedType": "COLOR",
                "variableDataValues": {
                    "entries": [{
                        "modeID": guid(1, 1),
                        "variableData": var_alias(1, 11, "COLOR")
                    }]
                }
            })
        ),
        fig_node(
            (1, 13),
            (0, 2),
            "VARIABLE",
            "Primitive/Slate800",
            json!({
                "key": "prim-slate-800",
                "isSoftDeleted": true,
                "variableSetID": {"guid": guid(1, 1)},
                "variableResolvedType": "COLOR",
                "variableDataValues": {
                    "entries": [{
                        "modeID": guid(1, 1),
                        "variableData": {"dataType": "COLOR", "value": {"colorValue": base_slate}}
                    }]
                }
            })
        ),
        fig_node(
            (1, 12),
            (0, 2),
            "VARIABLE",
            "Text/Muted",
            json!({
                "key": "text-muted",
                "isSoftDeleted": true,
                "variableSetID": {"guid": guid(1, 1)},
                "variableResolvedType": "COLOR",
                "variableDataValues": {
                    "entries": [{
                        "modeID": guid(1, 1),
                        "variableData": var_alias(1, 13, "COLOR")
                    }]
                }
            })
        ),
        fig_node(
            (1, 14),
            (0, 2),
            "VARIABLE",
            "Unused/Deleted",
            json!({
                "key": "unused-deleted",
                "isSoftDeleted": true,
                "variableSetID": {"guid": guid(1, 1)},
                "variableResolvedType": "COLOR",
                "variableDataValues": {
                    "entries": [{
                        "modeID": guid(1, 1),
                        "variableData": {"dataType": "COLOR", "value": {"colorValue": base_slate}}
                    }]
                }
            })
        ),
        fig_node(
            (2, 100),
            (0, 2),
            "RECTANGLE",
            "Brand/Surface",
            json!({
                "styleType": "FILL",
                "key": "style-brand-surface",
                "fillPaints": [{
                    "type": "SOLID",
                    "color": base_teal,
                    "opacity": 1.0,
                    "visible": true,
                    "blendMode": "NORMAL",
                    "colorVar": var_alias(1, 10, "COLOR")
                }]
            })
        ),
        fig_node(
            (3, 1),
            (0, 1),
            "FRAME",
            "Banner",
            json!({
                "fillPaints": [{
                    "type": "SOLID",
                    "color": base_slate,
                    "opacity": 1.0,
                    "visible": true,
                    "blendMode": "NORMAL",
                    "colorVar": var_alias(1, 12, "COLOR")
                }]
            })
        )
    ]);

    let mut doc = import_fig_node_changes(&changes, "StyleAndAliasVars").expect("import node changes");
    // Also add a variable-bound fill style whose paints vector is empty (the shape created by REST style metadata
    // when a shared fill style carries only `boundVariables`), bound to a variable that aliases another variable.
    doc.variables.insert(
        "VariableID:9:2".into(),
        Variable {
            id: "VariableID:9:2".into(),
            name: "Primitive/Amber500".into(),
            key: None,
            collection: "VariableCollectionId:1:1".into(),
            kind: VariableKind::Color,
            values: [("1:1".into(), VariableValue::Color(Color::from_hex("#F59E0B").unwrap()))].into_iter().collect(),
            description: String::new(),
            scopes: Vec::new(),
            hidden_from_publishing: false,
            code_syntax: Default::default(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        },
    );
    doc.variables.insert(
        "VariableID:9:1".into(),
        Variable {
            id: "VariableID:9:1".into(),
            name: "Accent/EmptyPaintStyleOnly".into(),
            key: None,
            collection: "VariableCollectionId:1:1".into(),
            kind: VariableKind::Color,
            values: [("1:1".into(), VariableValue::Alias("VariableID:9:2".into()))].into_iter().collect(),
            description: String::new(),
            scopes: Vec::new(),
            hidden_from_publishing: false,
            code_syntax: Default::default(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        },
    );
    if let Some(coll) = doc.variable_collections.get_mut("VariableCollectionId:1:1") {
        coll.variables.push("VariableID:9:2".into());
        coll.variables.push("VariableID:9:1".into());
    }
    doc.styles.insert("9:100".into(), Style::Fill(Vec::new()));
    doc.style_meta.insert(
        "9:100".into(),
        StyleMeta {
            name: "Accent/Surface".into(),
            key: Some("style-accent-surface".into()),
            property: StyleProperty::Fill,
            bindings: [("fills/0".into(), "VariableID:9:1".into())].into_iter().collect(),
            ..Default::default()
        },
    );

    let saved = export_fig(&doc).expect("export .fig");
    let (reopened, report) = import_fig_with_report(&saved).expect("reopen .fig");

    assert_eq!(report.missing_variable_bindings, 0, "reopened .fig must have zero missing variable bindings");
    for expected_name in ["Surface/StyleOnly", "Primitive/Teal700", "Text/Muted", "Primitive/Slate800", "Accent/EmptyPaintStyleOnly", "Primitive/Amber500"] {
        assert!(reopened.variables.values().any(|v| v.name == expected_name), "variable {expected_name} must survive the .fig round trip");
    }
    assert!(!reopened.variables.values().any(|v| v.name == "Unused/Deleted"), "unreferenced soft-deleted variable must not be imported");

    let brand_style = reopened.style_meta.values().find(|m| m.name == "Brand/Surface").expect("Brand/Surface style meta");
    let brand_var_id = brand_style.bindings.get("fills/0").expect("Brand/Surface fill variable binding");
    assert_eq!(reopened.variables[brand_var_id].name, "Surface/StyleOnly");

    let accent_style = reopened.style_meta.values().find(|m| m.name == "Accent/Surface").expect("Accent/Surface style meta");
    let accent_var_id = accent_style.bindings.get("fills/0").expect("Accent/Surface fill variable binding");
    assert_eq!(reopened.variables[accent_var_id].name, "Accent/EmptyPaintStyleOnly");
}

#[test]
fn fig_round_trip_writes_no_paint_override_for_inherited_variable_bound_instance_paints() {
    let white = json!({"r": 1.0, "g": 1.0, "b": 1.0, "a": 1.0});
    let gray = json!({"r": 0.4, "g": 0.45, "b": 0.52, "a": 1.0});
    let red = json!({"r": 0.9, "g": 0.2, "b": 0.2, "a": 1.0});

    // Main component (10, 1) with variable-bound fill and child vector (10, 2) with variable-bound fill and stroke.
    // Instance (20, 1) inherits all paints and variable bindings from (10, 1) without overrides.
    // Instance (20, 2) overrides its child (10, 2) fill to a fixed red paint.
    let changes = json!([
        fig_node((0, 0), (0, 0), "DOCUMENT", "Document", json!({})),
        fig_node((0, 1), (0, 0), "CANVAS", "Page 1", json!({})),
        fig_node(
            (1, 5),
            (0, 1),
            "VARIABLE_SET",
            "Colors",
            json!({
                "key": "colors-set",
                "variableSetModes": [{"id": guid(1, 6), "name": "Light", "sortPosition": "!"}]
            })
        ),
        fig_node(
            (1, 7),
            (0, 1),
            "VARIABLE",
            "Colors/Bg/Primary",
            json!({
                "key": "bg-primary",
                "variableSetID": {"guid": guid(1, 5)},
                "variableResolvedType": "COLOR",
                "variableDataValues": {
                    "entries": [{
                        "modeID": guid(1, 6),
                        "variableData": {"dataType": "COLOR", "value": {"colorValue": white}}
                    }]
                }
            })
        ),
        fig_node(
            (1, 8),
            (0, 1),
            "VARIABLE",
            "Colors/Fg/Quaternary",
            json!({
                "key": "fg-quaternary",
                "variableSetID": {"guid": guid(1, 5)},
                "variableResolvedType": "COLOR",
                "variableDataValues": {
                    "entries": [{
                        "modeID": guid(1, 6),
                        "variableData": {"dataType": "COLOR", "value": {"colorValue": gray}}
                    }]
                }
            })
        ),
        fig_node(
            (1, 9),
            (0, 1),
            "VARIABLE",
            "Colors/Bg/Danger",
            json!({
                "key": "bg-danger",
                "variableSetID": {"guid": guid(1, 5)},
                "variableResolvedType": "COLOR",
                "variableDataValues": {
                    "entries": [{
                        "modeID": guid(1, 6),
                        "variableData": {"dataType": "COLOR", "value": {"colorValue": red}}
                    }]
                }
            })
        ),
        fig_node(
            (11, 1),
            (0, 1),
            "SYMBOL",
            "BadgeVariant=Neutral",
            json!({
                "fillPaints": [{
                    "type": "SOLID",
                    "color": white,
                    "opacity": 1.0,
                    "visible": true,
                    "blendMode": "NORMAL",
                    "colorVar": var_alias(1, 7, "COLOR")
                }],
                "strokePaints": [{
                    "type": "SOLID",
                    "color": gray,
                    "opacity": 1.0,
                    "visible": true,
                    "blendMode": "NORMAL",
                    "colorVar": var_alias(1, 8, "COLOR")
                }],
                "strokeWeight": 1.0
            })
        ),
        fig_node(
            (11, 2),
            (0, 1),
            "SYMBOL",
            "BadgeVariant=Danger",
            json!({
                "fillPaints": [{
                    "type": "SOLID",
                    "color": red,
                    "opacity": 1.0,
                    "visible": true,
                    "blendMode": "NORMAL",
                    "colorVar": var_alias(1, 9, "COLOR")
                }],
                "strokePaints": [{
                    "type": "SOLID",
                    "color": red,
                    "opacity": 1.0,
                    "visible": true,
                    "blendMode": "NORMAL",
                    "colorVar": var_alias(1, 9, "COLOR")
                }],
                "strokeWeight": 1.0
            })
        ),
        fig_node(
            (10, 1),
            (0, 1),
            "SYMBOL",
            "HeaderChip",
            json!({
                "fillPaints": [{
                    "type": "SOLID",
                    "color": white,
                    "opacity": 1.0,
                    "visible": true,
                    "blendMode": "NORMAL",
                    "colorVar": var_alias(1, 7, "COLOR")
                }]
            })
        ),
        fig_node(
            (10, 2),
            (10, 1),
            "VECTOR",
            "Icon",
            json!({
                "fillPaints": [{
                    "type": "SOLID",
                    "color": white,
                    "opacity": 1.0,
                    "visible": true,
                    "blendMode": "NORMAL",
                    "colorVar": var_alias(1, 7, "COLOR")
                }],
                "strokePaints": [{
                    "type": "SOLID",
                    "color": gray,
                    "opacity": 1.0,
                    "visible": true,
                    "blendMode": "NORMAL",
                    "colorVar": var_alias(1, 8, "COLOR")
                }],
                "strokeWeight": 2.0
            })
        ),
        fig_node(
            (10, 3),
            (10, 1),
            "INSTANCE",
            "NestedBadge",
            json!({
                "symbolData": {
                    "symbolID": guid(11, 1),
                    "symbolOverrides": []
                },
                "fillPaints": [{
                    "type": "SOLID",
                    "color": white,
                    "opacity": 1.0,
                    "visible": true,
                    "blendMode": "NORMAL",
                    "colorVar": var_alias(1, 7, "COLOR")
                }],
                "strokePaints": [{
                    "type": "SOLID",
                    "color": gray,
                    "opacity": 1.0,
                    "visible": true,
                    "blendMode": "NORMAL",
                    "colorVar": var_alias(1, 8, "COLOR")
                }],
                "strokeWeight": 1.0
            })
        ),
        fig_node(
            (20, 1),
            (0, 1),
            "INSTANCE",
            "InheritedChip",
            json!({
                "symbolData": {
                    "symbolID": guid(10, 1),
                    "symbolOverrides": [{
                        "guidPath": {"guids": [guid(10, 3)]},
                        "overriddenSymbolID": guid(11, 2)
                    }]
                },
                "fillPaints": [{
                    "type": "SOLID",
                    "color": white,
                    "opacity": 1.0,
                    "visible": true,
                    "blendMode": "NORMAL",
                    "colorVar": var_alias(1, 7, "COLOR")
                }]
            })
        ),
        fig_node(
            (20, 2),
            (0, 1),
            "INSTANCE",
            "OverriddenChip",
            json!({
                "symbolData": {
                    "symbolID": guid(10, 1),
                    "symbolOverrides": [{
                        "guidPath": {"guids": [guid(10, 2)]},
                        "fillPaints": [{
                            "type": "SOLID",
                            "color": red,
                            "opacity": 1.0,
                            "visible": true,
                            "blendMode": "NORMAL"
                        }]
                    }]
                },
                "fillPaints": [{
                    "type": "SOLID",
                    "color": white,
                    "opacity": 1.0,
                    "visible": true,
                    "blendMode": "NORMAL",
                    "colorVar": var_alias(1, 7, "COLOR")
                }]
            })
        )
    ]);

    let doc = import_fig_node_changes(&changes, "InheritedInstancePaints").expect("import node changes");
    let saved = export_fig(&doc).expect("export .fig");
    let reopened = import_fig(&saved).expect("reopen .fig");

    let inherited = reopened.nodes.values().find(|n| n.name == "InheritedChip").expect("InheritedChip instance");
    assert!(
        !inherited.component.overrides.contains(&OverrideField::Fills),
        "InheritedChip root must not gain a fill override: {:?}",
        inherited.component.overrides
    );
    assert!(
        !inherited.component.overrides.contains(&OverrideField::Stroke),
        "InheritedChip root must not gain a stroke override: {:?}",
        inherited.component.overrides
    );
    for &cid in &inherited.children {
        let child = reopened.get(cid).expect("InheritedChip child");
        assert!(
            !child.component.overrides.contains(&OverrideField::Fills),
            "InheritedChip child {} must not gain a fill override: {:?}",
            child.name,
            child.component.overrides
        );
        assert!(
            !child.component.overrides.contains(&OverrideField::Stroke),
            "InheritedChip child {} must not gain a stroke override: {:?}",
            child.name,
            child.component.overrides
        );
    }

    let page_id = reopened.pages[0];
    let spec = serde_json::to_value(reopened.export_spec(page_id).expect("export_spec")).expect("spec JSON");
    let children = spec["children"].as_array().expect("page children in spec");
    let inherited_spec = children.iter().find(|c| c["name"] == "InheritedChip").expect("InheritedChip in spec");
    assert_eq!(
        inherited_spec["overrides"],
        json!([{"path": "NestedBadge", "swap": "BadgeVariant=Danger"}]),
        "InheritedChip must carry only its nested component swap and no fill or stroke override in exported spec: {inherited_spec}"
    );

    let overridden = reopened.nodes.values().find(|n| n.name == "OverriddenChip").expect("OverriddenChip instance");
    let overridden_child = overridden.children.iter().filter_map(|&cid| reopened.get(cid)).find(|n| n.name == "Icon").expect("OverriddenChip Icon child");
    assert!(overridden_child.component.overrides.contains(&OverrideField::Fills), "OverriddenChip child must retain its genuine fill override");
    assert!(!overridden_child.component.overrides.contains(&OverrideField::Stroke), "OverriddenChip child must not gain a stroke override");
}

#[test]
fn fig_round_trip_keeps_numbered_list_formatting_on_instance_text_override() {
    let changes = json!([
        fig_node((0, 0), (0, 0), "DOCUMENT", "Document", json!({})),
        fig_node((0, 1), (0, 0), "CANVAS", "Page 1", json!({})),
        fig_node((10, 1), (0, 1), "SYMBOL", "NoteCard", json!({})),
        fig_node(
            (10, 2),
            (10, 1),
            "TEXT",
            "Body",
            json!({
                "fontSize": 14.0,
                "fontName": {"family": "Inter", "style": "Regular", "postscript": ""},
                "textData": {
                    "characters": "Placeholder\nSecond",
                    "lines": [
                        {"lineType": "PLAIN", "indentationLevel": 0},
                        {"lineType": "PLAIN", "indentationLevel": 0}
                    ]
                }
            })
        ),
        fig_node(
            (20, 1),
            (0, 1),
            "INSTANCE",
            "NoteInstance",
            json!({
                "symbolData": {
                    "symbolID": guid(10, 1),
                    "symbolOverrides": [{
                        "guidPath": {"guids": [guid(10, 2)]},
                        "textData": {
                            "characters": "First step\nSecond step",
                            "lines": [
                                {"lineType": "ORDERED_LIST", "indentationLevel": 1},
                                {"lineType": "ORDERED_LIST", "indentationLevel": 1}
                            ]
                        }
                    }]
                }
            })
        )
    ]);

    let doc = import_fig_node_changes(&changes, "NumberedListOverride").expect("import node changes");
    let saved = export_fig(&doc).expect("export .fig");
    let reopened = import_fig(&saved).expect("reopen .fig");

    let inst = reopened.nodes.values().find(|n| n.name == "NoteInstance").expect("NoteInstance");
    let body = inst.children.iter().filter_map(|&cid| reopened.get(cid)).find(|n| n.name == "Body").expect("Body child");
    let NodeKind::Text(tp) = &body.kind else {
        panic!("expected text node");
    };
    assert_eq!(tp.content, "First step\nSecond step");
    assert_eq!(
        tp.paragraph_lists(),
        vec![Some(ListItem { kind: ListKind::Numbered, level: 0 }), Some(ListItem { kind: ListKind::Numbered, level: 0 }),],
        "numbered list formatting on an instance text override must survive a .fig round trip"
    );
}
