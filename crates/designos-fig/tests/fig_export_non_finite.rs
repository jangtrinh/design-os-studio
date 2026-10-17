//! `.fig` export must never write a NaN or an infinity. The kiwi encoder writes whatever bits it is
//! given, and a reader (ours turns a non-finite float into an absent field, Figma's behaviour is not
//! ours to promise) would see a damaged number. The exporter writes the neutral value instead: one
//! for opacity, alpha and a matrix diagonal, zero for every other number.

use designos_doc::vector::{Region, Segment, VectorNetwork, Vertex};
use designos_doc::*;
use designos_fig::{commands_blob_to_path, decode_fig_to_json, export_fig, import_fig, path_to_commands_blob};
use serde_json::Value;

/// A rectangle named "Poison"; with `poison` every kind of number it carries is NaN or infinite.
fn rectangle(poison: bool) -> Document {
    let mut d = Document::new("poison");
    let page = d.add_page("P");
    let id = d.alloc_id();
    let rect = d.insert(page, Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
    let n = d.get_mut(rect).unwrap();
    n.name = "Poison".into();
    if poison {
        n.opacity = f32::NAN;
        n.size.x = f32::INFINITY;
        n.transform.a = f32::NAN;
        n.transform.tx = f32::NEG_INFINITY;
        n.corner_radii[0] = f32::NAN;
        n.stroke.weight = f32::NAN;
        n.fills = vec![Paint { opacity: f32::NAN, ..Paint::solid(Color { r: 1.0, g: f32::NAN, b: 0.0, a: f32::NAN }) }];
    }
    d
}

fn nulls_in(v: &Value, path: &mut Vec<String>, found: &mut Vec<String>) {
    match v {
        Value::Null => found.push(path.join(".")),
        Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                path.push(i.to_string());
                nulls_in(x, path, found);
                path.pop();
            }
        }
        Value::Object(m) => {
            for (k, x) in m {
                path.push(k.clone());
                nulls_in(x, path, found);
                path.pop();
            }
        }
        _ => {}
    }
}

fn named<'a>(v: &'a Value, name: &str) -> Option<&'a Value> {
    match v {
        Value::Object(m) if m.get("name").and_then(Value::as_str) == Some(name) => Some(v),
        Value::Object(m) => m.values().find_map(|x| named(x, name)),
        Value::Array(a) => a.iter().find_map(|x| named(x, name)),
        _ => None,
    }
}

#[test]
fn the_exporter_writes_no_non_finite_float() {
    // Control: a clean document decodes with no null (a null is how the decoder shows a non-finite float).
    let mut found = Vec::new();
    nulls_in(&decode_fig_to_json(&export_fig(&rectangle(false)).unwrap()).unwrap(), &mut Vec::new(), &mut found);
    assert!(found.is_empty(), "control: {found:?}");

    let decoded = decode_fig_to_json(&export_fig(&rectangle(true)).unwrap()).unwrap();
    let mut found = Vec::new();
    nulls_in(&decoded, &mut Vec::new(), &mut found);
    assert!(found.is_empty(), "non-finite floats were written at: {found:?}");

    // The neutral values: one for opacity, alpha and the matrix diagonal, zero elsewhere.
    let node = named(&decoded, "Poison").expect("the exported node");
    assert_eq!(node["opacity"], 1.0);
    assert_eq!((node["size"]["x"].as_f64(), node["size"]["y"].as_f64()), (Some(0.0), Some(10.0)));
    assert_eq!((node["transform"]["m00"].as_f64(), node["transform"]["m11"].as_f64()), (Some(1.0), Some(1.0)));
    assert_eq!(node["transform"]["m02"].as_f64(), Some(0.0));
    assert_eq!(node["strokeWeight"].as_f64(), Some(0.0));
    let paint = &node["fillPaints"][0];
    assert_eq!((paint["opacity"].as_f64(), paint["color"]["a"].as_f64(), paint["color"]["g"].as_f64()), (Some(1.0), Some(1.0), Some(0.0)));
}

#[test]
fn an_exported_non_finite_number_re_imports_as_its_neutral_value() {
    let back = import_fig(&export_fig(&rectangle(true)).unwrap()).unwrap();
    assert_eq!(back.to_json_counted().unwrap().1, 0, "nothing non-finite after the round trip");
    let n = back.nodes.values().find(|n| n.name == "Poison").expect("the node survives");
    assert_eq!((n.opacity, n.size.x, n.size.y, n.transform.a, n.transform.tx), (1.0, 0.0, 10.0, 1.0, 0.0));
    let PaintKind::Solid(c) = &n.fills[0].kind else { panic!("solid fill") };
    assert_eq!((n.fills[0].opacity, c.a, c.g), (1.0, 1.0, 0.0));
}

#[test]
fn the_path_commands_blob_never_holds_a_non_finite_coordinate() {
    let path = Path {
        cmds: vec![
            PathCmd::MoveTo(Vec2::new(f32::NAN, 2.0)),
            PathCmd::LineTo(Vec2::new(3.0, f32::INFINITY)),
            PathCmd::CubicTo(Vec2::new(1.0, 1.0), Vec2::new(f32::NEG_INFINITY, 2.0), Vec2::new(4.0, 4.0)),
            PathCmd::Close,
        ],
        fill_rule: FillRule::NonZero,
    };
    let back = commands_blob_to_path(&path_to_commands_blob(&path), false);
    assert_eq!(
        back.cmds,
        vec![
            PathCmd::MoveTo(Vec2::new(0.0, 2.0)),
            PathCmd::LineTo(Vec2::new(3.0, 0.0)),
            PathCmd::CubicTo(Vec2::new(1.0, 1.0), Vec2::new(0.0, 2.0), Vec2::new(4.0, 4.0)),
            PathCmd::Close,
        ]
    );
}

#[test]
fn a_vector_network_with_a_non_finite_vertex_keeps_its_network_on_export() {
    let mut d = Document::new("net");
    let page = d.add_page("P");
    let id = d.alloc_id();
    let net = VectorNetwork {
        vertices: vec![Vertex::new(Vec2::new(f32::NAN, 0.0)), Vertex::new(Vec2::new(10.0, 0.0)), Vertex::new(Vec2::new(10.0, f32::INFINITY))],
        segments: vec![
            Segment { a: 0, b: 1, ta: Vec2::new(f32::NAN, 0.0), tb: Vec2::ZERO },
            Segment { a: 1, b: 2, ta: Vec2::ZERO, tb: Vec2::ZERO },
            Segment { a: 2, b: 0, ta: Vec2::ZERO, tb: Vec2::ZERO },
        ],
        regions: vec![Region { rule: FillRule::NonZero, loops: vec![vec![0, 1, 2]] }],
    };
    let mut node = Node::new(id, NodeKind::Vector { paths: Vec::new(), network: Some(net) });
    node.name = "Net".into();
    node.size = Vec2::new(10.0, 10.0);
    d.insert(page, node).unwrap();

    let back = import_fig(&export_fig(&d).unwrap()).unwrap();
    let n = back.nodes.values().find(|n| n.name == "Net").expect("the node survives");
    let NodeKind::Vector { network: Some(net), .. } = &n.kind else { panic!("the vector network was dropped") };
    assert_eq!((net.vertices[0].pos, net.vertices[2].pos), (Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0)));
    assert_eq!(net.segments[0].ta, Vec2::ZERO);
}
