use designos_doc::*;

#[test]
fn pillar1_suggest_auto_layout_1d_row_and_column() {
    let mut doc = Document::new("Pillar 1 — 1D Suggest Auto-Layout");
    let page = doc.add_page("Page 1");

    // 1. Three unstacked sibling rectangles arranged horizontally with a 16px gap.
    let r1 = doc.alloc_id();
    let r2 = doc.alloc_id();
    let r3 = doc.alloc_id();
    // Insert out of X-order to verify spatial left-to-right sorting.
    doc.insert(page, Node::rectangle(r2, Rect::new(136.0, 20.0, 100.0, 40.0))).unwrap();
    doc.insert(page, Node::rectangle(r1, Rect::new(20.0, 20.0, 100.0, 40.0))).unwrap();
    doc.insert(page, Node::rectangle(r3, Rect::new(252.0, 20.0, 100.0, 40.0))).unwrap();

    let res_row = doc.suggest_auto_layout(&[r2, r1, r3]).unwrap();
    assert_eq!(res_row.kind, SuggestedLayoutKind::HorizontalRow);
    assert_eq!(res_row.primary_spacing, 16.0);
    assert!(res_row.created_row_frames.is_empty());

    let row_frame = doc.get(res_row.root_frame).unwrap();
    assert_eq!(row_frame.children, vec![r1, r2, r3]);
    assert_eq!(row_frame.transform.translation(), Vec2::new(20.0, 20.0));
    assert_eq!(row_frame.size, Vec2::new(332.0, 40.0));
    assert_eq!(doc.get(r1).unwrap().transform.translation(), Vec2::new(0.0, 0.0));
    assert_eq!(doc.get(r2).unwrap().transform.translation(), Vec2::new(116.0, 0.0));
    assert_eq!(doc.get(r3).unwrap().transform.translation(), Vec2::new(232.0, 0.0));

    // 2. Three unstacked sibling rectangles arranged vertically with a 12px gap.
    let c1 = doc.alloc_id();
    let c2 = doc.alloc_id();
    let c3 = doc.alloc_id();
    doc.insert(page, Node::rectangle(c3, Rect::new(50.0, 114.0, 120.0, 40.0))).unwrap();
    doc.insert(page, Node::rectangle(c1, Rect::new(50.0, 10.0, 120.0, 40.0))).unwrap();
    doc.insert(page, Node::rectangle(c2, Rect::new(50.0, 62.0, 120.0, 40.0))).unwrap();

    let res_col = doc.suggest_auto_layout(&[c3, c1, c2]).unwrap();
    assert_eq!(res_col.kind, SuggestedLayoutKind::VerticalColumn);
    assert_eq!(res_col.primary_spacing, 12.0);

    let col_frame = doc.get(res_col.root_frame).unwrap();
    assert_eq!(col_frame.children, vec![c1, c2, c3]);
    assert_eq!(col_frame.size, Vec2::new(120.0, 144.0));
    assert_eq!(doc.get(c1).unwrap().transform.translation(), Vec2::new(0.0, 0.0));
    assert_eq!(doc.get(c2).unwrap().transform.translation(), Vec2::new(0.0, 52.0));
    assert_eq!(doc.get(c3).unwrap().transform.translation(), Vec2::new(0.0, 104.0));

    // 3. Converting an existing Frame with internal margins infers [top, right, bottom, left] padding.
    let container_id = doc.alloc_id();
    doc.insert(page, Node::frame(container_id, Rect::new(0.0, 0.0, 260.0, 88.0))).unwrap();
    let b1 = doc.alloc_id();
    let b2 = doc.alloc_id();
    doc.insert(container_id, Node::rectangle(b1, Rect::new(24.0, 16.0, 100.0, 56.0))).unwrap();
    doc.insert(container_id, Node::rectangle(b2, Rect::new(136.0, 16.0, 100.0, 56.0))).unwrap();

    let res_in_place = doc.suggest_auto_layout(&[container_id]).unwrap();
    assert_eq!(res_in_place.root_frame, container_id);
    assert_eq!(res_in_place.kind, SuggestedLayoutKind::HorizontalRow);
    assert_eq!(res_in_place.primary_spacing, 12.0);
    assert_eq!(res_in_place.padding, [16.0, 24.0, 16.0, 24.0]);
}

#[test]
fn pillar1_suggest_auto_layout_2d_column_of_rows() {
    let mut doc = Document::new("Pillar 1 — 2D Suggest Auto-Layout");
    let page = doc.add_page("Page 1");

    // 4 cards arranged in a 2x2 grid:
    // Row 1: (10, 10) and (130, 10) [w=100, h=60, horiz gap = 20]
    // Row 2: (10, 94) and (130, 94) [w=100, h=60, horiz gap = 20, vert gap = 24]
    let n00 = doc.alloc_id();
    let n01 = doc.alloc_id();
    let n10 = doc.alloc_id();
    let n11 = doc.alloc_id();
    doc.insert(page, Node::rectangle(n11, Rect::new(130.0, 94.0, 100.0, 60.0))).unwrap();
    doc.insert(page, Node::rectangle(n00, Rect::new(10.0, 10.0, 100.0, 60.0))).unwrap();
    doc.insert(page, Node::rectangle(n10, Rect::new(10.0, 94.0, 100.0, 60.0))).unwrap();
    doc.insert(page, Node::rectangle(n01, Rect::new(130.0, 10.0, 100.0, 60.0))).unwrap();

    let res = doc.suggest_auto_layout(&[n11, n00, n10, n01]).unwrap();
    assert_eq!(res.kind, SuggestedLayoutKind::NestedColumnOfRows { row_count: 2 });
    assert_eq!(res.primary_spacing, 24.0);
    assert_eq!(res.created_row_frames.len(), 2);

    let outer = doc.get(res.root_frame).unwrap();
    assert_eq!(outer.transform.translation(), Vec2::new(10.0, 10.0));
    assert_eq!(outer.size, Vec2::new(220.0, 144.0));

    let row1_id = res.created_row_frames[0];
    let row2_id = res.created_row_frames[1];
    let row1 = doc.get(row1_id).unwrap();
    let row2 = doc.get(row2_id).unwrap();

    assert_eq!(row1.children, vec![n00, n01]);
    assert_eq!(row2.children, vec![n10, n11]);
    assert_eq!(row1.transform.translation(), Vec2::new(0.0, 0.0));
    assert_eq!(row2.transform.translation(), Vec2::new(0.0, 84.0));
    assert_eq!(row1.size, Vec2::new(220.0, 60.0));
    assert_eq!(row2.size, Vec2::new(220.0, 60.0));
    assert_eq!(row1.kind.frame_props().unwrap().auto_layout.as_ref().unwrap().item_spacing, 20.0);
}

#[test]
fn pillar2_live_non_destructive_boolean_groups() {
    let mut doc = Document::new("Pillar 2 — Live Boolean Groups");
    let page = doc.add_page("Page 1");

    // 1. Live Union group: two 100x100 rectangles at (0,0) and (50,50).
    let a = doc.alloc_id();
    let b = doc.alloc_id();
    doc.insert(page, Node::rectangle(a, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
    doc.insert(page, Node::rectangle(b, Rect::new(50.0, 50.0, 100.0, 100.0))).unwrap();

    let union_id = doc.create_boolean_group(BooleanOp::Union, &[a, b]).unwrap();
    let union_node = doc.get(union_id).unwrap();
    assert_eq!(union_node.children, vec![a, b], "children must remain non-destructively in tree");
    assert_eq!(union_node.size, Vec2::new(150.0, 150.0));
    assert!(union_node.fill_geometry.as_ref().is_some_and(|p| !p.is_empty()));

    // Move child `b` to (100, 120) with size (80, 80) -> Union group recomputes to (180, 200).
    let updated_rect = update_boolean_child_rect(&mut doc, b, Rect::new(100.0, 120.0, 80.0, 80.0)).unwrap().unwrap();
    assert_eq!(updated_rect, Rect::new(0.0, 0.0, 180.0, 200.0));
    assert_eq!(doc.get(union_id).unwrap().size, Vec2::new(180.0, 200.0));

    // 2. Live Subtract group: 100x100 base at (0,0) minus 60x100 cutter at (40,0) -> (0,0,40,100).
    let s_base = doc.alloc_id();
    let s_cut = doc.alloc_id();
    doc.insert(page, Node::rectangle(s_base, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
    doc.insert(page, Node::rectangle(s_cut, Rect::new(40.0, 0.0, 60.0, 100.0))).unwrap();

    let sub_id = doc.create_boolean_group(BooleanOp::Subtract, &[s_base, s_cut]).unwrap();
    assert_eq!(doc.get(sub_id).unwrap().size, Vec2::new(40.0, 100.0));

    // Move cutter to (70, 0) -> remaining base widens to 70x100.
    let sub_rect = update_boolean_child_rect(&mut doc, s_cut, Rect::new(70.0, 0.0, 60.0, 100.0)).unwrap().unwrap();
    assert_eq!(sub_rect.w, 70.0);
    assert_eq!(sub_rect.h, 100.0);

    // 3. Live Intersect group: (0,0,100,100) ∩ (40,30,100,100) -> (40,30,60,70).
    let i1 = doc.alloc_id();
    let i2 = doc.alloc_id();
    doc.insert(page, Node::rectangle(i1, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
    doc.insert(page, Node::rectangle(i2, Rect::new(40.0, 30.0, 100.0, 100.0))).unwrap();

    let inter_id = doc.create_boolean_group(BooleanOp::Intersect, &[i1, i2]).unwrap();
    let inter_node = doc.get(inter_id).unwrap();
    assert_eq!(inter_node.transform.translation(), Vec2::new(40.0, 30.0));
    assert_eq!(inter_node.size, Vec2::new(60.0, 70.0));

    // 4. Live Exclude group: preserves children and produces EvenOdd fill geometry.
    let e1 = doc.alloc_id();
    let e2 = doc.alloc_id();
    doc.insert(page, Node::rectangle(e1, Rect::new(0.0, 0.0, 80.0, 80.0))).unwrap();
    doc.insert(page, Node::rectangle(e2, Rect::new(40.0, 40.0, 80.0, 80.0))).unwrap();
    let excl_id = doc.create_boolean_group(BooleanOp::Exclude, &[e1, e2]).unwrap();
    let excl_node = doc.get(excl_id).unwrap();
    assert_eq!(excl_node.size, Vec2::new(120.0, 120.0));
    assert_eq!(excl_node.fill_geometry.as_ref().unwrap()[0].fill_rule, FillRule::EvenOdd);
}

#[test]
fn pillar3_variable_bindings_and_subtree_mode_switching() {
    let mut doc = Document::new("Pillar 3 — Variable Bindings & Mode Switching");
    let page = doc.add_page("Page 1");

    // Define collection "density_theme" with modes "compact" and "spacious".
    doc.variable_collections.insert(
        "coll_theme".into(),
        VariableCollection {
            id: "coll_theme".into(),
            name: "Density & Theme".into(),
            modes: vec![
                VariableMode { id: "compact".into(), name: "Compact Light".into() },
                VariableMode { id: "spacious".into(), name: "Spacious Dark".into() },
            ],
            default_mode: "compact".into(),
            variables: vec![
                "v_pad".into(),
                "v_gap".into(),
                "v_radius".into(),
                "v_badge_w".into(),
                "v_badge_h".into(),
                "v_opacity".into(),
                "v_title".into(),
                "v_bg".into(),
            ],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );

    let mut insert_var = |id: &str, kind: VariableKind, compact: VariableValue, spacious: VariableValue| {
        let mut values = std::collections::BTreeMap::new();
        values.insert("compact".into(), compact);
        values.insert("spacious".into(), spacious);
        doc.variables.insert(
            id.into(),
            Variable {
                id: id.into(),
                name: id.into(),
                collection: "coll_theme".into(),
                kind,
                values,
                description: String::new(),
                scopes: Vec::new(),
                hidden_from_publishing: false,
                key: None,

                code_syntax: Default::default(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );
    };

    insert_var("v_pad", VariableKind::Number, VariableValue::Number(12.0), VariableValue::Number(24.0));
    insert_var("v_gap", VariableKind::Number, VariableValue::Number(8.0), VariableValue::Number(20.0));
    insert_var("v_radius", VariableKind::Number, VariableValue::Number(6.0), VariableValue::Number(16.0));
    insert_var("v_badge_w", VariableKind::Number, VariableValue::Number(80.0), VariableValue::Number(140.0));
    insert_var("v_badge_h", VariableKind::Number, VariableValue::Number(28.0), VariableValue::Number(44.0));
    insert_var("v_opacity", VariableKind::Number, VariableValue::Number(80.0), VariableValue::Number(100.0));
    insert_var("v_title", VariableKind::String, VariableValue::String("Compact Card".into()), VariableValue::String("Spacious Card".into()));
    insert_var("v_bg", VariableKind::Color, VariableValue::Color(Color::WHITE), VariableValue::Color(Color::rgb(0x18, 0x18, 0x1B)));

    // Create an Auto-Layout card frame with a badge rectangle and a title text node.
    let card_id = doc.alloc_id();
    let mut card = Node::frame(card_id, Rect::new(0.0, 0.0, 200.0, 100.0));
    card.kind = NodeKind::Frame(FrameProps {
        clips_content: false,
        auto_layout: Some(AutoLayout { direction: LayoutDirection::Vertical, primary_sizing: Sizing::Hug, counter_sizing: Sizing::Hug, ..Default::default() }),
        grids: Vec::new(),
    });
    doc.insert(page, card).unwrap();

    let badge_id = doc.alloc_id();
    doc.insert(card_id, Node::rectangle(badge_id, Rect::new(0.0, 0.0, 50.0, 20.0))).unwrap();

    let title_id = doc.alloc_id();
    doc.insert(card_id, Node::text(title_id, Rect::new(0.0, 0.0, 100.0, 24.0), "Placeholder", TextStyle::default())).unwrap();

    // Bind variables on card, badge, and title.
    doc.bind_node_variable(card_id, BindableProperty::Fill(0), "v_bg").unwrap();
    doc.bind_node_variable(card_id, BindableProperty::PaddingTop, "v_pad").unwrap();
    doc.bind_node_variable(card_id, BindableProperty::PaddingRight, "v_pad").unwrap();
    doc.bind_node_variable(card_id, BindableProperty::PaddingBottom, "v_pad").unwrap();
    doc.bind_node_variable(card_id, BindableProperty::PaddingLeft, "v_pad").unwrap();
    doc.bind_node_variable(card_id, BindableProperty::ItemSpacing, "v_gap").unwrap();
    doc.bind_node_variable(card_id, BindableProperty::CornerRadius, "v_radius").unwrap();

    doc.bind_node_variable(badge_id, BindableProperty::Width, "v_badge_w").unwrap();
    doc.bind_node_variable(badge_id, BindableProperty::Height, "v_badge_h").unwrap();
    doc.bind_node_variable(badge_id, BindableProperty::Opacity, "v_opacity").unwrap();

    doc.bind_node_variable(title_id, BindableProperty::TextContent, "v_title").unwrap();

    // Verify default "compact" mode values and layout:
    // card width = pad_left(12) + max(80, 100) + pad_right(12) = 124
    // card height = pad_top(12) + badge_h(28) + gap(8) + title_h(24) + pad_bottom(12) = 84
    assert_eq!(doc.get(card_id).unwrap().size, Vec2::new(124.0, 84.0));
    assert_eq!(doc.get(card_id).unwrap().corner_radii, [6.0; 4]);
    assert_eq!(doc.get(badge_id).unwrap().size, Vec2::new(80.0, 28.0));
    assert!((doc.get(badge_id).unwrap().opacity - 0.80).abs() < 1e-5);
    assert_eq!(doc.get(title_id).unwrap().transform.translation(), Vec2::new(12.0, 48.0));
    assert_eq!(doc.get(title_id).unwrap().kind.any_text().unwrap().content, "Compact Card");

    // Switch subtree to "spacious" mode!
    let updated = doc.apply_variable_mode_to_subtree(card_id, "coll_theme", "spacious").unwrap();
    assert_eq!(updated, 3);

    // In "spacious" mode:
    // pad = 24, gap = 20, radius = 16, badge = 140x44, opacity = 1.0, title = "Spacious Card"
    // card width = 24 + max(140, 100) + 24 = 188
    // card height = 24 + 44 + 20 + 24 + 24 = 136
    // title y = 24 + 44 + 20 = 88
    let card_spacious = doc.get(card_id).unwrap();
    assert_eq!(card_spacious.size, Vec2::new(188.0, 136.0));
    assert_eq!(card_spacious.corner_radii, [16.0; 4]);
    assert_eq!(card_spacious.fills[0].kind, PaintKind::Solid(Color::rgb(0x18, 0x18, 0x1B)));

    let badge_spacious = doc.get(badge_id).unwrap();
    assert_eq!(badge_spacious.size, Vec2::new(140.0, 44.0));
    assert!((badge_spacious.opacity - 1.0).abs() < 1e-5);

    let title_spacious = doc.get(title_id).unwrap();
    assert_eq!(title_spacious.transform.translation(), Vec2::new(24.0, 88.0));
    assert_eq!(title_spacious.kind.any_text().unwrap().content, "Spacious Card");
}

#[test]
fn pillar4_interactive_prototype_runtime() {
    let mut doc = Document::new("Pillar 4 — Interactive Prototype Runtime");
    let page = doc.add_page("Page 1");

    // 1. Setup scrollable screen frame (300x200) with fixed header and overflowing content (y up to 500, x up to 450).
    let screen_id = doc.alloc_id();
    doc.insert(page, Node::frame(screen_id, Rect::new(0.0, 0.0, 300.0, 200.0))).unwrap();
    let header_id = doc.alloc_id();
    doc.insert(screen_id, Node::rectangle(header_id, Rect::new(0.0, 0.0, 300.0, 48.0))).unwrap();
    let tall_card = doc.alloc_id();
    doc.insert(screen_id, Node::rectangle(tall_card, Rect::new(0.0, 60.0, 450.0, 440.0))).unwrap();

    doc.get_mut(screen_id).unwrap().prototype.fixed_children.push(header_id);
    PrototypeRuntime::set_overflow_scroll_mode(&mut doc, screen_id, OverflowScrollMode::Vertical).unwrap();

    let mut rt = PrototypeRuntime::new(&doc, Some(screen_id));

    // In Vertical mode, horizontal scroll is clamped to 0, vertical scroll clamps at 500 - 200 = 300.
    let offset1 = rt.scroll_by(&doc, screen_id, Vec2::new(80.0, 120.0));
    assert_eq!(offset1, Vec2::new(0.0, 120.0));
    // Fixed header stays at (0, 0), scrollable card shifts from (0, 60) to (0, -60).
    assert_eq!(rt.effective_child_position(&doc, screen_id, header_id), Some(Vec2::new(0.0, 0.0)));
    assert_eq!(rt.effective_child_position(&doc, screen_id, tall_card), Some(Vec2::new(0.0, -60.0)));

    // Scrolling past max clamps to 300.0, scrolling negative clamps to 0.0.
    assert_eq!(rt.scroll_by(&doc, screen_id, Vec2::new(0.0, 999.0)), Vec2::new(0.0, 300.0));
    assert_eq!(rt.scroll_by(&doc, screen_id, Vec2::new(0.0, -999.0)), Vec2::new(0.0, 0.0));

    // Switch to Both scroll mode -> horizontal clamps at 450 - 300 = 150.
    PrototypeRuntime::set_overflow_scroll_mode(&mut doc, screen_id, OverflowScrollMode::Both).unwrap();
    assert_eq!(rt.scroll_by(&doc, screen_id, Vec2::new(500.0, 500.0)), Vec2::new(150.0, 300.0));

    // 2. Interactive Component Variant State Swap (Default -> Hover -> Pressed).
    let comp_default = doc.alloc_id();
    let comp_hover = doc.alloc_id();
    let mut c_def = Node::new(comp_default, NodeKind::Component(FrameProps::default()));
    c_def.name = "State=Default".into();
    c_def.size = Vec2::new(120.0, 40.0);
    c_def.fills = vec![Paint::solid(Color::rgb(0x25, 0x63, 0xEB))];

    let mut c_hov = Node::new(comp_hover, NodeKind::Component(FrameProps::default()));
    c_hov.name = "State=Hover".into();
    c_hov.size = Vec2::new(132.0, 40.0);
    c_hov.fills = vec![Paint::solid(Color::rgb(0x1D, 0x4E, 0xD8))];

    // Attach prototype reaction on default variant: WhileHovering -> ChangeTo(comp_hover).
    c_def.prototype.reactions.push(Reaction { trigger: Trigger::WhileHovering, actions: vec![Action::ChangeTo { dest: comp_hover }], transition: None });

    doc.insert(page, c_def).unwrap();
    doc.insert(page, c_hov).unwrap();
    combine_as_variants(&mut doc, &[comp_default, comp_hover]).apply(&mut doc).unwrap();

    // Instantiate `comp_default` inside `screen_id`.
    let create_inst_cmd = create_instance(&mut doc, comp_default, screen_id, Vec2::new(16.0, 16.0));
    create_inst_cmd.apply(&mut doc).unwrap();
    let inst_id = *doc.get(screen_id).unwrap().children.last().unwrap();
    assert_eq!(instance_component(doc.get(inst_id).unwrap()), Some(comp_default));

    // Trigger WhileHovering on the instance -> swaps to `comp_hover`!
    let actions = rt.trigger_variant_interaction(&mut doc, inst_id, &Trigger::WhileHovering).unwrap();
    assert_eq!(actions.len(), 1);
    assert_eq!(instance_component(doc.get(inst_id).unwrap()), Some(comp_hover));
    assert_eq!(doc.get(inst_id).unwrap().size, Vec2::new(132.0, 40.0));
    assert_eq!(doc.get(inst_id).unwrap().component.values.get("State"), Some(&PropValue::Variant("Hover".into())));

    // Swap back to Default via `swap_instance_variant`.
    swap_instance_variant(&mut doc, inst_id, "State", "Default").unwrap();
    assert_eq!(instance_component(doc.get(inst_id).unwrap()), Some(comp_default));

    // 3. Conditional Variable Evaluation (`SetVariable` + `Conditional`).
    doc.variable_collections.insert(
        "c_proto".into(),
        VariableCollection {
            id: "c_proto".into(),
            name: "Proto".into(),
            modes: vec![VariableMode { id: "m1".into(), name: "Default".into() }],
            default_mode: "m1".into(),
            variables: vec!["v_count".into(), "v_label".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    let mut make_proto_var = |id: &str, kind: VariableKind, val: VariableValue| {
        let mut values = std::collections::BTreeMap::new();
        values.insert("m1".into(), val);
        doc.variables.insert(
            id.into(),
            Variable {
                id: id.into(),
                name: id.into(),
                collection: "c_proto".into(),
                kind,
                values,
                description: String::new(),
                scopes: Vec::new(),
                hidden_from_publishing: false,
                key: None,

                code_syntax: Default::default(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );
    };
    make_proto_var("v_count", VariableKind::Number, VariableValue::Number(0.0));
    make_proto_var("v_label", VariableKind::String, VariableValue::String("Empty".into()));

    let status_text_id = doc.alloc_id();
    doc.insert(screen_id, Node::text(status_text_id, Rect::new(16.0, 80.0, 120.0, 24.0), "Empty", TextStyle::default())).unwrap();
    doc.bind_node_variable(status_text_id, BindableProperty::TextContent, "v_label").unwrap();

    // Add OnClick reaction to `inst_id`:
    // 1. v_count = v_count + 1
    // 2. if v_count >= 2 { v_label = "Unlocked" } else { v_label = "Keep clicking" }
    let inc_expr = Expression::binary(BinaryOp::Add, Expression::var("v_count"), Expression::number(1.0));
    let cond_expr = Expression::binary(BinaryOp::Ge, Expression::var("v_count"), Expression::number(2.0));
    doc.get_mut(inst_id).unwrap().prototype.reactions.push(Reaction {
        trigger: Trigger::OnClick,
        actions: vec![
            Action::SetVariable { variable: "v_count".into(), value: inc_expr },
            Action::Conditional {
                branches: vec![(cond_expr, vec![Action::SetVariable { variable: "v_label".into(), value: Expression::text("Unlocked") }])],
                otherwise: vec![Action::SetVariable { variable: "v_label".into(), value: Expression::text("Keep clicking") }],
            },
        ],
        transition: None,
    });

    // First click: v_count becomes 1 -> otherwise branch sets v_label = "Keep clicking".
    rt.trigger_variant_interaction(&mut doc, inst_id, &Trigger::OnClick).unwrap();
    assert_eq!(rt.get_variable_value(&doc, "v_count"), Some(VariableValue::Number(1.0)));
    assert_eq!(doc.get(status_text_id).unwrap().kind.any_text().unwrap().content, "Keep clicking");

    // Second click: v_count becomes 2 -> true branch sets v_label = "Unlocked".
    rt.trigger_variant_interaction(&mut doc, inst_id, &Trigger::OnClick).unwrap();
    assert_eq!(rt.get_variable_value(&doc, "v_count"), Some(VariableValue::Number(2.0)));
    assert_eq!(doc.get(status_text_id).unwrap().kind.any_text().unwrap().content, "Unlocked");
}
