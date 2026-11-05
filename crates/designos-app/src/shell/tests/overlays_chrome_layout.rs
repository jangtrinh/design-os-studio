//! Floating surfaces: where they sit and what they paint (command bar, dialogs, menus, comments, presentation).

use super::*;
use crate::util::world_aabb;
use crate::widgets::kit;
use egui::epaint::{RectShape, Shape, TextShape};

const COMMAND_ROWS: [&str; 12] = [
    "cmdk-open-terminal",
    "cmdk-populate-content",
    "cmdk-synth-spec",
    "cmdk-rename-layers",
    "cmdk-translate-vi",
    "cmdk-wire-prototype",
    "cmdk-semantic-rename",
    "cmdk-design-gate",
    "cmdk-check-designs",
    "cmdk-send-selection",
    "cmdk-open-specs",
    "cmdk-export-fig",
];

fn rects(s: &Sim) -> impl Iterator<Item = &RectShape> + '_ {
    s.shapes.iter().filter_map(|clipped| match &clipped.shape {
        Shape::Rect(shape) => Some(shape),
        _ => None,
    })
}

fn texts(s: &Sim) -> impl Iterator<Item = &TextShape> + '_ {
    s.shapes.iter().filter_map(|clipped| match &clipped.shape {
        Shape::Text(shape) => Some(shape),
        _ => None,
    })
}

fn text_starting(s: &Sim, start: &str) -> TextShape {
    texts(s).find(|t| t.galley.text().starts_with(start)).cloned().unwrap_or_else(|| panic!("no text starting {start:?} was painted"))
}

fn font_size_of(shape: &TextShape) -> f32 {
    shape.galley.job.sections.first().map(|section| section.format.font_id.size).expect("a painted text has a section")
}

/// The frame of a modal or popover: the smallest `BG` rect with the frame radius that holds `inner`.
fn frame_around(s: &Sim, inner: Rect) -> RectShape {
    rects(s)
        .filter(|shape| shape.fill == theme::BG && shape.corner_radius == theme::cr(theme::RADIUS_FRAME) && shape.rect.contains_rect(inner))
        .min_by(|a, b| (a.rect.width() * a.rect.height()).total_cmp(&(b.rect.width() * b.rect.height())))
        .cloned()
        .expect("a frame surface around the control")
}

/// One frame in a window of `size`, the way `Sim::frame` runs one in the default window.
fn frame_at(s: &mut Sim, size: Vec2, events: Vec<Event>) {
    s.t += 0.05;
    let mut raw = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), time: Some(s.t), events, modifiers: s.mods, ..Default::default() };
    let ctx = s.ctx.clone();
    let shell = &mut s.shell;
    eframe::App::raw_input_hook(shell, &ctx, &mut raw);
    let out = ctx.run_ui(raw, |ui| shell.draw(ui));
    s.shapes = out.shapes;
    s.shell.app.settle_paste_jobs();
}

/// Frames until the fade-in of a floating surface is over, so painted colours are the exact roles.
fn settle(s: &mut Sim) {
    for _ in 0..12 {
        s.frame(vec![]);
    }
}

fn key_event(key: Key, pressed: bool) -> Event {
    Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers: Modifiers::NONE }
}

fn open_command_bar_with(query: &str) -> Sim {
    let mut s = Sim::new();
    s.shell.app.cmdk.open = true;
    s.shell.app.cmdk.query = query.into();
    s
}

#[test]
fn command_bar_keeps_its_height_when_every_row_fits() {
    let mut s = open_command_bar_with("a");
    s.frame(vec![]);
    s.frame(vec![]);
    for id in COMMAND_ROWS {
        s.rect_of(id);
    }
    let bar = s.rect_of("cmdk-modal");
    let natural = 40.0 + 34.0 + 28.0 + COMMAND_ROWS.len() as f32 * 36.0 + 10.0;
    assert!((bar.height() - natural).abs() < 0.1, "bar {} natural {natural}", bar.height());
    let toolbar = crate::panels::toolbar::toolbar_rect(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 1088.0)));
    assert!((bar.bottom() - (toolbar.top() - 8.0)).abs() < 0.1, "docked 8 above the toolbar: bar {bar:?} toolbar {toolbar:?}");
}

#[test]
fn command_bar_stays_above_the_toolbar_at_the_minimum_window() {
    let mut s = open_command_bar_with("a");
    let size = Vec2::new(theme::MIN_WINDOW.0, theme::MIN_WINDOW.1);
    for _ in 0..12 {
        frame_at(&mut s, size, vec![]);
    }
    let bar = s.rect_of("cmdk-modal");
    let toolbar = crate::panels::toolbar::toolbar_rect(Rect::from_min_size(Pos2::ZERO, size));
    assert!(toolbar.contains_rect(s.rect_of("toolbar-actions-btn")), "the toolbar sits where the arithmetic says");
    assert!(bar.bottom() <= toolbar.top() - 8.0 + 0.1, "bar {bar:?} reaches the toolbar {toolbar:?}");
    assert!(bar.top() >= 0.0, "bar {bar:?} leaves the window");
    let first = s.rect_of(COMMAND_ROWS[0]);
    let second = s.rect_of(COMMAND_ROWS[1]);
    assert!((second.top() - first.top() - 36.0).abs() < 0.1, "rows keep their 36 pitch: {first:?} {second:?}");
    assert!((first.height() - theme::CONTROL_XS).abs() < 0.1);
}

#[test]
fn command_bar_scrolls_the_selected_row_into_view() {
    let mut s = open_command_bar_with("a");
    let size = Vec2::new(theme::MIN_WINDOW.0, theme::MIN_WINDOW.1);
    for _ in 0..12 {
        frame_at(&mut s, size, vec![]);
    }
    let bar = s.rect_of("cmdk-modal");
    let last = s.rect_of(COMMAND_ROWS[11]);
    let first_before = s.rect_of(COMMAND_ROWS[0]);
    assert!(last.bottom() > bar.bottom(), "the last row starts below the bar: {last:?} in {bar:?}");
    frame_at(&mut s, size, vec![key_event(Key::ArrowUp, true)]);
    frame_at(&mut s, size, vec![key_event(Key::ArrowUp, false)]);
    frame_at(&mut s, size, vec![]);
    let last = s.rect_of(COMMAND_ROWS[11]);
    assert!(last.bottom() <= bar.bottom() && last.top() >= bar.top() + 74.0, "ArrowUp wraps to the last row and scrolls it into the bar: {last:?} in {bar:?}");
    let first = s.rect_of(COMMAND_ROWS[0]);
    assert!(first.top() < first_before.top() - 1.0, "the list moved up to show the last row: {first:?} was {first_before:?}");
}

#[test]
fn command_bar_shows_hover_apart_from_the_selected_row() {
    let mut s = open_command_bar_with("");
    settle(&mut s);
    let first = s.rect_of("cmdk-open-terminal");
    let second = s.rect_of("cmdk-populate-content");
    s.mv(second.center());
    s.frame(vec![]);
    let hot = kit::command_row_look(false, true).fill;
    let chosen = kit::command_row_look(true, false).fill;
    assert_ne!(hot, chosen);
    assert!(rects(&s).any(|shape| shape.rect == second && shape.fill == hot), "the hovered row paints the hover fill");
    assert!(rects(&s).any(|shape| shape.rect == first && shape.fill == chosen), "the selected row keeps the selected fill");
}

#[test]
fn command_field_holds_no_permanent_focus_ring() {
    let mut s = open_command_bar_with("");
    s.shell.app.cmdk.focus_requested = true;
    settle(&mut s);
    assert!(s.ctx.memory(|m| m.focused().is_some()), "the command field holds the keyboard focus");
    assert!(!rects(&s).any(|shape| shape.stroke.color == theme::FOCUS_RING && shape.stroke.width > 0.0), "no orange box around the always-focused field");
}

#[test]
fn dialog_body_text_is_text_sm() {
    let mut s = Sim::new();
    s.shell.app.confirm_open_github();
    settle(&mut s);
    let body = text_starting(&s, "This opens the design:os studio");
    assert_eq!(font_size_of(&body), theme::FONT_SIZE_TEXT_SM);
}

#[test]
fn save_version_dialog_text_is_text_sm() {
    let mut s = Sim::new();
    s.shell.app.history.save_modal = Some(crate::history::SaveModal { name: String::new(), description: String::new(), fresh: true });
    settle(&mut s);
    assert_eq!(font_size_of(&text_starting(&s, "Version name")), theme::FONT_SIZE_TEXT_SM);
    assert_eq!(font_size_of(&text_starting(&s, "Description (optional)")), theme::FONT_SIZE_TEXT_SM);
}

#[test]
fn dialog_footer_sits_a_modal_inset_inside_a_borderless_frame() {
    let mut s = Sim::new();
    s.shell.app.confirm_open_github();
    settle(&mut s);
    let ok = s.rect_of("dialog-ok");
    let cancel = s.rect_of("dialog-cancel");
    assert!((ok.height() - theme::CONTROL_MD).abs() < 0.1 && (cancel.height() - theme::CONTROL_MD).abs() < 0.1, "ok {ok:?} cancel {cancel:?}");
    let frame = frame_around(&s, ok);
    assert!((frame.rect.right() - theme::INSET_MODAL - ok.right()).abs() < 0.1, "right inset: frame {:?} ok {ok:?}", frame.rect);
    assert!((frame.rect.bottom() - theme::INSET_MODAL - ok.bottom()).abs() < 0.1, "bottom inset: frame {:?} ok {ok:?}", frame.rect);
    assert_eq!(frame.stroke.width, 0.0, "the modal frame has no border");
}

#[test]
fn import_path_prompt_paints_its_hint_as_a_placeholder() {
    let mut s = Sim::new();
    s.shell.app.open_import_path_dialog();
    settle(&mut s);
    let hint = text_starting(&s, crate::home::dialog::IMPORT_PATH_HINT);
    let colour = hint.galley.job.sections.first().map(|section| section.format.color);
    assert_eq!(colour, Some(theme::TEXT_PLACEHOLDER));
    let ok = s.rect_of("dialog-ok");
    assert!((ok.height() - theme::CONTROL_MD).abs() < 0.1);
    assert!((frame_around(&s, ok).rect.bottom() - theme::INSET_MODAL - ok.bottom()).abs() < 0.1);
}

#[test]
fn present_link_prompt_is_a_sixty_pixel_card() {
    let mut s = Sim::with(Options { present: true, ..Default::default() });
    s.shell.app.proto.present.as_mut().expect("presentation").pending_url = Some(("https://example.com".into(), false));
    s.frame(vec![]);
    let open = s.rect_of("present-url-open");
    let cancel = s.rect_of("present-url-cancel");
    assert!((open.height() - theme::CONTROL_SM).abs() < 0.1 && (cancel.height() - theme::CONTROL_SM).abs() < 0.1);
    let card = frame_around(&s, open).rect;
    let bottom = s.ctx.content_rect().bottom();
    assert!((card.height() - 60.0).abs() < 0.1, "card {card:?}");
    assert!((card.top() - (bottom - 88.0)).abs() < 0.1, "card {card:?} in a window {bottom} high");
    assert!(card.contains_rect(cancel));
}

#[test]
fn context_menu_and_layer_picker_rows_are_menu_items() {
    let mut s = Sim::new();
    let b = world_aabb(&s.app().doc, s.by_name("Welcome"));
    s.right_click(s.screen(b.x + 40.0, b.y + 40.0));
    let row = s.rect_of("menu-row-0-0");
    assert!((row.height() - theme::MENU_ITEM_HEIGHT).abs() < 0.1, "row h {}", row.height());

    s.key(Key::Escape);
    s.frame(vec![]);
    let ids = vec![s.by_name("Welcome"), s.by_name("Hero"), s.by_name("Card")];
    let at = Pos2::new(700.0, 400.0);
    s.shell.app.ui.layer_picker = Some((at, ids.clone()));
    settle(&mut s);
    let rows: Vec<Rect> = (0..3).map(|i| s.rect_of(&format!("layer-row-{i}"))).collect();
    for (i, r) in rows.iter().enumerate() {
        assert!((r.height() - theme::MENU_ITEM_HEIGHT).abs() < 0.1, "row {i} {r:?}");
        assert!((r.left() - (at.x + theme::INSET_FRAME)).abs() < 0.1, "row {i} inset: {r:?}");
        assert!((r.width() - (220.0 - theme::INSET_FRAME * 2.0)).abs() < 0.1, "row {i} width: {r:?}");
        assert!((r.top() - (at.y + theme::MENU_ITEM_HEIGHT + i as f32 * theme::MENU_ITEM_HEIGHT)).abs() < 0.1, "row {i} pitch: {r:?}");
    }
    // Hover a layer that is not selected (the right click above selected the one under it), and check the selected row.
    let picked: Vec<bool> = ids.iter().map(|id| s.app().is_selected(*id)).collect();
    let (idle, chosen) = (picked.iter().position(|on| !on).expect("an unselected layer"), picked.iter().position(|on| *on));
    s.mv(rows[idle].center());
    let hot = kit::menu_row_look(false, true, false).fill;
    assert!(
        rects(&s).any(|shape| shape.rect == rows[idle] && shape.fill == hot && shape.corner_radius == theme::cr(theme::RADIUS_CONTROL)),
        "hovered row paints the menu item hover"
    );
    if let Some(chosen) = chosen {
        assert!(rects(&s).any(|shape| shape.rect == rows[chosen] && shape.fill == theme::BG_SELECTED), "a selected layer keeps the selected fill");
    }
}

#[test]
fn comment_message_dots_stay_clear_of_the_message_text() {
    let s = Sim::with(Options { fixture: Some("comments".into()), ..Default::default() });
    let open = s.app().comments.open.clone().expect("the fixture opens a thread");
    let thread = s.app().doc.comments.iter().find(|t| t.id == open).expect("the open thread");
    let message = thread.messages[0].text.clone();
    let head: String = message.chars().take(12).collect();
    let dots = s.rect_of("comment-dots-0");
    // The message text of the popover: the highest matching text that sits left of the dots, within the popover width.
    let body_top = texts(&s)
        .filter(|t| t.galley.text().starts_with(&head) && t.pos.x < dots.left() && t.pos.x > dots.right() - theme::COMMENT_POP_W && t.pos.y > dots.top())
        .map(|t| t.pos.y)
        .fold(f32::INFINITY, f32::min);
    assert!(body_top.is_finite(), "the first message is painted in the popover");
    assert!((dots.width() - theme::CONTROL_XS).abs() < 0.1 && (dots.height() - theme::CONTROL_XS).abs() < 0.1, "dots {dots:?}");
    assert!(dots.bottom() <= body_top + 0.1, "dots {dots:?} reach the message text at y {body_top}");
}

#[test]
fn deck_mode_bar_keeps_its_own_button_look() {
    let mut s = Sim::new();
    let ctx = s.ctx.clone();
    crate::panels::menus::run(&mut s.shell.app, &ctx, crate::panels::menus::Action::NewDeck);
    s.frame(vec![]);
    s.frame(vec![]);
    let active = s.rect_of("deck-slides-mode").shrink(2.0);
    let idle = s.rect_of("deck-layers-mode").shrink(2.0);
    assert!(
        rects(&s).any(|shape| shape.rect == active && shape.fill == theme::BG_BRAND && shape.corner_radius == theme::cr_pill(active.height())),
        "the active mode stays a brand pill"
    );
    assert!(!rects(&s).any(|shape| shape.rect == idle && shape.stroke.width > 0.0), "the idle mode stays a plain tile without a border");
}
