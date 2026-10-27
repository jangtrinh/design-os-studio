//! The command palette's auto-fix and Figma AI actions run through the editor's transaction, so one Cmd+Z puts
//! the document back exactly and the toast that promises it tells the truth.
//!
//! The actions share a process-wide queue for the AI operation they run, so these tests hold
//! [`crate::cmdk::ai_op_test_lock`] while they drive one: two of them in parallel could take each other's queued
//! operation.

use super::unsaved_dot::{dot, lib, library_sim, wait, SETTLED};
use super::*;
use crate::cmdk::CmdKAction;
use designos_doc::{AutoLayout, Document, Flow, LayoutAlign, LayoutChild, LayoutDirection, Node, NodeKind, Rect as DRect, Sizing, TextStyle};
use std::collections::BTreeMap;
use std::sync::MutexGuard;

fn serialized() -> MutexGuard<'static, ()> {
    crate::cmdk::ai_op_test_lock()
}

/// Everything an undo step has to put back.
#[derive(Clone, PartialEq, Debug)]
struct State {
    nodes: BTreeMap<NodeId, Node>,
    flows: Vec<Flow>,
}

fn state(s: &Sim) -> State {
    State { nodes: s.app().doc.nodes.clone(), flows: s.app().doc.flows.clone() }
}

/// Fails with the names of the layers (and the flows) that differ, not with two dumped documents.
#[track_caller]
fn assert_same_state(actual: &State, expected: &State, what: &str) {
    if actual == expected {
        return;
    }
    let mut differing: Vec<String> = Vec::new();
    for (id, node) in &expected.nodes {
        match actual.nodes.get(id) {
            Some(now) if now == node => {}
            Some(now) => differing.push(format!("{} changed: was {:?} {:?}, now {:?} {:?}", node.name, node.transform, node.size, now.transform, now.size)),
            None => differing.push(format!("{} is gone", node.name)),
        }
    }
    for (id, node) in &actual.nodes {
        if !expected.nodes.contains_key(id) {
            differing.push(format!("{} is new", node.name));
        }
    }
    if actual.flows != expected.flows {
        differing.push(format!("flows: expected {:?}, got {:?}", expected.flows, actual.flows));
    }
    panic!("{what}: {}", differing.join("; "));
}

/// Ids of the layers `fixable_doc` builds.
struct Layers {
    first_frame: NodeId,
    button: NodeId,
    label: NodeId,
    toolbar: NodeId,
    toolbar_button: NodeId,
    toolbar_label: NodeId,
    tail: NodeId,
    placeholder: NodeId,
}

/// A page with three top-level frames. The first holds a 24px "Primary Button" and 9px copy, the second is
/// a toolbar on a 7px rhythm whose button, label and tail all move when the rhythm is snapped, the third is
/// a plain frame. Frame 12 and the text "Text 4" have generic names and sit on fractions of a pixel.
fn add_fixable_layers(d: &mut Document, page: NodeId) -> Layers {
    let first_frame = d.alloc_id();
    let mut frame = Node::frame(first_frame, DRect::new(0.0, 0.4, 320.0, 240.0));
    frame.name = "Frame 12".into();
    d.insert(page, frame).unwrap();
    let button = d.alloc_id();
    let mut btn = Node::frame(button, DRect::new(16.0, 180.0, 140.0, 24.0));
    btn.name = "Primary Button".into();
    d.insert(first_frame, btn).unwrap();
    let label = d.alloc_id();
    let style = TextStyle { font_size: 9.0, ..TextStyle::default() };
    d.insert(first_frame, Node::text(label, DRect::new(16.0, 20.0, 120.0, 12.0), "Welcome back", style)).unwrap();
    let title = d.alloc_id();
    d.insert(first_frame, Node::text(title, DRect::new(16.0, 60.0, 120.0, 20.0), "Title", TextStyle::default())).unwrap();

    let toolbar = d.alloc_id();
    let mut tb = Node::frame(toolbar, DRect::new(380.0, 0.0, 400.0, 60.0));
    tb.name = "Toolbar".into();
    if let Some(fp) = tb.kind.frame_props_mut() {
        fp.auto_layout = Some(AutoLayout { item_spacing: 7.0, padding: [5.0, 12.0, 5.0, 12.0], ..Default::default() });
    }
    d.insert(page, tb).unwrap();
    let tb_button = d.alloc_id();
    let mut b = Node::frame(tb_button, DRect::new(0.0, 0.0, 120.0, 24.0));
    b.name = "Save Button".into();
    d.insert(toolbar, b).unwrap();
    let tb_label = d.alloc_id();
    d.insert(toolbar, Node::text(tb_label, DRect::new(0.0, 0.0, 60.0, 12.0), "Click", TextStyle { font_size: 9.0, ..TextStyle::default() })).unwrap();
    let tail = d.alloc_id();
    let mut t = Node::frame(tail, DRect::new(0.0, 0.0, 40.0, 20.0));
    t.name = "Tail".into();
    d.insert(toolbar, t).unwrap();
    // an auto-layout frame in a saved file already holds its children where the layout puts them
    d.relayout_subtree(toolbar);

    let third = d.alloc_id();
    let mut f3 = Node::frame(third, DRect::new(820.0, 0.0, 320.0, 240.0));
    f3.name = "Onboarding Step 3".into();
    d.insert(page, f3).unwrap();

    let placeholder = d.alloc_id();
    let mut ph = Node::text(placeholder, DRect::new(0.0, 300.4, 120.0, 20.0), "$0", TextStyle::default());
    ph.name = "Text 4".into();
    d.insert(page, ph).unwrap();
    Layers { first_frame, button, label, toolbar, toolbar_button: tb_button, toolbar_label: tb_label, tail, placeholder }
}

fn fixable_doc() -> (Document, Layers) {
    let mut d = Document::new("Palette fix");
    let page = d.add_page("P");
    let layers = add_fixable_layers(&mut d, page);
    (d, layers)
}

/// `fixable_doc` as a file written by another tool leaves it: the toolbar's children sit where that tool put them,
/// not where this solver does, so laying the toolbar out again moves them.
fn imported_doc() -> (Document, Layers) {
    let (mut d, layers) = fixable_doc();
    for (id, x, y) in [(layers.toolbar_button, 3.0, 17.0), (layers.toolbar_label, 157.0, 29.0), (layers.tail, 311.0, 5.0)] {
        let node = d.get_mut(id).expect("a toolbar child");
        node.transform.tx = x;
        node.transform.ty = y;
    }
    (d, layers)
}

fn sim_on_fixable_doc() -> (Sim, Layers) {
    let (doc, layers) = fixable_doc();
    sim_on(doc, layers)
}

fn sim_on(doc: Document, layers: Layers) -> (Sim, Layers) {
    (open_in_sim(doc), layers)
}

fn open_in_sim(doc: Document) -> Sim {
    let mut s = Sim::new();
    s.use_stub_agent_clis();
    s.shell.app.terminal.platform_is_mac_override = Some(true);
    s.shell.app.replace_document(doc, None);
    s.frame(vec![]);
    s.frame(vec![]);
    s.shell.app.selection.clear();
    assert!(!s.app().hist.can_undo(), "a freshly opened document has no history");
    s
}

fn auto_fix(s: &mut Sim) {
    crate::shell::execute_cmdk_action(&mut s.shell.app, CmdKAction::AutoFixDesignGate);
    s.frame(vec![]);
}

fn toast_text(s: &Sim) -> Option<String> {
    s.app().toast.as_ref().map(|t| t.text.clone())
}

#[test]
fn the_design_gate_fix_is_one_undo_step_and_undo_puts_everything_back() {
    let _guard = serialized();
    let (mut s, l) = sim_on_fixable_doc();
    let before = state(&s);

    auto_fix(&mut s);

    let after = state(&s);
    assert_ne!(after, before, "the fix changed the document");
    assert_eq!(s.app().doc.get(l.label).unwrap().kind.any_text().unwrap().style.font_size, 11.0, "9px copy is raised to 11px");
    assert_eq!(s.app().doc.get(l.button).unwrap().size.y, 32.0, "the 24px button grows to 32px");
    assert_ne!(s.app().doc.get(l.tail).unwrap().transform, before.nodes[&l.tail].transform, "snapping the rhythm moved a sibling");
    assert!(s.app().hist.can_undo(), "the fix is on the undo stack");

    s.cmd_key(Key::Z);
    assert_same_state(&state(&s), &before, "one Cmd+Z restores the document exactly");
    assert!(!s.app().hist.can_undo(), "and it was the only step");

    s.cmd_key(Key::Y);
    assert_same_state(&state(&s), &after, "redo applies the fix again");
}

#[test]
fn the_palette_button_fix_closes_the_palette_so_cmd_z_undoes_it() {
    let _guard = serialized();
    let (mut s, _l) = sim_on_fixable_doc();
    let before = state(&s);

    s.shell.app.cmdk.open = true;
    s.shell.app.cmdk.active_tab = 3;
    s.frame(vec![]);
    s.frame(vec![]);
    let button = s.rect_of("cmdk-autofix-gate-btn");
    s.click(button.center());
    assert_ne!(state(&s), before, "the button fixed the document");
    assert!(!s.app().cmdk.open, "the palette's search field would keep the keyboard, so the fix closes it");

    s.cmd_key(Key::Z);
    assert_same_state(&state(&s), &before, "Cmd+Z undoes the fix, as the toast says");
}

#[test]
fn windows_ctrl_z_and_ctrl_y_undo_and_redo_palette_autofix() {
    let _guard = serialized();
    let (mut s, _l) = sim_on_fixable_doc();
    s.shell.app.terminal.platform_is_mac_override = Some(false);
    let before = state(&s);

    auto_fix(&mut s);
    let after = state(&s);
    assert_ne!(after, before, "the fix changed the document");

    // First verify real Windows Ctrl+Z / Ctrl+Y (`ctrl: true, command: true`) when no terminal tab is open.
    let win_ctrl = Modifiers { alt: false, ctrl: true, shift: false, mac_cmd: false, command: true };
    s.mods = win_ctrl;
    s.key(Key::Z);
    s.mods = Modifiers::NONE;
    assert_same_state(&state(&s), &before, "Windows Ctrl+Z restores the document when the Agents panel has no focused tab");

    s.mods = win_ctrl;
    s.key(Key::Y);
    s.mods = Modifiers::NONE;
    assert_same_state(&state(&s), &after, "Windows Ctrl+Y replays the fix");

    // And verify the `Sim::cmd_key` harness helper maps to the platform command modifier on non-macOS.
    s.cmd_key(Key::Z);
    assert_same_state(&state(&s), &before, "Sim::cmd_key(Key::Z) undoes the fix on Windows");
    s.cmd_key(Key::Y);
    assert_same_state(&state(&s), &after, "Sim::cmd_key(Key::Y) redoes the fix on Windows");
}

#[test]
fn a_fix_that_changes_nothing_adds_no_undo_step_and_no_applied_toast() {
    let _guard = serialized();
    let (mut s, _l) = sim_on_fixable_doc();
    auto_fix(&mut s);
    s.cmd_key(Key::Z);
    // run it twice from the fixed state: the second run has nothing left to change
    s.cmd_key(Key::Y);
    let fixed = state(&s);
    let undo_label = s.app().hist.peek_undo_label().map(str::to_string);
    s.shell.app.toast = None;

    auto_fix(&mut s);

    assert_same_state(&state(&s), &fixed, "nothing to fix, nothing written");
    assert_eq!(s.app().hist.peek_undo_label().map(str::to_string), undo_label, "no second undo step");
    s.cmd_key(Key::Z);
    assert!(!s.app().hist.can_undo(), "one Cmd+Z undid the one real fix: the empty run added no step");
    let toast = toast_text(&s).unwrap_or_default();
    assert!(!toast.contains("Applied") && !toast.contains("applied"), "no applied toast for a no-op, got {toast:?}");
}

#[test]
fn the_toast_follows_the_ux_writing_rules_and_promises_only_what_works() {
    let _guard = serialized();
    let (mut s, _l) = sim_on_fixable_doc();
    auto_fix(&mut s);
    let toast = toast_text(&s).expect("the fix shows a toast");
    assert_eq!(toast, "Spacing and tap targets fixed · Cmd+Z to undo");
    assert!(!toast.contains("(s)") && !toast.contains('('), "no (s) and no parentheses: {toast:?}");
    let words = toast.split_whitespace().filter(|w| w.chars().any(char::is_alphanumeric)).count();
    assert!(words <= 8, "microcopy of about eight words, the · separator is not one: {toast:?}");
    s.cmd_key(Key::Z);
    assert!(!s.app().hist.can_undo(), "the promised undo works");
}

#[test]
fn every_operation_names_what_it_did_in_its_toast() {
    let _guard = serialized();
    let (mut s, _l) = sim_on_fixable_doc();
    for (row, query, expected) in AI_ROWS {
        s.shell.app.toast = None;
        s.shell.app.cmdk.open = true;
        s.shell.app.cmdk.query = query.into();
        s.frame(vec![]);
        s.frame(vec![]);
        let rect = s.rect_of(row);
        s.click(rect.center());
        assert_eq!(toast_text(&s).as_deref(), Some(expected), "{row} says what it did");
        assert!(s.app().hist.can_undo(), "{row} changed the document, so the promised undo exists");
        assert!(!s.app().cmdk.open, "{row} committed a step: the palette closes so Cmd+Z reaches the canvas");
        s.cmd_key(Key::Z);
        s.shell.app.cmdk.open = false;
        s.shell.app.cmdk.query.clear();
    }
    let all: Vec<&str> = AI_ROWS.iter().map(|r| r.2).collect();
    let mut distinct = all.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct.len(), all.len(), "no two operations share a toast: {all:?}");
}

#[test]
fn a_no_op_says_nothing_changed_while_an_older_error_toast_is_still_on_screen() {
    let _guard = serialized();
    let (mut s, _l) = sim_on_fixable_doc();
    auto_fix(&mut s);
    let undo_label = s.app().hist.peek_undo_label().map(str::to_string);
    s.shell.app.show_toast("Something else failed", true);

    auto_fix(&mut s);

    let toast = s.app().toast.as_ref().expect("the no-op shows a toast");
    assert_eq!((toast.text.as_str(), toast.error), ("Nothing changed", false), "the notice replaces the older error");
    assert_eq!(s.app().hist.peek_undo_label().map(str::to_string), undo_label, "and no step was added");
}

/// Puts the fixable layers into the library file, saves that as a version and shows the version read-only.
fn view_an_old_version_with_fixable_layers(s: &mut Sim) {
    let id = s.app().lib_file.as_ref().unwrap().id.clone();
    {
        let app = &mut s.shell.app;
        let page = app.page;
        add_fixable_layers(&mut app.doc, page);
        app.mark_edited_outside_history();
        app.changed();
    }
    wait(s, 2.5);
    assert!(s.shell.app.save_named_version("before", ""));
    let n = lib(s).list_versions(&id).last().expect("the saved version").n;
    s.shell.app.view_version(n);
    wait(s, SETTLED);
    assert!(s.app().history.viewing.is_some(), "an old version is shown read-only");
}

#[test]
fn a_fix_in_an_old_version_view_keeps_its_read_only_toast_and_changes_nothing() {
    let _guard = serialized();
    let (mut s, _dir) = library_sim();
    view_an_old_version_with_fixable_layers(&mut s);
    let before = state(&s);
    s.shell.app.selection.clear();
    s.shell.app.show_toast("Saved", false);

    auto_fix(&mut s);

    let toast = s.app().toast.as_ref().expect("the refusal shows a toast");
    assert_eq!((toast.text.as_str(), toast.error), ("Viewing an old version is read-only", true), "the refusal is not turned into a no-op notice");
    assert_same_state(&state(&s), &before, "a refused fix writes nothing, flows included");
    assert!(!s.app().hist.can_undo(), "and leaves no undo step");
}

#[test]
fn the_fix_lights_the_unsaved_dot_and_autosave_writes_it() {
    let _guard = serialized();
    let (mut s, _dir) = library_sim();
    let id = s.app().lib_file.as_ref().unwrap().id.clone();
    // the fixable layers join the open library file as an ordinary edit, then autosave writes them
    let label;
    {
        let app = &mut s.shell.app;
        let page = app.page;
        label = add_fixable_layers(&mut app.doc, page).label;
        app.mark_edited_outside_history();
        app.changed();
    }
    wait(&mut s, 2.5);
    assert!(!dot(&s), "the setup edit is saved");
    let saves = s.app().autosave.saves;
    s.shell.app.selection.clear();

    auto_fix(&mut s);
    wait(&mut s, SETTLED);
    assert!(dot(&s), "the fix is an edit no save has written");

    wait(&mut s, 2.5);
    assert_eq!(s.app().autosave.saves, saves + 1, "autosave saw the fix as an edit and wrote it");
    assert!(!dot(&s), "saved: no dot");
    let on_disk = lib(&s).load_file(&id).unwrap();
    let label_on_disk = on_disk.nodes.get(&label).expect("the label is in the file");
    assert_eq!(label_on_disk.kind.any_text().unwrap().style.font_size, 11.0, "the file holds the fixed copy");
}

/// What the Figma AI rows run: each shares the action and the queue, so each must be one exact undo step too.
const AI_ROWS: [(&str, &str, &str); 5] = [
    ("cmdk-wire-prototype", "prototype", "Prototype flows wired · Cmd+Z to undo"),
    ("cmdk-rename-layers", "boost", "Resolution boosted · Cmd+Z to undo"),
    ("cmdk-semantic-rename", "semantic", "Layers renamed · Cmd+Z to undo"),
    ("cmdk-translate-vi", "translate", "Text rewritten · Cmd+Z to undo"),
    ("cmdk-populate-content", "replace", "Content replaced · Cmd+Z to undo"),
];

/// Opens the palette on `query` and clicks `row`, as a user does.
fn click_row(s: &mut Sim, row: &str, query: &str) {
    s.shell.app.cmdk.open = true;
    s.shell.app.cmdk.query = query.into();
    s.frame(vec![]);
    s.frame(vec![]);
    let rect = s.rect_of(row);
    s.click(rect.center());
}

/// A page with one frame that none of the operations has anything to change: whole pixels, a plain name, no copy,
/// no fills, no layout to solve, and no second frame to link it to.
fn quiet_doc() -> Document {
    let mut d = Document::new("Quiet");
    let page = d.add_page("P");
    let id = d.alloc_id();
    let mut frame = Node::frame(id, DRect::new(0.0, 0.0, 320.0, 240.0));
    frame.name = "Card".into();
    d.insert(page, frame).unwrap();
    d
}

#[test]
fn a_figma_ai_row_that_changes_nothing_leaves_the_palette_open_so_another_action_can_be_picked() {
    let _guard = serialized();
    let mut s = open_in_sim(quiet_doc());
    for (row, query, _) in AI_ROWS {
        let before = state(&s);
        let depth = s.app().hist.undo_depth();
        s.shell.app.toast = None;

        click_row(&mut s, row, query);

        assert_eq!(toast_text(&s).as_deref(), Some("Nothing changed"), "{row}: the page has nothing for it to change");
        assert_same_state(&state(&s), &before, &format!("{row}: nothing written"));
        assert_eq!(s.app().hist.undo_depth(), depth, "{row}: no undo step");
        assert!(s.app().cmdk.open, "{row}: with no step there is nothing for Cmd+Z to reach, so the palette stays open");
        s.shell.app.cmdk.open = false;
        s.shell.app.cmdk.query.clear();
    }
}

#[test]
fn the_palette_button_that_changes_nothing_leaves_the_palette_open() {
    let _guard = serialized();
    let (mut s, _l) = sim_on_fixable_doc();
    auto_fix(&mut s);
    let fixed = state(&s);
    let depth = s.app().hist.undo_depth();
    s.shell.app.toast = None;

    s.shell.app.cmdk.open = true;
    s.shell.app.cmdk.active_tab = 3;
    s.frame(vec![]);
    s.frame(vec![]);
    let button = s.rect_of("cmdk-autofix-gate-btn");
    s.click(button.center());

    assert_eq!(toast_text(&s).as_deref(), Some("Nothing changed"), "the second fix has nothing left to change");
    assert_same_state(&state(&s), &fixed, "nothing written");
    assert_eq!(s.app().hist.undo_depth(), depth, "no undo step");
    assert!(s.app().cmdk.open, "no step, so the palette stays open");
}

#[test]
fn a_figma_ai_row_in_an_old_version_view_is_refused_and_leaves_the_palette_open() {
    let _guard = serialized();
    let (mut s, _dir) = library_sim();
    view_an_old_version_with_fixable_layers(&mut s);
    s.shell.app.selection.clear();
    let before = state(&s);
    s.shell.app.toast = None;

    click_row(&mut s, "cmdk-semantic-rename", "semantic");

    let toast = s.app().toast.as_ref().expect("the refusal shows a toast");
    assert_eq!((toast.text.as_str(), toast.error), ("Viewing an old version is read-only", true), "the edit is refused, and says why");
    assert_same_state(&state(&s), &before, "a refused edit writes nothing");
    assert!(!s.app().hist.can_undo(), "and leaves no undo step");
    assert!(s.app().cmdk.open, "refused, so there is no step to undo and the palette stays open");
}

#[test]
fn every_figma_ai_row_is_one_exact_undo_step_with_and_without_a_selection() {
    let _guard = serialized();
    let (mut s, l) = sim_on_fixable_doc();
    for selection in [vec![], vec![l.placeholder], vec![l.first_frame]] {
        for (row, query, _) in AI_ROWS {
            s.shell.app.selection = selection.clone();
            let before = state(&s);
            s.shell.app.cmdk.open = true;
            s.shell.app.cmdk.query = query.into();
            s.frame(vec![]);
            s.frame(vec![]);
            let rect = s.rect_of(row);
            s.click(rect.center());
            let after = state(&s);
            assert_ne!(after, before, "{row} with selection {selection:?} changed the document");
            assert!(s.app().hist.can_undo(), "{row} left an undo step");

            s.cmd_key(Key::Z);
            assert_same_state(&state(&s), &before, &format!("{row} with selection {selection:?}: one Cmd+Z restores the document"));
            assert!(!s.app().hist.can_undo(), "{row}: and it was the only step");
            s.cmd_key(Key::Y);
            assert_same_state(&state(&s), &after, &format!("{row}: redo applies it again"));
            s.cmd_key(Key::Z);
            s.shell.app.cmdk.open = false;
            s.shell.app.cmdk.query.clear();
        }
    }
}

#[test]
fn wiring_two_selected_frames_links_only_those_and_one_cmd_z_restores_the_page() {
    let _guard = serialized();
    let (mut s, l) = sim_on_fixable_doc();
    let page = s.app().page;
    let third_frame = s
        .app()
        .doc
        .get(page)
        .unwrap()
        .children
        .iter()
        .copied()
        .find(|id| s.app().doc.get(*id).unwrap().name == "Onboarding Step 3")
        .expect("a frame outside the selection");
    s.shell.app.selection = vec![l.first_frame, l.toolbar];
    let before = state(&s);

    click_row(&mut s, "cmdk-wire-prototype", "prototype");

    let after = state(&s);
    assert_eq!(toast_text(&s).as_deref(), Some("Prototype flows wired · Cmd+Z to undo"));
    assert_eq!(after.flows.len(), 1, "one flow starts the wiring");
    assert_eq!(after.flows[0].start, l.first_frame, "it starts on the first selected frame");
    assert_eq!(after.nodes[&third_frame], before.nodes[&third_frame], "the frame outside the selection is not wired");
    assert_eq!(after.nodes[&l.placeholder], before.nodes[&l.placeholder], "nor is the copy on the page");
    let wired = |id: NodeId| !s.app().doc.get(id).unwrap().prototype.reactions.is_empty();
    assert!(wired(l.button) || wired(l.first_frame), "the first frame is wired through its button or itself");
    assert!(wired(l.toolbar_button) || wired(l.toolbar), "the toolbar is wired through its button or itself");

    s.cmd_key(Key::Z);
    assert_same_state(&state(&s), &before, "one Cmd+Z restores the nodes and the flows");
    assert!(!s.app().hist.can_undo(), "and it was the only step");
    s.cmd_key(Key::Y);
    assert_same_state(&state(&s), &after, "redo wires them again");
}

/// Where the toolbar's children are, in the order `Layers` names them.
fn toolbar_children_at(doc: &Document, l: &Layers) -> Vec<(f32, f32)> {
    [l.toolbar_button, l.toolbar_label, l.tail]
        .iter()
        .map(|id| {
            let n = doc.get(*id).expect("a toolbar child");
            (n.transform.tx, n.transform.ty)
        })
        .collect()
}

#[test]
fn on_a_page_the_solver_would_move_one_cmd_z_restores_it_exactly_and_redo_replays_the_fix() {
    let _guard = serialized();
    let (doc, l) = imported_doc();
    let hand_placed = toolbar_children_at(&doc, &l);
    let mut solved = doc.clone();
    solved.relayout_subtree(l.toolbar);
    assert_ne!(toolbar_children_at(&solved, &l), hand_placed, "precondition: laying the toolbar out again moves a child, as it does on an imported page");

    // what is selected, and whether the fix lays an auto-layout frame out itself (that moves the layout counter)
    let cases: [(&str, Vec<NodeId>, bool); 3] =
        [("nothing selected", vec![], true), ("the toolbar", vec![l.toolbar], true), ("a label inside the toolbar", vec![l.toolbar_label], false)];
    for (name, selection, lays_out) in cases {
        let (doc, l) = imported_doc();
        let (mut s, l) = sim_on(doc, l);
        assert_eq!(toolbar_children_at(&s.app().doc, &l), hand_placed, "{name}: opening the page left the toolbar as the file has it");
        s.shell.app.selection = selection;
        let before = state(&s);
        let epoch_before = s.app().doc.layout_epoch();

        auto_fix(&mut s);

        let after = state(&s);
        assert_ne!(after, before, "{name}: the fix changed the document");
        let epoch_fixed = s.app().doc.layout_epoch();
        let recorded = s.app().hist.peek_undo_layout_recorded();

        s.cmd_key(Key::Z);
        assert_same_state(&state(&s), &before, &format!("{name}: one Cmd+Z restores the imported page exactly"));
        assert_eq!(s.app().doc.layout_epoch(), epoch_fixed, "{name}: undo replays the stored step and solves no layout");
        s.cmd_key(Key::Y);
        assert_same_state(&state(&s), &after, &format!("{name}: redo is the fixed state"));
        assert_eq!(s.app().doc.layout_epoch(), epoch_fixed, "{name}: redo solves no layout either");
        assert!(recorded, "{name}: the step holds its own layout, so undo and redo replay it as stored");
        // the case is the one it is meant to be: the fix moves the layout counter exactly when it lays a frame out itself
        assert_eq!(epoch_fixed != epoch_before, lays_out, "{name}: whether the fix laid a frame out");
    }
}

/// Frames in the nest: past the 32 hops one relayout pass reaches, so widening the outer one leaves a tail queued,
/// and short enough that one more pass from the middle of the nest finishes it.
const NEST: usize = 40;

/// A nest of `NEST` end-aligned frames, each filling its parent, around a 10 px leaf, laid out at 200 px.
fn nest_doc() -> (Document, Vec<NodeId>, NodeId) {
    let mut d = Document::new("Nest");
    let page = d.add_page("P");
    let fill = LayoutChild { horizontal: Sizing::Fill, vertical: Sizing::Fill, ..LayoutChild::default() };
    let mut frames = Vec::with_capacity(NEST);
    let mut parent = page;
    for i in 0..NEST {
        let id = d.alloc_id();
        let mut f = Node::frame(id, DRect::new(0.0, 0.0, 200.0, 40.0));
        if let NodeKind::Frame(fp) = &mut f.kind {
            fp.auto_layout = Some(AutoLayout {
                direction: LayoutDirection::Horizontal,
                primary_sizing: Sizing::Fixed,
                counter_sizing: Sizing::Fixed,
                primary_align: LayoutAlign::End,
                item_spacing: 8.0,
                ..AutoLayout::default()
            });
        }
        f.layout_child = if i == 0 { LayoutChild::default() } else { fill };
        d.insert(parent, f).unwrap();
        frames.push(id);
        parent = id;
    }
    let leaf = d.alloc_id();
    d.insert(parent, Node::rectangle(leaf, DRect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
    d.relayout_subtree(frames[0]);
    d.flush_pending_relayout();
    (d, frames, leaf)
}

#[test]
fn a_fix_that_solves_layout_another_edit_left_queued_does_not_claim_it_as_its_own() {
    let _guard = serialized();
    let (mut d, frames, _leaf) = nest_doc();
    let page = d.pages[0];
    // an edit made the outer frame wider and its relayout stopped part way down, leaving the tail queued
    d.get_mut(frames[0]).unwrap().size.x = 600.0;
    d.relayout_subtree(frames[0]);
    assert!(d.relayout_skipped_count() > 0, "precondition: the nest is queued to be finished");
    // a frame in the middle of the nest has a spacing off the 4px grid, so the fix lays it, and the queue below it, out
    if let Some(fp) = d.get_mut(frames[10]).unwrap().kind.frame_props_mut() {
        if let Some(al) = fp.auto_layout.as_mut() {
            al.item_spacing = 7.0;
        }
    }
    let mut app = crate::app::App::new(d);
    app.page = page;
    let epoch_before = app.doc.layout_epoch();

    let outcome = crate::cmdk::run_auto_fix_design_gate(&mut app);

    assert!(matches!(outcome, crate::cmdk::AutoFixOutcome::Committed { .. }), "the fix changed the spacing: {outcome:?}");
    assert!(app.doc.pending_relayout().is_empty(), "precondition: the fix solved the queued tail as part of its own layout");
    assert_ne!(app.doc.layout_epoch(), epoch_before, "precondition: the fix's layout moved the counter");
    assert!(
        !app.hist.peek_undo_layout_recorded(),
        "the step holds the state of that tail from before it was solved, so it is not vouched for and undo solves layout again"
    );
}

#[test]
fn a_fix_with_nothing_to_change_is_decided_before_the_transaction_runs() {
    let _guard = serialized();
    let (mut s, _l) = sim_on_fixable_doc();
    auto_fix(&mut s);
    let fixed = state(&s);
    let depth = s.app().hist.undo_depth();
    // the transaction marks the canvas changed for every edit it handles, an empty one included
    s.shell.app.canvas.dirty = false;

    let outcome = crate::cmdk::run_auto_fix_design_gate(&mut s.shell.app);

    assert_eq!(outcome, crate::cmdk::AutoFixOutcome::NoChange);
    assert!(!s.app().canvas.dirty, "the transaction never ran for an edit that holds nothing");
    assert_same_state(&state(&s), &fixed, "nothing written");
    assert_eq!(s.app().hist.undo_depth(), depth, "no undo step");
}

#[test]
fn the_outcome_tells_a_committed_fix_from_a_no_op_and_from_a_refusal() {
    let _guard = serialized();
    let (mut s, _l) = sim_on_fixable_doc();
    let committed = crate::cmdk::run_auto_fix_design_gate(&mut s.shell.app);
    assert!(matches!(committed, crate::cmdk::AutoFixOutcome::Committed { op: None, count } if count > 0), "the fix changed layers: {committed:?}");
    let again = crate::cmdk::run_auto_fix_design_gate(&mut s.shell.app);
    assert_eq!(again, crate::cmdk::AutoFixOutcome::NoChange, "the second run has nothing left to change");

    let (mut s, _dir) = library_sim();
    view_an_old_version_with_fixable_layers(&mut s);
    s.shell.app.selection.clear();
    let refused = crate::cmdk::run_auto_fix_design_gate(&mut s.shell.app);
    assert_eq!(refused, crate::cmdk::AutoFixOutcome::Refused, "an old version is read-only");
    assert_eq!(toast_text(&s).as_deref(), Some("Viewing an old version is read-only"), "and the refusal says why");
}

const FLUSH_NOTICE: &str = "Saved without 3 numbers that were not finite";

/// Clicks the palette's fix right after a flush raised `FLUSH_NOTICE` in the same frame, as `save_library_file`
/// leaves it, and checks the toast says what the click did and still carries the notice.
#[track_caller]
fn click_the_fix_in_the_frame_of_a_flush_notice(s: &mut Sim, words: &str) {
    s.shell.app.show_toast(FLUSH_NOTICE, true);
    let now = s.shell.app.now;
    s.shell.app.pending_flush_notice = Some((now, FLUSH_NOTICE.to_string()));

    crate::shell::execute_cmdk_action(&mut s.shell.app, CmdKAction::AutoFixDesignGate);

    let toast = s.app().toast.as_ref().expect("the click shows a toast");
    assert!(toast.text.contains(words), "the click still says what it did: {:?}", toast.text);
    assert!(toast.text.contains(FLUSH_NOTICE), "the notice raised in the same frame is not dropped: {:?}", toast.text);
    assert!(toast.error, "a toast that carries a notice is an error toast");
}

#[test]
fn a_notice_raised_in_the_frame_of_the_click_is_kept_by_a_fix() {
    let _guard = serialized();
    let (mut s, _l) = sim_on_fixable_doc();
    click_the_fix_in_the_frame_of_a_flush_notice(&mut s, "Spacing and tap targets fixed");
}

#[test]
fn a_notice_raised_in_the_frame_of_the_click_is_kept_by_a_no_op() {
    let _guard = serialized();
    let (mut s, _l) = sim_on_fixable_doc();
    auto_fix(&mut s);
    click_the_fix_in_the_frame_of_a_flush_notice(&mut s, "Nothing changed");
}
