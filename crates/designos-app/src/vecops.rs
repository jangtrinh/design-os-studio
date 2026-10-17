//! Object-level vector commands: flatten, outline stroke, boolean groups.
//! Each is one undo step. The geometry comes from `designos_render::vectorops`.

use crate::app::App;
use crate::edit::Tx;
use crate::util::*;
use designos_doc::VectorNetwork;
use designos_doc::*;
use designos_render::vectorops::{count_subpaths, flatten_nodes, node_paths, offset_paths, outline_stroke};

/// A new Vector node holding `paths` (in the coordinates of the parent it will live in) with the
/// appearance of `template`. The node box is fitted to the geometry.
pub fn vector_node(id: NodeId, paths: &[Path], template: &Node) -> Node {
    let mut net = VectorNetwork::from_paths(paths);
    let bb = net.bounds();
    net.translate(v2(-bb.x, -bb.y));
    let mut n = Node::new(id, NodeKind::Vector { paths: net.to_paths(), network: Some(net) });
    n.transform = template.transform * Affine::translate(bb.x, bb.y);
    n.size = v2(bb.w.max(1.0), bb.h.max(1.0));
    n.fills = template.fills.clone();
    n.stroke = template.stroke.clone();
    n.effects = template.effects.clone();
    n.opacity = template.opacity;
    n.blend = if template.blend == BlendMode::PassThrough { BlendMode::Normal } else { template.blend };
    n.constraints = template.constraints;
    n.layout_child = template.layout_child;
    n.name = template.name.clone();
    n
}

/// Tolerance (in node units) of Simplify path.
pub const SIMPLIFY_TOLERANCE: f32 = 1.0;

/// Whether a node can be merged or outlined: it has geometry of its own.
fn has_geometry(app: &App, id: NodeId) -> bool {
    app.doc.get(id).is_some_and(|n| !matches!(n.kind, NodeKind::Text(_) | NodeKind::Page { .. } | NodeKind::Unknown { .. }))
}

/// Whether Offset path may change `id`: a basic shape or vector with no layers inside. Frames,
/// sections, groups, boolean groups, text and board items are not, because a node that becomes a
/// vector has neither its children nor its kind-specific data any more.
pub fn is_offsettable(app: &App, id: NodeId) -> bool {
    crate::canvas::vector::is_vector(app, id) && app.doc.get(id).is_some_and(|n| n.children.is_empty())
}

/// Stretches the geometry of vector `id` to its box when a layout pass gave the box another size
/// (a fixed minimum or maximum, or a fill slot), so the drawn shape never spills out of its box.
fn fit_geometry_to_box(doc: &mut Document, ed: &mut crate::edit::Edit, id: NodeId) {
    let Some(node) = doc.get(id) else { return };
    let NodeKind::Vector { network: Some(net), .. } = &node.kind else { return };
    let bb = net.bounds();
    let have = v2(bb.w.max(1.0), bb.h.max(1.0));
    if (node.size.x - have.x).abs() < 1e-3 && (node.size.y - have.y).abs() < 1e-3 {
        return;
    }
    let fitted = net.transformed(&Affine::scale(node.size.x / have.x, node.size.y / have.y));
    if let Some(node) = ed.node(doc, id) {
        node.kind = NodeKind::Vector { paths: fitted.to_paths(), network: Some(fitted) };
    }
}

fn linked(app: &App, id: NodeId) -> bool {
    app.doc
        .get(id)
        .is_some_and(|n| matches!(n.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_) | NodeKind::Instance { .. }) || n.component.source.is_some())
}

impl App {
    /// Ctrl+E: merges the selection (or a boolean group) into one vector with the computed geometry.
    pub fn flatten_selection(&mut self) {
        if self.canvas.vedit.is_some() {
            crate::canvas::vector::exit(self);
        }
        let ids: Vec<NodeId> = self.same_parent_tops().into_iter().filter(|i| has_geometry(self, *i)).collect();
        if ids.is_empty() {
            self.show_toast("Select a layer with a shape to flatten", false);
            return;
        }
        if ids.iter().any(|i| linked(self, *i)) {
            self.show_toast("Detach the instance or component before flattening", true);
            return;
        }
        let Some(parent) = self.doc.get(ids[0]).and_then(|n| n.parent) else { return };
        let Some(flat) = flatten_nodes(&self.doc, &ids, &self.doc.world_transform(parent)) else {
            self.show_toast("Nothing to flatten", false);
            return;
        };
        let Some(template) = self.doc.get(ids[flat.style_from]).cloned() else { return };
        let siblings = self.doc.get(parent).map(|p| p.children.clone()).unwrap_or_default();
        let index = ids.iter().filter_map(|i| siblings.iter().position(|c| c == i)).max().map(|i| i + 1).unwrap_or(siblings.len());
        let id = self.doc.alloc_id();
        // paths are in the parent's space already, so the template contributes no transform
        let mut base = template.clone();
        base.transform = Affine::IDENTITY;
        let mut node = vector_node(id, &flat.paths, &base);
        node.name = if ids.len() == 1 { template.name.clone() } else { self.unique_name("Vector") };
        node.component = ComponentLink::default();
        let mut tx = Tx::new();
        if tx.cmd(&mut self.doc, Command::Insert { parent, index, nodes: vec![node] }).is_err() {
            return;
        }
        for i in &ids {
            let _ = tx.cmd(&mut self.doc, Command::Remove { id: *i });
        }
        crate::layout::relayout_around(&mut self.doc, &mut tx.ed, parent);
        self.run_tx(tx);
        self.selection = vec![id];
    }

    /// Ctrl+Alt+O: turns each selected layer's stroke into filled geometry.
    pub fn outline_stroke_selection(&mut self) {
        if self.canvas.vedit.is_some() {
            crate::canvas::vector::exit(self);
        }
        let tops = self.top_selection();
        let mut tx = Tx::new();
        let mut made = vec![];
        for id in tops {
            let Some(n) = self.doc.get(id).cloned() else { continue };
            let visible_paints = n.stroke.paints.iter().any(|p| p.visible);
            if !visible_paints || n.stroke.weight <= 0.0 {
                continue;
            }
            let Some(paths) = node_paths(&self.doc, &n) else { continue };
            let net = match &n.kind {
                NodeKind::Vector { network: Some(net), .. } => Some(net.clone()),
                _ => None,
            };
            let outline = outline_stroke(&paths, &n.stroke, net.as_ref());
            if outline.is_empty() {
                continue;
            }
            let Some(parent) = n.parent else { continue };
            let Some(index) = self.doc.get(parent).and_then(|p| p.children.iter().position(|c| *c == id)) else { continue };
            // the stroke paints become the fills of the outline
            let mut template = n.clone();
            template.fills = n.stroke.paints.clone();
            template.stroke.paints.clear();
            template.effects.clear();
            let keeps_fill = n.fills.iter().any(|p| p.visible) || !n.children.is_empty();
            let vid = self.doc.alloc_id();
            let mut node = vector_node(vid, &outline, &template);
            node.name = if keeps_fill { self.unique_name(&format!("{} stroke", n.name)) } else { n.name.clone() };
            node.component = ComponentLink::default();
            let at = if keeps_fill { index + 1 } else { index };
            if tx.cmd(&mut self.doc, Command::Insert { parent, index: at, nodes: vec![node] }).is_err() {
                continue;
            }
            if keeps_fill {
                // the shape keeps its fill; only the stroke moved into the outline
                if let Some(orig) = tx.ed.node(&mut self.doc, id) {
                    orig.stroke.paints.clear();
                }
            } else {
                let _ = tx.cmd(&mut self.doc, Command::Remove { id });
            }
            made.push(vid);
        }
        if made.is_empty() {
            self.show_toast("Select a layer with a stroke to outline", false);
            tx.cancel(&mut self.doc);
            return;
        }
        self.run_tx(tx);
        self.selection = made;
    }

    /// Offsets the closed contours of each selected shape outward (`delta > 0.0`) or inward
    /// (`delta < 0.0`) using Figma's default round join semantics.
    pub fn offset_path_selection(&mut self, delta: f32) {
        self.offset_path_selection_with(delta, StrokeJoin::Round, 4.0);
    }

    /// Offsets the closed contours of each selected shape outward (`delta > 0.0`) or inward
    /// (`delta < 0.0`) with an explicit join and miter limit as a single Undo step.
    ///
    /// Only plain shapes are offset (see [`is_offsettable`]): a layer that holds other layers or
    /// carries board data is left alone, since turning it into a vector would drop them. Each
    /// offset layer keeps its id and every property but its geometry, so masks, prototype links
    /// and instances keep pointing at it. Open sub-paths are kept as they were, a shape that an
    /// inset would remove is left unchanged, and the layout around a grown or shrunk shape is
    /// re-solved in the same step.
    pub fn offset_path_selection_with(&mut self, delta: f32, join: StrokeJoin, miter_limit: f32) {
        if !delta.is_finite() || delta.abs() < 1e-5 {
            return;
        }
        if self.canvas.vedit.is_some() {
            crate::canvas::vector::exit(self);
        }
        let tops = self.top_selection();
        if tops.iter().any(|i| linked(self, *i)) {
            self.show_toast("Detach the instance or component before offsetting", true);
            return;
        }
        let mut tx = Tx::new();
        let mut made = vec![];
        let (mut skipped_layers, mut open_only, mut open_kept, mut collapsed) = (false, false, false, false);
        for id in tops {
            if !is_offsettable(self, id) {
                skipped_layers = true;
                continue;
            }
            let Some(n) = self.doc.get(id).cloned() else { continue };
            let Some(paths) = node_paths(&self.doc, &n) else { continue };
            let (closed, open) = count_subpaths(&paths);
            if closed == 0 {
                // nothing with an inside to grow or shrink
                open_only |= open > 0;
                continue;
            }
            let offset = offset_paths(&paths, delta, join, miter_limit);
            if offset.is_empty() {
                collapsed = true;
                continue;
            }
            // the layer stays the same node (same id, name, mask, prototype links, component link,
            // blend, ...) and only its geometry changes
            let mut net = VectorNetwork::from_paths(&offset);
            let bb = net.bounds();
            net.translate(v2(-bb.x, -bb.y));
            let Some(node) = tx.ed.node(&mut self.doc, id) else { continue };
            node.kind = NodeKind::Vector { paths: net.to_paths(), network: Some(net) };
            node.transform = n.transform * Affine::translate(bb.x, bb.y);
            node.size = v2(bb.w.max(1.0), bb.h.max(1.0));
            node.fill_geometry = None;
            node.stroke_geometry = None;
            node.corner_radii = [0.0; 4];
            node.corner_smoothing = 0.0;
            // the box has another size: siblings in an auto-layout row and fitted groups follow
            crate::layout::relayout_around(&mut self.doc, &mut tx.ed, id);
            open_kept |= open > 0;
            made.push(id);
        }
        // only now: a later layer's reflow can still change an earlier one's slot
        for id in &made {
            fit_geometry_to_box(&mut self.doc, &mut tx.ed, *id);
        }
        let notes: Vec<&str> = [
            (skipped_layers, "Skipped frames, groups, text and board items: only plain shapes can be offset"),
            (open_only || open_kept, "Open paths cannot be offset and were left as they were"),
            (collapsed, "The offset collapsed the shape, so it was left as it was"),
        ]
        .into_iter()
        .filter_map(|(on, text)| on.then_some(text))
        .collect();
        if made.is_empty() {
            self.show_toast(if notes.is_empty() { "Select a shape to offset".to_string() } else { notes.join(". ") }, false);
            tx.cancel(&mut self.doc);
            return;
        }
        if self.run_tx(tx) && !notes.is_empty() {
            self.show_toast(notes.join(". "), false);
        }
        self.selection = made;
    }

    /// Reduces the nodes of every selected vector within a small tolerance.
    pub fn simplify_selection(&mut self) {
        if self.canvas.vedit.is_some() {
            let removed = crate::canvas::vector::simplify(self, SIMPLIFY_TOLERANCE);
            if removed == 0 {
                self.show_toast("Nothing to simplify", false);
            }
            return;
        }
        let mut tx = Tx::new();
        let mut removed = 0;
        for id in self.top_selection() {
            let Some(n) = self.doc.get(id) else { continue };
            if !matches!(n.kind, NodeKind::Vector { .. }) {
                continue;
            }
            let Some(mut net) = crate::canvas::vector::network_of(self, n) else { continue };
            let r = net.simplify(SIMPLIFY_TOLERANCE);
            if r == 0 {
                continue;
            }
            removed += r;
            let bb = net.bounds();
            net.translate(v2(-bb.x, -bb.y));
            if let Some(node) = tx.ed.node(&mut self.doc, id) {
                node.transform = node.transform * Affine::translate(bb.x, bb.y);
                node.size = v2(bb.w.max(1.0), bb.h.max(1.0));
                node.kind = NodeKind::Vector { paths: net.to_paths(), network: Some(net) };
                node.fill_geometry = None;
                node.stroke_geometry = None;
            }
        }
        if removed == 0 {
            self.show_toast("Nothing to simplify", false);
            return;
        }
        self.run_tx(tx);
        self.show_toast(format!("Removed {removed} nodes"), false);
    }

    /// Wraps the selected layers into a boolean group (Union, Subtract, Intersect, Exclude).
    pub fn boolean_selection(&mut self, op: BooleanOp) {
        let ids: Vec<NodeId> = self.same_parent_tops().into_iter().filter(|i| has_geometry(self, *i)).collect();
        if ids.len() < 2 {
            self.show_toast("Select two or more layers to combine", false);
            return;
        }
        let mut tx = Tx::new();
        let first = self.doc.get(ids[0]).cloned();
        let Some(g) = self.wrap_in_kind(&mut tx, &ids, NodeKind::BooleanOp(op)) else { return };
        // the group is painted with the appearance of its bottom layer
        if let (Some(first), Some(gn)) = (first, tx.ed.node(&mut self.doc, g)) {
            gn.fills = first.fills.clone();
            gn.stroke = first.stroke.clone();
            gn.effects = first.effects.clone();
            gn.name = match op {
                BooleanOp::Union => "Union",
                BooleanOp::Subtract => "Subtract",
                BooleanOp::Intersect => "Intersect",
                BooleanOp::Exclude => "Exclude",
            }
            .to_string();
        }
        self.run_tx(tx);
        self.selection = vec![g];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_render::vectorops::paths_area;

    #[test]
    fn offset_path_selection_expands_and_shrinks_square_with_undo() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let rect_id = doc.alloc_id();
        let mut rect = Node::rectangle(rect_id, Rect::new(20.0, 30.0, 100.0, 100.0));
        rect.name = "Square".into();
        doc.insert(page, rect).unwrap();

        let mut app = App::new(doc);
        app.page = page;
        app.selection = vec![rect_id];

        app.offset_path_selection(10.0);
        assert_eq!(app.selection.len(), 1);
        let expanded_id = app.selection[0];
        let expanded = app.doc.get(expanded_id).unwrap();
        assert!((expanded.size.x - 120.0).abs() <= 0.5);
        assert!((expanded.size.y - 120.0).abs() <= 0.5);
        let expanded_paths = node_paths(&app.doc, expanded).unwrap();
        let expected_round = 120.0 * 120.0 - 4.0 * (100.0 - std::f64::consts::PI * 25.0);
        assert!((paths_area(&expanded_paths, 0.02) - expected_round).abs() <= 2.0);

        // Undo restores the 100x100 rectangle, then -10 shrinks it to 80x80
        app.undo();
        app.selection = vec![rect_id];
        app.offset_path_selection(-10.0);
        let shrunk_id = app.selection[0];
        let shrunk = app.doc.get(shrunk_id).unwrap();
        assert!((shrunk.size.x - 80.0).abs() <= 0.5);
        assert!((shrunk.size.y - 80.0).abs() <= 0.5);
        let shrunk_paths = node_paths(&app.doc, shrunk).unwrap();
        assert!((paths_area(&shrunk_paths, 0.02) - 6400.0).abs() <= 1.5);
    }

    fn app_on_new_page(doc: Document, page: NodeId) -> App {
        let mut app = App::new(doc);
        app.page = page;
        app
    }

    fn assert_offset_refused(make: impl FnOnce(NodeId) -> Node, with_kid: bool) {
        offset_and_assert_untouched(make, with_kid, 10.0);
    }

    /// Offsets `target` (holding a rectangle `kid` when `with_kid`) by `delta` and asserts nothing
    /// changed. Returns the text of the toast the user is shown.
    fn offset_and_assert_untouched(make: impl FnOnce(NodeId) -> Node, with_kid: bool, delta: f32) -> String {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let target = doc.alloc_id();
        doc.insert(page, make(target)).unwrap();
        let kid = doc.alloc_id();
        if with_kid {
            doc.insert(target, Node::rectangle(kid, Rect::new(10.0, 10.0, 50.0, 50.0))).unwrap();
        }
        let before: Vec<Option<Node>> = [page, target, kid].iter().map(|i| doc.get(*i).cloned()).collect();
        let mut app = app_on_new_page(doc, page);
        app.selection = vec![target];

        app.offset_path_selection(delta);

        let after: Vec<Option<Node>> = [page, target, kid].iter().map(|i| app.doc.get(*i).cloned()).collect();
        assert!(after == before, "nothing in the document may change: the page now lists {:?}", app.doc.get(page).map(|p| p.children.clone()));
        assert_eq!(app.hist.undo_depth(), 0, "a refused offset is not an undo step");
        assert_eq!(app.selection, vec![target]);
        app.toast.as_ref().expect("the user is told why nothing happened").text.clone()
    }

    #[test]
    fn offset_path_never_replaces_a_frame_that_holds_layers() {
        assert_offset_refused(|id| Node::frame(id, Rect::new(0.0, 0.0, 200.0, 200.0)), true);
    }

    #[test]
    fn offset_path_never_replaces_a_section_that_holds_layers() {
        assert_offset_refused(
            |id| {
                let mut n = Node::new(id, NodeKind::Section);
                n.size = v2(200.0, 200.0);
                n
            },
            true,
        );
    }

    #[test]
    fn offset_path_keeps_a_sticky_note_and_its_text() {
        assert_offset_refused(
            |id| {
                let mut props = StickyProps::default();
                props.text.content = "Remember the milk".into();
                let mut n = Node::new(id, NodeKind::Sticky(props));
                n.size = v2(100.0, 100.0);
                n
            },
            false,
        );
    }

    #[test]
    fn offset_path_keeps_a_group_and_its_layers() {
        assert_offset_refused(|id| Node::new(id, NodeKind::Group), true);
    }

    #[test]
    fn offset_path_leaves_an_open_line_as_it_was() {
        assert_offset_refused(
            |id| {
                let mut n = Node::new(id, NodeKind::Line);
                n.size = v2(100.0, 0.0);
                n.stroke.paints = vec![Paint::solid(Color::rgb(0, 0, 0))];
                n.stroke.weight = 2.0;
                n
            },
            false,
        );
    }

    /// A vector made of a 100 x 100 filled square and an open 100 px segment 200 px to its right.
    fn square_with_open_tail(id: NodeId) -> Node {
        let region = Path {
            cmds: vec![
                PathCmd::MoveTo(v2(0.0, 0.0)),
                PathCmd::LineTo(v2(100.0, 0.0)),
                PathCmd::LineTo(v2(100.0, 100.0)),
                PathCmd::LineTo(v2(0.0, 100.0)),
                PathCmd::Close,
            ],
            fill_rule: FillRule::NonZero,
        };
        let tail = Path { cmds: vec![PathCmd::MoveTo(v2(200.0, 0.0)), PathCmd::LineTo(v2(300.0, 0.0))], fill_rule: FillRule::NonZero };
        let template = Node::rectangle(NodeId(0), Rect::new(0.0, 0.0, 300.0, 100.0));
        vector_node(id, &[region, tail], &template)
    }

    #[test]
    fn offset_path_inset_keeps_the_open_tail_of_a_vector() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let vid = doc.alloc_id();
        doc.insert(page, square_with_open_tail(vid)).unwrap();
        let before = doc.get(vid).cloned();
        let mut app = app_on_new_page(doc, page);
        app.selection = vec![vid];

        app.offset_path_selection(-10.0);

        assert_eq!(app.selection.len(), 1);
        let made = app.doc.get(app.selection[0]).unwrap();
        let paths = node_paths(&app.doc, made).unwrap();
        let mut open = vec![];
        for p in &paths {
            let mut starts = p.cmds.iter().enumerate().filter(|(_, c)| matches!(c, PathCmd::MoveTo(_))).map(|(i, _)| i).collect::<Vec<_>>();
            starts.push(p.cmds.len());
            for w in starts.windows(2) {
                let sub = &p.cmds[w[0]..w[1]];
                if sub.len() > 1 && !matches!(sub.last(), Some(PathCmd::Close)) {
                    open.push(sub.to_vec());
                }
            }
        }
        assert_eq!(open.len(), 1, "the open tail must survive the inset: {paths:?}");
        let (PathCmd::MoveTo(a), Some(PathCmd::LineTo(b))) = (open[0][0], open[0].last().copied()) else { panic!("{:?}", open[0]) };
        assert!((b.x - a.x - 100.0).abs() < 1e-3 && (b.y - a.y).abs() < 1e-3, "the tail keeps its length: {:?}", open[0]);
        assert!((paths_area(&paths, 0.02) - 6400.0).abs() <= 1.5, "the region shrank to 80x80");
        assert!(app.toast.as_ref().is_some_and(|t| t.text.contains("Open paths")), "the user is told the open part was left as it was");
        assert_eq!(app.hist.undo_depth(), 1);
        app.undo();
        assert!(app.doc.get(vid).cloned() == before, "one undo brings the original vector back");
    }

    #[test]
    fn offset_path_that_collapses_the_shape_leaves_the_node_as_it_was() {
        let rectangle = |id| Node::rectangle(id, Rect::new(0.0, 0.0, 100.0, 100.0));
        let alone = offset_and_assert_untouched(rectangle, false, -60.0);
        assert!(alone.contains("collapsed"), "{alone}");
        // the open segment must not stand in for the shape that vanished
        let with_tail = offset_and_assert_untouched(square_with_open_tail, false, -60.0);
        assert!(with_tail.contains("collapsed"), "{with_tail}");
    }

    #[test]
    fn offset_path_keeps_the_node_and_everything_but_its_geometry() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let (rid, other) = (doc.alloc_id(), doc.alloc_id());
        doc.insert(page, Node::frame(other, Rect::new(300.0, 0.0, 200.0, 200.0))).unwrap();
        let mut rect = Node::rectangle(rid, Rect::new(20.0, 30.0, 100.0, 100.0));
        rect.name = "Mask".into();
        rect.is_mask = true;
        rect.locked = true;
        rect.visible = false;
        rect.opacity = 0.5;
        rect.corner_radii = [8.0; 4];
        rect.hyperlink = Some("https://example.com".into());
        rect.plugin_data.insert("plugin:key".into(), "value".into());
        rect.style_refs.insert("fill".into(), "S:1".into());
        rect.export_settings.push(ExportPreset { format: ExportFileFormat::Svg, constraint: ExportConstraint::Width(512.0), suffix: "@512".into() });
        rect.prototype.reactions.push(Reaction {
            trigger: Trigger::OnClick,
            actions: vec![designos_doc::Action::Navigate { dest: other, preserve_scroll: false }],
            transition: None,
        });
        doc.insert(page, rect).unwrap();
        let before = doc.get(rid).cloned().unwrap();
        let mut app = app_on_new_page(doc, page);
        app.selection = vec![rid];

        app.offset_path_selection(10.0);

        assert_eq!(app.selection, vec![rid], "the layer keeps its id, so references to it stay valid");
        let after = app.doc.get(rid).expect("the layer is still there").clone();
        assert!(matches!(after.kind, NodeKind::Vector { .. }));
        assert!((after.size.x - 120.0).abs() <= 0.5, "{:?}", after.size);
        let mut expected = before.clone();
        expected.kind = after.kind.clone();
        expected.transform = after.transform;
        expected.size = after.size;
        expected.corner_radii = [0.0; 4];
        assert!(after == expected, "only kind, transform, size and the baked corner radii may differ:\n{after:?}\nvs\n{expected:?}");
        assert!(after.is_mask && after.locked && !after.visible && after.blend == before.blend);
        assert_eq!(app.hist.undo_depth(), 1);

        app.undo();
        assert!(app.doc.get(rid).cloned() == Some(before), "one undo restores the rectangle exactly");
    }

    #[test]
    fn offset_path_in_a_main_component_keeps_instances_linked_to_the_same_layer() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let (fid, rid) = (doc.alloc_id(), doc.alloc_id());
        doc.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 200.0, 200.0))).unwrap();
        doc.insert(fid, Node::rectangle(rid, Rect::new(20.0, 20.0, 100.0, 100.0))).unwrap();
        let mut app = app_on_new_page(doc, page);
        app.selection = vec![fid];
        app.create_component();
        let comp = app.selection[0];
        app.insert_instance(comp, v2(600.0, 100.0)).expect("an instance");
        let mirror = |app: &App| app.doc.nodes.values().find(|n| n.component.source == Some(rid)).cloned();
        assert!(mirror(&app).is_some_and(|n| matches!(n.kind, NodeKind::Rectangle)), "the instance mirrors the rectangle");

        app.selection = vec![rid];
        app.offset_path_selection(10.0);

        assert!(matches!(app.doc.get(rid).map(|n| &n.kind), Some(NodeKind::Vector { .. })), "the main component keeps the layer");
        let mirrored = mirror(&app).expect("the instance still points at the layer by its id");
        assert!(matches!(mirrored.kind, NodeKind::Vector { .. }), "and follows the offset");
    }

    #[test]
    fn offset_path_fits_every_shape_to_its_final_slot_when_siblings_reflow_each_other() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let (_, a, b) = auto_layout_row(&mut doc, page, Sizing::Fixed);
        // `a` fills what `b` leaves free, so growing `b` after `a` changes `a`'s slot again
        doc.get_mut(a).unwrap().layout_child.horizontal = Sizing::Fill;
        let mut app = app_on_new_page(doc, page);
        app.selection = vec![a, b];

        app.offset_path_selection(10.0);

        for id in &app.selection {
            let node = app.doc.get(*id).unwrap();
            let ink = designos_doc::vector::paths_bounds(&node_paths(&app.doc, node).unwrap());
            assert!(
                (ink.w - node.size.x).abs() < 0.5 && (ink.h - node.size.y).abs() < 0.5,
                "the drawn shape must fill its layer box: ink {ink:?} in box {:?}",
                node.size
            );
        }
        assert!((app.doc.get(a).unwrap().size.x - 270.0).abs() < 1e-3, "400 - 10 gap - 120 for the grown sibling");
    }

    #[test]
    fn offset_path_fits_the_shape_to_a_layout_slot_that_clamps_its_size() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let (_, a, _) = auto_layout_row(&mut doc, page, Sizing::Fixed);
        doc.get_mut(a).unwrap().layout_child.max_w = Some(100.0);
        let mut app = app_on_new_page(doc, page);
        app.selection = vec![a];

        app.offset_path_selection(10.0);

        let node = app.doc.get(app.selection[0]).unwrap();
        assert!((node.size.x - 100.0).abs() < 1e-3, "the slot is clamped to 100: {:?}", node.size);
        let ink = designos_doc::vector::paths_bounds(&node_paths(&app.doc, node).unwrap());
        assert!(
            ink.x.abs() < 0.5 && ink.y.abs() < 0.5 && (ink.w - node.size.x).abs() < 0.5 && (ink.h - node.size.y).abs() < 0.5,
            "the drawn shape must fill the layer box and not spill into its sibling: ink {ink:?} in box {:?}",
            node.size
        );
    }

    fn auto_layout_row(doc: &mut Document, page: NodeId, sizing: Sizing) -> (NodeId, NodeId, NodeId) {
        let fid = doc.alloc_id();
        let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 400.0, 200.0));
        if let Some(fp) = frame.kind.frame_props_mut() {
            fp.auto_layout = Some(AutoLayout { item_spacing: 10.0, padding: [0.0; 4], primary_sizing: sizing, counter_sizing: sizing, ..Default::default() });
        }
        doc.insert(page, frame).unwrap();
        let (a, b) = (doc.alloc_id(), doc.alloc_id());
        doc.insert(fid, Node::rectangle(a, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
        doc.insert(fid, Node::rectangle(b, Rect::new(110.0, 0.0, 100.0, 100.0))).unwrap();
        (fid, a, b)
    }

    #[test]
    fn offset_path_reflows_the_auto_layout_siblings_in_the_same_undo_step() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let (_, a, b) = auto_layout_row(&mut doc, page, Sizing::Fixed);
        let mut app = app_on_new_page(doc, page);
        app.selection = vec![a];

        app.offset_path_selection(10.0);

        let grown = app.doc.get(app.selection[0]).unwrap();
        assert!((grown.size.x - 120.0).abs() <= 0.5, "{:?}", grown.size);
        assert!(grown.transform.tx.abs() < 1e-3, "the grown shape sits at the padding edge: {:?}", grown.transform);
        let next = app.doc.get(b).unwrap();
        assert!((next.transform.tx - 130.0).abs() < 1e-3, "120 wide + 10 gap pushes the next layer to 130, got {}", next.transform.tx);
        assert_eq!(app.hist.undo_depth(), 1);

        app.undo();
        assert!((app.doc.get(b).unwrap().transform.tx - 110.0).abs() < 1e-3, "one undo restores the sibling");
        assert!(app.doc.get(a).is_some_and(|n| matches!(n.kind, NodeKind::Rectangle) && n.size == v2(100.0, 100.0)), "and the original rectangle");
    }

    #[test]
    fn offset_path_refits_a_hugging_auto_layout_parent() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let (fid, a, _) = auto_layout_row(&mut doc, page, Sizing::Hug);
        let mut app = app_on_new_page(doc, page);
        app.selection = vec![a];

        app.offset_path_selection(10.0);

        let frame = app.doc.get(fid).unwrap();
        assert!((frame.size.x - 230.0).abs() < 1e-3 && (frame.size.y - 120.0).abs() < 1e-3, "120 + 10 + 100 by 120, got {:?}", frame.size);
    }

    #[test]
    fn offset_path_refits_the_group_that_holds_the_shape() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let gid = doc.alloc_id();
        let mut group = Node::new(gid, NodeKind::Group);
        group.transform = Affine::translate(50.0, 50.0);
        group.size = v2(250.0, 100.0);
        doc.insert(page, group).unwrap();
        let (a, b) = (doc.alloc_id(), doc.alloc_id());
        doc.insert(gid, Node::rectangle(a, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
        doc.insert(gid, Node::rectangle(b, Rect::new(150.0, 0.0, 100.0, 100.0))).unwrap();
        let mut app = app_on_new_page(doc, page);
        app.selection = vec![a];

        app.offset_path_selection(10.0);

        let group = app.doc.get(gid).unwrap();
        assert!((group.size.x - 260.0).abs() < 1e-3 && (group.size.y - 120.0).abs() < 1e-3, "the group follows the grown child, got {:?}", group.size);
    }
}
