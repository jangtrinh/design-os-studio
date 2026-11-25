//! A vector's node `cornerRadius` rounds the vertices of its network that the style table gives no radius of
//! their own; a vertex the table sets to 0 stays sharp. Built like the kit's document icons: a rounded sheet whose
//! dog-ear corner is cut by two vertices with their own radius of 0.

use super::fixtures::*;
use super::geometry::{compare, truth, Clip, Verdict};
use super::raster::figma_paths;
use super::scratch::{live_nodes, Pass};
use super::synthetic::load;
use designos_doc::*;
use designos_fig::kiwi::KiwiValue as V;
use designos_fig::testkit::{fl, msg, solid};

/// The sheet `v1 -> v2 -> v0 -> v3 -> v4 -> v1`: `v3` and `v4` are the dog-ear and use style 1.
const SHEET: [(u32, f32, f32); 5] = [(0, 0.0, 0.0), (0, 52.0, 71.72), (0, 0.0, 71.72), (1, 40.0, 0.0), (1, 52.0, 12.0)];

fn sheet_network_blob() -> Vec<u8> {
    let segments = [(1u32, 2u32), (2, 0), (0, 3), (4, 1), (3, 4)];
    let mut b = Vec::new();
    for n in [SHEET.len(), segments.len(), 0] {
        b.extend((n as u32).to_le_bytes());
    }
    for (style, x, y) in SHEET {
        b.extend(style.to_le_bytes());
        b.extend(x.to_le_bytes());
        b.extend(y.to_le_bytes());
    }
    for (a, e) in segments {
        b.extend(0u32.to_le_bytes());
        b.extend(a.to_le_bytes());
        b.extend([0.0f32; 2].iter().flat_map(|f| f.to_le_bytes()));
        b.extend(e.to_le_bytes());
        b.extend([0.0f32; 2].iter().flat_map(|f| f.to_le_bytes()));
    }
    b
}

/// Figma's stored fill outline of the sheet with the node radius 4 on its three square corners and a sharp dog-ear.
fn sheet_outline_blob() -> Vec<u8> {
    let k = 4.0 * 0.552_284_8;
    let mut b = Vec::new();
    let mut op = |code: u8, pts: &[f32]| {
        b.push(code);
        b.extend(pts.iter().flat_map(|f| f.to_le_bytes()));
    };
    op(1, &[4.0, 71.72]);
    op(2, &[48.0, 71.72]);
    op(4, &[48.0 + k, 71.72, 52.0, 71.72 - 4.0 + k, 52.0, 67.72]);
    op(2, &[52.0, 12.0]);
    op(2, &[40.0, 0.0]);
    op(2, &[4.0, 0.0]);
    op(4, &[4.0 - k, 0.0, 0.0, 4.0 - k, 0.0, 4.0]);
    op(2, &[0.0, 67.72]);
    op(4, &[0.0, 71.72 - 4.0 + k, 4.0 - k, 71.72, 4.0, 71.72]);
    op(0, &[]);
    b
}

fn sheet_fig(node_radius: Option<f32>, style_radius: Option<f32>) -> Vec<u8> {
    let mut extra = vec![
        ("fillPaints", V::Array(vec![solid(0.0, 0.0, 0.0)])),
        ("fillGeometry", geometry(1)),
        (
            "vectorData",
            msg(vec![
                ("vectorNetworkBlob", V::Uint(0)),
                ("normalizedSize", designos_fig::testkit::vec2(52.0, 71.72)),
                (
                    "styleOverrideTable",
                    V::Array(vec![msg(vec![("styleID", V::Uint(1))].into_iter().chain(style_radius.map(|r| ("cornerRadius", fl(r)))).collect())]),
                ),
            ]),
        ),
    ];
    if let Some(r) = node_radius {
        extra.push(("cornerRadius", fl(r)));
    }
    let sheet = layer(1, (0, 1), "!", "VECTOR", (52.0, 71.72), (0.0, 0.0), extra);
    fig(vec![sheet], vec![sheet_network_blob(), sheet_outline_blob()])
}

fn radii(doc: &Document, id: NodeId) -> Vec<f32> {
    let NodeKind::Vector { network: Some(net), .. } = &doc.get(id).unwrap().kind else { panic!("a vector with a network") };
    net.vertices.iter().map(|v| v.corner_radius).collect()
}

/// Nudges the first vertex by 0.25 px, as a vertex edit does, and compares the live fill with Figma's stored one.
fn fill_after_a_nudge(bytes: &[u8]) -> Verdict {
    let (r, mut doc, keys) = load(bytes);
    let id = keys.by_source["1:1"];
    if let NodeKind::Vector { network: Some(net), .. } = &mut doc.get_mut(id).unwrap().kind {
        net.vertices[0].pos.x += 0.25;
    }
    let nodes = live_nodes(&doc, id).expect("a live outline");
    let t = truth(&r).into_iter().find(|t| t.guid == "1:1").expect("stored outline");
    compare(&nodes, Pass::Fill, &figma_paths(&t.fill, &r.blobs), Clip::None)
}

#[test]
fn the_node_radius_rounds_every_vertex_the_style_table_leaves_alone() {
    let (_, doc, keys) = load(&sheet_fig(Some(4.0), Some(0.0)));
    assert_eq!(radii(&doc, keys.by_source["1:1"]), vec![4.0, 4.0, 4.0, 0.0, 0.0], "the dog-ear vertices set their own radius of 0");
}

#[test]
fn a_radius_of_the_style_table_wins_over_the_node_radius() {
    let (_, doc, keys) = load(&sheet_fig(Some(4.0), Some(2.0)));
    assert_eq!(radii(&doc, keys.by_source["1:1"]), vec![4.0, 4.0, 4.0, 2.0, 2.0]);
}

#[test]
fn a_vector_without_a_node_radius_keeps_its_vertices_sharp() {
    let (_, doc, keys) = load(&sheet_fig(None, Some(0.0)));
    assert_eq!(radii(&doc, keys.by_source["1:1"]), vec![0.0; 5]);
}

#[test]
fn a_rounded_sheet_fills_like_its_stored_outline_after_a_vertex_nudge() {
    assert!(matches!(fill_after_a_nudge(&sheet_fig(Some(4.0), Some(0.0))), Verdict::Match));
}

#[test]
fn a_sheet_with_square_corners_does_not_fill_like_the_rounded_outline() {
    let v = fill_after_a_nudge(&sheet_fig(None, Some(0.0)));
    assert!(matches!(v, Verdict::Mismatch(_)), "the comparison tells a 4 px corner radius from none");
}
