//! Tiny synthetic `.fig` files for the scoreboard's own tests: each holds one known deviation or none.

use designos_fig::container::{self, Compression};
use designos_fig::kiwi::KiwiValue as V;
use designos_fig::testkit::{en, figma_like_schema, fl, matrix, msg, node, st, vec2};

pub fn fig(mut changes: Vec<V>, blobs: Vec<Vec<u8>>) -> Vec<u8> {
    let mut all = vec![node((0, 0), (0, 0), "", "DOCUMENT", "Document", vec![]), node((0, 1), (0, 0), "!", "CANVAS", "Page", vec![])];
    all.append(&mut changes);
    let schema = figma_like_schema();
    let root = schema.find("Message").expect("Message");
    let blobs = V::Array(blobs.into_iter().map(|b| st(vec![("bytes", V::Bytes(b))])).collect());
    let body = schema.encode_message(root, &msg(vec![("nodeChanges", V::Array(all)), ("blobs", blobs)])).expect("encode");
    container::write_container(&schema.encode(), &body, Compression::Zstd, 106)
}

/// A layer on the page (or under `parent`) with a size and a position.
pub fn layer(id: u32, parent: (u32, u32), pos: &str, ty: &str, size: (f32, f32), at: (f32, f32), mut extra: Vec<(&str, V)>) -> V {
    let mut f = vec![("size", vec2(size.0, size.1)), ("transform", matrix(at.0, at.1)), ("visible", V::Bool(true)), ("opacity", fl(1.0))];
    f.append(&mut extra);
    node((1, id), parent, pos, ty, "Layer", f)
}

pub fn geometry(blob: u32) -> V {
    V::Array(vec![msg(vec![("windingRule", en("NONZERO")), ("commandsBlob", V::Uint(blob)), ("styleID", V::Uint(0))])])
}

/// A horizontal auto-layout frame (gap 10) holding two 20 x 20 rectangles; the second sits at `second_x`.
pub fn auto_layout_fig(second_x: f32) -> Vec<u8> {
    let frame = layer(1, (0, 1), "!", "FRAME", (100.0, 20.0), (0.0, 0.0), vec![("stackMode", en("HORIZONTAL")), ("stackSpacing", fl(10.0))]);
    let a = layer(2, (1, 1), "!", "RECTANGLE", (20.0, 20.0), (0.0, 0.0), vec![]);
    let b = layer(3, (1, 1), "\"", "RECTANGLE", (20.0, 20.0), (second_x, 0.0), vec![]);
    fig(vec![frame, a, b], vec![])
}

/// A frame carrying the string field `version` (not in the importer's field list): kept as metadata when it has a
/// value, dropped when it is empty.
pub fn version_fig(value: &str) -> Vec<u8> {
    fig(vec![layer(40, (0, 1), "!", "FRAME", (10.0, 10.0), (0.0, 0.0), vec![("version", V::String(value.into()))])], vec![])
}
