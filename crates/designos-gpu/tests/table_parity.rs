//! Tables on the GPU renderer against the CPU reference: a plain 3x3 table, a table with a merged
//! 2x2 block, Vietnamese multi-line text and a cell fill. The compile tests run without a GPU.

mod common;

use common::*;
use designos_doc::*;
use designos_gpu::SceneCompiler;
use designos_render::{board, Viewport};
use std::time::Duration;

const SIZE: (u32, u32) = (520, 360);

fn insert_table(t: &mut T, rows: usize, cols: usize, origin: Vec2, merged: bool, texts: &[(usize, &str)]) -> NodeId {
    let mut nodes = build_table(&mut t.doc, rows, cols, origin);
    let table_id = nodes[0].id;
    nodes[0].fills = vec![Paint::solid(Color::rgb(0xF4, 0xF4, 0xF5))];
    nodes[0].stroke.paints = vec![Paint::solid(Color::rgb(0x27, 0x27, 0x2A))];
    nodes[0].stroke.weight = 1.0;
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
        anchor.size = Vec2::new(2.0 * TABLE_DEFAULT_COLUMN_WIDTH, 2.0 * TABLE_DEFAULT_ROW_HEIGHT);
        anchor.fills = vec![Paint::solid(Color::rgb(0xFB, 0xCF, 0xE8))];
    }
    for (i, text) in texts {
        if let Some(n) = nodes[1..].get_mut(*i) {
            if let NodeKind::TableCell(c) = &mut n.kind {
                c.text = default_cell_text(text);
            }
        }
    }
    if let Some(n) = nodes.get_mut(2) {
        n.fills = vec![Paint::solid(Color::rgb(0xBF, 0xDB, 0xFE))];
    }
    Command::Insert { parent: t.page, index: 0, nodes }.apply(&mut t.doc).unwrap();
    table_id
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

fn scene() -> T {
    let mut t = T::new();
    insert_table(&mut t, 3, 3, Vec2::new(20.0, 20.0), false, &[(0, "Tên"), (1, "Số lượng"), (4, "Việt Nam, những người bạn đồng hành thân thiết nhất")]);
    insert_table(&mut t, 3, 3, Vec2::new(20.0, 190.0), true, &[(0, "Gộp ô 2x2"), (1, "Điểm")]);
    t
}

#[test]
fn tables_match_the_cpu_reference() {
    let Some(mut g) = gpu() else { return };
    let t = scene();
    let c = compare(&mut g, "table_parity", &t.doc, t.page, Viewport::default(), SIZE, 8);
    assert!(c.report.ratio < 0.02, "{:?}", c.report);
    // parity of two blank pages proves nothing: both renders must hold the fills
    for (x, y, want) in [(330, 60, [0xBF, 0xDB, 0xFE]), (300, 270, [0xFB, 0xCF, 0xE8]), (330, 130, [0xF4, 0xF4, 0xF5])] {
        for (name, pm) in [("cpu", &c.cpu), ("gpu", &c.gpu)] {
            let p = pm.pixel(x, y).unwrap();
            let got = [p.red(), p.green(), p.blue()];
            assert!(got.iter().zip(want).all(|(a, b)| (*a as i32 - b).abs() <= 3), "{name} at ({x}, {y}): {got:?}, want {want:?}");
        }
    }
}

/// Cell (0, 0) of a table with no fill or grid holds ten lines in a 160 x 44 box: on both renderers
/// the text has ink in its cell and none outside it.
#[test]
fn overflowing_cell_text_stays_inside_its_cell_on_both_renderers() {
    let Some(mut g) = gpu() else { return };
    let mut t = T::new();
    let mut nodes = build_table(&mut t.doc, 2, 2, Vec2::new(20.0, 20.0));
    nodes[0].stroke.weight = 0.0;
    nodes[0].stroke.paints.clear();
    if let NodeKind::TableCell(c) = &mut nodes[1].kind {
        c.text = default_cell_text("W\nW\nW\nW\nW\nW\nW\nW\nW\nW");
    }
    Command::Insert { parent: t.page, index: 0, nodes }.apply(&mut t.doc).unwrap();
    let c = compare(&mut g, "table_overflow", &t.doc, t.page, Viewport::default(), (320, 200), 8);
    for (name, pm) in [("cpu", &c.cpu), ("gpu", &c.gpu)] {
        let (mut inside, mut outside) = (0, 0);
        for y in 0..200u32 {
            for x in 0..320u32 {
                let p = pm.pixel(x, y).unwrap();
                if p.red() >= 160 {
                    continue;
                }
                if (20..180).contains(&x) && (20..64).contains(&y) {
                    inside += 1;
                } else {
                    outside += 1;
                }
            }
        }
        assert!(inside > 20, "{name}: the text is drawn in its cell ({inside})");
        assert_eq!(outside, 0, "{name}: no ink outside the cell box");
    }
    assert!(c.report.ratio < 0.02, "{:?}", c.report);
}

/// A new revision of an unchanged 100x100 table compiles without shaping any text again.
#[test]
fn a_second_compile_of_an_unchanged_100x100_table_shapes_no_text() {
    let mut t = T::new();
    let mut nodes = build_table(&mut t.doc, 100, 100, Vec2::ZERO);
    for (i, c) in nodes.iter_mut().skip(1).enumerate() {
        if let NodeKind::TableCell(p) = &mut c.kind {
            p.text = default_cell_text(&format!("Cell {i}"));
        }
    }
    Command::Insert { parent: t.page, index: 0, nodes }.apply(&mut t.doc).unwrap();
    let vp = Viewport { offset: Vec2::ZERO, zoom: 0.1, dpr: 1.0 };
    let before = designos_render::shaping_runs();
    SceneCompiler::new().compile(&t.doc, t.page, 1, &vp, (1600, 440));
    let first = designos_render::shaping_runs() - before;
    assert_eq!(first, 10_000, "the first compile shapes each cell's text exactly once, for the fit check and the painter together");
    let before = designos_render::shaping_runs();
    SceneCompiler::new().compile(&t.doc, t.page, 2, &vp, (1600, 440));
    assert_eq!(designos_render::shaping_runs() - before, 0, "a new revision of the same table is not shaped again");
}

/// The same at 200 x 200 (40,000 texts, more than the layout cache holds) with one compiler across
/// revisions, as the canvas has: the fit check keeps its own line boxes, so a new revision shapes nothing.
#[test]
fn a_second_compile_of_an_unchanged_200x200_table_shapes_no_text() {
    let mut t = T::new();
    let mut nodes = build_table(&mut t.doc, 200, 200, Vec2::ZERO);
    for (i, c) in nodes.iter_mut().skip(1).enumerate() {
        if let NodeKind::TableCell(p) = &mut c.kind {
            p.text = default_cell_text(&format!("Cell {i}"));
        }
    }
    Command::Insert { parent: t.page, index: 0, nodes }.apply(&mut t.doc).unwrap();
    let vp = Viewport { offset: Vec2::ZERO, zoom: 0.05, dpr: 1.0 };
    let mut compiler = SceneCompiler::new();
    let before = designos_render::shaping_runs();
    compiler.compile(&t.doc, t.page, 1, &vp, (1600, 440));
    let first = designos_render::shaping_runs() - before;
    assert_eq!(first, 40_000, "the first compile shapes each cell's text exactly once");
    let before = designos_render::shaping_runs();
    compiler.compile(&t.doc, t.page, 2, &vp, (1600, 440));
    assert_eq!(designos_render::shaping_runs() - before, 0, "a new revision of the same table is not shaped again");
}

/// Offscreen programs of a 20x20 table turned 15 degrees, every cell holding `texts[0]` except cell 0
/// which holds `texts[1]`, compiled the interactive way, next to those of an empty page.
fn rotated_table_programs(texts: [&str; 2]) -> (usize, usize) {
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let interactive = || {
        let mut c = SceneCompiler::new();
        c.geom.budget = Some(Duration::from_millis(8));
        c
    };
    let t0 = T::new();
    let empty = interactive().compile(&t0.doc, t0.page, 1, &vp, (3000, 3000)).programs.len();
    let mut t = T::new();
    let mut nodes = build_table(&mut t.doc, 20, 20, Vec2::new(500.0, 100.0));
    nodes[0].transform = Affine { tx: 500.0, ty: 100.0, ..Affine::rotate(15f32.to_radians()) };
    nodes[0].fills = vec![Paint::solid(Color::WHITE)];
    nodes[0].stroke.paints = vec![Paint::solid(Color::BLACK)];
    nodes[0].stroke.weight = 1.0;
    for (i, n) in nodes.iter_mut().skip(1).enumerate() {
        n.size = Vec2::new(60.0, 30.0);
        n.transform = Affine::translate((i % 20) as f32 * 60.0, (i / 20) as f32 * 30.0);
        if let NodeKind::TableCell(c) = &mut n.kind {
            c.text = default_cell_text(texts[usize::from(i == 0)]);
            c.text.style.font_size = 8.0;
        }
    }
    Command::Insert { parent: t.page, index: 0, nodes }.apply(&mut t.doc).unwrap();
    let with = interactive().compile(&t.doc, t.page, 1, &vp, (3000, 3000)).programs.len();
    (empty, with)
}

/// A turned table whose text fits opens no program: its cells stay plain rectangles.
#[test]
fn a_rotated_table_with_fitting_text_opens_no_offscreen_program() {
    let (empty, with) = rotated_table_programs(["a", "a"]);
    assert_eq!(with, empty, "a rotated 20x20 table opened {with} programs, an empty page {empty}");
}

/// One cell whose text overflows clips, which costs that cell exactly one program.
#[test]
fn a_rotated_table_with_one_overflowing_cell_opens_exactly_one_program() {
    let (empty, with) = rotated_table_programs(["a", "a\na\na\na\na\na"]);
    assert_eq!(with, empty + 1, "one overflowing cell opened {} programs", with.saturating_sub(empty));
}

/// A 15 degree turned table with ten lines in its first cell and nothing else: on both renderers the
/// ink stays in that cell's turned box (1.5 px of anti-aliasing allowed) and the two agree.
#[test]
fn overflowing_text_of_a_rotated_cell_stays_inside_the_cell_on_both_renderers() {
    let Some(mut g) = gpu() else { return };
    let mut t = T::new();
    let mut nodes = build_table(&mut t.doc, 2, 2, Vec2::ZERO);
    let turn = Affine { tx: 120.0, ty: 20.0, ..Affine::rotate(15f32.to_radians()) };
    nodes[0].transform = turn;
    nodes[0].stroke.weight = 0.0;
    nodes[0].stroke.paints.clear();
    if let NodeKind::TableCell(c) = &mut nodes[1].kind {
        c.text = default_cell_text("W\nW\nW\nW\nW\nW\nW\nW\nW\nW");
    }
    Command::Insert { parent: t.page, index: 0, nodes }.apply(&mut t.doc).unwrap();
    let (w, h) = (520u32, 360u32);
    let c = compare(&mut g, "table_rotated_overflow", &t.doc, t.page, Viewport::default(), (w, h), 8);
    let back = turn.invert().expect("a turned table inverts");
    let (cw, ch) = (TABLE_DEFAULT_COLUMN_WIDTH, TABLE_DEFAULT_ROW_HEIGHT);
    for (name, pm) in [("cpu", &c.cpu), ("gpu", &c.gpu)] {
        let (mut inside, mut outside) = (0, 0);
        for y in 0..h {
            for x in 0..w {
                if pm.pixel(x, y).unwrap().red() >= 160 {
                    continue;
                }
                let l = back.apply(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
                if l.x >= -1.5 && l.x <= cw + 1.5 && l.y >= -1.5 && l.y <= ch + 1.5 {
                    inside += 1;
                } else {
                    outside += 1;
                }
            }
        }
        assert!(inside > 20, "{name}: the text is drawn ({inside})");
        assert_eq!(outside, 0, "{name}: no ink outside the turned cell");
    }
    assert!(c.report.ratio < 0.02, "{:?}", c.report);
}

/// The GPU compile on its own meets a registry one below its limit: the grid must still be compiled.
/// (In the pixel test above the CPU render runs first and its table lowering clears the registry.)
#[test]
fn gpu_compile_keeps_the_grid_near_the_registry_limit() {
    let mut t = T::new();
    let cells: Vec<(usize, &str)> = (0..9).map(|i| (i, "x")).collect();
    insert_table(&mut t, 3, 3, Vec2::new(20.0, 20.0), false, &cells);
    let vp = Viewport::default();
    let empty_page = T::new();
    let blank = SceneCompiler::new().compile(&empty_page.doc, empty_page.page, 1, &vp, (520, 160)).stats();
    // the near-limit compile goes first: the filler needs an empty registry to land one below the limit
    fill_registry(49_999);
    let near = SceneCompiler::new().compile(&t.doc, t.page, 1, &vp, (520, 160)).stats();
    assert!(
        board::get(&Document::new("x"), board::synth_id(NodeId(1_000_000), 0)).is_some(),
        "the draw ran one below the registry limit: the registry was not cleared before the table drew"
    );
    let fresh = SceneCompiler::new().compile(&t.doc, t.page, 1, &vp, (520, 160)).stats();
    assert!(fresh.triangles > blank.triangles, "the table draws: {} vs blank {}", fresh.triangles, blank.triangles);
    assert_eq!(near.triangles, fresh.triangles, "grid lost on the GPU path near the registry limit");
}

/// A page of blurred rectangles opens many programs; a table never adds one.
#[test]
fn a_large_table_opens_no_offscreen_program() {
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let interactive = || {
        let mut c = SceneCompiler::new();
        c.geom.budget = Some(Duration::from_millis(8));
        c
    };
    let empty = {
        let t = T::new();
        interactive().compile(&t.doc, t.page, 1, &vp, (1000, 1000)).programs.len()
    };
    let mut t = T::new();
    let nodes = build_table(&mut t.doc, 50, 50, Vec2::new(0.0, 0.0));
    let mut nodes = nodes;
    nodes[0].fills = vec![Paint::solid(Color::WHITE)];
    nodes[0].stroke.paints = vec![Paint::solid(Color::BLACK)];
    nodes[0].stroke.weight = 1.0;
    for n in nodes.iter_mut().skip(1) {
        if let NodeKind::TableCell(c) = &mut n.kind {
            c.text = default_cell_text("Ô");
        }
    }
    Command::Insert { parent: t.page, index: 0, nodes }.apply(&mut t.doc).unwrap();
    let compiled = interactive().compile(&t.doc, t.page, 1, &vp, (1000, 1000));
    let blank_draws = interactive().compile(&T::new().doc, T::new().page, 1, &vp, (1000, 1000)).stats().draws;
    assert!(compiled.stats().draws > blank_draws, "the table is drawn: {} draws vs blank {blank_draws}", compiled.stats().draws);
    let with_table = compiled.programs.len();
    assert_eq!(with_table, empty, "a 50x50 table opened {with_table} programs, an empty page {empty}");
}
