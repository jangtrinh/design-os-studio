//! The command palette's auto-fix and Figma AI actions run through the editor's transaction, so one Cmd+Z puts
//! the document back exactly and the toast that promises it tells the truth.
//!
//! The actions share a process-wide queue for the AI operation they run, so these tests hold [`LOCK`] while they
//! drive one: two of them in parallel could take each other's queued operation.

use super::unsaved_dot::{dot, lib, library_sim, wait, SETTLED};
use super::*;
use crate::cmdk::CmdKAction;
use designos_doc::{AutoLayout, Document, Flow, Node, Rect as DRect, TextStyle};
use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

static LOCK: Mutex<()> = Mutex::new(());

fn serialized() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
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
    Layers { first_frame, button, label, tail, placeholder }
}

fn fixable_doc() -> (Document, Layers) {
    let mut d = Document::new("Palette fix");
    let page = d.add_page("P");
    let layers = add_fixable_layers(&mut d, page);
    (d, layers)
}

fn sim_on_fixable_doc() -> (Sim, Layers) {
    let mut s = Sim::new();
    s.use_stub_agent_clis();
    let (doc, layers) = fixable_doc();
    s.shell.app.replace_document(doc, None);
    s.frame(vec![]);
    s.frame(vec![]);
    s.shell.app.selection.clear();
    assert!(!s.app().hist.can_undo(), "a freshly opened document has no history");
    (s, layers)
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
    assert_eq!(toast, "Changes applied · Cmd+Z to undo");
    assert!(!toast.contains("(s)") && !toast.contains('('), "no (s) and no parentheses: {toast:?}");
    assert!(toast.split_whitespace().count() <= 8, "microcopy of about eight words: {toast:?}");
    s.cmd_key(Key::Z);
    assert!(!s.app().hist.can_undo(), "the promised undo works");
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
const AI_ROWS: [(&str, &str); 5] = [
    ("cmdk-wire-prototype", "prototype"),
    ("cmdk-rename-layers", "boost"),
    ("cmdk-semantic-rename", "semantic"),
    ("cmdk-translate-vi", "translate"),
    ("cmdk-populate-content", "replace"),
];

#[test]
fn every_figma_ai_row_is_one_exact_undo_step_with_and_without_a_selection() {
    let _guard = serialized();
    let (mut s, l) = sim_on_fixable_doc();
    for selection in [vec![], vec![l.placeholder], vec![l.first_frame]] {
        for (row, query) in AI_ROWS {
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
