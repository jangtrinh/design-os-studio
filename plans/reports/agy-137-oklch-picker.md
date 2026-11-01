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
     - **Correction (`d4a09c33` disclosure):** On `d4a09c33`, `apply_ok_input` preserved unedited channels only when at least one of the three channel tokens differed from its displayed string; when all three channel tokens matched the displayed strings (e.g. an alpha-only edit `"56.93% 0.14 254.47 / 0.5"` on `[52, 120, 200]` or pressing `Enter` without editing), lines `568–570` (`if !changed_mask.iter().any(|&c| c) { changed_mask = [true, true, true]; }`) forced all three channels to re-apply from the 2-decimal rounded strings, drifting `2,337 / 4,096` colours (`[52, 120, 200] -> [51, 121, 199]`). Additionally, editing one channel while `/ a` was shown re-parsed the 2-decimal `a` and drifted `155 / 254` non-endpoint alphas (`141 -> 140`). Both defects are fixed in [`## Round 3`](#round-3).
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
| **3** | Headless 2x CPU-raster proof sheet (`r2-picker-modes.png`) + visible strings per mode | **PARTIAL on `d4a09c33` (fixed in Round 3)** — zero `'…'` on `[255, 0, 0]` (`4`-char `H`) at `alpha = 255` and `128`, but on `d4a09c33` elided `3,103 / 4,096` (`75.8%`) of OKLCH and `1,722 / 4,096` (`42.0%`) of OKLAB at `alpha = 140` (`0.55`), `2,069 / 4,096` (`50.5%`) of OKLCH and `353 / 4,096` (`8.6%`) of OKLAB at `alpha = 128` (`0.5`), and `40 / 216` (`18.5%`) of `Fmt::Css` at `alpha = 140` | `r2-picker-modes.png`, `r2-visible-strings.log` |
| **4a** | `cargo fmt --all -- --check` exits 0 | **PASS** (`exit=0`) | `r2-gates.log` |
| **4b** | `cargo clippy --workspace --all-targets --locked -- -D warnings`: only `main`'s known errors (`agent_permissions.rs:1163,1181`, `panels/components.rs` items_after_test_module / unused run_ui / ptr_arg) | **PASS** (0 errors/warnings in `designos-doc` or `picker.rs`; only `main`'s 5 known errors) | `r2-gates.log` |
| **4c** | `cargo test -p designos-doc --locked --lib` passes | **PASS** (`710 passed; 0 failed`) | `r2-gates.log` |
| **4d** | `cargo test -p designos-app --bin designos --locked picker` passes | **PASS** (`92 passed; 0 failed`) | `r2-gates.log` |
| **4e** | `… layout_unchanged` passes, or each moved baseline is a picker cell, listed with its old and new rect | **FAIL on `d4a09c33` (`19/20` passed; `layout_unchanged_f` failed on the 9 moved picker rects across `f-picker`, `f-picker-gradient`, `picker-page`; fixed to `20/20` in Round 3)** | `r2-gates.log` |
| **4f** | `cargo test --workspace --locked --lib` passes | **PASS** (all 12 workspace lib crates `0 failed`, including `designos-eval`) | `r2-gates.log` |
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
designos-eval:       test result: ok. 78 passed; 0 failed
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

#### Acceptance 5 — Pushed Branch Log (`git log --oneline origin/main..HEAD`)

```text
$ git log --oneline origin/main..HEAD
6fd237b5 fix(picker): render OKLCH/OKLAB as a single readable cell and fit all mode labels (#137)
d1a007de Merge remote-tracking branch 'origin/main' into jangtrinh/agy-137-oklch-picker
f82be3e0 docs(report): add verification report and gate logs for #137
95d55aa1 feat(picker): OKLCH and OKLAB colour modes (#137)
42049287 test(picker): add failing tests for OKLCH/OKLAB conversion and picker modes (#137)
```

---

### 7. Epistemic Ledger (`FACT` / `INFERENCE` / `ASSUMPTION`), Weakest Link & Unresolved Questions (Corrected for `d4a09c33`)

- **FACT:** On `f82be3e0`, `picker_dropdown` was `55.0` px wide (`25.0` px text budget) and 3-channel cells were `29.0` px wide (`19.0` px text budget), causing `"OKLCH"` (`41.91` px), `"OKLAB"` (`41.09` px), `"255"` (`21.56` px), `"100"` (`20.03` px), `"360"` (`22.44` px), and all 3-cell OKLCH/OKLAB numbers to elide with `'…'` at rest.
- **FACT (Corrected per M3):** On `d4a09c33`, sizing `picker-fmt` to `66.0` px (`42.0` px text budget), rendering `Fmt::Oklch | Fmt::Oklab` as a single `138.0` px cell (`132.0` px text budget with `pad = 3.0` px), and sizing `Fmt::Rgb | Fmt::Hsl | Fmt::Hsb` to `30.67` px cells (`24.67` px text budget) + `43.0` px opacity cell (`21.0` px number text budget) eliminated elision at `alpha = 255` (`0 / 4,096` elided in `OKLCH` and `OKLAB`), and on `[255, 0, 0]` (`4`-char `H`) at `alpha = 128` (`"62.8% 0.26 29.23 / 0.5"`, `130.78` px). However, because `d4a09c33` appended ` / a` at rest when `alpha < 255` (and `css_text` formatted `rgba(r, g, b, a)`), `d4a09c33` elided `3,103 / 4,096` (`75.8%`) of OKLCH colours and `1,722 / 4,096` (`42.0%`) of OKLAB colours at `alpha = 140` (`0.55`), `2,069 / 4,096` (`50.5%`) of OKLCH and `353 / 4,096` (`8.6%`) of OKLAB at `alpha = 128` (`0.5`), and `40 / 216` (`18.5%`) of `Fmt::Css` at `alpha = 140`. Resolved in [`## Round 3`](#round-3) by omitting alpha from the value cell at rest in every mode.
- **FACT:** Clamping `+inf -> 1.0` in `sanitize_unit` and clamping `L` via `sanitize_unit(l)` at the entry of `oklab_to_srgb` maps `srgb_to_oklab(+inf, +inf, +inf)` to `[1.0, 0.0, 0.0]` and `oklab_to_srgb(1e13, 0.0, 0.0)` to `[1.0, 1.0, 1.0]`.
- **INFERENCE (Corrected per M3):** On `d4a09c33`, `State::apply_ok_input` retained unedited channel precision only when at least one of the three channel tokens differed from its displayed string; when all three channel tokens matched the displayed strings (e.g. an alpha-only edit `"56.93% 0.14 254.47 / 0.5"` on `[52, 120, 200]`), lines `568–570` (`if !changed_mask.iter().any(|&c| c) { changed_mask = [true, true, true]; }`) overwrote all three channels with the 2-decimal rounded values (`[52, 120, 200] -> [51, 121, 199]`), and editing one channel at non-1.0 alpha re-parsed the 2-decimal `/ a` and drifted `155 / 254` non-endpoint alphas (`141 -> 140`). Resolved in [`## Round 3`](#round-3).
- **ASSUMPTION:** In single-cell `Fmt::Oklch` / `Fmt::Oklab` input, both space-separated (`62.8% 0.26 29.23 / 0.5`) and comma-separated (`oklch(62.8%, 0.26, 29.23deg / 50%)`) token lists are accepted so CSS-style pastes and manual space-separated edits both succeed.
- **Weakest link (Corrected per M3):** On `d4a09c33`, `"100% -0.4 -0.4 / 0.25"` (`126.3` px) was not the widest string because `4` builds a wider glyph than `-` in Inter `11` px: `"68.46% 0.26 332.43 / 0.55"` measured `153.91` px and `"42.26% -0.04 -0.04 / 0.55"` measured `150.22` px, overflowing the `132.0` px inner text budget by up to `21.91` px (`25.88` px in `Fmt::Css` `"rgba(204, 204, 204, 0.44)"` at `141.66` px). Resolved in [`## Round 3`](#round-3).
- **Unresolved questions:** None.

---

## Round 3

### 8. What Could Not Be Verified (Stated First)

1. **Live Figma Desktop interactive runtime:** Per hard rules, Figma Desktop was not opened during the run; CSS `oklch(...)` / `oklab(...)` unitless `0..1` vs `%` `0..100%` lightness parsing and at-rest single-cell formatting follow the controller's rulings in `/Users/jangtrinh/Products/design-os-studio/plans/261030-2041-e1-wave-1/brief-agy-137-r3.txt`.
2. **Real-app windowed OS capture:** Per the controller's workflow, rendered verification in this worker was performed via the deterministic headless 2x (`ppp = 2.0`) CPU-raster proof sheet (`plans/reports/agy-137-oklch-picker-evidence/r3-modes.png`), visual inspection of the rendered PNG across all 7 modes at 50% alpha, and automated `visible_texts` + 4,096-colour grid width assertions across all 7 modes at 50% (`128`) and 55% (`140`) alpha.

---

### 9. Acceptance Table & Pasted Outputs (Round 3)

| # | Acceptance Criterion | Verdict | Evidence File |
|---|---|---|---|
| **1a** | New test (B2/M1): at-rest value cell has no alpha and does not elide over the colour grid at 50% (`128`) and 55% (`140`) alpha, in every mode (`OKLCH`, `OKLAB`, `CSS`, `RGB`, `HSL`, `HSB`, `Hex`) — RED on `d4a09c33`, GREEN on head | **PASS** (`FAILED` on `d4a09c33` -> `ok` on head; `0 / 4,096` overflow or elide in any mode at 50% or 55% alpha) | `r3-red-picker.log`, `r3-green-picker.log` |
| **1b** | New test (B3 + m1): `[52, 120, 200]` in OKLCH + `"56.93% 0.14 254.47 / 0.5"` commits `[52, 120, 200, 128]`; `[52, 120, 200, 141]` L-only edit with `/ 0.55` keeps alpha `141` — RED on `d4a09c33`, GREEN on head | **PASS** (`FAILED` on `d4a09c33` -> `ok` on head) | `r3-red-picker.log`, `r3-green-picker.log` |
| **1c** | New test (M2): pasting `"oklch(0.985 0 0)"` commits `[250, 250, 250, 255]`; pasting `"oklch(62.8% 0.26 29.23)"` commits `[255, 0, 0, 255]` — RED on `d4a09c33`, GREEN on head | **PASS** (`FAILED` on `d4a09c33` -> `ok` on head) | `r3-red-picker.log`, `r3-green-picker.log` |
| **1d** | New tests (m2, m3, l1, l2): tests kill surviving mutants `N01`, `N03`, `N05`, `N07`, `N15`, `N16`; mutant IDs removed from test comments; `"On this page"` stays at `main`'s `PAD + 8.0` (`24.0` px) and `"OKLCH"` keeps `>= 4.0` px gap to its chevron | **PASS** (`6/6` mutants `N01, N03, N05, N07, N15, N16` KILLED; `dropdown_insets_keep_source_at_main_and_fmt_chevron_gap` `FAILED` on `d4a09c33` -> `ok` on head) | `r3-mutants.log`, `r3-red-picker.log`, `r3-green-picker.log` |
| **2** | Visual check: render the picker in `OKLCH`, `OKLAB`, `CSS`, `RGB`, `HSL`, `HSB`, and `Hex` at 50% alpha (`plans/reports/agy-137-oklch-picker-evidence/r3-modes.png`) and visually confirm every value is readable and not elided | **PASS** (visually inspected `r3-modes.png` at `2x` `ppp`: all 7 modes at 50% alpha for `[52, 120, 200, 128]` and `[255, 0, 0, 128]` are crisp, unelided, and show no alpha in the value cell) | `r3-modes.png` |
| **3a** | `cargo fmt --all -- --check` exits 0 | **PASS** (`exit=0`) | `r3-gates.log` |
| **3b** | `cargo clippy --workspace --all-targets --locked -- -D warnings`: only `main`'s known lints (`agent_permissions.rs:1176,1194`) | **PASS** (0 errors/warnings in `picker.rs` or `designos-doc`; only `main`'s 2 unneeded-return errors in `agent_permissions.rs:1176:9` and `:1194:9`) | `r3-gates.log` |
| **3c** | `cargo test -p designos-app --bin designos --locked picker` passes | **PASS** (`96 passed; 0 failed`) | `r3-green-picker.log` |
| **3d** | `cargo test -p designos-app --bin designos --locked layout_unchanged` passes (`20/20`) | **PASS** (`20 passed; 0 failed`) | `r3-layout-unchanged.log` |
| **3e** | `cargo test -p designos-doc --locked --lib` passes | **PASS** (`712 passed; 0 failed`) | `r3-gates.log` |
| **3f** | `cargo test --workspace --locked --lib` passes | **PASS** (all 12 workspace lib targets pass, `0 failed`) | `r3-gates.log` |
| **3g** | `python3 scripts/check_chrome_colour_literals.py`, `python3 scripts/check_spacing_relationships.py`, `python3 scripts/zero_fake_ui_gate.py` pass | **PASS** (`199/199` chrome baseline; spacing `PASS: no count rose above the baseline`; zero-fake-UI `17/17` invariants) | `r3-gates.log` |

#### Acceptance 1 — RED on `d4a09c33` (`r3-red-picker.log`) and GREEN on Head (`r3-green-picker.log`)

```text
$ cargo test -p designos-app --bin designos --locked picker  # on d4a09c33 implementation (r3-red-picker.log)
failures:

---- widgets::picker::tests::alpha_only_or_retyped_channels_preserve_rgb_and_alpha_precision stdout ----
thread 'widgets::picker::tests::alpha_only_or_retyped_channels_preserve_rgb_and_alpha_precision' (57722977) panicked at crates/designos-app/src/widgets/picker.rs:2314:9:
retyping unchanged displayed channels must not mark out.changed

---- widgets::picker::tests::css_and_hsl_roundtrip stdout ----
thread 'widgets::picker::tests::css_and_hsl_roundtrip' (57722980) panicked at crates/designos-app/src/widgets/picker.rs:1909:9:
assertion `left == right` failed
  left: "rgba(217, 217, 217, 1)"
 right: "rgb(217, 217, 217)"

---- widgets::picker::tests::dropdown_insets_keep_source_at_main_and_fmt_chevron_gap stdout ----
thread 'widgets::picker::tests::dropdown_insets_keep_source_at_main_and_fmt_chevron_gap' (57722982) panicked at crates/designos-app/src/widgets/picker.rs:2409:9:
On this page must start at PAD + 8.0 (24.0), got 22

---- widgets::picker::tests::pasted_css_oklch_and_oklab_functions_accept_unit_and_percent_lightness stdout ----
thread 'widgets::picker::tests::pasted_css_oklch_and_oklab_functions_accept_unit_and_percent_lightness' (57722999) panicked at crates/designos-app/src/widgets/picker.rs:2354:9:
assertion failed: out_white.changed

---- widgets::picker::tests::red_shows_expected_oklch_and_oklab_cells stdout ----
thread 'widgets::picker::tests::red_shows_expected_oklch_and_oklab_cells' (57723002) panicked at crates/designos-app/src/widgets/picker.rs:2067:13:
mode Oklch missing expected visible text "62.8% 0.26 29.23" in ["Custom", "Libraries", "4.00 : 1", "AA Large", "OKLCH", "62.8% 0.26 29.23 / 0.5", "On this page"]

---- widgets::picker::tests::value_cell_at_rest_has_no_alpha_and_never_elides_across_grid stdout ----
thread 'widgets::picker::tests::value_cell_at_rest_has_no_alpha_and_never_elides_across_grid' (57723005) panicked at crates/designos-app/src/widgets/picker.rs:2192:21:
mode Css showed alpha in value cell at rest: ["Custom", "Libraries", "4.50 : 1", "AA", "CSS", "rgba(52, 120, 200, 0.5)", "On this page"]

---- widgets::picker::tests::visible_text_css_mode stdout ----
thread 'widgets::picker::tests::visible_text_css_mode' (57723008) panicked at crates/designos-app/src/widgets/picker.rs:2067:13:
mode Css missing expected visible text "rgb(255, 0, 0)" in ["Custom", "Libraries", "4.00 : 1", "AA Large", "CSS", "rgba(255, 0, 0, 1)", "On this page"]

test result: FAILED. 89 passed; 7 failed; 0 ignored; 0 measured; 3957 filtered out; finished in 7.14s
```

```text
$ cargo test -p designos-app --bin designos --locked picker  # on head (r3-green-picker.log)
running 96 tests
test widgets::picker::tests::active_color_edits_the_selected_stop ... ok
test widgets::picker::tests::alpha_only_or_retyped_channels_preserve_rgb_and_alpha_precision ... ok
test widgets::picker::tests::css_and_hsl_roundtrip ... ok
test widgets::picker::tests::dropdown_insets_keep_source_at_main_and_fmt_chevron_gap ... ok
test widgets::picker::tests::editing_oklch_and_oklab_channels_commits_srgb_rgba ... ok
test widgets::picker::tests::gradient_shape_switch_keeps_stops ... ok
test widgets::picker::tests::gradient_to_solid_takes_first_stop ... ok
test widgets::picker::tests::hsb_is_computed_in_gamma_space ... ok
test widgets::picker::tests::hue_survives_grey ... ok
test widgets::picker::tests::oklch_and_oklab_format_list_and_default ... ok
test widgets::picker::tests::oklch_state_preserves_typed_values_and_grey_hue ... ok
test widgets::picker::tests::parse_oklch_and_oklab_channel_rules_and_clamps ... ok
test widgets::picker::tests::pasted_css_oklch_and_oklab_functions_accept_unit_and_percent_lightness ... ok
test widgets::picker::tests::procedural_png_presets_generate_valid_png_headers_and_distinct_textures ... ok
test widgets::picker::tests::red_shows_expected_oklch_and_oklab_cells ... ok
test widgets::picker::tests::solid_to_gradient_keeps_color_as_first_stop ... ok
test widgets::picker::tests::stop_color_interpolates ... ok
test widgets::picker::tests::value_cell_at_rest_has_no_alpha_and_never_elides_across_grid ... ok
test widgets::picker::tests::visible_text_css_mode ... ok
test widgets::picker::tests::visible_text_hex_mode ... ok
test widgets::picker::tests::visible_text_hsb_at_full_s_b ... ok
test widgets::picker::tests::visible_text_hsl_mode ... ok
test widgets::picker::tests::visible_text_oklab_mode ... ok
test widgets::picker::tests::visible_text_oklch_mode ... ok
test widgets::picker::tests::visible_text_rgb_at_255_255_255 ... ok
...
test result: ok. 96 passed; 0 failed; 0 ignored; 0 measured; 3957 filtered out; finished in 7.77s
```

#### Surviving Mutants Killed (`r3-mutants.log`)

```text
N01	KILLED	37.0s	failed=['test widgets::picker::tests::alpha_only_or_retyped_channels_preserve_rgb_and_alpha_precision ... FAILED']
N03	KILLED	48.3s	failed=['test widgets::picker::tests::alpha_only_or_retyped_channels_preserve_rgb_and_alpha_precision ... FAILED']
N05	KILLED	41.4s	failed=['test widgets::picker::tests::pasted_css_oklch_and_oklab_functions_accept_unit_and_percent_lightness ... FAILED']
N07	KILLED	20.7s	failed=['test widgets::picker::tests::alpha_only_or_retyped_channels_preserve_rgb_and_alpha_precision ... FAILED']
N15	KILLED	15.5s	failed=['test widgets::picker::tests::pasted_css_oklch_and_oklab_functions_accept_unit_and_percent_lightness ... FAILED']
N16	KILLED	14.1s	failed=['test widgets::picker::tests::pasted_css_oklch_and_oklab_functions_accept_unit_and_percent_lightness ... FAILED']
SUMMARY: 6/6 killed
```

#### Acceptance 2 — Visual Proof Sheet at 50% Alpha (`r3-modes.png`)

- Rendered proof sheet: `plans/reports/agy-137-oklch-picker-evidence/r3-modes.png`
- Shows all 7 modes (`OKLCH`, `OKLAB`, `CSS`, `RGB`, `HSL`, `HSB`, `Hex`) at `ppp = 2.0` and `50%` alpha (`128` / `0.5`) for `[52, 120, 200, 128]` (top row) and `[255, 0, 0, 128]` (bottom row):
  - `OKLCH`: `"56.93% 0.14 254.47"` (top) and `"62.8% 0.26 29.23"` (bottom), widest across all 4,096 colours = `"68.46% 0.26 332.43"` (`118.03` px <= `132.0` px budget).
  - `OKLAB`: `"56.93% -0.04 -0.14"` (top) and `"62.8% 0.22 0.13"` (bottom), widest across all 4,096 colours = `"42.26% -0.04 -0.04"` (`114.34` px <= `132.0` px budget).
  - `CSS`: `"rgb(52, 120, 200)"` (top) and `"rgb(255, 0, 0)"` (bottom), widest across all 256³ colours = `"rgb(204, 204, 204)"` (`108.56` px <= `132.0` px budget).
  - `RGB`: `"52"`, `"120"`, `"200"`, `"50 %"` (top) and `"255"`, `"0"`, `"0"`, `"50 %"` (bottom).
  - `HSL`: `"212"`, `"59"`, `"49"`, `"50 %"` (top) and `"0"`, `"100"`, `"50"`, `"50 %"` (bottom).
  - `HSB`: `"212"`, `"74"`, `"78"`, `"50 %"` (top) and `"0"`, `"100"`, `"100"`, `"50 %"` (bottom).
  - `Hex`: `"3478C8"`, `"50 %"` (top) and `"FF0000"`, `"50 %"` (bottom).

#### Acceptance 3 — Pasted Gate Outputs (`r3-gates.log` & `r3-layout-unchanged.log`)

```text
$ cargo fmt --all -- --check
exit=0

$ cargo clippy --workspace --all-targets --locked -- -D warnings
error: unneeded `return` statement
    --> crates/designos-app/src/agent_permissions.rs:1176:9
error: unneeded `return` statement
    --> crates/designos-app/src/agent_permissions.rs:1194:9
error: could not compile `designos-app` (bin "designos-studio") due to 2 previous errors
error: could not compile `designos-app` (bin "designos") due to 2 previous errors
error: could not compile `designos-app` (bin "designos-studio" test) due to 2 previous errors
error: could not compile `designos-app` (bin "designos" test) due to 2 previous errors

$ cargo test -p designos-app --bin designos --locked picker
test result: ok. 96 passed; 0 failed; 0 ignored; 0 measured; 3957 filtered out; finished in 7.77s

$ cargo test -p designos-app --bin designos --locked layout_unchanged
running 20 tests
test shell::tests::layout_unchanged::cases_json_has_no_old_fixture ... ok
test shell::tests::layout_unchanged::panel_presets::cli_accepts_panel_comments ... ok
test shell::tests::layout_unchanged::cases_json_and_allow_json_match_spec_l_contract ... ok
test shell::tests::layout_unchanged::new_named_widget_is_reported_not_failed ... ok
test shell::tests::layout_unchanged::unblessed_cases_file_fails_the_gate ... ok
test shell::tests::layout_unchanged::prep_helpers_populate_missing_starter_states ... ok
test shell::tests::layout_unchanged::panel_presets::comments_panel_with_no_thread_opens_comment_mode_only ... ok
test shell::tests::layout_unchanged::panel_presets::comments_panel_opens_the_first_unresolved_thread_in_paint_order ... ok
test shell::tests::layout_unchanged::panel_presets::interaction_panel_opens_the_selected_nodes_first_reaction ... ok
test shell::tests::layout_unchanged::panel_presets::interaction_panel_with_no_reaction_opens_the_prototype_tab_only ... ok
test shell::tests::layout_unchanged::panel_presets::interaction_panel_without_a_reacting_selection_picks_the_first_reaction_in_paint_order ... ok
test shell::tests::layout_unchanged::sim_loads_starter_and_seed_library_at_seeds_starter_not_showcase ... ok
test shell::tests::layout_unchanged::h::layout_unchanged_h ... ok
test shell::tests::layout_unchanged::e::layout_unchanged_e ... ok
test shell::tests::layout_unchanged::k::layout_unchanged_k ... ok
test shell::tests::layout_unchanged::g::layout_unchanged_g ... ok
test shell::tests::layout_unchanged::d::layout_unchanged_d ... ok
test shell::tests::layout_unchanged::i::layout_unchanged_i ... ok
test shell::tests::layout_unchanged::j::layout_unchanged_j ... ok
test shell::tests::layout_unchanged::f::layout_unchanged_f ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 4033 filtered out; finished in 16.19s

$ cargo test -p designos-doc --locked --lib
test result: ok. 712 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.93s

$ cargo test --workspace --locked --lib
designos-diff:       test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
designos-doc:        test result: ok. 712 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.15s
designos-eval:       test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
designos-export:     test result: ok. 153 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.87s
designos-fig:        test result: ok. 143 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.94s
designos-fonts:      test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.28s
designos-gpu:        test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
designos-library:    test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.13s
designos-mcp:        test result: ok. 322 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.86s
designos-render:     test result: ok. 56 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 34.61s
designos-svg:        test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
designos-webcapture: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s

$ python3 scripts/check_chrome_colour_literals.py
Chrome colour literal gate PASSED (29 files with findings, 199/199 baseline chrome sites).

$ python3 scripts/check_spacing_relationships.py
literal geometry outside theme.rs: offset_literals=1521, inset_literals=41, radius_literals=171, font_literals=23, nonconcentric_pairs=2, legacy_control_refs=6
note: 2 entries are below baseline; run --write-baseline to lock the gain in
PASS: no count rose above the baseline

$ python3 scripts/zero_fake_ui_gate.py
 Gate Verdict: PASS (17/17 invariants satisfied)
```

#### Pushed Branch Log (`git log --oneline origin/main..HEAD`)

```text
$ git log --oneline origin/main..HEAD
ef1b0f23 fix(picker): omit alpha in value cell at rest and preserve channel precision (#137)
ffb2ee3f Merge remote-tracking branch 'origin/main' into jangtrinh/agy-137-oklch-picker
d4a09c33 docs(report): add round 2 verification report and evidence for #137
6fd237b5 fix(picker): render OKLCH/OKLAB as a single readable cell and fit all mode labels (#137)
d1a007de Merge remote-tracking branch 'origin/main' into jangtrinh/agy-137-oklch-picker
f82be3e0 docs(report): add verification report and gate logs for #137
95d55aa1 feat(picker): OKLCH and OKLAB colour modes (#137)
42049287 test(picker): add failing tests for OKLCH/OKLAB conversion and picker modes (#137)
```

---

### 10. Summary of Round 3 Changes & Skills Applied

1. **Ruling 1 (`B2` and `M1` — No alpha in the value cell at rest, in every mode):**
   - Updated `css_text(c: [u8; 4])` in `crates/designos-app/src/widgets/picker.rs` to return `format!("rgb({}, {}, {})", c[0], c[1], c[2])` (widest string `"rgb(204, 204, 204)"` is `108.56` px <= `132.0` px text budget). `parse_css` continues to accept `rgba(...)`, `rgb(... / a)`, and `#RGBA` / `#RRGGBBAA` in the edit buffer.
   - Updated `State::ok_text(&self, fmt: Fmt, _alpha: u8)` to return `format!("{} {} {}", fmt_ok_token(0, t[0]), fmt_ok_token(1, t[1]), fmt_ok_token(2, t[2]))` without ` / a` at rest (widest OKLCH string `"68.46% 0.26 332.43"` is `118.03` px; widest OKLAB string `"42.26% -0.04 -0.04"` is `114.34` px <= `132.0` px text budget). `State::apply_ok_input` continues to accept ` / a` (`0..=1` or `0%..=100%`) in the edit buffer.
2. **Ruling 2 (`B3` and `m1` — Apply `fix-trial-alpha-keep.diff`):**
   - Removed the `if !changed_mask.iter().any(|&c| c) { changed_mask = [true, true, true]; }` fallback from `State::apply_ok_input` so an alpha-only edit (`"56.93% 0.14 254.47 / 0.5"` on `[52, 120, 200]`) or retyping the shown channels never rewrites the `f32` channels from the 2-decimal display tokens.
   - Preserved `keep_alpha` in `State::apply_ok_input` when `keep_alpha < 255 && a_str == shown_a` (`shown_a = fmt_num(((keep_alpha as f32 / 255.0) * 100.0).round() / 100.0)`) so editing a channel with an unchanged 2-decimal `/ a` (e.g. `keep_alpha = 141` with `/ 0.55`) keeps `141` instead of quantizing to `140`.
3. **Ruling 3 (`B1` — Update the 9 picker rects in the 3 layout baselines):**
   - Applied `/Users/jangtrinh/Products/design-os-studio/plans/reports/code-reviewer-261101-1000-pr-1451-r2-evidence/fix-trial-9-rects.diff` to `crates/designos-app/tests/layout-baseline/{f-picker,f-picker-gradient,picker-page}.layout.json` (`15 insertions(+), 15 deletions(-)` across the 3 files; `layout_unchanged` passes `20/20`).
4. **Ruling 4 (`M2` — Pasted `oklch(...)` / `oklab(...)` follows CSS lightness rules):**
   - In `State::apply_ok_input`, tracked `is_func = true` when stripping `oklch(...)` or `oklab(...)`. When `is_func && i == 0 && !tok.ends_with('%')`, bare `L` is evaluated via `field::eval_number_expr(tok, Some(cur_triple[0]), false)?.clamp(0.0, 1.0)` in `0..=1` (`oklch(0.985 0 0)` commits `[250, 250, 250, 255]`), while `%`-suffixed `L` (`oklch(62.8% 0.26 29.23)`) continues to parse in `0..=100%`.
5. **Ruling 5 (`m2, m3` — Kill surviving mutants `N01, N03, N05, N07, N15, N16` & remove mutant IDs from comments):**
   - Added `value_cell_at_rest_has_no_alpha_and_never_elides_across_grid`, `alpha_only_or_retyped_channels_preserve_rgb_and_alpha_precision`, and `pasted_css_oklch_and_oklab_functions_accept_unit_and_percent_lightness`, killing all 6 surviving mutants (`N01, N03, N05, N07, N15, N16`).
   - Removed all mutant IDs (`(M05)`, `(M07)`, `(M08)`, `(M09)`, `(M10)`, `(M17)`, `M11:`, `M12:`) from comments in `crates/designos-app/src/widgets/picker.rs`.
6. **Ruling 6 (`l1, l2` — Preserve `main`'s `"On this page"` insets and `4.0` px format-chevron gap):**
   - In `picker_dropdown`, kept `"picker-source"` ("On this page") at `main`'s exact `(left_pad, right_reserve, chev_right) = (8.0, 22.0, 12.0)` (`l1`), and set `"picker-fmt"` to `(4.0, 20.0, 10.0)` with `dd_w = (max_label_w + 4.0 + 20.0).ceil() = 66.0` px so `"OKLCH"` (`41.91` px, ending at `rect.left() + 45.91`) keeps a `4.09` px (`>= 4.0` px) gap to `chev.left()` (`rect.right() - 16.0 = rect.left() + 50.0`, `l2`) while keeping `dd_w = 66.0` px.

---

### 11. Round 3 Epistemic Ledger (`FACT` / `INFERENCE` / `ASSUMPTION`), Weakest Link & Unresolved Questions

- **FACT:** With alpha omitted at rest from `ok_text` (`"L% C H"` / `"L% a b"`) and `css_text` (`"rgb(r, g, b)"`), the widest at-rest strings across the 16-step (`4,096`-colour) sRGB grid at any alpha (`255`, `140` / 55%, `128` / 50%) are `"68.46% 0.26 332.43"` (`118.03` px) in `Fmt::Oklch`, `"42.26% -0.04 -0.04"` (`114.34` px) in `Fmt::Oklab`, and `"rgb(204, 204, 204)"` (`108.56` px) in `Fmt::Css`, all fitting within the `132.0` px inner text budget (`138.0` px cell, `pad = 3.0` px) with `0` elided strings.
- **FACT:** Removing the `!changed_mask.iter().any` fallback and preserving `keep_alpha` when `a_str == shown_a` in `State::apply_ok_input` ensures `[52, 120, 200]` + `"56.93% 0.14 254.47 / 0.5"` commits `[52, 120, 200, 128]` with bit-identical `st.oklch`, and an `L`-only edit on `[52, 120, 200, 141]` with `"/ 0.55"` keeps alpha `141`.
- **FACT:** Parsing bare `L` as `0..=1` inside `oklch(...)` / `oklab(...)` functional syntax while keeping `%`-suffixed `L` as `0..=100%` makes `oklch(0.985 0 0)` commit `[250, 250, 250, 255]` and `oklch(62.8% 0.26 29.23)` commit `[255, 0, 0, 255]`.
- **INFERENCE:** Setting `disp_ch = disp_tok.clone()` for `i == 0` when `is_func` is `true` ensures a bare unitless `L` in `oklch(0.63 0.26 29.23)` is never mistaken for the `0..100` `fmt_ok_channel` display string while a `%`-suffixed `L` (`oklch(56.93% 0.14 254.47 / 0.5)`) still matches `disp_tok`.
- **ASSUMPTION:** Per Round 2 Ruling 1 ("One cell (`grouped_text` over `fields.width()`), like `Fmt::Css`. The opacity cell leaves the row in `Fmt::Oklch` and `Fmt::Oklab`, as in `Fmt::Css`") and Round 3 Ruling 1 ("The picker keeps its width (`W = 240.0`, `AREA = 208.0`)"), `Fmt::Css`, `Fmt::Oklch`, and `Fmt::Oklab` keep their `138.0` px single value cell (with alpha controlled via the alpha slider above the row or `/ a` in the edit buffer), because placing `opacity_cell` (`43.0` px) in those rows would shrink the single value cell to `94.0` px (`88.0` px text budget), which cannot fit `"62.8% 0.26 29.23"` (`102.03` px), `"62.8% 0.22 0.13"` (`91.25` px), or `"rgb(255, 255, 255)"` (`102.3` px).
- **Weakest link:** A typed OKLCH state can show `"44.44% 0.04 344.44"` (`119.81` px, vs `"68.46% 0.26 332.43"` at `118.03` px on the 4,096-colour sRGB grid), leaving `12.19` px of margin inside the `132.0` px inner text budget (`138.0` px cell, `pad = 3.0` px) and still painting in full.
- **Unresolved questions (at end of Round 3; resolved in [`## Round 4`](#round-4)):** In `Fmt::Oklch`, `Fmt::Oklab`, and `Fmt::Css`, the value cell spans the full `138.0` px row and has no numeric `opacity_cell` next to it at rest, so whether the alpha slider (and the inspector fill row for solid paints) suffices as the alpha display needed a controller ruling (resolved as Option A in Round 4 Ruling 1).

---

## Round 4

### 12. What Could Not Be Verified (Stated First)

1. **Live Figma Desktop interactive runtime:** Per hard rules, Figma Desktop was not opened during the run; CSS `rgb(...)` / `rgba(...)` / `hsl(...)` / `hsla(...)` percent-alpha parsing and single-cell OKLCH/OKLAB/CSS width/opacity conventions follow the controller's rulings in `/Users/jangtrinh/Products/design-os-studio/plans/261030-2041-e1-wave-1/brief-agy-137-r4.txt`.
2. **Real-app windowed OS capture:** Per the controller's workflow, no UI layout changed in Round 4 and real-app GUI captures remain with the controller; verification in this round was performed via the `picker` and `layout_unchanged` test suites (`full.shapes` text and chevron mesh geometry) and the mutation-testing harness (`NULL`, `R03`, `R04`, `R08`, `R11`).

---

### 13. Skills Loaded & Changes Driven by Them

- **`/es:dev` -> `es:feature-dev`** (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`): Executed directly in the DEV phase against the controller's Round 4 rulings without re-opening brainstorm/plan gates, verifying RED-before-GREEN on `59325c60` and all workspace gates before closing.
- **`/es:rust`** (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`): Kept `parse_css`'s borrowed slice tokenization (`parts: Vec<&str>`) zero-copy over `inner`, dropped the unused `_alpha: u8` parameter from `State::ok_text(&self, fmt: Fmt)` (`LOW-1`), and kept `picker.rs` free of compiler warnings.
- **`/es:rust-gpu-canvas`** (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`): Preserved `Paint` 8-bit sRGB storage and GPU/tile cache invariants (`0` changes outside `crates/designos-app/src/widgets/picker.rs`).
- **`es:astra-work` & `es:fable-thinking`** (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`): Verified RED on `59325c60` before applying production changes, killed all three surviving Round 3 mutants (`R03`, `R04`, `R08`) plus the CSS percent-alpha mutant (`R11`) with a null-control-first mutant run, and reconciled the 4 stale Round 3 report lines (`MINOR-1`) against the actual log outputs.

---

### 14. Summary of Round 4 Rulings & Fixes

1. **Ruling 1 (`MAJOR-1 -> Option A`):**
   - In OKLCH, OKLAB, and CSS modes, the single value cell spans the full row (`138.0` px) with no separate opacity cell so the picker keeps its width (`W = 240.0`, `AREA = 208.0`) without elision, while alpha is shown by the alpha slider (and the inspector fill row for solid paints) and accepted via `/ a` or CSS alpha in the edit buffer. No code change required; stated here and in the PR #1451 body.
2. **Ruling 2 (`MAJOR-2` — CSS percent alpha in `parse_css`):**
   - Applied `fix-trial-css-percent-alpha.diff` to `parse_css` in `crates/designos-app/src/widgets/picker.rs`: retained raw tokens in `parts: Vec<&str>` and divided the 4th token by `100.0` before clamping to `0.0..=1.0` when `parts[3].ends_with('%')`.
   - Added the 3 assertions to `css_and_hsl_roundtrip`:
     - `assert_eq!(parse_css("rgb(10 20 30 / 25%)", 255), Some([10, 20, 30, 64]));`
     - `assert_eq!(parse_css("rgba(10, 20, 30, 50%)", 255), Some([10, 20, 30, 128]));`
     - `assert_eq!(parse_css("hsl(240 100% 50% / 50%)", 255), Some([0, 0, 255, 128]));`
3. **Ruling 3 (`MINOR-2` — Pinning assertions killing `R03`, `R04`, and `R08`):**
   - **`R03`:** In `pasted_css_oklch_and_oklab_functions_accept_unit_and_percent_lightness`, pasted `"oklch(62.8 0.26 29.23)"` via `commit_picker_ok` and asserted `assert_eq!(st.oklch[0], 1.0)`.
   - **`R04`:** In `pasted_css_oklch_and_oklab_functions_accept_unit_and_percent_lightness`, pasted `"oklch(56.93 0.14 254.47)"` onto `[52, 120, 200]` (`"56.93% 0.14 254.47"`) and asserted `assert!(out_bare_match.changed)` and `assert_eq!(st_blue.oklch[0], 1.0)`.
   - **`R08`:** In `dropdown_insets_keep_source_at_main_and_fmt_chevron_gap`, replaced the constant `PAD + 66.0 - 10.0 - 6.0` with `find_mesh_in(&full.shapes, PAD, PAD + 66.0, fmt_rect.center().y)` to measure the painted `Icon::ChevronDown` `egui::Shape::Mesh` bounds inside the format dropdown and assert `fmt_chev.left() - fmt_rect.right() >= 4.0`.
4. **Ruling 4 (`MINOR-1` — Corrected Round 3 report lines):**
   - Corrected §9 3b and the pasted clippy block from `agent_permissions.rs:1163,1181` to `agent_permissions.rs:1176:9` and `:1194:9`.
   - Corrected §9 Acceptance 2, §10.1, and §11 FACT widest CSS `"rgb(204, 204, 204)"` width from `105.53` px to `108.56` px.
   - Corrected §11 Weakest link to record that a typed OKLCH state can show `"44.44% 0.04 344.44"` (`119.81` px, `12.19` px margin inside the `132.0` px inner text budget).
   - Updated §11 Unresolved questions to record the MAJOR-1 premise conflict resolved by Round 4 Ruling 1 (Option A).
5. **Ruling 5 (`LOW-1` — Drop unused `_alpha` parameter of `State::ok_text`):**
   - Changed `State::ok_text(&self, fmt: Fmt, _alpha: u8) -> String` to `State::ok_text(&self, fmt: Fmt) -> String` and updated its call sites in `show` (`st.ok_text(st.fmt)`) and `mod tests`. Left `LOW-2` and `LOW-3` unchanged as instructed.

---

### 15. Acceptance Table & Pasted Outputs (Round 4)

| # | Acceptance Criterion | Verdict | Evidence File |
|---|---|---|---|
| **1a** | New asserts in `css_and_hsl_roundtrip` RED on `59325c60` and GREEN on head (`5bb6735d`) | **PASS** (`css_and_hsl_roundtrip` `FAILED` on `59325c60` with `left: Some([10, 20, 30, 255]), right: Some([10, 20, 30, 64])` -> `ok. 96 passed; 0 failed` on head) | `r4-red-picker.log`, `r4-green-picker.log` |
| **1b** | Mutants `R03`, `R04`, and `R08` each KILLED (null control first) | **PASS** (`NULL control` `PASS` `45 passed; 0 failed`; `R03`, `R04`, `R08`, and `R11` all `KILLED` with `rc=101`) | `r4-mutants.log` |
| **2a** | `cargo fmt --all -- --check` exits 0 | **PASS** (`fmt_exit=0`) | `r4-gates.log` |
| **2b** | `cargo clippy --workspace --all-targets --locked -- -D warnings`: only `main`'s 2 known `agent_permissions.rs` lints (`:1176:9` and `:1194:9`) | **PASS** (0 warnings/errors in `picker.rs` or `designos-doc`; only `agent_permissions.rs:1176:9` and `:1194:9`) | `r4-gates.log` |
| **2c** | `cargo test -p designos-app --bin designos --locked picker` passes | **PASS** (`96 passed; 0 failed`) | `r4-gates.log` |
| **2d** | `cargo test -p designos-app --bin designos --locked layout_unchanged` passes (`20/20`) | **PASS** (`20 passed; 0 failed`) | `r4-gates.log` |
| **2e** | `cargo test --workspace --locked --lib` passes | **PASS** (12 workspace lib targets, `1,480 passed; 0 failed`) | `r4-gates.log` |
| **3** | Pushed; PR #1451 body updated with the Option A sentence; `git log --oneline origin/main..HEAD` pasted; `out/cargo` deleted at the end | **PASS** | See below |

#### Acceptance 1 — RED on `59325c60` (`r4-red-picker.log`), GREEN on Head (`r4-green-picker.log`), and Mutant Runs (`r4-mutants.log`)

```text
$ cargo test -p designos-app --bin designos --locked picker  # on 59325c60 production code (r4-red-picker.log)
failures:

---- widgets::picker::tests::css_and_hsl_roundtrip stdout ----

thread 'widgets::picker::tests::css_and_hsl_roundtrip' (61651543) panicked at crates/designos-app/src/widgets/picker.rs:1914:9:
assertion `left == right` failed
  left: Some([10, 20, 30, 255])
 right: Some([10, 20, 30, 64])
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    widgets::picker::tests::css_and_hsl_roundtrip

test result: FAILED. 95 passed; 1 failed; 0 ignored; 0 measured; 3957 filtered out; finished in 7.21s
```

```text
$ cargo test -p designos-app --bin designos --locked picker  # on head 5bb6735d (r4-green-picker.log)
running 96 tests
...
test widgets::picker::tests::active_color_edits_the_selected_stop ... ok
test widgets::picker::tests::alpha_only_or_retyped_channels_preserve_rgb_and_alpha_precision ... ok
test widgets::picker::tests::css_and_hsl_roundtrip ... ok
test widgets::picker::tests::dropdown_insets_keep_source_at_main_and_fmt_chevron_gap ... ok
test widgets::picker::tests::editing_oklch_and_oklab_channels_commits_srgb_rgba ... ok
test widgets::picker::tests::gradient_shape_switch_keeps_stops ... ok
test widgets::picker::tests::gradient_to_solid_takes_first_stop ... ok
test widgets::picker::tests::hsb_is_computed_in_gamma_space ... ok
test widgets::picker::tests::hue_survives_grey ... ok
test widgets::picker::tests::oklch_and_oklab_format_list_and_default ... ok
test widgets::picker::tests::oklch_state_preserves_typed_values_and_grey_hue ... ok
test widgets::picker::tests::parse_oklch_and_oklab_channel_rules_and_clamps ... ok
test widgets::picker::tests::pasted_css_oklch_and_oklab_functions_accept_unit_and_percent_lightness ... ok
test widgets::picker::tests::procedural_png_presets_generate_valid_png_headers_and_distinct_textures ... ok
test widgets::picker::tests::red_shows_expected_oklch_and_oklab_cells ... ok
test widgets::picker::tests::solid_to_gradient_keeps_color_as_first_stop ... ok
test widgets::picker::tests::stop_color_interpolates ... ok
test widgets::picker::tests::visible_text_css_mode ... ok
test widgets::picker::tests::visible_text_hex_mode ... ok
test widgets::picker::tests::visible_text_hsl_mode ... ok
test widgets::picker::tests::visible_text_hsb_at_full_s_b ... ok
test widgets::picker::tests::visible_text_oklch_mode ... ok
test widgets::picker::tests::visible_text_oklab_mode ... ok
test widgets::picker::tests::visible_text_rgb_at_255_255_255 ... ok
test widgets::picker::tests::value_cell_at_rest_has_no_alpha_and_never_elides_across_grid ... ok
...
test result: ok. 96 passed; 0 failed; 0 ignored; 0 measured; 3957 filtered out; finished in 6.84s
```

```text
$ cargo test -p designos-app --bin designos --locked -- widgets::picker::tests layout_unchanged  # mutant runs (r4-mutants.log)
NULL control | PASS | rc=0 test result: ok. 45 passed; 0 failed; 0 ignored; 0 measured; 4008 filtered out; finished in 47.86s failed=[]
R03 CSS bare-L clamp 0..1 removed | KILLED | rc=101 test result: FAILED. 44 passed; 1 failed; 0 ignored; 0 measured; 4008 filtered out; finished in 54.13s failed=['widgets::picker::tests::pasted_css_oklch_and_oklab_functions_accept_unit_and_percent_lightness']
R04 func-form disp_ch special case removed | KILLED | rc=101 test result: FAILED. 44 passed; 1 failed; 0 ignored; 0 measured; 4008 filtered out; finished in 49.09s failed=['widgets::picker::tests::pasted_css_oklch_and_oklab_functions_accept_unit_and_percent_lightness']
R08 format chevron 2 px left (gap 4 -> 2) | KILLED | rc=101 test result: FAILED. 44 passed; 1 failed; 0 ignored; 0 measured; 4008 filtered out; finished in 54.32s failed=['widgets::picker::tests::dropdown_insets_keep_source_at_main_and_fmt_chevron_gap']
R11 CSS percent-alpha /100 removed | KILLED | rc=101 test result: FAILED. 44 passed; 1 failed; 0 ignored; 0 measured; 4008 filtered out; finished in 18.67s failed=['widgets::picker::tests::css_and_hsl_roundtrip']
```

#### Acceptance 2 — Pasted Gate Outputs (`r4-gates.log`)

```text
$ cargo fmt --all -- --check
fmt_exit=0

$ cargo clippy --workspace --all-targets --locked -- -D warnings
error: unneeded `return` statement
    --> crates/designos-app/src/agent_permissions.rs:1176:9
error: unneeded `return` statement
    --> crates/designos-app/src/agent_permissions.rs:1194:9
error: could not compile `designos-app` (bin "designos-studio") due to 2 previous errors
error: could not compile `designos-app` (bin "designos") due to 2 previous errors
error: could not compile `designos-app` (bin "designos-studio" test) due to 2 previous errors
error: could not compile `designos-app` (bin "designos" test) due to 2 previous errors

$ cargo test -p designos-app --bin designos --locked picker
     Running unittests src/main.rs (out/cargo/debug/deps/designos-0146e1a2c129c956)
test result: ok. 96 passed; 0 failed; 0 ignored; 0 measured; 3957 filtered out; finished in 7.01s

$ cargo test -p designos-app --bin designos --locked layout_unchanged
     Running unittests src/main.rs (out/cargo/debug/deps/designos-0146e1a2c129c956)
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 4033 filtered out; finished in 15.71s

$ cargo test --workspace --locked --lib
designos-diff:       test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
designos-doc:        test result: ok. 712 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 53.60s
designos-eval:       test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
designos-export:     test result: ok. 153 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 1.16s
designos-fig:        test result: ok. 143 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 28.20s
designos-fonts:      test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.31s
designos-gpu:        test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
designos-library:    test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 27.95s
designos-mcp:        test result: ok. 322 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.49s
designos-render:     test result: ok. 56 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 82.61s
designos-svg:        test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
designos-webcapture: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.02s

$ python3 scripts/check_chrome_colour_literals.py
Chrome colour literal gate PASSED (29 files with findings, 199/199 baseline chrome sites).

$ python3 scripts/check_spacing_relationships.py
literal geometry outside theme.rs: offset_literals=1521, inset_literals=41, radius_literals=171, font_literals=23, nonconcentric_pairs=2, legacy_control_refs=6
PASS: no count rose above the baseline

$ python3 scripts/zero_fake_ui_gate.py
 Gate Verdict: PASS (17/17 invariants satisfied)
```

#### Acceptance 3 — Pushed Branch Log (`git log --oneline origin/main..HEAD`)

```text
$ git log --oneline origin/main..HEAD
5bb6735d fix(picker): parse CSS percent alpha and pin lightness and chevron assertions (#137)
59325c60 docs(report): add round 3 verification report and evidence for #137
ef1b0f23 fix(picker): omit alpha in value cell at rest and preserve channel precision (#137)
ffb2ee3f Merge remote-tracking branch 'origin/main' into jangtrinh/agy-137-oklch-picker
d4a09c33 docs(report): add round 2 verification report and evidence for #137
6fd237b5 fix(picker): render OKLCH/OKLAB as a single readable cell and fit all mode labels (#137)
d1a007de Merge remote-tracking branch 'origin/main' into jangtrinh/agy-137-oklch-picker
f82be3e0 docs(report): add verification report and gate logs for #137
95d55aa1 feat(picker): OKLCH and OKLAB colour modes (#137)
42049287 test(picker): add failing tests for OKLCH/OKLAB conversion and picker modes (#137)
```

---

### 16. Round 4 Epistemic Ledger (`FACT` / `INFERENCE` / `ASSUMPTION`), Weakest Link & Unresolved Questions

- **FACT:** Dividing the 4th CSS function argument by `100.0` before clamping when `parts[3].ends_with('%')` makes `parse_css("rgb(10 20 30 / 25%)", 255)` return `Some([10, 20, 30, 64])`, `parse_css("rgba(10, 20, 30, 50%)", 255)` return `Some([10, 20, 30, 128])`, and `parse_css("hsl(240 100% 50% / 50%)", 255)` return `Some([0, 0, 255, 128])` (RED on `59325c60`, GREEN on `5bb6735d`).
- **FACT:** Asserting `st.oklch[0] == 1.0` after pasting `"oklch(62.8 0.26 29.23)"` kills mutant `R03`; asserting `out_bare_match.changed && st_blue.oklch[0] == 1.0` after pasting `"oklch(56.93 0.14 254.47)"` on `[52, 120, 200]` kills mutant `R04`; and measuring the painted `egui::Shape::Mesh` chevron rect inside `dd` (`PAD..=PAD + 66.0`) via `find_mesh_in(&full.shapes, ...)` and asserting `fmt_chev.left() - fmt_rect.right() >= 4.0` kills mutant `R08`.
- **FACT:** Dropping `_alpha: u8` from `State::ok_text(&self, fmt: Fmt)` removes the dead parameter with zero warnings, and all gates (`cargo fmt --all -- --check`, `picker` `96/96`, `layout_unchanged` `20/20`, `cargo test --workspace --locked --lib` `1,480/1,480`) pass on `5bb6735d`.
- **INFERENCE:** Because `parts` and `nums` in `parse_css` iterate over the same non-empty comma/space/slash-split slices for valid 4-argument CSS color functions, `parts[3].ends_with('%')` pairs with `nums.get(3)` whenever a 4th numeric argument parses.
- **ASSUMPTION:** Per Round 4 Ruling 1 (`MAJOR-1 -> Option A`), in OKLCH, OKLAB, and CSS modes the value cell spans the row and the alpha is shown by the alpha slider (and the inspector fill row for solid paints), with no opacity cell added to those three rows so the picker keeps its width without elision.
- **Weakest link:** A typed OKLCH state can show `"44.44% 0.04 344.44"` (`119.81` px), leaving `12.19` px of margin inside the `132.0` px inner text budget (`138.0` px cell, `pad = 3.0` px).
- **Unresolved questions:** None.

Status: DONE
