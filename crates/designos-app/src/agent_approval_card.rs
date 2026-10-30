//! In-panel approval card and the `"Agent permissions"` preference modal (`#656`).
//!
//! Every colour, spacing, inset, control height, and radius comes from `crate::theme`.

use crate::agent_permissions::{self, AgentPermissionMode, ApprovalRequest};
use crate::terminal::AgentCliPreset;
use crate::theme;
use crate::widgets::{self, kit};
use egui::text::LayoutJob;
use egui::{Align2, Context, Id, Key, Modifiers, Order, Painter, Pos2, Rect, Sense, TextFormat, Ui, Vec2};
use std::path::Path;

/// User action taken on the in-panel approval card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CardAction {
    Approve(String),
    Deny(String),
    ShowPrompt,
    OpenTab(usize),
}

/// Height occupied by the approval card (including its top and bottom `SPACE_ROW` viewport margins).
pub fn approval_card_outer_height(preset: AgentCliPreset) -> f32 {
    let claude = preset == AgentCliPreset::Claude;
    let inner = theme::INSET_BUTTON
        + theme::TARGET_MIN
        + theme::SPACE_INLINE
        + if claude { theme::CONTROL_XS + theme::SPACE_INLINE } else { 0.0 }
        + theme::CONTROL_XS
        + theme::INSET_BUTTON;
    inner + theme::SPACE_ROW * 2.0
}

/// Paints the in-panel approval card pinned to the top of `viewport_rect` (inset by `SPACE_ROW`).
///
/// - For `Claude` on the active tab, shows the tool name, monospace input summary, queue count (`"1 of N"`), and
///   secondary **Deny** + primary **Approve** buttons.
/// - For `Codex`, `Grok`, `Gemini`, and `Antigravity` (`agy`), shows `"<CLI> needs approval in its terminal"`
///   and a single **Show prompt** button when already in that tab's terminal, or **Open tab** when the approval
///   is on another tab or view (never **Approve** or **Deny**).
#[cfg(test)]
pub fn paint_approval_card(
    ui: &mut Ui,
    viewport_rect: Rect,
    preset: AgentCliPreset,
    requests: &[ApprovalRequest],
    surface_id: &'static str,
) -> Option<CardAction> {
    paint_approval_card_for_tab(ui, viewport_rect, preset, requests, 0, preset == AgentCliPreset::Claude, requests.len(), surface_id)
}

/// `1 of N` at the right edge of the title row while more than one request waits. Returns the room it takes.
fn paint_queue_count(p: &egui::Painter, header_r: Rect, queue_len: usize, mono: &egui::FontId) -> f32 {
    if queue_len <= 1 {
        return 0.0;
    }
    let count_text = format!("1 of {queue_len}");
    let width = widgets::text_width(p, &count_text, mono) + theme::SPACE_INLINE;
    p.text(Pos2::new(header_r.right(), header_r.center().y), Align2::RIGHT_CENTER, count_text, mono.clone(), theme::TEXT_TERTIARY);
    width
}

/// Paints the in-panel approval card for `tab_id` (`#656`, `#1195`).
#[allow(clippy::too_many_arguments)]
pub fn paint_approval_card_for_tab(
    ui: &mut Ui,
    viewport_rect: Rect,
    preset: AgentCliPreset,
    requests: &[ApprovalRequest],
    tab_id: usize,
    in_active_terminal: bool,
    total_pending: usize,
    surface_id: &'static str,
) -> Option<CardAction> {
    let first_req = requests.first();
    if (preset == AgentCliPreset::Claude || matches!(preset, AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp)) && first_req.is_none() {
        return None;
    }
    let is_claude = preset == AgentCliPreset::Claude && in_active_terminal;
    let outer_h = approval_card_outer_height(if is_claude { preset } else { AgentCliPreset::Codex });
    let card_h = outer_h - theme::SPACE_ROW * 2.0;
    let card_rect = Rect::from_min_size(
        Pos2::new(viewport_rect.left() + theme::SPACE_ROW, viewport_rect.top() + theme::SPACE_ROW),
        Vec2::new((viewport_rect.width() - theme::SPACE_ROW * 2.0).max(theme::CONTROL_MD), card_h),
    );
    widgets::record(ui, "agent-approval-card", card_rect);

    let p = ui.painter().clone();
    let card_paint = kit::Look {
        fill: theme::BG_SECONDARY,
        border: theme::BORDER,
        border_w: theme::BORDER_WIDTH,
        text: theme::TEXT,
        icon: theme::ICON_SECONDARY,
        mark: egui::Color32::TRANSPARENT,
    };
    kit::paint_frame(&p, card_rect, theme::RADIUS_XL, &card_paint);

    let inner = card_rect.shrink(theme::INSET_BUTTON);
    let header_r = Rect::from_min_size(inner.min, Vec2::new(inner.width(), theme::TARGET_MIN));

    let mono_font = theme::font_mono();
    let count_reserve = paint_queue_count(&p, header_r, total_pending.max(requests.len()), &mono_font);

    let title_font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold);
    let raw_title = if is_claude {
        verb_title(first_req.map(|r| r.tool_name.as_str()).unwrap_or("tool"))
    } else {
        format!("{} needs approval in its terminal", preset.tab_title())
    };
    let title_max_w = (header_r.width() - count_reserve).max(theme::TARGET_MIN);
    let title_elided = widgets::elide(&p, &raw_title, &title_font, title_max_w);
    p.text(Pos2::new(header_r.left(), header_r.center().y), Align2::LEFT_CENTER, title_elided, title_font, theme::TEXT);

    let mut y = header_r.bottom() + theme::SPACE_INLINE;

    if let Some(req) = first_req.filter(|_| is_claude) {
        let sum_r = Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), theme::CONTROL_XS));
        let sum_paint = kit::Look {
            fill: theme::BG,
            border: egui::Color32::TRANSPARENT,
            border_w: 0.0,
            text: theme::TEXT,
            icon: theme::ICON_SECONDARY,
            mark: egui::Color32::TRANSPARENT,
        };
        kit::paint_frame(&p, sum_r, theme::RADIUS_CONTROL, &sum_paint);
        let sum_max_w = (sum_r.width() - theme::INSET_BUTTON * 2.0).max(theme::TARGET_MIN);
        let sum_elided = widgets::elide(&p, &req.input_summary, &mono_font, sum_max_w);
        p.text(Pos2::new(sum_r.left() + theme::INSET_BUTTON, sum_r.center().y), Align2::LEFT_CENTER, sum_elided, mono_font.clone(), theme::TEXT);
        widgets::record(ui, "agent-approval-summary", sum_r);
        y = sum_r.bottom() + theme::SPACE_INLINE;
    }

    let action_r = Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), theme::CONTROL_XS));
    let mut out = None;
    if let Some(req) = first_req.filter(|_| is_claude) {
        let req_id = req.id.clone();
        let (approve_r, approve_resp) =
            paint_card_button(ui, &p, (surface_id, "agent-approval-approve"), "agent-approval-approve", action_r.right(), action_r.top(), "Approve", true);
        let deny_right = approve_r.left() - theme::SPACE_ROW;
        let (_deny_r, deny_resp) =
            paint_card_button(ui, &p, (surface_id, "agent-approval-deny"), "agent-approval-deny", deny_right, action_r.top(), "Deny", false);
        if approve_resp.clicked() {
            out = Some(CardAction::Approve(req_id));
        } else if deny_resp.clicked() {
            out = Some(CardAction::Deny(req_id));
        }
    } else if in_active_terminal {
        let (_show_r, show_resp) = paint_card_button(
            ui,
            &p,
            (surface_id, "agent-approval-show-prompt"),
            "agent-approval-show-prompt",
            action_r.right(),
            action_r.top(),
            "Show prompt",
            false,
        );
        widgets::labelled(ui, show_resp.id, egui::accesskit::Role::Button, &format!("Show the {} prompt", preset.short_agent_name()));
        if show_resp.clicked() {
            out = Some(CardAction::ShowPrompt);
        }
    } else {
        let (_open_r, open_resp) =
            paint_card_button(ui, &p, (surface_id, "agent-approval-open-tab"), "agent-approval-open-tab", action_r.right(), action_r.top(), "Open tab", false);
        widgets::labelled(ui, open_resp.id, egui::accesskit::Role::Button, &format!("Open the {} tab waiting for approval", preset.short_agent_name()));
        if open_resp.clicked() {
            out = Some(CardAction::OpenTab(tab_id));
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn paint_card_button(
    ui: &mut Ui,
    p: &Painter,
    id: impl std::hash::Hash + Copy + std::fmt::Debug,
    record_name: &str,
    right_x: f32,
    top_y: f32,
    label: &str,
    primary: bool,
) -> (Rect, egui::Response) {
    let font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold);
    let w = (widgets::text_width(p, label, &font) + theme::INSET_BUTTON * 2.0).max(theme::CONTROL_MD);
    let rect = Rect::from_min_size(Pos2::new(right_x - w, top_y), Vec2::new(w, theme::CONTROL_XS));
    let resp = ui.interact(rect, Id::new(id), Sense::click());
    let hierarchy = if primary { kit::Hierarchy::Primary } else { kit::Hierarchy::Secondary };
    let look = kit::button_look(hierarchy, kit::state_of(&resp, false, true));
    kit::paint_frame(p, rect, theme::RADIUS_CONTROL, &look);
    if ui.memory(|m| m.has_focus(resp.id)) {
        kit::focus_ring(p, rect, theme::RADIUS_CONTROL as f32);
    }
    p.text(rect.center(), Align2::CENTER_CENTER, label, font, look.text);
    widgets::record(ui, record_name, rect);
    widgets::labelled(ui, resp.id, egui::accesskit::Role::Button, label);
    (rect, resp)
}

fn verb_title(tool: &str) -> String {
    match tool {
        "Bash" => "Run a command".to_string(),
        "Read" => "Read a file".to_string(),
        "Edit" | "MultiEdit" | "Write" | "NotebookEdit" => "Change a file".to_string(),
        other => format!("Use {other}"),
    }
}

/// Renders the `"Agent permissions"` modal opened from `Preferences -> Agent permissions…`
/// or the command palette.
pub fn paint_permissions_modal(ctx: &Context, open: &mut bool, mode: &mut AgentPermissionMode, lib_root: Option<&Path>) -> Option<AgentPermissionMode> {
    if !*open {
        return None;
    }
    let screen = ctx.content_rect();
    let esc = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
    let draft_id = Id::new("agent-permissions-draft");
    let mut draft = ctx.data(|d| d.get_temp::<AgentPermissionMode>(draft_id)).unwrap_or(*mode);
    let caption_lines = 3.0;
    let row_h = theme::INSET_PANEL * 2.0 + theme::LINE_TEXT_SM + theme::SPACE_FACT + theme::LINE_TEXT_SM * caption_lines;
    let modal_h = theme::INSET_MODAL
        + theme::LINE_TEXT_MD
        + theme::SPACE_GROUP
        + row_h
        + theme::SPACE_ROW
        + row_h
        + theme::SPACE_GROUP
        + theme::LINE_TEXT_SM
        + theme::SPACE_GROUP
        + theme::CONTROL_MD
        + theme::INSET_MODAL;
    let rect = Rect::from_center_size(screen.center(), Vec2::new(theme::PERMISSIONS_MODAL_WIDTH, modal_h));

    let mut close_modal = esc;
    let mut commit = false;

    egui::Area::new(Id::new("agent-permissions-modal")).order(Order::Foreground).fixed_pos(screen.min).interactable(true).show(ctx, |ui| {
        let scrim = ui.allocate_rect(screen, Sense::click_and_drag());
        let p = ui.painter().clone();
        kit::modal_surface(&p, screen, rect);
        if scrim.clicked() && !rect.contains(scrim.interact_pointer_pos().unwrap_or(Pos2::ZERO)) {
            close_modal = true;
        }
        widgets::record(ui, "agent-permissions-modal", rect);

        let inner_x = rect.left() + theme::INSET_MODAL;
        let inner_w = rect.width() - theme::INSET_MODAL - theme::INSET_MODAL;
        let mut y = rect.top() + theme::INSET_MODAL;

        let title_font = theme::font_text(theme::FONT_SIZE_TEXT_MD, theme::TextWeight::Semibold);
        p.text(Pos2::new(inner_x, y), Align2::LEFT_TOP, "Agent permissions", title_font, theme::TEXT);
        y += theme::LINE_TEXT_MD + theme::SPACE_GROUP;

        let options = [
            (AgentPermissionMode::Skip, "agent-perm-skip", "Skip permissions", "Launch CLIs with their auto-approve flag. Fastest inside a trusted project."),
            (
                AgentPermissionMode::Ask,
                "agent-perm-ask",
                "Ask before acting (default)",
                "Launch CLIs without skip flags and surface tool approvals in the Agents panel.",
            ),
        ];

        for (opt_mode, widget_id, title, caption) in options {
            let row_r = Rect::from_min_size(Pos2::new(inner_x, y), Vec2::new(inner_w, row_h));
            let selected = draft == opt_mode;
            let resp = kit::radio_card(ui, row_r, widget_id, "", selected, true);
            let text_x = row_r.left() + theme::INSET_PANEL + theme::LAYER_ICON_SIZE + theme::SPACE_INLINE;
            let text_w = (row_r.right() - theme::INSET_PANEL - text_x).max(theme::TARGET_MIN);
            let title_y = row_r.top() + theme::INSET_PANEL;
            let option_font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Medium);
            let option_color = if selected { theme::TEXT } else { theme::TEXT_SECONDARY };
            p.text(Pos2::new(text_x, title_y), Align2::LEFT_TOP, title, option_font, option_color);

            let caption_font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Regular);
            let mut job = LayoutJob::simple(caption.to_string(), caption_font.clone(), theme::TEXT_SECONDARY, text_w);
            if let Some(sec) = job.sections.first_mut() {
                sec.format = TextFormat { font_id: caption_font, color: theme::TEXT_SECONDARY, line_height: Some(theme::LINE_TEXT_SM), ..Default::default() };
            }
            let galley = ui.fonts_mut(|f| f.layout_job(job));
            p.galley(Pos2::new(text_x, title_y + theme::LINE_TEXT_SM + theme::SPACE_FACT), galley, theme::TEXT_SECONDARY);

            widgets::record(ui, widget_id, row_r);
            if resp.clicked() {
                draft = opt_mode;
            }
            y += row_h + theme::SPACE_ROW;
        }

        y = y - theme::SPACE_ROW + theme::SPACE_GROUP;
        let note_font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Regular);
        p.text(Pos2::new(inner_x, y), Align2::LEFT_TOP, "Applies to new agent tabs.", note_font, theme::TEXT_SECONDARY);

        let btn_font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold);
        let save_w = (widgets::text_width(&p, "Save", &btn_font) + theme::INSET_BUTTON * 2.0).max(theme::CONTROL_MD);
        let cancel_w = (widgets::text_width(&p, "Cancel", &btn_font) + theme::INSET_BUTTON * 2.0).max(theme::CONTROL_MD);
        let footer_y = rect.bottom() - theme::INSET_MODAL - theme::CONTROL_MD;
        let save_r = Rect::from_min_size(Pos2::new(rect.right() - theme::INSET_MODAL - save_w, footer_y), Vec2::new(save_w, theme::CONTROL_MD));
        let cancel_r = Rect::from_min_size(Pos2::new(save_r.left() - theme::SPACE_ROW - cancel_w, footer_y), Vec2::new(cancel_w, theme::CONTROL_MD));
        widgets::record(ui, "agent-permissions-cancel", cancel_r);
        widgets::record(ui, "agent-permissions-save", save_r);
        let cancel_resp = kit::button(ui, cancel_r, "agent-permissions-cancel", "Cancel", kit::Hierarchy::Secondary, kit::Size::Md, true);
        let save_resp = kit::button(ui, save_r, "agent-permissions-save", "Save", kit::Hierarchy::Primary, kit::Size::Md, true);
        widgets::labelled(ui, cancel_resp.id, egui::accesskit::Role::Button, "Cancel");
        widgets::labelled(ui, save_resp.id, egui::accesskit::Role::Button, "Save");
        if cancel_resp.clicked() {
            close_modal = true;
        }
        if save_resp.clicked() {
            commit = true;
        }
    });

    if commit {
        *mode = draft;
        if let Some(root) = lib_root {
            let _ = agent_permissions::save_permission_mode(root, draft);
        }
        *open = false;
        ctx.data_mut(|d| d.remove::<AgentPermissionMode>(draft_id));
        return Some(draft);
    }
    if close_modal {
        *open = false;
        ctx.data_mut(|d| d.remove::<AgentPermissionMode>(draft_id));
        return None;
    }
    ctx.data_mut(|d| d.insert_temp(draft_id, draft));
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modal_texts(mode: AgentPermissionMode) -> Vec<String> {
        let ctx = Context::default();
        theme::install_fonts(&ctx);
        theme::apply(&ctx);
        let mut open = true;
        let mut mode = mode;
        // egui sizes an `Area` on its first pass and paints it from the second.
        let mut texts = Vec::new();
        for _ in 0..2 {
            let out =
                ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), ..Default::default() }, |_ui| {
                    let _ = paint_permissions_modal(&ctx, &mut open, &mut mode, None);
                });
            texts = crate::lint::scan_shapes(&out.shapes).0.into_iter().map(|t| t.text).collect();
        }
        texts
    }

    /// Dialog footer actions are 40 high. The options stay radio cards; these two commit or discard the draft.
    #[test]
    fn permissions_modal_footer_is_control_md() {
        let ctx = Context::default();
        theme::install_fonts(&ctx);
        theme::apply(&ctx);
        let mut open = true;
        let mut mode = AgentPermissionMode::Ask;
        for _ in 0..2 {
            let _ = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), ..Default::default() }, |_ui| {
                let _ = paint_permissions_modal(&ctx, &mut open, &mut mode, None);
            });
        }
        let height = |name: &str| -> f32 { ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", name.to_string())))).unwrap_or(Rect::NOTHING).height() };
        assert_eq!(height("agent-permissions-cancel"), theme::CONTROL_MD, "Cancel is a dialog footer button");
        assert_eq!(height("agent-permissions-save"), theme::CONTROL_MD, "Save is a dialog footer button");
    }

    /// Ask is the default (`#841`), so the modal marks Ask and never calls Skip the default, whichever mode is selected.
    #[test]
    fn the_permissions_modal_marks_ask_as_the_default() {
        for selected in [AgentPermissionMode::Ask, AgentPermissionMode::Skip] {
            let texts = modal_texts(selected);
            assert!(texts.iter().any(|t| t == "Ask before acting (default)"), "{selected:?}: Ask carries the default mark: {texts:?}");
            assert!(texts.iter().any(|t| t == "Skip permissions"), "{selected:?}: Skip stays on offer, unmarked: {texts:?}");
            assert!(!texts.iter().any(|t| t.contains("Skip permissions (default)")), "{selected:?}: Skip is no longer the default: {texts:?}");
        }
    }
}
