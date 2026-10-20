//! Image comparison used to enforce pixel fidelity.
//!
//! Core pieces: [`compare`] (tolerance + anti-aliasing aware pixel diff, SSIM),
//! [`diff_image`] (visualisation), [`crop`] and [`downscale`] helpers.
//! The [`suite`] module runs manifests of reference/actual pairs.

pub mod suite;

use image::{Rgba, RgbaImage};
use serde::{Deserialize, Serialize};

/// Axis-aligned pixel rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn new(x: u32, y: u32, w: u32, h: u32) -> Self {
        Rect { x, y, w, h }
    }

    pub fn contains(&self, px: u32, py: u32) -> bool {
        px >= self.x && py >= self.y && px - self.x < self.w && py - self.y < self.h
    }

    /// Parse `x,y,w,h`.
    pub fn parse(s: &str) -> Result<Rect, String> {
        let parts: Vec<&str> = s.split(',').map(str::trim).collect();
        if parts.len() != 4 {
            return Err(format!("expected x,y,w,h but got `{s}`"));
        }
        let mut v = [0u32; 4];
        for (i, p) in parts.iter().enumerate() {
            v[i] = p.parse().map_err(|_| format!("`{p}` is not a non-negative integer"))?;
        }
        Ok(Rect::new(v[0], v[1], v[2], v[3]))
    }
}

#[derive(Debug, Clone)]
pub struct DiffOptions {
    /// Max per-channel delta (0..=255) still considered equal.
    pub tolerance: u8,
    /// Do not count pixels that look anti-aliased in either image (pixelmatch style).
    pub detect_aa: bool,
    /// Regions excluded from the comparison.
    pub ignore: Vec<Rect>,
}

impl Default for DiffOptions {
    fn default() -> Self {
        DiffOptions { tolerance: 0, detect_aa: true, ignore: Vec::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiffReport {
    /// Compared canvas size (the max of both images in each dimension).
    pub width: u32,
    pub height: u32,
    pub size_a: (u32, u32),
    pub size_b: (u32, u32),
    pub size_mismatch: bool,
    /// Pixels whose max channel delta exceeds the tolerance (and are not AA),
    /// including every pixel outside the overlapping region on a size mismatch.
    pub mismatched: u64,
    /// Differing pixels forgiven because they are anti-aliasing.
    pub aa_pixels: u64,
    /// Pixels excluded through ignore regions.
    pub ignored: u64,
    /// `mismatched / (width*height - ignored)`.
    pub ratio: f64,
    /// Grayscale SSIM over the overlapping region, scaled by overlap/canvas area.
    pub ssim: f64,
    /// Tight pixel-space bounding box over all mismatched pixels (`None` when `mismatched == 0`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changed_bbox: Option<Rect>,
}

const CLASS_SAME: u8 = 0;
const CLASS_MISMATCH: u8 = 1;
const CLASS_AA: u8 = 2;
const CLASS_IGNORED: u8 = 3;

/// Composite onto white, dropping alpha.
fn flatten(p: &Rgba<u8>) -> [u8; 3] {
    let a = p[3] as u32;
    let mut out = [0u8; 3];
    for i in 0..3 {
        out[i] = ((p[i] as u32 * a + 255 * (255 - a) + 127) / 255) as u8;
    }
    out
}

fn flatten_image(img: &RgbaImage) -> Vec<[u8; 3]> {
    img.pixels().map(flatten).collect()
}

fn luma(p: [u8; 3]) -> f64 {
    0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64
}

struct Plane<'a> {
    px: &'a [[u8; 3]],
    w: u32,
    h: u32,
}

impl Plane<'_> {
    fn at(&self, x: u32, y: u32) -> [u8; 3] {
        self.px[(y * self.w + x) as usize]
    }

    /// True if more than two neighbours of (x,y) have the identical colour.
    fn has_many_siblings(&self, x: u32, y: u32) -> bool {
        let c = self.at(x, y);
        let mut n = 0;
        for dy in -1i64..=1 {
            for dx in -1i64..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                if nx < 0 || ny < 0 || nx >= self.w as i64 || ny >= self.h as i64 {
                    n += 1; // edges count as siblings, like pixelmatch's zero count
                    continue;
                }
                if self.at(nx as u32, ny as u32) == c {
                    n += 1;
                }
            }
        }
        n > 2
    }

    /// pixelmatch's `antialiased` heuristic: is pixel (x,y) of `self` an AA pixel,
    /// cross-checked against `other`.
    fn is_antialiased(&self, other: &Plane, x: u32, y: u32) -> bool {
        let center = luma(self.at(x, y));
        let mut zeroes = 0;
        let (mut min, mut max) = (0.0f64, 0.0f64);
        let (mut min_xy, mut max_xy) = ((0u32, 0u32), (0u32, 0u32));
        for dy in -1i64..=1 {
            for dx in -1i64..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                if nx < 0 || ny < 0 || nx >= self.w as i64 || ny >= self.h as i64 {
                    zeroes += 1;
                    continue;
                }
                let d = center - luma(self.at(nx as u32, ny as u32));
                if d == 0.0 {
                    zeroes += 1;
                    if zeroes > 2 {
                        return false;
                    }
                } else if d < min {
                    min = d;
                    min_xy = (nx as u32, ny as u32);
                } else if d > max {
                    max = d;
                    max_xy = (nx as u32, ny as u32);
                }
            }
        }
        if min == 0.0 || max == 0.0 {
            return false;
        }
        (self.has_many_siblings(min_xy.0, min_xy.1) && other.has_many_siblings(min_xy.0, min_xy.1))
            || (self.has_many_siblings(max_xy.0, max_xy.1) && other.has_many_siblings(max_xy.0, max_xy.1))
    }
}

/// Mean SSIM over 8x8 windows (stride 4) on grayscale pixels.
fn ssim_gray(a: &[f64], b: &[f64], w: usize, h: usize) -> f64 {
    if w == 0 || h == 0 {
        return 1.0;
    }
    const C1: f64 = (0.01 * 255.0) * (0.01 * 255.0);
    const C2: f64 = (0.03 * 255.0) * (0.03 * 255.0);
    let win_w = w.min(8);
    let win_h = h.min(8);
    let mut total = 0.0;
    let mut count = 0u64;
    let positions = |len: usize, win: usize| -> Vec<usize> {
        let mut v: Vec<usize> = (0..=len - win).step_by(4).collect();
        if *v.last().unwrap() != len - win {
            v.push(len - win);
        }
        v
    };
    let xs = positions(w, win_w);
    let ys = positions(h, win_h);
    let n = (win_w * win_h) as f64;
    for &y0 in &ys {
        for &x0 in &xs {
            let (mut sa, mut sb, mut saa, mut sbb, mut sab) = (0.0, 0.0, 0.0, 0.0, 0.0);
            for y in y0..y0 + win_h {
                let row = y * w;
                for x in x0..x0 + win_w {
                    let (va, vb) = (a[row + x], b[row + x]);
                    sa += va;
                    sb += vb;
                    saa += va * va;
                    sbb += vb * vb;
                    sab += va * vb;
                }
            }
            let (ma, mb) = (sa / n, sb / n);
            let (va, vb, cov) = (saa / n - ma * ma, sbb / n - mb * mb, sab / n - ma * mb);
            total += ((2.0 * ma * mb + C1) * (2.0 * cov + C2)) / ((ma * ma + mb * mb + C1) * (va + vb + C2));
            count += 1;
        }
    }
    total / count as f64
}

/// Returns the report plus a per-pixel class map over the compared canvas.
fn analyze(a: &RgbaImage, b: &RgbaImage, opts: &DiffOptions) -> (DiffReport, Vec<u8>) {
    let (aw, ah) = a.dimensions();
    let (bw, bh) = b.dimensions();
    let (w, h) = (aw.max(bw), ah.max(bh));
    let (ow, oh) = (aw.min(bw), ah.min(bh));
    let fa = flatten_image(a);
    let fb = flatten_image(b);
    let pa = Plane { px: &fa, w: aw, h: ah };
    let pb = Plane { px: &fb, w: bw, h: bh };

    let mut classes = vec![CLASS_SAME; (w as usize) * (h as usize)];
    let (mut mismatched, mut aa_pixels, mut ignored) = (0u64, 0u64, 0u64);
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (u32::MAX, u32::MAX, 0u32, 0u32);
    let mut record_mismatch = |x: u32, y: u32| {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    };
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            if opts.ignore.iter().any(|r| r.contains(x, y)) {
                classes[idx] = CLASS_IGNORED;
                ignored += 1;
                continue;
            }
            if x >= ow || y >= oh {
                classes[idx] = CLASS_MISMATCH;
                mismatched += 1;
                record_mismatch(x, y);
                continue;
            }
            let (ca, cb) = (pa.at(x, y), pb.at(x, y));
            let delta = (0..3).map(|i| ca[i].abs_diff(cb[i])).max().unwrap();
            if delta <= opts.tolerance {
                continue;
            }
            if opts.detect_aa && (pa.is_antialiased(&pb, x, y) || pb.is_antialiased(&pa, x, y)) {
                classes[idx] = CLASS_AA;
                aa_pixels += 1;
            } else {
                classes[idx] = CLASS_MISMATCH;
                mismatched += 1;
                record_mismatch(x, y);
            }
        }
    }

    // SSIM on the overlap, ignored pixels zeroed in both.
    let gray = |p: &Plane, ow: u32, oh: u32| -> Vec<f64> {
        let mut v = Vec::with_capacity((ow * oh) as usize);
        for y in 0..oh {
            for x in 0..ow {
                v.push(if opts.ignore.iter().any(|r| r.contains(x, y)) { 0.0 } else { luma(p.at(x, y)) });
            }
        }
        v
    };
    let ssim = if ow == 0 || oh == 0 {
        if w == 0 || h == 0 {
            1.0
        } else {
            0.0
        }
    } else {
        let s = ssim_gray(&gray(&pa, ow, oh), &gray(&pb, ow, oh), ow as usize, oh as usize);
        s * (ow as f64 * oh as f64) / (w as f64 * h as f64)
    };

    let considered = (w as u64 * h as u64).saturating_sub(ignored);
    let ratio = if considered == 0 { 0.0 } else { mismatched as f64 / considered as f64 };
    let changed_bbox = if mismatched > 0 { Some(Rect::new(min_x, min_y, max_x - min_x + 1, max_y - min_y + 1)) } else { None };
    let report = DiffReport {
        width: w,
        height: h,
        size_a: (aw, ah),
        size_b: (bw, bh),
        size_mismatch: (aw, ah) != (bw, bh),
        mismatched,
        aa_pixels,
        ignored,
        ratio,
        ssim,
        changed_bbox,
    };
    (report, classes)
}

/// Compare two images. See [`DiffReport`] for the semantics.
pub fn compare(a: &RgbaImage, b: &RgbaImage, opts: &DiffOptions) -> DiffReport {
    analyze(a, b, opts).0
}

/// Compare and also render a visualisation: faded grayscale base of `a`,
/// red for mismatched pixels, yellow for forgiven anti-aliasing, and a blue
/// tint over ignored regions.
pub fn diff_image(a: &RgbaImage, b: &RgbaImage, opts: &DiffOptions) -> (DiffReport, RgbaImage) {
    let (report, classes) = analyze(a, b, opts);
    let (w, h) = (report.width, report.height);
    let mut out = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let class = classes[(y * w + x) as usize];
            let base = if x < a.width() && y < a.height() {
                luma(flatten(a.get_pixel(x, y)))
            } else if x < b.width() && y < b.height() {
                luma(flatten(b.get_pixel(x, y)))
            } else {
                255.0
            };
            let faded = (255.0 + (base - 255.0) * 0.1).round() as u8;
            let px = match class {
                CLASS_MISMATCH => Rgba([255, 0, 0, 255]),
                CLASS_AA => Rgba([255, 221, 0, 255]),
                CLASS_IGNORED => Rgba([faded.saturating_sub(40), faded.saturating_sub(20), faded, 255]),
                _ => Rgba([faded, faded, faded, 255]),
            };
            out.put_pixel(x, y, px);
        }
    }
    (report, out)
}

/// Crop to `r`, clamped to the image bounds.
pub fn crop(img: &RgbaImage, r: Rect) -> RgbaImage {
    let x = r.x.min(img.width());
    let y = r.y.min(img.height());
    let w = r.w.min(img.width() - x);
    let h = r.h.min(img.height() - y);
    image::imageops::crop_imm(img, x, y, w, h).to_image()
}

/// Nearest-integer downscale by `factor` (e.g. 2 for a 2x DPR screenshot).
pub fn downscale(img: &RgbaImage, factor: u32) -> RgbaImage {
    if factor <= 1 {
        return img.clone();
    }
    let (w, h) = (img.width() / factor, img.height() / factor);
    RgbaImage::from_fn(w, h, |x, y| *img.get_pixel(x * factor, y * factor))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, c: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(w, h, Rgba(c))
    }

    #[test]
    fn identical_is_zero() {
        let a = solid(20, 20, [10, 20, 30, 255]);
        let r = compare(&a, &a.clone(), &DiffOptions::default());
        assert_eq!(r.mismatched, 0);
        assert_eq!(r.ratio, 0.0);
        assert!(!r.size_mismatch);
        assert!((r.ssim - 1.0).abs() < 1e-12);
    }

    #[test]
    fn one_pixel_off() {
        let a = solid(10, 10, [255, 255, 255, 255]);
        let mut b = a.clone();
        b.put_pixel(3, 4, Rgba([0, 0, 0, 255]));
        let r = compare(&a, &b, &DiffOptions::default());
        assert_eq!(r.mismatched, 1);
        assert!((r.ratio - 0.01).abs() < 1e-12);
        assert!(r.ssim < 1.0);
    }

    #[test]
    fn tolerance_and_ignore() {
        let a = solid(10, 10, [100, 100, 100, 255]);
        let mut b = a.clone();
        b.put_pixel(1, 1, Rgba([105, 100, 100, 255]));
        let loose = DiffOptions { tolerance: 5, ..Default::default() };
        assert_eq!(compare(&a, &b, &loose).mismatched, 0);
        assert_eq!(compare(&a, &b, &DiffOptions::default()).mismatched, 1);
        let ign = DiffOptions { ignore: vec![Rect::new(0, 0, 2, 2)], ..Default::default() };
        let r = compare(&a, &b, &ign);
        assert_eq!((r.mismatched, r.ignored), (0, 4));
    }

    #[test]
    fn alpha_composited_on_white() {
        let a = solid(4, 4, [0, 0, 0, 0]);
        let b = solid(4, 4, [255, 255, 255, 255]);
        assert_eq!(compare(&a, &b, &DiffOptions::default()).mismatched, 0);
    }

    #[test]
    fn aa_edge_ignored() {
        let mut a = RgbaImage::from_pixel(16, 16, Rgba([255, 255, 255, 255]));
        for y in 0..16 {
            for x in 0..8 {
                a.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        let mut b = a.clone();
        b.put_pixel(8, 8, Rgba([128, 128, 128, 255]));
        let r = compare(&a, &b, &DiffOptions::default());
        assert_eq!(r.mismatched, 0);
        assert_eq!(r.aa_pixels, 1);
        let strict = DiffOptions { detect_aa: false, ..Default::default() };
        assert_eq!(compare(&a, &b, &strict).mismatched, 1);
        // A lone off pixel in a flat area is not AA.
        let flat = solid(16, 16, [255, 255, 255, 255]);
        let mut f2 = flat.clone();
        f2.put_pixel(8, 8, Rgba([128, 128, 128, 255]));
        assert_eq!(compare(&flat, &f2, &DiffOptions::default()).mismatched, 1);
    }

    #[test]
    fn size_mismatch() {
        let a = solid(10, 10, [0, 0, 0, 255]);
        let b = solid(12, 10, [0, 0, 0, 255]);
        let r = compare(&a, &b, &DiffOptions::default());
        assert!(r.size_mismatch);
        assert_eq!((r.width, r.height), (12, 10));
        assert_eq!(r.mismatched, 20);
        assert!(r.ssim < 1.0);
    }

    #[test]
    fn diff_image_colors() {
        let a = solid(5, 5, [255, 255, 255, 255]);
        let mut b = a.clone();
        b.put_pixel(2, 2, Rgba([0, 0, 0, 255]));
        let (_, img) = diff_image(&a, &b, &DiffOptions::default());
        assert_eq!(*img.get_pixel(2, 2), Rgba([255, 0, 0, 255]));
        assert_eq!(*img.get_pixel(0, 0), Rgba([255, 255, 255, 255]));
    }

    #[test]
    fn crop_and_downscale() {
        let img = RgbaImage::from_fn(8, 8, |x, y| Rgba([x as u8, y as u8, 0, 255]));
        let c = crop(&img, Rect::new(6, 6, 10, 10));
        assert_eq!(c.dimensions(), (2, 2));
        let d = downscale(&img, 2);
        assert_eq!(d.dimensions(), (4, 4));
        assert_eq!(*d.get_pixel(1, 1), Rgba([2, 2, 0, 255]));
    }

    #[test]
    fn diff_report_bbox_unit() {
        let a = solid(10, 10, [255, 255, 255, 255]);
        let mut b = a.clone();
        b.put_pixel(2, 3, Rgba([255, 0, 0, 255]));
        b.put_pixel(7, 8, Rgba([0, 0, 255, 255]));
        let opts = DiffOptions { tolerance: 0, detect_aa: false, ignore: Vec::new() };
        let r = compare(&a, &b, &opts);
        assert_eq!(r.mismatched, 2);
        assert_eq!(r.changed_bbox, Some(Rect::new(2, 3, 6, 6)));

        let same = compare(&a, &a, &opts);
        assert_eq!(same.mismatched, 0);
        assert_eq!(same.changed_bbox, None);
    }
}
