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
    let mut baseline_opened_doc = import_fig(&fig).expect("import baseline");
    let baseline_page = page_named(&baseline_opened_doc, "404 pages");
    assert!(baseline_opened_doc.ensure_page_hydrated(baseline_page));
    let baseline_opened = export_fig(&baseline_opened_doc).expect("save baseline with opened page");

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
    let (orig_canvas, orig_layer_entries) = page_subtree_entries(&baseline_opened, "404 pages");
    let (copy_canvas, mut copy_layer_entries) = page_subtree_entries(&saved, "404 pages copy");
    assert_eq!(copy_layer_entries.len(), orig_layer_entries.len(), "opened copy page has the same saved entries as the opened original");

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
    assert_eq!(copy_layer_entries, orig_layer_entries, "every saved entry on the opened duplicated page matches the opened original whole with ids mapped");

    let copy_inst_entry = page_subtree_entries(&saved, "404 pages copy").1.into_iter().find(|e| e["type"] == "INSTANCE").expect("copy instance");
    let copy_entries = saved_entries(&saved, guid_pair_of(&copy_inst_entry["guid"]));
    assert_eq!(copy_entries, saved_entries(&baseline_opened, INSTANCE), "opened copy instance symbolOverrides match opened original whole");
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

#[test]
fn reordering_an_unopened_page_preserves_instance_child_overrides_on_save() {
    let fig = deferred_fixture();
    let baseline = export_fig(&import_fig(&fig).expect("import baseline")).expect("baseline save");
    let (_, baseline_entries) = page_subtree_entries(&baseline, "404 pages");

    let mut live = import_fig(&fig).expect("import");
    let page = page_named(&live, "404 pages");
    assert!(!page_hydrated(&live, page));

    let mut nodes = Vec::new();
    live.walk(page, |n, _| nodes.push(n.clone()));
    let mut history = designos_doc::History::default();
    history.exec(&mut live, Command::Batch(vec![Command::RemovePage { id: page }, Command::InsertPage { index: 0, nodes }])).expect("reorder page");
    assert_eq!(live.pages[0], page);
    assert!(!page_hydrated(&live, page));

    let saved = export_fig(&live).expect("save after reorder");
    let (_, moved_entries) = page_subtree_entries(&saved, "404 pages");
    assert_eq!(moved_entries, baseline_entries, "reordered unopened page saved entries must equal a save without the move");
}

#[test]
fn reordering_an_unopened_page_then_undo_and_redo_preserves_instance_child_overrides_on_save() {
    let fig = deferred_fixture();
    let baseline = export_fig(&import_fig(&fig).expect("import baseline")).expect("baseline save");

    let mut live = import_fig(&fig).expect("import");
    let page = page_named(&live, "404 pages");
    let mut nodes = Vec::new();
    live.walk(page, |n, _| nodes.push(n.clone()));
    let mut history = designos_doc::History::default();
    history.exec(&mut live, Command::Batch(vec![Command::RemovePage { id: page }, Command::InsertPage { index: 0, nodes }])).expect("reorder page");
    let saved_moved = export_fig(&live).expect("save after move");

    assert!(history.undo(&mut live).expect("undo move"));
    let saved_undone = export_fig(&live).expect("save after undo");
    assert_eq!(node_changes(&saved_undone), node_changes(&baseline), "reorder then undo then save must equal a save without the move");

    assert!(history.redo(&mut live).expect("redo move"));
    let saved_redone = export_fig(&live).expect("save after redo");
    assert_eq!(node_changes(&saved_redone), node_changes(&saved_moved), "reorder then undo then redo then save must equal the save after reorder");
}

#[test]
fn deleting_an_unopened_page_then_undoing_preserves_instance_child_overrides_on_save() {
    let fig = deferred_fixture();
    let baseline = export_fig(&import_fig(&fig).expect("import baseline")).expect("baseline save");

    let mut live = import_fig(&fig).expect("import");
    let page = page_named(&live, "404 pages");
    let mut history = designos_doc::History::default();
    history.exec(&mut live, Command::RemovePage { id: page }).expect("delete unopened page");
    let stored_undo_cmd = history.peek_undo().cloned().expect("stored undo command");

    assert!(history.undo(&mut live).expect("undo delete"));
    let saved_after_undo = export_fig(&live).expect("save after undo of delete");
    assert_eq!(node_changes(&saved_after_undo), node_changes(&baseline), "delete then undo then save must equal a save without the delete");

    Command::RemovePage { id: page }.apply(&mut live).expect("remove page again");
    stored_undo_cmd.apply(&mut live).expect("reapply stored InsertPage command");
    let saved_after_reapply = export_fig(&live).expect("save after reapplying stored InsertPage");
    assert_eq!(node_changes(&saved_after_reapply), node_changes(&baseline), "applying an InsertPage must not consume the payload stored on a history command");
}

#[test]
fn duplicating_an_unopened_page_then_reordering_the_copy_preserves_child_overrides_on_save() {
    let fig = deferred_fixture();
    let mut live = import_fig(&fig).expect("import");
    let page = page_named(&live, "404 pages");
    let mut copy_nodes = duplicate_subtree(&mut live, page);
    copy_nodes[0].name = "404 pages copy".into();
    let copy_page = copy_nodes[0].id;

    let mut history = designos_doc::History::default();
    let idx = live.pages.len();
    history.exec(&mut live, Command::InsertPage { index: idx, nodes: copy_nodes }).expect("duplicate page");
    assert!(!page_hydrated(&live, copy_page));

    let mut move_nodes = Vec::new();
    live.walk(copy_page, |n, _| move_nodes.push(n.clone()));
    history
        .exec(&mut live, Command::Batch(vec![Command::RemovePage { id: copy_page }, Command::InsertPage { index: 1, nodes: move_nodes }]))
        .expect("reorder duplicated page");
    assert_eq!(live.pages[1], copy_page);
    assert!(!page_hydrated(&live, copy_page));

    let saved = export_fig(&live).expect("save after duplicate and reorder");
    let (orig_canvas, orig_entries) = page_subtree_entries(&saved, "404 pages");
    let (copy_canvas, mut copy_entries) = page_subtree_entries(&saved, "404 pages copy");
    assert_eq!(copy_entries.len(), orig_entries.len());

    let mut guid_map: Vec<(serde_json::Value, serde_json::Value)> = vec![(copy_canvas["guid"].clone(), orig_canvas["guid"].clone())];
    for (ce, oe) in copy_entries.iter().zip(orig_entries.iter()) {
        guid_map.push((ce["guid"].clone(), oe["guid"].clone()));
        if let (Some(ck), Some(ok)) = (ce.get("componentKey"), oe.get("componentKey")) {
            guid_map.push((ck.clone(), ok.clone()));
        }
    }
    for ce in &mut copy_entries {
        remap_guids_in_value(ce, &guid_map);
    }
    assert_eq!(copy_entries, orig_entries, "duplicated then reordered unopened page must keep its child override records whole");
    assert_reopened_copy_matches_original(&saved);
}

fn variant_main(doc: &Document) -> NodeId {
    doc.nodes.values().find(|n| matches!(n.kind, NodeKind::Component(_)) && n.name == "Type=Simple 03, Breakpoint=Desktop").expect("variant").id
}

#[derive(Debug, PartialEq)]
struct SubtreeEntry {
    depth: usize,
    kind: &'static str,
    name: String,
    text: Option<String>,
    size: (i32, i32),
    stroke_weight_milli: i32,
}

fn node_kind_tag(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Page { .. } => "Page",
        NodeKind::Frame(_) => "Frame",
        NodeKind::Group => "Group",
        NodeKind::Section => "Section",
        NodeKind::Rectangle => "Rectangle",
        NodeKind::Ellipse(_) => "Ellipse",
        NodeKind::Line => "Line",
        NodeKind::Polygon { .. } => "Polygon",
        NodeKind::Star { .. } => "Star",
        NodeKind::Vector { .. } => "Vector",
        NodeKind::Text(_) => "Text",
        NodeKind::BooleanOp(_) => "BooleanOp",
        NodeKind::Component(_) => "Component",
        NodeKind::ComponentSet(_) => "ComponentSet",
        NodeKind::Instance { .. } => "Instance",
        NodeKind::Sticky(_) => "Sticky",
        NodeKind::ShapeWithText(_) => "ShapeWithText",
        NodeKind::Connector(_) => "Connector",
        NodeKind::Stamp { .. } => "Stamp",
        NodeKind::Table(_) => "Table",
        NodeKind::TableCell(_) => "TableCell",
        NodeKind::Unknown { .. } => "Unknown",
    }
}

fn subtree_snapshot(doc: &Document, root: NodeId) -> Vec<SubtreeEntry> {
    let mut out = Vec::new();
    doc.walk(root, |n, depth| {
        let text = match &n.kind {
            NodeKind::Text(tp) => Some(tp.content.clone()),
            _ => None,
        };
        out.push(SubtreeEntry {
            depth,
            kind: node_kind_tag(&n.kind),
            name: n.name.clone(),
            text,
            size: ((n.size.x * 1000.0).round() as i32, (n.size.y * 1000.0).round() as i32),
            stroke_weight_milli: (n.stroke.weight * 1000.0).round() as i32,
        });
    });
    out
}

const STATED_SIZE: (f32, f32) = (130.0, 48.0);

fn fixture_with_own_entry() -> Vec<u8> {
    let mut overrides = text_and_stroke_overrides();
    overrides.as_array_mut().expect("entries").push(json!({
        "guidPath": {"guids": [guid(BUTTON.0, BUTTON.1)]},
        "size": {"x": STATED_SIZE.0, "y": STATED_SIZE.1},
        "fillPaints": [solid(&rgba(0.1, 0.2, 0.3), None)]
    }));
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

fn instance_node_change(saved: &[u8], id: (u64, u64)) -> Value {
    let want = guid(id.0, id.1);
    node_changes(saved).into_iter().find(|nc| nc["guid"] == want).unwrap_or_else(|| panic!("no node {id:?} in saved file"))
}

fn reopened_new_instance_label(saved: &[u8]) -> String {
    let doc = import_fig(saved).expect("reopen");
    let cover = page_named(&doc, "Cover");
    let new_inst = doc
        .get(cover)
        .expect("cover")
        .children
        .iter()
        .copied()
        .find(|&id| doc.get(id).is_some_and(|n| matches!(n.kind, NodeKind::Instance { .. })))
        .expect("new instance on Cover");
    let nested_copy = doc.get(new_inst).expect("new instance").children[0];
    label_of(&doc, nested_copy)
}

#[test]
fn new_instance_of_main_holding_deferred_page_instance_copies_nested_children_at_once() {
    let fig = fixture_with_own_entry();
    let mut opened = import_fig(&fig).expect("import opened reference");
    let opened_page_404 = page_named(&opened, "404 pages");
    assert!(opened.ensure_page_hydrated(opened_page_404));
    let opened_main_nested = variant_button(&opened);

    let mut live = import_fig(&fig).expect("import");
    let cover = page_named(&live, "Cover");
    let page_404 = page_named(&live, "404 pages");
    let variant = variant_main(&live);
    assert!(!page_hydrated(&live, page_404));

    let cmd = create_instance(&mut live, variant, cover, Vec2::ZERO);
    let new_inst = match &cmd {
        Command::Insert { nodes, .. } => nodes[0].id,
        _ => panic!("expected Insert"),
    };
    cmd.apply(&mut live).expect("create instance");

    assert!(!page_hydrated(&live, page_404), "creating an instance must not mark the deferred page open");
    let main_nested = variant_button(&live);
    assert!(live.get(main_nested).expect("main nested").children.is_empty(), "deferred instance inside the main stays empty");
    let nested_copy = live.get(new_inst).expect("new instance").children[0];
    assert_eq!(child_names(&live, nested_copy), vec!["Text", "Icon"], "nested instance copy has its children at once");
    assert_eq!(child_names(&live, nested_copy), child_names(&opened, opened_main_nested), "same child count and names as main's nested instance");
    assert_eq!(label_of(&live, nested_copy), "Go back", "nested instance copy carries the overridden text");
    assert_eq!(label_of(&live, nested_copy), label_of(&opened, opened_main_nested), "same text as main's nested instance");
    assert_eq!(subtree_snapshot(&live, nested_copy), subtree_snapshot(&opened, opened_main_nested), "same subtree sizes and strokes as main's nested instance");
}

#[test]
fn opening_deferred_page_and_syncing_leaves_new_instance_equal_to_post_open_instance() {
    let mut live = import_fig(&fixture_with_own_entry()).expect("import");
    let cover = page_named(&live, "Cover");
    let page_404 = page_named(&live, "404 pages");
    let variant = variant_main(&live);

    let cmd_before = create_instance(&mut live, variant, cover, Vec2::ZERO);
    let inst_before = match &cmd_before {
        Command::Insert { nodes, .. } => nodes[0].id,
        _ => panic!("expected Insert"),
    };
    cmd_before.apply(&mut live).expect("create instance before open");
    let snap_before_open = subtree_snapshot(&live, inst_before);
    assert_eq!(snap_before_open.len(), 4, "outer instance, nested instance, text, and icon are present at once");

    assert!(live.ensure_page_hydrated(page_404));
    assert_eq!(subtree_snapshot(&live, inst_before), snap_before_open, "opening the deferred page alone leaves the new instance unchanged");
    sync_instances(&mut live, variant).apply(&mut live).expect("sync variant");
    let snap_after_open = subtree_snapshot(&live, inst_before);

    let cmd_after = create_instance(&mut live, variant, cover, Vec2::ZERO);
    let inst_after = match &cmd_after {
        Command::Insert { nodes, .. } => nodes[0].id,
        _ => panic!("expected Insert"),
    };
    cmd_after.apply(&mut live).expect("create instance after open");
    let snap_post_open = subtree_snapshot(&live, inst_after);

    assert_eq!(snap_before_open, snap_after_open, "opening the deferred page and syncing does not duplicate or alter the subtree");
    assert_eq!(snap_after_open, snap_post_open, "new instance matches an instance created after opening the deferred page");
}

#[test]
fn undo_and_redo_of_new_instance_removes_and_restores_nested_subtree() {
    let mut live = import_fig(&fixture_with_own_entry()).expect("import");
    let cover = page_named(&live, "Cover");
    let variant = variant_main(&live);
    let main_nested = variant_button(&live);
    let cover_children_before = live.get(cover).expect("cover").children.len();
    let main_nested_before = live.get(main_nested).expect("main nested").clone();
    let variant_before = live.get(variant).expect("variant").clone();
    let records_before = live.deferred_page_record_count();
    let shared_before = live.deferred_shared_map_entries();
    let overrides_before = live.pending_instance_overrides(main_nested);
    let derived_before = live.pending_instance_derived(main_nested).expect("derived before");

    let mut history = History::default();
    let cmd = create_instance(&mut live, variant, cover, Vec2::ZERO);
    let (new_inst, created_ids): (NodeId, Vec<NodeId>) = match &cmd {
        Command::Insert { nodes, .. } => (nodes[0].id, nodes.iter().map(|n| n.id).collect()),
        _ => panic!("expected Insert"),
    };
    history.exec(&mut live, cmd).expect("exec create instance");

    let snap_created = subtree_snapshot(&live, new_inst);
    assert_eq!(snap_created.len(), 4);

    assert!(history.undo(&mut live).expect("undo"), "undo succeeds");
    assert_eq!(live.get(cover).expect("cover").children.len(), cover_children_before);
    for &id in &created_ids {
        assert!(live.get(id).is_none(), "undo removes created node {id:?}");
    }
    assert!(live.get(main_nested).expect("main nested").children.is_empty(), "main's nested instance is empty after undo");
    assert_eq!(live.get(main_nested).expect("main nested"), &main_nested_before, "main's nested instance node is byte-for-byte unchanged");
    assert_eq!(live.get(variant).expect("variant"), &variant_before, "main component node is byte-for-byte unchanged");
    assert_eq!(live.deferred_page_record_count(), records_before, "importer record count is unchanged");
    assert_eq!(live.deferred_shared_map_entries(), shared_before, "importer shared map entries are unchanged");
    assert_eq!(live.pending_instance_overrides(main_nested), overrides_before, "pending overrides are unchanged");
    let derived_after = live.pending_instance_derived(main_nested).expect("derived after undo");
    assert_eq!(derived_after.fingerprint, derived_before.fingerprint, "pending derived fingerprint is unchanged");
    assert_eq!(derived_after.entries.len(), derived_before.entries.len(), "pending derived entries are unchanged");

    assert!(history.redo(&mut live).expect("redo"), "redo succeeds");
    assert_eq!(subtree_snapshot(&live, new_inst), snap_created, "redo restores the full nested subtree");
}

#[test]
fn saving_after_creating_instance_of_main_with_deferred_nested_instance_keeps_saved_entries() {
    let fig = fixture_with_own_entry();
    let baseline = export_fig(&import_fig(&fig).expect("import baseline")).expect("baseline save");
    let baseline_entries = saved_entries(&baseline, INSTANCE);
    let baseline_derived = saved_derived(&baseline, INSTANCE);
    let baseline_inst_nc = instance_node_change(&baseline, INSTANCE);
    assert!(!baseline_derived.is_empty(), "fixture has derivedSymbolData");

    let mut live = import_fig(&fig).expect("import");
    let cover = page_named(&live, "Cover");
    let page_404 = page_named(&live, "404 pages");
    let variant = variant_main(&live);

    let cmd = create_instance(&mut live, variant, cover, Vec2::ZERO);
    let new_inst = match &cmd {
        Command::Insert { nodes, .. } => nodes[0].id,
        _ => panic!("expected Insert"),
    };
    cmd.apply(&mut live).expect("create instance");

    let nested_copy = live.get(new_inst).expect("new instance").children[0];
    assert_eq!(label_of(&live, nested_copy), "Go back");

    let saved = export_fig(&live).expect("export");
    assert!(!page_hydrated(&live, page_404), "deferred page remains unopened after save");
    assert_eq!(children_written_under(&saved, INSTANCE), 0, "unopened page instance writes no child node changes");
    assert_eq!(saved_entries(&saved, INSTANCE), baseline_entries, "saved entries of the main's nested instance match baseline");
    assert_eq!(saved_derived(&saved, INSTANCE), baseline_derived, "derivedSymbolData of the main's nested instance matches baseline");
    assert_eq!(instance_node_change(&saved, INSTANCE), baseline_inst_nc, "whole INSTANCE NodeChange matches baseline");
    assert_eq!(reopened_new_instance_label(&saved), "Go back", "reopened file has the overridden text on the new instance");
    assert_eq!(reopened_label(&saved), "Go back", "reopened file keeps the overridden text on the deferred instance");
}

#[test]
fn editing_main_creating_instance_and_undoing_main_edit_keeps_deferred_instance_saved_entries() {
    let fig = fixture_with_own_entry();
    let baseline = export_fig(&import_fig(&fig).expect("import baseline")).expect("baseline save");
    let baseline_inst_nc = instance_node_change(&baseline, INSTANCE);
    let baseline_derived = saved_derived(&baseline, INSTANCE);
    assert!(!baseline_derived.is_empty());

    let mut live = import_fig(&fig).expect("import");
    let cover = page_named(&live, "Cover");
    let variant = variant_main(&live);
    let button = button_main(&live);
    let label = live.get(button).expect("button").children[0];

    let mut edited_label = live.get(label).expect("label").clone();
    edited_label.size.x += 15.0;
    let edit_inv = Command::Update { node: Box::new(edited_label) }.apply(&mut live).expect("edit button label");
    let sync_inv = sync_instances(&mut live, button).apply(&mut live).expect("sync button after edit");

    let create_cmd = create_instance(&mut live, variant, cover, Vec2::ZERO);
    create_cmd.apply(&mut live).expect("create instance while button is edited");

    sync_inv.apply(&mut live).expect("undo sync");
    edit_inv.apply(&mut live).expect("undo button edit");

    let saved = export_fig(&live).expect("save after undoing button edit");
    assert_eq!(saved_entries(&saved, INSTANCE), saved_entries(&baseline, INSTANCE), "saved symbolOverrides equal save without the create");
    assert_eq!(saved_derived(&saved, INSTANCE), baseline_derived, "saved derivedSymbolData equals save without the create");
    assert_eq!(instance_node_change(&saved, INSTANCE), baseline_inst_nc, "whole deferred instance NodeChange equals save without the create");
    assert_eq!(
        page_subtree_entries(&saved, "404 pages"),
        page_subtree_entries(&baseline, "404 pages"),
        "saved 404 pages subtree equals save without the create"
    );
}

#[test]
fn editing_other_main_creating_unrelated_instance_and_undoing_keeps_derived_symbol_data() {
    let mut overrides = text_and_stroke_overrides();
    overrides.as_array_mut().expect("entries").push(json!({
        "guidPath": {"guids": [guid(BUTTON.0, BUTTON.1)]},
        "size": {"x": STATED_SIZE.0, "y": STATED_SIZE.1},
        "fillPaints": [solid(&rgba(0.1, 0.2, 0.3), None)]
    }));
    let derived = json!([
        {"guidPath": {"guids": [guid(BUTTON.0, BUTTON.1)]}, "size": {"x": STATED_SIZE.0, "y": STATED_SIZE.1}},
        {"guidPath": {"guids": [guid(LABEL.0, LABEL.1)]}, "size": {"x": 90.0, "y": 20.0}}
    ]);
    let own = json!({
        "size": {"x": STATED_SIZE.0, "y": STATED_SIZE.1},
        "fillPaints": [solid(&rgba(0.1, 0.2, 0.3), None)],
        "derivedSymbolData": derived
    });
    let extra = vec![
        fig_node((1, 30), (0, 3), "SYMBOL", "OtherLeaf", json!({"size": {"x": 40.0, "y": 20.0}})),
        fig_node((1, 31), (1, 30), "TEXT", "LeafText", json!({"fontSize": 12.0, "textData": {"characters": "Tag"}})),
        fig_node((1, 40), (0, 3), "SYMBOL", "OtherHolder", json!({"size": {"x": 120.0, "y": 40.0}})),
        fig_node((1, 41), (1, 40), "INSTANCE", "OtherLeaf", json!({"symbolData": {"symbolID": guid(1, 30)}})),
    ];
    let fig = fixture_with(own, overrides, extra, Vec::new());
    let baseline = export_fig(&import_fig(&fig).expect("import baseline")).expect("baseline save");
    let baseline_derived = saved_derived(&baseline, INSTANCE);
    assert!(!baseline_derived.is_empty());

    let mut live = import_fig(&fig).expect("import");
    let cover = page_named(&live, "Cover");
    let button = button_main(&live);
    let label = live.get(button).expect("button").children[0];
    let other_holder = live.nodes.values().find(|n| matches!(n.kind, NodeKind::Component(_)) && n.name == "OtherHolder").expect("OtherHolder").id;

    let mut edited_label = live.get(label).expect("label").clone();
    edited_label.size.x += 20.0;
    let edit_inv = Command::Update { node: Box::new(edited_label) }.apply(&mut live).expect("edit button label");

    let create_cmd = create_instance(&mut live, other_holder, cover, Vec2::ZERO);
    create_cmd.apply(&mut live).expect("create unrelated instance");

    edit_inv.apply(&mut live).expect("undo button edit");

    let saved = export_fig(&live).expect("save");
    assert_eq!(saved_derived(&saved, INSTANCE), baseline_derived, "derivedSymbolData on the untouched main's deferred instance is still written");
}

#[test]
fn creating_instance_editing_main_then_opening_deferred_page_matches_fresh_open() {
    let fig = fixture_with_own_entry();

    let mut fresh = import_fig(&fig).expect("import fresh");
    let fresh_page_404 = page_named(&fresh, "404 pages");
    let fresh_button = button_main(&fresh);
    let fresh_label = fresh.get(fresh_button).expect("button").children[0];
    let fresh_icon = fresh.get(fresh_button).expect("button").children[1];
    fresh.get_mut(fresh_label).expect("label").size.x += 25.0;
    for _ in 0..4 {
        fresh.alloc_id();
    }
    let extra_nodes = duplicate_subtree(&mut fresh, fresh_icon);
    let extra_id = extra_nodes[0].id;
    Command::Insert { parent: fresh_button, index: 2, nodes: extra_nodes }.apply(&mut fresh).expect("insert extra child");
    fresh.get_mut(extra_id).expect("extra").name = "Badge".into();
    sync_instances(&mut fresh, fresh_button).apply(&mut fresh).expect("sync fresh button");
    assert!(fresh.ensure_page_hydrated(fresh_page_404));
    let fresh_inst = variant_button(&fresh);
    let fresh_saved = export_fig(&fresh).expect("save fresh");

    let mut live = import_fig(&fig).expect("import live");
    let cover = page_named(&live, "Cover");
    let page_404 = page_named(&live, "404 pages");
    let variant = variant_main(&live);
    let button = button_main(&live);

    let mut history = History::default();
    let create_cmd = create_instance(&mut live, variant, cover, Vec2::ZERO);
    history.exec(&mut live, create_cmd).expect("create instance before editing button");
    assert!(history.undo(&mut live).expect("undo create so page set matches fresh open"));

    let label = live.get(button).expect("button").children[0];
    let icon = live.get(button).expect("button").children[1];
    live.get_mut(label).expect("label").size.x += 25.0;
    let extra_nodes = duplicate_subtree(&mut live, icon);
    let extra_id = extra_nodes[0].id;
    Command::Insert { parent: button, index: 2, nodes: extra_nodes }.apply(&mut live).expect("insert extra child");
    live.get_mut(extra_id).expect("extra").name = "Badge".into();
    sync_instances(&mut live, button).apply(&mut live).expect("sync live button");

    assert!(live.ensure_page_hydrated(page_404));
    let live_inst = variant_button(&live);
    assert_eq!(child_names(&live, live_inst), vec!["Text", "Icon", "Badge"], "deferred instance shows B's current children");
    assert_eq!(label_of(&live, live_inst), "Go back", "kept text override applied once");
    assert_eq!(subtree_snapshot(&live, live_inst), subtree_snapshot(&fresh, fresh_inst), "deferred instance matches fresh open");

    let live_saved = export_fig(&live).expect("save live");
    assert_eq!(node_changes(&live_saved), node_changes(&fresh_saved), "save after opening equals a save from a fresh open");
}
