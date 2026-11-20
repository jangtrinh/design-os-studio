#[path = "common/missing_font_fig.rs"]
mod missing_font_fig;

use designos_doc::{Node, NodeKind, PathCmd, Vec2};
use designos_fig::kiwi::KiwiValue as V;
use designos_fig::testkit::{en, fl, guid, msg, node, st, vec2};
use designos_fig::{decode_fig_to_json, export_fig, import_fig};
use missing_font_fig::*;

fn fixture() -> Vec<u8> {
    icon_fixture(FAMILY)
}

fn icon_node(doc: &designos_doc::Document) -> &Node {
    doc.nodes.values().find(|node| node.name == "Icon text").unwrap()
}

#[test]
fn missing_icon_font_uses_embedded_glyph_without_losing_editability() {
    let doc = import_fig(&fixture()).unwrap();
    let icon = icon_node(&doc);
    let saved = icon.fig_text_geometry.as_ref().expect("embedded glyphs retained");
    assert_eq!(
        eager_stored_path(saved),
        vec![PathCmd::MoveTo(Vec2::new(0.0, 20.0)), PathCmd::LineTo(Vec2::new(20.0, 20.0)), PathCmd::LineTo(Vec2::new(10.0, 4.0)), PathCmd::Close,]
    );
    assert_eq!(saved.glyphs.len(), 1, "one glyph in the table");
    let glyph = saved.glyphs.as_slice()[0];
    assert_eq!((glyph.x, glyph.y, glyph.font_size, glyph.blob_slot()), (0.0, 20.0, 20.0, Some(0)));
    assert!(saved.path.cmds.is_empty(), "no outline is built at import");
    assert!(saved.matches(icon));
    let outlines = designos_render::text_outlines_for_node(icon);
    assert_eq!(outlines.len(), 1);
    let bounds = outlines[0].path.bounds();
    assert!((bounds.left() - 0.0).abs() < 1e-4 && (bounds.right() - 20.0).abs() < 1e-4);

    let mut edited = icon.clone();
    if let NodeKind::Text(text) = &mut edited.kind {
        text.content = "edited".into();
    }
    assert!(!saved.matches(&edited), "the cached icon cannot override an edited text layer");
    edited.size.x += 1.0;
    assert!(!saved.matches(&edited), "the cached icon cannot override a resized text layer");

    // The geometry also survives Studio's document serialization.
    let restored: Node = serde_json::from_str(&serde_json::to_string(icon).unwrap()).unwrap();
    assert_eq!(restored.fig_text_geometry, icon.fig_text_geometry);

    let reopened = import_fig(&export_fig(&doc).unwrap()).unwrap();
    assert_eq!(icon_node(&reopened).fig_text_geometry, icon.fig_text_geometry);

    let mut edited_doc = doc.clone();
    if let NodeKind::Text(text) = &mut edited_doc.get_mut(icon.id).unwrap().kind {
        text.content = "edited".into();
    }
    let reopened_edit = import_fig(&export_fig(&edited_doc).unwrap()).unwrap();
    assert!(icon_node(&reopened_edit).fig_text_geometry.is_none(), "edited text must not export stale icon geometry");
}

fn child_text<'a>(doc: &'a designos_doc::Document, holder: &str) -> &'a Node {
    let holder = doc.nodes.values().find(|n| n.name == holder).unwrap();
    holder.children.iter().filter_map(|&c| doc.get(c)).find(|n| matches!(n.kind, NodeKind::Text(_))).expect("a text child")
}

/// Whether any outline drawn for `node` has the bounds of the fixture's saved triangle (x 0..20, y 4..20).
fn draws_saved_triangle(node: &Node) -> bool {
    designos_render::text_outlines_for_node(node).iter().any(|o| {
        let b = o.path.bounds();
        (b.left() - 0.0).abs() < 1e-4 && (b.right() - 20.0).abs() < 1e-4 && (b.top() - 4.0).abs() < 1e-4 && (b.bottom() - 20.0).abs() < 1e-4
    })
}

#[test]
fn instance_text_override_does_not_draw_the_component_glyph() {
    let doc = import_fig(&instance_override_fixture()).unwrap();

    let main = child_text(&doc, "Icon main");
    assert!(main.fig_text_geometry.as_ref().is_some_and(|g| g.matches(main)), "the main component keeps its embedded glyph");
    assert!(draws_saved_triangle(main));

    let inst = child_text(&doc, "Icon instance");
    let NodeKind::Text(text) = &inst.kind else { unreachable!() };
    assert_eq!(text.content, "\u{e901}", "the instance shows its own characters");
    assert!(inst.fig_text_geometry.as_ref().is_none_or(|g| !g.matches(inst)), "the component's glyph must not stand in for the overridden characters");
    assert!(!draws_saved_triangle(inst), "the instance text takes the shaping path");
}

#[test]
fn fill_edit_keeps_the_embedded_glyph_and_geometry_edits_drop_it() {
    let doc = import_fig(&fixture()).unwrap();
    let icon = icon_node(&doc);
    assert!(draws_saved_triangle(icon));

    let edit = |f: &dyn Fn(&mut designos_doc::TextProps)| {
        let mut n = icon.clone();
        let NodeKind::Text(t) = &mut n.kind else { unreachable!() };
        f(t);
        n
    };
    let recoloured = edit(&|t| t.style.fills = vec![designos_doc::Paint::solid(designos_doc::Color::rgb(255, 0, 0))]);
    assert!(icon.fig_text_geometry.as_ref().unwrap().matches(&recoloured), "a colour change does not move a glyph");
    assert!(draws_saved_triangle(&recoloured), "the recoloured layer still draws Figma's glyph");

    for (what, n) in [
        ("font weight", edit(&|t| t.style.font_weight = 700)),
        ("font size", edit(&|t| t.style.font_size = 24.0)),
        ("letter spacing", edit(&|t| t.style.letter_spacing = 2.0)),
    ] {
        assert!(!icon.fig_text_geometry.as_ref().unwrap().matches(&n), "a {what} edit changes the glyph shapes");
        assert!(!draws_saved_triangle(&n), "a {what} edit drops the saved glyph");
    }
}

fn blob_count(bytes: &[u8]) -> usize {
    decode_fig_to_json(bytes).unwrap()["message"]["blobs"].as_array().map_or(0, Vec::len)
}

#[test]
fn glyph_blobs_are_deduplicated_and_reindexed_on_export() {
    let doc = import_fig(&shared_blob_fixture()).unwrap();
    let text = doc.nodes.values().find(|n| n.name == "Two glyphs").unwrap();
    let saved = text.fig_text_geometry.as_ref().expect("both glyphs are retained");
    assert_eq!(eager_stored_path(saved).len(), 8, "two triangles");
    assert_eq!((saved.glyphs.len(), saved.blobs.len()), (2, 1), "two glyphs share one blob");

    let saved_bytes = export_fig(&doc).unwrap();
    let reopened = import_fig(&saved_bytes).unwrap();
    let again = reopened.nodes.values().find(|n| n.name == "Two glyphs").unwrap();
    // the wire bytes and the blob numbers are the file's own and differ between two files; everything else is the same
    let (a, b) = (again.fig_text_geometry.as_ref().unwrap(), text.fig_text_geometry.as_ref().unwrap());
    assert_eq!(
        (&a.glyphs, &a.blobs, &a.decorations, a.fallback),
        (&b.glyphs, &b.blobs, &b.decorations, b.fallback),
        "the glyphs survive a save with the vector's blob ahead of them"
    );

    // The same file without the glyph cache shows how many blobs the rest of the document needs.
    let mut bare = doc.clone();
    let id = text.id;
    bare.get_mut(id).unwrap().fig_text_geometry = None;
    assert_eq!(blob_count(&saved_bytes), blob_count(&export_fig(&bare).unwrap()) + 1, "one blob for the glyph shared by both occurrences");

    // Both glyphs point at the same table entry, and it is not entry 0: the vector's blob is written ahead of it.
    let changes = decode_fig_to_json(&saved_bytes).unwrap();
    let derived = changes["message"]["nodeChanges"].as_array().unwrap().iter().find(|n| n["name"] == "Two glyphs").unwrap()["derivedTextData"].clone();
    let slots: Vec<u64> = derived["glyphs"].as_array().unwrap().iter().map(|g| g["commandsBlob"].as_u64().unwrap()).collect();
    assert_eq!(slots.len(), 2);
    assert_eq!(slots[0], slots[1], "a shared glyph is one blob");
    assert_ne!(slots[0], 0, "the glyph blob was re-indexed behind the vector's blob");
}

#[test]
fn serialized_glyph_blobs_use_the_compact_hex_form() {
    let doc = import_fig(&fixture()).unwrap();
    let icon = icon_node(&doc);
    let json = serde_json::to_string(icon).unwrap();
    assert!(json.contains("hex:"), "blob bytes are written as hex strings: {json}");
    let restored: Node = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.fig_text_geometry, icon.fig_text_geometry);
}

#[test]
fn an_underlined_text_draws_the_bar_figma_stored() {
    let mut nodes = document_and_page();
    nodes.push(styled_glyph_text(
        (1, 2),
        (0, 1),
        "a",
        "Icon text",
        FAMILY,
        "\u{e900}",
        &[],
        vec![],
        vec![stored_glyph(Some(0), 0.0, 0, vec![])],
        &[FAMILY],
        vec![("decorations", V::Array(vec![stored_decoration(0, &[(0.0, 22.0, 20.0, 2.0)])]))],
        vec![("textDecoration", en("UNDERLINE"))],
    ));
    let doc = import_fig(&fig_file(nodes, vec![icon_blob()])).unwrap();
    let icon = icon_node(&doc);
    let NodeKind::Text(t) = &icon.kind else { unreachable!() };
    assert_ne!(t.style.decoration, designos_doc::TextDecoration::None, "the fixture imports as underlined");
    assert!(icon.fig_text_geometry.as_ref().is_some_and(|g| g.matches(icon)), "the underline is part of the snapshot");
    let b = designos_render::text_outlines_for_node(icon)[0].path.bounds();
    assert!(
        (b.left() - 0.0).abs() < 1e-3 && (b.right() - 20.0).abs() < 1e-3 && (b.top() - 4.0).abs() < 1e-3 && (b.bottom() - 24.0).abs() < 1e-3,
        "the glyph and Figma's bar: {b:?}"
    );
}

#[test]
fn an_underlined_text_with_no_stored_bar_is_shaped() {
    let mut nodes = document_and_page();
    nodes.push(glyph_text((1, 2), (0, 1), "a", "Icon text", FAMILY, "\u{e900}", &[(0, 0.0)], vec![("textDecoration", en("UNDERLINE"))]));
    let doc = import_fig(&fig_file(nodes, vec![icon_blob()])).unwrap();
    assert!(!draws_saved_triangle(icon_node(&doc)), "the saved outlines carry no underline, so shaping draws the text");
}

#[test]
fn text_outlines_of_a_non_text_node_are_empty() {
    let doc = import_fig(&fixture()).unwrap();
    let frame = doc.nodes.values().find(|n| n.name == "Clipping frame").unwrap();
    assert!(designos_render::text_outlines_for_node(frame).is_empty());
}

#[test]
fn flatten_turns_the_saved_glyph_into_vectors() {
    let doc = import_fig(&fixture()).unwrap();
    let runs = designos_render::vectorops::text_flatten_runs(&doc, icon_node(&doc));
    let points: Vec<Vec2> = runs
        .iter()
        .flat_map(|r| r.path.cmds.iter())
        .filter_map(|c| match *c {
            PathCmd::MoveTo(p) | PathCmd::LineTo(p) | PathCmd::QuadTo(_, p) | PathCmd::CubicTo(_, _, p) => Some(p),
            PathCmd::Close => None,
        })
        .collect();
    let (left, right) = points.iter().fold((f32::MAX, f32::MIN), |(l, r), p| (l.min(p.x), r.max(p.x)));
    assert!(!points.is_empty() && left.abs() < 1e-3 && (right - 20.0).abs() < 1e-3, "flatten keeps the saved glyph: x {left}..{right}");
}

#[test]
fn styled_text_keeps_its_glyphs_through_the_rest_of_the_import() {
    let style_ref = msg(vec![("guid", guid(2, 102))]);
    let mut nodes = document_and_page();
    nodes.push(node((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", vec![("internalOnly", V::Bool(true))]));
    nodes.push(node(
        (2, 102),
        (0, 2),
        "1",
        "RECTANGLE",
        "Body",
        vec![("styleType", en("TEXT")), ("styleDescription", V::String("Body copy".into())), ("size", vec2(1.0, 1.0)), ("key", V::String("key-body".into()))],
    ));
    nodes.push(node((1, 10), (0, 1), "a", "SYMBOL", "Icon main", vec![("size", vec2(20.0, 20.0))]));
    nodes.push(glyph_text(
        (1, 11),
        (1, 10),
        "a",
        "Styled",
        FAMILY,
        "\u{e900}",
        &[(0, 0.0)],
        vec![
            ("styleIdForText", style_ref),
            ("lineHeight", st(vec![("value", fl(150.0)), ("units", en("PERCENT"))])),
            ("letterSpacing", st(vec![("value", fl(2.0)), ("units", en("PIXELS"))])),
            ("textCase", en("UPPER")),
        ],
    ));
    nodes.push(node(
        (1, 20),
        (0, 1),
        "b",
        "INSTANCE",
        "Plain copy",
        vec![("size", vec2(20.0, 20.0)), ("symbolData", msg(vec![("symbolID", guid(1, 10)), ("symbolOverrides", V::Array(vec![]))]))],
    ));
    let doc = import_fig(&fig_file(nodes, vec![icon_blob()])).unwrap();

    let main = doc.nodes.values().find(|n| n.name == "Styled").unwrap();
    let NodeKind::Text(text) = &main.kind else { unreachable!() };
    assert_ne!(text.style.letter_spacing, 0.0, "the fixture sets the settings this test is about");
    assert!(draws_saved_triangle(main), "the import's later steps leave the settings the saved glyphs were taken with");

    let copy = child_text(&doc, "Plain copy");
    assert!(draws_saved_triangle(copy), "a copy that overrides nothing draws the component's glyphs");
}

/// Writes a small `.fig` for looking at the result on screen: a card with Figma's saved glyphs for a font that is
/// not installed, above the same family laid out by the stand-in font. Run with
/// `DESIGNOS_MISSING_FONT_FIXTURE=<out.fig> cargo test -p designos-fig --test fig_missing_font_glyphs -- --ignored`.
#[test]
#[ignore = "writes a fixture for the screenshot check"]
fn write_the_missing_font_fixture_for_a_screenshot() {
    let Ok(out) = std::env::var("DESIGNOS_MISSING_FONT_FIXTURE") else { panic!("set DESIGNOS_MISSING_FONT_FIXTURE to the output path") };
    // M(0,0) L(1,0) L(1,1) L(0,1) Z in em space, and a diamond.
    let blob = |pts: &[(f32, f32)]| {
        let mut b = Vec::new();
        for (i, (x, y)) in pts.iter().enumerate() {
            b.push(if i == 0 { 1 } else { 2 });
            b.extend(x.to_le_bytes());
            b.extend(y.to_le_bytes());
        }
        b.push(0);
        b
    };
    let square = blob(&[(0.1, 0.0), (0.9, 0.0), (0.9, 0.8), (0.1, 0.8)]);
    let diamond = blob(&[(0.5, 0.0), (1.0, 0.4), (0.5, 0.8), (0.0, 0.4)]);
    let mut nodes = document_and_page();
    nodes.push(node(
        (1, 1),
        (0, 1),
        "a",
        "FRAME",
        "Card",
        vec![("size", vec2(480.0, 300.0)), ("fillPaints", V::Array(vec![designos_fig::testkit::solid(1.0, 1.0, 1.0)])), ("frameMaskDisabled", V::Bool(false))],
    ));
    let big = |id: (u32, u32), pos: &str, name: &str, chars: &str, x: f32, y: f32, glyphs: V, size: (f32, f32), font_size: f32| {
        let mut fields = vec![
            ("transform", designos_fig::testkit::matrix(x, y)),
            ("size", vec2(size.0, size.1)),
            ("fontSize", fl(font_size)),
            ("fontName", font_key(FAMILY)),
            ("fillPaints", V::Array(vec![designos_fig::testkit::solid(0.1, 0.2, 0.6)])),
            ("textData", msg(vec![("characters", V::String(chars.into()))])),
        ];
        if let V::Array(g) = &glyphs {
            if !g.is_empty() {
                fields.push((
                    "derivedTextData",
                    msg(vec![("layoutSize", vec2(size.0, size.1)), ("fontMetaData", V::Array(vec![msg(vec![("key", font_key(FAMILY))])])), ("glyphs", glyphs)]),
                ));
            }
        }
        node(id, (1, 1), pos, "TEXT", name, fields)
    };
    let glyph = |blob: u32, x: f32| msg(vec![("commandsBlob", V::Uint(blob)), ("position", vec2(x, 130.0)), ("fontSize", fl(130.0))]);
    nodes.push(big(
        (1, 2),
        "a",
        "Saved glyphs",
        "\u{e900}\u{e901}\u{e900}",
        24.0,
        16.0,
        V::Array(vec![glyph(0, 0.0), glyph(1, 130.0), glyph(0, 260.0)]),
        (390.0, 130.0),
        130.0,
    ));
    nodes.push(big((1, 3), "b", "Stand-in font", "Aspect", 24.0, 180.0, V::Array(vec![]), (432.0, 80.0), 64.0));
    std::fs::write(out, fig_file(nodes, vec![square, diamond])).unwrap();
}

/// A blob of 1,999 bytes: a move and 221 lines, then a close.
fn long_blob() -> Vec<u8> {
    let mut b = vec![1];
    b.extend(0f32.to_le_bytes());
    b.extend(0f32.to_le_bytes());
    for i in 0..221 {
        b.push(2);
        b.extend((i as f32 / 221.0).to_le_bytes());
        b.extend(1f32.to_le_bytes());
    }
    b.push(0);
    b
}

fn text_using_one_blob(times: usize) -> Node {
    let glyphs: Vec<(u32, f32)> = (0..times).map(|i| (0, i as f32)).collect();
    let mut nodes = document_and_page();
    nodes.push(glyph_text((1, 11), (0, 1), "a", "Long", FAMILY, "\u{e900}", &glyphs, vec![]));
    let doc = import_fig(&fig_file(nodes, vec![long_blob()])).unwrap();
    doc.nodes.values().find(|n| n.name == "Long").unwrap().clone()
}

#[test]
fn a_blob_shared_by_many_glyphs_counts_once_per_glyph_against_the_size_limit() {
    assert_eq!(long_blob().len(), 1999);
    // 1,000 uses decode 1,999,000 bytes, under the 2 MB a text may decode; 1,001 uses go over it.
    let drawn = |n: &Node| designos_render::stored_glyph_use(n) == designos_render::StoredGlyphUse::Drawn;
    assert!(drawn(&text_using_one_blob(1000)), "under the limit the saved glyphs draw");
    assert!(!drawn(&text_using_one_blob(1001)), "a shared blob still costs its bytes for every glyph that draws it");
}
