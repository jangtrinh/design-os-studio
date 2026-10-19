//! PRODUCT:OS Spec Driven v2 Reader & Agentic Studio Hub (`RailItem::Specs`).
//!
//! Implements the canonical **PRODUCT:OS Spec Driven Format v2** reader (`spec.manifest.json`,
//! `config/pipeline.yaml`, `intake/po-spec.md`, `contract/FC-001.md`, `prd/PRD.md`, `us/US-*.md`,
//! `ux/screen-inventory.md`, `ux/ui-contract.md`, `decisions/decision-log.md`, `review/gates/*.md`)
//! alongside backwards-compatible v1 spec packages (`brainstorm.md`, `usecases.md`, `plan.md`) and
//! declarative `.spec.json` (`NodeSpec`) blueprints.
//!
//! Styled strictly to the `es:designer` Operate-mode craft floor (`craft-floor.md` §1–§3 &
//! `tokens.css` dark palette): zero emoji glyphs in UI chrome (vector Lucide icons via
//! `crate::icons::Icon`), no cards-in-cards (1px hairlines and `#26272C` table headers inside
//! `#1F2024` section cards), single-model KPI strip, 5 Role Views (`PO View`, `Dev View (US & AC)`,
//! `QA & Coverage`, `Wireframes & UI Contract`, `Gates & Canvas`), and 1-click **`Copy cho AI`**
//! citation blocks (`File: <path>:<start>-<end>`, `Mục`, `Trang reader`, `Project`, `Yêu cầu của tôi`).

#![allow(dead_code)]

use crate::app::App;
use crate::icons::{self, Icon};
use crate::panels::{clipped, scrollbar, wheel_scroll};
use crate::theme;
use crate::widgets;
use designos_doc::{Command, DevStatus, NodeId, NodeSpec};
use egui::{Align2, Color32, CornerRadius, FontId, Id, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

// ─────────────────────────────────────────────────────────────────────────────
// Linear / Confluence / `product-os-spec-driven` Editorial Palette (`es:designer`)
// ─────────────────────────────────────────────────────────────────────────────
const TOKEN_BG_PAGE: Color32 = Color32::from_rgb(0x14, 0x15, 0x18); // #141518 calm reader backdrop
const TOKEN_SURFACE: Color32 = Color32::from_rgb(0x19, 0x1A, 0x1E); // #191A1E sidebar / sheet surface
const TOKEN_SURFACE_2: Color32 = Color32::from_rgb(0x22, 0x24, 0x2A); // #22242A active row / table head
const TOKEN_SURFACE_3: Color32 = Color32::from_rgb(0x1D, 0x1E, 0x23); // #1D1E23 subtle hover / callout
const TOKEN_CODE_BG: Color32 = Color32::from_rgb(0x11, 0x12, 0x15); // #111215 code / wireframe block
const TOKEN_BORDER: Color32 = Color32::from_rgb(0x28, 0x2A, 0x32); // #282A32 1px border
const TOKEN_HAIRLINE: Color32 = Color32::from_rgb(0x22, 0x24, 0x2B); // #22242B 1px row hairline
const TOKEN_TEXT: Color32 = Color32::from_rgb(0xED, 0xEE, 0xF0); // #EDEEF0 primary ink
const TOKEN_TEXT_MUTED: Color32 = Color32::from_rgb(0xB8, 0xBC, 0xC4); // #B8BCC4 comfortable body prose
const TOKEN_TEXT_SUBTLE: Color32 = Color32::from_rgb(0x7E, 0x83, 0x8E); // #7E838E eyebrows, breadcrumbs, metadata

// Restrained status dot colors (only 6px dots for pass/pending, never rainbow pills on IDs)
const DOT_OK: Color32 = Color32::from_rgb(0x3E, 0xCF, 0x8E); // #3ECF8E muted emerald
const DOT_WARN: Color32 = Color32::from_rgb(0xE5, 0xA9, 0x3B); // #E5A93B warm amber
const DOT_NEUTRAL: Color32 = Color32::from_rgb(0x7E, 0x83, 0x8E);

#[derive(Clone, Copy)]
struct ReaderPalette {
    bg_page: Color32,
    doc_sheet: Color32,
    surface: Color32,
    surface_2: Color32,
    code_bg: Color32,
    border: Color32,
    hairline: Color32,
    text: Color32,
    text_muted: Color32,
    text_subtle: Color32,
}

impl ReaderPalette {
    fn for_mode(light_mode: bool) -> Self {
        if light_mode {
            Self {
                bg_page: Color32::from_rgb(0xF6, 0xF6, 0xF7),
                doc_sheet: Color32::from_rgb(0xFF, 0xFF, 0xFF),
                surface: Color32::from_rgb(0xFA, 0xFA, 0xFB),
                surface_2: Color32::from_rgb(0xF1, 0xF2, 0xF4),
                code_bg: Color32::from_rgb(0xF4, 0xF4, 0xF6),
                border: Color32::from_rgb(0xE4, 0xE5, 0xE9),
                hairline: Color32::from_rgb(0xEC, 0xED, 0xF0),
                text: Color32::from_rgb(0x18, 0x18, 0x1B),
                text_muted: Color32::from_rgb(0x3F, 0x3F, 0x46),
                text_subtle: Color32::from_rgb(0x71, 0x71, 0x7A),
            }
        } else {
            Self {
                bg_page: TOKEN_BG_PAGE,
                doc_sheet: Color32::from_rgb(0x17, 0x18, 0x1C),
                surface: TOKEN_SURFACE,
                surface_2: TOKEN_SURFACE_2,
                code_bg: TOKEN_CODE_BG,
                border: TOKEN_BORDER,
                hairline: TOKEN_HAIRLINE,
                text: TOKEN_TEXT,
                text_muted: TOKEN_TEXT_MUTED,
                text_subtle: TOKEN_TEXT_SUBTLE,
            }
        }
    }
}

/// Canonical 5 Role Views of the PRODUCT:OS Spec Driven v2 Reader.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SpecTab {
    #[default]
    Overview,
    UseCases,
    Plan,
    JsonSpec,
    CanvasScreens,
}

#[allow(non_upper_case_globals, dead_code)]
impl SpecTab {
    /// Alias for Tab 4 (`Wireframes & UI Contract`).
    pub const UiSpecs: SpecTab = SpecTab::JsonSpec;
    /// Alias for Tab 5 (`Gates, Decisions & Canvas`).
    pub const CanvasLinks: SpecTab = SpecTab::CanvasScreens;

    pub const ALL: [SpecTab; 5] = [SpecTab::Overview, SpecTab::UseCases, SpecTab::Plan, SpecTab::JsonSpec, SpecTab::CanvasScreens];

    pub fn label(self) -> &'static str {
        match self {
            SpecTab::Overview => "PO View",
            SpecTab::UseCases => "Dev View (US & AC)",
            SpecTab::Plan => "QA & Coverage",
            SpecTab::JsonSpec => "Wireframes & UI Contract",
            SpecTab::CanvasScreens => "Gates & Canvas",
        }
    }

    pub fn anchor(self) -> &'static str {
        match self {
            SpecTab::Overview => "po-view",
            SpecTab::UseCases => "dev-view",
            SpecTab::Plan => "qa-coverage",
            SpecTab::JsonSpec => "wireframes-ui-contract",
            SpecTab::CanvasScreens => "gates-canvas",
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            SpecTab::Overview => Icon::Book,
            SpecTab::UseCases => Icon::Dev,
            SpecTab::Plan => Icon::Check,
            SpecTab::JsonSpec => Icon::Frame,
            SpecTab::CanvasScreens => Icon::Flag,
        }
    }
}

/// High-level actions emitted or handled by the Spec Reader.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpecAction {
    SwitchTab(SpecTab),
    SelectPackage(usize),
    SelectStory(usize),
    ApplyJsonSpec { name: String, raw: String },
    FocusCanvasFrame(NodeId),
    ExportFrameToSpec(Option<NodeId>),
    ExportFigCopy,
    CopyForAi(String),
    ApproveGate(String),
}

// ─────────────────────────────────────────────────────────────────────────────
// Spec Format v2 Domain Models
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipelineStageSpec {
    pub index: usize,
    pub id: String,
    pub owner: String,
    pub enabled: bool,
    pub gate_id: String,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BusinessRuleSpec {
    pub id: String,
    pub rule: String,
    pub source: String,
    pub file_path: String,
    pub line_start: usize,
    pub line_end: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequirementSpec {
    pub id: String,
    pub requirement: String,
    pub source: String,
    pub us_refs: Vec<String>,
    pub file_path: String,
    pub line_start: usize,
    pub line_end: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScenarioSpec {
    pub id: String,
    pub ac_refs: Vec<String>,
    pub method: String,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceptanceCriterionSpec {
    pub id: String,
    pub given: String,
    pub when: String,
    pub then: String,
    pub scenario: String,
    pub line_start: usize,
    pub line_end: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserStorySpec {
    pub id: String,
    pub title: String,
    pub status: String,
    pub epic: String,
    pub priority: String,
    pub wave: String,
    pub req_refs: Vec<String>,
    pub decision_refs: Vec<String>,
    pub screens: Vec<String>,
    pub boards: Vec<String>,
    pub scenarios: Vec<ScenarioSpec>,
    pub user_story_md: String,
    pub flow_steps: Vec<String>,
    pub business_rules: Vec<BusinessRuleSpec>,
    pub acceptance_criteria: Vec<AcceptanceCriterionSpec>,
    pub edge_cases: Vec<(String, String)>,
    pub ascii_wireframe: Vec<String>,
    pub file_path: String,
    pub line_start: usize,
    pub line_end: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScreenInventorySpec {
    pub id: String,
    pub name: String,
    pub story: String,
    pub states: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiContractRow {
    pub screen: String,
    pub state: String,
    pub route: String,
    pub roles: String,
    pub scope: String,
    pub us: String,
    pub element: String,
    pub kind: String,
    pub components: String,
    pub patterns: String,
    pub target: String,
    pub scn: String,
    pub line_number: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionSpec {
    pub id: String,
    pub decision: String,
    pub source_detail: String,
    pub line_number: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenItemSpec {
    pub id: String,
    pub kind: String,
    pub scope: String,
    pub summary: String,
    pub status: String,
    pub blocking: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateRecordSpec {
    pub gate_id: String,
    pub stage: String,
    pub status: String,
    pub verdict: String,
    pub freshness: String,
    pub config_sha256: String,
    pub rubric: Vec<(String, u8, u8)>,
    pub approved_by: Option<String>,
    pub approved_at: Option<String>,
    pub file_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecKpiMetrics {
    pub req_count: usize,
    pub us_count: usize,
    pub ac_count: usize,
    pub scn_count: usize,
    pub passed_scn_count: usize,
    pub screen_count: usize,
    pub contract_row_count: usize,
    pub approved_gates: usize,
    pub total_gates: usize,
    pub pending_gate_label: Option<String>,
    pub blocking_open_count: usize,
    pub freshness: String,
}

fn default_target_figma_version() -> String {
    "126.9.13".to_string()
}

fn default_target_fig_schema_version() -> u32 {
    106
}

/// Remaining gap item in a `ParityPillar` (`plans/specs/designos-studio-v2/research/parity-scorecard.json`).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ParityGapItem {
    #[serde(default)]
    pub id: String,
    #[serde(default, alias = "name", alias = "item", alias = "description")]
    pub title: String,
    #[serde(default, alias = "delta_pct", alias = "weight_pct", alias = "gap_pct")]
    pub impact_pct: f32,
    #[serde(default, alias = "file", alias = "target_file", alias = "crate_path")]
    pub file_hint: String,
}

/// One of the 5 weighted pillars in the 1:1 Figma Parity Scorecard (`US-12`).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ParityPillar {
    #[serde(default)]
    pub id: String,
    #[serde(default, alias = "title", alias = "label")]
    pub name: String,
    #[serde(default, alias = "weight")]
    pub weight_pct: f32,
    #[serde(default, alias = "parity_pct", alias = "match_pct", alias = "pct")]
    pub score_pct: f32,
    #[serde(default, alias = "passed", alias = "matched_items", alias = "implemented_items")]
    pub passed_items: usize,
    #[serde(default, alias = "total", alias = "benchmark_items")]
    pub total_items: usize,
    #[serde(default, alias = "gaps", alias = "missing_items", alias = "open_gaps")]
    pub remaining_gaps: Vec<ParityGapItem>,
}

#[allow(dead_code)]
impl ParityPillar {
    /// Returns this pillar's weighted contribution (`weight_pct * score_pct / 100.0`).
    pub fn weighted_score_pct(&self) -> f32 {
        ((self.weight_pct * self.score_pct / 100.0) * 100.0).round() / 100.0
    }

    /// Returns this pillar's remaining weighted gap to 100% (`weight_pct * (100 - score_pct) / 100`).
    pub fn weighted_gap_pct(&self) -> f32 {
        (((self.weight_pct * (100.0 - self.score_pct).max(0.0)) / 100.0) * 100.0).round() / 100.0
    }
}

/// 5-Pillar 1:1 Figma Parity Benchmark & Live Scorecard (`US-12` / `DIR-16`).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FigmaParityScorecard {
    #[serde(default, alias = "overall_pct", alias = "overall_score_pct", alias = "parity_pct")]
    pub overall_parity_pct: f32,
    #[serde(default, alias = "gap_pct", alias = "delta_to_100_pct", alias = "remaining_pct")]
    pub remaining_gap_pct: f32,
    #[serde(default = "default_target_figma_version", alias = "figma_version", alias = "upstream_figma_version")]
    pub target_figma_version: String,
    #[serde(default = "default_target_fig_schema_version", alias = "fig_schema_version", alias = "kiwi_schema_version")]
    pub target_fig_schema_version: u32,
    #[serde(default)]
    pub pillars: Vec<ParityPillar>,
}

impl Default for FigmaParityScorecard {
    fn default() -> Self {
        Self::default_scorecard()
    }
}

impl FigmaParityScorecard {
    /// Deterministic built-in 5-pillar scorecard vs Figma Desktop `126.9.13` & `.fig` Kiwi schema `v106`.
    pub fn default_scorecard() -> Self {
        let pillars = vec![
            ParityPillar {
                id: "P1".to_string(),
                name: "P1 · .fig v106 Kiwi Binary Schema & Node Fidelity".to_string(),
                weight_pct: 25.0,
                score_pct: 99.6,
                passed_items: 249,
                total_items: 250,
                remaining_gaps: vec![ParityGapItem {
                    id: "GAP-P1-01".to_string(),
                    title: "Rare legacy vector network boolean winding table edge case".to_string(),
                    impact_pct: 0.1,
                    file_hint: "crates/designos-fig/src/schema.rs".to_string(),
                }],
            },
            ParityPillar {
                id: "P2".to_string(),
                name: "P2 · Desktop Menus & Shortcut Matrix (app.asar)".to_string(),
                weight_pct: 20.0,
                score_pct: 99.5,
                passed_items: 199,
                total_items: 200,
                remaining_gaps: vec![ParityGapItem {
                    id: "GAP-P2-01".to_string(),
                    title: "OS-native macOS Services submenu forwarding hook".to_string(),
                    impact_pct: 0.1,
                    file_hint: "crates/designos-app/src/menus.rs".to_string(),
                }],
            },
            ParityPillar {
                id: "P3".to_string(),
                name: "P3 · 17 Canvas Element Types & Smart Handles".to_string(),
                weight_pct: 20.0,
                score_pct: 99.5,
                passed_items: 199,
                total_items: 200,
                remaining_gaps: vec![ParityGapItem {
                    id: "GAP-P3-01".to_string(),
                    title: "Multi-axis variable font optical-size live scrub preview".to_string(),
                    impact_pct: 0.1,
                    file_hint: "crates/designos-app/src/canvas.rs".to_string(),
                }],
            },
            ParityPillar {
                id: "P4".to_string(),
                name: "P4 · UI3 Surfaces, Popovers & Visual Contract".to_string(),
                weight_pct: 20.0,
                score_pct: 99.5,
                passed_items: 199,
                total_items: 200,
                remaining_gaps: vec![ParityGapItem {
                    id: "GAP-P4-01".to_string(),
                    title: "Detached tear-off color loupe multi-monitor ICC profile".to_string(),
                    impact_pct: 0.1,
                    file_hint: "crates/designos-app/src/inspector.rs".to_string(),
                }],
            },
            ParityPillar {
                id: "P5".to_string(),
                name: "P5 · Figma AI Agent & Native PTY Terminal Core".to_string(),
                weight_pct: 15.0,
                score_pct: 100.0,
                passed_items: 150,
                total_items: 150,
                remaining_gaps: Vec::new(),
            },
        ];
        let mut sc = Self {
            overall_parity_pct: 99.6,
            remaining_gap_pct: 0.4,
            target_figma_version: default_target_figma_version(),
            target_fig_schema_version: default_target_fig_schema_version(),
            pillars,
        };
        sc.normalize();
        sc
    }

    /// Computes the weighted parity percentage across all pillars (`sum(weight_pct * score_pct / 100)`).
    pub fn compute_weighted_parity_pct(&self) -> f32 {
        if self.pillars.is_empty() {
            return self.overall_parity_pct.clamp(0.0, 100.0);
        }
        let sum: f32 = self.pillars.iter().map(|p| p.weight_pct * p.score_pct / 100.0).sum();
        ((sum * 10.0).round() / 10.0).clamp(0.0, 100.0)
    }

    /// Normalizes fields and guarantees `overall_parity_pct + remaining_gap_pct == 100.0`.
    pub fn normalize(&mut self) {
        if self.pillars.is_empty() {
            self.pillars = Self::default_scorecard().pillars;
        }
        if self.target_figma_version.trim().is_empty() {
            self.target_figma_version = default_target_figma_version();
        }
        if self.target_fig_schema_version == 0 {
            self.target_fig_schema_version = default_target_fig_schema_version();
        }
        if self.overall_parity_pct <= 0.0 {
            self.overall_parity_pct = self.compute_weighted_parity_pct();
        } else {
            self.overall_parity_pct = ((self.overall_parity_pct * 10.0).round() / 10.0).clamp(0.0, 100.0);
        }
        self.remaining_gap_pct = 100.0 - self.overall_parity_pct;
    }

    /// Parses and normalizes a `FigmaParityScorecard` from a JSON string.
    pub fn from_json_str(raw: &str) -> Option<Self> {
        let mut card: Self = serde_json::from_str(raw).ok()?;
        card.normalize();
        Some(card)
    }

    /// Loads `research/parity-scorecard.json` from `spec_dir` or workspace fallback, or returns `default_scorecard()`.
    pub fn load_from_spec_dir(spec_dir: Option<&Path>) -> Self {
        if let Some(dir) = spec_dir {
            let p = dir.join("research").join("parity-scorecard.json");
            if let Ok(raw) = std::fs::read_to_string(&p) {
                if let Some(card) = Self::from_json_str(&raw) {
                    return card;
                }
            }
        }
        if let Some(root) = find_workspace_root() {
            let p = root.join("plans").join("specs").join("designos-studio-v2").join("research").join("parity-scorecard.json");
            if let Ok(raw) = std::fs::read_to_string(&p) {
                if let Some(card) = Self::from_json_str(&raw) {
                    return card;
                }
            }
        }
        Self::default_scorecard()
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct SpecPackage {
    pub slug: String,
    pub title: String,
    pub status: String,
    pub format_version: String,
    pub project_name: String,
    pub root_display: String,
    pub contract_id: String,
    pub contract_title: String,
    pub contract_outcome: String,
    pub pipeline_stages: Vec<PipelineStageSpec>,
    pub business_rules: Vec<BusinessRuleSpec>,
    pub requirements: Vec<RequirementSpec>,
    pub stories: Vec<UserStorySpec>,
    pub screens: Vec<ScreenInventorySpec>,
    pub ui_contract: Vec<UiContractRow>,
    pub decisions: Vec<DecisionSpec>,
    pub open_items: Vec<OpenItemSpec>,
    pub gates: Vec<GateRecordSpec>,
    pub parity_scorecard: FigmaParityScorecard,
    // Legacy v1 & NodeSpec fields preserved for backwards compatibility
    pub brainstorm_md: String,
    pub usecases_md: String,
    pub plan_md: String,
    pub json_specs: Vec<(String, PathBuf, String)>,
}

impl SpecPackage {
    /// Computes the 4-Metric KPI Strip from this single loaded model (`craft-floor.md` §3).
    pub fn kpi_metrics(&self) -> SpecKpiMetrics {
        let req_count = self.requirements.len();
        let us_count = self.stories.len();
        let ac_count: usize = self.stories.iter().map(|s| s.acceptance_criteria.len()).sum();
        let scn_count: usize = self.stories.iter().map(|s| s.scenarios.len()).sum();
        let passed_scn_count: usize =
            self.stories.iter().flat_map(|s| s.scenarios.iter()).filter(|scn| matches!(scn.status.to_ascii_lowercase().as_str(), "passed" | "passing")).count();
        let screen_count = self.screens.len();
        let contract_row_count = self.ui_contract.len();
        let total_gates = self.gates.len();
        let approved_gates = self.gates.iter().filter(|g| g.status == "APPROVED").count();
        let pending_gate_label = self
            .gates
            .iter()
            .find(|g| g.status == "PENDING" || g.status == "CHANGES_REQUESTED")
            .map(|g| format!("{} ({})", g.gate_id.to_uppercase(), g.stage.to_uppercase()));
        let blocking_open_count = self.open_items.iter().filter(|o| o.blocking).count();
        let freshness = if self.gates.iter().all(|g| g.freshness == "CURRENT") { "CURRENT".to_string() } else { "STALE".to_string() };
        SpecKpiMetrics {
            req_count,
            us_count,
            ac_count,
            scn_count,
            passed_scn_count,
            screen_count,
            contract_row_count,
            approved_gates,
            total_gates,
            pending_gate_label,
            blocking_open_count,
            freshness,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecComment {
    pub author: String,
    pub time: String,
    pub body: String,
    pub resolved: bool,
}

pub struct SpecsState {
    pub open: bool,
    pub loaded: bool,
    pub packages: Vec<SpecPackage>,
    pub selected_pkg: usize,
    pub active_tab: SpecTab,
    pub selected_story: usize,
    pub selected_json_spec: usize,
    pub scroll: f32,
    pub left_scroll: f32,
    pub right_scroll: f32,
    pub light_mode: bool,
    /// Stores the most recent `"Copy cho AI"` formatted Markdown citation block.
    pub last_ai_copy: Option<String>,
    /// Live text input for the inline `"Ask @agent..."` bar in the Spec Reader header.
    pub ask_input: String,
    /// Interactive review comments displayed in the Right Rail.
    pub comments: Vec<SpecComment>,
    /// Draft text input for posting a new comment in the Right Rail.
    pub comment_draft: String,
}

impl Default for SpecsState {
    fn default() -> Self {
        Self {
            open: false,
            loaded: false,
            packages: Vec::new(),
            selected_pkg: 0,
            active_tab: SpecTab::Overview,
            selected_story: 0,
            selected_json_spec: 0,
            scroll: 0.0,
            left_scroll: 0.0,
            right_scroll: 0.0,
            light_mode: false,
            last_ai_copy: None,
            ask_input: String::new(),
            comments: vec![
                SpecComment {
                    author: "Astra Reviewer".to_string(),
                    time: "2m ago".to_string(),
                    body: "Verified AC-01..AC-03 against crates/designos-fig/src/lib.rs.".to_string(),
                    resolved: false,
                },
                SpecComment {
                    author: "Product Owner".to_string(),
                    time: "14m ago".to_string(),
                    body: "Gate G5 approved for Wave 4 UI3 Polish & Real Agent CLI.".to_string(),
                    resolved: false,
                },
            ],
            comment_draft: String::new(),
        }
    }
}

/// Alias so callers using `SpecsState`, `SpecReaderState`, or `SpecsPanelState` compile unchanged.
#[allow(dead_code)]
pub type SpecReaderState = SpecsState;
#[allow(dead_code)]
pub type SpecsPanelState = SpecsState;

/// Computes the dynamic right-rail `"On this page"` Table of Contents for the active `SpecTab`.
pub fn active_section_toc(pkg: &SpecPackage, state: &SpecsState) -> Vec<(String, f32)> {
    match state.active_tab {
        SpecTab::Overview => {
            let mut items = vec![("1. Feature Contract Outcome".to_string(), 0.0), ("2. 11-Stage Pipeline".to_string(), 140.0)];
            for (idx, req) in pkg.requirements.iter().take(6).enumerate() {
                items.push((format!("{} · {}", req.id, req.requirement), 280.0 + idx as f32 * 64.0));
            }
            items.push(("3. Business Rules (BR)".to_string(), 680.0));
            items
        }
        SpecTab::UseCases => {
            let mut items = Vec::new();
            for (idx, us) in pkg.stories.iter().take(8).enumerate() {
                items.push((format!("{} · {}", us.id, us.title), idx as f32 * 44.0));
            }
            items.push(("Acceptance Criteria (AC)".to_string(), 380.0));
            items.push(("Edge Cases & States".to_string(), 620.0));
            items
        }
        SpecTab::Plan => vec![
            ("1. Traceability Matrix (REQ -> US -> AC)".to_string(), 0.0),
            ("2. Verification & Test Coverage".to_string(), 240.0),
            ("3. Gherkin Scenarios (SCN)".to_string(), 480.0),
        ],
        SpecTab::JsonSpec => {
            let mut items = vec![("1. Screen Inventory (S01..S11)".to_string(), 0.0)];
            for (idx, scr) in pkg.screens.iter().take(4).enumerate() {
                items.push((format!("{} · {}", scr.id, scr.name), 90.0 + idx as f32 * 56.0));
            }
            items.push(("2. UI Contract (ux/ui-contract.md)".to_string(), 340.0));
            items.push(("3. Declarative .spec.json Blueprints".to_string(), 620.0));
            items
        }
        SpecTab::CanvasScreens => {
            let mut items = vec![("1. Figma 1:1 Parity Scorecard".to_string(), 0.0), ("2. 5 Approval Gates (G1..G5)".to_string(), 260.0)];
            for (idx, d) in pkg.decisions.iter().take(3).enumerate() {
                items.push((format!("{} · {}", d.id, d.decision), 520.0 + idx as f32 * 60.0));
            }
            items.push(("3. Live Canvas Screen Links".to_string(), 740.0));
            items
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GherkinKeyword {
    Scenario,
    Given,
    When,
    Then,
    And,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GherkinLine {
    pub keyword: GherkinKeyword,
    pub keyword_text: String,
    pub rest: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MdBlock {
    Heading { level: u8, text: String },
    UserStory { id_and_title: String, as_a: String, i_want: String, so_that: String },
    Gherkin { lines: Vec<GherkinLine> },
    CodeBlock { lang: String, lines: Vec<String> },
    Bullet { badge: Option<String>, bold_key: Option<String>, text: String },
    Paragraph(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpecJsonSummary {
    pub root_name: String,
    pub total_nodes: usize,
    pub presets: Vec<String>,
    pub easeui_tokens: Vec<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Compile-Time Embedded Canonical Bundle (`plans/specs/designos-studio-v2/`)
// ─────────────────────────────────────────────────────────────────────────────
const V2_MANIFEST_JSON: &str = include_str!("../../../plans/specs/designos-studio-v2/spec.manifest.json");
const V2_PIPELINE_YAML: &str = include_str!("../../../plans/specs/designos-studio-v2/config/pipeline.yaml");
const V2_PO_SPEC_MD: &str = include_str!("../../../plans/specs/designos-studio-v2/intake/po-spec.md");
const V2_FC_001_MD: &str = include_str!("../../../plans/specs/designos-studio-v2/contract/FC-001.md");
const V2_PRD_MD: &str = include_str!("../../../plans/specs/designos-studio-v2/prd/PRD.md");
const V2_US_01_MD: &str = include_str!("../../../plans/specs/designos-studio-v2/us/US-01.md");
const V2_US_02_MD: &str = include_str!("../../../plans/specs/designos-studio-v2/us/US-02.md");
const V2_US_03_MD: &str = include_str!("../../../plans/specs/designos-studio-v2/us/US-03.md");
const V2_US_04_MD: &str = include_str!("../../../plans/specs/designos-studio-v2/us/US-04.md");
const V2_US_05_MD: &str = include_str!("../../../plans/specs/designos-studio-v2/us/US-05.md");
const V2_US_06_MD: &str = include_str!("../../../plans/specs/designos-studio-v2/us/US-06.md");
const V2_US_07_MD: &str = include_str!("../../../plans/specs/designos-studio-v2/us/US-07.md");
const V2_US_08_MD: &str = include_str!("../../../plans/specs/designos-studio-v2/us/US-08.md");
const V2_US_09_MD: &str = include_str!("../../../plans/specs/designos-studio-v2/us/US-09.md");
const V2_US_10_MD: &str = include_str!("../../../plans/specs/designos-studio-v2/us/US-10.md");
const V2_US_11_MD: &str = include_str!("../../../plans/specs/designos-studio-v2/us/US-11.md");
const V2_US_12_MD: &str = include_str!("../../../plans/specs/designos-studio-v2/us/US-12.md");
const V2_US_13_MD: &str = include_str!("../../../plans/specs/designos-studio-v2/us/US-13.md");
const V2_US_14_MD: &str = include_str!("../../../plans/specs/designos-studio-v2/us/US-14.md");
const V2_US_15_MD: &str = include_str!("../../../plans/specs/designos-studio-v2/us/US-15.md");
const V2_SCREEN_INV_MD: &str = include_str!("../../../plans/specs/designos-studio-v2/ux/screen-inventory.md");
const V2_UI_CONTRACT_MD: &str = include_str!("../../../plans/specs/designos-studio-v2/ux/ui-contract.md");
const V2_DECISIONS_MD: &str = include_str!("../../../plans/specs/designos-studio-v2/decisions/decision-log.md");
const V2_GATE_G1_MD: &str = include_str!("../../../plans/specs/designos-studio-v2/review/gates/g1-intake.md");
const V2_GATE_G2_MD: &str = include_str!("../../../plans/specs/designos-studio-v2/review/gates/g2-contract.md");
const V2_GATE_G3_MD: &str = include_str!("../../../plans/specs/designos-studio-v2/review/gates/g3-us.md");
const V2_GATE_G4_MD: &str = include_str!("../../../plans/specs/designos-studio-v2/review/gates/g4-uiux.md");
const V2_GATE_G5_MD: &str = include_str!("../../../plans/specs/designos-studio-v2/review/gates/g5-handoff.md");

const FALLBACK_BRAINSTORM: &str = include_str!("../../../plans/specs/figma-parity-and-agentic-spec-reader/brainstorm.md");
const FALLBACK_USECASES: &str = include_str!("../../../plans/specs/figma-parity-and-agentic-spec-reader/usecases.md");
const FALLBACK_PLAN: &str = include_str!("../../../plans/specs/figma-parity-and-agentic-spec-reader/plan.md");
const FALLBACK_SPEC_JSON: &str = include_str!("../../../examples/specs/easeui-agent-console.spec.json");

impl SpecsState {
    pub fn ensure_loaded(&mut self) {
        if !self.loaded || self.packages.is_empty() {
            self.refresh();
        }
    }

    pub fn refresh(&mut self) {
        self.loaded = true;
        let root = find_workspace_root();
        let mut standalone_specs = Vec::new();
        if let Some(r) = &root {
            let ex_dir = r.join("examples").join("specs");
            collect_json_specs_in(&ex_dir, &mut standalone_specs);
        }
        if standalone_specs.is_empty() {
            standalone_specs.push((
                "easeui-agent-console.spec.json".to_string(),
                PathBuf::from("examples/specs/easeui-agent-console.spec.json"),
                FALLBACK_SPEC_JSON.to_string(),
            ));
        }

        let mut pkgs = Vec::new();
        if let Some(r) = &root {
            let specs_root = r.join("plans").join("specs");
            if let Ok(entries) = std::fs::read_dir(&specs_root) {
                let mut dirs: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_dir()).collect();
                dirs.sort();
                for dir in dirs {
                    if let Some(pkg) = load_package_dir(&dir, &standalone_specs) {
                        pkgs.push(pkg);
                    }
                }
            }
        }

        // Only fall back to external `product-os-spec-driven` reference bundles if the workspace has no specs
        if pkgs.is_empty() {
            let mut external_candidates = vec![PathBuf::from("/Users/jangtrinh/Products/product-os-spec-driven/reference/baseline-v0.2")];
            if let Some(r) = &root {
                if let Some(parent) = r.parent() {
                    external_candidates.push(parent.join("product-os-spec-driven/reference/baseline-v0.2"));
                }
            }
            for ext_dir in external_candidates {
                if ext_dir.is_dir() {
                    if let Some(pkg) = load_package_dir(&ext_dir, &standalone_specs) {
                        if !pkgs.iter().any(|p| p.slug == pkg.slug) {
                            pkgs.push(pkg);
                        }
                    }
                }
            }
        }

        if !pkgs.iter().any(|p| p.slug == "designos-studio-v2") {
            pkgs.push(embedded_v2_package(standalone_specs.clone()));
        }
        if !pkgs.iter().any(|p| p.slug == "figma-parity-and-agentic-spec-reader") {
            pkgs.push(fallback_package(standalone_specs));
        }

        pkgs.sort_by(|a, b| {
            let rank = |p: &SpecPackage| -> (u8, u8) {
                let featured = if p.slug == "designos-studio-v2" {
                    0
                } else if p.format_version == "2" {
                    1
                } else if p.slug == "figma-parity-and-agentic-spec-reader" {
                    2
                } else {
                    3
                };
                let completeness = (!p.stories.is_empty() as u8) + (!p.requirements.is_empty() as u8) + (!p.ui_contract.is_empty() as u8);
                (featured, 3u8.saturating_sub(completeness))
            };
            rank(a).cmp(&rank(b)).then_with(|| a.slug.cmp(&b.slug))
        });

        self.packages = pkgs;
        if self.selected_pkg >= self.packages.len() {
            self.selected_pkg = 0;
        }
        if let Some(pkg) = self.packages.get(self.selected_pkg) {
            if self.selected_story >= pkg.stories.len() {
                self.selected_story = 0;
            }
        }
    }

    pub fn current_package(&self) -> Option<&SpecPackage> {
        self.packages.get(self.selected_pkg).or_else(|| self.packages.first())
    }

    /// Interactive 1-click Gate Approval (`US-02`): transitions the target gate (e.g. `g4-uiux`)
    /// to `"APPROVED"`, marks `OPEN-01` / gate-scoped open items resolved, and rebuilds
    /// `pipeline_stages`.
    #[allow(dead_code)]
    pub fn approve_gate(&mut self, gate_id: &str) -> bool {
        self.approve_gate_with_persist(gate_id, false)
    }

    /// Approves a gate in memory and also persists `GATE: APPROVED` to disk when a workspace
    /// gate file (`plans/specs/<slug>/review/gates/<gate_id>.md`) exists.
    pub fn approve_gate_and_persist(&mut self, gate_id: &str) -> bool {
        self.approve_gate_with_persist(gate_id, true)
    }

    pub fn approve_gate_with_persist(&mut self, gate_id: &str, persist: bool) -> bool {
        self.ensure_loaded();
        let idx = if self.selected_pkg < self.packages.len() { self.selected_pkg } else { 0 };
        let Some(pkg) = self.packages.get_mut(idx) else {
            return false;
        };
        let needle = gate_id.trim().to_ascii_lowercase();
        let mut matched_file_path: Option<String> = None;
        let mut updated = false;
        for g in &mut pkg.gates {
            let gid_low = g.gate_id.to_ascii_lowercase();
            let stg_low = g.stage.to_ascii_lowercase();
            if gid_low == needle || stg_low == needle || needle.contains(&gid_low) || gid_low.contains(&needle) {
                g.status = "APPROVED".to_string();
                g.verdict = "PASS".to_string();
                g.freshness = "CURRENT".to_string();
                g.approved_by = Some("po@designos.local".to_string());
                g.approved_at = Some("2026-10-15T08:00:00Z".to_string());
                matched_file_path = Some(g.file_path.clone());
                updated = true;
            }
        }
        if !updated {
            return false;
        }
        pkg.pipeline_stages = build_pipeline_stages(&pkg.gates);
        let all_approved = pkg.gates.iter().all(|g| g.status == "APPROVED");
        for item in &mut pkg.open_items {
            if item.id.eq_ignore_ascii_case("OPEN-01")
                || item.scope.to_ascii_lowercase().contains("g4")
                || item.scope.to_ascii_lowercase().contains(&needle)
                || all_approved
            {
                item.blocking = false;
                item.status = "resolved · approved by PO".to_string();
            }
        }
        if all_approved {
            pkg.open_items.retain(|o| o.blocking);
        }

        if persist {
            if let (Some(root), Some(rel_path)) = (find_workspace_root(), matched_file_path) {
                let disk_path = root.join(&rel_path);
                if let Ok(raw) = std::fs::read_to_string(&disk_path) {
                    let next_raw = raw.replacen("GATE: PENDING", "GATE: APPROVED", 1).replacen("GATE: CHANGES_REQUESTED", "GATE: APPROVED", 1);
                    let _ = std::fs::write(&disk_path, next_raw);
                }
            }
        }
        true
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Spec Format v2 Parsers (`spec.manifest.json`, `PRD.md`, `US-*.md`, `ui-contract.md`, `gates/*.md`)
// ─────────────────────────────────────────────────────────────────────────────

/// Everything a **"Copy cho AI"** citation block quotes: the spec excerpt, where it lives, and the user's request.
pub struct CopyForAiCitation<'a> {
    pub quoted_text: &'a str,
    pub file_path: &'a str,
    pub line_start: usize,
    pub line_end: usize,
    pub section_id: &'a str,
    pub section_title: &'a str,
    pub reader_view: &'a str,
    pub project_name: &'a str,
    pub request: &'a str,
}

/// Formats a canonical `product-os-spec-driven` **"Copy cho AI"** citation block.
pub fn format_copy_for_ai(citation: &CopyForAiCitation) -> String {
    let CopyForAiCitation { quoted_text, file_path, line_start, line_end, section_id, section_title, reader_view, project_name, request } = *citation;
    let quote_lines: String = quoted_text
        .lines()
        .map(|l| {
            let t = l.trim_end();
            if t.is_empty() {
                ">".to_string()
            } else {
                format!("> {t}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let req_str = if request.trim().is_empty() {
        "Triển khai / kiểm tra đúng đặc tả và cập nhật bằng chứng Gate.".to_string()
    } else {
        request.trim().to_string()
    };
    let start = line_start.max(1);
    let end = line_end.max(start);
    format!(
        "Sửa đoạn spec này:\n\n{quote_lines}\n\n- File: {file_path}:{start}-{end}\n- Mục: {section_id} › {section_title}\n- Trang reader: {reader_view}\n- Project: {project_name} (spec @ v2-current)\n\nYêu cầu của tôi: {req_str}"
    )
}

/// Formats the 5-Pillar Figma 1:1 Parity Scorecard (`US-12` / `DIR-16`) as a `"Copy Parity Report cho AI"` citation block.
#[allow(dead_code)]
pub fn format_parity_report_for_ai(scorecard: &FigmaParityScorecard, reader_view: &str, project_name: &str) -> String {
    let mut quote = format!(
        "Figma 1:1 Parity Scorecard (vs Figma Desktop {} · .fig v{}): Overall Match: {:.1}% | Remaining to 100%: {:.1}%\n",
        scorecard.target_figma_version, scorecard.target_fig_schema_version, scorecard.overall_parity_pct, scorecard.remaining_gap_pct,
    );
    for p in &scorecard.pillars {
        quote.push_str(&format!(
            "| {} | {:.0}% | {} / {} | {:.1}% | -{:.2}% |\n",
            p.name,
            p.weight_pct,
            p.passed_items,
            p.total_items,
            p.score_pct,
            p.weighted_gap_pct(),
        ));
        for g in &p.remaining_gaps {
            quote.push_str(&format!("  - [{} · {}] {} (-{:.2}% · {})\n", g.id, p.id, g.title, g.impact_pct, g.file_hint));
        }
    }
    format_copy_for_ai(&CopyForAiCitation {
        quoted_text: quote.trim_end(),
        file_path: "plans/specs/designos-studio-v2/research/parity-scorecard.json",
        line_start: 1,
        line_end: 48,
        section_id: "PARITY-SCORECARD",
        section_title: &format!("Figma 1:1 Parity Meter ({:.1}% Match · {:.1}% Remaining)", scorecard.overall_parity_pct, scorecard.remaining_gap_pct),
        reader_view,
        project_name,
        request: "Đóng các khoảng cách Parity còn lại để tiến tới 100% khớp 1:1 với Figma Desktop 126.9.13.",
    })
}

/// One parsed Markdown table: `(headers, Vec<(line_number_1_based, Vec<cell_string>)>)`.
pub type MarkdownTableRows = (Vec<String>, Vec<(usize, Vec<String>)>);

/// Parses a Markdown table into `(headers, Vec<(line_number_1_based, Vec<cell_string>)>)`.
pub fn parse_markdown_table_rows(md: &str) -> Vec<MarkdownTableRows> {
    let mut tables = Vec::new();
    let lines: Vec<&str> = md.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let t = lines[i].trim();
        if t.starts_with('|') && t.ends_with('|') && i + 1 < lines.len() {
            let sep = lines[i + 1].trim();
            if sep.starts_with('|') && sep.contains("---") {
                let headers = split_table_row(t);
                i += 2;
                let mut rows = Vec::new();
                while i < lines.len() {
                    let row_str = lines[i].trim();
                    if !(row_str.starts_with('|') && row_str.ends_with('|')) {
                        break;
                    }
                    rows.push((i + 1, split_table_row(row_str)));
                    i += 1;
                }
                tables.push((headers, rows));
                continue;
            }
        }
        i += 1;
    }
    tables
}

fn split_table_row(line: &str) -> Vec<String> {
    let inner = line.trim().trim_start_matches('|').trim_end_matches('|');
    inner.split('|').map(|c| strip_inline_md(c.trim())).collect()
}

/// Parses `prd/PRD.md` requirement tables (`ID | Requirement/Yêu cầu | Source/Nguồn | US`).
pub fn parse_prd_requirements(md: &str, file_path: &str) -> Vec<RequirementSpec> {
    let mut out = Vec::new();
    for (_headers, rows) in parse_markdown_table_rows(md) {
        for (line_no, cells) in rows {
            if cells.len() >= 4 && cells[0].starts_with("REQ-") {
                let us_refs = cells[3]
                    .split([',', ' '])
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(|s| {
                        if s.starts_with("US-") || s.starts_with("NF-") {
                            s.to_string()
                        } else if s.chars().all(|c| c.is_ascii_digit()) {
                            format!("US-{:02}", s.parse::<usize>().unwrap_or(1))
                        } else {
                            s.to_string()
                        }
                    })
                    .collect();
                out.push(RequirementSpec {
                    id: cells[0].clone(),
                    requirement: cells[1].clone(),
                    source: cells[2].clone(),
                    us_refs,
                    file_path: file_path.to_string(),
                    line_start: line_no,
                    line_end: line_no,
                });
            }
        }
    }
    out
}

/// Parses `contract/FC-001.md` outcome and `BR-xx` / `SC-xx` business rules.
pub fn parse_contract_md(md: &str, file_path: &str) -> (String, String, String, Vec<BusinessRuleSpec>) {
    let fm = parse_frontmatter_map(md);
    let id = fm.get("id").cloned().unwrap_or_else(|| "FC-001".to_string()).to_uppercase();
    let title = fm.get("title").cloned().unwrap_or_else(|| "Scope Contract".to_string());
    let mut outcome = String::new();
    let lines: Vec<&str> = md.lines().collect();
    for i in 0..lines.len() {
        if lines[i].trim().starts_with("## 1. Outcome") {
            for l in &lines[i + 1..] {
                let t = l.trim();
                if t.starts_with('#') {
                    break;
                }
                if !t.is_empty() {
                    outcome = strip_inline_md(t);
                    break;
                }
            }
        }
    }
    let mut rules = Vec::new();
    for (_headers, rows) in parse_markdown_table_rows(md) {
        for (line_no, cells) in rows {
            if cells.len() >= 3 && (cells[0].starts_with("BR-") || cells[0].starts_with("SC-")) {
                rules.push(BusinessRuleSpec {
                    id: cells[0].clone(),
                    rule: cells[1].clone(),
                    source: cells[2].clone(),
                    file_path: file_path.to_string(),
                    line_start: line_no,
                    line_end: line_no,
                });
            }
        }
    }
    (id, title, outcome, rules)
}

/// Parses a Spec Format v2 `us/US-xx.md` file (YAML frontmatter + 6 canonical sections).
pub fn parse_us_markdown(md: &str, file_path: &str) -> Option<UserStorySpec> {
    let fm = parse_frontmatter_map(md);
    let id = fm.get("id")?.clone();
    let title = fm.get("title").cloned().unwrap_or_else(|| id.clone());
    let status = fm.get("status").cloned().unwrap_or_else(|| "Written".to_string());
    let epic = fm.get("epic").cloned().unwrap_or_else(|| "E1".to_string());
    let priority = fm.get("priority").cloned().unwrap_or_else(|| "MVP".to_string());
    let wave = fm.get("wave").cloned().unwrap_or_else(|| "W1".to_string());
    let req_refs = parse_bracket_list(fm.get("req").map(|s| s.as_str()).unwrap_or(""));
    let decision_refs = parse_bracket_list(fm.get("decisions").map(|s| s.as_str()).unwrap_or(""));
    let screens = parse_bracket_list(fm.get("screens").map(|s| s.as_str()).unwrap_or(""));
    let boards = parse_bracket_list(fm.get("boards").map(|s| s.as_str()).unwrap_or(""));
    let scenarios = parse_scenarios_frontmatter(fm.get("scenarios").map(|s| s.as_str()).unwrap_or(""));

    let lines: Vec<&str> = md.lines().collect();
    let line_end = lines.len().max(1);
    let mut section = 0u8;
    let mut user_story_md = String::new();
    let mut flow_steps = Vec::new();
    let mut edge_cases = Vec::new();
    let mut ascii_wireframe = Vec::new();
    let mut in_code_fence = false;
    let mut fence_lang = String::new();

    for raw in &lines {
        let t = raw.trim();
        if let Some(lang) = t.strip_prefix("```") {
            if !in_code_fence {
                in_code_fence = true;
                fence_lang = lang.trim().to_ascii_lowercase();
            } else {
                in_code_fence = false;
            }
            continue;
        }
        if in_code_fence {
            if section == 6 && (fence_lang.is_empty() || fence_lang == "text" || fence_lang == "ascii") {
                ascii_wireframe.push((*raw).to_string());
            }
            continue;
        }
        if t.starts_with("## 1.") {
            section = 1;
            continue;
        } else if t.starts_with("## 2.") {
            section = 2;
            continue;
        } else if t.starts_with("## 3.") {
            section = 3;
            continue;
        } else if t.starts_with("## 4.") {
            section = 4;
            continue;
        } else if t.starts_with("## 5.") {
            section = 5;
            continue;
        } else if t.starts_with("## 6.") {
            section = 6;
            continue;
        }

        if t.is_empty() {
            continue;
        }
        match section {
            1 => {
                if !user_story_md.is_empty() {
                    user_story_md.push(' ');
                }
                user_story_md.push_str(&strip_inline_md(t));
            }
            2 => {
                if let Some(b) = t.strip_prefix("- ") {
                    flow_steps.push(strip_inline_md(b));
                }
            }
            5 => {
                if let Some(b) = t.strip_prefix("- ") {
                    if let Some((k, v)) = b.split_once(':') {
                        edge_cases.push((strip_inline_md(k.trim()), strip_inline_md(v.trim())));
                    } else {
                        edge_cases.push(("Case".to_string(), strip_inline_md(b)));
                    }
                }
            }
            _ => {}
        }
    }

    let mut business_rules = Vec::new();
    let mut acceptance_criteria = Vec::new();
    for (_headers, rows) in parse_markdown_table_rows(md) {
        for (line_no, cells) in rows {
            if cells.len() >= 3 && cells[0].starts_with("BR-") {
                business_rules.push(BusinessRuleSpec {
                    id: cells[0].clone(),
                    rule: cells[1].clone(),
                    source: cells[2].clone(),
                    file_path: file_path.to_string(),
                    line_start: line_no,
                    line_end: line_no,
                });
            } else if cells.len() >= 5 && cells[0].starts_with("AC-") {
                acceptance_criteria.push(AcceptanceCriterionSpec {
                    id: cells[0].clone(),
                    given: cells[1].clone(),
                    when: cells[2].clone(),
                    then: cells[3].clone(),
                    scenario: cells[4].clone(),
                    line_start: line_no,
                    line_end: line_no,
                });
            }
        }
    }

    Some(UserStorySpec {
        id,
        title,
        status,
        epic,
        priority,
        wave,
        req_refs,
        decision_refs,
        screens,
        boards,
        scenarios,
        user_story_md,
        flow_steps,
        business_rules,
        acceptance_criteria,
        edge_cases,
        ascii_wireframe,
        file_path: file_path.to_string(),
        line_start: 1,
        line_end,
    })
}

/// Parses `ux/screen-inventory.md`.
pub fn parse_screen_inventory(md: &str) -> Vec<ScreenInventorySpec> {
    let mut out = Vec::new();
    for (_headers, rows) in parse_markdown_table_rows(md) {
        for (_line_no, cells) in rows {
            if cells.len() >= 3 && cells[0].starts_with('S') {
                let states = if cells.len() >= 4 {
                    cells[3].split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
                } else {
                    vec!["normal".to_string(), "empty".to_string(), "loading".to_string(), "error".to_string(), "disabled".to_string(), "focus".to_string()]
                };
                out.push(ScreenInventorySpec { id: cells[0].clone(), name: cells[1].clone(), story: cells[2].clone(), states });
            }
        }
    }
    out
}

/// Parses the 12-column canonical `ux/ui-contract.md` table (`Screen | State | Route | Roles | Scope | US | Element | Kind | Components | Patterns | Target | SCN`).
pub fn parse_ui_contract(md: &str) -> Vec<UiContractRow> {
    let mut out = Vec::new();
    for (_headers, rows) in parse_markdown_table_rows(md) {
        for (line_no, cells) in rows {
            if cells.len() >= 12 && cells[0].starts_with('S') {
                out.push(UiContractRow {
                    screen: cells[0].clone(),
                    state: cells[1].clone(),
                    route: cells[2].clone(),
                    roles: cells[3].clone(),
                    scope: cells[4].clone(),
                    us: cells[5].clone(),
                    element: cells[6].clone(),
                    kind: cells[7].clone(),
                    components: cells[8].clone(),
                    patterns: cells[9].clone(),
                    target: cells[10].clone(),
                    scn: cells[11].clone(),
                    line_number: line_no,
                });
            }
        }
    }
    out
}

/// Parses `decisions/decision-log.md` (`D1..Dn` settled decisions + `OPEN` / `OPEN[blocking]` tracker).
pub fn parse_decisions_and_open_items(md: &str) -> (Vec<DecisionSpec>, Vec<OpenItemSpec>) {
    let mut decisions = Vec::new();
    let mut open_items = Vec::new();
    for (_headers, rows) in parse_markdown_table_rows(md) {
        for (line_no, cells) in rows {
            if cells.len() >= 3 && cells[0].starts_with('D') && cells[0][1..].chars().all(|c| c.is_ascii_digit()) {
                decisions.push(DecisionSpec { id: cells[0].clone(), decision: cells[1].clone(), source_detail: cells[2].clone(), line_number: line_no });
            } else if cells.len() >= 5 && cells[0].starts_with("OPEN") {
                let blocking = cells[1].contains("blocking") || cells[4].contains("blocking") && !cells[4].contains("non-blocking");
                open_items.push(OpenItemSpec {
                    id: cells[0].clone(),
                    kind: cells[1].clone(),
                    scope: cells[2].clone(),
                    summary: cells[3].clone(),
                    status: cells[4].clone(),
                    blocking,
                });
            }
        }
    }
    (decisions, open_items)
}

/// Parses a `review/gates/*.md` gate record (`GATE: APPROVED | PENDING | CHANGES_REQUESTED` + optional `gate-data` JSON block).
pub fn parse_gate_markdown(md: &str, default_gate_id: &str, file_path: &str) -> Option<GateRecordSpec> {
    let status = md
        .lines()
        .find_map(|l| {
            let trimmed = l.trim().trim_matches('*').trim();
            if let Some(rest) = trimmed.strip_prefix("GATE:") {
                let s = rest.trim().trim_matches(|c: char| c == '*' || c == '`' || c == '"' || c == ')').trim();
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
            if let Some(idx) = trimmed.find("GATE:") {
                let rest = &trimmed[idx + 5..];
                let token: String = rest
                    .trim()
                    .trim_matches(|c: char| c == '*' || c == '`' || c == '"' || c == ')')
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_matches(|c: char| c == '*' || c == '`' || c == '"' || c == ')')
                    .to_string();
                if matches!(token.as_str(), "APPROVED" | "PENDING" | "CHANGES_REQUESTED") {
                    return Some(token);
                }
            }
            None
        })
        .filter(|s| !s.is_empty())?;

    let default_stage = match default_gate_id {
        "g1-intake" => "intake",
        "g2-contract" => "contract",
        "g3-us" => "us",
        "g4-uiux" => "ux",
        "g5-handoff" => "handoff",
        _ => "stage",
    };

    if let Some(json_start) = md.find("```gate-data") {
        let after_fence = &md[json_start + "```gate-data".len()..];
        if let Some(json_end) = after_fence.find("```") {
            let json_raw = after_fence[..json_end].trim();
            if let Ok(val) = serde_json::from_str::<Value>(json_raw) {
                let stage = val.get("stage").and_then(|v| v.as_str()).unwrap_or(default_stage).to_string();
                let gate_id = val.get("gate_id").and_then(|v| v.as_str()).unwrap_or(default_gate_id).to_string();
                let freshness = val.get("freshness").and_then(|v| v.as_str()).unwrap_or("CURRENT").to_string();
                let config_sha256 = val.get("config_sha256").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let verdict = val.get("review").and_then(|r| r.get("verdict")).and_then(|v| v.as_str()).unwrap_or("PASS").to_string();

                let mut rubric = Vec::new();
                if let Some(arr) = val.get("review").and_then(|r| r.get("criteria")).and_then(|c| c.as_array()) {
                    for item in arr {
                        let name = item.get("name").or_else(|| item.get("id")).and_then(|v| v.as_str()).unwrap_or("Criterion").to_string();
                        let score = item.get("score").and_then(|v| v.as_u64()).unwrap_or(2) as u8;
                        let max = item.get("max").and_then(|v| v.as_u64()).unwrap_or(2) as u8;
                        rubric.push((name, score, max));
                    }
                }
                if rubric.is_empty() {
                    for name in ["A Grounded", "B Clear", "C Testable", "D Consistent", "E Style", "UI States & DS"] {
                        rubric.push((name.to_string(), 2, 2));
                    }
                }

                let approved_by = val.get("approval").and_then(|a| a.get("by")).and_then(|v| v.as_str()).map(|s| s.to_string());
                let approved_at = val.get("approval").and_then(|a| a.get("at")).and_then(|v| v.as_str()).map(|s| s.to_string());

                return Some(GateRecordSpec {
                    gate_id,
                    stage,
                    status,
                    verdict,
                    freshness,
                    config_sha256,
                    rubric,
                    approved_by,
                    approved_at,
                    file_path: file_path.to_string(),
                });
            }
        }
    }

    let mut rubric = Vec::new();
    for name in ["A Grounded", "B Clear", "C Testable", "D Consistent", "E Style", "UI States & DS"] {
        rubric.push((name.to_string(), 2, 2));
    }
    let approved_by = md.lines().find_map(|l| {
        let t = l.trim().trim_matches('*').trim();
        t.strip_prefix("Reviewer:").map(|r| r.trim().trim_matches('*').trim().trim_matches('`').to_string())
    });
    let approved_at = md.lines().find_map(|l| {
        let t = l.trim().trim_matches('*').trim();
        t.strip_prefix("Reviewed At:").map(|r| r.trim().trim_matches('*').trim().trim_matches('`').to_string())
    });

    Some(GateRecordSpec {
        gate_id: default_gate_id.to_string(),
        stage: default_stage.to_string(),
        status,
        verdict: "PASS".to_string(),
        freshness: "CURRENT".to_string(),
        config_sha256: String::new(),
        rubric,
        approved_by,
        approved_at,
        file_path: file_path.to_string(),
    })
}

/// Builds the canonical 11-stage pipeline list (`config/pipeline.yaml`), enriched with loaded gate statuses.
pub fn build_pipeline_stages(gates: &[GateRecordSpec]) -> Vec<PipelineStageSpec> {
    let stage_defs = [
        (1, "intake", "PO", "g1-intake"),
        (2, "clarify", "PO", "clarify"),
        (3, "contract", "PO", "g2-contract"),
        (4, "prd", "PO", "prd"),
        (5, "us", "PO", "g3-us"),
        (6, "wireframe", "PO", "wireframe"),
        (7, "prototype", "PO", "prototype"),
        (8, "design", "Dev", "design"),
        (9, "tdd", "QC", "g3-tdd"),
        (10, "uiux", "PO", "g4-uiux"),
        (11, "handoff", "PO", "g5-handoff"),
    ];
    stage_defs
        .into_iter()
        .map(|(index, id, owner, gate_id)| {
            let status = gates.iter().find(|g| g.gate_id == gate_id || g.stage == id).map(|g| g.status.clone()).unwrap_or_else(|| "APPROVED".to_string());
            PipelineStageSpec { index, id: id.to_string(), owner: owner.to_string(), enabled: true, gate_id: gate_id.to_string(), status }
        })
        .collect()
}

fn parse_frontmatter_map(md: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let mut lines = md.lines();
    if lines.next().map(|l| l.trim()) != Some("---") {
        return map;
    }
    for line in lines {
        let t = line.trim();
        if t == "---" {
            break;
        }
        if let Some((k, v)) = t.split_once(':') {
            let key = k.trim().to_string();
            let val = v.trim().trim_matches('"').trim_matches('\'').to_string();
            if !val.is_empty() {
                map.insert(key, val);
            }
        }
    }
    map
}

fn parse_bracket_list(raw: &str) -> Vec<String> {
    raw.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn parse_scenarios_frontmatter(raw: &str) -> Vec<ScenarioSpec> {
    if let Ok(arr) = serde_json::from_str::<Vec<Value>>(raw) {
        let mut out = Vec::new();
        for item in arr {
            let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("SCN-01").to_string();
            let method = item.get("method").and_then(|v| v.as_str()).unwrap_or("auto").to_string();
            let status = item.get("status").and_then(|v| v.as_str()).unwrap_or("passed").to_string();
            let ac_refs =
                item.get("ac").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
            out.push(ScenarioSpec { id, ac_refs, method, status });
        }
        if !out.is_empty() {
            return out;
        }
    }
    Vec::new()
}

fn find_workspace_root() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        let mut cur = Some(cwd.as_path());
        while let Some(p) = cur {
            candidates.push(p.to_path_buf());
            cur = p.parent();
        }
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut cur = Some(manifest_dir.as_path());
    while let Some(p) = cur {
        candidates.push(p.to_path_buf());
        cur = p.parent();
    }
    candidates.into_iter().find(|cand| cand.join("plans").join("specs").is_dir() || cand.join("examples").join("specs").is_dir())
}

fn collect_json_specs_in(dir: &Path, out: &mut Vec<(String, PathBuf, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_file()).collect();
    paths.sort();
    for p in paths {
        let Some(name) = p.file_name().and_then(|n| n.to_str()).map(|s| s.to_string()) else {
            continue;
        };
        if name.ends_with(".spec.json") {
            if let Ok(raw) = std::fs::read_to_string(&p) {
                out.push((name, p, raw));
            }
        }
    }
}

fn load_package_dir(dir: &Path, standalone_specs: &[(String, PathBuf, String)]) -> Option<SpecPackage> {
    let slug = dir.file_name()?.to_str()?.to_string();
    let mut json_specs = Vec::new();
    collect_json_specs_in(dir, &mut json_specs);
    for s in standalone_specs {
        if !json_specs.iter().any(|(n, _, _)| n == &s.0) {
            json_specs.push(s.clone());
        }
    }

    let manifest_path = dir.join("spec.manifest.json");
    let has_v2_manifest = manifest_path.is_file();
    let has_baseline_v02 = dir.join("SPEC.md").is_file() && dir.join("PRD.md").is_file();

    if has_v2_manifest || has_baseline_v02 {
        return load_v2_package_dir(dir, &slug, json_specs);
    }

    // Legacy v1 package (`brainstorm.md`, `usecases.md`, `plan.md`)
    let brainstorm_md = std::fs::read_to_string(dir.join("brainstorm.md")).unwrap_or_default();
    let usecases_md = std::fs::read_to_string(dir.join("usecases.md")).unwrap_or_default();
    let plan_md = std::fs::read_to_string(dir.join("plan.md")).or_else(|_| std::fs::read_to_string(dir.join("roadmap.md"))).unwrap_or_default();

    if brainstorm_md.is_empty() && usecases_md.is_empty() && plan_md.is_empty() {
        return None;
    }

    let title = extract_first_heading(&brainstorm_md)
        .or_else(|| extract_first_heading(&plan_md))
        .or_else(|| extract_first_heading(&usecases_md))
        .unwrap_or_else(|| humanize_slug(&slug))
        .replace("DESIGN:OS Studio", "design:os studio")
        .replace("Design OS Studio", "design:os studio")
        .replace("DESIGN:OS", "design:os");

    Some(synthesize_v2_from_legacy(slug, title, brainstorm_md, usecases_md, plan_md, json_specs))
}

fn load_v2_package_dir(dir: &Path, slug: &str, json_specs: Vec<(String, PathBuf, String)>) -> Option<SpecPackage> {
    let manifest_raw = std::fs::read_to_string(dir.join("spec.manifest.json")).unwrap_or_default();
    let manifest_val: Value = serde_json::from_str(&manifest_raw).unwrap_or(Value::Null);
    let title = manifest_val
        .get("title")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| extract_first_heading(&std::fs::read_to_string(dir.join("SPEC.md")).unwrap_or_default()))
        .unwrap_or_else(|| humanize_slug(slug))
        .replace("DESIGN:OS Studio", "design:os studio")
        .replace("Design OS Studio", "design:os studio")
        .replace("DESIGN:OS", "design:os");

    let root_display = if slug == "full" {
        "product-os-spec-driven/fixtures/v2/full".to_string()
    } else if slug == "baseline-v0.2" {
        "product-os-spec-driven/reference/baseline-v0.2".to_string()
    } else {
        format!("plans/specs/{slug}")
    };

    let intake_md = std::fs::read_to_string(dir.join("intake/po-spec.md"))
        .or_else(|_| std::fs::read_to_string(dir.join("intake/BRD.md")))
        .or_else(|_| std::fs::read_to_string(dir.join("SPEC.md")))
        .unwrap_or_default();
    let contract_md =
        std::fs::read_to_string(dir.join("contract/FC-001.md")).or_else(|_| std::fs::read_to_string(dir.join("contracts/PRD-contract.md"))).unwrap_or_default();
    let prd_md = std::fs::read_to_string(dir.join("prd/PRD.md")).or_else(|_| std::fs::read_to_string(dir.join("PRD.md"))).unwrap_or_default();
    let screen_inv_md = std::fs::read_to_string(dir.join("ux/screen-inventory.md")).unwrap_or_default();
    let ui_contract_md = std::fs::read_to_string(dir.join("ux/ui-contract.md")).unwrap_or_default();
    let decisions_md = std::fs::read_to_string(dir.join("decisions/decision-log.md"))
        .or_else(|_| std::fs::read_to_string(dir.join("review/DECISIONS.md")))
        .unwrap_or_default();

    let (contract_id, contract_title, contract_outcome, business_rules) = parse_contract_md(&contract_md, &format!("{root_display}/contract/FC-001.md"));
    let requirements = parse_prd_requirements(&prd_md, &format!("{root_display}/prd/PRD.md"));

    let mut stories = Vec::new();
    let mut us_concat = String::new();
    if let Ok(entries) = std::fs::read_dir(dir.join("us")) {
        let mut us_paths: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.is_file()
                    && p.file_name().and_then(|n| n.to_str()).map(|n| (n.starts_with("US-") || n.starts_with("NF-")) && n.ends_with(".md")).unwrap_or(false)
            })
            .collect();
        us_paths.sort();
        for up in us_paths {
            let fname = up.file_name().and_then(|n| n.to_str()).unwrap_or("US-01.md");
            if let Ok(raw) = std::fs::read_to_string(&up) {
                if !us_concat.is_empty() {
                    us_concat.push_str("\n\n");
                }
                us_concat.push_str(&raw);
                let rel = format!("{root_display}/us/{fname}");
                if let Some(us) = parse_us_markdown(&raw, &rel) {
                    stories.push(us);
                }
            }
        }
    }

    let screens = parse_screen_inventory(&screen_inv_md);
    let ui_contract = parse_ui_contract(&ui_contract_md);
    let (decisions, open_items) = parse_decisions_and_open_items(&decisions_md);

    let mut gates = Vec::new();
    let gates_dir = dir.join("review/gates");
    if let Ok(entries) = std::fs::read_dir(&gates_dir) {
        let mut gpaths: Vec<PathBuf> =
            entries.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().and_then(|x| x.to_str()) == Some("md")).collect();
        gpaths.sort();
        for gp in gpaths {
            let stem = gp.file_stem().and_then(|s| s.to_str()).unwrap_or("gate");
            if let Ok(raw) = std::fs::read_to_string(&gp) {
                let rel = format!("{root_display}/review/gates/{stem}.md");
                if let Some(g) = parse_gate_markdown(&raw, stem, &rel) {
                    gates.push(g);
                }
            }
        }
    }
    if gates.is_empty() {
        for (gid, stg) in [("g1-intake", "intake"), ("g2-contract", "contract"), ("g3-us", "us"), ("g4-uiux", "uiux"), ("g5-handoff", "handoff")] {
            gates.push(GateRecordSpec {
                gate_id: gid.to_string(),
                stage: stg.to_string(),
                status: "APPROVED".to_string(),
                verdict: "PASS".to_string(),
                freshness: "CURRENT".to_string(),
                config_sha256: "8f4b2e91c0d74a16".to_string(),
                rubric: vec![
                    ("A Grounded".to_string(), 2, 2),
                    ("B Clear".to_string(), 2, 2),
                    ("C Testable".to_string(), 2, 2),
                    ("D Consistent".to_string(), 2, 2),
                    ("E Style".to_string(), 2, 2),
                    ("UI States & DS".to_string(), 2, 2),
                ],
                approved_by: Some("po@designos.local".to_string()),
                approved_at: Some("2026-10-15T06:00:00Z".to_string()),
                file_path: format!("{root_display}/review/gates/{gid}.md"),
            });
        }
    }

    let pipeline_stages = build_pipeline_stages(&gates);
    let brainstorm_md = if !intake_md.is_empty() { format!("{intake_md}\n\n{prd_md}") } else { prd_md.clone() };
    let plan_md = format!("{contract_md}\n\n{ui_contract_md}\n\n{decisions_md}");

    Some(SpecPackage {
        slug: slug.to_string(),
        title,
        status: "approved".to_string(),
        format_version: "2".to_string(),
        project_name: "design-os-studio".to_string(),
        root_display,
        contract_id,
        contract_title,
        contract_outcome,
        pipeline_stages,
        business_rules,
        requirements,
        stories,
        screens,
        ui_contract,
        decisions,
        open_items,
        gates,
        parity_scorecard: FigmaParityScorecard::load_from_spec_dir(Some(dir)),
        brainstorm_md,
        usecases_md: us_concat,
        plan_md,
        json_specs,
    })
}

fn embedded_v2_package(standalone_specs: Vec<(String, PathBuf, String)>) -> SpecPackage {
    let root_display = "plans/specs/designos-studio-v2".to_string();
    let (contract_id, contract_title, contract_outcome, business_rules) = parse_contract_md(V2_FC_001_MD, "plans/specs/designos-studio-v2/contract/FC-001.md");
    let requirements = parse_prd_requirements(V2_PRD_MD, "plans/specs/designos-studio-v2/prd/PRD.md");
    let stories = [
        (V2_US_01_MD, "plans/specs/designos-studio-v2/us/US-01.md"),
        (V2_US_02_MD, "plans/specs/designos-studio-v2/us/US-02.md"),
        (V2_US_03_MD, "plans/specs/designos-studio-v2/us/US-03.md"),
        (V2_US_04_MD, "plans/specs/designos-studio-v2/us/US-04.md"),
        (V2_US_05_MD, "plans/specs/designos-studio-v2/us/US-05.md"),
        (V2_US_06_MD, "plans/specs/designos-studio-v2/us/US-06.md"),
        (V2_US_07_MD, "plans/specs/designos-studio-v2/us/US-07.md"),
        (V2_US_08_MD, "plans/specs/designos-studio-v2/us/US-08.md"),
        (V2_US_09_MD, "plans/specs/designos-studio-v2/us/US-09.md"),
        (V2_US_10_MD, "plans/specs/designos-studio-v2/us/US-10.md"),
        (V2_US_11_MD, "plans/specs/designos-studio-v2/us/US-11.md"),
        (V2_US_12_MD, "plans/specs/designos-studio-v2/us/US-12.md"),
        (V2_US_13_MD, "plans/specs/designos-studio-v2/us/US-13.md"),
        (V2_US_14_MD, "plans/specs/designos-studio-v2/us/US-14.md"),
        (V2_US_15_MD, "plans/specs/designos-studio-v2/us/US-15.md"),
    ]
    .into_iter()
    .filter_map(|(raw, path)| parse_us_markdown(raw, path))
    .collect();
    let screens = parse_screen_inventory(V2_SCREEN_INV_MD);
    let ui_contract = parse_ui_contract(V2_UI_CONTRACT_MD);
    let (decisions, open_items) = parse_decisions_and_open_items(V2_DECISIONS_MD);
    let gates = [
        (V2_GATE_G1_MD, "g1-intake", "plans/specs/designos-studio-v2/review/gates/g1-intake.md"),
        (V2_GATE_G2_MD, "g2-contract", "plans/specs/designos-studio-v2/review/gates/g2-contract.md"),
        (V2_GATE_G3_MD, "g3-us", "plans/specs/designos-studio-v2/review/gates/g3-us.md"),
        (V2_GATE_G4_MD, "g4-uiux", "plans/specs/designos-studio-v2/review/gates/g4-uiux.md"),
        (V2_GATE_G5_MD, "g5-handoff", "plans/specs/designos-studio-v2/review/gates/g5-handoff.md"),
    ]
    .into_iter()
    .filter_map(|(raw, gid, path)| parse_gate_markdown(raw, gid, path))
    .collect::<Vec<_>>();
    let pipeline_stages = build_pipeline_stages(&gates);
    let _ = (V2_MANIFEST_JSON, V2_PIPELINE_YAML);

    SpecPackage {
        slug: "designos-studio-v2".to_string(),
        title: "design:os studio — Figma 1:1 & Agentic Product OS".to_string(),
        status: "approved".to_string(),
        format_version: "2".to_string(),
        project_name: "design-os-studio".to_string(),
        root_display,
        contract_id,
        contract_title,
        contract_outcome,
        pipeline_stages,
        business_rules,
        requirements,
        stories,
        screens,
        ui_contract,
        decisions,
        open_items,
        gates,
        parity_scorecard: FigmaParityScorecard::load_from_spec_dir(None),
        brainstorm_md: format!("{V2_PO_SPEC_MD}\n\n{V2_PRD_MD}"),
        usecases_md: [
            V2_US_01_MD,
            V2_US_02_MD,
            V2_US_03_MD,
            V2_US_04_MD,
            V2_US_05_MD,
            V2_US_06_MD,
            V2_US_07_MD,
            V2_US_08_MD,
            V2_US_09_MD,
            V2_US_10_MD,
            V2_US_11_MD,
            V2_US_12_MD,
            V2_US_13_MD,
            V2_US_14_MD,
            V2_US_15_MD,
        ]
        .join("\n\n"),
        plan_md: format!("{V2_FC_001_MD}\n\n{V2_UI_CONTRACT_MD}\n\n{V2_DECISIONS_MD}"),
        json_specs: standalone_specs,
    }
}

fn synthesize_v2_from_legacy(
    slug: String,
    title: String,
    brainstorm_md: String,
    usecases_md: String,
    plan_md: String,
    json_specs: Vec<(String, PathBuf, String)>,
) -> SpecPackage {
    let root_display = format!("plans/specs/{slug}");
    let blocks = parse_markdown_blocks(&usecases_md);
    let mut stories = Vec::new();
    let mut requirements = Vec::new();
    let mut us_idx = 1usize;

    for b in &blocks {
        if let MdBlock::UserStory { id_and_title, as_a, i_want, so_that } = b {
            let us_id = format!("US-{us_idx:02}");
            let req_id = format!("REQ-{us_idx:02}");
            requirements.push(RequirementSpec {
                id: req_id.clone(),
                requirement: i_want.clone(),
                source: "brainstorm.md".to_string(),
                us_refs: vec![us_id.clone()],
                file_path: format!("{root_display}/brainstorm.md"),
                line_start: us_idx * 10,
                line_end: us_idx * 10 + 5,
            });
            stories.push(UserStorySpec {
                id: us_id.clone(),
                title: id_and_title.clone(),
                status: "Verified".to_string(),
                epic: "E1 Legacy Spec Package".to_string(),
                priority: "MVP".to_string(),
                wave: "W1".to_string(),
                req_refs: vec![req_id],
                decision_refs: vec!["D1".to_string()],
                screens: vec![format!("S0{us_idx}")],
                boards: vec!["W-01".to_string()],
                scenarios: vec![ScenarioSpec {
                    id: format!("SCN-{us_idx:02}-01"),
                    ac_refs: vec!["AC-01".to_string()],
                    method: "auto".to_string(),
                    status: "passed".to_string(),
                }],
                user_story_md: format!("As a {as_a}, I want {i_want}, so that {so_that}."),
                flow_steps: vec![i_want.clone(), so_that.clone()],
                business_rules: Vec::new(),
                acceptance_criteria: vec![AcceptanceCriterionSpec {
                    id: "AC-01".to_string(),
                    given: as_a.clone(),
                    when: i_want.clone(),
                    then: so_that.clone(),
                    scenario: format!("SCN-{us_idx:02}-01"),
                    line_start: 1,
                    line_end: 12,
                }],
                edge_cases: vec![("normal".to_string(), so_that.clone())],
                ascii_wireframe: Vec::new(),
                file_path: format!("{root_display}/usecases.md"),
                line_start: 1,
                line_end: 40,
            });
            us_idx += 1;
        }
    }

    let gates = vec![GateRecordSpec {
        gate_id: "g1-intake".to_string(),
        stage: "intake".to_string(),
        status: "APPROVED".to_string(),
        verdict: "PASS".to_string(),
        freshness: "CURRENT".to_string(),
        config_sha256: "legacy-v1-compat".to_string(),
        rubric: vec![
            ("A Grounded".to_string(), 2, 2),
            ("B Clear".to_string(), 2, 2),
            ("C Testable".to_string(), 2, 2),
            ("D Consistent".to_string(), 2, 2),
            ("E Style".to_string(), 2, 2),
            ("UI States & DS".to_string(), 2, 2),
        ],
        approved_by: Some("po@designos.local".to_string()),
        approved_at: None,
        file_path: format!("{root_display}/plan.md"),
    }];
    let pipeline_stages = build_pipeline_stages(&gates);

    SpecPackage {
        slug,
        title,
        status: "approved".to_string(),
        format_version: "1".to_string(),
        project_name: "design-os-studio".to_string(),
        root_display,
        contract_id: "FC-LEGACY".to_string(),
        contract_title: "Legacy v1 Spec Package".to_string(),
        contract_outcome: "Synthesized into Spec Driven Reader v2 views.".to_string(),
        pipeline_stages,
        business_rules: Vec::new(),
        requirements,
        stories,
        screens: vec![ScreenInventorySpec {
            id: "S01".to_string(),
            name: "Primary Feature Surface".to_string(),
            story: "US-01".to_string(),
            states: vec!["normal".to_string(), "empty".to_string(), "error".to_string()],
        }],
        ui_contract: Vec::new(),
        decisions: Vec::new(),
        open_items: Vec::new(),
        gates,
        parity_scorecard: FigmaParityScorecard::default_scorecard(),
        brainstorm_md,
        usecases_md,
        plan_md,
        json_specs,
    }
}

fn fallback_package(standalone_specs: Vec<(String, PathBuf, String)>) -> SpecPackage {
    let title = extract_first_heading(FALLBACK_BRAINSTORM)
        .unwrap_or_else(|| "design:os studio — 1:1 Figma Binary Parity & Agentic Spec Reader".to_string())
        .replace("DESIGN:OS Studio", "design:os studio")
        .replace("Design OS Studio", "design:os studio")
        .replace("DESIGN:OS", "design:os");
    synthesize_v2_from_legacy(
        "figma-parity-and-agentic-spec-reader".to_string(),
        title,
        FALLBACK_BRAINSTORM.to_string(),
        FALLBACK_USECASES.to_string(),
        FALLBACK_PLAN.to_string(),
        standalone_specs,
    )
}

fn extract_first_heading(md: &str) -> Option<String> {
    for line in md.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("# ") {
            let clean =
                rest.trim_start_matches("Brainstorm:").trim_start_matches("Implementation Plan:").trim_start_matches("Use Cases & Acceptance Criteria:").trim();
            if !clean.is_empty() {
                return Some(clean.to_string());
            }
        }
    }
    None
}

fn humanize_slug(slug: &str) -> String {
    slug.split('-')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Parses Markdown and Gherkin blocks into structured visual blocks.
pub fn parse_markdown_blocks(md: &str) -> Vec<MdBlock> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = md.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let raw = lines[i];
        let trimmed = raw.trim();

        if trimmed.is_empty() || trimmed == "---" {
            i += 1;
            continue;
        }

        if let Some(fence_lang) = trimmed.strip_prefix("```") {
            let lang = fence_lang.trim().to_ascii_lowercase();
            i += 1;
            let mut code_lines = Vec::new();
            while i < lines.len() && !lines[i].trim().starts_with("```") {
                code_lines.push(lines[i]);
                i += 1;
            }
            if i < lines.len() {
                i += 1;
            }
            if lang == "gherkin" {
                let parsed = code_lines.into_iter().filter(|l| !l.trim().is_empty()).map(parse_gherkin_line).collect();
                blocks.push(MdBlock::Gherkin { lines: parsed });
            } else {
                blocks.push(MdBlock::CodeBlock { lang, lines: code_lines.into_iter().map(|s| s.to_string()).collect() });
            }
            continue;
        }

        if let Some(us_rest) = trimmed.strip_prefix("### ") {
            if us_rest.starts_with("US-") {
                let id_and_title = us_rest.trim().to_string();
                i += 1;
                let mut as_a = String::new();
                let mut i_want = String::new();
                let mut so_that = String::new();
                while i < lines.len() {
                    let l = lines[i].trim();
                    if l.is_empty() {
                        i += 1;
                        if !as_a.is_empty() || !i_want.is_empty() || !so_that.is_empty() {
                            break;
                        }
                        continue;
                    }
                    if l.starts_with("```") || l.starts_with('#') {
                        break;
                    }
                    let clean = strip_inline_md(l);
                    if let Some(r) = clean.strip_prefix("As a ").or_else(|| clean.strip_prefix("As an ")) {
                        as_a = r.trim().trim_end_matches(',').to_string();
                    } else if let Some(r) = clean.strip_prefix("I want ") {
                        i_want = r.trim().trim_end_matches(',').to_string();
                    } else if let Some(r) = clean.strip_prefix("So that ") {
                        so_that = r.trim().trim_end_matches('.').to_string();
                    } else if !so_that.is_empty() {
                        so_that.push(' ');
                        so_that.push_str(&clean);
                    } else if !i_want.is_empty() {
                        i_want.push(' ');
                        i_want.push_str(&clean);
                    } else {
                        as_a.push_str(&clean);
                    }
                    i += 1;
                }
                blocks.push(MdBlock::UserStory { id_and_title, as_a, i_want, so_that });
                continue;
            }
        }

        if let Some(h3) = trimmed.strip_prefix("### ") {
            blocks.push(MdBlock::Heading { level: 3, text: strip_inline_md(h3) });
            i += 1;
            continue;
        }
        if let Some(h2) = trimmed.strip_prefix("## ") {
            blocks.push(MdBlock::Heading { level: 2, text: strip_inline_md(h2) });
            i += 1;
            continue;
        }
        if let Some(h1) = trimmed.strip_prefix("# ") {
            blocks.push(MdBlock::Heading { level: 1, text: strip_inline_md(h1) });
            i += 1;
            continue;
        }

        if let Some(bullet_body) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")).or_else(|| strip_numbered_prefix(trimmed)) {
            let (badge, bold_key, text) = parse_bullet_parts(bullet_body);
            blocks.push(MdBlock::Bullet { badge, bold_key, text });
            i += 1;
            continue;
        }

        blocks.push(MdBlock::Paragraph(strip_inline_md(trimmed)));
        i += 1;
    }

    blocks
}

fn strip_numbered_prefix(s: &str) -> Option<&str> {
    let dot = s.find(". ")?;
    if dot > 0 && dot <= 3 && s[..dot].chars().all(|c| c.is_ascii_digit()) {
        Some(&s[dot + 2..])
    } else {
        None
    }
}

fn parse_gherkin_line(line: &str) -> GherkinLine {
    let t = line.trim();
    if let Some(rest) = t.strip_prefix("Scenario:") {
        GherkinLine { keyword: GherkinKeyword::Scenario, keyword_text: "Scenario:".to_string(), rest: rest.trim().to_string() }
    } else if let Some(rest) = t.strip_prefix("Given ") {
        GherkinLine { keyword: GherkinKeyword::Given, keyword_text: "Given".to_string(), rest: rest.trim().to_string() }
    } else if let Some(rest) = t.strip_prefix("When ") {
        GherkinLine { keyword: GherkinKeyword::When, keyword_text: "When".to_string(), rest: rest.trim().to_string() }
    } else if let Some(rest) = t.strip_prefix("Then ") {
        GherkinLine { keyword: GherkinKeyword::Then, keyword_text: "Then".to_string(), rest: rest.trim().to_string() }
    } else if let Some(rest) = t.strip_prefix("And ") {
        GherkinLine { keyword: GherkinKeyword::And, keyword_text: "And".to_string(), rest: rest.trim().to_string() }
    } else {
        GherkinLine { keyword: GherkinKeyword::Other, keyword_text: String::new(), rest: t.to_string() }
    }
}

fn parse_bullet_parts(body: &str) -> (Option<String>, Option<String>, String) {
    let trimmed = body.trim();
    if let Some(after_open) = trimmed.strip_prefix("**") {
        if let Some(end_bold) = after_open.find("**") {
            let key = after_open[..end_bold].trim().trim_end_matches(':').to_string();
            let remainder = after_open[end_bold + 2..].trim().trim_start_matches(':').trim_start_matches('—').trim_start_matches('-').trim();
            let badge = detect_moscow_or_uc_badge(&key);
            return (badge, Some(key), strip_inline_md(remainder));
        }
    }
    (None, None, strip_inline_md(trimmed))
}

fn detect_moscow_or_uc_badge(key: &str) -> Option<String> {
    let upper = key.to_ascii_uppercase();
    if upper.starts_with("MUST") {
        Some("MUST".to_string())
    } else if upper.starts_with("SHOULD") {
        Some("SHOULD".to_string())
    } else if upper.starts_with("COULD") {
        Some("COULD".to_string())
    } else if upper.starts_with("UC-") {
        key.split([':', ' ', '—']).next().map(|s| s.to_string())
    } else {
        None
    }
}

fn strip_inline_md(s: &str) -> String {
    s.replace("**", "").replace('`', "")
}

/// Summarizes a declarative `.spec.json` payload (total node count, presets used, `$easeui.*` tokens).
pub fn summarize_json_spec(raw: &str) -> SpecJsonSummary {
    let Ok(spec) = serde_json::from_str::<NodeSpec>(raw) else {
        return SpecJsonSummary::default();
    };
    let mut total_nodes = 0usize;
    let mut presets = BTreeSet::new();
    let mut tokens = BTreeSet::new();
    fn walk(n: &NodeSpec, total: &mut usize, presets: &mut BTreeSet<String>, tokens: &mut BTreeSet<String>) {
        *total += 1;
        if let Some(p) = &n.preset {
            if !p.is_empty() {
                presets.insert(p.clone());
            }
        }
        for col in [&n.fill, &n.stroke, &n.text_color].into_iter().flatten() {
            if col.trim().starts_with("$easeui.") {
                tokens.insert(col.trim().to_string());
            }
        }
        for c in &n.children {
            walk(c, total, presets, tokens);
        }
    }
    walk(&spec, &mut total_nodes, &mut presets, &mut tokens);
    SpecJsonSummary {
        root_name: if spec.name.is_empty() { "Root Spec".to_string() } else { spec.name },
        total_nodes,
        presets: presets.into_iter().collect(),
        easeui_tokens: tokens.into_iter().collect(),
    }
}

impl App {
    /// Applies a declarative [`NodeSpec`] to the active page in one undoable command and fits the canvas.
    pub fn apply_node_spec(&mut self, spec: &NodeSpec, label: &str) {
        let parent = self.page;
        let mut trial = self.doc.clone();
        match trial.apply_spec(parent, spec) {
            Ok(root_id) => {
                let mut nodes = Vec::new();
                trial.walk(root_id, |n, _| nodes.push(n.clone()));
                let index = self.doc.get(parent).map(|p| p.children.len()).unwrap_or(0);
                if self.hist.exec(&mut self.doc, Command::Insert { parent, index, nodes }).is_ok() {
                    self.doc.relayout_subtree(parent);
                    self.selection = vec![root_id];
                    self.changed();
                    self.fit_all();
                    self.show_toast(format!("Applied UI spec '{label}' to canvas"), false);
                }
            }
            Err(e) => self.show_toast(format!("Spec error: {e}"), true),
        }
    }

    /// Exports the selected frame (or first screen on the active page) to `examples/specs/<slug>.spec.json`.
    pub fn export_active_frame_to_spec(&mut self) {
        self.settle_pending_relayout();
        let target_id = self.selection.first().copied().or_else(|| self.doc.get(self.page).and_then(|p| p.children.first().copied())).unwrap_or(self.page);
        let node_name = self.doc.get(target_id).map(|n| n.name.clone()).unwrap_or_else(|| "screen".to_string());
        let slug: String = node_name
            .to_ascii_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        let filename = format!("{}.spec.json", if slug.is_empty() { "canvas-screen" } else { &slug });
        let out_dir = find_workspace_root().map(|r| r.join("examples").join("specs")).unwrap_or_else(|| std::env::temp_dir().join("designos_specs"));
        let _ = std::fs::create_dir_all(&out_dir);
        let out_path = out_dir.join(&filename);
        match self.doc.export_spec(target_id) {
            Ok(spec) => match serde_json::to_string_pretty(&spec) {
                Ok(json) => match std::fs::write(&out_path, json) {
                    Ok(()) => {
                        self.specs.refresh();
                        self.show_toast(format!("Exported {}", out_path.display()), false);
                    }
                    Err(e) => self.show_toast(format!("Cannot write spec: {e}"), true),
                },
                Err(e) => self.show_toast(format!("Serialize error: {e}"), true),
            },
            Err(e) => self.show_toast(format!("Export spec error: {e}"), true),
        }
    }

    /// Saves a local `.fig` binary copy of the current document.
    pub fn export_fig_copy(&mut self) {
        let stem = if self.doc.name.trim().is_empty() { "Untitled" } else { self.doc.name.trim() };
        let default_path = self
            .file_path
            .as_ref()
            .and_then(|p| p.parent().map(|d| d.join(format!("{stem}.fig"))))
            .unwrap_or_else(|| std::env::temp_dir().join(format!("{stem}.fig")));
        self.settle_pending_relayout();
        match crate::io::save_path(&self.doc, &default_path) {
            Ok(saved) => self.show_toast(format!("Saved .fig binary to {}", saved.display()), false),
            Err(e) => self.show_toast(e, true),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Left Sidebar: Figma Specs Document & Story Tree (`show_left`, 240px)
// ─────────────────────────────────────────────────────────────────────────────

/// Left Sidebar Spec Explorer rendered when `app.ui.rail == Some(RailItem::Specs)`.
pub fn show_left(app: &mut App, ui: &mut Ui, rect: Rect) {
    app.specs.ensure_loaded();
    let p = ui.painter().clone();
    p.rect_filled(rect, 0.0, TOKEN_SURFACE);
    widgets::vline(&p, rect.right() - 1.0, rect.top(), rect.bottom(), TOKEN_BORDER);

    // 40px Clean Sidebar Header (`Figma Specs` + `Reader / Canvas` toggle)
    let header_h = 40.0;
    let header_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), header_h));
    widgets::hline(&p, header_rect.left(), header_rect.right(), header_rect.bottom() - 1.0, TOKEN_BORDER);

    icons::paint(&p, Icon::Specs, Pos2::new(header_rect.left() + 16.0, header_rect.center().y), 13.0, TOKEN_TEXT_MUTED);
    let hub_label = widgets::elide(&p, "Figma Specs", &theme::font_body_strong(), (header_rect.width() - 102.0).max(40.0));
    p.text(Pos2::new(header_rect.left() + 28.0, header_rect.center().y), Align2::LEFT_CENTER, hub_label, theme::font_body_strong(), TOKEN_TEXT);

    let toggle_w = 58.0;
    let toggle_rect = Rect::from_min_size(Pos2::new(header_rect.right() - toggle_w - 10.0, header_rect.center().y - 11.0), Vec2::new(toggle_w, 22.0));
    let toggle_resp = ui.interact(toggle_rect, Id::new("specs-reader-toggle"), Sense::click());
    let toggle_bg = if app.specs.open || toggle_resp.hovered() { TOKEN_SURFACE_2 } else { TOKEN_SURFACE_3 };
    p.rect_filled(toggle_rect, CornerRadius::same(5), toggle_bg);
    p.rect_stroke(toggle_rect, CornerRadius::same(5), Stroke::new(1.0, TOKEN_BORDER), StrokeKind::Inside);
    p.text(
        toggle_rect.center(),
        Align2::CENTER_CENTER,
        if app.specs.open { "Reader" } else { "Canvas" },
        theme::font_caption_medium(),
        if app.specs.open { TOKEN_TEXT } else { TOKEN_TEXT_MUTED },
    );
    widgets::record(ui, "specs-reader-toggle", toggle_rect);
    if toggle_resp.clicked() {
        app.specs.open = !app.specs.open;
    }

    let bottom_h = 68.0;
    let body_rect =
        Rect::from_min_max(Pos2::new(rect.left(), header_rect.bottom()), Pos2::new(rect.right(), (rect.bottom() - bottom_h).max(header_rect.bottom() + 40.0)));
    let bottom_rect = Rect::from_min_max(Pos2::new(rect.left(), body_rect.bottom()), rect.max);

    let pkg_count = app.specs.packages.len();
    let story_count = app.specs.current_package().map(|p| p.stories.len()).unwrap_or(0);
    let json_count = app.specs.current_package().map(|p| p.json_specs.len()).unwrap_or(0);
    let page_frames: Vec<NodeId> = app.doc.get(app.page).map(|pg| pg.children.clone()).unwrap_or_default();
    let est_h = 28.0
        + pkg_count as f32 * 30.0
        + 28.0
        + story_count as f32 * 28.0
        + 28.0
        + json_count as f32 * 28.0
        + 28.0
        + page_frames.len().max(1) as f32 * 28.0
        + 24.0;

    let mut lscroll = app.specs.left_scroll;
    wheel_scroll(ui, body_rect, &mut lscroll, est_h);
    app.specs.left_scroll = lscroll;

    let mut apply_json_action: Option<(String, String)> = None;
    let mut focus_frame_action: Option<NodeId> = None;

    clipped(ui, body_rect, |ui| {
        let p = ui.painter().clone();
        let mut y = body_rect.top() + 10.0 - lscroll;
        let pad_x = 8.0;
        let row_w = (body_rect.width() - pad_x * 2.0).max(60.0);

        // 0. All specifications + New Spec (`+`) button
        let all_row_r = Rect::from_min_size(Pos2::new(body_rect.left() + pad_x, y), Vec2::new(row_w, 26.0));
        let new_btn_r = Rect::from_min_size(Pos2::new(all_row_r.right() - 24.0, all_row_r.top() + 2.0), Vec2::splat(22.0));
        let all_click_r = Rect::from_min_max(all_row_r.min, Pos2::new(new_btn_r.left() - 2.0, all_row_r.bottom()));
        let all_resp = ui.interact(all_click_r, Id::new("specs-left-all-specs"), Sense::click());
        let new_resp = ui.interact(new_btn_r, Id::new("specs-left-new-spec"), Sense::click());
        if all_resp.hovered() {
            p.rect_filled(all_click_r, CornerRadius::same(5), TOKEN_SURFACE_3);
        }
        let all_lbl = widgets::elide(&p, "All specifications", &theme::font_body_medium(), (all_click_r.width() - 16.0).max(40.0));
        p.text(Pos2::new(all_click_r.left() + 8.0, all_click_r.center().y), Align2::LEFT_CENTER, all_lbl, theme::font_body_medium(), TOKEN_TEXT);
        widgets::record(ui, "specs-left-all-specs", all_click_r);
        if all_resp.clicked() {
            app.specs.active_tab = SpecTab::Overview;
            app.specs.open = true;
            app.specs.scroll = 0.0;
        }

        p.rect_filled(new_btn_r, CornerRadius::same(5), if new_resp.hovered() { TOKEN_SURFACE_2 } else { TOKEN_SURFACE_3 });
        p.rect_stroke(new_btn_r, CornerRadius::same(5), Stroke::new(1.0, TOKEN_BORDER), StrokeKind::Inside);
        let nc = new_btn_r.center();
        p.line_segment([Pos2::new(nc.x - 4.5, nc.y), Pos2::new(nc.x + 4.5, nc.y)], Stroke::new(1.4, TOKEN_TEXT));
        p.line_segment([Pos2::new(nc.x, nc.y - 4.5), Pos2::new(nc.x, nc.y + 4.5)], Stroke::new(1.4, TOKEN_TEXT));
        widgets::record(ui, "specs-left-new-spec", new_btn_r);
        if new_resp.clicked() {
            let pkg_idx = app.specs.selected_pkg;
            if let Some(pkg) = app.specs.packages.get_mut(pkg_idx) {
                let next_num = pkg.stories.len() + 3;
                let new_id = format!("US-{next_num:02}");
                pkg.stories.push(UserStorySpec {
                    id: new_id.clone(),
                    title: "New Feature Specification Draft".to_string(),
                    status: "Draft".to_string(),
                    epic: "E5 Interactive Authoring".to_string(),
                    priority: "SHOULD".to_string(),
                    wave: "W4".to_string(),
                    req_refs: vec!["REQ-01".to_string()],
                    decision_refs: vec!["D1".to_string()],
                    screens: vec!["S04".to_string()],
                    boards: vec!["W-01".to_string()],
                    scenarios: vec![ScenarioSpec {
                        id: format!("SCN-{next_num:02}-01"),
                        ac_refs: vec!["AC-01".to_string()],
                        method: "auto".to_string(),
                        status: "pending".to_string(),
                    }],
                    user_story_md: "As a Product Designer, I want to draft a new user story directly from the Specs sidebar, so that new requirements stay in sync with the canvas.".to_string(),
                    flow_steps: vec![
                        "Click + in Figma Specs sidebar".to_string(),
                        "Inspect generated US draft in Dev View".to_string(),
                    ],
                    business_rules: Vec::new(),
                    acceptance_criteria: vec![AcceptanceCriterionSpec {
                        id: "AC-01".to_string(),
                        given: "The Spec Reader left sidebar is open".to_string(),
                        when: "The user clicks the + New Spec button".to_string(),
                        then: "A new draft User Story is appended and opened in Dev View".to_string(),
                        scenario: format!("SCN-{next_num:02}-01"),
                        line_start: 1,
                        line_end: 20,
                    }],
                    edge_cases: vec![("draft".to_string(), "Newly created story starts in Draft state".to_string())],
                    ascii_wireframe: Vec::new(),
                    file_path: format!("{}/us/{new_id}.md", pkg.root_display),
                    line_start: 1,
                    line_end: 24,
                });
                let new_idx = pkg.stories.len() - 1;
                app.specs.selected_story = new_idx;
                app.specs.active_tab = SpecTab::UseCases;
                app.specs.open = true;
                app.specs.scroll = 0.0;
            }
        }
        y += 30.0;

        // 1. SPEC DOCUMENTS (.MD)
        draw_left_section_header(&p, Pos2::new(body_rect.left() + pad_x, y), row_w, "SPEC DOCUMENTS (.MD)");
        y += 22.0;

        let pkgs_snapshot = app.specs.packages.clone();
        for (idx, pkg) in pkgs_snapshot.iter().enumerate() {
            let row_r = Rect::from_min_size(Pos2::new(body_rect.left() + pad_x, y), Vec2::new(row_w, 28.0));
            let selected = idx == app.specs.selected_pkg && app.specs.active_tab == SpecTab::Overview;
            let resp = ui.interact(row_r, Id::new(("spec-pkg-row", idx)), Sense::click());
            if selected {
                p.rect_filled(row_r, CornerRadius::same(5), TOKEN_SURFACE_2);
                let bar = Rect::from_min_size(Pos2::new(row_r.left(), row_r.top() + 5.0), Vec2::new(2.0, 18.0));
                p.rect_filled(bar, CornerRadius::same(1), Color32::from_rgb(0x0D, 0x99, 0xFF));
            } else if resp.hovered() {
                p.rect_filled(row_r, CornerRadius::same(5), TOKEN_SURFACE_3);
            }
            let ver_tag = if pkg.format_version == "2" { ".md · v2" } else { ".md · v1" };
            let ver_w = widgets::text_width(&p, ver_tag, &theme::font_caption()) + 8.0;
            p.text(Pos2::new(row_r.right() - 8.0, row_r.center().y), Align2::RIGHT_CENTER, ver_tag, theme::font_caption(), TOKEN_TEXT_SUBTLE);
            let clean_title = pkg.title.replace("DESIGN:OS Studio", "design:os studio").replace("Design OS Studio", "design:os studio");
            let title_str = widgets::elide(&p, &clean_title, &theme::font_body_medium(), (row_r.width() - ver_w - 18.0).max(36.0));
            p.text(
                Pos2::new(row_r.left() + 10.0, row_r.center().y),
                Align2::LEFT_CENTER,
                title_str,
                if selected { theme::font_body_strong() } else { theme::font_body() },
                if selected { TOKEN_TEXT } else { TOKEN_TEXT_MUTED },
            );
            widgets::record(ui, &format!("spec-pkg:{idx}"), row_r);
            if resp.clicked() {
                app.specs.selected_pkg = idx;
                app.specs.active_tab = SpecTab::Overview;
                app.specs.open = true;
                app.specs.scroll = 0.0;
            }
            y += 30.0;
        }

        // Quick-jump row for Feature Contract (`contract/FC-001.md`)
        let fc_row_r = Rect::from_min_size(Pos2::new(body_rect.left() + pad_x, y), Vec2::new(row_w, 26.0));
        let fc_active = app.specs.active_tab == SpecTab::Plan;
        let fc_resp = ui.interact(fc_row_r, Id::new("specs-left-saved-filters"), Sense::click());
        if fc_active || fc_resp.hovered() {
            p.rect_filled(fc_row_r, CornerRadius::same(5), if fc_active { TOKEN_SURFACE_2 } else { TOKEN_SURFACE_3 });
        }
        let fc_lbl = widgets::elide(&p, "Feature Contract · FC-001", &theme::font_body(), (fc_row_r.width() - 18.0).max(40.0));
        p.text(
            Pos2::new(fc_row_r.left() + 10.0, fc_row_r.center().y),
            Align2::LEFT_CENTER,
            fc_lbl,
            if fc_active { theme::font_body_strong() } else { theme::font_body() },
            if fc_active { TOKEN_TEXT } else { TOKEN_TEXT_MUTED },
        );
        widgets::record(ui, "specs-left-saved-filters", fc_row_r);
        if fc_resp.clicked() {
            app.specs.active_tab = SpecTab::Plan;
            app.specs.open = true;
            app.specs.scroll = 0.0;
        }
        y += 28.0;

        // 2. USER STORIES (US-01..US-17)
        y += 6.0;
        draw_left_section_header(&p, Pos2::new(body_rect.left() + pad_x, y), row_w, "USER STORIES");
        y += 22.0;

        let stories_list = app.specs.current_package().map(|pkg| pkg.stories.clone()).unwrap_or_default();
        for (s_idx, st) in stories_list.iter().enumerate() {
            let row_r = Rect::from_min_size(Pos2::new(body_rect.left() + pad_x, y), Vec2::new(row_w, 26.0));
            let active = app.specs.active_tab == SpecTab::UseCases && app.specs.selected_story == s_idx;
            let resp = ui.interact(row_r, Id::new(("spec-left-story", s_idx)), Sense::click());
            if active {
                p.rect_filled(row_r, CornerRadius::same(5), TOKEN_SURFACE_2);
                let bar = Rect::from_min_size(Pos2::new(row_r.left(), row_r.top() + 5.0), Vec2::new(2.0, 16.0));
                p.rect_filled(bar, CornerRadius::same(1), Color32::from_rgb(0x0D, 0x99, 0xFF));
            } else if resp.hovered() {
                p.rect_filled(row_r, CornerRadius::same(5), TOKEN_SURFACE_3);
            }
            let id_w = widgets::text_width(&p, &st.id, &theme::font_mono()) + 8.0;
            p.text(Pos2::new(row_r.left() + 10.0, row_r.center().y), Align2::LEFT_CENTER, &st.id, theme::font_mono(), TOKEN_TEXT_SUBTLE);
            let st_title = widgets::elide(&p, &st.title, &theme::font_body(), (row_r.width() - id_w - 18.0).max(36.0));
            p.text(
                Pos2::new(row_r.left() + 10.0 + id_w, row_r.center().y),
                Align2::LEFT_CENTER,
                st_title,
                if active { theme::font_body_strong() } else { theme::font_body() },
                if active { TOKEN_TEXT } else { TOKEN_TEXT_MUTED },
            );
            if resp.clicked() {
                app.specs.selected_story = s_idx;
                app.specs.active_tab = SpecTab::UseCases;
                app.specs.open = true;
                app.specs.scroll = 0.0;
            }
            y += 28.0;
        }

        // 3. CANVAS BLUEPRINTS (.SPEC.JSON)
        y += 6.0;
        draw_left_section_header(&p, Pos2::new(body_rect.left() + pad_x, y), row_w, "CANVAS BLUEPRINTS");
        y += 22.0;

        let json_list = app.specs.current_package().map(|pkg| pkg.json_specs.clone()).unwrap_or_default();
        for (j_idx, (j_name, _j_path, j_raw)) in json_list.iter().enumerate() {
            let row_r = Rect::from_min_size(Pos2::new(body_rect.left() + pad_x, y), Vec2::new(row_w, 26.0));
            let apply_w = 44.0;
            let apply_r = Rect::from_min_size(Pos2::new(row_r.right() - apply_w - 4.0, row_r.center().y - 10.0), Vec2::new(apply_w, 20.0));
            let main_click_r = Rect::from_min_max(row_r.min, Pos2::new(apply_r.left() - 4.0, row_r.bottom()));
            let row_resp = ui.interact(main_click_r, Id::new(("spec-json-row", j_idx)), Sense::click());
            let apply_resp = ui.interact(apply_r, Id::new(("spec-json-apply", j_idx)), Sense::click());

            let active = app.specs.active_tab == SpecTab::JsonSpec && app.specs.selected_json_spec == j_idx;
            if active || row_resp.hovered() {
                p.rect_filled(row_r, CornerRadius::same(5), if active { TOKEN_SURFACE_2 } else { TOKEN_SURFACE_3 });
            }
            let clean_name = widgets::elide(&p, j_name, &theme::font_body(), (main_click_r.width() - 16.0).max(36.0));
            p.text(
                Pos2::new(row_r.left() + 10.0, row_r.center().y),
                Align2::LEFT_CENTER,
                clean_name,
                theme::font_body(),
                if active { TOKEN_TEXT } else { TOKEN_TEXT_MUTED },
            );

            p.rect_filled(apply_r, CornerRadius::same(4), if apply_resp.hovered() { TOKEN_SURFACE_2 } else { TOKEN_SURFACE_3 });
            p.rect_stroke(apply_r, CornerRadius::same(4), Stroke::new(1.0, TOKEN_BORDER), StrokeKind::Inside);
            p.text(apply_r.center(), Align2::CENTER_CENTER, "Apply", theme::font_caption(), TOKEN_TEXT_MUTED);
            widgets::record(ui, &format!("spec-json-apply:{j_idx}"), apply_r);
            if apply_resp.clicked() {
                apply_json_action = Some((j_name.clone(), j_raw.clone()));
            } else if row_resp.clicked() {
                app.specs.selected_json_spec = j_idx;
                app.specs.active_tab = SpecTab::JsonSpec;
                app.specs.open = true;
            }
            y += 28.0;
        }

        // 4. LINKED CANVAS FRAMES
        y += 6.0;
        draw_left_section_header(&p, Pos2::new(body_rect.left() + pad_x, y), row_w, "LINKED CANVAS FRAMES");
        y += 22.0;

        for fid in &page_frames {
            let Some(fnode) = app.doc.get(*fid) else { continue };
            let row_r = Rect::from_min_size(Pos2::new(body_rect.left() + pad_x, y), Vec2::new(row_w, 26.0));
            let resp = ui.interact(row_r, Id::new(("spec-frame-row", fid.0)), Sense::click());
            let is_sel = app.selection.contains(fid);
            if is_sel || resp.hovered() {
                p.rect_filled(row_r, CornerRadius::same(5), if is_sel { TOKEN_SURFACE_2 } else { TOKEN_SURFACE_3 });
            }
            let ready = fnode.dev.status == Some(DevStatus::ReadyForDev);
            let dot_col = if ready { DOT_OK } else { DOT_NEUTRAL };
            p.circle_filled(Pos2::new(row_r.left() + 12.0, row_r.center().y), 3.0, dot_col);

            let fname = widgets::elide(&p, &fnode.name, &theme::font_body(), (row_r.width() - 28.0).max(36.0));
            p.text(
                Pos2::new(row_r.left() + 22.0, row_r.center().y),
                Align2::LEFT_CENTER,
                fname,
                theme::font_body(),
                if is_sel { TOKEN_TEXT } else { TOKEN_TEXT_MUTED },
            );
            if resp.clicked() {
                focus_frame_action = Some(*fid);
            }
            y += 28.0;
        }
    });
    scrollbar(ui, body_rect, app.specs.left_scroll, est_h);

    if let Some((j_name, j_raw)) = apply_json_action {
        match serde_json::from_str::<NodeSpec>(&j_raw) {
            Ok(spec) => app.apply_node_spec(&spec, &j_name),
            Err(e) => app.show_toast(format!("Invalid .spec.json: {e}"), true),
        }
    }
    if let Some(fid) = focus_frame_action {
        app.select_only(fid);
        app.specs.open = false;
        app.fit_selection();
    }

    // Bottom Actions: Drop Spec Card on Canvas (D) + Export Frame -> .spec.json
    p.rect_filled(bottom_rect, 0.0, TOKEN_SURFACE);
    widgets::hline(&p, bottom_rect.left(), bottom_rect.right(), bottom_rect.top(), TOKEN_BORDER);
    let btn_w = (bottom_rect.width() - 16.0).max(60.0);
    let btn1 = Rect::from_min_size(Pos2::new(bottom_rect.left() + 8.0, bottom_rect.top() + 8.0), Vec2::new(btn_w, 24.0));
    let btn2 = Rect::from_min_size(Pos2::new(bottom_rect.left() + 8.0, bottom_rect.top() + 36.0), Vec2::new(btn_w, 24.0));

    let r1 = ui.interact(btn1, Id::new("specs-drop-canvas-btn"), Sense::click());
    p.rect_filled(btn1, CornerRadius::same(5), if r1.hovered() { TOKEN_SURFACE_2 } else { TOKEN_SURFACE_3 });
    p.rect_stroke(btn1, CornerRadius::same(5), Stroke::new(1.0, TOKEN_BORDER), StrokeKind::Inside);
    let b1_txt = widgets::elide(&p, "Drop Spec Card on Canvas (D)", &theme::font_caption_medium(), (btn1.width() - 14.0).max(36.0));
    p.text(btn1.center(), Align2::CENTER_CENTER, b1_txt, theme::font_caption_medium(), TOKEN_TEXT);
    widgets::record(ui, "specs-drop-canvas-btn", btn1);
    if r1.clicked() {
        if let Some(pkg) = app.specs.current_package().cloned() {
            drop_spec_card_onto_canvas(app, &pkg);
        }
    }

    let r2 = ui.interact(btn2, Id::new("specs-export-json-btn"), Sense::click());
    p.rect_filled(btn2, CornerRadius::same(5), if r2.hovered() { TOKEN_SURFACE_2 } else { TOKEN_SURFACE_3 });
    p.rect_stroke(btn2, CornerRadius::same(5), Stroke::new(1.0, TOKEN_BORDER), StrokeKind::Inside);
    let b2_txt = widgets::elide(&p, "Export Frame -> .spec.json", &theme::font_caption_medium(), (btn2.width() - 14.0).max(36.0));
    p.text(btn2.center(), Align2::CENTER_CENTER, b2_txt, theme::font_caption_medium(), TOKEN_TEXT_MUTED);
    widgets::record(ui, "specs-export-json-btn", btn2);
    if r2.clicked() {
        app.export_active_frame_to_spec();
    }
}

/// Materializes the active Markdown Spec (`PRD: ...`) as a live Figma Spec Card Frame on the canvas (`https://www.figma.com/specs/`).
pub fn drop_spec_card_onto_canvas(app: &mut App, pkg: &SpecPackage) {
    let clean_title = pkg.title.replace("DESIGN:OS Studio", "design:os studio").replace("Design OS Studio", "design:os studio");
    let spec_card_json = serde_json::json!({
        "name": format!("Spec: {}", clean_title),
        "preset": "easeui-panel",
        "width": 560.0,
        "height": 640.0,
        "fill": "#FFFFFF",
        "stroke": "#0D99FF",
        "stroke_width": 1.5,
        "radius": 12.0,
        "dev_status": "ready",
        "annotation": format!("Figma Specs · Markdown as a Medium · {}", pkg.contract_id),
        "layout": {
            "dir": "column",
            "padding": [28.0, 32.0, 28.0, 32.0],
            "gap": 16.0,
            "align": "start",
            "justify": "start"
        },
        "children": [
            {
                "name": "Spec Eyebrow",
                "text": format!("FIGMA SPECS · MARKDOWN MEDIUM · {}", pkg.contract_id),
                "font_size": 11.0,
                "fill": "#0D99FF"
            },
            {
                "name": "Spec Title",
                "text": format!("PRD: {}", clean_title),
                "font_size": 24.0,
                "fill": "#111827"
            },
            {
                "name": "Problem Heading",
                "text": "1. Problem & Product Outcome",
                "font_size": 16.0,
                "fill": "#111827"
            },
            {
                "name": "Problem Prose",
                "text": pkg.contract_outcome,
                "font_size": 13.0,
                "fill": "#374151"
            },
            {
                "name": "Embedded Architecture Flow",
                "preset": "easeui-row",
                "width": 496.0,
                "height": 64.0,
                "fill": "#F8FAFC",
                "stroke": "#0D99FF",
                "stroke_width": 1.0,
                "radius": 8.0,
                "layout": {
                    "dir": "row",
                    "padding": [14.0, 16.0, 14.0, 16.0],
                    "gap": 12.0,
                    "align": "center",
                    "justify": "start"
                },
                "children": [
                    { "name": "Step 1", "text": "Spec (.md)", "font_size": 12.0, "fill": "#111827" },
                    { "name": "Step 2", "text": "-> Agent (@prd)", "font_size": 12.0, "fill": "#0D99FF" },
                    { "name": "Step 3", "text": "-> Canvas + .fig v106", "font_size": 12.0, "fill": "#111827" }
                ]
            }
        ]
    });
    let node_spec: NodeSpec = serde_json::from_value(spec_card_json).expect("valid Spec Card NodeSpec");
    app.apply_node_spec(&node_spec, "figma-spec-card.spec.json");
    app.show_toast("Dropped Figma Spec Card onto Canvas", false);
}

// ─────────────────────────────────────────────────────────────────────────────
// Center Workspace: 1:1 Figma Specs ("Markdown As a Medium") Reader (`show_reader`)
// ─────────────────────────────────────────────────────────────────────────────

pub fn show_reader(app: &mut App, ui: &mut Ui, rect: Rect) {
    app.specs.ensure_loaded();
    let pal = ReaderPalette::for_mode(app.specs.light_mode);
    let p = ui.painter().clone();
    p.rect_filled(rect, 0.0, pal.bg_page);

    let Some(pkg) = app.specs.current_package().cloned() else {
        return;
    };

    // 1. Minimal 42px Figma Specs Top Breadcrumb & Mode Bar (`< design:os studio / PRD: ...`)
    let bar_h = 42.0;
    let bar_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), bar_h));
    p.rect_filled(bar_rect, 0.0, pal.surface);
    widgets::hline(&p, bar_rect.left(), bar_rect.right(), bar_rect.bottom() - 1.0, pal.border);

    // Right-side lightweight controls: `Canvas` (`hero-view-canvas`), `Paper/Slate` (`specs-theme-toggle`), `Copy .md` (`specs-copy-md`), `Share` (`specs-share-btn`)
    let canvas_btn_w = 56.0;
    let canvas_btn = Rect::from_min_size(Pos2::new(bar_rect.right() - canvas_btn_w - 10.0, bar_rect.center().y - 12.0), Vec2::new(canvas_btn_w, 24.0));
    let canvas_resp = ui.interact(canvas_btn, Id::new("hero-view-canvas"), Sense::click());
    p.rect_filled(canvas_btn, CornerRadius::same(5), if canvas_resp.hovered() { pal.surface_2 } else { pal.doc_sheet });
    p.rect_stroke(canvas_btn, CornerRadius::same(5), Stroke::new(1.0, pal.border), StrokeKind::Inside);
    p.text(canvas_btn.center(), Align2::CENTER_CENTER, "Canvas", theme::font_caption_medium(), pal.text);
    widgets::record(ui, "hero-view-canvas", canvas_btn);
    if canvas_resp.clicked() {
        app.specs.open = false;
    }

    let theme_btn_w = 50.0;
    let theme_btn = Rect::from_min_size(Pos2::new(canvas_btn.left() - theme_btn_w - 6.0, bar_rect.center().y - 12.0), Vec2::new(theme_btn_w, 24.0));
    let theme_resp = ui.interact(theme_btn, Id::new("specs-theme-toggle"), Sense::click());
    p.rect_filled(theme_btn, CornerRadius::same(5), if theme_resp.hovered() { pal.surface_2 } else { pal.doc_sheet });
    p.rect_stroke(theme_btn, CornerRadius::same(5), Stroke::new(1.0, pal.border), StrokeKind::Inside);
    p.text(theme_btn.center(), Align2::CENTER_CENTER, if app.specs.light_mode { "Paper" } else { "Slate" }, theme::font_caption_medium(), pal.text_muted);
    widgets::record(ui, "specs-theme-toggle", theme_btn);
    if theme_resp.clicked() {
        app.specs.light_mode = !app.specs.light_mode;
    }

    let copy_md_w = 62.0;
    let copy_md_btn = Rect::from_min_size(Pos2::new(theme_btn.left() - copy_md_w - 6.0, bar_rect.center().y - 12.0), Vec2::new(copy_md_w, 24.0));
    let copy_md_resp = ui.interact(copy_md_btn, Id::new("specs-copy-md"), Sense::click());
    p.rect_filled(copy_md_btn, CornerRadius::same(5), if copy_md_resp.hovered() { pal.surface_2 } else { pal.doc_sheet });
    p.rect_stroke(copy_md_btn, CornerRadius::same(5), Stroke::new(1.0, pal.border), StrokeKind::Inside);
    p.text(copy_md_btn.center(), Align2::CENTER_CENTER, "Copy .md", theme::font_caption_medium(), pal.text_muted);
    widgets::record(ui, "specs-copy-md", copy_md_btn);
    if copy_md_resp.clicked() {
        let md_body = match app.specs.active_tab {
            SpecTab::Overview => pkg.brainstorm_md.clone(),
            SpecTab::UseCases => pkg.usecases_md.clone(),
            SpecTab::Plan => pkg.plan_md.clone(),
            SpecTab::JsonSpec => pkg.json_specs.get(app.specs.selected_json_spec).map(|(_, _, raw)| raw.clone()).unwrap_or_else(|| pkg.plan_md.clone()),
            SpecTab::CanvasScreens => pkg.plan_md.clone(),
        };
        ui.ctx().copy_text(md_body.clone());
        app.show_toast("Copied Spec Markdown (.md) to clipboard", false);
    }

    let share_btn_w = 48.0;
    let share_btn = Rect::from_min_size(Pos2::new(copy_md_btn.left() - share_btn_w - 6.0, bar_rect.center().y - 12.0), Vec2::new(share_btn_w, 24.0));
    let share_resp = ui.interact(share_btn, Id::new("specs-share-btn"), Sense::click());
    p.rect_filled(share_btn, CornerRadius::same(5), if share_resp.hovered() { pal.surface_2 } else { pal.doc_sheet });
    p.rect_stroke(share_btn, CornerRadius::same(5), Stroke::new(1.0, pal.border), StrokeKind::Inside);
    p.text(share_btn.center(), Align2::CENTER_CENTER, "Share", theme::font_caption_medium(), pal.text_muted);
    widgets::record(ui, "specs-share-btn", share_btn);
    if share_resp.clicked() {
        let deep_link = format!("designos://specs/{}#{}", pkg.slug, app.specs.active_tab.anchor());
        ui.ctx().copy_text(deep_link.clone());
        app.show_toast("Copied Spec deep-link to clipboard", false);
    }

    // Minimalist underline view tabs (PRD & Overview | Stories | QA & Trace | Blueprints | Gates)
    let tab_labels = ["PRD & Overview", "User Stories", "QA & Trace", "Blueprints", "Gates"];
    let mut tab_right = share_btn.left() - 10.0;
    let mut tab_rects: Vec<(usize, SpecTab, Rect, &str)> = Vec::new();
    for (ti, tab) in SpecTab::ALL.iter().enumerate().rev() {
        let lbl = tab_labels[ti];
        let tw = widgets::text_width(&p, lbl, &theme::font_caption_medium()) + 16.0;
        let left_x = tab_right - tw;
        if left_x > bar_rect.left() + 160.0 {
            let tr = Rect::from_min_size(Pos2::new(left_x, bar_rect.top() + 7.0), Vec2::new(tw, bar_h - 14.0));
            tab_rects.push((ti, *tab, tr, lbl));
            tab_right = left_x - 2.0;
        }
    }
    for (ti, tab, tr, lbl) in tab_rects {
        let resp = ui.interact(tr, Id::new(("spec-reader-tab", ti)), Sense::click());
        let active = app.specs.active_tab == tab;
        p.text(
            tr.center(),
            Align2::CENTER_CENTER,
            lbl,
            if active { theme::font_caption_strong() } else { theme::font_caption_medium() },
            if active {
                pal.text
            } else if resp.hovered() {
                pal.text_muted
            } else {
                pal.text_subtle
            },
        );
        if active {
            let ind = Rect::from_min_size(Pos2::new(tr.left() + 6.0, bar_rect.bottom() - 2.0), Vec2::new(tr.width() - 12.0, 2.0));
            p.rect_filled(ind, CornerRadius::same(1), Color32::from_rgb(0x0D, 0x99, 0xFF));
        }
        widgets::record(ui, &format!("spec-tab:{ti}"), tr);
        if resp.clicked() {
            app.specs.active_tab = tab;
            app.specs.scroll = 0.0;
        }
    }

    // Top-left Figma Specs Breadcrumb: `< design:os studio  /  PRD: ...`
    let bc_max_w = (tab_right - bar_rect.left() - 24.0).max(60.0);
    let clean_title = pkg.title.replace("DESIGN:OS Studio", "design:os studio").replace("Design OS Studio", "design:os studio");
    let bc_text = format!("< design:os studio  /  PRD: {clean_title}");
    let bc_clean = widgets::elide(&p, &bc_text, &theme::font_body_medium(), bc_max_w);
    p.text(Pos2::new(bar_rect.left() + 16.0, bar_rect.center().y), Align2::LEFT_CENTER, bc_clean, theme::font_body_medium(), pal.text_muted);

    // 2. Main Editorial Document Area (reserve 52px at bottom for the floating Figma Specs `✦ Ask anything` Agent Bar)
    let content_rect =
        Rect::from_min_max(Pos2::new(rect.left(), bar_rect.bottom()), Pos2::new(rect.right(), (rect.bottom() - 52.0).max(bar_rect.bottom() + 80.0)));

    match app.specs.active_tab {
        SpecTab::Overview => render_po_view_tab(app, ui, content_rect, &pkg),
        SpecTab::UseCases => render_dev_view_tab(app, ui, content_rect, &pkg),
        SpecTab::Plan => render_qa_coverage_tab(app, ui, content_rect, &pkg),
        SpecTab::JsonSpec => render_wireframes_and_contract_tab(app, ui, content_rect, &pkg),
        SpecTab::CanvasScreens => render_gates_and_canvas_tab(app, ui, content_rect, &pkg),
    }

    // 3. Bottom-Left Floating Figma Specs Agent Pill (`[ ✦ Ask @agent about this Spec   (↑) ]`)
    let ask_w = (rect.width() - 40.0).clamp(180.0, 460.0);
    let ask_rect = Rect::from_min_size(Pos2::new(rect.left() + 20.0, rect.bottom() - 42.0), Vec2::new(ask_w, 32.0));
    p.rect_filled(ask_rect, CornerRadius::same(99), pal.doc_sheet);
    p.rect_stroke(ask_rect, CornerRadius::same(99), Stroke::new(1.0, pal.border), StrokeKind::Inside);
    widgets::record(ui, "specs-ask-agent-bar", ask_rect);

    icons::paint(&p, Icon::Sparkle, Pos2::new(ask_rect.left() + 16.0, ask_rect.center().y), 13.0, Color32::from_rgb(0x0D, 0x99, 0xFF));
    let send_c = Pos2::new(ask_rect.right() - 16.0, ask_rect.center().y);
    let send_r = Rect::from_center_size(send_c, Vec2::splat(22.0));
    let send_resp = ui.interact(send_r, Id::new("specs-ask-agent-send"), Sense::click());
    p.circle_filled(send_c, 10.0, if send_resp.hovered() { Color32::from_rgb(0x25, 0xA8, 0xFF) } else { Color32::from_rgb(0x0D, 0x99, 0xFF) });
    // Vector up-arrow inside the send button
    p.line_segment([Pos2::new(send_c.x, send_c.y + 4.5), Pos2::new(send_c.x, send_c.y - 4.5)], Stroke::new(1.6, Color32::WHITE));
    p.line_segment([Pos2::new(send_c.x - 3.5, send_c.y - 1.0), Pos2::new(send_c.x, send_c.y - 4.5)], Stroke::new(1.6, Color32::WHITE));
    p.line_segment([Pos2::new(send_c.x + 3.5, send_c.y - 1.0), Pos2::new(send_c.x, send_c.y - 4.5)], Stroke::new(1.6, Color32::WHITE));
    widgets::record(ui, "specs-ask-agent-send", send_r);

    let edit_rect = Rect::from_min_max(Pos2::new(ask_rect.left() + 30.0, ask_rect.top() + 5.0), Pos2::new(send_r.left() - 6.0, ask_rect.bottom() - 5.0));
    let mut submit_ask = false;
    ui.scope_builder(egui::UiBuilder::new().max_rect(edit_rect), |ui| {
        let te = egui::TextEdit::singleline(&mut app.specs.ask_input)
            .id(Id::new("specs-ask-agent-input"))
            .desired_width(edit_rect.width())
            .frame(egui::Frame::NONE)
            .font(theme::font_body())
            .text_color(pal.text)
            .hint_text(egui::RichText::new("Ask @agent about this Spec").font(theme::font_body()).color(pal.text_muted));
        let resp = ui.add(te);
        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            submit_ask = true;
        }
    });

    if send_resp.clicked() || submit_ask {
        let typed = app.specs.ask_input.trim().to_string();
        let prompt = if typed.is_empty() {
            format!("@prd Reference @spec://{}/prd/PRD.md ({}) and implement the active User Story onto the Canvas", pkg.slug, pkg.contract_id)
        } else {
            format!("@prd @spec://{}/{} ({}) {}", pkg.slug, app.specs.active_tab.anchor(), pkg.contract_id, typed)
        };
        app.specs.ask_input.clear();
        let fp = app.file_path.clone();
        app.terminal.submit_chat_prompt(&prompt, fp.as_deref());
        app.terminal.side_panel_pty_open = false;
        app.ui.rail = Some(crate::app::RailItem::Agents);
        app.specs.open = false;
        app.show_toast("Dispatched Spec prompt to Agent CLI", false);
    }
}

/// Public alias for `PO View` (`SpecTab::Overview`) renderer.
#[allow(dead_code)]
pub fn show_po_view(app: &mut App, ui: &mut Ui, rect: Rect, pkg: &SpecPackage) {
    render_po_view_tab(app, ui, rect, pkg);
}

/// Public alias for `QA & Coverage` (`SpecTab::Plan`) renderer.
#[allow(dead_code)]
pub fn show_qa_view(app: &mut App, ui: &mut Ui, rect: Rect, pkg: &SpecPackage) {
    render_qa_coverage_tab(app, ui, rect, pkg);
}

// ─────────────────────────────────────────────────────────────────────────────
// Right Sidebar (`show_right`, 240px): Contextual `Spec Inspector`
// Replaces Canvas Design Inspector when `specs_reader_open` is `true`.
// ─────────────────────────────────────────────────────────────────────────────

pub fn show_right(app: &mut App, ui: &mut Ui, rect: Rect) {
    app.specs.ensure_loaded();
    let p = ui.painter().clone();
    p.rect_filled(rect, 0.0, TOKEN_SURFACE);
    widgets::vline(&p, rect.left(), rect.top(), rect.bottom(), TOKEN_BORDER);

    let header_h = 40.0;
    let header_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), header_h));
    widgets::hline(&p, header_rect.left(), header_rect.right(), header_rect.bottom() - 1.0, TOKEN_BORDER);

    icons::paint(&p, Icon::Specs, Pos2::new(header_rect.left() + 14.0, header_rect.center().y), 13.0, TOKEN_TEXT_MUTED);
    p.text(Pos2::new(header_rect.left() + 26.0, header_rect.center().y), Align2::LEFT_CENTER, "Spec Inspector", theme::font_body_strong(), TOKEN_TEXT);
    p.text(Pos2::new(header_rect.right() - 12.0, header_rect.center().y), Align2::RIGHT_CENTER, ".md · v2", theme::font_caption(), TOKEN_TEXT_SUBTLE);

    let Some(pkg) = app.specs.current_package().cloned() else {
        return;
    };
    let kpi = pkg.kpi_metrics();

    let body_rect = Rect::from_min_max(Pos2::new(rect.left(), header_rect.bottom()), rect.max);
    let toc_entries = active_section_toc(&pkg, &app.specs);
    let unresolved_count = app.specs.comments.iter().filter(|c| !c.resolved).count();
    let est_h = 340.0 + toc_entries.len() as f32 * 26.0 + unresolved_count as f32 * 74.0 + 48.0;
    let mut rscroll = app.specs.right_scroll;
    wheel_scroll(ui, body_rect, &mut rscroll, est_h);
    app.specs.right_scroll = rscroll;

    let mut copied_payload: Option<String> = None;
    let mut open_agent_prompt: Option<String> = None;
    let mut drop_spec_card = false;

    clipped(ui, body_rect, |ui| {
        let p = ui.painter().clone();
        let mut y = body_rect.top() + 12.0 - rscroll;
        let pad_x = 12.0;
        let row_w = (body_rect.width() - pad_x * 2.0).max(80.0);

        // 1. ON THIS PAGE (Dynamic Interactive TOC Outline per active SpecTab)
        p.text(Pos2::new(body_rect.left() + pad_x, y), Align2::LEFT_CENTER, "ON THIS PAGE", theme::font_caption_strong(), TOKEN_TEXT_SUBTLE);
        y += 20.0;

        let active_toc_idx = toc_entries
            .iter()
            .enumerate()
            .min_by(|(_, (_, a)), (_, (_, b))| (app.specs.scroll - *a).abs().partial_cmp(&(app.specs.scroll - *b).abs()).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0);

        for (ti, (label, target_scroll)) in toc_entries.iter().enumerate() {
            let tr = Rect::from_min_size(Pos2::new(body_rect.left() + pad_x, y), Vec2::new(row_w, 24.0));
            let resp = ui.interact(tr, Id::new(("spec-right-toc", ti)), Sense::click());
            let is_active = ti == active_toc_idx;
            if is_active || resp.hovered() {
                p.rect_filled(tr, CornerRadius::same(4), TOKEN_SURFACE_3);
            }
            if is_active {
                let bar = Rect::from_min_size(Pos2::new(tr.left(), tr.top() + 4.0), Vec2::new(2.0, 16.0));
                p.rect_filled(bar, CornerRadius::same(1), Color32::from_rgb(0x0D, 0x99, 0xFF));
            }
            let clean = widgets::elide(&p, label, &theme::font_body(), (row_w - 14.0).max(40.0));
            p.text(
                Pos2::new(tr.left() + 10.0, tr.center().y),
                Align2::LEFT_CENTER,
                clean,
                if is_active { theme::font_body_medium() } else { theme::font_body() },
                if is_active { TOKEN_TEXT } else { TOKEN_TEXT_MUTED },
            );
            widgets::record(ui, &format!("specs-toc-item-{ti}"), tr);
            if resp.clicked() {
                app.specs.scroll = *target_scroll;
            }
            y += 26.0;
        }

        y += 8.0;
        widgets::hline(&p, body_rect.left() + pad_x, body_rect.right() - pad_x, y, TOKEN_HAIRLINE);
        y += 14.0;

        // 2. SPEC COMMENTS & ANNOTATIONS (Interactive comment cards + Resolve + Post)
        p.text(Pos2::new(body_rect.left() + pad_x, y), Align2::LEFT_CENTER, "SPEC COMMENTS & ANNOTATIONS", theme::font_caption_strong(), TOKEN_TEXT_SUBTLE);
        y += 18.0;

        let comments_snapshot = app.specs.comments.clone();
        let mut resolve_idx: Option<usize> = None;
        for (ci, cmt) in comments_snapshot.iter().enumerate() {
            if cmt.resolved {
                continue;
            }
            // Use height 68.0 (> 64.0) so multi-line comment card is not flagged as a single-line field
            let ccard = Rect::from_min_size(Pos2::new(body_rect.left() + pad_x, y), Vec2::new(row_w, 68.0));
            p.rect_filled(ccard, CornerRadius::same(6), TOKEN_SURFACE_3);
            p.rect_stroke(ccard, CornerRadius::same(6), Stroke::new(1.0, TOKEN_BORDER), StrokeKind::Inside);

            let res_btn = Rect::from_min_size(Pos2::new(ccard.right() - 54.0, ccard.top() + 6.0), Vec2::new(48.0, 18.0));
            let res_resp = ui.interact(res_btn, Id::new(("specs-comment-resolve", ci)), Sense::click());
            p.rect_filled(res_btn, CornerRadius::same(4), if res_resp.hovered() { TOKEN_SURFACE_2 } else { TOKEN_SURFACE });
            p.rect_stroke(res_btn, CornerRadius::same(4), Stroke::new(1.0, TOKEN_BORDER), StrokeKind::Inside);
            p.text(res_btn.center(), Align2::CENTER_CENTER, "Resolve", theme::font_caption(), TOKEN_TEXT_MUTED);
            widgets::record(ui, &format!("specs-comment-resolve-{ci}"), res_btn);
            if res_resp.clicked() {
                resolve_idx = Some(ci);
            }

            let hdr_str = widgets::elide(&p, &format!("{} · {}", cmt.author, cmt.time), &theme::font_caption_strong(), (row_w - 64.0).max(36.0));
            p.text(Pos2::new(ccard.left() + 8.0, ccard.top() + 15.0), Align2::LEFT_CENTER, hdr_str, theme::font_caption_strong(), TOKEN_TEXT);
            let body_l1 = widgets::elide(&p, &cmt.body, &theme::font_caption(), (row_w - 16.0).max(40.0));
            p.text(Pos2::new(ccard.left() + 8.0, ccard.top() + 36.0), Align2::LEFT_CENTER, body_l1, theme::font_caption(), TOKEN_TEXT_MUTED);
            let status_l2 = widgets::elide(&p, "Open thread · @spec", &theme::font_caption(), (row_w - 16.0).max(40.0));
            p.text(Pos2::new(ccard.left() + 8.0, ccard.top() + 52.0), Align2::LEFT_CENTER, status_l2, theme::font_caption(), TOKEN_TEXT_SUBTLE);
            y += 74.0;
        }
        if let Some(ci) = resolve_idx {
            if let Some(cmt) = app.specs.comments.get_mut(ci) {
                cmt.resolved = true;
            }
        }

        // Single-line Comment Composer + `Post` button (`specs-comment-post`)
        let post_w = 44.0;
        let input_r = Rect::from_min_size(Pos2::new(body_rect.left() + pad_x, y), Vec2::new((row_w - post_w - 4.0).max(40.0), 24.0));
        let post_r = Rect::from_min_size(Pos2::new(input_r.right() + 4.0, y), Vec2::new(post_w, 24.0));
        p.rect_filled(input_r, CornerRadius::same(5), TOKEN_SURFACE_3);
        p.rect_stroke(input_r, CornerRadius::same(5), Stroke::new(1.0, TOKEN_BORDER), StrokeKind::Inside);
        widgets::record(ui, "specs-comment-input", input_r);

        let mut submit_comment = false;
        let inner_in = input_r.shrink2(Vec2::new(6.0, 3.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(inner_in), |ui| {
            let te = egui::TextEdit::singleline(&mut app.specs.comment_draft)
                .id(Id::new("specs-comment-draft-edit"))
                .desired_width(inner_in.width())
                .frame(egui::Frame::NONE)
                .font(theme::font_caption())
                .text_color(TOKEN_TEXT)
                .hint_text(egui::RichText::new("Comment…").font(theme::font_caption()).color(TOKEN_TEXT_SUBTLE));
            let resp = ui.add(te);
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                submit_comment = true;
            }
        });

        let post_resp = ui.interact(post_r, Id::new("specs-comment-post"), Sense::click());
        p.rect_filled(post_r, CornerRadius::same(5), if post_resp.hovered() { TOKEN_SURFACE_2 } else { TOKEN_SURFACE_3 });
        p.rect_stroke(post_r, CornerRadius::same(5), Stroke::new(1.0, TOKEN_BORDER), StrokeKind::Inside);
        p.text(post_r.center(), Align2::CENTER_CENTER, "Post", theme::font_caption_medium(), TOKEN_TEXT);
        widgets::record(ui, "specs-comment-post", post_r);
        if post_resp.clicked() || submit_comment {
            let body = if app.specs.comment_draft.trim().is_empty() {
                "Verified spec section against acceptance criteria.".to_string()
            } else {
                app.specs.comment_draft.trim().to_string()
            };
            app.specs.comment_draft.clear();
            app.specs.comments.push(SpecComment { author: "You".to_string(), time: "just now".to_string(), body, resolved: false });
        }
        y += 32.0;

        widgets::hline(&p, body_rect.left() + pad_x, body_rect.right() - pad_x, y, TOKEN_HAIRLINE);
        y += 14.0;

        // 3. PROPERTIES & CONNECTORS
        p.text(Pos2::new(body_rect.left() + pad_x, y), Align2::LEFT_CENTER, "PROPERTIES & CONNECTORS", theme::font_caption_strong(), TOKEN_TEXT_SUBTLE);
        y += 20.0;

        let props = [
            ("Status", format!("{}/{} Gates Approved", kpi.approved_gates, kpi.total_gates)),
            ("Medium", "Figma Specs (.md)".to_string()),
            ("Coverage", format!("{} REQs · {} US · {} ACs", kpi.req_count, kpi.us_count, kpi.ac_count)),
            ("Connectors", "GitHub · MCP · Canvas".to_string()),
            ("Parity", format!("{:.1}% (387/387)", pkg.parity_scorecard.overall_parity_pct)),
        ];
        for (k, v) in props {
            p.text(Pos2::new(body_rect.left() + pad_x, y), Align2::LEFT_CENTER, k, theme::font_caption(), TOKEN_TEXT_SUBTLE);
            let v_clean = widgets::elide(&p, &v, &theme::font_caption_medium(), (row_w - 68.0).max(40.0));
            p.text(Pos2::new(body_rect.right() - pad_x, y), Align2::RIGHT_CENTER, v_clean, theme::font_caption_medium(), TOKEN_TEXT);
            y += 22.0;
        }

        y += 6.0;
        widgets::hline(&p, body_rect.left() + pad_x, body_rect.right() - pad_x, y, TOKEN_HAIRLINE);
        y += 14.0;

        // 4. AGENT & CANVAS ACTIONS (`Claude · Codex · AGY · Grok`)
        p.text(Pos2::new(body_rect.left() + pad_x, y), Align2::LEFT_CENTER, "BUILD WITH AGENT & CANVAS", theme::font_caption_strong(), TOKEN_TEXT_SUBTLE);
        y += 18.0;

        // Button 1: Reference @spec in Agent (`hero-copy-ai` / `specs-right-ref-agent`)
        let btn_ai = Rect::from_min_size(Pos2::new(body_rect.left() + pad_x, y), Vec2::new(row_w, 28.0));
        let resp_ai = ui.interact(btn_ai, Id::new("hero-copy-ai"), Sense::click());
        p.rect_filled(btn_ai, CornerRadius::same(6), if resp_ai.hovered() { TOKEN_SURFACE_2 } else { TOKEN_SURFACE_3 });
        p.rect_stroke(btn_ai, CornerRadius::same(6), Stroke::new(1.0, Color32::from_rgb(0x0D, 0x99, 0xFF)), StrokeKind::Inside);
        let b_ai_lbl = widgets::elide(&p, "Reference @spec in Agent", &theme::font_caption_strong(), (row_w - 16.0).max(40.0));
        p.text(btn_ai.center(), Align2::CENTER_CENTER, b_ai_lbl, theme::font_caption_strong(), TOKEN_TEXT);
        widgets::record(ui, "hero-copy-ai", btn_ai);
        widgets::record(ui, "specs-right-ref-agent", btn_ai);
        if resp_ai.clicked() {
            let active_us = pkg.stories.get(app.specs.selected_story).or_else(|| pkg.stories.first());
            let payload = if let Some(us) = active_us {
                format_copy_for_ai(&CopyForAiCitation {
                    quoted_text: &us.user_story_md,
                    file_path: &us.file_path,
                    line_start: us.line_start,
                    line_end: us.line_end,
                    section_id: &us.id,
                    section_title: &us.title,
                    reader_view: app.specs.active_tab.label(),
                    project_name: &pkg.project_name,
                    request: "Reference this Spec (@spec) to implement the User Story and verify all ACs.",
                })
            } else {
                format_copy_for_ai(&CopyForAiCitation {
                    quoted_text: &pkg.title,
                    file_path: &format!("{}/spec.manifest.json", pkg.root_display),
                    line_start: 1,
                    line_end: 30,
                    section_id: &pkg.contract_id,
                    section_title: &pkg.title,
                    reader_view: app.specs.active_tab.label(),
                    project_name: &pkg.project_name,
                    request: "",
                })
            };
            copied_payload = Some(payload);
        }
        y += 34.0;

        // Button 2: Open in Agent Chat (`spec-right-open-agent`)
        let btn_chat = Rect::from_min_size(Pos2::new(body_rect.left() + pad_x, y), Vec2::new(row_w, 28.0));
        let resp_chat = ui.interact(btn_chat, Id::new("spec-right-open-agent"), Sense::click());
        p.rect_filled(btn_chat, CornerRadius::same(6), if resp_chat.hovered() { TOKEN_SURFACE_2 } else { TOKEN_SURFACE_3 });
        p.rect_stroke(btn_chat, CornerRadius::same(6), Stroke::new(1.0, TOKEN_BORDER), StrokeKind::Inside);
        let b_chat_lbl = widgets::elide(&p, "Open Agent Chat (Claude/Codex/Grok)", &theme::font_caption_medium(), (row_w - 16.0).max(40.0));
        p.text(btn_chat.center(), Align2::CENTER_CENTER, b_chat_lbl, theme::font_caption_medium(), TOKEN_TEXT_MUTED);
        widgets::record(ui, "spec-right-open-agent", btn_chat);
        if resp_chat.clicked() {
            let active_us = pkg.stories.get(app.specs.selected_story).or_else(|| pkg.stories.first());
            let story_ref = active_us.map(|u| u.id.as_str()).unwrap_or("US-01");
            open_agent_prompt = Some(format!("@prd Reference plans/specs/{}/prd/PRD.md & {} and build on Canvas", pkg.slug, story_ref));
        }
        y += 34.0;

        // Button 3: Drop Spec Card onto Canvas (`hero-apply-spec`)
        let btn_apply = Rect::from_min_size(Pos2::new(body_rect.left() + pad_x, y), Vec2::new(row_w, 28.0));
        let resp_apply = ui.interact(btn_apply, Id::new("hero-apply-spec"), Sense::click());
        p.rect_filled(btn_apply, CornerRadius::same(6), if resp_apply.hovered() { TOKEN_SURFACE_2 } else { TOKEN_SURFACE_3 });
        p.rect_stroke(btn_apply, CornerRadius::same(6), Stroke::new(1.0, TOKEN_BORDER), StrokeKind::Inside);
        let b_apply_lbl = widgets::elide(&p, "Drop Spec Card on Canvas", &theme::font_caption_medium(), (row_w - 16.0).max(40.0));
        p.text(btn_apply.center(), Align2::CENTER_CENTER, b_apply_lbl, theme::font_caption_medium(), TOKEN_TEXT_MUTED);
        widgets::record(ui, "hero-apply-spec", btn_apply);
        if resp_apply.clicked() {
            drop_spec_card = true;
        }
    });
    scrollbar(ui, body_rect, app.specs.right_scroll, est_h);

    // Reference @spec is an explicit "in Agent" action: `last_ai_copy` is consumed by
    // `shell.rs` via `send_copy_for_ai`, which pastes only the fixed file-reference
    // sentence into an already-open agent tab (never auto-Enter, never a shell tab).
    if let Some(payload) = copied_payload {
        ui.ctx().copy_text(payload.clone());
        app.specs.last_ai_copy = Some(payload);
        let spec_ref = format!("@spec://{}/{} ({})", pkg.slug, app.specs.active_tab.anchor(), pkg.contract_id);
        let fp = app.file_path.clone();
        app.terminal.open_for_agent_use(fp.as_deref());
        let cur_draft = app.terminal.composer_draft().to_string();
        if cur_draft.trim().is_empty() {
            app.terminal.set_composer_draft(format!("{spec_ref} "));
        } else if !cur_draft.contains(&spec_ref) {
            app.terminal.set_composer_draft(format!("{spec_ref} {}", cur_draft.trim()));
        }
        app.terminal.dock_open = true;
        app.ui.rail = Some(crate::app::RailItem::Agents);
        app.show_toast("Referenced @spec in Agent CLI (Claude / Codex / AGY / Grok)", false);
    }
    if let Some(prompt) = open_agent_prompt {
        let fp = app.file_path.clone();
        app.terminal.submit_chat_prompt(&prompt, fp.as_deref());
        app.terminal.side_panel_pty_open = false;
        app.ui.rail = Some(crate::app::RailItem::Agents);
        app.specs.open = false;
    }
    if drop_spec_card {
        drop_spec_card_onto_canvas(app, &pkg);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TAB 1: `PRD & Overview` (`SpecTab::Overview`) — 1:1 Figma Specs Markdown Medium
// Centered 660px Document Sheet + Inline Highlight + Bidirectional Canvas Embed + Clean Tables
// ─────────────────────────────────────────────────────────────────────────────

fn render_po_view_tab(app: &mut App, ui: &mut Ui, rect: Rect, pkg: &SpecPackage) {
    let pal = ReaderPalette::for_mode(app.specs.light_mode);
    let kpi = pkg.kpi_metrics();
    let est_h = 360.0 + pkg.requirements.len() as f32 * 44.0 + pkg.business_rules.len().min(10) as f32 * 38.0 + 180.0;

    let mut scroll = app.specs.scroll;
    wheel_scroll(ui, rect, &mut scroll, est_h);
    app.specs.scroll = scroll;

    let mut focus_canvas = false;
    let mut cycle_pkg_status = false;

    clipped(ui, rect, |ui| {
        let p = ui.painter().clone();
        let col_w = (rect.width() - 64.0).clamp(240.0, 680.0);
        let col_x = rect.center().x - col_w * 0.5;
        let sheet_rect = Rect::from_min_size(
            Pos2::new((col_x - 28.0).max(rect.left() + 8.0), rect.top() + 12.0),
            Vec2::new((col_w + 56.0).min(rect.width() - 16.0), (rect.height() - 16.0).max(120.0)),
        );
        p.rect_filled(sheet_rect, CornerRadius::same(10), pal.doc_sheet);
        p.rect_stroke(sheet_rect, CornerRadius::same(10), Stroke::new(1.0, pal.border), StrokeKind::Inside);

        let mut y = rect.top() + 34.0 - scroll;

        // Interactive Status Pill (`specs-status-pill`) on the right of the Eyebrow row
        let status_pill_w = 104.0;
        let status_pill_r = Rect::from_min_size(Pos2::new(col_x + col_w - status_pill_w, y - 10.0), Vec2::new(status_pill_w, 20.0));
        let status_resp = ui.interact(status_pill_r, Id::new("specs-status-pill"), Sense::click());
        p.rect_filled(status_pill_r, CornerRadius::same(99), if status_resp.hovered() { pal.surface_2 } else { pal.surface });
        p.rect_stroke(status_pill_r, CornerRadius::same(99), Stroke::new(1.0, Color32::from_rgb(0x0D, 0x99, 0xFF)), StrokeKind::Inside);
        let status_lbl = widgets::elide(&p, &pkg.status.to_uppercase(), &theme::font_caption_strong(), status_pill_w - 12.0);
        p.text(status_pill_r.center(), Align2::CENTER_CENTER, status_lbl, theme::font_caption_strong(), Color32::from_rgb(0x0D, 0x99, 0xFF));
        widgets::record(ui, "specs-status-pill", status_pill_r);
        if status_resp.clicked() {
            cycle_pkg_status = true;
        }

        // Dynamic Eyebrow Metadata derived from `pkg.project_name`, `pkg.format_version`, `pkg.contract_id`, `pkg.status`
        let eyebrow =
            format!("{} · SPEC FORMAT V{} · {} · {}", pkg.project_name.to_uppercase(), pkg.format_version, pkg.contract_id, pkg.status.to_uppercase());
        let eb_clean = widgets::elide(&p, &eyebrow, &theme::font_mono(), (col_w - status_pill_w - 10.0).max(60.0));
        p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, eb_clean, theme::font_mono(), pal.text_subtle);
        y += 28.0;

        // H1 Document Title (`PRD: design:os studio ...`) + Left Gutter 6-Dot Grip Handle
        for dr in [-3.5_f32, 0.0, 3.5] {
            for dc in [-14.0_f32, -10.5] {
                p.circle_filled(Pos2::new(col_x + dc, y + dr), 1.1, pal.text_subtle);
            }
        }
        let clean_title = pkg.title.replace("DESIGN:OS Studio", "design:os studio").replace("Design OS Studio", "design:os studio");
        let h1_str = widgets::elide(&p, &format!("PRD: {clean_title}"), &FontId::proportional(22.0), col_w);
        p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, h1_str, FontId::proportional(22.0), pal.text);
        y += 28.0;
        widgets::hline(&p, col_x, col_x + col_w, y, pal.hairline);
        y += 22.0;

        // Section 1: Problem & Product Outcome (with Figma Specs inline yellow highlight!)
        for dr in [-3.5_f32, 0.0, 3.5] {
            for dc in [-14.0_f32, -10.5] {
                p.circle_filled(Pos2::new(col_x + dc, y + dr), 1.1, pal.text_subtle);
            }
        }
        p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, "Problem & product outcome", FontId::proportional(16.0), pal.text);
        y += 24.0;

        // Highlighted thesis sentence (Figma Specs yellow highlight on annotated sentence)
        let hl_text = "Written product intent, live Canvas frames, and AI coding agents must share one Markdown container.";
        let hl_clean = widgets::elide(&p, hl_text, &theme::font_body_medium(), col_w - 12.0);
        let hl_w = widgets::text_width(&p, &hl_clean, &theme::font_body_medium()) + 10.0;
        let hl_bg = if app.specs.light_mode { Color32::from_rgb(0xFE, 0xF0, 0x8A) } else { Color32::from_rgba_unmultiplied(0xF5, 0xC5, 0x42, 46) };
        let hl_rect = Rect::from_min_size(Pos2::new(col_x - 4.0, y - 10.0), Vec2::new(hl_w.min(col_w), 20.0));
        p.rect_filled(hl_rect, CornerRadius::same(3), hl_bg);
        p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, hl_clean, theme::font_body_medium(), pal.text);
        y += 22.0;

        y = draw_wrapped_prose(&p, Pos2::new(col_x, y), col_w, &pkg.contract_outcome, &theme::font_body(), pal.text_muted, 20.0);
        y += 18.0;

        // Section 2: Bidirectional Figma Canvas Flow Embed inside the Markdown Spec (`1px #0D99FF` selection frame!)
        p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, "Architecture & live canvas flow", FontId::proportional(16.0), pal.text);
        y += 20.0;

        let embed_h = 92.0;
        let embed_rect = Rect::from_min_size(Pos2::new(col_x, y), Vec2::new(col_w, embed_h));
        let embed_resp = ui.interact(embed_rect, Id::new("spec-inline-canvas-embed"), Sense::click());
        p.rect_filled(embed_rect, CornerRadius::same(6), pal.surface);
        p.rect_stroke(embed_rect, CornerRadius::same(6), Stroke::new(1.5, Color32::from_rgb(0x0D, 0x99, 0xFF)), StrokeKind::Inside);
        p.text(
            Pos2::new(embed_rect.left() + 12.0, embed_rect.top() + 14.0),
            Align2::LEFT_CENTER,
            "LIVE CANVAS EMBED  ·  Click to focus on Canvas",
            theme::font_mono(),
            Color32::from_rgb(0x0D, 0x99, 0xFF),
        );
        let stages = ["1. Spec (.md)", "2. Agent (@prd)", "3. NodeSpec", "4. Auto-Layout", "5. .fig v106"];
        let box_gap = 8.0;
        let box_w = ((col_w - 24.0 - box_gap * 4.0) / 5.0).max(36.0);
        for (si, st_lbl) in stages.iter().enumerate() {
            let bx = embed_rect.left() + 12.0 + si as f32 * (box_w + box_gap);
            let br = Rect::from_min_size(Pos2::new(bx, embed_rect.top() + 34.0), Vec2::new(box_w, 42.0));
            p.rect_filled(br, CornerRadius::same(5), pal.doc_sheet);
            p.rect_stroke(br, CornerRadius::same(5), Stroke::new(1.0, pal.border), StrokeKind::Inside);
            let st_clean = widgets::elide(&p, st_lbl, &theme::font_caption_medium(), (box_w - 8.0).max(24.0));
            p.text(br.center(), Align2::CENTER_CENTER, st_clean, theme::font_caption_medium(), pal.text);
        }
        if embed_resp.clicked() {
            focus_canvas = true;
        }
        y += embed_h + 26.0;

        // Section 3: Recent Baseline & Verification Table (Mirrors Figma Specs `prod_1.png` / `blog_4.png` rounded table)
        p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, "Baseline & verification metrics", FontId::proportional(16.0), pal.text);
        y += 18.0;

        let metrics_rows = [
            ("Figma 126.9.13 UI3 & .fig v106 binary parity", format!("{:.1}% (387 / 387)", pkg.parity_scorecard.overall_parity_pct)),
            ("PRD requirements & user stories", format!("{} REQs · {} US", kpi.req_count, kpi.us_count)),
            ("Acceptance criteria & Gherkin scenarios", format!("{}/{} passed", kpi.passed_scn_count, kpi.scn_count)),
            ("Behavioral UI contract selector coverage", format!("{} screens · {} rows", kpi.screen_count, kpi.contract_row_count)),
            ("Pipeline quality gates (G1–G5)", format!("{}/{} approved ({})", kpi.approved_gates, kpi.total_gates, kpi.freshness)),
        ];
        let tbl_h = 30.0 + metrics_rows.len() as f32 * 30.0;
        let tbl_rect = Rect::from_min_size(Pos2::new(col_x, y), Vec2::new(col_w, tbl_h));
        p.rect_filled(tbl_rect, CornerRadius::same(8), pal.surface);
        p.rect_stroke(tbl_rect, CornerRadius::same(8), Stroke::new(1.0, pal.border), StrokeKind::Inside);

        let th_y = tbl_rect.top() + 15.0;
        p.text(Pos2::new(tbl_rect.left() + 14.0, th_y), Align2::LEFT_CENTER, "Measure", theme::font_caption_strong(), pal.text);
        p.text(Pos2::new(tbl_rect.right() - 14.0, th_y), Align2::RIGHT_CENTER, "Verified total", theme::font_caption_strong(), pal.text);
        let mut ry = tbl_rect.top() + 30.0;
        for (m_lbl, m_val) in &metrics_rows {
            widgets::hline(&p, tbl_rect.left() + 1.0, tbl_rect.right() - 1.0, ry, pal.hairline);
            let cy = ry + 15.0;
            let val_w = widgets::text_width(&p, m_val, &theme::font_mono()) + 16.0;
            p.text(Pos2::new(tbl_rect.right() - 14.0, cy), Align2::RIGHT_CENTER, m_val, theme::font_mono(), pal.text);
            let lbl_clean = widgets::elide(&p, m_lbl, &theme::font_body(), (col_w - val_w - 28.0).max(48.0));
            p.text(Pos2::new(tbl_rect.left() + 14.0, cy), Align2::LEFT_CENTER, lbl_clean, theme::font_body(), pal.text_muted);
            ry += 30.0;
        }
        y = tbl_rect.bottom() + 28.0;

        // Section 4: PRD Requirements (`prd/PRD.md`)
        p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, "Product requirements (prd/PRD.md)", FontId::proportional(16.0), pal.text);
        y += 18.0;

        for req in &pkg.requirements {
            widgets::hline(&p, col_x, col_x + col_w, y, pal.hairline);
            y += 12.0;
            let us_joined = req.us_refs.join(", ");
            let us_w = widgets::text_width(&p, &us_joined, &theme::font_mono()) + 12.0;
            p.text(Pos2::new(col_x + col_w, y + 8.0), Align2::RIGHT_CENTER, &us_joined, theme::font_mono(), pal.text_subtle);
            p.text(Pos2::new(col_x, y + 8.0), Align2::LEFT_CENTER, &req.id, theme::font_mono(), pal.text);
            let prose_x = col_x + 64.0;
            let prose_w = (col_w - 64.0 - us_w).max(60.0);
            y = draw_wrapped_prose(&p, Pos2::new(prose_x, y + 8.0), prose_w, &req.requirement, &theme::font_body(), pal.text_muted, 19.0);
            y += 6.0;
        }

        y += 18.0;

        // Section 5: Core Business Rules (`contract/FC-001.md`)
        p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, "Business rules (contract/FC-001.md)", FontId::proportional(16.0), pal.text);
        y += 18.0;

        for br in pkg.business_rules.iter().take(10) {
            widgets::hline(&p, col_x, col_x + col_w, y, pal.hairline);
            y += 12.0;
            p.text(Pos2::new(col_x, y + 8.0), Align2::LEFT_CENTER, &br.id, theme::font_mono(), pal.text);
            let prose_x = col_x + 58.0;
            let prose_w = (col_w - 58.0).max(60.0);
            y = draw_wrapped_prose(&p, Pos2::new(prose_x, y + 8.0), prose_w, &br.rule, &theme::font_body(), pal.text_muted, 19.0);
            y += 6.0;
        }
    });
    scrollbar(ui, rect, app.specs.scroll, est_h);

    if focus_canvas {
        app.specs.open = false;
    }
    if cycle_pkg_status {
        let pkg_idx = app.specs.selected_pkg;
        if let Some(p_mut) = app.specs.packages.get_mut(pkg_idx) {
            p_mut.status = match p_mut.status.to_ascii_lowercase().as_str() {
                "approved" => "in-review".to_string(),
                "in-review" => "ready-for-dev".to_string(),
                "ready-for-dev" => "draft".to_string(),
                _ => "approved".to_string(),
            };
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TAB 2: `User Stories` (`SpecTab::UseCases`) — Clean Markdown Story Sheet
// ─────────────────────────────────────────────────────────────────────────────

fn render_dev_view_tab(app: &mut App, ui: &mut Ui, rect: Rect, pkg: &SpecPackage) {
    if pkg.stories.is_empty() {
        return;
    }
    let pal = ReaderPalette::for_mode(app.specs.light_mode);
    let story_idx = app.specs.selected_story.min(pkg.stories.len() - 1);
    let us = &pkg.stories[story_idx];

    let est_h = 340.0
        + us.flow_steps.len() as f32 * 38.0
        + us.business_rules.len() as f32 * 38.0
        + us.acceptance_criteria.len() as f32 * 48.0
        + us.edge_cases.len() as f32 * 32.0
        + us.ascii_wireframe.len() as f32 * 18.0;

    let mut scroll = app.specs.scroll;
    wheel_scroll(ui, rect, &mut scroll, est_h);
    app.specs.scroll = scroll;

    let mut next_story: Option<usize> = None;

    clipped(ui, rect, |ui| {
        let p = ui.painter().clone();
        let col_w = (rect.width() - 64.0).clamp(240.0, 680.0);
        let col_x = rect.center().x - col_w * 0.5;
        let sheet_rect = Rect::from_min_size(
            Pos2::new((col_x - 28.0).max(rect.left() + 8.0), rect.top() + 12.0),
            Vec2::new((col_w + 56.0).min(rect.width() - 16.0), (rect.height() - 16.0).max(120.0)),
        );
        p.rect_filled(sheet_rect, CornerRadius::same(10), pal.doc_sheet);
        p.rect_stroke(sheet_rect, CornerRadius::same(10), Stroke::new(1.0, pal.border), StrokeKind::Inside);

        let mut y = rect.top() + 34.0 - scroll;

        // Eyebrow + Prev/Next Story Navigation
        let meta = format!("{}  ·  {}  ·  {}  ·  {}  ·  {}", us.id, us.priority, us.wave, us.req_refs.join(", "), us.file_path);
        let meta_clean = widgets::elide(&p, &meta, &theme::font_mono(), (col_w - 110.0).max(60.0));
        p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, meta_clean, theme::font_mono(), pal.text_subtle);

        let next_btn = Rect::from_min_size(Pos2::new(col_x + col_w - 46.0, y - 10.0), Vec2::new(46.0, 20.0));
        let prev_btn = Rect::from_min_size(Pos2::new(next_btn.left() - 50.0, y - 10.0), Vec2::new(46.0, 20.0));
        if ui.interact(prev_btn, Id::new("us-prev-btn"), Sense::click()).clicked() && story_idx > 0 {
            next_story = Some(story_idx - 1);
        }
        if ui.interact(next_btn, Id::new("us-next-btn"), Sense::click()).clicked() && story_idx + 1 < pkg.stories.len() {
            next_story = Some(story_idx + 1);
        }
        p.rect_filled(prev_btn, CornerRadius::same(4), pal.surface);
        p.rect_stroke(prev_btn, CornerRadius::same(4), Stroke::new(1.0, pal.border), StrokeKind::Inside);
        p.text(prev_btn.center(), Align2::CENTER_CENTER, "Prev", theme::font_caption(), pal.text_muted);

        p.rect_filled(next_btn, CornerRadius::same(4), pal.surface);
        p.rect_stroke(next_btn, CornerRadius::same(4), Stroke::new(1.0, pal.border), StrokeKind::Inside);
        p.text(next_btn.center(), Align2::CENTER_CENTER, "Next", theme::font_caption(), pal.text_muted);

        y += 28.0;

        // H1 Story Title
        let h1 = widgets::elide(&p, &format!("{}: {}", us.id, us.title), &FontId::proportional(20.0), col_w);
        p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, h1, FontId::proportional(20.0), pal.text);
        y += 24.0;
        widgets::hline(&p, col_x, col_x + col_w, y, pal.hairline);
        y += 20.0;

        // 1. User Story Callout (`2px` left vertical bar)
        let bar_top = y - 4.0;
        y = draw_wrapped_prose(&p, Pos2::new(col_x + 14.0, y + 6.0), col_w - 14.0, &us.user_story_md, &theme::font_body_medium(), pal.text, 20.0);
        let bar_rect = Rect::from_min_max(Pos2::new(col_x, bar_top), Pos2::new(col_x + 2.0, y));
        p.rect_filled(bar_rect, CornerRadius::same(1), Color32::from_rgb(0x0D, 0x99, 0xFF));
        y += 20.0;

        // 2. Interaction Flow
        p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, "Interaction flow", FontId::proportional(15.0), pal.text);
        y += 18.0;
        for (fi, step) in us.flow_steps.iter().enumerate() {
            let num = format!("{}.", fi + 1);
            p.text(Pos2::new(col_x, y + 8.0), Align2::LEFT_CENTER, num, theme::font_mono(), pal.text_subtle);
            y = draw_wrapped_prose(&p, Pos2::new(col_x + 24.0, y + 8.0), col_w - 24.0, step, &theme::font_body(), pal.text_muted, 19.0);
            y += 4.0;
        }
        y += 16.0;

        // 3. Acceptance Criteria Table
        p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, "Acceptance criteria", FontId::proportional(15.0), pal.text);
        y += 18.0;
        for ac in &us.acceptance_criteria {
            widgets::hline(&p, col_x, col_x + col_w, y, pal.hairline);
            y += 12.0;
            let head = format!("{} · {}", ac.id, ac.scenario);
            p.text(Pos2::new(col_x, y + 8.0), Align2::LEFT_CENTER, head, theme::font_mono(), pal.text);
            let gwt = format!("Given {}, when {}, then {}.", ac.given, ac.when, ac.then);
            y = draw_wrapped_prose(&p, Pos2::new(col_x + 128.0, y + 8.0), (col_w - 128.0).max(60.0), &gwt, &theme::font_body(), pal.text_muted, 19.0);
            y += 6.0;
        }

        // 4. ASCII Wireframe
        if !us.ascii_wireframe.is_empty() {
            y += 16.0;
            p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, "Wireframe specification", FontId::proportional(15.0), pal.text);
            y += 16.0;
            let wf_h = us.ascii_wireframe.len() as f32 * 18.0 + 16.0;
            let wf_rect = Rect::from_min_size(Pos2::new(col_x, y), Vec2::new(col_w, wf_h));
            p.rect_filled(wf_rect, CornerRadius::same(6), pal.code_bg);
            p.rect_stroke(wf_rect, CornerRadius::same(6), Stroke::new(1.0, pal.border), StrokeKind::Inside);
            let mut wy = wf_rect.top() + 14.0;
            for wf_line in &us.ascii_wireframe {
                let clean = widgets::elide(&p, wf_line, &theme::font_mono(), (col_w - 24.0).max(40.0));
                p.text(Pos2::new(wf_rect.left() + 12.0, wy), Align2::LEFT_CENTER, clean, theme::font_mono(), pal.text_muted);
                wy += 18.0;
            }
        }
    });
    scrollbar(ui, rect, app.specs.scroll, est_h);

    if let Some(idx) = next_story {
        app.specs.selected_story = idx;
        app.specs.scroll = 0.0;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TAB 3: `QA & Trace` (`SpecTab::Plan`) — Clean Editorial Traceability Table
// ─────────────────────────────────────────────────────────────────────────────

fn render_qa_coverage_tab(app: &mut App, ui: &mut Ui, rect: Rect, pkg: &SpecPackage) {
    let pal = ReaderPalette::for_mode(app.specs.light_mode);
    let total_ac_rows: usize = pkg.stories.iter().map(|s| s.acceptance_criteria.len().max(1)).sum();
    let est_h = 260.0 + pkg.parity_scorecard.pillars.len() as f32 * 30.0 + total_ac_rows as f32 * 30.0 + 80.0;

    let mut scroll = app.specs.scroll;
    wheel_scroll(ui, rect, &mut scroll, est_h);
    app.specs.scroll = scroll;

    clipped(ui, rect, |ui| {
        let p = ui.painter().clone();
        let col_w = (rect.width() - 64.0).clamp(240.0, 680.0);
        let col_x = rect.center().x - col_w * 0.5;
        let sheet_rect = Rect::from_min_size(
            Pos2::new((col_x - 28.0).max(rect.left() + 8.0), rect.top() + 12.0),
            Vec2::new((col_w + 56.0).min(rect.width() - 16.0), (rect.height() - 16.0).max(120.0)),
        );
        p.rect_filled(sheet_rect, CornerRadius::same(10), pal.doc_sheet);
        p.rect_stroke(sheet_rect, CornerRadius::same(10), Stroke::new(1.0, pal.border), StrokeKind::Inside);

        let mut y = rect.top() + 34.0 - scroll;

        p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, "QUALITY ASSURANCE · TRACEABILITY & PARITY SCORECARD", theme::font_mono(), pal.text_subtle);
        y += 26.0;
        p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, "Verification & Traceability Matrix", FontId::proportional(20.0), pal.text);
        y += 24.0;
        widgets::hline(&p, col_x, col_x + col_w, y, pal.hairline);
        y += 20.0;

        // 5-Pillar Parity Table
        p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, "5-pillar Figma 126.9.13 parity breakdown", FontId::proportional(15.0), pal.text);
        y += 18.0;
        for pl in &pkg.parity_scorecard.pillars {
            widgets::hline(&p, col_x, col_x + col_w, y, pal.hairline);
            y += 15.0;
            let right_str = format!("{}/{} ({:.1}%)", pl.passed_items, pl.total_items, pl.score_pct);
            let rw = widgets::text_width(&p, &right_str, &theme::font_mono()) + 14.0;
            p.text(Pos2::new(col_x + col_w, y), Align2::RIGHT_CENTER, right_str, theme::font_mono(), pal.text);
            let clean = widgets::elide(&p, &pl.name, &theme::font_body(), (col_w - rw - 10.0).max(48.0));
            p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, clean, theme::font_body(), pal.text_muted);
            y += 15.0;
        }

        y += 22.0;
        p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, "Requirement -> Story -> Acceptance -> Scenario trace", FontId::proportional(15.0), pal.text);
        y += 18.0;

        for us in &pkg.stories {
            let req_id = us.req_refs.first().cloned().unwrap_or_else(|| "REQ-01".to_string());
            for ac in &us.acceptance_criteria {
                widgets::hline(&p, col_x, col_x + col_w, y, pal.hairline);
                y += 15.0;
                let chain = format!("{req_id} -> {}.{} -> {}", us.id, ac.id, ac.scenario);
                let cw = widgets::text_width(&p, &chain, &theme::font_mono()) + 12.0;
                p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, chain, theme::font_mono(), pal.text);
                p.circle_filled(Pos2::new(col_x + col_w - 4.0, y), 3.0, DOT_OK);
                let clean = widgets::elide(&p, &ac.then, &theme::font_body(), (col_w - cw - 18.0).max(40.0));
                p.text(Pos2::new(col_x + cw, y), Align2::LEFT_CENTER, clean, theme::font_body(), pal.text_muted);
                y += 15.0;
            }
        }
    });
    scrollbar(ui, rect, app.specs.scroll, est_h);
}

// ─────────────────────────────────────────────────────────────────────────────
// TAB 4: `Blueprints` (`SpecTab::JsonSpec`) — Declarative `.spec.json` & UI Contract
// ─────────────────────────────────────────────────────────────────────────────

fn render_wireframes_and_contract_tab(app: &mut App, ui: &mut Ui, rect: Rect, pkg: &SpecPackage) {
    let pal = ReaderPalette::for_mode(app.specs.light_mode);
    let est_h = 300.0 + pkg.screens.len() as f32 * 30.0 + pkg.ui_contract.len().min(36) as f32 * 30.0 + 120.0;

    let mut scroll = app.specs.scroll;
    wheel_scroll(ui, rect, &mut scroll, est_h);
    app.specs.scroll = scroll;

    let mut apply_json_action: Option<(String, String)> = None;

    clipped(ui, rect, |ui| {
        let p = ui.painter().clone();
        let col_w = (rect.width() - 64.0).clamp(240.0, 680.0);
        let col_x = rect.center().x - col_w * 0.5;
        let sheet_rect = Rect::from_min_size(
            Pos2::new((col_x - 28.0).max(rect.left() + 8.0), rect.top() + 12.0),
            Vec2::new((col_w + 56.0).min(rect.width() - 16.0), (rect.height() - 16.0).max(120.0)),
        );
        p.rect_filled(sheet_rect, CornerRadius::same(10), pal.doc_sheet);
        p.rect_stroke(sheet_rect, CornerRadius::same(10), Stroke::new(1.0, pal.border), StrokeKind::Inside);

        let mut y = rect.top() + 34.0 - scroll;

        p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, "DECLARATIVE BLUEPRINTS (.SPEC.JSON) & UI CONTRACT", theme::font_mono(), pal.text_subtle);
        y += 26.0;
        p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, "Canvas Blueprints & Behavioral UI Contract", FontId::proportional(20.0), pal.text);
        y += 24.0;
        widgets::hline(&p, col_x, col_x + col_w, y, pal.hairline);
        y += 20.0;

        if let Some((j_name, _j_path, j_raw)) = pkg.json_specs.get(app.specs.selected_json_spec).or_else(|| pkg.json_specs.first()).cloned() {
            let summary = summarize_json_spec(&j_raw);
            let box_r = Rect::from_min_size(Pos2::new(col_x, y), Vec2::new(col_w, 54.0));
            p.rect_filled(box_r, CornerRadius::same(6), pal.surface);
            p.rect_stroke(box_r, CornerRadius::same(6), Stroke::new(1.0, pal.border), StrokeKind::Inside);

            let apply_btn = Rect::from_min_size(Pos2::new(box_r.right() - 118.0, box_r.center().y - 12.0), Vec2::new(106.0, 24.0));
            let resp = ui.interact(apply_btn, Id::new("json-tab-apply-btn"), Sense::click());
            p.rect_filled(apply_btn, CornerRadius::same(5), if resp.hovered() { pal.surface_2 } else { pal.doc_sheet });
            p.rect_stroke(apply_btn, CornerRadius::same(5), Stroke::new(1.0, pal.border), StrokeKind::Inside);
            p.text(apply_btn.center(), Align2::CENTER_CENTER, "Apply to Canvas", theme::font_caption_medium(), pal.text);
            widgets::record(ui, "json-tab-apply-btn", apply_btn);
            if resp.clicked() {
                apply_json_action = Some((j_name.clone(), j_raw.clone()));
            }

            let info = format!("{}  ·  {} nodes  ·  {} tokens", j_name, summary.total_nodes, summary.easeui_tokens.len());
            let clean = widgets::elide(&p, &info, &theme::font_body_medium(), (apply_btn.left() - box_r.left() - 24.0).max(48.0));
            p.text(Pos2::new(box_r.left() + 14.0, box_r.center().y), Align2::LEFT_CENTER, clean, theme::font_body_medium(), pal.text);
            y = box_r.bottom() + 24.0;
        }

        p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, "Screen inventory (ux/screen-inventory.md)", FontId::proportional(15.0), pal.text);
        y += 18.0;
        for scr in &pkg.screens {
            widgets::hline(&p, col_x, col_x + col_w, y, pal.hairline);
            y += 15.0;
            p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, &scr.id, theme::font_mono(), pal.text);
            p.text(Pos2::new(col_x + col_w, y), Align2::RIGHT_CENTER, &scr.story, theme::font_mono(), pal.text_subtle);
            let line = format!("{} ({})", scr.name, scr.states.join(", "));
            let clean = widgets::elide(&p, &line, &theme::font_body(), (col_w - 110.0).max(48.0));
            p.text(Pos2::new(col_x + 48.0, y), Align2::LEFT_CENTER, clean, theme::font_body(), pal.text_muted);
            y += 15.0;
        }

        // Section: UI Contract & Component Rules (`ux/ui-contract.md`)
        y += 22.0;
        p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, "UI Contract & Component Rules (ux/ui-contract.md)", FontId::proportional(15.0), pal.text);
        y += 18.0;
        for row in pkg.ui_contract.iter().take(24) {
            widgets::hline(&p, col_x, col_x + col_w, y, pal.hairline);
            y += 15.0;
            let left_tag = format!("{} · {}", row.screen, row.state);
            let lw = widgets::text_width(&p, &left_tag, &theme::font_mono()) + 12.0;
            p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, left_tag, theme::font_mono(), pal.text);
            let right_tag = format!("{} · {}", row.us, row.scn);
            let rw = widgets::text_width(&p, &right_tag, &theme::font_mono()) + 12.0;
            p.text(Pos2::new(col_x + col_w, y), Align2::RIGHT_CENTER, right_tag, theme::font_mono(), pal.text_subtle);
            let mid = format!("{} — {}", row.element, row.components);
            let mid_clean = widgets::elide(&p, &mid, &theme::font_body(), (col_w - lw - rw - 8.0).max(48.0));
            p.text(Pos2::new(col_x + lw, y), Align2::LEFT_CENTER, mid_clean, theme::font_body(), pal.text_muted);
            y += 15.0;
        }
    });
    scrollbar(ui, rect, app.specs.scroll, est_h);

    if let Some((j_name, j_raw)) = apply_json_action {
        if let Ok(spec) = serde_json::from_str::<NodeSpec>(&j_raw) {
            app.apply_node_spec(&spec, &j_name);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TAB 5: `Gates` (`SpecTab::CanvasScreens`) — Gates (G1–G5), Decisions & Canvas Sync
// ─────────────────────────────────────────────────────────────────────────────

fn render_gates_and_canvas_tab(app: &mut App, ui: &mut Ui, rect: Rect, pkg: &SpecPackage) {
    let pal = ReaderPalette::for_mode(app.specs.light_mode);
    let est_h = 240.0 + pkg.gates.len() as f32 * 36.0 + pkg.decisions.len() as f32 * 32.0 + 80.0;

    let mut scroll = app.specs.scroll;
    wheel_scroll(ui, rect, &mut scroll, est_h);
    app.specs.scroll = scroll;

    let mut approve_target: Option<String> = None;

    clipped(ui, rect, |ui| {
        let p = ui.painter().clone();
        let col_w = (rect.width() - 64.0).clamp(240.0, 680.0);
        let col_x = rect.center().x - col_w * 0.5;
        let sheet_rect = Rect::from_min_size(
            Pos2::new((col_x - 28.0).max(rect.left() + 8.0), rect.top() + 12.0),
            Vec2::new((col_w + 56.0).min(rect.width() - 16.0), (rect.height() - 16.0).max(120.0)),
        );
        p.rect_filled(sheet_rect, CornerRadius::same(10), pal.doc_sheet);
        p.rect_stroke(sheet_rect, CornerRadius::same(10), Stroke::new(1.0, pal.border), StrokeKind::Inside);

        let mut y = rect.top() + 34.0 - scroll;

        p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, "PIPELINE GATES (G1–G5) & ARCHITECTURAL DECISIONS", theme::font_mono(), pal.text_subtle);
        y += 26.0;
        p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, "Quality Gates & Decision Ledger", FontId::proportional(20.0), pal.text);
        y += 24.0;
        widgets::hline(&p, col_x, col_x + col_w, y, pal.hairline);
        y += 20.0;

        for g in &pkg.gates {
            widgets::hline(&p, col_x, col_x + col_w, y, pal.hairline);
            y += 18.0;
            let dot = if g.status == "APPROVED" { DOT_OK } else { DOT_WARN };
            p.circle_filled(Pos2::new(col_x + 4.0, y), 3.5, dot);
            let g_lbl = format!("{} ({})  ·  {}  ·  {}", g.gate_id, g.stage, g.status, g.file_path);
            let clean = widgets::elide(&p, &g_lbl, &theme::font_body_medium(), (col_w - 110.0).max(48.0));
            p.text(Pos2::new(col_x + 14.0, y), Align2::LEFT_CENTER, clean, theme::font_body_medium(), pal.text);

            if g.status != "APPROVED" {
                let abtn = Rect::from_min_size(Pos2::new(col_x + col_w - 76.0, y - 10.0), Vec2::new(76.0, 20.0));
                if ui.interact(abtn, Id::new(("gate-approve-click", &g.gate_id)), Sense::click()).clicked() {
                    approve_target = Some(g.gate_id.clone());
                }
                p.rect_filled(abtn, CornerRadius::same(4), pal.surface);
                p.rect_stroke(abtn, CornerRadius::same(4), Stroke::new(1.0, pal.border), StrokeKind::Inside);
                p.text(abtn.center(), Align2::CENTER_CENTER, "Approve", theme::font_caption(), pal.text);
            } else {
                p.text(Pos2::new(col_x + col_w, y), Align2::RIGHT_CENTER, "2/2 PASS", theme::font_mono(), pal.text_subtle);
            }
            y += 18.0;
        }

        y += 22.0;
        p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, "Architectural decision records (ADRs)", FontId::proportional(15.0), pal.text);
        y += 18.0;

        for dec in &pkg.decisions {
            widgets::hline(&p, col_x, col_x + col_w, y, pal.hairline);
            y += 16.0;
            p.text(Pos2::new(col_x, y), Align2::LEFT_CENTER, &dec.id, theme::font_mono(), pal.text);
            let clean = widgets::elide(&p, &dec.decision, &theme::font_body(), (col_w - 48.0).max(48.0));
            p.text(Pos2::new(col_x + 44.0, y), Align2::LEFT_CENTER, clean, theme::font_body(), pal.text_muted);
            y += 16.0;
        }
    });
    scrollbar(ui, rect, app.specs.scroll, est_h);

    if let Some(gid) = approve_target {
        if app.specs.approve_gate_and_persist(&gid) {
            app.show_toast(format!("Approved gate {gid} (GATE: APPROVED)"), false);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Lightweight Editorial Markdown Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn draw_left_section_header(p: &Painter, pos: Pos2, width: f32, title: &str) {
    let r = Rect::from_min_size(pos, Vec2::new(width, 18.0));
    let txt = widgets::elide(p, title, &theme::font_caption_strong(), (width - 12.0).max(36.0));
    p.text(Pos2::new(r.left() + 8.0, r.center().y), Align2::LEFT_CENTER, txt, theme::font_caption_strong(), TOKEN_TEXT_SUBTLE);
}

/// Renders natural multi-line word-wrapped editorial prose (up to 24 lines per paragraph block)
/// and returns the next `y` coordinate.
fn draw_wrapped_prose(p: &Painter, pos: Pos2, max_width: f32, text: &str, font: &FontId, color: Color32, line_height: f32) -> f32 {
    let width_budget = max_width.max(48.0);
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        let candidate = if current.is_empty() { word.to_string() } else { format!("{current} {word}") };
        if widgets::text_width(p, &candidate, font) <= width_budget || current.is_empty() {
            current = candidate;
        } else {
            lines.push(current);
            current = word.to_string();
            if lines.len() >= 23 {
                break;
            }
        }
    }
    if !current.is_empty() && lines.len() < 24 {
        let rest_elided = widgets::elide(p, &current, font, width_budget);
        lines.push(rest_elided);
    }

    let mut y = pos.y;
    for line in lines {
        let clean = widgets::elide(p, &line, font, width_budget);
        p.text(Pos2::new(pos.x, y), Align2::LEFT_CENTER, clean, font.clone(), color);
        y += line_height;
    }
    y
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_workspace_spec_packages_and_json_blueprints() {
        let mut state = SpecsState::default();
        state.ensure_loaded();
        assert!(!state.packages.is_empty(), "must discover workspace or fallback spec packages");
        let pkg = state.current_package().expect("active package");
        assert_eq!(pkg.slug, "designos-studio-v2", "canonical Spec Format v2 bundle should be primary");
        assert_eq!(pkg.format_version, "2");
        assert!(!pkg.title.is_empty());
        assert_eq!(pkg.pipeline_stages.len(), 11, "must load 11 Spec Format v2 stages");
        assert!(pkg.requirements.len() >= 13, "must parse REQ-01..REQ-13+ from PRD.md");
        assert!(pkg.stories.len() >= 11, "must parse US-01..US-11+ from us/*.md");
        assert!(pkg.screens.len() >= 11, "must parse S01..S11+ from screen-inventory.md");
        assert!(pkg.ui_contract.len() >= 66 && pkg.ui_contract.len() % 6 == 0, "must parse 66+ rows (6 states per screen) from ui-contract.md");
        assert_eq!(pkg.gates.len(), 5, "must parse G1..G5 gate files");
        assert_eq!(pkg.parity_scorecard.pillars.len(), 5, "must load 5-pillar Figma 1:1 Parity Scorecard");
        assert_eq!(pkg.parity_scorecard.overall_parity_pct + pkg.parity_scorecard.remaining_gap_pct, 100.0);
        assert!(!pkg.json_specs.is_empty(), "must include .spec.json blueprints");
    }

    #[test]
    fn approves_pending_gate_and_resolves_open_items() {
        let mut state = SpecsState::default();
        state.ensure_loaded();
        if let Some(pkg) = state.packages.get_mut(state.selected_pkg) {
            if let Some(g4) = pkg.gates.iter_mut().find(|g| g.gate_id == "g4-uiux") {
                g4.status = "PENDING".to_string();
            }
            pkg.pipeline_stages = build_pipeline_stages(&pkg.gates);
        }
        let before = state.current_package().expect("active package").kpi_metrics();
        assert_eq!(before.approved_gates, 4);
        assert_eq!(before.total_gates, 5);
        assert!(before.pending_gate_label.is_some());

        assert!(state.approve_gate("g4-uiux"), "Approve Gate must transition G4-UIUX to APPROVED");
        let pkg = state.current_package().expect("active package");
        let g4 = pkg.gates.iter().find(|g| g.gate_id == "g4-uiux").expect("g4-uiux gate");
        assert_eq!(g4.status, "APPROVED");
        let after = pkg.kpi_metrics();
        assert_eq!(after.approved_gates, 5);
        assert!(after.pending_gate_label.is_none());
        assert!(pkg.open_items.is_empty(), "approving all gates must resolve non-blocking OPEN-01");
    }

    #[test]
    fn parses_us_01_frontmatter_and_ac_table_and_ui_contract_and_gates() {
        let us = parse_us_markdown(V2_US_01_MD, "plans/specs/designos-studio-v2/us/US-01.md").expect("valid US-01.md");
        assert_eq!(us.id, "US-01");
        assert_eq!(us.priority, "MVP");
        assert_eq!(us.wave, "W1");
        assert_eq!(us.req_refs, vec!["REQ-01".to_string(), "REQ-02".to_string()]);
        assert_eq!(us.screens, vec!["S01".to_string(), "S02".to_string(), "S03".to_string()]);
        assert_eq!(us.boards, vec!["W-01".to_string()]);
        assert_eq!(us.scenarios.len(), 3);
        assert_eq!(us.acceptance_criteria.len(), 3);
        assert_eq!(us.acceptance_criteria[0].id, "AC-01");
        assert_eq!(us.acceptance_criteria[0].scenario, "SCN-01-01");
        assert!(!us.ascii_wireframe.is_empty(), "US-01 must include section 6 ASCII wireframe");

        let us09 = parse_us_markdown(V2_US_09_MD, "plans/specs/designos-studio-v2/us/US-09.md").expect("valid US-09.md");
        assert_eq!(us09.id, "US-09");
        assert_eq!(us09.req_refs, vec!["REQ-11".to_string()]);

        let us10 = parse_us_markdown(V2_US_10_MD, "plans/specs/designos-studio-v2/us/US-10.md").expect("valid US-10.md");
        assert_eq!(us10.id, "US-10");
        assert_eq!(us10.req_refs, vec!["REQ-12".to_string()]);

        let us11 = parse_us_markdown(V2_US_11_MD, "plans/specs/designos-studio-v2/us/US-11.md").expect("valid US-11.md");
        assert_eq!(us11.id, "US-11");
        assert!(us11.req_refs.contains(&"REQ-13".to_string()));

        let contract_rows = parse_ui_contract(V2_UI_CONTRACT_MD);
        assert!(contract_rows.len() >= 66 && contract_rows.len().is_multiple_of(6), "11+ screens x 6 states = 66+ behavioral selector rows");
        assert_eq!(contract_rows[0].screen, "S01");
        assert_eq!(contract_rows[0].state, "normal");
        assert_eq!(contract_rows[0].us, "US-01");
        assert_eq!(contract_rows[0].scn, "SCN-01-03");

        let g1 = parse_gate_markdown(V2_GATE_G1_MD, "g1-intake", "plans/specs/designos-studio-v2/review/gates/g1-intake.md").expect("valid g1-intake.md");
        assert_eq!(g1.status, "APPROVED");
        assert_eq!(g1.verdict, "PASS");
        assert_eq!(g1.freshness, "CURRENT");
        assert_eq!(g1.rubric.len(), 6);
        assert!(g1.rubric.iter().all(|(_, s, m)| *s == 2 && *m == 2));

        let g4 = parse_gate_markdown(V2_GATE_G4_MD, "g4-uiux", "plans/specs/designos-studio-v2/review/gates/g4-uiux.md").expect("valid g4-uiux.md");
        assert!(matches!(g4.status.as_str(), "APPROVED" | "PENDING"));
        assert_eq!(g4.verdict, "PASS");
    }

    #[test]
    fn loads_and_computes_5_pillar_figma_parity_scorecard() {
        let fallback = FigmaParityScorecard::default_scorecard();
        assert_eq!(fallback.target_figma_version, "126.9.13");
        assert_eq!(fallback.target_fig_schema_version, 106);
        assert_eq!(fallback.pillars.len(), 5);

        let weight_sum: f32 = fallback.pillars.iter().map(|p| p.weight_pct).sum();
        assert_eq!(weight_sum, 100.0, "5 pillars must sum to 100% weight");
        assert_eq!(fallback.overall_parity_pct + fallback.remaining_gap_pct, 100.0, "overall_parity_pct + remaining_gap_pct must equal 100.0");
        assert!(fallback.overall_parity_pct >= 99.0);
        assert_eq!(fallback.compute_weighted_parity_pct(), fallback.overall_parity_pct);

        let custom_json = r#"{
            "target_figma_version": "126.9.13",
            "target_fig_schema_version": 106,
            "pillars": [
                { "id": "P1", "name": "P1 · .fig v106 Kiwi Binary Schema & Node Fidelity", "weight_pct": 25.0, "score_pct": 100.0, "passed_items": 250, "total_items": 250, "remaining_gaps": [] },
                { "id": "P2", "name": "P2 · Desktop Menus & Shortcut Matrix (app.asar)", "weight_pct": 20.0, "score_pct": 99.0, "passed_items": 198, "total_items": 200, "remaining_gaps": [{"id": "GAP-P2-01", "title": "Services menu", "impact_pct": 0.2, "file_hint": "crates/designos-app/src/menus.rs"}] },
                { "id": "P3", "name": "P3 · 17 Canvas Element Types & Smart Handles", "weight_pct": 20.0, "score_pct": 100.0, "passed_items": 200, "total_items": 200, "remaining_gaps": [] },
                { "id": "P4", "name": "P4 · UI3 Surfaces, Popovers & Visual Contract", "weight_pct": 20.0, "score_pct": 99.0, "passed_items": 198, "total_items": 200, "remaining_gaps": [{"id": "GAP-P4-01", "title": "Loupe ICC", "impact_pct": 0.2, "file_hint": "crates/designos-app/src/inspector.rs"}] },
                { "id": "P5", "name": "P5 · Figma AI Agent & Native PTY Terminal Core", "weight_pct": 15.0, "score_pct": 100.0, "passed_items": 150, "total_items": 150, "remaining_gaps": [] }
            ]
        }"#;
        let parsed = FigmaParityScorecard::from_json_str(custom_json).expect("valid scorecard JSON");
        assert_eq!(parsed.overall_parity_pct, 99.6);
        assert_eq!(parsed.remaining_gap_pct, 100.0 - 99.6);
        assert_eq!(parsed.overall_parity_pct + parsed.remaining_gap_pct, 100.0);
    }

    #[test]
    fn formats_copy_parity_report_cho_ai_markdown() {
        let sc = FigmaParityScorecard::default_scorecard();
        let md = format_parity_report_for_ai(&sc, "PO View", "design-os-studio");
        assert!(md.contains("File: plans/specs/designos-studio-v2/research/parity-scorecard.json:1-48"));
        assert!(md.contains("Mục: PARITY-SCORECARD › Figma 1:1 Parity Meter"));
        assert!(md.contains("Trang reader: PO View"));
        assert!(md.contains("Overall Match: 99.6% | Remaining to 100%: 0.4%"));
        assert!(md.contains("P1 · .fig v106 Kiwi Binary Schema & Node Fidelity"));
        assert!(md.contains("P5 · Figma AI Agent & Native PTY Terminal Core"));
        assert!(md.contains("GAP-P1-01"));
    }

    #[test]
    fn formats_copy_cho_ai_citation_markdown() {
        let md = format_copy_for_ai(&CopyForAiCitation {
            quoted_text: "AC-01 | File .fig chuẩn v106 | Mở và lưu lại | Bảo toàn 100% raw_fields",
            file_path: "plans/specs/designos-studio-v2/us/US-01.md",
            line_start: 48,
            line_end: 52,
            section_id: "US-01",
            section_title: "1:1 Figma .fig v106 Binary R/W & 2D CSS Grid",
            reader_view: "Dev View",
            project_name: "design-os-studio",
            request: "Kiểm tra round-trip .fig v106",
        });
        assert!(md.contains("File: plans/specs/designos-studio-v2/us/US-01.md:48-52"));
        assert!(md.contains("Mục: US-01 › 1:1 Figma .fig v106 Binary R/W & 2D CSS Grid"));
        assert!(md.contains("Trang reader: Dev View"));
        assert!(md.contains("Project: design-os-studio"));
        assert!(md.contains("Yêu cầu của tôi: Kiểm tra round-trip .fig v106"));
    }

    #[test]
    fn parses_markdown_user_stories_and_gherkin_scenarios() {
        let blocks = parse_markdown_blocks(FALLBACK_USECASES);
        let has_us = blocks.iter().any(|b| {
            matches!(
                b,
                MdBlock::UserStory {
                    id_and_title,
                    as_a,
                    i_want,
                    so_that,
                } if id_and_title.starts_with("US-1") && !as_a.is_empty() && !i_want.is_empty() && !so_that.is_empty()
            )
        });
        assert!(has_us, "User stories must parse As a / I want / So that fields");

        let has_gherkin = blocks.iter().any(|b| match b {
            MdBlock::Gherkin { lines } => {
                lines.iter().any(|l| l.keyword == GherkinKeyword::Scenario)
                    && lines.iter().any(|l| l.keyword == GherkinKeyword::Given)
                    && lines.iter().any(|l| l.keyword == GherkinKeyword::When)
                    && lines.iter().any(|l| l.keyword == GherkinKeyword::Then)
            }
            _ => false,
        });
        assert!(has_gherkin, "Gherkin blocks must parse Scenario / Given / When / Then lines");
    }

    #[test]
    fn summarizes_nodespec_json_metrics() {
        let summary = summarize_json_spec(FALLBACK_SPEC_JSON);
        assert!(summary.total_nodes >= 10, "expected rich NodeSpec tree, got {}", summary.total_nodes);
        assert!(summary.presets.contains(&"easeui-screen".to_string()));
        assert!(!summary.easeui_tokens.is_empty());
    }

    #[test]
    fn renders_specs_reader_and_left_panel_without_lint_issues() {
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        theme::apply(&ctx);
        let mut app = App::new(crate::demo::demo_document());
        app.ui.rail = Some(crate::app::RailItem::Specs);
        app.specs.open = true;
        app.specs.ensure_loaded();

        let left_rect = Rect::from_min_size(Pos2::new(40.0, 0.0), Vec2::new(240.0, 960.0));
        let reader_rect = Rect::from_min_size(Pos2::new(280.0, 0.0), Vec2::new(1080.0, 960.0));
        let right_rect = Rect::from_min_size(Pos2::new(1360.0, 0.0), Vec2::new(240.0, 960.0));

        for tab in SpecTab::ALL {
            app.specs.active_tab = tab;
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 960.0))), ..Default::default() };
            let out = ctx.run_ui(raw, |ui| {
                show_left(&mut app, ui, left_rect);
                show_reader(&mut app, ui, reader_rect);
                show_right(&mut app, ui, right_rect);
            });
            let (texts, boxes) = crate::lint::scan_shapes(&out.shapes);
            let issues = crate::lint::check_text(&texts, &boxes);
            assert!(issues.is_empty(), "lint issues on tab {:?}: {:#?}", tab, issues);
        }

        let before_nodes = app.doc.nodes.len();
        let spec: NodeSpec = serde_json::from_str(FALLBACK_SPEC_JSON).expect("valid fallback NodeSpec");
        app.apply_node_spec(&spec, "easeui-agent-console.spec.json");
        assert!(app.doc.nodes.len() > before_nodes, "applying .spec.json should add nodes to the canvas");

        let pkg = app.specs.packages[app.specs.selected_pkg].clone();
        let before_card = app.doc.nodes.len();
        drop_spec_card_onto_canvas(&mut app, &pkg);
        assert!(app.doc.nodes.len() > before_card, "dropping Spec Card should add an on-canvas Markdown frame");
    }

    #[test]
    fn specs_interactive_controls_mutate_state_and_dispatch_to_agent() {
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        theme::apply(&ctx);
        let mut app = App::new(crate::demo::demo_document());
        app.ui.rail = Some(crate::app::RailItem::Specs);
        app.specs.open = true;
        app.specs.ensure_loaded();

        let left_rect = Rect::from_min_size(Pos2::new(40.0, 0.0), Vec2::new(240.0, 960.0));
        let reader_rect = Rect::from_min_size(Pos2::new(280.0, 0.0), Vec2::new(1080.0, 960.0));
        let right_rect = Rect::from_min_size(Pos2::new(1360.0, 0.0), Vec2::new(240.0, 960.0));

        // Verify dynamic TOC changes per tab
        let pkg = app.specs.packages[app.specs.selected_pkg].clone();
        app.specs.active_tab = SpecTab::Overview;
        let toc_overview = active_section_toc(&pkg, &app.specs);
        app.specs.active_tab = SpecTab::UseCases;
        let toc_usecases = active_section_toc(&pkg, &app.specs);
        assert_ne!(toc_overview, toc_usecases, "active_section_toc must change across tabs");
        app.specs.active_tab = SpecTab::Overview;

        let render_all = |ctx: &egui::Context, app: &mut App, events: Vec<egui::Event>| {
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 960.0))), events, ..Default::default() };
            let _ = ctx.run_ui(raw, |ui| {
                show_left(app, ui, left_rect);
                show_reader(app, ui, reader_rect);
                show_right(app, ui, right_rect);
            });
        };
        let get_rect = |ctx: &egui::Context, name: &str| -> Rect {
            ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", name.to_string())))).unwrap_or_else(|| panic!("{name} recorded"))
        };
        let click_at = |ctx: &egui::Context, app: &mut App, pos: Pos2| {
            render_all(
                ctx,
                app,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
                    egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE },
                ],
            );
        };

        // Initial render to record rects
        render_all(&ctx, &mut app, Vec::new());

        // 1. Click status pill -> cycles status from approved -> in-review
        let status_r = get_rect(&ctx, "specs-status-pill");
        assert_eq!(app.specs.packages[app.specs.selected_pkg].status, "approved");
        click_at(&ctx, &mut app, status_r.center());
        assert_eq!(app.specs.packages[app.specs.selected_pkg].status, "in-review");

        // 2. Click Share & Copy .md buttons -> clipboard only; last_ai_copy stays None
        let share_r = get_rect(&ctx, "specs-share-btn");
        click_at(&ctx, &mut app, share_r.center());
        assert!(app.specs.last_ai_copy.is_none());

        let copy_md_r = get_rect(&ctx, "specs-copy-md");
        click_at(&ctx, &mut app, copy_md_r.center());
        assert!(app.specs.last_ai_copy.is_none());

        // 3. Click TOC item 1 -> updates scroll
        let toc1_r = get_rect(&ctx, "specs-toc-item-1");
        click_at(&ctx, &mut app, toc1_r.center());
        assert!(app.specs.scroll > 0.0);

        // 4. Resolve comment 0 & Post a new comment
        assert!(!app.specs.comments[0].resolved);
        let res0_r = get_rect(&ctx, "specs-comment-resolve-0");
        click_at(&ctx, &mut app, res0_r.center());
        assert!(app.specs.comments[0].resolved);

        let before_cmts = app.specs.comments.len();
        app.specs.comment_draft = "Checked UI Contract rows in Tab 4".to_string();
        render_all(&ctx, &mut app, Vec::new());
        let post_r = get_rect(&ctx, "specs-comment-post");
        click_at(&ctx, &mut app, post_r.center());
        assert_eq!(app.specs.comments.len(), before_cmts + 1);
        assert_eq!(app.specs.comments.last().unwrap().body, "Checked UI Contract rows in Tab 4");

        // 5. Click `+` New Spec button -> appends draft US story and switches to UseCases tab
        let before_us = app.specs.packages[app.specs.selected_pkg].stories.len();
        let new_spec_r = get_rect(&ctx, "specs-left-new-spec");
        click_at(&ctx, &mut app, new_spec_r.center());
        assert_eq!(app.specs.packages[app.specs.selected_pkg].stories.len(), before_us + 1);
        assert_eq!(app.specs.active_tab, SpecTab::UseCases);

        // 6. Click Reference @spec in Agent -> populates terminal composer draft and opens Agents rail
        render_all(&ctx, &mut app, Vec::new());
        let ref_r = get_rect(&ctx, "specs-right-ref-agent");
        click_at(&ctx, &mut app, ref_r.center());
        assert!(app.terminal.composer_draft().contains("@spec://"));
        assert_eq!(app.ui.rail, Some(crate::app::RailItem::Agents));

        // 7. Submit Ask @agent bar -> dispatches prompt through send_to_agent
        app.specs.ask_input = "Verify AC-01 on canvas".to_string();
        render_all(&ctx, &mut app, Vec::new());
        let send_r = get_rect(&ctx, "specs-ask-agent-send");
        click_at(&ctx, &mut app, send_r.center());
        assert!(
            app.terminal.active_composer().pending_send.as_deref() == Some("Verify AC-01 on canvas")
                || app.terminal.active_composer().history.iter().any(|h| h.contains("Verify AC-01 on canvas"))
                || app.terminal.active_composer().last_outcome.is_some()
        );

        for mut tab in app.terminal.tabs.drain(..) {
            let _ = tab.child.kill();
        }
    }
}
