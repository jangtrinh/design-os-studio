use designos_doc::*;
use designos_fig::testkit::sample_fig;
use designos_fig::{commands_blob_to_path, export_fig, import_fig, import_fig_node_changes, path_to_commands_blob};
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

#[test]
fn commands_blob_roundtrip_preserves_all_verbs_and_winding_rules() {
    for (even_odd, fill_rule) in [(false, FillRule::NonZero), (true, FillRule::EvenOdd)] {
        let original = Path {
            cmds: vec![
                PathCmd::MoveTo(Vec2::new(10.0, 20.0)),
                PathCmd::LineTo(Vec2::new(110.0, 20.0)),
                PathCmd::QuadTo(Vec2::new(130.0, 20.0), Vec2::new(130.0, 40.0)),
                PathCmd::CubicTo(Vec2::new(130.0, 70.0), Vec2::new(90.0, 90.0), Vec2::new(60.0, 90.0)),
                PathCmd::Close,
            ],
            fill_rule,
        };
        let blob = path_to_commands_blob(&original);
        let decoded = commands_blob_to_path(&blob, even_odd);
        assert_eq!(decoded.fill_rule, fill_rule);
        assert_eq!(decoded.cmds, original.cmds);
    }
}

#[test]
fn roundtrip_preserves_node_count_across_saves_and_keeps_instance_child_alignment() {
    let g = |s: u32, l: u32| json!({"sessionID": s, "localID": l});
    let node = |guid: (u32, u32), parent: (u32, u32), pos: &str, ty: &str, name: &str, extra: Value| -> Value {
        let mut m = json!({
            "guid": g(guid.0, guid.1),
            "phase": "CREATED",
            "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
            "type": ty,
            "name": name,
            "visible": true,
            "opacity": 1.0,
            "size": {"x": 100.0, "y": 40.0},
            "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
        });
        if let (Some(dst), Some(src)) = (m.as_object_mut(), extra.as_object()) {
            for (k, v) in src {
                dst.insert(k.clone(), v.clone());
            }
        }
        m
    };

    // Document with:
    // - 2 shared styles on Internal Only Canvas (and an external component so library_page exists)
    // - A main component `Calendar` containing a nested auto-layout child `_Button group base`
    //   with `stackPrimaryAlignItems: "CENTER"` and `stackCounterAlignItems: "CENTER"`.
    // - An instance `CalendarInst` whose `symbolOverrides` overrides `stackPrimarySizing: "FIXED"`
    //   (or `stackChildPrimaryGrow: 1.0`) on `_Button group base` without overriding its alignment.
    let changes = json!([
        node((0, 0), (0, 0), "!", "DOCUMENT", "RoundtripDoc", json!({})),
        node((0, 1), (0, 0), "a", "CANVAS", "↳ Calendars", json!({})),
        node((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", json!({"internalOnly": true})),
        node((5, 1), (0, 2), "a", "SYMBOL", "LibComp", json!({"componentKey": "LIB_COMP_1", "sourceLibraryKey": "lib_ext"})),
        node(
            (7, 1),
            (0, 2),
            "b",
            "RECTANGLE",
            "Brand/Primary",
            json!({
                "styleType": "FILL",
                "key": "STYLE_FILL_1",
                "fillPaints": [{"type": "SOLID", "color": {"r": 0.1, "g": 0.2, "b": 0.8, "a": 1.0}, "opacity": 1.0, "visible": true}]
            })
        ),
        node(
            (7, 2),
            (0, 2),
            "c",
            "RECTANGLE",
            "Shadow/Sm",
            json!({
                "styleType": "EFFECT",
                "key": "STYLE_FX_1",
                "effects": [{"type": "DROP_SHADOW", "color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 0.2}, "offset": {"x": 0.0, "y": 2.0}, "radius": 4.0, "spread": 0.0, "visible": true}]
            })
        ),
        node(
            (1, 10),
            (0, 1),
            "a",
            "SYMBOL",
            "Calendar",
            json!({
                "size": {"x": 300.0, "y": 300.0},
                "stackMode": "VERTICAL",
                "stackPrimarySizing": "FIXED",
                "stackCounterSizing": "FIXED"
            })
        ),
        node(
            (1, 11),
            (1, 10),
            "a",
            "FRAME",
            "_Button group base",
            json!({
                "overrideKey": g(9, 11),
                "size": {"x": 263.0, "y": 36.0},
                "styleIdForFill": {"guid": g(7, 1)},
                "styleIdForEffect": {"guid": g(7, 2)},
                "stackMode": "HORIZONTAL",
                "stackPrimaryAlignItems": "CENTER",
                "stackCounterAlignItems": "CENTER",
                "stackPrimarySizing": "RESIZE_TO_FIT",
                "stackCounterSizing": "FIXED"
            })
        ),
        node(
            (1, 20),
            (0, 1),
            "b",
            "INSTANCE",
            "CalendarInst",
            json!({
                "size": {"x": 300.0, "y": 300.0},
                "symbolData": {
                    "symbolID": g(1, 10),
                    "symbolOverrides": [
                        {
                            "guidPath": {"guids": [g(9, 11)]},
                            "stackPrimarySizing": "FIXED",
                            "stackChildPrimaryGrow": 1.0
                        }
                    ]
                }
            })
        )
    ]);

    let doc1 = import_fig_node_changes(&changes, "RoundtripDoc").expect("import 1");
    let inst_btn_align = |doc: &Document| -> (LayoutAlign, LayoutAlign) {
        let inst = find(doc, "CalendarInst");
        let btn_id = inst.children[0];
        let btn = doc.get(btn_id).expect("_Button group base in instance");
        let al = btn.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).expect("auto_layout on _Button group base");
        (al.primary_align, al.counter_align)
    };
    assert_eq!(inst_btn_align(&doc1), (LayoutAlign::Center, LayoutAlign::Center));
    assert_eq!(doc1.styles.len(), 2);

    let bytes1 = export_fig(&doc1).expect("export 1");
    let doc2 = import_fig(&bytes1).expect("import 2");
    assert_eq!(doc2.nodes.len(), doc1.nodes.len(), "save-and-reopen cycle 1 must not add extra nodes");
    assert_eq!(inst_btn_align(&doc2), (LayoutAlign::Center, LayoutAlign::Center), "_Button group base alignment must stay Center/Center after roundtrip 1");

    let bytes2 = export_fig(&doc2).expect("export 2");
    let doc3 = import_fig(&bytes2).expect("import 3");
    assert_eq!(doc3.nodes.len(), doc1.nodes.len(), "save-and-reopen cycle 2 must not add extra nodes");
    assert_eq!(inst_btn_align(&doc3), (LayoutAlign::Center, LayoutAlign::Center), "_Button group base alignment must stay Center/Center after roundtrip 2");
}

#[test]
fn export_fig_then_import_fig_roundtrips_sample_and_rich_documents() {
    // 1. Roundtrip the canonical sample_fig fixture through export_fig -> import_fig
    let raw = sample_fig(designos_fig::container::Compression::Zstd, true);
    let doc1 = import_fig(&raw).expect("import sample_fig");
    let exported1 = export_fig(&doc1).expect("export_fig on sample_fig");
    let doc2 = import_fig(&exported1).expect("re-import exported sample_fig");

    assert_eq!(doc2.pages.len(), doc1.pages.len());
    let card = find(&doc2, "Card");
    assert_eq!(card.size, Vec2::new(200.0, 300.0));
    assert_eq!(card.corner_radii, [12.0; 4]);
    let f = card.kind.frame_props().expect("Card is a frame");
    let al = f.auto_layout.as_ref().expect("Card has auto_layout");
    assert_eq!(al.direction, LayoutDirection::Vertical);
    assert_eq!(al.padding, [12.0, 16.0, 12.0, 16.0]);

    let title = find(&doc2, "Title");
    let NodeKind::Text(tp) = &title.kind else { panic!("expected Text") };
    assert_eq!(tp.content, "Hi \u{1F600}!");
    assert_eq!(tp.runs.len(), 1);
    assert_eq!(tp.runs[0].style.font_weight, 700);
    assert!(tp.runs[0].style.italic);

    let tri = find(&doc2, "Tri");
    let NodeKind::Vector { paths, .. } = &tri.kind else { panic!("expected Vector") };
    assert_eq!(paths[0].cmds.len(), 4);

    // 2. Build a rich Document with variables, asymmetric auto-layout, wrap, individual stroke weights,
    // dev status, and embedded images.
    let mut rich = Document::new("Parity Rich Doc");
    let page_id = rich.add_page("Page 1");

    let fake_png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 1, 2, 3, 4];
    let img_hash = "0102030405060708090a0b0c0d0e0f1011121314";
    rich.images.insert(img_hash.into(), fake_png.clone());

    rich.guides.insert(page_id, vec![Guide { axis: GuideAxis::X, offset: 120.0, frame: None }]);

    // Variable collection + variables
    rich.variable_collections.insert(
        "VariableCollectionId:10:1".into(),
        VariableCollection {
            id: "VariableCollectionId:10:1".into(),
            name: "Theme".into(),
            modes: vec![VariableMode { id: "10:0".into(), name: "Light".into() }, VariableMode { id: "10:1".into(), name: "Dark".into() }],
            default_mode: "10:0".into(),
            variables: vec!["VariableID:10:10".into(), "VariableID:10:11".into(), "VariableID:10:12".into()],
            key: Some("theme_key".into()),
            source_library_key: None,
        },
    );
    let mut bg_vals = BTreeMap::new();
    bg_vals.insert("10:0".into(), VariableValue::Color(Color::rgb(240, 242, 245)));
    bg_vals.insert("10:1".into(), VariableValue::Color(Color::rgb(24, 28, 36)));
    rich.variables.insert(
        "VariableID:10:10".into(),
        Variable {
            id: "VariableID:10:10".into(),
            name: "surface/bg".into(),
            collection: "VariableCollectionId:10:1".into(),
            kind: VariableKind::Color,
            values: bg_vals,
            description: "Surface background".into(),
            scopes: vec!["ALL_FILLS".into()],
            hidden_from_publishing: false,
            key: Some("var_bg_key".into()),

            code_syntax: Default::default(),
        },
    );
    let mut pad_vals = BTreeMap::new();
    pad_vals.insert("10:0".into(), VariableValue::Number(16.0));
    pad_vals.insert("10:1".into(), VariableValue::Number(24.0));
    rich.variables.insert(
        "VariableID:10:11".into(),
        Variable {
            id: "VariableID:10:11".into(),
            name: "space/md".into(),
            collection: "VariableCollectionId:10:1".into(),
            kind: VariableKind::Number,
            values: pad_vals,
            description: String::new(),
            scopes: vec!["GAP".into()],
            hidden_from_publishing: false,
            key: Some("var_pad_key".into()),

            code_syntax: Default::default(),
        },
    );
    let mut alias_vals = BTreeMap::new();
    alias_vals.insert("10:0".into(), VariableValue::Alias("VariableID:10:10".into()));
    alias_vals.insert("10:1".into(), VariableValue::Alias("VariableID:10:10".into()));
    rich.variables.insert(
        "VariableID:10:12".into(),
        Variable {
            id: "VariableID:10:12".into(),
            name: "surface/alias".into(),
            collection: "VariableCollectionId:10:1".into(),
            kind: VariableKind::Color,
            values: alias_vals,
            description: String::new(),
            scopes: vec!["ALL_FILLS".into()],
            hidden_from_publishing: false,
            key: Some("var_alias_key".into()),

            code_syntax: Default::default(),
        },
    );

    // Component with asymmetric auto-layout, wrap, stroke_included, reverse_z, dev.status, bindings
    let comp_props = FrameProps {
        clips_content: false,
        auto_layout: Some(AutoLayout {
            direction: LayoutDirection::Horizontal,
            item_spacing: 10.0,
            padding: [8.0, 16.0, 12.0, 20.0], // top, right, bottom, left
            primary_align: LayoutAlign::Center,
            counter_align: LayoutAlign::Center,
            primary_sizing: Sizing::Fixed,
            counter_sizing: Sizing::Hug,
            wrap: true,
            counter_spacing: 14.0,
            stroke_included: true,
            reverse_z: true,
            ..AutoLayout::default()
        }),
        ..FrameProps::default()
    };
    let comp_id = rich.alloc_id();
    let mut comp_node = Node::new(comp_id, NodeKind::Component(comp_props));
    comp_node.name = "PrimaryButton".into();
    comp_node.size = Vec2::new(160.0, 48.0);
    comp_node.dev.status = Some(DevStatus::ReadyForDev);
    comp_node.variables.bindings.insert("paddingLeft".into(), "VariableID:10:11".into());
    comp_node.variables.bindings.insert("itemSpacing".into(), "VariableID:10:11".into());
    rich.insert(page_id, comp_node).unwrap();

    // Child shape inside PrimaryButton with side_weights + image fill + variable binding
    let bg_id = rich.alloc_id();
    let mut bg_rect = Node::new(bg_id, NodeKind::Rectangle);
    bg_rect.name = "BgSurface".into();
    bg_rect.size = Vec2::new(120.0, 32.0);
    bg_rect.fills = vec![
        Paint::solid(Color::rgb(240, 242, 245)),
        Paint {
            kind: PaintKind::Image { image_ref: img_hash.into(), scale_mode: ImageScaleMode::Fill },
            opacity: 0.9,
            visible: true,
            blend: BlendMode::Normal,
        },
    ];
    bg_rect.stroke = Stroke {
        paints: vec![Paint::solid(Color::rgb(50, 75, 100))],
        weight: 2.0,
        align: StrokeAlign::Inside,
        cap: StrokeCap::Round,
        join: StrokeJoin::Bevel,
        dashes: Vec::new(),
        miter_limit: 4.0,
        side_weights: Some([1.0, 2.0, 3.0, 4.0]),
    };
    bg_rect.layout_child = LayoutChild { horizontal: Sizing::Fill, vertical: Sizing::Fill, grow: 1.0, ..LayoutChild::default() };
    bg_rect.variables.bindings.insert("fills/0".into(), "VariableID:10:10".into());
    rich.insert(comp_id, bg_rect).unwrap();

    // Export and re-import
    let rich_bytes = export_fig(&rich).expect("export_fig rich doc");
    let rich_imp = import_fig(&rich_bytes).expect("import_fig rich doc");

    assert_eq!(rich_imp.images.get(img_hash), Some(&fake_png));
    assert_eq!(rich_imp.variable_collections.len(), 1);
    assert_eq!(rich_imp.variables.len(), 3);

    let imp_comp = find(&rich_imp, "PrimaryButton");
    assert_eq!(imp_comp.dev.status, Some(DevStatus::ReadyForDev));
    let imp_al = imp_comp.kind.frame_props().unwrap().auto_layout.as_ref().expect("auto_layout");
    assert_eq!(imp_al.direction, LayoutDirection::Horizontal);
    assert_eq!(imp_al.padding, [8.0, 16.0, 12.0, 20.0]);
    assert_eq!(imp_al.item_spacing, 10.0);
    assert!(imp_al.wrap);
    assert_eq!(imp_al.counter_spacing, 14.0);
    assert!(imp_al.stroke_included);
    assert!(imp_al.reverse_z);
    assert!(imp_comp.variables.bindings.contains_key("paddingLeft"));
    assert!(imp_comp.variables.bindings.contains_key("itemSpacing"));

    let imp_bg = find(&rich_imp, "BgSurface");
    assert_eq!(imp_bg.stroke.side_weights, Some([1.0, 2.0, 3.0, 4.0]));
    assert_eq!(imp_bg.layout_child.horizontal, Sizing::Fill);
    assert_eq!(imp_bg.layout_child.vertical, Sizing::Fill);
    assert_eq!(imp_bg.layout_child.grow, 1.0);
    assert!(imp_bg.variables.bindings.contains_key("fills/0"));
}

#[test]
fn fig_import_resolves_symbol_overrides_bindings_nested_swaps_and_handoff_map() {
    let g = |s: u64, l: u64| -> Value { json!({"sessionID": s, "localID": l}) };
    let node = |id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str, extra: Value| -> Value {
        let mut v = json!({
            "guid": g(id.0, id.1),
            "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
            "type": ty,
            "name": name,
            "phase": "CREATED",
            "visible": true,
            "opacity": 1.0,
            "size": {"x": 24.0, "y": 24.0},
            "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
        });
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        v
    };

    let changes = json!([
        node((0, 0), (0, 0), "!", "DOCUMENT", "Document", json!({})),
        node(
            (1, 10),
            (0, 0),
            "a",
            "VARIABLE_SET",
            "Tokens",
            json!({
                "variableSetModes": [
                    {"id": g(1, 100), "name": "Default", "sortPosition": "a"}
                ]
            })
        ),
        node(
            (1, 11),
            (1, 10),
            "a",
            "VARIABLE",
            "color/primary",
            json!({
                "variableSetID": g(1, 10),
                "variableResolvedType": "COLOR",
                "variableDataValues": {
                    "entries": [{
                        "modeID": g(1, 100),
                        "variableData": {
                            "dataType": "COLOR",
                            "value": {"colorValue": {"r": 0.1, "g": 0.3, "b": 0.9, "a": 1.0}}
                        }
                    }]
                }
            })
        ),
        node(
            (1, 12),
            (1, 10),
            "b",
            "VARIABLE",
            "color/accent",
            json!({
                "variableSetID": g(1, 10),
                "variableResolvedType": "COLOR",
                "variableDataValues": {
                    "entries": [{
                        "modeID": g(1, 100),
                        "variableData": {
                            "dataType": "COLOR",
                            "value": {"colorValue": {"r": 0.95, "g": 0.35, "b": 0.25, "a": 1.0}}
                        }
                    }]
                }
            })
        ),
        node(
            (1, 13),
            (1, 10),
            "c",
            "VARIABLE",
            "space/lg",
            json!({
                "variableSetID": g(1, 10),
                "variableResolvedType": "FLOAT",
                "variableDataValues": {
                    "entries": [{
                        "modeID": g(1, 100),
                        "variableData": {
                            "dataType": "FLOAT",
                            "value": {"floatValue": 24.0}
                        }
                    }]
                }
            })
        ),
        node(
            (1, 14),
            (1, 10),
            "d",
            "VARIABLE",
            "font/style",
            json!({
                "variableSetID": g(1, 10),
                "variableResolvedType": "STRING",
                "variableDataValues": {
                    "entries": [{
                        "modeID": g(1, 100),
                        "variableData": {
                            "dataType": "STRING",
                            "value": {"textValue": "Bold"}
                        }
                    }]
                }
            })
        ),
        node(
            (1, 2),
            (0, 0),
            "b",
            "CANVAS",
            "Page 1",
            json!({
                "handoffStatusMap": {
                    "entries": [
                        {"guid": g(1, 50), "handoffStatus": "READY_FOR_DEV"}
                    ]
                }
            })
        ),
        // IconA (SYMBOL) with child StarShape
        node((1, 20), (1, 2), "a", "SYMBOL", "IconA", json!({})),
        node((1, 21), (1, 20), "a", "STAR", "StarShape", json!({})),
        // IconB (SYMBOL) with child CircleShape
        node((1, 30), (1, 2), "b", "SYMBOL", "IconB", json!({})),
        node((1, 31), (1, 30), "a", "ELLIPSE", "CircleShape", json!({})),
        // Card (SYMBOL) with nested IconSlot (instance of IconA) and Surface (bound to color/primary)
        node(
            (1, 40),
            (1, 2),
            "c",
            "SYMBOL",
            "Card",
            json!({
                "size": {"x": 200.0, "y": 100.0},
                "stackMode": "HORIZONTAL",
                "stackSpacing": 8.0
            })
        ),
        node(
            (1, 41),
            (1, 40),
            "a",
            "INSTANCE",
            "IconSlot",
            json!({
                "symbolData": {"symbolID": g(1, 20)}
            })
        ),
        node(
            (1, 42),
            (1, 40),
            "b",
            "ROUNDED_RECTANGLE",
            "Surface",
            json!({
                "size": {"x": 160.0, "y": 80.0},
                "fillPaints": [{
                    "type": "SOLID",
                    "color": {"r": 0.1, "g": 0.3, "b": 0.9, "a": 1.0},
                    "opacity": 1.0,
                    "visible": true,
                    "colorVar": {
                        "dataType": "ALIAS",
                        "value": {"alias": {"guid": g(1, 11)}}
                    }
                }],
                "strokePaints": [{
                    "type": "SOLID",
                    "color": {"r": 0.1, "g": 0.3, "b": 0.9, "a": 1.0},
                    "opacity": 1.0,
                    "visible": true,
                    "colorVar": {
                        "dataType": "ALIAS",
                        "value": {"alias": {"guid": g(1, 11)}}
                    }
                }],
                "strokeWeight": 1.0
            })
        ),
        // CardInstance (INSTANCE of Card)
        node(
            (1, 50),
            (1, 2),
            "d",
            "INSTANCE",
            "CardInstance",
            json!({
                "size": {"x": 240.0, "y": 120.0},
                "stackMode": "HORIZONTAL",
                "parameterConsumptionMap": {
                    "entries": [
                        {
                            "variableField": "STACK_HORIZONTAL_PADDING",
                            "variableData": {
                                "dataType": "ALIAS",
                                "value": {"alias": {"guid": g(1, 13)}}
                            }
                        },
                        {
                            "variableField": "FONT_STYLE",
                            "variableData": {
                                "dataType": "FONT_STYLE",
                                "value": {
                                    "fontStyleValue": {
                                        "asString": {
                                            "dataType": "ALIAS",
                                            "value": {"alias": {"guid": g(1, 14)}}
                                        }
                                    }
                                }
                            }
                        }
                    ]
                },
                "symbolData": {
                    "symbolID": g(1, 40),
                    "symbolOverrides": [
                        {
                            "guidPath": {"guids": [g(1, 42)]},
                            "fillPaints": [{
                                "type": "SOLID",
                                "color": {"r": 1.0, "g": 1.0, "b": 1.0, "a": 1.0},
                                "opacity": 1.0,
                                "visible": true
                            }],
                            "strokePaints": [{
                                "type": "SOLID",
                                "color": {"r": 0.95, "g": 0.35, "b": 0.25, "a": 1.0},
                                "opacity": 1.0,
                                "visible": true,
                                "colorVar": {
                                    "dataType": "ALIAS",
                                    "value": {"alias": {"guid": g(1, 12)}}
                                }
                            }],
                            "borderStrokeWeightsIndependent": true,
                            "borderTopWeight": 2.0,
                            "borderRightWeight": 4.0,
                            "borderBottomWeight": 6.0,
                            "borderLeftWeight": 8.0,
                            "stackChildPrimaryGrow": 1.0,
                            "stackChildAlignSelf": "STRETCH"
                        },
                        {
                            "guidPath": {"guids": [g(1, 41)]},
                            "overriddenSymbolID": g(1, 30)
                        }
                    ]
                }
            })
        )
    ]);

    let doc = import_fig_node_changes(&changes, "Override Fidelity Doc").expect("import_fig_node_changes");

    let card_inst = find(&doc, "CardInstance");

    // 1. CANVAS.handoffStatusMap resolved READY_FOR_DEV on CardInstance
    assert_eq!(card_inst.dev.status, Some(DevStatus::ReadyForDev));

    // 2. parameterConsumptionMap resolved STACK_HORIZONTAL_PADDING and FONT_STYLE
    assert_eq!(card_inst.variables.bindings.get("paddingLeft").map(String::as_str), Some("VariableID:1:13"));
    assert_eq!(card_inst.variables.bindings.get("paddingRight").map(String::as_str), Some("VariableID:1:13"));
    assert_eq!(card_inst.variables.bindings.get("fontWeight").map(String::as_str), Some("VariableID:1:14"));

    // 3. Surface inside CardInstance:
    // - literal fillPaints override cleared master's fills/0 variable binding
    // - bound strokePaints override updated strokes/0 binding to VariableID:1:12
    // - independent border weights and stackChild overrides applied
    let inst_surface = card_inst.children.iter().filter_map(|id| doc.get(*id)).find(|n| n.name == "Surface").expect("Surface inside CardInstance");
    assert!(
        !inst_surface.variables.bindings.contains_key("fills/0"),
        "stale fills/0 binding from master component must be cleared when fillPaints is overridden with literal color"
    );
    assert_eq!(inst_surface.variables.bindings.get("strokes/0").map(String::as_str), Some("VariableID:1:12"));
    assert_eq!(inst_surface.stroke.side_weights, Some([2.0, 4.0, 6.0, 8.0]));
    assert_eq!(inst_surface.layout_child.grow, 1.0);
    assert_eq!(inst_surface.layout_child.horizontal, Sizing::Fill);
    assert_eq!(inst_surface.layout_child.vertical, Sizing::Fill);

    // 4. IconSlot inside CardInstance: overriddenSymbolID swapped it from IconA (StarShape) to IconB (CircleShape)
    let inst_icon_slot = card_inst.children.iter().filter_map(|id| doc.get(*id)).find(|n| n.name == "IconSlot").expect("IconSlot inside CardInstance");
    let icon_child_names: Vec<&str> = inst_icon_slot.children.iter().filter_map(|id| doc.get(*id)).map(|n| n.name.as_str()).collect();
    assert_eq!(icon_child_names, vec!["CircleShape"]);
}

#[test]
fn roundtrip_figma_parity_grid_slots_axes_props_and_overrides() {
    let mut doc = Document::new("FigmaParity100");
    let page_id = doc.add_page("Page 1");

    // 1. 2D CSS Grid Frame with explicit grid_columns, grid_rows, and child col_span/row_span/anchor
    let grid_id = doc.alloc_id();
    let grid_props = FrameProps {
        auto_layout: Some(AutoLayout {
            direction: LayoutDirection::Grid,
            item_spacing: 12.0,
            counter_spacing: 8.0,
            padding: [16.0, 16.0, 16.0, 16.0],
            grid_columns: 3,
            grid_rows: 2,
            ..AutoLayout::default()
        }),
        ..FrameProps::default()
    };
    let mut grid_frame = Node::new(grid_id, NodeKind::Frame(grid_props));
    grid_frame.name = "BentoGrid".into();
    grid_frame.size = Vec2::new(600.0, 400.0);
    doc.insert(page_id, grid_frame).unwrap();

    let cell_id = doc.alloc_id();
    let mut cell_node = Node::new(cell_id, NodeKind::Rectangle);
    cell_node.name = "HeroCell".into();
    cell_node.size = Vec2::new(380.0, 180.0);
    cell_node.layout_child.col_span = 2;
    cell_node.layout_child.row_span = 2;
    cell_node.layout_child.grid_column = Some(1);
    cell_node.layout_child.grid_row = Some(0);
    doc.insert(grid_id, cell_node).unwrap();

    // 2. Text node with OpenType features (on + off) and variable font axes
    let txt_id = doc.alloc_id();
    let mut style = TextStyle { font_family: "Inter Variable".into(), font_size: 24.0, font_weight: 650, ..TextStyle::default() };
    style.set_feature("tnum", true);
    style.set_feature("ss01", true);
    style.set_feature("liga", false);
    style.set_axis("wght", 650.0);
    style.set_axis("wdth", 112.5);
    style.set_axis("opsz", 24.0);
    let mut txt_node = Node::new(txt_id, NodeKind::Text(TextProps { content: "Variable Axes & OT".into(), style, ..TextProps::default() }));
    txt_node.name = "VarFontLabel".into();
    txt_node.size = Vec2::new(220.0, 32.0);
    doc.insert(page_id, txt_node).unwrap();

    // 3. Component with Boolean, Text, InstanceSwap, Slot props + bindings, and an Instance with overrides + prop values
    let icon_a_id = doc.alloc_id();
    let mut icon_a = Node::new(icon_a_id, NodeKind::Component(FrameProps::default()));
    icon_a.name = "IconStar".into();
    icon_a.size = Vec2::new(16.0, 16.0);
    doc.insert(page_id, icon_a).unwrap();

    let icon_b_id = doc.alloc_id();
    let mut icon_b = Node::new(icon_b_id, NodeKind::Component(FrameProps::default()));
    icon_b.name = "IconHeart".into();
    icon_b.size = Vec2::new(16.0, 16.0);
    doc.insert(page_id, icon_b).unwrap();

    let card_comp_id = doc.alloc_id();
    let mut card_comp = Node::new(card_comp_id, NodeKind::Component(FrameProps::default()));
    card_comp.name = "ModalCard".into();
    card_comp.size = Vec2::new(320.0, 200.0);
    card_comp.component.props = vec![
        ComponentPropDef { name: "ShowFooter#1:0".into(), kind: PropKind::Boolean, default: PropValue::Bool(true) },
        ComponentPropDef { name: "Title#1:1".into(), kind: PropKind::Text, default: PropValue::Text("Default Title".into()) },
        ComponentPropDef {
            name: "HeaderIcon#1:2".into(),
            kind: PropKind::InstanceSwap { preferred: vec![icon_a_id, icon_b_id] },
            default: PropValue::Instance(icon_a_id),
        },
        ComponentPropDef { name: "BodySlot#1:3".into(), kind: PropKind::Slot, default: PropValue::Slot(vec![]) },
    ];
    doc.insert(page_id, card_comp).unwrap();

    let title_id = doc.alloc_id();
    let mut title_node =
        Node::new(title_id, NodeKind::Text(TextProps { content: "Default Title".into(), style: TextStyle::default(), ..TextProps::default() }));
    title_node.name = "CardTitle".into();
    title_node.size = Vec2::new(200.0, 24.0);
    title_node.component.bindings.insert(OverrideField::Characters, "Title#1:1".into());
    doc.insert(card_comp_id, title_node).unwrap();

    let slot_frame_id = doc.alloc_id();
    let mut slot_frame = Node::new(slot_frame_id, NodeKind::Frame(FrameProps::default()));
    slot_frame.name = "BodyContainer".into();
    slot_frame.size = Vec2::new(280.0, 120.0);
    slot_frame.component.bindings.insert(OverrideField::SlotContent, "BodySlot#1:3".into());
    doc.insert(card_comp_id, slot_frame).unwrap();

    let footer_id = doc.alloc_id();
    let mut footer_rect = Node::new(footer_id, NodeKind::Rectangle);
    footer_rect.name = "FooterBg".into();
    footer_rect.size = Vec2::new(280.0, 36.0);
    footer_rect.fills = vec![Paint::solid(Color::rgb(230, 230, 230))];
    footer_rect.component.bindings.insert(OverrideField::Visible, "ShowFooter#1:0".into());
    doc.insert(card_comp_id, footer_rect).unwrap();

    // Create instance of ModalCard with overrides and prop assignments
    create_instance(&mut doc, card_comp_id, page_id, Vec2::new(400.0, 0.0)).apply(&mut doc).expect("create_instance");
    let inst_id = instances_of(&doc, card_comp_id)[0];

    let custom_slot_child_id = doc.alloc_id();
    let mut custom_slot_child = Node::new(custom_slot_child_id, NodeKind::Rectangle);
    custom_slot_child.name = "InjectedBanner".into();
    custom_slot_child.size = Vec2::new(260.0, 60.0);
    doc.insert(page_id, custom_slot_child).unwrap();

    let inst_children = doc.get(inst_id).unwrap().children.clone();
    if let Some(inst) = doc.get_mut(inst_id) {
        inst.name = "ModalCardInstance".into();
        inst.component.values.insert("ShowFooter#1:0".into(), PropValue::Bool(false));
        inst.component.values.insert("Title#1:1".into(), PropValue::Text("Assigned Title".into()));
        inst.component.values.insert("HeaderIcon#1:2".into(), PropValue::Instance(icon_b_id));
        inst.component.values.insert("BodySlot#1:3".into(), PropValue::Slot(vec![custom_slot_child_id]));
    }

    for cid in inst_children {
        if let Some(ch) = doc.get_mut(cid) {
            if ch.name == "CardTitle" {
                if let NodeKind::Text(ref mut tp) = ch.kind {
                    tp.content = "Overridden Modal Title".into();
                }
                ch.component.overrides.insert(OverrideField::Characters);
            } else if ch.name == "FooterBg" {
                ch.visible = false;
                ch.fills = vec![Paint::solid(Color::rgb(255, 64, 128))];
                ch.component.overrides.insert(OverrideField::Visible);
                ch.component.overrides.insert(OverrideField::Fills);
            }
        }
    }

    // Export -> Import round-trip
    let bytes = export_fig(&doc).expect("export_fig");
    let rt = import_fig(&bytes).expect("import_fig");

    // Verify 1: 2D CSS Grid
    let rt_grid = find(&rt, "BentoGrid");
    let NodeKind::Frame(ref g_fp) = rt_grid.kind else {
        panic!("expected BentoGrid Frame");
    };
    let g_al = g_fp.auto_layout.as_ref().expect("BentoGrid auto_layout");
    assert_eq!(g_al.direction, LayoutDirection::Grid);
    assert_eq!(g_al.grid_columns, 3);
    assert_eq!(g_al.grid_rows, 2);
    assert_eq!(g_al.item_spacing, 12.0);
    assert_eq!(g_al.counter_spacing, 8.0);

    let rt_cell = find(&rt, "HeroCell");
    assert_eq!(rt_cell.layout_child.col_span, 2);
    assert_eq!(rt_cell.layout_child.row_span, 2);
    assert_eq!(rt_cell.layout_child.grid_column, Some(1));
    assert_eq!(rt_cell.layout_child.grid_row, Some(0));

    // Verify 2: TextStyle OpenType features & variable font axes
    let rt_txt = find(&rt, "VarFontLabel");
    let NodeKind::Text(ref tp) = rt_txt.kind else {
        panic!("expected VarFontLabel Text");
    };
    assert!(tp.style.is_feature_enabled("tnum"));
    assert!(tp.style.is_feature_enabled("ss01"));
    assert!(!tp.style.is_feature_enabled("liga"));
    assert_eq!(tp.style.features.iter().find(|(k, _)| k.eq_ignore_ascii_case("liga")).map(|(_, v)| *v), Some(0));
    assert_eq!(tp.style.get_axis("wght"), Some(650.0));
    assert_eq!(tp.style.get_axis("wdth"), Some(112.5));
    assert_eq!(tp.style.get_axis("opsz"), Some(24.0));

    // Verify 3: Component props & bindings (including Slot)
    let rt_comp = find(&rt, "ModalCard");
    assert_eq!(rt_comp.component.props.len(), 4);
    assert!(rt_comp.component.props.iter().any(|p| p.name == "BodySlot" && p.kind == PropKind::Slot));
    let rt_icon_b = find(&rt, "IconHeart");
    let swap_def = rt_comp.component.props.iter().find(|p| p.name == "HeaderIcon").expect("HeaderIcon prop def");
    match &swap_def.kind {
        PropKind::InstanceSwap { preferred } => {
            assert!(preferred.contains(&rt_icon_b.id));
        }
        other => panic!("expected InstanceSwap prop kind, got {other:?}"),
    }

    let rt_slot_container = rt_comp.children.iter().filter_map(|id| rt.get(*id)).find(|n| n.name == "BodyContainer").expect("BodyContainer inside ModalCard");
    assert_eq!(rt_slot_container.component.bindings.get(&OverrideField::SlotContent).map(String::as_str), Some("BodySlot"));
    assert!(is_slot_container(&rt, rt_slot_container.id));

    // Verify 4: Instance prop values & materialized symbolOverrides
    let rt_inst = find(&rt, "ModalCardInstance");
    assert_eq!(rt_inst.component.values.get("ShowFooter"), Some(&PropValue::Bool(false)));
    assert_eq!(rt_inst.component.values.get("Title"), Some(&PropValue::Text("Assigned Title".into())));
    assert_eq!(rt_inst.component.values.get("HeaderIcon"), Some(&PropValue::Instance(rt_icon_b.id)));
    let rt_banner = find(&rt, "InjectedBanner");
    assert_eq!(rt_inst.component.values.get("BodySlot"), Some(&PropValue::Slot(vec![rt_banner.id])));

    let inst_title = rt_inst.children.iter().filter_map(|id| rt.get(*id)).find(|n| n.name == "CardTitle").expect("CardTitle in ModalCardInstance");
    let NodeKind::Text(ref inst_tp) = inst_title.kind else {
        panic!("expected CardTitle Text");
    };
    assert_eq!(inst_tp.content, "Overridden Modal Title");

    let inst_footer = rt_inst.children.iter().filter_map(|id| rt.get(*id)).find(|n| n.name == "FooterBg").expect("FooterBg in ModalCardInstance");
    assert!(!inst_footer.visible);
    assert_eq!(inst_footer.fills, vec![Paint::solid(Color::rgb(255, 64, 128))]);
}

#[test]
fn rea_v106_all_seven_gaps_fidelity_and_roundtrip() {
    let g = |s: u64, l: u64| -> Value { json!({"sessionID": s, "localID": l}) };
    let node = |id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str, extra: Value| -> Value {
        let mut v = json!({
            "guid": g(id.0, id.1),
            "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
            "type": ty,
            "name": name,
            "phase": "CREATED",
            "visible": true,
            "opacity": 1.0,
            "size": {"x": 100.0, "y": 50.0},
            "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
        });
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        v
    };
    let var_alias = |field: &str, s: u64, l: u64| -> Value {
        json!({
            "variableField": field,
            "variableData": {
                "dataType": "ALIAS",
                "value": {"alias": {"guid": g(s, l)}}
            }
        })
    };

    let changes = json!([
        node((0, 0), (0, 0), "!", "DOCUMENT", "REA_v106", json!({})),
        // Active variable set + variables for GAP-03
        node(
            (2, 1),
            (0, 0),
            "a",
            "VARIABLE_SET",
            "Semantic",
            json!({
                "variableSetModes": [
                    {"id": g(2, 10), "name": "Light", "sortPosition": "a"}
                ]
            })
        ),
        node(
            (2, 11),
            (2, 1),
            "a",
            "VARIABLE",
            "vis/showBadge",
            json!({
                "variableSetID": g(2, 1),
                "variableResolvedType": "BOOLEAN",
                "variableDataValues": {
                    "entries": [{
                        "modeID": g(2, 10),
                        "variableData": {"dataType": "BOOLEAN", "value": {"boolValue": false}}
                    }]
                }
            })
        ),
        node(
            (2, 12),
            (2, 1),
            "b",
            "VARIABLE",
            "copy/title",
            json!({
                "variableSetID": g(2, 1),
                "variableResolvedType": "STRING",
                "variableDataValues": {
                    "entries": [{
                        "modeID": g(2, 10),
                        "variableData": {"dataType": "STRING", "value": {"textValue": "Bound Headline"}}
                    }]
                }
            })
        ),
        node(
            (2, 13),
            (2, 1),
            "c",
            "VARIABLE",
            "border/top",
            json!({
                "variableSetID": g(2, 1),
                "variableResolvedType": "FLOAT",
                "variableDataValues": {
                    "entries": [{
                        "modeID": g(2, 10),
                        "variableData": {"dataType": "FLOAT", "value": {"floatValue": 3.0}}
                    }]
                }
            })
        ),
        // GAP-07: Soft-deleted variable set and variable must be ignored
        node(
            (2, 99),
            (0, 0),
            "z",
            "VARIABLE_SET",
            "DeletedSet",
            json!({
                "isSoftDeleted": true,
                "variableSetModes": [{"id": g(2, 990), "name": "Default", "sortPosition": "a"}]
            })
        ),
        node(
            (2, 98),
            (2, 1),
            "z",
            "VARIABLE",
            "deleted/token",
            json!({
                "isSoftDeleted": true,
                "variableSetID": g(2, 1),
                "variableResolvedType": "FLOAT",
                "variableDataValues": {"entries": []}
            })
        ),
        node((1, 1), (0, 0), "b", "CANVAS", "Page 1", json!({})),
        // GAP-07: Soft-deleted canvas node must be ignored
        node(
            (1, 999),
            (1, 1),
            "z",
            "FRAME",
            "GhostDeletedFrame",
            json!({
                "isSoftDeleted": true
            })
        ),
        // GAP-01 & GAP-02 & GAP-03: Frame with independent per-side border weights
        // and per-corner radii WITHOUT rectangleCornerRadiiIndependent, plus variableConsumptionMap
        node(
            (1, 10),
            (1, 1),
            "a",
            "FRAME",
            "HeaderBar",
            json!({
                "strokeWeight": 1.0,
                "borderStrokeWeightsIndependent": true,
                "borderBottomWeight": 2.0,
                "rectangleTopLeftCornerRadius": 10.0,
                "rectangleTopRightCornerRadius": 10.0,
                "rectangleBottomRightCornerRadius": 0.0,
                "rectangleBottomLeftCornerRadius": 0.0,
                "variableConsumptionMap": {
                    "entries": [
                        var_alias("VISIBLE", 2, 11),
                        var_alias("BORDER_TOP_WEIGHT", 2, 13),
                        var_alias("BORDER_RIGHT_WEIGHT", 2, 13),
                        var_alias("BORDER_BOTTOM_WEIGHT", 2, 13),
                        var_alias("BORDER_LEFT_WEIGHT", 2, 13)
                    ]
                }
            })
        ),
        node(
            (1, 11),
            (1, 10),
            "a",
            "TEXT",
            "HeaderText",
            json!({
                "textData": {"characters": "Placeholder"},
                "variableConsumptionMap": {
                    "entries": [
                        var_alias("TEXT_DATA", 2, 12)
                    ]
                }
            })
        ),
        // GAP-04: v106 CSS Grid Frame using gridColumnGap, gridRowGap, gridColumnsSizing, gridRowsSizing,
        // and child using gridColumnAnchor / gridRowAnchor
        node(
            (1, 20),
            (1, 1),
            "b",
            "FRAME",
            "V106Grid",
            json!({
                "stackMode": "GRID",
                "gridColumnGap": 16.0,
                "gridRowGap": 24.0,
                "gridColumnsSizing": "1fr 2fr 120px auto",
                "gridRowsSizing": "repeat(2, 80px)",
                "variableConsumptionMap": {
                    "entries": [
                        var_alias("GRID_COLUMN_GAP", 2, 13),
                        var_alias("GRID_ROW_GAP", 2, 13)
                    ]
                }
            })
        ),
        node(
            (1, 21),
            (1, 20),
            "a",
            "ROUNDED_RECTANGLE",
            "V106GridCell",
            json!({
                "gridColumnAnchor": 2,
                "gridRowAnchor": 1,
                "gridColumnSpan": 2,
                "gridRowSpan": 1
            })
        ),
        // GAP-05 & GAP-06: Master component with auto-layout Hug & Fill child, plus Instance with
        // symbolOverrides (sizing overrides) and derivedSymbolData (size & transform via master guid)
        node(
            (1, 30),
            (1, 1),
            "c",
            "SYMBOL",
            "ButtonComp",
            json!({
                "size": {"x": 120.0, "y": 40.0},
                "stackMode": "HORIZONTAL",
                "stackPrimarySizing": "RESIZE_TO_FIT",
                "stackCounterSizing": "RESIZE_TO_FIT_WITH_IMPLICIT_SIZE"
            })
        ),
        node(
            (1, 31),
            (1, 30),
            "a",
            "ROUNDED_RECTANGLE",
            "BtnBg",
            json!({
                "overrideKey": g(9, 31),
                "size": {"x": 100.0, "y": 32.0},
                "stackChildPrimaryGrow": 1.0,
                "stackChildAlignSelf": "STRETCH"
            })
        ),
        node(
            (1, 32),
            (1, 1),
            "d",
            "INSTANCE",
            "ButtonInst",
            json!({
                "size": {"x": 240.0, "y": 48.0},
                "stackMode": "HORIZONTAL",
                "stackPrimarySizing": "FIXED",
                "stackCounterSizing": "FIXED",
                "symbolData": {
                    "symbolID": g(1, 30),
                    "symbolOverrides": [
                        {
                            "guidPath": {"guids": [g(9, 31)]},
                            "stackChildPrimaryGrow": 0.0,
                            "stackChildAlignSelf": "AUTO",
                            "variableConsumptionMap": {
                                "entries": [
                                    var_alias("VISIBLE", 2, 11)
                                ]
                            }
                        }
                    ]
                },
                "derivedSymbolData": [
                    {
                        // Uses master node GUID (1, 31) rather than overrideKey (9, 31)!
                        "guidPath": {"guids": [g(1, 31)]},
                        "size": {"x": 220.0, "y": 44.0},
                        "transform": {"m00": 1.0, "m01": 0.0, "m02": 10.0, "m10": 0.0, "m11": 1.0, "m12": 2.0}
                    }
                ]
            })
        ),
        // GAP-07: Section with sectionStatusInfo, Sticky, ShapeWithText, and Connector linking two nodes
        node(
            (1, 40),
            (1, 1),
            "e",
            "SECTION",
            "DevSection",
            json!({
                "sectionStatusInfo": {"status": "READY_FOR_DEV"}
            })
        ),
        node(
            (1, 41),
            (1, 1),
            "f",
            "STICKY",
            "NoteA",
            json!({
                "authorVisible": true,
                "textData": {"characters": "Sticky content"}
            })
        ),
        node(
            (1, 42),
            (1, 1),
            "g",
            "SHAPE_WITH_TEXT",
            "DecisionNode",
            json!({
                "shapeWithTextType": "DIAMOND",
                "textData": {"characters": "Approve?"}
            })
        ),
        node(
            (1, 43),
            (1, 1),
            "h",
            "CONNECTOR",
            "FlowConnector",
            json!({
                "connectorLineStyle": "CURVED",
                "connectorStartStrokeCap": "NONE",
                "connectorEndStrokeCap": "ARROW_EQUILATERAL",
                "connectorStart": {
                    "endpointNodeID": g(1, 41),
                    "position": {"x": 100.0, "y": 25.0},
                    "magnet": "RIGHT"
                },
                "connectorEnd": {
                    "endpointNodeID": g(1, 42),
                    "position": {"x": 200.0, "y": 25.0},
                    "magnet": "LEFT"
                },
                "textData": {"characters": "Yes"}
            })
        )
    ]);

    let doc = import_fig_node_changes(&changes, "REA_v106").expect("import_fig_node_changes");

    // Verify GAP-07: soft-deleted nodes and variables are filtered out
    assert!(doc.nodes.values().all(|n| n.name != "GhostDeletedFrame"));
    assert!(doc.variable_collections.values().all(|c| c.name != "DeletedSet"));
    assert!(doc.variables.values().all(|v| v.name != "deleted/token"));

    // Verify GAP-01 & GAP-02 & GAP-03 on HeaderBar and HeaderText
    let header = find(&doc, "HeaderBar");
    assert_eq!(header.stroke.side_weights, Some([0.0, 0.0, 2.0, 0.0]));
    assert_eq!(header.corner_radii, [10.0, 10.0, 0.0, 0.0]);
    assert_eq!(header.variables.bindings.get("visible").map(String::as_str), Some("VariableID:2:11"));
    assert_eq!(header.variables.bindings.get("strokeTopWeight").map(String::as_str), Some("VariableID:2:13"));
    assert_eq!(header.variables.bindings.get("strokeBottomWeight").map(String::as_str), Some("VariableID:2:13"));

    let header_txt = find(&doc, "HeaderText");
    assert_eq!(header_txt.variables.bindings.get("characters").map(String::as_str), Some("VariableID:2:12"));

    // Verify GAP-04 on V106Grid and V106GridCell
    let grid = find(&doc, "V106Grid");
    let grid_al = grid.kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
    assert_eq!(grid_al.direction, LayoutDirection::Grid);
    assert_eq!(grid_al.item_spacing, 16.0);
    assert_eq!(grid_al.counter_spacing, 24.0);
    assert_eq!(grid_al.grid_columns, 4);
    assert_eq!(grid_al.grid_rows, 2);
    assert_eq!(&grid_al.grid_col_tracks[..4], &[GridTrack::Flex(1.0), GridTrack::Flex(2.0), GridTrack::Fixed(120.0), GridTrack::Hug]);
    assert_eq!(&grid_al.grid_row_tracks[..2], &[GridTrack::Fixed(80.0), GridTrack::Fixed(80.0)]);
    assert_eq!(grid.variables.bindings.get("itemSpacing").map(String::as_str), Some("VariableID:2:13"));
    assert_eq!(grid.variables.bindings.get("counterAxisSpacing").map(String::as_str), Some("VariableID:2:13"));

    let cell = find(&doc, "V106GridCell");
    assert_eq!(cell.layout_child.grid_column, Some(2));
    assert_eq!(cell.layout_child.grid_row, Some(1));
    assert_eq!(cell.layout_child.col_span, 2);
    assert_eq!(cell.layout_child.row_span, 1);

    // Verify GAP-05 & GAP-06 on ButtonInst and its cloned child BtnBg
    let btn_inst = find(&doc, "ButtonInst");
    let btn_inst_al = btn_inst.kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
    assert_eq!(btn_inst_al.primary_sizing, Sizing::Fixed);
    assert_eq!(btn_inst_al.counter_sizing, Sizing::Fixed);
    let inst_bg = btn_inst.children.iter().filter_map(|id| doc.get(*id)).find(|n| n.name == "BtnBg").expect("BtnBg in ButtonInst");
    assert_eq!(inst_bg.layout_child.grow, 0.0);
    assert_eq!(inst_bg.layout_child.horizontal, Sizing::Fixed);
    assert_eq!(inst_bg.layout_child.vertical, Sizing::Fixed);
    assert_eq!(inst_bg.size, Vec2::new(220.0, 44.0));
    assert_eq!(inst_bg.transform.tx, 10.0);
    assert_eq!(inst_bg.transform.ty, 2.0);
    assert_eq!(inst_bg.variables.bindings.get("visible").map(String::as_str), Some("VariableID:2:11"));

    // Verify GAP-07: Section devStatus, Sticky, ShapeWithText, Connector
    let sec = find(&doc, "DevSection");
    assert_eq!(sec.dev.status, Some(DevStatus::ReadyForDev));

    let sticky = find(&doc, "NoteA");
    match &sticky.kind {
        NodeKind::Sticky(sp) => {
            assert!(sp.show_author);
            assert_eq!(sp.text.content, "Sticky content");
        }
        other => panic!("expected Sticky, got {other:?}"),
    }

    let shape = find(&doc, "DecisionNode");
    match &shape.kind {
        NodeKind::ShapeWithText(sp) => {
            assert_eq!(sp.shape, ShapeKind::Diamond);
            assert_eq!(sp.text.content, "Approve?");
        }
        other => panic!("expected ShapeWithText, got {other:?}"),
    }

    let conn = find(&doc, "FlowConnector");
    match &conn.kind {
        NodeKind::Connector(cp) => {
            assert_eq!(cp.line, ConnectorLine::Curved);
            assert_eq!(cp.start.node, Some(sticky.id));
            assert_eq!(cp.end.node, Some(shape.id));
            assert_eq!(cp.end_cap, ConnectorCap::Triangle);
            assert_eq!(cp.label.as_ref().map(|l| l.content.as_str()), Some("Yes"));
        }
        other => panic!("expected Connector, got {other:?}"),
    }

    // Verify full binary export_fig -> import_fig round-trip preserves all new fields
    let bytes = export_fig(&doc).expect("export_fig");
    let rt = import_fig(&bytes).expect("import_fig");
    let rt_grid = find(&rt, "V106Grid");
    let rt_al = rt_grid.kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
    assert_eq!(&rt_al.grid_col_tracks[..4], &[GridTrack::Flex(1.0), GridTrack::Flex(2.0), GridTrack::Fixed(120.0), GridTrack::Hug]);
    let rt_header = find(&rt, "HeaderBar");
    assert!(rt_header.variables.bindings.contains_key("visible"));
    assert!(rt_header.variables.bindings.contains_key("strokeTopWeight"));
    let rt_conn = find(&rt, "FlowConnector");
    let rt_sticky = find(&rt, "NoteA");
    let rt_shape = find(&rt, "DecisionNode");
    match &rt_conn.kind {
        NodeKind::Connector(cp) => {
            assert_eq!(cp.line, ConnectorLine::Curved);
            assert_eq!(cp.start.node, Some(rt_sticky.id));
            assert_eq!(cp.end.node, Some(rt_shape.id));
        }
        other => panic!("expected Connector after roundtrip, got {other:?}"),
    }
}

#[test]
fn fig_v106_schema_functional_fields_roundtrip() {
    let g = |s: u64, l: u64| -> Value { json!({"sessionID": s, "localID": l}) };
    let node = |id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str, extra: Value| -> Value {
        let mut v = json!({
            "guid": g(id.0, id.1),
            "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
            "type": ty,
            "name": name,
            "phase": "CREATED",
            "visible": true,
            "opacity": 1.0,
            "size": {"x": 100.0, "y": 50.0},
            "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
        });
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        v
    };

    let changes = json!([
        node((0, 0), (0, 0), "", "DOCUMENT", "Document", json!({})),
        node((0, 1), (0, 0), "!", "CANVAS", "Page 1", json!({})),
        // 1. SYMBOL with symbolDescription, propsAreBubbled, proportionsConstrained, hyperlink (url), and pluginData
        node(
            (1, 10),
            (0, 1),
            "!",
            "SYMBOL",
            "DesignSystemCard",
            json!({
                "size": {"x": 320.0, "y": 180.0},
                "symbolDescription": "Primary card component for dashboard",
                "propsAreBubbled": true,
                "proportionsConstrained": true,
                "hyperlink": {"url": "https://figma.com/design-system"},
                "pluginData": [
                    {
                        "pluginID": "com.tokens.studio",
                        "key": "theme",
                        "value": "dark"
                    },
                    {
                        "key": "rawKey",
                        "value": "rawValue"
                    }
                ]
            })
        ),
        // 2. FRAME with targetAspectRatio (v106 aspect lock) and internal node hyperlink (guid)
        node(
            (1, 11),
            (0, 1),
            "\"",
            "FRAME",
            "AspectLockedMedia",
            json!({
                "size": {"x": 160.0, "y": 90.0},
                "targetAspectRatio": {"value": {"x": 16.0, "y": 9.0}},
                "hyperlink": {"guid": g(1, 10)}
            })
        ),
        // 3. TEXT with fontVariantCommonLigatures=false, fontVariantContextualLigatures=false, and textTracking
        node(
            (1, 12),
            (0, 1),
            "#",
            "TEXT",
            "TrackedHeadline",
            json!({
                "size": {"x": 240.0, "y": 32.0},
                "fontName": {"family": "Inter", "style": "Bold", "postscript": "Inter-Bold"},
                "fontSize": 20.0,
                "fontVariantCommonLigatures": false,
                "fontVariantContextualLigatures": false,
                "textTracking": 1.25,
                "textData": {"characters": "Headline"}
            })
        )
    ]);

    let doc = import_fig_node_changes(&changes, "v106").expect("import_fig_node_changes v106 functional fields");

    // Check DesignSystemCard
    let card = find(&doc, "DesignSystemCard");
    assert_eq!(card.component.description, "Primary card component for dashboard");
    assert!(card.component.expose_nested_instances);
    assert!(card.lock_aspect);
    assert_eq!(card.hyperlink.as_deref(), Some("https://figma.com/design-system"));
    assert_eq!(card.plugin_data.get("com.tokens.studio:theme").map(String::as_str), Some("dark"));
    assert_eq!(card.plugin_data.get("rawKey").map(String::as_str), Some("rawValue"));

    // Check AspectLockedMedia
    let media = find(&doc, "AspectLockedMedia");
    assert!(media.lock_aspect, "targetAspectRatio should set lock_aspect = true");
    assert_eq!(media.hyperlink.as_deref(), Some("guid:1:10"));

    // Check TrackedHeadline
    let txt = find(&doc, "TrackedHeadline");
    let NodeKind::Text(tp) = &txt.kind else {
        panic!("expected Text node");
    };
    assert!(!tp.style.is_feature_enabled("liga"));
    assert!(!tp.style.is_feature_enabled("calt"));
    assert_eq!(tp.style.features.iter().find(|(k, _)| k.eq_ignore_ascii_case("liga")).map(|(_, v)| *v), Some(0));
    assert_eq!(tp.style.features.iter().find(|(k, _)| k.eq_ignore_ascii_case("calt")).map(|(_, v)| *v), Some(0));
    assert!((tp.style.letter_spacing - 1.25).abs() < 1e-4, "expected letter_spacing 1.25 from textTracking, got {}", tp.style.letter_spacing);

    // Export back to binary .fig and re-import to verify lossless round-trip
    let exported = export_fig(&doc).expect("export_fig");
    let rt = import_fig(&exported).expect("re-import_fig");

    let rt_card = find(&rt, "DesignSystemCard");
    assert_eq!(rt_card.component.description, "Primary card component for dashboard");
    assert!(rt_card.component.expose_nested_instances);
    assert!(rt_card.lock_aspect);
    assert_eq!(rt_card.hyperlink.as_deref(), Some("https://figma.com/design-system"));
    assert_eq!(rt_card.plugin_data.get("com.tokens.studio:theme").map(String::as_str), Some("dark"));
    assert_eq!(rt_card.plugin_data.get("rawKey").map(String::as_str), Some("rawValue"));

    let rt_media = find(&rt, "AspectLockedMedia");
    assert!(rt_media.lock_aspect);
    assert_eq!(rt_media.hyperlink.as_deref(), Some("guid:1:10"));

    let rt_txt = find(&rt, "TrackedHeadline");
    let NodeKind::Text(rt_tp) = &rt_txt.kind else {
        panic!("expected Text node after roundtrip");
    };
    assert!(!rt_tp.style.is_feature_enabled("liga"));
    assert!(!rt_tp.style.is_feature_enabled("calt"));
    assert!((rt_tp.style.letter_spacing - 1.25).abs() < 1e-4);
}

#[test]
fn fig_v106_wave5_schema_99pct_coverage_roundtrip() {
    let g = |s: u64, l: u64| -> Value { json!({"sessionID": s, "localID": l}) };
    let node = |id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str, extra: Value| -> Value {
        let mut v = json!({
            "guid": g(id.0, id.1),
            "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
            "type": ty,
            "name": name,
            "phase": "CREATED",
            "visible": true,
            "opacity": 1.0,
            "size": {"x": 120.0, "y": 60.0},
            "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
        });
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        v
    };

    // Encode a simple MoveTo(0,0) -> LineTo(40,20) -> Close commandsBlob
    let mut cmd_blob = Vec::new();
    cmd_blob.push(1u8); // MoveTo
    cmd_blob.extend_from_slice(&0.0f32.to_le_bytes());
    cmd_blob.extend_from_slice(&0.0f32.to_le_bytes());
    cmd_blob.push(2u8); // LineTo
    cmd_blob.extend_from_slice(&40.0f32.to_le_bytes());
    cmd_blob.extend_from_slice(&20.0f32.to_le_bytes());
    cmd_blob.push(0u8); // Close

    let msg = json!({
        "blobs": [
            {"bytes": cmd_blob}
        ],
        "nodeChanges": [
            node((0, 0), (0, 0), "", "DOCUMENT", "Wave5Doc", json!({})),
            node((0, 1), (0, 0), "!", "CANVAS", "Page 1", json!({})),
            // Soft-deleted style node should be ignored
            node((9, 99), (0, 0), "z", "RECTANGLE", "DeletedStyle", json!({
                "styleType": "FILL",
                "isSoftDeleted": true,
                "fillPaints": [{"type": "SOLID", "color": {"r": 1.0, "g": 0.0, "b": 0.0, "a": 1.0}, "opacity": 1.0, "visible": true}]
            })),
            // 1. BRUSH node with strokeGeometry (no fillGeometry) and brushType
            node((1, 10), (0, 1), "!", "BRUSH", "ScatterBrush", json!({
                "brushType": "SCATTER",
                "strokeWeight": 4.0,
                "strokePaints": [{"type": "SOLID", "color": {"r": 0.1, "g": 0.1, "b": 0.1, "a": 1.0}, "opacity": 1.0, "visible": true}],
                "strokeGeometry": [{"windingRule": "NONZERO", "commandsBlob": 0}]
            })),
            // 2. SHAPE_WITH_TEXT with ENG_DATABASE
            node((1, 11), (0, 1), "\"", "SHAPE_WITH_TEXT", "DbNode", json!({
                "shapeWithTextType": "ENG_DATABASE",
                "textData": {"characters": "Users DB"}
            })),
            // 3. TEXT with autoRename=false, textDecorationSkipInk=false, and fontVariant* OpenType enums
            node((1, 12), (0, 1), "#", "TEXT", "RichVariantText", json!({
                "fontName": {"family": "Inter", "style": "Regular", "postscript": "Inter-Regular"},
                "fontSize": 16.0,
                "autoRename": false,
                "textDecorationSkipInk": false,
                "fontVariantNumericFigure": "OLDSTYLE",
                "fontVariantNumericSpacing": "TABULAR",
                "fontVariantNumericFraction": "DIAGONAL",
                "fontVariantCaps": "ALL_SMALL",
                "fontVariantPosition": "SUPER",
                "textData": {"characters": "123/456"}
            })),
            // 4. SYMBOL with variantPropSpecs and fig_metadata bag
            node((1, 20), (0, 1), "$", "SYMBOL", "VariantBtn", json!({
                "variantPropSpecs": [
                    {"name": "Size", "value": "Lg"},
                    {"name": "Tone", "value": "Brand"}
                ],
                "userFacingVersion": "v2.4",
                "sharedSymbolVersion": "108",
                "isSymbolPublishable": true,
                "publishID": g(7, 77),
                "widgetCachedAncestor": {"nodeId": g(8, 88)},
                "editInfo": {
                    "timestampIso8601": "2026-10-15T00:00:00Z",
                    "userId": "u-42",
                    "lastEditedAt": 1700000000,
                    "createdAt": 1690000000
                },
                "pluginRelaunchData": [
                    {"pluginID": "p1", "message": "Sync", "command": "sync", "isDeleted": false}
                ],
                "nodeGenerationData": {
                    "useFineGrainedSyncing": true
                }
            })),
            node((1, 21), (1, 20), "!", "ROUNDED_RECTANGLE", "VariantBtnChild", json!({
                "overrideKey": g(9, 21)
            })),
            // 5. Detached FRAME with detachedSymbolId + overrideLevel
            node((1, 30), (0, 1), "%", "FRAME", "DetachedCard", json!({
                "overrideLevel": 2,
                "detachedSymbolId": {"assetRef": {"key": "sym-key-99", "version": "v3"}}
            })),
            // 6. INSTANCE with symbolOverrides carrying overrideLevel + detachedSymbolId
            node((1, 31), (0, 1), "&", "INSTANCE", "VariantBtnInst", json!({
                "symbolData": {
                    "symbolID": g(1, 20),
                    "symbolOverrides": [
                        {
                            "guidPath": {"guids": [g(9, 21)]},
                            "overrideLevel": 1,
                            "detachedSymbolId": {"guid": g(1, 20)}
                        }
                    ]
                }
            }))
        ]
    });

    let doc = import_fig_node_changes(&msg, "Wave5Doc").expect("import Wave5Doc");

    // Soft-deleted style is excluded
    assert!(doc.styles.is_empty());

    // 1. BRUSH node
    let brush = find(&doc, "ScatterBrush");
    assert_eq!(brush.fig_metadata.get("brushType").map(String::as_str), Some("SCATTER"));
    match &brush.kind {
        NodeKind::Vector { paths, .. } => assert_eq!(paths.len(), 1, "BRUSH should populate paths from strokeGeometry fallback"),
        other => panic!("expected Vector for BRUSH, got {other:?}"),
    }

    // 2. SHAPE_WITH_TEXT (ENG_DATABASE)
    let db = find(&doc, "DbNode");
    assert_eq!(db.fig_metadata.get("shapeWithTextType").map(String::as_str), Some("ENG_DATABASE"));
    match &db.kind {
        NodeKind::ShapeWithText(sp) => {
            assert_eq!(sp.shape, ShapeKind::Ellipse);
            assert_eq!(sp.text.content, "Users DB");
        }
        other => panic!("expected ShapeWithText, got {other:?}"),
    }

    // 3. TEXT node
    let txt = find(&doc, "RichVariantText");
    let NodeKind::Text(tp) = &txt.kind else { panic!("expected Text") };
    assert!(!tp.auto_rename);
    assert!(!tp.decoration_skip_ink);
    for tag in ["onum", "tnum", "frac", "smcp", "c2sc", "sups"] {
        assert!(tp.style.is_feature_enabled(tag), "expected OpenType feature {tag} to be enabled");
    }

    // 4. SYMBOL variantPropSpecs + fig_metadata
    let sym = find(&doc, "VariantBtn");
    assert_eq!(sym.component.variant_props.get("Size").map(String::as_str), Some("Lg"));
    assert_eq!(sym.component.variant_props.get("Tone").map(String::as_str), Some("Brand"));
    assert_eq!(sym.fig_metadata.get("userFacingVersion").map(String::as_str), Some("v2.4"));
    assert_eq!(sym.fig_metadata.get("sharedSymbolVersion").map(String::as_str), Some("108"));
    assert_eq!(sym.fig_metadata.get("isSymbolPublishable").map(String::as_str), Some("true"));
    assert_eq!(sym.fig_metadata.get("publishID").map(String::as_str), Some("7:77"));
    assert_eq!(sym.fig_metadata.get("widgetCachedAncestor").map(String::as_str), Some("8:88"));
    assert!(sym.fig_metadata.contains_key("editInfo"));
    assert!(sym.fig_metadata.contains_key("pluginRelaunchData"));
    assert!(sym.fig_metadata.contains_key("nodeGenerationData"));

    // 5. Detached FRAME + INSTANCE child override
    let det = find(&doc, "DetachedCard");
    assert_eq!(det.component.override_level, Some(2));
    assert_eq!(det.detached_symbol_id.as_deref(), Some("assetRef:sym-key-99:v3"));

    let inst = find(&doc, "VariantBtnInst");
    let inst_child = inst.children.iter().filter_map(|id| doc.get(*id)).find(|n| n.name == "VariantBtnChild").expect("VariantBtnChild");
    assert_eq!(inst_child.component.override_level, Some(1));
    assert_eq!(inst_child.component.detached_symbol_id.as_deref(), Some("guid:1:20"));

    // Roundtrip through binary .fig export & re-import
    let bytes = export_fig(&doc).expect("export_fig Wave5Doc");
    let rt = import_fig(&bytes).expect("re-import Wave5Doc");

    let rt_brush = find(&rt, "ScatterBrush");
    assert_eq!(rt_brush.fig_metadata.get("brushType").map(String::as_str), Some("SCATTER"));

    let rt_db = find(&rt, "DbNode");
    assert_eq!(rt_db.fig_metadata.get("shapeWithTextType").map(String::as_str), Some("ENG_DATABASE"));
    match &rt_db.kind {
        NodeKind::ShapeWithText(sp) => assert_eq!(sp.shape, ShapeKind::Ellipse),
        other => panic!("expected ShapeWithText after roundtrip, got {other:?}"),
    }

    let rt_txt = find(&rt, "RichVariantText");
    let NodeKind::Text(rt_tp) = &rt_txt.kind else { panic!("expected Text after roundtrip") };
    assert!(!rt_tp.auto_rename);
    assert!(!rt_tp.decoration_skip_ink);
    for tag in ["onum", "tnum", "frac", "smcp", "c2sc", "sups"] {
        assert!(rt_tp.style.is_feature_enabled(tag), "expected OpenType feature {tag} after roundtrip");
    }

    let rt_sym = find(&rt, "VariantBtn");
    assert_eq!(rt_sym.component.variant_props.get("Size").map(String::as_str), Some("Lg"));
    assert_eq!(rt_sym.component.variant_props.get("Tone").map(String::as_str), Some("Brand"));
    assert_eq!(rt_sym.fig_metadata.get("userFacingVersion").map(String::as_str), Some("v2.4"));
    assert_eq!(rt_sym.fig_metadata.get("sharedSymbolVersion").map(String::as_str), Some("108"));
    assert_eq!(rt_sym.fig_metadata.get("isSymbolPublishable").map(String::as_str), Some("true"));
    assert_eq!(rt_sym.fig_metadata.get("publishID").map(String::as_str), Some("7:77"));
    assert_eq!(rt_sym.fig_metadata.get("widgetCachedAncestor").map(String::as_str), Some("8:88"));
    assert!(rt_sym.fig_metadata.contains_key("editInfo"));
    assert!(rt_sym.fig_metadata.contains_key("pluginRelaunchData"));
    assert!(rt_sym.fig_metadata.contains_key("nodeGenerationData"));

    let rt_det = find(&rt, "DetachedCard");
    assert_eq!(rt_det.component.override_level, Some(2));
    assert_eq!(rt_det.detached_symbol_id.as_deref(), Some("assetRef:sym-key-99:v3"));

    let rt_inst = find(&rt, "VariantBtnInst");
    let rt_inst_child = rt_inst.children.iter().filter_map(|id| rt.get(*id)).find(|n| n.name == "VariantBtnChild").expect("VariantBtnChild after roundtrip");
    assert_eq!(rt_inst_child.component.override_level, Some(1));
    assert_eq!(rt_inst_child.component.detached_symbol_id.as_deref(), Some("guid:1:20"));
}

#[test]
fn roundtrip_stack_child_margins_606_to_609() {
    let mut doc = Document::new("ChildMarginsTest");
    let page = doc.add_page("Page 1");
    let fid = doc.alloc_id();
    let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 300.0, 120.0));
    frame.name = "MarginFrame".into();
    if let NodeKind::Frame(fp) = &mut frame.kind {
        fp.auto_layout =
            Some(AutoLayout { direction: LayoutDirection::Horizontal, item_spacing: 8.0, padding: [10.0, 10.0, 10.0, 10.0], ..Default::default() });
    }
    doc.insert(page, frame).unwrap();

    let cid = doc.alloc_id();
    let mut child = Node::rectangle(cid, Rect::new(0.0, 0.0, 80.0, 40.0));
    child.name = "MarginChild".into();
    child.layout_child.margin = [6.0, 12.0, 18.0, 24.0];
    doc.insert(fid, child).unwrap();

    let bytes = export_fig(&doc).expect("export_fig with child margins");
    let rt = import_fig(&bytes).expect("import_fig with child margins");
    let rt_child = find(&rt, "MarginChild");
    assert_eq!(rt_child.layout_child.margin, [6.0, 12.0, 18.0, 24.0]);
}

#[test]
fn roundtrip_twelve_track_grid_columns_and_rows() {
    let g = |s: u64, l: u64| -> Value { json!({"sessionID": s, "localID": l}) };
    let changes = json!([
        {
            "guid": g(0, 0),
            "type": "DOCUMENT",
            "name": "Doc"
        },
        {
            "guid": g(0, 1),
            "parentIndex": {"guid": g(0, 0), "position": "!"},
            "type": "CANVAS",
            "name": "Page 1"
        },
        {
            "guid": g(1, 10),
            "parentIndex": {"guid": g(0, 1), "position": "!"},
            "type": "FRAME",
            "name": "TwelveColGrid",
            "size": {"x": 1200.0, "y": 800.0},
            "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0},
            "stackMode": "GRID",
            "gridColumns": 12,
            "gridRows": 10,
            "gridColumnsSizing": "1fr 2fr 80px auto 1fr 100px auto 2fr 90px 3fr 110px auto",
            "gridRowsSizing": "60px auto 1fr 80px 2fr auto 70px 1fr 90px auto"
        }
    ]);

    let doc = import_fig_node_changes(&changes, "TwelveTrackGrid").expect("import 12-track grid");
    let expected_cols = [
        GridTrack::Flex(1.0),
        GridTrack::Flex(2.0),
        GridTrack::Fixed(80.0),
        GridTrack::Hug,
        GridTrack::Flex(1.0),
        GridTrack::Fixed(100.0),
        GridTrack::Hug,
        GridTrack::Flex(2.0),
        GridTrack::Fixed(90.0),
        GridTrack::Flex(3.0),
        GridTrack::Fixed(110.0),
        GridTrack::Hug,
    ];
    let expected_rows = [
        GridTrack::Fixed(60.0),
        GridTrack::Hug,
        GridTrack::Flex(1.0),
        GridTrack::Fixed(80.0),
        GridTrack::Flex(2.0),
        GridTrack::Hug,
        GridTrack::Fixed(70.0),
        GridTrack::Flex(1.0),
        GridTrack::Fixed(90.0),
        GridTrack::Hug,
    ];

    let grid = find(&doc, "TwelveColGrid");
    let al = grid.kind.frame_props().unwrap().auto_layout.as_ref().expect("grid auto_layout");
    assert_eq!(al.grid_columns, 12);
    assert_eq!(al.grid_rows, 10);
    for (i, exp) in expected_cols.iter().enumerate() {
        assert_eq!(al.grid_col_tracks.get(i), Some(exp), "imported col track {i}");
    }
    for (i, exp) in expected_rows.iter().enumerate() {
        assert_eq!(al.grid_row_tracks.get(i), Some(exp), "imported row track {i}");
    }

    let bytes = export_fig(&doc).expect("export_fig 12-track grid");
    let rt = import_fig(&bytes).expect("re-import_fig 12-track grid");
    let rt_grid = find(&rt, "TwelveColGrid");
    let rt_al = rt_grid.kind.frame_props().unwrap().auto_layout.as_ref().expect("roundtrip grid auto_layout");
    assert_eq!(rt_al.grid_columns, 12);
    assert_eq!(rt_al.grid_rows, 10);
    for (i, exp) in expected_cols.iter().enumerate() {
        assert_eq!(rt_al.grid_col_tracks.get(i), Some(exp), "roundtrip col track {i}");
    }
    for (i, exp) in expected_rows.iter().enumerate() {
        assert_eq!(rt_al.grid_row_tracks.get(i), Some(exp), "roundtrip row track {i}");
    }
}

#[test]
fn typed_kiwi_passthrough_draw_motion_table_slide_and_large_file_roundtrip() {
    let g = |s: u64, l: u64| -> Value { json!({"sessionID": s, "localID": l}) };
    let node = |id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str, extra: Value| -> Value {
        let mut v = json!({
            "guid": g(id.0, id.1),
            "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
            "type": ty,
            "name": name,
            "phase": "CREATED",
            "visible": true,
            "opacity": 1.0,
            "size": {"x": 120.0, "y": 60.0},
            "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
        });
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        v
    };

    let build_passthrough_changes = |pad_count: usize| -> Value {
        let mut changes = vec![
            node((0, 0), (0, 0), "", "DOCUMENT", "PassthroughX1Doc", json!({})),
            node((0, 1), (0, 0), "!", "CANVAS", "Page 1", json!({})),
            // 1. Draw: BRUSH node with variableWidthPoints, dynamic/scatter/stretch settings, pathTrim, strokeData
            node(
                (1, 10),
                (0, 1),
                "!",
                "BRUSH",
                "ScatterBrushStroke",
                json!({
                    "brushType": "SCATTER",
                    "strokeSeed": 42,
                    "strokeOffset": 2.5,
                    "strokeBrushGuid": g(10, 20),
                    "variableWidthPoints": [
                        {"position": 0.25, "ascent": 3.5, "descent": 4.25, "segmentId": 2}
                    ],
                    "dynamicStrokeSettings": {"frequency": 8.0, "wiggle": 1.5, "smoothen": 0.75},
                    "scatterStrokeSettings": {
                        "gap": 12.0,
                        "wiggle": 2.0,
                        "angularJitter": 15.0,
                        "sizeJitter": 0.5,
                        "rotation": 15.0
                    },
                    "stretchStrokeSettings": {"orientation": "FORWARD"},
                    "scatterBrushTransforms": [
                        {"m00": 1.0, "m01": 0.0, "m02": 5.0, "m10": 0.0, "m11": 1.0, "m12": 2.0}
                    ],
                    "pathTrim": {"value": {"x": 0.1, "y": 0.9}},
                    "strokeData": {
                        "strokes": [{"strokeId": 1, "strokeWeight": 2.0}],
                        "version": 77
                    }
                }),
            ),
            // Draw: TRANSFORM node with transformModifiers
            node(
                (1, 11),
                (0, 1),
                "\"",
                "TRANSFORM",
                "RadialRepeater",
                json!({
                    "transformModifiers": [
                        {
                            "type": "REPEAT",
                            "count": 4,
                            "repeatType": "RADIAL",
                            "axis": "X",
                            "unitType": "PIXELS",
                            "offset": {"x": 16.0, "y": 8.0}
                        }
                    ]
                }),
            ),
            // Draw: TEXT_PATH node with textPathStart
            node(
                (1, 12),
                (0, 1),
                "#",
                "TEXT_PATH",
                "CurvedBannerPath",
                json!({
                    "textPathStart": {"tValue": 12.5, "forward": true}
                }),
            ),
            // 2. Motion: FRAME with timelineDefinitions & timelineAssignments + KEYFRAME_TRACK + KEYFRAME + ANIMATION_PRESET_INSTANCE
            node(
                (1, 20),
                (0, 1),
                "$",
                "FRAME",
                "MotionStage",
                json!({
                    "timelineDefinitions": {"entries": [{"id": g(3, 10), "data": {"durationUs": 1500000, "defaultTimeline": true}}]},
                    "timelineAssignments": {
                        "entries": [{"key": {"assignedTimelineId": g(3, 10), "containingTimelineId": g(1, 20)}, "value": {"offsetUs": 0, "disabled": false}}]
                    },
                    "animationPresets": {"presets": [{"animationPresetId": {"guid": g(3, 99)}, "timelineDefId": g(3, 10), "sortPosition": "!"}]},
                    "textElementDelayBehaviour": {"enabled": true, "grouping": "WORD", "delayMs": 50.0}
                }),
            ),
            node(
                (1, 21),
                (1, 20),
                "!",
                "KEYFRAME_TRACK",
                "OpacityTrack",
                json!({
                    "keyframeOperation": "SET",
                    "trackLoopMode": "LOOP",
                    "timelineDuration": 1500,
                    "timelineOffset": 250,
                    "timelineDisabled": false,
                    "styleIdsForAnimation": [
                        {"id": g(4, 1), "timelineDefId": g(3, 10), "sortPosition": "!"}
                    ]
                }),
            ),
            node(
                (1, 22),
                (1, 21),
                "!",
                "KEYFRAME",
                "MidKeyframe",
                json!({
                    "timelinePosition": 500,
                    "keyframeValue": {"value": {"floatValue": 0.75}, "valueType": "FLOAT"},
                    "interpolationType": "BEZIER",
                    "bezierHandles": {"p1x": -0.25, "p1y": 0.0, "p2x": 0.25, "p2y": 1.0},
                    "easingData": {
                        "easingType": "CUSTOM_CUBIC",
                        "easingValue": {"bezierEasing": {"p1x": 0.5, "p1y": 0.0, "p2x": 0.25, "p2y": 1.0}}
                    },
                    "easingType": "OUT_BACK_CUBIC",
                    "interpolationColorSpace": "OKLCH"
                }),
            ),
            node(
                (1, 23),
                (1, 20),
                "\"",
                "ANIMATION_PRESET_INSTANCE",
                "PresetBounce",
                json!({
                    "timelineDuration": 750
                }),
            ),
            // 3. Table: TABLE and TABLE_CELL
            node(
                (1, 30),
                (0, 1),
                "%",
                "TABLE",
                "PricingTable",
                json!({
                    "tableRowPositions": {"entries": [{"id": g(5, 1), "position": "!"}, {"id": g(5, 2), "position": "\""}]},
                    "tableColumnPositions": {"entries": [{"id": g(5, 3), "position": "!"}]},
                    "tableRowHeights": {"entries": [{"id": g(5, 1), "size": 44.0}]},
                    "tableColumnWidths": {"entries": [{"id": g(5, 3), "size": 180.0}]},
                    "tableMergedCells": {
                        "entries": [
                            {"rowId": g(5, 1), "colId": g(5, 3), "rowSpan": 2, "colSpan": 1}
                        ]
                    }
                }),
            ),
            node(
                (1, 31),
                (1, 30),
                "!",
                "TABLE_CELL",
                "CellR1C1",
                json!({
                    "gridChildVerticalAlign": "CENTER",
                    "gridChildHorizontalAlign": "MAX",
                    "lockMode": "ALL"
                }),
            ),
            // 4. Slide: SLIDE_GRID -> SLIDE_ROW -> SLIDE -> INTERACTIVE_SLIDE_ELEMENT
            node((1, 40), (0, 1), "&", "SLIDE_GRID", "DeckGrid", json!({})),
            node((1, 41), (1, 40), "!", "SLIDE_ROW", "IntroRow", json!({})),
            node(
                (1, 42),
                (1, 41),
                "!",
                "SLIDE",
                "OpeningSlide",
                json!({
                    "slideSpeakerNotes": "Highlight typed Kiwi passthrough X1",
                    "isSkippedSlide": false,
                    "slideNumber": "SLIDE",
                    "slideNumberSeparator": "/",
                    "objectAnimations": [
                        {
                            "id": g(6, 1),
                            "event": {"interactionType": "ON_CLICK"},
                            "actions": [{"transitionNodeID": g(1, 43), "transitionType": "SMART_ANIMATE", "transitionDuration": 0.5, "easingType": "INOUT_CUBIC"}]
                        }
                    ]
                }),
            ),
            node((1, 43), (1, 42), "!", "INTERACTIVE_SLIDE_ELEMENT", "LivePollWidget", json!({})),
        ];

        for i in 0..pad_count {
            let local = 1000 + i as u64;
            changes.push(node((9, local), (0, 1), &format!("p{i:04}"), "ROUNDED_RECTANGLE", &format!("PadRect_{i}"), json!({})));
        }

        Value::Array(changes)
    };

    let verify_doc = |doc: &Document, label: &str| {
        // Export to .fig binary and re-import
        let bytes = export_fig(doc).unwrap_or_else(|e| panic!("{label}: export_fig failed: {e}"));
        let rt = import_fig(&bytes).unwrap_or_else(|e| panic!("{label}: import_fig failed: {e}"));

        for name in [
            "ScatterBrushStroke",
            "RadialRepeater",
            "CurvedBannerPath",
            "MotionStage",
            "OpacityTrack",
            "MidKeyframe",
            "PresetBounce",
            "PricingTable",
            "CellR1C1",
            "DeckGrid",
            "IntroRow",
            "OpeningSlide",
            "LivePollWidget",
        ] {
            let before = find(doc, name);
            let after = find(&rt, name);
            assert_eq!(after.kind, before.kind, "{label}: NodeKind mismatch on {name}");
            assert!(
                !before.fig_metadata.is_empty() || matches!(name, "DeckGrid" | "IntroRow" | "LivePollWidget"),
                "{label}: expected non-empty fig_metadata on {name}"
            );
            assert_eq!(after.fig_metadata, before.fig_metadata, "{label}: fig_metadata mismatch on {name}");
        }

        // Spot-check specific typed passthrough fields after round-trip
        let brush = find(&rt, "ScatterBrushStroke");
        assert_eq!(brush.fig_metadata.get("brushType").map(String::as_str), Some("SCATTER"));
        assert_eq!(brush.fig_metadata.get("strokeSeed").map(String::as_str), Some("42"));
        assert_eq!(brush.fig_metadata.get("strokeOffset").map(String::as_str), Some("2.5"));
        assert!(brush.fig_metadata.contains_key("variableWidthPoints"));
        assert!(brush.fig_metadata.contains_key("dynamicStrokeSettings"));
        assert!(brush.fig_metadata.contains_key("scatterStrokeSettings"));
        assert!(brush.fig_metadata.contains_key("stretchStrokeSettings"));
        assert!(brush.fig_metadata.contains_key("scatterBrushTransforms"));
        assert!(brush.fig_metadata.contains_key("pathTrim"));
        assert!(brush.fig_metadata.contains_key("strokeData"));

        let xform = find(&rt, "RadialRepeater");
        assert_eq!(xform.kind, NodeKind::Unknown { type_name: "TRANSFORM".into() });
        assert!(xform.fig_metadata.contains_key("transformModifiers"));

        let tpath = find(&rt, "CurvedBannerPath");
        assert_eq!(tpath.kind, NodeKind::Unknown { type_name: "TEXT_PATH".into() });
        let start: Value = serde_json::from_str(tpath.fig_metadata.get("textPathStart").expect("textPathStart")).expect("textPathStart is JSON");
        assert_eq!((start["tValue"].as_f64(), start["forward"].as_bool()), (Some(12.5), Some(true)));

        let kf = find(&rt, "MidKeyframe");
        assert_eq!(kf.kind, NodeKind::Unknown { type_name: "KEYFRAME".into() });
        assert_eq!(kf.fig_metadata.get("timelinePosition").map(String::as_str), Some("500"));
        assert_eq!(kf.fig_metadata.get("interpolationType").map(String::as_str), Some("BEZIER"));
        assert_eq!(kf.fig_metadata.get("interpolationColorSpace").map(String::as_str), Some("OKLCH"));
        assert!(kf.fig_metadata.contains_key("bezierHandles"));
        assert!(kf.fig_metadata.contains_key("easingData"));

        let tbl = find(&rt, "PricingTable");
        assert_eq!(tbl.kind, NodeKind::Unknown { type_name: "TABLE".into() });
        assert!(tbl.fig_metadata.contains_key("tableRowPositions"));
        assert!(tbl.fig_metadata.contains_key("tableColumnPositions"));
        assert!(tbl.fig_metadata.contains_key("tableRowHeights"));
        assert!(tbl.fig_metadata.contains_key("tableColumnWidths"));
        assert!(tbl.fig_metadata.contains_key("tableMergedCells"));

        // The cell's grid alignment is a typed field, not passthrough metadata, and it survives the trip.
        let cell = find(&rt, "CellR1C1");
        assert!(!cell.fig_metadata.contains_key("gridChildVerticalAlign") && !cell.fig_metadata.contains_key("gridChildHorizontalAlign"));
        assert_eq!((cell.layout_child.grid_align_h, cell.layout_child.grid_align_v), (GridChildAlign::End, GridChildAlign::Center), "{label}");

        let slide = find(&rt, "OpeningSlide");
        assert_eq!(slide.kind, NodeKind::Unknown { type_name: "SLIDE".into() });
        assert_eq!(slide.fig_metadata.get("slideSpeakerNotes").map(String::as_str), Some("Highlight typed Kiwi passthrough X1"));
        assert_eq!(slide.fig_metadata.get("isSkippedSlide").map(String::as_str), Some("false"));
        assert_eq!(slide.fig_metadata.get("slideNumber").map(String::as_str), Some("SLIDE"));
        assert!(slide.fig_metadata.contains_key("objectAnimations"));
    };

    // 1. Standard file (< 5,000 entries)
    let small_changes = build_passthrough_changes(0);
    let small_doc = import_fig_node_changes(&small_changes, "PassthroughSmall").expect("import small");
    verify_doc(&small_doc, "small_file");

    // 2. Large file (> 5,000 entries) — must preserve all typed passthrough fields identically!
    let large_changes = build_passthrough_changes(5_005);
    let large_doc = import_fig_node_changes(&large_changes, "PassthroughLarge").expect("import large");
    verify_doc(&large_doc, "large_file_5000_plus");
}

/// `objectAnimations[].transitionType` and `symbolLinks[].displayText` are matched by exact schema
/// name on export; a stray space in either name drops the field on the way out.
#[test]
fn typed_kiwi_passthrough_keeps_slide_out_transition_and_symbol_link_display_text() {
    let g = |s: u64, l: u64| -> Value { json!({"sessionID": s, "localID": l}) };
    let node = |id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str, extra: Value| -> Value {
        let mut v = json!({
            "guid": g(id.0, id.1),
            "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
            "type": ty,
            "name": name,
            "phase": "CREATED",
            "visible": true,
            "opacity": 1.0,
            "size": {"x": 120.0, "y": 60.0},
            "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
        });
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        v
    };
    let changes = Value::Array(vec![
        node((0, 0), (0, 0), "", "DOCUMENT", "TransitionDoc", json!({})),
        node((0, 1), (0, 0), "!", "CANVAS", "Page 1", json!({})),
        node(
            (1, 1),
            (0, 1),
            "!",
            "SLIDE",
            "ExitSlide",
            json!({
                "objectAnimations": [
                    {
                        "id": g(6, 1),
                        "event": {"interactionType": "ON_CLICK"},
                        "actions": [{"transitionNodeID": g(1, 2), "transitionType": "SLIDE_OUT_TO_LEFT", "transitionDuration": 0.4}]
                    }
                ]
            }),
        ),
        node((1, 2), (1, 1), "!", "INTERACTIVE_SLIDE_ELEMENT", "ExitingWidget", json!({})),
        node(
            (1, 3),
            (0, 1),
            "\"",
            "SYMBOL",
            "LinkedComponent",
            json!({
                "symbolLinks": [
                    {"uri": "https://example.com/docs", "displayName": "Docs", "displayText": "Read"}
                ]
            }),
        ),
    ]);

    let doc = import_fig_node_changes(&changes, "TransitionDoc").expect("import node changes");
    let rt = import_fig(&export_fig(&doc).expect("export_fig")).expect("import_fig");

    let metadata_json = |d: &Document, name: &str, key: &str| -> Value {
        let raw = find(d, name).fig_metadata.get(key).unwrap_or_else(|| panic!("{name} lost fig_metadata[{key}]"));
        serde_json::from_str(raw).unwrap_or_else(|e| panic!("{name} fig_metadata[{key}] is not JSON: {e}"))
    };

    assert_eq!(metadata_json(&doc, "ExitSlide", "objectAnimations")[0]["actions"][0]["transitionType"], "SLIDE_OUT_TO_LEFT", "fixture sanity");
    assert_eq!(metadata_json(&doc, "LinkedComponent", "symbolLinks")[0]["displayText"], "Read", "fixture sanity");

    assert_eq!(
        metadata_json(&rt, "ExitSlide", "objectAnimations")[0]["actions"][0]["transitionType"],
        "SLIDE_OUT_TO_LEFT",
        "objectAnimations transitionType after round-trip"
    );
    assert_eq!(metadata_json(&rt, "LinkedComponent", "symbolLinks")[0]["displayText"], "Read", "symbolLinks displayText after round-trip");
    for (name, key) in [("ExitSlide", "objectAnimations"), ("LinkedComponent", "symbolLinks")] {
        assert_eq!(metadata_json(&rt, name, key), metadata_json(&doc, name, key), "{name} fig_metadata[{key}] changed across export + import");
    }
}

/// The slot flags and the slot content guid have typed document fields, so the passthrough bag must not
/// hold a second copy: a stale copy would bring a cleared flag back on the next export.
#[test]
fn slot_fields_stay_typed_and_a_cleared_flag_is_not_resurrected_by_passthrough_metadata() {
    let g = |s: u64, l: u64| -> Value { json!({"sessionID": s, "localID": l}) };
    let node = |id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str, extra: Value| -> Value {
        let mut v = json!({
            "guid": g(id.0, id.1),
            "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
            "type": ty,
            "name": name,
            "phase": "CREATED",
            "visible": true,
            "opacity": 1.0,
            "size": {"x": 120.0, "y": 60.0},
            "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
        });
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        v
    };
    let changes = Value::Array(vec![
        node((0, 0), (0, 0), "", "DOCUMENT", "SlotDoc", json!({})),
        node((0, 1), (0, 0), "!", "CANVAS", "Page 1", json!({})),
        node((1, 1), (0, 1), "!", "FRAME", "SlotHost", json!({"isSlot": true})),
        node((1, 2), (0, 1), "\"", "FRAME", "SlotCard", json!({"isSlotContent": true, "jsonSerialized": {"slot-content-id": {"guid": g(9, 9)}}})),
    ]);
    let mut doc = import_fig_node_changes(&changes, "SlotDoc").expect("import node changes");

    assert!(find(&doc, "SlotHost").component.is_slot);
    assert!(find(&doc, "SlotCard").component.is_slot_content);
    assert_eq!(find(&doc, "SlotCard").component.slot_content_id.as_deref(), Some("9:9"));
    for name in ["SlotHost", "SlotCard"] {
        for key in ["isSlot", "isSlotContent", "jsonSerialized"] {
            assert!(!find(&doc, name).fig_metadata.contains_key(key), "{name}: {key} is typed, it must not also sit in fig_metadata");
        }
    }

    let (host, card) = (find(&doc, "SlotHost").id, find(&doc, "SlotCard").id);
    doc.get_mut(host).expect("host node").component.is_slot = false;
    let card_component = &mut doc.get_mut(card).expect("card node").component;
    card_component.is_slot_content = false;
    card_component.slot_content_id = None;

    let rt = import_fig(&export_fig(&doc).expect("export_fig")).expect("import_fig");
    assert!(!find(&rt, "SlotHost").component.is_slot, "cleared isSlot came back");
    assert!(!find(&rt, "SlotCard").component.is_slot_content, "cleared isSlotContent came back");
    assert!(find(&rt, "SlotCard").component.slot_content_id.is_none(), "cleared slot content id came back");
}

#[test]
fn instance_multi_paragraph_and_mixed_style_text_override_resolves_with_null_override_key_and_linked_styles() {
    let g = |s: u64, l: u64| -> Value { json!({"sessionID": s, "localID": l}) };
    let null_g = g(u32::MAX as u64, u32::MAX as u64);
    let node = |id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str, extra: Value| -> Value {
        let mut v = json!({
            "guid": g(id.0, id.1),
            "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
            "type": ty,
            "name": name,
            "phase": "CREATED",
            "visible": true,
            "opacity": 1.0,
            "size": {"x": 520.0, "y": 120.0},
            "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
        });
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        v
    };

    let para1 = "We've included a few simple templates to help you get started.\n\n";
    let para2_prefix = "Check out the ";
    let para2_link = "Guide to variables in Figma";
    let para2_suffix = " to learn more.";
    let overridden_text = format!("{para1}{para2_prefix}{para2_link}{para2_suffix}");

    // Build characterStyleIDs: para1 = 0, para2_prefix = 13 (semibold), para2_link = 12 (semibold + underline + purple), para2_suffix = 13 (semibold).
    let mut char_ids: Vec<u64> = vec![0; para1.encode_utf16().count()];
    char_ids.extend(std::iter::repeat_n(13, para2_prefix.encode_utf16().count()));
    char_ids.extend(std::iter::repeat_n(12, para2_link.encode_utf16().count()));
    char_ids.extend(std::iter::repeat_n(13, para2_suffix.encode_utf16().count()));

    let mut raw_nodes = vec![
        node((0, 0), (0, 0), "", "DOCUMENT", "NoteDoc", json!({})),
        node((0, 1), (0, 0), "!", "CANVAS", "Internal Only Canvas", json!({"internalOnly": true})),
        node((0, 2), (0, 0), "\"", "CANVAS", "Examples", json!({})),
        // Shared text style 18:2016 ("Text lg/Semibold": Inter Semi Bold 600, 18px)
        node(
            (18, 2016),
            (0, 1),
            "!",
            "TEXT",
            "Text lg/Semibold",
            json!({
                "styleType": "TEXT",
                "fontName": {"family": "Inter", "style": "Semi Bold", "postscript": "Inter-SemiBold"},
                "fontSize": 18.0,
                "lineHeight": {"units": "PIXELS", "value": 28.0}
            }),
        ),
        // Component "Design note" (2680:401975) with child "Supporting text" (2680:401977)
        // carrying the null GUID sentinel overrideKey {4294967295, 4294967295}.
        node((2680, 401975), (0, 1), "\"", "SYMBOL", "Design note", json!({})),
        node(
            (2680, 401977),
            (2680, 401975),
            "!",
            "TEXT",
            "Supporting text",
            json!({
                "overrideKey": null_g,
                "fontName": {"family": "Inter", "style": "Regular", "postscript": "Inter-Regular"},
                "fontSize": 18.0,
                "textData": {
                    "characters": "Lorem ipsum dolor sit amet, consectetur adipiscing elit."
                }
            }),
        ),
        // Instance "Design note" (11185:8546) on the Examples page overriding 2680:401977
        // with two paragraphs and mixed character styles referencing shared style 18:2016.
        node(
            (11185, 8546),
            (0, 2),
            "!",
            "INSTANCE",
            "Design note instance",
            json!({
                "symbolData": {
                    "symbolID": g(2680, 401975),
                    "symbolOverrides": [
                        {
                            "guidPath": {"guids": [g(2680, 401977)]},
                            "textData": {
                                "characters": overridden_text,
                                "characterStyleIDs": char_ids,
                                "styleOverrideTable": [
                                    {
                                        "styleID": 12,
                                        "styleIdForText": {"guid": g(18, 2016)},
                                        "textDecoration": "UNDERLINE",
                                        "fillPaints": [
                                            {
                                                "type": "SOLID",
                                                "visible": true,
                                                "opacity": 1.0,
                                                "blendMode": "NORMAL",
                                                "color": {"r": 0.41, "g": 0.25, "b": 0.78, "a": 1.0}
                                            }
                                        ]
                                    },
                                    {
                                        "styleID": 13,
                                        "styleIdForText": {"guid": g(18, 2016)},
                                        "styleIdForFill": {"guid": null_g},
                                        "fillPaints": [
                                            {
                                                "type": "SOLID",
                                                "visible": true,
                                                "opacity": 1.0,
                                                "blendMode": "NORMAL",
                                                "color": {"r": 0.09, "g": 0.11, "b": 0.15, "a": 1.0}
                                            }
                                        ]
                                    }
                                ]
                            }
                        }
                    ]
                }
            }),
        ),
    ];

    // Pad past the 10_000-node fast_clone threshold so expand_instance takes the fast_clone path.
    for i in 0..10_010u64 {
        raw_nodes.push(node((5000, i + 1), (0, 2), "z", "RECTANGLE", "Pad", json!({})));
    }

    let doc = import_fig_node_changes(&Value::Array(raw_nodes), "NoteDoc").expect("import");
    let inst = find(&doc, "Design note instance");
    let child_id = *inst.children.first().expect("instance expanded");
    let child = doc.get(child_id).expect("supporting text child");
    assert!(child.component.overrides.contains(&OverrideField::Characters), "Characters override recorded");
    let NodeKind::Text(tp) = &child.kind else { panic!("expected Text child") };
    assert!(!tp.content.starts_with("Lorem ipsum"), "multi-paragraph override was lost and fell back to component Lorem ipsum: {:?}", tp.content);
    assert_eq!(tp.content, overridden_text);
    assert_eq!(tp.runs.len(), 3, "expected 3 styled runs in second paragraph");
    assert_eq!(tp.runs[0].style.font_weight, 600, "run 0 inherits semibold (600) from linked styleIdForText 18:2016");
    assert_eq!(tp.runs[1].style.font_weight, 600, "run 1 inherits semibold (600) from linked styleIdForText 18:2016");
    assert_eq!(tp.runs[1].style.decoration, TextDecoration::Underline, "run 1 keeps inline UNDERLINE override");
    assert_eq!(tp.runs[2].style.font_weight, 600, "run 2 inherits semibold (600) from linked styleIdForText 18:2016");
}

#[test]
fn stack_child_align_self_max_center_min_stretch_roundtrip_and_relayout() {
    let g = |s: u64, l: u64| -> Value { json!({"sessionID": s, "localID": l}) };
    let node = |id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str, extra: Value| -> Value {
        let mut v = json!({
            "guid": g(id.0, id.1),
            "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
            "type": ty,
            "name": name,
            "phase": "CREATED",
            "visible": true,
            "opacity": 1.0,
            "size": {"x": 30.0, "y": 20.0},
            "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
        });
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        v
    };
    let changes = json!([
        node((0, 0), (0, 0), "!", "DOCUMENT", "AlignSelfDoc", json!({})),
        node((0, 1), (0, 0), "!", "CANVAS", "Page 1", json!({})),
        node(
            (1, 10),
            (0, 1),
            "!",
            "FRAME",
            "Row",
            json!({
                "size": {"x": 200.0, "y": 100.0},
                "stackMode": "HORIZONTAL",
                "stackLayoutVersion": 1,
                "stackSpacing": 10.0,
                "stackPrimaryAlignItems": "MIN",
                "stackCounterAlignItems": "MIN",
                "stackPrimarySizing": "FIXED",
                "stackCounterSizing": "FIXED"
            })
        ),
        node((1, 11), (1, 10), "!", "RECTANGLE", "ChildCenter", json!({"stackChildAlignSelf": "CENTER"})),
        node((1, 12), (1, 10), "\"", "RECTANGLE", "ChildMax", json!({"stackChildAlignSelf": "MAX"})),
        node((1, 13), (1, 10), "#", "RECTANGLE", "ChildMin", json!({"stackChildAlignSelf": "MIN"})),
        node((1, 14), (1, 10), "$", "RECTANGLE", "ChildBaseline", json!({"stackChildAlignSelf": "BASELINE"})),
    ]);
    let mut doc = import_fig_node_changes(&changes, "AlignSelfDoc").expect("import AlignSelfDoc");
    let row_id = find(&doc, "Row").id;
    doc.relayout_subtree(row_id);

    let c_center = find(&doc, "ChildCenter");
    let c_max = find(&doc, "ChildMax");
    let c_min = find(&doc, "ChildMin");
    assert_eq!(c_center.layout_child.align_self, AlignSelf::Center);
    assert_eq!(c_max.layout_child.align_self, AlignSelf::End);
    assert_eq!(c_min.layout_child.align_self, AlignSelf::Start);
    // BASELINE is stored even though the solver does not align baselines yet
    assert_eq!(find(&doc, "ChildBaseline").layout_child.align_self, AlignSelf::Baseline);
    assert_eq!((c_center.transform.tx, c_center.transform.ty), (0.0, 40.0));
    assert_eq!((c_max.transform.tx, c_max.transform.ty), (40.0, 80.0));
    assert_eq!((c_min.transform.tx, c_min.transform.ty), (80.0, 0.0));

    let bytes = export_fig(&doc).expect("export_fig AlignSelfDoc");
    let mut rt = import_fig(&bytes).expect("re-import AlignSelfDoc");
    let rt_row = find(&rt, "Row").id;
    rt.relayout_subtree(rt_row);
    let rt_center = find(&rt, "ChildCenter");
    let rt_max = find(&rt, "ChildMax");
    let rt_min = find(&rt, "ChildMin");
    assert_eq!(rt_center.layout_child.align_self, AlignSelf::Center);
    assert_eq!(rt_max.layout_child.align_self, AlignSelf::End);
    assert_eq!(rt_min.layout_child.align_self, AlignSelf::Start);
    assert_eq!(find(&rt, "ChildBaseline").layout_child.align_self, AlignSelf::Baseline);
    assert_eq!((rt_center.transform.tx, rt_center.transform.ty), (0.0, 40.0));
    assert_eq!((rt_max.transform.tx, rt_max.transform.ty), (40.0, 80.0));
    assert_eq!((rt_min.transform.tx, rt_min.transform.ty), (80.0, 0.0));
}

#[test]
fn counter_fill_child_keeps_its_cross_size_in_a_hugging_frame_through_a_fig_round_trip() {
    let mut doc = Document::new("HugFill");
    let page = doc.add_page("Page 1");
    let fid = doc.alloc_id();
    let mut row = Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 1.0));
    row.name = "Row".into();
    let al = AutoLayout { direction: LayoutDirection::Horizontal, primary_sizing: Sizing::Fixed, counter_sizing: Sizing::Hug, ..Default::default() };
    row.kind = NodeKind::Frame(FrameProps { auto_layout: Some(al), ..Default::default() });
    let row_id = doc.insert(page, row).unwrap();
    let cid = doc.alloc_id();
    let mut child = Node::rectangle(cid, Rect::new(0.0, 0.0, 30.0, 40.0));
    child.name = "FillChild".into();
    child.layout_child.vertical = Sizing::Fill;
    doc.insert(row_id, child).unwrap();
    doc.relayout_subtree(row_id);
    assert_eq!(find(&doc, "Row").size.y, 40.0, "Fill on the counter axis alone keeps the natural size");

    let bytes = export_fig(&doc).expect("export_fig HugFill");
    let mut rt = import_fig(&bytes).expect("re-import HugFill");
    let rt_row = find(&rt, "Row").id;
    rt.relayout_subtree(rt_row);
    assert_eq!(find(&rt, "Row").size.y, 40.0, "the round trip must not change the cross size");
    assert_eq!(find(&rt, "FillChild").size.y, 40.0);
}

#[test]
fn resized_instance_applies_stretch_and_scale_constraints_to_children_without_derived_data() {
    let g = |s: u64, l: u64| -> Value { json!({"sessionID": s, "localID": l}) };
    let node = |id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str, extra: Value| -> Value {
        let mut v = json!({
            "guid": g(id.0, id.1),
            "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
            "type": ty,
            "name": name,
            "phase": "CREATED",
            "visible": true,
            "opacity": 1.0,
            "size": {"x": 160.0, "y": 160.0},
            "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
        });
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        v
    };
    let changes = json!([
        node((0, 0), (0, 0), "!", "DOCUMENT", "ResizedPieInstance", json!({})),
        node((0, 1), (0, 0), "!", "CANVAS", "Page 1", json!({})),
        node((1, 2), (0, 1), "!", "SYMBOL", "_Pie chart", json!({"size": {"x": 160.0, "y": 160.0}})),
        node(
            (1, 3),
            (1, 2),
            "!",
            "ELLIPSE",
            "Background",
            json!({
                "size": {"x": 160.0, "y": 160.0},
                "horizontalConstraint": "STRETCH",
                "verticalConstraint": "STRETCH",
            }),
        ),
        node(
            (1, 4),
            (1, 2),
            "\"",
            "ELLIPSE",
            "Series 2",
            json!({
                "size": {"x": 160.0, "y": 160.0},
                "transform": {
                    "m00": 0.5, "m01": 0.8660254, "m02": -29.282032,
                    "m10": -0.8660254, "m11": 0.5, "m12": 109.282036
                },
                "horizontalConstraint": "STRETCH",
                "verticalConstraint": "STRETCH",
                "overrideKey": {"sessionID": 4294967295u64, "localID": 4294967295u64},
            }),
        ),
        node(
            (1, 5),
            (1, 2),
            "#",
            "RECTANGLE",
            "ScaledBadge",
            json!({
                "size": {"x": 40.0, "y": 40.0},
                "transform": {
                    "m00": 1.0, "m01": 0.0, "m02": 20.0,
                    "m10": 0.0, "m11": 1.0, "m12": 40.0
                },
                "horizontalConstraint": "SCALE",
                "verticalConstraint": "SCALE",
            }),
        ),
        node(
            (1, 10),
            (0, 1),
            "\"",
            "INSTANCE",
            "_Pie chart",
            json!({
                "size": {"x": 280.0, "y": 280.0},
                "symbolData": {
                    "symbolID": {"sessionID": 1, "localID": 2},
                    "symbolOverrides": []
                }
            }),
        ),
    ]);
    let doc = import_fig_node_changes(&changes, "ResizedPieInstance").expect("import ResizedPieInstance");
    let inst = doc.nodes.values().find(|n| matches!(n.kind, NodeKind::Instance { .. })).expect("instance exists");
    assert_eq!(inst.size, Vec2::new(280.0, 280.0));
    let bg = inst.children.iter().filter_map(|id| doc.get(*id)).find(|n| n.name == "Background").expect("Background child");
    assert_eq!(bg.size, Vec2::new(280.0, 280.0), "160x160 component with Stretch/Stretch child instanced at 280x280 must import child at 280x280");
    assert_eq!((bg.transform.tx, bg.transform.ty), (0.0, 0.0));

    let series2 = inst.children.iter().filter_map(|id| doc.get(*id)).find(|n| n.name == "Series 2").expect("Series 2 child");
    assert_eq!(series2.size, Vec2::new(280.0, 280.0));
    assert!((series2.transform.tx - (-51.243556)).abs() < 1e-3, "got tx={}", series2.transform.tx);
    assert!((series2.transform.ty - 191.24356).abs() < 1e-3, "got ty={}", series2.transform.ty);

    let badge = inst.children.iter().filter_map(|id| doc.get(*id)).find(|n| n.name == "ScaledBadge").expect("ScaledBadge child");
    assert_eq!(badge.size, Vec2::new(70.0, 70.0));
    assert_eq!((badge.transform.tx, badge.transform.ty), (35.0, 70.0));
}

#[test]
fn grid_auto_tracks_and_v106_track_maps_roundtrip() {
    let g = |s: u64, l: u64| -> Value { json!({"sessionID": s, "localID": l}) };
    let nc = |id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str, extra: Value| -> Value {
        let mut v = json!({
            "guid": g(id.0, id.1),
            "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
            "type": ty,
            "name": name,
            "phase": "CREATED",
            "visible": true,
            "opacity": 1.0,
            "size": {"x": 120.0, "y": 60.0},
            "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
        });
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        v
    };
    let changes = json!([
        {"guid": g(0, 0), "type": "DOCUMENT", "name": "GridAutoDoc"},
        {"guid": g(0, 1), "parentIndex": {"guid": g(0, 0), "position": "a"}, "type": "CANVAS", "name": "Page 1"},
        nc(
            (1, 10),
            (0, 1),
            "a",
            "FRAME",
            "AutoRowsEnumGrid",
            json!({
                "size": {"x": 300.0, "y": 200.0},
                "stackMode": "GRID",
                "gridRows": {
                    "entries": [
                        {"id": g(9, 1), "position": "!"},
                        {"id": g(9, 2), "position": "\""}
                    ]
                },
                "gridColumns": {
                    "entries": [
                        {"id": g(9, 3), "position": "!"},
                        {"id": g(9, 4), "position": "\""}
                    ]
                },
                "gridColumnsSizing": {
                    "entries": [
                        {"id": g(9, 3), "trackSize": {"minSizing": {"type": "FLEX", "value": 1.0}, "maxSizing": {"type": "FLEX", "value": 1.0}}},
                        {"id": g(9, 4), "trackSize": {"minSizing": {"type": "FIXED", "value": 120.0}, "maxSizing": {"type": "FIXED", "value": 120.0}}}
                    ]
                },
                "gridRowsSizing": {
                    "entries": [
                        {"id": g(9, 1), "trackSize": {"minSizing": {"type": "HUG", "value": 1.0}, "maxSizing": {"type": "HUG", "value": 1.0}}},
                        {"id": g(9, 2), "trackSize": {"minSizing": {"type": "FIXED", "value": 48.0}, "maxSizing": {"type": "FIXED", "value": 48.0}}}
                    ]
                },
                "gridAutoTracks": "ROWS"
            })
        ),
        nc(
            (1, 11),
            (1, 10),
            "a",
            "RECTANGLE",
            "PinnedCell",
            json!({
                "size": {"x": 40.0, "y": 20.0},
                "gridRowAnchor": g(9, 2),
                "gridColumnAnchor": g(9, 4)
            })
        ),
        nc(
            (1, 20),
            (0, 1),
            "b",
            "FRAME",
            "AutoRowsSizedGrid",
            json!({
                "size": {"x": 300.0, "y": 200.0},
                "stackMode": "GRID",
                "gridColumns": 3,
                "gridRows": 1,
                "gridColumnsSizing": "1fr 1fr 1fr",
                "gridRowsSizing": "40px",
                "gridAutoTracks": "64px"
            })
        )
    ]);

    let doc = import_fig_node_changes(&changes, "GridAutoDoc").expect("import GridAutoDoc");

    let g1 = find(&doc, "AutoRowsEnumGrid");
    assert!(!g1.fig_metadata.contains_key("gridAutoTracks"), "gridAutoTracks must map to the model, not metadata");
    let al1 = g1.kind.frame_props().unwrap().auto_layout.clone().unwrap();
    assert_eq!(al1.grid_columns, 2);
    assert_eq!(al1.grid_rows, 2);
    assert_eq!(&al1.grid_col_tracks[..2], &[GridTrack::Flex(1.0), GridTrack::Fixed(120.0)]);
    assert_eq!(&al1.grid_row_tracks[..2], &[GridTrack::Hug, GridTrack::Fixed(48.0)]);
    assert_eq!(al1.grid_auto_tracks, GridAutoTracks::Rows);

    let cell = find(&doc, "PinnedCell");
    assert_eq!(cell.layout_child.grid_row, Some(1));
    assert_eq!(cell.layout_child.grid_column, Some(1));

    let g2 = find(&doc, "AutoRowsSizedGrid");
    let al2 = g2.kind.frame_props().unwrap().auto_layout.clone().unwrap();
    assert_eq!(al2.grid_auto_tracks, GridAutoTracks::Track(GridTrack::Fixed(64.0)));

    let bytes = export_fig(&doc).expect("export_fig GridAutoDoc");
    let rt = import_fig(&bytes).expect("re-import GridAutoDoc");

    let rt_al1 = find(&rt, "AutoRowsEnumGrid").kind.frame_props().unwrap().auto_layout.clone().unwrap();
    assert_eq!(rt_al1.grid_auto_tracks, GridAutoTracks::Rows);
    assert_eq!(&rt_al1.grid_col_tracks[..2], &[GridTrack::Flex(1.0), GridTrack::Fixed(120.0)]);
    assert_eq!(&rt_al1.grid_row_tracks[..2], &[GridTrack::Hug, GridTrack::Fixed(48.0)]);

    let rt_al2 = find(&rt, "AutoRowsSizedGrid").kind.frame_props().unwrap().auto_layout.clone().unwrap();
    assert_eq!(rt_al2.grid_auto_tracks, GridAutoTracks::Track(GridTrack::Fixed(64.0)));
}

#[test]
fn nested_instance_swap_updates_shell_fills_strokes_effects_and_reapplies_props() {
    let g = |s: u64, l: u64| -> Value { json!({"sessionID": s, "localID": l}) };
    let nc = |id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str, extra: Value| -> Value {
        let mut v = json!({
            "guid": g(id.0, id.1),
            "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
            "type": ty,
            "name": name,
            "phase": "CREATED",
            "visible": true,
            "opacity": 1.0,
            "size": {"x": 150.0, "y": 36.0},
            "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
        });
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        v
    };
    let purple_fill = json!([{"type": "SOLID", "visible": true, "opacity": 1.0, "color": {"r": 0.498, "g": 0.337, "b": 0.851, "a": 1.0}}]);
    let purple_stroke = json!([{"type": "SOLID", "visible": true, "opacity": 1.0, "color": {"r": 1.0, "g": 1.0, "b": 1.0, "a": 0.12}}]);
    let shadow_effect = json!([{"type": "DROP_SHADOW", "visible": true, "radius": 2.0, "spread": 0.0, "offset": {"x": 0.0, "y": 1.0}, "color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 0.05}}]);
    let prop_defs = json!([{"id": g(100, 1), "name": "Icon leading", "type": "BOOL", "initialValue": {"boolValue": true}}]);
    let icon_refs = json!([{"componentPropNodeField": "VISIBLE", "defID": g(100, 1)}]);

    let changes = json!([
        nc((0, 0), (0, 0), "!", "DOCUMENT", "ButtonSwapDoc", json!({})),
        nc((0, 1), (0, 0), "!", "CANVAS", "Page 1", json!({})),
        nc(
            (1, 2),
            (0, 1),
            "!",
            "SYMBOL",
            "Hierarchy=Primary",
            json!({
                "size": {"x": 150.0, "y": 36.0},
                "fillPaints": purple_fill,
                "strokePaints": purple_stroke,
                "strokeWeight": 2.0,
                "effects": shadow_effect,
                "cornerRadius": 8.0,
                "stackMode": "HORIZONTAL",
                "stackSpacing": 6.0,
                "stackHorizontalPadding": 12.0,
                "stackVerticalPadding": 8.0,
                "componentPropDefs": prop_defs,
            }),
        ),
        nc((1, 3), (1, 2), "!", "RECTANGLE", "Icon", json!({"size": {"x": 20.0, "y": 20.0}, "componentPropRefs": icon_refs})),
        nc((1, 4), (1, 2), "\"", "TEXT", "Text", json!({"textData": {"characters": "Button"}, "size": {"x": 60.0, "y": 20.0}})),
        nc(
            (1, 5),
            (0, 1),
            "\"",
            "SYMBOL",
            "Hierarchy=Link color",
            json!({
                "size": {"x": 126.0, "y": 20.0},
                "fillPaints": [],
                "strokePaints": [],
                "strokeWeight": 0.0,
                "effects": [],
                "cornerRadius": 0.0,
                "stackMode": "HORIZONTAL",
                "stackSpacing": 6.0,
                "stackHorizontalPadding": 0.0,
                "stackVerticalPadding": 0.0,
                "componentPropDefs": prop_defs,
            }),
        ),
        nc((1, 6), (1, 5), "!", "RECTANGLE", "Icon", json!({"size": {"x": 20.0, "y": 20.0}, "componentPropRefs": icon_refs})),
        nc((1, 7), (1, 5), "\"", "TEXT", "Text", json!({"textData": {"characters": "Button"}, "size": {"x": 60.0, "y": 20.0}})),
        nc((1, 10), (0, 1), "#", "SYMBOL", "Section footer", json!({"size": {"x": 343.0, "y": 40.0}}),),
        nc(
            (1, 11),
            (1, 10),
            "!",
            "INSTANCE",
            "ActionButton",
            json!({
                "size": {"x": 150.0, "y": 36.0},
                "fillPaints": purple_fill,
                "strokePaints": purple_stroke,
                "strokeWeight": 2.0,
                "effects": shadow_effect,
                "cornerRadius": 8.0,
                "stackMode": "HORIZONTAL",
                "stackSpacing": 6.0,
                "stackHorizontalPadding": 12.0,
                "stackVerticalPadding": 8.0,
                "symbolData": {
                    "symbolID": g(1, 2),
                    "symbolOverrides": []
                },
                "componentPropAssignments": [
                    {"defID": g(100, 1), "value": {"boolValue": false}}
                ],
            }),
        ),
        nc(
            (1, 20),
            (0, 1),
            "$",
            "INSTANCE",
            "FooterUse",
            json!({
                "size": {"x": 343.0, "y": 40.0},
                "symbolData": {
                    "symbolID": g(1, 10),
                    "symbolOverrides": [
                        {
                            "guidPath": {"guids": [g(1, 11)]},
                            "overriddenSymbolID": g(1, 5),
                        },
                        {
                            "guidPath": {"guids": [g(1, 11), g(1, 7)]},
                            "textData": {"characters": "View integration"},
                        }
                    ]
                }
            }),
        ),
    ]);

    let doc = import_fig_node_changes(&changes, "ButtonSwapDoc").expect("import ButtonSwapDoc");
    let link_comp = find(&doc, "Hierarchy=Link color").id;
    let footer_use = find(&doc, "FooterUse");
    let swapped_btn = footer_use.children.iter().filter_map(|id| doc.get(*id)).find(|n| n.name == "ActionButton").expect("ActionButton inside FooterUse");
    assert_eq!(instance_component(swapped_btn), Some(link_comp));
    assert!(swapped_btn.fills.is_empty(), "swapped Link color button must not keep Primary purple fill: {:?}", swapped_btn.fills);
    assert!(swapped_btn.stroke.paints.is_empty(), "swapped Link color button must not keep Primary stroke");
    assert!(swapped_btn.effects.is_empty(), "swapped Link color button must not keep Primary shadow");
    assert_eq!(swapped_btn.corner_radii, [0.0; 4]);
    assert_eq!(swapped_btn.size, Vec2::new(126.0, 20.0));
    let al = swapped_btn.kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
    assert_eq!(al.padding, [0.0, 0.0, 0.0, 0.0]);
    let icon = swapped_btn.children.iter().filter_map(|id| doc.get(*id)).find(|n| n.name == "Icon").expect("Icon inside ActionButton");
    assert!(!icon.visible, "Icon leading = false assignment on ActionButton must be re-applied after swap");
    let text = swapped_btn.children.iter().filter_map(|id| doc.get(*id)).find(|n| n.name == "Text").expect("Text inside ActionButton");
    let NodeKind::Text(tp) = &text.kind else { panic!("expected text") };
    assert_eq!(tp.content, "View integration");
}

#[test]
fn layout_guide_alignment_fixed_width_auto_count_and_visibility_roundtrip() {
    use designos_doc::{Color, Document, LayoutGrid, LayoutGridAlign, Node, NodeKind, Rect, Style};

    // Old serialized LayoutGrid JSON without alignment/section_size/visible must load with Stretch, 0.0, true.
    let legacy_grid: LayoutGrid =
        serde_json::from_str(r#"{"Columns":{"count":12,"gutter":20.0,"offset":16.0,"color":{"r":1.0,"g":0.0,"b":0.0,"a":0.1}}}"#).unwrap();
    assert_eq!(
        legacy_grid,
        LayoutGrid::Columns {
            count: 12,
            gutter: 20.0,
            offset: 16.0,
            section_size: 0.0,
            alignment: LayoutGridAlign::Stretch,
            color: Color { r: 1.0, g: 0.0, b: 0.0, a: 0.1 },
            visible: true,
        }
    );

    let mut doc = Document::new("GuideParity");
    let page = doc.add_page("Page 1");
    let fid = doc.alloc_id();
    let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 1440.0, 900.0));
    frame.name = "DesktopFrame".into();
    let g_cols = LayoutGrid::Columns {
        count: 12,
        gutter: 20.0,
        offset: 0.0,
        section_size: 60.0,
        alignment: LayoutGridAlign::Center,
        color: Color::rgba(255, 0, 0, 26),
        visible: true,
    };
    let g_rows = LayoutGrid::Rows {
        count: 0,
        gutter: 16.0,
        offset: 24.0,
        section_size: 48.0,
        alignment: LayoutGridAlign::Min,
        color: Color::rgba(0, 128, 255, 26),
        visible: true,
    };
    let g_hidden = LayoutGrid::Grid { size: 8.0, color: Color::rgba(0, 255, 0, 26), visible: false };
    if let NodeKind::Frame(fp) = &mut frame.kind {
        fp.grids = vec![g_cols.clone(), g_rows.clone(), g_hidden.clone()];
    }
    doc.insert(page, frame).unwrap();

    let style_grid = LayoutGrid::Columns {
        count: 0,
        gutter: 12.0,
        offset: 32.0,
        section_size: 72.0,
        alignment: LayoutGridAlign::Max,
        color: Color::rgba(255, 128, 0, 26),
        visible: false,
    };
    doc.styles.insert("S:grid-auto-max".into(), Style::Grid(vec![style_grid.clone()]));

    let bytes = export_fig(&doc).unwrap();
    let rt = import_fig(&bytes).unwrap();
    let rt_frame = find(&rt, "DesktopFrame");
    let rt_grids = &rt_frame.kind.frame_props().unwrap().grids;
    assert_eq!(rt_grids.len(), 3);
    assert!(matches!(
        &rt_grids[0],
        LayoutGrid::Columns { count: 12, gutter, offset, section_size, alignment: LayoutGridAlign::Center, visible: true, .. }
        if (*gutter - 20.0).abs() < 1e-4 && (*offset - 0.0).abs() < 1e-4 && (*section_size - 60.0).abs() < 1e-4
    ));
    assert!(matches!(
        &rt_grids[1],
        LayoutGrid::Rows { count: 0, gutter, offset, section_size, alignment: LayoutGridAlign::Min, visible: true, .. }
        if (*gutter - 16.0).abs() < 1e-4 && (*offset - 24.0).abs() < 1e-4 && (*section_size - 48.0).abs() < 1e-4
    ));
    assert!(matches!(
        &rt_grids[2],
        LayoutGrid::Grid { size, visible: false, .. } if (*size - 8.0).abs() < 1e-4
    ));

    let has_grid_style = rt.styles.values().any(|s| {
        matches!(
            s,
            Style::Grid(gs)
            if matches!(
                gs.as_slice(),
                [LayoutGrid::Columns { count: 0, gutter, offset, section_size, alignment: LayoutGridAlign::Max, visible: false, .. }]
                if (*gutter - 12.0).abs() < 1e-4 && (*offset - 32.0).abs() < 1e-4 && (*section_size - 72.0).abs() < 1e-4
            )
        )
    });
    assert!(has_grid_style, "grid style must round-trip alignment, section_size, auto count, and visible=false");
}

/// Equal-width columns: aligned but with no fixed width (paint width is (400 - 80) / 5).
fn equal_width_columns_doc() -> (Document, LayoutGrid) {
    use designos_doc::{Color, LayoutGridAlign, Node, NodeKind, Rect};
    let equal = LayoutGrid::Columns {
        count: 5,
        gutter: 20.0,
        offset: 0.0,
        section_size: 0.0,
        alignment: LayoutGridAlign::Min,
        color: Color::rgba(255, 0, 0, 26),
        visible: true,
    };
    let mut doc = Document::new("GuideZero");
    let page = doc.add_page("Page 1");
    let fid = doc.alloc_id();
    let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 400.0, 300.0));
    frame.name = "EqualColumns".into();
    if let NodeKind::Frame(fp) = &mut frame.kind {
        fp.grids = vec![equal.clone()];
    }
    doc.insert(page, frame).unwrap();
    (doc, equal)
}

#[test]
fn layout_guide_with_no_fixed_width_keeps_section_size_zero_through_fig() {
    let (doc, equal) = equal_width_columns_doc();
    let back = import_fig(&export_fig(&doc).unwrap()).unwrap();
    assert_eq!(find(&back, "EqualColumns").kind.frame_props().unwrap().grids, vec![equal]);
}

#[test]
fn layout_guide_with_no_fixed_width_keeps_section_size_zero_through_rest() {
    let (doc, equal) = equal_width_columns_doc();
    let json = designos_fig::export_rest_json(&doc);
    let back = designos_fig::import_rest_json(&json.to_string()).unwrap();
    assert_eq!(find(&back, "EqualColumns").kind.frame_props().unwrap().grids, vec![equal]);
}

/// Node name -> (`gridChildHorizontalAlign`, `gridChildVerticalAlign`) as written to the `.fig` message; a key that
/// was not written reads `<absent>`. Only nodes that carry at least one of the keys are listed.
fn wire_grid_child_aligns(bytes: &[u8]) -> BTreeMap<String, (String, String)> {
    use designos_fig::kiwi::{KiwiValue, Schema};
    let archive = designos_fig::container::open(bytes).expect("open fig archive");
    let (schema_bytes, message) = designos_fig::container::read_archive_chunks(&archive).expect("fig chunks");
    let schema = Schema::decode(&schema_bytes).expect("fig schema");
    let root = schema.root_message().expect("root message");
    let msg = schema.decode_message(root, &message).expect("decode fig message");
    let enum_of = |n: &KiwiValue, key: &str| match n.get(key) {
        Some(KiwiValue::Enum(e)) => e.to_string(),
        Some(other) => format!("{other:?}"),
        None => "<absent>".to_string(),
    };
    let mut out = BTreeMap::new();
    for n in msg.get("nodeChanges").and_then(KiwiValue::as_array).expect("nodeChanges") {
        let (h, v) = (enum_of(n, "gridChildHorizontalAlign"), enum_of(n, "gridChildVerticalAlign"));
        if h == "<absent>" && v == "<absent>" {
            continue;
        }
        if let Some(KiwiValue::String(name)) = n.get("name") {
            out.insert(name.clone(), (h, v));
        }
    }
    out
}

#[test]
fn grid_child_horizontal_and_vertical_align_roundtrip() {
    use designos_doc::GridChildAlign;

    let g = |s: u64, l: u64| -> Value { json!({"sessionID": s, "localID": l}) };
    let nc = |id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str, extra: Value| -> Value {
        let mut v = json!({
            "guid": g(id.0, id.1),
            "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
            "type": ty,
            "name": name,
            "phase": "CREATED",
            "visible": true,
            "opacity": 1.0,
            "size": {"x": 40.0, "y": 20.0},
            "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
        });
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        v
    };
    let changes = json!([
        {"guid": g(0, 0), "type": "DOCUMENT", "name": "GridChildAlignDoc"},
        {"guid": g(0, 1), "parentIndex": {"guid": g(0, 0), "position": "a"}, "type": "CANVAS", "name": "Page 1"},
        nc(
            (1, 10),
            (0, 1),
            "a",
            "FRAME",
            "GridParent",
            json!({
                "size": {"x": 200.0, "y": 200.0},
                "stackMode": "GRID",
                "gridColumns": 2,
                "gridRows": 2,
                "gridColumnsSizing": "100px 100px",
                "gridRowsSizing": "100px 100px"
            })
        ),
        nc((1, 11), (1, 10), "a", "RECTANGLE", "LegacyDefaultChild", json!({})),
        nc(
            (1, 12),
            (1, 10),
            "b",
            "RECTANGLE",
            "CenteredEndChild",
            json!({
                "gridChildHorizontalAlign": "CENTER",
                "gridChildVerticalAlign": "MAX"
            })
        ),
        nc(
            (1, 13),
            (1, 10),
            "c",
            "RECTANGLE",
            "EndCenterChild",
            json!({
                "gridChildHorizontalAlign": "MAX",
                "gridChildVerticalAlign": "CENTER"
            })
        ),
        nc(
            (1, 14),
            (1, 10),
            "d",
            "RECTANGLE",
            "AutoMinChild",
            json!({
                "gridChildHorizontalAlign": "AUTO",
                "gridChildVerticalAlign": "MIN"
            })
        ),
        nc(
            (1, 15),
            (1, 10),
            "e",
            "RECTANGLE",
            "StartStartChild",
            json!({
                "gridChildHorizontalAlign": "MIN",
                "gridChildVerticalAlign": "MIN"
            })
        )
    ]);

    let doc = import_fig_node_changes(&changes, "GridChildAlignDoc").expect("import GridChildAlignDoc");

    // Figma omits an enum at its schema default, and GridChildAlign defaults to AUTO (enum 0), so a grid child that
    // never set either key inherits the frame's alignment instead of being pinned to the cell origin.
    let legacy = find(&doc, "LegacyDefaultChild");
    assert_eq!((legacy.layout_child.grid_align_h, legacy.layout_child.grid_align_v), (GridChildAlign::Auto, GridChildAlign::Auto));

    let c1 = find(&doc, "CenteredEndChild");
    assert!(!c1.fig_metadata.contains_key("gridChildVerticalAlign"), "gridChildVerticalAlign must map to LayoutChild, not fig_metadata");
    assert!(!c1.fig_metadata.contains_key("gridChildHorizontalAlign"));
    assert_eq!((c1.layout_child.grid_align_h, c1.layout_child.grid_align_v), (GridChildAlign::Center, GridChildAlign::End));

    let c2 = find(&doc, "EndCenterChild");
    assert_eq!((c2.layout_child.grid_align_h, c2.layout_child.grid_align_v), (GridChildAlign::End, GridChildAlign::Center));

    let c3 = find(&doc, "AutoMinChild");
    assert_eq!((c3.layout_child.grid_align_h, c3.layout_child.grid_align_v), (GridChildAlign::Auto, GridChildAlign::Start));

    let c4 = find(&doc, "StartStartChild");
    assert_eq!((c4.layout_child.grid_align_h, c4.layout_child.grid_align_v), (GridChildAlign::Start, GridChildAlign::Start));

    let bytes = export_fig(&doc).expect("export_fig GridChildAlignDoc");

    // On the wire an explicit Start is MIN and an unset child is AUTO: omitting the key would read back as AUTO.
    let wire = wire_grid_child_aligns(&bytes);
    assert_eq!(wire["LegacyDefaultChild"], ("AUTO".to_string(), "AUTO".to_string()));
    assert_eq!(wire["CenteredEndChild"], ("CENTER".to_string(), "MAX".to_string()));
    assert_eq!(wire["EndCenterChild"], ("MAX".to_string(), "CENTER".to_string()));
    assert_eq!(wire["AutoMinChild"], ("AUTO".to_string(), "MIN".to_string()));
    assert_eq!(wire["StartStartChild"], ("MIN".to_string(), "MIN".to_string()));
    assert!(!wire.contains_key("GridParent"), "only grid children carry the alignment keys");

    let rt = import_fig(&bytes).expect("re-import GridChildAlignDoc");

    let rt_legacy = find(&rt, "LegacyDefaultChild");
    assert_eq!((rt_legacy.layout_child.grid_align_h, rt_legacy.layout_child.grid_align_v), (GridChildAlign::Auto, GridChildAlign::Auto));

    let rt_c4 = find(&rt, "StartStartChild");
    assert_eq!((rt_c4.layout_child.grid_align_h, rt_c4.layout_child.grid_align_v), (GridChildAlign::Start, GridChildAlign::Start));

    let rt_c1 = find(&rt, "CenteredEndChild");
    assert_eq!((rt_c1.layout_child.grid_align_h, rt_c1.layout_child.grid_align_v), (GridChildAlign::Center, GridChildAlign::End));

    let rt_c2 = find(&rt, "EndCenterChild");
    assert_eq!((rt_c2.layout_child.grid_align_h, rt_c2.layout_child.grid_align_v), (GridChildAlign::End, GridChildAlign::Center));

    let rt_c3 = find(&rt, "AutoMinChild");
    assert_eq!((rt_c3.layout_child.grid_align_h, rt_c3.layout_child.grid_align_v), (GridChildAlign::Auto, GridChildAlign::Start));
}

fn grid_align_changes(frame_extra: Value, children: Vec<(&str, Value)>) -> Value {
    let g = |s: u64, l: u64| -> Value { json!({"sessionID": s, "localID": l}) };
    let xform = json!({"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0});
    let mut frame = json!({
        "guid": g(1, 10),
        "parentIndex": {"guid": g(0, 1), "position": "a"},
        "type": "FRAME",
        "name": "AlignFrame",
        "phase": "CREATED",
        "visible": true,
        "opacity": 1.0,
        "size": {"x": 300.0, "y": 100.0},
        "transform": xform
    });
    for (k, x) in frame_extra.as_object().unwrap() {
        frame[k] = x.clone();
    }
    let mut changes = vec![
        json!({"guid": g(0, 0), "type": "DOCUMENT", "name": "AlignDoc"}),
        json!({"guid": g(0, 1), "parentIndex": {"guid": g(0, 0), "position": "a"}, "type": "CANVAS", "name": "Page 1"}),
        frame,
    ];
    for (i, (name, extra)) in children.into_iter().enumerate() {
        let mut child = json!({
            "guid": g(1, 20 + i as u64),
            "parentIndex": {"guid": g(1, 10), "position": ((b'a' + i as u8) as char).to_string()},
            "type": "RECTANGLE",
            "name": name,
            "phase": "CREATED",
            "visible": true,
            "opacity": 1.0,
            "size": {"x": 40.0, "y": 20.0},
            "transform": xform
        });
        for (k, x) in extra.as_object().unwrap() {
            child[k] = x.clone();
        }
        changes.push(child);
    }
    Value::Array(changes)
}

#[test]
fn grid_child_without_align_keys_is_placed_by_the_frame_alignment() {
    // Three 100x100 cells whose frame centres its children on x and ends them on y. A child that never set either
    // alignment key is AUTO in Figma, so it centres on x and ends on y; an explicit MIN pins it to the cell origin.
    let changes = grid_align_changes(
        json!({
            "stackMode": "GRID",
            "gridColumns": 3,
            "gridRows": 1,
            "gridColumnsSizing": "100px 100px 100px",
            "gridRowsSizing": "100px",
            "stackPrimaryAlignItems": "CENTER",
            "stackCounterAlignItems": "MAX",
            "stackPrimarySizing": "FIXED",
            "stackCounterSizing": "FIXED"
        }),
        vec![
            ("UnsetChild", json!({})),
            ("MinChild", json!({"gridChildHorizontalAlign": "MIN", "gridChildVerticalAlign": "MIN"})),
            ("AutoChild", json!({"gridChildHorizontalAlign": "AUTO", "gridChildVerticalAlign": "AUTO"})),
        ],
    );
    let mut doc = import_fig_node_changes(&changes, "AlignDoc").expect("import AlignDoc");
    let frame = find(&doc, "AlignFrame").id;
    doc.relayout_subtree(frame);
    let at = |doc: &Document, name: &str| {
        let n = find(doc, name);
        (n.transform.tx, n.transform.ty)
    };
    assert_eq!(at(&doc, "UnsetChild"), (30.0, 80.0));
    assert_eq!(at(&doc, "MinChild"), (100.0, 0.0));
    assert_eq!(at(&doc, "AutoChild"), (230.0, 80.0));

    // The same placement after a trip through the .fig writer.
    let mut rt = import_fig(&export_fig(&doc).expect("export_fig AlignDoc")).expect("re-import AlignDoc");
    let rt_frame = find(&rt, "AlignFrame").id;
    rt.relayout_subtree(rt_frame);
    assert_eq!(at(&rt, "UnsetChild"), (30.0, 80.0));
    assert_eq!(at(&rt, "MinChild"), (100.0, 0.0));
    assert_eq!(at(&rt, "AutoChild"), (230.0, 80.0));
}

#[test]
fn child_of_a_flow_frame_keeps_the_default_alignment_and_writes_no_alignment_keys() {
    use designos_doc::GridChildAlign;

    // The alignment keys only mean something under a grid: a flow child stays at the Start default, so a file full of
    // them does not grow two enum fields per node.
    let changes = grid_align_changes(
        json!({"stackMode": "HORIZONTAL", "stackPrimaryAlignItems": "CENTER", "stackCounterAlignItems": "MAX"}),
        vec![("FlowChild", json!({}))],
    );
    let doc = import_fig_node_changes(&changes, "AlignDoc").expect("import AlignDoc");
    let child = find(&doc, "FlowChild");
    assert_eq!((child.layout_child.grid_align_h, child.layout_child.grid_align_v), (GridChildAlign::Start, GridChildAlign::Start));
    let wire = wire_grid_child_aligns(&export_fig(&doc).expect("export_fig AlignDoc"));
    assert!(wire.is_empty(), "flow children wrote alignment keys: {wire:?}");
}

#[test]
fn instance_override_that_resets_a_grid_child_alignment_to_start_survives_a_roundtrip() {
    use designos_doc::GridChildAlign;

    let g = |s: u64, l: u64| -> Value { json!({"sessionID": s, "localID": l}) };
    let node = |id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str, extra: Value| -> Value {
        let mut v = json!({
            "guid": g(id.0, id.1),
            "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
            "type": ty,
            "name": name,
            "phase": "CREATED",
            "visible": true,
            "opacity": 1.0,
            "size": {"x": 40.0, "y": 20.0},
            "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
        });
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        v
    };
    // The main component centres its child horizontally and ends it vertically; the instance resets the horizontal
    // alignment to MIN (Start) and leaves the vertical one alone.
    let changes = json!([
        {"guid": g(0, 0), "type": "DOCUMENT", "name": "OverrideAlignDoc"},
        {"guid": g(0, 1), "parentIndex": {"guid": g(0, 0), "position": "a"}, "type": "CANVAS", "name": "Page 1"},
        node(
            (1, 10),
            (0, 1),
            "a",
            "SYMBOL",
            "AlignComp",
            json!({
                "size": {"x": 100.0, "y": 100.0},
                "stackMode": "GRID",
                "gridColumns": 1,
                "gridRows": 1,
                "gridColumnsSizing": "100px",
                "gridRowsSizing": "100px"
            })
        ),
        node(
            (1, 11),
            (1, 10),
            "a",
            "RECTANGLE",
            "AlignedChild",
            json!({
                "overrideKey": g(9, 11),
                "gridChildHorizontalAlign": "CENTER",
                "gridChildVerticalAlign": "MAX"
            })
        ),
        node(
            (1, 12),
            (0, 1),
            "b",
            "INSTANCE",
            "AlignInst",
            json!({
                "size": {"x": 100.0, "y": 100.0},
                "symbolData": {
                    "symbolID": g(1, 10),
                    "symbolOverrides": [
                        {"guidPath": {"guids": [g(9, 11)]}, "gridChildHorizontalAlign": "MIN"}
                    ]
                }
            })
        )
    ]);
    let instance_child = |doc: &Document| {
        let inst = find(doc, "AlignInst");
        let child = doc.get(*inst.children.first().expect("instance child")).expect("instance child node");
        (child.layout_child.grid_align_h, child.layout_child.grid_align_v)
    };

    let doc = import_fig_node_changes(&changes, "OverrideAlignDoc").expect("import OverrideAlignDoc");
    assert_eq!(instance_child(&doc), (GridChildAlign::Start, GridChildAlign::End), "the override applies on import");

    let bytes = export_fig(&doc).expect("export_fig OverrideAlignDoc");

    // The override entry names only the axis that changed, and names the reset to Start as MIN: an omitted key keeps
    // the main component's value, so the vertical axis (still the main's MAX) must not be written.
    let (_, overrides) = wire_instance_override(&bytes, "AlignInst", None);
    assert_eq!(overrides, ("MIN".to_string(), "<absent>".to_string()), "the symbolOverrides must carry the MIN reset and nothing for the unchanged axis");

    // A real .fig instance stores no children, so the copy is rebuilt from the component plus the written overrides.
    let (stripped, _) = wire_instance_override(&bytes, "AlignInst", None);
    let rt = import_fig(&stripped).expect("re-import OverrideAlignDoc");
    assert_eq!(instance_child(&rt), (GridChildAlign::Start, GridChildAlign::End), "the rebuilt copy kept the component's alignment");

    // The untouched axis keeps following the main component: after the main changes to CENTER/MIN the rebuilt copy is
    // still reset horizontally (Start) and now follows the main vertically (Start, no longer End).
    let (stripped, _) = wire_instance_override(&bytes, "AlignInst", Some(("AlignedChild", "CENTER", "MIN")));
    let rt = import_fig(&stripped).expect("re-import OverrideAlignDoc after the main changed");
    assert_eq!(instance_child(&rt), (GridChildAlign::Start, GridChildAlign::Start), "the unchanged axis froze the main's old value");
}

#[test]
fn instance_override_of_a_margin_leaves_the_grid_child_alignment_following_the_main_component() {
    use designos_doc::GridChildAlign;

    let g = |s: u64, l: u64| -> Value { json!({"sessionID": s, "localID": l}) };
    let node = |id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str, extra: Value| -> Value {
        let mut v = json!({
            "guid": g(id.0, id.1),
            "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
            "type": ty,
            "name": name,
            "phase": "CREATED",
            "visible": true,
            "opacity": 1.0,
            "size": {"x": 40.0, "y": 20.0},
            "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
        });
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        v
    };
    // The main child is centred horizontally and ends vertically; the instance only sets a left margin. A margin takes
    // the same Layout override flag as an alignment, but it names no alignment, so the alignment stays the main's.
    let changes = json!([
        {"guid": g(0, 0), "type": "DOCUMENT", "name": "MarginOverrideDoc"},
        {"guid": g(0, 1), "parentIndex": {"guid": g(0, 0), "position": "a"}, "type": "CANVAS", "name": "Page 1"},
        node(
            (1, 10),
            (0, 1),
            "a",
            "SYMBOL",
            "AlignComp",
            json!({
                "size": {"x": 100.0, "y": 100.0},
                "stackMode": "GRID",
                "gridColumns": 1,
                "gridRows": 1,
                "gridColumnsSizing": "100px",
                "gridRowsSizing": "100px"
            })
        ),
        node(
            (1, 11),
            (1, 10),
            "a",
            "RECTANGLE",
            "AlignedChild",
            json!({
                "overrideKey": g(9, 11),
                "gridChildHorizontalAlign": "CENTER",
                "gridChildVerticalAlign": "MAX"
            })
        ),
        node(
            (1, 12),
            (0, 1),
            "b",
            "INSTANCE",
            "AlignInst",
            json!({
                "size": {"x": 100.0, "y": 100.0},
                "symbolData": {
                    "symbolID": g(1, 10),
                    "symbolOverrides": [
                        {"guidPath": {"guids": [g(9, 11)]}, "stackChildMarginLeft": 8.0}
                    ]
                }
            })
        )
    ]);
    let instance_child = |doc: &Document| {
        let inst = find(doc, "AlignInst");
        let child = doc.get(*inst.children.first().expect("instance child")).expect("instance child node");
        (child.layout_child.grid_align_h, child.layout_child.grid_align_v, child.layout_child.margin[3])
    };

    let doc = import_fig_node_changes(&changes, "MarginOverrideDoc").expect("import MarginOverrideDoc");
    assert_eq!(instance_child(&doc), (GridChildAlign::Center, GridChildAlign::End, 8.0), "the margin applies on import, the alignment is the main's");

    let bytes = export_fig(&doc).expect("export_fig MarginOverrideDoc");
    let (_, overrides) = wire_instance_override(&bytes, "AlignInst", None);
    assert_eq!(overrides, ("<absent>".to_string(), "<absent>".to_string()), "a margin override must not write the alignment");

    // The main child changes to MIN/MIN and the instance is rebuilt: it follows, and keeps its own margin.
    let (stripped, _) = wire_instance_override(&bytes, "AlignInst", Some(("AlignedChild", "MIN", "MIN")));
    let rt = import_fig(&stripped).expect("re-import MarginOverrideDoc after the main changed");
    assert_eq!(instance_child(&rt), (GridChildAlign::Start, GridChildAlign::Start, 8.0), "the alignment froze at the main's old CENTER/MAX");
}

/// The `.fig` bytes with the children stored under instance `instance_name` removed, and the
/// (`gridChildHorizontalAlign`, `gridChildVerticalAlign`) of the first `symbolOverrides` entry that carries either key
/// (`<absent>` for a key it does not carry). `main_edit` rewrites both alignment keys of the remaining node of that
/// name (the main component's child) to the given `(name, horizontal, vertical)`, as if the main had changed.
fn wire_instance_override(bytes: &[u8], instance_name: &str, main_edit: Option<(&str, &str, &str)>) -> (Vec<u8>, (String, String)) {
    use designos_fig::container::{self, Compression};
    use designos_fig::kiwi::{KiwiValue, Schema};
    let archive = container::open(bytes).expect("open fig archive");
    let (schema_bytes, message) = container::read_archive_chunks(&archive).expect("fig chunks");
    let schema = Schema::decode(&schema_bytes).expect("fig schema");
    let root = schema.root_message().expect("root message");
    let mut msg = schema.decode_message(root, &message).expect("decode fig message");
    let Some(KiwiValue::Array(nodes)) = msg.take_field("nodeChanges") else { panic!("no nodeChanges") };
    let guid_of = |v: Option<&KiwiValue>| match (v.and_then(|g| g.get("sessionID")), v.and_then(|g| g.get("localID"))) {
        (Some(KiwiValue::Uint(s)), Some(KiwiValue::Uint(l))) => Some((*s, *l)),
        _ => None,
    };
    let instance = nodes
        .iter()
        .find(|n| matches!(n.get("name"), Some(KiwiValue::String(s)) if s == instance_name))
        .and_then(|n| guid_of(n.get("guid")))
        .expect("instance guid");
    let enum_of = |n: &KiwiValue, key: &str| match n.get(key) {
        Some(KiwiValue::Enum(e)) => e.to_string(),
        Some(other) => format!("{other:?}"),
        None => "<absent>".to_string(),
    };
    let mut carried = ("<absent>".to_string(), "<absent>".to_string());
    for n in &nodes {
        let entries = n.get("symbolData").and_then(|s| s.get("symbolOverrides")).and_then(KiwiValue::as_array).unwrap_or(&[]);
        for e in entries {
            let pair = (enum_of(e, "gridChildHorizontalAlign"), enum_of(e, "gridChildVerticalAlign"));
            if pair != ("<absent>".to_string(), "<absent>".to_string()) {
                carried = pair;
            }
        }
    }
    let mut kept: Vec<KiwiValue> = nodes.into_iter().filter(|n| guid_of(n.get("parentIndex").and_then(|p| p.get("guid"))) != Some(instance)).collect();
    if let Some((name, h, v)) = main_edit {
        let main = kept.iter_mut().find(|n| matches!(n.get("name"), Some(KiwiValue::String(s)) if s == name)).expect("main component child");
        let KiwiValue::Message(main_fields) = main else { panic!("node is not a message") };
        for (key, value) in [("gridChildHorizontalAlign", h), ("gridChildVerticalAlign", v)] {
            let value = KiwiValue::Enum(value.into());
            match main_fields.iter_mut().find(|(k, _)| &**k == key) {
                Some(slot) => slot.1 = value,
                None => main_fields.push((key.into(), value)),
            }
        }
    }
    let KiwiValue::Message(fields) = &mut msg else { panic!("root is not a message") };
    fields.push(("nodeChanges".into(), KiwiValue::Array(kept)));
    let encoded = schema.encode_message(root, &msg).expect("encode fig message");
    let version = container::container_version(&archive.canvas).unwrap_or(106);
    (container::write_container(&schema_bytes, &encoded, Compression::Deflate, version), carried)
}

#[test]
fn passthrough_color_struct_array_and_quoted_string_roundtrip() {
    let g = |s: u64, l: u64| -> Value { json!({"sessionID": s, "localID": l}) };
    let changes = json!([
        {"guid": g(0, 0), "type": "DOCUMENT", "name": "PassthroughHardeningDoc"},
        {"guid": g(0, 1), "parentIndex": {"guid": g(0, 0), "position": "a"}, "type": "CANVAS", "name": "Page 1"},
        {
            "guid": g(1, 10),
            "parentIndex": {"guid": g(0, 1), "position": "a"},
            "type": "SLIDE",
            "name": "SlideNode",
            "phase": "CREATED",
            "visible": true,
            "opacity": 1.0,
            "size": {"x": 200.0, "y": 100.0},
            "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0},
            "slideSpeakerNotes": "\"quoted\"",
            "keyframeValue": {
                "value": {"colorValue": {"r": 1.0, "g": 0.0, "b": 0.0}},
                "valueType": "COLOR"
            },
            "variableWidthPoints": [
                {"position": 0.25, "ascent": 2.0, "descent": 4.0},
                {"position": "nope"}
            ]
        }
    ]);

    let doc = import_fig_node_changes(&changes, "PassthroughHardeningDoc").expect("import PassthroughHardeningDoc");
    let bytes = export_fig(&doc).expect("export_fig PassthroughHardeningDoc");
    let rt = import_fig(&bytes).expect("re-import PassthroughHardeningDoc");
    let slide = find(&rt, "SlideNode");

    // 1. A speaker note stored as `"quoted"` must keep its leading and trailing quotes.
    assert_eq!(slide.fig_metadata.get("slideSpeakerNotes").map(String::as_str), Some("\"quoted\""));

    // 2. A passthrough Color struct without `a` defaults `a` to 1.0 (opaque), not 0.0.
    let kf_raw = slide.fig_metadata.get("keyframeValue").expect("keyframeValue in fig_metadata");
    let kf_json: Value = serde_json::from_str(kf_raw).expect("parse keyframeValue JSON");
    assert_eq!(kf_json["value"]["colorValue"]["r"].as_f64(), Some(1.0));
    assert_eq!(kf_json["value"]["colorValue"]["a"].as_f64(), Some(1.0), "Color.a must default to 1.0 when omitted: {kf_json}");

    // 3. One malformed element in an array must be skipped without dropping the valid elements.
    let vwp_raw = slide.fig_metadata.get("variableWidthPoints").expect("variableWidthPoints kept in fig_metadata");
    let vwp_json: Value = serde_json::from_str(vwp_raw).expect("parse variableWidthPoints JSON");
    let arr = vwp_json.as_array().expect("variableWidthPoints array");
    assert_eq!(arr.len(), 1, "valid element must survive when a sibling element is malformed: {arr:?}");
    assert_eq!(arr[0]["position"].as_f64(), Some(0.25));
    assert_eq!(arr[0]["ascent"].as_f64(), Some(2.0));
    assert_eq!(arr[0]["descent"].as_f64(), Some(4.0));
}

#[test]
fn per_side_stroke_weights_ignore_stale_borders_and_merge_partial_overrides() {
    let g = |s: u64, l: u64| -> Value { json!({"sessionID": s, "localID": l}) };
    let node = |id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str, extra: Value| -> Value {
        let mut v = json!({
            "guid": g(id.0, id.1),
            "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
            "type": ty,
            "name": name,
            "phase": "CREATED",
            "visible": true,
            "opacity": 1.0,
            "size": {"x": 100.0, "y": 50.0},
            "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
        });
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        v
    };
    let changes = json!([
        node((0, 0), (0, 0), "!", "DOCUMENT", "Doc", json!({})),
        node((1, 1), (0, 0), "!", "CANVAS", "Page 1", json!({})),
        // 1. strokeWeight 1, border*Weight 12, no flag -> side_weights is None.
        node(
            (1, 10),
            (1, 1),
            "!",
            "FRAME",
            "UniformPill",
            json!({
                "strokeWeight": 1.0,
                "borderTopWeight": 12.0,
                "borderRightWeight": 12.0,
                "borderBottomWeight": 12.0,
                "borderLeftWeight": 12.0
            })
        ),
        // 2. flag on, only borderRightWeight 1 -> [0, 1, 0, 0] even if strokeWeight is 1.
        node(
            (1, 20),
            (1, 1),
            "\"",
            "FRAME",
            "RightOnlyDivider",
            json!({
                "strokeWeight": 1.0,
                "borderStrokeWeightsIndependent": true,
                "borderRightWeight": 1.0
            })
        ),
        // 3. Component with borderStrokeWeightsIndependent: true, borderRightWeight: 1, borderBottomWeight: 1,
        // and an instance whose override sets only borderBottomWeight: 0 on that node -> [0, 1, 0, 0].
        node(
            (1, 30),
            (1, 1),
            "#",
            "SYMBOL",
            "CalendarHeader",
            json!({
                "strokeWeight": 1.0,
                "borderStrokeWeightsIndependent": true,
                "borderRightWeight": 1.0,
                "borderBottomWeight": 1.0
            })
        ),
        node(
            (1, 31),
            (1, 30),
            "!",
            "FRAME",
            "CellDivider",
            json!({
                "overrideKey": g(9, 31),
                "strokeWeight": 1.0,
                "borderStrokeWeightsIndependent": true,
                "borderRightWeight": 1.0,
                "borderBottomWeight": 1.0
            })
        ),
        node(
            (1, 40),
            (1, 1),
            "$",
            "INSTANCE",
            "CalendarHeaderInst",
            json!({
                "symbolData": {
                    "symbolID": g(1, 30),
                    "symbolOverrides": [
                        {
                            "guidPath": {"guids": [g(9, 31)]},
                            "borderBottomWeight": 0.0
                        }
                    ]
                }
            })
        )
    ]);

    let doc = import_fig_node_changes(&changes, "SideWeights515").expect("import_fig_node_changes");

    let pill = find(&doc, "UniformPill");
    assert_eq!(pill.stroke.weight, 1.0);
    assert_eq!(pill.stroke.side_weights, None, "stale border*Weight without borderStrokeWeightsIndependent must not set side_weights");

    let right_only = find(&doc, "RightOnlyDivider");
    assert_eq!(right_only.stroke.side_weights, Some([0.0, 1.0, 0.0, 0.0]), "missing side with flag on must default to 0");

    let inst = find(&doc, "CalendarHeaderInst");
    let cell = inst.children.iter().filter_map(|id| doc.get(*id)).find(|n| n.name == "CellDivider").expect("CellDivider in CalendarHeaderInst");
    assert_eq!(cell.stroke.side_weights, Some([0.0, 1.0, 0.0, 0.0]), "partial borderBottomWeight override must preserve borderRightWeight");
}

#[test]
fn radial_gradient_transform_imports_centred_handles_and_roundtrips() {
    let g = |s: u64, l: u64| -> Value { json!({"sessionID": s, "localID": l}) };
    // 90° clockwise gradient-space transform from Untitled UI `_Background mask` (4932:396988):
    // centered at (0.5, 0.5) with radius 0.5 in normalized node coordinates.
    let rot90 = json!({"m00": 0.0, "m01": 1.0, "m02": 0.0, "m10": -1.0, "m11": 0.0, "m12": 1.0});
    let stops = json!([
        {"position": 0.0, "color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 1.0}},
        {"position": 1.0, "color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 0.0}}
    ]);
    let changes = json!([
        {"guid": g(0, 0), "type": "DOCUMENT", "name": "RadialDoc"},
        {"guid": g(0, 1), "parentIndex": {"guid": g(0, 0), "position": "!"}, "type": "CANVAS", "name": "Page 1"},
        {
            "guid": g(1, 10),
            "parentIndex": {"guid": g(0, 1), "position": "!"},
            "type": "RECTANGLE",
            "name": "BackgroundMask",
            "phase": "CREATED",
            "visible": true,
            "opacity": 1.0,
            "size": {"x": 200.0, "y": 100.0},
            "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0},
            "fillPaints": [
                {
                    "type": "GRADIENT_RADIAL",
                    "opacity": 1.0,
                    "visible": true,
                    "blendMode": "NORMAL",
                    "transform": rot90,
                    "stops": stops
                },
                {
                    "type": "GRADIENT_ANGULAR",
                    "opacity": 1.0,
                    "visible": true,
                    "blendMode": "NORMAL",
                    "transform": rot90,
                    "stops": stops
                },
                {
                    "type": "GRADIENT_DIAMOND",
                    "opacity": 1.0,
                    "visible": true,
                    "blendMode": "NORMAL",
                    "transform": rot90,
                    "stops": stops
                }
            ]
        }
    ]);

    let doc = import_fig_node_changes(&changes, "RadialDoc").expect("import RadialDoc");
    let check = |doc: &Document| {
        let mask = find(doc, "BackgroundMask");
        assert_eq!(mask.fills.len(), 3);
        let expected = [Vec2::new(0.5, 0.5), Vec2::new(0.5, 1.0), Vec2::new(0.0, 0.5)];
        for (i, fill) in mask.fills.iter().enumerate() {
            let handles = match &fill.kind {
                PaintKind::RadialGradient { handles, .. } | PaintKind::AngularGradient { handles, .. } | PaintKind::DiamondGradient { handles, .. } => handles,
                other => panic!("fill {i}: expected radial/angular/diamond gradient, got {other:?}"),
            };
            for (h, e) in handles.iter().zip(expected.iter()) {
                assert!((h.x - e.x).abs() < 1e-5 && (h.y - e.y).abs() < 1e-5, "fill {i}: got {handles:?}, expected {expected:?}");
            }
        }
    };
    check(&doc);

    let bytes = export_fig(&doc).expect("export_fig RadialDoc");
    let rt = import_fig(&bytes).expect("re-import RadialDoc");
    check(&rt);
}
