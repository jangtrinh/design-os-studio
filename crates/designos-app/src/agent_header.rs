//! One-row agents header: session tabs, Stop, new chat and more.

use crate::agent_header_tabs::{layout_tabs, paint_tab};
use crate::agent_stop;
use crate::icons::{self, Icon};
use crate::terminal::{TerminalState, TERMINAL_HEADER_H, TERM_BG_BAR, TERM_BORDER};
use crate::theme;
use crate::widgets;
use crate::widgets::kit::{self, button_look, paint_frame, status_dot, DotTone, Hierarchy};
use egui::{Id, Pos2, Rect, Sense, Ui, Vec2};

const TAB_H: f32 = 24.0;
const MORE_DOT_RADIUS: f32 = 1.2;

/// Paints the header and returns its rect. A press on `+` opens New chat and focuses the composer.
pub fn paint(state: &mut TerminalState, ui: &mut Ui, rect: Rect) -> Rect {
    let bar = Rect::from_min_size(rect.min, Vec2::new(rect.width(), TERMINAL_HEADER_H));
    let p = ui.painter().clone();
    p.rect_filled(bar, 0.0, TERM_BG_BAR);
    widgets::hline(&p, bar.left(), bar.right(), bar.bottom() - theme::BORDER_WIDTH, TERM_BORDER);

    let side = theme::TARGET_MIN;
    let more = Rect::from_center_size(Pos2::new(bar.right() - theme::SPACE_INLINE - side * 0.5, bar.center().y), Vec2::splat(side));
    let plus = Rect::from_center_size(Pos2::new(more.left() - theme::SPACE_ROW - side * 0.5, bar.center().y), Vec2::splat(side));
    let stop_left = agent_stop::paint_header_stop_controls(ui, &p, state, bar, plus.left());
    let tabs_right = stop_left - theme::SPACE_ROW;

    let mut select_idx: Option<usize> = None;
    let mut close_idx: Option<usize> = None;
    let mut close_new_chat = false;
    let new_chat = state.in_new_chat();
    let (active_tab, no_sessions) = (state.active_tab, state.tabs.is_empty());
    let strip_left = bar.left() + theme::SPACE_INLINE;
    let mut x = strip_left;
    let hidden_needs = layout_tabs(state, &p, strip_left, tabs_right, |tab| {
        let t_rect = Rect::from_min_size(Pos2::new(x, bar.center().y - TAB_H * 0.5), Vec2::new(tab.width, TAB_H));
        let selected = if tab.idx.is_none() { new_chat } else { !new_chat && tab.idx == Some(active_tab) };
        let (clicked, close) = paint_tab(ui, &p, t_rect, &tab, selected, no_sessions);
        if close {
            if tab.idx.is_none() {
                close_new_chat = true;
            } else {
                close_idx = tab.idx;
            }
        } else if clicked {
            select_idx = tab.idx;
        }
        x = t_rect.right() + theme::SPACE_ROW;
    });

    let plus_resp = ui.interact(plus, Id::new("agents-new-chat-btn"), Sense::click());
    paint_icon_btn(ui, &p, plus, &plus_resp, state.picker_open, "agents-new-chat-btn", "New chat");
    icons::paint(&p, Icon::Plus, plus.center(), theme::LAYER_ICON_SIZE, icon_color(&plus_resp, state.picker_open));
    crate::agent_composer_menus::remember(ui.ctx(), "agents-new-chat-btn-rect", plus);

    let more_resp = ui.interact(more, Id::new("agents-more-btn"), Sense::click()).on_hover_ui(|ui| {
        if hidden_needs > 0 {
            ui.label(format!("{hidden_needs} waiting"));
        } else {
            ui.label("More");
        }
    });
    paint_icon_btn(ui, &p, more, &more_resp, state.sessions_popover_open, "agents-more-btn", "More");
    let dot_col = icon_color(&more_resp, state.sessions_popover_open);
    for dx in [-theme::SPACE_FACT, 0.0, theme::SPACE_FACT] {
        p.circle_filled(more.center() + Vec2::new(dx, 0.0), MORE_DOT_RADIUS, dot_col);
    }
    if hidden_needs > 0 {
        let dot_at = Pos2::new(more.right() - theme::SPACE_FACT, more.top() + theme::SPACE_FACT);
        status_dot(&p, dot_at, DotTone::Info);
        let dot_r = Rect::from_center_size(dot_at, Vec2::splat(theme::SPACE_INLINE));
        widgets::record(ui, "agents-more-dot", dot_r);
    }
    crate::agent_composer_menus::remember(ui.ctx(), "agents-more-btn-rect", more);

    if close_new_chat {
        state.picker_open = false;
    } else if let Some(idx) = close_idx {
        state.close_tab(idx);
    } else if let Some(idx) = select_idx {
        state.active_tab = idx;
        if let Some(tab) = state.tabs.get_mut(idx) {
            tab.unread = false;
        }
        state.pending_approvals = state.active_tab().map(|t| t.pending_approvals.clone()).unwrap_or_default();
        state.viewport_focused = true;
        state.picker_open = false;
        state.runtime_dropdown_open = false;
        state.sessions_popover_open = false;
        state.picker.on_close();
        state.refresh_tab_usage(idx);
    } else if plus_resp.clicked() {
        state.picker_open = true;
        state.runtime_dropdown_open = false;
        state.attach_menu_open = false;
        state.mode_menu_open = false;
        state.skills.picker_open = false;
        state.sessions_popover_open = false;
        state.resume_notice = None;
        state.picker.on_close();
        state.pending_composer_focus = true;
    } else if more_resp.clicked() {
        state.sessions_popover_open = !state.sessions_popover_open;
        state.resume_notice = None;
        if state.sessions_popover_open {
            state.runtime_dropdown_open = false;
            let pass_nr = ui.ctx().cumulative_pass_nr();
            ui.ctx().data_mut(|d| d.insert_temp(Id::new("agents-sessions-opened-pass"), pass_nr));
            widgets::popup_open::mark_open(ui.ctx());
        } else {
            state.cancel_rename_session();
        }
    }
    bar
}

fn paint_icon_btn(ui: &Ui, p: &egui::Painter, rect: Rect, resp: &egui::Response, selected: bool, name: &str, label: &str) {
    let paint = button_look(Hierarchy::Icon, kit::state_of(resp, selected, true));
    paint_frame(p, rect, theme::RADIUS_CONTROL, &paint);
    if ui.memory(|m| m.has_focus(resp.id)) {
        kit::focus_ring(p, rect, theme::RADIUS_CONTROL as f32);
    }
    widgets::record(ui, name, rect);
    widgets::labelled(ui, resp.id, egui::accesskit::Role::Button, label);
}

fn icon_color(resp: &egui::Response, selected: bool) -> egui::Color32 {
    button_look(Hierarchy::Icon, kit::state_of(resp, selected, true)).icon
}
