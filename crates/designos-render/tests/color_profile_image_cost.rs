//! Converting an image to Display P3 is a pass over its pixels, so it is done once per image and size:
//! the eyedropper loupe samples every frame and an export can hold many layers on one photo.

use designos_doc::*;
use designos_render::{image_conversions, render_page_native, tiny_skia::Pixmap, Viewport};

const W: u32 = 4000;
const H: u32 = 3000;

fn big_png() -> Vec<u8> {
    let img = image::RgbaImage::from_fn(W, H, |x, y| image::Rgba([(x % 251) as u8, (y % 241) as u8, ((x + y) % 239) as u8, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

fn doc(profile: ColorProfile, layers: usize) -> (Document, NodeId) {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    d.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::WHITE };
    for i in 0..layers {
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(i as f32 * 12.0, 0.0, 40.0, 40.0));
        n.fills = vec![Paint {
            kind: PaintKind::Image { image_ref: "photo".into(), scale_mode: ImageScaleMode::Fill, params: ImageParams::default() },
            ..Paint::solid(Color::WHITE)
        }];
        d.insert(page, n).unwrap();
    }
    d.images.insert("photo".into(), big_png());
    d.color_profile = profile;
    (d, page)
}

fn render(d: &Document, page: NodeId, w: u32) {
    let mut pm = Pixmap::new(w, 40).unwrap();
    render_page_native(d, page, &Viewport::default(), &mut pm);
}

fn many_unique_images(count: usize) -> (Document, NodeId, usize) {
    let mut d = Document::new("many images");
    let page = d.add_page("p");
    d.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::WHITE };
    for i in 0..count {
        let image_ref = format!("photo-{i}");
        let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([i as u8, 60, 90, 255]));
        let mut png = std::io::Cursor::new(Vec::new());
        img.write_to(&mut png, image::ImageFormat::Png).unwrap();
        d.images.insert(image_ref.clone(), png.into_inner());
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(i as f32 * 12.0, 0.0, 40.0, 40.0));
        n.fills = vec![Paint {
            kind: PaintKind::Image { image_ref, scale_mode: ImageScaleMode::Fill, params: ImageParams::default() },
            ..Paint::solid(Color::WHITE)
        }];
        d.insert(page, n).unwrap();
    }
    d.color_profile = ColorProfile::DisplayP3;
    (d, page, count * 12 + 40)
}

#[test]
fn more_than_32_images_stay_cached_when_they_fit_the_cache_budget() {
    let (d, page, width) = many_unique_images(40);
    let before = image_conversions();
    render(&d, page, width as u32);
    assert_eq!(image_conversions() - before, 40, "the first render converts each image once");
    let after_first = image_conversions();
    render(&d, page, width as u32);
    assert_eq!(image_conversions() - after_first, 0, "the second render reuses all 40 converted images");
}

#[test]
fn a_second_native_render_of_an_unchanged_p3_image_converts_nothing() {
    let (d, page) = doc(ColorProfile::DisplayP3, 1);
    let before = image_conversions();
    render(&d, page, 60);
    let first = image_conversions() - before;
    assert_eq!(first, 1, "the drawn copy is converted once");
    let again = image_conversions();
    render(&d, page, 60);
    render(&d, page, 60);
    assert_eq!(image_conversions() - again, 0, "an unchanged image is not converted again");
}

#[test]
fn eleven_layers_sharing_one_p3_image_convert_it_once() {
    let (d, page) = doc(ColorProfile::DisplayP3, 11);
    let before = image_conversions();
    render(&d, page, 11 * 12 + 40);
    assert_eq!(image_conversions() - before, 1);
}

#[test]
fn an_srgb_document_converts_no_image() {
    let (d, page) = doc(ColorProfile::Srgb, 11);
    let before = image_conversions();
    render(&d, page, 11 * 12 + 40);
    assert_eq!(image_conversions() - before, 0);
}

/// Release timing of a 1x1 native sample over the 4000x3000 photo, as the eyedropper loupe takes it every frame:
/// `cargo test --release -p designos-render --test color_profile_image_cost -- --ignored --nocapture`.
#[test]
#[ignore = "timing, run in release"]
fn timing_of_the_eyedropper_sample_over_a_large_p3_image() {
    let (d, page) = doc(ColorProfile::DisplayP3, 1);
    let sample = || {
        let mut pm = Pixmap::new(1, 1).unwrap();
        let vp = Viewport { offset: Vec2::new(19.5, 19.5), zoom: 1.0, dpr: 1.0 };
        let t = std::time::Instant::now();
        render_page_native(&d, page, &vp, &mut pm);
        t.elapsed()
    };
    let first = sample();
    let n = 50;
    let t = std::time::Instant::now();
    for _ in 0..n {
        sample();
    }
    eprintln!("first sample {first:?}, then {:?} per frame", t.elapsed() / n);
}
