//! The derived data of an unopened instance was computed against every main its nested instances resolve to, which
//! includes the mains its file record swaps them to and the mains its own instance-swap properties name. A `.fig` save
//! writes it only while all of those are as they were when the file was opened.

mod common;

use common::deferred_fig::*;
use common::fig_fixture::*;
use designos_doc::*;
use designos_fig::{export_fig, import_fig};
use serde_json::{json, Value};

const NESTED: (u64, u64) = (1, 15);
const ICON_A: (u64, u64) = (1, 70);
const ICON_A_SHAPE: (u64, u64) = (1, 71);
const ICON_B: (u64, u64) = (1, 80);
const ICON_B_SHAPE: (u64, u64) = (1, 81);
const ICON_C: (u64, u64) = (1, 90);
const ICON_C_SHAPE: (u64, u64) = (1, 91);
const SWAPPED_OUTLINE_BLOB: usize = 3;

fn matrix(tx: f64, ty: f64) -> Value {
    json!({"m00": 1.0, "m01": 0.0, "m02": tx, "m10": 0.0, "m11": 1.0, "m12": ty})
}

fn blobs() -> Vec<Vec<u8>> {
    vec![line_blob(1.0), line_blob(2.0), line_blob(3.0), line_blob(7.0)]
}

/// Three icon mains. The button holds a nested instance of `Icon/A`; the instance on the unopened page swaps it to
/// `Icon/B` and has derived data for the label and for the shape inside the swapped-in icon.
fn swap_fixture() -> Vec<u8> {
    let icon = |id, name| fig_node(id, (0, 3), "SYMBOL", name, json!({"size": {"x": 8.0, "y": 8.0}}));
    let shape = |id, parent, name| fig_node(id, parent, "ROUNDED_RECTANGLE", name, json!({"size": {"x": 8.0, "y": 8.0}}));
    let nodes = vec![
        icon(ICON_A, "Icon/A"),
        shape(ICON_A_SHAPE, ICON_A, "Shape A"),
        icon(ICON_B, "Icon/B"),
        shape(ICON_B_SHAPE, ICON_B, "Shape B"),
        icon(ICON_C, "Icon/C"),
        shape(ICON_C_SHAPE, ICON_C, "Shape C"),
        fig_node(NESTED, BUTTON, "INSTANCE", "Slot icon", json!({"size": {"x": 8.0, "y": 8.0}, "symbolData": {"symbolID": guid(ICON_A.0, ICON_A.1)}})),
    ];
    let mut overrides = text_and_stroke_overrides();
    overrides.as_array_mut().expect("entries").push(json!({"guidPath": {"guids": [guid(NESTED.0, NESTED.1)]}, "overriddenSymbolID": guid(ICON_B.0, ICON_B.1)}));
    let derived = json!([
        {"guidPath": {"guids": [guid(LABEL.0, LABEL.1)]}, "size": {"x": 90.0, "y": 20.0}, "transform": matrix(10.0, 14.0)},
        {
            "guidPath": {"guids": [guid(NESTED.0, NESTED.1), guid(ICON_B_SHAPE.0, ICON_B_SHAPE.1)]},
            "size": {"x": 8.0, "y": 8.0},
            "transform": matrix(0.0, 0.0),
            "fillGeometry": [{"windingRule": "NONZERO", "commandsBlob": SWAPPED_OUTLINE_BLOB}]
        }
    ]);
    fixture_with(json!({"derivedSymbolData": derived}), overrides, nodes, blobs())
}

fn main_named(doc: &Document, name: &str) -> NodeId {
    doc.nodes.values().find(|n| n.name == name && matches!(n.kind, NodeKind::Component(_))).map(|n| n.id).unwrap_or_else(|| panic!("no main {name}"))
}

fn widen_shape_of(doc: &mut Document, main: &str) {
    let main = main_named(doc, main);
    let shape = doc.get(main).expect("main").children[0];
    doc.get_mut(shape).expect("shape").size.x += 22.0;
    let _ = sync_instances(doc, main).apply(doc);
}

fn written(doc: &Document) -> usize {
    saved_derived(&export_fig(doc).expect("export"), INSTANCE).len()
}

#[test]
fn the_fixture_states_derived_data_and_a_save_without_edits_writes_all_of_it() {
    let fig = swap_fixture();
    assert_eq!(saved_derived(&fig, INSTANCE).len(), 2);
    let live = import_fig(&fig).expect("import");
    assert!(!page_hydrated(&live, page_named(&live, "404 pages")), "the page stays unloaded");
    assert_eq!(written(&live), 2);
}

#[test]
fn editing_a_main_the_record_swaps_a_nested_instance_to_drops_the_derived_data() {
    let mut live = import_fig(&swap_fixture()).expect("import");
    widen_shape_of(&mut live, "Icon/B");
    assert_eq!(written(&live), 0, "the shape inside Icon/B changed, so the outline stated for it is stale");
}

#[test]
fn editing_the_main_a_nested_instance_resolves_to_without_the_swap_drops_the_derived_data() {
    let mut live = import_fig(&swap_fixture()).expect("import");
    widen_shape_of(&mut live, "Icon/A");
    assert_eq!(written(&live), 0);
}

#[test]
fn editing_a_main_nothing_in_the_instance_resolves_to_keeps_the_derived_data() {
    let mut live = import_fig(&swap_fixture()).expect("import");
    widen_shape_of(&mut live, "Icon/C");
    assert_eq!(written(&live), 2);
}

#[test]
fn changing_the_instance_swap_property_value_of_the_instance_drops_the_derived_data() {
    let mut live = import_fig(&swap_fixture()).expect("import");
    let instance = variant_button(&live);
    let icon_c = main_named(&live, "Icon/C");
    live.get_mut(instance).expect("instance").component.values.insert("Icon".into(), PropValue::Instance(icon_c));
    assert_eq!(written(&live), 0, "the nested icon now resolves to a main the stated data was not computed against");
}
