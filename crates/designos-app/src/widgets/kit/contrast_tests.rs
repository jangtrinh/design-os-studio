use super::contrast_table::{exemptions, score};
use super::guard_tests::contrast;
use super::table_tests::{actual, doc, parse_state};
use super::*;
use crate::theme;

#[test]
fn every_state_meets_chrome_contrast() {
    let mut bad = Vec::new();
    let table = exemptions();
    let rows = doc()["rows"].as_object().expect("rows").clone();
    for (row, states) in &rows {
        for (name, cell) in states.as_object().expect("states") {
            if cell.is_null() {
                continue;
            }
            let state = parse_state(name);
            let (look, _) = actual(row, state);
            score(&mut bad, &table, row, state, look, theme::BG, "page");
            let on_secondary = match row.as_str() {
                "List/Row" => row_look(state, Surface::Secondary),
                "Tree/Row" => tree_row_look(state, Surface::Secondary),
                _ => look,
            };
            score(&mut bad, &table, row, state, on_secondary, theme::BG_SECONDARY, "secondary");
        }
    }
    for (row, tone) in [("Badge/Neutral", Tone::Neutral), ("Badge/Success", Tone::Success), ("Badge/Warning", Tone::Warning), ("Badge/Error", Tone::Error)] {
        score(&mut bad, &table, row, State::Default, count_badge_look(tone), theme::BG, "page");
    }
    for kind in [TileKind::Design, TileKind::Board, TileKind::Deck, TileKind::Spec, TileKind::Neutral] {
        score(&mut bad, &table, "Tile", State::Default, tile_look(kind), theme::BG, "page");
    }
    for tone in [DotTone::Info, DotTone::Success, DotTone::Error] {
        let dot = status_color(tone);
        for (surface, where_) in [(theme::BG, "page"), (theme::BG_SECONDARY, "secondary")] {
            let ratio = contrast(dot, surface);
            if ratio < 3.0 {
                bad.push(format!("dot {tone:?} {ratio:.2} {where_}"));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
}
