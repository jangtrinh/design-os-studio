//! The colour profile a document's colour values are written in.
//!
//! A [`crate::Color`] is a straight RGB triple. Which primaries those numbers refer to is a property
//! of the document: `Srgb` (the default, what every file used before profiles existed) or
//! `DisplayP3`. Renderers draw to an sRGB surface, so a Display P3 document converts each colour
//! with [`ColorProfile::to_srgb`]; an sRGB document is never touched (bit-exact).
//!
//! Scope: on screen a Display P3 document is converted to sRGB and colours outside the sRGB gamut
//! clip. The canvas is not a wide-gamut surface (nothing sets a Display P3 colour space on the
//! window layer), so this does not claim to match Figma on a P3 display. Exports are different:
//! a PNG keeps the document's own numbers and carries the Display P3 tag.
//!
//! Each paint is converted before it is blended (translucent layers, gradients and blend modes
//! combine converted colours). Whether Figma blends in the document's space is not documented.

use crate::color_space::{linear_channel_to_srgb, srgb_channel_to_linear};
use crate::{Color, NoiseEffect, NoiseMode};
use serde::{Deserialize, Serialize};

/// Colour space of the numbers stored in a document's [`Color`] values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum ColorProfile {
    /// sRGB primaries (the default).
    #[default]
    Srgb,
    /// Display P3 primaries with the sRGB transfer curve.
    DisplayP3,
}

/// Linear Display P3 -> linear sRGB (D65 both).
const P3_TO_SRGB: [[f32; 3]; 3] = [[1.224_940_2, -0.224_940_4, 0.0], [-0.042_056_9, 1.042_057_1, 0.0], [-0.019_637_6, -0.078_636_1, 1.098_273_5]];

/// Linear sRGB -> linear Display P3 (inverse of [`P3_TO_SRGB`]).
const SRGB_TO_P3: [[f32; 3]; 3] = [[0.822_462, 0.177_538, 0.0], [0.033_194_2, 0.966_805_8, 0.0], [0.017_082_6, 0.072_397_4, 0.910_519_9]];

impl ColorProfile {
    pub const ALL: [ColorProfile; 2] = [ColorProfile::Srgb, ColorProfile::DisplayP3];

    pub fn is_srgb(&self) -> bool {
        matches!(self, ColorProfile::Srgb)
    }

    /// Name shown to the user.
    pub fn label(&self) -> &'static str {
        match self {
            ColorProfile::Srgb => "sRGB",
            ColorProfile::DisplayP3 => "Display P3",
        }
    }

    /// Figma's `DocumentColorProfile` enum name. `LEGACY` and `SRGB` both read as sRGB.
    pub fn from_figma_name(name: &str) -> Self {
        match name {
            "DISPLAY_P3" => ColorProfile::DisplayP3,
            _ => ColorProfile::Srgb,
        }
    }

    /// Figma name written for this profile, `None` for sRGB (the key is then left out of the file).
    pub fn figma_name(&self) -> Option<&'static str> {
        match self {
            ColorProfile::Srgb => None,
            ColorProfile::DisplayP3 => Some("DISPLAY_P3"),
        }
    }

    /// `c`, an sRGB-encoded colour (an untagged image pixel, say), as numbers written in this profile.
    /// Every sRGB colour lies inside Display P3, so nothing clips. An sRGB profile returns `c`.
    pub fn from_srgb(&self, c: &Color) -> Color {
        match self {
            ColorProfile::Srgb => *c,
            ColorProfile::DisplayP3 => {
                let lin = [srgb_channel_to_linear(c.r), srgb_channel_to_linear(c.g), srgb_channel_to_linear(c.b)];
                let out = |row: &[f32; 3]| linear_channel_to_srgb((row[0] * lin[0] + row[1] * lin[1] + row[2] * lin[2]).clamp(0.0, 1.0));
                Color { r: out(&SRGB_TO_P3[0]), g: out(&SRGB_TO_P3[1]), b: out(&SRGB_TO_P3[2]), a: c.a }
            }
        }
    }

    /// `c` (written in this profile) as an sRGB-encoded colour, clipped to the sRGB gamut.
    /// Alpha is unchanged. An sRGB colour is returned as it is.
    pub fn to_srgb(&self, c: &Color) -> Color {
        match self {
            ColorProfile::Srgb => *c,
            ColorProfile::DisplayP3 => {
                let lin = [srgb_channel_to_linear(c.r), srgb_channel_to_linear(c.g), srgb_channel_to_linear(c.b)];
                let out = |row: &[f32; 3]| linear_channel_to_srgb((row[0] * lin[0] + row[1] * lin[1] + row[2] * lin[2]).clamp(0.0, 1.0));
                Color { r: out(&P3_TO_SRGB[0]), g: out(&P3_TO_SRGB[1]), b: out(&P3_TO_SRGB[2]), a: c.a }
            }
        }
    }

    /// Whether `c` (written in this profile) lies outside the sRGB gamut.
    ///
    /// An sRGB profile always returns `false`. In Display P3, colours whose unclipped linear sRGB
    /// coordinates exceed `0.0..=1.0` beyond matrix rounding tolerance are outside sRGB.
    pub fn is_outside_srgb(&self, c: &Color) -> bool {
        match self {
            ColorProfile::Srgb => false,
            ColorProfile::DisplayP3 => {
                if c.r == c.g && c.g == c.b {
                    return false;
                }
                const GAMUT_EPS: f32 = 1e-4;
                let lin = [srgb_channel_to_linear(c.r), srgb_channel_to_linear(c.g), srgb_channel_to_linear(c.b)];
                let dot = |row: &[f32; 3]| row[0] * lin[0] + row[1] * lin[1] + row[2] * lin[2];
                P3_TO_SRGB.iter().any(|row| {
                    let v = dot(row);
                    !(-GAMUT_EPS..=1.0 + GAMUT_EPS).contains(&v)
                })
            }
        }
    }

    /// Converts `c` (written in this profile) to `OKLab` `[L, a, b]` without clipping wide-gamut
    /// Display P3 colours to sRGB first.
    pub fn to_oklab(&self, c: &Color) -> [f32; 3] {
        match self {
            ColorProfile::Srgb => crate::color_space::srgb_to_oklab(c.r, c.g, c.b),
            ColorProfile::DisplayP3 => {
                if c.r == c.g && c.g == c.b {
                    return crate::color_space::srgb_to_oklab(c.r, c.g, c.b);
                }
                let lin = [srgb_channel_to_linear(c.r), srgb_channel_to_linear(c.g), srgb_channel_to_linear(c.b)];
                let dot = |row: &[f32; 3]| row[0] * lin[0] + row[1] * lin[1] + row[2] * lin[2];
                crate::color_space::linear_srgb_to_oklab(dot(&P3_TO_SRGB[0]), dot(&P3_TO_SRGB[1]), dot(&P3_TO_SRGB[2]))
            }
        }
    }

    /// Converts `c` (written in this profile) to `OKLCH` `[L, C, H_deg]` (`H_deg` in `0.0..360.0`)
    /// without clipping wide-gamut Display P3 colours to sRGB first.
    pub fn to_oklch(&self, c: &Color) -> [f32; 3] {
        let [l, a, b] = self.to_oklab(c);
        crate::color_space::oklab_to_oklch(l, a, b)
    }
}

impl ColorProfile {
    /// `n` with its grain colours converted to sRGB, `None` when nothing changes (sRGB profile, or a
    /// multi-hue grain, which has no stored colour).
    pub fn noise_to_srgb(&self, n: &NoiseEffect) -> Option<NoiseEffect> {
        if self.is_srgb() {
            return None;
        }
        let mode = match &n.mode {
            NoiseMode::Mono { color } => NoiseMode::Mono { color: self.to_srgb(color) },
            NoiseMode::Duo { color, secondary } => NoiseMode::Duo { color: self.to_srgb(color), secondary: self.to_srgb(secondary) },
            NoiseMode::Multi => return None,
        };
        Some(NoiseEffect { mode, ..n.clone() })
    }
}

/// Converts many 8-bit sRGB pixels (image bytes) to a profile with lookup tables, so a whole photo
/// takes tens of milliseconds instead of one `powf` pair per channel per pixel. Agrees with
/// [`ColorProfile::from_srgb`] to within one level.
pub struct SrgbPixelConverter {
    decode: [f32; 256],
    encode: Vec<u8>,
    matrix: Option<[[f32; 3]; 3]>,
}

const ENCODE_STEPS: usize = 16384;

impl SrgbPixelConverter {
    pub fn new(profile: ColorProfile) -> Self {
        let decode = std::array::from_fn(|i| srgb_channel_to_linear(i as f32 / 255.0));
        let encode = (0..ENCODE_STEPS).map(|i| (linear_channel_to_srgb(i as f32 / (ENCODE_STEPS - 1) as f32) * 255.0).round() as u8).collect();
        let matrix = match profile {
            ColorProfile::Srgb => None,
            ColorProfile::DisplayP3 => Some(SRGB_TO_P3),
        };
        Self { decode, encode, matrix }
    }

    /// `rgb` (sRGB bytes) as bytes written in the profile.
    #[inline]
    pub fn convert(&self, rgb: [u8; 3]) -> [u8; 3] {
        let Some(m) = &self.matrix else { return rgb };
        let lin = [self.decode[rgb[0] as usize], self.decode[rgb[1] as usize], self.decode[rgb[2] as usize]];
        let ch = |row: &[f32; 3]| {
            let v = (row[0] * lin[0] + row[1] * lin[1] + row[2] * lin[2]).clamp(0.0, 1.0);
            self.encode[(v * (ENCODE_STEPS - 1) as f32).round() as usize]
        };
        [ch(&m[0]), ch(&m[1]), ch(&m[2])]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_profile_returns_the_colour_untouched() {
        let c = Color { r: 0.123, g: 0.456, b: 0.789, a: 0.5 };
        assert_eq!(ColorProfile::Srgb.to_srgb(&c), c);
    }

    #[test]
    fn display_p3_white_black_and_grey_stay_neutral() {
        for v in [0.0_f32, 0.25, 0.5, 1.0] {
            let o = ColorProfile::DisplayP3.to_srgb(&Color { r: v, g: v, b: v, a: 0.7 });
            assert!((o.r - v).abs() < 2e-3 && (o.g - v).abs() < 2e-3 && (o.b - v).abs() < 2e-3, "{v} -> {o:?}");
            assert_eq!(o.a, 0.7);
        }
    }

    #[test]
    fn display_p3_red_clips_to_the_srgb_gamut() {
        // P3 red is outside sRGB: green and blue clip to 0, red clips to 1.
        let o = ColorProfile::DisplayP3.to_srgb(&Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 });
        assert!((o.r - 1.0).abs() < 1e-6 && o.g == 0.0 && o.b == 0.0, "{o:?}");
    }

    #[test]
    fn display_p3_mid_colour_matches_the_reference_conversion() {
        // Matrix derived from the CSS Color 4 primaries and white point: color(display-p3 0.5 0.4 0.8) is
        // rgb(0.51926, 0.39509, 0.82582) in sRGB.
        let o = ColorProfile::DisplayP3.to_srgb(&Color { r: 0.5, g: 0.4, b: 0.8, a: 1.0 });
        assert!((o.r - 0.51926).abs() < 1e-3 && (o.g - 0.39509).abs() < 1e-3 && (o.b - 0.82582).abs() < 1e-3, "{o:?}");
    }

    #[test]
    fn from_srgb_inverts_to_srgb_for_colours_inside_the_srgb_gamut() {
        for c in [Color { r: 0.2, g: 0.6, b: 0.9, a: 0.5 }, Color { r: 0.78, g: 0.23, b: 0.23, a: 1.0 }, Color::WHITE, Color::BLACK] {
            let back = ColorProfile::DisplayP3.to_srgb(&ColorProfile::DisplayP3.from_srgb(&c));
            assert!((back.r - c.r).abs() < 2e-3 && (back.g - c.g).abs() < 2e-3 && (back.b - c.b).abs() < 2e-3, "{c:?} -> {back:?}");
            assert_eq!(back.a, c.a);
        }
        let c = Color { r: 0.3, g: 0.4, b: 0.5, a: 1.0 };
        assert_eq!(ColorProfile::Srgb.from_srgb(&c), c);
    }

    #[test]
    fn the_pixel_converter_agrees_with_from_srgb_within_one_level() {
        let conv = SrgbPixelConverter::new(ColorProfile::DisplayP3);
        for r in (0..=255).step_by(15) {
            for g in (0..=255).step_by(15) {
                for b in (0..=255).step_by(15) {
                    let want = ColorProfile::DisplayP3.from_srgb(&Color::rgb(r, g, b));
                    let got = conv.convert([r, g, b]);
                    for (w, g) in [want.r, want.g, want.b].into_iter().zip(got) {
                        assert!(((w * 255.0).round() - f32::from(g)).abs() <= 1.0, "{r},{g},{b}");
                    }
                }
            }
        }
        assert_eq!(SrgbPixelConverter::new(ColorProfile::Srgb).convert([1, 2, 3]), [1, 2, 3]);
    }

    #[test]
    fn figma_names_round_trip_and_legacy_reads_as_srgb() {
        assert_eq!(ColorProfile::from_figma_name("DISPLAY_P3"), ColorProfile::DisplayP3);
        assert_eq!(ColorProfile::from_figma_name("SRGB"), ColorProfile::Srgb);
        assert_eq!(ColorProfile::from_figma_name("LEGACY"), ColorProfile::Srgb);
        assert_eq!(ColorProfile::DisplayP3.figma_name(), Some("DISPLAY_P3"));
        assert_eq!(ColorProfile::Srgb.figma_name(), None);
    }

    #[test]
    fn display_p3_gamut_boundary_and_outside_srgb_detection() {
        // sRGB profile never reports a colour as outside sRGB.
        assert!(!ColorProfile::Srgb.is_outside_srgb(&Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 }));
        assert!(!ColorProfile::Srgb.is_outside_srgb(&Color { r: 0.0, g: 1.0, b: 0.0, a: 1.0 }));
        assert!(!ColorProfile::Srgb.is_outside_srgb(&Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 }));

        // Neutral greys, black, and white in Display P3 share D65 and are inside sRGB.
        for v in [0.0_f32, 0.25, 0.5, 0.75, 1.0] {
            assert!(!ColorProfile::DisplayP3.is_outside_srgb(&Color { r: v, g: v, b: v, a: 1.0 }), "neutral {v}");
        }

        // Any sRGB primary or corner mapped into Display P3 via from_srgb lies on the sRGB gamut boundary (inside sRGB).
        for srgb_corner in [
            Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 },
            Color { r: 0.0, g: 1.0, b: 0.0, a: 1.0 },
            Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 },
            Color { r: 1.0, g: 1.0, b: 0.0, a: 1.0 },
            Color { r: 0.0, g: 1.0, b: 1.0, a: 1.0 },
            Color { r: 1.0, g: 0.0, b: 1.0, a: 1.0 },
            Color { r: 0.5, g: 0.4, b: 0.8, a: 1.0 },
        ] {
            let in_p3 = ColorProfile::DisplayP3.from_srgb(&srgb_corner);
            assert!(!ColorProfile::DisplayP3.is_outside_srgb(&in_p3), "sRGB corner {srgb_corner:?} in P3 ({in_p3:?}) must be inside sRGB");
        }

        // Display P3 primaries and wide-gamut secondaries are outside sRGB.
        for p3_wide in [
            Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 },
            Color { r: 0.0, g: 1.0, b: 0.0, a: 1.0 },
            Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 },
            Color { r: 1.0, g: 1.0, b: 0.0, a: 1.0 },
            Color { r: 0.0, g: 1.0, b: 1.0, a: 1.0 },
            Color { r: 1.0, g: 0.0, b: 1.0, a: 1.0 },
        ] {
            assert!(ColorProfile::DisplayP3.is_outside_srgb(&p3_wide), "P3 primary {p3_wide:?} must be outside sRGB");
        }
    }

    #[test]
    fn display_p3_oklab_and_oklch_preserve_wide_gamut_chroma_and_match_css_color_4() {
        let tol = 1e-3;
        // CSS Color 4: Display P3 red color(display-p3 1 0 0) -> OKLCH(0.6486, 0.2995, 28.96 deg), higher chroma than sRGB red (0.2577).
        let p3_red_lch = ColorProfile::DisplayP3.to_oklch(&Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 });
        let srgb_red_lch = ColorProfile::Srgb.to_oklch(&Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 });
        assert!((p3_red_lch[0] - 0.6486).abs() <= tol, "P3 red L: {:?}", p3_red_lch);
        assert!((p3_red_lch[1] - 0.2995).abs() <= tol, "P3 red C: {:?}", p3_red_lch);
        assert!((p3_red_lch[2] - 28.96).abs() <= 0.05, "P3 red H: {:?}", p3_red_lch);
        assert!(p3_red_lch[1] > srgb_red_lch[1] + 0.03, "P3 red chroma must exceed sRGB red chroma");

        // CSS Color 4: Display P3 green color(display-p3 0 1 0) -> OKLCH(0.8488, 0.3685, 145.64 deg), higher chroma than sRGB green (0.2948).
        let p3_green_lch = ColorProfile::DisplayP3.to_oklch(&Color { r: 0.0, g: 1.0, b: 0.0, a: 1.0 });
        let srgb_green_lch = ColorProfile::Srgb.to_oklch(&Color { r: 0.0, g: 1.0, b: 0.0, a: 1.0 });
        assert!((p3_green_lch[0] - 0.8488).abs() <= tol, "P3 green L: {:?}", p3_green_lch);
        assert!((p3_green_lch[1] - 0.3685).abs() <= tol, "P3 green C: {:?}", p3_green_lch);
        assert!((p3_green_lch[2] - 145.64).abs() <= 0.05, "P3 green H: {:?}", p3_green_lch);
        assert!(p3_green_lch[1] > srgb_green_lch[1] + 0.05, "P3 green C ({:?}) must exceed sRGB green C ({:?})", p3_green_lch, srgb_green_lch);

        // An sRGB colour converted into Display P3 via from_srgb has the same OKLCH as in sRGB.
        let sample = Color { r: 0.2, g: 0.6, b: 0.9, a: 1.0 };
        let in_p3 = ColorProfile::DisplayP3.from_srgb(&sample);
        let lch_srgb = ColorProfile::Srgb.to_oklch(&sample);
        let lch_p3 = ColorProfile::DisplayP3.to_oklch(&in_p3);
        assert!((lch_srgb[0] - lch_p3[0]).abs() <= tol && (lch_srgb[1] - lch_p3[1]).abs() <= tol && (lch_srgb[2] - lch_p3[2]).abs() <= 0.1);
    }
}
