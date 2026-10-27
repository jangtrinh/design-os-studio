//! Dev mode Inspect panel: the code keeps its lines and scrolls sideways, the `?` button never covers text, Colors lists
//! the used colors (and is absent when there are none), and the MCP prompt shows in full.

use super::*;
use crate::dev::Language;
use crate::panels::inspect::{self, Cur, PAD};
use crate::theme;
use designos_doc::{Document, Node, Rect as DRect, Stroke as DStroke, TextStyle};

/// One piece of text the window painted: what it says, where it lands and the clip it was drawn through.
struct Painted {
    text: String,
    rect: Rect,
    clip: Rect,
}

impl Painted {
    /// The part of the text a user can see.
    fn visible(&self) -> Rect {
        self.rect.intersect(self.clip)
    }
}

fn painted(shapes: &[egui::epaint::ClippedShape]) -> Vec<Painted> {
    fn walk(shape: &egui::Shape, clip: Rect, out: &mut Vec<Painted>) {
        match shape {
            egui::Shape::Text(t) => out.push(Painted { text: t.galley.text().to_string(), rect: t.galley.rect.translate(t.pos.to_vec2()), clip }),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for cs in shapes {
        walk(&cs.shape, cs.clip_rect, &mut out);
    }
    out
}

/// A bare egui window to draw one inspector piece in, with the pointer and wheel scripted by the test.
struct Bench {
    ctx: Context,
    t: f64,
}

impl Bench {
    fn new() -> Self {
        let ctx = Context::default();
        theme::install_fonts(&ctx);
        Self { ctx, t: 0.0 }
    }
    fn frame(&mut self, events: Vec<Event>, f: impl FnMut(&mut egui::Ui)) -> Vec<egui::epaint::ClippedShape> {
        self.t += 0.05;
        let raw = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 8000.0))), time: Some(self.t), events, ..Default::default() };
        self.ctx.run_ui(raw, f).shapes
    }
}

fn wheel(delta: Vec2) -> Event {
    Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta, phase: egui::TouchPhase::Move, modifiers: Modifiers::NONE }
}

const PANEL_W: f32 = 240.0;

fn draw_code(b: &mut Bench, events: Vec<Event>, lang: Language, code: &str) -> (Vec<Painted>, f32) {
    let mut height = 0.0;
    let shapes = b.frame(events, |ui| {
        let mut c = Cur { x0: 0.0, oy: 0.0, y: 0.0, w: PANEL_W };
        crate::dev::sections::code_block(ui, &mut c, lang, code);
        height = c.y;
    });
    (painted(&shapes), height)
}

/// Left edge of the painted piece that holds `needle`.
fn left_of(texts: &[Painted], needle: &str) -> f32 {
    texts
        .iter()
        .find(|p| p.text.contains(needle))
        .unwrap_or_else(|| panic!("nothing painted with {needle:?}: {:?}", texts.iter().map(|p| &p.text).collect::<Vec<_>>()))
        .rect
        .left()
}

fn right_of(texts: &[Painted], needle: &str) -> f32 {
    texts.iter().find(|p| p.text.contains(needle)).unwrap_or_else(|| panic!("nothing painted with {needle:?}")).rect.right()
}

const XML: &str = "<FrameLayout\n    xmlns:android=\"http://schemas.android.com/apk/res/android\"\n    android:layout_width=\"120dp\">\n</FrameLayout>";

/// Height of the code box: a row per line between the control insets, plus one 4px step under the code for the scroll
/// thumb when a line is wider than the box.
fn block_height(lines: usize, scrolls: bool) -> f32 {
    lines as f32 * theme::CODE_LINE_HEIGHT + 2.0 * theme::INSET_CONTROL + if scrolls { theme::SPACE_FACT } else { 0.0 }
}

/// Block geometry of `code_block` at `PANEL_W`: the box spans `PAD..PANEL_W-PAD` and the text sits one control inset in.
fn block_inner(lines: usize, scrolls: bool) -> Rect {
    let top = 0.0;
    Rect::from_min_size(Pos2::new(PAD + theme::INSET_CONTROL, top), Vec2::new(PANEL_W - 2.0 * PAD - 2.0 * theme::INSET_CONTROL, block_height(lines, scrolls)))
}

/// A rect the code box recorded for tests on its last frame.
fn recorded(b: &Bench, name: &str) -> Option<Rect> {
    b.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", name.to_string()))))
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}

/// Presses at `from` and drags by `by` in two moves, then releases.
fn drag_code(b: &mut Bench, from: Pos2, by: Vec2) {
    draw_code(b, vec![Event::PointerMoved(from)], Language::AndroidXml, XML);
    draw_code(b, vec![button(from, true)], Language::AndroidXml, XML);
    draw_code(b, vec![Event::PointerMoved(from + by * 0.5)], Language::AndroidXml, XML);
    draw_code(b, vec![Event::PointerMoved(from + by)], Language::AndroidXml, XML);
    draw_code(b, vec![button(from + by, false)], Language::AndroidXml, XML);
}

fn settle(b: &mut Bench, frames: usize) -> Vec<Painted> {
    for _ in 1..frames {
        draw_code(b, vec![], Language::AndroidXml, XML);
    }
    draw_code(b, vec![], Language::AndroidXml, XML).0
}

#[test]
fn a_code_line_stays_on_one_row_whatever_the_panel_width() {
    let mut b = Bench::new();
    let line = "background: var(--Colors-Background-bg-primary, #FFF);";
    let (texts, height) = draw_code(&mut b, vec![], Language::Css, line);
    let ys: Vec<f32> = texts.iter().map(|p| p.rect.center().y).collect();
    let (lo, hi) = ys.iter().fold((f32::MAX, f32::MIN), |(lo, hi), y| (lo.min(*y), hi.max(*y)));
    assert!(hi - lo < 0.5, "the line broke into several rows: {ys:?}");
    let block_h = block_height(1, true);
    assert!((height - block_h - theme::SPACE_INLINE).abs() < 0.01, "one source line is one row: block {block_h}, advanced {height}");
    // the line is wider than the box and starts at the first column
    assert!(texts.iter().any(|p| p.text.starts_with("background")), "{:?}", texts.iter().map(|p| &p.text).collect::<Vec<_>>());
}

#[test]
fn a_long_line_scrolls_sideways_with_the_trackpad_and_stops_at_both_ends() {
    let mut b = Bench::new();
    let inner = block_inner(XML.lines().count(), true);
    let centre = Pos2::new(inner.center().x, inner.center().y);
    let (texts, _) = draw_code(&mut b, vec![Event::PointerMoved(centre)], Language::AndroidXml, XML);
    let start = left_of(&texts, "\"http");
    assert!(right_of(&texts, "\"http") > inner.right(), "the long line is wider than the box, so it has to scroll");

    // a sideways swipe moves the code by the swipe
    for _ in 0..2 {
        draw_code(&mut b, vec![wheel(Vec2::new(-60.0, 0.0))], Language::AndroidXml, XML);
    }
    for _ in 0..12 {
        draw_code(&mut b, vec![], Language::AndroidXml, XML);
    }
    let (texts, _) = draw_code(&mut b, vec![], Language::AndroidXml, XML);
    let moved = start - left_of(&texts, "\"http");
    assert!((moved - 120.0).abs() < 1.0, "two 60px swipes move the code 120px, moved {moved}");

    // a far swipe ends with the last character at the right edge of the text area
    for _ in 0..2 {
        draw_code(&mut b, vec![wheel(Vec2::new(-5000.0, 0.0))], Language::AndroidXml, XML);
    }
    for _ in 0..30 {
        draw_code(&mut b, vec![], Language::AndroidXml, XML);
    }
    let (texts, _) = draw_code(&mut b, vec![], Language::AndroidXml, XML);
    let end = right_of(&texts, "android\"");
    assert!((end - inner.right()).abs() < 1.0, "scrolled to the end, the line ends at {end} and the text area at {}", inner.right());

    // and a far swipe back stops at the first column
    for _ in 0..2 {
        draw_code(&mut b, vec![wheel(Vec2::new(5000.0, 0.0))], Language::AndroidXml, XML);
    }
    for _ in 0..30 {
        draw_code(&mut b, vec![], Language::AndroidXml, XML);
    }
    let (texts, _) = draw_code(&mut b, vec![], Language::AndroidXml, XML);
    assert!((left_of(&texts, "\"http") - start).abs() < 0.5, "back at the first column");
}

#[test]
fn only_a_mostly_sideways_swipe_over_the_code_scrolls_it() {
    let mut b = Bench::new();
    let inner = block_inner(XML.lines().count(), true);
    let centre = Pos2::new(inner.center().x, inner.center().y);
    let (texts, _) = draw_code(&mut b, vec![Event::PointerMoved(centre)], Language::AndroidXml, XML);
    let start = left_of(&texts, "\"http");
    // a mostly vertical swipe belongs to the panel, which scrolls up and down
    draw_code(&mut b, vec![wheel(Vec2::new(-6.0, -40.0))], Language::AndroidXml, XML);
    for _ in 0..12 {
        draw_code(&mut b, vec![], Language::AndroidXml, XML);
    }
    let (texts, _) = draw_code(&mut b, vec![], Language::AndroidXml, XML);
    assert!((left_of(&texts, "\"http") - start).abs() < 0.5, "a vertical swipe leaves the code where it is");
    // a sideways swipe with the pointer elsewhere leaves it too
    draw_code(&mut b, vec![Event::PointerMoved(Pos2::new(inner.center().x, inner.bottom() + 200.0))], Language::AndroidXml, XML);
    draw_code(&mut b, vec![wheel(Vec2::new(-60.0, 0.0))], Language::AndroidXml, XML);
    for _ in 0..12 {
        draw_code(&mut b, vec![], Language::AndroidXml, XML);
    }
    let (texts, _) = draw_code(&mut b, vec![], Language::AndroidXml, XML);
    assert!((left_of(&texts, "\"http") - start).abs() < 0.5, "the pointer is not over the code");
    // the same sideways swipe over the code does scroll it
    draw_code(&mut b, vec![Event::PointerMoved(centre)], Language::AndroidXml, XML);
    draw_code(&mut b, vec![wheel(Vec2::new(-60.0, 0.0))], Language::AndroidXml, XML);
    for _ in 0..12 {
        draw_code(&mut b, vec![], Language::AndroidXml, XML);
    }
    let (texts, _) = draw_code(&mut b, vec![], Language::AndroidXml, XML);
    assert!((start - left_of(&texts, "\"http") - 60.0).abs() < 1.0, "a sideways swipe over the code scrolls it");
}

#[test]
fn dragging_the_code_pans_it_sideways() {
    let mut b = Bench::new();
    let inner = block_inner(XML.lines().count(), true);
    let a = Pos2::new(inner.center().x, inner.center().y);
    let (texts, _) = draw_code(&mut b, vec![Event::PointerMoved(a)], Language::AndroidXml, XML);
    let start = left_of(&texts, "\"http");
    let press = |pos, pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    draw_code(&mut b, vec![press(a, true)], Language::AndroidXml, XML);
    let to = a - Vec2::new(50.0, 0.0);
    draw_code(&mut b, vec![Event::PointerMoved(a - Vec2::new(20.0, 0.0))], Language::AndroidXml, XML);
    draw_code(&mut b, vec![Event::PointerMoved(to)], Language::AndroidXml, XML);
    draw_code(&mut b, vec![press(to, false)], Language::AndroidXml, XML);
    let (texts, _) = draw_code(&mut b, vec![], Language::AndroidXml, XML);
    let moved = start - left_of(&texts, "\"http");
    assert!((moved - 50.0).abs() < 1.0, "dragging 50px pans the code 50px, moved {moved}");
}

#[test]
fn dragging_the_thumb_moves_it_with_the_pointer_and_scrolls_the_code_by_the_scaled_amount() {
    let mut b = Bench::new();
    let inner = block_inner(XML.lines().count(), true);
    let (texts, _) = draw_code(&mut b, vec![Event::PointerMoved(inner.center())], Language::AndroidXml, XML);
    // the widest line, in px, is the whole width of the code
    let column = texts.iter().find(|p| p.text == "FrameLayout").expect("the first tag is painted").rect.width() / "FrameLayout".len() as f32;
    let content_w = XML.lines().map(|l| l.chars().count()).max().unwrap() as f32 * column;
    // scroll a little first, so a thumb that runs the wrong way has room to show it
    draw_code(&mut b, vec![wheel(Vec2::new(-20.0, 0.0))], Language::AndroidXml, XML);
    let texts = settle(&mut b, 12);
    let before = left_of(&texts, "\"http");
    let thumb = recorded(&b, "dev-code-thumb").expect("a box with a hidden part shows a thumb");
    drag_code(&mut b, thumb.center(), Vec2::new(40.0, 0.0));
    let texts = settle(&mut b, 4);
    let moved_thumb = recorded(&b, "dev-code-thumb").unwrap().center().x - thumb.center().x;
    assert!((moved_thumb - 40.0).abs() < 1.0, "dragging the thumb 40px right moves it 40px right, moved {moved_thumb}");
    let scrolled = before - left_of(&texts, "\"http");
    let expected = 40.0 * content_w / inner.width();
    assert!((scrolled - expected).abs() < 1.5, "the code scrolls toward its end by the scaled amount {expected}, scrolled {scrolled}");
}

#[test]
fn the_thumb_has_a_grab_area_of_at_least_24px_and_dragging_elsewhere_still_pans() {
    let mut b = Bench::new();
    let inner = block_inner(XML.lines().count(), true);
    draw_code(&mut b, vec![Event::PointerMoved(inner.center())], Language::AndroidXml, XML);
    draw_code(&mut b, vec![wheel(Vec2::new(-20.0, 0.0))], Language::AndroidXml, XML);
    settle(&mut b, 12);
    let thumb = recorded(&b, "dev-code-thumb").expect("a thumb");
    let grab = recorded(&b, "dev-code-thumb-grab").expect("the thumb has its own grab area");
    assert!(grab.height() >= theme::TARGET_MIN && grab.width() >= theme::TARGET_MIN, "grab area {grab:?}");
    assert!(grab.contains_rect(thumb), "the grab area covers the thumb: {grab:?} vs {thumb:?}");
    // pressing at the top edge of the grab area, well above the 4px thumb, still takes the thumb
    drag_code(&mut b, Pos2::new(thumb.center().x, grab.top() + 1.0), Vec2::new(40.0, 0.0));
    settle(&mut b, 4);
    let moved = recorded(&b, "dev-code-thumb").unwrap().center().x - thumb.center().x;
    assert!((moved - 40.0).abs() < 1.0, "the thumb follows a drag that starts on its grab area, moved {moved}");
    // a drag on the code itself pans it by the pointer, as before
    let texts = settle(&mut b, 2);
    let panned_from = left_of(&texts, "\"http");
    drag_code(&mut b, Pos2::new(inner.center().x, inner.top() + 8.0), Vec2::new(30.0, 0.0));
    let texts = settle(&mut b, 4);
    assert!((left_of(&texts, "\"http") - (panned_from + 30.0)).abs() < 1.0, "dragging the code 30px right moves it 30px right");
}

#[test]
fn the_scroll_thumb_sits_on_the_four_pixel_rhythm() {
    let mut b = Bench::new();
    draw_code(&mut b, vec![], Language::AndroidXml, XML);
    let block = recorded(&b, "dev-code-block").unwrap();
    let thumb = recorded(&b, "dev-code-thumb").expect("a thumb");
    let last_row_bottom = block.top() + theme::INSET_CONTROL + XML.lines().count() as f32 * theme::CODE_LINE_HEIGHT;
    let (above, below) = (thumb.top() - last_row_bottom, block.bottom() - thumb.bottom());
    assert_eq!((above, below), (theme::SPACE_FACT, theme::SPACE_FACT), "gaps above and below the thumb, in px");
    assert_eq!(thumb.height(), theme::SPACE_FACT);
}

#[test]
fn code_that_fits_does_not_scroll() {
    let mut b = Bench::new();
    let code = "width: 12px;";
    let inner = block_inner(1, false);
    let (texts, _) = draw_code(&mut b, vec![Event::PointerMoved(inner.center())], Language::Css, code);
    let start = left_of(&texts, "width");
    draw_code(&mut b, vec![wheel(Vec2::new(-80.0, 0.0))], Language::Css, code);
    for _ in 0..12 {
        draw_code(&mut b, vec![], Language::Css, code);
    }
    let (texts, _) = draw_code(&mut b, vec![], Language::Css, code);
    assert!((left_of(&texts, "width") - start).abs() < 0.5);
}

#[test]
fn a_tab_in_the_code_takes_four_columns_and_never_overlaps_the_next_token() {
    let mut b = Bench::new();
    let (texts, _) = draw_code(&mut b, vec![], Language::Css, "content: \"x\ty\";");
    let string = texts.iter().find(|p| p.text.starts_with("\"x")).expect("the string is painted");
    let semicolon = texts.iter().find(|p| p.text == ";").expect("the semicolon is painted");
    assert!(
        semicolon.rect.left() >= string.rect.right() - 0.5,
        "the string {:?} runs to {} but the semicolon starts at {}",
        string.text,
        string.rect.right(),
        semicolon.rect.left()
    );
}

#[test]
fn a_line_with_tabs_scrolls_to_its_last_character() {
    let mut b = Bench::new();
    let code = "/* a comment wider than the box, with a\ttab and\tanother inside it */";
    let inner = block_inner(1, true);
    let draw = |b: &mut Bench, events| draw_code(b, events, Language::Css, code).0;
    draw(&mut b, vec![Event::PointerMoved(inner.center())]);
    for _ in 0..2 {
        draw(&mut b, vec![wheel(Vec2::new(-5000.0, 0.0))]);
    }
    for _ in 0..30 {
        draw(&mut b, vec![]);
    }
    let texts = draw(&mut b, vec![]);
    let end = right_of(&texts, "inside it */");
    assert!((end - inner.right()).abs() < 1.0, "scrolled to the end, the line ends at {end} and the text area at {}", inner.right());
}

// ---- the real Inspect panel

fn dev_inspector(lang: &str) -> Sim {
    Sim::with(Options { select: Some("Card".into()), dev: Some(lang.into()), ..Default::default() })
}

fn scroll_dev_inspector_to(s: &mut Sim, scroll: f32) {
    let id = egui::Id::new("inspect-scroll");
    let (_, content_h) = s.ctx.data(|d| d.get_temp::<(f32, f32)>(id)).unwrap_or((0.0, 0.0));
    s.ctx.data_mut(|d| d.insert_temp(id, (scroll, content_h)));
    s.frame(vec![]);
    s.frame(vec![]);
}

#[test]
fn the_help_button_never_covers_code_or_any_other_inspector_text() {
    let mut s = dev_inspector("android-xml");
    // a long line to scroll and a short window of the panel at every scroll position, sideways scroll included
    for scroll in [0.0, 300.0, 900.0, 1.0e6] {
        scroll_dev_inspector_to(&mut s, scroll);
        let help = s.rect_of("help-button");
        for p in painted(&s.shapes) {
            let seen = p.visible();
            assert!(
                seen.width() <= 0.0 || seen.height() <= 0.0 || !seen.intersects(help),
                "scroll {scroll}: {:?} at {seen:?} runs under the help button at {help:?}",
                p.text
            );
        }
    }
}

#[test]
fn the_dev_inspector_shows_the_code_on_one_row_per_line() {
    let mut s = dev_inspector("android-xml");
    let card = s.by_name("Card");
    let code = s.app().dev_code_for(card, Language::AndroidXml, false);
    scroll_dev_inspector_to(&mut s, 1.0e6);
    let block = s.rect_of("dev-code-block");
    let expected = block_height(code.lines().count(), true);
    assert!((block.height() - expected).abs() < 0.01, "{} lines of code in a box {}px tall, expected {expected}px", code.lines().count(), block.height());
}

// ---- Colors

fn colors_app(prepare: impl FnOnce(&mut Document, designos_doc::NodeId)) -> (App, designos_doc::NodeId) {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let f = d.alloc_id();
    let f = d.insert(page, Node::frame(f, DRect::new(0.0, 0.0, 200.0, 100.0))).unwrap();
    prepare(&mut d, f);
    let mut app = App::new(d);
    app.ui.dev_mode = true;
    app.selection = vec![f];
    (app, f)
}

fn draw_inspector(b: &mut Bench, app: &mut App) -> Vec<Painted> {
    let region = Rect::from_min_size(Pos2::ZERO, Vec2::new(PANEL_W, 8000.0));
    let mut shapes = Vec::new();
    for _ in 0..3 {
        shapes = b.frame(vec![], |ui| inspect::show(app, ui, region));
    }
    painted(&shapes)
}

#[test]
fn colors_list_every_used_color_with_its_hex() {
    let (mut app, f) = colors_app(|d, f| {
        d.get_mut(f).unwrap().fills = vec![Paint::solid(Color::rgb(255, 255, 255))];
        d.get_mut(f).unwrap().stroke = DStroke { weight: 1.0, paints: vec![Paint::solid(Color::rgb(18, 52, 86))], ..Default::default() };
        let t = d.alloc_id();
        let mut text = Node::text(t, DRect::new(10.0, 10.0, 80.0, 20.0), "Hello", TextStyle::default());
        text.name = "Label".into();
        d.insert(f, text).unwrap();
    });
    let mut b = Bench::new();
    let texts = draw_inspector(&mut b, &mut app);
    let used = inspect::collect_colors(&app.doc, f);
    assert!(used.len() >= 2, "{used:?}");
    assert!(texts.iter().any(|p| p.text == "Colors"), "the section is there when the layer uses colors");
    for e in &used {
        assert!(texts.iter().any(|p| p.text == e.hex()), "{} is used but not listed", e.hex());
    }
}

#[test]
fn colors_of_an_instance_list_what_its_component_paints() {
    let (mut app, _) = colors_app(|d, f| {
        let page = d.get(f).unwrap().parent.unwrap();
        let comp = d.alloc_id();
        let comp = d.insert(page, Node::new(comp, NodeKind::Component(designos_doc::FrameProps::default()))).unwrap();
        let swatch = d.alloc_id();
        let mut swatch = Node::rectangle(swatch, DRect::new(0.0, 0.0, 20.0, 20.0));
        swatch.fills = vec![Paint::solid(Color::rgb(220, 20, 60))];
        d.insert(comp, swatch).unwrap();
        designos_doc::create_instance(d, comp, page, DV::new(300.0, 0.0)).apply(d).unwrap();
    });
    let instance = app.doc.nodes.values().find(|n| matches!(n.kind, NodeKind::Instance { .. })).map(|n| n.id).expect("an instance");
    app.selection = vec![instance];
    let mut b = Bench::new();
    let texts = draw_inspector(&mut b, &mut app);
    assert!(texts.iter().any(|p| p.text == "Colors"));
    assert!(texts.iter().any(|p| p.text == "#DC143C"), "the instance paints crimson through its component");
}

#[test]
fn colors_are_absent_when_the_layer_uses_none() {
    let (mut app, _) = colors_app(|d, f| {
        d.get_mut(f).unwrap().fills.clear();
        d.get_mut(f).unwrap().stroke = DStroke::default();
    });
    let mut b = Bench::new();
    let texts = draw_inspector(&mut b, &mut app);
    assert!(inspect::collect_colors(&app.doc, app.selection[0]).is_empty());
    assert!(!texts.iter().any(|p| p.text == "Colors"), "no Colors title without colors");
    assert!(!texts.iter().any(|p| p.text == "Hex"), "no color format picker without colors");
}

// ---- the MCP prompt

const PROMPT_HEADLINE: &str = "Implement this design from design:os.";

#[test]
fn the_mcp_prompt_shows_in_full_inside_its_box() {
    let (mut app, f) = colors_app(|d, f| {
        d.get_mut(f).unwrap().name = "A frame with a rather long name for the settings dialog".into();
    });
    let mut b = Bench::new();
    let texts = draw_inspector(&mut b, &mut app);
    let uri = format!("@designos://design/{}/A-frame-with-a-rather-long-name-for-the-settings-dialog", f.0);
    let limit = PANEL_W - PAD;
    let mut seen = String::new();
    for p in texts.iter().filter(|p| p.text.contains(PROMPT_HEADLINE) || p.text.contains("@designos://")) {
        assert!(!p.text.contains('…'), "the prompt is cut short with an ellipsis: {:?}", p.text);
        assert!(p.rect.right() <= limit + 0.5, "{:?} runs to {} past the box edge {limit}", p.text, p.rect.right());
        seen.push_str(&p.text);
    }
    assert!(seen.contains(PROMPT_HEADLINE), "the first line of the prompt is missing: {seen:?}");
    assert!(seen.replace('\n', "").contains(&uri), "the link line of the prompt is not whole: {seen:?}");
}
