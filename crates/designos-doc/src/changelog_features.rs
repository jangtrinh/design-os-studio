//! Figma Changelog 4 Feature Pillars (`FP-01` .. `FP-04`):
//!
//! 1. **Pillar 1 (`FP-01`: Suggest Auto-Layout `Cmd+Shift+A` / `Ctrl+Shift+A`)**:
//!    Spatial 1D/2D clustering of selected unstacked sibling nodes into a
//!    `Horizontal` row, `Vertical` column, or nested 2D `Column-of-Rows`
//!    Auto-Layout 5.0 hierarchy with automatic gap and padding inference.
//! 2. **Pillar 2 (`FP-02`: Live Non-Destructive Boolean Groups)**:
//!    Non-destructive `Union`, `Subtract`, `Intersect`, and `Exclude` boolean
//!    containers that preserve editable child shapes in the scene tree and
//!    dynamically recompute composite `fill_geometry` (`Vec<Path>`) and tight
//!    bounding boxes whenever any child moves or resizes.
//! 3. **Pillar 3 (`FP-03`: Variable Bindings on Numeric Geometry/Text & Subtree Mode Switching)**:
//!    Typed bindings for `Width`, `Height`, `PaddingTop/Right/Bottom/Left`,
//!    `ItemSpacing`, `CornerRadius`, `Opacity`, `Visible`, `TextContent`, and
//!    fills/strokes, plus [`apply_variable_mode_to_subtree`] with automatic
//!    [`relayout_subtree`].
//! 4. **Pillar 4 (`FP-04`: Interactive Prototype Runtime)**:
//!    Stateful [`PrototypeRuntime`] supporting [`OverflowScrollMode`] (`None`,
//!    `Horizontal`, `Vertical`, `Both`) with viewport scroll clamping,
//!    interactive component variant swapping ([`PrototypeRuntime::trigger_variant_interaction`]),
//!    and conditional variable evaluation (`Action::SetVariable`,
//!    `Action::SetVariableMode`, `Action::Conditional`).

use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ===========================================================================
// PILLAR 1 (FP-01): SUGGEST AUTO-LAYOUT & AUTO-LAYOUT 5.0 RELAYOUT ENGINE
// ===========================================================================

/// Topology inferred by [`suggest_auto_layout`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuggestedLayoutKind {
    /// Single horizontal row of sibling nodes (`LayoutDirection::Horizontal`).
    HorizontalRow,
    /// Single vertical column of sibling nodes (`LayoutDirection::Vertical`).
    VerticalColumn,
    /// 2D multi-row layout clustered into an outer `Vertical` column of inner
    /// `Horizontal` row frames.
    NestedColumnOfRows { row_count: usize },
}

/// Summary returned by [`suggest_auto_layout`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SuggestAutoLayoutResult {
    /// Outer Auto-Layout frame created (or configured in-place).
    pub root_frame: NodeId,
    /// Inner horizontal row frames created when `kind` is
    /// [`SuggestedLayoutKind::NestedColumnOfRows`].
    pub created_row_frames: Vec<NodeId>,
    /// Inferred layout topology.
    pub kind: SuggestedLayoutKind,
    /// Inferred primary-axis item spacing on `root_frame`.
    pub primary_spacing: f32,
    /// Inferred `[top, right, bottom, left]` padding on `root_frame`.
    pub padding: [f32; 4],
}

#[derive(Clone, Copy, Debug)]
struct ItemBox {
    id: NodeId,
    rect: Rect,
}

fn node_parent_rect(n: &Node) -> Rect {
    let t = n.transform.translation();
    Rect::new(t.x, t.y, n.size.x.max(0.0), n.size.y.max(0.0))
}

fn union_rects(rects: &[Rect]) -> Rect {
    let Some(first) = rects.first().copied() else {
        return Rect::default();
    };
    rects.iter().skip(1).fold(first, |acc, r| acc.union(r))
}

fn median_rounded_gap(mut gaps: Vec<f32>) -> f32 {
    if gaps.is_empty() {
        return 0.0;
    }
    gaps.sort_by(|a, b| a.total_cmp(b));
    let mid = gaps.len() / 2;
    let raw = if gaps.len() % 2 == 1 { gaps[mid] } else { (gaps[mid - 1] + gaps[mid]) * 0.5 };
    raw.max(0.0).round()
}

fn infer_horizontal_gap(items: &[ItemBox]) -> f32 {
    if items.len() < 2 {
        return 0.0;
    }
    let mut gaps = Vec::with_capacity(items.len() - 1);
    for pair in items.windows(2) {
        let gap = (pair[1].rect.x - (pair[0].rect.x + pair[0].rect.w)).max(0.0);
        gaps.push(gap);
    }
    median_rounded_gap(gaps)
}

fn infer_vertical_gap(rects: &[Rect]) -> f32 {
    if rects.len() < 2 {
        return 0.0;
    }
    let mut gaps = Vec::with_capacity(rects.len() - 1);
    for pair in rects.windows(2) {
        let gap = (pair[1].y - (pair[0].y + pair[0].h)).max(0.0);
        gaps.push(gap);
    }
    median_rounded_gap(gaps)
}

fn infer_row_counter_align(items: &[ItemBox]) -> LayoutAlign {
    if items.len() < 2 {
        return LayoutAlign::Start;
    }
    let first_cy = items[0].rect.y + items[0].rect.h * 0.5;
    if items.iter().all(|it| ((it.rect.y + it.rect.h * 0.5) - first_cy).abs() <= 2.0) {
        return LayoutAlign::Center;
    }
    let first_bot = items[0].rect.y + items[0].rect.h;
    if items.iter().all(|it| ((it.rect.y + it.rect.h) - first_bot).abs() <= 2.0) {
        return LayoutAlign::End;
    }
    LayoutAlign::Start
}

fn infer_col_counter_align(items: &[ItemBox]) -> LayoutAlign {
    if items.len() < 2 {
        return LayoutAlign::Start;
    }
    let first_cx = items[0].rect.x + items[0].rect.w * 0.5;
    if items.iter().all(|it| ((it.rect.x + it.rect.w * 0.5) - first_cx).abs() <= 2.0) {
        return LayoutAlign::Center;
    }
    let first_right = items[0].rect.x + items[0].rect.w;
    if items.iter().all(|it| ((it.rect.x + it.rect.w) - first_right).abs() <= 2.0) {
        return LayoutAlign::End;
    }
    LayoutAlign::Start
}

/// Partitions `items` into horizontal row bands based on vertical interval overlap.
/// Each returned row band has its items sorted left-to-right by `x`.
fn cluster_into_row_bands(mut items: Vec<ItemBox>) -> Vec<Vec<ItemBox>> {
    items.sort_by(|a, b| a.rect.y.total_cmp(&b.rect.y).then_with(|| a.rect.x.total_cmp(&b.rect.x)));
    let mut bands: Vec<(Rect, Vec<ItemBox>)> = Vec::new();
    for item in items {
        let mut matched = None;
        for (idx, (band_rect, _)) in bands.iter().enumerate() {
            let overlap_top = band_rect.y.max(item.rect.y);
            let overlap_bot = (band_rect.y + band_rect.h).min(item.rect.y + item.rect.h);
            let overlap_h = (overlap_bot - overlap_top).max(0.0);
            let min_h = band_rect.h.min(item.rect.h).max(1.0);
            let item_cy = item.rect.y + item.rect.h * 0.5;
            let center_in_band = item_cy >= band_rect.y && item_cy <= band_rect.y + band_rect.h;
            if overlap_h >= 0.30 * min_h || center_in_band {
                matched = Some(idx);
                break;
            }
        }
        if let Some(idx) = matched {
            bands[idx].0 = bands[idx].0.union(&item.rect);
            bands[idx].1.push(item);
        } else {
            bands.push((item.rect, vec![item]));
        }
    }
    bands.sort_by(|a, b| a.0.y.total_cmp(&b.0.y));
    bands
        .into_iter()
        .map(|(_, mut row)| {
            row.sort_by(|a, b| a.rect.x.total_cmp(&b.rect.x).then_with(|| a.rect.y.total_cmp(&b.rect.y)));
            row
        })
        .collect()
}

/// Infers 1D (`HorizontalRow` / `VerticalColumn`) or 2D (`NestedColumnOfRows`)
/// Auto-Layout 5.0 structure from `selected_ids` (`Cmd+Shift+A` / `Ctrl+Shift+A`).
///
/// - If `selected_ids` contains a single Frame/Component that already has children,
///   its children are clustered in-place, inferring padding from the frame's
///   current bounds relative to its children's union box.
/// - If `selected_ids` contains sibling nodes sharing a parent container, they
///   are wrapped into a new Auto-Layout frame (and inner row frames when a 2D
///   multi-row arrangement is detected).
pub fn suggest_auto_layout(doc: &mut Document, selected_ids: &[NodeId]) -> Result<SuggestAutoLayoutResult, DocError> {
    if selected_ids.is_empty() {
        return Err(DocError::NotFound(NodeId(0)));
    }
    for &id in selected_ids {
        if doc.get(id).is_none() {
            return Err(DocError::NotFound(id));
        }
    }

    // Case A: Single container Frame/Component selected with existing children -> convert in place.
    let (parent_id, target_frame_opt, child_ids, inferred_padding) = if selected_ids.len() == 1 {
        let only = selected_ids[0];
        let node = doc.get(only).ok_or(DocError::NotFound(only))?;
        if node.kind.frame_props().is_some() && !node.children.is_empty() {
            let kids: Vec<NodeId> = node.children.iter().copied().filter(|&cid| doc.get(cid).is_some_and(|c| c.visible)).collect();
            let kid_rects: Vec<Rect> = kids.iter().filter_map(|&cid| doc.get(cid).map(node_parent_rect)).collect();
            let u = union_rects(&kid_rects);
            let pad_top = u.y.max(0.0).round();
            let pad_left = u.x.max(0.0).round();
            let pad_right = (node.size.x - (u.x + u.w)).max(0.0).round();
            let pad_bottom = (node.size.y - (u.y + u.h)).max(0.0).round();
            (only, Some(only), kids, [pad_top, pad_right, pad_bottom, pad_left])
        } else {
            let parent = node.parent.ok_or(DocError::NotAContainer(only))?;
            (parent, None, vec![only], [0.0; 4])
        }
    } else {
        let first_parent = doc.get(selected_ids[0]).and_then(|n| n.parent).ok_or(DocError::NotAContainer(selected_ids[0]))?;
        for &id in &selected_ids[1..] {
            if doc.get(id).and_then(|n| n.parent) != Some(first_parent) {
                return Err(DocError::NotAContainer(id));
            }
        }
        (first_parent, None, selected_ids.to_vec(), [0.0; 4])
    };

    let items: Vec<ItemBox> = child_ids.iter().filter_map(|&id| doc.get(id).map(|n| ItemBox { id, rect: node_parent_rect(n) })).collect();
    if items.is_empty() {
        return Err(DocError::NotFound(selected_ids[0]));
    }

    let all_rects: Vec<Rect> = items.iter().map(|it| it.rect).collect();
    let total_union = union_rects(&all_rects);
    let bands = cluster_into_row_bands(items);

    let insert_index = if let Some(existing_frame) = target_frame_opt {
        doc.get(existing_frame).map(|_| 0).unwrap_or(0)
    } else {
        doc.get(parent_id).map(|p| p.children.iter().position(|c| child_ids.contains(c)).unwrap_or(p.children.len())).unwrap_or(0)
    };

    let root_frame = if let Some(existing) = target_frame_opt {
        existing
    } else {
        let fid = doc.alloc_id();
        let mut frame = Node::frame(fid, total_union);
        frame.name = "Suggested Auto Layout".into();
        frame.fills.clear();
        doc.insert(parent_id, frame)?;
        let cur_len = doc.get(parent_id).map(|p| p.children.len()).unwrap_or(1);
        doc.reparent(fid, parent_id, insert_index.min(cur_len.saturating_sub(1)))?;
        fid
    };

    let mut created_row_frames = Vec::new();
    let (kind, primary_spacing, counter_align) = if bands.len() == 1 {
        // 1D Horizontal Row
        let row = &bands[0];
        let gap = infer_horizontal_gap(row);
        let align = infer_row_counter_align(row);
        for (idx, it) in row.iter().enumerate() {
            doc.reparent(it.id, root_frame, idx)?;
        }
        (SuggestedLayoutKind::HorizontalRow, gap, align)
    } else if bands.iter().all(|b| b.len() == 1) {
        // 1D Vertical Column
        let col_items: Vec<ItemBox> = bands.into_iter().map(|mut b| b.remove(0)).collect();
        let col_rects: Vec<Rect> = col_items.iter().map(|it| it.rect).collect();
        let gap = infer_vertical_gap(&col_rects);
        let align = infer_col_counter_align(&col_items);
        for (idx, it) in col_items.iter().enumerate() {
            doc.reparent(it.id, root_frame, idx)?;
        }
        (SuggestedLayoutKind::VerticalColumn, gap, align)
    } else {
        // 2D Nested Column-of-Rows
        let row_count = bands.len();
        let mut row_rects = Vec::with_capacity(row_count);
        for (row_idx, row) in bands.into_iter().enumerate() {
            let r_rects: Vec<Rect> = row.iter().map(|it| it.rect).collect();
            let r_union = union_rects(&r_rects);
            row_rects.push(r_union);
            if row.len() == 1 {
                doc.reparent(row[0].id, root_frame, row_idx)?;
            } else {
                let row_id = doc.alloc_id();
                let row_gap = infer_horizontal_gap(&row);
                let row_align = infer_row_counter_align(&row);
                let mut row_node = Node::frame(row_id, r_union);
                row_node.name = format!("Row {}", row_idx + 1);
                row_node.fills.clear();
                if let Some(fp) = row_node.kind.frame_props_mut() {
                    fp.auto_layout = Some(AutoLayout {
                        direction: LayoutDirection::Horizontal,
                        item_spacing: row_gap,
                        counter_align: row_align,
                        primary_sizing: Sizing::Hug,
                        counter_sizing: Sizing::Hug,
                        ..AutoLayout::v1()
                    });
                }
                doc.insert(root_frame, row_node)?;
                doc.reparent(row_id, root_frame, row_idx)?;
                for (c_idx, it) in row.iter().enumerate() {
                    doc.reparent(it.id, row_id, c_idx)?;
                }
                created_row_frames.push(row_id);
            }
        }
        let v_gap = infer_vertical_gap(&row_rects);
        (SuggestedLayoutKind::NestedColumnOfRows { row_count }, v_gap, LayoutAlign::Start)
    };

    let direction = match kind {
        SuggestedLayoutKind::HorizontalRow => LayoutDirection::Horizontal,
        SuggestedLayoutKind::VerticalColumn | SuggestedLayoutKind::NestedColumnOfRows { .. } => LayoutDirection::Vertical,
    };

    if let Some(root_node) = doc.get_mut(root_frame) {
        let sizing = if target_frame_opt.is_some() { Sizing::Fixed } else { Sizing::Hug };
        if let Some(fp) = root_node.kind.frame_props_mut() {
            fp.auto_layout = Some(AutoLayout {
                direction,
                item_spacing: primary_spacing,
                padding: inferred_padding,
                primary_align: LayoutAlign::Start,
                counter_align,
                primary_sizing: sizing,
                counter_sizing: sizing,
                ..AutoLayout::v1()
            });
        }
    }

    relayout_subtree(doc, root_frame);

    Ok(SuggestAutoLayoutResult { root_frame, created_row_frames, kind, primary_spacing, padding: inferred_padding })
}

/// Recomputes auto-layout child positions and `Hug`/`Fill` dimensions across `root_id` and its
/// descendants. Same as [`Document::relayout_subtree`], which runs the one shared solver
/// (`layout_solver`).
pub fn relayout_subtree(doc: &mut Document, root_id: NodeId) {
    doc.relayout_subtree(root_id);
}

// ===========================================================================
// PILLAR 2 (FP-02): LIVE NON-DESTRUCTIVE BOOLEAN GROUPS
// ===========================================================================

const BOOL_EPS: f32 = 1e-4;

fn pt_dist(a: Vec2, b: Vec2) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

/// Samples a node's local shape into one or more closed polygon contours in
/// the node's parent coordinate space.
fn node_contours_in_parent(doc: &Document, node_id: NodeId) -> Vec<Vec<Vec2>> {
    let Some(n) = doc.get(node_id) else { return Vec::new() };
    if !n.visible {
        return Vec::new();
    }
    let w = n.size.x.max(0.0);
    let h = n.size.y.max(0.0);
    let local_contours: Vec<Vec<Vec2>> = match &n.kind {
        NodeKind::BooleanOp(_) => {
            if let Some(paths) = &n.fill_geometry {
                paths_to_contours(paths)
            } else {
                vec![vec![Vec2::new(0.0, 0.0), Vec2::new(w, 0.0), Vec2::new(w, h), Vec2::new(0.0, h)]]
            }
        }
        NodeKind::Vector { paths, .. } => {
            let cs = paths_to_contours(paths);
            if cs.is_empty() && w > 0.0 && h > 0.0 {
                vec![vec![Vec2::new(0.0, 0.0), Vec2::new(w, 0.0), Vec2::new(w, h), Vec2::new(0.0, h)]]
            } else {
                cs
            }
        }
        NodeKind::Ellipse(_) => {
            let steps = 24usize;
            let cx = w * 0.5;
            let cy = h * 0.5;
            let rx = w * 0.5;
            let ry = h * 0.5;
            let mut poly = Vec::with_capacity(steps);
            for i in 0..steps {
                let theta = (i as f32) * std::f32::consts::TAU / (steps as f32);
                poly.push(Vec2::new(cx + rx * theta.cos(), cy + ry * theta.sin()));
            }
            vec![poly]
        }
        NodeKind::Polygon { count } => {
            let sides = (*count).max(3) as usize;
            let cx = w * 0.5;
            let cy = h * 0.5;
            let rx = w * 0.5;
            let ry = h * 0.5;
            let mut poly = Vec::with_capacity(sides);
            for i in 0..sides {
                let theta = -std::f32::consts::FRAC_PI_2 + (i as f32) * std::f32::consts::TAU / (sides as f32);
                poly.push(Vec2::new(cx + rx * theta.cos(), cy + ry * theta.sin()));
            }
            vec![poly]
        }
        NodeKind::Star { count, inner_ratio } => {
            let pts = ((*count).max(3) as usize) * 2;
            let ratio = inner_ratio.clamp(0.05, 1.0);
            let cx = w * 0.5;
            let cy = h * 0.5;
            let rx = w * 0.5;
            let ry = h * 0.5;
            let mut poly = Vec::with_capacity(pts);
            for i in 0..pts {
                let r_scale = if i % 2 == 0 { 1.0 } else { ratio };
                let theta = -std::f32::consts::FRAC_PI_2 + (i as f32) * std::f32::consts::PI / ((*count).max(3) as f32);
                poly.push(Vec2::new(cx + rx * r_scale * theta.cos(), cy + ry * r_scale * theta.sin()));
            }
            vec![poly]
        }
        _ => {
            if w <= 0.0 || h <= 0.0 {
                Vec::new()
            } else {
                vec![vec![Vec2::new(0.0, 0.0), Vec2::new(w, 0.0), Vec2::new(w, h), Vec2::new(0.0, h)]]
            }
        }
    };

    local_contours.into_iter().map(|c| c.into_iter().map(|p| n.transform.apply(p)).collect()).collect()
}

fn paths_to_contours(paths: &[Path]) -> Vec<Vec<Vec2>> {
    let mut out = Vec::new();
    for path in paths {
        let mut cur = Vec::new();
        for cmd in &path.cmds {
            match *cmd {
                PathCmd::MoveTo(p) => {
                    if cur.len() >= 3 {
                        out.push(std::mem::take(&mut cur));
                    } else {
                        cur.clear();
                    }
                    cur.push(p);
                }
                PathCmd::LineTo(p) => {
                    if cur.last().is_none_or(|&last| pt_dist(last, p) > BOOL_EPS) {
                        cur.push(p);
                    }
                }
                PathCmd::QuadTo(c, p) => {
                    let p0 = cur.last().copied().unwrap_or(p);
                    for step in 1..=4 {
                        let t = (step as f32) / 4.0;
                        let mt = 1.0 - t;
                        let q = Vec2::new(mt * mt * p0.x + 2.0 * mt * t * c.x + t * t * p.x, mt * mt * p0.y + 2.0 * mt * t * c.y + t * t * p.y);
                        cur.push(q);
                    }
                }
                PathCmd::CubicTo(c1, c2, p) => {
                    let p0 = cur.last().copied().unwrap_or(p);
                    for step in 1..=6 {
                        let t = (step as f32) / 6.0;
                        let mt = 1.0 - t;
                        let q = Vec2::new(
                            mt * mt * mt * p0.x + 3.0 * mt * mt * t * c1.x + 3.0 * mt * t * t * c2.x + t * t * t * p.x,
                            mt * mt * mt * p0.y + 3.0 * mt * mt * t * c1.y + 3.0 * mt * t * t * c2.y + t * t * t * p.y,
                        );
                        cur.push(q);
                    }
                }
                PathCmd::Close => {
                    if let (Some(&first), Some(&last)) = (cur.first(), cur.last()) {
                        if pt_dist(first, last) <= BOOL_EPS {
                            cur.pop();
                        }
                    }
                    if cur.len() >= 3 {
                        out.push(std::mem::take(&mut cur));
                    } else {
                        cur.clear();
                    }
                }
            }
        }
        if cur.len() >= 3 {
            out.push(cur);
        }
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PointRel {
    Inside,
    Outside,
    BoundarySameDir,
    BoundaryOppDir,
}

fn classify_segment_midpoint(a: Vec2, b: Vec2, contours: &[Vec<Vec2>]) -> PointRel {
    let mid = Vec2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
    let seg_dx = b.x - a.x;
    let seg_dy = b.y - a.y;

    // 1. Check if `mid` lies directly on any edge of `contours`.
    for poly in contours {
        let m = poly.len();
        if m < 3 {
            continue;
        }
        for i in 0..m {
            let p0 = poly[i];
            let p1 = poly[(i + 1) % m];
            let edx = p1.x - p0.x;
            let edy = p1.y - p0.y;
            let len_sq = edx * edx + edy * edy;
            if len_sq < BOOL_EPS * BOOL_EPS {
                continue;
            }
            let t = ((mid.x - p0.x) * edx + (mid.y - p0.y) * edy) / len_sq;
            if (0.0..=1.0).contains(&t) {
                let proj = Vec2::new(p0.x + t * edx, p0.y + t * edy);
                if pt_dist(mid, proj) <= 1e-3 {
                    let dot = seg_dx * edx + seg_dy * edy;
                    return if dot >= 0.0 { PointRel::BoundarySameDir } else { PointRel::BoundaryOppDir };
                }
            }
        }
    }

    // 2. Even-odd ray casting to the right from `mid`.
    let mut inside = false;
    for poly in contours {
        let m = poly.len();
        if m < 3 {
            continue;
        }
        for i in 0..m {
            let p0 = poly[i];
            let p1 = poly[(i + 1) % m];
            if (p0.y > mid.y) != (p1.y > mid.y) {
                let x_hit = p0.x + (mid.y - p0.y) * (p1.x - p0.x) / (p1.y - p0.y);
                if x_hit > mid.x {
                    inside = !inside;
                }
            }
        }
    }
    if inside {
        PointRel::Inside
    } else {
        PointRel::Outside
    }
}

fn segment_intersections(a0: Vec2, a1: Vec2, b0: Vec2, b1: Vec2) -> Vec<f32> {
    let r = Vec2::new(a1.x - a0.x, a1.y - a0.y);
    let s = Vec2::new(b1.x - b0.x, b1.y - b0.y);
    let rxs = r.x * s.y - r.y * s.x;
    let qmp = Vec2::new(b0.x - a0.x, b0.y - a0.y);

    if rxs.abs() < 1e-6 {
        // Collinear or parallel: project b0 and b1 onto segment a0..a1 if collinear.
        let cross = qmp.x * r.y - qmp.y * r.x;
        let r_len_sq = r.x * r.x + r.y * r.y;
        if cross.abs() < 1e-4 && r_len_sq > BOOL_EPS {
            let t0 = (qmp.x * r.x + qmp.y * r.y) / r_len_sq;
            let qmp1 = Vec2::new(b1.x - a0.x, b1.y - a0.y);
            let t1 = (qmp1.x * r.x + qmp1.y * r.y) / r_len_sq;
            let mut ts = Vec::new();
            if t0 > BOOL_EPS && t0 < 1.0 - BOOL_EPS {
                ts.push(t0);
            }
            if t1 > BOOL_EPS && t1 < 1.0 - BOOL_EPS {
                ts.push(t1);
            }
            return ts;
        }
        return Vec::new();
    }

    let t = (qmp.x * s.y - qmp.y * s.x) / rxs;
    let u = (qmp.x * r.y - qmp.y * r.x) / rxs;
    if (-BOOL_EPS..=1.0 + BOOL_EPS).contains(&t) && (-BOOL_EPS..=1.0 + BOOL_EPS).contains(&u) {
        let tc = t.clamp(0.0, 1.0);
        if tc > BOOL_EPS && tc < 1.0 - BOOL_EPS {
            return vec![tc];
        }
    }
    Vec::new()
}

fn subdivide_contours(subject: &[Vec<Vec2>], clip: &[Vec<Vec2>]) -> Vec<(Vec2, Vec2)> {
    let mut sub_edges = Vec::new();
    for poly in subject {
        let m = poly.len();
        if m < 3 {
            continue;
        }
        for i in 0..m {
            let a0 = poly[i];
            let a1 = poly[(i + 1) % m];
            if pt_dist(a0, a1) <= BOOL_EPS {
                continue;
            }
            let mut ts = vec![0.0f32, 1.0f32];
            for cpoly in clip {
                let cm = cpoly.len();
                if cm < 3 {
                    continue;
                }
                for j in 0..cm {
                    let b0 = cpoly[j];
                    let b1 = cpoly[(j + 1) % cm];
                    ts.extend(segment_intersections(a0, a1, b0, b1));
                }
            }
            ts.sort_by(|x, y| x.total_cmp(y));
            ts.dedup_by(|x, y| (*x - *y).abs() < 1e-4);
            for pair in ts.windows(2) {
                let p0 = Vec2::new(a0.x + pair[0] * (a1.x - a0.x), a0.y + pair[0] * (a1.y - a0.y));
                let p1 = Vec2::new(a0.x + pair[1] * (a1.x - a0.x), a0.y + pair[1] * (a1.y - a0.y));
                if pt_dist(p0, p1) > BOOL_EPS {
                    sub_edges.push((p0, p1));
                }
            }
        }
    }
    sub_edges
}

fn stitch_segments_to_contours(mut segs: Vec<(Vec2, Vec2)>) -> Vec<Vec<Vec2>> {
    let mut contours = Vec::new();
    while let Some((start, mut cur)) = segs.pop() {
        let mut poly = vec![start, cur];
        loop {
            if pt_dist(cur, start) <= 1e-3 {
                poly.pop();
                break;
            }
            let next_idx = segs
                .iter()
                .enumerate()
                .filter_map(|(i, (s, _))| {
                    let d = pt_dist(cur, *s);
                    (d <= 1e-2).then_some((i, d))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(i, _)| i);
            if let Some(idx) = next_idx {
                let (_, next_end) = segs.swap_remove(idx);
                cur = next_end;
                poly.push(cur);
            } else {
                break;
            }
        }
        if poly.len() >= 3 {
            contours.push(poly);
        }
    }
    contours
}

fn boolean_clip_pair(op: BooleanOp, a: &[Vec<Vec2>], b: &[Vec<Vec2>]) -> Vec<Vec<Vec2>> {
    if a.is_empty() {
        return match op {
            BooleanOp::Union | BooleanOp::Exclude => b.to_vec(),
            BooleanOp::Subtract | BooleanOp::Intersect => Vec::new(),
        };
    }
    if b.is_empty() {
        return match op {
            BooleanOp::Union | BooleanOp::Subtract | BooleanOp::Exclude => a.to_vec(),
            BooleanOp::Intersect => Vec::new(),
        };
    }

    let a_segs = subdivide_contours(a, b);
    let b_segs = subdivide_contours(b, a);
    let mut kept: Vec<(Vec2, Vec2)> = Vec::new();

    match op {
        BooleanOp::Union => {
            for (p0, p1) in a_segs {
                if matches!(classify_segment_midpoint(p0, p1, b), PointRel::Outside | PointRel::BoundarySameDir) {
                    kept.push((p0, p1));
                }
            }
            for (p0, p1) in b_segs {
                if matches!(classify_segment_midpoint(p0, p1, a), PointRel::Outside) {
                    kept.push((p0, p1));
                }
            }
        }
        BooleanOp::Intersect => {
            for (p0, p1) in a_segs {
                if matches!(classify_segment_midpoint(p0, p1, b), PointRel::Inside | PointRel::BoundarySameDir) {
                    kept.push((p0, p1));
                }
            }
            for (p0, p1) in b_segs {
                if matches!(classify_segment_midpoint(p0, p1, a), PointRel::Inside) {
                    kept.push((p0, p1));
                }
            }
        }
        BooleanOp::Subtract => {
            for (p0, p1) in a_segs {
                if matches!(classify_segment_midpoint(p0, p1, b), PointRel::Outside | PointRel::BoundaryOppDir) {
                    kept.push((p0, p1));
                }
            }
            for (p0, p1) in b_segs {
                if matches!(classify_segment_midpoint(p0, p1, a), PointRel::Inside) {
                    kept.push((p1, p0));
                }
            }
        }
        BooleanOp::Exclude => {
            let a_minus_b = boolean_clip_pair(BooleanOp::Subtract, a, b);
            let b_minus_a = boolean_clip_pair(BooleanOp::Subtract, b, a);
            let mut combined = a_minus_b;
            combined.extend(b_minus_a);
            return combined;
        }
    }

    stitch_segments_to_contours(kept)
}

fn contours_bounds(contours: &[Vec<Vec2>]) -> Option<Rect> {
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    let mut any = false;
    for poly in contours {
        for p in poly {
            any = true;
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }
    }
    any.then(|| Rect::new(min_x, min_y, (max_x - min_x).max(0.0), (max_y - min_y).max(0.0)))
}

fn contours_to_local_paths(contours: &[Vec<Vec2>], origin: Vec2, fill_rule: FillRule) -> Vec<Path> {
    if contours.is_empty() {
        return Vec::new();
    }
    let mut cmds = Vec::new();
    for poly in contours {
        let Some(&first) = poly.first() else { continue };
        cmds.push(PathCmd::MoveTo(Vec2::new(first.x - origin.x, first.y - origin.y)));
        for &pt in &poly[1..] {
            cmds.push(PathCmd::LineTo(Vec2::new(pt.x - origin.x, pt.y - origin.y)));
        }
        cmds.push(PathCmd::Close);
    }
    vec![Path { cmds, fill_rule }]
}

/// Creates a live non-destructive `NodeKind::BooleanOp(op)` group wrapping
/// `child_ids` under their shared parent container, preserving every child node
/// in the scene tree and computing the initial composite bounding box and
/// `fill_geometry`.
pub fn create_boolean_group(doc: &mut Document, op: BooleanOp, child_ids: &[NodeId]) -> Result<NodeId, DocError> {
    if child_ids.len() < 2 {
        return Err(DocError::NotAContainer(child_ids.first().copied().unwrap_or(NodeId(0))));
    }
    let first = doc.get(child_ids[0]).cloned().ok_or(DocError::NotFound(child_ids[0]))?;
    let parent_id = first.parent.ok_or(DocError::NotAContainer(child_ids[0]))?;
    for &cid in &child_ids[1..] {
        let cn = doc.get(cid).ok_or(DocError::NotFound(cid))?;
        if cn.parent != Some(parent_id) {
            return Err(DocError::NotAContainer(cid));
        }
    }

    let insert_idx = doc.get(parent_id).map(|p| p.children.iter().position(|c| child_ids.contains(c)).unwrap_or(p.children.len())).unwrap_or(0);

    let bool_id = doc.alloc_id();
    let mut bool_node = Node::new(bool_id, NodeKind::BooleanOp(op));
    bool_node.name = match op {
        BooleanOp::Union => "Union".into(),
        BooleanOp::Subtract => "Subtract".into(),
        BooleanOp::Intersect => "Intersect".into(),
        BooleanOp::Exclude => "Exclude".into(),
    };
    bool_node.fills = first.fills.clone();
    bool_node.stroke = first.stroke.clone();

    doc.insert(parent_id, bool_node)?;
    let max_idx = doc.get(parent_id).map(|p| p.children.len().saturating_sub(1)).unwrap_or(0);
    doc.reparent(bool_id, parent_id, insert_idx.min(max_idx))?;

    for (idx, &cid) in child_ids.iter().enumerate() {
        doc.reparent(cid, bool_id, idx)?;
    }

    recompute_boolean_group(doc, bool_id)?;
    Ok(bool_id)
}

/// Dynamically recomputes the composite `fill_geometry` (`Vec<Path>`) and tight
/// bounding box of a `NodeKind::BooleanOp` container from its live children,
/// shifting child local transforms so their parent/world positions stay fixed.
pub fn recompute_boolean_group(doc: &mut Document, bool_id: NodeId) -> Result<Rect, DocError> {
    let bool_node = doc.get(bool_id).cloned().ok_or(DocError::NotFound(bool_id))?;
    let NodeKind::BooleanOp(op) = bool_node.kind else {
        return Err(DocError::NotAContainer(bool_id));
    };

    // Recursively recompute any nested BooleanOp children first.
    for &cid in &bool_node.children {
        if matches!(doc.get(cid).map(|n| &n.kind), Some(NodeKind::BooleanOp(_))) {
            let _ = recompute_boolean_group(doc, cid);
        }
    }

    let group_origin = bool_node.transform.translation();
    let visible_children: Vec<NodeId> = bool_node.children.iter().copied().filter(|&cid| doc.get(cid).is_some_and(|n| n.visible)).collect();

    if visible_children.is_empty() {
        if let Some(bn) = doc.get_mut(bool_id) {
            bn.size = Vec2::ZERO;
            bn.fill_geometry = Some(Vec::new());
        }
        return Ok(Rect::new(group_origin.x, group_origin.y, 0.0, 0.0));
    }

    // Collect each visible child's contours in the boolean group's parent space.
    let mut child_contours_parent: Vec<Vec<Vec<Vec2>>> = Vec::with_capacity(visible_children.len());
    for &cid in &visible_children {
        let local_contours = node_contours_in_parent(doc, cid);
        let parent_contours: Vec<Vec<Vec2>> =
            local_contours.into_iter().map(|poly| poly.into_iter().map(|p| Vec2::new(p.x + group_origin.x, p.y + group_origin.y)).collect()).collect();
        child_contours_parent.push(parent_contours);
    }

    let mut acc = child_contours_parent[0].clone();
    for next in &child_contours_parent[1..] {
        acc = boolean_clip_pair(op, &acc, next);
    }

    let new_bounds = contours_bounds(&acc).unwrap_or(Rect::new(group_origin.x, group_origin.y, 0.0, 0.0));
    let delta = Vec2::new(new_bounds.x - group_origin.x, new_bounds.y - group_origin.y);

    // Shift all children's local transforms by `-delta` so they remain at their exact parent-space positions.
    if delta.x.abs() > 1e-6 || delta.y.abs() > 1e-6 {
        for &cid in &bool_node.children {
            if let Some(cn) = doc.get_mut(cid) {
                cn.transform.tx -= delta.x;
                cn.transform.ty -= delta.y;
            }
        }
    }

    let fill_rule = match op {
        BooleanOp::Exclude | BooleanOp::Subtract => FillRule::EvenOdd,
        BooleanOp::Union | BooleanOp::Intersect => FillRule::NonZero,
    };
    let local_paths = contours_to_local_paths(&acc, Vec2::new(new_bounds.x, new_bounds.y), fill_rule);

    if let Some(bn) = doc.get_mut(bool_id) {
        bn.transform = Affine::translate(new_bounds.x, new_bounds.y);
        bn.size = Vec2::new(new_bounds.w, new_bounds.h);
        bn.fill_geometry = Some(local_paths);
    }

    Ok(new_bounds)
}

/// Updates a boolean child's local bounding rect (or moves/resizes it) and
/// automatically recomputes all enclosing `NodeKind::BooleanOp` ancestors.
pub fn update_boolean_child_rect(doc: &mut Document, child_id: NodeId, new_local_rect: Rect) -> Result<Option<Rect>, DocError> {
    let child = doc.get_mut(child_id).ok_or(DocError::NotFound(child_id))?;
    child.transform = Affine::translate(new_local_rect.x, new_local_rect.y);
    child.size = Vec2::new(new_local_rect.w.max(0.0), new_local_rect.h.max(0.0));
    recompute_ancestor_boolean_groups(doc, child_id)
}

/// Walks up the ancestor chain of `node_id` and recomputes every enclosing
/// `NodeKind::BooleanOp` group from innermost to outermost.
pub fn recompute_ancestor_boolean_groups(doc: &mut Document, node_id: NodeId) -> Result<Option<Rect>, DocError> {
    let mut cur = doc.get(node_id).ok_or(DocError::NotFound(node_id))?.parent;
    let mut last_rect = None;
    while let Some(pid) = cur {
        let is_bool = doc.get(pid).is_some_and(|n| matches!(n.kind, NodeKind::BooleanOp(_)));
        let next_parent = doc.get(pid).and_then(|n| n.parent);
        if is_bool {
            last_rect = Some(recompute_boolean_group(doc, pid)?);
        }
        cur = next_parent;
    }
    Ok(last_rect)
}

// ===========================================================================
// PILLAR 3 (FP-03): VARIABLE BINDINGS ON NUMERIC GEOMETRY/TEXT & MODE SWITCHING
// ===========================================================================

/// Strongly-typed node property that can be bound to a design token [`Variable`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BindableProperty {
    Width,
    Height,
    PaddingTop,
    PaddingRight,
    PaddingBottom,
    PaddingLeft,
    ItemSpacing,
    CounterAxisSpacing,
    /// Binds uniform corner radius across all 4 corners (`topLeftRadius`..`bottomLeftRadius`).
    CornerRadius,
    TopLeftRadius,
    TopRightRadius,
    BottomRightRadius,
    BottomLeftRadius,
    Opacity,
    Visible,
    TextContent,
    FontSize,
    StrokeWeight,
    Fill(usize),
    Stroke(usize),
}

impl BindableProperty {
    /// Canonical key stored in [`NodeVariables::bindings`].
    pub fn key(&self) -> String {
        match self {
            BindableProperty::Width => "width".into(),
            BindableProperty::Height => "height".into(),
            BindableProperty::PaddingTop => "paddingTop".into(),
            BindableProperty::PaddingRight => "paddingRight".into(),
            BindableProperty::PaddingBottom => "paddingBottom".into(),
            BindableProperty::PaddingLeft => "paddingLeft".into(),
            BindableProperty::ItemSpacing => "itemSpacing".into(),
            BindableProperty::CounterAxisSpacing => "counterAxisSpacing".into(),
            BindableProperty::CornerRadius => "cornerRadius".into(),
            BindableProperty::TopLeftRadius => "topLeftRadius".into(),
            BindableProperty::TopRightRadius => "topRightRadius".into(),
            BindableProperty::BottomRightRadius => "bottomRightRadius".into(),
            BindableProperty::BottomLeftRadius => "bottomLeftRadius".into(),
            BindableProperty::Opacity => "opacity".into(),
            BindableProperty::Visible => "visible".into(),
            BindableProperty::TextContent => "characters".into(),
            BindableProperty::FontSize => "fontSize".into(),
            BindableProperty::StrokeWeight => "strokeWeight".into(),
            BindableProperty::Fill(i) => format!("fills/{i}"),
            BindableProperty::Stroke(i) => format!("strokes/{i}"),
        }
    }
}

/// Applies a resolved [`VariableValue`] to `node` for `key`, extending
/// [`crate::variables_apply::set_bound_property`] with uniform `"cornerRadius"`
/// and smart opacity (`0.0..=1.0` fractional or `0.0..=100.0` percentage).
pub fn apply_bound_key_to_node(node: &mut Node, key: &str, value: &VariableValue) {
    if key == "cornerRadius" {
        if let VariableValue::Number(r) = value {
            if r.is_finite() {
                let clamped = r.max(0.0);
                node.corner_radii = [clamped; 4];
                node.fill_geometry = None;
                node.stroke_geometry = None;
            }
        }
        return;
    }
    if key == "opacity" {
        if let VariableValue::Number(x) = value {
            if x.is_finite() {
                // Support both 0..=1 fractional opacity and 0..=100 percentage tokens.
                node.opacity = if *x <= 1.0 && *x >= 0.0 { *x } else { (*x / 100.0).clamp(0.0, 1.0) };
            }
        }
        return;
    }
    if let Some(idx_str) = key.strip_prefix("fills/") {
        if let (Ok(idx), VariableValue::Color(c)) = (idx_str.parse::<usize>(), value) {
            let paints = match &mut node.kind {
                NodeKind::Text(t) => &mut t.style.fills,
                _ => &mut node.fills,
            };
            while paints.len() <= idx {
                paints.push(Paint::solid(*c));
            }
            paints[idx].kind = PaintKind::Solid(*c);
            return;
        }
    }
    set_bound_property(node, key, value);
}

/// Binds `property` on `node_id` to `variable_id`, resolves the variable in
/// `node_id`'s current effective mode, and updates the node in-place.
pub fn bind_node_variable(doc: &mut Document, node_id: NodeId, property: BindableProperty, variable_id: impl Into<String>) -> Result<(), DocError> {
    let var_id = variable_id.into();
    let key = property.key();
    {
        let node = doc.get_mut(node_id).ok_or(DocError::NotFound(node_id))?;
        node.variables.bindings.insert(key.clone(), var_id.clone());
        if property == BindableProperty::CornerRadius {
            for k in ["topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"] {
                node.variables.bindings.insert(k.into(), var_id.clone());
            }
        }
    }
    if let Some(val) = doc.resolve_variable(node_id, &var_id) {
        if let Some(node) = doc.get_mut(node_id) {
            apply_bound_key_to_node(node, &key, &val);
        }
    }
    let relayout_root = doc.get(node_id).and_then(|n| n.parent).unwrap_or(node_id);
    relayout_subtree(doc, relayout_root);
    let _ = recompute_ancestor_boolean_groups(doc, node_id);
    Ok(())
}

/// Switches `collection_id` to `mode_id` on `root_id`, re-evaluates all bound
/// fills, strokes, numbers (`Width`, `Height`, `Padding*`, `ItemSpacing`,
/// `CornerRadius`, `Opacity`), booleans (`Visible`), and strings (`TextContent`)
/// across `root_id`'s entire subtree, and triggers [`relayout_subtree`].
///
/// Returns the number of nodes in the subtree whose bound properties were updated.
pub fn apply_variable_mode_to_subtree(doc: &mut Document, root_id: NodeId, collection_id: &str, mode_id: &str) -> Result<usize, DocError> {
    {
        let root = doc.get_mut(root_id).ok_or(DocError::NotFound(root_id))?;
        root.variables.modes.insert(collection_id.to_string(), mode_id.to_string());
    }

    let mut subtree = Vec::new();
    doc.walk(root_id, |n, _| subtree.push(n.id));

    let mut updated_count = 0usize;
    for id in &subtree {
        let bindings = doc.get(*id).map(|n| n.variables.bindings.clone()).unwrap_or_default();
        if bindings.is_empty() {
            continue;
        }
        let mut resolved_pairs = Vec::with_capacity(bindings.len());
        for (key, var_id) in bindings {
            if let Some(val) = doc.resolve_variable(*id, &var_id) {
                resolved_pairs.push((key, val));
            }
        }
        if !resolved_pairs.is_empty() {
            if let Some(node) = doc.get_mut(*id) {
                for (key, val) in &resolved_pairs {
                    apply_bound_key_to_node(node, key, val);
                }
                updated_count += 1;
            }
        }
    }

    relayout_subtree(doc, root_id);

    for id in &subtree {
        if doc.get(*id).is_some_and(|n| matches!(n.kind, NodeKind::BooleanOp(_))) {
            let _ = recompute_boolean_group(doc, *id);
        }
    }

    Ok(updated_count)
}

// ===========================================================================
// PILLAR 4 (FP-04): INTERACTIVE PROTOTYPE RUNTIME
// ===========================================================================

/// Scroll behaviour of a prototype container frame (`None`, `Horizontal`,
/// `Vertical`, `Both`). Re-exported alias of [`Overflow`].
pub type OverflowScrollMode = Overflow;

/// Stateful interactive prototype session supporting overflow scroll clamping,
/// interactive component variant swapping (`Action::ChangeTo`), and conditional
/// variable evaluation (`Action::SetVariable`, `Action::SetVariableMode`,
/// `Action::Conditional`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PrototypeRuntime {
    /// Currently presented top-level frame.
    pub current_frame: Option<NodeId>,
    /// Navigation stack for `Action::Back`.
    pub navigation_history: Vec<NodeId>,
    /// Active overlay stack (`OpenOverlay`, `SwapOverlay`, `CloseOverlay`).
    pub overlays: Vec<NodeId>,
    /// Clamped scroll offsets `(scroll_x, scroll_y)` per scrollable container.
    pub scroll_offsets: BTreeMap<NodeId, Vec2>,
    /// Session-scoped variable value overrides set by `Action::SetVariable`.
    pub variable_overrides: BTreeMap<String, VariableValue>,
    /// Session-scoped collection mode overrides set by `Action::SetVariableMode`.
    pub mode_overrides: BTreeMap<String, String>,
}

impl PrototypeRuntime {
    /// Creates a new prototype runtime starting at `start_frame` (or the first
    /// flow starting frame in `doc.flows`).
    pub fn new(doc: &Document, start_frame: Option<NodeId>) -> Self {
        let initial = start_frame.or_else(|| doc.flows.first().map(|f| f.start));
        Self {
            current_frame: initial,
            navigation_history: Vec::new(),
            overlays: Vec::new(),
            scroll_offsets: BTreeMap::new(),
            variable_overrides: BTreeMap::new(),
            mode_overrides: BTreeMap::new(),
        }
    }

    /// Configures the [`OverflowScrollMode`] on `container_id`.
    pub fn set_overflow_scroll_mode(doc: &mut Document, container_id: NodeId, mode: OverflowScrollMode) -> Result<(), DocError> {
        let node = doc.get_mut(container_id).ok_or(DocError::NotFound(container_id))?;
        node.prototype.overflow = mode;
        Ok(())
    }

    /// Computes the bounding box of all visible, non-fixed children of
    /// `container_id` in container-local coordinates.
    pub fn content_bounds(doc: &Document, container_id: NodeId) -> Rect {
        let Some(container) = doc.get(container_id) else {
            return Rect::default();
        };
        let fixed = &container.prototype.fixed_children;
        let rects: Vec<Rect> = container
            .children
            .iter()
            .copied()
            .filter(|cid| !fixed.contains(cid))
            .filter_map(|cid| doc.get(cid))
            .filter(|c| c.visible)
            .map(node_parent_rect)
            .collect();
        if rects.is_empty() {
            return Rect::new(0.0, 0.0, container.size.x, container.size.y);
        }
        let mut u = union_rects(&rects);
        if let Some(al) = container.kind.frame_props().and_then(|f| f.auto_layout.as_ref()) {
            let [_, pad_right, pad_bottom, _] = al.padding;
            u.w += pad_right;
            u.h += pad_bottom;
        }
        u
    }

    /// Maximum legal scroll offset `(max_x, max_y)` for `container_id` given its
    /// [`OverflowScrollMode`] and content extent.
    pub fn max_scroll_offset(doc: &Document, container_id: NodeId) -> Vec2 {
        let Some(container) = doc.get(container_id) else {
            return Vec2::ZERO;
        };
        let content = Self::content_bounds(doc, container_id);
        let content_right = (content.x + content.w).max(content.w);
        let content_bottom = (content.y + content.h).max(content.h);
        let max_x = (content_right - container.size.x).max(0.0);
        let max_y = (content_bottom - container.size.y).max(0.0);
        match container.prototype.overflow {
            OverflowScrollMode::None => Vec2::ZERO,
            OverflowScrollMode::Horizontal => Vec2::new(max_x, 0.0),
            OverflowScrollMode::Vertical => Vec2::new(0.0, max_y),
            OverflowScrollMode::Both => Vec2::new(max_x, max_y),
        }
    }

    /// Current scroll offset `(scroll_x, scroll_y)` of `container_id`.
    pub fn scroll_offset(&self, container_id: NodeId) -> Vec2 {
        self.scroll_offsets.get(&container_id).copied().unwrap_or(Vec2::ZERO)
    }

    /// Scrolls `container_id` by `delta` and clamps the resulting scroll offset
    /// to `[0.0, max_scroll.x] × [0.0, max_scroll.y]` according to its
    /// [`OverflowScrollMode`].
    pub fn scroll_by(&mut self, doc: &Document, container_id: NodeId, delta: Vec2) -> Vec2 {
        let cur = self.scroll_offset(container_id);
        self.scroll_to_offset(doc, container_id, Vec2::new(cur.x + delta.x, cur.y + delta.y))
    }

    /// Sets `container_id`'s scroll offset to `offset`, clamped to valid bounds.
    pub fn scroll_to_offset(&mut self, doc: &Document, container_id: NodeId, offset: Vec2) -> Vec2 {
        let max_s = Self::max_scroll_offset(doc, container_id);
        let clamped = Vec2::new(offset.x.clamp(0.0, max_s.x), offset.y.clamp(0.0, max_s.y));
        self.scroll_offsets.insert(container_id, clamped);
        clamped
    }

    /// Effective viewport-relative position of `child_id` inside `container_id`
    /// after applying scroll offset (fixed children remain pinned).
    pub fn effective_child_position(&self, doc: &Document, container_id: NodeId, child_id: NodeId) -> Option<Vec2> {
        let container = doc.get(container_id)?;
        let child = doc.get(child_id)?;
        if child.parent != Some(container_id) {
            return None;
        }
        let pos = child.transform.translation();
        if container.prototype.fixed_children.contains(&child_id) {
            Some(pos)
        } else {
            let scroll = self.scroll_offset(container_id);
            Some(Vec2::new(pos.x - scroll.x, pos.y - scroll.y))
        }
    }

    /// Resolves a variable's current runtime value, checking session overrides
    /// first and then falling back to `doc.variables` in the active mode.
    pub fn get_variable_value(&self, doc: &Document, variable_id: &str) -> Option<VariableValue> {
        if let Some(ov) = self.variable_overrides.get(variable_id) {
            return Some(ov.clone());
        }
        let v = doc.variables.get(variable_id)?;
        let coll = doc.variable_collections.get(&v.collection)?;
        let mode = self
            .mode_overrides
            .get(&v.collection)
            .or_else(|| {
                self.current_frame
                    .and_then(|f| doc.variable_mode(f, &v.collection))
                    .as_ref()
                    .and_then(|m| coll.modes.iter().find(|cm| cm.id == *m).map(|cm| &cm.id))
            })
            .unwrap_or(&coll.default_mode);
        let val = v.values.get(mode).or_else(|| v.values.get(&coll.default_mode))?;
        match val {
            VariableValue::Alias(next) => self.get_variable_value(doc, next),
            other => Some(other.clone()),
        }
    }

    /// Evaluates a prototype [`Expression`] against the current runtime variable
    /// environment.
    pub fn eval_expression(&self, doc: &Document, expr: &Expression) -> Result<ExprValue, EvalError> {
        expr.eval(&|var_id| self.get_variable_value(doc, var_id).map(|v| ExprValue::from(&v)))
    }

    /// Sets a runtime variable override and re-evaluates all bound properties in
    /// `doc` so bound text, visibility, colors, and dimensions reflect the new value.
    pub fn set_variable_value(&mut self, doc: &mut Document, variable_id: &str, value: VariableValue) {
        self.variable_overrides.insert(variable_id.to_string(), value.clone());
        if let Some(v) = doc.variables.get_mut(variable_id) {
            let mode = self
                .mode_overrides
                .get(&v.collection)
                .cloned()
                .or_else(|| doc.variable_collections.get(&v.collection).map(|c| c.default_mode.clone()))
                .unwrap_or_default();
            if !mode.is_empty() {
                v.values.insert(mode, value);
            }
        }
        self.sync_runtime_variables_to_doc(doc);
    }

    fn sync_runtime_variables_to_doc(&self, doc: &mut Document) {
        let roots: Vec<NodeId> = match self.current_frame {
            Some(f) => vec![f],
            None => doc.pages.clone(),
        };
        for root in roots {
            let mut ids = Vec::new();
            doc.walk(root, |n, _| ids.push(n.id));
            for id in ids {
                let bindings = doc.get(id).map(|n| n.variables.bindings.clone()).unwrap_or_default();
                if bindings.is_empty() {
                    continue;
                }
                let mut resolved = Vec::new();
                for (key, var_id) in bindings {
                    if let Some(val) = self.get_variable_value(doc, &var_id) {
                        resolved.push((key, val));
                    }
                }
                if let Some(node) = doc.get_mut(id) {
                    for (key, val) in &resolved {
                        apply_bound_key_to_node(node, key, val);
                    }
                }
            }
            relayout_subtree(doc, root);
        }
    }

    /// Fires all prototype [`Reaction`]s on `node_id` (or its master component
    /// counterpart if `node_id` is an instance) matching `trigger`, executing
    /// `ChangeTo` variant swaps, `SetVariable`, `SetVariableMode`, `Conditional`,
    /// `ScrollTo`, and navigation actions.
    pub fn trigger_variant_interaction(&mut self, doc: &mut Document, node_id: NodeId, trigger: &Trigger) -> Result<Vec<Action>, DocError> {
        let node = doc.get(node_id).cloned().ok_or(DocError::NotFound(node_id))?;
        let mut reactions = node.prototype.reactions.clone();
        if reactions.is_empty() {
            if let Some(src_id) = node.component.source.or_else(|| instance_component(&node)) {
                if let Some(src_node) = doc.get(src_id) {
                    reactions.extend(src_node.prototype.reactions.clone());
                }
            }
        }

        let mut executed = Vec::new();
        for reaction in reactions {
            if &reaction.trigger == trigger {
                for action in &reaction.actions {
                    self.execute_action_trace(doc, node_id, action, &mut executed)?;
                }
            }
        }
        Ok(executed)
    }

    /// Executes a single prototype [`Action`] initiated from `source_node`.
    pub fn execute_action(&mut self, doc: &mut Document, source_node: NodeId, action: &Action) -> Result<(), DocError> {
        let mut trace = Vec::new();
        self.execute_action_trace(doc, source_node, action, &mut trace)
    }

    fn execute_action_trace(&mut self, doc: &mut Document, source_node: NodeId, action: &Action, executed: &mut Vec<Action>) -> Result<(), DocError> {
        executed.push(action.clone());
        match action {
            Action::Navigate { dest, preserve_scroll } => {
                if let Some(cur) = self.current_frame {
                    self.navigation_history.push(cur);
                }
                if !preserve_scroll {
                    self.scroll_offsets.remove(dest);
                }
                self.current_frame = Some(*dest);
            }
            Action::Back => {
                if let Some(prev) = self.navigation_history.pop() {
                    self.current_frame = Some(prev);
                }
            }
            Action::OpenOverlay { dest, .. } => {
                if !self.overlays.contains(dest) {
                    self.overlays.push(*dest);
                }
            }
            Action::SwapOverlay { dest } => {
                self.overlays.pop();
                self.overlays.push(*dest);
            }
            Action::CloseOverlay => {
                self.overlays.pop();
            }
            Action::ScrollTo { dest, offset } => {
                if let Some(target) = doc.get(*dest) {
                    let mut cur_anc = target.parent;
                    while let Some(anc_id) = cur_anc {
                        let Some(anc) = doc.get(anc_id) else { break };
                        if anc.prototype.overflow != OverflowScrollMode::None {
                            let pos = target.transform.translation();
                            self.scroll_to_offset(doc, anc_id, Vec2::new(pos.x + offset.x, pos.y + offset.y));
                            break;
                        }
                        cur_anc = anc.parent;
                    }
                }
            }
            Action::ChangeTo { dest } => {
                let inst_id = if doc.get(source_node).is_some_and(|n| matches!(n.kind, NodeKind::Instance { .. })) {
                    source_node
                } else {
                    instance_root_of(doc, source_node).unwrap_or(source_node)
                };
                swap_instance_to_component(doc, inst_id, *dest)?;
            }
            Action::SetVariable { variable, value } => {
                if let Ok(eval_val) = self.eval_expression(doc, value) {
                    let kind = doc.variables.get(variable).map(|v| v.kind).unwrap_or(match &eval_val {
                        ExprValue::Number(_) => VariableKind::Number,
                        ExprValue::String(_) => VariableKind::String,
                        ExprValue::Boolean(_) => VariableKind::Boolean,
                        ExprValue::Color(_) => VariableKind::Color,
                    });
                    if let Some(var_val) = eval_val.to_variable_value(kind) {
                        self.set_variable_value(doc, variable, var_val);
                    }
                }
            }
            Action::SetVariableMode { collection, mode } => {
                self.mode_overrides.insert(collection.clone(), mode.clone());
                if let Some(frame_id) = self.current_frame {
                    let _ = apply_variable_mode_to_subtree(doc, frame_id, collection, mode);
                }
            }
            Action::Conditional { branches, otherwise } => {
                let mut matched = false;
                for (cond, branch_actions) in branches {
                    if self.eval_expression(doc, cond).ok() == Some(ExprValue::Boolean(true)) {
                        matched = true;
                        for act in branch_actions {
                            self.execute_action_trace(doc, source_node, act, executed)?;
                        }
                        break;
                    }
                }
                if !matched {
                    for act in otherwise {
                        self.execute_action_trace(doc, source_node, act, executed)?;
                    }
                }
            }
            Action::OpenUrl { .. } | Action::MediaControl { .. } => {}
        }
        Ok(())
    }
}

/// Swaps `instance_id` to target component variant `dest_comp` in-place,
/// updating variant property values and reflowing any enclosing Auto-Layout.
pub fn swap_instance_to_component(doc: &mut Document, instance_id: NodeId, dest_comp: NodeId) -> Result<NodeId, DocError> {
    if doc.get(instance_id).is_none() {
        return Err(DocError::NotFound(instance_id));
    }
    let comp_node = doc.get(dest_comp).cloned().ok_or(DocError::NotFound(dest_comp))?;
    let cmd = swap_instance(doc, instance_id, dest_comp);
    cmd.apply(doc)?;

    // Record variant property values from the target variant's name (`State=Hover`, etc.).
    let pairs = parse_variant_name(&comp_node.name);
    if let Some(inst) = doc.get_mut(instance_id) {
        inst.size = comp_node.size;
        for (k, v) in pairs {
            inst.component.values.insert(k, PropValue::Variant(v));
        }
    }
    let relayout_root = doc.get(instance_id).and_then(|n| n.parent).unwrap_or(instance_id);
    relayout_subtree(doc, relayout_root);
    Ok(dest_comp)
}

/// Swaps `instance_id` to the sibling variant inside its `ComponentSet` where
/// `prop_name == variant_value` (e.g. `("State", "Hover")`).
pub fn swap_instance_variant(doc: &mut Document, instance_id: NodeId, prop_name: &str, variant_value: &str) -> Result<NodeId, DocError> {
    let cmd = set_instance_prop(doc, instance_id, prop_name, Some(PropValue::Variant(variant_value.to_string())));
    cmd.apply(doc)?;
    let new_comp = doc.get(instance_id).and_then(instance_component).ok_or(DocError::NotFound(instance_id))?;
    if let Some(comp_size) = doc.get(new_comp).map(|c| c.size) {
        if let Some(inst) = doc.get_mut(instance_id) {
            inst.size = comp_size;
        }
    }
    let relayout_root = doc.get(instance_id).and_then(|n| n.parent).unwrap_or(instance_id);
    relayout_subtree(doc, relayout_root);
    Ok(new_comp)
}

// ===========================================================================
// CONVENIENCE METHODS ON `Document`
// ===========================================================================

impl Document {
    /// Pillar 1 (`FP-01`): Suggest Auto-Layout (`Cmd+Shift+A` / `Ctrl+Shift+A`).
    pub fn suggest_auto_layout(&mut self, selected_ids: &[NodeId]) -> Result<SuggestAutoLayoutResult, DocError> {
        suggest_auto_layout(self, selected_ids)
    }

    /// Pillar 2 (`FP-02`): Creates a live non-destructive boolean group.
    pub fn create_boolean_group(&mut self, op: BooleanOp, child_ids: &[NodeId]) -> Result<NodeId, DocError> {
        create_boolean_group(self, op, child_ids)
    }

    /// Pillar 2 (`FP-02`): Recomputes a live boolean group's bounding box and `fill_geometry`.
    pub fn recompute_boolean_group(&mut self, bool_id: NodeId) -> Result<Rect, DocError> {
        recompute_boolean_group(self, bool_id)
    }

    /// Pillar 3 (`FP-03`): Binds a node property to a variable and resolves it immediately.
    pub fn bind_node_variable(&mut self, node_id: NodeId, property: BindableProperty, variable_id: impl Into<String>) -> Result<(), DocError> {
        bind_node_variable(self, node_id, property, variable_id)
    }

    /// Pillar 3 (`FP-03`): Switches variable mode on `root_id` and re-evaluates all bound properties in its subtree.
    pub fn apply_variable_mode_to_subtree(&mut self, root_id: NodeId, collection_id: &str, mode_id: &str) -> Result<usize, DocError> {
        apply_variable_mode_to_subtree(self, root_id, collection_id, mode_id)
    }
}
