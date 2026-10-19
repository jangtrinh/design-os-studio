# #651 Agent open flow: resume the last session for this file, or start a new one

| Field | Value |
|---|---|
| Issue | #651 (owner ruling 2026-10-19) |
| Size / priority | M / P1 |
| Depends on | #653 (registry, stable folder, resume commands); #652 (the picker shown when a file has no session) |
| Unblocks | #677 and #678 (they hang off the live tab this flow opens) |
| Touches | `crates/designos-app/src/terminal.rs` (hot spot: `ensure_default_tab`, `show_left`, `render_terminal_surface`; thin edits), new `crates/designos-app/src/agent_open_flow.rs` (decision and sessions dropdown), `crates/designos-app/src/panels/rail.rs` (`:116-125`, `:134-144`, `:166-174`), `crates/designos-app/src/shell.rs` (`:268-300`, `:697-702`, `:1141-1144`, `:1349-1355`), `crates/designos-app/src/home/mod.rs` (`:690-696`), `crates/designos-app/src/tabs.rs` (`:1772`), tests |

Code baseline: `origin/main@01d48f2` (0.5.0). Owner rulings that bind this spec are in section 13; the
details the rulings do not fix are listed under "Open questions for the owner" and are not decided here.

## 1. Summary and user story

As a designer working on a file, when I click Agent, the last conversation I had with an agent about this
file opens straight away, like `claude --continue`. A dropdown lists the file's other sessions so I can
switch. A file with no sessions yet shows the new-session picker; Studio never starts a plain shell for me.

## 2. Current state (FACT)

- Opening the Agents panel makes sure one tab exists, and that tab is a plain **Shell**:
  `TerminalState::ensure_default_tab` spawns `AgentCliPreset::Shell` when `tabs` is empty
  (`terminal.rs:1004-1008`).
- It is called from many places: the rail Agents item (`panels/rail.rs:116-125`), the rail Tools item
  (`:134-144`), the `>_` terminal toggle (`:166-174`), the backtick key (`shell.rs:697-702`), the home
  "Agents" action (`home/mod.rs:690-696`), the command palette action that opens the spec reader
  (`shell.rs:1349-1355`), the `--panel agents|terminal` fixtures (`shell.rs:268-300`), every frame while the
  panel or dock is open (`shell.rs:1141-1144`, skipped only in screenshot runs), both renderers
  (`terminal.rs:2472`, `:3408`), closing a tab (`:2577`, `:3542`), `set_active_runtime` (`:1018`) and
  `submit_chat_prompt` (`:1034`).
- `dispatch_figma_ai_prompt` (Cmd+K design actions) starts the first installed of Claude, Codex, Antigravity,
  Grok when no tab exists (`terminal.rs:1368-1375`).
- Tabs are per document tab: `ProjectTab.terminal` (`tabs.rs:99`). They are killed when the tab closes
  (`TerminalState::shutdown`, `terminal.rs:991-995`).
- The panel's tab bar has a `+` runtime dropdown listing Claude, Codex, Grok, Antigravity and Shell
  (`render_runtime_dropdown_menu`, `terminal.rs:3106-3169`); each choice starts a **new** conversation.
- The chat-mode composer ("Ask for changes") runs one-shot commands such as `claude -p ...` by typing a
  command line into a **shell** tab (`submit_chat_prompt`, `terminal.rs:1029-1155`); it needs a shell PTY.
- Session folder, registry and resume commands are specified in #653; none exists on `origin/main@01d48f2`.

## 3. Behaviour rules

1. One function decides what opening the agent surface does: `TerminalState::open_for_agent_use(file)`, called
   by every entry point listed in section 2 instead of `ensure_default_tab` (which is removed from them; the
   per-frame call at `shell.rs:1141-1144` is deleted, a frame never starts a process).
2. Decision, in order:
   1. a live agent tab exists: show the panel, do nothing else;
   2. no live tab and the file's registry (#653) has sessions: resume the one with the largest `last_active_ms`
      (any CLI) in a new tab, with that CLI's resume command (#653 rule 6);
   3. no live tab and no sessions: start nothing; the panel's center shows the new-session picker (#652).
3. Never spawn a Shell on open. A Shell tab starts only when the user picks Shell in the `+` menu or the picker
   (as today) or when a flow that needs one asks for it explicitly: the chat-mode composer starts its one-shot
   shell on the first submit, not on open.
4. A tab opened by rule 2 behaves like any other tab: it can be closed, it shows the live status dot, and the
   registry entry's `last_active_ms` updates.
5. **Sessions dropdown.** The issue fixes a dropdown that lists the file's other sessions with CLI, title, last
   active and status. Its form and placement are proposed in section 7 (open question 4). It lists the file's registry
   entries, newest first, one row each: the CLI (icon and name), the label (rename target), last active as
   a relative time, and a status. Status is the live tab's `AgentTabStatus` for a running tab
   (`Working`, `WaitingForInput`, `Done`, `Idle`, `terminal.rs:357-364`) and `Not running` for the rest;
   `Missing` when #653 marked the entry's session as gone. Choosing a row focuses its live tab, or resumes it
   in a new tab.
6. **Rename.** The issue requires that sessions can be renamed; the interaction is proposed in section 7. The new label is stored in the registry (#653) and shown on the tab title; it does not rename the
   conversation inside the CLI. Empty labels revert to the default (`<CLI> <date>`).
7. A resume that fails within the five-second window shows the existing start-failure panel
   (`agent_start_failure`) with the added fix "Start a new session" (#653 rule 8), which opens the picker.
8. Skip-permissions stays the default for resumed sessions; ask mode is the optional setting of #656.
9. `Done`/`Idle` tabs are not closed by opening the panel again. Opening the panel for a second file never
   touches the first file's tabs (each document tab owns its `TerminalState`).

## 4. Data model / schema changes

None beyond #653's registry. `TerminalState` gains `open_for_agent_use` and a `picker_open: bool` UI flag
(`TerminalState` fields are listed at `terminal.rs:625-682`).

## 5. Contracts

No MCP change. Internal contract: `ensure_default_tab` keeps existing for callers that truly need a shell
(only `submit_chat_prompt`'s lazy shell start), renamed `ensure_shell_for_chat` to make the exception visible.

## 6. File-format mapping

None.

## 7. UI

Described in words. Checked against `design/principles.md` (P1 to P10) and `design/soul.md`; the rules used, with the `theme.rs` constants, are recapped in README-part3 under "UI rules checked for part 3". In short: every colour, gap, inset, control height and radius below is a `crates/designos-app/src/theme.rs` constant (the panel's own `TERM_*` / `STATUS_*` constants at `terminal.rs:29-55` stay for the terminal viewport and the existing tab status dots and are not extended); selected and active rows use the neutral fill `BG_SELECTED`; the brand orange is the view's one primary action; counts sit in their own pill, not in a label.

**Proposal only:** the issue fixes the behaviour of section 3, not these control details; the owner confirms them through open question 4 before anyone builds.

- **Sessions control**: a ghost button labelled "Sessions" with a chevron, `CONTROL_DENSE` high, `RADIUS_CONTROL`, between the tab strip and `+`, `SPACE_INLINE` from each.
- **Popover**: `RADIUS_FRAME`, surface `BG_MENU` with a `BORDER_MENU` hairline, inset `INSET_POPOVER`, a caption heading "Sessions for this file" in `TEXT_SECONDARY` with `SPACE_FACT` below it and a `BORDER` hairline; rows are `CONTROL_ROW` high with two lines: line one the CLI name (body-medium) and the label (body), line two the last-active time and the status in caption; hover `BG_MENU_HOVER`; the row whose tab is focused uses the neutral `BG_SELECTED` fill. The status shown in a row uses the same dot as the tab (`paint_orca_tab_status_and_provider`, `terminal.rs:1983-2058`); no new colour is introduced.
- **Rename**: the label becomes a text field at `FIELD_HEIGHT` inside the row (Enter saves, Esc cancels); no modal.
- **Empty state**: the picker (#652) fills the viewport area under the tab bar, which keeps `+` and Sessions.
- **Failure**: the existing card (`agent_start_failure::paint`) with one more fix button text, "Start a new session".
- Desktop capture at 1440 for the three states: resumed tab, Sessions popover open, empty-state picker; responsive SKIPPED (owner default). Load es-designer before UI code. The Figma frames for these states are updated from the captures before gates run (owner rule, 2026-10-13).

## 8. Edge cases

1. Registry holds sessions but the CLI is no longer installed: rule 2.2 starts the tab, it exits within five
   seconds, the failure panel says "not installed" with its install link (#673 classification).
2. Two tabs for the same session: rule 5 focuses the live one.
3. The only session is `Missing`: treated as no session (rule 2.3), the picker shows.
4. Opening the panel while a resume is still starting: no second spawn (a tab in `spawned_at < 5 s` counts as live).
5. A file with no key (#653 question 1, option C): no registry, so rule 2.3 always applies.
6. Reopening after all tabs were closed by the user: rule 2.2 resumes the last session again (this is the
   ruled behaviour; closing a tab does not forget the session).
7. Window closed with agent tabs running: tabs are killed (existing), sessions stay resumable.
8. `--panel agents` screenshot fixture: with an empty registry it captures the picker (new fixture name
   `agents-picker`), no longer a Shell.

## 9. Non-goals

- Listing sessions started outside Studio.
- Renaming the conversation inside the CLI.
- A session search or filter.
- Changing the composer's chat-mode behaviour beyond the lazy shell.

## 10. Test plan (red first)

Seams: `shell` simulator tests (`Sim`, `shell/tests/*.rs`) with a temp library root and stub CLIs (shell scripts
that print their argv and stay alive, `spawn_exiting_stub_agent` pattern, `terminal.rs:544-548`).

| Case | Expected (fails before) |
|---|---|
| `open_with_no_sessions_shows_the_picker_and_spawns_nothing` | after the rail Agents click: `tabs.is_empty()`, picker flag set, no process started |
| `open_with_a_session_resumes_the_most_recent` | registry with two entries: the stub records the newest entry's resume argv |
| `open_never_spawns_a_shell` | across every entry point (rail Agents, rail Tools, `>_`, backtick, home action, palette action): no tab with `is_shell_process()` |
| `per_frame_call_removed` | N frames with the panel open and no tabs: still no tabs |
| `live_tab_is_not_duplicated` | open twice: one tab |
| `sessions_dropdown_lists_cli_label_time_status` | rows match registry; status of the live tab follows `agent_status` |
| `rename_persists_across_restart` | rename, rebuild the state from disk, label kept |
| `closing_the_last_tab_shows_the_picker` | (per the owner's answer to question 1) |
| `chat_submit_starts_its_shell_lazily` | composer submit with no tab: one shell tab starts then |
| `resume_failure_offers_a_new_session` | stub exits at once: failure card with the new fix; click opens the picker |

Run: `cargo test -p designos-app shell` and `cargo test -p designos-app terminal`. Evidence: red/green capture and
the three 1440 captures.

## 11. Performance budgets

- Opening the panel with a resume: the process starts within 200 ms of the click on the dev machine (same
  path as a new tab today); the registry read (<5 ms) happens before the spawn, not per frame.
- Idle: no timer, no repaint request beyond the existing 50 ms / 500 ms panel cadence (`terminal.rs:2625-2626`);
  the sessions control repaints only when its menu is open.

## 12. Dependencies and build order

After #653 (registry) and alongside #652 (picker). `terminal.rs` is the hot spot of this pack: keep the edits to
call-site changes and put the logic in `agent_open_flow.rs`; serialize with #652, #654, #656, #677, #678, #679
(see README-part3).

## 13. Owner rulings

- Clicking Agent on a file that has a past session resumes the last one directly, like `--continue`. A dropdown
  lists the other sessions (CLI, title, last active, status). A file with no sessions opens the new-session
  picker and never auto-spawns a Shell. Sessions can be renamed. (Issue text, 2026-10-19.)
- Skip-permissions stays the default; ask mode is optional (#656).
- Codex tabs keep the heuristic status for now.

## Open questions for the owner

1. **Closing the last tab.** After the user closes the only tab, what does the panel show?
   - A (recommended): the new-session picker, the same as a file with no sessions. The panel is never a blank area
     and the next click already does something useful.
   - B: an empty panel with only the tab bar (`+` and Sessions). Quieter, but the user must open a menu to start.
2. **Cmd+K design actions with no live tab.** `dispatch_figma_ai_prompt` starts the first installed CLI today
   (`terminal.rs:1368-1375`). With resume in place, what should it do?
   - A (recommended): resume the file's last session and send the prompt there, so the conversation keeps its context.
   - B: keep today's behaviour: start a new session with the first installed CLI.
   - C: open the picker and wait for a choice before sending.
3. **The `>_` dock and the backtick key.** They open the same agent terminal in a bottom dock.
   - A (recommended): the same rules as the panel (resume, or the picker), so there is one meaning of "open the agent".
   - B: the dock stays a plain shell, the way a terminal usually behaves.
4. **Form of the Sessions dropdown (section 7 is the proposal).**
   - A (recommended): a ghost "Sessions" button beside `+` that opens a popover list; a row renames in place and has a
     "Remove from list" action (forgets Studio's entry, leaves the CLI's conversation). It keeps tabs and sessions in one
     bar and lets a stale row go.
   - B: the same popover with rename only, no remove.
   - C: sessions as a section at the top of the existing `+` menu, so there is one menu instead of two.
