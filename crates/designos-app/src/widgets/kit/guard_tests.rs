use super::table_tests::{actual, doc, parse_state};
use super::*;
use egui::Color32;

pub(super) fn contrast(a: Color32, b: Color32) -> f64 {
    let ratio = (lum(a) + 0.05) / (lum(b) + 0.05);
    ratio.max(1.0 / ratio)
}

fn lum(c: Color32) -> f64 {
    let (r, g, b) = straight(c);
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

fn straight(c: Color32) -> (f64, f64, f64) {
    let a = u16::from(c.a()).max(1);
    let ch = |v: u8| f64::from(u16::from(v) * 255 / a) / 255.0;
    (ch(c.r()), ch(c.g()), ch(c.b()))
}

fn lin(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[test]
fn no_purple_in_kit() {
    let mut colors = Vec::new();
    let rows = doc()["rows"].as_object().expect("rows").clone();
    for (row, states) in &rows {
        for (name, cell) in states.as_object().expect("states") {
            if cell.is_null() {
                continue;
            }
            let (look, ring) = actual(row, parse_state(name));
            colors.extend([look.fill, look.border, look.text, look.icon, look.mark, ring]);
        }
    }
    for tone in [Tone::Neutral, Tone::Success, Tone::Warning, Tone::Error] {
        let look = count_badge_look(tone);
        colors.extend([look.fill, look.border, look.text]);
    }
    for kind in [TileKind::Design, TileKind::Board, TileKind::Deck, TileKind::Spec, TileKind::Neutral] {
        let look = tile_look(kind);
        colors.extend([look.fill, look.icon, look.border]);
    }
    for tone in [DotTone::Info, DotTone::Success, DotTone::Error] {
        colors.push(status_color(tone));
    }
    for color in colors {
        let (hue, sat) = hue_sat(color);
        let purple = (250.0..=300.0).contains(&hue) && sat > 0.25;
        assert!(!purple, "purple hue {hue:.0} sat {sat:.2}");
    }
}

fn hue_sat(c: Color32) -> (f64, f64) {
    if c.a() == 0 {
        return (0.0, 0.0);
    }
    let (r, g, b) = straight(c);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let sat = if max == 0.0 { 0.0 } else { d / max };
    let hue = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / d) % 6.0)
    } else if max == g {
        60.0 * (((b - r) / d) + 2.0)
    } else {
        60.0 * (((r - g) / d) + 4.0)
    };
    let hue = if hue < 0.0 { hue + 360.0 } else { hue };
    (hue, sat)
}

#[test]
fn kit_files_hold_no_hex_literals() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/src/widgets/kit");
    for entry in std::fs::read_dir(root).expect("kit dir") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("read");
        let rgb_call = ["rg", "b("].concat();
        assert!(!text.contains(&rgb_call), "{path:?} has a colour call");
        let bytes = text.as_bytes();
        let mut i = 0;
        while i + 8 <= bytes.len() {
            if bytes[i] == b'0' && bytes[i + 1] == b'x' && bytes[i + 2..i + 8].iter().all(u8::is_ascii_hexdigit) {
                panic!("hex literal in {path:?}");
            }
            i += 1;
        }
    }
}
