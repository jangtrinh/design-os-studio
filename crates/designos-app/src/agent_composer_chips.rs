//! The composer's chip row: the selection, the attached PRD, the chosen skills and pasted layers.

use egui::{Align2, Id, Pos2, Rect, Sense, Ui, Vec2};

use crate::agent_composer_chip_layout::{chip_font, chip_rows, place, selection_visible, CHIP_MIN_W};
use crate::agent_selection_chip::composer_chip_label;
use crate::icons::{self, Icon};
use crate::terminal::TerminalState;
use crate::theme;
use crate::widgets::{self, kit};

pub struct ChipOut {
    pub rows: usize,
    pub select_send: bool,
    pub select_hide: bool,
    pub prd_hide: bool,
    pub skills_open: bool,
    pub skills_clear: bool,
    /// Where the skills chip sits, for the Skills popover to leave a press on it alone. `None` without that chip.
    pub skills_rect: Option<Rect>,
    pub drop_layer: Option<designos_doc::NodeId>,
}

/// Paints the chips in `row`. `widths` is what `chip_widths` measured for this same frame.
pub fn paint_chips(ui: &mut Ui, state: &TerminalState, row: Rect, widths: &[f32]) -> ChipOut {
    let p = ui.painter().clone();
    let mut out = ChipOut {
        rows: 0,
        select_send: false,
        select_hide: false,
        prd_hide: false,
        skills_open: false,
        skills_clear: false,
        skills_rect: None,
        drop_layer: None,
    };
    out.rows = chip_rows(widths, row.width());
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
                salt: 0,
                label: &label,
                error: false,
                dismiss: Some("Dismiss selection"),
                tip: Some(TagTip::Text("Send the selection to the agent")),
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
            TagSpec { id: "agents-composer-chip-prd", salt: 0, label: "PRD", error: false, dismiss: Some("Remove PRD"), tip: None, name: Some("PRD attached") },
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
        let bad = state.skills.limit_violation().is_some();
        let hit = paint_tag(
            ui,
            &p,
            rect,
            TagSpec {
                id: "agents-composer-chip-skills",
                salt: 0,
                label: &label,
                error: bad,
                dismiss: Some("Detach skills"),
                tip: Some(TagTip::Names(&state.skills.selected_names)),
                name: None,
            },
        );
        out.skills_rect = Some(rect);
        out.skills_clear = hit.dismiss;
        out.skills_open = hit.body && !hit.dismiss;
    }
    for (i, chip) in state.active_composer().chips.iter().enumerate() {
        let w = widths.get(slot + i).copied().unwrap_or(CHIP_MIN_W);
        let rect = place(&mut x, &mut y, row, w);
        let spec = TagSpec { id: "agents-composer-chip", salt: chip.node_id.0 as usize, label: &chip.name, error: false, dismiss: None, tip: None, name: None };
        if paint_tag(ui, &p, rect, spec).body {
            out.drop_layer = Some(chip.node_id);
        }
    }
    out
}

struct TagHit {
    body: bool,
    dismiss: bool,
}

struct TagSpec<'a> {
    /// The recorded name of the chip; with a non-zero `salt` it makes the widget id, so no frame builds an id string.
    id: &'static str,
    salt: usize,
    label: &'a str,
    error: bool,
    dismiss: Option<&'a str>,
    tip: Option<TagTip<'a>>,
    name: Option<&'a str>,
}

/// A chip tooltip, built only while it is shown.
enum TagTip<'a> {
    Text(&'static str),
    Names(&'a [String]),
}

fn paint_tag(ui: &mut Ui, p: &egui::Painter, rect: Rect, spec: TagSpec<'_>) -> TagHit {
    let resp = ui.interact(rect, if spec.salt == 0 { Id::new(spec.id) } else { Id::new((spec.id, spec.salt)) }, Sense::click());
    // The kit's `Tag/Chip`: control radius, text-xs medium label, a 14 px close glyph. The error chip swaps the colours only.
    let look = kit::tag_look(if resp.hovered() { kit::State::Hover } else { kit::State::Default });
    let (fill, border, text) =
        if spec.error { (theme::BG_ERROR_PRIMARY, theme::BADGE_ERROR_BORDER, theme::BADGE_ERROR_TEXT) } else { (look.fill, look.border, look.text) };
    let radius = theme::cr(theme::RADIUS_CONTROL);
    p.rect_filled(rect, radius, fill);
    p.rect_stroke(rect, radius, egui::Stroke::new(theme::BORDER_WIDTH, border), egui::StrokeKind::Inside);
    let font = chip_font();
    let label_w = if spec.dismiss.is_some() { rect.width() - theme::TARGET_MIN - theme::INSET_FRAME } else { rect.width() - theme::INSET_FRAME * 2.0 };
    let shown = widgets::elide(p, spec.label, &font, label_w.max(theme::SPACE_BLOCK));
    p.text(Pos2::new(rect.left() + theme::INSET_FRAME, rect.center().y), Align2::LEFT_CENTER, shown, font, text);
    widgets::record(ui, spec.id, rect);
    widgets::labelled(ui, resp.id, egui::accesskit::Role::Button, spec.name.unwrap_or(spec.label));
    if resp.has_focus() {
        kit::focus_ring(p, rect, theme::RADIUS_CONTROL as f32);
    }
    if let Some(tip) = spec.tip {
        resp.clone().on_hover_ui(|ui| {
            match tip {
                TagTip::Text(text) => ui.label(text),
                TagTip::Names(names) => ui.label(names.join(", ")),
            };
        });
    }
    let mut dismiss = false;
    if let Some(name) = spec.dismiss {
        let x = Rect::from_min_size(Pos2::new(rect.right() - theme::TARGET_MIN, rect.top()), Vec2::new(theme::TARGET_MIN, rect.height()));
        let xr = ui.interact(x, Id::new((spec.id, spec.salt, "dismiss")), Sense::click());
        let glyph = kit::tag_look(if xr.hovered() { kit::State::Hover } else { kit::State::Default }).icon;
        icons::paint(p, Icon::Close, x.center(), theme::FONT_SIZE_TEXT_SM, glyph);
        widgets::labelled(ui, xr.id, egui::accesskit::Role::Button, name);
        if xr.has_focus() {
            kit::focus_ring(p, x, theme::RADIUS_CONTROL as f32);
        }
        dismiss = xr.clicked();
    }
    TagHit { body: resp.clicked() && !dismiss, dismiss }
}
