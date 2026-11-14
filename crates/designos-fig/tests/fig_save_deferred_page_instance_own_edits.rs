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
fn resetting_a_size_override_on_the_unopened_instance_drops_the_recorded_size() {
    let fig = fixture_with_own_entry();
    let mut live = import_fig(&fig).expect("import");
    let button = variant_button(&live);
    let node = live.get_mut(button).expect("instance");
    assert!(node.component.has_override(&OverrideField::Size), "the fixture records a size override");
    node.size = Vec2::new(120.0, 48.0);
    assert!(node.component.remove_override(&OverrideField::Size));

    let saved = export_fig(&live).expect("export");
    assert_eq!(
        own_entry(&saved),
        json!({
            "guidPath": {"guids": [guid(BUTTON.0, BUTTON.1)]},
            "fillPaints": own_entry(&fig)["fillPaints"]
        }),
        "the reset size is removed from the own entry while the untouched fill remains"
    );
    let back = reopened(&saved);
    let reopened_button = variant_button(&back);
    let size = back.get(reopened_button).expect("instance").size;
    assert_eq!((size.x, size.y), (120.0, 48.0), "a reset pending override follows the main size");

    // Resetting the remaining own-entry field removes the own entry completely rather than leaving a bare guidPath.
    let mut live_all_reset = import_fig(&fig).expect("import");
    let button_all = variant_button(&live_all_reset);
    {
        let n = live_all_reset.get_mut(button_all).expect("instance");
        n.size = Vec2::new(120.0, 48.0);
        n.component.remove_override(&OverrideField::Size);
        n.component.remove_override(&OverrideField::Fills);
    }
    let saved_all_reset = export_fig(&live_all_reset).expect("export");
    assert_eq!(
        saved_entries(&saved_all_reset, INSTANCE),
        text_and_stroke_overrides().as_array().expect("entries").as_slice(),
        "resetting all own-entry fields leaves only the child entries without a bare guidPath own entry"
    );
}

#[test]
fn a_resize_of_a_width_bound_unopened_instance_drops_the_stale_width_binding() {
    let mut own = json!({
        "guidPath": {"guids": [guid(BUTTON.0, BUTTON.1)]},
        "size": {"x": 130.0, "y": 48.0},
        "variableConsumptionMap": {"entries": [{
            "variableField": "WIDTH",
            "variableData": {"dataType": "ALIAS", "resolvedDataType": "FLOAT", "value": {"alias": {"guid": guid(2, 2)}}}
        }]},
        "parameterConsumptionMap": {"entries": [{
            "variableField": "WIDTH",
            "variableData": {"dataType": "ALIAS", "resolvedDataType": "FLOAT", "value": {"alias": {"guid": guid(2, 2)}}}
        }]}
    });
    let mut overrides = text_and_stroke_overrides();
    overrides.as_array_mut().expect("entries").push(std::mem::take(&mut own));
    let float_var = fig_node(
        (2, 2),
        (1, 2),
        "VARIABLE",
        "width-token",
        json!({
            "key": "WIDTH_TOKEN",
            "variableSetID": {"guid": guid(1, 2)},
            "variableResolvedType": "FLOAT",
            "variableDataValues": {"entries": [{"modeID": guid(1, 100), "variableData": {"dataType": "FLOAT", "value": {"floatValue": 130.0}}}]}
        }),
    );
    let fig = fixture_with(json!({}), overrides, vec![float_var], Vec::new());
    let mut live = import_fig(&fig).expect("import");
    let button = variant_button(&live);
    live.get_mut(button).expect("instance").size.x = 200.0;

    let saved = export_fig(&live).expect("export");
    assert_eq!(
        own_entry(&saved),
        json!({
            "guidPath": {"guids": [guid(BUTTON.0, BUTTON.1)]},
            "size": {"x": 200.0, "y": 48.0}
        }),
        "the stale width binding is removed from the restated own entry"
    );
    let back = reopened(&saved);
    assert_eq!(back.get(variant_button(&back)).expect("instance").size.x, 200.0);
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
fn deferred_own_entry_flags_are_not_serialized_as_native_component_overrides() {
    let live = import_fig(&fixture_with_own_entry()).expect("import");
    let button = variant_button(&live);
    let inst = live.get(button).expect("instance");
    assert!(inst.fig_metadata.is_empty(), "deferred instance carries no open-time snapshot in fig_metadata");
    let node_json = serde_json::to_value(inst).expect("node JSON");
    assert!(node_json["component"]["overrides"].is_null(), "transient .fig flags must not reach MCP/node JSON: {node_json}");

    let mut clean_node = inst.clone();
    clean_node.component.fig_own_entry_flags.clear();
    assert_eq!(
        serde_json::to_string(inst).expect("node JSON with transient flags"),
        serde_json::to_string(&clean_node).expect("node JSON without transient flags"),
        "node serialization with transient fig_own_entry_flags is byte-identical to one without them"
    );

    let mut loaded = live;
    loaded.hydrate_all_pages();
    let hydrated_button = variant_button(&loaded);
    assert!(loaded.get(hydrated_button).expect("hydrated instance").component.overrides.contains(&OverrideField::Size));
    let native = loaded.to_json().expect("native JSON after hydration");
    assert!(!native.contains("fig_own_entry_flags"), "transient .fig flags must not reach .designos JSON");

    let mut loaded_with_transient = loaded.clone();
    loaded_with_transient.get_mut(hydrated_button).expect("hydrated instance").component.fig_own_entry_flags.insert(OverrideField::Opacity);
    assert_eq!(
        loaded_with_transient.to_json().expect("native JSON with transient flag"),
        native,
        "native save of a document is byte-identical whether fig_own_entry_flags is empty or populated"
    );
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
