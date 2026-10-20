//! Full new-session CLI picker (`CONTROL_ROW` = 40 px) and compact `+` dropdown menu
//! (`CONTROL_STANDARD` = 28 px) sharing the single [`crate::agent_cli_catalog::build_picker_rows`]
//! catalog list, plus the inline `"Other CLI..."` two-field form (`#652`).

use std::path::{Path, PathBuf};

use egui::{Align2, Color32, Id, Key, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, TextEdit, Ui, Vec2};

use crate::agent_cli_catalog::{
    build_picker_rows, CatalogProbeCache, CustomCliEntry, PickerRow, PickerRowKind, SignIn, CUSTOM_CLI_WARNING, DEFAULT_PERMISSION_CAPTION,
};
use crate::agent_start_failure::FixAction;
use crate::comments::settings::{add_custom_cli, load_custom_clis};
use crate::terminal::{
    AgentCliPreset, TerminalState, STATUS_INFO_BG, STATUS_INFO_BORDER, STATUS_INFO_TEXT, STATUS_OK_BG, STATUS_OK_BORDER, STATUS_OK_TEXT, STATUS_WARN_BG,
    STATUS_WARN_BORDER, STATUS_WARN_TEXT,
};
use crate::theme;
use crate::widgets;

/// Per-terminal UI state for the new-session picker, compact `+` menu, and inline `"Other CLI..."` form (`#652`).
#[derive(Clone, Default)]
pub struct AgentPickerState {
    pub cache: CatalogProbeCache,
    pub probe_started: bool,
    pub selected_index: usize,
    pub other_form_open: bool,
    pub other_label_input: String,
    pub other_command_input: String,
    pub other_error: Option<String>,
    pub settings_root_override: Option<PathBuf>,
}

impl AgentPickerState {
    /// Starts the background version + sign-in probe when the picker or `+` menu is opened (`#652` §3 rule 2 & §5).
    /// Cached probe results stay on screen while the fresh probe runs in the background.
    pub fn ensure_probed(&mut self) {
        if !self.probe_started {
            self.probe_started = true;
            self.cache.refresh_async();
        }
    }

    /// Resets the open-latch when the picker or `+` menu closes so the next open triggers a fresh
    /// background probe while keeping the cached result on screen (`#652` §5).
    pub fn on_close(&mut self) {
        self.probe_started = false;
    }

    /// Triggers a fresh background probe (e.g. when re-opening the picker).
    #[allow(dead_code)]
    pub fn refresh(&mut self) {
        self.probe_started = true;
        self.cache.refresh_async();
    }

    /// Opens the inline two-field `"Other CLI..."` form.
    pub fn open_other_form(&mut self) {
        self.other_form_open = true;
        self.other_error = None;
    }

    /// Closes and resets the inline `"Other CLI..."` form.
    pub fn close_other_form(&mut self) {
        self.other_form_open = false;
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
pub fn rows_for_state(state: &mut TerminalState) -> Vec<PickerRow> {
    state.picker.ensure_probed();
    let settings_root = resolve_settings_root(state);
    let custom_clis: Vec<CustomCliEntry> = load_custom_clis(&settings_root);
    let snap = state.picker.cache.snapshot();
    let rows = build_picker_rows(&snap, state.session_registry.last_cli.as_deref(), &custom_clis);
    if state.picker.selected_index >= rows.len() || !rows.get(state.picker.selected_index).is_some_and(|r| r.selectable) {
        if let Some(first_sel) = rows.iter().position(|r| r.selectable) {
            state.picker.selected_index = first_sel;
        }
    }
    rows
}

/// Saves the inline `"Other CLI..."` form (`other_label_input`, `other_command_input`) to Studio's
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
    OpenOtherForm,
    SubmitOtherForm,
    CancelOtherForm,
    ResumeLastSession,
}

/// Executes selecting `row` in the new-session picker or `+` menu.
/// Returns `Some(FixAction)` when an uninstalled CLI row's install action is triggered without spawning a tab.
pub fn execute_row_selection(state: &mut TerminalState, row: &PickerRow, file_path: Option<&Path>) -> Option<FixAction> {
    match &row.kind {
        PickerRowKind::CatalogCli { preset, installed, install_action, .. } => {
            if *installed {
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

fn handle_picker_keyboard(state: &mut TerminalState, ui: &Ui, rows: &[PickerRow], file_path: Option<&Path>) {
    if state.picker.other_form_open || ui.ctx().egui_wants_keyboard_input() {
        if state.picker.other_form_open && ui.input(|i| i.key_pressed(Key::Escape)) {
            state.picker.close_other_form();
        }
        return;
    }
    let selectable_indices: Vec<usize> = rows.iter().enumerate().filter_map(|(i, r)| r.selectable.then_some(i)).collect();
    if selectable_indices.is_empty() {
        return;
    }
    if ui.input(|i| i.key_pressed(Key::ArrowDown)) {
        let cur_pos = selectable_indices.iter().position(|&idx| idx == state.picker.selected_index).unwrap_or(0);
        let next_pos = (cur_pos + 1) % selectable_indices.len();
        state.picker.selected_index = selectable_indices[next_pos];
    } else if ui.input(|i| i.key_pressed(Key::ArrowUp)) {
        let cur_pos = selectable_indices.iter().position(|&idx| idx == state.picker.selected_index).unwrap_or(0);
        let prev_pos = if cur_pos == 0 { selectable_indices.len() - 1 } else { cur_pos - 1 };
        state.picker.selected_index = selectable_indices[prev_pos];
    } else if ui.input(|i| i.key_pressed(Key::Enter)) {
        if let Some(row) = rows.get(state.picker.selected_index) {
            if row.selectable {
                let _ = execute_row_selection(state, row, file_path);
            }
        }
    } else if ui.input(|i| i.key_pressed(Key::Escape)) {
        state.runtime_dropdown_open = false;
        state.picker.on_close();
    }
}

fn state_pill_colors(signed_in: SignIn, installed: bool) -> (Color32, Color32, Color32) {
    if !installed {
        return (theme::BG_TERTIARY, theme::BORDER, theme::TEXT_DISABLED);
    }
    match signed_in {
        SignIn::Yes => (STATUS_OK_BG, STATUS_OK_BORDER, STATUS_OK_TEXT),
        SignIn::No => (STATUS_WARN_BG, STATUS_WARN_BORDER, STATUS_WARN_TEXT),
        SignIn::Unknown | SignIn::Checking => (theme::BG_TERTIARY, theme::BORDER, theme::TEXT_SECONDARY),
    }
}

/// Renders the full new-session picker panel (`CONTROL_ROW` = 40 px rows, `#652` §7).
pub fn show_picker_panel(state: &mut TerminalState, ui: &mut Ui, rect: Rect, file_path: Option<&Path>) {
    let rows = rows_for_state(state);
    if state.picker.cache.is_probing() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(50));
    }
    handle_picker_keyboard(state, ui, &rows, file_path);

    let p = ui.painter().clone();
    let pad = theme::INSET_POPOVER;
    let row_h = theme::CONTROL_ROW;
    let row_gap = theme::SPACE_ROW;
    let form_h = if state.picker.other_form_open { 108.0 } else { 0.0 };
    let header_h = theme::SPACE_SECTION;
    let footer_h = 22.0;
    let content_h = pad * 2.0 + header_h + rows.len() as f32 * (row_h + row_gap) + form_h + footer_h;
    let card_w = (rect.width() - pad * 2.0).clamp(220.0, 380.0);
    let card_h = content_h.min((rect.height() - pad).max(160.0));
    let card_x = rect.left() + ((rect.width() - card_w) * 0.5).max(theme::SPACE_INLINE);
    let card_y = rect.top() + pad;
    let card_r = Rect::from_min_size(Pos2::new(card_x, card_y), Vec2::new(card_w, card_h));

    p.rect_filled(card_r, theme::cr(theme::RADIUS_FRAME), theme::BG_MENU);
    p.rect_stroke(card_r, theme::cr(theme::RADIUS_FRAME), Stroke::new(theme::BORDER_WIDTH, theme::BORDER_MENU), StrokeKind::Inside);
    widgets::record(ui, "agents-new-session-picker", card_r);

    let inner_left = card_r.left() + pad;
    let inner_right = card_r.right() - pad;
    let inner_w = (inner_right - inner_left).max(120.0);
    let mut y = card_r.top() + pad;

    let hdr = widgets::elide(&p, "Start agent session", &theme::font_body_medium(), inner_w);
    p.text(Pos2::new(inner_left, y + theme::SPACE_INLINE), Align2::LEFT_CENTER, hdr, theme::font_body_medium(), theme::TEXT);
    y += header_h;

    let mut interaction = PickerInteraction::None;
    let caption_top_offset = theme::SPACE_GROUP;
    let caption_bottom_offset = theme::INSET_CONTROL;
    let st_reserve = 90.0;

    for (idx, row) in rows.iter().enumerate() {
        let r_rect = Rect::from_min_size(Pos2::new(inner_left, y), Vec2::new(inner_w, row_h));
        let row_id = format!("picker-row-{}", row.id);
        let resp = ui.interact(r_rect, Id::new(&row_id), Sense::click());
        let is_selected = idx == state.picker.selected_index && row.selectable;

        let bg = if is_selected {
            theme::BG_SELECTED
        } else if resp.hovered() && row.selectable {
            theme::BG_HOVER
        } else {
            theme::BG_SECONDARY
        };
        p.rect_filled(r_rect, theme::cr(theme::RADIUS_CONTROL), bg);
        p.rect_stroke(
            r_rect,
            theme::cr(theme::RADIUS_CONTROL),
            Stroke::new(theme::BORDER_WIDTH, if is_selected { STATUS_INFO_BORDER } else { theme::BORDER }),
            StrokeKind::Inside,
        );

        let text_col = if row.selectable { theme::TEXT } else { theme::TEXT_DISABLED };
        let top_line_y = if row.caption.is_some() { r_rect.top() + caption_top_offset } else { r_rect.center().y };

        // Left: Name + Version (`SPACE_INLINE` = 8 px)
        let name_str = widgets::elide(&p, &row.label, &theme::font_body_medium(), (inner_w * 0.45).max(48.0));
        p.text(Pos2::new(r_rect.left() + theme::INSET_CONTROL, top_line_y), Align2::LEFT_CENTER, &name_str, theme::font_body_medium(), text_col);
        let mut left_cursor = r_rect.left() + theme::INSET_CONTROL + widgets::text_width(&p, &name_str, &theme::font_body_medium()) + theme::SPACE_INLINE;

        if let Some(ref ver) = row.version {
            let ver_str = widgets::elide(&p, ver, &theme::font_mono(), 68.0);
            p.text(Pos2::new(left_cursor, top_line_y), Align2::LEFT_CENTER, &ver_str, theme::font_mono(), theme::TEXT_SECONDARY);
            left_cursor += widgets::text_width(&p, &ver_str, &theme::font_mono()) + theme::SPACE_INLINE;
        }
        let _ = left_cursor;

        // Right: State pill OR Install action button (`CONTROL_DENSE` = 24 px)
        if let Some(ref fix) = row.install_action {
            let btn_w = 56.0;
            let btn_r = Rect::from_min_size(
                Pos2::new(r_rect.right() - theme::INSET_CONTROL - btn_w, r_rect.center().y - theme::CONTROL_DENSE * 0.5),
                Vec2::new(btn_w, theme::CONTROL_DENSE),
            );
            let btn_resp = ui.interact(btn_r, Id::new((&row_id, "install-btn")), Sense::click());
            let btn_bg = if btn_resp.hovered() { theme::BG_HOVER } else { STATUS_INFO_BG };
            p.rect_filled(btn_r, theme::cr(theme::RADIUS_CONTROL), btn_bg);
            p.rect_stroke(btn_r, theme::cr(theme::RADIUS_CONTROL), Stroke::new(theme::BORDER_WIDTH, STATUS_INFO_BORDER), StrokeKind::Inside);
            p.text(btn_r.center(), Align2::CENTER_CENTER, "Install", theme::font_caption_medium(), STATUS_INFO_TEXT);
            widgets::record(ui, &format!("{}-install", row.id), btn_r);

            if let Some(ref st_lbl) = row.state_label {
                let st_w = (btn_r.left() - r_rect.left() - st_reserve).max(36.0);
                let st_elided = widgets::elide(&p, st_lbl, &theme::font_caption(), st_w);
                p.text(Pos2::new(btn_r.left() - theme::SPACE_INLINE, top_line_y), Align2::RIGHT_CENTER, st_elided, theme::font_caption(), theme::TEXT_DISABLED);
            }
            if btn_resp.clicked() {
                interaction = PickerInteraction::ActivateInstall(fix.clone());
            }
        } else if let PickerRowKind::CatalogCli { signed_in, installed, .. } = row.kind {
            if let Some(ref st_lbl) = row.state_label {
                let (p_bg, p_border, p_fg) = state_pill_colors(signed_in, installed);
                let lbl_w = widgets::text_width(&p, st_lbl, &theme::font_caption()) + theme::SPACE_GROUP;
                let pill_w = lbl_w.clamp(48.0, (inner_w * 0.46).max(64.0));
                let pill_h = 18.0;
                let pill_r =
                    Rect::from_min_size(Pos2::new(r_rect.right() - theme::INSET_CONTROL - pill_w, top_line_y - pill_h * 0.5), Vec2::new(pill_w, pill_h));
                p.rect_filled(pill_r, theme::cr_pill(pill_r.height()), p_bg);
                p.rect_stroke(pill_r, theme::cr_pill(pill_r.height()), Stroke::new(theme::BORDER_WIDTH, p_border), StrokeKind::Inside);
                let pill_txt = widgets::elide(&p, st_lbl, &theme::font_caption(), (pill_w - theme::SPACE_INLINE).max(24.0));
                p.text(pill_r.center(), Align2::CENTER_CENTER, pill_txt, theme::font_caption(), p_fg);
            }
        }

        if let Some(ref cap) = row.caption {
            let cap_elided = widgets::elide(&p, cap, &theme::font_caption(), (inner_w - theme::INSET_CONTROL * 2.0).max(48.0));
            p.text(
                Pos2::new(r_rect.left() + theme::INSET_CONTROL, r_rect.bottom() - caption_bottom_offset),
                Align2::LEFT_CENTER,
                cap_elided,
                theme::font_caption(),
                theme::TEXT_TERTIARY,
            );
        }

        widgets::record(ui, &row_id, r_rect);
        if resp.clicked() && matches!(interaction, PickerInteraction::None) {
            if row.kind == PickerRowKind::OtherCli {
                interaction = PickerInteraction::OpenOtherForm;
            } else if row.selectable {
                interaction = PickerInteraction::SelectRow(idx);
            }
        }
        y += row_h + row_gap;
    }

    if state.picker.other_form_open {
        let form_r = Rect::from_min_size(Pos2::new(inner_left, y + theme::INSET_SEGMENT), Vec2::new(inner_w, 100.0));
        render_other_cli_inline_form(state, ui, form_r, &mut interaction);
        y += form_h;
    }

    // Footer: Default permission caption (`Skip permissions`, `#652` §3 rule 9)
    let footer_y = (card_r.bottom() - theme::SPACE_GROUP).max(y + theme::SPACE_INLINE);
    let perm_elided = widgets::elide(&p, DEFAULT_PERMISSION_CAPTION, &theme::font_caption(), inner_w);
    p.text(Pos2::new(inner_left, footer_y), Align2::LEFT_CENTER, perm_elided, theme::font_caption(), theme::TEXT_TERTIARY);

    apply_picker_interaction(state, &rows, interaction, file_path);
}

/// Renders the compact `+` dropdown menu (`CONTROL_STANDARD` = 28 px rows, `#652` §3 rule 5 & §7).
pub fn render_runtime_dropdown_menu(state: &mut TerminalState, ui: &mut Ui, panel_rect: Rect, top_y: f32, file_path: Option<&Path>) {
    let rows = rows_for_state(state);
    if state.picker.cache.is_probing() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(50));
    }
    handle_picker_keyboard(state, ui, &rows, file_path);

    let last_resume = state.resumable_sessions().first().map(|e| (e.title.clone(), e.cli.clone()));
    let menu_inset = theme::SPACE_GROUP;
    let menu_edge = theme::INSET_CONTROL;
    let menu_w = (panel_rect.width() - menu_inset).clamp(196.0, 232.0);
    let row_h = theme::CONTROL_STANDARD; // 28 px compact row height
    let row_gap = theme::INSET_SEGMENT;
    let header_h = 22.0;
    let footer_h = 20.0;
    let resume_h = if last_resume.is_some() { row_h + row_gap } else { 0.0 };
    let form_h = if state.picker.other_form_open { 104.0 } else { 0.0 };
    let menu_h = header_h + resume_h + rows.len() as f32 * (row_h + row_gap) + form_h + footer_h + theme::SPACE_INLINE;
    let menu_x = (panel_rect.right() - menu_w - menu_edge).max(panel_rect.left() + menu_edge);
    let menu_r = Rect::from_min_size(Pos2::new(menu_x, top_y), Vec2::new(menu_w, menu_h));

    let p = ui.painter().clone();
    p.rect_filled(menu_r, theme::cr(theme::RADIUS_FRAME), theme::BG_MENU);
    p.rect_stroke(menu_r, theme::cr(theme::RADIUS_FRAME), Stroke::new(theme::BORDER_WIDTH, theme::BORDER_MENU), StrokeKind::Inside);
    widgets::record(ui, "agents-runtime-dropdown-menu", menu_r);

    let inner_left = menu_r.left() + menu_edge;
    let inner_w = (menu_w - menu_edge * 2.0).max(120.0);
    let hdr_txt = widgets::elide(&p, "New session", &theme::font_caption_medium(), inner_w);
    let hdr_center_y = menu_r.top() + header_h * 0.5;
    p.text(Pos2::new(inner_left + theme::SPACE_FACT, hdr_center_y), Align2::LEFT_CENTER, hdr_txt, theme::font_caption_medium(), theme::TEXT_SECONDARY);
    widgets::hline(&p, inner_left, menu_r.right() - menu_edge, menu_r.top() + header_h, theme::BORDER);

    let mut y = menu_r.top() + header_h + theme::SPACE_ROW;
    let mut interaction = PickerInteraction::None;

    if let Some((prev_title, prev_cli)) = &last_resume {
        let row_r = Rect::from_min_size(Pos2::new(inner_left, y), Vec2::new(inner_w, row_h));
        let resp = ui.interact(row_r, Id::new("agents-dropdown-resume-last"), Sense::click());
        if resp.hovered() {
            p.rect_filled(row_r, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
        }
        let resume_txt = format!("Resume {prev_title} ({prev_cli})");
        let elided = widgets::elide(&p, &resume_txt, &theme::font_caption_medium(), (inner_w - theme::INSET_PANEL).max(40.0));
        p.text(Pos2::new(row_r.left() + theme::INSET_CONTROL, row_r.center().y), Align2::LEFT_CENTER, elided, theme::font_caption_medium(), STATUS_OK_TEXT);
        widgets::record(ui, "agents-dropdown-resume-last", row_r);
        if resp.clicked() {
            interaction = PickerInteraction::ResumeLastSession;
        }
        y += row_h + row_gap;
    }

    for (idx, row) in rows.iter().enumerate() {
        let row_r = Rect::from_min_size(Pos2::new(inner_left, y), Vec2::new(inner_w, row_h));
        let resp = ui.interact(row_r, Id::new(&row.id), Sense::click());
        let is_selected = idx == state.picker.selected_index && row.selectable;
        if is_selected {
            p.rect_filled(row_r, theme::cr(theme::RADIUS_CONTROL), theme::BG_SELECTED);
        } else if resp.hovered() && row.selectable {
            p.rect_filled(row_r, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
        }

        let fg = if row.selectable { theme::TEXT } else { theme::TEXT_DISABLED };

        // Right-hand status / action in compact row
        let mut right_limit = row_r.right() - theme::INSET_CONTROL;
        if let Some(ref fix) = row.install_action {
            let btn_w = 48.0;
            let btn_h = 20.0;
            let btn_r = Rect::from_min_size(Pos2::new(row_r.right() - btn_w - theme::SPACE_FACT, row_r.center().y - btn_h * 0.5), Vec2::new(btn_w, btn_h));
            let btn_resp = ui.interact(btn_r, Id::new((&row.id, "install")), Sense::click());
            p.rect_filled(btn_r, theme::cr(theme::RADIUS_CONTROL), if btn_resp.hovered() { theme::BG_HOVER } else { STATUS_INFO_BG });
            p.rect_stroke(btn_r, theme::cr(theme::RADIUS_CONTROL), Stroke::new(theme::BORDER_WIDTH, STATUS_INFO_BORDER), StrokeKind::Inside);
            p.text(btn_r.center(), Align2::CENTER_CENTER, "Install", theme::font_caption(), STATUS_INFO_TEXT);
            widgets::record(ui, &format!("{}-install", row.id), btn_r);
            if btn_resp.clicked() {
                interaction = PickerInteraction::ActivateInstall(fix.clone());
            }
            right_limit = btn_r.left() - theme::SPACE_FACT;
        } else if let Some(ref st_lbl) = row.state_label {
            let st_col = match &row.kind {
                PickerRowKind::CatalogCli { signed_in: SignIn::Yes, .. } => STATUS_OK_TEXT,
                PickerRowKind::CatalogCli { signed_in: SignIn::No, .. } => STATUS_WARN_TEXT,
                _ => theme::TEXT_SECONDARY,
            };
            let max_st_w = (inner_w * 0.44).max(44.0);
            let st_str = widgets::elide(&p, st_lbl, &theme::font_caption(), max_st_w);
            let st_w = widgets::text_width(&p, &st_str, &theme::font_caption());
            p.text(Pos2::new(right_limit, row_r.center().y), Align2::RIGHT_CENTER, &st_str, theme::font_caption(), st_col);
            right_limit -= st_w + theme::SPACE_FACT;
        } else if let Some(ref cap) = row.caption {
            let max_cap_w = (inner_w * 0.48).max(48.0);
            let cap_str = widgets::elide(&p, cap, &theme::font_caption(), max_cap_w);
            let cap_w = widgets::text_width(&p, &cap_str, &theme::font_caption());
            p.text(Pos2::new(right_limit, row_r.center().y), Align2::RIGHT_CENTER, &cap_str, theme::font_caption(), theme::TEXT_TERTIARY);
            right_limit -= cap_w + theme::SPACE_FACT;
        }

        // Left: Label (+ version if available and fits)
        let left_x = row_r.left() + theme::INSET_CONTROL;
        let avail_left_w = (right_limit - left_x).max(36.0);
        let title_str = widgets::elide(&p, &row.label, &theme::font_caption_medium(), avail_left_w);
        let title_w = widgets::text_width(&p, &title_str, &theme::font_caption_medium());
        p.text(Pos2::new(left_x, row_r.center().y), Align2::LEFT_CENTER, &title_str, theme::font_caption_medium(), fg);

        if let Some(ref ver) = row.version {
            let ver_x = left_x + title_w + theme::SPACE_FACT;
            let ver_avail = right_limit - ver_x;
            if ver_avail >= 28.0 {
                let ver_str = widgets::elide(&p, ver, &theme::font_mono(), ver_avail);
                p.text(Pos2::new(ver_x, row_r.center().y), Align2::LEFT_CENTER, ver_str, theme::font_mono(), theme::TEXT_SECONDARY);
            }
        }

        widgets::record(ui, &row.id, row_r);
        if resp.clicked() && matches!(interaction, PickerInteraction::None) {
            if row.kind == PickerRowKind::OtherCli {
                interaction = PickerInteraction::OpenOtherForm;
            } else if row.selectable {
                interaction = PickerInteraction::SelectRow(idx);
            }
        }
        y += row_h + row_gap;
    }

    if state.picker.other_form_open {
        let form_r = Rect::from_min_size(Pos2::new(inner_left, y + theme::INSET_SEGMENT), Vec2::new(inner_w, 98.0));
        render_other_cli_inline_form(state, ui, form_r, &mut interaction);
        y += form_h;
    }

    // Footer permission caption (`Skip permissions`, `#652` §3 rule 9)
    let footer_y = (menu_r.bottom() - theme::INSET_CONTROL).max(y + theme::SPACE_FACT);
    let perm_str = widgets::elide(&p, DEFAULT_PERMISSION_CAPTION, &theme::font_caption(), inner_w - theme::SPACE_INLINE);
    p.text(Pos2::new(inner_left + theme::SPACE_FACT, footer_y), Align2::LEFT_CENTER, perm_str, theme::font_caption(), theme::TEXT_TERTIARY);

    if !matches!(interaction, PickerInteraction::None) {
        apply_picker_interaction(state, &rows, interaction, file_path);
    } else if ui.input(|i| i.pointer.any_pressed()) {
        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
            let add_btn = ui.ctx().data(|d| d.get_temp::<Rect>(Id::new(("test-rect", "agents-runtime-add-btn".to_string()))));
            let in_add = add_btn.map(|r| r.contains(pos)).unwrap_or(false);
            if !menu_r.contains(pos) && !in_add {
                state.runtime_dropdown_open = false;
                state.picker.on_close();
                state.picker.close_other_form();
            }
        }
    }
}

fn render_other_cli_inline_form(state: &mut TerminalState, ui: &mut Ui, form_r: Rect, interaction: &mut PickerInteraction) {
    let p = ui.painter().clone();
    p.rect_filled(form_r, theme::cr(theme::RADIUS_FRAME), theme::BG_SECONDARY);
    p.rect_stroke(form_r, theme::cr(theme::RADIUS_FRAME), Stroke::new(theme::BORDER_WIDTH, theme::BORDER), StrokeKind::Inside);
    widgets::record(ui, "agents-other-cli-form", form_r);

    let pad = theme::INSET_FRAME;
    let field_w = (form_r.width() - pad * 2.0).max(80.0);
    let field_h = 22.0;
    let field_inset = Vec2::new(theme::INSET_CONTROL, theme::INSET_SEGMENT);

    let lbl_rect = Rect::from_min_size(Pos2::new(form_r.left() + pad, form_r.top() + pad), Vec2::new(field_w, field_h));
    p.rect_filled(lbl_rect, theme::cr(theme::RADIUS_CONTROL), theme::BG_TERTIARY);
    p.rect_stroke(lbl_rect, theme::cr(theme::RADIUS_CONTROL), Stroke::new(theme::BORDER_WIDTH, theme::BORDER), StrokeKind::Inside);
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
    widgets::record(ui, "agents-other-cli-label-input", lbl_rect);

    let cmd_rect = Rect::from_min_size(Pos2::new(form_r.left() + pad, lbl_rect.bottom() + theme::SPACE_ROW), Vec2::new(field_w, field_h));
    p.rect_filled(cmd_rect, theme::cr(theme::RADIUS_CONTROL), theme::BG_TERTIARY);
    p.rect_stroke(cmd_rect, theme::cr(theme::RADIUS_CONTROL), Stroke::new(theme::BORDER_WIDTH, theme::BORDER), StrokeKind::Inside);
    let mut enter_pressed = false;
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
    widgets::record(ui, "agents-other-cli-command-input", cmd_rect);

    let warn_y = cmd_rect.bottom() + theme::SPACE_INLINE;
    let warn_txt = state.picker.other_error.as_deref().unwrap_or(CUSTOM_CLI_WARNING);
    let warn_col = if state.picker.other_error.is_some() { STATUS_WARN_TEXT } else { theme::TEXT_TERTIARY };
    let warn_elided = widgets::elide(&p, warn_txt, &theme::font_caption(), (field_w - 96.0).max(36.0));
    p.text(Pos2::new(form_r.left() + pad, warn_y), Align2::LEFT_CENTER, warn_elided, theme::font_caption(), warn_col);

    let save_w = 48.0;
    let cancel_w = 44.0;
    let btn_y = cmd_rect.bottom() + theme::SPACE_ROW;
    let save_r = Rect::from_min_size(Pos2::new(form_r.right() - pad - save_w, btn_y), Vec2::new(save_w, 20.0));
    let cancel_r = Rect::from_min_size(Pos2::new(save_r.left() - theme::SPACE_FACT - cancel_w, btn_y), Vec2::new(cancel_w, 20.0));

    let cancel_resp = ui.interact(cancel_r, Id::new("agents-other-cli-cancel"), Sense::click());
    p.rect_filled(cancel_r, theme::cr(theme::RADIUS_CONTROL), if cancel_resp.hovered() { theme::BG_HOVER } else { theme::BG_TERTIARY });
    p.rect_stroke(cancel_r, theme::cr(theme::RADIUS_CONTROL), Stroke::new(theme::BORDER_WIDTH, theme::BORDER), StrokeKind::Inside);
    p.text(cancel_r.center(), Align2::CENTER_CENTER, "Cancel", theme::font_caption(), theme::TEXT_SECONDARY);
    widgets::record(ui, "agents-other-cli-cancel", cancel_r);

    let save_resp = ui.interact(save_r, Id::new("agents-other-cli-save"), Sense::click());
    p.rect_filled(save_r, theme::cr(theme::RADIUS_CONTROL), if save_resp.hovered() { theme::BG_BRAND_HOVER } else { theme::BG_BRAND });
    p.text(save_r.center(), Align2::CENTER_CENTER, "Save", theme::font_caption_medium(), Color32::WHITE);
    widgets::record(ui, "agents-other-cli-save", save_r);

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
            let _ = state.resume_last_session(file_path);
        }
        PickerInteraction::SelectRow(idx) => {
            state.picker.selected_index = idx;
            if let Some(row) = rows.get(idx) {
                let _ = execute_row_selection(state, row, file_path);
            }
        }
        PickerInteraction::ActivateInstall(fix) => {
            state.runtime_dropdown_open = false;
            state.picker.on_close();
            state.apply_start_fix(fix, file_path);
        }
        PickerInteraction::OpenOtherForm => {
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

    // 7. `other_cli_form_saves_and_lists_the_entry`
    #[test]
    fn other_cli_form_saves_and_lists_the_entry() {
        let root = temp_test_dir("other-cli-form");
        let mut st = TerminalState::for_tab(1);
        st.picker.settings_root_override = Some(root.clone());
        seed_test_probes(&mut st);

        // Open the inline "Other CLI..." form and enter label + command
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
        assert_eq!(expected_labels, vec!["Claude", "Codex", "Grok", "Gemini CLI", "Antigravity", "Aider", "Other CLI...", "Shell"]);

        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let panel_rect = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(260.0, 800.0));

        // 1. Render full picker (`CONTROL_ROW` = 40 px)
        let out_full =
            ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), ..Default::default() }, |ui| {
                show_picker_panel(&mut st, ui, panel_rect, None);
            });
        let (full_texts, _full_boxes) = crate::lint::scan_shapes(&out_full.shapes);
        let full_all: Vec<&str> = full_texts.iter().map(|t| t.text.as_str()).collect();
        for lbl in &expected_labels {
            assert!(full_all.iter().any(|t| t.contains(lbl)), "Full picker missing '{lbl}': {full_all:?}");
        }
        assert!(full_all.iter().any(|t| t.contains("Skip permissions")), "Full picker must show permission caption");
        let full_claude_rect =
            ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", "picker-row-agents-dropdown-claude".to_string())))).expect("full picker claude rect");
        assert!((full_claude_rect.height() - theme::CONTROL_ROW).abs() < 0.5, "Full picker row height must be CONTROL_ROW (40px)");

        // 2. Render compact `+` menu (`CONTROL_STANDARD` = 28 px)
        st.runtime_dropdown_open = true;
        let out_plus =
            ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), ..Default::default() }, |ui| {
                render_runtime_dropdown_menu(&mut st, ui, panel_rect, 40.0, None);
            });
        let (plus_texts, _plus_boxes) = crate::lint::scan_shapes(&out_plus.shapes);
        let plus_all: Vec<&str> = plus_texts.iter().map(|t| t.text.as_str()).collect();
        for lbl in &expected_labels {
            assert!(plus_all.iter().any(|t| t.contains(lbl)), "Compact + menu missing '{lbl}': {plus_all:?}");
        }
        let plus_claude_rect =
            ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", "agents-dropdown-claude".to_string())))).expect("compact + menu claude rect");
        assert!((plus_claude_rect.height() - theme::CONTROL_STANDARD).abs() < 0.5, "Compact + menu row height must be CONTROL_STANDARD (28px)");

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
        let claude1 = rows1.iter().find(|r| r.label == "Claude").unwrap();
        assert_eq!(claude1.version.as_deref(), Some("2.1.1"));
        assert_eq!(claude1.state_label.as_deref(), Some("Signed in"));

        // Close and reopen: cached v2.1.1 remains on screen while a new probe runs and updates to v2.1.2
        let claude_row = rows1.iter().find(|r| r.label == "Claude").unwrap().clone();
        execute_row_selection(&mut st, &claude_row, None);
        st.picker.ensure_probed();
        let cached_snap = st.picker.cache.snapshot();
        assert!(cached_snap.get(&AgentCliPreset::Claude).unwrap().installed, "cached result must stay while re-probing");

        let deadline2 = Instant::now() + Duration::from_secs(2);
        while st.picker.cache.is_probing() && Instant::now() < deadline2 {
            std::thread::sleep(Duration::from_millis(10));
        }
        let rows2 = rows_for_state(&mut st);
        let claude2 = rows2.iter().find(|r| r.label == "Claude").unwrap();
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

        st.shutdown();
        let _ = std::fs::remove_dir_all(&root);
    }
}
