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

/// The paint a stroke is drawn with: the first visible one that is not a shader.
pub(crate) fn drawn_paint(st: &designos_doc::Stroke) -> Option<&designos_doc::Paint> {
    st.paints.iter().find(|p| p.visible && p.opacity > 0.0 && !matches!(p.kind, PaintKind::Shader(_)))
}

/// Whether `n` has a stroke that is exported: a visible paint and a width (stored, per side or bound).
pub(crate) fn stroke_is_drawn(n: &Node) -> bool {
    let st = &n.stroke;
    drawn_paint(st).is_some() && (st.weight > 0.0 || st.side_weights.is_some() || WIDTH_KEYS.iter().any(|key| n.variables.bindings.contains_key(*key)))
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
    /// Short note emitted as a code comment when CSS has no border-free primitive for a dashed or
    /// gradient per-side stroke whose padding is smaller than the stroke width.
    pub limit_note: Option<&'static str>,
    /// Whether every padding side is written from the stored padding plus [`Self::pad_extra`] and kept at 0 or
    /// more, even with nothing to add: set for every drawn stroke, so a negative stored side cannot make the
    /// whole `padding` shorthand invalid. A stroke that is not exported leaves the padding as stored.
    pub clamp_padding: bool,
}

impl GridStroke {
    /// Whether padding side `i` is written from the stored padding plus [`Self::pad_extra`] and kept at 0 or
    /// more. A shadow stroke and every drawn stroke ([`Self::clamp_padding`]) clamp every side: a negative
    /// side would make the whole `padding` shorthand invalid in CSS and drop the other sides with it.
    pub fn adjusts_padding(&self, i: usize) -> bool {
        self.pad_extra[i] != 0.0 || self.model == StrokeModel::Shadow || self.clamp_padding
    }

    /// The stroke is a CSS `border` and takes layout space.
    pub fn border(&self) -> bool {
        self.model == StrokeModel::Border
    }

    /// For [`StrokeModel::Shadow`]: `(x, y)` offset tokens of each stroked side in CSS (`css_fallback = true`,
    /// `var(--Name, fallback)` and `calc(-1 * var(...))`) or Tailwind (`css_fallback = false`, `var(--Name)`
    /// and `calc(-1*var(...))`).
    pub fn shadow_xy<'a>(
        &'a self,
        doc: &'a Document,
        n: &'a Node,
        css_fallback: bool,
        color_fmt: crate::util::ColorFormat,
    ) -> impl Iterator<Item = (String, String)> + 'a {
        let any_side_bound = SIDE_WEIGHT_KEYS.iter().any(|k| crate::codegen::bound(doc, n, k).is_some());
        self.shadow_offsets.into_iter().enumerate().filter_map(move |(i, off)| {
            let (x, y) = off?;
            let var = crate::codegen::bound(doc, n, SIDE_WEIGHT_KEYS[i])
                .or_else(|| (!any_side_bound && n.stroke.side_weights.is_none()).then(|| crate::codegen::bound(doc, n, "strokeWeight")).flatten());
            if let Some(v) = var {
                let token = if css_fallback { crate::codegen::stroke_width_var_css(doc, n, v, color_fmt) } else { crate::codegen::css_var(v) };
                let neg = if css_fallback { format!("calc(-1 * {token})") } else { format!("calc(-1*{token})") };
                Some(match i {
                    0 => ("0px".to_string(), token),
                    1 => (neg, "0px".to_string()),
                    2 => ("0px".to_string(), neg),
                    _ => (token, "0px".to_string()),
                })
            } else {
                Some((format!("{}px", crate::util::fmt_num(x)), format!("{}px", crate::util::fmt_num(y))))
            }
        })
    }
}

/// Stroke model of the grid frame `n` whose auto layout is `al`.
///
/// An Inside stroke is a border when the solver reserves its width (version 0 with `stroke_included`).
/// A per-side Inside stroke the solver does not reserve is inset shadows when it is solid (including
/// when side widths are bound to variables): they take no space, so content sits where the solver puts
/// it, and they paint under the children exactly as Studio paints a frame's stroke (a full-bleed child
/// covers it there too). A dashed or gradient per-side Inside stroke whose padding is at least the
/// stroke width on every side is a border whose padding shrinks by its width. When the padding on any
/// side is smaller than the stroke, CSS has no border-free dashed or gradient per-side stroke, so the
/// export keeps content in place with a solid inset `box-shadow` and writes a comment noting the CSS
/// limit. Every other stroke is an outline, offset inward for Inside and Center strokes so it still sits
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
    let outline = GridStroke { model: StrokeModel::Outline, pad_extra: reserved, shadow_offsets: none, limit_note: None, clamp_padding: stroke_is_drawn(n) };
    if !stroke_is_drawn(n) || st.align != StrokeAlign::Inside {
        return outline;
    }
    let sided = st.side_weights.is_some() || SIDE_WEIGHT_KEYS.iter().any(|key| n.variables.bindings.contains_key(*key));
    let gradient = drawn_paint(st).is_some_and(|p| !matches!(p.kind, PaintKind::Solid(_)));
    let reserves = reserved.iter().any(|&v| v > 0.0);
    if !(sided || gradient || reserves) {
        return outline;
    }
    let widths = st.side_weights.unwrap_or([st.weight; 4]);
    let dashed = st.dashes.iter().any(|d| *d > 0.0);
    let fits_in_padding = (0..4).all(|i| resolved_al.padding[i].max(0.0) + reserved[i] >= widths[i].max(0.0));
    if sided && !reserves && ((!gradient && !dashed) || !fits_in_padding) {
        let [t, r, b, l] = widths.map(|w| w.max(0.0));
        let side_bound: [bool; 4] = std::array::from_fn(|i| crate::codegen::bound(doc, n, SIDE_WEIGHT_KEYS[i]).is_some());
        let limit_note = match (dashed, gradient) {
            (true, true) => {
                Some("CSS cannot draw a per-side dashed gradient stroke without a border that would shift grid content; drawn as a solid inset box-shadow")
            }
            (true, false) => Some("CSS cannot draw a per-side dashed stroke without a border that would shift grid content; drawn as a solid inset box-shadow"),
            (false, true) => {
                Some("CSS cannot draw a per-side gradient stroke without a border that would shift grid content; drawn as a solid inset box-shadow")
            }
            (false, false) => None,
        };
        return GridStroke {
            model: StrokeModel::Shadow,
            pad_extra: [0.0; 4],
            shadow_offsets: [
                (t > 0.0 || side_bound[0]).then_some((0.0, t)),
                (r > 0.0 || side_bound[1]).then_some((-r, 0.0)),
                (b > 0.0 || side_bound[2]).then_some((0.0, -b)),
                (l > 0.0 || side_bound[3]).then_some((l, 0.0)),
            ],
            limit_note,
            clamp_padding: true,
        };
    }
    GridStroke {
        model: StrokeModel::Border,
        pad_extra: std::array::from_fn(|i| reserved[i] - widths[i].max(0.0)),
        shadow_offsets: none,
        limit_note: None,
        clamp_padding: true,
    }
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
        let (mut doc, grid) = stroked(1, false, StrokeAlign::Inside, Some([4.0, 1.0, 0.0, 3.0]));
        if let designos_doc::NodeKind::Frame(f) = &mut doc.get_mut(grid).unwrap().kind {
            f.auto_layout.as_mut().unwrap().padding = [4.0; 4];
        }
        let got = grid_stroke_of(&doc, doc.get(grid).unwrap()).unwrap();
        is(got, StrokeModel::Shadow, [0.0; 4]);
        assert_eq!(got.shadow_offsets, [Some((0.0, 4.0)), Some((-1.0, 0.0)), None, Some((3.0, 0.0))]);
    }

    #[test]
    fn per_side_stroke_wider_than_the_padding_is_still_an_inset_shadow() {
        is(model(1, false, StrokeAlign::Inside, Some([4.0, 1.0, 2.0, 3.0])), StrokeModel::Shadow, [0.0; 4]);
    }

    #[test]
    fn dashed_per_side_stroke_the_solver_does_not_reserve_is_a_border_that_shrinks_the_padding() {
        let (mut doc, grid) = stroked(1, false, StrokeAlign::Inside, Some([4.0, 1.0, 2.0, 3.0]));
        if let designos_doc::NodeKind::Frame(f) = &mut doc.get_mut(grid).unwrap().kind {
            f.auto_layout.as_mut().unwrap().padding = [8.0; 4];
        }
        doc.get_mut(grid).unwrap().stroke.dashes = vec![4.0, 2.0];
        is(grid_stroke_of(&doc, doc.get(grid).unwrap()).unwrap(), StrokeModel::Border, [-4.0, -1.0, -2.0, -3.0]);
    }

    #[test]
    fn dashed_per_side_stroke_wider_than_the_padding_stays_an_inset_shadow_with_limit_note() {
        let (mut doc, grid) = stroked(1, false, StrokeAlign::Inside, Some([4.0, 1.0, 2.0, 3.0]));
        doc.get_mut(grid).unwrap().stroke.dashes = vec![4.0, 2.0];
        let got = grid_stroke_of(&doc, doc.get(grid).unwrap()).unwrap();
        is(got, StrokeModel::Shadow, [0.0; 4]);
        assert!(got.limit_note.is_some());
    }
}
