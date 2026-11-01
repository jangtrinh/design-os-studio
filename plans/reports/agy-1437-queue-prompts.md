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
- `crates/designos-app/src/shell.rs` — Added `--panel agents-chat:two-waiting` CLI screenshot fixture (lines 714–724) so headless/windowed `--screenshot ... --panel agents-chat:two-waiting` deterministically opens the Agents panel with two queued prompts and `not_ready_gate = true`, allowing visual capture of the `"2 messages waiting to send"` state (`1437-two-waiting.png`) without racing the live PTY flush.
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
- **Ruling 3 — MAJOR-3 & Ruling Gap (readiness after a prompt is written & pause while Enter is held):** Added `flush_paused`, `awaiting_turn_end`, `turn_started_since_send`, `enter_written_at`, and `output_at_enter` to `ComposerState` (`crates/designos-app/src/agent_composer.rs`, `crates/designos-app/src/terminal.rs`). When `write_delayed_enter` holds back `\r` (`holds_the_enter(tab)`), it sets `flush_paused = true` and `awaiting_turn_end = true` so subsequent queued entries are not pasted behind the unsubmitted prompt. When `AgentTabStatus::Working(_)` is observed after `\r` (excluding a pre-marked `submit_system_image_prompt` label for the front system entry itself), `flush_paused` clears and `turn_started_since_send` becomes `true`. While `awaiting_turn_end` is `true`, `is_tab_ready_for_queued_send` never falls back to `spawned_at.elapsed() >= START_FAILURE_WINDOW`; it returns `true` only once `turn_started_since_send` is true and status is no longer `Working`/`WaitingForInput`, or once post-`\r` PTY output has arrived (`out_at >= enter_at || output_at_enter != Some(out_at)`) and been quiet for `>= READY_QUIET_OUTPUT` (150 ms).
- **Ruling 4 — MINOR-4 (Commit Round-1 report & evidence, merge `origin/main`, complete report):** Committed Round-1 report and evidence (`b91f561a`), merged `origin/main` with a merge commit (`1b64db96`), completed the Round-1 report sections and `--panel agents-chat:two-waiting` disclosure, and appended `## Round 2`.
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
| **M6** | `agent_composer.rs` (`tick_composer` skill-limit branch) | Replace `for tail in tab.composer.pending_send.drain(..)` with `tab.composer.pending_send.clear()` | **KILLED** (`rc=101`) | `skill_limit_refusal_on_entry_2_of_3_returns_entries_2_and_3_to_draft`, `flushing_updates_waiting_count_to_one_and_skill_limit_keeps_typed_draft` | `plans/reports/agy-1437-queue-prompts-evidence/r2-mutant-m6.log` |
| **M6b** | `agent_composer.rs` (`tick_composer` skill-limit branch) | Clear `tab.composer.draft` instead of appending non-empty `draft` to `user_texts` | **KILLED** (`rc=101`) | `flushing_updates_waiting_count_to_one_and_skill_limit_keeps_typed_draft` | `plans/reports/agy-1437-queue-prompts-evidence/r2-mutant-m6b.log` |
| **M7** | `agent_composer.rs` (`tick_composer` post-flush notice) | Disable post-flush waiting-count notice update (`if false && ...`) | **KILLED** (`rc=101`) | `flushing_updates_waiting_count_to_one_and_skill_limit_keeps_typed_draft` | `plans/reports/agy-1437-queue-prompts-evidence/r2-mutant-m7.log` |
| **M9** | `agent_composer.rs` (`is_tab_ready_for_queued_send`) | Remove `AgentTabStatus::Working(_)` refusal, `flush_paused`, and `awaiting_turn_end` turn-end guard | **KILLED** (`rc=101`) | `working_agent_waits_for_turn_end_before_flushing_next_entry`, `held_enter_pauses_flushing_until_next_turn_starts_and_ends` | `plans/reports/agy-1437-queue-prompts-evidence/r2-mutant-m9.log` |
| **M15** | `agent_composer.rs` (`deliver_to_agent`) | Replace `!covered_agent` with `self.picker_open` in spawn condition | **KILLED** (`rc=101`) | `send_on_an_exited_tab_beside_a_live_one_starts_a_new_agent`, `entry_point_in_new_chat_over_a_shell_tab_starts_agent_and_sends` | `plans/reports/agy-1437-queue-prompts-evidence/r2-mutant-m15.log` |
| **M16** | `terminal.rs` (`dispatch_figma_ai_prompt` & `spawn_tab`) | Restore unconditional `pending_prompt = Some(...)` and `already_queued` exact-string check | **KILLED** (`rc=101`) | `held_prompt_with_trailing_space_and_refused_held_prompt_replay_once_in_order` | `plans/reports/agy-1437-queue-prompts-evidence/r2-mutant-m16.log` |

### 6. Acceptance / Re-Anchor Table (Round 2)

| AC | Criterion | Status | Evidence |
|---|---|---|---|
| **AC 1** | Each of the 7 new Round-2 tests is RED on `22d129ea` and GREEN on HEAD | **PASS** | `r2-red-tests.log` (`0 passed; 7 failed`), `r2-green-tests.log` (`15 passed; 0 failed`) |
| **AC 2** | Gates: `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings` (only `main`'s 2 `needless_return` lints in `agent_permissions.rs:1176,1194`); `designos` test filters `agent` (`867 -> 887`), `composer` (`80 -> 87`), `terminal` (`331 -> 331`); `cargo test --workspace --locked --lib` (`1,476 passed; 0 failed`) | **PASS** | `r2-clippy.log`, `r2-gate-agent.log`, `r2-gate-composer.log`, `r2-gate-terminal.log`, `r2-gate-workspace-lib.log` |
| **AC 3** | CPU raster of composer with 2 waiting messages (`r2-composer.png`) inspected and verified | **PASS** | `plans/reports/agy-1437-queue-prompts-evidence/r2-composer.png` (`914x520` @ 2x, shows `"2 messages waiting to send"`) |
| **AC 4** | Pushed to `origin/jangtrinh/agy-1437-queue-prompts`, `git log --oneline origin/main..HEAD` pasted, `out/cargo` deleted | **PASS** | See Section 8 below |

### 7. Assumptions Held (Epistemic Ledger) & Weakest Link
- **[FACT]** `ComposerState::sent_in_start_window` is now `Vec<String>` and preserves every user prompt written within `START_FAILURE_WINDOW` in FIFO order on start failure (`start_window_keeps_every_written_entry_when_start_fails`).
- **[FACT]** `submit_system_image_prompt` in `terminal.rs:3374` pre-sets `tab.agent_status = AgentTabStatus::Working(Some(prompt.chars().take(64).collect()))` before calling `send_system_prompt`; `is_tab_ready_for_queued_send` checks `premarked_for_front` so that pre-marked status does not block its own queued system entry from flushing once the tab is quiet (`r2-gate-composer.log`: `87 passed; 0 failed`).
- **[INFERENCE]** For hookless CLIs (or tabs whose hook records are unavailable), `awaiting_turn_end` relies on post-`\r` PTY output arriving (`out_at >= enter_at || output_at_enter != Some(out_at)`) and settling for `>= READY_QUIET_OUTPUT` (150 ms).
- **Weakest Link:** A hookless CLI that produces zero bytes of PTY output after `\r` (not even a newline echo) and subsequently pauses for more than 150 ms mid-turn before emitting output would look indistinguishable from a completed turn; hooked CLIs (`Claude`, `Gemini`) avoid this via `AgentTabStatus::Working(_)` tracking (`turn_started_since_send`).

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
aec0c07d fix(agents): wait for turn end on queued sends and preserve start-window prompts (#1437)
2b61c790 Merge remote-tracking branch 'origin/main' into jangtrinh/agy-1437-queue-prompts
b91f561a docs(reports): add round 1 report and evidence for #1437
22d129ea fix(agents): queue a second send behind the waiting prompt instead of replacing it (#1437)
```

Status: DONE
