//! One-row agents header: session tabs, Stop, new chat and more.

use crate::agent_stop;
use crate::icons::{self, Icon};
use crate::terminal::{AgentTabStatus, TerminalState, TerminalTab, TERMINAL_HEADER_H, TERM_BG_BAR, TERM_BORDER};
use crate::theme;
use crate::widgets;
use crate::widgets::kit::{self, button_look, paint_frame, status_dot, tab_look, DotTone, Hierarchy};
use egui::{Align2, Id, Pos2, Rect, Sense, Ui, Vec2};

const TAB_H: f32 = 24.0;
const TAB_FLOOR: f32 = 56.0;
const TAB_MANY: f32 = 82.0;
const TAB_ONE_MIN: f32 = 104.0;
const TAB_PAD: f32 = 40.0;
const FIT_SLACK: f32 = 48.0;

struct DrawnTab {
    idx: Option<usize>,
    tab_id: usize,
    title: String,
    /// Screen-reader name: the title, with `(exited)` once the agent ended.
    a11y: String,
    tip: String,
    dot: Option<DotTone>,
    width: f32,
}

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

    let (drawn, hidden_needs) = layout_tabs(state, &p, bar.left() + theme::SPACE_INLINE, tabs_right);
    let mut select_idx: Option<usize> = None;
    let mut close_idx: Option<usize> = None;
    let mut close_new_chat = false;
    let new_chat = state.in_new_chat();
    let mut x = bar.left() + theme::SPACE_INLINE;
    for tab in &drawn {
        let t_rect = Rect::from_min_size(Pos2::new(x, bar.center().y - TAB_H * 0.5), Vec2::new(tab.width, TAB_H));
        let selected = if tab.idx.is_none() { new_chat } else { !new_chat && tab.idx == Some(state.active_tab) };
        let (clicked, close) = paint_tab(ui, &p, t_rect, tab, selected, state.tabs.is_empty());
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
    }

    let plus_resp = ui.interact(plus, Id::new("agents-new-chat-btn"), Sense::click());
    paint_icon_btn(ui, &p, plus, &plus_resp, state.picker_open, "agents-new-chat-btn", "New chat");
    icons::paint(&p, Icon::Plus, plus.center(), theme::LAYER_ICON_SIZE, icon_color(&plus_resp, state.picker_open));
    ui.ctx().data_mut(|d| d.insert_temp(Id::new("agents-new-chat-btn-rect"), plus));

    let more_tip = if hidden_needs > 0 { format!("{hidden_needs} waiting") } else { "More".to_string() };
    let more_resp = ui.interact(more, Id::new("agents-more-btn"), Sense::click()).on_hover_text(more_tip);
    paint_icon_btn(ui, &p, more, &more_resp, state.sessions_popover_open, "agents-more-btn", "More");
    let dot_col = icon_color(&more_resp, state.sessions_popover_open);
    for dx in [-theme::SPACE_FACT, 0.0, theme::SPACE_FACT] {
        p.circle_filled(more.center() + Vec2::new(dx, 0.0), 1.2, dot_col);
    }
    if hidden_needs > 0 {
        let dot_at = Pos2::new(more.right() - theme::SPACE_FACT, more.top() + theme::SPACE_FACT);
        status_dot(&p, dot_at, DotTone::Info);
        let dot_r = Rect::from_center_size(dot_at, Vec2::splat(theme::SPACE_INLINE));
        widgets::record(ui, "agents-more-dot", dot_r);
    }
    ui.ctx().data_mut(|d| d.insert_temp(Id::new("agents-more-btn-rect"), more));

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

fn layout_tabs(state: &TerminalState, p: &egui::Painter, left: f32, right: f32) -> (Vec<DrawnTab>, usize) {
    let now = crate::agent_sessions::unix_timestamp_secs();
    let pending = !state.pending_approvals.is_empty();
    let mut slots: Vec<DrawnTab> = state
        .tabs
        .iter()
        .enumerate()
        .map(|(idx, tab)| DrawnTab {
            idx: Some(idx),
            tab_id: tab.id,
            title: tab.title.clone(),
            a11y: tab.display_title(),
            tip: tab_tip(tab, now),
            dot: tab_dot(tab, idx == state.active_tab, pending),
            width: 0.0,
        })
        .collect();
    if state.in_new_chat() {
        slots.push(DrawnTab {
            idx: None,
            tab_id: 0,
            title: "New chat".to_string(),
            a11y: "New chat".to_string(),
            tip: "New chat".to_string(),
            dot: None,
            width: 0.0,
        });
    }
    let total = slots.len();
    let dots: Vec<Option<DotTone>> = slots.iter().map(|s| s.dot).collect();
    let mut x = left;
    let mut hidden_from = slots.len();
    for (i, slot) in slots.iter_mut().enumerate() {
        let room = right - x;
        if x + FIT_SLACK > right || room < TAB_FLOOR {
            hidden_from = i;
            break;
        }
        let width = if total == 1 {
            let title_w = widgets::text_width(p, &slot.title, &theme::font_caption()) + TAB_PAD;
            TAB_ONE_MIN.max(title_w).min(room.max(TAB_FLOOR))
        } else {
            TAB_MANY.min(((right - x) / (total as f32)).max(TAB_FLOOR)).min(room)
        };
        if width < TAB_FLOOR {
            hidden_from = i;
            break;
        }
        slot.width = width;
        x += width + theme::SPACE_ROW;
    }
    let hidden_needs = dots[hidden_from..].iter().filter(|d| d.is_some()).count();
    let drawn = slots.into_iter().take(hidden_from).filter(|s| s.width >= TAB_FLOOR).collect();
    (drawn, hidden_needs)
}

fn paint_tab(ui: &mut Ui, p: &egui::Painter, t_rect: Rect, tab: &DrawnTab, selected: bool, no_sessions: bool) -> (bool, bool) {
    let close_w = theme::TARGET_MIN;
    let close_rect = Rect::from_min_size(Pos2::new(t_rect.right() - close_w, t_rect.top()), Vec2::new(close_w, t_rect.height()));
    let show_close = tab.idx.is_some() || !no_sessions;
    let hit = if show_close { Rect::from_min_max(t_rect.min, Pos2::new(close_rect.left(), t_rect.bottom())) } else { t_rect };
    let id = if tab.idx.is_none() { Id::new("agents-new-chat-tab") } else { Id::new(("agents-left-tab-select", tab.tab_id)) };
    let resp = ui.interact(hit, id, Sense::click()).on_hover_text(&tab.tip);
    let close_id = if tab.idx.is_none() { Id::new("agents-new-chat-tab-close") } else { Id::new(("agents-left-tab-close", tab.tab_id)) };
    let close_resp = if show_close { Some(ui.interact(close_rect, close_id, Sense::click())) } else { None };
    let hot = resp.hovered() || close_resp.as_ref().is_some_and(|r| r.hovered());
    let state = if selected {
        kit::State::Selected
    } else if hot {
        kit::State::Hover
    } else {
        kit::State::Default
    };
    let paint = tab_look(state);
    paint_frame(p, t_rect, theme::RADIUS_CONTROL, &paint);
    if selected {
        p.rect_filled(
            Rect::from_min_max(
                Pos2::new(t_rect.left() + theme::SPACE_FACT, t_rect.bottom() - theme::INSET_SEGMENT),
                Pos2::new(t_rect.right() - theme::SPACE_FACT, t_rect.bottom()),
            ),
            theme::cr_pill(theme::INSET_SEGMENT),
            theme::BORDER_SELECTED,
        );
    }
    let mut label_x = t_rect.left() + theme::SPACE_INLINE;
    if let Some(tone) = tab.dot {
        let center = Pos2::new(label_x + theme::SPACE_FACT, t_rect.center().y);
        status_dot(p, center, tone);
        widgets::record(ui, &format!("agents-tab-dot-{}", tab.tab_id), Rect::from_center_size(center, Vec2::splat(theme::SPACE_INLINE)));
        label_x = center.x + theme::SPACE_INLINE;
    }
    let label_right = if show_close { close_rect.left() } else { t_rect.right() - theme::SPACE_INLINE };
    let title = widgets::elide(p, &tab.title, &theme::font_caption(), (label_right - label_x).max(theme::SPACE_INLINE));
    let color = if selected { theme::TEXT } else { paint.text };
    p.text(Pos2::new(label_x, t_rect.center().y), Align2::LEFT_CENTER, title, theme::font_caption(), color);
    let focused = ui.memory(|m| m.has_focus(resp.id));
    if focused {
        kit::focus_ring(p, t_rect, theme::RADIUS_CONTROL as f32);
    }
    if let Some(cr) = &close_resp {
        let close_focused = ui.memory(|m| m.has_focus(cr.id));
        if selected || hot || focused || close_focused {
            icons::paint(p, Icon::Close, close_rect.center(), theme::FONT_SIZE_TEXT_XS, theme::ICON_TERTIARY);
        }
        if close_focused {
            kit::focus_ring(p, close_rect, theme::RADIUS_CONTROL as f32);
        }
        widgets::labelled(ui, cr.id, egui::accesskit::Role::Button, "Close tab");
    }
    let record_name = if tab.idx.is_none() { "agents-new-chat-tab" } else { "agents-left-tab-select" };
    widgets::record(ui, record_name, t_rect);
    widgets::labelled(ui, resp.id, egui::accesskit::Role::Tab, &tab.a11y);
    (resp.clicked(), close_resp.is_some_and(|r| r.clicked()))
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

fn tab_dot(tab: &TerminalTab, active: bool, pending: bool) -> Option<DotTone> {
    let crashed = !tab.exited_notice_shown && matches!(tab.effective_status(), AgentTabStatus::Error(_));
    if crashed || tab.start_failure.is_some() || tab.session_failure.is_some() {
        return Some(DotTone::Error);
    }
    if tab.exited_notice_shown {
        return None;
    }
    let waiting = tab.needs_approval() || (active && pending) || matches!(tab.effective_status(), AgentTabStatus::WaitingForInput(_));
    waiting.then_some(DotTone::Info)
}

fn tab_tip(tab: &TerminalTab, now: u64) -> String {
    let mut lines = vec![tab.display_title()];
    if let Some((_, text)) = tab.header_status_indicator(now) {
        lines.push(text);
    }
    if let Some(usage) = tab.usage_tooltip_text() {
        lines.push(usage);
    }
    lines.join("\n")
}
