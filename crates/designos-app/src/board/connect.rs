//! Connectors: magnet dots on hovered nodes, dragging a connector out of a dot (or with the
//! Connector tool), dropping it on another node, and re-routing connectors whenever the nodes
//! they join change.

use crate::app::App;
use crate::canvas::{hit, interact, Interaction};
use crate::edit::{Edit, Tx};
use crate::theme;
use crate::util::*;
use designos_doc::*;

thread_local! {
    static DETACH_TOAST: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn notify_detached() {
    DETACH_TOAST.with(|c| c.set(true));
}

pub fn take_detach_toast() -> Option<&'static str> {
    DETACH_TOAST.with(|c| c.replace(false)).then_some("Connector detached from moved layer")
}

/// A connector being dragged out. Its transaction holds the insertion and every preview step.
pub struct ConnectDrag {
    pub tx: Tx,
    pub id: NodeId,
    /// The pointer has left the starting point (a plain click creates nothing).
    pub moved: bool,
    /// The node the connector starts at.
    pub from: Option<NodeId>,
    /// Where the drag started (page space).
    pub origin: Vec2,
}

fn candidate(app: &App, id: NodeId) -> bool {
    connectable_node(&app.doc, id)
}

fn is_whiteboard_dot_target(app: &App, id: NodeId) -> bool {
    app.doc.get(id).is_some_and(|n| {
        matches!(n.kind, NodeKind::Sticky(_) | NodeKind::ShapeWithText(_) | NodeKind::Stamp { .. }) || (app.is_board() && matches!(n.kind, NodeKind::Text(_)))
    })
}

fn dot_allowed(app: &App, id: NodeId) -> bool {
    candidate(app, id)
        && match app.tool {
            crate::canvas::tools::Tool::Connector => true,
            crate::canvas::tools::Tool::Move => is_whiteboard_dot_target(app, id) || app.selection.as_slice() == [id],
            _ => false,
        }
}

/// The four magnet dots of a node in page space: just outside the middle of each side.
pub fn dot_positions(app: &App, id: NodeId) -> Vec<(Magnet, Vec2)> {
    let r = world_aabb(&app.doc, id);
    let g = theme::BOARD_MAGNET_GAP / app.view.zoom;
    let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
    vec![(Magnet::Top, v2(cx, r.y - g)), (Magnet::Right, v2(r.x + r.w + g, cy)), (Magnet::Bottom, v2(cx, r.y + r.h + g)), (Magnet::Left, v2(r.x - g, cy))]
}

/// The dot of the hovered node under screen point `pos`, if any.
pub fn dot_hit(app: &App, rect: egui::Rect, pos: egui::Pos2) -> Option<(NodeId, Magnet)> {
    let id = app.board.dots.filter(|n| dot_allowed(app, *n))?;
    dot_positions(app, id).into_iter().find(|(_, p)| (app.view.to_screen(rect.min, *p) - pos).length() <= theme::BOARD_MAGNET_HIT).map(|(m, _)| (id, m))
}

/// Keeps `board.dots` on the node under the pointer (or the one whose dots the pointer is reaching for).
pub fn update_hover(app: &mut App, page: Option<Vec2>) {
    if !app.wb_connectors_enabled()
        || app.canvas.vedit.is_some()
        || !matches!(app.tool, crate::canvas::tools::Tool::Move | crate::canvas::tools::Tool::Connector)
    {
        app.board.dots = None;
        return;
    }
    if matches!(app.canvas.inter, Interaction::Connect(_)) {
        return;
    }
    if !matches!(app.canvas.inter, Interaction::None) {
        app.board.dots = None;
        return;
    }
    let single_sel = (app.tool == crate::canvas::tools::Tool::Move)
        .then(|| match app.selection.as_slice() {
            &[s] if candidate(app, s) => Some(s),
            _ => None,
        })
        .flatten();
    let Some(page) = page else {
        app.board.dots = single_sel;
        return;
    };
    let hit = app.hover.filter(|id| dot_allowed(app, *id));
    let near_current = app.board.dots.filter(|n| dot_allowed(app, *n)).filter(|n| {
        let r = world_aabb(&app.doc, *n);
        let m = (theme::BOARD_MAGNET_GAP + theme::BOARD_MAGNET_RADIUS + 8.0) / app.view.zoom;
        Rect::new(r.x - m, r.y - m, r.w + 2.0 * m, r.h + 2.0 * m).contains(page)
    });
    app.board.dots = hit.or(near_current).or(single_sel);
}

/// Node (and magnet) a connector dropped at `page` would attach to: the node under the pointer,
/// at the nearest magnet dot when the pointer is close to one, else on its side facing the start.
pub(crate) fn drop_target(app: &App, page: Vec2, deep: bool, exclude: &[NodeId]) -> Option<(NodeId, Magnet)> {
    let tol = 2.0 / app.view.zoom;
    let entered = interact::entered(app);
    let hit_id = match interact::active_index(app) {
        Some(idx) => hit::hit_test_excluding_with_index(&app.doc, app.page, page, tol, deep, &entered, exclude, Some(idx)),
        None => hit::hit_test_excluding(&app.doc, app.page, page, tol, deep, &entered, exclude),
    };
    let id = hit_id.filter(|id| candidate(app, *id))?;
    let near = dot_positions(app, id)
        .into_iter()
        .map(|(m, p)| (m, dist(p, page) * app.view.zoom))
        .filter(|(_, d)| *d <= theme::BOARD_MAGNET_HIT * 2.5)
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    Some((id, near.map(|(m, _)| m).unwrap_or(Magnet::Auto)))
}

fn default_connector_ink(app: &App, home: NodeId) -> Color {
    if app.is_board() {
        return app.board.ink;
    }
    let surface = app
        .doc
        .get(home)
        .and_then(|n| match &n.kind {
            NodeKind::Page { background } => Some(*background),
            _ => n.fills.iter().find(|p| p.visible).and_then(|p| match p.kind {
                PaintKind::Solid(c) => Some(c),
                _ => None,
            }),
        })
        .or_else(|| {
            app.doc.get(app.page).and_then(|n| match &n.kind {
                NodeKind::Page { background } => Some(*background),
                _ => None,
            })
        })
        .unwrap_or(Color::rgb(0x5A, 0x5A, 0x5A));
    let dark = super::color_of(theme::BOARD_INK[0]);
    if designos_doc::lint::rules::contrast::contrast_ratio(dark, surface) >= 3.0 {
        dark
    } else {
        super::color_of(theme::BOARD_INK_ON_DARK)
    }
}

/// Starts dragging a connector from `from` (node and magnet) or, without one, from a free point.
pub fn start(app: &mut App, from: Option<(NodeId, Magnet)>, page: Vec2) {
    let parent = connector_home(&app.doc, from.map(|f| f.0), None, app.page);
    let local_page = app.doc.world_transform(parent).invert().unwrap_or(Affine::IDENTITY).apply(page);
    let id = app.doc.alloc_id();
    let start = match from {
        Some((n, m)) => ConnectorEnd::attached(n, m),
        None => ConnectorEnd::free(local_page),
    };
    let props = ConnectorProps { start, end: ConnectorEnd::free(local_page), ..Default::default() };
    let mut node = Node::connector(id, props);
    node.layout_child.absolute = true;
    node.stroke.paints = vec![Paint::solid(default_connector_ink(app, parent))];
    node.name = app.unique_name("Connector");
    let mut tx = Tx::new();
    let index = app.doc.get(parent).map(|p| p.children.len()).unwrap_or(0);
    if tx.cmd(&mut app.doc, Command::Insert { parent, index, nodes: vec![node] }).is_err() {
        return;
    }
    place(app, &mut tx, id);
    app.canvas.inter = Interaction::Connect(ConnectDrag { tx, id, moved: false, from: from.map(|f| f.0), origin: page });
    app.board.dots = from.map(|f| f.0);
    app.changed();
}

/// Writes the connector's derived geometry (box and resolved end points) through `tx`.
fn place(app: &mut App, tx: &mut Tx, id: NodeId) {
    if let Some(g) = connector_geometry(&app.doc, id) {
        if let Some(n) = tx.ed.node(&mut app.doc, id) {
            g.apply(n);
        }
    }
}

/// Pointer moved to `page` while dragging a connector.
pub fn drag(app: &mut App, c: &mut ConnectDrag, page: Vec2, mods: egui::Modifiers) {
    let deep = mods.command || mods.ctrl;
    let target = (!mods.alt).then(|| drop_target(app, page, deep, &[c.id])).flatten().filter(|(t, _)| Some(*t) != c.from);
    app.board.target = target.map(|t| t.0);
    let home = connector_home(&app.doc, c.from, target.map(|t| t.0), app.page);
    let cur_parent = app.doc.get(c.id).and_then(|n| n.parent);
    if cur_parent != Some(home) {
        let old_parent = cur_parent.unwrap_or(app.page);
        let old_world = app.doc.world_transform(old_parent);
        let new_inv = app.doc.world_transform(home).invert().unwrap_or(Affine::IDENTITY);
        if c.tx.cmd(&mut app.doc, Command::Reparent { id: c.id, parent: home, index: usize::MAX }).is_ok() {
            let ink = default_connector_ink(app, home);
            let xform = new_inv * old_world;
            if let Some(n) = c.tx.ed.node(&mut app.doc, c.id) {
                n.layout_child.absolute = true;
                n.stroke.paints = vec![Paint::solid(ink)];
                if let NodeKind::Connector(p) = &mut n.kind {
                    if p.start.node.is_none() {
                        p.start.point = xform.apply(p.start.point);
                    }
                }
            }
        }
    }
    let local_page = app.doc.world_transform(home).invert().unwrap_or(Affine::IDENTITY).apply(page);
    let end = match target {
        Some((t, m)) => ConnectorEnd { node: Some(t), magnet: m, point: local_page },
        None => ConnectorEnd::free(local_page),
    };
    if let Some(n) = c.tx.ed.node(&mut app.doc, c.id) {
        n.layout_child.absolute = true;
        if let NodeKind::Connector(p) = &mut n.kind {
            p.end = end;
        }
    }
    place(app, &mut c.tx, c.id);
    if dist(c.origin, page) * app.view.zoom > theme::DRAG_THRESHOLD_PX {
        c.moved = true;
    }
    app.changed();
}

/// The pointer was released: keep the connector when it was dragged somewhere, else forget it.
pub fn release(app: &mut App, c: ConnectDrag) {
    app.board.target = None;
    if !c.moved {
        c.tx.cancel(&mut app.doc);
        app.changed();
        return;
    }
    let id = c.id;
    app.run_tx(c.tx);
    app.selection = vec![id];
    if app.tool == crate::canvas::tools::Tool::Connector && !app.keep_tool {
        app.tool = crate::canvas::tools::Tool::Move;
    }
}

// ----------------------------------------------------------------- re-routing

/// Brings connectors back in line with the nodes they join after an edit: those attached to a
/// touched node (or anything inside it) follow it, and a touched connector carries its free ends
/// along with its own move. Every change goes through `ed`, so it is part of the same undo step.
pub fn reroute_touched(doc: &mut Document, ed: &mut Edit) {
    if (ed.before_nodes().is_empty() && ed.extra_touched.is_empty()) || with_index(doc, |idx| idx.is_empty()) {
        return;
    }
    let mut touched = ed.touched();
    touched.extend(ed.extra_touched.iter().copied());
    // a moved connector drags its free ends with it
    for id in &touched {
        let Some(cur) = doc.get(*id) else { continue };
        let NodeKind::Connector(_) = cur.kind else { continue };
        // a connector that sits where its ends put it has not been moved
        let Some(g) = connector_geometry(doc, *id) else { continue };
        let (dx, dy) = (cur.transform.tx - g.transform.tx, cur.transform.ty - g.transform.ty);
        if dx.abs() < 1e-3 && dy.abs() < 1e-3 {
            continue;
        }
        let Some(n) = ed.node(doc, *id) else { continue };
        if let NodeKind::Connector(c) = &mut n.kind {
            for e in [&mut c.start, &mut c.end] {
                if e.node.is_none() {
                    e.point = v2(e.point.x + dx, e.point.y + dy);
                }
            }
        }
    }
    let plan = plan_reroute(doc, &touched, ed.reroute_viewport);
    for (cid, g) in &plan.updates {
        if let Some(n) = ed.node(doc, *cid) {
            g.apply(n);
        }
    }
    ed.last_reroute_plan = Some(plan);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn design_doc_with_connector() -> (Document, History, NodeId, NodeId, NodeId) {
        let mut doc = Document::new("design_connectors");
        assert_eq!(doc.kind, DocumentKind::Design);
        let page = doc.add_page("Page 1");
        let a = doc.alloc_id();
        doc.insert(page, Node::frame(a, Rect::new(0.0, 0.0, 120.0, 80.0))).unwrap();
        let b = doc.alloc_id();
        doc.insert(page, Node::frame(b, Rect::new(300.0, 0.0, 120.0, 80.0))).unwrap();
        let c = doc.alloc_id();
        let props = ConnectorProps { start: ConnectorEnd::attached(a, Magnet::Auto), end: ConnectorEnd::attached(b, Magnet::Auto), ..Default::default() };
        doc.insert(page, Node::connector(c, props)).unwrap();
        sync_connector(&mut doc, c);
        (doc, History::default(), a, b, c)
    }

    #[test]
    fn reroute_runs_in_a_design_file() {
        let (mut doc, mut hist, _a, b, c) = design_doc_with_connector();
        let before = doc.get(c).unwrap().transform;

        let mut tx = Tx::new();
        tx.ed.node(&mut doc, b).unwrap().transform.tx += 140.0;
        tx.ed.node(&mut doc, b).unwrap().transform.ty -= 90.0;
        assert!(tx.commit(&mut doc, &mut hist).unwrap());

        let expected = connector_geometry(&doc, c).unwrap();
        assert_ne!(doc.get(c).unwrap().transform, before);
        assert_eq!(doc.get(c).unwrap().transform, expected.transform);
        assert_eq!(doc.get(c).unwrap().size, expected.size);
    }

    #[test]
    fn locked_connector_still_reroutes() {
        let (mut doc, mut hist, _a, b, c) = design_doc_with_connector();
        doc.get_mut(c).unwrap().locked = true;
        let before = doc.get(c).unwrap().transform;

        let mut tx = Tx::new();
        tx.ed.node(&mut doc, b).unwrap().transform.tx += 160.0;
        tx.ed.node(&mut doc, b).unwrap().transform.ty -= 60.0;
        assert!(tx.commit(&mut doc, &mut hist).unwrap());

        let expected = connector_geometry(&doc, c).unwrap();
        assert!(doc.get(c).unwrap().locked);
        assert_ne!(doc.get(c).unwrap().transform, before);
        assert_eq!(doc.get(c).unwrap().transform, expected.transform);
    }

    #[test]
    fn deleting_a_target_leaves_a_free_end_and_undo_reattaches() {
        let (mut doc, mut hist, _a, b, c) = design_doc_with_connector();
        let last_end_pt = match &doc.get(c).unwrap().kind {
            NodeKind::Connector(cp) => cp.end.point,
            _ => unreachable!(),
        };

        let mut tx = Tx::new();
        tx.cmd(&mut doc, Command::Remove { id: b }).unwrap();
        assert!(tx.commit(&mut doc, &mut hist).unwrap());

        assert!(doc.get(b).is_none());
        let cp = match &doc.get(c).unwrap().kind {
            NodeKind::Connector(cp) => cp,
            _ => unreachable!(),
        };
        assert_eq!(cp.end.node, Some(b));
        assert_eq!(cp.end.point, last_end_pt);
        assert_eq!(connector_route(&doc, c).unwrap().end(), last_end_pt);

        assert!(hist.undo(&mut doc).unwrap());
        assert!(doc.get(b).is_some());
        doc.get_mut(b).unwrap().transform.ty -= 100.0;
        reroute_attached(&mut doc, &[b]);
        assert_ne!(connector_route(&doc, c).unwrap().end(), last_end_pt);
    }

    #[test]
    fn studio_move_of_end_node_undoes_in_one_step() {
        let (mut doc, mut hist, _a, b, c) = design_doc_with_connector();
        let orig_b_tx = doc.get(b).unwrap().transform;
        let orig_c_geom = connector_geometry(&doc, c).unwrap();

        let mut tx = Tx::new();
        tx.ed.node(&mut doc, b).unwrap().transform.tx += 180.0;
        tx.ed.node(&mut doc, b).unwrap().transform.ty -= 70.0;
        assert!(tx.commit(&mut doc, &mut hist).unwrap());
        assert_eq!(hist.undo_depth(), 1);
        assert_ne!(doc.get(c).unwrap().transform, orig_c_geom.transform);

        assert!(hist.undo(&mut doc).unwrap());
        assert_eq!(hist.undo_depth(), 0);
        assert_eq!(doc.get(b).unwrap().transform, orig_b_tx);
        assert_eq!(doc.get(c).unwrap().transform, orig_c_geom.transform);
        assert_eq!(doc.get(c).unwrap().size, orig_c_geom.size);
        let cp = match &doc.get(c).unwrap().kind {
            NodeKind::Connector(cp) => cp,
            _ => unreachable!(),
        };
        assert_eq!(cp.start.point, orig_c_geom.start_point);
        assert_eq!(cp.end.point, orig_c_geom.end_point);
    }
}
