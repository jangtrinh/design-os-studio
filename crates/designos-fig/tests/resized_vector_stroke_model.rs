//! A plain resize drops a vector's baked stroke outline and lets the renderers stroke the scaled centerline
//! only when that live stroke draws what the outline drew. Arrow caps are carried on `StrokeCap` (node and
//! vertex) and drawn live at the unscaled stroke weight, so they drop their bake on resize too. Everything the
//! stroke model still cannot carry keeps its outline, scaled with the shape: unmodelled caps (recorded in
//! `fig_metadata["strokeCap"]`), brushes, side weights, vertex caps on a non-centered stroke, dashes on inside
//! or outside strokes, or dashes on a centered stroke with a vertex join of its own.

use designos_doc::*;
use designos_fig::container::{self, Compression};
use designos_fig::kiwi::KiwiValue as V;
use designos_fig::testkit::{en, figma_like_schema, fl, guid, matrix, msg, node, solid, st, vec2};
use designos_fig::{decode_fig_to_json, export_fig, export_rest_json, import_fig, import_fig_node_changes, import_rest_json};
use serde_json::{json, Value};

fn file(vector: Value) -> String {
    json!({
        "name": "icons",
        "document": {"id": "0:0", "type": "DOCUMENT", "children": [
            {"id": "0:1", "name": "Page", "type": "CANVAS", "children": [vector]}
        ]}
    })
    .to_string()
}

/// The square-outline icon of the kit: a 2px centered stroke along the centerline `(2, 2)..(18, 18)`,
/// with Figma's baked outline `(1, 1)..(19, 19)` minus `(3, 3)..(17, 17)` as its stroke geometry.
fn icon(extra: Value) -> Value {
    let corner = |x: f32, y: f32| json!({"x": x, "y": y});
    let seg = |a: usize, b: usize| json!({"start": a, "end": b, "tangentStart": {"x": 0.0, "y": 0.0}, "tangentEnd": {"x": 0.0, "y": 0.0}});
    let mut v = json!({
        "id": "1:1", "name": "Icon", "type": "VECTOR",
        "size": {"x": 20.0, "y": 20.0},
        "relativeTransform": [[1, 0, 10], [0, 1, 10]],
        "strokes": [{"type": "SOLID", "color": {"r": 0.1, "g": 0.1, "b": 0.1, "a": 1.0}}],
        "strokeWeight": 2.0, "strokeAlign": "CENTER", "strokeCap": "ROUND", "strokeJoin": "ROUND",
        "vectorNetwork": {
            "vertices": [corner(2.0, 2.0), corner(18.0, 2.0), corner(18.0, 18.0), corner(2.0, 18.0)],
            "segments": [seg(0, 1), seg(1, 2), seg(2, 3), seg(3, 0)],
            "regions": []
        },
        "strokeGeometry": [{"path": "M1 1 L19 1 L19 19 L1 19 Z M3 3 L3 17 L17 17 L17 3 Z", "windingRule": "NONZERO"}]
    });
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    v
}

/// `icon` with `props` set on its first vertex.
fn icon_with_vertex(props: Value) -> Value {
    let mut v = icon(json!({}));
    for (k, x) in props.as_object().unwrap() {
        v["vectorNetwork"]["vertices"][0][k] = x.clone();
    }
    v
}

fn imported(vector: Value) -> (Document, NodeId) {
    let doc = import_rest_json(&file(vector)).unwrap();
    let id = doc.nodes.values().find(|n| n.name == "Icon").expect("the icon is imported").id;
    (doc, id)
}

fn bounds(paths: &[Path]) -> (f32, f32, f32, f32) {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    let mut touch = |p: Vec2| {
        (x0, y0, x1, y1) = (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y));
    };
    for cmd in paths.iter().flat_map(|p| &p.cmds) {
        match *cmd {
            PathCmd::MoveTo(p) | PathCmd::LineTo(p) => touch(p),
            PathCmd::QuadTo(c, p) => {
                touch(c);
                touch(p);
            }
            PathCmd::CubicTo(c1, c2, p) => {
                touch(c1);
                touch(c2);
                touch(p);
            }
            PathCmd::Close => {}
        }
    }
    (x0, y0, x1, y1)
}

fn resize_2x(doc: &mut Document, id: NodeId) {
    let n = doc.get_mut(id).unwrap();
    let (tf, size) = (n.transform, n.size);
    apply_constrained_geometry(n, tf, Vec2::new(size.x * 2.0, size.y * 2.0));
}

/// The baked outline stays and is scaled with the shape.
#[track_caller]
fn assert_outline_scaled(doc: &Document, id: NodeId, what: &str) {
    let n = doc.get(id).unwrap();
    let sg = n.stroke_geometry.as_ref().unwrap_or_else(|| panic!("{what}: the baked outline was dropped"));
    assert_eq!(bounds(sg), (2.0, 2.0, 38.0, 38.0), "{what}: the outline is the 2x scale of Figma's");
    assert_eq!(n.stroke.weight, 2.0, "{what}: the stroke weight is not touched");
}

/// The bake is dropped and the paths are the scaled centerline, stroked at the unscaled weight.
#[track_caller]
fn assert_centerline_stroked_live(doc: &Document, id: NodeId, what: &str) {
    let n = doc.get(id).unwrap();
    assert!(n.stroke_geometry.is_none(), "{what}: the stale outline is dropped");
    let NodeKind::Vector { paths, .. } = &n.kind else { panic!("{what}: not a vector") };
    assert_eq!(bounds(paths), (4.0, 4.0, 36.0, 36.0), "{what}: the paths are the centerline of the 2x resize");
    assert_eq!(n.stroke.weight, 2.0, "{what}: the stroke weight is not touched");
}

#[test]
fn the_kit_icon_drops_its_bake_and_draws_its_own_weight() {
    let (mut doc, id) = imported(icon(json!({})));
    assert!(doc.get(id).unwrap().fig_metadata.is_empty(), "nothing the stroke model lacks is recorded");
    resize_2x(&mut doc, id);
    assert_centerline_stroked_live(&doc, id, "kit icon");
}

#[test]
fn a_vertex_cap_equal_to_the_nodes_and_dashes_on_a_centered_stroke_still_drop_the_bake() {
    let (mut doc, id) = imported(icon_with_vertex(json!({"strokeCap": "ROUND", "strokeJoin": "ROUND"})));
    resize_2x(&mut doc, id);
    assert_centerline_stroked_live(&doc, id, "vertex cap and join equal to the node's");

    let (mut doc, id) = imported(icon(json!({"strokeDashes": [4.0, 2.0]})));
    resize_2x(&mut doc, id);
    assert_centerline_stroked_live(&doc, id, "dashes on a centered stroke");
}

#[test]
fn an_arrow_cap_on_the_node_drops_its_bake_and_is_modelled() {
    let (mut doc, id) = imported(icon(json!({"strokeCap": "ARROW_LINES"})));
    let n = doc.get(id).unwrap();
    assert_eq!(n.stroke.cap, StrokeCap::ArrowLines);
    assert!(!n.fig_metadata.contains_key("strokeCap"), "modelled arrow cap is not recorded in fig_metadata");
    resize_2x(&mut doc, id);
    assert_centerline_stroked_live(&doc, id, "arrow cap on the node");
}

#[test]
fn an_arrow_cap_on_a_vertex_drops_its_bake_and_is_modelled() {
    let (mut doc, id) = imported(icon_with_vertex(json!({"strokeCap": "ARROW_EQUILATERAL"})));
    let n = doc.get(id).unwrap();
    let NodeKind::Vector { network: Some(net), .. } = &n.kind else { panic!("expected vector network") };
    assert_eq!(net.vertices[0].cap, Some(StrokeCap::ArrowEquilateral));
    assert!(!n.fig_metadata.contains_key("strokeCap"), "modelled vertex arrow cap is not recorded in fig_metadata");
    resize_2x(&mut doc, id);
    assert_centerline_stroked_live(&doc, id, "arrow cap on a vertex");
}

#[test]
fn an_unmodelled_cap_on_the_node_or_on_a_vertex_keeps_the_scaled_bake() {
    let (mut doc, id) = imported(icon(json!({"strokeCap": "HIGHLIGHT"})));
    assert_eq!(doc.get(id).unwrap().fig_metadata.get("strokeCap").map(String::as_str), Some("HIGHLIGHT"));
    resize_2x(&mut doc, id);
    assert_outline_scaled(&doc, id, "unmodelled cap on the node");

    let (mut doc, id) = imported(icon_with_vertex(json!({"strokeCap": "WASHI_TAPE_1"})));
    assert_eq!(doc.get(id).unwrap().fig_metadata.get("strokeCap").map(String::as_str), Some("WASHI_TAPE_1"));
    resize_2x(&mut doc, id);
    assert_outline_scaled(&doc, id, "unmodelled cap on a vertex");
}

#[test]
fn a_brush_and_side_weights_keep_the_scaled_bake() {
    let (mut doc, id) = imported(icon(json!({"type": "BRUSH", "brushType": "STRETCH"})));
    resize_2x(&mut doc, id);
    assert_outline_scaled(&doc, id, "brush");

    let (mut doc, id) = imported(icon(json!({"individualStrokeWeights": {"top": 2.0, "right": 4.0, "bottom": 2.0, "left": 4.0}})));
    resize_2x(&mut doc, id);
    assert_outline_scaled(&doc, id, "side weights");
}

#[test]
fn a_vertex_cap_of_its_own_and_dashes_inside_keep_the_scaled_bake_while_a_vertex_join_drops_it() {
    let (mut doc, id) = imported(icon_with_vertex(json!({"strokeCap": "SQUARE"})));
    resize_2x(&mut doc, id);
    assert_outline_scaled(&doc, id, "vertex cap that differs from the node's");

    let (mut doc, id) = imported(icon_with_vertex(json!({"strokeJoin": "BEVEL"})));
    resize_2x(&mut doc, id);
    assert_centerline_stroked_live(&doc, id, "vertex join that differs from the node's");

    let (mut doc, id) = imported(icon(json!({"strokeAlign": "INSIDE", "strokeDashes": [4.0, 2.0]})));
    resize_2x(&mut doc, id);
    assert_outline_scaled(&doc, id, "dashes on an inside stroke");
}

#[test]
fn dashes_on_a_centered_stroke_whose_vertex_join_differs_from_the_nodes_keep_the_scaled_bake() {
    // the live outliner strokes dashes with the node join only, so a vertex join cannot be drawn live
    let mut dashed_with_vertex_join = icon_with_vertex(json!({"strokeJoin": "MITER"}));
    dashed_with_vertex_join["strokeDashes"] = json!([4.0, 2.0]);
    let (mut doc, id) = imported(dashed_with_vertex_join);
    resize_2x(&mut doc, id);
    assert_outline_scaled(&doc, id, "dashes on a centered stroke with a vertex join of its own");

    let mut dashed_with_equal_vertex_join = icon_with_vertex(json!({"strokeJoin": "ROUND"}));
    dashed_with_equal_vertex_join["strokeDashes"] = json!([4.0, 2.0]);
    let (mut doc, id) = imported(dashed_with_equal_vertex_join);
    resize_2x(&mut doc, id);
    assert_centerline_stroked_live(&doc, id, "dashes on a centered stroke with a vertex join equal to the node's");
}

#[test]
fn an_arrow_cap_survives_a_rest_round_trip_and_drops_its_bake_on_a_resize() {
    for (what, (doc, _), expected_node_cap, expected_vertex_cap) in [
        ("node arrow cap", imported(icon(json!({"strokeCap": "ARROW_LINES"}))), StrokeCap::ArrowLines, None),
        ("vertex arrow cap", imported(icon_with_vertex(json!({"strokeCap": "ARROW_EQUILATERAL"}))), StrokeCap::Round, Some(StrokeCap::ArrowEquilateral)),
    ] {
        let rest_json = export_rest_json(&doc);
        let mut reopened = import_rest_json(&rest_json.to_string()).expect("reimport the REST JSON");
        let id = reopened.nodes.values().find(|n| n.name == "Icon").unwrap().id;
        let n = reopened.get(id).unwrap();
        assert!(!n.fig_metadata.contains_key("strokeCap"), "{what}: modelled arrow cap is not in fig_metadata");
        assert_eq!(n.stroke.cap, expected_node_cap, "{what}: the modelled node stroke cap is preserved");
        let NodeKind::Vector { network: Some(net), .. } = &n.kind else { panic!("{what}: expected vector network") };
        assert_eq!(net.vertices[0].cap, expected_vertex_cap, "{what}: the modelled vertex stroke cap is preserved");
        resize_2x(&mut reopened, id);
        assert_centerline_stroked_live(&reopened, id, what);
    }
}

/// `icon` without the stroke alignment: REST omits the field for the default, which is an inside stroke.
fn icon_without_stroke_align(extra: Value) -> Value {
    let mut v = icon(extra);
    v.as_object_mut().unwrap().remove("strokeAlign");
    v
}

/// The open chain `(2, 2) -> (18, 2) -> (18, 18)` with the same baked outline as `icon`.
fn open_icon(extra: Value) -> Value {
    let mut v = icon(extra);
    let corner = |x: f32, y: f32| json!({"x": x, "y": y});
    let seg = |a: usize, b: usize| json!({"start": a, "end": b, "tangentStart": {"x": 0.0, "y": 0.0}, "tangentEnd": {"x": 0.0, "y": 0.0}});
    v["vectorNetwork"] = json!({"vertices": [corner(2.0, 2.0), corner(18.0, 2.0), corner(18.0, 18.0)], "segments": [seg(0, 1), seg(1, 2)], "regions": []});
    v
}

const ARROW_CAPS: [&str; 5] = ["ARROW_LINES", "ARROW_EQUILATERAL", "TRIANGLE_FILLED", "DIAMOND_FILLED", "CIRCLE_FILLED"];

/// Imports `vector`, which carries an arrow cap, resizes it 2x and expects the bake to be dropped.
#[track_caller]
fn assert_arrow_drops_its_bake(vector: Value, what: &str) {
    let (mut doc, id) = imported(vector);
    assert!(doc.get(id).unwrap().stroke.cap.is_arrow(), "{what}: the arrow cap is modelled");
    resize_2x(&mut doc, id);
    assert_centerline_stroked_live(&doc, id, what);
}

#[test]
fn an_arrow_cap_on_an_inside_or_outside_stroke_drops_the_bake_on_a_resize() {
    for cap in ARROW_CAPS {
        for align in ["INSIDE", "OUTSIDE"] {
            assert_arrow_drops_its_bake(icon(json!({"strokeCap": cap, "strokeAlign": align})), &format!("{cap} on a closed {align} stroke"));
            assert_arrow_drops_its_bake(open_icon(json!({"strokeCap": cap, "strokeAlign": align})), &format!("{cap} on an open {align} stroke"));
        }
    }
    let vertex_arrow = icon_with_vertex(json!({"strokeCap": "ARROW_EQUILATERAL", "strokeAlign": "INSIDE"}));
    let (mut doc, id) = imported(vertex_arrow);
    resize_2x(&mut doc, id);
    assert_centerline_stroked_live(&doc, id, "arrow cap on a vertex of an inside stroke");
}

#[test]
fn an_arrow_cap_on_a_stroke_with_no_alignment_given_drops_the_bake_on_a_resize() {
    // REST leaves the alignment out for its default, an inside stroke
    for cap in ARROW_CAPS {
        assert_arrow_drops_its_bake(icon_without_stroke_align(json!({"strokeCap": cap})), &format!("{cap} on a closed stroke with no alignment"));
        let mut open = open_icon(json!({"strokeCap": cap}));
        open.as_object_mut().unwrap().remove("strokeAlign");
        assert_arrow_drops_its_bake(open, &format!("{cap} on an open stroke with no alignment"));
    }
}

#[test]
fn a_stroke_without_an_arrow_cap_keeps_the_bake_rules_it_had_at_every_alignment() {
    for align in ["INSIDE", "OUTSIDE"] {
        let (mut doc, id) = imported(icon(json!({"strokeAlign": align})));
        resize_2x(&mut doc, id);
        assert_centerline_stroked_live(&doc, id, &format!("plain {align} stroke"));

        let (mut doc, id) = imported(icon(json!({"strokeAlign": align, "strokeCap": "HIGHLIGHT"})));
        resize_2x(&mut doc, id);
        assert_outline_scaled(&doc, id, &format!("unmodelled cap on an {align} stroke"));
    }
}

#[test]
fn a_dashed_arrow_stroke_keeps_the_scaled_bake_at_every_alignment() {
    for align in ["CENTER", "INSIDE", "OUTSIDE"] {
        let (mut doc, id) = imported(icon(json!({"strokeCap": "ARROW_LINES", "strokeAlign": align, "strokeDashes": [4.0, 2.0]})));
        resize_2x(&mut doc, id);
        assert_outline_scaled(&doc, id, &format!("dashed {align} stroke with an arrow cap"));
    }
}

/// `icon` with `props` set on its vertex number `index`.
fn icon_with_vertex_at(index: usize, node_props: Value, props: Value) -> Value {
    let mut v = icon(node_props);
    for (k, x) in props.as_object().unwrap() {
        v["vectorNetwork"]["vertices"][index][k] = x.clone();
    }
    v
}

fn named<'a>(v: &'a Value, name: &str) -> Option<&'a Value> {
    named_any(v, name, "VECTOR")
}

fn named_any<'a>(v: &'a Value, name: &str, node_type: &str) -> Option<&'a Value> {
    if v.get("name").and_then(Value::as_str) == Some(name) && v.get("type").is_some_and(|t| t == node_type) {
        return Some(v);
    }
    ["document", "children"].iter().find_map(|k| match v.get(*k)? {
        Value::Array(items) => items.iter().find_map(|c| named_any(c, name, node_type)),
        o => named_any(o, name, node_type),
    })
}

/// The cap names written for the exported icon: its own, then those of its vertices (`None` where a vertex has none).
fn written_caps(rest: &Value) -> (String, Vec<Option<String>>) {
    caps_of(named(rest, "Icon").expect("the exported icon"))
}

fn caps_of(icon: &Value) -> (String, Vec<Option<String>>) {
    let node = icon["strokeCap"].as_str().expect("the node's strokeCap").to_string();
    let vertices = icon["vectorNetwork"]["vertices"].as_array().expect("the exported vertices");
    (node, vertices.iter().map(|x| x.get("strokeCap").and_then(Value::as_str).map(String::from)).collect())
}

/// Exports `vector` (carrying a modelled arrow cap) as REST JSON, checks the cap names written for the node and for
/// each vertex, then reopens the JSON: the arrow cap must be in the model (`fig_metadata["strokeCap"]` absent), and a
/// resize must drop the bake.
#[track_caller]
fn assert_arrow_cap_round_trips(what: &str, vector: Value, node_cap: &str, vertex_caps: [Option<&str>; 4]) {
    let (doc, _) = imported(vector);
    let icon_node = doc.nodes.values().find(|n| n.name == "Icon").unwrap();
    assert!(!icon_node.fig_metadata.contains_key("strokeCap"), "{what}: modelled arrow cap is not recorded in fig_metadata");
    let cap_before = icon_node.stroke.cap;

    let rest = export_rest_json(&doc);
    let (written_node, written_vertices) = written_caps(&rest);
    let expected_vertices: Vec<Option<String>> = vertex_caps.iter().map(|c| c.map(String::from)).collect();
    assert_eq!(written_node, node_cap, "{what}: the node's strokeCap");
    assert_eq!(written_vertices, expected_vertices, "{what}: the strokeCap of each vertex");

    let mut reopened = import_rest_json(&rest.to_string()).expect("reimport the REST JSON");
    let id = reopened.nodes.values().find(|n| n.name == "Icon").unwrap().id;
    let n = reopened.get(id).unwrap();
    assert!(!n.fig_metadata.contains_key("strokeCap"), "{what}: modelled arrow cap stays out of fig_metadata");
    assert_eq!(n.stroke.cap, cap_before, "{what}: the modelled stroke cap is preserved");
    resize_2x(&mut reopened, id);
    assert_centerline_stroked_live(&reopened, id, what);

    // the second export writes it to the same place again
    let (again_node, again_vertices) = written_caps(&export_rest_json(&import_rest_json(&rest.to_string()).unwrap()));
    assert_eq!((again_node.as_str(), again_vertices), (node_cap, expected_vertices), "{what}: stable on a second round trip");
}

/// Exports `vector` (carrying an unmodelled cap) as REST JSON, checks the cap names written for the node and for each
/// vertex, then reopens the JSON: the unmodelled cap must be recorded in `fig_metadata["strokeCap"]`, and a resize must
/// keep the scaled bake.
#[track_caller]
fn assert_unmodelled_cap_round_trips(what: &str, vector: Value, node_cap: &str, vertex_caps: [Option<&str>; 4]) {
    let (doc, _) = imported(vector);
    let icon_node = doc.nodes.values().find(|n| n.name == "Icon").unwrap();
    let marker = icon_node.fig_metadata.get("strokeCap").cloned().expect("the unmodelled cap is recorded");
    let cap_before = icon_node.stroke.cap;

    let rest = export_rest_json(&doc);
    let (written_node, written_vertices) = written_caps(&rest);
    let expected_vertices: Vec<Option<String>> = vertex_caps.iter().map(|c| c.map(String::from)).collect();
    assert_eq!(written_node, node_cap, "{what}: the node's strokeCap");
    assert_eq!(written_vertices, expected_vertices, "{what}: the strokeCap of each vertex");

    let mut reopened = import_rest_json(&rest.to_string()).expect("reimport the REST JSON");
    let id = reopened.nodes.values().find(|n| n.name == "Icon").unwrap().id;
    let n = reopened.get(id).unwrap();
    assert_eq!(n.fig_metadata.get("strokeCap"), Some(&marker), "{what}: the unmodelled cap survives the round trip");
    assert_eq!(n.stroke.cap, cap_before, "{what}: the modelled stroke cap is preserved");
    resize_2x(&mut reopened, id);
    assert_outline_scaled(&reopened, id, what);

    let (again_node, again_vertices) = written_caps(&export_rest_json(&import_rest_json(&rest.to_string()).unwrap()));
    assert_eq!((again_node.as_str(), again_vertices), (node_cap, expected_vertices), "{what}: stable on a second round trip");
}

fn icon_with_arrow_on_vertex_3(node_cap: &str, arrow: &str) -> Value {
    icon_with_vertex_at(3, json!({"strokeCap": node_cap}), json!({"strokeCap": arrow}))
}

#[test]
fn a_rest_export_writes_a_vertex_arrow_back_onto_its_vertex_when_the_node_has_a_cap() {
    assert_arrow_cap_round_trips(
        "round node, arrow on vertex 3",
        icon_with_arrow_on_vertex_3("ROUND", "ARROW_EQUILATERAL"),
        "ROUND",
        [None, None, None, Some("ARROW_EQUILATERAL")],
    );
    assert_unmodelled_cap_round_trips(
        "round node, unmodelled cap on vertex 3",
        icon_with_arrow_on_vertex_3("ROUND", "WASHI_TAPE_1"),
        "ROUND",
        [None, None, None, Some("WASHI_TAPE_1")],
    );
}

#[test]
fn a_rest_export_keeps_a_vertex_arrow_off_a_plain_node() {
    assert_arrow_cap_round_trips(
        "plain node, arrow on vertex 3",
        icon_with_arrow_on_vertex_3("NONE", "ARROW_LINES"),
        "NONE",
        [None, None, None, Some("ARROW_LINES")],
    );
    assert_unmodelled_cap_round_trips(
        "plain node, unmodelled cap on vertex 3",
        icon_with_arrow_on_vertex_3("NONE", "WASHI_TAPE_1"),
        "NONE",
        [None, None, None, Some("WASHI_TAPE_1")],
    );
}

#[test]
fn a_rest_export_writes_a_node_arrow_into_the_nodes_stroke_cap() {
    assert_arrow_cap_round_trips("arrow on the node", icon(json!({"strokeCap": "ARROW_LINES"})), "ARROW_LINES", [None, None, None, None]);
    assert_unmodelled_cap_round_trips("unmodelled cap on the node", icon(json!({"strokeCap": "HIGHLIGHT"})), "HIGHLIGHT", [None, None, None, None]);
}

/// A REST file with a main component holding an icon that has `cap` on vertex 3, and an instance of it without
/// children of its own, so that the importer clones the component's layers; with `padding` rectangles the file has
/// the 10,000 nodes or more at which it clones them in bulk.
fn rest_instance_of_an_arrow_icon(cap: &str, padding: u32) -> Value {
    let mut icon_in_main = icon_with_arrow_on_vertex_3("ROUND", cap);
    icon_in_main["id"] = json!("1:2");
    icon_in_main["relativeTransform"] = json!([[1, 0, 0], [0, 1, 0]]);
    let mut children = vec![
        json!({"id": "1:1", "name": "Main", "type": "COMPONENT", "size": {"x": 20.0, "y": 20.0}, "relativeTransform": [[1, 0, 0], [0, 1, 0]], "children": [icon_in_main]}),
        json!({"id": "1:3", "name": "Use", "type": "INSTANCE", "componentId": "1:1", "size": {"x": 40.0, "y": 40.0}, "relativeTransform": [[1, 0, 60], [0, 1, 0]]}),
    ];
    for i in 0..padding {
        children.push(json!({"id": format!("9:{i}"), "name": format!("Pad {i}"), "type": "RECTANGLE", "size": {"x": 2.0, "y": 2.0}, "relativeTransform": [[1, 0, 0], [0, 1, 100]]}));
    }
    json!({"name": "icons", "document": {"id": "0:0", "type": "DOCUMENT", "children": [{"id": "0:1", "name": "Page", "type": "CANVAS", "children": children}]}})
}

#[test]
fn an_instance_clone_of_an_arrow_icon_writes_the_arrow_back_onto_its_vertex_at_any_file_size() {
    for cap in ["ARROW_EQUILATERAL", "WASHI_TAPE_1"] {
        for padding in [0, 10_100] {
            let doc = import_rest_json(&rest_instance_of_an_arrow_icon(cap, padding).to_string()).unwrap();
            let rest = export_rest_json(&doc);
            let instance = named_any(&rest, "Use", "INSTANCE").expect("the exported instance");
            let icon = named(instance, "Icon").unwrap_or_else(|| panic!("padding {padding}: the instance holds a copy of the icon"));
            assert_eq!(
                caps_of(icon),
                ("ROUND".to_string(), vec![None, None, None, Some(cap.to_string())]),
                "{cap} padding {padding}: the cloned icon keeps where its cap sat"
            );
        }
    }
}

#[test]
fn a_document_recorded_without_the_cap_source_keeps_writing_the_arrow_where_it_always_did() {
    let (mut doc, id) = imported(icon_with_vertex_at(3, json!({"strokeCap": "ROUND"}), json!({"strokeCap": "WASHI_TAPE_1"})));
    doc.get_mut(id).unwrap().fig_metadata.retain(|k, _| k == "strokeCap");
    let (node_cap, vertex_caps) = written_caps(&export_rest_json(&doc));
    assert_eq!(node_cap, "ROUND", "a modelled node cap stays the node's strokeCap");
    assert_eq!(vertex_caps.iter().position(|c| c.is_some()), Some(0), "the unmodelled cap goes to the first vertex without a cap");

    let (mut doc, id) = imported(icon_with_vertex_at(3, json!({"strokeCap": "NONE"}), json!({"strokeCap": "HIGHLIGHT"})));
    doc.get_mut(id).unwrap().fig_metadata.retain(|k, _| k == "strokeCap");
    let (node_cap, vertex_caps) = written_caps(&export_rest_json(&doc));
    assert_eq!(node_cap, "HIGHLIGHT", "an unmodelled cap fills a plain node's strokeCap");
    assert!(vertex_caps.iter().all(|c| c.is_none()));
}

// ---- `.fig` files ---------------------------------------------------------------------------------

/// Commands blob of a closed polygon through `pts`.
fn commands_blob(pts: &[(f32, f32)]) -> Vec<u8> {
    let mut b = Vec::new();
    for (i, (x, y)) in pts.iter().enumerate() {
        b.push(if i == 0 { 1u8 } else { 2u8 });
        b.extend(x.to_le_bytes());
        b.extend(y.to_le_bytes());
    }
    b.push(0u8);
    b
}

/// `vectorNetworkBlob` of the closed square `(2, 2)..(18, 18)`; `first_style` is the style id of its first vertex.
fn square_network_blob(first_style: u32) -> Vec<u8> {
    let mut b = Vec::new();
    for count in [4u32, 4, 0] {
        b.extend(count.to_le_bytes());
    }
    for (i, (x, y)) in [(2.0f32, 2.0f32), (18.0, 2.0), (18.0, 18.0), (2.0, 18.0)].into_iter().enumerate() {
        b.extend((if i == 0 { first_style } else { 0 }).to_le_bytes());
        b.extend(x.to_le_bytes());
        b.extend(y.to_le_bytes());
    }
    for (a, e) in [(0u32, 1u32), (1, 2), (2, 3), (3, 0)] {
        b.extend(0u32.to_le_bytes());
        b.extend(a.to_le_bytes());
        b.extend([0u8; 8]);
        b.extend(e.to_le_bytes());
        b.extend([0u8; 8]);
    }
    b
}

fn fig_guid(s: u64, l: u64) -> Value {
    json!({"sessionID": s, "localID": l})
}

fn fig_node(id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str, extra: Value) -> Value {
    let mut v = json!({
        "guid": fig_guid(id.0, id.1),
        "parentIndex": {"guid": fig_guid(parent.0, parent.1), "position": pos},
        "type": ty, "name": name, "phase": "CREATED", "visible": true, "opacity": 1.0,
        "size": {"x": 20.0, "y": 20.0},
        "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
    });
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    v
}

/// The `.fig` fields of the square-outline icon (blob 0 is its outline, blob 1 its network).
fn fig_vector_fields(extra: Value) -> Value {
    let mut vector = json!({
        "strokePaints": [{"type": "SOLID", "color": {"r": 0.1, "g": 0.1, "b": 0.1, "a": 1.0}, "opacity": 1.0, "visible": true}],
        "strokeWeight": 2.0, "strokeAlign": "CENTER", "strokeCap": "ROUND", "strokeJoin": "ROUND",
        "vectorData": {"vectorNetworkBlob": 1, "normalizedSize": {"x": 20.0, "y": 20.0}},
        "strokeGeometry": [{"windingRule": "NONZERO", "commandsBlob": 0}]
    });
    for (k, x) in extra.as_object().unwrap() {
        vector[k] = x.clone();
    }
    vector
}

fn fig_blobs(first_style: u32) -> Value {
    let mut ring = commands_blob(&[(1.0, 1.0), (19.0, 1.0), (19.0, 19.0), (1.0, 19.0)]);
    ring.extend(commands_blob(&[(3.0, 3.0), (3.0, 17.0), (17.0, 17.0), (17.0, 3.0)]));
    json!([{"bytes": ring}, {"bytes": square_network_blob(first_style)}])
}

fn fig_icon(extra: Value, first_style: u32) -> Document {
    let msg = json!({
        "blobs": fig_blobs(first_style),
        "nodeChanges": [
            fig_node((0, 0), (0, 0), "", "DOCUMENT", "Doc", json!({})),
            fig_node((0, 1), (0, 0), "!", "CANVAS", "Page 1", json!({})),
            fig_node((1, 10), (0, 1), "!", "VECTOR", "Icon", fig_vector_fields(extra))
        ]
    });
    import_fig_node_changes(&msg, "Doc").expect("import the .fig message")
}

#[test]
fn fig_arrow_caps_on_the_node_and_on_a_vertex_are_modelled_and_drop_the_bake() {
    let mut doc = fig_icon(json!({"strokeCap": "ARROW_LINES"}), 0);
    let id = doc.nodes.values().find(|n| n.name == "Icon").unwrap().id;
    assert_eq!(doc.get(id).unwrap().stroke.cap, StrokeCap::ArrowLines);
    assert!(!doc.get(id).unwrap().fig_metadata.contains_key("strokeCap"));
    resize_2x(&mut doc, id);
    assert_centerline_stroked_live(&doc, id, ".fig arrow cap on the node");

    let mut doc = fig_icon(
        json!({"vectorData": {
            "vectorNetworkBlob": 1,
            "normalizedSize": {"x": 20.0, "y": 20.0},
            "styleOverrideTable": [{"styleID": 1, "strokeCap": "ARROW_EQUILATERAL"}]
        }}),
        1,
    );
    let id = doc.nodes.values().find(|n| n.name == "Icon").unwrap().id;
    let NodeKind::Vector { network: Some(net), .. } = &doc.get(id).unwrap().kind else { panic!("expected vector network") };
    assert_eq!(net.vertices[0].cap, Some(StrokeCap::ArrowEquilateral));
    assert!(!doc.get(id).unwrap().fig_metadata.contains_key("strokeCap"));
    resize_2x(&mut doc, id);
    assert_centerline_stroked_live(&doc, id, ".fig arrow cap on a vertex");
}

#[test]
fn fig_unmodelled_caps_on_the_node_and_on_a_vertex_keep_the_scaled_bake() {
    let mut doc = fig_icon(json!({"strokeCap": "HIGHLIGHT"}), 0);
    let id = doc.nodes.values().find(|n| n.name == "Icon").unwrap().id;
    assert_eq!(doc.get(id).unwrap().fig_metadata.get("strokeCap").map(String::as_str), Some("HIGHLIGHT"));
    resize_2x(&mut doc, id);
    assert_outline_scaled(&doc, id, ".fig unmodelled cap on the node");

    let mut doc = fig_icon(
        json!({"vectorData": {
            "vectorNetworkBlob": 1,
            "normalizedSize": {"x": 20.0, "y": 20.0},
            "styleOverrideTable": [{"styleID": 1, "strokeCap": "WASHI_TAPE_1"}]
        }}),
        1,
    );
    let id = doc.nodes.values().find(|n| n.name == "Icon").unwrap().id;
    assert_eq!(doc.get(id).unwrap().fig_metadata.get("strokeCap").map(String::as_str), Some("WASHI_TAPE_1"));
    resize_2x(&mut doc, id);
    assert_outline_scaled(&doc, id, ".fig unmodelled cap on a vertex");
}

#[test]
fn fig_icon_without_an_arrow_cap_drops_its_bake() {
    let mut doc = fig_icon(json!({}), 0);
    let id = doc.nodes.values().find(|n| n.name == "Icon").unwrap().id;
    assert!(!doc.get(id).unwrap().fig_metadata.contains_key("strokeCap"));
    resize_2x(&mut doc, id);
    assert_centerline_stroked_live(&doc, id, ".fig icon");
}

/// A `.fig` file with a main component holding the icon and an instance of it at twice the size, whose icon
/// scales with it. With `padding` rectangles on top, the file is large enough (10,000 nodes or more) for the
/// importer to clone the instance's layers in bulk; the clone keeps what decides how a resize treats the
/// baked stroke.
fn icon_in_an_instance(stroke_cap: &str, padding: u32) -> (Document, NodeId) {
    let scale = |mut f: Vec<(&'static str, V)>| {
        f.push(("horizontalConstraint", en("SCALE")));
        f.push(("verticalConstraint", en("SCALE")));
        f
    };
    let geo = V::Array(vec![msg(vec![("windingRule", en("NONZERO")), ("commandsBlob", V::Uint(0))])]);
    let mut changes = vec![
        node((0, 0), (0, 0), "!", "DOCUMENT", "Document", vec![]),
        node((0, 1), (0, 0), "a", "CANVAS", "Page 1", vec![]),
        node((1, 1), (0, 1), "a", "SYMBOL", "Main", scale(vec![("size", vec2(20.0, 20.0)), ("transform", matrix(0.0, 0.0))])),
        node(
            (1, 2),
            (1, 1),
            "a",
            "VECTOR",
            "Icon",
            scale(vec![
                ("size", vec2(20.0, 20.0)),
                ("transform", matrix(0.0, 0.0)),
                ("strokePaints", V::Array(vec![solid(0.1, 0.1, 0.1)])),
                ("strokeWeight", fl(2.0)),
                ("strokeAlign", en("CENTER")),
                ("strokeCap", en(stroke_cap)),
                ("strokeJoin", en("ROUND")),
                ("vectorData", msg(vec![("vectorNetworkBlob", V::Uint(1)), ("normalizedSize", vec2(20.0, 20.0))])),
                ("strokeGeometry", geo),
            ]),
        ),
        node(
            (1, 3),
            (0, 1),
            "b",
            "INSTANCE",
            "Use",
            vec![("size", vec2(40.0, 40.0)), ("transform", matrix(60.0, 0.0)), ("symbolData", msg(vec![("symbolID", guid(1, 1))]))],
        ),
    ];
    for i in 0..padding {
        changes.push(node((9, i), (0, 1), &format!("z{i:05}"), "RECTANGLE", &format!("Pad {i}"), vec![("size", vec2(2.0, 2.0))]));
    }
    let mut ring = commands_blob(&[(1.0, 1.0), (19.0, 1.0), (19.0, 19.0), (1.0, 19.0)]);
    ring.extend(commands_blob(&[(3.0, 3.0), (3.0, 17.0), (17.0, 17.0), (17.0, 3.0)]));
    let blobs = V::Array(vec![st(vec![("bytes", V::Bytes(ring))]), st(vec![("bytes", V::Bytes(square_network_blob(0)))])]);
    let message = msg(vec![("nodeChanges", V::Array(changes)), ("blobs", blobs)]);
    let schema = figma_like_schema();
    let body = schema.encode_message(schema.find("Message").unwrap(), &message).unwrap();
    let bytes = container::write_container(&schema.encode(), &body, Compression::Zstd, 101);
    let doc = import_fig(&bytes).expect("import the .fig file");
    let instance = doc.nodes.values().find(|n| n.name == "Use").expect("the instance").id;
    let icon = doc.get(instance).unwrap().children.iter().copied().find(|c| doc.get(*c).is_some_and(|n| n.name == "Icon")).expect("the instance's icon");
    (doc, icon)
}

#[test]
fn an_arrow_vector_in_an_instance_drops_its_bake_while_an_unmodelled_cap_keeps_it() {
    for padding in [0, 10_100] {
        let (doc, icon) = icon_in_an_instance("ARROW_LINES", padding);
        let n = doc.get(icon).unwrap();
        assert!(doc.nodes.len() >= if padding == 0 { 5 } else { 10_000 }, "padding {padding}: the file has the size the case is about");
        assert_eq!(n.stroke.cap, StrokeCap::ArrowLines, "padding {padding}: the arrow cap is modelled on the clone");
        assert!(!n.fig_metadata.contains_key("strokeCap"), "padding {padding}: modelled arrow cap is not in fig_metadata");
        assert_eq!(n.size, Vec2::new(40.0, 40.0), "padding {padding}: the icon scales with its instance");
        assert!(n.stroke_geometry.is_none(), "padding {padding}: modelled arrow vector drops its bake and draws live");

        let (doc, icon) = icon_in_an_instance("ROUND", padding);
        let n = doc.get(icon).unwrap();
        assert_eq!(n.size, Vec2::new(40.0, 40.0), "padding {padding}: the icon scales with its instance");
        assert!(n.stroke_geometry.is_none(), "padding {padding}: a plain stroked icon draws its own weight at the new size");
    }
}

/// Saved as a `.fig` file and opened again.
fn saved_and_reopened(doc: &Document) -> Document {
    import_fig(&export_fig(doc).expect("export the .fig file")).expect("reopen the .fig file")
}

#[test]
fn an_arrow_cap_survives_a_saved_fig_file_and_the_reopened_vector_drops_its_bake_on_a_resize() {
    for (what, doc, expected_written_node, expected_node_cap, expected_vertex_cap) in [
        ("node cap", fig_icon(json!({"strokeCap": "ARROW_LINES"}), 0), "ARROW_LINES", StrokeCap::ArrowLines, None),
        (
            "vertex cap",
            fig_icon(
                json!({"vectorData": {
                    "vectorNetworkBlob": 1,
                    "normalizedSize": {"x": 20.0, "y": 20.0},
                    "styleOverrideTable": [{"styleID": 1, "strokeCap": "ARROW_EQUILATERAL"}]
                }}),
                1,
            ),
            "ROUND",
            StrokeCap::Round,
            Some(StrokeCap::ArrowEquilateral),
        ),
    ] {
        let decoded = decode_fig_to_json(&export_fig(&doc).unwrap()).unwrap();
        let written = decoded["message"]["nodeChanges"].as_array().unwrap().iter().find(|nc| nc["name"] == "Icon").unwrap()["strokeCap"].clone();
        assert_eq!(written, expected_written_node, "{what}: the cap written to the file is Figma's enum name");
        let mut reopened = saved_and_reopened(&doc);
        let id = reopened.nodes.values().find(|n| n.name == "Icon").unwrap().id;
        let n = reopened.get(id).unwrap();
        assert!(!n.fig_metadata.contains_key("strokeCap"), "{what}: modelled arrow cap is not in fig_metadata");
        assert_eq!(n.stroke.cap, expected_node_cap, "{what}: the exported node stroke cap round-trips");
        let NodeKind::Vector { network: Some(net), .. } = &n.kind else { panic!("{what}: expected vector network") };
        assert_eq!(net.vertices[0].cap, expected_vertex_cap, "{what}: the exported vertex stroke cap round-trips");
        resize_2x(&mut reopened, id);
        assert_centerline_stroked_live(&reopened, id, what);
    }
}

#[test]
fn a_saved_fig_file_of_a_plain_icon_carries_no_cap_marker_and_still_drops_its_bake() {
    let doc = fig_icon(json!({}), 0);
    let mut reopened = saved_and_reopened(&doc);
    let id = reopened.nodes.values().find(|n| n.name == "Icon").unwrap().id;
    assert!(reopened.get(id).unwrap().fig_metadata.is_empty(), "nothing is carried for an icon without an arrow cap");
    resize_2x(&mut reopened, id);
    assert_centerline_stroked_live(&reopened, id, "reopened plain icon");
}

#[test]
fn all_five_arrow_caps_round_trip_through_rest_and_fig_on_node_and_vertex_and_plain_json_stays_byte_identical() {
    let caps = [
        ("ARROW_LINES", StrokeCap::ArrowLines),
        ("ARROW_EQUILATERAL", StrokeCap::ArrowEquilateral),
        ("TRIANGLE_FILLED", StrokeCap::TriangleFilled),
        ("DIAMOND_FILLED", StrokeCap::DiamondFilled),
        ("CIRCLE_FILLED", StrokeCap::CircleFilled),
    ];

    for (figma_name, expected) in caps {
        let (doc, id) = imported(icon(json!({"strokeCap": figma_name})));
        let n = doc.get(id).unwrap();
        assert_eq!(n.stroke.cap, expected, "REST node import for {figma_name}");
        assert!(!n.fig_metadata.contains_key("strokeCap"), "REST node {figma_name} not in fig_metadata");
        let rest_out = export_rest_json(&doc);
        let re_rest = import_rest_json(&rest_out.to_string()).unwrap();
        let re_id = re_rest.nodes.values().find(|n| n.name == "Icon").unwrap().id;
        assert_eq!(re_rest.get(re_id).unwrap().stroke.cap, expected, "REST node round-trip for {figma_name}");

        let (vdoc, vid) = imported(icon_with_vertex(json!({"strokeCap": figma_name})));
        let vn = vdoc.get(vid).unwrap();
        let NodeKind::Vector { network: Some(net), .. } = &vn.kind else { panic!("expected vector network") };
        assert_eq!(net.vertices[0].cap, Some(expected), "REST vertex import for {figma_name}");
        assert!(!vn.fig_metadata.contains_key("strokeCap"), "REST vertex {figma_name} not in fig_metadata");
        let vrest_out = export_rest_json(&vdoc);
        let vre_rest = import_rest_json(&vrest_out.to_string()).unwrap();
        let vre_id = vre_rest.nodes.values().find(|n| n.name == "Icon").unwrap().id;
        let NodeKind::Vector { network: Some(vnet), .. } = &vre_rest.get(vre_id).unwrap().kind else { panic!("expected vector network") };
        assert_eq!(vnet.vertices[0].cap, Some(expected), "REST vertex round-trip for {figma_name}");

        let fdoc = fig_icon(json!({"strokeCap": figma_name}), 0);
        let fid = fdoc.nodes.values().find(|n| n.name == "Icon").unwrap().id;
        assert_eq!(fdoc.get(fid).unwrap().stroke.cap, expected, ".fig node import for {figma_name}");
        let f_reopened = saved_and_reopened(&fdoc);
        let f_re_id = f_reopened.nodes.values().find(|n| n.name == "Icon").unwrap().id;
        assert_eq!(f_reopened.get(f_re_id).unwrap().stroke.cap, expected, ".fig node round-trip for {figma_name}");

        let fvdoc = fig_icon(
            json!({"vectorData": {
                "vectorNetworkBlob": 1,
                "normalizedSize": {"x": 20.0, "y": 20.0},
                "styleOverrideTable": [{"styleID": 1, "strokeCap": figma_name}]
            }}),
            1,
        );
        let fvid = fvdoc.nodes.values().find(|n| n.name == "Icon").unwrap().id;
        let NodeKind::Vector { network: Some(fnet), .. } = &fvdoc.get(fvid).unwrap().kind else { panic!("expected vector network") };
        assert_eq!(fnet.vertices[0].cap, Some(expected), ".fig vertex import for {figma_name}");
        let fv_reopened = saved_and_reopened(&fvdoc);
        let fv_re_id = fv_reopened.nodes.values().find(|n| n.name == "Icon").unwrap().id;
        let NodeKind::Vector { network: Some(fvnet), .. } = &fv_reopened.get(fv_re_id).unwrap().kind else { panic!("expected vector network") };
        assert_eq!(fvnet.vertices[0].cap, Some(expected), ".fig vertex round-trip for {figma_name}");
    }

    for (cap, token) in [(StrokeCap::None, "\"cap\":\"None\""), (StrokeCap::Round, "\"cap\":\"Round\""), (StrokeCap::Square, "\"cap\":\"Square\"")] {
        let s = Stroke { cap, ..Stroke::default() };
        let j = serde_json::to_string(&s).unwrap();
        assert!(j.contains(token), "plain cap {cap:?} serializes with {token} in {j}");
        let back: Stroke = serde_json::from_str(&j).unwrap();
        assert_eq!(back.cap, cap);
        assert_eq!(serde_json::to_string(&back).unwrap(), j, "byte-identical round-trip for {cap:?}");
    }
}
