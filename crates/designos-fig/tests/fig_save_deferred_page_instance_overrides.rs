//! A `.fig` save of a document that still has pages the loader has not opened.
//!
//! A large multi-page file opens with the instances of its inactive pages unexpanded. A save must keep what the file
//! says about them (their `symbolOverrides`) without opening those pages, and it must leave the document it saves
//! exactly as it found it: the user can still open the pages afterwards, and a second save keeps the overrides too.

mod common;

use common::deferred_fig::*;
use common::fig_fixture::*;
use designos_doc::*;
use designos_fig::{export_fig, import_fig};
use serde_json::{json, Value};

#[test]
fn export_leaves_the_deferred_page_of_the_exported_document_loadable() {
    let mut live = import_fig(&deferred_fixture()).expect("import");
    let page = page_named(&live, "404 pages");
    let button = variant_button(&live);
    assert!(!page_hydrated(&live, page), "the importer defers the page");
    assert!(live.get(button).unwrap().children.is_empty(), "the instance is unexpanded");

    export_fig(&live).expect("export");

    assert!(!page_hydrated(&live, page), "an export does not load the page of the document it saves");
    assert!(live.ensure_page_hydrated(page), "the page still loads on demand");
    assert_eq!(live.get(button).unwrap().children.len(), 2, "the instance expands to the label and the icon");
    assert_eq!(label_of(&live, button), "Go back", "and carries its override");
}

#[test]
fn a_second_export_from_the_same_document_keeps_the_deferred_overrides() {
    let live = import_fig(&deferred_fixture()).expect("import");
    let first = export_fig(&live).expect("first export");
    let second = export_fig(&live).expect("second export");
    assert_eq!(reopened_label(&first), "Go back");
    assert_eq!(reopened_label(&second), "Go back");
}

#[test]
fn loading_every_page_after_an_export_gives_the_node_count_of_a_fresh_open() {
    let fig = deferred_fixture();
    let mut fresh = import_fig(&fig).expect("import");
    fresh.hydrate_all_pages();

    let mut live = import_fig(&fig).expect("import");
    export_fig(&live).expect("export");
    live.hydrate_all_pages();

    assert_eq!(live.nodes.len(), fresh.nodes.len());
}

#[test]
fn export_of_a_deferred_page_instance_writes_its_overrides_without_loading_the_page() {
    let fig = deferred_fixture();
    let original = saved_entries(&fig, INSTANCE);
    assert_eq!(original.len(), 2, "the fixture states two entries");

    let live = import_fig(&fig).expect("import");
    let page = page_named(&live, "404 pages");
    let saved = export_fig(&live).expect("export");

    assert!(!page_hydrated(&live, page), "the page stays unloaded");
    assert_eq!(children_written_under(&saved, INSTANCE), 0, "no children are written for the unexpanded instance");
    let entries = saved_entries(&saved, INSTANCE);
    assert_eq!(entries, original, "the instance's entries are the ones the file stated");
    assert_eq!(entry_at(&entries, LABEL)["textData"]["characters"], json!("Go back"));
    let stroke = entry_at(&entries, ICON);
    assert_eq!(stroke["strokePaints"][0]["colorVar"]["value"]["alias"]["guid"], guid(STROKE_VAR.0, STROKE_VAR.1), "the stroke keeps its variable");
    assert_eq!(reopened_label(&saved), "Go back");
}

#[test]
fn export_of_a_deferred_page_instance_keeps_a_component_property_it_sets_on_a_nested_instance() {
    // `Card` (1:40) holds an instance of `Badge` (1:30), which has a text property `Label`; the instance on the
    // deferred page sets that property on the nested badge (1:41).
    let label_def = (4, 31);
    let extra = vec![
        fig_node(
            (1, 30),
            (0, 3),
            "SYMBOL",
            "Badge",
            json!({"componentPropDefs": [{
                "id": guid(label_def.0, label_def.1), "name": "Label", "type": "TEXT",
                "initialValue": {"textValue": {"characters": "Badge"}}
            }]}),
        ),
        fig_node((1, 40), (0, 3), "SYMBOL", "Card", json!({})),
        fig_node((1, 41), (1, 40), "INSTANCE", "Badge", json!({"symbolData": {"symbolID": guid(1, 30)}})),
        fig_node(
            (1, 50),
            (1, 20),
            "INSTANCE",
            "Card",
            json!({"symbolData": {"symbolID": guid(1, 40), "symbolOverrides": [{
                "guidPath": {"guids": [guid(1, 41)]},
                "componentPropAssignments": [{"defID": guid(label_def.0, label_def.1), "value": {"textValue": {"characters": "New"}}}]
            }]}}),
        ),
    ];
    let live = import_fig(&fixture(text_and_stroke_overrides(), extra, Vec::new())).expect("import");
    let saved = export_fig(&live).expect("export");

    let entries = saved_entries(&saved, (1, 50));
    let assigned = entry_at(&entries, (1, 41))["componentPropAssignments"][0].clone();
    let saved_defs: Vec<Value> = node_changes(&saved)
        .into_iter()
        .find(|nc| nc["guid"] == guid(1, 30))
        .map(|nc| nc["componentPropDefs"].as_array().cloned().unwrap_or_default())
        .unwrap_or_default();
    assert_eq!(saved_defs.len(), 1, "the saved file defines the property once");
    assert_eq!(assigned["defID"], saved_defs[0]["id"], "the assignment names the definition the saved file writes");

    let mut reopened = import_fig(&saved).expect("reopen");
    reopened.hydrate_all_pages();
    let card = reopened.nodes.values().find(|n| n.name == "Card" && matches!(n.kind, NodeKind::Instance { .. })).expect("card instance");
    let badge = card.children.iter().filter_map(|&c| reopened.get(c)).find(|n| n.name == "Badge").expect("badge in the card instance");
    assert_eq!(badge.component.values.get("Label"), Some(&PropValue::Text("New".into())), "the property value survives the save");
}

#[test]
fn a_main_edit_and_sync_keeps_the_deferred_instance_record() {
    let mut live = import_fig(&deferred_fixture()).expect("import");
    let main = live.nodes.values().find(|n| n.name == "Buttons/Button" && matches!(n.kind, NodeKind::Component(_))).expect("main").id;
    let label = live.get(main).expect("main").children[0];
    live.get_mut(label).expect("label").size.x += 5.0;
    sync_instances(&mut live, main).apply(&mut live).expect("sync");

    let saved = export_fig(&live).expect("export");
    assert_eq!(reopened_label(&saved), "Go back", "sync must not replace the override record of an unopened instance");
}

fn button_main(doc: &Document) -> NodeId {
    doc.nodes.values().find(|n| n.name == "Buttons/Button" && matches!(n.kind, NodeKind::Component(_))).expect("main").id
}

fn child_names(doc: &Document, parent: NodeId) -> Vec<String> {
    doc.get(parent).expect("parent").children.iter().map(|&c| doc.get(c).expect("child").name.clone()).collect()
}

#[test]
fn an_unopened_instance_gets_the_latest_main_children_and_its_record_when_the_page_opens() {
    let mut live = import_fig(&deferred_fixture()).expect("import");
    let page = page_named(&live, "404 pages");
    let inst = variant_button(&live);
    let main = button_main(&live);
    let label = live.get(main).expect("main").children[0];
    let icon = live.get(main).expect("main").children[1];

    live.get_mut(label).expect("label").size.x += 5.0;
    sync_instances(&mut live, main).apply(&mut live).expect("sync after a resize");
    let nodes = duplicate_subtree(&mut live, icon);
    let extra = nodes[0].id;
    Command::Insert { parent: main, index: 2, nodes }.apply(&mut live).expect("insert");
    live.get_mut(extra).expect("extra").name = "Extra".into();
    sync_instances(&mut live, main).apply(&mut live).expect("sync after an insert");
    if let NodeKind::Text(t) = &mut live.get_mut(label).expect("label").kind {
        t.content = "Main text v2".into();
        t.runs.clear();
    }
    sync_instances(&mut live, main).apply(&mut live).expect("sync after a text edit");
    assert!(live.get(inst).expect("instance").children.is_empty(), "the instance stays unexpanded while its page is unopened");

    assert!(live.ensure_page_hydrated(page));
    assert_eq!(child_names(&live, inst).len(), 3, "the instance has the main's latest children");
    assert!(child_names(&live, inst).iter().any(|n| n == "Extra"));
    assert_eq!(label_of(&live, inst), "Go back", "the record's text override applies on open");
    let opened_icon = live.get(inst).expect("instance").children.iter().copied().find(|&c| live.get(c).is_some_and(|n| n.name == "Icon")).expect("icon");
    assert!((live.get(opened_icon).expect("icon").stroke.weight - 1.6666666).abs() < 1e-4, "the record's stroke override applies on open");
    let opened_label =
        live.get(inst).expect("instance").children.iter().copied().find(|&c| matches!(live.get(c).map(|n| &n.kind), Some(NodeKind::Text(_)))).expect("label");
    assert!((live.get(opened_label).expect("label").size.x - live.get(label).expect("main label").size.x).abs() < 1e-3, "the width of the latest main");
}

#[test]
fn an_unopened_instance_of_a_main_emptied_and_refilled_gets_the_current_children_on_open() {
    let mut live = import_fig(&deferred_fixture()).expect("import");
    let page = page_named(&live, "404 pages");
    let inst = variant_button(&live);
    let main = button_main(&live);
    let kids = live.get(main).expect("main").children.clone();
    let icon_copy = duplicate_subtree(&mut live, kids[1]);
    for k in kids {
        Command::Remove { id: k }.apply(&mut live).expect("remove");
    }
    sync_instances(&mut live, main).apply(&mut live).expect("sync the empty main");
    assert!(live.get(inst).expect("instance").children.is_empty());
    let only = icon_copy[0].id;
    Command::Insert { parent: main, index: 0, nodes: icon_copy }.apply(&mut live).expect("insert");
    live.get_mut(only).expect("copy").name = "Only".into();
    sync_instances(&mut live, main).apply(&mut live).expect("sync the refilled main");
    assert!(live.get(inst).expect("instance").children.is_empty(), "the instance stays unexpanded while its page is unopened");

    assert!(live.ensure_page_hydrated(page));
    assert_eq!(child_names(&live, inst), vec!["Only".to_string()]);
}

#[test]
fn export_of_a_deferred_page_instance_points_its_geometry_at_the_blobs_of_the_saved_file() {
    // The entry names blob 3 of the file it came from; the saved file has a blob table of its own.
    let wanted = line_blob(7.0);
    let blobs = vec![line_blob(1.0), line_blob(2.0), line_blob(3.0), wanted.clone()];
    let overrides = json!([{
        "guidPath": {"guids": [guid(ICON.0, ICON.1)]},
        "strokePaints": [solid(&rgba(0.64, 0.66, 0.68), None)],
        "strokeGeometry": [{"windingRule": "NONZERO", "commandsBlob": 3}]
    }]);
    let live = import_fig(&fixture(overrides, Vec::new(), blobs)).expect("import");
    let saved = export_fig(&live).expect("export");

    let entries = saved_entries(&saved, INSTANCE);
    let geometry = entry_at(&entries, ICON)["strokeGeometry"][0].clone();
    let index = geometry["commandsBlob"].as_u64().expect("the entry names a blob");
    assert_eq!(saved_blob(&saved, index), wanted, "the entry's blob is the one the file stated, wherever the saved table keeps it");
}

#[test]
fn export_writes_a_deferred_instance_from_the_document_when_its_component_changed() {
    let live = import_fig(&deferred_fixture()).expect("import");
    let other = live.nodes.values().find(|n| n.name == "Type=Simple 03, Breakpoint=Desktop").map(|n| n.id).expect("another component");
    let mut live = live;
    let instance = variant_button(&live);
    let NodeKind::Instance { component, .. } = &mut live.get_mut(instance).expect("instance").kind else { panic!("instance") };
    *component = Some(other);

    let saved = export_fig(&live).expect("export");
    assert!(saved_entries(&saved, INSTANCE).is_empty(), "entries written for one component are not written under another");
}
