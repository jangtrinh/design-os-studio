//! The tab strip of the Agents panel: the `+` button opens and closes its menu in a release build, and nothing in
//! the strip moves when an agent tab exits.
//!
//! A release build stores no widget rects for tests (`widgets::record` is compiled out), so the `+` tests read the
//! rects they need first, then drop every stored rect and hold [`RecordDisabled`], which makes the window run the
//! way a release build runs it.

use super::*;
use crate::terminal::AgentCliPreset;
use crate::widgets::RecordDisabled;
use std::path::PathBuf;

fn frames(s: &mut Sim, n: usize) {
    for _ in 0..n {
        s.frame(vec![]);
    }
}

/// A window with one stub agent tab in the Agents panel, so the tab strip and its `+` button are on screen.
fn strip_sim(tag: &str) -> (Sim, PathBuf) {
    let dir = std::env::temp_dir().join(format!("designos_tab_strip_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("settings dir");
    let mut s = Sim::with(Options::default());
    s.use_stub_agent_clis();
    s.shell.app.terminal.picker.settings_root_override = Some(dir.clone());
    s.cmd_key(Key::Backtick);
    s.shell.app.terminal.spawn_tab(AgentCliPreset::Claude, None).expect("stub claude starts");
    s.shell.app.terminal.viewport_focused = false;
    frames(&mut s, 3);
    (s, dir)
}

fn finish(mut s: Sim, dir: PathBuf) {
    for mut tab in s.shell.app.terminal.tabs.drain(..) {
        let _ = tab.child.kill();
    }
    let _ = std::fs::remove_dir_all(dir);
}

/// Drops every rect `widgets::record` stored so far and stops it storing more, as in a release build.
fn as_in_a_release_build(s: &mut Sim) -> RecordDisabled {
    s.ctx.data_mut(|d| d.remove_by_type::<Rect>());
    RecordDisabled::begin()
}

fn menu_is_open(s: &Sim) -> bool {
    s.app().terminal.runtime_dropdown_open
}

#[test]
fn plus_opens_its_menu_when_press_and_release_share_a_frame() {
    let (mut s, dir) = strip_sim("one-frame");
    let plus = s.rect_of("agents-runtime-add-btn").center();
    assert!(!menu_is_open(&s), "the menu starts closed");

    let _release = as_in_a_release_build(&mut s);
    s.mv(plus);
    // A slow frame: the OS hands egui the whole click between two frames.
    s.one_frame().press().release().send();
    assert!(menu_is_open(&s), "the click opens the menu, and the same frame's press must not close it again");
    frames(&mut s, 2);
    assert!(menu_is_open(&s), "and the menu stays open on the frames after it");
    finish(s, dir);
}

#[test]
fn a_second_click_on_plus_closes_its_menu() {
    let (mut s, dir) = strip_sim("second-click");
    let plus = s.rect_of("agents-runtime-add-btn").center();

    let _release = as_in_a_release_build(&mut s);
    s.click(plus);
    assert!(menu_is_open(&s), "the first click opens the menu");
    s.click(plus);
    assert!(!menu_is_open(&s), "the second click closes it: the press on `+` must not close it first and the release open it again");
    finish(s, dir);
}

#[test]
fn a_press_away_from_the_menu_and_from_plus_still_closes_it() {
    let (mut s, dir) = strip_sim("press-away");
    let plus = s.rect_of("agents-runtime-add-btn").center();
    s.click(plus);
    assert!(menu_is_open(&s));
    frames(&mut s, 2);
    let menu = s.rect_of("agents-runtime-dropdown-menu");
    let away = egui::pos2(menu.center().x, menu.bottom() + 300.0);

    let _release = as_in_a_release_build(&mut s);
    s.click(away);
    assert!(!menu_is_open(&s), "a press outside the menu closes it");
    finish(s, dir);
}

/// Position and width of what the strip holds, left to right after the tabs.
fn strip_controls(s: &Sim) -> (Rect, Rect) {
    (s.rect_of("agents-sessions-btn"), s.rect_of("agents-runtime-add-btn"))
}

/// Kills the agent of tab `idx` and runs frames until Studio has noticed the exit.
fn exit_tab(s: &mut Sim, idx: usize) {
    let _ = s.shell.app.terminal.tabs[idx].child.kill();
    for _ in 0..400 {
        s.frame(vec![]);
        if s.shell.app.terminal.tabs[idx].exited_notice_shown {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(s.shell.app.terminal.tabs[idx].exited_notice_shown, "Studio notices that the agent exited");
    frames(s, 3);
}

#[test]
fn the_buttons_stay_put_when_the_only_tab_exits() {
    let (mut s, dir) = strip_sim("exit-shift");
    let before = strip_controls(&s);

    exit_tab(&mut s, 0);

    let after = strip_controls(&s);
    assert_eq!(after.0, before.0, "the Sessions button does not move when the tab exits");
    assert_eq!(after.1, before.1, "the `+` button does not move when the tab exits: a click aimed at it must still land on it");
    finish(s, dir);
}

#[test]
fn an_exited_tab_keeps_its_title_and_still_says_it_exited() {
    let (mut s, dir) = strip_sim("exit-title");
    let title = s.shell.app.terminal.tabs[0].title.clone();
    assert!(!title.contains("exited"), "a running tab is named by its title alone: {title:?}");

    exit_tab(&mut s, 0);

    let tab = &s.shell.app.terminal.tabs[0];
    assert_eq!(tab.title, title, "the exit never rewrites the title: it is what a restored or renamed session keeps");
    assert_eq!(tab.display_title(), format!("{title} (exited)"), "the strip's tooltip and the screen-reader name still say that the agent exited");
    finish(s, dir);
}

#[test]
fn the_screen_reader_name_of_a_tab_says_when_its_agent_exited() {
    let (mut s, dir) = strip_sim("exit-a11y");
    crate::agent_a11y::enable_accesskit(&s.ctx);
    frames(&mut s, 3);
    let tab = &s.shell.app.terminal.tabs[0];
    let (title, node_id) = (tab.title.clone(), egui::Id::new(("agents-left-tab-select", tab.id)).accesskit_id());
    // AccessKit sends only the nodes that changed in a frame, so the name is read in the frame that carries it.
    let name_in_frame = |s: &Sim| -> Option<String> {
        let update = s.accesskit_update.as_ref()?;
        update.nodes.iter().find(|(id, _)| *id == node_id).and_then(|(_, node)| node.label().map(str::to_string))
    };

    let _ = s.shell.app.terminal.tabs[0].child.kill();
    let mut name = None;
    for _ in 0..400 {
        s.frame(vec![]);
        if let Some(label) = name_in_frame(&s).filter(|label| label.contains("exited")) {
            name = Some(label);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(name, Some(format!("{title} (exited)")), "a screen reader is told that the agent of the tab exited");
    finish(s, dir);
}
