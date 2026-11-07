//! Which edits make the auto-layout solver run, and where it starts.
//!
//! An edit re-solves only what layout reads. A paint, effect, opacity, radius, name or connector
//! change leaves every frame as the file stored it, so imported content around the edited node
//! keeps its place. [`update_affects_layout`] is that gate for one node; [`seeds_after_commands`]
//! applies it to a list of commands and adds the containers a removal, move or insert leaves.

use designos_doc::{Command, Document, Node, NodeId, NodeKind};

fn is_auto_layout(node: &Node) -> bool {
    node.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).is_some()
}

/// Where a re-solve starts after a list of commands.
///
/// A `leaf` is a layer whose own edit can change its size or place; the frame that holds it runs
/// too, because the solver does not see a leaf's size change. A `container` is a frame that lost
/// or gained a child; it runs on its own and the frames above it run only if its size changes.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct LayoutSeeds {
    pub(crate) leaves: Vec<NodeId>,
    pub(crate) containers: Vec<NodeId>,
}

fn push_unique(list: &mut Vec<NodeId>, id: NodeId) {
    if !list.contains(&id) {
        list.push(id);
    }
}

/// `kind` reduced to what the solver and the text measurer read, so an edit that only repaints a
/// layer compares equal. One line per group that stays:
/// - text content, runs, font, size, line height, letter and paragraph spacing, case, features and
///   axes, list ranges, direction, wrap, truncation and auto-resize: `designos_render::measure_text_box`
///   (through `layout_measurer`) and the hug text sizing in `layout_apply::flow_child_input`;
/// - the auto-layout fields of a frame, component or instance (direction, sizing, align, gaps,
///   padding, wrap, grid tracks, version, stroke flag): `layout_apply::solve_auto_layout_node`;
/// - every other kind, whole. The solver reads none of it; it is kept because no smaller set is
///   proven safe (a resize rewrites vector and boolean geometry in `solve_auto_layout_node`).
///
/// What is dropped, because neither reads it: the paints of a text style and of each run
/// (`measure_text_box` clears them before it measures), the text flags `auto_rename` and
/// `decoration_skip_ink`, and a frame's content clip and layout grids (guides only; the solver
/// reads neither `FrameProps::clips_content` nor `FrameProps::grids`).
fn layout_projection(kind: &NodeKind) -> NodeKind {
    let mut kind = kind.clone();
    match &mut kind {
        NodeKind::Text(tp) => {
            tp.style.fills.clear();
            for run in &mut tp.runs {
                run.style.fills.clear();
            }
            tp.auto_rename = true;
            tp.decoration_skip_ink = true;
        }
        NodeKind::Frame(f) | NodeKind::Component(f) | NodeKind::ComponentSet(f) | NodeKind::Instance { frame: f, .. } => {
            f.grids.clear();
            f.clips_content = false;
        }
        _ => {}
    }
    kind
}

/// The stroke inset the solver adds to padding, for the one kind of node that reads it: an
/// auto-layout frame in layout version 0 whose borders take space (`AutoLayout::effective_padding`,
/// called from `layout_solver::compute_with_stroke` on the frame being solved). Any other node's
/// stroke is not read by any solve.
pub(crate) fn stroke_inset_read_by_solver(node: &Node) -> Option<[f32; 4]> {
    let al = node.kind.frame_props().and_then(|f| f.auto_layout.as_ref())?;
    (al.layout_version == 0 && al.stroke_included).then(|| node.stroke.layout_inset())
}

/// Whether replacing `old` by `new` can change an auto-layout arrangement.
///
/// The field set follows `LAYOUT_FIELDS` in `designos-doc`'s `live_patch/replay.rs` (size, kind,
/// layout sizing, visibility), which Studio checks before it re-solves a replayed edit, with
/// `kind` reduced by [`layout_projection`]. Two more are read by the solver and so count here: a
/// flow child's position inside an auto-layout parent, and the stroke inset of a frame that reads
/// it ([`stroke_inset_read_by_solver`]). A node that stays outside the flow (absolutely positioned,
/// such as a connector) changes nothing in its parent; only an auto-layout frame among them
/// re-solves its own children.
pub(crate) fn update_affects_layout(doc: &Document, old: &Node, new: &Node) -> bool {
    if old.layout_child != new.layout_child {
        return true;
    }
    let own_geometry = old.size != new.size
        || (old.kind != new.kind && layout_projection(&old.kind) != layout_projection(&new.kind))
        || stroke_inset_read_by_solver(old) != stroke_inset_read_by_solver(new);
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

/// Where auto layout must be re-solved after `cmds` run on `before`, read before they run: an
/// `Update` that changes something layout reads (a leaf), the former and the new parent of a
/// removed or moved layer (containers), and the root of an insert (a leaf, or for a layer outside
/// the flow a container, so its home frame does not run). Nested batches are walked.
pub(crate) fn seeds_after_commands(before: &Document, cmds: &[Command]) -> LayoutSeeds {
    let mut flat = Vec::new();
    flatten(cmds, &mut flat);
    let removed: Vec<NodeId> = flat
        .iter()
        .filter_map(|c| match c {
            Command::Remove { id } => Some(*id),
            _ => None,
        })
        .collect();
    let mut seeds = LayoutSeeds::default();
    for c in flat {
        match c {
            Command::Update { node } => {
                if before.get(node.id).is_some_and(|old| update_affects_layout(before, old, node)) {
                    push_unique(&mut seeds.leaves, node.id);
                }
            }
            Command::Remove { id } => {
                if let Some(parent) = surviving_parent(before, &removed, *id) {
                    push_unique(&mut seeds.containers, parent);
                }
            }
            Command::Reparent { id, parent, .. } => {
                if before.get(*id).is_some_and(|n| !n.layout_child.absolute) {
                    if let Some(old_parent) = surviving_parent(before, &removed, *id) {
                        push_unique(&mut seeds.containers, old_parent);
                    }
                    push_unique(&mut seeds.containers, *parent);
                }
            }
            Command::Insert { nodes, .. } => {
                if let Some(root) = nodes.first() {
                    if root.layout_child.absolute {
                        push_unique(&mut seeds.containers, root.id);
                    } else {
                        push_unique(&mut seeds.leaves, root.id);
                    }
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
    fn a_stroke_width_re_solves_only_a_frame_whose_borders_take_space() {
        let mut f = fixture();
        let column = f.doc.get_mut(f.column).unwrap();
        column.stroke.paints = vec![Paint::solid(Color::BLACK)];
        column.stroke.weight = 2.0;
        if let Some(al) = column.kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) {
            al.layout_version = 0;
            al.stroke_included = true;
        }
        assert!(affects(&f.doc, f.column, |n| n.stroke.weight = 6.0));
        assert!(!affects(&f.doc, f.column, |n| n.stroke.paints = vec![Paint::solid(Color::WHITE)]), "a colour takes no space");
        assert!(!affects(&f.doc, f.in_column, |n| n.stroke.weight = 6.0), "the solver reads no child's stroke");

        let column = f.doc.get_mut(f.column).unwrap();
        if let Some(al) = column.kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) {
            al.layout_version = 1;
        }
        assert!(!affects(&f.doc, f.column, |n| n.stroke.weight = 6.0), "in layout version 1 the border sits inside the box");
    }

    #[test]
    fn a_text_fill_and_a_layout_grid_re_solve_nothing_and_a_font_size_does() {
        let mut f = fixture();
        let id = f.doc.alloc_id();
        let text = Node::text(id, Rect::new(0.0, 0.0, 50.0, 20.0), "hello", designos_doc::TextStyle::default());
        let text = f.doc.insert(f.column, text).unwrap();
        assert!(!affects(&f.doc, text, |n| {
            if let NodeKind::Text(tp) = &mut n.kind {
                tp.style.fills = vec![Paint::solid(Color::BLACK)];
            }
        }));
        assert!(affects(&f.doc, text, |n| {
            if let NodeKind::Text(tp) = &mut n.kind {
                tp.style.font_size += 10.0;
            }
        }));
        assert!(affects(&f.doc, text, |n| {
            if let NodeKind::Text(tp) = &mut n.kind {
                tp.content.push('!');
            }
        }));
        assert!(!affects(&f.doc, text, |n| {
            if let NodeKind::Text(tp) = &mut n.kind {
                tp.auto_rename = !tp.auto_rename;
                tp.decoration_skip_ink = !tp.decoration_skip_ink;
            }
        }));
        assert!(!affects(&f.doc, f.column, |n| {
            if let Some(fp) = n.kind.frame_props_mut() {
                fp.grids = vec![designos_doc::LayoutGrid::Grid { size: 8.0, color: Color::BLACK, visible: true }];
                fp.clips_content = !fp.clips_content;
            }
        }));
    }

    fn seeds(leaves: &[NodeId], containers: &[NodeId]) -> LayoutSeeds {
        LayoutSeeds { leaves: leaves.to_vec(), containers: containers.to_vec() }
    }

    #[test]
    fn seeds_follow_what_a_command_list_changes() {
        let f = fixture();
        let fill = update_of(&f.doc, f.in_column, |n| n.fills = vec![Paint::solid(Color::BLACK)]);
        let wider = update_of(&f.doc, f.in_column, |n| n.size.x = 80.0);
        assert_eq!(seeds_after_commands(&f.doc, std::slice::from_ref(&fill)), seeds(&[], &[]));
        assert_eq!(seeds_after_commands(&f.doc, &[fill, wider]), seeds(&[f.in_column], &[]));
        let nested_removal = Command::Batch(vec![Command::Remove { id: f.in_column }]);
        assert_eq!(
            seeds_after_commands(&f.doc, &[nested_removal]),
            seeds(&[], &[f.column]),
            "a removal seeds the layer's former parent as a container, nested or not"
        );
        let moved_out = Command::Reparent { id: f.in_column, parent: f.plain, index: 0 };
        assert_eq!(seeds_after_commands(&f.doc, &[moved_out]), seeds(&[], &[f.column, f.plain]), "a move seeds the parent it left and the one it joins");
    }

    #[test]
    fn a_removal_inside_a_removed_subtree_seeds_the_nearest_parent_that_stays() {
        let f = fixture();
        let page = f.doc.get(f.column).unwrap().parent.unwrap();
        let cmds = [Command::Remove { id: f.column }, Command::Remove { id: f.in_column }];
        assert_eq!(seeds_after_commands(&f.doc, &cmds), seeds(&[], &[page]));
    }

    #[test]
    fn a_layer_outside_the_flow_seeds_no_parent_when_inserted_or_moved() {
        let mut f = fixture();
        let absolute_id = f.doc.alloc_id();
        let mut absolute = Node::rectangle(absolute_id, Rect::new(0.0, 0.0, 10.0, 10.0));
        absolute.layout_child.absolute = true;
        let insert = Command::Insert { parent: f.column, index: 1, nodes: vec![absolute] };
        assert_eq!(seeds_after_commands(&f.doc, &[insert]), seeds(&[], &[absolute_id]), "only the inserted root itself, and its home frame stays out");

        let flow_id = f.doc.alloc_id();
        let flow = Command::Insert { parent: f.column, index: 1, nodes: vec![Node::rectangle(flow_id, Rect::new(0.0, 0.0, 10.0, 10.0))] };
        assert_eq!(seeds_after_commands(&f.doc, &[flow]), seeds(&[flow_id], &[]), "a flow child is a leaf, so its parent runs with it");

        f.doc.get_mut(f.in_column).unwrap().layout_child.absolute = true;
        let moved = Command::Reparent { id: f.in_column, parent: f.plain, index: 0 };
        assert_eq!(seeds_after_commands(&f.doc, &[moved]), seeds(&[], &[]));
    }
}
