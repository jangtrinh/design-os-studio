//! An instance that overrides the `arcData` of a nested ellipse (a progress ring) keeps that arc. Without
//! it the ring reverts to the main component's arc as soon as the shape is redrawn from its primitive.
use designos_doc::*;
use designos_fig::import_rest_json;
use serde_json::json;

fn file() -> String {
    let size = json!({"x": 48, "y": 48});
    json!({
        "document": {"type": "DOCUMENT", "children": [{
            "id": "0:1", "type": "CANVAS", "name": "Page 1", "children": [
                {"id": "10:1", "name": "Ring", "type": "COMPONENT", "size": size,
                 "children": [{"id": "10:2", "name": "arc", "type": "ELLIPSE", "size": size,
                               "arcData": {"startingAngle": 0.0, "endingAngle": std::f32::consts::TAU, "innerRadius": 0.8}}]},
                {"id": "1:1", "name": "Ring use", "type": "INSTANCE", "componentId": "10:1", "size": size,
                 "_overrides": [{"path": ["10:2"], "set": {
                     "arcData": {"startingAngle": -std::f32::consts::FRAC_PI_2, "endingAngle": 1.0, "innerRadius": 0.7}}}]}
            ]
        }]}
    })
    .to_string()
}

fn arc_of<'a>(doc: &'a Document, parent: &str) -> &'a ArcData {
    let p = doc.nodes.values().find(|n| n.name == parent).expect("parent");
    let child = p.children.iter().filter_map(|id| doc.get(*id)).find(|n| n.name == "arc").expect("arc child");
    match &child.kind {
        NodeKind::Ellipse(a) => a,
        other => panic!("not an ellipse: {other:?}"),
    }
}

#[test]
fn an_instance_keeps_its_overridden_arc() {
    let doc = import_rest_json(&file()).unwrap();
    let main = arc_of(&doc, "Ring");
    assert!((main.inner_radius - 0.8).abs() < 1e-4, "the main component keeps its arc");
    let a = arc_of(&doc, "Ring use");
    assert!((a.start + std::f32::consts::FRAC_PI_2).abs() < 1e-4 && (a.end - 1.0).abs() < 1e-4 && (a.inner_radius - 0.7).abs() < 1e-4, "{a:?}");
}
