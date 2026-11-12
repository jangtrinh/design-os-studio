//! An instance keeps the arc it draws itself (a progress ring showing a different sweep than its main) across a
//! `.fig` save and reopen, and still keeps it when its main's arc is edited and synced afterwards.
//!
//! The writer puts the arc into the entry of the ring layer as `arcData`; the importer has to turn that key back
//! into the arc and the override flag, or the reopened ring follows its main again the next time the main is synced.
use designos_doc::*;
use designos_fig::{export_fig, import_fig};

const MAIN_ARC: ArcData = ArcData { start: 0.0, end: std::f32::consts::TAU, inner_radius: 0.8 };
const OWN_ARC: ArcData = ArcData { start: -1.5, end: 1.0, inner_radius: 0.7 };

/// The baked outline an imported main ring carries. Baked geometry outranks the arc when drawing, so an instance that
/// holds the main's outline next to its own arc would draw the main's arc.
fn baked_ring() -> Vec<Path> {
    vec![Path {
        cmds: vec![PathCmd::MoveTo(Vec2::new(0.0, 0.0)), PathCmd::LineTo(Vec2::new(48.0, 0.0)), PathCmd::LineTo(Vec2::new(48.0, 48.0)), PathCmd::Close],
        ..Default::default()
    }]
}

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

fn arc_of(doc: &Document, name: &str) -> ArcData {
    match &find(doc, name).kind {
        NodeKind::Ellipse(a) => *a,
        other => panic!("{name} is not an ellipse: {other:?}"),
    }
}

fn set_arc(doc: &mut Document, name: &str, to: ArcData) {
    let before = find(doc, name).clone();
    let mut n = before.clone();
    n.kind = NodeKind::Ellipse(to);
    if n.component.source.is_some() {
        // the edited layer of an instance is redrawn from its arc; the main keeps its baked outline
        n.fill_geometry = None;
        n.component.overrides.extend(record_override(&before, &n));
    }
    Command::Update { node: Box::new(n) }.apply(doc).unwrap();
}

/// A ring main and two instances of it: `Own` draws its own arc, `Follows` draws the main's.
fn rings() -> Document {
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
    e.fill_geometry = Some(baked_ring());
    doc.insert(main, e).unwrap();
    create_component(&mut doc, main).apply(&mut doc).unwrap();
    for (name, x) in [("Own", 100.0), ("Follows", 200.0)] {
        create_instance(&mut doc, main, page, Vec2::new(x, 0.0)).apply(&mut doc).unwrap();
        let inst = *doc.get(page).unwrap().children.last().unwrap();
        doc.get_mut(inst).unwrap().name = name.into();
        let arc = doc.get(inst).unwrap().children[0];
        doc.get_mut(arc).unwrap().name = format!("{name}Arc");
    }
    set_arc(&mut doc, "OwnArc", OWN_ARC);
    doc
}

fn reopened() -> Document {
    let doc = rings();
    assert_eq!(arc_of(&doc, "OwnArc"), OWN_ARC, "fixture: the instance draws its own arc");
    assert_eq!(arc_of(&doc, "FollowsArc"), MAIN_ARC, "fixture: the other instance draws the main's");
    import_fig(&export_fig(&doc).expect("export_fig")).expect("import_fig")
}

#[test]
fn an_instance_with_its_own_arc_keeps_it_through_a_fig_reopen() {
    let doc = reopened();
    assert_eq!(arc_of(&doc, "MainArc"), MAIN_ARC, "the main keeps its arc");
    let own = ring_layer(&doc, "Own");
    let follows = ring_layer(&doc, "Follows");
    assert_eq!(arc_of_id(&doc, own), OWN_ARC, "the instance keeps its own arc");
    assert_ne!(doc.get(own).unwrap().fill_geometry, find(&doc, "MainArc").fill_geometry, "the instance does not carry the main's baked outline");
    assert_eq!(arc_of_id(&doc, follows), MAIN_ARC, "the other instance still draws the main's arc");
}

#[test]
fn a_reopened_instance_keeps_its_own_arc_over_a_later_main_edit_and_the_follower_follows() {
    let mut doc = reopened();
    let main = find(&doc, "Ring").id;
    for change in [ArcData { start: 0.5, ..MAIN_ARC }, ArcData { end: 3.0, ..MAIN_ARC }, ArcData { inner_radius: 0.4, ..MAIN_ARC }] {
        set_arc(&mut doc, "MainArc", change);
        sync_instances(&mut doc, main).apply(&mut doc).unwrap();
        assert_eq!(arc_of_id(&doc, ring_layer(&doc, "Own")), OWN_ARC, "the own arc survives a main edit to {change:?}");
        assert_ne!(
            doc.get(ring_layer(&doc, "Own")).unwrap().fill_geometry,
            find(&doc, "MainArc").fill_geometry,
            "the own ring is not drawn from the main's baked outline after a main edit to {change:?}"
        );
        assert_eq!(arc_of_id(&doc, ring_layer(&doc, "Follows")), change, "the follower takes the main's new arc");
    }
}

fn ring_layer(doc: &Document, instance: &str) -> NodeId {
    let inst = find(doc, instance);
    *inst.children.iter().find(|c| matches!(doc.get(**c).map(|n| &n.kind), Some(NodeKind::Ellipse(_)))).expect("ring layer")
}

fn arc_of_id(doc: &Document, id: NodeId) -> ArcData {
    match &doc.get(id).unwrap().kind {
        NodeKind::Ellipse(a) => *a,
        other => panic!("not an ellipse: {other:?}"),
    }
}
