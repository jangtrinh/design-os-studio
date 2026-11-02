//! Node-level instance aspect locks must survive import and later component sync.
use designos_doc::{sync_instances, Document, Node, NodeKind, OverrideField};
use designos_fig::{export_fig, import_fig, import_rest_json};
use serde_json::json;

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

fn rest_fixture(main_locked: bool, instance_locked: bool) -> String {
    let main_lock = main_locked.then_some(json!(true));
    let instance_lock = json!(instance_locked);
    let mut main = json!({
        "id": "1:1", "type": "COMPONENT", "name": "Main", "size": {"x": 100.0, "y": 50.0},
        "children": [{"id": "1:2", "type": "RECTANGLE", "name": "Body", "size": {"x": 100.0, "y": 50.0}}]
    });
    if let Some(v) = main_lock {
        main["preserveRatio"] = v;
    }
    let mut instance = json!({
        "id": "1:3", "type": "INSTANCE", "name": "Instance", "componentId": "1:1",
        "size": {"x": 100.0, "y": 50.0}
    });
    instance["preserveRatio"] = instance_lock;
    json!({
        "name": "node-lock",
        "document": {"id": "0:0", "type": "DOCUMENT", "children": [{
            "id": "0:1", "type": "CANVAS", "name": "Page 1", "children": [main, instance]
        }]}
    })
    .to_string()
}

fn sync_main_to(doc: &mut Document, name: &str, locked: bool) {
    let main_id = find(doc, name).id;
    let mut main = doc.get(main_id).unwrap().clone();
    main.lock_aspect = locked;
    designos_doc::Command::Update { node: Box::new(main) }.apply(doc).unwrap();
    sync_instances(doc, main_id).apply(doc).unwrap();
}

#[test]
fn rest_node_level_lock_is_recorded_and_survives_sync_in_both_directions() {
    for (main_locked, instance_locked) in [(false, true), (true, false)] {
        let mut doc = import_rest_json(&rest_fixture(main_locked, instance_locked)).expect("REST import");
        let instance = find(&doc, "Instance");
        assert_eq!(instance.lock_aspect, instance_locked);
        assert!(instance.component.overrides.contains(&OverrideField::LockAspect), "node-level difference must become an override");
        sync_main_to(&mut doc, "Main", !main_locked);
        assert_eq!(find(&doc, "Instance").lock_aspect, instance_locked, "own lock must survive main sync");
    }
}

#[test]
fn fig_node_level_false_lock_survives_conversion_and_reopen() {
    let mut doc = Document::new("fig-node-lock");
    let page = doc.add_page("Page 1");
    let main_id = doc.alloc_id();
    let main = doc.insert(page, Node::frame(main_id, designos_doc::Rect::new(0.0, 0.0, 100.0, 50.0))).unwrap();
    doc.get_mut(main).unwrap().name = "Main".into();
    designos_doc::create_component(&mut doc, main).apply(&mut doc).unwrap();
    let instance = designos_doc::create_instance(&mut doc, main, page, designos_doc::Vec2::new(200.0, 0.0));
    instance.apply(&mut doc).unwrap();
    let instance_id = *doc.get(page).unwrap().children.last().unwrap();
    doc.get_mut(instance_id).unwrap().name = "Instance".into();
    let n = doc.get_mut(instance_id).unwrap();
    n.lock_aspect = false;
    n.component.overrides.insert(OverrideField::LockAspect);
    let reopened = import_fig(&export_fig(&doc).expect(".fig export")).expect(".fig import");
    let instance = find(&reopened, "Instance");
    assert!(!instance.lock_aspect, "explicit node-level false must remain false after .fig reopen");
    assert!(instance.component.overrides.contains(&OverrideField::LockAspect));
    assert!(matches!(instance.kind, NodeKind::Instance { .. }));
}
