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

/// A 16x16 flat grey picture in a 64x32 Fit node (centred 32x32 square at x 16..48) or in a Crop turned a quarter.
fn letterbox_render(mode: ImageScaleMode, adjust: ImageAdjust) -> Pixmap {
    let mut doc = Document::new("t");
    let page = doc.add_page("p");
    doc.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::TRANSPARENT };
    doc.images.insert("flat".into(), png_bytes(16, 16, |_, _| [128, 128, 128, 255]));
    let id = doc.alloc_id();
    let mut node = Node::rectangle(id, Rect::new(0.0, 0.0, 64.0, 32.0));
    let params = match mode {
        ImageScaleMode::Crop => {
            // the Fit framing of the picture, then turned a quarter: a 32x32 square at x 16..48 either way
            let framing =
                designos_render::image_adjust::crop_transform_from_framing(Vec2::new(64.0, 32.0), 16.0, 16.0, ImageScaleMode::Fit, &ImageParams::default());
            let turned = designos_render::image_adjust::rotate_crop_transform(framing.unwrap(), Vec2::new(64.0, 32.0), true);
            ImageParams { transform: turned, adjust, ..Default::default() }
        }
        _ => ImageParams { adjust, ..Default::default() },
    };
    node.fills =
        vec![Paint { kind: PaintKind::Image { image_ref: "flat".into(), scale_mode: mode, params }, opacity: 1.0, visible: true, blend: BlendMode::Normal }];
    doc.insert(page, node).unwrap();
    let mut pm = Pixmap::new(64, 32).unwrap();
    render_page(&doc, page, &Viewport::default(), &mut pm);
    pm
}

#[test]
fn the_letterbox_and_the_area_outside_a_turned_crop_stay_transparent_when_the_picture_is_adjusted() {
    let adjust = ImageAdjust { exposure: 1.0, ..Default::default() };
    let brightened = adjust_rgba([128, 128, 128, 255], &adjust);
    for mode in [ImageScaleMode::Fit, ImageScaleMode::Crop] {
        let plain = letterbox_render(mode, ImageAdjust::default());
        let adjusted = letterbox_render(mode, adjust);
        for y in 0..32 {
            for x in (0..14).chain(50..64) {
                assert_eq!(px(&adjusted, x, y), px(&plain, x, y), "{mode:?}: ({x}, {y}) outside the picture is what the unadjusted fill gives");
                assert_eq!(px(&adjusted, x, y)[3], 0, "{mode:?}: ({x}, {y}) outside the picture is transparent");
            }
            // the soft ring where the picture meets the letterbox is left out: two pixels in from each edge
            if !(4..28).contains(&y) {
                continue;
            }
            for x in 20..44 {
                let got = px(&adjusted, x, y);
                assert!(got[3] == 255 && (0..3).all(|k| got[k].abs_diff(brightened[k]) <= 1), "{mode:?}: ({x}, {y}) inside the picture is adjusted: {got:?}");
            }
        }
    }
}

#[test]
fn fit_fill_renders_identical_pixels_across_all_image_adjustments() {
    let mut doc = Document::new("t");
    let page = doc.add_page("p");
    doc.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::TRANSPARENT };
    doc.images.insert(
        "gradient".into(),
        png_bytes(16, 16, |x, y| [((x * 15 + 20) % 240) as u8, ((y * 13 + 40) % 230) as u8, (((x + y) * 11 + 60) % 220) as u8, 255]),
    );
    let id = doc.alloc_id();
    let mut node = Node::rectangle(id, Rect::new(0.0, 0.0, 64.0, 32.0));
    let make_paint = |adjust| Paint {
        kind: PaintKind::Image { image_ref: "gradient".into(), scale_mode: ImageScaleMode::Fit, params: ImageParams { adjust, ..Default::default() } },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    };
    node.fills = vec![make_paint(ImageAdjust::default())];
    doc.insert(page, node).unwrap();
    let mut plain = Pixmap::new(64, 32).unwrap();
    render_page(&doc, page, &Viewport::default(), &mut plain);

    let cases = [
        ("exposure", ImageAdjust { exposure: 0.45, ..Default::default() }),
        ("contrast", ImageAdjust { contrast: 0.35, ..Default::default() }),
        ("saturation", ImageAdjust { vibrance: 0.5, ..Default::default() }),
        ("temperature", ImageAdjust { temperature: 0.4, ..Default::default() }),
        ("tint", ImageAdjust { tint: -0.35, ..Default::default() }),
        ("highlights", ImageAdjust { highlights: -0.4, ..Default::default() }),
        ("shadows", ImageAdjust { shadows: 0.45, ..Default::default() }),
        (
            "all_seven",
            ImageAdjust { exposure: 0.25, contrast: 0.2, vibrance: 0.3, temperature: 0.25, tint: -0.2, highlights: -0.25, shadows: 0.3, ..Default::default() },
        ),
    ];

    for (label, adjust) in cases {
        doc.get_mut(id).unwrap().fills = vec![make_paint(adjust)];
        let mut adjusted = Pixmap::new(64, 32).unwrap();
        render_page(&doc, page, &Viewport::default(), &mut adjusted);
        let mut moved = false;
        for y in 0..32 {
            for x in (0..14).chain(50..64) {
                assert_eq!(px(&adjusted, x, y), [0, 0, 0, 0], "{label}: letterbox at ({x}, {y}) must stay transparent");
            }
            if !(4..28).contains(&y) {
                continue;
            }
            for x in 20..44 {
                let sample = px(&plain, x, y);
                let want = adjust_rgba(sample, &adjust);
                let got = px(&adjusted, x, y);
                for k in 0..4 {
                    assert!(want[k].abs_diff(got[k]) <= 1, "{label}: ({x}, {y}) channel {k} got {got:?}, want {want:?}");
                }
                moved |= sample[..3] != got[..3];
            }
        }
        assert!(moved, "{label}: adjustment must alter visible Fit pixels");
    }
}

/// A 100x100 node at page x 99.8 whose only stroke is an outside outline of weight 8 on its right side, painted with
/// a crop image whose frame leaves the unit square at the left corners but covers everything right of the node.
fn crop_outline_render(width: u32, height: u32, viewport: &Viewport) -> Pixmap {
    let mut doc = Document::new("t");
    let page = doc.add_page("p");
    doc.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::TRANSPARENT };
    doc.images.insert("flat".into(), png_bytes(16, 16, |_, _| [40, 160, 80, 255]));
    let id = doc.alloc_id();
    let mut node = Node::rectangle(id, Rect::new(99.8, 10.0, 100.0, 100.0));
    node.fills.clear();
    node.stroke.weight = 8.0;
    node.stroke.align = StrokeAlign::Outside;
    node.stroke.side_weights = Some([0.0, 8.0, 0.0, 0.0]);
    // u = x / 200 - 0.25 and v = y / 200 + 0.25: the left edge of the node is outside the picture, the picture
    // spans local x 50..250, and the stroke band right of the node sits well inside it.
    let transform = Some([0.5, 0.0, -0.25, 0.0, 0.5, 0.25]);
    node.stroke.paints = vec![Paint {
        kind: PaintKind::Image { image_ref: "flat".into(), scale_mode: ImageScaleMode::Crop, params: ImageParams { transform, ..Default::default() } },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    }];
    doc.insert(page, node).unwrap();
    let mut pm = Pixmap::new(width, height).unwrap();
    render_page(&doc, page, viewport, &mut pm);
    pm
}

#[test]
fn an_outline_of_a_crop_image_stroke_is_drawn_on_a_tile_that_misses_the_node_box() {
    let whole = crop_outline_render(300, 120, &Viewport::default());
    // The tile starts at page x 200, so the node box ends 0.2 px left of the tile's pixmap while the outline
    // reaches 7.8 px into it.
    let tile = crop_outline_render(40, 120, &Viewport { offset: Vec2::new(200.0, 0.0), zoom: 1.0, dpr: 1.0 });
    for x in 1..7 {
        for y in 12..108 {
            assert_eq!(px(&whole, 200 + x, y), [40, 160, 80, 255], "the full page paints the outline at ({}, {y})", 200 + x);
        }
    }
    for y in 0..120 {
        for x in 0..40 {
            assert_eq!(px(&tile, x, y), px(&whole, 200 + x, y), "tile pixel ({x}, {y}) is the same pixel of the full page");
        }
    }
}

/// One 256x256 tile in the middle of a 4032x3024 picture fitted into a 4032x4032 node at a device pixel ratio of 2.
/// Run with `cargo test -p designos-render --release --test image_adjust_sampling -- --ignored --nocapture`.
#[test]
#[ignore = "timing only"]
fn time_one_tile_over_a_large_adjusted_fit_picture() {
    let mut doc = Document::new("t");
    let page = doc.add_page("p");
    doc.images.insert("photo".into(), png_bytes(4032, 3024, |x, y| [(x % 251) as u8, (y % 241) as u8, ((x + y) % 239) as u8, 255]));
    let id = doc.alloc_id();
    let mut node = Node::rectangle(id, Rect::new(0.0, 0.0, 4032.0, 4032.0));
    let paint = |adjust| Paint {
        kind: PaintKind::Image { image_ref: "photo".into(), scale_mode: ImageScaleMode::Fit, params: ImageParams { adjust, ..Default::default() } },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    };
    node.fills = vec![paint(ImageAdjust::default())];
    doc.insert(page, node).unwrap();
    // the picture sits at y 504..3528; the tile starts at page (2000, 1500)
    let vp = Viewport { offset: Vec2::new(2000.0, 1500.0), zoom: 1.0, dpr: 2.0 };
    for (label, adjust) in [
        ("unadjusted", ImageAdjust::default()),
        ("exposure+contrast+saturation", ImageAdjust { exposure: 0.5, contrast: 0.2, vibrance: 0.5, ..Default::default() }),
    ] {
        doc.get_mut(id).unwrap().fills = vec![paint(adjust)];
        let mut tile = Pixmap::new(256, 256).unwrap();
        render_page(&doc, page, &vp, &mut tile);
        let mut times: Vec<f64> = (0..5)
            .map(|_| {
                let t0 = std::time::Instant::now();
                render_page(&doc, page, &vp, &mut tile);
                t0.elapsed().as_secs_f64() * 1000.0
            })
            .collect();
        times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        eprintln!("FIT_TILE_TIME {label}: min {:.2} ms, median {:.2} ms, max {:.2} ms (5 runs)", times[0], times[2], times[4]);
    }
}
