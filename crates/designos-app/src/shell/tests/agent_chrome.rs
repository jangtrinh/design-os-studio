//! Agents panel chrome: palette aliases, card height, and colour census.

use super::chrome_census::chrome_colour_sites;

#[test]
fn terminal_palette_aliases_contract_roles() {
    use crate::terminal::{
        STATUS_BAD_BG, STATUS_BAD_BORDER, STATUS_BAD_TEXT, STATUS_INFO_BG, STATUS_INFO_BORDER, STATUS_INFO_TEXT, STATUS_OK_BG, STATUS_OK_BORDER,
        STATUS_OK_TEXT, STATUS_WARN_BG, STATUS_WARN_BORDER, STATUS_WARN_TEXT, TERM_BG_BAR, TERM_BG_ELEVATED, TERM_BG_HOVER, TERM_BG_VIEWPORT, TERM_BORDER,
        TERM_ORANGE,
    };
    assert_eq!(TERM_BG_VIEWPORT, crate::theme::BG);
    assert_eq!(TERM_BG_BAR, crate::theme::BG_PRIMARY);
    assert_eq!(TERM_BG_ELEVATED, crate::theme::BG_SECONDARY);
    assert_eq!(TERM_BG_HOVER, crate::theme::BG_HOVER);
    assert_eq!(TERM_BORDER, crate::theme::BORDER);
    assert_eq!(TERM_ORANGE, crate::theme::BORDER_SELECTED);
    assert_eq!(STATUS_INFO_TEXT, crate::theme::STATUS_INFO_TEXT);
    assert_eq!(STATUS_INFO_BG, crate::theme::STATUS_INFO_BG);
    assert_eq!(STATUS_INFO_BORDER, crate::theme::STATUS_INFO_BORDER);
    assert_eq!(STATUS_OK_BG, crate::theme::BG_SUCCESS_PRIMARY);
    assert_eq!(STATUS_OK_BORDER, crate::theme::BADGE_SUCCESS_BORDER);
    assert_eq!(STATUS_OK_TEXT, crate::theme::BADGE_SUCCESS_TEXT);
    assert_eq!(STATUS_WARN_BG, crate::theme::BG_WARNING_PRIMARY);
    assert_eq!(STATUS_WARN_BORDER, crate::theme::BADGE_WARNING_BORDER);
    assert_eq!(STATUS_WARN_TEXT, crate::theme::BADGE_WARNING_TEXT);
    assert_eq!(STATUS_BAD_BG, crate::theme::BG_ERROR_PRIMARY);
    assert_eq!(STATUS_BAD_BORDER, crate::theme::BADGE_ERROR_BORDER);
    assert_eq!(STATUS_BAD_TEXT, crate::theme::BADGE_ERROR_TEXT);
}

#[test]
fn terminal_content_colours_unchanged() {
    assert_eq!(crate::terminal::TERM_FG_DEFAULT, egui::Color32::from_rgb(0xE4, 0xE4, 0xE7));
    assert_eq!(crate::terminal::TERM_SELECTION_BG, egui::Color32::from_rgb(0x5A, 0x78, 0x98));
}

#[test]
fn approval_card_height_follows_control_xs() {
    use crate::terminal::AgentCliPreset;
    assert_eq!(crate::agent_approval_card::approval_card_outer_height(AgentCliPreset::Claude), 136.0);
    for preset in [AgentCliPreset::Codex, AgentCliPreset::Grok, AgentCliPreset::Gemini, AgentCliPreset::Antigravity, AgentCliPreset::Shell] {
        assert_eq!(crate::agent_approval_card::approval_card_outer_height(preset), 96.0, "{preset:?}");
    }
}

fn allowlisted_terminal_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.contains("TERM_FG_DEFAULT")
        || trimmed.contains("TERM_SELECTION_BG")
        || trimmed.contains("0x5C, 0x4A, 0x00")
        || trimmed.contains("0x1D, 0x1F, 0x21")
        || trimmed.contains("0xA1, 0xA1, 0xAA")
        || (trimmed.contains("Color32::WHITE") && (trimmed.contains("cursor") || trimmed.contains("CURSOR") || trimmed.contains("caret")))
}

#[test]
fn agent_sources_paint_no_chrome_colour_literals() {
    let files: &[(&str, &str)] = &[
        ("terminal.rs", include_str!("../../terminal.rs")),
        ("agent_approval_card.rs", include_str!("../../agent_approval_card.rs")),
        ("agent_composer.rs", include_str!("../../agent_composer.rs")),
        ("agent_composer_chips.rs", include_str!("../../agent_composer_chips.rs")),
        ("agent_composer_menus.rs", include_str!("../../agent_composer_menus.rs")),
        ("agent_composer_toolbar.rs", include_str!("../../agent_composer_toolbar.rs")),
        ("agent_header.rs", include_str!("../../agent_header.rs")),
        ("agent_mention_popup.rs", include_str!("../../agent_mention_popup.rs")),
        ("agent_open_flow.rs", include_str!("../../agent_open_flow.rs")),
        ("agent_picker.rs", include_str!("../../agent_picker.rs")),
        ("agent_selection_chip.rs", include_str!("../../agent_selection_chip.rs")),
        ("agent_session_failure.rs", include_str!("../../agent_session_failure.rs")),
        ("agent_start_failure.rs", include_str!("../../agent_start_failure.rs")),
        ("agent_stop.rs", include_str!("../../agent_stop.rs")),
        ("agent_turn_card.rs", include_str!("../../agent_turn_card.rs")),
        ("agent_presence.rs", include_str!("../../agent_presence.rs")),
        ("agent_busy_frames.rs", include_str!("../../agent_busy_frames.rs")),
        ("canvas/agent_banner.rs", include_str!("../../canvas/agent_banner.rs")),
        ("canvas/agent_changes_chip.rs", include_str!("../../canvas/agent_changes_chip.rs")),
        ("canvas/agent_overlay.rs", include_str!("../../canvas/agent_overlay.rs")),
        ("skills/mod.rs", include_str!("../../skills/mod.rs")),
        ("skills/picker_popover.rs", include_str!("../../skills/picker_popover.rs")),
        ("skills/pills.rs", include_str!("../../skills/pills.rs")),
        ("image_jobs.rs", include_str!("../../image_jobs.rs")),
    ];
    let mut left = Vec::new();
    for (rel, src) in files {
        let cleaned = super::chrome_census::clean_rust(src);
        let lines: Vec<&str> = cleaned.lines().collect();
        for site in chrome_colour_sites(src) {
            let line = lines.get(site - 1).copied().unwrap_or("");
            if *rel == "terminal.rs" && allowlisted_terminal_line(line) {
                continue;
            }
            left.push(format!("{rel}:{site}:{line}"));
        }
    }
    assert!(left.is_empty(), "chrome colour sites remain:\n{}", left.join("\n"));
}
