//! Issue #484 (native half): checks that SwiftUI, Compose, UIKit and Android XML export grids
//! with the layout solver's tracks, cell boxes, spans, holes, bound gap tokens and out-of-flow
//! children.

use designos_doc::layout_solver::{compute_grid, grid_column_widths, grid_row_heights, place_children, ChildIn, Placed, SizeLimits};
use designos_doc::{
    create_instance, instances_of, AutoLayout, CodeSyntaxPlatform, Color, ComponentPropDef, Document, FrameProps, GridChildAlign, GridTrack, LayoutDirection,
    Node, NodeId, NodeKind, OverrideField, Paint, PropKind, PropValue, Rect, Sizing, TextStyle, Variable, VariableCollection, VariableKind, VariableMode,
    VariableValue, Vec2,
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

fn solver_kids(scene: &NativeGridScene) -> Vec<ChildIn> {
    let n = scene.doc.get(scene.grid).unwrap();
    n.children.iter().filter_map(|id| scene.doc.flow_child_input(*id, true)).collect()
}

fn effective_align(k: &ChildIn) -> (GridChildAlign, GridChildAlign) {
    let norm = |a: GridChildAlign| match a {
        GridChildAlign::Center => GridChildAlign::Center,
        GridChildAlign::End => GridChildAlign::End,
        GridChildAlign::Start | GridChildAlign::Auto => GridChildAlign::Start,
    };
    let ah = if k.main_fill { GridChildAlign::Start } else { norm(k.cell.align_h) };
    let av = if k.cross_fill { GridChildAlign::Start } else { norm(k.cell.align_v) };
    (ah, av)
}

fn assert_swiftui_scene(scene: &NativeGridScene) {
    let name = scene.name;
    let (widths, rows, placed, col_gap, _, _) = solver_expectations(scene);
    let kids = solver_kids(scene);
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
    for (p, k) in placed.iter().zip(&kids) {
        let cell_w: f32 = (0..p.col_span).map(|dc| widths[p.col + dc]).sum::<f32>() + col_gap * (p.col_span.saturating_sub(1) as f32);
        let cell_h_first = rows[p.row];
        let (ah, av) = effective_align(k);
        let swift_align = match (ah, av) {
            (GridChildAlign::Center, GridChildAlign::Start) => ".top",
            (GridChildAlign::End, GridChildAlign::Start) => ".topTrailing",
            (GridChildAlign::Start, GridChildAlign::Center) => ".leading",
            (GridChildAlign::Center, GridChildAlign::Center) => ".center",
            (GridChildAlign::End, GridChildAlign::Center) => ".trailing",
            (GridChildAlign::Start, GridChildAlign::End) => ".bottomLeading",
            (GridChildAlign::Center, GridChildAlign::End) => ".bottom",
            (GridChildAlign::End, GridChildAlign::End) => ".bottomTrailing",
            _ => ".topLeading",
        };
        let needle = format!(".frame(width: {}, height: {}, alignment: {swift_align})", fmt_num(cell_w), fmt_num(cell_h_first));
        assert!(swift.contains(&needle), "{name} SwiftUI missing cell frame {needle}: {swift}");
        if p.col_span > 1 {
            assert!(swift.contains(&format!(".gridCellColumns({})", p.col_span)), "{name} SwiftUI missing .gridCellColumns({}): {swift}", p.col_span);
        }
    }
}

fn assert_compose_scene(scene: &NativeGridScene) {
    let name = scene.name;
    let (widths, rows, placed, _, row_gap, padding) = solver_expectations(scene);
    let kids = solver_kids(scene);
    let compose = compose_for(&scene.doc, scene.grid);
    assert!(compose.contains("LazyVerticalGrid("), "{name} Compose missing LazyVerticalGrid(: {compose}");
    let widths_dp = widths.iter().map(|w| format!("{}.dp", fmt_num(*w))).collect::<Vec<_>>().join(", ");
    assert!(compose.contains(&format!("listOf({widths_dp})")), "{name} Compose missing listOf({widths_dp}): {compose}");
    for &h in &rows {
        let hs = fmt_num(h);
        assert!(compose.contains(&format!("Modifier.height({hs}.dp)")), "{name} Compose missing row height {hs}.dp: {compose}");
    }
    for (p, k) in placed.iter().zip(&kids) {
        let cell_h_first = rows[p.row];
        let (ah, av) = effective_align(k);
        let compose_align = match (ah, av) {
            (GridChildAlign::Center, GridChildAlign::Start) => "Alignment.TopCenter",
            (GridChildAlign::End, GridChildAlign::Start) => "Alignment.TopEnd",
            (GridChildAlign::Start, GridChildAlign::Center) => "Alignment.CenterStart",
            (GridChildAlign::Center, GridChildAlign::Center) => "Alignment.Center",
            (GridChildAlign::End, GridChildAlign::Center) => "Alignment.CenterEnd",
            (GridChildAlign::Start, GridChildAlign::End) => "Alignment.BottomStart",
            (GridChildAlign::Center, GridChildAlign::End) => "Alignment.BottomCenter",
            (GridChildAlign::End, GridChildAlign::End) => "Alignment.BottomEnd",
            _ => "Alignment.TopStart",
        };
        let box_needle = format!("Box(Modifier.height({}.dp), contentAlignment = {compose_align})", fmt_num(cell_h_first));
        assert!(compose.contains(&box_needle), "{name} Compose missing cell box {box_needle}: {compose}");
        if p.col_span > 1 {
            assert!(compose.contains(&format!("GridItemSpan({})", p.col_span)), "{name} Compose missing GridItemSpan({}): {compose}", p.col_span);
        }
    }
    let n = scene.doc.get(scene.grid).unwrap();
    let al = n.kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
    let compose_h = if al.counter_sizing == Sizing::Fixed {
        n.size.y
    } else {
        rows.iter().sum::<f32>() + row_gap * (rows.len().saturating_sub(1) as f32) + padding[0] + padding[2]
    };
    let chs = fmt_num(compose_h);
    assert!(
        compose.contains(&format!("height = {chs}.dp")) || compose.contains(&format!(".height({chs}.dp)")) || compose.contains(&format!(".size({chs}.dp)")),
        "{name} Compose missing explicit container height {chs}.dp: {compose}"
    );
}

fn assert_uikit_scene(scene: &NativeGridScene) {
    let name = scene.name;
    let (widths, rows, placed, col_gap, _, _) = solver_expectations(scene);
    let kids = solver_kids(scene);
    let uikit = uikit_for(&scene.doc, scene.grid);
    assert_eq!(uikit.matches(".axis = .horizontal").count(), rows.len(), "{name} UIKit horizontal row stack count != {}: {uikit}", rows.len());
    let mut cell_counter = 0usize;
    let mut hole_counter = 0usize;
    for (r, &row_h) in rows.iter().enumerate() {
        let mut c = 0usize;
        while c < widths.len() {
            if let Some((p, k)) = placed.iter().zip(&kids).find(|(p, _)| p.row == r && p.col == c) {
                cell_counter += 1;
                let cell_name = if cell_counter == 1 { "cell".to_string() } else { format!("cell{cell_counter}") };
                let cell_w: f32 = (0..p.col_span).map(|dc| widths[p.col + dc]).sum::<f32>() + col_gap * (p.col_span.saturating_sub(1) as f32);
                let w_con = format!("{cell_name}.widthAnchor.constraint(equalToConstant: {})", fmt_num(cell_w));
                let h_con = format!("{cell_name}.heightAnchor.constraint(equalToConstant: {})", fmt_num(row_h));
                assert!(uikit.contains(&w_con), "{name} UIKit missing {w_con}: {uikit}");
                assert!(uikit.contains(&h_con), "{name} UIKit missing {h_con}: {uikit}");
                let (ah, av) = effective_align(k);
                let h_anchor = match ah {
                    GridChildAlign::Center => format!(".centerXAnchor.constraint(equalTo: {cell_name}.centerXAnchor)"),
                    GridChildAlign::End => format!(".trailingAnchor.constraint(equalTo: {cell_name}.trailingAnchor)"),
                    _ => format!(".leadingAnchor.constraint(equalTo: {cell_name}.leadingAnchor)"),
                };
                let v_anchor = match av {
                    GridChildAlign::Center => format!(".centerYAnchor.constraint(equalTo: {cell_name}.centerYAnchor)"),
                    GridChildAlign::End => format!(".bottomAnchor.constraint(equalTo: {cell_name}.bottomAnchor)"),
                    _ => format!(".topAnchor.constraint(equalTo: {cell_name}.topAnchor)"),
                };
                assert!(uikit.contains(&h_anchor), "{name} UIKit missing {h_anchor}: {uikit}");
                assert!(uikit.contains(&v_anchor), "{name} UIKit missing {v_anchor}: {uikit}");
                c += p.col_span.max(1);
            } else {
                hole_counter += 1;
                let hole_name = if hole_counter == 1 { "hole".to_string() } else { format!("hole{hole_counter}") };
                let w_con = format!("{hole_name}.widthAnchor.constraint(equalToConstant: {})", fmt_num(widths[c]));
                let h_con = format!("{hole_name}.heightAnchor.constraint(equalToConstant: {})", fmt_num(row_h));
                assert!(uikit.contains(&w_con), "{name} UIKit missing hole {w_con}: {uikit}");
                assert!(uikit.contains(&h_con), "{name} UIKit missing hole {h_con}: {uikit}");
                c += 1;
            }
        }
    }
}

fn assert_android_scene(scene: &NativeGridScene) {
    let name = scene.name;
    let (widths, rows, placed, col_gap, row_gap, _) = solver_expectations(scene);
    let kids = solver_kids(scene);
    let xml = android_xml_for(&scene.doc, scene.grid);
    assert!(xml.contains("<GridLayout"), "{name} Android missing <GridLayout: {xml}");
    assert!(xml.contains(&format!("android:columnCount=\"{}\"", widths.len())), "{name} Android missing columnCount={}: {xml}", widths.len());
    assert!(xml.contains(&format!("android:rowCount=\"{}\"", rows.len())), "{name} Android missing rowCount={}: {xml}", rows.len());
    for (c, &w) in widths.iter().enumerate() {
        let covered = placed.iter().any(|p| p.col == c && p.col_span == 1);
        if !covered {
            let space_col = format!(
                "android:layout_width=\"{}dp\"\n        android:layout_height=\"0dp\"\n        android:layout_row=\"0\"\n        android:layout_column=\"{c}\"",
                fmt_num(w)
            );
            assert!(xml.contains(&space_col), "{name} Android missing column <Space> for col {c}: {xml}");
        }
    }
    for (r, &h) in rows.iter().enumerate() {
        let covered = placed.iter().any(|p| p.row == r && p.row_span == 1);
        if !covered {
            let space_row = format!(
                "android:layout_width=\"0dp\"\n        android:layout_height=\"{}dp\"\n        android:layout_row=\"{r}\"\n        android:layout_column=\"0\"",
                fmt_num(h)
            );
            assert!(xml.contains(&space_row), "{name} Android missing row <Space> for row {r}: {xml}");
        }
    }
    for (p, k) in placed.iter().zip(&kids) {
        let cell_w: f32 = (0..p.col_span).map(|dc| widths[p.col + dc]).sum::<f32>() + col_gap * (p.col_span.saturating_sub(1) as f32);
        let cell_h_span: f32 = (0..p.row_span).map(|dr| rows[p.row + dr]).sum::<f32>() + row_gap * (p.row_span.saturating_sub(1) as f32);
        let mut cell_attrs = vec![
            format!("android:layout_width=\"{}dp\"", fmt_num(cell_w)),
            format!("android:layout_height=\"{}dp\"", fmt_num(cell_h_span)),
            format!("android:layout_row=\"{}\"", p.row),
            format!("android:layout_column=\"{}\"", p.col),
        ];
        if p.row_span > 1 {
            cell_attrs.push(format!("android:layout_rowSpan=\"{}\"", p.row_span));
        }
        if p.col_span > 1 {
            cell_attrs.push(format!("android:layout_columnSpan=\"{}\"", p.col_span));
        }
        if p.col > 0 && col_gap > 0.0 {
            cell_attrs.push(format!("android:layout_marginStart=\"{}dp\"", fmt_num(col_gap)));
        }
        if p.row > 0 && row_gap > 0.0 {
            cell_attrs.push(format!("android:layout_marginTop=\"{}dp\"", fmt_num(row_gap)));
        }
        let expected_tag = format!("<FrameLayout\n        {}>", cell_attrs.join("\n        "));
        assert!(xml.contains(&expected_tag), "{name} Android missing cell tag:\n{expected_tag}\nin:\n{xml}");

        let (ah, av) = effective_align(k);
        let gravity = match (ah, av) {
            (GridChildAlign::Center, GridChildAlign::Start) => Some("center_horizontal"),
            (GridChildAlign::End, GridChildAlign::Start) => Some("end"),
            (GridChildAlign::Start, GridChildAlign::Center) => Some("center_vertical"),
            (GridChildAlign::Center, GridChildAlign::Center) => Some("center"),
            (GridChildAlign::End, GridChildAlign::Center) => Some("center_vertical|end"),
            (GridChildAlign::Start, GridChildAlign::End) => Some("bottom"),
            (GridChildAlign::Center, GridChildAlign::End) => Some("bottom|center_horizontal"),
            (GridChildAlign::End, GridChildAlign::End) => Some("bottom|end"),
            _ => None,
        };
        if let Some(g) = gravity {
            assert!(xml.contains(&format!("android:layout_gravity=\"{g}\"")), "{name} Android missing layout_gravity={g}: {xml}");
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
    let grid_pos = compose.find("LazyVerticalGrid(").unwrap();
    let badge_pos = compose.find(".offset(x = 15.dp, y = 25.dp)").unwrap();
    assert!(
        compose[..grid_pos].contains("Box(") && compose[grid_pos..badge_pos].contains("\n    }\n    Box("),
        "Compose should wrap LazyVerticalGrid in an outer Box and emit absolute child after LazyVerticalGrid closes: {compose}"
    );

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
    assert_eq!(xml.matches("<FrameLayout").count(), 3, "Android XML should wrap GridLayout in 1 outer FrameLayout + 2 cell FrameLayouts: {xml}");
    let grid_close = xml.find("</GridLayout>").unwrap();
    let badge_margin = xml.find("android:layout_marginStart=\"15dp\"").unwrap();
    assert!(badge_margin > grid_close, "Android XML should emit absolute child after </GridLayout> inside outer FrameLayout: {xml}");
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

#[test]
fn b1_overlap_same_cell_keeps_all_children_and_solver_offsets() {
    let al = AutoLayout {
        grid_columns: 2,
        grid_rows: 1,
        item_spacing: 10.0,
        counter_spacing: 10.0,
        padding: [8.0, 8.0, 8.0, 8.0],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    let (mut doc, grid) = new_grid(al, Vec2::new(226.0, 76.0));
    let primary = add_named_item(&mut doc, grid, "PrimaryTile", 100.0, 60.0, Sizing::Fixed, Sizing::Fixed);
    {
        let p = doc.get_mut(primary).unwrap();
        p.layout_child.grid_column = Some(0);
        p.layout_child.grid_row = Some(0);
    }
    let overlap = add_named_item(&mut doc, grid, "OverlapBadge", 40.0, 20.0, Sizing::Fixed, Sizing::Fixed);
    {
        let o = doc.get_mut(overlap).unwrap();
        o.layout_child.grid_column = Some(0);
        o.layout_child.grid_row = Some(0);
        o.layout_child.grid_align_h = GridChildAlign::End;
        o.layout_child.grid_align_v = GridChildAlign::Center;
    }
    let right = add_named_item(&mut doc, grid, "RightTile", 100.0, 60.0, Sizing::Fixed, Sizing::Fixed);
    {
        let r = doc.get_mut(right).unwrap();
        r.layout_child.grid_column = Some(1);
        r.layout_child.grid_row = Some(0);
    }
    doc.relayout_subtree(grid);

    let swift = swiftui_for(&doc, grid);
    assert!(swift.contains("// \"OverlapBadge\": overlapping grid cells stacked in the cell of \"PrimaryTile\""), "SwiftUI missing overlap comment:\n{swift}");
    assert!(swift.contains("ZStack(alignment: .topLeading)"), "SwiftUI missing ZStack for overlapping cell:\n{swift}");
    assert!(
        swift.contains(".frame(width: 40, height: 20)") && swift.contains(".offset(x: 60, y: 20)"),
        "SwiftUI missing OverlapBadge or solver offset (60, 20):\n{swift}"
    );

    let compose = compose_for(&doc, grid);
    assert!(
        compose.contains("// \"OverlapBadge\": overlapping grid cells stacked in the cell of \"PrimaryTile\""),
        "Compose missing overlap comment:\n{compose}"
    );
    assert_eq!(compose.matches("item {").count(), 2, "Compose should emit 2 grid items (shared cell + right cell):\n{compose}");
    assert!(
        compose.contains(".size(width = 40.dp, height = 20.dp)") && compose.contains(".offset(x = 60.dp, y = 20.dp)"),
        "Compose missing OverlapBadge or solver offset (60.dp, 20.dp):\n{compose}"
    );

    let uikit = uikit_for(&doc, grid);
    assert!(uikit.contains("// \"OverlapBadge\": overlapping grid cells stacked in the cell of \"PrimaryTile\""), "UIKit missing overlap comment:\n{uikit}");
    assert!(
        uikit.contains("cell.addSubview(primaryTile)") && uikit.contains("cell.addSubview(overlapBadge)"),
        "UIKit should add both children to cell:\n{uikit}"
    );
    assert!(
        uikit.contains("overlapBadge.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 60)")
            && uikit.contains("overlapBadge.topAnchor.constraint(equalTo: cell.topAnchor, constant: 20)"),
        "UIKit missing solver offset constraints (60, 20) on cell:\n{uikit}"
    );

    let xml = android_xml_for(&doc, grid);
    assert_eq!(xml.matches("<FrameLayout").count(), 3, "Android XML should keep all 3 flow children:\n{xml}");
}

#[test]
fn b1_overlap_inside_col_span_keeps_all_children_and_solver_offsets() {
    let al = AutoLayout {
        grid_columns: 3,
        grid_rows: 1,
        item_spacing: 10.0,
        counter_spacing: 10.0,
        padding: [10.0, 10.0, 10.0, 10.0],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    let (mut doc, grid) = new_grid(al, Vec2::new(340.0, 80.0));
    let span_banner = add_named_item(&mut doc, grid, "SpanBanner", 210.0, 60.0, Sizing::Fixed, Sizing::Fixed);
    {
        let s = doc.get_mut(span_banner).unwrap();
        s.layout_child.grid_column = Some(0);
        s.layout_child.grid_row = Some(0);
        s.layout_child.col_span = 2;
    }
    let col1_chip = add_named_item(&mut doc, grid, "Col1Chip", 50.0, 24.0, Sizing::Fixed, Sizing::Fixed);
    {
        let c = doc.get_mut(col1_chip).unwrap();
        c.layout_child.grid_column = Some(1);
        c.layout_child.grid_row = Some(0);
        c.layout_child.grid_align_h = GridChildAlign::Start;
        c.layout_child.grid_align_v = GridChildAlign::Center;
    }
    let tail_card = add_named_item(&mut doc, grid, "TailCard", 100.0, 60.0, Sizing::Fixed, Sizing::Fixed);
    {
        let t = doc.get_mut(tail_card).unwrap();
        t.layout_child.grid_column = Some(2);
        t.layout_child.grid_row = Some(0);
    }
    doc.relayout_subtree(grid);

    let swift = swiftui_for(&doc, grid);
    assert!(
        swift.contains("// \"Col1Chip\": overlapping grid cells stacked in the cell of \"SpanBanner\""),
        "SwiftUI missing col_span overlap comment:\n{swift}"
    );
    assert!(
        swift.contains(".frame(width: 50, height: 24)") && swift.contains(".offset(x: 110, y: 18)"),
        "SwiftUI missing Col1Chip or solver offset (110, 18) inside SpanBanner cell:\n{swift}"
    );

    let compose = compose_for(&doc, grid);
    assert!(
        compose.contains("// \"Col1Chip\": overlapping grid cells stacked in the cell of \"SpanBanner\""),
        "Compose missing col_span overlap comment:\n{compose}"
    );
    assert!(
        compose.contains(".size(width = 50.dp, height = 24.dp)") && compose.contains(".offset(x = 110.dp, y = 18.dp)"),
        "Compose missing Col1Chip or solver offset (110.dp, 18.dp):\n{compose}"
    );

    let uikit = uikit_for(&doc, grid);
    assert!(
        uikit.contains("// \"Col1Chip\": overlapping grid cells stacked in the cell of \"SpanBanner\""),
        "UIKit missing col_span overlap comment:\n{uikit}"
    );
    assert!(
        uikit.contains("col1Chip.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 110)")
            && uikit.contains("col1Chip.topAnchor.constraint(equalTo: cell.topAnchor, constant: 18)"),
        "UIKit missing Col1Chip solver offset (110, 18) on cell:\n{uikit}"
    );

    let xml = android_xml_for(&doc, grid);
    assert_eq!(xml.matches("<FrameLayout").count(), 3, "Android XML should keep all 3 flow children:\n{xml}");
}

#[test]
fn b2_compose_out_of_flow_children_wrap_grid_in_outer_box() {
    let al = AutoLayout {
        grid_columns: 2,
        grid_rows: 1,
        item_spacing: 8.0,
        counter_spacing: 8.0,
        padding: [12.0, 12.0, 12.0, 12.0],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    let (mut doc, grid) = new_grid(al, Vec2::new(200.0, 100.0));
    add_named_item(&mut doc, grid, "Cell1", 84.0, 76.0, Sizing::Fixed, Sizing::Fixed);
    add_named_item(&mut doc, grid, "Cell2", 84.0, 76.0, Sizing::Fixed, Sizing::Fixed);
    let ribbon = add_named_item(&mut doc, grid, "CornerRibbon", 30.0, 16.0, Sizing::Fixed, Sizing::Fixed);
    {
        let r = doc.get_mut(ribbon).unwrap();
        r.layout_child.absolute = true;
        r.transform.tx = 160.0;
        r.transform.ty = 4.0;
    }
    doc.relayout_subtree(grid);

    let compose = compose_for(&doc, grid);
    assert!(
        compose.starts_with("Box(modifier = Modifier.size(width = 200.dp, height = 100.dp)) {\n    LazyVerticalGrid("),
        "Compose should wrap LazyVerticalGrid in an outer Box with frame size:\n{compose}"
    );
    assert!(
        compose.contains("modifier = Modifier\n            .fillMaxSize()\n            .padding(12.dp)"),
        "LazyVerticalGrid inside outer Box should use fillMaxSize + padding:\n{compose}"
    );
    let grid_lambda_open = compose.find("userScrollEnabled = false\n    ) {\n").expect("missing LazyVerticalGrid lambda open");
    let first_item = compose.find("        item {").expect("missing first item");
    assert_eq!(grid_lambda_open + "userScrollEnabled = false\n    ) {\n".len(), first_item, "no composable call may sit before first item(:\n{compose}");
    let grid_end = compose.find("    }\n    Box(").expect("CornerRibbon must be emitted after LazyVerticalGrid closes inside outer Box");
    let offset_pos = compose.find(".offset(x = 160.dp, y = 4.dp)").expect("missing CornerRibbon offset");
    assert!(offset_pos > grid_end, "CornerRibbon offset must appear after LazyVerticalGrid closes:\n{compose}");
}

#[test]
fn b3_grid_slot_keeps_slot_parameter() {
    let mut doc = Document::new("grid-slot");
    let page = doc.add_page("Page 1");
    let comp_id = doc.alloc_id();
    let mut comp = Node::new(comp_id, NodeKind::Component(FrameProps::default()));
    comp.name = "CardWithGridSlot".into();
    comp.size = Vec2::new(240.0, 120.0);
    if let NodeKind::Component(f) = &mut comp.kind {
        f.auto_layout = Some(AutoLayout {
            direction: LayoutDirection::Vertical,
            item_spacing: 8.0,
            padding: [12.0; 4],
            primary_sizing: Sizing::Fixed,
            counter_sizing: Sizing::Fixed,
            ..Default::default()
        });
    }
    comp.component.props.push(ComponentPropDef { name: "Tiles#1:0".into(), kind: PropKind::Slot, default: PropValue::Slot(vec![]) });
    let comp_id = doc.insert(page, comp).unwrap();

    let slot_id = doc.alloc_id();
    let mut slot = Node::frame(slot_id, Rect::new(0.0, 0.0, 216.0, 80.0));
    slot.name = "Tiles".into();
    slot.component.is_slot = true;
    slot.component.bindings.insert(OverrideField::SlotContent, "Tiles#1:0".into());
    if let NodeKind::Frame(f) = &mut slot.kind {
        f.auto_layout = Some(AutoLayout {
            direction: LayoutDirection::Grid,
            grid_columns: 2,
            item_spacing: 8.0,
            counter_spacing: 8.0,
            primary_sizing: Sizing::Fixed,
            counter_sizing: Sizing::Hug,
            ..Default::default()
        });
    }
    let slot_id = doc.insert(comp_id, slot).unwrap();
    let def_t = doc.alloc_id();
    let mut tnode = Node::text(def_t, Rect::new(0.0, 0.0, 80.0, 20.0), "DefaultTile", TextStyle::default());
    tnode.name = "DefaultTile".into();
    doc.insert(slot_id, tnode).unwrap();

    create_instance(&mut doc, comp_id, page, Vec2::new(300.0, 0.0)).apply(&mut doc).unwrap();
    let inst = instances_of(&doc, comp_id)[0];
    let copy_slot = doc.get(inst).unwrap().children[0];
    for c in doc.get(copy_slot).unwrap().children.clone() {
        doc.remove(c).unwrap();
    }
    let custom_id = doc.alloc_id();
    let mut custom = Node::text(custom_id, Rect::new(0.0, 0.0, 80.0, 20.0), "PassedTile", TextStyle::default());
    custom.name = "PassedTile".into();
    custom.component.is_slot_content = true;
    let custom_id = doc.insert(copy_slot, custom).unwrap();
    doc.get_mut(inst).unwrap().component.values.insert("Tiles#1:0".into(), PropValue::Slot(vec![custom_id]));

    let sw_def = swiftui_for(&doc, comp_id);
    assert!(sw_def.contains("if let tiles {") && sw_def.contains("Text(\"DefaultTile\")"), "SwiftUI component definition lost grid slot parameter:\n{sw_def}");
    let sw_inst = swiftui_for(&doc, inst);
    assert!(
        sw_inst.contains("CardWithGridSlot {") && sw_inst.contains("Text(\"PassedTile\")"),
        "SwiftUI instance failed to pass grid slot content:\n{sw_inst}"
    );

    let kt_def = compose_for(&doc, comp_id);
    assert!(
        kt_def.contains("tiles: @Composable () -> Unit = {") && kt_def.contains("tiles()"),
        "Compose component definition lost grid slot parameter:\n{kt_def}"
    );
    let kt_inst = compose_for(&doc, inst);
    assert!(
        kt_inst.contains("CardWithGridSlot(") && kt_inst.contains(") {\n    Text(\n        text = \"PassedTile\""),
        "Compose instance failed to pass grid slot content:\n{kt_inst}"
    );
}

#[test]
fn m1_compose_grid_component_instance_keeps_frame_height() {
    let mut doc = Document::new("grid-instance-height");
    let page = doc.add_page("Page 1");
    let comp_id = doc.alloc_id();
    let mut comp = Node::new(comp_id, NodeKind::Component(FrameProps::default()));
    comp.name = "TileGrid".into();
    comp.size = Vec2::new(210.0, 50.0);
    if let NodeKind::Component(f) = &mut comp.kind {
        f.auto_layout = Some(AutoLayout {
            direction: LayoutDirection::Grid,
            grid_columns: 2,
            item_spacing: 10.0,
            counter_spacing: 10.0,
            primary_sizing: Sizing::Fixed,
            counter_sizing: Sizing::Fixed,
            ..Default::default()
        });
    }
    let comp_id = doc.insert(page, comp).unwrap();
    for i in 0..2 {
        add_named_item(&mut doc, comp_id, &format!("Cell{i}"), 100.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    }
    create_instance(&mut doc, comp_id, page, Vec2::new(0.0, 80.0)).apply(&mut doc).unwrap();
    let inst = instances_of(&doc, comp_id)[0];

    let kt = compose_for(&doc, inst);
    assert!(
        kt.contains("TileGrid(modifier = Modifier.size(width = 210.dp, height = 50.dp))"),
        "Compose grid component instance should keep its 210x50 frame height 50.dp, not recomputed grid height 30.dp:\n{kt}"
    );
}

#[test]
fn m2_android_span_only_column_and_row_emit_space() {
    // 1. Span-only column: col 1 is covered only by a col_span=2 child in row 0, and (row 1, col 1) is empty.
    let al_col = AutoLayout {
        grid_columns: 3,
        grid_rows: 2,
        item_spacing: 10.0,
        counter_spacing: 10.0,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    let (mut doc1, grid1) = new_grid(al_col, Vec2::new(320.0, 90.0));
    let wide = add_named_item(&mut doc1, grid1, "WideTop", 210.0, 40.0, Sizing::Fixed, Sizing::Fixed);
    {
        let w = doc1.get_mut(wide).unwrap();
        w.layout_child.grid_column = Some(0);
        w.layout_child.grid_row = Some(0);
        w.layout_child.col_span = 2;
    }
    let top_right = add_named_item(&mut doc1, grid1, "TopRight", 100.0, 40.0, Sizing::Fixed, Sizing::Fixed);
    {
        let t = doc1.get_mut(top_right).unwrap();
        t.layout_child.grid_column = Some(2);
        t.layout_child.grid_row = Some(0);
    }
    let bot_left = add_named_item(&mut doc1, grid1, "BotLeft", 100.0, 40.0, Sizing::Fixed, Sizing::Fixed);
    {
        let b = doc1.get_mut(bot_left).unwrap();
        b.layout_child.grid_column = Some(0);
        b.layout_child.grid_row = Some(1);
    }
    let bot_right = add_named_item(&mut doc1, grid1, "BotRight", 100.0, 40.0, Sizing::Fixed, Sizing::Fixed);
    {
        let b = doc1.get_mut(bot_right).unwrap();
        b.layout_child.grid_column = Some(2);
        b.layout_child.grid_row = Some(1);
    }
    doc1.relayout_subtree(grid1);
    let xml1 = android_xml_for(&doc1, grid1);
    assert!(
        xml1.contains("<Space\n        android:layout_width=\"100dp\"\n        android:layout_height=\"0dp\"\n        android:layout_row=\"0\"\n        android:layout_column=\"1\"\n        android:layout_marginStart=\"10dp\" />"),
        "Android XML missing <Space> for span-only column 1:\n{xml1}"
    );

    // 2. Span-only row: row 1 is covered only by a row_span=2 child at (col 0, row 0).
    let mut al_row = AutoLayout {
        grid_columns: 2,
        grid_rows: 2,
        item_spacing: 10.0,
        counter_spacing: 10.0,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Hug,
        ..Default::default()
    };
    al_row.grid_row_tracks[0] = GridTrack::Fixed(40.0);
    al_row.grid_row_tracks[1] = GridTrack::Fixed(50.0);
    let (mut doc2, grid2) = new_grid(al_row, Vec2::new(210.0, 100.0));
    let tall = add_named_item(&mut doc2, grid2, "TallLeft", 100.0, 100.0, Sizing::Fixed, Sizing::Fixed);
    {
        let t = doc2.get_mut(tall).unwrap();
        t.layout_child.grid_column = Some(0);
        t.layout_child.grid_row = Some(0);
        t.layout_child.row_span = 2;
    }
    let short = add_named_item(&mut doc2, grid2, "ShortRight", 100.0, 40.0, Sizing::Fixed, Sizing::Fixed);
    {
        let s = doc2.get_mut(short).unwrap();
        s.layout_child.grid_column = Some(1);
        s.layout_child.grid_row = Some(0);
    }
    doc2.relayout_subtree(grid2);
    let xml2 = android_xml_for(&doc2, grid2);
    assert!(
        xml2.contains("<Space\n        android:layout_width=\"0dp\"\n        android:layout_height=\"50dp\"\n        android:layout_row=\"1\"\n        android:layout_column=\"0\"\n        android:layout_marginTop=\"10dp\" />"),
        "Android XML missing <Space> for span-only row 1:\n{xml2}"
    );
}

#[test]
fn m3_android_out_of_flow_and_variable_hidden_children_wrap_grid_in_frame_layout() {
    let al = AutoLayout {
        grid_columns: 2,
        grid_rows: 1,
        item_spacing: 10.0,
        counter_spacing: 10.0,
        padding: [8.0, 8.0, 8.0, 8.0],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Hug,
        ..Default::default()
    };
    let (mut doc, grid) = new_grid(al, Vec2::new(226.0, 76.0));
    doc.variable_collections.insert(
        "c".into(),
        VariableCollection {
            id: "c".into(),
            name: "Flags".into(),
            modes: vec![VariableMode { id: "m".into(), name: "Default".into() }],
            default_mode: "m".into(),
            variables: vec!["flag_banner".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    doc.variables.insert(
        "flag_banner".into(),
        Variable {
            id: "flag_banner".into(),
            name: "Flags/ShowBanner".into(),
            collection: "c".into(),
            kind: VariableKind::Boolean,
            values: [("m".to_string(), VariableValue::Boolean(false))].into_iter().collect(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,
            code_syntax: [(CodeSyntaxPlatform::Android, "Tokens.showBanner".to_string())].into_iter().collect(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        },
    );

    add_named_item(&mut doc, grid, "FlowA", 100.0, 60.0, Sizing::Fixed, Sizing::Fixed);
    add_named_item(&mut doc, grid, "FlowB", 100.0, 60.0, Sizing::Fixed, Sizing::Fixed);
    let abs_badge = add_named_item(&mut doc, grid, "AbsBadge", 28.0, 16.0, Sizing::Fixed, Sizing::Fixed);
    {
        let a = doc.get_mut(abs_badge).unwrap();
        a.layout_child.absolute = true;
        a.transform.tx = 12.0;
        a.transform.ty = 18.0;
    }
    let cond_banner = add_named_item(&mut doc, grid, "ConditionalBanner", 90.0, 24.0, Sizing::Fixed, Sizing::Fixed);
    {
        let c = doc.get_mut(cond_banner).unwrap();
        c.variables.bindings.insert("visible".into(), "flag_banner".into());
    }
    doc.relayout_subtree(grid);

    let xml = android_xml_for(&doc, grid);
    assert!(xml.contains("-->\n<FrameLayout\n"), "Android XML with out-of-flow/hidden children should start with outer <FrameLayout:\n{xml}");
    assert!(xml.contains("    <GridLayout\n"), "Android XML should nest <GridLayout> inside outer <FrameLayout>:\n{xml}");
    let grid_close = xml.find("    </GridLayout>").expect("missing </GridLayout>");
    let abs_pos = xml.find("android:layout_marginStart=\"12dp\"").expect("missing AbsBadge marginStart");
    let vis_pos = xml.find("android:layout_width=\"90dp\"\n        android:layout_height=\"24dp\"").expect("missing variable-hidden ConditionalBanner view");
    assert!(abs_pos > grid_close && vis_pos > grid_close, "Out-of-flow and variable-hidden children must be emitted after </GridLayout>:\n{xml}");
}

#[test]
fn m4_compose_grid_height_fixed_vs_hug_and_fill() {
    let mut al = AutoLayout {
        grid_columns: 2,
        grid_rows: 2,
        item_spacing: 10.0,
        counter_spacing: 10.0,
        padding: [8.0, 8.0, 8.0, 8.0],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    al.grid_row_tracks[0] = GridTrack::Fixed(40.0);
    al.grid_row_tracks[1] = GridTrack::Fixed(40.0);
    let (mut doc, grid) = new_grid(al, Vec2::new(220.0, 200.0));
    for i in 0..4 {
        add_named_item(&mut doc, grid, &format!("Cell{i}"), 90.0, 40.0, Sizing::Fixed, Sizing::Fixed);
    }
    doc.relayout_subtree(grid);

    // 1. counter_sizing == Fixed -> uses n.size.y (200.dp), not content height (40+40+10+16 = 106.dp).
    let kt_fixed = compose_for(&doc, grid);
    assert!(kt_fixed.contains(".size(width = 220.dp, height = 200.dp)"), "Fixed-height grid in Compose should use frame height 200.dp:\n{kt_fixed}");

    // 2. counter_sizing == Hug -> uses content height (106.dp) even if n.size.y is still 200.
    if let NodeKind::Frame(f) = &mut doc.get_mut(grid).unwrap().kind {
        f.auto_layout.as_mut().unwrap().counter_sizing = Sizing::Hug;
    }
    let kt_hug = compose_for(&doc, grid);
    assert!(kt_hug.contains(".size(width = 220.dp, height = 106.dp)"), "Hug-height grid in Compose should use content height 106.dp:\n{kt_hug}");
    assert!(!kt_hug.contains("fillMaxHeight"), "Compose LazyVerticalGrid must never emit fillMaxHeight:\n{kt_hug}");
}

#[test]
fn m5_bound_counter_axis_spacing_and_non_start_cell_alignments() {
    let al = AutoLayout {
        grid_columns: 2,
        grid_rows: 2,
        item_spacing: 12.0,
        counter_spacing: 20.0,
        padding: [8.0, 8.0, 8.0, 8.0],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Hug,
        ..Default::default()
    };
    let (mut doc, grid) = new_grid(al, Vec2::new(228.0, 140.0));
    doc.variable_collections.insert(
        "c".into(),
        VariableCollection {
            id: "c".into(),
            name: "Spacing".into(),
            modes: vec![VariableMode { id: "m".into(), name: "Default".into() }],
            default_mode: "m".into(),
            variables: vec!["row_gap_tok".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    doc.variables.insert(
        "row_gap_tok".into(),
        Variable {
            id: "row_gap_tok".into(),
            name: "Spacing/RowGap".into(),
            collection: "c".into(),
            kind: VariableKind::Number,
            values: [("m".to_string(), VariableValue::Number(20.0))].into_iter().collect(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,
            code_syntax: [(CodeSyntaxPlatform::Ios, "Tokens.spacingRowGap".to_string()), (CodeSyntaxPlatform::Android, "Tokens.spacingRowGap".to_string())]
                .into_iter()
                .collect(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        },
    );
    doc.get_mut(grid).unwrap().variables.bindings.insert("counterAxisSpacing".into(), "row_gap_tok".into());

    let c0 = add_named_item(&mut doc, grid, "CenterBottomItem", 60.0, 24.0, Sizing::Fixed, Sizing::Fixed);
    {
        let n = doc.get_mut(c0).unwrap();
        n.layout_child.grid_align_h = GridChildAlign::Center;
        n.layout_child.grid_align_v = GridChildAlign::End;
    }
    let c1 = add_named_item(&mut doc, grid, "EndCenterItem", 60.0, 24.0, Sizing::Fixed, Sizing::Fixed);
    {
        let n = doc.get_mut(c1).unwrap();
        n.layout_child.grid_align_h = GridChildAlign::End;
        n.layout_child.grid_align_v = GridChildAlign::Center;
    }
    add_named_item(&mut doc, grid, "Row2A", 100.0, 40.0, Sizing::Fixed, Sizing::Fixed);
    add_named_item(&mut doc, grid, "Row2B", 100.0, 40.0, Sizing::Fixed, Sizing::Fixed);
    doc.relayout_subtree(grid);

    let scene = NativeGridScene { name: "m5_counter_gap_and_align", doc, grid, items: vec![c0, c1] };
    let swift = swiftui_for(&scene.doc, scene.grid);
    assert!(swift.contains("verticalSpacing: Tokens.spacingRowGap"), "SwiftUI missing bound counterAxisSpacing token:\n{swift}");
    assert!(swift.contains("alignment: .bottom)") && swift.contains("alignment: .trailing)"), "SwiftUI missing cell alignments:\n{swift}");

    let compose = compose_for(&scene.doc, scene.grid);
    assert!(compose.contains("verticalArrangement = Arrangement.spacedBy(Tokens.spacingRowGap)"), "Compose missing bound counterAxisSpacing token:\n{compose}");
    assert!(
        compose.contains("contentAlignment = Alignment.BottomCenter") && compose.contains("contentAlignment = Alignment.CenterEnd"),
        "Compose missing cell alignments:\n{compose}"
    );

    let uikit = uikit_for(&scene.doc, scene.grid);
    assert!(uikit.contains("gridFrame.spacing = Tokens.spacingRowGap"), "UIKit missing bound counterAxisSpacing token:\n{uikit}");
    assert!(
        uikit.contains("centerBottomItem.centerXAnchor.constraint(equalTo: cell.centerXAnchor)")
            && uikit.contains("centerBottomItem.bottomAnchor.constraint(equalTo: cell.bottomAnchor)")
            && uikit.contains("endCenterItem.trailingAnchor.constraint(equalTo: cell2.trailingAnchor)")
            && uikit.contains("endCenterItem.centerYAnchor.constraint(equalTo: cell2.centerYAnchor)"),
        "UIKit missing cell alignment anchors:\n{uikit}"
    );

    let xml = android_xml_for(&scene.doc, scene.grid);
    assert!(xml.contains("android:layout_marginTop=\"Tokens.spacingRowGap\""), "Android XML missing bound counterAxisSpacing token:\n{xml}");
    assert!(
        xml.contains("android:layout_gravity=\"bottom|center_horizontal\"") && xml.contains("android:layout_gravity=\"center_vertical|end\""),
        "Android XML missing cell layout_gravity:\n{xml}"
    );

    // Also verify counter_spacing == 0 falls back to item_spacing across multi-row grids in codegen (kills M1).
    let al_fallback = AutoLayout {
        grid_columns: 2,
        grid_rows: 2,
        item_spacing: 14.0,
        counter_spacing: 0.0,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Hug,
        ..Default::default()
    };
    let (mut doc_fb, grid_fb) = new_grid(al_fallback, Vec2::new(220.0, 100.0));
    for i in 0..4 {
        add_named_item(&mut doc_fb, grid_fb, &format!("R{i}"), 100.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    }
    doc_fb.relayout_subtree(grid_fb);
    let swift_fb = swiftui_for(&doc_fb, grid_fb);
    assert!(swift_fb.contains("verticalSpacing: 14"), "SwiftUI should fall back to item_spacing=14 when counter_spacing=0:\n{swift_fb}");
}

#[test]
fn d3_explicit_trailing_grid_rows_exported() {
    let mut al = AutoLayout {
        grid_columns: 2,
        grid_rows: 3,
        item_spacing: 10.0,
        counter_spacing: 10.0,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Hug,
        ..Default::default()
    };
    al.grid_row_tracks[0] = GridTrack::Fixed(30.0);
    al.grid_row_tracks[1] = GridTrack::Fixed(40.0);
    al.grid_row_tracks[2] = GridTrack::Fixed(50.0);
    let (mut doc, grid) = new_grid(al, Vec2::new(210.0, 140.0));
    add_named_item(&mut doc, grid, "OnlyChild", 100.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    doc.relayout_subtree(grid);

    let swift = swiftui_for(&doc, grid);
    assert_eq!(swift.matches("GridRow {").count(), 3, "SwiftUI should emit all 3 explicit grid rows:\n{swift}");
    let xml = android_xml_for(&doc, grid);
    assert!(xml.contains("android:rowCount=\"3\""), "Android XML should have rowCount=3:\n{xml}");
}
