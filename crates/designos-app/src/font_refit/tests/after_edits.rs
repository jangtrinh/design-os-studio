//! What the re-fit does with a recorded text once the document has moved on: an undo, a redo, a colour,
//! a layout that resized it, a pass that ran in several slices.

use super::*;
use designos_doc::{Color, Paint};

/// A document opened from a file whose `count` texts each hug a row of their own.
pub(super) fn opened_rows(count: usize, family: &str, served: &[&str]) -> (App, Vec<NodeId>, Vec<NodeId>) {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let (mut rows, mut texts) = (Vec::new(), Vec::new());
    for _ in 0..count {
        let rid = d.alloc_id();
        let mut row = Node::frame(rid, Rect::new(0.0, 0.0, 100.0, 40.0));
        if let NodeKind::Frame(fp) = &mut row.kind {
            fp.auto_layout = Some(hugging(LayoutDirection::Horizontal, Sizing::Hug));
        }
        let row = d.insert(page, row).unwrap();
        texts.push(text_node(&mut d, row, family, NARROW, TextAutoResize::WidthAndHeight, FILE_BOX));
        rows.push(row);
    }
    let mut app = App::new(d);
    for row in &rows {
        layout::layout_node(&mut app.doc, &mut Edit::new(), *row);
    }
    app.fonts = testkit::serving(served);
    (app, rows, texts)
}

/// The network came back: the same loader state, now able to fetch `served`.
fn back_online(app: &mut App, served: &[&str]) {
    let refit = std::mem::take(&mut app.fonts.refit);
    app.fonts = testkit::serving(served);
    app.fonts.refit = refit;
}

/// What this app measures for a file text with its stand-in: the box a file text must not be mistaken for.
pub(super) fn stand_in_measure(app: &App, id: NodeId) -> Vec2 {
    match &app.doc.get(id).unwrap().kind {
        NodeKind::Text(t) => layout::fit_text(t, FILE_BOX),
        _ => unreachable!(),
    }
}

/// A column that hugs its width, holding a heading in Inter and a text of `family` with `content` that fills the
/// width, with the box a file stores for it (a height this app does not measure).
fn column(family: &str, content: &str) -> (App, NodeId, NodeId, NodeId) {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let cid = d.alloc_id();
    let mut col = Node::frame(cid, Rect::new(0.0, 0.0, 200.0, 100.0));
    if let NodeKind::Frame(fp) = &mut col.kind {
        fp.auto_layout = Some(hugging(LayoutDirection::Vertical, Sizing::Hug));
    }
    let col = d.insert(page, col).unwrap();
    let a = text_node(&mut d, col, "Inter", "Heading", TextAutoResize::WidthAndHeight, Vec2::new(60.0, 19.0));
    let b = text_node(&mut d, col, family, content, TextAutoResize::Height, Vec2::new(60.0, 33.0));
    d.get_mut(b).unwrap().layout_child.horizontal = Sizing::Fill;
    let mut app = App::new(d);
    layout::layout_node(&mut app.doc, &mut Edit::new(), col);
    app.doc.get_mut(b).unwrap().size.y = 33.0;
    layout::layout_node(&mut app.doc, &mut Edit::new(), col);
    app.fonts = testkit::serving(&[family]);
    (app, col, a, b)
}

/// A text session on `id` that appends a long phrase, not yet committed.
pub(super) fn type_a_phrase_in_a_session(app: &mut App, id: NodeId) {
    crate::canvas::textsession::begin_text_edit(app, id);
    let len = match &app.doc.get(id).unwrap().kind {
        NodeKind::Text(t) => t.content.len(),
        _ => 0,
    };
    if let Some(s) = app.canvas.text.as_mut() {
        s.caret = len;
        s.anchor = len;
    }
    crate::canvas::textsession::insert_text(app, " with a much longer title", crate::textedit::EditKind::Typing);
}

#[test]
fn undoing_the_edit_leaves_the_files_box_when_the_face_arrives() {
    let (mut app, row, texts) = opened_row(&["TestRefitR1"], &["TestRefitR1"]);
    let file = (size_of(&app, texts[0]), size_of(&app, row));
    type_a_letter(&mut app, texts[0]);
    assert_ne!(size_of(&app, texts[0]), file.0, "the edit measured a new box");
    app.undo();
    assert_eq!((size_of(&app, texts[0]), size_of(&app, row)), file, "the undo brought the file's box back");
    arrive(&mut app);
    assert_eq!((size_of(&app, texts[0]), size_of(&app, row)), file, "nobody edited this text since the file: its box is the file's");
    assert!(app.hist.can_redo(), "and the redo is still there");
}

#[test]
fn an_edit_undone_while_offline_leaves_the_files_box_when_the_face_comes_later() {
    let (mut app, row, texts) = opened_row(&["TestRefitS1"], &[]);
    // the open's fetch fails: nothing is served
    app.fonts.load_for(&app.doc);
    settle(&mut app);
    let file = (size_of(&app, texts[0]), size_of(&app, row));
    type_a_letter(&mut app, texts[0]);
    app.undo();
    assert_eq!((size_of(&app, texts[0]), size_of(&app, row)), file);
    // minutes later a paste, a tab switch or the family picker asks again, and the face is there
    back_online(&mut app, &["TestRefitS1"]);
    arrive(&mut app);
    assert!(designos_render::resolve_font("TestRefitS1", 400, false).is_some_and(|f| f.family == "TestRefitS1"), "the face did arrive");
    assert_eq!((size_of(&app, texts[0]), size_of(&app, row)), file, "the box the file stores is exactly what it was");
}

#[test]
fn redoing_the_edit_before_the_face_arrives_is_fitted_when_it_does() {
    let (mut app, _row, texts) = opened_row(&["TestRefitT1"], &["TestRefitT1"]);
    type_a_letter(&mut app, texts[0]);
    let typed = size_of(&app, texts[0]);
    app.undo();
    app.redo();
    assert_eq!(size_of(&app, texts[0]), typed, "the redo brings the committed box back");
    arrive(&mut app);
    assert!(fits(&app, texts[0]));
    assert!(size_of(&app, texts[0]).x > typed.x + 20.0, "the box the edit committed is fitted again: {typed:?} then {:?}", size_of(&app, texts[0]));
}

#[test]
fn undo_and_redo_after_the_refit_bring_back_the_fitted_size() {
    let (mut app, row, texts) = opened_row(&["TestRefitU1"], &["TestRefitU1"]);
    let file = (size_of(&app, texts[0]), size_of(&app, row));
    type_a_letter(&mut app, texts[0]);
    arrive(&mut app);
    let fitted = (size_of(&app, texts[0]), size_of(&app, row));
    assert!(fits(&app, texts[0]) && fitted.0 != file.0);
    app.undo();
    assert_eq!(size_of(&app, texts[0]), file.0, "the undo restores the file's box");
    app.redo();
    for _ in 0..5 {
        app.poll_fonts();
    }
    assert_eq!((size_of(&app, texts[0]), size_of(&app, row)), fitted, "the redo restores the fitted size, and nothing queues it again");
    assert!(!app.text_refit_pending());
}

#[test]
fn a_text_whose_second_face_arrives_later_is_fitted_again() {
    let (mut app, _row, texts) = opened_row(&["TestRefitV1"], &["TestRefitV1"]);
    app.edit_nodes(&[texts[0]], |n| {
        if let NodeKind::Text(t) = &mut n.kind {
            t.runs.push(TextRun { start: 0, end: 8, style: TextStyle { font_family: "TestRefitV2".into(), font_size: 16.0, ..Default::default() } });
            n.size = layout::fit_text(t, n.size);
        }
    });
    let typed = size_of(&app, texts[0]);
    arrive(&mut app);
    // the renderer shapes a text whose run is in a missing family with the stand-in for the whole text, so
    // the base face changes the line height only; what matters is that the box was written once
    let half = size_of(&app, texts[0]);
    assert_ne!(half, typed, "the base face arrived and the pass wrote the box");
    back_online(&mut app, &["TestRefitV2"]);
    arrive(&mut app);
    assert!(fits(&app, texts[0]));
    assert!(size_of(&app, texts[0]).x > half.x + 10.0, "the run's face arrived too: {half:?} then {:?}", size_of(&app, texts[0]));
}

#[test]
fn a_colour_change_on_a_file_text_whose_box_is_the_stand_in_measure_records_nothing() {
    let (mut app, row, texts) = opened_row(&["TestRefitW1"], &["TestRefitW1"]);
    // the file's box happens to be what this app measures with the stand-in
    let measure = stand_in_measure(&app, texts[0]);
    app.doc.get_mut(texts[0]).unwrap().size = measure;
    layout::layout_node(&mut app.doc, &mut Edit::new(), row);
    let before = (size_of(&app, texts[0]), size_of(&app, row));
    let red = Paint::solid(Color::rgb(200, 30, 30));
    app.edit_nodes(&[texts[0]], |n| {
        if let NodeKind::Text(t) = &mut n.kind {
            t.style.fills = vec![red.clone()];
        }
    });
    assert_eq!((size_of(&app, texts[0]), size_of(&app, row)), before, "a colour does not touch the box");
    arrive(&mut app);
    assert!(!fits(&app, texts[0]), "the face changes what this app measures, so a re-fit would show");
    assert_eq!((size_of(&app, texts[0]), size_of(&app, row)), before, "a recoloured file text keeps the box the file stored");
}

#[test]
fn a_colour_change_that_measures_the_text_again_is_fitted_when_the_face_arrives() {
    // the inspector applies a style and measures the text again, whatever the style changed
    let (mut app, _row, texts) = opened_row(&["TestRefitZ1"], &["TestRefitZ1"]);
    let red = Paint::solid(Color::rgb(200, 30, 30));
    crate::panels::typography::set_style(&mut app, &texts, |s| s.fills = vec![red.clone()]);
    let measured = size_of(&app, texts[0]);
    assert_ne!(measured, FILE_BOX, "the colour change measured the text with the stand-in");
    assert!(fits(&app, texts[0]));
    arrive(&mut app);
    assert!(fits(&app, texts[0]));
    assert!(size_of(&app, texts[0]).x > measured.x + 20.0, "the box this app measured is fitted: {measured:?} then {:?}", size_of(&app, texts[0]));
}

#[test]
fn the_colour_of_a_run_is_not_what_a_measure_reads() {
    let text = |fill: &Paint, run_fill: &Paint, content: &str, size: f32| TextProps {
        content: content.into(),
        style: TextStyle { fills: vec![fill.clone()], font_size: size, ..Default::default() },
        runs: vec![TextRun { start: 0, end: 2, style: TextStyle { fills: vec![run_fill.clone()], font_size: size, ..Default::default() } }],
        ..Default::default()
    };
    let (red, blue) = (Paint::solid(Color::rgb(200, 30, 30)), Paint::solid(Color::rgb(30, 30, 200)));
    let base = text(&red, &red, "abcd", 16.0);
    assert_eq!(measure_key(&base), measure_key(&text(&blue, &blue, "abcd", 16.0)), "paints, in the style and in the runs, are left out");
    assert_ne!(measure_key(&base), measure_key(&text(&red, &red, "abcde", 16.0)), "the characters are read");
    assert_ne!(measure_key(&base), measure_key(&text(&red, &red, "abcd", 18.0)), "the size is read");
}

#[test]
fn a_face_landing_during_a_gesture_does_not_re_measure_a_text_the_layout_resized() {
    let (mut app, _col, a, b) = column("TestRefitX1", "iiii iiii iiii iiii iiii iiii");
    type_a_phrase_in_a_session(&mut app, a);
    // the face lands while the text session holds the pass
    load_only(&mut app);
    assert!(app.text_refit_held());
    crate::canvas::textsession::finish_text(&mut app, true);
    // the heading is wider, so the column and the wrapped text beside it are wider: B itself was not edited
    let committed = size_of(&app, b);
    assert!(committed.x > 100.0, "the layout widened the wrapped text: {committed:?}");
    settle(&mut app);
    assert_eq!(size_of(&app, b), committed, "the box the file stores for B, at the width the layout gave it, is left alone");
}

#[test]
fn a_text_whose_box_this_app_never_measures_is_not_recorded() {
    let (mut app, _row, texts) = opened_row(&["TestRefitK2"], &["TestRefitK2"]);
    // a fixed-width text that truncates, in a row that gave it a new width: `fit_text` leaves its box as it is
    app.edit_nodes(&texts, |n| {
        if let NodeKind::Text(t) = &mut n.kind {
            t.auto_resize = TextAutoResize::Truncate;
        }
    });
    app.edit_nodes(&texts, |n| n.size.x += 8.0);
    assert!(app.fonts.refit.stand_in.is_empty(), "there is nothing in a truncated box for the pass to fit");
}

#[test]
fn a_parent_layout_that_widens_a_text_whose_box_is_the_stand_in_measure_records_nothing() {
    let (mut app, col, a, b) = column("TestRefitO1", "iiii");
    // the file's box for the text is exactly what this app measures with the stand-in, at any width
    let measure = match &app.doc.get(b).unwrap().kind {
        NodeKind::Text(t) => layout::fit_text(t, size_of(&app, b)),
        _ => unreachable!(),
    };
    app.doc.get_mut(b).unwrap().size.y = measure.y;
    layout::layout_node(&mut app.doc, &mut Edit::new(), col);
    let file = size_of(&app, b);
    // the user types into the heading while the face is missing: the column, and the text that fills it, get wider
    app.edit_nodes(&[a], |n| {
        if let NodeKind::Text(t) = &mut n.kind {
            t.content.push_str(" with a much longer title");
            n.size = layout::fit_text(t, n.size);
        }
    });
    let widened = size_of(&app, b);
    assert!(widened.x > file.x + 50.0 && widened.y == file.y, "the layout gave the text a width: {file:?} then {widened:?}");
    arrive(&mut app);
    assert!(!fits(&app, b), "the face changes what this app measures, so a re-fit would show");
    assert_eq!(size_of(&app, b), widened, "a text that only a layout resized keeps the box it had");
}

#[test]
fn a_slice_without_time_does_one_unit_of_work_texts_first_then_the_frames_that_hug_them() {
    let (mut app, rows, texts) = opened_rows(3, "TestRefitY1", &["TestRefitY1"]);
    for id in &texts {
        type_a_letter(&mut app, *id);
    }
    app.fonts.refit.budget = Duration::ZERO;
    // a unit is a text fitted, or the frame of a fitted text laid out
    let done = |app: &App| {
        let fitted = texts.iter().filter(|t| fits(app, **t)).count();
        let laid_out = rows.iter().zip(&texts).filter(|(r, t)| fits(app, **t) && size_of(app, **r).x == size_of(app, **t).x + 16.0).count();
        fitted + laid_out
    };
    load_only(&mut app);
    let mut units = done(&app);
    assert_eq!(units, 1, "the first slice had no time for more than one unit");
    let mut slices = 1;
    while app.text_refit_pending() {
        app.poll_fonts();
        slices += 1;
        let now = done(&app);
        assert!(now <= units + 1, "slice {slices} did {} units with no time", now - units);
        units = now;
        assert!(slices <= 2 * texts.len() + 1, "the pass does not end");
    }
    assert_eq!(units, 2 * texts.len(), "every text is fitted and every frame laid out in the end");
}
