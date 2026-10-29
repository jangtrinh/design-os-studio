use super::table_tests::{actual, doc, parse_state};
use super::*;
use crate::theme;
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

fn backdrop(fill: Color32, surface: Color32) -> Color32 {
    if fill.a() == 255 {
        fill
    } else {
        surface
    }
}

fn quiet_boundary(c: Color32) -> bool {
    c == Color32::TRANSPARENT
        || c.a() < 255
        || c == theme::BG
        || c == theme::BG_SECONDARY
        || c == theme::BORDER
        || c == theme::BORDER_MENU
        || c == theme::BORDER_STRONG
        || c == theme::BADGE_NEUTRAL_BORDER
        || c == theme::BADGE_SUCCESS_BORDER
        || c == theme::BADGE_WARNING_BORDER
        || c == theme::BADGE_ERROR_BORDER
}

fn score(row: &str, state: State, look: Look, surface: Color32) {
    if state == State::Disabled {
        return;
    }
    let bg = backdrop(look.fill, surface);
    if look.text != Color32::TRANSPARENT && look.text != theme::TEXT_PLACEHOLDER {
        let ratio = contrast(look.text, bg);
        assert!(ratio >= 4.5, "{row} {state:?} text {ratio:.2}");
    }
    if look.icon != Color32::TRANSPARENT && !quiet_boundary(look.icon) {
        let ratio = contrast(look.icon, bg);
        assert!(ratio >= 3.0, "{row} {state:?} icon {ratio:.2}");
    }
    if look.border != Color32::TRANSPARENT && look.border != look.fill && !quiet_boundary(look.border) {
        let ratio = contrast(look.border, bg);
        assert!(ratio >= 3.0, "{row} {state:?} border {ratio:.2}");
    }
    if look.mark.a() == 255 && look.icon.a() == 255 {
        let ratio = contrast(look.mark, look.icon);
        assert!(ratio >= 3.0, "{row} {state:?} mark {ratio:.2}");
    }
}

#[test]
fn every_state_meets_c1_contrast() {
    let doc = doc();
    let rows = doc["rows"].as_object().expect("rows");
    for (row, states) in rows {
        for (name, cell) in states.as_object().expect("states") {
            if cell.is_null() {
                continue;
            }
            let state = parse_state(name);
            let (look, _) = actual(row, state);
            score(row, state, look, theme::BG);
            let on_secondary = match row.as_str() {
                "List/Row" => row_look(state, Surface::Secondary),
                "Tree/Row" => tree_row_look(state, Surface::Secondary),
                _ => look,
            };
            score(row, state, on_secondary, theme::BG_SECONDARY);
        }
    }
    for tone in [Tone::Neutral, Tone::Success, Tone::Warning, Tone::Error] {
        score("Badge", State::Default, count_badge(tone), theme::BG);
    }
    for kind in [TileKind::Design, TileKind::Board, TileKind::Deck, TileKind::Spec, TileKind::Neutral] {
        score("Tile", State::Default, tile_icon(kind), theme::BG);
    }
    for tone in [DotTone::Info, DotTone::Success, DotTone::Error] {
        let dot = status_dot(tone);
        assert!(contrast(dot, theme::BG) >= 3.0, "dot on page");
        assert!(contrast(dot, theme::BG_SECONDARY) >= 3.0, "dot on secondary");
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
        let look = count_badge(tone);
        colors.extend([look.fill, look.border, look.text]);
    }
    for kind in [TileKind::Design, TileKind::Board, TileKind::Deck, TileKind::Spec, TileKind::Neutral] {
        let look = tile_icon(kind);
        colors.extend([look.fill, look.icon, look.border]);
    }
    for tone in [DotTone::Info, DotTone::Success, DotTone::Error] {
        colors.push(status_dot(tone));
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
