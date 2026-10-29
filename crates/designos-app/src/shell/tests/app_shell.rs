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

#[test]
fn toast_rules_for_d() {
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
    assert!((band.width() - theme::TARGET_MIN).abs() < 0.5, "band is 24 wide, got {}", band.width());
    assert!((handle.width() - theme::PAGES_SEPARATOR_HIT).abs() < 0.5, "painted core stays 6 wide");
    assert!((band.center().x - handle.center().x).abs() < 0.5, "band is centred on the panel edge");

    let edge = handle.center().x;
    let before = sim.app().ui.left_panel_width;
    sim.mv(Pos2::new(edge - 10.0, 400.0));
    sim.button(true);
    sim.pos = Pos2::new(edge - 10.0, 360.0);
    sim.frame(vec![Event::PointerMoved(sim.pos)]);
    sim.button(false);
    assert!((sim.app().ui.left_panel_width - before).abs() < 0.5, "a press over the scrollbar scrolls");

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
