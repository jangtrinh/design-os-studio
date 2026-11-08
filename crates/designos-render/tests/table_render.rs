//! Tables on the CPU renderer: cell fills, grid lines that skip merged blocks, cell text, and the
//! lowering registry surviving a clear in the middle of a draw.

mod common;

use common::*;
use designos_doc::*;
use designos_render::board;

const ORIGIN: f32 = 20.0;
const STROKE: [u8; 4] = [0, 0, 0, 255];
const ANCHOR: [u8; 4] = [255, 0, 0, 255];
const BODY: [u8; 4] = [0xEE, 0xEE, 0xEE, 255];

/// A 3x3 table of default tracks (160 x 44) at (20, 20), a grey table fill and a 2 px black grid.
/// With `merged`, the top-left 2x2 block is one cell filled red; otherwise cell (0, 0) is red too.
fn table(t: &mut T, merged: bool) -> (NodeId, Vec<NodeId>) {
    let mut nodes = build_table(&mut t.doc, 3, 3, Vec2::new(ORIGIN, ORIGIN));
    let table_id = nodes[0].id;
    nodes[0].fills = vec![Paint::solid(Color::rgb(0xEE, 0xEE, 0xEE))];
    nodes[0].stroke.paints = vec![Paint::solid(Color::BLACK)];
    nodes[0].stroke.weight = 2.0;
    if merged {
        let NodeKind::Table(p) = &nodes[0].kind else { unreachable!() };
        let (row, col) = (p.rows[0].id, p.cols[0].id);
        let covered: Vec<NodeId> = [(0, 1), (1, 0), (1, 1)]
            .iter()
            .map(|&(r, c)| {
                nodes.iter().find(|n| matches!(&n.kind, NodeKind::TableCell(x) if x.row == p.rows[r].id && x.col == p.cols[c].id)).map(|n| n.id).unwrap()
            })
            .collect();
        nodes.retain(|n| !covered.contains(&n.id));
        nodes[0].children.retain(|c| !covered.contains(c));
        if let NodeKind::Table(p) = &mut nodes[0].kind {
            p.merges.push(TableMerge { row, col, row_span: 2, col_span: 2 });
        }
        let anchor = nodes.iter_mut().find(|n| matches!(&n.kind, NodeKind::TableCell(x) if x.row == row && x.col == col)).unwrap();
        anchor.size = Vec2::new(320.0, 88.0);
    }
    nodes[1].fills = vec![Paint::solid(Color::rgb(255, 0, 0))];
    let cells: Vec<NodeId> = nodes[1..].iter().map(|n| n.id).collect();
    Command::Insert { parent: t.page, index: 0, nodes }.apply(&mut t.doc).unwrap();
    (table_id, cells)
}

/// Puts `entries` throwaway lowered children into the lowering registry of this thread by lowering
/// loose cells through the public entry point (cells never clear the registry themselves). Call it
/// on a thread whose registry is still empty: that is what lands the registry one below its limit.
fn fill_registry(entries: usize) {
    let doc = Document::new("registry filler");
    let track = TrackId { session: 0, local: 1 };
    for i in 0..entries as u64 {
        let mut n = Node::new(NodeId(1_000_000 + i), NodeKind::TableCell(TableCellProps { row: track, col: track, text: default_cell_text("x") }));
        n.size = Vec2::new(100.0, 40.0);
        board::lower(&doc, &n).expect("a cell lowers");
    }
}

#[test]
fn merged_block_is_one_cell_without_inner_grid_lines() {
    let mut t = T::new();
    table(&mut t, true);
    let pm = t.render(560, 200);
    // merged block spans x 20..340, y 20..108
    assert_eq!(px(&pm, 100, 50), ANCHOR, "centre of the merged block is the anchor fill");
    assert_eq!(px(&pm, 179, 40), ANCHOR, "no vertical line at the old column boundary inside the merge");
    assert_eq!(px(&pm, 100, 63), ANCHOR, "no horizontal line at the old row boundary inside the merge");
    // row 2 keeps its column boundaries and the line under the merge
    assert_eq!(px(&pm, 339, 130), STROKE, "line between columns 1 and 2 in row 2");
    assert_eq!(px(&pm, 179, 130), STROKE, "line between columns 0 and 1 in row 2");
    assert_eq!(px(&pm, 100, 107), STROKE, "line under the merged block");
    // the merged block's right edge continues as the column 1/2 line of the rows it spans
    assert_eq!(px(&pm, 339, 50), STROKE, "right edge of the block");
    // outer border
    assert_eq!(px(&pm, 20, 100), STROKE);
    assert_eq!(px(&pm, 100, 20), STROKE);
    assert_eq!(px(&pm, 100, 152), STROKE);
    assert_eq!(px(&pm, 500, 130), STROKE);
    assert_eq!(px(&pm, 400, 50), BODY, "cell without a fill shows the table fill");
}

#[test]
fn plain_table_draws_every_boundary() {
    let mut t = T::new();
    table(&mut t, false);
    let pm = t.render(560, 200);
    assert_eq!(px(&pm, 100, 50), ANCHOR, "cell (0,0) fill");
    for (x, y) in [(179, 40), (339, 40), (100, 63), (100, 107), (179, 130), (400, 63)] {
        assert_eq!(px(&pm, x, y), STROKE, "grid line at ({x}, {y})");
    }
    assert_eq!(px(&pm, 250, 40), BODY);
}

#[test]
fn cell_text_is_drawn_inside_the_padding() {
    let mut t = T::new();
    let (_, cells) = table(&mut t, false);
    if let NodeKind::TableCell(c) = &mut t.node(cells[4]).kind {
        c.text = default_cell_text("Việt Nam");
        c.text.style.font_size = 20.0;
    }
    let pm = t.render(560, 200);
    // cell (1,1) is x 180..340, y 64..108; text sits in the padded box x 192..328, y 72..100
    let inked = |x0: u32, y0: u32, x1: u32, y1: u32| (y0..y1).flat_map(|y| (x0..x1).map(move |x| (x, y))).filter(|&(x, y)| px(&pm, x, y)[0] < 100).count();
    assert!(inked(192, 72, 328, 100) > 40, "the text has ink in the padded box");
    assert_eq!(inked(184, 67, 191, 100), 0, "nothing left of the horizontal padding");
    assert_eq!(inked(184, 67, 336, 71), 0, "nothing above the vertical padding");
}

#[test]
fn text_in_a_24_px_column_is_still_drawn() {
    let mut t = T::new();
    let mut nodes = build_table(&mut t.doc, 1, 2, Vec2::new(ORIGIN, ORIGIN));
    nodes[0].fills = vec![Paint::solid(Color::WHITE)];
    if let NodeKind::Table(p) = &mut nodes[0].kind {
        p.cols[0].size = TABLE_MIN_TRACK;
    }
    nodes[1].size = Vec2::new(TABLE_MIN_TRACK, TABLE_DEFAULT_ROW_HEIGHT);
    nodes[2].transform = Affine::translate(TABLE_MIN_TRACK, 0.0);
    if let NodeKind::TableCell(c) = &mut nodes[1].kind {
        c.text = default_cell_text("W");
    }
    Command::Insert { parent: t.page, index: 0, nodes }.apply(&mut t.doc).unwrap();
    let pm = t.render(300, 100);
    let ink = (20..44).flat_map(|x| (20..64).map(move |y| (x, y))).filter(|&(x, y)| px(&pm, x, y)[0] < 100).count();
    assert!(ink > 0, "the text of a 24 px column has ink");
}

#[test]
fn scene_index_bounds_match_every_cell_rect() {
    let mut t = T::new();
    let (table_id, cells) = table(&mut t, false);
    let mut idx = designos_render::SceneIndex::new();
    idx.ensure(&t.doc, t.page, 1);
    assert_eq!(cells.len(), 9);
    let origin = t.doc.get(table_id).unwrap().transform;
    for id in cells {
        let n = t.doc.get(id).unwrap();
        let want = (origin.tx + n.transform.tx, origin.ty + n.transform.ty, origin.tx + n.transform.tx + n.size.x, origin.ty + n.transform.ty + n.size.y);
        assert_eq!(idx.bounds_of(id), Some(want), "cell {id:?}");
    }
}

#[test]
fn grid_survives_a_registry_clear_during_the_draw() {
    let mut t = T::new();
    let (_, cells) = table(&mut t, false);
    for (i, id) in cells.iter().enumerate() {
        if let NodeKind::TableCell(c) = &mut t.node(*id).kind {
            c.text = default_cell_text(&format!("c{i}"));
        }
    }
    fill_registry(49_999);
    let pm = t.render(560, 200);
    assert!(
        board::get(&Document::new("x"), board::synth_id(NodeId(1_000_000), 0)).is_some(),
        "the draw ran one below the registry limit: the registry was not cleared before the table drew"
    );
    for (x, y) in [(179, 40), (339, 40), (100, 63), (100, 107), (20, 100), (500, 130)] {
        assert_eq!(px(&pm, x, y), STROKE, "grid line at ({x}, {y}) after a registry near its limit");
    }
}

/// Gives the table the highest id, so a map walk that meets the cells first lowers them before it
/// reaches their table.
fn table_after_its_cells(t: &mut T) -> (NodeId, Vec<NodeId>) {
    let mut nodes = build_table(&mut t.doc, 3, 3, Vec2::new(ORIGIN, ORIGIN));
    let fresh: Vec<NodeId> = (0..nodes.len()).map(|_| t.doc.alloc_id()).collect();
    let map: std::collections::HashMap<NodeId, NodeId> = nodes[1..].iter().map(|n| n.id).chain([nodes[0].id]).zip(fresh.iter().copied()).collect();
    for n in &mut nodes {
        n.id = map[&n.id];
        n.parent = n.parent.map(|p| map[&p]);
        n.children = n.children.iter().map(|c| map[c]).collect();
    }
    let ids: Vec<NodeId> = nodes[1..].iter().map(|n| n.id).collect();
    let table_id = nodes[0].id;
    nodes[0].fills = vec![Paint::solid(Color::WHITE)];
    Command::Insert { parent: t.page, index: 0, nodes }.apply(&mut t.doc).unwrap();
    (table_id, ids)
}

#[test]
fn lowered_document_keeps_lowered_cells_whatever_the_order() {
    let mut t = T::new();
    let (table_id, cells) = table_after_its_cells(&mut t);
    assert!(cells.iter().all(|c| c.0 < table_id.0), "the cells come before their table");
    if let NodeKind::TableCell(c) = &mut t.node(cells[0]).kind {
        c.text = default_cell_text("Xin chào");
    }
    let lowered = board::lowered_document(&t.doc).expect("a table is a board item");
    for id in cells.iter().chain([&table_id]) {
        let k = &lowered.get(*id).unwrap().kind;
        assert!(!matches!(k, NodeKind::TableCell(_) | NodeKind::Table(_)), "{id:?} is still raw: {k:?}");
    }
}

/// A 2x2 table with no fill and no grid whose cell (0, 0) holds far more text than its stored
/// 160 x 44 box. Returns the ink counted inside and outside that box.
fn overflowing_cell_render() -> (usize, usize) {
    let mut t = T::new();
    let mut nodes = build_table(&mut t.doc, 2, 2, Vec2::new(ORIGIN, ORIGIN));
    nodes[0].stroke.weight = 0.0;
    nodes[0].stroke.paints.clear();
    if let NodeKind::TableCell(c) = &mut nodes[1].kind {
        c.text = default_cell_text("W\nW\nW\nW\nW\nW\nW\nW\nW\nW");
    }
    Command::Insert { parent: t.page, index: 0, nodes }.apply(&mut t.doc).unwrap();
    let pm = t.render(400, 240);
    let ink = |inside: bool| {
        (0..240u32)
            .flat_map(|y| (0..400u32).map(move |x| (x, y)))
            .filter(|&(x, y)| ((20..180).contains(&x) && (20..64).contains(&y)) == inside && px(&pm, x, y)[0] < 160)
            .count()
    };
    (ink(true), ink(false))
}

#[test]
fn cell_text_taller_than_its_cell_never_paints_outside_it() {
    let (inside, outside) = overflowing_cell_render();
    assert!(inside > 20, "the text is drawn in its cell: {inside}");
    assert_eq!(outside, 0, "no ink outside the cell box");
}

/// The lowered kind of cell (0, 0) of a 2x2 table whose first cell holds `text` in its 160 x 44 box.
fn lowered_cell_kind(text: &str) -> NodeKind {
    let mut t = T::new();
    let nodes = build_table(&mut t.doc, 2, 2, Vec2::new(ORIGIN, ORIGIN));
    let cell = nodes[1].id;
    Command::Insert { parent: t.page, index: 0, nodes }.apply(&mut t.doc).unwrap();
    if let NodeKind::TableCell(c) = &mut t.node(cell).kind {
        c.text = default_cell_text(text);
    }
    board::lower(&t.doc, t.doc.get(cell).unwrap()).expect("a cell lowers").kind
}

#[test]
fn a_cell_whose_text_fits_stays_a_plain_rectangle() {
    assert!(matches!(lowered_cell_kind("Việt Nam"), NodeKind::Rectangle));
    assert!(matches!(lowered_cell_kind(""), NodeKind::Rectangle));
    assert!(matches!(lowered_cell_kind("Ô"), NodeKind::Rectangle));
}

#[test]
fn only_a_cell_whose_text_does_not_fit_is_clipped() {
    let tall = lowered_cell_kind("W\nW\nW\nW\nW\nW");
    assert!(matches!(&tall, NodeKind::Frame(f) if f.clips_content), "too tall: {tall:?}");
    let wide = lowered_cell_kind("Supercalifragilisticexpialidociousandmoreletters");
    assert!(matches!(&wide, NodeKind::Frame(f) if f.clips_content), "one word wider than the cell: {wide:?}");
}

/// Times a 100 x 10 table of fitting text inside a clipping frame at 2x as it lowers now, and with every
/// cell forced to clip. Run with `--ignored --nocapture`; it prints the numbers and asserts nothing
/// about speed, because timings depend on the machine.
#[test]
#[ignore = "prints timings"]
fn clip_cost_probe() {
    use designos_render::{render_page, tiny_skia::Pixmap, Viewport};
    let (rows, cols) = (100usize, 10usize);
    let mut t = T::new();
    let frame = t.doc.alloc_id();
    let mut fr = Node::frame(frame, Rect::new(0.0, 0.0, cols as f32 * 160.0, rows as f32 * 44.0));
    if let NodeKind::Frame(f) = &mut fr.kind {
        f.clips_content = true;
    }
    t.doc.insert(t.page, fr).unwrap();
    let mut nodes = build_table(&mut t.doc, rows, cols, Vec2::new(0.0, 0.0));
    let table = nodes[0].id;
    for n in nodes[1..].iter_mut() {
        if let NodeKind::TableCell(c) = &mut n.kind {
            c.text = default_cell_text("Cell text");
        }
    }
    Command::Insert { parent: frame, index: 0, nodes }.apply(&mut t.doc).unwrap();
    let lowered = board::lowered_document(&t.doc).unwrap();
    let mut forced = lowered.clone();
    for id in t.doc.get(table).unwrap().children.clone() {
        if let Some(n) = forced.get_mut(id) {
            n.kind = NodeKind::Frame(FrameProps { clips_content: true, ..FrameProps::default() });
        }
    }
    let (w, h) = ((cols as f32 * 160.0 * 2.0) as u32, (rows as f32 * 44.0 * 2.0) as u32);
    let vp = Viewport { offset: Vec2::ZERO, zoom: 2.0, dpr: 1.0 };
    for round in 0..3 {
        // the forced pass is slow in a debug build: once is enough to see the gap
        for (label, d) in [("as lowered", &lowered), ("every cell clipped", &forced)].into_iter().filter(|(l, _)| round == 0 || *l == "as lowered") {
            let mut pm = Pixmap::new(w, h).unwrap();
            let t0 = std::time::Instant::now();
            render_page(d, t.page, &vp, &mut pm);
            eprintln!("round {round} {label}: {:?}", t0.elapsed());
        }
    }
}

/// Writes the documents the visual evidence is captured from. Set `DESIGNOS_TABLE_EVIDENCE_DIR` and run
/// `cargo test -p designos-render --test table_render -- --ignored write_evidence_documents`.
#[test]
#[ignore = "writes fixture files for screenshots"]
fn write_evidence_documents() {
    let Some(dir) = std::env::var_os("DESIGNOS_TABLE_EVIDENCE_DIR") else { return };
    let dir = std::path::Path::new(&dir);
    for (name, merged) in [("table-3x3", false), ("table-merged-2x2", true)] {
        let mut t = T::new();
        let (_, cells) = table(&mut t, merged);
        let words = ["Tên", "Số lượng", "Đơn giá", "Việt Nam", "Tổng", "Ghi chú", "Hà Nội", "Đà Nẵng", "Huế"];
        for (id, w) in cells.iter().zip(words) {
            if let NodeKind::TableCell(c) = &mut t.node(*id).kind {
                c.text = default_cell_text(w);
            }
        }
        std::fs::write(dir.join(format!("{name}.designos")), t.doc.to_json().unwrap()).unwrap();
    }
    // a 2 x 4 table shaped like the owner's FigJam board: 192 x 64 cells, white, 20 px text inset
    let mut t = T::new();
    let mut nodes = build_table(&mut t.doc, 2, 4, Vec2::new(ORIGIN, ORIGIN));
    let (w, h) = (192.0, 64.0);
    nodes[0].size = Vec2::new(4.0 * w, 2.0 * h);
    if let NodeKind::Table(p) = &mut nodes[0].kind {
        p.rows.iter_mut().for_each(|r| r.size = h);
        p.cols.iter_mut().for_each(|c| c.size = w);
    }
    nodes[0].stroke.paints = vec![Paint { opacity: 0.2, ..Paint::solid(Color::BLACK) }];
    nodes[0].stroke.weight = 1.0;
    let words = ["Tên", "Vai trò", "Trạng thái", "Ghi chú", "An", "Thiết kế", "Xong", "Việt Nam"];
    for (i, n) in nodes[1..].iter_mut().enumerate() {
        n.size = Vec2::new(w, h);
        n.transform = Affine::translate((i % 4) as f32 * w, (i / 4) as f32 * h);
        n.fills = vec![Paint::solid(Color::WHITE)];
        if let NodeKind::TableCell(c) = &mut n.kind {
            c.text = default_cell_text(words[i]);
        }
    }
    Command::Insert { parent: t.page, index: 0, nodes }.apply(&mut t.doc).unwrap();
    std::fs::write(dir.join("table-2x4.designos"), t.doc.to_json().unwrap()).unwrap();
}
