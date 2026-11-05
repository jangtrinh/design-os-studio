//! Which edits make the auto-layout solver run, and where it starts.
//!
//! An edit re-solves only what layout reads. A paint, effect, opacity, radius, name or connector
//! change leaves every frame as the file stored it, so imported content around the edited node
//! keeps its place. [`update_affects_layout`] is that gate for one node; [`seeds_after_commands`]
//! applies it to a list of commands and adds the containers a removal, move or insert leaves.

use designos_doc::{Command, Document, Node, NodeId};

fn is_auto_layout(node: &Node) -> bool {
    node.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).is_some()
}

/// Whether replacing `old` by `new` can change an auto-layout arrangement.
///
/// The field set follows `LAYOUT_FIELDS` in `designos-doc`'s `live_patch/replay.rs` (size, kind,
/// layout sizing, visibility), which Studio checks before it re-solves a replayed edit. Two more
/// are read by the solver and so count here: a flow child's position inside an auto-layout
/// parent, and the inward stroke width of a frame whose borders take space
/// ([`designos_doc::Stroke::layout_inset`]). A node that stays outside the flow (absolutely
/// positioned, such as a connector) changes nothing in its parent; only an auto-layout frame
/// among them re-solves its own children.
pub(crate) fn update_affects_layout(doc: &Document, old: &Node, new: &Node) -> bool {
    if old.layout_child != new.layout_child {
        return true;
    }
    let own_geometry = old.size != new.size || old.kind != new.kind || old.stroke.layout_inset() != new.stroke.layout_inset();
    if new.layout_child.absolute {
        return own_geometry && is_auto_layout(new);
    }
    let parent_is_auto_layout = || new.parent.and_then(|p| doc.get(p)).is_some_and(is_auto_layout);
    own_geometry || old.visible != new.visible || (old.transform != new.transform && parent_is_auto_layout())
}

fn flatten<'a>(cmds: &'a [Command], out: &mut Vec<&'a Command>) {
    for c in cmds {
        match c {
            Command::Batch(inner) => flatten(inner, out),
            other => out.push(other),
        }
    }
}

/// The nearest ancestor of `id` that is still in the document once `removed` (and the subtrees
/// under them) are gone.
fn surviving_parent(before: &Document, removed: &[NodeId], id: NodeId) -> Option<NodeId> {
    let mut cur = before.get(id)?.parent?;
    while removed.iter().any(|r| *r == cur || before.is_ancestor(*r, cur)) {
        cur = before.get(cur)?.parent?;
    }
    Some(cur)
}

/// The nodes whose auto-layout must be re-solved after `cmds` run on `before`, read before they
/// run: an `Update` that changes something layout reads, the former and the new parent of a
/// removed or moved layer, and the parent and root of an insert. A layer that stays outside the
/// flow does not seed its parent. Nested batches are walked.
pub(crate) fn seeds_after_commands(before: &Document, cmds: &[Command]) -> Vec<NodeId> {
    let mut flat = Vec::new();
    flatten(cmds, &mut flat);
    let removed: Vec<NodeId> = flat
        .iter()
        .filter_map(|c| match c {
            Command::Remove { id } => Some(*id),
            _ => None,
        })
        .collect();
    let mut seeds: Vec<NodeId> = Vec::new();
    let mut seed = |id: NodeId| {
        if !seeds.contains(&id) {
            seeds.push(id);
        }
    };
    for c in flat {
        match c {
            Command::Update { node } => {
                if before.get(node.id).is_some_and(|old| update_affects_layout(before, old, node)) {
                    seed(node.id);
                }
            }
            Command::Remove { id } => {
                if let Some(parent) = surviving_parent(before, &removed, *id) {
                    seed(parent);
                }
            }
            Command::Reparent { id, parent, .. } => {
                if before.get(*id).is_some_and(|n| !n.layout_child.absolute) {
                    if let Some(old_parent) = surviving_parent(before, &removed, *id) {
                        seed(old_parent);
                    }
                    seed(*parent);
                }
            }
            Command::Insert { parent, nodes, .. } => {
                if let Some(root) = nodes.first() {
                    if !root.layout_child.absolute {
                        seed(*parent);
                    }
                    seed(root.id);
                }
            }
            _ => {}
        }
    }
    seeds
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{AutoLayout, Color, FrameProps, NodeKind, Paint, Rect, Sizing};

    /// page > column (auto layout) > [in_column]; page > plain frame > [in_plain].
    struct Fixture {
        doc: Document,
        column: NodeId,
        in_column: NodeId,
        plain: NodeId,
        in_plain: NodeId,
    }

    fn fixture() -> Fixture {
        let mut doc = Document::new("scope");
        let page = doc.add_page("Page");
        let id = doc.alloc_id();
        let mut column = Node::frame(id, Rect::new(0.0, 0.0, 100.0, 100.0));
        column.kind = NodeKind::Frame(FrameProps { auto_layout: Some(AutoLayout::default()), ..FrameProps::default() });
        let column = doc.insert(page, column).unwrap();
        let id = doc.alloc_id();
        let in_column = doc.insert(column, Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0))).unwrap();
        let id = doc.alloc_id();
        let plain = doc.insert(page, Node::frame(id, Rect::new(200.0, 0.0, 100.0, 100.0))).unwrap();
        let id = doc.alloc_id();
        let in_plain = doc.insert(plain, Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0))).unwrap();
        Fixture { doc, column, in_column, plain, in_plain }
    }

    fn affects(doc: &Document, id: NodeId, edit: impl FnOnce(&mut Node)) -> bool {
        let old = doc.get(id).unwrap();
        let mut new = old.clone();
        edit(&mut new);
        update_affects_layout(doc, old, &new)
    }

    fn update_of(doc: &Document, id: NodeId, edit: impl FnOnce(&mut Node)) -> Command {
        let mut node = doc.get(id).unwrap().clone();
        edit(&mut node);
        Command::Update { node: Box::new(node) }
    }

    #[test]
    fn paint_opacity_radius_and_name_edits_re_solve_nothing() {
        let f = fixture();
        assert!(!affects(&f.doc, f.in_column, |n| n.fills = vec![Paint::solid(Color::BLACK)]));
        assert!(!affects(&f.doc, f.in_column, |n| n.opacity = 0.5));
        assert!(!affects(&f.doc, f.in_column, |n| n.corner_radii = [4.0; 4]));
        assert!(!affects(&f.doc, f.in_column, |n| n.name = "renamed".into()));
        assert!(!affects(&f.doc, f.column, |n| n.fills = vec![Paint::solid(Color::BLACK)]));
    }

    #[test]
    fn size_visibility_layout_sizing_and_kind_edits_re_solve() {
        let f = fixture();
        assert!(affects(&f.doc, f.in_column, |n| n.size.x = 80.0));
        assert!(affects(&f.doc, f.in_column, |n| n.visible = false));
        assert!(affects(&f.doc, f.in_column, |n| n.layout_child.horizontal = Sizing::Fill));
        assert!(affects(&f.doc, f.column, |n| n.kind = NodeKind::Frame(FrameProps::default())), "dropping the auto layout is a kind change");
    }

    #[test]
    fn a_position_edit_re_solves_only_a_flow_child_of_an_auto_layout_parent() {
        let mut f = fixture();
        assert!(affects(&f.doc, f.in_column, |n| n.transform.tx = 30.0));
        assert!(!affects(&f.doc, f.in_plain, |n| n.transform.tx = 30.0), "a plain frame keeps what it is given");
        f.doc.get_mut(f.in_column).unwrap().layout_child.absolute = true;
        assert!(!affects(&f.doc, f.in_column, |n| n.transform.tx = 30.0), "an absolute child is outside the flow");
    }

    #[test]
    fn an_absolute_layer_re_solves_only_its_own_children() {
        let mut f = fixture();
        f.doc.get_mut(f.in_column).unwrap().layout_child.absolute = true;
        f.doc.get_mut(f.column).unwrap().layout_child.absolute = true;
        assert!(!affects(&f.doc, f.in_column, |n| n.size.x = 80.0), "a connector-like layer changes nothing in its parent");
        assert!(affects(&f.doc, f.column, |n| n.size.x = 80.0), "an absolute auto-layout frame still places its own children");
        assert!(affects(&f.doc, f.in_column, |n| n.layout_child.absolute = false), "joining the flow changes the parent");
    }

    #[test]
    fn a_stroke_width_that_takes_space_re_solves_and_a_stroke_colour_does_not() {
        let mut f = fixture();
        let column = f.doc.get_mut(f.column).unwrap();
        column.stroke.paints = vec![Paint::solid(Color::BLACK)];
        column.stroke.weight = 2.0;
        assert!(affects(&f.doc, f.column, |n| n.stroke.weight = 6.0));
        assert!(!affects(&f.doc, f.column, |n| n.stroke.paints = vec![Paint::solid(Color::WHITE)]));
    }

    #[test]
    fn seeds_follow_what_a_command_list_changes() {
        let f = fixture();
        let fill = update_of(&f.doc, f.in_column, |n| n.fills = vec![Paint::solid(Color::BLACK)]);
        let wider = update_of(&f.doc, f.in_column, |n| n.size.x = 80.0);
        assert_eq!(seeds_after_commands(&f.doc, std::slice::from_ref(&fill)), Vec::<NodeId>::new());
        assert_eq!(seeds_after_commands(&f.doc, &[fill, wider]), vec![f.in_column]);
        let nested_removal = Command::Batch(vec![Command::Remove { id: f.in_column }]);
        assert_eq!(seeds_after_commands(&f.doc, &[nested_removal]), vec![f.column], "a removal seeds the layer's former parent, nested or not");
        let moved_out = Command::Reparent { id: f.in_column, parent: f.plain, index: 0 };
        assert_eq!(seeds_after_commands(&f.doc, &[moved_out]), vec![f.column, f.plain], "a move seeds the parent it left and the one it joins");
    }

    #[test]
    fn a_removal_inside_a_removed_subtree_seeds_the_nearest_parent_that_stays() {
        let f = fixture();
        let page = f.doc.get(f.column).unwrap().parent.unwrap();
        let cmds = [Command::Remove { id: f.column }, Command::Remove { id: f.in_column }];
        assert_eq!(seeds_after_commands(&f.doc, &cmds), vec![page]);
    }

    #[test]
    fn a_layer_outside_the_flow_seeds_no_parent_when_inserted_or_moved() {
        let mut f = fixture();
        let absolute_id = f.doc.alloc_id();
        let mut absolute = Node::rectangle(absolute_id, Rect::new(0.0, 0.0, 10.0, 10.0));
        absolute.layout_child.absolute = true;
        let insert = Command::Insert { parent: f.column, index: 1, nodes: vec![absolute] };
        assert_eq!(seeds_after_commands(&f.doc, &[insert]), vec![absolute_id], "only the inserted root itself");

        let flow_id = f.doc.alloc_id();
        let flow = Command::Insert { parent: f.column, index: 1, nodes: vec![Node::rectangle(flow_id, Rect::new(0.0, 0.0, 10.0, 10.0))] };
        assert_eq!(seeds_after_commands(&f.doc, &[flow]), vec![f.column, flow_id], "a flow child seeds its parent too");

        f.doc.get_mut(f.in_column).unwrap().layout_child.absolute = true;
        let moved = Command::Reparent { id: f.in_column, parent: f.plain, index: 0 };
        assert_eq!(seeds_after_commands(&f.doc, &[moved]), Vec::<NodeId>::new());
    }
}
