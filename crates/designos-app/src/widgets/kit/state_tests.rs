use super::guard_tests::contrast;
use super::table_tests::{actual, doc, expect, parse_state};
use super::*;
use crate::theme;

#[test]
fn toast_action_hover_uses_the_secondary_hover_fill() {
    let rest = toast_action_look(State::Default);
    let hot = toast_action_look(State::Hover);
    assert_eq!(rest.text, theme::TEXT_SECONDARY);
    assert_eq!(rest.fill.a(), 0);
    assert_eq!(hot.fill, theme::BG_SECONDARY_HOVER);
    assert_ne!(hot.fill, theme::BG_HOVER);
    assert_eq!(hot.text, theme::TEXT_SECONDARY_HOVER);
}

#[test]
fn button_looks_match_spec_c_table() {
    let doc = doc();
    let rows = doc["rows"].as_object().expect("rows");
    for (row, states) in rows {
        if !row.starts_with("Button/") {
            continue;
        }
        for (name, cell) in states.as_object().expect("states") {
            if cell.is_null() {
                continue;
            }
            let (want, _) = expect(cell);
            let (got, _) = actual(row, parse_state(name));
            assert_eq!(got.fill, want.fill, "{row} {name} fill");
            assert_eq!(got.border, want.border, "{row} {name} border");
            assert_eq!(got.border_w, want.border_w, "{row} {name} width");
            assert_eq!(got.text, want.text, "{row} {name} text");
        }
    }
    let kinds = [
        Hierarchy::Primary,
        Hierarchy::Secondary,
        Hierarchy::Tertiary,
        Hierarchy::Destructive,
        Hierarchy::SecondaryDestructive,
        Hierarchy::Icon,
        Hierarchy::Tool,
        Hierarchy::Close,
    ];
    for kind in kinds {
        assert_eq!(button_look(kind, State::Loading), button_look(kind, State::Hover));
    }
}

#[test]
fn primary_label_is_dark_on_orange() {
    let label = button_look(Hierarchy::Primary, State::Default).text;
    assert_eq!(label, theme::TEXT_ON_BRAND);
    for fill in [theme::BG_BRAND, theme::BG_BRAND_HOVER, theme::BG_BRAND_PRESSED] {
        let ratio = contrast(label, fill);
        assert!(ratio >= 4.5, "primary label {ratio:.2} on fill");
    }
}

#[test]
fn disabled_is_half_alpha_of_default() {
    let doc = doc();
    let rows = doc["rows"].as_object().expect("rows");
    for (row, states) in rows {
        let Some(disabled_cell) = states.get("Disabled").filter(|cell| !cell.is_null()) else {
            continue;
        };
        let resting = &states["Default"];
        let (got, _) = actual(row, State::Disabled);
        let (base, _) = actual(row, State::Default);
        let same = ["fill", "border", "text", "icon"].iter().all(|key| disabled_cell[key] == resting[key]);
        if row == "Checkbox/Box" {
            assert!(!same, "Checkbox/Box disabled follows its own row");
            assert_ne!(got, disabled(base), "Checkbox/Box disabled");
        } else {
            assert!(same, "{row} disabled cell");
            assert_eq!(got, disabled(base), "{row} disabled");
        }
        for (key, got_ch) in [("fill", got.fill), ("border", got.border), ("text", got.text), ("icon", got.icon)] {
            let src = token_fill(disabled_cell, key);
            if src.a() != 255 {
                continue;
            }
            assert_eq!(got_ch.a(), 128, "{row} {key} alpha");
            assert_eq!(got_ch.r(), (u16::from(src.r()) * 128 / 255) as u8, "{row} {key} red");
            assert_eq!(got_ch.g(), (u16::from(src.g()) * 128 / 255) as u8, "{row} {key} green");
            assert_eq!(got_ch.b(), (u16::from(src.b()) * 128 / 255) as u8, "{row} {key} blue");
        }
        if let Some(hover) = states.get("Hover").filter(|cell| !cell.is_null()) {
            if hover != disabled_cell {
                let (hot, _) = actual(row, State::Hover);
                assert_ne!(got, hot, "{row} disabled picked up hover");
            }
        }
    }
}

fn token_fill(cell: &serde_json::Value, key: &str) -> egui::Color32 {
    match cell.get(key).and_then(|v| v.as_str()) {
        None => egui::Color32::TRANSPARENT,
        Some(name) => super::token_tests::token(name),
    }
}

#[test]
fn field_focus_border_is_two_px_border_brand() {
    let focused = field_look(State::Focused, false);
    assert_eq!(focused.border_w, theme::BORDER_WIDTH * 2.0);
    assert_eq!(focused.border, theme::BORDER_SELECTED);
    let invalid = field_look(State::Focused, true);
    assert_eq!(invalid.border, theme::FOCUS_RING_ERROR);
    assert_eq!(invalid.border_w, theme::BORDER_WIDTH * 2.0);
    assert_eq!(field_look(State::Default, true).border, theme::BORDER_STRONG);
}

#[test]
fn hover_uses_the_surface_hover_token() {
    assert_eq!(row_look(State::Hover, Surface::Primary).fill, theme::BG_HOVER);
    assert_eq!(row_look(State::Hover, Surface::Secondary).fill, theme::BG_SECONDARY_HOVER);
    assert_ne!(theme::BG_HOVER, theme::BG_SECONDARY_HOVER);
    assert_eq!(tree_row_look(State::Hover, Surface::Secondary).fill, theme::BG_SECONDARY_HOVER);
}

#[test]
fn destructive_states_differ() {
    let fill = |state| button_look(Hierarchy::Destructive, state).fill;
    assert_eq!(fill(State::Default), theme::BG_ERROR_SOLID);
    assert_eq!(fill(State::Hover), theme::BG_ERROR_SOLID_HOVER);
    assert_eq!(fill(State::Pressed), theme::BG_ERROR_SOLID_PRESSED);
    for state in [State::Default, State::Hover, State::Pressed] {
        let look = button_look(Hierarchy::Destructive, state);
        assert_eq!(look.text, theme::TEXT_WHITE);
        assert!(contrast(look.text, look.fill) >= 4.5, "destructive label");
    }
    assert_eq!(button_ring(Hierarchy::Destructive, true), theme::FOCUS_RING_ERROR);
    let secondary = |state| button_look(Hierarchy::SecondaryDestructive, state);
    assert_eq!(secondary(State::Default).fill, theme::BG);
    assert_eq!(secondary(State::Hover).fill, theme::BG_ERROR_PRIMARY);
    assert_eq!(secondary(State::Pressed).fill, theme::BG_ERROR_PRIMARY);
    assert_eq!(secondary(State::Pressed).border, theme::BORDER_ERROR);
    for state in [State::Default, State::Hover, State::Pressed] {
        let look = secondary(state);
        assert!(contrast(look.text, look.fill) >= 4.5, "secondary destructive label");
    }
    assert_eq!(button_ring(Hierarchy::SecondaryDestructive, true), theme::FOCUS_RING_ERROR);
}

#[test]
fn menu_item_pitch_matches_size() {
    assert_eq!(menu_item_height(Size::Xs), theme::CONTROL_XS);
    assert_eq!(menu_item_height(Size::Sm), theme::CONTROL_SM);
    assert_eq!(menu_item_height(Size::Md), theme::CONTROL_MD);
    assert_eq!(theme::CONTROL_XS, 32.0);
    assert_eq!(theme::CONTROL_SM, 36.0);
    assert_eq!(MENU_INSET, theme::INSET_FRAME);
    assert_eq!(MENU_INSET, 8.0);
    assert_eq!(PILL_RADIUS, theme::RADIUS_CONTROL);
    assert_eq!(PILL_RADIUS, 8);
    assert_eq!(MENU_RADIUS, theme::RADIUS_FRAME);
    assert_eq!(MENU_RADIUS, 16);
}

#[test]
fn tab_heights_stay_as_today() {
    let tabs = include_str!("tabs.rs");
    assert!(!tabs.contains("FILE_TAB_H"));
    assert!(!tabs.contains("BG_SECONDARY_HOVER"));
    assert!(tabs.contains("BG_SELECTED"));
    assert_eq!(SEGMENTED_STRIP_H, theme::RIGHT_TABS_H);
    assert_eq!(segment_tile_h(), theme::RIGHT_TABS_H - theme::INSET_SEGMENT * 2.0);
    assert_eq!(FILE_TAB_BAR_H, theme::CONTROL_XS);
    assert_eq!(SESSION_TAB_H, theme::RIGHT_TABS_H);
    assert_eq!(HOME_TAB_H, theme::CONTROL_SM);
    assert_eq!(segmented_look(State::Selected).fill, theme::BG_SELECTED);
}
