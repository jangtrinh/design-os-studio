//! Converting an image to Display P3 is a pass over its pixels, so it is done once per image and size:
//! the eyedropper loupe samples every frame and an export can hold many layers on one photo.

use designos_doc::*;
use designos_render::{
    converted_image_bytes, decoded_image_bytes, image_conversions, render_page_native, retain_converted_images, tiny_skia::Pixmap, Viewport,
    MAX_CONVERTED_IMAGE_BYTES, MAX_DECODED_IMAGE_BYTES,
};

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

fn small_png(seed: u8) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([seed, seed.wrapping_mul(3), seed.wrapping_add(17), 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

#[test]
fn forty_p3_images_over_two_native_renders_convert_forty_times() {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    d.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::WHITE };
    d.color_profile = ColorProfile::DisplayP3;
    for i in 0..40u8 {
        let key = format!("img-{i}");
        d.images.insert(key.clone(), small_png(i));
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(i as f32 * 6.0, 0.0, 4.0, 4.0));
        n.fills = vec![Paint {
            kind: PaintKind::Image { image_ref: key, scale_mode: ImageScaleMode::Fill, params: ImageParams::default() },
            ..Paint::solid(Color::WHITE)
        }];
        d.insert(page, n).unwrap();
    }
    let before = image_conversions();
    render(&d, page, 40 * 6);
    render(&d, page, 40 * 6);
    assert_eq!(image_conversions() - before, 40, "two renders of 40 images must convert each image once");
}

fn solid_png(side: u32, seed: u8) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(side, side, image::Rgba([seed, seed.wrapping_mul(5), seed.wrapping_add(31), 255]));
    let mut out = Vec::new();
    let enc = image::codecs::png::PngEncoder::new_with_quality(&mut out, image::codecs::png::CompressionType::Fast, image::codecs::png::FilterType::NoFilter);
    image::ImageEncoder::write_image(enc, img.as_raw(), side, side, image::ExtendedColorType::Rgba8).unwrap();
    out
}

fn single_image_doc(name: &str, side: u32, seed: u8) -> (Document, NodeId) {
    let mut d = Document::new(name);
    let page = d.add_page("p");
    d.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::WHITE };
    d.color_profile = ColorProfile::DisplayP3;
    let key = format!("photo-{seed}");
    d.images.insert(key.clone(), solid_png(side, seed));
    let id = d.alloc_id();
    let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, side as f32, side as f32));
    n.fills = vec![Paint {
        kind: PaintKind::Image { image_ref: key, scale_mode: ImageScaleMode::Fill, params: ImageParams::default() },
        ..Paint::solid(Color::WHITE)
    }];
    d.insert(page, n).unwrap();
    (d, page)
}

fn render_one_px(d: &Document, page: NodeId) {
    let mut pm = Pixmap::new(1, 1).unwrap();
    render_page_native(d, page, &Viewport::default(), &mut pm);
}

#[test]
fn converted_image_cache_stays_within_byte_budget_and_evicts_oldest_first() {
    // 2500 x 2500 RGBA8 is 25 MB: two fit in the 64 MiB budget (50 MB <= 64 MiB), the third (75 MB) evicts the first.
    let (d0, p0) = single_image_doc("d0", 2500, 10);
    let (d1, p1) = single_image_doc("d1", 2500, 20);
    let (d2, p2) = single_image_doc("d2", 2500, 30);

    let before = image_conversions();
    render_one_px(&d0, p0);
    render_one_px(&d1, p1);
    assert_eq!(image_conversions() - before, 2);
    assert!(converted_image_bytes() <= MAX_CONVERTED_IMAGE_BYTES);

    render_one_px(&d2, p2);
    assert_eq!(image_conversions() - before, 3);
    assert!(
        converted_image_bytes() <= MAX_CONVERTED_IMAGE_BYTES,
        "cache bytes {} must not exceed budget {}",
        converted_image_bytes(),
        MAX_CONVERTED_IMAGE_BYTES
    );

    // d1 and d2 (the two newest) are still cached; d0 (the oldest) was evicted first.
    let after_three = image_conversions();
    render_one_px(&d1, p1);
    render_one_px(&d2, p2);
    assert_eq!(image_conversions() - after_three, 0, "the newer images stay in the cache");

    render_one_px(&d0, p0);
    assert_eq!(image_conversions() - after_three, 1, "the oldest image was evicted and converts again");
    assert!(converted_image_bytes() <= MAX_CONVERTED_IMAGE_BYTES);
}

#[test]
fn dropping_a_closed_document_releases_its_converted_images() {
    let (d0, p0) = single_image_doc("open", 64, 41);
    let (d1, p1) = single_image_doc("closed", 64, 42);
    retain_converted_images(std::iter::empty());
    render_one_px(&d0, p0);
    render_one_px(&d1, p1);
    assert_eq!(converted_image_bytes(), 2 * 64 * 64 * 4);
    assert_eq!(decoded_image_bytes(), 2 * 64 * 64 * 4);

    retain_converted_images([(&d0, None)]);
    assert_eq!(converted_image_bytes(), 64 * 64 * 4, "closing d1 drops its converted pixmap bytes");
    assert_eq!(decoded_image_bytes(), 64 * 64 * 4, "closing d1 drops its decoded pixmap bytes");

    let before = image_conversions();
    render_one_px(&d0, p0);
    assert_eq!(image_conversions() - before, 0, "the still-open document keeps its converted image");
    render_one_px(&d1, p1);
    assert_eq!(image_conversions() - before, 1, "the closed document's image was dropped from the cache");
}

#[test]
fn two_open_documents_sharing_one_image_keep_one_pixmap_after_one_closes() {
    let (d0, p0) = single_image_doc("shared-0", 64, 51);
    let (d1, p1) = single_image_doc("shared-1", 64, 51);
    retain_converted_images(std::iter::empty());

    let base_conv = image_conversions();
    render_one_px(&d0, p0);
    render_one_px(&d1, p1);
    assert_eq!(image_conversions() - base_conv, 1, "two documents sharing one (key, bytes) convert once");
    assert_eq!(converted_image_bytes(), 64 * 64 * 4, "two documents sharing one image keep one pixmap");
    assert_eq!(decoded_image_bytes(), 64 * 64 * 4, "two documents sharing one image keep one decoded pixmap");

    retain_converted_images([(&d0, None)]);
    assert_eq!(converted_image_bytes(), 64 * 64 * 4, "closing one document keeps the shared pixmap");
    assert_eq!(decoded_image_bytes(), 64 * 64 * 4, "closing one document keeps the shared decoded pixmap");
    render_one_px(&d0, p0);
    assert_eq!(image_conversions() - base_conv, 1, "surviving document reuses the shared pixmap");
}

#[test]
fn an_image_over_the_byte_budget_is_the_only_cached_entry_and_converts_once_across_samples() {
    // 4100 x 4100 RGBA8 is 67,240,000 bytes > MAX_CONVERTED_IMAGE_BYTES (67,108,864 bytes).
    let (big, page) = single_image_doc("oversized", 4100, 61);
    let (small, small_page) = single_image_doc("small-after-oversized", 64, 62);
    retain_converted_images(std::iter::empty());
    let conv_before = image_conversions();
    render_one_px(&big, page);
    render_one_px(&big, page);
    render_one_px(&big, page);
    assert_eq!(image_conversions() - conv_before, 1, "an eyedropper hovering an oversized image converts it once, not every frame");
    assert_eq!(converted_image_bytes(), 4100 * 4100 * 4, "the oversized image is held alone");

    render_one_px(&small, small_page);
    assert!(converted_image_bytes() <= MAX_CONVERTED_IMAGE_BYTES, "the next image evicts the oversized one: {}", converted_image_bytes());
    assert_eq!(converted_image_bytes(), 64 * 64 * 4);
}

#[test]
fn decoded_image_cache_stays_within_byte_budget() {
    let (d0, p0) = single_image_doc("decoded-0", 2500, 71);
    let (d1, p1) = single_image_doc("decoded-1", 2500, 72);
    let (d2, p2) = single_image_doc("decoded-2", 2500, 73);
    retain_converted_images(std::iter::empty());
    render_one_px(&d0, p0);
    render_one_px(&d1, p1);
    render_one_px(&d2, p2);
    assert!(decoded_image_bytes() <= MAX_DECODED_IMAGE_BYTES, "decoded cache bytes {} exceed {}", decoded_image_bytes(), MAX_DECODED_IMAGE_BYTES);
}

#[test]
fn an_oversized_decoded_image_is_cached_alone_decoded_once_and_evicted_by_the_next_image() {
    // 4100 x 4100 RGBA8 is 67,240,000 bytes > MAX_DECODED_IMAGE_BYTES (67,108,864 bytes).
    let (big, page) = single_image_doc("oversized-decoded", 4100, 81);
    let (small_a, page_a) = single_image_doc("small-decoded-a", 64, 82);
    let (small_b, page_b) = single_image_doc("small-decoded-b", 64, 83);
    retain_converted_images(std::iter::empty());
    let dec_before = designos_render::image_decodes();
    render_one_px(&big, page);
    render_one_px(&big, page);
    render_one_px(&big, page);
    assert_eq!(designos_render::image_decodes() - dec_before, 1, "an oversized image is decoded once across repeated paints");
    assert_eq!(decoded_image_bytes(), 4100 * 4100 * 4, "the oversized decoded image is held alone");

    render_one_px(&small_a, page_a);
    assert!(decoded_image_bytes() <= MAX_DECODED_IMAGE_BYTES, "inserting a second image evicts the oversized one: {}", decoded_image_bytes());
    assert_eq!(decoded_image_bytes(), 64 * 64 * 4);

    render_one_px(&small_b, page_b);
    assert!(decoded_image_bytes() <= MAX_DECODED_IMAGE_BYTES, "multiple cached entries stay within the byte bound");
    assert_eq!(decoded_image_bytes(), 2 * 64 * 64 * 4);
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
