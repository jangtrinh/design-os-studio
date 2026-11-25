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
    nodes.push(node((1, 1), (0, 1), "a", "FRAME", "Linked", vec![("size", vec2(10.0, 10.0)), ("hyperlink", msg(vec![("guid", guid(1, 2))]))]));
    nodes.push(node((1, 2), (0, 1), "b", "FRAME", "Target", vec![("size", vec2(10.0, 10.0))]));
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
    assert!(now.iter().all(|r| guid_of(r) != (1, 2)), "the deleted node's guid is spent: no layer is written under it");
    let new_guid = now.iter().map(guid_of).find(|g| g.0 == 1 && *g != (1, 1)).expect("the new frame is written");
    assert!(new_guid.1 > 2, "{new_guid:?}");
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

#[test]
fn a_guid_at_a_place_no_rule_names_drops_the_field_and_the_path_is_reported() {
    let mut fill = solid(0.25, 0.5, 0.75);
    set_field(&mut fill, "sourceNodeId", guid(1, 9));
    let mut nodes = document_and_page();
    nodes.push(node((2, 1), (0, 1), "a", "FRAME", "Frame", vec![("size", vec2(10.0, 10.0)), ("fillPaints", V::Array(vec![fill]))]));
    let doc = import_fig(&small(nodes)).expect("imports");
    let (_, report) = export_fig_with_kept_report(&doc).expect("saves");
    assert_eq!(report.fields_dropped_by_reason.get("id_at_an_unlisted_path"), Some(&1), "{report:?}");
    assert_eq!(report.unlisted_paths.get("fillPaints/#/sourceNodeId"), Some(&1), "{report:?}");
}

/// A file whose third page is deferred (25,000 filler layers on the first page and more than two pages): the main component
/// `Main` (1:10) on the first page, and on the deferred page an instance of it (1:21) that holds its own copy of the fill.
fn deferred_instance_file() -> Vec<u8> {
    let plain = |extra: Vec<(&'static str, V)>| {
        let mut fields = vec![
            ("visible", V::Bool(true)),
            ("opacity", fl(1.0)),
            ("phase", en("CREATED")),
            ("size", vec2(100.0, 40.0)),
            ("transform", designos_fig::testkit::matrix(0.0, 0.0)),
            ("blendMode", en("PASS_THROUGH")),
        ];
        fields.extend(extra);
        fields
    };
    let red = || ("fillPaints", V::Array(vec![solid(1.0, 0.0, 0.0)]));
    let mut changes = vec![
        node((0, 0), (0, 0), "", "DOCUMENT", "Document", vec![]),
        node((0, 1), (0, 0), "!", "CANVAS", "Cover", vec![]),
        node((0, 2), (0, 0), "\"", "CANVAS", "Page 2", vec![]),
        node((0, 3), (0, 0), "#", "CANVAS", "Deferred", vec![]),
        node((1, 10), (0, 1), "!", "SYMBOL", "Main", plain(vec![red()])),
        node((1, 21), (0, 3), "!", "INSTANCE", "Copy", plain(vec![red(), ("symbolData", msg(vec![("symbolID", guid(1, 10))]))])),
    ];
    changes.extend((0..25_000u32).map(|i| node((9, i + 1), (0, 1), "z", "ROUNDED_RECTANGLE", "F", plain(vec![]))));
    fig_file(changes, vec![icon_blob()])
}

#[test]
fn a_main_edited_before_a_deferred_page_loads_does_not_leave_the_instance_with_its_old_kept_fill() {
    let bytes = deferred_instance_file();
    let run = |keep_store: bool| {
        let mut doc = import_fig(&bytes).expect("imports");
        assert!(doc.has_pending_pages(), "the fixture defers a page");
        if !keep_store {
            doc.fig_kept = Default::default();
        }
        let main = by_name(&doc, "Main");
        doc.get_mut(main).expect("the main").fills = vec![designos_doc::Paint::solid(designos_doc::Color::rgb(0, 0, 255))];
        let page = doc.pages.iter().copied().find(|p| doc.get(*p).is_some_and(|n| n.name == "Deferred")).expect("the deferred page");
        assert!(doc.ensure_page_hydrated(page), "the page loads");
        let (saved, report) = export_fig_with_kept_report(&doc).expect("saves");
        (records(&saved), report)
    };
    let (with, report) = run(true);
    let (without, _) = run(false);
    // what the loader does with an instance's own fill when its main changed is whatever a document with no store ends up with
    // (here the instance keeps its own copy); the store must not write another colour
    let colour = |all: &[V]| -> Option<V> {
        record_of(all, (1, 21)).get("fillPaints").and_then(V::as_array).and_then(|a| a.first()).and_then(|p| p.get("color")).cloned()
    };
    assert!(colour(&without).is_some());
    assert_eq!(colour(&with), colour(&without), "the instance's fill colour is the one a document with no store writes: {report:?}");
}

#[test]
fn a_child_layout_key_that_reads_its_parent_is_written_as_studio_computes_it_after_the_parent_changed() {
    let mut nodes = document_and_page();
    nodes.push(node(
        (2, 1),
        (0, 1),
        "a",
        "FRAME",
        "Grid",
        vec![("size", vec2(100.0, 100.0)), ("stackMode", en("GRID")), ("stackPrimarySizing", en("FIXED")), ("stackCounterSizing", en("FIXED"))],
    ));
    // a start alignment in a grid is written because the parent is a grid, and not otherwise
    nodes.push(node((2, 2), (2, 1), "a", "ROUNDED_RECTANGLE", "Child", vec![("size", vec2(10.0, 10.0)), ("gridChildHorizontalAlign", en("MIN"))]));
    let bytes = small(nodes);
    let run = |keep_store: bool| {
        let mut doc = import_fig(&bytes).expect("imports");
        if !keep_store {
            doc.fig_kept = Default::default();
        }
        let parent = by_name(&doc, "Grid");
        let al = doc.get_mut(parent).expect("the parent").kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()).expect("auto layout");
        al.direction = designos_doc::LayoutDirection::Horizontal;
        records(&export_fig(&doc).expect("saves"))
    };
    let (with, without) = (run(true), run(false));
    assert!(record_of(&without, (2, 2)).get("gridChildHorizontalAlign").is_none(), "Studio writes no start alignment under a row");
    assert_eq!(record_of(&with, (2, 2)).get("gridChildHorizontalAlign"), None, "the kept alignment of the grid is not written under a row");
}

/// A grid frame with two columns and one row, each track named by a guid of the file, and a child anchored in the second column.
fn anchored_grid_nodes() -> Vec<V> {
    let map = |ids: &[(u32, u32)]| {
        msg(vec![(
            "entries",
            V::Array(
                ids.iter()
                    .enumerate()
                    .map(|(i, &(s, l))| msg(vec![("id", guid(s, l)), ("position", V::String(char::from(b'!' + i as u8).to_string()))]))
                    .collect(),
            ),
        )])
    };
    let sizing = |ids: &[(u32, u32)]| {
        let f = || msg(vec![("type", en("FLEX")), ("value", fl(1.0))]);
        msg(vec![(
            "entries",
            V::Array(ids.iter().map(|&(s, l)| msg(vec![("id", guid(s, l)), ("trackSize", msg(vec![("minSizing", f()), ("maxSizing", f())]))])).collect()),
        )])
    };
    let (cols, rows) = ([(3, 1), (3, 2)], [(3, 3)]);
    let mut nodes = document_and_page();
    nodes.push(node(
        (2, 1),
        (0, 1),
        "a",
        "FRAME",
        "Grid",
        vec![
            ("size", vec2(100.0, 100.0)),
            ("stackMode", en("GRID")),
            ("gridColumns", map(&cols)),
            ("gridColumnsSizing", sizing(&cols)),
            ("gridRows", map(&rows)),
            ("gridRowsSizing", sizing(&rows)),
        ],
    ));
    nodes.push(node(
        (2, 2),
        (2, 1),
        "a",
        "ROUNDED_RECTANGLE",
        "Cell",
        vec![("size", vec2(10.0, 10.0)), ("gridColumnAnchor", guid(3, 2)), ("gridRowAnchor", guid(3, 3))],
    ));
    nodes
}

#[test]
fn a_grid_child_anchored_in_a_cell_is_in_the_same_cell_after_a_no_edit_save() {
    let doc = import_fig(&small(anchored_grid_nodes())).expect("imports");
    let cell = by_name(&doc, "Cell");
    assert_eq!(
        (doc.get(cell).unwrap().layout_child.grid_column, doc.get(cell).unwrap().layout_child.grid_row),
        (Some(1), Some(0)),
        "the fixture places the child"
    );
    let saved = export_fig(&doc).expect("saves");
    let now = records(&saved);
    // the anchors name tracks the parent defines
    let ids = |record: &V, key: &str| -> Vec<V> {
        record
            .get(key)
            .and_then(|m| m.get("entries"))
            .and_then(V::as_array)
            .map(|e| e.iter().filter_map(|x| x.get("id").cloned()).collect())
            .unwrap_or_default()
    };
    let parent = record_of(&now, (2, 1));
    let column = record_of(&now, (2, 2)).get("gridColumnAnchor").cloned().expect("the child keeps a column anchor");
    assert!(ids(parent, "gridColumns").contains(&column), "the anchor names a column of the parent: {column:?} in {:?}", ids(parent, "gridColumns"));
    let row = record_of(&now, (2, 2)).get("gridRowAnchor").cloned().expect("the child keeps a row anchor");
    assert!(ids(parent, "gridRows").contains(&row));
    // and a reopen keeps the child in its cell
    let reopened = import_fig(&saved).expect("reopens");
    let cell = by_name(&reopened, "Cell");
    assert_eq!((reopened.get(cell).unwrap().layout_child.grid_column, reopened.get(cell).unwrap().layout_child.grid_row), (Some(1), Some(0)));
}

#[test]
fn media_a_string_hash_names_that_the_save_does_not_write_drops_the_field() {
    let mut nodes = document_and_page();
    nodes.push(node(
        (2, 1),
        (0, 1),
        "a",
        "FRAME",
        "Media",
        vec![("size", vec2(10.0, 10.0)), ("richMediaData", msg(vec![("mediaHash", V::String("deadbeef".into()))]))],
    ));
    let doc = import_fig(&small(nodes)).expect("imports");
    let (saved, report) = export_fig_with_kept_report(&doc).expect("saves");
    assert_eq!(report.fields_dropped_by_reason.get("image_not_written"), Some(&1), "{report:?}");
    assert!(record_of(&records(&saved), (2, 1)).get("richMediaData").is_none());
    assert_eq!((report.nodes_partial, report.records_written), (1, 0), "a layer with a dropped field is partial: {report:?}");
}
