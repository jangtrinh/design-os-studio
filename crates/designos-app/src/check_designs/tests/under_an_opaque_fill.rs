//! Whatever an earlier layer paints below an ancestor's opaque fill cannot show through it, so it
//! is no reason to skip text that sits on that fill: a card list whose cards cast shadows is
//! checked. Layers painted above the fill, and fills that are not opaque, still make the backdrop
//! unknown.

use super::*;
use designos_doc::{Effect, StrokeAlign, Vec2};

/// White page; Card A (200 x 100 at (40, 40), white fill) with `decorate`, and Card B 16 px below it
/// (200 x 100, filled with `fill`).
fn cards(decorate: impl FnOnce(&mut Node), fill: Option<Color>) -> (Fx, NodeId) {
    let mut f = fx();
    white_page(&mut f);
    let a = f.frame(f.page, "Card A", Some(Color::WHITE));
    resize(&mut f, a, 200.0, 100.0);
    place(&mut f, a, 40.0, 40.0);
    decorate(f.doc.get_mut(a).unwrap());
    let b = f.frame(f.page, "Card B", fill);
    resize(&mut f, b, 200.0, 100.0);
    place(&mut f, b, 40.0, 156.0);
    (f, b)
}

/// Grey 4.47:1 text 24 px inside the top edge of `parent` (Card B), so 40 px from Card A's bottom.
fn grey_text(f: &mut Fx, parent: NodeId) {
    let t = f.text(parent, "Grey text", 14.0, 400, gray(0x77));
    resize(f, t, 40.0, 16.0);
    place(f, t, 16.0, 24.0);
}

/// The card list with the text directly in Card B.
fn card_list(decorate: impl FnOnce(&mut Node), fill: Option<Color>) -> (Fx, NodeId) {
    let (mut f, b) = cards(decorate, fill);
    grey_text(&mut f, b);
    (f, b)
}

fn shadow(n: &mut Node) {
    n.effects =
        vec![Effect::DropShadow { color: Color::rgba(0, 0, 0, 51), offset: Vec2::new(0.0, 4.0), radius: 24.0, spread: 0.0, visible: true, show_behind: false }];
}

fn outside_stroke(n: &mut Node) {
    n.stroke.paints = vec![Paint::solid(Color::BLACK)];
    n.stroke.weight = 60.0;
    n.stroke.align = StrokeAlign::Outside;
}

fn blur(n: &mut Node) {
    n.effects = vec![Effect::LayerBlur { radius: 30.0, visible: true }];
}

/// How many contrast findings, and what "Fix all" lands.
fn audited(f: Fx) -> (usize, usize) {
    let mut app = f.app();
    let findings = audit(&app.doc, &Scope::Page(app.page)).findings;
    let contrast = findings.iter().filter(|x| x.rule == Rule::Contrast).count();
    (contrast, app.check_designs_fix_all(&findings))
}

#[test]
fn the_shadow_of_an_earlier_card_below_an_opaque_card_does_not_hide_the_text_on_it() {
    let (f, _) = card_list(shadow, Some(Color::WHITE));
    assert_eq!(audited(f), (1, 1));
}

#[test]
fn the_stroke_of_an_earlier_card_below_an_opaque_card_does_not_hide_the_text_on_it() {
    let (f, _) = card_list(outside_stroke, Some(Color::WHITE));
    assert_eq!(audited(f), (1, 1));
}

#[test]
fn the_blur_of_an_earlier_card_below_an_opaque_card_does_not_hide_the_text_on_it() {
    let (f, _) = card_list(blur, Some(Color::WHITE));
    assert_eq!(audited(f), (1, 1));
}

#[test]
fn text_in_a_frame_inside_the_opaque_card_is_found_too() {
    let (mut f, b) = cards(shadow, Some(Color::WHITE));
    let inner = f.frame(b, "Content", None);
    resize(&mut f, inner, 200.0, 100.0);
    place(&mut f, inner, 0.0, 0.0);
    f.doc.get_mut(inner).unwrap().kind.frame_props_mut().unwrap().clips_content = false;
    grey_text(&mut f, inner);
    assert_eq!(audited(f), (1, 1));
}

#[test]
fn a_translucent_layer_between_the_text_and_the_opaque_card_keeps_the_old_behaviour() {
    let (mut f, b) = cards(shadow, Some(Color::WHITE));
    let inner = f.frame(b, "Content", None);
    resize(&mut f, inner, 200.0, 100.0);
    place(&mut f, inner, 0.0, 0.0);
    let n = f.doc.get_mut(inner).unwrap();
    n.kind.frame_props_mut().unwrap().clips_content = false;
    n.opacity = 0.9;
    grey_text(&mut f, inner);
    assert_eq!(audited(f), (0, 0));
}

#[test]
fn text_only_partly_inside_a_clipping_opaque_card_keeps_the_old_behaviour() {
    // Half of the text is cut off by Card B, so the fill does not cover the whole text box.
    let (mut f, b) = cards(shadow, Some(Color::WHITE));
    f.doc.get_mut(b).unwrap().kind.frame_props_mut().unwrap().clips_content = true;
    let t = f.text(b, "Grey text", 14.0, 400, gray(0x77));
    resize(&mut f, t, 40.0, 16.0);
    place(&mut f, t, 180.0, 24.0);
    assert_eq!(audited(f), (0, 0));
}

#[test]
fn without_an_opaque_fill_under_the_text_the_earlier_card_still_hides_the_backdrop() {
    for (what, fill, opacity, paint_opacity) in [
        ("no fill", None, 1.0, 1.0),
        ("fill with alpha 0.5", Some(Color::rgba(255, 255, 255, 128)), 1.0, 1.0),
        ("paint opacity 0.5", Some(Color::WHITE), 1.0, 0.5),
        ("layer opacity 0.9", Some(Color::WHITE), 0.9, 1.0),
    ] {
        let (mut f, b) = card_list(shadow, fill);
        let n = f.doc.get_mut(b).unwrap();
        n.opacity = opacity;
        if let Some(p) = n.fills.first_mut() {
            p.opacity = paint_opacity;
        }
        assert_eq!(audited(f), (0, 0), "{what}: Card A's shadow can show through");
    }
}

#[test]
fn a_layer_painted_above_the_opaque_fill_still_hides_the_backdrop() {
    // An earlier child of Card B itself, painted after Card B's fill, under the text. A multiplied
    // fill is not a plain backdrop colour, so the text is left alone.
    let (mut f, b) = card_list(|_| {}, Some(Color::WHITE));
    let backing = f.rect(b, "Backing", gray(0x80));
    resize(&mut f, backing, 100.0, 60.0);
    place(&mut f, backing, 10.0, 10.0);
    f.doc.get_mut(backing).unwrap().fills[0].blend = designos_doc::BlendMode::Multiply;
    // Put it before the text in Card B's stacking order.
    f.doc.get_mut(b).unwrap().children.rotate_right(1);
    assert_eq!(audited(f), (0, 0), "a multiplied rectangle inside Card B sits between its fill and the text");
}

#[test]
fn a_shadow_of_an_earlier_child_inside_the_opaque_card_still_hides_the_backdrop() {
    let (mut f, b) = card_list(|_| {}, Some(Color::WHITE));
    let chip = f.frame(b, "Chip", Some(Color::WHITE));
    resize(&mut f, chip, 60.0, 10.0);
    place(&mut f, chip, 10.0, 0.0);
    shadow(f.doc.get_mut(chip).unwrap());
    f.doc.get_mut(b).unwrap().children.rotate_right(1);
    assert_eq!(audited(f), (0, 0), "the chip's shadow falls on the fill under the text");
}

#[test]
fn the_stroke_of_the_opaque_card_itself_still_hides_the_backdrop() {
    // The stroke is painted over Card B's fill and under its children: here an inside band 40 px wide.
    let (mut f, b) = card_list(|_| {}, Some(Color::WHITE));
    let n = f.doc.get_mut(b).unwrap();
    n.stroke.paints = vec![Paint::solid(Color::BLACK)];
    n.stroke.weight = 40.0;
    n.stroke.align = StrokeAlign::Inside;
    assert_eq!(audited(f), (0, 0));
}

#[test]
fn the_stroke_of_a_frame_below_the_opaque_card_does_not_hide_the_text_on_it() {
    // A section with a 100 px inside stroke, no fill; Card B inside it covers part of the band.
    let mut f = fx();
    white_page(&mut f);
    let section = f.frame(f.page, "Section", None);
    resize(&mut f, section, 300.0, 300.0);
    place(&mut f, section, 0.0, 0.0);
    {
        let n = f.doc.get_mut(section).unwrap();
        n.kind.frame_props_mut().unwrap().clips_content = false;
        n.stroke.paints = vec![Paint::solid(Color::BLACK)];
        n.stroke.weight = 100.0;
        n.stroke.align = StrokeAlign::Inside;
    }
    let card = f.frame(section, "Card", Some(Color::WHITE));
    resize(&mut f, card, 180.0, 180.0);
    place(&mut f, card, 60.0, 60.0);
    let t = f.text(card, "Grey text", 14.0, 400, gray(0x77));
    resize(&mut f, t, 40.0, 16.0);
    place(&mut f, t, 20.0, 20.0);
    assert_eq!(audited(f), (1, 1), "the band is under Card's white fill");
}

#[test]
fn a_later_card_is_not_looked_at_for_the_card_before_it() {
    // Card C comes after Card B in the stacking order, 16 px below it, with a shadow. The audit has
    // never looked at layers painted after the text's ancestors, and still does not.
    let (mut f, _) = card_list(|_| {}, Some(Color::WHITE));
    let c = f.frame(f.page, "Card C", Some(Color::WHITE));
    resize(&mut f, c, 200.0, 100.0);
    place(&mut f, c, 40.0, 272.0);
    shadow(f.doc.get_mut(c).unwrap());
    assert_eq!(audited(f), (1, 1));
}

#[test]
fn text_inside_a_rounded_opaque_card_inset_by_corner_radius_is_checked_while_corner_cutout_text_is_skipped() {
    let (mut f, b) = card_list(shadow, Some(Color::WHITE));
    f.doc.get_mut(b).unwrap().corner_radii = [16.0; 4];
    // Grey text at (16, 24) of size (40, 16) sits wholly within the 16 px corner-radius inset box [16..184, 16..84].
    assert_eq!(audited(f), (1, 1));

    // Text tucked into the top-left rounded corner cutout at (2, 2) can overlap Card A's shadow showing around the curve.
    let (mut f_corner, b_corner) = cards(shadow, Some(Color::WHITE));
    f_corner.doc.get_mut(b_corner).unwrap().corner_radii = [24.0; 4];
    let t = f_corner.text(b_corner, "Corner text", 14.0, 400, gray(0x77));
    resize(&mut f_corner, t, 40.0, 16.0);
    place(&mut f_corner, t, 2.0, 2.0);
    assert_eq!(audited(f_corner), (0, 0));
}

#[test]
fn an_opaque_card_with_a_visible_effect_does_not_occlude_an_earlier_sibling_shadow() {
    let (mut f, b) = card_list(shadow, Some(Color::WHITE));
    blur(f.doc.get_mut(b).unwrap());
    assert_eq!(audited(f), (0, 0));
}
