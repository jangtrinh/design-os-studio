//! An unopened page's instance keeps its record while the loader's own writes widen the set of mains its stored layout
//! depends on past what was trusted at open. The stored layout is then never accepted again, but the loader still holds
//! the record: the document must not fill the instance from its main, and a save must not write the layout.

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
const INNER: (u64, u64) = (1, 82);
const ICON_C: (u64, u64) = (1, 90);
const ICON_C_SHAPE: (u64, u64) = (1, 91);
const SLOT_IN_C: (u64, u64) = (1, 92);
const ICON_D: (u64, u64) = (1, 100);
const ICON_D_SHAPE: (u64, u64) = (1, 101);
const ICON_E: (u64, u64) = (1, 110);
const ICON_E_SHAPE: (u64, u64) = (1, 111);
const ON_PAGE_2: (u64, u64) = (2, 1);
const ON_PAGE_3: (u64, u64) = (3, 1);

fn matrix(tx: f64, ty: f64) -> Value {
    json!({"m00": 1.0, "m01": 0.0, "m02": tx, "m10": 0.0, "m11": 1.0, "m12": ty})
}

fn button_on_page(id: (u64, u64), page: u64, name: &str) -> Value {
    fig_node(
        id,
        (0, page),
        "INSTANCE",
        name,
        json!({
            "symbolData": {
                "symbolID": guid(BUTTON.0, BUTTON.1),
                "symbolOverrides": [{"guidPath": {"guids": [guid(NESTED.0, NESTED.1)]}, "overriddenSymbolID": guid(ICON_B.0, ICON_B.1)}]
            },
            "derivedSymbolData": [{"guidPath": {"guids": [guid(LABEL.0, LABEL.1)]}, "size": {"x": 90.0, "y": 20.0}, "transform": matrix(10.0, 14.0)}]
        }),
    )
}

/// Both buttons swap their nested icon to `Icon/B`, which no instance names directly. `Icon/B` holds an instance of
/// `Icon/C` whose record swaps the instance inside `Icon/C` from `Icon/E` to `Icon/D`: opening a page that swaps to
/// `Icon/B` puts an instance of `Icon/D` inside `Icon/B`, a main nobody reached when the file was opened.
fn fixture() -> Vec<u8> {
    let icon = |id, name| fig_node(id, (0, 3), "SYMBOL", name, json!({"size": {"x": 8.0, "y": 8.0}}));
    let shape = |id, parent, name| fig_node(id, parent, "ROUNDED_RECTANGLE", name, json!({"size": {"x": 8.0, "y": 8.0}}));
    let instance_of = |id, parent, name, main: (u64, u64), overrides: Value| {
        fig_node(
            id,
            parent,
            "INSTANCE",
            name,
            json!({"size": {"x": 8.0, "y": 8.0}, "symbolData": {"symbolID": guid(main.0, main.1), "symbolOverrides": overrides}}),
        )
    };
    let nodes = vec![
        icon(ICON_A, "Icon/A"),
        shape(ICON_A_SHAPE, ICON_A, "Shape A"),
        icon(ICON_B, "Icon/B"),
        shape(ICON_B_SHAPE, ICON_B, "Shape B"),
        instance_of(
            INNER,
            ICON_B,
            "Inner",
            ICON_C,
            json!([{"guidPath": {"guids": [guid(SLOT_IN_C.0, SLOT_IN_C.1)]}, "overriddenSymbolID": guid(ICON_D.0, ICON_D.1)}]),
        ),
        icon(ICON_C, "Icon/C"),
        shape(ICON_C_SHAPE, ICON_C, "Shape C"),
        instance_of(SLOT_IN_C, ICON_C, "Swappable", ICON_E, json!([])),
        icon(ICON_D, "Icon/D"),
        shape(ICON_D_SHAPE, ICON_D, "Shape D"),
        icon(ICON_E, "Icon/E"),
        shape(ICON_E_SHAPE, ICON_E, "Shape E"),
        instance_of(NESTED, BUTTON, "Slot icon", ICON_A, json!([])),
        fig_node((0, 4), (0, 0), "CANVAS", "Page 3", json!({})),
        button_on_page(ON_PAGE_2, 2, "Button on page 2"),
        button_on_page(ON_PAGE_3, 4, "Button on page 3"),
    ];
    fixture_with(json!({}), text_and_stroke_overrides(), nodes, Vec::new())
}

fn instance_named(doc: &Document, name: &str) -> NodeId {
    doc.nodes.values().find(|n| n.name == name && matches!(n.kind, NodeKind::Instance { .. })).map(|n| n.id).unwrap_or_else(|| panic!("no instance {name}"))
}

fn open(doc: &mut Document, page: &str) {
    let id = page_named(doc, page);
    assert!(doc.ensure_page_hydrated(id), "{page} opens");
}

fn icon_d_is_inside_icon_b(doc: &Document) -> bool {
    let b = doc.nodes.values().find(|n| n.name == "Icon/B" && matches!(n.kind, NodeKind::Component(_))).expect("Icon/B").id;
    let d = doc.nodes.values().find(|n| n.name == "Icon/D" && matches!(n.kind, NodeKind::Component(_))).expect("Icon/D").id;
    let mut stack = vec![b];
    while let Some(id) = stack.pop() {
        let n = doc.get(id).expect("node");
        if matches!(n.kind, NodeKind::Instance { component: Some(c), .. } if c == d) {
            return true;
        }
        stack.extend(n.children.iter().copied());
    }
    false
}

#[test]
fn an_unopened_instance_still_holds_its_record_after_the_loader_widened_what_it_depends_on() {
    let mut doc = import_fig(&fixture()).expect("import");
    let on_3 = instance_named(&doc, "Button on page 3");
    assert!(doc.pending_instance_derived(on_3).is_some(), "the instance holds derived data");
    open(&mut doc, "Page 2");
    assert!(icon_d_is_inside_icon_b(&doc), "opening page 2 filled Icon/B with an instance of a main nobody reached at open");
    assert!(doc.pending_instance_derived(on_3).is_some(), "the loader still holds the record of the unopened instance");
}

#[test]
fn a_main_sync_leaves_an_unopened_instance_that_holds_a_record_empty() {
    let mut doc = import_fig(&fixture()).expect("import");
    open(&mut doc, "Page 2");
    let on_3 = instance_named(&doc, "Button on page 3");
    let button = doc.nodes.values().find(|n| n.name == "Buttons/Button" && matches!(n.kind, NodeKind::Component(_))).expect("button").id;
    let _ = sync_instances(&mut doc, button).apply(&mut doc);
    assert!(doc.get(on_3).expect("instance").children.is_empty(), "the unopened instance waits for its page");
}

#[test]
fn a_save_omits_the_stored_layout_the_loader_can_no_longer_vouch_for() {
    let mut doc = import_fig(&fixture()).expect("import");
    open(&mut doc, "Page 2");
    let saved = export_fig(&doc).expect("export");
    assert!(saved_derived(&saved, ON_PAGE_3).is_empty(), "the layout was computed for an Icon/B without an instance of Icon/D");
}
