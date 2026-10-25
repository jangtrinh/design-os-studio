//! The stencil a mask node cuts the siblings that follow it with. Shared by the
//! CPU renderer and the GPU compiler so both give a mask the same alpha.

use designos_doc::{BlendMode, Color, Node, NodeKind, Paint};

fn draws_children(n: &Node) -> bool {
    !n.children.is_empty() && !matches!(n.kind, NodeKind::BooleanOp(_))
}

fn has_own_fill(n: &Node) -> bool {
    matches!(n.kind, NodeKind::Text(_) | NodeKind::Group) || n.fills.iter().any(|p| p.visible)
}

/// The node to draw into the mask layer for `mask`: the mask itself with the
/// paint it would not contribute to the stencil removed (blend mode, effects,
/// strokes).
///
/// A mask draws the alpha of its own fills plus its drawn subtree, so a frame or
/// instance with children and no visible fill of its own takes its alpha from
/// those children, and one with a gradient child fades. Only a shape with
/// nothing of its own to draw (no visible fill, no drawn children) stands in as
/// an opaque fill, so a bare vector or empty frame still cuts out its outline
/// or box.
pub fn mask_stencil(mask: &Node) -> Node {
    let mut n = mask.clone();
    n.blend = BlendMode::Normal;
    n.effects.clear();
    n.stroke.paints.clear();
    if !has_own_fill(&n) && !draws_children(&n) {
        n.fills = vec![Paint::solid(Color::BLACK)];
    }
    n
}

/// The opaque box of a mask whose stencil takes its alpha from its children
/// alone (a frame, component or instance with children and no visible fill of
/// its own), drawn without those children; `None` for any other mask.
pub fn mask_frame_box(mask: &Node) -> Option<Node> {
    if has_own_fill(mask) || !draws_children(mask) {
        return None;
    }
    let mut n = mask_stencil(mask);
    n.children.clear();
    n.fills = vec![Paint::solid(Color::BLACK)];
    Some(n)
}
