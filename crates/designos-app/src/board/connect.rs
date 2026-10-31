//! Connectors: magnet dots on hovered nodes, dragging a connector out of a dot (or with the
//! Connector tool), dropping it on another node, and re-routing connectors whenever the nodes
//! they join change.

use super::connectable;
use crate::app::App;
use crate::canvas::{hit, interact, Interaction};
use crate::edit::{Edit, Tx};
use crate::theme;
use crate::util::*;
use designos_doc::*;

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
    app.doc.get(id).is_some_and(|n| n.visible && !n.locked && connectable(&n.kind))
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
    let id = app.board.dots.filter(|n| candidate(app, *n))?;
    dot_positions(app, id).into_iter().find(|(_, p)| (app.view.to_screen(rect.min, *p) - pos).length() <= theme::BOARD_MAGNET_HIT).map(|(m, _)| (id, m))
}

/// Keeps `board.dots` on the node under the pointer (or the one whose dots the pointer is reaching for).
pub fn update_hover(app: &mut App, page: Option<Vec2>) {
    if !app.wb_connectors_enabled() || !matches!(app.tool, crate::canvas::tools::Tool::Move | crate::canvas::tools::Tool::Connector) {
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
    let Some(page) = page else {
        app.board.dots = None;
        return;
    };
    let hit = interact::hit_under(app, page, true).filter(|id| candidate(app, *id));
    app.board.dots = match hit {
        Some(h) => Some(h),
        None => app.board.dots.filter(|n| candidate(app, *n)).filter(|n| {
            let r = world_aabb(&app.doc, *n);
            let m = (theme::BOARD_MAGNET_GAP + theme::BOARD_MAGNET_RADIUS + 8.0) / app.view.zoom;
            Rect::new(r.x - m, r.y - m, r.w + 2.0 * m, r.h + 2.0 * m).contains(page)
        }),
    };
}

/// Node (and magnet) a connector dropped at `page` would attach to: the node under the pointer,
/// at the nearest magnet dot when the pointer is close to one, else on its side facing the start.
fn drop_target(app: &App, page: Vec2, exclude: &[NodeId]) -> Option<(NodeId, Magnet)> {
    let tol = 2.0 / app.view.zoom;
    let id = hit::hit_test_excluding(&app.doc, app.page, page, tol, true, &[], exclude).filter(|id| candidate(app, *id))?;
    let near = dot_positions(app, id)
        .into_iter()
        .map(|(m, p)| (m, dist(p, page) * app.view.zoom))
        .filter(|(_, d)| *d <= theme::BOARD_MAGNET_HIT * 2.5)
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    Some((id, near.map(|(m, _)| m).unwrap_or(Magnet::Auto)))
}

/// Starts dragging a connector from `from` (node and magnet) or, without one, from a free point.
pub fn start(app: &mut App, from: Option<(NodeId, Magnet)>, page: Vec2) {
    let parent = app.page;
    let id = app.doc.alloc_id();
    let start = match from {
        Some((n, m)) => ConnectorEnd::attached(n, m),
        None => ConnectorEnd::free(page),
    };
    let props = ConnectorProps { start, end: ConnectorEnd::free(page), ..Default::default() };
    let mut node = Node::connector(id, props);
    node.stroke.paints = vec![Paint::solid(app.board.ink)];
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
pub fn drag(app: &mut App, c: &mut ConnectDrag, page: Vec2) {
    let target = drop_target(app, page, &[c.id]).filter(|(t, _)| Some(*t) != c.from);
    app.board.target = target.map(|t| t.0);
    let end = match target {
        Some((t, m)) => ConnectorEnd { node: Some(t), magnet: m, point: page },
        None => ConnectorEnd::free(page),
    };
    if let Some(n) = c.tx.ed.node(&mut app.doc, c.id) {
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
    if doc.kind != DocumentKind::Board {
        return;
    }
    let touched = ed.touched();
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
    let mut ids = connectors_attached_to_tree(doc, &touched);
    ids.extend(touched.iter().copied().filter(|t| matches!(doc.get(*t).map(|n| &n.kind), Some(NodeKind::Connector(_)))));
    ids.sort();
    ids.dedup();
    for cid in ids {
        let Some(g) = connector_geometry(doc, cid) else { continue };
        let changed = doc.get(cid).is_some_and(|n| {
            let mut probe = n.clone();
            g.apply(&mut probe);
            probe != *n
        });
        if changed {
            if let Some(n) = ed.node(doc, cid) {
                g.apply(n);
            }
        }
    }
}

/// Connectors with an end on one of `nodes` or on a node inside one of them.
fn connectors_attached_to_tree(doc: &Document, nodes: &[NodeId]) -> Vec<NodeId> {
    let mut all = nodes.to_vec();
    for n in nodes {
        if doc.get(*n).is_some_and(|k| !k.children.is_empty()) {
            doc.walk(*n, |k, _| all.push(k.id));
        }
    }
    all.sort();
    all.dedup();
    connectors_attached_to(doc, &all)
}
