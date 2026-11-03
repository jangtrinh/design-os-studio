//! Image jobs read the pixels the canvas shows and keep the layer's framing.

use super::tests::{sample_app_with_image_layer, wait_for_job_completion, TestDir};
use super::*;
use designos_doc::{ImageAdjust, ImageParams};
use designos_render::tiny_skia::Pixmap;

const SIDE_BY_SIDE_TOLERANCE: i32 = 10;

/// A picture whose pixel `(x, y)` stores its own coordinates: red = `x % 256`,
/// green = `y % 256`, blue = `100 * (x / 256)`.
fn coordinate_picture(w: u32, h: u32) -> image::RgbaImage {
    image::RgbaImage::from_fn(w, h, |x, y| image::Rgba([(x % 256) as u8, (y % 256) as u8, (100 * (x / 256)) as u8, 255]))
}

fn coordinates_of(p: &image::Rgba<u8>) -> (u32, u32) {
    (p.0[0] as u32 + 256 * (p.0[2] as u32 / 100), p.0[1] as u32)
}

/// A smooth diagonal ramp: any flip, turn or shift changes most pixels.
fn gradient_picture(w: u32, h: u32) -> image::RgbaImage {
    image::RgbaImage::from_fn(w, h, |x, y| image::Rgba([(x * 255 / (w - 1)) as u8, (y * 255 / (h - 1)) as u8, ((x + y) * 255 / (w + h - 2)) as u8, 255]))
}

/// Replaces the sample layer's picture, size and image fill.
fn picture_layer(app: &mut App, node: NodeId, key: &str, picture: &image::RgbaImage, layer: Vec2, mode: ImageScaleMode, params: ImageParams) {
    app.doc.images.insert(key.to_string(), encode_png(picture).unwrap());
    let n = app.doc.get_mut(node).unwrap();
    n.size = layer;
    n.fills[0].kind = PaintKind::Image { image_ref: key.to_string(), scale_mode: mode, params };
}

/// Starts `op` with a stub agent that copies `input.png` back as `output.png`; returns the input it was given.
fn run_identity_job(app: &mut App, tmp: &TestDir, node: NodeId, op: ImageOp) -> image::RgbaImage {
    let job_dir = tmp.path().join("image-jobs").join("job-0001");
    app.image_jobs.agent_script = Some(format!("cp \"{}\" \"$DESIGNOS_IMAGE_OUTPUT\"", job_dir.join("input.png").display()));
    let job_id = app.start_image_job(node, 0, op).expect("job starts");
    assert_eq!(job_id, "job-0001");
    let input = image::open(job_dir.join("input.png")).unwrap().to_rgba8();
    wait_for_job_completion(app);
    input
}

fn erase() -> ImageOp {
    ImageOp::Erase { instruction: "tidy".into(), region: None }
}

fn result_node(app: &App) -> &Node {
    let children = &app.doc.get(app.page).unwrap().children;
    assert_eq!(children.len(), 2, "the result layer sits above the original");
    app.doc.get(children[1]).unwrap()
}

fn result_paint(app: &App) -> (ImageScaleMode, ImageParams) {
    match &result_node(app).fills[0].kind {
        PaintKind::Image { scale_mode, params, .. } => (*scale_mode, *params),
        other => panic!("expected an image fill, got {other:?}"),
    }
}

fn is_identity(m: Option<[f32; 6]>) -> bool {
    m.is_none_or(|m| m.iter().zip([1.0, 0.0, 0.0, 0.0, 1.0, 0.0]).all(|(a, b)| (a - b).abs() < 1e-3))
}

fn render_layer(app: &App, id: NodeId) -> Pixmap {
    designos_render::render_node(&app.doc, id, 1.0).expect("layer renders")
}

/// Fails unless at most 2% of the pixels differ by more than the tolerance.
fn assert_renders_alike(label: &str, before: &Pixmap, after: &Pixmap) {
    assert_eq!((before.width(), before.height()), (after.width(), after.height()), "{label}: same size");
    let total = (before.width() * before.height()) as usize;
    let off = before
        .data()
        .chunks_exact(4)
        .zip(after.data().chunks_exact(4))
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| (*x as i32 - *y as i32).abs() > SIDE_BY_SIDE_TOLERANCE))
        .count();
    assert!(off * 50 <= total, "{label}: {off} of {total} pixels differ from the original layer");
}

#[test]
fn identity_crop_job_sends_the_whole_image() {
    let tmp = TestDir::new("frame-identity-crop");
    let (mut app, node, key) = sample_app_with_image_layer(tmp.path());
    picture_layer(&mut app, node, &key, &coordinate_picture(400, 200), Vec2::new(100.0, 100.0), ImageScaleMode::Crop, ImageParams::default());

    let input = run_identity_job(&mut app, &tmp, node, erase());

    assert_eq!(input.dimensions(), (400, 200), "the canvas stretches the whole picture into the square, so the whole picture is sent");
    assert_eq!(coordinates_of(input.get_pixel(399, 199)), (399, 199));
    let req: JobRequestPayload = serde_json::from_str(&std::fs::read_to_string(tmp.path().join("image-jobs/job-0001/request.json")).unwrap()).unwrap();
    assert_eq!(req.expected_size, [400, 200]);
}

#[test]
fn axis_aligned_crop_job_sends_exactly_its_window() {
    let tmp = TestDir::new("frame-window-crop");
    let (mut app, node, key) = sample_app_with_image_layer(tmp.path());
    let params = ImageParams { transform: Some([0.5, 0.0, 0.25, 0.0, 0.5, 0.25]), ..Default::default() };
    picture_layer(&mut app, node, &key, &coordinate_picture(400, 200), Vec2::new(100.0, 100.0), ImageScaleMode::Crop, params);

    let input = run_identity_job(&mut app, &tmp, node, erase());

    assert_eq!(input.dimensions(), (200, 100), "u in 0.25..0.75 of 400 and v in 0.25..0.75 of 200");
    assert_eq!(coordinates_of(input.get_pixel(0, 0)), (100, 50));
    assert_eq!(coordinates_of(input.get_pixel(199, 99)), (299, 149));
}

#[test]
fn fill_and_fit_at_a_quarter_turn_send_the_rotated_picture() {
    // A quarter turn clockwise puts source pixel (x, y) of a 400x200 picture at (199 - y, x) of the 200x400 result.
    let picture = coordinate_picture(400, 200);
    for (mode, dims, first_pixel) in [(ImageScaleMode::Fill, (200, 200), (100, 199)), (ImageScaleMode::Fit, (200, 400), (0, 199))] {
        let tmp = TestDir::new("frame-quarter-turn");
        let (mut app, node, key) = sample_app_with_image_layer(tmp.path());
        let params = ImageParams { rotation: 90.0, ..Default::default() };
        picture_layer(&mut app, node, &key, &picture, Vec2::new(100.0, 100.0), mode, params);

        let input = run_identity_job(&mut app, &tmp, node, erase());

        assert_eq!(input.dimensions(), dims, "{mode:?}: the window is cut from the rotated picture");
        assert_eq!(coordinates_of(input.get_pixel(0, 0)), first_pixel, "{mode:?}: top-left pixel of the rotated picture");
    }
}

#[test]
fn fill_and_fit_at_no_turn_keep_their_windows() {
    let picture = coordinate_picture(400, 200);
    for (mode, dims, first_pixel) in [(ImageScaleMode::Fill, (200, 200), (100, 0)), (ImageScaleMode::Fit, (400, 200), (0, 0))] {
        let tmp = TestDir::new("frame-no-turn");
        let (mut app, node, key) = sample_app_with_image_layer(tmp.path());
        picture_layer(&mut app, node, &key, &picture, Vec2::new(100.0, 100.0), mode, ImageParams::default());

        let input = run_identity_job(&mut app, &tmp, node, erase());

        assert_eq!(input.dimensions(), dims, "{mode:?}");
        assert_eq!(coordinates_of(input.get_pixel(0, 0)), first_pixel, "{mode:?}");
    }
}

#[test]
fn erase_mask_follows_the_bitmap_the_job_sends() {
    let tmp = TestDir::new("frame-mask");
    let (mut app, node, key) = sample_app_with_image_layer(tmp.path());
    picture_layer(&mut app, node, &key, &coordinate_picture(400, 200), Vec2::new(100.0, 100.0), ImageScaleMode::Crop, ImageParams::default());
    app.image_jobs.agent_script = Some("sleep 0".into());

    app.start_image_job(node, 0, ImageOp::Erase { instruction: "tidy".into(), region: Some([10, 20, 30, 40]) }).expect("job starts");

    let mask = image::open(tmp.path().join("image-jobs/job-0001/mask.png")).unwrap().to_rgba8();
    assert_eq!(mask.dimensions(), (400, 200), "mask has the size of the image sent");
    assert_eq!(mask.get_pixel(15, 25).0, [255, 255, 255, 255]);
    app.cancel_image_job(Some(node));
}

#[test]
fn job_result_renders_like_the_layer_it_replaces() {
    // (label, layer size, mode, params): an identity job returns the bitmap it was given.
    let adjust = ImageAdjust { exposure: 0.3, contrast: -0.2, ..Default::default() };
    let cases: Vec<(&str, Vec2, ImageScaleMode, ImageParams)> = vec![
        ("fill", Vec2::new(100.0, 100.0), ImageScaleMode::Fill, ImageParams::default()),
        ("fill wide layer", Vec2::new(160.0, 60.0), ImageScaleMode::Fill, ImageParams::default()),
        ("fit quarter turn", Vec2::new(100.0, 100.0), ImageScaleMode::Fit, ImageParams { rotation: 90.0, ..Default::default() }),
        ("fill quarter turn", Vec2::new(100.0, 100.0), ImageScaleMode::Fill, ImageParams { rotation: 90.0, adjust, ..Default::default() }),
        ("fill half turn", Vec2::new(160.0, 60.0), ImageScaleMode::Fill, ImageParams { rotation: 180.0, ..Default::default() }),
        ("fill three quarter turns", Vec2::new(100.0, 100.0), ImageScaleMode::Fill, ImageParams { rotation: 270.0, ..Default::default() }),
        ("crop identity", Vec2::new(100.0, 100.0), ImageScaleMode::Crop, ImageParams::default()),
        ("crop window", Vec2::new(100.0, 100.0), ImageScaleMode::Crop, ImageParams { transform: Some([0.5, 0.0, 0.25, 0.0, 0.5, 0.25]), ..Default::default() }),
        (
            "crop zoomed out",
            Vec2::new(100.0, 100.0),
            ImageScaleMode::Crop,
            ImageParams { transform: Some([2.0, 0.0, -0.5, 0.0, 2.0, -0.5]), ..Default::default() },
        ),
        (
            "crop turned and sheared",
            Vec2::new(100.0, 100.0),
            ImageScaleMode::Crop,
            ImageParams { transform: Some([0.6, -0.3, 0.4, 0.3, 0.6, 0.1]), adjust, ..Default::default() },
        ),
    ];
    for (label, layer, mode, params) in cases {
        let tmp = TestDir::new("frame-render");
        let (mut app, node, key) = sample_app_with_image_layer(tmp.path());
        picture_layer(&mut app, node, &key, &gradient_picture(400, 200), layer, mode, params);
        let before = render_layer(&app, node);

        run_identity_job(&mut app, &tmp, node, erase());

        let after = render_layer(&app, result_node(&app).id);
        assert_renders_alike(label, &before, &after);
    }
}

#[test]
fn job_result_keeps_the_original_adjustments_and_flags() {
    let tmp = TestDir::new("frame-carry");
    let (mut app, node, key) = sample_app_with_image_layer(tmp.path());
    let adjust = ImageAdjust { exposure: 0.25, contrast: -0.5, vibrance: 0.75, brightness: 0.125, ..Default::default() };
    let params = ImageParams { adjust, color_manage: Some(true), tile_scale: 0.4, original_size: Some((1440, 960)), ..Default::default() };
    picture_layer(&mut app, node, &key, &coordinate_picture(400, 200), Vec2::new(100.0, 100.0), ImageScaleMode::Crop, params);

    run_identity_job(&mut app, &tmp, node, erase());

    let (mode, got) = result_paint(&app);
    assert_eq!(mode, ImageScaleMode::Crop);
    assert_eq!(got.adjust, adjust, "adjustments are carried");
    assert_eq!(got.color_manage, Some(true), "colour management flag is carried");
    assert_eq!(got.tile_scale, 0.4, "tile scale is carried");
    assert!(is_identity(got.transform), "the whole picture was sent, so the transform stays identity");
    assert_eq!(got.rotation, 0.0);
    assert_eq!(got.original_size, Some((400, 200)), "the recorded original size follows the bitmap that replaced the picture");
}

#[test]
fn quarter_turn_is_baked_into_the_result_bitmap() {
    let tmp = TestDir::new("frame-bake");
    let (mut app, node, key) = sample_app_with_image_layer(tmp.path());
    let adjust = ImageAdjust { highlights: -0.4, ..Default::default() };
    let params = ImageParams { rotation: 90.0, adjust, ..Default::default() };
    picture_layer(&mut app, node, &key, &coordinate_picture(400, 200), Vec2::new(100.0, 100.0), ImageScaleMode::Fit, params);

    run_identity_job(&mut app, &tmp, node, erase());

    let (mode, got) = result_paint(&app);
    assert_eq!(mode, ImageScaleMode::Fit, "scale mode is kept");
    assert_eq!(got.rotation, 0.0, "the quarter turn lives in the bitmap now");
    assert_eq!(got.adjust, adjust);
}

#[test]
fn crop_window_result_is_a_plain_stretch_of_the_sent_window() {
    let tmp = TestDir::new("frame-window-result");
    let (mut app, node, key) = sample_app_with_image_layer(tmp.path());
    let params = ImageParams { transform: Some([0.5, 0.0, 0.25, 0.0, 0.5, 0.25]), ..Default::default() };
    picture_layer(&mut app, node, &key, &coordinate_picture(400, 200), Vec2::new(100.0, 100.0), ImageScaleMode::Crop, params);

    run_identity_job(&mut app, &tmp, node, erase());

    let (mode, got) = result_paint(&app);
    assert_eq!(mode, ImageScaleMode::Crop);
    assert!(is_identity(got.transform), "the sent bitmap is the window, so the result transform is identity: {:?}", got.transform);
}

#[test]
fn expand_grows_an_identity_crop_by_the_pixels_it_sent() {
    let tmp = TestDir::new("frame-expand-crop");
    let (mut app, node, key) = sample_app_with_image_layer(tmp.path());
    picture_layer(&mut app, node, &key, &coordinate_picture(400, 200), Vec2::new(100.0, 100.0), ImageScaleMode::Crop, ImageParams::default());
    let expanded = tmp.path().join("expanded_500x300.png");
    image::RgbaImage::from_pixel(500, 300, image::Rgba([10, 20, 30, 255])).save(&expanded).unwrap();
    app.image_jobs.agent_script = Some(format!("cp \"{}\" \"$DESIGNOS_IMAGE_OUTPUT\"", expanded.display()));

    let job_id = app.start_image_job(node, 0, ImageOp::Expand { top: 50, right: 50, bottom: 50, left: 50 }).expect("expand starts");
    let input = image::open(tmp.path().join("image-jobs").join(&job_id).join("input.png")).unwrap().to_rgba8();
    assert_eq!(input.dimensions(), (400, 200), "the whole picture is the visible window");
    wait_for_job_completion(&mut app);

    let grown = result_node(&app);
    assert_eq!(grown.size, Vec2::new(125.0, 150.0), "0.25 and 0.5 layer units per picture pixel across and down");
    assert_eq!((grown.transform.tx, grown.transform.ty), (-12.5, -25.0));
}

#[test]
fn expand_is_refused_when_the_crop_is_not_a_plain_window() {
    let tmp = TestDir::new("frame-expand-refused");
    let (mut app, node, key) = sample_app_with_image_layer(tmp.path());
    let params = ImageParams { transform: Some([0.6, -0.3, 0.4, 0.3, 0.6, 0.1]), ..Default::default() };
    picture_layer(&mut app, node, &key, &coordinate_picture(400, 200), Vec2::new(100.0, 100.0), ImageScaleMode::Crop, params);

    let started = app.start_image_job(node, 0, ImageOp::Expand { top: 10, right: 10, bottom: 10, left: 10 });

    assert!(started.is_none(), "expanding a turned crop has no meaning on the canvas");
    assert!(app.toast.as_ref().is_some_and(|t| t.error));
}

#[test]
fn a_crop_that_shows_no_part_of_its_image_starts_no_job() {
    let tmp = TestDir::new("frame-empty-crop");
    let (mut app, node, key) = sample_app_with_image_layer(tmp.path());
    let params = ImageParams { transform: Some([0.5, 0.0, 2.0, 0.0, 0.5, 0.0]), ..Default::default() };
    picture_layer(&mut app, node, &key, &coordinate_picture(400, 200), Vec2::new(100.0, 100.0), ImageScaleMode::Crop, params);

    assert!(app.start_image_job(node, 0, erase()).is_none(), "the window lies right of the picture");
    assert!(app.toast.as_ref().is_some_and(|t| t.error));
}

mod frame_math {
    use super::super::framing::*;
    use designos_doc::{image_uv_map, ImageParams, ImageScaleMode, Vec2};

    /// The windows Fill and Fit cut before the picture could be rotated or cropped, kept as the oracle.
    fn untransformed_window(raw_w: u32, raw_h: u32, layer: Vec2, mode: ImageScaleMode) -> (u32, u32, u32, u32) {
        let (rw, rh) = (raw_w.max(1), raw_h.max(1));
        let (iw, ih) = (rw as f32, rh as f32);
        let (lx, ly) = (layer.x.max(1.0), layer.y.max(1.0));
        match mode {
            ImageScaleMode::Fill => {
                let s = (lx / iw).max(ly / ih).max(1e-6);
                let crop_w = (lx / s).round().clamp(1.0, iw) as u32;
                let crop_h = (ly / s).round().clamp(1.0, ih) as u32;
                (rw.saturating_sub(crop_w) / 2, rh.saturating_sub(crop_h) / 2, crop_w, crop_h)
            }
            _ => (0, 0, rw, rh),
        }
    }

    #[test]
    fn fill_and_fit_without_a_turn_cut_the_same_windows_as_before() {
        for (w, h) in [(400, 200), (200, 400), (333, 77), (1, 1), (4096, 3), (640, 640)] {
            for (lx, ly) in [(100.0, 100.0), (160.0, 60.0), (7.0, 301.0), (0.2, 0.2), (1234.5, 99.9)] {
                for mode in [ImageScaleMode::Fill, ImageScaleMode::Fit] {
                    let layer = Vec2::new(lx, ly);
                    let frame = plan_image_job_frame(w, h, layer, mode, &ImageParams::default()).unwrap();
                    assert_eq!(frame.window, untransformed_window(w, h, layer, mode), "{mode:?} {w}x{h} on {lx}x{ly}");
                    assert_eq!(frame.quarter_turns, 0);
                    assert_eq!(frame.params, ImageParams::default());
                }
            }
        }
    }

    #[test]
    fn a_rebased_crop_transform_keeps_every_node_point_on_the_same_picture_pixel() {
        let (iw, ih) = (400u32, 200u32);
        let layer = Vec2::new(120.0, 80.0);
        let transforms = [
            [0.5, 0.0, 0.25, 0.0, 0.5, 0.25],
            [2.0, 0.0, -0.5, 0.0, 2.0, -0.5],
            [0.6, -0.3, 0.4, 0.3, 0.6, 0.1],
            [-0.5, 0.0, 0.9, 0.0, 0.5, 0.2],
            [0.0, 0.7, 0.1, -0.6, 0.0, 0.9],
            [0.333, 0.0, 0.1234, 0.0, 0.777, 0.0321],
        ];
        for m in transforms {
            let params = ImageParams { transform: Some(m), ..Default::default() };
            let frame = plan_image_job_frame(iw, ih, layer, ImageScaleMode::Crop, &params).unwrap();
            let (x0, y0, bw, bh) = frame.window;
            let before = image_uv_map(layer, iw as f32, ih as f32, ImageScaleMode::Crop, &params).unwrap();
            let after = frame_uv_map(layer, ImageScaleMode::Crop, &frame).unwrap();
            for (x, y) in [(0.0, 0.0), (layer.x, 0.0), (0.0, layer.y), (layer.x, layer.y), (31.0, 47.0)] {
                let (u, v) = before.apply(x, y);
                let (bu, bv) = after.apply(x, y);
                // Where the point lands in the source picture, from the bitmap and from the picture itself.
                let from_bitmap = (x0 as f32 + bu * bw as f32, y0 as f32 + bv * bh as f32);
                let from_picture = (u * iw as f32, v * ih as f32);
                let slack = if frame.params.transform.is_none() { 1.0 } else { 1e-2 };
                assert!(
                    (from_bitmap.0 - from_picture.0).abs() <= slack && (from_bitmap.1 - from_picture.1).abs() <= slack,
                    "{m:?} at ({x}, {y}): {from_bitmap:?} vs {from_picture:?}"
                );
            }
        }
    }

    #[test]
    fn a_window_inside_the_picture_is_a_plain_stretch_and_a_leaking_one_is_not() {
        let inside = ImageParams { transform: Some([0.5, 0.0, 0.25, 0.0, 0.5, 0.25]), ..Default::default() };
        assert_eq!(plan_image_job_frame(400, 200, Vec2::new(100.0, 100.0), ImageScaleMode::Crop, &inside).unwrap().params.transform, None);
        let leaking = ImageParams { transform: Some([1.5, 0.0, -0.25, 0.0, 1.0, 0.0]), ..Default::default() };
        let frame = plan_image_job_frame(400, 200, Vec2::new(100.0, 100.0), ImageScaleMode::Crop, &leaking).unwrap();
        assert_eq!(frame.window, (0, 0, 400, 200), "the visible window is clipped to the picture");
        assert!(frame.params.transform.is_some(), "the empty margins are part of the framing");
        let flipped = ImageParams { transform: Some([-1.0, 0.0, 1.0, 0.0, 1.0, 0.0]), ..Default::default() };
        let frame = plan_image_job_frame(400, 200, Vec2::new(100.0, 100.0), ImageScaleMode::Crop, &flipped).unwrap();
        assert!(frame.params.transform.is_some(), "a mirrored crop is not a plain stretch");
    }

    #[test]
    fn crop_windows_that_show_nothing_plan_no_frame() {
        let size = Vec2::new(100.0, 100.0);
        for m in [[0.0; 6], [0.5, 0.0, 2.0, 0.0, 0.5, 0.0], [0.5, 0.0, -1.0, 0.0, 0.5, 0.0], [f32::NAN, 0.0, 0.0, 0.0, 1.0, 0.0]] {
            let params = ImageParams { transform: Some(m), ..Default::default() };
            assert_eq!(plan_image_job_frame(400, 200, size, ImageScaleMode::Crop, &params), None, "{m:?}");
        }
    }

    #[test]
    fn region_and_local_rect_map_back_and_forth_for_a_cropped_layer() {
        let layer = Vec2::new(100.0, 100.0);
        let params = ImageParams { transform: Some([0.5, 0.0, 0.25, 0.0, 0.5, 0.25]), ..Default::default() };
        let frame = plan_image_job_frame(400, 200, layer, ImageScaleMode::Crop, &params).unwrap();
        assert_eq!(frame.dims(), (200, 100));
        let map = frame_uv_map(layer, ImageScaleMode::Crop, &frame).unwrap();
        let region = local_rect_to_bitmap_region(&map, frame.dims(), Vec2::new(10.0, 20.0), Vec2::new(60.0, 70.0)).unwrap();
        assert_eq!(region, [20, 20, 100, 50], "a layer rectangle covers the pixels of the window it shows");
        let (x, y, w, h) = bitmap_region_to_local_rect(&map, frame.dims(), region).unwrap();
        assert!((x - 10.0).abs() < 1e-3 && (y - 20.0).abs() < 1e-3 && (w - 50.0).abs() < 1e-3 && (h - 50.0).abs() < 1e-3, "{x} {y} {w} {h}");
        assert_eq!(local_rect_to_bitmap_region(&map, frame.dims(), Vec2::new(10.0, 10.0), Vec2::new(10.2, 60.0)), None, "less than a pixel wide");
    }

    #[test]
    fn expand_is_refused_only_for_a_crop_that_is_not_a_plain_stretch() {
        let layer = Vec2::new(100.0, 100.0);
        let plain = plan_image_job_frame(400, 200, layer, ImageScaleMode::Crop, &ImageParams::default()).unwrap();
        assert_eq!(expand_refusal(ImageScaleMode::Crop, &plain), None);
        let turned = ImageParams { transform: Some([0.0, 0.7, 0.1, -0.6, 0.0, 0.9]), ..Default::default() };
        let turned = plan_image_job_frame(400, 200, layer, ImageScaleMode::Crop, &turned).unwrap();
        assert!(expand_refusal(ImageScaleMode::Crop, &turned).is_some());
        let fill = plan_image_job_frame(400, 200, layer, ImageScaleMode::Fill, &ImageParams { rotation: 90.0, ..Default::default() }).unwrap();
        assert_eq!(expand_refusal(ImageScaleMode::Fill, &fill), None, "a quarter turn is baked into the bitmap, so Expand works");
    }
}

mod tile_erase_drag {
    use super::super::framing::*;
    use super::super::{page_drag_to_image_region, page_drag_to_local_rect};
    use designos_doc::{Affine, ImageParams, ImageScaleMode, Vec2};
    use egui::Pos2;

    const WORLD: Affine = Affine { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx: 0.0, ty: 0.0 };

    /// An 80x80 picture tiled at `tile_scale` over a 160x160 layer.
    fn tiled_frame(tile_scale: f32) -> ImageJobFrame {
        let params = ImageParams { tile_scale, ..Default::default() };
        plan_image_job_frame(80, 80, Vec2::new(160.0, 160.0), ImageScaleMode::Tile, &params).expect("a tiled fill shows its picture")
    }

    fn drag(frame: &ImageJobFrame, from: (f32, f32), to: (f32, f32)) -> Option<[u32; 4]> {
        page_drag_to_image_region(WORLD, Vec2::new(160.0, 160.0), ImageScaleMode::Tile, frame, Pos2::new(from.0, from.1), Pos2::new(to.0, to.1))
    }

    #[test]
    fn a_drag_on_a_later_copy_maps_to_the_pixels_of_that_copy() {
        // The top-right copy of four shows source pixels 20..60 in the middle of its 80x80 cell.
        assert_eq!(drag(&tiled_frame(1.0), (100.0, 20.0), (140.0, 60.0)), Some([20, 20, 40, 40]));
        assert_eq!(drag(&tiled_frame(1.0), (20.0, 100.0), (60.0, 140.0)), Some([20, 20, 40, 40]), "bottom-left copy");
        assert_eq!(drag(&tiled_frame(1.0), (140.0, 60.0), (100.0, 20.0)), Some([20, 20, 40, 40]), "dragged the other way");
    }

    #[test]
    fn a_drag_across_a_tile_edge_takes_the_whole_bitmap_on_that_axis() {
        // The vertical tile edge sits at x = 80: one mask cannot hold two wrapped intervals, so x is the whole width.
        assert_eq!(drag(&tiled_frame(1.0), (60.0, 20.0), (100.0, 60.0)), Some([0, 20, 80, 40]));
        assert_eq!(drag(&tiled_frame(1.0), (20.0, 60.0), (60.0, 100.0)), Some([20, 0, 40, 80]), "horizontal tile edge");
        assert_eq!(drag(&tiled_frame(1.0), (10.0, 10.0), (150.0, 30.0)), Some([0, 10, 80, 20]), "spanning more than one copy");
    }

    #[test]
    fn a_drag_ending_exactly_on_a_tile_edge_stays_inside_its_copy() {
        assert_eq!(drag(&tiled_frame(1.0), (20.0, 20.0), (80.0, 60.0)), Some([20, 20, 60, 40]));
        assert_eq!(drag(&tiled_frame(1.0), (80.0, 20.0), (120.0, 60.0)), Some([0, 20, 40, 40]), "starting on the edge");
    }

    #[test]
    fn a_half_scale_tile_maps_a_drag_in_the_third_column_copy() {
        // At tile scale 0.5 each copy is 40 layer units wide, so x 80..120 is the third column and y 40..80 the second row.
        assert_eq!(drag(&tiled_frame(0.5), (90.0, 52.0), (110.0, 70.0)), Some([20, 24, 40, 36]));
        assert_eq!(drag(&tiled_frame(0.5), (45.0, 45.0), (75.0, 75.0)), Some([10, 10, 60, 60]));
    }

    #[test]
    fn a_drag_that_leaves_the_layer_is_cut_to_the_layer() {
        assert_eq!(drag(&tiled_frame(1.0), (-20.0, 20.0), (40.0, 60.0)), Some([0, 20, 40, 40]));
        assert_eq!(drag(&tiled_frame(1.0), (120.0, 100.0), (400.0, 140.0)), Some([40, 20, 40, 40]));
    }

    #[test]
    fn a_tile_drag_under_a_pixel_is_dropped() {
        assert_eq!(drag(&tiled_frame(1.0), (100.0, 20.0), (100.4, 60.0)), None);
    }
    #[test]
    fn the_overlay_of_a_tile_drag_is_the_dragged_rectangle() {
        let frame = tiled_frame(1.0);
        let layer = Vec2::new(160.0, 160.0);
        let (start, cur) = (Pos2::new(100.0, 20.0), Pos2::new(140.0, 60.0));
        let region = page_drag_to_image_region(WORLD, layer, ImageScaleMode::Tile, &frame, start, cur).unwrap();
        assert_eq!(region, [20, 20, 40, 40]);
        let drag = page_drag_to_local_rect(WORLD, layer, start, cur);
        assert_eq!(drag, Some((100.0, 20.0, 40.0, 40.0)));
        assert_eq!(erase_overlay_local_rect(layer, ImageScaleMode::Tile, &frame, region, drag), Some((100.0, 20.0, 40.0, 40.0)), "not the first copy");
        // The inverse of a mask that holds one copy's pixels lands on the first copy, which is why the overlay does not use it.
        let map = frame_uv_map(layer, ImageScaleMode::Tile, &frame).unwrap();
        let (x, y, w, h) = bitmap_region_to_local_rect(&map, frame.dims(), region).unwrap();
        assert!((x - 20.0).abs() < 1e-3 && (y - 20.0).abs() < 1e-3 && (w - 40.0).abs() < 1e-3 && (h - 40.0).abs() < 1e-3, "{x} {y} {w} {h}");
        // Dragged the other way and past the layer edge, the overlay is cut to the layer.
        let drag = page_drag_to_local_rect(WORLD, layer, Pos2::new(200.0, 60.0), Pos2::new(140.0, 20.0));
        assert_eq!(drag, Some((140.0, 20.0, 20.0, 40.0)));
    }

    #[test]
    fn the_overlay_of_a_fit_drag_is_still_the_inverse_of_the_mask() {
        let layer = Vec2::new(100.0, 100.0);
        let frame = plan_image_job_frame(400, 200, layer, ImageScaleMode::Fit, &ImageParams::default()).unwrap();
        let (x, y, w, h) = erase_overlay_local_rect(layer, ImageScaleMode::Fit, &frame, [40, 40, 200, 100], None).unwrap();
        assert!((x - 10.0).abs() < 1e-3 && (y - 35.0).abs() < 1e-3 && (w - 50.0).abs() < 1e-3 && (h - 25.0).abs() < 1e-3, "{x} {y} {w} {h}");
    }
}
