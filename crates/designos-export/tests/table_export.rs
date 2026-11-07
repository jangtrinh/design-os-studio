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
fn pdf_page_has_the_cell_text() {
    let (doc, frame, ..) = framed_table();
    let bytes = export_pdf(&doc, &[frame]).expect("a frame holding a table exports");
    let pdf = Pdf::parse(&bytes);
    let text = pdf.pages().iter().flat_map(|p| pdf.text_runs(p)).collect::<Vec<_>>().concat();
    for w in WORDS {
        assert!(text.contains(w), "the PDF has {w:?}, found {text:?}");
    }
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
