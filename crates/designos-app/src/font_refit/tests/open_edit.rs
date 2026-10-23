//! A re-fit that lands while a user edit is still open. The colour picker keeps its transaction open
//! across frames and does not hold the pass, so the pass can lay out boxes the open edit already
//! touched. Those writes are made outside every history step, and the edit's "before" values predate
//! them: its step must not replay them as stored.

use super::*;

#[test]
fn a_refit_that_lands_while_an_edit_is_open_leaves_that_edit_unrecorded_and_its_undo_consistent() {
    let (mut app, row, texts) = opened_row(&["TestRefitR9"], &["TestRefitR9"]);
    type_a_letter(&mut app, texts[0]);
    let typed = size_of(&app, texts[0]);
    let name = app.doc.get(row).unwrap().name.clone();
    // an edit of the row that stays open across frames, as the colour picker's does
    let mut tx = Tx::new();
    tx.ed.node(&mut app.doc, row).unwrap().name = "renamed while the face loads".into();
    assert!(!app.text_refit_held(), "precondition: an open edit that is not a gesture does not hold the pass");
    arrive(&mut app);
    let fitted = size_of(&app, texts[0]);
    assert!(fitted.x > typed.x + 20.0, "precondition: the face arrived and the text was fitted to it ({typed:?} -> {fitted:?})");
    assert_eq!(size_of(&app, row).x, fitted.x + 16.0, "precondition: the pass laid the row out around the fitted text");

    assert!(app.run_tx(tx));
    let recorded = app.hist.peek_undo_layout_recorded();
    app.undo();
    while app.step_pending_relayout() {}
    let (text_w, row_w) = (size_of(&app, texts[0]).x, size_of(&app, row).x);
    assert_eq!(app.doc.get(row).unwrap().name, name, "the rename is undone");
    assert_eq!(row_w, text_w + 16.0, "after Cmd+Z the row still hugs its text: text {text_w}, row {row_w}, step recorded {recorded}");
    assert!(!recorded, "the re-fit wrote layout while the edit was open, so the edit's step is not recorded");
}
