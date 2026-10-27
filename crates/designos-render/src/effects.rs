//! Drop shadows, inner shadows and background blur, computed on small
//! region pixmaps around the node and composited onto the target.

use crate::blur::{gaussian_blur_at, Edges};
use crate::paint::sk_color;
use designos_doc as d;
use std::cell::Cell;
use tiny_skia as sk;

/// Draws the node silhouette into a pixmap: `(pixmap, dx, dy, grow, mode)`.
/// `dx, dy` translate in device pixels after the node transform, `grow` expands
/// (positive) or erodes (negative) the shape in local units, `mode` picks the silhouette kind (`Opaque` and `Knockout` ignore
/// fill alpha).
pub type Sil<'a> = &'a dyn Fn(&mut sk::Pixmap, f32, f32, f32, d::SilhouetteMode);

const MAX_REGION_PIXELS: u64 = 48_000_000;

thread_local! {
    /// Absolute device position of the current render target's pixel (0, 0):
    /// lets large blurs anchor their reduced-resolution grid to absolute
    /// pixels, so tiles and full renders agree exactly.
    static GRID_ORIGIN: Cell<(i32, i32)> = const { Cell::new((0, 0)) };
}

/// Sets the grid origin for renders on this thread from the root transform
/// (page space to target pixels).
pub(crate) fn set_grid_origin(ts: &sk::Transform) {
    let o = (-ts.tx.round(), -ts.ty.round());
    let clamp = |v: f32| if v.is_finite() { v.clamp(-1.0e9, 1.0e9) as i32 } else { 0 };
    GRID_ORIGIN.with(|g| g.set((clamp(o.0), clamp(o.1))));
}

/// Absolute grid position of the target pixel `(x, y)`.
pub(crate) fn grid_pos(x: i32, y: i32) -> (i32, i32) {
    let (gx, gy) = GRID_ORIGIN.with(|g| g.get());
    (gx.wrapping_add(x), gy.wrapping_add(y))
}

/// Device-space bounding box (x0, y0, x1, y1).
pub type BBox = (f32, f32, f32, f32);

/// Integer region around `bbox` grown by `margin`, limited to the target
/// expanded by `margin`.
pub(crate) fn region(bbox: BBox, margin: f32, tw: u32, th: u32) -> Option<(i32, i32, u32, u32)> {
    let x0 = ((bbox.0 - margin).floor() as i32).max(-(margin.ceil() as i32));
    let y0 = ((bbox.1 - margin).floor() as i32).max(-(margin.ceil() as i32));
    let x1 = ((bbox.2 + margin).ceil() as i32).min(tw as i32 + margin.ceil() as i32);
    let y1 = ((bbox.3 + margin).ceil() as i32).min(th as i32 + margin.ceil() as i32);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let (w, h) = ((x1 - x0) as u32, (y1 - y0) as u32);
    if (w as u64) * (h as u64) > MAX_REGION_PIXELS {
        return None;
    }
    Some((x0, y0, w, h))
}

fn colorize(pm: &mut sk::Pixmap, color: sk::Color) {
    let mut p = sk::Paint { blend_mode: sk::BlendMode::SourceIn, ..Default::default() };
    p.set_color(color);
    if let Some(r) = sk::Rect::from_xywh(0.0, 0.0, pm.width() as f32, pm.height() as f32) {
        pm.fill_rect(r, &p, sk::Transform::identity(), None);
    }
}

pub(crate) fn composite(target: &mut sk::Pixmap, x: i32, y: i32, pm: &sk::Pixmap, blend: sk::BlendMode, clip: Option<&sk::Mask>) {
    let pp = sk::PixmapPaint { blend_mode: blend, ..Default::default() };
    target.draw_pixmap(x, y, pm.as_ref(), &pp, sk::Transform::identity(), clip);
}

#[allow(clippy::too_many_arguments)]
pub fn drop_shadow(
    target: &mut sk::Pixmap,
    clip: Option<&sk::Mask>,
    bbox: BBox,
    scale: f32,
    color: &d::Color,
    offset: d::Vec2,
    radius: f32,
    spread: f32,
    show_behind: bool,
    alpha: f32,
    sil: Sil,
) {
    drop_shadow_with(
        &|pm, s, o| gaussian_blur_at(pm, s, o, Edges::Margined),
        target,
        clip,
        bbox,
        scale,
        color,
        offset,
        radius,
        spread,
        show_behind,
        alpha,
        sil,
    );
}

/// Blur function: `(pixmap, sigma, absolute origin of pixel (0, 0))`.
pub(crate) type BlurFn<'a> = &'a dyn Fn(&mut sk::Pixmap, f32, (i32, i32));

/// [`drop_shadow`] with an explicit blur (tests compare against the reference).
#[allow(clippy::too_many_arguments)]
pub(crate) fn drop_shadow_with(
    blur: BlurFn,
    target: &mut sk::Pixmap,
    clip: Option<&sk::Mask>,
    bbox: BBox,
    scale: f32,
    color: &d::Color,
    offset: d::Vec2,
    radius: f32,
    spread: f32,
    show_behind: bool,
    alpha: f32,
    sil: Sil,
) {
    let sigma = radius.max(0.0) * scale / 2.0;
    let (ox, oy) = (offset.x * scale, offset.y * scale);
    let margin = 3.0 * sigma + ox.abs() + oy.abs() + (spread * scale).max(0.0) + 2.0;
    let Some((rx, ry, rw, rh)) = region(bbox, margin, target.width(), target.height()) else { return };
    let Some(mut pm) = sk::Pixmap::new(rw, rh) else { return };
    sil(&mut pm, -(rx as f32) + ox, -(ry as f32) + oy, spread, d::SilhouetteMode::Alpha);
    blur(&mut pm, sigma, grid_pos(rx, ry));
    colorize(&mut pm, sk_color(color, alpha));
    if !show_behind {
        if let Some(mut knock) = sk::Pixmap::new(rw, rh) {
            sil(&mut knock, -(rx as f32), -(ry as f32), 0.0, d::SilhouetteMode::Knockout);
            composite(&mut pm, 0, 0, &knock, sk::BlendMode::DestinationOut, None);
        }
    }
    composite(target, rx, ry, &pm, sk::BlendMode::SourceOver, clip);
}

#[allow(clippy::too_many_arguments)]
pub fn inner_shadow(
    target: &mut sk::Pixmap,
    clip: Option<&sk::Mask>,
    bbox: BBox,
    scale: f32,
    color: &d::Color,
    offset: d::Vec2,
    radius: f32,
    spread: f32,
    alpha: f32,
    sil: Sil,
) {
    let sigma = radius.max(0.0) * scale / 2.0;
    let (ox, oy) = (offset.x * scale, offset.y * scale);
    let margin = 3.0 * sigma + ox.abs() + oy.abs() + (spread * scale).abs() + 2.0;
    // only the node itself matters: don't extend the region past the target
    let Some((rx, ry, rw, rh)) = region(bbox, margin, target.width(), target.height()) else { return };
    let Some(mut pm) = sk::Pixmap::new(rw, rh) else { return };
    pm.fill(sk::Color::BLACK);
    if let Some(mut hole) = sk::Pixmap::new(rw, rh) {
        sil(&mut hole, -(rx as f32) + ox, -(ry as f32) + oy, -spread, d::SilhouetteMode::Alpha);
        composite(&mut pm, 0, 0, &hole, sk::BlendMode::DestinationOut, None);
    }
    gaussian_blur_at(&mut pm, sigma, grid_pos(rx, ry), Edges::Margined);
    colorize(&mut pm, sk_color(color, alpha));
    if let Some(mut shape) = sk::Pixmap::new(rw, rh) {
        sil(&mut shape, -(rx as f32), -(ry as f32), 0.0, d::SilhouetteMode::Alpha);
        composite(&mut pm, 0, 0, &shape, sk::BlendMode::DestinationIn, None);
    }
    composite(target, rx, ry, &pm, sk::BlendMode::SourceOver, clip);
}

/// Replaces the backdrop inside the node shape with a blurred copy of it. When
/// drawing into an isolated layer, `backdrop` is what lies beneath the layer
/// (it is sampled together with the layer's own content).
#[allow(clippy::too_many_arguments)]
pub fn background_blur(target: &mut sk::Pixmap, backdrop: Option<&sk::Pixmap>, clip: Option<&sk::Mask>, bbox: BBox, scale: f32, radius: f32, sil: Sil) {
    let sigma = radius.max(0.0) * scale / 2.0;
    let (tw, th) = (target.width() as i32, target.height() as i32);
    let x0 = (bbox.0.floor() as i32).max(0);
    let y0 = (bbox.1.floor() as i32).max(0);
    let x1 = (bbox.2.ceil() as i32).min(tw);
    let y1 = (bbox.3.ceil() as i32).min(th);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let pad = (3.0 * sigma).ceil() as i32 + 1;
    let (px0, py0) = ((x0 - pad).max(0), (y0 - pad).max(0));
    let (px1, py1) = ((x1 + pad).min(tw), (y1 + pad).min(th));
    let Some(prect) = sk::IntRect::from_xywh(px0, py0, (px1 - px0) as u32, (py1 - py0) as u32) else { return };
    let Some(mut sub) = backdrop.unwrap_or(target).clone_rect(prect) else { return };
    if backdrop.is_some() {
        if let Some(own) = target.clone_rect(prect) {
            composite(&mut sub, 0, 0, &own, sk::BlendMode::SourceOver, None);
        }
    }
    gaussian_blur_at(&mut sub, sigma, grid_pos(px0, py0), Edges::Exact);
    let Some(crop) = sk::IntRect::from_xywh(x0 - px0, y0 - py0, (x1 - x0) as u32, (y1 - y0) as u32) else { return };
    let Some(mut blurred) = sub.clone_rect(crop) else { return };
    let Some(mut mask) = sk::Pixmap::new(blurred.width(), blurred.height()) else { return };
    sil(&mut mask, -(x0 as f32), -(y0 as f32), 0.0, d::SilhouetteMode::Opaque);
    composite(&mut blurred, 0, 0, &mask, sk::BlendMode::DestinationIn, None);
    composite(target, x0, y0, &mask, sk::BlendMode::DestinationOut, clip);
    composite(target, x0, y0, &blurred, sk::BlendMode::SourceOver, clip);
}

/// Blurs the rectangle `rect` of `pm` in place (device pixels).
pub fn blur_rect(pm: &mut sk::Pixmap, bbox: BBox, sigma: f32) {
    let pad = (3.0 * sigma).ceil();
    let x0 = ((bbox.0 - pad).floor() as i32).max(0);
    let y0 = ((bbox.1 - pad).floor() as i32).max(0);
    let x1 = ((bbox.2 + pad).ceil() as i32).min(pm.width() as i32);
    let y1 = ((bbox.3 + pad).ceil() as i32).min(pm.height() as i32);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let Some(r) = sk::IntRect::from_xywh(x0, y0, (x1 - x0) as u32, (y1 - y0) as u32) else { return };
    let Some(mut sub) = pm.clone_rect(r) else { return };
    gaussian_blur_at(&mut sub, sigma, grid_pos(x0, y0), Edges::Exact);
    let pp = sk::PixmapPaint { blend_mode: sk::BlendMode::Source, ..Default::default() };
    pm.draw_pixmap(x0, y0, sub.as_ref(), &pp, sk::Transform::identity(), None);
}
