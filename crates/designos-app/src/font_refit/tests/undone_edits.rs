//! An edit that changes a text but leaves its box as it was, undone before the face arrives. The edit
//! committed the file's own box, so the box alone cannot tell the file's text from the edited one: the
//! record also keeps what the measure reads of the text.

use super::after_edits::{opened_rows, stand_in_measure};
use super::*;
use designos_doc::{TextAlignH, TextDecoration};

/// `count` file texts whose stored box is exactly what this app measures with the stand-in, each in a
/// hugging row of its own.
fn coincident_rows(family: &str, count: usize) -> (App, Vec<NodeId>, Vec<NodeId>) {
    let (mut app, rows, texts) = opened_rows(count, family, &[family]);
    for (row, id) in rows.iter().zip(&texts) {
        let measure = stand_in_measure(&app, *id);
        app.doc.get_mut(*id).unwrap().size = measure;
        layout::layout_node(&mut app.doc, &mut Edit::new(), *row);
    }
    (app, rows, texts)
}

fn coincident_row(family: &str) -> (App, NodeId, NodeId) {
    let (app, rows, texts) = coincident_rows(family, 1);
    (app, rows[0], texts[0])
}

/// The alignment shortcut outside a text session: sets it and measures the text again.
fn centre(app: &mut App, id: NodeId) {
    app.edit_nodes(&[id], |n| {
        if let NodeKind::Text(t) = &mut n.kind {
            t.style.align_h = TextAlignH::Center;
            n.size = layout::fit_text(t, n.size);
        }
    });
}

fn aligned_left(app: &App, id: NodeId) -> bool {
    matches!(&app.doc.get(id).unwrap().kind, NodeKind::Text(t) if t.style.align_h == TextAlignH::Left)
}

/// A wrapped text whose stored height is exactly the stand-in measure, outside any frame.
fn coincident_wrapped(family: &str) -> (App, NodeId) {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let id = text_node(&mut d, page, family, WRAPPED, TextAutoResize::Height, Vec2::new(120.0, 33.0));
    let mut app = App::new(d);
    app.fonts = testkit::serving(&[family]);
    let measure = match &app.doc.get(id).unwrap().kind {
        NodeKind::Text(t) => layout::fit_text(t, Vec2::new(120.0, 33.0)),
        _ => unreachable!(),
    };
    app.doc.get_mut(id).unwrap().size = measure;
    (app, id)
}

#[test]
fn an_alignment_undone_before_the_face_arrives_leaves_the_files_box() {
    let (mut app, row, id) = coincident_row("TestRefitM1");
    let file = (size_of(&app, id), size_of(&app, row));
    centre(&mut app, id);
    assert_eq!(size_of(&app, id), file.0, "the alignment left the box as it was");
    assert!(app.fonts.refit.stand_in.contains_key(&id), "the edit measured the text with the stand-in, so it is recorded");
    app.undo();
    assert!(aligned_left(&app, id), "the undo brought the alignment back");
    assert_eq!((size_of(&app, id), size_of(&app, row)), file);
    arrive(&mut app);
    assert_eq!((size_of(&app, id), size_of(&app, row)), file, "nobody edited this text since the file: its box is the file's");
    assert!(!fits(&app, id), "the face changes what this app measures, so a re-fit would show");
    assert!(app.hist.can_redo(), "and the redo is still there");
}

#[test]
fn an_underline_undone_before_the_face_arrives_leaves_the_files_box() {
    let (mut app, row, id) = coincident_row("TestRefitN1");
    let file = (size_of(&app, id), size_of(&app, row));
    crate::panels::typography::set_style(&mut app, &[id], |s| s.decoration = TextDecoration::Underline);
    assert_eq!(size_of(&app, id), file.0, "the underline left the box as it was");
    assert!(app.fonts.refit.stand_in.contains_key(&id), "the edit measured the text with the stand-in, so it is recorded");
    app.undo();
    assert_eq!((size_of(&app, id), size_of(&app, row)), file);
    arrive(&mut app);
    assert_eq!((size_of(&app, id), size_of(&app, row)), file);
    assert!(!fits(&app, id), "the face changes what this app measures, so a re-fit would show");
}

#[test]
fn a_letter_typed_into_a_wrapped_text_without_a_new_line_and_undone_leaves_the_files_box() {
    let (mut app, id) = coincident_wrapped("TestRefitN2");
    let file = size_of(&app, id);
    type_a_letter(&mut app, id);
    assert_eq!(size_of(&app, id), file, "the letter did not add a line, so the box is the file's");
    assert!(app.fonts.refit.stand_in.contains_key(&id), "the edit measured the text with the stand-in, so it is recorded");
    app.undo();
    assert!(matches!(&app.doc.get(id).unwrap().kind, NodeKind::Text(t) if t.content == WRAPPED), "the undo brought the characters back");
    assert_eq!(size_of(&app, id), file);
    arrive(&mut app);
    assert_eq!(size_of(&app, id), file, "the box the file stores is exactly what it was");
    assert!(!fits(&app, id), "the face changes what this app measures, so a re-fit would show");
}

#[test]
fn an_alignment_that_stays_is_fitted_when_the_face_arrives() {
    let (mut app, _row, id) = coincident_row("TestRefitM2");
    centre(&mut app, id);
    let measured = size_of(&app, id);
    arrive(&mut app);
    assert!(fits(&app, id), "the user's alignment was measured with the stand-in, and the face has arrived");
    assert!(size_of(&app, id).x > measured.x + 20.0, "{measured:?} then {:?}", size_of(&app, id));
}

#[test]
fn redoing_an_alignment_before_the_face_arrives_is_fitted_when_it_does() {
    let (mut app, _row, id) = coincident_row("TestRefitM3");
    centre(&mut app, id);
    let measured = size_of(&app, id);
    app.undo();
    app.redo();
    assert!(!aligned_left(&app, id), "the redo brought the alignment back");
    arrive(&mut app);
    assert!(fits(&app, id));
    assert!(size_of(&app, id).x > measured.x + 20.0, "the redo brought the record's box and text back: {measured:?} then {:?}", size_of(&app, id));
}

#[test]
fn an_alignment_undone_between_two_slices_leaves_the_files_box() {
    let (mut app, rows, texts) = coincident_rows("TestRefitQ1", 2);
    let file = (size_of(&app, texts[1]), size_of(&app, rows[1]));
    centre(&mut app, texts[0]);
    centre(&mut app, texts[1]);
    // a slice with no time fits one text: the first
    app.fonts.refit.budget = Duration::ZERO;
    load_only(&mut app);
    assert!(fits(&app, texts[0]) && !fits(&app, texts[1]), "the first slice fitted the first text and left the second queued");
    assert!(app.text_refit_pending());
    // the user undoes the last alignment while the pass is still running
    app.undo();
    assert!(aligned_left(&app, texts[1]), "the undo brought the second text's alignment back");
    settle(&mut app);
    assert_eq!((size_of(&app, texts[1]), size_of(&app, rows[1])), file, "the second text is the file's again: its box is the file's");
    assert!(!fits(&app, texts[1]), "the face changes what this app measures, so a re-fit would show");
}
