//! Creating board content: new board files, stickies, shapes, stamps, brush strokes, the
//! Tab-to-next-sticky shortcut, and the text frame the editor lays text out in.

use super::{color_of, rgb_of};
use crate::app::App;
use crate::canvas::tools::Tool;
use crate::edit::Tx;
use crate::theme;
use crate::util::*;
use designos_doc::*;

/// An empty board file: one page on the (transparent) canvas colour, so the dots show through.
pub fn new_document(name: &str) -> Document {
    let mut d = Document::new(name);
    d.kind = DocumentKind::Board;
    let page = d.add_page("Board");
    if let Some(p) = d.get_mut(page) {
        p.kind = NodeKind::Page { background: Color { a: 0.0, ..color_of(theme::BOARD_BG) } };
    }
    d
}

/// The name shown on the author line of sticky notes: the signed-in user of the machine.
pub fn author_name() -> String {
    std::env::var("USERNAME").or_else(|_| std::env::var("USER")).ok().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "Me".to_string())
}

/// A fresh node for a board drawing tool (position and size are filled in by the caller);
/// `None` for tools that are not board item tools.
pub fn item(app: &mut App, tool: Tool) -> Option<Node> {
    let id = app.doc.alloc_id();
    let r = Rect::new(0.0, 0.0, 0.0, 0.0);
    Some(match tool {
        Tool::Sticky => {
            let mut n = Node::sticky(id, r, "");
            if let NodeKind::Sticky(s) = &mut n.kind {
                s.color = app.board.color;
                s.author = Some(author_name());
            }
            n.name = app.unique_name("Sticky note");
            n
        }
        Tool::BoardShape => {
            let mut n = Node::shape_with_text(id, r, app.board.shape, "");
            n.name = app.unique_name("Shape");
            n
        }
        Tool::Stamp => {
            let mut n = Node::stamp(id, Vec2::ZERO, &app.board.stamp, theme::BOARD_STAMP_SIZE);
            n.size = Vec2::ZERO;
            n.name = stamp_label(&app.board.stamp).to_string();
            n
        }
        _ => return None,
    })
}

/// Default box of a board item placed by a click at `at` (centred on the point).
pub fn click_rect(tool: Tool, at: Vec2) -> Rect {
    let (w, h) = match tool {
        Tool::Sticky => (STICKY_SIZE, STICKY_SIZE),
        Tool::Stamp => (theme::BOARD_STAMP_SIZE, theme::BOARD_STAMP_SIZE),
        _ => (theme::BOARD_SHAPE_W, theme::BOARD_SHAPE_H),
    };
    Rect::new((at.x - w / 2.0).round(), (at.y - h / 2.0).round(), w, h)
}

/// Whether dragging out this tool keeps the box square.
pub fn keeps_square(tool: Tool) -> bool {
    matches!(tool, Tool::Sticky | Tool::Stamp)
}

/// Whether a click with this tool starts typing into the new item.
pub fn types_into(tool: Tool) -> bool {
    matches!(tool, Tool::Sticky | Tool::BoardShape)
}

/// Styles a freshly drawn marker or highlighter stroke.
pub fn brush_style(n: &mut Node, tool: Tool, ink: Color) {
    let (weight, alpha) =
        if tool == Tool::Highlighter { (theme::BOARD_HIGHLIGHTER_WIDTH, theme::BOARD_HIGHLIGHTER_ALPHA) } else { (theme::BOARD_MARKER_WIDTH, 1.0) };
    n.stroke.paints = vec![Paint::solid(ink.with_alpha(alpha))];
    n.stroke.weight = weight;
    n.stroke.cap = StrokeCap::Round;
    n.stroke.join = StrokeJoin::Round;
    n.stroke.align = StrokeAlign::Center;
    n.fills.clear();
    n.name = if tool == Tool::Highlighter { "Highlighter".into() } else { "Marker".into() };
}

/// Creates the sticky note to the right of the selected (or edited) one, same colour, and starts
/// typing into it. Returns false when no single sticky is selected.
pub fn tab_next(app: &mut App) -> bool {
    let cur = app.canvas.text.as_ref().map(|s| s.id).or_else(|| (app.selection.len() == 1).then(|| app.selection[0]));
    let Some(cur) = cur else { return false };
    let Some(n) = app.doc.get(cur) else { return false };
    let NodeKind::Sticky(src) = &n.kind else { return false };
    let (color, tpos, size, parent) = (src.color, n.transform, n.size, n.parent);
    let Some(parent) = parent else { return false };
    // commit the text being typed, then add the neighbour in its own undo step
    crate::canvas::interact::finish_text(app, true);
    let id = app.doc.alloc_id();
    let mut next = Node::sticky(id, Rect::new(0.0, 0.0, size.x, size.y), "");
    next.transform = Affine::translate(tpos.tx + size.x + theme::BOARD_STICKY_GAP, tpos.ty);
    next.name = app.unique_name("Sticky note");
    if let NodeKind::Sticky(s) = &mut next.kind {
        s.color = color;
        s.author = Some(author_name());
    }
    let index = app.doc.get(parent).map(|p| p.children.len()).unwrap_or(0);
    let mut tx = Tx::new();
    if tx.cmd(&mut app.doc, Command::Insert { parent, index, nodes: vec![next] }).is_err() {
        return false;
    }
    app.run_tx(tx);
    app.selection = vec![id];
    crate::canvas::interact::begin_text_edit(app, id);
    app.changed();
    true
}

/// Where the text of a node is laid out: its local origin and box, and the props to lay out with.
pub struct TextFrame {
    pub origin: Vec2,
    pub size: Vec2,
    pub props: TextProps,
}

/// The frame of the editable text of `n`: a text layer's own box, or the text area of a sticky,
/// shape or connector label (with the font size the renderer fits).
pub fn text_frame(doc: &Document, n: &Node) -> Option<TextFrame> {
    match &n.kind {
        NodeKind::Text(t) => Some(TextFrame { origin: Vec2::ZERO, size: n.size, props: t.clone() }),
        NodeKind::TableCell(c) => {
            // the text of a cell sits inside its padding and wraps at the cell width, in the frame the renderer fits
            let r = designos_doc::table::table_cell_text_rect(n.size);
            let mut props = c.text.clone();
            props.auto_resize = designos_doc::TextAutoResize::Height;
            Some(TextFrame { origin: v2(r.x, r.y), size: v2(r.w, r.h), props })
        }
        k if k.board_text().is_some() || matches!(k, NodeKind::Connector(_)) => {
            let f = designos_render::board::text_frame(doc, n)?;
            Some(TextFrame { origin: v2(f.rect.x, f.rect.y), size: v2(f.rect.w, f.rect.h), props: f.props })
        }
        _ => None,
    }
}

/// Starts editing the label of connector `id`, adding an empty one first when it has none.
pub fn edit_label(app: &mut App, id: NodeId) {
    crate::canvas::interact::finish_text(app, true);
    let has = matches!(app.doc.get(id).map(|n| &n.kind), Some(NodeKind::Connector(c)) if c.label.is_some());
    if !has {
        app.edit_nodes(&[id], |n| {
            if let NodeKind::Connector(c) = &mut n.kind {
                c.label = Some(board_text(14.0, TextAlignH::Center, TextAlignV::Center));
            }
        });
    }
    crate::canvas::interact::begin_text_edit(app, id);
}

/// `0xRRGGBB` of a paint when it is a solid colour.
pub fn solid_rgb(p: &Paint) -> Option<u32> {
    match &p.kind {
        PaintKind::Solid(c) => Some(rgb_of(*c)),
        _ => None,
    }
}

/// Duplicates `source_id` along `side` at [`theme::BOARD_QUICK_CREATE_GAP`], stacking along the
/// perpendicular axis when the primary spot is already occupied, and attaches an elbowed arrow
/// connector from `(source_id, side)` to `(copy_id, opposite)` in one undo step.
pub fn quick_create(app: &mut App, source_id: NodeId, side: Magnet) -> Option<NodeId> {
    if !super::connect::quick_create_allowed(app, source_id) {
        return None;
    }
    crate::canvas::interact::finish_text(app, true);
    let prev_selection = if app.selection.is_empty() { vec![source_id] } else { app.selection.clone() };
    let (parent, t0, size0, orig_kind, orig_name) = {
        let n = app.doc.get(source_id)?;
        (n.parent?, n.transform, n.size, n.kind.clone(), n.name.clone())
    };
    let mut copies = crate::clipboard::capture(&app.doc, &[source_id])
        .map(|c| crate::clipboard::instantiate_linking_mains(&mut app.doc, &c, |doc, id| doc.get(id).and_then(|n| n.parent)))
        .unwrap_or_default();
    let mut nodes = copies.pop()?;
    crate::agent_busy_frames::strip_busy_metadata_from_copy(&mut app.doc, &mut app.agent_busy_frames.frame_owners, &mut nodes);

    let start_side = match side {
        Magnet::Top | Magnet::Right | Magnet::Bottom | Magnet::Left => side,
        Magnet::Auto | Magnet::Center => Magnet::Right,
    };
    let end_side = match start_side {
        Magnet::Right => Magnet::Left,
        Magnet::Left => Magnet::Right,
        Magnet::Bottom => Magnet::Top,
        Magnet::Top => Magnet::Bottom,
        Magnet::Auto | Magnet::Center => Magnet::Left,
    };
    let gap = theme::BOARD_QUICK_CREATE_GAP;
    let (base_x, base_y, step_x, step_y) = match start_side {
        Magnet::Right => (t0.tx + size0.x + gap, t0.ty, 0.0, size0.y + gap * 0.5),
        Magnet::Left => (t0.tx - size0.x - gap, t0.ty, 0.0, size0.y + gap * 0.5),
        Magnet::Bottom => (t0.tx, t0.ty + size0.y + gap, size0.x + gap * 0.5, 0.0),
        Magnet::Top => (t0.tx, t0.ty - size0.y - gap, size0.x + gap * 0.5, 0.0),
        Magnet::Auto | Magnet::Center => (t0.tx + size0.x + gap, t0.ty, 0.0, size0.y + gap * 0.5),
    };
    let sib_rects: Vec<Rect> = app
        .doc
        .get(parent)
        .map(|p| {
            p.children
                .iter()
                .rev()
                .filter_map(|&cid| {
                    let cn = app.doc.get(cid)?;
                    if !cn.visible || matches!(cn.kind, NodeKind::Connector(_)) {
                        return None;
                    }
                    Some(Rect::new(cn.transform.tx, cn.transform.ty, cn.size.x, cn.size.y))
                })
                .take(256)
                .collect()
        })
        .unwrap_or_default();
    let mut chosen = (base_x, base_y);
    for k in 0..64i32 {
        let mult = if k == 0 {
            0.0
        } else if k % 2 == 1 {
            ((k + 1) / 2) as f32
        } else {
            -((k / 2) as f32)
        };
        let cx = base_x + mult * step_x;
        let cy = base_y + mult * step_y;
        let cand = Rect::new(cx, cy, size0.x, size0.y);
        if !sib_rects.iter().any(|sr| crate::agent_busy_frames::rects_overlap(cand, *sr)) {
            chosen = (cx, cy);
            break;
        }
    }

    let is_wb = super::connect::is_whiteboard_item(&orig_kind);
    let copy_id = {
        let root = nodes.first_mut()?;
        root.transform = Affine { tx: chosen.0, ty: chosen.1, ..t0 };
        root.name = orig_name;
        if is_wb {
            if let Some(t) = root.kind.any_text_mut() {
                t.content.clear();
                t.runs.clear();
            }
            if let NodeKind::Sticky(s) = &mut root.kind {
                s.author = Some(author_name());
            }
        }
        root.id
    };
    if let Some(root) = nodes.first() {
        if !app.slot_insert_allowed(parent, &[root]) {
            return None;
        }
    }

    let mut tx = Tx::new();
    let copy_idx = app.doc.get(parent).map(|p| p.children.len()).unwrap_or(0);
    if tx.cmd(&mut app.doc, Command::Insert { parent, index: copy_idx, nodes }).is_err() {
        return None;
    }
    let home = super::connect::home_for(app, Some(source_id), Some(copy_id));
    let conn_id = app.doc.alloc_id();
    let props = ConnectorProps {
        start: ConnectorEnd::attached(source_id, start_side),
        end: ConnectorEnd::attached(copy_id, end_side),
        line: ConnectorLine::Elbowed,
        start_cap: ConnectorCap::None,
        end_cap: ConnectorCap::Arrow,
        label: None,
    };
    let mut conn = Node::connector(conn_id, props);
    super::connect::keep_out_of_flow(app.wb_board_targeting(), &mut conn);
    conn.stroke.paints = vec![Paint::solid(super::connect::default_connector_ink(app, home))];
    conn.name = app.unique_name("Connector");
    let conn_idx = app.doc.get(home).map(|p| p.children.len()).unwrap_or(0);
    if tx.cmd(&mut app.doc, Command::Insert { parent: home, index: conn_idx, nodes: vec![conn] }).is_err() {
        tx.cancel(&mut app.doc);
        return None;
    }
    super::connect::place(app, &mut tx, conn_id);
    App::mark_slot_content_override(&mut app.doc, &mut tx.ed, parent);
    if app.doc.get(parent).is_some_and(|p| matches!(p.kind, NodeKind::Group) || p.kind.frame_props().is_some_and(|f| f.auto_layout.is_some())) {
        crate::layout::relayout_up(&mut app.doc, &mut tx.ed, copy_id);
    }
    if !app.run_tx(tx) {
        return None;
    }
    app.agent_busy_frames.last_copy_undo = Some((copy_id, prev_selection, app.hist.undo_depth()));
    app.selection = vec![copy_id];
    if is_wb && matches!(orig_kind, NodeKind::Sticky(_) | NodeKind::ShapeWithText(_) | NodeKind::Text(_)) {
        crate::canvas::interact::begin_text_edit(app, copy_id);
    }
    app.changed();
    Some(copy_id)
}
