//! A change made on a layer inside a nested component copy stays after the file is saved as `.fig`, opened again, and a
//! main component below it is edited and synced.
//!
//! A page copy of `O` (which holds a copy of `I`, which holds a copy of `A`) has its own fill, opacity or ring arc on the
//! deepest layer. The copy keeps one link per level; a reopened `.fig` has to give every layer the link to the layer one
//! level up, or the first sync takes the layer for a leftover and rebuilds it from the main without the change.
use designos_doc::*;
use designos_fig::{export_fig, import_fig};

const MAIN_ARC: ArcData = ArcData { start: 0.0, end: std::f32::consts::TAU, inner_radius: 0.8 };
const OWN_ARC: ArcData = ArcData { start: -1.5, end: 1.0, inner_radius: 0.7 };
const NEW_MAIN_ARC: ArcData = ArcData { start: 0.5, end: 3.0, inner_radius: 0.4 };

#[derive(Clone, Copy, Debug)]
enum Kind {
    Fill,
    Opacity,
    Arc,
}

#[derive(Clone, Copy, Debug)]
enum Medium {
    Fig,
    /// A `.fig` written by Studio, opened, then saved natively and opened again.
    FigThenNative,
}

#[derive(Clone, Copy, Debug)]
enum Edit {
    /// The main changes something the copy did not override (corner radius).
    Other,
    /// The main changes the very property the copy overrides.
    Same,
}

fn baked_ring() -> Vec<Path> {
    vec![Path {
        cmds: vec![PathCmd::MoveTo(Vec2::new(0.0, 0.0)), PathCmd::LineTo(Vec2::new(48.0, 0.0)), PathCmd::LineTo(Vec2::new(48.0, 48.0)), PathCmd::Close],
        ..Default::default()
    }]
}

fn shape_node(doc: &mut Document, kind: Kind) -> Node {
    let id = doc.alloc_id();
    let mut n = match kind {
        Kind::Fill | Kind::Opacity => Node::rectangle(id, Rect::new(0.0, 0.0, 48.0, 48.0)),
        Kind::Arc => {
            let mut e = Node::ellipse(id, Rect::new(0.0, 0.0, 48.0, 48.0));
            e.kind = NodeKind::Ellipse(MAIN_ARC);
            e.fill_geometry = Some(baked_ring());
            e
        }
    };
    n.name = "Shape".into();
    n.fills = vec![Paint::solid(Color::rgb(255, 0, 0))];
    n
}

fn component_named(doc: &Document, name: &str) -> NodeId {
    doc.nodes.values().find(|n| n.name == name && matches!(n.kind, NodeKind::Component(_))).map(|n| n.id).unwrap_or_else(|| panic!("no component {name}"))
}

fn instance_named(doc: &Document, name: &str) -> NodeId {
    doc.nodes.values().find(|n| n.name == name && matches!(n.kind, NodeKind::Instance { .. })).map(|n| n.id).unwrap_or_else(|| panic!("no instance {name}"))
}

/// The `Shape` copy below `root`, depth first.
fn deep_shape(doc: &Document, root: NodeId) -> Option<NodeId> {
    for c in &doc.get(root)?.children {
        if doc.get(*c)?.name == "Shape" {
            return Some(*c);
        }
        if let Some(found) = deep_shape(doc, *c) {
            return Some(found);
        }
    }
    None
}

fn value_of(kind: Kind, n: &Node) -> String {
    match kind {
        Kind::Fill => format!("{:?}", n.fills),
        Kind::Opacity => format!("{:.2}", n.opacity),
        Kind::Arc => format!("{:?}", n.kind),
    }
}

fn apply_own_value(kind: Kind, n: &mut Node) {
    match kind {
        Kind::Fill => n.fills = vec![Paint::solid(Color::rgb(0, 200, 0))],
        Kind::Opacity => n.opacity = 0.5,
        Kind::Arc => {
            n.kind = NodeKind::Ellipse(OWN_ARC);
            n.fill_geometry = None;
        }
    }
}

fn field_of(kind: Kind) -> OverrideField {
    match kind {
        Kind::Fill => OverrideField::Fills,
        Kind::Opacity => OverrideField::Opacity,
        Kind::Arc => OverrideField::Arc,
    }
}

/// `A` (a shape) <- `I` (a copy of `A`) <- `O` (a copy of `I`), with `levels` wrappers: 1 gives page > I > A, 2 gives
/// page > O > I > A. The page holds `Target` (its deepest layer carries the override) and `Follower` (no override).
fn fixture(kind: Kind, levels: usize) -> Document {
    let mut doc = Document::new("deep");
    let page = doc.add_page("Page 1");
    let a_id = doc.alloc_id();
    let a = doc.insert(page, Node::frame(a_id, Rect::new(0.0, 0.0, 48.0, 48.0))).unwrap();
    doc.get_mut(a).unwrap().name = "A".into();
    let shape = shape_node(&mut doc, kind);
    doc.insert(a, shape).unwrap();
    create_component(&mut doc, a).apply(&mut doc).unwrap();
    let mut top = a;
    for name in ["I", "O"].iter().take(levels) {
        let id = doc.alloc_id();
        let wrapper = doc.insert(page, Node::frame(id, Rect::new(0.0, 0.0, 48.0, 48.0))).unwrap();
        doc.get_mut(wrapper).unwrap().name = (*name).into();
        create_instance(&mut doc, top, wrapper, Vec2::new(0.0, 0.0)).apply(&mut doc).unwrap();
        create_component(&mut doc, wrapper).apply(&mut doc).unwrap();
        top = wrapper;
    }
    for (name, x) in [("Target", 300.0), ("Follower", 400.0)] {
        create_instance(&mut doc, top, page, Vec2::new(x, 0.0)).apply(&mut doc).unwrap();
        let inst = *doc.get(page).unwrap().children.last().unwrap();
        doc.get_mut(inst).unwrap().name = name.into();
    }
    let deep = deep_shape(&doc, instance_named(&doc, "Target")).expect("deep shape copy");
    let before = doc.get(deep).unwrap().clone();
    let mut n = before.clone();
    apply_own_value(kind, &mut n);
    n.component.overrides.extend(record_override(&before, &n));
    Command::Update { node: Box::new(n) }.apply(&mut doc).unwrap();
    doc
}

fn reopen(doc: &Document, medium: Medium) -> Document {
    let fig = import_fig(&export_fig(doc).expect("export_fig")).expect("import_fig");
    match medium {
        Medium::Fig => fig,
        Medium::FigThenNative => Document::from_json(&fig.to_json().expect("to_json")).expect("from_json"),
    }
}

fn edit_main(doc: &mut Document, kind: Kind, edit: Edit) {
    let a = component_named(doc, "A");
    let shape_id = *doc.get(a).unwrap().children.iter().find(|c| doc.get(**c).is_some_and(|n| n.name == "Shape")).expect("main shape");
    let mut n = doc.get(shape_id).unwrap().clone();
    match edit {
        Edit::Other => n.corner_radii = [5.0; 4],
        Edit::Same => match kind {
            Kind::Fill => n.fills = vec![Paint::solid(Color::rgb(0, 0, 255))],
            Kind::Opacity => n.opacity = 0.25,
            Kind::Arc => {
                n.kind = NodeKind::Ellipse(NEW_MAIN_ARC);
                n.fill_geometry = None;
            }
        },
    }
    Command::Update { node: Box::new(n) }.apply(doc).unwrap();
}

/// Runs one case and returns what went wrong, one line per problem.
fn run_case(kind: Kind, levels: usize, medium: Medium, edit: Edit) -> Vec<String> {
    let tag = format!("{kind:?} levels={levels} {medium:?} edit={edit:?}");
    let mut own = Document::new("own");
    let mut expected = shape_node(&mut own, kind);
    apply_own_value(kind, &mut expected);
    let want = value_of(kind, &expected);

    let mut doc = reopen(&fixture(kind, levels), medium);
    let target = deep_shape(&doc, instance_named(&doc, "Target")).expect("target deep shape");
    let follower = deep_shape(&doc, instance_named(&doc, "Follower")).expect("follower deep shape");
    let follower_before = value_of(kind, doc.get(follower).unwrap());
    let mut problems = Vec::new();
    if value_of(kind, doc.get(target).unwrap()) != want {
        problems.push(format!("{tag}: the change is already gone right after the reopen"));
    }

    edit_main(&mut doc, kind, edit);
    let mut syncs = vec![("A", component_named(&doc, "A"))];
    for wrapper in ["I", "O"].iter().take(levels) {
        syncs.push((wrapper, component_named(&doc, wrapper)));
    }
    for (name, comp) in syncs {
        sync_instances(&mut doc, comp).apply(&mut doc).unwrap();
        let Some(now) = deep_shape(&doc, instance_named(&doc, "Target")) else {
            problems.push(format!("{tag} after sync of {name}: the deep layer is missing"));
            continue;
        };
        let n = doc.get(now).unwrap();
        if now != target {
            problems.push(format!("{tag} after sync of {name}: the layer was rebuilt ({target:?} -> {now:?})"));
        }
        if value_of(kind, n) != want {
            problems.push(format!("{tag} after sync of {name}: value {} is not the copy's own {want}", value_of(kind, n)));
        }
        if !n.component.overrides.contains(&field_of(kind)) {
            problems.push(format!("{tag} after sync of {name}: the {:?} flag is gone", field_of(kind)));
        }
    }
    let f = doc.get(deep_shape(&doc, instance_named(&doc, "Follower")).unwrap()).unwrap();
    let took = match edit {
        Edit::Other => f.corner_radii[0] == 5.0,
        Edit::Same => value_of(kind, f) != follower_before,
    };
    if !took {
        problems.push(format!("{tag}: the follower copy did not take the main's edit"));
    }
    problems
}

#[test]
fn a_change_on_a_nested_copy_layer_survives_a_fig_reopen_and_a_main_edit() {
    let mut problems = Vec::new();
    for kind in [Kind::Fill, Kind::Opacity, Kind::Arc] {
        for levels in [1usize, 2] {
            for edit in [Edit::Other, Edit::Same] {
                problems.extend(run_case(kind, levels, Medium::Fig, edit));
            }
        }
    }
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
}

#[test]
fn a_change_on_a_nested_copy_layer_survives_a_fig_reopen_saved_natively_and_a_main_edit() {
    let mut problems = Vec::new();
    for kind in [Kind::Fill, Kind::Opacity, Kind::Arc] {
        for levels in [1usize, 2] {
            problems.extend(run_case(kind, levels, Medium::FigThenNative, Edit::Same));
        }
    }
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
}
