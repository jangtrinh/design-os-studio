//! Text that something painted before it can sit under is left alone: a painted descendant of an
//! earlier frame that does not clip, or a stroke band of an ancestor. The audit then says nothing
//! and "Fix all" changes nothing, because what is really behind the text is not modelled. Text with
//! nothing painted behind it keeps its finding and its fix.

use super::*;
use designos_doc::{StrokeAlign, Vec2};

const FIX_LABEL: &str = "Change text to #767676 (4.54:1)";

/// White page. An earlier 4 x 4 frame that does not clip holds an 80 x 40 rectangle of `child`
/// colour at `child_at` in its space; a later frame at (100, 100) holds 14 px white text at (4, 10).
fn behind_unclipped_child(child_at: (f32, f32), child: Color) -> Fx {
    let mut f = fx();
    white_page(&mut f);
    let earlier = f.frame(f.page, "Earlier", None);
    resize(&mut f, earlier, 4.0, 4.0);
    place(&mut f, earlier, 0.0, 0.0);
    f.doc.get_mut(earlier).unwrap().kind.frame_props_mut().unwrap().clips_content = false;
    let r = f.rect(earlier, "Child", child);
    resize(&mut f, r, 80.0, 40.0);
    place(&mut f, r, child_at.0, child_at.1);
    let later = f.frame(f.page, "Later", None);
    resize(&mut f, later, 80.0, 40.0);
    place(&mut f, later, 100.0, 100.0);
    let t = f.text(later, "White text", 14.0, 400, Color::WHITE);
    resize(&mut f, t, 70.0, 17.0);
    place(&mut f, t, 4.0, 10.0);
    f
}

/// A 100 x 40 white card on a light grey page with a 30 px inside stroke of `stroke`, and 14 px
/// text of `color` at its top edge.
fn inside_stroke(color: Color, stroke: Color) -> Fx {
    let mut f = fx();
    f.doc.get_mut(f.page).unwrap().kind = NodeKind::Page { background: gray(0xED) };
    let card = f.frame(f.page, "Card", Some(Color::WHITE));
    resize(&mut f, card, 100.0, 40.0);
    place(&mut f, card, 40.0, 40.0);
    let n = f.doc.get_mut(card).unwrap();
    n.kind.frame_props_mut().unwrap().clips_content = false;
    n.stroke.paints = vec![Paint::solid(stroke)];
    n.stroke.weight = 30.0;
    n.stroke.align = StrokeAlign::Inside;
    let t = f.text(card, "Stroke text", 14.0, 400, color);
    resize(&mut f, t, 70.0, 17.0);
    place(&mut f, t, 10.0, 0.0);
    f
}

/// A white card with a 1 px light border and light grey caption well inside it.
fn bordered_card() -> Fx {
    let mut f = fx();
    f.doc.get_mut(f.page).unwrap().kind = NodeKind::Page { background: gray(0xED) };
    let card = f.frame(f.page, "Card", Some(Color::WHITE));
    resize(&mut f, card, 200.0, 80.0);
    place(&mut f, card, 40.0, 40.0);
    let n = f.doc.get_mut(card).unwrap();
    n.stroke.paints = vec![Paint::solid(Color::rgb(0xD4, 0xD4, 0xD8))];
    n.stroke.weight = 1.0;
    n.stroke.align = StrokeAlign::Inside;
    let t = f.text(card, "Light grey caption", 14.0, 400, gray(0xBB));
    resize(&mut f, t, 100.0, 17.0);
    place(&mut f, t, 16.0, 30.0);
    f
}

/// The text is skipped: no finding of any rule, "Fix all" lands nothing and leaves no undo step.
fn assert_left_alone(f: Fx, what: &str) {
    let mut app = f.app();
    let findings = audit(&app.doc, &Scope::Page(app.page)).findings;
    assert!(findings.is_empty(), "{what}: {findings:#?}");
    assert_eq!(app.check_designs_fix_all(&findings), 0, "{what}");
    assert!(!app.hist.can_undo(), "{what}: nothing was edited");
}

/// The text keeps exactly one contrast finding whose fix is the mildest recolour that passes.
fn assert_flagged_with_fix(f: Fx, what: &str) {
    let mut app = f.app();
    let findings = audit(&app.doc, &Scope::Page(app.page)).findings;
    let contrast: Vec<&Finding> = findings.iter().filter(|x| x.rule == Rule::Contrast).collect();
    assert_eq!(contrast.len(), 1, "{what}: {findings:#?}");
    assert_eq!(contrast[0].fix_label, FIX_LABEL, "{what}");
    assert_eq!(app.check_designs_fix_all(&findings), 1, "{what}");
    assert!(app.hist.can_undo(), "{what}");
}

#[test]
fn text_over_a_painted_child_of_a_small_earlier_frame_that_does_not_clip_is_not_checked() {
    assert_left_alone(behind_unclipped_child((100.0, 100.0), Color::BLACK), "black child");
    // A mid-grey child: a recolour of the white text would otherwise land on the colour it must not.
    assert_left_alone(behind_unclipped_child((100.0, 100.0), gray(0x80)), "grey child");
}

#[test]
fn text_over_a_stroke_band_of_its_ancestor_is_not_checked() {
    assert_left_alone(inside_stroke(gray(0x77), Color::BLACK), "black stroke, grey text");
    assert_left_alone(inside_stroke(Color::WHITE, gray(0x80)), "grey stroke, white text");
}

#[test]
fn a_child_that_does_not_reach_the_text_leaves_the_finding_and_its_fix() {
    assert_flagged_with_fix(behind_unclipped_child((400.0, 300.0), Color::BLACK), "far child");
}

#[test]
fn a_fully_transparent_child_over_the_text_paints_nothing_so_the_finding_stays() {
    assert_flagged_with_fix(behind_unclipped_child((100.0, 100.0), Color::rgba(0, 0, 0, 0)), "transparent child");
}

#[test]
fn a_thin_border_that_does_not_reach_the_text_leaves_the_finding_and_its_fix() {
    assert_flagged_with_fix(bordered_card(), "bordered card");
}

#[test]
fn a_stroke_band_hides_only_the_text_that_overlaps_it() {
    // A frame with no fill, 100 x 40 at (40, 40), a 10 px stroke, on a white page. 20 x 17 text of
    // 4.47:1 at the given x (y = 12, in the interior). Per alignment (inside, centred, outside): is
    // the text skipped?
    let cases: [(f32, [bool; 3]); 5] = [
        (40.0, [false, false, false]),  // in the interior, clear of every band
        (-5.0, [true, true, true]),     // across the left edge: every band is under it
        (97.0, [true, true, true]),     // across the right edge
        (105.0, [false, false, true]),  // just right of the edge: only an outside band reaches it
        (300.0, [false, false, false]), // nowhere near the frame or its stroke
    ];
    for (x, skipped) in cases {
        for (align, skipped) in [StrokeAlign::Inside, StrokeAlign::Center, StrokeAlign::Outside].into_iter().zip(skipped) {
            let mut f = fx();
            white_page(&mut f);
            let card = f.frame(f.page, "Card", None);
            resize(&mut f, card, 100.0, 40.0);
            place(&mut f, card, 40.0, 40.0);
            let n = f.doc.get_mut(card).unwrap();
            n.kind.frame_props_mut().unwrap().clips_content = false;
            n.stroke.paints = vec![Paint::solid(Color::BLACK)];
            n.stroke.weight = 10.0;
            n.stroke.align = align;
            let t = f.text(card, "Grey text", 14.0, 400, gray(0x77));
            resize(&mut f, t, 20.0, 17.0);
            place(&mut f, t, x, 12.0);
            let found = f.report().of(Rule::Contrast).count();
            assert_eq!(found, usize::from(!skipped), "{align:?} stroke, text at x = {x}: skipped = {skipped}");
        }
    }
}

#[test]
fn a_clipping_frame_cuts_the_children_before_they_reach_the_text() {
    for clips in [true, false] {
        let mut f = fx();
        white_page(&mut f);
        let earlier = f.frame(f.page, "Earlier", None);
        resize(&mut f, earlier, 4.0, 4.0);
        place(&mut f, earlier, 0.0, 0.0);
        f.doc.get_mut(earlier).unwrap().kind.frame_props_mut().unwrap().clips_content = false;
        let cut = f.frame(earlier, "Cut", None);
        resize(&mut f, cut, 10.0, 10.0);
        place(&mut f, cut, 500.0, 500.0);
        f.doc.get_mut(cut).unwrap().kind.frame_props_mut().unwrap().clips_content = clips;
        let big = f.rect(cut, "Big", Color::BLACK);
        f.doc.get_mut(big).unwrap().size = Vec2::new(1100.0, 1100.0);
        place(&mut f, big, -500.0, -500.0);
        let later = f.frame(f.page, "Later", None);
        resize(&mut f, later, 80.0, 40.0);
        place(&mut f, later, 100.0, 100.0);
        let t = f.text(later, "White", 14.0, 400, Color::WHITE);
        resize(&mut f, t, 40.0, 17.0);
        place(&mut f, t, 4.0, 10.0);
        let found = f.report().of(Rule::Contrast).count();
        assert_eq!(found, usize::from(clips), "clips_content = {clips}: the black rectangle reaches the text only when nothing clips it");
    }
}
