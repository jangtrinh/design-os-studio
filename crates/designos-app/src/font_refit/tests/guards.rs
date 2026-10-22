//! Two checks that only a layout or an undo between the steps of a pass reaches: a text that leaves
//! the queue is looked at again, and a text a layout resized while a gesture held the pass is not one
//! the edit measured.

use super::after_edits::type_a_phrase_in_a_session;
use super::*;

/// A column that hugs its width: a heading in Inter, then `count` texts of `family` with `content` in
/// `mode` that fill the width, each with the box a file stores for it.
fn filled_column(family: &str, content: &str, mode: TextAutoResize, count: usize) -> (App, NodeId, Vec<NodeId>) {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let cid = d.alloc_id();
    let mut col = Node::frame(cid, Rect::new(0.0, 0.0, 200.0, 100.0));
    if let NodeKind::Frame(fp) = &mut col.kind {
        fp.auto_layout = Some(hugging(LayoutDirection::Vertical, Sizing::Hug));
    }
    let col = d.insert(page, col).unwrap();
    let heading = text_node(&mut d, col, "Inter", "Heading", TextAutoResize::WidthAndHeight, Vec2::new(60.0, 19.0));
    let texts: Vec<NodeId> = (0..count)
        .map(|_| {
            let id = text_node(&mut d, col, family, content, mode, Vec2::new(60.0, 33.0));
            d.get_mut(id).unwrap().layout_child.horizontal = Sizing::Fill;
            id
        })
        .collect();
    let mut app = App::new(d);
    layout::layout_node(&mut app.doc, &mut Edit::new(), col);
    for id in &texts {
        app.doc.get_mut(*id).unwrap().size.y = 33.0;
    }
    layout::layout_node(&mut app.doc, &mut Edit::new(), col);
    app.fonts = testkit::serving(&[family]);
    (app, heading, texts)
}

#[test]
fn a_text_that_a_layout_resized_between_two_slices_is_left_alone() {
    let (mut app, heading, texts) = filled_column("TestRefitP1", "iiii iiii iiii iiii iiii iiii", TextAutoResize::Height, 2);
    let (first, second) = (texts[0], texts[1]);
    type_a_letter(&mut app, first);
    type_a_letter(&mut app, second);
    assert!(app.fonts.refit.stand_in.contains_key(&second), "the edit measured the text with the stand-in, so it is recorded");
    // a slice with no time fits one text: the first
    app.fonts.refit.budget = Duration::ZERO;
    load_only(&mut app);
    assert!(fits(&app, first) && !fits(&app, second), "the first slice fitted the first text and left the second queued");
    assert!(app.text_refit_pending());
    // between two slices the user types into the heading: the column, and the text that fills it, get wider
    let recorded = size_of(&app, second);
    app.edit_nodes(&[heading], |n| {
        if let NodeKind::Text(t) = &mut n.kind {
            t.content.push_str(" with a much longer title");
            n.size = layout::fit_text(t, n.size);
        }
    });
    let widened = size_of(&app, second);
    assert!(widened.x > recorded.x + 50.0 && widened.y == recorded.y, "the layout gave the text a width: {recorded:?} then {widened:?}");
    assert!(app.fonts.refit.stand_in.contains_key(&second), "the record is still there: a width a layout gave is not an edit of the text");
    settle(&mut app);
    assert!(!fits(&app, second), "the face changes what this app measures, so a re-fit would show");
    assert_eq!(size_of(&app, second), widened, "the text no longer holds the box its edit committed: it keeps the box it has");
}

#[test]
fn a_face_landing_during_a_gesture_does_not_re_measure_a_hugging_text_the_layout_resized() {
    let (mut app, heading, texts) = filled_column("TestRefitP2", "iiiiiiiiiiiiiiii", TextAutoResize::WidthAndHeight, 1);
    let id = texts[0];
    type_a_phrase_in_a_session(&mut app, heading);
    // the face lands while the text session holds the pass
    load_only(&mut app);
    assert!(app.text_refit_held());
    crate::canvas::textsession::finish_text(&mut app, true);
    // the heading is wider, so the column and the hugging text that fills it are wider: that text was not edited
    let committed = size_of(&app, id);
    assert!(committed.x > 100.0, "the layout widened the hugging text: {committed:?}");
    assert!(app.fonts.refit.stand_in.is_empty(), "nothing the edit measured is waiting for the face");
    settle(&mut app);
    assert!(!fits(&app, id), "the face changes what this app measures, so a re-fit would show");
    assert_eq!(size_of(&app, id), committed, "the box the file stores for it, at the width the layout gave it, is left alone");
}
