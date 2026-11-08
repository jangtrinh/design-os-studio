//! In a Display P3 document the eyedropper, the panel swatches and the scoped agent canvas read the
//! document's colours the way the canvas draws them.

use super::*;
use crate::panels::properties::sample_page;
use designos_doc::{Color as DColor, ColorProfile, Document, ImageParams, ImageScaleMode, Node as DNode, Paint, PaintKind, Rect as DRect};

const MID: DColor = DColor { r: 0.5, g: 0.4, b: 0.8, a: 1.0 };

fn doc_with_fill(fill: Paint) -> (Document, NodeId, NodeId) {
    let mut doc = Document::new("t");
    let page = doc.add_page("p");
    doc.get_mut(page).unwrap().kind = NodeKind::Page { background: DColor::WHITE };
    let id = doc.alloc_id();
    let mut n = DNode::rectangle(id, DRect::new(0.0, 0.0, 40.0, 40.0));
    n.fills = vec![fill];
    let id = doc.insert(page, n).unwrap();
    doc.color_profile = ColorProfile::DisplayP3;
    (doc, page, id)
}

fn pick_and_apply(doc: &mut Document, page: NodeId, id: NodeId, rounds: usize) -> Vec<[u8; 3]> {
    let mut seen = Vec::new();
    for _ in 0..rounds {
        let [r, g, b] = sample_page(doc, page, designos_doc::Vec2::new(20.0, 20.0)).unwrap();
        seen.push([r, g, b]);
        doc.get_mut(id).unwrap().fills = vec![Paint::solid(DColor::rgb(r, g, b))];
    }
    seen
}

#[test]
fn picking_a_swatch_in_a_p3_document_returns_its_stored_numbers_every_time() {
    let (mut doc, page, id) = doc_with_fill(Paint::solid(MID));
    let seen = pick_and_apply(&mut doc, page, id, 4);
    assert_eq!(seen[0], [128, 102, 204], "the stored P3 numbers");
    assert!(seen.iter().all(|s| *s == seen[0]), "four picks in a row give one colour: {seen:?}");
}

#[test]
fn picking_from_an_image_in_a_p3_document_is_stable() {
    let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([200, 60, 60, 255]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    img.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    let fill = Paint {
        kind: PaintKind::Image { image_ref: "img".into(), scale_mode: ImageScaleMode::Fill, params: ImageParams::default() },
        ..Paint::solid(DColor::WHITE)
    };
    let (mut doc, page, id) = doc_with_fill(fill);
    doc.images.insert("img".into(), bytes.into_inner());
    let seen = pick_and_apply(&mut doc, page, id, 4);
    assert_ne!(seen[0], [200, 60, 60], "the sRGB image bytes are converted into P3 numbers");
    assert!(seen.iter().all(|s| *s == seen[0]), "picks from an image give one colour: {seen:?}");
}

fn filled_near(s: &Sim, c: [u8; 3]) -> bool {
    s.shapes.iter().any(|cs| match &cs.shape {
        egui::Shape::Rect(r) => {
            (r.fill.r() as i32 - c[0] as i32).abs() <= 1 && (r.fill.g() as i32 - c[1] as i32).abs() <= 1 && (r.fill.b() as i32 - c[2] as i32).abs() <= 1
        }
        _ => false,
    })
}

fn filled_with(s: &Sim, c: egui::Color32) -> bool {
    s.shapes.iter().any(|cs| matches!(&cs.shape, egui::Shape::Rect(r) if r.fill == c))
}

#[test]
fn the_page_swatch_of_a_p3_document_shows_the_converted_colour() {
    let mut s = Sim::new();
    s.shell.app.selection.clear();
    let page = s.app().page;
    s.shell.app.doc.get_mut(page).unwrap().kind = NodeKind::Page { background: MID };
    s.shell.app.doc.color_profile = ColorProfile::DisplayP3;
    s.frame(vec![]);
    s.frame(vec![]);
    // the canvas pixel is (132,101,211); the swatch goes through 8-bit channels first, so it may differ by one level
    assert!(filled_near(&s, [132, 101, 211]), "the swatch shows the colour the canvas draws");
    assert!(!filled_with(&s, egui::Color32::from_rgb(128, 102, 204)), "never the raw P3 numbers, swatch or canvas backdrop");
    s.shell.app.doc.color_profile = ColorProfile::Srgb;
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(filled_with(&s, egui::Color32::from_rgb(128, 102, 204)), "an sRGB document shows its numbers as they are");
}

#[test]
fn the_scoped_agent_canvas_keeps_the_document_profile() {
    let (doc, page, _) = doc_with_fill(Paint::solid(MID));
    let scoped = crate::live_canvas_scope::scoped_canvas(&doc, page, &[], "scoped");
    assert_eq!(scoped.color_profile, ColorProfile::DisplayP3);
}

#[test]
fn a_gradient_swatch_in_a_p3_document_shows_the_converted_stops() {
    let stops = vec![designos_doc::GradientStop { pos: 0.0, color: MID }, designos_doc::GradientStop { pos: 1.0, color: MID }];
    let paint = Paint::solid(DColor::WHITE);
    let paint = Paint {
        kind: PaintKind::LinearGradient {
            handles: [designos_doc::Vec2::new(0.0, 0.5), designos_doc::Vec2::new(1.0, 0.5), designos_doc::Vec2::new(0.0, 1.0)],
            stops,
        },
        ..paint
    };
    let vertex_colors = |profile: ColorProfile| {
        crate::widgets::set_document_profile(profile);
        let ctx = egui::Context::default();
        let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(100.0, 100.0))), ..Default::default() };
        let out = ctx.run_ui(raw, |ui| {
            crate::widgets::picker::paint_swatch(ui.painter(), egui::Rect::from_min_size(Pos2::new(10.0, 10.0), egui::vec2(24.0, 24.0)), &paint);
        });
        crate::widgets::set_document_profile(ColorProfile::Srgb);
        out.shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Mesh(m) => m.vertices.first().map(|v| v.color),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let p3 = vertex_colors(ColorProfile::DisplayP3);
    assert!(p3.iter().any(|c| (c.r() as i32 - 133).abs() <= 1 && (c.g() as i32 - 101).abs() <= 1 && (c.b() as i32 - 211).abs() <= 1), "{p3:?}");
    let srgb = vertex_colors(ColorProfile::Srgb);
    assert!(srgb.iter().any(|c| c.r() == 128 && c.g() == 102 && c.b() == 204), "{srgb:?}");
}

#[test]
fn swatches_are_right_in_the_frame_a_tab_switch_happens() {
    let mut s = Sim::new();
    s.shell.app.selection.clear();
    let page = s.app().page;
    s.shell.app.doc.get_mut(page).unwrap().kind = NodeKind::Page { background: MID };
    s.frame(vec![]);
    s.shell.app.open_new_design_tab();
    let page = s.app().page;
    s.shell.app.doc.get_mut(page).unwrap().kind = NodeKind::Page { background: MID };
    s.shell.app.doc.color_profile = ColorProfile::DisplayP3;
    s.shell.app.selection.clear();
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(filled_near(&s, [132, 101, 211]), "the Display P3 tab shows the converted swatch");
    // click the first (sRGB) tab: the frame that switches must already draw its swatch with raw numbers
    let tab = s.rect_of("project-tab-0").center();
    s.mv(tab);
    s.button(true);
    s.button(false);
    assert_eq!(s.app().doc.color_profile, ColorProfile::Srgb, "the click switched to the sRGB tab");
    assert!(filled_with(&s, egui::Color32::from_rgb(128, 102, 204)), "the sRGB tab's swatch is raw in the switching frame");
    assert!(!filled_near(&s, [132, 101, 211]), "no converted swatch in the switching frame");
}
