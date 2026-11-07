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

#[test]
fn grid_survives_a_registry_clear_on_the_gpu_path() {
    let Some(mut g) = gpu() else { return };
    let mut t = T::new();
    let table = insert_table(&mut t, 3, 3, Vec2::new(20.0, 20.0), false, &[(0, "a"), (1, "b"), (2, "c"), (3, "d"), (4, "e"), (5, "f")]);
    let _ = table;
    board::fill_registry_for_tests(49_999);
    let c = compare(&mut g, "table_registry_clear", &t.doc, t.page, Viewport::default(), (520, 160), 8);
    // a 1 px line centred on an integer coordinate covers half of two pixels: the darker one is the line
    let darkest = |pm: &designos_render::tiny_skia::Pixmap, x: u32, y: u32, dx: u32, dy: u32| {
        (0..2).map(|k| pm.pixel(x - k * dx, y - k * dy).unwrap().red()).min().unwrap()
    };
    for (x, y, dx, dy) in [(180, 40, 1, 0), (340, 40, 1, 0), (100, 64, 0, 1), (100, 108, 0, 1)] {
        for (name, pm) in [("cpu", &c.cpu), ("gpu", &c.gpu)] {
            let d = darkest(pm, x, y, dx, dy);
            assert!(d < 0xF4 - 60, "{name} grid line at ({x}, {y}): red {d}");
        }
    }
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
    let with_table = interactive().compile(&t.doc, t.page, 1, &vp, (1000, 1000)).programs.len();
    assert_eq!(with_table, empty, "a 50x50 table opened {with_table} programs, an empty page {empty}");
}
