//! An aspect lock carried on the instance node itself is the instance's own lock.
//!
//! A source file can state an instance's lock on the node, with no lock entry in its `symbolOverrides`. When that
//! lock differs from the main's, import has to record it as the instance's own (the `LockAspect` override), or the
//! next sync of the main hands the instance the main's lock again. `false` counts as much as `true`.
use designos_doc::{sync_instances, Command, Document, Node, NodeKind, OverrideField};
use designos_fig::container::{write_container, Compression};
use designos_fig::kiwi::KiwiValue as V;
use designos_fig::testkit::{color, figma_like_schema, guid, msg, node, vec2};
use designos_fig::{import_fig, import_rest_json};
use serde_json::json;

/// The two ways an instance's lock can differ from its main's, as `(main locked, instance locked)`.
const OPPOSITE_LOCKS: [(bool, bool); 2] = [(true, false), (false, true)];

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

fn rest_fixture(main_locked: bool, instance_locked: bool) -> String {
    let mut main = json!({
        "id": "1:1", "type": "COMPONENT", "name": "Main", "size": {"x": 100.0, "y": 50.0},
        "children": [{"id": "1:2", "type": "RECTANGLE", "name": "Body", "size": {"x": 100.0, "y": 50.0}}]
    });
    if main_locked {
        main["preserveRatio"] = json!(true);
    }
    let mut instance = json!({
        "id": "1:3", "type": "INSTANCE", "name": "Instance", "componentId": "1:1",
        "size": {"x": 100.0, "y": 50.0}
    });
    instance["preserveRatio"] = json!(instance_locked);
    json!({
        "name": "node-lock",
        "document": {"id": "0:0", "type": "DOCUMENT", "children": [{
            "id": "0:1", "type": "CANVAS", "name": "Page 1", "children": [main, instance]
        }]}
    })
    .to_string()
}

/// A binary `.fig`: a locked or open main, and an instance that states its own lock on the node with no lock entry
/// in its `symbolOverrides`. The main states a lock only when it is on.
fn fig_fixture(main_locked: bool, instance_locked: bool) -> Vec<u8> {
    let lock = |locked: bool| ("proportionsConstrained", V::Bool(locked));
    let mut main_fields = vec![("size", vec2(100.0, 50.0))];
    if main_locked {
        main_fields.push(lock(true));
    }
    let nodes = vec![
        node((0, 0), (0, 0), "!", "DOCUMENT", "Document", vec![]),
        node((0, 1), (0, 0), "a", "CANVAS", "Page 1", vec![("backgroundColor", color(1.0, 1.0, 1.0, 1.0))]),
        node((2, 10), (0, 1), "a", "SYMBOL", "Main", main_fields),
        node((2, 11), (2, 10), "a", "ROUNDED_RECTANGLE", "Body", vec![("size", vec2(100.0, 50.0))]),
        node(
            (2, 20),
            (0, 1),
            "b",
            "INSTANCE",
            "Instance",
            vec![
                ("size", vec2(100.0, 50.0)),
                lock(instance_locked),
                ("symbolData", msg(vec![("symbolID", guid(2, 10)), ("symbolOverrides", V::Array(vec![]))])),
            ],
        ),
    ];
    let schema = figma_like_schema();
    let root = schema.find("Message").expect("Message in the embedded schema");
    let body = schema.encode_message(root, &msg(vec![("nodeChanges", V::Array(nodes))])).expect("encode nodeChanges");
    write_container(&schema.encode(), &body, Compression::Deflate, 106)
}

fn sync_main_to(doc: &mut Document, name: &str, locked: bool) {
    let main_id = find(doc, name).id;
    let mut main = doc.get(main_id).unwrap().clone();
    main.lock_aspect = locked;
    Command::Update { node: Box::new(main) }.apply(doc).unwrap();
    sync_instances(doc, main_id).apply(doc).unwrap();
}

/// The instance holds `instance_locked` as its own lock: it is linked to its main, the lock is recorded as an
/// override, and a sync of the main to the opposite of what the instance holds leaves the instance alone.
fn assert_own_lock_survives_sync(mut doc: Document, instance_locked: bool) {
    let main_id = find(&doc, "Main").id;
    let instance = find(&doc, "Instance");
    assert!(matches!(instance.kind, NodeKind::Instance { .. }));
    assert_eq!(instance.component.source, Some(main_id), "the instance is linked to its main, so a sync reaches it");
    assert_eq!(instance.lock_aspect, instance_locked, "the node-level lock is read as written");
    assert!(instance.component.overrides.contains(&OverrideField::LockAspect), "a lock that differs from the main's must be recorded as the instance's own");
    assert_ne!(find(&doc, "Main").lock_aspect, instance_locked, "fixture: the lock differs from the main's");

    sync_main_to(&mut doc, "Main", !instance_locked);
    assert_eq!(find(&doc, "Main").lock_aspect, !instance_locked, "the main now holds the lock the instance does not");
    assert_eq!(find(&doc, "Instance").lock_aspect, instance_locked, "the instance's own lock must survive a sync of its main");
}

#[test]
fn rest_node_level_lock_that_differs_from_its_main_survives_sync_in_both_directions() {
    for (main_locked, instance_locked) in OPPOSITE_LOCKS {
        let doc = import_rest_json(&rest_fixture(main_locked, instance_locked)).expect("REST import");
        assert_own_lock_survives_sync(doc, instance_locked);
    }
}

#[test]
fn fig_node_level_lock_that_differs_from_its_main_survives_sync_in_both_directions() {
    for (main_locked, instance_locked) in OPPOSITE_LOCKS {
        let doc = import_fig(&fig_fixture(main_locked, instance_locked)).expect(".fig import");
        assert_own_lock_survives_sync(doc, instance_locked);
    }
}
