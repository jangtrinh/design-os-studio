//! Number, color and XML helpers shared by the SVG and CSS writers, and the overlay split shared by
//! the native stack writers.

use designos_doc::{Affine, Color, ColorProfile, Node};

/// Colour notation used by Dev Mode inspection and CSS / Tailwind code generation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ColorFormat {
    #[default]
    Hex,
    Rgb,
    Oklch,
    Hsl,
}

impl ColorFormat {
    pub const ALL: [Self; 3] = [Self::Hex, Self::Rgb, Self::Oklch];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Hex => "Hex",
            Self::Rgb => "RGB",
            Self::Oklch => "OKLCH",
            Self::Hsl => "HSL",
        }
    }

    /// `alpha` already includes paint opacity; do not multiply color alpha again.
    pub fn value(self, color: Color, alpha: f32) -> String {
        self.value_in(ColorProfile::Srgb, color, alpha)
    }

    /// Formats `color` (written in `profile`) for Dev Mode inspection with combined `alpha`.
    pub fn value_in(self, profile: ColorProfile, color: Color, alpha: f32) -> String {
        let alpha = alpha.clamp(0.0, 1.0);
        let suffix = if alpha < 1.0 { format!(" / {}", decimal(alpha)) } else { String::new() };
        if profile.is_outside_srgb(&color) && self != Self::Oklch {
            let [r, g, b] = [color.r, color.g, color.b].map(|v| v.clamp(0.0, 1.0));
            return format!("color(display-p3 {} {} {}{suffix})", num(r), num(g), num(b));
        }
        if self == Self::Oklch {
            let [l, c, h] = profile.to_oklch(&color);
            return format!("oklch({}% {} {}{suffix})", fmt_num(l * 100.0), fmt_num(c), fmt_num(h));
        }
        let srgb = profile.to_srgb(&color);
        let [r, g, b] = [srgb.r, srgb.g, srgb.b].map(|v| v.clamp(0.0, 1.0));
        match self {
            Self::Hex => {
                let rgb = format!("#{:02X}{:02X}{:02X}", byte(r), byte(g), byte(b));
                if alpha < 1.0 {
                    format!("{rgb}{:02X}", byte(alpha))
                } else {
                    rgb
                }
            }
            Self::Rgb => format!("rgb({} {} {}{suffix})", byte(r), byte(g), byte(b)),
            Self::Oklch => unreachable!(),
            Self::Hsl => {
                let hi = r.max(g).max(b);
                let lo = r.min(g).min(b);
                let delta = hi - lo;
                let lightness = (hi + lo) / 2.0;
                let (hue, saturation) = if delta == 0.0 {
                    (0.0, 0.0)
                } else {
                    let hue = if hi == r {
                        ((g - b) / delta).rem_euclid(6.0)
                    } else if hi == g {
                        (b - r) / delta + 2.0
                    } else {
                        (r - g) / delta + 4.0
                    };
                    (hue * 60.0, delta / (1.0 - (2.0 * lightness - 1.0).abs()))
                };
                format!("hsl({} {}% {}%{suffix})", decimal(hue), decimal(saturation * 100.0), decimal(lightness * 100.0))
            }
        }
    }
}

fn decimal(v: f32) -> String {
    format!("{v:.6}").trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Shortest decimal form with at most 3 fractional digits (`12`, `0.5`, `-1.25`).
pub fn num(v: f32) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let s = format!("{:.3}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".into()
    } else {
        s.to_string()
    }
}

/// Like [`num`] with two fractional digits (alpha values in CSS).
pub fn num2(v: f32) -> String {
    let s = format!("{:.2}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".into()
    } else {
        s.to_string()
    }
}

/// Shortest decimal form with at most 2 fractional digits (`12`, `0.5`, `-1.25`),
/// matching UI / Inspect panel number formatting.
pub fn fmt_num(v: f32) -> String {
    let s = format!("{:.2}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" || s.is_empty() {
        "0".into()
    } else {
        s
    }
}

pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if (c as u32) < 0x20 && c != '\n' && c != '\t' => {}
            c => out.push(c),
        }
    }
    out
}

/// `#RRGGBB` ignoring alpha.
pub fn hex(c: &Color) -> String {
    format!("#{}", c.to_hex())
}

fn byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// CSS color the way Figma Dev Mode prints it: `#FFF` shorthand when
/// possible, `rgba(r, g, b, a)` when translucent.
pub fn css_color(c: &Color, opacity: f32) -> String {
    let a = (c.a * opacity).clamp(0.0, 1.0);
    let (r, g, b) = (byte(c.r), byte(c.g), byte(c.b));
    if byte(a) == 255 {
        let full = format!("{r:02X}{g:02X}{b:02X}");
        let b = full.as_bytes();
        if b[0] == b[1] && b[2] == b[3] && b[4] == b[5] {
            return format!("#{}{}{}", b[0] as char, b[2] as char, b[4] as char);
        }
        return format!("#{full}");
    }
    format!("rgba({r}, {g}, {b}, {})", num2(a))
}

/// Formats `c` (written in `profile`) for CSS declarations according to `fmt` and `profile`.
///
/// * In an sRGB document with [`ColorFormat::Hex`], delegates to [`css_color`] so existing snapshots
///   stay byte-for-byte identical.
/// * In a Display P3 document, colours outside the sRGB gamut emit `color(display-p3 r g b)` (or
///   `color(display-p3 r g b / a)` when translucent) unless [`ColorFormat::Oklch`] is selected.
pub fn css_color_in(profile: ColorProfile, fmt: ColorFormat, c: &Color, opacity: f32) -> String {
    let a = (c.a * opacity).clamp(0.0, 1.0);
    if profile.is_outside_srgb(c) && fmt != ColorFormat::Oklch {
        let [r, g, b] = [c.r, c.g, c.b].map(|v| v.clamp(0.0, 1.0));
        return if byte(a) == 255 {
            format!("color(display-p3 {} {} {})", num(r), num(g), num(b))
        } else {
            format!("color(display-p3 {} {} {} / {})", num(r), num(g), num(b), num2(a))
        };
    }
    match fmt {
        ColorFormat::Hex => {
            let srgb = profile.to_srgb(c);
            css_color(&srgb, opacity)
        }
        ColorFormat::Rgb | ColorFormat::Oklch | ColorFormat::Hsl => {
            let alpha = if byte(a) == 255 { 1.0 } else { num2(a).parse::<f32>().unwrap_or(a) };
            fmt.value_in(profile, *c, alpha)
        }
    }
}

pub fn is_identity(a: &Affine) -> bool {
    let e = 1e-6;
    (a.a - 1.0).abs() < e && a.b.abs() < e && a.c.abs() < e && (a.d - 1.0).abs() < e && a.tx.abs() < e && a.ty.abs() < e
}

/// SVG `transform` attribute value, or `None` for the identity.
pub fn svg_transform(a: &Affine) -> Option<String> {
    if is_identity(a) {
        return None;
    }
    let e = 1e-6;
    if (a.a - 1.0).abs() < e && a.b.abs() < e && a.c.abs() < e && (a.d - 1.0).abs() < e {
        return Some(format!("translate({} {})", num(a.tx), num(a.ty)));
    }
    Some(format!("matrix({} {} {} {} {} {})", num(a.a), num(a.b), num(a.c), num(a.d), num(a.tx), num(a.ty)))
}

/// The children of an auto layout frame split by where they paint relative to its in-flow content,
/// each list in child order.
pub(crate) struct OverlaySplit<'a> {
    /// Absolute children that come before the first in-flow child: they paint behind the content.
    pub behind: Vec<&'a Node>,
    /// Every child that is not absolute.
    pub flow: Vec<&'a Node>,
    /// The other absolute children: they paint over the content.
    pub front: Vec<&'a Node>,
}

/// Splits `kids` (in child order) into the in-flow content and the absolute children around it. A later
/// child paints over an earlier one, so an absolute child is emitted behind the content only when it
/// precedes every in-flow child. Without in-flow children every absolute child is `behind`, which
/// keeps child order.
pub(crate) fn split_overlay_children<'a>(kids: &[&'a Node]) -> OverlaySplit<'a> {
    let first_flow = kids.iter().position(|k| !k.layout_child.absolute).unwrap_or(kids.len());
    let mut split = OverlaySplit { behind: Vec::new(), flow: Vec::new(), front: Vec::new() };
    for (i, k) in kids.iter().enumerate() {
        if !k.layout_child.absolute {
            split.flow.push(k);
        } else if i < first_flow {
            split.behind.push(k);
        } else {
            split.front.push(k);
        }
    }
    split
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_trimmed() {
        assert_eq!(num(12.0), "12");
        assert_eq!(num(0.5), "0.5");
        assert_eq!(num(-0.0001), "0");
        assert_eq!(num(1.23456), "1.235");
        assert_eq!(fmt_num(-0.001), "0");
        assert_eq!(fmt_num(12.0), "12");
        assert_eq!(fmt_num(1.23456), "1.23");
    }

    #[test]
    fn css_colors_match_figma_style() {
        assert_eq!(css_color(&Color::WHITE, 1.0), "#FFF");
        assert_eq!(css_color(&Color::rgb(0xD9, 0xD9, 0xD9), 1.0), "#D9D9D9");
        assert_eq!(css_color(&Color::rgba(0, 0, 0, 64), 1.0), "rgba(0, 0, 0, 0.25)");
        assert_eq!(css_color(&Color::BLACK, 0.5), "rgba(0, 0, 0, 0.5)");
    }

    #[test]
    fn overlay_children_split_around_the_first_in_flow_child() {
        let mut doc = designos_doc::Document::new("overlay-split");
        let kids: Vec<Node> = [true, false, true, false, true]
            .into_iter()
            .map(|absolute| {
                let id = doc.alloc_id();
                let mut n = Node::rectangle(id, designos_doc::Rect::new(0.0, 0.0, 1.0, 1.0));
                n.layout_child.absolute = absolute;
                n
            })
            .collect();
        let refs: Vec<&Node> = kids.iter().collect();
        let ids = |v: &[&Node]| v.iter().map(|n| n.id).collect::<Vec<_>>();
        let split = split_overlay_children(&refs);
        assert_eq!(ids(&split.behind), vec![kids[0].id]);
        assert_eq!(ids(&split.flow), vec![kids[1].id, kids[3].id]);
        assert_eq!(ids(&split.front), vec![kids[2].id, kids[4].id]);

        let only_absolute: Vec<&Node> = vec![&kids[0], &kids[2]];
        let split = split_overlay_children(&only_absolute);
        assert_eq!(ids(&split.behind), vec![kids[0].id, kids[2].id]);
        assert!(split.flow.is_empty() && split.front.is_empty());
    }
}
