//! A vector resized inside an instance on a page the loader has not opened.
//!
//! The save keeps the record the file states for the instance (the size override of the vector) without opening the
//! page, and the page, once opened after a reopen, shows the vector at the size the record states.

mod common;

use common::deferred_fig::*;
use common::fig_fixture::*;
use designos_doc::*;
use designos_fig::{export_fig, import_fig};
use serde_json::json;

const GLYPH: (u64, u64) = (1, 13);

/// Closed square outline of side `side` in the `commandsBlob` encoding: op byte then little-endian `f32` points.
fn square_blob(side: f32) -> Vec<u8> {
    let mut b = Vec::new();
    for (op, pts) in [(1u8, [0.0, 0.0]), (2, [side, 0.0]), (2, [side, side]), (2, [0.0, side])] {
        b.push(op);
        for p in pts {
            b.extend_from_slice(&f32::to_le_bytes(p));
        }
    }
    b.push(0);
    b
}

fn file_with_resized_glyph(width: f32, height: f32) -> Vec<u8> {
    let glyph = fig_node(
        GLYPH,
        BUTTON,
        "VECTOR",
        "Glyph",
        json!({"size": {"x": 100.0, "y": 100.0}, "fillPaints": [solid(&rgba(1.0, 0.0, 0.0), None)], "fillGeometry": [{"windingRule": "NONZERO", "commandsBlob": 0}]}),
    );
    let overrides = json!([{"guidPath": {"guids": [guid(GLYPH.0, GLYPH.1)]}, "size": {"x": width, "y": height}}]);
    fixture(overrides, vec![glyph], vec![square_blob(100.0)])
}

fn glyph_of(doc: &Document, owner: NodeId) -> &Node {
    doc.get(owner).expect("owner").children.iter().filter_map(|&c| doc.get(c)).find(|n| n.name == "Glyph").expect("Glyph")
}

fn outline_extent(n: &Node) -> (f32, f32) {
    let NodeKind::Vector { paths, .. } = &n.kind else { panic!("Glyph is {:?}", n.kind) };
    let pts: Vec<Vec2> = paths
        .iter()
        .flat_map(|p| p.cmds.iter())
        .filter_map(|c| match c {
            PathCmd::MoveTo(p) | PathCmd::LineTo(p) => Some(*p),
            _ => None,
        })
        .collect();
    let span = |f: fn(&Vec2) -> f32| pts.iter().map(f).fold(f32::MIN, f32::max) - pts.iter().map(f).fold(f32::MAX, f32::min);
    (span(|p| p.x), span(|p| p.y))
}

fn check_resize(width: f32, height: f32) {
    let fig = file_with_resized_glyph(width, height);
    let original = saved_entries(&fig, INSTANCE);
    assert_eq!(entry_at(&original, GLYPH)["size"], json!({"x": width, "y": height}), "the fixture states the resize");

    let live = import_fig(&fig).expect("import");
    let page = page_named(&live, "404 pages");
    assert!(!page_hydrated(&live, page), "the importer defers the page");
    let saved = export_fig(&live).expect("export");

    assert!(!page_hydrated(&live, page), "the save does not open the page");
    assert_eq!(children_written_under(&saved, INSTANCE), 0, "no children are written for the unopened instance");
    let entries = saved_entries(&saved, INSTANCE);
    assert_eq!(entry_at(&entries, GLYPH)["size"], entry_at(&original, GLYPH)["size"], "the record of the vector keeps its size");

    let mut reopened = import_fig(&saved).expect("reopen");
    reopened.hydrate_all_pages();
    let glyph = glyph_of(&reopened, variant_button(&reopened));
    let (w, h) = outline_extent(glyph);
    assert!((w - width).abs() <= 0.5 && (h - height).abs() <= 0.5, "the vector in the instance is {width} x {height} after the reopen, got {w} x {h}");
    assert_eq!(glyph.size, Vec2::new(width, height));
    let main = reopened.nodes.values().find(|n| n.name == "Buttons/Button" && matches!(n.kind, NodeKind::Component(_))).expect("main").id;
    let (mw, mh) = outline_extent(glyph_of(&reopened, main));
    assert!((mw - 100.0).abs() <= 0.5 && (mh - 100.0).abs() <= 0.5, "the main component's vector keeps its own size, got {mw} x {mh}");
}

#[test]
fn a_save_keeps_the_width_record_of_a_vector_in_an_unopened_instance() {
    check_resize(250.0, 100.0);
}

#[test]
fn a_save_keeps_the_height_record_of_a_vector_in_an_unopened_instance() {
    check_resize(100.0, 40.0);
}

#[test]
fn a_save_keeps_a_record_that_scales_a_vector_by_different_factors_in_an_unopened_instance() {
    check_resize(250.0, 40.0);
}
