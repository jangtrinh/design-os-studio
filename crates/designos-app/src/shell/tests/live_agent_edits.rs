//! Agent edits from `designos-mcp` arriving while the user drags a layer: they wait for the drag
//! to end, so cancelling the drag cannot drop them and each stays its own undo step.

use super::*;
use designos_doc::live_patch::{LiveOp, LivePatch};

/// A window bound to a live agent session (the dock stays closed, so no shell is spawned).
fn live_sim() -> (Sim, designos_mcp::server::Session, std::path::PathBuf) {
    let s = Sim::new();
    let session_id = format!("{}-tab-1", s.app().tabs.instance_id);
    assert_eq!(s.app().terminal.session_id, session_id, "precondition: the window keeps its own session");
    let workspace = std::env::temp_dir().join(format!("designos-live-drag-ws-{}", crate::live_sync_tests::unique_tab_id()));
    std::fs::create_dir_all(&workspace).unwrap();
    let session = designos_mcp::server::Session::with_session_dir(&workspace, &s.app().terminal.session_dir).expect("mcp session");
    (s, session, workspace)
}

fn finish(mut s: Sim, workspace: std::path::PathBuf) {
    let dir = s.app().terminal.session_dir.clone();
    s.shell.app.terminal.shutdown();
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_dir_all(workspace);
}

/// Starts dragging Hero to the right and stops halfway, with the button still down.
fn start_hero_drag(s: &mut Sim) -> (NodeId, f32, Pos2) {
    let hero = s.by_name("Hero");
    s.shell.app.selection = vec![hero];
    s.frame(vec![]);
    let a = s.screen(300.0, 150.0);
    let t0 = s.app().doc.get(hero).unwrap().transform.tx;
    let b = s.screen(330.0, 150.0);
    s.mv(a);
    s.button(true);
    s.mv(a + (b - a) * 0.5);
    assert!(matches!(s.app().canvas.inter, canvas::Interaction::Move(_)), "precondition: dragging");
    (hero, t0, b)
}

fn name(s: &Sim, id: NodeId) -> String {
    s.app().doc.get(id).map(|n| n.name.clone()).unwrap_or_default()
}

#[test]
fn an_agent_edit_waits_for_the_drag_and_survives_escape() {
    let (mut s, mut session, workspace) = live_sim();
    let (hero, t0, b) = start_hero_drag(&mut s);
    session.call("designos_update_node", serde_json::json!({ "node_id": hero.0, "name": "Hero (agent)" })).expect("agent rename");
    s.mv(b);
    s.frame(vec![]);
    assert_eq!(name(&s, hero), "Hero", "the agent edit landed in the middle of the drag");
    s.key(Key::Escape);
    s.frame(vec![]);
    assert_eq!(name(&s, hero), "Hero (agent)", "the agent edit was lost when the drag was cancelled");
    assert_eq!(s.app().doc.get(hero).unwrap().transform.tx, t0, "Escape puts the layer back");
    finish(s, workspace);
}

#[test]
fn an_agent_edit_lands_after_the_drag_as_its_own_undo_step() {
    let (mut s, mut session, workspace) = live_sim();
    let (hero, t0, b) = start_hero_drag(&mut s);
    session.call("designos_update_node", serde_json::json!({ "node_id": hero.0, "name": "Hero (agent)" })).expect("agent rename");
    s.mv(b);
    s.button(false);
    s.frame(vec![]);
    let moved = s.app().doc.get(hero).unwrap().transform.tx;
    assert!((moved - t0 - 30.0).abs() < 8.0, "moved by {}", moved - t0);
    assert_eq!(name(&s, hero), "Hero (agent)");
    s.cmd_key(Key::Z);
    assert_eq!(name(&s, hero), "Hero", "the first undo takes back the agent edit");
    assert_eq!(s.app().doc.get(hero).unwrap().transform.tx, moved, "the user's move stays");
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(hero).unwrap().transform.tx, t0, "the second undo takes back the move");
    finish(s, workspace);
}

/// Every child link points back at its parent and every listed child exists.
fn assert_tree_consistent(s: &Sim) {
    let doc = &s.app().doc;
    for (id, n) in &doc.nodes {
        for c in &n.children {
            assert_eq!(doc.get(*c).and_then(|k| k.parent), Some(*id), "child {c:?} of {id:?}");
        }
        if let Some(p) = n.parent {
            assert!(doc.get(p).is_some_and(|p| p.children.contains(id)), "{id:?} is missing from its parent");
        }
    }
}

#[test]
fn removing_the_dragged_layer_mid_drag_does_not_panic() {
    for cancel in [false, true] {
        let (mut s, _session, workspace) = live_sim();
        let (hero, _, b) = start_hero_drag(&mut s);
        // Applied directly, as any removal that reaches the document mid-drag would be.
        crate::live_patch_apply::apply_live_patches(&mut s.shell.app, &[LivePatch::new(vec![LiveOp::Remove { id: hero }])]);
        assert!(s.app().doc.get(hero).is_none());
        s.mv(b);
        if cancel {
            s.key(Key::Escape);
        } else {
            s.button(false);
        }
        s.frame(vec![]);
        s.frame(vec![]);
        assert!(matches!(s.app().canvas.inter, canvas::Interaction::None), "the drag ended (cancel: {cancel})");
        assert_tree_consistent(&s);
        finish(s, workspace);
    }
}

/// `--panel agents`, `terminal` and `agents-chat` start their shell while the window is built,
/// before the first frame. The shell captures the session directory in its environment once, so
/// the first frame must keep the window on that directory: the agent writes there and
/// `designos-mcp` reads from there.
#[test]
fn a_shell_started_while_the_window_is_built_keeps_its_session_after_the_first_frame() {
    for panel in ["agents", "terminal", "agents-chat"] {
        let ctx = Context::default();
        let mut shell = Shell::build(&ctx, Options { panel: Some(panel.into()), ..Options::default() });
        shell.app.system_html = no_system_clipboard;
        assert!(shell.app.terminal.picker_open || !shell.app.terminal.tabs.is_empty(), "precondition: --panel {panel} opens the agent surface during build");
        let started_with = shell.app.terminal.session_env();

        let mut s = Sim { ctx, shell, t: 0.0, pos: Pos2::new(700.0, 500.0), mods: Modifiers::NONE, cursor: egui::CursorIcon::Default, shapes: Vec::new() };
        s.frame(vec![]);
        s.frame(vec![]);

        let terminal = &s.app().terminal;
        assert_eq!(
            terminal.session_id, started_with.session_id,
            "--panel {panel}: the first frame moved the window off the session its shell was started with"
        );
        assert_eq!(terminal.session_dir, started_with.session_dir);
        assert_eq!(terminal.live_spec_path, started_with.live_spec_path);
        assert_eq!(
            terminal.session_id,
            format!("{}-tab-1", s.app().tabs.instance_id),
            "--panel {panel}: the shell was started on an id the tab bar does not bind"
        );
        let dir = terminal.session_dir.clone();
        s.shell.app.terminal.shutdown();
        let _ = std::fs::remove_dir_all(dir);
    }
}
