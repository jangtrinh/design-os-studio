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
    match row {
        "Button/Primary" => (button_look(Hierarchy::Primary, state), button_ring(Hierarchy::Primary, state)),
        "Button/Secondary" => (button_look(Hierarchy::Secondary, state), button_ring(Hierarchy::Secondary, state)),
        "Button/Tertiary" => (button_look(Hierarchy::Tertiary, state), button_ring(Hierarchy::Tertiary, state)),
        "Button/Destructive" => (button_look(Hierarchy::Destructive, state), button_ring(Hierarchy::Destructive, state)),
        "Button/SecondaryDestructive" => (button_look(Hierarchy::SecondaryDestructive, state), button_ring(Hierarchy::SecondaryDestructive, state)),
        "Button/Icon" => (button_look(Hierarchy::Icon, state), button_ring(Hierarchy::Icon, state)),
        "Button/Tool" => (button_look(Hierarchy::Tool, state), button_ring(Hierarchy::Tool, state)),
        "Button/Close" => (button_look(Hierarchy::Close, state), button_ring(Hierarchy::Close, state)),
        "Input/Field" => (field_look(state, false), clear),
        "Input/PropertyField" => (property_field_look(state), clear),
        "Input/Search" => (search_look(state), clear),
        "Select/Trigger" => (select_look(state), clear),
        "Menu/Panel" => (menu_panel_look(), clear),
        "Menu/Context" => (menu_context_look(), clear),
        "Menu/Item" => (menu_item_look(state), clear),
        "Menu/Shortcut" => (shortcut_look(state), clear),
        "Menu/Separator" => (separator_look(), clear),
        "Tabs/Segmented" => (segmented_look(state), segmented_ring(state)),
        "Tabs/Pill" => (tab_look(state), tab_ring(state)),
        "Toggle/Switch" => (toggle_look(state), toggle_ring(state)),
        "Checkbox/Box" => (checkbox_look(state), checkbox_ring(state)),
        "Radio/Card" => (radio_card_look(state), radio_ring(state)),
        "Badge/StatusCount" => (count_badge(Tone::Neutral), clear),
        "Tag/Chip" => (tag_look(state), tag_ring(state)),
        "Tooltip/Default" => (tooltip_look(), clear),
        "Tree/Row" => (tree_row_look(state, Surface::Primary), clear),
        "List/Row" => (row_look(state, Surface::Primary), list_row_ring(state)),
        "Panel/Header" => (panel_header_look(), clear),
        "Section/Header" => (section_header_look(state), clear),
        "Dialog/Modal" => (modal_look(), clear),
        "Popover/Panel" => (popover_look(), clear),
        "Dialog/CommandPalette" => (command_look(), clear),
        "Toast/Default" => (toast_look(), clear),
        "Swatch/Color" => (swatch_look(state), swatch_ring(state)),
        "Slider/Handle" => (slider_handle_look(state), slider_ring(state)),
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
    for name in constants.keys() {
        let _ = token(name);
    }
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
