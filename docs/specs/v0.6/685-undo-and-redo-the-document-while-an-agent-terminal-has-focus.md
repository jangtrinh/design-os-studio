# #685 Cmd+Z and Shift+Cmd+Z undo document edits while an agent terminal has focus

| Field | Value |
|---|---|
| Issue | #685 (parent #585) |
| Size | S |
| Depends on | none (granularity follows #589 when it lands) |
| Unblocks | none |
| Touches | `crates/designos-app/src/shell.rs` (`handle_keys` `:482-492`), a small pure function in a new `crates/designos-app/src/terminal_undo_chord.rs`, shell tests in `crates/designos-app/src/shell/tests/` |

Code baseline: `origin/main@9f3f7a5`. The issue cites `shell.rs:482-485` and `terminal.rs:3520-3585`; on this commit the early return is at `shell.rs:487-491` and the terminal key map is at `terminal.rs:3699-3790`.

## 1. Summary and user story

As a designer watching an agent work in its terminal, I want Cmd+Z to undo the agent's last edit without first clicking the canvas, and Shift+Cmd+Z to redo it.

## 2. Current state (FACT)

- While the terminal viewport has keyboard focus (`TerminalState::viewport_focused`) and the Agents panel or the dock is open, `is_capturing_keyboard` is true (`terminal.rs:999-1001`). `Shell::handle_keys` then returns before any shortcut runs (`shell.rs:487-491`), so Cmd+Z does nothing.
- The undo and redo chords are `Key::Z` with `command` (undo), `Key::Z` with `command` and shift, and `Key::Y` with `command` (redo) (`shell.rs:618-619`). On macOS `command` is Cmd; on Windows and Linux egui sets it equal to Ctrl (`egui-0.35.0/src/data/input/modifiers.rs:33-37`).
- The terminal key map has no `Z` in any branch: Cmd (`terminal.rs:3724-3741`), Alt (`:3742-3748`), Ctrl (`:3749-3766`). Cmd+Z is not sent to the CLI.
- Divergence from the issue: no Ctrl+Z reaches the shell today on any platform. The Ctrl branch has no `Z` (so `0x1a`, the suspend key, is never written), and on Windows and Linux Ctrl takes the Cmd branch first. The issue's "Ctrl+Z inside the terminal stays the shell's suspend key" describes a behaviour the code does not have.
- Agent edits are one undo step per patch (`live_patch_apply.rs:112-132`). `App::undo` and `redo` are the actions behind `Action::Undo` and `Action::Redo`.
- UNKNOWN: whether each agent CLI binds Ctrl+Z (Claude Code is believed to use it to suspend on Unix). Check: each CLI's own docs or help, recorded in the table of rule 3.

## 3. Behaviour rules

1. **macOS.** With an agent or shell tab focused, Cmd+Z undoes the document and Shift+Cmd+Z (and Cmd+Y) redoes it. Cmd+Z is not a terminal key on macOS, so nothing clashes.
2. **Windows and Linux.** With an **agent** tab focused, Ctrl+Z undoes and Ctrl+Y or Ctrl+Shift+Z redoes, unless the CLI's row in the table below says it uses Ctrl+Z. With a **shell** tab focused, Ctrl+Z and Ctrl+Y are left to the shell: the document is not touched.
3. **Table** (data, one row per CLI): `uses_ctrl_z`. Claude: not grounded, default `true`. Codex, Grok, Antigravity: not grounded, default `true`. A `true` row on Windows and Linux makes Ctrl+Z do nothing for that tab, as it does today; flipping a row to `false` needs a captured check with that CLI. This keeps the change safe where the answer is unknown, and fully effective on macOS.
4. The undo is the app's own undo: same history, same granularity (one step per agent tool call, per turn once #589 lands), same republish of the canvas to the agent (`publish_live_context`).
5. The chord is ignored while a user edit is in progress (`user_edit_in_progress`), as the canvas shortcut is.
6. The terminal does not receive the chord: nothing is written to the PTY.
7. Document the rule in the shortcuts list (the in-app list and `docs`), with the Windows and Linux exception.

## 4. Data model / schema changes

None. The table lives in `terminal_undo_chord.rs` as a constant.

## 5. Contracts

`fn terminal_undo_chord(key: Key, mods: Modifiers, is_mac: bool, tab_is_agent: bool, cli_uses_ctrl_z: bool) -> Option<Action>` (pure, tested). `handle_keys` calls it before the early return.

## 6. File-format mapping

None.

## 7. UI

No new control. The shortcuts list gains the line "Undo / Redo work while an agent terminal has focus (macOS; Windows and Linux for agent tabs)". Evidence: a terminal capture (the PR shows the document before the keypress, after Cmd+Z, and after Shift+Cmd+Z, from a stub agent edit) at 1440. Responsive: SKIPPED (owner default). The shortcuts-list line is text only; es-designer is loaded if the list's layout changes.

## 8. Edge cases

1. The terminal search bar is open (Cmd+F): the chord still undoes the document; the search field keeps its own text undo while it has the text focus (`egui_wants_keyboard_input`).
2. Nothing to undo: the app's normal no-op.
3. The agent writes again right after the undo: the new write is a new step; the user can undo again. #677's Revert is the tool for a whole turn.
4. Shift+Cmd+Z with no redo step: no-op.
5. A key repeat held down: each repeat undoes one step, as on the canvas.
6. A dead agent tab: still an agent tab for this rule.

## 9. Non-goals

- Forwarding Ctrl keys correctly to the PTY on Windows and Linux (the `command` equals `ctrl` branch order at `terminal.rs:3724` is a separate defect; see README-part4 follow-ups).
- Text undo inside the CLI. A new undo scope.

## 10. Test plan (red first)

Seams: the pure function; a `Sim` test with the terminal focused and a live patch applied from a stub log.

| Case | Expected | Fails before |
|---|---|---|
| `mac_cmd_z_undoes_with_the_terminal_focused` | after an applied live edit, Cmd+Z restores the pre-edit `doc_hash` | yes |
| `mac_shift_cmd_z_redoes` | hash equals the post-edit hash | yes |
| `windows_ctrl_z_undoes_in_an_agent_tab_when_the_cli_does_not_use_it` | function returns `Undo` | yes |
| `windows_ctrl_z_is_left_to_a_shell_tab` | `None` | guard |
| `ctrl_z_is_left_to_a_cli_that_uses_it` | `None` for a `true` row | guard |
| `undo_chord_writes_nothing_to_the_pty` | stub agent receives zero bytes | guard |
| `chord_is_ignored_during_a_user_edit` | document unchanged | yes |
| `granularity_is_one_step_per_tool_call` | two patches need two undos | guard |

Run: `cargo test -p designos-app shell terminal_undo_chord`.

## 11. Performance budgets

One key match per key event; no allocation; no new repaint request. Undo cost is the app's existing undo.

## 12. Dependencies and build order

Independent; any wave. It touches `shell.rs` only at `handle_keys`, so it does not queue on `terminal.rs`. If #589 lands later, the granularity text in the shortcuts line stays true.

## 13. Owner rulings

- Cmd+Z and Shift+Cmd+Z (Ctrl+Z and Ctrl+Y on Windows and Linux) undo and redo the document while the terminal has focus; pick and document the Windows and Linux rule; granularity unchanged. (Issue text.)

## Open questions for the owner (recommended first)

1. Windows and Linux rule. Recommended: the rule in section 3 (agent tabs undo unless the CLI uses Ctrl+Z, shell tabs keep the key), because the table is safe while it is ungrounded.
2. Whether to fix the Ctrl-key forwarding on Windows and Linux in a separate task. Recommended: yes, a small bug task; it changes how Ctrl+C, Ctrl+D and Ctrl+L reach the CLI there.
