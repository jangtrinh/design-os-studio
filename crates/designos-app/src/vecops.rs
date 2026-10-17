//! Object-level vector commands: flatten, outline stroke, boolean groups.
//! Each is one undo step. The geometry comes from `designos_render::vectorops`.

use crate::app::App;
use crate::edit::Tx;
use crate::util::*;
use designos_doc::VectorNetwork;
use designos_doc::*;
use designos_render::vectorops::{flatten_nodes, node_paths, offset_paths, outline_stroke};

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

    /// Offsets each selected shape's paths outward (`delta > 0.0`) or inward (`delta < 0.0`)
    /// using Figma's default round join semantics.
    pub fn offset_path_selection(&mut self, delta: f32) {
        self.offset_path_selection_with(delta, StrokeJoin::Round, 4.0);
    }

    /// Offsets each selected shape's paths outward (`delta > 0.0`) or inward (`delta < 0.0`)
    /// with an explicit join and miter limit as a single Undo step.
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
        for id in tops {
            let Some(n) = self.doc.get(id).cloned() else { continue };
            let Some(paths) = node_paths(&self.doc, &n) else { continue };
            let offset = offset_paths(&paths, delta, join, miter_limit);
            if offset.is_empty() {
                continue;
            }
            let Some(parent) = n.parent else { continue };
            let Some(index) = self.doc.get(parent).and_then(|p| p.children.iter().position(|c| *c == id)) else { continue };
            let vid = self.doc.alloc_id();
            let mut node = vector_node(vid, &offset, &n);
            node.name = n.name.clone();
            node.component = ComponentLink::default();
            if tx.cmd(&mut self.doc, Command::Insert { parent, index: index + 1, nodes: vec![node] }).is_err() {
                continue;
            }
            let _ = tx.cmd(&mut self.doc, Command::Remove { id });
            made.push(vid);
        }
        if made.is_empty() {
            self.show_toast("Select a shape to offset", false);
            tx.cancel(&mut self.doc);
            return;
        }
        self.run_tx(tx);
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
}
