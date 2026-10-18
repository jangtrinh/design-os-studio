# E15-38 Add the variant explorer UI: count, mode, model per agent, progress and ranking

- Issue: #627 (task E15-38, epic #585, M / P0)
- Spec base: `main` at `767acf9`.
- Files: `crates/designos-app/src/panels/variants_view.rs` (new), `crates/designos-app/src/panels/variants_ranking.rs` (new), the AI panel segmented control added by E15-20 (`crates/designos-app/src/panels/style_gallery.rs` host in `shell.rs`), `crates/designos-app/src/terminal.rs` (agent profile list for the model picker; read-only use), shell tests under `crates/designos-app/src/shell/tests/`.

## 1. Summary and user story

As a designer in Studio, I want to start an exploration without typing into an agent CLI: pick how many variants, same or new style, which model each agent uses, or a quick action; watch each variant's progress and owner; then see the ranking with scores and merge or keep the winner.

## 2. Current state

- FACT: the AI panel opens from `RailItem::Agents` with its own width `ui.ai_panel_width` clamped to at least 260 px (`crates/designos-app/src/app.rs:55-63`, `crates/designos-app/src/shell.rs:1121-1124`). E15-20 adds a `Terminal | Styles` segmented control to its header.
- FACT: known agent CLIs are listed by `AgentCliPreset` (Claude Code, Codex CLI, Antigravity, Grok Build, shell; `terminal.rs:62-101`).
- FACT: E15-36 tools start, report and stop explorations; E15-37 tools rank, merge, keep and restore. RD-3 (#572) shows frame ownership and progress on the canvas.
- FACT: design rules: 28 px rows and tabs, 24 px minimum targets, 4/8 px rhythm, hairlines, separate count badges (D85), monospace for numbers (D6), no fake UI (`design/principles.md` P2–P6).

## 3. Behaviour rules

1. **Entry.** The segmented control becomes `Terminal | Styles | Variants`. `Variants` shows the explorer for the current selection: if a frame is selected it is the anchor; otherwise the explorer offers to build an anchor from the brief.
2. **Setup form** (all controls call real tools; nothing is decorative):
   - **Brief**: multi-line text field (3–8 lines, 2,000 characters max), prefilled from the selected frame's name.
   - **Count**: a stepper `N ×` from 2 to 8 (default 3).
   - **Style**: segmented `Same style | New style`.
   - **Placement**: segmented `Frames | Branches` (Branches disabled with a tooltip when the document cannot hold branches).
   - **Models**: one row per variant (`Variant 1…N`) with a dropdown listing configured agent profiles that can run headless (profiles discovered from the agent runtime bridge, E5-10). A `Same model for all` checkbox (default on) collapses the rows into one dropdown.
   - **Quick actions**: five 28 px chips `Layout`, `Style`, `Colour`, `Typography`, `Surprise`; clicking one starts an exploration immediately with that action and the current count and models.
   - **Start**: primary button (`BG_BRAND` `#E36323`). Disabled with an explanation line when no agent profile is configured (`agent_not_configured`) or the brief is empty.
3. **Progress list** (while running): one 28 px row per variant: colour dot for state (`Running` info blue `#0D99FF`, `Done` success emerald `#14AE5C`, `Failed` danger `#FE5938`, `Stopped`/`Queued` tertiary text), `Variant k`, model name, owner run label, tool-call count in a separate count badge (monospace), elapsed time (monospace, updated only when a status event arrives, no timer), and a `Locate` icon button that selects and zooms to the variant's frame (or switches to its branch). A `Stop all` secondary button calls `designos_stop_variants`.
4. **Ranking view** (when every variant is done, failed or stopped): a `Rank` button calls `designos_rank_variants`; the list shows rows sorted by total score: rank number, thumbnail (CPU render of the frame at 64 px tall, rendered off the UI thread), `Variant k`, model, total, and the three component scores D, C, P in monospace. A caption line states `Ranking: full` or `Ranking: deterministic only (<reason>)` and the judge model. Expanding a row shows the checklist results (item, answer, evidence) and the key deterministic metrics.
5. **Actions per row:** `Merge` (primary on the top row, secondary elsewhere) calls `designos_merge_variant(action: "merge")` after a confirm dialog that states what will be archived; `Keep` calls `action: "keep"`. In the archive part of the list, archived variants show `Restore`.
6. **Live updates.** The view subscribes to exploration status changes delivered through the existing live document sync path; it repaints only when a status or document change arrives (no periodic repaint at rest).

## 4. Data model / schema

UI state only: `VariantsViewState { brief: String, count: u8, mode, placement, same_model: bool, models: Vec<String>, expanded_row: Option<u32> }` in `app.ui`. No document change beyond what the tools write.

## 5. Contracts

Uses E15-36 and E15-37 tools through the same in-process engine calls Studio uses for MCP operations; no new tool.

## 6. File-format mapping

None.

## 7. UI

- Surfaces: panel background `BG_PRIMARY`; form groups separated by 1 px `BORDER` hairlines, not nested cards; 12 px panel padding; 8 px gaps; 28 px control height; labels 11 px body in `TEXT_SECONDARY`; section headings 13 px.
- Counts in pill badges, never in label strings (D85). Numbers (scores, counts, times) in `theme::font_mono()`.
- Focus rings use the brand orange; all interactive targets ≥ 24×24 px.
- Empty state (no exploration yet): one line "Explore variants of the selected frame or a new brief" plus the setup form.
- Error states: a failed variant row shows the last error in one elided line with a tooltip for the full text.
- Evidence: desktop capture at 1440 showing the setup form, a run in progress with three rows, and the ranking view (responsive: SKIPPED, owner default). Load `es-designer` before writing UI code; run `python3 scripts/zero_fake_ui_gate.py`.

## 8. Edge cases

1. Panel narrower than 300 px: model rows stack the dropdown under the label; scores wrap to a second line.
2. An exploration started from an agent CLI (not the UI): the view shows it in the progress list when opened (explorations are read from the document).
3. Two explorations in one document: a picker at the top lists them by brief and date; the newest is selected.
4. Agent profile removed while an exploration references it: the row shows the model name with `not configured` and the variant is `failed` if it had not started.
5. Merge confirm cancelled: nothing happens.
6. Board documents: Variants tab hidden (no frames to explore).

## 9. Non-goals

- Editing archetype parameters per variant (the style assignment is automatic, E15-36 rule 6).
- Side-by-side comparison canvas mode.

## 10. Test plan

Seam: shell tests (`crates/designos-app/src/shell/tests/`) driving the panel with stub agent profiles, plus a `--screenshot` capture.

Fail-before: the view does not exist.

| Test | Input | Expected |
|---|---|---|
| `start_three_shows_three_rows_and_frames` (AC1) | select anchor, N=3, Start with a stub profile | 3 progress rows; 3 frames exist, each owned by a distinct run (RD-3 ownership readout) |
| `ranking_lists_scores_and_merges` (AC2) | after stubs finish: Rank, then Merge on row 1 | rows sorted by total with D/C/P shown; merge archives the others; one undo reverts |
| `start_disabled_without_agent` | no profiles | Start disabled; explanation text visible |
| `quick_action_starts_immediately` | click `Colour` | exploration with mode new_style and quick action colour |
| `no_repaint_at_rest` | idle 2 s after a run finishes | zero repaint requests from the view |
| zero fake UI gate | `python3 scripts/zero_fake_ui_gate.py` | passes |

Visual evidence: the 1440 captures listed in §7.

## 11. Performance budgets

- Panel frame cost ≤ 1 ms with 8 rows.
- Thumbnails rendered off the UI thread and uploaded as textures outside `ctx.data_mut` (CLAUDE.md §4.2).
- No `app.changed()` or `TileView::rev` bump from view interactions that do not edit the document (§4.1).

## 12. Dependencies and build order

- Needs E15-36, E15-37; merge after E15-20 (shared segmented control).

## 13. Owner rulings that apply

- R4.13: models listed are the user's agent CLI profiles; no built-in model.
- Parallel agents are hard-locked to their frames; the UI shows the owner per frame.
- Zero fake UI; full scope (count, mode, per-agent model, quick actions, progress, ranking, merge/keep/restore).
