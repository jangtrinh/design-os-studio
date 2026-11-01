//! Object-level vector commands: flatten, outline stroke, boolean groups.
//! Each is one undo step. The geometry comes from `designos_render::vectorops`.

use crate::app::App;
use crate::edit::Tx;
use crate::util::*;
use designos_doc::VectorNetwork;
use designos_doc::*;
use designos_render::vectorops::{count_subpaths, flatten_nodes, node_paths, offset_paths, outline_stroke, text_flatten_runs, text_glyph_runs, TextVectorRun};
use std::collections::BTreeSet;

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

/// Whether a node can be merged into a boolean group: it has shape geometry of its own.
fn has_geometry(app: &App, id: NodeId) -> bool {
    app.doc.get(id).is_some_and(|n| !matches!(n.kind, NodeKind::Text(_) | NodeKind::Page { .. } | NodeKind::Unknown { .. }))
}

/// Whether a node can be flattened into vector paths.
fn can_flatten(app: &App, id: NodeId) -> bool {
    app.doc.get(id).is_some_and(|n| !matches!(n.kind, NodeKind::Page { .. } | NodeKind::Unknown { .. }))
}

fn remap_paint_gradients(paints: &mut [Paint], from_size: Vec2, sub_origin: Vec2, sub_size: Vec2) {
    let sx = sub_size.x.max(1e-6);
    let sy = sub_size.y.max(1e-6);
    for p in paints {
        match &mut p.kind {
            PaintKind::LinearGradient { handles, .. }
            | PaintKind::RadialGradient { handles, .. }
            | PaintKind::AngularGradient { handles, .. }
            | PaintKind::DiamondGradient { handles, .. } => {
                for h in handles {
                    h.x = (h.x * from_size.x - sub_origin.x) / sx;
                    h.y = (h.y * from_size.y - sub_origin.y) / sy;
                }
            }
            _ => {}
        }
    }
}

fn carry_paint_bindings(src: &Node, resolved_base_fills: &[Paint], dst: &mut Node, carry_fills: bool, carry_strokes: bool) {
    dst.variables.modes = src.variables.modes.clone();
    let fill_matches = resolved_base_fills.is_empty() || dst.fills == resolved_base_fills;
    for (k, v) in &src.variables.bindings {
        let keep_fill = carry_fills && fill_matches && k.starts_with("fills/");
        let keep_stroke = carry_strokes && k.starts_with("stroke");
        if keep_fill || keep_stroke {
            dst.variables.bindings.insert(k.clone(), v.clone());
        }
    }
    if carry_fills && fill_matches {
        if let Some(fill_ref) = src.style_refs.get("fill") {
            dst.style_refs.insert("fill".into(), fill_ref.clone());
        }
    }
    if carry_strokes {
        if let Some(stroke_ref) = src.style_refs.get("stroke") {
            dst.style_refs.insert("stroke".into(), stroke_ref.clone());
        }
    }
}

fn collect_text_font_requests(doc: &Document, ids: &[NodeId]) -> BTreeSet<designos_fonts::FontRequest> {
    let mut out = BTreeSet::new();
    for id in ids {
        doc.walk(*id, |raw, _| {
            let resolved = resolve_node(doc, raw);
            let NodeKind::Text(tp) = &resolved.kind else { return };
            out.extend(designos_fonts::text_faces(tp));
            for script in designos_fonts::scripts_in(&tp.content) {
                let mut weights = BTreeSet::from([tp.style.font_weight]);
                weights.extend(tp.runs.iter().map(|r| r.style.font_weight));
                for w in weights {
                    out.insert(designos_fonts::FontRequest::new(script.family(), w, false));
                }
            }
        });
    }
    out
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
    fn ensure_text_fonts(&mut self, ids: &[NodeId]) -> bool {
        let requests = collect_text_font_requests(&self.doc, ids);
        if requests.is_empty() {
            return true;
        }
        match self.fonts.ensure_loaded(requests) {
            Ok(()) => true,
            Err(failed) => {
                if let Some(first) = failed.first() {
                    let more = failed.len() - 1;
                    let suffix = if more > 0 { format!(" (+{more} more)") } else { String::new() };
                    self.show_toast(format!("Font unavailable: {first}{suffix}"), true);
                }
                false
            }
        }
    }

    fn text_run_to_single_vector(&mut self, raw: &Node, resolved: &Node, run: &TextVectorRun) -> Node {
        let vid = self.doc.alloc_id();
        let mut template = resolved.clone();
        template.name = raw.name.clone();
        template.fills = run.fills.clone();
        template.stroke = resolved.stroke.clone();
        let paths = std::slice::from_ref(&run.path);
        let bb = VectorNetwork::from_paths(paths).bounds();
        let mut node = vector_node(vid, paths, &template);
        remap_paint_gradients(&mut node.fills, resolved.size, v2(bb.x, bb.y), node.size);
        remap_paint_gradients(&mut node.stroke.paints, resolved.size, v2(bb.x, bb.y), node.size);
        node.component = ComponentLink::default();
        let base_fills = match &resolved.kind {
            NodeKind::Text(tp) => tp.style.fills.as_slice(),
            _ => &[],
        };
        carry_paint_bindings(raw, base_fills, &mut node, run.uses_base_fill, true);
        node
    }

    fn text_runs_to_group(&mut self, raw: &Node, resolved: &Node, runs: &[TextVectorRun]) -> Vec<Node> {
        let gid = self.doc.alloc_id();
        let mut group = Node::new(gid, NodeKind::Group);
        group.name = raw.name.clone();
        group.transform = raw.transform;
        group.size = resolved.size;
        group.opacity = resolved.opacity;
        group.blend = raw.blend;
        group.constraints = raw.constraints;
        group.layout_child = raw.layout_child;
        group.effects = resolved.effects.clone();
        group.variables.modes = raw.variables.modes.clone();
        let base_fills = match &resolved.kind {
            NodeKind::Text(tp) => tp.style.fills.as_slice(),
            _ => &[],
        };
        let mut children_nodes = Vec::with_capacity(runs.len());
        for (idx, run) in runs.iter().enumerate() {
            let cid = self.doc.alloc_id();
            let mut child_template = resolved.clone();
            child_template.transform = Affine::IDENTITY;
            child_template.fills = run.fills.clone();
            child_template.stroke = resolved.stroke.clone();
            child_template.effects.clear();
            child_template.opacity = 1.0;
            child_template.blend = BlendMode::Normal;
            let paths = std::slice::from_ref(&run.path);
            let bb = VectorNetwork::from_paths(paths).bounds();
            let mut child = vector_node(cid, paths, &child_template);
            remap_paint_gradients(&mut child.fills, resolved.size, v2(bb.x, bb.y), child.size);
            remap_paint_gradients(&mut child.stroke.paints, resolved.size, v2(bb.x, bb.y), child.size);
            child.parent = Some(gid);
            child.name = if run.name.trim().is_empty() { format!("Vector {}", idx + 1) } else { run.name.clone() };
            child.component = ComponentLink::default();
            carry_paint_bindings(raw, base_fills, &mut child, run.uses_base_fill, true);
            group.children.push(cid);
            children_nodes.push(child);
        }
        let mut out = Vec::with_capacity(1 + children_nodes.len());
        out.push(group);
        out.extend(children_nodes);
        out
    }

    /// Ctrl+E: merges the selection (or a boolean group) into one vector with the computed geometry.
    pub fn flatten_selection(&mut self) {
        if self.canvas.vedit.is_some() {
            crate::canvas::vector::exit(self);
        }
        let ids: Vec<NodeId> = self.same_parent_tops().into_iter().filter(|i| can_flatten(self, *i)).collect();
        if ids.is_empty() {
            self.show_toast("Select a layer with a shape to flatten", false);
            return;
        }
        if ids.iter().any(|i| linked(self, *i)) {
            self.show_toast("Detach the instance or component before flattening", true);
            return;
        }
        if !self.ensure_text_fonts(&ids) {
            return;
        }
        if ids.len() == 1 && self.doc.get(ids[0]).is_some_and(|n| matches!(n.kind, NodeKind::Text(_))) {
            let id = ids[0];
            let Some(n) = self.doc.get(id).cloned() else { return };
            let Some(parent) = n.parent else { return };
            let Some(index) = self.doc.get(parent).and_then(|p| p.children.iter().position(|c| *c == id)) else { return };
            let runs = text_flatten_runs(&self.doc, &n);
            if runs.is_empty() {
                self.show_toast("Nothing to flatten", false);
                return;
            }
            let resolved = resolve_node(&self.doc, &n).into_owned();
            let nodes =
                if runs.len() == 1 { vec![self.text_run_to_single_vector(&n, &resolved, &runs[0])] } else { self.text_runs_to_group(&n, &resolved, &runs) };
            let out_id = nodes[0].id;
            let mut tx = Tx::new();
            if tx.cmds_all_or_none(&mut self.doc, vec![Command::Remove { id }, Command::Insert { parent, index, nodes }]).is_err() {
                tx.cancel(&mut self.doc);
                return;
            }
            self.run_tx(tx);
            self.selection = vec![out_id];
            return;
        }
        let Some(parent) = self.doc.get(ids[0]).and_then(|n| n.parent) else { return };
        let Some(flat) = flatten_nodes(&self.doc, &ids, &self.doc.world_transform(parent)) else {
            self.show_toast("Nothing to flatten", false);
            return;
        };
        let Some(template) = self.doc.get(ids[flat.style_from]).cloned() else { return };
        let resolved_template = resolve_node(&self.doc, &template).into_owned();
        let siblings = self.doc.get(parent).map(|p| p.children.clone()).unwrap_or_default();
        let index = ids.iter().filter_map(|i| siblings.iter().position(|c| c == i)).max().map(|i| i + 1).unwrap_or(siblings.len());
        let id = self.doc.alloc_id();
        // paths are in the parent's space already, so the template contributes no transform
        let mut base = resolved_template.clone();
        base.transform = Affine::IDENTITY;
        let base_fills = match &resolved_template.kind {
            NodeKind::Text(tp) => {
                base.fills = tp.runs.first().map(|r| r.style.fills.clone()).unwrap_or_else(|| tp.style.fills.clone());
                tp.style.fills.as_slice()
            }
            _ => resolved_template.fills.as_slice(),
        };
        let mut node = vector_node(id, &flat.paths, &base);
        node.name = if ids.len() == 1 { template.name.clone() } else { self.unique_name("Vector") };
        node.component = ComponentLink::default();
        carry_paint_bindings(&template, base_fills, &mut node, true, true);
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

    /// Ctrl+Alt+O: turns each selected layer's stroke into filled geometry, or converts selected text layers into per-glyph vector outlines.
    pub fn outline_stroke_selection(&mut self) {
        if self.canvas.vedit.is_some() {
            crate::canvas::vector::exit(self);
        }
        let tops = self.top_selection();
        if !self.ensure_text_fonts(&tops) {
            return;
        }
        let mut tx = Tx::new();
        let mut made = vec![];
        for id in tops {
            let Some(n) = self.doc.get(id).cloned() else { continue };
            if matches!(n.kind, NodeKind::Text(_)) {
                if linked(self, id) {
                    continue;
                }
                let runs = text_glyph_runs(&self.doc, &n);
                if runs.is_empty() {
                    continue;
                }
                let Some(parent) = n.parent else { continue };
                let Some(index) = self.doc.get(parent).and_then(|p| p.children.iter().position(|c| *c == id)) else { continue };
                let resolved = resolve_node(&self.doc, &n).into_owned();
                let nodes = self.text_runs_to_group(&n, &resolved, &runs);
                let gid = nodes[0].id;
                if tx.cmds_all_or_none(&mut self.doc, vec![Command::Remove { id }, Command::Insert { parent, index, nodes }]).is_err() {
                    continue;
                }
                made.push(gid);
                continue;
            }
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

    fn two_run_text_node(id: NodeId, red: Color, green: Color) -> Node {
        let base_style = TextStyle { font_family: "Inter".into(), font_weight: 400, font_size: 24.0, fills: vec![Paint::solid(red)], ..Default::default() };
        let green_style = TextStyle { fills: vec![Paint::solid(green)], ..base_style.clone() };
        let mut t = Node::text(id, Rect::new(24.0, 36.0, 160.0, 40.0), "AB cd", base_style.clone());
        t.name = "Headline".into();
        if let NodeKind::Text(tp) = &mut t.kind {
            tp.runs = vec![TextRun { start: 0, end: 3, style: base_style }, TextRun { start: 3, end: 5, style: green_style }];
        }
        t
    }

    #[test]
    fn outline_stroke_of_two_run_text_gives_one_vector_per_glyph_with_each_runs_fill() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let (before_id, tid, after_id) = (doc.alloc_id(), doc.alloc_id(), doc.alloc_id());
        doc.insert(page, Node::rectangle(before_id, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        let red = Color::rgb(220, 40, 40);
        let green = Color::rgb(30, 180, 70);
        let mut t = two_run_text_node(tid, red, green);
        t.transform = Affine::translate(24.0, 36.0) * Affine::rotate(0.15);
        let expected_tf = t.transform;
        doc.insert(page, t).unwrap();
        doc.insert(page, Node::rectangle(after_id, Rect::new(200.0, 0.0, 10.0, 10.0))).unwrap();

        let mut app = app_on_new_page(doc, page);
        app.selection = vec![tid];
        app.outline_stroke_selection();

        assert_eq!(app.selection.len(), 1);
        let gid = app.selection[0];
        assert_eq!(app.doc.get(page).unwrap().children, vec![before_id, gid, after_id], "inserted at the text layer's index");
        let group = app.doc.get(gid).unwrap();
        assert!(matches!(group.kind, NodeKind::Group));
        assert_eq!(group.name, "Headline");
        assert_eq!(group.transform, expected_tf, "group preserves the text layer's transform");
        assert_eq!(group.children.len(), 4, "one vector per visible glyph in 'AB cd' (space has no outline)");

        for (idx, &cid) in group.children.iter().enumerate() {
            let child = app.doc.get(cid).unwrap();
            assert!(matches!(child.kind, NodeKind::Vector { .. }));
            let expected_color = if idx < 2 { red } else { green };
            assert_eq!(child.fills, vec![Paint::solid(expected_color)], "glyph {idx} carries its run's fill");
        }
    }

    #[test]
    fn flatten_of_single_paint_text_gives_one_vector() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let (before_id, tid, after_id) = (doc.alloc_id(), doc.alloc_id(), doc.alloc_id());
        doc.insert(page, Node::rectangle(before_id, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        let blue = Color::rgb(20, 80, 220);
        let style = TextStyle { font_family: "Inter".into(), font_size: 20.0, fills: vec![Paint::solid(blue)], ..Default::default() };
        let mut t = Node::text(tid, Rect::new(15.0, 25.0, 120.0, 32.0), "Hello", style);
        t.name = "Greeting".into();
        doc.insert(page, t).unwrap();
        doc.insert(page, Node::rectangle(after_id, Rect::new(200.0, 0.0, 10.0, 10.0))).unwrap();

        let mut app = app_on_new_page(doc, page);
        app.selection = vec![tid];
        app.flatten_selection();

        assert_eq!(app.selection.len(), 1);
        let vid = app.selection[0];
        assert_eq!(app.doc.get(page).unwrap().children, vec![before_id, vid, after_id]);
        let v = app.doc.get(vid).unwrap();
        assert_eq!(v.name, "Greeting");
        assert_eq!(v.fills, vec![Paint::solid(blue)]);
        let NodeKind::Vector { paths, network: Some(net) } = &v.kind else {
            panic!("expected a single Vector node, got {:?}", v.kind);
        };
        assert!(!paths.is_empty());
        assert!(!net.vertices.is_empty());
    }

    #[test]
    fn flatten_of_two_paint_text_gives_two_vectors_in_a_group() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let tid = doc.alloc_id();
        let red = Color::rgb(220, 40, 40);
        let green = Color::rgb(30, 180, 70);
        let t = two_run_text_node(tid, red, green);
        let expected_tf = t.transform;
        doc.insert(page, t).unwrap();

        let mut app = app_on_new_page(doc, page);
        app.selection = vec![tid];
        app.flatten_selection();

        assert_eq!(app.selection.len(), 1);
        let gid = app.selection[0];
        let group = app.doc.get(gid).unwrap();
        assert!(matches!(group.kind, NodeKind::Group));
        assert_eq!(group.name, "Headline");
        assert_eq!(group.transform, expected_tf);
        assert_eq!(group.children.len(), 2, "two distinct paint sets produce two vectors in a group");
        let v0 = app.doc.get(group.children[0]).unwrap();
        let v1 = app.doc.get(group.children[1]).unwrap();
        assert!(matches!(v0.kind, NodeKind::Vector { .. }));
        assert!(matches!(v1.kind, NodeKind::Vector { .. }));
        assert_eq!(v0.fills, vec![Paint::solid(red)]);
        assert_eq!(v1.fills, vec![Paint::solid(green)]);
    }

    #[test]
    fn text_to_vector_carries_over_fill_and_stroke_bindings() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        doc.variable_collections.insert(
            "coll_1".into(),
            VariableCollection {
                id: "coll_1".into(),
                name: "Tokens".into(),
                modes: vec![VariableMode { id: "m1".into(), name: "Default".into() }],
                default_mode: "m1".into(),
                variables: vec!["var_fill".into(), "var_stroke".into(), "var_weight".into()],
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );
        let fill_color = Color::rgb(12, 140, 230);
        let stroke_color = Color::rgb(240, 90, 20);
        for (id, name, kind, val) in [
            ("var_fill", "Color/Fill", VariableKind::Color, VariableValue::Color(fill_color)),
            ("var_stroke", "Color/Stroke", VariableKind::Color, VariableValue::Color(stroke_color)),
            ("var_weight", "Stroke/Weight", VariableKind::Number, VariableValue::Number(2.0)),
        ] {
            doc.variables.insert(
                id.into(),
                Variable {
                    id: id.into(),
                    name: name.into(),
                    collection: "coll_1".into(),
                    kind,
                    values: std::collections::BTreeMap::from([("m1".into(), val)]),
                    description: String::new(),
                    scopes: Vec::new(),
                    hidden_from_publishing: false,
                    key: None,
                    code_syntax: Default::default(),
                    extensions: Default::default(),
                    mode_extensions: Default::default(),
                },
            );
        }

        let tid = doc.alloc_id();
        let style = TextStyle { font_family: "Inter".into(), font_size: 20.0, fills: vec![Paint::solid(Color::BLACK)], ..Default::default() };
        let mut t = Node::text(tid, Rect::new(10.0, 10.0, 80.0, 30.0), "Hi", style);
        t.stroke.paints = vec![Paint::solid(Color::WHITE)];
        t.stroke.weight = 1.0;
        t.variables.bindings.insert("fills/0".into(), "var_fill".into());
        t.variables.bindings.insert("strokes/0".into(), "var_stroke".into());
        t.variables.bindings.insert("strokeWeight".into(), "var_weight".into());
        t.variables.modes.insert("coll_1".into(), "m1".into());
        doc.insert(page, t).unwrap();

        let mut app = app_on_new_page(doc, page);
        app.selection = vec![tid];
        app.flatten_selection();

        let flat_v = app.doc.get(app.selection[0]).unwrap();
        assert_eq!(flat_v.variables.bindings.get("fills/0").map(String::as_str), Some("var_fill"));
        assert_eq!(flat_v.variables.bindings.get("strokes/0").map(String::as_str), Some("var_stroke"));
        assert_eq!(flat_v.variables.bindings.get("strokeWeight").map(String::as_str), Some("var_weight"));
        assert_eq!(flat_v.variables.modes.get("coll_1").map(String::as_str), Some("m1"));
        assert_eq!(flat_v.fills, vec![Paint::solid(fill_color)]);
        assert_eq!(flat_v.stroke.paints, vec![Paint::solid(stroke_color)]);

        app.undo();
        app.selection = vec![tid];
        app.outline_stroke_selection();
        let group = app.doc.get(app.selection[0]).unwrap();
        assert!(!group.children.is_empty());
        for &cid in &group.children {
            let gv = app.doc.get(cid).unwrap();
            assert_eq!(gv.variables.bindings.get("fills/0").map(String::as_str), Some("var_fill"));
            assert_eq!(gv.variables.bindings.get("strokes/0").map(String::as_str), Some("var_stroke"));
            assert_eq!(gv.variables.bindings.get("strokeWeight").map(String::as_str), Some("var_weight"));
        }
    }

    #[test]
    fn one_undo_restores_text_node_exactly_in_json() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let tid = doc.alloc_id();
        let mut t = two_run_text_node(tid, Color::rgb(200, 30, 30), Color::rgb(30, 120, 220));
        t.variables.bindings.insert("fills/0".into(), "var_brand".into());
        doc.insert(page, t).unwrap();
        let before_node_json = serde_json::to_string(doc.get(tid).unwrap()).unwrap();
        let before_page_json = serde_json::to_string(doc.get(page).unwrap()).unwrap();

        let mut app = app_on_new_page(doc, page);
        app.selection = vec![tid];
        app.flatten_selection();
        assert!(app.doc.get(tid).is_none());
        assert_eq!(app.hist.undo_depth(), 1);

        app.undo();
        assert_eq!(serde_json::to_string(app.doc.get(tid).unwrap()).unwrap(), before_node_json);
        assert_eq!(serde_json::to_string(app.doc.get(page).unwrap()).unwrap(), before_page_json);

        app.selection = vec![tid];
        app.outline_stroke_selection();
        assert!(app.doc.get(tid).is_none());
        assert_eq!(app.hist.undo_depth(), 1);

        app.undo();
        assert_eq!(serde_json::to_string(app.doc.get(tid).unwrap()).unwrap(), before_node_json);
        assert_eq!(serde_json::to_string(app.doc.get(page).unwrap()).unwrap(), before_page_json);
    }

    #[test]
    fn text_to_vector_loads_unloaded_font_and_refuses_unavailable_font() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let missing_id = doc.alloc_id();
        let missing_style = TextStyle { font_family: "MissingFont".into(), font_size: 18.0, ..Default::default() };
        doc.insert(page, Node::text(missing_id, Rect::new(0.0, 0.0, 100.0, 30.0), "No fallback", missing_style)).unwrap();

        let mut app = app_on_new_page(doc, page);
        app.fonts = crate::fonts::testkit::serving(&["Vecops Font"]);
        app.selection = vec![missing_id];

        app.flatten_selection();
        assert!(app.doc.get(missing_id).is_some(), "text node must stay untouched when font cannot load");
        assert_eq!(app.hist.undo_depth(), 0);
        let toast = app.toast.as_ref().expect("error toast shown");
        assert!(toast.error && toast.text.starts_with("Font unavailable: MissingFont 400:"), "got toast: {:?}", toast.text);

        app.toast = None;
        app.outline_stroke_selection();
        assert!(app.doc.get(missing_id).is_some());
        assert_eq!(app.hist.undo_depth(), 0);
        let toast = app.toast.as_ref().expect("error toast shown");
        assert!(toast.error && toast.text.starts_with("Font unavailable: MissingFont 400:"));

        // Now test a font that is not yet loaded in `app.fonts` but is available in the font cache/fetcher.
        let loadable_id = app.doc.alloc_id();
        let loadable_style = TextStyle { font_family: "Vecops Font".into(), font_size: 18.0, ..Default::default() };
        app.doc.insert(page, Node::text(loadable_id, Rect::new(0.0, 40.0, 100.0, 30.0), "Loaded!", loadable_style)).unwrap();
        assert!(app.fonts.registered().is_empty(), "font is not loaded before the command runs");

        app.selection = vec![loadable_id];
        app.flatten_selection();
        assert!(!app.fonts.registered().is_empty(), "command loaded the font before outlining");
        assert!(app.doc.get(loadable_id).is_none());
        assert!(matches!(app.doc.get(app.selection[0]).unwrap().kind, NodeKind::Vector { .. }));
    }

    #[test]
    fn mixed_selection_converts_text_and_shapes_together() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let (tid, rid) = (doc.alloc_id(), doc.alloc_id());
        doc.insert(page, Node::text(tid, Rect::new(10.0, 10.0, 60.0, 24.0), "Hi", TextStyle::default())).unwrap();
        let mut rect = Node::rectangle(rid, Rect::new(80.0, 10.0, 40.0, 24.0));
        rect.fills.clear();
        rect.stroke.paints = vec![Paint::solid(Color::rgb(10, 20, 30))];
        rect.stroke.weight = 4.0;
        doc.insert(page, rect).unwrap();

        let mut app = app_on_new_page(doc, page);
        app.selection = vec![tid, rid];
        app.outline_stroke_selection();
        assert_eq!(app.selection.len(), 2, "both the text and the stroked shape were outlined");
        assert!(matches!(app.doc.get(app.selection[0]).unwrap().kind, NodeKind::Group));
        assert!(matches!(app.doc.get(app.selection[1]).unwrap().kind, NodeKind::Vector { .. }));
        assert_eq!(app.hist.undo_depth(), 1);

        app.undo();
        assert!(app.doc.get(tid).is_some() && app.doc.get(rid).is_some());

        app.selection = vec![tid, rid];
        app.flatten_selection();
        assert_eq!(app.selection.len(), 1, "flatten merges mixed selection into one vector");
        assert!(matches!(app.doc.get(app.selection[0]).unwrap().kind, NodeKind::Vector { .. }));
        assert_eq!(app.hist.undo_depth(), 1);
    }

    /// Computes `(max_channel_diff, mean_channel_diff, max_non_edge_diff)` between two RGBA pixmaps.
    fn pixmap_diff_stats(before: &designos_render::tiny_skia::Pixmap, after: &designos_render::tiny_skia::Pixmap) -> (u8, f64, u8) {
        assert_eq!((before.width(), before.height()), (after.width(), after.height()));
        let (w, h) = (before.width() as i32, before.height() as i32);
        let b_data = before.data();
        let a_data = after.data();
        let mut max_diff = 0u8;
        let mut sum_diff = 0u64;
        let mut max_non_edge_diff = 0u8;
        for y in 0..h {
            for x in 0..w {
                let idx = ((y * w + x) * 4) as usize;
                let mut px_max = 0u8;
                for c in 0..4 {
                    let d = b_data[idx + c].abs_diff(a_data[idx + c]);
                    px_max = px_max.max(d);
                    max_diff = max_diff.max(d);
                    sum_diff += u64::from(d);
                }
                if px_max > 0 {
                    // Check whether this pixel is on a 1px anti-aliased edge (i.e. 3x3 neighborhood is not uniform in `before`).
                    let center = &b_data[idx..idx + 4];
                    let mut on_edge = false;
                    for dy in -1i32..=1 {
                        for dx in -1i32..=1 {
                            let nx = (x + dx).clamp(0, w - 1);
                            let ny = (y + dy).clamp(0, h - 1);
                            let nidx = ((ny * w + nx) * 4) as usize;
                            if &b_data[nidx..nidx + 4] != center {
                                on_edge = true;
                            }
                        }
                    }
                    if !on_edge {
                        max_non_edge_diff = max_non_edge_diff.max(px_max);
                    }
                }
            }
        }
        let mean_diff = sum_diff as f64 / b_data.len() as f64;
        (max_diff, mean_diff, max_non_edge_diff)
    }

    #[test]
    fn pixel_test_converted_text_vectors_match_original_text_render() {
        let mut doc = Document::new("PixelTest");
        let page = doc.add_page("Page 1");
        let fid = doc.alloc_id();
        let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 320.0, 140.0));
        frame.fills = vec![Paint::solid(Color::WHITE)];
        doc.insert(page, frame).unwrap();

        let t1 = doc.alloc_id();
        let mut s1 = TextStyle {
            font_family: "Inter".into(),
            font_weight: 700,
            font_size: 28.0,
            decoration: TextDecoration::Underline,
            fills: vec![Paint::solid(Color::rgb(20, 30, 45))],
            ..Default::default()
        };
        s1.align_h = TextAlignH::Center;
        let mut n1 = Node::text(t1, Rect::new(20.0, 16.0, 280.0, 44.0), "Vector Text 1469", s1);
        if let NodeKind::Text(tp) = &mut n1.kind {
            tp.auto_resize = TextAutoResize::None;
        }
        doc.insert(fid, n1).unwrap();

        let t2 = doc.alloc_id();
        let n2 = two_run_text_node(t2, Color::rgb(210, 45, 45), Color::rgb(25, 135, 84));
        doc.insert(fid, n2).unwrap();

        let before_pm = designos_render::render_node(&doc, fid, 1.0).expect("render before");

        let mut app = app_on_new_page(doc, page);
        app.selection = vec![t1];
        app.flatten_selection();
        app.selection = vec![t2];
        app.flatten_selection();
        let after_flatten_pm = designos_render::render_node(&app.doc, fid, 1.0).expect("render after flatten");
        let (flat_max, flat_mean, flat_non_edge) = pixmap_diff_stats(&before_pm, &after_flatten_pm);
        println!("PIXEL_DIFF_FLATTEN: max={flat_max} mean={flat_mean:.6} non_edge_max={flat_non_edge}");
        assert!(flat_mean < 0.5, "flatten mean diff {flat_mean} >= 0.5");
        assert_eq!(flat_non_edge, 0, "flatten diff beyond 1px anti-aliased edge: {flat_non_edge}");

        app.undo();
        app.undo();
        app.selection = vec![t1, t2];
        app.outline_stroke_selection();
        let after_outline_pm = designos_render::render_node(&app.doc, fid, 1.0).expect("render after outline stroke");
        let (out_max, out_mean, out_non_edge) = pixmap_diff_stats(&before_pm, &after_outline_pm);
        println!("PIXEL_DIFF_OUTLINE_STROKE: max={out_max} mean={out_mean:.6} non_edge_max={out_non_edge}");
        assert!(out_mean < 0.5, "outline stroke mean diff {out_mean} >= 0.5");
        assert_eq!(out_non_edge, 0, "outline stroke diff beyond 1px anti-aliased edge: {out_non_edge}");
    }
}
