# Report: Issue #484 (CSS / Tailwind Half) — Exported Grids Land Where the Solver Puts Them, Checked in Chrome

## What Could NOT Be Verified and Why

- **Out-of-scope mobile/native exporters (`swiftui.rs`, `compose.rs`, `uikit.rs`, `android_xml.rs`)**: Per the task brief's scope boundaries, this task covers the CSS and Tailwind half of Issue #484 only and explicitly forbids modifying `swiftui.rs`, `compose.rs`, `uikit.rs`, `android_xml.rs`, or the layout solver in `designos-doc`. Those native exporters were not modified; their existing unit tests in `cargo test -p designos-export --locked` continue to pass.
- All required CSS and Tailwind grid export checks, headless Chrome layout measurements across all 6 fixture scenes, the `--screenshot` render comparison, and all workspace gates (`cargo fmt`, `cargo clippy`, `cargo test --workspace --locked --lib`, `cargo test -p designos-export --locked`, `python3 -m unittest discover -s scripts/tests`) were verified directly on this machine.

---

## Root Cause & Fix Summary

1. **Hugging grids with `GridTrack::Flex` columns (`unequal_tracks`, `hugging_grid`)**:
   - **Root cause**: `designos_doc::layout::solve_grid` sizes `GridTrack::Flex(f)` columns inside a width-hugging grid (`PrimarySizing::Hug`) by taking the maximum natural width of non-spanning children in each column (producing unequal column widths such as `100px`, `40px`, `60px` or `100px`, `40px`, `40px`) and sets the grid frame's `size.w` to the sum of those solved columns plus padding and column gaps. However, `css.rs` and `tailwind.rs` emitted `grid-template-columns: repeat(3, minmax(0, 1fr))` / `grid-cols-3` directly from the raw `al.grid_cols` definition. In CSS Grid, `1fr` tracks inside a fixed-width container divide the available width equally (`66.67px` each in a `200px` grid, or `60px` each in a `180px` content box), causing items in columns 2 and 3 and any `LayoutAlign::Stretch` (`w-full`) children to land up to `40.00px` away from the layout solver's coordinates.
   - **Fix**: Added `resolved_grid_col_tracks(doc, n, al)` in `crates/designos-export/src/codegen.rs`. When `al.primary_sizing == PrimarySizing::Hug` and any column is `GridTrack::Flex`, it computes the per-column natural widths using the exact same single-column child measurement rule as `designos_doc::layout::solve_grid`. If the solved `Flex` columns do not all resolve to the exact same width per `fr` unit, `resolved_grid_col_tracks` returns `GridTrack::Fixed(px)` for each column so `css.rs` emits `grid-template-columns: 100px 40px 60px;` and `tailwind.rs` emits `grid-cols-[100px_40px_60px]`. When all `Flex` columns resolve to equal widths, `repeat(N, minmax(0, 1fr))` / `grid-cols-N` is preserved.

2. **Fixed-height grids with auto-sized rows (`equal_tracks`, `unequal_tracks`, `fixed_first_column`, `fixed_height_auto_rows`, `row_and_column_spans`)**:
   - **Root cause**: `designos_doc::layout::solve_grid` packs auto-sized rows (`GridTrack::Auto`, or implicit rows when `grid_rows` is empty) at the top of the grid starting at `padding.top`, advancing each row by `row_height + row_gap` regardless of whether `al.counter_sizing == PrimarySizing::Fixed` leaves extra vertical space at the bottom of the container. In CSS Grid, a container with explicit `height: <px>` and auto-sized row tracks defaults to `align-content: normal` (`stretch`), which distributes free vertical space across the auto row tracks and shifts row 2+ downward by `18.66px`–`58.66px` in Chrome.
   - **Fix**: Added `grid_packs_auto_rows(al)` in `crates/designos-export/src/codegen.rs` (true when `al.mode == LayoutMode::Grid`, `al.counter_sizing == PrimarySizing::Fixed`, and no explicit `GridTrack::Flex` row absorbs the leftover height). When true, `css.rs` emits `align-content: start;` and `tailwind.rs` emits `content-start`.

---

## Acceptance Table

| Check | Command | Result | Evidence |
|---|---|---|---|
| Pre-fix unit tests fail (RED) | `cargo test -p designos-export --test grid_chrome_fixtures --locked` (before exporter fix) | **FAIL** (`0 passed; 6 failed`, exit code `101`) | `plans/reports/agy-484-grid-tracks-css-evidence/red.log` |
| Pre-fix Chrome comparison fails (RED) | `python3 scripts/qa/grid-chrome-compare.py` (before exporter fix) | **FAIL** (all 6 scenes fail in Chrome for both CSS and Tailwind; max diff up to `58.66px`, exit code `1`) | `plans/reports/agy-484-grid-tracks-css-evidence/chrome-red.log` |
| Post-fix unit tests pass (GREEN) | `cargo test -p designos-export --test grid_chrome_fixtures --locked` | **PASS** (`6 passed; 0 failed`, exit code `0`) | `plans/reports/agy-484-grid-tracks-css-evidence/green.log` |
| Post-fix Chrome comparison + screenshot pass (GREEN) | `python3 scripts/qa/grid-chrome-compare.py --screenshot plans/reports/agy-484-grid-tracks-css-evidence/unequal-tracks-chrome-vs-solver.png` | **PASS** (all 6 scenes × CSS & Tailwind = 12 checks pass with `0.00px` max diff, exit code `0`) | `plans/reports/agy-484-grid-tracks-css-evidence/chrome-green.log`, `plans/reports/agy-484-grid-tracks-css-evidence/unequal-tracks-chrome-vs-solver.png` |
| Missing Chrome binary exits `2` | `python3 scripts/qa/grid-chrome-compare.py --chrome /nonexistent/chrome` | **PASS** (exits `2` with clear error message) | Verified via CLI invocation |
| `cargo fmt --all -- --check` | `cargo fmt --all -- --check` | **PASS** (exit code `0`) | Clean stdout |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | `cargo clippy --workspace --all-targets --locked -- -D warnings` | **PASS** (exit code `0`, 0 warnings) | Step 224 log |
| `cargo test --workspace --locked --lib` | `cargo test --workspace --locked --lib` | **PASS** (all workspace lib test suites pass, exit code `0`) | Step 224 log |
| `cargo test -p designos-export --locked` | `cargo test -p designos-export --locked` | **PASS** (`154` unit + `6` `grid_chrome_fixtures` + `49` `pdf_export` + `4` `pixmap_size_cap` + `28` `slot_codegen` + `3` `variable_code_syntax_per_platform` passed, `0` failed) | Step 218 log |
| `python3 -m unittest discover -s scripts/tests` | `python3 -m unittest discover -s scripts/tests` | **PASS** (`Ran 199 tests in 187.064s — OK`) | Step 218 log |

---

## Verbatim Evidence Outputs

### 1. Pre-Fix Unit Tests (`plans/reports/agy-484-grid-tracks-css-evidence/red.log`)

```text
running 6 tests
test equal_tracks_css_and_tailwind ... FAILED
test hugging_grid_css_and_tailwind ... FAILED
test row_and_column_spans_css_and_tailwind ... FAILED
test fixed_first_column_css_and_tailwind ... FAILED
test fixed_height_auto_rows_css_and_tailwind ... FAILED
test unequal_tracks_css_and_tailwind ... FAILED

failures:

---- equal_tracks_css_and_tailwind stdout ----

thread 'equal_tracks_css_and_tailwind' (34842019) panicked at crates/designos-export/tests/grid_chrome_fixtures.rs:22:5:
assertion `left == right` failed: equal_tracks CSS mismatch
  left: "display: grid;\nwidth: 324px;\nheight: 180px;\npadding: 12px;\ngrid-template-columns: repeat(3, minmax(0, 1fr));\ngap: 12px;"
 right: "display: grid;\nwidth: 324px;\nheight: 180px;\npadding: 12px;\ngrid-template-columns: repeat(3, minmax(0, 1fr));\ngap: 12px;\nalign-content: start;"
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- hugging_grid_css_and_tailwind stdout ----

thread 'hugging_grid_css_and_tailwind' (34842022) panicked at crates/designos-export/tests/grid_chrome_fixtures.rs:22:5:
assertion `left == right` failed: hugging_grid CSS mismatch
  left: "display: grid;\nwidth: 212px;\nheight: 72px;\npadding: 8px;\ngrid-template-columns: repeat(3, minmax(0, 1fr));\ngap: 8px;"
 right: "display: grid;\nwidth: 212px;\nheight: 72px;\npadding: 8px;\ngrid-template-columns: 100px 40px 40px;\ngap: 8px;"

---- row_and_column_spans_css_and_tailwind stdout ----

thread 'row_and_column_spans_css_and_tailwind' (34842023) panicked at crates/designos-export/tests/grid_chrome_fixtures.rs:22:5:
assertion `left == right` failed: row_and_column_spans CSS mismatch
  left: "display: grid;\nwidth: 428px;\nheight: 220px;\npadding: 12px;\ngrid-template-columns: repeat(4, minmax(0, 1fr));\ngap: 12px;"
 right: "display: grid;\nwidth: 428px;\nheight: 220px;\npadding: 12px;\ngrid-template-columns: repeat(4, minmax(0, 1fr));\ngap: 12px;\nalign-content: start;"

---- fixed_first_column_css_and_tailwind stdout ----

thread 'fixed_first_column_css_and_tailwind' (34842020) panicked at crates/designos-export/tests/grid_chrome_fixtures.rs:22:5:
assertion `left == right` failed: fixed_first_column CSS mismatch
  left: "display: grid;\nwidth: 440px;\nheight: 180px;\ngrid-template-columns: 200px repeat(2, minmax(0, 1fr));\ngap: 10px;"
 right: "display: grid;\nwidth: 440px;\nheight: 180px;\ngrid-template-columns: 200px repeat(2, minmax(0, 1fr));\ngap: 10px;\nalign-content: start;"

---- fixed_height_auto_rows_css_and_tailwind stdout ----

thread 'fixed_height_auto_rows_css_and_tailwind' (34842021) panicked at crates/designos-export/tests/grid_chrome_fixtures.rs:22:5:
assertion `left == right` failed: fixed_height_auto_rows CSS mismatch
  left: "display: grid;\nwidth: 260px;\nheight: 240px;\npadding: 16px;\ngrid-template-columns: repeat(2, minmax(0, 1fr));\ngap: 16px 12px;"
 right: "display: grid;\nwidth: 260px;\nheight: 240px;\npadding: 16px;\ngrid-template-columns: repeat(2, minmax(0, 1fr));\ngap: 16px 12px;\nalign-content: start;"

---- unequal_tracks_css_and_tailwind stdout ----

thread 'unequal_tracks_css_and_tailwind' (34842024) panicked at crates/designos-export/tests/grid_chrome_fixtures.rs:22:5:
assertion `left == right` failed: unequal_tracks CSS mismatch
  left: "display: grid;\nwidth: 200px;\nheight: 140px;\ngrid-template-columns: repeat(3, minmax(0, 1fr));\ngap: 10px 0px;"
 right: "display: grid;\nwidth: 200px;\nheight: 140px;\ngrid-template-columns: 100px 40px 60px;\ngap: 10px 0px;\nalign-content: start;"

test result: FAILED. 0 passed; 6 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
```

### 2. Post-Fix Unit Tests (`plans/reports/agy-484-grid-tracks-css-evidence/green.log`)

```text
running 6 tests
test row_and_column_spans_css_and_tailwind ... ok
test hugging_grid_css_and_tailwind ... ok
test fixed_height_auto_rows_css_and_tailwind ... ok
test equal_tracks_css_and_tailwind ... ok
test fixed_first_column_css_and_tailwind ... ok
test unequal_tracks_css_and_tailwind ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### 3. Pre-Fix Chrome Comparison (`plans/reports/agy-484-grid-tracks-css-evidence/chrome-red.log`)

```text
=== Scene: equal_tracks [CSS] ===
item   solver (x, y, w, h)          chrome (x, y, w, h)          delta (dx, dy, dw, dh)       max_diff status
#0     (12.0, 12.0, 92.0, 36.0)     (12.00, 12.00, 92.00, 36.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#1     (116.0, 12.0, 92.0, 36.0)    (116.00, 12.00, 92.00, 36.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#2     (220.0, 12.0, 92.0, 36.0)    (220.00, 12.00, 92.00, 36.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#3     (12.0, 60.0, 92.0, 36.0)     (12.00, 96.00, 92.00, 36.00) (+0.00, +36.00, +0.00, +0.00) 36.00    FAIL
#4     (116.0, 60.0, 92.0, 36.0)    (116.00, 96.00, 92.00, 36.00) (+0.00, +36.00, +0.00, +0.00) 36.00    FAIL
#5     (220.0, 60.0, 92.0, 36.0)    (220.00, 96.00, 92.00, 36.00) (+0.00, +36.00, +0.00, +0.00) 36.00    FAIL
Result [equal_tracks / CSS]: FAIL

=== Scene: equal_tracks [Tailwind] ===
item   solver (x, y, w, h)          chrome (x, y, w, h)          delta (dx, dy, dw, dh)       max_diff status
#0     (12.0, 12.0, 92.0, 36.0)     (12.00, 12.00, 92.00, 36.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#1     (116.0, 12.0, 92.0, 36.0)    (116.00, 12.00, 92.00, 36.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#2     (220.0, 12.0, 92.0, 36.0)    (220.00, 12.00, 92.00, 36.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#3     (12.0, 60.0, 92.0, 36.0)     (12.00, 96.00, 92.00, 36.00) (+0.00, +36.00, +0.00, +0.00) 36.00    FAIL
#4     (116.0, 60.0, 92.0, 36.0)    (116.00, 96.00, 92.00, 36.00) (+0.00, +36.00, +0.00, +0.00) 36.00    FAIL
#5     (220.0, 60.0, 92.0, 36.0)    (220.00, 96.00, 92.00, 36.00) (+0.00, +36.00, +0.00, +0.00) 36.00    FAIL
Result [equal_tracks / Tailwind]: FAIL

=== Scene: unequal_tracks [CSS] ===
item   solver (x, y, w, h)          chrome (x, y, w, h)          delta (dx, dy, dw, dh)       max_diff status
#0     (0.0, 0.0, 100.0, 30.0)      (0.00, 0.00, 100.00, 30.00)  (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#1     (100.0, 0.0, 40.0, 30.0)     (66.66, 0.00, 40.00, 30.00)  (-33.34, +0.00, +0.00, +0.00) 33.34    FAIL
#2     (140.0, 0.0, 60.0, 30.0)     (133.33, 0.00, 60.00, 30.00) (-6.67, +0.00, +0.00, +0.00) 6.67     FAIL
#3     (0.0, 40.0, 100.0, 30.0)     (0.00, 75.00, 66.66, 30.00)  (+0.00, +35.00, -33.34, +0.00) 35.00    FAIL
#4     (100.0, 40.0, 40.0, 30.0)    (66.66, 75.00, 66.67, 30.00) (-33.34, +35.00, +26.67, +0.00) 35.00    FAIL
#5     (140.0, 40.0, 60.0, 30.0)    (133.33, 75.00, 66.66, 30.00) (-6.67, +35.00, +6.66, +0.00) 35.00    FAIL
Result [unequal_tracks / CSS]: FAIL

=== Scene: unequal_tracks [Tailwind] ===
item   solver (x, y, w, h)          chrome (x, y, w, h)          delta (dx, dy, dw, dh)       max_diff status
#0     (0.0, 0.0, 100.0, 30.0)      (0.00, 0.00, 100.00, 30.00)  (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#1     (100.0, 0.0, 40.0, 30.0)     (66.66, 0.00, 40.00, 30.00)  (-33.34, +0.00, +0.00, +0.00) 33.34    FAIL
#2     (140.0, 0.0, 60.0, 30.0)     (133.33, 0.00, 60.00, 30.00) (-6.67, +0.00, +0.00, +0.00) 6.67     FAIL
#3     (0.0, 40.0, 100.0, 30.0)     (0.00, 75.00, 66.66, 30.00)  (+0.00, +35.00, -33.34, +0.00) 35.00    FAIL
#4     (100.0, 40.0, 40.0, 30.0)    (66.66, 75.00, 66.67, 30.00) (-33.34, +35.00, +26.67, +0.00) 35.00    FAIL
#5     (140.0, 40.0, 60.0, 30.0)    (133.33, 75.00, 66.66, 30.00) (-6.67, +35.00, +6.66, +0.00) 35.00    FAIL
Result [unequal_tracks / Tailwind]: FAIL

=== Scene: fixed_first_column [CSS] ===
item   solver (x, y, w, h)          chrome (x, y, w, h)          delta (dx, dy, dw, dh)       max_diff status
#0     (0.0, 0.0, 200.0, 40.0)      (0.00, 0.00, 200.00, 40.00)  (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#1     (210.0, 0.0, 110.0, 40.0)    (210.00, 0.00, 110.00, 40.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#2     (330.0, 0.0, 110.0, 40.0)    (330.00, 0.00, 110.00, 40.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#3     (0.0, 50.0, 200.0, 40.0)     (0.00, 95.00, 200.00, 40.00) (+0.00, +45.00, +0.00, +0.00) 45.00    FAIL
#4     (210.0, 50.0, 110.0, 40.0)   (210.00, 95.00, 110.00, 40.00) (+0.00, +45.00, +0.00, +0.00) 45.00    FAIL
#5     (330.0, 50.0, 110.0, 40.0)   (330.00, 95.00, 110.00, 40.00) (+0.00, +45.00, +0.00, +0.00) 45.00    FAIL
Result [fixed_first_column / CSS]: FAIL

=== Scene: fixed_first_column [Tailwind] ===
item   solver (x, y, w, h)          chrome (x, y, w, h)          delta (dx, dy, dw, dh)       max_diff status
#0     (0.0, 0.0, 200.0, 40.0)      (0.00, 0.00, 200.00, 40.00)  (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#1     (210.0, 0.0, 110.0, 40.0)    (210.00, 0.00, 110.00, 40.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#2     (330.0, 0.0, 110.0, 40.0)    (330.00, 0.00, 110.00, 40.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#3     (0.0, 50.0, 200.0, 40.0)     (0.00, 95.00, 200.00, 40.00) (+0.00, +45.00, +0.00, +0.00) 45.00    FAIL
#4     (210.0, 50.0, 110.0, 40.0)   (210.00, 95.00, 110.00, 40.00) (+0.00, +45.00, +0.00, +0.00) 45.00    FAIL
#5     (330.0, 50.0, 110.0, 40.0)   (330.00, 95.00, 110.00, 40.00) (+0.00, +45.00, +0.00, +0.00) 45.00    FAIL
Result [fixed_first_column / Tailwind]: FAIL

=== Scene: hugging_grid [CSS] ===
item   solver (x, y, w, h)          chrome (x, y, w, h)          delta (dx, dy, dw, dh)       max_diff status
#0     (8.0, 8.0, 100.0, 28.0)      (8.00, 8.00, 100.00, 28.00)  (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#1     (116.0, 8.0, 24.0, 28.0)     (76.00, 8.00, 24.00, 28.00)  (-40.00, +0.00, +0.00, +0.00) 40.00    FAIL
#2     (164.0, 8.0, 24.0, 28.0)     (144.00, 8.00, 24.00, 28.00) (-20.00, +0.00, +0.00, +0.00) 20.00    FAIL
#3     (8.0, 44.0, 100.0, 20.0)     (8.00, 44.00, 60.00, 20.00)  (+0.00, +0.00, -40.00, +0.00) 40.00    FAIL
#4     (116.0, 44.0, 40.0, 20.0)    (76.00, 44.00, 60.00, 20.00) (-40.00, +0.00, +20.00, +0.00) 40.00    FAIL
#5     (164.0, 44.0, 40.0, 20.0)    (144.00, 44.00, 60.00, 20.00) (-20.00, +0.00, +20.00, +0.00) 20.00    FAIL
Result [hugging_grid / CSS]: FAIL

=== Scene: hugging_grid [Tailwind] ===
item   solver (x, y, w, h)          chrome (x, y, w, h)          delta (dx, dy, dw, dh)       max_diff status
#0     (8.0, 8.0, 100.0, 28.0)      (8.00, 8.00, 100.00, 28.00)  (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#1     (116.0, 8.0, 24.0, 28.0)     (76.00, 8.00, 24.00, 28.00)  (-40.00, +0.00, +0.00, +0.00) 40.00    FAIL
#2     (164.0, 8.0, 24.0, 28.0)     (144.00, 8.00, 24.00, 28.00) (-20.00, +0.00, +0.00, +0.00) 20.00    FAIL
#3     (8.0, 44.0, 100.0, 20.0)     (8.00, 44.00, 60.00, 20.00)  (+0.00, +0.00, -40.00, +0.00) 40.00    FAIL
#4     (116.0, 44.0, 40.0, 20.0)    (76.00, 44.00, 60.00, 20.00) (-40.00, +0.00, +20.00, +0.00) 40.00    FAIL
#5     (164.0, 44.0, 40.0, 20.0)    (144.00, 44.00, 60.00, 20.00) (-20.00, +0.00, +20.00, +0.00) 20.00    FAIL
Result [hugging_grid / Tailwind]: FAIL

=== Scene: fixed_height_auto_rows [CSS] ===
item   solver (x, y, w, h)          chrome (x, y, w, h)          delta (dx, dy, dw, dh)       max_diff status
#0     (16.0, 16.0, 108.0, 24.0)    (16.00, 16.00, 108.00, 24.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#1     (136.0, 16.0, 108.0, 24.0)   (136.00, 16.00, 108.00, 24.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#2     (16.0, 56.0, 108.0, 36.0)    (16.00, 85.33, 108.00, 36.00) (+0.00, +29.33, +0.00, +0.00) 29.33    FAIL
#3     (136.0, 56.0, 108.0, 36.0)   (136.00, 85.33, 108.00, 36.00) (+0.00, +29.33, +0.00, +0.00) 29.33    FAIL
#4     (16.0, 108.0, 108.0, 28.0)   (16.00, 166.66, 108.00, 28.00) (+0.00, +58.66, +0.00, +0.00) 58.66    FAIL
#5     (136.0, 108.0, 108.0, 28.0)  (136.00, 166.66, 108.00, 28.00) (+0.00, +58.66, +0.00, +0.00) 58.66    FAIL
Result [fixed_height_auto_rows / CSS]: FAIL

=== Scene: fixed_height_auto_rows [Tailwind] ===
item   solver (x, y, w, h)          chrome (x, y, w, h)          delta (dx, dy, dw, dh)       max_diff status
#0     (16.0, 16.0, 108.0, 24.0)    (16.00, 16.00, 108.00, 24.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#1     (136.0, 16.0, 108.0, 24.0)   (136.00, 16.00, 108.00, 24.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#2     (16.0, 56.0, 108.0, 36.0)    (16.00, 85.33, 108.00, 36.00) (+0.00, +29.33, +0.00, +0.00) 29.33    FAIL
#3     (136.0, 56.0, 108.0, 36.0)   (136.00, 85.33, 108.00, 36.00) (+0.00, +29.33, +0.00, +0.00) 29.33    FAIL
#4     (16.0, 108.0, 108.0, 28.0)   (16.00, 108.00, 108.00, 28.00) (+0.00, +58.66, +0.00, +0.00) 58.66    FAIL
#5     (136.0, 108.0, 108.0, 28.0)  (136.00, 166.66, 108.00, 28.00) (+0.00, +58.66, +0.00, +0.00) 58.66    FAIL
Result [fixed_height_auto_rows / Tailwind]: FAIL

=== Scene: row_and_column_spans [CSS] ===
item   solver (x, y, w, h)          chrome (x, y, w, h)          delta (dx, dy, dw, dh)       max_diff status
#0     (12.0, 12.0, 196.0, 92.0)    (12.00, 12.00, 196.00, 92.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#1     (220.0, 12.0, 196.0, 40.0)   (220.00, 12.00, 196.00, 40.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#2     (220.0, 64.0, 92.0, 40.0)    (220.00, 82.66, 92.00, 40.00) (+0.00, +18.66, +0.00, +0.00) 18.66    FAIL
#3     (324.0, 64.0, 92.0, 40.0)    (324.00, 82.66, 92.00, 40.00) (+0.00, +18.66, +0.00, +0.00) 18.66    FAIL
#4     (12.0, 116.0, 404.0, 36.0)   (12.00, 153.33, 404.00, 36.00) (+0.00, +37.33, +0.00, +0.00) 37.33    FAIL
Result [row_and_column_spans / CSS]: FAIL

=== Scene: row_and_column_spans [Tailwind] ===
item   solver (x, y, w, h)          chrome (x, y, w, h)          delta (dx, dy, dw, dh)       max_diff status
#0     (12.0, 12.0, 196.0, 92.0)    (12.00, 12.00, 196.00, 92.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#1     (220.0, 12.0, 196.0, 40.0)   (220.00, 12.00, 196.00, 40.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#2     (220.0, 64.0, 92.0, 40.0)    (220.00, 82.66, 92.00, 40.00) (+0.00, +18.66, +0.00, +0.00) 18.66    FAIL
#3     (324.0, 64.0, 92.0, 40.0)    (324.00, 82.66, 92.00, 40.00) (+0.00, +18.66, +0.00, +0.00) 18.66    FAIL
#4     (12.0, 116.0, 404.0, 36.0)   (12.00, 153.33, 404.00, 36.00) (+0.00, +37.33, +0.00, +0.00) 37.33    FAIL
Result [row_and_column_spans / Tailwind]: FAIL

exit_code=1
```

### 4. Post-Fix Chrome Comparison (`plans/reports/agy-484-grid-tracks-css-evidence/chrome-green.log`)

```text
=== Scene: equal_tracks [CSS] ===
item   solver (x, y, w, h)          chrome (x, y, w, h)          delta (dx, dy, dw, dh)       max_diff status
#0     (12.0, 12.0, 92.0, 36.0)     (12.00, 12.00, 92.00, 36.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#1     (116.0, 12.0, 92.0, 36.0)    (116.00, 12.00, 92.00, 36.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#2     (220.0, 12.0, 92.0, 36.0)    (220.00, 12.00, 92.00, 36.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#3     (12.0, 60.0, 92.0, 36.0)     (12.00, 60.00, 92.00, 36.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#4     (116.0, 60.0, 92.0, 36.0)    (116.00, 60.00, 92.00, 36.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#5     (220.0, 60.0, 92.0, 36.0)    (220.00, 60.00, 92.00, 36.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
Result [equal_tracks / CSS]: PASS

=== Scene: equal_tracks [Tailwind] ===
item   solver (x, y, w, h)          chrome (x, y, w, h)          delta (dx, dy, dw, dh)       max_diff status
#0     (12.0, 12.0, 92.0, 36.0)     (12.00, 12.00, 92.00, 36.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#1     (116.0, 12.0, 92.0, 36.0)    (116.00, 12.00, 92.00, 36.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#2     (220.0, 12.0, 92.0, 36.0)    (220.00, 12.00, 92.00, 36.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#3     (12.0, 60.0, 92.0, 36.0)     (12.00, 60.00, 92.00, 36.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#4     (116.0, 60.0, 92.0, 36.0)    (116.00, 60.00, 92.00, 36.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#5     (220.0, 60.0, 92.0, 36.0)    (220.00, 60.00, 92.00, 36.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
Result [equal_tracks / Tailwind]: PASS

=== Scene: unequal_tracks [CSS] ===
item   solver (x, y, w, h)          chrome (x, y, w, h)          delta (dx, dy, dw, dh)       max_diff status
#0     (0.0, 0.0, 100.0, 30.0)      (0.00, 0.00, 100.00, 30.00)  (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#1     (100.0, 0.0, 40.0, 30.0)     (100.00, 0.00, 40.00, 30.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#2     (140.0, 0.0, 60.0, 30.0)     (140.00, 0.00, 60.00, 30.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#3     (0.0, 40.0, 100.0, 30.0)     (0.00, 40.00, 100.00, 30.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#4     (100.0, 40.0, 40.0, 30.0)    (100.00, 40.00, 40.00, 30.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#5     (140.0, 40.0, 60.0, 30.0)    (140.00, 40.00, 60.00, 30.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
Result [unequal_tracks / CSS]: PASS

=== Scene: unequal_tracks [Tailwind] ===
item   solver (x, y, w, h)          chrome (x, y, w, h)          delta (dx, dy, dw, dh)       max_diff status
#0     (0.0, 0.0, 100.0, 30.0)      (0.00, 0.00, 100.00, 30.00)  (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#1     (100.0, 0.0, 40.0, 30.0)     (100.00, 0.00, 40.00, 30.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#2     (140.0, 0.0, 60.0, 30.0)     (140.00, 0.00, 60.00, 30.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#3     (0.0, 40.0, 100.0, 30.0)     (0.00, 40.00, 100.00, 30.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#4     (100.0, 40.0, 40.0, 30.0)    (100.00, 40.00, 40.00, 30.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#5     (140.0, 40.0, 60.0, 30.0)    (140.00, 40.00, 60.00, 30.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
Result [unequal_tracks / Tailwind]: PASS

=== Scene: fixed_first_column [CSS] ===
item   solver (x, y, w, h)          chrome (x, y, w, h)          delta (dx, dy, dw, dh)       max_diff status
#0     (0.0, 0.0, 200.0, 40.0)      (0.00, 0.00, 200.00, 40.00)  (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#1     (210.0, 0.0, 110.0, 40.0)    (210.00, 0.00, 110.00, 40.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#2     (330.0, 0.0, 110.0, 40.0)    (330.00, 0.00, 110.00, 40.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#3     (0.0, 50.0, 200.0, 40.0)     (0.00, 50.00, 200.00, 40.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#4     (210.0, 50.0, 110.0, 40.0)   (210.00, 50.00, 110.00, 40.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#5     (330.0, 50.0, 110.0, 40.0)   (330.00, 50.00, 110.00, 40.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
Result [fixed_first_column / CSS]: PASS

=== Scene: fixed_first_column [Tailwind] ===
item   solver (x, y, w, h)          chrome (x, y, w, h)          delta (dx, dy, dw, dh)       max_diff status
#0     (0.0, 0.0, 200.0, 40.0)      (0.00, 0.00, 200.00, 40.00)  (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#1     (210.0, 0.0, 110.0, 40.0)    (210.00, 0.00, 110.00, 40.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#2     (330.0, 0.0, 110.0, 40.0)    (330.00, 0.00, 110.00, 40.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#3     (0.0, 50.0, 200.0, 40.0)     (0.00, 50.00, 200.00, 40.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#4     (210.0, 50.0, 110.0, 40.0)   (210.00, 50.00, 110.00, 40.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#5     (330.0, 50.0, 110.0, 40.0)   (330.00, 50.00, 110.00, 40.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
Result [fixed_first_column / Tailwind]: PASS

=== Scene: hugging_grid [CSS] ===
item   solver (x, y, w, h)          chrome (x, y, w, h)          delta (dx, dy, dw, dh)       max_diff status
#0     (8.0, 8.0, 100.0, 28.0)      (8.00, 8.00, 100.00, 28.00)  (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#1     (116.0, 8.0, 24.0, 28.0)     (116.00, 8.00, 24.00, 28.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#2     (164.0, 8.0, 24.0, 28.0)     (164.00, 8.00, 24.00, 28.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#3     (8.0, 44.0, 100.0, 20.0)     (8.00, 44.00, 100.00, 20.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#4     (116.0, 44.0, 40.0, 20.0)    (116.00, 44.00, 40.00, 20.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#5     (164.0, 44.0, 40.0, 20.0)    (164.00, 44.00, 40.00, 20.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
Result [hugging_grid / CSS]: PASS

=== Scene: hugging_grid [Tailwind] ===
item   solver (x, y, w, h)          chrome (x, y, w, h)          delta (dx, dy, dw, dh)       max_diff status
#0     (8.0, 8.0, 100.0, 28.0)      (8.00, 8.00, 100.00, 28.00)  (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#1     (116.0, 8.0, 24.0, 28.0)     (116.00, 8.00, 24.00, 28.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#2     (164.0, 8.0, 24.0, 28.0)     (164.00, 8.00, 24.00, 28.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#3     (8.0, 44.0, 100.0, 20.0)     (8.00, 44.00, 100.00, 20.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#4     (116.0, 44.0, 40.0, 20.0)    (116.00, 44.00, 40.00, 20.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#5     (164.0, 44.0, 40.0, 20.0)    (164.00, 44.00, 40.00, 20.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
Result [hugging_grid / Tailwind]: PASS

=== Scene: fixed_height_auto_rows [CSS] ===
item   solver (x, y, w, h)          chrome (x, y, w, h)          delta (dx, dy, dw, dh)       max_diff status
#0     (16.0, 16.0, 108.0, 24.0)    (16.00, 16.00, 108.00, 24.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#1     (136.0, 16.0, 108.0, 24.0)   (136.00, 16.00, 108.00, 24.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#2     (16.0, 56.0, 108.0, 36.0)    (16.00, 56.00, 108.00, 36.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#3     (136.0, 56.0, 108.0, 36.0)   (136.00, 56.00, 108.00, 36.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#4     (16.0, 108.0, 108.0, 28.0)   (16.00, 108.00, 108.00, 28.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#5     (136.0, 108.0, 108.0, 28.0)  (136.00, 108.00, 108.00, 28.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
Result [fixed_height_auto_rows / CSS]: PASS

=== Scene: fixed_height_auto_rows [Tailwind] ===
item   solver (x, y, w, h)          chrome (x, y, w, h)          delta (dx, dy, dw, dh)       max_diff status
#0     (16.0, 16.0, 108.0, 24.0)    (16.00, 16.00, 108.00, 24.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#1     (136.0, 16.0, 108.0, 24.0)   (136.00, 16.00, 108.00, 24.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#2     (16.0, 56.0, 108.0, 36.0)    (16.00, 56.00, 108.00, 36.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#3     (136.0, 56.0, 108.0, 36.0)   (136.00, 56.00, 108.00, 36.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#4     (16.0, 108.0, 108.0, 28.0)   (16.00, 108.00, 108.00, 28.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#5     (136.0, 108.0, 108.0, 28.0)  (136.00, 108.00, 108.00, 28.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
Result [fixed_height_auto_rows / Tailwind]: PASS

=== Scene: row_and_column_spans [CSS] ===
item   solver (x, y, w, h)          chrome (x, y, w, h)          delta (dx, dy, dw, dh)       max_diff status
#0     (12.0, 12.0, 196.0, 92.0)    (12.00, 12.00, 196.00, 92.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#1     (220.0, 12.0, 196.0, 40.0)   (220.00, 12.00, 196.00, 40.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#2     (220.0, 64.0, 92.0, 40.0)    (220.00, 64.00, 92.00, 40.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#3     (324.0, 64.0, 92.0, 40.0)    (324.00, 64.00, 92.00, 40.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#4     (12.0, 116.0, 404.0, 36.0)   (12.00, 116.00, 404.00, 36.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
Result [row_and_column_spans / CSS]: PASS

=== Scene: row_and_column_spans [Tailwind] ===
item   solver (x, y, w, h)          chrome (x, y, w, h)          delta (dx, dy, dw, dh)       max_diff status
#0     (12.0, 12.0, 196.0, 92.0)    (12.00, 12.00, 196.00, 92.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#1     (220.0, 12.0, 196.0, 40.0)   (220.00, 12.00, 196.00, 40.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#2     (220.0, 64.0, 92.0, 40.0)    (220.00, 64.00, 92.00, 40.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#3     (324.0, 64.0, 92.0, 40.0)    (324.00, 64.00, 92.00, 40.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
#4     (12.0, 116.0, 404.0, 36.0)   (12.00, 116.00, 404.00, 36.00) (+0.00, +0.00, +0.00, +0.00) 0.00     PASS
Result [row_and_column_spans / Tailwind]: PASS

Wrote screenshot: plans/reports/agy-484-grid-tracks-css-evidence/unequal-tracks-chrome-vs-solver.png
exit_code=0
```

### 5. Missing Chrome Binary Check

```text
$ python3 scripts/qa/grid-chrome-compare.py --chrome /nonexistent/chrome
ERROR: Google Chrome binary not found or not executable at '/nonexistent/chrome'
exit_code=2
```

---

## Assumptions

1. **Equal solved `Flex` tracks keep `1fr` / `grid-cols-N`**: When a hugging grid's `GridTrack::Flex` columns all solve to the exact same pixel width per `fr` unit, `repeat(N, minmax(0, 1fr))` / `grid-cols-N` already produces identical item boxes in Chrome and is preserved so existing equal-column tests remain idiomatic.
2. **Tailwind class-to-CSS translation in `scripts/qa/grid-chrome-compare.py`**: Since headless Chrome cannot load the Tailwind CDN offline, `scripts/qa/grid-chrome-compare.py` translates the emitted Tailwind utility classes (including arbitrary track templates `grid-cols-[100px_40px_60px]`, `grid-cols-[200px_minmax(0,1fr)_minmax(0,1fr)]`, spacing scale tokens, spans, and `content-start`) directly into their canonical CSS declarations before measuring in Chrome.

---

## Skills Loaded and What Changed Because of Them

- **`es:rust`** (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`): Guided borrowing/allocation discipline in `resolved_grid_col_tracks` (borrowing `&al.grid_cols` via `Cow::Borrowed` when no Hugging-Flex inequality occurs, avoiding unnecessary `Vec` clones on the common path).
- **`es:astra-work`** (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`): Enforced red-before-green evidence collection (`red.log` and `chrome-red.log` captured before any exporter fix, followed by `green.log`, `chrome-green.log`, and visual inspection of `unequal-tracks-chrome-vs-solver.png`).
- **`es:fable-thinking`** (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`): Drove falsification-first root-cause analysis (checking `designos_doc::layout::solve_grid` directly to prove how `PrimarySizing::Hug` resolves `GridTrack::Flex` per column and how `PrimarySizing::Fixed` on counter-sizing packs auto rows from `padding.top`), plus explicit predictions before every verification command.

---

## Epistemic Ledger (`FACT` / `INFERENCE` / `ASSUMPTION`)

- **`FACT`**: In `crates/designos-doc/src/layout.rs` (`solve_grid`), when `al.primary_sizing == PrimarySizing::Hug`, each `GridTrack::Flex` column width is set to the maximum natural width (`min_w.max(w)`) of non-spanning (`col_span == 1`) children placed in that column, rather than enforcing an equal width per `1fr` across all `Flex` columns.
- **`FACT`**: In `crates/designos-doc/src/layout.rs` (`solve_grid`), when `al.counter_sizing == PrimarySizing::Fixed` and rows are auto-sized (`grid_rows` empty or `GridTrack::Auto`), row `y` offsets advance by `row_h + row_gap` from `padding.top` without stretching auto row tracks to fill the container height.
- **`FACT`**: In headless Chrome (`Google Chrome --headless=new --disable-gpu --dump-dom`), a CSS Grid container with `height: 180px` and implicit auto rows defaults to `align-content: normal` (`stretch`), shifting row 2 downward by `36.00px` in `equal_tracks` unless `align-content: start` (`content-start` in Tailwind) is specified.
- **`FACT`**: With `resolved_grid_col_tracks` and `grid_packs_auto_rows` applied in `css.rs` and `tailwind.rs`, all 6 fixture scenes (`equal_tracks`, `unequal_tracks`, `fixed_first_column`, `hugging_grid`, `fixed_height_auto_rows`, `row_and_column_spans`) render in headless Chrome with `0.00px` maximum bounding-box delta against `designos_doc::layout::solve_document` across both CSS and Tailwind exports.
- **`FACT`**: `scripts/tests/test_shipped_resources.py` (`test_examples_removed_and_unreferenced`) forbids the substring `"examples/"` anywhere in `crates/`, `docs/`, or `scripts/`. Hosting the JSON fixture dumper inside `crates/designos-export/tests/grid_chrome_fixtures.rs` (triggered via `GRID_CHROME_DUMP_JSON=1`) keeps `crates/` and `scripts/` free of any `examples/` reference and passes all 199 tests in `scripts/tests`.
- **`INFERENCE`**: Any width-hugging grid whose `GridTrack::Flex` columns resolve to equal widths per `fr` unit will lay out identically in Chrome whether exported as `repeat(N, minmax(0, 1fr))` or fixed pixel tracks, because `(container_width - padding - gaps)` equals the sum of those equal tracks.
- **`ASSUMPTION`**: `scripts/qa/grid-chrome-compare.py`'s class-to-CSS map covers the full vocabulary of grid utility classes emitted by `crates/designos-export/src/tailwind.rs` for grid containers and grid items.

---

## Round 2

### 1. What Could NOT Be Verified (Round 2)

1. **Full Tailwind v3/v4 JIT compiler binary inside the offline Chrome harness**: As in Round 1, headless Chrome runs offline against `file://` without loading the Tailwind CDN or JIT binary; `scripts/qa/grid_chrome/tailwind_css_map.py:92-145` (`tailwind_token_to_decls`) translates the utility classes emitted by `crates/designos-export/src/tailwind.rs:347-380` into CSS declarations.
2. **Native mobile/desktop stack generators (`crates/designos-export/src/{swiftui.rs,compose.rs,uikit.rs,android_xml.rs}`)**: Out of scope by controller instruction ("Leave the native generators (`swiftui.rs`, `compose.rs`, `uikit.rs`, `android_xml.rs`) for the later task").
3. **3 ignored external-tool tests in `crates/designos-export/tests/slot_codegen.rs` and 1 ignored private-fixture test in `crates/designos-export/tests/kit_export_bounds.rs`**: Require `npx`/`esbuild`/`typescript`/`swiftc` or the private Untitled UI reference `.fig` file.
4. **Live CSS Grid with `grid_auto_tracks = GridAutoTracks::Track(GridTrack::Fixed(px))` or `GridAutoTracks::Track(GridTrack::Flex(fr))` (scenes S8/S9 in reviewer probe)**: `crates/designos-export/src/css.rs` and `crates/designos-export/src/tailwind.rs` do not yet emit `grid-auto-rows` / `auto-rows-*`; noted as a follow-up outside Round 2's stated scope.

---

### 2. Conclusion & Root-Cause Fixes (Round 2)

1. **MAJOR 1 — Deleted copied column rule in `crates/designos-export/src/codegen.rs` and exposed `layout_solver::grid_column_widths`**:
   - Extracted `column_widths` and added the public read-only accessor `pub fn grid_column_widths(al: &AutoLayout, frame: Vec2, own: SizeLimits, kids: &[ChildIn]) -> Vec<f32>` in `crates/designos-doc/src/layout_solver_grid.rs:111-168` (re-exported at `crates/designos-doc/src/layout_solver.rs:16`), with unit test `grid_column_widths_matches_compute_grid_columns` at `crates/designos-doc/src/layout_solver_grid.rs:518-539`.
   - Replaced the 100-line duplicate column solver in `crates/designos-export/src/codegen.rs:590-629` (`resolved_grid_col_tracks`) with `doc.flow_child_input(*id, true)` (`crates/designos-doc/src/layout_apply.rs:85-112`), `al.with_stroke(&n.stroke)`, `SizeLimits::from(&n.layout_child)`, `(n.size.x - compute_grid(&al_s, n.size, own, &kids).frame.x).abs() > 0.5`, and `grid_column_widths(&al_s, n.size, own, &kids)`.
   - Added the 3 drift scenes `s1_stretch_child_hug`, `s2_empty_hug_column`, and `s3_hug_grid_max_w` to `crates/designos-export/tests/grid_chrome_fixtures.rs:237-315`. Each renders at `0.00px` drift in headless Chrome across both CSS and Tailwind.
2. **MAJOR 2 — Fixed `grid_packs_auto_rows` (`crates/designos-export/src/codegen.rs:582-585`)**:
   - In `crates/designos-doc/src/layout_solver_grid.rs:235-238` (`compute_grid`), the solver levels rows across `frame.y` only when `al.counter_sizing == Sizing::Fixed && matches!(al.grid_auto_tracks, GridAutoTracks::Track(GridTrack::Flex(fr)) if fr > 0.0)`; in every other grid (including a `Sizing::Hug` height raised by `min_h` with or without `GridTrack::Flex` auto rows, and explicit `grid_rows > 0` with `GridTrack::Hug` row tracks), rows are packed from `pt` (`crates/designos-doc/src/layout_solver_grid.rs:240-251`).
   - Updated `grid_packs_auto_rows` in `crates/designos-export/src/codegen.rs:582-585` to `al.direction == LayoutDirection::Grid && !(al.counter_sizing == Sizing::Fixed && matches!(al.grid_auto_tracks, GridAutoTracks::Track(GridTrack::Flex(fr)) if fr > 0.0))`, updated the affected golden strings in `crates/designos-export/src/css.rs:745,926`, `crates/designos-export/src/tailwind.rs:692`, and `crates/designos-export/src/react.rs:345,362`, and added scenes `s5_hug_height_min_h`, `s5b_hug_height_min_h_flex_auto`, and `s7_explicit_rows_hug` (`crates/designos-export/tests/grid_chrome_fixtures.rs:317-402`). All 3 render at `0.00px` in Chrome.
3. **Box-Sizing in Export & Chrome Harness Hardening / Split**:
   - Removed the `* { box-sizing: border-box; }` reset from `scripts/qa/grid_chrome/probe_page.py:42-55` and `scripts/qa/grid_chrome/screenshot.py:55-65`.
   - Emitted `box-sizing: border-box;` on padded grids in `crates/designos-export/src/css.rs:122-124` and `box-border` on padded grids in `crates/designos-export/src/tailwind.rs:376-380` (mapped in `scripts/qa/grid_chrome/tailwind_css_map.py:67`), matching `compute_grid`'s border-box `inner_w = frame.x - pl - pr` (`crates/designos-doc/src/layout_solver_grid.rs:121`).
   - Hardened `compare_boxes` (`scripts/qa/grid_chrome/probe_page.py:131-147`) and `parse_tailwind_html` (`scripts/qa/grid_chrome/tailwind_css_map.py:148-159`) so any missing or extra item fails with `inf` error / `ValueError` and exits `1`, and `TailwindChildExtractor` (`scripts/qa/grid_chrome/tailwind_css_map.py:12-38`) extracts all direct child elements (including void tags such as `<img>`).
   - Split `scripts/qa/grid-chrome-compare.py` into 5 files under 200 lines each (`scripts/qa/grid-chrome-compare.py`: 51 lines; `scripts/qa/grid_chrome/__init__.py`: 28 lines; `scripts/qa/grid_chrome/fixtures.py`: 88 lines; `scripts/qa/grid_chrome/probe_page.py`: 172 lines; `scripts/qa/grid_chrome/screenshot.py`: 176 lines; `scripts/qa/grid_chrome/tailwind_css_map.py`: 165 lines) plus unit tests in `scripts/tests/test_grid_chrome_compare.py` (61 lines).
   - Added unit/integration tests killing all 6 previously-surviving mutations (`M4`, `M6`, `M7`, `M11`, `M13`, `M14`) in `crates/designos-export/tests/grid_chrome_fixtures.rs:536-625` and `crates/designos-doc/src/layout_solver_grid.rs:518-539`.

---

### 3. Acceptance Table (Round 2)

| # | Check | Command / Artifact | Result |
|---|---|---|---|
| 1a | Chrome harness: all 12 scenes (6 original + S1, S2, S3 + S5, S5b, S7), CSS & Tailwind at `0.00px` | `python3 scripts/qa/grid-chrome-compare.py --screenshot plans/reports/agy-484-grid-tracks-css-evidence/r2-unequal-tracks-chrome-vs-solver.png` (`plans/reports/agy-484-grid-tracks-css-evidence/r2-chrome-green.log`) | **PASS** (`24/24` checks at `0.00px`, `exit=0`) |
| 1b | Deleted item makes Chrome harness exit non-zero | `plans/reports/agy-484-grid-tracks-css-evidence/r2-chrome-missing-item.log` | **PASS** (`MISSING_ITEM_EXIT=1`, `item count mismatch: solver=6 vs chrome=5`) |
| 2a | `r2-red.log` & `r2-green.log` regenerated from the final `crates/designos-export/tests/grid_chrome_fixtures.rs` | `plans/reports/agy-484-grid-tracks-css-evidence/r2-red.log` (`10 failed, RED_EXIT=101`) & `plans/reports/agy-484-grid-tracks-css-evidence/r2-green.log` (`17 passed, GREEN_EXIT=0`) | **PASS** |
| 2b | All 6 surviving mutations (`M4`, `M6`, `M7`, `M11`, `M13`, `M14`) killed | `plans/reports/agy-484-grid-tracks-css-evidence/r2-mutations.log` | **PASS** (`6/6 KILLED`, each `exit=101`) |
| 3a | `cargo fmt --all -- --check` | Under mandatory sandbox harness | **PASS** (`exit=0`) |
| 3b | `cargo clippy --workspace --all-targets --locked -- -D warnings` | Under mandatory sandbox harness | **PASS** (`exit=0`) |
| 3c | `cargo test --workspace --locked --lib` | Under mandatory sandbox harness | **PASS** (`exit=0`) |
| 3d | `cargo test -p designos-export --locked` & `cargo test -p designos-doc --locked` | Under mandatory sandbox harness | **PASS** (`exit=0`) |
| 3e | `python3 -m unittest discover -s scripts/tests` | 203 tests (including `test_grid_chrome_compare.py`) | **PASS** (`Ran 203 tests`, `OK`, `exit=0`) |
| 4 | Delete `out/cargo` at the end | `rm -rf out/cargo` | **PASS** |

---

### 4. Pasted Verbatim Outputs (Round 2)

#### 4.1 Chrome Harness (`r2-chrome-green.log` & `r2-chrome-missing-item.log`)

```text
$ python3 scripts/qa/grid-chrome-compare.py --screenshot plans/reports/agy-484-grid-tracks-css-evidence/r2-unequal-tracks-chrome-vs-solver.png
[PASS] equal_tracks                   (css     ) max_item_err=0.00px grid_err=(0.00px, 0.00px)
[PASS] equal_tracks                   (tailwind) max_item_err=0.00px grid_err=(0.00px, 0.00px)
[PASS] unequal_tracks                 (css     ) max_item_err=0.00px grid_err=(0.00px, 0.00px)
[PASS] unequal_tracks                 (tailwind) max_item_err=0.00px grid_err=(0.00px, 0.00px)
[PASS] fixed_first_column             (css     ) max_item_err=0.00px grid_err=(0.00px, 0.00px)
[PASS] fixed_first_column             (tailwind) max_item_err=0.00px grid_err=(0.00px, 0.00px)
[PASS] hugging_grid                   (css     ) max_item_err=0.00px grid_err=(0.00px, 0.00px)
[PASS] hugging_grid                   (tailwind) max_item_err=0.00px grid_err=(0.00px, 0.00px)
[PASS] fixed_height_auto_rows         (css     ) max_item_err=0.00px grid_err=(0.00px, 0.00px)
[PASS] fixed_height_auto_rows         (tailwind) max_item_err=0.00px grid_err=(0.00px, 0.00px)
[PASS] row_and_column_spans           (css     ) max_item_err=0.00px grid_err=(0.00px, 0.00px)
[PASS] row_and_column_spans           (tailwind) max_item_err=0.00px grid_err=(0.00px, 0.00px)
[PASS] s1_stretch_child_hug           (css     ) max_item_err=0.00px grid_err=(0.00px, 0.00px)
[PASS] s1_stretch_child_hug           (tailwind) max_item_err=0.00px grid_err=(0.00px, 0.00px)
[PASS] s2_empty_hug_column            (css     ) max_item_err=0.00px grid_err=(0.00px, 0.00px)
[PASS] s2_empty_hug_column            (tailwind) max_item_err=0.00px grid_err=(0.00px, 0.00px)
[PASS] s3_hug_grid_max_w              (css     ) max_item_err=0.00px grid_err=(0.00px, 0.00px)
[PASS] s3_hug_grid_max_w              (tailwind) max_item_err=0.00px grid_err=(0.00px, 0.00px)
[PASS] s5_hug_height_min_h            (css     ) max_item_err=0.00px grid_err=(0.00px, 0.00px)
[PASS] s5_hug_height_min_h            (tailwind) max_item_err=0.00px grid_err=(0.00px, 0.00px)
[PASS] s5b_hug_height_min_h_flex_auto (css     ) max_item_err=0.00px grid_err=(0.00px, 0.00px)
[PASS] s5b_hug_height_min_h_flex_auto (tailwind) max_item_err=0.00px grid_err=(0.00px, 0.00px)
[PASS] s7_explicit_rows_hug           (css     ) max_item_err=0.00px grid_err=(0.00px, 0.00px)
[PASS] s7_explicit_rows_hug           (tailwind) max_item_err=0.00px grid_err=(0.00px, 0.00px)
Saved screenshot: plans/reports/agy-484-grid-tracks-css-evidence/r2-unequal-tracks-chrome-vs-solver.png
CHROME_GREEN_EXIT=0
```

```text
[FAIL] equal_tracks (css) max_item_err=inf rows=[{'index': -1, 'solver': {'count': 6}, 'chrome': {'count': 5}, 'dx': inf, 'dy': inf, 'dw': inf, 'dh': inf, 'max_abs': inf, 'error': 'item count mismatch: solver=6 vs chrome=5 (diff=1)'}]
MISSING_ITEM_EXIT=1
```

#### 4.2 Red & Green Logs from the Final `grid_chrome_fixtures.rs` (`r2-red.log` & `r2-green.log`)

```text
     Running tests/grid_chrome_fixtures.rs (out/cargo/debug/deps/grid_chrome_fixtures-68e44f521f53a4ec)

running 17 tests
test mutation_m11_fixed_height_grid_with_flex_auto_rows_omits_align_content_start ... ok
test mutation_m7_non_fill_child_width_wins_over_stale_wider_fill_child ... ok
test mutation_m14_flex_unit_tolerance_distinguishes_unequal_and_weighted_equal_columns ... ok
test mutation_m4_hugging_grid_min_w_distributes_extra_across_flex_columns ... ok
test mutation_m13_fixed_width_grid_with_weighted_flex_columns_keeps_fractional_tracks ... ok
test s2_empty_hug_column_css_and_tailwind ... FAILED
test row_and_column_spans_css_and_tailwind ... FAILED
test s5_hug_height_min_h_css_and_tailwind ... FAILED
test equal_tracks_css_and_tailwind ... FAILED
test hugging_grid_css_and_tailwind ... FAILED
test s1_stretch_child_hug_css_and_tailwind ... FAILED
test s3_hug_grid_max_w_css_and_tailwind ... FAILED
test fixed_height_auto_rows_css_and_tailwind ... FAILED
test fixed_first_column_css_and_tailwind ... ok
test s7_explicit_rows_hug_css_and_tailwind ... FAILED
test s5b_hug_height_min_h_flex_auto_css_and_tailwind ... FAILED
test unequal_tracks_css_and_tailwind ... ok

failures:
    equal_tracks_css_and_tailwind
    fixed_height_auto_rows_css_and_tailwind
    hugging_grid_css_and_tailwind
    row_and_column_spans_css_and_tailwind
    s1_stretch_child_hug_css_and_tailwind
    s2_empty_hug_column_css_and_tailwind
    s3_hug_grid_max_w_css_and_tailwind
    s5_hug_height_min_h_css_and_tailwind
    s5b_hug_height_min_h_flex_auto_css_and_tailwind
    s7_explicit_rows_hug_css_and_tailwind

test result: FAILED. 7 passed; 10 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
RED_EXIT=101
```

```text
     Running tests/grid_chrome_fixtures.rs (out/cargo/debug/deps/grid_chrome_fixtures-68e44f521f53a4ec)

running 17 tests
test mutation_m4_hugging_grid_min_w_distributes_extra_across_flex_columns ... ok
test mutation_m7_non_fill_child_width_wins_over_stale_wider_fill_child ... ok
test fixed_height_auto_rows_css_and_tailwind ... ok
test s2_empty_hug_column_css_and_tailwind ... ok
test s3_hug_grid_max_w_css_and_tailwind ... ok
test hugging_grid_css_and_tailwind ... ok
test mutation_m11_fixed_height_grid_with_flex_auto_rows_omits_align_content_start ... ok
test mutation_m13_fixed_width_grid_with_weighted_flex_columns_keeps_fractional_tracks ... ok
test s5_hug_height_min_h_css_and_tailwind ... ok
test mutation_m14_flex_unit_tolerance_distinguishes_unequal_and_weighted_equal_columns ... ok
test s5b_hug_height_min_h_flex_auto_css_and_tailwind ... ok
test equal_tracks_css_and_tailwind ... ok
test s7_explicit_rows_hug_css_and_tailwind ... ok
test s1_stretch_child_hug_css_and_tailwind ... ok
test row_and_column_spans_css_and_tailwind ... ok
test fixed_first_column_css_and_tailwind ... ok
test unequal_tracks_css_and_tailwind ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
GREEN_EXIT=0
```

#### 4.3 Mutation Results (`r2-mutations.log`)

```text
M4_disable_min_w_extra_distribution: KILLED (exit=101)
M6_flow_child_input_horiz_false: KILLED (exit=101)
M7_drop_non_fill_precedence_in_widest_in_column: KILLED (exit=101)
M11_drop_flex_auto_exclusion_in_grid_packs_auto_rows: KILLED (exit=101)
M13_drop_hug_gate_in_resolved_grid_col_tracks: KILLED (exit=101)
M14_flex_unit_tolerance_50px: KILLED (exit=101)
Summary: 6/6 mutations KILLED
```

#### 4.4 Mandatory Gates (`r2-gates.log`)

```text
=== GATE 1: cargo fmt --all -- --check ===
FMT_EXIT=0
=== GATE 2: cargo clippy --workspace --all-targets --locked -- -D warnings ===
CLIPPY_EXIT=0
=== GATE 3: cargo test --workspace --locked --lib ===
TEST_WORKSPACE_LIB_EXIT=0
=== GATE 4: cargo test -p designos-export --locked ===
test result: ok. 153 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.16s
...
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
...
TEST_EXPORT_EXIT=0
=== GATE 5: cargo test -p designos-doc --locked ===
test result: ok. 673 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.97s
...
TEST_DOC_EXIT=0
=== GATE 6: python3 -m unittest discover -s scripts/tests ===
Ran 203 tests in 55.709s

OK
PYTEST_EXIT=0
```

---

### 5. Assumptions (Round 2)

1. **`box-sizing: border-box;` / `box-border` emitted on padded grid containers**: In `crates/designos-doc/src/layout_solver_grid.rs:121`, `compute_grid` subtracts horizontal padding (`pl + pr`) from `frame.x` (`n.size.x`) and vertical padding (`pt + pb`) from `frame.y` (`n.size.y`). Emitting `box-sizing: border-box;` in `crates/designos-export/src/css.rs:122-124` when `t + r + b + l > 0.0` and `box-border` in `crates/designos-export/src/tailwind.rs:376-380` when `!pads.is_empty()` ensures padded grids have border-box dimensions on a real page without any global CSS reset while keeping unpadded grid output unchanged.
2. **Golden test updates in `crates/designos-export/src/react.rs:341-366`**: `react::tests::golden_react_grid_inline_and_tailwind` tests `react_for` and `react_with(..., &ReactOptions { tailwind: true })` on a padded `Sizing::Fixed`-height grid with explicit `[Fixed(80.0), Hug]` rows, which calls `codegen::grid_packs_auto_rows` and `tailwind::markup`. Updating its expected strings was explicitly listed in the review (`code-reviewer-261031-0845-pr-1426.md:93`) and controller instructions (`brief-agy-484-r2.txt:22`).

---

### 6. Skills Loaded and What Changed Because of Them (Round 2)

- **`es:dev` & `es:feature-dev`** (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`): Executed directly in the DEV phase using the controller's ruling as the approved spec, with strict red-before-green `test(…)` then `fix(…)` commits for each deliverable slice.
- **`es:rust` & `es:rust-gpu-canvas`** (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`): Kept `grid_column_widths` (`crates/designos-doc/src/layout_solver_grid.rs:111-116`) pure and read-only, sharing `column_widths` with `compute_grid` (`crates/designos-doc/src/layout_solver_grid.rs:118-168,189`) without altering solver behavior, and preserved `Cow::Borrowed(&al.grid_col_tracks)` on the fast path in `resolved_grid_col_tracks` (`crates/designos-export/src/codegen.rs:590-629`).
- **`es:fable-thinking` & `es:astra-work`** (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`): Verified every symbol and `file:line` directly from files opened in this session, removed the harness's global `* { box-sizing: border-box }` reset so the browser comparison reflects real page rendering, and verified all 6 previously-surviving mutations are killed.

---

### 7. Epistemic Ledger (Round 2 — `FACT` / `INFERENCE` / `ASSUMPTION`)

- **`FACT`**: `crates/designos-doc/src/layout_solver_grid.rs:111-168` defines `pub fn grid_column_widths(al: &AutoLayout, frame: Vec2, own: SizeLimits, kids: &[ChildIn]) -> Vec<f32>` and shares `column_widths` with `compute_grid` (`crates/designos-doc/src/layout_solver_grid.rs:174-189`), re-exported at `crates/designos-doc/src/layout_solver.rs:16`.
- **`FACT`**: `crates/designos-export/src/codegen.rs:590-629` (`resolved_grid_col_tracks`) builds `kids` via `doc.flow_child_input(*id, true)` (`crates/designos-doc/src/layout_apply.rs:85-112`), applies `al.with_stroke(&n.stroke)` and `SizeLimits::from(&n.layout_child)`, checks `(n.size.x - solved.frame.x).abs() > 0.5` against `compute_grid`, and reads column widths from `grid_column_widths(&al_s, n.size, own, &kids)`.
- **`FACT`**: `crates/designos-export/src/codegen.rs:582-585` (`grid_packs_auto_rows`) returns `al.direction == LayoutDirection::Grid && !(al.counter_sizing == Sizing::Fixed && matches!(al.grid_auto_tracks, GridAutoTracks::Track(GridTrack::Flex(fr)) if fr > 0.0))`, matching the exact condition in `crates/designos-doc/src/layout_solver_grid.rs:235` where `compute_grid` levels flex auto-rows.
- **`FACT`**: With `* { box-sizing: border-box; }` removed from `scripts/qa/grid_chrome/probe_page.py:42-55` and `scripts/qa/grid_chrome/screenshot.py:55-65`, all 12 fixture scenes (`equal_tracks`, `unequal_tracks`, `fixed_first_column`, `hugging_grid`, `fixed_height_auto_rows`, `row_and_column_spans`, `s1_stretch_child_hug`, `s2_empty_hug_column`, `s3_hug_grid_max_w`, `s5_hug_height_min_h`, `s5b_hug_height_min_h_flex_auto`, `s7_explicit_rows_hug`) pass at `0.00px` drift in headless Chrome across both CSS and Tailwind (`plans/reports/agy-484-grid-tracks-css-evidence/r2-chrome-green.log`).
- **`FACT`**: All 6 previously-surviving code mutations (`M4`, `M6`, `M7`, `M11`, `M13`, `M14`) are killed (`exit=101`) by `cargo test -p designos-export --test grid_chrome_fixtures --locked` (`plans/reports/agy-484-grid-tracks-css-evidence/r2-mutations.log`).
- **`INFERENCE`**: Calling `doc.flow_child_input(*id, true)` and `grid_column_widths(&al_s, n.size, own, &kids)` in `resolved_grid_col_tracks` prevents future drift between `compute_grid` and CSS/Tailwind column track export because both execute the same `column_widths` function on the same `ChildIn` slice.
- **`ASSUMPTION`**: Unpadded grids without borders do not need `box-sizing: border-box;` / `box-border` because `content-box` and `border-box` coincide when padding and border are zero.

**Weakest link**: `scripts/qa/grid_chrome/tailwind_css_map.py` translates Tailwind utility classes via a static stdlib table rather than running the full Tailwind CSS compiler offline, though every token emitted by `tailwind_for` across all 12 scenes is covered and verified in headless Chrome.

**Unresolved questions**: None.

Status: DONE
