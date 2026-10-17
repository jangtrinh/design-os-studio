//! Decides, before a page is written, whether the vector writer can draw a frame the way the canvas
//! does. The conditions mirror the CPU renderer (`designos_render`): when a frame uses something the
//! writer does not model, the page is flattened into an image instead of being drawn wrong.

use designos_doc::{resolve_node, BlendMode, Document, Node, NodeId, NodeKind, Paint, PaintKind};
use designos_render::{outline_of, vectorops::vertex_caps};

/// Flattening tolerance, in local units, for shapes the renderer builds from other shapes (booleans).
pub(super) const FLATTEN_TOLERANCE: f32 = 0.05;

/// Why `root` has to be written as a flattened image, or `None` when it can be written as vector content.
pub(super) fn raster_reason(doc: &Document, root: NodeId) -> Option<&'static str> {
    let node = resolve_node(doc, doc.get(root)?);
    if !is_drawn(&node) {
        return None;
    }
    subtree_reason(doc, &node)
}

/// The renderer draws nothing for hidden nodes, unknown kinds, and fully transparent nodes that do not blend.
pub(super) fn is_drawn(n: &Node) -> bool {
    if !n.visible || matches!(n.kind, NodeKind::Unknown { .. } | NodeKind::Page { .. }) {
        return false;
    }
    n.opacity.clamp(0.0, 1.0) > 0.0 || layer_blend(n)
}

/// A blend mode other than normal makes the renderer composite the node as a layer.
fn layer_blend(n: &Node) -> bool {
    !paintable_blend(n.blend)
}

fn paintable_blend(b: BlendMode) -> bool {
    matches!(b, BlendMode::Normal | BlendMode::PassThrough)
}

/// Whether the node paints its children (boolean operations only use them as operands).
pub(super) fn draws_children(n: &Node) -> bool {
    !n.children.is_empty() && !matches!(n.kind, NodeKind::BooleanOp(_))
}

/// The fills the renderer paints: a text node's base style fills, the node's own fills otherwise.
pub(super) fn visible_fills(n: &Node) -> Vec<&Paint> {
    match &n.kind {
        NodeKind::Text(t) => t.style.fills.iter().filter(|p| p.visible).collect(),
        _ => n.fills.iter().filter(|p| p.visible).collect(),
    }
}

/// Stroke paints, as the renderer decides them: a stroke exists with a weight or with per-side weights.
pub(super) fn visible_strokes(n: &Node) -> Vec<&Paint> {
    if n.stroke.weight <= 0.0 && n.stroke.side_weights.is_none() {
        return Vec::new();
    }
    n.stroke.paints.iter().filter(|p| p.visible).collect()
}

fn subtree_reason(doc: &Document, n: &Node) -> Option<&'static str> {
    if let Some(reason) = node_reason(doc, n) {
        return Some(reason);
    }
    if !draws_children(n) {
        return None;
    }
    for cid in &n.children {
        let Some(raw) = doc.get(*cid) else { continue };
        let child = resolve_node(doc, raw);
        if child.is_mask {
            if child.visible {
                if let Some(reason) = mask_reason(doc, &child) {
                    return Some(reason);
                }
            }
            continue;
        }
        if is_drawn(&child) {
            if let Some(reason) = subtree_reason(doc, &child) {
                return Some(reason);
            }
        }
    }
    None
}

fn node_reason(doc: &Document, n: &Node) -> Option<&'static str> {
    if layer_blend(n) {
        return Some("a layer blend mode");
    }
    if n.effects.iter().any(|e| e.visible()) {
        return Some("a visible effect");
    }
    let fills = visible_fills(n);
    let strokes = visible_strokes(n);
    if n.opacity.clamp(0.0, 1.0) < 1.0 && (draws_children(n) || fills.len() + strokes.len() > 1) {
        // a faded node with several parts is composited as one layer, then faded
        return Some("a faded group");
    }
    if let Some(reason) = fills.iter().chain(&strokes).find_map(|p| paint_reason(p)) {
        return Some(reason);
    }
    if let NodeKind::Text(t) = &n.kind {
        if let Some(reason) = t.runs.iter().flat_map(|r| &r.style.fills).filter(|p| p.visible).find_map(paint_reason) {
            return Some(reason);
        }
        if !strokes.is_empty() {
            return Some("an outlined text node");
        }
    }
    if vertex_caps(n).is_some() {
        return Some("arrow heads on a vector network");
    }
    clip_reason(doc, n, !strokes.is_empty())
}

/// Clips and inside/outside strokes are drawn from one outline; shapes made of several paths are not.
fn clip_reason(doc: &Document, n: &Node, has_strokes: bool) -> Option<&'static str> {
    let clips = draws_children(n) && n.kind.frame_props().is_some_and(|f| f.clips_content);
    let aligned = has_strokes && n.stroke.align != designos_doc::StrokeAlign::Center;
    if !clips && !aligned {
        return None;
    }
    let shape = outline_of(doc, n, FLATTEN_TOLERANCE)?;
    if clips && shape.paths.len() != 1 {
        return Some("a clipping frame made of several paths");
    }
    if aligned && shape.stroke_paths.is_none() && shape.closed && shape.fill_paths().len() != 1 {
        return Some("an aligned stroke around several paths");
    }
    None
}

fn paint_reason(p: &Paint) -> Option<&'static str> {
    if !paintable_blend(p.blend) {
        return Some("a paint blend mode");
    }
    match &p.kind {
        PaintKind::Solid(_) => None,
        PaintKind::Image { .. } => Some("an image paint"),
        PaintKind::AngularGradient { .. } | PaintKind::DiamondGradient { .. } => Some("an angular or diamond gradient"),
        PaintKind::LinearGradient { stops, .. } | PaintKind::RadialGradient { stops, .. } => {
            let first = stops.first().map_or(1.0, |s| s.color.a);
            stops.iter().any(|s| (s.color.a - first).abs() > 1e-4).then_some("a gradient whose stops differ in opacity")
        }
    }
}

/// A mask is a plain clip when it is one opaque shape; anything else lets part of the following
/// siblings through at partial strength, which a clip cannot express.
fn mask_reason(doc: &Document, m: &Node) -> Option<&'static str> {
    const NOT_A_CLIP: &str = "a mask that is not one opaque shape";
    if matches!(m.kind, NodeKind::Text(_) | NodeKind::Group) || draws_children(m) || m.opacity < 1.0 {
        return Some(NOT_A_CLIP);
    }
    let Some(shape) = outline_of(doc, m, FLATTEN_TOLERANCE) else { return Some(NOT_A_CLIP) };
    if !shape.fillable || shape.fill_paths().len() != 1 {
        return Some(NOT_A_CLIP);
    }
    let opaque = |p: &&Paint| matches!(&p.kind, PaintKind::Solid(c) if c.a * p.opacity >= 0.999);
    let fills = visible_fills(m);
    (!fills.iter().all(opaque)).then_some(NOT_A_CLIP)
}
