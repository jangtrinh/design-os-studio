//! Export bounds: the layer box grown by everything painted outside it
//! (shadows, blur, outside strokes, unclipped children), like Figma does.

use designos_doc::{Affine, Document, Effect, Node, NodeId, NodeKind, Rect, StrokeAlign, Vec2};

/// Bounds in the node's local space (origin = its top-left corner, so
/// `x`/`y` are `<= 0` when effects spill to the top/left).
pub fn render_bounds(doc: &Document, id: NodeId) -> Rect {
    match doc.get(id) {
        Some(n) => bounds_of(doc, n),
        None => Rect::default(),
    }
}

fn transform_rect(t: &Affine, r: Rect) -> Rect {
    let pts = [t.apply(Vec2::new(r.x, r.y)), t.apply(Vec2::new(r.x + r.w, r.y)), t.apply(Vec2::new(r.x + r.w, r.y + r.h)), t.apply(Vec2::new(r.x, r.y + r.h))];
    let x0 = pts.iter().map(|p| p.x).fold(f32::MAX, f32::min);
    let y0 = pts.iter().map(|p| p.y).fold(f32::MAX, f32::min);
    let x1 = pts.iter().map(|p| p.x).fold(f32::MIN, f32::max);
    let y1 = pts.iter().map(|p| p.y).fold(f32::MIN, f32::max);
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

fn grow(r: Rect, l: f32, t: f32, rt: f32, b: f32) -> Rect {
    Rect::new(r.x - l, r.y - t, r.w + l + rt, r.h + t + b)
}

/// How far a node's own paint reaches outside its box: (left, top, right, bottom).
pub fn own_overflow(n: &Node) -> (f32, f32, f32, f32) {
    let (mut l, mut t, mut r, mut b) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    if matches!(n.kind, NodeKind::Sticky(_)) {
        // the soft shadow a sticky note draws
        let reach = designos_render::board::sticky_shadow_reach(n.size);
        l = reach;
        t = reach;
        r = reach;
        b = reach;
    }
    let has_stroke = n.stroke.paints.iter().any(|p| p.visible) && (n.stroke.weight > 0.0 || n.stroke.side_weights.is_some());
    if has_stroke {
        let out = match n.stroke.align {
            StrokeAlign::Inside => 0.0,
            StrokeAlign::Center => n.stroke.weight / 2.0,
            StrokeAlign::Outside => n.stroke.weight,
        };
        l = l.max(out);
        t = t.max(out);
        r = r.max(out);
        b = b.max(out);
    }
    for e in &n.effects {
        match e {
            Effect::DropShadow { offset, radius, spread, visible: true, .. } => {
                let reach = radius.max(0.0) + spread.max(0.0);
                l = l.max(reach - offset.x);
                r = r.max(reach + offset.x);
                t = t.max(reach - offset.y);
                b = b.max(reach + offset.y);
            }
            Effect::LayerBlur { radius, visible: true } => {
                let reach = radius.max(0.0);
                l = l.max(reach);
                t = t.max(reach);
                r = r.max(reach);
                b = b.max(reach);
            }
            // Texture displaces edges by up to `radius`.
            Effect::Texture(tx) if tx.visible && !tx.clip_to_shape => {
                let reach = tx.radius.max(0.0);
                l = l.max(reach);
                t = t.max(reach);
                r = r.max(reach);
                b = b.max(reach);
            }
            _ => {}
        }
    }
    (l.max(0.0).ceil(), t.max(0.0).ceil(), r.max(0.0).ceil(), b.max(0.0).ceil())
}

fn bounds_of(doc: &Document, n: &Node) -> Rect {
    let n = designos_doc::resolve_node(doc, n);
    let n = n.as_ref();
    let is_page = matches!(n.kind, NodeKind::Page { .. });
    let own = if is_page {
        None
    } else {
        let (l, t, r, b) = own_overflow(n);
        Some(grow(Rect::new(0.0, 0.0, n.size.x, n.size.y), l, t, r, b))
    };
    let clips = n.kind.frame_props().map(|f| f.clips_content).unwrap_or(false);
    let mut acc = own;
    // Boolean operands are not drawn on their own.
    let draws_kids = !matches!(n.kind, NodeKind::BooleanOp(_));
    if draws_kids && !clips {
        for cid in &n.children {
            let Some(c) = doc.get(*cid) else { continue };
            let rc = designos_doc::resolve_node(doc, c);
            if !rc.visible {
                continue;
            }
            let cr = transform_rect(&rc.transform, bounds_of(doc, c));
            acc = Some(match acc {
                Some(a) => a.union(&cr),
                None => cr,
            });
        }
    }
    acc.unwrap_or_default()
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
        n.effects.push(Effect::DropShadow { color: Color::BLACK, offset: Vec2::new(0.0, 4.0), radius: 8.0, spread: 0.0, visible: true, show_behind: false });
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
}
