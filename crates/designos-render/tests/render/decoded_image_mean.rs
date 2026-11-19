//! The mean colour of a document image, the sampler connector ink reads image fills through.

use designos_doc::Document;
use designos_render::decoded_image_mean;
use image::ImageEncoder;

fn png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes).write_image(rgba, width, height, image::ExtendedColorType::Rgba8).expect("encode test image");
    bytes
}

fn doc_with(image_ref: &str, bytes: Vec<u8>) -> Document {
    let mut doc = Document::new("Image mean");
    doc.images.insert(image_ref.into(), bytes);
    doc
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

#[test]
fn an_opaque_image_is_the_mean_of_its_pixels() {
    let doc = doc_with("bw", png(2, 1, &[0, 0, 0, 255, 255, 255, 255, 255]));
    let mean = decoded_image_mean(&doc, "bw").expect("a decodable image has a mean");
    assert!(close(mean.r, 0.5) && close(mean.g, 0.5) && close(mean.b, 0.5), "black and white average to mid grey, got {mean:?}");
    assert!(close(mean.a, 1.0), "an opaque image has mean alpha 1, got {}", mean.a);

    let doc = doc_with("dark", png(1, 1, &[0x10, 0x20, 0x30, 255]));
    let mean = decoded_image_mean(&doc, "dark").expect("a decodable image has a mean");
    assert!(close(mean.r, 0x10 as f32 / 255.0) && close(mean.g, 0x20 as f32 / 255.0) && close(mean.b, 0x30 as f32 / 255.0), "got {mean:?}");
}

#[test]
fn transparent_pixels_lower_the_alpha_without_darkening_the_colour() {
    let doc = doc_with("half", png(2, 1, &[255, 255, 255, 255, 0, 0, 0, 0]));
    let mean = decoded_image_mean(&doc, "half").expect("a half transparent image has a mean");
    assert!(close(mean.r, 1.0) && close(mean.g, 1.0) && close(mean.b, 1.0), "the visible pixel is white, got {mean:?}");
    assert!(close(mean.a, 0.5), "half of the pixels are transparent, got {}", mean.a);
}

#[test]
fn an_image_with_nothing_to_read_has_no_mean() {
    let doc = doc_with("clear", png(1, 1, &[10, 20, 30, 0]));
    assert_eq!(decoded_image_mean(&doc, "clear"), None, "a fully transparent image is not a surface");
    assert_eq!(decoded_image_mean(&doc, "missing"), None, "an unknown image reference");
    let doc = doc_with("junk", b"not an image".to_vec());
    assert_eq!(decoded_image_mean(&doc, "junk"), None, "bytes that do not decode");
}

#[test]
fn a_photo_past_sixteen_million_pixels_still_averages_every_pixel() {
    // 17,000,000 black pixels then 8,000,000 white ones: a single-precision running count stops
    // growing at 16,777,216 and would read this as a mid grey instead of the true third.
    let (width, height, black_rows) = (5000u32, 5000u32, 3400u32);
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for row in 0..height {
        let value = if row < black_rows { 0 } else { 255 };
        for _ in 0..width {
            rgba.extend_from_slice(&[value, value, value, 255]);
        }
    }
    let doc = doc_with("big", png(width, height, &rgba));
    let mean = decoded_image_mean(&doc, "big").expect("a large image has a mean");
    let white_share = (height - black_rows) as f32 / height as f32;
    assert!(close(mean.r, white_share) && close(mean.g, white_share) && close(mean.b, white_share), "expected grey {white_share}, got {mean:?}");
    assert!(close(mean.a, 1.0), "got {}", mean.a);
}
