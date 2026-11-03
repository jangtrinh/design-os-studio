//! A plain resize drops a vector's baked stroke outline and lets the renderers stroke the scaled centerline
//! only when that live stroke draws what the outline drew. Everything the stroke model cannot carry keeps
//! its outline, scaled with the shape: arrow caps (recorded at import, because `StrokeCap` has no name for
//! them), brushes, side weights, vertex caps and joins of their own, dashes on inside or outside strokes.

use designos_doc::*;
use designos_fig::container::{self, Compression};
use designos_fig::kiwi::KiwiValue as V;
use designos_fig::testkit::{en, figma_like_schema, fl, guid, matrix, msg, node, solid, st, vec2};
use designos_fig::{import_fig, import_fig_node_changes, import_rest_json};
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
fn an_arrow_cap_on_the_node_keeps_the_scaled_bake() {
    let (mut doc, id) = imported(icon(json!({"strokeCap": "ARROW_LINES"})));
    assert_eq!(doc.get(id).unwrap().fig_metadata.get("strokeCap").map(String::as_str), Some("ARROW_LINES"));
    resize_2x(&mut doc, id);
    assert_outline_scaled(&doc, id, "arrow cap on the node");
}

#[test]
fn an_arrow_cap_on_a_vertex_keeps_the_scaled_bake() {
    let (mut doc, id) = imported(icon_with_vertex(json!({"strokeCap": "ARROW_EQUILATERAL"})));
    assert_eq!(doc.get(id).unwrap().fig_metadata.get("strokeCap").map(String::as_str), Some("ARROW_EQUILATERAL"));
    resize_2x(&mut doc, id);
    assert_outline_scaled(&doc, id, "arrow cap on a vertex");
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
fn a_vertex_cap_or_join_of_its_own_and_dashes_inside_keep_the_scaled_bake() {
    let (mut doc, id) = imported(icon_with_vertex(json!({"strokeCap": "SQUARE"})));
    resize_2x(&mut doc, id);
    assert_outline_scaled(&doc, id, "vertex cap that differs from the node's");

    let (mut doc, id) = imported(icon_with_vertex(json!({"strokeJoin": "BEVEL"})));
    resize_2x(&mut doc, id);
    assert_outline_scaled(&doc, id, "vertex join that differs from the node's");

    let (mut doc, id) = imported(icon(json!({"strokeAlign": "INSIDE", "strokeDashes": [4.0, 2.0]})));
    resize_2x(&mut doc, id);
    assert_outline_scaled(&doc, id, "dashes on an inside stroke");
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
fn fig_arrow_caps_on_the_node_and_on_a_vertex_keep_the_scaled_bake() {
    let node_cap = fig_icon(json!({"strokeCap": "ARROW_LINES"}), 0);
    let id = node_cap.nodes.values().find(|n| n.name == "Icon").unwrap().id;
    assert_eq!(node_cap.get(id).unwrap().fig_metadata.get("strokeCap").map(String::as_str), Some("ARROW_LINES"));
    let mut doc = node_cap;
    resize_2x(&mut doc, id);
    assert_outline_scaled(&doc, id, ".fig arrow cap on the node");

    let mut doc = fig_icon(
        json!({"vectorData": {
            "vectorNetworkBlob": 1,
            "normalizedSize": {"x": 20.0, "y": 20.0},
            "styleOverrideTable": [{"styleID": 1, "strokeCap": "ARROW_EQUILATERAL"}]
        }}),
        1,
    );
    let id = doc.nodes.values().find(|n| n.name == "Icon").unwrap().id;
    assert_eq!(doc.get(id).unwrap().fig_metadata.get("strokeCap").map(String::as_str), Some("ARROW_EQUILATERAL"));
    resize_2x(&mut doc, id);
    assert_outline_scaled(&doc, id, ".fig arrow cap on a vertex");
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
fn an_arrow_vector_in_an_instance_keeps_its_bake_when_the_instance_is_resized() {
    for padding in [0, 10_100] {
        let (doc, icon) = icon_in_an_instance("ARROW_LINES", padding);
        let n = doc.get(icon).unwrap();
        assert!(doc.nodes.len() >= if padding == 0 { 5 } else { 10_000 }, "padding {padding}: the file has the size the case is about");
        assert_eq!(n.fig_metadata.get("strokeCap").map(String::as_str), Some("ARROW_LINES"), "padding {padding}: the marker survives the clone");
        assert_eq!(n.size, Vec2::new(40.0, 40.0), "padding {padding}: the icon scales with its instance");
        let sg = n.stroke_geometry.as_ref().unwrap_or_else(|| panic!("padding {padding}: the arrow outline is kept"));
        assert_eq!(bounds(sg), (2.0, 2.0, 38.0, 38.0), "padding {padding}: scaled with the icon");

        let (doc, icon) = icon_in_an_instance("ROUND", padding);
        let n = doc.get(icon).unwrap();
        assert_eq!(n.size, Vec2::new(40.0, 40.0), "padding {padding}: the icon scales with its instance");
        assert!(n.stroke_geometry.is_none(), "padding {padding}: a plain stroked icon draws its own weight at the new size");
    }
}
