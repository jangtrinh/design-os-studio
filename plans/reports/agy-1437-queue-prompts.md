# Report: #1437 Agents Composer — Queue a Second Send Behind the Waiting Prompt

## 0. What Could Not Be Verified (and Why)
- **Live external agent CLI binaries (`claude`, `codex`, `gemini`, `grok`, `agy`, `opencode`)**: Per the hermetic testing contract (`brief-common-argon.txt`), real external AI CLIs were never launched. All PTY, hook, and UI interactions were verified against local shell stubs (`#!/bin/sh`) under isolated sibling `HOME` and `TMPDIR` directories.

## 0b. Skills Loaded
- `es-astra-work` (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`)
- `es:fable-thinking` (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`)
- `es:rust` (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`)
- `es:rust-gpu-canvas` (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`)
- `es:dev` (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`)
- `es:feature-dev` (`/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`)

## 1. Executive Summary
- Replaced `ComposerState::pending_send: Option<String>` and `pending_send_is_system: bool` with an ordered FIFO queue `pending_send: VecDeque<QueuedSend>` (`QueuedSend { text: String, is_system: bool }`) so every write site appends and never overwrites a waiting prompt.
- Updated `tick_composer` (`is_tab_ready_for_queued_send`) to pop and write one entry per ready state (`pop_front()`), waiting until the previous turn's delayed Enter (`pending_enter_deadline`) completes before writing the next entry, while preserving all user entries in order joined by `"\n\n"` on failed start, early exit, or skill-limit refusal.
- Updated `spawn_custom_cli_tab` and `spawn_tab_with_registration` in `terminal.rs` so the entire `New chat` queue moves into the new tab, held `pending_prompt` replays append behind the queue without duplication, system prompts queue on starting/queued tabs instead of refusing with `RefuseReason::NotReady`, and two or more waiting entries show `"N messages waiting to send"`.

## 2. Files Changed (Round 1)
- `crates/designos-app/src/agent_composer.rs`
- `crates/designos-app/src/agent_composer_text.rs`
- `crates/designos-app/src/agent_open_flow.rs`
- `crates/designos-app/src/layout_snapshot.rs`
- `crates/designos-app/src/shell.rs` — Added `--panel agents-chat:two-waiting` CLI screenshot fixture (lines 443–449) so `--screenshot ... --panel agents-chat:two-waiting` opens the Agents panel, sets `active_runtime` to `Claude`, and calls `submit_chat_prompt` twice (`"First queued prompt"`, `"Second queued prompt"`) to capture the `"2 messages waiting to send"` state (`1437-two-waiting.png`).
- `crates/designos-app/src/shell/tests/agent_new_chat_composer.rs`
- `crates/designos-app/src/shell/tests/agent_simplified_panel.rs`
- `crates/designos-app/src/shell/tests/home.rs`
- `crates/designos-app/src/specs.rs`
- `crates/designos-app/src/terminal.rs`
- `crates/designos-app/src/terminal/gemini_composer_tests.rs`

## 3. Red -> Green Test Output (Round 1)

### Red Phase (on base commit `0953889a` before production changes)
Log: `plans/reports/agy-1437-queue-prompts-evidence/red-tests.log`
```text
running 8 tests
test agent_composer::tests::notice_reads_2_messages_waiting_to_send_with_two_entries ... FAILED
test agent_composer::tests::system_entry_queued_behind_user_entry_is_written_after_it ... FAILED
test agent_composer::tests::three_sends_while_starting_are_written_in_order_one_per_ready_state ... FAILED
test agent_composer::tests::failed_start_returns_user_entries_in_order_and_discards_system_entry_with_notice ... FAILED
test agent_composer::tests::skill_limit_refusal_on_entry_2_of_3_returns_entries_2_and_3_to_draft ... FAILED
test shell::tests::agent_new_chat_composer::p12_spec_ask_then_send_queues_both_and_writes_in_order ... FAILED
test shell::tests::agent_new_chat_composer::p12b_make_design_then_send_queues_both_and_writes_in_order ... FAILED
test shell::tests::agent_new_chat_composer::p13_queued_first_message_then_spec_ask_queues_both_in_order ... FAILED

test result: FAILED. 0 passed; 8 failed; 0 ignored; 0 measured; 3972 filtered out; finished in 1.78s
```

### Green Phase (with production changes)
Log: `plans/reports/agy-1437-queue-prompts-evidence/green-tests.log`
```text
running 8 tests
test agent_composer::tests::system_entry_queued_behind_user_entry_is_written_after_it ... ok
test agent_composer::tests::three_sends_while_starting_are_written_in_order_one_per_ready_state ... ok
test agent_composer::tests::notice_reads_2_messages_waiting_to_send_with_two_entries ... ok
test agent_composer::tests::skill_limit_refusal_on_entry_2_of_3_returns_entries_2_and_3_to_draft ... ok
test agent_composer::tests::failed_start_returns_user_entries_in_order_and_discards_system_entry_with_notice ... ok
test shell::tests::agent_new_chat_composer::p12b_make_design_then_send_queues_both_and_writes_in_order ... ok
test shell::tests::agent_new_chat_composer::p13_queued_first_message_then_spec_ask_queues_both_in_order ... ok
test shell::tests::agent_new_chat_composer::p12_spec_ask_then_send_queues_both_and_writes_in_order ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 3980 filtered out; finished in 0.90s
```

## 4. Mutation Test Results (4 Mutants Run & Reverted)

| Mutant | Mutation Applied | Result | Killing Tests | Evidence Log |
|---|---|---|---|---|
| **1. Overwrite instead of append** | `pending_send.clear()` before `push_back(...)` in `deliver_to_agent` and `start_new_chat_session` | **KILLED** (`0 passed; 8 failed`) | `three_sends_while_starting_are_written_in_order_one_per_ready_state`, `p12_spec_ask_then_send_queues_both_and_writes_in_order`, `p12b_make_design_then_send_queues_both_and_writes_in_order`, `p13_queued_first_message_then_spec_ask_queues_both_in_order`, `failed_start_returns_user_entries_in_order_and_discards_system_entry_with_notice`, `skill_limit_refusal_on_entry_2_of_3_returns_entries_2_and_3_to_draft`, `notice_reads_2_messages_waiting_to_send_with_two_entries`, `system_entry_queued_behind_user_entry_is_written_after_it` | `plans/reports/agy-1437-queue-prompts-evidence/mutant-1.log` |
| **2. Flush newest entry first** | `tab.composer.pending_send.pop_back()` instead of `pop_front()` in `tick_composer` | **KILLED** (`2 passed; 6 failed`) | `three_sends_while_starting_are_written_in_order_one_per_ready_state`, `p12_spec_ask_then_send_queues_both_and_writes_in_order`, `p12b_make_design_then_send_queues_both_and_writes_in_order`, `p13_queued_first_message_then_spec_ask_queues_both_in_order`, `system_entry_queued_behind_user_entry_is_written_after_it`, `skill_limit_refusal_on_entry_2_of_3_returns_entries_2_and_3_to_draft` | `plans/reports/agy-1437-queue-prompts-evidence/mutant-2.log` |
| **3. Flush two entries in one ready state** | `while let Some(queued) = tab.composer.pending_send.pop_front()` instead of `if let Some(queued) = ...` in `tick_composer` | **KILLED** (`2 passed; 6 failed`) | `three_sends_while_starting_are_written_in_order_one_per_ready_state`, `p12_spec_ask_then_send_queues_both_and_writes_in_order`, `p12b_make_design_then_send_queues_both_and_writes_in_order`, `p13_queued_first_message_then_spec_ask_queues_both_in_order`, `system_entry_queued_behind_user_entry_is_written_after_it`, `skill_limit_refusal_on_entry_2_of_3_returns_entries_2_and_3_to_draft` | `plans/reports/agy-1437-queue-prompts-evidence/mutant-3.log` |
| **4. Drop the tail on a skill-limit refusal** | `tab.composer.pending_send.clear()` without draining user entries to `user_texts` on skill-limit refusal in `tick_composer` | **KILLED** (`7 passed; 1 failed`) | `skill_limit_refusal_on_entry_2_of_3_returns_entries_2_and_3_to_draft` | `plans/reports/agy-1437-queue-prompts-evidence/mutant-4.log` |

## 5. Re-Anchor / Acceptance Table (Round 1)

| AC | Criterion | Status | Evidence |
|---|---|---|---|
| **AC 1** | Ordered `VecDeque<QueuedSend>` queue (`text`, `is_system`), every write site appends, flushes one entry per ready state (`pop_front()`) | **PASS** | `green-tests.log`, `mutant-1.log`, `mutant-2.log`, `mutant-3.log` |
| **AC 2** | `New chat` -> new tab transfers entire queue and appends held `pending_prompt` behind it without duplication | **PASS** | `p12_...`, `p12b_...`, `p13_...` in `green-tests.log` |
| **AC 3** | Failed start, early exit, and skill-limit refusal restore all user entries in order joined by `"\n\n"` and discard system entries with notice | **PASS** | `failed_start_...` and `skill_limit_refusal_...` in `green-tests.log`, `mutant-4.log` |
| **AC 4** | Waiting notice shows `"N messages waiting to send"` when 2+ entries wait; system prompts queue behind user prompts | **PASS** | `notice_reads_2_messages_waiting_to_send_with_two_entries`, `1437-two-waiting.png` |
| **AC 5** | Full gates (`fmt`, `clippy`, `agent`, `composer`, `terminal`, `--workspace --lib`) green | **PASS** | `clippy.log`, `gate-agent.log`, `gate-composer.log`, `gate-terminal.log`, `gate-workspace-lib.log` |

## 6. Assumptions Held & Weakest Link (Round 1)
- **[FACT]** `ComposerState::pending_send` is `VecDeque<QueuedSend>` and `std::mem::take(&mut self.unattached_composer)` moves the full queue into the newly spawned tab.
- **[INFERENCE]** Holding `pending_enter_deadline` prevents a second queued entry from pasting before the first entry's delayed `\r` is written.
- **Weakest Link (addressed in Round 2):** `is_tab_ready_for_queued_send` only waited for `pending_enter_deadline` and did not wait for the first turn itself to finish (`Working -> Idle/Done` or post-`\r` PTY output quiet), and `sent_in_start_window` held only a single `Option<String>`. Both were fixed in Round 2 below.

## 7. Gate Output & Pasted Commands (Round 1)
```bash
cargo fmt --all -- --check
HOME="$S/home" TMPDIR="$S/tmp/" CARGO_TARGET_DIR="$PWD/out/cargo" cargo clippy --workspace --all-targets --locked -- -D warnings
HOME="$S/home" TMPDIR="$S/tmp/" CARGO_TARGET_DIR="$PWD/out/cargo" cargo test -p designos-app --bin designos --locked agent
HOME="$S/home" TMPDIR="$S/tmp/" CARGO_TARGET_DIR="$PWD/out/cargo" cargo test -p designos-app --bin designos --locked composer
HOME="$S/home" TMPDIR="$S/tmp/" CARGO_TARGET_DIR="$PWD/out/cargo" cargo test -p designos-app --bin designos --locked terminal
HOME="$S/home" TMPDIR="$S/tmp/" CARGO_TARGET_DIR="$PWD/out/cargo" cargo test --workspace --locked --lib
```
- `cargo fmt --all -- --check`: **PASS** (exit 0).
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: **PASS** for all touched files (zero warnings in touched files; only `main`'s pre-existing warnings in `agent_permissions.rs` and `panels/components.rs`, logged in `plans/reports/agy-1437-queue-prompts-evidence/clippy.log`).
- `cargo test -p designos-app --bin designos --locked agent`: **PASS** (`867 passed; 0 failed; 3 ignored; finished in 175.10s`, logged in `plans/reports/agy-1437-queue-prompts-evidence/gate-agent.log`).
- `cargo test -p designos-app --bin designos --locked composer`: **PASS** (`80 passed; 0 failed; 0 ignored; finished in 20.48s`, logged in `plans/reports/agy-1437-queue-prompts-evidence/gate-composer.log`).
- `cargo test -p designos-app --bin designos --locked terminal`: **PASS** (`331 passed; 0 failed; 1 ignored; finished in 45.03s`, logged in `plans/reports/agy-1437-queue-prompts-evidence/gate-terminal.log`).
- `cargo test --workspace --locked --lib`: **PASS** (`0 failed` across all workspace library crates, logged in `plans/reports/agy-1437-queue-prompts-evidence/gate-workspace-lib.log`).

## 8. Visual Evidence & Pull Request (Round 1)
- Checked macOS `HIDIdleTime` before capture: `3599s` (`>= 300s`).
- Captured `out/evidence/1437-two-waiting.png` (also copied to `plans/reports/agy-1437-queue-prompts-evidence/1437-two-waiting.png`) using a scratch copy of `/Users/jangtrinh/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig` under the hermetic harness (`$TMPDIR/.designos-hermetic-active` verified with `pid=56520`, `dir=/Users/jangtrinh/orca/.s1437/bin`).
- Visually inspected `out/evidence/1437-two-waiting.png`: the Agents panel (`Claude Code #1`) displays `2 messages waiting to send` in the composer above the `+ / Claude Code / Ask first / ↑` control row.
- **Draft PR URL:** https://github.com/jangtrinh/design-os-studio/pull/1461
- **Branch:** `jangtrinh/agy-1437-queue-prompts` (`22d129ea`)
- **Disk Cleanup Confirmation:** `rm -rf out/cargo` and temporary sibling stub directories removed; `df -h /` reports `140Gi` free (`8%` used).

Status: DONE

---

## Round 2

### 1. What Could Not Be Verified (and Why)
- **Live external agent CLIs (`claude`, `codex`, `gemini`, `grok`, `agy`, `opencode`)**: Per the hermetic testing contract (`brief-common-argon.txt`), real external AI CLIs were never launched. All Round-2 readiness, start-window recovery, shell-tab coverage, and held-prompt replay scenarios were verified deterministically using local shell stubs (`#!/bin/sh`) and CPU rasterization under isolated sibling `HOME` and `TMPDIR` folders.

### 2. Skills Loaded
- `es-astra-work` (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`)
- `es:fable-thinking` (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`)
- `es:rust` (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`)
- `es:rust-gpu-canvas` (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`)
- `es:dev` (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`)
- `es:feature-dev` (`/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`)

### 3. Summary of Round-2 Rulings Implemented
- **Ruling 1 — MAJOR-1 (`sent_in_start_window: Vec<String>`):** Changed `ComposerState::sent_in_start_window` from `Option<String>` to `Vec<String>` (`crates/designos-app/src/agent_composer.rs`). `write_prompt_to_live_tab` pushes each user prompt written within `START_FAILURE_WINDOW`; `recover_undelivered_queue` appends all start-window prompts in order ahead of `pending_send` and any typed `draft` (`user_texts.append(&mut self.sent_in_start_window)`); `tick_composer` calls `sent_in_start_window.clear()` once `START_FAILURE_WINDOW` elapses.
- **Ruling 2 — MAJOR-2 (`covered_agent` & never queue a user prompt on a Shell tab):** In `deliver_to_agent` (`crates/designos-app/src/agent_composer.rs`), replaced `if self.picker_open && self.tabs.len() == tabs_before && startable && runtime_installed` with `let covered_agent = self.active_tab_mut().is_some_and(|t| t.is_alive() && !t.is_shell_process()); if !covered_agent && self.tabs.len() == tabs_before && startable && runtime_installed { let _ = self.spawn_tab(self.active_runtime, file_path); }`, followed by a guard that refuses with `RefuseReason::NoAgent` (restoring `draft` when `from_box`) if `!self.in_new_chat() && self.active_tab().is_some_and(|t| t.is_shell_process())`.
- **Ruling 3 — MAJOR-3 & Ruling Gap (readiness after a prompt is written & pause while Enter is held):** Added `flush_paused`, `awaiting_turn_end`, `turn_started_since_send`, `enter_written_at`, and `output_at_enter` to `ComposerState` (`crates/designos-app/src/agent_composer.rs`, `crates/designos-app/src/terminal.rs`). When `write_delayed_enter` holds back `\r` (`holds_the_enter(tab)`), it sets `flush_paused = true` so subsequent queued entries are not pasted behind the unsubmitted prompt. While `awaiting_turn_end` is `true`, `is_tab_ready_for_queued_send` never falls back to `spawned_at.elapsed() >= START_FAILURE_WINDOW`; it returns `true` only once `turn_started_since_send` is true and status is no longer `Working`/`WaitingForInput`, or once post-`\r` PTY output has arrived (`out_at >= enter_at || output_at_enter != Some(out_at)`) and been quiet for `>= READY_QUIET_OUTPUT` (180 ms).
- **Ruling 4 — MINOR-4 (tests for the six Round-1 review mutants, plus MINOR-1 / MINOR-2 / MINOR-3 report & git hygiene):** Added tests killing all six Round-1 review mutants (`M6_recover_drops_typed_draft`, `M6b_limit_drops_typed_draft`, `M7_no_start_window_on_failure`, `M9_held_replay_skipped_if_queue_nonempty`, `M15_no_notice_after_flush`, `M16_no_caption_with_draft`), committed Round-1 report and evidence (`b91f561a`), merged `origin/main` with a merge commit (`2b61c790`), and completed the Round-1 report sections.
- **Ruling 5 — LOW-6 (`pending_prompt` only when not `Queued`):** In `dispatch_figma_ai_prompt` (`crates/designos-app/src/terminal.rs`), `self.pending_prompt` is now set only when `outcome != SendOutcome::Queued`. In `spawn_custom_cli_tab` and `spawn_tab_with_registration`, removed the exact-string `already_queued` check and unconditionally replay `self.pending_prompt.take()`. Updated the two `agent_open_flow.rs` assertions accordingly.
- **Ruling 6 — LOW-7 (Copy consistency):** Updated held notice in `deliver_to_agent` to `"Waiting for an agent… your message will be sent"`, and updated the post-flush notice in `tick_composer` to `"1 message waiting to send"` when `remaining == 1` (`crates/designos-app/src/agent_composer.rs`).
- **Ruling 7 — LOW-8 (Naming & comments):** Removed `#1437` issue numbers from comments in `crates/designos-app/src/agent_composer.rs` and renamed `p12_spec_ask_then_send_queues_both_and_writes_in_order`, `p12b_make_design_then_send_queues_both_and_writes_in_order`, and `p13_queued_first_message_then_spec_ask_queues_both_in_order` in `crates/designos-app/src/shell/tests/agent_new_chat_composer.rs`.

### 4. Red -> Green Test Output (Round 2)

#### Red Phase (on `22d129ea` before Round-2 production fixes)
Log: `plans/reports/agy-1437-queue-prompts-evidence/r2-red-tests.log`
```text
running 7 tests
test agent_composer::tests::flushing_updates_waiting_count_to_one_and_skill_limit_keeps_typed_draft ... FAILED
test agent_composer::tests::working_agent_waits_for_turn_end_before_flushing_next_entry ... FAILED
test agent_composer::tests::held_enter_pauses_flushing_until_next_turn_starts_and_ends ... FAILED
test agent_composer::tests::entry_point_in_new_chat_over_a_shell_tab_starts_agent_and_sends ... FAILED
test agent_composer::tests::held_prompt_with_trailing_space_and_refused_held_prompt_replay_once_in_order ... FAILED
test agent_composer::tests::send_on_an_exited_tab_beside_a_live_one_starts_a_new_agent ... FAILED
test agent_composer::tests::start_window_keeps_every_written_entry_when_start_fails ... FAILED

test result: FAILED. 0 passed; 7 failed; 0 ignored; 0 measured; 3988 filtered out; finished in 3.25s
```

#### Green Phase (7 new Round-2 tests + 8 Round-1 tests on HEAD)
Log: `plans/reports/agy-1437-queue-prompts-evidence/r2-green-tests.log`
```text
running 15 tests
test agent_composer::tests::three_sends_while_starting_are_written_in_order_one_per_ready_state ... ok
test agent_composer::tests::working_agent_waits_for_turn_end_before_flushing_next_entry ... ok
test agent_composer::tests::system_entry_queued_behind_user_entry_is_written_after_it ... ok
test agent_composer::tests::notice_reads_2_messages_waiting_to_send_with_two_entries ... ok
test agent_composer::tests::held_enter_pauses_flushing_until_next_turn_starts_and_ends ... ok
test agent_composer::tests::flushing_updates_waiting_count_to_one_and_skill_limit_keeps_typed_draft ... ok
test agent_composer::tests::skill_limit_refusal_on_entry_2_of_3_returns_entries_2_and_3_to_draft ... ok
test agent_composer::tests::failed_start_returns_user_entries_in_order_and_discards_system_entry_with_notice ... ok
test agent_composer::tests::held_prompt_with_trailing_space_and_refused_held_prompt_replay_once_in_order ... ok
test shell::tests::agent_new_chat_composer::make_design_then_send_queues_both_and_writes_in_order ... ok
test shell::tests::agent_new_chat_composer::queued_first_message_then_spec_ask_queues_both_in_order ... ok
test shell::tests::agent_new_chat_composer::spec_ask_then_send_queues_both_and_writes_in_order ... ok
test agent_composer::tests::send_on_an_exited_tab_beside_a_live_one_starts_a_new_agent ... ok
test agent_composer::tests::entry_point_in_new_chat_over_a_shell_tab_starts_agent_and_sends ... ok
test agent_composer::tests::start_window_keeps_every_written_entry_when_start_fails ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 4037 filtered out; finished in 3.79s
```

### 5. Round-2 Mutation Test Results (6 Mutants Run & Reverted)

| Mutant | File & Site | Mutation Applied | Result | Killing Tests | Evidence Log |
|---|---|---|---|---|---|
| **M6_recover_drops_typed_draft** | `agent_composer.rs` (`recover_undelivered_queue`) | Drop `self.draft` instead of appending non-empty `draft` to `user_texts` on start failure / early exit | **KILLED** (`rc=101`) | `start_window_keeps_every_written_entry_when_start_fails` | `plans/reports/agy-1437-queue-prompts-evidence/r2-mutant-m6.log` |
| **M6b_limit_drops_typed_draft** | `agent_composer.rs` (`tick_composer` skill-limit branch) | Clear `tab.composer.draft` instead of appending non-empty `draft` to `user_texts` on skill-limit refusal | **KILLED** (`rc=101`) | `flushing_updates_waiting_count_to_one_and_skill_limit_keeps_typed_draft` | `plans/reports/agy-1437-queue-prompts-evidence/r2-mutant-m6b.log` |
| **M7_no_start_window_on_failure** | `agent_composer.rs` (`recover_undelivered_queue`) | Omit `user_texts.append(&mut self.sent_in_start_window)` when recovering undelivered prompts | **KILLED** (`rc=101`) | `start_window_keeps_every_written_entry_when_start_fails` | `plans/reports/agy-1437-queue-prompts-evidence/r2-mutant-m7.log` |
| **M9_held_replay_skipped_if_queue_nonempty** | `terminal.rs` (`spawn_tab_with_registration`) | Skip replaying `pending_prompt` when `pending_send` is already non-empty | **KILLED** (`rc=101`) | `held_prompt_with_trailing_space_and_refused_held_prompt_replay_once_in_order`, `queued_first_message_then_spec_ask_queues_both_in_order` | `plans/reports/agy-1437-queue-prompts-evidence/r2-mutant-m9.log` |
| **M15_no_notice_after_flush** | `agent_composer.rs` (`tick_composer` post-flush notice) | Disable post-flush waiting-count notice update (`if false && remaining >= 2`) | **KILLED** (`rc=101`) | `flushing_updates_waiting_count_to_one_and_skill_limit_keeps_typed_draft` | `plans/reports/agy-1437-queue-prompts-evidence/r2-mutant-m15.log` |
| **M16_no_caption_with_draft** | `agent_composer_text.rs` (`caption`) | Disable waiting-count caption when `draft` is non-empty (`if false && !comp.pending_send.is_empty() && !comp.draft.is_empty()`) | **KILLED** (`rc=101`) | `flushing_updates_waiting_count_to_one_and_skill_limit_keeps_typed_draft` | `plans/reports/agy-1437-queue-prompts-evidence/r2-mutant-m16.log` |

### 6. Acceptance / Re-Anchor Table (Round 2)

| AC | Criterion | Status | Evidence |
|---|---|---|---|
| **AC 1** | Each of the 7 new Round-2 tests is RED on `22d129ea` and GREEN on HEAD | **PASS** | `r2-red-tests.log` (`0 passed; 7 failed`), `r2-green-tests.log` (`15 passed; 0 failed`) |
| **AC 2** | Gates: `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings` (only `main`'s 2 `needless_return` lints in `agent_permissions.rs:1176,1194`); `designos` test filters `agent` (`867 -> 887`), `composer` (`80 -> 87`), `terminal` (`331 -> 331`); `cargo test --workspace --locked --lib` (`1,476 passed; 0 failed`) | **PASS** | `r2-clippy.log`, `r2-gate-agent.log`, `r2-gate-composer.log`, `r2-gate-terminal.log`, `r2-gate-workspace-lib.log` |
| **AC 3** | CPU raster of composer with 2 waiting messages (`r2-composer.png`) inspected and verified | **PASS** | `plans/reports/agy-1437-queue-prompts-evidence/r2-composer.png` (`914x520` @ 2x, shows `"2 messages waiting to send"`) |
| **AC 4** | Pushed to `origin/jangtrinh/agy-1437-queue-prompts`, `git log --oneline origin/main..HEAD` pasted, `out/cargo` deleted | **PASS** | See Section 8 below |

### 7. Assumptions Held (Epistemic Ledger) & Weakest Link
- **[FACT]** `ComposerState::sent_in_start_window` is now `Vec<String>` and preserves every user prompt written within `START_FAILURE_WINDOW` in FIFO order on start failure (`start_window_keeps_every_written_entry_when_start_fails`).
- **[INFERENCE]** For hookless CLIs (or tabs whose hook records are unavailable), `awaiting_turn_end` relies on post-`\r` PTY output arriving (`out_at >= enter_at || output_at_enter != Some(out_at)`) and settling for `>= READY_QUIET_OUTPUT` (180 ms).
- **Weakest Links Identified in Round-2 Review (resolved in Round 3 below):**
  1. Clearing `flush_paused` on any `AgentTabStatus::Working(_)` allowed a tool-use hook record (`PreToolUse` / `PostToolUse`) after a permission prompt was answered to unpause the queue before the held line was ever submitted (`r2-MAJOR-1`).
  2. Writing the image job's label into `tab.agent_status` in `submit_system_image_prompt` blocked a user prompt queued ahead of an image job (`r2-MAJOR-2`) and overwrote a real running turn's `Working` status so an image job arriving mid-turn pasted early (`r2-LOW-2`).

### 8. Pasted Commands & Exact Outputs (Round 2)

```bash
cargo fmt --all -- --check
# Exit code: 0

S=$(mktemp -d "/private/tmp/x.XXXXXX")
mkdir -p "$S/home" "$S/tmp"
HOME="$S/home" TMPDIR="$S/tmp/" CARGO_TARGET_DIR="$PWD/out/cargo" \
  cargo clippy --workspace --all-targets --locked -- -D warnings
# Only main's 2 pre-existing needless_return warnings in crates/designos-app/src/agent_permissions.rs:1176,1194 (logged in r2-clippy.log)

HOME="$S/home" TMPDIR="$S/tmp/" CARGO_TARGET_DIR="$PWD/out/cargo" \
  cargo test -p designos-app --bin designos --locked agent
# test result: ok. 887 passed; 0 failed; 3 ignored; 0 measured; 3162 filtered out; finished in 97.82s (Round 1: 867 passed)

HOME="$S/home" TMPDIR="$S/tmp/" CARGO_TARGET_DIR="$PWD/out/cargo" \
  cargo test -p designos-app --bin designos --locked composer
# test result: ok. 87 passed; 0 failed; 0 ignored; 0 measured; 3965 filtered out; finished in 40.68s (Round 1: 80 passed)

HOME="$S/home" TMPDIR="$S/tmp/" CARGO_TARGET_DIR="$PWD/out/cargo" \
  cargo test -p designos-app --bin designos --locked terminal
# test result: ok. 331 passed; 0 failed; 1 ignored; 0 measured; 3720 filtered out; finished in 97.29s (Round 1: 331 passed)

HOME="$S/home" TMPDIR="$S/tmp/" CARGO_TARGET_DIR="$PWD/out/cargo" \
  cargo test --workspace --locked --lib
# 1,476 passed; 0 failed; 3 ignored across all workspace library crates (logged in r2-gate-workspace-lib.log)
```

```bash
git log --oneline origin/main..HEAD
f53533fb fix(agents): wait for turn end on queued sends and preserve start-window prompts (#1437)
2b61c790 Merge remote-tracking branch 'origin/main' into jangtrinh/agy-1437-queue-prompts
b91f561a docs(reports): add round 1 report and evidence for #1437
22d129ea fix(agents): queue a second send behind the waiting prompt instead of replacing it (#1437)
```

Status: DONE

---

## Round 3

### 1. What Could Not Be Verified (and Why)
- **Live external agent CLIs (`claude`, `codex`, `gemini`, `grok`, `agy`, `opencode`)**: Per the hermetic testing contract (`brief-common-argon.txt`), real external AI CLIs were never launched. All Round-3 turn-start hook record transitions (`PermissionRequest` / `PostToolUse` / `UserPromptSubmit` / `Stop` via `run_session_hook`), image-job composer premarking (`composer.premark`), Shell-tab refusal draft preservation, and composer Sends while Enter is held were verified deterministically using local shell stubs (`#!/bin/sh`) and CPU rasterization under isolated sibling `HOME` and `TMPDIR` folders.

### 2. Skills Loaded
- `es-astra-work` (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`)
- `es:fable-thinking` (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`)
- `es:rust` (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`)
- `es:rust-gpu-canvas` (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`)
- `es:dev` (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`)
- `es:feature-dev` (`/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`)

### 3. Summary of Round-3 Rulings Implemented
- **Ruling 1 — `r2-MAJOR-1` (Single `flush_paused` gate cleared only on turn-start records):**
  - Made `tab.composer.flush_paused` the single gate for the held-Enter pause (`crates/designos-app/src/agent_composer.rs:435-437, 501-511`). When `write_delayed_enter` holds back `\r` (`holds_the_enter(tab)`), it sets `tab.composer.flush_paused = true` and `tab.composer.awaiting_turn_end = false`.
  - Removed `tab.composer.flush_paused = false` from `is_tab_ready_for_queued_send`'s `Working` arm (`agent_composer.rs:439-444`), and in `poll_live_agent_bridge` (`crates/designos-app/src/terminal.rs:3883-3887`) gated `flush_paused = false; awaiting_turn_end = true; turn_started_since_send = true;` on `is_turn_start && self.tabs[tab_idx].composer.pending_enter_deadline.is_none()` (`UserPromptSubmit` / `PreInvocation`). Tool records (`PreToolUse` / `PostToolUse`) and status pokes never end the held-Enter pause.
  - Updated `held_enter_pauses_flushing_until_next_turn_starts_and_ends` (`agent_composer.rs:3415-3465`) to drive hook records via `run_session_hook` (`PermissionRequest` -> hold -> `PostToolUse` -> `Stop` [entry two not written] -> `\r` + `UserPromptSubmit` -> `Stop` [entry two written after `\r`]).
- **Ruling 2 — `r2-MAJOR-2` & `r2-LOW-2` (`composer.premark: Option<String>` instead of writing into `agent_status`):**
  - Added `pub premark: Option<String>` to `ComposerState` (`crates/designos-app/src/agent_composer.rs:176-178`), cleared in `reset_send_receipt` (`agent_composer.rs:352`) and when the last queued system prompt is written (`agent_composer.rs:572-574`).
  - Updated `submit_system_image_prompt` (`crates/designos-app/src/terminal.rs:3369-3375`) to record `tab.composer.premark = Some(prompt.chars().take(64).collect())` instead of writing `AgentTabStatus::Working(...)` into `tab.agent_status`, and removed `premarked_for_front` from `is_tab_ready_for_queued_send`.
  - Added regression tests `image_job_queued_behind_user_prompt_at_start_flushes_both_in_order` and `image_job_submitted_during_running_turn_waits_for_turn_end`.
- **Ruling 3 — `r2-LOW-1` (Keep entry-point text in composer draft on Shell-tab refusal):**
  - Updated `deliver_to_agent` (`crates/designos-app/src/agent_composer.rs:871-885, 995-1007`) so when a non-system prompt is refused with `RefuseReason::NoAgent` over a Shell tab, `text` is preserved in `comp.draft` (or appended with `"\n\n"` when `!from_box` and `draft` is non-empty).
  - Added regression test `entry_point_refused_over_shell_tab_with_no_startable_agent_keeps_draft`.
- **Ruling 4 — `r2-LOW-3` (Queue composer Send while Enter is held):**
  - Updated `must_queue` in `deliver_to_agent` (`crates/designos-app/src/agent_composer.rs:901-904`) to include `tab.composer.flush_paused` so a composer `Send` while an Enter is held queues behind the held line instead of pasting onto the unsubmitted line.
  - Added regression test `composer_send_while_enter_is_held_queues_behind_held_line`.
- **Ruling 5 — `r2-MINOR-2` (Kill mutants N6, N7, N9):**
  - `held_enter_pauses_flushing_until_next_turn_starts_and_ends` (record-driven via `run_session_hook`) kills both **N6** and **N7**; `image_job_queued_behind_user_prompt_at_start_flushes_both_in_order` exercises the hook-less `out_at < enter_at && output_at_enter != Some(out_at)` branch and kills **N9**.
- **Ruling 6 — `r2-MINOR-1` (7 report & PR-body items):**
  - Corrected all 6 report items in Round 1 and Round 2 above (`shell.rs:443-449` fixture description, `READY_QUIET_OUTPUT = 180 ms`, Ruling 4 label, Round-2 mutant table matching `mutants_r1.py`, weakest-link note, and `f53533fb` in `git log`), and updated the PR #1461 body for a public reader with no internal symbol names and accurate send/queue behavior.
- **Ruling 7 — `r2-LOW-4`:** No action.

### 4. Acceptance 1 — Red -> Green Test Output (Round 3)

#### Red Phase (on `f53533fb` before Round-3 production fixes)
Log: `plans/reports/agy-1437-queue-prompts-evidence/r3-red-tests.log`
```text
=== RED CHECK: agent_composer::tests::held_enter_pauses_flushing_until_next_turn_starts_and_ends ===
running 1 test
thread 'agent_composer::tests::held_enter_pauses_flushing_until_next_turn_starts_and_ends' (62137498) panicked at crates/designos-app/src/agent_composer.rs:3432:9:
entry two must not be pasted after PostToolUse + Stop while entry one is unsubmitted: "\u{1b}[200~entry one\u{1b}[201~\u{1b}[200~entry two\u{1b}[201~\r"
test agent_composer::tests::held_enter_pauses_flushing_until_next_turn_starts_and_ends ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 4055 filtered out; finished in 0.27s

=== RED CHECK: agent_composer::tests::image_job_queued_behind_user_prompt_at_start_flushes_both_in_order ===
running 1 test
thread 'agent_composer::tests::image_job_queued_behind_user_prompt_at_start_flushes_both_in_order' (62153283) panicked at crates/designos-app/src/agent_composer.rs:3472:9:
user prompt at start must flush on first ready tick: ""
test agent_composer::tests::image_job_queued_behind_user_prompt_at_start_flushes_both_in_order ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 4055 filtered out; finished in 0.43s

=== RED CHECK: agent_composer::tests::image_job_submitted_during_running_turn_waits_for_turn_end ===
running 1 test
thread 'agent_composer::tests::image_job_submitted_during_running_turn_waits_for_turn_end' (62163828) panicked at crates/designos-app/src/agent_composer.rs:3507:9:
assertion `left == right` failed: submitting an image job must not overwrite a running turn's Working status
  left: Working(Some("Image task 8: Upscale."))
 right: Working(Some("Reading the file…"))
test agent_composer::tests::image_job_submitted_during_running_turn_waits_for_turn_end ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 4055 filtered out; finished in 0.24s

=== RED CHECK: agent_composer::tests::entry_point_refused_over_shell_tab_with_no_startable_agent_keeps_draft ===
running 1 test
thread 'agent_composer::tests::entry_point_refused_over_shell_tab_with_no_startable_agent_keeps_draft' (62179505) panicked at crates/designos-app/src/agent_composer.rs:3547:9:
assertion `left == right` failed: refused entry-point prompt must remain in the composer draft
  left: ""
 right: "pricing table"
test agent_composer::tests::entry_point_refused_over_shell_tab_with_no_startable_agent_keeps_draft ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 4055 filtered out; finished in 0.23s

=== RED CHECK: agent_composer::tests::composer_send_while_enter_is_held_queues_behind_held_line ===
running 1 test
thread 'agent_composer::tests::composer_send_while_enter_is_held_queues_behind_held_line' (62184520) panicked at crates/designos-app/src/agent_composer.rs:3581:9:
assertion `left == right` failed: a composer Send while an Enter is held must queue behind the held line
  left: Written
 right: Queued
test agent_composer::tests::composer_send_while_enter_is_held_queues_behind_held_line ... FAILED
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 4055 filtered out; finished in 0.22s
```

#### Green Phase (on HEAD with Round-3 production fixes)
Log: `plans/reports/agy-1437-queue-prompts-evidence/r3-green-tests.log`
```text
=== GREEN CHECK: agent_composer::tests::held_enter_pauses_flushing_until_next_turn_starts_and_ends ===
running 1 test
test agent_composer::tests::held_enter_pauses_flushing_until_next_turn_starts_and_ends ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4055 filtered out; finished in 3.15s

=== GREEN CHECK: agent_composer::tests::image_job_queued_behind_user_prompt_at_start_flushes_both_in_order ===
running 1 test
test agent_composer::tests::image_job_queued_behind_user_prompt_at_start_flushes_both_in_order ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4055 filtered out; finished in 0.26s

=== GREEN CHECK: agent_composer::tests::image_job_submitted_during_running_turn_waits_for_turn_end ===
running 1 test
test agent_composer::tests::image_job_submitted_during_running_turn_waits_for_turn_end ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4055 filtered out; finished in 0.46s

=== GREEN CHECK: agent_composer::tests::entry_point_refused_over_shell_tab_with_no_startable_agent_keeps_draft ===
running 1 test
test agent_composer::tests::entry_point_refused_over_shell_tab_with_no_startable_agent_keeps_draft ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4055 filtered out; finished in 0.32s

=== GREEN CHECK: agent_composer::tests::composer_send_while_enter_is_held_queues_behind_held_line ===
running 1 test
test agent_composer::tests::composer_send_while_enter_is_held_queues_behind_held_line ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4055 filtered out; finished in 1.95s
```

### 5. Acceptance 2 — Reviewer Probes on HEAD & Mutation Tests (`M0_null`, `N6`, `N7`, `N9`)

#### Reviewer Probes Mounted on HEAD (and Removed Before Commit)
Log: `plans/reports/agy-1437-queue-prompts-evidence/r3-reviewer-probes.log`
```text
running 6 tests
test agent_composer::tests::rvw2_direct_send_after_a_held_enter ... PROBE r2_direct o1=Written o2=Queued written="\u{1b}[200~entry one\u{1b}[201~" enter_between=false
ok
test agent_composer::tests::rvw2_entry_point_over_a_shell_with_no_startable_agent ... PROBE r2_shell_noagent picker_open=false active=0 last_outcome=Some(Refused(NoAgent)) notice=Some("Select an agent tab") status=Some("Select an agent tab") pending_prompt=None text_kept_anywhere=true
ok
test agent_composer::tests::rvw2_held_enter_then_permission_answered_and_turn_ends ... PROBE r2_held_answer held_after_hold=true written="\u{1b}[200~entry one\u{1b}[201~" enter_between=false
ok
test agent_composer::tests::rvw2_image_job_behind_a_waiting_user_prompt_at_start ... PROBE r2_image_stall tabs=1 queue_at_submit=["user prompt first", "Image task 4: Remove background."] user_written_after_ticks=Some(0) status=Idle written="\u{1b}[200~user prompt first\u{1b}[201~\r"
ok
test agent_composer::tests::rvw2_image_job_during_a_running_turn ... PROBE r2_image_midturn tabs=1 pasted_while_turn_one_runs_after_ticks=None written="\u{1b}[200~turn one\u{1b}[201~\r"
ok
test agent_composer::tests::rvw2_render_r3_composer_png ... PROBE r3_raster texts=["2 messages waiting to send", "Ask first", "Claude Code", "Answer the permission prompt in the terminal"]
PROBE r3_raster saved "/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1437-queue-prompts/crates/designos-app/../../plans/reports/agy-1437-queue-prompts-evidence/r3-composer.png" (914x520)
ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 4056 filtered out; finished in 2.08s
```

#### Null Control & Mutants N6, N7, N9 (Run & Reverted)
Log: `plans/reports/agy-1437-queue-prompts-evidence/r3-mutants.log`
```text
=== M0_null (SURVIVED, rc=0) ===
   test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4055 filtered out; finished in 1.34s
=== N6_no_pause_flag_on_hold (KILLED, rc=101) ===
   test agent_composer::tests::held_enter_pauses_flushing_until_next_turn_starts_and_ends ... FAILED
   test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 4055 filtered out; finished in 0.37s
   thread 'agent_composer::tests::held_enter_pauses_flushing_until_next_turn_starts_and_ends' (62509047) panicked at crates/designos-app/src/agent_composer.rs:3444:9:
=== N7_terminal_never_clears_pause (KILLED, rc=101) ===
   test agent_composer::tests::held_enter_pauses_flushing_until_next_turn_starts_and_ends ... FAILED
   test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 4055 filtered out; finished in 1.16s
   thread 'agent_composer::tests::held_enter_pauses_flushing_until_next_turn_starts_and_ends' (62523948) panicked at crates/designos-app/src/agent_composer.rs:3460:39:
=== N9_no_output_at_enter_fallback (KILLED, rc=101) ===
   test agent_composer::tests::image_job_queued_behind_user_prompt_at_start_flushes_both_in_order ... FAILED
   test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 4055 filtered out; finished in 0.17s
   thread 'agent_composer::tests::image_job_queued_behind_user_prompt_at_start_flushes_both_in_order' (62542629) panicked at crates/designos-app/src/agent_composer.rs:3496:65:
```

| Mutant | File & Site | Mutation Applied | Result | Killing Test | Evidence Log |
|---|---|---|---|---|---|
| **M0_null** | None (null control) | Unmodified HEAD | **PASS** (`rc=0`) | — | `plans/reports/agy-1437-queue-prompts-evidence/r3-mutants.log` |
| **N6_no_pause_flag_on_hold** | `agent_composer.rs` (`write_delayed_enter`) | Delete `tab.composer.flush_paused = true;` | **KILLED** (`rc=101`) | `held_enter_pauses_flushing_until_next_turn_starts_and_ends` | `plans/reports/agy-1437-queue-prompts-evidence/r3-mutants.log` |
| **N7_terminal_never_clears_pause** | `terminal.rs` (`poll_live_agent_bridge`) | Replace `if is_turn_start && ...` with `if false && is_turn_start && ...` | **KILLED** (`rc=101`) | `held_enter_pauses_flushing_until_next_turn_starts_and_ends` | `plans/reports/agy-1437-queue-prompts-evidence/r3-mutants.log` |
| **N9_no_output_at_enter_fallback** | `agent_composer.rs` (`is_tab_ready_for_queued_send`) | Replace `(out_at >= enter_at \|\| tab.composer.output_at_enter != Some(out_at))` with `(out_at >= enter_at)` | **KILLED** (`rc=101`) | `image_job_queued_behind_user_prompt_at_start_flushes_both_in_order` | `plans/reports/agy-1437-queue-prompts-evidence/r3-mutants.log` |

### 6. Acceptance 3, 4 & 5 — Re-Anchor Table & Gate Outputs (Round 3)

| AC | Criterion | Status | Evidence |
|---|---|---|---|
| **AC 1** | Each new/updated Round-3 test is RED on `f53533fb` and GREEN on HEAD | **PASS** | `r3-red-tests.log` (`5 failed`), `r3-green-tests.log` (`5 passed; 0 failed`) |
| **AC 2** | Null control (`M0_null`) and mutants `N6`, `N7`, `N9` run and reverted; reviewer probes verified on HEAD | **PASS** | `r3-mutants.log`, `r3-reviewer-probes.log` |
| **AC 3** | Gates: `cargo fmt --all -- --check` (`rc=0`); `cargo clippy --workspace --all-targets --locked -- -D warnings` (only `main`'s 2 `agent_permissions.rs:1176,1194` lints); `cargo test -p designos-app --bin designos --locked composer` (`91 passed; 0 failed`); `cargo test --workspace --locked --lib` (`1,476 passed; 0 failed`) | **PASS** | `r3-fmt.log`, `r3-clippy.log`, `r3-gate-composer.log`, `r3-gate-workspace-lib.log` |
| **AC 4** | CPU raster of the composer with two waiting messages and the held-Enter notice (`r3-composer.png`) | **PASS** | `plans/reports/agy-1437-queue-prompts-evidence/r3-composer.png` (`914x520` @ 2x, shows `"2 messages waiting to send"` and `"Answer the permission prompt in the terminal"`) |
| **AC 5** | Pushed to `origin/jangtrinh/agy-1437-queue-prompts`; PR body updated; `git log --oneline origin/main..HEAD` pasted; `out/cargo` deleted | **PASS** | See Section 8 below |

### 7. Assumptions Held (Epistemic Ledger) & Weakest Link (Round 3)
- **[FACT]** `tab.composer.flush_paused` is the sole gate for the held-Enter pause: `write_delayed_enter` sets `flush_paused = true` and `awaiting_turn_end = false` on hold, `is_tab_ready_for_queued_send` returns `false` while `flush_paused` is `true` without clearing it on `Working`, and `poll_live_agent_bridge` clears `flush_paused = false` (and arms `awaiting_turn_end = true, turn_started_since_send = true`) only when `is_turn_start` (`UserPromptSubmit` / `PreInvocation`) is true and `pending_enter_deadline.is_none()`.
- **[FACT]** `submit_system_image_prompt` records its 64-char label on `tab.composer.premark` rather than mutating `tab.agent_status`, so queued user prompts ahead of an image job flush on the first ready tick and a running turn's `Working` status is never overwritten (`image_job_queued_behind_user_prompt_at_start_flushes_both_in_order`, `image_job_submitted_during_running_turn_waits_for_turn_end`).
- **[INFERENCE]** For a hookless CLI tab where a permission hold is impossible (since `holds_the_enter` requires either `tab.gemini_dialog_open()` or a hook-driven `WaitingForInput` status), turn completion continues to be detected via post-`\r` PTY output settling for `>= READY_QUIET_OUTPUT` (`180 ms`).
- **Weakest Link:** If a user manually edits or deletes the held text in the terminal instead of pressing Enter (cancelling the held prompt without ever triggering `UserPromptSubmit` / `PreInvocation`), `flush_paused` remains `true` until the user submits a turn in the terminal or starts a new tab.

### 8. Pasted Commands & Exact Outputs (Round 3)

```bash
cargo fmt --all -- --check
# fmt_rc=0 (logged in r3-fmt.log)

HOME="$S/home" TMPDIR="$S/tmp/" CARGO_TARGET_DIR="$PWD/out/cargo" \
  cargo clippy --workspace --all-targets --locked -- -D warnings
# Zero warnings in modified files; only main's 2 pre-existing needless_return lints in crates/designos-app/src/agent_permissions.rs:1176,1194 (logged in r3-clippy.log)

HOME="$S/home" TMPDIR="$S/tmp/" CARGO_TARGET_DIR="$PWD/out/cargo" \
  cargo test -p designos-app --bin designos --locked composer
# running 91 tests
# test result: ok. 91 passed; 0 failed; 0 ignored; 0 measured; 3965 filtered out; finished in 35.46s (logged in r3-gate-composer.log)

HOME="$S/home" TMPDIR="$S/tmp/" CARGO_TARGET_DIR="$PWD/out/cargo" \
  cargo test --workspace --locked --lib
# 1,476 passed; 0 failed; 3 ignored across all workspace library crates (logged in r3-gate-workspace-lib.log)
```

```bash
git log --oneline origin/main..HEAD
d7dc4258 fix(agents): gate held-enter unpause on turn start and move image premark to composer (#1437)
f53533fb fix(agents): wait for turn end on queued sends and preserve start-window prompts (#1437)
2b61c790 Merge remote-tracking branch 'origin/main' into jangtrinh/agy-1437-queue-prompts
b91f561a docs(reports): add round 1 report and evidence for #1437
22d129ea fix(agents): queue a second send behind the waiting prompt instead of replacing it (#1437)
```

Status: DONE
