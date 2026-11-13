# Tables for 0.7.11: build-ready spec pack

## Changelog

**2026-11-07, revision after T0** (source: `plans/reports/tables-t0-probe.md`, the owner's 2x4 FigJam board).
- **Why:** the real file contradicts the pack's file-format assumption. Figma writes no `TABLE_CELL` record. A table is one `TABLE` record, and a cell is the pair `(rowId, colId)` addressing overrides inside it (text in `nodeGenerationData`, layout in `derivedImmutableFrameData`).
- **T0:** AC-2 now counts derived frame entries, not `TABLE_CELL` lines. Fixture F1 is superseded by the committed 2x4 golden plus F2 (owner re-save). Status table per unknown added. Output path corrected.
- **T1:** the model was re-decided on its merits, and it stays cells as child nodes. The deciding facts are the text session, Find and undo, which are all keyed by `NodeId`. New decision 8 adds tree guards, plus AC-8 (+1 h). Kind checks run per command in `Command::apply`. Position checks run once per undo step in `History`, with rollback, because `Command::Batch` is not atomic and inverses run in reverse order. The "mirrors Kiwi TABLE_CELL" reason is gone.
- **T3 split:**
  - **T3a** covers unmerged tables, both directions, with golden `figjam-table-2x4.jam`. It starts after T1, without waiting for T0 or the owner.
  - **[T3b](T3b-fig-table-merges-second-fixture.md)** covers merges, reorder ordering and per-cell style. It has a **hard gate**: F2 must be committed and re-probed first.
  - The synthetic `TABLE_CELL` emitter in `fig_roundtrip.rs` is flagged for replacement.
- **Other specs:**
  - T1b ops must leave each table whole at the end of a step (guard, no ordering rule).
  - T2 records the real file's look as input to the OQ-1 / OQ-2 mocks.
  - T4 gets the exact Layers filter for owner ruling OQ-16, plus AC-9 (layer commands on a cell).
  - T5b refuses cell-breaking edits in the generic MCP node tools, and refuses Slides per ruling OQ-13.
- **README:**
  - Rulings R1 and R2 are marked granted.
  - New gate F2.
  - Critical path recomputed: 48 h, T3 chain slack about 19 h.
  - Contradictions with controller rulings are listed below.

**2026-11-06 23:58:** pack written. Owner ruling: tables ship in 0.7.11, and the release waits for them.

## Issues

- #317: model, render and `.fig` round trip, merged cells (E9-12).
- #331: editing UI (E9-13).
- #1231: WB-16 Table tool in Design and Board files.

Epic context: #1215 (whiteboard in Design files; WB-1 #1216 merged, so capability checks replace file-kind checks) and E9 #22.

Code baseline: `origin/main@d22dad970` (fetched 2026-11-06). Citations added on 2026-11-07 say `main@ec47da26d`. Every builder re-anchors against HEAD and reports drift before coding.

## Ground truth (verified by the planner)

- **FACT:** `origin/main` has no table node kind. `NodeKind` (`crates/designos-doc/src/node.rs:487-537`) has `Sticky`, `Connector`, `ShapeWithText`, `Stamp` and `Unknown`. A real Figma `TABLE` imports as `Unknown{TABLE}` with no children. The table fields, `nodeGenerationData` and `derivedImmutableFrameData` are kept in `fig_metadata` (T0 run on the 2x4 board).
- **FACT:** the X1 passthrough keeps Kiwi fields 308-311 and 538 on `Unknown{TABLE}` (`fig_roundtrip.rs:2044-2054`). That test's fixture is synthetic, and it emits `TABLE_CELL` records that Figma never writes. T3a replaces it.
- **FACT:** board items draw by lowering to ordinary nodes in both renderers and in SVG/PDF export (`designos-render/src/board.rs:1-8, 173, 237`; `lib.rs:494`; `designos-gpu/src/compile.rs:891`). The synthetic-child id has 4 slot bits (`board.rs:28-30`). The text session, Find and undo are keyed by `NodeId`. Both facts keep cells as real nodes (T1 re-check).
- **FACT (T0):** what Figma writes for a table:
  - One `TABLE` record (type 29) and zero `TABLE_CELL` records.
  - Tracks in 308 / 309: GUIDs in the file's id space, with fractional-index position strings.
  - Column widths are written. A default row height is omitted (64 in that file).
  - Cell text sits at `nodeGenerationData` override `[40000000:1, rowId, colId]`. An empty cell has no entry.
  - The shared cell fill and stroke sit on template `[40000000:2]`, the shared text style on `[40000000:3]`.
  - `derivedImmutableFrameData` holds one frame and one text entry per cell.
- **UNKNOWN (no merged cell in the fixture):**
  - merge storage (field 538 meaning, anchor);
  - covered positions;
  - reordered-track ordering;
  - per-cell style overrides;
  - a header flag;
  - whether Figma opens Studio's output (U9).
  - Each one has a named probe in [T0](T0-kiwi-table-probe.md) / [T3b](T3b-fig-table-merges-second-fixture.md).
- **FACT:** `.brv/context-tree/architecture/crate-map.md:59` lists `Table`, `WashiTape`, `CodeBlock` and `Slice` as node kinds. That is stale; do not cite it (T6 fixes it).

## Phases

| Phase | Spec | Size (builder h) | Tier | Depends on | Parallel with |
|---|---|---|---|---|---|
| T0 | [Kiwi probe + fixtures](T0-kiwi-table-probe.md) | first run DONE; F2 re-probe 1.5 h + owner 10 min | Sonnet + owner | none | everything |
| T1 | [Model, layout, serde, load repair, tree guards, MCP read](T1-table-model-layout-serde-repair-mcp-read.md) | 10 h | Sonnet | WB-2 #1217 merged; R1 (granted) | T0 |
| T1b | [Edit ops, one undo step each](T1b-table-edit-ops-undo.md) | 6 h | Sonnet | T1 | T2, T3a |
| T2 | [CPU render, GPU parity, PNG/SVG/PDF](T2-cpu-render-gpu-parity-export.md) | 9 h | Sonnet | T1 | T1b, T3a |
| T3a | [`.fig` round trip, unmerged tables, legacy upgrade](T3-fig-import-export-table-round-trip.md) | 9 h | Sonnet | T1, fig PRs #1664/#1671/#1685 (+#1672 per ledger), R2 (granted), golden fixture PR | T1b, T2, T4, T5b |
| T3b | [`.fig` merges, reorder, per-cell style](T3b-fig-table-merges-second-fixture.md) | 5 h | Sonnet | **F2 committed + re-probed (hard gate)**, T3a, T1b | T4, T5, T5b |
| T4 | [Editing UI: Tab, add/remove/reorder/resize, merge, IME](T4-table-editing-ui.md) | 16 h | Sonnet (es-designer) | T1b, T2, rulings in, OQ-2/7/9 mocks answered | T3a, T3b, T5b |
| T5 | [Table tool, Shift+T, toolbar, Design + Board, export](T5-table-tool-toolbar-design-and-board.md) | 8 h | Sonnet (es-designer) | T4, WB-3 #1218, OQ-1 answered | T3b, T5b |
| T5b | [MCP `table` op in `designos_apply_batch`, node-tool guards](T5b-mcp-batch-table-op.md) | 6 h | Sonnet | T1b | T3a, T3b, T4, T5 |
| T6 | [Perf, docs, release evidence](T6-docs-perf-release-evidence.md) | 5 h | Sonnet | T2, T3a, T3b, T4, T5, T5b | none |

Total: about 75.5 builder hours from here (the T0 first run is done) plus review rounds (about 1.5 h each per phase). Opus reviews every phase, and the controller reruns the gates before merging.

## Dependency graph

```
T0 first run (DONE) ─► golden 2x4 ─────────────┐
WB-2 #1217 ─► T1 ─┬─► T1b ─┬─► T4 ─► T5 ─► T6 ─► tag 0.7.11
                  │        └─► T5b ───────┘ ▲     ▲
                  ├─► T2 ──────► (T4) ───────┤     │
                  └─► T3a ◄── golden, fig PRs │     │
                        └─► P4 ─► T3b ─► P4' ┘     │
owner F2 ─► T0 re-probe ─► (T3b gate) ─────────────┘
WB-3 #1218 ─────────────────────► (T5)
OQ-2/7/9 mocks ─► (T4); OQ-1 ─► (T5)
```

## File ownership: no two parallel phases share a file

| Phase | Owns |
|---|---|
| T1 | `designos-doc/src/{node.rs, table.rs, table_tests.rs, lib.rs, history.rs (guard only), load_repair.rs, load_repair_tests.rs, openpencil.rs, branch_diff.rs}`; `designos-mcp/src/tools/metadata.rs`; `designos-mcp/tests/table_read.rs`; single-line forced arms elsewhere (listed in its PR) |
| T1b | `designos-doc/src/{table_ops.rs, table_ops_tests.rs}` + one `lib.rs` line (T1 is merged by then) |
| T2 | `designos-render/src/board.rs` (+ `geom.rs` arm list), new tests in `designos-render`, `designos-gpu` and `designos-export` |
| T3a | `designos-fig/src/{fig.rs, rest_import.rs, fig_export.rs, export.rs, table.rs(new)}`, `designos-doc/src/table_kiwi.rs` (new) + one `load_repair.rs` line, `designos-fig/tests/*table*`, the table part of `fig_roundtrip.rs` |
| T3b | the same files as T3a, serial after it, plus the F2 fixture |
| T4 | `designos-app/src/board/table/*` (new), `panels/layers.rs` filter, hook lines in `canvas/interact.rs`, `canvas/textsession.rs`, `shell.rs`, `shell/tests/table_edit.rs` |
| T5 | `designos-app/src/canvas/tools.rs`, `board/{caps.rs, create.rs, toolbar.rs}`, the WB-3 group file, the creation branch in `canvas/interact.rs` (serial after T4), shortcuts dialog, icon, `shell/tests/table_tool.rs` |
| T5b | `designos-mcp/src/tools/{batch_table.rs(new), batch.rs, mod.rs}`, the node-tool refusal lines (file cited by the builder), `designos-mcp/tests/table_batch.rs`, tool snapshot |
| T6 | `designos-gpu/tests/table_perf.rs`, `README.md`, `CLAUDE.md` (tool row), `CONTEXT.md`, `crate-map.md`, docs page |

Overlaps, all serial and never parallel:
- T1 and T3a share `load_repair.rs` (T3a adds one line after T1 merges).
- T1 and T3a share `fig_export.rs` / `export.rs` (T1 single-line arm, which T3a replaces).
- T3a and T3b share every T3 file (T3b after T3a).
- T4 and T5 share `canvas/interact.rs` (T5 after T4).
- T1b and T5b both read `table_ops`; only T1b writes it.

## Waves

1. **Now:**
   - Merge the golden fixture PR (`jangtrinh/tables-t0-probe`).
   - Ask the owner for F2 (this morning).
   - Owner mocks for OQ-1/2/7/9 wait for T2.
2. **After WB-2 merges:** T1.
3. **After T1 merges:**
   - T1b, T2 and T3a run in parallel. T3a waits only for the fig PRs.
   - The T0 re-probe runs as soon as F2 lands.
4. **After T1b and T2 merge:** T4 and T5b run in parallel, alongside T3a if it is still running.
5. **After T3a and P4, with the F2 gate clear:** T3b.
6. **After T4 merges and WB-3 is in:** T5.
7. **After all of them:**
   - T6.
   - The controller's final gate rerun.
   - P4 again on T3b's merged-table output.
   - The 0.7.11 tag.

## Critical path to the 0.7.11 tag

T1 10 h → T2 9 h (longer than T1b's 6 h) → T4 16 h → T5 8 h → T6 5 h = **48 builder hours**. With five review and CI rounds (about 8 h), the elapsed time is about 56 h of work.

T3 chain: T1 → T3a 9 h → T3b 5 h, about 14 h after T1, against about 33 h before T6 starts. That leaves **about 19 h of slack**, less the owner's P4 turnaround after T3a, provided F2 is committed and re-probed before T3a finishes (about 9 h after T1 merges). If F2 arrives later, every hour of delay past that point eats the slack. Once the slack is used up, the F2 gate is on the critical path.

External gates on the path:

| Gate | Status 2026-11-07 | Effect |
|---|---|---|
| WB-2 #1217 (Argon, owns `designos-doc` load repair) | OPEN, in flight | T1 start |
| WB-3 #1218 (Argon, toolbar group) | OPEN, in flight | T5 start (ruling OQ-14: wait) |
| Owner mocks OQ-1/2/7/9 (visual, after T2) | not asked | T4 / T5 start |
| fig PRs #1664, #1671, #1685 | OPEN | T3a start |
| Golden fixture PR (`figjam-table-2x4.jam` + seed test) | pending | T3a start |
| **F2: owner re-saves a board with a merge, a reordered row, a styled cell and Vietnamese text** | **to ask this morning** | **T3b start (hard gate), therefore the tag** |
| R2 hot-file queue | GRANTED (#317 ahead of #224) | none |
| P4 owner check (Figma opens Studio's TABLE), after T3a and after T3b | after T3a / T3b | T3b start / tag |

## Rulings (controller, `controller-rulings.md`)

- **R1, granted.** Single-line match arms forced by the new node kinds may land in hot-file-queue files (`fig_export.rs`, `export.rs`) during T1. They keep today's `Unknown{TABLE}` output.
- **R2, granted.** #317 moves ahead of #224 in the hot-file queue. #224 is v0.17 and not started.

## Spec items that contradicted a controller ruling (found 2026-11-07)

| Item | Ruling | Status |
|---|---|---|
| README called R2 "BLOCKER / pending" and said the critical path "does not hold"; R1 was a "Proposal" | R1 and R2 granted | Fixed here, in T1 (header, forced-arm rule, risk row) and in T3a (Depends on) |
| T1 non-goal "no header field unless the owner picks a header option" | OQ-3 (a): no special header | Fixed in T1 |
| T5b "In Slides it follows OQ-13" (left open) | OQ-13 (a): Board and Design only, no Slides | Fixed: T5b refuses in Slides with a next-action message |
| T4 listed OQ-16 and OQ-4 as "recommended", and its dependencies as unanswered rounds 1-3 | OQ-16 owner-ruled; OQ-4, -15 and the rest ruled | Fixed in T4 |
| T1b merge text "kept or dropped per owner OQ-5" | OQ-5 (a): appended, one line per covered cell | Fixed in T1b |
| T5 depended on "owner answers round 4 (OQ-10, -12, -13, -14) plus OQ-1 to OQ-3" and labelled OQ-12 / OQ-13 as recommended; its WB-3 risk row left OQ-14 to a later decision | OQ-3, -10, -12, -13, -14 ruled (OQ-14: wait for #1218) | Fixed in T5; only OQ-1 (and OQ-2) remain owner questions |
| `owner-questions.md` still lists ruled questions as open (OQ-3, -4, -5, -6, -8, -10 to -16) | rulings file | Not changed here. The open set is OQ-1, -2, -7 and -9, asked with mocks after T2. |

OQ-13 re-ruled 2026-11-07 01:10 (controller): new tables use FigJam's measured default text, Inter Medium 16, in Board and Design files, the same as imported tables (T0 FACT). See `controller-rulings.md`.

## Test matrix

| Level | What | Where |
|---|---|---|
| Unit (doc) | layout, merges, repair rules, tree guards, every op's undo/redo, 50×50 build, random-op invariant fuzz | T1, T1b |
| Unit (render/export) | merged block draws as one, grid skips merges, SVG/PDF text, PNG fills, `lowered_document` order bug | T2 |
| Parity | CPU vs GPU pixel diff on table scenes, interactive compile opens no extra programs | T2 |
| Integration (fig, raw) | what Figma writes: one `TABLE`, overrides keyed `[template, row, col]` | `figjam_table_raw_records.rs` (T0; extended by the F2 re-probe) |
| Integration (fig) | golden import, native export → decode → re-import, golden → export → import, no-ratchet heights, typing gate, legacy upgrade | T3a |
| Integration (fig, merges) | F2 import, merged native round trip, reorder order, per-cell style | T3b |
| Integration (MCP) | metadata/xpath/get_node read; batch `table` op atomic, one undo group, 50×50; node-tool refusals | T1, T5b |
| UI (shell tests) | Tab order, one undo step per UI action, IME, refused merge, Tab elsewhere unchanged, Layers filter, layer commands on a cell, Shift+T conflicts, Design + Board | T4, T5 |
| Perf | 50×50 compile ≤ 8 ms or drains ≤ 10 frames, idle 0% CPU, open ≤ 2 s | T6 |
| Manual (owner, flagged) | F2 authored in FigJam; Studio's `.fig` opens in FigJam and Figma Design (P4 after T3a and after T3b) | T0, T3a, T3b |
| Visual | 1440 captures per UI phase, Figma update before gates | T4, T5, T6 |

## Backwards compatibility

- Files without tables are unchanged (T1 AC-2).
- Files holding `Unknown{TABLE}` from earlier `.fig` imports keep working in T1. T3a upgrades them on load when they match its typing gate; T3b widens that to merged tables. Otherwise they stay `Unknown` with all fields kept (T3a AC-7).
- Files saved with tables need 0.7.11 to open (serde tag; noted in T6).

## Rollback

Each phase is one squash commit that reverts cleanly:
- T5 → T4 → T2: the feature degrades to "renders but cannot be edited", then to "kept but not drawn".
- T3b revert: merged tables import as `Unknown` again (T3a gate), and native merges export without 538, counted.
- T3a revert: back to the X1 passthrough (T3b must be reverted first).
- T1 revert: only if T1b-T6 are reverted first.

## Phase risk summary (High items have mitigations in each spec)

| Phase | Top risk | L×I |
|---|---|---|
| T1 | WB-2 overlap in `designos-doc`; a generic edit breaks a table (guards, AC-8) | H×M / M×H |
| T2 | `lowered_document` overwrites lowered cells | H×H (fixed and tested in T2) |
| T3a | Figma needs `derivedImmutableFrameData` (P4 at the end of T3a) | M×H |
| T3b | F2 arrives late and the gate lands on the critical path | M×H |
| T4 | Tab precedence breaks existing Tab paths | M×H |
| T5 | WB-3 slips | M×H |

## Owner questions

[owner-questions.md](owner-questions.md) holds 16 questions. The controller has ruled most of them (`controller-rulings.md`), and the owner answered OQ-16. Still open, asked with mocks once T2 renders tables: OQ-1, OQ-2, OQ-7 and OQ-9. New ask for this morning: fixture F2 (T3b).
