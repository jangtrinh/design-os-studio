//! #923: an open dialog, menu or palette gets `Esc` (and `Enter` and the arrows) before the agent's terminal does, and `Esc`
//! in the composer never stops a running turn.

use super::*;
use crate::terminal::AgentTabStatus;

const ESC_BYTE: u8 = 0x1b;

/// One dialog, menu or palette that reads `Esc`: its name, how a test opens it, and how it tells that it is open.
type Overlay = (&'static str, fn(&mut Sim), fn(&Sim) -> bool);

fn overlays() -> Vec<Overlay> {
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
    fn open_ai_fill_menu(s: &mut Sim) {
        let hero = s.by_name("Hero");
        s.shell.app.image_jobs.open_fill_menu = Some((hero, 0, Rect::from_min_size(Pos2::new(1300.0, 300.0), Vec2::splat(20.0))));
    }
    vec![
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
        ("AI fill menu", open_ai_fill_menu, |s| s.app().image_jobs.open_fill_menu.is_some()),
    ]
}

fn terminal_bytes(s: &Sim) -> Vec<u8> {
    s.app().terminal.tabs[0].written_bytes.clone()
}

/// Opens `overlay` over a terminal that holds the keyboard; `None` when it did not open (the test setup is wrong).
fn sim_with_overlay_open(overlay: &Overlay) -> Result<Sim, String> {
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
