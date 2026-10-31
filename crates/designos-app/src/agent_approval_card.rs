//! In-panel approval card and the `"Agent permissions"` preference modal (`#656`).
//!
//! Every colour, spacing, inset, control height, and radius comes from `crate::theme`.

use crate::agent_permissions::{self, AgentPermissionMode, ApprovalRequest};
use crate::terminal::AgentCliPreset;
use crate::theme;
use crate::widgets::{self, kit};
use egui::text::LayoutJob;
use egui::{Align2, Context, Id, Key, Modifiers, Order, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, TextFormat, Ui, Vec2};
use std::path::Path;

/// User action taken on the in-panel approval card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CardAction {
    Approve(String),
    /// Allow this modify call, and remember the same tool and target for this window.
    AllowSession(String),
    Deny(String),
    ShowPrompt,
    OpenTab(usize),
}

/// A Revoke click on one remembered grant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrantRevoke {
    pub tool_name: String,
    pub target: String,
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
    mode: AgentPermissionMode,
) -> Option<CardAction> {
    paint_approval_card_for_tab(ui, viewport_rect, preset, requests, 0, preset == AgentCliPreset::Claude, requests.len(), surface_id, mode)
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
    mode: AgentPermissionMode,
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
        let mut next_right = approve_r.left() - theme::SPACE_ROW;
        if agent_permissions::offer_session_grant(mode, req) {
            let (session_r, session_resp) = paint_card_button(
                ui,
                &p,
                (surface_id, "agent-approval-allow-session"),
                "agent-approval-allow-session",
                next_right,
                action_r.top(),
                "Allow this session",
                false,
            );
            next_right = session_r.left() - theme::SPACE_ROW;
            if session_resp.clicked() {
                out = Some(CardAction::AllowSession(req_id.clone()));
            }
        }
        let (_deny_r, deny_resp) =
            paint_card_button(ui, &p, (surface_id, "agent-approval-deny"), "agent-approval-deny", next_right, action_r.top(), "Deny", false);
        if approve_resp.clicked() {
            out = Some(CardAction::Approve(req_id));
        } else if out.is_none() && deny_resp.clicked() {
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

/// The auto-save row of the permissions modal: a text-sm title, two text-xs note lines and the switch.
const AUTOSAVE_ROW_H: f32 = theme::INSET_PANEL * 2.0 + theme::LINE_TEXT_SM + theme::SPACE_FACT + theme::LINE_TEXT_XS * 2.0;

/// Height of one remembered-grant row: a `Button/Secondary` xs with `SPACE_ROW` above and below.
const GRANT_ROW_H: f32 = theme::CONTROL_XS + theme::SPACE_ROW * 2.0;

/// Lists remembered grants for this window. Returns the strip height and a Revoke click, if any.
pub fn paint_grant_strip(ui: &mut Ui, origin: Pos2, width: f32, grants: &[agent_permissions::SessionGrant]) -> (f32, Option<GrantRevoke>) {
    if grants.is_empty() || width <= 0.0 {
        return (0.0, None);
    }
    let height = grants.len() as f32 * GRANT_ROW_H;
    let p = ui.painter().clone();
    let strip = Rect::from_min_size(origin, Vec2::new(width, height));
    p.rect_filled(strip, 0.0, theme::BG_PRIMARY);
    widgets::hline(&p, strip.left(), strip.right(), strip.bottom() - theme::BORDER_WIDTH, theme::BORDER);
    widgets::record(ui, "agent-grant-strip", strip);
    let font = theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Regular);
    let mut revoke = None;
    for (idx, grant) in grants.iter().enumerate() {
        let row = Rect::from_min_size(Pos2::new(origin.x, origin.y + idx as f32 * GRANT_ROW_H), Vec2::new(width, GRANT_ROW_H));
        let (btn, resp) = paint_card_button(
            ui,
            &p,
            ("agent-grant-revoke", idx, grant.tool_name.as_str(), grant.target.as_str()),
            "agent-grant-revoke",
            row.right() - theme::INSET_CONTROL,
            row.top() + theme::SPACE_ROW,
            "Revoke",
            false,
        );
        let text_w = (btn.left() - row.left() - theme::INSET_CONTROL - theme::SPACE_INLINE).max(theme::TARGET_MIN);
        let label = format!("{} {}", grant.tool_name, grant.target);
        let elided = widgets::elide(&p, &label, &font, text_w);
        p.text(Pos2::new(row.left() + theme::INSET_CONTROL, row.center().y), Align2::LEFT_CENTER, elided, font.clone(), theme::TEXT_SECONDARY);
        if resp.clicked() && revoke.is_none() {
            revoke = Some(GrantRevoke { tool_name: grant.tool_name.clone(), target: grant.target.clone() });
        }
    }
    (height, revoke)
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
    let autosave_h = AUTOSAVE_ROW_H;
    let modal_h = theme::INSET_MODAL
        + theme::LINE_TEXT_MD
        + theme::SPACE_GROUP
        + row_h
        + theme::SPACE_ROW
        + row_h
        + theme::SPACE_ROW
        + row_h
        + theme::SPACE_GROUP
        + autosave_h
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
            (
                AgentPermissionMode::AskRisky,
                "agent-perm-ask-risky",
                "Ask for risky only",
                "Reads and additions run. A change on one target can be allowed for this session. Deletes always ask.",
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

        y += theme::LINE_TEXT_SM + theme::SPACE_GROUP;
        let autosave_on = lib_root.map(crate::comments::settings::load_autosave_ai_changes).unwrap_or(true);
        let auto_r = Rect::from_min_size(Pos2::new(inner_x, y), Vec2::new(inner_w, autosave_h));
        let auto_resp = ui.interact(auto_r, Id::new("agent-autosave-ai"), Sense::click());
        let auto_fill = if auto_resp.hovered() { theme::BG_HOVER } else { theme::BG_TERTIARY };
        p.rect_filled(auto_r, theme::cr(theme::RADIUS_CONTROL), auto_fill);
        p.rect_stroke(auto_r, theme::cr(theme::RADIUS_CONTROL), Stroke::new(theme::BORDER_WIDTH, theme::BORDER), StrokeKind::Inside);
        let text_x = auto_r.left() + theme::INSET_PANEL;
        let switch_rect = Rect::from_min_size(
            Pos2::new(auto_r.right() - theme::INSET_PANEL - theme::BADGE_LG, auto_r.center().y - theme::LAYER_ICON_SIZE * 0.5),
            Vec2::new(theme::BADGE_LG, theme::LAYER_ICON_SIZE),
        );
        let caption_w = switch_rect.left() - theme::SPACE_INLINE - text_x;
        let title_y = auto_r.top() + theme::INSET_PANEL;
        let auto_title_font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Medium);
        p.text(Pos2::new(text_x, title_y), Align2::LEFT_TOP, "Auto-save changes made by AI", auto_title_font, theme::TEXT);
        let caption = if autosave_on {
            "On. Save after each agent tool batch, outside the library. This session too."
        } else {
            "Off. Edits stay unsaved until the agent or you save. This session too."
        };
        let auto_caption_font = theme::font_text(theme::FONT_SIZE_TEXT_XS, theme::TextWeight::Regular);
        let mut job = LayoutJob::simple(caption.to_string(), auto_caption_font.clone(), theme::TEXT_SECONDARY, caption_w);
        job.wrap.max_width = caption_w;
        job.wrap.max_rows = 2;
        job.wrap.break_anywhere = false;
        job.wrap.overflow_character = Some('…');
        if let Some(sec) = job.sections.first_mut() {
            sec.format = TextFormat { font_id: auto_caption_font, color: theme::TEXT_SECONDARY, line_height: Some(theme::LINE_TEXT_XS), ..Default::default() };
        }
        let galley = ui.fonts_mut(|f| f.layout_job(job));
        p.galley(Pos2::new(text_x, title_y + theme::LINE_TEXT_SM + theme::SPACE_FACT), galley, theme::TEXT_SECONDARY);
        let sw = switch_rect;
        let fill = if autosave_on { theme::BG_BRAND } else { theme::BG_PRESSED };
        p.rect_filled(sw, theme::cr_pill(sw.height()), fill);
        if !autosave_on {
            p.rect_stroke(sw, theme::cr_pill(sw.height()), Stroke::new(theme::BORDER_WIDTH, theme::BORDER_STRONG), StrokeKind::Inside);
        }
        let knob_r = theme::LAYER_ICON_SIZE * 0.5 - theme::INSET_SEGMENT;
        let kx = if autosave_on { sw.right() - theme::INSET_SEGMENT - knob_r } else { sw.left() + theme::INSET_SEGMENT + knob_r };
        p.circle_filled(Pos2::new(kx, sw.center().y), knob_r, theme::ICON_WHITE);
        widgets::record(ui, "agent-autosave-switch", switch_rect);
        widgets::record(ui, "agent-autosave-ai", auto_r);
        if auto_resp.clicked() {
            if let Some(root) = lib_root {
                let _ = crate::comments::settings::save_autosave_ai_changes(root, !autosave_on);
            }
        }

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
    use egui::epaint::{RectShape, Shape};

    fn painted_rects(shapes: &[egui::epaint::ClippedShape]) -> Vec<(Rect, egui::Color32, Stroke)> {
        fn walk(shape: &Shape, out: &mut Vec<(Rect, egui::Color32, Stroke)>) {
            match shape {
                Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, out)),
                Shape::Rect(RectShape { rect, fill, stroke, .. }) => out.push((*rect, *fill, *stroke)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        for clipped in shapes {
            walk(&clipped.shape, &mut out);
        }
        out
    }

    fn autosave_frame(on: bool) -> (Vec<crate::lint::TextRun>, Vec<(Rect, egui::Color32, Stroke)>, std::path::PathBuf) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!("designos-autosave-card-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir_all(&root).expect("could not create autosave test directory");
        crate::comments::settings::save_autosave_ai_changes(&root, on).expect("could not save autosave test setting");
        let ctx = Context::default();
        theme::install_fonts(&ctx);
        theme::apply(&ctx);
        ctx.all_styles_mut(|s| s.animation_time = 0.0);
        let mut open = true;
        let mut mode = AgentPermissionMode::Ask;
        let mut output = None;
        for frame in 0..4 {
            output = Some(ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))),
                    time: Some(frame as f64 * 0.1),
                    ..Default::default()
                },
                |_ui| {
                    let _ = paint_permissions_modal(&ctx, &mut open, &mut mode, Some(&root));
                },
            ));
        }
        let output = output.expect("autosave modal did not produce a frame");
        (crate::lint::scan_shapes(&output.shapes).0, painted_rects(&output.shapes), root)
    }

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
        for selected in [AgentPermissionMode::Ask, AgentPermissionMode::Skip, AgentPermissionMode::AskRisky] {
            let texts = modal_texts(selected);
            assert!(texts.iter().any(|t| t == "Ask before acting (default)"), "{selected:?}: Ask carries the default mark: {texts:?}");
            assert!(texts.iter().any(|t| t == "Skip permissions"), "{selected:?}: Skip stays on offer, unmarked: {texts:?}");
            assert!(texts.iter().any(|t| t == "Ask for risky only"), "{selected:?}: the third mode is on the selector: {texts:?}");
            assert!(!texts.iter().any(|t| t.contains("Skip permissions (default)")), "{selected:?}: Skip is no longer the default: {texts:?}");
            assert!(texts.iter().any(|t| t == "Applies to new agent tabs."), "{selected:?}: permission modes keep their footnote: {texts:?}");
            assert!(texts.iter().any(|t| t.contains("This session too.")), "{selected:?}: autosave says it applies to this session: {texts:?}");
        }
    }

    fn card_texts(preset: AgentCliPreset, requests: &[ApprovalRequest], mode: AgentPermissionMode) -> Vec<String> {
        let ctx = Context::default();
        theme::install_fonts(&ctx);
        theme::apply(&ctx);
        let mut texts = Vec::new();
        for _ in 0..2 {
            let out = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), ..Default::default() }, |ui| {
                let panel = Rect::from_min_size(Pos2::new(48.0, 0.0), Vec2::new(420.0, 800.0));
                let _ = paint_approval_card(ui, panel, preset, requests, "test-card", mode);
            });
            texts = crate::lint::scan_shapes(&out.shapes).0.into_iter().map(|t| t.text).collect();
        }
        texts
    }

    fn request(tool: &str, target: &str) -> ApprovalRequest {
        ApprovalRequest { id: "req-1".into(), tool_name: tool.into(), input_summary: "node 2".into(), timestamp: 1, target: target.into() }
    }

    #[test]
    fn the_session_button_is_only_offered_for_a_modify_with_a_target() {
        let modify = card_texts(AgentCliPreset::Claude, &[request("designos_update_node", "node_id:2")], AgentPermissionMode::AskRisky);
        assert!(modify.iter().any(|t| t == "Allow this session"), "{modify:?}");
        assert!(modify.iter().any(|t| t == "Approve"), "{modify:?}");
        let delete = card_texts(AgentCliPreset::Claude, &[request("designos_delete_node", "node_id:3")], AgentPermissionMode::AskRisky);
        assert!(!delete.iter().any(|t| t == "Allow this session"), "{delete:?}");
        assert!(delete.iter().any(|t| t == "Deny"), "{delete:?}");
        let no_target = card_texts(AgentCliPreset::Claude, &[request("designos_update_node", "")], AgentPermissionMode::Ask);
        assert!(!no_target.iter().any(|t| t == "Allow this session"), "{no_target:?}");
    }

    #[test]
    fn the_grant_strip_lists_a_grant_and_a_revoke_control() {
        let ctx = Context::default();
        theme::install_fonts(&ctx);
        theme::apply(&ctx);
        let grant = agent_permissions::SessionGrant {
            session_id: "s".into(),
            tool_name: "designos_update_node".into(),
            target: "node_id:2".into(),
            last_used_secs: 10,
        };
        let mut texts = Vec::new();
        for _ in 0..2 {
            let out = ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))), ..Default::default() }, |ui| {
                let _ = paint_grant_strip(ui, Pos2::ZERO, 360.0, std::slice::from_ref(&grant));
            });
            texts = crate::lint::scan_shapes(&out.shapes).0.into_iter().map(|t| t.text).collect();
        }
        assert!(texts.iter().any(|t| t == "Revoke"), "{texts:?}");
        assert!(texts.iter().any(|t| t.contains("designos_update_node")), "{texts:?}");
    }

    #[test]
    fn autosave_row_caption_wraps_inside_its_column_and_clear_of_the_switch() {
        for on in [true, false] {
            let (texts, rects, root) = autosave_frame(on);
            let prefix = if on { "On." } else { "Off." };
            let caption = texts.iter().find(|text| text.text.starts_with(prefix)).expect("autosave caption was not painted");
            let title = texts.iter().find(|text| text.text == "Auto-save changes made by AI").expect("autosave title was not painted");
            let switch = rects
                .iter()
                .find(|(r, _, _)| (r.width() - theme::BADGE_LG).abs() < 0.5 && (r.height() - theme::LAYER_ICON_SIZE).abs() < 0.5)
                .expect("autosave switch was not painted")
                .0;
            let row = rects
                .iter()
                .find(|(r, _, _)| {
                    (r.height() - AUTOSAVE_ROW_H).abs() < 0.5
                        && (r.width() - (theme::PERMISSIONS_MODAL_WIDTH - theme::INSET_MODAL - theme::INSET_MODAL)).abs() < 0.5
                        && r.contains(caption.rect.center())
                })
                .expect("autosave row containing the caption was not painted")
                .0;
            assert!(caption.rect.right() <= switch.left() - theme::SPACE_INLINE + 0.5, "autosave caption overlaps the switch");
            assert!(row.contains(caption.rect.min) && row.contains(caption.rect.max), "autosave caption extends outside its row");
            assert!(caption.text.lines().count() <= 2, "autosave caption wraps to more than two lines");
            assert!(texts.iter().all(|text| !text.rect.intersects(switch)), "autosave text overlaps the switch");
            assert!(row.contains(switch.min) && row.contains(switch.max), "autosave switch extends outside its row");
            assert!(row.contains(title.rect.min) && row.contains(title.rect.max), "autosave title extends outside its row");
            assert!(caption.text.contains("This session too."), "autosave caption omits its session note");
            std::fs::remove_dir_all(root).expect("could not remove autosave test directory");
        }
    }

    #[test]
    fn autosave_row_off_track_is_visible_against_the_row_and_the_modal() {
        let (_, off_rects, off_root) = autosave_frame(false);
        let switch_off = off_rects
            .iter()
            .find(|(r, _, _)| (r.width() - theme::BADGE_LG).abs() < 0.5 && (r.height() - theme::LAYER_ICON_SIZE).abs() < 0.5)
            .expect("off autosave switch was not painted");
        assert_eq!(switch_off.1, theme::BG_PRESSED, "off autosave switch does not use the exact opaque pressed fill");
        assert_ne!(switch_off.1, theme::BG_TERTIARY, "off autosave switch matches the tertiary background");
        assert_ne!(switch_off.1, theme::BG_SECONDARY, "off autosave switch matches the secondary background");
        let luma = |c: egui::Color32| (u16::from(c.r()) * 299 + u16::from(c.g()) * 587 + u16::from(c.b()) * 114) / 1000;
        assert!(luma(switch_off.1).abs_diff(luma(theme::BG_TERTIARY)) >= 0x10, "off autosave switch lacks contrast against the row");
        assert!(
            off_rects.iter().any(|(r, _, stroke)| {
                stroke.color == theme::BORDER_STRONG
                    && stroke.width > 0.0
                    && switch_off.0.expand(theme::BORDER_WIDTH).contains_rect(*r)
                    && r.contains_rect(switch_off.0.shrink(theme::BORDER_WIDTH))
            }),
            "off autosave switch has no visible strong border"
        );
        std::fs::remove_dir_all(off_root).expect("could not remove off autosave test directory");

        let (_, on_rects, on_root) = autosave_frame(true);
        let switch_on = on_rects
            .iter()
            .find(|(r, _, _)| (r.width() - theme::BADGE_LG).abs() < 0.5 && (r.height() - theme::LAYER_ICON_SIZE).abs() < 0.5)
            .expect("on autosave switch was not painted");
        assert_eq!(switch_on.1, theme::BG_BRAND, "on autosave switch does not use the brand fill");
        std::fs::remove_dir_all(on_root).expect("could not remove on autosave test directory");
    }
}
