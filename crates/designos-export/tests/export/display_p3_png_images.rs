//! A Display P3 PNG export looks like the canvas in a colour-managed viewer, photos included, and the
//! untagged preview export shows the canvas colours to readers that ignore tags.

use designos_doc::*;
use designos_export::{export_png, export_png_srgb};
use designos_render::{render_page, tiny_skia::Pixmap, Viewport};

fn tiny_png(rgb: [u8; 3]) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([rgb[0], rgb[1], rgb[2], 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

fn doc_with(fill: Paint, images: &[(&str, Vec<u8>)]) -> (Document, NodeId, NodeId) {
    let mut doc = Document::new("t");
    let page = doc.add_page("p");
    doc.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::WHITE };
    let id = doc.alloc_id();
    let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0));
    n.fills = vec![fill];
    let id = doc.insert(page, n).unwrap();
    for (k, v) in images {
        doc.images.insert((*k).into(), v.clone());
    }
    doc.color_profile = ColorProfile::DisplayP3;
    (doc, page, id)
}

fn as_srgb_if_p3(raw: [u8; 4]) -> [u8; 3] {
    let s = ColorProfile::DisplayP3.to_srgb(&Color { r: raw[0] as f32 / 255.0, g: raw[1] as f32 / 255.0, b: raw[2] as f32 / 255.0, a: 1.0 });
    [(s.r * 255.0).round() as u8, (s.g * 255.0).round() as u8, (s.b * 255.0).round() as u8]
}

fn close(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| (*x as i32 - y as i32).abs() <= 1)
}

#[test]
fn an_image_fill_in_a_p3_png_export_looks_like_the_canvas() {
    let image_fill = Paint {
        kind: PaintKind::Image { image_ref: "img".into(), scale_mode: ImageScaleMode::Fill, params: ImageParams::default() },
        ..Paint::solid(Color::WHITE)
    };
    let (doc, page, id) = doc_with(image_fill, &[("img", tiny_png([200, 60, 60]))]);
    let mut canvas = Pixmap::new(40, 40).unwrap();
    render_page(&doc, page, &Viewport::default(), &mut canvas);
    let c = canvas.pixel(20, 20).unwrap();
    let on_canvas = [c.red(), c.green(), c.blue()];
    let exported = image::load_from_memory(&export_png(&doc, id, 1.0)).unwrap().to_rgba8().get_pixel(20, 20).0;
    let viewed = as_srgb_if_p3(exported);
    assert!(close(on_canvas, viewed), "canvas {on_canvas:?}, the tagged PNG shows {viewed:?} in a colour-managed viewer");
}

#[test]
fn the_untagged_preview_export_of_a_p3_document_shows_the_canvas_colours() {
    let (doc, _, id) = doc_with(Paint::solid(Color { r: 0.5, g: 0.4, b: 0.8, a: 1.0 }), &[]);
    let png = export_png_srgb(&doc, id, 1.0);
    let px = image::load_from_memory(&png).unwrap().to_rgba8().get_pixel(20, 20).0;
    assert!(close([px[0], px[1], px[2]], [132, 101, 211]), "{px:?}");
    let has_profile = png.windows(4).any(|w| w == b"iCCP" || w == b"cICP");
    assert!(!has_profile, "a preview export carries no profile tag");
}
