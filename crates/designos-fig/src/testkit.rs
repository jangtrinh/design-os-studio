//! Test support: a Figma-like kiwi schema and a synthetic message, so `.fig`
//! files can be synthesized without any real Figma file. Not part of the
//! stable API.

use crate::container::{self, Compression};
use crate::kiwi::{DefKind, FieldType, KiwiValue as V, Native, Schema, SchemaBuilder};
use std::collections::BTreeMap;

const SCHEMA_TEXT: &str = include_str!("figma_schema.txt");

/// Parses the text form of a Kiwi schema (the format of `figma_schema.txt` and of the real-schema reference
/// kept under `tests/data`). Panics on a malformed line: both texts are static.
pub fn parse_schema_text(text: &str) -> Schema {
    let mut b = SchemaBuilder::new();
    let mut kinds: Vec<(String, DefKind)> = Vec::new();
    for line in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        if !line.starts_with(' ') {
            let (kind, name) = line.split_once(' ').unwrap_or_else(|| panic!("bad definition line `{line}`"));
            let kind = match kind {
                "Enum" => DefKind::Enum,
                "Struct" => DefKind::Struct,
                "Message" => DefKind::Message,
                other => panic!("bad definition kind `{other}`"),
            };
            b.define(name, kind);
            kinds.push((name.to_string(), kind));
        }
    }
    let mut cur = 0usize;
    let mut next_def = 0usize;
    for line in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        if !line.starts_with(' ') {
            cur = next_def;
            next_def += 1;
            continue;
        }
        let (lhs, ty) = match line.trim().split_once(" : ") {
            Some((l, t)) => (l, Some(t)),
            None => (line.trim(), None),
        };
        let (name, value) = lhs.split_once(" = ").unwrap_or_else(|| panic!("bad field line `{line}`"));
        let value: u32 = value.parse().unwrap_or_else(|_| panic!("bad field value in `{line}`"));
        match (kinds[cur].1, ty) {
            (DefKind::Enum, None) => b.variant(cur, name, value),
            (DefKind::Enum, Some(_)) => panic!("enum variant with a type: `{line}`"),
            (_, None) => panic!("field without a type: `{line}`"),
            (_, Some(ty)) => {
                let (ty, is_array) = ty.strip_suffix("[]").map_or((ty, false), |t| (t, true));
                let ft = match ty {
                    "Bool" => FieldType::Native(Native::Bool),
                    "Byte" => FieldType::Native(Native::Byte),
                    "Int" => FieldType::Native(Native::Int),
                    "Uint" => FieldType::Native(Native::Uint),
                    "Float" => FieldType::Native(Native::Float),
                    "String" => FieldType::Native(Native::String),
                    "Int64" => FieldType::Native(Native::Int64),
                    "Uint64" => FieldType::Native(Native::Uint64),
                    def => FieldType::Def(kinds.iter().position(|(n, _)| n == def).unwrap_or_else(|| panic!("unknown type `{def}` in `{line}`"))),
                };
                b.field(cur, name, ft, is_array, value);
            }
        }
    }
    b.build()
}

/// The embedded export schema: Figma's real definitions, ids and enum values for the part of the format Studio reads
/// and writes (see `figma_schema.txt`).
pub fn figma_like_schema() -> Schema {
    parse_schema_text(SCHEMA_TEXT)
}

/// The id of a NodeChange field in Figma's schema, `None` for a name Figma does not know.
pub fn real_node_change_tag(name: &str) -> Option<u32> {
    static TAGS: std::sync::OnceLock<BTreeMap<String, u32>> = std::sync::OnceLock::new();
    TAGS.get_or_init(|| {
        let schema = figma_like_schema();
        let nc = schema.find("NodeChange").expect("NodeChange in the embedded schema");
        schema.defs[nc].fields.iter().map(|f| (f.name.to_string(), f.value)).collect()
    })
    .get(name)
    .copied()
}

// ---- value builders

pub fn msg(fields: Vec<(&str, V)>) -> V {
    V::Message(fields.into_iter().map(|(k, v)| (k.into(), v)).collect())
}
pub fn st(fields: Vec<(&str, V)>) -> V {
    V::Struct(fields.into_iter().map(|(k, v)| (k.into(), v)).collect())
}
pub fn en(n: &str) -> V {
    V::Enum(n.into())
}
pub fn fl(x: f32) -> V {
    V::Float(x)
}
pub fn guid(s: u32, l: u32) -> V {
    st(vec![("sessionID", V::Uint(s)), ("localID", V::Uint(l))])
}
pub fn vec2(x: f32, y: f32) -> V {
    st(vec![("x", fl(x)), ("y", fl(y))])
}
pub fn matrix(tx: f32, ty: f32) -> V {
    st(vec![("m00", fl(1.0)), ("m01", fl(0.0)), ("m02", fl(tx)), ("m10", fl(0.0)), ("m11", fl(1.0)), ("m12", fl(ty))])
}
pub fn color(r: f32, g: f32, b: f32, a: f32) -> V {
    st(vec![("r", fl(r)), ("g", fl(g)), ("b", fl(b)), ("a", fl(a))])
}
pub fn solid(r: f32, g: f32, b: f32) -> V {
    msg(vec![("type", en("SOLID")), ("color", color(r, g, b, 1.0)), ("opacity", fl(1.0)), ("visible", V::Bool(true))])
}
pub fn node(id: (u32, u32), parent: (u32, u32), pos: &str, ty: &str, name: &str, mut extra: Vec<(&str, V)>) -> V {
    let mut f = vec![
        ("guid", guid(id.0, id.1)),
        ("parentIndex", st(vec![("guid", guid(parent.0, parent.1)), ("position", V::String(pos.into()))])),
        ("type", en(ty)),
        ("name", V::String(name.into())),
    ];
    f.append(&mut extra);
    msg(f)
}

/// Commands blob for a closed triangle: M(0,0) L(10,0) L(5,8) Z.
pub fn triangle_blob() -> Vec<u8> {
    let mut b = Vec::new();
    for (op, pts) in [(1u8, &[0.0f32, 0.0][..]), (2, &[10.0, 0.0]), (2, &[5.0, 8.0])] {
        b.push(op);
        for p in pts {
            b.extend(p.to_le_bytes());
        }
    }
    b.push(0);
    b
}

pub const IMAGE_HASH: [u8; 4] = [0xde, 0xad, 0xbe, 0xef];

/// A small but representative document. Siblings are deliberately listed out
/// of z-order to exercise fractional-index sorting.
pub fn sample_message() -> V {
    let doc = node((0, 0), (0, 0), "!", "DOCUMENT", "Document", vec![]);
    let page = node((0, 1), (0, 0), "a", "CANVAS", "Page 1", vec![("backgroundColor", color(0.9, 0.9, 0.9, 1.0))]);
    let internal = node((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", vec![("internalOnly", V::Bool(true))]);
    let frame = node(
        (1, 1),
        (0, 1),
        "a",
        "FRAME",
        "Card",
        vec![
            ("size", vec2(200.0, 300.0)),
            ("transform", matrix(40.0, 50.0)),
            ("fillPaints", V::Array(vec![solid(1.0, 1.0, 1.0)])),
            ("cornerRadius", fl(12.0)),
            (
                "effects",
                V::Array(vec![msg(vec![
                    ("type", en("DROP_SHADOW")),
                    ("color", color(0.0, 0.0, 0.0, 0.25)),
                    ("offset", vec2(0.0, 4.0)),
                    ("radius", fl(8.0)),
                    ("spread", fl(0.0)),
                    ("visible", V::Bool(true)),
                ])]),
            ),
            ("stackMode", en("VERTICAL")),
            ("stackSpacing", fl(8.0)),
            ("stackHorizontalPadding", fl(16.0)),
            ("stackVerticalPadding", fl(12.0)),
            ("stackPrimarySizing", en("RESIZE_TO_FIT")),
            ("stackCounterAlignItems", en("CENTER")),
        ],
    );
    let rect = node(
        (1, 2),
        (1, 1),
        "b", // listed first, but sorts after the text
        "ROUNDED_RECTANGLE",
        "Image rect",
        vec![
            ("size", vec2(100.0, 40.0)),
            ("transform", matrix(16.0, 12.0)),
            (
                "fillPaints",
                V::Array(vec![msg(vec![("type", en("IMAGE")), ("image", msg(vec![("hash", V::Bytes(IMAGE_HASH.to_vec()))])), ("imageScaleMode", en("FILL"))])]),
            ),
            ("strokePaints", V::Array(vec![solid(1.0, 0.0, 0.0)])),
            ("strokeWeight", fl(2.0)),
            ("strokeAlign", en("OUTSIDE")),
            ("stackChildAlignSelf", en("STRETCH")),
            ("blendMode", en("MULTIPLY")),
        ],
    );
    let text = node(
        (1, 3),
        (1, 1),
        "a",
        "TEXT",
        "Title",
        vec![
            ("size", vec2(120.0, 20.0)),
            ("transform", matrix(16.0, 60.0)),
            ("fillPaints", V::Array(vec![solid(0.1, 0.2, 0.3)])),
            ("fontSize", fl(16.0)),
            (
                "fontName",
                st(vec![("family", V::String("Inter".into())), ("style", V::String("Medium".into())), ("postscript", V::String("Inter-Medium".into()))]),
            ),
            ("letterSpacing", st(vec![("value", fl(10.0)), ("units", en("PERCENT"))])),
            ("lineHeight", st(vec![("value", fl(24.0)), ("units", en("PIXELS"))])),
            ("textAlignHorizontal", en("CENTER")),
            ("textAutoResize", en("WIDTH_AND_HEIGHT")),
            (
                "textData",
                msg(vec![
                    ("characters", V::String("Hi \u{1F600}!".into())),
                    // 'H','i',' ', surrogate pair (2 units), '!' -> UTF-16 length 6
                    ("characterStyleIDs", V::Array([0u32, 0, 0, 1, 1, 0].iter().map(|x| V::Uint(*x)).collect())),
                    (
                        "styleOverrideTable",
                        V::Array(vec![msg(vec![
                            ("styleID", V::Uint(1)),
                            (
                                "fontName",
                                st(vec![
                                    ("family", V::String("Inter".into())),
                                    ("style", V::String("Bold Italic".into())),
                                    ("postscript", V::String("".into())),
                                ]),
                            ),
                            ("fillPaints", V::Array(vec![solid(1.0, 0.0, 0.0)])),
                            ("textDecoration", en("UNDERLINE")),
                        ])]),
                    ),
                ]),
            ),
        ],
    );
    let vector = node(
        (1, 4),
        (1, 1),
        "c",
        "VECTOR",
        "Tri",
        vec![
            ("size", vec2(10.0, 8.0)),
            ("transform", matrix(0.0, 100.0)),
            ("fillPaints", V::Array(vec![solid(0.0, 0.0, 0.0)])),
            ("fillGeometry", V::Array(vec![msg(vec![("windingRule", en("NONZERO")), ("commandsBlob", V::Uint(0))])])),
        ],
    );
    let comp = node(
        (1, 5),
        (1, 1),
        "d",
        "SYMBOL",
        "Button",
        vec![("size", vec2(80.0, 30.0)), ("transform", matrix(0.0, 120.0)), ("fillPaints", V::Array(vec![solid(0.0, 0.5, 1.0)]))],
    );
    let comp_child = node((1, 6), (1, 5), "a", "RECTANGLE", "Bg", vec![("size", vec2(80.0, 30.0)), ("fillPaints", V::Array(vec![solid(0.2, 0.2, 0.2)]))]);
    let inst = node(
        (1, 7),
        (1, 1),
        "e",
        "INSTANCE",
        "Button copy",
        vec![("size", vec2(80.0, 30.0)), ("transform", matrix(100.0, 120.0)), ("symbolData", msg(vec![("symbolID", guid(1, 5))]))],
    );
    let ell = node(
        (1, 8),
        (0, 1),
        "b",
        "ELLIPSE",
        "Pie",
        vec![
            ("size", vec2(50.0, 50.0)),
            ("transform", matrix(300.0, 50.0)),
            ("arcData", msg(vec![("startingAngle", fl(0.0)), ("endingAngle", fl(3.0)), ("innerRadius", fl(0.5))])),
        ],
    );
    let removed = V::Message(match node((1, 9), (1, 1), "z", "RECTANGLE", "Removed", vec![("phase", en("REMOVED"))]) {
        V::Message(f) => f,
        _ => unreachable!(),
    });
    let star = node((1, 10), (0, 1), "c", "STAR", "Star", vec![("size", vec2(40.0, 40.0)), ("count", V::Uint(6)), ("starInnerScale", fl(0.5))]);
    let sticky = node((1, 11), (0, 1), "d", "STICKY", "Note", vec![("size", vec2(40.0, 40.0))]);

    msg(vec![
        // Child before parent and shuffled positions on purpose.
        ("nodeChanges", V::Array(vec![inst, rect, text, vector, comp_child, comp, frame, ell, removed, star, sticky, internal, page, doc])),
        ("blobs", V::Array(vec![st(vec![("bytes", V::Bytes(triangle_blob()))])])),
    ])
}

/// Builds a complete `.fig` byte stream from the sample message.
pub fn sample_fig(how: Compression, zipped: bool) -> Vec<u8> {
    let schema = figma_like_schema();
    let root = schema.find("Message").unwrap();
    let body = schema.encode_message(root, &sample_message()).unwrap();
    let canvas = container::write_container(&schema.encode(), &body, how, 101);
    if !zipped {
        return canvas;
    }
    let mut images = BTreeMap::new();
    images.insert(crate::kiwi::hex(&IMAGE_HASH), vec![0x89, b'P', b'N', b'G']);
    container::write_zip(&canvas, &images, Some(&serde_json::json!({"file_name": "Sample file"})))
}

// ---- upgrade fixture: geometry, styles, grids, text clamping, instance overrides

/// Commands blob for a closed square: M(0,0) L(10,0) L(10,10) L(0,10) Z.
pub fn square_blob() -> Vec<u8> {
    let mut b = Vec::new();
    for (op, pts) in [(1u8, [0.0f32, 0.0]), (2, [10.0, 0.0]), (2, [10.0, 10.0]), (2, [0.0, 10.0])] {
        b.push(op);
        for p in pts {
            b.extend(p.to_le_bytes());
        }
    }
    b.push(0);
    b
}

fn path_of(guids: &[(u32, u32)]) -> V {
    msg(vec![("guids", V::Array(guids.iter().map(|g| guid(g.0, g.1)).collect()))])
}

fn ov(path: &[(u32, u32)], mut fields: Vec<(&str, V)>) -> V {
    let mut f = vec![("guidPath", path_of(path))];
    f.append(&mut fields);
    msg(f)
}

fn num(value: f32, units: &str) -> V {
    st(vec![("value", fl(value)), ("units", en(units))])
}

fn text_data(s: &str) -> V {
    msg(vec![("characters", V::String(s.into()))])
}

/// Message exercising the `.fig` upgrade features (see `tests/fig_upgrade.rs`).
/// Blob 0 is the triangle, blob 1 the square.
pub fn upgrade_message() -> V {
    let geo = |rule: &str, blob: u32| V::Array(vec![msg(vec![("windingRule", en(rule)), ("commandsBlob", V::Uint(blob))])]);
    let size = |w: f32, h: f32| ("size", vec2(w, h));
    let doc = node((0, 0), (0, 0), "!", "DOCUMENT", "Document", vec![]);
    let page = node((0, 1), (0, 0), "a", "CANVAS", "Up", vec![("backgroundColor", color(1.0, 1.0, 1.0, 1.0))]);
    let internal = node((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", vec![("internalOnly", V::Bool(true))]);
    let style_node = |l: u32, name: &str, ty: &str, desc: &str, key: Option<&str>| {
        let mut f = vec![("styleType", en(ty)), ("styleDescription", V::String(desc.into())), size(1.0, 1.0)];
        if let Some(k) = key {
            f.push(("key", V::String(k.into())));
        }
        node((2, l), (0, 2), &format!("{l}"), "RECTANGLE", name, f)
    };
    let styles = [
        style_node(100, "Brand/Primary", "FILL", "Main brand fill", Some("key-primary")),
        style_node(101, "Border/Strong", "FILL", "", None),
        style_node(102, "Body", "TEXT", "Body copy", Some("key-body")),
    ];
    let sref = |l: u32| msg(vec![("guid", guid(2, l))]);

    let grids = node(
        (2, 1),
        (0, 1),
        "a",
        "FRAME",
        "Grids",
        vec![
            size(300.0, 200.0),
            (
                "layoutGrids",
                V::Array(vec![
                    msg(vec![
                        ("pattern", en("STRIPES")),
                        ("axis", en("X")),
                        ("numSections", V::Int(4)),
                        ("gutterSize", fl(8.0)),
                        ("offset", fl(2.0)),
                        ("color", color(1.0, 0.0, 0.0, 0.1)),
                        ("visible", V::Bool(true)),
                    ]),
                    msg(vec![("pattern", en("GRID")), ("sectionSize", fl(10.0)), ("color", color(0.0, 0.0, 1.0, 0.2))]),
                    msg(vec![("pattern", en("STRIPES")), ("axis", en("Y")), ("numSections", V::Int(3)), ("gutterSize", fl(4.0)), ("visible", V::Bool(false))]),
                ]),
            ),
        ],
    );
    let boolean = node(
        (2, 2),
        (0, 1),
        "b",
        "BOOLEAN_OPERATION",
        "Bool",
        vec![
            size(10.0, 10.0),
            ("fillGeometry", geo("NONZERO", 0)),
            ("strokeGeometry", geo("ODD", 1)),
            ("blendMode", en("LINEAR_BURN")),
            ("styleIdForFill", sref(100)),
            ("styleIdForStrokeFill", sref(101)),
            ("fillPaints", V::Array(vec![solid(1.0, 0.0, 0.0)])),
            ("strokePaints", V::Array(vec![solid(0.0, 0.0, 0.0)])),
        ],
    );
    let star = node(
        (2, 3),
        (0, 1),
        "c",
        "STAR",
        "Star",
        vec![size(10.0, 10.0), ("count", V::Uint(5)), ("fillGeometry", geo("NONZERO", 0)), ("blendMode", en("LINEAR_DODGE"))],
    );
    let text = |l: u32, pos: &str, name: &str, mut extra: Vec<(&str, V)>| {
        let mut f = vec![size(80.0, 40.0), ("fontSize", fl(10.0)), ("textData", text_data(name))];
        f.append(&mut extra);
        node((2, l), (0, 1), pos, "TEXT", name, f)
    };
    let clamp = text(
        4,
        "d",
        "Clamp",
        vec![("textTruncation", en("ENDING")), ("maxLines", V::Int(2)), ("lineHeight", num(100.0, "PERCENT")), ("styleIdForText", sref(102))],
    );
    let lh_auto = text(5, "e", "LhAuto", vec![("lineHeight", num(1.0, "RAW"))]);
    let lh_pct = text(6, "f", "LhPct", vec![("lineHeight", num(150.0, "PERCENT"))]);
    let lh_none = text(7, "g", "LhNone", vec![]);
    let lh_px = text(8, "h", "LhPx", vec![("lineHeight", num(18.0, "PIXELS"))]);
    let plain = text(9, "i", "Plain", vec![]);
    // Lists: bullet, nested number, plain line, in a right-to-left, cap-trimmed layer.
    let line = |ty: &str, level: u32| msg(vec![("lineType", en(ty)), ("indentationLevel", V::Int(level as i32)), ("sourceDirectionality", en("RTL"))]);
    let listy = node(
        (2, 50),
        (0, 1),
        "o",
        "TEXT",
        "Listy",
        vec![
            size(80.0, 40.0),
            ("fontSize", fl(10.0)),
            (
                "textData",
                msg(vec![
                    (
                        "characters",
                        V::String(
                            "one
two
three"
                                .into(),
                        ),
                    ),
                    ("lines", V::Array(vec![line("UNORDERED_LIST", 1), line("ORDERED_LIST", 2), line("PLAIN", 0)])),
                ]),
            ),
            ("leadingTrim", en("CAP_HEIGHT")),
            ("paragraphIndent", fl(6.0)),
            ("toggledOnOTFeatures", V::Array(vec![en("SS01")])),
            ("fontVariantNumericSpacing", en("TABULAR")),
            ("toggledOffOTFeatures", V::Array(vec![en("LIGA")])),
        ],
    );

    // Component with a label, background and a nested frame.
    let card = node((2, 10), (0, 1), "j", "SYMBOL", "Card", vec![size(100.0, 50.0), ("transform", matrix(0.0, 300.0))]);
    let label = node(
        (2, 11),
        (2, 10),
        "a",
        "TEXT",
        "Label",
        vec![size(60.0, 12.0), ("fontSize", fl(10.0)), ("textData", text_data("Old")), ("fillPaints", V::Array(vec![solid(0.0, 0.0, 0.0)]))],
    );
    let bg = node((2, 12), (2, 10), "b", "RECTANGLE", "Bg", vec![size(100.0, 50.0), ("fillPaints", V::Array(vec![solid(0.5, 0.5, 0.5)]))]);
    let inner = node((2, 13), (2, 10), "c", "FRAME", "Inner", vec![size(20.0, 20.0)]);
    let dot = node((2, 14), (2, 13), "a", "RECTANGLE", "Dot", vec![size(5.0, 5.0), ("fillPaints", V::Array(vec![solid(0.0, 1.0, 0.0)]))]);

    // Wrapper component containing a nested instance of Card with its own override.
    let wrapper = node((2, 20), (0, 1), "k", "SYMBOL", "Wrapper", vec![size(100.0, 50.0), ("transform", matrix(200.0, 300.0))]);
    let card_in = node(
        (2, 21),
        (2, 20),
        "a",
        "INSTANCE",
        "CardIn",
        vec![
            size(100.0, 50.0),
            (
                "symbolData",
                msg(vec![("symbolID", guid(2, 10)), ("symbolOverrides", V::Array(vec![ov(&[(2, 11)], vec![("textData", text_data("Inner text"))])]))]),
            ),
        ],
    );
    let wrap_inst = node(
        (2, 30),
        (0, 1),
        "l",
        "INSTANCE",
        "WrapInst",
        vec![
            size(100.0, 50.0),
            ("transform", matrix(400.0, 300.0)),
            (
                "symbolData",
                msg(vec![("symbolID", guid(2, 20)), ("symbolOverrides", V::Array(vec![ov(&[(2, 21), (2, 11)], vec![("textData", text_data("Deep"))])]))]),
            ),
        ],
    );

    let card_a = node(
        (2, 40),
        (0, 1),
        "m",
        "INSTANCE",
        "Card A",
        vec![
            size(200.0, 80.0),
            ("transform", matrix(0.0, 400.0)),
            (
                "symbolData",
                msg(vec![
                    ("symbolID", guid(2, 10)),
                    (
                        "symbolOverrides",
                        V::Array(vec![
                            ov(
                                &[(2, 11)],
                                vec![
                                    ("textData", text_data("New \u{1F600}")),
                                    ("fillPaints", V::Array(vec![solid(1.0, 0.0, 0.0)])),
                                    ("letterSpacing", num(50.0, "PERCENT")),
                                    ("opacity", fl(0.75)),
                                ],
                            ),
                            ov(&[(2, 12)], vec![("visible", V::Bool(false)), ("blendMode", en("LINEAR_DODGE")), ("cornerRadius", fl(4.0))]),
                            ov(&[(2, 13), (2, 14)], vec![("fillPaints", V::Array(vec![solid(0.0, 0.0, 1.0)])), size(7.0, 7.0)]),
                            ov(&[(9, 99)], vec![("visible", V::Bool(false))]), // dangling path: ignored
                        ]),
                    ),
                ]),
            ),
            (
                "derivedSymbolData",
                V::Array(vec![
                    ov(
                        &[(2, 12)],
                        vec![
                            size(200.0, 80.0),
                            ("transform", matrix(5.0, 6.0)),
                            ("fillGeometry", geo("NONZERO", 1)),
                            ("name", V::String("ignored by derived".into())),
                        ],
                    ),
                    ov(&[], vec![size(200.0, 80.0)]),
                ]),
            ),
        ],
    );
    let card_b = node(
        (2, 41),
        (0, 1),
        "n",
        "INSTANCE",
        "Card B",
        vec![size(100.0, 50.0), ("transform", matrix(300.0, 400.0)), ("symbolData", msg(vec![("symbolID", guid(2, 10))]))],
    );

    let mut nodes = vec![doc, page, internal];
    nodes.extend(styles);
    nodes.extend([grids, boolean, star, clamp, lh_auto, lh_pct, lh_none, lh_px, plain, listy]);
    nodes.extend([card, label, bg, inner, dot, wrapper, card_in, wrap_inst, card_a, card_b]);
    msg(vec![
        ("nodeChanges", V::Array(nodes)),
        ("blobs", V::Array(vec![st(vec![("bytes", V::Bytes(triangle_blob()))]), st(vec![("bytes", V::Bytes(square_blob()))])])),
    ])
}

/// `.fig` bytes for [`upgrade_message`].
pub fn upgrade_fig(how: Compression) -> Vec<u8> {
    let schema = figma_like_schema();
    let root = schema.find("Message").unwrap();
    let body = schema.encode_message(root, &upgrade_message()).unwrap();
    container::write_container(&schema.encode(), &body, how, 101)
}
