//! The tab strip of the Agents panel: the `+` button opens and closes its menu in a release build.
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
