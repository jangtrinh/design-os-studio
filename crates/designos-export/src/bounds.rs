//! Export bounds: the layer box grown by everything painted outside it
//! (shadows, blur, outside strokes, unclipped children), like Figma does.

use designos_doc::{Affine, Document, Effect, Node, NodeId, NodeKind, Rect, StrokeAlign, Vec2};

/// How far something reaches past a box: (left, top, right, bottom).
type Reach = (f32, f32, f32, f32);

/// Bounds in the node's local space (origin = its top-left corner, so
/// `x`/`y` are `<= 0` when effects spill to the top/left).
pub fn render_bounds(doc: &Document, id: NodeId) -> Rect {
    try_render_bounds(doc, id).unwrap_or_default()
}

/// Bounds in the node's local space, or `None` when the node has no painted
/// bounds (for example, a missing node or a group whose children are all hidden).
pub(crate) fn try_render_bounds(doc: &Document, id: NodeId) -> Option<Rect> {
    bounds_of(doc, doc.get(id)?, true)
}

fn transform_rect(t: &Affine, r: Rect) -> Rect {
    let pts = [t.apply(Vec2::new(r.x, r.y)), t.apply(Vec2::new(r.x + r.w, r.y)), t.apply(Vec2::new(r.x + r.w, r.y + r.h)), t.apply(Vec2::new(r.x, r.y + r.h))];
    let x0 = pts.iter().map(|p| p.x).fold(f32::MAX, f32::min);
    let y0 = pts.iter().map(|p| p.y).fold(f32::MAX, f32::min);
    let x1 = pts.iter().map(|p| p.x).fold(f32::MIN, f32::max);
    let y1 = pts.iter().map(|p| p.y).fold(f32::MIN, f32::max);
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

fn grow(r: Rect, (l, t, rt, b): Reach) -> Rect {
    Rect::new(r.x - l, r.y - t, r.w + l + rt, r.h + t + b)
}

fn union_into(acc: Option<Rect>, r: Rect) -> Option<Rect> {
    Some(match acc {
        Some(a) => a.union(&r),
        None => r,
    })
}

/// The overlap of `a` and `b`, or `None` when they do not overlap.
fn intersect(a: Rect, b: Rect) -> Option<Rect> {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = (a.x + a.w).min(b.x + b.w);
    let y1 = (a.y + a.h).min(b.y + b.h);
    (x1 > x0 && y1 > y0).then(|| Rect::new(x0, y0, x1 - x0, y1 - y0))
}

/// How far a table's border reaches outside its box. A table draws its grid as one centred stroke,
/// so the outer border hangs half its weight outside the box whatever the stroke alignment says.
pub(crate) fn table_stroke_outset(n: &Node) -> f32 {
    let visible = n.stroke.weight > 0.0 && n.stroke.paints.iter().any(|p| p.visible && p.opacity > 0.0);
    if matches!(n.kind, NodeKind::Table(_)) && visible {
        n.stroke.weight / 2.0
    } else {
        0.0
    }
}

fn ceil_reach((l, t, r, b): Reach) -> Reach {
    (l.max(0.0).ceil(), t.max(0.0).ceil(), r.max(0.0).ceil(), b.max(0.0).ceil())
}

fn add_reach(a: Reach, b: Reach) -> Reach {
    (a.0 + b.0, a.1 + b.1, a.2 + b.2, a.3 + b.3)
}

/// How far the node's stroke reaches outside its box, side by side: (left, top, right, bottom).
/// Each side uses its own weight. Outside adds all of it, centre half of it, inside nothing.
fn stroke_reach(n: &Node) -> Reach {
    if matches!(n.kind, NodeKind::Table(_)) {
        let o = table_stroke_outset(n);
        return (o, o, o, o);
    }
    // [top, right, bottom, left]
    let weights = n.stroke.side_weights.unwrap_or([n.stroke.weight; 4]);
    let widest = weights.iter().fold(0.0f32, |m, &w| m.max(w));
    if !n.stroke.paints.iter().any(|p| p.visible) || widest <= 0.0 {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let factor = match n.stroke.align {
        StrokeAlign::Inside => 0.0,
        StrokeAlign::Center => 0.5,
        StrokeAlign::Outside => 1.0,
    };
    let [top, right, bottom, left] = weights.map(|w| w.max(0.0) * factor);
    // a measured centred line (solid, butt or round cap) is stroked in the band y in [-weight, 0] (see
    // `Node::line_centerline`): it hangs a full weight above the box and nothing below it. Square, arrow
    // and dashed lines stay on the axis and keep the symmetric padding
    match n.line_centerline() {
        Some(c) if c.y < 0.0 => (left, -2.0 * c.y, right, 0.0),
        _ => (left, top, right, bottom),
    }
}

/// How far the node's effects reach past the shape they are cast from: (left, top, right, bottom).
fn effect_reach(n: &Node) -> Reach {
    let (mut l, mut t, mut r, mut b) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    if matches!(n.kind, NodeKind::Sticky(_)) {
        // the soft shadow a sticky note draws
        let reach = designos_render::board::sticky_shadow_reach(n.size);
        (l, t, r, b) = (reach, reach, reach, reach);
    }
    for e in &n.effects {
        match e {
            Effect::DropShadow { offset, radius, spread, visible: true, .. } => {
                let reach = radius.max(0.0) + spread;
                l = l.max((reach - offset.x).max(0.0));
                r = r.max((reach + offset.x).max(0.0));
                t = t.max((reach - offset.y).max(0.0));
                b = b.max((reach + offset.y).max(0.0));
            }
            Effect::LayerBlur { radius, visible: true } => {
                let reach = radius.max(0.0);
                (l, t, r, b) = (l.max(reach), t.max(reach), r.max(reach), b.max(reach));
            }
            // Texture displaces edges by up to `radius`.
            Effect::Texture(tx) if tx.visible && !tx.clip_to_shape => {
                let reach = tx.radius.max(0.0);
                (l, t, r, b) = (l.max(reach), t.max(reach), r.max(reach), b.max(reach));
            }
            _ => {}
        }
    }
    (l, t, r, b)
}

/// How far a node's own paint reaches outside its box: (left, top, right, bottom).
pub fn own_overflow(n: &Node) -> (f32, f32, f32, f32) {
    ceil_reach(add_reach(stroke_reach(n), effect_reach(n)))
}

fn has_visible_paint(paints: &[designos_doc::Paint]) -> bool {
    paints.iter().any(|p| p.visible && p.silhouette_alpha() > 0.0)
}

/// Whether the node paints anything of its own: a visible fill or stroke for a shape or a frame, a
/// non-empty string for text. Every other kind (board items, tables, ...) paints through its own
/// drawing, so it counts as painting.
fn paints(n: &Node) -> bool {
    match &n.kind {
        NodeKind::Text(t) => !t.content.is_empty(),
        NodeKind::Rectangle
        | NodeKind::Ellipse(_)
        | NodeKind::Line
        | NodeKind::Polygon { .. }
        | NodeKind::Star { .. }
        | NodeKind::Vector { .. }
        | NodeKind::BooleanOp(_)
        | NodeKind::Frame(_)
        | NodeKind::Component(_)
        | NodeKind::ComponentSet(_)
        | NodeKind::Instance { .. } => {
            has_visible_paint(&n.fills) || ((n.stroke.weight > 0.0 || n.stroke.side_weights.is_some()) && has_visible_paint(&n.stroke.paints))
        }
        _ => true,
    }
}

fn clips(n: &Node) -> bool {
    n.kind.frame_props().is_some_and(|f| f.clips_content)
}

/// The rectangles of the node's visible children in the node's space, cut to the node's box when it
/// clips. `measure` gives a child's rectangle in the child's own space.
fn child_rects(doc: &Document, n: &Node, measure: fn(&Document, &Node) -> Option<Rect>) -> Vec<Rect> {
    // Boolean operands are not drawn on their own.
    if matches!(n.kind, NodeKind::BooleanOp(_)) {
        return Vec::new();
    }
    let clip = clips(n).then(|| Rect::new(0.0, 0.0, n.size.x, n.size.y));
    let mut out = Vec::new();
    for cid in &n.children {
        let Some(c) = doc.get(*cid) else { continue };
        let rc = designos_doc::resolve_node(doc, c);
        if !rc.visible || rc.opacity <= 0.0 {
            continue;
        }
        let Some(cb) = measure(doc, c) else { continue };
        let cr = transform_rect(&rc.transform, cb);
        match clip {
            Some(clip) => out.extend(intersect(cr, clip)),
            None => out.push(cr),
        }
    }
    out
}

/// The shape the node and its children paint, with strokes but without any effect: the rectangle a
/// shadow or blur is cast from.
fn painted_geometry(doc: &Document, n: &Node) -> Option<Rect> {
    let n = designos_doc::resolve_node(doc, n);
    let n = n.as_ref();
    if matches!(n.kind, NodeKind::Page { .. }) {
        return None;
    }
    let own = (!matches!(n.kind, NodeKind::Group) && paints(n)).then(|| grow(Rect::new(0.0, 0.0, n.size.x, n.size.y), ceil_reach(stroke_reach(n))));
    // children cut to a box the node already paints cannot grow it
    if own.is_some() && clips(n) {
        return own;
    }
    child_rects(doc, n, painted_geometry).into_iter().fold(own, union_into)
}

/// The shapes a node's effects are cast from, grown by the reach those effects give them: the shape the
/// node paints, and the shapes its children paint (cut to the node's box when it clips), but not the
/// children's own effects. A group has no shape of its own, so this is all of a group's effects.
fn effect_shapes(doc: &Document, n: &Node) -> Option<Rect> {
    let is_group = matches!(n.kind, NodeKind::Group);
    let mut acc = (!is_group && paints(n)).then(|| grow(Rect::new(0.0, 0.0, n.size.x, n.size.y), own_overflow(n)));
    let kids_reach = if is_group { own_overflow(n) } else { ceil_reach(effect_reach(n)) };
    // a node that clips and paints already holds its box grown by its whole reach, which covers every
    // child cut to that box and grown by the effect reach alone
    if acc.is_some() && clips(n) {
        return acc;
    }
    if kids_reach != (0.0, 0.0, 0.0, 0.0) {
        for cr in child_rects(doc, n, painted_geometry) {
            acc = union_into(acc, grow(cr, kids_reach));
        }
    }
    acc
}

/// `is_root` marks the layer being exported: it always keeps its own box.
///
/// A layer that paints nothing adds no box, except a frame: Figma counts a frame's own box whatever it
/// holds, painted or not, clipping or not.
fn bounds_of(doc: &Document, n: &Node, is_root: bool) -> Option<Rect> {
    let n = designos_doc::resolve_node(doc, n);
    let n = n.as_ref();
    let is_page = matches!(n.kind, NodeKind::Page { .. });
    let is_group = matches!(n.kind, NodeKind::Group);
    if is_page {
        let kids = child_rects(doc, n, |d, c| bounds_of(d, c, false));
        return Some(kids.into_iter().fold(None, union_into).unwrap_or_default());
    }
    let mut acc = effect_shapes(doc, n);
    let keeps_box = is_root || n.kind.frame_props().is_some();
    if !is_group && !paints(n) && keeps_box {
        acc = union_into(acc, Rect::new(0.0, 0.0, n.size.x, n.size.y));
    }
    // a clipping node's own box is already in `acc`, and its children are cut to that box
    if clips(n) {
        return acc;
    }
    child_rects(doc, n, |d, c| bounds_of(d, c, false)).into_iter().fold(acc, union_into)
}

/// The area a node's shadows and blur are drawn in, in its own space: the shape it paints and its
/// children's painted shapes, each grown by the reach of the node's effects and stroke.
pub(crate) fn effect_region(doc: &Document, n: &Node) -> Rect {
    let n = designos_doc::resolve_node(doc, n);
    let n = n.as_ref();
    effect_shapes(doc, n).unwrap_or_else(|| Rect::new(0.0, 0.0, n.size.x, n.size.y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Color, Paint};

    fn doc_with_shadow() -> (Document, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(50.0, 50.0, 100.0, 40.0));
        n.effects.push(Effect::DropShadow {
            color: Color::BLACK,
            offset: Vec2::new(0.0, 4.0),
            radius: 8.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
            blend: Default::default(),
        });
        d.insert(page, n).unwrap();
        (d, id)
    }

    #[test]
    fn shadow_extends_bounds_with_offset() {
        let (d, id) = doc_with_shadow();
        let b = render_bounds(&d, id);
        assert_eq!((b.x, b.y, b.w, b.h), (-8.0, -4.0, 116.0, 40.0 + 4.0 + 12.0));
    }

    #[test]
    fn outside_stroke_and_clipping() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let mut f = Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 100.0));
        if let NodeKind::Frame(fp) = &mut f.kind {
            fp.clips_content = true;
        }
        let f = d.insert(page, f).unwrap();
        let cid = d.alloc_id();
        let mut c = Node::rectangle(cid, Rect::new(90.0, 90.0, 50.0, 50.0));
        c.stroke.paints = vec![Paint::solid(Color::BLACK)];
        c.stroke.align = StrokeAlign::Outside;
        c.stroke.weight = 4.0;
        d.insert(f, c).unwrap();
        // clipped frame: children do not extend it
        assert_eq!(render_bounds(&d, f), Rect::new(0.0, 0.0, 100.0, 100.0));
        // the child alone grows by the outside stroke
        assert_eq!(render_bounds(&d, cid), Rect::new(-4.0, -4.0, 58.0, 58.0));
    }

    #[test]
    fn date_picker_fit_328x540_negative_spread() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 328.0, 540.0));
        n.stroke.paints = vec![Paint::solid(Color::BLACK)];
        n.stroke.align = StrokeAlign::Outside;
        n.stroke.weight = 1.0;
        n.effects.push(Effect::DropShadow {
            color: Color::BLACK,
            offset: Vec2::new(0.0, 20.0),
            radius: 24.0,
            spread: -4.0,
            visible: true,
            show_behind: false,
            blend: Default::default(),
        });
        d.insert(page, n).unwrap();
        let b = render_bounds(&d, id);
        assert_eq!((b.x, b.y, b.w, b.h), (-21.0, -1.0, 370.0, 582.0));
    }

    #[test]
    fn footer_badge_fit_304x36_additive_stroke_and_shadow() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 304.0, 36.0));
        n.stroke.paints = vec![Paint::solid(Color::BLACK)];
        n.stroke.align = StrokeAlign::Outside;
        n.stroke.weight = 1.0;
        n.effects.push(Effect::DropShadow {
            color: Color::BLACK,
            offset: Vec2::new(0.0, 1.0),
            radius: 2.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
            blend: Default::default(),
        });
        d.insert(page, n).unwrap();
        let b = render_bounds(&d, id);
        assert_eq!((b.x, b.y, b.w, b.h), (-3.0, -2.0, 310.0, 42.0));
    }

    #[test]
    fn negative_spread_larger_than_blur_adds_nothing() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 100.0, 50.0));
        n.stroke.paints = vec![Paint::solid(Color::BLACK)];
        n.stroke.align = StrokeAlign::Outside;
        n.stroke.weight = 2.0;
        n.effects.push(Effect::DropShadow {
            color: Color::BLACK,
            offset: Vec2::new(0.0, 0.0),
            radius: 4.0,
            spread: -6.0,
            visible: true,
            show_behind: false,
            blend: Default::default(),
        });
        d.insert(page, n).unwrap();
        let b = render_bounds(&d, id);
        // stroke outset 2 only; shadow adds nothing
        assert_eq!((b.x, b.y, b.w, b.h), (-2.0, -2.0, 104.0, 54.0));
    }

    #[test]
    fn each_stroke_alignment_adds_0_half_or_full() {
        let make = |align| {
            let mut d = Document::new("t");
            let page = d.add_page("p");
            let id = d.alloc_id();
            let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 100.0, 50.0));
            n.stroke.paints = vec![Paint::solid(Color::BLACK)];
            n.stroke.align = align;
            n.stroke.weight = 4.0;
            d.insert(page, n).unwrap();
            render_bounds(&d, id)
        };
        assert_eq!(make(StrokeAlign::Inside), Rect::new(0.0, 0.0, 100.0, 50.0));
        assert_eq!(make(StrokeAlign::Center), Rect::new(-2.0, -2.0, 104.0, 54.0));
        assert_eq!(make(StrokeAlign::Outside), Rect::new(-4.0, -4.0, 108.0, 58.0));
    }

    #[test]
    fn stroke_and_shadow_add_not_max() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 100.0, 50.0));
        n.stroke.paints = vec![Paint::solid(Color::BLACK)];
        n.stroke.align = StrokeAlign::Outside;
        n.stroke.weight = 1.0;
        n.effects.push(Effect::DropShadow {
            color: Color::BLACK,
            offset: Vec2::new(0.0, 0.0),
            radius: 8.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
            blend: Default::default(),
        });
        d.insert(page, n).unwrap();
        let b = render_bounds(&d, id);
        // stroke 1 + shadow 8 = 9 on every side
        assert_eq!((b.x, b.y, b.w, b.h), (-9.0, -9.0, 118.0, 68.0));
    }

    #[test]
    fn two_shadows_take_the_union_not_the_sum() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 100.0, 50.0));
        n.effects.push(Effect::DropShadow {
            color: Color::BLACK,
            offset: Vec2::new(0.0, 4.0),
            radius: 8.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
            blend: Default::default(),
        });
        n.effects.push(Effect::DropShadow {
            color: Color::BLACK,
            offset: Vec2::new(0.0, 0.0),
            radius: 12.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
            blend: Default::default(),
        });
        d.insert(page, n).unwrap();
        let b = render_bounds(&d, id);
        // s1: l=8, r=8, t=4, b=12. s2: 12 on all sides. union: 12 on all sides
        assert_eq!((b.x, b.y, b.w, b.h), (-12.0, -12.0, 124.0, 74.0));
    }

    /// A 100x40 rectangle with the per-side stroke weights measured in Figma: top 1, right 4, bottom 2, left 8.
    fn rect_with_side_weights(align: StrokeAlign) -> (Document, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 100.0, 40.0));
        n.stroke.paints = vec![Paint::solid(Color::BLACK)];
        n.stroke.align = align;
        n.stroke.weight = 0.0;
        n.stroke.side_weights = Some([1.0, 4.0, 2.0, 8.0]);
        d.insert(page, n).unwrap();
        (d, id)
    }

    fn png_size(d: &Document, id: NodeId) -> (u32, u32) {
        let pm = crate::raster::render_export(d, id, 1.0).expect("export png");
        (pm.width(), pm.height())
    }

    #[test]
    fn outside_stroke_adds_each_sides_own_weight() {
        // Figma: 112x43, the render box runs from -8 to 104 and from -1 to 42.
        let (d, id) = rect_with_side_weights(StrokeAlign::Outside);
        let b = render_bounds(&d, id);
        assert_eq!((b.x, b.y, b.w, b.h), (-8.0, -1.0, 112.0, 43.0));
        assert_eq!(png_size(&d, id), (112, 43));
        let svg = crate::svg::export_svg(&d, id, &Default::default());
        assert!(svg.contains("width=\"112\"") && svg.contains("height=\"43\""), "{svg}");
    }

    #[test]
    fn centre_stroke_adds_half_of_each_sides_weight_rounded_out() {
        // Figma: the render box is 106x41.5 (-4 to 102, -0.5 to 41) and the PNG rounds it out to 106x42.
        let (d, id) = rect_with_side_weights(StrokeAlign::Center);
        let b = render_bounds(&d, id);
        assert_eq!((b.x, b.y, b.w, b.h), (-4.0, -1.0, 106.0, 42.0));
        assert_eq!(png_size(&d, id), (106, 42));
        let svg = crate::svg::export_svg(&d, id, &Default::default());
        assert!(svg.contains("width=\"106\"") && svg.contains("height=\"42\""), "{svg}");
    }

    #[test]
    fn inside_stroke_adds_nothing_whatever_the_side_weights() {
        let (d, id) = rect_with_side_weights(StrokeAlign::Inside);
        assert_eq!(render_bounds(&d, id), Rect::new(0.0, 0.0, 100.0, 40.0));
        assert_eq!(png_size(&d, id), (100, 40));
    }

    #[test]
    fn uniform_outside_stroke_is_unchanged_by_the_per_side_rule() {
        // Figma: a uniform 8 outside stroke on 100x40 gives 116x56.
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 100.0, 40.0));
        n.stroke.paints = vec![Paint::solid(Color::BLACK)];
        n.stroke.align = StrokeAlign::Outside;
        n.stroke.weight = 8.0;
        d.insert(page, n).unwrap();
        assert_eq!(render_bounds(&d, id), Rect::new(-8.0, -8.0, 116.0, 56.0));
    }

    #[test]
    fn text_highlight_shape_one_sided_fractional_stroke_does_not_grow_the_other_axis() {
        // The kit's Text highlight: a 400x24 component with an outside stroke on its left and right sides
        // only (1.5 each), and a tooltip above it. Figma exports it at 403x76; folding the weights to the
        // largest one gave 404x78.
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let mut root = Node::frame(id, Rect::new(0.0, 0.0, 400.0, 24.0));
        root.stroke.paints = vec![Paint::solid(Color::BLACK)];
        root.stroke.align = StrokeAlign::Outside;
        root.stroke.weight = 0.0;
        root.stroke.side_weights = Some([0.0, 1.5, 0.0, 1.5]);
        let root = d.insert(page, root).unwrap();
        let tip = d.alloc_id();
        d.insert(root, Node::rectangle(tip, Rect::new(100.0, -52.0, 80.0, 52.0))).unwrap();
        let b = render_bounds(&d, root);
        assert_eq!((b.y, b.h), (-52.0, 76.0));
        assert!((b.w - 403.0).abs() <= 1.0, "width {} is not within 1 px of 403", b.w);
    }

    #[test]
    fn clipped_frame_counts_own_shadow_not_children() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let mut f = Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 100.0));
        if let NodeKind::Frame(fp) = &mut f.kind {
            fp.clips_content = true;
        }
        f.effects.push(Effect::DropShadow {
            color: Color::BLACK,
            offset: Vec2::new(0.0, 4.0),
            radius: 8.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
            blend: Default::default(),
        });
        let f = d.insert(page, f).unwrap();

        let cid = d.alloc_id();
        let mut c = Node::rectangle(cid, Rect::new(20.0, 20.0, 50.0, 50.0));
        c.effects.push(Effect::DropShadow {
            color: Color::BLACK,
            offset: Vec2::new(0.0, 50.0),
            radius: 50.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
            blend: Default::default(),
        });
        d.insert(f, c).unwrap();

        let b = render_bounds(&d, f);
        // Only f's own shadow counts: l=8, r=8, t=4, b=12
        assert_eq!((b.x, b.y, b.w, b.h), (-8.0, -4.0, 116.0, 116.0));
    }

    #[test]
    fn child_rotated_30_degrees_contributes_its_bounding_box() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let f = Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 100.0));
        let f = d.insert(page, f).unwrap();

        let cid = d.alloc_id();
        let mut c = Node::rectangle(cid, Rect::new(0.0, 0.0, 50.0, 50.0));
        let rad = 30.0f32.to_radians();
        let cos = rad.cos();
        let sin = rad.sin();
        c.transform = Affine { a: cos, b: sin, c: -sin, d: cos, tx: 80.0, ty: 80.0 };
        d.insert(f, c).unwrap();

        // Unrotated at (80, 80) size 50x50 would end at (130, 130).
        // Rotated 30° around (80, 80):
        // x_min = 80 - 50*sin(30°) = 55, x_max = 80 + 50*cos(30°) ≈ 123.30127
        // y_min = 80, y_max = 80 + 50*(sin(30°) + cos(30°)) ≈ 148.30127
        let b = render_bounds(&d, f);
        let expected_w = 80.0 + 50.0 * cos;
        let expected_h = 80.0 + 50.0 * (sin + cos);
        assert_eq!((b.x, b.y), (0.0, 0.0));
        assert!((b.w - expected_w).abs() < 1e-4, "expected w={expected_w}, got {}", b.w);
        assert!((b.h - expected_h).abs() < 1e-4, "expected h={expected_h}, got {}", b.h);
    }

    #[test]
    fn scales_0_5_and_1_5_on_odd_sizes_keep_ceil() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 99.0, 49.0));
        n.stroke.paints = vec![Paint::solid(Color::BLACK)];
        n.stroke.align = StrokeAlign::Outside;
        n.stroke.weight = 1.0;
        d.insert(page, n).unwrap();

        // 1x render bounds are 101 x 51
        let b = render_bounds(&d, id);
        assert_eq!((b.w, b.h), (101.0, 51.0));

        // 0.5x -> ceil(50.5) x ceil(25.5) = 51 x 26
        let pm_half = crate::raster::render_export(&d, id, 0.5).expect("0.5x export");
        assert_eq!((pm_half.width(), pm_half.height()), (51, 26));

        // 1.5x -> ceil(151.5) x ceil(76.5) = 152 x 77
        let pm_one_half = crate::raster::render_export(&d, id, 1.5).expect("1.5x export");
        assert_eq!((pm_one_half.width(), pm_one_half.height()), (152, 77));
    }

    #[test]
    fn render_bounds_svg_filter_region_matches_own_overflow() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 328.0, 540.0));
        n.stroke.paints = vec![Paint::solid(Color::BLACK)];
        n.stroke.align = StrokeAlign::Outside;
        n.stroke.weight = 1.0;
        n.effects.push(Effect::DropShadow {
            color: Color::BLACK,
            offset: Vec2::new(0.0, 20.0),
            radius: 24.0,
            spread: -4.0,
            visible: true,
            show_behind: false,
            blend: Default::default(),
        });
        let (l, t, r, b) = own_overflow(&n);
        d.insert(page, n).unwrap();

        let bounds = render_bounds(&d, id);
        assert_eq!((bounds.x, bounds.y, bounds.w, bounds.h), (-l, -t, 328.0 + l + r, 540.0 + t + b));

        let svg = crate::svg::export_svg(&d, id, &Default::default());
        let expected_filter = format!("<filter id=\"shadow0\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"", -l, -t, 328.0 + l + r, 540.0 + t + b);
        assert!(svg.contains(&expected_filter), "svg missing {expected_filter}:\n{svg}");
    }

    #[test]
    fn group_with_offset_children_and_own_shadow() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let gid = d.alloc_id();
        // Give the group an arbitrary stored box at (0, 0) to prove its own box is ignored.
        let mut g = Node::rectangle(gid, Rect::new(0.0, 0.0, 10.0, 10.0));
        g.kind = NodeKind::Group;
        g.fills.clear();
        g.effects.push(Effect::DropShadow {
            color: Color::BLACK,
            offset: Vec2::new(0.0, 4.0),
            radius: 8.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
            blend: Default::default(),
        });
        let g = d.insert(page, g).unwrap();

        let c1_id = d.alloc_id();
        let c1 = Node::rectangle(c1_id, Rect::new(20.0, 30.0, 40.0, 50.0));
        d.insert(g, c1).unwrap();

        let c2_id = d.alloc_id();
        let c2 = Node::rectangle(c2_id, Rect::new(50.0, 40.0, 30.0, 20.0));
        d.insert(g, c2).unwrap();

        // Children union is (20, 30, 60, 50); group shadow adds l=8, t=4, r=8, b=12 -> (12, 26, 76, 66).
        let b = render_bounds(&d, g);
        assert_eq!((b.x, b.y, b.w, b.h), (12.0, 26.0, 76.0, 66.0));
    }

    #[test]
    fn group_with_all_hidden_children_has_no_bounds() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let f = d.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();

        let gid = d.alloc_id();
        let mut g = Node::rectangle(gid, Rect::new(-50.0, -50.0, 20.0, 20.0));
        g.kind = NodeKind::Group;
        g.fills.clear();
        let g = d.insert(f, g).unwrap();

        let cid = d.alloc_id();
        let mut c = Node::rectangle(cid, Rect::new(0.0, 0.0, 20.0, 20.0));
        c.visible = false;
        d.insert(g, c).unwrap();

        assert_eq!(try_render_bounds(&d, g), None);
        assert_eq!(render_bounds(&d, g), Rect::default());
        assert!(crate::raster::render_export(&d, g, 1.0).is_none());
        assert!(crate::export_png(&d, g, 1.0).is_empty());
        // Parent frame is not pulled to (-50, -50) by the empty/all-hidden group.
        assert_eq!(render_bounds(&d, f), Rect::new(0.0, 0.0, 100.0, 100.0));
    }

    #[test]
    fn opacity_zero_child_skipped_in_bounds() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let f = d.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();

        let cid = d.alloc_id();
        let mut c = Node::rectangle(cid, Rect::new(200.0, 200.0, 50.0, 50.0));
        c.opacity = 0.0;
        d.insert(f, c).unwrap();

        assert_eq!(render_bounds(&d, f), Rect::new(0.0, 0.0, 100.0, 100.0));
    }

    #[test]
    fn width_bound_to_variable_resolved_in_bounds() {
        use designos_doc::{Variable, VariableCollection, VariableKind, VariableMode, VariableValue};

        let mut d = Document::new("t");
        let page = d.add_page("p");
        d.variable_collections.insert(
            "c".into(),
            VariableCollection {
                id: "c".into(),
                name: "C".into(),
                modes: vec![VariableMode { id: "m".into(), name: "Default".into() }],
                default_mode: "m".into(),
                variables: vec!["w_var".into()],
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );
        d.variables.insert(
            "w_var".into(),
            Variable {
                id: "w_var".into(),
                name: "w_var".into(),
                collection: "c".into(),
                kind: VariableKind::Number,
                values: [("m".to_string(), VariableValue::Number(240.0))].into_iter().collect(),
                description: String::new(),
                scopes: vec![],
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );

        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 100.0, 40.0));
        n.variables.bindings.insert("width".into(), "w_var".into());
        d.insert(page, n).unwrap();

        assert_eq!(render_bounds(&d, id), Rect::new(0.0, 0.0, 240.0, 40.0));
    }

    #[test]
    fn rectangle_100x40_with_drop_shadow_export_bounds_and_svg() {
        // Guard test for a 100x40 rectangle with drop shadow (offset (0, 4), blur 8);
        // 116x56 is the uncalibrated per-side prediction until calibrated against Figma.
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 100.0, 40.0));
        n.effects.push(Effect::DropShadow {
            color: Color::BLACK,
            offset: Vec2::new(0.0, 4.0),
            radius: 8.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
            blend: Default::default(),
        });
        d.insert(page, n).unwrap();
        let b = render_bounds(&d, id);
        assert_eq!((b.x, b.y, b.w, b.h), (-8.0, -4.0, 116.0, 56.0));
        let pm = crate::raster::render_export(&d, id, 1.0).expect("export png");
        assert_eq!((pm.width(), pm.height()), (116, 56));
        let svg = crate::svg::export_svg(&d, id, &Default::default());
        assert!(svg.contains("width=\"116\""));
        assert!(svg.contains("height=\"56\""));
        assert!(svg.contains("viewBox=\"-8 -4 116 56\""));
    }
}
