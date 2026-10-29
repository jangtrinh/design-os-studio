//! The Dev mode left panel: "Ready for dev" items per page with status pills.

use super::sections::{status_color, status_pill, status_pill_width};
use super::LeftTab;
use crate::app::App;
use crate::panels::{clipped, scrollbar, wheel_scroll};
use crate::theme;
use crate::widgets::{self, elide};
use designos_doc::*;
use egui::{Align2, Id, Pos2, Rect, Sense, Ui, Vec2};

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

/// Tab switcher replacing the "Layers" title in Dev mode.
pub fn tabs(app: &mut App, ui: &mut Ui, row: Rect) {
    let font = theme::font_body_strong();
    let mono_font = theme::font_mono();
    let h = theme::TARGET_MIN;
    let y = row.center().y - h * 0.5;
    let start = row.left() + theme::PANEL_PADDING_X;
    let search_left = row.right() - theme::PANEL_ACTION_RIGHT_PAD - theme::PANEL_ACTION_BUTTON;
    let cluster_left = search_left - theme::PAGES_SEARCH_DX;
    let budget = (cluster_left - theme::SPACE_FACT - start).max(0.0);
    let gap = theme::SPACE_FACT;
    let total: usize = rows(&app.doc).iter().filter(|r| matches!(r, Row::Item { .. })).count();
    let ready_label = "Ready for dev";
    let layers_label = "Layers";
    let label_w = widgets::text_width(ui.painter(), ready_label, &font).ceil();
    let layers_w = widgets::text_width(ui.painter(), layers_label, &font).ceil() + 2.0 * theme::INSET_CONTROL;
    let ready_budget = (budget - gap - layers_w).max(0.0);
    let badge_str = total.to_string();
    let badge_w = (widgets::text_width(ui.painter(), &badge_str, &mono_font) + 10.0).max(18.0);
    let with_badge = label_w + 6.0 + badge_w + 2.0 * theme::INSET_CONTROL;
    let bare = label_w + 2.0 * theme::INSET_CONTROL;
    let (ready_text, show_badge, ready_w) = if total > 0 && with_badge <= ready_budget {
        (ready_label.to_string(), true, with_badge)
    } else if bare <= ready_budget {
        (ready_label.to_string(), false, bare)
    } else {
        let room = (ready_budget - 2.0 * theme::INSET_CONTROL).max(0.0);
        let cut = elide(ui.painter(), ready_label, &font, room);
        let cut_w = widgets::text_width(ui.painter(), &cut, &font).ceil() + 2.0 * theme::INSET_CONTROL;
        (cut, false, cut_w.min(ready_budget))
    };
    let layers_draw = layers_w.min((budget - ready_w - gap).max(0.0));
    let specs = [(LeftTab::ReadyForDev, ready_text, ready_w, show_badge), (LeftTab::Layers, layers_label.to_string(), layers_draw, false)];
    let mut x = start;
    for (i, (tab, label, w, badge)) in specs.into_iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
        widgets::record(ui, if tab == LeftTab::ReadyForDev { "dev-left-tab-ready" } else { "dev-left-tab-layers" }, r);
        let resp = ui.interact(r, ui.id().with(("dev-left-tab", i)), Sense::click());
        let active = app.dev.left_tab == tab;
        let state = if active {
            crate::widgets::kit::State::Selected
        } else if resp.hovered() {
            crate::widgets::kit::State::Hover
        } else {
            crate::widgets::kit::State::Default
        };
        let paint = crate::widgets::kit::tab_look(state);
        crate::widgets::kit::paint_frame(ui.painter(), r, theme::RADIUS_CONTROL, &paint);
        ui.painter().text(Pos2::new(r.left() + theme::INSET_CONTROL, r.center().y), Align2::LEFT_CENTER, &label, font.clone(), paint.text);
        if badge {
            let badge_r = Rect::from_min_size(
                Pos2::new(r.left() + theme::INSET_CONTROL + label_w + 6.0, r.center().y - theme::LAYER_ICON_SIZE * 0.5),
                Vec2::new(badge_w, theme::LAYER_ICON_SIZE),
            );
            ui.painter().rect_filled(badge_r, theme::cr_pill(badge_r.height()), theme::DEV_READY.gamma_multiply(super::STATUS_TINT));
            ui.painter().text(badge_r.center(), Align2::CENTER_CENTER, &badge_str, mono_font.clone(), theme::TEXT);
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

    fn ready_app(n: usize) -> App {
        let mut doc = Document::new("t");
        let page = doc.add_page("Page");
        for i in 0..n {
            let id = doc.alloc_id();
            let mut node = Node::frame(id, designos_doc::Rect::new(0.0, 0.0, 10.0, 10.0));
            node.name = format!("Item {i}");
            doc.insert(page, node).unwrap();
            mark_ready(&doc, id, "t", None).unwrap().apply(&mut doc).unwrap();
        }
        App::new(doc)
    }

    fn paint_tabs(app: &mut App, row: Rect) -> (egui::Context, Vec<egui::epaint::ClippedShape>) {
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 400.0))), ..Default::default() };
        let shapes = ctx.run_ui(raw, |ui| tabs(app, ui, row)).shapes;
        (ctx, shapes)
    }

    fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<(String, egui::Color32)> {
        shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::epaint::Shape::Text(text) => Some((text.galley.text().to_string(), text.fallback_color)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn count_badge_uses_the_dev_accent() {
        let mut app = ready_app(3);
        let row = Rect::from_min_size(Pos2::new(0.0, 40.0), Vec2::new(800.0, theme::LAYER_HEADER_HEIGHT));
        let (_, shapes) = paint_tabs(&mut app, row);
        let tint = theme::DEV_READY.gamma_multiply(0.22);
        assert!(
            shapes.iter().any(|clipped| matches!(&clipped.shape, egui::epaint::Shape::Rect(shape) if shape.fill == tint)),
            "count fill is the dev accent at the status tint"
        );
        let painted = texts(&shapes);
        assert!(painted.iter().any(|(label, color)| label == "3" && *color == theme::TEXT), "count label is primary text, got {painted:?}");
        let mut empty = ready_app(0);
        let (_, none) = paint_tabs(&mut empty, row);
        assert!(texts(&none).iter().all(|(label, _)| label != "0"), "a zero count is not drawn");
    }

    #[test]
    fn tabs_never_paint_under_the_layers_actions() {
        let mut app = ready_app(12);
        let row = Rect::from_min_size(Pos2::new(49.0, 80.0), Vec2::new(theme::LEFT_PANEL_WIDTH + 1.0, theme::LAYER_HEADER_HEIGHT));
        let (ctx, shapes) = paint_tabs(&mut app, row);
        let search_left = row.right() - theme::PANEL_ACTION_RIGHT_PAD - theme::PANEL_ACTION_BUTTON;
        let cluster_left = search_left - theme::PAGES_SEARCH_DX;
        let limit = cluster_left - theme::SPACE_FACT;
        let recorded = |name: &str| ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", name.to_string()))));
        let ready = recorded("dev-left-tab-ready").expect("ready tab");
        let layers = recorded("dev-left-tab-layers").expect("layers tab");
        assert!(ready.right() <= limit + 0.5, "ready ends at {}, actions start at {limit}", ready.right());
        assert!(layers.right() <= limit + 0.5, "layers ends at {}, actions start at {limit}", layers.right());
        assert!(texts(&shapes).iter().any(|(label, color)| label == "Ready for dev" && *color == theme::TEXT_SECONDARY));
        assert!(texts(&shapes).iter().all(|(label, _)| label != "12"), "the count drops before the label is cut");
    }
}
