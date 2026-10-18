//! Auto-layout subtrees a relayout deferred past its re-solve budget: the window finishes them
//! lazily, one bounded step per frame, and every save, version snapshot and export finishes them
//! first so nothing leaves the process with a stale layout.

use super::*;
use crate::home::Act;
use designos_doc::{AutoLayout, Command, Document, LayoutAlign, LayoutChild, LayoutDirection, Node, Rect as DRect, Sizing};
use designos_library::Library;
use std::path::PathBuf;

/// One more than the 4096 re-solves a single relayout pass may spend.
const ROWS: usize = 4_097;
/// Frames in the nest: past 33 hops from the changed node, and needing two lazy steps to finish.
const CHAIN: usize = 70;
const NOTICE: &str = "Layout incomplete";

struct TmpDir(PathBuf);

impl TmpDir {
    fn new() -> Self {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("designos_relayout_budget_{}_{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        Self(p)
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// An editor with the library's "Welcome" file open.
fn file_sim() -> (Sim, TmpDir) {
    let dir = TmpDir::new();
    let mut s = Sim::with(Options { home: true, library: Some(dir.0.clone()), ..Default::default() });
    let id = lib(&s).all_files().into_iter().find(|f| f.name == "Welcome").expect("seeded file").id;
    crate::home::apply(&mut s.shell.app, Act::Open(id));
    s.frame(vec![]);
    s.frame(vec![]);
    (s, dir)
}

fn lib(s: &Sim) -> Library {
    s.app().lib.clone().expect("a library")
}

fn file_id(s: &Sim) -> String {
    s.app().lib_file.as_ref().expect("a library file is open").id.clone()
}

fn horizontal(align: LayoutAlign) -> AutoLayout {
    AutoLayout {
        direction: LayoutDirection::Horizontal,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        primary_align: align,
        ..AutoLayout::default()
    }
}

fn add_frame(doc: &mut Document, parent: NodeId, rect: DRect, layout: AutoLayout, as_child: LayoutChild) -> NodeId {
    let id = doc.alloc_id();
    let mut f = Node::frame(id, rect);
    if let NodeKind::Frame(fp) = &mut f.kind {
        fp.auto_layout = Some(layout);
    }
    f.layout_child = as_child;
    doc.insert(parent, f).unwrap();
    id
}

fn fill(horizontal: Sizing, vertical: Sizing) -> LayoutChild {
    LayoutChild { horizontal, vertical, ..LayoutChild::default() }
}

/// A vertical frame of `ROWS` centred rows, each holding one 20 px rectangle. Widening the frame
/// to 500 moves every rectangle from x = 40 to x = 240, and the last row is the one the 4096
/// re-solve budget leaves for the lazy pass.
fn add_rows(doc: &mut Document, page: NodeId) -> (NodeId, Vec<NodeId>) {
    let vertical = AutoLayout { direction: LayoutDirection::Vertical, primary_sizing: Sizing::Fixed, counter_sizing: Sizing::Fixed, ..AutoLayout::default() };
    let parent = add_frame(doc, page, DRect::new(0.0, 0.0, 100.0, 2000.0), vertical, LayoutChild::default());
    let mut rects = Vec::with_capacity(ROWS);
    for _ in 0..ROWS {
        let row = add_frame(doc, parent, DRect::new(0.0, 0.0, 100.0, 20.0), horizontal(LayoutAlign::Center), fill(Sizing::Fill, Sizing::Fixed));
        let r = doc.alloc_id();
        doc.insert(row, Node::rectangle(r, DRect::new(40.0, 0.0, 20.0, 20.0))).unwrap();
        rects.push(r);
    }
    (parent, rects)
}

/// Widens the rows frame and relayouts it the way an edit does: one row is left queued.
fn widen_rows(app: &mut App, parent: NodeId) {
    app.doc.get_mut(parent).unwrap().size.x = 500.0;
    app.doc.relayout_subtree(parent);
    app.changed();
    assert_eq!(app.doc.relayout_skipped_count(), 1, "precondition: the row past the budget is queued");
}

/// A file sim whose open document holds the rows frame, widened, with its last row still queued.
fn sim_with_one_row_queued() -> (Sim, TmpDir, Vec<NodeId>) {
    let (mut s, dir) = file_sim();
    let page = s.app().page;
    let (parent, rects) = add_rows(&mut s.shell.app.doc, page);
    widen_rows(&mut s.shell.app, parent);
    (s, dir, rects)
}

fn saved_tx(s: &Sim, rect: NodeId) -> f32 {
    lib(s).load_file(&file_id(s)).unwrap().get(rect).unwrap().transform.tx
}

fn toast_text(s: &Sim) -> Option<String> {
    s.app().toast.as_ref().map(|t| t.text.clone())
}

#[test]
fn a_saved_file_holds_the_finished_layout_not_the_queued_one() {
    let (mut s, _d, rects) = sim_with_one_row_queued();
    assert_eq!(s.app().doc.get(rects[ROWS - 1]).unwrap().transform.tx, 40.0, "precondition: the last row is still stale in memory");
    s.shell.app.save_library_file().unwrap();
    assert_eq!(saved_tx(&s, rects[ROWS - 1]), 240.0, "the queued row must be finished before the file is written");
    assert_eq!(saved_tx(&s, rects[0]), 240.0);
    assert_eq!(s.app().doc.relayout_skipped_count(), 0);
}

#[test]
fn saving_an_external_file_writes_the_finished_layout() {
    let dir = TmpDir::new();
    std::fs::create_dir_all(&dir.0).unwrap();
    let ext = dir.0.join("outside.designos");
    let mut d = Document::new("Outside");
    d.add_page("P");
    std::fs::write(&ext, d.to_json().unwrap()).unwrap();
    let mut s = Sim::with(Options { file: Some(ext.clone()), library: Some(dir.0.join("lib")), ..Default::default() });
    assert!(s.app().lib_file.is_none(), "precondition: the file lives outside the library");
    let page = s.app().page;
    let (parent, rects) = add_rows(&mut s.shell.app.doc, page);
    widen_rows(&mut s.shell.app, parent);
    s.shell.app.save(false);
    let saved = Document::from_json(&std::fs::read_to_string(&ext).unwrap()).unwrap();
    assert_eq!(saved.get(rects[ROWS - 1]).unwrap().transform.tx, 240.0, "Save wrote the last row still queued");
    assert_eq!(s.app().doc.relayout_skipped_count(), 0);
}

#[test]
fn closing_the_window_saves_the_finished_layout() {
    let (mut s, _d, rects) = sim_with_one_row_queued();
    s.close_frame();
    assert_eq!(saved_tx(&s, rects[ROWS - 1]), 240.0, "the close frame wrote the file with the last row still queued");
    let lib = lib(&s);
    let versions = lib.list_versions(&file_id(&s));
    let last = versions.last().expect("closing stores a last version");
    assert_eq!(lib.load_version(&file_id(&s), last.n).unwrap().get(rects[ROWS - 1]).unwrap().transform.tx, 240.0, "the close version holds the stale row");
}

#[test]
fn viewing_a_version_writes_the_finished_layout_to_disk_first() {
    let (mut s, _d) = file_sim();
    assert!(s.shell.app.save_named_version("before", ""), "a version to view later");
    let n = lib(&s).list_versions(&file_id(&s)).last().unwrap().n;
    let page = s.app().page;
    let (parent, rects) = add_rows(&mut s.shell.app.doc, page);
    widen_rows(&mut s.shell.app, parent);
    s.shell.app.view_version(n);
    assert_eq!(saved_tx(&s, rects[ROWS - 1]), 240.0, "the file written before the live document is parked holds the queued row stale");
}

#[test]
fn an_export_reads_the_finished_layout() {
    let (s, _d, rects) = sim_with_one_row_queued();
    assert_eq!(s.app().export_doc().get(rects[ROWS - 1]).unwrap().transform.tx, 240.0, "exporters must not see the queued row stale");
}

#[test]
fn a_one_step_deferral_is_finished_before_paint_and_never_toasts() {
    let (mut s, _d, rects) = sim_with_one_row_queued();
    s.frame(vec![]);
    assert_eq!(s.app().doc.relayout_skipped_count(), 0, "one step finishes a single deferred row");
    assert_eq!(s.app().doc.get(rects[ROWS - 1]).unwrap().transform.tx, 240.0);
    assert!(toast_text(&s).is_none_or(|t| !t.starts_with(NOTICE)), "nothing is incomplete any more: {:?}", toast_text(&s));
}

/// `CHAIN` nested Fill frames under the page; the leaf hugs the right edge of the innermost one.
fn add_chain(doc: &mut Document, page: NodeId, width: f32) -> (Vec<NodeId>, NodeId) {
    let mut frames = Vec::with_capacity(CHAIN);
    let mut cur = page;
    for i in 0..CHAIN {
        let as_child = if i == 0 { LayoutChild::default() } else { fill(Sizing::Fill, Sizing::Fill) };
        cur = add_frame(doc, cur, DRect::new(0.0, 0.0, width, 40.0), horizontal(LayoutAlign::End), as_child);
        frames.push(cur);
    }
    let leaf = doc.alloc_id();
    doc.insert(cur, Node::rectangle(leaf, DRect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
    (frames, leaf)
}

/// Where the CPU canvas believes `id` sits: the tile cache culls by this index.
fn indexed_x(s: &Sim, id: NodeId) -> f32 {
    s.app().canvas.tiles.scene_index().bounds_of(id).expect("the node is indexed").0
}

#[test]
fn the_canvas_follows_the_lazy_steps_on_every_frame() {
    let mut s = Sim::new();
    let page = s.app().page;
    let (frames, leaf) = add_chain(&mut s.shell.app.doc, page, 200.0);
    s.shell.app.doc.relayout_subtree(page);
    s.shell.app.changed();
    s.frame(vec![]);
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(leaf).unwrap().transform.tx, 190.0, "precondition: the leaf hugs the 200 px frame");
    assert!(indexed_x(&s, leaf) < 200.0, "precondition: the canvas has indexed the leaf at its first place");

    s.shell.app.doc.get_mut(frames[0]).unwrap().size.x = 600.0;
    s.shell.app.doc.relayout_subtree(page);
    s.shell.app.changed();
    assert!(s.app().doc.relayout_skipped_count() > 0, "precondition: the nest is deeper than one relayout reaches");

    s.frame(vec![]);
    assert!(s.app().doc.relayout_skipped_count() > 0, "the first lazy step leaves the innermost frames queued");
    assert!(toast_text(&s).is_some_and(|t| t.starts_with(NOTICE)), "the user is told while layout is unfinished");
    s.frame(vec![]);
    assert_eq!(s.app().doc.relayout_skipped_count(), 0, "the second step finishes the nest");
    assert_eq!(s.app().doc.get(leaf).unwrap().transform.tx, 590.0);
    assert!(indexed_x(&s, leaf) > 500.0, "the lazy step moved the leaf, so the canvas index must follow: {}", indexed_x(&s, leaf));
    assert!(toast_text(&s).is_none_or(|t| !t.starts_with(NOTICE)), "the notice goes away with the work it reported");
}

#[test]
fn undo_and_redo_finish_the_queue_and_retire_the_notice() {
    let mut s = Sim::new();
    let page = s.app().page;
    let (frames, leaf) = add_chain(&mut s.shell.app.doc, page, 200.0);
    s.shell.app.doc.relayout_subtree(page);
    s.shell.app.changed();
    s.frame(vec![]);
    s.frame(vec![]);

    // The edit is one command, the outermost frame growing; its relayout reaches only the first frames.
    let mut wide = s.app().doc.get(frames[0]).unwrap().clone();
    wide.size.x = 600.0;
    let app = &mut s.shell.app;
    app.hist.exec(&mut app.doc, Command::Update { node: Box::new(wide) }).unwrap();
    app.doc.relayout_subtree(page);
    app.changed();
    s.frame(vec![]);
    assert!(s.app().doc.relayout_skipped_count() > 0, "precondition: the first lazy step leaves frames queued");
    assert!(toast_text(&s).is_some_and(|t| t.starts_with(NOTICE)), "precondition: the user was told");

    s.shell.app.undo();
    assert_eq!(s.app().doc.get(frames[0]).unwrap().size.x, 200.0, "the undo itself restores the edited frame");
    assert!(s.app().doc.relayout_skipped_count() > 0, "the undo solves within its hop budget and leaves the deep tail to the next frames");
    assert!(toast_text(&s).is_some_and(|t| t.starts_with(NOTICE)), "the user is told while the tail is unfinished");
    for _ in 0..4 {
        s.frame(vec![]);
    }
    assert_eq!(s.app().doc.relayout_skipped_count(), 0, "the lazy steps finish the tail");
    assert!(toast_text(&s).is_none_or(|t| !t.starts_with(NOTICE)), "the notice goes away with the work it reported: {:?}", toast_text(&s));
    assert!(
        frames.iter().all(|f| s.app().doc.get(*f).unwrap().size.x == 200.0),
        "undo restores every frame in the chain to 200 px: {:?}",
        frames.iter().map(|f| s.app().doc.get(*f).unwrap().size.x).collect::<Vec<_>>()
    );
    assert_eq!(s.app().doc.get(leaf).unwrap().transform.tx, 190.0, "undo restores the leaf position to 190 px");

    s.shell.app.redo();
    for _ in 0..4 {
        s.frame(vec![]);
    }
    assert_eq!(s.app().doc.relayout_skipped_count(), 0);
    assert!(frames.iter().all(|f| s.app().doc.get(*f).unwrap().size.x == 600.0), "redo restores every frame in the chain to 600 px");
    assert_eq!(s.app().doc.get(leaf).unwrap().transform.tx, 590.0);
}

#[test]
fn sync_live_file_context_waits_for_pending_relayout_to_drain() {
    let mut s = Sim::new();
    let page = s.app().page;
    let (frames, leaf) = add_chain(&mut s.shell.app.doc, page, 200.0);
    s.shell.app.doc.relayout_subtree(page);
    s.shell.app.changed();
    s.frame(vec![]);
    s.frame(vec![]);
    assert_eq!(s.app().doc.relayout_skipped_count(), 0);
    assert_eq!(s.app().doc.get(leaf).unwrap().transform.tx, 190.0);

    let session_dir = s.app().terminal.session_dir.clone();
    let canvas_path = s.app().terminal.live_canvas_path.clone();
    let initial = Document::from_json(&std::fs::read_to_string(&canvas_path).unwrap()).unwrap();
    assert_eq!(initial.get(frames[0]).unwrap().size.x, 200.0);
    assert_eq!(initial.get(leaf).unwrap().transform.tx, 190.0);

    // Widen the outermost frame: `relayout_subtree` defers the deeper half of the chain.
    s.shell.app.doc.get_mut(frames[0]).unwrap().size.x = 600.0;
    s.shell.app.doc.relayout_subtree(page);
    s.shell.app.changed();
    assert!(s.app().doc.relayout_skipped_count() > 0, "precondition: relayout is still pending");

    // Calling publish while the queue is non-empty must NOT write the half-solved document.
    crate::live_patch_apply::publish_live_context(&mut s.shell.app);
    let while_queued = Document::from_json(&std::fs::read_to_string(&canvas_path).unwrap()).unwrap();
    assert_eq!(while_queued.get(frames[0]).unwrap().size.x, 200.0, "live canvas must not publish while relayout is incomplete");
    assert_eq!(while_queued.get(leaf).unwrap().transform.tx, 190.0);

    // Frame 1 advances one lazy step; the innermost frames are still queued, so publish stays held.
    s.frame(vec![]);
    assert!(s.app().doc.relayout_skipped_count() > 0, "precondition: first lazy step leaves tail queued");
    let after_step_1 = Document::from_json(&std::fs::read_to_string(&canvas_path).unwrap()).unwrap();
    assert_eq!(after_step_1.get(frames[0]).unwrap().size.x, 200.0, "live canvas must stay held until the queue finishes");

    // Frame 2 drains the queue and publishes the fully solved document once.
    s.frame(vec![]);
    assert_eq!(s.app().doc.relayout_skipped_count(), 0, "second lazy step drains the queue");
    let after_drain = Document::from_json(&std::fs::read_to_string(&canvas_path).unwrap()).unwrap();
    assert!(frames.iter().all(|f| after_drain.get(*f).unwrap().size.x == 600.0), "live canvas publishes once all 70 frames are solved");
    assert_eq!(after_drain.get(leaf).unwrap().transform.tx, 590.0);

    s.shell.app.terminal.shutdown();
    let _ = std::fs::remove_dir_all(&session_dir);
}
