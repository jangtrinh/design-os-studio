//! What a connector does when the layers at both of its ends are deleted in one batch.

use designos_doc::{ConnectorEnd, NodeId, NodeKind};
use designos_mcp::engine::Engine;
use serde_json::json;

fn ends(e: &Engine, id: NodeId) -> (ConnectorEnd, ConnectorEnd) {
    let NodeKind::Connector(props) = &e.doc.get(id).expect("connector").kind else { panic!("expected a connector") };
    (props.start, props.end)
}

fn setup() -> (Engine, NodeId, NodeId, NodeId) {
    let mut e = Engine::default();
    let page = e.resolve_default_page().unwrap();
    let setup = e
        .call(
            "designos_apply_batch",
            json!({"ops":[
                {"op":"insert","as":"a","parent":page.0,"kind":"rectangle","name":"A","x":120.0,"y":120.0,"width":80.0,"height":40.0},
                {"op":"insert","as":"b","parent":page.0,"kind":"rectangle","name":"B","x":300.0,"y":240.0,"width":80.0,"height":40.0},
                {"op":"connect","as":"link","from":"@a","to":"@b"}
            ]}),
        )
        .expect("setup connector fixture");
    let id = |name: &str| NodeId(setup["name_to_id"][name].as_u64().unwrap());
    (e, id("a"), id("b"), id("link"))
}

#[test]
fn deleting_both_ends_in_one_batch_keeps_the_connector_with_its_last_geometry_and_undo_restores_the_ends() {
    let (mut e, a, b, link) = setup();
    let (start_before, end_before) = ends(&e, link);
    let transform_before = e.doc.get(link).unwrap().transform;

    e.call("designos_apply_batch", json!({"ops": [{"op": "delete", "node": a.0}, {"op": "delete", "node": b.0}]})).expect("delete both ends");

    assert!(e.doc.get(a).is_none() && e.doc.get(b).is_none());
    let connector = e.doc.get(link).expect("the connector is not deleted with its ends");
    let (start, end) = ends(&e, link);
    assert_eq!(start.node, start_before.node, "the stored end ids are kept (they point at layers that no longer exist)");
    assert_eq!(end.node, end_before.node);
    assert_eq!((start.point, end.point), (start_before.point, end_before.point), "the end points keep where the ends were");
    // The box is re-derived from the stored points once the ends are gone (it can come out a little taller than the box that
    // routed around the two layers), and what is stored equals what the geometry function gives for the document as it is.
    let geometry = designos_doc::connector_geometry(&e.doc, link).expect("connector geometry");
    assert_eq!((connector.transform, connector.size), (geometry.transform, geometry.size));
    assert_eq!(connector.transform, transform_before, "the box keeps its origin");

    e.call("designos_undo", json!({})).expect("undo the batch");
    assert!(e.doc.get(a).is_some() && e.doc.get(b).is_some(), "undo brings both ends back in one step");
    assert_eq!(ends(&e, link).0.node, Some(a));
    assert_eq!(ends(&e, link).1.node, Some(b));
}
