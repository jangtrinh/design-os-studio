//! A `.pen` export writes the arc that an instance's ring draws itself (a progress ring that shows another sweep than
//! its main) into the instance's descendant override, and a reopened file keeps it as the instance's own arc, so a
//! later change of the main's arc no longer overwrites it. An instance that follows its main writes no arc.
use designos_doc::*;
use designos_fig::{export_pen_json, import_pen_json};
use serde_json::Value;

const MAIN_ARC: ArcData = ArcData { start: 0.0, end: std::f32::consts::TAU, inner_radius: 0.8 };
const OWN_ARC: ArcData = ArcData { start: -1.5, end: 1.0, inner_radius: 0.7 };
/// The `.pen` text rounds angles to a thousandth of a degree.
const ANGLE_TOLERANCE: f32 = 1e-3;

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

fn ring_layer(doc: &Document, instance: &str) -> NodeId {
    let inst = find(doc, instance);
    *inst.children.iter().find(|c| matches!(doc.get(**c).map(|n| &n.kind), Some(NodeKind::Ellipse(_)))).expect("ring layer")
}

fn arc_of(doc: &Document, id: NodeId) -> ArcData {
    match &doc.get(id).unwrap().kind {
        NodeKind::Ellipse(a) => *a,
        other => panic!("not an ellipse: {other:?}"),
    }
}

fn assert_arc_close(got: ArcData, want: ArcData, what: &str) {
    let close = |a: f32, b: f32| (a - b).abs() < ANGLE_TOLERANCE;
    assert!(close(got.start, want.start) && close(got.end, want.end) && close(got.inner_radius, want.inner_radius), "{what}: {got:?} is not {want:?}");
}

fn set_arc(doc: &mut Document, id: NodeId, to: ArcData) {
    let before = doc.get(id).unwrap().clone();
    let mut n = before.clone();
    n.kind = NodeKind::Ellipse(to);
    if n.component.source.is_some() {
        n.fill_geometry = None;
        n.component.overrides.extend(record_override(&before, &n));
    }
    Command::Update { node: Box::new(n) }.apply(doc).unwrap();
}

/// A ring main and two instances of it: `Own` draws `own`, `Follows` draws the main's arc.
fn rings(own: ArcData) -> Document {
    let mut doc = Document::new("rings");
    let page = doc.add_page("Page 1");
    let frame_id = doc.alloc_id();
    let main = doc.insert(page, Node::frame(frame_id, Rect::new(0.0, 0.0, 48.0, 48.0))).unwrap();
    doc.get_mut(main).unwrap().name = "Ring".into();
    let id = doc.alloc_id();
    let mut e = Node::ellipse(id, Rect::new(0.0, 0.0, 48.0, 48.0));
    e.name = "MainArc".into();
    e.kind = NodeKind::Ellipse(MAIN_ARC);
    e.fills = vec![Paint::solid(Color::rgb(255, 0, 0))];
    doc.insert(main, e).unwrap();
    create_component(&mut doc, main).apply(&mut doc).unwrap();
    for (name, x) in [("Own", 100.0), ("Follows", 200.0)] {
        create_instance(&mut doc, main, page, Vec2::new(x, 0.0)).apply(&mut doc).unwrap();
        let inst = *doc.get(page).unwrap().children.last().unwrap();
        doc.get_mut(inst).unwrap().name = name.into();
    }
    let own_ring = ring_layer(&doc, "Own");
    set_arc(&mut doc, own_ring, own);
    assert_eq!(arc_of(&doc, own_ring), own, "fixture: the instance draws its own arc");
    assert_eq!(arc_of(&doc, ring_layer(&doc, "Follows")), MAIN_ARC, "fixture: the other instance draws the main's");
    doc
}

fn json_node<'a>(v: &'a Value, name: &str) -> Option<&'a Value> {
    if v.get("name").and_then(Value::as_str) == Some(name) {
        return Some(v);
    }
    v.get("children")?.as_array()?.iter().find_map(|c| json_node(c, name))
}

/// The override that the `.pen` instance `name` writes for the ring layer of its main, if any.
fn ring_override<'a>(json: &'a Value, name: &str) -> Option<&'a serde_json::Map<String, Value>> {
    json_node(json, name)?.get("descendants")?.as_object()?.values().find_map(Value::as_object)
}

fn export(doc: &Document) -> Value {
    serde_json::from_str(&export_pen_json(doc).expect("export_pen_json")).expect("the export is json")
}

#[test]
fn an_instance_with_its_own_arc_writes_it_and_one_that_follows_the_main_writes_none() {
    let json = export(&rings(OWN_ARC));
    let own = ring_override(&json, "Own").expect("the instance with its own arc writes a descendant override");
    let number = |k: &str| own.get(k).and_then(Value::as_f64).unwrap_or_else(|| panic!("no {k} in {own:?}"));
    assert!((number("startAngle") - (-1.5f64).to_degrees()).abs() < 2e-3, "start in degrees: {own:?}");
    assert!((number("sweepAngle") - 2.5f64.to_degrees()).abs() < 2e-3, "sweep in degrees: {own:?}");
    assert!((number("innerRadius") - 0.7).abs() < 1e-6, "inner radius: {own:?}");
    let follows = ring_override(&json, "Follows");
    assert!(
        follows.is_none_or(|o| !["startAngle", "sweepAngle", "innerRadius"].iter().any(|k| o.contains_key(*k))),
        "an instance without its own arc writes none: {follows:?}"
    );
}

#[test]
fn an_own_arc_that_matches_the_defaults_is_still_written_in_full() {
    // a full disc differs from the main's ring: no key may be left out, or the reopened ring would follow the main again
    let json = export(&rings(ArcData::default()));
    let own = ring_override(&json, "Own").expect("the instance writes a descendant override");
    for (key, want) in [("startAngle", 0.0), ("sweepAngle", 360.0), ("innerRadius", 0.0)] {
        assert_eq!(own.get(key).and_then(Value::as_f64), Some(want), "{key} in {own:?}");
    }
}

#[test]
fn a_reopened_instance_keeps_its_own_arc_as_its_own_and_the_follower_follows_the_main() {
    for own in [OWN_ARC, ArcData::default()] {
        let doc = rings(own);
        let mut back = import_pen_json(&export_pen_json(&doc).expect("export_pen_json")).expect("import_pen_json");
        let (own_ring, follows_ring) = (ring_layer(&back, "Own"), ring_layer(&back, "Follows"));
        assert_arc_close(arc_of(&back, own_ring), own, "the own arc survives the reopen");
        assert!(back.get(own_ring).unwrap().component.overrides.contains(&OverrideField::Arc), "it is recorded as the instance's own");
        assert_arc_close(arc_of(&back, follows_ring), MAIN_ARC, "the follower draws the main's arc");
        assert!(!back.get(follows_ring).unwrap().component.overrides.contains(&OverrideField::Arc), "the follower records no arc");

        // a later edit of the main's arc reaches the follower and leaves the own arc alone
        let main = find(&back, "Ring").id;
        let main_ring = ring_layer(&back, "Ring");
        let changed = ArcData { end: 3.0, ..MAIN_ARC };
        set_arc(&mut back, main_ring, changed);
        sync_instances(&mut back, main).apply(&mut back).unwrap();
        assert_arc_close(arc_of(&back, own_ring), own, "the own arc survives a main edit");
        assert_arc_close(arc_of(&back, follows_ring), changed, "the follower takes the main's new arc");
    }
}

// ---------- the other exports that name an instance's overrides

#[test]
fn the_figma_rest_export_names_the_arc_as_an_override_and_writes_the_arc_of_the_ring() {
    let json = designos_fig::export_rest_json(&rings(OWN_ARC));
    let own = json_node(&json["document"], "Own").expect("the Own instance");
    let overridden: Vec<&str> = own["overrides"]
        .as_array()
        .expect("overrides")
        .iter()
        .flat_map(|o| o["overriddenFields"].as_array().into_iter().flatten())
        .filter_map(Value::as_str)
        .collect();
    assert!(overridden.contains(&"arcData"), "{overridden:?}");
    let arc = &own["children"][0]["arcData"];
    assert!((arc["startingAngle"].as_f64().unwrap() - f64::from(OWN_ARC.start)).abs() < 1e-5, "{arc}");
    assert!((arc["endingAngle"].as_f64().unwrap() - f64::from(OWN_ARC.end)).abs() < 1e-5, "{arc}");
    let follows = json_node(&json["document"], "Follows").expect("the Follows instance");
    assert!(follows.get("overrides").is_none_or(|o| !o.to_string().contains("arcData")), "the follower names no arc override");
}

/// CSS, SVG and PNG export draw the resolved layers, so an instance's own arc reaches them through the ring layer
/// itself; one render shows it.
#[test]
fn a_render_of_an_instance_draws_its_own_arc_and_keeps_it_over_a_main_edit() {
    let mut doc = rings(OWN_ARC);
    let pixels = |doc: &Document, name: &str| designos_render::render_node(doc, find(doc, name).id, 1.0).expect("render").data().to_vec();
    let (own, follows) = (pixels(&doc, "Own"), pixels(&doc, "Follows"));
    assert_ne!(own, follows, "the two rings draw different arcs");
    let main = find(&doc, "Ring").id;
    let main_ring = ring_layer(&doc, "Ring");
    set_arc(&mut doc, main_ring, ArcData { end: 3.0, ..MAIN_ARC });
    sync_instances(&mut doc, main).apply(&mut doc).unwrap();
    assert_eq!(pixels(&doc, "Own"), own, "the own arc still draws the same");
    assert_ne!(pixels(&doc, "Follows"), follows, "the follower draws the main's new arc");
}
