//! The re-fit after a face arrives, end to end through `App::poll_fonts`: it covers what a user edit
//! measured with a stand-in, and nothing that came from the file.

use super::*;
use crate::canvas::handles::Handle;
use crate::canvas::{Interaction, ResizeKind, ResizeState};
use crate::edit::Tx;
use crate::fonts::testkit;
use designos_doc::{Affine, AutoLayout, Document, LayoutDirection, Node, Rect, Sizing, TextRun, TextStyle, Vec2};

/// Narrow letters: far narrower in the stand-in font than in the Roboto Mono the test faces are made of.
const NARROW: &str = "iiiiiiiiiiiiiiii";
/// Groups of narrow letters that wrap in two lines in the stand-in and in six in the test face, 120 px wide.
const WRAPPED: &str = "iiiiiiii iiiiiiii iiiiiiii iiiiiiii iiiiiiii iiiiiiii";

/// What a Figma file stores for a text: a box that is not what this app measures.
const FILE_BOX: Vec2 = Vec2 { x: 77.0, y: 33.0 };

fn text_node(d: &mut Document, parent: NodeId, family: &str, content: &str, mode: TextAutoResize, size: Vec2) -> NodeId {
    let id = d.alloc_id();
    let style = TextStyle { font_family: family.into(), font_size: 16.0, ..Default::default() };
    let mut n = Node::text(id, Rect::new(0.0, 0.0, size.x, size.y), content, style);
    if let NodeKind::Text(t) = &mut n.kind {
        t.auto_resize = mode;
    }
    d.insert(parent, n).unwrap()
}

fn hugging(direction: LayoutDirection, counter: Sizing) -> AutoLayout {
    AutoLayout { direction, padding: [8.0; 4], primary_sizing: Sizing::Hug, counter_sizing: counter, layout_version: 1, ..AutoLayout::default() }
}

/// A document opened from a file: a horizontal frame that hugs both ways, padded 8, holding one
/// hugging text per entry of `families`, each with the box the file stores (`FILE_BOX`). The app's
/// loader serves `served`.
fn opened_row(families: &[&str], served: &[&str]) -> (App, NodeId, Vec<NodeId>) {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let rid = d.alloc_id();
    let mut row = Node::frame(rid, Rect::new(0.0, 0.0, 100.0, 40.0));
    if let NodeKind::Frame(fp) = &mut row.kind {
        fp.auto_layout = Some(hugging(LayoutDirection::Horizontal, Sizing::Hug));
    }
    let row = d.insert(page, row).unwrap();
    let texts: Vec<NodeId> = families.iter().map(|f| text_node(&mut d, row, f, NARROW, TextAutoResize::WidthAndHeight, FILE_BOX)).collect();
    let mut app = App::new(d);
    layout::layout_node(&mut app.doc, &mut Edit::new(), row);
    app.fonts = testkit::serving(served);
    (app, row, texts)
}

/// A user edit that grows the text by a letter and measures it, as the editor does: with whatever
/// face is in place when the edit runs.
fn type_a_letter(app: &mut App, id: NodeId) {
    app.edit_nodes(&[id], |n| {
        if let NodeKind::Text(t) = &mut n.kind {
            t.content.push('i');
            n.size = layout::fit_text(t, n.size);
        }
    });
}

/// Loads what the document names and runs the load and the re-fit to the end, one frame at a time.
fn arrive(app: &mut App) {
    app.fonts.load_for(&app.doc);
    settle(app);
}

fn settle(app: &mut App) {
    for _ in 0..6000 {
        app.poll_fonts();
        if !app.fonts.is_loading() && !app.text_refit_pending() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the load or the re-fit did not finish");
}

/// Polls until the load has finished and registered its faces, whatever the re-fit does.
fn load_only(app: &mut App) {
    app.fonts.load_for(&app.doc);
    for _ in 0..6000 {
        app.poll_fonts();
        if !app.fonts.is_loading() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the load did not finish");
}

fn size_of(app: &App, id: NodeId) -> Vec2 {
    app.doc.get(id).unwrap().size
}

fn fits(app: &App, id: NodeId) -> bool {
    let n = app.doc.get(id).unwrap();
    matches!(&n.kind, NodeKind::Text(t) if layout::fit_text(t, n.size) == n.size)
}

#[test]
fn a_text_from_the_file_is_untouched_when_its_family_arrives() {
    let (mut app, row, texts) = opened_row(&["TestRefitA1", "TestRefitA1"], &["TestRefitA1"]);
    // the second text happens to be the size this app measures with the stand-in: still from the file
    let stand_in = match &app.doc.get(texts[1]).unwrap().kind {
        NodeKind::Text(t) => layout::fit_text(t, FILE_BOX),
        _ => unreachable!(),
    };
    app.doc.get_mut(texts[1]).unwrap().size = stand_in;
    layout::layout_node(&mut app.doc, &mut Edit::new(), row);
    let before = (size_of(&app, texts[0]), size_of(&app, texts[1]), size_of(&app, row));
    let epoch = app.doc.mutation_epoch();
    arrive(&mut app);
    assert_eq!(designos_render::resolve_font("TestRefitA1", 400, false).map(|f| f.family), Some("TestRefitA1".into()), "the face did arrive");
    assert!(!fits(&app, texts[1]), "the face changes what this app measures, so a re-fit would show");
    assert_eq!(size_of(&app, texts[0]), FILE_BOX, "the box the file stores is exactly what it was");
    assert_eq!((size_of(&app, texts[0]), size_of(&app, texts[1]), size_of(&app, row)), before, "nothing the file stored moved");
    assert_eq!(app.doc.mutation_epoch(), epoch, "the document was not written at all");
}

#[test]
fn a_text_typed_while_its_face_was_missing_is_re_fitted_on_arrival() {
    let (mut app, row, texts) = opened_row(&["TestRefitB1"], &["TestRefitB1"]);
    type_a_letter(&mut app, texts[0]);
    let (typed, row_before) = (size_of(&app, texts[0]), size_of(&app, row));
    assert!(fits(&app, texts[0]), "the box is the stand-in measure");
    let depth = app.hist.undo_depth();
    arrive(&mut app);
    let text = size_of(&app, texts[0]);
    assert!(fits(&app, texts[0]), "the box is what the arrived face needs: {text:?}");
    assert!(text.x > typed.x + 20.0, "the box was {typed:?} with the stand-in and is {text:?} with the face");
    assert_eq!(size_of(&app, row).x, text.x + 16.0, "the frame that hugs the text follows it (was {row_before:?})");
    assert_eq!(app.hist.undo_depth(), depth, "re-fitting is not an edit: no undo step of its own");
}

#[test]
fn a_family_picked_for_a_file_text_is_measured_again_when_it_arrives() {
    let (mut app, _row, texts) = opened_row(&["Inter"], &["TestRefitC1"]);
    app.edit_nodes(&[texts[0]], |n| {
        if let NodeKind::Text(t) = &mut n.kind {
            t.style.font_family = "TestRefitC1".into();
            n.size = layout::fit_text(t, n.size);
        }
    });
    let picked = size_of(&app, texts[0]);
    arrive(&mut app);
    assert!(fits(&app, texts[0]));
    assert!(size_of(&app, texts[0]).x > picked.x + 20.0, "{picked:?} then {:?}", size_of(&app, texts[0]));
}

#[test]
fn an_edit_that_leaves_the_box_alone_does_not_make_a_file_text_ours() {
    let (mut app, row, texts) = opened_row(&["TestRefitD1"], &["TestRefitD1"]);
    // moved and made half transparent, not measured: the file's box is still the box (a text colour is in `after_edits`)
    app.edit_nodes(&[texts[0]], |n| {
        n.transform.tx += 5.0;
        n.opacity = 0.5;
    });
    let before = (size_of(&app, texts[0]), size_of(&app, row));
    arrive(&mut app);
    assert_eq!((size_of(&app, texts[0]), size_of(&app, row)), before);
}

#[test]
fn a_wrapped_text_takes_the_lines_of_the_arrived_face() {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let fid = d.alloc_id();
    let mut col = Node::frame(fid, Rect::new(0.0, 0.0, 120.0, 40.0));
    if let NodeKind::Frame(fp) = &mut col.kind {
        fp.auto_layout = Some(hugging(LayoutDirection::Vertical, Sizing::Fixed));
    }
    let col = d.insert(page, col).unwrap();
    let t = text_node(&mut d, col, "TestRefitE1", WRAPPED, TextAutoResize::Height, Vec2::new(120.0, 20.0));
    let mut app = App::new(d);
    app.fonts = testkit::serving(&["TestRefitE1"]);
    type_a_letter(&mut app, t);
    let before = (size_of(&app, t), size_of(&app, col));
    arrive(&mut app);
    assert!(fits(&app, t));
    assert!(size_of(&app, t).y > before.0.y + 40.0, "more lines: {:?} then {:?}", before.0, size_of(&app, t));
    assert_eq!(size_of(&app, col).y, size_of(&app, t).y + 16.0, "the column that hugs the text follows its height (was {:?})", before.1);
    assert_eq!(size_of(&app, t).x, 120.0, "a wrapped text keeps its width");
}

#[test]
fn texts_in_other_families_are_left_alone() {
    let (mut app, _row, texts) = opened_row(&["TestRefitF1", "TestRefitF2"], &["TestRefitF1"]);
    type_a_letter(&mut app, texts[0]);
    type_a_letter(&mut app, texts[1]);
    let other = size_of(&app, texts[1]);
    arrive(&mut app);
    assert!(fits(&app, texts[0]));
    assert_eq!(size_of(&app, texts[1]), other, "a family that did not arrive is not measured again");
}

#[test]
fn a_run_in_the_arrived_family_counts() {
    let (mut app, _row, texts) = opened_row(&["Inter"], &["TestRefitG1"]);
    app.edit_nodes(&[texts[0]], |n| {
        if let NodeKind::Text(t) = &mut n.kind {
            t.runs.push(TextRun { start: 0, end: 8, style: TextStyle { font_family: "TestRefitG1".into(), font_size: 16.0, ..Default::default() } });
            n.size = layout::fit_text(t, n.size);
        }
    });
    let before = size_of(&app, texts[0]);
    arrive(&mut app);
    assert!(fits(&app, texts[0]));
    assert!(size_of(&app, texts[0]).x > before.x + 10.0, "the run in the new face widens the text: {before:?} then {:?}", size_of(&app, texts[0]));
}

#[test]
fn one_pass_per_arrival_then_nothing_more() {
    let (mut app, _row, texts) = opened_row(&["TestRefitH1"], &["TestRefitH1"]);
    type_a_letter(&mut app, texts[0]);
    arrive(&mut app);
    assert!(fits(&app, texts[0]));
    let epoch = app.doc.mutation_epoch();
    for _ in 0..5 {
        app.poll_fonts();
    }
    assert_eq!(app.doc.mutation_epoch(), epoch, "later frames write nothing");
    assert!(!app.text_refit_pending(), "and ask for no repaint");
}

#[test]
fn the_time_is_checked_before_each_text_so_a_slice_without_time_fits_one() {
    let (mut app, _row, texts) = opened_row(&["TestRefitI1"; 6], &["TestRefitI1"]);
    for id in &texts {
        type_a_letter(&mut app, *id);
    }
    app.fonts.refit.budget = Duration::ZERO;
    load_only(&mut app);
    assert!(app.text_refit_pending(), "the first slice left texts for the next frame");
    let fitted = |app: &App| texts.iter().filter(|t| fits(app, **t)).count();
    assert_eq!(fitted(&app), 1, "with no time, one text per slice, and the clock is read before the next one");
    let mut frames = 1;
    while app.text_refit_pending() {
        app.poll_fonts();
        frames += 1;
        // one unit per frame at most: the texts, then the frame that hugs them
        assert!(frames <= texts.len() + 1, "{frames} frames for {} texts and their row", texts.len());
    }
    assert_eq!(fitted(&app), texts.len(), "every text is fitted in the end");
}

#[test]
fn a_frame_is_laid_out_once_per_slice_not_once_per_text() {
    let (mut app, row, texts) = opened_row(&["TestRefitL1"; 6], &["TestRefitL1"]);
    for id in &texts {
        type_a_letter(&mut app, *id);
    }
    app.fonts.refit.budget = Duration::from_secs(60);
    let epoch = app.doc.mutation_epoch();
    load_only(&mut app);
    // the load's own poll ran the whole pass in one slice: every text is in place and the row hugs them
    assert!(texts.iter().all(|t| fits(&app, *t)));
    let width: f32 = texts.iter().map(|t| size_of(&app, *t).x).sum();
    assert_eq!(size_of(&app, row).x, width + 16.0);
    // six texts and the six boxes they move: laying the row out after each text would write it six times over
    let written = app.doc.mutation_epoch() - epoch;
    assert!(written <= 14, "{written} writes for six texts: the row was laid out more than once");
}

#[test]
fn a_drag_in_progress_holds_the_pass_and_the_text_is_fitted_after_it() {
    let (mut app, row, texts) = opened_row(&["TestRefitJ1"], &["TestRefitJ1"]);
    type_a_letter(&mut app, texts[0]);
    let typed = size_of(&app, row);
    // a drag that has touched the row, as a resize does
    let mut tx = Tx::new();
    tx.ed.node(&mut app.doc, row).unwrap().size.x += 1.0;
    app.canvas.inter = Interaction::Resize(ResizeState {
        tx,
        handle: Handle::R,
        kind: ResizeKind::Single { id: row, world0: Affine::IDENTITY, t0: Affine::IDENTITY, size0: typed },
        targets: Vec::new(),
    });
    load_only(&mut app);
    assert!(app.text_refit_pending() && app.text_refit_held(), "the pass waits for the drag");
    let epoch = app.doc.mutation_epoch();
    for _ in 0..3 {
        app.poll_fonts();
    }
    assert_eq!(app.doc.mutation_epoch(), epoch, "nothing is written while the drag runs");
    assert!(!fits(&app, texts[0]));
    // the next frame of the drag puts the layers back, and the drag ends
    if let Interaction::Resize(r) = &mut app.canvas.inter {
        r.tx.ed.revert(&mut app.doc);
    }
    app.canvas.inter = Interaction::None;
    settle(&mut app);
    assert!(fits(&app, texts[0]), "the set was kept, and the pass went on once the drag ended");
    assert!(size_of(&app, row).x > typed.x + 20.0, "the frame that hugs it follows");
}

#[test]
fn a_text_edited_while_a_gesture_holds_the_pass_joins_it() {
    let (mut app, _row, texts) = opened_row(&["TestRefitK1"], &["TestRefitK1"]);
    // what the stand-in measures for the text after the edit below, before the face is here
    let edited = match &app.doc.get(texts[0]).unwrap().kind {
        NodeKind::Text(t) => {
            let mut t = t.clone();
            t.content.push('i');
            layout::fit_text(&t, FILE_BOX)
        }
        _ => unreachable!(),
    };
    app.canvas.inter = Interaction::Pan;
    load_only(&mut app);
    assert!(!app.text_refit_pending(), "nothing is recorded yet: the text is still the file's");
    // a session that began before the face arrived commits the box it measured then
    app.edit_nodes(&[texts[0]], |n| {
        if let NodeKind::Text(t) = &mut n.kind {
            t.content.push('i');
            n.size = edited;
        }
    });
    assert!(app.text_refit_pending(), "the commit joined the held pass");
    app.canvas.inter = Interaction::None;
    settle(&mut app);
    assert!(fits(&app, texts[0]), "measured with the face that arrived: {:?}", size_of(&app, texts[0]));
    assert!(size_of(&app, texts[0]).x > edited.x + 20.0);
}

/// A 1x1 table, 144 wide so its text box is 120, whose cell holds `WRAPPED` in `family` with the
/// 40 px row the file stores.
fn opened_table(family: &str, served: &[&str]) -> (App, NodeId, NodeId) {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let mut nodes = designos_doc::build_table(&mut d, 1, 1, Vec2::new(0.0, 0.0));
    let (table, cell) = (nodes[0].id, nodes[1].id);
    if let NodeKind::Table(p) = &mut nodes[0].kind {
        p.cols[0].size = 144.0;
        p.rows[0].size = 40.0;
    }
    nodes[0].size = Vec2::new(144.0, 40.0);
    nodes[1].size = Vec2::new(144.0, 40.0);
    if let NodeKind::TableCell(c) = &mut nodes[1].kind {
        c.text.content = WRAPPED.into();
        c.text.style.font_family = family.into();
    }
    designos_doc::Command::Insert { parent: page, index: 0, nodes }.apply(&mut d).unwrap();
    let mut app = App::new(d);
    app.fonts = testkit::serving(served);
    (app, table, cell)
}

#[test]
fn a_table_row_grows_to_the_wrapped_text_of_a_face_that_arrives_late() {
    let (mut app, table, cell) = opened_table("TestRefTbl1", &["TestRefTbl1"]);
    designos_doc::apply_table_relayout(&mut app.doc, table);
    let with_stand_in = size_of(&app, cell).y;
    let depth = app.hist.undo_depth();
    arrive(&mut app);
    let with_face = size_of(&app, cell).y;
    assert!(with_face > with_stand_in + 20.0, "the row was {with_stand_in} with the stand-in and is {with_face} with the face");
    assert_eq!(size_of(&app, table).y, with_face, "the table follows its row");
    assert_eq!(app.hist.undo_depth(), depth, "a late face is not an edit");
}

#[test]
fn a_table_in_other_families_is_left_alone_when_a_face_arrives() {
    let (mut app, table, cell) = opened_table("Inter", &["TestRefTbl2"]);
    // a text elsewhere in the document asks for a family that has to be fetched, so a face does arrive
    let page = app.doc.get(table).and_then(|n| n.parent).unwrap();
    text_node(&mut app.doc, page, "TestRefTbl2", NARROW, TextAutoResize::WidthAndHeight, FILE_BOX);
    // the stored row is shorter than the wrapped Inter text needs: a re-solve would move it
    assert!(!designos_doc::table::relayout(&app.doc, table).is_empty(), "a re-solve would change this table");
    let before = size_of(&app, cell);
    let epoch = app.doc.mutation_epoch();
    arrive(&mut app);
    assert!(designos_render::resolve_font("TestRefTbl2", 400, false).is_some_and(|f| f.family == "TestRefTbl2"), "the face did arrive");
    assert_eq!(size_of(&app, cell), before, "the table in Inter did not move");
    assert_eq!(app.doc.mutation_epoch(), epoch, "the document was not written");
}

/// A 20 x 20 table (400 cells) of `WRAPPED` text in `family`, 144 wide columns, stored 40 px rows.
fn opened_big_table(family: &str, served: &[&str]) -> (App, NodeId, NodeId) {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let mut nodes = designos_doc::build_table(&mut d, 20, 20, Vec2::new(0.0, 0.0));
    let table = nodes[0].id;
    let first = nodes[1].id;
    if let NodeKind::Table(p) = &mut nodes[0].kind {
        p.cols.iter_mut().for_each(|c| c.size = 144.0);
        p.rows.iter_mut().for_each(|r| r.size = 40.0);
    }
    for n in nodes.iter_mut().skip(1) {
        n.size = Vec2::new(144.0, 40.0);
        if let NodeKind::TableCell(c) = &mut n.kind {
            c.text.content = WRAPPED.into();
            c.text.style.font_family = family.into();
        }
    }
    designos_doc::Command::Insert { parent: page, index: 0, nodes }.apply(&mut d).unwrap();
    let mut app = App::new(d);
    app.fonts = testkit::serving(served);
    (app, table, first)
}

#[test]
fn a_big_tables_layout_is_spread_over_frames_not_done_in_one() {
    let (mut app, table, first) = opened_big_table("TestRefTbl3", &["TestRefTbl3"]);
    app.fonts.refit.budget = Duration::ZERO;
    load_only(&mut app);
    assert!(app.text_refit_pending(), "the first slice left the table for the next frame");
    assert_eq!(size_of(&app, first).y, 40.0, "the table has not been laid out yet");
    let mut frames = 1;
    while app.text_refit_pending() {
        app.poll_fonts();
        frames += 1;
        assert!(frames <= 8, "{frames} frames for 400 cells in slices of 128");
    }
    assert!(frames >= 4, "400 cells took {frames} frames: the slices are 128 cells");
    assert!(size_of(&app, first).y > 60.0, "and the rows follow the face in the end: {:?}", size_of(&app, first));
    let _ = table;
}

#[test]
fn a_gesture_holds_the_table_layout_after_a_face_arrives() {
    let (mut app, table, cell) = opened_table("TestRefTbl4", &["TestRefTbl4"]);
    let row = size_of(&app, cell).y;
    let mut tx = Tx::new();
    tx.ed.node(&mut app.doc, table).unwrap().size.x += 1.0;
    app.canvas.inter = Interaction::Resize(ResizeState {
        tx,
        handle: Handle::R,
        kind: ResizeKind::Single { id: table, world0: Affine::IDENTITY, t0: Affine::IDENTITY, size0: Vec2::new(144.0, 40.0) },
        targets: Vec::new(),
    });
    load_only(&mut app);
    assert!(app.text_refit_pending() && app.text_refit_held(), "the table waits for the gesture");
    let epoch = app.doc.mutation_epoch();
    for _ in 0..3 {
        app.poll_fonts();
    }
    assert_eq!(app.doc.mutation_epoch(), epoch, "nothing is written while the gesture runs");
    assert_eq!(size_of(&app, cell).y, row);
    if let Interaction::Resize(r) = &mut app.canvas.inter {
        r.tx.ed.revert(&mut app.doc);
    }
    app.canvas.inter = Interaction::None;
    settle(&mut app);
    assert!(size_of(&app, cell).y > row + 20.0, "the queue was kept, and the table follows the face once the gesture ended");
}

/// A library file opened and left alone is not written back because a late face changed its table's
/// row heights: they are derived from the text, so the file still holds what the document holds.
#[test]
fn a_late_face_does_not_make_an_untouched_library_file_dirty_or_rewrite_it() {
    let dir = std::env::temp_dir().join(format!("designos-refit-lib-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut d = Document::new("File");
    let page = d.add_page("p");
    let mut nodes = designos_doc::build_table(&mut d, 1, 1, Vec2::new(0.0, 0.0));
    let (table, cell) = (nodes[0].id, nodes[1].id);
    if let NodeKind::Table(p) = &mut nodes[0].kind {
        p.cols[0].size = 144.0;
        p.rows[0].size = 40.0;
    }
    if let NodeKind::TableCell(c) = &mut nodes[1].kind {
        c.text.content = WRAPPED.into();
        c.text.style.font_family = "TestRefTbl5".into();
    }
    designos_doc::Command::Insert { parent: page, index: 0, nodes }.apply(&mut d).unwrap();
    let lib = designos_library::Library::open(dir.join("library")).unwrap();
    let proj = lib.create_project("Project").unwrap();
    let entry = lib.create_file(&proj.id, None, "File", &d).unwrap();
    let mut app = App::new(Document::new("empty"));
    app.fonts = testkit::serving(&["TestRefTbl5"]);
    app.lib = Some(lib.clone());
    app.open_library_file(&entry.id);
    let _ = table;
    let cell_height = |app: &App| size_of(app, cell).y;
    let at_open = cell_height(&app);
    let modified = lib.file(&entry.id).unwrap().modified;
    let saves = app.autosave.saves;
    arrive(&mut app);
    assert!(cell_height(&app) > at_open + 20.0, "the late face did change the row: {at_open} then {}", cell_height(&app));
    let ctx = egui::Context::default();
    for step in 1..=40 {
        app.now = 100.0 + step as f64 * 0.1;
        // every frame takes the revision a change marked, as the canvas does
        crate::canvas::tiles::take_rev(&mut app);
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            app.autosave_tick(ui.ctx());
        });
    }
    assert_eq!(app.autosave.saves, saves, "autosave wrote nothing");
    assert_eq!(lib.file(&entry.id).unwrap().modified, modified, "the file was not rewritten");
    assert!(!app.has_unsaved_edits(), "and the tab shows no dot");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A library file holding a 1 x 1 (or `n` x `n`) table in `family` and a marker rectangle, opened in the app.
struct OpenFile {
    app: App,
    lib: designos_library::Library,
    entry: String,
    cell: NodeId,
    marker: NodeId,
    dir: std::path::PathBuf,
}

fn open_file_with_table(family: &str, n: usize, tag: &str) -> OpenFile {
    let dir = std::env::temp_dir().join(format!("designos-refit-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut d = Document::new("File");
    let page = d.add_page("p");
    let mut nodes = designos_doc::build_table(&mut d, n, n, Vec2::new(0.0, 0.0));
    let cell = nodes[1].id;
    if let NodeKind::Table(p) = &mut nodes[0].kind {
        p.cols.iter_mut().for_each(|c| c.size = 144.0);
        p.rows.iter_mut().for_each(|r| r.size = 40.0);
    }
    for node in nodes.iter_mut().skip(1) {
        node.size = Vec2::new(144.0, 40.0);
        if let NodeKind::TableCell(c) = &mut node.kind {
            c.text.content = WRAPPED.into();
            c.text.style.font_family = family.into();
        }
    }
    designos_doc::Command::Insert { parent: page, index: 0, nodes }.apply(&mut d).unwrap();
    let marker = d.alloc_id();
    let mut m = Node::new(marker, NodeKind::Rectangle);
    m.size = Vec2::new(10.0, 10.0);
    m.transform = Affine::translate(-100.0, -100.0);
    d.insert(page, m).unwrap();
    let lib = designos_library::Library::open(dir.join("library")).unwrap();
    let proj = lib.create_project("Project").unwrap();
    let entry = lib.create_file(&proj.id, None, "File", &d).unwrap();
    let mut app = App::new(Document::new("empty"));
    app.fonts = testkit::serving(&[family]);
    app.lib = Some(lib.clone());
    app.open_library_file(&entry.id);
    OpenFile { app, lib, entry: entry.id, cell, marker, dir }
}

fn autosave_ticks(app: &mut App, from: f64, count: usize) {
    let ctx = egui::Context::default();
    for step in 1..=count {
        app.now = from + step as f64 * 0.1;
        crate::canvas::tiles::take_rev(app);
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| app.autosave_tick(ui.ctx()));
    }
}

fn move_marker(f: &mut OpenFile, x: f32) {
    f.app.edit_nodes(&[f.marker], |n| n.transform = Affine::translate(x, -100.0));
}

fn marker_x_on_disk(f: &OpenFile) -> f32 {
    f.lib.load_file(&f.entry).unwrap().get(f.marker).unwrap().transform.translation().x
}

/// A face arrives and lays out a table while the file holds an unsaved edit: the edit must still reach
/// the file, and the document must stay dirty until it does. Rebasing the saved state onto the new row
/// heights is only for a document that matched its file.
fn assert_the_edit_outlives_the_layout(f: &mut OpenFile, row_before: f32) {
    assert!(size_of(&f.app, f.cell).y > row_before, "the table was laid out with the face");
    assert!(f.app.has_unsaved_edits(), "the document is still dirty after the layout");
    autosave_ticks(&mut f.app, 100.0, 40);
    f.app.flush_autosave();
    assert_eq!(marker_x_on_disk(f), 500.0, "the edit reached the file");
    assert!(!f.app.has_unsaved_edits(), "and it is clean once saved");
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn an_edit_with_a_save_pending_reaches_the_file_when_a_late_face_lays_out_a_table() {
    let mut f = open_file_with_table("TestRefEd1", 1, "pending");
    move_marker(&mut f, 500.0);
    autosave_ticks(&mut f.app, 10.0, 1); // one tick has seen the edit: its save is pending
    let before = size_of(&f.app, f.cell).y;
    arrive(&mut f.app);
    assert_the_edit_outlives_the_layout(&mut f, before);
}

#[test]
fn an_edit_no_autosave_tick_has_seen_reaches_the_file_when_a_late_face_lays_out_a_table() {
    let mut f = open_file_with_table("TestRefEd2", 1, "unseen");
    let before = size_of(&f.app, f.cell).y;
    move_marker(&mut f, 500.0);
    arrive(&mut f.app);
    assert_the_edit_outlives_the_layout(&mut f, before);
}

#[test]
fn an_edit_made_between_two_warm_up_slices_reaches_the_file() {
    let mut f = open_file_with_table("TestRefEd3", 20, "slices");
    f.app.fonts.refit.budget = Duration::ZERO;
    load_only(&mut f.app);
    assert!(f.app.text_refit_pending(), "the table layout is sliced");
    f.app.poll_fonts();
    move_marker(&mut f, 500.0);
    let before = size_of(&f.app, f.cell).y;
    let mut frames = 0;
    while f.app.text_refit_pending() {
        f.app.poll_fonts();
        frames += 1;
        assert!(frames < 50, "the pass ends");
    }
    assert_the_edit_outlives_the_layout(&mut f, before);
}

mod after_edits;
mod guards;
mod open_edit;
mod recorded_edits;
mod undone_edits;

#[test]
fn a_script_face_matches_the_texts_that_use_the_script() {
    let arrived = Arrived::new(&["Noto Sans Arabic".to_string()]);
    let props = |family: &str, content: &str| TextProps {
        content: content.into(),
        style: TextStyle { font_family: family.into(), ..Default::default() },
        ..Default::default()
    };
    assert!(arrived.shapes(&props("Inter", "\u{0645}\u{0631}\u{062D}\u{0628}\u{0627}")), "Arabic text in Inter draws with the Noto fallback");
    assert!(!arrived.shapes(&props("Inter", "plain")), "Latin text does not");
    assert!(!arrived.shapes(&props("Inter", "\u{05E9}\u{05DC}\u{05D5}\u{05DD}")), "Hebrew is another script");
    assert!(arrived.shapes(&props("noto sans arabic", "plain")), "a text that names the family is matched by name, in any case");
}
