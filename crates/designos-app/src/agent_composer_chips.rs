//! The composer's chip row: the selection, the attached PRD, the chosen skills and pasted layers.

use egui::{Align2, Id, Pos2, Rect, Sense, Ui, Vec2};

use crate::agent_selection_chip::{composer_chip_label, selection_fingerprint};
use crate::icons::{self, Icon};
use crate::terminal::TerminalState;
use crate::theme;
use crate::widgets::{self, kit};

pub const CHIP_MAX_W: f32 = 148.0;
const CHIP_MIN_W: f32 = 72.0;

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
                name: Some("Send selection to agent"),
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
        if paint_tag(
            ui,
            &p,
            rect,
            TagSpec { id: "agents-composer-chip-prd", label: "PRD", error: false, dismiss: Some("Remove PRD"), tip: None, name: Some("PRD attached") },
        )
        .dismiss
        {
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
        let hit = paint_tag(
            ui,
            &p,
            rect,
            TagSpec { id: "agents-composer-chip-skills", label: &label, error: bad, dismiss: Some("Detach skills"), tip: Some(&tip), name: None },
        );
        out.skills_clear = hit.dismiss;
        out.skills_open = hit.body && !hit.dismiss;
    }
    for (i, chip) in state.active_composer().chips.iter().enumerate() {
        let w = widths.get(slot + i).copied().unwrap_or(CHIP_MIN_W);
        let rect = place(&mut x, &mut y, row, w);
        let id = format!("agents-composer-chip-{}", chip.node_id.0);
        if paint_tag(ui, &p, rect, TagSpec { id: &id, label: &chip.name, error: false, dismiss: None, tip: None, name: None }).body {
            out.drop_layer = Some(chip.node_id);
        }
        widgets::record(ui, "agents-composer-chip", rect);
    }
    out
}

pub(crate) fn selection_visible(state: &TerminalState) -> bool {
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
    name: Option<&'a str>,
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
    widgets::labelled(ui, resp.id, egui::accesskit::Role::Button, spec.name.unwrap_or(spec.label));
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
