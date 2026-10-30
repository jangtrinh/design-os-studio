//! Which geometry an effect silhouette (drop shadow, inner shadow, background blur, glass,
//! noise, texture) is drawn from. The CPU renderer (`designos-render`) and the GPU compiler
//! (`designos-gpu`) both call [`silhouette_source`], so the two cannot disagree on the rule.

use crate::{Node, NodeKind};

/// What a silhouette is drawn for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SilhouetteMode {
    /// The shape's own alpha: the source of a shadow, glass or inner shadow.
    Alpha,
    /// Fill-agnostic footprint at full alpha: noise, texture grain, glass and blur masks.
    Opaque,
    /// The mask that removes a drop shadow behind the node (show behind off).
    Knockout,
}

impl SilhouetteMode {
    /// Whether paint alpha is ignored (every covered pixel counts fully).
    pub fn ignores_alpha(self) -> bool {
        !matches!(self, SilhouetteMode::Alpha)
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
/// it (lines are not fillable). In `Opaque` and `Knockout` mode a fillable shape counts even
/// without a visible fill, except that a `Knockout` of a fill-less, stroke-less fillable node
/// that has children follows [`FILLLESS_KNOCKOUT`] instead of covering its rectangle.
///
/// When the node clips its content, the caller clips the children by the node's shape in both
/// the silhouette and the knockout.
pub fn silhouette_source(n: &Node, shape_fillable: Option<bool>, mode: SilhouetteMode) -> SilhouetteSource {
    if matches!(n.kind, NodeKind::Text(_)) {
        return SilhouetteSource::Text;
    }
    let has_children = !n.children.is_empty() && !matches!(n.kind, NodeKind::BooleanOp(_));
    if let Some(fillable) = shape_fillable {
        if (fillable && has_visible_fills(n)) || has_visible_strokes(n) {
            return SilhouetteSource::OwnShape;
        }
        if fillable && mode == SilhouetteMode::Knockout && has_children {
            return match FILLLESS_KNOCKOUT {
                FilllessKnockout::None => SilhouetteSource::Nothing,
                FilllessKnockout::ChildrenOpaque => SilhouetteSource::ChildrenOpaque,
            };
        }
        if fillable && mode == SilhouetteMode::Opaque {
            return SilhouetteSource::OwnShape;
        }
    }
    if mode == SilhouetteMode::Knockout && has_children {
        return SilhouetteSource::ChildrenOpaque;
    }
    if has_children {
        SilhouetteSource::Children
    } else {
        SilhouetteSource::Nothing
    }
}

fn has_visible_fills(n: &Node) -> bool {
    n.fills.iter().any(|p| p.visible && p.silhouette_alpha() > 0.0)
}

/// A stroke exists with a weight or with per-side weights.
fn has_visible_strokes(n: &Node) -> bool {
    (n.stroke.weight > 0.0 || n.stroke.side_weights.is_some()) && n.stroke.paints.iter().any(|p| p.visible)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BooleanOp, Color, NodeId, Paint, Rect, TextStyle};

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
        // without children there is nothing to follow
        assert_eq!(knock(&bare(Node::frame(NodeId(1), rect()))), SilhouetteSource::Nothing);
    }

    #[test]
    fn knockout_of_filled_or_stroked_node_is_its_own_shape() {
        assert_eq!(knock(&with_child(Node::frame(NodeId(1), rect()))), SilhouetteSource::OwnShape);
        assert_eq!(knock(&with_child(stroked(bare(Node::frame(NodeId(1), rect()))))), SilhouetteSource::OwnShape);
    }

    #[test]
    fn knockout_of_group_and_text_uses_opaque_children_for_groups() {
        let g = with_child(Node::new(NodeId(1), NodeKind::Group));
        assert_eq!(silhouette_source(&g, None, SilhouetteMode::Knockout), SilhouetteSource::ChildrenOpaque);
        let t = Node::text(NodeId(1), rect(), "Hi", TextStyle::default());
        assert_eq!(silhouette_source(&t, None, SilhouetteMode::Knockout), SilhouetteSource::Text);
    }
}
