# #684 Stop an agent from the panel: interrupt the CLI, close its open edit batch as one undo step, and record that it was stopped

| Field | Value |
|---|---|
| Issue | #684 (parent #585) |
| Size | M |
| Depends on | #589 (batch undo group), #592 (in-progress flag), #677 (turn marks and ledger); soft: #572 (parallel agents, "Stop all"), #683 (the stopped state and receipt) |
| Unblocks | none |
| Touches | new `crates/designos-app/src/agent_stop.rs` (interrupt table, stop sequence, merge of the turn's steps), `crates/designos-doc/src/history.rs` (one function to fold steps), `crates/designos-app/src/terminal.rs` (hot spot: header strip, key handling `:3699-3790`, `write_bytes` `:566`), `crates/designos-app/src/live_patch_apply.rs`, tests |

Code baseline: `origin/main@9f3f7a5`. The Grok facts come from its bundled user guide (`~/.grok/docs/user-guide/10-hooks.md`, read 2026-10-19).

## 1. Summary and user story

As a designer, when an agent goes the wrong way I want one Stop button in the panel. The agent stops, everything it already did stays and undoes as one step, and the receipt says I stopped it.

## 2. Current state (FACT)

- No Stop control exists. The tab strip has the `+` menu and the close button (`terminal.rs:2515-2595`); the dock bar has "Send Selection & Comments" and other pills (`:3466-3482`).
- Raw bytes reach a tab through `TerminalTab::write_bytes` (`:566`). The terminal sends Esc as `0x1b` when it has focus and no search is open (`:3772-3777`), and Ctrl+C as `0x03` only when nothing is selected, in the Ctrl branch (`:3749-3756`).
- On Windows and Linux egui sets `modifiers.command` equal to `ctrl` (`egui-0.35.0/src/data/input/modifiers.rs:33-37`). The key map tests `modifiers.command` first (`:3724`), so there Ctrl+C reaches the Ctrl branch never: the interrupt key is not forwarded by hand-typing on those systems. Stop sends bytes itself and does not depend on that map.
- Agent edits apply as one undo step per patch (`live_patch_apply.rs:112-132`); a patch waits while the user is mid-edit (`:50-68`). `History` is a linear stack of inverse commands with `undo_depth` and `drop_undo_steps_after` (`designos-doc/src/history.rs:206-256`) and a `Command::Batch(Vec<Command>)` variant (`history.rs:74`). Nothing folds several steps into one.
- #677 (not on `main`) groups patches into turns with hook marks and keeps a ledger with the undo depth. Its rule 10 leaves Ctrl+Z undoing single agent steps.
- Grok: Ctrl+C interrupts a turn; "`Esc` never cancels a running turn" (guide, hooks chapter line 478). An interrupt fires `StopCancelled` with reason `user_interrupt`.
- UNKNOWN: which key interrupts a running turn in Claude Code, Codex and Antigravity. The issue says "Esc for TUIs that use it, otherwise Ctrl+C". Check: read each CLI's own help or docs, as part 3 did, and record the source in the table below.
- UNKNOWN: whether a second Ctrl+C at an idle prompt exits a CLI. For Claude Code and Codex this is believed, not verified. The rule below sends nothing at an idle prompt.
- #592 (in-progress flags) and #589 are not on `main`; their final APIs are UNKNOWN.

## 3. Behaviour rules

1. **Stop button** in the panel header, shown only while the active tab's turn is running (status `Working`). Esc triggers the same stop only when focus is in the panel chrome (header, composer, tab strip) and not in the terminal. Inside the terminal, Esc is a CLI key and is forwarded unchanged. "Chrome focus" is a flag set by a press inside the panel's non-terminal area and cleared by a press elsewhere.
2. **Interrupt table** (data, one row per CLI): first key, second key, source, grounded. Grok: Ctrl+C then Ctrl+C, grounded (guide). Claude: Esc then Ctrl+C, not grounded. Codex and Antigravity: Esc then Ctrl+C, not grounded. A row marked not grounded is shown in the report of the PR and flipped only by a captured test run. The table is the one place the keys live.
3. **Sequence.** At t = 0 send the first key. At t = 3 s, if the turn is still running (status read at that moment), send Ctrl+C once. Never send a second key when the turn already ended. After a second key the button reads "Stopping…" and stays disabled; no third key is ever sent.
4. **Close the turn.** Treat the stop like a turn end:
   - apply every patch already in the log and every held patch, unless the user is mid-edit (then as soon as the edit ends), for up to 2 s after the turn end mark or process exit;
   - fold the turn's agent steps on top of the undo stack into one `Command::Batch` step labelled "Agent turn (stopped)" through a new `History::fold_since(depth)`; the depth is the one #677 recorded at the turn start. The fold stops at the first step that is not an agent step (a user edit in between); only the contiguous agent tail is folded;
   - clear the in-progress flags of the agent's frames (#592);
   - write the receipt `Stopped by you after 42s` (#683).
5. **Late writes.** A tool call that completes after the close lands as a normal single step and shows in the next receipt. It is never dropped.
6. **Parallel agents** (#572): the header shows "Stop all" beside the per-tab Stop whenever two or more tabs run. "Stop all" runs the sequence for every running tab; each tab folds its own steps.
7. **Idle and shell tabs.** A shell tab has no Stop. A tab whose turn is not running shows no Stop. Heuristic-status tabs (Codex) show Stop while the heuristic says running.
8. A tab that exits during the sequence ends it: the receipt says "Stopped by you after Ns" only if the stop was sent before the exit.

## 4. Data model / schema changes

- `StopState { sent_at, second_sent, key_table_row }` per tab in memory. `History::fold_since(depth, label) -> usize` in `designos-doc` (returns the number of folded steps).
- No file or log format change. The turn outcome `Stopped` is part of #683's receipt type.

## 5. Contracts

- None toward agents: Stop writes keys to the PTY like a person would.
- `designos-doc` public API gains `History::fold_since`. Redo behaviour: folding clears nothing; the redo stack is empty at that point (the turn just ended) and the fold refuses if it is not.

## 6. File-format mapping

None.

## 7. UI

Described in words; checked against `design/principles.md` P1 to P10 and `design/soul.md`.

- **Stop** is a secondary button, `CONTROL_STANDARD` (28 px) high, `RADIUS_CONTROL`, in the panel header strip at the right, with the label "Stop" and a square glyph. It is not the primary action: orange stays with Send. A pointer target of at least `TARGET_MIN` 24 px. Hover steps the fill to `BG_HOVER`. While stopping it reads "Stopping…" at `TEXT_SECONDARY`.
- **Stop all** is a second secondary button of the same size, `SPACE_INLINE` (8 px) from the first.
- The stopped receipt uses #683's receipt line.
- No animation beyond the existing running spinner; no timer except one one-shot at t = 3 s.
- Desktop capture at 1440: Stop while running, Stop all with two agents, the stopped receipt. Responsive: SKIPPED (owner default). Load es-designer before UI code; update the Figma frames from the captures first.

## 8. Edge cases

1. The CLI ignores both keys: after the second key the tab shows "Still running. Close the tab to end it." Studio never kills the process on its own.
2. The user pressed Stop at the instant the turn ended: the sequence checks the status first and does nothing; the receipt is the normal one.
3. The user edits while stopping: their steps stay; the fold covers the contiguous agent tail only.
4. A turn of zero edits: the fold does nothing; the receipt still says stopped.
5. Esc pressed in the composer text field: egui uses it to leave the field; the stop fires only if the composer is not focused (chrome focus is the header or tab strip).
6. A restart-resumed tab (#653) that was running: no turn is known, no Stop.
7. Two Stop clicks: the second does nothing while `StopState` exists.

## 9. Non-goals

- Killing the CLI process. Rolling the turn back (that is #677's Revert). Pausing. Per-tool-call cancel.

## 10. Test plan (red first)

Seams: a stub CLI script that writes a patch every 100 ms and exits on a byte (`0x1b` or `0x03`), with an option to ignore the first; an injected clock for the 3 s; unit tests for `fold_since`.

| Case | Expected | Fails before |
|---|---|---|
| `stop_ends_a_looping_stub_within_three_seconds` | status leaves running by 3 s | yes (no control) |
| `second_key_is_sent_only_if_still_running` | stub that ignores the first key gets a second; a stub that exits gets none | yes |
| `no_key_is_sent_after_the_turn_ended` | zero bytes after `Done` | yes |
| `fold_since_makes_the_tail_one_undo_step` | `undo_depth` shrinks to start + 1; one undo restores the start hash | yes |
| `fold_stops_at_a_user_step` | user edit in between: steps before it stay separate | yes |
| `held_patches_apply_before_the_fold` | patches held for a user edit are in the document after the edit ends | yes |
| `in_progress_flags_clear_on_stop` | no node has the flag (needs #592) | yes |
| `receipt_says_stopped_by_you` | "Stopped by you after 42s" from fixed times | yes |
| `esc_in_the_terminal_reaches_the_cli_unchanged` | `0x1b` written, no stop | guard |
| `esc_in_the_chrome_stops` | stop sequence starts | yes |
| `stop_all_stops_every_running_tab` | two stubs stop | yes (needs #572) |

Run: `cargo test -p designos-doc history`, `cargo test -p designos-app agent_stop`. Evidence: red/green capture, the 1440 captures, an owner-manual stop of a real Claude and a real Grok turn (the table rows turn grounded only from that).

## 11. Performance budgets

- Fold: O(steps of the turn); under 5 ms for 1,000 steps. No document clone.
- No UI-thread `to_json`; the one-shot timer is the only wake-up added.

## 12. Dependencies and build order

After #589, #592 and #677. Part 4 merge order is in README-part4: #684 follows #683. `terminal.rs` gets call sites only; the table and sequence live in `agent_stop.rs`. `history.rs` is shared with part 1's E15-14 (`SetAutoFix`) and part 3's #677 reads `peek_undo`: add `fold_since` in its own commit.

## 13. Owner rulings

- Owner ruling 2026-10-19: a stopped turn folds into one undo step. A finished turn keeps its single steps, and #677's "Revert agent turn" is the one-step undo for finished turns.

- A Stop button in the panel header while a turn runs; Esc stops only from the chrome; the stop is Esc or Ctrl+C, retried once at 3 s; pending edits close as one undo step; the receipt records the stop; "Stop all" with parallel agents. (Issue text.)
- Skip-permissions stays the default.

## Open questions for the owner (recommended first)

(Answered by the owner 2026-10-19, see §13.) 1. "One undo step" versus #677's rule that Ctrl+Z undoes single agent steps. Recommended: a stopped turn folds into one step (the issue's acceptance test), and a finished turn keeps single steps; "Revert agent turn" is the one-step undo for finished turns.
2. The interrupt keys of Claude, Codex and Antigravity are not grounded. Recommended: ship the table with Grok grounded, and block the PR's "done" on one captured run per CLI.
