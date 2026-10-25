//! Font selection: bundled Inter, variable weights, substitution.

use designos_doc::*;
use designos_render::{render_page, resolve_font, tiny_skia::Pixmap, Viewport};

const SAMPLE: &str = "Hamburgefonstiv";

/// Builds the font database from the bundled fonts only, so results never depend on what the
/// host has installed (a machine with SF Pro installed resolves "SF Pro Text" to the real face).
/// The database is process-wide and built on first use, so every test calls this first.
fn hermetic_fonts() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| std::env::set_var("DESIGNOS_BUNDLED_FONTS_ONLY", "1"));
}

fn render_text(family: &str, weight: u16, italic: bool) -> Pixmap {
    render_styled(TextStyle { font_family: family.into(), font_weight: weight, italic, font_size: 32.0, ..Default::default() }, &[])
}

/// Draws `SAMPLE` in `style`, with `runs` (byte range, style) laid over it.
fn render_styled(style: TextStyle, runs: &[(usize, usize, TextStyle)]) -> Pixmap {
    hermetic_fonts();
    let mut doc = Document::new("t");
    let page = doc.add_page("p");
    doc.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::WHITE };
    let id = doc.alloc_id();
    let mut node = Node::text(id, Rect::new(4.0, 4.0, 400.0, 48.0), SAMPLE, style);
    if let NodeKind::Text(t) = &mut node.kind {
        t.auto_resize = TextAutoResize::WidthAndHeight;
        t.runs = runs.iter().map(|(start, end, style)| TextRun { start: *start, end: *end, style: style.clone() }).collect();
    }
    doc.insert(page, node).unwrap();
    let mut pm = Pixmap::new(320, 56).unwrap();
    render_page(&doc, page, &Viewport::default(), &mut pm);
    pm
}

/// Total darkness (ink coverage) of a white-background render.
fn ink(pm: &Pixmap) -> u64 {
    pm.pixels().iter().map(|p| 255 - p.red() as u64).sum()
}

#[test]
fn inter_resolves_to_the_bundled_face() {
    hermetic_fonts();
    for (w, italic) in [(400, false), (700, false), (400, true), (900, true)] {
        let r = resolve_font("Inter", w, italic).expect("Inter resolves");
        assert_eq!(r.family, "Inter");
        assert!(r.bundled, "Inter {w} italic={italic} should be the bundled face");
    }
}

#[test]
fn sf_pro_text_renders_through_inter() {
    hermetic_fonts();
    let r = resolve_font("SF Pro Text", 400, false).unwrap();
    assert_eq!((r.family.as_str(), r.bundled), ("Inter", true));
    for fam in ["SF Pro Display", "-apple-system", "system-ui"] {
        assert_eq!(resolve_font(fam, 600, false).unwrap().family, "Inter", "{fam}");
    }
    let a = render_text("SF Pro Text", 500, false);
    let b = render_text("Inter", 500, false);
    assert_eq!(a.data(), b.data(), "SF Pro Text must be drawn with Inter");
}

#[test]
fn unknown_family_falls_back_to_inter() {
    hermetic_fonts();
    let r = resolve_font("Definitely Not A Font", 400, false).unwrap();
    assert_eq!((r.family.as_str(), r.bundled), ("Inter", true));
    assert_eq!(render_text("Definitely Not A Font", 400, false).data(), render_text("Inter", 400, false).data());
}

#[test]
fn variable_weight_axis_is_applied() {
    let (w400, w700, w900) = (ink(&render_text("Inter", 400, false)), ink(&render_text("Inter", 700, false)), ink(&render_text("Inter", 900, false)));
    assert!(w400 > 0);
    assert!(w700 > w400 + w400 / 10, "700 should be clearly heavier: {w400} vs {w700}");
    assert!(w900 > w700, "900 heavier than 700: {w700} vs {w900}");
    let w100 = ink(&render_text("Inter", 100, false));
    assert!(w100 < w400, "100 lighter than 400: {w100} vs {w400}");
}

/// A bold run is what Ctrl+B writes on a selected word: the base stays at 400, the run carries 700.
#[test]
fn a_bold_run_on_inter_draws_heavier_than_the_regular_run() {
    let regular = TextStyle { font_family: "Inter".into(), font_weight: 400, font_size: 32.0, ..Default::default() };
    let bold = TextStyle { font_weight: 700, ..regular.clone() };
    let plain = render_styled(regular.clone(), &[]);
    let whole = render_styled(regular.clone(), &[(0, SAMPLE.len(), bold.clone())]);
    let first_half = render_styled(regular.clone(), &[(0, SAMPLE.len() / 2, bold)]);
    let (p, w, h) = (ink(&plain), ink(&whole), ink(&first_half));
    assert!(w > p + p / 10, "a bold run over the whole text should be clearly heavier: {p} vs {w}");
    assert!(p < h && h < w, "bold on half the text lands in between: {p} < {h} < {w}");
    assert_eq!(whole.data(), render_text("Inter", 700, false).data(), "a bold run draws like a bold node");
}

/// The shaper reads the `wght` axis before `font_weight`, so a weight edit has to move the axis too.
#[test]
fn a_wght_axis_overrides_font_weight_when_drawn() {
    let axis = |w: f32| {
        let mut st = TextStyle { font_family: "Inter".into(), font_weight: 700, font_size: 32.0, ..Default::default() };
        st.set_axis("wght", w);
        st
    };
    let stale = ink(&render_styled(axis(400.0), &[]));
    let in_step = render_styled(axis(700.0), &[]);
    assert_eq!(stale, ink(&render_text("Inter", 400, false)), "the axis wins over font_weight");
    assert_eq!(in_step.data(), render_text("Inter", 700, false).data(), "an axis in step with the weight draws that weight");
}

#[test]
fn italic_uses_the_italic_face() {
    let normal = render_text("Inter", 400, false);
    let italic = render_text("Inter", 400, true);
    assert_ne!(normal.data(), italic.data());
}

#[test]
fn rendering_is_deterministic() {
    assert_eq!(render_text("Inter", 600, false).data(), render_text("Inter", 600, false).data());
}

#[test]
fn hermetic_database_holds_only_bundled_faces() {
    hermetic_fonts();
    let bundled: Vec<String> = designos_fonts::bundled().iter().map(|a| a.family.to_lowercase()).collect();
    for fam in designos_render::available_families() {
        assert!(bundled.contains(&fam.to_lowercase()), "non-bundled family leaked into the hermetic database: {fam}");
    }
}
