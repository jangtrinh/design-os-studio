//! A face that arrives after text was measured or drawn. Own test binary: registering fonts bumps a
//! process-wide generation that would invalidate caches used by tests running in parallel.
//! Every test names its own family, so the registrations of one never change another's result.

use designos_doc::*;
use designos_render::{register_font, text_geometry, text_outlines};
use std::sync::mpsc::channel;

/// Only the bundled faces, so nothing depends on what the host has installed.
fn hermetic_fonts() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| std::env::set_var("DESIGNOS_BUNDLED_FONTS_ONLY", "1"));
}

/// Swaps `from` for `to` (same length) in every ASCII and UTF-16BE spelling inside `data`.
fn swap_name(data: &mut [u8], from: &str, to: &str) {
    assert_eq!(from.len(), to.len(), "the name is swapped in place");
    let utf16 = |s: &str| s.encode_utf16().flat_map(u16::to_be_bytes).collect::<Vec<u8>>();
    for (a, b) in [(from.as_bytes().to_vec(), to.as_bytes().to_vec()), (utf16(from), utf16(to))] {
        let mut i = 0;
        while i + a.len() <= data.len() {
            if data[i..i + a.len()] == a[..] {
                data[i..i + a.len()].copy_from_slice(&b);
                i += a.len();
            } else {
                i += 1;
            }
        }
    }
}

/// The bundled Roboto Mono, which names itself `family` (11 characters). Every glyph is 0.6 em wide,
/// far wider than the narrow letters of the Inter that stands in for a family that is not installed.
fn wide_face(family: &str) -> Vec<u8> {
    let mut data = designos_fonts::bundled().iter().find(|a| a.family == "Roboto Mono").expect("Roboto Mono is bundled").data.to_vec();
    swap_name(&mut data, "Roboto Mono", family);
    data
}

fn hug(content: &str, family: &str) -> TextProps {
    TextProps {
        content: content.into(),
        auto_resize: TextAutoResize::WidthAndHeight,
        style: TextStyle { font_family: family.into(), font_size: 16.0, ..Default::default() },
        ..Default::default()
    }
}

const NARROW: &str = "iiiiiiiiiiiiiiii\nllllllllll";

/// Left and right edge of every glyph the renderer draws for `props` laid out in `size`.
fn ink_span(props: &TextProps, size: Vec2) -> (f32, f32) {
    let (mut l, mut r) = (f32::MAX, f32::MIN);
    for o in text_outlines(props, size) {
        let b = o.path.bounds();
        l = l.min(b.left());
        r = r.max(b.right());
    }
    (l, r)
}

/// The width a hugging box takes, as the editor measures it.
fn hug_width(props: &TextProps) -> f32 {
    text_geometry(props, Vec2::ZERO).width.ceil().max(1.0)
}

#[test]
fn what_the_thread_measures_follows_a_face_that_arrives() {
    hermetic_fonts();
    let p = hug(NARROW, "TestWiderA1");
    let before = hug_width(&p);
    register_font(wide_face("TestWiderA1"));
    let after = hug_width(&p);
    assert!(after > before + 20.0, "the box was measured with the stand-in ({before}) and stays that wide after the face arrives ({after})");
}

#[test]
fn a_long_lived_thread_draws_with_a_face_registered_after_it_cached_the_text() {
    hermetic_fonts();
    let p = hug(NARROW, "TestWiderB1");
    let (to_worker, from_main) = channel::<()>();
    let (to_main, from_worker) = channel::<(f32, f32, f32)>();
    let worker_props = p.clone();
    // a tile worker: it lives across the registration and keeps its own caches
    let worker = std::thread::spawn(move || {
        let w0 = hug_width(&worker_props);
        let r0 = ink_span(&worker_props, Vec2::new(w0, 40.0)).1;
        to_main.send((w0, r0, 0.0)).unwrap();
        from_main.recv().unwrap();
        let w1 = hug_width(&worker_props);
        let r1 = ink_span(&worker_props, Vec2::new(w1, 40.0)).1;
        to_main.send((w1, r1, 0.0)).unwrap();
    });
    let (w0, r0, _) = from_worker.recv().unwrap();
    register_font(wide_face("TestWiderB1"));
    to_worker.send(()).unwrap();
    let (w1, r1, _) = from_worker.recv().unwrap();
    worker.join().unwrap();
    assert!(w1 > w0 + 20.0, "the worker keeps measuring with the stand-in: {w0} then {w1}");
    assert!(r1 > r0 + 20.0, "the worker keeps drawing the stand-in glyphs: ink right {r0} then {r1}");
    assert!(r1 <= w1 + 0.5, "the glyphs it draws fit the box it measures: ink right {r1}, box {w1}");
}

#[test]
fn a_hugging_box_holds_its_glyphs_before_and_after_a_face_arrives() {
    hermetic_fonts();
    let p = hug(NARROW, "TestWiderC1");
    for stage in ["fallback", "arrived"] {
        if stage == "arrived" {
            register_font(wide_face("TestWiderC1"));
        }
        let w = hug_width(&p);
        let (l, r) = ink_span(&p, Vec2::new(w, 40.0));
        assert!(l >= -0.5 && r <= w + 0.5, "{stage}: ink {l}..{r} must sit inside the box 0..{w}");
    }
}
