//! In-Canvas `Cmd+K` Agent Command Bar, Prompt-to-`NodeSpec` Synthesizer,
//! and Live Design Gate (`vsf-reviewer`) Audit Engine.
//!
//! Crafted to `es:designer` (`craft-floor.md` + `tokens.css`) and **EaseUI BrainWave 2.0**:
//! - Zero emoji icons (`§2 Refuse by default`): uses crisp Lucide vector glyphs (`crate::icons::Icon`)
//! - No cards-in-cards (`§2`): uses 1px hairlines (`#2A2B31`), spacing steps (`4/8/12/16/24px`),
//!   and surface steps (`#16171B` -> `#1F2024` -> `#26272C`)
//! - Concentric radii (`12px` outer dialog, `6px` inner rows and controls)
//! - Semantic status pills (`OK #4CC58B`, `WARN #E0A53C`, `BAD #FF7A8C`, `INFO #8FB0FF`)

use crate::icons::{self, Icon};
use crate::terminal::{
    AgentCliPreset, STATUS_BAD_BG, STATUS_BAD_BORDER, STATUS_BAD_TEXT, STATUS_INFO_BG, STATUS_INFO_BORDER, STATUS_INFO_TEXT, STATUS_OK_BG, STATUS_OK_BORDER,
    STATUS_OK_TEXT, STATUS_WARN_BG, STATUS_WARN_BORDER, STATUS_WARN_TEXT, TERM_BG_HOVER, TERM_BG_VIEWPORT, TERM_BORDER,
};
use crate::theme;
use crate::widgets;
use designos_doc::{Document, LayoutSpec, NodeId, NodeKind, NodeSpec, PaintKind};
use egui::{Align2, Area, Color32, Context, CornerRadius, Id, Key, Order, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

/// Persistent UI state for the floating `Cmd+K` Agent & Design Gate Bar.
#[derive(Clone, Debug, Default)]
pub struct CmdKState {
    pub open: bool,
    pub query: String,
    pub selected_idx: usize,
    pub active_tab: usize,
    pub last_audit: Option<DesignGateReport>,
    pub status: Option<String>,
    pub focus_requested: bool,
}

/// Summary report produced by `run_design_gate_audit`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesignGateReport {
    pub score: u32,
    pub passed: usize,
    pub warnings: Vec<DesignGateIssue>,
    pub node_count: usize,
}

/// A single Design Gate violation tied to a concrete `NodeId` on the canvas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesignGateIssue {
    pub node_id: NodeId,
    pub node_name: String,
    pub rule: &'static str,
    pub detail: String,
}

/// Text transformation modes for the Figma AI `Rewrite / Translate / Shorten` engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAiMode {
    TranslateVi,
    TranslateEn,
    Shorten,
    Polish,
}

/// Native Figma AI canvas operations executed on `&mut Document`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FigmaAiOp {
    MakePrototype,
    RenameLayers,
    BoostResolution,
    RewriteText(TextAiMode),
    PopulateContent,
}

static PENDING_AI_OP: std::sync::Mutex<Option<FigmaAiOp>> = std::sync::Mutex::new(None);

fn queue_pending_ai_op(op: FigmaAiOp) {
    if let Ok(mut slot) = PENDING_AI_OP.lock() {
        *slot = Some(op);
    }
}

fn take_pending_ai_op() -> Option<FigmaAiOp> {
    PENDING_AI_OP.lock().ok().and_then(|mut slot| slot.take())
}

/// Actions returned by `cmdk::show` for the host `App` / `Shell` to execute.
#[derive(Clone, Debug, PartialEq)]
pub enum CmdKAction {
    ApplySpec(Box<NodeSpec>),
    OpenSpecReader,
    OpenTerminal(AgentCliPreset),
    SendSelectionToTerminal,
    ExportFig,
    SelectNode(NodeId),
    AutoFixDesignGate,
    /// Opens the Check designs panel on the selection (or the page).
    OpenCheckDesigns,
}

#[allow(non_upper_case_globals, non_snake_case)]
impl CmdKAction {
    pub const MakePrototype: CmdKAction = CmdKAction::AutoFixDesignGate;
    pub const RenameLayers: CmdKAction = CmdKAction::AutoFixDesignGate;
    pub const BoostResolution: CmdKAction = CmdKAction::AutoFixDesignGate;
    pub const PopulateContent: CmdKAction = CmdKAction::AutoFixDesignGate;

    pub fn RewriteText(mode: TextAiMode) -> CmdKAction {
        queue_pending_ai_op(FigmaAiOp::RewriteText(mode));
        CmdKAction::AutoFixDesignGate
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
struct QuickCommandItem {
    id: &'static str,
    section: &'static str,
    icon: Icon,
    title: &'static str,
    subtitle: &'static str,
    badge: &'static str,
    shortcut: &'static str,
    tab_mask: u8, // bit 0: All, bit 1: Assets, bit 2: Plugins & widgets, bit 3: Design Gate
    keywords: &'static str,
}

const QUICK_COMMANDS: [QuickCommandItem; 12] = [
    QuickCommandItem {
        id: "cmdk-open-terminal",
        section: "Recents",
        icon: Icon::Dev,
        title: "Ease Design Figma Agent",
        subtitle: "Launches an embedded interactive PTY session with DESIGNOS_BIN & DESIGNOS_LIVE_SPEC injected",
        badge: "Development",
        shortcut: "⌃`",
        tab_mask: 0b0101,
        keywords: "ease design figma agent terminal pty claude codex antigravity agy grok shell cli plugin development",
    },
    QuickCommandItem {
        id: "cmdk-populate-content",
        section: "Recents",
        icon: Icon::Sparkle,
        title: "Replace content",
        subtitle: "Replaces placeholder labels and duplicate card copy with realistic SaaS data",
        badge: "AI",
        shortcut: "⌥⌘D",
        tab_mask: 0b1001,
        keywords: "replace populate content realistic data metrics names status cards table ai",
    },
    QuickCommandItem {
        id: "cmdk-synth-spec",
        section: "Image editing",
        icon: Icon::Image,
        title: "Make an image",
        subtitle: "Synthesizes an EaseUI BrainWave 2.0 NodeSpec subtree with Auto-Layout & $easeui.* tokens",
        badge: "AI",
        shortcut: "↵",
        tab_mask: 0b1011,
        keywords: "make an image make design first draft generate create prompt card modal table pricing login hero sidebar console spec assets",
    },
    QuickCommandItem {
        id: "cmdk-rename-layers",
        section: "Image editing",
        icon: Icon::Sparkle,
        title: "Boost resolution",
        subtitle: "Upgrades image fills to high-DPI Fill, snaps fractional bounds to integers, and sharpens strokes",
        badge: "AI",
        shortcut: "⌥⌘R",
        tab_mask: 0b1001,
        keywords: "boost resolution upscale crisp pixel snap integer bounds sharpen stroke image",
    },
    QuickCommandItem {
        id: "cmdk-translate-vi",
        section: "Image editing",
        icon: Icon::Sparkle,
        title: "Edit image with prompt",
        subtitle: "Rewrites or translates selected copy (VI <-> EN) and applies prompt-driven visual edits",
        badge: "AI",
        shortcut: "⌥⌘T",
        tab_mask: 0b1001,
        keywords: "edit image with prompt translate rewrite shorten polish vietnamese english text copy microcopy",
    },
    QuickCommandItem {
        id: "cmdk-wire-prototype",
        section: "AI & Prototype",
        icon: Icon::Sparkle,
        title: "Make prototype flows",
        subtitle: "Automatically wires OnClick -> NavigateTo SmartAnimate flows between top-level frames",
        badge: "AI",
        shortcut: "⇧⌥P",
        tab_mask: 0b1100,
        keywords: "make prototype auto wire flow navigate smart animate interaction click",
    },
    QuickCommandItem {
        id: "cmdk-semantic-rename",
        section: "AI & Prototype",
        icon: Icon::Sparkle,
        title: "Rename layers semantically",
        subtitle: "Replaces generic Frame / Rectangle / Text names with semantic component labels",
        badge: "AI",
        shortcut: "⇧⌥R",
        tab_mask: 0b1100,
        keywords: "rename layers semantic clean frame rectangle text label button header",
    },
    QuickCommandItem {
        id: "cmdk-design-gate",
        section: "Quality & Audit",
        icon: Icon::Check,
        title: "Run Design Gate Audit (VSF / EaseUI Rules)",
        subtitle: "Audits 4px spacing rhythm, >=32px tap targets, token compliance, and >=11px typography",
        badge: "Audit",
        shortcut: "⌥⌘G",
        tab_mask: 0b1100,
        keywords: "audit gate review vsf lint spacing tap target token readability plugin widget",
    },
    QuickCommandItem {
        id: "cmdk-check-designs",
        section: "Quality & Audit",
        icon: Icon::Check,
        title: "Check designs",
        subtitle: "Finds hard-coded values that match a variable or style, WCAG AA contrast failures and detached instances, each with a one-click fix",
        badge: "Audit",
        shortcut: "",
        tab_mask: 0b1000,
        keywords: "check designs lint hard-coded hardcoded variable style bind contrast wcag detached instance reconnect fix all",
    },
    QuickCommandItem {
        id: "cmdk-send-selection",
        section: "Development",
        icon: Icon::Comment,
        title: "Send Selection & Pin-Comments to Agent Terminal",
        subtitle: "Exports selected frame to /tmp/designos-live-selection.spec.json and pipes into PTY stdin",
        badge: "Development",
        shortcut: "⇧⌘E",
        tab_mask: 0b0100,
        keywords: "send selection comments pin export terminal bridge",
    },
    QuickCommandItem {
        id: "cmdk-open-specs",
        section: "Assets",
        icon: Icon::Specs,
        title: "Open Native Spec Reader (plans/specs/)",
        subtitle: "Inspects User Stories, Gherkin acceptance criteria, and NodeSpec blueprints",
        badge: "Assets",
        shortcut: "⇧⌘S",
        tab_mask: 0b0010,
        keywords: "spec reader plans brainstorm usecases gherkin prd assets",
    },
    QuickCommandItem {
        id: "cmdk-export-fig",
        section: "Assets",
        icon: Icon::Export,
        title: "Save Local Figma .fig Binary Copy",
        subtitle: "Writes a 1:1 round-trip Figma kiwi binary snapshot of the current document",
        badge: "Assets",
        shortcut: "⇧⌘K",
        tab_mask: 0b0010,
        keywords: "save export fig figma binary file assets",
    },
];

const CMDK_BG_SURFACE: Color32 = Color32::from_rgb(0x2C, 0x2C, 0x2C);
const CMDK_BG_ROW_HOVER: Color32 = Color32::from_rgb(0x38, 0x38, 0x38);
const CMDK_BORDER: Color32 = Color32::from_rgb(0x38, 0x38, 0x38);
const CMDK_BADGE_BG: Color32 = Color32::from_rgb(0x38, 0x38, 0x38);
const CMDK_BADGE_TEXT: Color32 = Color32::from_rgb(0xB3, 0xB3, 0xB3);

/// Renders the bottom-docked 1:1 Figma UI3 `Actions (Cmd+K)` modal (`figma_ui3_actions_cmdk.png`) when `state.open` is true.
pub fn show(ctx: &Context, state: &mut CmdKState, doc: &Document, page: NodeId, selection: &[NodeId]) -> Option<CmdKAction> {
    if !state.open {
        return None;
    }

    let mut out_action: Option<CmdKAction> = None;
    let screen = ctx.content_rect();
    let bar_w = 440.0f32.min((screen.width() - 32.0).max(320.0));

    let q_lower = state.query.trim().to_ascii_lowercase();
    let tab_bit = 1u8 << (state.active_tab.min(3));
    let filtered_cmds: Vec<(usize, QuickCommandItem)> = QUICK_COMMANDS
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, item)| {
            if !q_lower.is_empty() {
                item.title.to_ascii_lowercase().contains(&q_lower) || item.keywords.contains(&q_lower) || item.id == "cmdk-synth-spec"
            } else {
                (item.tab_mask & tab_bit) != 0
            }
        })
        .collect();

    if state.selected_idx >= filtered_cmds.len() && !filtered_cmds.is_empty() {
        state.selected_idx = 0;
    }

    let mut trigger_selected = false;
    let mut close_requested = false;
    ctx.input(|i| {
        if i.key_pressed(Key::Escape) {
            close_requested = true;
        }
        if i.key_pressed(Key::ArrowDown) && !filtered_cmds.is_empty() {
            state.selected_idx = (state.selected_idx + 1) % filtered_cmds.len();
        }
        if i.key_pressed(Key::ArrowUp) && !filtered_cmds.is_empty() {
            state.selected_idx = if state.selected_idx == 0 { filtered_cmds.len() - 1 } else { state.selected_idx - 1 };
        }
        if i.key_pressed(Key::Enter) {
            trigger_selected = true;
        }
    });

    if close_requested {
        state.open = false;
        return None;
    }

    // Count distinct section headers that will be rendered
    let mut section_count = 0usize;
    let mut prev_sec = "";
    for (_, item) in &filtered_cmds {
        let sec = if q_lower.is_empty() { item.section } else { "Matching actions" };
        if sec != prev_sec {
            section_count += 1;
            prev_sec = sec;
        }
    }

    if state.active_tab == 3 && state.last_audit.is_none() {
        state.last_audit = Some(run_design_gate_audit(doc, page, selection));
    }
    let show_audit = state.active_tab == 3;
    let audit_rows = state.last_audit.as_ref().map(|a| a.warnings.len().min(4)).unwrap_or(0);
    let audit_h = if show_audit && state.last_audit.is_some() { 48.0 + audit_rows as f32 * 26.0 } else { 0.0 };
    // Search row (40) + Tab bar (34) + Section headers (28 each) + Rows (32 each) + audit + bottom pad (10)
    let bar_h = 40.0 + 34.0 + (section_count as f32 * 28.0) + (filtered_cmds.len() as f32 * 32.0) + audit_h + 10.0;
    // Bottom-docked right above the bottom floating toolbar (`screen.bottom() - 68.0`)
    let bar_pos = Pos2::new((screen.center().x - bar_w / 2.0).max(16.0), (screen.bottom() - 68.0 - bar_h).max(20.0));

    Area::new(Id::new("designos-cmdk-agent-bar")).order(Order::Foreground).fixed_pos(bar_pos).show(ctx, |ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(bar_w, bar_h), Sense::click());
        widgets::record(ui, "cmdk-modal", rect);
        let p = ui.painter().clone();

        // Outer modal: 12px radius + 1px `#383838` border + deep elevation shadow
        widgets::elevation_400(&p, rect, theme::RADIUS_FRAME);
        p.rect_filled(rect, theme::cr(theme::RADIUS_FRAME), CMDK_BG_SURFACE);
        p.rect_stroke(rect, theme::cr(theme::RADIUS_FRAME), Stroke::new(1.0, CMDK_BORDER), StrokeKind::Inside);

        // 1. Top Search Row (40px) — 1:1 `figma_ui3_actions_cmdk.png`:
        // Left search icon, clean borderless input (`Search`), right `Icon::Image` visual-search button.
        let input_row = Rect::from_min_size(rect.min, Vec2::new(bar_w, 40.0));
        widgets::hline(&p, input_row.left(), input_row.right(), input_row.bottom() - 1.0, CMDK_BORDER);

        icons::paint(&p, Icon::Search, Pos2::new(input_row.left() + 18.0, input_row.center().y), 15.0, theme::ICON_SECONDARY);

        let img_btn_r = Rect::from_center_size(Pos2::new(input_row.right() - 20.0, input_row.center().y), Vec2::splat(26.0));
        widgets::record(ui, "cmdk-image-search-btn", img_btn_r);
        let img_resp = ui.interact(img_btn_r, Id::new("cmdk-image-search-btn"), Sense::click());
        if img_resp.hovered() {
            p.rect_filled(img_btn_r, CornerRadius::same(6), CMDK_BG_ROW_HOVER);
        }
        icons::paint(&p, Icon::Image, img_btn_r.center(), 14.0, theme::ICON_SECONDARY);
        if img_resp.on_hover_text("Search with image or generate visual draft").clicked() {
            let prompt = if state.query.trim().is_empty() { "pricing card" } else { state.query.trim() };
            let spec = synthesize_spec_from_prompt(prompt);
            state.status = Some(format!("Synthesized '{}' on Canvas", spec.name));
            state.open = false;
            out_action = Some(CmdKAction::ApplySpec(Box::new(spec)));
        }

        let edit_rect =
            Rect::from_min_max(Pos2::new(input_row.left() + 34.0, input_row.top() + 8.0), Pos2::new(img_btn_r.left() - 8.0, input_row.bottom() - 8.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(edit_rect), |ui| {
            let te = egui::TextEdit::singleline(&mut state.query)
                .frame(egui::Frame::NONE)
                .font(theme::font_body())
                .text_color(theme::TEXT)
                .hint_text("Search actions or type layout prompt (↵ Local Spec · ⌃` Live Agent CLI)")
                .desired_width(edit_rect.width());
            let resp = ui.add(te);
            if state.open && (state.focus_requested || !resp.has_focus()) {
                resp.request_focus();
                state.focus_requested = false;
            }
        });

        // 2. Tab Bar (`All`, `Assets`, `Plugins & widgets`, `Design Gate`) — 34px height, 2px white underline on active tab
        let tab_row = Rect::from_min_size(Pos2::new(rect.left(), input_row.bottom()), Vec2::new(bar_w, 34.0));
        widgets::hline(&p, tab_row.left(), tab_row.right(), tab_row.bottom() - 1.0, CMDK_BORDER);
        let mut tx = tab_row.left() + 14.0;
        let tab_tips = [
            "All actions: local layout synthesis (↵) + live Agent PTY CLI (⌃`)",
            "Assets & Specs: local NodeSpec synthesis, Spec Reader, and .fig binary export",
            "Plugins, Widgets & Agent CLI: interactive PTY session + canvas bridge",
            "Design Gate: live VSF / EaseUI audit & 1-click auto-fix",
        ];
        for (t_idx, t_label) in ["All", "Assets", "Plugins & widgets", "Design Gate"].iter().enumerate() {
            let tw = widgets::text_width(&p, t_label, &theme::font_body_strong());
            let tr = Rect::from_min_size(Pos2::new(tx, tab_row.top()), Vec2::new(tw + 8.0, tab_row.height()));
            let tresp = ui.interact(tr, Id::new(("cmdk-tab", t_idx)), Sense::click()).on_hover_text(tab_tips[t_idx]);
            let is_active = state.active_tab == t_idx;
            p.text(
                tr.center(),
                Align2::CENTER_CENTER,
                *t_label,
                if is_active { theme::font_body_strong() } else { theme::font_body() },
                if is_active {
                    Color32::WHITE
                } else if tresp.hovered() {
                    theme::TEXT
                } else {
                    theme::TEXT_SECONDARY
                },
            );
            if is_active {
                let underline = Rect::from_min_max(Pos2::new(tr.left() + 2.0, tab_row.bottom() - 2.0), Pos2::new(tr.right() - 2.0, tab_row.bottom()));
                p.rect_filled(underline, 0.0, Color32::WHITE);
            }
            if tresp.clicked() {
                state.active_tab = t_idx;
                state.selected_idx = 0;
                if t_idx == 3 && state.last_audit.is_none() {
                    state.last_audit = Some(run_design_gate_audit(doc, page, selection));
                }
            }
            tx = tr.right() + 12.0;
        }

        // 3. Grouped Sections (`Recents`, `Image editing`, etc.) + Command Rows (32px height)
        let mut row_y = tab_row.bottom() + 4.0;
        let mut current_sec = "";
        for (list_idx, (_, item)) in filtered_cmds.iter().enumerate() {
            let sec = if q_lower.is_empty() { item.section } else { "Matching actions" };
            if sec != current_sec {
                current_sec = sec;
                p.text(Pos2::new(rect.left() + 16.0, row_y + 15.0), Align2::LEFT_CENTER, sec, theme::font_caption(), theme::TEXT_SECONDARY);
                row_y += 28.0;
            }

            let row_rect = Rect::from_min_size(Pos2::new(rect.left() + 8.0, row_y), Vec2::new(bar_w - 16.0, 30.0));
            widgets::record(ui, item.id, row_rect);
            let resp = ui.interact(row_rect, Id::new(item.id), Sense::click()).on_hover_text(item.subtitle);
            if resp.hovered() {
                state.selected_idx = list_idx;
            }
            let is_sel = list_idx == state.selected_idx;
            if is_sel {
                p.rect_filled(row_rect, CornerRadius::same(6), CMDK_BG_ROW_HOVER);
            }

            icons::paint(
                &p,
                item.icon,
                Pos2::new(row_rect.left() + 14.0, row_rect.center().y),
                14.0,
                if is_sel { Color32::WHITE } else { theme::ICON_SECONDARY },
            );

            // Right-aligned rounded badge (`Development` or `AI` in `#383838` pill with `#B3B3B3` text)
            let badge_w = widgets::text_width(&p, item.badge, &theme::font_caption()) + 12.0;
            let badge_r = Rect::from_min_size(Pos2::new(row_rect.right() - badge_w - 8.0, row_rect.center().y - 9.0), Vec2::new(badge_w, 18.0));
            p.rect_filled(badge_r, CornerRadius::same(4), CMDK_BADGE_BG);
            p.text(badge_r.center(), Align2::CENTER_CENTER, item.badge, theme::font_caption(), CMDK_BADGE_TEXT);

            let title_str = widgets::elide(&p, item.title, &theme::font_body(), (badge_r.left() - row_rect.left() - 40.0).max(60.0));
            p.text(Pos2::new(row_rect.left() + 32.0, row_rect.center().y), Align2::LEFT_CENTER, title_str, theme::font_body(), Color32::WHITE);

            let activated = resp.clicked() || (trigger_selected && is_sel);
            if activated {
                trigger_selected = false;
                match item.id {
                    "cmdk-synth-spec" => {
                        let prompt = if state.query.trim().is_empty() { "pricing card" } else { state.query.trim() };
                        let spec = synthesize_spec_from_prompt(prompt);
                        state.status = Some(format!("Synthesized '{}' (Instant Local NodeSpec)", spec.name));
                        state.open = false;
                        out_action = Some(CmdKAction::ApplySpec(Box::new(spec)));
                    }
                    "cmdk-wire-prototype" => {
                        queue_pending_ai_op(FigmaAiOp::MakePrototype);
                        state.status = Some("Wired SmartAnimate prototype flows across frames".into());
                        state.open = false;
                        out_action = Some(CmdKAction::MakePrototype);
                    }
                    "cmdk-rename-layers" => {
                        queue_pending_ai_op(FigmaAiOp::BoostResolution);
                        state.status = Some("Boosted resolution, snapped integer bounds & renamed layers".into());
                        state.open = false;
                        out_action = Some(CmdKAction::BoostResolution);
                    }
                    "cmdk-semantic-rename" => {
                        queue_pending_ai_op(FigmaAiOp::RenameLayers);
                        state.status = Some("Renamed generic layers semantically".into());
                        state.open = false;
                        out_action = Some(CmdKAction::RenameLayers);
                    }
                    "cmdk-translate-vi" => {
                        let mode = if q_lower.contains("en") {
                            TextAiMode::TranslateEn
                        } else if q_lower.contains("short") {
                            TextAiMode::Shorten
                        } else if q_lower.contains("polish") {
                            TextAiMode::Polish
                        } else {
                            TextAiMode::TranslateVi
                        };
                        state.status = Some(format!("Applied {:?} across selected layers", mode));
                        state.open = false;
                        out_action = Some(CmdKAction::RewriteText(mode));
                    }
                    "cmdk-populate-content" => {
                        queue_pending_ai_op(FigmaAiOp::PopulateContent);
                        state.status = Some("Populated realistic SaaS content & metrics".into());
                        state.open = false;
                        out_action = Some(CmdKAction::PopulateContent);
                    }
                    "cmdk-design-gate" => {
                        let report = run_design_gate_audit(doc, page, selection);
                        state.status =
                            Some(format!("Design Gate Score: {}/100 ({} warnings across {} nodes)", report.score, report.warnings.len(), report.node_count));
                        state.last_audit = Some(report);
                    }
                    "cmdk-check-designs" => {
                        state.open = false;
                        out_action = Some(CmdKAction::OpenCheckDesigns);
                    }
                    "cmdk-open-terminal" => {
                        state.status = Some("Opened live interactive Agent PTY CLI".into());
                        state.open = false;
                        out_action = Some(CmdKAction::OpenTerminal(AgentCliPreset::Claude));
                    }
                    "cmdk-send-selection" => {
                        state.open = false;
                        out_action = Some(CmdKAction::SendSelectionToTerminal);
                    }
                    "cmdk-open-specs" => {
                        state.open = false;
                        out_action = Some(CmdKAction::OpenSpecReader);
                    }
                    "cmdk-export-fig" => {
                        state.open = false;
                        out_action = Some(CmdKAction::ExportFig);
                    }
                    _ => {}
                }
            }
            row_y += 32.0;
        }

        // 4. Live Design Gate Audit Section (when Design Gate tab is active or an audit was run)
        if show_audit {
            if let Some(audit) = state.last_audit.clone() {
                render_audit_section(ui, &p, rect, &mut row_y, &audit, state, &mut out_action);
            }
        }
    });

    out_action
}

fn render_audit_section(
    ui: &mut egui::Ui,
    p: &Painter,
    bar_rect: Rect,
    row_y: &mut f32,
    audit: &DesignGateReport,
    state: &mut CmdKState,
    out_action: &mut Option<CmdKAction>,
) {
    let shown_issues = audit.warnings.len().min(4);
    let sec_h = 40.0 + shown_issues as f32 * 26.0;
    let sec_rect = Rect::from_min_size(Pos2::new(bar_rect.left(), *row_y + 2.0), Vec2::new(bar_rect.width(), sec_h));
    p.rect_filled(sec_rect, 0.0, TERM_BG_VIEWPORT);
    widgets::hline(p, sec_rect.left(), sec_rect.right(), sec_rect.top(), TERM_BORDER);

    // Semantic status pill (`tokens.css`: OK / WARN / BAD)
    let (score_fg, score_bg, score_border) = if audit.score >= 90 {
        (STATUS_OK_TEXT, STATUS_OK_BG, STATUS_OK_BORDER)
    } else if audit.score >= 70 {
        (STATUS_WARN_TEXT, STATUS_WARN_BG, STATUS_WARN_BORDER)
    } else {
        (STATUS_BAD_TEXT, STATUS_BAD_BG, STATUS_BAD_BORDER)
    };
    let score_badge = Rect::from_min_size(Pos2::new(sec_rect.left() + 12.0, sec_rect.top() + 8.0), Vec2::new(86.0, 22.0));
    p.rect_filled(score_badge, CornerRadius::same(6), score_bg);
    p.rect_stroke(score_badge, CornerRadius::same(6), Stroke::new(1.0, score_border), StrokeKind::Inside);
    p.text(score_badge.center(), Align2::CENTER_CENTER, format!("SCORE {}/100", audit.score), theme::font_caption_strong(), score_fg);

    let summary_txt = format!("{} passed • {} warnings ({} nodes audited)", audit.passed, audit.warnings.len(), audit.node_count);
    let elided_sum = widgets::elide(p, &summary_txt, &theme::font_caption_medium(), (sec_rect.width() - 320.0).max(60.0));
    p.text(Pos2::new(score_badge.right() + 10.0, score_badge.center().y), Align2::LEFT_CENTER, elided_sum, theme::font_caption_medium(), theme::TEXT_SECONDARY);

    // 1-Click "Auto-Fix Spacing & Tap Targets" button with Sparkle vector icon
    let fix_w = 198.0;
    let fix_rect = Rect::from_min_size(Pos2::new(sec_rect.right() - fix_w - 12.0, sec_rect.top() + 8.0), Vec2::new(fix_w, 22.0));
    let fix_resp = ui.interact(fix_rect, Id::new("cmdk-autofix-gate-btn"), Sense::click());
    p.rect_filled(fix_rect, CornerRadius::same(6), if fix_resp.hovered() { TERM_BG_HOVER } else { STATUS_WARN_BG });
    p.rect_stroke(fix_rect, CornerRadius::same(6), Stroke::new(1.0, STATUS_WARN_BORDER), StrokeKind::Inside);
    icons::paint(p, Icon::Sparkle, Pos2::new(fix_rect.left() + 11.0, fix_rect.center().y), 11.0, STATUS_WARN_TEXT);
    p.text(
        Pos2::new(fix_rect.left() + 20.0, fix_rect.center().y),
        Align2::LEFT_CENTER,
        "Auto-Fix Spacing & Tap Targets",
        theme::font_caption_strong(),
        STATUS_WARN_TEXT,
    );
    if fix_resp.clicked() {
        state.status = Some("Applied 4px grid & >=32px tap target auto-fixes".into());
        *out_action = Some(CmdKAction::AutoFixDesignGate);
    }

    let mut iy = sec_rect.top() + 36.0;
    for (idx, issue) in audit.warnings.iter().take(4).enumerate() {
        let focus_r = Rect::from_min_size(Pos2::new(sec_rect.right() - 58.0, iy), Vec2::new(46.0, 20.0));
        let focus_resp = ui.interact(focus_r, Id::new(("cmdk-issue-focus", idx, issue.node_id.0)), Sense::click());
        p.rect_filled(focus_r, CornerRadius::same(4), if focus_resp.hovered() { TERM_BG_HOVER } else { STATUS_INFO_BG });
        p.rect_stroke(focus_r, CornerRadius::same(4), Stroke::new(1.0, STATUS_INFO_BORDER), StrokeKind::Inside);
        p.text(focus_r.center(), Align2::CENTER_CENTER, "Focus", theme::font_caption_medium(), STATUS_INFO_TEXT);
        if focus_resp.clicked() {
            state.open = false;
            *out_action = Some(CmdKAction::SelectNode(issue.node_id));
        }

        let line = format!("[{}] {}: {}", issue.rule, issue.node_name, issue.detail);
        let elided_line = widgets::elide(p, &line, &theme::font_caption(), (sec_rect.width() - 80.0).max(60.0));
        p.text(Pos2::new(sec_rect.left() + 12.0, iy + 10.0), Align2::LEFT_CENTER, elided_line, theme::font_caption(), theme::TEXT);
        iy += 26.0;
    }

    *row_y += sec_h + 4.0;
}

/// Synthesizes a multi-node EaseUI BrainWave 2.0 `NodeSpec` subtree from a natural-language prompt.
pub fn synthesize_spec_from_prompt(prompt: &str) -> NodeSpec {
    let raw = prompt.trim();
    let lower = raw.to_ascii_lowercase();

    if lower.contains("pricing") || lower.contains("plan") || lower.contains("billing") {
        synthesize_pricing_card(raw)
    } else if lower.contains("login") || lower.contains("auth") || lower.contains("sign in") || lower.contains("modal") {
        synthesize_login_modal(raw)
    } else if lower.contains("table") || lower.contains("audit") || lower.contains("log") || lower.contains("grid") {
        synthesize_audit_table(raw)
    } else if lower.contains("console") || lower.contains("terminal") || lower.contains("agent") || lower.contains("telemetry") {
        synthesize_agent_console(raw)
    } else if lower.contains("sidebar") || lower.contains("nav") || lower.contains("menu") {
        synthesize_sidebar_nav(raw)
    } else if lower.contains("hero") || lower.contains("banner") || lower.contains("landing") {
        synthesize_hero_banner(raw)
    } else {
        synthesize_custom_easeui_card(raw)
    }
}

fn text_node(name: &str, text: &str, size: f32, weight: u16, color: &str) -> NodeSpec {
    NodeSpec {
        kind: "text".into(),
        name: name.into(),
        text: Some(text.into()),
        font_size: Some(size),
        font_weight: Some(weight),
        text_color: Some(color.into()),
        ..Default::default()
    }
}

fn badge_node(name: &str, label: &str, fill: &str, text_color: &str) -> NodeSpec {
    NodeSpec {
        kind: "frame".into(),
        name: name.into(),
        preset: Some("easeui-badge".into()),
        fill: Some(fill.into()),
        text: Some(label.into()),
        text_color: Some(text_color.into()),
        ..Default::default()
    }
}

fn button_node(name: &str, label: &str, primary: bool) -> NodeSpec {
    NodeSpec {
        kind: "frame".into(),
        name: name.into(),
        preset: Some(if primary { "easeui-button-primary" } else { "easeui-button-secondary" }.into()),
        min_height: Some(36.0),
        text: Some(label.into()),
        ..Default::default()
    }
}

fn synthesize_pricing_card(_prompt: &str) -> NodeSpec {
    NodeSpec {
        kind: "frame".into(),
        name: "EaseUI Pro Pricing Card".into(),
        preset: Some("easeui-card".into()),
        width: Some(380.0),
        fill: Some("$easeui.bg.panel".into()),
        stroke: Some("$easeui.accent.orange".into()),
        stroke_weight: Some(1.5),
        corner_radius: Some(16.0),
        dev_status: Some("ready".into()),
        layout: Some(LayoutSpec { direction: "column".into(), gap: 16.0, padding: [24.0, 24.0, 24.0, 24.0], ..Default::default() }),
        children: vec![
            badge_node("Tier Badge", "MOST POPULAR • BRAINWAVE 2.0", "$easeui.accent.orange", "$easeui.text.primary"),
            text_node("Plan Title", "Studio Autonomous Pro", 22.0, 700, "$easeui.text.primary"),
            text_node("Plan Price", "$49 / month • Unlimited Agent Specs", 15.0, 600, "$easeui.accent.orange"),
            text_node("Feature 1", "Native Embedded PTY (Claude Code, Codex, Grok)", 13.0, 500, "$easeui.text.secondary"),
            text_node("Feature 2", "2D CSS Grid + Auto-Layout 5.0 Subtree Solver", 13.0, 500, "$easeui.text.secondary"),
            text_node("Feature 3", "1:1 Figma .fig Binary Round-Trip + React/Tailwind Export", 13.0, 500, "$easeui.text.secondary"),
            button_node("Upgrade CTA Button", "Upgrade to Studio Pro ->", true),
        ],
        ..Default::default()
    }
}

fn synthesize_login_modal(_prompt: &str) -> NodeSpec {
    NodeSpec {
        kind: "frame".into(),
        name: "EaseUI Auth Sign-In Modal".into(),
        preset: Some("easeui-card".into()),
        width: Some(420.0),
        fill: Some("$easeui.bg.panel".into()),
        stroke: Some("$easeui.border.subtle".into()),
        corner_radius: Some(16.0),
        dev_status: Some("ready".into()),
        layout: Some(LayoutSpec { direction: "column".into(), gap: 16.0, padding: [28.0, 28.0, 28.0, 28.0], ..Default::default() }),
        children: vec![
            badge_node("Auth Badge", "SSO • ZERO-TRUST WORKSPACE", "$easeui.accent.blue", "$easeui.text.primary"),
            text_node("Modal Heading", "Sign in to design:os studio", 20.0, 700, "$easeui.text.primary"),
            text_node("Modal Subtitle", "Connect your Claude Code, Codex, and Figma workspace credentials.", 13.0, 400, "$easeui.text.secondary"),
            NodeSpec {
                kind: "frame".into(),
                name: "Email Input Field".into(),
                preset: Some("easeui-input".into()),
                width_sizing: Some("fill".into()),
                min_height: Some(40.0),
                text: Some("jang@easestart.dev".into()),
                ..Default::default()
            },
            NodeSpec {
                kind: "frame".into(),
                name: "API Token Input Field".into(),
                preset: Some("easeui-input".into()),
                width_sizing: Some("fill".into()),
                min_height: Some(40.0),
                text: Some("••••••••••••••••••••••••".into()),
                ..Default::default()
            },
            button_node("Sign In Primary Button", "Continue with Agent Key ->", true),
        ],
        ..Default::default()
    }
}

fn synthesize_audit_table(_prompt: &str) -> NodeSpec {
    NodeSpec {
        kind: "frame".into(),
        name: "EaseUI Design Gate Audit Table".into(),
        preset: Some("easeui-panel".into()),
        width: Some(680.0),
        fill: Some("$easeui.bg.panel".into()),
        stroke: Some("$easeui.border".into()),
        corner_radius: Some(12.0),
        dev_status: Some("ready".into()),
        layout: Some(LayoutSpec { direction: "column".into(), gap: 12.0, padding: [20.0, 20.0, 20.0, 20.0], ..Default::default() }),
        children: vec![
            badge_node("Gate Status Badge", "VSF GATE G4 • 98/100 PASS", "$easeui.status.success", "$easeui.text.primary"),
            text_node("Table Title", "Live Agent & Design Gate Telemetry Log", 18.0, 700, "$easeui.text.primary"),
            NodeSpec {
                kind: "frame".into(),
                name: "Audit Row 1".into(),
                preset: Some("easeui-row".into()),
                width_sizing: Some("fill".into()),
                fill: Some("$easeui.bg.surface".into()),
                corner_radius: Some(8.0),
                layout: Some(LayoutSpec { direction: "row".into(), gap: 16.0, padding: [12.0, 16.0, 12.0, 16.0], ..Default::default() }),
                children: vec![
                    badge_node("Pass Pill 1", "PASS", "$easeui.status.success", "$easeui.text.primary"),
                    text_node("Rule 1", "4px Spacing Rhythm & Auto-Layout 5.0 Grid", 13.0, 600, "$easeui.text.primary"),
                ],
                ..Default::default()
            },
            NodeSpec {
                kind: "frame".into(),
                name: "Audit Row 2".into(),
                preset: Some("easeui-row".into()),
                width_sizing: Some("fill".into()),
                fill: Some("$easeui.bg.surface".into()),
                corner_radius: Some(8.0),
                layout: Some(LayoutSpec { direction: "row".into(), gap: 16.0, padding: [12.0, 16.0, 12.0, 16.0], ..Default::default() }),
                children: vec![
                    badge_node("Pass Pill 2", "PASS", "$easeui.accent.blue", "$easeui.text.primary"),
                    text_node("Rule 2", "100% $easeui.* Semantic Token Binding Coverage", 13.0, 600, "$easeui.text.primary"),
                ],
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}

fn synthesize_agent_console(_prompt: &str) -> NodeSpec {
    NodeSpec {
        kind: "frame".into(),
        name: "EaseUI Agent Console Panel".into(),
        preset: Some("easeui-panel".into()),
        width: Some(640.0),
        fill: Some("$easeui.bg.panel".into()),
        stroke: Some("$easeui.accent.violet".into()),
        corner_radius: Some(12.0),
        dev_status: Some("ready".into()),
        layout: Some(LayoutSpec { direction: "column".into(), gap: 12.0, padding: [20.0, 20.0, 20.0, 20.0], ..Default::default() }),
        children: vec![
            badge_node("Agent Runtime Badge", "CLAUDE + CODEX + GROK PTY", "$easeui.accent.violet", "$easeui.text.primary"),
            text_node("Console Header", "Autonomous Design-to-Code Agent Bridge", 18.0, 700, "$easeui.text.primary"),
            text_node(
                "Console Stream",
                "$ designos --apply-spec examples/specs/easeui-agent-console.spec.json --export-code react",
                13.0,
                500,
                "$easeui.accent.emerald",
            ),
            button_node("Run Agent Task Button", "Sync Selection to Active PTY", true),
        ],
        ..Default::default()
    }
}

fn synthesize_sidebar_nav(_prompt: &str) -> NodeSpec {
    NodeSpec {
        kind: "frame".into(),
        name: "EaseUI Sidebar Navigation".into(),
        preset: Some("easeui-panel".into()),
        width: Some(260.0),
        fill: Some("$easeui.bg.panel".into()),
        stroke: Some("$easeui.border".into()),
        corner_radius: Some(12.0),
        dev_status: Some("ready".into()),
        layout: Some(LayoutSpec { direction: "column".into(), gap: 8.0, padding: [16.0, 16.0, 16.0, 16.0], ..Default::default() }),
        children: vec![
            badge_node("Workspace Pill", "design:os studio", "$easeui.accent.orange", "$easeui.text.primary"),
            button_node("Nav Item Canvas", "Canvas & Layers", true),
            button_node("Nav Item Specs", "Specs & Gherkin Hub", false),
            button_node("Nav Item Terminal", "Embedded Agent PTY", false),
            button_node("Nav Item Tokens", "EaseUI Token Variables", false),
        ],
        ..Default::default()
    }
}

fn synthesize_hero_banner(_prompt: &str) -> NodeSpec {
    NodeSpec {
        kind: "frame".into(),
        name: "EaseUI BrainWave 2.0 Hero Banner".into(),
        preset: Some("easeui-card".into()),
        width: Some(840.0),
        fill: Some("$easeui.bg.panel".into()),
        stroke: Some("$easeui.accent.orange".into()),
        corner_radius: Some(16.0),
        dev_status: Some("ready".into()),
        layout: Some(LayoutSpec { direction: "column".into(), gap: 16.0, padding: [32.0, 32.0, 32.0, 32.0], ..Default::default() }),
        children: vec![
            badge_node("Hero Tag", "AGENT-NATIVE VECTOR & PRODUCT STUDIO", "$easeui.accent.orange", "$easeui.text.primary"),
            text_node("Hero Headline", "Design, Audit, and Ship Production UI with Live Agent PTYs", 28.0, 700, "$easeui.text.primary"),
            text_node(
                "Hero Description",
                "Bidirectional NodeSpec blueprints, 2D CSS Grid layout, and 1:1 Figma .fig binary compatibility.",
                15.0,
                400,
                "$easeui.text.secondary",
            ),
            button_node("Hero Primary CTA", "Launch Agent Workspace ->", true),
        ],
        ..Default::default()
    }
}

fn synthesize_custom_easeui_card(prompt: &str) -> NodeSpec {
    let title = if prompt.is_empty() {
        "EaseUI Generated Component".to_string()
    } else {
        let mut chars = prompt.chars();
        match chars.next() {
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            None => "EaseUI Generated Component".to_string(),
        }
    };
    NodeSpec {
        kind: "frame".into(),
        name: format!("EaseUI — {title}"),
        preset: Some("easeui-card".into()),
        width: Some(420.0),
        fill: Some("$easeui.bg.panel".into()),
        stroke: Some("$easeui.border.subtle".into()),
        corner_radius: Some(12.0),
        dev_status: Some("ready".into()),
        layout: Some(LayoutSpec { direction: "column".into(), gap: 12.0, padding: [20.0, 20.0, 20.0, 20.0], ..Default::default() }),
        children: vec![
            badge_node("Spec Badge", "CMD+K SYNTHESIZED • EASEUI 2.0", "$easeui.accent.orange", "$easeui.text.primary"),
            text_node("Card Title", &title, 18.0, 700, "$easeui.text.primary"),
            text_node("Card Subtitle", "Declarative Auto-Layout 5.0 frame bound to $easeui.* semantic tokens.", 13.0, 400, "$easeui.text.secondary"),
            button_node("Primary Action Button", "Inspect & Export Code", true),
        ],
        ..Default::default()
    }
}

/// Audits the active selection (or the entire `page` subtree when `selection` is empty)
/// against VSF / EaseUI BrainWave 2.0 design rules:
/// 1. **Spacing Rhythm (4px grid)**: `AutoLayout::item_spacing` and `padding` multiples of 4px.
/// 2. **Tap Target (`>= 32px`)**: interactive frames/buttons/instances at least `32px` wide & tall.
/// 3. **Token Compliance**: solid fills/strokes bound to variables or standard EaseUI palette colors.
/// 4. **Text Readability (`>= 11px`)**: `TextStyle::font_size >= 11.0`.
pub fn run_design_gate_audit(doc: &Document, page: NodeId, selection: &[NodeId]) -> DesignGateReport {
    let roots: Vec<NodeId> = if !selection.is_empty() { selection.to_vec() } else { doc.get(page).map(|pg| pg.children.clone()).unwrap_or_default() };

    let mut visited = Vec::new();
    for r in roots {
        collect_subtree(doc, r, &mut visited);
    }

    let mut passed = 0usize;
    let mut warnings = Vec::new();

    for &nid in &visited {
        let Some(node) = doc.get(nid) else { continue };
        let mut node_ok = true;

        // 1. Spacing Rhythm (4px grid)
        if let Some(fp) = node.kind.frame_props() {
            if let Some(al) = &fp.auto_layout {
                if !is_multiple_of_4(al.item_spacing) {
                    node_ok = false;
                    warnings.push(DesignGateIssue {
                        node_id: nid,
                        node_name: node.name.clone(),
                        rule: "Spacing Rhythm (4px)",
                        detail: format!("item_spacing {:.1}px is off the 4px grid", al.item_spacing),
                    });
                }
                for (side_name, pad_val) in [("top", al.padding[0]), ("right", al.padding[1]), ("bottom", al.padding[2]), ("left", al.padding[3])] {
                    if !is_multiple_of_4(pad_val) {
                        node_ok = false;
                        warnings.push(DesignGateIssue {
                            node_id: nid,
                            node_name: node.name.clone(),
                            rule: "Spacing Rhythm (4px)",
                            detail: format!("padding-{side_name} {pad_val:.1}px is off the 4px grid"),
                        });
                        break;
                    }
                }
            }
        }

        // 2. Tap Target (>= 32px) for interactive frames/buttons/instances
        if is_interactive_candidate(node) && (node.size.x < 32.0 || node.size.y < 32.0) {
            node_ok = false;
            warnings.push(DesignGateIssue {
                node_id: nid,
                node_name: node.name.clone(),
                rule: "Tap Target (>=32px)",
                detail: format!("interactive size {:.0}x{:.0}px < 32px minimum", node.size.x, node.size.y),
            });
        }

        // 3. Token Compliance
        if !matches!(node.kind, NodeKind::Page { .. } | NodeKind::Text(_)) {
            let has_solid_fill = node.fills.iter().any(|p| p.visible && matches!(p.kind, PaintKind::Solid(_)));
            if has_solid_fill && node.variables.bindings.is_empty() && !uses_easeui_standard_palette(node) {
                node_ok = false;
                warnings.push(DesignGateIssue {
                    node_id: nid,
                    node_name: node.name.clone(),
                    rule: "Token Compliance",
                    detail: "unbound raw hex fill (bind a $easeui.* variable token)".into(),
                });
            }
        }

        // 4. Text Readability (>= 11px)
        if let Some(tp) = node.kind.any_text() {
            if tp.style.font_size < 11.0 {
                node_ok = false;
                warnings.push(DesignGateIssue {
                    node_id: nid,
                    node_name: node.name.clone(),
                    rule: "Text Readability (>=11px)",
                    detail: format!("font_size {:.1}px is below 11px floor", tp.style.font_size),
                });
            }
        }

        if node_ok {
            passed += 1;
        }
    }

    let node_count = visited.len();
    let penalty = (warnings.len() as u32).saturating_mul(7);
    let score = if node_count == 0 { 100 } else { 100u32.saturating_sub(penalty).max(15) };

    DesignGateReport { score: if warnings.is_empty() { 100 } else { score }, passed, warnings, node_count }
}

/// Auto-fixes spacing rhythm (snaps `item_spacing` and `padding` to nearest 4px multiple),
/// enlarges interactive buttons/inputs `<32px` up to `32px`, and raises `<11px` text to `11px`
/// (or executes a queued `FigmaAiOp` when triggered from a Figma AI `CmdKAction`).
/// Returns the number of modified nodes.
pub fn auto_fix_design_gate(doc: &mut Document, page: NodeId, selection: &[NodeId]) -> usize {
    if let Some(op) = take_pending_ai_op() {
        return match op {
            FigmaAiOp::MakePrototype => auto_wire_prototype_flows(doc, page, selection),
            FigmaAiOp::RenameLayers => auto_rename_layers_semantically(doc, page, selection),
            FigmaAiOp::RewriteText(mode) => rewrite_selected_text(doc, page, selection, mode),
            FigmaAiOp::PopulateContent => populate_realistic_content(doc, page, selection),
            FigmaAiOp::BoostResolution => boost_resolution_on_selection(doc, page, selection),
        };
    }

    let roots: Vec<NodeId> = if !selection.is_empty() { selection.to_vec() } else { doc.get(page).map(|pg| pg.children.clone()).unwrap_or_default() };

    let mut visited = Vec::new();
    for r in roots {
        collect_subtree(doc, r, &mut visited);
    }

    let mut fixed_count = 0usize;
    let mut relayout_roots = Vec::new();

    for &nid in &visited {
        let Some(node) = doc.get_mut(nid) else {
            continue;
        };
        let mut changed = false;

        if let Some(fp) = node.kind.frame_props_mut() {
            if let Some(al) = &mut fp.auto_layout {
                if !is_multiple_of_4(al.item_spacing) {
                    al.item_spacing = snap_to_4px(al.item_spacing);
                    changed = true;
                }
                for p in &mut al.padding {
                    if !is_multiple_of_4(*p) {
                        *p = snap_to_4px(*p);
                        changed = true;
                    }
                }
            }
        }

        if is_interactive_candidate(node) {
            if node.size.x < 32.0 {
                node.size.x = 32.0;
                changed = true;
            }
            if node.size.y < 32.0 {
                node.size.y = 32.0;
                changed = true;
            }
        }

        if let Some(tp) = node.kind.any_text_mut() {
            if tp.style.font_size < 11.0 {
                tp.style.font_size = 11.0;
                changed = true;
            }
        }

        if changed {
            fixed_count += 1;
            relayout_roots.push(nid);
        }
    }

    for r in relayout_roots {
        doc.relayout_subtree(r);
    }
    fixed_count
}

/// Automatically wires `OnClick -> NavigateTo` prototype flows with `SmartAnimate` (300ms EaseInOut)
/// between top-level frames on `page` (or selected frames when `selection.len() >= 2`) and sets
/// a Flow starting point on the first frame if `doc.flows` has none for `page`.
/// Returns the number of interactions wired.
pub fn auto_wire_prototype_flows(doc: &mut Document, page: NodeId, selection: &[NodeId]) -> usize {
    use designos_doc::{Action, Easing, Flow, Reaction, Transition, TransitionKind, Trigger};

    let selected_frames: Vec<NodeId> = selection.iter().copied().filter(|&id| doc.get(id).is_some_and(|n| matches!(n.kind, NodeKind::Frame(_)))).collect();

    let frames: Vec<NodeId> = if selected_frames.len() >= 2 {
        selected_frames
    } else {
        doc.get(page)
            .map(|pg| pg.children.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|&id| doc.get(id).is_some_and(|n| matches!(n.kind, NodeKind::Frame(_))))
            .collect()
    };

    if frames.len() < 2 {
        return 0;
    }

    let mut wired = 0usize;
    for i in 0..frames.len() {
        let src_frame = frames[i];
        let dest_frame = frames[(i + 1) % frames.len()];

        // Prefer wiring an interactive child (button/CTA) inside `src_frame`; fallback to `src_frame`.
        let mut sub = Vec::new();
        collect_subtree(doc, src_frame, &mut sub);
        let source_node = sub.into_iter().find(|&nid| nid != src_frame && doc.get(nid).is_some_and(is_interactive_candidate)).unwrap_or(src_frame);

        if let Some(node) = doc.get_mut(source_node) {
            let already_wired = node
                .prototype
                .reactions
                .iter()
                .any(|r| r.trigger == Trigger::OnClick && r.actions.iter().any(|a| matches!(a, Action::Navigate { dest, .. } if *dest == dest_frame)));
            if !already_wired {
                node.prototype.reactions.push(Reaction {
                    trigger: Trigger::OnClick,
                    actions: vec![Action::Navigate { dest: dest_frame, preserve_scroll: false }],
                    transition: Some(Transition { kind: TransitionKind::SmartAnimate, easing: Easing::EaseInOut, duration_ms: 300 }),
                });
                wired += 1;
            }
        }
    }

    if !doc.flows.iter().any(|f| f.page == page) {
        doc.flows.push(Flow { name: "Main Flow".into(), page, start: frames[0], description: "Auto-wired by design:os Figma AI".into() });
    }

    wired
}

/// Semantically renames generic layers (`Frame`, `Frame 12`, `Rectangle`, `Group`, `Ellipse`,
/// `Vector`, `Line`, `Text`, etc.) across `selection` (or `page` if empty) based on node kind,
/// dimensions, auto-layout direction, and child text content.
/// Returns the number of renamed layers.
pub fn auto_rename_layers_semantically(doc: &mut Document, page: NodeId, selection: &[NodeId]) -> usize {
    let roots: Vec<NodeId> = if !selection.is_empty() { selection.to_vec() } else { doc.get(page).map(|pg| pg.children.clone()).unwrap_or_default() };

    let mut visited = Vec::new();
    for r in roots {
        collect_subtree(doc, r, &mut visited);
    }

    let mut renamed = 0usize;
    for nid in visited {
        let Some(node) = doc.get(nid) else { continue };
        if !is_generic_layer_name(&node.name) {
            continue;
        }

        let new_name = infer_semantic_layer_name(doc, nid);
        if let Some(n) = doc.get_mut(nid) {
            if n.name != new_name {
                n.name = new_name;
                renamed += 1;
            }
        }
    }
    renamed
}

fn is_generic_layer_name(name: &str) -> bool {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return true;
    }
    let lower = trimmed.to_ascii_lowercase();
    for prefix in ["frame", "rectangle", "rect", "group", "ellipse", "vector", "line", "text", "node", "layer"] {
        if lower == prefix {
            return true;
        }
        if let Some(rest) = lower.strip_prefix(prefix) {
            let rest = rest.trim();
            if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit() || c == '#' || c == '-') {
                return true;
            }
        }
    }
    false
}

fn infer_semantic_layer_name(doc: &Document, nid: NodeId) -> String {
    let Some(node) = doc.get(nid) else {
        return "Layer".into();
    };
    if let Some(tp) = node.kind.any_text() {
        let snippet: String = tp.content.trim().chars().take(18).collect();
        if !snippet.is_empty() {
            return format!("Label · {snippet}");
        }
        return "Label · Copy".into();
    }
    match &node.kind {
        NodeKind::Frame(fp) => {
            let first_text = node.children.iter().find_map(|&cid| {
                doc.get(cid).and_then(|c| c.kind.any_text()).map(|t| t.content.trim().chars().take(16).collect::<String>()).filter(|s| !s.is_empty())
            });
            if node.size.x <= 240.0 && node.size.y <= 52.0 {
                if let Some(txt) = first_text {
                    return format!("Button · {txt}");
                }
                return "Action Button".into();
            }
            if fp.auto_layout.is_some() && node.size.y <= 56.0 && node.size.x >= 240.0 {
                return "Header / Toolbar".into();
            }
            if node.children.len() >= 2 {
                return "Card Container".into();
            }
            "Section Frame".into()
        }
        NodeKind::Ellipse { .. } => {
            if (node.size.x - node.size.y).abs() < 2.0 {
                "Avatar / Badge".into()
            } else {
                "Accent Ellipse".into()
            }
        }
        NodeKind::Rectangle => {
            if node.size.y <= 2.0 || node.size.x <= 2.0 {
                "Divider Line".into()
            } else if node.size.x <= 220.0 && node.size.y <= 48.0 {
                "Surface Pill".into()
            } else {
                "Surface Card".into()
            }
        }
        NodeKind::Group => "Content Group".into(),
        _ => "Vector Shape".into(),
    }
}

/// Rewrites or translates text nodes across `selection` (or `page` if empty) according to `mode`
/// (`TranslateVi`, `TranslateEn`, `Shorten`, `Polish`).
/// Returns the number of updated text layers.
pub fn rewrite_selected_text(doc: &mut Document, page: NodeId, selection: &[NodeId], mode: TextAiMode) -> usize {
    let roots: Vec<NodeId> = if !selection.is_empty() { selection.to_vec() } else { doc.get(page).map(|pg| pg.children.clone()).unwrap_or_default() };

    let mut visited = Vec::new();
    for r in roots {
        collect_subtree(doc, r, &mut visited);
    }

    let mut updated = 0usize;
    let mut relayout_roots = Vec::new();

    for nid in visited {
        let Some(node) = doc.get_mut(nid) else { continue };
        let Some(tp) = node.kind.any_text_mut() else { continue };
        let orig = tp.content.trim();
        if orig.is_empty() {
            continue;
        }
        let rewritten = transform_text_for_mode(orig, mode);
        if rewritten != tp.content {
            tp.content = rewritten;
            tp.runs.clear();
            updated += 1;
            relayout_roots.push(nid);
        }
    }

    for r in relayout_roots {
        doc.relayout_subtree(r);
    }
    updated
}

fn transform_text_for_mode(input: &str, mode: TextAiMode) -> String {
    const EN_VI_PAIRS: &[(&str, &str)] = &[
        ("Welcome back", "Chào mừng trở lại"),
        ("Get started", "Bắt đầu ngay"),
        ("Continue", "Tiếp tục"),
        ("Learn more", "Tìm hiểu thêm"),
        ("Dashboard", "Bảng điều khiển"),
        ("Quarterly report", "Báo cáo quý"),
        ("Settings", "Cài đặt"),
        ("Search", "Tìm kiếm"),
        ("Save", "Lưu"),
        ("Cancel", "Huỷ"),
        ("Upgrade to Pro", "Nâng cấp Pro"),
        ("Sign in", "Đăng nhập"),
        ("Sign out", "Đăng xuất"),
        ("Active", "Đang hoạt động"),
    ];

    match mode {
        TextAiMode::TranslateVi => {
            for &(en, vi) in EN_VI_PAIRS {
                if input.eq_ignore_ascii_case(en) {
                    return vi.to_string();
                }
            }
            let mut out = input.to_string();
            for &(en, vi) in EN_VI_PAIRS {
                if out.contains(en) {
                    out = out.replace(en, vi);
                }
            }
            if out == input {
                format!("VI · {input}")
            } else {
                out
            }
        }
        TextAiMode::TranslateEn => {
            for &(en, vi) in EN_VI_PAIRS {
                if input == vi {
                    return en.to_string();
                }
            }
            if let Some(stripped) = input.strip_prefix("VI · ") {
                return stripped.to_string();
            }
            let mut out = input.to_string();
            for &(en, vi) in EN_VI_PAIRS {
                if out.contains(vi) {
                    out = out.replace(vi, en);
                }
            }
            out
        }
        TextAiMode::Shorten => {
            let words: Vec<&str> = input.split_whitespace().collect();
            if words.len() <= 4 {
                input.trim_end_matches('.').to_string()
            } else {
                words[..4].join(" ")
            }
        }
        TextAiMode::Polish => {
            let trimmed = input.trim().trim_end_matches('.');
            let mut chars = trimmed.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        }
    }
}

/// Replaces placeholder or repeated text nodes (`"Text"`, `"Label"`, `"Item"`, `"Name"`,
/// `"Title"`, `"Description"`, `"0"`, `"$0"`, etc.) across `selection` (or `page` if empty)
/// with varied, realistic SaaS product names, roles, metrics, and statuses.
/// Returns the number of populated text layers.
pub fn populate_realistic_content(doc: &mut Document, page: NodeId, selection: &[NodeId]) -> usize {
    const SAMPLE_NAMES: &[&str] =
        &["Linh Nguyễn · Design Lead", "Minh Trần · Staff Engineer", "An Phạm · Product Manager", "Khánh Lê · AI Systems", "Thảo Vũ · Frontend Architect"];
    const SAMPLE_METRICS: &[&str] = &["$12,480", "+24.8%", "99.94%", "14.2 ms", "$48,920"];
    const SAMPLE_TITLES: &[&str] =
        &["Realtime Token Sync", "Auto-Layout 5.0 Solver", "WCAG 2.1 Contrast Gate", "Native PTY Agent Bridge", "Kiwi .fig Binary Export"];
    const SAMPLE_STATUSES: &[&str] = &["Ready for dev", "Active · Verified", "In review · G4", "Deployed · v2.4", "Synced · 6.5ms"];

    let roots: Vec<NodeId> = if !selection.is_empty() { selection.to_vec() } else { doc.get(page).map(|pg| pg.children.clone()).unwrap_or_default() };

    let mut visited = Vec::new();
    for r in roots {
        collect_subtree(doc, r, &mut visited);
    }

    let mut populated = 0usize;
    let mut seen_texts = std::collections::HashSet::new();
    let mut relayout_roots = Vec::new();

    for (idx, nid) in visited.into_iter().enumerate() {
        let Some(node) = doc.get_mut(nid) else { continue };
        let Some(tp) = node.kind.any_text_mut() else { continue };
        let trimmed = tp.content.trim();
        let lower = trimmed.to_ascii_lowercase();
        let is_placeholder = matches!(
            lower.as_str(),
            "" | "text" | "label" | "item" | "name" | "title" | "description" | "value" | "status" | "0" | "$0" | "0%" | "lorem ipsum"
        );
        let is_duplicate = !trimmed.is_empty() && !seen_texts.insert(trimmed.to_string());

        if is_placeholder || is_duplicate {
            let replacement = if lower.contains('$') || lower == "0" || lower == "0%" || lower == "value" {
                SAMPLE_METRICS[(idx + populated) % SAMPLE_METRICS.len()]
            } else if lower == "name" {
                SAMPLE_NAMES[(idx + populated) % SAMPLE_NAMES.len()]
            } else if lower == "status" {
                SAMPLE_STATUSES[(idx + populated) % SAMPLE_STATUSES.len()]
            } else if idx % 3 == 1 {
                SAMPLE_METRICS[(idx + populated) % SAMPLE_METRICS.len()]
            } else if idx % 3 == 2 {
                SAMPLE_STATUSES[(idx + populated) % SAMPLE_STATUSES.len()]
            } else {
                SAMPLE_TITLES[(idx + populated) % SAMPLE_TITLES.len()]
            };
            tp.content = replacement.to_string();
            tp.runs.clear();
            populated += 1;
            relayout_roots.push(nid);
        }
    }

    for r in relayout_roots {
        doc.relayout_subtree(r);
    }
    populated
}

/// Upgrades image fills on the selected subtree to crisp `ImageScaleMode::Fill` at full opacity,
/// snaps fractional node coordinates/sizes (`transform.tx`, `transform.ty`, `size.x`, `size.y`) to
/// integer device pixels, and sharpens sub-pixel stroke weights to crisp integer widths.
pub fn boost_resolution_on_selection(doc: &mut Document, page: NodeId, selection: &[NodeId]) -> usize {
    let roots: Vec<NodeId> = if !selection.is_empty() { selection.to_vec() } else { doc.get(page).map(|pg| pg.children.clone()).unwrap_or_default() };

    let mut visited = Vec::new();
    for r in &roots {
        collect_subtree(doc, *r, &mut visited);
    }

    let mut boosted = 0usize;
    for nid in visited {
        let Some(node) = doc.get_mut(nid) else { continue };
        let mut changed = false;

        for fill in &mut node.fills {
            if let PaintKind::Image { scale_mode, .. } = &mut fill.kind {
                if *scale_mode != designos_doc::ImageScaleMode::Fill {
                    *scale_mode = designos_doc::ImageScaleMode::Fill;
                    changed = true;
                }
                if (fill.opacity - 1.0).abs() > f32::EPSILON || !fill.visible {
                    fill.opacity = 1.0;
                    fill.visible = true;
                    changed = true;
                }
            }
        }

        let rx = node.transform.tx.round();
        let ry = node.transform.ty.round();
        let rw = node.size.x.round().max(1.0);
        let rh = node.size.y.round().max(1.0);
        if (node.transform.tx - rx).abs() > 0.001
            || (node.transform.ty - ry).abs() > 0.001
            || (node.size.x - rw).abs() > 0.001
            || (node.size.y - rh).abs() > 0.001
        {
            node.transform.tx = rx;
            node.transform.ty = ry;
            node.size.x = rw;
            node.size.y = rh;
            changed = true;
        }

        if node.stroke.weight > 0.0 {
            let sw = node.stroke.weight.round().max(1.0);
            if (node.stroke.weight - sw).abs() > 0.001 {
                node.stroke.weight = sw;
                changed = true;
            }
        }

        if changed {
            boosted += 1;
        }
    }
    boosted
}

fn collect_subtree(doc: &Document, root: NodeId, out: &mut Vec<NodeId>) {
    let Some(node) = doc.get(root) else { return };
    if !matches!(node.kind, NodeKind::Page { .. }) {
        out.push(root);
    }
    let children = node.children.clone();
    for child in children {
        collect_subtree(doc, child, out);
    }
}

fn is_multiple_of_4(v: f32) -> bool {
    if v <= 0.0 {
        return true;
    }
    let rounded = v.round();
    (v - rounded).abs() < 0.05 && (rounded as i32) % 4 == 0
}

fn snap_to_4px(v: f32) -> f32 {
    if v <= 0.0 {
        0.0
    } else {
        ((v / 4.0).round() * 4.0).max(4.0)
    }
}

fn is_interactive_candidate(node: &designos_doc::Node) -> bool {
    if matches!(node.kind, NodeKind::Instance { .. }) {
        return true;
    }
    let lower = node.name.to_ascii_lowercase();
    lower.contains("button")
        || lower.contains("btn")
        || lower.contains("cta")
        || lower.contains("input")
        || lower.contains("chip")
        || lower.contains("toggle")
        || lower.contains("tab")
}

fn uses_easeui_standard_palette(node: &designos_doc::Node) -> bool {
    const EASEUI_HEXES: &[&str] = &[
        "121215", "0E0E12", "16171B", "18181B", "18181C", "1F2024", "202024", "26272C", "27272A", "27272D", "2A2B31", "303036", "3F3F46", "F4F4F5", "A1A1AA",
        "71717A", "E36323", "FF6A3D", "3582FF", "2563EB", "06B6D4", "22D3EE", "18B368", "10B981", "4CC58B", "F59E0B", "E0A53C", "EF4444", "FF7A8C", "8FB0FF",
        "FFFFFF", "FAFAFA", "E4E4E7",
    ];
    node.fills.iter().all(|p| match &p.kind {
        PaintKind::Solid(c) => {
            let hex = c.to_hex();
            EASEUI_HEXES.iter().any(|&h| h.eq_ignore_ascii_case(&hex))
        }
        _ => true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{AutoLayout, Color, Node, Paint, Rect, TextStyle};

    #[test]
    fn synthesize_spec_from_prompt_produces_rich_easeui_subtrees() {
        for prompt in ["pricing card", "login modal", "audit log table", "agent console", "sidebar nav", "hero banner", "custom telemetry widget"] {
            let spec = synthesize_spec_from_prompt(prompt);
            assert!(!spec.name.is_empty());
            assert!(spec.preset.is_some());
            assert!(!spec.children.is_empty());

            let mut doc = Document::new("Test");
            let page = doc.add_page("Page 1");
            let root_id = doc.apply_spec(page, &spec).expect("synthesized NodeSpec should apply cleanly");
            let root = doc.get(root_id).unwrap();
            assert!(!root.children.is_empty());
        }
    }

    #[test]
    fn design_gate_audit_detects_and_autofixes_spacing_tap_and_font_issues() {
        let mut doc = Document::new("Audit Test");
        let page = doc.add_page("Page 1");

        let btn_id = doc.alloc_id();
        let mut btn = Node::frame(btn_id, Rect::new(0.0, 0.0, 120.0, 24.0));
        btn.name = "Primary Action Button".into();
        btn.fills = vec![Paint::solid(Color::rgb(0x12, 0x34, 0x56))];
        if let Some(fp) = btn.kind.frame_props_mut() {
            fp.auto_layout = Some(AutoLayout { item_spacing: 7.0, padding: [5.0, 12.0, 5.0, 12.0], ..Default::default() });
        }
        doc.insert(page, btn).unwrap();

        let txt_id = doc.alloc_id();
        let style = TextStyle { font_size: 9.0, ..TextStyle::default() };
        let txt = Node::text(txt_id, Rect::new(0.0, 0.0, 60.0, 12.0), "Click", style);
        doc.insert(btn_id, txt).unwrap();

        let report = run_design_gate_audit(&doc, page, &[]);
        assert!(report.score < 100);
        assert!(report.warnings.iter().any(|w| w.rule == "Spacing Rhythm (4px)"));
        assert!(report.warnings.iter().any(|w| w.rule == "Tap Target (>=32px)"));
        assert!(report.warnings.iter().any(|w| w.rule == "Token Compliance"));
        assert!(report.warnings.iter().any(|w| w.rule == "Text Readability (>=11px)"));

        let fixed = auto_fix_design_gate(&mut doc, page, &[]);
        assert!(fixed >= 2);

        let after = run_design_gate_audit(&doc, page, &[]);
        assert!(!after.warnings.iter().any(|w| w.rule == "Spacing Rhythm (4px)"));
        assert!(!after.warnings.iter().any(|w| w.rule == "Tap Target (>=32px)"));
        assert!(!after.warnings.iter().any(|w| w.rule == "Text Readability (>=11px)"));
    }

    #[test]
    fn figma_ai_make_prototype_auto_wires_smart_animate_flows() {
        let mut doc = Document::new("Proto AI Test");
        let page = doc.add_page("Page 1");

        let f1 = doc.alloc_id();
        let mut frame1 = Node::frame(f1, Rect::new(0.0, 0.0, 320.0, 240.0));
        frame1.name = "Onboarding Step 1".into();
        doc.insert(page, frame1).unwrap();

        let cta = doc.alloc_id();
        let mut btn = Node::frame(cta, Rect::new(16.0, 180.0, 140.0, 40.0));
        btn.name = "Primary CTA Button".into();
        doc.insert(f1, btn).unwrap();

        let f2 = doc.alloc_id();
        let mut frame2 = Node::frame(f2, Rect::new(380.0, 0.0, 320.0, 240.0));
        frame2.name = "Onboarding Step 2".into();
        doc.insert(page, frame2).unwrap();

        let wired = auto_wire_prototype_flows(&mut doc, page, &[]);
        assert_eq!(wired, 2);
        assert_eq!(doc.flows.len(), 1);
        assert_eq!(doc.flows[0].start, f1);
        assert!(!doc.get(cta).unwrap().prototype.reactions.is_empty());
    }

    #[test]
    fn figma_ai_rename_rewrite_and_populate_mutate_document_cleanly() {
        let mut doc = Document::new("AI Suite Test");
        let page = doc.add_page("Page 1");

        let f_id = doc.alloc_id();
        let mut frame = Node::frame(f_id, Rect::new(0.0, 0.0, 200.0, 44.0));
        frame.name = "Frame 12".into();
        doc.insert(page, frame).unwrap();

        let t1 = doc.alloc_id();
        let txt1 = Node::text(t1, Rect::new(0.0, 0.0, 120.0, 20.0), "Welcome back", TextStyle::default());
        doc.insert(f_id, txt1).unwrap();

        let t2 = doc.alloc_id();
        let mut txt2 = Node::text(t2, Rect::new(0.0, 24.0, 120.0, 20.0), "$0", TextStyle::default());
        txt2.name = "Text 4".into();
        doc.insert(page, txt2).unwrap();

        let renamed = auto_rename_layers_semantically(&mut doc, page, &[]);
        assert!(renamed >= 2);
        assert!(doc.get(f_id).unwrap().name.starts_with("Button ·"));
        assert!(doc.get(t2).unwrap().name.starts_with("Label ·"));

        let translated = rewrite_selected_text(&mut doc, page, &[t1], TextAiMode::TranslateVi);
        assert_eq!(translated, 1);
        assert_eq!(doc.get(t1).unwrap().kind.any_text().unwrap().content, "Chào mừng trở lại");

        let back_en = rewrite_selected_text(&mut doc, page, &[t1], TextAiMode::TranslateEn);
        assert_eq!(back_en, 1);
        assert_eq!(doc.get(t1).unwrap().kind.any_text().unwrap().content, "Welcome back");

        let populated = populate_realistic_content(&mut doc, page, &[t2]);
        assert_eq!(populated, 1);
        assert_ne!(doc.get(t2).unwrap().kind.any_text().unwrap().content, "$0");
    }

    #[test]
    fn actions_cmdk_modal_matches_figma_ui3_recents_and_image_editing_sections() {
        let all_tab_items: Vec<QuickCommandItem> = QUICK_COMMANDS.iter().copied().filter(|item| (item.tab_mask & 1) != 0).collect();
        assert_eq!(all_tab_items.len(), 5);
        assert_eq!(all_tab_items[0].section, "Recents");
        assert_eq!(all_tab_items[0].title, "Ease Design Figma Agent");
        assert_eq!(all_tab_items[0].badge, "Development");
        assert_eq!(all_tab_items[1].section, "Recents");
        assert_eq!(all_tab_items[1].title, "Replace content");
        assert_eq!(all_tab_items[1].badge, "AI");
        assert_eq!(all_tab_items[2].section, "Image editing");
        assert_eq!(all_tab_items[2].title, "Make an image");
        assert_eq!(all_tab_items[2].badge, "AI");
        assert_eq!(all_tab_items[3].title, "Boost resolution");
        assert_eq!(all_tab_items[4].title, "Edit image with prompt");
        assert!(QUICK_COMMANDS.iter().all(|item| item.title != "Remove background"), "the fake fill-strip entry must stay removed");
    }

    #[test]
    fn boost_resolution_mutates_document_cleanly() {
        let mut doc = Document::new("Image Editing AI Test");
        let page = doc.add_page("Page 1");

        let nid = doc.alloc_id();
        let mut node = Node::frame(nid, Rect::new(10.35, 20.72, 199.6, 120.4));
        node.fills = vec![Paint {
            kind: PaintKind::Image { image_ref: "hero-asset".into(), scale_mode: designos_doc::ImageScaleMode::Fit },
            opacity: 0.85,
            visible: true,
            blend: designos_doc::BlendMode::Normal,
        }];
        node.stroke.weight = 1.4;
        doc.insert(page, node).unwrap();

        let boosted = boost_resolution_on_selection(&mut doc, page, &[nid]);
        assert_eq!(boosted, 1);
        let updated = doc.get(nid).unwrap();
        assert_eq!(updated.transform.tx, 10.0);
        assert_eq!(updated.transform.ty, 21.0);
        assert_eq!(updated.size.x, 200.0);
        assert_eq!(updated.size.y, 120.0);
        assert_eq!(updated.stroke.weight, 1.0);
        assert!(matches!(updated.fills[0].kind, PaintKind::Image { scale_mode: designos_doc::ImageScaleMode::Fill, .. }));
        assert_eq!(updated.fills[0].opacity, 1.0);
    }
}
