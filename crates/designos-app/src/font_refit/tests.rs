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
