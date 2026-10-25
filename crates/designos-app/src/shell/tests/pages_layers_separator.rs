use super::*;
use crate::theme;
use egui::epaint::{ClippedShape, Shape};
use egui::Color32;

fn shape_has_hline_stroke(shape: &Shape, y: f32, width: f32, color: Color32) -> bool {
    match shape {
        Shape::LineSegment { points, stroke } => {
            let [a, b] = *points;
            let same_y = (a.y - y).abs() <= 1.5 && (b.y - y).abs() <= 1.5;
            let same_w = (stroke.width - width).abs() <= 0.1;
            same_y && same_w && stroke.color == color && (b.x - a.x).abs() > 100.0
        }
        Shape::Vec(children) => children.iter().any(|c| shape_has_hline_stroke(c, y, width, color)),
        _ => false,
    }
}

fn has_hline_stroke(shapes: &[ClippedShape], y: f32, width: f32, color: Color32) -> bool {
    shapes.iter().any(|cs| shape_has_hline_stroke(&cs.shape, y, width, color))
}

fn shape_has_rect_stroke_color(shape: &Shape, target: Rect, color: Color32) -> bool {
    match shape {
        Shape::Rect(r) => {
            let stroke = r.stroke;
            let matches_rect = (r.rect.min.x - target.min.x).abs() <= 2.0
                && (r.rect.max.x - target.max.x).abs() <= 2.0
                && (r.rect.center().y - target.center().y).abs() <= 2.0;
            matches_rect && stroke.width > 0.0 && stroke.color == color
        }
        Shape::Vec(children) => children.iter().any(|c| shape_has_rect_stroke_color(c, target, color)),
        _ => false,
    }
}

fn has_focus_ring_around(shapes: &[ClippedShape], target: Rect, color: Color32) -> bool {
    shapes.iter().any(|cs| shape_has_rect_stroke_color(&cs.shape, target, color))
}

#[test]
fn dragging_the_separator_changes_the_pages_and_layers_split() {
    let mut s = Sim::new();
    for i in 4..=6 {
        s.shell.app.doc.add_page(format!("Page {i}"));
    }
    s.frame(vec![]);

    let sep0 = s.rect_of("pages-layers-separator");
    let pages0 = s.rect_of("pages-list");
    let layers0 = s.rect_of("layers-list");
    let hit_h = 6.0;
    assert!((sep0.height() - hit_h).abs() < 0.1, "separator hit zone must be 6 px tall, got {:?}", sep0);
    let default_pages_h = theme::PAGES_MAX_VISIBLE_ROWS as f32 * theme::PAGE_ROW_HEIGHT;
    assert!((pages0.height() - default_pages_h).abs() < 0.1, "default 6-page height must be 4 rows (96 px), got {}", pages0.height());

    // Hovering shows ResizeVertical cursor and 2px brand-colour highlight.
    let center = sep0.center();
    s.mv(center);
    assert_eq!(s.cursor, egui::CursorIcon::ResizeVertical, "hovering separator must show ResizeVertical cursor");
    assert!(has_hline_stroke(&s.shapes, center.y, 2.0, theme::BG_BRAND), "hovering separator must paint 2px brand-colour highlight");

    // Drag up by 24 px: Pages shrinks by 24 px and Layers grows by 24 px.
    let delta = theme::PAGE_ROW_HEIGHT;
    let up_target = Pos2::new(center.x, center.y - delta);
    s.drag(center, up_target);
    let pages_up = s.rect_of("pages-list");
    let layers_up = s.rect_of("layers-list");
    assert!((pages_up.height() - (pages0.height() - delta)).abs() < 0.1, "Pages height after dragging up: {}", pages_up.height());
    assert!((layers_up.height() - (layers0.height() + delta)).abs() < 0.1, "Layers height after dragging up: {}", layers_up.height());

    // Drag down by 48 px (from 72 px to 120 px): Pages grows and Layers shrinks.
    let sep_up = s.rect_of("pages-layers-separator").center();
    let grow = delta * 2.0;
    let down_target = Pos2::new(sep_up.x, sep_up.y + grow);
    s.drag(sep_up, down_target);
    let pages_down = s.rect_of("pages-list");
    let layers_down = s.rect_of("layers-list");
    assert!((pages_down.height() - (pages_up.height() + grow)).abs() < 0.1, "Pages height after dragging down: {}", pages_down.height());
    assert!((layers_down.height() - (layers_up.height() - grow)).abs() < 0.1, "Layers height after dragging down: {}", layers_down.height());
}

#[test]
fn dragging_the_separator_clamps_at_min_and_max_bounds() {
    let mut s = Sim::new();
    // Base demo document has 3 pages (72 px content).
    let sep0 = s.rect_of("pages-layers-separator").center();

    // Drag far above the panel: Pages clamps to 1 row (24 px) plus its 40 px header.
    s.drag(sep0, Pos2::new(sep0.x, 5.0));
    let pages_min = s.rect_of("pages-list");
    let header_rect = s.rect_of("pages-toggle");
    assert!((pages_min.height() - theme::PAGE_ROW_HEIGHT).abs() < 0.1, "Pages min list height must be 1 row (24 px), got {}", pages_min.height());
    assert!(
        ((pages_min.bottom() - header_rect.top()) - (theme::PANEL_TITLE_ROW_HEIGHT + theme::PAGE_ROW_HEIGHT)).abs() < 0.1,
        "Pages section min total height must be header (40 px) + 1 row (24 px)"
    );

    // With 3 pages, dragging far down cannot exceed content height (3 * 24 = 72 px).
    let sep_min = s.rect_of("pages-layers-separator").center();
    s.drag(sep_min, Pos2::new(sep_min.x, 900.0));
    let pages_content_max = s.rect_of("pages-list");
    let three_rows = 3.0 * theme::PAGE_ROW_HEIGHT;
    assert!((pages_content_max.height() - three_rows).abs() < 0.1, "Pages must never be taller than its 3 rows (72 px), got {}", pages_content_max.height());

    // Add many pages (45 total) and drag far down: Layers must keep at least 120 px.
    for i in 4..=45 {
        s.shell.app.doc.add_page(format!("Page {i}"));
    }
    s.frame(vec![]);
    let sep_mid = s.rect_of("pages-layers-separator").center();
    s.drag(sep_mid, Pos2::new(sep_mid.x, 1050.0));
    let layers_clamped = s.rect_of("layers-list");
    let min_layers_h = 120.0;
    assert!(
        (layers_clamped.height() - min_layers_h).abs() < 0.5 && layers_clamped.height() >= min_layers_h - 0.1,
        "Layers must stay at least 120 px tall when clamped at bottom, got {}",
        layers_clamped.height()
    );

    // Turn on Dev Mode ("Ready for development") and verify layout remains intact and Layers >= 120 px.
    s.shell.app.ui.dev_mode = true;
    s.frame(vec![]);
    let dev_toggle = s.rect_of("dev-ready-toggle");
    let pages_dev = s.rect_of("pages-list");
    let layers_dev = s.rect_of("layers-list");
    assert!(dev_toggle.top() >= pages_dev.bottom(), "Dev Mode section must sit below Pages");
    assert!(layers_dev.top() > dev_toggle.bottom(), "Layers must sit below Dev Mode section");
    assert!(layers_dev.height() >= min_layers_h - 0.1, "Layers must still keep >= 120 px in Dev Mode, got {}", layers_dev.height());
}

#[test]
fn separator_height_persists_across_restart() {
    let tmp = std::env::temp_dir().join(format!("designos-sep-persist-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("create temp library dir");

    let target_h = 2.0 * theme::PAGE_ROW_HEIGHT;
    {
        let mut s = Sim::with(Options { library: Some(tmp.clone()), ..Default::default() });
        let sep = s.rect_of("pages-layers-separator").center();
        let delta = theme::PAGE_ROW_HEIGHT;
        s.drag(sep, Pos2::new(sep.x, sep.y - delta));
        assert!((s.rect_of("pages-list").height() - target_h).abs() < 0.1);
    }

    {
        let s2 = Sim::with(Options { library: Some(tmp.clone()), ..Default::default() });
        let persisted_h = s2.rect_of("pages-list").height();
        assert!((persisted_h - target_h).abs() < 0.1, "restarted session must restore persisted Pages height {target_h}, got {persisted_h}");
    }

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn double_clicking_the_separator_resets_to_default_height() {
    let tmp = std::env::temp_dir().join(format!("designos-sep-dblclick-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("create temp library dir");

    let mut s = Sim::with(Options { library: Some(tmp.clone()), ..Default::default() });
    let default_h = s.rect_of("pages-list").height();
    let sep = s.rect_of("pages-layers-separator").center();
    let delta = theme::PAGE_ROW_HEIGHT;
    s.drag(sep, Pos2::new(sep.x, sep.y - delta));
    assert!((s.rect_of("pages-list").height() - (default_h - delta)).abs() < 0.1);

    // Double-click the separator to reset to today's default height.
    let sep_now = s.rect_of("pages-layers-separator").center();
    s.click(sep_now);
    s.click(sep_now);
    assert!(
        (s.rect_of("pages-list").height() - default_h).abs() < 0.1,
        "double-click must reset Pages height to {default_h}, got {}",
        s.rect_of("pages-list").height()
    );

    // Restart also sees the reset default height.
    drop(s);
    let s2 = Sim::with(Options { library: Some(tmp.clone()), ..Default::default() });
    assert!((s2.rect_of("pages-list").height() - default_h).abs() < 0.1);
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn keyboard_navigation_steps_clamps_and_resets_with_accesskit_and_focus_ring() {
    let mut s = Sim::new();
    crate::agent_a11y::enable_accesskit(&s.ctx);
    for i in 4..=6 {
        s.shell.app.doc.add_page(format!("Page {i}"));
    }
    let sep_id = egui::Id::new("pages-layers-separator");
    s.ctx.memory_mut(|m| m.request_focus(sep_id));
    s.frame(vec![]);

    let sep_rect = s.rect_of("pages-layers-separator");
    assert!(has_focus_ring_around(&s.shapes, sep_rect, theme::BORDER_SELECTED), "focused separator must draw a visible focus ring with theme::BORDER_SELECTED");

    let default_h = theme::PAGES_MAX_VISIBLE_ROWS as f32 * theme::PAGE_ROW_HEIGHT;
    let min_h = theme::PAGE_ROW_HEIGHT;
    let max_h = 6.0 * theme::PAGE_ROW_HEIGHT;

    let update = s.accesskit_update.as_ref().expect("AccessKit TreeUpdate expected");
    let (_, ak_node) = update.nodes.iter().find(|(id, _)| *id == sep_id.accesskit_id()).expect("separator AccessKit node must exist");
    assert_eq!(ak_node.role(), egui::accesskit::Role::Splitter);
    assert_eq!(ak_node.orientation(), Some(egui::accesskit::Orientation::Horizontal));
    assert_eq!(ak_node.numeric_value(), Some(default_h as f64));
    assert_eq!(ak_node.min_numeric_value(), Some(min_h as f64));
    assert_eq!(ak_node.max_numeric_value(), Some(max_h as f64));

    let step = 8.0;
    let shift_step = 40.0;

    // ArrowDown increases Pages height by 8 px; ArrowUp decreases by 8 px.
    s.key(Key::ArrowDown);
    assert!((s.rect_of("pages-list").height() - (default_h + step)).abs() < 0.1);
    s.key(Key::ArrowUp);
    assert!((s.rect_of("pages-list").height() - default_h).abs() < 0.1);

    // Shift + ArrowUp decreases by 40 px; Shift + ArrowDown increases by 40 px.
    s.mods = Modifiers::SHIFT;
    s.key(Key::ArrowUp);
    assert!((s.rect_of("pages-list").height() - (default_h - shift_step)).abs() < 0.1);
    s.key(Key::ArrowDown);
    s.mods = Modifiers::NONE;
    assert!((s.rect_of("pages-list").height() - default_h).abs() < 0.1);

    // Home goes to min (24 px); End goes to max (144 px); Enter resets to default (96 px).
    s.key(Key::Home);
    assert!((s.rect_of("pages-list").height() - min_h).abs() < 0.1);
    s.key(Key::End);
    assert!((s.rect_of("pages-list").height() - max_h).abs() < 0.1);
    s.key(Key::Enter);
    assert!((s.rect_of("pages-list").height() - default_h).abs() < 0.1);
}

#[test]
fn slow_frame_drag_ends_at_the_release_point() {
    let delta = theme::PAGE_ROW_HEIGHT;
    let expected_h = 2.0 * theme::PAGE_ROW_HEIGHT;

    // 1. OneFrame: press, moves, and release all arrive in a single frame.
    {
        let mut s = Sim::new();
        let start = s.rect_of("pages-layers-separator").center();
        let target = Pos2::new(start.x, start.y - delta);
        s.drag_in_one_frame(start, target);
        assert!(
            (s.rect_of("pages-list").height() - expected_h).abs() < 0.1,
            "OneFrame drag must land at release point height {expected_h}, got {}",
            s.rect_of("pages-list").height()
        );
    }

    // 2. ReleaseFrame: final move and release share a frame, then pointer moves elsewhere.
    {
        let mut s = Sim::new();
        let start = s.rect_of("pages-layers-separator").center();
        let mid = Pos2::new(start.x, start.y - 8.0);
        let target = Pos2::new(start.x, start.y - delta);
        s.mv(start);
        s.button(true);
        s.mv(mid);
        s.one_frame().move_to(target).release().send();
        s.mv(Pos2::new(700.0, 500.0));
        assert!(
            (s.rect_of("pages-list").height() - expected_h).abs() < 0.1,
            "ReleaseFrame drag must land at release point height {expected_h}, got {}",
            s.rect_of("pages-list").height()
        );
    }

    // 3. PointerElsewhere: pointer rests on the canvas before moving to separator, pressing, dragging, and releasing in one frame.
    {
        let mut s = Sim::new();
        let start = s.rect_of("pages-layers-separator").center();
        let mid = Pos2::new(start.x, start.y - 12.0);
        let target = Pos2::new(start.x, start.y - delta);
        s.mv(Pos2::new(700.0, 500.0));
        s.one_frame().move_to(start).press().move_to(mid).move_to(target).release().send();
        s.frame(vec![]);
        assert!(
            (s.rect_of("pages-list").height() - expected_h).abs() < 0.1,
            "PointerElsewhere drag must land at release point height {expected_h}, got {}",
            s.rect_of("pages-list").height()
        );
    }
}
