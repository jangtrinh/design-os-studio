//! Composer toolbar: chip row, add button, agent pill, permission mode, and Send.

use egui::{Align2, Id, Key, Pos2, Rect, Sense, Ui, Vec2};

use crate::agent_selection_chip::{composer_chip_label, selection_fingerprint};
use crate::icons::{self, Icon};
use crate::terminal::{PermissionMode, TerminalState};
use crate::theme;
use crate::widgets::{self, kit};

pub const CHIP_MAX_W: f32 = 148.0;
const CHIP_MIN_W: f32 = 72.0;
const PILL_MIN_W: f32 = 72.0;

pub enum AttachPick {
    Selection,
    Prd,
    Skills,
}

pub struct ToolbarOut {
    pub add: bool,
    pub pill: bool,
    pub mode: bool,
    pub send: bool,
    pub attach: Option<AttachPick>,
    pub mode_pick: Option<PermissionMode>,
}

pub struct ChipOut {
    pub rows: usize,
    pub select_send: bool,
    pub select_hide: bool,
    pub prd_hide: bool,
    pub skills_open: bool,
    pub skills_clear: bool,
    pub drop_layer: Option<designos_doc::NodeId>,
}

pub fn chip_rows(p: &egui::Painter, state: &TerminalState, inner_w: f32) -> usize {
    let widths = chip_widths(p, state);
    wrap_rows(&widths, inner_w)
}

pub fn paint_chips(ui: &mut Ui, state: &TerminalState, row: Rect) -> ChipOut {
    let p = ui.painter().clone();
    let mut out = ChipOut { rows: 0, select_send: false, select_hide: false, prd_hide: false, skills_open: false, skills_clear: false, drop_layer: None };
    let widths = chip_widths(&p, state);
    out.rows = wrap_rows(&widths, row.width());
    let mut x = row.left();
    let mut y = row.top();
    let mut slot = 0usize;
    if selection_visible(state) {
        let w = widths[slot];
        slot += 1;
        let rect = place(&mut x, &mut y, row, w);
        let label = composer_chip_label(&state.composer_selection).unwrap_or_default();
        let hit = paint_tag(
            ui,
            &p,
            rect,
            TagSpec {
                id: "agents-composer-chip-selection",
                label: &label,
                error: false,
                dismiss: Some("Dismiss selection"),
                tip: Some("Send the selection to the agent"),
            },
        );
        widgets::record(ui, "agent-selection-chip", rect);
        #[cfg(test)]
        ui.ctx().data_mut(|d| d.insert_temp(Id::new(("test-text", "agent-selection-chip".to_string())), label.clone()));
        out.select_hide = hit.dismiss;
        out.select_send = hit.body && !hit.dismiss;
    }
    if state.active_composer().attach_prd {
        let w = widths[slot];
        slot += 1;
        let rect = place(&mut x, &mut y, row, w);
        if paint_tag(ui, &p, rect, TagSpec { id: "agents-composer-chip-prd", label: "PRD", error: false, dismiss: Some("Remove PRD"), tip: None }).dismiss {
            out.prd_hide = true;
        }
    }
    let skills_n = state.skills.selected_names.len();
    if skills_n > 0 {
        let w = widths[slot];
        slot += 1;
        let rect = place(&mut x, &mut y, row, w);
        let label = format!("{skills_n} skills");
        let tip = state.skills.selected_names.join(", ");
        let bad = state.skills.limit_violation().is_some();
        let hit =
            paint_tag(ui, &p, rect, TagSpec { id: "agents-composer-chip-skills", label: &label, error: bad, dismiss: Some("Detach skills"), tip: Some(&tip) });
        out.skills_clear = hit.dismiss;
        out.skills_open = hit.body && !hit.dismiss;
    }
    for (i, chip) in state.active_composer().chips.iter().enumerate() {
        let w = widths.get(slot + i).copied().unwrap_or(CHIP_MIN_W);
        let rect = place(&mut x, &mut y, row, w);
        let id = format!("agents-composer-chip-{}", chip.node_id.0);
        if paint_tag(ui, &p, rect, TagSpec { id: &id, label: &chip.name, error: false, dismiss: None, tip: None }).body {
            out.drop_layer = Some(chip.node_id);
        }
        widgets::record(ui, "agents-composer-chip", rect);
    }
    out
}

pub fn toolbar(ui: &mut Ui, state: &mut TerminalState, composer: Rect, pill: &str, send_on: bool, send_tip: Option<&str>) -> ToolbarOut {
    let p = ui.painter().clone();
    let y = composer.bottom() - theme::SPACE_ROW - theme::CONTROL_XS;
    let left = composer.left() + theme::SPACE_ROW;
    let right = composer.right() - theme::SPACE_ROW;
    let mut out = ToolbarOut { add: false, pill: false, mode: false, send: false, attach: None, mode_pick: None };
    let add = Rect::from_min_size(Pos2::new(left, y), Vec2::splat(theme::CONTROL_XS));
    let add_resp = ui.interact(add, Id::new("agents-composer-add-btn"), Sense::click());
    if paint_icon_button(
        ui,
        &p,
        IconPaint {
            rect: add,
            resp: &add_resp,
            id: "agents-composer-add-btn",
            name: "Attach context",
            icon: Icon::Plus,
            selected: state.attach_menu_open,
            enabled: true,
            kind: kit::Hierarchy::Icon,
        },
    ) {
        state.attach_menu_open = !state.attach_menu_open;
        state.mode_menu_open = false;
        if state.attach_menu_open {
            stamp_pass(ui, ATTACH_PASS);
        }
        out.add = true;
    }
    let send = Rect::from_min_size(Pos2::new(right - theme::CONTROL_XS, y), Vec2::splat(theme::CONTROL_XS));
    let send_resp = ui.interact(send, Id::new("agents-composer-send"), if send_on { Sense::click() } else { Sense::hover() });
    paint_icon_button(
        ui,
        &p,
        IconPaint {
            rect: send,
            resp: &send_resp,
            id: "agents-composer-send",
            name: "Send",
            icon: Icon::ArrowUp,
            selected: false,
            enabled: send_on,
            kind: kit::Hierarchy::Primary,
        },
    );
    if let Some(tip) = send_tip {
        send_resp.clone().on_hover_text(tip);
    }
    if send_resp.clicked() && send_on {
        out.send = true;
    }
    let mode_icon = wants_icon_mode(&p, composer.width(), state.permission_mode);
    let mode_w = if mode_icon { theme::CONTROL_XS } else { mode_width(&p, state.permission_mode) };
    let mode = Rect::from_min_size(Pos2::new(send.left() - theme::SPACE_ROW - mode_w, y), Vec2::new(mode_w, theme::CONTROL_XS));
    if mode_btn(ui, &p, mode, state.permission_mode, state.mode_menu_open, mode_icon) {
        state.mode_menu_open = !state.mode_menu_open;
        state.attach_menu_open = false;
        if state.mode_menu_open {
            stamp_pass(ui, MODE_PASS);
        }
        out.mode = true;
    }
    let pill_right = mode.left() - theme::SPACE_ROW;
    let pill_w = (pill_right - add.right() - theme::SPACE_ROW).clamp(PILL_MIN_W, 220.0);
    let pill_r = Rect::from_min_size(Pos2::new(pill_right - pill_w, y), Vec2::new(pill_w, theme::CONTROL_XS));
    let pill_resp = ui.interact(pill_r, Id::new("agents-composer-agent-pill"), Sense::click());
    if paint_text_button(
        ui,
        &p,
        TextPaint { rect: pill_r, resp: &pill_resp, id: "agents-composer-agent-pill", label: pill, selected: state.runtime_dropdown_open, chevron: true },
    ) {
        state.runtime_dropdown_open = !state.runtime_dropdown_open;
        state.attach_menu_open = false;
        state.mode_menu_open = false;
        if state.runtime_dropdown_open {
            stamp_pass(ui, "agents-runtime-opened-pass");
        }
        out.pill = true;
    }
    ui.ctx().data_mut(|d| d.insert_temp(Id::new("agents-composer-agent-pill-rect"), pill_r));
    widgets::labelled(ui, Id::new("agents-composer-agent-pill"), egui::accesskit::Role::Button, &format!("Agent: {pill}"));
    let skills_n = state.skills.selected_names.len();
    let prd_on = state.active_composer().attach_prd;
    if state.attach_menu_open {
        let (pick, stay) = attach_menu(ui, add, prd_on, skills_n);
        state.attach_menu_open = stay;
        out.attach = pick;
    }
    if state.mode_menu_open {
        let (pick, stay) = mode_menu(ui, mode, state.permission_mode);
        state.mode_menu_open = stay;
        out.mode_pick = pick;
    }
    out
}

pub fn placeholder(state: &TerminalState, pill: &str, shell: bool, any_cli: bool) -> String {
    let new_chat = state.picker_open || state.tabs.is_empty();
    if !any_cli && new_chat {
        return "No agent CLI found. Open the agent menu to install one.".to_string();
    }
    if shell && !new_chat {
        return "Select an agent tab".to_string();
    }
    if state.active_composer().pending_send.is_some() {
        return format!("Starting {pill}… your message will be sent");
    }
    if state.active_composer().send_count == 0 {
        format!("Ask {pill}…  @ to mention a layer")
    } else {
        format!("Reply to {pill}…")
    }
}

pub fn caption(state: &TerminalState) -> Option<String> {
    if let Some(note) = state.active_tab().and_then(|t| t.still_running_notice.clone()) {
        return Some(note);
    }
    let comp = state.active_composer();
    match comp.last_outcome {
        Some(crate::agent_composer::SendOutcome::Refused(reason)) if reason != crate::agent_composer::RefuseReason::NoAgent => {
            comp.notice.clone().or_else(|| Some(reason.message().to_string()))
        }
        _ => None,
    }
}

fn selection_visible(state: &TerminalState) -> bool {
    let summary = &state.composer_selection;
    summary.count > 0 && state.active_composer().selection_chip_dismissed != Some(selection_fingerprint(summary))
}

fn chip_widths(p: &egui::Painter, state: &TerminalState) -> Vec<f32> {
    let font = theme::font_caption();
    let mut widths = Vec::new();
    if selection_visible(state) {
        widths.push(tag_w(p, &composer_chip_label(&state.composer_selection).unwrap_or_default(), &font));
    }
    if state.active_composer().attach_prd {
        widths.push(tag_w(p, "PRD", &font));
    }
    let n = state.skills.selected_names.len();
    if n > 0 {
        widths.push(tag_w(p, &format!("{n} skills"), &font));
    }
    for chip in &state.active_composer().chips {
        widths.push(tag_w(p, &chip.name, &font));
    }
    widths
}

fn tag_w(p: &egui::Painter, label: &str, font: &egui::FontId) -> f32 {
    (widgets::text_width(p, label, font) + theme::INSET_BUTTON * 2.0 + theme::TARGET_MIN).clamp(CHIP_MIN_W, CHIP_MAX_W)
}

fn wrap_rows(widths: &[f32], inner_w: f32) -> usize {
    if widths.is_empty() {
        return 0;
    }
    let mut rows = 1usize;
    let mut x = 0.0;
    for w in widths {
        if x > 0.0 && x + w > inner_w {
            rows += 1;
            x = 0.0;
        }
        x += w + theme::SPACE_ROW;
    }
    rows
}

fn place(x: &mut f32, y: &mut f32, row: Rect, w: f32) -> Rect {
    if *x > row.left() && *x + w > row.right() {
        *x = row.left();
        *y += theme::BADGE_MD + theme::SPACE_ROW;
    }
    let rect = Rect::from_min_size(Pos2::new(*x, *y), Vec2::new(w, theme::BADGE_MD));
    *x = rect.right() + theme::SPACE_ROW;
    rect
}

struct TagHit {
    body: bool,
    dismiss: bool,
}

struct TagSpec<'a> {
    id: &'a str,
    label: &'a str,
    error: bool,
    dismiss: Option<&'a str>,
    tip: Option<&'a str>,
}

fn paint_tag(ui: &mut Ui, p: &egui::Painter, rect: Rect, spec: TagSpec<'_>) -> TagHit {
    let resp = ui.interact(rect, Id::new(spec.id), Sense::click());
    let fill = if spec.error { theme::BG_ERROR_PRIMARY } else { theme::BG };
    let border = if spec.error { theme::BADGE_ERROR_BORDER } else { theme::BORDER_STRONG };
    let text = if spec.error { theme::BADGE_ERROR_TEXT } else { theme::TEXT_SECONDARY };
    p.rect_filled(rect, theme::cr_pill(theme::BADGE_MD), fill);
    p.rect_stroke(rect, theme::cr_pill(theme::BADGE_MD), egui::Stroke::new(theme::BORDER_WIDTH, border), egui::StrokeKind::Inside);
    let label_w = if spec.dismiss.is_some() { rect.width() - theme::TARGET_MIN - theme::SPACE_ROW } else { rect.width() - theme::SPACE_INLINE };
    let shown = widgets::elide(p, spec.label, &theme::font_caption(), label_w.max(theme::SPACE_BLOCK));
    p.text(Pos2::new(rect.left() + theme::SPACE_ROW, rect.center().y), Align2::LEFT_CENTER, shown, theme::font_caption(), text);
    widgets::record(ui, spec.id, rect);
    if resp.has_focus() {
        kit::focus_ring(p, rect, theme::RADIUS_CONTROL as f32);
    }
    if let Some(tip) = spec.tip {
        resp.clone().on_hover_text(tip);
    }
    let mut dismiss = false;
    if let Some(name) = spec.dismiss {
        let x = Rect::from_min_size(Pos2::new(rect.right() - theme::TARGET_MIN, rect.top()), Vec2::new(theme::TARGET_MIN, theme::BADGE_MD));
        let xr = ui.interact(x, Id::new(format!("{}-x", spec.id)), Sense::click());
        icons::paint(p, Icon::Close, x.center(), theme::FONT_SIZE_CAPTION, if xr.hovered() { theme::ICON_QUATERNARY_HOVER } else { theme::ICON_QUATERNARY });
        widgets::labelled(ui, xr.id, egui::accesskit::Role::Button, name);
        dismiss = xr.clicked();
    }
    TagHit { body: resp.clicked() && !dismiss, dismiss }
}

struct IconPaint<'a> {
    rect: Rect,
    resp: &'a egui::Response,
    id: &'a str,
    name: &'a str,
    icon: Icon,
    selected: bool,
    enabled: bool,
    kind: kit::Hierarchy,
}

struct TextPaint<'a> {
    rect: Rect,
    resp: &'a egui::Response,
    id: &'a str,
    label: &'a str,
    selected: bool,
    chevron: bool,
}

fn paint_icon_button(ui: &Ui, p: &egui::Painter, spec: IconPaint<'_>) -> bool {
    let look = kit::button_look(spec.kind, kit::state_of(spec.resp, spec.selected, spec.enabled));
    kit::paint_frame(p, spec.rect, theme::RADIUS_CONTROL, &look);
    widgets::paint_icon_in(p, spec.rect, spec.icon, theme::LAYER_ICON_SIZE, look.icon);
    if spec.resp.has_focus() {
        kit::focus_ring(p, spec.rect, theme::RADIUS_CONTROL as f32);
    }
    widgets::record(ui, spec.id, spec.rect);
    widgets::labelled(ui, Id::new(spec.id), egui::accesskit::Role::Button, spec.name);
    spec.resp.clicked() && spec.enabled
}

fn paint_text_button(ui: &Ui, p: &egui::Painter, spec: TextPaint<'_>) -> bool {
    let look = kit::button_look(kit::Hierarchy::Tertiary, kit::state_of(spec.resp, spec.selected, true));
    kit::paint_frame(p, spec.rect, theme::RADIUS_CONTROL, &look);
    let font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold);
    let reserve = if spec.chevron { theme::LAYER_ICON_SIZE + theme::SPACE_ROW } else { 0.0 };
    let shown = widgets::elide(p, spec.label, &font, (spec.rect.width() - theme::SPACE_INLINE * 2.0 - reserve).max(theme::SPACE_BLOCK));
    p.text(Pos2::new(spec.rect.left() + theme::SPACE_INLINE, spec.rect.center().y), Align2::LEFT_CENTER, shown, font, look.text);
    if spec.chevron {
        icons::paint(
            p,
            Icon::ChevronDown,
            Pos2::new(spec.rect.right() - theme::SPACE_INLINE - theme::LAYER_ICON_SIZE * 0.5, spec.rect.center().y),
            theme::LAYER_ICON_SIZE,
            look.icon,
        );
    }
    if spec.resp.has_focus() {
        kit::focus_ring(p, spec.rect, theme::RADIUS_CONTROL as f32);
    }
    widgets::record(ui, spec.id, spec.rect);
    spec.resp.clicked()
}

fn mode_btn(ui: &mut Ui, p: &egui::Painter, rect: Rect, mode: PermissionMode, selected: bool, icon_only: bool) -> bool {
    let resp = ui.interact(rect, Id::new("agents-composer-mode-btn"), Sense::click());
    let icon = if mode == PermissionMode::Skip { Icon::Unlock } else { Icon::Lock };
    let a11y = format!("Permissions: {}", mode_label(mode));
    if icon_only {
        resp.clone().on_hover_text(mode_label(mode));
        paint_icon_button(
            ui,
            p,
            IconPaint { rect, resp: &resp, id: "agents-composer-mode-btn", name: &a11y, icon, selected, enabled: true, kind: kit::Hierarchy::Tertiary },
        )
    } else {
        widgets::labelled(ui, Id::new("agents-composer-mode-btn"), egui::accesskit::Role::Button, &a11y);
        paint_text_button(ui, p, TextPaint { rect, resp: &resp, id: "agents-composer-mode-btn", label: mode_label(mode), selected, chevron: true })
    }
}

fn wants_icon_mode(p: &egui::Painter, width: f32, mode: PermissionMode) -> bool {
    let fixed = theme::SPACE_ROW * 5.0 + theme::CONTROL_XS * 2.0 + PILL_MIN_W;
    width - fixed < mode_width(p, mode)
}

fn mode_width(p: &egui::Painter, mode: PermissionMode) -> f32 {
    let font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold);
    widgets::text_width(p, mode_label(mode), &font) + theme::SPACE_INLINE * 2.0 + theme::LAYER_ICON_SIZE + theme::SPACE_ROW
}

pub fn mode_label(mode: PermissionMode) -> &'static str {
    match mode {
        PermissionMode::Ask => "Ask first",
        PermissionMode::Skip => "Skip permissions",
    }
}

const ATTACH_PASS: &str = "agents-composer-attach-menu-opened-pass";
const MODE_PASS: &str = "agents-composer-mode-menu-opened-pass";
const ADD_SELECTION_TIP: &str = "Sends the selection and its unresolved comments to the agent";

#[derive(Clone, Copy)]
struct AttachRow<'a> {
    id: &'a str,
    label: &'a str,
    caption: Option<&'a str>,
    checked: bool,
    tip: Option<&'a str>,
}

fn attach_menu(ui: &mut Ui, anchor: Rect, prd_on: bool, skills_n: usize) -> (Option<AttachPick>, bool) {
    let skills_caption = (skills_n > 0).then(|| format!("{skills_n} on"));
    let h = theme::INSET_FRAME * 2.0 + 3.0 * theme::MENU_ITEM_HEIGHT;
    let w = 196.0;
    let rect = Rect::from_min_size(Pos2::new(anchor.left(), anchor.top() - theme::SPACE_ROW - h), Vec2::new(w, h));
    menu_frame(ui, rect, "agents-composer-attach-menu");
    let mut pick = None;
    let rows = [
        AttachRow { id: "agents-composer-add-selection", label: "Add selection", caption: None, checked: false, tip: Some(ADD_SELECTION_TIP) },
        AttachRow { id: "agents-composer-attach-prd", label: "Attach PRD", caption: None, checked: prd_on, tip: None },
        AttachRow { id: "agents-composer-skills-item", label: "Skills…", caption: skills_caption.as_deref(), checked: false, tip: None },
    ];
    for (i, row) in rows.iter().enumerate() {
        let AttachRow { id, label, caption, checked, tip } = *row;
        let row = Rect::from_min_size(
            Pos2::new(rect.left() + theme::INSET_FRAME, rect.top() + theme::INSET_FRAME + i as f32 * theme::MENU_ITEM_HEIGHT),
            Vec2::new(w - theme::INSET_FRAME * 2.0, theme::MENU_ITEM_HEIGHT),
        );
        if menu_row(ui, row, id, label, caption, checked, tip) {
            pick = Some(match i {
                0 => AttachPick::Selection,
                1 => AttachPick::Prd,
                _ => AttachPick::Skills,
            });
        }
    }
    let stay = pick.is_none() && !close_menu(ui, rect, ATTACH_PASS);
    (pick, stay)
}

fn mode_menu(ui: &mut Ui, anchor: Rect, current: PermissionMode) -> (Option<PermissionMode>, bool) {
    let modes = [PermissionMode::Skip, PermissionMode::Ask];
    let h = theme::INSET_FRAME * 2.0 + (modes.len() as f32 + 1.0) * theme::MENU_ITEM_HEIGHT;
    let w = 232.0;
    let rect = Rect::from_min_size(Pos2::new(anchor.right() - w, anchor.top() - theme::SPACE_ROW - h), Vec2::new(w, h));
    menu_frame(ui, rect, "agents-composer-mode-menu");
    let mut pick = None;
    for (i, mode) in modes.iter().enumerate() {
        let row = Rect::from_min_size(
            Pos2::new(rect.left() + theme::INSET_FRAME, rect.top() + theme::INSET_FRAME + i as f32 * theme::MENU_ITEM_HEIGHT),
            Vec2::new(w - theme::INSET_FRAME * 2.0, theme::MENU_ITEM_HEIGHT),
        );
        if menu_row(ui, row, mode_id(*mode), mode_label(*mode), None, *mode == current, Some(mode_tip(*mode))) {
            pick = Some(*mode);
        }
    }
    let cap = Rect::from_min_size(
        Pos2::new(rect.left() + theme::INSET_FRAME, rect.bottom() - theme::INSET_FRAME - theme::MENU_ITEM_HEIGHT),
        Vec2::new(w - theme::INSET_FRAME * 2.0, theme::MENU_ITEM_HEIGHT),
    );
    ui.painter().text(
        Pos2::new(cap.left() + theme::SPACE_INLINE, cap.center().y),
        Align2::LEFT_CENTER,
        "Applies to new agent tabs.",
        theme::font_caption(),
        theme::TEXT_TERTIARY,
    );
    let stay = pick.is_none() && !close_menu(ui, rect, MODE_PASS);
    (pick, stay)
}

fn mode_tip(mode: PermissionMode) -> &'static str {
    match mode {
        PermissionMode::Skip => "Launch CLIs with their auto-approve flag. Fastest inside a trusted project.",
        PermissionMode::Ask => "Launch CLIs without skip flags and surface tool approvals in the Agents panel.",
    }
}

fn mode_id(mode: PermissionMode) -> &'static str {
    match mode {
        PermissionMode::Ask => "agents-composer-mode-ask",
        PermissionMode::Skip => "agents-composer-mode-skip",
    }
}

fn menu_frame(ui: &mut Ui, rect: Rect, id: &str) {
    kit::menu_surface(ui.painter(), rect);
    widgets::record(ui, id, rect);
    crate::widgets::popup_open::mark_open(ui.ctx());
}

fn menu_row(ui: &mut Ui, rect: Rect, id: &str, label: &str, caption: Option<&str>, checked: bool, tip: Option<&str>) -> bool {
    let resp = ui.interact(rect, Id::new(id), Sense::click());
    let look = kit::menu_row_look(false, resp.hovered() || resp.has_focus(), checked);
    kit::paint_frame(ui.painter(), rect, theme::RADIUS_CONTROL, &look);
    let p = ui.painter().clone();
    let mut right = rect.right() - theme::SPACE_INLINE;
    if checked {
        let mark = "✓";
        let mark_w = widgets::text_width(&p, mark, &theme::font_caption());
        p.text(Pos2::new(right, rect.center().y), Align2::RIGHT_CENTER, mark, theme::font_caption(), look.mark);
        right -= mark_w + theme::SPACE_ROW;
    }
    if let Some(caption) = caption {
        let cap_w = widgets::text_width(&p, caption, &theme::font_caption());
        p.text(Pos2::new(right, rect.center().y), Align2::RIGHT_CENTER, caption, theme::font_caption(), theme::TEXT_TERTIARY);
        right -= cap_w + theme::SPACE_ROW;
    }
    let shown = widgets::elide(&p, label, &theme::font_caption(), (right - rect.left() - theme::SPACE_INLINE).max(theme::TARGET_MIN));
    p.text(Pos2::new(rect.left() + theme::SPACE_INLINE, rect.center().y), Align2::LEFT_CENTER, shown, theme::font_caption(), look.text);
    if let Some(tip) = tip {
        resp.clone().on_hover_text(tip);
    }
    widgets::record(ui, id, rect);
    resp.clicked()
}

fn stamp_pass(ui: &Ui, id: &str) {
    let pass = ui.ctx().cumulative_pass_nr();
    ui.ctx().data_mut(|d| d.insert_temp(Id::new(id), pass));
}

fn close_menu(ui: &Ui, rect: Rect, pass_id: &str) -> bool {
    if ui.input(|i| i.key_pressed(Key::Escape)) {
        return true;
    }
    let opened_this_pass = ui.ctx().data(|d| d.get_temp::<u64>(Id::new(pass_id))) == Some(ui.ctx().cumulative_pass_nr());
    if opened_this_pass {
        return false;
    }
    if ui.input(|i| i.pointer.any_pressed()) {
        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
            return !rect.contains(pos);
        }
    }
    false
}
