//! Export bounds: the layer box grown by everything painted outside it
//! (shadows, blur, outside strokes, unclipped children), like Figma does.

use designos_doc::{Affine, Document, Effect, Node, NodeId, NodeKind, Rect, StrokeAlign, Vec2};

/// Bounds in the node's local space (origin = its top-left corner, so
/// `x`/`y` are `<= 0` when effects spill to the top/left).
pub fn render_bounds(doc: &Document, id: NodeId) -> Rect {
    try_render_bounds(doc, id).unwrap_or_default()
}

/// Bounds in the node's local space, or `None` when the node has no painted
/// bounds (for example, a missing node or a group whose children are all hidden).
pub(crate) fn try_render_bounds(doc: &Document, id: NodeId) -> Option<Rect> {
    try_render_bounds_at_scale(doc, id, 1.0)
}

/// Bounds in local space for a raster export at `scale` device pixels per
/// document unit. Effect padding that the renderer applies in device pixels
/// is converted back to local units before the subtree is walked.
pub(crate) fn try_render_bounds_at_scale(doc: &Document, id: NodeId, scale: f32) -> Option<Rect> {
    let scale = scale.max(0.01);
    bounds_of(doc, doc.get(id)?, scale, Affine::IDENTITY)
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
    own_overflow_at_scale(n, 1.0)
}

fn own_overflow_at_scale(n: &Node, scale: f32) -> (f32, f32, f32, f32) {
    let scale = scale.max(0.01);
    let (mut eff_l, mut eff_t, mut eff_r, mut eff_b) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    if matches!(n.kind, NodeKind::Sticky(_)) {
        let reach = designos_render::board::sticky_shadow_reach(n.size);
        eff_l = reach;
        eff_t = reach;
        eff_r = reach;
        eff_b = reach;
    }

    let stroke_outset = stroke_outset(n);

    for e in &n.effects {
        match e {
            Effect::DropShadow { offset, radius, spread, visible: true, .. } => {
                let reach = (3.0 * radius.max(0.0) / 2.0 + spread + 2.0 / scale).max(0.0);
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

fn bounds_of(doc: &Document, n: &Node, scale: f32, world: Affine) -> Option<Rect> {
    let n = designos_doc::resolve_node(doc, n);
    let n = n.as_ref();
    let is_page = matches!(n.kind, NodeKind::Page { .. });
    let is_group = matches!(n.kind, NodeKind::Group);
    let own = if is_page || is_group {
        None
    } else if needs_device_space_shadow_bounds(&world) && has_visible_drop_shadow(n) {
        Some(anisotropic_effect_bounds(n, scale, world, Rect::new(0.0, 0.0, n.size.x, n.size.y)))
    } else {
        let factor = scale * geometric_scale(&world);
        let (l, t, r, b) = own_overflow_at_scale(n, factor);
        Some(transform_rect(&world, grow(Rect::new(0.0, 0.0, n.size.x, n.size.y), l, t, r, b)))
    };
    let clips = n.kind.frame_props().map(|f| f.clips_content).unwrap_or(false);
    let mut acc = own;
    let draws_kids = !matches!(n.kind, NodeKind::BooleanOp(_));
    if draws_kids && !clips {
        for cid in &n.children {
            let Some(c) = doc.get(*cid) else { continue };
            let rc = designos_doc::resolve_node(doc, c);
            if !rc.visible || rc.opacity <= 0.0 {
                continue;
            }
            let Some(cb) = bounds_of(doc, c, scale, world * rc.transform) else { continue };
            acc = Some(match acc {
                Some(a) => a.union(&cb),
                None => cb,
            });
        }
    }
    if is_group {
        let a = acc?;
        if needs_device_space_shadow_bounds(&world) && has_visible_drop_shadow(n) {
            let local = world.invert().map(|inv| transform_rect(&inv, a)).unwrap_or(a);
            return Some(anisotropic_effect_bounds(n, scale, world, local));
        }
        let factor = scale * geometric_scale(&world);
        let local = world.invert().map(|inv| transform_rect(&inv, a)).unwrap_or(a);
        let (l, t, r, b) = own_overflow_at_scale(n, factor);
        return Some(transform_rect(&world, grow(local, l, t, r, b)));
    }
    if is_page {
        return Some(acc.unwrap_or_default());
    }
    acc
}

fn has_visible_drop_shadow(n: &Node) -> bool {
    n.effects.iter().any(|e| matches!(e, Effect::DropShadow { visible: true, .. }))
}

fn is_anisotropic(t: &Affine) -> bool {
    let x = t.a * t.a + t.b * t.b;
    let y = t.c * t.c + t.d * t.d;
    let dot = t.a * t.c + t.b * t.d;
    (x - y).abs() > 1e-4 * x.max(y).max(1.0) || dot.abs() > 1e-4 * x.max(y).max(1.0)
}

fn needs_device_space_shadow_bounds(t: &Affine) -> bool {
    let scale = (t.a.abs().max(t.b.abs()).max(t.c.abs()).max(t.d.abs())).max(1.0);
    let off_axis = t.b.abs() > 1e-4 * scale || t.c.abs() > 1e-4 * scale;
    // A reflection or 180° rotation can be axis-aligned without being
    // positive-diagonal.  Shadow offsets are applied in device space by the
    // renderer, so local per-side growth reverses the offset under a sign flip.
    let axis_sign_flip = !off_axis && (t.a < 0.0 || t.d < 0.0);
    is_anisotropic(t) || off_axis || axis_sign_flip
}

fn anisotropic_effect_bounds(n: &Node, scale: f32, world: Affine, local_base: Rect) -> Rect {
    let stroke = stroke_outset(n);
    let mut base = local_base;
    let mut non_shadow = stroke;
    for e in &n.effects {
        match e {
            Effect::LayerBlur { radius, visible: true } => non_shadow = non_shadow.max(radius.max(0.0)),
            Effect::Texture(tx) if tx.visible && !tx.clip_to_shape => non_shadow = non_shadow.max(tx.radius.max(0.0)),
            _ => {}
        }
    }
    base = grow(base, non_shadow, non_shadow, non_shadow, non_shadow);
    let device = Affine { a: world.a * scale, b: world.b * scale, c: world.c * scale, d: world.d * scale, tx: world.tx * scale, ty: world.ty * scale };
    let mut bounds = transform_rect(&device, base);
    // `effects::drop_shadow_with` receives the determinant scale of the full
    // page-to-device transform, including the node's world transform.
    let device_scale = scale * geometric_scale(&world);
    for e in &n.effects {
        let Effect::DropShadow { offset, radius, spread, visible: true, .. } = e else { continue };
        let silhouette_grow = stroke + spread;
        let silhouette = grow(local_base, silhouette_grow, silhouette_grow, silhouette_grow, silhouette_grow);
        let mut shadow = transform_rect(&device, silhouette);
        // Offset, blur sigma and spread use the renderer's determinant scale;
        // the silhouette itself still follows the full affine transform.
        shadow.x += offset.x * device_scale;
        shadow.y += offset.y * device_scale;
        let margin = (3.0 * radius.max(0.0) * device_scale / 2.0
            + offset.x.abs() * device_scale
            + offset.y.abs() * device_scale
            + (spread * device_scale).max(0.0)
            + 2.0)
            .max(0.0);
        bounds = bounds.union(&grow(shadow, margin, margin, margin, margin));
    }
    device.invert().map(|inv| transform_rect(&world, transform_rect(&inv, bounds))).unwrap_or(bounds)
}

fn stroke_outset(n: &Node) -> f32 {
    let weight = n.stroke.side_weights.map(|s| s.iter().fold(0.0f32, |m, &w| m.max(w))).unwrap_or(n.stroke.weight);
    if !n.stroke.paints.iter().any(|p| p.visible) || weight <= 0.0 {
        return 0.0;
    }
    match n.stroke.align {
        StrokeAlign::Inside => 0.0,
        StrokeAlign::Center => weight / 2.0,
        StrokeAlign::Outside => weight,
    }
}

fn geometric_scale(t: &Affine) -> f32 {
    (t.a * t.d - t.b * t.c).abs().sqrt().max(1e-6)
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
        assert_eq!((b.x, b.y, b.w, b.h), (-14.0, -10.0, 128.0, 68.0));
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
        assert_eq!((b.x, b.y, b.w, b.h), (-35.0, -15.0, 398.0, 610.0));
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
        assert_eq!((b.x, b.y, b.w, b.h), (-6.0, -5.0, 316.0, 48.0));
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
        // Negative spread erodes the silhouette before the blur is applied.
        assert_eq!((b.x, b.y, b.w, b.h), (-4.0, -4.0, 108.0, 58.0));
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
        // stroke 1 + shadow tail 14 = 15 on every side
        assert_eq!((b.x, b.y, b.w, b.h), (-15.0, -15.0, 130.0, 80.0));
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
        // s1: l=14, r=14, t=10, b=18. s2: 20 on all sides. union: 20 on all sides
        assert_eq!((b.x, b.y, b.w, b.h), (-20.0, -20.0, 140.0, 90.0));
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
        // Only f's own shadow counts: l=14, r=14, t=10, b=18
        assert_eq!((b.x, b.y, b.w, b.h), (-14.0, -10.0, 128.0, 128.0));
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
    fn rotated_child_shadow_offset_keeps_device_space_bounds() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 20.0, 20.0));
        n.transform = Affine::translate(40.0, 40.0) * Affine::rotate(30.0_f32.to_radians());
        n.effects.push(Effect::DropShadow {
            color: Color::BLACK,
            offset: Vec2::new(10.0, 6.0),
            radius: 8.0,
            spread: 2.0,
            visible: true,
            show_behind: true,
            blend: Default::default(),
        });
        d.insert(page, n).unwrap();

        let b = render_bounds(&d, id);
        let pm = crate::raster::render_export(&d, id, 1.0).expect("rotated shadow export");
        let w = pm.width() as usize;
        let h = pm.height() as usize;
        let edge_alpha = (0..w)
            .flat_map(|x| [pm.data()[x * 4 + 3], pm.data()[((h - 1) * w + x) * 4 + 3]])
            .chain((0..h).flat_map(|y| [pm.data()[(y * w) * 4 + 3], pm.data()[(y * w + w - 1) * 4 + 3]]))
            .max()
            .unwrap_or(0);
        assert_eq!(edge_alpha, 0, "rotated shadow clipped at an image edge: bounds={b:?} size={w}x{h}");
    }

    #[test]
    fn rotated_group_shadow_offset_keeps_device_space_bounds() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let frame = d.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 0.0, 0.0))).unwrap();

        let gid = d.alloc_id();
        let mut group = Node::rectangle(gid, Rect::new(0.0, 0.0, 1.0, 1.0));
        group.kind = NodeKind::Group;
        group.fills.clear();
        group.transform = Affine::translate(40.0, 40.0) * Affine::rotate((-25.0_f32).to_radians());
        group.effects.push(Effect::DropShadow {
            color: Color::BLACK,
            offset: Vec2::new(10.0, 6.0),
            radius: 8.0,
            spread: 2.0,
            visible: true,
            show_behind: true,
            blend: Default::default(),
        });
        let group = d.insert(frame, group).unwrap();

        let child = d.alloc_id();
        d.insert(group, Node::rectangle(child, Rect::new(0.0, 0.0, 20.0, 20.0))).unwrap();

        let b = render_bounds(&d, frame);
        let pm = crate::raster::render_export(&d, frame, 1.0).expect("rotated group shadow export");
        let w = pm.width() as usize;
        let h = pm.height() as usize;
        let edge_alpha = (0..w)
            .flat_map(|x| [pm.data()[x * 4 + 3], pm.data()[((h - 1) * w + x) * 4 + 3]])
            .chain((0..h).flat_map(|y| [pm.data()[(y * w) * 4 + 3], pm.data()[(y * w + w - 1) * 4 + 3]]))
            .max()
            .unwrap_or(0);
        assert_eq!(edge_alpha, 0, "rotated group shadow clipped at an image edge: bounds={b:?} size={w}x{h}");
    }

    #[test]
    fn axis_aligned_sign_flip_shadow_offset_keeps_device_space_bounds() {
        for (label, transform) in [
            ("180-degree rotation", Affine::translate(40.0, 40.0) * Affine::rotate(std::f32::consts::PI)),
            ("horizontal reflection", Affine::translate(40.0, 40.0) * Affine::scale(-1.0, 1.0)),
        ] {
            let mut d = Document::new("t");
            let page = d.add_page("p");
            let gid = d.alloc_id();
            let mut group = Node::rectangle(gid, Rect::new(0.0, 0.0, 1.0, 1.0));
            group.kind = NodeKind::Group;
            group.fills.clear();
            let group = d.insert(page, group).unwrap();

            let cid = d.alloc_id();
            let mut child = Node::rectangle(cid, Rect::new(0.0, 0.0, 20.0, 20.0));
            child.transform = transform;
            child.effects.push(Effect::DropShadow {
                color: Color::BLACK,
                offset: Vec2::new(10.0, 0.0),
                radius: 8.0,
                spread: 2.0,
                visible: true,
                show_behind: true,
                blend: Default::default(),
            });
            d.insert(group, child).unwrap();

            let b = render_bounds(&d, group);
            let pm = crate::raster::render_export(&d, group, 1.0).expect("sign-flipped shadow export");
            let w = pm.width() as usize;
            let h = pm.height() as usize;
            let edge_alpha = (0..w)
                .flat_map(|x| [pm.data()[x * 4 + 3], pm.data()[((h - 1) * w + x) * 4 + 3]])
                .chain((0..h).flat_map(|y| [pm.data()[(y * w) * 4 + 3], pm.data()[(y * w + w - 1) * 4 + 3]]))
                .max()
                .unwrap_or(0);
            assert_eq!(edge_alpha, 0, "{label} shadow clipped at an image edge: bounds={b:?} size={w}x{h}");
        }
    }

    #[test]
    fn transformed_outside_stroke_shadow_uses_stroked_silhouette() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 20.0, 20.0));
        n.transform = Affine::translate(40.0, 40.0) * Affine::scale(2.0, 1.0);
        n.stroke.paints = vec![Paint::solid(Color::BLACK)];
        n.stroke.align = StrokeAlign::Outside;
        n.stroke.weight = 12.0;
        n.effects.push(Effect::DropShadow {
            color: Color::BLACK,
            offset: Vec2::new(4.0, 4.0),
            radius: 8.0,
            spread: 0.0,
            visible: true,
            show_behind: true,
            blend: Default::default(),
        });
        d.insert(page, n).unwrap();

        let b = render_bounds(&d, id);
        let pm = crate::raster::render_export(&d, id, 1.0).expect("transformed stroked shadow export");
        let w = pm.width() as usize;
        let h = pm.height() as usize;
        let edge_alpha = (0..w)
            .flat_map(|x| [pm.data()[x * 4 + 3], pm.data()[((h - 1) * w + x) * 4 + 3]])
            .chain((0..h).flat_map(|y| [pm.data()[(y * w) * 4 + 3], pm.data()[(y * w + w - 1) * 4 + 3]]))
            .max()
            .unwrap_or(0);
        assert_eq!(edge_alpha, 0, "transformed stroke shadow clipped at an image edge: bounds={b:?} size={w}x{h}");
    }

    #[test]
    fn anisotropic_child_shadow_keeps_device_space_blur_margin() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let gid = d.alloc_id();
        let mut group = Node::rectangle(gid, Rect::new(0.0, 0.0, 0.0, 0.0));
        group.kind = NodeKind::Group;
        group.fills.clear();
        let g = d.insert(page, group).unwrap();

        let cid = d.alloc_id();
        let mut c = Node::rectangle(cid, Rect::new(0.0, 0.0, 20.0, 20.0));
        c.transform = Affine::translate(80.0, 80.0) * Affine::scale(2.0, 1.0);
        c.effects.push(Effect::DropShadow {
            color: Color::BLACK,
            offset: Vec2::new(6.0, 6.0),
            radius: 8.0,
            spread: 2.0,
            visible: true,
            show_behind: true,
            blend: Default::default(),
        });
        d.insert(g, c).unwrap();

        let b = render_bounds(&d, g);
        // The renderer scales the effect parameters by sqrt(abs(det)) = sqrt(2)
        // while the silhouette follows the 2x/1y affine transform.
        assert!(b.x < 46.0, "anisotropic child shadow clipped on left: {b:?}");
        assert!(b.y <= 48.0, "anisotropic child shadow clipped on top: {b:?}");
        assert!(b.y + b.h >= 149.0, "anisotropic child shadow clipped on bottom: {b:?}");

        let pm = crate::raster::render_export(&d, g, 1.0).expect("anisotropic shadow export");
        let w = pm.width() as usize;
        let h = pm.height() as usize;
        let edge_alpha = (0..w)
            .flat_map(|x| [pm.data()[x * 4 + 3], pm.data()[((h - 1) * w + x) * 4 + 3]])
            .chain((0..h).flat_map(|y| [pm.data()[(y * w) * 4 + 3], pm.data()[(y * w + w - 1) * 4 + 3]]))
            .max()
            .unwrap_or(0);
        assert_eq!(edge_alpha, 0, "anisotropic export clipped shadow pixels at an image edge: bounds={b:?} size={w}x{h}");
    }

    #[test]
    fn anisotropic_group_own_shadow_is_included_in_export_bounds() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let mut frame_node = Node::frame(fid, Rect::new(0.0, 0.0, 200.0, 200.0));
        frame_node.fills.clear();
        let frame = d.insert(page, frame_node).unwrap();
        let gid = d.alloc_id();
        let mut group = Node::rectangle(gid, Rect::new(0.0, 0.0, 1.0, 1.0));
        group.kind = NodeKind::Group;
        group.fills.clear();
        group.transform = Affine::translate(2.0, 2.0) * Affine::scale(2.0, 1.0);
        group.effects.push(Effect::DropShadow {
            color: Color::BLACK,
            offset: Vec2::new(6.0, 6.0),
            radius: 8.0,
            spread: 2.0,
            visible: true,
            show_behind: true,
            blend: Default::default(),
        });
        let g = d.insert(frame, group).unwrap();

        let cid = d.alloc_id();
        d.insert(g, Node::rectangle(cid, Rect::new(0.0, 0.0, 20.0, 20.0))).unwrap();

        let b = render_bounds(&d, frame);
        assert!(b.x < 0.0, "group own shadow clipped on left: {b:?}");
        assert!(b.y < 0.0, "group own shadow clipped on top: {b:?}");
        assert!(b.y + b.h >= 199.9, "group own shadow bounds lost bottom extent: {b:?}");

        let pm = crate::raster::render_export(&d, frame, 1.0).expect("anisotropic group shadow export");
        let w = pm.width() as usize;
        let h = pm.height() as usize;
        let edge_alpha = (0..w)
            .flat_map(|x| [pm.data()[x * 4 + 3], pm.data()[((h - 1) * w + x) * 4 + 3]])
            .chain((0..h).flat_map(|y| [pm.data()[(y * w) * 4 + 3], pm.data()[(y * w + w - 1) * 4 + 3]]))
            .max()
            .unwrap_or(0);
        assert_eq!(edge_alpha, 0, "group own shadow clipped at an image edge: bounds={b:?} size={w}x{h}");
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

        // Children union is (20, 30, 60, 50); group shadow adds l=14, t=10, r=14, b=18 -> (6, 20, 88, 78).
        let b = render_bounds(&d, g);
        assert_eq!((b.x, b.y, b.w, b.h), (6.0, 20.0, 88.0, 78.0));
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
        // Guard test for a 100x40 rectangle with drop shadow (offset (0, 4), blur 8).
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
        assert_eq!((b.x, b.y, b.w, b.h), (-14.0, -10.0, 128.0, 68.0));
        let pm = crate::raster::render_export(&d, id, 1.0).expect("export png");
        assert_eq!((pm.width(), pm.height()), (128, 68));
        let svg = crate::svg::export_svg(&d, id, &Default::default());
        assert!(svg.contains("width=\"128\""));
        assert!(svg.contains("height=\"68\""));
        assert!(svg.contains("viewBox=\"-14 -10 128 68\""));
    }

    #[test]
    fn drop_shadow_export_bounds_do_not_clip_blur_tail() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 20.0, 20.0));
        n.effects.push(Effect::DropShadow {
            color: Color::BLACK,
            offset: Vec2::ZERO,
            radius: 8.0,
            spread: 0.0,
            visible: true,
            show_behind: true,
            blend: Default::default(),
        });
        d.insert(page, n).unwrap();

        let pm = crate::raster::render_export(&d, id, 1.0).expect("shadow export");
        let w = pm.width() as usize;
        let h = pm.height() as usize;
        let edge_alpha = (0..w)
            .flat_map(|x| [pm.data()[x * 4 + 3], pm.data()[((h - 1) * w + x) * 4 + 3]])
            .chain((0..h).flat_map(|y| [pm.data()[(y * w) * 4 + 3], pm.data()[(y * w + w - 1) * 4 + 3]]))
            .max()
            .unwrap_or(0);
        assert_eq!(edge_alpha, 0, "export clipped non-transparent shadow pixels at an image edge");
    }

    #[test]
    fn drop_shadow_export_bounds_do_not_clip_blur_tail_at_half_scale() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 20.0, 20.0));
        n.effects.push(Effect::DropShadow {
            color: Color::BLACK,
            offset: Vec2::ZERO,
            radius: 8.0,
            spread: 0.0,
            visible: true,
            show_behind: true,
            blend: Default::default(),
        });
        d.insert(page, n).unwrap();

        for scale in [0.5, 0.25, 0.1, 0.05, 0.01] {
            let pm = crate::raster::render_export(&d, id, scale).expect("scaled shadow export");
            let w = pm.width() as usize;
            let h = pm.height() as usize;
            let edge_alpha = (0..w)
                .flat_map(|x| [pm.data()[x * 4 + 3], pm.data()[((h - 1) * w + x) * 4 + 3]])
                .chain((0..h).flat_map(|y| [pm.data()[(y * w) * 4 + 3], pm.data()[(y * w + w - 1) * 4 + 3]]))
                .max()
                .unwrap_or(0);
            assert_eq!(edge_alpha, 0, "scale={scale}: export clipped non-transparent shadow pixels at an image edge");
        }
    }
}
