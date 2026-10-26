//! The fill colour picker's Libraries tab shows a colour style's description on hover,
//! the same text as the style picker, and only when that description is not empty.

use super::*;

fn painted_texts(s: &Sim) -> Vec<(String, Rect)> {
    fn collect(shape: &egui::epaint::Shape, out: &mut Vec<(String, Rect)>) {
        match shape {
            egui::epaint::Shape::Text(t) => out.push((t.galley.job.text.clone(), t.visual_bounding_rect())),
            egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for cs in &s.shapes {
        collect(&cs.shape, &mut out);
    }
    out
}

fn hover_at(s: &mut Sim, p: Pos2) {
    s.mv(p + Vec2::new(0.0, 1.0));
    s.mv(p);
    for _ in 0..30 {
        s.frame(vec![]);
    }
}

#[test]
fn colour_picker_libraries_tab_shows_style_description_tooltip_only_when_non_empty() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    assert!(s.shell.app.set_style_description_to("Brand/Primary", "Brand primary fill"));
    assert!(crate::styles::style_description(&s.app().doc, "Neutral/Ink").is_empty());

    s.click(s.rect_of("sw:false:0").center());
    s.frame(vec![]);
    s.click(s.rect_of("picker-tab:Libraries").center());
    s.frame(vec![]);

    let items = painted_texts(&s);
    let (_, primary) = items.iter().find(|(t, _)| t == "Brand/Primary").cloned().expect("Brand/Primary row in the Libraries tab");
    let (_, ink) = items.iter().find(|(t, _)| t == "Neutral/Ink").cloned().expect("Neutral/Ink row in the Libraries tab");
    let (_, header) = items.iter().find(|(t, _)| t.starts_with("Variables")).cloned().expect("a section header in the Libraries tab");

    hover_at(&mut s, primary.center());
    assert!(
        painted_texts(&s).iter().any(|(t, _)| t == "Brand primary fill"),
        "hovering a colour style with a description in the Libraries tab should paint its description tooltip"
    );

    hover_at(&mut s, ink.center());
    assert!(
        !painted_texts(&s).iter().any(|(t, _)| t == "Brand primary fill"),
        "hovering a colour style without a description in the Libraries tab must show no tooltip"
    );

    hover_at(&mut s, header.center());
    assert!(!painted_texts(&s).iter().any(|(t, _)| t == "Brand primary fill"), "hovering a section header in the Libraries tab must show no tooltip");
}
