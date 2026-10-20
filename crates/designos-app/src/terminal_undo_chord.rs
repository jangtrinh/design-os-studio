//! Pure routing for document Undo / Redo keyboard chords while an agent terminal has focus (`#685`).
//!
//! - **macOS**: `Cmd+Z` undoes and `Shift+Cmd+Z` / `Cmd+Y` redoes the document in both agent and
//!   shell tabs. Raw `Ctrl+Z` (`mods.ctrl && !mods.command`) is never matched here, so it still
//!   reaches the PTY as `0x1A` (`SIGTSTP`).
//! - **Windows & Linux**: `Ctrl+Z` undoes and `Ctrl+Shift+Z` / `Ctrl+Y` redoes the document only
//!   when an **agent** tab is focused (`tab_is_agent == true`) and that CLI does not reserve
//!   `Ctrl+Z` (`cli_uses_ctrl_z == false`). In a `Shell` tab (`tab_is_agent == false`) or when
//!   `cli_uses_ctrl_z == true`, `Ctrl+Z` is left to the terminal.

use crate::panels::menus::Action;
use crate::terminal::AgentCliPreset;
use egui::{Key, Modifiers};

/// Per-CLI table recording whether each agent CLI uses `Ctrl+Z` on Windows and Linux (`#685` §3.3).
/// Until grounded per CLI, all four agent CLIs default to `true`.
pub const CLI_USES_CTRL_Z: &[(AgentCliPreset, bool)] =
    &[(AgentCliPreset::Claude, true), (AgentCliPreset::Codex, true), (AgentCliPreset::Antigravity, true), (AgentCliPreset::Grok, true)];

/// Returns `(tab_is_agent, cli_uses_ctrl_z)` for the given [`AgentCliPreset`].
pub fn preset_ctrl_z_policy(preset: AgentCliPreset) -> (bool, bool) {
    match preset {
        AgentCliPreset::Shell | AgentCliPreset::DesignOsHelp => (false, true),
        agent => {
            let uses = CLI_USES_CTRL_Z.iter().find_map(|&(p, u)| (p == agent).then_some(u)).unwrap_or(true);
            (true, uses)
        }
    }
}

/// Maps a key press while the terminal viewport captures the keyboard to a document
/// [`Action::Undo`] or [`Action::Redo`] when allowed by platform and tab rules (`#685` §5).
pub fn terminal_undo_chord(key: Key, mods: Modifiers, is_mac: bool, tab_is_agent: bool, cli_uses_ctrl_z: bool) -> Option<Action> {
    let chord_active = if is_mac {
        mods.command && !mods.ctrl && !mods.alt
    } else {
        tab_is_agent && !cli_uses_ctrl_z && (mods.command || mods.ctrl) && !mods.alt && !mods.mac_cmd
    };
    if !chord_active {
        return None;
    }
    match (key, mods.shift) {
        (Key::Z, false) => Some(Action::Undo),
        (Key::Z, true) | (Key::Y, false) => Some(Action::Redo),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_ctrl_z_undoes_in_an_agent_tab_when_the_cli_does_not_use_it() {
        let ctrl = Modifiers { ctrl: true, command: true, ..Modifiers::NONE };
        assert_eq!(terminal_undo_chord(Key::Z, ctrl, false, true, false), Some(Action::Undo));
        assert_eq!(terminal_undo_chord(Key::Z, ctrl | Modifiers::SHIFT, false, true, false), Some(Action::Redo));
        assert_eq!(terminal_undo_chord(Key::Y, ctrl, false, true, false), Some(Action::Redo));
    }

    #[test]
    fn windows_ctrl_z_is_left_to_a_shell_tab() {
        let ctrl = Modifiers { ctrl: true, command: true, ..Modifiers::NONE };
        assert_eq!(terminal_undo_chord(Key::Z, ctrl, false, false, false), None);
        assert_eq!(terminal_undo_chord(Key::Z, ctrl | Modifiers::SHIFT, false, false, false), None);
        assert_eq!(terminal_undo_chord(Key::Y, ctrl, false, false, false), None);
    }

    #[test]
    fn ctrl_z_is_left_to_a_cli_that_uses_it() {
        let ctrl = Modifiers { ctrl: true, command: true, ..Modifiers::NONE };
        for &(preset, uses_ctrl_z) in CLI_USES_CTRL_Z {
            assert!(uses_ctrl_z, "{preset:?} defaults to uses_ctrl_z = true");
            let (tab_is_agent, cli_uses) = preset_ctrl_z_policy(preset);
            assert!(tab_is_agent);
            assert!(cli_uses);
            assert_eq!(terminal_undo_chord(Key::Z, ctrl, false, tab_is_agent, cli_uses), None);
        }
    }

    #[test]
    fn mac_cmd_z_and_redo_chords_apply_in_agent_and_shell_tabs_while_ctrl_z_is_ignored() {
        let cmd = Modifiers::COMMAND;
        for tab_is_agent in [true, false] {
            for cli_uses_ctrl_z in [true, false] {
                assert_eq!(terminal_undo_chord(Key::Z, cmd, true, tab_is_agent, cli_uses_ctrl_z), Some(Action::Undo));
                assert_eq!(terminal_undo_chord(Key::Z, cmd | Modifiers::SHIFT, true, tab_is_agent, cli_uses_ctrl_z), Some(Action::Redo));
                assert_eq!(terminal_undo_chord(Key::Y, cmd, true, tab_is_agent, cli_uses_ctrl_z), Some(Action::Redo));
                assert_eq!(terminal_undo_chord(Key::Z, Modifiers::CTRL, true, tab_is_agent, cli_uses_ctrl_z), None);
            }
        }
    }
}
