//! Image bytes are sRGB. On screen they are drawn as they are; in a native render of a Display P3
//! document they are converted to Display P3 so the P3 tag of an export is true for photos too.

use designos_doc::*;
use designos_render::{render_page, render_page_native, tiny_skia::Pixmap, Viewport};

fn tiny_png(rgb: [u8; 3]) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([rgb[0], rgb[1], rgb[2], 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

fn image_doc(profile: ColorProfile, rgb: [u8; 3]) -> (Document, NodeId) {
    let mut doc = Document::new("t");
    let page = doc.add_page("p");
    doc.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::WHITE };
    let id = doc.alloc_id();
    let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0));
    n.fills = vec![Paint {
        kind: PaintKind::Image { image_ref: "img".into(), scale_mode: ImageScaleMode::Fill, params: ImageParams::default() },
        ..Paint::solid(Color::WHITE)
    }];
    doc.insert(page, n).unwrap();
    doc.images.insert("img".into(), tiny_png(rgb));
    doc.color_profile = profile;
    (doc, page)
}

fn px(pm: &Pixmap, x: u32, y: u32) -> [u8; 3] {
    let p = pm.pixel(x, y).unwrap().demultiply();
    [p.red(), p.green(), p.blue()]
}

fn render(doc: &Document, page: NodeId, native: bool) -> Pixmap {
    let mut pm = Pixmap::new(40, 40).unwrap();
    let vp = Viewport::default();
    if native {
        render_page_native(doc, page, &vp, &mut pm);
    } else {
        render_page(doc, page, &vp, &mut pm);
    }
    pm
}

fn within(a: [u8; 3], b: [u8; 3], tol: i32) -> bool {
    a.iter().zip(b).all(|(x, y)| (*x as i32 - y as i32).abs() <= tol)
}

#[test]
fn an_image_in_a_p3_document_looks_the_same_on_screen_and_in_a_native_render_read_as_p3() {
    let rgb = [200u8, 60, 60];
    let (doc, page) = image_doc(ColorProfile::DisplayP3, rgb);
    let screen = px(&render(&doc, page, false), 20, 20);
    assert_eq!(screen, rgb, "an sRGB image is drawn as it is on screen");
    let native = px(&render(&doc, page, true), 20, 20);
    assert_ne!(native, rgb, "the native render holds P3 numbers, so the sRGB bytes must change");
    let s = ColorProfile::DisplayP3.to_srgb(&Color { r: native[0] as f32 / 255.0, g: native[1] as f32 / 255.0, b: native[2] as f32 / 255.0, a: 1.0 });
    let shown = [(s.r * 255.0).round() as u8, (s.g * 255.0).round() as u8, (s.b * 255.0).round() as u8];
    assert!(within(shown, screen, 1), "a viewer reading the native pixels as P3 sees {shown:?}, the canvas shows {screen:?}");
}

#[test]
fn an_image_in_an_srgb_document_is_the_same_bytes_native_or_not() {
    let (doc, page) = image_doc(ColorProfile::Srgb, [200, 60, 60]);
    assert_eq!(render(&doc, page, true).data(), render(&doc, page, false).data());
}
