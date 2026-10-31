//! Perceptual colour-space conversions (`sRGB` ↔ `OKLab` ↔ `OKLCH`) in `f32`.
//!
//! Uses Björn Ottosson's canonical OKLab matrices and the IEC 61966-2-1 sRGB transfer
//! function, clamping linear sRGB channels to `[0.0, 1.0]` before gamma encoding.

#[inline]
fn sanitize_unit(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[inline]
fn sanitize_finite(v: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        0.0
    }
}

#[inline]
fn srgb_channel_to_linear(v: f32) -> f32 {
    let c = sanitize_unit(v);
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[inline]
fn linear_channel_to_srgb(v: f32) -> f32 {
    let c = sanitize_unit(v);
    if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Converts straight sRGB channels (`0.0..=1.0`) to `OKLab` `[L, a, b]`.
pub fn srgb_to_oklab(r: f32, g: f32, b: f32) -> [f32; 3] {
    let r_lin = srgb_channel_to_linear(r);
    let g_lin = srgb_channel_to_linear(g);
    let b_lin = srgb_channel_to_linear(b);

    if r_lin == g_lin && g_lin == b_lin {
        return [r_lin.cbrt().clamp(0.0, 1.0), 0.0, 0.0];
    }

    let l = 0.412_221_46 * r_lin + 0.536_332_55 * g_lin + 0.051_445_995 * b_lin;
    let m = 0.211_903_5 * r_lin + 0.680_699_5 * g_lin + 0.107_396_96 * b_lin;
    let s = 0.088_302_46 * r_lin + 0.281_718_85 * g_lin + 0.629_978_7 * b_lin;

    let l_ = l.cbrt();
    let m_ = m.cbrt();
    let s_ = s.cbrt();

    let ok_l = 0.210_454_26 * l_ + 0.793_617_8 * m_ - 0.004_072_047 * s_;
    let ok_a = 1.977_998_5 * l_ - 2.428_592_2 * m_ + 0.450_593_7 * s_;
    let ok_b = 0.025_904_037 * l_ + 0.782_771_77 * m_ - 0.808_675_77 * s_;

    [sanitize_unit(ok_l), sanitize_finite(ok_a), sanitize_finite(ok_b)]
}

/// Converts `OKLab` `[L, a, b]` to straight sRGB channels (`0.0..=1.0`),
/// clamping each linear sRGB channel to `[0.0, 1.0]`.
pub fn oklab_to_srgb(l: f32, a: f32, b: f32) -> [f32; 3] {
    let l = sanitize_finite(l);
    let a = sanitize_finite(a);
    let b = sanitize_finite(b);

    let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
    let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;

    let l3 = l_ * l_ * l_;
    let m3 = m_ * m_ * m_;
    let s3 = s_ * s_ * s_;

    let r_lin = 4.076_741_7 * l3 - 3.307_711_6 * m3 + 0.230_969_94 * s3;
    let g_lin = -1.268_438 * l3 + 2.609_757_4 * m3 - 0.341_319_38 * s3;
    let b_lin = -0.004_196_086_3 * l3 - 0.703_418_6 * m3 + 1.707_614_7 * s3;

    [linear_channel_to_srgb(r_lin), linear_channel_to_srgb(g_lin), linear_channel_to_srgb(b_lin)]
}

/// Converts `OKLab` `[L, a, b]` to `OKLCH` `[L, C, H_deg]` with `H_deg` in `0.0..360.0`.
pub fn oklab_to_oklch(l: f32, a: f32, b: f32) -> [f32; 3] {
    let l = sanitize_finite(l);
    let a = sanitize_finite(a);
    let b = sanitize_finite(b);
    let c = sanitize_finite(a.hypot(b)).max(0.0);
    let h = if c < 1e-4 {
        0.0
    } else {
        let deg = sanitize_finite(b.atan2(a).to_degrees()).rem_euclid(360.0);
        if deg >= 360.0 {
            0.0
        } else {
            deg
        }
    };
    [l, c, h]
}

/// Converts `OKLCH` `[L, C, H_deg]` (`H_deg` in degrees) to `OKLab` `[L, a, b]`.
pub fn oklch_to_oklab(l: f32, c: f32, h_deg: f32) -> [f32; 3] {
    let l = sanitize_finite(l);
    let c = sanitize_finite(c).max(0.0);
    let h_deg = sanitize_finite(h_deg).rem_euclid(360.0);
    let h_rad = h_deg.to_radians();
    [l, sanitize_finite(c * h_rad.cos()), sanitize_finite(c * h_rad.sin())]
}

/// Converts straight sRGB channels (`0.0..=1.0`) to `OKLCH` `[L, C, H_deg]`.
pub fn srgb_to_oklch(r: f32, g: f32, b: f32) -> [f32; 3] {
    let [l, a, b_ok] = srgb_to_oklab(r, g, b);
    oklab_to_oklch(l, a, b_ok)
}

/// Converts `OKLCH` `[L, C, H_deg]` to straight sRGB channels (`0.0..=1.0`),
/// clamping each linear sRGB channel to `[0.0, 1.0]`.
pub fn oklch_to_srgb(l: f32, c: f32, h_deg: f32) -> [f32; 3] {
    let [l, a, b] = oklch_to_oklab(l, c, h_deg);
    oklab_to_srgb(l, a, b)
}

/// Converts 8-bit straight sRGB `[R, G, B]` to `OKLab` `[L, a, b]`.
pub fn srgb_u8_to_oklab(rgb: [u8; 3]) -> [f32; 3] {
    srgb_to_oklab(rgb[0] as f32 / 255.0, rgb[1] as f32 / 255.0, rgb[2] as f32 / 255.0)
}

/// Converts `OKLab` `[L, a, b]` to 8-bit straight sRGB `[R, G, B]`.
pub fn oklab_to_srgb_u8(l: f32, a: f32, b: f32) -> [u8; 3] {
    let [r, g, b] = oklab_to_srgb(l, a, b);
    [(r.clamp(0.0, 1.0) * 255.0).round() as u8, (g.clamp(0.0, 1.0) * 255.0).round() as u8, (b.clamp(0.0, 1.0) * 255.0).round() as u8]
}

/// Converts 8-bit straight sRGB `[R, G, B]` to `OKLCH` `[L, C, H_deg]`.
pub fn srgb_u8_to_oklch(rgb: [u8; 3]) -> [f32; 3] {
    srgb_to_oklch(rgb[0] as f32 / 255.0, rgb[1] as f32 / 255.0, rgb[2] as f32 / 255.0)
}

/// Converts `OKLCH` `[L, C, H_deg]` to 8-bit straight sRGB `[R, G, B]`.
pub fn oklch_to_srgb_u8(l: f32, c: f32, h_deg: f32) -> [u8; 3] {
    let [r, g, b] = oklch_to_srgb(l, c, h_deg);
    [(r.clamp(0.0, 1.0) * 255.0).round() as u8, (g.clamp(0.0, 1.0) * 255.0).round() as u8, (b.clamp(0.0, 1.0) * 255.0).round() as u8]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_near(actual: f32, expected: f32, tol: f32, label: &str) {
        assert!((actual - expected).abs() <= tol, "{label}: expected {expected} ± {tol}, got {actual}");
    }

    #[test]
    fn known_values_srgb_to_oklab_and_oklch() {
        let tol = 1e-3;

        // sRGB red -> OKLab (0.6280, 0.2249, 0.1258) and OKLCH (0.6280, 0.2577, 29.23 deg)
        let red_lab = srgb_to_oklab(1.0, 0.0, 0.0);
        assert_near(red_lab[0], 0.6280, tol, "red OKLab L");
        assert_near(red_lab[1], 0.2249, tol, "red OKLab a");
        assert_near(red_lab[2], 0.1258, tol, "red OKLab b");

        let red_lch = srgb_to_oklch(1.0, 0.0, 0.0);
        assert_near(red_lch[0], 0.6280, tol, "red OKLCH L");
        assert_near(red_lch[1], 0.2577, tol, "red OKLCH C");
        assert_near(red_lch[2], 29.23, 1e-2, "red OKLCH H");

        // sRGB green -> OKLab (0.8664, -0.2339, 0.1795)
        let green_lab = srgb_to_oklab(0.0, 1.0, 0.0);
        assert_near(green_lab[0], 0.8664, tol, "green OKLab L");
        assert_near(green_lab[1], -0.2339, tol, "green OKLab a");
        assert_near(green_lab[2], 0.1795, tol, "green OKLab b");

        // sRGB blue -> OKLab (0.4520, -0.0325, -0.3115)
        let blue_lab = srgb_to_oklab(0.0, 0.0, 1.0);
        assert_near(blue_lab[0], 0.4520, tol, "blue OKLab L");
        assert_near(blue_lab[1], -0.0325, tol, "blue OKLab a");
        assert_near(blue_lab[2], -0.3115, tol, "blue OKLab b");

        // sRGB white -> L 1.0, C ~ 0
        let white_lch = srgb_to_oklch(1.0, 1.0, 1.0);
        assert_near(white_lch[0], 1.0, tol, "white OKLCH L");
        assert_near(white_lch[1], 0.0, tol, "white OKLCH C");

        // sRGB black -> L 0
        let black_lch = srgb_to_oklch(0.0, 0.0, 0.0);
        assert_near(black_lch[0], 0.0, tol, "black OKLCH L");
        assert_near(black_lch[1], 0.0, tol, "black OKLCH C");
    }

    #[test]
    fn round_trip_17_step_srgb_grid_4913_colours() {
        let tol = 1.0 / 255.0;
        let mut count = 0usize;
        for ri in 0..17u8 {
            for gi in 0..17u8 {
                for bi in 0..17u8 {
                    count += 1;
                    let r = (ri * 15) as f32 / 255.0;
                    let g = (gi * 15) as f32 / 255.0;
                    let b = (bi * 15) as f32 / 255.0;

                    let [l, a, b_ok] = srgb_to_oklab(r, g, b);
                    let [r_lab, g_lab, b_lab] = oklab_to_srgb(l, a, b_ok);
                    assert!(
                        (r_lab - r).abs() <= tol && (g_lab - g).abs() <= tol && (b_lab - b).abs() <= tol,
                        "OKLab round-trip failed for ({ri}, {gi}, {bi}): got ({r_lab}, {g_lab}, {b_lab}) vs ({r}, {g}, {b})"
                    );

                    let [l2, c, h] = srgb_to_oklch(r, g, b);
                    let [r_lch, g_lch, b_lch] = oklch_to_srgb(l2, c, h);
                    assert!(
                        (r_lch - r).abs() <= tol && (g_lch - g).abs() <= tol && (b_lch - b).abs() <= tol,
                        "OKLCH round-trip failed for ({ri}, {gi}, {bi}): got ({r_lch}, {g_lch}, {b_lch}) vs ({r}, {g}, {b})"
                    );
                }
            }
        }
        assert_eq!(count, 4_913);
    }

    #[test]
    fn gamut_clamping_and_non_finite_inputs() {
        // oklch(0.7 0.4 150) is outside the sRGB gamut; must clamp each channel to [0, 1] without panic.
        let rgb = oklch_to_srgb(0.7, 0.4, 150.0);
        for (idx, ch) in rgb.iter().copied().enumerate() {
            assert!(ch.is_finite(), "channel {idx} must be finite");
            assert!((0.0..=1.0).contains(&ch), "channel {idx} out of [0, 1]: {ch}");
        }
        assert!(rgb[1] > 0.8 && rgb[0] == 0.0, "oklch(0.7 0.4 150) should clamp red to 0.0 and keep high green, got {rgb:?}");

        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            for out in [
                srgb_to_oklab(bad, 0.5, 0.5),
                srgb_to_oklab(0.5, bad, 0.5),
                srgb_to_oklab(0.5, 0.5, bad),
                oklab_to_srgb(bad, 0.1, 0.1),
                oklab_to_srgb(0.5, bad, 0.1),
                oklab_to_srgb(0.5, 0.1, bad),
                oklab_to_oklch(bad, 0.1, 0.1),
                oklch_to_oklab(bad, 0.2, 30.0),
                oklch_to_oklab(0.5, bad, 30.0),
                oklch_to_oklab(0.5, 0.2, bad),
                srgb_to_oklch(bad, 0.0, 0.0),
                oklch_to_srgb(bad, 0.2, 150.0),
                oklch_to_srgb(0.7, bad, 150.0),
                oklch_to_srgb(0.7, 0.2, bad),
            ] {
                assert!(out.iter().all(|v| v.is_finite()), "non-finite output for {bad}: {out:?}");
            }
            for srgb_out in [oklab_to_srgb(bad, bad, bad), oklch_to_srgb(bad, bad, bad)] {
                assert!(srgb_out.iter().all(|v| (0.0..=1.0).contains(v)), "sRGB output out of [0, 1]: {srgb_out:?}");
            }
        }
    }
}
