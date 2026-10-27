# Report — DEV · #1154 (P0, US-18): NodeSpec Hug Text Measured via `cosmic-text` Instead of Character-Count Estimate

- **Issue:** `#1154` (P0, US-18)
- **Branch:** `jangtrinh/agy-1154-nodespec-text-measure` (based on `origin/release/v0.6.4` at `6f0c3bb2`)
- **Evidence folder:** `plans/reports/agy-1154-nodespec-text-measure-evidence/`

---

## 1. Executive Summary

- **Root Cause (`FACT`):** `crates/designos-doc/src/spec.rs:1166-1231` (`estimate_intrinsic_text_size`) sized hug (`WidthAndHeight`) and fixed-width (`Height`) text nodes using a `0.56 * font_size * weight_factor` character-count heuristic (`+ 2.0 px` padding, rounded up). For proportional Latin labels such as `"Delete"` in 14px Semibold Inter, the estimate returned `52.0 px` while the actual shaped width in `cosmic-text` is `43.5 px`, leaving `+8.5 px` of dead space on the right inside hug auto-layout buttons (`left_gap = 13.0 px` vs `right_gap = 21.33 px`). In addition, `designos_create_node` (`crates/designos-mcp/src/tools/mod.rs:33-53`), `designos_apply_batch` `Insert` (`crates/designos-mcp/src/tools/batch.rs:84-96`), `build_updated_node` (`crates/designos-mcp/src/tools/mod.rs:288-334`), and `apply_set_style_op` (`crates/designos-mcp/src/tools/batch.rs:365-391`) either defaulted text nodes to `100 × 100` or mutated `tp.content` without re-measuring `node.size`.
- **What Changed (`FACT`):**
  1. Added `crates/designos-doc/src/text_measure.rs` with `pub trait TextMeasure: Send + Sync`, `OnceLock<&'static dyn TextMeasure>`, `install`, `is_installed`, `intrinsic_text_size` (falling back to `estimate_intrinsic_text_size` when no measurer is installed), and `uses_fallback_font`.
  2. Added `pub fn measure_text_box(tp: &TextProps, wrap_width: Option<f32>) -> Vec2` and `pub struct RenderTextMeasure` in `crates/designos-render/src/text.rs` (and re-exported from `crates/designos-render/src/lib.rs`), shaping against bundled Inter (`cosmic-text`), rounding up to `0.25 px` with a `16.0 px` minimum width for empty strings, stripping fill colors on the temporary `TextProps` so `GEOMS` caches purely by `(content, typography, wrap_width)`, and loading host system fonts lazily so cold CLI runs using bundled Inter do not scan `/System/Library/Fonts`.
  3. Replaced `estimate_intrinsic_text_size` calls with `crate::text_measure::intrinsic_text_size` in `crates/designos-doc/src/spec.rs` (text nodes and generated button/frame child labels) and `crates/designos-doc/src/spec_instance.rs` (`overrides[].text` and `props` text property bindings), and recorded fallback font usage in `SpecApplyReport::measured_with_fallback` with a `"TEXT_MEASURED_WITH_FALLBACK_FONT"` warning.
  4. Installed `designos_render::RenderTextMeasure` at startup in `crates/designos-app/src/main.rs`, `crates/designos-app/src/shell/tests.rs` (`Sim::with`), and `crates/designos-mcp/src/engine.rs` (`Engine::default()`).
  5. Updated `designos_create_node`, `designos_apply_batch` (`Insert` and `SetStyle`), and `designos_update_node` (`build_updated_node`) to size hug and fixed-width text nodes via `intrinsic_text_size`, re-layout ancestor frames, and report fallback font warnings.

---

## 2. Spec Drift vs `6f0c3bb2`

| Location in Spec (`1154-nodespec-text-measured-not-estimated.md`) | Spec Citation | Actual on `6f0c3bb2` | Resolution |
|---|---|---|---|
| `crates/designos-doc/src/spec.rs` | `spec.rs:1129-1194` (`estimate_intrinsic_text_size`), `spec.rs:820-835` (text node sizing), `spec.rs:902-921` (child label sizing) | `spec.rs:1166-1231`, `spec.rs:857-875`, `spec.rs:939-958` | Applied at the actual line ranges on `6f0c3bb2`. |
| `crates/designos-doc/src/spec_instance.rs` | `spec_instance.rs:342-375` (`overrides[].text`), `spec_instance.rs:412-438` (`props` text bindings) | `spec_instance.rs:333-364`, `spec_instance.rs:419-443` | Applied at both instance text override sites and added `relayout_from_text_parent` when `auto_resize == Height`. |
| `crates/designos-render/src/text.rs` | Calls `layout_text(tp, wrap_width)` | Function is named `text_geometry(props, size)` at `text.rs:774` | `measure_text_box` calls `text_geometry(&p, size)` with `Vec2::ZERO` (`WidthAndHeight`) or `Vec2::new(w, 0.0)` (`Height`). |
| Test 1 & Test 9 (`"Delete"` at 14px Semibold Inter) | Spec prose estimated `< 42.0 px` (`~40 px`) | `estimate_intrinsic_text_size` returns `52.0 px`; `cosmic-text` (`measure_text_box`) shapes 14px Semibold Inter `"Delete"` at `43.50 px` (`41.75 px` at weight 400 vs `43.50 px` at weight 600; `52.0 - 8.5 = 43.50 px`) | Asserted `(label.size.x - measured.x).abs() <= 0.5` and `label.size.x < 46.0` (which fails in RED at `52.0` and passes in GREEN at `43.5`). |
| Test 7 (`no_measurer_keeps_the_estimate`) | Spec test wrote `assert_eq!(s.x, 48.0)` (`6 * 14 * 0.56 = 47.04` without weight factor or `+ 2.0` pad) | `estimate_intrinsic_text_size` on `6f0c3bb2` applies `weight_factor = 1.07` for `font_weight >= 600` plus `+ 2.0` pad: `(6 * 14 * 0.56 * 1.07 + 2.0).ceil() = 52.0` | Asserted `Vec2::new(52.0, 17.0)`, the exact return value of `estimate_intrinsic_text_size(&tp, None)` on `6f0c3bb2`. |
| Test 8 (`fallback_font_is_reported`) | Passed `"font_family": "NonExistentFont_1154"` in `NodeSpec` JSON | `NodeSpec` had `#[serde(deny_unknown_fields)]` and did not have a top-level `font_family` field | Added `pub font_family: Option<String>` to `NodeSpec` (applied in `apply_text_overrides`, with `#[cfg_attr(feature = "schema", schemars(skip))]` to keep `generated/rules.md` unchanged). |

---

## 3. Changed Test Assertions (Controller Ruling #3)

- **`crates/designos-doc` (`634` unit tests):** `0` existing assertions changed (`designos-doc` unit tests run without `RenderTextMeasure` installed, preserving `estimate_intrinsic_text_size` per Controller Ruling #1).
- **`crates/designos-render` (`56` unit tests):** `0` existing assertions changed.
- **`crates/designos-mcp` (`252` unit tests + `5` snapshot tests):** `0` existing assertions changed.
- **`crates/designos-app` (`3,346` unit tests in `--bin designos`):** `0` existing assertions changed (`3,334` passed on the full run; the single failure was known PTY flake `#1211` `shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one`, which passed 3/3 times when rerun alone).

---

## 4. AC-02 Dogfood Render & Button Label Gap Measurements (Before vs After)

- **Spec file:** `plans/reports/agy-1154-nodespec-text-measure-evidence/buttons-spec.json` (three hug-width auto-layout buttons with `padding: [0, 12, 0, 12]`, `height: 28`, 14px Semibold Inter labels `"Delete"`, `"Export"`, `"Share"`).
- **Measurement script:** `scripts/measure_button_label_gaps.py` (measures button outer edges and 50%-coverage glyph ink bounds at 300% zoom / `3.0x` scale).
- **Screenshots:**
  - Before (`6f0c3bb2`): `plans/reports/agy-1154-nodespec-text-measure-evidence/buttons-before.png`
  - After: `plans/reports/agy-1154-nodespec-text-measure-evidence/buttons-after.png`

| Button Label | Metric (1x px) | Before (`6f0c3bb2`) | After Fix | Target |
|---|---|---:|---:|---|
| **`Delete`** | Button width | `76.00` | `67.33` | — |
| | Left gap | `13.00` | `13.00` | — |
| | Right gap | `21.33` | `12.67` | — |
| | **Gap diff (`\|L - R\|`)** | **`8.33`** | **`0.33`** | **`<= 0.50` (PASS)** |
| **`Export`** | Button width | `80.00` | `68.33` | — |
| | Left gap | `13.00` | `12.67` | — |
| | Right gap | `24.00` | `12.33` | — |
| | **Gap diff (`\|L - R\|`)** | **`11.00`** | **`0.34`** | **`<= 0.50` (PASS)** |
| **`Share`** | Button width | `71.00` | `63.00` | — |
| | Left gap | `12.67` | `12.67` | — |
| | Right gap | `20.33` | `12.33` | — |
| | **Gap diff (`\|L - R\|`)** | **`7.66`** | **`0.34`** | **`<= 0.50` (PASS)** |

---

## 5. AC-03 156-Node Board Timing (Before vs After)

- **Fixture:** `plans/reports/agy-1154-nodespec-text-measure-evidence/board-156-spec.json` (self-created 156-node board spec -> `157` document nodes including `Page 1`; zero client files used).
- **Evidence files:** `plans/reports/agy-1154-nodespec-text-measure-evidence/timing-before.json` and `plans/reports/agy-1154-nodespec-text-measure-evidence/timing-after.json`.

| Build | Runs (ms) | Median (ms) | Mean (ms) | Ratio vs Before | Target |
|---|---|---:|---:|---:|---|
| **Before (`6f0c3bb2`)** | `[27.98, 27.29, 28.70, 24.72, 28.88]` | `27.98` | `27.51` | `1.00×` | — |
| **After Fix** | `[48.14, 61.40, 42.82, 46.99, 47.31]` | `47.31` | `49.33` | **`1.69×`** | **`<= 2.00×` (`<= 55.96 ms`, PASS)** |

---

## 6. Acceptance Criteria & Gate Evidence

- **Red/Green logs:**
  - `plans/reports/agy-1154-nodespec-text-measure-evidence/red.log`: 8 tests fail against `6f0c3bb2` (`delete_button_label_has_equal_side_padding`, `semibold_measures_wider_than_regular`, `fixed_width_text_wraps_to_measured_height`, `instance_text_override_resizes_hug_pill`, `create_node_and_batch_insert_measure_text`, `update_node_and_set_style_remeasure_hug_text`, `fallback_font_is_reported`, `apply_spec_is_one_undo_step_and_one_patch`); `no_measurer_keeps_the_estimate` passes.
  - `plans/reports/agy-1154-nodespec-text-measure-evidence/green.log`: All 9 spec tests pass (`ok`).

### Gate Commands & Final Outputs

#### 1. `cargo fmt --all -- --check`
```text
$ cargo fmt --all -- --check
(exit code 0)
```

#### 2. `cargo clippy --workspace --all-targets --locked -- -D warnings`
```text
    Checking designos-render v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1154-nodespec-text-measure/crates/designos-render)
    Checking designos-svg v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1154-nodespec-text-measure/crates/designos-svg)
    Checking designos-export v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1154-nodespec-text-measure/crates/designos-export)
    Checking designos-gpu v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1154-nodespec-text-measure/crates/designos-gpu)
    Checking egui-wgpu v0.35.0
    Checking designos-webcapture v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1154-nodespec-text-measure/crates/designos-webcapture)
    Checking eframe v0.35.0
    Checking designos-fig v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1154-nodespec-text-measure/crates/designos-fig)
    Checking designos-library v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1154-nodespec-text-measure/crates/designos-library)
    Checking designos-eval v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1154-nodespec-text-measure/crates/designos-eval)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 29s
(exit code 0)
```

#### 3. `cargo test --workspace --locked --lib`
```text
designos-diff:       test result: ok. 13 passed; 0 failed; 0 ignored
designos-doc:        test result: ok. 634 passed; 0 failed; 0 ignored
designos-eval:       test result: ok. 11 passed; 0 failed; 0 ignored
designos-export:     test result: ok. 87 passed; 0 failed; 0 ignored
designos-fig:        test result: ok. 232 passed; 0 failed; 0 ignored
designos-fonts:      test result: ok. 38 passed; 0 failed; 0 ignored
designos-gpu:        test result: ok. 142 passed; 0 failed; 0 ignored
designos-library:    test result: ok. 44 passed; 0 failed; 0 ignored
designos-mcp:        test result: ok. 252 passed; 0 failed; 0 ignored
designos-render:     test result: ok. 56 passed; 0 failed; 2 ignored
designos-svg:        test result: ok. 1 passed; 0 failed; 0 ignored
designos-webcapture: test result: ok. 8 passed; 0 failed; 0 ignored
(exit code 0)
```

#### 4. `cargo test -p designos-app --bin designos --locked` (+ 3× Rerun of Known Flake `#1211`)
```text
test result: FAILED. 3334 passed; 1 failed; 11 ignored; 0 measured; 0 filtered out; finished in 1125.05s
failures:
    shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one

=== Rerun 1 ===
running 1 test
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3345 filtered out; finished in 0.70s

=== Rerun 2 ===
running 1 test
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3345 filtered out; finished in 0.86s

=== Rerun 3 ===
running 1 test
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3345 filtered out; finished in 0.70s
```

#### 5. `python3 scripts/zero_fake_ui_gate.py` & `python3 scripts/check_spacing_relationships.py`
```text
Gate Verdict: PASS (17/17 invariants satisfied)
literal geometry outside theme.rs: offset_literals=1668, inset_literals=45, radius_literals=257, font_literals=25, nonconcentric_pairs=2
note: 14 entries are below baseline; run --write-baseline to lock the gain in
PASS: no count rose above the baseline
```

---

## 7. Files Modified & Files Outside Scope

### Files Modified
- `crates/designos-doc/src/lib.rs`
- `crates/designos-doc/src/text_measure.rs`
- `crates/designos-doc/src/spec.rs`
- `crates/designos-doc/src/spec_bind.rs`
- `crates/designos-doc/src/spec_bind_reference_forms_tests.rs`
- `crates/designos-doc/src/spec_instance.rs`
- `crates/designos-render/src/lib.rs`
- `crates/designos-render/src/text.rs`
- `crates/designos-app/src/main.rs`
- `crates/designos-app/src/shell/tests.rs`
- `crates/designos-mcp/src/engine.rs`
- `crates/designos-mcp/src/tools/mod.rs`
- `crates/designos-mcp/src/tools/batch.rs`
- `scripts/measure_button_label_gaps.py`
- `plans/reports/agy-1154-nodespec-text-measure.md`
- `plans/reports/agy-1154-nodespec-text-measure-evidence/*`

### Files Outside Scope
- None.
