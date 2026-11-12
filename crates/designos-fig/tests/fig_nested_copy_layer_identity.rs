//! A layer inside a nested component copy is told apart by the authored layer it stands for, one level at a time.
//!
//! Two copies of one main inside a wrapper are two authored layers even though everything about them is named the same;
//! and a layer's link points to the matching layer one level up, so a reset takes back the value of the main right above
//! the copy, not the value of the innermost main.
use designos_doc::*;
use designos_fig::{export_fig, import_fig, import_rest_json};
use serde_json::json;

fn rect(doc: &mut Document, name: &str, rgb: (u8, u8, u8)) -> Node {
    let id = doc.alloc_id();
    let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0));
    n.name = name.into();
    n.fills = vec![Paint::solid(Color::rgb(rgb.0, rgb.1, rgb.2))];
    n
}

fn frame_with(doc: &mut Document, page: NodeId, name: &str, child: Node) -> NodeId {
    let id = doc.alloc_id();
    let f = doc.insert(page, Node::frame(id, Rect::new(0.0, 0.0, 40.0, 40.0))).unwrap();
    doc.get_mut(f).unwrap().name = name.into();
    doc.insert(f, child).unwrap();
    f
}

fn component_named(doc: &Document, name: &str) -> NodeId {
    doc.nodes.values().find(|n| n.name == name && matches!(n.kind, NodeKind::Component(_))).map(|n| n.id).unwrap_or_else(|| panic!("no component {name}"))
}

fn instance_named(doc: &Document, name: &str) -> NodeId {
    doc.nodes.values().find(|n| n.name == name && matches!(n.kind, NodeKind::Instance { .. })).map(|n| n.id).unwrap_or_else(|| panic!("no instance {name}"))
}

fn set_fill(doc: &mut Document, id: NodeId, rgb: (u8, u8, u8)) {
    let before = doc.get(id).unwrap().clone();
    let mut n = before.clone();
    n.fills = vec![Paint::solid(Color::rgb(rgb.0, rgb.1, rgb.2))];
    n.component.overrides.extend(record_override(&before, &n));
    Command::Update { node: Box::new(n) }.apply(doc).unwrap();
}

fn fill_of(doc: &Document, id: NodeId) -> Paint {
    doc.get(id).unwrap().fills[0].clone()
}

fn reopen(doc: &Document) -> Document {
    import_fig(&export_fig(doc).expect("export_fig")).expect("import_fig")
}

/// Main `I` holds two copies of main `X`, both named `Item` and both holding a layer named `Shape`; main `O` holds a copy
/// of `I`; the page copy `Trip` of `O` has its second `Item` swapped to main `Y`. The `Shape` of each `Item` carries a fill
/// of its own. The two `Item`s sit side by side one level below the first nested copy, so a link that skips that level
/// cannot tell them apart by name, by shape or by what they show.
fn twins() -> Document {
    let mut doc = Document::new("twins");
    let page = doc.add_page("Page 1");
    let shape = rect(&mut doc, "Shape", (255, 0, 0));
    let x = frame_with(&mut doc, page, "X", shape);
    create_component(&mut doc, x).apply(&mut doc).unwrap();
    let shape = rect(&mut doc, "Shape", (0, 0, 255));
    let y = frame_with(&mut doc, page, "Y", shape);
    create_component(&mut doc, y).apply(&mut doc).unwrap();
    let id = doc.alloc_id();
    let i = doc.insert(page, Node::frame(id, Rect::new(0.0, 0.0, 120.0, 40.0))).unwrap();
    doc.get_mut(i).unwrap().name = "I".into();
    for x_pos in [0.0, 60.0] {
        create_instance(&mut doc, x, i, Vec2::new(x_pos, 0.0)).apply(&mut doc).unwrap();
        let item = *doc.get(i).unwrap().children.last().unwrap();
        doc.get_mut(item).unwrap().name = "Item".into();
    }
    create_component(&mut doc, i).apply(&mut doc).unwrap();
    let id = doc.alloc_id();
    let o = doc.insert(page, Node::frame(id, Rect::new(0.0, 0.0, 120.0, 40.0))).unwrap();
    doc.get_mut(o).unwrap().name = "O".into();
    create_instance(&mut doc, i, o, Vec2::new(0.0, 0.0)).apply(&mut doc).unwrap();
    create_component(&mut doc, o).apply(&mut doc).unwrap();
    create_instance(&mut doc, o, page, Vec2::new(300.0, 0.0)).apply(&mut doc).unwrap();
    let trip = *doc.get(page).unwrap().children.last().unwrap();
    doc.get_mut(trip).unwrap().name = "Trip".into();
    let trail = items_of(&doc)[1];
    swap_instance(&mut doc, trail, y).apply(&mut doc).unwrap();
    for (item, rgb) in [(0usize, (0, 200, 0)), (1, (250, 200, 0))] {
        let shape = doc.get(items_of(&doc)[item]).unwrap().children[0];
        set_fill(&mut doc, shape, rgb);
    }
    doc
}

/// The two `Item` copies inside the page copy `Trip`.
fn items_of(doc: &Document) -> Vec<NodeId> {
    let trip = instance_named(doc, "Trip");
    let inner = doc.get(trip).unwrap().children[0];
    doc.get(inner).unwrap().children.clone()
}

/// Each `Item` of the page copy, as (main it shows, id of its inner layer, that layer's fill).
fn pair_state(doc: &Document) -> Vec<(Option<NodeId>, NodeId, Paint)> {
    items_of(doc)
        .into_iter()
        .map(|item| {
            let n = doc.get(item).unwrap();
            let shape = *n.children.first().expect("copy holds its layer");
            (instance_component(n), shape, fill_of(doc, shape))
        })
        .collect()
}

#[test]
fn two_copies_of_one_main_stay_two_copies_and_keep_their_own_layer_after_a_fig_round_trip_and_a_wrapper_sync() {
    let session = twins();
    let session_state = pair_state(&session);
    assert_eq!(session_state.len(), 2, "fixture: the page copy holds both copies");
    assert_ne!(session_state[0].0, session_state[1].0, "fixture: the second copy is swapped to another main");

    let mut doc = reopen(&session);
    let layers_before = doc.nodes.len();
    let opened = pair_state(&doc);
    assert_eq!(opened.len(), 2);
    for wrapper in ["I", "O"] {
        let main = component_named(&doc, wrapper);
        sync_instances(&mut doc, main).apply(&mut doc).unwrap();
    }
    let synced = pair_state(&doc);

    assert_eq!(synced.len(), 2, "still exactly two copies");
    assert_eq!(doc.nodes.len(), layers_before, "no layer was added or dropped by the syncs");
    assert_eq!(synced[0].0, Some(component_named(&doc, "X")), "the first copy shows X");
    assert_eq!(synced[1].0, Some(component_named(&doc, "Y")), "the swap stays on the same copy");
    for (i, (open, now)) in opened.iter().zip(&synced).enumerate() {
        assert_eq!(open.1, now.1, "copy {i}: its layer was rebuilt, not kept");
        assert_eq!(open.2, now.2, "copy {i}: its own fill was lost");
    }
    assert_ne!(synced[0].2, synced[1].2, "the two copies keep different fills of their own");
}

/// `A` (red shape) <- `I` (holds a copy of `A` whose shape is blue in `I`) <- page copy `Page copy` of `I`.
fn blue_in_the_middle() -> Document {
    let mut doc = Document::new("reset");
    let page = doc.add_page("Page 1");
    let shape = rect(&mut doc, "Shape", (255, 0, 0));
    let a = frame_with(&mut doc, page, "A", shape);
    create_component(&mut doc, a).apply(&mut doc).unwrap();
    let id = doc.alloc_id();
    let i = doc.insert(page, Node::frame(id, Rect::new(0.0, 0.0, 40.0, 40.0))).unwrap();
    doc.get_mut(i).unwrap().name = "I".into();
    create_instance(&mut doc, a, i, Vec2::new(0.0, 0.0)).apply(&mut doc).unwrap();
    let inner = doc.get(i).unwrap().children[0];
    let inner_shape = doc.get(inner).unwrap().children[0];
    set_fill(&mut doc, inner_shape, (0, 0, 255));
    create_component(&mut doc, i).apply(&mut doc).unwrap();
    create_instance(&mut doc, i, page, Vec2::new(300.0, 0.0)).apply(&mut doc).unwrap();
    let copy = *doc.get(page).unwrap().children.last().unwrap();
    doc.get_mut(copy).unwrap().name = "Page copy".into();
    doc
}

fn page_copy_shape(doc: &Document) -> NodeId {
    let inner = doc.get(instance_named(doc, "Page copy")).unwrap().children[0];
    doc.get(inner).unwrap().children[0]
}

#[test]
fn reset_of_an_override_on_a_deep_layer_of_a_reopened_fig_gives_the_outer_mains_value() {
    let blue = Paint::solid(Color::rgb(0, 0, 255));
    let mut session = blue_in_the_middle();
    let shape = page_copy_shape(&session);
    assert_eq!(fill_of(&session, shape), blue, "fixture: the copy shows the blue the wrapper set");
    set_fill(&mut session, shape, (0, 200, 0));
    reset_overrides(&mut session, shape, &[OverrideField::Fills]).apply(&mut session).unwrap();
    assert_eq!(fill_of(&session, shape), blue, "in the session a reset gives the wrapper's blue");

    let mut doc = blue_in_the_middle();
    let shape = page_copy_shape(&doc);
    set_fill(&mut doc, shape, (0, 200, 0));
    let mut doc = reopen(&doc);
    let shape = page_copy_shape(&doc);
    assert_eq!(fill_of(&doc, shape), Paint::solid(Color::rgb(0, 200, 0)), "the override survives the reopen");
    reset_overrides(&mut doc, shape, &[OverrideField::Fills]).apply(&mut doc).unwrap();
    assert_eq!(fill_of(&doc, page_copy_shape(&doc)), blue, "after the reopen a reset gives the wrapper's blue, not the innermost main's red");
}

/// The first child at each step down from `root`.
fn first_child_chain(doc: &Document, root: NodeId) -> Vec<NodeId> {
    let mut chain = Vec::new();
    let mut cur = root;
    while let Some(next) = doc.get(cur).unwrap().children.first().copied() {
        chain.push(next);
        cur = next;
    }
    chain
}

/// A Figma REST document names a nested copy's layers by the ids of the level they sit in
/// (`I<page copy>;<instance in the outer main>;<layer in the inner main>`), so the importer already resolves each name one
/// level up. Pinned so that the relink pass stays a no-op for such a file; it was green before the relink pass existed.
#[test]
fn a_rest_document_links_each_nested_copy_layer_one_level_up_and_needs_no_relink() {
    let rect = |id: &str| json!({"id": id, "name": "Dot", "type": "RECTANGLE", "size": {"x": 5, "y": 5}});
    let inst = |id: &str, main: &str, kids: Vec<serde_json::Value>| json!({"id": id, "name": "N", "type": "INSTANCE", "componentId": main, "size": {"x": 5, "y": 5}, "children": kids});
    let j = json!({"document": {"type": "DOCUMENT", "children": [{"id": "0:1", "type": "CANVAS", "children": [
        {"id": "1:1", "name": "Inner", "type": "COMPONENT", "size": {"x": 5, "y": 5}, "children": [rect("1:2")]},
        {"id": "2:1", "name": "Mid", "type": "COMPONENT", "size": {"x": 5, "y": 5}, "children": [inst("2:2", "1:1", vec![rect("I2:2;1:2")])]},
        {"id": "3:1", "name": "Outer", "type": "COMPONENT", "size": {"x": 5, "y": 5}, "children": [
            inst("3:2", "2:1", vec![inst("I3:2;2:2", "1:1", vec![rect("I3:2;2:2;1:2")])])]},
        {"id": "4:1", "name": "Use", "type": "INSTANCE", "componentId": "3:1", "size": {"x": 5, "y": 5}, "children": [
            inst("I4:1;3:2", "2:1", vec![inst("I4:1;3:2;2:2", "1:1", vec![rect("I4:1;3:2;2:2;1:2")])])]}
    ]}]}});
    let mut doc = import_rest_json(&j.to_string()).unwrap();
    let copies = first_child_chain(&doc, instance_named(&doc, "Use"));
    let mains = first_child_chain(&doc, component_named(&doc, "Outer"));
    assert_eq!((copies.len(), mains.len()), (3, 3), "the mid copy, the inner copy and the layer, in the page copy and in the main");
    for (level, (copy, one_up)) in copies.iter().zip(&mains).enumerate() {
        assert_eq!(doc.get(*copy).unwrap().component.source, Some(*one_up), "level {level}: linked to the layer one level up");
    }
    assert_eq!(relink_copy_layers(&mut doc), 0, "nothing to relink");
}
