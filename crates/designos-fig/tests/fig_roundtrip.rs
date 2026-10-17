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
    let al = f.auto_layout.expect("Card has auto_layout");
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
    let imp_al = imp_comp.kind.frame_props().unwrap().auto_layout.expect("auto_layout");
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
        // GAP-01 & GAP-02 & GAP-03: Frame with per-side border weights WITHOUT borderStrokeWeightsIndependent
        // and per-corner radii WITHOUT rectangleCornerRadiiIndependent, plus variableConsumptionMap
        node(
            (1, 10),
            (1, 1),
            "a",
            "FRAME",
            "HeaderBar",
            json!({
                "strokeWeight": 1.0,
                "borderTopWeight": 0.0,
                "borderRightWeight": 0.0,
                "borderBottomWeight": 2.0,
                "borderLeftWeight": 0.0,
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
                    "endpointNodeId": g(1, 41),
                    "position": {"x": 100.0, "y": 25.0},
                    "magnet": "RIGHT"
                },
                "connectorEnd": {
                    "endpointNodeId": g(1, 42),
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
    let grid_al = grid.kind.frame_props().unwrap().auto_layout.unwrap();
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
    let btn_inst_al = btn_inst.kind.frame_props().unwrap().auto_layout.unwrap();
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
    let rt_al = rt_grid.kind.frame_props().unwrap().auto_layout.unwrap();
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
            node((1, 10), (0, 1), "!", "BRUSH", "CharcoalStroke", json!({
                "brushType": "CHARCOAL",
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
    let brush = find(&doc, "CharcoalStroke");
    assert_eq!(brush.fig_metadata.get("brushType").map(String::as_str), Some("CHARCOAL"));
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

    let rt_brush = find(&rt, "CharcoalStroke");
    assert_eq!(rt_brush.fig_metadata.get("brushType").map(String::as_str), Some("CHARCOAL"));

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
