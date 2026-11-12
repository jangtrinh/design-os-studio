//! A vector resized inside an instance keeps its resized shape across a `.fig` save and reopen, also after the main
//! component is synced to the instance again.
//!
//! The file records the resize as a size override on the vector inside the instance. The reopened instance has to
//! give the vector's outline that size again, not the outline of the main component.

use designos_doc::*;
use designos_fig::{export_fig, import_fig};

const TOLERANCE: f32 = 0.5;

fn box_outline(w: f32, h: f32) -> Vec<Path> {
    vec![Path {
        cmds: vec![
            PathCmd::MoveTo(Vec2::new(0.0, 0.0)),
            PathCmd::LineTo(Vec2::new(w, 0.0)),
            PathCmd::LineTo(Vec2::new(w, h)),
            PathCmd::LineTo(Vec2::new(0.0, h)),
            PathCmd::Close,
        ],
        ..Default::default()
    }]
}

fn path_bounds(paths: &[Path]) -> (f32, f32, f32, f32) {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in paths {
        for c in &p.cmds {
            let pts: Vec<Vec2> = match c {
                PathCmd::MoveTo(p) | PathCmd::LineTo(p) => vec![*p],
                PathCmd::QuadTo(a, b) => vec![*a, *b],
                PathCmd::CubicTo(a, b, e) => vec![*a, *b, *e],
                PathCmd::Close => vec![],
            };
            for q in pts {
                x0 = x0.min(q.x);
                y0 = y0.min(q.y);
                x1 = x1.max(q.x);
                y1 = y1.max(q.y);
            }
        }
    }
    (x0, y0, x1, y1)
}

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

fn main_of(doc: &Document, name: &str) -> NodeId {
    doc.nodes.values().find(|n| n.name == name && matches!(n.kind, NodeKind::Component(_))).unwrap_or_else(|| panic!("no component {name}")).id
}

fn glyph_in_instance(doc: &Document, instance: &str) -> NodeId {
    let inst = find(doc, instance);
    assert!(matches!(inst.kind, NodeKind::Instance { .. }), "{instance} is an instance");
    *inst.children.iter().find(|&&c| doc.get(c).is_some_and(|n| n.name == "Glyph")).unwrap_or_else(|| panic!("no Glyph in {instance}"))
}

/// Width and height of the vector's outline, and the size of its node.
fn measured(doc: &Document, glyph: NodeId) -> ((f32, f32), (f32, f32)) {
    let n = doc.get(glyph).expect("glyph");
    let NodeKind::Vector { paths, .. } = &n.kind else { panic!("Glyph is not a vector: {:?}", n.kind) };
    let (x0, y0, x1, y1) = path_bounds(paths);
    ((x1 - x0, y1 - y0), (n.size.x, n.size.y))
}

fn assert_shape(doc: &Document, glyph: NodeId, want: (f32, f32), what: &str) {
    let (path, size) = measured(doc, glyph);
    assert!((path.0 - want.0).abs() <= TOLERANCE && (path.1 - want.1).abs() <= TOLERANCE, "{what}: outline is {path:?}, want {want:?}");
    assert!((size.0 - want.0).abs() <= 1e-3 && (size.1 - want.1).abs() <= 1e-3, "{what}: node size is {size:?}, want {want:?}");
}

/// A main component `Mark` (100 x 100) holding a vector `Glyph`, and two instances `Resized` and `Plain` of it.
/// `Resized` has its Glyph set to `target` the way the editor does it (constraint-aware geometry plus the recorded override).
fn fixture(target: Option<(f32, f32)>) -> Document {
    fixture_with(target, box_glyph)
}

/// The plain `Glyph`: a 100 x 100 square with its own fill bake and no network.
fn box_glyph() -> Node {
    let mut glyph = Node::new(NodeId(0), NodeKind::Vector { paths: box_outline(100.0, 100.0), network: None });
    glyph.name = "Glyph".into();
    glyph.size = Vec2::new(100.0, 100.0);
    glyph.fills = vec![Paint::solid(Color::rgb(255, 0, 0))];
    glyph.fill_geometry = Some(box_outline(100.0, 100.0));
    glyph
}

/// [`fixture`] where the main's `Glyph` is the node `glyph()` builds (its id is replaced on insert).
fn fixture_with(target: Option<(f32, f32)>, glyph: fn() -> Node) -> Document {
    let mut doc = Document::new("InstanceVectorResize");
    let page = doc.add_page("Page 1");

    let comp_id = doc.alloc_id();
    let mut frame = Node::frame(comp_id, Rect::new(0.0, 0.0, 100.0, 100.0));
    frame.name = "Mark".into();
    let comp = doc.insert(page, frame).expect("insert frame");
    let mut glyph = glyph();
    glyph.id = doc.alloc_id();
    doc.insert(comp, glyph).expect("insert glyph");
    create_component(&mut doc, comp).apply(&mut doc).expect("component");

    for (name, x) in [("Resized", 200.0), ("Plain", 500.0)] {
        create_instance(&mut doc, comp, page, Vec2::new(x, 0.0)).apply(&mut doc).expect("instance");
        let inst = *doc.get(page).expect("page").children.last().expect("instance");
        doc.get_mut(inst).expect("instance").name = name.into();
    }

    if let Some((w, h)) = target {
        let glyph = glyph_in_instance(&doc, "Resized");
        let mut n = doc.get(glyph).expect("glyph").clone();
        let before = n.clone();
        let tf = n.transform;
        apply_constrained_geometry(&mut n, tf, Vec2::new(w, h));
        n.component.overrides = record_override(&before, &n);
        Command::Update { node: Box::new(n) }.apply(&mut doc).expect("resize");
        assert_shape(&doc, glyph_in_instance(&doc, "Resized"), (w, h), "fixture: the resize scales the outline in memory");
    }
    doc
}

fn reopen(doc: &Document) -> Document {
    import_fig(&export_fig(doc).expect("export")).expect("reopen")
}

fn resize_round_trip(target: (f32, f32)) {
    let mut doc = reopen(&fixture(Some(target)));
    assert_shape(&doc, glyph_in_instance(&doc, "Resized"), target, "after the reopen");
    assert_shape(&doc, glyph_in_instance(&doc, "Plain"), (100.0, 100.0), "the untouched instance after the reopen");

    // The reopened instance must not snap back when its main component is synced again.
    let main = main_of(&doc, "Mark");
    sync_instances(&mut doc, main).apply(&mut doc).expect("sync");
    assert_shape(&doc, glyph_in_instance(&doc, "Resized"), target, "after a sync of the reopened document");
    assert_shape(&doc, glyph_in_instance(&doc, "Plain"), (100.0, 100.0), "the untouched instance after a sync");

    // And a second save and reopen keeps it.
    let again = reopen(&doc);
    assert_shape(&again, glyph_in_instance(&again, "Resized"), target, "after a second save and reopen");
}

#[test]
fn a_vector_resized_in_width_inside_an_instance_keeps_its_width_across_save_and_sync() {
    resize_round_trip((250.0, 100.0));
}

#[test]
fn a_vector_resized_in_height_inside_an_instance_keeps_its_height_across_save_and_sync() {
    resize_round_trip((100.0, 40.0));
}

#[test]
fn a_vector_resized_on_both_axes_by_different_factors_keeps_its_shape_across_save_and_sync() {
    resize_round_trip((250.0, 40.0));
}

/// Runs `check` on the reopened document, on the reopened document after a sync of its main, and on a second save and
/// reopen of that.
fn after_reopen_sync_and_second_reopen(doc: &Document, check: impl Fn(&Document, &str)) {
    let mut doc = reopen(doc);
    check(&doc, "after the reopen");
    let main = main_of(&doc, "Mark");
    sync_instances(&mut doc, main).apply(&mut doc).expect("sync");
    check(&doc, "after a sync of the reopened document");
    check(&reopen(&doc), "after a second save and reopen");
}

fn extent_of(points: impl IntoIterator<Item = Vec2>) -> (f32, f32) {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in points {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    (x1 - x0, y1 - y0)
}

fn assert_extent(got: (f32, f32), want: (f32, f32), what: &str) {
    assert!((got.0 - want.0).abs() <= TOLERANCE && (got.1 - want.1).abs() <= TOLERANCE, "{what}: is {got:?}, want {want:?}");
}

fn diamond_outline() -> Vec<Path> {
    vec![Path {
        cmds: vec![
            PathCmd::MoveTo(Vec2::new(50.0, 0.0)),
            PathCmd::LineTo(Vec2::new(100.0, 50.0)),
            PathCmd::LineTo(Vec2::new(50.0, 100.0)),
            PathCmd::LineTo(Vec2::new(0.0, 50.0)),
            PathCmd::Close,
        ],
        ..Default::default()
    }]
}

/// A pen-drawn glyph: a vector network of four corners, with the outline and the fill bake kept next to it.
fn network_glyph() -> Node {
    let outline = diamond_outline();
    let network = VectorNetwork::from_paths(&outline);
    assert_eq!(network.vertices.len(), 4, "the fixture network has the four corners");
    let mut glyph = Node::new(NodeId(0), NodeKind::Vector { paths: outline.clone(), network: Some(network) });
    glyph.name = "Glyph".into();
    glyph.size = Vec2::new(100.0, 100.0);
    glyph.fills = vec![Paint::solid(Color::rgb(255, 0, 0))];
    glyph.fill_geometry = Some(outline);
    glyph
}

#[test]
fn a_pen_drawn_vector_resized_on_both_axes_keeps_outline_network_and_fill_bake_across_save_and_sync() {
    let target = (250.0, 40.0);
    let doc = fixture_with(Some(target), network_glyph);
    let in_memory = doc.get(glyph_in_instance(&doc, "Resized")).expect("glyph");
    assert!(matches!(&in_memory.kind, NodeKind::Vector { network: Some(_), .. }), "fixture: the resized vector keeps its network");
    after_reopen_sync_and_second_reopen(&doc, |doc, when| {
        let glyph = glyph_in_instance(doc, "Resized");
        assert_shape(doc, glyph, target, when);
        let n = doc.get(glyph).expect("glyph");
        let NodeKind::Vector { network, .. } = &n.kind else { panic!("{when}: Glyph is {:?}", n.kind) };
        let network = network.as_ref().unwrap_or_else(|| panic!("{when}: the resized vector lost its network"));
        assert_extent(extent_of(network.vertices.iter().map(|v| v.pos)), target, &format!("{when}: network extent"));
        let bake = n.fill_geometry.as_ref().unwrap_or_else(|| panic!("{when}: the resized vector lost its fill bake"));
        let points = bake.iter().flat_map(|p| p.cmds.iter()).filter_map(|c| match c {
            PathCmd::MoveTo(p) | PathCmd::LineTo(p) => Some(*p),
            _ => None,
        });
        assert_extent(extent_of(points), target, &format!("{when}: fill bake extent"));
        assert_shape(doc, glyph_in_instance(doc, "Plain"), (100.0, 100.0), &format!("{when}: untouched instance"));
    });
}

const TILT: f32 = std::f32::consts::PI / 6.0;

fn tilted_glyph() -> Node {
    let mut glyph = box_glyph();
    glyph.transform = Affine::translate(30.0, 10.0) * Affine::rotate(TILT);
    glyph
}

#[test]
fn a_rotated_vector_resized_inside_an_instance_keeps_its_local_size_and_rotation_across_save_and_sync() {
    let target = (250.0, 40.0);
    let doc = fixture_with(Some(target), tilted_glyph);
    after_reopen_sync_and_second_reopen(&doc, |doc, when| {
        let glyph = glyph_in_instance(doc, "Resized");
        assert_shape(doc, glyph, target, when);
        let t = doc.get(glyph).expect("glyph").transform;
        let want = Affine::rotate(TILT);
        for (name, got, want) in [("a", t.a, want.a), ("b", t.b, want.b), ("c", t.c, want.c), ("d", t.d, want.d)] {
            assert!((got - want).abs() <= 1e-4, "{when}: transform {name} is {got}, want {want}");
        }
        assert!((t.rotation_rad() - TILT).abs() <= 1e-4, "{when}: rotation is {}, want {TILT}", t.rotation_rad());
        assert_shape(doc, glyph_in_instance(doc, "Plain"), (100.0, 100.0), &format!("{when}: untouched instance"));
    });
}

fn solid_of(doc: &Document, id: NodeId) -> Color {
    match doc.get(id).and_then(|n| n.fills.first()).map(|p| &p.kind) {
        Some(PaintKind::Solid(c)) => *c,
        other => panic!("expected a solid fill, got {other:?}"),
    }
}

#[test]
fn a_main_fill_edited_after_the_instance_vector_was_resized_reaches_it_and_keeps_its_size_across_save_and_sync() {
    let target = (250.0, 40.0);
    let blue = Color::rgb(0, 0, 255);
    let mut doc = fixture(Some(target));
    let main = main_of(&doc, "Mark");
    let main_glyph = *doc.get(main).expect("main").children.first().expect("main glyph");
    let mut edited = doc.get(main_glyph).expect("main glyph").clone();
    edited.fills = vec![Paint::solid(blue)];
    Command::Update { node: Box::new(edited) }.apply(&mut doc).expect("edit the main");
    sync_instances(&mut doc, main).apply(&mut doc).expect("sync the edit");
    assert_eq!(solid_of(&doc, glyph_in_instance(&doc, "Resized")), blue, "fixture: the instance vector follows the main's new fill");
    assert_shape(&doc, glyph_in_instance(&doc, "Resized"), target, "fixture: the resize survives the sync of the edit");

    after_reopen_sync_and_second_reopen(&doc, |doc, when| {
        assert_shape(doc, glyph_in_instance(doc, "Resized"), target, when);
        assert_eq!(solid_of(doc, glyph_in_instance(doc, "Resized")), blue, "{when}: the resized instance vector has the main's new fill");
        assert_eq!(solid_of(doc, glyph_in_instance(doc, "Plain")), blue, "{when}: the untouched instance vector has it too");
    });
}

#[test]
fn a_vector_inside_an_instance_that_was_not_resized_round_trips_unchanged() {
    let mut doc = reopen(&fixture(None));
    for name in ["Resized", "Plain"] {
        assert_shape(&doc, glyph_in_instance(&doc, name), (100.0, 100.0), name);
    }
    let main = main_of(&doc, "Mark");
    sync_instances(&mut doc, main).apply(&mut doc).expect("sync");
    for name in ["Resized", "Plain"] {
        assert_shape(&doc, glyph_in_instance(&doc, name), (100.0, 100.0), name);
    }
}
