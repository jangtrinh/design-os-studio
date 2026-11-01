//! The stencil a mask node cuts the siblings that follow it with. Shared by the
//! CPU renderer and the GPU compiler so both give a mask the same alpha.

use designos_doc::{BlendMode, Color, MaskType, Node, NodeKind, Paint};

/// Rec. 709 luminance coefficients `(R, G, B)` applied to sRGB-encoded channels
/// for [`MaskType::Luminance`]:
/// `luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b`.
///
/// Note: this formula (Rec. 709 coefficients on sRGB-encoded channels, times
/// the mask's alpha) is to be confirmed against a Figma render.
pub const LUMINANCE_MASK_COEFFS: [f32; 3] = [0.2126, 0.7152, 0.0722];

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
/// - [`MaskType::Alpha`]: draws the alpha of its own fills plus its drawn
///   subtree. Only a leaf with nothing of its own to draw stands in as an
///   opaque fill.
/// - [`MaskType::Vector`]: draws the mask's geometry coverage only (fills, fill
///   opacity and effects of the mask ignored).
/// - [`MaskType::Luminance`]: draws the mask's rendered colour and alpha so the
///   compositor can weight its sRGB channels by [`LUMINANCE_MASK_COEFFS`].
pub fn mask_stencil(mask: &Node) -> Node {
    let mut n = mask.clone();
    n.blend = BlendMode::Normal;
    n.effects.clear();
    n.stroke.paints.clear();
    match mask.mask_type {
        MaskType::Alpha => {
            if !has_own_fill(&n) && !draws_children(&n) {
                n.fills = vec![Paint::solid(Color::BLACK)];
            }
        }
        MaskType::Vector => {
            n.opacity = 1.0;
            if let NodeKind::Text(t) = &mut n.kind {
                t.style.fills = vec![Paint::solid(Color::BLACK)];
                for r in &mut t.runs {
                    r.style.fills = vec![Paint::solid(Color::BLACK)];
                }
            } else if !matches!(n.kind, NodeKind::Group) {
                n.fills = vec![Paint::solid(Color::BLACK)];
                if !matches!(n.kind, NodeKind::BooleanOp(_)) {
                    n.children.clear();
                }
            } else {
                n.fills.clear();
            }
        }
        MaskType::Luminance => {}
    }
    n
}

/// The stencil node for a descendant inside a [`MaskType::Vector`] group mask:
/// applies the [`MaskType::Vector`] outline rule to `node`.
pub fn vector_mask_descendant_stencil(node: &Node) -> Node {
    let mut v = node.clone();
    v.mask_type = MaskType::Vector;
    mask_stencil(&v)
}

/// The opaque box of a mask whose stencil takes its alpha from its children
/// alone (a frame, component or instance with children and no visible fill of
/// its own), drawn without those children; `None` for any other mask.
pub fn mask_frame_box(mask: &Node) -> Option<Node> {
    if matches!(mask.kind, NodeKind::Group) {
        return None;
    }
    if (mask.mask_type != MaskType::Vector && has_own_fill(mask)) || !draws_children(mask) {
        return None;
    }
    let mut n = mask_stencil(mask);
    n.children.clear();
    let color = if mask.mask_type == MaskType::Luminance { Color::WHITE } else { Color::BLACK };
    n.fills = vec![Paint::solid(color)];
    Some(n)
}

/// Converts `pixmap` in place so each pixel's alpha is its Rec. 709 sRGB
/// luminance times its alpha ([`LUMINANCE_MASK_COEFFS`]), ready for
/// `DestinationIn` compositing.
pub fn apply_luminance_mask(pixmap: &mut tiny_skia::Pixmap) {
    let [cr, cg, cb] = LUMINANCE_MASK_COEFFS;
    for px in pixmap.pixels_mut() {
        let lum = (cr * px.red() as f32 + cg * px.green() as f32 + cb * px.blue() as f32).round().clamp(0.0, 255.0) as u8;
        *px = tiny_skia::PremultipliedColorU8::from_rgba(0, 0, 0, lum).unwrap();
    }
}
