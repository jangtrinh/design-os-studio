//! Exporting tables: SVG, PNG and PDF carry the cells, their fills and text, and the grid.

#[allow(dead_code)]
mod pdf_probe;

use designos_doc::*;
use designos_export::{export_pdf, export_png, export_svg, render_export, SvgOptions};
use pdf_probe::Pdf;

const WORDS: [&str; 4] = ["Tên", "Số lượng", "Việt Nam", "Gộp"];

/// A frame at (0, 0) holding a 3x3 table. The table takes the highest node id, so a walk of the
/// document's nodes meets the cells before their table. Cell (0, 0) is red, the others have no fill.
fn framed_table() -> (Document, NodeId, NodeId, Vec<NodeId>) {
    let mut doc = Document::new("tables");
    let page = doc.add_page("p");
    let frame_id = doc.alloc_id();
    let mut frame = Node::frame(frame_id, Rect::new(0.0, 0.0, 520.0, 200.0));
    frame.fills = vec![Paint::solid(Color::WHITE)];
    doc.insert(page, frame).unwrap();
    let mut nodes = build_table(&mut doc, 3, 3, Vec2::new(20.0, 20.0));
    let fresh: Vec<NodeId> = (0..nodes.len()).map(|_| doc.alloc_id()).collect();
    let map: std::collections::HashMap<NodeId, NodeId> = nodes[1..].iter().map(|n| n.id).chain([nodes[0].id]).zip(fresh.iter().copied()).collect();
    for n in &mut nodes {
        n.id = map[&n.id];
        n.parent = n.parent.map(|p| map[&p]);
        n.children = n.children.iter().map(|c| map[c]).collect();
    }
    nodes[0].fills = vec![Paint::solid(Color::rgb(0xEE, 0xEE, 0xEE))];
    nodes[0].stroke.paints = vec![Paint::solid(Color::BLACK)];
    nodes[0].stroke.weight = 2.0;
    nodes[1].fills = vec![Paint::solid(Color::rgb(255, 0, 0))];
    for (i, w) in WORDS.iter().enumerate() {
        if let NodeKind::TableCell(c) = &mut nodes[1 + i].kind {
            c.text = default_cell_text(w);
        }
    }
    let table = nodes[0].id;
    let cells: Vec<NodeId> = nodes[1..].iter().map(|n| n.id).collect();
    Command::Insert { parent: frame_id, index: 0, nodes }.apply(&mut doc).unwrap();
    (doc, frame_id, table, cells)
}

#[test]
fn svg_has_every_cell_text_and_one_grid_path() {
    let (doc, frame, table, cells) = framed_table();
    assert!(cells.iter().all(|c| c.0 < table.0));
    for node in [frame, table] {
        let svg = export_svg(&doc, node, &SvgOptions { outline_text: false });
        for w in WORDS {
            assert!(svg.contains(w), "the SVG of {node:?} has the cell text {w:?}:\n{svg}");
        }
        assert!(svg.contains("fill=\"#FF0000\""), "the red cell fill is there");
        // the grid is one stroked path in the table's stroke colour and weight
        let grid = svg.lines().filter(|l| l.contains("<path") && l.contains("stroke=\"#000000\"") && l.contains("stroke-width=\"2\"")).count();
        assert_eq!(grid, 1, "one stroked grid path in the SVG of {node:?}:\n{svg}");
    }
}

#[test]
fn png_shows_the_cell_fill_and_the_grid() {
    let (doc, frame, ..) = framed_table();
    let png = export_png(&doc, frame, 1.0);
    assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
    let pm = render_export(&doc, frame, 1.0).unwrap();
    let px = |x: u32, y: u32| {
        let p = pm.pixel(x, y).unwrap();
        [p.red(), p.green(), p.blue()]
    };
    assert_eq!(px(150, 60), [255, 0, 0], "centre of cell (0, 0)");
    assert_eq!(px(250, 80), [0xEE, 0xEE, 0xEE], "an unfilled cell shows the table fill");
    assert_eq!(px(180, 130), [0, 0, 0], "a grid line");
    assert_eq!(px(5, 5), [255, 255, 255], "the frame around the table");
}

#[test]
fn png_of_the_table_node_has_its_cells_and_outer_border() {
    let (doc, _frame, table, _) = framed_table();
    let pm = render_export(&doc, table, 1.0).expect("a table exports on its own");
    assert!(pm.width() >= 480 && pm.height() >= 132, "{}x{}", pm.width(), pm.height());
    let dark = |x: u32, y: u32| {
        let p = pm.pixel(x, y).unwrap();
        p.red() < 60 && p.green() < 60 && p.blue() < 60 && p.alpha() > 200
    };
    // the export is tight to the table: find its left and top border from the middle row and column
    let row = pm.height() / 2;
    let col = pm.width() / 2;
    assert!((0..6).any(|x| dark(x, row)), "left border is inside the exported image");
    assert!((0..6).any(|y| dark(col, y)), "top border is inside the exported image");
    assert!((pm.width() - 6..pm.width()).any(|x| dark(x, row)), "right border is inside the exported image");
    assert!((pm.height() - 6..pm.height()).any(|y| dark(col, y)), "bottom border is inside the exported image");
}

#[test]
fn pdf_page_has_the_cell_text() {
    let (doc, frame, ..) = framed_table();
    let bytes = export_pdf(&doc, &[frame]).expect("a frame holding a table exports");
    let pdf = Pdf::parse(&bytes);
    let text = pdf.pages().iter().flat_map(|p| pdf.text_runs(p)).collect::<Vec<_>>().concat();
    for w in WORDS {
        assert!(text.contains(w), "the PDF has {w:?}, found {text:?}");
    }
}

const BORDER: f32 = 8.0;

/// A 3x3 table (480 x 132) alone on its page with an 8 px black border and white cells.
fn bordered_table() -> (Document, NodeId) {
    let mut doc = Document::new("bordered");
    let page = doc.add_page("p");
    let mut nodes = build_table(&mut doc, 3, 3, Vec2::new(40.0, 40.0));
    nodes[0].fills = vec![Paint::solid(Color::WHITE)];
    nodes[0].stroke.paints = vec![Paint::solid(Color::BLACK)];
    nodes[0].stroke.weight = BORDER;
    let table = nodes[0].id;
    Command::Insert { parent: page, index: 0, nodes }.apply(&mut doc).unwrap();
    (doc, table)
}

/// The border weight is 8 px, so a tight export is 8 px wider and taller than the 480 x 132 table.
const EXPORT_W: u32 = 488;
const EXPORT_H: u32 = 140;

#[test]
fn png_of_a_table_keeps_the_full_border_weight_on_every_side() {
    let (doc, table) = bordered_table();
    let pm = render_export(&doc, table, 1.0).expect("a table exports on its own");
    assert_eq!((pm.width(), pm.height()), (EXPORT_W, EXPORT_H));
    let dark = |x: u32, y: u32| {
        let p = pm.pixel(x, y).unwrap();
        p.red() < 60 && p.green() < 60 && p.blue() < 60 && p.alpha() > 200
    };
    let (row, col) = (EXPORT_H / 2, EXPORT_W / 2);
    for i in 0..8 {
        assert!(dark(i, row), "left border pixel {i}");
        assert!(dark(EXPORT_W - 1 - i, row), "right border pixel {i}");
        assert!(dark(col, i), "top border pixel {i}");
        assert!(dark(col, EXPORT_H - 1 - i), "bottom border pixel {i}");
    }
}

#[test]
fn svg_of_a_table_keeps_the_full_border_weight() {
    let (doc, table) = bordered_table();
    let svg = export_svg(&doc, table, &SvgOptions { outline_text: false });
    assert!(svg.contains("viewBox=\"-4 -4 488 140\""), "the view box holds the whole border:\n{svg}");
}

#[test]
fn pdf_of_a_table_keeps_the_full_border_weight_on_every_side() {
    let (doc, table) = bordered_table();
    let bytes = export_pdf(&doc, &[table]).expect("a table exports as a page");
    let pdf = Pdf::parse(&bytes);
    let page = &pdf.pages()[0];
    assert_eq!((page.width, page.height), (EXPORT_W as f32 * 0.75, EXPORT_H as f32 * 0.75), "the page holds the whole border");
    let (_, px) = pdf_probe::page_pixels(&bytes, 0, 2.0);
    let (w, h) = (EXPORT_W as f32, EXPORT_H as f32);
    for i in 0..8 {
        let d = i as f32 + 0.5;
        for (name, [x, y]) in [("left", [d, h / 2.0]), ("right", [w - d, h / 2.0]), ("top", [w / 2.0, d]), ("bottom", [w / 2.0, h - d])] {
            assert!(px.at(x, y).iter().all(|c| *c < 60), "{name} border pixel {i}: {:?}", px.at(x, y));
        }
    }
}

#[test]
fn pdf_and_png_read_a_variable_bound_border_weight_alike() {
    let (mut doc, table) = bordered_table();
    let weight = Variable {
        id: "bw".into(),
        name: "bw".into(),
        collection: "c".into(),
        kind: VariableKind::Number,
        values: [("light".to_string(), VariableValue::Number(8.0))].into_iter().collect(),
        description: String::new(),
        scopes: vec![],
        hidden_from_publishing: false,
        key: None,
        code_syntax: Default::default(),
        extensions: Default::default(),
        mode_extensions: Default::default(),
    };
    doc.variable_collections.insert(
        "c".into(),
        VariableCollection {
            id: "c".into(),
            name: "C".into(),
            modes: vec![VariableMode { id: "light".into(), name: "Light".into() }],
            default_mode: "light".into(),
            variables: vec![],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    doc.variables.insert(weight.id.clone(), weight);
    let n = doc.get_mut(table).unwrap();
    n.stroke.weight = 1.0;
    n.variables.bindings.insert("strokeWeight".into(), "bw".into());
    let pm = render_export(&doc, table, 1.0).unwrap();
    assert_eq!((pm.width(), pm.height()), (EXPORT_W, EXPORT_H), "PNG takes the bound weight of 8");
    let bytes = export_pdf(&doc, &[table]).unwrap();
    let page = &Pdf::parse(&bytes).pages()[0];
    assert_eq!((page.width, page.height), (EXPORT_W as f32 * 0.75, EXPORT_H as f32 * 0.75), "PDF takes the same weight");
}

#[test]
fn pdf_size_limit_counts_the_border_that_hangs_outside_the_table() {
    let (mut doc, table) = bordered_table();
    doc.get_mut(table).unwrap().size.x = 19_200.0;
    let err = export_pdf(&doc, &[table]).expect_err("the page is 19,208 px wide");
    assert!(err.contains("19,200"), "{err}");
    doc.get_mut(table).unwrap().size.x = 19_190.0;
    assert!(export_pdf(&doc, &[table]).is_ok(), "19,198 px fits");
}

#[test]
fn a_frame_with_an_inside_border_exports_at_its_own_size() {
    let mut doc = Document::new("frame");
    let page = doc.add_page("p");
    let id = doc.alloc_id();
    let mut frame = Node::frame(id, Rect::new(0.0, 0.0, 120.0, 60.0));
    frame.fills = vec![Paint::solid(Color::WHITE)];
    frame.stroke.paints = vec![Paint::solid(Color::BLACK)];
    frame.stroke.weight = BORDER;
    doc.insert(page, frame).unwrap();
    let pm = render_export(&doc, id, 1.0).unwrap();
    assert_eq!((pm.width(), pm.height()), (120, 60), "a non-table node keeps its bounds");
    assert!(export_svg(&doc, id, &SvgOptions { outline_text: false }).contains("viewBox=\"0 0 120 60\""));
}

/// Writes the exports the visual evidence is captured from. Set `DESIGNOS_TABLE_EVIDENCE_DIR` and run
/// `cargo test -p designos-export --test table_export -- --ignored write_evidence_exports`.
#[test]
#[ignore = "writes export files for screenshots"]
fn write_evidence_exports() {
    let Some(dir) = std::env::var_os("DESIGNOS_TABLE_EVIDENCE_DIR") else { return };
    let dir = std::path::Path::new(&dir);
    let (doc, frame, ..) = framed_table();
    std::fs::write(dir.join("table-export.png"), export_png(&doc, frame, 2.0)).unwrap();
    std::fs::write(dir.join("table-export.svg"), export_svg(&doc, frame, &SvgOptions::default())).unwrap();
    std::fs::write(dir.join("table-export.pdf"), export_pdf(&doc, &[frame]).unwrap()).unwrap();
}
