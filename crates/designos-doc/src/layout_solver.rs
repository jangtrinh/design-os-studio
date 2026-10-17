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

use crate::{AutoLayout, CounterAlignContent, LayoutAlign, LayoutDirection, Rect, Sizing, Vec2};

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
    let hug_main = al.primary_sizing == Sizing::Hug && !al.wrap;
    let hug_cross = al.counter_sizing == Sizing::Hug;
    // a Fixed axis takes the clamped size; a hugging axis is clamped again once its content is known
    let mut frame_main = ax.clamp_main(&own, ax.main(frame));
    let mut frame_cross = ax.clamp_cross(&own, ax.cross(frame));
    // natural extents with the child's own min/max applied
    let nat_main = |k: &ChildIn| ax.clamp_main(&k.limits, ax.main(k.size));
    let nat_cross = |k: &ChildIn| ax.clamp_cross(&k.limits, ax.cross(k.size));

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
            let m = nat_main(k);
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
        let total: f32 = kids.iter().map(nat_main).sum::<f32>() + al.item_spacing * (kids.len() as f32 - 1.0);
        frame_main = ax.clamp_main(&own, total + pm);
    }
    let main_avail = (frame_main - pm).max(0.0);

    // Cross extent of each line.
    let mut line_cross: Vec<f32> =
        lines.iter().map(|l| l.iter().filter(|i| !kids[**i].cross_fill || al.wrap).map(|i| nat_cross(&kids[*i])).fold(0.0, f32::max)).collect();
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
        // Main sizes (distribute fill children by grow weight, then apply each child's min/max).
        let fixed: f32 = line.iter().filter(|i| !kids[**i].main_fill || al.wrap).map(|i| nat_main(&kids[*i])).sum();
        let weight: f32 = line.iter().filter(|i| kids[**i].main_fill && !al.wrap).map(|i| fill_weight(&kids[*i])).sum();
        let free_for_fill = (main_avail - al.item_spacing * (k - 1.0) - fixed).max(0.0);
        let mains: Vec<f32> = line
            .iter()
            .map(|i| {
                let kid = &kids[*i];
                if kid.main_fill && !al.wrap && !hug_main && weight > 0.0 {
                    ax.clamp_main(&kid.limits, free_for_fill * fill_weight(kid) / weight)
                } else {
                    nat_main(kid)
                }
            })
            .collect();
        let sum_main: f32 = mains.iter().sum();
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
            let c = if kid.cross_fill { ax.clamp_cross(&kid.limits, line_cross[li]) } else { nat_cross(kid) };
            let off = match al.counter_align {
                LayoutAlign::Center => (line_cross[li] - c) / 2.0,
                LayoutAlign::End => line_cross[li] - c,
                _ => 0.0,
            };
            let pos = ax.vec(cursor, cross_cursor + off);
            let size = ax.vec(m, c);
            rects[*i] = Rect::new(pos.x, pos.y, size.x, size.y);
            cursor += m + gap;
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
