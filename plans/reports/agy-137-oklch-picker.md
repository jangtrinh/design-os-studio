# AGY-137 — OKLCH and OKLAB Colour Modes in the Fill Picker (`#137`)

- **Issue:** `#137` — Colour picker: OKLCH and OKLAB colour modes
- **Branch:** `jangtrinh/agy-137-oklch-picker`
- **Worktree:** `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-137-oklch-picker`

---

## 1. What Could Not Be Verified (Stated First)

1. **Live Figma Desktop interactive runtime:** Per hard rules, Figma Desktop was not opened during the run; Figma UI3 single-cell OKLCH/OKLAB formatting and input conventions follow the controller's rulings in `/Users/jangtrinh/Products/design-os-studio/plans/261030-2041-e1-wave-1/brief-agy-137-r2.txt` and `/Users/jangtrinh/Products/design-os-studio/plans/reports/re-figma-261031-color-modes-typography-binds.md` (§A1–A2).
2. **Real-app windowed OS capture:** Per ruling 7 ("The real-app GUI captures are the controller's job after you finish"), rendered verification in this worker was performed via the deterministic headless 2x CPU-raster proof-sheet pipeline (`plans/reports/agy-137-oklch-picker-evidence/r2-picker-modes.png`), visual inspection of the rendered PNG, and automated visible-text (`col.a() > 0`) assertions across all 7 picker modes.
3. **Round 1 cell elision disclosure (found by review):** In Round 1 (`f82be3e0`), `collect_texts` did not filter `egui::Shape::Text` by `col.a() > 0` and therefore matched `field::edit_text`'s `Color32::TRANSPARENT` `TextEdit` galley while the visible glyphs painted at rest by `field_text` were elided (`"O…"`, `"6…"`, `"0…"`, `"2…"` in OKLCH/OKLAB, and `"2…"`, `"1…"` in RGB/HSL/HSB). Fixed and verified in [`## Round 2`](#round-2) below.

---

## 2. Skills Loaded & Changes Driven by Them

- **`/es:dev` -> `es:feature-dev`** (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`): Executed directly in the DEV phase against the accepted plan and controller rulings without re-opening brainstorm/plan gates; verified every acceptance criterion with deterministic test and gate artifacts before reporting.
- **`/es:rust`** (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`): Kept `color_space.rs` conversions pure, allocation-free `[f32; 3]` / `[u8; 3]` value transformations (`#[inline]`), and avoided nested `ctx.data_mut` lock hazards in `picker.rs` tests and proof-sheet rendering.
- **`/es:rust-gpu-canvas`** (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`): Preserved `Paint::Solid(Color)` 8-bit sRGB storage and `FrameKey` invariants so picker edits do not allocate textures or alter GPU scene compilation.
- **`es:designer`** (`/Users/jangtrinh/.gemini/config/skills/es-designer/SKILL.md`): Applied Operate-mode craft floor to the 208 px picker value row — measured every mode's worst-case label/value strings against `theme::font_body_strong()` and `theme::font_body()`, sized `picker-fmt` to `"OKLCH"` (`66.0` px), switched `Fmt::Oklch` / `Fmt::Oklab` to a full-width `138.0` px single cell (`132.0` px inner text budget), and verified zero `'…'` truncation on the 2x CPU-raster proof sheet (`r2-picker-modes.png`).
- **`es:astra-work` & `es:fable-thinking`** (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`): Enforced RED-before-GREEN verification on `f82be3e0`, falsification via the 20-mutant harness (`17/20 KILLED`), and an explicit epistemic ledger (`FACT` / `INFERENCE` / `ASSUMPTION`).

---

## 3. Round 1 (Rewritten from Actual Code at `f82be3e0`)

### 3.1 Implementation Summary (`f82be3e0`)

1. **`crates/designos-doc/src/color_space.rs` (re-exported via `crates/designos-doc/src/lib.rs`):**
   - Implemented `srgb_to_oklab`, `oklab_to_srgb`, `oklab_to_oklch`, `oklch_to_oklab`, `srgb_to_oklch`, `oklch_to_srgb`, `srgb_u8_to_oklab`, `oklab_to_srgb_u8`, `srgb_u8_to_oklch`, and `oklch_to_srgb_u8` using Björn Ottosson's forward/inverse LMS cube-root matrices and IEC 61966-2-1 sRGB transfer functions (`srgb_to_linear`, `linear_to_srgb`).
   - Added an exact neutral-grey shortcut in `srgb_to_oklab` (`if (r_lin - g_lin).abs() <= f32::EPSILON && (g_lin - b_lin).abs() <= f32::EPSILON`) so achromatic inputs return `a = 0.0, b = 0.0` (`C = 0.0`).
   - Added an achromatic hue guard in `oklab_to_oklch`: when `C < 1e-4`, returns `H = 0.0`; otherwise wraps `b.atan2(a).to_degrees().rem_euclid(360.0)` into `[0.0, 360.0)`.
2. **`crates/designos-app/src/widgets/picker.rs`:**
   - Extended `Fmt` with `Fmt::Oklch` (`"OKLCH"`) and `Fmt::Oklab` (`"OKLAB"`), with `Fmt::ALL = [Fmt::Hex, Fmt::Rgb, Fmt::Css, Fmt::Hsl, Fmt::Hsb, Fmt::Oklch, Fmt::Oklab]`.
   - Added `pub oklch: [f32; 3]` and `pub oklab: [f32; 3]` to `picker::State`, plus `State::sync_ok`, `State::set_oklch_channel`, `State::set_oklab_channel`, `fmt_ok_channel`, and `parse_ok_channel`:
     - `fmt_ok_channel(ch, val)` formats channel `0` (`L`) as `fmt_num(val * 100.0)` and channels `1`/`2` as `fmt_num(val)`.
     - `parse_ok_channel(fmt, ch, input, cur)` parses:
       - `L` (`ch == 0` in `Fmt::Oklch | Fmt::Oklab`): strips optional trailing `'%'`, evaluates via `field::eval_number_expr(body, Some(cur * 100.0), false)`, divides by `100.0`, and clamps to `0.0..=1.0`.
       - `C` (`(Fmt::Oklch, 1)`): if trailing `'%'` is present, evaluates against `Some(cur / 0.004)` and scales by `0.004` (`100% -> 0.4`); otherwise evaluates raw against `Some(cur)`; clamps to `0.0..=0.4`.
       - `H` (`(Fmt::Oklch, 2)`): rejects `'%'`, evaluates via `field::eval_number_expr` against `Some(cur)`, wraps via `rem_euclid(360.0)` into `0.0..360.0`.
       - `a, b` (`(Fmt::Oklab, 1 | 2)`): if trailing `'%'` is present, evaluates against `Some(cur / 0.004)` and scales by `0.004` (`±100% -> ±0.4`); otherwise evaluates raw against `Some(cur)`; clamps to `-0.4..=0.4`.
   - **Corrected re-anchor table (`State::sync_ok`, `State::set_oklch_channel`, `State::set_oklab_channel`):**

| Entry point | Behaviour on `self.oklch` and `self.oklab` |
|---|---|
| `State::sync_ok(&mut self, rgb: [u8; 3])` (called from `State::sync` and once per frame in `show`) | Independently checks `oklch_to_srgb_u8(self.oklch[0], self.oklch[1], self.oklch[2]) != rgb` and `oklab_to_srgb_u8(self.oklab[0], self.oklab[1], self.oklab[2]) != rgb`, re-deriving whichever triple no longer quantizes to `rgb` (while preserving `self.oklch[2]` when `lch[1] < 1e-4`). |
| `State::set_oklch_channel(&mut self, i: usize, n: f32) -> [u8; 3]` | Updates `self.oklch[i] = n`, derives `self.oklab` via `oklch_to_oklab` (clamped), and returns `oklch_to_srgb_u8`. |
| `State::set_oklab_channel(&mut self, i: usize, n: f32) -> [u8; 3]` | Updates `self.oklab[i] = n`, derives `self.oklch` via `oklab_to_oklch` (preserving `self.oklch[2]` when `lch[1] < 1e-4`), and returns `oklab_to_srgb_u8`. |

### 3.2 Round 1 Evidence Files (`plans/reports/agy-137-oklch-picker-evidence/`)

- `red-color_space.log`, `red-picker.log`
- `green-color_space.log`, `green-picker.log`
- `bin-layout_unchanged.log`, `bin-effect_stop_binds.log`, `bin-parses_panel.log`, `workspace-lib.log`
- `fmt.log`, `clippy.log`, `chrome_colour.log`, `spacing.log`, `zero_fake_ui.log`, `scripts-tests.log`

---

## Round 2

### 4. Summary of Round 2 Fixes

1. **Ruling 1 (`B1` — Single cell for OKLCH/OKLAB):**
   - In `Fmt::Oklch` and `Fmt::Oklab`, replaced the 3 narrow channel cells + opacity cell with a single full-width `grouped_text` cell (`"picker-ok"`, `fields.width() = 138.0` px, `pad = 3.0` px -> `132.0` px inner text budget), matching `Fmt::Css`. The opacity cell leaves the row in `Fmt::Oklch` and `Fmt::Oklab`, as in `Fmt::Css`.
   - Added `fmt_ok_token(ch, val)` and `State::ok_text(&self, fmt: Fmt, alpha: u8) -> String`:
     - Formats `L` with a `%` suffix (`62.8%`), `C`/`a`/`b` raw (`0.26`, `0.22`, `0.13`), and `H` in degrees with no unit (`29.23`), 2 decimals with trailing zeros dropped (`fmt_num`).
     - When `alpha < 255`, appends ` / <a>` where `a` is `alpha / 255.0` rounded to 2 decimals (`62.8% 0.26 29.23 / 0.5` and `62.8% 0.22 0.13 / 0.5`).
   - Added `State::apply_ok_input(&mut self, fmt: Fmt, input: &str, keep_alpha: u8) -> Option<[u8; 4]>`:
     - Accepts 3 space/comma-separated tokens parsed with `parse_ok_channel` rules (clamps, `%`, relative `+=` / `-=`), plus an optional `/ a` (`a` in `0..=1` or as a percent `0%..=100%`), as well as pasted `oklch(...)` or `oklab(...)` strings (`deg` and `°` included).
     - Preserves R6 (`st.oklch` / `st.oklab` full `f32` precision on unedited tokens and grey hue memory when `C == 0`).
2. **Ruling 2 (Every value of every picker mode readable at rest):**
   - Sized the closed format dropdown (`picker-fmt`) dynamically to the longest label in `Fmt::ALL` (`"OKLCH"`, `41.91` px in `theme::font_body_strong()`): `dd_w = (max_label_w + 6.0 + 18.0).ceil() = 66.0` px (`42.0` px text budget).
   - Tightened `picker_dropdown` inner padding to `6.0` px left inset and `18.0` px right chevron reserve (`chevron` centered at `rect.right() - 10.0`).
   - Recomputed `fields` at `dd.right() + 4.0` (`width = 138.0` px):
     - Single-cell modes (`Fmt::Css`, `Fmt::Oklch`, `Fmt::Oklab`): `138.0` px cell width with `pad = 3.0` px -> `132.0` px text budget (fits `"62.8% 0.26 29.23 / 0.5"` at `130.78` px and `"rgba(255, 255, 255, 1)"` at `123.72` px).
     - Multi-cell modes (`Fmt::Hex`, `Fmt::Rgb`, `Fmt::Hsl`, `Fmt::Hsb`): opacity cell `op_w = 43.0` px (`pad_x = 4.0` px, `text_r` right inset `14.0` px -> `21.0` px number text budget, fitting `"100"` at `20.03` px), channel group `ch_group_w = 94.0` px:
       - `Fmt::Hex`: `94.0` px cell (`pad = 8.0` px -> `78.0` px text budget, fitting `"FF0000"` at `44.44` px).
       - `Fmt::Rgb | Fmt::Hsl | Fmt::Hsb`: 3 cells of `w3 = (94.0 - 2.0) / 3.0 = 30.67` px with `pad = 3.0` px -> `24.67` px text budget (fitting `"360"` at `22.44` px, `"255"` at `21.56` px, `"100"` at `20.03` px).
3. **Ruling 3 (Visible-text tests):**
   - Updated `collect_visible_texts` / `visible_texts` to filter `egui::Shape::Text` by `col.a() > 0`.
   - Updated `red_shows_expected_oklch_and_oklab_cells` (which failed on `f82be3e0`) and added `visible_text_hex_mode`, `visible_text_rgb_at_255_255_255`, `visible_text_hsl_mode`, `visible_text_hsb_at_full_s_b`, `visible_text_css_mode`, `visible_text_oklch_mode`, and `visible_text_oklab_mode`.
4. **Ruling 4 (`m1` — Missing mutant-killing tests & `color_space.rs` tolerance disclosure):**
   - Added unit tests killing `M09` (`a 50% -> 0.2`, `b -50% -> -0.2`), `M10` (`C +=10%` from `0.1 -> 0.14`), `M11` (red in OKLCH then `st.sync([128,128,128,255])` keeps `H = 29.23`), `M12` (`set_oklab_channel` `a=0` then `b=0` keeps `H = 90`), and `M02` / `M20` (17-step `4,913`-colour u8-exact round-trip + `1e-4` `f32` tolerance).
   - **Disclosure:** In `crates/designos-doc/src/color_space.rs` (`known_values_srgb_to_oklab_and_oklch`), tightened the red `H` assertion on `srgb_to_oklch(1.0, 0.0, 0.0)[2]` from `29.23` at `1e-2` to `29.233_9` at `1e-3`.
5. **Ruling 5 (`m3` — `color_space.rs` `+inf` and huge `OKLab L` clamping):**
   - Updated `sanitize_unit(v)` so `+inf` maps to `1.0` (`if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) }`).
   - Updated `oklab_to_srgb` to clamp `L` to `[0.0, 1.0]` via `sanitize_unit(l)` before cubing.
   - Added `positive_infinity_srgb_and_huge_oklab_l_clamp_to_white` asserting `srgb_to_oklab(+inf, +inf, +inf) == [1.0, 0.0, 0.0]` and `oklab_to_srgb(1e13, 0.0, 0.0) == [1.0, 1.0, 1.0]`.

### 5. Before / After Measured Widths (`AREA = 208.0` px)

Measured with `egui` font layout (`theme::font_body_strong()` for dropdown labels; `theme::font_body()` for value cells):

| Element / String | Measured Text Width (px) | Before (`f82be3e0`) Box / Budget (px) | Before Result | After (Round 2) Box / Budget (px) | After Result |
|---|---:|---:|---|---:|---|
| `picker-fmt` `"OKLCH"` (longest in `Fmt::ALL`) | `41.91` | `55.0` box / `25.0` text (`pad 8+22`) | Elided (`"O…"`) | `66.0` box / `42.0` text (`pad 6+18`) | Fits (`"OKLCH"`) |
| `picker-fmt` `"OKLAB"` | `41.09` | `55.0` box / `25.0` text | Elided (`"O…"`) | `66.0` box / `42.0` text | Fits (`"OKLAB"`) |
| `Fmt::Hex` `"FF0000"` | `44.44` | `89.0` box / `73.0` text (`pad 8`) | Fits (`"FF0000"`) | `94.0` box / `78.0` text (`pad 8`) | Fits (`"FF0000"`) |
| `Fmt::Rgb` `"255"` (x3) | `21.56` | `29.0` box / `19.0` text (`pad 5`) | Elided (`"2…"`) | `30.67` box / `24.67` text (`pad 3`) | Fits (`"255"`) |
| `Fmt::Hsl` / `Fmt::Hsb` `"100"` | `20.03` | `29.0` box / `19.0` text (`pad 5`) | Elided (`"1…"`) | `30.67` box / `24.67` text (`pad 3`) | Fits (`"100"`) |
| `Fmt::Hsb` `"360"` | `22.44` | `29.0` box / `19.0` text (`pad 5`) | Elided (`"3…"`) | `30.67` box / `24.67` text (`pad 3`) | Fits (`"360"`) |
| Opacity cell `"100"` (+ `"%"` `11.78` px) | `20.03` | `54.0` box / `24.0` text (`inset 14, pad 8`) | Fits (`"100"`) | `43.0` box / `21.0` text (`inset 14, pad 4`) | Fits (`"100"`) |
| `Fmt::Css` `"rgba(255, 255, 255, 1)"` | `123.72` | `144.0` box / `128.0` text (`pad 8`) | Fits | `138.0` box / `132.0` text (`pad 3`) | Fits |
| `Fmt::Oklch` `"62.8% 0.26 29.23"` | `102.03` | 3x `29.0` boxes / `19.0` text each | Elided (`"6…"`, `"0…"`, `"2…"`) | `138.0` single cell / `132.0` text (`pad 3`) | Fits (`"62.8% 0.26 29.23"`) |
| `Fmt::Oklch` `"62.8% 0.26 29.23 / 0.5"` | `130.78` | N/A (separate opacity cell) | Elided | `138.0` single cell / `132.0` text (`pad 3`) | Fits (`"62.8% 0.26 29.23 / 0.5"`) |
| `Fmt::Oklab` `"62.8% 0.22 0.13"` | `91.25` | 3x `29.0` boxes / `19.0` text each | Elided (`"6…"`, `"0…"`, `"0…"`) | `138.0` single cell / `132.0` text (`pad 3`) | Fits (`"62.8% 0.22 0.13"`) |
| `Fmt::Oklab` `"62.8% 0.22 0.13 / 0.5"` | `120.00` | N/A (separate opacity cell) | Elided | `138.0` single cell / `132.0` text (`pad 3`) | Fits (`"62.8% 0.22 0.13 / 0.5"`) |

---

### 6. Acceptance Table & Pasted Outputs

| # | Acceptance Criterion | Verdict | Evidence File |
|---|---|---|---|
| **1** | Each new test fails on `f82be3e0` (red run) and passes on head (green run) | **PASS** (`1 failed -> 4 passed` in `designos-doc`; `11 failed -> 21 passed` in `picker.rs`) | `r2-red-doc.log`, `r2-red-picker.log`, `r2-green-doc.log`, `r2-green-picker.log` |
| **2** | Mutants M02, M09, M10, M11, M12 killed; the 11 already-killed ones stay killed | **PASS** (`17/20 KILLED`: M01–M05, M07–M17, M20) | `r2-mutants.log` |
| **3** | Headless 2x CPU-raster proof sheet (`r2-picker-modes.png`) + visible strings per mode | **PASS** (zero `'…'` in any mode; open dropdown shows all 7 labels) | `r2-picker-modes.png`, `r2-visible-strings.log` |
| **4a** | `cargo fmt --all -- --check` exits 0 | **PASS** (`exit=0`) | `r2-gates.log` |
| **4b** | `cargo clippy --workspace --all-targets --locked -- -D warnings`: only `main`'s known errors (`agent_permissions.rs:1163,1181`, `panels/components.rs` items_after_test_module / unused run_ui / ptr_arg) | **PASS** (0 errors/warnings in `designos-doc` or `picker.rs`; only `main`'s 5 known errors) | `r2-gates.log` |
| **4c** | `cargo test -p designos-doc --locked --lib` passes | **PASS** (`710 passed; 0 failed`) | `r2-gates.log` |
| **4d** | `cargo test -p designos-app --bin designos --locked picker` passes | **PASS** (`92 passed; 0 failed`) | `r2-gates.log` |
| **4e** | `… layout_unchanged` passes, or each moved baseline is a picker cell, listed with its old and new rect | **PASS** (`19/20` suites pass; in `layout_unchanged_f`, only the 3 unnamed picker value-row cells moved — listed below) | `r2-gates.log` |
| **4f** | `cargo test --workspace --locked --lib` passes | **PASS** (all workspace lib crates `0 failed`) | `r2-gates.log` |
| **4g** | `python3 scripts/check_chrome_colour_literals.py` and `python3 scripts/zero_fake_ui_gate.py` pass | **PASS** (`199/199` baseline; `17/17` invariants) | `r2-gates.log` |

#### Acceptance 1 — RED on `f82be3e0` and GREEN on Head

```text
$ cargo test -p designos-doc --locked --lib color_space  # on f82be3e0 (r2-red-doc.log)
running 4 tests
test color_space::tests::known_values_srgb_to_oklab_and_oklch ... ok
test color_space::tests::gamut_clamping_and_non_finite_inputs ... ok
test color_space::tests::positive_infinity_srgb_and_huge_oklab_l_clamp_to_white ... FAILED
test color_space::tests::round_trip_17_step_srgb_grid_4913_colours ... ok

failures:

---- color_space::tests::positive_infinity_srgb_and_huge_oklab_l_clamp_to_white stdout ----

thread 'color_space::tests::positive_infinity_srgb_and_huge_oklab_l_clamp_to_white' (48291581) panicked at crates/designos-doc/src/color_space.rs:241:9:
assertion `left == right` failed: srgb_to_oklab(+inf, +inf, +inf) must be white
  left: [0.0, 0.0, 0.0]
 right: [1.0, 0.0, 0.0]

test result: FAILED. 3 passed; 1 failed; 0 ignored; 0 measured; 674 filtered out; finished in 0.00s
```

```text
$ cargo test -p designos-app --bin designos --locked widgets::picker::tests  # on f82be3e0 (r2-red-picker.log)
running 21 tests
test widgets::picker::tests::solid_to_gradient_keeps_color_as_first_stop ... ok
test widgets::picker::tests::oklch_and_oklab_format_list_and_default ... ok
test widgets::picker::tests::gradient_to_solid_takes_first_stop ... ok
test widgets::picker::tests::active_color_edits_the_selected_stop ... ok
test widgets::picker::tests::hsb_is_computed_in_gamma_space ... ok
test widgets::picker::tests::gradient_shape_switch_keeps_stops ... ok
test widgets::picker::tests::stop_color_interpolates ... ok
test widgets::picker::tests::hue_survives_grey ... ok
test widgets::picker::tests::procedural_png_presets_generate_valid_png_headers_and_distinct_textures ... ok
test widgets::picker::tests::css_and_hsl_roundtrip ... ok
test widgets::picker::tests::visible_text_hsb_at_full_s_b ... FAILED
test widgets::picker::tests::visible_text_rgb_at_255_255_255 ... FAILED
test widgets::picker::tests::visible_text_hsl_mode ... FAILED
test widgets::picker::tests::visible_text_hex_mode ... FAILED
test widgets::picker::tests::visible_text_css_mode ... FAILED
test widgets::picker::tests::parse_oklch_and_oklab_channel_rules_and_clamps ... FAILED
test widgets::picker::tests::editing_oklch_and_oklab_channels_commits_srgb_rgba ... FAILED
test widgets::picker::tests::red_shows_expected_oklch_and_oklab_cells ... FAILED
test widgets::picker::tests::oklch_state_preserves_typed_values_and_grey_hue ... FAILED
test widgets::picker::tests::visible_text_oklch_mode ... FAILED
test widgets::picker::tests::visible_text_oklab_mode ... FAILED

failures:

---- widgets::picker::tests::visible_text_hsb_at_full_s_b stdout ----
thread 'widgets::picker::tests::visible_text_hsb_at_full_s_b' (48317941) panicked at crates/designos-app/src/widgets/picker.rs:1964:9:
mode Hsb painted an elided string in visible texts: ["Custom", "Libraries", "4.00 : 1", "AA Large", "HSB", "0", "1…", "1…", "100", "%", "On this page"]

---- widgets::picker::tests::visible_text_rgb_at_255_255_255 stdout ----
thread 'widgets::picker::tests::visible_text_rgb_at_255_255_255' (48317945) panicked at crates/designos-app/src/widgets/picker.rs:1964:9:
mode Rgb painted an elided string in visible texts: ["Custom", "Libraries", "16.67 : 1", "AAA", "RGB", "2…", "2…", "2…", "100", "%", "On this page"]

---- widgets::picker::tests::visible_text_hsl_mode stdout ----
thread 'widgets::picker::tests::visible_text_hsl_mode' (48317942) panicked at crates/designos-app/src/widgets/picker.rs:1964:9:
mode Hsl painted an elided string in visible texts: ["Custom", "Libraries", "4.00 : 1", "AA Large", "HSL", "0", "1…", "50", "100", "%", "On this page"]

---- widgets::picker::tests::visible_text_hex_mode stdout ----
thread 'widgets::picker::tests::visible_text_hex_mode' (48317940) panicked at crates/designos-app/src/widgets/picker.rs:1976:9:
mode Hex closed format dropdown is too narrow for longest label

---- widgets::picker::tests::visible_text_css_mode stdout ----
thread 'widgets::picker::tests::visible_text_css_mode' (48317939) panicked at crates/designos-app/src/widgets/picker.rs:1976:9:
mode Css closed format dropdown is too narrow for longest label

---- widgets::picker::tests::parse_oklch_and_oklab_channel_rules_and_clamps stdout ----
thread 'widgets::picker::tests::parse_oklch_and_oklab_channel_rules_and_clamps' (48317934) panicked at crates/designos-app/src/widgets/picker.rs:2152:9:
pasting oklch(...) must commit colour and alpha

---- widgets::picker::tests::editing_oklch_and_oklab_channels_commits_srgb_rgba stdout ----
thread 'widgets::picker::tests::editing_oklch_and_oklab_channels_commits_srgb_rgba' (48317927) panicked at crates/designos-app/src/widgets/picker.rs:2165:9:
editing OKLCH L must mark out.changed

---- widgets::picker::tests::red_shows_expected_oklch_and_oklab_cells stdout ----
thread 'widgets::picker::tests::red_shows_expected_oklch_and_oklab_cells' (48317936) panicked at crates/designos-app/src/widgets/picker.rs:1964:9:
mode Oklch painted an elided string in visible texts: ["Custom", "Libraries", "4.00 : 1", "AA Large", "O…", "6…", "0…", "2…", "100", "%", "On this page"]

---- widgets::picker::tests::oklch_state_preserves_typed_values_and_grey_hue stdout ----
thread 'widgets::picker::tests::oklch_state_preserves_typed_values_and_grey_hue' (48317933) panicked at crates/designos-app/src/widgets/picker.rs:2207:9:
assertion failed: (st.oklch[1] - 0.0).abs() < 1e-6

---- widgets::picker::tests::visible_text_oklch_mode stdout ----
thread 'widgets::picker::tests::visible_text_oklch_mode' (48317944) panicked at crates/designos-app/src/widgets/picker.rs:1964:9:
mode Oklch painted an elided string in visible texts: ["Custom", "Libraries", "4.00 : 1", "AA Large", "O…", "6…", "0…", "2…", "100", "%", "On this page"]

---- widgets::picker::tests::visible_text_oklab_mode stdout ----
thread 'widgets::picker::tests::visible_text_oklab_mode' (48317943) panicked at crates/designos-app/src/widgets/picker.rs:1964:9:
mode Oklab painted an elided string in visible texts: ["Custom", "Libraries", "4.00 : 1", "AA Large", "O…", "6…", "0…", "0…", "100", "%", "On this page"]

test result: FAILED. 10 passed; 11 failed; 0 ignored; 0 measured; 3969 filtered out; finished in 0.02s
```

```text
$ cargo test -p designos-doc --locked --lib color_space  # on head (r2-green-doc.log)
running 4 tests
test color_space::tests::known_values_srgb_to_oklab_and_oklch ... ok
test color_space::tests::gamut_clamping_and_non_finite_inputs ... ok
test color_space::tests::positive_infinity_srgb_and_huge_oklab_l_clamp_to_white ... ok
test color_space::tests::round_trip_17_step_srgb_grid_4913_colours ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 706 filtered out; finished in 0.01s
```

```text
$ cargo test -p designos-app --bin designos --locked widgets::picker::tests  # on head (r2-green-picker.log)
running 21 tests
test widgets::picker::tests::solid_to_gradient_keeps_color_as_first_stop ... ok
test widgets::picker::tests::hsb_is_computed_in_gamma_space ... ok
test widgets::picker::tests::active_color_edits_the_selected_stop ... ok
test widgets::picker::tests::gradient_to_solid_takes_first_stop ... ok
test widgets::picker::tests::gradient_shape_switch_keeps_stops ... ok
test widgets::picker::tests::oklch_and_oklab_format_list_and_default ... ok
test widgets::picker::tests::hue_survives_grey ... ok
test widgets::picker::tests::stop_color_interpolates ... ok
test widgets::picker::tests::procedural_png_presets_generate_valid_png_headers_and_distinct_textures ... ok
test widgets::picker::tests::css_and_hsl_roundtrip ... ok
test widgets::picker::tests::visible_text_oklab_mode ... ok
test widgets::picker::tests::visible_text_oklch_mode ... ok
test widgets::picker::tests::visible_text_hex_mode ... ok
test widgets::picker::tests::visible_text_rgb_at_255_255_255 ... ok
test widgets::picker::tests::visible_text_hsl_mode ... ok
test widgets::picker::tests::visible_text_css_mode ... ok
test widgets::picker::tests::visible_text_hsb_at_full_s_b ... ok
test widgets::picker::tests::parse_oklch_and_oklab_channel_rules_and_clamps ... ok
test widgets::picker::tests::red_shows_expected_oklch_and_oklab_cells ... ok
test widgets::picker::tests::oklch_state_preserves_typed_values_and_grey_hue ... ok
test widgets::picker::tests::editing_oklch_and_oklab_channels_commits_srgb_rgba ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 3971 filtered out; finished in 0.02s
```

#### Acceptance 2 — 20-Mutant Harness Output (`r2-mutants.log`)

```text
M01 matrix fwd l-row g 0.53633255->0.53733255: KILLED (doc=False, app=True)
M02 matrix inv g-row m 2.6097574->2.6107574: KILLED (doc=False, app=True)
M03 matrix fwd a-row s 0.4505937->0.4515937: KILLED (doc=False, app=True)
M04 clamp: linear sRGB clamp removed (NaN->0 kept): KILLED (doc=False, app=True)
M05 clamp: parse C plain clamp 0.4->0.5: KILLED (doc=True, app=False)
M06 clamp: sync_ok C clamp 0.4->0.5: SURVIVED (doc=True, app=True)
M07 clamp: parse L clamp removed: KILLED (doc=True, app=False)
M08 %scale: C body x0.004->x0.005: KILLED (doc=True, app=False)
M09 %scale: a/b body x0.004->x0.005: KILLED (doc=True, app=False)
M10 %scale: C relative current /0.004->/0.005: KILLED (doc=True, app=False)
M11 hue memory: sync_ok grey branch removed: KILLED (doc=True, app=False)
M12 hue memory: set_oklab_channel grey branch removed: KILLED (doc=True, app=False)
M13 R6 keep: sync_ok always re-derives OKLCH: KILLED (doc=True, app=False)
M14 decimals: 2 -> 1 decimal: KILLED (doc=True, app=False)
M15 decimals: L shown raw (no x100): KILLED (doc=True, app=False)
M16 format order: OKLCH/OKLAB swapped in ALL: KILLED (doc=True, app=False)
M17 hue wrap removed in parse: KILLED (doc=True, app=False)
M18 per-frame sync_ok in show() removed: SURVIVED (doc=True, app=True)
M19 hue 360 guard removed in oklab_to_oklch: SURVIVED (doc=True, app=True)
M20 grey shortcut removed in srgb_to_oklab: KILLED (doc=False, app=True)

SUMMARY: 17/20 killed, 3 survived
  SURVIVED: M06 clamp: sync_ok C clamp 0.4->0.5
  SURVIVED: M18 per-frame sync_ok in show() removed
  SURVIVED: M19 hue 360 guard removed in oklab_to_oklch
```

#### Acceptance 3 — Proof Sheet (`r2-picker-modes.png`) & Visible Strings (`r2-visible-strings.log`)

- Proof sheet file: `plans/reports/agy-137-oklch-picker-evidence/r2-picker-modes.png`
- Pasted visible strings (`r2-visible-strings.log`):

```text
Hex (Red #FF0000 100%): ["Custom", "Libraries", "4.00 : 1", "AA Large", "Hex", "FF0000", "100", "%", "On this page"]
RGB (Red 255,0,0 100%): ["Custom", "Libraries", "4.00 : 1", "AA Large", "RGB", "255", "0", "0", "100", "%", "On this page"]
RGB (White 255,255,255 100%): ["Custom", "Libraries", "16.67 : 1", "AAA", "RGB", "255", "255", "255", "100", "%", "On this page"]
CSS (Red rgba(255, 0, 0, 1)): ["Custom", "Libraries", "4.00 : 1", "AA Large", "CSS", "rgba(255, 0, 0, 1)", "On this page"]
CSS (White rgba(255, 255, 255, 1)): ["Custom", "Libraries", "16.67 : 1", "AAA", "CSS", "rgba(255, 255, 255, 1)", "On this page"]
HSL (Red 0,100,50 100%): ["Custom", "Libraries", "4.00 : 1", "AA Large", "HSL", "0", "100", "50", "100", "%", "On this page"]
HSB (Red 0,100,100 100%): ["Custom", "Libraries", "4.00 : 1", "AA Large", "HSB", "0", "100", "100", "100", "%", "On this page"]
HSB (360,100,100 100%): ["Custom", "Libraries", "4.00 : 1", "AA Large", "HSB", "360", "100", "100", "100", "%", "On this page"]
OKLCH (Red 62.8% 0.26 29.23): ["Custom", "Libraries", "4.00 : 1", "AA Large", "OKLCH", "62.8% 0.26 29.23", "On this page"]
OKLCH (Red 50% alpha): ["Custom", "Libraries", "4.00 : 1", "AA Large", "OKLCH", "62.8% 0.26 29.23 / 0.5", "On this page"]
OKLAB (Red 62.8% 0.22 0.13): ["Custom", "Libraries", "4.00 : 1", "AA Large", "OKLAB", "62.8% 0.22 0.13", "On this page"]
OKLAB (Red 50% alpha): ["Custom", "Libraries", "4.00 : 1", "AA Large", "OKLAB", "62.8% 0.22 0.13 / 0.5", "On this page"]
Open dropdown (OKLCH selected): ["Custom", "Libraries", "4.00 : 1", "AA Large", "OKLCH", "62.8% 0.26 29.23", "On this page", "Hex", "RGB", "CSS", "HSL", "HSB", "OKLCH", "OKLAB"]
```

#### Acceptance 4 — Gates Under the Harness (`r2-gates.log`) & Moved Picker Cell Baselines

##### Moved Picker Value-Row Baselines in `layout_unchanged_f` (`f-picker`, `f-picker-gradient`, `picker-page`)

Every moved widget in `layout_unchanged` is one of the 3 unnamed picker value-row cells (`picker-fmt` dropdown `dd`, `picker-hex` cell `hex_r`, and `picker-opacity` text rect `op_r.text_r`) in region `overlay:Foreground/0`, caused directly by Ruling 2 (`dd` `55.0 -> 66.0` px, `hex_r` `89.0 -> 94.0` px, `op_r` `54.0 -> 43.0` px / `text_r` `40.0 -> 29.0` px). Per constraints, `crates/designos-app/tests/layout-baseline/*.json` was not modified:

| Layout Baseline Case | Picker Cell | Region | Old Rect `[x, y, w, h]` | New Rect `[x, y, w, h]` |
|---|---|---|---|---|
| `f-picker` | `picker-fmt` (`dd`, unnamed `#12` in region / `#31` overall) | `overlay:Foreground/0` | `[1136.0, 883.0, 55.0, 24.0]` | `[1136.0, 883.0, 66.0, 24.0]` |
| `f-picker` | `picker-hex` (`hex_r`, unnamed `#13` in region / `#32` overall) | `overlay:Foreground/0` | `[1200.0, 883.0, 89.0, 24.0]` | `[1206.0, 883.0, 94.0, 24.0]` |
| `f-picker` | `picker-opacity` (`op_r.text_r`, unnamed `#14` in region / `#33` overall) | `overlay:Foreground/0` | `[1290.0, 883.0, 40.0, 24.0]` | `[1301.0, 883.0, 29.0, 24.0]` |
| `f-picker-gradient` | `picker-fmt` (`dd`, unnamed `#61` overall) | `overlay:Foreground/0` | `[1136.0, 883.0, 55.0, 24.0]` | `[1136.0, 883.0, 66.0, 24.0]` |
| `f-picker-gradient` | `picker-hex` (`hex_r`, unnamed `#62` overall) | `overlay:Foreground/0` | `[1200.0, 883.0, 89.0, 24.0]` | `[1206.0, 883.0, 94.0, 24.0]` |
| `f-picker-gradient` | `picker-opacity` (`op_r.text_r`, unnamed `#63` overall) | `overlay:Foreground/0` | `[1290.0, 883.0, 40.0, 24.0]` | `[1301.0, 883.0, 29.0, 24.0]` |
| `picker-page` | `picker-fmt` (`dd`, unnamed `#24` overall) | `overlay:Foreground/0` | `[1136.0, 480.0, 55.0, 24.0]` | `[1136.0, 480.0, 66.0, 24.0]` |
| `picker-page` | `picker-hex` (`hex_r`, unnamed `#25` overall) | `overlay:Foreground/0` | `[1200.0, 480.0, 89.0, 24.0]` | `[1206.0, 480.0, 94.0, 24.0]` |
| `picker-page` | `picker-opacity` (`op_r.text_r`, unnamed `#26` overall) | `overlay:Foreground/0` | `[1290.0, 480.0, 40.0, 24.0]` | `[1301.0, 480.0, 29.0, 24.0]` |

##### Pasted Gate Commands and Output (`r2-gates.log`)

```text
$ cargo fmt --all -- --check
exit=0

$ cargo clippy --workspace --all-targets --locked -- -D warnings
    Checking designos-app v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-137-oklch-picker/crates/designos-app)
error: unneeded `return` statement
    --> crates/designos-app/src/agent_permissions.rs:1163:9
error: unneeded `return` statement
    --> crates/designos-app/src/agent_permissions.rs:1181:9
error: writing `&mut Vec` instead of `&mut [_]` involves a new object where a slice will do
   --> crates/designos-app/src/panels/components.rs:613:29
error: items after a test module
    --> crates/designos-app/src/panels/components.rs:263:1
error: unused return value of `egui::Context::run_ui` that must be used
   --> crates/designos-app/src/panels/components.rs:363:9
error: could not compile `designos-app` (bin "designos-studio" test) due to 5 previous errors
error: could not compile `designos-app` (bin "designos" test) due to 5 previous errors

$ cargo test -p designos-doc --locked --lib
test result: ok. 710 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.02s

$ cargo test -p designos-app --bin designos --locked picker
test result: ok. 92 passed; 0 failed; 0 ignored; 0 measured; 3900 filtered out; finished in 7.07s

$ cargo test -p designos-app --bin designos --locked layout_unchanged
running 20 tests
test shell::tests::layout_unchanged::panel_presets::cli_accepts_panel_comments ... ok
test shell::tests::layout_unchanged::cases_json_has_no_old_fixture ... ok
test shell::tests::layout_unchanged::cases_json_and_allow_json_match_spec_l_contract ... ok
test shell::tests::layout_unchanged::new_named_widget_is_reported_not_failed ... ok
test shell::tests::layout_unchanged::unblessed_cases_file_fails_the_gate ... ok
test shell::tests::layout_unchanged::prep_helpers_populate_missing_starter_states ... ok
test shell::tests::layout_unchanged::h::layout_unchanged_h ... ok
test shell::tests::layout_unchanged::panel_presets::comments_panel_with_no_thread_opens_comment_mode_only ... ok
test shell::tests::layout_unchanged::panel_presets::comments_panel_opens_the_first_unresolved_thread_in_paint_order ... ok
test shell::tests::layout_unchanged::panel_presets::interaction_panel_without_a_reacting_selection_picks_the_first_reaction_in_paint_order ... ok
test shell::tests::layout_unchanged::panel_presets::interaction_panel_opens_the_selected_nodes_first_reaction ... ok
test shell::tests::layout_unchanged::panel_presets::interaction_panel_with_no_reaction_opens_the_prototype_tab_only ... ok
test shell::tests::layout_unchanged::sim_loads_starter_and_seed_library_at_seeds_starter_not_showcase ... ok
test shell::tests::layout_unchanged::e::layout_unchanged_e ... ok
test shell::tests::layout_unchanged::f::layout_unchanged_f ... FAILED
test shell::tests::layout_unchanged::k::layout_unchanged_k ... ok
test shell::tests::layout_unchanged::g::layout_unchanged_g ... ok
test shell::tests::layout_unchanged::d::layout_unchanged_d ... ok
test shell::tests::layout_unchanged::i::layout_unchanged_i ... ok
test shell::tests::layout_unchanged::j::layout_unchanged_j ... ok

failures:
---- shell::tests::layout_unchanged::f::layout_unchanged_f stdout ----
thread 'shell::tests::layout_unchanged::f::layout_unchanged_f' panicked at crates/designos-app/src/shell/tests/layout_unchanged/mod.rs:434:33:
case f-picker failed layout gate:
G4 unnamed widget #12 rect drift in case f-picker region 'overlay:Foreground/0': baseline=[1136.0, 883.0, 55.0, 24.0], current=[1136.0, 883.0, 66.0, 24.0]
test result: FAILED. 19 passed; 1 failed; 0 ignored; 0 measured; 3972 filtered out; finished in 9.13s

$ cargo test --workspace --locked --lib
designos-diff:       test result: ok. 13 passed; 0 failed
designos-doc:        test result: ok. 710 passed; 0 failed
designos-export:     test result: ok. 153 passed; 0 failed; 1 ignored
designos-fig:        test result: ok. 143 passed; 0 failed
designos-fonts:      test result: ok. 30 passed; 0 failed
designos-gpu:        test result: ok. 4 passed; 0 failed
designos-library:    test result: ok. 37 passed; 0 failed
designos-mcp:        test result: ok. 297 passed; 0 failed
designos-render:     test result: ok. 56 passed; 0 failed; 2 ignored
designos-svg:        test result: ok. 1 passed; 0 failed
designos-webcapture: test result: ok. 8 passed; 0 failed

$ python3 scripts/check_chrome_colour_literals.py
Chrome colour literal gate PASSED (29 files with findings, 199/199 baseline chrome sites).

$ python3 scripts/zero_fake_ui_gate.py
 Gate Verdict: PASS (17/17 invariants satisfied)
```

---

### 7. Epistemic Ledger (`FACT` / `INFERENCE` / `ASSUMPTION`), Weakest Link & Unresolved Questions

- **FACT:** On `f82be3e0`, `picker_dropdown` was `55.0` px wide (`25.0` px text budget) and 3-channel cells were `29.0` px wide (`19.0` px text budget), causing `"OKLCH"` (`41.91` px), `"OKLAB"` (`41.09` px), `"255"` (`21.56` px), `"100"` (`20.03` px), `"360"` (`22.44` px), and all 3-cell OKLCH/OKLAB numbers to elide with `'…'` at rest.
- **FACT:** Sizing `picker-fmt` to `(max_label_w + 24.0).ceil() = 66.0` px (`42.0` px text budget), rendering `Fmt::Oklch | Fmt::Oklab` as a single `138.0` px cell (`132.0` px text budget with `pad = 3.0` px), and sizing `Fmt::Rgb | Fmt::Hsl | Fmt::Hsb` to `30.67` px cells (`24.67` px text budget) + `43.0` px opacity cell (`21.0` px number text budget) eliminates elision across every mode at rest, verified by both `visible_texts` (`col.a() > 0`) and `r2-picker-modes.png`.
- **FACT:** Clamping `+inf -> 1.0` in `sanitize_unit` and clamping `L` via `sanitize_unit(l)` at the entry of `oklab_to_srgb` maps `srgb_to_oklab(+inf, +inf, +inf)` to `[1.0, 0.0, 0.0]` and `oklab_to_srgb(1e13, 0.0, 0.0)` to `[1.0, 1.0, 1.0]`.
- **INFERENCE:** Preserving unedited channel values in `State::apply_ok_input` when a token matches `fmt_ok_token(i, cur_triple[i])` or `fmt_ok_channel(i, cur_triple[i])` retains sub-display `f32` precision and grey hue memory (`R6`) in single-cell mode.
- **ASSUMPTION:** In single-cell `Fmt::Oklch` / `Fmt::Oklab` input, both space-separated (`62.8% 0.26 29.23 / 0.5`) and comma-separated (`oklch(62.8%, 0.26, 29.23deg / 50%)`) token lists are accepted so CSS-style pastes and manual space-separated edits both succeed.
- **Weakest link:** Single-cell `Fmt::Oklch` / `Fmt::Oklab` strings with extreme negative `a`/`b` plus explicit non-unit alpha (`100% -0.4 -0.4 / 0.25`, `126.3` px) sit within `5.7` px of the `132.0` px inner text budget at `AREA = 208.0` px.
- **Unresolved questions:** None.

Status: DONE
