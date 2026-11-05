//! The composer toolbar's two button painters: an icon button and a text button with a trailing chevron.

use egui::{Align2, Id, Pos2, Rect, Ui};

use crate::icons::{self, Icon};
use crate::theme;
use crate::widgets::{self, kit};

pub(crate) struct IconPaint<'a> {
    pub rect: Rect,
    pub resp: &'a egui::Response,
    pub id: &'a str,
    pub name: &'a str,
    pub icon: Icon,
    pub selected: bool,
    pub enabled: bool,
    pub kind: kit::Hierarchy,
}

pub(crate) struct TextPaint<'a> {
    pub rect: Rect,
    pub resp: &'a egui::Response,
    pub id: &'a str,
    pub label: &'a str,
    pub selected: bool,
    pub chevron: bool,
}

pub(crate) fn paint_icon_button(ui: &Ui, p: &egui::Painter, spec: IconPaint<'_>) -> bool {
    let look = kit::button_look(spec.kind, kit::state_of(spec.resp, spec.selected, spec.enabled));
    kit::paint_frame(p, spec.rect, theme::RADIUS_CONTROL, &look);
    widgets::paint_icon_in(p, spec.rect, spec.icon, theme::LAYER_ICON_SIZE, look.icon);
    if spec.resp.has_focus() {
        kit::focus_ring(p, spec.rect, theme::RADIUS_CONTROL as f32);
    }
    widgets::record(ui, spec.id, spec.rect);
    widgets::labelled(ui, Id::new(spec.id), egui::accesskit::Role::Button, spec.name);
    spec.resp.clicked() && spec.enabled
}

pub(crate) fn paint_text_button(ui: &Ui, p: &egui::Painter, spec: TextPaint<'_>) -> bool {
    let look = kit::button_look(kit::Hierarchy::Tertiary, kit::state_of(spec.resp, spec.selected, true));
    kit::paint_frame(p, spec.rect, theme::RADIUS_CONTROL, &look);
    let font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold);
    let reserve = if spec.chevron { theme::LAYER_ICON_SIZE + theme::SPACE_ROW } else { 0.0 };
    let shown = widgets::elide(p, spec.label, &font, (spec.rect.width() - theme::SPACE_INLINE * 2.0 - reserve).max(theme::SPACE_BLOCK));
    p.text(Pos2::new(spec.rect.left() + theme::SPACE_INLINE, spec.rect.center().y), Align2::LEFT_CENTER, shown, font, look.text);
    if spec.chevron {
        icons::paint(
            p,
            Icon::ChevronDown,
            Pos2::new(spec.rect.right() - theme::SPACE_INLINE - theme::LAYER_ICON_SIZE * 0.5, spec.rect.center().y),
            theme::LAYER_ICON_SIZE,
            look.icon,
        );
    }
    if spec.resp.has_focus() {
        kit::focus_ring(p, spec.rect, theme::RADIUS_CONTROL as f32);
    }
    widgets::record(ui, spec.id, spec.rect);
    spec.resp.clicked()
}
