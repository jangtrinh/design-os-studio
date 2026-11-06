//! An instance on a page the loader has not opened is written back with the entries the file stated for it. What the
//! user changed on the instance itself since the file was opened (its size, its fills...) is written from the node
//! instead, and the rest of the record is kept as it was.

mod common;

use common::deferred_fig::*;
use common::fig_fixture::*;
use designos_doc::*;
use designos_fig::{export_fig, import_fig};
use serde_json::{json, Value};

const STATED_SIZE: (f32, f32) = (130.0, 48.0);

/// The pending instance states its own size and fill, and a text override for the label.
fn fixture_with_own_entry() -> Vec<u8> {
    let mut overrides = text_and_stroke_overrides();
    overrides.as_array_mut().expect("entries").push(json!({
        "guidPath": {"guids": [guid(BUTTON.0, BUTTON.1)]},
        "size": {"x": STATED_SIZE.0, "y": STATED_SIZE.1},
        "fillPaints": [solid(&rgba(0.1, 0.2, 0.3), None)]
    }));
    // The instance's own node change states the same size and fill, as a file written by the producing app does.
    // It also has derived data: the size it was computed at for the instance itself, and one for the label.
    let derived = json!([
        {"guidPath": {"guids": [guid(BUTTON.0, BUTTON.1)]}, "size": {"x": STATED_SIZE.0, "y": STATED_SIZE.1}},
        {"guidPath": {"guids": [guid(LABEL.0, LABEL.1)]}, "size": {"x": 90.0, "y": 20.0}}
    ]);
    let own = json!({
        "size": {"x": STATED_SIZE.0, "y": STATED_SIZE.1},
        "fillPaints": [solid(&rgba(0.1, 0.2, 0.3), None)],
        "derivedSymbolData": derived
    });
    fixture_with(own, overrides, Vec::new(), Vec::new())
}

fn reopened(saved: &[u8]) -> Document {
    let mut doc = import_fig(saved).expect("reopen");
    doc.hydrate_all_pages();
    doc
}

fn first_fill(doc: &Document, id: NodeId) -> Option<Color> {
    match doc.get(id)?.fills.first()?.kind {
        PaintKind::Solid(c) => Some(c),
        _ => None,
    }
}

fn main_named(doc: &Document, name: &str) -> NodeId {
    doc.nodes.values().find(|n| n.name == name && matches!(n.kind, NodeKind::Component(_))).map(|n| n.id).expect("main")
}

fn own_entry(saved: &[u8]) -> Value {
    entry_at(&saved_entries(saved, INSTANCE), BUTTON).clone()
}

#[test]
fn a_resize_of_the_unopened_instance_survives_a_save() {
    let fig = fixture_with_own_entry();
    let mut live = import_fig(&fig).expect("import");
    let button = variant_button(&live);
    assert!(live.get(button).expect("instance").children.is_empty(), "the page is not opened");
    live.get_mut(button).expect("instance").size = Vec2::new(200.0, 60.0);

    let saved = export_fig(&live).expect("export");
    let back = reopened(&saved);
    let size = back.get(variant_button(&back)).expect("instance").size;
    assert_eq!((size.x, size.y), (200.0, 60.0), "the edit is in the reopened file, not the size the file stated");
    assert_eq!(own_entry(&saved)["fillPaints"], own_entry(&fig)["fillPaints"], "the fill nobody touched is as the file stated it");
    assert_eq!(saved_entries(&saved, INSTANCE).len(), saved_entries(&fig, INSTANCE).len(), "no entry is added or lost");
    assert!(saved_derived(&saved, INSTANCE).is_empty(), "derived data computed for the old size would put it back over the new one");
}

#[test]
fn a_fill_change_of_the_unopened_instance_survives_a_save() {
    let fig = fixture_with_own_entry();
    let mut live = import_fig(&fig).expect("import");
    let button = variant_button(&live);
    live.get_mut(button).expect("instance").fills = vec![Paint::solid(Color::rgba(255, 0, 0, 255))];

    let saved = export_fig(&live).expect("export");
    let back = reopened(&saved);
    let b = variant_button(&back);
    assert_eq!(first_fill(&back, b), Some(Color::rgba(255, 0, 0, 255)));
    let size = back.get(b).expect("instance").size;
    assert_eq!((size.x, size.y), STATED_SIZE, "the size nobody touched is as the file stated it");
    assert_eq!(own_entry(&saved)["size"], own_entry(&fig)["size"]);
    assert_eq!(saved_derived(&saved, INSTANCE), saved_derived(&fig, INSTANCE), "a fill does not change the sizes inside, so the derived data stays");
}

#[test]
fn an_edit_of_a_field_the_file_did_not_state_is_not_invented() {
    // Nothing on the instance says it overrides its opacity, so a node whose opacity differs is following its main.
    let fig = fixture_with_own_entry();
    let mut live = import_fig(&fig).expect("import");
    let button = variant_button(&live);
    live.get_mut(button).expect("instance").opacity = 0.5;
    let saved = export_fig(&live).expect("export");
    assert_eq!(saved_entries(&saved, INSTANCE), saved_entries(&fig, INSTANCE));
}

#[test]
fn a_main_edit_and_sync_with_no_edit_of_the_instance_reopens_as_the_page_load_gives() {
    let fig = fixture_with_own_entry();
    let mut live = import_fig(&fig).expect("import");
    let button = variant_button(&live);
    let at_open = live.get(button).expect("instance").size;
    let main = main_named(&live, "Buttons/Button");
    let label = live.get(main).expect("main").children[0];
    live.get_mut(label).expect("label").size.x += 5.0;
    let _ = sync_instances(&mut live, main).apply(&mut live);
    let after_sync = live.get(button).expect("instance").size;
    assert_eq!(after_sync, at_open, "the size the file states for the instance is an override, which a sync leaves alone");

    // Opening the page applies the instance's own entry (`apply_override` resizes the node to the size it states and
    // paints it with the fill it states), so the original reopens at the stated size and fill.
    let original = reopened(&fig);
    let (original_size, original_fill) = (original.get(variant_button(&original)).expect("instance").size, first_fill(&original, variant_button(&original)));
    assert_eq!((original_size.x, original_size.y), STATED_SIZE);

    let back = reopened(&export_fig(&live).expect("export"));
    let b = variant_button(&back);
    assert_eq!(back.get(b).expect("instance").size, original_size, "the save of the synced document reopens as a page load gives");
    assert_eq!(first_fill(&back, b), original_fill);
}

#[test]
fn a_save_without_edits_writes_the_entries_the_file_stated() {
    let fig = fixture_with_own_entry();
    let live = import_fig(&fig).expect("import");
    let saved = export_fig(&live).expect("export");
    assert_eq!(saved_entries(&saved, INSTANCE), saved_entries(&fig, INSTANCE));
    assert_eq!(saved_derived(&saved, INSTANCE), saved_derived(&fig, INSTANCE), "nothing was edited, so the derived data is written");
}

#[test]
fn an_instance_the_file_states_no_own_entry_for_gets_one_from_the_edit() {
    let fig = deferred_fixture();
    let mut live = import_fig(&fig).expect("import");
    let button = variant_button(&live);
    {
        let n = live.get_mut(button).expect("instance");
        n.size = Vec2::new(200.0, 60.0);
        n.component.overrides.insert(OverrideField::Size);
    }
    let saved = export_fig(&live).expect("export");
    let back = reopened(&saved);
    let size = back.get(variant_button(&back)).expect("instance").size;
    assert_eq!((size.x, size.y), (200.0, 60.0));
    assert_eq!(label_of(&back, variant_button(&back)), "Go back", "the label override of the record is kept");
}
