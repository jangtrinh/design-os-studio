# #677 Agent tabs: review each agent turn's canvas edits and keep or revert them as one step

| Field | Value |
|---|---|
| Issue | #677 (owner request 2026-10-18, ruled for v0.6 on 2026-10-19; audit proposal 11) |
| Size / priority | L / P1 |
| Depends on | #653 (session folder), #651 (a live tab to attach to); E15-4 (batch writes become one agent step, optional); T10.2 three-way diff (optional, only if the owner wants property-level revert) |
| Unblocks | none |
| Touches | new `crates/designos-app/src/agent_turn_marks.rs` (reader of `turn-marks.jsonl`), new `crates/designos-app/src/agent_turn_ledger.rs` (turns, fingerprints, revert plan), new `crates/designos-app/src/agent_turn_card.rs` (UI), `crates/designos-app/src/live_patch_apply.rs` (`apply_one`, `:112-132`), `crates/designos-app/src/live_patch_log.rs` (`poll`, `:92-120`: expose each line's start offset), `crates/designos-app/src/terminal.rs` (hot spot: hook files `:2194-2233`, status polling `:1328-1338`), `crates/designos-doc/src/history.rs` (read the last undo step, `peek_undo` exists), tests |

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

- The session hooks are provisioned per tab (`provision_session_agent_files`, `terminal.rs:906-915`) and their commands embed absolute paths, as `status_hook_command` does with
  `status_path` (`terminal.rs:2194-2196`). The PTY environment also carries `DESIGNOS_LIVE_DOC_PATCH` (`configure_pty_env_with_session`, `terminal.rs:2294`), but whether a CLI passes its environment
  to hook processes was not verified, so the hook command uses the literal log path.
- **Who writes and reads `doc-patch.jsonl` (checked for this spec).** `designos-mcp` appends one line per call with `OpenOptions::new().create(true).append(true)` and a single `write_all`
  (`append_log_line`, `designos-mcp/src/server.rs:109-115`); it never truncates, rewrites or renames this file (the `truncate` and `remove_file` calls at `server.rs:451-503` belong to save and
  export paths). Studio's reader treats the log as rewritten only when the file shrank, its identity changed, or its first 256 bytes differ (`live_patch_log.rs:15`, `:98-99`, `:221-223`); an
  appended line changes none of these. The only production parser is `LiveLogLine::parse` in Studio's reader; the other readers are MCP's own tests (`designos-mcp/tests/live_doc_epoch.rs:40`).
- A node's identity and content are hashed without building a JSON string by `HashSerializer` (CLAUDE.md invariant 1; `doc_hash`).

## 3. Behaviour rules

1. **Turn marks in their own file, ordered by byte offset.** The session hooks append one line to a **separate** file, `<session folder>/turn-marks.jsonl`, on the start and end events of the table:
   `{"v":1,"turn_mark":"start"|"end","cli":"claude","patch_bytes":N}`, where `N` is the size in bytes of `doc-patch.jsonl` at that moment (`wc -c < '<log path>'` inside the hook command, literal absolute paths). Hooks are the
   only writers of `turn-marks.jsonl`, and `designos-mcp` stays the only writer of `doc-patch.jsonl`, so no file has two writers and a large patch line can never be interleaved with a mark (the earlier idea of
   appending marks to `doc-patch.jsonl` was dropped for that reason: two processes appending to one file are not guaranteed to keep a long line whole). Studio reads both files; the log reader reports each parsed
   line's start offset (the reader already tracks `offset`, `live_patch_log.rs:92-120`), and a patch belongs to the turn whose `start.patch_bytes <= patch.start_offset < end.patch_bytes` (an open turn has no end yet).
   A patch that lands between the hook's `wc -c` and its `printf` is assigned to the side where it started, which is the correct side for a start mark (the turn began before it) and, for an end mark, a patch that
   finished after the CLI's `Stop` cannot belong to the turn. `designos-mcp` and `designos-doc` need no change, and no `LiveLogLine` variant is added, so older readers are unaffected.
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

- `TurnMarkLine { v, turn_mark: Phase, cli: Option<String>, patch_bytes: u64 }` read from `turn-marks.jsonl` by the new reader (app crate); `doc-patch.jsonl` and `LIVE_PATCH_VERSION` are untouched.
- Ledger types live in the app crate (`TurnRecord`, `LedgerStep`); nothing is saved in the document or the file.

## 5. Contracts

- Hook scripts: the Claude and Grok hook files gain a `UserPromptSubmit` entry and a `Stop` entry that run the mark command (`wc -c` of the log, then `printf >>` to `turn-marks.jsonl`); agy's file gets the
  same for `PreInvocation` and `Stop`. The existing status writes stay (the status file is unchanged).
- `designos-mcp` is unchanged and never sees `turn-marks.jsonl`.

## 6. File-format mapping

None in `.designos` / `.fig`. The log line is a session-folder file, not part of a document.

## 7. UI

Described in words. Checked against `design/principles.md` (P1 to P10) and `design/soul.md`; the rules used, with the `theme.rs` constants, are recapped in README-part3 under "UI rules checked for part 3". In short: every colour, gap, inset, control height and radius below is a `crates/designos-app/src/theme.rs` constant (the panel's own `TERM_*` / `STATUS_*` constants at `terminal.rs:29-55` stay for the terminal viewport and the existing tab status dots and are not extended); selected and active rows use the neutral fill `BG_SELECTED`; the brand orange is the view's one primary action; counts sit in their own pill, not in a label.

The issue fixes: a per-turn summary listing the changed layers, clicking one selects it, Keep and Revert. The form is a proposal for the owner's confirmation:

- A card at the bottom of the agent panel: `RADIUS_FRAME`, `BG_SECONDARY`, `BORDER` hairline, inset `INSET_FRAME`. Headline "Claude changed layers" in body-medium followed by a count pill with the number (the existing count pill of `panels/check_designs.rs:186-196`, `BG_TERTIARY` fill, number in its own pill, P5); a caption line with the time; a collapsible list (the first 5 rows visible, then "Show all") with rows `CONTROL_DENSE` high (layer name in body, change kind "added" / "changed" / "removed" in caption `TEXT_SECONDARY`; each row is its own pointer target); two secondary buttons, `Revert` and `Keep`, `CONTROL_STANDARD` high, `RADIUS_CONTROL`, `SPACE_INLINE` apart. Neither is the primary action: the view's one primary action stays with the approval card (#656) or the composer's send.
- After Revert: one caption line "Reverted" and, when applicable, "Kept 2 layers you changed afterwards" (the number in `font_mono`).
- No animation, no timer; the card exists only while it has a turn to show.
- Desktop captures at 1440: card with a short list, long list collapsed, after Revert with the note. Responsive SKIPPED (owner default). Load es-designer before UI code; update the Figma frames from the captures first.

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
| `marks_assign_patches_by_byte_offset` | a log of two patches, a start mark with `patch_bytes` = 0 and an end mark with `patch_bytes` = the size after the second patch: both patches belong to the turn; a third patch after the end belongs to none |
| `a_large_patch_and_a_mark_never_interleave` | a stub writer appends a 1 MiB patch line to `doc-patch.jsonl` while the hook command runs repeatedly against `turn-marks.jsonl`: every patch line parses and every mark line parses |
| `hook_command_writes_the_current_log_size` | run the hook command text against a temp log of known size: one valid mark line with that `patch_bytes` |
| `log_reader_reports_line_start_offsets` | offsets of three lines equal their positions |
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
