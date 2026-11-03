//! Which pixels of an image fill an image job reads, and how the result sits on the layer.
//!
//! Every window comes from [`designos_doc::image_uv_map`], the node-to-image mapping the CPU
//! and GPU renderers draw with, so a job edits what the canvas shows.

use designos_doc::{image_quarter_turns, image_uv_map, ImageParams, ImageScaleMode, ImageUvMap, Vec2};
use image::RgbaImage;

/// Slack that keeps float noise from adding or dropping a whole pixel at a window edge.
const PIXEL_EPS: f32 = 1e-3;
/// A crop window whose edges sit within this many pixels of the bitmap's edges is a plain stretch.
const PLAIN_WINDOW_SLACK_PX: f32 = 1.0;
const IDENTITY: [f32; 6] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];

/// The bitmap an image job sends and the paint parameters that put it back on the layer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageJobFrame {
    /// Clockwise quarter turns applied to the decoded picture before the window is cut.
    pub quarter_turns: u8,
    /// `(x, y, w, h)` of the window in the turned picture, in pixels.
    pub window: (u32, u32, u32, u32),
    /// Paint parameters of a layer that shows the cut bitmap the way the original fill showed the picture:
    /// a baked quarter turn is gone from `rotation`, a re-based crop keeps its framing in `transform`,
    /// and everything else is carried over.
    pub params: ImageParams,
}

impl ImageJobFrame {
    /// Size in pixels of the bitmap sent to the agent.
    pub fn dims(&self) -> (u32, u32) {
        (self.window.2, self.window.3)
    }
}

/// Plans the bitmap for an image fill of `raw_w` x `raw_h` pixels shown on a layer of `layer_size`.
/// `None` when the canvas shows no part of the picture (a singular crop transform, or a window that
/// lies wholly outside the picture).
pub fn plan_image_job_frame(raw_w: u32, raw_h: u32, layer_size: Vec2, scale_mode: ImageScaleMode, params: &ImageParams) -> Option<ImageJobFrame> {
    let raw_w = raw_w.max(1);
    let raw_h = raw_h.max(1);
    let size = Vec2::new(layer_size.x.max(1.0), layer_size.y.max(1.0));
    match scale_mode {
        ImageScaleMode::Fill | ImageScaleMode::Fit => {
            let quarter_turns = image_quarter_turns(params.rotation);
            let (turned_w, turned_h) = if quarter_turns % 2 == 1 { (raw_h, raw_w) } else { (raw_w, raw_h) };
            let window = if scale_mode == ImageScaleMode::Fill { cover_window(turned_w, turned_h, size) } else { (0, 0, turned_w, turned_h) };
            Some(ImageJobFrame { quarter_turns, window, params: ImageParams { rotation: 0.0, ..*params } })
        }
        ImageScaleMode::Tile => Some(ImageJobFrame { quarter_turns: 0, window: (0, 0, raw_w, raw_h), params: *params }),
        ImageScaleMode::Crop => crop_frame(raw_w, raw_h, size, params),
    }
}

/// The centred window `Fill` shows: the picture scaled to cover the layer, cut to the layer's aspect ratio.
fn cover_window(w: u32, h: u32, layer: Vec2) -> (u32, u32, u32, u32) {
    let (iw, ih) = (w as f32, h as f32);
    let s = (layer.x / iw).max(layer.y / ih).max(1e-6);
    let crop_w = (layer.x / s).round().clamp(1.0, iw) as u32;
    let crop_h = (layer.y / s).round().clamp(1.0, ih) as u32;
    (w.saturating_sub(crop_w) / 2, h.saturating_sub(crop_h) / 2, crop_w, crop_h)
}

/// The bounding box in pixels of the part of the picture the crop transform shows, with the transform
/// re-based onto that window so the result layer frames its bitmap exactly as the layer framed the picture.
fn crop_frame(raw_w: u32, raw_h: u32, size: Vec2, params: &ImageParams) -> Option<ImageJobFrame> {
    let (iw, ih) = (raw_w as f32, raw_h as f32);
    let map = image_uv_map(size, iw, ih, ImageScaleMode::Crop, params)?;
    let corners = [map.apply(0.0, 0.0), map.apply(size.x, 0.0), map.apply(0.0, size.y), map.apply(size.x, size.y)];
    let span = |pick: fn(&(f32, f32)) -> f32, dim: f32| {
        let lo = corners.iter().map(pick).fold(f32::INFINITY, f32::min);
        let hi = corners.iter().map(pick).fold(f32::NEG_INFINITY, f32::max);
        let start = ((lo.max(0.0) * dim) + PIXEL_EPS).floor().clamp(0.0, dim);
        let end = ((hi.min(1.0) * dim) - PIXEL_EPS).ceil().clamp(0.0, dim);
        (end > start).then_some((start, end))
    };
    let (x0, x1) = span(|c| c.0, iw)?;
    let (y0, y1) = span(|c| c.1, ih)?;
    let (bw, bh) = (x1 - x0, y1 - y0);

    let m = params.transform.unwrap_or(IDENTITY);
    let rebased = [m[0] * iw / bw, m[1] * iw / bw, (m[2] * iw - x0) / bw, m[3] * ih / bh, m[4] * ih / bh, (m[5] * ih - y0) / bh];
    let slack_x = PLAIN_WINDOW_SLACK_PX / bw + 1e-4;
    let slack_y = PLAIN_WINDOW_SLACK_PX / bh + 1e-4;
    let plain = rebased[1].abs() <= 1e-6
        && rebased[3].abs() <= 1e-6
        && rebased[2].abs() <= slack_x
        && (rebased[2] + rebased[0] - 1.0).abs() <= slack_x
        && rebased[5].abs() <= slack_y
        && (rebased[5] + rebased[4] - 1.0).abs() <= slack_y;
    let transform = if plain { None } else { Some(rebased) };
    Some(ImageJobFrame { quarter_turns: 0, window: (x0 as u32, y0 as u32, bw as u32, bh as u32), params: ImageParams { transform, ..*params } })
}

/// Cuts the bitmap `frame` describes out of the decoded picture.
pub fn cut_job_bitmap(decoded: &RgbaImage, frame: &ImageJobFrame) -> RgbaImage {
    let (x, y, w, h) = frame.window;
    let cut = |src: &RgbaImage| {
        if (x, y, w, h) == (0, 0, src.width(), src.height()) {
            src.clone()
        } else {
            image::imageops::crop_imm(src, x, y, w, h).to_image()
        }
    };
    match frame.quarter_turns {
        1 => cut(&image::imageops::rotate90(decoded)),
        2 => cut(&image::imageops::rotate180(decoded)),
        3 => cut(&image::imageops::rotate270(decoded)),
        _ => cut(decoded),
    }
}

/// How the frame's bitmap maps onto the layer, as the renderer draws the result layer.
pub fn frame_uv_map(layer_size: Vec2, scale_mode: ImageScaleMode, frame: &ImageJobFrame) -> Option<ImageUvMap> {
    let (bw, bh) = frame.dims();
    let size = Vec2::new(layer_size.x.max(1.0), layer_size.y.max(1.0));
    image_uv_map(size, bw as f32, bh as f32, scale_mode, &frame.params)
}

/// The pixels `[x, y, w, h]` of a `dims` bitmap that the layer-local rectangle `(p0, p1)` covers.
pub fn local_rect_to_bitmap_region(map: &ImageUvMap, dims: (u32, u32), p0: Vec2, p1: Vec2) -> Option<[u32; 4]> {
    let (bw, bh) = (dims.0 as f32, dims.1 as f32);
    let corners = [map.apply(p0.x, p0.y), map.apply(p1.x, p0.y), map.apply(p0.x, p1.y), map.apply(p1.x, p1.y)];
    let extent = |pick: fn(&(f32, f32)) -> f32, dim: f32| {
        let lo = (corners.iter().map(pick).fold(f32::INFINITY, f32::min) * dim).clamp(0.0, dim);
        let hi = (corners.iter().map(pick).fold(f32::NEG_INFINITY, f32::max) * dim).clamp(0.0, dim);
        ((hi - lo) >= 1.0).then_some((lo, hi))
    };
    let (min_x, max_x) = extent(|c| c.0, bw)?;
    let (min_y, max_y) = extent(|c| c.1, bh)?;
    let rx = ((min_x + PIXEL_EPS).floor() as u32).min(dims.0.saturating_sub(1));
    let ry = ((min_y + PIXEL_EPS).floor() as u32).min(dims.1.saturating_sub(1));
    let rw = (((max_x - min_x) - PIXEL_EPS).ceil().max(1.0) as u32).min(dims.0.saturating_sub(rx));
    let rh = (((max_y - min_y) - PIXEL_EPS).ceil().max(1.0) as u32).min(dims.1.saturating_sub(ry));
    Some([rx, ry, rw, rh])
}

/// The layer-local rectangle `(x, y, w, h)` that the bitmap pixels `region` cover, or `None` when the mapping is singular.
pub fn bitmap_region_to_local_rect(map: &ImageUvMap, dims: (u32, u32), region: [u32; 4]) -> Option<(f32, f32, f32, f32)> {
    let det = map.u.0 * map.v.1 - map.u.1 * map.v.0;
    if !det.is_finite() || det.abs() <= 1e-12 {
        return None;
    }
    let [rx, ry, rw, rh] = region;
    let (bw, bh) = (dims.0.max(1) as f32, dims.1.max(1) as f32);
    let to_local = |px: u32, py: u32| {
        let (du, dv) = (px as f32 / bw - map.u.2, py as f32 / bh - map.v.2);
        ((du * map.v.1 - dv * map.u.1) / det, (dv * map.u.0 - du * map.v.0) / det)
    };
    let corners = [to_local(rx, ry), to_local(rx + rw, ry), to_local(rx, ry + rh), to_local(rx + rw, ry + rh)];
    let lo = |pick: fn(&(f32, f32)) -> f32| corners.iter().map(pick).fold(f32::INFINITY, f32::min);
    let hi = |pick: fn(&(f32, f32)) -> f32| corners.iter().map(pick).fold(f32::NEG_INFINITY, f32::max);
    let (x0, y0) = (lo(|c| c.0), lo(|c| c.1));
    Some((x0, y0, hi(|c| c.0) - x0, hi(|c| c.1) - y0))
}

/// Why Expand cannot run on this fill, when it cannot. Growing the picture outward only has a meaning
/// when the bitmap sits on the layer as a plain stretch, so a crop that turns, flips or leaves an empty margin is refused.
pub fn expand_refusal(scale_mode: ImageScaleMode, frame: &ImageJobFrame) -> Option<&'static str> {
    (scale_mode == ImageScaleMode::Crop && frame.params.transform.is_some()).then_some("Expand needs a crop with no turn, flip or empty margin")
}
