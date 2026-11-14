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

fn variant_button_on_page(doc: &Document, page: NodeId) -> NodeId {
    let variant = doc
        .get(page)
        .expect("page")
        .children
        .iter()
        .copied()
        .find(|&id| doc.get(id).is_some_and(|n| matches!(n.kind, NodeKind::Component(_)) && n.name == "Type=Simple 03, Breakpoint=Desktop"))
        .expect("variant on page");
    doc.get(variant).expect("variant").children[0]
}

fn guid_pair_of(v: &Value) -> (u64, u64) {
    (v["sessionID"].as_u64().expect("sessionID"), v["localID"].as_u64().expect("localID"))
}

fn remap_guids_in_value(val: &mut Value, map: &[(Value, Value)]) {
    for (from, to) in map {
        if val == from {
            *val = to.clone();
            return;
        }
    }
    match val {
        Value::Array(arr) => {
            for item in arr {
                remap_guids_in_value(item, map);
            }
        }
        Value::Object(obj) => {
            for v in obj.values_mut() {
                remap_guids_in_value(v, map);
            }
        }
        _ => {}
    }
}

fn page_subtree_entries(saved: &[u8], page_name: &str) -> (Value, Vec<Value>) {
    let all = node_changes(saved);
    let page_entry = all.iter().find(|nc| nc["type"] == "CANVAS" && nc["name"] == page_name).expect("page entry").clone();
    let page_guid = page_entry["guid"].clone();
    let mut subtree = Vec::new();
    let mut parents = vec![page_guid];
    while let Some(parent_guid) = parents.pop() {
        let mut kids: Vec<Value> = all.iter().filter(|nc| nc["parentIndex"]["guid"] == parent_guid).cloned().collect();
        for k in &kids {
            parents.push(k["guid"].clone());
        }
        subtree.append(&mut kids);
    }
    (page_entry, subtree)
}

fn assert_reopened_copy_matches_original(saved: &[u8]) {
    let mut reopened = import_fig(saved).expect("reopen");
    let orig_page = page_named(&reopened, "404 pages");
    let copy_page = page_named(&reopened, "404 pages copy");
    assert!(reopened.ensure_page_hydrated(orig_page));
    assert!(reopened.ensure_page_hydrated(copy_page));

    let orig_inst = variant_button_on_page(&reopened, orig_page);
    let copy_inst = variant_button_on_page(&reopened, copy_page);

    let orig_kids = &reopened.get(orig_inst).expect("orig inst").children;
    let copy_kids = &reopened.get(copy_inst).expect("copy inst").children;
    assert_eq!(copy_kids.len(), 2, "copy instance expands to the label and the icon");
    assert_eq!(orig_kids.len(), 2);

    assert_eq!(label_of(&reopened, copy_inst), "Go back");
    assert_eq!(label_of(&reopened, copy_inst), label_of(&reopened, orig_inst));

    for (&ok, &ck) in orig_kids.iter().zip(copy_kids.iter()) {
        let on = reopened.get(ok).expect("orig child");
        let cn = reopened.get(ck).expect("copy child");
        assert_eq!(cn.name, on.name);
        assert_eq!(cn.fills, on.fills);
        assert_eq!(cn.stroke, on.stroke);
        assert_eq!(cn.variables, on.variables);
        assert_eq!(cn.component.overrides, on.component.overrides);
    }
}

#[test]
fn duplicating_an_unopened_page_keeps_instance_child_overrides_on_save_and_reopen() {
    let fig = deferred_fixture();
    let mut live = import_fig(&fig).expect("import");
    let page = page_named(&live, "404 pages");
    assert!(!page_hydrated(&live, page));

    let mut copy_nodes = duplicate_subtree(&mut live, page);
    copy_nodes[0].name = "404 pages copy".into();
    let copy_page = copy_nodes[0].id;
    Command::InsertPage { index: live.pages.len(), nodes: copy_nodes }.apply(&mut live).expect("duplicate page");

    let saved = export_fig(&live).expect("save");
    assert!(!page_hydrated(&live, page), "original page stays unloaded on save");
    assert!(!page_hydrated(&live, copy_page), "duplicated page stays unloaded on save");

    let (orig_canvas, orig_layer_entries) = page_subtree_entries(&saved, "404 pages");
    let (copy_canvas, mut copy_layer_entries) = page_subtree_entries(&saved, "404 pages copy");
    assert_eq!(copy_layer_entries.len(), orig_layer_entries.len(), "copy page has the same saved entries as the original");

    let mut guid_map: Vec<(Value, Value)> = vec![(copy_canvas["guid"].clone(), orig_canvas["guid"].clone())];
    for (copy_e, orig_e) in copy_layer_entries.iter().zip(orig_layer_entries.iter()) {
        guid_map.push((copy_e["guid"].clone(), orig_e["guid"].clone()));
        if let (Some(ck), Some(ok)) = (copy_e.get("componentKey"), orig_e.get("componentKey")) {
            guid_map.push((ck.clone(), ok.clone()));
        }
    }
    for copy_e in &mut copy_layer_entries {
        remap_guids_in_value(copy_e, &guid_map);
    }
    assert_eq!(copy_layer_entries, orig_layer_entries, "every saved entry on the duplicated page matches the original whole with ids mapped");

    let copy_inst_guid =
        guid_pair_of(&page_subtree_entries(&saved, "404 pages copy").1.into_iter().find(|e| e["type"] == "INSTANCE").expect("copy instance")["guid"]);
    assert_eq!(saved_entries(&saved, copy_inst_guid), saved_entries(&saved, INSTANCE), "copy instance symbolOverrides match original whole");

    assert_reopened_copy_matches_original(&saved);
}

#[test]
fn duplicating_an_unopened_page_opening_the_copy_and_saving_keeps_child_overrides() {
    let fig = deferred_fixture();
    let mut live = import_fig(&fig).expect("import");
    let page = page_named(&live, "404 pages");
    assert!(!page_hydrated(&live, page));

    let mut copy_nodes = duplicate_subtree(&mut live, page);
    copy_nodes[0].name = "404 pages copy".into();
    let copy_page = copy_nodes[0].id;
    Command::InsertPage { index: live.pages.len(), nodes: copy_nodes }.apply(&mut live).expect("duplicate page");

    assert!(live.ensure_page_hydrated(copy_page), "opening the duplicated page hydrates its instances");
    assert!(!page_hydrated(&live, page), "original page stays unloaded when only the copy is opened");
    let copy_inst = variant_button_on_page(&live, copy_page);
    assert_eq!(live.get(copy_inst).expect("copy inst").children.len(), 2);
    assert_eq!(label_of(&live, copy_inst), "Go back");

    let saved = export_fig(&live).expect("save");
    let copy_inst_entry = page_subtree_entries(&saved, "404 pages copy").1.into_iter().find(|e| e["type"] == "INSTANCE").expect("copy instance");
    let copy_entries = saved_entries(&saved, guid_pair_of(&copy_inst_entry["guid"]));
    assert_eq!(entry_at(&copy_entries, LABEL)["textData"]["characters"], json!("Go back"));
    assert_eq!(entry_at(&copy_entries, ICON)["strokePaints"][0]["colorVar"]["value"]["alias"]["guid"], guid(STROKE_VAR.0, STROKE_VAR.1));

    assert_reopened_copy_matches_original(&saved);
}

#[test]
fn undo_and_redo_of_duplicated_unopened_page_removes_and_restores_child_overrides_on_save() {
    let fig = deferred_fixture();
    let baseline = export_fig(&import_fig(&fig).expect("import baseline")).expect("baseline save");

    let mut live = import_fig(&fig).expect("import");
    let page = page_named(&live, "404 pages");
    let mut copy_nodes = duplicate_subtree(&mut live, page);
    copy_nodes[0].name = "404 pages copy".into();
    let undo_cmd = Command::InsertPage { index: live.pages.len(), nodes: copy_nodes }.apply(&mut live).expect("duplicate page");
    let saved_with_dup = export_fig(&live).expect("save with duplicate");

    let redo_cmd = undo_cmd.apply(&mut live).expect("undo duplicate page");
    let saved_after_undo = export_fig(&live).expect("save after undo");
    assert_eq!(node_changes(&saved_after_undo), node_changes(&baseline), "save after undo equals a save without the duplicate");
    let mut reopened_undo = import_fig(&saved_after_undo).expect("reopen after undo");
    assert!(
        !reopened_undo.pages.iter().any(|&p| reopened_undo.get(p).is_some_and(|n| n.name == "404 pages copy")),
        "reopened save after undo has no duplicated page"
    );
    let reopened_orig_page = page_named(&reopened_undo, "404 pages");
    assert!(reopened_undo.ensure_page_hydrated(reopened_orig_page));
    let reopened_orig_inst = variant_button_on_page(&reopened_undo, reopened_orig_page);
    assert_eq!(label_of(&reopened_undo, reopened_orig_inst), "Go back");

    let _ = redo_cmd.apply(&mut live).expect("redo duplicate page");
    let saved_after_redo = export_fig(&live).expect("save after redo");
    assert_eq!(node_changes(&saved_after_redo), node_changes(&saved_with_dup), "save after redo equals the initial duplicate save");
    assert_reopened_copy_matches_original(&saved_after_redo);
}

#[test]
fn original_page_saved_entries_are_unchanged_by_duplicating_the_unopened_page() {
    let fig = deferred_fixture();
    let baseline = export_fig(&import_fig(&fig).expect("import baseline")).expect("baseline save");

    let mut live = import_fig(&fig).expect("import");
    let page = page_named(&live, "404 pages");
    let mut copy_nodes = duplicate_subtree(&mut live, page);
    copy_nodes[0].name = "404 pages copy".into();
    let copy_page = copy_nodes[0].id;
    Command::InsertPage { index: live.pages.len(), nodes: copy_nodes }.apply(&mut live).expect("duplicate page");
    let copy_inst = variant_button_on_page(&live, copy_page);
    assert!(live.pending_instance_overrides(copy_inst).is_some(), "duplicated instance has its own pending override record");

    let saved_with_dup = export_fig(&live).expect("save with duplicate");
    assert_eq!(
        page_subtree_entries(&saved_with_dup, "404 pages"),
        page_subtree_entries(&baseline, "404 pages"),
        "original page saved entries equal a save without the duplicate"
    );
    assert_eq!(saved_entries(&saved_with_dup, INSTANCE), saved_entries(&baseline, INSTANCE));
}

#[test]
fn duplicating_an_unopened_page_counts_a_record_whose_layer_is_missing_in_the_copy() {
    let fig = deferred_fixture();
    let mut live = import_fig(&fig).expect("import");
    let page = page_named(&live, "404 pages");
    assert!(!page_hydrated(&live, page));

    let mut copy_nodes = duplicate_subtree(&mut live, page);
    copy_nodes[0].name = "404 pages copy".into();
    let copy_inst_id = copy_nodes.iter().find(|n| matches!(n.kind, NodeKind::Instance { .. })).expect("copy instance").id;
    for node in &mut copy_nodes {
        node.children.retain(|&id| id != copy_inst_id);
    }
    copy_nodes.retain(|n| n.id != copy_inst_id);

    assert_eq!(live.dropped_duplicate_page_records(), 0);
    Command::InsertPage { index: live.pages.len(), nodes: copy_nodes }.apply(&mut live).expect("insert partial page copy");
    assert_eq!(live.dropped_duplicate_page_records(), 1, "missing instance layer in copy is counted as a dropped duplicate record");
    assert_eq!(live.take_dropped_duplicate_page_records(), 1);
    assert_eq!(live.dropped_duplicate_page_records(), 0);
}

#[test]
fn unstaged_page_insert_gets_no_records_and_counts_dropped_override_records() {
    let fig = deferred_fixture();
    let mut live = import_fig(&fig).expect("import");
    let page = page_named(&live, "404 pages");
    assert!(!page_hydrated(&live, page));

    let mut raw_ids = Vec::new();
    live.walk(page, |n, _| raw_ids.push(n.id));
    let mut id_map = std::collections::HashMap::new();
    for &id in &raw_ids {
        id_map.insert(id, live.alloc_id());
    }
    let mut unstaged_nodes: Vec<designos_doc::Node> = raw_ids
        .iter()
        .map(|&id| {
            let mut n = live.get(id).expect("node").clone();
            n.id = id_map[&id];
            n.parent = n.parent.and_then(|p| id_map.get(&p).copied());
            n.children = n.children.iter().filter_map(|c| id_map.get(c).copied()).collect();
            n.source_id = None;
            n
        })
        .collect();
    unstaged_nodes[0].name = "404 pages unstaged".into();
    let unstaged_page = unstaged_nodes[0].id;

    assert_eq!(live.dropped_duplicate_page_records(), 0);
    Command::InsertPage { index: live.pages.len(), nodes: unstaged_nodes }.apply(&mut live).expect("insert unstaged page");
    let unstaged_inst = variant_button_on_page(&live, unstaged_page);
    assert!(live.pending_instance_overrides(unstaged_inst).is_none(), "unstaged page must not guess or receive override records");
    assert_eq!(live.dropped_duplicate_page_records(), 1, "unstaged unhydrated instance on inserted page is counted as dropped");
}

#[test]
fn undone_duplicate_followed_by_another_edit_drops_the_copied_override_records() {
    let fig = deferred_fixture();
    let mut live = import_fig(&fig).expect("import");
    let page = page_named(&live, "404 pages");
    let baseline_records = live.deferred_page_record_count();
    assert_eq!(baseline_records, 1);

    let mut history = designos_doc::History::default();
    let mut copy_nodes = duplicate_subtree(&mut live, page);
    copy_nodes[0].name = "404 pages copy".into();
    let idx = live.pages.len();
    history.exec(&mut live, Command::InsertPage { index: idx, nodes: copy_nodes }).expect("duplicate page");
    assert_eq!(live.deferred_page_record_count(), 2);

    assert!(history.undo(&mut live).expect("undo duplicate"));
    assert_eq!(live.deferred_page_record_count(), 2, "stashed record lives while on redo stack");

    let new_page_id = live.alloc_id();
    let new_page = designos_doc::Node::new(new_page_id, NodeKind::Page { background: designos_doc::Color::WHITE });
    let idx = live.pages.len();
    history.exec(&mut live, Command::InsertPage { index: idx, nodes: vec![new_page] }).expect("other edit clears redo");
    assert_eq!(live.deferred_page_record_count(), baseline_records, "clearing redo drops the copied override records");
}

#[test]
fn ten_duplicate_and_undo_cycles_do_not_grow_stored_override_record_count() {
    let fig = deferred_fixture();
    let mut live = import_fig(&fig).expect("import");
    let page = page_named(&live, "404 pages");
    let mut history = designos_doc::History::default();

    let mut after_first_cycle = 0;
    for i in 0..10 {
        let mut copy_nodes = duplicate_subtree(&mut live, page);
        copy_nodes[0].name = format!("404 pages copy {i}");
        let idx = live.pages.len();
        history.exec(&mut live, Command::InsertPage { index: idx, nodes: copy_nodes }).expect("duplicate page");
        assert!(history.undo(&mut live).expect("undo duplicate"));
        if i == 0 {
            after_first_cycle = live.deferred_page_record_count();
            assert_eq!(after_first_cycle, 2);
        } else {
            assert_eq!(live.deferred_page_record_count(), after_first_cycle, "stored record count must not grow on cycle {i}");
        }
    }
}

#[test]
fn hydrating_the_last_real_page_after_an_undone_duplicate_releases_shared_hydrator_maps() {
    let fig = deferred_fixture();
    let mut live = import_fig(&fig).expect("import");
    let page = page_named(&live, "404 pages");
    assert!(live.deferred_shared_map_entries() > 0);

    let mut history = designos_doc::History::default();
    let mut copy_nodes = duplicate_subtree(&mut live, page);
    copy_nodes[0].name = "404 pages copy".into();
    let idx = live.pages.len();
    history.exec(&mut live, Command::InsertPage { index: idx, nodes: copy_nodes }).expect("duplicate page");
    assert!(history.undo(&mut live).expect("undo duplicate"));

    // Hydrate the last real pending page while the undone duplicate still sits on the redo stack:
    // what stays is reported as 1 stashed record.
    assert!(live.ensure_page_hydrated(page));
    assert_eq!(live.deferred_page_record_count(), 1, "one stashed record remains while the undone duplicate sits on the redo stack");

    // Once any edit drops the redo stack, no pending page and no live history entry remain,
    // so all shared hydrator maps are released.
    let new_page_id = live.alloc_id();
    let new_page = designos_doc::Node::new(new_page_id, NodeKind::Page { background: designos_doc::Color::WHITE });
    let idx = live.pages.len();
    history.exec(&mut live, Command::InsertPage { index: idx, nodes: vec![new_page] }).expect("edit drops redo stack");
    assert_eq!(live.deferred_page_record_count(), 0);
    assert_eq!(live.deferred_shared_map_entries(), 0, "shared hydrator maps must be released once no pending page or live history entry remains");
}
