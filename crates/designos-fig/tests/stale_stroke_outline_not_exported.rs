//! A layer stores the outline its stroke was baked into, together with the stroke it was baked for. Once the
//! weight changes the outline no longer describes the layer, and a file that carried it would draw the old
//! stroke again when it is opened (the readers take a stored outline as the layer's stroke). Both writers, the
//! `.fig` file and the REST JSON, therefore write `strokeGeometry` only while it still matches the layer.

use designos_doc::*;
use designos_fig::{export_fig, export_rest_json, import_fig, import_rest_json};
use serde_json::Value;

const SIZE: f32 = 100.0;
const BAKED: f32 = 4.0;

fn ring(outer: f32, inset: f32) -> Path {
    let square = |lo: f32, hi: f32| {
        [
            PathCmd::MoveTo(Vec2::new(lo, lo)),
            PathCmd::LineTo(Vec2::new(hi, lo)),
            PathCmd::LineTo(Vec2::new(hi, hi)),
            PathCmd::LineTo(Vec2::new(lo, hi)),
            PathCmd::Close,
        ]
    };
    let mut cmds = square(0.0, outer).to_vec();
    cmds.extend(square(inset, outer - inset));
    Path { cmds, fill_rule: FillRule::EvenOdd }
}

/// A stroke-only rectangle named "Baked" that stores the outline of its 4px inside stroke, and a twin named
/// "Control" that stores none. Both are read back by name after a round trip.
fn document() -> Document {
    let mut d = Document::new("stale-outline");
    let page = d.add_page("P");
    for (name, baked) in [("Baked", true), ("Control", false)] {
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, SIZE, SIZE));
        n.name = name.into();
        n.fills.clear();
        n.stroke.paints = vec![Paint::solid(Color::rgb(0, 0, 255))];
        n.stroke.weight = BAKED;
        n.stroke.align = StrokeAlign::Inside;
        if baked {
            n.stroke_geometry = Some(vec![ring(SIZE, BAKED)]);
        }
        d.insert(page, n).unwrap();
    }
    d
}

fn id_of(d: &Document, name: &str) -> NodeId {
    d.nodes.values().find(|n| n.name == name).map(|n| n.id).unwrap_or_else(|| panic!("no node {name}"))
}

fn pixels(d: &Document, name: &str) -> Vec<u8> {
    designos_render::render_node(d, id_of(d, name), 1.0).expect("render").data().to_vec()
}

/// What an MCP `update_node`, the command palette or a script does: set the weight on the node directly.
fn edit_weight(d: &mut Document, weight: f32) {
    for name in ["Baked", "Control"] {
        let id = id_of(d, name);
        d.get_mut(id).unwrap().stroke.weight = weight;
    }
}

fn find<'a>(v: &'a Value, name: &str) -> Option<&'a Value> {
    let v = v.get("document").unwrap_or(v);
    find_in(v, name)
}

fn find_in<'a>(v: &'a Value, name: &str) -> Option<&'a Value> {
    if v.get("name").and_then(Value::as_str) == Some(name) {
        return Some(v);
    }
    v.get("children")?.as_array()?.iter().find_map(|c| find_in(c, name))
}

#[test]
fn an_unedited_outline_is_still_written_and_read_back() {
    let fig = import_fig(&export_fig(&document()).unwrap()).unwrap();
    let n = fig.get(id_of(&fig, "Baked")).unwrap();
    assert!(n.live_stroke_outline().is_some(), ".fig keeps a matching outline");
    assert!(fig.get(id_of(&fig, "Control")).unwrap().stroke_geometry.is_none());

    let rest_json = export_rest_json(&document());
    assert!(find(&rest_json, "Baked").unwrap().get("strokeGeometry").is_some(), "REST keeps a matching outline");
    assert!(find(&rest_json, "Control").unwrap().get("strokeGeometry").is_none());
}

#[test]
fn a_weight_edit_is_not_undone_by_saving_the_file_and_opening_it_again() {
    let mut d = document();
    edit_weight(&mut d, 20.0);
    assert!(d.get(id_of(&d, "Baked")).unwrap().stroke_geometry.is_some(), "the stale outline is still on the node");

    let reopened = import_fig(&export_fig(&d).unwrap()).unwrap();
    let baked = reopened.get(id_of(&reopened, "Baked")).unwrap();
    assert_eq!(baked.stroke.weight, 20.0);
    assert!(baked.stroke_geometry.is_none(), ".fig must not carry the outline baked at 4px");
    assert!(pixels(&reopened, "Baked") == pixels(&reopened, "Control"), "the reopened layer draws the live 20px stroke");
}

#[test]
fn a_weight_edit_is_not_undone_by_a_rest_json_round_trip() {
    let mut d = document();
    edit_weight(&mut d, 20.0);

    let json = export_rest_json(&d);
    assert!(find(&json, "Baked").unwrap().get("strokeGeometry").is_none(), "REST must not carry the outline baked at 4px");
    let reopened = import_rest_json(&json.to_string()).unwrap();
    assert!(pixels(&reopened, "Baked") == pixels(&reopened, "Control"), "the reopened layer draws the live 20px stroke");
}

#[test]
fn setting_the_weight_back_to_the_baked_value_writes_the_outline_again() {
    let mut d = document();
    edit_weight(&mut d, 20.0);
    edit_weight(&mut d, BAKED);
    let reopened = import_fig(&export_fig(&d).unwrap()).unwrap();
    assert!(reopened.get(id_of(&reopened, "Baked")).unwrap().live_stroke_outline().is_some());
    assert!(find(&export_rest_json(&d), "Baked").unwrap().get("strokeGeometry").is_some());
}

/// The second layer named `name` in `doc` that is not the component's own: the copy inside the instance.
fn child_of_instance(d: &Document, name: &str) -> NodeId {
    let inst = d.nodes.values().find(|n| matches!(n.kind, NodeKind::Instance { .. })).expect("an instance").id;
    d.get(inst).unwrap().children.iter().copied().find(|&c| d.get(c).is_some_and(|n| n.name == name)).unwrap_or_else(|| panic!("no {name} in the instance"))
}

#[test]
fn an_instance_child_weight_edit_is_not_undone_by_saving_the_file_and_opening_it_again() {
    let mut d = Document::new("stale-outline-instance");
    let page = d.add_page("P");
    let comp = d.alloc_id();
    let mut c = Node::new(comp, NodeKind::Component(FrameProps::default()));
    c.name = "Icon".into();
    c.size = Vec2::new(SIZE, SIZE);
    d.insert(page, c).unwrap();
    let id = d.alloc_id();
    let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, SIZE, SIZE));
    n.name = "Glyph".into();
    n.fills.clear();
    n.stroke.paints = vec![Paint::solid(Color::rgb(0, 0, 255))];
    n.stroke.weight = BAKED;
    n.stroke.align = StrokeAlign::Inside;
    n.stroke_geometry = Some(vec![ring(SIZE, BAKED)]);
    d.insert(comp, n).unwrap();
    create_instance(&mut d, comp, page, Vec2::new(200.0, 0.0)).apply(&mut d).unwrap();
    let copy = child_of_instance(&d, "Glyph");
    assert!(d.get(copy).unwrap().live_stroke_outline().is_some(), "fixture: the copy starts with the outline baked at 4px");

    // What a weight edit on the copy in the stroke panel leaves behind: the weight, and the stroke marked as overridden.
    let n = d.get_mut(copy).unwrap();
    n.stroke.weight = 20.0;
    n.component.overrides.insert(OverrideField::Stroke);

    let reopened = import_fig(&export_fig(&d).unwrap()).unwrap();
    let glyph = reopened.get(child_of_instance(&reopened, "Glyph")).unwrap();
    assert_eq!(glyph.stroke.weight, 20.0);
    assert!(glyph.live_stroke_outline().is_none(), "the override must not carry the outline baked at 4px");
}

/// A vector with no vector network whose only geometry is its stroke outline: what a Figma REST file gives for a
/// stroke-only icon. The shape lives in `paths`, equal to the stored outline.
fn outline_only_vector() -> Document {
    let mut d = Document::new("outline-only");
    let page = d.add_page("P");
    let id = d.alloc_id();
    let band = vec![ring(SIZE, BAKED)];
    let mut n = Node::new(id, NodeKind::Vector { paths: band.clone(), network: None });
    n.name = "Icon".into();
    n.size = Vec2::new(SIZE, SIZE);
    n.stroke.paints = vec![Paint::solid(Color::rgb(0, 0, 255))];
    n.stroke.weight = BAKED;
    n.stroke.align = StrokeAlign::Inside;
    n.stroke_geometry = Some(band);
    d.insert(page, n).unwrap();
    d
}

fn vector_paths(d: &Document, name: &str) -> usize {
    match &d.get(id_of(d, name)).unwrap().kind {
        NodeKind::Vector { paths, .. } => paths.len(),
        other => panic!("not a vector: {other:?}"),
    }
}

fn has_ink(d: &Document, name: &str) -> bool {
    pixels(d, name).chunks(4).any(|p| p[3] != 0)
}

#[test]
fn an_outline_only_vector_keeps_its_shape_through_a_weight_edit_and_a_fig_save() {
    let reopened = import_fig(&export_fig(&outline_only_vector()).unwrap()).unwrap();
    assert!(vector_paths(&reopened, "Icon") > 0, "unedited: the shape is written and read back");

    let mut d = outline_only_vector();
    let id = id_of(&d, "Icon");
    d.get_mut(id).unwrap().stroke.weight = 20.0;
    let before = pixels(&d, "Icon");
    assert!(has_ink(&d, "Icon"), "the edited layer draws its live stroke");

    let reopened = import_fig(&export_fig(&d).unwrap()).unwrap();
    assert!(vector_paths(&reopened, "Icon") > 0, "the shape survives: the reopened vector is not empty");
    assert_eq!(reopened.get(id_of(&reopened, "Icon")).unwrap().stroke.weight, 20.0);
    assert!(reopened.get(id_of(&reopened, "Icon")).unwrap().live_stroke_outline().is_none(), "and it carries no outline baked at 4px");
    assert!(has_ink(&reopened, "Icon"), "the reopened layer is not blank");
    assert!(pixels(&reopened, "Icon") == before, "the reopened layer draws what it drew before saving");
}
