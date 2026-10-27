//! What an earlier layer paints beyond its own box (an outside or centred stroke, a drop shadow, a
//! blur, text colour kept in style runs) is behind the text when it reaches it, even with a gap
//! between the two boxes. Such text is neither reported nor fixed.

use super::*;
use designos_doc::{Effect, StrokeAlign, StrokeJoin, TextRun, Vec2};

/// White page; an earlier 100 x 40 rectangle at (0, 40) with no fill and a 20 px black stroke of the
/// given alignment (the band runs x = 80..100 inside, 90..110 centred, 100..120 outside); later
/// 40 x 16 grey text of 4.47:1 at (x, 48), a 5 px gap away from the rectangle for x = 105.
fn stroked_rectangle_and_text(align: StrokeAlign, text_x: f32) -> Fx {
    let mut f = fx();
    white_page(&mut f);
    let r = f.rect(f.page, "Earlier", Color::WHITE);
    resize(&mut f, r, 100.0, 40.0);
    place(&mut f, r, 0.0, 40.0);
    let n = f.doc.get_mut(r).unwrap();
    n.fills.clear();
    n.stroke.paints = vec![Paint::solid(Color::BLACK)];
    n.stroke.weight = 20.0;
    n.stroke.align = align;
    later_text(&mut f, text_x);
    f
}

fn later_text(f: &mut Fx, x: f32) -> NodeId {
    let t = f.text(f.page, "Grey text", 14.0, 400, gray(0x77));
    resize(f, t, 40.0, 16.0);
    place(f, t, x, 48.0);
    t
}

/// White page; an earlier white 100 x 40 rectangle at (0, 40) with the given effects; later grey
/// text at (x, 48).
fn effect_rectangle_and_text(effects: Vec<Effect>, text_x: f32) -> Fx {
    let mut f = fx();
    white_page(&mut f);
    let r = f.rect(f.page, "Earlier", Color::WHITE);
    resize(&mut f, r, 100.0, 40.0);
    place(&mut f, r, 0.0, 40.0);
    f.doc.get_mut(r).unwrap().effects = effects;
    later_text(&mut f, text_x);
    f
}

fn shadow(offset_x: f32, visible: bool) -> Effect {
    Effect::DropShadow {
        color: Color::BLACK,
        offset: Vec2::new(offset_x, 0.0),
        radius: 10.0,
        spread: 0.0,
        visible,
        show_behind: false,
        blend: Default::default(),
    }
}

/// How many contrast findings the page has, and what "Fix all" lands.
fn audited(f: Fx) -> (usize, usize) {
    let mut app = f.app();
    let findings = audit(&app.doc, &Scope::Page(app.page)).findings;
    let contrast = findings.iter().filter(|x| x.rule == Rule::Contrast).count();
    (contrast, app.check_designs_fix_all(&findings))
}

#[test]
fn a_stroke_that_reaches_the_text_across_a_gap_is_behind_it() {
    // Per alignment (inside, centred, outside): is the text skipped at x = 105, 115, 125?
    let cases: [(StrokeAlign, [bool; 3]); 3] =
        [(StrokeAlign::Inside, [false, false, false]), (StrokeAlign::Center, [true, false, false]), (StrokeAlign::Outside, [true, true, false])];
    for (align, skipped) in cases {
        for (x, skipped) in [105.0, 115.0, 125.0].into_iter().zip(skipped) {
            let (found, fixed) = audited(stroked_rectangle_and_text(align, x));
            assert_eq!((found, fixed), if skipped { (0, 0) } else { (1, 1) }, "{align:?} stroke, text at x = {x}: skipped = {skipped}");
        }
    }
}

#[test]
fn a_drop_shadow_or_a_blur_that_reaches_the_text_across_a_gap_is_behind_it() {
    let blur = |radius| Effect::LayerBlur { radius, visible: true };
    // The box ends at x = 100 and the text starts at 105 or 300.
    for (what, effects, near, far) in [("shadow", vec![shadow(20.0, true)], 105.0, 300.0), ("blur", vec![blur(20.0)], 105.0, 300.0)] {
        assert_eq!(audited(effect_rectangle_and_text(effects.clone(), near)), (0, 0), "{what}: the text is on the shadow or blur");
        assert_eq!(audited(effect_rectangle_and_text(effects, far)), (1, 1), "{what}: the text is clear of it");
    }
    assert_eq!(audited(effect_rectangle_and_text(vec![shadow(20.0, false)], 105.0)), (1, 1), "a hidden shadow paints nothing");
}

#[test]
fn a_stroked_line_has_no_height_but_its_stroke_still_reaches_the_text_next_to_it() {
    let mut f = fx();
    white_page(&mut f);
    let line = f.rect(f.page, "Rule", Color::WHITE);
    resize(&mut f, line, 100.0, 0.0);
    place(&mut f, line, 0.0, 66.0);
    let n = f.doc.get_mut(line).unwrap();
    n.kind = NodeKind::Line;
    n.fills.clear();
    n.stroke.paints = vec![Paint::solid(Color::BLACK)];
    n.stroke.weight = 10.0;
    later_text(&mut f, 20.0);
    assert_eq!(audited(f), (0, 0));
}

#[test]
fn a_stroke_given_only_by_per_side_weights_still_counts() {
    // The renderer draws a box stroke whose `weight` is 0 when per-side weights are set.
    let mut f = stroked_rectangle_and_text(StrokeAlign::Outside, 105.0);
    let r = f.doc.get(f.page).unwrap().children[0];
    let n = f.doc.get_mut(r).unwrap();
    n.stroke.weight = 0.0;
    n.stroke.side_weights = Some([0.0, 20.0, 0.0, 0.0]);
    assert_eq!(audited(f), (0, 0));
}

#[test]
fn a_mitred_corner_can_stick_out_far_but_the_corners_of_a_box_do_not() {
    // An earlier 100 x 40 shape at (0, 40) with a 4 px outside stroke, mitre limit 10; grey text starts
    // at x = 120, 20 px from the shape and 16 px beyond the stroke band itself.
    let cases = [
        (NodeKind::Star { count: 5, inner_ratio: 0.4 }, StrokeJoin::Miter, (0, 0), "a star's mitred tips can reach the text"),
        (NodeKind::Star { count: 5, inner_ratio: 0.4 }, StrokeJoin::Round, (1, 1), "round joins stay within the band"),
        (NodeKind::Rectangle, StrokeJoin::Miter, (1, 1), "the corners of a box reach no further than the band"),
    ];
    for (kind, join, expected, what) in cases {
        let mut f = fx();
        white_page(&mut f);
        let r = f.rect(f.page, "Earlier", Color::WHITE);
        resize(&mut f, r, 100.0, 40.0);
        place(&mut f, r, 0.0, 40.0);
        let n = f.doc.get_mut(r).unwrap();
        n.kind = kind;
        n.fills.clear();
        n.stroke.paints = vec![Paint::solid(Color::BLACK)];
        n.stroke.weight = 4.0;
        n.stroke.align = StrokeAlign::Outside;
        n.stroke.join = join;
        n.stroke.miter_limit = 10.0;
        later_text(&mut f, 120.0);
        assert_eq!(audited(f), expected, "{what}");
    }
}

/// White page; an earlier 4 x 4 frame that does not clip holds a text of 100 x 20 at (100, 100)
/// whose base fills are empty and whose one run covers all of it with `run`; a later frame at
/// (100, 100) holds white 14 px text.
fn run_coloured_text_behind(run: Color) -> Fx {
    let mut f = fx();
    white_page(&mut f);
    let earlier = f.frame(f.page, "Earlier", None);
    resize(&mut f, earlier, 4.0, 4.0);
    place(&mut f, earlier, 0.0, 0.0);
    f.doc.get_mut(earlier).unwrap().kind.frame_props_mut().unwrap().clips_content = false;
    let label = f.text(earlier, "Caption", 14.0, 400, Color::WHITE);
    place(&mut f, label, 100.0, 100.0);
    resize(&mut f, label, 100.0, 20.0);
    let NodeKind::Text(tp) = &mut f.doc.get_mut(label).unwrap().kind else { panic!("text") };
    tp.style.fills.clear();
    tp.runs = vec![TextRun { start: 0, end: tp.content.len(), style: TextStyle { fills: vec![Paint::solid(run)], ..tp.style.clone() } }];
    let later = f.frame(f.page, "Later", None);
    resize(&mut f, later, 80.0, 40.0);
    place(&mut f, later, 100.0, 100.0);
    let t = f.text(later, "White text", 14.0, 400, Color::WHITE);
    resize(&mut f, t, 70.0, 17.0);
    place(&mut f, t, 4.0, 10.0);
    f
}

/// White page; an earlier 4 x 4 frame that does not clip holds another such frame with `effects`,
/// which holds a black 40 x 40 rectangle spanning x = 60..100; a later 40 x 16 grey text sits at
/// x = 105, a 5 px gap away.
fn rectangle_in_a_group_and_text(effects: Vec<Effect>) -> Fx {
    let mut f = fx();
    white_page(&mut f);
    let outer = f.frame(f.page, "Earlier", None);
    let group = f.frame(outer, "Group", None);
    for id in [outer, group] {
        resize(&mut f, id, 4.0, 4.0);
        place(&mut f, id, 0.0, 0.0);
        f.doc.get_mut(id).unwrap().kind.frame_props_mut().unwrap().clips_content = false;
    }
    f.doc.get_mut(group).unwrap().effects = effects;
    let r = f.rect(group, "Child", Color::BLACK);
    resize(&mut f, r, 40.0, 40.0);
    place(&mut f, r, 60.0, 40.0);
    later_text(&mut f, 105.0);
    f
}

#[test]
fn the_blur_of_an_earlier_group_spreads_what_its_children_paint() {
    let blur = Effect::LayerBlur { radius: 20.0, visible: true };
    assert_eq!(audited(rectangle_in_a_group_and_text(vec![blur])), (0, 0), "the blurred child reaches the text");
    assert_eq!(audited(rectangle_in_a_group_and_text(vec![])), (1, 1), "a sharp child ends before the text");
}

#[test]
fn a_child_inside_a_clipping_frame_counts_even_when_its_parent_sits_outside_that_frame() {
    // Earlier (4 x 4, no clip) > Cut (clips, 80 x 40 at (100, 100)) > Far (no clip, 4 x 4, far outside
    // Cut) > a black rectangle that sits back inside Cut, under the later text at (104, 110).
    let mut f = fx();
    white_page(&mut f);
    let earlier = f.frame(f.page, "Earlier", None);
    resize(&mut f, earlier, 4.0, 4.0);
    place(&mut f, earlier, 0.0, 0.0);
    f.doc.get_mut(earlier).unwrap().kind.frame_props_mut().unwrap().clips_content = false;
    let cut = f.frame(earlier, "Cut", None);
    resize(&mut f, cut, 80.0, 40.0);
    place(&mut f, cut, 100.0, 100.0);
    f.doc.get_mut(cut).unwrap().kind.frame_props_mut().unwrap().clips_content = true;
    let far = f.frame(cut, "Far", None);
    resize(&mut f, far, 4.0, 4.0);
    place(&mut f, far, 500.0, 500.0);
    f.doc.get_mut(far).unwrap().kind.frame_props_mut().unwrap().clips_content = false;
    let r = f.rect(far, "Child", Color::BLACK);
    resize(&mut f, r, 40.0, 20.0);
    place(&mut f, r, -490.0, -490.0);
    let later = f.frame(f.page, "Later", None);
    resize(&mut f, later, 80.0, 40.0);
    place(&mut f, later, 100.0, 100.0);
    let t = f.text(later, "White text", 14.0, 400, Color::WHITE);
    resize(&mut f, t, 70.0, 17.0);
    place(&mut f, t, 4.0, 10.0);
    assert_eq!(audited(f), (0, 0));
}

#[test]
fn text_painted_by_a_style_run_is_behind_the_text_it_overlaps() {
    assert_eq!(audited(run_coloured_text_behind(Color::BLACK)), (0, 0), "an opaque run paints under the white text");
    assert_eq!(audited(run_coloured_text_behind(Color::rgba(0, 0, 0, 0))), (1, 1), "a fully transparent run paints nothing");
}
