//! A re-fit that lands after a recorded edit closed. The edit's step stores the geometry it made with
//! the stand-in face. A face that arrives afterwards re-fits the text and lays out boxes the step never
//! touched, outside every step. Replaying the step as stored would put the text back and leave those
//! boxes as the re-fit sized them, so after a re-fit every earlier step undoes with the layout solve.

use super::*;

#[test]
fn a_refit_after_a_recorded_step_makes_its_undo_solve_layout_again() {
    let family = "TestRefitW2";
    // outer (hug) > row (hug width, fixed height, min width) > text
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let oid = d.alloc_id();
    let mut outer = Node::frame(oid, Rect::new(0.0, 0.0, 200.0, 60.0));
    if let NodeKind::Frame(fp) = &mut outer.kind {
        fp.auto_layout = Some(hugging(LayoutDirection::Horizontal, Sizing::Hug));
    }
    let outer = d.insert(page, outer).unwrap();
    let rid = d.alloc_id();
    let mut row = Node::frame(rid, Rect::new(0.0, 0.0, 100.0, 40.0));
    if let NodeKind::Frame(fp) = &mut row.kind {
        // a fixed height: only the width can climb past the row
        fp.auto_layout = Some(hugging(LayoutDirection::Horizontal, Sizing::Fixed));
    }
    let row = d.insert(outer, row).unwrap();
    let text = text_node(&mut d, row, family, NARROW, TextAutoResize::WidthAndHeight, FILE_BOX);
    // the row's min width sits above what the typed text measures with the stand-in, so typing does
    // not move the row and the edit's layout stops at it
    let typed_with_stand_in = match &d.get(text).unwrap().kind {
        NodeKind::Text(t) => {
            let mut t = t.clone();
            t.content.push('i');
            layout::fit_text(&t, FILE_BOX).x
        }
        _ => unreachable!(),
    };
    let min = (FILE_BOX.x + 16.0).max(typed_with_stand_in + 16.0) + 4.0;
    d.get_mut(row).unwrap().layout_child.min_w = Some(min);
    let mut app = App::new(d);
    layout::layout_node(&mut app.doc, &mut Edit::new(), outer);
    app.fonts = testkit::serving(&[family]);
    let opened = size_of(&app, outer);

    type_a_letter(&mut app, text);
    assert!(app.hist.peek_undo_layout_recorded(), "precondition: the typed step recorded its layout");
    assert_eq!(size_of(&app, row).x, min, "precondition: the stand-in measure leaves the row at its min width");
    assert_eq!(size_of(&app, outer), opened, "precondition: the step never reached the outer frame");
    let depth = app.hist.undo_depth();

    arrive(&mut app);
    let (fitted, widened_row, widened_outer) = (size_of(&app, text).x, size_of(&app, row).x, size_of(&app, outer).x);
    assert!(widened_row > min && widened_row == fitted + 16.0, "precondition: the face grew the text ({fitted}) and the row with it ({widened_row})");
    assert_eq!(widened_outer, widened_row + 16.0, "precondition: the re-fit climbed to the outer frame, which the step never touched");
    assert_eq!(app.hist.undo_depth(), depth, "re-fitting is not an edit: no undo step of its own");

    app.undo();
    for _ in 0..5 {
        app.poll_fonts();
    }
    while app.step_pending_relayout() {}
    let (text_w, row_w, outer_w) = (size_of(&app, text).x, size_of(&app, row).x, size_of(&app, outer).x);
    assert_eq!(text_w, FILE_BOX.x, "the typed letter is undone: the text is back to the box the file stores");
    assert_eq!(row_w, (text_w + 16.0).max(min), "after Cmd+Z the row hugs its text: text {text_w}, row {row_w}");
    assert_eq!(outer_w, row_w + 16.0, "after Cmd+Z the outer frame hugs the row: row {row_w}, outer {outer_w}");
}
