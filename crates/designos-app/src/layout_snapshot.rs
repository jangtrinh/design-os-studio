//! Layout-unchanged gate (`#1126`, Spec L): deterministic geometry snapshot capture and G1–G5 comparer.

use crate::app::ShellRegions;
pub use crate::widgets::RECORDING;
use egui::{Context, Pos2, Rect};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const SNAPSHOT_SCHEMA: u32 = 1;

pub const REGION_KEYS: [&str; 10] = ["title_bar", "tab_bar", "rail", "left_panel", "canvas", "notes", "right_panel", "home", "present", "variables_full"];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayoutSnapshot {
    pub schema: u32,
    pub case: String,
    pub size: [u32; 2],
    pub scale: f32,
    pub regions: BTreeMap<String, Option<[f32; 4]>>,
    pub overlays: Vec<OverlayGeom>,
    pub named: BTreeMap<String, NamedWidget>,
    pub unnamed: Vec<UnnamedWidget>,
    pub scroll: Vec<ScrollGeom>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OverlayGeom {
    pub name: String,
    pub rect: [f32; 4],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NamedWidget {
    pub region: String,
    pub rect: [f32; 4],
    pub count: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UnnamedWidget {
    pub region: String,
    pub rect: [f32; 4],
    pub sense: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScrollGeom {
    pub region: String,
    pub axis: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub maybe: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AnchorX {
    #[default]
    Left,
    Right,
    #[serde(alias = "center")]
    Centre,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AnchorY {
    #[default]
    Top,
    Bottom,
    #[serde(alias = "center")]
    Centre,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AllowEntry {
    pub case: String,
    pub region: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h_base: Option<f32>,
    /// With `h_base` or `name_prefix`: the entry covers only widgets whose baseline left edge lies in
    /// `[lo, hi]`, so two legs can allow same-height widgets in different parts of one region. A covered
    /// widget's derived x shift counts only row-mates inside the same band; rows, the row height check and
    /// vertical shifts still span the whole region.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x_band: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_x: Option<AnchorX>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_y: Option<AnchorY>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_dw: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_dw: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_dh: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_dh: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dx: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dy: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap_dy: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inventory_max_delta: Option<isize>,
    pub spec: String,
    pub reason: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AllowList {
    pub entries: Vec<AllowEntry>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompareReport {
    pub new_names: Vec<String>,
    pub scroll_changes: Vec<String>,
    pub matched_allow_indices: BTreeSet<usize>,
}

#[inline]
pub fn round_quarter(v: f32) -> f32 {
    if !v.is_finite() {
        return 0.0;
    }
    let q = (v * 4.0).round() / 4.0;
    if q == -0.0 {
        0.0
    } else {
        q
    }
}

#[inline]
pub fn rect_to_q4(r: Rect) -> [f32; 4] {
    [round_quarter(r.min.x), round_quarter(r.min.y), round_quarter(r.width()), round_quarter(r.height())]
}

#[inline]
pub fn q4_to_rect(q: [f32; 4]) -> Rect {
    Rect::from_min_size(Pos2::new(q[0], q[1]), egui::Vec2::new(q[2], q[3]))
}

fn region_entries(regions: &ShellRegions) -> [(&'static str, Option<Rect>); 10] {
    [
        ("title_bar", regions.title_bar),
        ("tab_bar", regions.tab_bar),
        ("rail", regions.rail),
        ("left_panel", regions.left_panel),
        ("canvas", regions.canvas),
        ("notes", regions.notes),
        ("right_panel", regions.right_panel),
        ("home", regions.home),
        ("present", regions.present),
        ("variables_full", regions.variables_full),
    ]
}

fn classify_point(pos: Pos2, regions: &ShellRegions, overlays: &[OverlayGeom]) -> String {
    for ov in overlays {
        let r = q4_to_rect(ov.rect);
        if r.contains(pos) {
            return if ov.name.starts_with("overlay:") { ov.name.clone() } else { format!("overlay:{}", ov.name) };
        }
    }
    let mut best: Option<(&str, f32)> = None;
    for (name, opt_rect) in region_entries(regions) {
        if let Some(r) = opt_rect {
            if r.contains(pos) {
                let area = r.width().max(0.0) * r.height().max(0.0);
                if best.is_none_or(|(_, best_area)| area < best_area) {
                    best = Some((name, area));
                }
            }
        }
    }
    if let Some((name, _)) = best {
        return name.to_string();
    }
    if let Some(ov) = overlays.first() {
        return if ov.name.starts_with("overlay:") { ov.name.clone() } else { format!("overlay:{}", ov.name) };
    }
    "unknown".to_string()
}

fn sense_list(sense: egui::Sense) -> Vec<String> {
    let mut out = Vec::new();
    if sense.contains(egui::Sense::CLICK) {
        out.push("click".to_string());
    }
    if sense.contains(egui::Sense::DRAG) {
        out.push("drag".to_string());
    }
    if sense.contains(egui::Sense::FOCUSABLE) {
        out.push("focus".to_string());
    }
    out
}

fn cmp_q4(a: &[f32; 4], b: &[f32; 4]) -> std::cmp::Ordering {
    a[1].total_cmp(&b[1]).then_with(|| a[0].total_cmp(&b[0])).then_with(|| a[2].total_cmp(&b[2])).then_with(|| a[3].total_cmp(&b[3]))
}

fn rects_match_within_half_pt(a: [f32; 4], b: [f32; 4]) -> bool {
    (a[0] - b[0]).abs() <= 0.5 && (a[1] - b[1]).abs() <= 0.5 && (a[2] - b[2]).abs() <= 0.5 && (a[3] - b[3]).abs() <= 0.5
}

/// Capture a deterministic `LayoutSnapshot` after the settle frame.
pub fn capture(ctx: &Context, regions: &ShellRegions) -> LayoutSnapshot {
    let screen = ctx.content_rect();
    let scale = round_quarter(ctx.pixels_per_point());
    let size = [screen.width().round().max(0.0) as u32, screen.height().round().max(0.0) as u32];

    let mut region_map = BTreeMap::new();
    for (name, opt_rect) in region_entries(regions) {
        region_map.insert(name.to_string(), opt_rect.map(rect_to_q4));
    }

    let area_layers: Vec<(egui::LayerId, Rect)> = ctx.memory(|m| {
        m.layer_ids()
            .filter(|layer| layer.order != egui::Order::Background && m.areas().is_visible(layer))
            .filter_map(|layer| m.area_rect(layer.id).map(|r| (layer, r)))
            .collect()
    });

    let mut order_counts: BTreeMap<String, usize> = BTreeMap::new();
    let overlays: Vec<OverlayGeom> = area_layers
        .iter()
        .filter(|(_, r)| r.width() > 0.0 && r.height() > 0.0)
        .map(|(layer, r)| {
            let global_r = ctx.layer_transform_to_global(*layer).map_or(*r, |t| t * *r);
            let order_name = format!("{:?}", layer.order);
            let idx = *order_counts.entry(order_name.clone()).and_modify(|c| *c += 1).or_insert(0);
            OverlayGeom { name: format!("overlay:{order_name}/{idx}"), rect: rect_to_q4(global_r) }
        })
        .collect();

    let recorded_pairs = crate::widgets::last_pass_names();
    let mut recorded: BTreeMap<String, (Rect, usize)> = BTreeMap::new();
    for (name, rect) in recorded_pairs {
        let entry = recorded.entry(name).or_insert((rect, 0));
        entry.0 = rect;
        entry.1 += 1;
    }
    let mut named = BTreeMap::new();
    let mut named_q4_rects: Vec<[f32; 4]> = Vec::with_capacity(recorded.len());
    for (name, (rect, count)) in recorded {
        let q4 = rect_to_q4(rect);
        let center = rect.center();
        let region = classify_point(center, regions, &overlays);
        named.insert(name, NamedWidget { region, rect: q4, count });
        named_q4_rects.push(q4);
    }

    let widgets = ctx.viewport(|vp| vp.prev_pass.widgets.clone());
    let global_style = ctx.global_style();
    #[cfg(debug_assertions)]
    let widget_info_enabled = global_style.debug.show_interactive_widgets;
    #[cfg(not(debug_assertions))]
    let widget_info_enabled = false;
    let bar_width = global_style.spacing.scroll.bar_width;
    let mut unnamed = Vec::new();
    let mut scroll_set: BTreeSet<(String, String, bool)> = BTreeSet::new();

    for (&layer_id, rect_entries) in widgets.layers() {
        let to_global = ctx.layer_transform_to_global(layer_id);
        for rect_entry in rect_entries {
            if !rect_entry.enabled {
                continue;
            }
            let global_rect = to_global.map_or(rect_entry.interact_rect, |t| t * rect_entry.interact_rect);
            if global_rect.width() <= 0.0 || global_rect.height() <= 0.0 {
                continue;
            }
            let q4 = rect_to_q4(global_rect);
            let region = classify_point(global_rect.center(), regions, &overlays);

            if widget_info_enabled {
                if widgets.info(rect_entry.id).is_some_and(|info| info.typ == egui::WidgetType::ScrollBar) {
                    let axis = if global_rect.height() >= global_rect.width() { "y" } else { "x" };
                    scroll_set.insert((region, axis.to_string(), false));
                    continue;
                }
            } else if rect_entry.sense.contains(egui::Sense::DRAG) {
                let w = global_rect.width();
                let h = global_rect.height();
                let narrow_axis = if (w - bar_width).abs() <= 0.5 && h >= 24.0 {
                    Some("y")
                } else if (h - bar_width).abs() <= 0.5 && w >= 24.0 {
                    Some("x")
                } else {
                    None
                };
                if let Some(axis) = narrow_axis {
                    scroll_set.insert((region, axis.to_string(), true));
                    continue;
                }
            }

            let senses = sense_list(rect_entry.sense);
            if senses.is_empty() {
                continue;
            }
            if named_q4_rects.iter().any(|nr| rects_match_within_half_pt(*nr, q4)) {
                continue;
            }
            unnamed.push(UnnamedWidget { region, rect: q4, sense: senses });
        }
    }

    unnamed.sort_by(|a, b| a.region.cmp(&b.region).then_with(|| cmp_q4(&a.rect, &b.rect)).then_with(|| a.sense.cmp(&b.sense)));
    unnamed.dedup_by(|a, b| a.region == b.region && a.rect == b.rect && a.sense == b.sense);

    let scroll = scroll_set.into_iter().map(|(region, axis, maybe)| ScrollGeom { region, axis, maybe }).collect();

    LayoutSnapshot { schema: SNAPSHOT_SCHEMA, case: String::new(), size, scale, regions: region_map, overlays, named, unnamed, scroll }
}

pub fn parse_allow_list(json: &str) -> Result<AllowList, String> {
    let entries: Vec<AllowEntry> = serde_json::from_str(json).map_err(|e| format!("failed to parse allow.json: {e}"))?;
    validate_allow_entries(entries)
}

fn entry_specificity(e: &AllowEntry) -> u8 {
    if e.name_prefix.is_some() {
        2
    } else if e.h_base.is_some() {
        1
    } else {
        0
    }
}

pub fn validate_allow_entries(entries: Vec<AllowEntry>) -> Result<AllowList, String> {
    let valid_specs = ["D", "E", "F", "G", "H", "I", "J", "K"];
    for (idx, e) in entries.iter().enumerate() {
        if e.case.trim().is_empty() {
            return Err(format!("allow.json[{idx}]: case must be non-empty"));
        }
        if e.case == "*" {
            if e.name_prefix.is_none() && e.h_base.is_none() {
                return Err(format!("allow.json[{idx}]: wildcard case '*' requires name_prefix or h_base"));
            }
        } else if e.case.contains('*') {
            return Err(format!("allow.json[{idx}]: partial wildcard case {:?} is not allowed", e.case));
        }
        if e.region.trim().is_empty() || e.region.contains('*') {
            return Err(format!("allow.json[{idx}]: region must be non-empty (no wildcards), got {:?}", e.region));
        }
        if !valid_specs.contains(&e.spec.as_str()) {
            return Err(format!("allow.json[{idx}]: spec must be one of D..K, got {:?}", e.spec));
        }
        if e.reason.trim().is_empty() {
            return Err(format!("allow.json[{idx}]: reason must be non-empty"));
        }
        if let Some(inv_delta) = e.inventory_max_delta {
            if inv_delta < 0 {
                return Err(format!("allow.json[{idx}]: inventory_max_delta must be non-negative, got {inv_delta}"));
            }
        }
        let check_delta = |field: &str, val: Option<f32>| -> Result<(), String> {
            if let Some(v) = val {
                if !v.is_finite() {
                    return Err(format!("allow.json[{idx}]: {field}={v} must be finite"));
                }
            }
            Ok(())
        };
        check_delta("min_dw", e.min_dw)?;
        check_delta("max_dw", e.max_dw)?;
        check_delta("min_dh", e.min_dh)?;
        check_delta("max_dh", e.max_dh)?;
        check_delta("dx", e.dx)?;
        check_delta("dy", e.dy)?;
        check_delta("gap_dy", e.gap_dy)?;
        let has_negative_size_delta = [e.min_dw, e.max_dw, e.min_dh, e.max_dh].into_iter().flatten().any(|v| v < 0.0);
        let has_r5_token = e.reason.split(|c: char| !c.is_ascii_alphanumeric() && c != '_').any(|tok| tok == "R5");
        if has_negative_size_delta && !has_r5_token {
            return Err(format!("allow.json[{idx}]: negative dw/dh requires R5 in reason, got {:?}", e.reason));
        }
        if let Some([lo, hi]) = e.x_band {
            if e.h_base.is_none() && e.name_prefix.is_none() {
                return Err(format!("allow.json[{idx}]: x_band needs h_base or name_prefix"));
            }
            if lo > hi {
                return Err(format!("allow.json[{idx}]: x_band must run from low to high, got [{lo}, {hi}]"));
            }
        }
    }

    for i in 0..entries.len() {
        for j in (i + 1)..entries.len() {
            let a = &entries[i];
            let b = &entries[j];
            let cases_overlap = a.case == b.case || a.case == "*" || b.case == "*";
            if !cases_overlap || a.region != b.region {
                continue;
            }
            let sa = entry_specificity(a);
            let sb = entry_specificity(b);
            if sa != sb {
                continue;
            }
            let same_target = match sa {
                2 => {
                    let pa = a.name_prefix.as_deref().unwrap_or("");
                    let pb = b.name_prefix.as_deref().unwrap_or("");
                    (pa.starts_with(pb) || pb.starts_with(pa)) && x_bands_overlap(a.x_band, b.x_band)
                }
                1 => {
                    let ha = a.h_base.unwrap_or(0.0);
                    let hb = b.h_base.unwrap_or(0.0);
                    (ha - hb).abs() <= 0.5 && x_bands_overlap(a.x_band, b.x_band)
                }
                _ => true,
            };
            if same_target {
                let bands = if a.x_band.is_some() || b.x_band.is_some() { " (their x_band ranges overlap)" } else { "" };
                return Err(format!("allow.json[{i}] and allow.json[{j}]: equally specific entries covering case='{}' region='{}'{bands}", b.case, b.region));
            }
        }
    }

    Ok(AllowList { entries })
}

fn entry_matches_widget(e: &AllowEntry, case: &str, region: &str, name: Option<&str>, base_rect: [f32; 4]) -> bool {
    if (e.case != case && e.case != "*") || e.region != region {
        return false;
    }
    if let Some(prefix) = &e.name_prefix {
        match name {
            Some(n) if n.starts_with(prefix.as_str()) => {}
            _ => return false,
        }
    } else if let Some(hb) = e.h_base {
        if (base_rect[3] - hb).abs() > 0.5 {
            return false;
        }
    }
    if let Some([lo, hi]) = e.x_band {
        if base_rect[0] < lo || base_rect[0] > hi {
            return false;
        }
    }
    true
}

/// An entry without a band covers every x, so it overlaps any other entry.
fn x_bands_overlap(a: Option<[f32; 2]>, b: Option<[f32; 2]>) -> bool {
    match (a, b) {
        (Some([alo, ahi]), Some([blo, bhi])) => alo <= bhi && blo <= ahi,
        _ => true,
    }
}

fn best_matching_entry<'a>(allow: &'a AllowList, case: &str, region: &str, name: Option<&str>, base_rect: [f32; 4]) -> Option<(usize, &'a AllowEntry)> {
    allow
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| entry_matches_widget(e, case, region, name, base_rect))
        .max_by_key(|(_, e)| (entry_specificity(e), e.name_prefix.as_ref().map_or(0, |s| s.len()), usize::from(e.case != "*")))
}

#[derive(Clone, Copy, Debug, Default)]
struct DerivedShifts {
    sum_dw_left: f32,
    sum_dw_right: f32,
    sum_dh_above: f32,
    sum_dh_below: f32,
    gaps_above: usize,
}

fn widget_satisfies_allow_bounds(e: &AllowEntry, base: [f32; 4], cur: [f32; 4], shifts: DerivedShifts) -> bool {
    let dw = cur[2] - base[2];
    let dh = cur[3] - base[3];

    if dw.abs() > 0.5 {
        let min_dw = e.min_dw.unwrap_or(0.0);
        let max_dw = e.max_dw.unwrap_or(0.0);
        if dw < min_dw - 0.5 || dw > max_dw + 0.5 {
            return false;
        }
    }

    if dh.abs() > 0.5 {
        let min_dh = e.min_dh.unwrap_or(0.0);
        let max_dh = e.max_dh.unwrap_or(0.0);
        if dh < min_dh - 0.5 || dh > max_dh + 0.5 {
            return false;
        }
    }

    let expected_x = match e.anchor_x.unwrap_or_default() {
        AnchorX::Left => base[0] + shifts.sum_dw_left,
        AnchorX::Right => base[0] + base[2] - cur[2] - shifts.sum_dw_right,
        AnchorX::Centre => base[0] + (base[2] - cur[2]) * 0.5,
    } + e.dx.unwrap_or(0.0);

    let gap_dy = e.gap_dy.unwrap_or(0.0);
    let expected_y = match e.anchor_y.unwrap_or_default() {
        AnchorY::Top => base[1] + shifts.sum_dh_above + (shifts.gaps_above as f32) * gap_dy,
        AnchorY::Bottom => base[1] + base[3] - cur[3] - shifts.sum_dh_below + (shifts.gaps_above as f32) * gap_dy,
        AnchorY::Centre => base[1] + (base[3] - cur[3]) * 0.5 + (shifts.gaps_above as f32) * gap_dy,
    } + e.dy.unwrap_or(0.0);

    let x_tol = 0.5;
    let y_tol = 0.5;

    (cur[0] - expected_x).abs() <= x_tol && (cur[1] - expected_y).abs() <= y_tol
}

fn rects_differ_beyond_half_pt(a: [f32; 4], b: [f32; 4]) -> bool {
    !rects_match_within_half_pt(a, b)
}

/// A widget paired across snapshots for the G3 order check: `(baseline rect, current rect, name)`.
type OrderItem<'a> = ([f32; 4], [f32; 4], &'a str);

fn centre_x(r: &[f32; 4]) -> f32 {
    r[0] + r[2] * 0.5
}

/// Cluster paired widgets into rows by their BASELINE rects (centres within `max(4, 0.5*min(h1,h2))`, the same rule
/// the G4/G5 shift derivation uses).
///
/// Rows come from the baseline only: re-clustering the current rects would merge two baseline rows when controls grow
/// taller (the threshold scales with height), which reorders widgets that never moved past each other.
fn cluster_baseline_rows(mut items: Vec<OrderItem<'_>>) -> Vec<Vec<OrderItem<'_>>> {
    items.sort_by(|(ra, _, na), (rb, _, nb)| {
        let cya = ra[1] + ra[3] * 0.5;
        let cyb = rb[1] + rb[3] * 0.5;
        cya.total_cmp(&cyb).then_with(|| cmp_q4(ra, rb)).then_with(|| na.cmp(nb))
    });
    let mut rows: Vec<Vec<OrderItem<'_>>> = Vec::new();
    for item in items {
        let cy = item.0[1] + item.0[3] * 0.5;
        let h = item.0[3];
        let joins_last = rows.last().is_some_and(|row| {
            let first_r = row[0].0;
            let cy0 = first_r[1] + first_r[3] * 0.5;
            let h0 = first_r[3];
            (cy - cy0).abs() <= (4.0_f32).max(0.5 * h0.min(h))
        });
        if joins_last {
            rows.last_mut().unwrap().push(item);
        } else {
            rows.push(vec![item]);
        }
    }
    rows
}

/// Per row, the names sorted by baseline centre x and by current centre x.
///
/// Sorting by centre rather than left edge keeps a control that grows sideways under an overlapping neighbour in place.
/// Equal centres break by name only: a half-point y jitter or a centred growth must not flip two widgets that share a
/// centre. For widgets that do not overlap in x, centre order equals left-edge order, so side-by-side controls stay
/// guarded; G4/G5 pin every widget's position separately. Cross-row x order is not compared on purpose: a region can
/// hold several columns, and an allow entry that authorises a horizontal move is reviewed on its own.
fn row_name_sequences(rows: &mut [Vec<OrderItem<'_>>]) -> (Vec<Vec<String>>, Vec<Vec<String>>) {
    let mut base_rows = Vec::with_capacity(rows.len());
    let mut cur_rows = Vec::with_capacity(rows.len());
    for row in rows.iter_mut() {
        row.sort_by(|(ra, _, na), (rb, _, nb)| centre_x(ra).total_cmp(&centre_x(rb)).then_with(|| na.cmp(nb)));
        base_rows.push(row.iter().map(|(_, _, name)| name.to_string()).collect());
        row.sort_by(|(_, ca, na), (_, cb, nb)| centre_x(ca).total_cmp(&centre_x(cb)).then_with(|| na.cmp(nb)));
        cur_rows.push(row.iter().map(|(_, _, name)| name.to_string()).collect());
    }
    (base_rows, cur_rows)
}

/// Describe the first pair of widgets from DIFFERENT baseline rows whose baseline x-extents overlap and whose current
/// centre-y order no longer matches the baseline order (a tie counts as a change).
///
/// An allow entry can translate a widget out of its baseline row and into another one without changing any row's
/// sequence; this catches it. Only x-overlapping pairs are compared because a region can hold two columns (a Home
/// sidebar next to a content area), and rows of one column that grow may legitimately pass rows of the other in y.
fn vertical_order_violation(rows: &[Vec<OrderItem<'_>>]) -> Option<String> {
    let cy = |r: &[f32; 4]| r[1] + r[3] * 0.5;
    for (i, row_above) in rows.iter().enumerate() {
        for row_below in &rows[i + 1..] {
            for (b_up, c_up, n_up) in row_above {
                for (b_down, c_down, n_down) in row_below {
                    let overlap_x = b_up[0] < b_down[0] + b_down[2] && b_down[0] < b_up[0] + b_up[2];
                    if !overlap_x || cy(b_up) >= cy(b_down) {
                        continue;
                    }
                    if cy(c_up) >= cy(c_down) {
                        return Some(format!(
                            "'{n_up}' and '{n_down}' changed vertical order (baseline cy {} < {}, current cy {} >= {})",
                            cy(b_up),
                            cy(b_down),
                            cy(c_up),
                            cy(c_down)
                        ));
                    }
                }
            }
        }
    }
    None
}

/// Compare `cur` against `base` enforcing Rules G1–G5.
pub fn compare(base: &LayoutSnapshot, cur: &LayoutSnapshot, allow: &AllowList, virtualized_regions: &[String]) -> Result<CompareReport, String> {
    let case_id = if !cur.case.is_empty() { cur.case.as_str() } else { base.case.as_str() };

    if base.size != cur.size {
        return Err(format!("case {case_id}: viewport size changed from {:?} to {:?}", base.size, cur.size));
    }
    if (base.scale - cur.scale).abs() > 0.01 {
        return Err(format!("case {case_id}: scale changed from {} to {}", base.scale, cur.scale));
    }

    let mut report = CompareReport::default();

    // G1 — Regions: x and w exact (0.0 pt tolerance); y and h exact unless covered by an allow entry.
    let mut check_region_pair = |key: &str, b_reg: Option<[f32; 4]>, c_reg: Option<[f32; 4]>| -> Result<(), String> {
        if let (Some(b), Some(c)) = (b_reg, c_reg) {
            let matched = best_matching_entry(allow, case_id, key, None, b);
            if let Some((idx, _)) = matched {
                report.matched_allow_indices.insert(idx);
            }
            if b == c {
                return Ok(());
            }
            for (idx, field) in [(0, "x"), (2, "w")] {
                if b[idx] != c[idx] {
                    return Err(format!("G1 region mismatch in case {case_id} on {key}.{field}: baseline={}, current={}", b[idx], c[idx]));
                }
            }
            if (b[1] != c[1] || b[3] != c[3]) && matched.is_some_and(|(_, entry)| widget_satisfies_allow_bounds(entry, b, c, DerivedShifts::default())) {
                return Ok(());
            }
            for (idx, field) in [(1, "y"), (3, "h")] {
                if b[idx] != c[idx] {
                    return Err(format!("G1 region mismatch in case {case_id} on {key}.{field}: baseline={}, current={}", b[idx], c[idx]));
                }
            }
            return Ok(());
        }
        if b_reg == c_reg {
            return Ok(());
        }
        Err(format!("G1 region mismatch in case {case_id} for region '{key}': baseline={b_reg:?}, current={c_reg:?}"))
    };
    for key in REGION_KEYS {
        let b_reg = base.regions.get(key).copied().flatten();
        let c_reg = cur.regions.get(key).copied().flatten();
        check_region_pair(key, b_reg, c_reg)?;
    }
    for (key, b_val) in &base.regions {
        let c_val = cur.regions.get(key).copied().flatten();
        check_region_pair(key, *b_val, c_val)?;
    }

    // G2 — Overlays: same names and rects within ±0.5 pt unless matched by an allow entry.
    if base.overlays.len() != cur.overlays.len() {
        let b_names: Vec<&str> = base.overlays.iter().map(|o| o.name.as_str()).collect();
        let c_names: Vec<&str> = cur.overlays.iter().map(|o| o.name.as_str()).collect();
        return Err(format!("G2 overlay count/name mismatch in case {case_id}: baseline={b_names:?}, current={c_names:?}"));
    }
    for (b_ov, c_ov) in base.overlays.iter().zip(cur.overlays.iter()) {
        if b_ov.name != c_ov.name {
            return Err(format!("G2 overlay name mismatch in case {case_id}: baseline={:?}, current={:?}", b_ov.name, c_ov.name));
        }
        let ov_region = if b_ov.name.starts_with("overlay:") { b_ov.name.clone() } else { format!("overlay:{}", b_ov.name) };
        let covering = best_matching_entry(allow, case_id, &ov_region, Some(&b_ov.name), b_ov.rect);
        if let Some((idx, _)) = covering {
            report.matched_allow_indices.insert(idx);
        }
        if rects_differ_beyond_half_pt(b_ov.rect, c_ov.rect) {
            let allowed = covering.is_some_and(|(_, e)| widget_satisfies_allow_bounds(e, b_ov.rect, c_ov.rect, DerivedShifts::default()));
            if !allowed {
                return Err(format!("G2 overlay rect drift in case {case_id} overlay '{}': baseline={:?}, current={:?}", b_ov.name, b_ov.rect, c_ov.rect));
            }
        }
    }

    let mut base_unnamed_by_reg: BTreeMap<String, Vec<&UnnamedWidget>> = BTreeMap::new();
    let mut cur_unnamed_by_reg: BTreeMap<String, Vec<&UnnamedWidget>> = BTreeMap::new();
    for u in &base.unnamed {
        base_unnamed_by_reg.entry(u.region.clone()).or_default().push(u);
    }
    for u in &cur.unnamed {
        cur_unnamed_by_reg.entry(u.region.clone()).or_default().push(u);
    }

    // G3 — Named widgets: every baseline key must still exist with same count and region; new keys are reported.
    for name in cur.named.keys() {
        if !base.named.contains_key(name) {
            report.new_names.push(name.clone());
        }
    }
    for (name, b_w) in &base.named {
        let Some(c_w) = cur.named.get(name) else {
            return Err(format!("G3 missing baseline named widget '{name}' in case {case_id} (region '{}')", b_w.region));
        };
        if b_w.region != c_w.region {
            return Err(format!("G3 named widget '{name}' changed region in case {case_id}: baseline='{}', current='{}'", b_w.region, c_w.region));
        }
        if b_w.count != c_w.count {
            return Err(format!("G3 named widget '{name}' count changed in case {case_id}: baseline={}, current={}", b_w.count, c_w.count));
        }
        if let Some((idx, _)) = best_matching_entry(allow, case_id, &b_w.region, Some(name.as_str()), b_w.rect) {
            report.matched_allow_indices.insert(idx);
        }
    }

    // G4 — Unnamed widgets: check per-region inventory count and sense before building rows.
    let all_unnamed_regs: BTreeSet<String> = base_unnamed_by_reg.keys().chain(cur_unnamed_by_reg.keys()).cloned().collect();
    for reg in &all_unnamed_regs {
        let b_list = base_unnamed_by_reg.get(reg).cloned().unwrap_or_default();
        let c_list = cur_unnamed_by_reg.get(reg).cloned().unwrap_or_default();

        for (idx, e) in allow.entries.iter().enumerate() {
            if (e.case == case_id || e.case == "*") && &e.region == reg && e.inventory_max_delta.is_some() && (!b_list.is_empty() || !c_list.is_empty()) {
                report.matched_allow_indices.insert(idx);
            }
        }

        if b_list.len() != c_list.len() {
            let count_diff = (c_list.len() as isize - b_list.len() as isize).abs();
            let is_virt = virtualized_regions.iter().any(|vr| vr == reg);
            let mut inv_allowed = false;
            for (idx, e) in allow.entries.iter().enumerate() {
                if (e.case == case_id || e.case == "*") && &e.region == reg {
                    if let Some(max_delta) = e.inventory_max_delta {
                        if count_diff <= max_delta {
                            report.matched_allow_indices.insert(idx);
                            inv_allowed = true;
                            break;
                        }
                    }
                }
            }
            if !inv_allowed {
                return Err(format!(
                    "G4 unnamed widget count mismatch in case {case_id} region '{reg}': baseline={}, current={} (virtualized={is_virt})",
                    b_list.len(),
                    c_list.len()
                ));
            }
        }

        let shared_len = b_list.len().min(c_list.len());
        for (i, (b_u, c_u)) in b_list[..shared_len].iter().zip(c_list[..shared_len].iter()).enumerate() {
            if b_u.sense != c_u.sense {
                return Err(format!(
                    "G4 unnamed widget #{i} sense mismatch in case {case_id} region '{reg}': baseline={:?}, current={:?}",
                    b_u.sense, c_u.sense
                ));
            }
            if let Some((idx, _)) = best_matching_entry(allow, case_id, reg, None, b_u.rect) {
                report.matched_allow_indices.insert(idx);
            }
        }
    }

    // Build per-region G3 rows (named + unnamed widgets), range-check each row's own `dh`, and compute derived shifts for all anchors.
    struct RowWidget<'a> {
        label: String,
        name: Option<&'a str>,
        b_rect: [f32; 4],
        c_rect: [f32; 4],
    }
    let mut shifts_by_widget: BTreeMap<(String, String), DerivedShifts> = BTreeMap::new();
    let mut paired_by_region: BTreeMap<String, Vec<RowWidget<'_>>> = BTreeMap::new();
    for (name, b_w) in &base.named {
        if let Some(c_w) = cur.named.get(name) {
            if b_w.region == c_w.region {
                paired_by_region.entry(b_w.region.clone()).or_default().push(RowWidget {
                    label: name.clone(),
                    name: Some(name.as_str()),
                    b_rect: b_w.rect,
                    c_rect: c_w.rect,
                });
            }
        }
    }
    for (reg, b_list) in &base_unnamed_by_reg {
        if let Some(c_list) = cur_unnamed_by_reg.get(reg) {
            let shared_len = b_list.len().min(c_list.len());
            for (i, (b_u, c_u)) in b_list[..shared_len].iter().zip(c_list[..shared_len].iter()).enumerate() {
                paired_by_region.entry(reg.clone()).or_default().push(RowWidget {
                    label: format!("unnamed#{i}"),
                    name: None,
                    b_rect: b_u.rect,
                    c_rect: c_u.rect,
                });
            }
        }
    }
    for (reg, mut pairs) in paired_by_region {
        pairs.sort_by(|a, b| {
            let cya = a.b_rect[1] + a.b_rect[3] * 0.5;
            let cyb = b.b_rect[1] + b.b_rect[3] * 0.5;
            cya.total_cmp(&cyb).then_with(|| cmp_q4(&a.b_rect, &b.b_rect)).then_with(|| a.label.cmp(&b.label))
        });
        let mut rows: Vec<Vec<RowWidget<'_>>> = Vec::new();
        for w in pairs {
            let cy = w.b_rect[1] + w.b_rect[3] * 0.5;
            let h = w.b_rect[3];
            let joins_last = rows.last().is_some_and(|row| {
                let first_b = row[0].b_rect;
                let cy0 = first_b[1] + first_b[3] * 0.5;
                let h0 = first_b[3];
                (cy - cy0).abs() <= (4.0_f32).max(0.5 * h0.min(h))
            });
            if joins_last {
                rows.last_mut().unwrap().push(w);
            } else {
                rows.push(vec![w]);
            }
        }
        let mut row_dhs = Vec::with_capacity(rows.len());
        for (row_idx, row) in rows.iter_mut().enumerate() {
            row.sort_by(|a, b| a.b_rect[0].total_cmp(&b.b_rect[0]).then_with(|| cmp_q4(&a.b_rect, &b.b_rect)).then_with(|| a.label.cmp(&b.label)));
            let b_min_y = row.iter().map(|w| w.b_rect[1]).fold(f32::INFINITY, f32::min);
            let b_max_y = row.iter().map(|w| w.b_rect[1] + w.b_rect[3]).fold(f32::NEG_INFINITY, f32::max);
            let c_min_y = row.iter().map(|w| w.c_rect[1]).fold(f32::INFINITY, f32::min);
            let c_max_y = row.iter().map(|w| w.c_rect[1] + w.c_rect[3]).fold(f32::NEG_INFINITY, f32::max);
            let b_h = (b_max_y - b_min_y).max(0.0);
            let c_h = (c_max_y - c_min_y).max(0.0);
            let row_dh = c_h - b_h;

            let mut has_covering = false;
            let mut min_dh = f32::NEG_INFINITY;
            let mut max_dh = f32::INFINITY;
            for w in row.iter() {
                if let Some((idx, e)) = best_matching_entry(allow, case_id, &reg, w.name, w.b_rect) {
                    report.matched_allow_indices.insert(idx);
                    has_covering = true;
                    min_dh = min_dh.max(e.min_dh.unwrap_or(0.0));
                    max_dh = max_dh.min(e.max_dh.unwrap_or(0.0));
                }
            }
            if !has_covering {
                min_dh = 0.0;
                max_dh = 0.0;
            }
            if row_dh.abs() > 0.5 && (!has_covering || row_dh < min_dh - 0.5 || row_dh > max_dh + 0.5) {
                let row_names: Vec<&str> = row.iter().map(|w| w.label.as_str()).collect();
                return Err(format!(
                    "G3/G4 row #{row_idx} [{}] dh={row_dh} out of allowed [{min_dh}, {max_dh}] in case {case_id} region '{reg}': baseline_h={b_h}, current_h={c_h}",
                    row_names.join(", ")
                ));
            }

            row_dhs.push(row_dh);
        }

        for (row_idx, row) in rows.into_iter().enumerate() {
            let sum_dh_above: f32 = row_dhs[..row_idx].iter().sum();
            let sum_dh_below: f32 = row_dhs[(row_idx + 1)..].iter().sum();
            let gaps_above = row_idx;
            for (col_idx, w) in row.iter().enumerate() {
                // A banded entry scopes the sideways shift: only row-mates inside the band move the widget.
                let band = best_matching_entry(allow, case_id, &reg, w.name, w.b_rect).and_then(|(_, e)| e.x_band);
                let in_band = |o: &&RowWidget<'_>| band.is_none_or(|[lo, hi]| o.b_rect[0] >= lo && o.b_rect[0] <= hi);
                let sum_dw_left: f32 = row[..col_idx].iter().filter(in_band).map(|lw| lw.c_rect[2] - lw.b_rect[2]).sum();
                let sum_dw_right: f32 = row[(col_idx + 1)..].iter().filter(in_band).map(|rw| rw.c_rect[2] - rw.b_rect[2]).sum();
                shifts_by_widget.insert((reg.clone(), w.label.clone()), DerivedShifts { sum_dw_left, sum_dw_right, sum_dh_above, sum_dh_below, gaps_above });
            }
        }
    }

    let default_entry = AllowEntry::default();
    for (name, b_w) in &base.named {
        let Some(c_w) = cur.named.get(name) else {
            continue;
        };
        let covering = best_matching_entry(allow, case_id, &b_w.region, Some(name.as_str()), b_w.rect);
        if rects_differ_beyond_half_pt(b_w.rect, c_w.rect) {
            let shifts = shifts_by_widget.get(&(b_w.region.clone(), name.clone())).copied().unwrap_or_default();
            let size_changed = (c_w.rect[2] - b_w.rect[2]).abs() > 0.5 || (c_w.rect[3] - b_w.rect[3]).abs() > 0.5;
            let allowed = match covering {
                Some((_, e)) => widget_satisfies_allow_bounds(e, b_w.rect, c_w.rect, shifts),
                None if !size_changed => widget_satisfies_allow_bounds(&default_entry, b_w.rect, c_w.rect, shifts),
                None => false,
            };
            if !allowed {
                return Err(format!(
                    "G3 named widget '{name}' rect drift in case {case_id} (region '{}'): baseline={:?}, current={:?}",
                    b_w.region, b_w.rect, c_w.rect
                ));
            }
        }
    }

    for reg in all_unnamed_regs {
        let b_list = base_unnamed_by_reg.get(&reg).cloned().unwrap_or_default();
        let c_list = cur_unnamed_by_reg.get(&reg).cloned().unwrap_or_default();
        let shared_len = b_list.len().min(c_list.len());
        for (i, (b_u, c_u)) in b_list[..shared_len].iter().zip(c_list[..shared_len].iter()).enumerate() {
            let covering = best_matching_entry(allow, case_id, &reg, None, b_u.rect);
            if rects_differ_beyond_half_pt(b_u.rect, c_u.rect) {
                let shifts = shifts_by_widget.get(&(reg.clone(), format!("unnamed#{i}"))).copied().unwrap_or_default();
                let size_changed = (c_u.rect[2] - b_u.rect[2]).abs() > 0.5 || (c_u.rect[3] - b_u.rect[3]).abs() > 0.5;
                let allowed = match covering {
                    Some((_, e)) => widget_satisfies_allow_bounds(e, b_u.rect, c_u.rect, shifts),
                    None if !size_changed => widget_satisfies_allow_bounds(&default_entry, b_u.rect, c_u.rect, shifts),
                    None => false,
                };
                if !allowed {
                    return Err(format!("G4 unnamed widget #{i} rect drift in case {case_id} region '{reg}': baseline={:?}, current={:?}", b_u.rect, c_u.rect));
                }
            }
        }
    }

    // G3 Order — Per region, pair each baseline widget with its current rect (named by name, unnamed as "_" by index),
    // verify that the name sequence per BASELINE row is equal when sorted by baseline and by current centre x, and
    // that widgets in different rows whose baseline x-extents overlap keep their vertical order.
    let mut order_regions: BTreeSet<String> = base.named.values().map(|w| w.region.clone()).collect();
    order_regions.extend(base_unnamed_by_reg.keys().cloned());
    for reg in order_regions {
        let mut items: Vec<OrderItem<'_>> = Vec::new();
        for (name, b_w) in &base.named {
            if b_w.region == reg {
                if let Some(c_w) = cur.named.get(name) {
                    items.push((b_w.rect, c_w.rect, name.as_str()));
                }
            }
        }
        let b_u_list = base_unnamed_by_reg.get(&reg).map(Vec::as_slice).unwrap_or(&[]);
        let c_u_list = cur_unnamed_by_reg.get(&reg).map(Vec::as_slice).unwrap_or(&[]);
        let shared_len = b_u_list.len().min(c_u_list.len());
        for (b_u, c_u) in b_u_list[..shared_len].iter().zip(c_u_list[..shared_len].iter()) {
            items.push((b_u.rect, c_u.rect, "_"));
        }
        let mut rows = cluster_baseline_rows(items);
        let (base_rows, cur_rows) = row_name_sequences(&mut rows);
        if base_rows != cur_rows {
            return Err(format!("G3 row order mismatch in case {case_id} region '{reg}': baseline={base_rows:?}, current={cur_rows:?}"));
        }
        if let Some(detail) = vertical_order_violation(&rows) {
            return Err(format!("G3 row order mismatch in case {case_id} region '{reg}': {detail}"));
        }
    }

    // Rule 8 G2: scroll entries are reported, never failed.
    let base_scroll: BTreeMap<(&str, &str), bool> = base.scroll.iter().map(|s| ((s.region.as_str(), s.axis.as_str()), s.maybe)).collect();
    let cur_scroll: BTreeMap<(&str, &str), bool> = cur.scroll.iter().map(|s| ((s.region.as_str(), s.axis.as_str()), s.maybe)).collect();
    let all_scroll_keys: BTreeSet<(&str, &str)> = base_scroll.keys().chain(cur_scroll.keys()).copied().collect();
    for (reg, axis) in all_scroll_keys {
        let b_s = base_scroll.get(&(reg, axis)).copied();
        let c_s = cur_scroll.get(&(reg, axis)).copied();
        if b_s != c_s {
            report.scroll_changes.push(format!("{case_id}:{reg}:{axis} (baseline={b_s:?}, current={c_s:?})"));
        }
    }

    Ok(report)
}

pub fn check_stale_allow_entries_for_run(
    allow: &AllowList,
    matched_indices: &BTreeSet<usize>,
    scope_cases: Option<&BTreeSet<String>>,
    spec_filter: Option<&str>,
) -> Result<(), String> {
    for (idx, e) in allow.entries.iter().enumerate() {
        if let Some(scope) = scope_cases {
            if e.case != "*" && !scope.contains(&e.case) {
                continue;
            }
        } else if let Some(sp) = spec_filter {
            if e.case != "*" && e.spec != sp {
                continue;
            }
        }
        if !matched_indices.contains(&idx) {
            return Err(format!("stale allow.json[{idx}] entry (case='{}', region='{}', spec='{}'): matched 0 widgets", e.case, e.region, e.spec));
        }
    }
    Ok(())
}

pub fn check_stale_allow_entries(allow: &AllowList, matched_indices: &BTreeSet<usize>, scope_cases: Option<&BTreeSet<String>>) -> Result<(), String> {
    check_stale_allow_entries_for_run(allow, matched_indices, scope_cases, None)
}

pub fn compare_with_stale_check(
    base: &LayoutSnapshot,
    cur: &LayoutSnapshot,
    allow: &AllowList,
    virtualized_regions: &[String],
) -> Result<CompareReport, String> {
    let report = compare(base, cur, allow, virtualized_regions)?;
    let mut scope = BTreeSet::new();
    if !cur.case.is_empty() {
        scope.insert(cur.case.clone());
    } else if !base.case.is_empty() {
        scope.insert(base.case.clone());
    }
    check_stale_allow_entries(allow, &report.matched_allow_indices, Some(&scope))?;
    Ok(report)
}

pub fn load_and_compare_baseline(
    baseline_path: &Path,
    cur: &LayoutSnapshot,
    allow: &AllowList,
    virtualized_regions: &[String],
) -> Result<CompareReport, String> {
    if !baseline_path.exists() {
        return Err(format!(
            "missing layout baseline file: {} — run `DESIGNOS_LAYOUT_BLESS=1` or `python3 scripts/layout_baseline_capture.py --bless` to capture baselines",
            baseline_path.display()
        ));
    }
    let raw = std::fs::read_to_string(baseline_path).map_err(|e| format!("failed to read layout baseline {}: {e}", baseline_path.display()))?;
    let base: LayoutSnapshot = serde_json::from_str(&raw).map_err(|e| format!("failed to parse layout baseline {}: {e}", baseline_path.display()))?;
    compare(&base, cur, allow, virtualized_regions)
}

/// Test-only (`#1126` §3 rule 7 & rule 14): seeds `root` with the shipped starter (`DESIGN-OS Studio Starter`)
/// stamped with a fixed `modified` timestamp; never seeds the legacy showcase documents.
#[cfg(test)]
pub fn seed_library_at(root: &std::path::Path, modified: u64) -> Result<(), String> {
    let lib = designos_library::Library::open(root).map_err(|e| e.to_string())?;
    if let Some(entry) = lib.seed_starter(crate::home::load_starter_document).map_err(|e| e.to_string())? {
        if let Ok(doc) = lib.load_file(&entry.id) {
            crate::home::thumb::refresh(&lib, &entry.id, &doc);
        }
    }
    let projects_dir = root.join("projects");
    if let Ok(entries) = std::fs::read_dir(&projects_dir) {
        for entry in entries.flatten() {
            let manifest_path = entry.path().join("project.json");
            if let Ok(raw) = std::fs::read_to_string(&manifest_path) {
                if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&raw) {
                    if let Some(obj) = val.as_object_mut() {
                        obj.insert("created".into(), serde_json::json!(modified));
                        obj.insert("modified".into(), serde_json::json!(modified));
                        if let Some(files) = obj.get_mut("files").and_then(|v| v.as_array_mut()) {
                            for f in files.iter_mut() {
                                if let Some(fobj) = f.as_object_mut() {
                                    fobj.insert("created".into(), serde_json::json!(modified));
                                    fobj.insert("modified".into(), serde_json::json!(modified));
                                    fobj.insert("opened".into(), serde_json::json!(modified));
                                }
                            }
                        }
                    }
                    if let Ok(pretty) = serde_json::to_string_pretty(&val) {
                        let _ = std::fs::write(&manifest_path, pretty);
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Options;
    use crate::shell::Shell;
    use crate::theme;

    fn sample_snapshot(case: &str) -> LayoutSnapshot {
        let mut regions = BTreeMap::new();
        for key in REGION_KEYS {
            regions.insert(key.to_string(), None);
        }
        regions.insert("tab_bar".into(), Some([0.0, 0.0, 1440.0, 40.0]));
        regions.insert("rail".into(), Some([0.0, 40.0, 49.0, 860.0]));
        regions.insert("left_panel".into(), Some([49.0, 40.0, 241.0, 860.0]));
        regions.insert("canvas".into(), Some([290.0, 40.0, 909.0, 860.0]));
        regions.insert("right_panel".into(), Some([1199.0, 40.0, 241.0, 860.0]));

        let mut named = BTreeMap::new();
        named.insert("left-panel-resize-handle".into(), NamedWidget { region: "left_panel".into(), rect: [287.0, 40.0, 6.0, 860.0], count: 1 });
        named.insert("prop-row-0".into(), NamedWidget { region: "right_panel".into(), rect: [1210.0, 60.0, 200.0, 24.0], count: 1 });
        named.insert("prop-row-1".into(), NamedWidget { region: "right_panel".into(), rect: [1210.0, 90.0, 200.0, 24.0], count: 1 });
        named.insert("prop-row-2".into(), NamedWidget { region: "right_panel".into(), rect: [1210.0, 120.0, 200.0, 24.0], count: 1 });

        LayoutSnapshot {
            schema: SNAPSHOT_SCHEMA,
            case: case.into(),
            size: [1440, 900],
            scale: 1.0,
            regions,
            overlays: Vec::new(),
            named,
            unnamed: Vec::new(),
            scroll: Vec::new(),
        }
    }

    /// Test 1: `d-editor-default` regions; rail `RAIL_WIDTH + BORDER_WIDTH`; `named` holds `left-panel-resize-handle`.
    #[test]
    fn snapshot_reports_every_region_of_the_editor() {
        crate::widgets::clear_pass_records();
        let ctx = Context::default();
        let mut shell = Shell::build(&ctx, Options::default());
        shell.apply_launch_defaults(true);
        ctx.all_styles_mut(|s| s.debug.show_interactive_widgets = true);

        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::Vec2::new(1440.0, 900.0))), time: Some(0.05), ..Default::default() };
        for i in 0..8 {
            let mut r = raw.clone();
            r.time = Some((i + 1) as f64 * 0.05);
            let _ = ctx.run_ui(r, |ui| shell.draw(ui));
        }

        let mut snap = capture(&ctx, &shell.app.ui.regions);
        snap.case = "d-editor-default".into();

        assert_eq!(snap.size, [1440, 900]);
        for key in REGION_KEYS {
            assert!(snap.regions.contains_key(key), "missing region key {key}");
        }
        assert_eq!(snap.regions["title_bar"], None, "d-editor-default has screenshot_chrome=true so title_bar is None");
        assert!(snap.regions["tab_bar"].is_some(), "tab_bar must be present");
        let rail = snap.regions["rail"].expect("rail region must be present");
        assert_eq!(rail[2], theme::RAIL_WIDTH + theme::BORDER_WIDTH, "rail width must equal RAIL_WIDTH + BORDER_WIDTH");
        assert!(snap.regions["left_panel"].is_some(), "left_panel must be present");
        assert!(snap.regions["canvas"].is_some(), "canvas must be present");
        assert!(snap.regions["right_panel"].is_some(), "right_panel must be present");
        assert_eq!(snap.regions["notes"], None);
        assert_eq!(snap.regions["home"], None);
        assert_eq!(snap.regions["present"], None);
        assert_eq!(snap.regions["variables_full"], None);
        assert!(snap.named.contains_key("left-panel-resize-handle"), "named must contain left-panel-resize-handle, got {} named widgets", snap.named.len());
        assert_eq!(snap.named["left-panel-resize-handle"].region, "left_panel", "resize handle centre tie must go left to left_panel");
    }

    /// Test 3: Widening `left_panel` by +8 fails G1, naming `left_panel.w`.
    #[test]
    fn widening_the_left_panel_fails_g1() {
        let base = sample_snapshot("d-editor-default");
        let mut cur = base.clone();
        cur.regions.get_mut("left_panel").unwrap().as_mut().unwrap()[2] += 8.0;

        let err = compare(&base, &cur, &AllowList::default(), &[]).expect_err("widening left_panel by +8 must fail G1");
        assert!(err.contains("left_panel.w"), "G1 error must name left_panel.w, got: {err}");
    }

    /// Test 4: Growing a row by `dh +8` without an allow entry fails; with `max_dh: 8` it passes.
    #[test]
    fn growing_a_row_without_allow_fails_and_with_allow_passes() {
        let base = sample_snapshot("d-editor-default");
        let mut cur = base.clone();
        cur.named.get_mut("prop-row-2").unwrap().rect[3] += 8.0;

        let err = compare(&base, &cur, &AllowList::default(), &[]).expect_err("dh +8 without allow entry must fail");
        assert!(err.contains("prop-row-2"), "error must name prop-row-2, got: {err}");

        let allow = validate_allow_entries(vec![AllowEntry {
            case: "d-editor-default".into(),
            region: "right_panel".into(),
            name_prefix: Some("prop-row-2".into()),
            max_dh: Some(8.0),
            spec: "F".into(),
            reason: "R5: row height 24->32".into(),
            ..Default::default()
        }])
        .expect("valid allow list");

        compare_with_stale_check(&base, &cur, &allow, &[]).expect("dh +8 covered by max_dh=8 must pass");
    }

    /// Test 4b: Palette growing above bottom anchor (`y - 64`, `dh +64`, bottom edge fixed) passes with `anchor_y: bottom` and fails without.
    #[test]
    fn palette_growing_above_bottom_anchor_passes_with_allow() {
        let mut base = sample_snapshot("d-cmdk");
        base.named.insert("cmdk-palette".into(), NamedWidget { region: "canvas".into(), rect: [500.0, 400.0, 440.0, 260.0], count: 1 });
        let mut cur = base.clone();
        // Bottom edge stays at 660.0: top moves up by 64 (400 -> 336), height grows by 64 (260 -> 324).
        cur.named.get_mut("cmdk-palette").unwrap().rect = [500.0, 336.0, 440.0, 324.0];

        assert!(compare(&base, &cur, &AllowList::default(), &[]).is_err(), "palette growing above bottom anchor without allow must fail");

        let allow = validate_allow_entries(vec![AllowEntry {
            case: "d-cmdk".into(),
            region: "canvas".into(),
            name_prefix: Some("cmdk-palette".into()),
            anchor_y: Some(AnchorY::Bottom),
            max_dh: Some(64.0),
            spec: "D".into(),
            reason: "R5: command palette rows 24->32 above bottom toolbar".into(),
            ..Default::default()
        }])
        .expect("valid allow list");

        compare_with_stale_check(&base, &cur, &allow, &[]).expect("bottom-anchored palette growth with allow must pass");
    }

    /// Test 4c: Cumulative shift is derived (not a range):
    /// - rows 24->32 (+8 each, `h_base 24`, `[8,8]`): rows below shift by the derived sum and pass.
    /// - a header +16 fails on its own `dh`.
    /// - a widget 8 pt lower than the sum explains fails on its edge.
    #[test]
    fn cumulative_shift_is_derived_not_a_range() {
        let base = sample_snapshot("f-inspector");
        let mut cur = base.clone();
        // Row 0: y=60, h=24 -> y=60, h=32 (dh=+8, shift=0)
        cur.named.get_mut("prop-row-0").unwrap().rect = [1210.0, 60.0, 200.0, 32.0];
        // Row 1: y=90, h=24 -> y=98, h=32 (dh=+8, shift=+8 from row 0)
        cur.named.get_mut("prop-row-1").unwrap().rect = [1210.0, 98.0, 200.0, 32.0];
        // Row 2: y=120, h=24 -> y=136, h=32 (dh=+8, shift=+16 from rows 0+1)
        cur.named.get_mut("prop-row-2").unwrap().rect = [1210.0, 136.0, 200.0, 32.0];

        let allow = validate_allow_entries(vec![AllowEntry {
            case: "f-inspector".into(),
            region: "right_panel".into(),
            h_base: Some(24.0),
            min_dh: Some(8.0),
            max_dh: Some(8.0),
            spec: "F".into(),
            reason: "R5: editor rows 24->32".into(),
            ..Default::default()
        }])
        .expect("valid allow list");

        compare_with_stale_check(&base, &cur, &allow, &[]).expect("rows shifting by derived cumulative sum (+0, +8, +16) must pass");

        // A header in the same region growing by +16 (24 -> 40) must fail on its own dh.
        let mut cur_header_plus_16 = cur.clone();
        cur_header_plus_16.named.get_mut("prop-row-0").unwrap().rect[3] = 40.0;
        let err_dh = compare(&base, &cur_header_plus_16, &allow, &[]).expect_err("+16 row in a [8,8] region must fail on its own dh");
        assert!(err_dh.contains("prop-row-0"), "dh failure must name prop-row-0, got: {err_dh}");

        // A widget 8 pt lower than the derived sum explains (y=144 instead of 136) must fail on its edge.
        let mut cur_extra_drift = cur.clone();
        cur_extra_drift.named.get_mut("prop-row-2").unwrap().rect[1] = 144.0;
        let err_edge = compare(&base, &cur_extra_drift, &allow, &[]).expect_err("widget 8pt lower than derived cumulative shift must fail on edge");
        assert!(err_edge.contains("prop-row-2"), "edge failure must name prop-row-2, got: {err_edge}");
    }

    /// Round 5 Major A: Each row's own bounding-box `dh` (`c_h - b_h`) must also lie in its
    /// covering entry's `[min_dh, max_dh]` ±0.5, failing with an error that names the row.
    /// Grok's scenario: one row holding A `[0,0,40,24]` and B `[0,10,40,24]`, both grow to `h=32`,
    /// and B moves to `y=18`. Under `{h_base:24, min_dh:8, max_dh:8}` it must fail because the
    /// row's own bounding-box `dh` is +16 (`34 -> 50`).
    #[test]
    fn row_own_dh_is_range_checked_against_covering_entry() {
        let mut base = sample_snapshot("f-inspector");
        base.named.clear();
        base.named.insert("A".into(), NamedWidget { region: "right_panel".into(), rect: [0.0, 0.0, 40.0, 24.0], count: 1 });
        base.named.insert("B".into(), NamedWidget { region: "right_panel".into(), rect: [0.0, 10.0, 40.0, 24.0], count: 1 });

        let mut cur = base.clone();
        cur.named.get_mut("A").unwrap().rect = [0.0, 0.0, 40.0, 32.0];
        cur.named.get_mut("B").unwrap().rect = [0.0, 18.0, 40.0, 32.0];

        let allow = validate_allow_entries(vec![AllowEntry {
            case: "f-inspector".into(),
            region: "right_panel".into(),
            h_base: Some(24.0),
            min_dh: Some(8.0),
            max_dh: Some(8.0),
            spec: "F".into(),
            reason: "R5: row height 24->32".into(),
            ..Default::default()
        }])
        .expect("valid allow list");

        let err = compare(&base, &cur, &allow, &[]).expect_err("row with bounding-box dh=+16 under [8,8] must fail");
        assert!(
            err.contains("row") && err.contains("dh") && (err.contains("A") || err.contains("B")),
            "row dh failure must name the row and report dh out of range, got: {err}"
        );

        // Even if B has an explicit dy: 8 entry excusing its y shift, the row's own dh (+16) must still fail.
        let allow_with_b_dy = validate_allow_entries(vec![
            AllowEntry {
                case: "f-inspector".into(),
                region: "right_panel".into(),
                h_base: Some(24.0),
                min_dh: Some(8.0),
                max_dh: Some(8.0),
                spec: "F".into(),
                reason: "R5: row height 24->32".into(),
                ..Default::default()
            },
            AllowEntry {
                case: "f-inspector".into(),
                region: "right_panel".into(),
                name_prefix: Some("B".into()),
                dy: Some(8.0),
                min_dh: Some(8.0),
                max_dh: Some(8.0),
                spec: "F".into(),
                reason: "R5: B shifts +8 and grows 24->32".into(),
                ..Default::default()
            },
        ])
        .expect("valid allow list");

        let err2 = compare(&base, &cur, &allow_with_b_dy, &[]).expect_err("row with bounding-box dh=+16 must fail even when B's dy=+8 is allowed");
        assert!(
            err2.contains("row") && err2.contains("dh") && (err2.contains("A") || err2.contains("B")),
            "row dh failure must name the row and report dh out of range, got: {err2}"
        );

        // Round 6 Major 1: B has a wider max_dh (32) than A's best entry (h_base=24 -> [8,8]).
        // The row's dh (+16) must lie in the intersection of each widget's best entry ([8,8] ∩ [8,32] = [8,8]),
        // so it must fail naming the row and dh=16.
        let allow_wider_b = validate_allow_entries(vec![
            AllowEntry {
                case: "f-inspector".into(),
                region: "right_panel".into(),
                h_base: Some(24.0),
                min_dh: Some(8.0),
                max_dh: Some(8.0),
                spec: "F".into(),
                reason: "R5: row height 24->32".into(),
                ..Default::default()
            },
            AllowEntry {
                case: "f-inspector".into(),
                region: "right_panel".into(),
                name_prefix: Some("B".into()),
                dy: Some(8.0),
                min_dh: Some(8.0),
                max_dh: Some(32.0),
                spec: "F".into(),
                reason: "R5: B shifts +8 and allows up to +32".into(),
                ..Default::default()
            },
        ])
        .expect("valid allow list");

        let err3 =
            compare(&base, &cur, &allow_wider_b, &[]).expect_err("row with bounding-box dh=+16 must fail when A's best entry is [8,8] even if B allows [8,32]");
        assert!(
            err3.contains("row") && err3.contains("dh=16") && err3.contains("A") && err3.contains("B"),
            "row dh failure must name the row and dh=16, got: {err3}"
        );
    }

    /// Round 5 Major B: Derived cumulative shift covers `anchor_x: left` (sum of `dw` to the left
    /// in the same G3 row), `anchor_x: right` (sum of `dw` to the right in the same G3 row), and
    /// `anchor_y: bottom` (sum of `dh` of the rows below), while `centre` keeps today's rule.
    #[test]
    fn derived_shift_covers_left_right_and_bottom_anchors() {
        // 1. anchor_x: left — left button widens 32->40 under max_dw: 8, right button moves x +8: passes; +12 fails on edge.
        let mut base_x = sample_snapshot("f-inspector");
        base_x.named.retain(|k, _| !k.starts_with("prop-row-"));
        base_x.named.insert("btn-left".into(), NamedWidget { region: "right_panel".into(), rect: [1210.0, 60.0, 32.0, 24.0], count: 1 });
        base_x.named.insert("btn-right".into(), NamedWidget { region: "right_panel".into(), rect: [1250.0, 60.0, 32.0, 24.0], count: 1 });

        let mut cur_x_ok = base_x.clone();
        cur_x_ok.named.get_mut("btn-left").unwrap().rect = [1210.0, 60.0, 40.0, 24.0];
        cur_x_ok.named.get_mut("btn-right").unwrap().rect = [1258.0, 60.0, 32.0, 24.0];

        let allow_x_left = validate_allow_entries(vec![AllowEntry {
            case: "f-inspector".into(),
            region: "right_panel".into(),
            h_base: Some(24.0),
            max_dw: Some(8.0),
            spec: "F".into(),
            reason: "R5: left button widens 32->40".into(),
            ..Default::default()
        }])
        .expect("valid allow list");

        compare_with_stale_check(&base_x, &cur_x_ok, &allow_x_left, &[]).expect("left button widening 32->40 and right button shifting x +8 must pass");

        let mut cur_x_bad = cur_x_ok.clone();
        cur_x_bad.named.get_mut("btn-right").unwrap().rect[0] = 1262.0; // +12 instead of +8
        let err_x = compare(&base_x, &cur_x_bad, &allow_x_left, &[]).expect_err("right button moving x +12 when left button widened +8 must fail on edge");
        assert!(err_x.contains("btn-right"), "edge failure must name btn-right, got: {err_x}");

        // 2. anchor_x: right — right button widens 32->40 anchored right, left button shifts x -8: passes; -12 fails on edge.
        let mut base_xr = sample_snapshot("f-inspector");
        base_xr.named.retain(|k, _| !k.starts_with("prop-row-"));
        base_xr.named.insert("btn-left".into(), NamedWidget { region: "right_panel".into(), rect: [1328.0, 60.0, 32.0, 24.0], count: 1 });
        base_xr.named.insert("btn-right".into(), NamedWidget { region: "right_panel".into(), rect: [1368.0, 60.0, 32.0, 24.0], count: 1 });

        let mut cur_xr_ok = base_xr.clone();
        cur_xr_ok.named.get_mut("btn-right").unwrap().rect = [1360.0, 60.0, 40.0, 24.0];
        cur_xr_ok.named.get_mut("btn-left").unwrap().rect = [1320.0, 60.0, 32.0, 24.0];

        let allow_x_right = validate_allow_entries(vec![AllowEntry {
            case: "f-inspector".into(),
            region: "right_panel".into(),
            h_base: Some(24.0),
            anchor_x: Some(AnchorX::Right),
            max_dw: Some(8.0),
            spec: "F".into(),
            reason: "R5: right-anchored button widens 32->40".into(),
            ..Default::default()
        }])
        .expect("valid allow list");

        compare_with_stale_check(&base_xr, &cur_xr_ok, &allow_x_right, &[])
            .expect("right-anchored button widening 32->40 and left neighbor shifting x -8 must pass");

        let mut cur_xr_bad = cur_xr_ok.clone();
        cur_xr_bad.named.get_mut("btn-left").unwrap().rect[0] = 1316.0; // -12 instead of -8
        let err_xr = compare(&base_xr, &cur_xr_bad, &allow_x_right, &[]).expect_err("left button moving x -12 when right button widened +8 must fail on edge");
        assert!(err_xr.contains("btn-left"), "edge failure must name btn-left, got: {err_xr}");

        // 3. anchor_y: bottom — mirror of top-anchor test: rows 24->32 (+8 each) stack upward from bottom.
        let base_yb = sample_snapshot("f-inspector");
        let mut cur_yb = base_yb.clone();
        // Row 2 (bottom): y=120, h=24 (bottom=144) -> y=112, h=32 (bottom=144, dh=+8, shift_below=0)
        cur_yb.named.get_mut("prop-row-2").unwrap().rect = [1210.0, 112.0, 200.0, 32.0];
        // Row 1 (middle): y=90, h=24 (bottom=114) -> y=74, h=32 (bottom=106, dh=+8, shift_below=-8 from row 2)
        cur_yb.named.get_mut("prop-row-1").unwrap().rect = [1210.0, 74.0, 200.0, 32.0];
        // Row 0 (top): y=60, h=24 (bottom=84) -> y=36, h=32 (bottom=68, dh=+8, shift_below=-16 from rows 1+2)
        cur_yb.named.get_mut("prop-row-0").unwrap().rect = [1210.0, 36.0, 200.0, 32.0];

        let allow_yb = validate_allow_entries(vec![AllowEntry {
            case: "f-inspector".into(),
            region: "right_panel".into(),
            h_base: Some(24.0),
            anchor_y: Some(AnchorY::Bottom),
            min_dh: Some(8.0),
            max_dh: Some(8.0),
            spec: "F".into(),
            reason: "R5: bottom-anchored editor rows 24->32".into(),
            ..Default::default()
        }])
        .expect("valid allow list");

        compare_with_stale_check(&base_yb, &cur_yb, &allow_yb, &[]).expect("bottom-anchored rows shifting by derived cumulative sum (0, -8, -16) must pass");

        // Bottom row growing by +16 (24 -> 40) must fail on its own dh.
        let mut cur_yb_plus16 = cur_yb.clone();
        cur_yb_plus16.named.get_mut("prop-row-2").unwrap().rect = [1210.0, 104.0, 200.0, 40.0];
        let err_yb_dh = compare(&base_yb, &cur_yb_plus16, &allow_yb, &[]).expect_err("+16 bottom-anchored row in a [8,8] region must fail on its own dh");
        assert!(err_yb_dh.contains("prop-row-2"), "dh failure must name prop-row-2, got: {err_yb_dh}");

        // Top row shifting 8pt higher than derived sum explains (y=28 instead of 36) must fail on its edge.
        let mut cur_yb_extra = cur_yb.clone();
        cur_yb_extra.named.get_mut("prop-row-0").unwrap().rect[1] = 28.0;
        let err_yb_edge = compare(&base_yb, &cur_yb_extra, &allow_yb, &[]).expect_err("widget 8pt higher than derived bottom-anchor shift must fail on edge");
        assert!(err_yb_edge.contains("prop-row-0"), "edge failure must name prop-row-0, got: {err_yb_edge}");
    }

    /// Round 4 Major 1: Three unnamed inspector rows go 24->32 and step down by +0, +8, +16
    /// under `{h_base: 24, min_dh: 8, max_dh: 8}`; G4 applies the cumulative row-box `dh` shift.
    #[test]
    fn unnamed_rows_shift_by_cumulative_row_box_dh() {
        let mut base = sample_snapshot("f-inspector");
        base.named.retain(|k, _| !k.starts_with("prop-row-"));
        base.unnamed = vec![
            UnnamedWidget { region: "right_panel".into(), rect: [1210.0, 60.0, 90.0, 24.0], sense: vec!["click".into()] },
            UnnamedWidget { region: "right_panel".into(), rect: [1310.0, 60.0, 90.0, 24.0], sense: vec!["click".into()] },
            UnnamedWidget { region: "right_panel".into(), rect: [1210.0, 90.0, 200.0, 24.0], sense: vec!["click".into()] },
            UnnamedWidget { region: "right_panel".into(), rect: [1210.0, 120.0, 200.0, 24.0], sense: vec!["click".into()] },
        ];

        let mut cur = base.clone();
        // Row 0 has two unnamed widgets at y=60, h=24 -> y=60, h=32 (row-box dh = +8, not +16).
        cur.unnamed[0].rect = [1210.0, 60.0, 90.0, 32.0];
        cur.unnamed[1].rect = [1310.0, 60.0, 90.0, 32.0];
        // Row 1 shifts down by +8 (90 -> 98) and grows 24 -> 32.
        cur.unnamed[2].rect = [1210.0, 98.0, 200.0, 32.0];
        // Row 2 shifts down by +16 (120 -> 136) and grows 24 -> 32.
        cur.unnamed[3].rect = [1210.0, 136.0, 200.0, 32.0];

        let allow = validate_allow_entries(vec![AllowEntry {
            case: "f-inspector".into(),
            region: "right_panel".into(),
            h_base: Some(24.0),
            min_dh: Some(8.0),
            max_dh: Some(8.0),
            spec: "F".into(),
            reason: "R5: unnamed editor rows 24->32".into(),
            ..Default::default()
        }])
        .expect("valid allow list");

        compare_with_stale_check(&base, &cur, &allow, &[]).expect("unnamed rows shifting by cumulative row-box dh (+0, +8, +16) must pass G4");

        let mut cur_extra = cur.clone();
        cur_extra.unnamed[3].rect[1] = 144.0;
        let err = compare(&base, &cur_extra, &allow, &[]).expect_err("unnamed row 8pt below derived sum must fail G4");
        assert!(err.contains("G4"), "expected G4 error, got: {err}");
    }

    /// Round 4 Major 2: `gap_dy` adds `gaps_above * gap_dy` into `expected_y` with a fixed ±0.5 pt tolerance:
    /// - A widget with no row above it (`gaps_above = 0`) moving ±8 pt under `{gap_dy: 8}` fails.
    /// - Rows 0, 1, 2 (`gaps_above = 0, 1, 2`) shifting by `+0, +8, +16` under `{gap_dy: 8}` pass;
    ///   shifting row 1 by 0 or -8 fails.
    #[test]
    fn gap_dy_shifts_expected_y_per_row_gap_with_half_pt_tolerance() {
        let base = sample_snapshot("f-inspector");
        let allow = validate_allow_entries(vec![AllowEntry {
            case: "f-inspector".into(),
            region: "right_panel".into(),
            name_prefix: Some("prop-row-".into()),
            gap_dy: Some(8.0),
            spec: "F".into(),
            reason: "R5: inspector row gap +8".into(),
            ..Default::default()
        }])
        .expect("valid allow list");

        // Row 0 has no row above it (`gaps_above = 0`), so moving it by +8 or -8 must fail.
        let mut cur_row0_plus8 = base.clone();
        cur_row0_plus8.named.get_mut("prop-row-0").unwrap().rect[1] += 8.0;
        assert!(compare(&base, &cur_row0_plus8, &allow, &[]).is_err(), "widget with 0 rows above it must not be allowed to move +8pt under gap_dy=8");

        let mut cur_row0_minus8 = base.clone();
        cur_row0_minus8.named.get_mut("prop-row-0").unwrap().rect[1] -= 8.0;
        assert!(compare(&base, &cur_row0_minus8, &allow, &[]).is_err(), "widget with 0 rows above it must not be allowed to move -8pt under gap_dy=8");

        // Rows 0, 1, 2 have 0, 1, 2 row gaps above them -> expected y shifts of +0, +8, +16.
        let mut cur_valid = base.clone();
        cur_valid.named.get_mut("prop-row-0").unwrap().rect[1] = 60.0;
        cur_valid.named.get_mut("prop-row-1").unwrap().rect[1] = 98.0;
        cur_valid.named.get_mut("prop-row-2").unwrap().rect[1] = 136.0;
        compare_with_stale_check(&base, &cur_valid, &allow, &[]).expect("rows shifting by gaps_above * gap_dy (+0, +8, +16) must pass");

        // Row 1 moving by -8 (wrong direction) must fail even though |dy| == 8.
        let mut cur_wrong_dir = cur_valid.clone();
        cur_wrong_dir.named.get_mut("prop-row-1").unwrap().rect[1] = 82.0;
        assert!(compare(&base, &cur_wrong_dir, &allow, &[]).is_err(), "row 1 moving -8pt instead of +8pt under gap_dy=8 must fail");

        // Round 6 Minor 1: `gaps_above * gap_dy` also applies to `anchor_y: centre` and `anchor_y: bottom`.
        // One row above (`prop-row-0`), and an unchanged-size widget (`prop-row-1`) that moves +8 passes.
        let mut base_two = base.clone();
        base_two.named.remove("prop-row-2");
        let mut cur_two_plus8 = base_two.clone();
        cur_two_plus8.named.get_mut("prop-row-1").unwrap().rect[1] += 8.0; // 90 -> 98, size unchanged (200x24)

        for anchor in [AnchorY::Centre, AnchorY::Bottom] {
            let allow_anchor = validate_allow_entries(vec![AllowEntry {
                case: "f-inspector".into(),
                region: "right_panel".into(),
                name_prefix: Some("prop-row-".into()),
                anchor_y: Some(anchor),
                gap_dy: Some(8.0),
                spec: "F".into(),
                reason: "R5: inspector row gap +8 under centre/bottom anchor".into(),
                ..Default::default()
            }])
            .expect("valid allow list");

            compare_with_stale_check(&base_two, &cur_two_plus8, &allow_anchor, &[])
                .unwrap_or_else(|e| panic!("anchor_y={anchor:?} with gap_dy=8 and 1 row above moving +8 must pass: {e}"));

            let mut cur_two_plus16 = cur_two_plus8.clone();
            cur_two_plus16.named.get_mut("prop-row-1").unwrap().rect[1] += 8.0; // +16 total
            assert!(compare(&base_two, &cur_two_plus16, &allow_anchor, &[]).is_err(), "anchor_y={anchor:?} with gap_dy=8 and 1 row above moving +16 must fail");
        }
    }

    /// Round 4 Major 3: `inventory_max_delta` forgives only the count delta and still compares
    /// the shared prefix (leading `min(n)` widgets) for sense and rect drift.
    #[test]
    fn inventory_max_delta_forgives_count_only_and_compares_shared_prefix() {
        let mut base = sample_snapshot("f-inspector");
        base.unnamed = vec![
            UnnamedWidget { region: "right_panel".into(), rect: [1210.0, 200.0, 200.0, 24.0], sense: vec!["click".into()] },
            UnnamedWidget { region: "right_panel".into(), rect: [1210.0, 230.0, 200.0, 24.0], sense: vec!["click".into()] },
            UnnamedWidget { region: "right_panel".into(), rect: [1210.0, 260.0, 200.0, 24.0], sense: vec!["click".into()] },
        ];

        let allow = validate_allow_entries(vec![AllowEntry {
            case: "f-inspector".into(),
            region: "right_panel".into(),
            inventory_max_delta: Some(5),
            spec: "F".into(),
            reason: "R5: virtualized row count delta".into(),
            ..Default::default()
        }])
        .expect("valid allow list");

        // Adding 1 widget at the tail while keeping the leading 3 identical must pass.
        let mut cur_ok = base.clone();
        cur_ok.unnamed.push(UnnamedWidget { region: "right_panel".into(), rect: [1210.0, 290.0, 200.0, 24.0], sense: vec!["click".into()] });
        compare_with_stale_check(&base, &cur_ok, &allow, &["right_panel".into()]).expect("valid count delta with identical leading min(n) must pass");

        // Moving one of the leading min(n) widgets without rect bounds must fail G4 even when count delta <= inventory_max_delta.
        let mut cur_moved = cur_ok.clone();
        cur_moved.unnamed[1].rect[1] += 20.0;
        let err_rect = compare(&base, &cur_moved, &allow, &["right_panel".into()])
            .expect_err("moving a widget in the shared prefix under inventory_max_delta must fail G4");
        assert!(err_rect.contains("G4"), "expected G4 rect error, got: {err_rect}");

        // Changing sense of one of the leading min(n) widgets must fail G4 even when count delta <= inventory_max_delta.
        let mut cur_sense = cur_ok.clone();
        cur_sense.unnamed[0].sense = vec!["drag".into()];
        let err_sense = compare(&base, &cur_sense, &allow, &["right_panel".into()])
            .expect_err("changing sense in the shared prefix under inventory_max_delta must fail G4");
        assert!(err_sense.contains("G4") && err_sense.contains("sense"), "expected G4 sense error, got: {err_sense}");

        // Building a count delta above inventory_max_delta (delta=6 > 5) must fail with the G4 count error.
        let mut cur_over_delta = base.clone();
        for i in 0..6 {
            cur_over_delta.unnamed.push(UnnamedWidget {
                region: "right_panel".into(),
                rect: [1210.0, 290.0 + (i as f32) * 30.0, 200.0, 24.0],
                sense: vec!["click".into()],
            });
        }
        let err_count =
            compare(&base, &cur_over_delta, &allow, &["right_panel".into()]).expect_err("count delta above inventory_max_delta must fail with G4 count error");
        assert!(err_count.contains("G4 unnamed widget count mismatch"), "expected G4 count mismatch error, got: {err_count}");
    }

    /// Round 4 Major 4: An allow entry is counted as matched when it is the covering entry for a
    /// widget that exists (even with 0 drift), and staleness is checked once over the union of a run
    /// (`"*"` is stale only when no case in the run used it).
    #[test]
    fn stale_check_counts_covering_entries_and_runs_over_union_of_run() {
        // 1. Case-specific entry whose covered widgets stay within 0.5pt (0 drift) must count as matched.
        let base_ed = sample_snapshot("d-editor-default");
        let cur_ed = base_ed.clone();
        let allow_case = validate_allow_entries(vec![AllowEntry {
            case: "d-editor-default".into(),
            region: "right_panel".into(),
            name_prefix: Some("prop-row-".into()),
            max_dh: Some(8.0),
            spec: "F".into(),
            reason: "R5: row height 24->32".into(),
            ..Default::default()
        }])
        .expect("valid allow list");
        let rep_ed = compare_with_stale_check(&base_ed, &cur_ed, &allow_case, &[])
            .expect("covering entry for existing widgets with 0 drift must not be flagged as stale");
        assert!(rep_ed.matched_allow_indices.contains(&0));

        // 2. Wildcard entry across a multi-case run (`f-inspector` with drift, `d-editor-default` with 0 drift,
        //    and `h-home` with no right_panel widgets): stale check over the union of the run passes.
        let base_insp = sample_snapshot("f-inspector");
        let mut cur_insp = base_insp.clone();
        cur_insp.named.get_mut("prop-row-0").unwrap().rect = [1210.0, 60.0, 200.0, 32.0];
        cur_insp.named.get_mut("prop-row-1").unwrap().rect = [1210.0, 98.0, 200.0, 32.0];
        cur_insp.named.get_mut("prop-row-2").unwrap().rect = [1210.0, 136.0, 200.0, 32.0];

        let mut base_home = sample_snapshot("h-home");
        base_home.named.clear();
        let cur_home = base_home.clone();

        let allow_wild = validate_allow_entries(vec![AllowEntry {
            case: "*".into(),
            region: "right_panel".into(),
            name_prefix: Some("prop-row-".into()),
            max_dh: Some(8.0),
            spec: "F".into(),
            reason: "R5: row height 24->32".into(),
            ..Default::default()
        }])
        .expect("valid wildcard allow list");

        let mut union_matched = BTreeSet::new();
        let mut scope_cases = BTreeSet::new();
        for (b, c) in [(&base_insp, &cur_insp), (&base_ed, &cur_ed), (&base_home, &cur_home)] {
            let r = compare(b, c, &allow_wild, &[]).expect("case compare must succeed");
            union_matched.extend(r.matched_allow_indices);
            scope_cases.insert(b.case.clone());
        }
        check_stale_allow_entries_for_run(&allow_wild, &union_matched, Some(&scope_cases), Some("F"))
            .expect("wildcard entry matched in at least one case of the run must not be stale");

        // If no case in the run has any widget covered by the wildcard entry, the run-level stale check fails.
        let mut empty_matched = BTreeSet::new();
        let mut home_only_scope = BTreeSet::new();
        let r_home = compare(&base_home, &cur_home, &allow_wild, &[]).expect("home compare");
        empty_matched.extend(r_home.matched_allow_indices);
        home_only_scope.insert("h-home".into());
        let err = check_stale_allow_entries_for_run(&allow_wild, &empty_matched, Some(&home_only_scope), Some("F"))
            .expect_err("wildcard entry unused by every case in the run must fail as stale");
        assert!(err.contains("stale allow.json[0]"), "unexpected error: {err}");

        // 3. Round 5 Major C (Grok's scenario): `{case:"*", spec:"D", region:"right_panel", name_prefix:"prop-row-", max_dh:8}`
        //    whose only match is an F case, checked under `check_stale_allow_entries_for_run(..., Some("F"))`.
        let allow_d_wild = validate_allow_entries(vec![AllowEntry {
            case: "*".into(),
            region: "right_panel".into(),
            name_prefix: Some("prop-row-".into()),
            max_dh: Some(8.0),
            spec: "D".into(),
            reason: "R5: row height 24->32".into(),
            ..Default::default()
        }])
        .expect("valid wildcard allow list with spec D");

        let mut f_scope = BTreeSet::new();
        f_scope.insert("f-inspector".into());
        let rep_f = compare(&base_insp, &cur_insp, &allow_d_wild, &[]).expect("compare applies spec D wildcard entry to F case");
        assert!(rep_f.matched_allow_indices.contains(&0));
        check_stale_allow_entries_for_run(&allow_d_wild, &rep_f.matched_allow_indices, Some(&f_scope), Some("F"))
            .expect("spec D wildcard entry matched by F case in Some(\"F\") run must pass");

        let mut base_f_no_match = base_insp.clone();
        base_f_no_match.named.clear();
        let cur_f_no_match = base_f_no_match.clone();
        let rep_f_empty = compare(&base_f_no_match, &cur_f_no_match, &allow_d_wild, &[]).expect("compare with no matching widgets");
        let err_d_in_f = check_stale_allow_entries_for_run(&allow_d_wild, &rep_f_empty.matched_allow_indices, Some(&f_scope), Some("F"))
            .expect_err("spec D wildcard entry that compare can apply in Some(\"F\") run must fail as stale when unmatched");
        assert!(err_d_in_f.contains("stale allow.json[0]"), "unexpected error: {err_d_in_f}");
    }

    /// Round 4 Minor 5: Overlay names are `overlay:{order}/{index}` in `areas().order()`,
    /// contain no egui `Id`, and renaming an area's `Id` without moving its rect passes G2.
    #[test]
    fn overlay_names_use_areas_order_and_omit_egui_id() {
        let capture_with_ids = |id_mid: &'static str, id_fg0: &'static str, id_fg1: &'static str| -> LayoutSnapshot {
            crate::widgets::clear_pass_records();
            let ctx = Context::default();
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::Vec2::new(1440.0, 900.0))), ..Default::default() };
            for _ in 0..2 {
                let _ = ctx.run_ui(raw.clone(), |ui| {
                    egui::Area::new(egui::Id::new(id_mid)).order(egui::Order::Middle).fixed_pos(Pos2::new(100.0, 100.0)).show(ui.ctx(), |ui| {
                        ui.allocate_exact_size(egui::Vec2::new(120.0, 80.0), egui::Sense::click());
                    });
                    egui::Area::new(egui::Id::new(id_fg0)).order(egui::Order::Foreground).fixed_pos(Pos2::new(300.0, 100.0)).show(ui.ctx(), |ui| {
                        ui.allocate_exact_size(egui::Vec2::new(140.0, 90.0), egui::Sense::click());
                    });
                    egui::Area::new(egui::Id::new(id_fg1)).order(egui::Order::Foreground).fixed_pos(Pos2::new(500.0, 100.0)).show(ui.ctx(), |ui| {
                        ui.allocate_exact_size(egui::Vec2::new(160.0, 100.0), egui::Sense::click());
                    });
                });
            }
            let mut snap = capture(&ctx, &ShellRegions::default());
            snap.case = "d-overlay-test".into();
            snap
        };

        let snap_a = capture_with_ids("popup-mid-alpha", "popup-fg-beta", "popup-fg-gamma");
        let names_a: Vec<&str> = snap_a.overlays.iter().map(|o| o.name.as_str()).collect();
        assert_eq!(
            names_a,
            vec!["overlay:Middle/0", "overlay:Foreground/0", "overlay:Foreground/1"],
            "overlay names must follow areas().order() as overlay:{{order}}/{{index}} with no egui Id"
        );
        for ov in &snap_a.overlays {
            assert!(!ov.name.contains("Id("), "overlay name must not contain egui Id: {}", ov.name);
        }

        let snap_b = capture_with_ids("renamed-mid-1", "renamed-fg-2", "renamed-fg-3");
        compare(&snap_a, &snap_b, &AllowList::default(), &[]).expect("renaming area Ids without moving overlay rects must pass G2");
    }

    /// Test 4d: 24->32 makes the right panel overflow: a `scroll` entry appears; G2 passes.
    #[test]
    fn inspector_overflow_does_not_fail_g2() {
        let base = sample_snapshot("f-inspector");
        let mut cur = base.clone();
        cur.named.get_mut("prop-row-0").unwrap().rect = [1210.0, 60.0, 200.0, 32.0];
        cur.named.get_mut("prop-row-1").unwrap().rect = [1210.0, 98.0, 200.0, 32.0];
        cur.named.get_mut("prop-row-2").unwrap().rect = [1210.0, 136.0, 200.0, 32.0];
        cur.scroll.push(ScrollGeom { region: "right_panel".into(), axis: "y".into(), maybe: false });

        let allow = validate_allow_entries(vec![AllowEntry {
            case: "f-inspector".into(),
            region: "right_panel".into(),
            h_base: Some(24.0),
            min_dh: Some(8.0),
            max_dh: Some(8.0),
            spec: "F".into(),
            reason: "R5: editor rows 24->32".into(),
            ..Default::default()
        }])
        .expect("valid allow list");

        let rep = compare_with_stale_check(&base, &cur, &allow, &[]).expect("new scroll entry on overflow must not fail G2");
        assert!(!rep.scroll_changes.is_empty(), "CompareReport must report scroll change without failing G2");
    }

    /// Test 4e: Centred bar widening by 12 (`x - 6`, `w + 12`, each edge moves ±6) passes with `anchor_x: centre` and fails without.
    #[test]
    fn centred_bar_widening_by_12_passes_with_anchor_x_centre() {
        let mut base = sample_snapshot("d-editor-default");
        base.named.insert("contextual-bar".into(), NamedWidget { region: "canvas".into(), rect: [680.0, 820.0, 128.0, 32.0], count: 1 });
        let mut cur = base.clone();
        cur.named.get_mut("contextual-bar").unwrap().rect = [674.0, 820.0, 140.0, 32.0];

        assert!(compare(&base, &cur, &AllowList::default(), &[]).is_err(), "centred bar widening by 12 without allow entry must fail G4");

        let allow = validate_allow_entries(vec![AllowEntry {
            case: "d-editor-default".into(),
            region: "canvas".into(),
            name_prefix: Some("contextual-bar".into()),
            anchor_x: Some(AnchorX::Centre),
            max_dw: Some(12.0),
            spec: "D".into(),
            reason: "R5: contextual bar widened 128->140 centred".into(),
            ..Default::default()
        }])
        .expect("valid allow list");

        compare_with_stale_check(&base, &cur, &allow, &[]).expect("centred bar widening by 12 with anchor_x=centre must pass");
    }

    /// Test 5: Reordering two buttons in the same row fails G3.
    #[test]
    fn reordering_two_buttons_fails_g3() {
        let mut base = sample_snapshot("d-editor-default");
        base.named.insert("btn-align-left".into(), NamedWidget { region: "right_panel".into(), rect: [1210.0, 200.0, 28.0, 28.0], count: 1 });
        base.named.insert("btn-align-right".into(), NamedWidget { region: "right_panel".into(), rect: [1244.0, 200.0, 28.0, 28.0], count: 1 });
        let mut cur = base.clone();
        cur.named.get_mut("btn-align-left").unwrap().rect[0] = 1244.0;
        cur.named.get_mut("btn-align-right").unwrap().rect[0] = 1210.0;

        let err = compare(&base, &cur, &AllowList::default(), &[]).expect_err("swapping two buttons must fail G3");
        assert!(err.contains("G3"), "reordering error must cite G3, got: {err}");
    }

    /// Two toolbar rows of two 28 pt controls whose centres sit 16 pt apart (separate rows at threshold 14).
    /// In the current snapshot every control is 36 pt tall and centred on its old box, so the centres stay 16 pt
    /// apart but the row threshold becomes 18: clustering the current rects on their own would merge the rows.
    fn taller_toolbar_rows() -> (LayoutSnapshot, LayoutSnapshot) {
        let mut base = sample_snapshot("d-editor-default");
        base.named.retain(|k, _| !k.starts_with("prop-row-"));
        for (name, rect) in [
            ("tool-a1", [1210.0, 100.0, 28.0, 28.0]),
            ("tool-a2", [1250.0, 100.0, 28.0, 28.0]),
            ("tool-b1", [1210.0, 116.0, 28.0, 28.0]),
            ("tool-b2", [1250.0, 116.0, 28.0, 28.0]),
        ] {
            base.named.insert(name.into(), NamedWidget { region: "right_panel".into(), rect, count: 1 });
        }
        let mut cur = base.clone();
        for name in ["tool-a1", "tool-a2", "tool-b1", "tool-b2"] {
            let w = cur.named.get_mut(name).unwrap();
            w.rect[1] -= 4.0;
            w.rect[3] = 36.0;
        }
        (base, cur)
    }

    fn taller_toolbar_entry() -> AllowEntry {
        AllowEntry {
            case: "d-editor-default".into(),
            region: "right_panel".into(),
            h_base: Some(28.0),
            anchor_y: Some(AnchorY::Centre),
            max_dh: Some(8.0),
            spec: "F".into(),
            reason: "R5: toolbar controls 28->36 centred".into(),
            ..Default::default()
        }
    }

    /// G3 Order groups widgets into the BASELINE rows: taller controls whose own centres now fall within the
    /// larger row threshold must not merge two baseline rows and reorder their widgets.
    #[test]
    fn taller_controls_that_merge_rows_keep_g3_order() {
        let (base, cur) = taller_toolbar_rows();
        let allow = validate_allow_entries(vec![taller_toolbar_entry()]).expect("valid allow list");

        compare_with_stale_check(&base, &cur, &allow, &[]).expect("controls growing 28->36 under an allow entry keep their order and must pass G3");
    }

    /// A narrow control that grows left (anchored right) past the left edge of a wide row neighbour in another layer
    /// keeps its place by centre: the left-edge sort would flip the pair although nothing moved past anything.
    #[test]
    fn a_control_growing_left_under_an_overlapping_row_keeps_g3_order() {
        let mut base = sample_snapshot("d-editor-default");
        base.named.insert("menu-row".into(), NamedWidget { region: "left_panel".into(), rect: [222.0, 100.0, 200.0, 28.0], count: 1 });
        base.named.insert("menu-btn".into(), NamedWidget { region: "left_panel".into(), rect: [229.0, 102.0, 24.0, 24.0], count: 1 });
        let mut cur = base.clone();
        cur.named.get_mut("menu-btn").unwrap().rect = [221.0, 102.0, 32.0, 24.0];

        let allow = validate_allow_entries(vec![AllowEntry {
            case: "d-editor-default".into(),
            region: "left_panel".into(),
            name_prefix: Some("menu-btn".into()),
            anchor_x: Some(AnchorX::Right),
            max_dw: Some(8.0),
            spec: "E".into(),
            reason: "R5: menu button 24->32 anchored right".into(),
            ..Default::default()
        }])
        .expect("valid allow list");

        compare_with_stale_check(&base, &cur, &allow, &[]).expect("a button growing left under an overlapping row neighbour must pass G3");
    }

    /// The baseline-row rule still guards side-by-side controls: swapping two of them in a row that the current
    /// rects alone would merge with its neighbour fails G3 Order, and the error shows the baseline rows intact.
    #[test]
    fn swapping_two_side_by_side_buttons_in_a_merged_row_still_fails_g3() {
        let (base, mut cur) = taller_toolbar_rows();
        cur.named.get_mut("tool-a1").unwrap().rect[0] = 1250.0;
        cur.named.get_mut("tool-a2").unwrap().rect[0] = 1210.0;

        let swapped = |prefix: &str, dx: f32| AllowEntry { name_prefix: Some(prefix.into()), dx: Some(dx), ..taller_toolbar_entry() };
        let allow = validate_allow_entries(vec![taller_toolbar_entry(), swapped("tool-a1", 40.0), swapped("tool-a2", -40.0)]).expect("valid allow list");

        // The dx entries let G4/G5 accept both new positions, so only G3 Order can reject the swap.
        let err = compare(&base, &cur, &allow, &[]).expect_err("swapping two side-by-side buttons must fail G3 Order");
        assert!(err.contains("G3 row order mismatch"), "swap must be rejected by G3 Order, got: {err}");
        assert!(
            err.contains(r#"current=[["tool-a2", "tool-a1"], ["tool-b1", "tool-b2"]]"#),
            "current order must be read inside the two baseline rows, got: {err}"
        );
    }

    fn named_in(region: &str, rect: [f32; 4]) -> NamedWidget {
        NamedWidget { region: region.into(), rect, count: 1 }
    }

    /// Two widgets whose centres coincide must keep their order when one only jitters by half a point in y:
    /// equal centres break by name, never by the y of either snapshot.
    #[test]
    fn equal_centres_with_half_point_y_jitter_keep_g3_order() {
        let mut base = sample_snapshot("d-editor-default");
        base.named.insert("chip".into(), named_in("canvas", [100.0, 100.0, 20.0, 20.0]));
        base.named.insert("badge".into(), named_in("canvas", [105.0, 100.25, 10.0, 20.0]));
        let mut cur = base.clone();
        cur.named.get_mut("badge").unwrap().rect[1] = 99.75;

        compare(&base, &cur, &AllowList::default(), &[]).expect("a half-point y jitter between equal centres must pass G3");
    }

    /// A control that grows evenly on both sides keeps its centre; against a neighbour with the same centre the order
    /// must not flip because its left edge moved past the neighbour's.
    #[test]
    fn centred_growth_with_an_equal_centre_keeps_g3_order() {
        let mut base = sample_snapshot("d-editor-default");
        base.named.insert("row".into(), named_in("canvas", [100.0, 100.0, 20.0, 20.0]));
        base.named.insert("btn".into(), named_in("canvas", [105.0, 100.0, 10.0, 20.0]));
        let mut cur = base.clone();
        cur.named.get_mut("btn").unwrap().rect = [99.0, 100.0, 22.0, 20.0];

        let allow = validate_allow_entries(vec![AllowEntry {
            case: "d-editor-default".into(),
            region: "canvas".into(),
            name_prefix: Some("btn".into()),
            anchor_x: Some(AnchorX::Centre),
            max_dw: Some(12.0),
            spec: "D".into(),
            reason: "R5: button 10->22 centred".into(),
            ..Default::default()
        }])
        .expect("valid allow list");

        compare_with_stale_check(&base, &cur, &allow, &[]).expect("centred growth that keeps the centre must pass G3");
    }

    /// An allow entry may authorise a translation, but a widget that leaves its baseline row for another one,
    /// landing level with a widget above it in the same column, changes the vertical order and fails G3.
    #[test]
    fn a_control_moving_into_another_row_fails_g3() {
        let mut base = sample_snapshot("d-editor-default");
        base.named.insert("a1".into(), named_in("canvas", [1210.0, 100.0, 28.0, 28.0]));
        base.named.insert("a2".into(), named_in("canvas", [1250.0, 100.0, 28.0, 28.0]));
        base.named.insert("solo".into(), named_in("canvas", [1210.0, 200.0, 28.0, 28.0]));
        let mut cur = base.clone();
        cur.named.get_mut("solo").unwrap().rect = [1180.0, 100.0, 28.0, 28.0];

        let allow = validate_allow_entries(vec![AllowEntry {
            case: "d-editor-default".into(),
            region: "canvas".into(),
            name_prefix: Some("solo".into()),
            dx: Some(-30.0),
            dy: Some(-100.0),
            spec: "D".into(),
            reason: "R5: solo control moved up and left".into(),
            ..Default::default()
        }])
        .expect("valid allow list");

        let err = compare(&base, &cur, &allow, &[]).expect_err("a widget landing level with the row above it must fail G3");
        assert!(
            err.contains("G3 row order mismatch") && err.contains("'a1' and 'solo' changed vertical order"),
            "must cite the vertical order change, got: {err}"
        );
    }

    /// One region can hold two columns (Home has a sidebar and a content area): sidebar rows that grow may pass a
    /// content row in y, because the vertical order is only kept between widgets whose x-extents overlap.
    #[test]
    fn sidebar_rows_growing_past_content_rows_keep_g3_order() {
        let mut base = sample_snapshot("d-editor-default");
        for i in 0..5 {
            base.named.insert(format!("side-{i}"), named_in("home", [0.0, 100.0 + 40.0 * i as f32, 200.0, 32.0]));
        }
        // Centre y 214 sits between side-2 (196) and side-3 (236) and is more than half a row from both.
        base.named.insert("content".into(), named_in("home", [300.0, 198.0, 200.0, 32.0]));
        let mut cur = base.clone();
        for i in 0..5 {
            let w = cur.named.get_mut(&format!("side-{i}")).unwrap();
            w.rect[1] += 8.0 * i as f32;
            w.rect[3] = 40.0;
        }

        let allow = validate_allow_entries(vec![AllowEntry {
            case: "d-editor-default".into(),
            region: "home".into(),
            name_prefix: Some("side-".into()),
            max_dh: Some(8.0),
            spec: "H".into(),
            reason: "R5: sidebar rows 32->40".into(),
            ..Default::default()
        }])
        .expect("valid allow list");

        compare_with_stale_check(&base, &cur, &allow, &[]).expect("sidebar rows passing a content row in y must pass G3");
    }

    /// Test 6: An allow entry that matched nothing in a run is stale and fails with its index.
    #[test]
    fn stale_allow_entry_fails() {
        let base = sample_snapshot("d-editor-default");
        let cur = base.clone();
        let allow = validate_allow_entries(vec![AllowEntry {
            case: "d-editor-default".into(),
            region: "right_panel".into(),
            name_prefix: Some("nonexistent-widget-".into()),
            max_dh: Some(8.0),
            spec: "F".into(),
            reason: "R5: unused rule".into(),
            ..Default::default()
        }])
        .expect("valid allow list");

        let err = compare_with_stale_check(&base, &cur, &allow, &[]).expect_err("unmatched allow entry must fail as stale");
        assert!(err.contains("0"), "stale allow error must report entry index 0, got: {err}");
    }

    /// Test 6b: `"*"` without `name_prefix` or `h_base` fails to parse.
    #[test]
    fn region_wide_wildcard_entry_is_rejected() {
        let bad_json = r#"[{"case":"*","region":"right_panel","max_dh":8,"spec":"F","reason":"R5: row 32"}]"#;
        let err = parse_allow_list(bad_json).expect_err("wildcard case '*' without name_prefix or h_base must be rejected");
        assert!(err.contains("*") || err.contains("wildcard") || err.contains("name_prefix"), "unexpected error: {err}");
    }

    /// Test 6c: Two equally specific (`h_base: 24`) entries on one region fail to parse; `name_prefix` beats `h_base`.
    #[test]
    fn equally_specific_entries_fail_to_parse() {
        let dup_json = r#"[
            {"case":"f-inspector","region":"right_panel","h_base":24,"max_dh":8,"spec":"F","reason":"R5: row 32"},
            {"case":"f-inspector","region":"right_panel","h_base":24,"max_dh":12,"spec":"F","reason":"R5: row 36"}
        ]"#;
        assert!(parse_allow_list(dup_json).is_err(), "two h_base=24 entries on the same case and region must fail to parse");

        let precedence_json = r#"[
            {"case":"f-inspector","region":"right_panel","h_base":24,"min_dh":8,"max_dh":8,"spec":"F","reason":"R5: row 32"},
            {"case":"f-inspector","region":"right_panel","name_prefix":"prop-row-0","min_dh":12,"max_dh":12,"spec":"F","reason":"R5: header row 36"}
        ]"#;
        let allow = parse_allow_list(precedence_json).expect("name_prefix alongside h_base must parse");

        let base = sample_snapshot("f-inspector");
        let mut cur = base.clone();
        // prop-row-0 matches both name_prefix ("prop-row-0", dh=+12) and h_base (24); name_prefix wins.
        cur.named.get_mut("prop-row-0").unwrap().rect = [1210.0, 60.0, 200.0, 36.0];
        cur.named.get_mut("prop-row-1").unwrap().rect = [1210.0, 102.0, 200.0, 32.0];
        cur.named.get_mut("prop-row-2").unwrap().rect = [1210.0, 140.0, 200.0, 32.0];

        compare_with_stale_check(&base, &cur, &allow, &[]).expect("name_prefix must beat h_base on prop-row-0");
    }

    /// Two legs may allow same-height widgets in one region when each entry names a disjoint band of baseline left edges.
    #[test]
    fn same_height_entries_in_disjoint_x_bands_each_cover_their_own_widgets() {
        let banded = r#"[
            {"case":"k","region":"overlay","h_base":24,"x_band":[0,300],"anchor_x":"right","min_dw":8,"max_dw":8,"min_dh":8,"max_dh":8,"anchor_y":"centre","spec":"E","reason":"R5: header buttons"},
            {"case":"k","region":"overlay","h_base":24,"x_band":[390,2000],"min_dh":8,"max_dh":8,"anchor_y":"centre","spec":"K","reason":"R5: value inputs"}
        ]"#;
        let allow = parse_allow_list(banded).expect("disjoint x bands on one height must parse");
        let left = best_matching_entry(&allow, "k", "overlay", None, [229.0, 105.0, 24.0, 24.0]).map(|(i, _)| i);
        let right = best_matching_entry(&allow, "k", "overlay", None, [477.0, 220.0, 96.0, 24.0]).map(|(i, _)| i);
        let outside = best_matching_entry(&allow, "k", "overlay", None, [340.0, 220.0, 24.0, 24.0]);
        assert_eq!((left, right), (Some(0), Some(1)), "each widget takes the entry whose band holds its left edge");
        assert!(outside.is_none(), "a widget between the bands takes neither entry");

        let overlapping = banded.replace("[390,2000]", "[250,2000]");
        assert!(parse_allow_list(&overlapping).is_err(), "overlapping bands on one height must fail to parse");
        let unbanded = banded.replace(r#""x_band":[390,2000],"#, "");
        assert!(parse_allow_list(&unbanded).is_err(), "an unbanded entry covers every x and still collides");
        let reversed = banded.replace("[0,300]", "[300,0]");
        assert!(parse_allow_list(&reversed).is_err(), "a band must run from low to high");
    }

    /// A banded widget takes no derived x shift from row-mates outside its band.
    #[test]
    fn a_banded_widget_ignores_width_changes_outside_its_band() {
        let allow = parse_allow_list(
            r#"[
            {"case":"k","region":"right_panel","h_base":24,"x_band":[0,1250],"anchor_x":"right","min_dw":8,"max_dw":8,"min_dh":8,"max_dh":8,"anchor_y":"centre","spec":"E","reason":"R5: buttons"},
            {"case":"k","region":"right_panel","h_base":24,"x_band":[1250.5,4000],"min_dh":8,"max_dh":8,"anchor_y":"centre","spec":"K","reason":"R5: inputs"}
        ]"#,
        )
        .expect("disjoint bands parse");
        let mut base = sample_snapshot("k");
        base.named.clear();
        base.named.insert("button".into(), NamedWidget { region: "right_panel".into(), rect: [1210.0, 60.0, 24.0, 24.0], count: 1 });
        base.named.insert("input".into(), NamedWidget { region: "right_panel".into(), rect: [1300.0, 62.0, 96.0, 24.0], count: 1 });
        let mut cur = base.clone();
        // The button grows 8 with its right edge held; the input grows 8 tall, centred, at its own left edge.
        cur.named.get_mut("button").unwrap().rect = [1202.0, 56.0, 32.0, 32.0];
        cur.named.get_mut("input").unwrap().rect = [1300.0, 58.0, 96.0, 32.0];
        compare_with_stale_check(&base, &cur, &allow, &[]).expect("the input's band holds no width change, so its left edge stays");
    }

    /// Test 7: Missing baseline file fails with the bless hint.
    #[test]
    fn missing_baseline_fails() {
        let snap = sample_snapshot("d-missing-case");
        let missing = std::env::temp_dir().join(format!("designos-missing-baseline-{}.layout.json", std::process::id()));
        let _ = std::fs::remove_file(&missing);

        let err = load_and_compare_baseline(&missing, &snap, &AllowList::default(), &[]).expect_err("missing baseline file must fail");
        assert!(err.contains("DESIGNOS_LAYOUT_BLESS=1"), "missing baseline error must contain the bless hint (DESIGNOS_LAYOUT_BLESS=1), got: {err}");
    }

    /// Test 8c: Each agent case, `d-window-chrome` and `k-libraries` is snapshotted with the host `PATH` empty
    /// and with fake `claude`/`codex` on it (both run under `ScopedCliDirs::enter(&stub_dir)`). The JSON is identical,
    /// the stub log records only the stub dir, and `app.libs.root` and every opened `Library` root lie under the case
    /// temp dir, never under `dirs::data_dir()`.
    #[test]
    fn agent_cases_do_not_depend_on_installed_clis() {
        let stub_dir = crate::cli_resolve::stub_dir().to_path_buf();
        let canon_stub_dir = std::fs::canonicalize(&stub_dir).unwrap_or_else(|_| stub_dir.clone());
        let data_dir = {
            let prev = std::env::var_os("DESIGNOS_LIBRARY");
            std::env::remove_var("DESIGNOS_LIBRARY");
            let default_lib_root = designos_library::Library::default_root();
            if let Some(v) = prev {
                std::env::set_var("DESIGNOS_LIBRARY", v);
            }
            default_lib_root.parent().and_then(|p| p.parent()).map(std::path::Path::to_path_buf).unwrap_or(default_lib_root)
        };

        let cases: &[(&str, Option<&str>, bool)] = &[
            ("d-window-chrome", None, false),
            ("k-libraries", Some("libraries"), true),
            ("i-agents", Some("agents"), true),
            ("i-agents-picker", Some("agents-picker"), true),
            ("i-agents-chat", Some("agents-chat"), true),
            ("i-agents-skills", Some("agents-skills"), true),
            ("i-agents-start-failure", Some("agents-start-failure"), true),
            ("i-agent-permissions", Some("agent-permissions"), true),
            ("i-agents-approval-command", Some("agents-approval-command"), true),
            ("i-agents-approval-queued", Some("agents-approval-queued"), true),
            ("i-agents-approval-open-tab", Some("agents-approval-open-tab"), true),
            ("i-terminal", Some("terminal"), true),
        ];

        if let Some(child_out) = std::env::var_os("DESIGNOS_8C_CHILD_OUT") {
            let mut results: BTreeMap<String, String> = BTreeMap::new();
            for &(id, panel, screenshot_chrome) in cases {
                crate::widgets::clear_pass_records();
                let _scoped = crate::cli_resolve::ScopedCliDirs::enter(&stub_dir);
                let ctx = Context::default();
                let opts = Options { panel: panel.map(String::from), agent_cli_dir: Some(stub_dir.clone()), ..Default::default() };
                let mut shell = Shell::build(&ctx, opts);
                if panel == Some("agents-chat") {
                    assert_eq!(
                        shell.app.terminal.active_composer().pending_send.as_deref(),
                        Some("Create a SaaS analytics card in Claude Code"),
                        "agents-chat must queue prompt right after Shell::build before any frame runs"
                    );
                    assert_eq!(
                        shell.app.terminal.active_composer().notice.as_deref(),
                        Some("Starting Claude Code\u{2026} your message will be sent"),
                        "agents-chat must set starting notice right after Shell::build before any frame runs"
                    );
                    assert!(
                        shell.app.terminal.active_tab().is_some_and(|t| t.last_usage_status.is_none()),
                        "agents-chat must not run poll_live_agent_bridge() sleep loop inside Shell::build"
                    );
                }
                shell.apply_launch_defaults(screenshot_chrome);
                ctx.all_styles_mut(|s| s.debug.show_interactive_widgets = true);

                let lib_root = shell.app.libs.root.as_ref().expect("Tier A seam must set app.libs.root on every case");
                assert!(
                    !lib_root.starts_with(&data_dir),
                    "case {id}: app.libs.root ({}) must not lie under dirs::data_dir() ({})",
                    lib_root.display(),
                    data_dir.display()
                );
                if let Some(lib) = &shell.app.lib {
                    assert!(
                        !lib.root().starts_with(&data_dir),
                        "case {id}: opened Library root ({}) must not lie under dirs::data_dir() ({})",
                        lib.root().display(),
                        data_dir.display()
                    );
                }
                assert_eq!(shell.app.terminal.path_override.as_deref(), Some(stub_dir.as_os_str()), "case {id}: terminal.path_override must equal stub_dir");
                assert!(shell.app.terminal.picker.settings_root_override.is_some(), "case {id}: picker.settings_root_override must be set");

                let raw = egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::Vec2::new(1440.0, 900.0))),
                    time: Some(0.05),
                    ..Default::default()
                };
                for i in 0..8 {
                    let mut r = raw.clone();
                    r.time = Some((i + 1) as f64 * 0.05);
                    let _ = ctx.run_ui(r, |ui| shell.draw(ui));
                }
                if id == "i-agents-start-failure" {
                    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
                    let mut exited = false;
                    while std::time::Instant::now() < deadline {
                        let done = shell.app.terminal.tabs.first_mut().is_none_or(|t| !t.is_alive() && t.reader_eof.load(std::sync::atomic::Ordering::Relaxed));
                        if done {
                            exited = true;
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    assert!(exited, "case {id}: stub child did not exit within 15s deadline");
                    if let Some(t) = shell.app.terminal.tabs.first_mut() {
                        t.spawned_at = std::time::Instant::now();
                    }
                    for i in 0..2 {
                        let mut r = raw.clone();
                        r.time = Some((9 + i) as f64 * 0.05);
                        let _ = ctx.run_ui(r, |ui| shell.draw(ui));
                    }
                    assert!(
                        shell.app.terminal.active_tab().is_some_and(|t| t.start_failure.is_some()),
                        "i-agents-start-failure must capture the diagnosed start_failure state"
                    );
                }
                let mut snap = capture(&ctx, &shell.app.ui.regions);
                snap.case = id.to_string();
                shell.app.terminal.shutdown();
                results.insert(id.to_string(), serde_json::to_string(&snap).expect("serialize snapshot"));
            }
            let encoded = serde_json::to_string(&results).expect("serialize 8c map");
            std::fs::write(child_out, encoded).expect("write DESIGNOS_8C_CHILD_OUT");
            return;
        }

        let tmp = std::env::temp_dir().join(format!("designos-8c-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let fake_cli_dir = tmp.join("fake-host-clis");
        std::fs::create_dir_all(&fake_cli_dir).expect("create fake_cli_dir");
        for bin in ["claude", "codex"] {
            let p = fake_cli_dir.join(bin);
            std::fs::write(
                &p,
                "#!/bin/sh\nif [ -n \"$DESIGNOS_STUB_LOG\" ]; then\n  printf 'FAKE_HOST %s %s\\n' \"$0\" \"$*\" >> \"$DESIGNOS_STUB_LOG\"\nfi\nexit 99\n",
            )
            .expect("write fake host cli");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).expect("chmod fake host cli");
            }
        }

        let out_empty = tmp.join("out-empty.json");
        let out_fake = tmp.join("out-fake.json");
        let log_empty = tmp.join("stub-empty.log");
        let log_fake = tmp.join("stub-fake.log");
        let exe = std::env::current_exe().expect("current_exe");

        let child_empty = std::process::Command::new(&exe)
            .args(["--exact", "layout_snapshot::tests::agent_cases_do_not_depend_on_installed_clis", "--nocapture"])
            .env("PATH", "")
            .env("DESIGNOS_STUB_LOG", &log_empty)
            .env("DESIGNOS_8C_CHILD_OUT", &out_empty)
            .spawn()
            .expect("spawn empty-PATH 8c child");

        let sep = if cfg!(windows) { ";" } else { ":" };
        let fake_path = format!("{}{sep}/usr/bin{sep}/bin", fake_cli_dir.display());
        let child_fake = std::process::Command::new(&exe)
            .args(["--exact", "layout_snapshot::tests::agent_cases_do_not_depend_on_installed_clis", "--nocapture"])
            .env("PATH", &fake_path)
            .env("DESIGNOS_STUB_LOG", &log_fake)
            .env("DESIGNOS_8C_CHILD_OUT", &out_fake)
            .spawn()
            .expect("spawn fake-PATH 8c child");

        let res_empty = child_empty.wait_with_output().expect("wait empty-PATH child");
        let res_fake = child_fake.wait_with_output().expect("wait fake-PATH child");
        assert!(res_empty.status.success(), "empty-PATH 8c child failed:\n{}", String::from_utf8_lossy(&res_empty.stdout));
        assert!(res_fake.status.success(), "fake-PATH 8c child failed:\n{}", String::from_utf8_lossy(&res_fake.stdout));

        let map_empty: BTreeMap<String, String> = serde_json::from_str(&std::fs::read_to_string(&out_empty).expect("read out_empty")).expect("parse out_empty");
        let map_fake: BTreeMap<String, String> = serde_json::from_str(&std::fs::read_to_string(&out_fake).expect("read out_fake")).expect("parse out_fake");
        for &(id, _, _) in cases {
            assert_eq!(map_empty.get(id), map_fake.get(id), "case {id}: snapshot JSON under empty PATH must equal snapshot JSON under fake-CLI PATH");
        }

        for (label, log_path) in [("empty PATH", &log_empty), ("fake-CLI PATH", &log_fake)] {
            let log_body = std::fs::read_to_string(log_path).unwrap_or_default();
            let lines: Vec<&str> = log_body.lines().filter(|l| !l.trim().is_empty()).collect();
            assert!(!lines.is_empty(), "stub log for {label} must not be empty");
            for line in lines {
                let invoked = line.split_whitespace().next().unwrap_or("");
                let invoked_path = Path::new(invoked);
                let canon_parent = invoked_path.parent().and_then(|p| std::fs::canonicalize(p).ok());
                assert_eq!(
                    canon_parent.as_deref(),
                    Some(canon_stub_dir.as_path()),
                    "stub log for {label} recorded invocation outside stub_dir ({}): {line}",
                    canon_stub_dir.display()
                );
            }
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Test 8d: `h-home` with `fixed_now = T` and with `fixed_now = T + 60 s` on a seed with fixed `modified`:
    /// the geometry is equal and the label text is the expected relative string; no wall-time wait.
    #[test]
    fn home_is_deterministic_with_fixed_clock() {
        let stub_dir = crate::cli_resolve::stub_dir().to_path_buf();
        let lib_dir = std::env::temp_dir().join(format!("designos-8d-home-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&lib_dir);
        let t0: u64 = 1_700_000_000_000;
        seed_library_at(&lib_dir, t0).expect("seed_library_at");

        let run_home = |now_ms: u64| -> (LayoutSnapshot, Vec<String>) {
            crate::widgets::clear_pass_records();
            let ctx = Context::default();
            let opts =
                Options { home: true, library: Some(lib_dir.clone()), agent_cli_dir: Some(stub_dir.clone()), fixed_now: Some(now_ms), ..Default::default() };
            let mut shell = Shell::build(&ctx, opts);
            shell.apply_launch_defaults(true);
            ctx.all_styles_mut(|s| s.debug.show_interactive_widgets = true);

            let raw =
                egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::Vec2::new(1440.0, 900.0))), time: Some(0.05), ..Default::default() };
            let mut shapes = Vec::new();
            for i in 0..8 {
                let mut r = raw.clone();
                r.time = Some((i + 1) as f64 * 0.05);
                let out = ctx.run_ui(r, |ui| shell.draw(ui));
                shapes = out.shapes;
            }
            let mut snap = capture(&ctx, &shell.app.ui.regions);
            snap.case = "h-home".into();
            let (texts, _) = crate::lint::scan_shapes(&shapes);
            let strings = texts.into_iter().map(|t| t.text).collect();
            shell.app.terminal.shutdown();
            (snap, strings)
        };

        let (snap_t0, texts_t0) = run_home(t0);
        let (snap_t60, texts_t60) = run_home(t0 + 60_000);
        let _ = std::fs::remove_dir_all(&lib_dir);

        assert_eq!(snap_t0, snap_t60, "h-home geometry must be identical at T and T + 60s");
        assert!(texts_t0.iter().any(|t| t.contains("just now")), "at fixed_now = T, Home card label must show 'just now', got: {texts_t0:?}");
        assert!(texts_t60.iter().any(|t| t.contains("1 minute ago")), "at fixed_now = T + 60s, Home card label must show '1 minute ago', got: {texts_t60:?}");
    }

    /// Test 8: `layout_json_requires_screenshot` — `cli::parse` error without `--screenshot`.
    #[test]
    fn layout_json_requires_screenshot() {
        let err =
            crate::cli::parse(["--layout-json", "out/snap.json"].into_iter().map(String::from)).expect_err("--layout-json without --screenshot must fail");
        assert_eq!(err, "--layout-json needs --screenshot");

        let ok = crate::cli::parse(["--screenshot", "out/snap.png", "--layout-json", "out/snap.json"].into_iter().map(String::from))
            .expect("--layout-json with --screenshot must succeed");
        assert_eq!(ok.layout_json, Some(std::path::PathBuf::from("out/snap.json")));
        assert!(crate::cli::USAGE.contains("[--layout-json OUT.json]"), "USAGE must include [--layout-json OUT.json]");
    }

    /// Test 8b: `layout_json_matches_headless_snapshot` — a `Sim` with `apply_launch_defaults(true)` and a `shot`;
    /// a synthetic `Event::Screenshot` (1x1) injected after frame 8; the written JSON equals `capture` in `regions` and `named`.
    #[test]
    fn layout_json_matches_headless_snapshot() {
        let stub_dir = crate::cli_resolve::stub_dir().to_path_buf();
        let tmp = std::env::temp_dir().join(format!("designos-8b-layout-json-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("create temp dir");
        let png_path = tmp.join("d-editor-default.png");
        let json_path = tmp.join("d-editor-default.layout.json");

        let starter_path = tmp.join("starter.designos");
        std::fs::write(&starter_path, include_bytes!("../assets/starter/designos-studio-starter.designos")).expect("write starter copy");

        crate::widgets::clear_pass_records();
        let ctx = Context::default();
        let opts = Options {
            file: Some(starter_path),
            select: Some("id:1".into()),
            rulers: true,
            no_toast: true,
            size: Some((1440, 900)),
            scale: Some(1.0),
            screenshot: Some(png_path.clone()),
            layout_json: Some(json_path.clone()),
            agent_cli_dir: Some(stub_dir),
            ..Default::default()
        };
        let mut shell = Shell::build(&ctx, opts);
        shell.apply_launch_defaults(true);
        ctx.all_styles_mut(|s| s.debug.show_interactive_widgets = true);

        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::Vec2::new(1440.0, 900.0))), time: Some(0.05), ..Default::default() };
        for i in 0..8 {
            let mut r = raw.clone();
            r.time = Some((i + 1) as f64 * 0.05);
            let _ = ctx.run_ui(r, |ui| shell.draw(ui));
        }
        let headless = capture(&ctx, &shell.app.ui.regions);

        let mut shot_raw = raw.clone();
        shot_raw.time = Some(9.0 * 0.05);
        shot_raw.events.push(egui::Event::Screenshot {
            viewport_id: egui::ViewportId::ROOT,
            user_data: egui::UserData::default(),
            image: std::sync::Arc::new(egui::ColorImage::filled([1, 1], egui::Color32::BLACK)),
        });
        let _ = ctx.run_ui(shot_raw, |ui| shell.draw(ui));
        shell.app.terminal.shutdown();

        let written_raw = std::fs::read_to_string(&json_path).expect("--layout-json file must be written when Event::Screenshot arrives");
        let written: LayoutSnapshot = serde_json::from_str(&written_raw).expect("parse written LayoutSnapshot JSON");
        let _ = std::fs::remove_dir_all(&tmp);

        assert_eq!(written.regions, headless.regions, "--layout-json regions must equal headless capture");
        assert_eq!(written.named, headless.named, "--layout-json named widgets must equal headless capture");
        assert!(!written.named.is_empty(), "--layout-json named map must not be empty");
    }

    /// Round 4 Minor 6: `scroll_detection_uses_widget_type_scrollbar_and_maybe_in_release` —
    /// When widget info is enabled (`show_interactive_widgets = true`), scroll detection uses
    /// `WidgetType::ScrollBar` (so a 16x24 `Sense::drag()` splitter is NOT misclassified as a
    /// scrollbar, and an actual `ScrollArea` scrollbar emits `axis: "y"`, `maybe: false` and is
    /// excluded from `unnamed`). When widget info is absent (`show_interactive_widgets = false`,
    /// matching the release `--layout-json` path), the fallback sets `axis: "y"`, `maybe: true`.
    #[test]
    fn scroll_detection_uses_widget_type_scrollbar_and_maybe_in_release() {
        crate::widgets::clear_pass_records();
        let ctx = Context::default();
        ctx.all_styles_mut(|s| s.debug.show_interactive_widgets = true);
        let regions = ShellRegions {
            left_panel: Some(Rect::from_min_size(Pos2::new(49.0, 40.0), egui::Vec2::new(241.0, 860.0))),
            right_panel: Some(Rect::from_min_size(Pos2::new(1199.0, 40.0), egui::Vec2::new(241.0, 860.0))),
            ..Default::default()
        };

        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::Vec2::new(1440.0, 900.0))), ..Default::default() };
        for _ in 0..2 {
            let _ = ctx.run_ui(raw.clone(), |ui| {
                // Non-scrollbar 16x24 drag handle in left_panel: must stay in unnamed, not scroll.
                let drag_rect = Rect::from_min_size(Pos2::new(60.0, 100.0), egui::Vec2::new(16.0, 24.0));
                let _ = ui.interact(drag_rect, egui::Id::new("splitter_drag_handle"), egui::Sense::drag());

                // Overflowing vertical ScrollArea in right_panel: emits WidgetType::ScrollBar.
                let right_rect = regions.right_panel.unwrap();
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(right_rect));
                egui::ScrollArea::vertical().id_salt("r4_minor6_scroll").max_height(100.0).show(&mut child, |ui| {
                    ui.set_min_size(egui::Vec2::new(200.0, 400.0));
                });
            });
        }
        let snap_info = capture(&ctx, &regions);
        assert_eq!(
            snap_info.scroll,
            vec![ScrollGeom { region: "right_panel".into(), axis: "y".into(), maybe: false }],
            "with widget_info enabled, only the overflowing ScrollArea in right_panel should emit ScrollGeom(y, maybe=false); got {:?}",
            snap_info.scroll
        );
        assert!(
            snap_info.unnamed.iter().any(|u| u.region == "left_panel" && u.rect == [60.0, 100.0, 16.0, 24.0]),
            "non-scrollbar 16x24 drag handle in left_panel must remain in unnamed"
        );
        assert!(
            !snap_info.unnamed.iter().any(|u| u.region == "right_panel" && u.sense.iter().any(|s| s == "click")),
            "scrollbar widget itself must be excluded from unnamed; got {:?}",
            snap_info.unnamed
        );

        // Now simulate the release --layout-json path where widget info is absent, including the same 16x24 splitter.
        let ctx_no_info = Context::default();
        ctx_no_info.all_styles_mut(|s| s.debug.show_interactive_widgets = false);
        for _ in 0..2 {
            let _ = ctx_no_info.run_ui(raw.clone(), |ui| {
                let drag_rect = Rect::from_min_size(Pos2::new(60.0, 100.0), egui::Vec2::new(16.0, 24.0));
                let _ = ui.interact(drag_rect, egui::Id::new("splitter_drag_handle"), egui::Sense::drag());

                let right_rect = regions.right_panel.unwrap();
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(right_rect));
                egui::ScrollArea::vertical().id_salt("r4_minor6_scroll_release").max_height(100.0).show(&mut child, |ui| {
                    ui.set_min_size(egui::Vec2::new(200.0, 400.0));
                });
            });
        }
        let snap_release = capture(&ctx_no_info, &regions);
        assert_eq!(
            snap_release.scroll,
            vec![ScrollGeom { region: "right_panel".into(), axis: "y".into(), maybe: true }],
            "when widget info is absent, fallback scroll detection must emit only right_panel axis='y', maybe=true; got {:?}",
            snap_release.scroll
        );
        assert_eq!(snap_release.unnamed, snap_info.unnamed, "both gate and release paths must classify the 16x24 splitter in left_panel as unnamed");
    }

    /// Round 4 Minor 7: `g1_allows_covered_region_y_and_h_while_keeping_x_and_w_exact` —
    /// G1 keeps `x` and `w` exact (even when an allow entry covers the region), while allowing
    /// `y` and `h` changes covered by the allow entry's bounds.
    #[test]
    fn g1_allows_covered_region_y_and_h_while_keeping_x_and_w_exact() {
        let base = sample_snapshot("c-title-bar");
        let mut cur = base.clone();
        // tab_bar grows h by +4 (40 -> 44), shifting left_panel y by +4 (40 -> 44) and shrinking its h by -4 (860 -> 856).
        cur.regions.insert("tab_bar".into(), Some([0.0, 0.0, 1440.0, 44.0]));
        cur.regions.insert("left_panel".into(), Some([49.0, 44.0, 241.0, 856.0]));

        let allow = validate_allow_entries(vec![
            AllowEntry {
                case: "c-title-bar".into(),
                region: "tab_bar".into(),
                min_dh: Some(4.0),
                max_dh: Some(4.0),
                spec: "D".into(),
                reason: "R5: tab bar height 40->44".into(),
                ..Default::default()
            },
            AllowEntry {
                case: "c-title-bar".into(),
                region: "left_panel".into(),
                dy: Some(4.0),
                min_dh: Some(-4.0),
                max_dh: Some(-4.0),
                spec: "D".into(),
                reason: "R5: left panel shifts down +4 and shrinks -4 under 44pt tab bar".into(),
                ..Default::default()
            },
        ])
        .expect("valid allow list");

        // With covering allow entries, y and h changes on tab_bar and left_panel pass G1 (and are not stale).
        compare_with_stale_check(&base, &cur, &allow, &[]).expect("covered region y/h changes must pass G1");

        // Even with max_dw in an allow entry, G1 must reject a region w change (x and w are always 0.0pt exact).
        let mut cur_w_drift = base.clone();
        cur_w_drift.regions.insert("left_panel".into(), Some([49.0, 44.0, 249.0, 856.0]));
        let allow_with_dw = validate_allow_entries(vec![AllowEntry {
            case: "c-title-bar".into(),
            region: "left_panel".into(),
            dy: Some(4.0),
            min_dh: Some(-4.0),
            max_dh: Some(-4.0),
            max_dw: Some(8.0),
            spec: "D".into(),
            reason: "R5: attempt to allow region width change".into(),
            ..Default::default()
        }])
        .expect("valid allow list");
        let err = compare(&base, &cur_w_drift, &allow_with_dw, &[]).expect_err("G1 must reject region width change even when allow entry has max_dw");
        assert!(err.contains("left_panel.w"), "G1 error must name left_panel.w, got: {err}");
    }

    /// Round 4 Minor 9: `open_failed_start_fixture_does_not_block_waiting_for_exit` —
    /// `open_failed_start_fixture` spawns the tab and returns immediately without a 5 s sleep loop;
    /// the gate's 2 s `is_alive` poll is the only wait.
    #[test]
    #[cfg(unix)]
    fn open_failed_start_fixture_does_not_block_waiting_for_exit() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = std::env::temp_dir().join(format!("designos-r4-minor9-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("create temp stub dir");
        let agy_stub = tmp.join("agy");
        std::fs::write(&agy_stub, "#!/bin/sh\nexec /bin/cat\n").expect("write blocking agy stub");
        std::fs::set_permissions(&agy_stub, std::fs::Permissions::from_mode(0o755)).expect("chmod +x agy stub");

        let _scoped = crate::cli_resolve::ScopedCliDirs::enter(&tmp);
        let mut terminal = crate::terminal::TerminalState::default();
        terminal.path_override = Some(tmp.clone().into_os_string());
        terminal.deferred_bridge_actions.push(crate::terminal::TerminalAction::LiveSyncError("sentinel".into()));
        terminal.open_failed_start_fixture(None);
        let bridge_polled = terminal.deferred_bridge_actions.is_empty();
        let unpolled_tab = terminal.active_tab().is_some_and(|t| t.start_failure.is_none() && t.last_usage_status.is_none());
        terminal.shutdown();
        let _ = std::fs::remove_dir_all(&tmp);

        assert!(
            !bridge_polled && unpolled_tab,
            "open_failed_start_fixture must return immediately after spawning without running poll_live_agent_bridge: bridge_polled={bridge_polled}, unpolled_tab={unpolled_tab}"
        );
    }

    /// Round 4 Nit 10: `negative_dw_or_dh_requires_r5_in_reason` —
    /// Per §5 Decision 9, a negative `min_dw` / `max_dw` / `min_dh` / `max_dh` must cite `R5` in `reason`.
    #[test]
    fn negative_dw_or_dh_requires_r5_in_reason() {
        let missing_r5_dh = validate_allow_entries(vec![AllowEntry {
            case: "f-inspector".into(),
            region: "right_panel".into(),
            name_prefix: Some("prop-row-0".into()),
            min_dh: Some(-4.0),
            spec: "F".into(),
            reason: "tighten inspector row".into(),
            ..Default::default()
        }])
        .expect_err("negative min_dh without R5 in reason must fail validate_allow_entries");
        assert!(missing_r5_dh.contains("R5"), "error must mention R5, got: {missing_r5_dh}");

        let missing_r5_dw = validate_allow_entries(vec![AllowEntry {
            case: "f-inspector".into(),
            region: "right_panel".into(),
            name_prefix: Some("prop-row-0".into()),
            min_dw: Some(-4.0),
            spec: "F".into(),
            reason: "tighten inspector width".into(),
            ..Default::default()
        }])
        .expect_err("negative min_dw without R5 in reason must fail validate_allow_entries");
        assert!(missing_r5_dw.contains("R5"), "error must mention R5, got: {missing_r5_dw}");

        validate_allow_entries(vec![AllowEntry {
            case: "f-inspector".into(),
            region: "right_panel".into(),
            name_prefix: Some("prop-row-0".into()),
            min_dh: Some(-4.0),
            spec: "F".into(),
            reason: "R5: intentional tightening keeping >= 24x24pt hit target".into(),
            ..Default::default()
        }])
        .expect("negative min_dh citing R5 in reason must pass validate_allow_entries");

        let r50_not_r5 = validate_allow_entries(vec![AllowEntry {
            case: "f-inspector".into(),
            region: "right_panel".into(),
            name_prefix: Some("prop-row-0".into()),
            min_dh: Some(-4.0),
            spec: "F".into(),
            reason: "R50: tighten inspector row".into(),
            ..Default::default()
        }])
        .expect_err("R50 must not satisfy the R5 token requirement in reason");
        assert!(r50_not_r5.contains("R5"), "error must mention R5, got: {r50_not_r5}");
    }

    /// Round 4 Nit 12: `g3_checks_row_name_sequence_with_underscore_for_unnamed` —
    /// Per §3 rule 8 G3 Order, widgets are clustered into baseline rows (centres within `max(4, 0.5*min(h1,h2))`)
    /// and sorted by centre x; the name sequence per row (with `_` for unnamed widgets) must be equal.
    #[test]
    fn g3_checks_row_name_sequence_with_underscore_for_unnamed() {
        let mut base = sample_snapshot("f-inspector");
        base.named.retain(|k, _| !k.starts_with("prop-row-"));
        base.named.insert("btn-align-left".into(), NamedWidget { region: "right_panel".into(), rect: [1210.0, 60.0, 24.0, 24.0], count: 1 });
        base.unnamed = vec![UnnamedWidget { region: "right_panel".into(), rect: [1242.0, 60.0, 24.0, 24.0], sense: vec!["click".into()] }];

        let mut cur = base.clone();
        // Swap x positions on the same row: ["btn-align-left", "_"] -> ["_", "btn-align-left"].
        cur.named.get_mut("btn-align-left").unwrap().rect[0] = 1242.0;
        cur.unnamed[0].rect[0] = 1210.0;

        let allow = validate_allow_entries(vec![
            AllowEntry {
                case: "f-inspector".into(),
                region: "right_panel".into(),
                name_prefix: Some("btn-align-left".into()),
                dx: Some(32.0),
                spec: "F".into(),
                reason: "R5: shift named button right".into(),
                ..Default::default()
            },
            AllowEntry {
                case: "f-inspector".into(),
                region: "right_panel".into(),
                h_base: Some(24.0),
                dx: Some(-32.0),
                spec: "F".into(),
                reason: "R5: shift unnamed button left".into(),
                ..Default::default()
            },
        ])
        .expect("valid allow list");

        let err = compare(&base, &cur, &allow, &[]).expect_err("swapping named and unnamed widget order on a row must fail G3 Order");
        assert!(err.contains("G3") && err.contains("row"), "error must report G3 row order mismatch, got: {err}");
    }
}
