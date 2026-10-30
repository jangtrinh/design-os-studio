use super::token_tests::token;
use super::*;
use crate::theme;
use egui::Color32;

const TABLE: &str = include_str!("testdata/kit-states.json");

pub(super) fn doc() -> serde_json::Value {
    serde_json::from_str(TABLE).expect("kit state table")
}

fn paint(cell: &serde_json::Value, key: &str, alpha: f64) -> Color32 {
    match cell.get(key).and_then(|v| v.as_str()) {
        None => Color32::TRANSPARENT,
        Some(name) => {
            let color = token(name);
            if alpha < 1.0 {
                half_color(color)
            } else {
                color
            }
        }
    }
}

pub(super) fn parse_hex(text: &str) -> Color32 {
    let hex = text.trim_start_matches('#');
    let packed = u32::from_str_radix(hex, 16).unwrap_or_else(|_| panic!("stopped, bad hex {text}"));
    if hex.len() == 6 {
        Color32::from_rgba_premultiplied((packed >> 16) as u8, (packed >> 8) as u8, packed as u8, 255)
    } else if hex.len() == 8 {
        let alpha = (packed & 255) as u8;
        let scale = u32::from(alpha);
        let body = packed >> 8;
        let ch = |shift: u32| (((body >> shift) & 255) * scale / 255) as u8;
        Color32::from_rgba_premultiplied(ch(16), ch(8), ch(0), alpha)
    } else {
        panic!("stopped, bad hex {text}");
    }
}

pub(super) fn parse_state(name: &str) -> State {
    match name {
        "Default" => State::Default,
        "Hover" => State::Hover,
        "Pressed" => State::Pressed,
        "Focused" => State::Focused,
        "Disabled" => State::Disabled,
        "Selected" => State::Selected,
        other => panic!("stopped, unknown state {other}"),
    }
}

pub(super) fn expect(cell: &serde_json::Value) -> (Look, Color32) {
    let alpha = cell["alpha"].as_f64().expect("alpha");
    let look = Look {
        fill: paint(cell, "fill", alpha),
        border: paint(cell, "border", alpha),
        border_w: cell["border_w"].as_f64().expect("border_w") as f32,
        text: paint(cell, "text", alpha),
        icon: paint(cell, "icon", alpha),
        mark: Color32::TRANSPARENT,
    };
    let ring = match cell.get("ring").and_then(|v| v.as_str()) {
        None => Color32::TRANSPARENT,
        Some(name) => token(name),
    };
    (look, ring)
}

pub(super) fn actual(row: &str, state: State) -> (Look, Color32) {
    let clear = Color32::TRANSPARENT;
    let focused = state == State::Focused;
    match row {
        "Button/Primary" => (button_look(Hierarchy::Primary, state), button_ring(Hierarchy::Primary, focused)),
        "Button/Secondary" => (button_look(Hierarchy::Secondary, state), button_ring(Hierarchy::Secondary, focused)),
        "Button/Tertiary" => (button_look(Hierarchy::Tertiary, state), button_ring(Hierarchy::Tertiary, focused)),
        "Button/Destructive" => (button_look(Hierarchy::Destructive, state), button_ring(Hierarchy::Destructive, focused)),
        "Button/SecondaryDestructive" => (button_look(Hierarchy::SecondaryDestructive, state), button_ring(Hierarchy::SecondaryDestructive, focused)),
        "Button/Icon" => (button_look(Hierarchy::Icon, state), button_ring(Hierarchy::Icon, focused)),
        "Button/Tool" => (button_look(Hierarchy::Tool, state), button_ring(Hierarchy::Tool, focused)),
        "Button/Close" => (button_look(Hierarchy::Close, state), button_ring(Hierarchy::Close, focused)),
        "Input/Field" => (field_look(state, false), clear),
        "Input/PropertyField" => (property_field_look(state), clear),
        "Input/Search" => (search_look(state), clear),
        "Select/Trigger" => (select_look(state), clear),
        "Menu/Panel" => (menu_panel_look(), clear),
        "Menu/Context" => (menu_context_look(), clear),
        "Menu/Item" => (menu_item_look(state), clear),
        "Menu/Shortcut" => (shortcut_look(state), clear),
        "Menu/Separator" => (separator_look(), clear),
        "Tabs/Segmented" => (segmented_look(state), segmented_ring(focused)),
        "Tabs/Pill" => (tab_look(state), tab_ring(focused)),
        "Toggle/Switch" => (toggle_look(state), toggle_ring(focused)),
        "Checkbox/Box" => (checkbox_look(state), checkbox_ring(focused)),
        "Radio/Card" => (radio_card_look(state), radio_ring(focused)),
        "Badge/StatusCount" => (count_badge_look(Tone::Neutral), clear),
        "Tag/Chip" => (tag_look(state), tag_ring(focused)),
        "Tooltip/Default" => (tooltip_look(), clear),
        "Tree/Row" => (tree_row_look(state, Surface::Primary), clear),
        "List/Row" => (row_look(state, Surface::Primary), list_row_ring(focused)),
        "Panel/Header" => (panel_header_look(), clear),
        "Section/Header" => (section_header_look(state), clear),
        "Dialog/Modal" => (modal_look(), clear),
        "Popover/Panel" => (popover_look(), clear),
        "Dialog/CommandPalette" => (command_look(), clear),
        "Toast/Default" => (toast_look(), clear),
        "Swatch/Color" => (swatch_look(state), swatch_ring_color(focused)),
        "Slider/Handle" => (slider_handle_look(state), slider_ring(focused)),
        "Progress/Bar" => (progress_look(), clear),
        "Loading/Inline" => (loading_look(), clear),
        "EmptyState/Default" => (empty_state_look(), clear),
        "Divider/Hairline" => (divider_look(), clear),
        other => panic!("stopped, no look for {other}"),
    }
}

#[test]
fn every_table_row_matches_its_look() {
    let doc = doc();
    let constants = doc["constants"].as_object().expect("constants");
    assert_eq!(constants.len(), 45, "constant table");
    for (name, value) in constants {
        let hex = value.as_str().expect("hex");
        assert_eq!(token(name), parse_hex(hex), "{name}");
    }
    assert_eq!(constants["BORDER_STRONG"].as_str(), Some("#404040"));
    assert_eq!(theme::BORDER_STRONG, parse_hex("#404040"));
    let rows = doc["rows"].as_object().expect("rows");
    assert_eq!(rows.len(), 39, "widget rows");
    for (row, states) in rows {
        for (name, cell) in states.as_object().expect("states") {
            if cell.is_null() {
                continue;
            }
            for key in ["fill", "border", "text", "icon", "ring"] {
                if let Some(value) = cell.get(key).and_then(|v| v.as_str()) {
                    assert!(!value.contains('-'), "stopped, role name {value} on {row}");
                }
            }
            let state = parse_state(name);
            let (mut want, want_ring) = expect(cell);
            if row == "Radio/Card" && state == State::Selected {
                want.mark = theme::TEXT_ON_BRAND;
            }
            let (got, got_ring) = actual(row, state);
            assert_eq!(got, want, "{row} {name}");
            assert_eq!(got_ring, want_ring, "{row} {name} ring");
        }
    }
}
