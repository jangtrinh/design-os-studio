//! The session tabs of the agents header: which ones fit, what each shows, and how one is drawn.

use crate::icons::{self, Icon};
use crate::terminal::{AgentTabStatus, TerminalState, TerminalTab};
use crate::theme;
use crate::widgets;
use crate::widgets::kit::{self, paint_frame, status_dot, tab_look, DotTone};
use egui::{Align2, Id, Pos2, Rect, Sense, Ui, Vec2};

const TAB_FLOOR: f32 = 56.0;
const TAB_MANY: f32 = 82.0;
const TAB_ONE_MIN: f32 = 104.0;
const TAB_PAD: f32 = 40.0;
const FIT_SLACK: f32 = 48.0;
/// How far the status dot pushes the title right: the gap before the dot's centre plus the gap after it, as `paint_tab` places them.
const DOT_LABEL_SHIFT: f32 = theme::SPACE_FACT + theme::SPACE_INLINE;
pub(super) const NEW_CHAT_LABEL: &str = "New chat";

/// One tab that fits the strip. `tab` is `None` for the New chat tab; its tooltip and screen-reader name are built when read.
pub(super) struct DrawnTab<'a> {
    pub idx: Option<usize>,
    pub tab: Option<&'a TerminalTab>,
    pub dot: Option<DotTone>,
    pub width: f32,
}

impl DrawnTab<'_> {
    fn title(&self) -> &str {
        self.tab.map_or(NEW_CHAT_LABEL, |t| t.title.as_str())
    }
}

/// Lays the session tabs out between `left` and `right` and hands each one that fits to `draw`, in strip order. The New chat tab
/// claims its width first and sits after the session tabs, so on a narrow strip the session tabs go (they stay reachable from `⋯`)
/// and New chat is the last to be hidden. Returns how many hidden session tabs need the user. A callback instead of a returned `Vec`
/// keeps the strip, which is laid out on every frame the panel is open, free of an allocation.
pub(super) fn layout_tabs<'a>(state: &'a TerminalState, p: &egui::Painter, left: f32, right: f32, mut draw: impl FnMut(DrawnTab<'a>)) -> usize {
    let pending = !state.pending_approvals.is_empty();
    let new_chat = state.in_new_chat();
    let total = state.tabs.len() + usize::from(new_chat);
    let new_chat_w = if !new_chat {
        0.0
    } else if total == 1 {
        (widgets::text_width(p, NEW_CHAT_LABEL, &theme::font_caption()) + TAB_PAD).max(TAB_ONE_MIN).min((right - left).max(TAB_FLOOR))
    } else {
        TAB_MANY.min(((right - left) / (total as f32)).max(TAB_FLOOR))
    };
    let tabs_right = if new_chat && !state.tabs.is_empty() { right - new_chat_w - theme::SPACE_ROW } else { right };
    let mut x = left;
    let mut hidden_from = state.tabs.len();
    for (idx, tab) in state.tabs.iter().enumerate() {
        let room = tabs_right - x;
        if x + FIT_SLACK > tabs_right || room < TAB_FLOOR {
            hidden_from = idx;
            break;
        }
        let dot = tab_dot(tab, idx == state.active_tab, pending);
        let width = if total == 1 {
            // The dot moves the label right (see `paint_tab`): its room counts in the width, or the lone tab elides its own title.
            let dot_w = if dot.is_some() { DOT_LABEL_SHIFT } else { 0.0 };
            let title_w = widgets::text_width(p, &tab.title, &theme::font_caption()) + TAB_PAD + dot_w;
            TAB_ONE_MIN.max(title_w).min(room.max(TAB_FLOOR))
        } else {
            TAB_MANY.min(((right - x) / (total as f32)).max(TAB_FLOOR)).min(room)
        };
        if width < TAB_FLOOR {
            hidden_from = idx;
            break;
        }
        draw(DrawnTab { idx: Some(idx), tab: Some(tab), dot, width });
        x += width + theme::SPACE_ROW;
    }
    if new_chat && new_chat_w >= TAB_FLOOR {
        draw(DrawnTab { idx: None, tab: None, dot: None, width: new_chat_w });
    }
    state.tabs[hidden_from..].iter().enumerate().filter(|(i, t)| tab_dot(t, hidden_from + i == state.active_tab, pending).is_some()).count()
}

/// Paints one tab. Returns `(clicked, close clicked)`.
pub(super) fn paint_tab(ui: &mut Ui, p: &egui::Painter, t_rect: Rect, tab: &DrawnTab<'_>, selected: bool, no_sessions: bool) -> (bool, bool) {
    let close_w = theme::TARGET_MIN;
    let close_rect = Rect::from_min_size(Pos2::new(t_rect.right() - close_w, t_rect.top()), Vec2::new(close_w, t_rect.height()));
    let show_close = tab.idx.is_some() || !no_sessions;
    let hit = if show_close { Rect::from_min_max(t_rect.min, Pos2::new(close_rect.left(), t_rect.bottom())) } else { t_rect };
    let tab_id = tab.tab.map_or(0, |t| t.id);
    let id = if tab.idx.is_none() { Id::new("agents-new-chat-tab") } else { Id::new(("agents-left-tab-select", tab_id)) };
    let resp = ui.interact(hit, id, Sense::click()).on_hover_ui(|ui| {
        match tab.tab {
            Some(t) => ui.label(tab_tip(t, crate::agent_sessions::unix_timestamp_secs())),
            None => ui.label(NEW_CHAT_LABEL),
        };
    });
    let close_id = if tab.idx.is_none() { Id::new("agents-new-chat-tab-close") } else { Id::new(("agents-left-tab-close", tab_id)) };
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
        widgets::record_with(ui, || format!("agents-tab-dot-{tab_id}"), Rect::from_center_size(center, Vec2::splat(theme::SPACE_INLINE)));
        label_x = center.x + theme::SPACE_INLINE;
    }
    let label_right = if show_close { close_rect.left() } else { t_rect.right() - theme::SPACE_INLINE };
    let title = widgets::elide(p, tab.title(), &theme::font_caption(), (label_right - label_x).max(theme::SPACE_INLINE));
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
    if crate::agent_a11y::is_accesskit_active() {
        let name = tab.tab.map_or_else(|| NEW_CHAT_LABEL.to_string(), TerminalTab::display_title);
        widgets::labelled(ui, resp.id, egui::accesskit::Role::Tab, &name);
    }
    (resp.clicked(), close_resp.is_some_and(|r| r.clicked()))
}

/// The one dot a tab carries, only while its session needs the user.
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

/// The tooltip: the full title, the status text, then the usage reading, one per line.
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
