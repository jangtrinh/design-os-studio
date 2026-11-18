//! Synthetic `.fig` files whose text layers carry Figma's embedded glyph outlines for a font that is not installed.
//! Shared by the fig, render, export and MCP tests through `#[path]`.
#![allow(dead_code)]

use designos_fig::container::{self, Compression};
use designos_fig::kiwi::KiwiValue as V;
use designos_fig::testkit::{figma_like_schema, fl, guid, msg, node, solid, st, vec2};

pub const FAMILY: &str = "MissingIconFaceForTest";

/// A closed triangle in font-em space: M(0,0) L(1,0) L(0.5,0.8) Z.
pub fn icon_blob() -> Vec<u8> {
    let mut blob = Vec::new();
    for (op, x, y) in [(1, 0.0f32, 0.0f32), (2, 1.0, 0.0), (2, 0.5, 0.8)] {
        blob.push(op);
        blob.extend(x.to_le_bytes());
        blob.extend(y.to_le_bytes());
    }
    blob.push(0);
    blob
}

pub fn font_key(family: &str) -> V {
    st(vec![("family", V::String(family.into())), ("style", V::String("Regular".into())), ("postscript", V::String(String::new()))])
}

/// A 20x20 text layer in `family` whose derived data holds one glyph per `(blob index, x)` pair, baseline at y = 20.
#[allow(clippy::too_many_arguments)]
pub fn glyph_text(
    id: (u32, u32),
    parent: (u32, u32),
    pos: &str,
    name: &str,
    family: &str,
    characters: &str,
    glyphs: &[(u32, f32)],
    mut extra: Vec<(&str, V)>,
) -> V {
    let glyphs = glyphs.iter().map(|&(blob, x)| msg(vec![("commandsBlob", V::Uint(blob)), ("position", vec2(x, 20.0)), ("fontSize", fl(20.0))])).collect();
    let mut fields = vec![
        ("size", vec2(20.0, 20.0)),
        ("fontSize", fl(20.0)),
        ("fontName", font_key(family)),
        ("fillPaints", V::Array(vec![solid(0.0, 0.0, 0.0)])),
        ("textData", msg(vec![("characters", V::String(characters.into()))])),
        (
            "derivedTextData",
            msg(vec![("layoutSize", vec2(20.0, 20.0)), ("fontMetaData", V::Array(vec![msg(vec![("key", font_key(family))])])), ("glyphs", V::Array(glyphs))]),
        ),
    ];
    fields.append(&mut extra);
    node(id, parent, pos, "TEXT", name, fields)
}

pub fn fig_file(nodes: Vec<V>, blobs: Vec<Vec<u8>>) -> Vec<u8> {
    let schema = figma_like_schema();
    let blobs = blobs.into_iter().map(|b| st(vec![("bytes", V::Bytes(b))])).collect();
    let body = schema.encode_message(schema.find("Message").unwrap(), &msg(vec![("nodeChanges", V::Array(nodes)), ("blobs", V::Array(blobs))])).unwrap();
    container::write_container(&schema.encode(), &body, Compression::Zstd, 161)
}

pub fn document_and_page() -> Vec<V> {
    vec![node((0, 0), (0, 0), "!", "DOCUMENT", "Document", vec![]), node((0, 1), (0, 0), "a", "CANVAS", "Page", vec![])]
}

/// A frame holding the text layer `Icon text` with one triangle glyph.
pub fn icon_fixture(family: &str) -> Vec<u8> {
    let mut nodes = document_and_page();
    nodes.push(node((1, 1), (0, 1), "a", "FRAME", "Clipping frame", vec![("size", vec2(24.0, 24.0))]));
    nodes.push(glyph_text((1, 2), (1, 1), "a", "Icon text", family, "\u{e900}", &[(0, 0.0)], vec![]));
    fig_file(nodes, vec![icon_blob()])
}

/// A main component whose text carries the glyph, and an instance that overrides the characters while keeping the
/// component's box.
pub fn instance_override_fixture() -> Vec<u8> {
    let mut nodes = document_and_page();
    nodes.push(node((1, 10), (0, 1), "a", "SYMBOL", "Icon main", vec![("size", vec2(20.0, 20.0))]));
    nodes.push(glyph_text((1, 11), (1, 10), "a", "Glyph source", FAMILY, "\u{e900}", &[(0, 0.0)], vec![]));
    nodes.push(node(
        (1, 20),
        (0, 1),
        "b",
        "INSTANCE",
        "Icon instance",
        vec![
            ("size", vec2(20.0, 20.0)),
            (
                "symbolData",
                msg(vec![
                    ("symbolID", guid(1, 10)),
                    (
                        "symbolOverrides",
                        V::Array(vec![msg(vec![
                            ("guidPath", msg(vec![("guids", V::Array(vec![guid(1, 11)]))])),
                            ("textData", msg(vec![("characters", V::String("\u{e901}".into()))])),
                        ])]),
                    ),
                ]),
            ),
        ],
    ));
    fig_file(nodes, vec![icon_blob()])
}

/// A vector that writes a blob of its own before a two-glyph text whose glyphs share one blob. The glyph blob is
/// number 0 in the file and lands behind the vector's blob in a saved file.
pub fn shared_blob_fixture() -> Vec<u8> {
    let mut nodes = document_and_page();
    nodes.push(node(
        (1, 1),
        (0, 1),
        "a",
        "VECTOR",
        "Square",
        vec![
            ("size", vec2(10.0, 10.0)),
            ("fillPaints", V::Array(vec![solid(0.0, 0.0, 0.0)])),
            ("fillGeometry", V::Array(vec![msg(vec![("windingRule", designos_fig::testkit::en("NONZERO")), ("commandsBlob", V::Uint(1))])])),
        ],
    ));
    nodes.push(glyph_text((1, 2), (0, 1), "b", "Two glyphs", FAMILY, "\u{e900}\u{e900}", &[(0, 0.0), (0, 10.0)], vec![]));
    fig_file(nodes, vec![icon_blob(), designos_fig::testkit::square_blob()])
}
