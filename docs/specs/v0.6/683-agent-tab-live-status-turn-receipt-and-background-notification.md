# #683 Agent tabs: live turn status, a "worked for" receipt per turn, and a notification when an agent finishes in the background

| Field | Value |
|---|---|
| Issue | #683 (parent #585; owner ruling 2026-10-19: notify only when Studio is unfocused or the tab is hidden) |
| Size | L |
| Depends on | #677 (turn grouping and the turn ledger); soft: #653 (CLI session ids in the registry), #651 (sessions dropdown rows) |
| Unblocks | #687 (announces the state changes this task emits), #684 (the stopped state), #686 (the error state) |
| Touches | new `crates/designos-app/src/agent_turn_status.rs` (event reader, per-tab status, receipt), new `crates/designos-app/src/agent_notify.rs` (OS notification), `crates/designos-app/src/terminal.rs` (hot spot: status type `:358-364`, hook files `:2194-2233`, status polling `:1328-1338`, dot painter `:1980-2040`), `crates/designos-app/src/theme.rs` (state colours), tests |

Code baseline: `origin/main@9f3f7a5`. Hook facts: Grok from its bundled user guide (`~/.grok/docs/user-guide/10-hooks.md`, read 2026-10-19 on this machine); Claude Code hook payloads are UNKNOWN on this machine until captured (see "Could not ground").

## 1. Summary and user story

As a designer with several agent tabs, I want each tab to show what its agent is doing, and I want one clear line when a turn is over: how long it took, how many tool calls, how many layers changed. If I am elsewhere when the agent finishes or needs me, I want one notification.

## 2. Current state (FACT)

- `AgentTabStatus` is `Idle`, `Working(Option<String>)`, `WaitingForInput(Option<String>)`, `Done` (`terminal.rs:358-364`). There is no error, no stopped, no "done and unread".
- The tab dot paints those states: a stepped 12 fps arc in `#EAB308` for working (it asks for a repaint every 83 ms), an orange ring for waiting, a `#10B981` dot for done and idle; the colours are literals (`terminal.rs:1980-2040`).
- Hooks write one fixed word: `status_hook_command` runs `printf '{"state":"working"}' > <agent-status.json>` (`:2194-2196`); `session_hooks_settings` registers it for `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `PermissionRequest` and `Stop` (`:2207-2221`); Antigravity gets `PreInvocation` and `Stop` (`:2225-2233`). The hook command never reads the payload on stdin.
- Studio reads that file and applies it to the active tab only, whichever tab wrote it (`:1328-1338`). The file is one per session folder, which is one per file, so two agent tabs of one file share it. Two events inside one poll leave only the last.
- Codex loads no hooks (comment at `:160-169`); its status comes from OSC sequences and PTY activity (`parse_osc_agent_status_and_strip`, `:1611`).
- Grok sends each event as one line of JSON on stdin with `toolName`, `sessionId`, `timestamp`, `promptId` (the turn) and, for failures, `error` classes `rate_limit`, `authentication_failed`, `invalid_request`, `server_error`, `max_output_tokens`, `unknown` (guide lines 91, 180, 327-345, 480). `StopCancelled` carries a reason such as `user_interrupt` (line 103, 180).
- #677's spec adds turn marks to `doc-patch.jsonl` and a per-tab turn ledger. #683 builds on its turn boundaries.
- Studio has no OS notification code: no notification crate is a direct dependency of `designos-app` (`crates/designos-app/Cargo.toml`).
- egui reports window focus as `ViewportInfo.focused: Option<bool>` (`egui-0.35.0/src/data/input/viewport_info.rs:80`).
- `theme.rs` has no `info`, `success` or `warning` constant; `design/design.tokens.json` has all three (`info` `#0d99ff`, `success` `#14ae5c`, `warning` at line 295).

## 3. Behaviour rules

1. **Event stream.** Each hook appends the CLI's payload to the file `agent-events.jsonl` in the session folder as one wrapped record: `{"t":<unix seconds>,"e":"<hook event>","p":<payload>}`. The command is a shell pipeline (`printf` the prefix, `cat` the payload, `printf` the suffix). Studio parses with a streaming JSON reader, so a multi-line payload is fine. The hook never parses JSON. The existing `agent-status.json` write stays until the stream is proven, then is removed.
2. **Attribution.** A record belongs to the tab whose CLI session id equals the payload's session id (Studio chooses the id for Claude and Grok, #653 rule 5). A record whose id matches no tab goes to the only live tab of that CLI, else to none. It is never given to the active tab by default.
3. **Status per tab.** `AgentTabStatus` gains `Error(Option<String>)` and `Stopped`. A tab also holds `unread: bool`, set when a turn ends while the tab is not visible and cleared when the user shows it.
   - running: from `UserPromptSubmit`, tool events;
   - needs input: from `PermissionRequest` (and, for Grok, a `Notification` of type `permission_prompt`);
   - done: from `Stop` (Grok: `reason == "end_turn"`);
   - error: from `StopFailure` (class kept for #686 and #687);
   - stopped: from `StopCancelled` with reason `user_interrupt`, or from #684's own stop.
   Codex keeps the OSC and PTY heuristic and produces running, needs input and done only.
4. **Status text** (panel header, only while running): a phrase from the current tool name. Names of Studio's own tools map to phrases ("Editing layers…", "Reading the file…", "Checking the design…"); any other tool shows "Using <tool name>…". With no tool step it shows "Working…", and after 30 s "Still working…". The text never contains a layer count unless the payload carries one (UNKNOWN for every CLI; do not estimate).
5. **Turn receipt** after each turn: `Worked for 1m 20s · 12 tool calls · 7 layers changed`.
   - Duration is the difference of the start and end record times, rounded up to whole seconds, minimum 1 s; `Ns` under a minute, else `Nm Ns`.
   - Tool calls are the count of `PreToolUse` records of that turn. Layers changed is the number of distinct layers #677's ledger recorded for the turn. Neither is estimated; a count Studio cannot know is omitted from the line, not guessed.
   - Clicking the receipt opens #677's per-turn card. A turn that was stopped reads `Stopped by you after 42s` (#684); a failed turn reads `Failed after 42s · <class>`.
6. **Notification.** When a turn ends or the tab needs input, and the window is unfocused or the tab is not visible, send one OS notification: "<agent name> finished in <file name>" or "<agent name> needs your input". One per finished turn. None when the window is focused and the tab is visible. Clicking it focuses the window and selects the tab (UNKNOWN, rule 8).
7. **Visible** means: the document tab is active, the Agents panel (or dock) is open, and this tab is the active agent tab.
8. **Spike S1 (before coding the click).** Show whether the chosen notification mechanism can report a click on macOS, Windows and Linux. If it cannot, ship the notification without the click and record which platform lacks it (open question 2).
9. Status changes are emitted as a drained list `Vec<StatusChange { tab, from, to, class }>` that #687 reads. This task adds no accessibility code.

## 4. Data model / schema changes

- `AgentTabStatus::{Error(Option<String>), Stopped}`, `TerminalTab.unread`, `TurnReceipt { started_s, ended_s, tool_calls, layers_changed, outcome }` in memory.
- `agent-events.jsonl` is a session-folder file, not part of the document.
- State colours as tokens in `theme.rs` and `design/design.tokens.json` (P1): `INFO`, `SUCCESS`, `WARNING`.

## 5. Contracts

- Hook files of Claude, Grok and Antigravity register the append command for the events of rule 3 (Claude and Grok: `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `PermissionRequest`, `Stop`, `StopFailure`, `StopCancelled` where the CLI has it; Antigravity: `PreInvocation`, `Stop`). Pin the event names against the captured help and docs, as the existing pin tests do.
- Coordinate with #677: its `turn_mark` lines and this stream both mark turns. The turn boundary used by the receipt is #677's; this stream supplies tool counts, step text and attribution.

## 6. File-format mapping

None.

## 7. UI

Described in words; checked against `design/principles.md` P1 to P10 and `design/soul.md`.

- **Status dot** on each tab and in the sessions dropdown rows (#651), `radius_full`, 8 px, inside a 24 px pointer target. running: the `info` colour, animated (the existing stepped arc; frames only while running); needs input: `WARNING`; done and unread: `SUCCESS`, filled; done and read, or idle: a hollow ring in `TEXT_SECONDARY`; error: `DANGER`; stopped: `TEXT_SECONDARY`, filled. Purple stays banned; orange stays the one primary action.
- **Status text** sits in the panel header strip right of the file name, caption type, `TEXT_SECONDARY`, elided with `widgets::elide`; the running dot sits at its left.
- **Receipt** is one caption line under the last turn: numbers in `font_mono`, the rest in caption type. It is a button when #677's card exists (24 px tall target). It has no animation.
- No fake state: a dot shows only what the event stream or the heuristic reported.
- Desktop capture at 1440: five dot states side by side in the tab strip, the header text while running, a receipt line, a stopped receipt. Responsive: SKIPPED (owner default). Load es-designer before UI code; update the Figma frames from the captures first.

## 8. Edge cases

1. Two Claude tabs in one file: records route by session id; each tab has its own dot.
2. A payload without a session id (Antigravity): single live tab of that CLI gets it, else none.
3. A hook that fails or a CLI that never fires `Stop`: the tab leaves running when its process exits; the receipt says "Ended without a stop event".
4. A `Stop` that fires twice for one turn (a blocking Stop hook): the second is ignored while no `UserPromptSubmit` came between.
5. A turn with no tool calls: "Worked for 3s · 0 tool calls".
6. The user switches documents: notifications still use the file name of the tab's own document.
7. The window regains focus after a finished turn: the unread dot stays until the tab is shown.
8. Notifications denied by the OS: the dot and receipt still work; the status line says once that notifications are off.
9. A turn longer than an hour: `61m 5s`, not hours.

## 9. Non-goals

- Token and cost usage (#678). Estimating counts. Notification sounds. Per-tab notification settings.

## 10. Test plan (red first)

Seams: pure parsing and receipt tests; a stub CLI that writes records with `printf`; the `Sim` harness for dots and header text.

| Case | Expected | Fails before |
|---|---|---|
| `hook_command_appends_one_wrapped_record` | run the command text with a payload on stdin: one valid JSON record with `t`, `e`, `p` | yes |
| `stub_cli_drives_running_needs_input_done` | tab status sequence | yes |
| `records_route_by_session_id_not_by_active_tab` | the background tab changes, the active one does not | yes (`:1328-1338`) |
| `receipt_shows_exact_duration_and_counts` | `Worked for 1m 20s · 12 tool calls · 7 layers changed` from fixed records | yes |
| `duration_rounds_up_with_minimum_one_second` | 0.2 s gives `1s`; 59.1 s gives `1m 0s`; 60.0 s gives `1m 0s`; 61 s gives `1m 1s` | yes |
| `no_count_is_invented_without_data` | no layers record: the part is absent | yes |
| `one_notification_per_turn_when_unfocused` | stub sink gets 1 call; 0 when focused and visible | yes |
| `notification_when_the_tab_is_hidden_but_window_focused` | 1 call | yes |
| `works_for_each_cli_hook_file` | the generated hook files contain the append command for each pinned event | yes |
| `codex_uses_the_heuristic` | no events file read; running and done still shown | guard |
| `theme_has_info_success_warning_in_token_parity` | `design_os_self_evolve.py` parity passes | yes |

Run: `cargo test -p designos-app terminal agent_turn_status`. Evidence: red/green capture; the 1440 captures; an owner-manual run per CLI (Claude, Codex, Grok, Antigravity) with a real turn.

## 11. Performance budgets

- Event reading: tail the file off the UI thread; the UI thread drains a channel. Under 0.1 ms per frame for a 1,000-record turn (parse is on the worker).
- No `to_json` on the UI thread. No timer except the existing running spinner and one one-shot at 30 s for "Still working…".
- Idle (no running tab): 0.00% CPU.

## 12. Dependencies and build order

After #677 (log marks and ledger), which follows #651 and #653. `terminal.rs` is a hot spot: the new modules hold the logic; `terminal.rs` gets the status enum, the hook-file additions and the dot call site. Merge order for the whole panel family is in README-part4.

## 13. Owner rulings

- Notification only when Studio is unfocused or the tab is hidden (2026-10-19).
- Codex tabs keep the OSC and heuristic status. Counts come only from data. (Issue text.)
- Skip-permissions stays the default; `GROK_FOLDER_TRUST=0` stays.

## Could not ground

- Claude Code's hook payload fields (tool name, session id, timestamp): not read on this machine. Check: capture one `PreToolUse` and one `Stop` payload from a real tab into the session folder and pin the field names.
- Antigravity hook payloads and whether its hooks receive a session id. Check: same capture.
- Whether a CLI passes its environment to hook processes: not needed here, because attribution uses the payload's session id.

## Open questions for the owner (recommended first)

1. "Layers changed" comes from Studio's own record of applied edits, not from a hook. Recommended: accept, since it is measured, and label it in the receipt's tooltip.
2. Notification library. Recommended: one cross-platform crate on a worker thread, with click-to-focus only where the spike shows it works.
3. Dot colour for running. Recommended: the `info` blue, as the brief says; note that the soul also names orange for "live state", so the owner may prefer orange here.
