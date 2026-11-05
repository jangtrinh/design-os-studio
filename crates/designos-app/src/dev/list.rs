//! The Dev mode left panel: "Ready for dev" items per page with status pills.

use super::sections::{status_color, status_pill, status_pill_width};
use super::LeftTab;
use crate::app::App;
use crate::panels::{clipped, scrollbar, wheel_scroll};
use crate::theme;
use crate::widgets::{self, elide};
use designos_doc::*;
use egui::{Align2, Id, Pos2, Rect, Sense, Ui, Vec2};
use std::borrow::Cow;

/// One row of the list.
#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    Page { id: NodeId, name: String, count: usize },
    Item { id: NodeId, page: NodeId, name: String, status: DevStatus },
}

/// Pages that have tracked layers, each followed by its layers (document order).
pub fn rows(doc: &Document) -> Vec<Row> {
    let mut out = Vec::new();
    for &page in &doc.pages {
        let items = items_on_page(doc, page);
        if items.is_empty() {
            continue;
        }
        out.push(Row::Page { id: page, name: doc.get(page).map(|n| n.name.clone()).unwrap_or_default(), count: items.len() });
        for id in items {
            if let Some(n) = doc.get(id) {
                if let Some(status) = n.dev.status {
                    out.push(Row::Item { id, page, name: n.name.clone(), status });
                }
            }
        }
    }
    out
}

const TAB_BADGE_TEXT_PAD: f32 = 10.0;
const TAB_BADGE_MIN: f32 = 18.0;
const TAB_BADGE_GAP: f32 = 6.0;
const TAB_BADGE_H: f32 = 16.0;

fn tab_width(p: &egui::Painter, label: &str, font: &egui::FontId, badge_w: f32) -> (f32, f32) {
    let label_w = widgets::text_width(p, label, font).ceil();
    let extra = if badge_w > 0.0 { TAB_BADGE_GAP + badge_w } else { 0.0 };
    (label_w, label_w + extra + 2.0 * theme::INSET_CONTROL)
}

/// Tab switcher replacing the "Layers" title in Dev mode.
/// The strip stops short of the layers action cluster: the count badge is dropped first, then the ready label is elided.
pub fn tabs(app: &mut App, ui: &mut Ui, row: Rect) {
    let font = theme::font_body_strong();
    let mono_font = theme::font_mono();
    let gap = theme::SPACE_FACT;
    let origin = row.left() + theme::PANEL_PADDING_X;
    let right_limit = crate::panels::left::action_cluster(row.right(), row.top()).1.left() - theme::SPACE_FACT;
    let budget = (right_limit - origin).max(0.0);
    let total: usize = rows(&app.doc).iter().filter(|r| matches!(r, Row::Item { .. })).count();
    let tabs_def = [(LeftTab::ReadyForDev, "Ready for dev"), (LeftTab::Layers, "Layers")];
    let layers_label = tabs_def[1].1;
    let (layers_label_w, layers_w) = tab_width(ui.painter(), layers_label, &font, 0.0);
    let mut ready_label = Cow::Borrowed(tabs_def[0].1);
    let mut show_badge = total > 0;
    let badge_str = total.to_string();
    let full_badge_w = (widgets::text_width(ui.painter(), &badge_str, &mono_font) + TAB_BADGE_TEXT_PAD).max(TAB_BADGE_MIN);
    let mut badge_w = if show_badge { full_badge_w } else { 0.0 };
    let (mut ready_label_w, mut ready_w) = tab_width(ui.painter(), &ready_label, &font, badge_w);
    if ready_w + gap + layers_w > budget && show_badge {
        show_badge = false;
        badge_w = 0.0;
        (ready_label_w, ready_w) = tab_width(ui.painter(), &ready_label, &font, 0.0);
    }
    if ready_w + gap + layers_w > budget {
        let max_label = (budget - gap - layers_w - 2.0 * theme::INSET_CONTROL).max(0.0);
        ready_label = Cow::Owned(elide(ui.painter(), &ready_label, &font, max_label));
        show_badge = false;
        badge_w = 0.0;
        (ready_label_w, ready_w) = tab_width(ui.painter(), &ready_label, &font, 0.0);
    }
    let mut x = origin;
    for (i, (tab, _)) in tabs_def.into_iter().enumerate() {
        let (label, label_w, w, badge_here) = if tab == LeftTab::ReadyForDev {
            (ready_label.as_ref(), ready_label_w, ready_w, show_badge)
        } else {
            (layers_label, layers_label_w, layers_w, false)
        };
        let r = Rect::from_min_size(Pos2::new(x, row.center().y - theme::TARGET_MIN * 0.5), Vec2::new(w, theme::TARGET_MIN));
        widgets::record(ui, if tab == LeftTab::ReadyForDev { "dev-left-tab-ready" } else { "dev-left-tab-layers" }, r);
        let resp = ui.interact(r, ui.id().with(("dev-left-tab", i)), Sense::click());
        let active = app.dev.left_tab == tab;
        if active {
            ui.painter().rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
        } else if resp.hovered() {
            ui.painter().rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
        }
        ui.painter().text(
            Pos2::new(r.left() + theme::INSET_CONTROL, r.center().y),
            Align2::LEFT_CENTER,
            label,
            font.clone(),
            if active { theme::TEXT } else { theme::TEXT_SECONDARY },
        );
        if badge_here {
            let badge_r = Rect::from_min_size(
                Pos2::new(r.left() + theme::INSET_CONTROL + label_w + TAB_BADGE_GAP, r.center().y - TAB_BADGE_H * 0.5),
                Vec2::new(badge_w, TAB_BADGE_H),
            );
            ui.painter().rect_filled(badge_r, egui::CornerRadius::same(7), egui::Color32::from_rgba_unmultiplied(0x14, 0xAE, 0x5C, 38));
            ui.painter().text(badge_r.center(), Align2::CENTER_CENTER, &badge_str, mono_font.clone(), egui::Color32::from_rgb(0x34, 0xD3, 0x99));
        }
        if resp.clicked() {
            app.dev.left_tab = tab;
        }
        x += w + gap;
    }
}

const ROW_H: f32 = 32.0;

/// The list below the tab row.
pub fn show(app: &mut App, ui: &mut Ui, region: Rect) {
    let list = rows(&app.doc);
    if list.is_empty() {
        let p = ui.painter();
        p.text(
            Pos2::new(region.center().x, region.top() + 48.0),
            Align2::CENTER_CENTER,
            "Nothing is ready for dev yet",
            theme::font_body_strong(),
            theme::TEXT_SECONDARY,
        );
        p.text(
            Pos2::new(region.center().x, region.top() + 70.0),
            Align2::CENTER_CENTER,
            "Select a frame or section and mark it Ready for dev",
            theme::font_caption(),
            theme::TEXT_TERTIARY,
        );
        return;
    }
    let content_h = list.len() as f32 * ROW_H + 8.0;
    let state = Id::new("dev-ready-scroll");
    let mut scroll = ui.ctx().data(|d| d.get_temp::<f32>(state)).unwrap_or(0.0);
    wheel_scroll(ui, region, &mut scroll, content_h);
    let mut go: Option<(NodeId, NodeId)> = None;
    clipped(ui, region, |ui| {
        for (i, r) in list.iter().enumerate() {
            let row = Rect::from_min_size(Pos2::new(region.left(), region.top() + i as f32 * ROW_H - scroll), Vec2::new(region.width(), ROW_H));
            if row.bottom() < region.top() || row.top() > region.bottom() {
                continue;
            }
            match r {
                Row::Page { name, count, .. } => {
                    let p = ui.painter();
                    p.text(
                        Pos2::new(row.left() + theme::PANEL_PADDING_X, row.center().y),
                        Align2::LEFT_CENTER,
                        elide(p, name, &theme::font_body_strong(), row.width() - 64.0),
                        theme::font_body_strong(),
                        theme::TEXT,
                    );
                    p.text(
                        Pos2::new(row.right() - theme::PANEL_PADDING_X, row.center().y),
                        Align2::RIGHT_CENTER,
                        count.to_string(),
                        theme::font_caption(),
                        theme::TEXT_TERTIARY,
                    );
                }
                Row::Item { id, page, name, status } => {
                    widgets::record(ui, &format!("dev-item-{}", id.0), row);
                    let resp = ui.interact(row, Id::new(("dev-ready-item", id.0)), Sense::click());
                    let selected = app.selection.contains(id);
                    let inner = row.shrink2(Vec2::new(8.0, 1.0));
                    let p = ui.painter();
                    if selected {
                        p.rect_filled(inner, theme::cr(theme::RADIUS_CONTROL), theme::BG_SELECTED);
                    } else if resp.hovered() {
                        p.rect_filled(inner, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
                    }
                    let pill_w = status_pill_width(p, *status);
                    let pill_at = Pos2::new(inner.right() - pill_w - 6.0, row.center().y - 10.0);
                    p.text(
                        Pos2::new(inner.left() + 16.0, row.center().y),
                        Align2::LEFT_CENTER,
                        elide(p, name, &theme::font_body(), inner.width() - pill_w - 32.0),
                        theme::font_body(),
                        theme::TEXT,
                    );
                    status_pill(p, pill_at, *status);
                    let _ = status_color;
                    if resp.clicked() {
                        go = Some((*page, *id));
                    }
                }
            }
        }
    });
    scrollbar(ui, region, scroll, content_h);
    ui.ctx().data_mut(|d| d.insert_temp(state, scroll));
    if let Some((page, id)) = go {
        if app.page != page {
            app.switch_page(page);
        }
        app.select_only(id);
        app.fit_selection();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_group_tracked_layers_under_their_pages() {
        let mut d = Document::new("t");
        let p1 = d.add_page("Home");
        let p2 = d.add_page("Empty");
        let p3 = d.add_page("Settings");
        let mk = |d: &mut Document, page: NodeId, name: &str| {
            let i = d.alloc_id();
            let mut n = Node::frame(i, designos_doc::Rect::new(0.0, 0.0, 10.0, 10.0));
            n.name = name.into();
            d.insert(page, n).unwrap()
        };
        let a = mk(&mut d, p1, "Hero");
        let b = mk(&mut d, p1, "Footer");
        let c = mk(&mut d, p3, "Form");
        mk(&mut d, p1, "Untracked");
        mark_ready(&d, a, "t", None).unwrap().apply(&mut d).unwrap();
        mark_completed(&d, b, "t").unwrap().apply(&mut d).unwrap();
        d.get_mut(c).unwrap().dev.status = Some(DevStatus::Changed);
        let _ = p2;
        assert_eq!(
            rows(&d),
            vec![
                Row::Page { id: p1, name: "Home".into(), count: 2 },
                Row::Item { id: a, page: p1, name: "Hero".into(), status: DevStatus::ReadyForDev },
                Row::Item { id: b, page: p1, name: "Footer".into(), status: DevStatus::Completed },
                Row::Page { id: p3, name: "Settings".into(), count: 1 },
                Row::Item { id: c, page: p3, name: "Form".into(), status: DevStatus::Changed },
            ]
        );
    }

    fn shape_text(shape: &egui::Shape, out: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(t) => out.push(t.galley.job.text.clone()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| shape_text(s, out)),
            _ => {}
        }
    }

    #[test]
    fn tabs_never_paint_under_the_layers_actions() {
        let mut d = Document::new("t");
        let page = d.add_page("Home");
        for i in 0..12 {
            let id = d.alloc_id();
            let mut n = Node::frame(id, designos_doc::Rect::new(0.0, 0.0, 10.0, 10.0));
            n.name = format!("Frame {i}");
            n.dev.status = Some(DevStatus::ReadyForDev);
            d.insert(page, n).unwrap();
        }
        let mut app = App::new(d);
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let row = Rect::from_min_size(Pos2::ZERO, Vec2::new(239.0, theme::PANEL_TITLE_ROW_HEIGHT));
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 80.0))), ..Default::default() };
        let out = ctx.run_ui(raw, |ui| tabs(&mut app, ui, row));
        let mut texts = Vec::new();
        for clipped in &out.shapes {
            shape_text(&clipped.shape, &mut texts);
        }
        assert!(texts.iter().all(|t| t != "12"), "the count is dropped before the label is elided, texts={texts:?}");
        assert!(texts.iter().any(|t| t.contains('…')), "the ready label elides once the count is gone, texts={texts:?}");
        let ready = ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", "dev-left-tab-ready".to_string())))).expect("ready tab");
        let layers = ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", "dev-left-tab-layers".to_string())))).expect("layers tab");
        let (primary, secondary) = crate::panels::left::action_cluster(row.right(), row.top());
        let cluster = primary.union(secondary);
        assert!(
            !ready.intersects(cluster) && !layers.intersects(cluster),
            "tabs ready {ready:?} and layers {layers:?} paint under the action cluster {cluster:?}"
        );
        assert!(ready.right() <= layers.left(), "the tabs keep their order: ready {ready:?}, layers {layers:?}");
        assert!(ready.left() >= row.left() + theme::PANEL_PADDING_X - 0.5, "the strip starts at the panel padding, got {ready:?}");
    }
}
