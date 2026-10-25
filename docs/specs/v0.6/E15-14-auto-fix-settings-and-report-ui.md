# E15-14 Configure auto-fix and review its diff report in Studio

| Field | Value |
|---|---|
| Epic / pack | E15 / v0.6 AI design |
| Size / priority | M / P0 |
| Depends on | E15-13 |
| Unblocks | part-2 eval dashboards (optional) |
| Touches | `crates/designos-app/src/panels/check_designs.rs`, `crates/designos-app/src/check_designs/state.rs`, `crates/designos-app/src/comments/settings.rs` (shared settings file), `crates/designos-app/src/terminal.rs` (agent panel line, `context.json`), `crates/designos-doc/src/history.rs` (`SetAutoFix` command), `crates/designos-doc/src/live_patch/mod.rs` (report payload) |

## 1. Summary and user story

As a user, I see what the engine changed on my behalf after each agent write, jump to each changed
layer, revert a single fix, and choose whether auto-fix is on, report-only or off, for this file and
as my default, with per-rule switches. My choice survives restarts.

## 2. Current state (FACT)

- Check designs is a floating panel 320 px wide with fixed 60 px rows, Fix and Fix all buttons, and
  row click to focus a layer (`crates/designos-app/src/panels/check_designs.rs:13-36`, actions
  `Act::{Close, FixAll, Fix, Focus}`).
- The only persisted app setting is the author name in `settings.json` at the library root; unknown
  keys are preserved for other settings (`crates/designos-app/src/comments/settings.rs:1-8`).
- The agent runtime writes `context.json` for the MCP session on every relevant change
  (`crates/designos-app/src/terminal.rs:769-802`); the MCP reads it (`crates/designos-mcp/src/server.rs:222-229`).
- E15-13 publishes auto-fixes as a separate `LivePatch` line labelled `Auto-fix` and defines
  `AutoFixSettings` at session, file (`Document.auto_fix`) and user levels.
- `History` has no command for document-level settings (`crates/designos-doc/src/history.rs:8-74`).

## 3. Behaviour rules

### 3.1 Report delivery

- `LivePatch` gains `auto_fix_report: Option<Vec<FixRow>>` (`FixRow { node, node_name, property,
  before, after, rule }`), set only on the `Auto-fix` line. Studio keeps the latest report per open
  file in memory (`CheckDesignsState.last_auto_fix`) plus the 20 previous ones for the session.
- Report-only runs travel as a `LivePatch` with empty `ops`, `label: "Auto-fix (report only)"` and the
  `would_apply` rows in `auto_fix_report`. The ordinary publish skips empty patches
  (`server.rs:254`), so this line is written by a dedicated path; the editor's replay applies no ops,
  creates no undo step and only records the report.

### 3.2 Settings

- User default: `settings.json` key `auto_fix` = `{"mode": "on"|"report_only"|"off", "rules": {...}}`,
  read and written with the same preserve-unknown-keys helper as the author setting.
- This file: `Document.auto_fix`, changed through a new undoable `Command::SetAutoFix { settings:
  Option<AutoFixSettings> }` (inverse restores the previous value), so it saves with the file.
- Effective settings reach the MCP: Studio adds `auto_fix_user` to `context.json`; the MCP reads the
  file level from the document and the user level from `context.json`, with E15-13's precedence
  (session > file > user).

### 3.3 Revert one fix

A row's Revert action creates one undo step `Revert auto-fix: <rule> on <layer>` that sets the property
back to `before` (or unbinds and restores the raw value). Enabled only while the current value still
equals `after`; otherwise the button is disabled with the tooltip `Changed since the fix`.

### 3.4 Navigation

Clicking a report row selects the node and zooms to it (the existing Focus action). Missing node
(deleted since): row shows `Layer deleted`, not clickable.

## 4. Data model / schema changes

- `Command::SetAutoFix` in designos-doc `history.rs`.
- `LivePatch.auto_fix_report` (optional, skipped when `None`).
- `CheckDesignsState { last_auto_fix: Option<AutoFixReport>, history: VecDeque<AutoFixReport> }`.
- `settings.json` key `auto_fix` (new; old files without it mean the defaults).

## 5. Contracts

No new MCP tool. Settings precedence and the meaning of each mode are E15-13's. `context.json`
schema gains `"auto_fix_user": {"mode": "...", "rules": {}}`.

## 6. File-format mapping

`Document.auto_fix` export to `.fig` plugin data is defined in E15-13.

## 7. UI (follows `design/principles.md`, `theme.rs`; load es-designer before UI code)

Inside the Check designs panel (320 px), below the summary row, a section header `Auto-fix` (13 px
section heading) with two parts:

1. **Last auto-fix** list: one 28 px row per fix (`D12` row height): layer name (11 px body), rule and
   `before → after` in `theme::font_mono()` (P5 technical data), and a 24 x 24 Revert icon button
   (P3 pointer floor). A count badge pill next to the header (`Badge/StatusCount`, D85), never
   "Auto-fix (3)". Empty state: `No automatic fixes yet` (real empty state, P6).
2. **Settings** disclosure: a 28 px segmented control `On | Report only | Off` for `This file`, a second
   for `My default`, and a list of exactly the safe-tier rules (E15-13 3.1: `sizing-conflict`, `hardcoded-value`) with 24 px toggles; suggest-tier rules have no toggle and cannot be made automatic, grouped under
   the same two scopes. Colours: Brand Orange for the active segment and focus ring, Zinc surfaces and
   1 px `#27272A` borders (P2, P4). No purple.

Agents panel: after an agent write with fixes, the chat stream shows one system line `Auto-fixed 3
issues · Review` where `Review` opens Check designs scrolled to the report. Report-only shows
`3 fixes available · Review`.

Every control is wired to real state (P6 Zero Fake UI; `scripts/zero_fake_ui_gate.py` must pass).
Desktop capture at 1440 in the PR; responsive SKIPPED (owner default).

## 8. Edge cases

1. Two agents (RD-3) produce fixes: rows group by agent run label.
2. The user switches files: the report shown is the active file's.
3. Revert after the agent edited the node again: disabled (drift rule).
4. `settings.json` unreadable or malformed: defaults apply; the panel shows `Using defaults: settings
   file could not be read` and does not overwrite the file until the user changes a setting.
5. File setting on, user default off: the file setting wins for this file; the UI shows both values.
6. Undoing a Revert re-applies the fix (it is an ordinary undo step).

## 9. Non-goals

- A global preferences window.
- Editing session-level options from Studio (agents set those).

## 10. Test plan

Seams: shell tests (`crates/designos-app/src/shell/tests.rs` harness) and a screenshot.

| Case | Steps | Expected |
|---|---|---|
| `report_only_stops_changes_and_lists` | set file mode Report only; replay a stub agent patch with report-only rows | document unchanged; rows listed under Last auto-fix with "available" wording |
| `row_click_selects_and_zooms` | click row for node 91 | selection == [91]; viewport centred on its bounds |
| `settings_persist_across_restart` | set My default Off; reload app state from the same library root | `settings.json` has `"auto_fix":{"mode":"off"}`; segmented control shows Off |
| `revert_one_fix_is_one_undo_step` | Revert row 1 | property equals `before`; one undo restores `after` |
| `revert_disabled_after_drift` | change the property, then view row | button disabled with tooltip |
| `zero_fake_ui_gate` | `python3 scripts/zero_fake_ui_gate.py` | passes |

## 11. Performance budgets

Report rows ≤ 200 per report, virtualised like existing rows. No per-frame file IO; `settings.json` is
written only on change. Idle CPU stays 0% (no timers).

## 12. Dependencies and build order

After E15-13. Last task of the lint track.

## 13. Owner rulings

- Auto-fix (2026-10-18): configurable to off or report-only; this task is the user-facing control.
- Zero fake UI (design principle P6).
- Responsive checks only on request; desktop capture by default.
