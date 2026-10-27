# DEV Report · #1195 (P1, v0.6.4): Approval Banner Tab Actions (`Show prompt` on Active Tab, `Open tab` on Background Tab)

## Status
**COMPLETED** — Branch `jangtrinh/agy-1195-approval-banner-tab` (based on `origin/release/v0.6.4` at `6f0c3bb2`).

## Summary
- **Root cause (`FACT`):** `agent_approval_card::CardAction::OpenTab` carried no tab id (`agent_approval_card.rs:19`), `TerminalState::needs_approval()` checked only `self.active_tab()` (`terminal.rs:1618`), and `paint_approval_card` unconditionally rendered **Open tab** for every non-Claude tab even when the user was already viewing that tab's live PTY terminal (`terminal.rs:5991`), while `handle_approval_card_action` (`terminal.rs:1666`) only flipped `side_panel_pty_open`/`chat_mode`/`viewport_focused` without switching tabs or surrendering egui button focus.
- **AC-1 (Approval on active tab):** Added `CardAction::ShowPrompt` and `paint_approval_card_for_tab(..., tab_id, in_active_terminal, total_pending, surface_id)`. When the waiting approval belongs to the active tab and the terminal viewport is open (`in_active_terminal = is_active_tab && state.side_panel_pty_open && !state.picker_open`), the card renders **Show prompt** (`"agent-approval-show-prompt"`) in the exact same position, size, and style as **Open tab**. Clicking or pressing `Enter` on **Show prompt** focuses the terminal viewport (`viewport_focused = true`, `chrome_focused = false`, surrendering egui widget focus and suppressing that frame's activating `Enter` from leaking into the PTY) so subsequent `Enter` and `Esc` keystrokes reach the CLI immediately.
- **AC-2 (Approval on background tab & multiple waiting tabs):** `CardAction::OpenTab(usize)` now carries the waiting tab's id. `TerminalState::needs_approval()` checks all tabs, and `TerminalState::approval_banner_target()` selects the oldest waiting tab across `self.tabs` (lowest spawn order) while summing pending approvals across all waiting tabs (`total_pending`). We chose the **single-banner-with-count (`"1 of N"`) targeting the oldest waiting tab** design because the card header already has a right-aligned `"1 of N"` slot (`agent_approval_card.rs:74`), requiring zero card height or layout changes. Clicking **Open tab** calls `focus_tab_by_id(tab_id)` and focuses that tab's terminal viewport.
- **AC-3 (Out of scope per PM decision 2026-10-27 02:06Z):** No `Allow`/`Deny` keystroke-injection buttons or PTY scrapers were added.
- **AC-4 (Keyboard navigation & accessible labels):** Both **Show prompt** and **Open tab** are reachable via `Tab` and activated with `Enter`, paint a `theme::BORDER_SELECTED` focus ring when focused, and attach AccessKit `Role::Button` labels via `widgets::labelled`: `"Show the <Agent> prompt"` (e.g. `"Show the Codex prompt"`) and `"Open the <Agent> tab waiting for approval"` (e.g. `"Open the Codex tab waiting for approval"`). `TerminalState::is_capturing_keyboard` now keeps keyboard capture while an approval banner is active in the open Agents panel so canvas `Tab` sibling traversal does not surrender widget focus during `Tab` navigation.
- **Layout-gate preset diff (`--panel agents-approval-open-tab`):** Captured before (`plans/reports/agy-1195-approval-banner-tab-evidence/before-agents-approval-open-tab.png`) and after (`plans/reports/agy-1195-approval-banner-tab-evidence/after-agents-approval-open-tab.png`) at `1440x900` with `--no-toast`. Pixel diff bounding box is `(351, 132, 433, 160)` (`82x28 px`) — strictly the action button changing from `Open tab` to `Show prompt`, with zero other render changes anywhere on screen. Added `--panel agents-approval-background-tab` (`plans/reports/agy-1195-approval-banner-tab-evidence/after-agents-approval-background-tab.png`) to cover the background-tab **Open tab** case.

## Files modified
- [`crates/designos-app/src/agent_approval_card.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1195-approval-banner-tab/crates/designos-app/src/agent_approval_card.rs) — Added `CardAction::ShowPrompt`, changed `CardAction::OpenTab` to `CardAction::OpenTab(usize)`, added `paint_approval_card_for_tab`, generalized the `"1 of N"` header indicator to `total_pending`, added AccessKit labels (`"Show the <Agent> prompt"` / `"Open the <Agent> tab waiting for approval"`), and painted `BORDER_SELECTED` on focused card buttons.
- [`crates/designos-app/src/terminal.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1195-approval-banner-tab/crates/designos-app/src/terminal.rs) — Extended `needs_approval()` across all tabs, added `approval_banner_target()`, updated `handle_approval_card_action` and `focus_tab_by_id`, wired `paint_approval_card_for_tab` in chat and PTY views with focus surrender and `card_activated` Enter-leak guard, suppressed PTY `Tab` injection while an approval banner is active, and added the `"agents-approval-background-tab"` fixture.
- [`crates/designos-app/src/cli.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1195-approval-banner-tab/crates/designos-app/src/cli.rs) — Registered `"agents-approval-background-tab"` in `PANELS`.
- [`crates/designos-app/src/shell.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1195-approval-banner-tab/crates/designos-app/src/shell.rs) — Routed `Some("agents-approval-background-tab")` to `open_approval_fixture`.
- [`crates/designos-app/src/shell/tests.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1195-approval-banner-tab/crates/designos-app/src/shell/tests.rs) — Registered `mod approval_banner_tab;`.
- [`crates/designos-app/src/shell/tests/approval_banner_tab.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1195-approval-banner-tab/crates/designos-app/src/shell/tests/approval_banner_tab.rs) — Added 5 `Sim` GUI tests covering AC-1, AC-2, multiple waiting tabs, terminal-answered prompt clearing, and AC-4 keyboard `Tab`/`Enter` navigation + AccessKit labels.
- **Files outside scope:** None (`agent_turn_card.rs` and `panels/left.rs` untouched).

## Acceptance & gate output

### 1. Red-first & Green test logs (`approval_banner_tab`)
- **Red log (`plans/reports/agy-1195-approval-banner-tab-evidence/red.log` on `396f2bc1`):**
  ```text
  running 5 tests
  test shell::tests::approval_banner_tab::active_tab_approval_shows_show_prompt_and_focuses_terminal ... FAILED
  test shell::tests::approval_banner_tab::approval_answered_in_terminal_clears_banner_and_sends_nothing_twice ... FAILED
  test shell::tests::approval_banner_tab::background_tab_approval_shows_banner_and_open_tab_switches_to_waiting_tab ... FAILED
  test shell::tests::approval_banner_tab::keyboard_tab_and_enter_reach_show_prompt_and_open_tab_with_accessible_labels ... FAILED
  test shell::tests::approval_banner_tab::two_pending_approvals_in_two_tabs_shows_count_and_opens_oldest_first ... FAILED

  test result: FAILED. 0 passed; 5 failed; 0 ignored; 0 measured; 3346 filtered out; finished in 2.78s
  ```
- **Green log (`plans/reports/agy-1195-approval-banner-tab-evidence/green.log`):**
  ```text
  running 5 tests
  test shell::tests::approval_banner_tab::active_tab_approval_shows_show_prompt_and_focuses_terminal ... ok
  test shell::tests::approval_banner_tab::approval_answered_in_terminal_clears_banner_and_sends_nothing_twice ... ok
  test shell::tests::approval_banner_tab::background_tab_approval_shows_banner_and_open_tab_switches_to_waiting_tab ... ok
  test shell::tests::approval_banner_tab::keyboard_tab_and_enter_reach_show_prompt_and_open_tab_with_accessible_labels ... ok
  test shell::tests::approval_banner_tab::two_pending_approvals_in_two_tabs_shows_count_and_opens_oldest_first ... ok

  test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 3346 filtered out; finished in 1.39s
  ```

### 2. AC-5 Ask-mode Safety Tests (`#1035`, `#1041`, `#1060`, `#1078`, `#1103`) & Workspace `--lib` Tests
- `cargo test --workspace --locked --lib` -> exit `0` (all workspace library crates green)
- `cargo test -p designos-mcp --test mcp_paths --locked` (`#1103`) -> exit `0` (`58 passed; 0 failed`)
- `cargo test -p designos-app --bin designos --locked -- approval_banner_tab agent_approval_channel:: agent_permissions::approval_channel_tests:: terminal::ask_hooks_tests:: agent_session_files::folder_tests:: agent_session_files::grok_config_tests:: agent_session_files::agy_config_tests:: agent_session_files::exact_config_race_tests:: terminal::ask_mode_tests:: terminal::gemini_wait_tests:: sim_approval_card_and_permissions_modal_pass_layout_lint grok_and_agy_cards_have_no_buttons closing_or_exiting_one_tab_preserves_other_tab_pending_approvals skip_mode_and_empty_claude_requests_never_show_approval_card_or_notify`:
  ```text
  running 96 tests
  test agent_approval_channel::tests::the_channel_name_is_short_stable_and_per_session ... ok
  test agent_approval_channel::tests::a_root_inside_the_session_folder_gets_no_channel ... ok
  test agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root ... ok
  test agent_approval_channel::tests::a_link_in_place_of_the_root_is_refused ... ok
  test agent_approval_channel::tests::prepared_folders_are_owner_only_even_when_they_existed_with_a_looser_mode ... ok
  test agent_permissions::approval_channel_tests::forged_allow_in_the_old_channel_and_every_agent_writable_root_does_not_approve ... ok
  test agent_permissions::approval_channel_tests::an_environment_pointing_the_hook_at_an_agent_writable_folder_changes_nothing ... ok
  test agent_session_files::folder_tests::a_link_planted_at_a_library_session_path_is_refused ... ok
  test agent_session_files::folder_tests::a_link_planted_at_the_next_ephemeral_session_path_is_refused ... ok
  test agent_session_files::grok_config_tests::an_ask_mode_grok_tab_moves_aside_every_other_project_source_grok_would_run_and_keeps_what_is_not_one ... ok
  test agent_session_files::agy_config_tests::an_ask_mode_agy_tab_does_not_start_when_its_hooks_do_not_read_back_as_written ... ok
  test agent_session_files::exact_config_race_tests::a_plugin_that_appears_while_studio_writes_stops_an_agy_tab_and_is_set_aside ... ok
  test terminal::ask_hooks_tests::in_ask_mode_claude_reads_its_hooks_from_the_private_folder ... ok
  test terminal::ask_hooks_tests::rewriting_the_session_hooks_json_in_ask_mode_changes_nothing_claude_reads ... ok
  test terminal::ask_mode_tests::claude_ask_launches_without_the_skip_flag_and_writes_no_allow_rule ... ok
  test terminal::ask_mode_tests::codex_ask_launches_without_the_bypass_flag_and_sets_no_approval_override ... ok
  test terminal::ask_mode_tests::grok_ask_launches_without_always_approve_and_writes_no_permission_rule ... ok
  test terminal::ask_mode_tests::antigravity_ask_launches_without_the_skip_flag_and_its_plugin_config_trusts_nothing ... ok
  test terminal::gemini_wait_tests::gemini_ask_tab_waiting_needs_approval_and_shows_open_tab ... ok
  test terminal::tests::grok_and_agy_cards_have_no_buttons ... ok
  test terminal::tests::sim_approval_card_and_permissions_modal_pass_layout_lint ... ok
  test shell::tests::approval_banner_tab::active_tab_approval_shows_show_prompt_and_focuses_terminal ... ok
  test shell::tests::approval_banner_tab::approval_answered_in_terminal_clears_banner_and_sends_nothing_twice ... ok
  test shell::tests::approval_banner_tab::background_tab_approval_shows_banner_and_open_tab_switches_to_waiting_tab ... ok
  test shell::tests::approval_banner_tab::keyboard_tab_and_enter_reach_show_prompt_and_open_tab_with_accessible_labels ... ok
  test shell::tests::approval_banner_tab::two_pending_approvals_in_two_tabs_shows_count_and_opens_oldest_first ... ok
  ...
  test result: ok. 96 passed; 0 failed; 0 ignored; 0 measured; 3255 filtered out; finished in 143.51s
  ```

### 3. Formatting, Clippy & UI/Spacing Gates
- `cargo fmt --all -- --check` -> exit `0`
- `cargo clippy --workspace --all-targets --locked -- -D warnings` -> exit `0` (`Finished dev profile [unoptimized + debuginfo] target(s) in 2m 29s`)
- `python3 scripts/zero_fake_ui_gate.py` -> `Gate Verdict: PASS (17/17 invariants satisfied)`
- `python3 scripts/check_spacing_relationships.py` -> `PASS: no count rose above the baseline`

### 4. Visual Evidence (`plans/reports/agy-1195-approval-banner-tab-evidence/`)
- `before-agents-approval-open-tab.png` (`1440x900`, `--panel agents-approval-open-tab --no-toast`)
- `after-agents-approval-open-tab.png` (`1440x900`, `--panel agents-approval-open-tab --no-toast`, pixel diff bbox `(351, 132, 433, 160)` only)
- `after-agents-approval-background-tab.png` (`1440x900`, `--panel agents-approval-background-tab --no-toast`)

## Residual risks
- `--panel agents-approval-open-tab` is a `#1126` visual baseline preset whose action button label intentionally changes from `Open tab` to `Show prompt` (bbox `(351, 132, 433, 160)`). As noted in Controller Ruling #6, the `#1126` baseline should be re-blessed on `main` after merge.
