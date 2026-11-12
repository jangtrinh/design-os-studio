//! An instance that draws its own arc (a progress ring showing a different sweep than its main) keeps it when the
//! main's arc changes, and follows the main again once it is reset.

use crate::{
    create_component, create_instance, record_override, reset_instance, reset_overrides, sync_instances, ArcData, Color, Command, Document, Node, NodeId,
    NodeKind, OverrideField, Paint, Path, PathCmd, Rect, Vec2,
};

const MAIN_ARC: ArcData = ArcData { start: 0.0, end: std::f32::consts::TAU, inner_radius: 0.8 };
const OWN_ARC: ArcData = ArcData { start: -1.5, end: 1.0, inner_radius: 0.7 };

struct Ring {
    d: Document,
    comp: NodeId,
    main_arc: NodeId,
    inst_arc: NodeId,
    inst: NodeId,
}

fn apply(d: &mut Document, c: Command) {
    c.apply(d).unwrap();
}

/// The baked outline of the main's ring, as an imported file carries it. A sync that copies it onto an instance with a
/// different arc would draw the main's arc, because baked geometry outranks the arc.
fn baked_ring() -> Vec<Path> {
    vec![Path {
        cmds: vec![PathCmd::MoveTo(Vec2::new(0.0, 0.0)), PathCmd::LineTo(Vec2::new(48.0, 0.0)), PathCmd::LineTo(Vec2::new(48.0, 48.0)), PathCmd::Close],
        ..Default::default()
    }]
}

fn ring() -> Ring {
    let mut d = Document::new("ring");
    let page = d.add_page("P");
    let id = d.alloc_id();
    let comp = d.insert(page, Node::frame(id, Rect::new(0.0, 0.0, 48.0, 48.0))).unwrap();
    let id = d.alloc_id();
    let mut e = Node::ellipse(id, Rect::new(0.0, 0.0, 48.0, 48.0));
    e.name = "arc".into();
    e.kind = NodeKind::Ellipse(MAIN_ARC);
    e.fills = vec![Paint::solid(Color::rgb(255, 0, 0))];
    e.fill_geometry = Some(baked_ring());
    let main_arc = d.insert(comp, e).unwrap();
    let c = create_component(&mut d, comp);
    apply(&mut d, c);
    let c = create_instance(&mut d, comp, page, Vec2::new(100.0, 0.0));
    apply(&mut d, c);
    let inst = *d.get(page).unwrap().children.last().unwrap();
    let inst_arc = d.get(inst).unwrap().children[0];
    Ring { d, comp, main_arc, inst_arc, inst }
}

fn arc_of(d: &Document, id: NodeId) -> ArcData {
    match &d.get(id).unwrap().kind {
        NodeKind::Ellipse(a) => *a,
        other => panic!("not an ellipse: {other:?}"),
    }
}

/// Redraws the arc of `id` the way an edit of the layer does, recording what it changed on a layer of an instance.
fn edit_arc(d: &mut Document, id: NodeId, to: ArcData) {
    let before = d.get(id).unwrap().clone();
    let mut n = before.clone();
    n.kind = NodeKind::Ellipse(to);
    n.fill_geometry = None;
    if n.component.source.is_some() {
        n.component.overrides.extend(record_override(&before, &n));
    }
    apply(d, Command::Update { node: Box::new(n) });
}

fn resync(r: &mut Ring) {
    let c = sync_instances(&mut r.d, r.comp);
    apply(&mut r.d, c);
}

const CHANGES: [(&str, ArcData); 3] =
    [("start", ArcData { start: 0.5, ..MAIN_ARC }), ("sweep", ArcData { end: 3.0, ..MAIN_ARC }), ("inner radius", ArcData { inner_radius: 0.4, ..MAIN_ARC })];

#[test]
fn an_instance_keeps_its_own_arc_when_the_main_arc_changes() {
    for (what, change) in CHANGES {
        let mut r = ring();
        edit_arc(&mut r.d, r.inst_arc, OWN_ARC);
        edit_arc(&mut r.d, r.main_arc, change);
        resync(&mut r);
        assert_eq!(arc_of(&r.d, r.main_arc), change, "the main keeps its edit ({what})");
        assert_eq!(arc_of(&r.d, r.inst_arc), OWN_ARC, "the instance's own arc survives a main {what} change");
    }
}

#[test]
fn an_instance_with_its_own_arc_draws_it_not_the_baked_ring_of_the_main() {
    let mut r = ring();
    edit_arc(&mut r.d, r.inst_arc, OWN_ARC);
    edit_arc(&mut r.d, r.main_arc, ArcData { end: 3.0, ..MAIN_ARC });
    // the main still carries its baked ring
    let mut m = r.d.get(r.main_arc).unwrap().clone();
    m.fill_geometry = Some(baked_ring());
    apply(&mut r.d, Command::Update { node: Box::new(m) });
    resync(&mut r);
    assert!(r.d.get(r.inst_arc).unwrap().fill_geometry.is_none(), "the main's baked ring would outdraw the instance's own arc");
}

#[test]
fn an_instance_without_its_own_arc_follows_the_main_arc() {
    for (what, change) in CHANGES {
        let mut r = ring();
        edit_arc(&mut r.d, r.main_arc, change);
        resync(&mut r);
        assert_eq!(arc_of(&r.d, r.inst_arc), change, "an instance with no arc of its own follows a main {what} change");
    }
}

#[test]
fn resetting_an_instance_returns_its_arc_to_the_main() {
    let mut r = ring();
    let changed = ArcData { end: 3.0, ..MAIN_ARC };
    edit_arc(&mut r.d, r.inst_arc, OWN_ARC);
    edit_arc(&mut r.d, r.main_arc, changed);
    resync(&mut r);
    assert_eq!(arc_of(&r.d, r.inst_arc), OWN_ARC, "own arc kept before the reset");
    let c = reset_instance(&mut r.d, r.inst);
    apply(&mut r.d, c);
    assert_eq!(arc_of(&r.d, r.inst_arc), changed, "reset all changes drops the arc override");
    edit_arc(&mut r.d, r.main_arc, MAIN_ARC);
    resync(&mut r);
    assert_eq!(arc_of(&r.d, r.inst_arc), MAIN_ARC, "and the instance follows the main from then on");
}

#[test]
fn resetting_the_overrides_of_a_layer_returns_its_arc_to_the_main() {
    let mut r = ring();
    let changed = ArcData { inner_radius: 0.4, ..MAIN_ARC };
    edit_arc(&mut r.d, r.inst_arc, OWN_ARC);
    edit_arc(&mut r.d, r.main_arc, changed);
    resync(&mut r);
    assert_eq!(arc_of(&r.d, r.inst_arc), OWN_ARC, "own arc kept before the reset");
    let fields: Vec<OverrideField> = r.d.get(r.inst_arc).unwrap().component.overrides.iter().copied().collect();
    assert!(!fields.is_empty(), "the arc edit is recorded as an override");
    let c = reset_overrides(&mut r.d, r.inst_arc, &fields);
    apply(&mut r.d, c);
    assert_eq!(arc_of(&r.d, r.inst_arc), changed, "resetting the recorded override follows the main again");
}

#[test]
fn the_arc_override_survives_the_native_round_trip_and_old_files_still_load() {
    let mut r = ring();
    edit_arc(&mut r.d, r.inst_arc, OWN_ARC);
    let json = r.d.to_json().unwrap();
    let mut back = Document::from_json(&json).unwrap();
    assert_eq!(arc_of(&back, r.inst_arc), OWN_ARC);
    edit_arc(&mut back, r.main_arc, ArcData { end: 3.0, ..MAIN_ARC });
    let c = sync_instances(&mut back, r.comp);
    apply(&mut back, c);
    assert_eq!(arc_of(&back, r.inst_arc), OWN_ARC, "the override still wins after the file is reloaded");

    // a file written before the override existed has no such entry
    let old = Document::from_json(&ring().d.to_json().unwrap()).unwrap();
    assert_eq!(arc_of(&old, r.inst_arc), MAIN_ARC);
    assert!(old.get(r.inst_arc).unwrap().component.overrides.is_empty());
}

/// The outline a resized ring of an instance holds: drawn for its own size, not the main's.
fn own_sized_outline() -> Vec<Path> {
    vec![Path {
        cmds: vec![PathCmd::MoveTo(Vec2::new(0.0, 0.0)), PathCmd::LineTo(Vec2::new(30.0, 0.0)), PathCmd::LineTo(Vec2::new(30.0, 30.0)), PathCmd::Close],
        ..Default::default()
    }]
}

#[test]
fn a_resized_ring_that_follows_the_main_arc_keeps_its_outline_until_the_main_arc_changes() {
    let mut r = ring();
    let mut n = r.d.get(r.inst_arc).unwrap().clone();
    n.size = Vec2::new(30.0, 30.0);
    n.fill_geometry = Some(own_sized_outline());
    n.component.overrides.insert(OverrideField::Size);
    apply(&mut r.d, Command::Update { node: Box::new(n) });

    // a change of the main that is not its arc leaves the ring as it is, outline included
    let mut m = r.d.get(r.main_arc).unwrap().clone();
    m.opacity = 0.5;
    apply(&mut r.d, Command::Update { node: Box::new(m) });
    resync(&mut r);
    assert_eq!(r.d.get(r.inst_arc).unwrap().fill_geometry, Some(own_sized_outline()), "its outline still fits its arc and size");

    // a change of the main's arc: the ring follows it, and the outline of the old arc is dropped
    let changed = ArcData { end: 3.0, ..MAIN_ARC };
    edit_arc(&mut r.d, r.main_arc, changed);
    resync(&mut r);
    let n = r.d.get(r.inst_arc).unwrap();
    assert_eq!(arc_of(&r.d, r.inst_arc), changed, "the ring follows the main's new arc");
    assert!(n.fill_geometry.is_none() && n.stroke_geometry.is_none(), "the outline of the old arc would be drawn instead of the new arc");
    assert_eq!(n.size, Vec2::new(30.0, 30.0), "at its own size");
}
