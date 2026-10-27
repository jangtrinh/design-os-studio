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

---

## Round 2

### 1. What could NOT be verified
- Live interactive manual typing inside a real external vendor CLI (`claude`, `codex`, `gemini`, `grok`, `agy`) could not be run in this session because the worker harness forbids invoking real agent CLI binaries (`Never run claude, codex, gemini, grok or agy`). All terminal PTY byte delivery (`\t`, `\r`, `\x1b`), focus transitions, and canvas shortcut behaviors were verified deterministically through the `Sim` GUI harness against stubbed agent PTY tabs (`FACT`).

### 2. Conclusion & Epistemic Summary
- **Conclusion (`FACT`):** Fixed both Round 2 keyboard regressions test-first on branch `jangtrinh/agy-1195-approval-banner-tab` (`243e8568` failing tests -> final fix commit) by removing the `has_approval_banner` `Key::Tab` suppression in `render_vt100_viewport` (`crates/designos-app/src/terminal.rs:6295`) and restoring `TerminalState::is_capturing_keyboard` to its base predicate (`crates/designos-app/src/terminal.rs:2906-2908`).
- **Weakest link:** `shell::tests::approval_banner_tab` tests exercise stubbed PTY processes rather than live third-party agent CLIs, though the PTY write path (`tab.write_bytes` / `tab.written_bytes`) is identical across stub and real child processes (`FACT`).
- **Skills loaded & changes made because of them:**
  - `/es:dev` -> `es:feature-dev`: Followed the DEV phase red-first vertical slice discipline (committed failing tests in `243e8568` before the implementation fix) and verified all gates after the final code edit.
  - `/es:rust`: Preserved borrow-first / zero-allocation state inspection and avoided nested `egui::Context` locks when checking focus (`ctx.memory(|m| ...)`).
  - `/es:rust-gpu-canvas`: Left GPU compilation and `FrameKey` paths untouched.
  - `es:fable-thinking` & `es-astra-work`: Verified whether `agent_a11y::non_canvas_has_focus(ctx)` already guards canvas `Tab` traversal before adding any special-case focus check, predicted outcomes before each tool run, and ran full adversarial test gates.
  - `es-designer`: Confirmed no visual layout, token, or pixel changes occurred in Round 2 (`agents-approval-open-tab` and `agents-approval-background-tab` presets render identically to Round 1).

### 3. Drift (`spec says / HEAD says`)
- **Spec says:** `terminal.rs` `render_vt100_viewport`: `has_approval_banner = state.needs_approval()`, and `needs_approval()` now checks every tab. `Key::Tab` is then dropped instead of sending `\t`.
  - **HEAD (`af4143cc`) said (`FACT`):** `crates/designos-app/src/terminal.rs:6141` set `let has_approval_banner = state.needs_approval();` and `terminal.rs:6298-6302` gated `Key::Tab` on `if !has_approval_banner { raw_bytes.push(b'\t'); }`.
  - **HEAD (Round 2) says (`FACT`):** `has_approval_banner` is removed and `crates/designos-app/src/terminal.rs:6295` unconditionally executes `Key::Tab => raw_bytes.push(b'\t'),` when the terminal viewport is focused.
- **Spec says:** `is_capturing_keyboard` now returns true for `agent_panel_open && side_panel_pty_open && needs_approval()`, even when the viewport is not focused.
  - **HEAD (`af4143cc`) said (`FACT`):** `crates/designos-app/src/terminal.rs:2906-2908` returned `self.mention_popup.is_open() || (agent_panel_open && self.side_panel_pty_open && (self.viewport_focused || self.needs_approval()))`.
  - **HEAD (Round 2) says (`FACT`):** `crates/designos-app/src/terminal.rs:2906-2908` is restored to base: `self.mention_popup.is_open() || (agent_panel_open && self.side_panel_pty_open && self.viewport_focused)`.
- **Spec says:** Its callers include `canvas/interact.rs:296` (the canvas `typing` guard) and `shell.rs:85`, `545` and `1090` (Escape).
  - **HEAD says (`FACT`):**
    - `crates/designos-app/src/canvas/interact.rs:296`: `let typing = (crate::agent_a11y::non_canvas_has_focus(&ctx) || app.terminal.is_capturing_keyboard(agent_rail_open)) && app.canvas.text.is_none();`
    - `crates/designos-app/src/shell.rs:85`: `crate::agent_a11y::non_canvas_has_focus(ctx) || app.terminal.is_capturing_keyboard(agent_rail_open)`
    - `crates/designos-app/src/shell.rs:545`: `let capturing_terminal = self.app.terminal.is_capturing_keyboard(agent_rail_open);`
    - `crates/designos-app/src/shell.rs:1090`: `if !self.app.terminal.is_capturing_keyboard(agent_rail_open) && ctx.input(|i| i.key_pressed(Key::Escape)) {`
- **Spec says:** Check first whether `agent_a11y::non_canvas_has_focus(ctx)` already covers that, since the button is an egui widget with focus.
  - **HEAD says (`FACT`):** `crates/designos-app/src/agent_a11y.rs:74-76` defines `pub fn non_canvas_has_focus(ctx: &Context) -> bool { ctx.memory(|m| m.focused().is_some_and(|id| id != canvas_id())) }`. Because panel chrome widgets (`("agents-left-tab-select", tab_id)`, etc.) and approval card buttons (`("agents-left-pty", "agent-approval-open-tab")` / `("agents-left-pty", "agent-approval-show-prompt")`) are egui widgets with ids `!= canvas_id()`, `non_canvas_has_focus(ctx)` is already `true` whenever focus is in panel chrome or on an approval card button.

### 4. Root Cause (`file:line` evidence)
1. **Blocker 1 — Terminal drops `Tab` whenever any tab waits for approval (`FACT`):**
   - In `crates/designos-app/src/terminal.rs:6141` and `6298-6302` (at `af4143cc`), `render_vt100_viewport` computed `let has_approval_banner = state.needs_approval();` and suppressed `Key::Tab => raw_bytes.push(b'\t')` whenever `has_approval_banner` was true. Because `state.needs_approval()` (`terminal.rs:1618-1620`) checks all tabs (`self.tabs.iter().any(...)`), any waiting tab (background or active) caused the focused active terminal viewport to swallow `\t` instead of forwarding it to the PTY.
   - **Fix (`FACT`):** Removed `has_approval_banner` and restored `Key::Tab => raw_bytes.push(b'\t')` at `crates/designos-app/src/terminal.rs:6295`. When focus is in panel chrome (`viewport_focused == false`), the `if viewport_focused && !card_activated && ...` block (`terminal.rs:6186`) does not run, so `Tab` traverses panel chrome into `Show prompt` / `Open tab` as required by AC-4.
2. **Blocker 2 — Canvas single-key shortcuts (`V`, `F`, `R`, `T`, `Backspace`, `Escape`) die while any tab waits (`FACT`):**
   - In `crates/designos-app/src/terminal.rs:2906-2908` (at `af4143cc`), `TerminalState::is_capturing_keyboard` returned `true` whenever `agent_panel_open && self.side_panel_pty_open && (self.viewport_focused || self.needs_approval())`. Even after clicking the canvas (`viewport_focused == false`), `is_capturing_keyboard` stayed `true` as long as any tab waited for approval, forcing `typing = true` in `crates/designos-app/src/canvas/interact.rs:296` and early-returning in `crates/designos-app/src/shell.rs:545-549`.
   - **Fix (`FACT`):** Restored `TerminalState::is_capturing_keyboard` (`crates/designos-app/src/terminal.rs:2906-2908`) to `self.mention_popup.is_open() || (agent_panel_open && self.side_panel_pty_open && self.viewport_focused)`. `agent_a11y::non_canvas_has_focus(&ctx)` (`agent_a11y.rs:74-76`) already keeps `typing = true` whenever focus is on a panel chrome widget or approval card button.

### 5. Acceptance Table & Pasted Command Outputs

| Check | Status | Evidence |
|---|---|---|
| Red-first Round 2 tests (`243e8568`) | PASS (failed as expected) | `plans/reports/agy-1195-approval-banner-tab-evidence/round2-red.log` (`5 passed; 2 failed`) |
| Green Round 2 `approval_banner_tab` (7 tests) | PASS | `plans/reports/agy-1195-approval-banner-tab-evidence/round2-green.log` (`7 passed; 0 failed`) |
| `agent_panel_keyboard` (27 tests) | PASS | `27 passed; 0 failed` |
| AC-5 safety set (`mcp_paths` + 98 `designos-app` tests) | PASS | `21 passed; 0 failed` + `98 passed; 0 failed` |
| `cargo fmt --all -- --check` | PASS | Exit `0` |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS | Exit `0` |
| `cargo test --workspace --locked --lib` | PASS | Exit `0` (all library crates 0 failed) |
| `cargo test -p designos-app --bin designos --locked` | PASS | `3341 passed; 1 failed (#996 flake); 11 ignored`; `#996` rerun 3x solo: `3/3 ok` |
| `python3 scripts/zero_fake_ui_gate.py` | PASS | `Gate Verdict: PASS (17/17 invariants satisfied)` |
| `python3 scripts/check_spacing_relationships.py` | PASS | `PASS: no count rose above the baseline` |

#### 5.1 Red log (`plans/reports/agy-1195-approval-banner-tab-evidence/round2-red.log` on `243e8568`)
```text
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked approval_banner_tab
running 7 tests
test shell::tests::approval_banner_tab::active_tab_approval_shows_show_prompt_and_focuses_terminal ... ok
test shell::tests::approval_banner_tab::approval_answered_in_terminal_clears_banner_and_sends_nothing_twice ... ok
test shell::tests::approval_banner_tab::focused_terminal_receives_tab_while_background_or_active_tab_waits_for_approval ... FAILED
test shell::tests::approval_banner_tab::background_tab_approval_shows_banner_and_open_tab_switches_to_waiting_tab ... ok
test shell::tests::approval_banner_tab::keyboard_tab_and_enter_reach_show_prompt_and_open_tab_with_accessible_labels ... ok
test shell::tests::approval_banner_tab::canvas_single_key_shortcuts_work_when_canvas_clicked_while_background_tab_waits_for_approval ... FAILED
test shell::tests::approval_banner_tab::two_pending_approvals_in_two_tabs_shows_count_and_opens_oldest_first ... ok

failures:

---- shell::tests::approval_banner_tab::focused_terminal_receives_tab_while_background_or_active_tab_waits_for_approval stdout ----

thread 'shell::tests::approval_banner_tab::focused_terminal_receives_tab_while_background_or_active_tab_waits_for_approval' (83619776) panicked at crates/designos-app/src/shell/tests/approval_banner_tab.rs:313:5:
assertion `left == right` failed: active focused terminal PTY must receive '\t' even while a background tab waits for approval
  left: []
 right: [9]

---- shell::tests::approval_banner_tab::canvas_single_key_shortcuts_work_when_canvas_clicked_while_background_tab_waits_for_approval stdout ----

thread 'shell::tests::approval_banner_tab::canvas_single_key_shortcuts_work_when_canvas_clicked_while_background_tab_waits_for_approval' (83619775) panicked at crates/designos-app/src/shell/tests/approval_banner_tab.rs:350:5:
assertion `left == right` failed: pressing R after clicking the canvas must switch to the Rectangle tool while a background tab waits
  left: Move
 right: Rectangle

failures:
    shell::tests::approval_banner_tab::canvas_single_key_shortcuts_work_when_canvas_clicked_while_background_tab_waits_for_approval
    shell::tests::approval_banner_tab::focused_terminal_receives_tab_while_background_or_active_tab_waits_for_approval

test result: FAILED. 5 passed; 2 failed; 0 ignored; 0 measured; 3346 filtered out; finished in 2.61s
```

#### 5.2 Green log (`plans/reports/agy-1195-approval-banner-tab-evidence/round2-green.log`)
```text
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked approval_banner_tab
running 7 tests
test shell::tests::approval_banner_tab::active_tab_approval_shows_show_prompt_and_focuses_terminal ... ok
test shell::tests::approval_banner_tab::approval_answered_in_terminal_clears_banner_and_sends_nothing_twice ... ok
test shell::tests::approval_banner_tab::background_tab_approval_shows_banner_and_open_tab_switches_to_waiting_tab ... ok
test shell::tests::approval_banner_tab::keyboard_tab_and_enter_reach_show_prompt_and_open_tab_with_accessible_labels ... ok
test shell::tests::approval_banner_tab::focused_terminal_receives_tab_while_background_or_active_tab_waits_for_approval ... ok
test shell::tests::approval_banner_tab::two_pending_approvals_in_two_tabs_shows_count_and_opens_oldest_first ... ok
test shell::tests::approval_banner_tab::canvas_single_key_shortcuts_work_when_canvas_clicked_while_background_tab_waits_for_approval ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 3346 filtered out; finished in 8.31s
```

#### 5.3 `agent_panel_keyboard` & AC-5 safety set
```text
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked agent_panel_keyboard
running 27 tests
test shell::tests::agent_panel_keyboard::clipped_other_cli_form::tab_does_not_walk_into_the_field_of_a_form_that_is_out_of_view ... ok
test shell::tests::agent_panel_keyboard::hidden_other_cli_form::closing_the_form_or_leaving_the_agents_panel_leaves_no_caret_in_it ... ok
test shell::tests::agent_panel_keyboard::hidden_other_cli_form::a_hidden_form_in_the_new_session_list_gets_none_of_a_renames_letters ... ok
test shell::tests::agent_panel_keyboard::clipped_other_cli_form::the_form_takes_letters_once_it_is_scrolled_into_view ... ok
test shell::tests::agent_panel_keyboard::clipped_other_cli_form::tab_does_not_walk_into_a_form_that_is_out_of_view ... ok
test shell::tests::agent_panel_keyboard::overlay_escape::esc_closes_the_overlay_without_also_closing_the_session_menu_below_it ... ok
test shell::tests::agent_panel_keyboard::hidden_other_cli_form::a_hidden_form_in_the_plus_menu_gets_none_of_a_renames_letters ... ok
test shell::tests::agent_panel_keyboard::dev_mode_shortcut::a_capital_d_typed_while_the_agent_terminal_has_focus_goes_to_the_terminal ... ok
test shell::tests::agent_panel_keyboard::overlay_escape::enter_and_arrows_stay_out_of_the_terminal_while_an_overlay_is_open ... ok
test shell::tests::agent_panel_keyboard::overlay_escape::esc_belongs_to_the_terminal_while_it_holds_the_keyboard_and_to_the_report_sheet_otherwise ... ok
test shell::tests::agent_panel_keyboard::hidden_other_cli_form::another_text_field_gets_the_letters_after_the_form_is_hidden ... ok
test shell::tests::agent_panel_keyboard::hand_built_popovers::the_check_designs_panel_never_holds_the_keys_from_the_terminal ... ok
test shell::tests::agent_panel_keyboard::dev_mode_shortcut::a_capital_d_typed_into_the_composer_does_not_toggle_dev_mode ... ok
test shell::tests::agent_panel_keyboard::overlay_escape::esc_for_a_dialog_never_stops_the_turn_after_a_click_in_the_panel_header ... ok
test shell::tests::agent_panel_keyboard::overlay_escape::a_font_dropdown_whose_section_disappeared_does_not_leave_the_terminal_deaf ... ok
test shell::tests::agent_panel_keyboard::overlay_escape::esc_in_the_composer_never_stops_a_running_turn ... ok
test shell::tests::agent_panel_keyboard::session_rename_keyboard::a_rename_takes_the_keyboard_from_the_agent_terminal ... ok
test shell::tests::agent_panel_keyboard::overlay_escape::the_floating_web_capture_report_does_not_take_the_keyboard_from_the_terminal ... ok
test shell::tests::agent_panel_keyboard::overlay_escape::an_export_scale_menu_whose_section_disappeared_does_not_leave_the_terminal_deaf ... ok
test shell::tests::agent_panel_keyboard::review_card_focus::keep_hands_the_keyboard_back_to_the_composer ... ok
test shell::tests::agent_panel_keyboard::review_card_focus::revert_hands_the_keyboard_back_to_the_composer ... ok
test shell::tests::agent_panel_keyboard::review_card_focus::revert_does_not_steal_the_keyboard_from_the_canvas ... ok
test shell::tests::agent_panel_keyboard::hand_built_popovers::an_outside_press_closes_each_hand_built_popover_and_the_terminal_hears_typing_again ... ok
test shell::tests::agent_panel_keyboard::overlay_escape::a_menu_whose_host_stopped_drawing_does_not_leave_the_terminal_deaf ... ok
test shell::tests::agent_panel_keyboard::overlay_escape::enter_reaches_the_terminal_again_once_the_overlay_is_closed ... ok
test shell::tests::agent_panel_keyboard::overlay_escape::esc_closes_the_open_overlay_and_never_reaches_the_terminal ... ok
test shell::tests::agent_panel_keyboard::overlay_escape::enter_never_reaches_the_terminal_while_an_overlay_is_open ... ok

test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 3326 filtered out; finished in 157.88s

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test mcp_paths --locked
running 21 tests
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.04s

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked -- approval_banner_tab agent_approval_channel:: agent_permissions::approval_channel_tests:: terminal::ask_hooks_tests:: agent_session_files::folder_tests:: agent_session_files::grok_config_tests:: agent_session_files::agy_config_tests:: agent_session_files::exact_config_race_tests:: terminal::ask_mode_tests:: terminal::gemini_wait_tests:: sim_approval_card_and_permissions_modal_pass_layout_lint grok_and_agy_cards_have_no_buttons closing_or_exiting_one_tab_preserves_other_tab_pending_approvals skip_mode_and_empty_claude_requests_never_show_approval_card_or_notify
running 98 tests
test result: ok. 98 passed; 0 failed; 0 ignored; 0 measured; 3255 filtered out; finished in 39.40s
```

#### 5.4 Gates (`fmt`, `clippy`, `zero_fake_ui_gate.py`, `check_spacing_relationships.py`, `--workspace --lib`, full `--bin designos`)
```text
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo fmt --all -- --check
(exited 0)

$ python3 scripts/zero_fake_ui_gate.py
Gate Verdict: PASS (17/17 invariants satisfied)

$ python3 scripts/check_spacing_relationships.py
literal geometry outside theme.rs: offset_literals=1668, inset_literals=45, radius_literals=257, font_literals=25, nonconcentric_pairs=2
note: 14 entries are below baseline; run --write-baseline to lock the gain in
PASS: no count rose above the baseline

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings
   Compiling designos-mcp v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1195-approval-banner-tab/crates/designos-mcp)
    Checking designos-app v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1195-approval-banner-tab/crates/designos-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 17.85s

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test --workspace --locked --lib
test result: ok. 13 passed; 0 failed (designos-diff)
test result: ok. 633 passed; 0 failed (designos-doc)
test result: ok. 11 passed; 0 failed (designos-export)
test result: ok. 179 passed; 0 failed (designos-fig)
test result: ok. 10 passed; 0 failed (designos-fonts)
test result: ok. 99 passed; 0 failed (designos-gpu)
test result: ok. 46 passed; 0 failed (designos-library)
test result: ok. 235 passed; 0 failed (designos-mcp)
test result: ok. 55 passed; 0 failed; 2 ignored (designos-render)
test result: ok. 1 passed; 0 failed (designos-svg)
test result: ok. 8 passed; 0 failed (designos-webcapture)

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked --no-fail-fast
running 3353 tests
...
failures:
    shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one
test result: FAILED. 3341 passed; 1 failed; 11 ignored; 0 measured; 0 filtered out; finished in 674.80s

$ # Solo 3x rerun of known flake #996 (an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one):
=== Rerun 1 ===
running 1 test
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3352 filtered out; finished in 0.96s
=== Rerun 2 ===
running 1 test
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3352 filtered out; finished in 0.84s
=== Rerun 3 ===
running 1 test
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3352 filtered out; finished in 0.74s
```

### 6. Assumptions & Unresolved Questions
- **Assumptions:** None beyond the spec's ruling that AC-4's `"Tab reaches the buttons"` applies when focus is in the panel chrome (verified by `keyboard_tab_and_enter_reach_show_prompt_and_open_tab_with_accessible_labels`), while the focused terminal viewport owns `Tab` (`\t`).
- **Unresolved questions:** None.

Status: COMPLETED — Round 2 keyboard regressions fixed and verified on `jangtrinh/agy-1195-approval-banner-tab`.
