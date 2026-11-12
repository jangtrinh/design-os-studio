# #1442 Grid export: the remaining track drifts (Fixed implicit rows, Fill in an auto column, S6/S8/S10)

| Field | Value |
|---|---|
| Issue | #1442 (epic #1486) |
| Code baseline | `origin/main@d22dad970`. Re-anchor before coding. |
| Verdict | **Still real** for C3, S6, S8, S10, N2d and the empty-grid nit (code paths unchanged since the 2026-10-31 reviews; #1553 fixed only the levelled-row cases N1b/S9). Re-measure first (AC-1). |
| Size | M: 6–8 h |
| Tier | Argon: reversible, Chrome harness is the oracle (0.00 px). |
| Touches | `crates/designos-export/src/codegen.rs` (`resolved_grid_col_tracks` `:738-785`, `resolved_grid_row_tracks` `:787-806`), `crates/designos-export/src/css.rs` grid block (`:123-142`), `crates/designos-export/src/tailwind.rs` grid block (`:360-368`), `crates/designos-doc/src/layout_solver_grid.rs` (`grid_packs_auto_rows` `:129-132`), `crates/designos-export/tests/grid_chrome_fixtures.rs` |
| Depends on | #1455 first (both edit `codegen.rs` and `layout_solver_grid.rs`). #1440 after this (same `css.rs`/`tailwind.rs`). No open PR touches these files except #1578 (`tailwind.rs`, toolchain). |

## 1. Still real? (FACT on `d22dad970`)

- Rows: `resolved_grid_row_tracks` emits solver px rows only when the solver levels rows (`!grid_packs_auto_rows`, `codegen.rs:793-801`). Every packed grid keeps the stored tracks and count `grid_rows` (`:805`).
  - C3 (`grid_rows == 0`, stored Fixed row tracks, packed): solver uses `grid_row_tracks[r]` for implicit rows (`fixed_row`, `layout_solver_grid.rs:134-157`, branch `explicit_rows == 0`); export count = 0 → no `grid-template-rows` (`css.rs:131-134`). → drift.
  - S8 (`grid_auto_tracks = Fixed(50)`, packed): solver sizes implicit rows 50 (`fixed_row` last branches); export writes no `grid-auto-rows` (grep `grid-auto-rows` in `css.rs`/`tailwind.rs`: 0 hits). → drift.
  - S6 (explicit `grid_rows` with default Flex row tracks, packed): solver sizes explicit Flex rows from content (`row_heights` → `auto_row_height`, `layout_solver_grid.rs:229-231`; explicit rows frozen in levelling, `:164`); export writes `repeat(n, minmax(0, 1fr))` (`grid_tracks_css`, `codegen.rs:826-851`). → drift.
- Columns: `resolved_grid_col_tracks` returns stored tracks for any grid that does not hug its width (`codegen.rs:739-741`).
  - S10 (fixed-width grid, only Hug columns): solver widths = widest child (`column_widths`, `layout_solver_grid.rs:180-183`); CSS `auto` tracks stretch to fill (`justify-content: normal`). → drift.
  - N2d (Fill child alone in a Hug column, any grid): solver `widest_in_column` counts a Fill child at its minimum; CSS `auto` sizes to content/stretch. → drift.
- Nit: `grid_packs_auto_rows` uses `.max(1)` (`layout_solver_grid.rs:130`), so an empty Fixed + Flex-auto grid reports "levels" and loses `align-content: start`, while `compute_grid` returns early for no children (`:266-270`). No geometry effect.

## 2. User-visible outcome

Every listed grid exports to CSS and Tailwind with each child at Studio's position (0.00 px in Chrome); tracks come from the solver, so a later solver change cannot drift the export.

## 3. Behaviour rules (decided)

- R1 Rows: when the stored row tracks would not reproduce the solver's rows in CSS (any packed grid with a Fixed implicit track, a Fixed auto track, or explicit Flex rows that the solver sizes from content), emit `grid-template-rows` covering every solver row, one token per row by how the solver sized it: Fixed track → `{h}px`; content-sized row in a packed grid → `auto` (with the existing `align-content: start`, CSS also sizes it from content, and it keeps responding to text changes); levelled row → `{h}px` (as today). Keep today's output when it already matches (packed Hug/auto rows) so unchanged goldens stay unchanged. The per-row "how sized" answer comes from a solver accessor (add `pub fn grid_row_kinds` beside `grid_row_heights`, `layout_solver_grid.rs:119`), not from a re-derivation in the exporter.
- R2 `grid-auto-rows`: not needed once R1 lists every solver row; do not add a second rule.
- R3 Columns: any `GridTrack::Hug` column exports its solver width in px, in every grid (extends today's hugging-grid rule at `codegen.rs:760-764` to fixed/fill grids). Flex columns in fixed grids stay `minmax(0, Nfr)` (responsive), Fixed stay px.
- R4 Empty grid: `grid_packs_auto_rows` returns true when there are no flow children (mirror `compute_grid`'s early return).
- R5 One source: the decision "does CSS reproduce this track?" lives in `codegen.rs` next to `resolved_grid_*`, fed by `designos_doc::layout_solver` accessors; no solver rule copied into `css.rs`/`tailwind.rs` (issue "no second copy").

## 4. Acceptance criteria

- AC-1 Re-measure on `d22dad970`: add scenes C3, S6, S8, S10, N2d and "empty Fixed + Flex auto" to `grid_chrome_fixtures.rs` (definitions from `plans/reports/code-reviewer-261031-0845-pr-1426.md:126-129`, `…-1100-pr-1426-r2.md:92,128`, `…-1429-pr-1426-r3.md:105`; probe drivers in their `-evidence/` folders). Run `python3 scripts/qa/grid-chrome-compare.py`; record the baseline drifts (expect ≈ C3 20, S6 75, S8 30, S10 100, N2d 30 px).
- AC-2 After the fix: all scenes, old and new, ≤ 0.5 px (target 0.00) in CSS and Tailwind; harness exit 0; log + screenshot PNG under `../e19-e18/evidence/1442/`.
- AC-3 Unit tests in `crates/designos-export/tests/grid_codegen_resolved_row_tracks.rs` (rows) and `grid_codegen_track_counts.rs` (columns) pin the new track text for each scene (red first).
- AC-4 Empty-grid nit: test that an empty Fixed + Flex-auto grid emits `align-content: start` / `content-start`.
- AC-5 Existing goldens: only scenes covered by R1/R3 change; list them in the PR. `cargo test -p designos-export`, `-p designos-doc --lib` green; clippy clean.

## 5. Test steps

1. AC-1 scenes + baseline run (needs Chrome; `CHROME_BIN` optional).
2. Red unit tests (AC-3, AC-4).
3. Implement R1–R5; rerun harness and tests.

## 6. Edge cases

- Bound gap/padding variables: rows/cols come from `solved_grid` which already resolves variables (`codegen.rs:668-700`, `solver_layout`).
- Row spans across Fixed and auto rows: px rows keep span math exact.
- Grid with only absolute/hidden children: `solved_grid` returns None (`:687-689`); keep stored output.
- More rows than `MAX_GRID_TRACKS`: clamp as today.
- Tailwind arbitrary values: `grid-rows-[20px_30px]` via `tailwind_tracks_value` (`codegen.rs:854-856`).

## 7. Non-goals

Column-locked placement order (#1441, closed); strokes (#1440); native exporters (#1455/#1456).

## 8. Figma-parity facts

None needed: the oracle is Studio's own solver, measured in Chrome.

## 9. Undecided user-visible details

None material. Decided: exported px rows replace `1fr`/`auto` only where they differ from the solver (R1), so most users' output is unchanged.

## 10. Builder tier

Argon.

## Epistemic ledger

- FACT: code paths for C3/S6/S8/S10/N2d unchanged on `d22dad970` (lines above).
- INFERENCE: drift magnitudes unchanged (not re-measured here; AC-1 does it).
- ASSUMPTION (high): px rows for packed grids are acceptable output (rows in a packed grid are content-sized anyway).
