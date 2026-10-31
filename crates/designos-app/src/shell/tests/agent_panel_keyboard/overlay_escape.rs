//! #923: an open dialog, menu or palette gets `Esc` (and `Enter` and the arrows) before the agent's terminal does, and `Esc`
//! in the composer never stops a running turn.

use super::*;
use crate::terminal::AgentTabStatus;

pub(super) const ESC_BYTE: u8 = 0x1b;

/// One dialog, menu or palette that reads `Esc`: its name, how a test opens it, and how it tells that it is open.
pub(super) type Overlay = (&'static str, fn(&mut Sim), fn(&Sim) -> bool);

pub(super) fn overlays() -> Vec<Overlay> {
    fn open_main_menu(s: &mut Sim) {
        s.click(Pos2::new(28.0, 24.0));
    }
    // The inspector menus draw after the Agents panel: a click in the inspector does not take the keyboard from the terminal.
    fn select(s: &mut Sim, name: &str) {
        let id = s.by_name(name);
        s.shell.app.selection = vec![id];
        s.frame(vec![]);
        s.frame(vec![]);
    }
    fn open_instance_menu(s: &mut Sim) {
        select(s, "Get started");
        let more = s.rect_of("instance-more");
        s.click(more.center());
    }
    fn open_add_property_menu(s: &mut Sim) {
        select(s, "Primary button");
        let plus = s.rect_of("prop-add");
        s.click(plus.center());
    }
    fn open_slot_picker(s: &mut Sim) {
        let main = s.by_name("Primary button");
        let id = s.shell.app.doc.alloc_id();
        let mut frame = designos_doc::Node::frame(id, designos_doc::Rect::new(8.0, 8.0, 24.0, 24.0));
        frame.name = "Leading slot".into();
        let slot = s.shell.app.doc.insert(main, frame).unwrap();
        let cmd = designos_doc::convert_to_slot(&mut s.shell.app.doc, slot);
        let mut tx = crate::edit::Tx::new();
        let _ = tx.cmd(&mut s.shell.app.doc, cmd);
        s.shell.app.run_tx(tx);
        s.shell.app.ui.slot_picker = Some((slot, Rect::from_min_size(Pos2::new(700.0, 400.0), Vec2::splat(20.0))));
    }
    fn open_layer_picker(s: &mut Sim) {
        let hero = s.by_name("Hero");
        s.shell.app.ui.layer_picker = Some((Pos2::new(700.0, 400.0), vec![hero]));
    }
    fn open_effects_menu(s: &mut Sim) {
        select(s, "Hero");
        s.shell.app.props.fx_menu = true;
    }
    fn open_style_menu(s: &mut Sim) {
        select(s, "Hero");
        s.shell.app.props.style_menu = Some(designos_doc::StyleProperty::Effect);
    }
    fn open_paint_styles_menu(s: &mut Sim) {
        select(s, "Hero");
        s.shell.app.props.styles_menu = Some(false);
    }
    fn open_variables_menu(s: &mut Sim) {
        s.shell.app.open_variables();
        let anchor = Rect::from_min_size(Pos2::new(690.0, 280.0), Vec2::splat(20.0));
        s.shell.app.vars.menu = Some(crate::variables::VarMenu { kind: crate::variables::VarMenuKind::Filter, pos: Pos2::new(700.0, 310.0), anchor });
    }
    // The two menus below keep their open state in egui temp data, keyed by ids the inspector builds, so a test can only
    // open them with a click.
    fn open_blend_menu(s: &mut Sim) {
        select(s, "Hero");
        let screen = s.ctx.content_rect().size();
        crate::panels::properties::preset_popup(&mut s.shell.app, "picker", screen);
        s.frame(vec![]);
        s.frame(vec![]);
        let blend = s.rect_of("picker-blend");
        s.click(blend.center());
    }
    fn open_export_scale_menu(s: &mut Sim) {
        select(s, "Hero");
        // The Export section is the last of the inspector: scroll to it, then lay it out from its title. The "+" sits at
        // SECTION_TRAILING_X; once a row exists, the 56 px scale field sits at SECTION_CONTENT_X under the title row and its
        // chevron takes the right 20 px.
        let scroll_to_the_end = |s: &mut Sim| {
            s.shell.app.props.scroll = 1.0e6;
            s.frame(vec![]);
            s.frame(vec![]);
        };
        scroll_to_the_end(s);
        let (x0, top) = export_section_origin(s);
        s.click(Pos2::new(x0 + theme::SECTION_TRAILING_X + theme::PANEL_ACTION_BUTTON / 2.0, top + 8.0 + theme::PANEL_ACTION_BUTTON / 2.0));
        scroll_to_the_end(s);
        let (x0, top) = export_section_origin(s);
        s.click(Pos2::new(x0 + theme::SECTION_CONTENT_X + 56.0 - 10.0, top + theme::SECTION_TITLE_HEIGHT + theme::FIELD_HEIGHT / 2.0));
    }
    fn open_ai_fill_menu(s: &mut Sim) {
        let hero = s.by_name("Hero");
        s.shell.app.image_jobs.open_fill_menu = Some((hero, 0, Rect::from_min_size(Pos2::new(1300.0, 300.0), Vec2::splat(20.0))));
    }
    let mut rows: Vec<Overlay> = vec![
        ("Agent permissions modal", |s| s.shell.app.ui.permissions_modal_open = true, |s| s.app().ui.permissions_modal_open),
        ("command palette", |s| s.shell.app.cmdk.open = true, |s| s.app().cmdk.open),
        ("keyboard shortcuts dialog", |s| s.shell.app.show_shortcuts(), |s| s.app().dialog.is_some()),
        ("main menu", open_main_menu, |s| s.app().ui.menu.is_some()),
        ("inspector instance menu", open_instance_menu, |s| s.app().ui.inst_menu.is_some()),
        ("inspector add-property menu", open_add_property_menu, |s| s.app().ui.prop_menu.is_some()),
        ("slot picker", open_slot_picker, |s| s.app().ui.slot_picker.is_some()),
        ("overlapping layer picker", open_layer_picker, |s| s.app().ui.layer_picker.is_some()),
        ("inspector effects menu", open_effects_menu, |s| s.app().props.fx_menu),
        ("inspector style menu", open_style_menu, |s| s.app().props.style_menu.is_some()),
        ("inspector paint styles menu", open_paint_styles_menu, |s| s.app().props.styles_menu.is_some()),
        ("variables menu", open_variables_menu, |s| s.app().vars.menu.is_some()),
        ("AI fill menu", open_ai_fill_menu, |s| s.app().image_jobs.open_fill_menu.is_some()),
        ("export scale menu", open_export_scale_menu, |s| painted_text(s).iter().any(|(t, _)| t == "0.75x")),
        ("blend mode menu", open_blend_menu, |s| painted_text(s).iter().any(|(t, _)| t == "Multiply")),
    ];
    rows.extend(super::hand_built_popovers::rows());
    rows
}

/// Every text painted in the last frame, with the centre of its first line.
pub(super) fn painted_text(s: &Sim) -> Vec<(String, Pos2)> {
    fn collect(shape: &egui::Shape, out: &mut Vec<(String, Pos2)>) {
        match shape {
            egui::Shape::Text(t) => out.push((t.galley.job.text.clone(), t.pos + Vec2::new(0.0, t.galley.size().y / 2.0))),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|x| collect(x, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    s.shapes.iter().for_each(|c| collect(&c.shape, &mut out));
    out
}

/// `(x0, top)` of the inspector's Export section, read from where its title "Export" was painted (left aligned at
/// SECTION_CONTENT_X, centred in a SECTION_TITLE_HEIGHT row).
fn export_section_origin(s: &Sim) -> (f32, f32) {
    let (_, at) =
        painted_text(s).into_iter().rfind(|(t, p)| t == "Export" && p.x > s.app().canvas_rect.right()).expect("the Export section is painted in the inspector");
    (at.x - theme::SECTION_CONTENT_X, at.y - theme::SECTION_TITLE_HEIGHT / 2.0)
}

pub(super) fn terminal_bytes(s: &Sim) -> Vec<u8> {
    s.app().terminal.tabs[0].written_bytes.clone()
}

/// Opens `overlay` over a terminal that holds the keyboard; `None` when it did not open (the test setup is wrong).
pub(super) fn sim_with_overlay_open(overlay: &Overlay) -> Result<Sim, String> {
    let (name, open, is_open) = overlay;
    let mut s = agent_sim();
    assert!(s.app().terminal.is_capturing_keyboard(true), "precondition: the terminal holds the keyboard");
    open(&mut s);
    s.frame(vec![]);
    s.frame(vec![]);
    if !is_open(&s) {
        return Err(format!("{name}: precondition failed, it is not open"));
    }
    s.shell.app.terminal.tabs[0].written_bytes.clear();
    Ok(s)
}

/// Closes the popover that hosts the open menu, if there is one: the blend menu lives in the colour picker, which stays open
/// when only the menu closes. False when the popover is still open after three Esc.
pub(super) fn close_host_popover(s: &mut Sim) -> bool {
    for _ in 0..3 {
        if s.app().props.picker.is_none() {
            return true;
        }
        s.key(Key::Escape);
    }
    s.app().props.picker.is_none()
}

#[test]
fn esc_closes_the_open_overlay_and_never_reaches_the_terminal() {
    let mut problems = Vec::new();
    for overlay in overlays() {
        let (name, _, is_open) = overlay;
        let mut s = match sim_with_overlay_open(&overlay) {
            Ok(s) => s,
            Err(why) => {
                problems.push(why);
                continue;
            }
        };
        s.key(Key::Escape);
        if is_open(&s) {
            problems.push(format!("{name}: Esc did not close it"));
        }
        if terminal_bytes(&s).contains(&ESC_BYTE) {
            problems.push(format!("{name}: Esc went to the agent's terminal as well: {:?}", terminal_bytes(&s)));
        }
        // A menu inside a popover (the blend menu in the colour picker) leaves the popover open. Close it too, and check that
        // its Esc did not reach the terminal either, before the key that must.
        s.shell.app.terminal.tabs[0].written_bytes.clear();
        if !close_host_popover(&mut s) {
            problems.push(format!("{name}: the popover that hosts the menu did not close on Esc"));
        }
        if terminal_bytes(&s).contains(&ESC_BYTE) {
            problems.push(format!("{name}: the Esc that closed the host popover went to the agent's terminal: {:?}", terminal_bytes(&s)));
        }
        s.shell.app.terminal.tabs[0].written_bytes.clear();
        // With nothing open the same key belongs to the terminal again.
        s.key(Key::Escape);
        if !terminal_bytes(&s).contains(&ESC_BYTE) {
            problems.push(format!("{name}: with nothing open Esc must reach the terminal"));
        }
        finish(s);
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn enter_never_reaches_the_terminal_while_an_overlay_is_open() {
    let mut problems = Vec::new();
    for overlay in overlays() {
        let (name, _, _) = overlay;
        let mut s = match sim_with_overlay_open(&overlay) {
            Ok(s) => s,
            Err(why) => {
                problems.push(why);
                continue;
            }
        };
        s.key(Key::Enter);
        if terminal_bytes(&s).contains(&b'\r') {
            problems.push(format!("{name}: Enter went to the agent's terminal as well: {:?}", terminal_bytes(&s)));
        }
        finish(s);
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn enter_reaches_the_terminal_again_once_the_overlay_is_closed() {
    let mut problems = Vec::new();
    for overlay in overlays() {
        let (name, _, _) = overlay;
        let mut s = match sim_with_overlay_open(&overlay) {
            Ok(s) => s,
            Err(why) => {
                problems.push(why);
                continue;
            }
        };
        s.key(Key::Escape);
        if !close_host_popover(&mut s) {
            problems.push(format!("{name}: the popover that hosts the menu did not close on Esc"));
        }
        s.shell.app.terminal.tabs[0].written_bytes.clear();
        s.key(Key::Enter);
        if !terminal_bytes(&s).contains(&b'\r') {
            problems.push(format!("{name}: with nothing open Enter must reach the terminal"));
        }
        finish(s);
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn enter_and_arrows_stay_out_of_the_terminal_while_an_overlay_is_open() {
    let mut s = agent_sim();
    s.shell.app.show_shortcuts();
    s.frame(vec![]);
    s.frame(vec![]);
    s.shell.app.terminal.tabs[0].written_bytes.clear();
    for key in [Key::ArrowUp, Key::ArrowDown, Key::Enter] {
        s.key(key);
    }
    assert!(terminal_bytes(&s).is_empty(), "keys meant for the dialog reached the terminal: {:?}", terminal_bytes(&s));
    finish(s);
}

#[test]
fn esc_closes_the_overlay_without_also_closing_the_session_menu_below_it() {
    // No session yet: the panel offers the `+` menu of agents, which closes on Esc too.
    let mut s = Sim::new();
    s.use_stub_agent_clis();
    let rail = s.rect_of("rail-Agents");
    s.click(rail.center());
    s.shell.app.terminal.runtime_dropdown_open = true;
    s.shell.app.ui.permissions_modal_open = true;
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(s.app().terminal.runtime_dropdown_open && s.app().ui.permissions_modal_open, "precondition: both are open");

    s.key(Key::Escape);
    assert!(!s.app().ui.permissions_modal_open, "Esc closes the topmost overlay");
    assert!(s.app().terminal.runtime_dropdown_open, "the same Esc must not also close the menu below it");

    s.key(Key::Escape);
    assert!(!s.app().terminal.runtime_dropdown_open, "the next Esc closes the menu");
}

#[test]
fn esc_on_runtime_menu_does_not_reach_terminal() {
    let mut s = super::agent_sim();
    s.shell.app.terminal.runtime_dropdown_open = true;
    s.shell.app.terminal.tabs[0].written_bytes.clear();
    s.key(Key::Escape);
    assert!(!s.app().terminal.runtime_dropdown_open, "Esc closes the + menu");
    assert!(terminal_bytes(&s).is_empty(), "Esc closing the + menu must not reach the terminal");
}

#[test]
fn esc_on_other_cli_form_does_not_reach_terminal() {
    let mut s = super::agent_sim();
    s.shell.app.terminal.runtime_dropdown_open = true;
    s.shell.app.terminal.picker.open_other_form();
    s.shell.app.terminal.tabs[0].written_bytes.clear();
    s.key(Key::Escape);
    assert!(!s.app().terminal.picker.other_form_open, "Esc closes the Other CLI form");
    assert!(terminal_bytes(&s).is_empty(), "Esc must not reach the terminal");
}

/// The menus and popovers whose host section draws only while something is selected or the Variables view is open: the host
/// can stop drawing (an agent edit removes the node, an undo, the view closes) without a press, and the flag then has nothing
/// to clear it.
const HOST_DEPENDENT: &[&str] = &[
    "inspector instance menu",
    "inspector add-property menu",
    "inspector effects menu",
    "inspector style menu",
    "inspector paint styles menu",
    "variables menu",
    "Variables edit popover",
    "Variables cell colour picker",
    "Variables skipped-details popover",
];

#[test]
fn a_menu_whose_host_stopped_drawing_does_not_leave_the_terminal_deaf() {
    let mut problems = Vec::new();
    for overlay in overlays().into_iter().filter(|(name, _, _)| HOST_DEPENDENT.contains(name)) {
        let (name, _, is_open) = overlay;
        let mut s = match sim_with_overlay_open(&overlay) {
            Ok(s) => s,
            Err(why) => {
                problems.push(why);
                continue;
            }
        };
        // What an agent edit that deletes the selected node does: the inspector section that draws the menu is gone.
        s.shell.app.selection.clear();
        s.shell.app.vars.open = false;
        s.frame(vec![]);
        s.frame(vec![]);
        if !is_open(&s) {
            problems.push(format!("{name}: precondition failed, the flag was cleared by something else"));
            continue;
        }
        s.shell.app.terminal.tabs[0].written_bytes.clear();
        s.type_text("y");
        if !terminal_bytes(&s).contains(&b'y') {
            problems.push(format!("{name}: the menu is not on screen but the terminal ignores typing: {:?}", terminal_bytes(&s)));
        }
        finish(s);
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

/// Opens the popup `row`, then removes the inspector section that draws it: the terminal must hear typing again.
fn assert_typing_returns_once_the_section_stops_drawing(row: &str) {
    let overlay = overlays().into_iter().find(|(name, _, _)| *name == row).expect("the row");
    let mut s = sim_with_overlay_open(&overlay).expect("the popup opens");
    s.shell.app.selection.clear();
    s.frame(vec![]);
    s.frame(vec![]);
    s.shell.app.terminal.tabs[0].written_bytes.clear();
    s.type_text("y");
    assert!(terminal_bytes(&s).contains(&b'y'), "{row}: it is gone but the terminal ignores typing: {:?}", terminal_bytes(&s));
    finish(s);
}

#[test]
fn an_export_scale_menu_whose_section_disappeared_does_not_leave_the_terminal_deaf() {
    // Its open flag lives in egui temp data and cannot be cleared from outside: only the pass counter keeps the terminal
    // from waiting for a menu nothing draws any more.
    assert_typing_returns_once_the_section_stops_drawing("export scale menu");
}

#[test]
fn a_font_dropdown_whose_section_disappeared_does_not_leave_the_terminal_deaf() {
    // Same as the export scale menu: its open flag lives in egui temp data under an id the inspector builds.
    assert_typing_returns_once_the_section_stops_drawing("font dropdown");
}

/// The web capture import report: a floating sheet at the right of the canvas with no backdrop, closed by its own button or Esc.
fn open_web_capture_report(s: &mut Sim) {
    s.shell.app.web_capture_report = Some(designos_webcapture::ImportReport::default());
    s.shell.app.web_capture_report_open = true;
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(s.app().web_capture_report_open, "precondition: the report sheet is open");
    s.shell.app.terminal.tabs[0].written_bytes.clear();
}

#[test]
fn the_floating_web_capture_report_does_not_take_the_keyboard_from_the_terminal() {
    let mut s = agent_sim();
    open_web_capture_report(&mut s);
    s.type_text("y");
    s.key(Key::Enter);
    let bytes = terminal_bytes(&s);
    assert!(bytes.contains(&b'y') && bytes.contains(&b'\r'), "answering a prompt in the terminal must work while the sheet is open: {bytes:?}");
    assert!(s.app().web_capture_report_open, "typing in the terminal leaves the sheet open");
    finish(s);
}

#[test]
fn esc_belongs_to_the_terminal_while_it_holds_the_keyboard_and_to_the_report_sheet_otherwise() {
    let mut s = agent_sim();
    open_web_capture_report(&mut s);
    s.key(Key::Escape);
    assert!(terminal_bytes(&s).contains(&ESC_BYTE), "the terminal holds the keyboard, so Esc is its key");
    assert!(s.app().web_capture_report_open, "and that Esc must not also close the sheet");

    // A click on the canvas takes the keyboard from the terminal: now Esc closes the sheet and goes no further.
    let canvas = s.app().canvas_rect;
    s.click(canvas.left_bottom() + Vec2::new(120.0, -120.0));
    assert!(!s.app().terminal.is_capturing_keyboard(true), "precondition: the terminal no longer holds the keyboard");
    s.shell.app.terminal.tabs[0].written_bytes.clear();
    s.key(Key::Escape);
    assert!(!s.app().web_capture_report_open, "Esc closes the sheet when the terminal does not hold the keyboard");
    assert!(terminal_bytes(&s).is_empty(), "and nothing reaches the agent: {:?}", terminal_bytes(&s));
    finish(s);
}

fn mark_turn_running(s: &mut Sim) {
    *s.shell.app.terminal.tabs[0].agent_status.lock().expect("status lock") = AgentTabStatus::Working(Some("Thinking".into()));
}

#[test]
fn esc_for_a_dialog_never_stops_the_turn_after_a_click_in_the_panel_header() {
    let mut s = agent_sim();
    // A press in the header strip is panel chrome: there Esc is the documented Stop.
    s.shell.app.terminal.viewport_focused = false;
    s.shell.app.terminal.chrome_focused = true;
    s.shell.app.ui.permissions_modal_open = true;
    s.frame(vec![]);
    mark_turn_running(&mut s);

    s.key(Key::Escape);
    assert!(!s.app().ui.permissions_modal_open, "Esc closes the dialog");
    assert!(s.app().terminal.tabs[0].stop_state.is_none(), "the Esc that closed the dialog must not also stop the turn");
    assert!(s.app().terminal.tabs[0].stop_sent_keys.is_empty(), "no interrupt key went to the agent");

    mark_turn_running(&mut s);
    s.key(Key::Escape);
    assert!(s.app().terminal.tabs[0].stop_state.is_some(), "with no dialog open Esc in the chrome still stops the turn (#684)");
    finish(s);
}

#[test]
fn esc_in_the_composer_never_stops_a_running_turn() {
    let mut s = agent_sim();
    // The user clicked the header earlier, then moved the caret into the composer with the keyboard shortcut.
    s.shell.app.terminal.chrome_focused = true;
    s.ctx.memory_mut(|m| m.request_focus(crate::agent_a11y::composer_id()));
    s.frame(vec![]);
    s.frame(vec![]);
    assert_eq!(s.ctx.memory(|m| m.focused()), Some(crate::agent_a11y::composer_id()), "precondition: the composer has the caret");
    mark_turn_running(&mut s);

    s.key(Key::Escape);
    assert!(s.app().terminal.tabs[0].stop_state.is_none(), "Esc in the composer only leaves the field; it must not stop the turn");
    assert!(s.app().terminal.tabs[0].stop_sent_keys.is_empty(), "no interrupt key went to the agent");
    finish(s);
}
