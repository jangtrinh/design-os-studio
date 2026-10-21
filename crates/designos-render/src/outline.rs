//! Geometry and decoding helpers for GPU renderers (`designos-gpu`).
//!
//! Everything here reuses the CPU renderer's own code (node shapes, glyph
//! outlines, image decoding, stroke expansion), so a GPU renderer that fills
//! these outlines draws the same geometry as [`crate::render_page`]. Nothing in
//! here changes how the CPU renderer works.

use crate::geom::{node_shape, Shape};
use crate::paint::PaintEnv;
use crate::{text, Renderer};
use designos_doc as d;
use designos_doc::{resolve_node, Document, Node, NodeId, NodeKind, TextProps, Vec2};
use std::rc::Rc;
use tiny_skia as sk;

/// Fill/stroke geometry of a node in local space (see [`outline_of`]).
pub type NodeShape = Shape;

/// Local-space geometry of `n`, or `None` for nodes without their own geometry
/// (groups, pages, text). `tol` is the flattening tolerance of boolean
/// operations in local units.
pub fn outline_of(doc: &Document, n: &Node, tol: f32) -> Option<NodeShape> {
    node_shape(doc, n, tol)
}

/// One glyph outline group of a text node.
#[derive(Clone)]
pub struct TextOutline {
    /// All glyphs (and decoration rectangles) of one style, node-local space.
    pub path: sk::Path,
    /// Visible fills of the style the glyphs belong to.
    pub fills: Vec<d::Paint>,
}

/// Glyph outlines of a text node, one entry per style, laid out exactly like
/// the CPU renderer does (same shaping, same positions).
pub fn text_outlines(props: &TextProps, size: Vec2) -> Vec<TextOutline> {
    let lay = text::layout(props, size);
    lay.paths
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let path = p.clone()?;
            let fills = lay.styles[i].fills.iter().filter(|f| f.visible).cloned().collect();
            Some(TextOutline { path, fills })
        })
        .collect()
}

/// Extra extent (local units) around the node bounds touched by strokes and effects.
pub fn node_margin(n: &Node) -> f32 {
    crate::index::node_margin(n)
}

/// Decoded, premultiplied RGBA8 image of a document image ref (cached per thread).
pub fn decoded_image(doc: &Document, image_ref: &str) -> Option<Rc<sk::Pixmap>> {
    crate::paint::load_image(doc, image_ref, None)
}

/// The stroke style the renderer uses for `n` at line width `width`.
pub fn node_stroke_style(n: &Node, width: f32) -> sk::Stroke {
    let s = &n.stroke;
    let mut st = sk::Stroke {
        width,
        miter_limit: s.miter_limit.max(1.0),
        line_cap: match s.cap {
            d::StrokeCap::None => sk::LineCap::Butt,
            d::StrokeCap::Round => sk::LineCap::Round,
            d::StrokeCap::Square => sk::LineCap::Square,
        },
        line_join: match s.join {
            d::StrokeJoin::Miter => sk::LineJoin::Miter,
            d::StrokeJoin::Bevel => sk::LineJoin::Bevel,
            d::StrokeJoin::Round => sk::LineJoin::Round,
        },
        dash: None,
    };
    if !s.dashes.is_empty() && s.dashes.iter().all(|d| *d >= 0.0) && s.dashes.iter().sum::<f32>() > 0.0 {
        let mut v = s.dashes.clone();
        if v.len() % 2 == 1 {
            v.extend_from_within(..);
        }
        st.dash = sk::StrokeDash::new(v, 0.0);
    }
    st
}

/// Expands `path` by `stroke` (dashes first) into an outline to be filled with
/// the non-zero rule. `res_scale` is the device scale the result is drawn at.
pub fn stroke_outline(path: &sk::Path, stroke: &sk::Stroke, res_scale: f32) -> Option<sk::Path> {
    let res_scale = res_scale.max(1e-4);
    match &stroke.dash {
        Some(dash) => path.dash(dash, res_scale)?.stroke(stroke, res_scale),
        None => path.stroke(stroke, res_scale),
    }
}

/// Which part of the plane a stroke outline is limited to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrokeClip {
    None,
    /// Only inside the node shape.
    Inside,
    /// Only outside the node shape.
    Outside,
}

/// How to paint the strokes of a node: fill `outline` (each path with its rule),
/// restricted to `clip` of the node's own shape.
#[derive(Clone)]
pub struct StrokePlan {
    pub outline: Vec<(sk::Path, sk::FillRule)>,
    pub clip: StrokeClip,
}

const KAPPA: f32 = 0.552_284_8;

fn push_elliptical_rrect(pb: &mut sk::PathBuilder, x0: f32, y0: f32, x1: f32, y1: f32, corners: [(f32, f32); 4], reverse: bool) {
    let [(tl_x, tl_y), (tr_x, tr_y), (br_x, br_y), (bl_x, bl_y)] = corners;
    let k = KAPPA;
    if !reverse {
        pb.move_to(x0 + tl_x, y0);
        pb.line_to(x1 - tr_x, y0);
        if tr_x > 0.0 && tr_y > 0.0 {
            pb.cubic_to(x1 - tr_x + tr_x * k, y0, x1, y0 + tr_y - tr_y * k, x1, y0 + tr_y);
        }
        pb.line_to(x1, y1 - br_y);
        if br_x > 0.0 && br_y > 0.0 {
            pb.cubic_to(x1, y1 - br_y + br_y * k, x1 - br_x + br_x * k, y1, x1 - br_x, y1);
        }
        pb.line_to(x0 + bl_x, y1);
        if bl_x > 0.0 && bl_y > 0.0 {
            pb.cubic_to(x0 + bl_x - bl_x * k, y1, x0, y1 - bl_y + bl_y * k, x0, y1 - bl_y);
        }
        pb.line_to(x0, y0 + tl_y);
        if tl_x > 0.0 && tl_y > 0.0 {
            pb.cubic_to(x0, y0 + tl_y - tl_y * k, x0 + tl_x - tl_x * k, y0, x0 + tl_x, y0);
        }
        pb.close();
    } else {
        pb.move_to(x0 + tl_x, y0);
        if tl_x > 0.0 && tl_y > 0.0 {
            pb.cubic_to(x0 + tl_x - tl_x * k, y0, x0, y0 + tl_y - tl_y * k, x0, y0 + tl_y);
        }
        pb.line_to(x0, y1 - bl_y);
        if bl_x > 0.0 && bl_y > 0.0 {
            pb.cubic_to(x0, y1 - bl_y + bl_y * k, x0 + bl_x - bl_x * k, y1, x0 + bl_x, y1);
        }
        pb.line_to(x1 - br_x, y1);
        if br_x > 0.0 && br_y > 0.0 {
            pb.cubic_to(x1 - br_x + br_x * k, y1, x1, y1 - br_y + br_y * k, x1, y1 - br_y);
        }
        pb.line_to(x1, y0 + tr_y);
        if tr_x > 0.0 && tr_y > 0.0 {
            pb.cubic_to(x1, y0 + tr_y - tr_y * k, x1 - tr_x + tr_x * k, y0, x1 - tr_x, y0);
        }
        pb.line_to(x0 + tl_x, y0);
        pb.close();
    }
}

/// Rounded ring path for a rectangle-like node with independent per-side stroke weights.
pub fn per_side_stroke_path(n: &Node, align: d::StrokeAlign) -> Option<sk::Path> {
    let [t, r, b, l] = n.stroke.side_weights?.map(|v| v.max(0.0));
    if t <= 0.0 && r <= 0.0 && b <= 0.0 && l <= 0.0 {
        return None;
    }
    let (w, h) = (n.size.x, n.size.y);
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let f = match align {
        d::StrokeAlign::Inside => 0.0,
        d::StrokeAlign::Center => 0.5,
        d::StrokeAlign::Outside => 1.0,
    };
    let g = 1.0 - f;
    let radii = if matches!(n.kind, d::NodeKind::Section) {
        [0.0; 4]
    } else {
        let lim = w.min(h) / 2.0;
        n.corner_radii.map(|v| v.max(0.0).min(lim))
    };
    let [rtl, rtr, rbr, rbl] = radii;
    let out_corner = |rc: f32, sx: f32, sy: f32| {
        if rc > 0.0 {
            (rc + f * sx, rc + f * sy)
        } else {
            (0.0, 0.0)
        }
    };
    let in_corner = |rc: f32, sx: f32, sy: f32| {
        let rx = rc - g * sx;
        let ry = rc - g * sy;
        if rx > 0.0 && ry > 0.0 {
            (rx, ry)
        } else {
            (0.0, 0.0)
        }
    };
    let (ox0, oy0, ox1, oy1) = (-f * l, -f * t, w + f * r, h + f * b);
    let out_corners = [out_corner(rtl, l, t), out_corner(rtr, r, t), out_corner(rbr, r, b), out_corner(rbl, l, b)];
    let mut pb = sk::PathBuilder::new();
    push_elliptical_rrect(&mut pb, ox0, oy0, ox1, oy1, out_corners, false);
    let (ix0, iy0, ix1, iy1) = (g * l, g * t, w - g * r, h - g * b);
    if ix1 > ix0 && iy1 > iy0 {
        let in_corners = [in_corner(rtl, l, t), in_corner(rtr, r, t), in_corner(rbr, r, b), in_corner(rbl, l, b)];
        push_elliptical_rrect(&mut pb, ix0, iy0, ix1, iy1, in_corners, true);
    }
    pb.finish()
}

/// Stroke geometry of a node with a shape, mirroring the CPU renderer's
/// handling of precomputed outlines, per-side weights and the three alignments.
pub fn stroke_plan(n: &Node, sh: &Shape, res_scale: f32) -> Option<StrokePlan> {
    let align = if sh.closed { n.stroke.align } else { d::StrokeAlign::Center };
    if let Some(sp) = &sh.stroke_paths {
        let clip = if align == d::StrokeAlign::Outside { StrokeClip::Outside } else { StrokeClip::None };
        return Some(StrokePlan { outline: sp.clone(), clip });
    }
    if sh.is_rect && n.stroke.side_weights.is_some() {
        let path = per_side_stroke_path(n, align)?;
        let rounded = n.corner_radii.iter().any(|v| *v > 0.0);
        let clip = if rounded && n.corner_smoothing > 0.0 && align == d::StrokeAlign::Inside { StrokeClip::Inside } else { StrokeClip::None };
        return Some(StrokePlan { outline: vec![(path, sk::FillRule::Winding)], clip });
    }
    let w = n.stroke.weight;
    if w <= 0.0 {
        return None;
    }
    let (width, clip) = match align {
        d::StrokeAlign::Center => (w, StrokeClip::None),
        d::StrokeAlign::Inside => (w * 2.0, StrokeClip::Inside),
        d::StrokeAlign::Outside => (w * 2.0, StrokeClip::Outside),
    };
    let st = node_stroke_style(n, width);
    let outline: Vec<_> = sh.paths.iter().filter_map(|(p, _)| stroke_outline(p, &st, res_scale)).map(|p| (p, sk::FillRule::Winding)).collect();
    (!outline.is_empty()).then_some(StrokePlan { outline, clip })
}

/// Outline of the footprint a stroke adds around a shape (`outset` wide, mitered),
/// used for drop-shadow silhouettes and to grow/shrink silhouettes by `width / 2`.
pub fn miter_outline(path: &sk::Path, width: f32, res_scale: f32) -> Option<sk::Path> {
    let st = sk::Stroke { width, line_join: sk::LineJoin::Miter, miter_limit: 4.0, ..Default::default() };
    stroke_outline(path, &st, res_scale)
}

/// Renders the node `id` (with its subtree, opacity, effects and masks, but
/// without its own blend mode) into `pm` with the CPU renderer. `ts` maps the
/// node's *parent* space to `pm` pixels; `grid_ts` is the page-to-device
/// transform of the whole frame, which anchors noise and blur grids so the
/// result lines up with GPU output.
pub fn render_node_into(doc: &Document, id: NodeId, ts: sk::Transform, grid_ts: sk::Transform, pm: &mut sk::Pixmap) {
    let Some(node) = crate::board::get(doc, id) else { return };
    let mut n = resolve_node(doc, &node).into_owned();
    if matches!(n.kind, NodeKind::Page { .. }) {
        return;
    }
    n.blend = d::BlendMode::PassThrough;
    crate::effects::set_grid_origin(&grid_ts);
    let r = Renderer { doc, env: PaintEnv { doc }, cull: None, runtime_opacities: &[] };
    let ts = ts.pre_concat(crate::geom::to_sk_transform(&n.transform));
    r.draw_node(&n, ts, pm, None, None);
}
