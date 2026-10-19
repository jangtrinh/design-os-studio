# #677 Agent tabs: review each agent turn's canvas edits and keep or revert them as one step

| Field | Value |
|---|---|
| Issue | #677 (owner request 2026-10-18, ruled for v0.6 on 2026-10-19; audit proposal 11) |
| Size / priority | L / P1 |
| Depends on | #653 (session folder), #651 (a live tab to attach to); E15-4 (batch writes become one agent step, optional); T10.2 three-way diff (optional, only if the owner wants property-level revert) |
| Unblocks | none |
| Touches | `crates/designos-doc/src/live_patch/mod.rs` (`LiveLogLine::TurnMark`), new `crates/designos-app/src/agent_turn_ledger.rs` (turns, fingerprints, revert plan), new `crates/designos-app/src/agent_turn_card.rs` (UI), `crates/designos-app/src/live_patch_apply.rs` (`apply_one`, `:112-132`), `crates/designos-app/src/live_patch_log.rs` (`poll`, `:92-120`), `crates/designos-app/src/terminal.rs` (hot spot: hook files `:2194-2233`, status polling `:1328-1338`), `crates/designos-doc/src/history.rs` (read the last undo step, `peek_undo` exists), tests |

Code baseline: `origin/main@01d48f2` (0.5.0). Hook facts: Claude from its hooks page (code.claude.com/docs/en/hooks, 2026-10-19), Grok from its bundled user
guide (`~/.grok/docs/user-guide/10-hooks.md`, grok 1.0.62), Antigravity from Studio's own hook file (`agy_session_hooks`, `terminal.rs:2225-2233`; no agy
hook documentation was read).

## 1. Summary and user story

As a designer, after the agent finishes a turn that changed my file, Studio shows me what that turn changed, as a list of layers I can click, and
offers Keep or Revert for the whole turn. Revert undoes exactly what the agent did in that turn, in one step, and leaves anything I did myself.

## 2. Current state (FACT)

- Agent edits arrive as live patches in `doc-patch.jsonl`, one line per MCP tool call (`LivePatch`, `designos-doc/src/live_patch/mod.rs:27-35`), mixed with
  document-switch lines (`DocEpoch`). The log reader yields `LiveLogLine::{Patch, DocEpoch}` in log order (`mod.rs:78-97`; `LivePatchLog::poll`,
  `live_patch_log.rs:92-120`).
- Studio replays each patch as its own undo step: `apply_one` runs the replay in a `Tx` and commits it to `app.hist` (`live_patch_apply.rs:112-132`);
  `apply_live_patches` does this for every patch of a poll (`:81-110`). A patch held while the user is mid-edit is applied later (`:50-68`).
  `History` is a linear stack of inverse commands (`designos-doc/src/history.rs:205-254`); `peek_undo` returns the inverse of the last step.
- Nothing groups steps by agent turn. E15-4 (batched writes as one transaction) and RD-3 (per-agent atomic undo) are v0.6 specs, not on main.
- Turn boundaries exist only in the CLIs' hooks, and Studio's hooks overwrite one status file: `status_hook_command` writes `{"state": ...}` to
  `agent-status.json` (`terminal.rs:2194-2196`); two events in one poll interval leave only the last, so this file cannot mark turns.
- Hook events per CLI (sources above):

| CLI | Turn start | Turn end | Notes |
|---|---|---|---|
| Claude Code | `UserPromptSubmit` (fires "also for turns Claude Code starts on its own") | `Stop`; `StopFailure` for an API error | `Stop` is not strictly once per turn: a Stop hook that blocks makes it fire again; Studio registers no blocking Stop hook |
| Grok 1.0.62 | `UserPromptSubmit` | `Stop` (genuine completion), `StopCancelled` (interrupt, declined permission, max turns), `StopFailure` | per-turn cadence documented |
| Antigravity 1.4.2 | `PreInvocation` (Studio's file) | `Stop` | event names come from Studio's own hook file; agy documentation not read |
| Codex 0.163.0 | none: no hooks are loaded; heuristic status only (`terminal.rs:160-169`) | none | ruling: Codex keeps the heuristic |

- The session hooks are provisioned per tab and run with the PTY's environment, which includes `DESIGNOS_LIVE_DOC_PATCH`
  (`configure_pty_env_with_session`, `terminal.rs:2294`).
- A node's identity and content are hashed without building a JSON string by `HashSerializer` (CLAUDE.md invariant 1; `doc_hash`).

## 3. Behaviour rules

1. **Turn marks in the one ordered log.** A new log line `{"v":1,"turn_mark":"start"|"end","cli":"claude"}` is a third `LiveLogLine` variant,
   `TurnMark`. The session hooks append it with a one-line `printf >> "$DESIGNOS_LIVE_DOC_PATCH"` on the start and end events of the table. It
   goes into `doc-patch.jsonl` because that file already fixes the order between edits and other markers (the `DocEpoch` precedent): patches
   between a start and an end belong to that turn. Writing a short line with `>>` is atomic; MCP appends to the same file.
2. **Ledger.** `agent_turn_ledger` keeps, per agent tab, the last N turns (open question 4; the text assumes the recommended 10). A turn records: its number, CLI, start and end time, and for each
   patch applied inside it the inverse command taken from `peek_undo` right after `apply_one` commits, the ids of the layers it inserted,
   updated, removed or moved, and a fingerprint (the zero-allocation node hash) of each touched layer right after that patch. Memory per tab
   is capped (initial value 64 MB, then the oldest turn is dropped and its Revert disabled with "Too old to revert").
3. **Turn without marks (Codex, or hooks that failed to load).** The ledger closes a turn from the heuristic status: a turn starts when the tab
   goes from `Idle`/`Done` to `Working` and ends when it returns to `Done`/`Idle`; patches are attached to the open turn, or to the last closed
   one if they arrive within 2 s after it closed. The card says the grouping is approximate (open question 3).
4. **Summary.** When a turn ends and it changed the document, the card shows the CLI, the count of layers changed (inserted, updated, removed),
   and the list of those layers by name with a short change kind. Clicking a layer selects it on the canvas and reveals it
   (`App::select_only` then `fit_selection`, as `Act::Focus` does in `panels/check_designs.rs`); a removed layer is listed but not clickable. A turn
   that changed nothing shows no card.
5. **Keep** dismisses the card. The turn's steps stay in normal undo history.
6. **Revert** applies, as ONE new undo step labelled "Revert agent turn", the inverses of the turn's steps in reverse order, with these rules:
   - (open question 5) a touched layer whose current fingerprint differs from the one recorded after the agent's last edit to it was changed by the user afterwards:
     its inverse is skipped and the layer is listed in a note ("Kept 2 layers you changed afterwards");
   - an inverse whose target no longer exists (the user deleted it) is skipped the same way;
   - edits the user made on other layers during the turn are untouched because they are not in the ledger;
   - the revert is itself undoable; undoing it reapplies the turn.
   After a Revert the card shows "Reverted" and the note, then dismisses.
7. **Next turn.** When a new turn starts while the previous card is still showing, the previous card is dismissed as Keep (open question 2).
8. **Scope.** The card appears for Claude, Grok, Antigravity (marks from hooks) and Codex (heuristic marks), per the issue.
9. **Documents.** A turn that switched documents (`DocEpoch`) is split at the switch; each part belongs to its document's tab; a turn on a document
   that is no longer open cannot be reverted.
10. No change to how patches are applied, ordered or undone one by one (Ctrl+Z still undoes single agent steps).

## 4. Data model / schema changes

- `LiveLogLine::TurnMark(TurnMark { v, turn_mark: Phase, cli: Option<String> })`; `LiveLogLine::parse` probes the `turn_mark` key like it
  probes `doc_epoch` (`mod.rs:84-97`). An older Studio reading a newer log reports an unreadable line (`poll` already returns such lines as errors), so
  the format version stays `LIVE_PATCH_VERSION = 1` for patches and a mark carries `v: 1`.
- Ledger types live in the app crate (`TurnRecord`, `LedgerStep`); nothing is saved in the document or the file.

## 5. Contracts

- Hook scripts: the Claude and Grok hook files gain a `UserPromptSubmit` entry and a `Stop` entry that run the append command; agy's file gets the
  same for `PreInvocation` and `Stop`. The existing status writes stay (the status file is unchanged).
- `designos-mcp` does not write turn marks and needs no change.

## 6. File-format mapping

None in `.designos` / `.fig`. The log line is a session-folder file, not part of a document.

## 7. UI

Described in words; follows `design/principles.md` and `crates/designos-app/src/theme.rs`. The issue fixes: a per-turn summary listing the changed layers,
clicking one selects it, Keep and Revert. The form is a proposal for the owner's confirmation:

- A card at the bottom of the agent panel, `BG_SECONDARY` with a 1 px `BORDER`, radius `RADIUS_FRAME`: a headline "Claude changed 7 layers" in
  body-medium, a one-line caption with the time, a collapsible list (first 5 rows visible, then "Show all 7"), and two buttons: `Revert` (ghost) and
  `Keep` (primary). Rows: layer name and a caption "added" / "changed" / "removed"; pointer targets at least 24 px high.
- After Revert: a single line "Reverted" with "Kept 2 layers you changed afterwards" when applicable.
- The card has no animation and no timer; it exists only while it has a turn to show.
- Desktop captures at 1440: card with a short list, long list collapsed, after Revert with the note. Responsive SKIPPED (owner default). Load es-designer
  before UI code; update the Figma frames from the captures first.

## 8. Edge cases

1. The user edits a layer the agent also edited, after the turn: the agent's inverse for that layer is skipped, the user's version stays, and it is reported.
2. The user edits the same layer during the turn, before the agent's last write to it: the agent's write replaced it; revert restores the pre-agent state,
   which includes the user's earlier edit.
3. The agent inserted a frame and the user later added a child to it: reverting the insert removes the frame and its children (the child was part of the
   agent's layer); the note lists it as "removed with the frame".
4. Two agent tabs, two turns at once (RD-3): each tab has its own ledger and its own card.
5. A turn of 300 patches: 300 inverse commands kept; revert is one `Tx`; memory cap applies (rule 2).
6. A patch refused or partly skipped by replay (`LivePatchReport.skipped`): the ledger records only what was committed; skipped parts are not reverted.
7. The agent crashes without an end mark: the turn closes when the tab exits (`is_alive` false) or when the next start mark arrives.
8. Studio restarts: the ledger is in memory and is gone; the card cannot appear for older turns (limit stated in non-goals).
9. A Revert while a user edit is open: deferred like a held patch (`user_edit_in_progress`, `live_patch_apply.rs:51`) until the edit ends.
10. Large document: revert touches only the turn's layers; it does not clone the document.

## 9. Non-goals

- Reverting across restarts, or turns older than the last 10.
- Property-level merge of user and agent edits to the same layer (T10.2 would give it; the owner can ask for it).
- Reviewing edits made by a CLI outside Studio's session folder.
- A visual diff of the turn (E15-8 before/after images owns pictures).

## 10. Test plan (red first)

Seams: `live_patch` log tests for the new line, a ledger unit test with synthetic patches, and `Sim` tests with a stub agent that writes marks and
patches (the harness of `live_sync_tests.rs`).

| Case | Expected (fails before) |
|---|---|
| `turn_mark_line_parses_and_orders_with_patches` | a log of start, patch, patch, end yields the four lines in order; `LiveLogLine::parse` reads `turn_mark` |
| `old_reader_reports_the_mark_as_an_error_not_a_patch` | the unknown line is returned in `poll`'s errors |
| `hook_command_appends_one_line` | run the hook command text with `DESIGNOS_LIVE_DOC_PATCH` set to a temp file: one valid line appended |
| `ledger_groups_patches_between_marks` | two turns, three patches: grouped 2 + 1 |
| `summary_lists_changed_layers_and_selects_on_click` | card rows equal the touched layers; click selects the layer |
| `revert_restores_the_document_in_one_step` | hash of the document equals the hash before the turn; the history grew by one step; undo reapplies the turn |
| `revert_keeps_layers_the_user_changed_afterwards` | the user edits one touched layer after the turn: after Revert that layer keeps the user's edit and the others are reverted; the note names it |
| `revert_keeps_edits_on_other_layers` | the user edits an untouched layer during the turn: unchanged by Revert |
| `keep_leaves_normal_undo_history` | Keep: ten single undos still step back through the patches |
| `codex_heuristic_turns_are_marked_approximate` | status transitions only: turn recorded, card says approximate |
| `no_card_for_a_turn_without_changes` | a turn with no patches: no card |
| `ledger_is_capped` | 11 turns: the oldest is dropped and its Revert is disabled |
| `works_for_each_cli_hook_file` | the generated Claude, Grok and agy hook files contain the start and end append commands (string pins against the captured event names) |

Run: `cargo test -p designos-doc live_patch` and `cargo test -p designos-app`. Evidence: red/green capture, the three 1440 captures, and an owner-manual run with
a real Claude tab (a turn that adds frames, then Revert; before/after screenshots of the canvas).

## 11. Performance budgets

- Per patch: one `peek_undo` clone and a hash of the touched layers; no tree walk. Budget: under 1 ms per patch of typical size (a few layers), measured.
- Revert: O(layers touched by the turn); under 50 ms for a turn of 1,000 layers on the large synthetic fixture.
- Memory: 64 MB per tab (initial value), measured on a 300-patch turn of the synthetic generator.
- No timers; the card repaints on events only (idle CPU unchanged).
- No `doc.to_json()` on the UI thread, no viewport-driven revision bump (CLAUDE.md invariants 1).

## 12. Dependencies and build order

After #653 and #651. The log-line variant lands first (small, in `designos-doc`), then the ledger, then the card. `terminal.rs`
is a hot spot: all new logic is in new files, `terminal.rs` gets the hook-file additions. Coordinate with E15-4 (agent batches become one step, so a turn has fewer steps) and
RD-3 (`plugin_data["designos:run"]` authorship), which do not block this task.

## 13. Owner rulings

- After an agent CLI turn changes the document, Studio shows what that turn changed and offers Keep or Revert for the whole turn. The session hooks mark
  turn start and end; Codex uses the OSC/heuristic status. A per-turn summary lists the changed layers and clicking one selects it. Revert undoes exactly that
  turn's edits in one step, and edits the user made during the turn are kept. Keep dismisses the summary; the turn stays in normal undo history. Works for
  Claude, Grok, Antigravity and Codex tabs. (Issue text, ruled for v0.6 on 2026-10-19.)
- Codex keeps the heuristic status for now.

## Open questions for the owner

1. **Where the review shows.**
   - A (recommended): a card at the bottom of the agent panel, so it sits next to the conversation that caused it.
   - B: a bar over the canvas, closer to the changed layers.
   - C: a toast with a link to a list in the panel; lighter, but easy to miss.
2. **An unanswered card when the next turn starts.**
   - A (recommended): it is dismissed as Keep, so cards never pile up.
   - B: the cards stack until answered.
3. **Codex and other approximate turns.**
   - A (recommended): show the card with a small "approximate grouping" note, because the heuristic can split or merge turns.
   - B: show it without any note.
   - C: do not offer Revert for approximate turns (only the summary list).
4. **How many turns stay revertable.**
   - A (recommended): the last 10 per tab.
   - B: the last 3, which uses less memory and keeps the card meaningful.
   - C: unlimited while the tab lives, bounded only by memory.
5. **A layer the user changed after the turn.**
   - A (recommended): Revert skips that layer and says so, so nothing the user did is lost.
   - B: Revert restores it anyway and warns first.
