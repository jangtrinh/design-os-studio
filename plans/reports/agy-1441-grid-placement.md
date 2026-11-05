# Report — `agy-1441-grid-placement` (`#1441`)

## 0. What Could Not Be Verified

1. **Figma live runtime execution of a partial column-locked (`row: None, column: Some(c)`) node inside Figma Desktop:** In Figma's binary Kiwi schema (`27756-1dc8c27de1ebafa4.min.js` byte offsets `13493033` and `13493330`) and native C++/WASM scene graph (`fullscreen/lib/scenegraph/FGGridCache.cpp` `func[19807]` at WASM offset `0x04a8c3` / file offset `0x116e4be`; `fullscreen/lib/scenegraph/FGGridLayout.cpp` `func[5480]` `updateGridChildPositions` at WASM offset `0x036659` / file offset `0x2e3254`), every placed grid child stores concrete `gridRowAnchor` and `gridColumnAnchor` track GUIDs rather than a partial single-axis lock. Consequently, partial column-only lock placement (`row: None, column: Some(c)`) cannot be exercised as a distinct persisted state in Figma's native scene graph; instead, we verified static WASM disassembly of Figma's sparse auto-placement cursor (`func[17885]` `computeIdealPositions` at `0x04514e` and `func[9636]` at `0x03b062`) and verified live headless Chrome rendering against CSS Grid Layout Level 1 §8.5 (`plans/reports/agy-1441-grid-placement-evidence/05-c5-chrome-before.log:1-11`, `plans/reports/agy-1441-grid-placement-evidence/06-c5-chrome-after.log:1-6`).
2. **Full WABT `rea inspect-wasm-artifact` on the unstripped 53 MB `compiled_wasm.wasm`:** `rea inspect-wasm-artifact` enforces an 8 MiB (`8,388,608`-byte) input cap per invocation, so full-binary WABT validation was performed on the 1,730,652-byte validated code+name module slice `grid_slice_valid.wasm` (`ev_cd538bfc4e23bbca10b21efde01a2a94f3cb826a681669a456c6c715d1b51a45`) while offsets in the 53,241,904-byte `compiled_wasm.wasm` were verified directly via `wasm-objdump -d` and Data-section string offsets.

---

## 1. Step 1 — Re-Anchor & Figma RE Facts (Recorded Before Editing Code)

### 1.1 Re-Anchor (`crates/designos-doc/src/layout_solver_grid_placement.rs` on Base `4683fe99`)

1. **`Want::of(k: &ChildIn, cols: usize) -> Self`** (`crates/designos-doc/src/layout_solver_grid_placement.rs:38-44` on base `4683fe99`):
   - Clamps `col_span` to `(k.cell.col_span.max(1) as usize).min(cols)` (`crates/designos-doc/src/layout_solver_grid_placement.rs:39`) and `row_span` to `(k.cell.row_span.max(1) as usize).min(MAX_GRID_TRACKS)` (`crates/designos-doc/src/layout_solver_grid_placement.rs:40`).
   - Resolves `column` via `k.cell.column.map(usize::from).filter(|c| *c < cols).map(|c| c.min(cols - col_span))` (`crates/designos-doc/src/layout_solver_grid_placement.rs:42`), dropping out-of-range columns (`*c >= cols`) and pulling back wide column spans so `column + col_span <= cols`.
   - Resolves `row` via `k.cell.row.map(usize::from)` (`crates/designos-doc/src/layout_solver_grid_placement.rs:43`).
2. **`first_free_cell`** (`crates/designos-doc/src/layout_solver_grid_placement.rs:55-70` on base `4683fe99`, updated at `crates/designos-doc/src/layout_solver_grid_placement.rs:55-79` on HEAD `a256d679`):
   - On base `4683fe99`, scanned rows `r` starting at `start_row` (`:56`), calling `ensure_rows(occupied, r + row_span)` (`:58`), and tested `c..=c` when `column == Some(c)` (`:60`) or `0..=(cols - col_span)` when `column == None` (`:61`), with no `start_col` cursor parameter.
3. **`place_children(kids: &[ChildIn], cols: usize) -> Vec<Placed>`** (`crates/designos-doc/src/layout_solver_grid_placement.rs:73-105` on base `4683fe99`, updated at `crates/designos-doc/src/layout_solver_grid_placement.rs:82-128` on HEAD `a256d679`):
   - On base `4683fe99`:
     - **Pass 1** (`:87-91`): placed every child with `(Some(row), Some(col))` directly at `(row, col)`.
     - **Pass 2 & Pass 3** (`:93-103`): iterated `for locked_pass in [true, false]` where `let locked = w.row.is_some() || w.column.is_some()` (`:96`). Because `locked` treated `w.column.is_some()` identically to `w.row.is_some()`, Pass 2 (`locked_pass == true`) placed **both** row-locked `(Some(row), None)` and column-locked `(None, Some(col))` children before any auto `(None, None)` child, starting column-locked search at `start_row = 0` (`w.row.unwrap_or(0)` at `:100`), and Pass 3 (`locked_pass == false`) placed auto children `(None, None)` from `(0, 0)` (dense packing into earlier holes).
4. **All Workspace Callers of `place_children`**:
   - Re-exported at `crates/designos-doc/src/layout_solver.rs:17` (`pub use crate::layout_solver_grid_placement::{place_children, Placed};`).
   - `crates/designos-doc/src/layout_solver_grid.rs`:
     - `placed_grid_rows` (`crates/designos-doc/src/layout_solver_grid.rs:106-108`, calls `place_children` at `:107`).
     - `grid_column_widths` (`crates/designos-doc/src/layout_solver_grid.rs:111-116`, calls `place_children` at `:114`).
     - `grid_row_heights` (`crates/designos-doc/src/layout_solver_grid.rs:119-125`, calls `place_children` at `:122`).
     - `compute_grid` (`crates/designos-doc/src/layout_solver_grid.rs:259-311`, calls `place_children` at `:272`).
     - Unit test `row_heights_match_compute_grid_across_fixed_hug_flex_and_spans` (`crates/designos-doc/src/layout_solver_grid.rs:729`).
   - `crates/designos-export/src/codegen.rs`:
     - `solved_grid` (`crates/designos-export/src/codegen.rs:674-698`, calls `place_children` at `:695`), feeding native exporters (`crates/designos-export/src/compose.rs:328`, `crates/designos-export/src/swiftui.rs:427`, `crates/designos-export/src/android_xml.rs:353`, `crates/designos-export/src/uikit.rs:177`).
     - `resolved_grid_col_tracks` (`crates/designos-export/src/codegen.rs:738-758`, via `compute_grid` at `:748` and `grid_column_widths` at `:752`), feeding `crates/designos-export/src/css.rs:126` and `crates/designos-export/src/tailwind.rs:348`.
     - `grid_packs_auto_rows` (`crates/designos-export/src/codegen.rs:581-587`, via `designos_doc::layout_solver::grid_packs_auto_rows` -> `placed_grid_rows`), feeding `crates/designos-export/src/css.rs:137` and `crates/designos-export/src/tailwind.rs:355`.
     - Unit test `solved_grid_matches_solver_accessors_and_place_children` (`crates/designos-export/src/codegen.rs:1478, 1511`).
     - Integration test `crates/designos-export/tests/grid_native_fixtures.rs:221, 265, 977`.
   - Web exporters (`crates/designos-export/src/codegen.rs:870-885`, `crates/designos-export/src/css.rs:55-61`, `crates/designos-export/src/tailwind.rs:410-422`): emit `grid-column: <c+1> / span <col_span>` and leave `grid-row` auto when `row: None, column: Some(c)`, so browsers place those children via **CSS Grid Layout Level 1 §8.5 (Grid Item Placement Algorithm)** under default `grid-auto-flow: row` (sparse).
   - `crates/designos-mcp`: does not call `place_children` directly; invokes `Document::relayout_subtree` (`crates/designos-doc/src/layout.rs`), which calls `layout_solver::compute` -> `compute_grid` -> `place_children`.

### 1.2 Figma RE Facts (`rea` + Figma Desktop Bundle & WASM Disassembly)

1. **REA Artifact Inspection**:
   - `rea inspect-macho "/Applications/Figma.app/Contents/MacOS/Figma" --json`:
     - REA `evidence_id`: `ev_0cd5f3f9cd98cd6b00b4abca25c0efd5ef3b4108880aca82eba125aa3cc16c4a`
     - Binary SHA-256: `7dfab8de8442f59fd8fbdd85cfdf4774746da6160257920e07d6d5ab4833039b` (`arm64` Mach-O executable, Electron `39.2.7`).
   - `rea inspect-wasm-artifact` on the validated code+name slice (`grid_slice_valid.wasm`, SHA-256 `5caaf51c1564e617d4c79b843f4bdea7d1668b78792706d170bffc2056208acf`) of Figma's 53,241,904-byte `compiled_wasm.wasm.br`:
     - REA `evidence_id`: `ev_cd538bfc4e23bbca10b21efde01a2a94f3cb826a681669a456c6c715d1b51a45` (`52,638` functions, `1,779` imports, `5,074` exports).
2. **Figma Kiwi Schema & Dev Mode CSS Export (`27756-1dc8c27de1ebafa4.min.js`, 13,805,154 bytes)**:
   - **Kiwi Schema** (`27756-1dc8c27de1ebafa4.min.js` byte offsets `13493033` and `13493330`):
     - Container fields: `GUIDPositionMap gridRows=435; GUIDPositionMap gridColumns=436; ... bool gridReflowEnabled=556;` (exposed in Plugin API as `gridItemsPositioning: "MANUAL" | "ROW_AUTO_FLOW"` at byte offsets `3673594` and `4165596`).
     - Child fields: `GUID gridRowAnchor=439; GUID gridColumnAnchor=440; uint gridRowSpan=441; uint gridColumnSpan=442;` (resolved to 0-based `gridRowAnchorIndex` and `gridColumnAnchorIndex` at byte offset `10780737`).
   - **Dev Mode CSS Export** (`27756-1dc8c27de1ebafa4.min.js` byte offsets `2653742`, `2669001`, `2670598`):
     - Emits `grid-auto-flow: row` when `gridItemsPositioning === "ROW_AUTO_FLOW"` (`offset 2669001`).
     - Reads `gridRowAnchorIndex` (`a`), `gridColumnAnchorIndex` (`r`), `gridRowSpan` (`t`), `gridColumnSpan` (`n`): under `"ROW_AUTO_FLOW"` emits `grid-row: span ${t}` / `grid-column: span ${n}`; under `"MANUAL"` emits `grid-row: ${a+1} / span ${t}; grid-column: ${r+1} / span ${n}`.
3. **Figma C++/WASM Grid Placement Engine (`compiled_wasm.wasm`, `fullscreen/lib/scenegraph/FGGridLayout.cpp` `@0xebca3` & `fullscreen/lib/scenegraph/FGGridCache.cpp` `@0xf1d5e`)**:
   - `func[19807]` (`GridCache`, `FGGridCache.cpp`, WASM code offset `0x04a8c3`, file offset `0x116e4be`, size `3,227` bytes) and `func[5480]` (`updateGridChildPositions`, `FGGridLayout.cpp`, WASM code offset `0x036659`, file offset `0x2e3254`, size `12,172` bytes): in Figma's native scene graph, every placed child holds concrete `gridRowAnchor` and `gridColumnAnchor` track GUIDs (validated by `hasInvalidGridAnchors` `@0xce920`).
   - `func[17885]` (`computeIdealPositions`, `FGGridLayout.cpp:2721`, WASM code offset `0x04514e`, file offset `0xf44eab`, size `2,366` bytes) + `func[9636]` (first-free-cell search, WASM code offset `0x03b062`, file offset `0x63fd83`, size `1,074` bytes) + `func[31394]` (`increaseGridCapacityForChild`, WASM code offset `0x065c87`, file offset `0x24317fa`, size `780` bytes):
     - `computeIdealPositions` maintains a monotonic **sparse** auto-placement cursor `(local 7 = cursor_row, local 8 = cursor_col)` initialized to `(0, 0)`.
     - At `0x0453df..0x045439`, it passes `startCoord = (cursor_row, cursor_col)` at `fp+40` to `func[9636]`.
     - Inside `func[9636]`, on the starting row `r = start_row` (`local 7`), the column loop (`0x03b1cc..0x03b254`) starts at `c = start_col` (`local 5`) and scans `start_col ..= numCols - colSpan`; on subsequent rows `r = start_row + 1 ..= numRows - rowSpan` (`0x03b26e..0x03b464`), it resets `c = 0` (`0x03b2b1`) and scans `0 ..= numCols - colSpan`, checking overlap against occupied cells via `func[8800]` (`0x039b69`, file offset `0x557511`).
     - After placing a child at `(row, col)` (packed `i64` in `local 17`), `computeIdealPositions` updates `cursor_row = i32.wrap_i64(local 17)` (`0x045676`) and `cursor_col = i32.wrap_i64(local 17 >> 32)` (`0x0458d3`), then marks `row..row+rowSpan` × `col..col+colSpan` occupied in the excluded set (`0x045910..0x0459e8`, `call 3017`).

---

## 2. Conclusion

- Fixed `place_children` and `first_free_cell` in [`crates/designos-doc/src/layout_solver_grid_placement.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1441-grid-placement/crates/designos-doc/src/layout_solver_grid_placement.rs#L55-L128) so that:
  1. **Pass 1** (`crates/designos-doc/src/layout_solver_grid_placement.rs:95-99`) places children with both `row` and `column` set (`(Some(row), Some(col))`) directly at `(row, col)`.
  2. **Pass 2** (`crates/designos-doc/src/layout_solver_grid_placement.rs:101-107`) places only row-locked children (`(Some(row), None)`) in child order starting at `row`.
  3. **Pass 3** (`crates/designos-doc/src/layout_solver_grid_placement.rs:108-127`) processes remaining children (`w.row.is_none()` — both column-locked `(None, Some(col))` and auto `(None, None)`) together in child order using a single sparse cursor `(cursor_row, cursor_col)` initialized to `(0, 0)`.
- Updated the placement-specific parity test expectation in [`crates/designos-app/src/layout/solver_parity_tests.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1441-grid-placement/crates/designos-app/src/layout/solver_parity_tests.rs#L565-L578) (`grid_locked_children_place_after_pinned_ones_and_before_auto_ones`).
- Opened draft PR [`#1560`](https://github.com/jangtrinh/design-os-studio/pull/1560) (`jangtrinh/agy-1441-grid-placement` -> `main`, commit `a256d679`).

---

## 3. Acceptance Table & Pasted Evidence

| # | Gate / Requirement | Status | Evidence File & Line Citations |
|---|---|---|---|
| 1 | Tests `(a)`, `(b)`, `(c)`, `(d)` red on base `4683fe99`; null control green on base | **PASS** | `plans/reports/agy-1441-grid-placement-evidence/01-red-base.log:26-84`, `crates/designos-doc/src/layout_solver_grid_placement.rs:153-185` |
| 1b | Tests `(a)`, `(b)`, `(c)`, `(d)` + null control green on HEAD `a256d679` | **PASS** | `plans/reports/agy-1441-grid-placement-evidence/02-green.log:12-26` |
| 2a | Kill Mutant 1 (column-locked children placed in pass 2 again) | **PASS** | `plans/reports/agy-1441-grid-placement-evidence/03-mutant1-col-in-pass2.log:12-69` |
| 2b | Kill Mutant 2 (cursor resets to row 0 for every child, dense) | **PASS** | `plans/reports/agy-1441-grid-placement-evidence/04-mutant2-dense-cursor.log:12-61` |
| 3 | Solver-vs-browser Chrome probe on scene C5 (`96.00px` before -> `0.00px` after) | **PASS** | `plans/reports/agy-1441-grid-placement-evidence/05-c5-chrome-before.log:1-11`, `plans/reports/agy-1441-grid-placement-evidence/06-c5-chrome-after.log:1-6` |
| 4 | `cargo fmt --all -- --check` (`fmt_rc=0`) | **PASS** | `plans/reports/agy-1441-grid-placement-evidence/07-fmt.log:1` |
| 5 | `cargo clippy --workspace --all-targets --locked` (`clippy_rc=0`, 0 new warnings) | **PASS** | `plans/reports/agy-1441-grid-placement-evidence/08-clippy.log:412-413` |
| 6a | `cargo test -p designos-doc --locked` (`727 + 3 + 5 + 2 passed; 0 failed`) | **PASS** | `plans/reports/agy-1441-grid-placement-evidence/09-test-doc.log:735-773` |
| 6b | `cargo test -p designos-export --locked` (`0 failed`, `export_rc=0`) | **PASS** | `plans/reports/agy-1441-grid-placement-evidence/10-test-export.log:168-377` |
| 6c | `cargo test -p designos-mcp --locked` (`0 failed`, `mcp_rc=0`) | **PASS** | `plans/reports/agy-1441-grid-placement-evidence/11-test-mcp.log:19-400` |
| 6d | `cargo test --workspace --locked --lib` (`0 failed`, `ws_lib_rc=0`) | **PASS** | `plans/reports/agy-1441-grid-placement-evidence/12-test-workspace-lib.log:741-1507` |
| 7 | `python3 -B scripts/zero_fake_ui_gate.py` (`17/17 PASS`, `zfu_rc=0`) | **PASS** | `plans/reports/agy-1441-grid-placement-evidence/13-zero-fake-ui.log:1-25` |
| 8 | Draft PR opened (`#1560`) and `out/cargo` deleted (`ls out/cargo` fails) | **PASS** | `plans/reports/agy-1441-grid-placement-evidence/14-cleanup-out-cargo.log:1-2` |

### 3.1 Red Run on Base `4683fe99` (`plans/reports/agy-1441-grid-placement-evidence/01-red-base.log:26-84`)

```text
running 8 tests
test layout_solver_grid_placement::tests::a_missing_column_keeps_the_row_lock_and_a_wide_span_is_pulled_back_in ... ok
test layout_solver_grid_placement::tests::explicit_cells_may_overlap_each_other_as_in_css ... ok
test layout_solver_grid_placement::tests::auto_children_never_take_a_cell_a_later_child_pinned ... ok
test layout_solver_grid_placement::tests::null_control_all_auto_and_all_pinned_grids_match_base ... ok
test layout_solver_grid_placement::tests::c_column_locked_child_after_cursor_passed_its_column_goes_to_next_row_sparse ... FAILED
test layout_solver_grid_placement::tests::b_row_locked_child_is_still_placed_before_auto_children ... FAILED
test layout_solver_grid_placement::tests::d_column_locked_child_with_column_span_places_in_child_order ... FAILED
test layout_solver_grid_placement::tests::a_auto_children_before_and_after_column_locked_child_match_css_grid ... FAILED

failures:

---- layout_solver_grid_placement::tests::c_column_locked_child_after_cursor_passed_its_column_goes_to_next_row_sparse stdout ----
assertion `left == right` failed
  left: [(0, 2), (0, 0), (0, 1)]
 right: [(0, 2), (1, 0), (1, 1)]

---- layout_solver_grid_placement::tests::b_row_locked_child_is_still_placed_before_auto_children stdout ----
assertion `left == right` failed
  left: [(1, 1), (1, 0), (0, 1), (0, 0)]
 right: [(1, 0), (2, 0), (0, 1), (0, 0)]

---- layout_solver_grid_placement::tests::d_column_locked_child_with_column_span_places_in_child_order stdout ----
assertion `left == right` failed
  left: [(0, 0), (1, 0), (0, 1), (1, 1)]
 right: [(0, 0), (0, 1), (1, 1), (2, 0)]

---- layout_solver_grid_placement::tests::a_auto_children_before_and_after_column_locked_child_match_css_grid stdout ----
assertion `left == right` failed
  left: [(0, 1), (0, 0), (1, 0)]
 right: [(0, 0), (1, 0), (1, 1)]

test result: FAILED. 4 passed; 4 failed; 0 ignored; 0 measured; 719 filtered out; finished in 0.00s
rc=101
```

### 3.2 Green Run on HEAD `a256d679` (`plans/reports/agy-1441-grid-placement-evidence/02-green.log:12-26`)

```text
running 8 tests
test layout_solver_grid_placement::tests::explicit_cells_may_overlap_each_other_as_in_css ... ok
test layout_solver_grid_placement::tests::c_column_locked_child_after_cursor_passed_its_column_goes_to_next_row_sparse ... ok
test layout_solver_grid_placement::tests::a_missing_column_keeps_the_row_lock_and_a_wide_span_is_pulled_back_in ... ok
test layout_solver_grid_placement::tests::null_control_all_auto_and_all_pinned_grids_match_base ... ok
test layout_solver_grid_placement::tests::b_row_locked_child_is_still_placed_before_auto_children ... ok
test layout_solver_grid_placement::tests::d_column_locked_child_with_column_span_places_in_child_order ... ok
test layout_solver_grid_placement::tests::auto_children_never_take_a_cell_a_later_child_pinned ... ok
test layout_solver_grid_placement::tests::a_auto_children_before_and_after_column_locked_child_match_css_grid ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 719 filtered out; finished in 0.00s
rc=0
```

### 3.3 Mutant 1 Killed (`plans/reports/agy-1441-grid-placement-evidence/03-mutant1-col-in-pass2.log:12-69`)

```text
failures:
    layout_solver_grid_placement::tests::a_auto_children_before_and_after_column_locked_child_match_css_grid
    layout_solver_grid_placement::tests::b_row_locked_child_is_still_placed_before_auto_children
    layout_solver_grid_placement::tests::c_column_locked_child_after_cursor_passed_its_column_goes_to_next_row_sparse
    layout_solver_grid_placement::tests::d_column_locked_child_with_column_span_places_in_child_order

test result: FAILED. 4 passed; 4 failed; 0 ignored; 0 measured; 719 filtered out; finished in 0.01s
rc=101
```

### 3.4 Mutant 2 Killed (`plans/reports/agy-1441-grid-placement-evidence/04-mutant2-dense-cursor.log:12-61`)

```text
---- layout_solver_grid_placement::tests::d_column_locked_child_with_column_span_places_in_child_order stdout ----
assertion `left == right` failed
  left: [(0, 0), (0, 1), (1, 1), (0, 2)]
 right: [(0, 0), (0, 1), (1, 1), (2, 0)]

---- layout_solver_grid_placement::tests::a_auto_children_before_and_after_column_locked_child_match_css_grid stdout ----
assertion `left == right` failed
  left: [(0, 0), (1, 0), (0, 1)]
 right: [(0, 0), (1, 0), (1, 1)]

---- layout_solver_grid_placement::tests::c_column_locked_child_after_cursor_passed_its_column_goes_to_next_row_sparse stdout ----
assertion `left == right` failed
  left: [(0, 2), (0, 0), (0, 1)]
 right: [(0, 2), (1, 0), (1, 1)]

test result: FAILED. 5 passed; 3 failed; 0 ignored; 0 measured; 719 filtered out; finished in 0.00s
rc=101
```

### 3.5 Scene C5 Chrome Probe — Before vs After (`plans/reports/agy-1441-grid-placement-evidence/05-c5-chrome-before.log:1-11`, `06-c5-chrome-after.log:1-6`)

**Before (`4683fe99`):**
```text
[FAIL] C5_hug_span_across_hug_cols    (css     ) max_item_err=96.00px grid_err=(0.00px, 0.00px)
       item[1] solver=(0.0,38.0,30.0,30.0) chrome=(68.0,0.0,30.0,30.0) max_abs=68.00px
       item[2] solver=(68.0,38.0,20.0,30.0) chrome=(96.0,0.0,20.0,30.0) max_abs=38.00px
       item[3] solver=(96.0,38.0,40.0,30.0) chrome=(0.0,38.0,40.0,30.0) max_abs=96.00px
       item[4] solver=(68.0,0.0,150.0,30.0) chrome=(68.0,38.0,150.0,30.0) max_abs=38.00px
[FAIL] C5_hug_span_across_hug_cols    (tailwind) max_item_err=96.00px grid_err=(0.00px, 0.00px)
       item[1] solver=(0.0,38.0,30.0,30.0) chrome=(68.0,0.0,30.0,30.0) max_abs=68.00px
       item[2] solver=(68.0,38.0,20.0,30.0) chrome=(96.0,0.0,20.0,30.0) max_abs=38.00px
       item[3] solver=(96.0,38.0,40.0,30.0) chrome=(0.0,38.0,40.0,30.0) max_abs=96.00px
       item[4] solver=(68.0,0.0,150.0,30.0) chrome=(68.0,38.0,150.0,30.0) max_abs=38.00px
WORST=96.00px
```

**After (`a256d679`):**
```text
[PASS] C5_hug_span_across_hug_cols    (css     ) max_item_err=0.00px grid_err=(0.00px, 0.00px)
[FAIL] C5_hug_span_across_hug_cols    (tailwind) max_item_err=infpx grid_err=(1226.00px, 68.00px)
       item count mismatch: solver=5 vs chrome=0 (diff=5)
[PASS] C5_hug_span_across_hug_cols_boxf (css     ) max_item_err=0.00px grid_err=(0.00px, 0.00px)
[PASS] C5_hug_span_across_hug_cols_boxf (tailwind) max_item_err=0.00px grid_err=(0.00px, 0.00px)
```
*(Note: Once `s` moves from `(0, 1)` to `(1, 1)`, the hugged grid width shrinks from `136px` (`60 + 8 + 20 + 8 + 40`) to `126px` (`60 + 8 + 30 + 8 + 20`). Because `126.0 <= 128.0` and `68.0 <= 128.0`, `is_vector_asset` at `crates/designos-export/src/codegen.rs:249-265` classifies a `Frame` with all-`Rectangle` children as an SVG icon (`<img src="/assets/frame.svg">`) in `tailwind_for`. Constructing the children with `boxf` (`Node::frame`, as `review_probe_1426_r3.rs:429` does for `N2`/`N2b`) bypasses `is_vector_asset` and confirms `max_item_err=0.00px` in both `css` and `tailwind` modes.)*

### 3.6 Gates & `out/cargo` Cleanup (`plans/reports/agy-1441-grid-placement-evidence/07-fmt.log` .. `14-cleanup-out-cargo.log`)

```text
# 07-fmt.log:1
fmt_rc=0

# 08-clippy.log:412-413
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 08s
clippy_rc=0

# 09-test-doc.log:735-773
test result: ok. 727 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.92s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
doc_rc=0

# 10-test-export.log:377
export_rc=0

# 11-test-mcp.log:400
mcp_rc=0

# 12-test-workspace-lib.log:1507
ws_lib_rc=0

# 13-zero-fake-ui.log:23-25
 Gate Verdict: PASS (17/17 invariants satisfied)
============================================================================================================
zfu_rc=0

# 14-cleanup-out-cargo.log:1-2
ls: out/cargo: No such file or directory
ls_rc=1
```

---

## 4. Epistemic Ledger (`FACT` / `INFERENCE` / `ASSUMPTION`)

- **`FACT`:** On base `4683fe99`, `place_children` (`crates/designos-doc/src/layout_solver_grid_placement.rs:93-103`) defined `let locked = w.row.is_some() || w.column.is_some()` (`:96`), placing column-locked children (`w.row.is_none() && w.column.is_some()`) in Pass 2 before all auto children (`w.row.is_none() && w.column.is_none()`), and starting every Pass 2/3 scan at `start_row = w.row.unwrap_or(0)` with no column cursor (`:100`).
- **`FACT`:** In Figma's WASM engine (`compiled_wasm.wasm`, `func[17885]` `computeIdealPositions` at `0x04514e` / file offset `0xf44eab` and `func[9636]` at `0x03b062` / file offset `0x63fd83`), auto-placement maintains a monotonic sparse cursor `(local 7 = cursor_row, local 8 = cursor_col)` initialized to `(0, 0)`, scanning `c = start_col ..= numCols - colSpan` on `r = start_row` (`0x03b1cc..0x03b254`) and `c = 0 ..= numCols - colSpan` on `r > start_row` (`0x03b26e..0x03b464`), then updating `(cursor_row, cursor_col)` to the placed `(row, col)` (`0x045676`, `0x0458d3`).
- **`FACT`:** In CSS Grid Layout Level 1 §8.5 (used by Studio's CSS and Tailwind exporters at `crates/designos-export/src/codegen.rs:870-885`, `crates/designos-export/src/css.rs:55-61`, `crates/designos-export/src/tailwind.rs:410-422`), Step 2 places only row-locked items (`w.row.is_some() && w.column.is_none()`), and Step 4 places column-locked items (`w.row.is_none() && w.column.is_some()`) and auto items (`w.row.is_none() && w.column.is_none()`) together in child order with a sparse `(cursor_row, cursor_col)` cursor.
- **`FACT`:** After moving column-locked items to Pass 3 with the sparse cursor (`crates/designos-doc/src/layout_solver_grid_placement.rs:108-127`), scene `C5_hug_span_across_hug_cols` drops from `max_item_err=96.00px` (`plans/reports/agy-1441-grid-placement-evidence/05-c5-chrome-before.log:1`) to `max_item_err=0.00px` (`plans/reports/agy-1441-grid-placement-evidence/06-c5-chrome-after.log:1,4-5`).
- **`INFERENCE`:** Because `occupied[row..row + w.row_span][col..col + w.col_span]` is filled with `true` immediately upon placing a child at `(row, col)` (`crates/designos-doc/src/layout_solver_grid_placement.rs:89-93`), setting `(cursor_row, cursor_col) = (row, col)` causes the next auto child's scan starting at `(cursor_row, cursor_col)` to automatically skip `col..col + w.col_span` andcauses the next column-locked child at `col2` to advance `cursor_row += 1` when `col2 < col` or see `(cursor_row, col2)` occupied when `col <= col2 < col + w.col_span`, matching CSS Grid §8.5 Steps 4.1 and 4.2.
- **`ASSUMPTION`:** None remaining in the placement algorithm; row-locked overflow behavior when a locked `row` is already full (`first_free_cell(&mut occupied, row, 0, None, cols, w.col_span, w.row_span)` advancing to `r >= row`) is preserved unchanged per Ruling 1 and Ruling 3 (`crates/designos-doc/src/layout_solver_grid_placement.rs:103, 188-197`).

---

## 5. Skills Loaded & What Changed Because of Them

1. **`es:rust` (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`):** Kept `first_free_cell` and `place_children` allocation-free beyond the existing `wants`, `occupied`, and `placed` vectors (`crates/designos-doc/src/layout_solver_grid_placement.rs:55-128`), with zero clones or extra passes.
2. **`es:rust-gpu-canvas` (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`):** Verified that `place_children` remains deterministic and pure so layout solver callers (`compute_grid`, `grid_column_widths`, `grid_row_heights`, `placed_grid_rows` in `crates/designos-doc/src/layout_solver_grid.rs:106-125, 272`) do not perturb scene hashing or GPU frame caching.
3. **`es:astra-work` (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`) & `es:fable-thinking` (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`):** Recorded predictions before running tests, ran the 4 acceptance tests red on base `4683fe99` before editing `place_children` (`01-red-base.log`), killed both required mutants (`03-mutant1-col-in-pass2.log`, `04-mutant2-dense-cursor.log`), and ran `cargo test -p designos-app --bin designos --locked solver_parity_tests`, catching and updating the placement expectation in `crates/designos-app/src/layout/solver_parity_tests.rs:574-578` that `cargo test --workspace --locked --lib` skips because `designos-app` is a binary-only package.
4. **`es:dev` (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`) & `es:feature-dev` (`/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`):** Scoped the change strictly to `crates/designos-doc/src/layout_solver_grid_placement.rs` and the placement parity test in `crates/designos-app/src/layout/solver_parity_tests.rs`, avoiding all files owned by `#1543`, `#1546`, and `#1559`.
5. **`reverse-engineer-anything` (`/Users/jangtrinh/.gemini/config/skills/reverse-engineer-anything/SKILL.md`):** Used `rea inspect-macho` (`ev_0cd5f3f9cd98cd6b00b4abca25c0efd5ef3b4108880aca82eba125aa3cc16c4a`) and `rea inspect-wasm-artifact` (`ev_cd538bfc4e23bbca10b21efde01a2a94f3cb826a681669a456c6c715d1b51a45`) plus `wasm-objdump -d` on Figma's `compiled_wasm.wasm` to trace `func[17885]` (`computeIdealPositions`) and `func[9636]` before writing code.

---

## 6. Weakest Link

- In `C5_hug_span_across_hug_cols` (`crates/designos-doc/src/layout_solver_grid.rs:19-34, 177-191`), `widest_in_column` only considers `p.col_span == 1` children when sizing `GridTrack::Hug` columns (`crates/designos-doc/src/layout_solver_grid.rs:21`). In scene C5, `s` (`width = 150px`, `col_span = 2`, `grid_column = Some(1)`) spans across two `GridTrack::Hug` columns (`col 1` with `30px` and `col 2` with `20px`), and neither the solver nor the CSS exporter (which emits `grid-template-columns: 60px 30px 20px` via `resolved_grid_col_tracks` at `crates/designos-export/src/codegen.rs:738-758`) distributes a multi-column span's width across `Hug` columns. Solver and Chrome agree (`0.00px` error) because `css.rs:126` and `tailwind.rs:348` emit the solver's resolved column widths as fixed `px` tracks (`60px 30px 20px`), so `s` overflows its `58px` cell in both.

---

## 7. Unresolved Questions

1. Should `is_vector_asset` in `crates/designos-export/src/codegen.rs:249-265` (owned by `#1546`) exclude `NodeKind::Frame` containers that have `auto_layout.is_some()` (such as small `<= 128x128` grid or flex frames whose children happen to be `NodeKind::Rectangle` placeholders) so `tailwind_for` and `react_for` do not collapse them into `<img src="/assets/frame.svg" />`?

Status: DONE
