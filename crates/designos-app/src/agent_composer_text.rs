//! The composer's words: the placeholder, the caption under a refusal, and the room the permission mode label needs.

use crate::agent_composer_toolbar::mode_label;
use crate::terminal::{PermissionMode, TerminalState};
use crate::theme;
use crate::widgets;

pub fn placeholder(state: &TerminalState, pill: &str, shell: bool, any_cli: bool) -> String {
    let new_chat = state.in_new_chat();
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
    if let Some(note) = state.still_running_notice_on_screen() {
        return Some(note.to_string());
    }
    let comp = state.active_composer();
    match comp.last_outcome {
        Some(crate::agent_composer::SendOutcome::Refused(reason)) if reason != crate::agent_composer::RefuseReason::NoAgent => {
            comp.notice.clone().or_else(|| Some(reason.message().to_string()))
        }
        _ => None,
    }
}

pub(super) fn wants_icon_mode(p: &egui::Painter, width: f32, mode: PermissionMode) -> bool {
    let fixed = theme::SPACE_ROW * 5.0 + theme::CONTROL_XS * 2.0 + crate::agent_composer_toolbar::PILL_MIN_W;
    width - fixed < mode_width(p, mode)
}

pub(crate) fn mode_width(p: &egui::Painter, mode: PermissionMode) -> f32 {
    let font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold);
    widgets::text_width(p, mode_label(mode), &font) + theme::SPACE_INLINE * 2.0 + theme::LAYER_ICON_SIZE + theme::SPACE_ROW
}
