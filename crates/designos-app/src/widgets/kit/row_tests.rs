use super::*;
use crate::theme;
use egui::Color32;

#[test]
fn rows_follow_the_secondary_surface() {
    let states = [State::Selected, State::Pressed, State::Hover, State::Focused, State::Default, State::Disabled, State::Loading];
    for state in states {
        let list = row_look(state, Surface::Secondary);
        let (fill, text, icon) = list_secondary(state);
        assert_eq!(list.fill, fill, "list {state:?} fill");
        assert_eq!(list.text, text, "list {state:?} text");
        assert_eq!(list.icon, icon, "list {state:?} icon");
        let tree = tree_row_look(state, Surface::Secondary);
        let (fill, text, icon) = tree_secondary(state);
        assert_eq!(tree.fill, fill, "tree {state:?} fill");
        assert_eq!(tree.text, text, "tree {state:?} text");
        assert_eq!(tree.icon, icon, "tree {state:?} icon");
    }
}

fn list_secondary(state: State) -> (Color32, Color32, Color32) {
    match state {
        State::Hover => (theme::BG_SECONDARY_HOVER, theme::TEXT_SECONDARY_HOVER, theme::ICON_QUATERNARY_HOVER),
        State::Focused => (theme::BG, theme::TEXT_SECONDARY, theme::ICON_QUATERNARY),
        State::Selected | State::Pressed => (theme::BG_PRESSED, theme::TEXT_SECONDARY, theme::ICON_QUATERNARY_HOVER),
        _ => (Color32::TRANSPARENT, theme::TEXT_SECONDARY, theme::ICON_QUATERNARY),
    }
}

#[test]
fn command_row_splits_selected_from_hover() {
    let selected = command_row_look(true, false);
    assert_eq!(selected.fill, theme::BG_SELECTED);
    assert_eq!(selected.text, theme::TEXT);
    assert_eq!(selected.icon, theme::ICON);
    let hot = command_row_look(false, true);
    assert_eq!(hot.fill, theme::BG_MENU_HOVER);
    assert_eq!(hot.text, theme::TEXT_SECONDARY);
    assert_eq!(hot.icon, theme::ICON_TERTIARY);
    assert_ne!(hot.fill, theme::BG_SELECTED);
    let rest = command_row_look(false, false);
    assert_eq!(rest.fill, Color32::TRANSPARENT);
    assert_eq!(rest.text, theme::TEXT_SECONDARY);
}

fn tree_secondary(state: State) -> (Color32, Color32, Color32) {
    match state {
        State::Hover => (theme::BG_SECONDARY_HOVER, theme::TEXT_TERTIARY, theme::ICON_QUATERNARY),
        State::Selected | State::Pressed => (theme::BG_PRESSED, theme::TEXT_SECONDARY, theme::ICON_QUATERNARY_HOVER),
        _ => (Color32::TRANSPARENT, theme::TEXT_TERTIARY, theme::ICON_QUATERNARY),
    }
}
