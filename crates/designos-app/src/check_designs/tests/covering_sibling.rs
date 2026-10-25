//! A plain opaque rectangle or frame painted before the text and covering all of it is the text's
//! backdrop: a button made of a rectangle and a label is checked against the rectangle. Anything
//! that makes the rectangle not the whole story (rotated, partial cover, translucent, an effect, a
//! stroke under the text, a mask, a painted child over the text, a nearer layer in between) leaves
//! the backdrop unknown and the text unchecked.

use super::*;
use designos_doc::{Affine, BlendMode, GradientStop, StrokeAlign, Vec2};

const FIX_LABEL: &str = "Change text to #767676 (4.54:1)";

/// A page of `page` colour, a 120 x 40 rectangle of `fill` at (40, 40), and 14 px text of `text`
/// colour (70 x 17 at (60, 50)) painted after it, wholly within it.
fn button(page: Color, fill: Color, text: Color) -> (Fx, NodeId, NodeId) {
    let mut f = fx();
    f.doc.get_mut(f.page).unwrap().kind = NodeKind::Page { background: page };
    let r = f.rect(f.page, "Button", fill);
    resize(&mut f, r, 120.0, 40.0);
    place(&mut f, r, 40.0, 40.0);
    let t = f.text(f.page, "Label", 14.0, 400, text);
    resize(&mut f, t, 70.0, 17.0);
    place(&mut f, t, 60.0, 50.0);
    (f, r, t)
}

/// The contrast findings of the page, and what "Fix all" lands.
fn audited(f: Fx) -> (Vec<Finding>, usize) {
    let mut app = f.app();
    let findings: Vec<Finding> = audit(&app.doc, &Scope::Page(app.page)).findings.into_iter().filter(|x| x.rule == Rule::Contrast).collect();
    let fixed = app.check_designs_fix_all(&findings);
    (findings, fixed)
}

fn counts(f: Fx) -> (usize, usize) {
    let (findings, fixed) = audited(f);
    (findings.len(), fixed)
}

fn edit(f: &mut Fx, id: NodeId, change: impl FnOnce(&mut Node)) {
    change(f.doc.get_mut(id).unwrap());
}

#[test]
fn a_light_label_on_a_white_rectangle_is_flagged_with_a_fix() {
    let (f, _, _) = button(Color::WHITE, Color::WHITE, gray(0xBB));
    let (findings, fixed) = audited(f);
    assert_eq!(findings.len(), 1, "{findings:#?}");
    assert_eq!(findings[0].fix_label, FIX_LABEL);
    assert_eq!(fixed, 1);
}

#[test]
fn the_rectangle_and_not_the_page_is_the_backdrop() {
    // Grey on a slightly lighter grey rectangle on a white page: 1.13:1 against the rectangle, 4.47:1
    // against the page.
    let (f, _, _) = button(Color::WHITE, gray(0x80), gray(0x77));
    let (findings, fixed) = audited(f);
    assert_eq!(findings.len(), 1, "{findings:#?}");
    assert!(findings[0].message.starts_with("Contrast 1.13:1"), "{}", findings[0].message);
    assert_eq!(fixed, 1);

    // The page passes and the rectangle does not.
    let (f, _, _) = button(gray(0x20), Color::WHITE, gray(0xBB));
    assert_eq!(counts(f), (1, 1), "light grey text on a white rectangle on a dark page");

    // Dark text on a dark rectangle on a white page passes against the page only.
    let (f, _, _) = button(Color::WHITE, gray(0x11), gray(0x22));
    assert_eq!(counts(f), (1, 1));

    // White text on a dark rectangle on a white page fails against the page only.
    let (f, _, _) = button(Color::WHITE, gray(0x11), Color::WHITE);
    assert_eq!(counts(f), (0, 0));
}

#[test]
fn the_nearest_layer_that_reaches_the_text_decides() {
    // Black rectangle first, white rectangle over it: the white one is the backdrop.
    let (mut f, _, _) = button(Color::WHITE, Color::WHITE, gray(0xBB));
    let below = f.rect(f.page, "Below", Color::BLACK);
    resize(&mut f, below, 200.0, 100.0);
    place(&mut f, below, 0.0, 0.0);
    f.doc.get_mut(f.page).unwrap().children.rotate_right(1);
    assert_eq!(counts(f), (1, 1), "the black rectangle is hidden by the white one");

    // White rectangle first, black rectangle over it: the black one is the backdrop.
    let (mut f, _, _) = button(Color::WHITE, Color::WHITE, gray(0xBB));
    let above = f.rect(f.page, "Above", Color::BLACK);
    resize(&mut f, above, 200.0, 100.0);
    place(&mut f, above, 0.0, 0.0);
    let n = f.doc.get_mut(f.page).unwrap();
    let last = n.children.pop().unwrap();
    n.children.insert(1, last);
    assert_eq!(counts(f), (0, 0), "light grey on black passes");
}

#[test]
fn a_frame_with_an_opaque_fill_and_no_child_over_the_text_is_the_backdrop() {
    let mut f = fx();
    white_page(&mut f);
    let card = f.frame(f.page, "Card", Some(Color::WHITE));
    resize(&mut f, card, 200.0, 100.0);
    place(&mut f, card, 40.0, 40.0);
    let t = f.text(f.page, "Label", 14.0, 400, gray(0xBB));
    resize(&mut f, t, 70.0, 17.0);
    place(&mut f, t, 60.0, 50.0);
    assert_eq!(counts(f), (1, 1));
}

#[test]
fn a_rectangle_inside_a_card_covers_what_is_below_it_in_the_card() {
    let mut f = fx();
    white_page(&mut f);
    let card = f.frame(f.page, "Card", Some(Color::BLACK));
    resize(&mut f, card, 300.0, 200.0);
    place(&mut f, card, 0.0, 0.0);
    let r = f.rect(card, "Button", Color::WHITE);
    resize(&mut f, r, 120.0, 40.0);
    place(&mut f, r, 40.0, 40.0);
    let t = f.text(card, "Label", 14.0, 400, gray(0xBB));
    resize(&mut f, t, 70.0, 17.0);
    place(&mut f, t, 60.0, 50.0);
    assert_eq!(counts(f), (1, 1), "light grey on the white button, not on the black card");
}

#[test]
fn a_group_with_opacity_still_shows_what_is_below_it_through_the_covering_rectangle() {
    // The card is half transparent, so a layer below the card shows through its white rectangle.
    let build = |earlier: bool| {
        let mut f = fx();
        white_page(&mut f);
        if earlier {
            let under = f.rect(f.page, "Under", Color::BLACK);
            resize(&mut f, under, 300.0, 200.0);
            place(&mut f, under, 0.0, 0.0);
        }
        let card = f.frame(f.page, "Card", None);
        resize(&mut f, card, 300.0, 200.0);
        place(&mut f, card, 0.0, 0.0);
        f.doc.get_mut(card).unwrap().opacity = 0.5;
        let r = f.rect(card, "Button", Color::WHITE);
        resize(&mut f, r, 120.0, 40.0);
        place(&mut f, r, 40.0, 40.0);
        let t = f.text(card, "Label", 14.0, 400, gray(0xBB));
        resize(&mut f, t, 70.0, 17.0);
        place(&mut f, t, 60.0, 50.0);
        f
    };
    assert_eq!(counts(build(true)), (0, 0), "the black rectangle below the card shows through");
    // Nothing below: the backdrop is the white rectangle at half opacity over the white page.
    // The half transparent text cannot be recoloured to pass, so there is a finding and no fix.
    assert_eq!(counts(build(false)), (1, 0));
}

#[test]
fn a_rotated_rectangle_leaves_the_backdrop_unknown() {
    let (mut f, r, _) = button(Color::WHITE, Color::WHITE, gray(0xBB));
    resize(&mut f, r, 200.0, 200.0);
    place(&mut f, r, 0.0, 0.0);
    // Turned about its centre (100, 100): the text at (60..130, 50..67) stays on the rectangle.
    edit(&mut f, r, |n| n.transform = Affine::translate(100.0, 100.0) * Affine::rotate(0.2) * Affine::translate(-100.0, -100.0));
    assert_eq!(counts(f), (0, 0));
}

#[test]
fn a_rectangle_that_covers_only_part_of_the_text_leaves_the_backdrop_unknown() {
    let (mut f, r, _) = button(Color::WHITE, Color::WHITE, gray(0xBB));
    resize(&mut f, r, 80.0, 40.0);
    assert_eq!(counts(f), (0, 0), "the rectangle ends at x = 120, the text at x = 130");
}

#[test]
fn a_rounded_corner_cutout_leaves_the_backdrop_unknown() {
    let (mut f, r, t) = button(Color::WHITE, Color::WHITE, gray(0xBB));
    edit(&mut f, r, |n| n.corner_radii = [20.0; 4]);
    // The label's top left sits in the cutout of the top left corner (box 40..160, 40..80).
    place(&mut f, t, 42.0, 42.0);
    assert_eq!(counts(f), (0, 0));
}

#[test]
fn a_translucent_rectangle_leaves_the_backdrop_unknown() {
    for (what, change) in [
        ("layer opacity", (|n: &mut Node| n.opacity = 0.9) as fn(&mut Node)),
        ("paint opacity", |n| n.fills[0].opacity = 0.9),
        ("fill alpha", |n| n.fills[0].kind = PaintKind::Solid(Color::rgba(255, 255, 255, 230))),
    ] {
        let (mut f, r, _) = button(Color::WHITE, Color::WHITE, gray(0xBB));
        edit(&mut f, r, change);
        assert_eq!(counts(f), (0, 0), "{what}");
    }
}

#[test]
fn a_rectangle_that_is_not_painted_normally_leaves_the_backdrop_unknown() {
    for (what, change) in [
        ("multiply blend", (|n: &mut Node| n.blend = BlendMode::Multiply) as fn(&mut Node)),
        ("gradient fill", |n| {
            n.fills = vec![Paint {
                kind: PaintKind::LinearGradient {
                    handles: [Vec2::new(0.0, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.0, 1.0)],
                    stops: vec![GradientStop { pos: 0.0, color: Color::WHITE }, GradientStop { pos: 1.0, color: Color::WHITE }],
                },
                ..Paint::solid(Color::WHITE)
            }]
        }),
        ("fill blend", |n| n.fills[0].blend = BlendMode::Multiply),
        ("drop shadow", |n| n.effects = vec![shadow(8.0)]),
        ("mask", |n| n.is_mask = true),
    ] {
        let (mut f, r, _) = button(Color::WHITE, Color::WHITE, gray(0xBB));
        edit(&mut f, r, change);
        assert_eq!(counts(f), (0, 0), "{what}");
    }
}

#[test]
fn a_mask_before_the_rectangle_leaves_the_backdrop_unknown() {
    let (mut f, _, _) = button(Color::WHITE, Color::WHITE, gray(0xBB));
    let mask = f.rect(f.page, "Mask", Color::BLACK);
    resize(&mut f, mask, 10.0, 10.0);
    place(&mut f, mask, 400.0, 400.0);
    edit(&mut f, mask, |n| n.is_mask = true);
    f.doc.get_mut(f.page).unwrap().children.rotate_right(1);
    assert_eq!(counts(f), (0, 0));
}

#[test]
fn a_stroke_under_the_text_leaves_the_backdrop_unknown() {
    let (mut f, r, _) = button(Color::WHITE, Color::WHITE, gray(0xBB));
    edit(&mut f, r, |n| {
        n.stroke.paints = vec![Paint::solid(Color::BLACK)];
        n.stroke.weight = 30.0;
        n.stroke.align = StrokeAlign::Inside;
    });
    assert_eq!(counts(f), (0, 0));
}

#[test]
fn a_painted_child_of_a_covering_frame_over_the_text_leaves_the_backdrop_unknown() {
    let mut f = fx();
    white_page(&mut f);
    let card = f.frame(f.page, "Card", Some(Color::WHITE));
    resize(&mut f, card, 200.0, 100.0);
    place(&mut f, card, 40.0, 40.0);
    let chip = f.rect(card, "Chip", gray(0x80));
    resize(&mut f, chip, 100.0, 40.0);
    place(&mut f, chip, 10.0, 5.0);
    let t = f.text(f.page, "Label", 14.0, 400, gray(0x77));
    resize(&mut f, t, 70.0, 17.0);
    place(&mut f, t, 60.0, 50.0);
    assert_eq!(counts(f), (0, 0));
}

#[test]
fn a_nearer_translucent_layer_leaves_the_backdrop_unknown() {
    let (mut f, _, _) = button(Color::WHITE, Color::WHITE, gray(0xBB));
    let veil = f.rect(f.page, "Veil", Color::rgba(0, 0, 0, 128));
    resize(&mut f, veil, 20.0, 20.0);
    place(&mut f, veil, 100.0, 45.0);
    let n = f.doc.get_mut(f.page).unwrap();
    let last = n.children.pop().unwrap();
    n.children.insert(1, last);
    assert_eq!(counts(f), (0, 0));
}
