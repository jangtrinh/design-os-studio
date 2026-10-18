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

/// Stroke geometry of a node with a shape, mirroring the CPU renderer's
/// handling of precomputed outlines, per-side weights and the three alignments.
pub fn stroke_plan(n: &Node, sh: &Shape, res_scale: f32) -> Option<StrokePlan> {
    let align = if sh.closed { n.stroke.align } else { d::StrokeAlign::Center };
    if let Some(sp) = &sh.stroke_paths {
        let clip = if align == d::StrokeAlign::Outside { StrokeClip::Outside } else { StrokeClip::None };
        return Some(StrokePlan { outline: sp.clone(), clip });
    }
    if let (Some(sw), true) = (n.stroke.side_weights, sh.is_rect) {
        let [t, r, b, l] = sw;
        let f = match align {
            d::StrokeAlign::Inside => 0.0,
            d::StrokeAlign::Center => 0.5,
            d::StrokeAlign::Outside => 1.0,
        };
        let (w, h) = (n.size.x, n.size.y);
        let mut pb = sk::PathBuilder::new();
        let mut rect = |x: f32, y: f32, rw: f32, rh: f32| {
            if let Some(rc) = sk::Rect::from_xywh(x, y, rw, rh) {
                pb.push_rect(rc);
            }
        };
        if t > 0.0 {
            rect(-f * l, -f * t, w + f * l + f * r, t);
        }
        if b > 0.0 {
            rect(-f * l, h - (1.0 - f) * b, w + f * l + f * r, b);
        }
        if l > 0.0 {
            rect(-f * l, -f * t, l, h + f * t + f * b);
        }
        if r > 0.0 {
            rect(w - (1.0 - f) * r, -f * t, r, h + f * t + f * b);
        }
        let path = pb.finish()?;
        let rounded = n.corner_radii.iter().any(|v| *v > 0.0);
        let clip = if rounded && align == d::StrokeAlign::Inside { StrokeClip::Inside } else { StrokeClip::None };
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
    let r = Renderer { doc, env: PaintEnv { doc }, cull: None };
    let ts = ts.pre_concat(crate::geom::to_sk_transform(&n.transform));
    r.draw_node(&n, ts, pm, None, None);
}
