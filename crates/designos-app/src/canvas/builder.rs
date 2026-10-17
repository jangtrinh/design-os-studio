//! Shape builder: with overlapping shapes selected, every region their
//! outlines cut out highlights on hover. Click (or drag across) regions to
//! merge them into one shape, Alt-click or Alt-drag to delete them. Leaving the
//! tool (Enter, Esc or another tool) replaces the shapes with vector networks.

use super::*;
use crate::edit::Tx;
use crate::theme;
use designos_doc::{Command, Node, NodeKind};
use designos_render::vectorops::{arrange, node_paths, paths_to_shapes, resolve_builder, shapes_contain, shapes_to_paths, transform_shapes, Cell, Shapes};
use egui::{Painter, Pos2, Stroke};

/// A brush stroke over regions: merge, or delete with Alt.
pub struct Brush {
    pub subtract: bool,
    pub cells: Vec<usize>,
}

pub struct Builder {
    /// The shapes being built from, bottom to top.
    pub ids: Vec<NodeId>,
    /// Their outlines in page space.
    pub originals: Vec<Shapes>,
    pub cells: Vec<Cell>,
    /// Merge group of every region (`None`: stays part of its shape).
    pub group: Vec<Option<usize>>,
    pub deleted: Vec<bool>,
    pub hover: Option<usize>,
    pub brush: Option<Brush>,
    pub next_group: usize,
    /// Keep the hovered region whatever the pointer does until the first press (screenshot fixture).
    pub pin_hover: bool,
}

impl Builder {
    pub fn edited(&self) -> bool {
        self.deleted.iter().any(|d| *d) || self.group.iter().any(|g| g.is_some())
    }

    /// The smallest live region under `p` (page space).
    pub fn cell_at(&self, p: DV) -> Option<usize> {
        (0..self.cells.len())
            .filter(|i| !self.deleted[*i] && shapes_contain(&self.cells[*i].shape, p))
            .min_by(|a, b| self.cells[*a].area.total_cmp(&self.cells[*b].area))
    }
}

/// Starts the tool on the current selection. False (with a toast) when fewer than two shapes are selected.
pub fn start(app: &mut App) -> bool {
    let ids: Vec<NodeId> = app.same_parent_tops().into_iter().filter(|i| app.doc.get(*i).is_some_and(|n| !matches!(n.kind, NodeKind::Text(_)))).collect();
    if ids.len() < 2 {
        app.show_toast("Select two or more overlapping shapes to use the shape builder", false);
        return false;
    }
    let mut originals = vec![];
    let mut kept = vec![];
    for id in ids {
        let Some(n) = app.doc.get(id) else { continue };
        let Some(paths) = node_paths(&app.doc, n) else { continue };
        let mut shapes = paths_to_shapes(&paths, 0.05);
        if shapes.is_empty() {
            continue;
        }
        transform_shapes(&mut shapes, &app.doc.world_transform(id));
        originals.push(shapes);
        kept.push(id);
    }
    if kept.len() < 2 {
        app.show_toast("Select two or more overlapping shapes to use the shape builder", false);
        return false;
    }
    let cells = arrange(&originals);
    let n = cells.len();
    app.canvas.builder =
        Some(Builder { ids: kept, originals, cells, group: vec![None; n], deleted: vec![false; n], hover: None, brush: None, next_group: 0, pin_hover: false });
    true
}

pub fn hover(app: &mut App, page: Option<DV>) {
    let Some(b) = app.canvas.builder.as_mut() else { return };
    if b.brush.is_some() || b.pin_hover {
        return;
    }
    b.hover = page.and_then(|p| b.cell_at(p));
}

pub fn press(app: &mut App, page: DV, alt: bool) {
    let Some(b) = app.canvas.builder.as_mut() else { return };
    b.brush = None;
    b.pin_hover = false;
    if let Some(c) = b.cell_at(page) {
        b.brush = Some(Brush { subtract: alt, cells: vec![c] });
        b.hover = Some(c);
        app.canvas.inter = Interaction::Builder;
    }
}

pub fn drag(app: &mut App, page: DV) {
    let Some(b) = app.canvas.builder.as_mut() else { return };
    let hit = b.cell_at(page);
    b.hover = hit;
    if let (Some(c), Some(brush)) = (hit, b.brush.as_mut()) {
        if !brush.cells.contains(&c) {
            brush.cells.push(c);
        }
    }
}

pub fn release(app: &mut App) {
    let Some(b) = app.canvas.builder.as_mut() else { return };
    let Some(brush) = b.brush.take() else { return };
    if brush.subtract {
        for c in brush.cells {
            b.deleted[c] = true;
            b.group[c] = None;
        }
    } else {
        // everything the brush touched, and whatever was already merged with it, becomes one shape
        let touched: Vec<usize> = brush.cells.iter().filter_map(|c| b.group[*c]).collect();
        let g = touched.first().copied().unwrap_or_else(|| {
            b.next_group += 1;
            b.next_group
        });
        for i in 0..b.cells.len() {
            if b.group[i].is_some_and(|x| touched.contains(&x)) {
                b.group[i] = Some(g);
            }
        }
        for c in brush.cells {
            b.group[c] = Some(g);
        }
    }
    b.hover = None;
    app.changed();
}

/// Drops the brush stroke in progress.
pub fn cancel_brush(app: &mut App) {
    if let Some(b) = app.canvas.builder.as_mut() {
        b.brush = None;
    }
    if matches!(app.canvas.inter, Interaction::Builder) {
        app.canvas.inter = Interaction::None;
    }
}

/// Applies the result to the document as one undo step and leaves the tool.
pub fn finish(app: &mut App) {
    if matches!(app.canvas.inter, Interaction::Builder) {
        app.canvas.inter = Interaction::None;
    }
    let Some(b) = app.canvas.builder.take() else { return };
    if !b.edited() {
        return;
    }
    let res = resolve_builder(&b.originals, &b.cells, &b.group, &b.deleted);
    let Some(&top) = b.ids.last() else { return };
    let Some(parent) = app.doc.get(top).and_then(|n| n.parent) else { return };
    let Some(inv) = app.doc.world_transform(parent).invert() else { return };
    let mut tx = Tx::new();
    let mut made: Vec<NodeId> = vec![];
    let build = |app: &mut App, shapes: &Shapes, template_id: NodeId, keep_name: bool| -> Option<Node> {
        let mut sh = shapes.clone();
        transform_shapes(&mut sh, &inv);
        let paths = shapes_to_paths(&sh, 0.1);
        if paths.is_empty() {
            return None;
        }
        let mut template = app.doc.get(template_id)?.clone();
        template.transform = Affine::IDENTITY;
        let id = app.doc.alloc_id();
        let mut node = crate::vecops::vector_node(id, &paths, &template);
        if !keep_name {
            node.name = app.unique_name("Vector");
        }
        node.component = Default::default();
        Some(node)
    };
    let index_of = |app: &App, id: NodeId| app.doc.get(parent).and_then(|p| p.children.iter().position(|c| *c == id));
    // merged shapes go on top of everything they came from
    for (shapes, style) in &res.pieces {
        let Some(node) = build(app, shapes, b.ids[*style], false) else { continue };
        let Some(top_index) = index_of(app, top) else { continue };
        let id = node.id;
        if tx.cmd(&mut app.doc, Command::Insert { parent, index: top_index + 1, nodes: vec![node] }).is_ok() {
            made.push(id);
        }
    }
    // what is left of each touched shape replaces it
    for (i, rest) in res.remainders.iter().enumerate() {
        let Some(rest) = rest else { continue };
        let orig = b.ids[i];
        let node = if rest.is_empty() { None } else { build(app, rest, orig, true) };
        if let (Some(node), Some(at)) = (node, index_of(app, orig)) {
            let id = node.id;
            if tx.cmd(&mut app.doc, Command::Insert { parent, index: at + 1, nodes: vec![node] }).is_ok() {
                made.push(id);
                // Agent edits held while this tool was open still name the layer it replaced.
                app.terminal.live_patch.state.record_replacement(orig, id);
            }
        }
        let _ = tx.cmd(&mut app.doc, Command::Remove { id: orig });
    }
    app.run_tx(tx);
    app.selection = made;
}

pub fn overlay(app: &App, p: &Painter, rect: ERect) {
    let Some(b) = &app.canvas.builder else { return };
    let to_scr = |q: [f64; 2]| app.view.to_screen(rect.min, DV::new(q[0] as f32, q[1] as f32));
    let col = theme::CANVAS_SELECTION;
    let brushed = |i: usize| b.brush.as_ref().is_some_and(|br| br.cells.contains(&i));
    for (i, c) in b.cells.iter().enumerate() {
        let contours: Vec<Vec<Pos2>> = c.shape.iter().flat_map(|sh| sh.iter().map(|ct| ct.iter().map(|q| to_scr(*q)).collect())).collect();
        let Some((outer, holes)) = contours.split_first() else { continue };
        if b.deleted[i] {
            for ct in &contours {
                p.add(egui::Shape::closed_line(ct.clone(), Stroke::new(1.0, theme::DANGER.gamma_multiply(0.6))));
            }
            continue;
        }
        let hot = b.hover == Some(i) || brushed(i);
        let alpha = if brushed(i) && b.brush.as_ref().is_some_and(|br| br.subtract) {
            0.0
        } else if hot {
            0.45
        } else if b.group[i].is_some() {
            0.28
        } else {
            0.0
        };
        if alpha > 0.0 {
            super::polyfill::fill(p, outer, holes, col.gamma_multiply(alpha));
        }
        if brushed(i) && b.brush.as_ref().is_some_and(|br| br.subtract) {
            super::polyfill::fill(p, outer, holes, theme::DANGER.gamma_multiply(0.4));
        }
        let w = if hot { 2.0 } else { 1.0 };
        for ct in &contours {
            p.add(egui::Shape::closed_line(ct.clone(), Stroke::new(w, col.gamma_multiply(if hot { 1.0 } else { 0.55 }))));
        }
    }
}
