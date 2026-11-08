use std::path::Path;
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

fn make_absolute(doc: &mut Document, id: NodeId, x: f32, y: f32) {
    let n = doc.get_mut(id).unwrap();
    n.layout_child.absolute = true;
    n.transform.tx = x;
    n.transform.ty = y;
}

/// Hides `id` behind the boolean variable `flag` and leaves a stale position on it, as an edit that
/// toggled the flag would.
fn hide_by_flag(doc: &mut Document, id: NodeId, flag: &str, x: f32, y: f32) {
    let n = doc.get_mut(id).unwrap();
    n.variables.bindings.insert("visible".into(), flag.into());
    n.transform.tx = x;
    n.transform.ty = y;
}

fn solved_at(doc: &Document, id: NodeId) -> (f32, f32) {
    let n = doc.get(id).unwrap();
    (n.transform.tx, n.transform.ty)
}

/// Asserts that every needle occurs in `src`, each after the end of the previous one.
fn assert_in_order(src: &str, needles: &[&str]) {
    let mut from = 0;
    for needle in needles {
        match src[from..].find(needle) {
            Some(i) => from += i + needle.len(),
            None => panic!("missing, or out of order: {needle:?}\n{src}"),
        }
    }
}

/// 300x200 vertical frame, padding 16, space-between, counter axis start: two in-flow children and a
/// corner badge placed absolutely.
fn badge_column() -> (Document, NodeId) {
    let al = AutoLayout {
        direction: LayoutDirection::Vertical,
        primary_align: LayoutAlign::SpaceBetween,
        counter_align: LayoutAlign::Start,
        item_spacing: 8.0,
        padding: [16.0; 4],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    let (mut doc, frame) = new_stack(al, Vec2::new(300.0, 200.0));
    add_box(&mut doc, frame, "FirstFlow", 100.0, 24.0, Sizing::Fixed, Sizing::Fixed, (0x1A, 0x33, 0x4D));
    add_box(&mut doc, frame, "SecondFlow", 100.0, 32.0, Sizing::Fixed, Sizing::Fixed, (0x33, 0x4D, 0x66));
    let badge = add_box(&mut doc, frame, "CornerBadge", 24.0, 16.0, Sizing::Fixed, Sizing::Fixed, (0xE6, 0x33, 0x33));
    make_absolute(&mut doc, badge, 260.0, 12.0);
    doc.relayout_subtree(frame);
    (doc, frame)
}

/// 280x80 horizontal frame, padding 12, counter axis centred: two in-flow children and a pin placed
/// absolutely.
fn pin_row() -> (Document, NodeId) {
    let al = AutoLayout {
        direction: LayoutDirection::Horizontal,
        primary_align: LayoutAlign::Start,
        counter_align: LayoutAlign::Center,
        item_spacing: 10.0,
        padding: [12.0; 4],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    let (mut doc, frame) = new_stack(al, Vec2::new(280.0, 80.0));
    add_box(&mut doc, frame, "LeftItem", 60.0, 36.0, Sizing::Fixed, Sizing::Fixed, (0x33, 0x80, 0xCC));
    add_box(&mut doc, frame, "RightItem", 60.0, 36.0, Sizing::Fixed, Sizing::Fixed, (0x4D, 0x99, 0xE6));
    let pin = add_box(&mut doc, frame, "FloatingPin", 18.0, 18.0, Sizing::Fixed, Sizing::Fixed, (0xF2, 0x4D, 0x33));
    make_absolute(&mut doc, pin, 248.0, 8.0);
    doc.relayout_subtree(frame);
    (doc, frame)
}

/// 240x160 frame centred on both axes whose children are all absolute. (A frame of 128 or less on both
/// axes with only shapes inside exports as one image.)
fn absolute_only_frame() -> (Document, NodeId) {
    let al = AutoLayout {
        direction: LayoutDirection::Vertical,
        primary_align: LayoutAlign::Center,
        counter_align: LayoutAlign::Center,
        padding: [8.0; 4],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    let (mut doc, frame) = new_stack(al, Vec2::new(240.0, 160.0));
    let dot_a = add_box(&mut doc, frame, "DotA", 10.0, 10.0, Sizing::Fixed, Sizing::Fixed, (0xE6, 0x33, 0x33));
    let dot_b = add_box(&mut doc, frame, "DotB", 10.0, 10.0, Sizing::Fixed, Sizing::Fixed, (0x33, 0x99, 0xE6));
    make_absolute(&mut doc, dot_a, 10.0, 10.0);
    make_absolute(&mut doc, dot_b, 200.0, 120.0);
    doc.relayout_subtree(frame);
    (doc, frame)
}

/// 200x120 vertical frame in Studio's child order: an absolute underlay, two in-flow children, then an
/// absolute badge.
fn underlay_column() -> (Document, NodeId) {
    let al = AutoLayout {
        direction: LayoutDirection::Vertical,
        item_spacing: 8.0,
        padding: [12.0; 4],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    let (mut doc, frame) = new_stack(al, Vec2::new(200.0, 120.0));
    let underlay = add_box(&mut doc, frame, "Underlay", 192.0, 112.0, Sizing::Fixed, Sizing::Fixed, (0x10, 0x20, 0x30));
    add_box(&mut doc, frame, "FlowA", 80.0, 20.0, Sizing::Fixed, Sizing::Fixed, (0x33, 0x4D, 0x66));
    add_box(&mut doc, frame, "FlowB", 80.0, 20.0, Sizing::Fixed, Sizing::Fixed, (0x4D, 0x66, 0x80));
    let badge = add_box(&mut doc, frame, "TopBadge", 20.0, 20.0, Sizing::Fixed, Sizing::Fixed, (0xE6, 0x33, 0x33));
    make_absolute(&mut doc, underlay, 4.0, 4.0);
    make_absolute(&mut doc, badge, 170.0, 4.0);
    doc.relayout_subtree(frame);
    (doc, frame)
}

/// Vertical frame that hugs its height, and its width too unless `counter_sizing` is fixed, with a
/// badge placed absolutely.
fn hug_badge_column(counter_sizing: Sizing) -> (Document, NodeId) {
    let al = AutoLayout {
        direction: LayoutDirection::Vertical,
        item_spacing: 8.0,
        padding: [12.0; 4],
        primary_sizing: Sizing::Hug,
        counter_sizing,
        ..Default::default()
    };
    let (mut doc, frame) = new_stack(al, Vec2::new(200.0, 100.0));
    add_box(&mut doc, frame, "Label", 150.0, 24.0, Sizing::Fixed, Sizing::Fixed, (0x33, 0x4D, 0x66));
    add_box(&mut doc, frame, "Caption", 150.0, 16.0, Sizing::Fixed, Sizing::Fixed, (0x4D, 0x66, 0x80));
    let badge = add_box(&mut doc, frame, "Badge", 16.0, 16.0, Sizing::Fixed, Sizing::Fixed, (0xE6, 0x33, 0x33));
    make_absolute(&mut doc, badge, 158.0, 2.0);
    doc.relayout_subtree(frame);
    (doc, frame)
}

/// The [`badge_column`] frame with a child that a boolean variable hides (its stored position is
/// stale) in place of the corner badge, or after it.
fn hidden_column(with_badge: bool) -> (Document, NodeId) {
    let al = AutoLayout {
        direction: LayoutDirection::Vertical,
        primary_align: LayoutAlign::SpaceBetween,
        counter_align: LayoutAlign::Start,
        item_spacing: 8.0,
        padding: [16.0; 4],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    let (mut doc, frame) = new_stack(al, Vec2::new(300.0, 200.0));
    add_false_flag(&mut doc, "flag_hide");
    add_box(&mut doc, frame, "FirstFlow", 100.0, 24.0, Sizing::Fixed, Sizing::Fixed, (0x1A, 0x33, 0x4D));
    add_box(&mut doc, frame, "SecondFlow", 100.0, 32.0, Sizing::Fixed, Sizing::Fixed, (0x33, 0x4D, 0x66));
    if with_badge {
        let badge = add_box(&mut doc, frame, "CornerBadge", 24.0, 16.0, Sizing::Fixed, Sizing::Fixed, (0xE6, 0x33, 0x33));
        make_absolute(&mut doc, badge, 260.0, 12.0);
    }
    let hidden = add_box(&mut doc, frame, "HiddenBanner", 80.0, 20.0, Sizing::Fixed, Sizing::Fixed, (0x66, 0x66, 0x66));
    hide_by_flag(&mut doc, hidden, "flag_hide", 16.0, 88.0);
    doc.relayout_subtree(frame);
    (doc, frame)
}

/// 220x100 grid frame, two flexible columns and one fixed row, padding 10, with the flag that hides
/// variable-bound children registered and no children yet.
fn empty_flex_grid() -> (Document, NodeId) {
    let mut al = AutoLayout {
        direction: LayoutDirection::Grid,
        grid_columns: 2,
        grid_rows: 1,
        item_spacing: 10.0,
        counter_spacing: 10.0,
        padding: [10.0; 4],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    al.grid_col_tracks = vec![GridTrack::Flex(1.0), GridTrack::Flex(1.0)].into();
    al.grid_row_tracks = vec![GridTrack::Fixed(40.0)].into();
    let (mut doc, grid) = new_stack(al, Vec2::new(220.0, 100.0));
    add_false_flag(&mut doc, "flag_grid_hide");
    (doc, grid)
}

fn add_cells(doc: &mut Document, grid: NodeId) {
    add_box(doc, grid, "CellA", 95.0, 40.0, Sizing::Fill, Sizing::Fixed, (0x33, 0x66, 0xCC));
    add_box(doc, grid, "CellB", 95.0, 40.0, Sizing::Fill, Sizing::Fixed, (0x4D, 0x80, 0xE6));
}

fn add_hidden_chip(doc: &mut Document, grid: NodeId) {
    let chip = add_box(doc, grid, "HiddenChip", 60.0, 20.0, Sizing::Fixed, Sizing::Fixed, (0x80, 0x80, 0x80));
    hide_by_flag(doc, chip, "flag_grid_hide", 14.0, 70.0);
}

fn add_tag(doc: &mut Document, grid: NodeId) {
    let tag = add_box(doc, grid, "FloatingTag", 28.0, 16.0, Sizing::Fixed, Sizing::Fixed, (0xE6, 0x33, 0x33));
    make_absolute(doc, tag, 180.0, 6.0);
}

/// Grid with its two cells, an absolute tag and a variable-hidden chip, in that child order.
fn tag_and_chip_grid() -> (Document, NodeId) {
    let (mut doc, grid) = empty_flex_grid();
    add_cells(&mut doc, grid);
    add_tag(&mut doc, grid);
    add_hidden_chip(&mut doc, grid);
    doc.relayout_subtree(grid);
    (doc, grid)
}

/// Grid whose only child besides its cells is a variable-hidden chip.
fn chip_grid() -> (Document, NodeId) {
    let (mut doc, grid) = empty_flex_grid();
    add_cells(&mut doc, grid);
    add_hidden_chip(&mut doc, grid);
    doc.relayout_subtree(grid);
    (doc, grid)
}

/// Grid in Studio's child order: an absolute underlay, the two cells, then an absolute tag.
fn underlay_grid() -> (Document, NodeId) {
    let (mut doc, grid) = empty_flex_grid();
    let underlay = add_box(&mut doc, grid, "Underlay", 212.0, 92.0, Sizing::Fixed, Sizing::Fixed, (0x10, 0x20, 0x30));
    make_absolute(&mut doc, underlay, 4.0, 4.0);
    add_cells(&mut doc, grid);
    add_tag(&mut doc, grid);
    doc.relayout_subtree(grid);
    (doc, grid)
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
fn column_with_absolute_child_keeps_in_flow_children_where_the_solver_puts_them() {
    let (doc, frame) = badge_column();
    let kids = doc.get(frame).unwrap().children.clone();
    // The solver places the in-flow children at the padding edge and at the far end of the
    // space-between axis, and the absolute badge keeps its own position.
    assert_eq!(solved_at(&doc, kids[0]), (16.0, 16.0));
    assert_eq!(solved_at(&doc, kids[1]), (16.0, 152.0));
    assert_eq!(solved_at(&doc, kids[2]), (260.0, 12.0));

    let swift = swiftui_for(&doc, frame);
    assert!(swift.starts_with("ZStack(alignment: .topLeading) {\n    VStack(alignment: .leading) {"), "the stack sits inside a ZStack:\n{swift}");
    assert_eq!(swift.matches("Spacer()").count(), 1, "two in-flow children share one space-between spacer:\n{swift}");
    assert_in_order(
        &swift,
        &[
            "        Spacer()\n",
            "    }\n        .padding(16)\n        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)\n",
            ".offset(x: 260, y: 12)",
            "}\n    .frame(width: 300, height: 200, alignment: .topLeading)",
        ],
    );

    let kotlin = compose_for(&doc, frame);
    assert!(
        kotlin.starts_with("Box(\n    modifier = Modifier\n        .size(width = 300.dp, height = 200.dp)"),
        "the column sits inside a Box of the frame's size:\n{kotlin}"
    );
    // The column fills the box before it pads, so space-between runs across the frame's own height.
    assert!(
        kotlin.contains(
            "    Column(\n        modifier = Modifier\n            .fillMaxSize()\n            .padding(16.dp),\n        verticalArrangement = Arrangement.SpaceBetween,\n        horizontalAlignment = Alignment.Start\n    ) {"
        ),
        "the inner column fills the frame before its padding:\n{kotlin}"
    );
    assert_in_order(&kotlin, &["Column(", "\n    }\n    Box(", ".offset(x = 260.dp, y = 12.dp)"]);
}

#[test]
fn row_with_absolute_pin_keeps_centred_children_where_the_solver_puts_them() {
    let (doc, frame) = pin_row();
    let kids = doc.get(frame).unwrap().children.clone();
    // Both children are centred on the counter axis inside the padded box: y = 12 + (56 - 36) / 2.
    assert_eq!(solved_at(&doc, kids[0]), (12.0, 22.0));
    assert_eq!(solved_at(&doc, kids[1]), (82.0, 22.0));
    assert_eq!(solved_at(&doc, kids[2]), (248.0, 8.0));

    let swift = swiftui_for(&doc, frame);
    assert!(swift.starts_with("ZStack(alignment: .topLeading) {\n    HStack(spacing: 10) {"), "the stack sits inside a ZStack:\n{swift}");
    // The stack fills the ZStack and places its content by the frame's alignment (leading, centred
    // vertically), so the ZStack stays 280x80 and the pin's offset is read from the frame's top-left.
    assert_in_order(
        &swift,
        &[
            "    }\n        .padding(12)\n        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)\n",
            ".offset(x: 248, y: 8)",
            "}\n    .frame(width: 280, height: 80, alignment: .topLeading)",
        ],
    );

    let kotlin = compose_for(&doc, frame);
    assert!(
        kotlin.contains(
            "    Row(\n        modifier = Modifier\n            .fillMaxSize()\n            .padding(12.dp),\n        horizontalArrangement = Arrangement.spacedBy(10.dp),\n        verticalAlignment = Alignment.CenterVertically\n    ) {"
        ),
        "the inner row fills the frame before its padding, so the children centre in the frame:\n{kotlin}"
    );
    assert_in_order(&kotlin, &["Row(", "\n    }\n    Box(", ".offset(x = 248.dp, y = 8.dp)"]);
}

#[test]
fn frame_with_only_absolute_children_is_sized_to_the_frame() {
    let (doc, frame) = absolute_only_frame();

    // The layout alignment is centre on both axes, which would move a ZStack that is smaller than
    // the frame, so the children are placed from the frame's top-left instead.
    let swift = swiftui_for(&doc, frame);
    assert!(swift.starts_with("ZStack(alignment: .topLeading) {\n    Rectangle()"), "no stack inside the ZStack:\n{swift}");
    assert!(!swift.contains("VStack") && !swift.contains("Spacer()"), "an absolute-only frame has no stack:\n{swift}");
    assert_in_order(&swift, &[".offset(x: 10, y: 10)", ".offset(x: 200, y: 120)", "}\n    .frame(width: 240, height: 160, alignment: .topLeading)"]);

    let kotlin = compose_for(&doc, frame);
    assert!(kotlin.starts_with("Box(\n    modifier = Modifier\n        .size(width = 240.dp, height = 160.dp)"), "a Box of the frame's size:\n{kotlin}");
    assert!(!kotlin.contains("Column(") && !kotlin.contains("Row("), "an absolute-only frame has no stack:\n{kotlin}");
    assert_in_order(&kotlin, &[".offset(x = 10.dp, y = 10.dp)", ".offset(x = 200.dp, y = 120.dp)"]);
}

#[test]
fn absolute_children_before_the_first_in_flow_child_paint_behind_the_stack() {
    let (doc, frame) = underlay_column();

    let swift = swiftui_for(&doc, frame);
    assert_in_order(
        &swift,
        &[
            "ZStack(alignment: .topLeading) {",
            ".offset(x: 4, y: 4)",
            "VStack(alignment: .leading, spacing: 8) {",
            "    }\n        .padding(12)\n        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)\n",
            ".offset(x: 170, y: 4)",
        ],
    );

    let kotlin = compose_for(&doc, frame);
    assert_in_order(&kotlin, &["Box(", ".offset(x = 4.dp, y = 4.dp)", "Column(", "\n    }\n    Box(", ".offset(x = 170.dp, y = 4.dp)"]);
}

#[test]
fn hugging_frame_with_absolute_child_does_not_fill_its_container() {
    // A frame that hugs both axes takes its content's size: nothing in the export may stretch it.
    let (doc, frame) = hug_badge_column(Sizing::Hug);
    let swift = swiftui_for(&doc, frame);
    assert!(swift.starts_with("ZStack(alignment: .topLeading) {\n    VStack(alignment: .leading, spacing: 8) {"), "{swift}");
    assert!(!swift.contains(".infinity"), "a hugging frame must not fill:\n{swift}");
    assert_in_order(&swift, &["    }\n        .padding(12)\n", ".offset(x: 158, y: 2)"]);
    let kotlin = compose_for(&doc, frame);
    assert!(!kotlin.contains("fillMax"), "a hugging frame must not fill:\n{kotlin}");
    assert!(kotlin.contains("    Column(\n        modifier = Modifier.padding(12.dp),"), "{kotlin}");

    // Fixed width, hugging height: only the width fills.
    let (doc, frame) = hug_badge_column(Sizing::Fixed);
    let swift = swiftui_for(&doc, frame);
    assert_in_order(
        &swift,
        &[
            "    }\n        .padding(12)\n        .frame(maxWidth: .infinity, alignment: .topLeading)\n",
            ".offset(x: 158, y: 2)",
            "}\n    .frame(width: 200, alignment: .topLeading)",
        ],
    );
    assert!(!swift.contains("maxHeight: .infinity"), "the hugging height must not fill:\n{swift}");
    let kotlin = compose_for(&doc, frame);
    assert!(kotlin.contains("    Column(\n        modifier = Modifier\n            .fillMaxWidth()\n            .padding(12.dp),"), "{kotlin}");
    assert!(!kotlin.contains("fillMaxSize") && !kotlin.contains("fillMaxHeight"), "the hugging height must not fill:\n{kotlin}");
}

#[test]
fn variable_hidden_child_stays_in_the_stack_flow() {
    // Without an absolute sibling nothing leaves the flow: the export is the same text as before a
    // frame could carry an overlay, and the hidden child is an ordinary stack item with no offset.
    let (doc, frame) = hidden_column(false);
    let expected_swift = "\
VStack(alignment: .leading) {
    Rectangle()
        .fill(Color(red: 0.102, green: 0.2, blue: 0.302))
        .frame(width: 100, height: 24)
    Spacer()
    Rectangle()
        .fill(Color(red: 0.2, green: 0.302, blue: 0.4))
        .frame(width: 100, height: 32)
    Spacer()
    Rectangle()
        .fill(Color(red: 0.4, green: 0.4, blue: 0.4))
        .frame(width: 80, height: 20)
}
    .padding(16)
    .frame(width: 300, height: 200, alignment: .topLeading)
    .background(RoundedRectangle(cornerRadius: 8).fill(Color(red: 0.961, green: 0.961, blue: 0.98)))
    .clipShape(RoundedRectangle(cornerRadius: 8))";
    assert_eq!(swiftui_for(&doc, frame), expected_swift);
    let expected_compose = "\
Column(
    modifier = Modifier
        .size(width = 300.dp, height = 200.dp)
        .clip(RoundedCornerShape(8.dp))
        .background(Color(0xFFF5F5FA), RoundedCornerShape(8.dp))
        .padding(16.dp),
    verticalArrangement = Arrangement.SpaceBetween,
    horizontalAlignment = Alignment.Start
) {
    Box(
        modifier = Modifier
            .size(width = 100.dp, height = 24.dp)
            .background(Color(0xFF1A334D))
    )
    Box(
        modifier = Modifier
            .size(width = 100.dp, height = 32.dp)
            .background(Color(0xFF334D66))
    )
    Box(
        modifier = Modifier
            .size(width = 80.dp, height = 20.dp)
            .background(Color(0xFF666666))
    )
}";
    assert_eq!(compose_for(&doc, frame), expected_compose);

    // Next to an absolute badge the hidden child still sits in the stack, and only the badge overlays.
    let (doc, frame) = hidden_column(true);
    let swift = swiftui_for(&doc, frame);
    assert_eq!(swift.matches("Spacer()").count(), 2, "three in-flow children, two spacers:\n{swift}");
    assert!(!swift.contains(".offset(x: 16, y: 88)"), "a hidden child keeps no stale offset:\n{swift}");
    assert_in_order(
        &swift,
        &[
            "ZStack(alignment: .topLeading) {\n    VStack(alignment: .leading) {",
            "Color(red: 0.4, green: 0.4, blue: 0.4)",
            "    }\n        .padding(16)\n",
            ".offset(x: 260, y: 12)",
        ],
    );
    let kotlin = compose_for(&doc, frame);
    assert!(!kotlin.contains(".offset(x = 16.dp, y = 88.dp)"), "a hidden child keeps no stale offset:\n{kotlin}");
    assert_in_order(&kotlin, &["Column(", ".background(Color(0xFF666666))", "\n    }\n    Box(", ".offset(x = 260.dp, y = 12.dp)"]);
}

#[test]
fn grid_overlays_its_absolute_child_and_leaves_a_variable_hidden_child_in_place() {
    let (doc, grid) = tag_and_chip_grid();

    // SwiftUI: the hidden chip stays inside the Grid after its rows, with no offset; the tag overlays.
    let swift = swiftui_for(&doc, grid);
    assert!(
        swift.starts_with("ZStack(alignment: .topLeading) {\n    Grid(alignment: .topLeading, horizontalSpacing: 10, verticalSpacing: 10) {"),
        "the grid sits inside a ZStack:\n{swift}"
    );
    assert!(!swift.contains(".offset(x: 14, y: 70)"), "a hidden chip keeps no stale offset:\n{swift}");
    assert_in_order(
        &swift,
        &[
            "GridRow {",
            "Color(red: 0.502, green: 0.502, blue: 0.502)",
            "    }\n        .padding(10)\n        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)\n",
            ".offset(x: 180, y: 6)",
            "}\n    .frame(width: 220, height: 100, alignment: .topLeading)",
        ],
    );

    // Compose: the tag keeps its offset and the chip has none.
    let kotlin = compose_for(&doc, grid);
    assert!(!kotlin.contains(".offset(x = 14.dp, y = 70.dp)"), "a hidden chip keeps no stale offset:\n{kotlin}");
    assert_in_order(&kotlin, &["Box(", "LazyVerticalGrid(", "\n    }\n    Box(", ".offset(x = 180.dp, y = 6.dp)", ".background(Color(0xFF808080))"]);
}

#[test]
fn variable_hidden_grid_child_exports_where_it_was_without_an_overlay() {
    let (doc, grid) = chip_grid();
    let expected_swift = "\
Grid(alignment: .topLeading, horizontalSpacing: 10, verticalSpacing: 10) {
    GridRow {
        Rectangle()
            .fill(Color(red: 0.2, green: 0.4, blue: 0.8))
            .frame(maxWidth: .infinity, minHeight: 40, maxHeight: 40)
            .frame(minWidth: 0, maxWidth: .infinity, minHeight: 40, maxHeight: 40, alignment: .topLeading)
        Rectangle()
            .fill(Color(red: 0.302, green: 0.502, blue: 0.902))
            .frame(maxWidth: .infinity, minHeight: 40, maxHeight: 40)
            .frame(minWidth: 0, maxWidth: .infinity, minHeight: 40, maxHeight: 40, alignment: .topLeading)
    }
    Rectangle()
        .fill(Color(red: 0.502, green: 0.502, blue: 0.502))
        .frame(width: 60, height: 20)
}
    .padding(10)
    .frame(width: 220, height: 100, alignment: .topLeading)
    .background(RoundedRectangle(cornerRadius: 8).fill(Color(red: 0.961, green: 0.961, blue: 0.98)))
    .clipShape(RoundedRectangle(cornerRadius: 8))";
    assert_eq!(swiftui_for(&doc, grid), expected_swift);

    let expected_compose = "\
Box(
    modifier = Modifier
        .size(width = 220.dp, height = 100.dp)
        .clip(RoundedCornerShape(8.dp))
        .background(Color(0xFFF5F5FA), RoundedCornerShape(8.dp))
) {
    LazyVerticalGrid(
        columns = object : GridCells { override fun Density.calculateCrossAxisCellSizes(availableSize: Int, spacing: Int): List<Int> { val fixed = listOf(0.dp, 0.dp).map { it.roundToPx() }; val weights = listOf(1f, 1f); val unit = (availableSize - spacing * 1 - fixed.sum()).coerceAtLeast(0) / weights.sum(); return fixed.indices.map { fixed[it] + (weights[it] * unit + 0.5f).toInt() } } },
        modifier = Modifier
            .fillMaxSize()
            .padding(10.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
        userScrollEnabled = false
    ) {
        item {
            Box(Modifier.height(40.dp), contentAlignment = Alignment.TopStart) {
                Box(
                    modifier = Modifier
                        .height(40.dp)
                        .fillMaxWidth()
                        .background(Color(0xFF3366CC))
                )
            }
        }
        item {
            Box(Modifier.height(40.dp), contentAlignment = Alignment.TopStart) {
                Box(
                    modifier = Modifier
                        .height(40.dp)
                        .fillMaxWidth()
                        .background(Color(0xFF4D80E6))
                )
            }
        }
    }
    Box(
        modifier = Modifier
            .size(width = 60.dp, height = 20.dp)
            .background(Color(0xFF808080))
    )
}";
    assert_eq!(compose_for(&doc, grid), expected_compose);
}

#[test]
fn grid_absolute_child_before_the_cells_paints_behind_the_grid() {
    let (doc, grid) = underlay_grid();

    let swift = swiftui_for(&doc, grid);
    assert_in_order(
        &swift,
        &["ZStack(alignment: .topLeading) {", ".offset(x: 4, y: 4)", "Grid(alignment: .topLeading", "    }\n        .padding(10)\n", ".offset(x: 180, y: 6)"],
    );

    let kotlin = compose_for(&doc, grid);
    assert_in_order(&kotlin, &["Box(", ".offset(x = 4.dp, y = 4.dp)", "LazyVerticalGrid(", "\n    }\n    Box(", ".offset(x = 180.dp, y = 6.dp)"]);
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
    // 11. column_with_absolute_child
    out.push(("11_column_with_absolute_child", swiftui_for_fixture(badge_column())));
    // 12. row_with_absolute_pin
    out.push(("12_row_with_absolute_pin", swiftui_for_fixture(pin_row())));
    // 13. frame_with_only_absolute_children
    out.push(("13_frame_with_only_absolute_children", swiftui_for_fixture(absolute_only_frame())));
    // 14. column_with_absolute_underlay_first
    out.push(("14_column_with_absolute_underlay_first", swiftui_for_fixture(underlay_column())));
    // 15. hugging_column_with_absolute_badge
    out.push(("15_hugging_column_with_absolute_badge", swiftui_for_fixture(hug_badge_column(Sizing::Hug))));
    // 16. fixed_width_hugging_height_column_with_absolute_badge
    out.push(("16_fixed_width_hugging_height_column_with_absolute_badge", swiftui_for_fixture(hug_badge_column(Sizing::Fixed))));
    // 17. column_with_variable_hidden_child_and_badge
    out.push(("17_column_with_variable_hidden_child_and_badge", swiftui_for_fixture(hidden_column(true))));
    // 18. grid_with_absolute_tag_and_variable_hidden_chip
    out.push(("18_grid_with_absolute_tag_and_variable_hidden_chip", swiftui_for_fixture(tag_and_chip_grid())));
    // 19. grid_with_absolute_underlay_and_tag
    out.push(("19_grid_with_absolute_underlay_and_tag", swiftui_for_fixture(underlay_grid())));

    out
}

fn swiftui_for_fixture((doc, root): (Document, NodeId)) -> String {
    swiftui_for(&doc, root)
}

const SWIFTC: &str = "/usr/bin/swiftc";

#[test]
fn every_swiftui_test_fixture_passes_swiftc_typecheck() {
    let fixtures = build_swiftui_fixtures_for_typecheck();
    assert_eq!(fixtures.len(), 19);

    // CI images without a Swift toolchain (Linux, Windows) cannot typecheck: say so and pass.
    if !Path::new(SWIFTC).exists() {
        eprintln!("swiftc_typecheck_summary: skipped, {SWIFTC} is not installed on this machine");
        return;
    }

    let tmp = std::env::temp_dir().join(format!("designos-swiftc-check-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("create temp dir for swiftc");

    let mut failures = Vec::new();
    for (idx, (name, body)) in fixtures.iter().enumerate() {
        let indented = body.lines().map(|l| format!("        {l}")).collect::<Vec<_>>().join("\n");
        let swift_src = format!("import SwiftUI\n\nstruct Fixture_{idx}: View {{\n    var body: some View {{\n{indented}\n    }}\n}}\n");
        let path = tmp.join(format!("{name}.swift"));
        std::fs::write(&path, &swift_src).expect("write swift file");
        let output = Command::new(SWIFTC).arg("-typecheck").arg(&path).output().expect("invoke swiftc");
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
