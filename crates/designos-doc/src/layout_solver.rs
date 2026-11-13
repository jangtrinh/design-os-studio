//! The one auto-layout solver.
//!
//! [`compute`] is a pure function from an [`AutoLayout`] frame and its flow children to child
//! rectangles. Every caller adapts its own storage around it and nothing else solves layout:
//!
//! - the editor (`designos-app` `layout.rs`) reads sizes through `resolve_node` and writes through
//!   an `Edit` session so every touched node is undoable;
//! - [`Document::relayout_subtree`](crate::Document::relayout_subtree) writes straight into the
//!   document (specs, MCP, suggest auto layout, variable modes, component swaps).
//!
//! Flow frames (`Horizontal` / `Vertical`) live here; the 2D `Grid` solver is in
//! `layout_solver_grid.rs` and is reached through [`compute`] too.

use crate::{AlignSelf, AutoLayout, CounterAlignContent, LayoutAlign, LayoutDirection, NodeId, Rect, Sizing, Vec2};

pub use crate::layout_solver_grid::{
    compute_grid, grid_column_widths, grid_packs_auto_rows, grid_row_gap, grid_row_heights, placed_grid_rows, resolve_cell_align,
};
pub use crate::layout_solver_grid_placement::{place_children, Placed};
pub use crate::layout_solver_types::{ChildIn, GridCell, LayoutOut, SizeLimits};

struct Axes {
    horiz: bool,
}

impl Axes {
    fn main(&self, v: Vec2) -> f32 {
        if self.horiz {
            v.x
        } else {
            v.y
        }
    }
    fn cross(&self, v: Vec2) -> f32 {
        if self.horiz {
            v.y
        } else {
            v.x
        }
    }
    fn vec(&self, main: f32, cross: f32) -> Vec2 {
        if self.horiz {
            Vec2::new(main, cross)
        } else {
            Vec2::new(cross, main)
        }
    }
    fn clamp_main(&self, l: &SizeLimits, v: f32) -> f32 {
        if self.horiz {
            l.clamp_w(v)
        } else {
            l.clamp_h(v)
        }
    }
    fn clamp_cross(&self, l: &SizeLimits, v: f32) -> f32 {
        if self.horiz {
            l.clamp_h(v)
        } else {
            l.clamp_w(v)
        }
    }
    fn margin_main_start(&self, m: &[f32; 4]) -> f32 {
        if self.horiz {
            m[3]
        } else {
            m[0]
        }
    }
    fn margin_main_end(&self, m: &[f32; 4]) -> f32 {
        if self.horiz {
            m[1]
        } else {
            m[2]
        }
    }
    fn margin_main(&self, m: &[f32; 4]) -> f32 {
        self.margin_main_start(m) + self.margin_main_end(m)
    }
    fn margin_cross_start(&self, m: &[f32; 4]) -> f32 {
        if self.horiz {
            m[0]
        } else {
            m[3]
        }
    }
    fn margin_cross_end(&self, m: &[f32; 4]) -> f32 {
        if self.horiz {
            m[2]
        } else {
            m[1]
        }
    }
    fn margin_cross(&self, m: &[f32; 4]) -> f32 {
        self.margin_cross_start(m) + self.margin_cross_end(m)
    }
}

/// Solves an auto-layout frame accounting for its [`crate::Stroke`]: in version 0
/// (`layout_version == 0`) with `stroke_included == true` (`bordersTakeSpace`), inward stroke
/// widths add to padding; in version 1 and up (`layout_version >= 1`), stroke inset is ignored
/// (border inside the box).
pub fn compute_with_stroke(al: &AutoLayout, frame: Vec2, own: SizeLimits, stroke: &crate::Stroke, kids: &[ChildIn]) -> LayoutOut {
    compute(&al.with_stroke(stroke), frame, own, kids)
}

/// First solve, then rewrite wrapping text children's intrinsic height to the lines at the solved
/// width and solve once more. `height_at(id, width)` returns `None` for children that are not
/// wrapping height-resize text.
pub fn compute_then_refit_wrapped(
    al: &AutoLayout,
    frame: Vec2,
    own: SizeLimits,
    stroke: &crate::Stroke,
    ids: &[NodeId],
    kids: &mut [ChildIn],
    mut height_at: impl FnMut(NodeId, f32) -> Option<f32>,
) -> LayoutOut {
    let out = compute_with_stroke(al, frame, own, stroke, kids);
    let mut changed = false;
    for ((id, kid), r) in ids.iter().zip(kids.iter_mut()).zip(&out.rects) {
        let Some(h) = height_at(*id, r.w) else { continue };
        if (kid.size.y - h).abs() > 1e-3 {
            kid.size.y = h;
            changed = true;
        }
    }
    if changed {
        compute_with_stroke(al, frame, own, stroke, kids)
    } else {
        out
    }
}

/// The axes of an auto-layout frame that [`compute`] sizes from its content, as `(width, height)`.
/// A wrapping flow frame keeps its primary axis: the content is cut into lines to fit it.
pub fn hugged_axes(al: &AutoLayout) -> (bool, bool) {
    let primary = al.primary_sizing == Sizing::Hug;
    let counter = al.counter_sizing == Sizing::Hug;
    match al.direction {
        LayoutDirection::Horizontal => (primary && !al.wrap, counter),
        LayoutDirection::Vertical => (counter, primary && !al.wrap),
        LayoutDirection::Grid => (primary, counter),
    }
}

/// Solves an auto-layout frame. `frame` is the current frame size (used for fixed axes) and `own`
/// the frame's own min/max: they clamp the frame before any child is aligned, so a hugging frame
/// raised by its minimum centers its children in the raised box. Padding order is top, right,
/// bottom, left.
pub fn compute(al: &AutoLayout, frame: Vec2, own: SizeLimits, kids: &[ChildIn]) -> LayoutOut {
    if al.direction == LayoutDirection::Grid {
        return compute_grid(al, frame, own, kids);
    }
    let ax = Axes { horiz: al.direction == LayoutDirection::Horizontal };
    let [pt, pr, pb, pl] = al.padding;
    let (pm_start, pm_end, pc_start, pc_end) = if ax.horiz { (pl, pr, pt, pb) } else { (pt, pb, pl, pr) };
    let pm = pm_start + pm_end;
    let pc = pc_start + pc_end;
    let (hug_width, hug_height) = hugged_axes(al);
    let (hug_main, hug_cross) = if ax.horiz { (hug_width, hug_height) } else { (hug_height, hug_width) };
    // a Fixed axis takes the clamped size; a hugging axis is clamped again once its content is known
    let mut frame_main = ax.clamp_main(&own, ax.main(frame));
    let mut frame_cross = ax.clamp_cross(&own, ax.cross(frame));
    // natural extents with the child's own min/max applied
    let nat_main = |k: &ChildIn| ax.clamp_main(&k.limits, ax.main(k.size));
    let nat_cross = |k: &ChildIn| ax.clamp_cross(&k.limits, ax.cross(k.size));
    let outer_main = |k: &ChildIn| nat_main(k) + ax.margin_main(&k.margin);
    let outer_cross = |k: &ChildIn| nat_cross(k) + ax.margin_cross(&k.margin);

    if kids.is_empty() {
        if hug_main {
            frame_main = ax.clamp_main(&own, pm);
        }
        if hug_cross {
            frame_cross = ax.clamp_cross(&own, pc);
        }
        return LayoutOut { frame: ax.vec(frame_main, frame_cross), rects: vec![] };
    }

    // Split into lines.
    let mut lines: Vec<Vec<usize>> = Vec::new();
    if al.wrap {
        let avail = (frame_main - pm).max(0.0);
        let mut cur: Vec<usize> = Vec::new();
        let mut used = 0.0;
        for (i, k) in kids.iter().enumerate() {
            let m = outer_main(k);
            let add = if cur.is_empty() { m } else { al.item_spacing + m };
            if !cur.is_empty() && used + add > avail + 1e-3 {
                lines.push(std::mem::take(&mut cur));
                used = 0.0;
                cur.push(i);
                used += m;
            } else {
                cur.push(i);
                used += add;
            }
        }
        lines.push(cur);
    } else {
        lines.push((0..kids.len()).collect());
    }

    // Hug along the primary axis (single line only).
    if hug_main {
        let total: f32 = kids.iter().map(outer_main).sum::<f32>() + al.item_spacing * (kids.len() as f32 - 1.0);
        frame_main = ax.clamp_main(&own, total + pm);
    }
    let main_avail = (frame_main - pm).max(0.0);

    // Cross extent of each line. A plain child needs its outer extent (size plus margins). A child that stretches the
    // counter axis (Fill or `align_self` stretch) stores the stretched size, so counting it would stop the line from ever
    // shrinking again: with cross margins it only holds the line open by its minimum size plus margins, wrapped or not.
    // Without margins it counts for nothing while a plain child sizes the line, and keeps its natural margin box as the line
    // size when none does (wrapped or not), so a hugging frame whose children all stretch does not fold to its padding.
    // Its cross minimum floors the line either way, so a stretched child never overflows the frame.
    let mut line_cross: Vec<f32> = lines
        .iter()
        .map(|l| {
            let (mut plain, mut natural, mut floor) = (None::<f32>, 0.0f32, 0.0f32);
            for k in l.iter().map(|i| &kids[*i]) {
                let margins = ax.margin_cross(&k.margin);
                if !k.stretches_cross() {
                    plain = Some(plain.map_or(outer_cross(k), |p| p.max(outer_cross(k))));
                } else {
                    floor = floor.max(ax.clamp_cross(&k.limits, 0.0) + margins);
                    if margins <= 0.0 {
                        natural = natural.max(outer_cross(k));
                    }
                }
            }
            plain.unwrap_or(natural).max(floor)
        })
        .collect();
    // Does the frame's cross extent exceed what its lines need (a Fixed frame, or a hugging one raised by its minimum)?
    let mut cross_has_slack = !hug_cross;
    if !al.wrap {
        if hug_cross {
            frame_cross = ax.clamp_cross(&own, line_cross[0] + pc);
            cross_has_slack = frame_cross - pc > line_cross[0] + 1e-3;
        }
        if cross_has_slack {
            line_cross[0] = (frame_cross - pc).max(0.0);
        }
    } else {
        let total: f32 = line_cross.iter().sum::<f32>() + al.counter_spacing * (lines.len() as f32 - 1.0);
        if hug_cross {
            frame_cross = ax.clamp_cross(&own, total + pc);
            cross_has_slack = frame_cross - pc > total + 1e-3;
        }
    }

    // Wrapped lines spread edge to edge (CSS align-content: space-between); the counter spacing stays the minimum gap.
    let counter_gap = if al.wrap && al.counter_align_content == CounterAlignContent::SpaceBetween && lines.len() > 1 && cross_has_slack {
        ((frame_cross - pc - line_cross.iter().sum::<f32>()) / (lines.len() as f32 - 1.0)).max(al.counter_spacing)
    } else {
        al.counter_spacing
    };
    let mut rects = vec![Rect::default(); kids.len()];
    let mut cross_cursor = pc_start;
    if al.wrap && lines.len() > 1 && cross_has_slack && al.counter_align_content == CounterAlignContent::Auto && !kids.iter().all(|k| k.stretches_cross()) {
        let used: f32 = line_cross.iter().sum::<f32>() + al.counter_spacing * (lines.len() as f32 - 1.0);
        let slack = (frame_cross - pc - used).max(0.0);
        cross_cursor += match al.counter_align {
            LayoutAlign::Center => slack / 2.0,
            LayoutAlign::End => slack,
            _ => 0.0,
        };
    }
    for (li, line) in lines.iter().enumerate() {
        let k = line.len() as f32;
        // Main sizes (distribute fill children by grow weight with a CSS flex freeze loop for min/max clamping).
        let mut mains: Vec<f32> = line.iter().map(|i| nat_main(&kids[*i])).collect();
        if !al.wrap && !hug_main {
            let mut frozen: Vec<bool> = line.iter().map(|i| !kids[*i].main_fill).collect();
            let fixed: f32 = line.iter().zip(&frozen).filter(|(_, &f)| f).map(|(i, _)| nat_main(&kids[*i])).sum();
            let total_margins: f32 = line.iter().map(|i| ax.margin_main(&kids[*i].margin)).sum();
            // The auto spacing modes place the children by free space alone: their stored item spacing is not reserved.
            let reserved_spacing = if is_auto_spacing(al.primary_align) { 0.0 } else { al.item_spacing };
            let mut rem_for_fill = (main_avail - reserved_spacing * (k - 1.0) - fixed - total_margins).max(0.0);
            loop {
                let weight: f32 = line.iter().zip(&frozen).filter(|(_, &f)| !f).map(|(i, _)| fill_weight(&kids[*i])).sum();
                if weight <= 0.0 {
                    break;
                }
                let mut total_violation = 0.0f32;
                let mut any_violation = false;
                for (j, i) in line.iter().enumerate() {
                    if frozen[j] {
                        continue;
                    }
                    let kid = &kids[*i];
                    let raw = rem_for_fill * fill_weight(kid) / weight;
                    let clamped = ax.clamp_main(&kid.limits, raw);
                    let diff = clamped - raw;
                    if diff.abs() > 1e-4 {
                        any_violation = true;
                        total_violation += diff;
                    }
                }
                if !any_violation {
                    for (j, i) in line.iter().enumerate() {
                        if !frozen[j] {
                            let kid = &kids[*i];
                            mains[j] = ax.clamp_main(&kid.limits, rem_for_fill * fill_weight(kid) / weight);
                        }
                    }
                    break;
                }
                let mut newly_frozen = false;
                for (j, i) in line.iter().enumerate() {
                    if frozen[j] {
                        continue;
                    }
                    let kid = &kids[*i];
                    let raw = rem_for_fill * fill_weight(kid) / weight;
                    let clamped = ax.clamp_main(&kid.limits, raw);
                    let diff = clamped - raw;
                    let freeze_this = if total_violation > 1e-4 {
                        diff > 1e-4
                    } else if total_violation < -1e-4 {
                        diff < -1e-4
                    } else {
                        diff.abs() > 1e-4
                    };
                    if freeze_this {
                        frozen[j] = true;
                        mains[j] = clamped;
                        rem_for_fill = (rem_for_fill - clamped).max(0.0);
                        newly_frozen = true;
                    }
                }
                if !newly_frozen {
                    for (j, i) in line.iter().enumerate() {
                        if !frozen[j] {
                            let kid = &kids[*i];
                            mains[j] = ax.clamp_main(&kid.limits, rem_for_fill * fill_weight(kid) / weight);
                        }
                    }
                    break;
                }
            }
        }
        let sum_main: f32 = line.iter().enumerate().map(|(j, i)| mains[j] + ax.margin_main(&kids[*i].margin)).sum();
        // For Space* distribution modes, free space ignores the fixed gap and negative free space packs from the start
        // (space-between in version 0 excepted, below); Center and End keep the signed offset so overflowing content stays
        // centered or end-anchored.
        let free = main_avail - sum_main;
        // Layout version 0 is Figma's legacy engine: space-between centres a frame's only child (unless it fills the main
        // axis) and lets overflowing children overlap. Version 1 and up start a lone child at the start and stop at a zero gap.
        let legacy = al.layout_version == 0;
        let (start, gap) = match al.primary_align {
            LayoutAlign::SpaceBetween if legacy && !al.wrap && kids.len() == 1 && !kids[0].main_fill => (free / 2.0, al.item_spacing),
            // Wrapping lines keep their stored item spacing as the floor of the gap in every version: they were broken with it.
            LayoutAlign::SpaceBetween if k > 1.0 && al.wrap => (0.0, (free / (k - 1.0)).max(al.item_spacing.min(0.0))),
            LayoutAlign::SpaceBetween if k > 1.0 && legacy => (0.0, free / (k - 1.0)),
            LayoutAlign::SpaceBetween if k > 1.0 => (0.0, (free / (k - 1.0)).max(0.0)),
            LayoutAlign::SpaceAround if free > 0.0 => (free / k / 2.0, free / k),
            LayoutAlign::SpaceEvenly if free > 0.0 => (free / (k + 1.0), free / (k + 1.0)),
            LayoutAlign::SpaceAround | LayoutAlign::SpaceEvenly => (0.0, 0.0),
            LayoutAlign::Center => ((main_avail - sum_main - al.item_spacing * (k - 1.0)) / 2.0, al.item_spacing),
            LayoutAlign::End => (main_avail - sum_main - al.item_spacing * (k - 1.0), al.item_spacing),
            _ => (0.0, al.item_spacing),
        };
        let mut cursor = pm_start + start;
        for (j, i) in line.iter().enumerate() {
            let kid = &kids[*i];
            let m = mains[j];
            let m_start = ax.margin_main_start(&kid.margin);
            let m_end = ax.margin_main_end(&kid.margin);
            let c_start = ax.margin_cross_start(&kid.margin);
            let c_margin = ax.margin_cross(&kid.margin);
            let c = if kid.stretches_cross() { ax.clamp_cross(&kid.limits, (line_cross[li] - c_margin).max(0.0)) } else { nat_cross(kid) };
            let outer_c = c + c_margin;
            let off = match kid.align_self {
                AlignSelf::Start => 0.0,
                AlignSelf::Center => (line_cross[li] - outer_c) / 2.0,
                AlignSelf::End => line_cross[li] - outer_c,
                // A stretched child fills the line unless a max (or min) size clamps it; the clamped remainder is placed by the
                // frame's counter_align like any other child (an unclamped one has no slack, so its offset is 0).
                // Baseline is stored (and round-trips through .fig) but baselines are not aligned yet:
                // it behaves like Auto and inherits the frame's counter_align.
                AlignSelf::Auto | AlignSelf::Stretch | AlignSelf::Baseline => match al.counter_align {
                    LayoutAlign::Center => (line_cross[li] - outer_c) / 2.0,
                    LayoutAlign::End => line_cross[li] - outer_c,
                    _ => 0.0,
                },
            } + c_start;
            let pos = ax.vec(cursor + m_start, cross_cursor + off);
            let size = ax.vec(m, c);
            rects[*i] = Rect::new(pos.x, pos.y, size.x, size.y);
            cursor += m_start + m + m_end + gap;
        }
        cross_cursor += line_cross[li] + counter_gap;
    }
    LayoutOut { frame: ax.vec(frame_main, frame_cross), rects }
}

/// Space-between, space-around and space-evenly: the stored item spacing is not used, free space sets every gap.
fn is_auto_spacing(align: LayoutAlign) -> bool {
    matches!(align, LayoutAlign::SpaceBetween | LayoutAlign::SpaceAround | LayoutAlign::SpaceEvenly)
}

fn fill_weight(k: &ChildIn) -> f32 {
    if k.grow > 0.0 {
        k.grow
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Document, FrameProps, LayoutChild, Node, NodeId, NodeKind};

    /// A row (wrapping or not) hugging its height: `A` is a fixed 30 x `a_height` child, `B` a 20 x 10 child that fills the
    /// row's height and carries 30 of margin above and below. Returns the document with the row's, `A`'s and `B`'s ids.
    fn row_with_cross_fill_margins(a_height: f32, wrap: bool) -> (Document, NodeId, NodeId, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let row_id = d.alloc_id();
        let mut row = Node::frame(row_id, Rect::new(0.0, 0.0, 100.0, 1.0));
        row.kind = NodeKind::Frame(FrameProps {
            auto_layout: Some(AutoLayout { direction: LayoutDirection::Horizontal, counter_sizing: Sizing::Hug, wrap, ..Default::default() }),
            ..Default::default()
        });
        let row = d.insert(page, row).unwrap();
        let a_id = d.alloc_id();
        let a = d.insert(row, Node::rectangle(a_id, Rect::new(0.0, 0.0, 30.0, a_height))).unwrap();
        let b_id = d.alloc_id();
        let mut b = Node::rectangle(b_id, Rect::new(0.0, 0.0, 20.0, 10.0));
        b.layout_child = LayoutChild { vertical: Sizing::Fill, margin: [30.0, 0.0, 30.0, 0.0], ..Default::default() };
        let b = d.insert(row, b).unwrap();
        (d, row, a, b)
    }

    #[test]
    fn a_hugging_row_shrinks_back_around_a_cross_fill_child_with_margins() {
        for wrap in [false, true] {
            let (mut d, row, a, b) = row_with_cross_fill_margins(40.0, wrap);
            d.relayout_subtree(row);
            assert_eq!(d.get(row).unwrap().size.y, 60.0, "wrap {wrap}: B's margins need 60, more than A's 40");
            assert_eq!((d.get(b).unwrap().transform.ty, d.get(b).unwrap().size.y), (30.0, 0.0), "wrap {wrap}");

            // A grows to 100: the row follows and B stretches to what its margins leave
            d.get_mut(a).unwrap().size.y = 100.0;
            d.relayout_subtree(row);
            assert_eq!(d.get(row).unwrap().size.y, 100.0, "wrap {wrap}");
            assert_eq!(d.get(b).unwrap().size.y, 40.0, "wrap {wrap}");

            // A shrinks back: the stretched B must not hold the row open at its stretched size
            d.get_mut(a).unwrap().size.y = 40.0;
            d.relayout_subtree(row);
            assert_eq!(d.get(row).unwrap().size.y, 60.0, "wrap {wrap}");
            assert_eq!((d.get(b).unwrap().transform.ty, d.get(b).unwrap().size.y), (30.0, 0.0), "wrap {wrap}");
        }
    }

    #[test]
    fn a_hugging_row_grows_to_the_minimum_height_of_a_cross_fill_child_with_margins() {
        for wrap in [false, true] {
            let (mut d, row, a, b) = row_with_cross_fill_margins(5.0, wrap);
            d.get_mut(b).unwrap().layout_child.min_h = Some(40.0);
            d.get_mut(b).unwrap().layout_child.margin = [10.0, 0.0, 10.0, 0.0];
            d.relayout_subtree(row);
            // 40 minimum + 10 + 10 of margin; B sits at its minimum inside its margin box
            assert_eq!(d.get(row).unwrap().size.y, 60.0, "wrap {wrap}");
            assert_eq!((d.get(b).unwrap().transform.ty, d.get(b).unwrap().size.y), (10.0, 40.0), "wrap {wrap}");
            assert_eq!(d.get(a).unwrap().transform.ty, 0.0, "wrap {wrap}");
        }
    }

    #[test]
    fn wrapped_line_shrinks_back_around_a_margin_free_cross_fill_child_when_plain_sibling_shrinks() {
        let (mut d, row, a, b) = row_with_cross_fill_margins(100.0, true);
        d.get_mut(b).unwrap().layout_child.margin = [0.0; 4];
        d.relayout_subtree(row);
        assert_eq!(d.get(row).unwrap().size.y, 100.0);
        assert_eq!(d.get(b).unwrap().size.y, 100.0);

        d.get_mut(a).unwrap().size.y = 40.0;
        d.relayout_subtree(row);
        assert_eq!(d.get(row).unwrap().size.y, 40.0);
        assert_eq!(d.get(b).unwrap().size.y, 40.0);
    }

    #[test]
    fn lone_cross_fill_child_without_margins_and_with_min_height_raises_hugging_frame() {
        // Issue #530 Repro 1: horizontal frame, counter Hug, width 100, lone child 20x10 with
        // vertical = Fill and min_h = 40, no margin: frame must be (100, 40) and child [0, 0, 20, 40].
        for wrap in [false, true] {
            let mut d = Document::new("t");
            let page = d.add_page("p");
            let row_id = d.alloc_id();
            let mut row = Node::frame(row_id, Rect::new(0.0, 0.0, 100.0, 1.0));
            row.kind = NodeKind::Frame(FrameProps {
                auto_layout: Some(AutoLayout { direction: LayoutDirection::Horizontal, counter_sizing: Sizing::Hug, wrap, ..Default::default() }),
                ..Default::default()
            });
            let row = d.insert(page, row).unwrap();
            let cid = d.alloc_id();
            let mut c = Node::rectangle(cid, Rect::new(0.0, 0.0, 20.0, 10.0));
            c.layout_child = LayoutChild { vertical: Sizing::Fill, min_h: Some(40.0), ..Default::default() };
            let cid = d.insert(row, c).unwrap();

            d.relayout_subtree(row);
            assert_eq!(d.get(row).unwrap().size, Vec2::new(100.0, 40.0), "wrap {wrap}");
            let cn = d.get(cid).unwrap();
            assert_eq!((cn.transform.tx, cn.transform.ty, cn.size.x, cn.size.y), (0.0, 0.0, 20.0, 40.0), "wrap {wrap}");
        }
    }

    #[test]
    fn wrapped_hugging_frame_with_several_lines_reserves_and_shrinks_margins_on_line_two() {
        // Issue #530 item 4: wrapped hugging frame (width 100) with line 1 (80x30) and line 2
        // holding a plain child A2 (40x80) + a margined Fill child B2 (40x10, margin top 25, bottom 25).
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let row_id = d.alloc_id();
        let mut row = Node::frame(row_id, Rect::new(0.0, 0.0, 100.0, 1.0));
        row.kind = NodeKind::Frame(FrameProps {
            auto_layout: Some(AutoLayout {
                direction: LayoutDirection::Horizontal,
                counter_sizing: Sizing::Hug,
                wrap: true,
                counter_spacing: 10.0,
                ..Default::default()
            }),
            ..Default::default()
        });
        let row = d.insert(page, row).unwrap();
        let l1_id = d.alloc_id();
        let _l1 = d.insert(row, Node::rectangle(l1_id, Rect::new(0.0, 0.0, 80.0, 30.0))).unwrap();
        let a2_id = d.alloc_id();
        let a2 = d.insert(row, Node::rectangle(a2_id, Rect::new(0.0, 0.0, 40.0, 80.0))).unwrap();
        let b2_id = d.alloc_id();
        let mut b2 = Node::rectangle(b2_id, Rect::new(0.0, 0.0, 40.0, 10.0));
        b2.layout_child = LayoutChild { vertical: Sizing::Fill, margin: [25.0, 0.0, 25.0, 0.0], ..Default::default() };
        let b2 = d.insert(row, b2).unwrap();

        // Line 1 is 30 tall; gap is 10; Line 2 is 80 tall (A2), so B2 stretches to 80 - 50 = 30 at y = 30 + 10 + 25 = 65.
        d.relayout_subtree(row);
        assert_eq!(d.get(row).unwrap().size.y, 120.0);
        assert_eq!((d.get(b2).unwrap().transform.ty, d.get(b2).unwrap().size.y), (65.0, 30.0));

        // A2 shrinks to 40: Line 2 shrinks to B2's margin floor (50), so total height is 30 + 10 + 50 = 90.
        d.get_mut(a2).unwrap().size.y = 40.0;
        d.relayout_subtree(row);
        assert_eq!(d.get(row).unwrap().size.y, 90.0);
        assert_eq!((d.get(b2).unwrap().transform.ty, d.get(b2).unwrap().size.y), (65.0, 0.0));
    }

    #[test]
    fn cross_fill_child_with_max_limit_and_margins_clamps_size_and_aligns_margin_box_in_hugging_line() {
        // Issue #530 item 4: Fill child with max_h = 30 and margins [10, 0, 10, 0] beside an 80px tall plain child
        // in a Center-aligned hugging row: B clamps to 30, outer box is 50, centered at (80 - 50)/2 + 10 = 25.
        let (mut d, row, _a, b) = row_with_cross_fill_margins(80.0, false);
        if let Some(al) = d.get_mut(row).and_then(|n| n.kind.frame_props_mut()).and_then(|f| f.auto_layout.as_mut()) {
            al.counter_align = LayoutAlign::Center;
        }
        let bn = d.get_mut(b).unwrap();
        bn.layout_child.max_h = Some(30.0);
        bn.layout_child.margin = [10.0, 0.0, 10.0, 0.0];
        d.relayout_subtree(row);
        assert_eq!(d.get(row).unwrap().size.y, 80.0);
        assert_eq!((d.get(b).unwrap().transform.ty, d.get(b).unwrap().size.y), (25.0, 30.0));
    }

    #[test]
    fn counter_hugging_wrapped_row_measures_a_lone_cross_fill_child_by_its_margin_box_across_relayouts() {
        // One 20x40 child that fills the height, alone in its line. The line is the child's margin box (40 + top margin), and
        // the child stretches back to 40, so a negative margin must not feed the stretched size back into the measure.
        for (top, line) in [(0.0f32, 40.0f32), (-10.0, 30.0)] {
            let mut d = Document::new("t");
            let page = d.add_page("p");
            let row_id = d.alloc_id();
            let mut row = Node::frame(row_id, Rect::new(0.0, 0.0, 100.0, 1.0));
            row.kind = NodeKind::Frame(FrameProps {
                auto_layout: Some(AutoLayout { direction: LayoutDirection::Horizontal, counter_sizing: Sizing::Hug, wrap: true, ..Default::default() }),
                ..Default::default()
            });
            let row = d.insert(page, row).unwrap();
            let cid = d.alloc_id();
            let mut c = Node::rectangle(cid, Rect::new(0.0, 0.0, 20.0, 40.0));
            c.layout_child = LayoutChild { vertical: Sizing::Fill, margin: [top, 0.0, 0.0, 0.0], ..Default::default() };
            let cid = d.insert(row, c).unwrap();
            for pass in 0..3 {
                d.relayout_subtree(row);
                assert_eq!(d.get(row).unwrap().size.y, line, "top margin {top}, relayout {pass}: line");
                assert_eq!(d.get(cid).unwrap().size.y, 40.0, "top margin {top}, relayout {pass}: child");
            }
        }
    }

    #[test]
    fn end_aligned_overflow_places_first_child_at_negative_free_space() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let container_id = d.alloc_id();
        let mut container = Node::frame(container_id, Rect::new(0.0, 0.0, 400.0, 782.0));
        container.kind = NodeKind::Frame(FrameProps {
            auto_layout: Some(AutoLayout {
                direction: LayoutDirection::Vertical,
                primary_sizing: Sizing::Fixed,
                counter_sizing: Sizing::Fixed,
                primary_align: LayoutAlign::End,
                padding: [0.0, 24.0, 24.0, 24.0],
                item_spacing: 32.0,
                ..Default::default()
            }),
            ..Default::default()
        });
        let container = d.insert(page, container).unwrap();
        let c1_id = d.alloc_id();
        let c1 = d.insert(container, Node::rectangle(c1_id, Rect::new(24.0, 0.0, 352.0, 888.1))).unwrap();
        let c2_id = d.alloc_id();
        let c2 = d.insert(container, Node::rectangle(c2_id, Rect::new(24.0, 0.0, 352.0, 20.0))).unwrap();
        let c3_id = d.alloc_id();
        let c3 = d.insert(container, Node::rectangle(c3_id, Rect::new(24.0, 0.0, 352.0, 306.0))).unwrap();

        // Inner main height is 782 - 24 = 758; content + gaps = 888.1 + 20 + 306 + 64 = 1278.1;
        // free space is 758 - 1278.1 = -520.1, so the last child sits flush with y = 758 - 306 = 452
        // and the first child overflows at the start at y = -520.1.
        d.relayout_subtree(container);
        assert!((d.get(c1).unwrap().transform.ty - (-520.1)).abs() < 1e-3, "got {}", d.get(c1).unwrap().transform.ty);
        assert!((d.get(c2).unwrap().transform.ty - 400.0).abs() < 1e-3, "got {}", d.get(c2).unwrap().transform.ty);
        assert!((d.get(c3).unwrap().transform.ty - 452.0).abs() < 1e-3, "got {}", d.get(c3).unwrap().transform.ty);
    }

    #[test]
    fn center_aligned_overflow_splits_negative_free_space_on_both_sides() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let container_id = d.alloc_id();
        let mut container = Node::frame(container_id, Rect::new(0.0, 0.0, 200.0, 200.0));
        container.kind = NodeKind::Frame(FrameProps {
            auto_layout: Some(AutoLayout {
                direction: LayoutDirection::Vertical,
                primary_sizing: Sizing::Fixed,
                counter_sizing: Sizing::Fixed,
                primary_align: LayoutAlign::Center,
                item_spacing: 20.0,
                ..Default::default()
            }),
            ..Default::default()
        });
        let container = d.insert(page, container).unwrap();
        let c1_id = d.alloc_id();
        let c1 = d.insert(container, Node::rectangle(c1_id, Rect::new(0.0, 0.0, 100.0, 150.0))).unwrap();
        let c2_id = d.alloc_id();
        let c2 = d.insert(container, Node::rectangle(c2_id, Rect::new(0.0, 0.0, 100.0, 150.0))).unwrap();

        // Inner height is 200; content + gap = 320; free space is -120, split evenly as -60 on each side.
        d.relayout_subtree(container);
        assert!((d.get(c1).unwrap().transform.ty - (-60.0)).abs() < 1e-3, "got {}", d.get(c1).unwrap().transform.ty);
        assert!((d.get(c2).unwrap().transform.ty - 110.0).abs() < 1e-3, "got {}", d.get(c2).unwrap().transform.ty);
    }

    #[test]
    fn space_distribution_modes_keep_overflowing_content_anchored_at_start() {
        // Space-between overflow collapses the gap from layout version 1 on; version 0 lets the children overlap (tested below).
        for (align, layout_version) in [(LayoutAlign::SpaceBetween, 1), (LayoutAlign::SpaceAround, 0), (LayoutAlign::SpaceEvenly, 0)] {
            let mut d = Document::new("t");
            let page = d.add_page("p");
            let container_id = d.alloc_id();
            let mut container = Node::frame(container_id, Rect::new(0.0, 0.0, 100.0, 100.0));
            container.kind = NodeKind::Frame(FrameProps {
                auto_layout: Some(AutoLayout {
                    direction: LayoutDirection::Vertical,
                    primary_sizing: Sizing::Fixed,
                    counter_sizing: Sizing::Fixed,
                    primary_align: align,
                    item_spacing: 10.0,
                    layout_version,
                    ..Default::default()
                }),
                ..Default::default()
            });
            let container = d.insert(page, container).unwrap();
            let c1_id = d.alloc_id();
            let c1 = d.insert(container, Node::rectangle(c1_id, Rect::new(0.0, 0.0, 50.0, 80.0))).unwrap();
            let c2_id = d.alloc_id();
            let c2 = d.insert(container, Node::rectangle(c2_id, Rect::new(0.0, 0.0, 50.0, 80.0))).unwrap();

            d.relayout_subtree(container);
            assert_eq!(d.get(c1).unwrap().transform.ty, 0.0, "{align:?}: first child stays at start");
            assert_eq!(d.get(c2).unwrap().transform.ty, 80.0, "{align:?}: gap collapses to 0 when overflowing");
        }
    }

    /// A fixed `frame`-sized frame laid out by `al` (both axes forced to Fixed) holding one `(w, h)` rectangle per entry of
    /// `kids`. Nothing is solved yet.
    fn fixed_frame_with_rectangles(al: AutoLayout, frame: (f32, f32), kids: &[(f32, f32)]) -> (Document, NodeId, Vec<NodeId>) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let frame_id = d.alloc_id();
        let mut f = Node::frame(frame_id, Rect::new(0.0, 0.0, frame.0, frame.1));
        f.kind = NodeKind::Frame(FrameProps {
            auto_layout: Some(AutoLayout { primary_sizing: Sizing::Fixed, counter_sizing: Sizing::Fixed, ..al }),
            ..Default::default()
        });
        let frame_id = d.insert(page, f).unwrap();
        let ids = kids
            .iter()
            .map(|&(w, h)| {
                let id = d.alloc_id();
                d.insert(frame_id, Node::rectangle(id, Rect::new(0.0, 0.0, w, h))).unwrap()
            })
            .collect();
        (d, frame_id, ids)
    }

    /// `(x, y, width, height)` of a node inside its parent.
    fn slot_of(d: &Document, id: NodeId) -> (f32, f32, f32, f32) {
        let n = d.get(id).unwrap();
        (n.transform.tx, n.transform.ty, n.size.x, n.size.y)
    }

    fn space_between(direction: LayoutDirection, layout_version: u8) -> AutoLayout {
        AutoLayout { direction, primary_align: LayoutAlign::SpaceBetween, layout_version, ..Default::default() }
    }

    #[test]
    fn a_version_0_space_between_frame_centres_its_lone_child_on_the_main_axis() {
        // Vertical 720 x 960 frame, one 720 x 484 child: (960 - 484) / 2 = 238.
        let (mut d, frame, kids) = fixed_frame_with_rectangles(space_between(LayoutDirection::Vertical, 0), (720.0, 960.0), &[(720.0, 484.0)]);
        d.get_mut(kids[0]).unwrap().layout_child.horizontal = Sizing::Fill;
        d.relayout_subtree(frame);
        assert_eq!(slot_of(&d, kids[0]), (0.0, 238.0, 720.0, 484.0));

        // The padding along the main axis counts: 32 + (1216 - 360) / 2 = 460.
        let al = AutoLayout { padding: [0.0, 32.0, 0.0, 32.0], ..space_between(LayoutDirection::Horizontal, 0) };
        let (mut d, frame, kids) = fixed_frame_with_rectangles(al, (1280.0, 100.0), &[(360.0, 100.0)]);
        d.relayout_subtree(frame);
        assert_eq!(slot_of(&d, kids[0]), (460.0, 0.0, 360.0, 100.0));
    }

    #[test]
    fn a_version_0_space_between_frame_leaves_a_lone_main_axis_fill_child_clamped_by_its_max_at_the_start() {
        // Pins today's result for the lone Fill child that its max clamps: it is not centred. Whether Figma centres it is
        // not settled; a decision either way has to change this test.
        let al = AutoLayout { padding: [0.0, 32.0, 0.0, 32.0], ..space_between(LayoutDirection::Horizontal, 0) };
        let (mut d, frame, kids) = fixed_frame_with_rectangles(al, (1280.0, 100.0), &[(360.0, 100.0)]);
        let lc = &mut d.get_mut(kids[0]).unwrap().layout_child;
        lc.horizontal = Sizing::Fill;
        lc.max_w = Some(360.0);
        d.relayout_subtree(frame);
        assert_eq!(slot_of(&d, kids[0]), (32.0, 0.0, 360.0, 100.0));
    }

    #[test]
    fn a_version_1_space_between_frame_keeps_its_lone_child_at_the_start() {
        let (mut d, frame, kids) = fixed_frame_with_rectangles(space_between(LayoutDirection::Vertical, 1), (720.0, 960.0), &[(720.0, 484.0)]);
        d.get_mut(kids[0]).unwrap().layout_child.horizontal = Sizing::Fill;
        d.relayout_subtree(frame);
        assert_eq!(slot_of(&d, kids[0]), (0.0, 0.0, 720.0, 484.0));
    }

    #[test]
    fn a_space_between_frame_with_one_child_per_wrapped_line_keeps_every_child_at_the_start() {
        // Wrap frames are not part of the legacy lone-child centring: a line with one child (or a frame with one child) stays put.
        for layout_version in [0, 1] {
            let al = AutoLayout { wrap: true, ..space_between(LayoutDirection::Horizontal, layout_version) };
            let (mut d, frame, kids) = fixed_frame_with_rectangles(al, (400.0, 100.0), &[(300.0, 30.0), (300.0, 30.0)]);
            d.relayout_subtree(frame);
            assert_eq!(slot_of(&d, kids[0]), (0.0, 0.0, 300.0, 30.0), "version {layout_version}");
            assert_eq!(slot_of(&d, kids[1]), (0.0, 30.0, 300.0, 30.0), "version {layout_version}");

            let (mut d, frame, kids) = fixed_frame_with_rectangles(
                AutoLayout { wrap: true, ..space_between(LayoutDirection::Horizontal, layout_version) },
                (400.0, 100.0),
                &[(100.0, 30.0)],
            );
            d.relayout_subtree(frame);
            assert_eq!(slot_of(&d, kids[0]), (0.0, 0.0, 100.0, 30.0), "version {layout_version}");
        }
    }

    #[test]
    fn space_between_overflow_overlaps_children_in_version_0_and_stops_at_a_zero_gap_from_version_1() {
        // Two 80 long children in a 100 long frame: free space is -60.
        for (layout_version, second) in [(0, 20.0), (1, 80.0)] {
            let (mut d, frame, kids) =
                fixed_frame_with_rectangles(space_between(LayoutDirection::Vertical, layout_version), (50.0, 100.0), &[(50.0, 80.0), (50.0, 80.0)]);
            d.relayout_subtree(frame);
            assert_eq!(slot_of(&d, kids[0]).1, 0.0, "version {layout_version}");
            assert_eq!(slot_of(&d, kids[1]).1, second, "version {layout_version}");
        }
    }

    #[test]
    fn space_between_distributes_positive_free_space_the_same_in_every_version() {
        for layout_version in [0, 1] {
            let (mut d, frame, kids) = fixed_frame_with_rectangles(
                space_between(LayoutDirection::Vertical, layout_version),
                (50.0, 100.0),
                &[(50.0, 20.0), (50.0, 20.0), (50.0, 20.0)],
            );
            d.relayout_subtree(frame);
            let ys: Vec<f32> = kids.iter().map(|k| slot_of(&d, *k).1).collect();
            assert_eq!(ys, [0.0, 40.0, 80.0], "version {layout_version}");
        }
    }

    #[test]
    fn a_negative_item_spacing_does_not_overlap_children_under_version_1_space_between() {
        let al = AutoLayout { item_spacing: -10.0, ..space_between(LayoutDirection::Vertical, 1) };
        let (mut d, frame, kids) = fixed_frame_with_rectangles(al, (50.0, 100.0), &[(50.0, 80.0), (50.0, 80.0)]);
        d.relayout_subtree(frame);
        assert_eq!(slot_of(&d, kids[0]).1, 0.0);
        assert_eq!(slot_of(&d, kids[1]).1, 80.0);
    }

    #[test]
    fn a_wrapping_space_between_line_keeps_the_negative_item_spacing_gap_in_every_version() {
        // 80 + (-30) + 40 = 90 fits the 100 wide frame, so both children share one line with free space -20.
        for layout_version in [0, 1] {
            let al = AutoLayout { wrap: true, item_spacing: -30.0, ..space_between(LayoutDirection::Horizontal, layout_version) };
            let (mut d, frame, kids) = fixed_frame_with_rectangles(al, (100.0, 100.0), &[(80.0, 30.0), (40.0, 30.0)]);
            d.relayout_subtree(frame);
            assert_eq!(slot_of(&d, kids[0]).0, 0.0, "version {layout_version}");
            assert_eq!(slot_of(&d, kids[1]).0, 60.0, "version {layout_version}");
        }
    }

    #[test]
    fn a_version_0_space_between_frame_centres_a_lone_child_inside_its_padding_beside_an_absolute_sibling() {
        // Width 200, padding left 40 and right 0: the content box is 160 wide, so a 40 wide child sits at 40 + 60 = 100. The
        // absolutely positioned sibling is not a flow child, so the child is still alone.
        let al = AutoLayout { padding: [0.0, 0.0, 0.0, 40.0], ..space_between(LayoutDirection::Horizontal, 0) };
        let (mut d, frame, kids) = fixed_frame_with_rectangles(al, (200.0, 100.0), &[(40.0, 100.0), (30.0, 30.0)]);
        d.get_mut(kids[1]).unwrap().layout_child.absolute = true;
        d.relayout_subtree(frame);
        assert_eq!(slot_of(&d, kids[0]).0, 100.0);
    }

    #[test]
    fn auto_spacing_modes_split_the_space_two_max_clamped_fill_children_leave_free() {
        // Frame 300, Fill children clamped to 100 and 80: free space 120 goes to the mode's own distribution.
        for layout_version in [0, 1] {
            for (align, second_x, first_x) in [
                (LayoutAlign::SpaceBetween, 220.0, 0.0),
                (LayoutAlign::SpaceAround, 30.0 + 100.0 + 60.0, 30.0),
                (LayoutAlign::SpaceEvenly, 40.0 + 100.0 + 40.0, 40.0),
            ] {
                let al = AutoLayout { direction: LayoutDirection::Horizontal, primary_align: align, item_spacing: 96.0, layout_version, ..Default::default() };
                let (mut d, frame, kids) = fixed_frame_with_rectangles(al, (300.0, 100.0), &[(10.0, 100.0), (10.0, 100.0)]);
                for (kid, max) in kids.iter().zip([100.0, 80.0]) {
                    let lc = &mut d.get_mut(*kid).unwrap().layout_child;
                    lc.horizontal = Sizing::Fill;
                    lc.max_w = Some(max);
                }
                d.relayout_subtree(frame);
                let what = format!("{align:?}, version {layout_version}");
                assert_eq!(slot_of(&d, kids[0]).0, first_x, "{what}");
                assert_eq!(slot_of(&d, kids[0]).2, 100.0, "{what}");
                assert_eq!(slot_of(&d, kids[1]).0, second_x, "{what}");
                assert_eq!(slot_of(&d, kids[1]).2, 80.0, "{what}");
            }
        }
    }

    /// A horizontal 1216 wide frame with item spacing 96 holding a 328 wide fixed child and a Fill child.
    fn fixed_child_beside_fill_child(primary_align: LayoutAlign, layout_version: u8) -> (Document, NodeId, Vec<NodeId>) {
        let al = AutoLayout { direction: LayoutDirection::Horizontal, primary_align, item_spacing: 96.0, layout_version, ..Default::default() };
        let (mut d, frame, kids) = fixed_frame_with_rectangles(al, (1216.0, 200.0), &[(328.0, 100.0), (100.0, 100.0)]);
        d.get_mut(kids[1]).unwrap().layout_child.horizontal = Sizing::Fill;
        d.relayout_subtree(frame);
        (d, frame, kids)
    }

    #[test]
    fn auto_spacing_modes_reserve_no_item_spacing_for_a_fill_child() {
        for layout_version in [0, 1] {
            for align in [LayoutAlign::SpaceBetween, LayoutAlign::SpaceAround, LayoutAlign::SpaceEvenly] {
                let (d, _, kids) = fixed_child_beside_fill_child(align, layout_version);
                let what = format!("{align:?}, version {layout_version}");
                assert_eq!(slot_of(&d, kids[0]), (0.0, 0.0, 328.0, 100.0), "{what}");
                assert_eq!(slot_of(&d, kids[1]), (328.0, 0.0, 888.0, 100.0), "{what}");
            }
        }
    }

    #[test]
    fn start_center_and_end_packing_still_reserve_item_spacing_for_a_fill_child_in_every_version() {
        for layout_version in [0, 1] {
            for align in [LayoutAlign::Start, LayoutAlign::Center, LayoutAlign::End] {
                let (d, _, kids) = fixed_child_beside_fill_child(align, layout_version);
                let what = format!("{align:?}, version {layout_version}");
                assert_eq!(slot_of(&d, kids[1]), (424.0, 0.0, 792.0, 100.0), "{what}");
            }
        }
    }

    #[test]
    fn start_center_and_end_packing_place_a_lone_child_the_same_in_every_version() {
        for layout_version in [0, 1] {
            for (align, x) in [(LayoutAlign::Start, 0.0), (LayoutAlign::Center, 508.0), (LayoutAlign::End, 1016.0)] {
                let al = AutoLayout { direction: LayoutDirection::Horizontal, primary_align: align, layout_version, ..Default::default() };
                let (mut d, frame, kids) = fixed_frame_with_rectangles(al, (1216.0, 100.0), &[(200.0, 100.0)]);
                d.relayout_subtree(frame);
                assert_eq!(slot_of(&d, kids[0]).0, x, "{align:?}, version {layout_version}");
            }
        }
    }

    /// A 1440 x 1440 fixed frame (direction `dir`, counter alignment `align`) holding one rectangle whose cross-axis
    /// size is `Fill`, `align_self` as given and cross-axis limits `(min, max)`. Returns the document and the child's id
    /// after one relayout.
    fn frame_with_cross_fill_child(dir: LayoutDirection, align: LayoutAlign, align_self: AlignSelf, limits: (Option<f32>, Option<f32>)) -> (Document, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let frame_id = d.alloc_id();
        let mut frame = Node::frame(frame_id, Rect::new(0.0, 0.0, 1440.0, 1440.0));
        frame.kind = NodeKind::Frame(FrameProps {
            auto_layout: Some(AutoLayout {
                direction: dir,
                primary_sizing: Sizing::Fixed,
                counter_sizing: Sizing::Fixed,
                counter_align: align,
                ..Default::default()
            }),
            ..Default::default()
        });
        let frame = d.insert(page, frame).unwrap();
        let kid_id = d.alloc_id();
        let mut kid = Node::rectangle(kid_id, Rect::new(0.0, 0.0, 100.0, 100.0));
        kid.layout_child = LayoutChild { align_self, ..Default::default() };
        if dir == LayoutDirection::Vertical {
            kid.layout_child.horizontal = Sizing::Fill;
            (kid.layout_child.min_w, kid.layout_child.max_w) = limits;
        } else {
            kid.layout_child.vertical = Sizing::Fill;
            (kid.layout_child.min_h, kid.layout_child.max_h) = limits;
        }
        let kid = d.insert(frame, kid).unwrap();
        d.relayout_subtree(frame);
        (d, kid)
    }

    /// Offset and size of a node along the frame's cross axis.
    fn cross_span(d: &Document, id: NodeId, dir: LayoutDirection) -> (f32, f32) {
        let n = d.get(id).unwrap();
        if dir == LayoutDirection::Vertical {
            (n.transform.tx, n.size.x)
        } else {
            (n.transform.ty, n.size.y)
        }
    }

    #[test]
    fn a_stretched_child_clamped_by_its_max_size_follows_the_frames_counter_alignment() {
        for dir in [LayoutDirection::Vertical, LayoutDirection::Horizontal] {
            for (align, offset) in [(LayoutAlign::Start, 0.0), (LayoutAlign::Center, 80.0), (LayoutAlign::End, 160.0)] {
                for align_self in [AlignSelf::Stretch, AlignSelf::Auto] {
                    let (d, kid) = frame_with_cross_fill_child(dir, align, align_self, (None, Some(1280.0)));
                    assert_eq!(cross_span(&d, kid, dir), (offset, 1280.0), "{dir:?} {align:?} {align_self:?}");
                }
            }
        }
    }

    #[test]
    fn a_stretched_child_below_its_max_size_fills_the_frame_whatever_the_alignment() {
        for dir in [LayoutDirection::Vertical, LayoutDirection::Horizontal] {
            for align in [LayoutAlign::Start, LayoutAlign::Center, LayoutAlign::End] {
                let (d, kid) = frame_with_cross_fill_child(dir, align, AlignSelf::Stretch, (None, Some(2000.0)));
                assert_eq!(cross_span(&d, kid, dir), (0.0, 1440.0), "{dir:?} {align:?}");
            }
        }
    }

    #[test]
    fn an_explicit_start_align_self_still_pins_a_clamped_child_to_the_start() {
        let (d, kid) = frame_with_cross_fill_child(LayoutDirection::Vertical, LayoutAlign::Center, AlignSelf::Start, (None, Some(1280.0)));
        assert_eq!(cross_span(&d, kid, LayoutDirection::Vertical), (0.0, 1280.0));
    }

    fn wrapped_two_line_layout(counter_align: LayoutAlign) -> AutoLayout {
        AutoLayout {
            direction: LayoutDirection::Horizontal,
            wrap: true,
            primary_align: LayoutAlign::Center,
            counter_align,
            item_spacing: 32.0,
            counter_spacing: 32.0,
            padding: [32.0, 32.0, 32.0, 32.0],
            ..AutoLayout::default()
        }
    }

    #[test]
    fn wrap_auto_content_aligns_line_block_by_counter_align() {
        let eight_rects = [(88.0, 88.0); 8];
        for (align, (y0, y1)) in [(LayoutAlign::Start, (32.0, 152.0)), (LayoutAlign::Center, (33.0, 153.0))] {
            let (mut d, frame, kids) = fixed_frame_with_rectangles(wrapped_two_line_layout(align), (560.0, 274.0), &eight_rects);
            d.relayout_subtree(frame);
            for &k in &kids[0..4] {
                assert_eq!(slot_of(&d, k).1, y0, "{align:?} first line");
            }
            for &k in &kids[4..8] {
                assert_eq!(slot_of(&d, k).1, y1, "{align:?} second line");
            }
        }
    }

    #[test]
    fn wrap_auto_content_end_align_with_positive_slack_places_block_at_far_padding_edge() {
        let eight_rects = [(88.0, 88.0); 8];
        let (mut d, frame, kids) = fixed_frame_with_rectangles(wrapped_two_line_layout(LayoutAlign::End), (560.0, 274.0), &eight_rects);
        d.relayout_subtree(frame);
        for &k in &kids[0..4] {
            assert_eq!(slot_of(&d, k).1, 34.0);
        }
        for &k in &kids[4..8] {
            assert_eq!(slot_of(&d, k).1, 154.0);
        }
    }

    #[test]
    fn wrap_auto_content_overflow_under_center_and_end_stays_at_near_padding_edge() {
        let eight_rects = [(88.0, 88.0); 8];
        for align in [LayoutAlign::Center, LayoutAlign::End] {
            let (mut d, frame, kids) = fixed_frame_with_rectangles(wrapped_two_line_layout(align), (560.0, 200.0), &eight_rects);
            d.relayout_subtree(frame);
            assert_eq!(slot_of(&d, kids[0]).1, 32.0, "{align:?} first line");
            assert_eq!(slot_of(&d, kids[4]).1, 152.0, "{align:?} second line");
        }
    }

    #[test]
    fn wrap_auto_content_hugging_counter_axis_does_not_offset_lines() {
        let eight_rects = [(88.0, 88.0); 8];
        for align in [LayoutAlign::Center, LayoutAlign::End] {
            let (mut d, frame, kids) = fixed_frame_with_rectangles(wrapped_two_line_layout(align), (560.0, 400.0), &eight_rects);
            if let Some(al) = d.get_mut(frame).and_then(|n| n.kind.frame_props_mut()).and_then(|f| f.auto_layout.as_mut()) {
                al.counter_sizing = Sizing::Hug;
            }
            d.relayout_subtree(frame);
            assert_eq!(d.get(frame).unwrap().size.y, 272.0, "{align:?} hugged height");
            assert_eq!(slot_of(&d, kids[0]).1, 32.0, "{align:?} first line");
            assert_eq!(slot_of(&d, kids[4]).1, 152.0, "{align:?} second line");
        }
    }

    #[test]
    fn wrap_auto_content_single_line_with_center_alignment_has_no_double_offset() {
        let four_rects = [(88.0, 88.0), (88.0, 48.0), (88.0, 88.0), (88.0, 48.0)];
        let (mut d, frame, kids) = fixed_frame_with_rectangles(wrapped_two_line_layout(LayoutAlign::Center), (560.0, 274.0), &four_rects);
        d.relayout_subtree(frame);
        assert_eq!(slot_of(&d, kids[0]).1, 32.0);
        assert_eq!(slot_of(&d, kids[1]).1, 52.0);
        assert_eq!(slot_of(&d, kids[2]).1, 32.0);
        assert_eq!(slot_of(&d, kids[3]).1, 52.0);
    }

    #[test]
    fn wrap_auto_content_with_absolute_child_aligns_flow_lines_and_leaves_absolute_child_unmoved() {
        let eight_rects = [(88.0, 88.0); 8];
        let (mut d, frame, kids) = fixed_frame_with_rectangles(wrapped_two_line_layout(LayoutAlign::Center), (560.0, 274.0), &eight_rects);
        let abs_id = d.alloc_id();
        let mut abs_node = Node::rectangle(abs_id, Rect::new(10.0, 15.0, 40.0, 40.0));
        abs_node.layout_child.absolute = true;
        let abs_id = d.insert(frame, abs_node).unwrap();
        d.relayout_subtree(frame);
        assert_eq!(slot_of(&d, abs_id), (10.0, 15.0, 40.0, 40.0));
        assert_eq!(slot_of(&d, kids[0]).1, 33.0);
        assert_eq!(slot_of(&d, kids[4]).1, 153.0);
    }

    #[test]
    fn wrap_auto_content_with_every_child_stretching_keeps_main_positions() {
        let eight_rects = [(88.0, 88.0); 8];
        let (mut d, frame, kids) = fixed_frame_with_rectangles(wrapped_two_line_layout(LayoutAlign::Center), (560.0, 274.0), &eight_rects);
        for &k in &kids {
            d.get_mut(k).unwrap().layout_child.vertical = Sizing::Fill;
        }
        d.relayout_subtree(frame);
        for &k in &kids[0..4] {
            assert_eq!((slot_of(&d, k).1, slot_of(&d, k).3), (32.0, 88.0));
        }
        for &k in &kids[4..8] {
            assert_eq!((slot_of(&d, k).1, slot_of(&d, k).3), (152.0, 88.0));
        }
    }

    #[test]
    fn wrap_auto_content_with_one_fixed_child_among_stretching_ones_still_aligns_the_block() {
        let eight_rects = [(88.0, 88.0); 8];
        let (mut d, frame, kids) = fixed_frame_with_rectangles(wrapped_two_line_layout(LayoutAlign::Center), (560.0, 274.0), &eight_rects);
        // Only the block of lines made of stretching children alone keeps main's start-packed
        // positions; one fixed child is enough for the block to follow the counter alignment.
        for &k in &kids[0..7] {
            d.get_mut(k).unwrap().layout_child.vertical = Sizing::Fill;
        }
        d.relayout_subtree(frame);
        for &k in &kids[0..4] {
            assert_eq!((slot_of(&d, k).1, slot_of(&d, k).3), (33.0, 88.0));
        }
        for &k in &kids[4..8] {
            assert_eq!((slot_of(&d, k).1, slot_of(&d, k).3), (153.0, 88.0));
        }
    }
}
