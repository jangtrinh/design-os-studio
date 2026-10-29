//! Wrapped Fill-width text must take the height of its solved width after every layout pass.
//!
//! The H field (`widgets::fmt_num` on `n.size.y`) and the canvas badge (`n.size.y.round()` in
//! `canvas/draw.rs`) both read the stored size, so they cannot disagree with each other. They
//! disagree with the drawn lines when auto layout writes a new width and keeps the old height.

use super::*;
use crate::textmeasure;
use crate::widgets::fmt_num;
use designos_doc::{AutoLayout, FrameProps, LayoutChild, Node, Rect, TextStyle, TextWrap};

const TITLE: &str = "Title";
const P_SHORT: &str = "Night falls.";
const P_MED: &str = "The quick brown fox jumps over the lazy dog and keeps running across the field until nightfall.";
const P_LONG: &str = "The quick brown fox jumps over the lazy dog and keeps running across the field until nightfall, then turns back at the fence and tells the whole story again in a quieter voice so nobody is left waiting.";

struct Fixture {
    doc: Document,
    frame: NodeId,
    title: NodeId,
    short: NodeId,
    med: NodeId,
    long: NodeId,
}

fn style(size: f32) -> TextStyle {
    TextStyle { font_family: "Inter".into(), font_size: size, ..Default::default() }
}

fn needed_h(t: &TextProps, width: f32) -> f32 {
    textmeasure::measure(t, Some(width.max(1.0))).y
}

fn hug_size(t: &TextProps) -> Vec2 {
    textmeasure::measure(t, None)
}

fn h_field(n: &Node) -> String {
    fmt_num(n.size.y)
}

fn canvas_badge(n: &Node) -> String {
    format!("{} × {}", n.size.x.round() as i64, n.size.y.round() as i64)
}

fn text_node(d: &mut Document, parent: NodeId, content: &str, font: f32, mode: TextAutoResize, fill_width: bool, size: Vec2) -> NodeId {
    let id = d.alloc_id();
    let mut n = Node::text(id, Rect::new(0.0, 0.0, size.x, size.y), content, style(font));
    if let NodeKind::Text(t) = &mut n.kind {
        t.auto_resize = mode;
    }
    if fill_width {
        n.layout_child.horizontal = Sizing::Fill;
    }
    d.insert(parent, n).unwrap()
}

fn column(height_hug: bool) -> AutoLayout {
    AutoLayout {
        direction: LayoutDirection::Vertical,
        item_spacing: 8.0,
        padding: [10.0; 4],
        primary_sizing: if height_hug { Sizing::Hug } else { Sizing::Fixed },
        counter_sizing: Sizing::Fixed,
        layout_version: 1,
        ..AutoLayout::default()
    }
}

fn build(width: f32, height_hug: bool) -> Fixture {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let fid = d.alloc_id();
    let mut f = Node::frame(fid, Rect::new(0.0, 0.0, width, 40.0));
    f.kind = NodeKind::Frame(FrameProps { auto_layout: Some(column(height_hug)), ..Default::default() });
    f.layout_child = LayoutChild { horizontal: Sizing::Fixed, vertical: if height_hug { Sizing::Hug } else { Sizing::Fixed }, ..Default::default() };
    let frame = d.insert(page, f).unwrap();
    let title = text_node(
        &mut d,
        frame,
        TITLE,
        20.0,
        TextAutoResize::WidthAndHeight,
        false,
        hug_size(&TextProps { content: TITLE.into(), style: style(20.0), auto_resize: TextAutoResize::WidthAndHeight, ..Default::default() }),
    );
    let stale = v2(20.0, 20.0);
    let short = text_node(&mut d, frame, P_SHORT, 14.0, TextAutoResize::Height, true, stale);
    let med = text_node(&mut d, frame, P_MED, 14.0, TextAutoResize::Height, true, stale);
    let long = text_node(&mut d, frame, P_LONG, 14.0, TextAutoResize::Height, true, stale);
    Fixture { doc: d, frame, title, short, med, long }
}

fn layout(f: &mut Fixture) {
    layout_node(&mut f.doc, &mut Edit::new(), f.frame);
}

fn snapshot(f: &Fixture) -> Vec<(f32, f32)> {
    [f.frame, f.title, f.short, f.med, f.long]
        .into_iter()
        .map(|id| {
            let n = f.doc.get(id).unwrap();
            (n.size.x, n.size.y)
        })
        .collect()
}

fn body_of(f: &Fixture, id: NodeId) -> &TextProps {
    match &f.doc.get(id).unwrap().kind {
        NodeKind::Text(t) => t,
        _ => unreachable!("text child"),
    }
}

fn assert_wrapped_matches_lines(f: &Fixture, id: NodeId, label: &str) {
    let n = f.doc.get(id).unwrap();
    let t = body_of(f, id);
    let need = needed_h(t, n.size.x);
    let lines = textmeasure::geometry(t, v2(n.size.x, need)).lines.len();
    assert!((n.size.y - need).abs() < 1e-3, "{label}: width {} height {} need {} lines {lines}", n.size.x, n.size.y, need);
    assert!(lines >= 1, "{label}: no drawn lines");
}

fn assert_h_field_matches_badge(f: &Fixture, id: NodeId, label: &str) {
    let n = f.doc.get(id).unwrap();
    assert_eq!(h_field(n), fmt_num(n.size.y), "{label}: H field reads n.size.y");
    assert_eq!(canvas_badge(n), format!("{} × {}", n.size.x.round() as i64, n.size.y.round() as i64), "{label}: badge reads n.size");
}

fn assert_hug_frame_fits_content(f: &Fixture) {
    let frame = f.doc.get(f.frame).unwrap();
    let title = f.doc.get(f.title).unwrap().size.y;
    let short = f.doc.get(f.short).unwrap().size.y;
    let med = f.doc.get(f.med).unwrap().size.y;
    let long = f.doc.get(f.long).unwrap().size.y;
    let need = 10.0 + title + 8.0 + short + 8.0 + med + 8.0 + long + 10.0;
    assert!((frame.size.y - need).abs() < 1e-3, "frame height {} need {need}", frame.size.y);
}

fn assert_bodies_fit(f: &Fixture) {
    for (id, label) in [(f.short, "short"), (f.med, "med"), (f.long, "long")] {
        assert_wrapped_matches_lines(f, id, label);
        assert_h_field_matches_badge(f, id, label);
    }
    assert_h_field_matches_badge(f, f.frame, "frame");
}

fn set_frame_width(f: &mut Fixture, width: f32) {
    f.doc.get_mut(f.frame).unwrap().size.x = width;
}

/// The three Fill-width paragraphs at one frame width: reports the measured heights in the
/// assertion so a red run on main records what this box actually measures (PM saw ~59 / 136 / 39).
fn assert_bodies_at_width(f: &Fixture, inner: f32) {
    let got = [f.short, f.med, f.long].map(|id| {
        let n = f.doc.get(id).unwrap();
        (n.size.x, n.size.y, needed_h(body_of(f, id), n.size.x))
    });
    assert!(
        got.iter().all(|(w, h, need)| (*w - inner).abs() < 1e-3 && (*h - *need).abs() < 1e-3),
        "inner {inner}: (width, height, need) short={:?} med={:?} long={:?}",
        got[0],
        got[1],
        got[2]
    );
}

#[test]
fn wrapped_fill_text_takes_the_height_of_its_solved_width_when_narrowed_and_widened() {
    let mut f = build(300.0, true);
    layout(&mut f);
    assert_bodies_at_width(&f, 280.0);
    assert_bodies_fit(&f);
    assert_hug_frame_fits_content(&f);

    set_frame_width(&mut f, 140.0);
    layout(&mut f);
    assert_bodies_at_width(&f, 120.0);
    assert_bodies_fit(&f);
    assert_hug_frame_fits_content(&f);

    set_frame_width(&mut f, 420.0);
    layout(&mut f);
    assert_bodies_at_width(&f, 400.0);
    assert_bodies_fit(&f);
    assert_hug_frame_fits_content(&f);
}

#[test]
fn a_second_layout_pass_changes_nothing() {
    let mut f = build(140.0, true);
    layout(&mut f);
    let first = snapshot(&f);
    layout(&mut f);
    assert_eq!(snapshot(&f), first);
    layout(&mut f);
    layout(&mut f);
    layout(&mut f);
    assert_eq!(snapshot(&f), first, "five passes must not oscillate");
}

#[test]
fn typing_the_w_field_refits_wrapped_fill_text() {
    let mut f = build(300.0, true);
    layout(&mut f);
    let t = f.doc.get(f.frame).unwrap().transform;
    let h = f.doc.get(f.frame).unwrap().size.y;
    crate::ops::apply_resize(&mut f.doc, &mut Edit::new(), f.frame, v2(140.0, h), t, true);
    assert_bodies_at_width(&f, 120.0);
    assert_bodies_fit(&f);
    assert_hug_frame_fits_content(&f);
}

#[test]
fn nested_auto_layout_hug_height_follows_the_inner_wrapped_text() {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let outer_id = d.alloc_id();
    let mut outer = Node::frame(outer_id, Rect::new(0.0, 0.0, 300.0, 40.0));
    outer.kind = NodeKind::Frame(FrameProps { auto_layout: Some(column(true)), ..Default::default() });
    let outer = d.insert(page, outer).unwrap();
    let _title = text_node(
        &mut d,
        outer,
        TITLE,
        20.0,
        TextAutoResize::WidthAndHeight,
        false,
        hug_size(&TextProps { content: TITLE.into(), style: style(20.0), auto_resize: TextAutoResize::WidthAndHeight, ..Default::default() }),
    );
    let inner_id = d.alloc_id();
    let mut inner = Node::frame(inner_id, Rect::new(0.0, 0.0, 20.0, 20.0));
    inner.kind = NodeKind::Frame(FrameProps {
        auto_layout: Some(AutoLayout {
            direction: LayoutDirection::Vertical,
            item_spacing: 4.0,
            padding: [8.0; 4],
            primary_sizing: Sizing::Hug,
            counter_sizing: Sizing::Fixed,
            layout_version: 1,
            ..AutoLayout::default()
        }),
        ..Default::default()
    });
    inner.layout_child.horizontal = Sizing::Fill;
    let inner = d.insert(outer, inner).unwrap();
    let body = text_node(&mut d, inner, P_MED, 14.0, TextAutoResize::Height, true, v2(20.0, 20.0));
    layout_node(&mut d, &mut Edit::new(), outer);
    d.get_mut(outer).unwrap().size.x = 140.0;
    layout_node(&mut d, &mut Edit::new(), outer);

    let body_n = d.get(body).unwrap();
    let t = match &body_n.kind {
        NodeKind::Text(t) => t,
        _ => unreachable!(),
    };
    let need = needed_h(t, body_n.size.x);
    assert!((body_n.size.x - 104.0).abs() < 1e-3, "inner padding 8+8, outer padding 10+10, width 140 -> body 104, got {}", body_n.size.x);
    assert!((body_n.size.y - need).abs() < 1e-3, "nested body height {} need {need}", body_n.size.y);
    let inner_n = d.get(inner).unwrap();
    let inner_need = 8.0 + need + 8.0;
    assert!((inner_n.size.y - inner_need).abs() < 1e-3, "inner hug {} need {inner_need}", inner_n.size.y);
    let outer_n = d.get(outer).unwrap();
    let title_h = d.get(_title).unwrap().size.y;
    let outer_need = 10.0 + title_h + 8.0 + inner_n.size.y + 10.0;
    assert!((outer_n.size.y - outer_need).abs() < 1e-3, "outer hug {} need {outer_need}", outer_n.size.y);
}

#[test]
fn fractional_frame_width_still_matches_the_measured_lines() {
    let mut f = build(140.25, true);
    layout(&mut f);
    assert_bodies_fit(&f);
    assert_hug_frame_fits_content(&f);
    let med = f.doc.get(f.med).unwrap();
    assert!((med.size.x - 120.25).abs() < 1e-3, "fill width {}", med.size.x);
}

#[test]
fn a_fixed_height_frame_still_refits_wrapped_children() {
    let mut f = build(140.0, false);
    f.doc.get_mut(f.frame).unwrap().size.y = 400.0;
    layout(&mut f);
    assert_bodies_at_width(&f, 120.0);
    assert_bodies_fit(&f);
    assert!((f.doc.get(f.frame).unwrap().size.y - 400.0).abs() < 1e-3, "fixed height held");
}

#[test]
fn editing_wrapped_text_refits_its_height() {
    let mut f = build(300.0, true);
    layout(&mut f);
    {
        let n = f.doc.get_mut(f.med).unwrap();
        if let NodeKind::Text(t) = &mut n.kind {
            t.content = P_LONG.into();
        }
    }
    layout(&mut f);
    assert_wrapped_matches_lines(&f, f.med, "edited");
    assert_hug_frame_fits_content(&f);
}

#[test]
fn toggling_wrap_mode_refits_the_box() {
    let mut f = build(140.0, true);
    layout(&mut f);
    {
        let n = f.doc.get_mut(f.long).unwrap();
        if let NodeKind::Text(t) = &mut n.kind {
            t.text_wrap = TextWrap::Balance;
        }
    }
    layout(&mut f);
    assert_wrapped_matches_lines(&f, f.long, "balance");
    {
        let n = f.doc.get_mut(f.long).unwrap();
        if let NodeKind::Text(t) = &mut n.kind {
            t.text_wrap = TextWrap::Auto;
        }
    }
    layout(&mut f);
    assert_wrapped_matches_lines(&f, f.long, "auto");
    assert_hug_frame_fits_content(&f);
}

#[test]
fn layout_after_a_late_font_matches_the_new_measure() {
    const FAMILY: &str = "Test856Fn01";
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let fid = d.alloc_id();
    let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 140.0, 40.0));
    frame.kind = NodeKind::Frame(FrameProps { auto_layout: Some(column(true)), ..Default::default() });
    let frame = d.insert(page, frame).unwrap();
    let id = d.alloc_id();
    let mut n = Node::text(id, Rect::new(0.0, 0.0, 20.0, 20.0), P_MED, TextStyle { font_family: FAMILY.into(), font_size: 14.0, ..Default::default() });
    if let NodeKind::Text(t) = &mut n.kind {
        t.auto_resize = TextAutoResize::Height;
    }
    n.layout_child.horizontal = Sizing::Fill;
    let body = d.insert(frame, n).unwrap();

    layout_node(&mut d, &mut Edit::new(), frame);
    let before = d.get(body).unwrap().size;
    let t_before = match &d.get(body).unwrap().kind {
        NodeKind::Text(t) => t.clone(),
        _ => unreachable!(),
    };
    let need_before = needed_h(&t_before, before.x);
    assert!((before.y - need_before).abs() < 1e-3, "fallback height {} need {need_before}", before.y);

    designos_render::register_font(crate::fonts::testkit::font_named(FAMILY));
    layout_node(&mut d, &mut Edit::new(), frame);
    let after = d.get(body).unwrap().size;
    let t_after = match &d.get(body).unwrap().kind {
        NodeKind::Text(t) => t.clone(),
        _ => unreachable!(),
    };
    let need_after = needed_h(&t_after, after.x);
    assert!((after.y - need_after).abs() < 1e-3, "arrived height {} need {need_after}", after.y);
    assert!((need_after - need_before).abs() > 1.0, "the test face must change the wrap height (fallback {need_before} arrived {need_after})");
}
