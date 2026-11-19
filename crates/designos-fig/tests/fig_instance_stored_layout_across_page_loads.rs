//! A `.fig` stores, for an instance, the layout Figma computed for it from its main component. Opening a deferred page
//! applies that layout only while the main components it was computed from are as the file had them. Opening another
//! page fills instances inside those mains (and the nested instances of a main an instance swaps to): the loader's own
//! writes, which must not count as an edit. A real edit, before or after such a write, still must.

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
/// An instance inside `Icon/B`, which no instance of the file names directly, so it is filled when a page that swaps
/// to `Icon/B` (or its own page) opens.
const INNER: (u64, u64) = (1, 82);
const ON_PAGE_2: (u64, u64) = (2, 1);
const ON_PAGE_3: (u64, u64) = (3, 1);
const STORED_LABEL: Vec2 = Vec2 { x: 90.0, y: 20.0 };

fn matrix(tx: f64, ty: f64) -> Value {
    json!({"m00": 1.0, "m01": 0.0, "m02": tx, "m10": 0.0, "m11": 1.0, "m12": ty})
}

/// What the file stores for a button whose nested icon was swapped to `Icon/B`: the label's laid-out size.
fn stored_layout() -> Value {
    json!([{"guidPath": {"guids": [guid(LABEL.0, LABEL.1)]}, "size": {"x": 90.0, "y": 20.0}, "transform": matrix(10.0, 14.0)}])
}

fn swap_to_icon_b() -> Value {
    json!({"guidPath": {"guids": [guid(NESTED.0, NESTED.1)]}, "overriddenSymbolID": guid(ICON_B.0, ICON_B.1)})
}

fn button_on_page(id: (u64, u64), page: u64, name: &str) -> Value {
    fig_node(
        id,
        (0, page),
        "INSTANCE",
        name,
        json!({
            "symbolData": {"symbolID": guid(BUTTON.0, BUTTON.1), "symbolOverrides": [swap_to_icon_b()]},
            "derivedSymbolData": stored_layout()
        }),
    )
}

/// Three icon mains on the last page, `Icon/B` holding an instance of `Icon/C` that has a file record. The button holds
/// a nested instance of `Icon/A`. Pages 2 and 3 each hold a button swapped to `Icon/B` with the stored layout; the
/// variant's own button on the last page is left as it is.
fn fixture_of_swapped_buttons() -> Vec<u8> {
    let icon = |id, name| fig_node(id, (0, 3), "SYMBOL", name, json!({"size": {"x": 8.0, "y": 8.0}}));
    let shape = |id, parent, name| fig_node(id, parent, "ROUNDED_RECTANGLE", name, json!({"size": {"x": 8.0, "y": 8.0}}));
    let nodes = vec![
        icon(ICON_A, "Icon/A"),
        shape(ICON_A_SHAPE, ICON_A, "Shape A"),
        icon(ICON_B, "Icon/B"),
        shape(ICON_B_SHAPE, ICON_B, "Shape B"),
        fig_node(
            INNER,
            ICON_B,
            "INSTANCE",
            "Inner",
            json!({
                "size": {"x": 8.0, "y": 8.0},
                "symbolData": {
                    "symbolID": guid(ICON_C.0, ICON_C.1),
                    "symbolOverrides": [{"guidPath": {"guids": [guid(ICON_C_SHAPE.0, ICON_C_SHAPE.1)]}, "name": "Renamed"}]
                }
            }),
        ),
        icon(ICON_C, "Icon/C"),
        shape(ICON_C_SHAPE, ICON_C, "Shape C"),
        fig_node(NESTED, BUTTON, "INSTANCE", "Slot icon", json!({"size": {"x": 8.0, "y": 8.0}, "symbolData": {"symbolID": guid(ICON_A.0, ICON_A.1)}})),
        fig_node((0, 4), (0, 0), "CANVAS", "Page 3", json!({})),
        button_on_page(ON_PAGE_2, 2, "Button on page 2"),
        button_on_page(ON_PAGE_3, 4, "Button on page 3"),
    ];
    fixture_with(json!({}), text_and_stroke_overrides(), nodes, Vec::new())
}

fn main_named(doc: &Document, name: &str) -> NodeId {
    doc.nodes.values().find(|n| n.name == name && matches!(n.kind, NodeKind::Component(_))).map(|n| n.id).unwrap_or_else(|| panic!("no main {name}"))
}

fn instance_named(doc: &Document, name: &str) -> NodeId {
    doc.nodes.values().find(|n| n.name == name && matches!(n.kind, NodeKind::Instance { .. })).map(|n| n.id).unwrap_or_else(|| panic!("no instance {name}"))
}

fn open(doc: &mut Document, page: &str) {
    let id = page_named(doc, page);
    assert!(!page_hydrated(doc, id), "{page} is still unopened");
    assert!(doc.ensure_page_hydrated(id), "{page} opens");
}

fn label_size(doc: &Document, instance: NodeId) -> Vec2 {
    doc.get(instance).expect("instance").children.iter().filter_map(|&c| doc.get(c)).find(|n| matches!(n.kind, NodeKind::Text(_))).expect("label").size
}

/// Whether the instance shows the layout the file stored for it, and holds the record that keeps it through a sync.
fn keeps_stored_layout(doc: &Document, instance: NodeId) -> bool {
    label_size(doc, instance) == STORED_LABEL && doc.get(instance).expect("instance").component.imported_layout.is_some()
}

fn widen_shape_of_icon_b(doc: &mut Document) {
    let shape = doc.get(main_named(doc, "Icon/B")).expect("main").children[0];
    doc.get_mut(shape).expect("shape").size.x += 22.0;
}

fn stored_entries_written(doc: &Document, instance: (u64, u64)) -> usize {
    saved_derived(&export_fig(doc).expect("export"), instance).len()
}

#[test]
fn the_fixture_defers_every_page_with_a_button() {
    let fig = fixture_of_swapped_buttons();
    assert_eq!(saved_derived(&fig, ON_PAGE_2).len(), 1);
    assert_eq!(saved_derived(&fig, ON_PAGE_3).len(), 1);
    let doc = import_fig(&fig).expect("import");
    for page in ["Page 2", "Page 3", "404 pages"] {
        assert!(!page_hydrated(&doc, page_named(&doc, page)), "{page} stays unopened");
    }
}

#[test]
fn a_page_keeps_its_stored_layout_after_another_page_filled_the_main_it_swaps_to() {
    let mut doc = import_fig(&fixture_of_swapped_buttons()).expect("import");
    open(&mut doc, "Page 2");
    open(&mut doc, "Page 3");
    let on_2 = instance_named(&doc, "Button on page 2");
    let on_3 = instance_named(&doc, "Button on page 3");
    assert!(keeps_stored_layout(&doc, on_2), "the first page to open has its layout");
    assert!(keeps_stored_layout(&doc, on_3), "so has the second, although the first filled Icon/B");
}

#[test]
fn a_page_keeps_its_stored_layout_after_another_page_filled_an_instance_inside_a_main() {
    let mut doc = import_fig(&fixture_of_swapped_buttons()).expect("import");
    // The main components' own page opens first and fills the instance inside Icon/B.
    open(&mut doc, "404 pages");
    open(&mut doc, "Page 2");
    open(&mut doc, "Page 3");
    assert!(keeps_stored_layout(&doc, instance_named(&doc, "Button on page 2")));
    assert!(keeps_stored_layout(&doc, instance_named(&doc, "Button on page 3")));
}

#[test]
fn opening_the_pages_in_the_other_order_gives_the_same_layout() {
    let layouts = |order: [&str; 3]| {
        let mut doc = import_fig(&fixture_of_swapped_buttons()).expect("import");
        for page in order {
            open(&mut doc, page);
        }
        (label_size(&doc, instance_named(&doc, "Button on page 2")), label_size(&doc, instance_named(&doc, "Button on page 3")))
    };
    assert_eq!(layouts(["Page 2", "Page 3", "404 pages"]), layouts(["404 pages", "Page 3", "Page 2"]));
    assert_eq!(layouts(["Page 3", "404 pages", "Page 2"]), (STORED_LABEL, STORED_LABEL));
}

#[test]
fn a_save_keeps_the_stored_layout_of_an_unopened_page_after_another_page_filled_the_main() {
    let mut doc = import_fig(&fixture_of_swapped_buttons()).expect("import");
    open(&mut doc, "Page 2");
    assert!(!page_hydrated(&doc, page_named(&doc, "Page 3")));
    assert_eq!(stored_entries_written(&doc, ON_PAGE_3), 1, "the layout stored for the unopened page is written back");
}

#[test]
fn an_edit_of_the_main_before_another_page_fills_it_still_refuses_the_stored_layout() {
    let mut doc = import_fig(&fixture_of_swapped_buttons()).expect("import");
    widen_shape_of_icon_b(&mut doc);
    open(&mut doc, "Page 2");
    open(&mut doc, "Page 3");
    let on_3 = instance_named(&doc, "Button on page 3");
    assert_ne!(label_size(&doc, on_3), STORED_LABEL, "the layout was computed for the main as it was");
    assert!(doc.get(on_3).expect("instance").component.imported_layout.is_none());
}

#[test]
fn an_edit_of_the_main_after_another_page_filled_it_still_refuses_the_stored_layout() {
    let mut doc = import_fig(&fixture_of_swapped_buttons()).expect("import");
    open(&mut doc, "Page 2");
    widen_shape_of_icon_b(&mut doc);
    open(&mut doc, "Page 3");
    let on_3 = instance_named(&doc, "Button on page 3");
    assert_ne!(label_size(&doc, on_3), STORED_LABEL);
    assert!(doc.get(on_3).expect("instance").component.imported_layout.is_none());
}

#[test]
fn a_save_after_an_edit_that_followed_the_loader_writes_omits_the_stored_layout() {
    let mut doc = import_fig(&fixture_of_swapped_buttons()).expect("import");
    open(&mut doc, "Page 2");
    widen_shape_of_icon_b(&mut doc);
    assert_eq!(stored_entries_written(&doc, ON_PAGE_3), 0, "the layout stored for the old Icon/B is not written");
}

#[test]
fn an_edit_of_the_main_a_swapped_nested_instance_started_from_still_refuses_the_stored_layout() {
    let mut doc = import_fig(&fixture_of_swapped_buttons()).expect("import");
    open(&mut doc, "Page 2");
    let shape = doc.get(main_named(&doc, "Icon/A")).expect("main").children[0];
    // Icon/A is the main of the nested instance before the swap, which the button's stored layout was computed with.
    doc.get_mut(shape).expect("shape").size.x += 22.0;
    open(&mut doc, "Page 3");
    let on_3 = instance_named(&doc, "Button on page 3");
    assert_ne!(label_size(&doc, on_3), STORED_LABEL, "Icon/A is part of what the layout depends on");
    assert!(doc.get(on_3).expect("instance").component.imported_layout.is_none());
}
