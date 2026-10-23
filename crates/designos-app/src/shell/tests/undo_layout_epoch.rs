//! A recorder that stays open across frames (a scrub, or an edit session kept alive by a gesture)
//! must not take credit for layout that was written outside it. When the per-frame drain of
//! another step's deferred layout runs while the recorder is open, the recorder's "before" values
//! hold the unsolved geometry. A step marked as recorded would then replay that geometry on undo,
//! with nothing left in the queue to finish it, and leave the nest half laid out.

use crate::app::App;
use crate::edit::Tx;
use designos_doc::{AutoLayout, Command, Document, LayoutAlign, LayoutChild, LayoutDirection, Node, NodeId, NodeKind, Rect as DRect, Sizing};

/// Frames in the nest: past the 32 hops one relayout pass reaches, so a widening leaves a tail.
const CHAIN: usize = 70;

fn horizontal() -> AutoLayout {
    AutoLayout {
        direction: LayoutDirection::Horizontal,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        primary_align: LayoutAlign::End,
        ..AutoLayout::default()
    }
}

fn add_frame(doc: &mut Document, parent: NodeId, as_child: LayoutChild) -> NodeId {
    let id = doc.alloc_id();
    let mut f = Node::frame(id, DRect::new(0.0, 0.0, 200.0, 40.0));
    if let NodeKind::Frame(fp) = &mut f.kind {
        fp.auto_layout = Some(horizontal());
    }
    f.layout_child = as_child;
    doc.insert(parent, f).unwrap();
    id
}

/// A nest of `CHAIN` end-aligned frames, each filling its parent, around a 10 px leaf.
fn add_chain(doc: &mut Document, page: NodeId) -> (Vec<NodeId>, NodeId) {
    let fill = LayoutChild { horizontal: Sizing::Fill, vertical: Sizing::Fill, ..LayoutChild::default() };
    let mut frames = Vec::with_capacity(CHAIN);
    let mut cur = page;
    for i in 0..CHAIN {
        cur = add_frame(doc, cur, if i == 0 { LayoutChild::default() } else { fill });
        frames.push(cur);
    }
    let leaf = doc.alloc_id();
    doc.insert(cur, Node::rectangle(leaf, DRect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
    (frames, leaf)
}

struct Scene {
    app: App,
    frames: Vec<NodeId>,
    leaf: NodeId,
    /// A rectangle outside the nest.
    rect: NodeId,
}

/// A page with the nest laid out at 200 px and an unrelated rectangle at x = 900.
fn scene() -> Scene {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let (frames, leaf) = add_chain(&mut d, page);
    d.relayout_subtree(frames[0]);
    d.flush_pending_relayout();
    let rid = d.alloc_id();
    let rect = d.insert(page, Node::rectangle(rid, DRect::new(900.0, 900.0, 20.0, 20.0))).unwrap();
    let mut app = App::new(d);
    app.page = page;
    Scene { app, frames, leaf, rect }
}

/// One bounded relayout step per call, as the window runs one per frame, until the queue is empty.
fn drain(app: &mut App) {
    while app.step_pending_relayout() {}
}

/// An agent step widens the outer frame and lays it out after the step, the way a live patch does.
/// The nest is deeper than one pass reaches, so the rest of it stays queued.
fn agent_widens_the_nest(app: &mut App, frames: &[NodeId]) {
    let mut wide = app.doc.get(frames[0]).unwrap().clone();
    wide.size.x = 600.0;
    app.hist.exec_agent(&mut app.doc, Command::Update { node: Box::new(wide) }).unwrap();
    app.doc.relayout_subtree(frames[0]);
    assert!(app.doc.relayout_skipped_count() > 0, "precondition: the agent's relayout left a tail queued");
}

/// The nest agrees with its outer frame: every frame is `width` wide, the leaf sits at its right
/// edge, and no deferred layout is left to finish it.
fn assert_nest_settled(app: &App, frames: &[NodeId], leaf: NodeId, width: f32) {
    let widths: Vec<f32> = frames.iter().map(|f| app.doc.get(*f).unwrap().size.x).collect();
    let off = widths.iter().position(|w| *w != width);
    assert_eq!(off, None, "every frame of the nest is {width} px wide; the first that is not is at depth {off:?}: {widths:?}");
    assert_eq!(app.doc.get(leaf).unwrap().transform.tx, width - 10.0, "the leaf sits at the right edge of the innermost frame");
    assert_eq!(app.doc.pending_relayout().len(), 0, "nothing is queued, so nothing will finish a half laid out nest later");
}

#[test]
fn a_scrub_that_sees_a_tail_drain_is_not_recorded_and_undoes_to_a_consistent_nest() {
    let Scene { mut app, frames, leaf, rect } = scene();
    agent_widens_the_nest(&mut app, &frames);
    // the scrub takes its whole-document snapshot while the tail is still queued
    app.begin_scrub(app.doc.nodes.clone());
    drain(&mut app);
    assert_nest_settled(&app, &frames, leaf, 600.0);
    app.edit_nodes(&[rect], |n| n.transform.tx += 5.0);
    app.scrub_end();
    assert_eq!(app.hist.undo_depth(), 2, "precondition: the agent step and the scrub step");
    assert_eq!(app.doc.get(rect).unwrap().transform.tx, 905.0);
    assert!(!app.hist.peek_undo_layout_recorded(), "the scrub took in the drain of a tail that is not its own, so its step is not recorded");

    app.undo();
    drain(&mut app);
    assert_eq!(app.doc.get(rect).unwrap().transform.tx, 900.0, "the scrub is undone");
    assert_nest_settled(&app, &frames, leaf, 600.0);
}

#[test]
fn a_tx_kept_open_while_a_tail_drains_is_not_recorded_and_undoes_to_a_consistent_nest() {
    let Scene { mut app, frames, leaf, .. } = scene();
    agent_widens_the_nest(&mut app, &frames);
    // a gesture touched a frame in the middle of the queued tail before the drain wrote it
    let name = app.doc.get(frames[50]).unwrap().name.clone();
    let mut tx = Tx::new();
    tx.ed.node(&mut app.doc, frames[50]).unwrap().name = "renamed during the gesture".into();
    drain(&mut app);
    assert_nest_settled(&app, &frames, leaf, 600.0);
    assert!(app.run_tx(tx));
    assert_eq!(app.hist.undo_depth(), 2, "precondition: the agent step and the gesture's step");
    assert!(!app.hist.peek_undo_layout_recorded(), "the gesture's before values predate the drain, so its step is not recorded");

    app.undo();
    drain(&mut app);
    assert_eq!(app.doc.get(frames[50]).unwrap().name, name, "the rename is undone");
    assert_nest_settled(&app, &frames, leaf, 600.0);
}

/// Size and position of every node, in id order: what a canvas draws.
fn geometry(app: &App) -> Vec<(NodeId, designos_doc::Vec2, f32, f32)> {
    app.doc.nodes.values().map(|n| (n.id, n.size, n.transform.tx, n.transform.ty)).collect()
}

#[test]
fn a_scrub_and_an_edit_opened_after_the_tail_drained_are_still_recorded_and_undo_exactly() {
    let Scene { mut app, frames, leaf, .. } = scene();
    agent_widens_the_nest(&mut app, &frames);
    drain(&mut app);
    assert_nest_settled(&app, &frames, leaf, 600.0);
    let wide = geometry(&app);

    // a scrub that opens once the tail is gone narrows the whole nest
    app.begin_scrub(app.doc.nodes.clone());
    app.edit_nodes(&[frames[0]], |n| n.size.x = 300.0);
    app.scrub_end();
    assert!(app.hist.peek_undo_layout_recorded(), "nothing was written outside the scrub, so its step holds all of its layout");
    assert_nest_settled(&app, &frames, leaf, 300.0);
    let narrow = geometry(&app);
    app.undo();
    assert_eq!(geometry(&app), wide, "undo puts all 70 frames and the leaf back in the same call");
    app.redo();
    assert_eq!(geometry(&app), narrow, "redo narrows all 70 frames and the leaf in the same call");
    app.undo();

    // an edit that opens after the drain is recorded as well
    app.edit_nodes(&[frames[0]], |n| n.size.x = 300.0);
    assert!(app.hist.peek_undo_layout_recorded());
    app.undo();
    assert_eq!(geometry(&app), wide);
    assert_eq!(app.doc.pending_relayout().len(), 0, "an exact undo leaves nothing queued");
    app.redo();
    assert_eq!(geometry(&app), narrow);
}

#[test]
fn an_edit_that_only_issues_commands_is_not_recorded_when_a_tail_drains_before_it_commits() {
    let Scene { mut app, frames, leaf, .. } = scene();
    agent_widens_the_nest(&mut app, &frames);
    let (page, id) = (app.page, app.doc.alloc_id());
    let mut tx = Tx::new();
    let index = app.doc.get(page).unwrap().children.len();
    tx.cmd(&mut app.doc, Command::Insert { parent: page, index, nodes: vec![Node::rectangle(id, DRect::new(0.0, 0.0, 5.0, 5.0))] }).unwrap();
    drain(&mut app);
    assert_nest_settled(&app, &frames, leaf, 600.0);
    assert!(app.run_tx(tx));
    assert!(!app.hist.peek_undo_layout_recorded(), "the edit began before the drain, so the drain is not the edit's to replay");
}

#[test]
fn a_writer_outside_the_history_that_reports_itself_leaves_an_open_edit_or_scrub_unrecorded() {
    let Scene { mut app, rect, .. } = scene();
    app.edit_nodes(&[rect], |n| n.transform.tx += 5.0);
    assert!(app.hist.peek_undo_layout_recorded(), "control: with no outside writer the edit is recorded");

    // the font re-fit lays boxes out with the editor solver on the live document and commits nothing
    let mut tx = Tx::new();
    tx.ed.node(&mut app.doc, rect).unwrap().transform.tx += 5.0;
    app.doc.note_layout_written_outside_history();
    assert!(app.run_tx(tx));
    assert!(!app.hist.peek_undo_layout_recorded(), "an outside writer ran while the edit was open");

    app.begin_scrub(app.doc.nodes.clone());
    app.doc.note_layout_written_outside_history();
    app.edit_nodes(&[rect], |n| n.transform.tx += 5.0);
    app.scrub_end();
    assert!(!app.hist.peek_undo_layout_recorded(), "an outside writer ran while the scrub was open");

    // the next edit opens after the writer finished and is recorded again
    app.edit_nodes(&[rect], |n| n.transform.tx += 5.0);
    assert!(app.hist.peek_undo_layout_recorded());
}
