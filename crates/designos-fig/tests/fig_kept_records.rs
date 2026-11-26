//! A `.fig` save writes Figma's own records of the styles, variables and variable sets nobody edited, writes Studio's record for one
//! that was, brings back the records Figma marked deleted that Studio has no entry for, and keeps the layers and records of the
//! internal canvas under the guid the file gave it.
#[path = "common/missing_font_fig.rs"]
mod missing_font_fig;

use designos_doc::{Color, Document, Paint, Style, VariableValue};
use designos_fig::container;
use designos_fig::kiwi::{KiwiValue as V, Schema};
use designos_fig::testkit::{color, en, fl, guid, msg, node, solid, st, vec2};
use designos_fig::{export_fig, import_fig};
use missing_font_fig::{document_and_page, fig_file, font_key, icon_blob};
use std::collections::BTreeSet;

/// The internal canvas of the fixtures: Figma's guid for it is not the one Studio writes its own under.
const INTERNAL: (u32, u32) = (7, 3);
const SET: (u32, u32) = (7, 20);
const LIGHT: (u32, u32) = (7, 21);
const DARK: (u32, u32) = (7, 22);
const COLOR_VAR: (u32, u32) = (7, 30);
const PAINT_STYLE: (u32, u32) = (7, 40);
const TEXT_STYLE: (u32, u32) = (7, 41);
const DELETED_STYLE: (u32, u32) = (7, 50);

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

fn find(all: &[V], id: (u32, u32)) -> Option<&V> {
    all.iter().find(|r| guid_of(r) == id)
}

fn record_of(all: &[V], id: (u32, u32)) -> &V {
    find(all, id).unwrap_or_else(|| panic!("no record {id:?}"))
}

fn parent_of(record: &V) -> (u32, u32) {
    let g = record.get("parentIndex").and_then(|p| p.get("guid")).expect("a parent guid");
    let n = |k: &str| match g.get(k) {
        Some(V::Uint(u)) => *u,
        other => panic!("parent guid {k} is {other:?}"),
    };
    (n("sessionID"), n("localID"))
}

fn text(s: &str) -> V {
    V::String(s.into())
}

fn set_field(record: &mut V, name: &str, value: V) {
    let V::Message(fields) = record else { unreachable!() };
    match fields.iter_mut().find(|(k, _)| &**k == name) {
        Some(slot) => slot.1 = value,
        None => fields.push((name.into(), value)),
    }
}

/// A solid fill whose colour Figma authored in terms of a variable: two subfields Studio does not hold.
fn authored_fill(r: f32) -> V {
    let mut fill = solid(r, 0.5, 0.75);
    set_field(&mut fill, "authoredColor", msg(vec![("space", en("SRGB")), ("c0", fl(r)), ("c1", fl(0.5)), ("c2", fl(0.75)), ("c3", fl(1.0))]));
    set_field(&mut fill, "colorVar", msg(vec![("dataType", en("COLOR")), ("resolvedDataType", en("COLOR"))]));
    fill
}

fn internal_canvas() -> V {
    node(
        INTERNAL,
        (0, 0),
        "~",
        "CANVAS",
        "Internal Only Canvas",
        vec![("internalOnly", V::Bool(true)), ("opacity", fl(1.0)), ("visible", V::Bool(true)), ("version", text("9:9"))],
    )
}

fn style_extras(key: &str, sort: &str) -> Vec<(&'static str, V)> {
    vec![
        ("key", text(key)),
        ("sortPosition", text(sort)),
        ("size", vec2(1.0, 1.0)),
        ("opacity", fl(1.0)),
        ("visible", V::Bool(true)),
        ("strokeWeight", fl(1.0)),
        ("version", text("1:1")),
        ("userFacingVersion", text("2:2")),
        ("isPublishable", V::Bool(true)),
    ]
}

fn paint_style(id: (u32, u32), name: &str, pos: &str, key: &str, r: f32, deleted: bool) -> V {
    let mut extra = style_extras(key, pos);
    extra.push(("styleType", en("FILL")));
    extra.push(("styleDescription", text("brand colour")));
    extra.push(("fillPaints", V::Array(vec![authored_fill(r)])));
    if deleted {
        extra.push(("isSoftDeleted", V::Bool(true)));
        extra.push(("isSoftDeletedStyle", V::Bool(true)));
    }
    node(id, INTERNAL, pos, "ROUNDED_RECTANGLE", name, extra)
}

fn percent(value: f32) -> V {
    st(vec![("value", fl(value)), ("units", en("PERCENT"))])
}

fn text_style(id: (u32, u32), pos: &str) -> V {
    let mut extra = style_extras("text-key", pos);
    extra.push(("styleType", en("TEXT")));
    extra.push(("fontName", font_key("Inter")));
    extra.push(("fontSize", fl(14.0)));
    extra.push(("letterSpacing", percent(12.5)));
    extra.push((
        "derivedTextData",
        msg(vec![
            ("layoutSize", vec2(20.0, 20.0)),
            (
                "glyphs",
                V::Array(vec![msg(vec![("commandsBlob", V::Uint(0)), ("position", vec2(0.0, 20.0)), ("fontSize", fl(14.0)), ("firstCharacter", V::Uint(0))])]),
            ),
        ]),
    ));
    node(id, INTERNAL, pos, "TEXT", "Body/Regular", extra)
}

fn variable_set() -> V {
    let mode = |id: (u32, u32), name: &str, pos: &str| msg(vec![("id", guid(id.0, id.1)), ("name", text(name)), ("sortPosition", text(pos))]);
    node(
        SET,
        INTERNAL,
        "a",
        "VARIABLE_SET",
        "Colors",
        vec![
            ("key", text("set-key")),
            ("version", text("3:3")),
            ("userFacingVersion", text("4:4")),
            ("isPublishable", V::Bool(true)),
            ("variableSetModes", V::Array(vec![mode(LIGHT, "Light", "!"), mode(DARK, "Dark", "\"")])),
        ],
    )
}

fn value_entry(mode: (u32, u32), r: f32) -> V {
    msg(vec![
        ("modeID", guid(mode.0, mode.1)),
        (
            "variableData",
            msg(vec![("value", msg(vec![("colorValue", color(r, 0.2, 0.3, 1.0))])), ("dataType", en("COLOR")), ("resolvedDataType", en("COLOR"))]),
        ),
    ])
}

fn alias_entry(mode: (u32, u32), target: (u32, u32)) -> V {
    msg(vec![
        ("modeID", guid(mode.0, mode.1)),
        (
            "variableData",
            msg(vec![
                ("value", msg(vec![("alias", msg(vec![("guid", guid(target.0, target.1))]))])),
                ("dataType", en("ALIAS")),
                ("resolvedDataType", en("COLOR")),
            ]),
        ),
    ])
}

fn variable(id: (u32, u32), set: (u32, u32), name: &str, entries: Vec<V>, deleted: bool) -> V {
    let mut extra = vec![
        ("key", text(&format!("var-key-{}", id.1))),
        ("description", text("a colour")),
        ("sortPosition", text("!")),
        ("version", text("5:5")),
        ("userFacingVersion", text("6:6")),
        ("isPublishable", V::Bool(true)),
        ("variableSetID", msg(vec![("guid", guid(set.0, set.1))])),
        ("variableResolvedType", en("COLOR")),
        ("variableDataValues", msg(vec![("entries", V::Array(entries))])),
    ];
    if deleted {
        extra.push(("isSoftDeleted", V::Bool(true)));
    }
    node(id, INTERNAL, "b", "VARIABLE", name, extra)
}

/// A page with one frame, the internal canvas, a variable set with a variable, a paint style and a text style.
fn live_records() -> Vec<V> {
    let mut nodes = document_and_page();
    nodes.push(node((2, 1), (0, 1), "a", "FRAME", "Frame", vec![("size", vec2(100.0, 60.0))]));
    nodes.push(internal_canvas());
    nodes.push(variable_set());
    nodes.push(variable(COLOR_VAR, SET, "Brand/Primary", vec![value_entry(LIGHT, 0.1), value_entry(DARK, 0.9)], false));
    nodes.push(paint_style(PAINT_STYLE, "Brand/Primary", "a", "paint-key", 0.25, false));
    nodes.push(text_style(TEXT_STYLE, "b"));
    nodes
}

/// The file, with a blob that keeps the capture's bound on ranges per payload byte clear of these small records.
fn file_of(nodes: Vec<V>) -> Vec<u8> {
    fig_file(nodes, vec![icon_blob(), vec![0u8; 20_000]])
}

/// The same records in a file the decoder reads on its heavy path (more than 5,000 entries and a payload over 2 MB): the way the kit
/// is read. Its decoder keeps the stored glyphs of a text apart from the record.
fn large_file_of(mut nodes: Vec<V>) -> Vec<u8> {
    nodes.extend((1..=5_001u32).map(|i| node((9, i), (0, 1), "z", "FRAME", "Filler", vec![("size", vec2(4.0, 4.0))])));
    fig_file(nodes, vec![icon_blob(), vec![0u8; 2_100_000]])
}

/// The blob table of a saved file.
fn blobs(bytes: &[u8]) -> Vec<Vec<u8>> {
    let archive = container::open(bytes).expect("open the container");
    let (schema_bin, msg_bin) = container::read_archive_chunks(&archive).expect("read the chunks");
    let schema = Schema::decode(&schema_bin).expect("decode the schema");
    let root = schema.root_message().expect("root message");
    let mut message = schema.decode_message(root, &msg_bin).expect("decode the message");
    match message.take_field("blobs") {
        Some(V::Array(a)) => a.iter().filter_map(|b| b.get("bytes").and_then(V::as_bytes).map(<[u8]>::to_vec)).collect(),
        _ => Vec::new(),
    }
}

/// Every field of the original record `id` is in the saved record, with the same value.
fn assert_record_kept(original: &[V], saved: &[V], id: (u32, u32), what: &str) {
    let want = record_of(original, id);
    let got = find(saved, id).unwrap_or_else(|| panic!("{what}: no saved record"));
    let V::Message(fields) = want else { unreachable!() };
    for (name, value) in fields {
        if &**name == "derivedTextData" {
            continue;
        }
        assert_eq!(got.get(name), Some(value), "{what}: field {name}");
    }
}

/// The stored glyphs of the record `id` are in the saved file, and their outline is the file's own (its blob number is the save's).
fn assert_stored_glyphs_kept(saved: &[V], saved_blobs: &[Vec<u8>], id: (u32, u32), what: &str) {
    let derived = record_of(saved, id).get("derivedTextData").unwrap_or_else(|| panic!("{what}: no stored glyphs"));
    let Some(V::Array(glyphs)) = derived.get("glyphs") else { panic!("{what}: no glyphs") };
    let Some(V::Uint(blob)) = glyphs[0].get("commandsBlob") else { panic!("{what}: no outline") };
    assert_eq!(saved_blobs[*blob as usize], icon_blob(), "{what}: the outline");
}

fn saved_of(doc: &Document) -> Vec<V> {
    records(&export_fig(doc).expect("saves"))
}

#[test]
fn an_unedited_paint_style_text_style_variable_and_variable_set_are_saved_as_the_file_had_them() {
    let original = file_of(live_records());
    let before = records(&original);
    let doc = import_fig(&original).expect("imports");
    let saved_bytes = export_fig(&doc).expect("saves");
    let (saved, saved_blobs) = (records(&saved_bytes), blobs(&saved_bytes));
    assert_record_kept(&before, &saved, PAINT_STYLE, "paint style");
    assert_record_kept(&before, &saved, TEXT_STYLE, "text style");
    assert_stored_glyphs_kept(&saved, &saved_blobs, TEXT_STYLE, "text style");
    assert_record_kept(&before, &saved, COLOR_VAR, "variable");
    assert_record_kept(&before, &saved, SET, "variable set");
}

#[test]
fn an_edited_style_variable_and_set_are_written_as_studio_models_them() {
    let original = file_of(live_records());
    let mut doc = import_fig(&original).expect("imports");
    let paint = doc.styles.keys().find(|k| k.as_str() == "7:40").cloned().expect("the paint style");
    doc.styles.insert(paint, Style::Fill(vec![Paint::solid(Color::rgb(200, 10, 10))]));
    doc.variables.get_mut("VariableID:7:30").expect("the variable").values.insert("7:21".into(), VariableValue::Color(Color::rgb(1, 2, 3)));
    doc.variable_collections.get_mut("VariableCollectionId:7:20").expect("the set").name = "Renamed".into();
    let saved = saved_of(&doc);
    for (id, what) in [(PAINT_STYLE, "style"), (COLOR_VAR, "variable"), (SET, "set")] {
        let rec = record_of(&saved, id);
        assert!(rec.get("version").is_none(), "an edited {what} is Studio's record, which holds no version: {rec:?}");
        assert!(rec.get("userFacingVersion").is_none(), "{what}");
    }
    let style = record_of(&saved, PAINT_STYLE);
    let V::Array(paints) = style.get("fillPaints").expect("paints") else { panic!() };
    assert!(paints[0].get("authoredColor").is_none(), "the edited style's paint is Studio's");
    assert_eq!(record_of(&saved, SET).get("name"), Some(&text("Renamed")));
    // the text style nobody edited is still Figma's
    assert_eq!(record_of(&saved, TEXT_STYLE).get("version"), Some(&text("1:1")));
}

#[test]
fn a_soft_deleted_paint_style_and_its_internal_canvas_are_written_as_the_file_had_them_and_read_back() {
    let mut nodes = document_and_page();
    nodes.push(internal_canvas());
    nodes.push(paint_style(DELETED_STYLE, "Old/Brand", "a", "old-key", 0.4, true));
    let original = file_of(nodes);
    let before = records(&original);
    let doc = import_fig(&original).expect("imports");
    assert!(doc.styles.is_empty(), "a deleted style is not an entry of the document");
    let saved_bytes = export_fig(&doc).expect("saves");
    let saved = records(&saved_bytes);
    assert_record_kept(&before, &saved, DELETED_STYLE, "deleted style");
    assert_record_kept(&before, &saved, INTERNAL, "internal canvas");
    assert_eq!(parent_of(record_of(&saved, DELETED_STYLE)), INTERNAL, "the deleted style stays under the canvas it was on");
    // it reads back: the file opens and the deleted style is still no entry of the document, as it was at the first open
    let reopened = import_fig(&saved_bytes).expect("the saved file opens");
    assert!(reopened.styles.is_empty(), "a deleted style is not an entry of the reopened document either");
    export_fig(&reopened).expect("the reopened file saves");
    let guids: Vec<(u32, u32)> = saved.iter().map(guid_of).collect();
    assert_eq!(guids.len(), guids.iter().collect::<BTreeSet<_>>().len(), "no guid is written twice");
}

#[test]
fn a_variable_whose_mode_is_gone_loses_its_value_for_that_mode_and_no_mode_id_dangles() {
    let original = file_of(live_records());
    let mut doc = import_fig(&original).expect("imports");
    // the set loses its Dark mode but the variable keeps its table of values: the variable is unedited
    doc.variable_collections.get_mut("VariableCollectionId:7:20").expect("the set").modes.retain(|m| m.id != "7:22");
    let saved = saved_of(&doc);
    let set_modes: BTreeSet<(u32, u32)> = match record_of(&saved, SET).get("variableSetModes") {
        Some(V::Array(modes)) => modes.iter().map(|m| guid_of(&msg(vec![("guid", m.get("id").cloned().expect("a mode id"))]))).collect(),
        other => panic!("no modes: {other:?}"),
    };
    assert_eq!(set_modes, BTreeSet::from([LIGHT]), "the set is Studio's, with the one mode it has left");
    let var = record_of(&saved, COLOR_VAR);
    assert_eq!(var.get("version"), Some(&text("5:5")), "the variable is still written from the file's record");
    let Some(V::Array(entries)) = var.get("variableDataValues").and_then(|v| v.get("entries")) else { panic!("no values") };
    let modes: Vec<(u32, u32)> = entries.iter().map(|e| guid_of(&msg(vec![("guid", e.get("modeID").cloned().expect("a mode id"))]))).collect();
    assert_eq!(modes, vec![LIGHT], "the value for the mode that is gone is not written: {entries:?}");
}

#[test]
fn deleting_a_mode_through_the_variable_store_writes_studios_record_with_no_dangling_mode() {
    let original = file_of(live_records());
    let mut doc = import_fig(&original).expect("imports");
    let mut store = designos_doc::VariableStore::from_doc(&doc);
    assert!(store.delete_mode("VariableCollectionId:7:20", "7:22"));
    doc.variables = store.variables;
    doc.variable_collections = store.collections;
    let saved = saved_of(&doc);
    let var = record_of(&saved, COLOR_VAR);
    assert!(var.get("version").is_none(), "the store's delete edits the variable too, so it is Studio's record");
    let Some(V::Array(entries)) = var.get("variableDataValues").and_then(|v| v.get("entries")) else { panic!("no values") };
    assert_eq!(entries.len(), 1, "{entries:?}");
}

#[test]
fn the_records_and_layers_of_figmas_internal_canvas_keep_the_guid_of_their_parent() {
    let mut nodes = live_records();
    nodes.push(node((7, 60), INTERNAL, "c", "SYMBOL", "Library component", vec![("size", vec2(20.0, 20.0))]));
    let original = file_of(nodes);
    let doc = import_fig(&original).expect("imports");
    let saved = saved_of(&doc);
    assert_eq!(parent_of(record_of(&saved, (7, 60))), INTERNAL, "a layer of the internal canvas");
    for id in [SET, COLOR_VAR, PAINT_STYLE, TEXT_STYLE] {
        assert_eq!(parent_of(record_of(&saved, id)), INTERNAL, "{id:?}");
    }
    let canvas = record_of(&saved, INTERNAL);
    assert_eq!(canvas.get("internalOnly"), Some(&V::Bool(true)));
    assert!(find(&saved, (0, 999_999)).is_none(), "Studio's own guid is not written when the file gave one");
}

#[test]
fn the_internal_canvas_keeps_its_guid_when_only_styles_sit_on_it() {
    let mut nodes = document_and_page();
    nodes.push(internal_canvas());
    nodes.push(paint_style(PAINT_STYLE, "Brand/Primary", "a", "paint-key", 0.25, false));
    let doc = import_fig(&file_of(nodes)).expect("imports");
    let saved = saved_of(&doc);
    assert_eq!(parent_of(record_of(&saved, PAINT_STYLE)), INTERNAL);
    assert!(find(&saved, INTERNAL).is_some(), "the canvas is written under the file's guid");
    assert!(find(&saved, (0, 999_999)).is_none());
}

#[test]
fn a_file_with_no_internal_canvas_still_writes_studios_own() {
    let mut nodes = document_and_page();
    nodes.push(node((2, 1), (0, 1), "a", "FRAME", "Frame", vec![("size", vec2(10.0, 10.0))]));
    let mut doc = import_fig(&file_of(nodes)).expect("imports");
    let mut store = designos_doc::VariableStore::from_doc(&doc);
    let coll = store.add_collection("Mine");
    store.add_variable(&coll, "Primary", designos_doc::VariableKind::Color);
    doc.variables = store.variables;
    doc.variable_collections = store.collections;
    let saved = saved_of(&doc);
    let canvas = saved.iter().find(|r| r.get("internalOnly") == Some(&V::Bool(true))).expect("an internal canvas");
    assert_eq!(guid_of(canvas), (0, 999_999));
}

#[test]
fn deleted_records_that_name_each_other_are_written_together_and_one_that_names_nothing_takes_its_dependents_with_it() {
    let mut nodes = live_records();
    let deleted_set = (7, 70);
    nodes.push(node(
        deleted_set,
        INTERNAL,
        "d",
        "VARIABLE_SET",
        "Gone",
        vec![
            ("isSoftDeleted", V::Bool(true)),
            ("variableSetModes", V::Array(vec![msg(vec![("id", guid(7, 71)), ("name", text("Only")), ("sortPosition", text("!"))])])),
        ],
    ));
    // B is in the deleted set; A aliases B; C aliases a variable the file does not hold; D aliases C
    let b = variable((7, 81), deleted_set, "B", vec![value_entry((7, 71), 0.5)], true);
    let a = variable((7, 80), SET, "A", vec![alias_entry(LIGHT, (7, 81))], true);
    let c = variable((7, 82), SET, "C", vec![alias_entry(LIGHT, (7, 99))], true);
    let d = variable((7, 83), SET, "D", vec![alias_entry(LIGHT, (7, 82))], true);
    nodes.extend([b, a, c, d]);
    let original = file_of(nodes);
    let before = records(&original);
    let doc = import_fig(&original).expect("imports");
    let (saved_bytes, report) = designos_fig::export_fig_with_kept_report(&doc).expect("saves");
    let saved = records(&saved_bytes);
    for (id, what) in [(deleted_set, "the deleted set"), ((7, 81), "B"), ((7, 80), "A")] {
        assert_record_kept(&before, &saved, id, what);
    }
    assert!(find(&saved, (7, 82)).is_none(), "C names a variable that is not written");
    assert!(find(&saved, (7, 83)).is_none(), "D names C, which is not written");
    let records_report = &report.records;
    assert_eq!(records_report.soft_deleted_written.values().sum::<usize>(), 3, "{records_report:?}");
    assert_eq!(records_report.soft_deleted_dropped.values().sum::<usize>(), 2, "{records_report:?}");
    assert!(records_report.soft_deleted_dropped.keys().all(|k| k.starts_with("VARIABLE")), "dropped records are counted by type: {records_report:?}");
    let guids: Vec<(u32, u32)> = saved.iter().map(guid_of).collect();
    assert_eq!(guids.len(), guids.iter().collect::<BTreeSet<_>>().len(), "no guid is written twice");
    // the file opens again
    import_fig(&saved_bytes).expect("the saved file opens");
}

#[test]
fn a_deleted_variable_a_layer_still_uses_is_written_once() {
    let mut nodes = live_records();
    let referenced = variable((7, 90), SET, "Used", vec![value_entry(LIGHT, 0.7)], true);
    nodes.push(referenced);
    let mut frame = node((2, 2), (0, 1), "b", "FRAME", "Banner", vec![("size", vec2(10.0, 10.0))]);
    let mut paint = solid(0.7, 0.2, 0.3);
    set_field(&mut paint, "colorVar", msg(vec![("dataType", en("ALIAS")), ("value", msg(vec![("alias", msg(vec![("guid", guid(7, 90))]))]))]));
    set_field(&mut frame, "fillPaints", V::Array(vec![paint]));
    nodes.push(frame);
    let doc = import_fig(&file_of(nodes)).expect("imports");
    assert!(doc.variables.contains_key("VariableID:7:90"), "a deleted variable a layer uses stays in the document");
    let saved = saved_of(&doc);
    let guids: Vec<(u32, u32)> = saved.iter().map(guid_of).collect();
    assert_eq!(guids.iter().filter(|g| **g == (7, 90)).count(), 1, "the record is written once");
    assert_eq!(record_of(&saved, (7, 90)).get("isSoftDeleted"), Some(&V::Bool(true)));
}

#[test]
fn a_style_variable_or_set_the_user_deleted_is_not_brought_back_from_the_store() {
    let original = file_of(live_records());
    let mut doc = import_fig(&original).expect("imports");
    doc.styles.remove("7:40");
    doc.style_meta.remove("7:40");
    doc.variables.remove("VariableID:7:30");
    doc.variable_collections.get_mut("VariableCollectionId:7:20").expect("the set").variables.clear();
    let saved = saved_of(&doc);
    assert!(find(&saved, PAINT_STYLE).is_none(), "a style the user deleted is gone: it is live in the file, not deleted there");
    assert!(find(&saved, COLOR_VAR).is_none(), "a variable the user deleted is gone");
    assert!(find(&saved, SET).is_some(), "the set is still there");
}

#[test]
fn a_deleted_record_whose_parent_this_save_does_not_write_is_left_out_and_counted() {
    let mut nodes = document_and_page();
    nodes.push(internal_canvas());
    let mut orphan = paint_style(DELETED_STYLE, "Old/Brand", "a", "old-key", 0.4, true);
    set_field(&mut orphan, "parentIndex", st(vec![("guid", guid(7, 999)), ("position", text("a"))]));
    nodes.push(orphan);
    nodes.push(paint_style(PAINT_STYLE, "Brand/Primary", "b", "paint-key", 0.25, true));
    let doc = import_fig(&file_of(nodes)).expect("imports");
    let (saved_bytes, report) = designos_fig::export_fig_with_kept_report(&doc).expect("saves");
    let saved = records(&saved_bytes);
    assert!(find(&saved, DELETED_STYLE).is_none(), "its parent is not a record of this file");
    assert!(find(&saved, PAINT_STYLE).is_some(), "a deleted record under the internal canvas is written");
    assert_eq!(report.records.soft_deleted_written.values().sum::<usize>(), 1, "{:?}", report.records);
    assert_eq!(report.records.soft_deleted_dropped.get("ROUNDED_RECTANGLE/FILL: parent_not_written"), Some(&1), "{:?}", report.records);
}

#[test]
fn a_no_edit_save_counts_no_entry_as_edited_and_writes_every_record_from_the_file() {
    let original = file_of(live_records());
    let doc = import_fig(&original).expect("imports");
    let (_, report) = designos_fig::export_fig_with_kept_report(&doc).expect("saves");
    let r = &report.records;
    assert_eq!((r.styles_edited, r.variables_edited, r.sets_edited, r.internal_canvas_edited), (0, 0, 0, 0), "{r:?}");
    assert_eq!((r.styles_written, r.variables_written, r.sets_written, r.internal_canvas_written), (2, 1, 1, 1), "{r:?}");
    assert_eq!(r.fields_dropped(), 0, "{r:?}");
    assert_eq!(r.internal_canvas_guid, Some(INTERNAL));
}

#[test]
fn a_large_file_is_saved_as_a_small_one_is_text_style_glyphs_included() {
    let original = large_file_of(live_records());
    let before = records(&original);
    let doc = import_fig(&original).expect("imports");
    let (saved_bytes, report) = designos_fig::export_fig_with_kept_report(&doc).expect("saves");
    let (saved, saved_blobs) = (records(&saved_bytes), blobs(&saved_bytes));
    for (id, what) in [(PAINT_STYLE, "paint style"), (TEXT_STYLE, "text style"), (COLOR_VAR, "variable"), (SET, "variable set"), (INTERNAL, "internal canvas")]
    {
        assert_record_kept(&before, &saved, id, what);
    }
    assert_stored_glyphs_kept(&saved, &saved_blobs, TEXT_STYLE, "text style of a large file");
    assert_eq!(report.records.fields_dropped(), 0, "{:?}", report.records);
}

#[test]
fn a_deleted_text_style_of_a_large_file_is_written_with_its_stored_glyphs() {
    let mut nodes = document_and_page();
    nodes.push(internal_canvas());
    let mut deleted_text = text_style(DELETED_STYLE, "a");
    set_field(&mut deleted_text, "isSoftDeleted", V::Bool(true));
    set_field(&mut deleted_text, "isSoftDeletedStyle", V::Bool(true));
    nodes.push(deleted_text);
    let original = large_file_of(nodes);
    let before = records(&original);
    let doc = import_fig(&original).expect("imports");
    let saved_bytes = export_fig(&doc).expect("saves");
    let (saved, saved_blobs) = (records(&saved_bytes), blobs(&saved_bytes));
    assert_record_kept(&before, &saved, DELETED_STYLE, "deleted text style");
    assert_stored_glyphs_kept(&saved, &saved_blobs, DELETED_STYLE, "deleted text style of a large file");
}
