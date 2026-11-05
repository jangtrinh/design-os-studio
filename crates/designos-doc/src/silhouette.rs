//! Which geometry an effect silhouette (drop shadow, inner shadow, background blur, glass,
//! noise, texture) is drawn from. The CPU renderer (`designos-render`) and the GPU compiler
//! (`designos-gpu`) both call [`silhouette_source`], so the two cannot disagree on the rule.

use crate::{Node, NodeKind, Paint};

/// What a silhouette is drawn for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SilhouetteMode {
    /// The shape's own alpha: the source of a shadow, glass or inner shadow.
    Alpha,
    /// Fill-agnostic footprint at full alpha: noise, texture grain and glass masks. A node with
    /// no shape of its own (a group) follows its children at their own alpha.
    Opaque,
    /// The mask of a background blur: the same footprint as `Opaque`, except that a node with
    /// no shape of its own (a group) takes the union of its children's opaque silhouettes. A
    /// translucent child then blurs the backdrop behind it at full strength, so the pixels
    /// under it stay as opaque as the backdrop was.
    BackdropBlur,
    /// The mask that removes a drop shadow behind the node (show behind off).
    Knockout,
}

impl SilhouetteMode {
    /// Whether paint alpha is ignored (every covered pixel counts fully).
    pub fn ignores_alpha(self) -> bool {
        !matches!(self, SilhouetteMode::Alpha)
    }

    /// The coverage one run of text glyphs adds to a silhouette of this mode; `fills` are the
    /// fills of that run and `n` is the text node.
    ///
    /// `Alpha` follows the most opaque visible fill. `Opaque` and `BackdropBlur` cover every
    /// glyph fully.
    /// `Knockout` covers a glyph fully when the text paints anything (a fill that is not fully
    /// transparent, or a stroke) and adds nothing when it paints nothing. The glyph outline is
    /// the footprint in every mode; strokes are never drawn into it.
    pub fn text_run_alpha(self, n: &Node, fills: &[Paint]) -> f32 {
        match self {
            SilhouetteMode::Alpha => fills.iter().filter(|p| p.visible).map(Paint::silhouette_alpha).fold(0.0, f32::max),
            SilhouetteMode::Opaque | SilhouetteMode::BackdropBlur => 1.0,
            SilhouetteMode::Knockout => {
                if fills.iter().any(|p| paints_in(p, self)) || has_visible_strokes(n, self) {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }
}

/// How the shadow knockout of a fill-less, stroke-less fillable node is built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilllessKnockout {
    /// F1: no knockout. The shadow of the children stays visible under them.
    None,
    /// F2: the union of the children's opaque silhouettes, never the frame rectangle.
    ChildrenOpaque,
}

/// The rule in force for #520 until the Figma read settles it (spec 520, section 5).
pub const FILLLESS_KNOCKOUT: FilllessKnockout = FilllessKnockout::ChildrenOpaque;

/// The geometry a node's silhouette is built from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SilhouetteSource {
    /// Text glyph outlines.
    Text,
    /// The node's own shape: fill, stroke footprint and growth.
    OwnShape,
    /// The node paints nothing itself; its drawn children form the silhouette.
    Children,
    /// Knockout of a fill-less node: each visible child's own opaque silhouette.
    ChildrenOpaque,
    /// No silhouette.
    Nothing,
}

/// Decides the silhouette source of `n`.
///
/// `shape_fillable` is `None` when the node has no geometry, otherwise whether fills apply to
/// it (lines are not fillable). In `Opaque` and `BackdropBlur` mode a fillable shape counts even
/// without a visible fill.
///
/// `Knockout` removes only what the node paints, so a fill or stroke whose paint is fully
/// transparent counts as no paint there (`Alpha`, `Opaque` and `BackdropBlur` keep every visible
/// paint). A fillable node left with no paint is [`SilhouetteSource::Nothing`] when it is a leaf,
/// and follows [`FILLLESS_KNOCKOUT`] when it has children, instead of covering its rectangle.
///
/// A node with no shape of its own and with children (a group) takes its children at their own
/// alpha in `Alpha` and `Opaque`, and as their opaque silhouettes in `BackdropBlur` and `Knockout`.
///
/// When the node clips its content, the caller clips the children by the node's shape in both
/// the silhouette and the knockout.
pub fn silhouette_source(n: &Node, shape_fillable: Option<bool>, mode: SilhouetteMode) -> SilhouetteSource {
    if matches!(n.kind, NodeKind::Text(_)) {
        return SilhouetteSource::Text;
    }
    let has_children = !n.children.is_empty() && !matches!(n.kind, NodeKind::BooleanOp(_));
    if let Some(fillable) = shape_fillable {
        if (fillable && has_visible_fills(n, mode)) || has_visible_strokes(n, mode) {
            return SilhouetteSource::OwnShape;
        }
        if fillable && matches!(mode, SilhouetteMode::Opaque | SilhouetteMode::BackdropBlur) {
            return SilhouetteSource::OwnShape;
        }
    }
    if has_children {
        match mode {
            SilhouetteMode::BackdropBlur => SilhouetteSource::ChildrenOpaque,
            SilhouetteMode::Knockout => match FILLLESS_KNOCKOUT {
                FilllessKnockout::None => SilhouetteSource::Nothing,
                FilllessKnockout::ChildrenOpaque => SilhouetteSource::ChildrenOpaque,
            },
            SilhouetteMode::Alpha | SilhouetteMode::Opaque => SilhouetteSource::Children,
        }
    } else {
        SilhouetteSource::Nothing
    }
}

/// Whether `p` paints something a silhouette of `mode` accounts for: a visible paint, and in
/// `Knockout` mode one that is not fully transparent.
fn paints_in(p: &Paint, mode: SilhouetteMode) -> bool {
    p.visible && (mode != SilhouetteMode::Knockout || p.silhouette_alpha() > 0.0)
}

fn has_visible_fills(n: &Node, mode: SilhouetteMode) -> bool {
    n.fills.iter().any(|p| paints_in(p, mode))
}

/// A stroke exists with a weight or with per-side weights.
fn has_visible_strokes(n: &Node, mode: SilhouetteMode) -> bool {
    (n.stroke.weight > 0.0 || n.stroke.side_weights.is_some()) && n.stroke.paints.iter().any(|p| paints_in(p, mode))
}

impl Node {
    /// Whether a background blur on this node has a painted surface to blur behind. The CPU renderer
    /// and the GPU compiler both ask this, so the two cannot disagree.
    ///
    /// A node whose silhouette is its own shape or its own glyphs blurs only while it paints
    /// something: a visible fill (on text, one on the base style or on any run) or a visible stroke.
    /// A node with no shape of its own (a group) takes its silhouette from its children, and a board
    /// item is painted through its lowered body, so for those the node's own paint says nothing and
    /// the effect stays.
    pub fn background_blur_has_surface(&self) -> bool {
        let own_silhouette = !matches!(self.kind, NodeKind::Page { .. } | NodeKind::Group | NodeKind::Unknown { .. }) && !self.kind.is_board_item();
        if !own_silhouette {
            return true;
        }
        let mode = SilhouetteMode::Opaque;
        let filled = match &self.kind {
            NodeKind::Text(t) => std::iter::once(&t.style).chain(t.runs.iter().map(|r| &r.style)).any(|s| s.fills.iter().any(|p| paints_in(p, mode))),
            _ => has_visible_fills(self, mode),
        };
        filled || has_visible_strokes(self, mode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BooleanOp, Color, NodeId, Paint, Rect, TextRun, TextStyle};

    fn rect() -> Rect {
        Rect::new(0.0, 0.0, 10.0, 10.0)
    }
    fn bare(mut n: Node) -> Node {
        n.fills.clear();
        n
    }
    fn with_child(mut n: Node) -> Node {
        n.children.push(NodeId(99));
        n
    }
    fn stroked(mut n: Node) -> Node {
        n.stroke.weight = 2.0;
        n.stroke.paints = vec![Paint::solid(Color::BLACK)];
        n
    }
    fn src(n: &Node, opaque: bool) -> SilhouetteSource {
        silhouette_source(n, Some(true), if opaque { SilhouetteMode::Opaque } else { SilhouetteMode::Alpha })
    }
    fn knock(n: &Node) -> SilhouetteSource {
        silhouette_source(n, Some(true), SilhouetteMode::Knockout)
    }

    #[test]
    fn frame_with_visible_fill_uses_its_own_shape() {
        let n = with_child(Node::frame(NodeId(1), rect()));
        assert_eq!(src(&n, false), SilhouetteSource::OwnShape);
    }

    #[test]
    fn fill_less_frame_with_children_uses_the_children() {
        let n = with_child(bare(Node::frame(NodeId(1), rect())));
        assert_eq!(src(&n, false), SilhouetteSource::Children);
    }

    #[test]
    fn fill_less_clipping_frame_uses_children() {
        let mut n = with_child(bare(Node::frame(NodeId(1), rect())));
        n.kind.frame_props_mut().unwrap().clips_content = true;
        assert_eq!(src(&n, false), SilhouetteSource::Children);
    }

    #[test]
    fn hidden_fill_does_not_count() {
        let mut n = with_child(Node::frame(NodeId(1), rect()));
        n.fills[0].visible = false;
        assert_eq!(src(&n, false), SilhouetteSource::Children);
        assert_eq!(src(&bare(Node::frame(NodeId(1), rect())), false), SilhouetteSource::Nothing);
        let mut lone = Node::rectangle(NodeId(1), rect());
        lone.fills[0].visible = false;
        assert_eq!(src(&lone, false), SilhouetteSource::Nothing);
    }

    #[test]
    fn hidden_stroke_or_zero_weight_does_not_count() {
        let mut n = stroked(bare(Node::rectangle(NodeId(1), rect())));
        n.stroke.paints[0].visible = false;
        assert_eq!(src(&n, false), SilhouetteSource::Nothing);
        let mut z = stroked(bare(Node::rectangle(NodeId(1), rect())));
        z.stroke.weight = 0.0;
        assert_eq!(src(&z, false), SilhouetteSource::Nothing);
        z.stroke.side_weights = Some([1.0, 0.0, 0.0, 0.0]);
        assert_eq!(src(&z, false), SilhouetteSource::OwnShape);
    }

    #[test]
    fn hidden_children_are_decided_at_draw_time() {
        // The decision sees only the node, so a hidden child still yields `Children`; the
        // children walk skips it, which leaves the silhouette empty in both renderers.
        let mut n = with_child(bare(Node::frame(NodeId(1), rect())));
        n.children.clear();
        assert_eq!(src(&n, false), SilhouetteSource::Nothing);
        n.children.push(NodeId(7));
        assert_eq!(src(&n, false), SilhouetteSource::Children);
    }

    #[test]
    fn stroke_only_node_uses_its_own_shape() {
        let n = stroked(bare(Node::rectangle(NodeId(1), rect())));
        assert_eq!(src(&n, false), SilhouetteSource::OwnShape);
        // a stroke wins over children
        let f = with_child(stroked(bare(Node::frame(NodeId(1), rect()))));
        assert_eq!(src(&f, false), SilhouetteSource::OwnShape);
    }

    #[test]
    fn opaque_mode_counts_fill_less_fillable_shapes_only() {
        let r = bare(Node::rectangle(NodeId(1), rect()));
        assert_eq!(src(&r, false), SilhouetteSource::Nothing);
        assert_eq!(src(&r, true), SilhouetteSource::OwnShape);
        // not fillable (a line): opaque does not help
        assert_eq!(silhouette_source(&r, Some(false), SilhouetteMode::Opaque), SilhouetteSource::Nothing);
        // opaque frame with children takes its own shape, not the children
        let f = with_child(bare(Node::frame(NodeId(1), rect())));
        assert_eq!(src(&f, true), SilhouetteSource::OwnShape);
    }

    #[test]
    fn unfillable_fill_is_ignored_but_stroke_counts() {
        let n = Node::rectangle(NodeId(1), rect());
        assert_eq!(silhouette_source(&n, Some(false), SilhouetteMode::Alpha), SilhouetteSource::Nothing);
        assert_eq!(silhouette_source(&stroked(n), Some(false), SilhouetteMode::Alpha), SilhouetteSource::OwnShape);
    }

    #[test]
    fn text_node_uses_glyphs() {
        let t = Node::text(NodeId(1), rect(), "Hi", TextStyle::default());
        assert_eq!(silhouette_source(&t, None, SilhouetteMode::Alpha), SilhouetteSource::Text);
        assert_eq!(silhouette_source(&with_child(t), Some(true), SilhouetteMode::Opaque), SilhouetteSource::Text);
    }

    #[test]
    fn no_shape_falls_back_to_children_and_boolean_ops_draw_none() {
        let g = with_child(Node::new(NodeId(1), NodeKind::Group));
        assert_eq!(silhouette_source(&g, None, SilhouetteMode::Alpha), SilhouetteSource::Children);
        let b = with_child(Node::new(NodeId(1), NodeKind::BooleanOp(BooleanOp::Union)));
        assert_eq!(silhouette_source(&b, None, SilhouetteMode::Alpha), SilhouetteSource::Nothing);
    }

    #[test]
    fn knockout_of_fill_less_frame_is_its_children_not_its_rectangle() {
        let f = with_child(bare(Node::frame(NodeId(1), rect())));
        assert_eq!(knock(&f), SilhouetteSource::ChildrenOpaque);
        // the opaque mode used by grain and glass still covers the rectangle
        assert_eq!(src(&f, true), SilhouetteSource::OwnShape);
        // without children a paint-free frame adds nothing to the knockout
        assert_eq!(knock(&bare(Node::frame(NodeId(1), rect()))), SilhouetteSource::Nothing);
    }

    fn zero_opacity_paint() -> Paint {
        let mut p = Paint::solid(Color::WHITE);
        p.opacity = 0.0;
        p
    }

    #[test]
    fn knockout_ignores_zero_alpha_fills_and_strokes() {
        // a visible fill at 0% opacity, no children: nothing to knock out
        let mut leaf = Node::frame(NodeId(1), rect());
        leaf.fills = vec![zero_opacity_paint()];
        assert_eq!(knock(&leaf), SilhouetteSource::Nothing);

        // a visible fill whose colour is fully transparent, with children: the children knock out
        let mut parent = Node::frame(NodeId(2), rect());
        parent.fills = vec![Paint::solid(Color::TRANSPARENT)];
        assert_eq!(knock(&with_child(parent)), SilhouetteSource::ChildrenOpaque);
        let mut parent = Node::frame(NodeId(2), rect());
        parent.fills = vec![zero_opacity_paint()];
        assert_eq!(knock(&with_child(parent)), SilhouetteSource::ChildrenOpaque);

        // a stroke whose paint is fully transparent
        let mut zero_stroke = bare(Node::frame(NodeId(4), rect()));
        zero_stroke.stroke.weight = 2.0;
        zero_stroke.stroke.paints = vec![zero_opacity_paint()];
        assert_eq!(knock(&zero_stroke), SilhouetteSource::Nothing);
        assert_eq!(knock(&with_child(zero_stroke)), SilhouetteSource::ChildrenOpaque);
    }

    #[test]
    fn alpha_and_opaque_keep_every_visible_paint_whatever_its_alpha() {
        // the shadow, glass and grain of a 0% fill frame are drawn from its own shape (at alpha 0)
        let mut leaf = Node::frame(NodeId(1), rect());
        leaf.fills = vec![zero_opacity_paint()];
        assert_eq!(src(&leaf, false), SilhouetteSource::OwnShape);
        assert_eq!(src(&leaf, true), SilhouetteSource::OwnShape);
        let mut parent = Node::frame(NodeId(2), rect());
        parent.fills = vec![Paint::solid(Color::TRANSPARENT)];
        let parent = with_child(parent);
        assert_eq!(src(&parent, false), SilhouetteSource::OwnShape);
        assert_eq!(src(&parent, true), SilhouetteSource::OwnShape);

        // the stroke footprint of a 0% stroke still casts, and still counts as a stroke
        let mut zero_stroke = bare(Node::frame(NodeId(4), rect()));
        zero_stroke.stroke.weight = 2.0;
        zero_stroke.stroke.paints = vec![zero_opacity_paint()];
        assert_eq!(src(&zero_stroke, false), SilhouetteSource::OwnShape);
        assert_eq!(src(&with_child(zero_stroke), false), SilhouetteSource::OwnShape);
    }

    #[test]
    fn knockout_of_filled_or_stroked_node_is_its_own_shape() {
        assert_eq!(knock(&with_child(Node::frame(NodeId(1), rect()))), SilhouetteSource::OwnShape);
        assert_eq!(knock(&with_child(stroked(bare(Node::frame(NodeId(1), rect()))))), SilhouetteSource::OwnShape);
    }

    #[test]
    fn group_mask_follows_children_alpha_except_for_a_backdrop_blur() {
        let g = with_child(Node::new(NodeId(1), NodeKind::Group));
        // grain, glass and texture masks keep the children at their own alpha
        assert_eq!(silhouette_source(&g, None, SilhouetteMode::Opaque), SilhouetteSource::Children);
        assert_eq!(silhouette_source(&g, None, SilhouetteMode::Alpha), SilhouetteSource::Children);
        // a backdrop blur needs the children's footprint at full alpha
        assert_eq!(silhouette_source(&g, None, SilhouetteMode::BackdropBlur), SilhouetteSource::ChildrenOpaque);
        // a group without children has no footprint in any mode
        let empty = Node::new(NodeId(1), NodeKind::Group);
        assert_eq!(silhouette_source(&empty, None, SilhouetteMode::BackdropBlur), SilhouetteSource::Nothing);
    }

    #[test]
    fn backdrop_blur_mask_matches_opaque_for_every_node_that_has_a_shape() {
        let modes = [SilhouetteMode::Opaque, SilhouetteMode::BackdropBlur];
        let same = |n: &Node, fillable: Option<bool>| {
            let [a, b] = modes.map(|m| silhouette_source(n, fillable, m));
            assert_eq!(a, b, "{:?}", n.kind);
            a
        };
        // fill-less fillable shapes and frames keep their own footprint, children or not
        assert_eq!(same(&bare(Node::rectangle(NodeId(1), rect())), Some(true)), SilhouetteSource::OwnShape);
        assert_eq!(same(&with_child(bare(Node::frame(NodeId(1), rect()))), Some(true)), SilhouetteSource::OwnShape);
        // filled, stroked, not fillable and glyph sources agree too
        assert_eq!(same(&with_child(Node::frame(NodeId(1), rect())), Some(true)), SilhouetteSource::OwnShape);
        assert_eq!(same(&stroked(bare(Node::rectangle(NodeId(1), rect()))), Some(false)), SilhouetteSource::OwnShape);
        assert_eq!(same(&bare(Node::rectangle(NodeId(1), rect())), Some(false)), SilhouetteSource::Nothing);
        assert_eq!(same(&Node::text(NodeId(1), rect(), "Hi", TextStyle::default()), None), SilhouetteSource::Text);
        // a 0% fill and a fully transparent fill still count, unlike in a knockout
        let mut pane = Node::rectangle(NodeId(1), rect());
        pane.fills = vec![Paint::solid(Color::TRANSPARENT)];
        assert_eq!(same(&pane, Some(true)), SilhouetteSource::OwnShape);
    }

    #[test]
    fn knockout_of_group_and_text_is_children_opaque_and_text() {
        let g = with_child(Node::new(NodeId(1), NodeKind::Group));
        assert_eq!(silhouette_source(&g, None, SilhouetteMode::Knockout), SilhouetteSource::ChildrenOpaque);
        assert_eq!(silhouette_source(&g, None, SilhouetteMode::Alpha), SilhouetteSource::Children);
        let t = Node::text(NodeId(1), rect(), "Hi", TextStyle::default());
        assert_eq!(silhouette_source(&t, None, SilhouetteMode::Knockout), SilhouetteSource::Text);
    }

    fn run_alpha(mode: SilhouetteMode, fills: &[Paint], stroke: Option<Paint>) -> f32 {
        let mut t = Node::text(NodeId(1), rect(), "Hi", TextStyle::default());
        if let Some(p) = stroke {
            t.stroke.weight = 1.0;
            t.stroke.paints = vec![p];
        }
        mode.text_run_alpha(&t, fills)
    }

    #[test]
    fn text_run_alpha_follows_the_mode() {
        let half = {
            let mut p = Paint::solid(Color::BLACK);
            p.opacity = 0.5;
            p
        };
        let mut hidden = Paint::solid(Color::BLACK);
        hidden.visible = false;
        // Alpha: the most opaque visible fill
        assert_eq!(run_alpha(SilhouetteMode::Alpha, std::slice::from_ref(&half), None), 0.5);
        assert_eq!(run_alpha(SilhouetteMode::Alpha, &[zero_opacity_paint(), half.clone()], None), 0.5);
        assert_eq!(run_alpha(SilhouetteMode::Alpha, &[hidden.clone()], None), 0.0);
        assert_eq!(run_alpha(SilhouetteMode::Alpha, &[], None), 0.0);
        // Opaque and BackdropBlur: every glyph at full coverage
        for mode in [SilhouetteMode::Opaque, SilhouetteMode::BackdropBlur] {
            assert_eq!(run_alpha(mode, &[], None), 1.0);
            assert_eq!(run_alpha(mode, &[zero_opacity_paint()], None), 1.0);
            assert_eq!(run_alpha(mode, std::slice::from_ref(&half), None), 1.0);
        }
    }

    #[test]
    fn paint_free_text_adds_nothing_to_a_knockout() {
        let mode = SilhouetteMode::Knockout;
        let half = {
            let mut p = Paint::solid(Color::BLACK);
            p.opacity = 0.5;
            p
        };
        let mut hidden = Paint::solid(Color::BLACK);
        hidden.visible = false;
        // any fill that paints something covers the glyph fully, whatever its alpha
        assert_eq!(run_alpha(mode, std::slice::from_ref(&half), None), 1.0);
        assert_eq!(run_alpha(mode, &[zero_opacity_paint(), half], None), 1.0);
        // no fill, a hidden fill or a 0% fill paints nothing
        assert_eq!(run_alpha(mode, &[], None), 0.0);
        assert_eq!(run_alpha(mode, &[hidden], None), 0.0);
        assert_eq!(run_alpha(mode, &[zero_opacity_paint()], None), 0.0);
        assert_eq!(run_alpha(mode, &[Paint::solid(Color::TRANSPARENT)], None), 0.0);
    }

    #[test]
    fn stroked_text_keeps_its_glyph_footprint_in_a_knockout() {
        let mode = SilhouetteMode::Knockout;
        assert_eq!(run_alpha(mode, &[], Some(Paint::solid(Color::BLACK))), 1.0);
        assert_eq!(run_alpha(mode, &[zero_opacity_paint()], Some(Paint::solid(Color::BLACK))), 1.0);
        // a stroke that paints nothing does not count
        assert_eq!(run_alpha(mode, &[], Some(zero_opacity_paint())), 0.0);
    }

    #[test]
    fn background_blur_surface_rule_by_node_kind() {
        let at = |n: &Node| n.background_blur_has_surface();
        // a node that is its own silhouette blurs only while it paints something
        assert!(at(&Node::rectangle(NodeId(1), rect())));
        assert!(!at(&bare(Node::rectangle(NodeId(1), rect()))));
        assert!(!at(&bare(Node::frame(NodeId(1), rect()))));
        assert!(!at(&with_child(bare(Node::frame(NodeId(1), rect())))));
        let mut hidden = Node::rectangle(NodeId(1), rect());
        hidden.fills[0].visible = false;
        assert!(!at(&hidden));
        let mut zero_opacity = bare(Node::rectangle(NodeId(1), rect()));
        zero_opacity.fills = vec![Paint { opacity: 0.0, ..Paint::solid(Color::WHITE) }];
        assert!(at(&zero_opacity));
        let mut stroke_only = stroked(bare(Node::rectangle(NodeId(1), rect())));
        assert!(at(&stroke_only));
        stroke_only.stroke.weight = 0.0;
        assert!(!at(&stroke_only));
        stroke_only.stroke.side_weights = Some([1.0, 0.0, 0.0, 0.0]);
        assert!(at(&stroke_only));

        // a node without a shape of its own takes its silhouette from its children (group) or its
        // lowered body (board item): its own paint says nothing, the effect stays
        assert!(at(&Node::new(NodeId(1), NodeKind::Group)));
        assert!(at(&with_child(Node::new(NodeId(1), NodeKind::Group))));
        assert!(at(&Node::new(NodeId(1), NodeKind::Sticky(Default::default()))));
    }

    #[test]
    fn background_blur_surface_on_text_counts_a_visible_run_fill() {
        let text = |base: Vec<Paint>, run: Option<Vec<Paint>>| {
            let style = TextStyle { fills: base, ..Default::default() };
            let mut n = Node::text(NodeId(1), rect(), "ab", style.clone());
            let NodeKind::Text(tp) = &mut n.kind else { unreachable!() };
            tp.runs = run.map(|fills| TextRun { start: 0, end: 1, style: TextStyle { fills, ..style } }).into_iter().collect();
            n
        };
        let ink = vec![Paint::solid(Color::WHITE)];
        let hidden_ink = vec![Paint { visible: false, ..Paint::solid(Color::WHITE) }];
        assert!(text(ink.clone(), None).background_blur_has_surface());
        assert!(!text(Vec::new(), None).background_blur_has_surface());
        assert!(!text(Vec::new(), Some(hidden_ink)).background_blur_has_surface());
        assert!(text(Vec::new(), Some(ink)).background_blur_has_surface());
    }
}
