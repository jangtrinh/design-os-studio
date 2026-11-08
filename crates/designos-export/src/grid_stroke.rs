//! How the stroke of a grid frame reaches CSS layout, so a browser puts the content where the layout
//! solver puts it.
//!
//! The solver insets a grid's content by [`designos_doc::Stroke::layout_inset`] when the frame is
//! layout version 0 with `stroke_included` (a Center stroke by half its weight, an Inside one by the
//! full weight, an Outside one not at all), and never in version 1. A CSS `border` takes layout space
//! on every side it is drawn on; an `outline` takes none. The export therefore chooses the model per
//! frame and folds the difference into the padding it writes: `padding = padding + inset - border`.

use designos_doc::{AutoLayout, Document, Node, PaintKind, StrokeAlign};

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

/// How a grid frame's stroke is drawn in CSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StrokeModel {
    /// A `border`: it takes layout space.
    Border,
    /// An `outline`: it takes none.
    Outline,
    /// One inset `box-shadow` layer per stroked side: it takes none, and unlike an outline it can differ
    /// per side. Its layers are [`GridStroke::shadow_offsets`].
    Shadow,
}

/// The CSS model of a grid frame's stroke and the padding that keeps content on the solver's boxes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GridStroke {
    pub model: StrokeModel,
    /// Pixels to add to each padding side (top, right, bottom, left) on top of the stored padding.
    /// Negative when a border takes more space than the solver reserves; the padding then shrinks.
    pub pad_extra: [f32; 4],
    /// For [`StrokeModel::Shadow`]: the offset of the inset shadow layer of each stroked side (top,
    /// right, bottom, left), so a layer is a band of that side's width.
    pub shadow_offsets: [Option<(f32, f32)>; 4],
}

impl GridStroke {
    /// The stroke is a CSS `border` and takes layout space.
    pub fn border(&self) -> bool {
        self.model == StrokeModel::Border
    }
}

/// Stroke model of the grid frame `n` whose auto layout is `al`.
///
/// An Inside stroke is a border when the solver reserves its width (version 0 with `stroke_included`).
/// A per-side Inside stroke the solver does not reserve is inset shadows, which take no space on any
/// side, unless it is dashed, a gradient, or its widths are variables (then a border, whose padding
/// shrinks by its width and stops at 0). A gradient Inside stroke is a border (`border-image` needs
/// one). Every other stroke is an outline, offset inward for Inside and Center strokes so it still sits
/// on the frame's edge.
pub(crate) fn grid_stroke(doc: &Document, n: &Node, al: &AutoLayout) -> GridStroke {
    let resolved = designos_doc::resolve_node(doc, n);
    let resolved_al = resolved.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).unwrap_or(al);
    let st = &resolved.stroke;
    let reserved = {
        let effective = resolved_al.effective_padding(st);
        std::array::from_fn::<f32, 4, _>(|i| effective[i] - resolved_al.padding[i])
    };
    let none = [None; 4];
    let outline = GridStroke { model: StrokeModel::Outline, pad_extra: reserved, shadow_offsets: none };
    if !stroke_is_drawn(n) || st.align != StrokeAlign::Inside {
        return outline;
    }
    let sided = st.side_weights.is_some() || SIDE_WEIGHT_KEYS.iter().any(|key| n.variables.bindings.contains_key(*key));
    let gradient = st.paints.iter().find(|p| p.visible && p.opacity > 0.0).is_some_and(|p| !matches!(p.kind, PaintKind::Solid(_)));
    let reserves = reserved.iter().any(|&v| v > 0.0);
    if !(sided || gradient || reserves) {
        return outline;
    }
    let widths = st.side_weights.unwrap_or([st.weight; 4]);
    let bound = WIDTH_KEYS.iter().any(|key| n.variables.bindings.contains_key(*key));
    let dashed = st.dashes.iter().any(|d| *d > 0.0);
    if sided && !reserves && !gradient && !dashed && !bound {
        let [t, r, b, l] = widths.map(|w| w.max(0.0));
        let layer = |w: f32, at: (f32, f32)| (w > 0.0).then_some(at);
        return GridStroke {
            model: StrokeModel::Shadow,
            pad_extra: [0.0; 4],
            shadow_offsets: [layer(t, (0.0, t)), layer(r, (-r, 0.0)), layer(b, (0.0, -b)), layer(l, (l, 0.0))],
        };
    }
    GridStroke { model: StrokeModel::Border, pad_extra: std::array::from_fn(|i| reserved[i] - widths[i].max(0.0)), shadow_offsets: none }
}

/// [`grid_stroke`] for any node: `None` unless `n` is a grid container (a frame, component, component
/// set or instance with a Grid auto layout).
pub(crate) fn grid_stroke_of(doc: &Document, n: &Node) -> Option<GridStroke> {
    let al = n.kind.frame_props().and_then(|f| f.auto_layout.as_ref())?;
    (al.direction == designos_doc::LayoutDirection::Grid).then(|| grid_stroke(doc, n, al))
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

    fn is(got: GridStroke, model: StrokeModel, pad_extra: [f32; 4]) {
        assert_eq!((got.model, got.pad_extra), (model, pad_extra), "{got:?}");
    }

    #[test]
    fn included_inside_stroke_is_a_border_that_needs_no_extra_padding() {
        is(model(0, true, StrokeAlign::Inside, None), StrokeModel::Border, [0.0; 4]);
        is(model(0, true, StrokeAlign::Inside, Some([4.0, 1.0, 2.0, 3.0])), StrokeModel::Border, [0.0; 4]);
    }

    #[test]
    fn included_center_stroke_is_an_outline_that_pads_by_half_the_weight() {
        is(model(0, true, StrokeAlign::Center, None), StrokeModel::Outline, [1.0; 4]);
    }

    #[test]
    fn strokes_the_solver_does_not_reserve_take_no_layout_space() {
        is(model(1, false, StrokeAlign::Inside, None), StrokeModel::Outline, [0.0; 4]);
        is(model(0, false, StrokeAlign::Inside, None), StrokeModel::Outline, [0.0; 4]);
        is(model(0, true, StrokeAlign::Outside, None), StrokeModel::Outline, [0.0; 4]);
    }

    #[test]
    fn per_side_stroke_the_solver_does_not_reserve_is_inset_shadows_with_no_padding_change() {
        let got = model(1, false, StrokeAlign::Inside, Some([4.0, 1.0, 0.0, 3.0]));
        is(got, StrokeModel::Shadow, [0.0; 4]);
        assert_eq!(got.shadow_offsets, [Some((0.0, 4.0)), Some((-1.0, 0.0)), None, Some((3.0, 0.0))]);
    }

    #[test]
    fn dashed_per_side_stroke_the_solver_does_not_reserve_is_a_border_that_shrinks_the_padding() {
        let (mut doc, grid) = stroked(1, false, StrokeAlign::Inside, Some([4.0, 1.0, 2.0, 3.0]));
        doc.get_mut(grid).unwrap().stroke.dashes = vec![4.0, 2.0];
        is(grid_stroke_of(&doc, doc.get(grid).unwrap()).unwrap(), StrokeModel::Border, [-4.0, -1.0, -2.0, -3.0]);
    }
}
