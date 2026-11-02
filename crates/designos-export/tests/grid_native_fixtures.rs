//! Checks that SwiftUI, Compose, UIKit and Android XML export grids
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

fn solver_flow_ids(doc: &Document, grid: NodeId) -> Vec<NodeId> {
    let n = doc.get(grid).unwrap();
    n.children.iter().copied().filter(|id| doc.flow_child_input(*id, true).is_some()).collect()
}

fn solver_rects(doc: &Document, grid: NodeId) -> Vec<(String, Rect)> {
    let n = doc.get(grid).unwrap();
    let al = n.kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
    let al_s = al.with_stroke(&n.stroke);
    let own = SizeLimits::from(&n.layout_child);
    let flow: Vec<(String, ChildIn)> =
        n.children.iter().copied().filter_map(|id| doc.flow_child_input(id, true).map(|ci| (doc.get(id).unwrap().name.clone(), ci))).collect();
    let kids: Vec<ChildIn> = flow.iter().map(|(_, ci)| *ci).collect();
    let out = compute_grid(&al_s, n.size, own, &kids);
    flow.into_iter().zip(out.rects).map(|((name, _), r)| (name, r)).collect()
}

struct TestCellSlot {
    row: usize,
    flat_item_idx: usize,
    row_slot_idx: usize,
    sub_idx: Option<usize>,
}

fn find_child_slot(doc: &Document, grid: NodeId, child_name: &str) -> TestCellSlot {
    let n = doc.get(grid).unwrap();
    let al = n.kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
    let al_s = al.with_stroke(&n.stroke);
    let own = SizeLimits::from(&n.layout_child);
    let flow: Vec<(NodeId, ChildIn)> = n.children.iter().copied().filter_map(|id| doc.flow_child_input(id, true).map(|ci| (id, ci))).collect();
    let kids: Vec<ChildIn> = flow.iter().map(|(_, ci)| *ci).collect();
    let widths = grid_column_widths(&al_s, n.size, own, &kids);
    let rows = grid_row_heights(&al_s, n.size, own, &kids);
    let placed = place_children(&kids, widths.len());
    let children: Vec<(NodeId, Placed)> = flow.into_iter().zip(placed).map(|((id, _), p)| (id, p)).collect();
    let (target_id, _) = children.iter().find(|(id, _)| doc.get(*id).unwrap().name == child_name).unwrap_or_else(|| panic!("child {child_name:?} not found"));

    struct Group {
        host: (NodeId, Placed),
        stacked: Vec<NodeId>,
    }
    let cols = widths.len();
    let num_rows = rows.len();
    let mut flat_item_idx = 0usize;
    for r in 0..num_rows {
        let mut groups: Vec<Group> = Vec::new();
        for &(cid, p) in &children {
            if p.row != r || p.col >= cols {
                continue;
            }
            let c_start = p.col;
            let c_end = p.col + p.col_span.max(1);
            if let Some(g) = groups.iter_mut().find(|g| {
                let g_start = g.host.1.col;
                let g_end = g.host.1.col + g.host.1.col_span.max(1);
                c_start < g_end && g_start < c_end
            }) {
                g.stacked.push(cid);
            } else {
                groups.push(Group { host: (cid, p), stacked: Vec::new() });
            }
        }
        let mut c = 0usize;
        let mut row_slot_idx = 0usize;
        while c < cols {
            if let Some(g) = groups.iter().find(|g| g.host.1.col == c) {
                if g.host.0 == *target_id {
                    let sub_idx = if g.stacked.is_empty() { None } else { Some(0) };
                    return TestCellSlot { row: r, flat_item_idx, row_slot_idx, sub_idx };
                }
                if let Some(pos) = g.stacked.iter().position(|id| *id == *target_id) {
                    return TestCellSlot { row: r, flat_item_idx, row_slot_idx, sub_idx: Some(1 + pos) };
                }
                let span = g.host.1.col_span.max(1).min(cols - c);
                c += span;
            } else {
                c += 1;
            }
            row_slot_idx += 1;
            flat_item_idx += 1;
        }
    }
    panic!("slot for child {child_name:?} not placed in grid");
}

fn split_swiftui_blocks(lines: &[&str], block_indent: usize) -> Vec<String> {
    let mut blocks: Vec<Vec<&str>> = Vec::new();
    let mut pending: Vec<&str> = Vec::new();
    for &line in lines {
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        let indent = line.len() - trimmed.len();
        if indent == block_indent && trimmed.starts_with("//") {
            pending.push(line);
        } else if indent == block_indent && !trimmed.starts_with('.') && !trimmed.starts_with('}') {
            let mut cur = std::mem::take(&mut pending);
            cur.push(line);
            blocks.push(cur);
        } else if let Some(last) = blocks.last_mut() {
            last.append(&mut pending);
            last.push(line);
        }
    }
    blocks.into_iter().map(|b| b.join("\n")).collect()
}

fn swiftui_cell_for(doc: &Document, grid: NodeId, swift: &str, child_name: &str) -> String {
    let slot = find_child_slot(doc, grid, child_name);
    let all_lines: Vec<&str> = swift.lines().collect();
    let mut row_seen = 0usize;
    let mut row_start = None;
    let mut row_indent = 0usize;
    for (i, &line) in all_lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if trimmed == "GridRow {" {
            if row_seen == slot.row {
                row_start = Some(i);
                row_indent = line.len() - trimmed.len();
                break;
            }
            row_seen += 1;
        }
    }
    let start_i = row_start.unwrap_or_else(|| panic!("GridRow {} not found for {child_name:?}:\n{swift}", slot.row));
    let mut end_i = all_lines.len();
    for (i, &line) in all_lines.iter().enumerate().skip(start_i + 1) {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if indent == row_indent && trimmed == "}" {
            end_i = i;
            break;
        }
    }
    let cell_indent = row_indent + 4;
    let cell_blocks = split_swiftui_blocks(&all_lines[start_i + 1..end_i], cell_indent);
    let cell_block = cell_blocks
        .get(slot.row_slot_idx)
        .unwrap_or_else(|| panic!("SwiftUI cell slot {} in row {} not found for {child_name:?}:\n{swift}", slot.row_slot_idx, slot.row));
    let Some(sub_i) = slot.sub_idx else {
        return cell_block.clone();
    };
    let cell_lines: Vec<&str> = cell_block.lines().collect();
    let z_open = cell_lines
        .iter()
        .position(|l| l.trim_start().starts_with("ZStack("))
        .unwrap_or_else(|| panic!("expected ZStack in stacked cell for {child_name:?}:\n{cell_block}"));
    let z_close = cell_lines
        .iter()
        .enumerate()
        .skip(z_open + 1)
        .find(|(_, l)| {
            let t = l.trim_start();
            l.len() - t.len() == cell_indent && t == "}"
        })
        .map(|(i, _)| i)
        .unwrap_or(cell_lines.len());
    let sub_blocks = split_swiftui_blocks(&cell_lines[z_open + 1..z_close], cell_indent + 4);
    let sub = sub_blocks.get(sub_i).unwrap_or_else(|| panic!("SwiftUI ZStack subview {sub_i} not found for {child_name:?}:\n{cell_block}"));
    if sub_i == 0 {
        let tail = cell_lines[z_close..].join("\n");
        format!("{sub}\n{tail}")
    } else {
        sub.clone()
    }
}

fn compose_cell_for(doc: &Document, grid: NodeId, compose: &str, child_name: &str) -> String {
    let slot = find_child_slot(doc, grid, child_name);
    let all_lines: Vec<&str> = compose.lines().collect();
    let grid_line = all_lines.iter().position(|l| l.contains("LazyVerticalGrid(")).unwrap_or_else(|| panic!("LazyVerticalGrid not found:\n{compose}"));
    let grid_indent = all_lines[grid_line].len() - all_lines[grid_line].trim_start().len();
    let item_indent = grid_indent + 4;
    let mut items: Vec<Vec<&str>> = Vec::new();
    let mut cur_item: Option<Vec<&str>> = None;
    for &line in all_lines.iter().skip(grid_line + 1) {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if indent == grid_indent && trimmed == "}" && cur_item.is_none() {
            break;
        }
        if indent == item_indent && (trimmed.starts_with("item {") || trimmed.starts_with("item(")) {
            if trimmed.ends_with('}') {
                items.push(vec![line]);
            } else {
                cur_item = Some(vec![line]);
            }
        } else if let Some(ref mut buf) = cur_item {
            buf.push(line);
            if indent == item_indent && trimmed == "}" {
                items.push(cur_item.take().unwrap());
            }
        }
    }
    let item_lines = items
        .get(slot.flat_item_idx)
        .unwrap_or_else(|| panic!("Compose item {} not found for {child_name:?} (found {}):\n{compose}", slot.flat_item_idx, items.len()));
    let Some(sub_i) = slot.sub_idx else {
        return item_lines.join("\n");
    };
    let box_indent = item_indent + 4;
    let box_open = item_lines
        .iter()
        .position(|l| {
            let t = l.trim_start();
            l.len() - t.len() == box_indent && t.starts_with("Box(")
        })
        .unwrap_or_else(|| panic!("outer cell Box not found for {child_name:?}:\n{}", item_lines.join("\n")));
    let box_close = item_lines
        .iter()
        .enumerate()
        .skip(box_open + 1)
        .find(|(_, l)| {
            let t = l.trim_start();
            l.len() - t.len() == box_indent && t == "}"
        })
        .map(|(i, _)| i)
        .unwrap_or(item_lines.len());
    let sub_indent = box_indent + 4;
    let mut subs: Vec<Vec<&str>> = Vec::new();
    let mut pending: Vec<&str> = Vec::new();
    for &line in &item_lines[box_open + 1..box_close] {
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        let indent = line.len() - trimmed.len();
        if indent == sub_indent && trimmed.starts_with("//") {
            pending.push(line);
        } else if indent == sub_indent && !trimmed.starts_with('.') && !trimmed.starts_with('}') && !trimmed.starts_with(')') {
            let mut cur = std::mem::take(&mut pending);
            cur.push(line);
            subs.push(cur);
        } else if let Some(last) = subs.last_mut() {
            last.append(&mut pending);
            last.push(line);
        }
    }
    let sub = subs.get(sub_i).unwrap_or_else(|| panic!("Compose stacked subview {sub_i} not found for {child_name:?}:\n{}", item_lines.join("\n")));
    if sub_i == 0 {
        let header = item_lines[..=box_open].join("\n");
        format!("{header}\n{}", sub.join("\n"))
    } else {
        sub.join("\n")
    }
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
    let (widths, rows, placed, col_gap, row_gap, _) = solver_expectations(scene);
    let kids = solver_kids(scene);
    let flow_ids = solver_flow_ids(&scene.doc, scene.grid);
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
    for (cid, (p, k)) in flow_ids.into_iter().zip(placed.iter().zip(&kids)) {
        let child_name = &scene.doc.get(cid).unwrap().name;
        let cell_code = swiftui_cell_for(&scene.doc, scene.grid, &swift, child_name);
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
        assert!(cell_code.contains(&needle), "{name} SwiftUI cell for {child_name:?} missing cell frame {needle}:\n{cell_code}");
        if p.row_span > 1 {
            let cell_h_span: f32 = (0..p.row_span).map(|dr| rows[p.row + dr]).sum::<f32>() + row_gap * (p.row_span.saturating_sub(1) as f32);
            if (cell_h_span - cell_h_first).abs() > 1e-4 {
                let bad_needle = format!(".frame(width: {}, height: {}, alignment: {swift_align})", fmt_num(cell_w), fmt_num(cell_h_span));
                assert!(
                    !cell_code.contains(&bad_needle),
                    "{name} SwiftUI row-span cell for {child_name:?} must use first-row height {}, not span height {bad_needle}:\n{cell_code}",
                    fmt_num(cell_h_first)
                );
            }
        }
        if p.col_span > 1 {
            assert!(
                cell_code.contains(&format!(".gridCellColumns({})", p.col_span)),
                "{name} SwiftUI cell for {child_name:?} missing .gridCellColumns({}):\n{cell_code}",
                p.col_span
            );
        }
    }
}

fn assert_compose_scene(scene: &NativeGridScene) {
    let name = scene.name;
    let (widths, rows, placed, _, row_gap, padding) = solver_expectations(scene);
    let kids = solver_kids(scene);
    let flow_ids = solver_flow_ids(&scene.doc, scene.grid);
    let compose = compose_for(&scene.doc, scene.grid);
    assert!(compose.contains("LazyVerticalGrid("), "{name} Compose missing LazyVerticalGrid(: {compose}");
    let widths_dp = widths.iter().map(|w| format!("{}.dp", fmt_num(*w))).collect::<Vec<_>>().join(", ");
    assert!(compose.contains(&format!("listOf({widths_dp})")), "{name} Compose missing listOf({widths_dp}): {compose}");
    for &h in &rows {
        let hs = fmt_num(h);
        assert!(compose.contains(&format!("Modifier.height({hs}.dp)")), "{name} Compose missing row height {hs}.dp: {compose}");
    }
    for (cid, (p, k)) in flow_ids.into_iter().zip(placed.iter().zip(&kids)) {
        let child_name = &scene.doc.get(cid).unwrap().name;
        let cell_code = compose_cell_for(&scene.doc, scene.grid, &compose, child_name);
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
        assert!(cell_code.contains(&box_needle), "{name} Compose cell for {child_name:?} missing cell box {box_needle}:\n{cell_code}");
        if p.row_span > 1 {
            let cell_h_span: f32 = (0..p.row_span).map(|dr| rows[p.row + dr]).sum::<f32>() + row_gap * (p.row_span.saturating_sub(1) as f32);
            if (cell_h_span - cell_h_first).abs() > 1e-4 {
                let bad_box = format!("Box(Modifier.height({}.dp), contentAlignment = {compose_align})", fmt_num(cell_h_span));
                assert!(
                    !cell_code.contains(&bad_box),
                    "{name} Compose row-span cell for {child_name:?} must use first-row height {}, not span height {bad_box}:\n{cell_code}",
                    fmt_num(cell_h_first)
                );
            }
        }
        if p.col_span > 1 {
            assert!(
                cell_code.contains(&format!("GridItemSpan({})", p.col_span)),
                "{name} Compose cell for {child_name:?} missing GridItemSpan({}):\n{cell_code}",
                p.col_span
            );
        }
    }
    let n = scene.doc.get(scene.grid).unwrap();
    let al = n.kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
    let compose_h = if al.counter_sizing == Sizing::Fixed || n.layout_child.vertical == Sizing::Fill {
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
fn swiftui_exports_all_six_grid_scenes() {
    for scene in build_six_scenes() {
        assert_swiftui_scene(&scene);
    }
}

#[test]
fn compose_exports_all_six_grid_scenes() {
    for scene in build_six_scenes() {
        assert_compose_scene(&scene);
    }
}

#[test]
fn uikit_exports_all_six_grid_scenes() {
    for scene in build_six_scenes() {
        assert_uikit_scene(&scene);
    }
}

#[test]
fn android_xml_exports_all_six_grid_scenes() {
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

    // SwiftUI, Compose and UIKit emit the row-span comment with the node's name.
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
    for scene in build_six_scenes() {
        let xml = android_xml_for(&scene.doc, scene.grid);
        assert!(xml.contains("<GridLayout") && xml.contains("</GridLayout>"));
    }
    let span = scene_row_and_column_spans();
    let span_swift = swiftui_for(&span.doc, span.grid);
    let span_xml = android_xml_for(&span.doc, span.grid);
    let span_compose = compose_for(&span.doc, span.grid);
    let span_uikit = uikit_for(&span.doc, span.grid);
    assert!(span_xml.contains("<GridLayout") && span_xml.contains("</GridLayout>"));
    assert!(span_swift.contains("Grid(") && span_swift.contains("GridRow {"));
    assert!(span_compose.contains("LazyVerticalGrid("));
    assert!(span_uikit.contains("UIStackView()"));
}

#[test]
fn overlap_in_same_cell_keeps_all_children_and_solver_offsets() {
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
    let primary_swift = swiftui_cell_for(&doc, grid, &swift, "PrimaryTile");
    assert!(
        primary_swift.contains(".frame(width: 100, height: 60)\n                .frame(width: 100, height: 60, alignment: .topLeading)")
            && primary_swift.contains("}\n            .frame(width: 100, height: 60, alignment: .topLeading)"),
        "SwiftUI PrimaryTile cell should frame primary and outer ZStack at 100x60 .topLeading:\n{primary_swift}"
    );
    let badge_swift = swiftui_cell_for(&doc, grid, &swift, "OverlapBadge");
    assert!(
        badge_swift.contains(".frame(width: 40, height: 20)\n                .frame(width: 100, height: 60, alignment: .trailing)")
            && !badge_swift.contains(".offset("),
        "SwiftUI OverlapBadge should have own 100x60 .trailing cell box and no offset in same cell:\n{badge_swift}"
    );

    let compose = compose_for(&doc, grid);
    assert!(
        compose.contains("// \"OverlapBadge\": overlapping grid cells stacked in the cell of \"PrimaryTile\""),
        "Compose missing overlap comment:\n{compose}"
    );
    assert_eq!(compose.matches("item {").count(), 2, "Compose should emit 2 grid items (shared cell + right cell):\n{compose}");
    let primary_compose = compose_cell_for(&doc, grid, &compose, "PrimaryTile");
    assert!(
        primary_compose.contains("Box(Modifier.height(60.dp), contentAlignment = Alignment.TopStart)")
            && primary_compose.contains(".size(width = 100.dp, height = 60.dp)"),
        "Compose PrimaryTile cell:\n{primary_compose}"
    );
    let badge_compose = compose_cell_for(&doc, grid, &compose, "OverlapBadge");
    assert!(
        badge_compose.contains(
            "Box(Modifier.align(Alignment.TopStart).wrapContentSize(Alignment.TopStart, unbounded = true).size(width = 100.dp, height = 60.dp), contentAlignment = Alignment.CenterEnd)"
        ) && badge_compose.contains(".size(width = 40.dp, height = 20.dp)")
            && !badge_compose.contains(".offset("),
        "Compose OverlapBadge should have own 100x60 CenterEnd sub-cell Box and no offset in same cell:\n{badge_compose}"
    );

    let uikit = uikit_for(&doc, grid);
    assert!(uikit.contains("// \"OverlapBadge\": overlapping grid cells stacked in the cell of \"PrimaryTile\""), "UIKit missing overlap comment:\n{uikit}");
    assert!(
        uikit.contains("cell.addSubview(primaryTile)") && uikit.contains("cell.addSubview(stackCell)") && uikit.contains("stackCell.addSubview(overlapBadge)"),
        "UIKit should add primaryTile and stackCell to cell, and overlapBadge to stackCell:\n{uikit}"
    );
    assert!(
        uikit.contains("stackCell.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 0)")
            && uikit.contains("stackCell.topAnchor.constraint(equalTo: cell.topAnchor, constant: 0)")
            && uikit.contains("stackCell.widthAnchor.constraint(equalToConstant: 100)")
            && uikit.contains("stackCell.heightAnchor.constraint(equalToConstant: 60)")
            && uikit.contains("overlapBadge.trailingAnchor.constraint(equalTo: stackCell.trailingAnchor)")
            && uikit.contains("overlapBadge.centerYAnchor.constraint(equalTo: stackCell.centerYAnchor)"),
        "UIKit missing stackCell constraints or OverlapBadge trailing/centerY anchors on stackCell:\n{uikit}"
    );

    let xml = android_xml_for(&doc, grid);
    assert_eq!(xml.matches("<FrameLayout").count(), 3, "Android XML should keep all 3 flow children:\n{xml}");
}

#[test]
fn overlap_inside_col_span_keeps_all_children_and_solver_offsets() {
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
    let banner_swift = swiftui_cell_for(&doc, grid, &swift, "SpanBanner");
    assert!(
        banner_swift.contains(".frame(width: 210, height: 60)\n                .frame(width: 210, height: 60, alignment: .topLeading)")
            && banner_swift.contains("}\n            .frame(width: 210, height: 60, alignment: .topLeading)\n            .gridCellColumns(2)"),
        "SwiftUI SpanBanner cell:\n{banner_swift}"
    );
    let chip_swift = swiftui_cell_for(&doc, grid, &swift, "Col1Chip");
    assert!(
        chip_swift.contains(
            ".frame(width: 50, height: 24)\n                .frame(width: 100, height: 60, alignment: .leading)\n                .offset(x: 110, y: 0)"
        ),
        "SwiftUI Col1Chip should have 100x60 .leading cell box and cell-origin offset (110, 0):\n{chip_swift}"
    );

    let compose = compose_for(&doc, grid);
    assert!(
        compose.contains("// \"Col1Chip\": overlapping grid cells stacked in the cell of \"SpanBanner\""),
        "Compose missing col_span overlap comment:\n{compose}"
    );
    let chip_compose = compose_cell_for(&doc, grid, &compose, "Col1Chip");
    assert!(
        chip_compose.contains(
            "Box(Modifier.align(Alignment.TopStart).offset(x = 110.dp, y = 0.dp).wrapContentSize(Alignment.TopStart, unbounded = true).size(width = 100.dp, height = 60.dp), contentAlignment = Alignment.CenterStart)"
        ) && chip_compose.contains(".size(width = 50.dp, height = 24.dp)"),
        "Compose Col1Chip should have cell-origin offset (110.dp, 0.dp) and 100x60 CenterStart sub-cell Box:\n{chip_compose}"
    );

    let uikit = uikit_for(&doc, grid);
    assert!(
        uikit.contains("// \"Col1Chip\": overlapping grid cells stacked in the cell of \"SpanBanner\""),
        "UIKit missing col_span overlap comment:\n{uikit}"
    );
    assert!(
        uikit.contains("stackCell.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 110)")
            && uikit.contains("stackCell.topAnchor.constraint(equalTo: cell.topAnchor, constant: 0)")
            && uikit.contains("stackCell.widthAnchor.constraint(equalToConstant: 100)")
            && uikit.contains("stackCell.heightAnchor.constraint(equalToConstant: 60)")
            && uikit.contains("col1Chip.leadingAnchor.constraint(equalTo: stackCell.leadingAnchor)")
            && uikit.contains("col1Chip.centerYAnchor.constraint(equalTo: stackCell.centerYAnchor)"),
        "UIKit missing Col1Chip stackCell (110, 0, 100x60) or leading/centerY anchors:\n{uikit}"
    );

    let xml = android_xml_for(&doc, grid);
    assert_eq!(xml.matches("<FrameLayout").count(), 3, "Android XML should keep all 3 flow children:\n{xml}");
}

#[test]
fn stacked_badge_keeps_its_own_cell_alignment_beside_a_centred_icon() {
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
    al.grid_col_tracks[1] = GridTrack::Fixed(100.0);
    al.grid_row_tracks[0] = GridTrack::Fixed(60.0);
    let (mut doc, grid) = new_grid(al, Vec2::new(210.0, 60.0));
    let icon = add_named_item(&mut doc, grid, "Icon", 20.0, 20.0, Sizing::Fixed, Sizing::Fixed);
    {
        let n = doc.get_mut(icon).unwrap();
        n.fills = vec![Paint::solid(Color::rgb(0x35, 0x82, 0xFF))];
        n.layout_child.grid_column = Some(0);
        n.layout_child.grid_row = Some(0);
        n.layout_child.grid_align_h = GridChildAlign::Center;
        n.layout_child.grid_align_v = GridChildAlign::Center;
    }
    let badge = add_named_item(&mut doc, grid, "Badge", 20.0, 20.0, Sizing::Fixed, Sizing::Fixed);
    {
        let n = doc.get_mut(badge).unwrap();
        n.fills = vec![Paint::solid(Color::rgb(0xFF, 0x30, 0x30))];
        n.layout_child.grid_column = Some(0);
        n.layout_child.grid_row = Some(0);
        n.layout_child.grid_align_h = GridChildAlign::End;
        n.layout_child.grid_align_v = GridChildAlign::Start;
    }
    let caption = add_named_item(&mut doc, grid, "Caption", 100.0, 60.0, Sizing::Fixed, Sizing::Fixed);
    {
        let n = doc.get_mut(caption).unwrap();
        n.fills = vec![Paint::solid(Color::rgb(0x20, 0xC0, 0x60))];
        n.layout_child.grid_column = Some(1);
        n.layout_child.grid_row = Some(0);
    }
    doc.relayout_subtree(grid);
    assert_eq!(
        solver_rects(&doc, grid),
        vec![
            ("Icon".to_string(), Rect::new(40.0, 20.0, 20.0, 20.0)),
            ("Badge".to_string(), Rect::new(80.0, 0.0, 20.0, 20.0)),
            ("Caption".to_string(), Rect::new(110.0, 0.0, 100.0, 60.0)),
        ]
    );

    let swift = swiftui_for(&doc, grid);
    let icon_swift = swiftui_cell_for(&doc, grid, &swift, "Icon");
    assert!(
        icon_swift.contains(".frame(width: 20, height: 20)\n                .frame(width: 100, height: 60, alignment: .center)")
            && icon_swift.contains("}\n            .frame(width: 100, height: 60, alignment: .topLeading)"),
        "SwiftUI Icon should be centered in its own 100x60 frame inside a .topLeading ZStack:\n{icon_swift}"
    );
    let badge_swift = swiftui_cell_for(&doc, grid, &swift, "Badge");
    assert!(
        badge_swift.contains(".frame(width: 20, height: 20)\n                .frame(width: 100, height: 60, alignment: .topTrailing)")
            && !badge_swift.contains(".offset("),
        "SwiftUI Badge should be .topTrailing in its own 100x60 frame without double-counting offset:\n{badge_swift}"
    );

    let compose = compose_for(&doc, grid);
    let icon_compose = compose_cell_for(&doc, grid, &compose, "Icon");
    assert!(
        icon_compose.contains("Box(Modifier.height(60.dp), contentAlignment = Alignment.Center)") && icon_compose.contains(".size(20.dp)"),
        "Compose Icon cell:\n{icon_compose}"
    );
    let badge_compose = compose_cell_for(&doc, grid, &compose, "Badge");
    assert!(
        badge_compose.contains(
            "Box(Modifier.align(Alignment.TopStart).wrapContentSize(Alignment.TopStart, unbounded = true).size(width = 100.dp, height = 60.dp), contentAlignment = Alignment.TopEnd)"
        ) && badge_compose.contains(".size(20.dp)")
            && !badge_compose.contains(".offset("),
        "Compose Badge sub-cell:\n{badge_compose}"
    );

    let uikit = uikit_for(&doc, grid);
    assert!(
        uikit.contains("icon.centerXAnchor.constraint(equalTo: cell.centerXAnchor)")
            && uikit.contains("icon.centerYAnchor.constraint(equalTo: cell.centerYAnchor)")
            && uikit.contains("stackCell.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 0)")
            && uikit.contains("stackCell.topAnchor.constraint(equalTo: cell.topAnchor, constant: 0)")
            && uikit.contains("stackCell.widthAnchor.constraint(equalToConstant: 100)")
            && uikit.contains("stackCell.heightAnchor.constraint(equalToConstant: 60)")
            && uikit.contains("badge.trailingAnchor.constraint(equalTo: stackCell.trailingAnchor)")
            && uikit.contains("badge.topAnchor.constraint(equalTo: stackCell.topAnchor)"),
        "UIKit Icon/Badge anchors:\n{uikit}"
    );
}

#[test]
fn stacked_start_chip_keeps_top_leading_alignment_over_centred_photo() {
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
    al.grid_col_tracks[1] = GridTrack::Fixed(100.0);
    al.grid_row_tracks[0] = GridTrack::Fixed(60.0);
    let (mut doc, grid) = new_grid(al, Vec2::new(210.0, 60.0));
    let photo = add_named_item(&mut doc, grid, "Photo", 60.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    {
        let n = doc.get_mut(photo).unwrap();
        n.fills = vec![Paint::solid(Color::rgb(0x35, 0x82, 0xFF))];
        n.layout_child.grid_column = Some(0);
        n.layout_child.grid_row = Some(0);
        n.layout_child.grid_align_h = GridChildAlign::Center;
        n.layout_child.grid_align_v = GridChildAlign::Center;
    }
    let chip = add_named_item(&mut doc, grid, "Chip", 30.0, 12.0, Sizing::Fixed, Sizing::Fixed);
    {
        let n = doc.get_mut(chip).unwrap();
        n.fills = vec![Paint::solid(Color::rgb(0xFF, 0x30, 0x30))];
        n.layout_child.grid_column = Some(0);
        n.layout_child.grid_row = Some(0);
        n.layout_child.grid_align_h = GridChildAlign::Start;
        n.layout_child.grid_align_v = GridChildAlign::Start;
    }
    let caption = add_named_item(&mut doc, grid, "Caption", 100.0, 60.0, Sizing::Fixed, Sizing::Fixed);
    {
        let n = doc.get_mut(caption).unwrap();
        n.fills = vec![Paint::solid(Color::rgb(0x20, 0xC0, 0x60))];
        n.layout_child.grid_column = Some(1);
        n.layout_child.grid_row = Some(0);
    }
    doc.relayout_subtree(grid);
    assert_eq!(
        solver_rects(&doc, grid),
        vec![
            ("Photo".to_string(), Rect::new(20.0, 15.0, 60.0, 30.0)),
            ("Chip".to_string(), Rect::new(0.0, 0.0, 30.0, 12.0)),
            ("Caption".to_string(), Rect::new(110.0, 0.0, 100.0, 60.0)),
        ]
    );

    let swift = swiftui_for(&doc, grid);
    let photo_swift = swiftui_cell_for(&doc, grid, &swift, "Photo");
    assert!(
        photo_swift.contains(".frame(width: 60, height: 30)\n                .frame(width: 100, height: 60, alignment: .center)")
            && photo_swift.contains("}\n            .frame(width: 100, height: 60, alignment: .topLeading)"),
        "SwiftUI Photo cell:\n{photo_swift}"
    );
    let chip_swift = swiftui_cell_for(&doc, grid, &swift, "Chip");
    assert!(
        chip_swift.contains(".frame(width: 30, height: 12)\n                .frame(width: 100, height: 60, alignment: .topLeading)")
            && !chip_swift.contains(".offset("),
        "SwiftUI Chip should land at (0, 0) via .topLeading in 100x60 without inheriting Photo's center alignment:\n{chip_swift}"
    );

    let compose = compose_for(&doc, grid);
    let chip_compose = compose_cell_for(&doc, grid, &compose, "Chip");
    assert!(
        chip_compose.contains(
            "Box(Modifier.align(Alignment.TopStart).wrapContentSize(Alignment.TopStart, unbounded = true).size(width = 100.dp, height = 60.dp), contentAlignment = Alignment.TopStart)"
        ) && chip_compose.contains(".size(width = 30.dp, height = 12.dp)"),
        "Compose Chip sub-cell:\n{chip_compose}"
    );

    let uikit = uikit_for(&doc, grid);
    assert!(
        uikit.contains("photo.centerXAnchor.constraint(equalTo: cell.centerXAnchor)")
            && uikit.contains("photo.centerYAnchor.constraint(equalTo: cell.centerYAnchor)")
            && uikit.contains("chip.leadingAnchor.constraint(equalTo: stackCell.leadingAnchor)")
            && uikit.contains("chip.topAnchor.constraint(equalTo: stackCell.topAnchor)"),
        "UIKit Photo/Chip anchors:\n{uikit}"
    );
}

#[test]
fn centred_dot_keeps_its_position_when_larger_card_is_stacked_on_it() {
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
    al.grid_col_tracks[1] = GridTrack::Fixed(100.0);
    al.grid_row_tracks[0] = GridTrack::Fixed(60.0);
    let (mut doc, grid) = new_grid(al, Vec2::new(210.0, 60.0));
    let dot = add_named_item(&mut doc, grid, "Dot", 20.0, 20.0, Sizing::Fixed, Sizing::Fixed);
    {
        let n = doc.get_mut(dot).unwrap();
        n.fills = vec![Paint::solid(Color::rgb(0xFF, 0x30, 0x30))];
        n.layout_child.grid_column = Some(0);
        n.layout_child.grid_row = Some(0);
        n.layout_child.grid_align_h = GridChildAlign::Center;
        n.layout_child.grid_align_v = GridChildAlign::Center;
    }
    let card = add_named_item(&mut doc, grid, "Card", 100.0, 60.0, Sizing::Fixed, Sizing::Fixed);
    {
        let n = doc.get_mut(card).unwrap();
        n.fills = vec![Paint::solid(Color::rgb(0x35, 0x82, 0xFF))];
        n.layout_child.grid_column = Some(0);
        n.layout_child.grid_row = Some(0);
        n.layout_child.grid_align_h = GridChildAlign::Start;
        n.layout_child.grid_align_v = GridChildAlign::Start;
    }
    let other = add_named_item(&mut doc, grid, "Other", 100.0, 60.0, Sizing::Fixed, Sizing::Fixed);
    {
        let n = doc.get_mut(other).unwrap();
        n.fills = vec![Paint::solid(Color::rgb(0x20, 0xC0, 0x60))];
        n.layout_child.grid_column = Some(1);
        n.layout_child.grid_row = Some(0);
    }
    doc.relayout_subtree(grid);
    assert_eq!(
        solver_rects(&doc, grid),
        vec![
            ("Dot".to_string(), Rect::new(40.0, 20.0, 20.0, 20.0)),
            ("Card".to_string(), Rect::new(0.0, 0.0, 100.0, 60.0)),
            ("Other".to_string(), Rect::new(110.0, 0.0, 100.0, 60.0)),
        ]
    );

    let swift = swiftui_for(&doc, grid);
    let dot_swift = swiftui_cell_for(&doc, grid, &swift, "Dot");
    assert!(
        dot_swift.contains(".frame(width: 20, height: 20)\n                .frame(width: 100, height: 60, alignment: .center)")
            && dot_swift.contains("}\n            .frame(width: 100, height: 60, alignment: .topLeading)"),
        "SwiftUI Dot must stay centered in its own 100x60 frame even when Card (100x60) is larger:\n{dot_swift}"
    );
    let card_swift = swiftui_cell_for(&doc, grid, &swift, "Card");
    assert!(
        card_swift.contains(".frame(width: 100, height: 60)\n                .frame(width: 100, height: 60, alignment: .topLeading)")
            && !card_swift.contains(".offset("),
        "SwiftUI Card cell:\n{card_swift}"
    );

    let compose = compose_for(&doc, grid);
    let dot_compose = compose_cell_for(&doc, grid, &compose, "Dot");
    assert!(
        dot_compose.contains("Box(Modifier.height(60.dp), contentAlignment = Alignment.Center)") && dot_compose.contains(".size(20.dp)"),
        "Compose Dot cell:\n{dot_compose}"
    );
    let card_compose = compose_cell_for(&doc, grid, &compose, "Card");
    assert!(
        card_compose.contains(
            "Box(Modifier.align(Alignment.TopStart).wrapContentSize(Alignment.TopStart, unbounded = true).size(width = 100.dp, height = 60.dp), contentAlignment = Alignment.TopStart)"
        ) && card_compose.contains(".size(width = 100.dp, height = 60.dp)"),
        "Compose Card sub-cell:\n{card_compose}"
    );

    let uikit = uikit_for(&doc, grid);
    assert!(
        uikit.contains("dot.centerXAnchor.constraint(equalTo: cell.centerXAnchor)")
            && uikit.contains("dot.centerYAnchor.constraint(equalTo: cell.centerYAnchor)")
            && uikit.contains("card.leadingAnchor.constraint(equalTo: stackCell.leadingAnchor)")
            && uikit.contains("card.topAnchor.constraint(equalTo: stackCell.topAnchor)"),
        "UIKit Dot/Card anchors:\n{uikit}"
    );
}

#[test]
fn fill_child_stacked_inside_column_span_uses_its_own_column_width() {
    let mut al = AutoLayout {
        grid_columns: 3,
        grid_rows: 1,
        item_spacing: 10.0,
        counter_spacing: 10.0,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    al.grid_col_tracks[0] = GridTrack::Fixed(100.0);
    al.grid_col_tracks[1] = GridTrack::Fixed(40.0);
    al.grid_col_tracks[2] = GridTrack::Fixed(60.0);
    al.grid_row_tracks[0] = GridTrack::Fixed(40.0);
    let (mut doc, grid) = new_grid(al, Vec2::new(220.0, 40.0));
    let wide = add_named_item(&mut doc, grid, "Wide", 150.0, 40.0, Sizing::Fixed, Sizing::Fixed);
    {
        let n = doc.get_mut(wide).unwrap();
        n.fills = vec![Paint::solid(Color::rgb(0x35, 0x82, 0xFF))];
        n.layout_child.grid_column = Some(0);
        n.layout_child.grid_row = Some(0);
        n.layout_child.col_span = 2;
    }
    let inner_fill = add_named_item(&mut doc, grid, "InnerFill", 40.0, 20.0, Sizing::Fill, Sizing::Fixed);
    {
        let n = doc.get_mut(inner_fill).unwrap();
        n.fills = vec![Paint::solid(Color::rgb(0xFF, 0x30, 0x30))];
        n.layout_child.grid_column = Some(1);
        n.layout_child.grid_row = Some(0);
    }
    doc.relayout_subtree(grid);
    assert_eq!(
        solver_rects(&doc, grid),
        vec![("Wide".to_string(), Rect::new(0.0, 0.0, 150.0, 40.0)), ("InnerFill".to_string(), Rect::new(110.0, 0.0, 40.0, 20.0)),]
    );

    let swift = swiftui_for(&doc, grid);
    let fill_swift = swiftui_cell_for(&doc, grid, &swift, "InnerFill");
    assert!(
        fill_swift.contains(".frame(maxWidth: .infinity, height: 20)\n                .frame(width: 40, height: 40, alignment: .topLeading)\n                .offset(x: 110, y: 0)"),
        "SwiftUI InnerFill should fill its own 40x40 column-1 cell at offset (110, 0), not the 150x40 span:\n{fill_swift}"
    );

    let compose = compose_for(&doc, grid);
    let fill_compose = compose_cell_for(&doc, grid, &compose, "InnerFill");
    assert!(
        fill_compose.contains(
            "Box(Modifier.align(Alignment.TopStart).offset(x = 110.dp, y = 0.dp).wrapContentSize(Alignment.TopStart, unbounded = true).size(width = 40.dp, height = 40.dp), contentAlignment = Alignment.TopStart)"
        ) && fill_compose.contains(".height(20.dp)\n                        .fillMaxWidth()"),
        "Compose InnerFill should fill its own 40x40 sub-cell Box at offset (110.dp, 0.dp):\n{fill_compose}"
    );

    let uikit = uikit_for(&doc, grid);
    assert!(
        uikit.contains("stackCell.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 110)")
            && uikit.contains("stackCell.widthAnchor.constraint(equalToConstant: 40)")
            && uikit.contains("stackCell.heightAnchor.constraint(equalToConstant: 40)")
            && uikit.contains("innerFill.widthAnchor.constraint(equalTo: stackCell.widthAnchor)")
            && uikit.contains("innerFill.heightAnchor.constraint(equalToConstant: 20)")
            && uikit.contains("innerFill.leadingAnchor.constraint(equalTo: stackCell.leadingAnchor)")
            && uikit.contains("innerFill.topAnchor.constraint(equalTo: stackCell.topAnchor)"),
        "UIKit InnerFill should stretch to stackCell (40x40 at x=110), not cell (150x40):\n{uikit}"
    );
}

#[test]
fn spanning_banner_stacked_over_earlier_column_child_uses_negative_offset_and_own_span_width() {
    let mut al = AutoLayout {
        grid_columns: 3,
        grid_rows: 1,
        item_spacing: 10.0,
        counter_spacing: 10.0,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    al.grid_col_tracks[0] = GridTrack::Fixed(100.0);
    al.grid_col_tracks[1] = GridTrack::Fixed(40.0);
    al.grid_col_tracks[2] = GridTrack::Fixed(60.0);
    al.grid_row_tracks[0] = GridTrack::Fixed(40.0);
    let (mut doc, grid) = new_grid(al, Vec2::new(220.0, 40.0));
    let chip = add_named_item(&mut doc, grid, "Chip", 40.0, 20.0, Sizing::Fixed, Sizing::Fixed);
    {
        let n = doc.get_mut(chip).unwrap();
        n.layout_child.grid_column = Some(1);
        n.layout_child.grid_row = Some(0);
    }
    let banner = add_named_item(&mut doc, grid, "Banner", 150.0, 40.0, Sizing::Fixed, Sizing::Fixed);
    {
        let n = doc.get_mut(banner).unwrap();
        n.layout_child.grid_column = Some(0);
        n.layout_child.grid_row = Some(0);
        n.layout_child.col_span = 2;
    }
    doc.relayout_subtree(grid);
    assert_eq!(
        solver_rects(&doc, grid),
        vec![("Chip".to_string(), Rect::new(110.0, 0.0, 40.0, 20.0)), ("Banner".to_string(), Rect::new(0.0, 0.0, 150.0, 40.0)),]
    );

    let swift = swiftui_for(&doc, grid);
    let chip_swift = swiftui_cell_for(&doc, grid, &swift, "Chip");
    assert!(
        chip_swift.contains(".frame(width: 40, height: 20)\n                .frame(width: 40, height: 40, alignment: .topLeading)")
            && chip_swift.contains("}\n            .frame(width: 40, height: 40, alignment: .topLeading)"),
        "SwiftUI Chip host cell:\n{chip_swift}"
    );
    let banner_swift = swiftui_cell_for(&doc, grid, &swift, "Banner");
    assert!(
        banner_swift.contains(
            ".frame(width: 150, height: 40)\n                .frame(width: 150, height: 40, alignment: .topLeading)\n                .offset(x: -110, y: 0)"
        ),
        "SwiftUI Banner should have 150x40 span cell box and negative offset (-110, 0):\n{banner_swift}"
    );

    let compose = compose_for(&doc, grid);
    let chip_compose = compose_cell_for(&doc, grid, &compose, "Chip");
    assert!(
        chip_compose.contains("Box(Modifier.height(40.dp), contentAlignment = Alignment.TopStart)")
            && chip_compose.contains(".size(width = 40.dp, height = 20.dp)"),
        "Compose Chip host cell:\n{chip_compose}"
    );
    let banner_compose = compose_cell_for(&doc, grid, &compose, "Banner");
    assert!(
        banner_compose.contains(
            "Box(Modifier.align(Alignment.TopStart).offset(x = -110.dp, y = 0.dp).wrapContentSize(Alignment.TopStart, unbounded = true).size(width = 150.dp, height = 40.dp), contentAlignment = Alignment.TopStart)"
        ) && banner_compose.contains(".size(width = 150.dp, height = 40.dp)"),
        "Compose Banner sub-cell should use unbounded 150x40 wrapper at offset (-110.dp, 0.dp):\n{banner_compose}"
    );

    let uikit = uikit_for(&doc, grid);
    assert!(
        uikit.contains("stackCell.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: -110)")
            && uikit.contains("stackCell.topAnchor.constraint(equalTo: cell.topAnchor, constant: 0)")
            && uikit.contains("stackCell.widthAnchor.constraint(equalToConstant: 150)")
            && uikit.contains("stackCell.heightAnchor.constraint(equalToConstant: 40)")
            && uikit.contains("banner.leadingAnchor.constraint(equalTo: stackCell.leadingAnchor)")
            && uikit.contains("banner.topAnchor.constraint(equalTo: stackCell.topAnchor)"),
        "UIKit Banner stackCell constraints:\n{uikit}"
    );
}

#[test]
fn stacked_child_spanning_rows_uses_first_row_height_in_sub_cell() {
    let mut al = AutoLayout {
        grid_columns: 2,
        grid_rows: 2,
        item_spacing: 10.0,
        counter_spacing: 10.0,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    al.grid_col_tracks[0] = GridTrack::Fixed(100.0);
    al.grid_col_tracks[1] = GridTrack::Fixed(100.0);
    al.grid_row_tracks[0] = GridTrack::Fixed(40.0);
    al.grid_row_tracks[1] = GridTrack::Fixed(50.0);
    let (mut doc, grid) = new_grid(al, Vec2::new(210.0, 100.0));
    let host = add_named_item(&mut doc, grid, "HostTile", 100.0, 40.0, Sizing::Fixed, Sizing::Fixed);
    {
        let n = doc.get_mut(host).unwrap();
        n.layout_child.grid_column = Some(0);
        n.layout_child.grid_row = Some(0);
    }
    let tall_badge = add_named_item(&mut doc, grid, "TallBadge", 30.0, 20.0, Sizing::Fixed, Sizing::Fixed);
    {
        let n = doc.get_mut(tall_badge).unwrap();
        n.layout_child.grid_column = Some(0);
        n.layout_child.grid_row = Some(0);
        n.layout_child.row_span = 2;
        n.layout_child.grid_align_h = GridChildAlign::Center;
        n.layout_child.grid_align_v = GridChildAlign::Center;
    }
    let row2_right = add_named_item(&mut doc, grid, "Row2Right", 100.0, 50.0, Sizing::Fixed, Sizing::Fixed);
    {
        let n = doc.get_mut(row2_right).unwrap();
        n.layout_child.grid_column = Some(1);
        n.layout_child.grid_row = Some(1);
    }
    doc.relayout_subtree(grid);

    let swift = swiftui_for(&doc, grid);
    assert!(
        swift.contains("// \"TallBadge\": row span 2 is not supported by SwiftUI Grid; placed in row 1"),
        "SwiftUI missing row-span comment for stacked TallBadge:\n{swift}"
    );
    let badge_swift = swiftui_cell_for(&doc, grid, &swift, "TallBadge");
    assert!(
        badge_swift.contains(".frame(width: 30, height: 20)\n                .frame(width: 100, height: 40, alignment: .center)"),
        "SwiftUI stacked TallBadge must use first-row height 40, not two-row span height 100:\n{badge_swift}"
    );

    let compose = compose_for(&doc, grid);
    assert!(
        compose.contains("// \"TallBadge\": row span 2 is not supported by Compose LazyVerticalGrid; placed in row 1"),
        "Compose missing row-span comment for stacked TallBadge:\n{compose}"
    );
    let badge_compose = compose_cell_for(&doc, grid, &compose, "TallBadge");
    assert!(
        badge_compose.contains(
            "Box(Modifier.align(Alignment.TopStart).wrapContentSize(Alignment.TopStart, unbounded = true).size(width = 100.dp, height = 40.dp), contentAlignment = Alignment.Center)"
        ),
        "Compose stacked TallBadge sub-cell must use first-row height 40.dp:\n{badge_compose}"
    );

    let uikit = uikit_for(&doc, grid);
    assert!(
        uikit.contains("// \"TallBadge\": row span 2 is not supported by UIKit stack grid; placed in row 1")
            && uikit.contains("stackCell.widthAnchor.constraint(equalToConstant: 100)")
            && uikit.contains("stackCell.heightAnchor.constraint(equalToConstant: 40)")
            && uikit.contains("tallBadge.centerXAnchor.constraint(equalTo: stackCell.centerXAnchor)")
            && uikit.contains("tallBadge.centerYAnchor.constraint(equalTo: stackCell.centerYAnchor)"),
        "UIKit stacked TallBadge must use first-row height 40 on stackCell:\n{uikit}"
    );
}

#[test]
fn compose_out_of_flow_children_wrap_grid_in_outer_box() {
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
fn grid_slot_keeps_slot_parameter() {
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
fn compose_grid_component_instance_keeps_frame_height() {
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
fn android_span_only_column_and_row_emit_space() {
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
fn android_out_of_flow_and_variable_hidden_children_wrap_grid_in_frame_layout() {
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
    assert!(
        xml.contains("    <GridLayout\n        android:layout_width=\"match_parent\"\n        android:layout_height=\"wrap_content\""),
        "Android XML should nest <GridLayout> with wrap_content height inside outer <FrameLayout>:\n{xml}"
    );
    let grid_close = xml.find("    </GridLayout>").expect("missing </GridLayout>");
    let abs_pos = xml.find("android:layout_marginStart=\"12dp\"").expect("missing AbsBadge marginStart");
    let vis_pos = xml.find("android:layout_width=\"90dp\"\n        android:layout_height=\"24dp\"").expect("missing variable-hidden ConditionalBanner view");
    assert!(abs_pos > grid_close && vis_pos > grid_close, "Out-of-flow and variable-hidden children must be emitted after </GridLayout>:\n{xml}");
}

#[test]
fn compose_grid_height_fixed_vs_hug_and_fill() {
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

    // 3. layout_child.vertical == Fill -> exports the frame's solved height (rows + gaps + padding applies to Hug only).
    doc.get_mut(grid).unwrap().layout_child.vertical = Sizing::Fill;
    let kt_fill = compose_for(&doc, grid);
    assert!(
        kt_fill.contains(".size(width = 220.dp, height = 200.dp)"),
        "Fill-height grid in Compose should export the frame's solved height 200.dp, not content height 106.dp:\n{kt_fill}"
    );
    assert!(!kt_fill.contains("fillMaxHeight"), "Compose LazyVerticalGrid must never emit fillMaxHeight:\n{kt_fill}");
}

#[test]
fn bound_counter_axis_spacing_and_non_start_cell_alignments() {
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
    let c1 = add_named_item(&mut doc, grid, "EndCenterItem", 64.0, 24.0, Sizing::Fixed, Sizing::Fixed);
    {
        let n = doc.get_mut(c1).unwrap();
        n.layout_child.grid_align_h = GridChildAlign::End;
        n.layout_child.grid_align_v = GridChildAlign::Center;
    }
    add_named_item(&mut doc, grid, "Row2A", 100.0, 40.0, Sizing::Fixed, Sizing::Fixed);
    add_named_item(&mut doc, grid, "Row2B", 100.0, 40.0, Sizing::Fixed, Sizing::Fixed);
    doc.relayout_subtree(grid);

    let scene = NativeGridScene { name: "counter_gap_and_align", doc, grid, items: vec![c0, c1] };
    let swift = swiftui_for(&scene.doc, scene.grid);
    assert!(swift.contains("verticalSpacing: Tokens.spacingRowGap"), "SwiftUI missing bound counterAxisSpacing token:\n{swift}");
    let cb_swift = swiftui_cell_for(&scene.doc, scene.grid, &swift, "CenterBottomItem");
    assert!(
        cb_swift.contains(".frame(width: 60, height: 24)\n            .frame(width: 100, height: 24, alignment: .bottom)"),
        "SwiftUI CenterBottomItem cell must have 60x24 view in 100x24 .bottom cell:\n{cb_swift}"
    );
    let ec_swift = swiftui_cell_for(&scene.doc, scene.grid, &swift, "EndCenterItem");
    assert!(
        ec_swift.contains(".frame(width: 64, height: 24)\n            .frame(width: 100, height: 24, alignment: .trailing)"),
        "SwiftUI EndCenterItem cell must have 64x24 view in 100x24 .trailing cell:\n{ec_swift}"
    );

    let compose = compose_for(&scene.doc, scene.grid);
    assert!(compose.contains("verticalArrangement = Arrangement.spacedBy(Tokens.spacingRowGap)"), "Compose missing bound counterAxisSpacing token:\n{compose}");
    let cb_compose = compose_cell_for(&scene.doc, scene.grid, &compose, "CenterBottomItem");
    assert!(
        cb_compose.contains("Box(Modifier.height(24.dp), contentAlignment = Alignment.BottomCenter)")
            && cb_compose.contains(".size(width = 60.dp, height = 24.dp)"),
        "Compose CenterBottomItem cell must have 60x24 view in 24.dp BottomCenter cell:\n{cb_compose}"
    );
    let ec_compose = compose_cell_for(&scene.doc, scene.grid, &compose, "EndCenterItem");
    assert!(
        ec_compose.contains("Box(Modifier.height(24.dp), contentAlignment = Alignment.CenterEnd)")
            && ec_compose.contains(".size(width = 64.dp, height = 24.dp)"),
        "Compose EndCenterItem cell must have 64x24 view in 24.dp CenterEnd cell:\n{ec_compose}"
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

    // Also verify counter_spacing == 0 falls back to item_spacing across multi-row grids in codegen.
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
fn explicit_trailing_grid_rows_exported() {
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
