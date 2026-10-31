//! The agent menu above the composer pill, built from the single [`crate::agent_cli_catalog::build_picker_rows`] catalog list,
//! and its inline `"Other CLI…"` two-field form (`#652`).

use std::path::{Path, PathBuf};

use egui::{Align2, Id, Key, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, TextEdit, Ui, Vec2};

use crate::agent_cli_catalog::{build_picker_rows, CatalogProbeCache, CustomCliEntry, PickerRow, PickerRowKind, SignIn, CUSTOM_CLI_WARNING};
use crate::agent_composer_menus::{MENU_W_NARROW, MENU_W_WIDE};
use crate::agent_start_failure::{login_command, FixAction};
use crate::comments::settings::{add_custom_cli, load_custom_clis};
use crate::terminal::{AgentCliPreset, TerminalState};
use crate::theme;
use crate::widgets;

/// How often the window repaints while the CLI probes are running, so a result shows without another input.
pub const PROBE_REPAINT: std::time::Duration = std::time::Duration::from_millis(50);

/// Per-terminal UI state for the new-session picker, compact `+` menu, and inline `"Other CLI…"` form (`#652`).
#[derive(Clone, Default)]
pub struct AgentPickerState {
    pub cache: CatalogProbeCache,
    pub probe_started: bool,
    pub open_row_order: Vec<String>,
    /// True once `open_row_order` was built from fully-probed rows (no catalog row left `Checking`).
    pub open_row_order_settled: bool,
    pub other_form_open: bool,
    pub other_label_input: String,
    pub other_command_input: String,
    pub other_error: Option<String>,
    pub settings_root_override: Option<PathBuf>,
    /// Memo of the OpenCode PATH check, which the probed catalog does not cover; cleared when a probe cycle starts.
    pub opencode_installed: Option<bool>,
    /// Cached composer pill. Rebuilt when probes settle or a probe result changes, not every frame.
    pub pill_label_cache: &'static str,
    pub pill_preset_cache: Option<AgentCliPreset>,
    pub pill_cache_valid: bool,
    /// The probe generation the cached pill was built from.
    pub pill_cache_generation: u64,
    /// `More…` in the agent menu stays open while the other-CLI form is open.
    pub menu_more_open: bool,
    /// Scroll of the agent menu when it is taller than the room above the pill.
    pub menu_scroll: f32,
    /// Set when the `Other CLI…` form opens: the next menu pass scrolls the form into view (`#1010`).
    pub reveal_other_form: bool,
    /// The agent menu's height on its last pass: a change while the `Other CLI…` form is open (a window resize) reveals the form again.
    pub menu_view_h: f32,
}

impl AgentPickerState {
    /// True while the cached pill was built from settled probes and no probe result has changed since.
    pub fn pill_cache_current(&self) -> bool {
        self.pill_cache_valid && self.pill_cache_generation == self.cache.generation()
    }

    /// Starts the background version + sign-in probe when the picker or `+` menu is opened (`#652` §3 rule 2 & §5).
    /// Cached probe results stay on screen while the fresh probe runs in the background.
    pub fn ensure_probed(&mut self) {
        if !self.probe_started {
            self.probe_started = true;
            self.opencode_installed = None;
            self.cache.refresh_async();
        }
    }

    /// Resets the open-latch when the picker or `+` menu closes so the next open triggers a fresh
    /// background probe while keeping the cached result on screen (`#652` §5). Closing also drops the `Other CLI…`
    /// form and what was typed in it, however the menu was closed (a press elsewhere, `+`, a tab, Esc, a pick).
    pub fn on_close(&mut self) {
        self.probe_started = false;
        self.open_row_order.clear();
        self.open_row_order_settled = false;
        self.menu_more_open = false;
        self.menu_scroll = 0.0;
        self.close_other_form();
    }

    /// Triggers a fresh background probe (e.g. when re-opening the picker).
    #[allow(dead_code)]
    pub fn refresh(&mut self) {
        self.probe_started = true;
        self.opencode_installed = None;
        self.open_row_order.clear();
        self.open_row_order_settled = false;
        self.cache.refresh_async();
    }

    /// Opens the inline two-field `"Other CLI…"` form.
    pub fn open_other_form(&mut self) {
        self.other_form_open = true;
        self.other_error = None;
        self.reveal_other_form = true;
    }

    /// Closes and resets the inline `"Other CLI…"` form.
    pub fn close_other_form(&mut self) {
        self.other_form_open = false;
        self.reveal_other_form = false;
        self.other_label_input.clear();
        self.other_command_input.clear();
        self.other_error = None;
    }
}

/// Resolves the Studio `settings.json` directory for `state`:
/// 1. `state.picker.settings_root_override` if set (used by tests);
/// 2. `state.library_root` if set (shared across library files, unsaved docs, and `.fig` imports);
/// 3. `lib_root` when `state.key` is `SessionKey::LibraryFile { lib_root, .. }`;
/// 4. `designos_library::Library::default_root()` (or `state.session_dir` in unit tests without a library root).
pub fn resolve_settings_root(state: &TerminalState) -> PathBuf {
    if let Some(ref root) = state.picker.settings_root_override {
        return root.clone();
    }
    if let Some(ref root) = state.library_root {
        return root.clone();
    }
    if let crate::agent_sessions::SessionKey::LibraryFile { ref lib_root, .. } = state.key {
        return lib_root.clone();
    }
    if cfg!(test) {
        return state.session_dir.clone();
    }
    designos_library::Library::default_root()
}

/// Builds the current [`PickerRow`] list for `state` using its cached probes, per-file `last_cli`,
/// and Studio's saved `custom_clis` (`#652` §3 rules 3–8).
/// While the picker or `+` menu remains open, row order is pinned (`open_row_order`) so background
/// probes finishing at different times only update row pills in-place without shifting rows (`#783`).
/// A cold open pins catalog order while rows are `Checking`, then re-sorts exactly once per `#652` rule 4
/// (installed first, last choice first, uninstalled after) when the last probe settles, and pins that order.
fn startable_preset(preset: AgentCliPreset) -> Option<AgentCliPreset> {
    match preset {
        AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp => None,
        other => Some(other),
    }
}

/// Label and preset a Send in the New chat state would start. The label is one of the catalog's own names, so no frame builds a string.
pub fn composer_pill(state: &mut TerminalState) -> (&'static str, Option<AgentCliPreset>) {
    let new_chat = state.in_new_chat();
    if !new_chat {
        if let Some(tab) = state.active_tab() {
            return (tab.spawn_preset.display_name(), startable_preset(tab.spawn_preset));
        }
    }
    if let Some(preset) = startable_preset(state.active_runtime) {
        if state.picker.cache.is_preset_installed(preset) {
            return (preset.display_name(), Some(preset));
        }
    }
    if state.picker.pill_cache_current() {
        return (state.picker.pill_label_cache, state.picker.pill_preset_cache);
    }
    // Starting the probe cycle writes seed probes, so it comes first: the generation read next is the one the rows are built from.
    state.picker.ensure_probed();
    let generation = state.picker.cache.generation();
    let rows = rows_for_state(state);
    let found = rows.iter().find_map(|row| match &row.kind {
        PickerRowKind::CatalogCli { preset, installed, .. } if *installed && row.selectable => {
            startable_preset(*preset).map(|preset| (preset.display_name(), Some(preset)))
        }
        _ => None,
    });
    let (label, preset) = found.unwrap_or((CHOOSE_AGENT, None));
    state.picker.pill_label_cache = label;
    state.picker.pill_preset_cache = preset;
    state.picker.pill_cache_valid = state.picker.open_row_order_settled;
    state.picker.pill_cache_generation = generation;
    (label, preset)
}

/// The pill's label while no installed agent can be started.
pub const CHOOSE_AGENT: &str = "Choose agent";

pub fn rows_for_state(state: &mut TerminalState) -> Vec<PickerRow> {
    state.picker.ensure_probed();
    let settings_root = resolve_settings_root(state);
    let custom_clis: Vec<CustomCliEntry> = load_custom_clis(&settings_root);
    let snap = state.picker.cache.snapshot();
    let mut rows = build_picker_rows(&snap, state.session_registry.last_cli.as_deref(), &custom_clis);
    let any_checking = rows.iter().any(|r| matches!(r.kind, PickerRowKind::CatalogCli { signed_in: SignIn::Checking, .. }));
    if state.picker.open_row_order.is_empty() || (!state.picker.open_row_order_settled && !any_checking) {
        // First build, or the moment the last probe settles: take the `#652` rule 4 order once, then pin it.
        state.picker.open_row_order = rows.iter().map(|r| r.id.clone()).collect();
        state.picker.open_row_order_settled = !any_checking;
    } else {
        let order = state.picker.open_row_order.clone();
        rows.sort_by_key(|r| order.iter().position(|id| id == &r.id).unwrap_or(usize::MAX));
    }
    apply_ask_risky_availability(state, &mut rows);
    rows
}

/// In Ask for risky only, a CLI with no shared approval hook cannot be started from the picker.
fn apply_ask_risky_availability(state: &TerminalState, rows: &mut [PickerRow]) {
    if state.permission_mode != crate::terminal::PermissionMode::AskRisky {
        return;
    }
    for row in rows.iter_mut() {
        if !row.selectable {
            continue;
        }
        let reason = match &row.kind {
            PickerRowKind::CatalogCli { preset, .. } => crate::agent_permissions::ask_risky_unsupported_reason(*preset),
            PickerRowKind::CustomCli { .. } | PickerRowKind::OtherCli => {
                Some("A custom command does not share the approval hook, so Ask for risky only is unavailable.")
            }
            PickerRowKind::Shell => None,
        };
        let Some(reason) = reason else { continue };
        row.selectable = false;
        row.state_label = Some("Unavailable".to_string());
        row.caption = Some(reason.to_string());
    }
}

/// Saves the inline `"Other CLI…"` form (`other_label_input`, `other_command_input`) to Studio's
/// `settings.json` under `custom_clis` and launches the custom CLI tab (`#652` §3 rule 7).
pub fn submit_other_cli_form(state: &mut TerminalState, file_path: Option<&Path>) -> Result<CustomCliEntry, String> {
    let label = state.picker.other_label_input.trim().to_string();
    let command = state.picker.other_command_input.trim().to_string();
    if command.is_empty() {
        let err = "Enter a command to run.".to_string();
        state.picker.other_error = Some(err.clone());
        return Err(err);
    }
    let root = resolve_settings_root(state);
    let entries = add_custom_cli(&root, &label, &command)?;
    let entry = entries.into_iter().last().unwrap_or(CustomCliEntry { label: if label.is_empty() { command.clone() } else { label }, command });
    state.picker.close_other_form();
    state.runtime_dropdown_open = false;
    state.picker.on_close();
    let _ = state.spawn_custom_cli_tab(&entry.label, &entry.command, file_path);
    Ok(entry)
}

#[derive(Clone, Debug)]
enum PickerInteraction {
    None,
    SelectRow(usize),
    ActivateInstall(FixAction),
    RunLogin(AgentCliPreset),
    OpenOtherForm,
    SubmitOtherForm,
    CancelOtherForm,
    #[cfg_attr(not(test), allow(dead_code))]
    ResumeLastSession,
}

fn is_row_clickable_on_pointer(state: &TerminalState, row: &PickerRow) -> bool {
    if row.selectable {
        return true;
    }
    if let PickerRowKind::CatalogCli { preset, signed_in: SignIn::Checking, .. } = row.kind {
        return state.picker.cache.is_preset_installed(preset);
    }
    false
}

/// Executes selecting `row` in the new-session picker or `+` menu.
/// Returns `Some(FixAction)` when an uninstalled CLI row's install action is triggered without spawning a tab.
pub fn execute_row_selection(state: &mut TerminalState, row: &PickerRow, file_path: Option<&Path>) -> Option<FixAction> {
    match &row.kind {
        PickerRowKind::CatalogCli { preset, installed, signed_in, install_action, .. } => {
            if *installed || (*signed_in == SignIn::Checking && state.picker.cache.is_preset_installed(*preset)) {
                state.runtime_dropdown_open = false;
                state.picker.on_close();
                state.active_runtime = *preset;
                let _ = state.spawn_tab(*preset, file_path);
                None
            } else if let Some(fix) = install_action.clone() {
                state.picker.on_close();
                state.apply_start_fix(fix.clone(), file_path);
                Some(fix)
            } else {
                None
            }
        }
        PickerRowKind::CustomCli { entry } => {
            state.runtime_dropdown_open = false;
            state.picker.on_close();
            let _ = state.spawn_custom_cli_tab(&entry.label, &entry.command, file_path);
            None
        }
        PickerRowKind::OtherCli => {
            state.picker.menu_more_open = true;
            state.picker.open_other_form();
            None
        }
        PickerRowKind::Shell => {
            state.runtime_dropdown_open = false;
            state.picker.on_close();
            state.active_runtime = AgentCliPreset::Shell;
            let _ = state.spawn_tab(AgentCliPreset::Shell, file_path);
            None
        }
    }
}

/// The row focus moves to from `from` on an arrow key: the next (`forward`) or previous selectable row, wrapping
/// at both ends. `None` when no row is selectable.
fn step_row(selectable: &[bool], from: usize, forward: bool) -> Option<usize> {
    let n = selectable.len();
    (1..=n).map(|step| if forward { (from + step) % n } else { (from + n - step % n) % n }).find(|&idx| selectable[idx])
}

/// Arrow keys move egui focus between the rows of the list, and Return is egui's own click on the focused row.
/// Both act only while a row has focus (reached with Tab, or by a click): with nothing focused the picker ignores
/// them, so a layer selected on the canvas keeps its arrow and Return shortcuts and a stray Return starts nothing.
/// `row_id` is the widget id the surface draws a row with.
fn handle_picker_keyboard(state: &mut TerminalState, ui: &Ui, rows: &[PickerRow], row_id: impl Fn(&PickerRow) -> Id) {
    // An open dialog or menu owns the keys, Esc included: the Esc that closes it must not also close this list (`#923`).
    if state.modal_owns_keyboard {
        return;
    }
    let ctx = ui.ctx();
    if state.picker.other_form_open {
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            state.picker.close_other_form();
        }
        return;
    }
    let focused = rows.iter().position(|r| ctx.memory(|m| m.has_focus(row_id(r))));
    if let Some(from) = focused {
        let forward = if ui.input(|i| i.key_pressed(Key::ArrowDown)) {
            Some(true)
        } else if ui.input(|i| i.key_pressed(Key::ArrowUp)) {
            Some(false)
        } else {
            None
        };
        let selectable: Vec<bool> = rows.iter().map(|r| r.selectable).collect();
        if let Some(to) = forward.and_then(|forward| step_row(&selectable, from, forward)) {
            ctx.memory_mut(|m| m.request_focus(row_id(&rows[to])));
        }
    }
    // egui drops a focused widget's focus on Escape before this runs, so Escape on a focused row lands here too.
    if !ctx.egui_wants_keyboard_input() && ui.input(|i| i.key_pressed(Key::Escape)) {
        state.runtime_dropdown_open = false;
        state.picker.on_close();
    }
}

/// Keeps arrow keys with the focused row: egui would otherwise also move focus to the nearest widget on its own.
fn keep_arrows_on_row(ui: &Ui, resp: &egui::Response) {
    if resp.has_focus() {
        ui.memory_mut(|m| m.set_focus_lock_filter(resp.id, egui::EventFilter { vertical_arrows: true, ..Default::default() }));
    }
}

/// Height of the `Other CLI…` form card, summed from its parts so it follows the caption line height: padding, the label
/// and command fields, a caption block of two caption lines, the gap, the button row, and padding. The card is the same
/// in the full picker and in the agent menu, and its height is a multiple of 4 px.
const OTHER_FORM_CARD_HEIGHT: f32 = theme::INSET_FRAME
    + 2.0 * OTHER_FORM_FIELD_HEIGHT
    + 2.0 * theme::SPACE_ROW
    + OTHER_FORM_CAPTION_HEIGHT
    + theme::SPACE_INLINE
    + OTHER_FORM_BUTTON_HEIGHT
    + theme::INSET_FRAME;
const OTHER_FORM_FIELD_HEIGHT: f32 = 22.0;
/// Room the picker and the `+` menu give the open form: the 2 px inset above the card, the card, and a 6 px margin.
const OTHER_FORM_HEIGHT: f32 = OTHER_FORM_CARD_HEIGHT + 8.0;
/// Two lines of the caption at its line height. The caption wraps to this block and never runs under the buttons.
const OTHER_FORM_CAPTION_HEIGHT: f32 = 2.0 * theme::LINE_CAPTION;
const OTHER_FORM_BUTTON_HEIGHT: f32 = theme::CONTROL_XS;
fn preset_menu_slug(preset: AgentCliPreset) -> &'static str {
    match preset {
        AgentCliPreset::Claude => "claude",
        AgentCliPreset::Codex => "codex",
        AgentCliPreset::Grok => "grok",
        AgentCliPreset::Antigravity => "antigravity",
        AgentCliPreset::Gemini => "gemini",
        AgentCliPreset::OpenCode => "opencode",
        AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp => "shell",
    }
}

fn row_matches_pill(row: &PickerRow, pill_label: &str, pill_preset: Option<AgentCliPreset>) -> bool {
    match &row.kind {
        PickerRowKind::CatalogCli { preset, .. } => pill_preset == Some(*preset),
        PickerRowKind::CustomCli { .. } => pill_preset.is_none() && row.label == pill_label,
        PickerRowKind::Shell => pill_preset.is_none() && pill_label == row.label,
        PickerRowKind::OtherCli => false,
    }
}

fn row_tooltip(row: &PickerRow) -> Option<String> {
    let mut lines = Vec::new();
    if let Some(version) = &row.version {
        lines.push(format!("{} {version}", row.label));
    }
    if let Some(caption) = &row.caption {
        lines.push(caption.clone());
    }
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

fn menu_label_font() -> egui::FontId {
    theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold)
}

fn menu_slot_width(p: &egui::Painter, label: &str) -> f32 {
    (widgets::text_width(p, label, &menu_label_font()) + theme::INSET_FRAME * 2.0).max(theme::TARGET_MIN)
}

/// The menu scroll that shows the content span `top..bottom` in a viewport `view_h` tall with the least movement: a span that
/// fits ends at the viewport's bottom edge, one that does not starts at its top edge. Stays within `0..=max_scroll`.
fn scroll_to_reveal(top: f32, bottom: f32, view_h: f32, max_scroll: f32) -> f32 {
    let target = if bottom - top <= view_h { bottom - view_h } else { top };
    target.clamp(0.0, max_scroll.max(0.0))
}

/// Agent menu above the composer pill. Other CLI and Shell stay behind `More…`.
pub fn render_runtime_dropdown_menu(state: &mut TerminalState, ui: &mut Ui, panel_rect: Rect, top_y: f32, file_path: Option<&Path>) {
    let (pill_label, pill_preset) = composer_pill(state);
    let rows = rows_for_state(state);
    if state.picker.cache.is_probing() {
        ui.ctx().request_repaint_after(PROBE_REPAINT);
    }
    let show_folded = state.picker.menu_more_open || state.picker.other_form_open;
    let mut main_idx = Vec::new();
    let mut folded_idx = Vec::new();
    for (idx, row) in rows.iter().enumerate() {
        if matches!(row.kind, PickerRowKind::OtherCli | PickerRowKind::Shell) {
            folded_idx.push(idx);
        } else {
            main_idx.push(idx);
        }
    }
    let mut keyboard_rows = Vec::with_capacity(main_idx.len() + folded_idx.len());
    keyboard_rows.extend(main_idx.iter().map(|&idx| rows[idx].clone()));
    if show_folded {
        keyboard_rows.extend(folded_idx.iter().map(|&idx| rows[idx].clone()));
    }
    handle_picker_keyboard(state, ui, &keyboard_rows, |row| Id::new(&row.id));
    if !state.runtime_dropdown_open {
        return;
    }
    crate::widgets::popup_open::mark_open(ui.ctx());

    let inset = theme::INSET_FRAME;
    let row_h = theme::MENU_ITEM_HEIGHT;
    let fit = (panel_rect.width() - inset * 2.0).max(row_h);
    let menu_w = if fit < MENU_W_NARROW { fit } else { fit.min(MENU_W_WIDE) };
    let form_h = if state.picker.other_form_open { OTHER_FORM_HEIGHT } else { 0.0 };
    let shown = main_idx.len() + 1 + if show_folded { folded_idx.len() } else { 0 };
    let content_h = inset * 2.0 + shown as f32 * row_h + form_h;
    let pill_rect = ui.ctx().data(|d| d.get_temp::<Rect>(Id::new("agents-composer-agent-pill-rect")));
    let floor = panel_rect.top() + crate::terminal::TERMINAL_HEADER_H + theme::SPACE_INLINE;
    let (menu_top, view_h) = if let Some(pill) = pill_rect {
        let anchor_bottom = pill.top() - theme::SPACE_ROW;
        let room = (anchor_bottom - floor).max(row_h + inset * 2.0);
        let view_h = content_h.min(room);
        ((anchor_bottom - view_h).max(floor), view_h)
    } else {
        let room = (panel_rect.bottom() - top_y).max(row_h + inset * 2.0);
        (top_y, content_h.min(room))
    };
    let raw_x = pill_rect.map(|pill| pill.left()).unwrap_or(panel_rect.right() - menu_w - inset);
    let x_lo = panel_rect.left() + inset;
    let x_hi = (panel_rect.right() - menu_w - inset).max(x_lo);
    let menu_x = raw_x.clamp(x_lo, x_hi);
    let menu_r = Rect::from_min_size(Pos2::new(menu_x, menu_top), Vec2::new(menu_w, view_h));
    let max_scroll = (content_h - view_h).max(0.0);
    let resized = (state.picker.menu_view_h - view_h).abs() > 0.5;
    state.picker.menu_view_h = view_h;
    if (state.picker.reveal_other_form || resized) && state.picker.other_form_open {
        // The form sits under the Other CLI row, which Shell may follow; a menu capped by the room above the pill would leave it clipped until the wheel moves.
        let rows_above =
            main_idx.len() + 1 + folded_idx.iter().position(|&idx| matches!(rows[idx].kind, PickerRowKind::OtherCli)).map_or(folded_idx.len(), |at| at + 1);
        let form_top = inset + rows_above as f32 * row_h;
        state.picker.menu_scroll = scroll_to_reveal(form_top, form_top + form_h, view_h, max_scroll);
        state.picker.reveal_other_form = false;
    }
    if ui.rect_contains_pointer(menu_r) {
        let dy = ui.input(|i| i.smooth_scroll_delta.y);
        if dy != 0.0 {
            state.picker.menu_scroll = (state.picker.menu_scroll - dy).clamp(0.0, max_scroll);
            ui.ctx().input_mut(|i| i.smooth_scroll_delta = Vec2::ZERO);
        }
    }
    state.picker.menu_scroll = state.picker.menu_scroll.clamp(0.0, max_scroll);

    crate::widgets::kit::menu_surface(ui.painter(), menu_r);
    widgets::record(ui, "agents-agent-menu", menu_r);

    let old_clip = ui.clip_rect();
    ui.set_clip_rect(old_clip.intersect(menu_r));
    let inner_left = menu_r.left() + inset;
    let inner_w = (menu_w - inset * 2.0).max(row_h);
    let mut y = menu_r.top() + inset - state.picker.menu_scroll;
    let mut interaction = PickerInteraction::None;

    for idx in main_idx {
        let row_r = Rect::from_min_size(Pos2::new(inner_left, y), Vec2::new(inner_w, row_h));
        let hit = paint_agent_menu_row(state, ui, &rows[idx], idx, row_r, pill_label, pill_preset);
        if matches!(interaction, PickerInteraction::None) {
            interaction = hit;
        }
        y += row_h;
    }
    let more_r = Rect::from_min_size(Pos2::new(inner_left, y), Vec2::new(inner_w, row_h));
    if paint_agent_menu_more(ui, more_r, show_folded) && !state.picker.other_form_open {
        state.picker.menu_more_open = !state.picker.menu_more_open;
    }
    y += row_h;
    if show_folded {
        for idx in folded_idx {
            let row_r = Rect::from_min_size(Pos2::new(inner_left, y), Vec2::new(inner_w, row_h));
            let hit = paint_agent_menu_row(state, ui, &rows[idx], idx, row_r, pill_label, pill_preset);
            if matches!(interaction, PickerInteraction::None) {
                interaction = hit;
            }
            y += row_h;
            if matches!(rows[idx].kind, PickerRowKind::OtherCli) && state.picker.other_form_open {
                let form_r = Rect::from_min_size(Pos2::new(inner_left, y + theme::INSET_SEGMENT), Vec2::new(inner_w, OTHER_FORM_CARD_HEIGHT));
                render_other_cli_inline_form(state, ui, form_r, &mut interaction);
                y += form_h;
            }
        }
    }
    ui.set_clip_rect(old_clip);

    if !matches!(interaction, PickerInteraction::None) {
        apply_picker_interaction(state, &rows, interaction, file_path);
    } else if ui.input(|i| i.pointer.any_pressed()) {
        let opened_this_pass = ui.ctx().data(|d| d.get_temp::<u64>(Id::new("agents-runtime-opened-pass"))) == Some(ui.ctx().cumulative_pass_nr());
        if !opened_this_pass {
            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                let on_pill = ui.ctx().data(|d| d.get_temp::<Rect>(Id::new("agents-composer-agent-pill-rect"))).is_some_and(|r| r.contains(pos));
                if !menu_r.contains(pos) && !on_pill {
                    state.runtime_dropdown_open = false;
                    state.picker.on_close();
                }
            }
        }
    }
}

fn paint_agent_menu_row(
    state: &TerminalState,
    ui: &mut Ui,
    row: &PickerRow,
    idx: usize,
    row_r: Rect,
    pill_label: &str,
    pill_preset: Option<AgentCliPreset>,
) -> PickerInteraction {
    let clickable = is_row_clickable_on_pointer(state, row);
    let resp = ui.interact(row_r, Id::new(&row.id), if clickable { Sense::click() } else { Sense::hover() });
    keep_arrows_on_row(ui, &resp);
    let checked = row_matches_pill(row, pill_label, pill_preset);
    let look = crate::widgets::kit::menu_row_look(!clickable, resp.hovered() || resp.has_focus(), checked);
    let p = ui.painter().clone();
    crate::widgets::kit::paint_frame(&p, row_r, theme::RADIUS_CONTROL, &look);
    if resp.has_focus() {
        crate::widgets::kit::focus_ring(&p, row_r, theme::RADIUS_CONTROL as f32);
    }
    let catalog = match &row.kind {
        PickerRowKind::CatalogCli { preset, installed, signed_in, .. } => Some((*preset, *installed, *signed_in)),
        _ => None,
    };
    let mut right = row_r.right() - theme::INSET_FRAME;
    let mut slot_hit = PickerInteraction::None;
    if let Some(fix) = &row.install_action {
        let label = "Install";
        let btn_w = menu_slot_width(&p, label);
        let btn_r = Rect::from_min_size(Pos2::new(right - btn_w, row_r.center().y - theme::TARGET_MIN * 0.5), Vec2::new(btn_w, theme::TARGET_MIN));
        let btn = ui.interact(btn_r, Id::new(format!("{}-install", row.id)), Sense::click());
        let paint = crate::widgets::kit::button_look(crate::widgets::kit::Hierarchy::Tertiary, crate::widgets::kit::state_of(&btn, false, true));
        crate::widgets::kit::paint_frame(&p, btn_r, theme::RADIUS_CONTROL, &paint);
        p.text(btn_r.center(), Align2::CENTER_CENTER, label, menu_label_font(), paint.text);
        widgets::record(ui, &format!("{}-install", row.id), btn_r);
        widgets::labelled(ui, btn.id, egui::accesskit::Role::Button, label);
        if btn.clicked() {
            slot_hit = PickerInteraction::ActivateInstall(fix.clone());
        }
        right = btn_r.left() - theme::SPACE_ROW;
    } else if let Some((preset, installed, signed_in)) = catalog {
        if installed && signed_in == SignIn::No {
            let label = "Sign in";
            let btn_w = menu_slot_width(&p, label);
            let btn_r = Rect::from_min_size(Pos2::new(right - btn_w, row_r.center().y - theme::TARGET_MIN * 0.5), Vec2::new(btn_w, theme::TARGET_MIN));
            let key = format!("agents-menu-sign-in-{}", preset_menu_slug(preset));
            let btn = ui.interact(btn_r, Id::new(&key), Sense::click());
            let paint = crate::widgets::kit::button_look(crate::widgets::kit::Hierarchy::Tertiary, crate::widgets::kit::state_of(&btn, false, true));
            crate::widgets::kit::paint_frame(&p, btn_r, theme::RADIUS_CONTROL, &paint);
            p.text(btn_r.center(), Align2::CENTER_CENTER, label, menu_label_font(), theme::TEXT_BRAND);
            widgets::record(ui, &key, btn_r);
            widgets::labelled(ui, btn.id, egui::accesskit::Role::Button, label);
            if btn.clicked() {
                slot_hit = PickerInteraction::RunLogin(preset);
            }
            right = btn_r.left() - theme::SPACE_ROW;
        } else if signed_in == SignIn::Checking {
            let status = "Checking…";
            let status_w = widgets::text_width(&p, status, &theme::font_caption());
            p.text(Pos2::new(right, row_r.center().y), Align2::RIGHT_CENTER, status, theme::font_caption(), theme::TEXT_TERTIARY);
            right -= status_w + theme::SPACE_ROW;
        }
    }
    if checked {
        let mark_box = Rect::from_center_size(Pos2::new(right - theme::LAYER_ICON_SIZE * 0.5, row_r.center().y), Vec2::splat(theme::LAYER_ICON_SIZE));
        widgets::paint_icon_in(&p, mark_box, crate::icons::Icon::Check, theme::LAYER_ICON_SIZE, look.mark);
        right -= theme::LAYER_ICON_SIZE + theme::SPACE_ROW;
    }
    let left_x = row_r.left() + theme::INSET_FRAME;
    let title = widgets::elide(&p, &row.label, &menu_label_font(), (right - left_x).max(theme::TARGET_MIN));
    p.text(Pos2::new(left_x, row_r.center().y), Align2::LEFT_CENTER, title, menu_label_font(), look.text);
    let resp = if let Some(tip) = row_tooltip(row) { resp.on_hover_text(tip) } else { resp };
    widgets::record(ui, &row.id, row_r);
    if !matches!(slot_hit, PickerInteraction::None) {
        return slot_hit;
    }
    if resp.clicked() && clickable {
        if matches!(row.kind, PickerRowKind::OtherCli) {
            return PickerInteraction::OpenOtherForm;
        }
        return PickerInteraction::SelectRow(idx);
    }
    PickerInteraction::None
}

fn paint_agent_menu_more(ui: &mut Ui, row_r: Rect, expanded: bool) -> bool {
    let resp = ui.interact(row_r, Id::new("agents-menu-more-row"), Sense::click());
    keep_arrows_on_row(ui, &resp);
    let look = crate::widgets::kit::menu_row_look(false, resp.hovered() || resp.has_focus(), false);
    let p = ui.painter().clone();
    crate::widgets::kit::paint_frame(&p, row_r, theme::RADIUS_CONTROL, &look);
    if resp.has_focus() {
        crate::widgets::kit::focus_ring(&p, row_r, theme::RADIUS_CONTROL as f32);
    }
    p.text(Pos2::new(row_r.left() + theme::INSET_FRAME, row_r.center().y), Align2::LEFT_CENTER, "More…", menu_label_font(), look.text);
    let icon = if expanded { crate::icons::Icon::ChevronUp } else { crate::icons::Icon::ChevronDown };
    let icon_box = Rect::from_center_size(
        Pos2::new(row_r.right() - theme::INSET_FRAME - theme::LAYER_ICON_SIZE * 0.5, row_r.center().y),
        Vec2::splat(theme::LAYER_ICON_SIZE),
    );
    widgets::paint_icon_in(&p, icon_box, icon, theme::LAYER_ICON_SIZE, look.icon);
    widgets::record(ui, "agents-menu-more-row", row_r);
    widgets::labelled(ui, resp.id, egui::accesskit::Role::Button, "More");
    resp.clicked()
}

fn render_other_cli_inline_form(state: &mut TerminalState, ui: &mut Ui, form_r: Rect, interaction: &mut PickerInteraction) {
    let p = ui.painter().clone();
    p.rect_filled(form_r, theme::cr(theme::RADIUS_FRAME), theme::BG_SECONDARY);
    p.rect_stroke(form_r, theme::cr(theme::RADIUS_FRAME), Stroke::new(theme::BORDER_WIDTH, theme::BORDER), StrokeKind::Inside);
    widgets::record(ui, "agents-other-cli-form", form_r);
    // Scrolled or clipped out of the panel's view (a short window puts it under the end of the list), nothing of the form is
    // drawn below: a field that is not drawn cannot be reached with Tab and loses the caret it had, so no letter goes into
    // a form that cannot be seen.
    let view = ui.clip_rect();
    if !view.intersects(form_r) {
        return;
    }

    let pad = theme::INSET_FRAME;
    let field_w = (form_r.width() - pad * 2.0).max(80.0);
    let field_h = OTHER_FORM_FIELD_HEIGHT;
    let field_inset = Vec2::new(theme::INSET_CONTROL, theme::INSET_SEGMENT);

    let lbl_rect = Rect::from_min_size(Pos2::new(form_r.left() + pad, form_r.top() + pad), Vec2::new(field_w, field_h));
    p.rect_filled(lbl_rect, theme::cr(theme::RADIUS_CONTROL), theme::BG_TERTIARY);
    p.rect_stroke(lbl_rect, theme::cr(theme::RADIUS_CONTROL), Stroke::new(theme::BORDER_WIDTH, theme::BORDER), StrokeKind::Inside);
    if view.intersects(lbl_rect) {
        ui.scope_builder(egui::UiBuilder::new().max_rect(lbl_rect.shrink2(field_inset)), |ui| {
            let te = TextEdit::singleline(&mut state.picker.other_label_input)
                .id(Id::new("agents-other-cli-label-input"))
                .desired_width(field_w - theme::SPACE_GROUP)
                .frame(egui::Frame::NONE)
                .font(theme::font_caption())
                .text_color(theme::TEXT)
                .hint_text(RichText::new("Label (e.g. Aider)").font(theme::font_caption()).color(theme::TEXT_TERTIARY));
            let r = ui.add(te);
            if r.clicked() || r.has_focus() {
                state.viewport_focused = false;
            }
        });
    }
    widgets::record(ui, "agents-other-cli-label-input", lbl_rect);

    let cmd_rect = Rect::from_min_size(Pos2::new(form_r.left() + pad, lbl_rect.bottom() + theme::SPACE_ROW), Vec2::new(field_w, field_h));
    p.rect_filled(cmd_rect, theme::cr(theme::RADIUS_CONTROL), theme::BG_TERTIARY);
    p.rect_stroke(cmd_rect, theme::cr(theme::RADIUS_CONTROL), Stroke::new(theme::BORDER_WIDTH, theme::BORDER), StrokeKind::Inside);
    let mut enter_pressed = false;
    if view.intersects(cmd_rect) {
        ui.scope_builder(egui::UiBuilder::new().max_rect(cmd_rect.shrink2(field_inset)), |ui| {
            let te = TextEdit::singleline(&mut state.picker.other_command_input)
                .id(Id::new("agents-other-cli-command-input"))
                .desired_width(field_w - theme::SPACE_GROUP)
                .frame(egui::Frame::NONE)
                .font(theme::font_mono())
                .text_color(theme::TEXT)
                .hint_text(RichText::new("Command (e.g. aider)").font(theme::font_mono()).color(theme::TEXT_TERTIARY));
            let r = ui.add(te);
            if r.clicked() || r.has_focus() {
                state.viewport_focused = false;
            }
            if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                enter_pressed = true;
            }
        });
    }
    widgets::record(ui, "agents-other-cli-command-input", cmd_rect);

    // The caption has a row of its own under the fields and wraps to two lines at the field width, so it never
    // shares the button row. The buttons sit 8 px under its block.
    let caption_block =
        Rect::from_min_size(Pos2::new(form_r.left() + pad, cmd_rect.bottom() + theme::SPACE_ROW), Vec2::new(field_w, OTHER_FORM_CAPTION_HEIGHT));
    let caption_txt = state.picker.other_error.as_deref().unwrap_or(CUSTOM_CLI_WARNING);
    let caption_col = if state.picker.other_error.is_some() { theme::TEXT_WARNING } else { theme::TEXT_TERTIARY };
    let caption_format = egui::TextFormat { line_height: Some(theme::LINE_CAPTION), ..egui::TextFormat::simple(theme::font_caption(), caption_col) };
    let mut caption_job = egui::text::LayoutJob::single_section(caption_txt.to_string(), caption_format);
    caption_job.wrap = egui::text::TextWrapping { max_width: field_w, max_rows: 2, break_anywhere: false, overflow_character: Some('…') };
    let caption = p.layout_job(caption_job);
    let caption_pos = Pos2::new(caption_block.left(), caption_block.center().y - caption.size().y * 0.5);
    p.galley(caption_pos, caption, caption_col);

    let save_w = 48.0;
    let cancel_w = 44.0;
    let btn_y = form_r.bottom() - pad - OTHER_FORM_BUTTON_HEIGHT;
    let save_r = Rect::from_min_size(Pos2::new(form_r.right() - pad - save_w, btn_y), Vec2::new(save_w, OTHER_FORM_BUTTON_HEIGHT));
    let cancel_r = Rect::from_min_size(Pos2::new(save_r.left() - theme::SPACE_FACT - cancel_w, btn_y), Vec2::new(cancel_w, OTHER_FORM_BUTTON_HEIGHT));

    widgets::record(ui, "agents-other-cli-cancel", cancel_r);
    widgets::record(ui, "agents-other-cli-save", save_r);
    let cancel_resp = widgets::kit::button(ui, cancel_r, "agents-other-cli-cancel", "Cancel", widgets::kit::Hierarchy::Secondary, widgets::kit::Size::Xs, true);
    let save_resp = widgets::kit::button(ui, save_r, "agents-other-cli-save", "Save", widgets::kit::Hierarchy::Primary, widgets::kit::Size::Xs, true);

    if cancel_resp.clicked() {
        *interaction = PickerInteraction::CancelOtherForm;
    } else if save_resp.clicked() || enter_pressed {
        *interaction = PickerInteraction::SubmitOtherForm;
    }
}

fn apply_picker_interaction(state: &mut TerminalState, rows: &[PickerRow], interaction: PickerInteraction, file_path: Option<&Path>) {
    match interaction {
        PickerInteraction::None => {}
        PickerInteraction::ResumeLastSession => {
            state.runtime_dropdown_open = false;
            state.picker.on_close();
            if let Err(e) = state.resume_last_session(file_path) {
                state.runtime_dropdown_open = true;
                state.set_resume_failure(&e);
            }
        }
        PickerInteraction::SelectRow(idx) => {
            if let Some(row) = rows.get(idx) {
                let _ = execute_row_selection(state, row, file_path);
            }
        }
        PickerInteraction::ActivateInstall(fix) => {
            state.runtime_dropdown_open = false;
            state.picker.on_close();
            state.apply_start_fix(fix, file_path);
        }
        PickerInteraction::RunLogin(preset) => {
            state.runtime_dropdown_open = false;
            state.picker.on_close();
            state.apply_start_fix(FixAction::RunLogin(login_command(preset)), file_path);
        }
        PickerInteraction::OpenOtherForm => {
            state.picker.menu_more_open = true;
            state.picker.open_other_form();
        }
        PickerInteraction::SubmitOtherForm => {
            let _ = submit_other_cli_form(state, file_path);
        }
        PickerInteraction::CancelOtherForm => {
            state.picker.close_other_form();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_cli_catalog::{CliProbe, SignIn};

    fn temp_test_dir(tag: &str) -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("dos-652-picker-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn seed_test_probes(st: &mut TerminalState) {
        st.picker.cache.set_probe(AgentCliPreset::Claude, CliProbe { installed: true, version: Some("2.1.302".into()), signed_in: SignIn::Yes });
        st.picker.cache.set_probe(AgentCliPreset::Codex, CliProbe { installed: true, version: Some("0.163.0".into()), signed_in: SignIn::Yes });
        st.picker.cache.set_probe(AgentCliPreset::Grok, CliProbe { installed: true, version: Some("1.0.62".into()), signed_in: SignIn::Unknown });
        st.picker.cache.set_probe(AgentCliPreset::Gemini, CliProbe { installed: true, version: Some("0.38.2".into()), signed_in: SignIn::Unknown });
        st.picker.cache.set_probe(AgentCliPreset::Antigravity, CliProbe { installed: false, version: None, signed_in: SignIn::Unknown });
        st.picker.probe_started = true;
    }

    #[test]
    fn failed_resume_last_session_keeps_the_plus_menu_open_with_a_notice() {
        let dir = temp_test_dir("resume-last-fail");
        let mut st = TerminalState::new_for_session("doc-resume-last-fail", "Landing", Some(&dir));
        st.codex_cwd_scoped = false;
        st.session_registry.sessions.push(crate::agent_sessions::SessionEntry {
            tab_id: 1,
            title: "Codex #1".into(),
            cli: "codex".into(),
            session_id: None,
            created_at: 100,
            last_active_at: 200,
            status: crate::agent_sessions::SessionEntryStatus::Exited,
            extra: Default::default(),
        });

        apply_picker_interaction(&mut st, &[], PickerInteraction::ResumeLastSession, None);

        assert!(st.tabs.is_empty());
        assert!(st.runtime_dropdown_open, "menu stays open so the notice is seen");
        assert_eq!(st.resume_notice.as_deref(), Some("Can't resume. Start a new session."));

        st.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // 7. `other_cli_form_saves_and_lists_the_entry`
    #[test]
    fn other_cli_form_saves_and_lists_the_entry() {
        let root = temp_test_dir("other-cli-form");
        let mut st = TerminalState::for_tab(1);
        st.picker.settings_root_override = Some(root.clone());
        seed_test_probes(&mut st);

        // Open the inline "Other CLI…" form and enter label + command
        st.picker.open_other_form();
        st.picker.other_label_input = "Aider".to_string();
        st.picker.other_command_input = "echo AIDER-CUSTOM-TEST".to_string();

        let saved = submit_other_cli_form(&mut st, None).expect("submit custom cli form");
        assert_eq!(saved, CustomCliEntry { label: "Aider".into(), command: "echo AIDER-CUSTOM-TEST".into() });
        assert!(!st.picker.other_form_open, "form must close after saving");

        // Verify persisted in settings.json and listed in picker rows with warning caption
        let rows = rows_for_state(&mut st);
        let custom_row = rows.iter().find(|r| r.label == "Aider").expect("Aider row in picker");
        assert_eq!(custom_row.caption.as_deref(), Some("Plain command: Studio tools not connected"));
        assert!(custom_row.selectable);

        // Never registered as a resumable session (#653 rule 4)
        assert!(st.session_registry.sessions.is_empty());

        st.shutdown();
        let _ = std::fs::remove_dir_all(&root);
    }

    // 13. `plus_menu_shows_the_same_list_compactly`
    #[test]
    fn plus_menu_shows_the_same_list_compactly() {
        let root = temp_test_dir("plus-menu-compact");
        let mut st = TerminalState::for_tab(1);
        st.picker.settings_root_override = Some(root.clone());
        seed_test_probes(&mut st);
        add_custom_cli(&root, "Aider", "aider").expect("seed custom cli");

        let expected_rows = rows_for_state(&mut st);
        let expected_labels: Vec<&str> = expected_rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(expected_labels, vec!["Claude Code", "Codex", "Grok", "Gemini CLI", "Antigravity", "Aider", "Other CLI…", "Shell"]);

        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(260.0, 800.0));

        // Agent menu, collapsed: catalog and saved CLIs, `More…` instead of Other CLI and Shell.
        st.runtime_dropdown_open = true;
        let out_plus =
            ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), ..Default::default() }, |ui| {
                render_runtime_dropdown_menu(&mut st, ui, panel_rect, 40.0, None);
            });
        let (plus_texts, _plus_boxes) = crate::lint::scan_shapes(&out_plus.shapes);
        let plus_all: Vec<&str> = plus_texts.iter().map(|t| t.text.as_str()).collect();
        for lbl in ["Claude Code", "Codex", "Grok", "Gemini CLI", "Antigravity", "Aider", "More…"] {
            assert!(plus_all.iter().any(|t| t.contains(lbl)), "agent menu missing '{lbl}': {plus_all:?}");
        }
        assert!(!plus_all.iter().any(|t| t.contains("Other CLI")), "Other CLI stays behind More: {plus_all:?}");
        assert!(!plus_all.contains(&"Shell"), "Shell stays behind More: {plus_all:?}");
        assert!(!plus_all.iter().any(|t| t.contains("New session")), "no menu header: {plus_all:?}");
        assert!(
            !plus_all.iter().any(|t| t.contains("Ask before acting") || t.contains("Plain command") || t.contains("Signed in")),
            "no footer, caption or status: {plus_all:?}"
        );
        assert!(!plus_all.iter().any(|t| t.contains("2.1.302") || t.contains("0.163.0")), "version stays out of the row: {plus_all:?}");
        let plus_claude_rect = ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", "agents-dropdown-claude".to_string())))).expect("agent menu claude rect");
        assert!((plus_claude_rect.height() - theme::MENU_ITEM_HEIGHT).abs() < 0.5, "agent menu row height is the menu item height");
        assert!(ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", "agents-agent-menu".to_string())))).is_some(), "agent menu records its panel");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&root);
    }

    // 14. `reopening_picker_keeps_cached_result_while_starting_new_probe`
    #[test]
    fn reopening_picker_keeps_cached_result_while_starting_new_probe() {
        use crate::agent_cli_catalog::{CommandOutput, RunCommand, PROBE_TIMEOUT};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use std::time::{Duration, Instant};

        struct CountingRunner {
            calls: AtomicUsize,
        }
        impl RunCommand for CountingRunner {
            fn is_installed(&self, binary: &str, _install_folders: &[PathBuf]) -> bool {
                binary == "claude"
            }
            fn run(&self, argv: &[&str], _install_folders: &[PathBuf], _timeout: Duration) -> CommandOutput {
                if argv == ["claude", "--version"] {
                    let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
                    return CommandOutput::ok(format!("2.1.{n} (Claude Code)\n"));
                }
                if argv == ["claude", "auth", "status"] {
                    return CommandOutput::ok(r#"{"loggedIn": true}"#);
                }
                CommandOutput::exited(1, "", "")
            }
        }

        let runner = Arc::new(CountingRunner { calls: AtomicUsize::new(0) });
        let mut st = TerminalState::for_tab(1);
        st.picker.cache = CatalogProbeCache::new(runner.clone(), PROBE_TIMEOUT);

        // First open: probes and populates v2.1.1
        st.picker.ensure_probed();
        let deadline = Instant::now() + Duration::from_secs(2);
        while st.picker.cache.is_probing() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        let rows1 = rows_for_state(&mut st);
        let claude1 = rows1.iter().find(|r| r.label == "Claude Code").unwrap();
        assert_eq!(claude1.version.as_deref(), Some("2.1.1"));
        assert_eq!(claude1.state_label.as_deref(), Some("Signed in"));

        // Close and reopen: cached v2.1.1 remains on screen while a new probe runs and updates to v2.1.2
        let claude_row = rows1.iter().find(|r| r.label == "Claude Code").unwrap().clone();
        execute_row_selection(&mut st, &claude_row, None);
        st.picker.ensure_probed();
        let cached_snap = st.picker.cache.snapshot();
        assert!(cached_snap.get(&AgentCliPreset::Claude).unwrap().installed, "cached result must stay while re-probing");

        let deadline2 = Instant::now() + Duration::from_secs(2);
        while st.picker.cache.is_probing() && Instant::now() < deadline2 {
            std::thread::sleep(Duration::from_millis(10));
        }
        let rows2 = rows_for_state(&mut st);
        let claude2 = rows2.iter().find(|r| r.label == "Claude Code").unwrap();
        assert_eq!(claude2.version.as_deref(), Some("2.1.2"), "reopening picker must trigger a fresh probe");

        st.shutdown();
    }

    // 15. `custom_cli_saved_in_fallback_mode_uses_library_root`
    #[test]
    fn custom_cli_saved_in_fallback_mode_uses_library_root() {
        let root = temp_test_dir("fallback-shares-lib-root");
        let mut lib_state = TerminalState::for_tab(1);
        lib_state.bind_library_file(&root, "lib-file-1", "win-1");
        seed_test_probes(&mut lib_state);

        lib_state.picker.open_other_form();
        lib_state.picker.other_label_input = "Aider".to_string();
        lib_state.picker.other_command_input = "aider".to_string();
        submit_other_cli_form(&mut lib_state, None).expect("save in library file");

        // Switch to an unsaved / external .fig document (`unbind_library_file` -> `SessionKey::Fallback`)
        lib_state.unbind_library_file("win-1-fallback");
        assert!(matches!(lib_state.key, crate::agent_sessions::SessionKey::Fallback(_)));
        assert_eq!(resolve_settings_root(&lib_state), root, "fallback mode must still resolve to the active library root");

        let rows = rows_for_state(&mut lib_state);
        assert!(rows.iter().any(|r| r.label == "Aider"), "custom CLI must remain visible in unsaved/.fig fallback mode");

        lib_state.shutdown();
        let _ = std::fs::remove_dir_all(&root);
    }

    // #679 4. `uninstalled_cli_opens_its_install_page_not_a_tab`
    #[test]
    fn uninstalled_cli_opens_its_install_page_not_a_tab() {
        let root = temp_test_dir("uninstalled-install-page");
        let mut st = TerminalState::for_tab(1);
        st.picker.settings_root_override = Some(root.clone());
        seed_test_probes(&mut st);
        st.picker.cache.set_probe(AgentCliPreset::Gemini, CliProbe { installed: false, version: None, signed_in: SignIn::Unknown });
        st.picker.cache.set_probe(AgentCliPreset::Grok, CliProbe { installed: false, version: None, signed_in: SignIn::Unknown });

        let rows = rows_for_state(&mut st);

        let gemini_row = rows.iter().find(|r| r.label == "Gemini CLI").expect("Gemini CLI row").clone();
        assert!(!gemini_row.selectable);
        let gemini_fix = execute_row_selection(&mut st, &gemini_row, None);
        assert_eq!(gemini_fix, Some(FixAction::OpenInstallPage("https://github.com/google-gemini/gemini-cli")));
        assert!(st.tabs.is_empty(), "uninstalled Gemini CLI must not spawn a tab");
        assert_eq!(st.status_msg.as_deref(), Some("Open https://github.com/google-gemini/gemini-cli in your browser"));

        let agy_row = rows.iter().find(|r| r.label == "Antigravity").expect("Antigravity row").clone();
        assert!(!agy_row.selectable);
        let agy_fix = execute_row_selection(&mut st, &agy_row, None);
        assert_eq!(agy_fix, Some(FixAction::OpenInstallPage("https://antigravity.google/")));
        assert!(st.tabs.is_empty(), "uninstalled Antigravity must not spawn a tab");
        assert_eq!(st.status_msg.as_deref(), Some("Open https://antigravity.google/ in your browser"));

        let grok_row = rows.iter().find(|r| r.label == "Grok").expect("Grok row").clone();
        assert!(!grok_row.selectable);
        let grok_fix = execute_row_selection(&mut st, &grok_row, None);
        assert_eq!(grok_fix, Some(FixAction::OpenInstallPage("https://x.ai/cli")));
        assert!(st.tabs.is_empty(), "uninstalled Grok must not spawn a tab");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn ask_risky_disables_clis_without_a_shared_hook() {
        let root = temp_test_dir("ask-risky-rows");
        let mut st = TerminalState::for_tab(1);
        st.picker.settings_root_override = Some(root.clone());
        seed_test_probes(&mut st);
        st.permission_mode = crate::terminal::PermissionMode::AskRisky;
        let rows = rows_for_state(&mut st);
        let claude = rows.iter().find(|r| r.label == "Claude Code").expect("Claude");
        assert!(claude.selectable, "Claude shares the approval hook");
        let codex = rows.iter().find(|r| r.label == "Codex").expect("Codex");
        assert!(!codex.selectable);
        assert_eq!(codex.state_label.as_deref(), Some("Unavailable"));
        assert!(codex.caption.as_deref().is_some_and(|c| c.contains("Codex")));
        let grok = rows.iter().find(|r| r.label == "Grok").expect("Grok");
        assert!(!grok.selectable);
        let shell = rows.iter().find(|r| r.label == "Shell").expect("Shell");
        assert!(shell.selectable);
        let other = rows.iter().find(|r| r.kind == PickerRowKind::OtherCli).expect("Other CLI");
        assert!(!other.selectable);
        st.permission_mode = crate::terminal::PermissionMode::Ask;
        let ask_rows = rows_for_state(&mut st);
        assert!(ask_rows.iter().find(|r| r.label == "Codex").expect("Codex").selectable, "Ask still offers Codex");
        st.shutdown();
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn ask_risky_menu_rows_without_a_hook_are_disabled_and_keep_the_reason_in_the_tooltip() {
        let root = temp_test_dir("ask-risky-menu");
        let mut st = TerminalState::for_tab(1);
        st.picker.settings_root_override = Some(root.clone());
        seed_test_probes(&mut st);
        st.permission_mode = crate::terminal::PermissionMode::AskRisky;
        let rows = rows_for_state(&mut st);
        let codex = rows.iter().find(|r| r.label == "Codex").expect("Codex");
        let reason = codex.caption.clone().expect("the reason");
        assert_eq!(row_tooltip(codex).as_deref(), Some(format!("Codex 0.163.0\n{reason}").as_str()), "the version and the reason sit in the tooltip");
        assert!(!is_row_clickable_on_pointer(&st, codex), "an unavailable row does not start an agent");

        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(280.0, 900.0));
        st.runtime_dropdown_open = true;
        let out = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), ..Default::default() }, |ui| {
            render_runtime_dropdown_menu(&mut st, ui, panel_rect, 40.0, None);
        });
        let (texts, _) = crate::lint::scan_shapes(&out.shapes);
        let joined: String = texts.iter().map(|t| t.text.as_str()).collect::<Vec<_>>().join(" ");
        assert!(!joined.contains("Unavailable") && !joined.contains(&reason), "the menu paints no state label and no caption: {joined}");
        assert!(joined.contains("Codex"), "the row itself is still listed: {joined}");
        st.shutdown();
        let _ = std::fs::remove_dir_all(&root);
    }

    // `an_install_action_opens_the_cli_install_page`
    #[test]
    fn an_install_action_opens_the_cli_install_page() {
        let root = temp_test_dir("install-action");
        let mut st = TerminalState::for_tab(1);
        st.picker.settings_root_override = Some(root.clone());
        seed_test_probes(&mut st);
        let agy = rows_for_state(&mut st).into_iter().find(|r| r.label == "Antigravity").expect("Antigravity row");
        assert_eq!(agy.install_action, Some(FixAction::OpenInstallPage("https://antigravity.google/")), "a missing CLI carries its install page");
        assert!(!agy.selectable, "and the row itself is disabled");

        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0));
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(280.0, 800.0));
        let frame = |st: &mut TerminalState, events: Vec<egui::Event>| {
            let _ = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), events, ..Default::default() }, |ui| {
                render_runtime_dropdown_menu(st, ui, panel_rect, 40.0, None);
            });
        };
        st.runtime_dropdown_open = true;
        frame(&mut st, vec![]);
        let install: Rect = ctx.data(|d| d.get_temp(Id::new(("test-rect", format!("{}-install", agy.id))))).expect("the row's Install button");
        assert!(install.height() >= theme::TARGET_MIN, "Install is a real target: {install:?}");
        let at = install.center();
        let click = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        frame(&mut st, vec![egui::Event::PointerMoved(at)]);
        frame(&mut st, vec![click(true), click(false)]);
        assert_eq!(st.status_msg.as_deref(), Some("Open https://antigravity.google/ in your browser"), "the click opened the install page");
        assert!(st.tabs.is_empty(), "and started no tab");
        st.shutdown();
        let _ = std::fs::remove_dir_all(&root);
    }

    // #783: `clicking_row_mid_staggered_probe_starts_the_clicked_cli`
    #[test]
    fn clicking_row_mid_staggered_probe_starts_the_clicked_cli() {
        use crate::agent_cli_catalog::{CommandOutput, RunCommand, PROBE_TIMEOUT};
        use std::sync::Arc;
        use std::time::{Duration, Instant};

        struct StaggeredRunner;
        impl RunCommand for StaggeredRunner {
            fn is_installed(&self, _binary: &str, _install_folders: &[PathBuf]) -> bool {
                true
            }
            fn run(&self, argv: &[&str], _install_folders: &[PathBuf], _timeout: Duration) -> CommandOutput {
                match argv {
                    // Claude and Grok finish after a short delay (25ms), after frame 1 paints
                    // initial "Checking…" rows, while Codex takes longer (250ms).
                    ["claude", "--version"] => {
                        std::thread::sleep(Duration::from_millis(25));
                        CommandOutput::ok("2.1.302 (Claude Code)\n")
                    }
                    ["claude", "auth", "status"] => CommandOutput::ok(r#"{"loggedIn": true}"#),
                    ["grok", "--version"] => {
                        std::thread::sleep(Duration::from_millis(25));
                        CommandOutput::ok("grok 1.0.62\n")
                    }
                    ["codex", "--version"] => {
                        std::thread::sleep(Duration::from_millis(250));
                        CommandOutput::ok("codex-cli 0.163.0\n")
                    }
                    ["codex", "login", "status"] => CommandOutput::ok("Logged in using ChatGPT\n"),
                    ["agy", "--version"] => {
                        std::thread::sleep(Duration::from_millis(250));
                        CommandOutput::ok("1.4.2\n")
                    }
                    ["gemini", "--version"] => {
                        std::thread::sleep(Duration::from_millis(250));
                        CommandOutput::ok("0.38.2\n")
                    }
                    _ => CommandOutput::exited(1, "", ""),
                }
            }
        }

        let root = temp_test_dir("staggered-probe-click");
        let mut st = TerminalState::for_tab(783);
        st.picker.settings_root_override = Some(root.clone());
        st.picker.cache = CatalogProbeCache::new(Arc::new(StaggeredRunner), PROBE_TIMEOUT);
        st.runtime_dropdown_open = true;

        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(260.0, 800.0));
        let screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0)));

        // Frame 1: open menu while all probes are in flight; record where Codex is painted.
        let _ = ctx.run_ui(egui::RawInput { screen_rect, ..Default::default() }, |ui| {
            render_runtime_dropdown_menu(&mut st, ui, panel_rect, 40.0, None);
        });
        let codex_rect = ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", "agents-dropdown-codex".to_string())))).expect("codex row painted on frame 1");
        let click_pos = codex_rect.center();

        // Wait until Claude and Grok probes have finished while Codex is still probing mid-flight.
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            let snap = st.picker.cache.snapshot();
            let claude_done = snap.get(&AgentCliPreset::Claude).is_some_and(|p| p.installed);
            let grok_done = snap.get(&AgentCliPreset::Grok).is_some_and(|p| p.installed);
            if claude_done && grok_done {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(st.picker.cache.is_probing(), "Codex probe must still be in flight when clicking");

        // Click at `codex_rect.center()` mid-probe (move -> press -> release).
        for events in [
            vec![egui::Event::PointerMoved(click_pos)],
            vec![egui::Event::PointerButton { pos: click_pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE }],
            vec![egui::Event::PointerButton { pos: click_pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE }],
        ] {
            let _ = ctx.run_ui(egui::RawInput { screen_rect, events, ..Default::default() }, |ui| {
                render_runtime_dropdown_menu(&mut st, ui, panel_rect, 40.0, None);
            });
        }

        assert_eq!(st.active_runtime, AgentCliPreset::Codex, "clicking Codex row mid-probe must start Codex, not a row that shifted into its slot");

        st.shutdown();
        let _ = std::fs::remove_dir_all(&st.session_dir);
        let _ = std::fs::remove_dir_all(&root);
    }

    fn position_of(rows: &[PickerRow], id: &str) -> usize {
        rows.iter().position(|r| r.id == id).unwrap_or_else(|| panic!("row {id} missing"))
    }

    // #783 + #652 rule 4: a cold open (every probe `Checking`) must still end in spec order once probes settle.
    #[test]
    fn cold_open_settles_to_spec_order_then_stays_pinned() {
        let root = temp_test_dir("cold-open-order");
        let mut st = TerminalState::for_tab(784);
        st.picker.settings_root_override = Some(root.clone());
        st.picker.probe_started = true; // no background probe: the test settles probes by hand
        st.session_registry.last_cli = Some("grok".to_string());

        // Frame 1: `refresh_async` seeds every catalog preset as `Checking`; mirror that here.
        for preset in [AgentCliPreset::Claude, AgentCliPreset::Codex, AgentCliPreset::Grok, AgentCliPreset::Antigravity, AgentCliPreset::Gemini] {
            st.picker.cache.set_probe(preset, CliProbe { installed: false, version: None, signed_in: SignIn::Checking });
        }
        let cold = rows_for_state(&mut st);
        assert!(position_of(&cold, "agents-dropdown-claude") < position_of(&cold, "agents-dropdown-codex"));

        // Claude settles first; the Checking phase must not shuffle rows.
        st.picker.cache.set_probe(AgentCliPreset::Claude, CliProbe { installed: true, version: Some("2.1.302".into()), signed_in: SignIn::Yes });
        let mid = rows_for_state(&mut st);
        assert_eq!(
            mid.iter().map(|r| r.id.clone()).collect::<Vec<_>>(),
            cold.iter().map(|r| r.id.clone()).collect::<Vec<_>>(),
            "rows must not move while probes are still running"
        );

        // Remaining probes settle: Codex is NOT installed, Grok is the last choice.
        st.picker.cache.set_probe(AgentCliPreset::Codex, CliProbe { installed: false, version: None, signed_in: SignIn::Unknown });
        st.picker.cache.set_probe(AgentCliPreset::Grok, CliProbe { installed: true, version: Some("1.0.62".into()), signed_in: SignIn::Yes });
        st.picker.cache.set_probe(AgentCliPreset::Antigravity, CliProbe { installed: true, version: Some("1.4.2".into()), signed_in: SignIn::Unknown });
        st.picker.cache.set_probe(AgentCliPreset::Gemini, CliProbe { installed: true, version: Some("0.38.2".into()), signed_in: SignIn::Unknown });
        let settled = rows_for_state(&mut st);
        let ids: Vec<&str> = settled.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids[0], "agents-dropdown-grok", "last choice first among installed: {ids:?}");
        assert!(settled[0].is_last_choice);
        let codex = position_of(&settled, "agents-dropdown-codex");
        for installed in ["claude", "grok", "antigravity", "gemini"] {
            assert!(position_of(&settled, &format!("agents-dropdown-{installed}")) < codex, "{installed} must precede uninstalled Codex: {ids:?}");
        }

        // Once settled the order is pinned: a later probe flip must not move rows.
        st.picker.cache.set_probe(AgentCliPreset::Codex, CliProbe { installed: true, version: Some("0.163.0".into()), signed_in: SignIn::Yes });
        let later = rows_for_state(&mut st);
        assert_eq!(later.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ids);

        let _ = std::fs::remove_dir_all(&st.session_dir);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn arrow_steps_skip_disabled_rows_and_wrap() {
        // rows: 0 on, 1 off, 2 off, 3 on, 4 on, 5 off
        let rows = [true, false, false, true, true, false];
        assert_eq!(step_row(&rows, 0, true), Some(3), "down skips the disabled rows");
        assert_eq!(step_row(&rows, 3, true), Some(4));
        assert_eq!(step_row(&rows, 4, true), Some(0), "down wraps from the last enabled row to the first");
        assert_eq!(step_row(&rows, 0, false), Some(4), "up wraps from the first enabled row to the last");
        assert_eq!(step_row(&rows, 4, false), Some(3));
        assert_eq!(step_row(&rows, 3, false), Some(0), "up skips the disabled rows");
        assert_eq!(step_row(&rows, 1, true), Some(3), "from a disabled row down goes to the next enabled row");
        assert_eq!(step_row(&rows, 5, false), Some(4), "from a disabled row up goes to the previous enabled row");
        assert_eq!(step_row(&[true], 0, true), Some(0), "a lone enabled row keeps focus");
        assert_eq!(step_row(&[false, false], 0, true), None, "nothing to step to");
        assert_eq!(step_row(&[], 0, true), None);
    }

    /// Where the open `Other CLI…` form is painted.
    #[derive(Clone, Copy, Debug)]
    enum FormSurface {
        /// The agent menu above the composer pill.
        PlusMenu,
    }

    /// What one frame of the open `Other CLI…` form painted.
    struct PaintedForm {
        /// Text runs inside the form card, other than the two fields' own text.
        texts: Vec<crate::lint::TextRun>,
        card: Rect,
        cancel: Rect,
        save: Rect,
    }

    /// Distance between two rects along their widest gap: 0 when they touch or overlap.
    fn gap_between(a: Rect, b: Rect) -> f32 {
        let dx = (a.left() - b.right()).max(b.left() - a.right()).max(0.0);
        let dy = (a.top() - b.bottom()).max(b.top() - a.bottom()).max(0.0);
        dx.max(dy)
    }

    fn paint_other_form(surface: FormSurface, panel_w: f32, error: Option<&str>) -> PaintedForm {
        let root = temp_test_dir("other-form-layout");
        let mut st = TerminalState::for_tab(1);
        st.picker.settings_root_override = Some(root.clone());
        seed_test_probes(&mut st);
        st.picker.open_other_form();
        st.picker.other_error = error.map(str::to_string);
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(panel_w, 800.0));
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), ..Default::default() };
        let out = ctx.run_ui(raw, |ui| match surface {
            FormSurface::PlusMenu => {
                st.runtime_dropdown_open = true;
                render_runtime_dropdown_menu(&mut st, ui, panel_rect, 40.0, None);
            }
        });
        let rect_of =
            |name: &str| ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", name.to_string())))).unwrap_or_else(|| panic!("nothing recorded for {name}"));
        let card = rect_of("agents-other-cli-form");
        let fields = [rect_of("agents-other-cli-label-input"), rect_of("agents-other-cli-command-input")];
        let (texts, _) = crate::lint::scan_shapes(&out.shapes);
        let texts = texts.into_iter().filter(|t| card.contains(t.rect.center()) && !fields.iter().any(|f| f.contains(t.rect.center()))).collect();
        let painted = PaintedForm { texts, card, cancel: rect_of("agents-other-cli-cancel"), save: rect_of("agents-other-cli-save") };
        st.shutdown();
        let _ = std::fs::remove_dir_all(&root);
        painted
    }

    /// The caption of the form: every text run in the card that is not a button label.
    fn caption_of(form: &PaintedForm) -> crate::lint::TextRun {
        let mut runs = form.texts.iter().filter(|t| t.text != "Cancel" && t.text != "Save");
        let caption = runs.next().expect("the form paints a caption").clone();
        assert!(runs.next().is_none(), "the form paints one caption, found more: {:?}", form.texts);
        caption
    }

    #[test]
    fn closing_the_picker_discards_the_other_cli_draft() {
        let mut st = TerminalState::for_tab(1);
        st.picker.open_other_form();
        st.picker.other_label_input = "Aider".into();
        st.picker.other_command_input = "aider --draft".into();
        st.picker.other_error = Some("Enter a command to run.".into());

        st.picker.on_close();

        assert!(!st.picker.other_form_open);
        assert!(st.picker.other_label_input.is_empty() && st.picker.other_command_input.is_empty());
        assert_eq!(st.picker.other_error, None);
        st.shutdown();
    }

    fn one_line(s: &str) -> String {
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn assert_caption_clear_of_buttons(form: &PaintedForm, what: &str) {
        let caption = caption_of(form);
        for (name, button) in [("Cancel", form.cancel), ("Save", form.save)] {
            let gap = gap_between(caption.rect, button);
            assert!(
                gap >= theme::SPACE_INLINE,
                "{what}: the caption ({:?} at {:?}) is {gap} px from {name} at {button:?}, 8 px is the least",
                caption.text,
                caption.rect
            );
        }
        let rows = caption.text.lines().count() as f32;
        assert!(
            (caption.rect.height() - rows * theme::LINE_CAPTION).abs() < 0.5,
            "{what}: a caption row is {} px high, the caption line height is {} px ({:?})",
            caption.rect.height() / rows,
            theme::LINE_CAPTION,
            caption.text
        );
        for (name, button) in [("Cancel", form.cancel), ("Save", form.save)] {
            assert!(button.height() >= theme::TARGET_MIN, "{what}: {name} is {} px high, the least for a target is {}", button.height(), theme::TARGET_MIN);
        }
        assert!(form.card.height() % theme::SPACE_FACT == 0.0, "{what}: the card is {} px high, a multiple of 4 is the rhythm", form.card.height());
        assert!(form.card.contains_rect(caption.rect), "{what}: the caption {:?} stays inside the form card {:?}", caption.rect, form.card);
        assert!(form.card.contains_rect(form.cancel) && form.card.contains_rect(form.save), "{what}: both buttons stay inside the form card {:?}", form.card);
    }

    #[test]
    fn the_other_cli_caption_keeps_8px_from_the_buttons_and_is_not_cut() {
        for (surface, panel_w) in [(FormSurface::PlusMenu, 208.0), (FormSurface::PlusMenu, 260.0), (FormSurface::PlusMenu, 440.0)] {
            let what = format!("{surface:?} in a {panel_w} px panel");
            let form = paint_other_form(surface, panel_w, None);
            assert_caption_clear_of_buttons(&form, &what);
            assert_eq!(one_line(&caption_of(&form).text), CUSTOM_CLI_WARNING, "{what}: the caption is shown whole, not cut with an ellipsis");
        }
    }

    #[test]
    fn a_long_other_cli_error_keeps_8px_from_the_buttons_and_stays_in_the_card() {
        let error = "Enter a command to run, for example aider or ollama run llama3, then save it.";
        for (surface, panel_w) in [(FormSurface::PlusMenu, 208.0), (FormSurface::PlusMenu, 440.0)] {
            let what = format!("{surface:?} in a {panel_w} px panel, error shown");
            let form = paint_other_form(surface, panel_w, Some(error));
            assert_caption_clear_of_buttons(&form, &what);
        }
    }
}

#[cfg(test)]
mod reveal_tests {
    use super::scroll_to_reveal;

    #[test]
    fn a_form_that_fits_ends_at_the_bottom_edge_and_one_that_does_not_starts_at_the_top_edge() {
        assert_eq!(scroll_to_reveal(300.0, 440.0, 360.0, 500.0), 80.0, "a fitting span ends at the viewport's bottom");
        assert_eq!(scroll_to_reveal(300.0, 440.0, 100.0, 500.0), 300.0, "a span taller than the viewport starts at its top");
        assert_eq!(scroll_to_reveal(20.0, 100.0, 360.0, 500.0), 0.0, "a span already in view does not scroll");
        assert_eq!(scroll_to_reveal(300.0, 440.0, 360.0, 50.0), 50.0, "never past the end of the content");
    }
}
