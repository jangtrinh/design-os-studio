//! CSS background, border, corner radius, shadow, blur, opacity, blend-mode, and rotation
//! mapping into [`designos_doc`] paint, stroke, effect, and transform primitives.

use crate::import::{ImportReport, UnsupportedCssEntry};
use designos_doc::{Affine, BlendMode, Color, Effect, GradientStop, ImageScaleMode, Paint, PaintKind, Stroke, StrokeAlign, StrokeCap, StrokeJoin, Vec2};
use std::collections::BTreeMap;

/// Parses a CSS length (`"16px"`, `"1.5rem"`, `"0"`, `"-4px"`) into pixels.
/// Uses `font_size` for `em`/`rem` (defaulting to `16.0`).
pub fn parse_css_px(raw: &str, font_size: f32) -> Option<f32> {
    let s = raw.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("none") || s.eq_ignore_ascii_case("auto") || s.eq_ignore_ascii_case("normal") {
        return None;
    }
    if let Some(num) = s.strip_suffix("px") {
        return num.trim().parse::<f32>().ok().filter(|v| v.is_finite());
    }
    if let Some(num) = s.strip_suffix("rem") {
        return num.trim().parse::<f32>().ok().filter(|v| v.is_finite()).map(|v| v * 16.0);
    }
    if let Some(num) = s.strip_suffix("em") {
        let base = if font_size > 0.0 { font_size } else { 16.0 };
        return num.trim().parse::<f32>().ok().filter(|v| v.is_finite()).map(|v| v * base);
    }
    if let Some(num) = s.strip_suffix("pt") {
        return num.trim().parse::<f32>().ok().filter(|v| v.is_finite()).map(|v| v * (96.0 / 72.0));
    }
    s.parse::<f32>().ok().filter(|v| v.is_finite())
}

/// Parses a CSS length or percentage relative to `basis`.
pub fn parse_css_px_or_percent(raw: &str, basis: f32, font_size: f32) -> Option<f32> {
    let s = raw.trim();
    if let Some(pct) = s.strip_suffix('%') {
        let p = pct.trim().parse::<f32>().ok().filter(|v| v.is_finite())?;
        return Some(basis * (p / 100.0));
    }
    parse_css_px(s, font_size)
}

/// Parses any CSS color (`#hex`, named, `rgb()`, `rgba()`, `hsl()`, `hsla()`, `oklch()`, `color(srgb ...)`).
pub fn parse_css_color(raw: &str) -> Option<Color> {
    let s = raw.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("none") || s.eq_ignore_ascii_case("initial") || s.eq_ignore_ascii_case("inherit") {
        return None;
    }
    if s.eq_ignore_ascii_case("transparent") {
        return Some(Color::TRANSPARENT);
    }
    if s.eq_ignore_ascii_case("currentcolor") {
        return None;
    }
    if let Some(c) = Color::from_hex(s) {
        return Some(c);
    }
    let lower = s.to_ascii_lowercase();
    if let Some(inner) = lower.strip_prefix("rgba(").or_else(|| lower.strip_prefix("rgb(")).and_then(|v| v.strip_suffix(')')) {
        return parse_rgb_args(inner);
    }
    if let Some(inner) = lower.strip_prefix("hsla(").or_else(|| lower.strip_prefix("hsl(")).and_then(|v| v.strip_suffix(')')) {
        return parse_hsl_args(inner);
    }
    if let Some(inner) = lower.strip_prefix("oklch(").and_then(|v| v.strip_suffix(')')) {
        return parse_oklch_args(inner);
    }
    if let Some(inner) = lower.strip_prefix("color(").and_then(|v| v.strip_suffix(')')) {
        let inner = inner.trim();
        if let Some(rest) = inner.strip_prefix("srgb") {
            return parse_srgb_float_args(rest);
        }
    }
    None
}

fn split_color_components(inner: &str) -> (Vec<&str>, Option<&str>) {
    if let Some((lhs, alpha)) = inner.split_once('/') {
        let comps: Vec<&str> = lhs.split(|c: char| c == ',' || c.is_ascii_whitespace()).map(str::trim).filter(|t| !t.is_empty()).collect();
        (comps, Some(alpha.trim()))
    } else if inner.contains(',') {
        let comps: Vec<&str> = inner.split(',').map(str::trim).filter(|t| !t.is_empty()).collect();
        if comps.len() >= 4 {
            (comps[..3].to_vec(), Some(comps[3]))
        } else {
            (comps, None)
        }
    } else {
        let comps: Vec<&str> = inner.split_whitespace().collect();
        if comps.len() >= 4 {
            (comps[..3].to_vec(), Some(comps[3]))
        } else {
            (comps, None)
        }
    }
}

fn parse_alpha(raw: Option<&str>) -> Option<f32> {
    let Some(a) = raw else { return Some(1.0) };
    if let Some(pct) = a.strip_suffix('%') {
        return pct.trim().parse::<f32>().ok().map(|v| (v / 100.0).clamp(0.0, 1.0));
    }
    a.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0))
}

fn parse_channel_u8_or_pct(raw: &str) -> Option<f32> {
    if let Some(pct) = raw.strip_suffix('%') {
        return pct.trim().parse::<f32>().ok().map(|v| (v / 100.0).clamp(0.0, 1.0));
    }
    raw.parse::<f32>().ok().map(|v| (v / 255.0).clamp(0.0, 1.0))
}

fn parse_rgb_args(inner: &str) -> Option<Color> {
    let (comps, alpha) = split_color_components(inner);
    if comps.len() < 3 {
        return None;
    }
    let r = parse_channel_u8_or_pct(comps[0])?;
    let g = parse_channel_u8_or_pct(comps[1])?;
    let b = parse_channel_u8_or_pct(comps[2])?;
    let a = parse_alpha(alpha)?;
    Some(Color { r, g, b, a })
}

fn parse_srgb_float_args(inner: &str) -> Option<Color> {
    let (comps, alpha) = split_color_components(inner);
    if comps.len() < 3 {
        return None;
    }
    let parse_f = |s: &str| -> Option<f32> {
        if let Some(pct) = s.strip_suffix('%') {
            pct.trim().parse::<f32>().ok().map(|v| (v / 100.0).clamp(0.0, 1.0))
        } else {
            s.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0))
        }
    };
    Some(Color { r: parse_f(comps[0])?, g: parse_f(comps[1])?, b: parse_f(comps[2])?, a: parse_alpha(alpha)? })
}

fn parse_angle_deg(raw: &str) -> Option<f32> {
    let s = raw.trim();
    if let Some(v) = s.strip_suffix("deg") {
        return v.trim().parse::<f32>().ok();
    }
    if let Some(v) = s.strip_suffix("rad") {
        return v.trim().parse::<f32>().ok().map(|r| r.to_degrees());
    }
    if let Some(v) = s.strip_suffix("turn") {
        return v.trim().parse::<f32>().ok().map(|t| t * 360.0);
    }
    if let Some(v) = s.strip_suffix("grad") {
        return v.trim().parse::<f32>().ok().map(|g| g * 0.9);
    }
    s.parse::<f32>().ok()
}

fn parse_hsl_args(inner: &str) -> Option<Color> {
    let (comps, alpha) = split_color_components(inner);
    if comps.len() < 3 {
        return None;
    }
    let h = parse_angle_deg(comps[0])?.rem_euclid(360.0) / 360.0;
    let s = comps[1].strip_suffix('%').unwrap_or(comps[1]).trim().parse::<f32>().ok()? / 100.0;
    let l = comps[2].strip_suffix('%').unwrap_or(comps[2]).trim().parse::<f32>().ok()? / 100.0;
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    let a = parse_alpha(alpha)?;

    let (r, g, b) = if s == 0.0 {
        (l, l, l)
    } else {
        let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
        let p = 2.0 * l - q;
        let hue_to_rgb = |mut t: f32| -> f32 {
            if t < 0.0 {
                t += 1.0;
            }
            if t > 1.0 {
                t -= 1.0;
            }
            if t < 1.0 / 6.0 {
                p + (q - p) * 6.0 * t
            } else if t < 1.0 / 2.0 {
                q
            } else if t < 2.0 / 3.0 {
                p + (q - p) * (2.0 / 3.0 - t) * 6.0
            } else {
                p
            }
        };
        (hue_to_rgb(h + 1.0 / 3.0), hue_to_rgb(h), hue_to_rgb(h - 1.0 / 3.0))
    };
    Some(Color { r, g, b, a })
}

fn parse_oklch_args(inner: &str) -> Option<Color> {
    let (comps, alpha) = split_color_components(inner);
    if comps.len() < 3 {
        return None;
    }
    let l = if let Some(pct) = comps[0].strip_suffix('%') { pct.trim().parse::<f32>().ok()? / 100.0 } else { comps[0].parse::<f32>().ok()? };
    let c = if let Some(pct) = comps[1].strip_suffix('%') { (pct.trim().parse::<f32>().ok()? / 100.0) * 0.4 } else { comps[1].parse::<f32>().ok()? };
    let h_deg = parse_angle_deg(comps[2])?;
    let h_rad = h_deg.to_radians();
    let a_ok = c * h_rad.cos();
    let b_ok = c * h_rad.sin();

    let l_ = l + 0.396_337_78 * a_ok + 0.215_803_76 * b_ok;
    let m_ = l - 0.105_561_346 * a_ok - 0.063_854_17 * b_ok;
    let s_ = l - 0.089_484_18 * a_ok - 1.291_485_5 * b_ok;

    let l3 = l_ * l_ * l_;
    let m3 = m_ * m_ * m_;
    let s3 = s_ * s_ * s_;

    let r_lin = 4.076_741_7 * l3 - 3.307_711_6 * m3 + 0.230_969_94 * s3;
    let g_lin = -1.268_438 * l3 + 2.609_757_4 * m3 - 0.341_319_38 * s3;
    let b_lin = -0.004_196_086_3 * l3 - 0.703_418_6 * m3 + 1.707_614_7 * s3;

    let to_srgb = |x: f32| -> f32 {
        let x = x.clamp(0.0, 1.0);
        if x <= 0.003_130_8 {
            12.92 * x
        } else {
            1.055 * x.powf(1.0 / 2.4) - 0.055
        }
    };
    Some(Color { r: to_srgb(r_lin), g: to_srgb(g_lin), b: to_srgb(b_lin), a: parse_alpha(alpha)? })
}

/// Decodes a `data:[<mediatype>][;base64],<payload>` URL into raw bytes.
pub fn decode_data_url(url: &str) -> Option<(String, Vec<u8>)> {
    let rest = url.trim().strip_prefix("data:")?;
    let (meta, payload) = rest.split_once(',')?;
    let is_base64 = meta.split(';').any(|p| p.trim().eq_ignore_ascii_case("base64"));
    let mime = meta.split(';').next().unwrap_or("application/octet-stream").trim().to_ascii_lowercase();
    let bytes = if is_base64 { decode_base64(payload)? } else { percent_decode(payload) };
    Some((mime, bytes))
}

fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u8;
    for b in input.bytes() {
        if b.is_ascii_whitespace() {
            continue;
        }
        if b == b'=' {
            break;
        }
        let val = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        };
        buf = (buf << 6) | (val as u32);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xFF) as u8);
        }
    }
    Some(out)
}

fn percent_decode(input: &str) -> Vec<u8> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = ((bytes[i + 1] as char).to_digit(16), (bytes[i + 2] as char).to_digit(16)) {
                out.push(((h << 4) | l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

/// Computes a deterministic 16-hex-char FNV-1a 64-bit hash and stores `bytes` in `images`
/// if not already present (deduplicating identical images across the document).
pub fn store_image_dedup(images: &mut BTreeMap<String, Vec<u8>>, bytes: Vec<u8>) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in &bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    let key = format!("webcap_{hash:016x}");
    images.entry(key.clone()).or_insert(bytes);
    key
}

/// Maps CSS `background-color` and `background-image` (`linear-gradient`, `radial-gradient`, `url(data:...)`)
/// into [`Paint`] layers.
pub fn map_background_paints(el_id: &str, styles: &BTreeMap<String, String>, images: &mut BTreeMap<String, Vec<u8>>, report: &mut ImportReport) -> Vec<Paint> {
    let mut paints = Vec::new();

    if let Some(bg_color) = styles.get("background-color") {
        if let Some(c) = parse_css_color(bg_color) {
            if c.a > 0.0 {
                paints.push(Paint::solid(c));
            }
        }
    }

    if let Some(bg_img) = styles.get("background-image") {
        let trimmed = bg_img.trim();
        if !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("none") {
            for layer in split_top_level_commas(trimmed) {
                let layer = layer.trim();
                let lower = layer.to_ascii_lowercase();
                if lower.starts_with("linear-gradient(") && layer.ends_with(')') {
                    let inner = &layer["linear-gradient(".len()..layer.len() - 1];
                    if let Some(paint) = parse_linear_gradient(inner) {
                        paints.push(paint);
                    } else {
                        report.unsupported_css.push(UnsupportedCssEntry {
                            element_id: el_id.to_string(),
                            property: "background-image".into(),
                            value: layer.into(),
                            note: "complex linear-gradient skipped".into(),
                        });
                    }
                } else if lower.starts_with("radial-gradient(") && layer.ends_with(')') {
                    let inner = &layer["radial-gradient(".len()..layer.len() - 1];
                    if let Some(paint) = parse_radial_gradient(inner) {
                        paints.push(paint);
                    } else {
                        report.unsupported_css.push(UnsupportedCssEntry {
                            element_id: el_id.to_string(),
                            property: "background-image".into(),
                            value: layer.into(),
                            note: "complex radial-gradient skipped".into(),
                        });
                    }
                } else if lower.starts_with("url(") && layer.ends_with(')') {
                    let url_inner = layer[4..layer.len() - 1].trim().trim_matches(|c| c == '"' || c == '\'');
                    if url_inner.starts_with("data:") {
                        if let Some((_, bytes)) = decode_data_url(url_inner) {
                            let scale_mode = match styles.get("background-size").map(|s| s.trim().to_ascii_lowercase()).as_deref() {
                                Some("contain") => ImageScaleMode::Fit,
                                Some("cover") => ImageScaleMode::Fill,
                                _ => {
                                    if matches!(styles.get("background-repeat").map(|s| s.trim()), Some("repeat")) {
                                        ImageScaleMode::Tile
                                    } else {
                                        ImageScaleMode::Fill
                                    }
                                }
                            };
                            let image_ref = store_image_dedup(images, bytes);
                            paints.push(Paint { kind: PaintKind::Image { image_ref, scale_mode }, opacity: 1.0, visible: true, blend: BlendMode::Normal });
                        }
                    } else {
                        report.unsupported_css.push(UnsupportedCssEntry {
                            element_id: el_id.to_string(),
                            property: "background-image".into(),
                            value: layer.into(),
                            note: "external background url not inlined".into(),
                        });
                    }
                } else {
                    report.unsupported_css.push(UnsupportedCssEntry {
                        element_id: el_id.to_string(),
                        property: "background-image".into(),
                        value: layer.into(),
                        note: "unsupported background-image function".into(),
                    });
                }
            }
        }
    }

    paints
}

pub fn split_top_level_commas(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut in_quote: Option<char> = None;
    let mut start = 0usize;
    for (idx, ch) in s.char_indices() {
        if let Some(q) = in_quote {
            if ch == q {
                in_quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => in_quote = Some(ch),
            '(' => depth += 1,
            ')' => depth = (depth - 1).max(0),
            ',' if depth == 0 => {
                parts.push(s[start..idx].trim());
                start = idx + 1;
            }
            _ => {}
        }
    }
    let tail = s[start..].trim();
    if !tail.is_empty() {
        parts.push(tail);
    }
    parts
}

fn parse_linear_gradient(inner: &str) -> Option<Paint> {
    let parts = split_top_level_commas(inner);
    if parts.is_empty() {
        return None;
    }
    let mut angle_deg = 180.0f32;
    let mut stop_start = 0usize;

    let first = parts[0].trim().to_ascii_lowercase();
    if let Some(dir) = first.strip_prefix("to ") {
        stop_start = 1;
        angle_deg = match dir.trim() {
            "top" => 0.0,
            "right" => 90.0,
            "bottom" => 180.0,
            "left" => 270.0,
            "top right" | "right top" => 45.0,
            "bottom right" | "right bottom" => 135.0,
            "bottom left" | "left bottom" => 225.0,
            "top left" | "left top" => 315.0,
            _ => 180.0,
        };
    } else if let Some(deg) = parse_angle_deg(&first) {
        if first.ends_with("deg") || first.ends_with("rad") || first.ends_with("turn") || first.ends_with("grad") {
            stop_start = 1;
            angle_deg = deg;
        }
    }

    let stops = parse_gradient_stops(&parts[stop_start..])?;
    let rad = (angle_deg - 90.0).to_radians();
    let (sin, cos) = rad.sin_cos();
    let start = Vec2::new(0.5 - 0.5 * cos, 0.5 - 0.5 * sin);
    let end = Vec2::new(0.5 + 0.5 * cos, 0.5 + 0.5 * sin);
    let width_handle = Vec2::new(start.x - 0.5 * sin, start.y + 0.5 * cos);

    Some(Paint { kind: PaintKind::LinearGradient { handles: [start, end, width_handle], stops }, opacity: 1.0, visible: true, blend: BlendMode::Normal })
}

fn parse_radial_gradient(inner: &str) -> Option<Paint> {
    let parts = split_top_level_commas(inner);
    if parts.is_empty() {
        return None;
    }
    let first_is_color = parse_gradient_stop(parts[0]).is_some();
    let stop_slice = if first_is_color { &parts[..] } else { &parts[1..] };
    let stops = parse_gradient_stops(stop_slice)?;
    Some(Paint {
        kind: PaintKind::RadialGradient { handles: [Vec2::new(0.5, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.5, 1.0)], stops },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    })
}

fn parse_gradient_stops(raw_stops: &[&str]) -> Option<Vec<GradientStop>> {
    if raw_stops.len() < 2 {
        return None;
    }
    let mut parsed: Vec<(Color, Option<f32>)> = Vec::with_capacity(raw_stops.len());
    for raw in raw_stops {
        parsed.push(parse_gradient_stop(raw)?);
    }
    let n = parsed.len();
    let mut stops = Vec::with_capacity(n);
    for (i, (color, pos)) in parsed.into_iter().enumerate() {
        let p = pos.unwrap_or_else(|| i as f32 / (n.saturating_sub(1).max(1) as f32));
        stops.push(GradientStop { pos: p.clamp(0.0, 1.0), color });
    }
    Some(stops)
}

fn parse_gradient_stop(raw: &str) -> Option<(Color, Option<f32>)> {
    let s = raw.trim();
    if let Some(idx) = s.rfind(|c: char| c.is_ascii_whitespace()) {
        let (color_part, pos_part) = s.split_at(idx);
        let pos_part = pos_part.trim();
        if let Some(pct) = pos_part.strip_suffix('%') {
            if let Ok(v) = pct.trim().parse::<f32>() {
                if let Some(c) = parse_css_color(color_part.trim()) {
                    return Some((c, Some((v / 100.0).clamp(0.0, 1.0))));
                }
            }
        }
    }
    let c = parse_css_color(s)?;
    Some((c, None))
}

/// Maps CSS `border-*-width`, `border-*-style`, and `border-*-color` to a [`Stroke`].
pub fn map_stroke(styles: &BTreeMap<String, String>, font_size: f32) -> Stroke {
    let side = |side_name: &str| -> (f32, &str, Option<Color>) {
        let style_key = format!("border-{side_name}-style");
        let width_key = format!("border-{side_name}-width");
        let color_key = format!("border-{side_name}-color");
        let st = styles.get(&style_key).map(|s| s.trim()).unwrap_or("none");
        if st.eq_ignore_ascii_case("none") || st.eq_ignore_ascii_case("hidden") {
            return (0.0, "none", None);
        }
        let w = styles.get(&width_key).and_then(|v| parse_css_px(v, font_size)).unwrap_or(0.0).max(0.0);
        let c = styles.get(&color_key).and_then(|v| parse_css_color(v));
        (w, st, c)
    };

    let (wt, st_t, ct) = side("top");
    let (wr, st_r, cr) = side("right");
    let (wb, st_b, cb) = side("bottom");
    let (wl, st_l, cl) = side("left");

    let max_w = wt.max(wr).max(wb).max(wl);
    if max_w <= 0.0 {
        return Stroke { weight: 1.0, miter_limit: 4.0, ..Default::default() };
    }

    let color = [ct, cr, cb, cl].into_iter().flatten().find(|c| c.a > 0.0).unwrap_or(Color::TRANSPARENT);
    if color.a <= 0.0 {
        return Stroke { weight: 1.0, miter_limit: 4.0, ..Default::default() };
    }

    let uniform = (wt - wr).abs() < 0.05 && (wt - wb).abs() < 0.05 && (wt - wl).abs() < 0.05;
    let active_style = [st_t, st_r, st_b, st_l].into_iter().find(|s| !s.eq_ignore_ascii_case("none")).unwrap_or("solid");
    let dashes = if active_style.eq_ignore_ascii_case("dashed") {
        vec![max_w * 3.0, max_w * 2.0]
    } else if active_style.eq_ignore_ascii_case("dotted") {
        vec![max_w, max_w]
    } else {
        Vec::new()
    };

    Stroke {
        paints: vec![Paint::solid(color)],
        weight: max_w,
        side_weights: if uniform { None } else { Some([wt, wr, wb, wl]) },
        align: StrokeAlign::Inside,
        cap: StrokeCap::None,
        join: StrokeJoin::Miter,
        miter_limit: 4.0,
        dashes,
    }
}

/// Maps CSS `border-*-radius` into `[tl, tr, br, bl]` corner radii clamped to half of `min(w, h)`.
pub fn map_corner_radii(styles: &BTreeMap<String, String>, w: f32, h: f32, font_size: f32) -> [f32; 4] {
    let basis = w.min(h).max(0.0);
    let max_r = basis * 0.5;
    let parse_r = |key: &str| -> f32 {
        let Some(raw) = styles.get(key) else { return 0.0 };
        let first = raw.split('/').next().unwrap_or(raw).split_whitespace().next().unwrap_or("");
        parse_css_px_or_percent(first, basis, font_size).unwrap_or(0.0).clamp(0.0, max_r)
    };
    [parse_r("border-top-left-radius"), parse_r("border-top-right-radius"), parse_r("border-bottom-right-radius"), parse_r("border-bottom-left-radius")]
}

/// Maps CSS `box-shadow`, `filter`, `backdrop-filter`, `mix-blend-mode`, and `opacity`.
pub fn map_effects_and_blend(el_id: &str, styles: &BTreeMap<String, String>, font_size: f32, report: &mut ImportReport) -> (Vec<Effect>, BlendMode, f32) {
    let mut effects = Vec::new();

    if let Some(bs) = styles.get("box-shadow") {
        let trimmed = bs.trim();
        if !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("none") {
            for item in split_top_level_commas(trimmed) {
                if let Some(eff) = parse_box_shadow_item(item, font_size) {
                    effects.push(eff);
                }
            }
        }
    }

    if let Some(filter) = styles.get("filter") {
        let trimmed = filter.trim();
        if !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("none") {
            if let Some(r) = extract_blur_fn(trimmed, font_size) {
                if r > 0.0 {
                    effects.push(Effect::LayerBlur { radius: r, visible: true });
                }
            } else {
                report.unsupported_css.push(UnsupportedCssEntry {
                    element_id: el_id.to_string(),
                    property: "filter".into(),
                    value: trimmed.into(),
                    note: "only blur() filter is mapped".into(),
                });
            }
        }
    }

    if let Some(bf) = styles.get("backdrop-filter").or_else(|| styles.get("-webkit-backdrop-filter")) {
        let trimmed = bf.trim();
        if !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("none") {
            if let Some(r) = extract_blur_fn(trimmed, font_size) {
                if r > 0.0 {
                    effects.push(Effect::BackgroundBlur { radius: r, visible: true });
                }
            } else {
                report.unsupported_css.push(UnsupportedCssEntry {
                    element_id: el_id.to_string(),
                    property: "backdrop-filter".into(),
                    value: trimmed.into(),
                    note: "only blur() backdrop-filter is mapped".into(),
                });
            }
        }
    }

    for prop in ["clip-path", "mask", "mask-image", "-webkit-mask-image"] {
        if let Some(val) = styles.get(prop) {
            let v = val.trim();
            if !v.is_empty() && !v.eq_ignore_ascii_case("none") {
                report.unsupported_css.push(UnsupportedCssEntry {
                    element_id: el_id.to_string(),
                    property: prop.into(),
                    value: v.into(),
                    note: "CSS masks/clip-paths are not imported in v0.6".into(),
                });
            }
        }
    }

    let blend = styles
        .get("mix-blend-mode")
        .map(|s| match s.trim().to_ascii_lowercase().as_str() {
            "multiply" => BlendMode::Multiply,
            "screen" => BlendMode::Screen,
            "overlay" => BlendMode::Overlay,
            "darken" => BlendMode::Darken,
            "lighten" => BlendMode::Lighten,
            "color-dodge" => BlendMode::ColorDodge,
            "color-burn" => BlendMode::ColorBurn,
            "hard-light" => BlendMode::HardLight,
            "soft-light" => BlendMode::SoftLight,
            "difference" => BlendMode::Difference,
            "exclusion" => BlendMode::Exclusion,
            "hue" => BlendMode::Hue,
            "saturation" => BlendMode::Saturation,
            "color" => BlendMode::Color,
            "luminosity" => BlendMode::Luminosity,
            _ => BlendMode::PassThrough,
        })
        .unwrap_or(BlendMode::PassThrough);

    let opacity = styles.get("opacity").and_then(|s| s.trim().parse::<f32>().ok()).map(|v| v.clamp(0.0, 1.0)).unwrap_or(1.0);

    (effects, blend, opacity)
}

fn extract_blur_fn(s: &str, font_size: f32) -> Option<f32> {
    let lower = s.to_ascii_lowercase();
    let idx = lower.find("blur(")?;
    let rest = &s[idx + 5..];
    let end = rest.find(')')?;
    parse_css_px(&rest[..end], font_size)
}

fn parse_box_shadow_item(item: &str, font_size: f32) -> Option<Effect> {
    let mut inset = false;
    let mut color: Option<Color> = None;
    let mut lengths: Vec<f32> = Vec::new();

    for tok in split_whitespace_preserving_parens(item) {
        if tok.eq_ignore_ascii_case("inset") {
            inset = true;
        } else if let Some(px) = parse_css_px(tok, font_size) {
            lengths.push(px);
        } else if let Some(c) = parse_css_color(tok) {
            color = Some(c);
        }
    }

    if lengths.len() < 2 {
        return None;
    }
    let ox = lengths[0];
    let oy = lengths[1];
    let blur = lengths.get(2).copied().unwrap_or(0.0).max(0.0);
    let spread = lengths.get(3).copied().unwrap_or(0.0);
    let c = color.unwrap_or(Color::rgba(0, 0, 0, 64));
    if c.a <= 0.0 {
        return None;
    }

    if inset {
        Some(Effect::InnerShadow { color: c, offset: Vec2::new(ox, oy), radius: blur, spread, visible: true, blend: Default::default() })
    } else {
        Some(Effect::DropShadow { color: c, offset: Vec2::new(ox, oy), radius: blur, spread, visible: true, show_behind: false, blend: Default::default() })
    }
}

fn split_whitespace_preserving_parens(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start: Option<usize> = None;
    for (idx, ch) in s.char_indices() {
        match ch {
            '(' => {
                if start.is_none() {
                    start = Some(idx);
                }
                depth += 1;
            }
            ')' => {
                depth = (depth - 1).max(0);
            }
            c if c.is_ascii_whitespace() && depth == 0 => {
                if let Some(st) = start.take() {
                    out.push(&s[st..idx]);
                }
            }
            _ => {
                if start.is_none() {
                    start = Some(idx);
                }
            }
        }
    }
    if let Some(st) = start {
        out.push(&s[st..]);
    }
    out
}

/// Maps CSS `transform` 2D `matrix(a, b, c, d, tx, ty)` or `rotate(...)` into a rotation in radians,
/// recording a note in `report.unsupported_css` if a scale, 3D, or skew transform is flattened (§3.3, §8.5).
pub fn map_rotation_rad(el_id: &str, styles: &BTreeMap<String, String>, report: &mut ImportReport) -> Option<f32> {
    let raw = styles.get("transform")?.trim();
    if raw.is_empty() || raw.eq_ignore_ascii_case("none") {
        return None;
    }
    let lower = raw.to_ascii_lowercase();
    if let Some(inner) = lower.strip_prefix("matrix(").and_then(|v| v.strip_suffix(')')) {
        let nums: Vec<f32> = inner.split(',').filter_map(|p| p.trim().parse::<f32>().ok()).collect();
        if nums.len() == 6 {
            let (a, b, c, d) = (nums[0], nums[1], nums[2], nums[3]);
            let skew = a * c + b * d;
            if skew.abs() > 1e-2 {
                report.unsupported_css.push(UnsupportedCssEntry {
                    element_id: el_id.to_string(),
                    property: "transform".into(),
                    value: raw.into(),
                    note: "2D skew flattened to bounding box rotation".into(),
                });
            }
            let scale_x = (a * a + b * b).sqrt();
            let scale_y = (c * c + d * d).sqrt();
            if (scale_x - 1.0).abs() > 1e-2 || (scale_y - 1.0).abs() > 1e-2 {
                report.unsupported_css.push(UnsupportedCssEntry {
                    element_id: el_id.to_string(),
                    property: "transform".into(),
                    value: raw.into(),
                    note: format!("transform scale({scale_x:.2}, {scale_y:.2}) applied via captured bounding box"),
                });
            }
            let rad = b.atan2(a);
            if rad.abs() > 1e-4 {
                return Some(rad);
            }
            return None;
        }
    }
    if let Some(inner) = lower.strip_prefix("rotate(").and_then(|v| v.strip_suffix(')')) {
        if let Some(deg) = parse_angle_deg(inner) {
            let rad = deg.to_radians();
            if rad.abs() > 1e-4 {
                return Some(rad);
            }
            return None;
        }
    }
    if lower.contains("scale(") || lower.contains("scalex(") || lower.contains("scaley(") || lower.contains("scale3d(") {
        report.unsupported_css.push(UnsupportedCssEntry {
            element_id: el_id.to_string(),
            property: "transform".into(),
            value: raw.into(),
            note: "transform scale applied via captured bounding box".into(),
        });
        return None;
    }
    if lower.starts_with("matrix3d(") || lower.contains("3d") || lower.contains("perspective") {
        report.unsupported_css.push(UnsupportedCssEntry {
            element_id: el_id.to_string(),
            property: "transform".into(),
            value: raw.into(),
            note: "3D transform flattened to captured 2D bounding box".into(),
        });
    }
    None
}

/// Builds a [`Node::transform`] placing a node at `(x, y)` with optional CSS rotation `rad` around its center.
pub fn node_transform_at(x: f32, y: f32, w: f32, h: f32, rad: Option<f32>) -> Affine {
    match rad {
        Some(r) if r.abs() > 1e-4 => {
            let cx = x + w * 0.5;
            let cy = y + h * 0.5;
            Affine::translate(cx, cy) * Affine::rotate(r) * Affine::translate(-w * 0.5, -h * 0.5)
        }
        _ => Affine::translate(x, y),
    }
}

/// Returns `true` if `styles` gives the element any visible background, border, or shadow.
pub fn has_own_box_paint(styles: &BTreeMap<String, String>) -> bool {
    if let Some(bg) = styles.get("background-color") {
        if let Some(c) = parse_css_color(bg) {
            if c.a > 0.0 {
                return true;
            }
        }
    }
    if let Some(bg_img) = styles.get("background-image") {
        let t = bg_img.trim();
        if !t.is_empty() && !t.eq_ignore_ascii_case("none") {
            return true;
        }
    }
    let stroke = map_stroke(styles, 16.0);
    if !stroke.paints.is_empty() && stroke.weight > 0.0 {
        return true;
    }
    if let Some(bs) = styles.get("box-shadow") {
        let t = bs.trim();
        if !t.is_empty() && !t.eq_ignore_ascii_case("none") {
            return true;
        }
    }
    false
}
