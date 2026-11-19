//! Lists, paragraph settings, OpenType features, leading trim and writing direction.

use designos_doc::*;
use designos_render::{font_features, render_page, text_geometry, tiny_skia::Pixmap, Viewport};

fn props(content: &str) -> TextProps {
    TextProps {
        content: content.into(),
        style: TextStyle { font_size: 16.0, line_height: LineHeight::Pixels(20.0), ..Default::default() },
        auto_resize: TextAutoResize::WidthAndHeight,
        ..Default::default()
    }
}

fn render(t: &TextProps, w: u32, h: u32, box_w: f32) -> Pixmap {
    let mut doc = Document::new("t");
    let page = doc.add_page("p");
    doc.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::WHITE };
    let id = doc.alloc_id();
    let mut node = Node::text(id, Rect::new(0.0, 0.0, box_w, h as f32), "", TextStyle::default());
    node.kind = NodeKind::Text(t.clone());
    doc.insert(page, node).unwrap();
    let mut pm = Pixmap::new(w, h).unwrap();
    render_page(&doc, page, &Viewport::default(), &mut pm);
    pm
}

fn dark(pm: &Pixmap, x: u32, y: u32) -> bool {
    pm.pixel(x, y).is_some_and(|p| p.red() < 160)
}

/// Whether any pixel in the column range / row range is dark.
fn ink_in(pm: &Pixmap, xs: std::ops::Range<u32>, ys: std::ops::Range<u32>) -> bool {
    ys.clone().any(|y| xs.clone().any(|x| dark(pm, x, y)))
}

fn lists(t: &mut TextProps, items: &[Option<ListItem>]) {
    t.set_paragraph_lists(items);
}

fn bullet(level: u8) -> Option<ListItem> {
    Some(ListItem { kind: ListKind::Bullet, level })
}

fn numbered(level: u8) -> Option<ListItem> {
    Some(ListItem { kind: ListKind::Numbered, level })
}

#[test]
fn bullets_draw_a_marker_and_indent_the_text() {
    let mut plain = props("one\ntwo");
    let mut list = plain.clone();
    lists(&mut list, &[bullet(0), bullet(0)]);
    plain.paragraph.hanging_list = false;
    // text starts at 1.5em = 24px; the marker sits in the first 24px
    let g = text_geometry(&list, Vec2::ZERO);
    assert_eq!(g.lines.len(), 2);
    assert!((g.lines[0].glyphs[0].x - 24.0).abs() < 0.6, "text indent {}", g.lines[0].glyphs[0].x);
    let p = text_geometry(&plain, Vec2::ZERO);
    assert!(p.lines[0].glyphs[0].x < 1.0);
    let pm = render(&list, 80, 44, 0.0);
    assert!(ink_in(&pm, 1..22, 0..20), "bullet in the first line's marker column");
    assert!(ink_in(&pm, 1..22, 20..40), "bullet in the second line's marker column");
}

#[test]
fn wrapped_list_lines_hang_under_the_text() {
    let mut t = props("alpha beta gamma delta epsilon zeta eta theta");
    t.auto_resize = TextAutoResize::Height;
    lists(&mut t, &[bullet(0)]);
    let g = text_geometry(&t, Vec2::new(120.0, 0.0));
    assert!(g.lines.len() >= 3, "wraps: {}", g.lines.len());
    let x0 = g.lines[0].glyphs[0].x;
    for l in &g.lines {
        assert!((l.glyphs[0].x - x0).abs() < 0.01, "hanging indent keeps every line at {x0}");
        let right = l.glyphs.last().map(|g| g.x + g.w).unwrap();
        assert!(right <= 120.5, "lines wrap at the reduced width: {right}");
    }
}

#[test]
fn numbered_markers_count_and_restart() {
    let mut t = props("a\nb\nplain\nc");
    lists(&mut t, &[numbered(0), numbered(0), None, numbered(0)]);
    let m = t.list_markers();
    assert_eq!(m.iter().map(|m| m.map(|m| m.number)).collect::<Vec<_>>(), vec![Some(1), Some(2), None, Some(1)]);
    let pm = render(&t, 80, 84, 0.0);
    // every numbered line has ink in the marker column; the plain line does not
    assert!(ink_in(&pm, 0..22, 0..20));
    assert!(ink_in(&pm, 0..22, 20..40));
    assert!(ink_in(&pm, 0..22, 60..80));
    let g = text_geometry(&t, Vec2::ZERO);
    assert!(g.lines[2].glyphs[0].x < 0.6, "the plain paragraph is not indented");
    assert!(g.lines[3].glyphs[0].x > 20.0);
    // the 1. marker of line 4 is the same drawing as the 1. of line 1
    let row = |y0: u32| -> Vec<bool> { (0..22).flat_map(|x| (0..20).map(move |y| (x, y))).map(|(x, y)| dark(&pm, x, y0 + y)).collect() };
    assert_eq!(row(0), row(60));
    assert_ne!(row(0), row(20));
}

#[test]
fn nested_levels_indent_further() {
    let mut t = props("a\nb\nc");
    lists(&mut t, &[bullet(0), bullet(1), bullet(2)]);
    let g = text_geometry(&t, Vec2::ZERO);
    let xs: Vec<f32> = g.lines.iter().map(|l| l.glyphs[0].x).collect();
    assert!(xs[1] > xs[0] + 20.0 && xs[2] > xs[1] + 20.0, "{xs:?}");
    let pm = render(&t, 100, 64, 0.0);
    // level 1 and 2 markers sit right of the level 0 column
    assert!(ink_in(&pm, 24..46, 20..40));
    assert!(ink_in(&pm, 48..70, 40..60));
    // distinct marker shapes per level: disc, ring (hollow centre), square
    assert!(dark(&pm, 2, 10), "disc is filled");
    assert!(!dark(&pm, 26, 30) && ink_in(&pm, 24..30, 24..36), "ring is hollow but drawn");
    assert!(dark(&pm, 50, 50), "square is filled");
}

#[test]
fn hanging_list_puts_markers_outside_the_box() {
    let mut t = props("one");
    lists(&mut t, &[bullet(0)]);
    t.paragraph.hanging_list = true;
    let g = text_geometry(&t, Vec2::ZERO);
    assert!(g.lines[0].glyphs[0].x < 0.6, "text aligned with the box edge");
}

#[test]
fn list_spacing_replaces_paragraph_spacing_between_items() {
    let mut t = props("a\nb\nc");
    t.style.paragraph_spacing = 30.0;
    t.paragraph.list_spacing = 4.0;
    lists(&mut t, &[bullet(0), bullet(0), None]);
    let g = text_geometry(&t, Vec2::ZERO);
    let gap01 = g.lines[1].top - (g.lines[0].top + g.lines[0].height);
    let gap12 = g.lines[2].top - (g.lines[1].top + g.lines[1].height);
    assert!((gap01 - 4.0).abs() < 0.01, "list spacing between items: {gap01}");
    assert!((gap12 - 30.0).abs() < 0.01, "paragraph spacing after the list: {gap12}");
}

#[test]
fn paragraph_indent_shifts_first_lines_only() {
    let mut t = props("alpha beta gamma delta epsilon zeta eta theta\nsecond paragraph here");
    t.auto_resize = TextAutoResize::Height;
    t.paragraph.indent = 20.0;
    let g = text_geometry(&t, Vec2::new(120.0, 0.0));
    let first_of_para: Vec<f32> = g.lines.iter().filter(|l| l.start == 0 || l.start == 46).map(|l| l.glyphs[0].x).collect();
    assert_eq!(first_of_para.len(), 2);
    assert!(first_of_para.iter().all(|x| (x - 20.0).abs() < 0.6), "{first_of_para:?}");
    let others: Vec<f32> = g.lines.iter().filter(|l| l.start != 0 && l.start != 46).map(|l| l.glyphs[0].x).collect();
    assert!(!others.is_empty() && others.iter().all(|x| x.abs() < 0.6), "continuation lines are flush: {others:?}");
    // the first line wraps earlier than the next ones
    let first = g.lines[0].glyphs.last().map(|g| g.x + g.w).unwrap();
    assert!(first <= 120.5);
}

#[test]
fn hanging_punctuation_pulls_an_opening_quote_out() {
    let mut t = props("\u{201C}Quoted\u{201D} text");
    t.auto_resize = TextAutoResize::Height;
    let flat = text_geometry(&t, Vec2::new(200.0, 0.0));
    t.paragraph.hanging_punctuation = true;
    let hung = text_geometry(&t, Vec2::new(200.0, 0.0));
    assert!(flat.lines[0].glyphs[0].x.abs() < 0.01);
    assert!(hung.lines[0].glyphs[0].x < -3.0, "quote hangs: {}", hung.lines[0].glyphs[0].x);
    let q = &hung.lines[0].glyphs[1];
    assert!(q.x.abs() < 0.1, "the first letter sits on the margin: {}", q.x);
}

#[test]
fn tabular_figures_equalise_digit_widths() {
    assert!(font_features("Inter", 400, false).iter().any(|f| f == "tnum"), "Inter has tnum");
    let widths = |t: &TextProps| -> Vec<f32> { text_geometry(t, Vec2::ZERO).lines[0].glyphs.iter().map(|g| g.w).collect() };
    let mut t = props("1180");
    let prop = widths(&t);
    assert!((prop[0] - prop[2]).abs() > 0.5, "proportional: 1 is narrower than 8 {prop:?}");
    t.style.features = vec![("tnum".into(), 1)];
    let tab = widths(&t);
    assert!(tab.iter().all(|w| (w - tab[0]).abs() < 0.01), "tabular digits share one width {tab:?}");
    // per-run features: only the run is tabular
    // (cosmic-text shapes a word with the features of its first character, so the run
    // covers whole words)
    let mut r = props("11 80");
    let mut st = r.style.clone();
    st.features = vec![("tnum".into(), 1)];
    r.runs = vec![TextRun { start: 3, end: 5, style: st }];
    let w = widths(&r);
    assert!((w[0] - prop[0]).abs() < 0.01 && (w[3] - tab[2]).abs() < 0.01 && (w[4] - tab[3]).abs() < 0.01, "{w:?}");
}

#[test]
fn stylistic_sets_and_features_change_the_outlines() {
    let feats = font_features("Inter", 400, false);
    for f in ["calt", "ss01", "case", "frac", "zero"] {
        assert!(feats.iter().any(|x| x == f), "Inter lists {f}: {feats:?}");
    }
    let base = props("Illegal 0 a");
    let mut zero = base.clone();
    zero.style.features = vec![("zero".into(), 1)];
    let a = render(&base, 120, 24, 0.0);
    let b = render(&zero, 120, 24, 0.0);
    assert_ne!(a.data(), b.data(), "slashed zero changes the picture");
}

#[test]
fn leading_trim_cuts_the_box_to_cap_height_and_baseline() {
    let mut t = props("HEH");
    t.style.line_height = LineHeight::Pixels(40.0);
    t.style.font_size = 32.0;
    let full = text_geometry(&t, Vec2::ZERO);
    t.paragraph.leading_trim = LeadingTrim::CapHeight;
    let trim = text_geometry(&t, Vec2::ZERO);
    assert!(trim.height < full.height - 10.0, "{} vs {}", trim.height, full.height);
    // cap height of Inter is ~0.727em
    assert!((trim.height - 0.727 * 32.0).abs() < 1.5, "box = cap height: {}", trim.height);
    assert!(trim.lines[0].top < 0.0 && (trim.lines[0].baseline - trim.height).abs() < 0.01, "baseline is the bottom edge");
    // the ink of H touches the top and the bottom of the box
    let pm = render(&t, 100, 40, 0.0);
    let rows: Vec<u32> = (0..40).filter(|y| ink_in(&pm, 0..100, *y..*y + 1)).collect();
    assert!(*rows.first().unwrap() <= 1, "ink starts at the top: {rows:?}");
    assert!(((*rows.last().unwrap() + 1) as f32 - trim.height).abs() < 1.5, "ink ends at the baseline: {rows:?} vs {}", trim.height);
}

#[test]
fn leading_trim_respects_fixed_box_alignment() {
    let mut t = props("HEH");
    t.auto_resize = TextAutoResize::None;
    t.style.font_size = 32.0;
    t.style.align_v = TextAlignV::Center;
    t.paragraph.leading_trim = LeadingTrim::CapHeight;
    let g = text_geometry(&t, Vec2::new(100.0, 100.0));
    let ink_h = g.lines[0].baseline - (g.lines[0].baseline - 0.727 * 32.0);
    let mid = g.lines[0].baseline - ink_h / 2.0;
    assert!((mid - 50.0).abs() < 1.5, "cap-height ink is vertically centred: {mid}");
}

#[test]
fn forced_direction_reorders_runs() {
    // A right-to-left character splits "abc" and "def" into separate runs, which swap
    // places in a right-to-left paragraph. (The Hebrew letter may come from any fallback
    // font, even a .notdef box: only its position matters here.)
    let mut t = props("abc \u{5D0} def");
    let ltr = text_geometry(&t, Vec2::ZERO);
    let (a, d) = (ltr.lines[0].glyphs.iter().find(|g| g.start == 0).unwrap().x, ltr.lines[0].glyphs.iter().find(|g| g.start == 7).unwrap().x);
    assert!(a < d, "abc before def in an LTR paragraph");
    assert!(!ltr.lines[0].rtl);
    t.direction = TextDirection::Rtl;
    let rtl = text_geometry(&t, Vec2::ZERO);
    let gl = &rtl.lines[0].glyphs;
    assert!(rtl.lines[0].rtl);
    assert_eq!(gl.len(), 9, "the invisible direction mark is not a glyph");
    let a = gl.iter().find(|g| g.start == 0).unwrap();
    let d = gl.iter().find(|g| g.start == 7).unwrap();
    assert!(d.x < a.x, "def comes first (left) in an RTL paragraph: {} vs {}", d.x, a.x);
    // each run keeps its internal left-to-right order
    let b = gl.iter().find(|g| g.start == 1).unwrap();
    assert!(a.x < b.x);
    // glyphs are reported left to right, offsets map to the original content
    assert!(gl.windows(2).all(|w| w[0].x <= w[1].x));
    assert!(gl.iter().all(|g| g.end <= 10));
    t.direction = TextDirection::Ltr;
    let forced_ltr = text_geometry(&t, Vec2::ZERO);
    assert_eq!(forced_ltr.lines[0].glyphs.len(), 9);
    assert!(forced_ltr.lines[0].glyphs.windows(2).all(|w| w[0].x <= w[1].x));
}

#[test]
fn forced_rtl_keeps_a_latin_line_looking_left_to_right() {
    // Pure Latin text is one LTR run in an RTL paragraph.
    let mut t = props("abc 123");
    t.direction = TextDirection::Rtl;
    let g = text_geometry(&t, Vec2::ZERO);
    assert!(g.lines[0].rtl);
    let gl = &g.lines[0].glyphs;
    assert!(gl.windows(2).all(|w| w[0].x <= w[1].x));
    assert_eq!((gl[0].start, gl[6].end), (0, 7));
}

#[test]
fn right_to_left_list_items_put_the_marker_on_the_right() {
    let mut t = props("\u{5E9}\u{5DC}\u{5D5}\u{5DD}");
    t.auto_resize = TextAutoResize::None;
    t.style.align_h = TextAlignH::Right;
    lists(&mut t, &[bullet(0)]);
    let g = text_geometry(&t, Vec2::new(100.0, 20.0));
    assert!(g.lines[0].rtl);
    let right = g.lines[0].glyphs.iter().map(|g| g.x + g.w).fold(0.0f32, f32::max);
    assert!(right <= 100.0 - 23.0, "the text stops short of the marker column: {right}");
    let pm = render(&t, 100, 24, 100.0);
    assert!(ink_in(&pm, 78..100, 0..20), "bullet at the right edge");
    // a left-to-right paragraph keeps it on the left
    let mut l = t.clone();
    l.content = "abc".into();
    l.style.align_h = TextAlignH::Left;
    let g = text_geometry(&l, Vec2::new(100.0, 20.0));
    assert!(!g.lines[0].rtl && g.lines[0].glyphs[0].x >= 23.0);
}
