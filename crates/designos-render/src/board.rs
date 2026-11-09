//! Board items (stickies, connectors, shapes with text, stamps) are drawn by
//! *lowering* them to ordinary nodes: a body (rectangle, ellipse or vector with the
//! item's fills, stroke and effects) plus synthetic children (text, end caps, label,
//! stamp glyph layers). Both renderers and the exporters run the same lowering, so
//! they all draw the same pixels.
//!
//! The synthetic children live in a per-thread registry under ids that cannot clash
//! with document ids ([`is_synthetic`]); look nodes up through [`get`].

use crate::text::{text_geometry, text_line_box};
use designos_doc as d;
use designos_doc::{
    Affine, ArcData, Color, ConnectorCap, Document, Effect, FrameProps, Node, NodeId, NodeKind, Paint, Path, PathCmd, Rect, ShapeKind, StampLayer, StickyProps,
    StrokeAlign, StrokeCap, StrokeJoin, TableProps, TextAlignH, TextAlignV, TextAutoResize, TextProps, Vec2,
};
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

const SYNTH_BIT: u64 = 1 << 62;

thread_local! {
    static SYNTH: RefCell<HashMap<NodeId, Rc<Node>>> = RefCell::new(HashMap::new());
}

/// Id of the `slot`-th (0..16) synthetic child of the board item `base`.
pub fn synth_id(base: NodeId, slot: u64) -> NodeId {
    NodeId(SYNTH_BIT | (base.0 << 4) | (slot & 15))
}

/// Whether `id` names a lowered child rather than a document node.
pub fn is_synthetic(id: NodeId) -> bool {
    id.0 & SYNTH_BIT != 0
}

/// A document node, or a synthetic child registered by the last [`lower`] on this thread.
pub fn get(doc: &Document, id: NodeId) -> Option<Cow<'_, Node>> {
    if is_synthetic(id) {
        SYNTH.with(|s| s.borrow().get(&id).map(|n| Cow::Owned((**n).clone())))
    } else {
        doc.get(id).map(Cow::Borrowed)
    }
}

/// Largest extent (local units) the shadow of a sticky note reaches past its box.
pub fn sticky_shadow_reach(size: Vec2) -> f32 {
    let m = size.x.min(size.y);
    m * 0.025 + m * 0.06 * 1.6 + 4.0
}

fn solid(c: Color) -> Vec<Paint> {
    vec![Paint::solid(c)]
}

fn child(base: &Node, slot: u64, kind: NodeKind, r: Rect) -> Node {
    let mut n = Node::new(synth_id(base.id, slot), kind);
    n.parent = Some(base.id);
    n.transform = Affine::translate(r.x, r.y);
    n.size = Vec2::new(r.w, r.h);
    n.name = String::new();
    n
}

fn scaled(paths: &[Path], w: f32, h: f32) -> Vec<Path> {
    let s = |p: Vec2| Vec2::new(p.x * w, p.y * h);
    paths
        .iter()
        .map(|p| Path {
            cmds: p
                .cmds
                .iter()
                .map(|c| match *c {
                    PathCmd::MoveTo(a) => PathCmd::MoveTo(s(a)),
                    PathCmd::LineTo(a) => PathCmd::LineTo(s(a)),
                    PathCmd::QuadTo(a, b) => PathCmd::QuadTo(s(a), s(b)),
                    PathCmd::CubicTo(a, b, c) => PathCmd::CubicTo(s(a), s(b), s(c)),
                    PathCmd::Close => PathCmd::Close,
                })
                .collect(),
            fill_rule: p.fill_rule,
        })
        .collect()
}

fn vector(paths: Vec<Path>) -> NodeKind {
    NodeKind::Vector { paths, network: None }
}

/// Where the text of a board item is laid out (node-local) and the props it is laid out with:
/// for stickies and shapes the font size already shrunk to fit; for connector labels the
/// text box inside the label background. The editor uses this to place its caret exactly
/// where the renderer draws the glyphs.
#[derive(Clone, Debug, PartialEq)]
pub struct TextFrame {
    pub rect: Rect,
    pub props: TextProps,
}

/// `text` fitted into `bx`: the base size shrinks until the wrapped text fits the height.
fn fit_into(text: &TextProps, bx: Rect, max: f32, min: f32) -> TextFrame {
    let mut props = text.clone();
    props.auto_resize = TextAutoResize::None;
    if !text.content.is_empty() && bx.w >= 1.0 && bx.h >= 1.0 {
        let size = Vec2::new(bx.w, bx.h);
        let fs = d::fit_font_size(max, min, |s| {
            props.style.font_size = s;
            text_geometry(&props, size).height <= bx.h + 0.5
        });
        props.style.font_size = fs;
    }
    TextFrame { rect: bx, props }
}

fn fitted_text(base: &Node, slot: u64, f: TextFrame) -> Option<Node> {
    if f.props.content.is_empty() || f.rect.w < 1.0 || f.rect.h < 1.0 {
        return None;
    }
    Some(child(base, slot, NodeKind::Text(f.props), f.rect))
}

fn shape_text_box(shape: ShapeKind, size: Vec2) -> Rect {
    let (w, h) = (size.x, size.y);
    let (fx, fy, fw, fh) = shape.text_box();
    let inset = (w.min(h) * 0.04).min(10.0);
    Rect::new(w * fx + inset, h * fy + inset, (w * fw - 2.0 * inset).max(1.0), (h * fh - 2.0 * inset).max(1.0))
}

/// Background box and text box of a connector label centered on `mid`, and the props to lay it out with.
fn label_frame(mid: Vec2, label: &TextProps) -> (Rect, TextFrame) {
    let mut props = label.clone();
    props.auto_resize = TextAutoResize::WidthAndHeight;
    let fs = label.style.font_size.max(1.0);
    let (tw, th) = if label.content.is_empty() {
        (fs * 3.0, fs * 1.3)
    } else {
        let g = text_geometry(&props, Vec2::ZERO);
        (g.width.max(1.0), g.height.max(1.0))
    };
    let (px, py) = (fs * 0.6, fs * 0.3);
    let bg = Rect::new((mid.x - tw / 2.0 - px).round(), (mid.y - th / 2.0 - py).round(), tw + 2.0 * px, th + 2.0 * py);
    (bg, TextFrame { rect: Rect::new(bg.x + px, bg.y + py, tw, th), props })
}

/// Route of connector `n` relative to the node, the way it is drawn.
fn local_route(doc: &Document, n: &Node) -> Option<d::Route> {
    let NodeKind::Connector(c) = &n.kind else { return None };
    let route = d::connector_route(doc, n.id).or_else(|| {
        let (a, b) = d::resolve_anchors((None, c.start.magnet, c.start.point), (None, c.end.magnet, c.end.point));
        Some(d::route(c.line, &a, &b))
    })?;
    Some(route.translated(Vec2::new(-n.transform.tx, -n.transform.ty)))
}

/// The text frame of a sticky, shape or connector label; `None` for every other node.
pub fn text_frame(doc: &Document, n: &Node) -> Option<TextFrame> {
    match &n.kind {
        NodeKind::Sticky(s) => {
            let lay = d::sticky_layout(n.size, s.show_author && s.author.is_some());
            Some(fit_into(&s.text, lay.text_box, s.text.style.font_size, d::STICKY_FONT_MIN))
        }
        NodeKind::ShapeWithText(s) => Some(fit_into(&s.text, shape_text_box(s.shape, n.size), s.text.style.font_size, d::STICKY_FONT_MIN)),
        NodeKind::Connector(c) => {
            let label = c.label.as_ref()?;
            let route = local_route(doc, n)?;
            Some(label_frame(route.midpoint(), label).1)
        }
        _ => None,
    }
}

/// Background box of connector `n`'s label in the connector's own space: the box the renderer
/// paints white over the line. `None` without a label or with an empty one (nothing is painted).
pub fn connector_label_box(doc: &Document, n: &Node) -> Option<Rect> {
    let NodeKind::Connector(c) = &n.kind else { return None };
    let label = c.label.as_ref().filter(|l| !l.content.is_empty())?;
    let route = local_route(doc, n)?;
    Some(label_frame(route.midpoint(), label).0)
}

/// Lowers `n` (a board item) to a body node with synthetic children, or `None` for other kinds.
pub fn lower(doc: &Document, n: &Node) -> Option<Node> {
    let mut extras: Vec<Node> = Vec::new();
    let mut body = n.clone();
    body.children.clear();
    body.is_mask = false;
    match &n.kind {
        NodeKind::Sticky(s) => lower_sticky(n, s, &mut body, &mut extras),
        NodeKind::ShapeWithText(s) => {
            let (w, h) = (n.size.x, n.size.y);
            body.kind = match s.shape {
                ShapeKind::Square => NodeKind::Rectangle,
                ShapeKind::RoundedRect => {
                    body.corner_radii = [w.min(h) * 0.2; 4];
                    NodeKind::Rectangle
                }
                ShapeKind::Ellipse => NodeKind::Ellipse(ArcData::default()),
                k => vector(k.path(w, h).into_iter().collect()),
            };
            if s.shape == ShapeKind::Square {
                body.corner_radii = [0.0; 4];
            }
            if body.stroke.join == StrokeJoin::Miter && !matches!(body.kind, NodeKind::Rectangle) {
                body.stroke.join = StrokeJoin::Round;
            }
            if !matches!(body.kind, NodeKind::Rectangle) {
                body.stroke.align = StrokeAlign::Center;
            }
            let bx = shape_text_box(s.shape, n.size);
            extras.extend(fitted_text(n, 0, fit_into(&s.text, bx, s.text.style.font_size, d::STICKY_FONT_MIN)));
        }
        NodeKind::Stamp { kind } => {
            let layers = d::stamp_layers(kind);
            let (w, h) = (n.size.x, n.size.y);
            body.stroke.paints.clear();
            body.stroke.weight = 0.0;
            let mut it = layers.into_iter();
            if let Some(first) = it.next() {
                body.kind = vector(scaled(&first.paths, w, h));
                body.fills = first.fill.map(solid).unwrap_or_default();
            }
            for (i, l) in it.enumerate() {
                extras.push(stamp_layer(n, 6 + i as u64, &l, w, h));
            }
        }
        NodeKind::Connector(_) => lower_connector(doc, n, &mut body, &mut extras)?,
        NodeKind::TableCell(_) => lower_table_cell(n, &mut body, &mut extras),
        NodeKind::Table(p) => {
            lower_table(doc, n, p, &mut body, &mut extras);
            // the table's own cells come first (real ids), the grid drawn over them last
            body.children = n.children.iter().copied().chain(extras.iter().map(|e| e.id)).collect();
        }
        _ => return None,
    }
    if !matches!(n.kind, NodeKind::Table(_)) {
        body.children = extras.iter().map(|e| e.id).collect();
    }
    let is_cell = matches!(n.kind, NodeKind::TableCell(_));
    SYNTH.with(|s| {
        let mut s = s.borrow_mut();
        // ids of deleted items are never reused: forget their children when the table gets big.
        // A cell is lowered while its table's grid child is still waiting to be drawn, so a cell
        // never clears: the clear would drop the grid (the table's own lowering clears instead).
        if s.len() > 50_000 && !is_cell {
            s.clear();
        }
        for e in extras {
            s.insert(e.id, Rc::new(e));
        }
    });
    Some(body)
}

/// A copy of `doc` in which every board item is replaced by its lowered form (the body node
/// plus its synthetic children as real nodes), for exporters that walk the node tree
/// themselves. `None` when the document has no board items.
pub fn lowered_document(doc: &Document) -> Option<Document> {
    let items: Vec<NodeId> = doc.nodes.values().filter(|n| n.kind.is_board_item()).map(|n| n.id).collect();
    if items.is_empty() {
        return None;
    }
    let mut out = doc.clone();
    for id in items {
        let Some(n) = doc.get(id) else { continue };
        let Some(body) = lower(doc, n) else { continue };
        // real children (a table's cells) lower themselves; only lowered children are copied in,
        // or a cell met before its table would be overwritten by the raw document node
        for c in body.children.iter().filter(|c| is_synthetic(**c)) {
            if let Some(k) = get(doc, *c) {
                out.nodes.insert(*c, k.into_owned());
            }
        }
        // keep the document's own tree links
        let mut body = body;
        body.parent = n.parent;
        out.nodes.insert(id, body);
    }
    Some(out)
}

fn lower_sticky(n: &Node, s: &StickyProps, body: &mut Node, extras: &mut Vec<Node>) {
    let lay = d::sticky_layout(n.size, s.show_author && s.author.is_some());
    let m = n.size.x.min(n.size.y);
    body.kind = NodeKind::Rectangle;
    body.corner_radii = [lay.corner; 4];
    body.corner_smoothing = 0.0;
    body.fills = solid(s.color);
    // soft shadow: a wide faint one and a tight contact one, then the user's own effects
    let mut effects = vec![
        Effect::DropShadow {
            color: Color { a: 0.18, ..Color::BLACK },
            offset: Vec2::new(0.0, m * 0.025),
            radius: m * 0.06,
            spread: 0.0,
            visible: true,
            show_behind: false,
            blend: Default::default(),
        },
        Effect::DropShadow {
            color: Color { a: 0.12, ..Color::BLACK },
            offset: Vec2::new(0.0, 1.0),
            radius: 2.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
            blend: Default::default(),
        },
    ];
    effects.extend(n.effects.iter().cloned());
    body.effects = effects;
    extras.extend(fitted_text(n, 0, fit_into(&s.text, lay.text_box, s.text.style.font_size, d::STICKY_FONT_MIN)));
    if let (Some(author), Some(bx)) = (&s.author, lay.author_box) {
        let mut t = d::board_text(lay.author_size, TextAlignH::Left, TextAlignV::Bottom);
        t.content = author.clone();
        t.style.font_weight = 400;
        t.style.fills = solid(Color { a: 0.55, ..Color::rgb(0x1E, 0x1E, 0x1E) });
        extras.push(child(n, 1, NodeKind::Text(t), bx));
    }
}

fn stamp_layer(base: &Node, slot: u64, l: &StampLayer, w: f32, h: f32) -> Node {
    let mut c = child(base, slot, vector(scaled(&l.paths, w, h)), Rect::new(0.0, 0.0, w, h));
    c.fills = l.fill.map(solid).unwrap_or_default();
    if let Some((color, weight)) = l.stroke {
        c.stroke.paints = solid(color);
        c.stroke.weight = weight * w.min(h);
        c.stroke.align = StrokeAlign::Center;
        c.stroke.cap = StrokeCap::Round;
        c.stroke.join = StrokeJoin::Round;
    } else {
        c.stroke.weight = 0.0;
    }
    c
}

fn lower_connector(doc: &Document, n: &Node, body: &mut Node, extras: &mut Vec<Node>) -> Option<()> {
    let NodeKind::Connector(c) = &n.kind else { return None };
    // the route from the live endpoints, relative to this node
    let route = local_route(doc, n)?;
    body.kind = vector(vec![route.path()]);
    body.fills.clear();
    body.corner_radii = [0.0; 4];
    body.stroke.align = StrokeAlign::Center;
    let weight = n.stroke.weight.max(0.5);
    let ink = n.stroke.paints.clone();
    let full = Rect::new(0.0, 0.0, n.size.x, n.size.y);
    // the line stops at the base of a filled cap so a wide cap is not struck through
    for (slot, cap, tip, dir) in [(2u64, c.start_cap, route.start(), neg(route.start_tangent())), (3u64, c.end_cap, route.end(), route.end_tangent())] {
        let Some(path) = d::cap_path(cap, tip, dir, weight) else { continue };
        let mut k = child(n, slot, vector(vec![path]), full);
        match cap {
            ConnectorCap::Arrow => {
                k.stroke = n.stroke.clone();
                k.stroke.dashes.clear();
                k.stroke.cap = StrokeCap::Round;
                k.stroke.join = StrokeJoin::Round;
                k.fills.clear();
            }
            _ => {
                k.fills = ink.clone();
                k.stroke.weight = 0.0;
                k.stroke.paints.clear();
            }
        }
        extras.push(k);
    }
    if let Some(label) = c.label.as_ref().filter(|l| !l.content.is_empty()) {
        let (bg, f) = label_frame(route.midpoint(), label);
        let fs = label.style.font_size.max(1.0);
        let mut b = child(n, 4, NodeKind::Rectangle, bg);
        b.fills = solid(Color::WHITE);
        b.corner_radii = [(fs * 0.4).min(bg.h / 2.0); 4];
        b.stroke.weight = 0.0;
        extras.push(b);
        extras.push(child(n, 5, NodeKind::Text(f.props), f.rect));
    }
    Some(())
}

/// A table cell: its fills as a plain rectangle, and its text inside the cell minus the padding.
/// The text wraps at the cell width and never shrinks to fit. Table layout grows a row to its text, so
/// the text normally fits. A cell whose text still does not fit (a row held short, a word wider than
/// the cell) becomes a clipping frame, so the text never paints over its neighbours; every other cell
/// stays a rectangle and costs no clip mask. The fit test goes by the text's line box ([`text_leaves_cell`]).
/// The GPU clips to whole pixels where the CPU mask is anti-aliased, so at a fractional zoom or
/// origin an overflowing line can differ by a pixel at the cell edge.
fn lower_table_cell(n: &Node, body: &mut Node, extras: &mut Vec<Node>) {
    let NodeKind::TableCell(c) = &n.kind else { return };
    body.kind = NodeKind::Rectangle;
    body.corner_radii = [0.0; 4];
    body.stroke.paints.clear();
    body.stroke.weight = 0.0;
    body.stroke.side_weights = None;
    let bx = d::table::table_cell_text_rect(n.size);
    // wraps at the cell width, the width the table layout measures with
    let mut props = c.text.clone();
    props.auto_resize = TextAutoResize::Height;
    if !props.content.is_empty() && text_leaves_cell(n.size, &bx, &props) {
        body.kind = NodeKind::Frame(FrameProps { clips_content: true, ..FrameProps::default() });
    }
    extras.extend(fitted_text(n, 0, TextFrame { rect: bx, props }));
}

/// Pixels the lines may pass the cell edge before the cell clips them (rounding in the shaper).
const TEXT_FIT_SLACK: f32 = 0.5;

/// Whether the lines of `props`, laid out in `bx`, pass the edge of the cell of size `cell`. It goes by
/// the text's line box, the size table layout gives a row, not by glyph ink: ink that overhangs the
/// line box (accents, descenders at a small line height) still paints, as text does elsewhere. The
/// line box comes from a small cache of its own and, on a miss, from the layout the painter draws
/// with, so a text is shaped once however often the cell is lowered.
fn text_leaves_cell(cell: Vec2, bx: &Rect, props: &TextProps) -> bool {
    let line_box = text_line_box(props, Vec2::new(bx.w, bx.h));
    bx.x + line_box.x > cell.x + TEXT_FIT_SLACK || bx.y + line_box.y > cell.y + TEXT_FIT_SLACK
}

/// A table: its fills as a frame that clips nothing, with the grid lines as one synthetic vector
/// child (slot 0). The cells are the table's real children, drawn before the grid.
fn lower_table(doc: &Document, n: &Node, p: &TableProps, body: &mut Node, extras: &mut Vec<Node>) {
    body.kind = NodeKind::Frame(FrameProps::default());
    body.stroke.paints.clear();
    body.stroke.weight = 0.0;
    body.stroke.side_weights = None;
    let visible = n.stroke.weight > 0.0 && n.stroke.paints.iter().any(|q| q.visible && q.opacity > 0.0);
    if !visible {
        return;
    }
    let path = grid_path(doc, n, p);
    if path.cmds.is_empty() {
        return;
    }
    let mut g = child(n, 0, vector(vec![path]), Rect::new(0.0, 0.0, n.size.x, n.size.y));
    g.stroke = n.stroke.clone();
    g.stroke.align = StrokeAlign::Center;
    g.stroke.side_weights = None;
    g.stroke.cap = StrokeCap::None;
    g.stroke.join = StrokeJoin::Miter;
    extras.push(g);
}

/// Merged blocks of `p` as `(row, col, row_span, col_span)` by track position, dropping the ones
/// that name no track or overlap an earlier block (the table model repairs these on load).
fn merge_blocks(p: &TableProps) -> Vec<(usize, usize, usize, usize)> {
    let (rows, cols) = (p.rows.len(), p.cols.len());
    let mut taken = vec![false; rows * cols];
    let mut out = Vec::new();
    for m in &p.merges {
        let (Some(r0), Some(c0)) = (p.row_index(m.row), p.col_index(m.col)) else { continue };
        let rs = (m.row_span.max(1) as usize).min(rows - r0);
        let cs = (m.col_span.max(1) as usize).min(cols - c0);
        let cells = || (r0..r0 + rs).flat_map(move |r| (c0..c0 + cs).map(move |c| r * cols + c));
        if cells().any(|i| taken[i]) {
            continue;
        }
        cells().for_each(|i| taken[i] = true);
        out.push((r0, c0, rs, cs));
    }
    out
}

/// The grid of table `n`: the outer border plus every interior row and column boundary, except the
/// stretches that lie inside a merged block. Positions come from the cells' stored geometry, so the
/// lines meet the drawn cells even where a row grew to fit its text.
fn grid_path(doc: &Document, n: &Node, p: &TableProps) -> Path {
    let (rows, cols) = (p.rows.len(), p.cols.len());
    let mut path = Path { cmds: Vec::new(), fill_rule: Default::default() };
    if rows == 0 || cols == 0 {
        return path;
    }
    // boundary offsets: track sizes first, then what the cells say
    let cum = |tracks: &[d::TableTrack]| {
        let mut acc = 0.0f32;
        let mut v = vec![0.0f32];
        for t in tracks {
            acc += if t.size.is_finite() && t.size > 0.0 { t.size } else { d::TABLE_MIN_TRACK };
            v.push(acc);
        }
        v
    };
    let (mut xs, mut ys) = (cum(&p.cols), cum(&p.rows));
    let blocks = merge_blocks(p);
    let mut spans: std::collections::HashMap<(usize, usize), (usize, usize)> = blocks.iter().map(|&(r, c, rs, cs)| ((r, c), (rs, cs))).collect();
    for cell in n.children.iter().filter_map(|&id| doc.get(id)) {
        let NodeKind::TableCell(c) = &cell.kind else { continue };
        let (Some(r), Some(k)) = (p.row_index(c.row), p.col_index(c.col)) else { continue };
        let (rs, cs) = spans.remove(&(r, k)).unwrap_or((1, 1));
        let (x, y) = (cell.transform.tx, cell.transform.ty);
        if [x, y, cell.size.x, cell.size.y].iter().all(|v| v.is_finite()) {
            xs[k] = x;
            xs[k + cs] = x + cell.size.x;
            ys[r] = y;
            ys[r + rs] = y + cell.size.y;
        }
    }
    // owner[r][c]: the anchor index of the block a position belongs to
    let mut owner: Vec<usize> = (0..rows * cols).collect();
    for &(r0, c0, rs, cs) in &blocks {
        for r in r0..r0 + rs {
            for c in c0..c0 + cs {
                owner[r * cols + c] = r0 * cols + c0;
            }
        }
    }
    let (x0, x1, y0, y1) = (xs[0], xs[cols], ys[0], ys[rows]);
    let pt = |x: f32, y: f32| Vec2::new(x, y);
    path.cmds.extend([PathCmd::MoveTo(pt(x0, y0)), PathCmd::LineTo(pt(x1, y0)), PathCmd::LineTo(pt(x1, y1)), PathCmd::LineTo(pt(x0, y1)), PathCmd::Close]);
    // vertical boundaries: runs of rows where the two sides are different blocks
    for c in 1..cols {
        let mut start: Option<usize> = None;
        for r in 0..=rows {
            let line = r < rows && owner[r * cols + c - 1] != owner[r * cols + c];
            match (line, start) {
                (true, None) => start = Some(r),
                (false, Some(s)) => {
                    path.cmds.extend([PathCmd::MoveTo(pt(xs[c], ys[s])), PathCmd::LineTo(pt(xs[c], ys[r]))]);
                    start = None;
                }
                _ => {}
            }
        }
    }
    for r in 1..rows {
        let mut start: Option<usize> = None;
        for c in 0..=cols {
            let line = c < cols && owner[(r - 1) * cols + c] != owner[r * cols + c];
            match (line, start) {
                (true, None) => start = Some(c),
                (false, Some(s)) => {
                    path.cmds.extend([PathCmd::MoveTo(pt(xs[s], ys[r])), PathCmd::LineTo(pt(xs[c], ys[r]))]);
                    start = None;
                }
                _ => {}
            }
        }
    }
    path
}

fn neg(v: Vec2) -> Vec2 {
    Vec2::new(-v.x, -v.y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{ConnectorEnd, ConnectorProps, Magnet, StickyProps};

    fn doc_with(f: impl FnOnce(&mut Document, NodeId)) -> Document {
        let mut doc = Document::new("t");
        let page = doc.add_page("p");
        f(&mut doc, page);
        doc
    }

    #[test]
    fn sticky_lowers_to_rect_shadow_and_fitted_text() {
        let mut ids = (NodeId(0), NodeId(0));
        let doc = doc_with(|d, page| {
            let a = d.alloc_id();
            let short = d.insert(page, Node::sticky(a, Rect::new(0.0, 0.0, 240.0, 240.0), "Hi")).unwrap();
            let b = d.alloc_id();
            let long = d.insert(page, Node::sticky(b, Rect::new(300.0, 0.0, 240.0, 240.0), "word ".repeat(80))).unwrap();
            ids = (short, long);
        });
        let short = lower(&doc, doc.get(ids.0).unwrap()).unwrap();
        let long = lower(&doc, doc.get(ids.1).unwrap()).unwrap();
        assert!(matches!(short.kind, NodeKind::Rectangle));
        assert!(short.corner_radii[0] > 0.0);
        assert_eq!(short.effects.iter().filter(|e| matches!(e, Effect::DropShadow { .. })).count(), 2);
        let size_of = |b: &Node| match &*get(&doc, b.children[0]).unwrap() {
            Node { kind: NodeKind::Text(t), .. } => t.style.font_size,
            _ => panic!("text child expected"),
        };
        let (s, l) = (size_of(&short), size_of(&long));
        assert_eq!(s, d::STICKY_FONT_MAX, "short text keeps the full size");
        assert!(l < s * 0.7, "long text shrinks: {l} vs {s}");
        // and the shrunk text really fits the box
        let NodeKind::Text(t) = &get(&doc, long.children[0]).unwrap().kind.clone() else { panic!() };
        let tb = get(&doc, long.children[0]).unwrap().size;
        assert!(text_geometry(t, tb).height <= tb.y + 0.5);
    }

    #[test]
    fn empty_sticky_has_no_text_and_author_adds_a_line() {
        let mut id = NodeId(0);
        let doc = doc_with(|d, page| {
            let a = d.alloc_id();
            id = d.insert(page, Node::sticky(a, Rect::new(0.0, 0.0, 200.0, 200.0), "")).unwrap();
            if let NodeKind::Sticky(StickyProps { author, show_author, .. }) = &mut d.get_mut(id).unwrap().kind {
                *author = Some("Kir".into());
                *show_author = true;
            }
        });
        let b = lower(&doc, doc.get(id).unwrap()).unwrap();
        assert_eq!(b.children.len(), 1, "only the author line");
        assert!(matches!(&get(&doc, b.children[0]).unwrap().kind, NodeKind::Text(t) if t.content == "Kir"));
    }

    #[test]
    fn connector_lowers_to_path_caps_and_label() {
        let mut cid = NodeId(0);
        let doc = doc_with(|d, page| {
            let a = d.alloc_id();
            let a = d.insert(page, Node::sticky(a, Rect::new(0.0, 0.0, 100.0, 100.0), "a")).unwrap();
            let b = d.alloc_id();
            let b = d.insert(page, Node::sticky(b, Rect::new(300.0, 0.0, 100.0, 100.0), "b")).unwrap();
            let id = d.alloc_id();
            let mut props =
                ConnectorProps { start: ConnectorEnd::attached(a, Magnet::Auto), end: ConnectorEnd::attached(b, Magnet::Auto), ..Default::default() };
            props.start_cap = ConnectorCap::Circle;
            props.end_cap = ConnectorCap::Triangle;
            let mut label = d::board_text(14.0, TextAlignH::Center, TextAlignV::Center);
            label.content = "yes".into();
            props.label = Some(label);
            cid = d.insert(page, Node::connector(id, props)).unwrap();
            d::sync_connector(d, cid);
        });
        let body = lower(&doc, doc.get(cid).unwrap()).unwrap();
        assert!(matches!(body.kind, NodeKind::Vector { .. }));
        assert!(body.fills.is_empty());
        // two caps, label background and label text
        assert_eq!(body.children.len(), 4);
        assert!(body.children.iter().all(|c| is_synthetic(*c) && get(&doc, *c).is_some()));
    }

    #[test]
    fn connector_label_box_is_the_painted_background_and_needs_text() {
        let mut ids = (NodeId(0), NodeId(0));
        let doc = doc_with(|d, page| {
            let a = d.alloc_id();
            let a = d.insert(page, Node::sticky(a, Rect::new(0.0, 0.0, 100.0, 100.0), "a")).unwrap();
            let b = d.alloc_id();
            let b = d.insert(page, Node::sticky(b, Rect::new(300.0, 0.0, 100.0, 100.0), "b")).unwrap();
            let mut with = |text: &str| {
                let id = d.alloc_id();
                let mut props =
                    ConnectorProps { start: ConnectorEnd::attached(a, Magnet::Auto), end: ConnectorEnd::attached(b, Magnet::Auto), ..Default::default() };
                let mut label = d::board_text(14.0, TextAlignH::Center, TextAlignV::Center);
                label.content = text.into();
                props.label = Some(label);
                let cid = d.insert(page, Node::connector(id, props)).unwrap();
                d::sync_connector(d, cid);
                cid
            };
            ids = (with("Open settings"), with(""));
        });
        let n = doc.get(ids.0).unwrap();
        let bx = connector_label_box(&doc, n).expect("a labelled connector has a label box");
        // the same box the lowered connector paints white over its line
        let body = lower(&doc, n).unwrap();
        let bg = body.children.iter().map(|c| get(&doc, *c).unwrap()).find(|c| matches!(c.kind, NodeKind::Rectangle)).expect("label background");
        assert_eq!((bx.x, bx.y, bx.w, bx.h), (bg.transform.tx, bg.transform.ty, bg.size.x, bg.size.y));
        // and it sits on the line: the route midpoint is inside it
        let mid = local_route(&doc, n).unwrap().midpoint();
        assert!(bx.x <= mid.x && mid.x <= bx.x + bx.w && bx.y <= mid.y && mid.y <= bx.y + bx.h);
        assert!(connector_label_box(&doc, doc.get(ids.1).unwrap()).is_none(), "an empty label paints no box");
    }

    #[test]
    fn stamps_and_shapes_lower() {
        let mut ids = Vec::new();
        let doc = doc_with(|d, page| {
            for k in d::STAMP_KINDS {
                let id = d.alloc_id();
                ids.push(d.insert(page, Node::stamp(id, Vec2::ZERO, k, 64.0)).unwrap());
            }
            for k in ShapeKind::ALL {
                let id = d.alloc_id();
                ids.push(d.insert(page, Node::shape_with_text(id, Rect::new(0.0, 0.0, 120.0, 80.0), k, "Text")).unwrap());
            }
        });
        for id in ids {
            let b = lower(&doc, doc.get(id).unwrap()).unwrap();
            assert!(!matches!(b.kind, NodeKind::Sticky(_) | NodeKind::Stamp { .. } | NodeKind::ShapeWithText(_)));
        }
    }
}
