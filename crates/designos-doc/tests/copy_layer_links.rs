//! Copy layers link to the layer one level up. A file that names a nested copy's layers by the layer of the innermost
//! main (how older builds read a Studio-written `.fig`) is relinked level by level, and only where the match is clear.
use designos_doc::*;
use std::collections::BTreeMap;

/// `A` (one `Shape`) <- `I` (a copy of `A`) <- `O` (a copy of `I`), and a page copy `Page copy` of the outermost main.
/// `levels` is 1 (page copy of `I`) or 2 (page copy of `O`).
fn nested(levels: usize) -> Document {
    let mut doc = Document::new("nested");
    let page = doc.add_page("Page 1");
    let a_id = doc.alloc_id();
    let a = doc.insert(page, Node::frame(a_id, Rect::new(0.0, 0.0, 48.0, 48.0))).unwrap();
    doc.get_mut(a).unwrap().name = "A".into();
    let shape_id = doc.alloc_id();
    let mut shape = Node::rectangle(shape_id, Rect::new(0.0, 0.0, 48.0, 48.0));
    shape.name = "Shape".into();
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
    create_instance(&mut doc, top, page, Vec2::new(300.0, 0.0)).apply(&mut doc).unwrap();
    let copy = *doc.get(page).unwrap().children.last().unwrap();
    doc.get_mut(copy).unwrap().name = "Page copy".into();
    doc
}

fn named(doc: &Document, name: &str, component: bool) -> NodeId {
    doc.nodes.values().find(|n| n.name == name && matches!(n.kind, NodeKind::Component(_)) == component).map(|n| n.id).unwrap_or_else(|| panic!("no {name}"))
}

/// Every layer below `root`, parents before children.
fn below(doc: &Document, root: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut stack: Vec<NodeId> = doc.get(root).unwrap().children.iter().rev().copied().collect();
    while let Some(id) = stack.pop() {
        out.push(id);
        stack.extend(doc.get(id).unwrap().children.iter().rev().copied());
    }
    out
}

fn links(doc: &Document, root: NodeId) -> BTreeMap<NodeId, Option<NodeId>> {
    below(doc, root).into_iter().map(|id| (id, doc.get(id).unwrap().component.source)).collect()
}

/// Links every layer below `root` to the layer it stands for in the innermost main, which is what the importer of the
/// previous release wrote for a Studio-written `.fig`.
fn skip_levels(doc: &mut Document, root: NodeId) {
    for id in below(doc, root) {
        let authored = authored_layer(doc, id);
        doc.get_mut(id).unwrap().component.source = Some(authored);
    }
}

#[test]
fn an_authored_layer_is_the_layer_of_the_innermost_main_and_stops_before_a_main_root() {
    let doc = nested(2);
    let page_copy = named(&doc, "Page copy", false);
    let (i_copy, a_copy, shape_copy) = (below(&doc, page_copy)[0], below(&doc, page_copy)[1], *below(&doc, page_copy).last().unwrap());
    let a = named(&doc, "A", true);
    let main_shape = doc.get(a).unwrap().children[0];

    assert_eq!(authored_layer(&doc, main_shape), main_shape, "a main's own layer stands for itself");
    assert_eq!(authored_layer(&doc, shape_copy), main_shape, "a copy layer stands for the layer of the innermost main");
    let a_in_i = doc.get(named(&doc, "I", true)).unwrap().children[0];
    assert_eq!(authored_layer(&doc, a_copy), a_in_i, "an inner instance stands for the instance in its wrapper, not for the main it shows");
    let i_in_o = doc.get(named(&doc, "O", true)).unwrap().children[0];
    assert_eq!(authored_layer(&doc, i_copy), i_in_o);
}

#[test]
fn links_that_skip_a_level_are_relinked_to_the_layer_one_level_up() {
    for levels in [1usize, 2] {
        let mut doc = nested(levels);
        let root = named(&doc, "Page copy", false);
        let right = links(&doc, root);
        skip_levels(&mut doc, root);
        let broken = links(&doc, root);
        assert_ne!(broken, right, "fixture: the links are wrong now (levels={levels})");

        let changed = relink_copy_layers(&mut doc);

        assert_eq!(links(&doc, root), right, "levels={levels}: every link is back one level up");
        assert_eq!(changed, broken.iter().filter(|(id, s)| right[*id] != **s).count(), "levels={levels}: the count is the links that changed");
        assert_eq!(relink_copy_layers(&mut doc), 0, "levels={levels}: a second pass finds nothing to do");
    }
}

#[test]
fn links_that_are_already_one_level_up_are_left_alone() {
    let mut doc = nested(2);
    let root = named(&doc, "Page copy", false);
    let before = links(&doc, root);
    let epoch = doc.mutation_epoch();
    assert_eq!(relink_copy_layers(&mut doc), 0);
    assert_eq!(links(&doc, root), before);
    assert_eq!(doc.mutation_epoch(), epoch, "a document with nothing to relink is not touched");
}

#[test]
fn a_layer_with_two_candidates_one_level_up_is_not_relinked() {
    let mut doc = nested(1);
    let root = named(&doc, "Page copy", false);
    skip_levels(&mut doc, root);
    // `I` now holds two layers that stand for the main's one `Shape`: the page copy's layer cannot tell which it is.
    let i = named(&doc, "I", true);
    let a_in_i = doc.get(i).unwrap().children[0];
    let twin = doc.get(a_in_i).unwrap().children[0];
    let mut dup = doc.get(twin).unwrap().clone();
    dup.id = doc.alloc_id();
    dup.children.clear();
    doc.insert(a_in_i, dup).unwrap();
    let before = links(&doc, root);

    let changed = relink_copy_layers(&mut doc);

    assert_eq!(links(&doc, root)[&below(&doc, root)[1]], before[&below(&doc, root)[1]], "the ambiguous layer keeps its link");
    assert_eq!(changed, 0, "nothing is guessed");
}

#[test]
fn two_layers_that_want_one_candidate_are_not_relinked() {
    let mut doc = nested(1);
    let root = named(&doc, "Page copy", false);
    skip_levels(&mut doc, root);
    // the page copy's inner instance holds two layers that stand for the same main layer
    let a_copy = doc.get(root).unwrap().children[0];
    let first = doc.get(a_copy).unwrap().children[0];
    let mut dup = doc.get(first).unwrap().clone();
    dup.id = doc.alloc_id();
    doc.insert(a_copy, dup).unwrap();
    let before = links(&doc, a_copy);

    let changed = relink_copy_layers(&mut doc);

    assert_eq!(links(&doc, a_copy), before, "neither duplicate is relinked");
    assert_eq!(changed, 0);
}
