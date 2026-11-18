//! Tiny synthetic `.fig` files for the scoreboard's own tests: each holds one known deviation or none.

use designos_fig::container::{self, Compression};
use designos_fig::kiwi::KiwiValue as V;
use designos_fig::testkit::{en, figma_like_schema, fl, guid, matrix, msg, node, st, vec2};

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

pub fn guid_path(ids: &[u32]) -> V {
    msg(vec![("guids", V::Array(ids.iter().map(|i| guid(1, *i)).collect()))])
}

/// A horizontal auto-layout frame (gap 10) holding two 20 x 20 rectangles; the second sits at `second_x`.
pub fn auto_layout_fig(second_x: f32) -> Vec<u8> {
    let frame = layer(1, (0, 1), "!", "FRAME", (100.0, 20.0), (0.0, 0.0), vec![("stackMode", en("HORIZONTAL")), ("stackSpacing", fl(10.0))]);
    let a = layer(2, (1, 1), "!", "RECTANGLE", (20.0, 20.0), (0.0, 0.0), vec![]);
    let b = layer(3, (1, 1), "\"", "RECTANGLE", (20.0, 20.0), (second_x, 0.0), vec![]);
    fig(vec![frame, a, b], vec![])
}

/// A rectangle of `size` whose stored fill outline is the 10 x 10 square of blob 0.
pub fn rectangle_fig(size: f32) -> Vec<u8> {
    let r = layer(2, (0, 1), "!", "RECTANGLE", (size, size), (0.0, 0.0), vec![("fillGeometry", geometry(0))]);
    fig(vec![r], vec![designos_fig::testkit::square_blob()])
}

/// Commands blob of the band Figma stores for a weight-1 INSIDE stroke of a 10 x 10 box: the centred band of
/// twice the weight, from -1 to 11 outside and 1 to 9 inside.
pub fn double_band_blob() -> Vec<u8> {
    let mut b = Vec::new();
    for ring in [[(-1.0f32, -1.0f32), (11.0, -1.0), (11.0, 11.0), (-1.0, 11.0)], [(1.0, 1.0), (9.0, 1.0), (9.0, 9.0), (1.0, 9.0)]] {
        for (i, (x, y)) in ring.iter().enumerate() {
            b.push(if i == 0 { 1u8 } else { 2 });
            b.extend(x.to_le_bytes());
            b.extend(y.to_le_bytes());
        }
        b.push(0);
    }
    b
}

/// A 10 x 10 rectangle with an INSIDE stroke of `weight`, stored as the band of a weight-1 stroke.
pub fn inside_stroke_fig(weight: f32) -> Vec<u8> {
    let stroke = V::Array(vec![msg(vec![("windingRule", en("ODD")), ("commandsBlob", V::Uint(1)), ("styleID", V::Uint(0))])]);
    let r = layer(
        2,
        (0, 1),
        "!",
        "RECTANGLE",
        (10.0, 10.0),
        (0.0, 0.0),
        vec![("strokeWeight", fl(weight)), ("strokeAlign", en("INSIDE")), ("fillGeometry", geometry(0)), ("strokeGeometry", stroke)],
    );
    fig(vec![r], vec![designos_fig::testkit::square_blob(), double_band_blob()])
}

/// A main (100 x 40, one rectangle) and an instance of it whose derived data restates the rectangle.
pub fn instance_fig() -> Vec<u8> {
    let main = layer(10, (0, 1), "!", "SYMBOL", (100.0, 40.0), (0.0, 0.0), vec![]);
    let rect = layer(11, (1, 10), "!", "RECTANGLE", (100.0, 40.0), (0.0, 0.0), vec![]);
    let derived = msg(vec![("guidPath", guid_path(&[11])), ("size", vec2(100.0, 40.0)), ("transform", matrix(0.0, 0.0))]);
    let inst = layer(
        20,
        (0, 1),
        "\"",
        "INSTANCE",
        (100.0, 40.0),
        (0.0, 200.0),
        vec![("symbolData", msg(vec![("symbolID", guid(1, 10))])), ("derivedSymbolData", V::Array(vec![derived]))],
    );
    fig(vec![main, rect, inst], vec![])
}

/// A text layer with stored derived text data: one line starting at character 0, baseline at `y`.
pub fn text_fig(y: f32) -> Vec<u8> {
    let font = st(vec![("family", V::String("Inter".into())), ("style", V::String("Regular".into())), ("postscript", V::String(String::new()))]);
    let derived = msg(vec![
        ("layoutSize", vec2(80.0, 20.0)),
        ("baselines", V::Array(vec![msg(vec![("position", vec2(0.0, y)), ("width", fl(80.0)), ("firstCharacter", V::Uint(0)), ("endCharacter", V::Uint(5))])])),
    ]);
    let t = layer(
        30,
        (0, 1),
        "!",
        "TEXT",
        (80.0, 20.0),
        (0.0, 0.0),
        vec![
            ("textData", msg(vec![("characters", V::String("Hello".into()))])),
            ("fontName", font),
            ("fontSize", fl(16.0)),
            ("textAutoResize", en("WIDTH_AND_HEIGHT")),
            ("derivedTextData", derived),
        ],
    );
    fig(vec![t], vec![])
}

/// A frame carrying the string field `version` (not in the importer's field list): kept as metadata when it has a
/// value, dropped when it is empty.
pub fn version_fig(value: &str) -> Vec<u8> {
    fig(vec![layer(40, (0, 1), "!", "FRAME", (10.0, 10.0), (0.0, 0.0), vec![("version", V::String(value.into()))])], vec![])
}
