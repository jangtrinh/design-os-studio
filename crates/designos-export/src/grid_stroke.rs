//! How the stroke of a grid frame reaches CSS layout, so a browser puts the content where the layout
//! solver puts it.
//!
//! The solver insets a grid's content by [`designos_doc::Stroke::layout_inset`] when the frame is
//! layout version 0 with `stroke_included` (a Center stroke by half its weight, an Inside one by the
//! full weight, an Outside one not at all), and never in version 1. A CSS `border` takes layout space
//! on every side it is drawn on; an `outline` takes none. The export therefore chooses the model per
//! frame and folds the difference into the padding it writes: `padding = padding + inset - border`.

use designos_doc::{AutoLayout, Document, Node, NodeKind, PaintKind, StrokeAlign};

/// The stroke keys a variable can bind to a single side of the stroke.
const SIDE_WEIGHT_KEYS: [&str; 4] = ["strokeTopWeight", "strokeRightWeight", "strokeBottomWeight", "strokeLeftWeight"];

/// Every key that binds a stroke width.
const WIDTH_KEYS: [&str; 5] = ["strokeWeight", "strokeTopWeight", "strokeRightWeight", "strokeBottomWeight", "strokeLeftWeight"];

/// Whether `n` has a stroke that is exported: a visible paint and a width (stored, per side or bound).
pub(crate) fn stroke_is_drawn(n: &Node) -> bool {
    let st = &n.stroke;
    st.paints.iter().any(|p| p.visible && p.opacity > 0.0 && !matches!(p.kind, PaintKind::Shader(_)))
        && (st.weight > 0.0 || st.side_weights.is_some() || WIDTH_KEYS.iter().any(|key| n.variables.bindings.contains_key(*key)))
}

/// The CSS model of a grid frame's stroke and the padding that keeps content on the solver's boxes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GridStroke {
    /// The stroke is a CSS `border` (it takes layout space) and not an `outline` (it takes none).
    pub border: bool,
    /// Pixels to add to each padding side (top, right, bottom, left) on top of the stored padding.
    /// Negative when a border takes more space than the solver reserves; the padding then shrinks.
    pub pad_extra: [f32; 4],
}

/// Stroke model of the grid frame `n` whose auto layout is `al`.
///
/// An Inside stroke is a border when it is drawn per side, is a gradient (`border-image` needs a
/// border), or the solver reserves its width (version 0 with `stroke_included`). Every other stroke
/// is an outline, offset inward for Inside and Center strokes so it still sits on the frame's edge.
pub(crate) fn grid_stroke(doc: &Document, n: &Node, al: &AutoLayout) -> GridStroke {
    let resolved = designos_doc::resolve_node(doc, n);
    let resolved_al = resolved.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).unwrap_or(al);
    let st = &resolved.stroke;
    let reserved = {
        let effective = resolved_al.effective_padding(st);
        std::array::from_fn::<f32, 4, _>(|i| effective[i] - resolved_al.padding[i])
    };
    let border = stroke_is_drawn(n) && st.align == StrokeAlign::Inside && {
        let sided = st.side_weights.is_some() || SIDE_WEIGHT_KEYS.iter().any(|key| n.variables.bindings.contains_key(*key));
        let gradient = st.paints.iter().find(|p| p.visible && p.opacity > 0.0).is_some_and(|p| !matches!(p.kind, PaintKind::Solid(_)));
        sided || gradient || reserved.iter().any(|&v| v > 0.0)
    };
    let taken = if border { st.side_weights.unwrap_or([st.weight; 4]) } else { [0.0; 4] };
    GridStroke { border, pad_extra: std::array::from_fn(|i| reserved[i] - taken[i].max(0.0)) }
}

/// [`grid_stroke`] for any node: `None` unless `n` is a grid frame.
pub(crate) fn grid_stroke_of(doc: &Document, n: &Node) -> Option<GridStroke> {
    let al = n.kind.frame_props().and_then(|f| f.auto_layout.as_ref())?;
    (al.direction == designos_doc::LayoutDirection::Grid && matches!(n.kind, NodeKind::Frame(_))).then(|| grid_stroke(doc, n, al))
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Color, Paint, Sizing, Vec2};

    fn stroked(version: u8, included: bool, align: StrokeAlign, sides: Option<[f32; 4]>) -> (Document, designos_doc::NodeId) {
        let al = AutoLayout { grid_columns: 2, layout_version: version, stroke_included: included, primary_sizing: Sizing::Fixed, ..Default::default() };
        let (mut doc, grid) = crate::codegen::fixture::grid_frame(al, Vec2::new(200.0, 100.0));
        let n = doc.get_mut(grid).unwrap();
        n.stroke.paints = vec![Paint::solid(Color::rgb(0x33, 0x33, 0x33))];
        n.stroke.weight = 2.0;
        n.stroke.align = align;
        n.stroke.side_weights = sides;
        (doc, grid)
    }

    fn model(version: u8, included: bool, align: StrokeAlign, sides: Option<[f32; 4]>) -> GridStroke {
        let (doc, grid) = stroked(version, included, align, sides);
        grid_stroke_of(&doc, doc.get(grid).unwrap()).unwrap()
    }

    #[test]
    fn included_inside_stroke_is_a_border_that_needs_no_extra_padding() {
        assert_eq!(model(0, true, StrokeAlign::Inside, None), GridStroke { border: true, pad_extra: [0.0; 4] });
        assert_eq!(model(0, true, StrokeAlign::Inside, Some([4.0, 1.0, 2.0, 3.0])), GridStroke { border: true, pad_extra: [0.0; 4] });
    }

    #[test]
    fn included_center_stroke_is_an_outline_that_pads_by_half_the_weight() {
        assert_eq!(model(0, true, StrokeAlign::Center, None), GridStroke { border: false, pad_extra: [1.0; 4] });
    }

    #[test]
    fn strokes_the_solver_does_not_reserve_take_no_layout_space() {
        assert_eq!(model(1, false, StrokeAlign::Inside, None), GridStroke { border: false, pad_extra: [0.0; 4] });
        assert_eq!(model(0, false, StrokeAlign::Inside, None), GridStroke { border: false, pad_extra: [0.0; 4] });
        assert_eq!(model(0, true, StrokeAlign::Outside, None), GridStroke { border: false, pad_extra: [0.0; 4] });
    }

    #[test]
    fn per_side_border_the_solver_does_not_reserve_shrinks_the_padding() {
        assert_eq!(model(1, false, StrokeAlign::Inside, Some([4.0, 1.0, 2.0, 3.0])), GridStroke { border: true, pad_extra: [-4.0, -1.0, -2.0, -3.0] });
    }
}
