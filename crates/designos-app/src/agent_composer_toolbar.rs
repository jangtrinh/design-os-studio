//! Composer toolbar: add button, agent pill, permission mode, and Send. Menus and chips live in their own modules.

use egui::{Id, Pos2, Rect, Sense, Ui, Vec2};

use crate::agent_composer_buttons::{paint_icon_button, paint_text_button, IconPaint, TextPaint};
use crate::agent_composer_menus::{attach_menu, mode_menu, stamp_pass, AttachPick, ATTACH_PASS, MODE_PASS};
use crate::agent_composer_text::{mode_width, wants_icon_mode};
use crate::icons::Icon;
use crate::terminal::{PermissionMode, TerminalState};
use crate::theme;
use crate::widgets::{self, kit};

pub(super) const PILL_MIN_W: f32 = 72.0;
const PILL_MAX_W: f32 = 220.0;

pub struct ToolbarOut {
    pub add: bool,
    pub pill: bool,
    pub mode: bool,
    pub send: bool,
    pub attach: Option<AttachPick>,
    pub mode_pick: Option<PermissionMode>,
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
    let pill_w = (pill_right - add.right() - theme::SPACE_ROW).clamp(PILL_MIN_W, PILL_MAX_W);
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
        } else {
            state.picker.on_close();
        }
        out.pill = true;
    }
    crate::agent_composer_menus::remember(ui.ctx(), "agents-composer-agent-pill-rect", pill_r);
    if crate::agent_a11y::is_accesskit_active() {
        widgets::labelled(ui, Id::new("agents-composer-agent-pill"), egui::accesskit::Role::Button, &format!("Agent: {pill}"));
    }
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

fn mode_btn(ui: &mut Ui, p: &egui::Painter, rect: Rect, mode: PermissionMode, selected: bool, icon_only: bool) -> bool {
    let resp = ui.interact(rect, Id::new("agents-composer-mode-btn"), Sense::click());
    let icon = if mode == PermissionMode::Skip { Icon::Unlock } else { Icon::Lock };
    let a11y = if crate::agent_a11y::is_accesskit_active() { format!("Permissions: {}", mode_label(mode)) } else { String::new() };
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

pub fn mode_label(mode: PermissionMode) -> &'static str {
    match mode {
        PermissionMode::Ask => "Ask first",
        PermissionMode::Skip => "Skip permissions",
        PermissionMode::AskRisky => "Ask risky only",
    }
}
