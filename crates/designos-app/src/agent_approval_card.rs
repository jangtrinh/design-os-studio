//! In-panel approval card and the `"Agent permissions"` preference modal (`#656`).
//!
//! Every colour, spacing, inset, control height, and radius comes from `crate::theme`
//! (and the semantic `STATUS_WARN_*` tokens in `crate::terminal`).

use crate::agent_permissions::{self, AgentPermissionMode, ApprovalRequest};
use crate::terminal::{AgentCliPreset, STATUS_WARN_BG, STATUS_WARN_BORDER, STATUS_WARN_TEXT};
use crate::theme;
use crate::widgets::{self, menu_panel};
use egui::text::LayoutJob;
use egui::{Align2, Context, Id, Key, Modifiers, Order, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, TextFormat, Ui, Vec2};
use std::path::Path;

/// User action taken on the in-panel approval card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CardAction {
    Approve(String),
    Deny(String),
    OpenTab,
}

/// Height occupied by the approval card (including its top and bottom `SPACE_ROW` viewport margins).
pub fn approval_card_outer_height(preset: AgentCliPreset) -> f32 {
    let is_claude = preset == AgentCliPreset::Claude;
    let summary_and_gap = if is_claude { theme::SPACE_ROW + theme::CONTROL_STANDARD } else { 0.0 };
    let card_h = theme::INSET_FRAME + theme::CONTROL_DENSE + summary_and_gap + theme::SPACE_ROW + theme::CONTROL_STANDARD + theme::INSET_FRAME;
    card_h + theme::SPACE_ROW + theme::SPACE_ROW
}

/// Paints the in-panel approval card pinned to the top of `viewport_rect` (inset by `SPACE_ROW`).
///
/// - For `Claude`, shows the tool name, monospace input summary, queue count (`"1 of N"`), and
///   secondary **Deny** + primary **Approve** buttons.
/// - For `Codex`, `Grok`, and `Antigravity` (`agy`), shows `"<CLI> is waiting for approval in its terminal"`
///   and a single **Open tab** button (never **Approve** or **Deny**).
pub fn paint_approval_card(
    ui: &mut Ui,
    viewport_rect: Rect,
    preset: AgentCliPreset,
    requests: &[ApprovalRequest],
    surface_id: &'static str,
) -> Option<CardAction> {
    let is_claude = preset == AgentCliPreset::Claude;
    let first_req = requests.first();
    if (is_claude || matches!(preset, AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp)) && first_req.is_none() {
        return None;
    }
    let summary_and_gap = if is_claude { theme::SPACE_ROW + theme::CONTROL_STANDARD } else { 0.0 };
    let card_h = theme::INSET_FRAME + theme::CONTROL_DENSE + summary_and_gap + theme::SPACE_ROW + theme::CONTROL_STANDARD + theme::INSET_FRAME;
    let card_rect = Rect::from_min_size(
        Pos2::new(viewport_rect.left() + theme::SPACE_ROW, viewport_rect.top() + theme::SPACE_ROW),
        Vec2::new((viewport_rect.width() - theme::SPACE_ROW - theme::SPACE_ROW).max(theme::CONTROL_ROW), card_h),
    );
    widgets::record(ui, "agent-approval-card", card_rect);

    let p = ui.painter().clone();
    p.rect_filled(card_rect, theme::cr(theme::RADIUS_FRAME), theme::BG_TERTIARY);
    p.rect_stroke(card_rect, theme::cr(theme::RADIUS_FRAME), Stroke::new(theme::BORDER_WIDTH, theme::BORDER), StrokeKind::Inside);

    let inner = card_rect.shrink(theme::INSET_FRAME);
    let header_r = Rect::from_min_size(inner.min, Vec2::new(inner.width(), theme::CONTROL_DENSE));

    // CLI badge pill
    let badge_font = theme::font_caption_medium();
    let badge_label = preset.badge();
    let badge_w = widgets::text_width(&p, badge_label, &badge_font) + theme::INSET_FRAME + theme::INSET_FRAME;
    let badge_r = Rect::from_min_size(header_r.min, Vec2::new(badge_w, theme::CONTROL_DENSE));
    p.rect_filled(badge_r, theme::cr_pill(theme::CONTROL_DENSE), STATUS_WARN_BG);
    p.rect_stroke(badge_r, theme::cr_pill(theme::CONTROL_DENSE), Stroke::new(theme::BORDER_WIDTH, STATUS_WARN_BORDER), StrokeKind::Inside);
    p.text(badge_r.center(), Align2::CENTER_CENTER, badge_label, badge_font, STATUS_WARN_TEXT);

    // Optional right-aligned "1 of N" queue indicator
    let count_font = theme::font_caption();
    let count_reserve = if is_claude && requests.len() > 1 {
        let count_text = format!("1 of {}", requests.len());
        let cw = widgets::text_width(&p, &count_text, &count_font) + theme::SPACE_INLINE;
        p.text(Pos2::new(header_r.right(), header_r.center().y), Align2::RIGHT_CENTER, count_text, count_font, theme::TEXT_SECONDARY);
        cw
    } else {
        0.0
    };

    // Title
    let title_font = theme::font_body_medium();
    let raw_title = if is_claude {
        let tool = first_req.map(|r| r.tool_name.as_str()).unwrap_or("tool");
        format!("{} wants to run {tool}", preset.tab_title())
    } else {
        format!("{} is waiting for approval in its terminal", preset.tab_title())
    };
    let title_x = badge_r.right() + theme::SPACE_INLINE;
    let title_max_w = (header_r.right() - count_reserve - title_x).max(theme::CONTROL_DENSE);
    let title_elided = widgets::elide(&p, &raw_title, &title_font, title_max_w);
    p.text(Pos2::new(title_x, header_r.center().y), Align2::LEFT_CENTER, title_elided, title_font, theme::TEXT);

    let mut y = header_r.bottom() + theme::SPACE_ROW;

    // Summary block (Claude only)
    if let Some(req) = first_req.filter(|_| is_claude) {
        let sum_r = Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), theme::CONTROL_STANDARD));
        p.rect_filled(sum_r, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
        p.rect_stroke(sum_r, theme::cr(theme::RADIUS_CONTROL), Stroke::new(theme::BORDER_WIDTH, theme::BORDER), StrokeKind::Inside);
        let mono_font = theme::font_mono();
        let sum_max_w = (sum_r.width() - theme::INSET_BUTTON - theme::INSET_BUTTON).max(theme::CONTROL_DENSE);
        let sum_elided = widgets::elide(&p, &req.input_summary, &mono_font, sum_max_w);
        p.text(Pos2::new(sum_r.left() + theme::INSET_BUTTON, sum_r.center().y), Align2::LEFT_CENTER, sum_elided, mono_font, theme::TEXT_SECONDARY);
        widgets::record(ui, "agent-approval-summary", sum_r);
        y = sum_r.bottom() + theme::SPACE_ROW;
    }

    // Action row (right-aligned)
    let action_r = Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), theme::CONTROL_STANDARD));
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
    } else {
        let (_open_r, open_resp) =
            paint_card_button(ui, &p, (surface_id, "agent-approval-open-tab"), "agent-approval-open-tab", action_r.right(), action_r.top(), "Open tab", false);
        if open_resp.clicked() {
            out = Some(CardAction::OpenTab);
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
    let font = theme::font_body_medium();
    let w = (widgets::text_width(p, label, &font) + theme::INSET_BUTTON + theme::INSET_BUTTON).max(theme::CONTROL_ROW);
    let rect = Rect::from_min_size(Pos2::new(right_x - w, top_y), Vec2::new(w, theme::CONTROL_STANDARD));
    let resp = ui.interact(rect, Id::new(id), Sense::click());
    let fill = match (primary, resp.hovered()) {
        (true, true) => theme::BG_BRAND_HOVER,
        (true, false) => theme::BG_BRAND,
        (false, true) => theme::BG_HOVER,
        (false, false) => theme::BG_SECONDARY,
    };
    p.rect_filled(rect, theme::cr(theme::RADIUS_CONTROL), fill);
    if !primary {
        p.rect_stroke(rect, theme::cr(theme::RADIUS_CONTROL), Stroke::new(theme::BORDER_WIDTH, theme::BORDER), StrokeKind::Inside);
    }
    p.text(rect.center(), Align2::CENTER_CENTER, label, font, theme::TEXT);
    widgets::record(ui, record_name, rect);
    (rect, resp)
}

/// Renders the `"Agent permissions"` modal opened from `Preferences -> Agent permissions…`
/// or the command palette.
pub fn paint_permissions_modal(ctx: &Context, open: &mut bool, mode: &mut AgentPermissionMode, lib_root: Option<&Path>) -> Option<AgentPermissionMode> {
    if !*open {
        return None;
    }
    let screen = ctx.content_rect();
    let esc = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
    let row_h = theme::CONTROL_ROW + theme::CONTROL_STANDARD;
    let modal_h = theme::INSET_MODAL
        + theme::LINE_BODY
        + theme::SPACE_GROUP
        + row_h
        + theme::SPACE_ROW
        + row_h
        + theme::SPACE_GROUP
        + theme::LINE_CAPTION
        + theme::INSET_MODAL;
    let rect = Rect::from_center_size(screen.center(), Vec2::new(theme::PERMISSIONS_MODAL_WIDTH, modal_h));

    let mut close_modal = esc;
    let mut picked_mode: Option<AgentPermissionMode> = None;

    egui::Area::new(Id::new("agent-permissions-modal")).order(Order::Foreground).fixed_pos(screen.min).interactable(true).show(ctx, |ui| {
        let scrim = ui.allocate_rect(screen, Sense::click_and_drag());
        let p = ui.painter().clone();
        p.rect_filled(screen, 0.0, theme::SHADOW_COLOR_STRONG);
        if scrim.clicked() && !rect.contains(scrim.interact_pointer_pos().unwrap_or(Pos2::ZERO)) {
            close_modal = true;
        }

        menu_panel(&p, rect, theme::RADIUS_FRAME);
        p.rect_filled(rect, theme::cr(theme::RADIUS_FRAME), theme::BG_SECONDARY);
        p.rect_stroke(rect, theme::cr(theme::RADIUS_FRAME), Stroke::new(theme::BORDER_WIDTH, theme::BORDER), StrokeKind::Inside);
        widgets::record(ui, "agent-permissions-modal", rect);

        let inner_x = rect.left() + theme::INSET_MODAL;
        let inner_w = rect.width() - theme::INSET_MODAL - theme::INSET_MODAL;
        let mut y = rect.top() + theme::INSET_MODAL;

        p.text(Pos2::new(inner_x, y), Align2::LEFT_TOP, "Agent permissions", theme::font_body_medium(), theme::TEXT);
        y += theme::LINE_BODY + theme::SPACE_GROUP;

        let options = [
            (
                AgentPermissionMode::Skip,
                "agent-perm-skip",
                "Skip permissions (default)",
                "Launch CLIs with their auto-approve flag. Fastest inside a trusted project.",
            ),
            (
                AgentPermissionMode::Ask,
                "agent-perm-ask",
                "Ask before agents act",
                "Launch CLIs without skip flags and surface tool approvals in the Agents panel.",
            ),
        ];

        for (opt_mode, widget_id, title, caption) in options {
            let row_r = Rect::from_min_size(Pos2::new(inner_x, y), Vec2::new(inner_w, row_h));
            let resp = ui.interact(row_r, Id::new(widget_id), Sense::click());
            let selected = *mode == opt_mode;
            let fill = if selected {
                theme::BG_SELECTED
            } else if resp.hovered() {
                theme::BG_HOVER
            } else {
                theme::BG_TERTIARY
            };
            p.rect_filled(row_r, theme::cr(theme::RADIUS_CONTROL), fill);
            p.rect_stroke(
                row_r,
                theme::cr(theme::RADIUS_CONTROL),
                Stroke::new(theme::BORDER_WIDTH, if selected { theme::BORDER_SELECTED } else { theme::BORDER }),
                StrokeKind::Inside,
            );

            let text_x = row_r.left() + theme::INSET_FRAME;
            let text_w = (row_r.width() - theme::INSET_FRAME - theme::INSET_FRAME).max(theme::CONTROL_ROW);
            let title_y = row_r.top() + theme::INSET_FRAME;
            p.text(Pos2::new(text_x, title_y), Align2::LEFT_TOP, title, theme::font_body_medium(), theme::TEXT);

            let mut job = LayoutJob::simple(caption.to_string(), theme::font_caption(), theme::TEXT_SECONDARY, text_w);
            if let Some(sec) = job.sections.first_mut() {
                sec.format =
                    TextFormat { font_id: theme::font_caption(), color: theme::TEXT_SECONDARY, line_height: Some(theme::LINE_CAPTION), ..Default::default() };
            }
            let galley = ui.fonts_mut(|f| f.layout_job(job));
            p.galley(Pos2::new(text_x, title_y + theme::LINE_BODY + theme::SPACE_FACT), galley, theme::TEXT_SECONDARY);

            widgets::record(ui, widget_id, row_r);
            if resp.clicked() {
                picked_mode = Some(opt_mode);
            }
            y += row_h + theme::SPACE_ROW;
        }

        y = y - theme::SPACE_ROW + theme::SPACE_GROUP;
        p.text(Pos2::new(inner_x, y), Align2::LEFT_TOP, "Applies to new agent tabs.", theme::font_caption(), theme::TEXT_SECONDARY);
    });

    if let Some(new_mode) = picked_mode {
        *mode = new_mode;
        if let Some(root) = lib_root {
            let _ = agent_permissions::save_permission_mode(root, new_mode);
        }
        return Some(new_mode);
    }
    if close_modal {
        *open = false;
    }
    None
}
