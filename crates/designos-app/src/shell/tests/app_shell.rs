//! App-shell paint and hit targets: window clear, title bar, toast, resize band.

use super::*;

fn lin(channel: u8) -> f32 {
    let c = channel as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn circles(shapes: &[egui::epaint::ClippedShape]) -> Vec<egui::epaint::CircleShape> {
    fn walk(shape: &egui::Shape, out: &mut Vec<egui::epaint::CircleShape>) {
        match shape {
            egui::Shape::Circle(c) => out.push(*c),
            egui::Shape::Vec(inner) => inner.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for clipped in shapes {
        walk(&clipped.shape, &mut out);
    }
    out
}

fn rects(shapes: &[egui::epaint::ClippedShape]) -> Vec<egui::epaint::RectShape> {
    fn walk(shape: &egui::Shape, out: &mut Vec<egui::epaint::RectShape>) {
        match shape {
            egui::Shape::Rect(r) => out.push(r.clone()),
            egui::Shape::Vec(inner) => inner.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for clipped in shapes {
        walk(&clipped.shape, &mut out);
    }
    out
}

fn recorded(sim: &Sim, name: &str) -> Option<Rect> {
    sim.ctx.data(|d| d.get_temp(egui::Id::new(("test-rect", name.to_string()))))
}

#[test]
fn window_clear_is_the_page_background() {
    let sim = Sim::built(Options::default());
    let got = eframe::App::clear_color(&sim.shell, &egui::Visuals::dark());
    let bg = theme::BG.to_array();
    let expect = [lin(bg[0]), lin(bg[1]), lin(bg[2]), 1.0];
    for i in 0..4 {
        assert!((got[i] - expect[i]).abs() < 1.0e-4, "clear {got:?} expected {expect:?}");
    }
}

#[test]
fn title_bar_uses_the_page_background() {
    let mut sim = Sim::new();
    sim.shell.app.tabs.always_visible = true;
    sim.frame(vec![]);
    let painted = rects(&sim.shapes);
    let legacy = egui::Color32::from_rgb(0x18, 0x18, 0x1B);
    assert!(painted.iter().all(|r| r.fill != legacy), "title bar still uses the old fill");
    assert!(
        painted.iter().any(|r| r.fill == theme::BG_PRIMARY && (r.rect.height() - 28.0).abs() < 0.5 && r.rect.top() < 0.5),
        "title bar fills with the page background"
    );
    assert!(recorded(&sim, "window-title-brand").is_some());
}

fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
    fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::Shape::Vec(inner) => inner.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for clipped in shapes {
        walk(&clipped.shape, &mut out);
    }
    out
}

fn font_of(shapes: &[egui::epaint::ClippedShape], text: &str) -> Option<egui::FontId> {
    fn walk(shape: &egui::Shape, text: &str) -> Option<egui::FontId> {
        match shape {
            egui::Shape::Text(t) if t.galley.text() == text => t.galley.job.sections.first().map(|s| s.format.font_id.clone()),
            egui::Shape::Vec(inner) => inner.iter().find_map(|s| walk(s, text)),
            _ => None,
        }
    }
    shapes.iter().find_map(|c| walk(&c.shape, text))
}

#[test]
fn the_import_toast_shows_details_once_as_a_button() {
    let mut sim = Sim::new();
    let report = designos_webcapture::ImportReport::default();
    let summary = report.summary_toast();
    assert!(summary.ends_with("Details"), "precondition: the import summary carries a Details hint, got {summary:?}");
    sim.shell.app.web_capture_report = Some(report);
    sim.shell.app.show_toast(summary, false);
    sim.frame(vec![]);
    sim.frame(vec![]);
    let painted = texts(&sim.shapes);
    let message = painted.iter().find(|t| t.starts_with("Imported page:")).expect("the toast message is painted");
    assert!(!message.ends_with("Details"), "the message does not repeat the button's word, got {message:?}");
    assert!(!message.trim_end().ends_with('\u{00b7}'), "no dangling separator is left, got {message:?}");
    assert_eq!(painted.iter().filter(|t| t.as_str() == "Details").count(), 1, "Details is painted once, as the button label");
    assert!(recorded(&sim, "toast-details").is_some(), "the button is there");
    assert_eq!(font_of(&sim.shapes, "Details"), Some(theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold)), "an action label is Semibold");
    assert_eq!(font_of(&sim.shapes, message), Some(theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Medium)), "the message stays Medium");
    assert!(recorded(&sim, "toast-details-message").is_none(), "the message is not a second Details target");
}

#[test]
fn toast_follows_the_toast_surface_rules() {
    let mut sim = Sim::new();
    sim.shell.app.show_toast_with_reveal("Exported the file", std::path::PathBuf::from("/tmp/out.designos"));
    sim.frame(vec![]);
    let painted = rects(&sim.shapes);
    let toast_fill = painted.iter().find(|r| r.fill == theme::BG_PRIMARY_ALT && r.corner_radius == theme::cr(theme::RADIUS_XL));
    assert!(toast_fill.is_some(), "toast surface is the alt page fill at the card radius");
    let toast_rect = toast_fill.unwrap().rect;
    assert!(
        painted.iter().any(|r| r.rect == toast_rect && r.stroke.color == theme::BORDER_MENU && r.stroke.width > 0.0),
        "toast surface draws the menu border and no shadow"
    );
    assert!(circles(&sim.shapes).iter().any(|c| c.fill == theme::ICON_BRAND_PRIMARY), "a reveal toast uses the info dot");
    assert!(circles(&sim.shapes).iter().all(|c| c.fill != theme::SUCCESS), "no caller paints the success dot");
    let reveal = recorded(&sim, "toast-reveal").expect("reveal action");
    assert!(reveal.height() + 0.5 >= theme::CONTROL_XS, "reveal target is at least 32 tall, got {}", reveal.height());

    sim.shell.app.toast = None;
    sim.shell.app.web_capture_report = Some(designos_webcapture::ImportReport::default());
    sim.shell.app.show_toast("Imported page: 1 layer · Details", false);
    sim.frame(vec![]);
    let details = recorded(&sim, "toast-details").expect("details action");
    assert!(details.height() + 0.5 >= theme::CONTROL_XS, "details is a button, not the whole toast");
    assert!(details.width() + 0.5 < 200.0, "details does not cover the toast");

    sim.shell.app.toast = None;
    sim.shell.app.web_capture_report = None;
    sim.shell.app.show_toast("Choose a file", false);
    if let Some(t) = sim.shell.app.toast.as_mut() {
        t.enter_path = true;
    }
    sim.frame(vec![]);
    let enter = recorded(&sim, "toast-enter-path").expect("enter path action");
    assert!(enter.height() + 0.5 >= theme::CONTROL_XS, "enter path target is at least 32 tall");
    assert!(circles(&sim.shapes).iter().any(|c| c.fill == theme::ICON_BRAND_PRIMARY), "enter path keeps the info dot");

    sim.shell.app.toast = None;
    sim.shell.app.show_toast("Export failed", true);
    sim.frame(vec![]);
    assert!(circles(&sim.shapes).iter().any(|c| c.fill == theme::DANGER), "an error toast uses the danger dot");
}

#[test]
fn left_panel_resize_hit_band_meets_the_target_floor() {
    let mut sim = Sim::new();
    sim.shell.app.ui.rulers = false;
    sim.frame(vec![]);
    let band = recorded(&sim, "left-panel-resize-band").expect("resize band");
    let handle = recorded(&sim, "left-panel-resize-handle").expect("resize handle");
    assert!((band.width() - theme::DRAG_DIVIDER_HIT).abs() < 0.5, "band is 24 wide, got {}", band.width());
    assert!((handle.width() - theme::PAGES_SEPARATOR_HIT).abs() < 0.5, "painted core stays 6 wide");
    assert!((band.center().x - handle.center().x).abs() < 0.5, "band is centred on the panel edge");
    assert!(
        band.max.x < handle.center().x + theme::DRAG_DIVIDER_HIT * 0.5,
        "the canvas side is half open: a click 12 px right of the edge belongs to the canvas, band ends at {}",
        band.max.x
    );

    // Enough layers that the list scrolls and draws its scrollbar, which keeps its track under the band.
    let page = sim.app().page;
    for i in 0..40 {
        let id = sim.shell.app.doc.alloc_id();
        sim.shell
            .app
            .doc
            .insert(page, designos_doc::Node::rectangle(id, designos_doc::Rect::new(i as f32 * 4.0, 0.0, 10.0, 10.0)))
            .expect("page takes the rectangle");
    }
    sim.frame(vec![]);
    sim.frame(vec![]);
    let track = recorded(&sim, "scrollbar-track").expect("the layers list draws a scrollbar");
    assert!(band.contains(track.center()) && !handle.contains(track.center()), "the track sits in the band, outside the 6 px core");
    let (width, scrolled) = (sim.app().ui.left_panel_width, sim.app().layers.scroll);
    // the thumb is most of the track (the list is only a little longer than its window), so the grab goes far down
    sim.drag(track.center_top() + Vec2::new(0.0, 20.0), track.center_top() + Vec2::new(0.0, 700.0));
    assert!(sim.app().layers.scroll > scrolled + 1.0, "a press and drag on the scrollbar track scrolls the list, offset {}", sim.app().layers.scroll);
    assert!((sim.app().ui.left_panel_width - width).abs() < 0.5, "the same drag does not resize the panel");

    let edge = recorded(&sim, "left-panel-resize-handle").unwrap().center().x;
    let before = sim.app().ui.left_panel_width;
    sim.mv(Pos2::new(edge + 8.0, 420.0));
    sim.button(true);
    sim.pos = Pos2::new(edge + 28.0, 420.0);
    sim.frame(vec![Event::PointerMoved(sim.pos)]);
    sim.button(false);
    assert!(sim.app().ui.left_panel_width > before + 8.0, "rulers off: a press 8px right of the edge resizes");

    sim.shell.app.ui.rulers = true;
    sim.frame(vec![]);
    let edge = recorded(&sim, "left-panel-resize-handle").unwrap().center().x;
    let before = sim.app().ui.left_panel_width;
    sim.shell.app.canvas.inter = canvas::Interaction::None;
    sim.mv(Pos2::new(edge + 8.0, 440.0));
    sim.button(true);
    assert!(matches!(sim.app().canvas.inter, canvas::Interaction::Guide(_)), "rulers on: the same press starts a guide");
    assert!((sim.app().ui.left_panel_width - before).abs() < 0.5, "rulers on: that press does not resize");
}

fn guide_count(sim: &Sim) -> usize {
    sim.app().doc.guides.get(&sim.app().page).map_or(0, Vec::len)
}

#[test]
fn a_click_without_a_drag_beside_a_ruler_leaves_no_guide() {
    let mut sim = Sim::new();
    sim.shell.app.ui.rulers = true;
    sim.frame(vec![]);
    let rect = sim.app().canvas_rect;
    // 10 px is the ruler itself, 22 px the 4 px press band under it, and the same beside the left ruler
    for (dx, dy) in [(100.0, 10.0), (100.0, 22.0), (10.0, 100.0), (22.0, 100.0)] {
        sim.click(rect.min + Vec2::new(dx, dy));
        assert_eq!(guide_count(&sim), 0, "a click at ({dx}, {dy}) from the canvas corner drops no guide");
    }
    // a real drag out of the band still places the guide
    let from = rect.min + Vec2::new(100.0, 22.0);
    sim.drag(from, from + Vec2::new(0.0, 120.0));
    assert_eq!(guide_count(&sim), 1, "dragging out of the band into the canvas places a guide");
}
