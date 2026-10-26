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
    let widget_info_enabled = cfg!(debug_assertions) && global_style.debug.show_interactive_widgets;
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
                let is_drag_only = !rect_entry.sense.contains(egui::Sense::CLICK);
                let narrow_axis = if w <= 16.0 && h >= 24.0 && (is_drag_only || (w - bar_width).abs() <= 0.5) {
                    Some("y")
                } else if h <= 16.0 && w >= 24.0 && (is_drag_only || (h - bar_width).abs() <= 0.5) {
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
        if has_negative_size_delta && !e.reason.contains("R5") {
            return Err(format!("allow.json[{idx}]: negative dw/dh requires R5 in reason, got {:?}", e.reason));
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
                    pa.starts_with(pb) || pb.starts_with(pa)
                }
                1 => {
                    let ha = a.h_base.unwrap_or(0.0);
                    let hb = b.h_base.unwrap_or(0.0);
                    (ha - hb).abs() <= 0.5
                }
                _ => true,
            };
            if same_target {
                return Err(format!("allow.json[{i}] and allow.json[{j}]: equally specific entries covering case='{}' region='{}'", b.case, b.region));
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
    true
}

fn best_matching_entry<'a>(allow: &'a AllowList, case: &str, region: &str, name: Option<&str>, base_rect: [f32; 4]) -> Option<(usize, &'a AllowEntry)> {
    allow
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| entry_matches_widget(e, case, region, name, base_rect))
        .max_by_key(|(_, e)| (entry_specificity(e), e.name_prefix.as_ref().map_or(0, |s| s.len()), usize::from(e.case != "*")))
}

fn widget_satisfies_allow_bounds(e: &AllowEntry, base: [f32; 4], cur: [f32; 4], gaps_above: usize) -> bool {
    let dw = cur[2] - base[2];
    let dh = cur[3] - base[3];

    let min_dw = e.min_dw.unwrap_or(0.0);
    let max_dw = e.max_dw.unwrap_or(0.0);
    if dw < min_dw - 0.5 || dw > max_dw + 0.5 {
        return false;
    }

    let min_dh = e.min_dh.unwrap_or(0.0);
    let max_dh = e.max_dh.unwrap_or(0.0);
    if dh < min_dh - 0.5 || dh > max_dh + 0.5 {
        return false;
    }

    let expected_x = match e.anchor_x.unwrap_or_default() {
        AnchorX::Left => base[0],
        AnchorX::Right => base[0] + base[2] - cur[2],
        AnchorX::Centre => base[0] + (base[2] - cur[2]) * 0.5,
    } + e.dx.unwrap_or(0.0);

    let expected_y = match e.anchor_y.unwrap_or_default() {
        AnchorY::Top => base[1],
        AnchorY::Bottom => base[1] + base[3] - cur[3],
        AnchorY::Centre => base[1] + (base[3] - cur[3]) * 0.5,
    } + e.dy.unwrap_or(0.0)
        + (gaps_above as f32) * e.gap_dy.unwrap_or(0.0);

    let x_tol = 0.5;
    let y_tol = 0.5;

    (cur[0] - expected_x).abs() <= x_tol && (cur[1] - expected_y).abs() <= y_tol
}

fn rects_differ_beyond_half_pt(a: [f32; 4], b: [f32; 4]) -> bool {
    !rects_match_within_half_pt(a, b)
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
            if (b[1] != c[1] || b[3] != c[3]) && matched.is_some_and(|(_, entry)| widget_satisfies_allow_bounds(entry, b, c, 0)) {
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
            let allowed = covering.is_some_and(|(_, e)| widget_satisfies_allow_bounds(e, b_ov.rect, c_ov.rect, 0));
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
    for (name, c_w) in &cur.named {
        if !base.named.contains_key(name) {
            report.new_names.push(format!("{case_id}:{name} (region={})", c_w.region));
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

    // Build per-region row bounding-box height deltas (named + unnamed widgets) and range-check each row's own `dh`.
    struct RowWidget<'a> {
        label: String,
        name: Option<&'a str>,
        b_rect: [f32; 4],
        c_rect: [f32; 4],
    }
    let mut row_deltas_by_region: BTreeMap<String, Vec<(f32, f32, f32)>> = BTreeMap::new();
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
        let mut row_deltas = Vec::with_capacity(rows.len());
        for (row_idx, mut row) in rows.into_iter().enumerate() {
            row.sort_by(|a, b| a.b_rect[0].total_cmp(&b.b_rect[0]).then_with(|| cmp_q4(&a.b_rect, &b.b_rect)).then_with(|| a.label.cmp(&b.label)));
            let b_min_x = row.iter().map(|w| w.b_rect[0]).fold(f32::INFINITY, f32::min);
            let b_max_x = row.iter().map(|w| w.b_rect[0] + w.b_rect[2]).fold(f32::NEG_INFINITY, f32::max);
            let b_min_y = row.iter().map(|w| w.b_rect[1]).fold(f32::INFINITY, f32::min);
            let b_max_y = row.iter().map(|w| w.b_rect[1] + w.b_rect[3]).fold(f32::NEG_INFINITY, f32::max);
            let c_min_y = row.iter().map(|w| w.c_rect[1]).fold(f32::INFINITY, f32::min);
            let c_max_y = row.iter().map(|w| w.c_rect[1] + w.c_rect[3]).fold(f32::NEG_INFINITY, f32::max);
            let b_w = (b_max_x - b_min_x).max(0.0);
            let b_h = (b_max_y - b_min_y).max(0.0);
            let c_h = (c_max_y - c_min_y).max(0.0);
            let b_cy = (b_min_y + b_max_y) * 0.5;
            let row_dh = c_h - b_h;

            let row_b_rect = [b_min_x, b_min_y, b_w, b_h];
            let row_covering = row
                .iter()
                .filter_map(|w| best_matching_entry(allow, case_id, &reg, w.name, w.b_rect))
                .chain(best_matching_entry(allow, case_id, &reg, None, row_b_rect))
                .max_by_key(|(_, e)| {
                    (
                        usize::from(e.min_dh.is_some() || e.max_dh.is_some()),
                        entry_specificity(e),
                        e.name_prefix.as_ref().map_or(0, |s| s.len()),
                        usize::from(e.case != "*"),
                    )
                });
            if let Some((idx, _)) = row_covering {
                report.matched_allow_indices.insert(idx);
            }
            if row_dh.abs() > 0.5 {
                let (min_dh, max_dh, has_covering) = match row_covering {
                    Some((_, e)) => (e.min_dh.unwrap_or(0.0), e.max_dh.unwrap_or(0.0), true),
                    None => (0.0, 0.0, false),
                };
                if !has_covering || row_dh < min_dh - 0.5 || row_dh > max_dh + 0.5 {
                    let row_names: Vec<&str> = row.iter().map(|w| w.label.as_str()).collect();
                    return Err(format!(
                        "G3/G4 row #{row_idx} [{}] dh={row_dh} out of allowed [{min_dh}, {max_dh}] in case {case_id} region '{reg}': baseline_h={b_h}, current_h={c_h}",
                        row_names.join(", ")
                    ));
                }
            }

            row_deltas.push((b_cy, b_h, row_dh));
        }
        row_deltas_by_region.insert(reg, row_deltas);
    }

    let cumulative_dh_and_gaps_above = |region: &str, b_rect: [f32; 4]| -> (f32, usize) {
        let b_cy = b_rect[1] + b_rect[3] * 0.5;
        let b_h = b_rect[3];
        row_deltas_by_region
            .get(region)
            .map(|rows| {
                let mut sum_dh = 0.0;
                let mut gaps = 0usize;
                for (r_cy, r_h, dh) in rows {
                    let thresh = (4.0_f32).max(0.5 * r_h.min(b_h));
                    if *r_cy + thresh < b_cy {
                        sum_dh += *dh;
                        gaps += 1;
                    }
                }
                (sum_dh, gaps)
            })
            .unwrap_or((0.0, 0))
    };

    for (name, b_w) in &base.named {
        let Some(c_w) = cur.named.get(name) else {
            continue;
        };
        let covering = best_matching_entry(allow, case_id, &b_w.region, Some(name.as_str()), b_w.rect);
        if rects_differ_beyond_half_pt(b_w.rect, c_w.rect) {
            let (sum_dh_above, gaps_above) = cumulative_dh_and_gaps_above(&b_w.region, b_w.rect);

            let mut allowed = false;
            if let Some((_, e)) = covering {
                let shift_y = if matches!(e.anchor_y.unwrap_or_default(), AnchorY::Top) { sum_dh_above } else { 0.0 };
                let shifted_base = [b_w.rect[0], b_w.rect[1] + shift_y, b_w.rect[2], b_w.rect[3]];
                if widget_satisfies_allow_bounds(e, shifted_base, c_w.rect, gaps_above) {
                    allowed = true;
                }
            }
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
                let (sum_dh_above, gaps_above) = cumulative_dh_and_gaps_above(&reg, b_u.rect);
                let mut allowed = false;
                if let Some((_, e)) = covering {
                    let shift_y = if matches!(e.anchor_y.unwrap_or_default(), AnchorY::Top) { sum_dh_above } else { 0.0 };
                    let shifted_base = [b_u.rect[0], b_u.rect[1] + shift_y, b_u.rect[2], b_u.rect[3]];
                    if widget_satisfies_allow_bounds(e, shifted_base, c_u.rect, gaps_above) {
                        allowed = true;
                    }
                }
                if !allowed {
                    return Err(format!("G4 unnamed widget #{i} rect drift in case {case_id} region '{reg}': baseline={:?}, current={:?}", b_u.rect, c_u.rect));
                }
            }
        }
    }

    // G3 Order — Per region, cluster widgets into rows (centres within max(4, 0.5*min(h1,h2))), sort by x,
    // and verify the name sequence per row (with "_" for unnamed widgets) is equal.
    let cluster_row_names = |mut items: Vec<([f32; 4], &str)>| -> Vec<Vec<String>> {
        items.sort_by(|(ra, na), (rb, nb)| {
            let cya = ra[1] + ra[3] * 0.5;
            let cyb = rb[1] + rb[3] * 0.5;
            cya.total_cmp(&cyb).then_with(|| cmp_q4(ra, rb)).then_with(|| na.cmp(nb))
        });
        let mut rows: Vec<Vec<([f32; 4], &str)>> = Vec::new();
        for (r, name) in items {
            let cy = r[1] + r[3] * 0.5;
            let h = r[3];
            let joins_last = rows.last().is_some_and(|row| {
                let first_r = row[0].0;
                let cy0 = first_r[1] + first_r[3] * 0.5;
                let h0 = first_r[3];
                (cy - cy0).abs() <= (4.0_f32).max(0.5 * h0.min(h))
            });
            if joins_last {
                rows.last_mut().unwrap().push((r, name));
            } else {
                rows.push(vec![(r, name)]);
            }
        }
        rows.into_iter()
            .map(|mut row| {
                row.sort_by(|(ra, na), (rb, nb)| ra[0].total_cmp(&rb[0]).then_with(|| cmp_q4(ra, rb)).then_with(|| na.cmp(nb)));
                row.into_iter().map(|(_, name)| name.to_string()).collect()
            })
            .collect()
    };

    let mut order_regions: BTreeSet<String> = base.named.values().map(|w| w.region.clone()).collect();
    order_regions.extend(base_unnamed_by_reg.keys().cloned());
    for reg in order_regions {
        let mut base_items: Vec<([f32; 4], &str)> = Vec::new();
        let mut cur_items: Vec<([f32; 4], &str)> = Vec::new();
        for (name, b_w) in &base.named {
            if b_w.region == reg {
                if let Some(c_w) = cur.named.get(name) {
                    base_items.push((b_w.rect, name.as_str()));
                    cur_items.push((c_w.rect, name.as_str()));
                }
            }
        }
        let b_u_list = base_unnamed_by_reg.get(&reg).map(Vec::as_slice).unwrap_or(&[]);
        let c_u_list = cur_unnamed_by_reg.get(&reg).map(Vec::as_slice).unwrap_or(&[]);
        let shared_len = b_u_list.len().min(c_u_list.len());
        for (b_u, c_u) in b_u_list[..shared_len].iter().zip(c_u_list[..shared_len].iter()) {
            base_items.push((b_u.rect, "_"));
            cur_items.push((c_u.rect, "_"));
        }
        let base_rows = cluster_row_names(base_items);
        let cur_rows = cluster_row_names(cur_items);
        if base_rows != cur_rows {
            return Err(format!("G3 row order mismatch in case {case_id} region '{reg}': baseline={base_rows:?}, current={cur_rows:?}"));
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
        if let Some(sp) = spec_filter {
            if e.spec != sp && (e.case == "*" || scope_cases.is_none_or(|s| !s.contains(&e.case))) {
                continue;
            }
        }
        if let Some(scope) = scope_cases {
            if e.case != "*" && !scope.contains(&e.case) {
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
        let mut cur_sense = cur_ok;
        cur_sense.unnamed[0].sense = vec!["drag".into()];
        let err_sense = compare(&base, &cur_sense, &allow, &["right_panel".into()])
            .expect_err("changing sense in the shared prefix under inventory_max_delta must fail G4");
        assert!(err_sense.contains("G4") && err_sense.contains("sense"), "expected G4 sense error, got: {err_sense}");
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
        check_stale_allow_entries(&allow_wild, &union_matched, Some(&scope_cases))
            .expect("wildcard entry matched in at least one case of the run must not be stale");

        // If no case in the run has any widget covered by the wildcard entry, the run-level stale check fails.
        let mut empty_matched = BTreeSet::new();
        let mut home_only_scope = BTreeSet::new();
        let r_home = compare(&base_home, &cur_home, &allow_wild, &[]).expect("home compare");
        empty_matched.extend(r_home.matched_allow_indices);
        home_only_scope.insert("h-home".into());
        let err = check_stale_allow_entries(&allow_wild, &empty_matched, Some(&home_only_scope))
            .expect_err("wildcard entry unused by every case in the run must fail as stale");
        assert!(err.contains("stale allow.json[0]"), "unexpected error: {err}");
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
        let default_lib_root = designos_library::Library::default_root();
        let data_dir = default_lib_root.parent().and_then(|p| p.parent()).map(std::path::Path::to_path_buf).unwrap_or_else(|| default_lib_root.clone());

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
                let started = std::time::Instant::now();
                let mut shell = Shell::build(&ctx, opts);
                let build_elapsed = started.elapsed();
                if panel == Some("agents-chat") {
                    assert!(
                        build_elapsed < std::time::Duration::from_millis(600),
                        "agents-chat must be state-driven without the 800ms sleep loop, took {build_elapsed:?}"
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
                    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
                    while std::time::Instant::now() < deadline {
                        let done = shell.app.terminal.tabs.first_mut().is_none_or(|t| !t.is_alive() && t.reader_eof.load(std::sync::atomic::Ordering::Relaxed));
                        if done {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(10));
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
        crate::showcase::seed_library_at(&lib_dir, t0).expect("seed_library_at");

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

        crate::widgets::clear_pass_records();
        let ctx = Context::default();
        let opts = Options {
            fixture: Some("showcase".into()),
            select: Some("none".into()),
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

        // Now simulate the release --layout-json path where widget info is absent.
        let ctx_no_info = Context::default();
        ctx_no_info.all_styles_mut(|s| s.debug.show_interactive_widgets = false);
        for _ in 0..2 {
            let _ = ctx_no_info.run_ui(raw.clone(), |ui| {
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
            "when widget info is absent, fallback scroll detection must emit axis='y', maybe=true; got {:?}",
            snap_release.scroll
        );
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
        let started = std::time::Instant::now();
        terminal.open_failed_start_fixture(None);
        let elapsed = started.elapsed();
        terminal.shutdown();
        let _ = std::fs::remove_dir_all(&tmp);

        assert!(elapsed < std::time::Duration::from_millis(500), "open_failed_start_fixture must not block in a 5s sleep loop; took {elapsed:?}");
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
    }

    /// Round 4 Nit 12: `g3_checks_row_name_sequence_with_underscore_for_unnamed` —
    /// Per §3 rule 8 G3 Order, widgets are clustered into rows (centres within `max(4, 0.5*min(h1,h2))`)
    /// and sorted by x; the name sequence per row (with `_` for unnamed widgets) must be equal.
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
