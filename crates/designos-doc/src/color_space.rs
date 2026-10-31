//! Perceptual colour-space conversions (`sRGB` ↔ `OKLab` ↔ `OKLCH`) in `f32`.
//!
//! Uses Björn Ottosson's canonical OKLab matrices and the IEC 61966-2-1 sRGB transfer
//! function, clamping linear sRGB channels to `[0.0, 1.0]` before gamma encoding.

/// Converts straight sRGB channels (`0.0..=1.0`) to `OKLab` `[L, a, b]`.
pub fn srgb_to_oklab(_r: f32, _g: f32, _b: f32) -> [f32; 3] {
    [0.0, 0.0, 0.0]
}

/// Converts `OKLab` `[L, a, b]` to straight sRGB channels (`0.0..=1.0`),
/// clamping each linear sRGB channel to `[0.0, 1.0]`.
pub fn oklab_to_srgb(_l: f32, _a: f32, _b: f32) -> [f32; 3] {
    [0.0, 0.0, 0.0]
}

/// Converts `OKLab` `[L, a, b]` to `OKLCH` `[L, C, H_deg]` with `H_deg` in `0.0..360.0`.
pub fn oklab_to_oklch(_l: f32, _a: f32, _b: f32) -> [f32; 3] {
    [0.0, 0.0, 0.0]
}

/// Converts `OKLCH` `[L, C, H_deg]` (`H_deg` in degrees) to `OKLab` `[L, a, b]`.
pub fn oklch_to_oklab(_l: f32, _c: f32, _h_deg: f32) -> [f32; 3] {
    [0.0, 0.0, 0.0]
}

/// Converts straight sRGB channels (`0.0..=1.0`) to `OKLCH` `[L, C, H_deg]`.
pub fn srgb_to_oklch(_r: f32, _g: f32, _b: f32) -> [f32; 3] {
    [0.0, 0.0, 0.0]
}

/// Converts `OKLCH` `[L, C, H_deg]` to straight sRGB channels (`0.0..=1.0`),
/// clamping each linear sRGB channel to `[0.0, 1.0]`.
pub fn oklch_to_srgb(_l: f32, _c: f32, _h_deg: f32) -> [f32; 3] {
    [0.0, 0.0, 0.0]
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
