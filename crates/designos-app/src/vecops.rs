//! Object-level vector commands: flatten, outline stroke, boolean groups.
//! Each is one undo step. The geometry comes from `designos_render::vectorops`.

use crate::app::App;
use crate::edit::Tx;
use crate::util::*;
use designos_doc::VectorNetwork;
use designos_doc::*;
use designos_render::vectorops::{
    count_subpaths, flatten_nodes, node_paths, offset_paths, outline_node_stroke, text_aligned_stroke_layers, text_flatten_runs, text_glyph_runs,
    text_style_runs, AlignedTextLayer, TextVectorRun,
};
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

/// Builds a vector from paths while retaining their curve geometry for rendering.
/// The editable network is kept alongside the paths, but converting a text outline
/// through the network can otherwise perturb tiny-skia's antialiasing at glyph edges.
/// Also returns the bounds of `paths` in the space they were given in.
fn vector_node_preserving_paths(id: NodeId, paths: &[Path], template: &Node) -> (Node, Rect) {
    let mut net = VectorNetwork::from_paths(paths);
    let bb = net.bounds();
    net.translate(v2(-bb.x, -bb.y));
    let local_paths = paths
        .iter()
        .map(|path| Path {
            cmds: path
                .cmds
                .iter()
                .map(|cmd| match *cmd {
                    PathCmd::MoveTo(p) => PathCmd::MoveTo(v2(p.x - bb.x, p.y - bb.y)),
                    PathCmd::LineTo(p) => PathCmd::LineTo(v2(p.x - bb.x, p.y - bb.y)),
                    PathCmd::QuadTo(c, p) => PathCmd::QuadTo(v2(c.x - bb.x, c.y - bb.y), v2(p.x - bb.x, p.y - bb.y)),
                    PathCmd::CubicTo(c1, c2, p) => PathCmd::CubicTo(v2(c1.x - bb.x, c1.y - bb.y), v2(c2.x - bb.x, c2.y - bb.y), v2(p.x - bb.x, p.y - bb.y)),
                    PathCmd::Close => PathCmd::Close,
                })
                .collect(),
            fill_rule: path.fill_rule,
        })
        .collect();
    let mut n = Node::new(id, NodeKind::Vector { paths: local_paths, network: Some(net) });
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
    (n, bb)
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

/// The stroke paints are now the node's fills, so `strokes/N` bindings and the stroke style follow them.
fn carry_stroke_paint_as_fill(src: &Node, dst: &mut Node) {
    dst.variables.modes = src.variables.modes.clone();
    for (k, v) in &src.variables.bindings {
        let Some(rest) = k.strip_prefix("strokes/") else { continue };
        dst.variables.bindings.insert(format!("fills/{rest}"), v.clone());
    }
    if let Some(stroke_ref) = src.style_refs.get("stroke") {
        dst.style_refs.insert("fill".into(), stroke_ref.clone());
    }
}

/// Whether a text node draws an `Inside` or `Outside` stroke: the canvas clips such a stroke to the outline of each
/// style of the text, which a conversion has to keep.
fn has_aligned_text_stroke(stroke: &Stroke) -> bool {
    matches!(stroke.align, StrokeAlign::Inside | StrokeAlign::Outside) && stroke.weight > 0.0 && stroke.paints.iter().any(|p| p.visible)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PendingVecOp {
    Flatten,
    OutlineStroke,
}

#[derive(Clone, Debug)]
pub(crate) struct QueuedVecOp {
    pub doc_id: u64,
    pub op: PendingVecOp,
    pub ids: Vec<NodeId>,
    pub requests: BTreeSet<designos_fonts::FontRequest>,
}

fn collect_text_font_requests(doc: &Document, ids: &[NodeId], walk_subtree: bool) -> BTreeSet<designos_fonts::FontRequest> {
    let mut out = BTreeSet::new();
    let mut add_node = |raw: &Node| {
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
    };
    for id in ids {
        if walk_subtree {
            doc.walk(*id, |raw, _| add_node(raw));
        } else if let Some(raw) = doc.get(*id) {
            add_node(raw);
        }
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

fn set_fixed_layout_child(lc: &mut LayoutChild, counter: Option<bool>) {
    lc.set_axis_sizing(true, Sizing::Fixed, counter);
    lc.set_axis_sizing(false, Sizing::Fixed, counter);
    lc.align_self = AlignSelf::Auto;
    lc.grow = 0.0;
}

impl App {
    fn ensure_text_fonts(&mut self, ids: &[NodeId], walk_subtree: bool, op: PendingVecOp) -> bool {
        let doc_id = self.doc.runtime_id();
        self.fonts.pending_vecop.retain(|q| q.doc_id == doc_id);
        let requests = collect_text_font_requests(&self.doc, ids, walk_subtree);
        let (missing, failed) = self.fonts.check_requests(&requests);
        if let Some(first) = failed.first() {
            let more = failed.len() - 1;
            let suffix = if more > 0 { format!(" (+{more} more)") } else { String::new() };
            self.show_toast(format!("Font unavailable: {first}{suffix}"), true);
            return false;
        }
        if missing.is_empty() && self.fonts.pending_vecop.is_empty() {
            return true;
        }
        self.fonts.pending_vecop.push_back(QueuedVecOp { doc_id, op, ids: ids.to_vec(), requests });
        if !missing.is_empty() {
            self.fonts.request_new_faces(&self.doc, missing);
        }
        false
    }

    pub(crate) fn finish_pending_vecop(&mut self) {
        while let Some(mut queued) = self.fonts.pending_vecop.pop_front() {
            if queued.doc_id != self.doc.runtime_id() {
                continue;
            }
            let walk_subtree = matches!(queued.op, PendingVecOp::Flatten);
            queued.requests = collect_text_font_requests(&self.doc, &queued.ids, walk_subtree);
            let (missing, failed) = self.fonts.check_requests(&queued.requests);
            if let Some(first) = failed.first() {
                let more = failed.len() - 1;
                let suffix = if more > 0 { format!(" (+{more} more)") } else { String::new() };
                self.show_toast(format!("Font unavailable: {first}{suffix}"), true);
                continue;
            }
            if !missing.is_empty() {
                self.fonts.pending_vecop.push_front(queued);
                self.fonts.request_new_faces(&self.doc, missing);
                break;
            }
            if self.canvas.vedit.is_some() {
                let msg = match queued.op {
                    PendingVecOp::Flatten => "Flatten cancelled",
                    PendingVecOp::OutlineStroke => "Outline stroke cancelled",
                };
                self.show_toast(msg, true);
                continue;
            }
            match queued.op {
                PendingVecOp::Flatten => self.flatten_ids(&queued.ids, true),
                PendingVecOp::OutlineStroke => self.outline_stroke_ids(&queued.ids, true),
            }
        }
    }

    fn text_run_to_single_vector(&mut self, raw: &Node, resolved: &Node, run: &TextVectorRun) -> Node {
        let vid = self.doc.alloc_id();
        let counter = raw.parent.and_then(|p| self.doc.get(p)).and_then(LayoutChild::counter_axis_is_horizontal);
        let mut template = resolved.clone();
        template.name = raw.name.clone();
        template.fills = run.fills.clone();
        template.stroke = resolved.stroke.clone();
        let (mut node, bb) = vector_node_preserving_paths(vid, std::slice::from_ref(&run.path), &template);
        remap_paint_gradients(&mut node.fills, resolved.size, v2(bb.x, bb.y), node.size);
        remap_paint_gradients(&mut node.stroke.paints, resolved.size, v2(bb.x, bb.y), node.size);
        node.component = ComponentLink::default();
        node.visible = raw.visible;
        node.locked = raw.locked;
        set_fixed_layout_child(&mut node.layout_child, counter);
        let base_fills: &[Paint] = &[];
        carry_paint_bindings(raw, base_fills, &mut node, run.uses_base_fill, true);
        node
    }

    /// The group that replaces a text node converted into one vector per run or glyph.
    fn text_group_node(&mut self, raw: &Node, resolved: &Node) -> Node {
        let gid = self.doc.alloc_id();
        let counter = raw.parent.and_then(|p| self.doc.get(p)).and_then(LayoutChild::counter_axis_is_horizontal);
        let mut group = Node::new(gid, NodeKind::Group);
        group.name = raw.name.clone();
        group.transform = raw.transform;
        group.size = resolved.size;
        group.visible = raw.visible;
        group.locked = raw.locked;
        group.opacity = resolved.opacity;
        group.blend = raw.blend;
        group.constraints = raw.constraints;
        group.layout_child = raw.layout_child;
        set_fixed_layout_child(&mut group.layout_child, counter);
        group.effects = resolved.effects.clone();
        group.variables.modes = raw.variables.modes.clone();
        group
    }

    /// One vector of a converted text node's group: the geometry of `run` with its fills. It keeps the text node's
    /// stroke unless `carries_stroke` is false (the stroke then lives in a vector of its own).
    fn text_run_child(&mut self, raw: &Node, resolved: &Node, run: &TextVectorRun, gid: NodeId, position: usize, carries_stroke: bool) -> Node {
        let cid = self.doc.alloc_id();
        let mut child_template = resolved.clone();
        child_template.transform = Affine::IDENTITY;
        child_template.fills = run.fills.clone();
        child_template.stroke = if carries_stroke { resolved.stroke.clone() } else { Stroke::default() };
        child_template.effects.clear();
        child_template.opacity = 1.0;
        child_template.blend = BlendMode::Normal;
        let (mut child, bb) = vector_node_preserving_paths(cid, std::slice::from_ref(&run.path), &child_template);
        remap_paint_gradients(&mut child.fills, resolved.size, v2(bb.x, bb.y), child.size);
        remap_paint_gradients(&mut child.stroke.paints, resolved.size, v2(bb.x, bb.y), child.size);
        child.parent = Some(gid);
        child.name = if run.name.trim().is_empty() { format!("Vector {}", position + 1) } else { run.name.clone() };
        child.component = ComponentLink::default();
        set_fixed_layout_child(&mut child.layout_child, None);
        let base_fills: &[Paint] = &[];
        carry_paint_bindings(raw, base_fills, &mut child, run.uses_base_fill, carries_stroke);
        child
    }

    fn text_runs_to_group(&mut self, raw: &Node, resolved: &Node, runs: &[TextVectorRun]) -> Vec<Node> {
        let group = self.text_group_node(raw, resolved);
        let gid = group.id;
        let mut out = Vec::with_capacity(1 + runs.len());
        out.push(group);
        for (idx, run) in runs.iter().enumerate() {
            let child = self.text_run_child(raw, resolved, run, gid, idx, true);
            out[0].children.push(child.id);
            out.push(child);
        }
        out
    }

    /// The baked stroke of one glyph, or of several glyphs whose strokes touch, of Inside/Outside text. It is fill
    /// geometry, so the stroke paints become its fills.
    fn text_stroke_child(&mut self, raw: &Node, resolved: &Node, name: String, paths: &[Path], gid: NodeId) -> Node {
        let sid = self.doc.alloc_id();
        let mut template = resolved.clone();
        template.transform = Affine::IDENTITY;
        template.fills = resolved.stroke.paints.clone();
        template.stroke = Stroke::default();
        template.effects.clear();
        template.opacity = 1.0;
        template.blend = BlendMode::Normal;
        let (mut node, bb) = vector_node_preserving_paths(sid, paths, &template);
        remap_paint_gradients(&mut node.fills, resolved.size, v2(bb.x, bb.y), node.size);
        node.parent = Some(gid);
        node.name = name;
        node.component = ComponentLink::default();
        set_fixed_layout_child(&mut node.layout_child, None);
        carry_stroke_paint_as_fill(raw, &mut node);
        node
    }

    /// Inside/Outside text becomes a vector per glyph for its fill and a few vectors for its stroke (one per word, as a
    /// rule), in the order the canvas paints them. The stroke is baked as fill geometry cut like the canvas cuts it,
    /// with the stroke paints as its fills, so the converted text renders as before.
    fn text_aligned_outline_nodes(&mut self, raw: &Node, resolved: &Node) -> Vec<Node> {
        let layers = text_aligned_stroke_layers(&self.doc, raw);
        if layers.is_empty() {
            return Vec::new();
        }
        let group = self.text_group_node(raw, resolved);
        let gid = group.id;
        let mut out = Vec::with_capacity(1 + layers.len());
        out.push(group);
        for layer in layers {
            let child = match layer {
                AlignedTextLayer::Fill(run) => self.text_run_child(raw, resolved, &run, gid, out.len() - 1, false),
                AlignedTextLayer::Stroke { name, paths } => self.text_stroke_child(raw, resolved, name, &paths, gid),
            };
            out[0].children.push(child.id);
            out.push(child);
        }
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
        if !self.ensure_text_fonts(&ids, true, PendingVecOp::Flatten) {
            return;
        }
        self.flatten_ids(&ids, false);
    }

    fn flatten_ids(&mut self, raw_ids: &[NodeId], deferred: bool) {
        let ids: Vec<NodeId> = raw_ids.iter().copied().filter(|i| can_flatten(self, *i) && !linked(self, *i)).collect();
        let Some(first_parent) = ids.first().and_then(|i| self.doc.get(*i)).and_then(|n| n.parent) else {
            return;
        };
        if ids.iter().any(|i| self.doc.get(*i).and_then(|n| n.parent) != Some(first_parent)) {
            self.show_toast("Flatten cancelled", true);
            return;
        }
        if ids.len() == 1 && self.doc.get(ids[0]).is_some_and(|n| matches!(n.kind, NodeKind::Text(_))) {
            let id = ids[0];
            let Some(n) = self.doc.get(id).cloned() else { return };
            let Some(parent) = n.parent else { return };
            let Some(index) = self.doc.get(parent).and_then(|p| p.children.iter().position(|c| *c == id)) else { return };
            let resolved = resolve_node(&self.doc, &n).into_owned();
            // an Inside/Outside stroke is clipped per style on the canvas, so each style stays its own vector
            let runs = if has_aligned_text_stroke(&resolved.stroke) { text_style_runs(&self.doc, &n) } else { text_flatten_runs(&self.doc, &n) };
            if runs.is_empty() {
                if !deferred {
                    self.show_toast("Nothing to flatten", false);
                }
                return;
            }
            let is_group = runs.len() > 1;
            let nodes = if !is_group { vec![self.text_run_to_single_vector(&n, &resolved, &runs[0])] } else { self.text_runs_to_group(&n, &resolved, &runs) };
            let out_id = nodes[0].id;
            let mut tx = Tx::new();
            if tx.cmds_all_or_none(&mut self.doc, vec![Command::Remove { id }, Command::Insert { parent, index, nodes }]).is_err() {
                tx.cancel(&mut self.doc);
                return;
            }
            if is_group {
                crate::layout::fit_group(&mut self.doc, &mut tx.ed, out_id);
            }
            crate::layout::relayout_up(&mut self.doc, &mut tx.ed, out_id);
            self.run_tx(tx);
            self.selection = vec![out_id];
            return;
        }
        let Some(parent) = self.doc.get(ids[0]).and_then(|n| n.parent) else { return };
        let Some(flat) = flatten_nodes(&self.doc, &ids, &self.doc.world_transform(parent)) else {
            if !deferred {
                self.show_toast("Nothing to flatten", false);
            }
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
                base.fills = tp.runs.iter().find(|r| r.start == 0 && r.end > 0).map(|r| r.style.fills.clone()).unwrap_or_else(|| tp.style.fills.clone());
                tp.style.fills.as_slice()
            }
            _ => resolved_template.fills.as_slice(),
        };
        let mut node = vector_node(id, &flat.paths, &base);
        node.name = if ids.len() == 1 { template.name.clone() } else { self.unique_name("Vector") };
        node.component = ComponentLink::default();
        node.visible = template.visible;
        node.locked = template.locked;
        if matches!(template.kind, NodeKind::Text(_)) {
            let counter = self.doc.get(parent).and_then(LayoutChild::counter_axis_is_horizontal);
            set_fixed_layout_child(&mut node.layout_child, counter);
        }
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
        if !self.ensure_text_fonts(&tops, false, PendingVecOp::OutlineStroke) {
            return;
        }
        self.outline_stroke_ids(&tops, false);
    }

    fn outline_stroke_ids(&mut self, raw_ids: &[NodeId], deferred: bool) {
        let mut tx = Tx::new();
        let mut made = vec![];
        for &id in raw_ids {
            let Some(n) = self.doc.get(id).cloned() else { continue };
            if matches!(n.kind, NodeKind::Text(_)) {
                if linked(self, id) {
                    continue;
                }
                let resolved = resolve_node(&self.doc, &n).into_owned();
                let nodes = if has_aligned_text_stroke(&resolved.stroke) {
                    self.text_aligned_outline_nodes(&n, &resolved)
                } else {
                    let runs = text_glyph_runs(&self.doc, &n);
                    if runs.is_empty() {
                        continue;
                    }
                    self.text_runs_to_group(&n, &resolved, &runs)
                };
                if nodes.is_empty() {
                    continue;
                }
                let Some(parent) = n.parent else { continue };
                let Some(index) = self.doc.get(parent).and_then(|p| p.children.iter().position(|c| *c == id)) else { continue };
                let gid = nodes[0].id;
                if tx.cmds_all_or_none(&mut self.doc, vec![Command::Remove { id }, Command::Insert { parent, index, nodes }]).is_err() {
                    continue;
                }
                crate::layout::fit_group(&mut self.doc, &mut tx.ed, gid);
                crate::layout::relayout_up(&mut self.doc, &mut tx.ed, gid);
                made.push(gid);
                continue;
            }
            let visible_paints = n.stroke.paints.iter().any(|p| p.visible);
            if !visible_paints || n.stroke.weight <= 0.0 {
                continue;
            }
            let outline = outline_node_stroke(&self.doc, &n);
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
            if !deferred {
                self.show_toast("Select a layer with a stroke to outline", false);
            }
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
        assert_eq!(
            [group.transform.a, group.transform.b, group.transform.c, group.transform.d],
            [expected_tf.a, expected_tf.b, expected_tf.c, expected_tf.d],
            "group preserves the text layer's rotation/scale"
        );
        let min_x = group.children.iter().map(|c| app.doc.get(*c).unwrap().transform.tx).fold(f32::INFINITY, f32::min);
        let min_y = group.children.iter().map(|c| app.doc.get(*c).unwrap().transform.ty).fold(f32::INFINITY, f32::min);
        assert!(min_x.abs() < 1e-3 && min_y.abs() < 1e-3, "group origin is fitted to its children");
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
        assert_eq!([group.transform.a, group.transform.b, group.transform.c, group.transform.d], [expected_tf.a, expected_tf.b, expected_tf.c, expected_tf.d]);
        assert!(group.transform.tx > expected_tf.tx && group.transform.ty > expected_tf.ty);
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
        let mut fill_nodes = 0;
        let mut stroke_nodes = 0;
        for &cid in &group.children {
            let gv = app.doc.get(cid).unwrap();
            match gv.variables.bindings.get("fills/0").map(String::as_str) {
                Some("var_fill") => {
                    assert!(gv.stroke.paints.is_empty(), "the glyph fill no longer carries the stroke");
                    fill_nodes += 1;
                }
                Some("var_stroke") => {
                    assert_eq!(gv.fills, vec![Paint::solid(stroke_color)], "the baked stroke uses the stroke paint");
                    assert!(gv.stroke.paints.is_empty());
                    stroke_nodes += 1;
                }
                other => panic!("unexpected fills/0 binding {other:?}"),
            }
        }
        assert!(fill_nodes >= 1 && stroke_nodes >= 1, "fill geometry and baked stroke are both present");
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
        settle_app_fonts(&mut app);
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
        settle_app_fonts(&mut app);
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

    /// Computes `(max_channel_diff, mean_all_diff, mean_ink_diff, hard_px, max_non_edge_diff, ink_px)`
    /// between two RGBA pixmaps rendered on a white background.
    fn pixmap_diff_stats(before: &designos_render::tiny_skia::Pixmap, after: &designos_render::tiny_skia::Pixmap) -> (u8, f64, f64, usize, u8, usize) {
        assert_eq!((before.width(), before.height()), (after.width(), after.height()));
        let (w, h) = (before.width() as i32, before.height() as i32);
        let b_data = before.data();
        let a_data = after.data();
        let mut max_diff = 0u8;
        let mut sum_diff = 0u64;
        let mut ink_sum = 0u64;
        let mut ink_px = 0usize;
        let mut hard_px = 0usize;
        let mut max_non_edge_diff = 0u8;
        for y in 0..h {
            for x in 0..w {
                let idx = ((y * w + x) * 4) as usize;
                let is_white = |d: &[u8]| d[idx] == 255 && d[idx + 1] == 255 && d[idx + 2] == 255 && d[idx + 3] == 255;
                let mut px_max = 0u8;
                let mut px_sum = 0u64;
                for c in 0..4 {
                    let d = b_data[idx + c].abs_diff(a_data[idx + c]);
                    px_max = px_max.max(d);
                    px_sum += u64::from(d);
                }
                max_diff = max_diff.max(px_max);
                sum_diff += px_sum;
                if !is_white(b_data) || !is_white(a_data) {
                    ink_px += 1;
                    ink_sum += px_sum;
                }
                if px_max > 64 {
                    hard_px += 1;
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
        let mean_all = sum_diff as f64 / b_data.len() as f64;
        let mean_ink = if ink_px > 0 { ink_sum as f64 / (ink_px as f64 * 4.0) } else { 0.0 };
        (max_diff, mean_all, mean_ink, hard_px, max_non_edge_diff, ink_px)
    }

    fn settle_app_fonts(app: &mut App) {
        for _ in 0..6000 {
            app.poll_fonts();
            if !app.fonts.is_loading() && !app.text_refit_pending() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("font load or text refit did not finish");
    }

    #[test]
    fn text_in_auto_layout_keeps_fixed_size_after_relayout_and_redo_matches_commit() {
        let mut doc = Document::new("AutoLayoutText");
        let page = doc.add_page("Page 1");
        let pid = doc.alloc_id();
        let mut parent = Node::frame(pid, Rect::new(0.0, 0.0, 400.0, 100.0));
        parent.kind = NodeKind::Frame(FrameProps {
            auto_layout: Some(AutoLayout {
                direction: LayoutDirection::Vertical,
                primary_sizing: Sizing::Hug,
                counter_sizing: Sizing::Fixed,
                item_spacing: 8.0,
                padding: [10.0; 4],
                ..Default::default()
            }),
            ..Default::default()
        });
        doc.insert(page, parent).unwrap();

        let tid = doc.alloc_id();
        let style = TextStyle { font_family: "Inter".into(), font_size: 24.0, fills: vec![Paint::solid(Color::BLACK)], ..Default::default() };
        let mut t = Node::text(tid, Rect::new(0.0, 0.0, 200.0, 40.0), "Label text", style);
        t.layout_child.horizontal = Sizing::Fill;
        t.layout_child.align_self = AlignSelf::Stretch;
        t.layout_child.grow = 1.0;
        t.visible = false;
        t.locked = true;
        doc.insert(pid, t).unwrap();

        let rid = doc.alloc_id();
        doc.insert(pid, Node::rectangle(rid, Rect::new(0.0, 0.0, 100.0, 30.0))).unwrap();
        doc.relayout_subtree(pid);

        let mut app = app_on_new_page(doc, page);
        app.selection = vec![tid];
        app.flatten_selection();

        assert!(app.doc.get(tid).is_none());
        let vid = app.selection[0];
        let v = app.doc.get(vid).unwrap();
        assert_eq!(v.layout_child.horizontal, Sizing::Fixed);
        assert_eq!(v.layout_child.vertical, Sizing::Fixed);
        assert_eq!(v.layout_child.align_self, AlignSelf::Auto);
        assert_eq!(v.layout_child.grow, 0.0);
        assert!(!v.visible, "hidden text produces hidden vector");
        assert!(v.locked, "locked text produces locked vector");
        let committed_size = v.size;
        let committed_parent_size = app.doc.get(pid).unwrap().size;
        let committed_json = app.doc.to_json().unwrap();

        let mut re_doc = app.doc.clone();
        re_doc.relayout_subtree(pid);
        assert_eq!(re_doc.get(vid).unwrap().size, committed_size, "relayout_subtree leaves flattened vector size unchanged");
        assert_eq!(re_doc.get(pid).unwrap().size, committed_parent_size, "parent auto-layout frame was already solved at commit");

        app.edit_nodes(&[rid], |n| n.opacity = 0.8);
        assert_eq!(app.doc.get(vid).unwrap().size, committed_size, "subsequent sibling edit does not stretch vector");
        app.undo();

        app.undo();
        app.redo();
        assert_eq!(app.doc.to_json().unwrap(), committed_json, "redo state matches committed state exactly");

        // Now verify Outline stroke on the same auto-layout text: group gets Fixed sizing, visible/locked,
        // and its box is its children's union at commit.
        app.undo();
        app.selection = vec![tid];
        app.outline_stroke_selection();
        let gid = app.selection[0];
        let g = app.doc.get(gid).unwrap();
        assert!(matches!(g.kind, NodeKind::Group));
        assert_eq!(g.layout_child.horizontal, Sizing::Fixed);
        assert_eq!(g.layout_child.vertical, Sizing::Fixed);
        assert_eq!(g.layout_child.align_self, AlignSelf::Auto);
        assert_eq!(g.layout_child.grow, 0.0);
        assert!(!g.visible, "hidden text produces hidden group");
        assert!(g.locked, "locked text produces locked group");

        let g_committed_box = (g.size, g.transform.tx, g.transform.ty);
        let g_committed_json = app.doc.to_json().unwrap();
        let first_child = g.children[0];
        app.selection = vec![first_child];
        app.nudge(1.0, 0.0);
        app.nudge(-1.0, 0.0);
        let g_after_nudge = app.doc.get(gid).unwrap();
        assert_eq!(
            (g_after_nudge.size, g_after_nudge.transform.tx, g_after_nudge.transform.ty),
            g_committed_box,
            "group box is already its children's union at commit"
        );
        app.undo();
        app.undo();
        app.undo();
        app.redo();
        assert_eq!(app.doc.to_json().unwrap(), g_committed_json, "outline stroke redo equals commit");
    }

    #[test]
    fn text_to_vector_queues_missing_font_without_blocking_and_respects_failed_and_outline_scope() {
        let mut doc = Document::new("AsyncFontTest");
        let page = doc.add_page("Page 1");
        let t_flat = doc.alloc_id();
        let s_flat = TextStyle { font_family: "SlowFntA001".into(), font_size: 20.0, ..Default::default() };
        doc.insert(page, Node::text(t_flat, Rect::new(0.0, 0.0, 120.0, 32.0), "Async Flat", s_flat)).unwrap();

        let mut app = app_on_new_page(doc, page);
        app.fonts = crate::fonts::testkit::serving_with_delay(&["SlowFntA001", "SlowFntA002"], std::time::Duration::from_millis(250));
        let _ = designos_render::resolve_font("Inter", 400, false);

        // 1. Flatten with a slow fetcher returns immediately on the UI thread and converts in poll_fonts.
        app.selection = vec![t_flat];
        let t0 = std::time::Instant::now();
        app.flatten_selection();
        let call_ms = t0.elapsed().as_millis();
        assert!(call_ms < 100, "flatten_selection must not block UI thread on font fetch (took {call_ms} ms)");
        assert!(app.doc.get(t_flat).is_some(), "text stays until font arrives");
        assert!(app.fonts.is_loading(), "background font load started");

        settle_app_fonts(&mut app);
        assert!(app.doc.get(t_flat).is_none(), "pending flatten ran once font arrived in poll_fonts");
        assert!(matches!(app.doc.get(app.selection[0]).unwrap().kind, NodeKind::Vector { .. }));

        // 2. Outline stroke with a slow fetcher also returns immediately and converts in poll_fonts.
        let t_out = app.doc.alloc_id();
        let s_out = TextStyle { font_family: "SlowFntA002".into(), font_size: 20.0, ..Default::default() };
        app.doc.insert(page, Node::text(t_out, Rect::new(0.0, 40.0, 120.0, 32.0), "Async Out", s_out)).unwrap();
        app.selection = vec![t_out];
        let t1 = std::time::Instant::now();
        app.outline_stroke_selection();
        let out_ms = t1.elapsed().as_millis();
        assert!(out_ms < 100, "outline_stroke_selection must not block UI thread on font fetch (took {out_ms} ms)");
        assert!(app.doc.get(t_out).is_some());
        settle_app_fonts(&mut app);
        assert!(app.doc.get(t_out).is_none(), "pending outline stroke ran once font arrived in poll_fonts");
        assert!(matches!(app.doc.get(app.selection[0]).unwrap().kind, NodeKind::Group));

        // 3. Outline stroke on a stroked frame + stroked rect is not blocked by a descendant text's missing font.
        let fid = app.doc.alloc_id();
        let mut frame = Node::frame(fid, Rect::new(0.0, 100.0, 200.0, 100.0));
        frame.stroke.paints = vec![Paint::solid(Color::BLACK)];
        frame.stroke.weight = 4.0;
        app.doc.insert(page, frame).unwrap();
        let child_tid = app.doc.alloc_id();
        let child_style = TextStyle { font_family: "MissingChildFont".into(), font_size: 18.0, ..Default::default() };
        app.doc.insert(fid, Node::text(child_tid, Rect::new(10.0, 10.0, 100.0, 30.0), "child", child_style)).unwrap();
        let rid = app.doc.alloc_id();
        let mut rect = Node::rectangle(rid, Rect::new(240.0, 100.0, 80.0, 80.0));
        rect.fills.clear();
        rect.stroke.paints = vec![Paint::solid(Color::BLACK)];
        rect.stroke.weight = 4.0;
        app.doc.insert(page, rect).unwrap();

        let undo_before = app.hist.undo_depth();
        app.toast = None;
        app.selection = vec![fid, rid];
        app.outline_stroke_selection();
        assert_eq!(app.hist.undo_depth(), undo_before + 1, "outline stroke outlines the frame and rect immediately");
        assert!(app.toast.is_none(), "child text font inside frame does not block outline stroke");
        assert!(app.doc.get(rid).is_none(), "stroked rectangle was replaced by its outline");

        // Flatten on the frame DOES walk the subtree, requests MissingChildFont, fails in poll_fonts,
        // and a subsequent Flatten on the already-failed face shows the toast immediately without starting a load.
        app.selection = vec![fid];
        app.flatten_selection();
        settle_app_fonts(&mut app);
        let toast = app.toast.as_ref().expect("missing child font toast shown on flatten");
        assert!(toast.error && toast.text.starts_with("Font unavailable: MissingChildFont 400:"));

        app.toast = None;
        let started_before = app.fonts.started();
        app.flatten_selection();
        assert_eq!(app.fonts.started(), started_before, "already-failed face starts no new load");
        let toast2 = app.toast.as_ref().expect("already-failed face shows toast immediately");
        assert!(toast2.error && toast2.text.starts_with("Font unavailable: MissingChildFont 400:"));
    }

    #[test]
    fn text_to_vector_preserves_gradient_style_links_and_merges_shared_paint_runs_without_leaking_bindings() {
        let mut doc = Document::new("GradientAndRuns");
        let page = doc.add_page("Page 1");

        // Part A: gradient fill style link and fills/0/stops/1/color binding survive Flatten and Outline stroke.
        let grad_tid = doc.alloc_id();
        let mut grad_style = TextStyle { font_family: "Inter".into(), font_weight: 700, font_size: 36.0, ..Default::default() };
        grad_style.fills = vec![Paint {
            kind: PaintKind::LinearGradient {
                handles: [Vec2::new(0.0, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.0, 1.0)],
                stops: vec![GradientStop { pos: 0.0, color: Color::rgb(230, 30, 30) }, GradientStop { pos: 1.0, color: Color::rgb(30, 60, 230) }],
            },
            opacity: 1.0,
            visible: true,
            blend: BlendMode::Normal,
        }];
        let mut grad_node = Node::text(grad_tid, Rect::new(0.0, 0.0, 320.0, 50.0), "Gradient", grad_style);
        grad_node.style_refs.insert("fill".into(), "S:brand-gradient".into());
        grad_node.variables.bindings.insert("fills/0/stops/1/color".into(), "var_stop".into());
        doc.insert(page, grad_node).unwrap();

        let mut app = app_on_new_page(doc, page);
        app.selection = vec![grad_tid];
        app.flatten_selection();
        let flat_v = app.doc.get(app.selection[0]).unwrap();
        assert_eq!(flat_v.style_refs.get("fill").map(String::as_str), Some("S:brand-gradient"));
        assert_eq!(flat_v.variables.bindings.get("fills/0/stops/1/color").map(String::as_str), Some("var_stop"));

        app.undo();
        app.selection = vec![grad_tid];
        app.outline_stroke_selection();
        let out_g = app.doc.get(app.selection[0]).unwrap();
        assert!(!out_g.children.is_empty());
        for &cid in &out_g.children {
            let child = app.doc.get(cid).unwrap();
            assert_eq!(child.style_refs.get("fill").map(String::as_str), Some("S:brand-gradient"));
            assert_eq!(child.variables.bindings.get("fills/0/stops/1/color").map(String::as_str), Some("var_stop"));
        }

        // Part B: runs that share a paint set merge into one vector under Flatten, and
        // base-fill bindings/style_refs do not leak to recoloured runs.
        let red = Color::rgb(200, 20, 20);
        let green = Color::rgb(20, 160, 60);
        let base_red = TextStyle { font_family: "Inter".into(), font_weight: 400, font_size: 24.0, fills: vec![Paint::solid(red)], ..Default::default() };
        let bold_red = TextStyle { font_weight: 700, ..base_red.clone() };
        let green_run = TextStyle { fills: vec![Paint::solid(green)], ..base_red.clone() };

        // Two runs with the SAME paint set (normal red + bold red) flatten into a single Vector.
        let same_paint_tid = app.doc.alloc_id();
        let mut same_paint_node = Node::text(same_paint_tid, Rect::new(0.0, 60.0, 200.0, 40.0), "norm BOLD", base_red.clone());
        if let NodeKind::Text(tp) = &mut same_paint_node.kind {
            tp.runs = vec![TextRun { start: 5, end: 9, style: bold_red.clone() }];
        }
        app.doc.insert(page, same_paint_node).unwrap();
        app.selection = vec![same_paint_tid];
        app.flatten_selection();
        let merged_node = app.doc.get(app.selection[0]).unwrap();
        assert!(matches!(merged_node.kind, NodeKind::Vector { .. }), "shared paint set across runs merges into 1 vector");

        // Three runs where run 0 and run 2 share red and run 1 is green produce 2 children,
        // and only the red child carries the base fill binding.
        let mixed_tid = app.doc.alloc_id();
        let mut mixed_node = Node::text(mixed_tid, Rect::new(0.0, 120.0, 300.0, 40.0), "red GREEN boldred", base_red);
        if let NodeKind::Text(tp) = &mut mixed_node.kind {
            tp.runs = vec![TextRun { start: 4, end: 9, style: green_run }, TextRun { start: 10, end: 17, style: bold_red }];
        }
        mixed_node.variables.bindings.insert("fills/0".into(), "var_red".into());
        mixed_node.style_refs.insert("fill".into(), "S:red".into());
        app.doc.insert(page, mixed_node).unwrap();
        app.selection = vec![mixed_tid];
        app.flatten_selection();
        let grp = app.doc.get(app.selection[0]).unwrap();
        assert!(matches!(grp.kind, NodeKind::Group));
        assert_eq!(grp.children.len(), 2, "three runs with two distinct paint sets merge into 2 vectors");
        let c0 = app.doc.get(grp.children[0]).unwrap();
        let c1 = app.doc.get(grp.children[1]).unwrap();
        assert_eq!(c0.variables.bindings.get("fills/0").map(String::as_str), Some("var_red"));
        assert_eq!(c0.style_refs.get("fill").map(String::as_str), Some("S:red"));
        assert!(c1.variables.bindings.is_empty(), "recoloured run must not inherit base fill binding");
        assert!(!c1.style_refs.contains_key("fill"), "recoloured run must not inherit base fill style_ref");
    }

    /// Outline stroke on a dashed inside rectangle gives the dashes the canvas draws: of the stroke's own
    /// weight, with round caps that keep the gap after each dash open.
    #[test]
    fn outline_stroke_of_a_dashed_inside_rectangle_matches_the_canvas_stroke() {
        let mut doc = Document::new("PixelTest");
        let page = doc.add_page("Page 1");
        let fid = doc.alloc_id();
        doc.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 200.0, 120.0))).unwrap();
        let rid = doc.alloc_id();
        let mut rect = Node::rectangle(rid, Rect::new(30.0, 30.0, 100.0, 60.0));
        rect.fills.clear();
        rect.stroke = Stroke {
            paints: vec![Paint::solid(Color::rgb(200, 30, 30))],
            weight: 8.0,
            align: StrokeAlign::Inside,
            cap: StrokeCap::Round,
            dashes: vec![10.0, 14.0],
            miter_limit: 4.0,
            ..Default::default()
        };
        doc.insert(fid, rect).unwrap();
        let before = designos_render::render_node(&doc, fid, 2.0).expect("render before");

        let mut app = app_on_new_page(doc, page);
        app.selection = vec![rid];
        app.outline_stroke_selection();

        let outlined = app.doc.get(app.selection[0]).expect("the outline is selected");
        assert!(matches!(outlined.kind, NodeKind::Vector { .. }));
        assert_eq!(outlined.fills, vec![Paint::solid(Color::rgb(200, 30, 30))], "the stroke paint becomes the fill");
        let after = designos_render::render_node(&app.doc, fid, 2.0).expect("render after outline stroke");
        let (_max, _all, _ink, hard_px, _non_edge, ink_px) = pixmap_diff_stats(&before, &after);
        assert!(ink_px > 0, "the dashes are drawn");
        assert_eq!(hard_px, 0, "the outline covers what the canvas strokes: {hard_px} pixels differ strongly");
        let pixel = |pm: &designos_render::tiny_skia::Pixmap, x: f32, y: f32| {
            let p = pm.pixel((x * 2.0) as u32, (y * 2.0) as u32).unwrap();
            [p.red(), p.green(), p.blue()]
        };
        assert_eq!(pixel(&after, 39.0, 34.0), [200, 30, 30], "a dash");
        assert_eq!(pixel(&after, 51.0, 34.0), [255, 255, 255], "the gap after it stays open");
    }

    fn assert_text_scenario_pixels(label: &str, make: impl FnOnce(&mut Document, NodeId) -> Vec<NodeId>) {
        let mut doc = Document::new("PixelTest");
        let page = doc.add_page("Page 1");
        let fid = doc.alloc_id();
        let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 480.0, 160.0));
        frame.fills = vec![Paint::solid(Color::WHITE)];
        doc.insert(page, frame).unwrap();
        let text_ids = make(&mut doc, fid);

        let before_pm = designos_render::render_node(&doc, fid, 2.0).expect("render before");

        let mut app = app_on_new_page(doc, page);
        for &tid in &text_ids {
            app.selection = vec![tid];
            app.flatten_selection();
            assert!(app.doc.get(tid).is_none(), "{label}: flatten must remove original text node {tid:?}");
            let out = app.doc.get(app.selection[0]).expect("converted node exists");
            assert!(matches!(out.kind, NodeKind::Vector { .. } | NodeKind::Group), "{label}: expected vector or group after flatten");
        }
        let after_flatten_pm = designos_render::render_node(&app.doc, fid, 2.0).expect("render after flatten");
        let (flat_max, _flat_all, flat_ink, flat_hard, flat_non_edge, flat_ink_px) = pixmap_diff_stats(&before_pm, &after_flatten_pm);
        println!("PIXEL_DIFF_FLATTEN[{label}]: max={flat_max} mean_ink={flat_ink:.6} hard_px={flat_hard} non_edge_max={flat_non_edge} ink_px={flat_ink_px}");
        assert!(flat_ink_px > 0, "{label}: expected non-empty ink pixels");
        assert!(flat_ink < 1.0, "{label}: flatten mean_ink {flat_ink} >= 1.0");
        assert_eq!(flat_hard, 0, "{label}: flatten hard_px {flat_hard} > 0");
        assert_eq!(flat_non_edge, 0, "{label}: flatten diff beyond 1px anti-aliased edge: {flat_non_edge}");

        for _ in &text_ids {
            app.undo();
        }
        app.selection = text_ids.clone();
        app.outline_stroke_selection();
        for &tid in &text_ids {
            assert!(app.doc.get(tid).is_none(), "{label}: outline stroke must remove original text node {tid:?}");
        }
        for &sel in &app.selection {
            let out = app.doc.get(sel).expect("outlined group exists");
            assert!(matches!(out.kind, NodeKind::Group) && !out.children.is_empty(), "{label}: expected non-empty group after outline stroke");
        }
        let after_outline_pm = designos_render::render_node(&app.doc, fid, 2.0).expect("render after outline stroke");
        let (out_max, _out_all, out_ink, out_hard, out_non_edge, out_ink_px) = pixmap_diff_stats(&before_pm, &after_outline_pm);
        println!("PIXEL_DIFF_OUTLINE_STROKE[{label}]: max={out_max} mean_ink={out_ink:.6} hard_px={out_hard} non_edge_max={out_non_edge} ink_px={out_ink_px}");
        assert!(out_ink_px > 0, "{label}: expected non-empty ink pixels");
        assert!(out_ink < 1.0, "{label}: outline stroke mean_ink {out_ink} >= 1.0");
        assert_eq!(out_hard, 0, "{label}: outline stroke hard_px {out_hard} > 0");
        assert_eq!(out_non_edge, 0, "{label}: outline stroke diff beyond 1px anti-aliased edge: {out_non_edge}");
    }

    fn o_page(align: Option<StrokeAlign>) -> (Document, NodeId) {
        let mut doc = Document::new("StrokeAlignmentConversion");
        let page = doc.add_page("Page 1");
        doc.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::WHITE };
        let tid = doc.alloc_id();
        let mut text = Node::text(
            tid,
            Rect::new(40.0, 24.0, 160.0, 140.0),
            "O",
            TextStyle { font_family: "Inter".into(), font_size: 96.0, fills: vec![Paint::solid(Color::BLACK)], ..Default::default() },
        );
        if let Some(align) = align {
            text.stroke.paints = vec![Paint::solid(Color::rgb(255, 0, 0))];
            text.stroke.weight = 6.0;
            text.stroke.align = align;
        }
        doc.insert(page, text).unwrap();
        (doc, page)
    }

    fn o_pixmap(doc: &Document, page: NodeId) -> designos_render::tiny_skia::Pixmap {
        let mut pm = designos_render::tiny_skia::Pixmap::new(260, 200).unwrap();
        designos_render::render_page(doc, page, &designos_render::Viewport::default(), &mut pm);
        pm
    }

    fn o_px(pm: &designos_render::tiny_skia::Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
        let p = pm.pixel(x, y).unwrap();
        (p.red(), p.green(), p.blue())
    }

    fn o_page_color(c: (u8, u8, u8)) -> bool {
        c.0 > 245 && c.1 > 245 && c.2 > 245
    }

    fn o_red(c: (u8, u8, u8)) -> bool {
        c.0 > 180 && c.1 < 60 && c.2 < 60
    }

    fn o_black(c: (u8, u8, u8)) -> bool {
        c.0 < 32 && c.1 < 32 && c.2 < 32
    }

    fn o_ink_runs(pm: &designos_render::tiny_skia::Pixmap, y: u32) -> Vec<(u32, u32)> {
        let mut runs = Vec::new();
        let mut start = None;
        for x in 0..pm.width() {
            let ink = !o_page_color(o_px(pm, x, y));
            match (start, ink) {
                (None, true) => start = Some(x),
                (Some(s), false) => {
                    runs.push((s, x - 1));
                    start = None;
                }
                _ => {}
            }
        }
        if let Some(s) = start {
            runs.push((s, pm.width() - 1));
        }
        runs
    }

    struct OCounter {
        y: u32,
        outer: u32,
        inner: u32,
        stem_mid: u32,
        counter_mid: u32,
    }

    fn o_counter(pm: &designos_render::tiny_skia::Pixmap) -> OCounter {
        let mut best_gap = 0u32;
        let mut best_y = 0u32;
        let mut best_runs = Vec::new();
        for y in 0..pm.height() {
            let runs = o_ink_runs(pm, y);
            if runs.len() != 2 {
                continue;
            }
            let gap = runs[1].0.saturating_sub(runs[0].1);
            if gap > best_gap {
                best_gap = gap;
                best_y = y;
                best_runs = runs;
            }
        }
        assert!(!best_runs.is_empty(), "glyph has a counter");
        let y = best_y;
        let runs = best_runs;
        let (outer, inner) = runs[0];
        let mut solid = Vec::new();
        for x in outer..=inner {
            if o_black(o_px(pm, x, y)) {
                solid.push(x);
            }
        }
        assert!(!solid.is_empty(), "stem has a solid fill");
        OCounter { y, outer, inner, stem_mid: solid[solid.len() / 2], counter_mid: (inner + runs[1].0) / 2 }
    }

    fn assert_o_sample(pm: &designos_render::tiny_skia::Pixmap, c: &OCounter, align: StrokeAlign) {
        let y = c.y;
        assert!(c.outer >= 4 && c.inner + 4 < pm.width(), "samples stay inside the pixmap");
        assert!(c.counter_mid > c.inner + 3, "counter is wider than the stroke");
        let red = |x: u32, what: &str| {
            let rgb = o_px(pm, x, y);
            assert!(o_red(rgb), "{what} at ({x},{y}) is rgb{rgb:?}");
        };
        let page = |x: u32, what: &str| {
            let rgb = o_px(pm, x, y);
            assert!(o_page_color(rgb), "{what} at ({x},{y}) is rgb{rgb:?}");
        };
        let black = |x: u32, what: &str| {
            let rgb = o_px(pm, x, y);
            assert!(o_black(rgb), "{what} at ({x},{y}) is rgb{rgb:?}");
        };
        match align {
            StrokeAlign::Inside => {
                red(c.outer + 3, "red just inside the outer edge");
                page(c.outer - 3, "no red outside the outer edge");
                page(c.inner + 3, "no red in the counter next to the inner contour");
                page(c.counter_mid, "no red in the counter center");
            }
            StrokeAlign::Outside => {
                red(c.outer - 3, "red just past the outer edge");
                red(c.inner + 3, "red in the counter next to the inner contour");
                black(c.stem_mid, "no red deep in the fill");
            }
            StrokeAlign::Center => {
                red(c.outer - 2, "red outside the edge");
                red(c.outer + 2, "red inside the edge");
            }
        }
    }

    #[test]
    fn text_vector_conversions_preserve_stroke_alignment() {
        let (fill_doc, fill_page) = o_page(None);
        let ring = o_counter(&o_pixmap(&fill_doc, fill_page));
        eprintln!("counter y={} outer={} inner={} stem_mid={} counter_mid={}", ring.y, ring.outer, ring.inner, ring.stem_mid, ring.counter_mid);
        for align in [StrokeAlign::Inside, StrokeAlign::Center, StrokeAlign::Outside] {
            for outline in [false, true] {
                let (doc, page) = o_page(Some(align));
                let tid = doc.get(page).unwrap().children[0];
                let mut app = app_on_new_page(doc, page);
                app.selection = vec![tid];
                if outline {
                    app.outline_stroke_selection();
                } else {
                    app.flatten_selection();
                }
                eprintln!("conversion outline={outline} align={align:?}");
                assert_o_sample(&o_pixmap(&app.doc, page), &ring, align);
            }
        }
    }

    #[test]
    fn aligned_text_stroke_matches_live_text() {
        for align in [StrokeAlign::Outside, StrokeAlign::Inside] {
            let label = match align {
                StrokeAlign::Outside => "aligned_outside",
                StrokeAlign::Inside => "aligned_inside",
                StrokeAlign::Center => "aligned_center",
            };
            assert_text_scenario_pixels(label, |doc, fid| {
                let t = doc.alloc_id();
                let mut n = Node::text(
                    t,
                    Rect::new(20.0, 40.0, 320.0, 80.0),
                    "Aligned",
                    TextStyle { font_family: "Inter".into(), font_size: 32.0, fills: vec![Paint::solid(Color::BLACK)], ..Default::default() },
                );
                n.stroke.paints = vec![Paint::solid(Color::rgb(220, 30, 30))];
                n.stroke.weight = 6.0;
                n.stroke.align = align;
                doc.insert(fid, n).unwrap();
                vec![t]
            });
        }
    }

    fn styled_text(doc: &mut Document, fid: NodeId, content: &str, align: StrokeAlign, base: TextStyle, runs: Vec<(usize, usize, TextStyle)>) -> NodeId {
        let t = doc.alloc_id();
        let mut n = Node::text(t, Rect::new(20.0, 40.0, 440.0, 80.0), content, base);
        if let NodeKind::Text(tp) = &mut n.kind {
            tp.runs = runs.into_iter().map(|(start, end, style)| TextRun { start, end, style }).collect();
        }
        n.stroke.paints = vec![Paint::solid(Color::rgb(220, 30, 30))];
        n.stroke.weight = 6.0;
        n.stroke.align = align;
        doc.insert(fid, n).unwrap();
        t
    }

    fn inter_32() -> TextStyle {
        TextStyle { font_family: "Inter".into(), font_size: 32.0, fills: vec![Paint::solid(Color::BLACK)], ..Default::default() }
    }

    #[test]
    fn aligned_multi_run_text_stroke_matches_live_text() {
        for align in [StrokeAlign::Outside, StrokeAlign::Inside] {
            let tag = format!("{align:?}").to_lowercase();
            // the same style in two runs split inside a word
            assert_text_scenario_pixels(&format!("same_style_two_runs_{tag}"), |doc, fid| {
                let base = inter_32();
                vec![styled_text(doc, fid, "Aligned", align, base.clone(), vec![(0, 3, base.clone()), (3, 7, base)])]
            });
            // a bold run with the same fill as the rest
            assert_text_scenario_pixels(&format!("bold_run_same_fill_{tag}"), |doc, fid| {
                let base = inter_32();
                let bold = TextStyle { font_weight: 700, ..base.clone() };
                vec![styled_text(doc, fid, "Bolder", align, base, vec![(0, 4, bold)])]
            });
            // black, blue, black: the middle run paints over the outer ones
            assert_text_scenario_pixels(&format!("black_blue_black_{tag}"), |doc, fid| {
                let base = inter_32();
                let blue = TextStyle { fills: vec![Paint::solid(Color::rgb(30, 60, 220))], ..base.clone() };
                vec![styled_text(doc, fid, "Aligned", align, base.clone(), vec![(0, 2, base.clone()), (2, 4, blue), (4, 7, base)])]
            });
        }
    }

    fn outline_group_children(doc: Document, page: NodeId, text: NodeId) -> (App, Vec<Node>) {
        let mut app = app_on_new_page(doc, page);
        app.selection = vec![text];
        app.outline_stroke_selection();
        let group = app.doc.get(app.selection[0]).expect("the outline is selected").clone();
        assert!(matches!(group.kind, NodeKind::Group));
        let children = group.children.iter().map(|c| app.doc.get(*c).unwrap().clone()).collect();
        (app, children)
    }

    /// The run lengths of the layer kinds of an outlined group: `f` for a glyph fill, `s` for a baked stroke.
    fn layer_runs(children: &[Node]) -> Vec<(char, usize)> {
        let mut runs: Vec<(char, usize)> = Vec::new();
        for n in children {
            let kind = if n.name.ends_with(" stroke") { 's' } else { 'f' };
            match runs.last_mut() {
                Some((k, count)) if *k == kind => *count += 1,
                _ => runs.push((kind, 1)),
            }
        }
        runs
    }

    #[test]
    fn aligned_text_outline_paints_fills_then_strokes_style_by_style() {
        for align in [StrokeAlign::Inside, StrokeAlign::Outside] {
            let mut doc = Document::new("StyleOrder");
            let page = doc.add_page("Page 1");
            let fid = doc.alloc_id();
            doc.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 480.0, 160.0))).unwrap();
            let base = inter_32();
            let blue_fill = vec![Paint::solid(Color::rgb(30, 60, 220))];
            let blue = TextStyle { fills: blue_fill.clone(), ..base.clone() };
            let t = styled_text(&mut doc, fid, "Aligned", align, base, vec![(2, 4, blue)]);
            let (_app, children) = outline_group_children(doc, page, t);
            for c in &children {
                let NodeKind::Vector { paths, .. } = &c.kind else { panic!("{align:?}: {} is not a vector", c.name) };
                let cmds: usize = paths.iter().map(|p| p.cmds.len()).sum();
                assert!(cmds < 600, "{align:?}: {} holds {cmds} path commands", c.name);
                assert!(c.stroke.paints.is_empty(), "{align:?}: {} carries no live stroke", c.name);
            }
            // the base style (A l n e d) paints first: its five glyph fills, then its strokes; then the blue run (i g) the same way
            let runs = layer_runs(&children);
            let kinds: Vec<char> = runs.iter().map(|r| r.0).collect();
            assert_eq!(kinds, vec!['f', 's', 'f', 's'], "{align:?}: layer order {runs:?}");
            assert_eq!((runs[0].1, runs[2].1), (5, 2), "{align:?}: a fill vector per glyph {runs:?}");
            assert!(runs[1].1 >= 1 && runs[3].1 >= 1, "{align:?}: every style has a stroke {runs:?}");
            assert_eq!(children[runs[0].1 + runs[1].1].fills, blue_fill, "{align:?}: the blue fill follows the base strokes");
        }
    }

    #[test]
    fn aligned_text_outline_shares_a_stroke_vector_only_between_glyphs_whose_strokes_touch() {
        let outline = |content: &str, weight: f32| {
            let mut doc = Document::new("Touching");
            let page = doc.add_page("Page 1");
            let fid = doc.alloc_id();
            doc.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 480.0, 160.0))).unwrap();
            let t = styled_text(&mut doc, fid, content, StrokeAlign::Outside, inter_32(), vec![]);
            doc.get_mut(t).unwrap().stroke.weight = weight;
            let (_app, children) = outline_group_children(doc, page, t);
            layer_runs(&children)
        };
        assert_eq!(outline("II", 6.0), vec![('f', 2), ('s', 1)], "touching strokes share one vector");
        assert_eq!(outline("I I", 1.0), vec![('f', 2), ('s', 2)], "strokes apart stay separate");
    }

    /// What `outline_chain_line` returns: the pixel difference between the live text and the outlined group
    /// (`pixmap_diff_stats`), and every baked stroke vector as (name, left edge in the frame, right edge in the
    /// frame, path commands).
    struct ChainLine {
        diff: (u8, f64, f64, usize, u8, usize),
        strokes: Vec<(String, f32, f32, usize)>,
    }

    const CHAIN_LINE_FRAME_W: f32 = 1100.0;
    /// One line of 140 glyphs ("mn" x 70, Inter 10, stroke weight 2) in a 1100 x 60 frame. The text spans about
    /// 1030 px and never wraps, and the stroke of every glyph touches the next, so the whole line is one chain.
    const CHAIN_LINE_GLYPHS: usize = 140;

    /// Outline stroke of the chain line with a `paint` stroke aligned `align`, rendered at 2x as live text and as
    /// the outlined group.
    fn outline_chain_line(align: StrokeAlign, paint: Paint) -> ChainLine {
        let mut doc = Document::new("Chain");
        let page = doc.add_page("Page 1");
        let fid = doc.alloc_id();
        let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, CHAIN_LINE_FRAME_W, 60.0));
        frame.fills = vec![Paint::solid(Color::WHITE)];
        doc.insert(page, frame).unwrap();
        let content: String = "mn".chars().cycle().take(CHAIN_LINE_GLYPHS).collect();
        let style = TextStyle { font_size: 10.0, ..inter_32() };
        let t = doc.alloc_id();
        let mut text = Node::text(t, Rect::new(10.0, 10.0, 100.0, 40.0), &content, style);
        text.stroke.paints = vec![paint];
        text.stroke.weight = 2.0;
        text.stroke.align = align;
        doc.insert(fid, text).unwrap();
        let before = designos_render::render_node(&doc, fid, 2.0).expect("render before");
        let (app, children) = outline_group_children(doc, page, t);
        let group_x = app.doc.get(app.selection[0]).expect("the outline is selected").transform.tx;
        let strokes = children
            .iter()
            .filter(|n| n.name.ends_with(" stroke"))
            .map(|n| {
                let NodeKind::Vector { paths, .. } = &n.kind else { panic!("{align:?}: {} is not a vector", n.name) };
                let left = group_x + n.transform.tx;
                (n.name.clone(), left, left + n.size.x, paths.iter().map(|p| p.cmds.len()).sum())
            })
            .collect();
        let after = designos_render::render_node(&app.doc, fid, 2.0).expect("render after outline stroke");
        ChainLine { diff: pixmap_diff_stats(&before, &after), strokes }
    }

    /// The chain of the line is cut after 128 glyphs, and the cut lies inside the rendered frame (the second
    /// vector starts at about x = 948 of 1100), so the picture compares the seam of the two vectors with the canvas.
    fn assert_chain_line_cut_matches_live_text(align: StrokeAlign, paint: Paint) {
        let line = outline_chain_line(align, paint);
        let names: Vec<&str> = line.strokes.iter().map(|s| s.0.as_str()).collect();
        assert_eq!(line.strokes.len(), 2, "{align:?}: the chain is cut into two stroke vectors, got {names:?}");
        assert_eq!(line.strokes[1].0, "mnmnmnmnmnmn stroke", "{align:?}: the second vector holds the 12 glyphs after the cut");
        for (name, left, right, cmds) in &line.strokes {
            assert!(*left >= 0.0 && *right <= CHAIN_LINE_FRAME_W, "{align:?}: {name} spans {left}..{right} of a {CHAIN_LINE_FRAME_W} px frame");
            assert!(*cmds < 10_000, "{align:?}: {name} holds {cmds} path commands");
        }
        let cut_x = line.strokes[1].1;
        assert!(cut_x > 0.0 && cut_x < CHAIN_LINE_FRAME_W, "{align:?}: the cut at x = {cut_x} lies inside the frame");
        let (_max, _all, _ink, hard_px, non_edge, ink_px) = line.diff;
        assert!(ink_px > 0, "{align:?}: nothing rendered");
        assert_eq!((hard_px, non_edge), (0, 0), "{align:?}: the cut vectors still render as the canvas does");
    }

    #[test]
    fn aligned_text_outline_cuts_a_chain_of_touching_strokes_into_bounded_vectors() {
        for align in [StrokeAlign::Inside, StrokeAlign::Outside] {
            assert_chain_line_cut_matches_live_text(align, Paint::solid(Color::rgb(220, 30, 30)));
        }
    }

    #[test]
    fn aligned_text_outline_skips_the_fill_layer_of_a_run_without_fill() {
        for align in [StrokeAlign::Inside, StrokeAlign::Outside] {
            let mut doc = Document::new("NoFillRun");
            let page = doc.add_page("Page 1");
            let fid = doc.alloc_id();
            doc.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 480.0, 160.0))).unwrap();
            let base = inter_32();
            let none = TextStyle { fills: vec![], ..base.clone() };
            let t = styled_text(&mut doc, fid, "Aligned", align, base, vec![(3, 7, none)]);
            let (_app, children) = outline_group_children(doc, page, t);
            assert!(children.iter().all(|n| !n.fills.is_empty()), "{align:?}: no layer without paint");
            // fills for A, l, i only; the strokes of both styles remain, one after the other
            let runs = layer_runs(&children);
            assert_eq!(runs[0], ('f', 3), "{align:?}: {runs:?}");
            assert_eq!(runs.len(), 2, "{align:?}: {runs:?}");
            assert!(runs[1].0 == 's' && runs[1].1 >= 2, "{align:?}: a stroke vector for each style {runs:?}");
        }
    }

    const PARAGRAPH: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum. ";

    /// Outline stroke of a stroked Inter 16 paragraph of `chars` characters: wall time of the whole command, and the
    /// path commands and JSON bytes of what it inserted.
    fn outline_stroke_paragraph(chars: usize, align: StrokeAlign) -> (std::time::Duration, usize, usize) {
        let mut text = PARAGRAPH.repeat(chars / PARAGRAPH.len() + 1);
        text.truncate(chars);
        let mut doc = Document::new("Paragraph");
        let page = doc.add_page("Page 1");
        let tid = doc.alloc_id();
        let mut n = Node::text(
            tid,
            Rect::new(10.0, 10.0, 600.0, 4000.0),
            &text,
            TextStyle { font_family: "Inter".into(), font_size: 16.0, fills: vec![Paint::solid(Color::BLACK)], ..Default::default() },
        );
        n.stroke.paints = vec![Paint::solid(Color::rgb(220, 30, 30))];
        n.stroke.weight = 1.0;
        n.stroke.align = align;
        doc.insert(page, n).unwrap();
        let mut app = app_on_new_page(doc, page);
        app.selection = vec![tid];
        let started = std::time::Instant::now();
        app.outline_stroke_selection();
        let elapsed = started.elapsed();
        let mut stack = vec![app.selection[0]];
        let (mut cmds, mut json) = (0, 0);
        while let Some(id) = stack.pop() {
            let node = app.doc.get(id).expect("the outline exists");
            if let NodeKind::Vector { paths, .. } = &node.kind {
                cmds += paths.iter().map(|p| p.cmds.len()).sum::<usize>();
            }
            json += serde_json::to_string(node).expect("node serializes").len();
            stack.extend(node.children.iter().copied());
        }
        (elapsed, cmds, json)
    }

    /// Checks the outline stroke of a `chars` characters paragraph against `budget_ms` (times `scale`) and against the
    /// size of the centered outline: Inside within 2x; Outside within 3x, which is a guard against growth, not the target.
    fn assert_paragraph_outline_stroke_budget(chars: usize, budget_ms: u64, scale: u64) {
        let budget = std::time::Duration::from_millis(budget_ms * scale);
        // the centered outline is one vector per glyph with the live stroke: the size to compare with (this run also loads the fonts)
        let (center_time, center_cmds, center_json) = outline_stroke_paragraph(chars, StrokeAlign::Center);
        println!("PARAGRAPH {chars} chars Center: {center_time:?}, {center_cmds} path commands, {center_json} JSON bytes");
        let mut failures = Vec::new();
        for align in [StrokeAlign::Inside, StrokeAlign::Outside] {
            // a baked stroke has two outlines per contour next to the one of the fill
            let size_limit = if align == StrokeAlign::Inside { 2 } else { 3 };
            let (time, cmds, json) = outline_stroke_paragraph(chars, align);
            println!("PARAGRAPH {chars} chars {align:?}: {time:?}, {cmds} path commands, {json} JSON bytes");
            if time > budget {
                failures.push(format!("{align:?}: outline stroke of {chars} chars took {time:?}, budget {budget:?}"));
            }
            if cmds > size_limit * center_cmds {
                failures.push(format!("{align:?}: {cmds} path commands against {center_cmds} for the centered outline (limit {size_limit}x)"));
            }
            if json > size_limit * center_json {
                failures.push(format!("{align:?}: {json} JSON bytes against {center_json} for the centered outline (limit {size_limit}x)"));
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }

    /// The default suite runs unoptimized: its time budget is 15 times the release one.
    #[test]
    fn aligned_text_outline_stroke_of_a_paragraph_is_fast_and_small() {
        let scale = if cfg!(debug_assertions) || std::env::var("CI").is_ok() { 15 } else { 1 };
        assert_paragraph_outline_stroke_budget(1000, 150, scale);
    }

    /// The release budget: 150 ms for 1000 characters, 400 ms for 3000. The crate's own test code needs
    /// `debug_assertions`, so a release run turns them on for `designos-app` and for `egui` only; every geometry crate
    /// stays optimized without them. Run:
    /// `cargo test --release --config 'profile.release.package.egui.debug-assertions=true' --config 'profile.release.package.designos-app.debug-assertions=true' -p designos-app --bin designos aligned_text_outline_stroke_of_paragraphs_meets_the_release_budget -- --ignored --nocapture`
    #[test]
    #[ignore = "release budget; see the command in the doc comment"]
    fn aligned_text_outline_stroke_of_paragraphs_meets_the_release_budget() {
        assert_paragraph_outline_stroke_budget(1000, 150, 1);
        assert_paragraph_outline_stroke_budget(3000, 400, 1);
    }

    #[test]
    fn pixel_test_converted_text_vectors_match_original_text_render() {
        // 1. Decorated (underline) and two-run text preserve underline geometry, group transform, and glyph alignment.
        assert_text_scenario_pixels("decor_and_runs", |doc, fid| {
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
            let mut n1 = Node::text(t1, Rect::new(20.0, 16.0, 280.0, 44.0), "Vector Text", s1);
            if let NodeKind::Text(tp) = &mut n1.kind {
                tp.auto_resize = TextAutoResize::None;
            }
            doc.insert(fid, n1).unwrap();
            let t2 = doc.alloc_id();
            let n2 = two_run_text_node(t2, Color::rgb(210, 45, 45), Color::rgb(25, 135, 84));
            doc.insert(fid, n2).unwrap();
            vec![t1, t2]
        });

        // 2. Stroked text with default StrokeAlign::Inside keeps its alignment when converted.
        assert_text_scenario_pixels("stroked_default_inside", |doc, fid| {
            let t = doc.alloc_id();
            let s = TextStyle {
                font_family: "Inter".into(),
                font_weight: 700,
                font_size: 64.0,
                fills: vec![Paint::solid(Color::rgb(250, 220, 60))],
                ..Default::default()
            };
            let mut n = Node::text(t, Rect::new(20.0, 20.0, 440.0, 120.0), "Stroked Ag", s);
            n.stroke.paints = vec![Paint::solid(Color::rgb(200, 20, 20))];
            n.stroke.weight = 4.0;
            doc.insert(fid, n).unwrap();
            vec![t]
        });

        // 3. Linear gradient text remaps gradient handles across per-glyph and single-run boxes.
        assert_text_scenario_pixels("linear_gradient", |doc, fid| {
            let t = doc.alloc_id();
            let mut s = TextStyle { font_family: "Inter".into(), font_weight: 800, font_size: 48.0, ..Default::default() };
            s.fills = vec![Paint {
                kind: PaintKind::LinearGradient {
                    handles: [Vec2::new(0.0, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.0, 1.0)],
                    stops: vec![GradientStop { pos: 0.0, color: Color::rgb(230, 30, 30) }, GradientStop { pos: 1.0, color: Color::rgb(30, 60, 230) }],
                },
                opacity: 1.0,
                visible: true,
                blend: BlendMode::Normal,
            }];
            doc.insert(fid, Node::text(t, Rect::new(20.0, 30.0, 420.0, 80.0), "Gradient words", s)).unwrap();
            vec![t]
        });

        // 4. Bulleted and numbered list text includes list markers in both Flatten and Outline stroke.
        assert_text_scenario_pixels("lists", |doc, fid| {
            let t = doc.alloc_id();
            let content = "Bullet one\nNumber one";
            let s = TextStyle {
                font_family: "Inter".into(),
                font_weight: 500,
                font_size: 22.0,
                fills: vec![Paint::solid(Color::rgb(20, 24, 31))],
                ..Default::default()
            };
            let mut n = Node::text(t, Rect::new(20.0, 16.0, 400.0, 120.0), content, s);
            if let NodeKind::Text(tp) = &mut n.kind {
                let first_end = "Bullet one".len();
                tp.lists = vec![
                    ListRange { start: 0, end: first_end, kind: ListKind::Bullet, level: 0 },
                    ListRange { start: first_end + 1, end: content.len(), kind: ListKind::Numbered, level: 0 },
                ];
            }
            doc.insert(fid, n).unwrap();
            vec![t]
        });

        // 5. Semi-transparent shadowed text applies opacity and drop shadow once without doubling on glyph vectors.
        assert_text_scenario_pixels("opacity_shadow", |doc, fid| {
            let t = doc.alloc_id();
            let s = TextStyle {
                font_family: "Inter".into(),
                font_weight: 800,
                font_size: 44.0,
                fills: vec![Paint::solid(Color::rgb(30, 30, 30))],
                ..Default::default()
            };
            let mut n = Node::text(t, Rect::new(20.0, 25.0, 420.0, 90.0), "Shadowed AV", s);
            n.opacity = 0.5;
            n.effects = vec![Effect::DropShadow {
                color: Color::rgba(0, 0, 0, 120),
                offset: Vec2::new(3.0, 4.0),
                radius: 4.0,
                spread: 0.0,
                visible: true,
                show_behind: false,
                blend: BlendMode::Normal,
            }];
            doc.insert(fid, n).unwrap();
            vec![t]
        });
    }

    #[test]
    fn deferred_flatten_converts_invoked_text_not_later_selected_rectangle() {
        let mut doc = Document::new("DeferredFlattenTarget");
        let page = doc.add_page("Page 1");
        let tid = doc.alloc_id();
        let style = TextStyle { font_family: "SlowFntB001".into(), font_size: 20.0, ..Default::default() };
        doc.insert(page, Node::text(tid, Rect::new(0.0, 0.0, 120.0, 32.0), "Target Text", style)).unwrap();
        let rid = doc.alloc_id();
        doc.insert(page, Node::rectangle(rid, Rect::new(160.0, 0.0, 80.0, 60.0))).unwrap();
        let rect = doc.get(rid).cloned().unwrap();

        let mut app = app_on_new_page(doc, page);
        app.fonts = crate::fonts::testkit::serving_with_delay(&["SlowFntB001"], std::time::Duration::from_millis(250));

        app.selection = vec![tid];
        app.flatten_selection();
        assert!(app.doc.get(tid).is_some(), "text stays while font is loading");

        // User selects the rectangle while the font is still downloading.
        app.selection = vec![rid];
        settle_app_fonts(&mut app);

        assert_eq!(app.doc.get(rid), Some(&rect), "later-selected rectangle must remain unchanged");
        assert!(app.doc.get(tid).is_none(), "originally targeted text must be converted");
        assert_eq!(app.selection.len(), 1);
        assert_ne!(app.selection[0], rid, "selection must be the produced vector, not the rectangle");
        assert!(matches!(app.doc.get(app.selection[0]).unwrap().kind, NodeKind::Vector { .. }));
    }

    #[test]
    fn deferred_outline_stroke_converts_invoked_text_not_later_selected_rectangle() {
        let mut doc = Document::new("DeferredOutlineTarget");
        let page = doc.add_page("Page 1");
        let tid = doc.alloc_id();
        let style = TextStyle { font_family: "SlowFntC001".into(), font_size: 20.0, ..Default::default() };
        doc.insert(page, Node::text(tid, Rect::new(0.0, 0.0, 120.0, 32.0), "Target Outline", style)).unwrap();
        let rid = doc.alloc_id();
        let mut rect_node = Node::rectangle(rid, Rect::new(160.0, 0.0, 80.0, 60.0));
        rect_node.fills.clear();
        rect_node.stroke.paints = vec![Paint::solid(Color::BLACK)];
        rect_node.stroke.weight = 4.0;
        doc.insert(page, rect_node).unwrap();
        let rect = doc.get(rid).cloned().unwrap();

        let mut app = app_on_new_page(doc, page);
        app.fonts = crate::fonts::testkit::serving_with_delay(&["SlowFntC001"], std::time::Duration::from_millis(250));

        app.selection = vec![tid];
        app.outline_stroke_selection();
        assert!(app.doc.get(tid).is_some(), "text stays while font is loading");

        // User selects the stroked rectangle while the font is still downloading.
        app.selection = vec![rid];
        settle_app_fonts(&mut app);

        assert_eq!(app.doc.get(rid), Some(&rect), "later-selected stroked rectangle must remain unchanged");
        assert!(app.doc.get(tid).is_none(), "originally targeted text must be converted");
        assert_eq!(app.selection.len(), 1);
        assert_ne!(app.selection[0], rid, "selection must be the produced group, not the rectangle");
        assert!(matches!(app.doc.get(app.selection[0]).unwrap().kind, NodeKind::Group));
    }

    #[test]
    fn two_deferred_text_commands_both_convert_in_order() {
        let mut doc = Document::new("TwoDeferredCommands");
        let page = doc.add_page("Page 1");
        let t_fail = doc.alloc_id();
        let s_fail = TextStyle { font_family: "MissingFont".into(), font_size: 20.0, ..Default::default() };
        doc.insert(page, Node::text(t_fail, Rect::new(0.0, 0.0, 120.0, 32.0), "Fail", s_fail)).unwrap();

        let mut app = app_on_new_page(doc, page);
        app.fonts = crate::fonts::testkit::serving_with_delay(&["SlowFntD001", "SlowFntD002"], std::time::Duration::from_millis(250));

        // First mark MissingFont as already failed before introducing the slow-font text layers.
        app.selection = vec![t_fail];
        app.flatten_selection();
        settle_app_fonts(&mut app);
        assert!(app.doc.get(t_fail).is_some());

        let t1 = app.doc.alloc_id();
        let s1 = TextStyle { font_family: "SlowFntD001".into(), font_size: 20.0, ..Default::default() };
        app.doc.insert(page, Node::text(t1, Rect::new(0.0, 40.0, 120.0, 32.0), "First", s1)).unwrap();

        let t2 = app.doc.alloc_id();
        let s2 = TextStyle { font_family: "SlowFntD002".into(), font_size: 20.0, ..Default::default() };
        app.doc.insert(page, Node::text(t2, Rect::new(0.0, 80.0, 120.0, 32.0), "Second", s2)).unwrap();

        // Queue command 1 (Flatten on t1) and command 2 (Outline stroke on t2) while fonts load.
        app.selection = vec![t1];
        app.flatten_selection();
        app.selection = vec![t2];
        app.outline_stroke_selection();

        // A command on an already-failed face toasts immediately and leaves the waiting queue intact.
        app.toast = None;
        app.selection = vec![t_fail];
        app.flatten_selection();
        assert!(app.toast.as_ref().is_some_and(|t| t.error && t.text.starts_with("Font unavailable: MissingFont 400:")));

        assert!(app.doc.get(t1).is_some() && app.doc.get(t2).is_some(), "both texts still waiting for slow fonts");
        settle_app_fonts(&mut app);

        assert!(app.doc.get(t1).is_none(), "first deferred text must be converted");
        assert!(app.doc.get(t2).is_none(), "second deferred text must be converted");
        assert_eq!(app.hist.undo_depth(), 2, "both deferred commands committed as separate undo steps");
        let second_out = app.selection[0];
        assert!(matches!(app.doc.get(second_out).unwrap().kind, NodeKind::Group), "second command (outline stroke) ran last and set selection");

        // Undoing once undoes the second command (restoring t2) while keeping the first command's vector in place.
        app.undo();
        assert!(app.doc.get(t2).is_some(), "undoing once restores the second text");
        assert!(app.doc.get(t1).is_none(), "first text remains converted after undoing only the second command");
    }

    #[test]
    fn deferred_command_on_deleted_text_edits_nothing_without_panic() {
        let mut doc = Document::new("DeletedTarget");
        let page = doc.add_page("Page 1");
        let tid = doc.alloc_id();
        let style = TextStyle { font_family: "SlowFntE001".into(), font_size: 20.0, ..Default::default() };
        doc.insert(page, Node::text(tid, Rect::new(0.0, 0.0, 120.0, 32.0), "To Delete", style)).unwrap();
        let rid = doc.alloc_id();
        doc.insert(page, Node::rectangle(rid, Rect::new(160.0, 0.0, 80.0, 60.0))).unwrap();
        let rect = doc.get(rid).cloned().unwrap();

        let mut app = app_on_new_page(doc, page);
        app.fonts = crate::fonts::testkit::serving_with_delay(&["SlowFntE001"], std::time::Duration::from_millis(250));

        app.selection = vec![tid];
        app.flatten_selection();
        assert!(app.doc.get(tid).is_some());

        // Delete the stored text node and select the rectangle before the font finishes loading.
        app.delete_selection();
        assert!(app.doc.get(tid).is_none());
        let undo_after_delete = app.hist.undo_depth();
        app.selection = vec![rid];

        settle_app_fonts(&mut app);

        assert_eq!(app.hist.undo_depth(), undo_after_delete, "no edit may happen when stored ids were deleted");
        assert_eq!(app.doc.get(rid), Some(&rect), "rectangle must not be touched");
        assert_eq!(app.selection, vec![rid]);
    }

    #[test]
    fn deferred_command_cancelled_when_pen_edit_is_open_at_arrival() {
        let mut doc = Document::new("PenEditOpenAtArrival");
        let page = doc.add_page("Page 1");
        let tid = doc.alloc_id();
        let style = TextStyle { font_family: "SlowFntF001".into(), font_size: 20.0, ..Default::default() };
        doc.insert(page, Node::text(tid, Rect::new(0.0, 0.0, 120.0, 32.0), "Pen Wait", style)).unwrap();
        let rid = doc.alloc_id();
        doc.insert(page, Node::rectangle(rid, Rect::new(160.0, 0.0, 80.0, 60.0))).unwrap();
        let rect = doc.get(rid).cloned().unwrap();

        let mut app = app_on_new_page(doc, page);
        app.fonts = crate::fonts::testkit::serving_with_delay(&["SlowFntF001"], std::time::Duration::from_millis(250));

        app.selection = vec![tid];
        app.flatten_selection();
        assert!(app.doc.get(tid).is_some());

        // Open vector pen edit on the rectangle before the font arrives.
        assert!(crate::canvas::vector::enter(&mut app, rid));
        assert_eq!(app.canvas.vedit.as_ref().map(|v| v.id), Some(rid));

        settle_app_fonts(&mut app);

        assert_eq!(app.canvas.vedit.as_ref().map(|v| v.id), Some(rid), "pen edit must stay open on the rectangle");
        assert!(app.doc.get(tid).is_some(), "text must not be converted while pen edit is open");
        assert_eq!(app.doc.get(rid), Some(&rect), "rectangle in pen edit must not be flattened");
        assert_eq!(app.hist.undo_depth(), 0);
        assert!(app.toast.as_ref().is_some_and(|t| t.text.contains("cancelled")), "cancellation toast shown");
    }

    #[test]
    fn deferred_flatten_rechecks_live_text_font_when_family_changes_before_arrival() {
        let mut doc = Document::new("LiveFontRecheck");
        let page = doc.add_page("Page 1");
        let tid = doc.alloc_id();
        let style_a = TextStyle { font_family: "SlowFntG001".into(), font_size: 20.0, ..Default::default() };
        doc.insert(page, Node::text(tid, Rect::new(0.0, 0.0, 120.0, 32.0), "Font Switch", style_a)).unwrap();

        let mut app = app_on_new_page(doc, page);
        app.fonts = crate::fonts::testkit::serving_with_delay(&["SlowFntG001", "SlowFntG002"], std::time::Duration::from_millis(250));

        app.selection = vec![tid];
        app.flatten_selection();
        assert!(app.doc.get(tid).is_some(), "text stays while family A is loading");

        // Before family A arrives, change the text to family B (not loaded).
        app.edit_nodes(&[tid], |n| {
            if let NodeKind::Text(tp) = &mut n.kind {
                tp.style.font_family = "SlowFntG002".into();
            }
        });

        let req_a = designos_fonts::FontRequest::new("SlowFntG001", 400, false);
        let req_b = designos_fonts::FontRequest::new("SlowFntG002", 400, false);
        for _ in 0..6000 {
            app.poll_fonts();
            if app.fonts.registered().contains(&req_a) && !app.text_refit_pending() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(app.fonts.registered().contains(&req_a), "family A arrived");
        assert!(!app.fonts.registered().contains(&req_b), "family B has not arrived yet");
        assert!(app.doc.get(tid).is_some(), "text must still exist when only family A has arrived");

        settle_app_fonts(&mut app);
        assert!(app.fonts.registered().contains(&req_b), "family B arrived");
        assert!(app.doc.get(tid).is_none(), "text must be converted once family B arrives");
        assert!(matches!(app.doc.get(app.selection[0]).unwrap().kind, NodeKind::Vector { .. }));
    }

    #[test]
    fn deferred_flatten_cancelled_when_one_text_moves_under_another_parent() {
        let mut doc = Document::new("ReparentedSibling");
        let page = doc.add_page("Page 1");
        let t1 = doc.alloc_id();
        let style = TextStyle { font_family: "SlowFntH001".into(), font_size: 20.0, ..Default::default() };
        doc.insert(page, Node::text(t1, Rect::new(0.0, 0.0, 120.0, 32.0), "Text One", style.clone())).unwrap();
        let t2 = doc.alloc_id();
        doc.insert(page, Node::text(t2, Rect::new(0.0, 40.0, 120.0, 32.0), "Text Two", style)).unwrap();
        let fid = doc.alloc_id();
        doc.insert(page, Node::frame(fid, Rect::new(200.0, 0.0, 160.0, 100.0))).unwrap();

        let mut app = app_on_new_page(doc, page);
        app.fonts = crate::fonts::testkit::serving_with_delay(&["SlowFntH001"], std::time::Duration::from_millis(250));

        app.selection = vec![t1, t2];
        app.flatten_selection();
        assert!(app.doc.get(t1).is_some() && app.doc.get(t2).is_some());

        // Move one of the two texts under another parent before the font arrives.
        app.toast = None;
        app.move_to_parent(&[t2], fid, 0);
        assert_eq!(app.doc.get(t1).and_then(|n| n.parent), Some(page));
        assert_eq!(app.doc.get(t2).and_then(|n| n.parent), Some(fid));
        let undo_after_move = app.hist.undo_depth();

        settle_app_fonts(&mut app);

        assert!(matches!(app.doc.get(t1).map(|n| &n.kind), Some(NodeKind::Text(_))), "first text must not be converted");
        assert!(matches!(app.doc.get(t2).map(|n| &n.kind), Some(NodeKind::Text(_))), "reparented text must not be converted");
        assert_eq!(app.hist.undo_depth(), undo_after_move, "no flatten commit may occur after reparenting");
        assert!(app.toast.as_ref().is_some_and(|t| t.text.contains("cancelled")), "cancellation toast must be shown");
    }
}
