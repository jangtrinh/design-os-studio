//! Before / after / diff region rasteriser (`E15-8` / `#593`).

use crate::{checked_pixmap_size, render_page, Viewport};
use designos_doc::{Document, NodeId, Rect, Vec2};
use tiny_skia::{IntSize, Pixmap};

/// Rasterised before, after, and highlighted diff PNGs plus pixel-change metrics for one page region.
#[derive(Clone, Debug)]
pub struct DiffImageResult {
    pub page_id: NodeId,
    pub page_name: String,
    pub bbox: Rect,
    pub changed_bbox: Option<Rect>,
    pub scale_used: f32,
    pub width: u32,
    pub height: u32,
    pub changed_pixels: u64,
    pub total_pixels: u64,
    pub changed_ratio: f64,
    pub before_png: Vec<u8>,
    pub after_png: Vec<u8>,
    pub diff_png: Vec<u8>,
}

fn compute_diff_scale(bounds: Rect, scale: f32, max_long_edge: u32) -> f32 {
    let long_edge = bounds.w.max(bounds.h);
    if max_long_edge > 0 && long_edge > 0.0 && long_edge * scale > max_long_edge as f32 {
        let mut scale_used = (max_long_edge as f32) / long_edge;
        while (long_edge * scale_used).ceil() > max_long_edge as f32 && scale_used > 0.0 {
            scale_used = f32::from_bits(scale_used.to_bits() - 1);
        }
        scale_used
    } else {
        scale
    }
}

/// Renders `region` on `page_id` in `before_doc` and `after_doc` (capping the long edge at 1024 px)
/// and produces `before_png`, `after_png`, and `diff_png` along with changed-pixel metrics.
pub fn render_diff_image(before_doc: &Document, after_doc: &Document, page_id: NodeId, region: Rect, scale: f32) -> Option<DiffImageResult> {
    if !region.x.is_finite() || !region.y.is_finite() || !region.w.is_finite() || !region.h.is_finite() || region.w <= 0.0 || region.h <= 0.0 {
        return None;
    }
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let scale_used = compute_diff_scale(region, scale, 1024);
    if scale_used < 0.01 {
        return None;
    }
    let (width, height) = checked_pixmap_size(region.w * scale_used, region.h * scale_used)?;
    let mut before_pm = Pixmap::new(width, height)?;
    let mut after_pm = Pixmap::new(width, height)?;
    let vp = Viewport { offset: Vec2::new(region.x, region.y), zoom: scale_used, dpr: 1.0 };
    if before_doc.get(page_id).is_some() {
        render_page(before_doc, page_id, &vp, &mut before_pm);
    }
    if after_doc.get(page_id).is_some() {
        render_page(after_doc, page_id, &vp, &mut after_pm);
    }

    let total_pixels = (width as u64) * (height as u64);
    let mut changed_pixels = 0u64;
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (u32::MAX, u32::MAX, 0u32, 0u32);
    let mut diff_buf = vec![0u8; (width as usize) * (height as usize) * 4];

    let b_data = before_pm.data();
    let a_data = after_pm.data();
    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) as usize) * 4;
            let bp = &b_data[idx..idx + 4];
            let ap = &a_data[idx..idx + 4];
            if bp != ap {
                changed_pixels += 1;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
                diff_buf[idx..idx + 4].copy_from_slice(&[255, 45, 85, 255]);
            } else {
                // Dimmed after pixel blended 30% onto #18181B dark background
                let r = ((ap[0] as u32 * 30 + 0x18 * 70) / 100) as u8;
                let g = ((ap[1] as u32 * 30 + 0x18 * 70) / 100) as u8;
                let b = ((ap[2] as u32 * 30 + 0x1B * 70) / 100) as u8;
                diff_buf[idx..idx + 4].copy_from_slice(&[r, g, b, 255]);
            }
        }
    }

    let changed_bbox = if changed_pixels > 0 {
        let px_w = (max_x - min_x + 1) as f32;
        let px_h = (max_y - min_y + 1) as f32;
        Some(Rect::new(region.x + (min_x as f32) / scale_used, region.y + (min_y as f32) / scale_used, px_w / scale_used, px_h / scale_used))
    } else {
        None
    };

    let changed_ratio = if total_pixels == 0 { 0.0 } else { changed_pixels as f64 / total_pixels as f64 };
    let diff_pm = Pixmap::from_vec(diff_buf, IntSize::from_wh(width, height)?)?;
    let before_png = before_pm.encode_png().ok()?;
    let after_png = after_pm.encode_png().ok()?;
    let diff_png = diff_pm.encode_png().ok()?;
    let page_name = after_doc.get(page_id).or_else(|| before_doc.get(page_id)).map(|n| n.name.clone()).unwrap_or_else(|| format!("Page {}", page_id.0));

    Some(DiffImageResult {
        page_id,
        page_name,
        bbox: region,
        changed_bbox,
        scale_used,
        width,
        height,
        changed_pixels,
        total_pixels,
        changed_ratio,
        before_png,
        after_png,
        diff_png,
    })
}
