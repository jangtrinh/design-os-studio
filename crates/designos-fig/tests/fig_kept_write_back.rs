//! A `.fig` save writes Figma's own fields back on a layer nobody edited, and writes Studio's record for one that was.
#[path = "common/missing_font_fig.rs"]
mod missing_font_fig;

use designos_doc::{Document, Node, NodeId, NodeKind, Rect};
use designos_fig::container;
use designos_fig::kiwi::{KiwiValue as V, Schema};
use designos_fig::testkit::{en, fl, guid, msg, node, solid, vec2};
use designos_fig::{export_fig, export_fig_with_kept_report, import_fig};
use missing_font_fig::*;

/// Every record of a `.fig`, as kiwi values.
fn records(bytes: &[u8]) -> Vec<V> {
    let archive = container::open(bytes).expect("open the container");
    let (schema_bin, msg_bin) = container::read_archive_chunks(&archive).expect("read the chunks");
    let schema = Schema::decode(&schema_bin).expect("decode the schema");
    let root = schema.root_message().expect("root message");
    let mut message = schema.decode_message(root, &msg_bin).expect("decode the message");
    match message.take_field("nodeChanges") {
        Some(V::Array(a)) => a,
        _ => Vec::new(),
    }
}

fn guid_of(record: &V) -> (u32, u32) {
    let g = record.get("guid").expect("a record has a guid");
    let n = |k: &str| match g.get(k) {
        Some(V::Uint(u)) => *u,
        other => panic!("guid.{k} is {other:?}"),
    };
    (n("sessionID"), n("localID"))
}

fn record_of(all: &[V], id: (u32, u32)) -> &V {
    all.iter().find(|r| guid_of(r) == id).unwrap_or_else(|| panic!("no record {id:?}"))
}

fn count_of(record: &V, name: &str) -> usize {
    match record {
        V::Message(fields) => fields.iter().filter(|(k, _)| &**k == name).count(),
        _ => 0,
    }
}

fn set_field(change: &mut V, name: &str, value: V) {
    let V::Message(fields) = change else { unreachable!() };
    match fields.iter_mut().find(|(k, _)| &**k == name) {
        Some(slot) => slot.1 = value,
        None => fields.push((name.into(), value)),
    }
}

fn by_name(doc: &Document, name: &str) -> NodeId {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}")).id
}

fn authored() -> V {
    msg(vec![("space", en("SRGB")), ("c0", fl(0.25)), ("c1", fl(0.5)), ("c2", fl(0.75)), ("c3", fl(1.0))])
}

/// A solid fill whose colour Figma authored in a variable's terms: two subfields Studio does not hold.
fn authored_fill() -> V {
    let mut fill = solid(0.25, 0.5, 0.75);
    set_field(&mut fill, "authoredColor", authored());
    set_field(&mut fill, "colorVar", msg(vec![("dataType", en("COLOR")), ("resolvedDataType", en("COLOR"))]));
    fill
}

fn edit_info() -> V {
    msg(vec![("userId", V::String("user-1".into())), ("createdAt", V::Uint(1_700_000_000))])
}

fn percent(value: f32) -> V {
    designos_fig::testkit::st(vec![("value", fl(value)), ("units", en("PERCENT"))])
}

/// A frame with Figma-only fields, a rounded rectangle with four radii of its own, and a text with Figma's text versions.
fn figma_nodes() -> Vec<V> {
    let mut nodes = document_and_page();
    nodes.push(node(
        (2, 1),
        (0, 1),
        "a",
        "FRAME",
        "Frame",
        vec![("size", vec2(100.0, 60.0)), ("editInfo", edit_info()), ("fillPaints", V::Array(vec![authored_fill()]))],
    ));
    nodes.push(node(
        (2, 2),
        (0, 1),
        "b",
        "ROUNDED_RECTANGLE",
        "Rect",
        vec![
            ("size", vec2(40.0, 40.0)),
            ("rectangleCornerRadiiIndependent", V::Bool(true)),
            ("rectangleTopLeftCornerRadius", fl(1.0)),
            ("rectangleTopRightCornerRadius", fl(2.0)),
            ("rectangleBottomLeftCornerRadius", fl(3.0)),
            ("rectangleBottomRightCornerRadius", fl(4.0)),
        ],
    ));
    nodes.push(glyph_text(
        (2, 3),
        (0, 1),
        "c",
        "Text",
        FAMILY,
        "\u{e900}",
        &[(0, 0.0)],
        vec![
            ("fontVersion", V::String("1.2.3".into())),
            ("textBidiVersion", V::Uint(2)),
            ("textUserLayoutVersion", V::Uint(5)),
            ("letterSpacing", percent(12.5)),
        ],
    ));
    nodes
}

/// More than 5,000 entries and a payload over 2 MB, so the decoder is heavy and the importer's large-file guards drop
/// `editInfo`, and the file a save of it writes is still a large one.
fn large(mut nodes: Vec<V>) -> Vec<u8> {
    nodes.extend((1..=5_001u32).map(|i| node((9, i), (0, 1), "z", "FRAME", "Filler", vec![("size", vec2(4.0, 4.0))])));
    fig_file(nodes, vec![icon_blob(), vec![0u8; 2_100_000]])
}

fn small(nodes: Vec<V>) -> Vec<u8> {
    fig_file(nodes, vec![icon_blob()])
}

const FRAME: (u32, u32) = (2, 1);
const RECT: (u32, u32) = (2, 2);
const TEXT: (u32, u32) = (2, 3);

#[test]
fn a_large_file_saved_with_no_edit_writes_figmas_fields_back() {
    let original = large(figma_nodes());
    let doc = import_fig(&original).expect("imports");
    let (saved, report) = export_fig_with_kept_report(&doc).expect("saves");
    let (was, now) = (records(&original), records(&saved));

    for (id, keys) in [
        (FRAME, vec!["editInfo", "fillPaints"]),
        (
            RECT,
            vec![
                "rectangleCornerRadiiIndependent",
                "rectangleTopLeftCornerRadius",
                "rectangleTopRightCornerRadius",
                "rectangleBottomLeftCornerRadius",
                "rectangleBottomRightCornerRadius",
            ],
        ),
        (TEXT, vec!["fontVersion", "textBidiVersion", "textUserLayoutVersion", "letterSpacing"]),
    ] {
        for key in keys {
            assert_eq!(record_of(&now, id).get(key), record_of(&was, id).get(key), "{id:?} {key} is Figma's");
            assert_eq!(count_of(record_of(&now, id), key), 1, "{id:?} {key} is written once");
        }
    }
    let fill = record_of(&now, FRAME).get("fillPaints").and_then(V::as_array).and_then(|a| a.first()).expect("a fill");
    assert!(fill.get("authoredColor").is_some(), "authoredColor is written");
    assert!(fill.get("colorVar").and_then(|c| c.get("resolvedDataType")).is_some(), "colorVar.resolvedDataType is written");
    assert_eq!(report.nodes_edited, 0, "nobody edited anything");
    assert!(report.nodes_written >= 3 && report.fields_written > 10, "{report:?}");
    assert_eq!(report.records_written, report.nodes_written);
}

/// A text of two characters whose two style rows (ids 3 and 5 in the file) have one style, so Studio holds them as one run and
/// would write one row with an id of its own; the glyphs name Figma's rows.
fn text_with_two_rows_of_one_style() -> Vec<V> {
    let mut text = glyph_text((2, 3), (0, 1), "c", "Text", FAMILY, "\u{e900}\u{e900}", &[(0, 0.0), (0, 10.0)], vec![]);
    let row = |id: u32| msg(vec![("styleID", V::Uint(id)), ("fontSize", fl(12.0))]);
    set_field(
        &mut text,
        "textData",
        msg(vec![
            ("characters", V::String("\u{e900}\u{e900}".into())),
            ("characterStyleIDs", V::Array(vec![V::Uint(3), V::Uint(5)])),
            ("styleOverrideTable", V::Array(vec![row(3), row(5)])),
        ]),
    );
    let glyph = |blob: u32, x: f32, first: u32, style: u32| {
        msg(vec![
            ("commandsBlob", V::Uint(blob)),
            ("position", vec2(x, 20.0)),
            ("styleID", V::Uint(style)),
            ("fontSize", fl(20.0)),
            ("firstCharacter", V::Uint(first)),
        ])
    };
    set_field(
        &mut text,
        "derivedTextData",
        msg(vec![
            ("layoutSize", vec2(20.0, 20.0)),
            ("fontMetaData", V::Array(vec![msg(vec![("key", font_key(FAMILY))])])),
            ("glyphs", V::Array(vec![glyph(0, 0.0, 0, 3), glyph(0, 10.0, 1, 5)])),
        ]),
    );
    let mut nodes = document_and_page();
    nodes.push(text);
    nodes
}

fn glyph_styles(all: &[V]) -> Vec<V> {
    record_of(all, TEXT)
        .get("derivedTextData")
        .and_then(|d| d.get("glyphs"))
        .and_then(V::as_array)
        .map(|glyphs| glyphs.iter().filter_map(|g| g.get("styleID").cloned()).collect())
        .unwrap_or_default()
}

#[test]
fn a_text_whose_style_rows_would_be_renumbered_saves_figmas_text_data_and_glyph_style_ids() {
    let nodes = text_with_two_rows_of_one_style();
    for bytes in [small(nodes.clone()), large(nodes)] {
        let doc = import_fig(&bytes).expect("imports");
        let saved = export_fig(&doc).expect("saves");
        let (was, now) = (records(&bytes), records(&saved));
        assert_eq!(record_of(&now, TEXT).get("textData"), record_of(&was, TEXT).get("textData"), "the text's rows keep Figma's ids");
        assert_eq!(glyph_styles(&now), glyph_styles(&was), "the glyphs name the same rows");
        assert_eq!(glyph_styles(&now), vec![V::Uint(3), V::Uint(5)]);
    }
}

#[test]
fn a_text_that_is_written_as_studio_models_it_has_glyphs_that_follow_the_rows_studio_wrote() {
    let bytes = small(text_with_two_rows_of_one_style());
    let mut doc = import_fig(&bytes).expect("imports");
    // a recolour leaves the stored glyphs valid but is an edit of the layer, so its text data is Studio's
    let id = by_name(&doc, "Text");
    doc.get_mut(id).expect("the text").fills = vec![designos_doc::Paint::solid(designos_doc::Color::WHITE)];
    let (saved, report) = export_fig_with_kept_report(&doc).expect("saves");
    let now = records(&saved);
    assert_eq!(report.nodes_edited, 1);
    let data = record_of(&now, TEXT).get("textData").expect("text data");
    assert_ne!(data, record_of(&records(&bytes), TEXT).get("textData").expect("text data"), "Studio's rows are its own");
    // Studio's numbering and the glyphs agree: the glyphs are written with it, or not at all
    let rows: Vec<V> = data.get("styleOverrideTable").and_then(V::as_array).expect("rows").iter().filter_map(|r| r.get("styleID").cloned()).collect();
    assert!(
        glyph_styles(&now).iter().all(|g| *g == V::Uint(0) || rows.contains(g)),
        "every glyph names a row of Studio's table: {:?} {:?}",
        glyph_styles(&now),
        rows
    );
}

#[test]
fn editing_a_texts_characters_writes_studios_record_and_no_font_version_comes_back() {
    let original = large(figma_nodes());
    let mut doc = import_fig(&original).expect("imports");
    let id = by_name(&doc, "Text");
    let NodeKind::Text(tp) = &mut doc.get_mut(id).expect("the text").kind else { panic!("a text") };
    tp.content = "\u{e900}\u{e900}\u{e900}".into();
    let (saved, report) = export_fig_with_kept_report(&doc).expect("saves");
    let now = records(&saved);
    let text = record_of(&now, TEXT);
    assert!(text.get("fontVersion").is_none(), "a version of the old layout does not come back from the metadata copy");
    assert!(text.get("textBidiVersion").is_none() && text.get("textUserLayoutVersion").is_none());
    assert_eq!(text.get("textData").and_then(|d| d.get("characters")), Some(&V::String("\u{e900}\u{e900}\u{e900}".into())));
    assert_eq!(report.nodes_edited, 1, "{report:?}");
    assert_eq!(report.edited_sample, vec![id]);
    // the other layers were not touched and are still written from the store
    assert_eq!(record_of(&now, FRAME).get("editInfo"), record_of(&records(&original), FRAME).get("editInfo"));
}

#[test]
fn an_edited_layer_is_written_exactly_as_a_document_with_no_store_writes_it() {
    let original = large(figma_nodes());
    let mut with_store = import_fig(&original).expect("imports");
    let mut without = import_fig(&original).expect("imports");
    without.fig_kept = Default::default();
    for doc in [&mut with_store, &mut without] {
        let id = by_name(doc, "Frame");
        doc.get_mut(id).expect("the frame").name = "Renamed".into();
    }
    let (a, b) = (records(&export_fig(&with_store).expect("saves")), records(&export_fig(&without).expect("saves")));
    assert_eq!(record_of(&a, FRAME), record_of(&b, FRAME), "one edit: Studio's own record, as it was written before");
    // an untouched layer differs, which is the point
    assert_ne!(record_of(&a, TEXT), record_of(&b, TEXT));
}

#[test]
fn a_no_edit_save_reopens_to_the_same_document_node_by_node() {
    for original in [small(figma_nodes()), large(figma_nodes())] {
        let first = import_fig(&original).expect("imports");
        let saved = export_fig(&first).expect("saves");
        let second = import_fig(&saved).expect("reopens");
        let by_source = |doc: &Document| -> std::collections::BTreeMap<String, u64> {
            doc.nodes.values().filter_map(|n| Some((n.source_id.clone()?, designos_doc::branch_diff::node_content_hash(n)))).collect()
        };
        let (a, b) = (by_source(&first), by_source(&second));
        assert!(a.len() >= 4, "the document, the page and three layers");
        for (source, hash) in &a {
            if b.get(source) != Some(hash) {
                let (x, y) = (
                    first.nodes.values().find(|n| n.source_id.as_deref() == Some(source)).unwrap(),
                    second.nodes.values().find(|n| n.source_id.as_deref() == Some(source)).unwrap(),
                );
                panic!("{source} reopens different\n{}\n{}", serde_json::to_string(x).unwrap(), serde_json::to_string(y).unwrap());
            }
        }
        assert_eq!(a.len(), b.len());
    }
}

#[test]
fn a_kept_reference_to_a_deleted_node_drops_the_field_and_the_guid_is_not_given_to_a_new_node() {
    let mut nodes = document_and_page();
    nodes.push(node((2, 1), (0, 1), "a", "FRAME", "Linked", vec![("size", vec2(10.0, 10.0)), ("hyperlink", msg(vec![("guid", guid(2, 2))]))]));
    nodes.push(node((2, 2), (0, 1), "b", "FRAME", "Target", vec![("size", vec2(10.0, 10.0))]));
    let bytes = small(nodes);

    let doc = import_fig(&bytes).expect("imports");
    let (_, report) = export_fig_with_kept_report(&doc).expect("saves");
    assert_eq!(report.fields_dropped_by_reason.get("node_not_written"), None, "the target is written: {report:?}");

    let mut doc = import_fig(&bytes).expect("imports");
    let target = by_name(&doc, "Target");
    doc.remove(target).expect("the target goes");
    let page = doc.pages[0];
    let id = doc.alloc_id();
    doc.insert(page, Node::frame(id, Rect::new(0.0, 0.0, 5.0, 5.0))).expect("a new frame");
    let (saved, report) = export_fig_with_kept_report(&doc).expect("saves");
    assert_eq!(report.fields_dropped_by_reason.get("node_not_written"), Some(&1), "{report:?}");
    let now = records(&saved);
    assert!(now.iter().all(|r| guid_of(r) != (2, 2)), "the deleted node's guid is spent: no layer is written under it");
}

#[test]
fn what_the_save_still_writes_as_studio_models_it_is_not_taken_from_the_store() {
    let mut nodes = document_and_page();
    nodes.push(node(
        (2, 1),
        (0, 1),
        "a",
        "FRAME",
        "Frame",
        vec![
            ("size", vec2(10.0, 10.0)),
            ("prototypeInteractions", V::Array(vec![msg(vec![("id", guid(2, 9)), ("event", msg(vec![("interactionType", en("ON_CLICK"))]))])])),
            ("scrollBehavior", en("SCROLLS")),
            ("editInfo", edit_info()),
        ],
    ));
    let bytes = small(nodes);
    let doc = import_fig(&bytes).expect("imports");
    let (saved, report) = export_fig_with_kept_report(&doc).expect("saves");
    let now = records(&saved);
    assert!(record_of(&now, FRAME).get("prototypeInteractions").is_none(), "no writer yet: nothing comes back");
    assert!(record_of(&now, FRAME).get("scrollBehavior").is_none());
    assert_eq!(record_of(&now, FRAME).get("editInfo"), Some(&edit_info()), "the rest is Figma's");
    assert_eq!(report.fields_dropped(), 0);
}

#[test]
fn a_document_with_no_store_saves_with_an_empty_report() {
    let bytes = small(figma_nodes());
    let mut doc = import_fig(&bytes).expect("imports");
    doc.fig_kept = Default::default();
    let (_, report) = export_fig_with_kept_report(&doc).expect("saves");
    assert_eq!(report, Default::default());
}
