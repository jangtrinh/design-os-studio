//! Issue #484 (native half): checks that SwiftUI, Compose, UIKit and Android XML export grids
//! with the layout solver's tracks, cell boxes, spans, holes, bound gap tokens and out-of-flow
//! children.

use designos_doc::layout_solver::{compute_grid, grid_column_widths, grid_row_heights, place_children, ChildIn, Placed, SizeLimits};
use designos_doc::{
    AutoLayout, CodeSyntaxPlatform, Color, Document, FrameProps, GridTrack, LayoutDirection, Node, NodeId, NodeKind, Paint, Rect, Sizing, Variable,
    VariableCollection, VariableKind, VariableMode, VariableValue, Vec2,
};
use designos_export::{android_xml_for, compose_for, swiftui_for, uikit_for};

pub struct NativeGridScene {
    pub name: &'static str,
    pub doc: Document,
    pub grid: NodeId,
    pub items: Vec<NodeId>,
}

fn fmt_num(v: f32) -> String {
    let r = (v * 100.0).round() / 100.0;
    if (r - r.round()).abs() < 1e-4 {
        format!("{}", r.round() as i64)
    } else {
        format!("{r}")
    }
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

fn add_named_item(doc: &mut Document, grid: NodeId, name: &str, w: f32, h: f32, horiz: Sizing, vert: Sizing) -> NodeId {
    let id = doc.alloc_id();
    let mut rect = Node::rectangle(id, Rect::new(0.0, 0.0, w, h));
    rect.name = name.into();
    rect.fills = vec![Paint::solid(Color::rgb(0x35, 0x82, 0xFF))];
    rect.layout_child.horizontal = horiz;
    rect.layout_child.vertical = vert;
    doc.insert(grid, rect).unwrap()
}

fn add_item(doc: &mut Document, grid: NodeId, w: f32, h: f32, horiz: Sizing, vert: Sizing) -> NodeId {
    let idx = doc.get(grid).map(|n| n.children.len() + 1).unwrap_or(1);
    add_named_item(doc, grid, &format!("Item {idx}"), w, h, horiz, vert)
}

pub fn scene_equal_tracks() -> NativeGridScene {
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
    let (mut doc, grid) = new_grid(al, Vec2::new(324.0, 180.0));
    let mut items = Vec::new();
    for _ in 0..6 {
        items.push(add_item(&mut doc, grid, 80.0, 36.0, Sizing::Fill, Sizing::Fixed));
    }
    doc.relayout_subtree(grid);
    NativeGridScene { name: "equal_tracks", doc, grid, items }
}

pub fn scene_unequal_tracks() -> NativeGridScene {
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
    let (mut doc, grid) = new_grid(al, Vec2::new(500.0, 140.0));
    let items = vec![
        add_item(&mut doc, grid, 100.0, 30.0, Sizing::Fixed, Sizing::Fixed),
        add_item(&mut doc, grid, 40.0, 30.0, Sizing::Fixed, Sizing::Fixed),
        add_item(&mut doc, grid, 60.0, 30.0, Sizing::Fixed, Sizing::Fixed),
        add_item(&mut doc, grid, 50.0, 30.0, Sizing::Fill, Sizing::Fixed),
        add_item(&mut doc, grid, 50.0, 30.0, Sizing::Fill, Sizing::Fixed),
        add_item(&mut doc, grid, 50.0, 30.0, Sizing::Fill, Sizing::Fixed),
    ];
    doc.relayout_subtree(grid);
    NativeGridScene { name: "unequal_tracks", doc, grid, items }
}

pub fn scene_fixed_first_column() -> NativeGridScene {
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
    let (mut doc, grid) = new_grid(al, Vec2::new(440.0, 180.0));
    let mut items = Vec::new();
    for _ in 0..6 {
        items.push(add_item(&mut doc, grid, 50.0, 40.0, Sizing::Fill, Sizing::Fixed));
    }
    doc.relayout_subtree(grid);
    NativeGridScene { name: "fixed_first_column", doc, grid, items }
}

pub fn scene_hugging_grid() -> NativeGridScene {
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
    let (mut doc, grid) = new_grid(al, Vec2::new(500.0, 500.0));
    let items = vec![
        add_item(&mut doc, grid, 100.0, 28.0, Sizing::Fixed, Sizing::Fixed),
        add_item(&mut doc, grid, 24.0, 28.0, Sizing::Fixed, Sizing::Fixed),
        add_item(&mut doc, grid, 24.0, 28.0, Sizing::Fixed, Sizing::Fixed),
        add_item(&mut doc, grid, 50.0, 20.0, Sizing::Fill, Sizing::Fixed),
        add_item(&mut doc, grid, 50.0, 20.0, Sizing::Fill, Sizing::Fixed),
        add_item(&mut doc, grid, 50.0, 20.0, Sizing::Fill, Sizing::Fixed),
    ];
    doc.relayout_subtree(grid);
    NativeGridScene { name: "hugging_grid", doc, grid, items }
}

pub fn scene_fixed_height_auto_rows() -> NativeGridScene {
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
    let (mut doc, grid) = new_grid(al, Vec2::new(260.0, 240.0));
    let items = vec![
        add_item(&mut doc, grid, 50.0, 24.0, Sizing::Fill, Sizing::Fixed),
        add_item(&mut doc, grid, 50.0, 24.0, Sizing::Fill, Sizing::Fixed),
        add_item(&mut doc, grid, 50.0, 36.0, Sizing::Fill, Sizing::Fixed),
        add_item(&mut doc, grid, 50.0, 36.0, Sizing::Fill, Sizing::Fixed),
        add_item(&mut doc, grid, 50.0, 28.0, Sizing::Fill, Sizing::Fixed),
        add_item(&mut doc, grid, 50.0, 28.0, Sizing::Fill, Sizing::Fixed),
    ];
    doc.relayout_subtree(grid);
    NativeGridScene { name: "fixed_height_auto_rows", doc, grid, items }
}

pub fn scene_row_and_column_spans() -> NativeGridScene {
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
    let (mut doc, grid) = new_grid(al, Vec2::new(428.0, 220.0));
    let hero = add_named_item(&mut doc, grid, "HeroTile", 196.0, 40.0, Sizing::Fixed, Sizing::Fill);
    doc.get_mut(hero).unwrap().layout_child.col_span = 2;
    doc.get_mut(hero).unwrap().layout_child.row_span = 2;

    let top_wide = add_named_item(&mut doc, grid, "TopWide", 196.0, 40.0, Sizing::Fixed, Sizing::Fixed);
    doc.get_mut(top_wide).unwrap().layout_child.col_span = 2;

    let mid_a = add_named_item(&mut doc, grid, "MidA", 92.0, 40.0, Sizing::Fixed, Sizing::Fixed);
    let mid_b = add_named_item(&mut doc, grid, "MidB", 92.0, 40.0, Sizing::Fixed, Sizing::Fixed);

    let footer = add_named_item(&mut doc, grid, "FooterBar", 404.0, 36.0, Sizing::Fixed, Sizing::Fixed);
    doc.get_mut(footer).unwrap().layout_child.grid_column = Some(0);
    doc.get_mut(footer).unwrap().layout_child.grid_row = Some(2);
    doc.get_mut(footer).unwrap().layout_child.col_span = 4;
    doc.get_mut(footer).unwrap().layout_child.row_span = 1;

    doc.relayout_subtree(grid);
    NativeGridScene { name: "row_and_column_spans", doc, grid, items: vec![hero, top_wide, mid_a, mid_b, footer] }
}

pub fn build_six_scenes() -> Vec<NativeGridScene> {
    vec![
        scene_equal_tracks(),
        scene_unequal_tracks(),
        scene_fixed_first_column(),
        scene_hugging_grid(),
        scene_fixed_height_auto_rows(),
        scene_row_and_column_spans(),
    ]
}

fn solver_expectations(scene: &NativeGridScene) -> (Vec<f32>, Vec<f32>, Vec<Placed>, f32, f32, [f32; 4]) {
    let n = scene.doc.get(scene.grid).unwrap();
    let al = n.kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
    let al_s = al.with_stroke(&n.stroke);
    let own = SizeLimits::from(&n.layout_child);
    let kids: Vec<ChildIn> = n.children.iter().filter_map(|id| scene.doc.flow_child_input(*id, true)).collect();
    let widths = grid_column_widths(&al_s, n.size, own, &kids);
    let rows = grid_row_heights(&al_s, n.size, own, &kids);
    let placed = place_children(&kids, widths.len());
    let col_gap = al_s.item_spacing.max(0.0);
    let row_gap = if al_s.counter_spacing > 0.0 { al_s.counter_spacing } else { col_gap };
    (widths, rows, placed, col_gap, row_gap, al.padding)
}

fn assert_swiftui_scene(scene: &NativeGridScene) {
    let name = scene.name;
    let (widths, rows, placed, col_gap, _, _) = solver_expectations(scene);
    let swift = swiftui_for(&scene.doc, scene.grid);
    assert!(swift.contains("Grid("), "{name} SwiftUI missing Grid(: {swift}");
    assert_eq!(swift.matches("GridRow {").count(), rows.len(), "{name} SwiftUI GridRow count != {}: {swift}", rows.len());
    for &w in &widths {
        let ws = fmt_num(w);
        assert!(swift.contains(&format!("width: {ws}, height:")), "{name} SwiftUI missing column width {ws}: {swift}");
    }
    for &h in &rows {
        let hs = fmt_num(h);
        assert!(swift.contains(&format!("height: {hs}")), "{name} SwiftUI missing row height {hs}: {swift}");
    }
    for p in &placed {
        let cell_w: f32 = (0..p.col_span).map(|dc| widths[p.col + dc]).sum::<f32>() + col_gap * (p.col_span.saturating_sub(1) as f32);
        let cell_h_first = rows[p.row];
        let needle = format!(".frame(width: {}, height: {}, alignment:", fmt_num(cell_w), fmt_num(cell_h_first));
        assert!(swift.contains(&needle), "{name} SwiftUI missing cell frame {needle}: {swift}");
        if p.col_span == 2 {
            assert!(swift.contains(".gridCellColumns(2)"), "{name} SwiftUI missing .gridCellColumns(2): {swift}");
        }
    }
}

fn assert_compose_scene(scene: &NativeGridScene) {
    let name = scene.name;
    let (widths, rows, placed, _, row_gap, padding) = solver_expectations(scene);
    let compose = compose_for(&scene.doc, scene.grid);
    assert!(compose.contains("LazyVerticalGrid("), "{name} Compose missing LazyVerticalGrid(: {compose}");
    let widths_dp = widths.iter().map(|w| format!("{}.dp", fmt_num(*w))).collect::<Vec<_>>().join(", ");
    assert!(compose.contains(&format!("listOf({widths_dp})")), "{name} Compose missing listOf({widths_dp}): {compose}");
    for &h in &rows {
        let hs = fmt_num(h);
        assert!(compose.contains(&format!("Modifier.height({hs}.dp)")), "{name} Compose missing row height {hs}.dp: {compose}");
    }
    if placed.iter().any(|p| p.col_span == 2) {
        assert!(compose.contains("GridItemSpan(2)"), "{name} Compose missing GridItemSpan(2): {compose}");
    }
    let compose_h = rows.iter().sum::<f32>() + row_gap * (rows.len().saturating_sub(1) as f32) + padding[0] + padding[2];
    let chs = fmt_num(compose_h);
    assert!(
        compose.contains(&format!("height = {chs}.dp")) || compose.contains(&format!(".height({chs}.dp)")) || compose.contains(&format!(".size({chs}.dp)")),
        "{name} Compose missing explicit container height {chs}.dp: {compose}"
    );
}

fn assert_uikit_scene(scene: &NativeGridScene) {
    let name = scene.name;
    let (widths, rows, placed, col_gap, _, _) = solver_expectations(scene);
    let uikit = uikit_for(&scene.doc, scene.grid);
    assert_eq!(uikit.matches(".axis = .horizontal").count(), rows.len(), "{name} UIKit horizontal row stack count != {}: {uikit}", rows.len());
    for p in &placed {
        let cell_w: f32 = (0..p.col_span).map(|dc| widths[p.col + dc]).sum::<f32>() + col_gap * (p.col_span.saturating_sub(1) as f32);
        let cell_h_first = rows[p.row];
        let w_con = format!("widthAnchor.constraint(equalToConstant: {})", fmt_num(cell_w));
        let h_con = format!("heightAnchor.constraint(equalToConstant: {})", fmt_num(cell_h_first));
        assert!(uikit.contains(&w_con), "{name} UIKit missing {w_con}: {uikit}");
        assert!(uikit.contains(&h_con), "{name} UIKit missing {h_con}: {uikit}");
    }
}

fn assert_android_scene(scene: &NativeGridScene) {
    let name = scene.name;
    let (widths, rows, placed, col_gap, row_gap, _) = solver_expectations(scene);
    let xml = android_xml_for(&scene.doc, scene.grid);
    assert!(xml.contains("<GridLayout"), "{name} Android missing <GridLayout: {xml}");
    assert!(xml.contains(&format!("android:columnCount=\"{}\"", widths.len())), "{name} Android missing columnCount={}: {xml}", widths.len());
    assert!(xml.contains(&format!("android:rowCount=\"{}\"", rows.len())), "{name} Android missing rowCount={}: {xml}", rows.len());
    for p in &placed {
        let cell_w: f32 = (0..p.col_span).map(|dc| widths[p.col + dc]).sum::<f32>() + col_gap * (p.col_span.saturating_sub(1) as f32);
        let cell_h_span: f32 = (0..p.row_span).map(|dr| rows[p.row + dr]).sum::<f32>() + row_gap * (p.row_span.saturating_sub(1) as f32);
        assert!(xml.contains(&format!("android:layout_row=\"{}\"", p.row)), "{name} Android missing layout_row={}: {xml}", p.row);
        assert!(xml.contains(&format!("android:layout_column=\"{}\"", p.col)), "{name} Android missing layout_column={}: {xml}", p.col);
        assert!(xml.contains(&format!("android:layout_width=\"{}dp\"", fmt_num(cell_w))), "{name} Android missing cell width {}dp: {xml}", fmt_num(cell_w));
        assert!(
            xml.contains(&format!("android:layout_height=\"{}dp\"", fmt_num(cell_h_span))),
            "{name} Android missing cell height {}dp: {xml}",
            fmt_num(cell_h_span)
        );
        if p.row_span > 1 {
            assert!(xml.contains(&format!("android:layout_rowSpan=\"{}\"", p.row_span)), "{name} Android missing layout_rowSpan={}: {xml}", p.row_span);
        }
        if p.col_span > 1 {
            assert!(xml.contains(&format!("android:layout_columnSpan=\"{}\"", p.col_span)), "{name} Android missing layout_columnSpan={}: {xml}", p.col_span);
        }
    }
}

fn assert_scene_outputs(scene: &NativeGridScene) {
    assert_swiftui_scene(scene);
    assert_compose_scene(scene);
    assert_uikit_scene(scene);
    assert_android_scene(scene);
}

#[test]
fn item3_swiftui_all_six_scenes() {
    for scene in build_six_scenes() {
        assert_swiftui_scene(&scene);
    }
}

#[test]
fn item3_compose_all_six_scenes() {
    for scene in build_six_scenes() {
        assert_compose_scene(&scene);
    }
}

#[test]
fn item3_uikit_all_six_scenes() {
    for scene in build_six_scenes() {
        assert_uikit_scene(&scene);
    }
}

#[test]
fn item3_android_xml_all_six_scenes() {
    for scene in build_six_scenes() {
        assert_android_scene(&scene);
    }
}

#[test]
fn equal_tracks_native_generators() {
    assert_scene_outputs(&scene_equal_tracks());
}

#[test]
fn unequal_tracks_native_generators() {
    assert_scene_outputs(&scene_unequal_tracks());
}

#[test]
fn fixed_first_column_native_generators() {
    assert_scene_outputs(&scene_fixed_first_column());
}

#[test]
fn hugging_grid_native_generators() {
    assert_scene_outputs(&scene_hugging_grid());
}

#[test]
fn fixed_height_auto_rows_native_generators() {
    assert_scene_outputs(&scene_fixed_height_auto_rows());
}

#[test]
fn row_and_column_spans_native_generators() {
    let scene = scene_row_and_column_spans();
    assert_scene_outputs(&scene);

    // Acceptance #4 (R5): SwiftUI, Compose and UIKit emit the row-span comment with the node's name.
    for (platform, out) in
        [("SwiftUI", swiftui_for(&scene.doc, scene.grid)), ("Compose", compose_for(&scene.doc, scene.grid)), ("UIKit", uikit_for(&scene.doc, scene.grid))]
    {
        assert!(
            out.contains("// \"HeroTile\": row span 2 is not supported by ") && out.contains("; placed in row 1"),
            "{platform} missing row-span comment for HeroTile:\n{out}"
        );
    }
}

#[test]
fn explicit_hole_emits_filler_and_keeps_column_width() {
    let mut al = AutoLayout {
        grid_columns: 3,
        grid_rows: 1,
        item_spacing: 10.0,
        counter_spacing: 10.0,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Hug,
        ..Default::default()
    };
    al.grid_col_tracks[1] = GridTrack::Fixed(75.0);
    let (mut doc, grid) = new_grid(al, Vec2::new(305.0, 40.0));
    let first = add_named_item(&mut doc, grid, "LeftItem", 60.0, 32.0, Sizing::Fill, Sizing::Fixed);
    doc.get_mut(first).unwrap().layout_child.grid_column = Some(0);
    doc.get_mut(first).unwrap().layout_child.grid_row = Some(0);
    let last = add_named_item(&mut doc, grid, "RightItem", 60.0, 32.0, Sizing::Fill, Sizing::Fixed);
    doc.get_mut(last).unwrap().layout_child.grid_column = Some(2);
    doc.get_mut(last).unwrap().layout_child.grid_row = Some(0);
    doc.relayout_subtree(grid);

    let scene = NativeGridScene { name: "explicit_hole", doc, grid, items: vec![first, last] };
    let (widths, rows, _, _, _, _) = solver_expectations(&scene);
    let hole_w = fmt_num(widths[1]);
    let row_h = fmt_num(rows[0]);

    let swift = swiftui_for(&scene.doc, scene.grid);
    assert!(swift.contains(&format!("Color.clear.frame(width: {hole_w}, height: {row_h})")), "SwiftUI missing hole filler: {swift}");

    let compose = compose_for(&scene.doc, scene.grid);
    assert!(compose.contains(&format!("item {{ Spacer(Modifier.height({row_h}.dp)) }}")), "Compose missing hole filler: {compose}");
    assert!(compose.contains(&format!("{hole_w}.dp")), "Compose missing hole column width {hole_w}.dp: {compose}");

    let uikit = uikit_for(&scene.doc, scene.grid);
    assert!(uikit.contains(&format!("widthAnchor.constraint(equalToConstant: {hole_w})")), "UIKit missing hole width constraint {hole_w}: {uikit}");

    let xml = android_xml_for(&scene.doc, scene.grid);
    assert!(xml.contains("<Space"), "Android XML missing <Space filler: {xml}");
    assert!(xml.contains(&format!("android:layout_width=\"{hole_w}dp\"")), "Android XML missing hole width {hole_w}dp: {xml}");
}

#[test]
fn bound_item_spacing_prints_token_in_all_four_generators() {
    let al = AutoLayout {
        grid_columns: 2,
        grid_rows: 0,
        item_spacing: 16.0,
        counter_spacing: 0.0,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Hug,
        ..Default::default()
    };
    let (mut doc, grid) = new_grid(al, Vec2::new(240.0, 100.0));
    doc.variable_collections.insert(
        "c".into(),
        VariableCollection {
            id: "c".into(),
            name: "Tokens".into(),
            modes: vec![VariableMode { id: "m".into(), name: "Mode".into() }],
            default_mode: "m".into(),
            variables: vec!["gap_md".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    doc.variables.insert(
        "gap_md".into(),
        Variable {
            id: "gap_md".into(),
            name: "Spacing/GridGap".into(),
            collection: "c".into(),
            kind: VariableKind::Number,
            values: [("m".to_string(), VariableValue::Number(16.0))].into_iter().collect(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,
            code_syntax: [(CodeSyntaxPlatform::Ios, "Tokens.spacingGridGap".to_string()), (CodeSyntaxPlatform::Android, "Tokens.spacingGridGap".to_string())]
                .into_iter()
                .collect(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        },
    );
    doc.get_mut(grid).unwrap().variables.bindings.insert("itemSpacing".into(), "gap_md".into());
    add_item(&mut doc, grid, 80.0, 32.0, Sizing::Fill, Sizing::Fixed);
    add_item(&mut doc, grid, 80.0, 32.0, Sizing::Fill, Sizing::Fixed);
    doc.relayout_subtree(grid);

    let swift = swiftui_for(&doc, grid);
    assert!(swift.contains("Grid(") && swift.contains("Tokens.spacingGridGap"), "SwiftUI: {swift}");
    assert!(!swift.contains("horizontalSpacing: 16"), "SwiftUI printed raw number: {swift}");

    let compose = compose_for(&doc, grid);
    assert!(compose.contains("LazyVerticalGrid(") && compose.contains("Arrangement.spacedBy(Tokens.spacingGridGap)"), "Compose: {compose}");
    assert!(!compose.contains("spacedBy(16.dp)"), "Compose printed raw number: {compose}");

    let uikit = uikit_for(&doc, grid);
    assert!(uikit.contains(".axis = .horizontal") && uikit.contains(".spacing = Tokens.spacingGridGap"), "UIKit: {uikit}");
    assert!(!uikit.contains(".spacing = 16"), "UIKit printed raw number: {uikit}");

    let xml = android_xml_for(&doc, grid);
    assert!(xml.contains("<GridLayout") && xml.contains("Tokens.spacingGridGap"), "Android XML: {xml}");
    assert!(!xml.contains("16dp"), "Android XML printed raw number: {xml}");
}

#[test]
fn absolute_child_in_grid_keeps_generator_handling_in_all_four_generators() {
    let al = AutoLayout {
        grid_columns: 2,
        grid_rows: 0,
        item_spacing: 10.0,
        counter_spacing: 10.0,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Hug,
        ..Default::default()
    };
    let (mut doc, grid) = new_grid(al, Vec2::new(210.0, 80.0));
    add_named_item(&mut doc, grid, "CellA", 80.0, 36.0, Sizing::Fill, Sizing::Fixed);
    add_named_item(&mut doc, grid, "CellB", 80.0, 36.0, Sizing::Fill, Sizing::Fixed);
    let badge = add_named_item(&mut doc, grid, "FloatingBadge", 24.0, 18.0, Sizing::Fixed, Sizing::Fixed);
    {
        let b = doc.get_mut(badge).unwrap();
        b.layout_child.absolute = true;
        b.transform.tx = 15.0;
        b.transform.ty = 25.0;
    }
    doc.relayout_subtree(grid);

    let swift = swiftui_for(&doc, grid);
    assert!(swift.contains("Grid("), "SwiftUI: {swift}");
    assert!(swift.contains(".offset(x: 15, y: 25)"), "SwiftUI missing absolute offset: {swift}");
    assert!(!swift.contains(".frame(width: 24, height: 18)\n            .frame("), "SwiftUI wrapped absolute child in a cell box: {swift}");

    let compose = compose_for(&doc, grid);
    assert!(compose.contains("LazyVerticalGrid("), "Compose: {compose}");
    assert!(compose.contains(".offset(x = 15.dp, y = 25.dp)"), "Compose missing absolute offset: {compose}");

    let uikit = uikit_for(&doc, grid);
    assert_eq!(uikit.matches(".axis = .horizontal").count(), 1, "UIKit: {uikit}");
    assert!(uikit.contains("floatingBadge.frame = CGRect(x: 15, y: 25, width: 24, height: 18)"), "UIKit missing CGRect frame for absolute child: {uikit}");
    assert!(uikit.contains("gridFrame.addSubview(floatingBadge)"), "UIKit should add absolute child with addSubview: {uikit}");

    let xml = android_xml_for(&doc, grid);
    assert!(xml.contains("<GridLayout"), "Android XML: {xml}");
    assert!(
        xml.contains("android:layout_marginStart=\"15dp\"") && xml.contains("android:layout_marginTop=\"25dp\""),
        "Android XML missing absolute child margins: {xml}"
    );
    assert_eq!(xml.matches("<FrameLayout").count(), 2, "Android XML should only wrap the 2 flow children in cell FrameLayouts: {xml}");
}

#[test]
fn solver_row_heights_match_compute_grid_across_all_six_scenes() {
    for scene in build_six_scenes() {
        let name = scene.name;
        let n = scene.doc.get(scene.grid).unwrap();
        let al = n.kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
        let al_s = al.with_stroke(&n.stroke);
        let own = SizeLimits::from(&n.layout_child);
        let kids: Vec<ChildIn> = n.children.iter().filter_map(|id| scene.doc.flow_child_input(*id, true)).collect();
        let widths = grid_column_widths(&al_s, n.size, own, &kids);
        let rows = grid_row_heights(&al_s, n.size, own, &kids);
        let out = compute_grid(&al_s, n.size, own, &kids);
        let placed = place_children(&kids, widths.len());
        let col_gap = al_s.item_spacing.max(0.0);
        let row_gap = if al_s.counter_spacing > 0.0 { al_s.counter_spacing } else { col_gap };
        let [pt, _, pb, _] = al_s.padding;

        for ((p, k), rect) in placed.iter().zip(&kids).zip(&out.rects) {
            let expected_y = pt + rows[..p.row].iter().sum::<f32>() + (p.row as f32) * row_gap;
            assert!((rect.y - expected_y).abs() < 1e-4, "{name}: rect.y={} != {expected_y}", rect.y);
            if p.row_span == 1 && k.cross_fill {
                assert!((rect.h - rows[p.row]).abs() < 1e-4, "{name}: rect.h={} != {}", rect.h, rows[p.row]);
            }
        }
        if al_s.counter_sizing == Sizing::Hug {
            let expected_h = pt + pb + rows.iter().sum::<f32>() + (rows.len().saturating_sub(1) as f32) * row_gap;
            assert!((out.frame.y - expected_h).abs() < 1e-4, "{name}: frame.y={} != {expected_h}", out.frame.y);
        }
    }
}

#[test]
fn syntax_smoke_android_xml_and_swiftui_span_scene() {
    let ev_dir = std::env::var("AGY_484_EVIDENCE_DIR").ok().map(std::path::PathBuf::from);
    if let Some(ref d) = ev_dir {
        let _ = std::fs::create_dir_all(d);
    }
    for scene in build_six_scenes() {
        let xml = android_xml_for(&scene.doc, scene.grid);
        if let Some(ref d) = ev_dir {
            std::fs::write(d.join(format!("scene_{}.xml", scene.name)), &xml).unwrap();
        }
    }
    let span = scene_row_and_column_spans();
    let span_swift = swiftui_for(&span.doc, span.grid);
    let span_xml = android_xml_for(&span.doc, span.grid);
    let span_compose = compose_for(&span.doc, span.grid);
    let span_uikit = uikit_for(&span.doc, span.grid);
    if let Some(ref d) = ev_dir {
        std::fs::write(d.join("span_swiftui.swift"), &span_swift).unwrap();
        std::fs::write(d.join("span_android.xml"), &span_xml).unwrap();
        std::fs::write(d.join("span_compose.kt"), &span_compose).unwrap();
        std::fs::write(d.join("span_uikit.swift"), &span_uikit).unwrap();
    }
    assert!(span_xml.contains("<GridLayout") && span_xml.contains("</GridLayout>"));
    assert!(span_swift.contains("Grid(") && span_swift.contains("GridRow {"));
}
