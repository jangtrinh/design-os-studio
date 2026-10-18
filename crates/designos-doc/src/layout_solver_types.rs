//! Input and output types of the one auto-layout solver (see `layout_solver.rs`).

use crate::{AlignSelf, LayoutChild, LayoutDirection, Node, Rect, Sizing, Vec2};

/// Optional min/max size of a layout child, in px (`x` = width, `y` = height).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SizeLimits {
    pub min_w: Option<f32>,
    pub max_w: Option<f32>,
    pub min_h: Option<f32>,
    pub max_h: Option<f32>,
}

fn clamp_opt(v: f32, min: Option<f32>, max: Option<f32>) -> f32 {
    let v = min.map_or(v, |lo| v.max(lo));
    max.map_or(v, |hi| v.min(hi))
}

impl SizeLimits {
    pub fn clamp_w(&self, w: f32) -> f32 {
        clamp_opt(w, self.min_w, self.max_w)
    }

    pub fn clamp_h(&self, h: f32) -> f32 {
        clamp_opt(h, self.min_h, self.max_h)
    }

    pub fn clamp(&self, size: Vec2) -> Vec2 {
        Vec2::new(self.clamp_w(size.x), self.clamp_h(size.y))
    }
}

impl From<&LayoutChild> for SizeLimits {
    fn from(lc: &LayoutChild) -> Self {
        Self { min_w: lc.min_w, max_w: lc.max_w, min_h: lc.min_h, max_h: lc.max_h }
    }
}

/// Where a child asks to sit in a `Grid` frame; ignored by flow frames.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridCell {
    /// Explicit zero-based column, otherwise auto-placed.
    pub column: Option<u16>,
    /// Explicit zero-based row, otherwise auto-placed.
    pub row: Option<u16>,
    pub col_span: u16,
    pub row_span: u16,
}

impl Default for GridCell {
    fn default() -> Self {
        Self { column: None, row: None, col_span: 1, row_span: 1 }
    }
}

/// One participating child, as seen by the solver.
#[derive(Clone, Copy, Debug, Default)]
pub struct ChildIn {
    pub size: Vec2,
    /// Stretches along the primary axis (Fill or grow > 0).
    pub main_fill: bool,
    /// Stretches along the counter axis.
    pub cross_fill: bool,
    pub grow: f32,
    pub align_self: AlignSelf,
    pub limits: SizeLimits,
    pub cell: GridCell,
    /// Per-child margins `[top, right, bottom, left]`.
    pub margin: [f32; 4],
}

impl ChildIn {
    /// Reads a child's layout properties. `horiz` is the flow's primary axis (grids count as horizontal).
    pub fn from_layout_child(size: Vec2, lc: &LayoutChild, horiz: bool) -> Self {
        let (main_s, cross_s) = if horiz { (lc.horizontal, lc.vertical) } else { (lc.vertical, lc.horizontal) };
        Self {
            size,
            main_fill: main_s == Sizing::Fill || lc.grow > 0.0,
            cross_fill: cross_s == Sizing::Fill || lc.align_self == AlignSelf::Stretch,
            grow: lc.grow,
            align_self: lc.align_self,
            limits: SizeLimits::from(lc),
            cell: GridCell { column: lc.grid_column, row: lc.grid_row, col_span: lc.col_span.max(1), row_span: lc.row_span.max(1) },
            margin: lc.margin,
        }
    }

    /// Whether the child stretches along the counter axis: counter-axis Fill, or an `AlignSelf::Stretch` override.
    pub fn stretches_cross(&self) -> bool {
        self.cross_fill || self.align_self == AlignSelf::Stretch
    }
}

impl LayoutChild {
    /// The axis a flow `parent` stretches its children along: `Some(true)` for width (vertical flow),
    /// `Some(false)` for height (horizontal and grid flows), `None` when `parent` is not an auto-layout frame.
    pub fn counter_axis_is_horizontal(parent: &Node) -> Option<bool> {
        let al = parent.kind.frame_props()?.auto_layout.as_ref()?;
        Some(al.direction == LayoutDirection::Vertical)
    }

    /// Sets the sizing of one axis (`horizontal` = width). Stretching along the parent's counter axis
    /// (`counter_horizontal`, see [`Self::counter_axis_is_horizontal`]) is Fill on that axis, so any other
    /// sizing there also ends an `AlignSelf::Stretch` override, which would otherwise keep stretching a
    /// child just made Fixed or Hug.
    pub fn set_axis_sizing(&mut self, horizontal: bool, sizing: Sizing, counter_horizontal: Option<bool>) {
        if horizontal {
            self.horizontal = sizing;
        } else {
            self.vertical = sizing;
        }
        if sizing != Sizing::Fill && counter_horizontal == Some(horizontal) && self.align_self == AlignSelf::Stretch {
            self.align_self = AlignSelf::Auto;
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutOut {
    pub frame: Vec2,
    /// Child rectangles in frame-local coordinates, same order as the input.
    pub rects: Vec<Rect>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AutoLayout, Document, FrameProps, LayoutAlign, NodeId, NodeKind};

    #[test]
    fn limits_apply_the_minimum_first_so_the_maximum_wins_a_conflict() {
        let l = SizeLimits { min_w: Some(50.0), max_w: Some(40.0), min_h: None, max_h: Some(10.0) };
        assert_eq!(l.clamp(Vec2::new(5.0, 30.0)), Vec2::new(40.0, 10.0));
        assert_eq!(SizeLimits::default().clamp(Vec2::new(7.0, 9.0)), Vec2::new(7.0, 9.0));
    }

    #[test]
    fn child_in_reads_the_primary_axis_and_grid_cell_from_the_layout_child() {
        let lc = LayoutChild { horizontal: Sizing::Fill, grid_column: Some(2), col_span: 0, row_span: 3, max_h: Some(9.0), ..Default::default() };
        let horiz = ChildIn::from_layout_child(Vec2::new(4.0, 5.0), &lc, true);
        assert!(horiz.main_fill && !horiz.cross_fill);
        assert_eq!(horiz.cell, GridCell { column: Some(2), row: None, col_span: 1, row_span: 3 });
        assert_eq!(horiz.limits.max_h, Some(9.0));
        let vert = ChildIn::from_layout_child(Vec2::new(4.0, 5.0), &lc, false);
        assert!(!vert.main_fill && vert.cross_fill);
        let grown = ChildIn::from_layout_child(Vec2::ZERO, &LayoutChild { grow: 2.0, ..Default::default() }, false);
        assert!(grown.main_fill);
        assert_eq!(grown.grow, 2.0);
    }

    #[test]
    fn leaving_fill_on_the_counter_axis_clears_a_stretch_override_and_nothing_else_does() {
        let stretched = LayoutChild { vertical: Sizing::Fill, align_self: AlignSelf::Stretch, ..Default::default() };
        // horizontal and grid flows stretch along height, a vertical flow along width
        for (horizontal, counter) in [(false, Some(false)), (true, Some(true))] {
            for s in [Sizing::Fixed, Sizing::Hug] {
                let mut lc = stretched;
                lc.set_axis_sizing(horizontal, s, counter);
                assert_eq!(lc.align_self, AlignSelf::Auto, "{s:?} on the counter axis");
            }
            let mut lc = stretched;
            lc.set_axis_sizing(horizontal, Sizing::Fill, counter);
            assert_eq!(lc.align_self, AlignSelf::Stretch, "Fill keeps the override");
        }
        // the main axis, and a parent that is not a flow, leave the override alone
        let mut lc = stretched;
        lc.set_axis_sizing(true, Sizing::Fixed, Some(false));
        assert_eq!((lc.horizontal, lc.align_self), (Sizing::Fixed, AlignSelf::Stretch));
        lc.set_axis_sizing(false, Sizing::Fixed, None);
        assert_eq!((lc.vertical, lc.align_self), (Sizing::Fixed, AlignSelf::Stretch));
        // other overrides are not stretches
        let mut centered = LayoutChild { align_self: AlignSelf::Center, ..Default::default() };
        centered.set_axis_sizing(false, Sizing::Fixed, Some(false));
        assert_eq!(centered.align_self, AlignSelf::Center);
    }

    #[test]
    fn counter_axis_follows_the_parent_flow_direction() {
        let parent = |direction: Option<LayoutDirection>| {
            let mut n = Node::frame(NodeId(1), Rect::new(0.0, 0.0, 10.0, 10.0));
            n.kind =
                NodeKind::Frame(FrameProps { auto_layout: direction.map(|direction| AutoLayout { direction, ..Default::default() }), ..Default::default() });
            n
        };
        assert_eq!(LayoutChild::counter_axis_is_horizontal(&parent(Some(LayoutDirection::Horizontal))), Some(false));
        assert_eq!(LayoutChild::counter_axis_is_horizontal(&parent(Some(LayoutDirection::Grid))), Some(false));
        assert_eq!(LayoutChild::counter_axis_is_horizontal(&parent(Some(LayoutDirection::Vertical))), Some(true));
        assert_eq!(LayoutChild::counter_axis_is_horizontal(&parent(None)), None);
        assert_eq!(LayoutChild::counter_axis_is_horizontal(&Node::rectangle(NodeId(2), Rect::new(0.0, 0.0, 1.0, 1.0))), None);
    }

    /// A fixed-width horizontal frame hugging its height, holding one rectangle per `(w, h, align_self)`.
    fn hugging_row(kids: &[(f32, f32, AlignSelf)]) -> (Document, NodeId, Vec<NodeId>) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 1.0));
        let al = AutoLayout {
            direction: LayoutDirection::Horizontal,
            primary_align: LayoutAlign::Start,
            primary_sizing: Sizing::Fixed,
            counter_sizing: Sizing::Hug,
            ..Default::default()
        };
        frame.kind = NodeKind::Frame(FrameProps { auto_layout: Some(al), ..Default::default() });
        let frame = d.insert(page, frame).unwrap();
        let ids = kids
            .iter()
            .map(|&(w, h, align_self)| {
                let id = d.alloc_id();
                let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, w, h));
                n.layout_child.align_self = align_self;
                d.insert(frame, n).unwrap()
            })
            .collect();
        (d, frame, ids)
    }

    fn heights(d: &Document, frame: NodeId, kids: &[NodeId]) -> (f32, Vec<f32>) {
        (d.get(frame).unwrap().size.y, kids.iter().map(|id| d.get(*id).unwrap().size.y).collect())
    }

    #[test]
    fn a_lone_stretch_child_keeps_its_height_in_a_hugging_frame() {
        let (mut d, frame, kids) = hugging_row(&[(30.0, 20.0, AlignSelf::Stretch)]);
        d.relayout_subtree(frame);
        assert_eq!(heights(&d, frame, &kids), (20.0, vec![20.0]));
    }

    #[test]
    fn stretch_children_all_take_the_tallest_natural_height() {
        let (mut d, frame, kids) = hugging_row(&[(30.0, 20.0, AlignSelf::Stretch), (30.0, 100.0, AlignSelf::Stretch)]);
        d.relayout_subtree(frame);
        assert_eq!(heights(&d, frame, &kids), (100.0, vec![100.0, 100.0]));
    }

    #[test]
    fn a_plain_sibling_sizes_the_line_and_the_frame_shrinks_back_with_it() {
        let (mut d, frame, kids) = hugging_row(&[(30.0, 100.0, AlignSelf::Auto), (30.0, 20.0, AlignSelf::Stretch)]);
        d.relayout_subtree(frame);
        assert_eq!(heights(&d, frame, &kids), (100.0, vec![100.0, 100.0]));

        // the stretch child now stores 100; shrinking the plain sibling must still pull the frame down
        d.get_mut(kids[0]).unwrap().size.y = 50.0;
        d.relayout_subtree(frame);
        assert_eq!(heights(&d, frame, &kids), (50.0, vec![50.0, 50.0]));
    }
}
