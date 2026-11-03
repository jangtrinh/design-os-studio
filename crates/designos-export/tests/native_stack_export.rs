use std::process::Command;

use designos_doc::{
    AutoLayout, CodeSyntaxPlatform, Color, Document, FrameProps, GridTrack, LayoutAlign, LayoutDirection, Node, NodeId, NodeKind, Paint, Rect, Sizing,
    Variable, VariableCollection, VariableKind, VariableMode, VariableValue, Vec2,
};
use designos_export::{compose_for, swiftui_for};

fn new_stack(al: AutoLayout, size: Vec2) -> (Document, NodeId) {
    let mut doc = Document::new("native-stack-export");
    let page = doc.add_page("Page 1");
    let id = doc.alloc_id();
    let mut n = Node::frame(id, Rect::new(0.0, 0.0, size.x, size.y));
    n.name = "StackFrame".into();
    n.fills = vec![Paint::solid(Color::rgb(0xF5, 0xF5, 0xFA))];
    n.corner_radii = [8.0; 4];
    n.kind = NodeKind::Frame(FrameProps { clips_content: true, auto_layout: Some(al), ..Default::default() });
    let id = doc.insert(page, n).unwrap();
    (doc, id)
}

fn new_grid(al: AutoLayout, size: Vec2) -> (Document, NodeId) {
    let mut doc = Document::new("grid-native-fixtures");
    let page = doc.add_page("Page 1");
    let id = doc.alloc_id();
    let mut frame = Node::frame(id, Rect::new(0.0, 0.0, size.x, size.y));
    frame.name = "GridFrame".into();
    frame.fills.clear();
    frame.kind =
        NodeKind::Frame(FrameProps { clips_content: false, auto_layout: Some(AutoLayout { direction: LayoutDirection::Grid, ..al }), ..Default::default() });
    let grid = doc.insert(page, frame).unwrap();
    (doc, grid)
}

#[allow(clippy::too_many_arguments)]
fn add_box(doc: &mut Document, parent: NodeId, name: &str, w: f32, h: f32, sh: Sizing, sv: Sizing, rgb: (u8, u8, u8)) -> NodeId {
    let id = doc.alloc_id();
    let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, w, h));
    n.name = name.into();
    n.fills = vec![Paint::solid(Color::rgb(rgb.0, rgb.1, rgb.2))];
    n.layout_child.horizontal = sh;
    n.layout_child.vertical = sv;
    doc.insert(parent, n).unwrap()
}

fn add_item(doc: &mut Document, grid: NodeId, w: f32, h: f32, horiz: Sizing, vert: Sizing) -> NodeId {
    let idx = doc.get(grid).map(|n| n.children.len() + 1).unwrap_or(1);
    add_box(doc, grid, &format!("Item {idx}"), w, h, horiz, vert, (0x35, 0x82, 0xFF))
}

fn add_false_flag(doc: &mut Document, var_id: &str) {
    doc.variable_collections.insert(
        "c_flags".into(),
        VariableCollection {
            id: "c_flags".into(),
            name: "Flags".into(),
            modes: vec![VariableMode { id: "m".into(), name: "Default".into() }],
            default_mode: "m".into(),
            variables: vec![var_id.into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    doc.variables.insert(
        var_id.into(),
        Variable {
            id: var_id.into(),
            name: "Flags/Show".into(),
            collection: "c_flags".into(),
            kind: VariableKind::Boolean,
            values: [("m".to_string(), VariableValue::Boolean(false))].into_iter().collect(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,
            code_syntax: [(CodeSyntaxPlatform::Android, "Tokens.show".to_string())].into_iter().collect(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        },
    );
}

#[test]
fn fill_width_fixed_height_and_fixed_width_fill_height_emit_valid_swiftui_frames() {
    // 1. VStack with a Fill-width + Fixed-height child and a Fill-width + Fixed-height sub-container.
    let v_al = AutoLayout {
        direction: LayoutDirection::Vertical,
        item_spacing: 8.0,
        padding: [12.0; 4],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    let (mut doc, vstack) = new_stack(v_al, Vec2::new(320.0, 200.0));
    add_box(&mut doc, vstack, "FillWidthFixedHeightLeaf", 100.0, 44.0, Sizing::Fill, Sizing::Fixed, (0x33, 0x66, 0xCC));
    let sub_al = AutoLayout {
        direction: LayoutDirection::Horizontal,
        primary_align: LayoutAlign::Center,
        counter_align: LayoutAlign::Center,
        item_spacing: 6.0,
        primary_sizing: Sizing::Fill,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    let sub_id = doc.alloc_id();
    let mut sub = Node::frame(sub_id, Rect::new(0.0, 0.0, 100.0, 64.0));
    sub.name = "SubRow".into();
    sub.fills.clear();
    sub.layout_child.horizontal = Sizing::Fill;
    sub.layout_child.vertical = Sizing::Fixed;
    sub.kind = NodeKind::Frame(FrameProps { clips_content: false, auto_layout: Some(sub_al), ..Default::default() });
    let sub_id = doc.insert(vstack, sub).unwrap();
    add_box(&mut doc, sub_id, "FixedWidthFillHeightLeaf", 120.0, 20.0, Sizing::Fixed, Sizing::Fill, (0x1A, 0xB3, 0x80));
    doc.relayout_subtree(vstack);

    let swift = swiftui_for(&doc, vstack);
    assert!(
        swift.contains(".frame(maxWidth: .infinity, minHeight: 44, maxHeight: 44)"),
        "Fill-width + Fixed-height leaf must use minHeight/maxHeight overload:\n{swift}"
    );
    assert!(
        swift.contains(".frame(maxWidth: .infinity, minHeight: 64, maxHeight: 64)"),
        "Fill-width + Fixed-height container must use minHeight/maxHeight overload:\n{swift}"
    );
    assert!(
        swift.contains(".frame(minWidth: 120, maxWidth: 120, maxHeight: .infinity)"),
        "Fixed-width + Fill-height leaf must use minWidth/maxWidth overload:\n{swift}"
    );
    assert!(!swift.contains("maxWidth: .infinity, height:"), "Invalid .frame(maxWidth: .infinity, height:) found:\n{swift}");
    assert!(!swift.contains(".frame(width: 120, maxHeight: .infinity"), "Invalid .frame(width:, maxHeight: .infinity) found:\n{swift}");

    // 2. Fixed-width + Fill-height container with alignment inside an HStack.
    let h_al = AutoLayout {
        direction: LayoutDirection::Horizontal,
        item_spacing: 10.0,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    let (mut hdoc, hstack) = new_stack(h_al, Vec2::new(400.0, 180.0));
    let col_al = AutoLayout { direction: LayoutDirection::Vertical, primary_sizing: Sizing::Fixed, counter_sizing: Sizing::Fill, ..Default::default() };
    let col_id = hdoc.alloc_id();
    let mut col = Node::frame(col_id, Rect::new(0.0, 0.0, 140.0, 100.0));
    col.name = "SideCol".into();
    col.fills.clear();
    col.layout_child.horizontal = Sizing::Fixed;
    col.layout_child.vertical = Sizing::Fill;
    col.kind = NodeKind::Frame(FrameProps { clips_content: false, auto_layout: Some(col_al), ..Default::default() });
    let col_id = hdoc.insert(hstack, col).unwrap();
    add_box(&mut hdoc, col_id, "BothFillLeaf", 50.0, 50.0, Sizing::Fill, Sizing::Fill, (0x80, 0x4D, 0xE6));
    hdoc.relayout_subtree(hstack);

    let hswift = swiftui_for(&hdoc, hstack);
    assert!(
        hswift.contains(".frame(minWidth: 140, maxWidth: 140, maxHeight: .infinity, alignment: .topLeading)"),
        "Fixed-width + Fill-height container must use minWidth/maxWidth overload with alignment:\n{hswift}"
    );
    assert!(
        hswift.contains(".frame(maxWidth: .infinity, maxHeight: .infinity)"),
        "Fill + Fill child keeps .frame(maxWidth: .infinity, maxHeight: .infinity):\n{hswift}"
    );
}

#[test]
fn absolute_and_variable_hidden_children_in_stacks_are_emitted_in_overlay_out_of_flow() {
    // 1. Vertical stack (VStack / Column) with SpaceBetween, padding, 2 in-flow children,
    //    1 absolute child, and 1 variable-hidden child.
    let v_al = AutoLayout {
        direction: LayoutDirection::Vertical,
        primary_align: LayoutAlign::SpaceBetween,
        counter_align: LayoutAlign::Start,
        item_spacing: 8.0,
        padding: [16.0, 16.0, 16.0, 16.0],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    let (mut doc, vstack) = new_stack(v_al, Vec2::new(300.0, 200.0));
    add_false_flag(&mut doc, "flag_hide");

    let first = add_box(&mut doc, vstack, "FirstFlow", 100.0, 24.0, Sizing::Fixed, Sizing::Fixed, (0x1A, 0x33, 0x4D));
    let second = add_box(&mut doc, vstack, "SecondFlow", 100.0, 32.0, Sizing::Fixed, Sizing::Fixed, (0x33, 0x4D, 0x66));
    let badge = add_box(&mut doc, vstack, "CornerBadge", 24.0, 16.0, Sizing::Fixed, Sizing::Fixed, (0xE6, 0x33, 0x33));
    {
        let b = doc.get_mut(badge).unwrap();
        b.layout_child.absolute = true;
        b.transform.tx = 260.0;
        b.transform.ty = 12.0;
    }
    let hidden = add_box(&mut doc, vstack, "HiddenBanner", 80.0, 20.0, Sizing::Fixed, Sizing::Fixed, (0x66, 0x66, 0x66));
    {
        let h = doc.get_mut(hidden).unwrap();
        h.variables.bindings.insert("visible".into(), "flag_hide".into());
        h.transform.tx = 16.0;
        h.transform.ty = 88.0;
    }
    doc.relayout_subtree(vstack);

    // Siblings keep their exact in-flow solver positions: (16, 16) and (16, 200 - 16 - 32 = 152).
    let first_n = doc.get(first).unwrap();
    let second_n = doc.get(second).unwrap();
    assert_eq!((first_n.transform.tx, first_n.transform.ty), (16.0, 16.0));
    assert_eq!((second_n.transform.tx, second_n.transform.ty), (16.0, 152.0));

    let v_swift = swiftui_for(&doc, vstack);
    assert!(
        v_swift.starts_with("ZStack(alignment: .topLeading) {\n    VStack(alignment: .leading) {"),
        "SwiftUI VStack with absolute/hidden children must be wrapped in ZStack(alignment: .topLeading):\n{v_swift}"
    );
    assert_eq!(v_swift.matches("Spacer()").count(), 1, "SpaceBetween VStack with 2 in-flow children must emit exactly 1 Spacer():\n{v_swift}");
    let vstack_close = v_swift.find("    }\n        .padding(16)").expect("Inner VStack should close and carry .padding(16)");
    let badge_pos = v_swift.find(".offset(x: 260, y: 12)").expect("Missing CornerBadge .offset(x: 260, y: 12)");
    let hidden_pos = v_swift.find(".offset(x: 16, y: 88)").expect("Missing HiddenBanner .offset(x: 16, y: 88)");
    let outer_frame_pos = v_swift.find("}\n    .frame(width: 300, height: 200").expect("Outer ZStack should carry the 300x200 frame modifier");
    assert!(
        vstack_close < badge_pos && badge_pos < hidden_pos && hidden_pos < outer_frame_pos,
        "Absolute and variable-hidden children must be emitted after inner VStack and before outer ZStack closes in paint order:\n{v_swift}"
    );

    let v_compose = compose_for(&doc, vstack);
    assert!(
        v_compose.starts_with("Box(\n    modifier = Modifier\n        .size(width = 300.dp, height = 200.dp)"),
        "Compose Column with absolute/hidden children must be wrapped in an outer Box with the frame's size:\n{v_compose}"
    );
    let col_open = v_compose
        .find("    Column(\n        modifier = Modifier.padding(16.dp),\n        verticalArrangement = Arrangement.SpaceBetween,\n        horizontalAlignment = Alignment.Start\n    ) {")
        .expect("Inner Column should carry padding(16.dp) and SpaceBetween");
    let col_close = v_compose[col_open..].find("\n    }\n    Box(").map(|i| col_open + i).expect("Inner Column must close before out-of-flow Box children");
    let kt_badge_pos = v_compose.find(".offset(x = 260.dp, y = 12.dp)").expect("Missing CornerBadge .offset(x = 260.dp, y = 12.dp)");
    let kt_hidden_pos = v_compose.find(".offset(x = 16.dp, y = 88.dp)").expect("Missing HiddenBanner .offset(x = 16.dp, y = 88.dp)");
    assert!(
        col_close < kt_badge_pos && kt_badge_pos < kt_hidden_pos,
        "Compose out-of-flow children must appear after Column closes in paint order:\n{v_compose}"
    );

    // 2. Horizontal stack (HStack / Row) with an absolute child.
    let h_al = AutoLayout {
        direction: LayoutDirection::Horizontal,
        primary_align: LayoutAlign::Start,
        counter_align: LayoutAlign::Center,
        item_spacing: 10.0,
        padding: [12.0, 12.0, 12.0, 12.0],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    let (mut hdoc, hstack) = new_stack(h_al, Vec2::new(280.0, 80.0));
    add_box(&mut hdoc, hstack, "LeftItem", 60.0, 36.0, Sizing::Fixed, Sizing::Fixed, (0x33, 0x80, 0xCC));
    add_box(&mut hdoc, hstack, "RightItem", 60.0, 36.0, Sizing::Fixed, Sizing::Fixed, (0x4D, 0x99, 0xE6));
    let pin = add_box(&mut hdoc, hstack, "FloatingPin", 18.0, 18.0, Sizing::Fixed, Sizing::Fixed, (0xF2, 0x4D, 0x33));
    {
        let p = hdoc.get_mut(pin).unwrap();
        p.layout_child.absolute = true;
        p.transform.tx = 248.0;
        p.transform.ty = 8.0;
    }
    hdoc.relayout_subtree(hstack);

    let h_swift = swiftui_for(&hdoc, hstack);
    assert!(
        h_swift.starts_with("ZStack(alignment: .topLeading) {\n    HStack(spacing: 10) {"),
        "SwiftUI HStack with absolute child must be wrapped in ZStack(alignment: .topLeading):\n{h_swift}"
    );
    let hstack_close = h_swift.find("    }\n        .padding(12)").expect("Inner HStack should close and carry .padding(12)");
    let pin_pos = h_swift.find(".offset(x: 248, y: 8)").expect("Missing FloatingPin .offset(x: 248, y: 8)");
    assert!(hstack_close < pin_pos, "FloatingPin must be emitted after HStack closes:\n{h_swift}");

    let h_compose = compose_for(&hdoc, hstack);
    let row_open = h_compose
        .find("    Row(\n        modifier = Modifier.padding(12.dp),\n        horizontalArrangement = Arrangement.spacedBy(10.dp),\n        verticalAlignment = Alignment.CenterVertically\n    ) {")
        .expect("Inner Row should carry padding(12.dp)");
    let row_close = h_compose[row_open..].find("\n    }\n    Box(").map(|i| row_open + i).expect("Inner Row must close before FloatingPin");
    let kt_pin_pos = h_compose.find(".offset(x = 248.dp, y = 8.dp)").expect("Missing FloatingPin .offset(x = 248.dp, y = 8.dp)");
    assert!(row_close < kt_pin_pos, "Compose FloatingPin must appear after Row closes:\n{h_compose}");
}

#[test]
fn absolute_and_variable_hidden_children_in_grid_are_emitted_in_overlay_out_of_flow() {
    let mut g_al = AutoLayout {
        direction: LayoutDirection::Grid,
        grid_columns: 2,
        grid_rows: 1,
        item_spacing: 10.0,
        counter_spacing: 10.0,
        padding: [10.0, 10.0, 10.0, 10.0],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    g_al.grid_col_tracks = vec![GridTrack::Flex(1.0), GridTrack::Flex(1.0)].into();
    g_al.grid_row_tracks = vec![GridTrack::Fixed(40.0)].into();

    let (mut doc, grid) = new_stack(g_al, Vec2::new(220.0, 100.0));
    add_false_flag(&mut doc, "flag_grid_hide");

    add_box(&mut doc, grid, "CellA", 95.0, 40.0, Sizing::Fill, Sizing::Fixed, (0x33, 0x66, 0xCC));
    add_box(&mut doc, grid, "CellB", 95.0, 40.0, Sizing::Fill, Sizing::Fixed, (0x4D, 0x80, 0xE6));
    let tag = add_box(&mut doc, grid, "FloatingTag", 28.0, 16.0, Sizing::Fixed, Sizing::Fixed, (0xE6, 0x33, 0x33));
    {
        let t = doc.get_mut(tag).unwrap();
        t.layout_child.absolute = true;
        t.transform.tx = 180.0;
        t.transform.ty = 6.0;
    }
    let chip = add_box(&mut doc, grid, "HiddenChip", 60.0, 20.0, Sizing::Fixed, Sizing::Fixed, (0x80, 0x80, 0x80));
    {
        let c = doc.get_mut(chip).unwrap();
        c.variables.bindings.insert("visible".into(), "flag_grid_hide".into());
        c.transform.tx = 14.0;
        c.transform.ty = 70.0;
    }
    doc.relayout_subtree(grid);

    let swift = swiftui_for(&doc, grid);
    assert!(
        swift.starts_with("ZStack(alignment: .topLeading) {\n    Grid(alignment: .topLeading, horizontalSpacing: 10, verticalSpacing: 10) {"),
        "SwiftUI Grid with absolute/hidden children must be wrapped in ZStack(alignment: .topLeading):\n{swift}"
    );
    let grid_close = swift.find("    }\n        .padding(10)").expect("Inner SwiftUI Grid should close and carry .padding(10)");
    let tag_pos = swift.find(".offset(x: 180, y: 6)").expect("Missing FloatingTag .offset(x: 180, y: 6)");
    let chip_pos = swift.find(".offset(x: 14, y: 70)").expect("Missing HiddenChip .offset(x: 14, y: 70)");
    let outer_frame = swift.find("}\n    .frame(width: 220, height: 100").expect("Outer ZStack should carry the 220x100 frame modifier");
    assert!(
        grid_close < tag_pos && tag_pos < chip_pos && chip_pos < outer_frame,
        "SwiftUI Grid out-of-flow children must appear after Grid closes and before ZStack closes in paint order:\n{swift}"
    );

    let compose = compose_for(&doc, grid);
    let kt_grid_pos = compose.find("    LazyVerticalGrid(").expect("Missing LazyVerticalGrid");
    let kt_grid_close =
        compose[kt_grid_pos..].find("\n    }\n    Box(").map(|i| kt_grid_pos + i).expect("LazyVerticalGrid must close before out-of-flow children");
    let kt_tag_pos = compose.find(".offset(x = 180.dp, y = 6.dp)").expect("Missing FloatingTag .offset(x = 180.dp, y = 6.dp)");
    let kt_chip_pos = compose.find(".offset(x = 14.dp, y = 70.dp)").expect("Missing HiddenChip .offset(x = 14.dp, y = 70.dp)");
    assert!(
        kt_grid_close < kt_tag_pos && kt_tag_pos < kt_chip_pos,
        "Compose Grid out-of-flow and variable-hidden children must appear after LazyVerticalGrid closes with offsets:\n{compose}"
    );
}

#[test]
fn null_control_frames_without_abs_children_or_fill_fixed_pairs_export_identically() {
    let al = AutoLayout {
        direction: LayoutDirection::Vertical,
        primary_align: LayoutAlign::Start,
        counter_align: LayoutAlign::Start,
        item_spacing: 8.0,
        padding: [16.0, 16.0, 16.0, 16.0],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    let (mut doc, card) = new_stack(al, Vec2::new(240.0, 160.0));
    add_box(&mut doc, card, "FixedBoth", 120.0, 32.0, Sizing::Fixed, Sizing::Fixed, (0x33, 0x4D, 0x66));
    add_box(&mut doc, card, "FillBoth", 120.0, 48.0, Sizing::Fill, Sizing::Fill, (0x66, 0x80, 0x99));
    doc.relayout_subtree(card);

    let expected_swift = "\
VStack(alignment: .leading, spacing: 8) {
    Rectangle()
        .fill(Color(red: 0.2, green: 0.302, blue: 0.4))
        .frame(width: 120, height: 32)
    Rectangle()
        .fill(Color(red: 0.4, green: 0.502, blue: 0.6))
        .frame(maxWidth: .infinity, maxHeight: .infinity)
}
    .padding(16)
    .frame(width: 240, height: 160, alignment: .topLeading)
    .background(RoundedRectangle(cornerRadius: 8).fill(Color(red: 0.961, green: 0.961, blue: 0.98)))
    .clipShape(RoundedRectangle(cornerRadius: 8))";
    assert_eq!(swiftui_for(&doc, card), expected_swift);

    let expected_compose = "\
Column(
    modifier = Modifier
        .size(width = 240.dp, height = 160.dp)
        .clip(RoundedCornerShape(8.dp))
        .background(Color(0xFFF5F5FA), RoundedCornerShape(8.dp))
        .padding(16.dp),
    verticalArrangement = Arrangement.spacedBy(8.dp),
    horizontalAlignment = Alignment.Start
) {
    Box(
        modifier = Modifier
            .size(width = 120.dp, height = 32.dp)
            .background(Color(0xFF334D66))
    )
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .weight(1f)
            .background(Color(0xFF668099))
    )
}";
    assert_eq!(compose_for(&doc, card), expected_compose);
}

fn build_swiftui_fixtures_for_typecheck() -> Vec<(&'static str, String)> {
    let mut out = Vec::new();

    // 1..6: The 6 canonical grid fixtures from grid_native_fixtures.rs (build_six_scenes).
    // 1. equal_tracks
    {
        let al = AutoLayout {
            grid_columns: 3,
            grid_rows: 0,
            item_spacing: 12.0,
            counter_spacing: 12.0,
            padding: [12.0, 12.0, 12.0, 12.0],
            primary_sizing: Sizing::Fixed,
            counter_sizing: Sizing::Fixed,
            ..Default::default()
        };
        let (mut doc, g) = new_grid(al, Vec2::new(324.0, 180.0));
        for _ in 0..6 {
            add_item(&mut doc, g, 80.0, 36.0, Sizing::Fill, Sizing::Fixed);
        }
        doc.relayout_subtree(g);
        out.push(("01_grid_equal_tracks", swiftui_for(&doc, g)));
    }
    // 2. unequal_tracks
    {
        let al = AutoLayout {
            grid_columns: 3,
            grid_rows: 0,
            item_spacing: 0.0,
            counter_spacing: 10.0,
            padding: [0.0, 0.0, 0.0, 0.0],
            primary_sizing: Sizing::Hug,
            counter_sizing: Sizing::Fixed,
            ..Default::default()
        };
        let (mut doc, g) = new_grid(al, Vec2::new(500.0, 140.0));
        add_item(&mut doc, g, 100.0, 30.0, Sizing::Fixed, Sizing::Fixed);
        add_item(&mut doc, g, 40.0, 30.0, Sizing::Fixed, Sizing::Fixed);
        add_item(&mut doc, g, 60.0, 30.0, Sizing::Fixed, Sizing::Fixed);
        add_item(&mut doc, g, 50.0, 30.0, Sizing::Fill, Sizing::Fixed);
        add_item(&mut doc, g, 50.0, 30.0, Sizing::Fill, Sizing::Fixed);
        add_item(&mut doc, g, 50.0, 30.0, Sizing::Fill, Sizing::Fixed);
        doc.relayout_subtree(g);
        out.push(("02_grid_unequal_tracks", swiftui_for(&doc, g)));
    }
    // 3. fixed_first_column
    {
        let mut al = AutoLayout {
            grid_columns: 3,
            grid_rows: 0,
            item_spacing: 10.0,
            counter_spacing: 10.0,
            padding: [0.0, 0.0, 0.0, 0.0],
            primary_sizing: Sizing::Fixed,
            counter_sizing: Sizing::Fixed,
            ..Default::default()
        };
        al.grid_col_tracks[0] = GridTrack::Fixed(200.0);
        let (mut doc, g) = new_grid(al, Vec2::new(440.0, 180.0));
        for _ in 0..6 {
            add_item(&mut doc, g, 50.0, 40.0, Sizing::Fill, Sizing::Fixed);
        }
        doc.relayout_subtree(g);
        out.push(("03_grid_fixed_first_column", swiftui_for(&doc, g)));
    }
    // 4. hugging_grid
    {
        let al = AutoLayout {
            grid_columns: 3,
            grid_rows: 0,
            item_spacing: 8.0,
            counter_spacing: 8.0,
            padding: [8.0, 8.0, 8.0, 8.0],
            primary_sizing: Sizing::Hug,
            counter_sizing: Sizing::Hug,
            ..Default::default()
        };
        let (mut doc, g) = new_grid(al, Vec2::new(500.0, 500.0));
        add_item(&mut doc, g, 100.0, 28.0, Sizing::Fixed, Sizing::Fixed);
        add_item(&mut doc, g, 24.0, 28.0, Sizing::Fixed, Sizing::Fixed);
        add_item(&mut doc, g, 24.0, 28.0, Sizing::Fixed, Sizing::Fixed);
        add_item(&mut doc, g, 50.0, 20.0, Sizing::Fill, Sizing::Fixed);
        add_item(&mut doc, g, 50.0, 20.0, Sizing::Fill, Sizing::Fixed);
        add_item(&mut doc, g, 50.0, 20.0, Sizing::Fill, Sizing::Fixed);
        doc.relayout_subtree(g);
        out.push(("04_grid_hugging_grid", swiftui_for(&doc, g)));
    }
    // 5. fixed_height_auto_rows
    {
        let al = AutoLayout {
            grid_columns: 2,
            grid_rows: 0,
            item_spacing: 12.0,
            counter_spacing: 16.0,
            padding: [16.0, 16.0, 16.0, 16.0],
            primary_sizing: Sizing::Fixed,
            counter_sizing: Sizing::Fixed,
            ..Default::default()
        };
        let (mut doc, g) = new_grid(al, Vec2::new(260.0, 240.0));
        add_item(&mut doc, g, 50.0, 24.0, Sizing::Fill, Sizing::Fixed);
        add_item(&mut doc, g, 50.0, 24.0, Sizing::Fill, Sizing::Fixed);
        add_item(&mut doc, g, 50.0, 36.0, Sizing::Fill, Sizing::Fixed);
        add_item(&mut doc, g, 50.0, 36.0, Sizing::Fill, Sizing::Fixed);
        add_item(&mut doc, g, 50.0, 28.0, Sizing::Fill, Sizing::Fixed);
        add_item(&mut doc, g, 50.0, 28.0, Sizing::Fill, Sizing::Fixed);
        doc.relayout_subtree(g);
        out.push(("05_grid_fixed_height_auto_rows", swiftui_for(&doc, g)));
    }
    // 6. row_and_column_spans
    {
        let al = AutoLayout {
            grid_columns: 4,
            grid_rows: 0,
            item_spacing: 12.0,
            counter_spacing: 12.0,
            padding: [12.0, 12.0, 12.0, 12.0],
            primary_sizing: Sizing::Fixed,
            counter_sizing: Sizing::Fixed,
            ..Default::default()
        };
        let (mut doc, g) = new_grid(al, Vec2::new(428.0, 220.0));
        let hero = add_box(&mut doc, g, "HeroTile", 196.0, 40.0, Sizing::Fixed, Sizing::Fill, (0x35, 0x82, 0xFF));
        doc.get_mut(hero).unwrap().layout_child.col_span = 2;
        doc.get_mut(hero).unwrap().layout_child.row_span = 2;
        let top_wide = add_box(&mut doc, g, "TopWide", 196.0, 40.0, Sizing::Fixed, Sizing::Fixed, (0x35, 0x82, 0xFF));
        doc.get_mut(top_wide).unwrap().layout_child.col_span = 2;
        add_box(&mut doc, g, "MidA", 92.0, 40.0, Sizing::Fixed, Sizing::Fixed, (0x35, 0x82, 0xFF));
        add_box(&mut doc, g, "MidB", 92.0, 40.0, Sizing::Fixed, Sizing::Fixed, (0x35, 0x82, 0xFF));
        let footer = add_box(&mut doc, g, "FooterBar", 404.0, 36.0, Sizing::Fixed, Sizing::Fixed, (0x35, 0x82, 0xFF));
        doc.get_mut(footer).unwrap().layout_child.grid_column = Some(0);
        doc.get_mut(footer).unwrap().layout_child.grid_row = Some(2);
        doc.get_mut(footer).unwrap().layout_child.col_span = 4;
        doc.get_mut(footer).unwrap().layout_child.row_span = 1;
        doc.relayout_subtree(g);
        out.push(("06_grid_row_and_column_spans", swiftui_for(&doc, g)));
    }
    // 7. grid_fill_child_stacked_inside_column_span
    {
        let mut al = AutoLayout {
            grid_columns: 2,
            grid_rows: 1,
            item_spacing: 10.0,
            counter_spacing: 10.0,
            primary_sizing: Sizing::Fixed,
            counter_sizing: Sizing::Fixed,
            ..Default::default()
        };
        al.grid_col_tracks[0] = GridTrack::Fixed(100.0);
        al.grid_col_tracks[1] = GridTrack::Fixed(40.0);
        al.grid_row_tracks[0] = GridTrack::Fixed(40.0);
        let (mut doc, g) = new_grid(al, Vec2::new(150.0, 40.0));
        let wide = add_box(&mut doc, g, "Wide", 150.0, 40.0, Sizing::Fixed, Sizing::Fixed, (0x4D, 0x4D, 0x4D));
        {
            let w = doc.get_mut(wide).unwrap();
            w.layout_child.grid_column = Some(0);
            w.layout_child.grid_row = Some(0);
            w.layout_child.col_span = 2;
        }
        let fill_c = add_box(&mut doc, g, "FillCol1", 40.0, 20.0, Sizing::Fill, Sizing::Fixed, (0xCC, 0x66, 0x33));
        {
            let f = doc.get_mut(fill_c).unwrap();
            f.layout_child.grid_column = Some(1);
            f.layout_child.grid_row = Some(0);
        }
        doc.relayout_subtree(g);
        out.push(("07_grid_fill_child_stacked_in_col_span", swiftui_for(&doc, g)));
    }
    // 8. grid_with_absolute_child
    {
        let al = AutoLayout {
            grid_columns: 2,
            grid_rows: 0,
            item_spacing: 10.0,
            counter_spacing: 10.0,
            padding: [10.0; 4],
            primary_sizing: Sizing::Fixed,
            counter_sizing: Sizing::Hug,
            ..Default::default()
        };
        let (mut doc, g) = new_grid(al, Vec2::new(210.0, 80.0));
        add_box(&mut doc, g, "CellA", 80.0, 36.0, Sizing::Fill, Sizing::Fixed, (0x33, 0x66, 0xCC));
        add_box(&mut doc, g, "CellB", 80.0, 36.0, Sizing::Fill, Sizing::Fixed, (0x4D, 0x80, 0xE6));
        let badge = add_box(&mut doc, g, "Badge", 24.0, 18.0, Sizing::Fixed, Sizing::Fixed, (0xE6, 0x33, 0x33));
        {
            let b = doc.get_mut(badge).unwrap();
            b.layout_child.absolute = true;
            b.transform.tx = 15.0;
            b.transform.ty = 25.0;
        }
        doc.relayout_subtree(g);
        out.push(("08_grid_with_absolute_child", swiftui_for(&doc, g)));
    }
    // 9. stack_fill_width_fixed_height
    {
        let al = AutoLayout {
            direction: LayoutDirection::Vertical,
            item_spacing: 8.0,
            padding: [12.0; 4],
            primary_sizing: Sizing::Fixed,
            counter_sizing: Sizing::Fixed,
            ..Default::default()
        };
        let (mut doc, v) = new_stack(al, Vec2::new(300.0, 160.0));
        add_box(&mut doc, v, "FillWFixedH", 100.0, 44.0, Sizing::Fill, Sizing::Fixed, (0x33, 0x80, 0xCC));
        doc.relayout_subtree(v);
        out.push(("09_stack_fill_width_fixed_height", swiftui_for(&doc, v)));
    }
    // 10. stack_fixed_width_fill_height
    {
        let al = AutoLayout {
            direction: LayoutDirection::Horizontal,
            item_spacing: 8.0,
            padding: [12.0; 4],
            primary_sizing: Sizing::Fixed,
            counter_sizing: Sizing::Fixed,
            ..Default::default()
        };
        let (mut doc, h) = new_stack(al, Vec2::new(300.0, 160.0));
        add_box(&mut doc, h, "FixedWFillH", 88.0, 100.0, Sizing::Fixed, Sizing::Fill, (0x33, 0xB3, 0x80));
        doc.relayout_subtree(h);
        out.push(("10_stack_fixed_width_fill_height", swiftui_for(&doc, h)));
    }
    // 11. vstack_with_absolute_child
    {
        let al = AutoLayout {
            direction: LayoutDirection::Vertical,
            primary_align: LayoutAlign::SpaceBetween,
            item_spacing: 8.0,
            padding: [16.0; 4],
            primary_sizing: Sizing::Fixed,
            counter_sizing: Sizing::Fixed,
            ..Default::default()
        };
        let (mut doc, v) = new_stack(al, Vec2::new(300.0, 200.0));
        add_box(&mut doc, v, "Item1", 100.0, 24.0, Sizing::Fixed, Sizing::Fixed, (0x33, 0x4D, 0x66));
        add_box(&mut doc, v, "Item2", 100.0, 32.0, Sizing::Fixed, Sizing::Fixed, (0x4D, 0x66, 0x80));
        let abs = add_box(&mut doc, v, "Pin", 24.0, 16.0, Sizing::Fixed, Sizing::Fixed, (0xE6, 0x33, 0x33));
        {
            let a = doc.get_mut(abs).unwrap();
            a.layout_child.absolute = true;
            a.transform.tx = 260.0;
            a.transform.ty = 12.0;
        }
        doc.relayout_subtree(v);
        out.push(("11_vstack_with_absolute_child", swiftui_for(&doc, v)));
    }
    // 12. hstack_with_absolute_child
    {
        let al = AutoLayout {
            direction: LayoutDirection::Horizontal,
            item_spacing: 10.0,
            padding: [12.0; 4],
            primary_sizing: Sizing::Fixed,
            counter_sizing: Sizing::Fixed,
            ..Default::default()
        };
        let (mut doc, h) = new_stack(al, Vec2::new(280.0, 80.0));
        add_box(&mut doc, h, "Left", 60.0, 36.0, Sizing::Fixed, Sizing::Fixed, (0x33, 0x80, 0xCC));
        let abs = add_box(&mut doc, h, "Pin", 18.0, 18.0, Sizing::Fixed, Sizing::Fixed, (0xE6, 0x4D, 0x33));
        {
            let a = doc.get_mut(abs).unwrap();
            a.layout_child.absolute = true;
            a.transform.tx = 248.0;
            a.transform.ty = 8.0;
        }
        doc.relayout_subtree(h);
        out.push(("12_hstack_with_absolute_child", swiftui_for(&doc, h)));
    }

    out
}

#[test]
fn every_swiftui_test_fixture_passes_swiftc_typecheck() {
    let fixtures = build_swiftui_fixtures_for_typecheck();
    assert_eq!(fixtures.len(), 12);

    let tmp = std::env::temp_dir().join(format!("designos-swiftc-check-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("create temp dir for swiftc");

    let mut failures = Vec::new();
    for (idx, (name, body)) in fixtures.iter().enumerate() {
        let indented = body.lines().map(|l| format!("        {l}")).collect::<Vec<_>>().join("\n");
        let swift_src = format!("import SwiftUI\n\nstruct Fixture_{idx}: View {{\n    var body: some View {{\n{indented}\n    }}\n}}\n");
        let path = tmp.join(format!("{name}.swift"));
        std::fs::write(&path, &swift_src).expect("write swift file");
        let output = Command::new("/usr/bin/swiftc").arg("-typecheck").arg(&path).output().expect("invoke /usr/bin/swiftc");
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            failures.push((name.to_string(), stderr));
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);

    let checked = fixtures.len();
    let failed = failures.len();
    let passed = checked - failed;
    eprintln!("swiftc_typecheck_summary: checked={checked} passed={passed} failed={failed}");
    for (name, err) in &failures {
        eprintln!("--- FAILED {name} ---\n{err}");
    }
    assert!(failures.is_empty(), "swiftc -typecheck failed on {failed}/{checked} SwiftUI fixtures");
}
