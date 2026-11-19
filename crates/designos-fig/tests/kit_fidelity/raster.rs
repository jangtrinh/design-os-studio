//! Coverage rasters of outlines and the dilation test of Hausdorff distance between two of them.

use designos_doc::{Path, PathCmd};
use designos_render::tiny_skia as sk;

pub fn to_sk(p: &Path) -> Option<sk::Path> {
    let mut b = sk::PathBuilder::new();
    for c in &p.cmds {
        match *c {
            PathCmd::MoveTo(v) => b.move_to(v.x, v.y),
            PathCmd::LineTo(v) => b.line_to(v.x, v.y),
            PathCmd::QuadTo(c1, v) => b.quad_to(c1.x, c1.y, v.x, v.y),
            PathCmd::CubicTo(c1, c2, v) => b.cubic_to(c1.x, c1.y, c2.x, c2.y, v.x, v.y),
            PathCmd::Close => b.close(),
        }
    }
    b.finish()
}

/// Stored outlines as `(path, even-odd)`.
pub fn figma_paths(geo: &[(usize, bool)], blobs: &[Vec<u8>]) -> Vec<(sk::Path, bool)> {
    geo.iter().filter_map(|&(i, odd)| Some((to_sk(&designos_fig::commands_blob_to_path(blobs.get(i)?, odd))?, odd))).collect()
}

/// Per-pixel coverage (0..=255) of `paths` filled with their own winding rules.
pub fn coverage(paths: &[(sk::Path, bool)], w: u32, h: u32, ts: sk::Transform) -> Option<Vec<u8>> {
    let mut pm = sk::Pixmap::new(w, h)?;
    let mut paint = sk::Paint::default();
    paint.set_color_rgba8(0, 0, 0, 255);
    paint.anti_alias = true;
    for (p, odd) in paths {
        pm.fill_path(p, &paint, if *odd { sk::FillRule::EvenOdd } else { sk::FillRule::Winding }, ts, None);
    }
    Some(alpha(&pm))
}

pub fn alpha(pm: &sk::Pixmap) -> Vec<u8> {
    pm.pixels().iter().map(|p| p.alpha()).collect()
}

/// Covered pixels: at least half coverage.
pub fn covered(a: &[u8]) -> Vec<bool> {
    a.iter().map(|x| *x >= 128).collect()
}

/// Whether every covered pixel of `a` has a covered pixel of `b` within `r` pixels.
pub fn within(a: &[bool], b: &[bool], w: usize, h: usize, r: i32) -> bool {
    let offs: Vec<(i32, i32)> = (-r..=r).flat_map(|dy| (-r..=r).map(move |dx| (dx, dy))).filter(|(dx, dy)| dx * dx + dy * dy <= r * r).collect();
    for y in 0..h {
        for x in 0..w {
            if !a[y * w + x] || b[y * w + x] {
                continue;
            }
            let hit = offs.iter().any(|&(dx, dy)| {
                let (xx, yy) = (x as i32 + dx, y as i32 + dy);
                xx >= 0 && yy >= 0 && (xx as usize) < w && (yy as usize) < h && b[yy as usize * w + xx as usize]
            });
            if !hit {
                return false;
            }
        }
    }
    true
}

/// Whether any covered pixel lies on the border of a `w` x `h` raster: the outline may run past the window.
pub fn touches_border(m: &[bool], w: usize, h: usize) -> bool {
    (0..w).any(|x| m[x] || m[(h - 1) * w + x]) || (0..h).any(|y| m[y * w] || m[y * w + w - 1])
}

/// Soft IoU of two coverage rasters: per pixel, the smaller coverage over the larger.
pub fn soft_iou(a: &[u8], b: &[u8]) -> f32 {
    let (mut lo, mut hi) = (0u64, 0u64);
    for (x, y) in a.iter().zip(b) {
        lo += u64::from(*x.min(y));
        hi += u64::from(*x.max(y));
    }
    if hi == 0 {
        1.0
    } else {
        lo as f32 / hi as f32
    }
}
