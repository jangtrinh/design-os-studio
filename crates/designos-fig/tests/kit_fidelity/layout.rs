//! Auto layout: every auto-layout frame is solved again against the imported state. Each flow child must land within
//! 0.5 px of the transform and size the file stored (placement), and on every axis the frame hugs, the frame's own
//! solved size must be within 0.5 px of its stored size. Content-sized children (hugging frames, groups, auto-size
//! text) enter their parent's solve at their stored size, so only the frame-size check can catch a wrong hug size.
//!
//! Frames inside an instance whose stored layout hydration refused are left out: their imported state is Studio's
//! own layout, not Figma's, so a solve would be compared with itself. The instances part scores those sublayers
//! against the layout Figma stored for them.

use super::board::{broken_comparator, Part};
use super::keys::NodeKeys;
use designos_doc::*;
use std::collections::HashSet;

pub const TOL: f32 = 0.5;

pub type Solved = (Vec2, Vec<(NodeId, Rect)>);

pub fn bounds(wt: Affine, size: Vec2) -> [f32; 4] {
    let c = [wt.apply(Vec2::new(0.0, 0.0)), wt.apply(Vec2::new(size.x, 0.0)), wt.apply(Vec2::new(0.0, size.y)), wt.apply(Vec2::new(size.x, size.y))];
    let (mut x0, mut y0, mut x1, mut y1) = (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for p in c {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    [x0, y0, x1, y1]
}

pub fn edge_diff(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    (a[0] - b[0]).abs().max((a[1] - b[1]).abs()).max((a[2] - b[2]).abs()).max((a[3] - b[3]).abs())
}

pub fn auto_layout_of(doc: &Document, id: NodeId) -> Option<AutoLayout> {
    let n = doc.get(id)?;
    resolve_node(doc, n).kind.frame_props()?.auto_layout.clone()
}

/// The census's solve of one frame against the current document state.
pub fn solve(doc: &Document, fid: NodeId) -> Option<Solved> {
    let root = doc.get(fid)?;
    let resolved = resolve_node(doc, root);
    let al = resolved.kind.frame_props()?.auto_layout.clone()?;
    if root.children.is_empty() {
        return None;
    }
    let own = layout_solver::SizeLimits::from(&root.layout_child);
    let stroke = resolved.stroke.clone();
    let horiz = al.direction != LayoutDirection::Vertical;
    let (ids, mut kids): (Vec<NodeId>, Vec<layout_solver::ChildIn>) =
        root.children.iter().filter_map(|c| doc.flow_child_input(*c, horiz).map(|k| (*c, k))).unzip();
    let measure = designos_doc::text_measure::layout_measurer();
    let out = layout_solver::compute_then_refit_wrapped(&al, root.size, own, &stroke, &ids, &mut kids, |id, w| {
        let m = measure.as_deref()?;
        let resolved = resolve_node(doc, doc.get(id)?);
        designos_doc::text_measure::height_resize_text_height(&resolved, w, m)
    });
    Some((out.frame, ids.into_iter().zip(out.rects).collect()))
}

pub fn auto_frames(doc: &Document) -> Vec<NodeId> {
    let mut ids: Vec<NodeId> = doc.nodes.keys().copied().filter(|&id| auto_layout_of(doc, id).is_some()).collect();
    ids.sort_by_key(|i| i.0);
    ids
}

fn sizing(s: Sizing) -> &'static str {
    match s {
        Sizing::Fixed => "fixed",
        Sizing::Hug => "hug",
        Sizing::Fill => "fill",
    }
}

fn child_buckets(al: &AutoLayout, c: &Node) -> Vec<String> {
    let mut b = vec!["child placement: total".to_string()];
    b.push(match al.direction {
        LayoutDirection::Horizontal => "direction: horizontal".into(),
        LayoutDirection::Vertical => "direction: vertical".into(),
        LayoutDirection::Grid => "grid".into(),
    });
    if al.direction != LayoutDirection::Grid {
        b.push(if al.primary_align == LayoutAlign::SpaceBetween { "spacing: space-between".into() } else { "spacing: gap".into() });
        b.push(if al.wrap { "wrap: yes".into() } else { "wrap: no".into() });
    }
    b.push(if al.padding.iter().any(|p| *p != 0.0) { "padding: set".into() } else { "padding: none".into() });
    let lc = &c.layout_child;
    let fill_w = lc.horizontal == Sizing::Fill || (al.direction == LayoutDirection::Horizontal && lc.grow > 0.0);
    let fill_h = lc.vertical == Sizing::Fill || (al.direction == LayoutDirection::Vertical && lc.grow > 0.0);
    b.push(format!("child placement: width {}", if fill_w { "fill" } else { sizing(lc.horizontal) }));
    b.push(format!("child placement: height {}", if fill_h { "fill" } else { sizing(lc.vertical) }));
    if lc.min_w.is_some() || lc.max_w.is_some() || lc.min_h.is_some() || lc.max_h.is_some() {
        b.push("min/max: set".into());
    }
    if al.counter_align == LayoutAlign::Baseline || lc.align_self == AlignSelf::Baseline {
        b.push("baseline".into());
    }
    if matches!(c.kind, NodeKind::Text(_)) {
        b.push("text child".into());
    }
    let t = c.transform;
    if t.b.abs() > 1e-3 || t.c.abs() > 1e-3 || t.a < 0.0 || t.d < 0.0 {
        b.push("rotated child".into());
    }
    b
}

/// Top-level instances that carry Figma's stored layout (`derivedSymbolData`, guids in `derived`) but hold no record of
/// an imported layout after hydration: their sublayers carry Studio's own layout.
pub fn refused_holders(doc: &Document, keys: &NodeKeys, derived: &HashSet<String>) -> HashSet<NodeId> {
    derived.iter().filter_map(|g| keys.by_source.get(g).copied()).filter(|id| doc.get(*id).is_some_and(|n| n.component.imported_layout.is_none())).collect()
}

fn under(doc: &Document, mut id: NodeId, set: &HashSet<NodeId>) -> bool {
    loop {
        if set.contains(&id) {
            return true;
        }
        match doc.get(id).and_then(|n| n.parent) {
            Some(p) => id = p,
            None => return false,
        }
    }
}

const REFUSED: &str = "inside an instance whose stored layout hydration refused";
const REFUSED_WHY: &str = "the imported state there is Studio's own layout, not Figma's; the instances part scores it";

/// Scores each flow child of each auto-layout frame (placement) and each frame's size on the axes it hugs, on the
/// document, solving one frame at a time.
pub fn part(doc: &Document, keys: &NodeKeys, refused: &HashSet<NodeId>) -> Part {
    part_with(doc, keys, refused, "auto_layout", "Auto layout")
}

/// The same scoring with the app's text measurer installed (`RenderTextMeasure`, which the app installs for every
/// relayout), on its own part id so the two rows keep separate item lists for `SCOREBOARD_PREV`. The measurer is
/// installed for this thread only and removed when the scoring ends.
pub fn measurer_part(doc: &Document, keys: &NodeKeys, refused: &HashSet<NodeId>) -> Part {
    let _measurer = designos_doc::text_measure::ThreadLayoutMeasurerGuard::install(std::sync::Arc::new(designos_render::RenderTextMeasure));
    part_with(doc, keys, refused, "auto_layout_measurer", "Auto layout, text measurer installed")
}

fn part_with(doc: &Document, keys: &NodeKeys, refused: &HashSet<NodeId>, id: &'static str, title: &'static str) -> Part {
    let mut p = Part::new(
        id,
        title,
        "each flow child within 0.5 px of its stored transform and size, and each hugging frame within 0.5 px of its stored size on the axes it hugs, after the frame is solved again",
    );
    let tol = if broken_comparator() { f32::INFINITY } else { TOL };
    for fid in auto_frames(doc) {
        if !keys.key.contains_key(&fid) {
            continue;
        }
        let Some(al) = auto_layout_of(doc, fid) else { continue };
        let Some(n) = doc.get(fid) else { continue };
        let inside_refused = under(doc, fid, refused);
        for &cid in &n.children {
            if doc.get(cid).is_some_and(|c| c.layout_child.absolute) {
                p.unscored("absolute child", "not placed by the flow solve, so a solve can never move it");
            }
        }
        let Some((frame, rects)) = solve(doc, fid) else { continue };
        let (hug_w, hug_h) = layout_solver::hugged_axes(&al);
        for (axis, hugs, solved, stored) in [("width", hug_w, frame.x, n.size.x), ("height", hug_h, frame.y, n.size.y)] {
            if !hugs {
                continue;
            }
            if inside_refused {
                p.unscored(&format!("frame size {REFUSED}"), REFUSED_WHY);
                continue;
            }
            let ok = (solved - stored).abs() <= tol;
            if !ok && !keys.is_hidden(fid) {
                p.detail(&format!("failing hug {axis} (Studio minus Figma)"), &format!("{:+.1}", ((solved - stored) * 2.0).round() / 2.0));
            }
            p.record(
                format!("{}#hug-{axis}", keys.key_of(fid)),
                keys.is_hidden(fid),
                ok,
                &[format!("frame size: hug {axis}"), "frame size: total".to_string()],
            );
        }
        for (cid, r) in rects {
            let Some(c) = doc.get(cid) else { continue };
            if inside_refused {
                p.unscored(&format!("child {REFUSED}"), REFUSED_WHY);
                continue;
            }
            let w = designos_doc::slot_write(&c.transform, c.size, r);
            let d = edge_diff(&bounds(c.transform, c.size), &bounds(w.transform, w.size));
            p.record(keys.key_of(cid), keys.is_hidden(cid), d <= tol, &child_buckets(&al, c));
        }
    }
    let measured = designos_doc::text_measure::layout_measurer().is_some();
    p.note(format!("text measurer installed: {measured}"));
    if measured {
        p.note("the app's measurer rounds a text's width and height up to whole pixels (measure_text_box), so part of any drop against Figma's fractional sizes is that ceiling, not shaping");
    }
    p
}
