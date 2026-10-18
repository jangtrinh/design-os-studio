//! UI tests for prototype mode: the Prototype tab, canvas hotspots and the Present view.

use super::*;
use crate::app::RightTab;
use crate::proto::player::When;
use designos_doc::{Action, Direction, Easing, OverlayPosition, Reaction, Transition, TransitionKind, Trigger};

fn proto_sim(select: Option<&str>) -> Sim {
    let mut s = Sim::with(Options { select: select.map(String::from), tab: Some("prototype".into()), ..Default::default() });
    s.frame(vec![]);
    s
}

fn reactions(s: &Sim, name: &str) -> Vec<Reaction> {
    s.app().doc.get(s.by_name(name)).unwrap().prototype.reactions.clone()
}

/// Clicks item `i` of the dropdown menu that opened under `ctl`. The row is found from the panel the
/// popup drew: a menu taller than the window is clamped to the window instead of opening under `ctl`.
fn pick(s: &mut Sim, name: &str, i: usize) {
    let ctl = s.rect_of(name);
    s.click(ctl.center());
    let menu = s.rect_of("menu-level-0");
    let y = menu.top() + theme::MENU_PADDING + i as f32 * theme::MENU_ITEM_HEIGHT + theme::MENU_ITEM_HEIGHT / 2.0;
    s.click(Pos2::new(ctl.left() + 20.0, y));
}

fn field_dbg(s: &Sim, key: impl std::fmt::Debug) -> Rect {
    s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("field-rect", format!("{key:?}"))))).expect("field recorded")
}

fn present_open(s: &mut Sim) {
    s.click(s.rect_of("present-button").center());
    assert!(s.app().proto.present.is_some(), "Present opens from the header button");
    s.frame(vec![]);
}

fn finish_anim(s: &mut Sim) {
    for _ in 0..40 {
        s.frame(vec![]);
        let p = &s.app().proto.present.as_ref().unwrap().player;
        if p.anim.is_none() {
            break;
        }
    }
}

#[test]
fn prototype_tab_lists_the_interaction_of_the_selected_layer() {
    let mut s = proto_sim(Some("Button"));
    assert_eq!(s.app().ui.right_tab, Some(RightTab::Prototype));
    let row = s.rect_of("proto-row-0");
    assert!(row.width() > 200.0 && row.height() == theme::PROTO_ROW_HEIGHT);
    assert!(s.app().proto.editing.is_none());
    s.click(row.center());
    assert_eq!(s.app().proto.editing, Some((s.by_name("Button"), 0)));
    let pop = s.rect_of("proto-popover");
    assert!(pop.right() < s.rect_of("proto-row-0").left(), "popover opens left of the panel");
    // Escape closes it
    s.key(Key::Escape);
    assert!(s.app().proto.editing.is_none());
}

#[test]
fn plus_adds_an_interaction_in_one_undo_step() {
    let mut s = proto_sim(Some("Tile A"));
    assert!(reactions(&s, "Tile A").is_empty());
    s.click(s.rect_of("proto-add").center());
    let r = reactions(&s, "Tile A");
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].trigger, Trigger::OnClick);
    assert!(matches!(r[0].actions[0], Action::Navigate { .. }));
    assert!(s.app().proto.editing.is_some(), "the new interaction opens in the editor");
    s.cmd_key(Key::Z);
    assert!(reactions(&s, "Tile A").is_empty());
}

#[test]
fn deleting_a_row_removes_only_that_interaction() {
    let mut s = proto_sim(Some("Tile A"));
    s.click(s.rect_of("proto-add").center());
    s.key(Key::Escape);
    s.click(s.rect_of("proto-add").center());
    s.key(Key::Escape);
    assert_eq!(reactions(&s, "Tile A").len(), 2);
    let row = s.rect_of("proto-row-0");
    s.mv(row.center());
    s.click(s.rect_of("proto-row-del-0").center());
    assert_eq!(reactions(&s, "Tile A").len(), 1);
}

#[test]
fn popover_edits_trigger_animation_and_undo_individually() {
    let mut s = proto_sim(Some("Button"));
    s.click(s.rect_of("proto-row-0").center());
    // animation: Smart animate -> Dissolve
    pick(&mut s, "proto-pop-animation", 1);
    let t = reactions(&s, "Button")[0].transition.unwrap();
    assert_eq!(t.kind, TransitionKind::Dissolve);
    assert_eq!(t.duration_ms, 400, "duration kept when switching animation");
    // direction-capable animation shows direction buttons
    pick(&mut s, "proto-pop-animation", 3);
    assert_eq!(reactions(&s, "Button")[0].transition.unwrap().kind, TransitionKind::MoveIn(Direction::Left));
    s.click(s.rect_of("proto-pop-dir-Bottom").center());
    assert_eq!(reactions(&s, "Button")[0].transition.unwrap().kind, TransitionKind::MoveIn(Direction::Bottom));
    // one undo per edit
    s.cmd_key(Key::Z);
    assert_eq!(reactions(&s, "Button")[0].transition.unwrap().kind, TransitionKind::MoveIn(Direction::Left));
    s.cmd_key(Key::Z);
    assert_eq!(reactions(&s, "Button")[0].transition.unwrap().kind, TransitionKind::Dissolve);
    s.cmd_key(Key::Z);
    assert_eq!(reactions(&s, "Button")[0].transition.unwrap().kind, TransitionKind::SmartAnimate);
}

#[test]
fn popover_trigger_delay_and_instant_animation() {
    let mut s = proto_sim(Some("Button"));
    s.click(s.rect_of("proto-row-0").center());
    pick(&mut s, "proto-pop-trigger", 8);
    assert_eq!(reactions(&s, "Button")[0].trigger, Trigger::AfterDelay { delay_ms: 800 });
    // edit the delay field
    let f = s.field_rect("pop-delay");
    s.click(f.center());
    s.cmd_key(Key::A);
    s.type_text("1500");
    s.key(Key::Enter);
    assert_eq!(reactions(&s, "Button")[0].trigger, Trigger::AfterDelay { delay_ms: 1500 });
    pick(&mut s, "proto-pop-animation", 0);
    assert_eq!(reactions(&s, "Button")[0].transition, None, "Instant means no transition");
}

#[test]
fn popover_action_dropdown_switches_kind_and_destination_picker_lists_frames() {
    let mut s = proto_sim(Some("Button"));
    s.click(s.rect_of("proto-row-0").center());
    // destination: pick the Card frame (second top-level frame)
    let card = s.by_name("Card");
    pick(&mut s, "proto-pop-nav-0", 1);
    let r = reactions(&s, "Button");
    assert!(matches!(r[0].actions[0], Action::Navigate { dest, .. } if dest == card), "{:?}", r[0].actions);
    // action -> Open overlay keeps the destination, shows the position grid
    pick(&mut s, "proto-pop-action-0", 2);
    let r = reactions(&s, "Button");
    assert!(matches!(r[0].actions[0], Action::OpenOverlay { dest, position: OverlayPosition::Center, close_on_click_outside: true, .. } if dest == card));
    s.click(s.rect_of("proto-pop-pos-Top right").center());
    assert!(matches!(reactions(&s, "Button")[0].actions[0], Action::OpenOverlay { position: OverlayPosition::TopRight, .. }));
    s.click(s.rect_of("proto-pop-pos-Manual").center());
    assert!(matches!(reactions(&s, "Button")[0].actions[0], Action::OpenOverlay { position: OverlayPosition::Manual, .. }));
    // Back needs no destination and no animation controls
    pick(&mut s, "proto-pop-action-0", 1);
    assert_eq!(reactions(&s, "Button")[0].actions, vec![Action::Back]);
}

#[test]
fn easing_preset_and_custom_bezier() {
    let mut s = proto_sim(Some("Button"));
    s.click(s.rect_of("proto-row-0").center());
    assert!(s.rect_of("proto-pop-curve").height() > 40.0, "the curve preview is shown");
    pick(&mut s, "proto-pop-easing", 7); // Gentle spring
    assert!(matches!(reactions(&s, "Button")[0].transition.unwrap().easing, Easing::Spring { stiffness, .. } if stiffness == 100.0));
    pick(&mut s, "proto-pop-easing", 11); // Custom bezier
    assert!(matches!(reactions(&s, "Button")[0].transition.unwrap().easing, Easing::Bezier(_)));
    let f = field_dbg(&s, ("pop-bz", 0usize));
    s.click(f.center());
    s.cmd_key(Key::A);
    s.type_text("0.7");
    s.key(Key::Enter);
    assert!(matches!(reactions(&s, "Button")[0].transition.unwrap().easing, Easing::Bezier(c) if (c[0] - 0.7).abs() < 1e-6));
}

#[test]
fn flows_list_add_rename_delete_with_undo() {
    let mut s = proto_sim(None);
    assert!(s.app().selection.is_empty());
    assert_eq!(s.app().doc.flows.len(), 1);
    s.click(s.rect_of("proto-flows-add").center());
    assert_eq!(s.app().doc.flows.len(), 2);
    assert_eq!(s.app().doc.flows[1].name, "Flow 2");
    // rename through the list
    let row = s.rect_of("proto-flow-1");
    s.click(Pos2::new(row.left() + 60.0, row.center().y));
    s.cmd_key(Key::A);
    s.type_text("Checkout");
    s.key(Key::Enter);
    assert_eq!(s.app().doc.flows[1].name, "Checkout");
    // delete
    s.click(s.rect_of("proto-flow-del-1").center());
    assert_eq!(s.app().doc.flows.len(), 1);
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.flows.len(), 2);
    assert_eq!(s.app().doc.flows[1].name, "Checkout");
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.flows[1].name, "Flow 2");
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.flows.len(), 1);
}

#[test]
fn device_dropdown_and_background_swatch() {
    let mut s = proto_sim(None);
    assert_eq!(s.app().doc.prototype_device, designos_doc::PrototypeDevice::default());
    pick(&mut s, "proto-device", 1);
    let d = s.app().doc.prototype_device.clone();
    assert_eq!(d.preset.as_deref(), Some("iPhone 15"));
    assert_eq!(d.size, Some(DV::new(393.0, 852.0)));
    pick(&mut s, "proto-device", 2);
    assert_eq!(s.app().doc.prototype_device.size, Some(DV::new(430.0, 932.0)));
    // each pick is one undo step
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.prototype_device.preset.as_deref(), Some("iPhone 15"));
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.prototype_device, designos_doc::PrototypeDevice::default());
    s.click(s.rect_of("proto-bg-swatch").center());
    assert!(s.app().props.picker.is_some(), "the background swatch opens the color picker");
}

#[test]
fn device_survives_save_and_load_and_letterboxes_the_presented_frame() {
    let mut s = proto_sim(None);
    pick(&mut s, "proto-device", 1);
    let mut path = std::env::temp_dir();
    path.push(format!("designos_device_{}", std::process::id()));
    let p = crate::io::save_path(&s.app().doc, &path).unwrap();
    let back = crate::io::load_path(&p).unwrap();
    std::fs::remove_file(p).ok();
    assert_eq!(back.prototype_device, s.app().doc.prototype_device);

    s.shell.app.doc.prototype_device.background = Some(Color::rgba(10, 20, 30, 255));
    present_open(&mut s);
    let st = s.app().proto.present.as_ref().unwrap();
    let (screen, radius) = st.device_screen.expect("a device screen is laid out");
    assert!(radius > 0.0);
    // the screen has the device's aspect, the frame fits inside it and is centered
    assert!((screen.width() / screen.height() - 393.0 / 852.0).abs() < 0.01);
    assert!(screen.contains_rect(st.stage.shrink(0.5)));
    assert!((st.stage.center() - screen.center()).length() < 1.0);
    let frame_aspect = st.player.size_of(st.player.current);
    assert!((st.stage.width() / st.stage.height() - frame_aspect.x / frame_aspect.y).abs() < 0.01);
    assert!(st.stage.width() <= screen.width() + 0.5 && st.stage.height() <= screen.height() + 0.5);
}

#[test]
fn overflow_dropdown_and_fixed_position() {
    let mut s = proto_sim(Some("Dashboard"));
    let dash = s.by_name("Dashboard");
    pick(&mut s, "proto-overflow", 2);
    assert_eq!(s.app().doc.get(dash).unwrap().prototype.overflow, designos_doc::Overflow::Vertical);
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(dash).unwrap().prototype.overflow, designos_doc::Overflow::None);
    // a layer inside can be fixed
    let hero = s.by_name("Tile A");
    s.shell.app.select_only(hero);
    s.frame(vec![]);
    s.frame(vec![]);
    pick(&mut s, "proto-position", 1);
    assert_eq!(s.app().doc.get(dash).unwrap().prototype.fixed_children, vec![hero]);
    pick(&mut s, "proto-position", 0);
    assert!(s.app().doc.get(dash).unwrap().prototype.fixed_children.is_empty());
}

#[test]
fn flow_starting_point_row_for_a_selected_frame() {
    let mut s = proto_sim(Some("Dashboard"));
    let dash = s.by_name("Dashboard");
    assert!(s.app().doc.flows.iter().all(|f| f.start != dash));
    s.click(s.rect_of("proto-flow-add").center());
    assert!(s.app().doc.flows.iter().any(|f| f.start == dash));
    s.cmd_key(Key::Z);
    assert!(s.app().doc.flows.iter().all(|f| f.start != dash));
}

// ------------------------------------------------------------------ canvas

fn hotspot_of(s: &Sim, name: &str) -> Pos2 {
    let a = s.app();
    crate::canvas::proto::hotspot_pos(a, a.canvas_rect, s.by_name(name))
}

#[test]
fn hovering_a_layer_shows_its_hotspot_and_a_hand_cursor() {
    let mut s = proto_sim(None);
    let tile = s.by_name("Tile A");
    let c = crate::util::rect_center(&crate::util::world_aabb(&s.app().doc, tile));
    s.mv(s.screen(c.x, c.y));
    assert_eq!(s.app().proto.hover, Some(tile));
    let hs = hotspot_of(&s, "Tile A");
    s.mv(hs);
    assert_eq!(s.app().proto.hot, Some(tile));
    assert_eq!(s.cursor, egui::CursorIcon::PointingHand);
    // away again
    s.mv(s.screen(900.0, 900.0));
    assert_eq!(s.app().proto.hot, None);
}

#[test]
fn dragging_a_hotspot_onto_a_frame_connects_it() {
    let mut s = proto_sim(Some("Tile A"));
    let welcome = s.by_name("Welcome");
    let from = hotspot_of(&s, "Tile A");
    let to = s.screen(200.0, 650.0);
    s.drag(from, to);
    let r = reactions(&s, "Tile A");
    assert_eq!(r.len(), 1, "one interaction created");
    assert_eq!(r[0].trigger, Trigger::OnClick);
    assert_eq!(r[0].actions, vec![Action::Navigate { dest: welcome, preserve_scroll: false }]);
    assert_eq!(r[0].transition, None);
    assert!(s.app().proto.connect.is_none());
    s.cmd_key(Key::Z);
    assert!(reactions(&s, "Tile A").is_empty(), "undone in one step");
}

#[test]
fn dropping_a_hotspot_drag_on_empty_canvas_cancels() {
    let mut s = proto_sim(Some("Tile A"));
    let from = hotspot_of(&s, "Tile A");
    s.drag(from, s.screen(700.0, 900.0));
    assert!(reactions(&s, "Tile A").is_empty());
    assert!(s.app().proto.connect.is_none());
    // dropping on its own frame is not a connection either
    let from = hotspot_of(&s, "Tile A");
    s.drag(from, s.screen(900.0, 700.0));
    assert!(reactions(&s, "Tile A").is_empty());
}

#[test]
fn escape_cancels_a_hotspot_drag() {
    let mut s = proto_sim(Some("Tile A"));
    let from = hotspot_of(&s, "Tile A");
    s.mv(from);
    s.button(true);
    s.mv(s.screen(200.0, 650.0));
    assert!(s.app().proto.connect.is_some());
    s.key(Key::Escape);
    assert!(s.app().proto.connect.is_none());
    s.button(false);
    assert!(reactions(&s, "Tile A").is_empty());
}

#[test]
fn hotspots_are_off_outside_prototype_mode() {
    let mut s = Sim::with(Options { select: Some("Tile A".into()), ..Default::default() });
    let from = hotspot_of(&s, "Tile A");
    s.mv(from);
    assert_eq!(s.app().proto.hot, None);
    assert!(!crate::canvas::proto::active(s.app()));
}

// ----------------------------------------------------------------- present

#[test]
fn present_opens_clicking_the_button_lands_on_dashboard() {
    let mut s = Sim::new();
    present_open(&mut s);
    let welcome = s.by_name("Welcome");
    let dash = s.by_name("Dashboard");
    {
        let st = s.app().proto.present.as_ref().unwrap();
        assert_eq!(st.player.current, welcome, "starts at the flow's frame");
        assert!(st.stage.width() > 100.0 && st.scale > 0.5);
        assert!((st.stage.center().x - s.app().canvas_rect.center().x).abs() < 400.0);
    }
    // find the button in the presented frame (the Welcome frame holds the first "Button")
    let btn = s.by_name("Button");
    let at = s.app().proto.present.as_ref().unwrap().screen_center_of(btn).unwrap();
    s.mv(at);
    assert_eq!(s.cursor, egui::CursorIcon::PointingHand, "hotspots get the hand cursor");
    s.click(at);
    {
        let st = s.app().proto.present.as_ref().unwrap();
        assert_eq!(st.player.current, dash);
        assert!(st.player.anim.is_some(), "smart animate runs");
        assert_eq!(st.player.anim.as_ref().unwrap().kind, TransitionKind::SmartAnimate);
    }
    finish_anim(&mut s);
    let st = s.app().proto.present.as_ref().unwrap();
    assert_eq!(st.player.current, dash);
    assert!(st.player.anim.is_none());
    assert_eq!(st.player.back.len(), 1);
}

#[test]
fn present_back_button_returns_and_escape_closes() {
    let mut s = Sim::new();
    present_open(&mut s);
    let btn = s.by_name("Button");
    let at = s.app().proto.present.as_ref().unwrap().screen_center_of(btn).unwrap();
    s.click(at);
    finish_anim(&mut s);
    // the Dashboard's own button goes back to Welcome
    let dash = s.by_name("Dashboard");
    let dash_btn = s.app().doc.get(dash).unwrap().children.iter().copied().find(|c| s.app().doc.get(*c).unwrap().name == "Button").unwrap();
    let at = s.app().proto.present.as_ref().unwrap().screen_center_of(dash_btn).unwrap();
    s.click(at);
    finish_anim(&mut s);
    assert_eq!(s.app().proto.present.as_ref().unwrap().player.current, s.by_name("Welcome"));
    s.key(Key::Escape);
    assert!(s.app().proto.present.is_none());
}

#[test]
fn present_restart_and_close_buttons() {
    let mut s = Sim::new();
    present_open(&mut s);
    let btn = s.by_name("Button");
    let at = s.app().proto.present.as_ref().unwrap().screen_center_of(btn).unwrap();
    s.click(at);
    finish_anim(&mut s);
    assert_eq!(s.app().proto.present.as_ref().unwrap().player.current, s.by_name("Dashboard"));
    s.click(s.rect_of("present-restart").center());
    let st = s.app().proto.present.as_ref().unwrap();
    assert_eq!(st.player.current, s.by_name("Welcome"));
    assert!(st.player.back.is_empty());
    s.click(s.rect_of("present-close").center());
    assert!(s.app().proto.present.is_none());
}

#[test]
fn present_starts_from_the_selected_frame_and_never_edits_the_document() {
    let mut s = Sim::with(Options { select: Some("Dashboard".into()), ..Default::default() });
    let before = s.app().doc.nodes.clone();
    present_open(&mut s);
    assert_eq!(s.app().proto.present.as_ref().unwrap().player.current, s.by_name("Dashboard"));
    s.key(Key::Escape);
    assert_eq!(s.app().doc.nodes, before);
    assert!(!s.app().hist.can_undo());
}

#[test]
fn present_fires_hover_and_after_delay_reactions() {
    let mut s = Sim::new();
    let welcome = s.by_name("Welcome");
    let dash = s.by_name("Dashboard");
    let card = s.by_name("Card");
    // hovering the avatar goes to Dashboard instantly; after a delay Dashboard goes to Card
    let avatar = s.by_name("Avatar");
    s.shell.app.doc.get_mut(avatar).unwrap().prototype.reactions.push(Reaction {
        trigger: Trigger::MouseEnter { delay_ms: 0 },
        actions: vec![Action::Navigate { dest: dash, preserve_scroll: false }],
        transition: None,
    });
    s.shell.app.doc.get_mut(dash).unwrap().prototype.reactions.push(Reaction {
        trigger: Trigger::AfterDelay { delay_ms: 300 },
        actions: vec![Action::Navigate { dest: card, preserve_scroll: false }],
        transition: Some(Transition { kind: TransitionKind::Push(Direction::Left), easing: Easing::Linear, duration_ms: 100 }),
    });
    s.shell.app.select_only(welcome);
    present_open(&mut s);
    let at = s.app().proto.present.as_ref().unwrap().screen_center_of(avatar).unwrap();
    s.mv(at);
    assert_eq!(s.app().proto.present.as_ref().unwrap().player.current, dash);
    for _ in 0..20 {
        s.frame(vec![]);
    }
    assert_eq!(s.app().proto.present.as_ref().unwrap().player.current, card);
    let _ = When::Click;
}

#[test]
fn present_wheel_scrolls_a_scrolling_frame_and_fixed_children_stay() {
    let mut s = Sim::new();
    let welcome = s.by_name("Welcome");
    {
        let d = &mut s.shell.app.doc;
        d.get_mut(welcome).unwrap().prototype.overflow = designos_doc::Overflow::Vertical;
        let hero = d.get(welcome).unwrap().children.iter().copied().find(|c| d.get(*c).unwrap().name == "Hero").unwrap();
        d.get_mut(welcome).unwrap().prototype.fixed_children.push(hero);
        // a tall block that makes the frame scroll
        let id = d.alloc_id();
        let mut tall = designos_doc::Node::rectangle(id, designos_doc::Rect::new(0.0, 700.0, 360.0, 900.0));
        tall.name = "Tall".into();
        d.insert(welcome, tall).unwrap();
    }
    s.shell.app.select_only(welcome);
    present_open(&mut s);
    let st = s.app().proto.present.as_ref().unwrap();
    assert!(st.player.scroll_range(welcome).y > 800.0);
    let hero = s.by_name("Hero");
    let before = st.screen_center_of(hero).unwrap();
    let center = st.stage.center();
    s.mv(center);
    s.frame(vec![Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, -300.0),
        phase: egui::TouchPhase::Move,
        modifiers: Modifiers::NONE,
    }]);
    s.frame(vec![]);
    let st = s.app().proto.present.as_ref().unwrap();
    assert!(st.player.scroll_of(welcome).y > 100.0, "wheel scrolled: {:?}", st.player.scroll_of(welcome));
    assert_eq!(st.screen_center_of(hero).unwrap(), before, "fixed children do not move");
}

#[test]
fn present_open_link_asks_first() {
    let mut s = Sim::new();
    let btn = s.by_name("Button");
    s.shell.app.doc.get_mut(btn).unwrap().prototype.reactions =
        vec![Reaction { trigger: Trigger::OnClick, actions: vec![Action::OpenUrl { url: "https://example.com/x".into(), new_tab: true }], transition: None }];
    present_open(&mut s);
    let at = s.app().proto.present.as_ref().unwrap().screen_center_of(btn).unwrap();
    s.click(at);
    s.frame(vec![]);
    assert_eq!(s.app().proto.present.as_ref().unwrap().pending_url.as_ref().map(|u| u.0.as_str()), Some("https://example.com/x"));
    // cancel: nothing opens, the confirmation goes away
    s.click(s.rect_of("present-url-cancel").center());
    assert!(s.app().proto.present.as_ref().unwrap().pending_url.is_none());
}

#[test]
fn toolbar_prototype_mode_switches_the_tab() {
    let mut s = Sim::new();
    assert_ne!(s.app().ui.right_tab, Some(RightTab::Prototype));
    s.shell.app.ui.right_tab = Some(RightTab::Prototype);
    s.frame(vec![]);
    assert!(crate::canvas::proto::active(s.app()));
    s.shell.app.ui.dev_mode = true;
    assert!(!crate::canvas::proto::active(s.app()));
}

#[test]
fn present_after_delay_waits_from_the_moment_presenting_starts() {
    let mut s = Sim::new();
    let dash = s.by_name("Dashboard");
    let card = s.by_name("Card");
    s.shell.app.doc.get_mut(dash).unwrap().prototype.reactions.push(Reaction {
        trigger: Trigger::AfterDelay { delay_ms: 600 },
        actions: vec![Action::Navigate { dest: card, preserve_scroll: false }],
        transition: None,
    });
    s.shell.app.select_only(dash);
    // the editor has been running for a while (clock far from zero)
    for _ in 0..30 {
        s.frame(vec![]);
    }
    present_open(&mut s);
    s.frame(vec![]);
    assert_eq!(s.app().proto.present.as_ref().unwrap().player.current, dash, "not fired immediately");
    for _ in 0..20 {
        s.frame(vec![]);
    }
    assert_eq!(s.app().proto.present.as_ref().unwrap().player.current, card);
}
