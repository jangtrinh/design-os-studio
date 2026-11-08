//! Copyable CSS color values for the developer inspector.

pub use designos_export::ColorFormat;

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Color, ColorProfile};

    #[test]
    fn css_values_include_combined_alpha_once() {
        let c = Color::rgb(255, 0, 0).with_alpha(0.5);
        assert_eq!(ColorFormat::ALL, [ColorFormat::Hex, ColorFormat::Rgb, ColorFormat::Oklch]);
        assert_eq!(ColorFormat::Hex.value(c, 0.25), "#FF000040");
        assert_eq!(ColorFormat::Rgb.value(c, 0.25), "rgb(255 0 0 / 0.25)");
        assert_eq!(ColorFormat::Oklch.value(c, 0.25), "oklch(62.8% 0.26 29.23 / 0.25)");
        assert_eq!(ColorFormat::Hsl.value(c, 0.25), "hsl(0 100% 50% / 0.25)");
        assert_eq!(ColorFormat::Rgb.value(c, 0.001), "rgb(255 0 0 / 0.001)");
        assert_eq!(ColorFormat::Hex.value(c, 0.0), "#FF000000");
        assert_eq!(ColorFormat::Hex.value_in(ColorProfile::DisplayP3, Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 }, 0.25), "color(display-p3 1 0 0 / 0.25)");
    }

    #[test]
    fn hsl_handles_achromatic_and_all_hue_sectors() {
        for (c, expected) in [
            (Color::BLACK, "hsl(0 0% 0%)"),
            (Color::WHITE, "hsl(0 0% 100%)"),
            (Color::rgb(255, 255, 0), "hsl(60 100% 50%)"),
            (Color::rgb(0, 255, 0), "hsl(120 100% 50%)"),
            (Color::rgb(0, 255, 255), "hsl(180 100% 50%)"),
            (Color::rgb(0, 0, 255), "hsl(240 100% 50%)"),
            (Color::rgb(255, 0, 255), "hsl(300 100% 50%)"),
        ] {
            assert_eq!(ColorFormat::Hsl.value(c, 1.0), expected);
        }
    }
}
