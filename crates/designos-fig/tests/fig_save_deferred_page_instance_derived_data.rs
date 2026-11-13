//! A `.fig` save of a document with pages the loader has not opened keeps the derived data of their instances (the sizes,
//! transforms and outlines the producing app computed for the nodes inside them), as long as the main component they
//! were computed from is as it was when the file was opened.

mod common;

use common::deferred_fig::*;
use common::fig_fixture::*;
use designos_doc::*;
use designos_fig::{export_fig, import_fig};
use serde_json::{json, Value};

const DOT: (u64, u64) = (1, 13);
const SECOND_DOT: (u64, u64) = (1, 14);
const WANTED_BLOB: usize = 3;

fn matrix(tx: f64, ty: f64) -> Value {
    json!({"m00": 1.0, "m01": 0.0, "m02": tx, "m10": 0.0, "m11": 1.0, "m12": ty})
}

/// Sizes and transforms for the label and two dots of the button, and an outline for each dot: both name blob 3 of the
/// file, one filled non-zero and one even-odd.
fn derived_data() -> Value {
    json!([
        {"guidPath": {"guids": [guid(LABEL.0, LABEL.1)]}, "size": {"x": 90.0, "y": 20.0}, "transform": matrix(10.0, 14.0)},
        {
            "guidPath": {"guids": [guid(DOT.0, DOT.1)]},
            "size": {"x": 12.0, "y": 12.0},
            "transform": matrix(4.0, 4.0),
            "strokeGeometry": [{"windingRule": "NONZERO", "commandsBlob": WANTED_BLOB}]
        },
        {
            "guidPath": {"guids": [guid(SECOND_DOT.0, SECOND_DOT.1)]},
            "size": {"x": 6.0, "y": 6.0},
            "transform": matrix(20.0, 4.0),
            "strokeGeometry": [{"windingRule": "ODD", "commandsBlob": WANTED_BLOB}]
        }
    ])
}

fn dots() -> Vec<Value> {
    [(DOT, "Dot"), (SECOND_DOT, "Second dot")]
        .into_iter()
        .map(|(id, name)| fig_node(id, BUTTON, "ELLIPSE", name, json!({"size": {"x": 8.0, "y": 8.0}})))
        .collect()
}

fn blobs() -> Vec<Vec<u8>> {
    vec![line_blob(1.0), line_blob(2.0), line_blob(3.0), line_blob(7.0)]
}

fn derived_fixture(extra: Vec<Value>) -> Vec<u8> {
    let mut nodes = dots();
    nodes.extend(extra);
    fixture_with(json!({"derivedSymbolData": derived_data()}), text_and_stroke_overrides(), nodes, blobs())
}

/// The size the reopened file gives the label inside the variant's button.
fn reopened_label_size(file: &[u8]) -> Vec2 {
    let mut doc = import_fig(file).expect("reopen");
    doc.hydrate_all_pages();
    let button = variant_button(&doc);
    doc.get(button).expect("button").children.iter().filter_map(|&c| doc.get(c)).find(|n| matches!(n.kind, NodeKind::Text(_))).expect("label").size
}

fn edit_size(doc: &mut Document, name: &str, parent_name: &str) {
    let id = doc
        .nodes
        .values()
        .find(|n| n.name == name && n.parent.and_then(|p| doc.get(p)).is_some_and(|p| p.name == parent_name))
        .map(|n| n.id)
        .unwrap_or_else(|| panic!("no {name} in {parent_name}"));
    doc.get_mut(id).expect("node").size.x += 5.0;
}

#[test]
fn opening_a_deferred_page_after_main_edit_does_not_apply_stale_derived_size() {
    let fig = derived_fixture(Vec::new());
    let mut live = import_fig(&fig).expect("import");
    let page = page_named(&live, "404 pages");
    let main_label = live
        .nodes
        .values()
        .find(|n| n.name == "Text" && n.parent.and_then(|p| live.get(p)).is_some_and(|p| p.name == "Buttons/Button"))
        .map(|n| n.id)
        .expect("main label");
    assert!(!page_hydrated(&live, page));
    live.get_mut(main_label).expect("main label").size = Vec2::new(125.0, 48.0);
    assert!(live.ensure_page_hydrated(page), "page opens");
    let instance = variant_button(&live);
    let label = live
        .get(instance)
        .expect("hydrated instance")
        .children
        .iter()
        .filter_map(|&c| live.get(c))
        .find(|n| matches!(n.kind, NodeKind::Text(_)))
        .expect("hydrated label");
    assert_eq!(label.size, Vec2::new(125.0, 48.0), "page hydration must use the edited main, not stale derived 90x20");
}

#[test]
fn opening_a_deferred_page_after_main_edit_keeps_instance_overrides() {
    let fig = derived_fixture(Vec::new());
    let mut live = import_fig(&fig).expect("import");
    let page = page_named(&live, "404 pages");
    let main_label = live
        .nodes
        .values()
        .find(|n| n.name == "Text" && n.parent.and_then(|p| live.get(p)).is_some_and(|p| p.name == "Buttons/Button"))
        .map(|n| n.id)
        .expect("main label");
    assert!(!page_hydrated(&live, page));
    live.get_mut(main_label).expect("main label").size = Vec2::new(125.0, 48.0);
    assert!(live.ensure_page_hydrated(page), "page opens");

    let instance = variant_button(&live);
    assert_eq!(label_of(&live, instance), "Go back", "clearing stale derived data must keep text override on the instance");
    let icon = live.get(instance).expect("hydrated instance").children.iter().filter_map(|&c| live.get(c)).find(|n| n.name == "Icon").expect("hydrated icon");
    assert!((icon.stroke.weight - 1.6666666).abs() < 1e-5, "clearing stale derived data must keep stroke override on the instance");

    let saved = export_fig(&live).expect("export");
    assert_eq!(reopened_label_size(&saved), Vec2::new(125.0, 48.0), "saving after opening the page must not write stale derived size back");
    assert_eq!(reopened_label(&saved), "Go back", "override survives save after opening the page");
}

#[test]
fn a_deferred_instance_keeps_its_derived_data_through_a_save() {
    let fig = derived_fixture(Vec::new());
    let original = saved_derived(&fig, INSTANCE);
    assert_eq!(original.len(), 3, "the fixture states three entries");

    let live = import_fig(&fig).expect("import");
    let page = page_named(&live, "404 pages");
    let saved = export_fig(&live).expect("export");

    assert!(!page_hydrated(&live, page), "the page stays unloaded");
    let kept = saved_derived(&saved, INSTANCE);
    assert_eq!(kept.len(), original.len(), "every entry is written");
    for (was, now) in original.iter().zip(&kept) {
        assert_eq!(now["guidPath"], was["guidPath"]);
        assert_eq!(now["size"], was["size"]);
        assert_eq!(now["transform"], was["transform"]);
    }
    let blob_of = |entry: &Value, bytes_of: &dyn Fn(u64) -> Vec<u8>| bytes_of(entry["strokeGeometry"][0]["commandsBlob"].as_u64().expect("an outline"));
    let in_original = blob_of(&original[1], &|i| saved_blob(&fig, i));
    let in_saved = blob_of(&kept[1], &|i| saved_blob(&saved, i));
    assert_eq!(in_saved, in_original, "the outline is the one the file stated, wherever the saved table keeps it");
    assert_eq!(kept[1]["strokeGeometry"][0]["commandsBlob"], kept[2]["strokeGeometry"][0]["commandsBlob"], "an outline that two entries share is stored once");
    assert_eq!(kept[1]["strokeGeometry"][0]["windingRule"], "NONZERO");
    assert_eq!(kept[2]["strokeGeometry"][0]["windingRule"], "ODD", "each entry keeps its own winding rule");

    let without = fixture_with(json!({}), text_and_stroke_overrides(), dots(), blobs());
    assert_ne!(reopened_label_size(&fig), reopened_label_size(&without), "the derived data changes what a reader shows");
    assert_eq!(reopened_label_size(&saved), reopened_label_size(&fig), "a reopened save shows the instance as the original does");
}

#[test]
fn after_its_main_component_is_edited_the_derived_data_is_omitted_and_the_overrides_are_kept() {
    let fig = derived_fixture(Vec::new());
    let mut live = import_fig(&fig).expect("import");
    edit_size(&mut live, "Text", "Buttons/Button");

    let saved = export_fig(&live).expect("export");
    assert!(saved_derived(&saved, INSTANCE).is_empty(), "derived data computed for the old content is not written");
    assert_eq!(saved_entries(&saved, INSTANCE), saved_entries(&fig, INSTANCE), "the override entries are");
}

#[test]
fn after_a_main_component_inside_its_main_is_edited_the_derived_data_of_that_instance_alone_is_omitted() {
    // `Card` (1:40) holds an instance of `Badge` (1:30); the instance of `Card` on the deferred page (1:50) has derived
    // data, like the instance of the button.
    let extra = vec![
        fig_node((1, 30), (0, 3), "SYMBOL", "Badge", json!({})),
        fig_node((1, 40), (0, 3), "SYMBOL", "Card", json!({})),
        fig_node((1, 41), (1, 40), "INSTANCE", "Badge", json!({"symbolData": {"symbolID": guid(1, 30)}})),
        fig_node(
            (1, 50),
            VARIANT,
            "INSTANCE",
            "Card",
            json!({
                "symbolData": {"symbolID": guid(1, 40)},
                "derivedSymbolData": [{"guidPath": {"guids": [guid(1, 41)]}, "size": {"x": 33.0, "y": 11.0}, "transform": matrix(1.0, 2.0)}]
            }),
        ),
    ];
    let fig = derived_fixture(extra);
    let live = import_fig(&fig).expect("import");
    let saved = export_fig(&live).expect("export");
    assert_eq!(saved_derived(&saved, (1, 50)).len(), 1, "unedited, the card instance keeps its derived data");

    let mut live = import_fig(&fig).expect("import");
    let badge = live.nodes.values().find(|n| n.name == "Badge" && matches!(n.kind, NodeKind::Component(_))).map(|n| n.id).expect("badge component");
    live.get_mut(badge).expect("badge").size.x += 5.0;
    let saved = export_fig(&live).expect("export");
    assert!(saved_derived(&saved, (1, 50)).is_empty(), "the card contains the edited main");
    assert_eq!(saved_derived(&saved, INSTANCE).len(), 3, "the button does not, so its instance keeps its derived data");
}

#[test]
fn opening_another_page_keeps_the_derived_data_of_instances_on_pages_still_unopened() {
    // The instance on "Page 2" expands when that page is opened; the one on "404 pages" is still unopened.
    let on_page_2 = fig_node(
        (1, 60),
        (0, 2),
        "INSTANCE",
        "Buttons/Button",
        json!({"symbolData": {"symbolID": guid(BUTTON.0, BUTTON.1), "symbolOverrides": text_and_stroke_overrides()}, "derivedSymbolData": derived_data()}),
    );
    let fig = derived_fixture(vec![on_page_2]);
    let mut live = import_fig(&fig).expect("import");
    let (page_2, page_3) = (page_named(&live, "Page 2"), page_named(&live, "404 pages"));
    assert!(live.ensure_page_hydrated(page_2), "page 2 opens");
    assert!(!page_hydrated(&live, page_3), "page 3 does not");

    let saved = export_fig(&live).expect("export");
    assert_eq!(saved_derived(&saved, INSTANCE).len(), 3, "the instance on the unopened page keeps its derived data");
    assert!(saved_derived(&saved, (1, 60)).is_empty(), "the instance on the opened page is written from its children");
    assert!(children_written_under(&saved, (1, 60)) > 0);
}

#[test]
fn a_deferred_instance_pointed_at_another_component_gets_no_derived_data() {
    let fig = derived_fixture(Vec::new());
    let mut live = import_fig(&fig).expect("import");
    let other = live.nodes.values().find(|n| n.name == "Type=Simple 03, Breakpoint=Desktop").map(|n| n.id).expect("another component");
    let instance = variant_button(&live);
    let NodeKind::Instance { component, .. } = &mut live.get_mut(instance).expect("instance").kind else { panic!("instance") };
    *component = Some(other);

    let saved = export_fig(&live).expect("export");
    assert!(saved_derived(&saved, INSTANCE).is_empty(), "derived data computed for one component is not written under another");
}

#[test]
fn a_failed_page_hydration_after_a_main_edit_preserves_the_instance_derived_data() {
    let on_page_2 = fig_node(
        (1, 60),
        (0, 2),
        "INSTANCE",
        "Buttons/Button",
        json!({"symbolData": {"symbolID": guid(BUTTON.0, BUTTON.1), "symbolOverrides": text_and_stroke_overrides()}, "derivedSymbolData": derived_data()}),
    );
    let fig = derived_fixture(vec![on_page_2]);
    let mut live = import_fig(&fig).expect("import");
    let page = page_named(&live, "404 pages");
    let instance = variant_button(&live);
    let before = live.pending_instance_derived(instance).expect("derived data before hydration attempt");
    assert_eq!(before.entries.len(), 3);

    edit_size(&mut live, "Text", "Buttons/Button");
    live.reserve_ids(NodeId(u64::MAX - 2));
    assert!(!live.ensure_page_hydrated(page), "hydration that exhausts node id space must fail");
    let err = live.hydration_error().expect("hydration error must be recorded");
    assert!(err.contains("node id space exhausted"), "{err}");

    let after = live.pending_instance_derived(instance);
    assert_eq!(
        after.as_ref().map(|d| (d.fingerprint, d.entries.len(), d.entries[0].size)),
        Some((before.fingerprint, before.entries.len(), before.entries[0].size)),
        "failed hydration must leave the instance's derived data as it was before the attempt"
    );
}
