# Report: #1437 Agents Composer — Queue a Second Send Behind the Waiting Prompt

## 1. Executive Summary
- Replaced `AgentComposer::pending_send: Option<String>` and `pending_send_is_system: bool` with an ordered FIFO queue `pending_send: VecDeque<QueuedSend>` (`QueuedSend { text: String, is_system: bool }`) so every write site appends and never overwrites a waiting prompt.
- Updated `tick_composer` (`is_tab_ready_for_queued_send`) to pop and write one entry per ready state (`pop_front()`), waiting until the previous turn's delayed Enter (`pending_enter_deadline`) completes before writing the next entry, while preserving all user entries in order joined by `"\n\n"` on failed start, early exit, or skill-limit refusal.
- Updated `spawn_custom_cli_tab` and `spawn_tab_with_registration` in `terminal.rs` so the entire `New chat` queue moves into the new tab, held `pending_prompt` replays append behind the queue without duplication, system prompts queue on starting/queued tabs instead of refusing with `RefuseReason::NotReady`, and two or more waiting entries show `"N messages waiting to send"`.

## 2. Files Changed
- `crates/designos-app/src/agent_composer.rs`
- `crates/designos-app/src/agent_composer_text.rs`
- `crates/designos-app/src/agent_open_flow.rs`
- `crates/designos-app/src/layout_snapshot.rs`
- `crates/designos-app/src/shell.rs`
- `crates/designos-app/src/shell/tests/agent_new_chat_composer.rs`
- `crates/designos-app/src/shell/tests/agent_simplified_panel.rs`
- `crates/designos-app/src/shell/tests/home.rs`
- `crates/designos-app/src/specs.rs`
- `crates/designos-app/src/terminal.rs`
- `crates/designos-app/src/terminal/gemini_composer_tests.rs`

## 3. Red -> Green Test Output

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

## 5. Gate Output
- `cargo fmt --all -- --check`: **PASS** (exit 0).
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: **PASS** for all touched files (zero warnings in touched files; only `main`'s 5 pre-existing warnings in `agent_permissions.rs:1163,1181` and `panels/components.rs`, logged in `plans/reports/agy-1437-queue-prompts-evidence/clippy.log`).
- `cargo test -p designos-app --bin designos --locked agent`: **PASS** (`867 passed; 0 failed; 3 ignored; finished in 175.10s`, logged in `plans/reports/agy-1437-queue-prompts-evidence/gate-agent.log`).
- `cargo test -p designos-app --bin designos --locked composer`: **PASS** (`80 passed; 0 failed; 0 ignored; finished in 20.48s`, logged in `plans/reports/agy-1437-queue-prompts-evidence/gate-composer.log`).
- `cargo test -p designos-app --bin designos --locked terminal`: **PASS** (`331 passed; 0 failed; 1 ignored; finished in 45.03s`, logged in `plans/reports/agy-1437-queue-prompts-evidence/gate-terminal.log`).
- `cargo test --workspace --locked --lib`: **PASS** (`0 failed` across all workspace library crates, logged in `plans/reports/agy-1437-queue-prompts-evidence/gate-workspace-lib.log`).

## 6. Visual Evidence
- Checked macOS `HIDIdleTime` before capture: `3599s` (`>= 300s`).
- Captured `out/evidence/1437-two-waiting.png` (also copied to `plans/reports/agy-1437-queue-prompts-evidence/1437-two-waiting.png`) using a scratch copy of `/Users/jangtrinh/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig` under the hermetic harness (`$TMPDIR/.designos-hermetic-active` verified with `pid=56520`, `dir=/Users/jangtrinh/orca/.s1437/bin`).
- Visually inspected `out/evidence/1437-two-waiting.png`: the Agents panel (`Claude Code #1`) displays `2 messages waiting to send` in the composer above the `+ / Claude Code / Ask first / ↑` control row.

## 7. Pull Request & Disk Cleanup
- **Draft PR URL:** https://github.com/jangtrinh/design-os-studio/pull/1461
- **Branch:** `jangtrinh/agy-1437-queue-prompts` (`22d129ea`)
- **Disk Cleanup Confirmation:** `rm -rf out/cargo` and temporary sibling stub directories (`/Users/jangtrinh/orca/.s1437` and `/Users/jangtrinh/orca/workspaces/design-os-studio/.stub-agy-1437-queue-prompts`) removed; `df -h /` reports `140Gi` free (`8%` used).
