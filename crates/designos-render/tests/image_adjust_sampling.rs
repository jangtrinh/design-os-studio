//! The CPU reference adjusts the colour it samples from an image fill, not the picture's texels: at a
//! hard edge the blended pixel is adjusted as one colour.

use designos_doc::*;
use designos_render::image_adjust::adjust_rgba;
use designos_render::{render_page, tiny_skia::Pixmap, Viewport};

fn png_bytes(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    let mut pm = Pixmap::new(w, h).unwrap();
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            pm.data_mut()[i..i + 4].copy_from_slice(&f(x, y));
        }
    }
    pm.encode_png().unwrap()
}

fn px(pm: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let p = pm.pixel(x, y).unwrap();
    [p.red(), p.green(), p.blue(), p.alpha()]
}

/// A 2x1 picture stretched over a 64x4 node on a page of `background`.
fn edge_render(left: [u8; 4], right: [u8; 4], adjust: ImageAdjust, background: Color) -> Pixmap {
    let mut doc = Document::new("t");
    let page = doc.add_page("p");
    doc.get_mut(page).unwrap().kind = NodeKind::Page { background };
    doc.images.insert("edge".into(), png_bytes(2, 1, |x, _| if x == 0 { left } else { right }));
    let id = doc.alloc_id();
    let mut node = Node::rectangle(id, Rect::new(0.0, 0.0, 64.0, 4.0));
    node.fills = vec![Paint {
        kind: PaintKind::Image { image_ref: "edge".into(), scale_mode: ImageScaleMode::Fill, params: ImageParams { adjust, ..Default::default() } },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    }];
    doc.insert(page, node).unwrap();
    let mut pm = Pixmap::new(64, 4).unwrap();
    render_page(&doc, page, &Viewport::default(), &mut pm);
    pm
}

/// Every pixel of the adjusted render must be the adjustment of the same pixel of the unadjusted render.
fn assert_adjusts_the_sampled_colour(left: [u8; 4], right: [u8; 4], adjust: ImageAdjust, label: &str) {
    let plain = edge_render(left, right, ImageAdjust::default(), Color::WHITE);
    let adjusted = edge_render(left, right, adjust, Color::WHITE);
    let mut worst = 0;
    let mut moved = false;
    for x in 0..64 {
        let want = adjust_rgba(px(&plain, x, 2), &adjust);
        let got = px(&adjusted, x, 2);
        for k in 0..4 {
            worst = worst.max(want[k].abs_diff(got[k]));
        }
        moved |= px(&plain, x, 2)[..3] != got[..3];
    }
    assert!(worst <= 1, "{label}: adjusted pixel differs from the adjusted sample by {worst} levels");
    assert!(moved, "{label}: the adjustment must change some pixel");
}

#[test]
fn exposure_is_applied_to_the_blended_colour_at_a_black_and_white_edge() {
    assert_adjusts_the_sampled_colour([0, 0, 0, 255], [255, 255, 255, 255], ImageAdjust { exposure: 1.0, ..Default::default() }, "exposure +1");
}

#[test]
fn saturation_is_applied_to_the_blended_colour_at_a_coloured_edge() {
    assert_adjusts_the_sampled_colour([200, 100, 100, 255], [100, 100, 200, 255], ImageAdjust { vibrance: 1.0, ..Default::default() }, "saturation +1");
}

#[test]
fn contrast_and_temperature_are_applied_to_the_blended_colour_at_an_edge() {
    let adjust = ImageAdjust { contrast: 0.3, temperature: 0.6, ..Default::default() };
    assert_adjusts_the_sampled_colour([30, 60, 120, 255], [240, 220, 90, 255], adjust, "contrast and temperature");
}

#[test]
fn a_translucent_edge_is_adjusted_as_straight_colour_and_keeps_its_alpha() {
    let adjust = ImageAdjust { exposure: 1.0, ..Default::default() };
    let plain = edge_render([255, 0, 0, 255], [0, 0, 255, 100], ImageAdjust::default(), Color::TRANSPARENT);
    let adjusted = edge_render([255, 0, 0, 255], [0, 0, 255, 100], adjust, Color::TRANSPARENT);
    let mut checked = 0;
    for x in 0..64 {
        let p = plain.pixel(x, 2).unwrap();
        let a = p.alpha();
        if a == 0 {
            continue;
        }
        let d = p.demultiply();
        let straight = adjust_rgba([d.red(), d.green(), d.blue(), 255], &adjust);
        let got = adjusted.pixel(x, 2).unwrap();
        assert_eq!(got.alpha(), a, "alpha of pixel {x} is never changed");
        let want = [straight[0] as f32 * a as f32 / 255.0, straight[1] as f32 * a as f32 / 255.0, straight[2] as f32 * a as f32 / 255.0];
        for (k, c) in [got.red(), got.green(), got.blue()].into_iter().enumerate() {
            assert!((c as f32 - want[k]).abs() <= 2.5, "pixel {x} channel {k}: got {c}, want about {:.1}", want[k]);
        }
        checked += 1;
    }
    assert!(checked > 32, "most of the strip is painted, checked {checked}");
}

/// One 256x256 tile in the middle of a 4032x3024 picture drawn at its native size at a device pixel ratio of 2.
/// Run with `cargo test -p designos-render --release --test image_adjust_sampling -- --ignored --nocapture`.
#[test]
#[ignore = "timing only"]
fn time_one_tile_over_a_large_adjusted_picture() {
    let mut doc = Document::new("t");
    let page = doc.add_page("p");
    doc.images.insert("photo".into(), png_bytes(4032, 3024, |x, y| [(x % 251) as u8, (y % 241) as u8, ((x + y) % 239) as u8, 255]));
    let id = doc.alloc_id();
    let mut node = Node::rectangle(id, Rect::new(0.0, 0.0, 4032.0, 3024.0));
    let paint = |adjust| Paint {
        kind: PaintKind::Image { image_ref: "photo".into(), scale_mode: ImageScaleMode::Fill, params: ImageParams { adjust, ..Default::default() } },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    };
    node.fills = vec![paint(ImageAdjust::default())];
    doc.insert(page, node).unwrap();
    // the tile's top-left corner is at page (2000, 1500), so device pixel (4000, 3000)
    let vp = Viewport { offset: Vec2::new(2000.0, 1500.0), zoom: 1.0, dpr: 2.0 };
    for (label, adjust) in [
        ("unadjusted", ImageAdjust::default()),
        ("exposure+contrast+saturation", ImageAdjust { exposure: 0.5, contrast: 0.2, vibrance: 0.5, ..Default::default() }),
    ] {
        doc.get_mut(id).unwrap().fills = vec![paint(adjust)];
        let mut tile = Pixmap::new(256, 256).unwrap();
        render_page(&doc, page, &vp, &mut tile); // decodes the picture once
        let mut times: Vec<f64> = (0..5)
            .map(|_| {
                let t0 = std::time::Instant::now();
                render_page(&doc, page, &vp, &mut tile);
                t0.elapsed().as_secs_f64() * 1000.0
            })
            .collect();
        times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        eprintln!("TILE_TIME {label}: min {:.2} ms, median {:.2} ms, max {:.2} ms (5 runs)", times[0], times[2], times[4]);
    }
}
