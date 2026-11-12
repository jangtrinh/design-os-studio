# #1455 Export reads grid row gap and cell alignment from the layout solver

| Field | Value |
|---|---|
| Issue | #1455 (epic #1486) |
| Code baseline | `origin/main@d22dad970`. Re-anchor before coding. |
| Verdict | **Still real.** Both solver rules are copied into the exporter. |
| Size | S: 2–3 h |
| Tier | Grok build-fast (small reversible refactor; existing native and web fixture tests are the local check). Review by Claude, never by Grok. |
| Touches | `crates/designos-doc/src/layout_solver_grid.rs` (two new `pub fn`s), `crates/designos-doc/src/layout_solver.rs` (re-export), `crates/designos-export/src/codegen.rs` (`solved_grid` `:694`, `grid_row_gap_key` `:712-718`, `grid_cell_align` `:720-736`, CSS-var gap `:989`), `crates/designos-export/src/css.rs:136`, `crates/designos-export/src/tailwind.rs:370` |
| Depends on | Nothing. Must land before #1442, #1440, #1456 (they share these files). |

## 1. Still real? (FACT on `d22dad970`)

Solver rules (source of truth), `crates/designos-doc/src/layout_solver_grid.rs`:
- Row gap fallback: `let row_gap = if al.counter_spacing > 0.0 { al.counter_spacing } else { col_gap };` at `:223` (`row_heights`) and `:264` (`compute_grid`).
- Cell alignment fallback: `cell_offset` (`:317-332`): `Auto` follows the container's `primary_align` / `counter_align` (Center → center, End → end, else start).

Copies in the exporter:
- `codegen.rs:694` (`solved_grid`) row-gap rule; `codegen.rs:713` (`grid_row_gap_key`) a variant of it (uses `> 0.0` or a binding to pick the variable key); `codegen.rs:989` (CSS var fallback) row-gap rule.
- `css.rs:136`, `tailwind.rs:370`: row-gap rule on the stored (unresolved) `al`.
- `codegen.rs:720-736` (`grid_cell_align`): copy of `cell_offset`'s `Auto` mapping. Used by SwiftUI (`swiftui.rs:626,639`), Compose (`compose.rs:502,509`), UIKit (`uikit.rs:279,294`), Android XML (`android_xml.rs:448`).

## 2. User-visible outcome

No change in exported code for today's documents (goal: zero golden diffs). A possible latent mismatch closes: when `counterAxisSpacing` is bound to a variable whose stored value is 0 but resolves > 0, CSS/Tailwind today pick the gap from the stored 0 (`css.rs:136`) while the solver uses the resolved value (`solver_layout`, `codegen.rs:668-672`).

## 3. Behaviour rules (decided)

- R1 Add to `layout_solver_grid.rs`, re-exported via `layout_solver`:
  - `pub fn grid_row_gap(al: &AutoLayout) -> f32` — the one rule; used by `row_heights`, `compute_grid`, and every exporter site.
  - `pub fn resolve_cell_align(child: GridChildAlign, container: LayoutAlign) -> GridChildAlign` — maps `Auto` to Start/Center/End; `cell_offset` is rewritten to call it, so solver and export share it.
- R2 `SolvedGrid` gains no new field; `solved_grid` calls `grid_row_gap(&al_s)`.
- R3 `grid_row_gap_key` keeps choosing the variable key but its numeric predicate calls the solver: "row gap is its own" = `al.counter_spacing > 0.0 || bound(counterAxisSpacing)` stays a key choice, documented as such (it selects a variable name, not a size).
- R4 CSS and Tailwind take the row gap from `solved_grid(...).row_gap` when a grid has flow children (resolved variables), else `grid_row_gap(al)`.
- R5 `grid_cell_align` becomes a thin wrapper over `resolve_cell_align` (or is deleted and callers call the solver fn directly).

## 4. Acceptance criteria

- AC-1 Grep check in a test or CI step: no `counter_spacing > 0.0 {` row-gap ternary outside `layout_solver_grid.rs` (`rg -n "counter_spacing > 0\.0 \{ al" crates/designos-export` → 0 hits).
- AC-2 Guard test: grid with `counterAxisSpacing` bound to a variable resolving to 12 while stored 0, item spacing 4 → CSS row gap is the variable (fallback = the resolved 12px the solver uses), Tailwind `gap-y` likewise, and the row positions in `SolvedGrid` match. Run it on `d22dad970` first: the CSS-var path (`codegen.rs:989-997`) may already emit the variable, so this may be green before the change; record which. It must be green after.
- AC-3 Solver unit test: `resolve_cell_align(Auto, Center) == Center`, `(Auto, End) == End`, `(Auto, Start/SpaceBetween) == Start`, explicit values pass through; `compute_grid` outputs unchanged (existing `layout_solver_grid` tests green).
- AC-4 All existing export tests green with zero golden changes except AC-2's new case: `cargo test -p designos-export`, `cargo test -p designos-doc --lib`.
- AC-5 Terminal capture of the two test runs saved as evidence.

## 5. Test steps

1. Add AC-2 and AC-3 tests; run on the baseline and record AC-2 red or green (the refactor has no other red-first case: it must not change output).
2. Implement R1–R5; rerun all; grep AC-1.

## 6. Edge cases

- Negative `item_spacing` (`.max(0.0)` at `:222`): keep inside `grid_row_gap`.
- `SpaceBetween`/`SpaceAround` container align: map to Start (as `cell_offset` does today).
- Baseline align: Start.

## 7. Non-goals

Changing the solver's rule; track weights (#1456).

## 8. Figma-parity facts

Not applicable (internal single-source refactor).

## 9. Undecided user-visible details

None.

## 10. Builder tier

Grok build-fast; Claude reviews.

## Epistemic ledger

- FACT: duplicated rules at the listed lines.
- INFERENCE (low): the bound-row-gap mismatch is user-visible; `css.rs:136` reads the stored `al`, but `apply_css_vars` (`codegen.rs:989-997`) later rewrites `gap` with the variable, which may hide it. AC-2 settles it; the refactor is justified by de-duplication alone.
