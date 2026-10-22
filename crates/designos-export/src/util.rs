//! Number, color and XML helpers shared by the SVG and CSS writers.

use designos_doc::{Affine, Color};

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
}
