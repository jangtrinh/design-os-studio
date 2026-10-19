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
    let (mut eff_l, mut eff_t, mut eff_r, mut eff_b) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    if matches!(n.kind, NodeKind::Sticky(_)) {
        // the soft shadow a sticky note draws
        let reach = designos_render::board::sticky_shadow_reach(n.size);
        eff_l = reach;
        eff_t = reach;
        eff_r = reach;
        eff_b = reach;
    }

    let stroke_weight = n.stroke.side_weights.map(|s| s.iter().fold(0.0f32, |m, &w| m.max(w))).unwrap_or(n.stroke.weight);
    let has_stroke = n.stroke.paints.iter().any(|p| p.visible) && stroke_weight > 0.0;
    let stroke_outset = if has_stroke {
        match n.stroke.align {
            StrokeAlign::Inside => 0.0,
            StrokeAlign::Center => stroke_weight / 2.0,
            StrokeAlign::Outside => stroke_weight,
        }
    } else {
        0.0
    };

    for e in &n.effects {
        match e {
            Effect::DropShadow { offset, radius, spread, visible: true, .. } => {
                let reach = radius.max(0.0) + spread;
                eff_l = eff_l.max((reach - offset.x).max(0.0));
                eff_r = eff_r.max((reach + offset.x).max(0.0));
                eff_t = eff_t.max((reach - offset.y).max(0.0));
                eff_b = eff_b.max((reach + offset.y).max(0.0));
            }
            Effect::LayerBlur { radius, visible: true } => {
                let reach = radius.max(0.0);
                eff_l = eff_l.max(reach);
                eff_t = eff_t.max(reach);
                eff_r = eff_r.max(reach);
                eff_b = eff_b.max(reach);
            }
            // Texture displaces edges by up to `radius`.
            Effect::Texture(tx) if tx.visible && !tx.clip_to_shape => {
                let reach = tx.radius.max(0.0);
                eff_l = eff_l.max(reach);
                eff_t = eff_t.max(reach);
                eff_r = eff_r.max(reach);
                eff_b = eff_b.max(reach);
            }
            _ => {}
        }
    }

    let l = stroke_outset + eff_l;
    let t = stroke_outset + eff_t;
    let r = stroke_outset + eff_r;
    let b = stroke_outset + eff_b;

    (l.max(0.0).ceil(), t.max(0.0).ceil(), r.max(0.0).ceil(), b.max(0.0).ceil())
}

fn bounds_of(doc: &Document, n: &Node) -> Rect {
    let n = &*designos_doc::resolve_node(doc, n);
    let is_page = matches!(n.kind, NodeKind::Page { .. });
    let is_group = matches!(n.kind, NodeKind::Group);
    let own = if is_page || is_group {
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
            if !c.visible || c.opacity <= 0.0 {
                continue;
            }
            let cr = transform_rect(&c.transform, bounds_of(doc, c));
            acc = Some(match acc {
                Some(a) => a.union(&cr),
                None => cr,
            });
        }
    }
    if is_group {
        let (l, t, r, b) = own_overflow(n);
        acc = acc.map(|a| grow(a, l, t, r, b));
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

    #[test]
    fn date_picker_fit_328x540_negative_spread() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 328.0, 540.0));
        n.stroke.paints = vec![Paint::solid(Color::BLACK)];
        n.stroke.align = StrokeAlign::Outside;
        n.stroke.weight = 1.0;
        n.effects.push(Effect::DropShadow { color: Color::BLACK, offset: Vec2::new(0.0, 20.0), radius: 24.0, spread: -4.0, visible: true, show_behind: false });
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
        n.effects.push(Effect::DropShadow { color: Color::BLACK, offset: Vec2::new(0.0, 1.0), radius: 2.0, spread: 0.0, visible: true, show_behind: false });
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
        n.effects.push(Effect::DropShadow { color: Color::BLACK, offset: Vec2::new(0.0, 0.0), radius: 4.0, spread: -6.0, visible: true, show_behind: false });
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
        n.effects.push(Effect::DropShadow { color: Color::BLACK, offset: Vec2::new(0.0, 0.0), radius: 8.0, spread: 0.0, visible: true, show_behind: false });
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
        n.effects.push(Effect::DropShadow { color: Color::BLACK, offset: Vec2::new(0.0, 4.0), radius: 8.0, spread: 0.0, visible: true, show_behind: false });
        n.effects.push(Effect::DropShadow { color: Color::BLACK, offset: Vec2::new(0.0, 0.0), radius: 12.0, spread: 0.0, visible: true, show_behind: false });
        d.insert(page, n).unwrap();
        let b = render_bounds(&d, id);
        // s1: l=8, r=8, t=4, b=12. s2: 12 on all sides. union: 12 on all sides
        assert_eq!((b.x, b.y, b.w, b.h), (-12.0, -12.0, 124.0, 74.0));
    }

    #[test]
    fn per_side_stroke_weights_gate_and_outset_agree() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 100.0, 50.0));
        n.stroke.paints = vec![Paint::solid(Color::BLACK)];
        n.stroke.align = StrokeAlign::Outside;
        n.stroke.weight = 0.0;
        n.stroke.side_weights = Some([1.0, 4.0, 2.0, 8.0]);
        d.insert(page, n).unwrap();
        let b = render_bounds(&d, id);
        // outset 8 on every side
        assert_eq!((b.x, b.y, b.w, b.h), (-8.0, -8.0, 116.0, 66.0));
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
        f.effects.push(Effect::DropShadow { color: Color::BLACK, offset: Vec2::new(0.0, 4.0), radius: 8.0, spread: 0.0, visible: true, show_behind: false });
        let f = d.insert(page, f).unwrap();

        let cid = d.alloc_id();
        let mut c = Node::rectangle(cid, Rect::new(20.0, 20.0, 50.0, 50.0));
        c.effects.push(Effect::DropShadow { color: Color::BLACK, offset: Vec2::new(0.0, 50.0), radius: 50.0, spread: 0.0, visible: true, show_behind: false });
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
        c.transform = Affine { a: cos, b: sin, c: -sin, d: cos, tx: 50.0, ty: 50.0 };
        d.insert(f, c).unwrap();

        let b = render_bounds(&d, f);
        assert!(b.x <= 0.0);
        assert!(b.y <= 0.0);
        assert!(b.w >= 100.0);
        assert!(b.h >= 100.0);
    }

    #[test]
    fn unit_test_100x40_rectangle_with_drop_shadow_figma_rule() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 100.0, 40.0));
        n.effects.push(Effect::DropShadow { color: Color::BLACK, offset: Vec2::new(0.0, 4.0), radius: 8.0, spread: 0.0, visible: true, show_behind: false });
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
