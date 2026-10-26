//! Layout-unchanged gate (`#1126`, Spec L): deterministic geometry snapshot capture and G1–G5 comparer.

use crate::app::ShellRegions;
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

pub fn capture(_ctx: &Context, _regions: &ShellRegions) -> LayoutSnapshot {
    LayoutSnapshot {
        schema: SNAPSHOT_SCHEMA,
        case: String::new(),
        size: [0, 0],
        scale: 1.0,
        regions: BTreeMap::new(),
        overlays: Vec::new(),
        named: BTreeMap::new(),
        unnamed: Vec::new(),
        scroll: Vec::new(),
    }
}

pub fn parse_allow_list(_json: &str) -> Result<AllowList, String> {
    Ok(AllowList { entries: Vec::new() })
}

pub fn validate_allow_entries(_entries: Vec<AllowEntry>) -> Result<AllowList, String> {
    Ok(AllowList { entries: Vec::new() })
}

pub fn compare(_base: &LayoutSnapshot, _cur: &LayoutSnapshot, _allow: &AllowList, _virtualized_regions: &[String]) -> Result<CompareReport, String> {
    Ok(CompareReport::default())
}

pub fn check_stale_allow_entries(_allow: &AllowList, _matched_indices: &BTreeSet<usize>, _scope_cases: Option<&BTreeSet<String>>) -> Result<(), String> {
    Ok(())
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
    _baseline_path: &Path,
    _cur: &LayoutSnapshot,
    _allow: &AllowList,
    _virtualized_regions: &[String],
) -> Result<CompareReport, String> {
    Ok(CompareReport::default())
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
}
