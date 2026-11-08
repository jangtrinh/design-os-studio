//! The colour profile a document's colour values are written in.
//!
//! A [`crate::Color`] is a straight RGB triple. Which primaries those numbers refer to is a property
//! of the document: `Srgb` (the default, what every file used before profiles existed) or
//! `DisplayP3`. Renderers draw to an sRGB surface, so a Display P3 document converts each colour
//! with [`ColorProfile::to_srgb`]; an sRGB document is never touched (bit-exact).

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
    fn figma_names_round_trip_and_legacy_reads_as_srgb() {
        assert_eq!(ColorProfile::from_figma_name("DISPLAY_P3"), ColorProfile::DisplayP3);
        assert_eq!(ColorProfile::from_figma_name("SRGB"), ColorProfile::Srgb);
        assert_eq!(ColorProfile::from_figma_name("LEGACY"), ColorProfile::Srgb);
        assert_eq!(ColorProfile::DisplayP3.figma_name(), Some("DISPLAY_P3"));
        assert_eq!(ColorProfile::Srgb.figma_name(), None);
    }
}
