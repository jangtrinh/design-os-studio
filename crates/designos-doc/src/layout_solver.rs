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

use crate::{AlignSelf, AutoLayout, CounterAlignContent, LayoutAlign, LayoutDirection, Rect, Sizing, Vec2};

pub use crate::layout_solver_grid::compute_grid;
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
    // Without margins it counts as a plain child in a wrapped line; in a flow line that does not wrap it counts for
    // nothing while a plain child sizes the line, and keeps its natural extent as the line size when none does, so a
    // hugging frame whose children all stretch does not fold to its padding. Its cross minimum floors the line either
    // way, so a stretched child never overflows the frame.
    let mut line_cross: Vec<f32> = lines
        .iter()
        .map(|l| {
            let (mut plain, mut natural, mut floor) = (None::<f32>, 0.0f32, 0.0f32);
            for k in l.iter().map(|i| &kids[*i]) {
                let margins = ax.margin_cross(&k.margin);
                if !k.stretches_cross() || (al.wrap && margins <= 0.0) {
                    plain = Some(plain.map_or(outer_cross(k), |p| p.max(outer_cross(k))));
                } else {
                    floor = floor.max(ax.clamp_cross(&k.limits, 0.0) + margins);
                    if margins <= 0.0 {
                        natural = natural.max(nat_cross(k));
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
    for (li, line) in lines.iter().enumerate() {
        let k = line.len() as f32;
        // Main sizes (distribute fill children by grow weight with a CSS flex freeze loop for min/max clamping).
        let mut mains: Vec<f32> = line.iter().map(|i| nat_main(&kids[*i])).collect();
        if !al.wrap && !hug_main {
            let mut frozen: Vec<bool> = line.iter().map(|i| !kids[*i].main_fill).collect();
            let fixed: f32 = line.iter().zip(&frozen).filter(|(_, &f)| f).map(|(i, _)| nat_main(&kids[*i])).sum();
            let total_margins: f32 = line.iter().map(|i| ax.margin_main(&kids[*i].margin)).sum();
            let mut rem_for_fill = (main_avail - al.item_spacing * (k - 1.0) - fixed - total_margins).max(0.0);
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
        // CSS justify-content: the free space ignores the fixed gap; with no free space the items pack from the start.
        let free = main_avail - sum_main;
        let (start, gap) = match al.primary_align {
            LayoutAlign::SpaceBetween if k > 1.0 => (0.0, (free / (k - 1.0)).max(al.item_spacing.min(0.0))),
            LayoutAlign::SpaceAround if free > 0.0 => (free / k / 2.0, free / k),
            LayoutAlign::SpaceEvenly if free > 0.0 => (free / (k + 1.0), free / (k + 1.0)),
            LayoutAlign::SpaceAround | LayoutAlign::SpaceEvenly => (0.0, 0.0),
            LayoutAlign::Center => (((main_avail - sum_main - al.item_spacing * (k - 1.0)) / 2.0).max(0.0), al.item_spacing),
            LayoutAlign::End => ((main_avail - sum_main - al.item_spacing * (k - 1.0)).max(0.0), al.item_spacing),
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
                AlignSelf::Start | AlignSelf::Stretch => 0.0,
                AlignSelf::Center => (line_cross[li] - outer_c) / 2.0,
                AlignSelf::End => line_cross[li] - outer_c,
                // Baseline is stored (and round-trips through .fig) but baselines are not aligned yet:
                // it behaves like Auto and inherits the frame's counter_align.
                AlignSelf::Auto | AlignSelf::Baseline => match al.counter_align {
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
}
