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
    // Figma writes the index of each glyph's first character; a glyph beyond the last character shares the last
    let last = characters.encode_utf16().count().saturating_sub(1) as u32;
    let glyphs = glyphs
        .iter()
        .enumerate()
        .map(|(i, &(blob, x))| {
            msg(vec![("commandsBlob", V::Uint(blob)), ("position", vec2(x, 20.0)), ("fontSize", fl(20.0)), ("firstCharacter", V::Uint((i as u32).min(last)))])
        })
        .collect();
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

/// One stored glyph at baseline y = 20: `blob` names its command blob (a glyph without one is a space), `first` is its
/// first character in UTF-16 units. `extra` adds fields of the glyph itself (rotation, emoji code points, ...).
pub fn stored_glyph(blob: Option<u32>, x: f32, first: u32, extra: Vec<(&str, V)>) -> V {
    let mut fields = vec![("position", vec2(x, 20.0)), ("fontSize", fl(20.0)), ("firstCharacter", V::Uint(first))];
    if let Some(blob) = blob {
        fields.insert(0, ("commandsBlob", V::Uint(blob)));
    }
    fields.extend(extra);
    msg(fields)
}

/// One bar of a `Decoration`: `rects` are `(x, y, w, h)`, `style` is the id of the style it belongs to.
pub fn stored_decoration(style: u32, rects: &[(f32, f32, f32, f32)]) -> V {
    let rects = rects.iter().map(|&(x, y, w, h)| st(vec![("x", fl(x)), ("y", fl(y)), ("w", fl(w)), ("h", fl(h))])).collect();
    msg(vec![("rects", V::Array(rects)), ("styleID", V::Uint(style))])
}

/// A 40 x 26 text layer built from parts: `base` family, `characters`, per-character `style_ids` with their override
/// table entries `overrides`, the stored `glyphs`, the families of `fontMetaData`, and extra fields of the derived data
/// (`decorations`, `truncationStartIndex`, ...) and of the layer.
#[allow(clippy::too_many_arguments)]
pub fn styled_glyph_text(
    id: (u32, u32),
    parent: (u32, u32),
    pos: &str,
    name: &str,
    base: &str,
    characters: &str,
    style_ids: &[u32],
    overrides: Vec<V>,
    glyphs: Vec<V>,
    fonts: &[&str],
    derived_extra: Vec<(&str, V)>,
    mut extra: Vec<(&str, V)>,
) -> V {
    let mut text_data = vec![("characters", V::String(characters.into()))];
    if !style_ids.is_empty() {
        text_data.push(("characterStyleIDs", V::Array(style_ids.iter().map(|&i| V::Uint(i)).collect())));
        text_data.push(("styleOverrideTable", V::Array(overrides)));
    }
    let mut derived = vec![
        ("layoutSize", vec2(40.0, 26.0)),
        ("fontMetaData", V::Array(fonts.iter().map(|f| msg(vec![("key", font_key(f))])).collect())),
        ("glyphs", V::Array(glyphs)),
    ];
    for (k, v) in derived_extra {
        derived.retain(|(name, _)| *name != k);
        derived.push((k, v));
    }
    let mut fields = vec![
        ("size", vec2(40.0, 26.0)),
        ("fontSize", fl(20.0)),
        ("fontName", font_key(base)),
        ("fillPaints", V::Array(vec![solid(0.0, 0.0, 0.0)])),
        ("textData", msg(text_data)),
        ("derivedTextData", msg(derived)),
    ];
    for (k, v) in extra.drain(..) {
        fields.retain(|(name, _)| *name != k);
        fields.push((k, v));
    }
    node(id, parent, pos, "TEXT", name, fields)
}

/// An entry of `styleOverrideTable`: style `id` in `family`, filled with the colour `rgb`, optionally underlined.
pub fn style_override(id: u32, family: &str, rgb: (f32, f32, f32), underline: bool) -> V {
    let mut fields = vec![("styleID", V::Uint(id)), ("fontName", font_key(family)), ("fillPaints", V::Array(vec![solid(rgb.0, rgb.1, rgb.2)]))];
    if underline {
        fields.push(("textDecoration", designos_fig::testkit::en("UNDERLINE")));
    }
    msg(fields)
}

/// The second font of [`two_run_fixture`].
pub const SECOND_FAMILY: &str = "SecondIconFaceForTest";

/// The text `Two runs` in a frame `Runs frame`, "abcd" in a 40 x 26 box: the first two characters in [`FAMILY`] in black, the last two in
/// [`SECOND_FAMILY`] in red and underlined. Two triangles, then two curves; Figma's bar for the second run runs under
/// the curves (x 20..40, y 22..24), and its glyphs carry no style id of their own, as Figma writes them.
pub fn two_run_fixture() -> Vec<u8> {
    let mut nodes = document_and_page();
    nodes.push(node((1, 1), (0, 1), "a", "FRAME", "Runs frame", vec![("size", vec2(40.0, 26.0))]));
    nodes.push(styled_glyph_text(
        (1, 2),
        (1, 1),
        "a",
        "Two runs",
        FAMILY,
        "abcd",
        &[0, 0, 7, 7],
        vec![style_override(7, SECOND_FAMILY, (1.0, 0.0, 0.0), true)],
        vec![
            stored_glyph(Some(0), 0.0, 0, vec![]),
            stored_glyph(Some(0), 10.0, 1, vec![]),
            stored_glyph(Some(1), 20.0, 2, vec![]),
            stored_glyph(Some(1), 30.0, 3, vec![]),
        ],
        &[FAMILY, SECOND_FAMILY],
        vec![("decorations", V::Array(vec![stored_decoration(7, &[(20.0, 22.0, 20.0, 2.0)])]))],
        vec![],
    ));
    fig_file(nodes, vec![icon_blob(), curve_blob()])
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

/// A smooth glyph in font-em space: M(0.1,0) Q(0.5,0.9)(0.9,0) C(0.7,0.3)(0.3,0.3)(0.1,0) Z.
pub fn curve_blob() -> Vec<u8> {
    let mut blob = Vec::new();
    let mut points = |op: u8, pts: &[f32]| {
        blob.push(op);
        blob.extend(pts.iter().flat_map(|v| v.to_le_bytes()));
    };
    points(1, &[0.1, 0.0]);
    points(3, &[0.5, 0.9, 0.9, 0.0]);
    points(4, &[0.7, 0.3, 0.3, 0.3, 0.1, 0.0]);
    points(0, &[]);
    blob
}

/// A 40 x 24 text layer in `family` whose derived data holds one glyph per `(blob index, x, y, font size)` entry; a
/// `None` blob is a glyph Figma stored without outline (a space).
pub fn mixed_glyph_text(id: (u32, u32), parent: (u32, u32), pos: &str, name: &str, family: &str, glyphs: &[(Option<u32>, f32, f32, f32)]) -> V {
    let glyphs = glyphs
        .iter()
        .enumerate()
        .map(|(i, &(blob, x, y, size))| {
            // the text is "ab c": the fifth glyph shares the last character
            let mut fields = vec![("position", vec2(x, y)), ("fontSize", fl(size)), ("firstCharacter", V::Uint((i as u32).min(3)))];
            if let Some(blob) = blob {
                fields.insert(0, ("commandsBlob", V::Uint(blob)));
            }
            msg(fields)
        })
        .collect();
    node(
        id,
        parent,
        pos,
        "TEXT",
        name,
        vec![
            ("size", vec2(40.0, 24.0)),
            ("fontSize", fl(20.0)),
            ("fontName", font_key(family)),
            ("fillPaints", V::Array(vec![solid(0.2, 0.4, 0.8)])),
            ("textData", msg(vec![("characters", V::String("ab c".into()))])),
            (
                "derivedTextData",
                msg(vec![
                    ("layoutSize", vec2(40.0, 24.0)),
                    ("fontMetaData", V::Array(vec![msg(vec![("key", font_key(family))])])),
                    ("glyphs", V::Array(glyphs)),
                ]),
            ),
        ],
    )
}

/// The texts every stored-glyph golden draws: an icon font glyph in a frame, a missing-font text of several glyphs
/// (two outlines, one reused, one glyph without outline, two sizes), and a component's text with an instance copy.
pub fn golden_fixture() -> Vec<u8> {
    let mut nodes = document_and_page();
    nodes.push(node((1, 1), (0, 1), "a", "FRAME", "Icon frame", vec![("size", vec2(24.0, 24.0))]));
    nodes.push(glyph_text((1, 2), (1, 1), "a", "Icon text", FAMILY, "\u{e900}", &[(0, 0.0)], vec![]));
    nodes.push(node((1, 4), (0, 1), "e", "FRAME", "Mixed frame", vec![("size", vec2(40.0, 24.0))]));
    nodes.push(mixed_glyph_text(
        (1, 3),
        (1, 4),
        "a",
        "Mixed glyphs",
        FAMILY,
        &[(Some(0), 0.0, 20.0, 20.0), (Some(1), 9.5, 20.0, 20.0), (None, 17.0, 20.0, 20.0), (Some(0), 20.25, 18.5, 14.0), (Some(1), 29.0, 18.5, 14.0)],
    ));
    nodes.push(node((1, 10), (0, 1), "c", "SYMBOL", "Icon main", vec![("size", vec2(20.0, 20.0))]));
    nodes.push(glyph_text((1, 11), (1, 10), "a", "Glyph source", FAMILY, "\u{e900}", &[(0, 0.0)], vec![]));
    nodes.push(node((1, 5), (0, 1), "f", "FRAME", "Copy frame", vec![("size", vec2(20.0, 20.0))]));
    nodes.push(node((1, 20), (1, 5), "a", "INSTANCE", "Icon copy", vec![("size", vec2(20.0, 20.0)), ("symbolData", msg(vec![("symbolID", guid(1, 10))]))]));
    fig_file(nodes, vec![icon_blob(), curve_blob()])
}

/// Every outline of `saved` as one command list, built the way the importer built it before the glyph table existed:
/// each glyph's blob decoded and placed at `x + p.x * size`, `y - p.y * size`.
pub fn eager_stored_path(saved: &designos_doc::FigTextGeometry) -> Vec<designos_doc::PathCmd> {
    use designos_doc::{PathCmd, Vec2};
    let mut out = Vec::new();
    for glyph in &saved.glyphs {
        let Some(slot) = glyph.blob_slot() else { continue };
        let at = |p: Vec2| Vec2::new(glyph.x + p.x * glyph.font_size, glyph.y - p.y * glyph.font_size);
        for cmd in designos_fig::commands_blob_to_path(&saved.blobs[slot as usize], false).cmds {
            out.push(match cmd {
                PathCmd::MoveTo(p) => PathCmd::MoveTo(at(p)),
                PathCmd::LineTo(p) => PathCmd::LineTo(at(p)),
                PathCmd::QuadTo(a, b) => PathCmd::QuadTo(at(a), at(b)),
                PathCmd::CubicTo(a, b, c) => PathCmd::CubicTo(at(a), at(b), at(c)),
                PathCmd::Close => PathCmd::Close,
            });
        }
    }
    out
}
