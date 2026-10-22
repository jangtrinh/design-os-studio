//! A box that hugs its text holds the glyphs on both renderers, also when a face arrives after the
//! box was measured. Own test binary: it registers a font, which bumps a process-wide generation.

mod common;

use common::*;
use designos_doc::*;
use designos_render::tiny_skia::Pixmap;
use designos_render::{render_page, text_geometry, Viewport};

/// Device pixels per document pixel: ink edges are read to a quarter pixel.
const DPR: u32 = 4;
/// What the brief allows beyond the box, in document pixels.
const TOLERANCE: f32 = 0.5;
const LEFT: f32 = 20.0;
const TOP: f32 = 20.0;

/// The bundled Roboto Mono, named `family` (11 characters): every glyph 0.6 em wide, far wider than
/// the Inter that stands in for a family that is not installed.
fn wide_face(family: &str) -> Vec<u8> {
    let mut data = include_bytes!("../../designos-fonts/assets/roboto-mono/RobotoMono[wght].ttf").to_vec();
    let utf16 = |s: &str| s.encode_utf16().flat_map(u16::to_be_bytes).collect::<Vec<u8>>();
    for (a, b) in [(b"Roboto Mono".to_vec(), family.as_bytes().to_vec()), (utf16("Roboto Mono"), utf16(family))] {
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
    data
}

fn hug_props(content: &str, family: &str) -> TextProps {
    TextProps {
        content: content.into(),
        auto_resize: TextAutoResize::WidthAndHeight,
        style: TextStyle { font_family: family.into(), font_size: 16.0, fills: vec![Paint::solid(Color::BLACK)], ..Default::default() },
        ..Default::default()
    }
}

/// The box a hugging text takes: its measured extent, rounded up, as the editor stores it.
fn hug_box(props: &TextProps) -> Vec2 {
    let g = text_geometry(props, Vec2::ZERO);
    Vec2::new(g.width.ceil().max(1.0), g.height.ceil().max(1.0))
}

/// A white page with one text of box `size` at (`LEFT`, `TOP`).
fn scene(props: &TextProps, size: Vec2) -> (Document, NodeId) {
    let mut t = T::new();
    t.add(None, |id| {
        let mut n = Node::text(id, Rect::new(LEFT, TOP, size.x, size.y), props.content.clone(), props.style.clone());
        n.kind = NodeKind::Text(props.clone());
        n
    });
    (t.doc, t.page)
}

fn viewport() -> Viewport {
    Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: DPR as f32 }
}

fn canvas_size(size: Vec2) -> (u32, u32) {
    (((size.x + 2.0 * LEFT) as u32) * DPR, ((size.y + 2.0 * TOP) as u32) * DPR)
}

/// Left and right edge, in document pixels, of every pixel that is not the white page.
fn ink_span(pm: &Pixmap) -> (f32, f32) {
    let (mut l, mut r) = (u32::MAX, 0);
    for y in 0..pm.height() {
        for x in 0..pm.width() {
            let p = pm.pixel(x, y).unwrap();
            if p.red() < 250 || p.green() < 250 || p.blue() < 250 {
                l = l.min(x);
                r = r.max(x + 1);
            }
        }
    }
    (l as f32 / DPR as f32, r as f32 / DPR as f32)
}

fn cpu_ink(doc: &Document, page: NodeId, size: Vec2) -> (f32, f32) {
    let (w, h) = canvas_size(size);
    let mut pm = Pixmap::new(w, h).unwrap();
    render_page(doc, page, &viewport(), &mut pm);
    ink_span(&pm)
}

fn assert_inside(who: &str, ink: (f32, f32), size: Vec2) {
    let (left, right) = (LEFT - TOLERANCE, LEFT + size.x + TOLERANCE);
    assert!(ink.0 >= left && ink.1 <= right, "{who}: ink {:.2}..{:.2} must sit inside the box {:.2}..{:.2} (+-{TOLERANCE})", ink.0, ink.1, LEFT, LEFT + size.x);
}

const TWO_LINES: &str = "Hello brave world\nsecond line here";
const NARROW: &str = "iiiiiiiiiiiiiiii\nllllllllll";

#[test]
fn a_two_line_hug_box_holds_its_glyphs_on_the_cpu_and_the_gpu() {
    let props = hug_props(TWO_LINES, "Inter");
    let size = hug_box(&props);
    let (doc, page) = scene(&props, size);
    assert_inside("cpu", cpu_ink(&doc, page, size), size);
    let Some(mut g) = gpu() else { return };
    let frame = g.compile(&doc, page, 1, &viewport(), canvas_size(size));
    assert_inside("gpu", ink_span(&g.render_frame(&frame, canvas_size(size))), size);
}

#[test]
fn a_hug_box_holds_the_stand_in_glyphs_and_then_the_glyphs_of_the_face_that_arrives() {
    let family = "TestHugBox1";
    let props = hug_props(NARROW, family);
    let mut gpu = gpu();
    let mut rev = 0;
    let mut spans = Vec::new();
    for stage in ["stand-in", "arrived"] {
        if stage == "arrived" {
            designos_render::register_font(wide_face(family));
        }
        // the editor re-measures the box once the face is there, then draws
        let size = hug_box(&props);
        let (doc, page) = scene(&props, size);
        let cpu = cpu_ink(&doc, page, size);
        assert_inside(&format!("{stage} cpu"), cpu, size);
        if let Some(g) = gpu.as_mut() {
            // one revision per frame, as the canvas does after a font arrives (`App::changed`)
            rev += 1;
            let frame = g.compile(&doc, page, rev, &viewport(), canvas_size(size));
            let on_gpu = ink_span(&g.render_frame(&frame, canvas_size(size)));
            assert_inside(&format!("{stage} gpu"), on_gpu, size);
            assert!((on_gpu.1 - cpu.1).abs() <= 1.0, "{stage}: the GPU ends the ink at {:.2}, the CPU at {:.2}", on_gpu.1, cpu.1);
        }
        spans.push((size, cpu));
    }
    assert!(spans[1].0.x > spans[0].0.x + 20.0, "the box grew with the face: {:?} then {:?}", spans[0].0, spans[1].0);
    assert!(spans[1].1 .1 > spans[0].1 .1 + 20.0, "the glyphs on screen changed with the face: {:?} then {:?}", spans[0].1, spans[1].1);
}
