//! What the contrast rule treats as "behind the text": an ancestor fill counts only where it is
//! really painted under the text, and a blend mode anywhere above the text means the colour on
//! screen is not the plain composite the rule computes, so nothing is reported.

use super::*;
use designos_doc::{Affine, BlendMode};

// ---------------------------------------------------------------- painted bounds

#[test]
fn a_fill_that_stops_short_of_the_text_is_not_what_the_text_is_on() {
    let mut f = fx();
    white_page(&mut f);
    let dot = f.frame(f.page, "Dot", Some(Color::BLACK));
    resize(&mut f, dot, 10.0, 10.0);
    // Nothing clips the frame, so its child paints far outside the 10 x 10 fill, on the white page.
    let t = f.text(dot, "Far away", 14.0, 400, Color::BLACK);
    place(&mut f, t, 500.0, 500.0);
    assert!(f.report().findings.is_empty(), "black text on a white page is 21:1: {:#?}", f.report().findings);
}

#[test]
fn the_fix_never_recolours_text_against_a_fill_that_is_not_under_it() {
    let mut f = fx();
    white_page(&mut f);
    let dot = f.frame(f.page, "Dot", Some(Color::BLACK));
    resize(&mut f, dot, 10.0, 10.0);
    let t = f.text(dot, "Far away", 14.0, 400, Color::BLACK);
    place(&mut f, t, 500.0, 500.0);
    let before = f.doc.get(t).cloned();
    let mut app = f.app();
    assert_eq!(app.check_designs_fix_all(&audit(&app.doc, &Scope::Page(app.page)).findings), 0);
    assert_eq!(app.doc.get(t).cloned(), before, "the text stays black on white");
    assert!(!app.hist.can_undo());
}

#[test]
fn an_outer_fill_still_counts_when_an_inner_one_does_not_reach_the_text() {
    let mut f = fx();
    let outer = f.frame(f.page, "Outer", Some(Color::WHITE));
    let dot = f.frame(outer, "Dark dot", Some(Color::BLACK));
    resize(&mut f, dot, 10.0, 10.0);
    // Inside the outer 200 x 120 white frame, outside the dark dot: #777777 on white is 4.47:1.
    let t = f.text(dot, "Beside the dot", 14.0, 400, gray(0x77));
    place(&mut f, t, 100.0, 50.0);
    let found = only(&f.report(), Rule::Contrast);
    assert_eq!(found.node, t);
    assert_eq!(found.message, "Contrast 4.47:1 is below 4.5:1 (WCAG AA)");
}

#[test]
fn text_half_over_a_fill_is_not_checked_because_its_backdrop_changes_under_it() {
    let mut f = fx();
    white_page(&mut f);
    let dot = f.frame(f.page, "Dot", Some(Color::BLACK));
    resize(&mut f, dot, 10.0, 10.0);
    let t = f.text(dot, "Half on, half off", 14.0, 400, Color::BLACK);
    place(&mut f, t, 5.0, 5.0);
    assert!(f.report().findings.is_empty(), "{:#?}", f.report().findings);
}

#[test]
fn text_fully_inside_a_fill_is_still_checked_against_it() {
    let mut f = fx();
    let frame = f.frame(f.page, "Card", Some(Color::WHITE));
    let t = f.text(frame, "Low", 14.0, 400, gray(0x77));
    assert_eq!(only(&f.report(), Rule::Contrast).node, t);
}

#[test]
fn text_flush_with_the_edge_of_a_fill_is_still_on_it_through_a_rotated_frame() {
    let mut f = fx();
    let outer = f.frame(f.page, "Card", Some(Color::WHITE));
    resize(&mut f, outer, 100.0, 100.0);
    // A quarter turn leaves float noise on the corners; text that fills the card is still on its fill.
    let turned = f.frame(outer, "Turned", None);
    resize(&mut f, turned, 100.0, 100.0);
    f.doc.get_mut(turned).unwrap().transform = Affine::translate(100.0, 0.0) * Affine::rotate(std::f32::consts::FRAC_PI_2);
    let t = f.text(turned, "Flush", 14.0, 400, gray(0x77));
    resize(&mut f, t, 100.0, 100.0);
    place(&mut f, t, 0.0, 0.0);
    assert_eq!(only(&f.report(), Rule::Contrast).node, t);
}

#[test]
fn a_rotated_frame_paints_its_rotated_shape_not_its_bounding_box() {
    let mut f = fx();
    white_page(&mut f);
    let diamond = f.frame(f.page, "Diamond", Some(Color::BLACK));
    resize(&mut f, diamond, 100.0, 100.0);
    f.doc.get_mut(diamond).unwrap().transform = Affine::rotate(std::f32::consts::FRAC_PI_4);
    // Local (-30, 60) is left of the frame, yet inside the page-space box of the rotated shape.
    let t = f.text(diamond, "Off the corner", 14.0, 400, Color::BLACK);
    resize(&mut f, t, 10.0, 10.0);
    place(&mut f, t, -30.0, 60.0);
    assert!(f.report().findings.is_empty(), "{:#?}", f.report().findings);
    // The same text well inside the shape is on black and is reported.
    place(&mut f, t, 40.0, 40.0);
    assert_eq!(only(&f.report(), Rule::Contrast).node, t);
}

// ---------------------------------------------------------------- clipping frames

fn clip(f: &mut Fx, id: NodeId) {
    f.doc.get_mut(id).unwrap().kind.frame_props_mut().expect("a frame").clips_content = true;
}

#[test]
fn text_overflowing_a_clipping_frame_is_scored_on_the_frame_fill_it_shows_on() {
    let mut f = fx();
    let card = f.frame(f.page, "Card", Some(Color::WHITE));
    clip(&mut f, card);
    // 50 px of the 100 px wide text hang outside the card, where the card clips them away.
    let t = f.text(card, "Overflowing label", 14.0, 400, gray(0x77));
    place(&mut f, t, 150.0, 8.0);
    let found = only(&f.report(), Rule::Contrast);
    assert_eq!(found.node, t);
    assert_eq!(found.message, "Contrast 4.47:1 is below 4.5:1 (WCAG AA)");
}

#[test]
fn text_entirely_outside_a_clipping_frame_is_not_painted_so_it_is_not_checked() {
    let mut f = fx();
    let card = f.frame(f.page, "Card", Some(Color::WHITE));
    clip(&mut f, card);
    let t = f.text(card, "Clipped away", 14.0, 400, gray(0x77));
    place(&mut f, t, 500.0, 500.0);
    assert!(f.report().findings.is_empty(), "{:#?}", f.report().findings);
}

#[test]
fn only_the_visible_part_of_overflowing_text_has_to_sit_on_the_fills_around_it() {
    let mut f = fx();
    let outer = f.frame(f.page, "Outer", Some(Color::WHITE));
    let dark = f.frame(outer, "Dark chip", Some(gray(0x10)));
    resize(&mut f, dark, 50.0, 50.0);
    clip(&mut f, dark);
    // Visible only over the chip (x 10..50): #202020 on #101010 fails, whatever the white card is.
    let t = f.text(dark, "Chip label", 14.0, 400, gray(0x20));
    place(&mut f, t, 10.0, 10.0);
    assert_eq!(only(&f.report(), Rule::Contrast).node, t);
}

// ---------------------------------------------------------------- blend modes

#[test]
fn text_in_a_frame_blended_with_the_page_is_not_checked() {
    let mut f = fx();
    let frame = f.frame(f.page, "Multiplied", Some(Color::WHITE));
    f.doc.get_mut(frame).unwrap().blend = BlendMode::Multiply;
    f.text(frame, "Low", 14.0, 400, gray(0x77));
    assert!(f.report().findings.is_empty(), "the frame is multiplied with the page: {:#?}", f.report().findings);
}

#[test]
fn a_blend_mode_on_any_ancestor_or_on_the_text_itself_skips_the_text() {
    let mut f = fx();
    let outer = f.frame(f.page, "Outer", None);
    f.doc.get_mut(outer).unwrap().blend = BlendMode::Screen;
    let inner = f.frame(outer, "Inner", Some(Color::WHITE));
    f.text(inner, "Low", 14.0, 400, gray(0x77));
    assert!(f.report().findings.is_empty(), "the group above the text is blended: {:#?}", f.report().findings);
    f.doc.get_mut(outer).unwrap().blend = BlendMode::Normal;
    let t = f.text(inner, "Also low", 14.0, 400, gray(0x77));
    assert_eq!(f.report().of(Rule::Contrast).count(), 2);
    f.doc.get_mut(t).unwrap().blend = BlendMode::Overlay;
    let still: Vec<_> = f.report().findings.iter().map(|x| x.node).collect();
    assert!(!still.contains(&t), "a blended text layer is not composited as plain colour");
    assert_eq!(still.len(), 1, "the sibling without a blend is still checked");
}

#[test]
fn normal_and_pass_through_blending_are_checked_like_no_blend() {
    for mode in [BlendMode::Normal, BlendMode::PassThrough] {
        let mut f = fx();
        let frame = f.frame(f.page, "Card", Some(Color::WHITE));
        f.doc.get_mut(frame).unwrap().blend = mode;
        let t = f.text(frame, "Low", 14.0, 400, gray(0x77));
        assert_eq!(only(&f.report(), Rule::Contrast).node, t, "{mode:?}");
    }
}
