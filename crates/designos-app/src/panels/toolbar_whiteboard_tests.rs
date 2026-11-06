use super::*;
use crate::canvas::tools::Tool;
use designos_doc::Document;

fn temp_root(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("designos-wb-group-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn named(ctx: &egui::Context, name: &str) -> Option<Rect> {
    ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", name.to_string()))))
}

/// A toolbar frame at `screen` with an optional primary click, in a context that keeps its animation state.
fn frame(ctx: &egui::Context, app: &mut App, screen: Rect, click: Option<Pos2>) {
    let mut events = Vec::new();
    if let Some(pos) = click {
        events.push(egui::Event::PointerMoved(pos));
        events.push(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
        events.push(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    }
    let raw = egui::RawInput { screen_rect: Some(screen), events, ..Default::default() };
    let _ = ctx.run_ui(raw, |ui| super::super::toolbar::show(app, ui, screen));
}

fn setup() -> (egui::Context, App, Rect) {
    let ctx = egui::Context::default();
    theme::install_fonts(&ctx);
    let mut app = App::new(Document::new("WB"));
    app.wb_group.reduced_motion_override = Some(true);
    (ctx, app, Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0)))
}

#[test]
fn the_group_is_collapsed_on_a_new_design_file() {
    let (ctx, mut app, screen) = setup();
    assert_eq!(app.doc.kind, DocumentKind::Design);
    frame(&ctx, &mut app, screen, None);
    frame(&ctx, &mut app, screen, None);
    assert!(!app.wb_group.open);
    assert!(named(&ctx, "toolbar-whiteboard-group").is_some(), "the group button is on the Design toolbar");
    assert!(named(&ctx, "toolbar-whiteboard-tool-0").is_none(), "no row while collapsed");
}

#[test]
fn a_click_on_the_group_button_expands_and_collapses_it() {
    let (ctx, mut app, screen) = setup();
    frame(&ctx, &mut app, screen, None);
    frame(&ctx, &mut app, screen, None);
    let btn = named(&ctx, "toolbar-whiteboard-group").unwrap();
    frame(&ctx, &mut app, screen, Some(btn.center()));
    assert!(app.wb_group.open);
    frame(&ctx, &mut app, screen, None);
    frame(&ctx, &mut app, screen, None);
    for i in 0..ROW_TOOLS.len() {
        assert!(named(&ctx, &format!("toolbar-whiteboard-tool-{i}")).is_some(), "row tool {i} is shown");
    }
    let btn = named(&ctx, "toolbar-whiteboard-group").unwrap();
    frame(&ctx, &mut app, screen, Some(btn.center()));
    assert!(!app.wb_group.open);
}

#[test]
fn every_row_button_starts_a_tool_that_works_in_a_design_file() {
    let (ctx, mut app, screen) = setup();
    frame(&ctx, &mut app, screen, None);
    app.wb_group.toggle(None).unwrap();
    for _ in 0..3 {
        frame(&ctx, &mut app, screen, None);
    }
    for (i, tool) in ROW_TOOLS.iter().enumerate() {
        assert!(app.tool_offered(*tool), "{tool:?} is offered in a Design file");
        app.tool = Tool::Move;
        let r = named(&ctx, &format!("toolbar-whiteboard-tool-{i}")).unwrap();
        frame(&ctx, &mut app, screen, Some(r.center()));
        assert_eq!(app.tool, *tool, "button {i} activates {tool:?}");
    }
}

#[test]
fn the_open_state_is_saved_and_read_back() {
    let root = temp_root("save");
    let mut a = WbGroup::default();
    a.ensure_loaded(Some(&root));
    assert!(!a.open, "no settings file: collapsed");
    a.toggle(Some(&root)).unwrap();
    let mut b = WbGroup::default();
    b.ensure_loaded(Some(&root));
    assert!(b.open, "a new run reads the saved expanded state");
    b.toggle(Some(&root)).unwrap();
    let mut c = WbGroup::default();
    c.ensure_loaded(Some(&root));
    assert!(!c.open, "collapsing is saved too");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_corrupt_or_wrongly_typed_settings_file_means_collapsed() {
    for (tag, text) in [("garbage", "{ not json"), ("array", "[1,2]"), ("wrong-type", r#"{"toolbar.whiteboard_group_open": "yes"}"#), ("empty", "")] {
        let root = temp_root(tag);
        std::fs::write(root.join(crate::comments::settings::FILE), text).unwrap();
        let mut g = WbGroup::default();
        g.ensure_loaded(Some(&root));
        assert!(!g.open, "{tag}: collapsed");
        let _ = std::fs::remove_dir_all(&root);
    }
    let mut g = WbGroup::default();
    g.ensure_loaded(Some(Path::new("/nonexistent/designos/library")));
    assert!(!g.open, "a missing folder: collapsed");
}

#[test]
fn saving_keeps_the_other_settings() {
    let root = temp_root("keep");
    crate::comments::settings::save_flag(&root, "agent.autosave_ai_changes", false).unwrap();
    let mut g = WbGroup::default();
    g.toggle(Some(&root)).unwrap();
    assert_eq!(crate::comments::settings::load_flag(&root, "agent.autosave_ai_changes"), Some(false));
    assert_eq!(crate::comments::settings::load_flag(&root, SETTING_KEY), Some(true));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_failed_save_still_applies_and_reports() {
    let mut g = WbGroup::default();
    let err = g.toggle(Some(Path::new("/nonexistent/designos/library"))).unwrap_err();
    assert!(err.contains("Cannot save settings"), "{err}");
    assert!(g.open, "the choice applies in this run");
}

/// The open fraction of the first frame after a toggle, with the OS reduced-motion setting stood in by `reduced`.
fn first_fraction_after_toggle(reduced: bool) -> f32 {
    let ctx = egui::Context::default();
    let mut app = App::new(Document::new("Anim"));
    app.wb_group.reduced_motion_override = Some(reduced);
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
    let mut t = -1.0;
    let run = |app: &mut App, t: &mut f32| {
        let raw = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        let _ = ctx.run_ui(raw, |_ui| *t = open_fraction(app, &ctx));
    };
    run(&mut app, &mut t);
    assert_eq!(t, 0.0);
    app.wb_group.toggle(None).unwrap();
    run(&mut app, &mut t);
    t
}

#[test]
fn reduced_motion_opens_the_row_without_animation_frames() {
    assert_eq!(first_fraction_after_toggle(true), 1.0, "reduced motion: fully open on the first frame");
    let t = first_fraction_after_toggle(false);
    assert!(t > 0.0 && t < 1.0, "normal motion animates, first frame at {t}");
}

/// The row opens and closes in at most 150 ms.
const _: () = assert!(ANIM_SECS <= 0.15);

#[test]
fn a_saved_expanded_state_is_open_on_the_first_frame() {
    let root = temp_root("first-frame");
    crate::comments::settings::save_flag(&root, SETTING_KEY, true).unwrap();
    let (ctx, mut app, screen) = setup();
    app.wb_group.reduced_motion_override = Some(false);
    app.attach_library(root.clone());
    frame(&ctx, &mut app, screen, None);
    assert!(app.wb_group.open);
    assert!(named(&ctx, "toolbar-whiteboard-tool-0").is_some(), "the row is already out, not animating in at launch");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn row_slots_split_between_the_bar_and_the_more_menu() {
    let all = ROW_TOOLS.len() as f32 * SLOT_W;
    assert_eq!(fit_row(all), RowFit { shown: ROW_TOOLS.len(), more: false });
    assert_eq!(fit_row(all + 100.0), RowFit { shown: ROW_TOOLS.len(), more: false });
    let tight = fit_row(all - 1.0);
    assert!(tight.more && tight.shown < ROW_TOOLS.len());
    assert_eq!(fit_row(0.0), RowFit { shown: 0, more: true });
    assert_eq!(fit_row(-50.0), RowFit { shown: 0, more: true });
    for room in [0.0, 30.0, 70.0, 90.0, 400.0] {
        let fit = fit_row(room);
        assert!(fit.shown <= ROW_TOOLS.len());
        assert!(fit.width() <= room.max(SLOT_W) + 0.01, "room {room}: width {}", fit.width());
    }
}

#[test]
fn a_narrow_window_moves_the_tools_into_a_more_menu() {
    let (ctx, mut app, _) = setup();
    let narrow = Rect::from_min_size(Pos2::ZERO, Vec2::new(640.0, 700.0));
    frame(&ctx, &mut app, narrow, None);
    app.wb_group.toggle(None).unwrap();
    for _ in 0..3 {
        frame(&ctx, &mut app, narrow, None);
    }
    let bar = super::super::toolbar::toolbar_rect_x(narrow, extra_width(narrow, super::super::toolbar::toolbar_width(), 1.0));
    assert!(bar.left() >= 0.0 && bar.right() <= narrow.right(), "the bar {bar:?} stays inside a {} px window", narrow.width());
    let more = named(&ctx, "toolbar-whiteboard-more").expect("a ... button");
    assert!(bar.contains_rect(more), "the ... button is inside the bar");
    assert!(!app.wb_group.overflow.is_empty());
    assert!(named(&ctx, "toolbar-whiteboard-tool-0").is_none() || app.wb_group.overflow.len() < ROW_TOOLS.len());
    frame(&ctx, &mut app, narrow, Some(more.center()));
    assert_eq!(app.ui.menu, Some(MenuKind::WhiteboardMore));
}

#[test]
fn a_wide_window_needs_no_more_menu() {
    let (ctx, mut app, screen) = setup();
    frame(&ctx, &mut app, screen, None);
    app.wb_group.toggle(None).unwrap();
    for _ in 0..3 {
        frame(&ctx, &mut app, screen, None);
    }
    assert!(named(&ctx, "toolbar-whiteboard-more").is_none());
    assert!(app.wb_group.overflow.is_empty());
}

#[test]
fn the_bar_widens_by_the_group_and_nothing_else_when_collapsed() {
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
    let base = super::super::toolbar::toolbar_width();
    assert_eq!(base, 512.0, "the base toolbelt is unchanged");
    assert_eq!(extra_width(screen, base, 0.0), COLLAPSED_EXTRA);
    assert_eq!(COLLAPSED_EXTRA, 49.0, "divider 1 + gap 8 + button 32 + gap 8");
}

#[test]
fn deck_and_board_files_and_dev_mode_have_no_group() {
    for kind in [DocumentKind::Deck, DocumentKind::Board] {
        let mut d = Document::new("Other");
        d.kind = kind;
        let app = App::new(d);
        assert!(!available(&app), "{kind:?}");
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        assert_eq!(super::super::toolbar::bar_extra_settled(&app, screen), 0.0);
    }
    let mut app = App::new(Document::new("Dev"));
    assert!(available(&app));
    app.ui.dev_mode = true;
    assert!(!available(&app));
}
