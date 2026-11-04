//! Unit tests of the image fill adjustment helpers.

use super::*;
use crate::{render_page, Viewport};
use designos_doc::{BlendMode, Document, Node, Rect};

fn flat_png(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
    let mut pm = sk::Pixmap::new(w, h).expect("pixmap");
    pm.fill(sk::Color::from_rgba8(rgba[0], rgba[1], rgba[2], rgba[3]));
    pm.encode_png().expect("png")
}

fn image_paint(image_ref: &str, mode: ImageScaleMode, params: ImageParams) -> Paint {
    Paint { kind: PaintKind::Image { image_ref: image_ref.into(), scale_mode: mode, params }, opacity: 1.0, visible: true, blend: BlendMode::Normal }
}

/// A 4000x3000 picture drawn at its native size, of which a 64x64 window is on screen.
#[test]
fn adjusted_fill_work_is_bounded_by_the_visible_region() {
    let mut doc = Document::new("t");
    let page = doc.add_page("p");
    doc.images.insert("photo".into(), flat_png(4000, 3000, [120, 90, 60, 255]));
    let id = doc.alloc_id();
    let mut node = Node::rectangle(id, Rect::new(0.0, 0.0, 4000.0, 3000.0));
    node.fills =
        vec![image_paint("photo", ImageScaleMode::Fill, ImageParams { adjust: ImageAdjust { exposure: 1.0, ..Default::default() }, ..Default::default() })];
    doc.insert(page, node).expect("insert");

    let vp = Viewport { offset: Vec2::new(1800.0, 1400.0), zoom: 1.0, dpr: 1.0 };
    let mut pm = sk::Pixmap::new(64, 64).expect("target");
    ADJUSTED_PIXELS.with(|n| n.set(0));
    render_page(&doc, page, &vp, &mut pm);
    let adjusted = ADJUSTED_PIXELS.with(|n| n.get());

    assert!(adjusted > 0, "the fill must be adjusted");
    assert!(adjusted <= 64 * 64, "adjustment ran on {adjusted} pixels, more than the 64x64 window that is on screen");
    let centre = pm.pixel(32, 32).expect("pixel");
    assert!(centre.red() > 120 && centre.green() > 90 && centre.blue() > 60, "the visible pixel is brightened: {centre:?}");
}

fn near(a: [f32; 6], b: [f32; 6]) -> bool {
    a.iter().zip(b.iter()).all(|(x, y)| (x - y).abs() < 1e-4)
}

fn uv_at(m: [f32; 6], size: Vec2, x: f32, y: f32) -> (f32, f32) {
    let (u, v) = (x / size.x, y / size.y);
    (m[0] * u + m[1] * v + m[2], m[3] * u + m[4] * v + m[5])
}

#[test]
fn four_quarter_turns_return_the_frame_and_the_two_directions_undo_each_other() {
    let frame = [0.8, 0.1, 0.05, -0.2, 1.3, 0.4];
    for size in [Vec2::new(2.0, 2.0), Vec2::new(200.0, 100.0), Vec2::new(100.0, 200.0), Vec2::new(37.0, 911.0)] {
        let mut m = frame;
        for _ in 0..4 {
            m = rotate_crop_transform(m, size, true).expect("rotates");
        }
        assert!(near(m, frame), "four clockwise turns on {size:?}: {m:?}");
        let there = rotate_crop_transform(frame, size, true).unwrap();
        assert!(near(rotate_crop_transform(there, size, false).unwrap(), frame), "counter-clockwise undoes clockwise on {size:?}");
    }
}

#[test]
fn a_quarter_turn_moves_the_picture_rigidly_about_the_node_centre() {
    let frame = [0.8, 0.1, 0.05, -0.2, 1.3, 0.4];
    let size = Vec2::new(200.0, 100.0);
    let (cx, cy) = (size.x / 2.0, size.y / 2.0);
    let cw = rotate_crop_transform(frame, size, true).unwrap();
    let ccw = rotate_crop_transform(frame, size, false).unwrap();
    assert_eq!(uv_at(cw, size, cx, cy).0 - uv_at(frame, size, cx, cy).0, 0.0, "the point under the centre stays put");
    for (x, y) in [(10.0, 20.0), (150.0, 30.0), (60.0, 90.0)] {
        // Clockwise: the node point shows what the point turned back counter-clockwise about the centre showed.
        let (bx, by) = (cx + (y - cy), cy - (x - cx));
        let (got, want) = (uv_at(cw, size, x, y), uv_at(frame, size, bx, by));
        assert!((got.0 - want.0).abs() < 1e-4 && (got.1 - want.1).abs() < 1e-4, "clockwise at ({x}, {y}): {got:?} vs {want:?}");
        let (bx, by) = (cx - (y - cy), cy + (x - cx));
        let (got, want) = (uv_at(ccw, size, x, y), uv_at(frame, size, bx, by));
        assert!((got.0 - want.0).abs() < 1e-4 && (got.1 - want.1).abs() < 1e-4, "counter-clockwise at ({x}, {y}): {got:?} vs {want:?}");
    }
}

#[test]
fn a_degenerate_node_or_frame_cannot_be_turned() {
    let id = IDENTITY_CROP_TRANSFORM;
    assert_eq!(rotate_crop_transform(id, Vec2::new(0.0, 10.0), true), None);
    assert_eq!(rotate_crop_transform(id, Vec2::new(f32::NAN, 10.0), true), None);
    assert_eq!(rotate_crop_transform([f32::INFINITY, 0.0, 0.0, 0.0, 1.0, 0.0], Vec2::new(10.0, 10.0), false), None);
}

#[test]
fn the_identity_crop_turned_clockwise_is_the_fill_rotation_of_90_degrees_on_a_square_node() {
    let size = Vec2::new(2.0, 2.0);
    let turned = rotate_crop_transform(IDENTITY_CROP_TRANSFORM, size, true).unwrap();
    let fill90 = designos_doc::image_uv_map(size, 2.0, 2.0, ImageScaleMode::Fill, &ImageParams { rotation: 90.0, ..Default::default() }).unwrap();
    let crop = designos_doc::image_uv_map(size, 2.0, 2.0, ImageScaleMode::Crop, &ImageParams { transform: Some(turned), ..Default::default() }).unwrap();
    for (x, y) in [(0.5, 0.5), (1.5, 0.5), (0.5, 1.5), (1.5, 1.5)] {
        let (a, b) = (crop.apply(x, y), fill90.apply(x, y));
        assert!((a.0 - b.0).abs() < 1e-5 && (a.1 - b.1).abs() < 1e-5, "({x}, {y}): {a:?} vs {b:?}");
    }
}

#[test]
fn a_fill_frame_at_90_degrees_turned_once_is_the_half_turn_fill_frame() {
    let size = Vec2::new(200.0, 100.0);
    let frame = |rotation: f32| crop_transform_from_framing(size, 100.0, 100.0, ImageScaleMode::Fill, &ImageParams { rotation, ..Default::default() }).unwrap();
    assert!(near(rotate_crop_transform(frame(90.0), size, true).unwrap(), frame(180.0)));
    assert!(near(rotate_crop_transform(frame(180.0), size, false).unwrap(), frame(90.0)));
}

fn picture_paint(mode: ImageScaleMode, transform: Option<[f32; 6]>, rotation: f32) -> Paint {
    image_paint("picture", mode, ImageParams { transform, rotation, ..Default::default() })
}

fn transform_of(paint: &Paint) -> Option<[f32; 6]> {
    match &paint.kind {
        PaintKind::Image { params, .. } => params.transform,
        _ => panic!("image paint"),
    }
}

fn rotation_of(paint: &Paint) -> f32 {
    match &paint.kind {
        PaintKind::Image { params, .. } => params.rotation,
        _ => panic!("image paint"),
    }
}

#[test]
fn entering_crop_frames_each_node_by_its_own_size_and_angle() {
    let picture = || Some((100, 100));
    for mode in [ImageScaleMode::Fill, ImageScaleMode::Fit, ImageScaleMode::Tile] {
        for size in [Vec2::new(200.0, 100.0), Vec2::new(100.0, 200.0)] {
            let old = picture_paint(mode, None, 90.0);
            let mut next = picture_paint(ImageScaleMode::Crop, Some([9.0, 9.0, 9.0, 9.0, 9.0, 9.0]), 0.0);
            crop_paint_for_node(&old, &mut next, size, picture, 0);
            let PaintKind::Image { params, .. } = &old.kind else { unreachable!() };
            assert_eq!(transform_of(&next), crop_transform_from_framing(size, 100.0, 100.0, mode, params), "{mode:?} on {size:?}");
            assert_eq!(rotation_of(&next), 90.0, "the angle shown is the node's own");
        }
    }
}

#[test]
fn entering_crop_keeps_a_frame_the_node_stored_and_asks_no_picture_size() {
    let stored = [0.5, 0.0, 0.1, 0.0, 0.5, 0.2];
    let old = picture_paint(ImageScaleMode::Fill, Some(stored), 0.0);
    let mut next = picture_paint(ImageScaleMode::Crop, None, 0.0);
    crop_paint_for_node(&old, &mut next, Vec2::new(200.0, 100.0), || panic!("the picture size is not needed"), 0);
    assert_eq!(transform_of(&next), Some(stored));
}

#[test]
fn entering_crop_with_an_unknown_picture_invents_no_frame() {
    let old = picture_paint(ImageScaleMode::Fit, None, 0.0);
    let mut next = picture_paint(ImageScaleMode::Crop, Some([2.0, 0.0, 0.0, 0.0, 2.0, 0.0]), 0.0);
    crop_paint_for_node(&old, &mut next, Vec2::new(200.0, 100.0), || None, 0);
    assert_eq!(transform_of(&next), None, "the node had no frame and none is made up");
}

#[test]
fn a_cropped_node_keeps_its_own_frame_until_it_is_turned() {
    let (wide, tall) = (Vec2::new(200.0, 100.0), Vec2::new(100.0, 200.0));
    let own = [0.6, 0.0, 0.2, 0.0, 0.9, 0.05];
    let old = picture_paint(ImageScaleMode::Crop, Some(own), 0.0);

    let mut next = picture_paint(ImageScaleMode::Crop, Some([1.0, 0.0, 0.0, 0.0, 1.0, 0.0]), 0.0);
    crop_paint_for_node(&old, &mut next, wide, || panic!("no picture size needed"), 0);
    assert_eq!(transform_of(&next), Some(own), "an edit that does not turn leaves the stored frame");

    for size in [wide, tall] {
        let mut next = picture_paint(ImageScaleMode::Crop, Some([1.0, 0.0, 0.0, 0.0, 1.0, 0.0]), 90.0);
        crop_paint_for_node(&old, &mut next, size, || None, 1);
        assert_eq!(transform_of(&next), rotate_crop_transform(own, size, true), "one clockwise click on {size:?}");
        assert_eq!(rotation_of(&next), 90.0);
    }
    let mut next = picture_paint(ImageScaleMode::Crop, None, 270.0);
    crop_paint_for_node(&old, &mut next, wide, || None, -1);
    assert_eq!(transform_of(&next), rotate_crop_transform(own, wide, false), "one counter-clockwise click");
    assert_eq!(rotation_of(&next), 270.0);
}

#[test]
fn a_turn_on_an_uncropped_frame_starts_from_the_identity_and_other_edits_pass_through() {
    let size = Vec2::new(40.0, 10.0);
    let old = picture_paint(ImageScaleMode::Crop, None, 0.0);
    let mut next = picture_paint(ImageScaleMode::Crop, None, 90.0);
    crop_paint_for_node(&old, &mut next, size, || None, 1);
    assert_eq!(transform_of(&next), rotate_crop_transform(IDENTITY_CROP_TRANSFORM, size, true));

    // Another picture, or a mode other than Crop, is written exactly as the picker has it.
    let other = image_paint("other", ImageScaleMode::Fill, ImageParams::default());
    let mut next = picture_paint(ImageScaleMode::Crop, Some([3.0, 0.0, 0.0, 0.0, 3.0, 0.0]), 0.0);
    crop_paint_for_node(&other, &mut next, size, || Some((10, 10)), 0);
    assert_eq!(transform_of(&next), Some([3.0, 0.0, 0.0, 0.0, 3.0, 0.0]));
    let old = picture_paint(ImageScaleMode::Fill, None, 0.0);
    let mut next = picture_paint(ImageScaleMode::Tile, Some([3.0, 0.0, 0.0, 0.0, 3.0, 0.0]), 0.0);
    crop_paint_for_node(&old, &mut next, size, || Some((10, 10)), 0);
    assert_eq!(transform_of(&next), Some([3.0, 0.0, 0.0, 0.0, 3.0, 0.0]));
}

fn flat_scene(adjust: ImageAdjust) -> (Document, designos_doc::NodeId) {
    let mut doc = Document::new("t");
    let page = doc.add_page("p");
    doc.images.insert("flat".into(), flat_png(8, 8, [100, 100, 100, 255]));
    let id = doc.alloc_id();
    let mut node = Node::rectangle(id, Rect::new(0.0, 0.0, 8.0, 8.0));
    node.fills = vec![image_paint("flat", ImageScaleMode::Fill, ImageParams { adjust, ..Default::default() })];
    doc.insert(page, node).expect("insert");
    (doc, page)
}

fn centre_red(doc: &Document, page: designos_doc::NodeId) -> u8 {
    let mut pm = sk::Pixmap::new(8, 8).expect("target");
    render_page(doc, page, &Viewport::default(), &mut pm);
    pm.pixel(4, 4).unwrap().red()
}

#[test]
fn a_fill_over_the_scratch_limit_is_drawn_unadjusted_and_counted() {
    let (doc, page) = flat_scene(ImageAdjust { exposure: 1.0, ..Default::default() });
    let adjusted = centre_red(&doc, page);
    assert!(adjusted > 100, "within the limit the fill is adjusted: {adjusted}");

    let counted = unadjusted_fill_fallbacks();
    TEST_MAX_PIXELS.with(|l| l.set(Some(4)));
    let degraded = centre_red(&doc, page);
    TEST_MAX_PIXELS.with(|l| l.set(None));
    assert_eq!(degraded, 100, "over the limit the picture is drawn as it is, not left out");
    assert!(unadjusted_fill_fallbacks() > counted, "the fallback leaves a count");
}
