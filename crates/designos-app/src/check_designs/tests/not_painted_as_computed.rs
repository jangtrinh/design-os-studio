//! Text the renderer paints differently from the plain composite the rule computes is not checked:
//! a text paint with a blend other than source-over, and a layer with opacity 0, which is not drawn
//! at all.

use super::*;
use designos_doc::BlendMode;

fn contrast_findings(f: Fx) -> usize {
    f.report().of(Rule::Contrast).count()
}

/// White page; 14 px #777777 text (4.47:1) whose only fill blends with `blend`.
fn grey_text_with_blend(blend: BlendMode) -> Fx {
    let mut f = fx();
    white_page(&mut f);
    let t = f.text(f.page, "Grey text", 14.0, 400, gray(0x77));
    let NodeKind::Text(tp) = &mut f.doc.get_mut(t).unwrap().kind else { panic!("text") };
    tp.style.fills[0].blend = blend;
    f
}

#[test]
fn a_text_paint_that_does_not_blend_as_source_over_is_not_checked() {
    for blend in [BlendMode::Screen, BlendMode::Multiply, BlendMode::Difference] {
        assert_eq!(contrast_findings(grey_text_with_blend(blend)), 0, "{blend:?}: what reaches the screen is not the colour the rule computes");
    }
}

#[test]
fn a_text_paint_the_renderer_draws_as_source_over_is_checked() {
    // designos-render draws Normal and PassThrough paints with the same source-over blend.
    for blend in [BlendMode::Normal, BlendMode::PassThrough] {
        assert_eq!(contrast_findings(grey_text_with_blend(blend)), 1, "{blend:?}");
    }
}

#[test]
fn the_fix_is_not_offered_for_a_screened_text_paint() {
    let mut app = grey_text_with_blend(BlendMode::Screen).app();
    let findings = audit(&app.doc, &Scope::Page(app.page)).findings;
    assert_eq!(app.check_designs_fix_all(&findings), 0);
    assert!(!app.hist.can_undo());
}

/// White page; a frame holding 14 px #111111 text (it passes AA when drawn), with `opacity` on the
/// text or on the frame.
fn dark_text(frame_opacity: f32, text_opacity: f32) -> Fx {
    let mut f = fx();
    white_page(&mut f);
    let frame = f.frame(f.page, "Card", None);
    f.doc.get_mut(frame).unwrap().opacity = frame_opacity;
    let t = f.text(frame, "Dark text", 14.0, 400, gray(0x11));
    f.doc.get_mut(t).unwrap().opacity = text_opacity;
    f
}

#[test]
fn a_layer_that_is_not_drawn_because_its_opacity_is_zero_has_no_contrast() {
    assert_eq!(contrast_findings(dark_text(1.0, 0.0)), 0, "text with opacity 0");
    assert_eq!(contrast_findings(dark_text(0.0, 1.0)), 0, "text inside a frame with opacity 0");
}

#[test]
fn faint_but_drawn_text_is_still_checked() {
    assert_eq!(contrast_findings(dark_text(1.0, 0.2)), 1, "text with opacity 0.2 is drawn, and too faint");
    assert_eq!(contrast_findings(dark_text(0.2, 1.0)), 1, "text inside a frame with opacity 0.2 is drawn, and too faint");
}

#[test]
fn a_text_paint_whose_own_alpha_or_opacity_is_zero_is_not_drawn_and_has_no_contrast() {
    let mut zero_color_alpha = fx();
    white_page(&mut zero_color_alpha);
    zero_color_alpha.text(zero_color_alpha.page, "Invisible colour", 14.0, 400, Color::rgba(17, 17, 17, 0));
    assert_eq!(contrast_findings(zero_color_alpha), 0, "text colour alpha 0 paints nothing");

    let mut zero_paint_opacity = fx();
    white_page(&mut zero_paint_opacity);
    let t = zero_paint_opacity.text(zero_paint_opacity.page, "Invisible paint", 14.0, 400, gray(0x11));
    if let NodeKind::Text(tp) = &mut zero_paint_opacity.doc.get_mut(t).unwrap().kind {
        tp.style.fills[0].opacity = 0.0;
    }
    assert_eq!(contrast_findings(zero_paint_opacity), 0, "text paint opacity 0 paints nothing");
}
