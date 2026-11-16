# #1456 Native grid export: flexible tracks as weights, not solved pixels

| Field | Value |
|---|---|
| Issue | #1456 (epic #1486) |
| Code baseline | `origin/main@d22dad970`. Re-anchor before coding. |
| Verdict | **Still real.** All four native generators write every column as the solver's px width at the frame's current size. |
| Size | M–L: 9–12 h (four targets, each with a compile check) |
| Tier | Claude Sonnet (per-platform API judgment; output must compile) |
| Touches | `crates/designos-export/src/codegen.rs` (`SolvedGrid` `:603-611`, `solved_grid` `:674-700`: add per-column track kind + weight), `crates/designos-export/src/compose.rs` (`:440-443`), `crates/designos-export/src/android_xml.rs` (`:376-412`), `crates/designos-export/src/uikit.rs` (`:177-300`), `crates/designos-export/src/swiftui.rs` (`:466-660`), `crates/designos-export/tests/grid_native_fixtures.rs` |
| Depends on | #1455 (same files, lands first). #1457 strongly recommended first, so every new output is compile-checked (soft: can start, cannot close without it). Owner Q2. |

## 1. Still real? (FACT on `d22dad970`)

- `SolvedGrid.widths` is `grid_column_widths` at `n.size` (`codegen.rs:691`); no track kind is kept.
- Compose: `GridCells.calculateCrossAxisCellSizes` returns `listOf(<px>.dp…)` and ignores `availableSize` (`compose.rs:440-443`).
- Android XML: each column spacer gets `layout_width="{w}dp"` (`android_xml.rs:390-395`); cells `layout_width="{cell_w}dp"` (`:424-431`). No `layout_columnWeight`.
- SwiftUI: every cell `.frame(width: <px>, height: <px>)` (`swiftui.rs:624-630`); empty cells `Color.clear.frame(width:…)` (`:654`).
- UIKit: rows of `UIStackView` with fixed-width cell containers (`uikit.rs:236-300`, `cell_width` at `:270`).

## 2. User-visible outcome

Exported native code for a grid whose frame is Fixed or Fill width keeps Flex columns flexible: on a wider or narrower device the columns share the space by their weights, like the canvas grid. Fixed columns stay fixed; Hug columns keep their solved width.

## 3. Behaviour rules

- R1 Data: `SolvedGrid` gains `col_tracks: Vec<TrackOut>` with `TrackOut::{Fixed(px), Hug(px), Flex { weight, px }}` built from `al.grid_col_tracks` (default Flex(1.0)) and the solver widths. A hugging-width grid (`primary_sizing == Hug`) reports every column as `Hug(px)` (its Flex columns are content-sized in the solver, `layout_solver_grid.rs:194-195`).
- R2 Compose: `calculateCrossAxisCellSizes(availableSize, spacing)` computes fixed px (Fixed, Hug) then splits `availableSize − fixed − spacing·(n−1)` by weight; emitted as a small Kotlin expression, no helper class.
- R3 Android XML: Flex columns → `layout_width="0dp"` + `layout_columnWeight="{weight}"` on the column spacer and on cells starting in that column; Fixed/Hug → dp as today.
- R4 UIKit: Flex column containers get width constraints relative to the first Flex column (`multiplier = weight / weight_ref`); Fixed/Hug keep constants.
- R5 SwiftUI: per owner Q2 (recommended A: equal-weight Flex columns → `.frame(maxWidth: .infinity)`; unequal weights → px as today plus a one-line comment naming the weights).
- R6 Rows: unchanged (px heights) in all four targets, except Android when the solver levels rows (Fixed height + Flex auto rows): `layout_rowWeight`. Decided: rows in Compose/SwiftUI/UIKit are content- or px-sized today and stay so (non-goal).
- R7 Spans: a cell spanning Flex columns uses the sum of weights (Android `columnWeight` on a spanning cell = sum), Compose `GridItemSpan` unchanged.

## 4. Acceptance criteria (red on `d22dad970`)

- AC-1 Fixture "fixed 400 wide, no padding, tracks Fixed 100 / Flex 1 / Flex 2, column gap 10": the emitted Compose `calculateCrossAxisCellSizes` yields [100, 93.33, 186.67] dp at availableSize 400 and [100, 193.33, 386.67] at 700 (rounding to px per Compose's `roundToPx`). Asserted twice: the expression text in a Rust test, and the numbers in a Kotlin unit test run by the #1457 job.
- AC-2 Android: the same fixture has `layout_columnWeight="1"` and `"2"` on Flex columns, `layout_width="100dp"` on the Fixed one.
- AC-3 UIKit: width constraints with multiplier 2.0 between the two Flex columns; Fixed column constant 100.
- AC-4 SwiftUI per Q2: equal-weight fixture emits `.frame(maxWidth: .infinity…)`; unequal emits px + weight comment.
- AC-5 Hugging-width grid fixture: output unchanged from today (all px).
- AC-6 Every fixture compiles in the #1457 job (swiftc SwiftUI + UIKit, Compose, aapt2/Android).
- AC-7 Existing `grid_native_fixtures.rs` solver-geometry assertions still hold at the frame's own size (the design size renders identically).
- AC-8 Visual: screenshots of the Compose preview / iOS simulator are NOT required (owner default); instead a terminal capture of the compile job and the fixture diff.

## 5. Test steps

1. Red: AC-1..AC-5 fixtures in `grid_native_fixtures.rs`.
2. Implement R1–R7.
3. Run `cargo test -p designos-export`; run the #1457 job locally (`scripts/ci/native-export-compile-check.sh`, from #1457) on macOS.

## 6. Edge cases

- `Flex(0)` or non-finite weight: treat as 1.0 (same as `grid_track_token`, `codegen.rs:810-812`).
- All columns Fixed: identical to today.
- Fill-width grid inside a Hug parent: weights still emitted; the platform resolves width.
- Bound gap variables: spacing tokens unchanged (`dp(doc, n, "itemSpacing", …)`).
- Column count > `MAX_GRID_TRACKS`: clamp as today.

## 7. Non-goals

Row weights outside Android; web exporters (#1442); SwiftUI `LazyVGrid` migration.

## 8. Figma-parity facts

Not applicable (export target semantics). Platform facts the builder must confirm in current docs before coding (`[UNVERIFIED]` here): Android `GridLayout` `layout_columnWeight` (API 21+); Compose `GridCells` custom `Density.calculateCrossAxisCellSizes(availableSize: Int, spacing: Int)`; SwiftUI `Grid` column sizing with `maxWidth: .infinity` cells.

## 9. Undecided user-visible details

- Q2: SwiftUI shape for weighted columns.

## 10. Builder tier

Claude Sonnet.

## Epistemic ledger

- FACT: px-only output in all four generators (lines above).
- INFERENCE: SwiftUI `Grid` has no native per-column weights; equal flexible cells split remaining width equally.
- ASSUMPTION (medium): rows stay px outside Android. Flips if the owner wants row weights too.
