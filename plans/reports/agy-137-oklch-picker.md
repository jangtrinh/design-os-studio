# DEV Report — `#137` Picker Half: OKLCH and OKLAB Colour Modes

## 1. What Could NOT Be Verified

- **[UNVERIFIED — Prohibited by brief]** Live Figma Desktop manual check of `oklch()` / `oklab()` channel scrubbing and expression edge cases (`Never open Figma or any client file` constraint). Relied on the clean-room reverse-engineering report at `plans/reports/re-figma-261031-color-modes-typography-binds.md` (§1.5–1.9).
- **[SKIPPED — Per brief Captures rule]** Live `--panel` screenshot captures showing the picker open in `OKLCH` and `OKLAB` modes: **SKIPPED**. No existing CLI flag opens the colour picker in `OKLCH` or `OKLAB` mode, and adding one requires modifying `crates/designos-app/src/panels/properties.rs` (W2a's file) or `crates/designos-app/src/cli.rs`, both of which are on the never-touch list. The controller takes these captures after merge.
- **[SKIPPED (owner active) — `out/evidence/picker-1440.png`]** Polled `ioreg -c IOHIDSystem | awk '/HIDIdleTime/ {print int($NF/1e9); exit}'` every 60 s for 30 min (`19:56:33` → `20:27:13`); owner remained active (`HIDIdleTime` never reached `>= 300` s, peaking at `178` s before resetting to `0` s). Written as **SKIPPED (owner active)** per Capture timing rule. The headless ImageMagick + Menlo terminal render of Acceptance items 1 and 3 (RED before and GREEN after) is saved at `out/evidence/oklch-tests.png` (git-ignored).
- **[UNVERIFIED — Known `main` clippy blockers outside scope]** `cargo clippy --workspace --all-targets --locked -- -D warnings` exits non-zero solely due to the 5 pre-existing `main` errors listed in the brief (`crates/designos-app/src/agent_permissions.rs:1163`, `:1181` and `crates/designos-app/src/panels/components.rs:263`, `:363`, `:409`). Zero clippy warnings or errors were emitted in `crates/designos-doc` or `crates/designos-app/src/widgets/picker.rs`.

---

## 2. Conclusion

Implemented DEV task `#137` (picker half) on branch `jangtrinh/agy-137-oklch-picker` (based on `origin/main` at `0c894123` with `#1425` merged) in two commits (`42049287` `test(picker): ...` followed by `95d55aa1` `feat(picker): ...` with zero test changes between RED and GREEN):

1. **`crates/designos-doc/src/color_space.rs` (NEW) & `crates/designos-doc/src/lib.rs` (+2 lines):**
   - Added `srgb_to_oklab`, `oklab_to_srgb`, `oklab_to_oklch`, `oklch_to_oklab`, `srgb_to_oklch`, `oklch_to_srgb`, `srgb_u8_to_oklch`, `oklch_to_srgb_u8`, `srgb_u8_to_oklab`, and `oklab_to_srgb_u8` using Björn Ottosson's `M1`/`M2` and inverse matrices (`f32`), standard sRGB transfer curve (`0.04045` / `0.0031308` thresholds, `2.4` gamma), per-channel linear sRGB clamping (`l_r.clamp(0.0, 1.0)` etc. per R4), exact achromatic neutrality (`r == g && g == b` → `a = 0.0, b = 0.0`), and `.is_finite()` sanitization against `NaN`/`±Inf`.
2. **`crates/designos-app/src/widgets/picker.rs`:**
   - Extended `Fmt` to `Hex, Rgb, Css, Hsl, Hsb, Oklch, Oklab` (`Fmt::ALL` length 7; `Fmt::Hex` remains the default).
   - Added 3-channel display & editing for `Fmt::Oklch` (`L%`, `C`, `H°`) and `Fmt::Oklab` (`L%`, `a`, `b`) sharing the `29 px` × 3 + `1 px` gap + `38 px` opacity cell geometry (`129 px` total).
   - Formatted display values with `fmt_num` (`(v * 100.0).round() / 100.0`, trimming trailing zeros) while seeding focused `TextEdit` drafts with full `f32` precision (`fmt_ok_edit`, up to 4 decimals) so clicking into and leaving a cell does not quantize the colour to 2 decimals.
   - Implemented `parse_ok_channel`, `parse_oklch_channel`, and `parse_oklab_channel` matching Figma's `oklch()` / `oklab()` rules (`L` plain or `%` divided by 100 and clamped to `[0, 1]`; `C` plain raw or `n%` = `n * 0.004`, clamped to `[0, 0.4]`; `H` plain or `deg`/`°` wrapped via `rem_euclid(360.0)`; `a`/`b` plain raw or `n%` = `n * 0.004`, clamped to `[-0.4, 0.4]`), delegating arithmetic expressions (`50%+10%`, `180+45`, etc.) to `super::field::eval_number_expr`.
   - Added `oklch: [f32; 3]` and `oklab: [f32; 3]` to `State` with `sync_ok` so typed `OKLCH`/`OKLAB` triples survive across frames when their quantized 8-bit sRGB matches `self.rgb()`, and `oklch[2]` (hue) is preserved across achromatic (`C < 1e-4`) colours (R6).

---

## 3. Re-Anchor Drift & R3 Concern

### Line-Number Re-Anchor (`origin/main` @ `0c894123`)

| File | Brief Citation | Actual Line(s) at `0c894123` | Drift |
|---|---|---|---|
| `crates/designos-doc/src/paint.rs` | `:4-12` (`Color { r, g, b, a: u8 }`) | `:4-12` | `0` |
| `crates/designos-doc/src/lib.rs` | `:51` (`mod paint;`), `:100` (`pub use paint::*;`) | `:51`, `:100` | `0` |
| `crates/designos-app/src/widgets/picker.rs` | `:31-52` (`Fmt`), `:213` (`fmt: Fmt`), `:258` (`State::open`), `:277` (`sync_hsb`), `:389-441` (`rgb_to_hsl` / `hsl_to_rgb`), `:1112-1216` (`show` format row), `:1434-1470` (`fmt_num`, `to_hex`, `from_hex`, `parse_css`), `:1612` (`#[cfg(test)] mod tests`) | Exact match at every anchor | `0` |
| `crates/designos-app/src/widgets/field.rs` | `:128-179` (`eval_number_expr`), `:340-540` (unit tests) | `:128-179`, `:340-540` | `0` |
| `crates/designos-export/src/util.rs` | `:32-34` (`color_css` / `color_hex`) | `:32-34` | `0` |
| `crates/designos-app/src/widgets/mod.rs` | `:319` (`pub fn color_swatch`) | `:319` | `0` |
| `crates/designos-fig/src/paint_map.rs` | `:194-227` (`fig_color`) | `:194-227` | `0` |
| `crates/designos-fig/src/figma_schema.txt` | `:486-500` (`ColorSpace`), `:879-881` (`ColorProfile`) | `:486-500`, `:879-881` | `0` |

### R3 Concern: Closed Format Dropdown Width (`55 px`)

- **[FACT]** In `crates/designos-app/src/widgets/picker.rs:1170`, the format dropdown rect has width `55.0` px (`Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(55.0, 24.0))`).
- **[FACT]** `picker_dropdown` (`crates/designos-app/src/widgets/picker.rs:1346`) calls `elide(p, label, &font, rect.width() - 8.0 - 22.0)`, which leaves `25.0` px of inner horizontal text budget for the closed button label.
- **[FACT]** While 3-letter uppercase labels (`"HEX"`, `"RGB"`, `"CSS"`, `"HSL"`, `"HSB"`) fit inside `25.0` px, the 5-letter uppercase labels `"OKLCH"` and `"OKLAB"` exceed `25.0` px in `theme::font_body_strong()` and are therefore elided with `…` inside the closed `55.0` px button (they display in full inside the `120.0` px open dropdown menu at `picker.rs:1268`).
- **[ACTION]** Per controller ruling **R3** (*"Keep the format dropdown at 55 px... If `'OKLCH'` or `'OKLAB'` clips at 55 px, report it as a concern; do not widen the dropdown and break the 3-cell layout"*), the dropdown width was kept at `55.0` px.

---

## 4. Skills Loaded & Applied

| Skill | How It Shaped This Change |
|---|---|
| `/es:dev` → `es:feature-dev` | Executed strict RED → GREEN commit discipline (`42049287` failing test commit before `95d55aa1` implementation commit with zero test modifications in `95d55aa1`); treated brief rulings R1–R6 as the approved spec without pausing for brainstorm or spawning a sub-verifier. |
| `/es:rust` | Kept `crates/designos-doc/src/color_space.rs` allocation-free (`[f32; 3]` by value, `const` transfer functions), sanitized non-finite floats (`NaN`, `±Inf`) before `cbrt()`/power/cast operations, and short-circuited pure grey (`r == g && g == b`) in `srgb_to_oklab` to return exact `[L, 0.0, 0.0]`. |
| `/es:rust-gpu-canvas` | Preserved all canvas/render invariants and touched only `designos-doc` (`color_space.rs`, `lib.rs`) and `designos-app` (`widgets/picker.rs`). |
| `es:designer` | Reused the existing `Fmt::Rgb \| Fmt::Hsl \| Fmt::Hsb` geometry arm for `Fmt::Oklch \| Fmt::Oklab` in `picker.rs` so zero new inline `Color32` literals (`199/199`) and zero new geometry/spacing literals (`offset_literals=1525, inset_literals=41, radius_literals=171, font_literals=23`) were introduced. |
| `es:astra-work` & `es:fable-thinking` | Maintained the epistemic ledger (`FACT` / `INFERENCE` / `ASSUMPTION`), stated `PREDICT` before consequential commands, ran all Cargo gates in the hermetic sibling `HOME`/`TMPDIR` harness, and captured full command logs in `plans/reports/agy-137-oklch-picker-evidence/`. |

---

## 5. Acceptance Table

| # | Acceptance Item | Verdict | Evidence |
|---|---|---|---|
| **1** | `color_space.rs` unit tests: known values (`#000000`, `#ffffff`, `#ff0000`, `#00ff00`, `#0000ff`, `#808080` with `C == 0` for greys, `±1e-3` on `L`/`a`/`b`/`C`, `±0.05°` on `H`) | **PASS** | `color_space::tests::known_values_srgb_to_oklab_and_oklch` — RED in `42049287`, GREEN in `95d55aa1` (`plans/reports/agy-137-oklch-picker-evidence/green-color_space.log`) |
| **2** | `color_space.rs` unit tests: 17-step sRGB grid (`17³ = 4,913` colours, max channel error `<= 1e-5` in `f32` and `0` in `u8`) + out-of-gamut clamping (`oklch(0.7, 0.4, 150°)`, `L=1.5`, `L=-0.2`, `C=-0.1`, `NaN`, `±Inf`) | **PASS** | `color_space::tests::round_trip_17_step_srgb_grid_4913_colours` & `color_space::tests::gamut_clamping_and_non_finite_inputs` — RED in `42049287`, GREEN in `95d55aa1` |
| **3** | `picker.rs` unit tests: `Fmt::ALL` order (`Hex, Rgb, Css, Hsl, Hsb, Oklch, Oklab`), `Fmt::Hex` default, `#ff0000` displays `["62.8", "0.26", "29.23"]` in OKLCH and `["62.8", "0.22", "0.13"]` in OKLAB, editing `L`/`C`/`H` and `L`/`a`/`b` commits expected sRGB `Color32`, `%`/`deg`/plain/expression parsing (`"50%+10%"` -> `0.60`), and R6 precision/grey-hue retention | **PASS** | 5 new tests in `widgets::picker::tests` (85 total `picker` filter tests in `designos` bin) — RED in `42049287`, GREEN in `95d55aa1` (`plans/reports/agy-137-oklch-picker-evidence/green-picker.log`) |
| **4** | Existing picker & layout tests still pass (`picker`, `effect_stop_binds`, `layout_unchanged`, `parses_panel`) | **PASS** | `85 passed` (`picker`), `16 passed` (`effect_stop_binds`), `20 passed` (`layout_unchanged`), `1 passed` (`parses_panel`) |
| **5** | Repo gates (`cargo fmt`, `cargo clippy`, `cargo test --workspace --locked --lib`, `check_chrome_colour_literals.py`, `check_spacing_relationships.py`, `scripts/tests`, `zero_fake_ui_gate.py`) | **PASS** | All pass (clippy shows only the 5 pre-existing `main` errors documented in the brief) |
| **6** | Delete `out/cargo` at the end | **PASS** | Removed `out/cargo` after all builds, tests, and captures settled |

---

## 6. Epistemic Ledger & Assumptions

### Epistemic Ledger

- **[FACT]** `origin/main` at `0c894123` includes PR `#1425` (`feat(inspector): bind variables on layer effect rows and gradient stops (#132) (#1425)`) and has zero line-number drift against all brief anchors.
- **[FACT]** Pure sRGB `#ff0000` converts via Ottosson's matrices to OKLab `L ≈ 0.627955, a ≈ 0.224863, b ≈ 0.125846` and OKLCH `L ≈ 0.627955, C ≈ 0.257683, H ≈ 29.233885°`, which format via `fmt_num` as `["62.8", "0.26", "29.23"]` (OKLCH) and `["62.8", "0.22", "0.13"]` (OKLAB).
- **[FACT]** Every point on the 17-step sRGB grid (`4,913` colours) round-trips through both `srgb -> oklab -> srgb` and `srgb -> oklch -> srgb` with max `f32` error `< 1e-6` (`<= 1e-5` threshold) and `0` `u8` error.
- **[FACT]** `State::sync_ok` preserves the user-typed `oklch` / `oklab` triple whenever `oklch_to_srgb_u8(self.oklch) == self.rgb()` (or `oklab_to_srgb_u8(self.oklab) == self.rgb()`), preventing 8-bit `Color32` quantization from overwriting the other two channels on the next frame, and preserves `self.oklch[2]` (hue) when `C < 1e-4`.
- **[FACT]** Focused OKLCH/OKLAB cells seed their edit buffer with `fmt_ok_edit` (4 decimals, trailing zeros trimmed) while unfocused cells display `fmt_num` (2 decimals, trailing zeros trimmed), so focusing and blurring an unedited cell does not quantize the channel to 2 decimals.
- **[INFERENCE]** Because `Document` stores `Color { r, g, b, a: u8 }` until `#136` lands, two distinct OKLCH/OKLAB triples that quantize to the same 8-bit sRGB stay distinct within the active picker `State` session (`sync_ok`), while reopening the picker from the node's `Color` re-derives OKLCH/OKLAB from 8-bit sRGB.
- **[ASSUMPTION]** When a user types an arithmetic expression with `%` in an OKLCH `C` or OKLAB `a`/`b` cell (e.g. `50%+10%`), treating the expression as a percentage (`60%` → `60 * 0.004 = 0.24`) matches the brief's rule (`plain number = raw value; n% = n * 0.004`).

### Weakest Link

- **Weakest link:** Closed `55 px` format dropdown button elides the 5-letter labels `"OKLCH"` and `"OKLAB"` (`25 px` inner text budget inside `picker_dropdown`), kept at `55 px` per ruling R3 so the 3-cell `129 px` row geometry stays untouched.

### Unresolved Questions

- None.

---

## 7. Exact Commands & Pasted Outputs

### Hermetic Test Harness Setup

```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home" "$S/tmp"
export CARGO_TARGET_DIR="$PWD/out/cargo"
df -h /
```

### RED Phase (Commit `42049287`)

#### Command 1 (RED): `cargo test -p designos-doc --locked --lib color_space`

```text
running 3 tests
test color_space::tests::gamut_clamping_and_non_finite_inputs ... FAILED
test color_space::tests::round_trip_17_step_srgb_grid_4913_colours ... FAILED
test color_space::tests::known_values_srgb_to_oklab_and_oklch ... FAILED

failures:

---- color_space::tests::gamut_clamping_and_non_finite_inputs stdout ----

thread 'color_space::tests::gamut_clamping_and_non_finite_inputs' (45622535) panicked at crates/designos-doc/src/color_space.rs:156:9:
oklch(0.7 0.4 150) should clamp red to 0.0 and keep high green, got [0.0, 0.0, 0.0]

---- color_space::tests::round_trip_17_step_srgb_grid_4913_colours stdout ----

thread 'color_space::tests::round_trip_17_step_srgb_grid_4913_colours' (45622537) panicked at crates/designos-doc/src/color_space.rs:131:21:
OKLab round-trip failed for (0, 0, 1): got (0, 0, 0) vs (0, 0, 0.05882353)

---- color_space::tests::known_values_srgb_to_oklab_and_oklch stdout ----

thread 'color_space::tests::known_values_srgb_to_oklab_and_oklch' (45622536) panicked at crates/designos-doc/src/color_space.rs:73:9:
red OKLab L: expected 0.628 ± 0.001, got 0
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    color_space::tests::gamut_clamping_and_non_finite_inputs
    color_space::tests::known_values_srgb_to_oklab_and_oklch
    color_space::tests::round_trip_17_step_srgb_grid_4913_colours

test result: FAILED. 0 passed; 3 failed; 0 ignored; 0 measured; 674 filtered out; finished in 0.00s
```

#### Command 2 (RED): `cargo test -p designos-app --bin designos --locked widgets::picker::tests`

```text
running 14 tests
test widgets::picker::tests::hsb_is_computed_in_gamma_space ... ok
test widgets::picker::tests::solid_to_gradient_keeps_color_as_first_stop ... ok
test widgets::picker::tests::active_color_edits_the_selected_stop ... ok
test widgets::picker::tests::gradient_to_solid_takes_first_stop ... ok
test widgets::picker::tests::gradient_shape_switch_keeps_stops ... ok
test widgets::picker::tests::hue_survives_grey ... ok
test widgets::picker::tests::oklch_and_oklab_format_list_and_default ... FAILED
test widgets::picker::tests::parse_oklch_and_oklab_channel_rules_and_clamps ... FAILED
test widgets::picker::tests::red_shows_expected_oklch_and_oklab_cells ... FAILED
test widgets::picker::tests::stop_color_interpolates ... ok
test widgets::picker::tests::procedural_png_presets_generate_valid_png_headers_and_distinct_textures ... ok
test widgets::picker::tests::css_and_hsl_roundtrip ... ok
test widgets::picker::tests::oklch_state_preserves_typed_values_and_grey_hue ... FAILED
test widgets::picker::tests::editing_oklch_and_oklab_channels_commits_srgb_rgba ... FAILED

failures:

---- widgets::picker::tests::oklch_and_oklab_format_list_and_default stdout ----

thread 'widgets::picker::tests::oklch_and_oklab_format_list_and_default' (45717268) panicked at crates/designos-app/src/widgets/picker.rs:1817:9:
assertion `left == right` failed
  left: [Hex, Rgb, Css, Hsl, Hsb, Hex, Hex]
 right: [Hex, Rgb, Css, Hsl, Hsb, Oklch, Oklab]

---- widgets::picker::tests::parse_oklch_and_oklab_channel_rules_and_clamps stdout ----

thread 'widgets::picker::tests::parse_oklch_and_oklab_channel_rules_and_clamps' (45717270) panicked at crates/designos-app/src/widgets/picker.rs:1853:65:
C 50%
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- widgets::picker::tests::red_shows_expected_oklch_and_oklab_cells stdout ----

thread 'widgets::picker::tests::red_shows_expected_oklch_and_oklab_cells' (45717272) panicked at crates/designos-app/src/widgets/picker.rs:1831:9:
assertion `left == right` failed
  left: ["", "", ""]
 right: ["62.8", "0.26", "29.23"]

---- widgets::picker::tests::oklch_state_preserves_typed_values_and_grey_hue stdout ----

thread 'widgets::picker::tests::oklch_state_preserves_typed_values_and_grey_hue' (45717269) panicked at crates/designos-app/src/widgets/picker.rs:1942:9:
assertion failed: (red_hue - 29.233_885).abs() < 1e-2

---- widgets::picker::tests::editing_oklch_and_oklab_channels_commits_srgb_rgba stdout ----

thread 'widgets::picker::tests::editing_oklch_and_oklab_channels_commits_srgb_rgba' (45717263) panicked at crates/designos-app/src/widgets/picker.rs:1907:9:
editing OKLCH L must mark out.changed


failures:
    widgets::picker::tests::editing_oklch_and_oklab_channels_commits_srgb_rgba
    widgets::picker::tests::oklch_and_oklab_format_list_and_default
    widgets::picker::tests::oklch_state_preserves_typed_values_and_grey_hue
    widgets::picker::tests::parse_oklch_and_oklab_channel_rules_and_clamps
    widgets::picker::tests::red_shows_expected_oklch_and_oklab_cells

test result: FAILED. 9 passed; 5 failed; 0 ignored; 0 measured; 3969 filtered out; finished in 0.01s
```

### GREEN Phase & Repo Gates (Commit `95d55aa1`)

#### Acceptance 1 & 2: `cargo test -p designos-doc --locked --lib color_space`

```text
running 3 tests
test color_space::tests::known_values_srgb_to_oklab_and_oklch ... ok
test color_space::tests::gamut_clamping_and_non_finite_inputs ... ok
test color_space::tests::round_trip_17_step_srgb_grid_4913_colours ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 674 filtered out; finished in 0.00s
```

#### Acceptance 3 & 4a: `cargo test -p designos-app --bin designos --locked picker`

```text
running 85 tests
test agent_picker::tests::arrow_steps_skip_disabled_rows_and_wrap ... ok
test agent_picker::tests::closing_the_picker_discards_the_other_cli_draft ... ok
test agent_picker::tests::ask_risky_disables_clis_without_a_shared_hook ... ok
test agent_picker::tests::cold_open_settles_to_spec_order_then_stays_pinned ... ok
test agent_picker::tests::uninstalled_cli_opens_its_install_page_not_a_tab ... ok
test agent_picker::tests::ask_risky_menu_rows_without_a_hook_are_disabled_and_keep_the_reason_in_the_tooltip ... ok
test agent_picker::tests::an_install_action_opens_the_cli_install_page ... ok
test agent_picker::tests::other_cli_form_saves_and_lists_the_entry ... ok
test agent_open_flow::tests::open_flow_shows_picker_when_no_resumable_sessions ... ok
test agent_picker::tests::plus_menu_shows_the_same_list_compactly ... ok
test agent_open_flow::tests::open_with_no_sessions_shows_the_picker_and_spawns_nothing ... ok
test agent_picker::tests::a_long_other_cli_error_keeps_8px_from_the_buttons_and_stays_in_the_card ... ok
test io::file_dialog_flow_tests::other_dialog_sites::the_image_fill_picker_is_silent_on_cancel ... ok
test io::file_dialog_flow_tests::other_dialog_sites::the_image_fill_picker_says_so_and_leaves_the_paint_alone ... ok
test panels::style_picker::tests::us01_ac03_layer_with_deleted_or_orphan_style_shows_no_style_chip ... ok
test panels::style_picker::tests::us01_ac04_unnamed_style_shows_fallback_label_and_id_only_in_tooltip ... ok
test panels::style_picker::tests::the_create_row_uses_the_noun_of_the_empty_state_row ... ok
test panels::style_picker::tests::us01_ac02_orphan_styles_are_excluded_from_picker_and_reopened_designos_while_unpublished_stay_listed ... ok
test agent_picker::tests::reopening_picker_keeps_cached_result_while_starting_new_probe ... ok
test agent_picker::tests::failed_resume_last_session_keeps_the_plus_menu_open_with_a_notice ... ok
test agent_picker::tests::the_other_cli_caption_keeps_8px_from_the_buttons_and_is_not_cut ... ok
test proto::tests::animation_picker_keeps_duration_and_direction ... ok
test agent_sessions::tests::unresumable_newest_entry_leaves_picker_open_and_does_not_skip_to_older_entry ... ok
test canvas::textsession::tests::the_family_picker_keeps_its_face_on_a_large_document ... ok
test panels::style_picker::tests::us01_ac02_round2_style_entries_of_on_200k_nodes_and_50_styles_is_o1_per_key_and_under_budget ... ok
test agent_picker::tests::custom_cli_saved_in_fallback_mode_uses_library_root ... ok
test agent_picker::tests::clicking_row_mid_staggered_probe_starts_the_clicked_cli ... ok
test agent_open_flow::tests::closing_last_tab_shows_picker ... ok
test agent_open_flow::tests::closing_the_last_tab_shows_the_picker ... ok
test shell::tests::assets_variables_libraries_chrome::variable_picker_rows_are_menu_rows ... ok
test shell::tests::agent_picker_return::bare_return_in_the_agents_panel_starts_no_session ... ok
test shell::tests::agent_picker_return::return_on_the_row_the_arrows_reached_opens_it ... ok
test shell::tests::agent_picker_return::arrows_move_focus_between_selectable_rows_and_wrap ... ok
test shell::tests::agent_picker_return::no_row_looks_focused_until_the_keyboard_is_on_one ... ok
test shell::tests::assets_variables_libraries_chrome::variables_cell_colour_picker_keeps_the_field_outline ... ok
test agent_open_flow::tests::cmd_k_prompt_uses_resume_or_picker_never_first_installed ... ok
test shell::tests::agent_picker_return::arrows_and_return_with_no_row_focused_leave_the_picker_to_the_canvas ... ok
test agent_sessions::tests::closing_the_last_agent_tab_shows_the_picker_and_does_not_resume ... ok
test shell::tests::agent_simplified_panel::the_agents_picker_capture_panel_opens_the_agent_menu_and_the_agents_panel_does_not ... ok
test shell::tests::color_swatch_opens_picker_without_panic ... ok
test shell::tests::agent_simplified_panel::tab_click_clears_picker_open_and_new_chat_card_reads_open_tab ... ok
test shell::tests::board::shapes_are_created_with_text_and_the_shape_picker_changes_what_is_placed ... ok
test shell::tests::agent_picker_return::tab_reaches_a_row_and_return_acts_on_that_row_only ... ok
test shell::tests::agent_picker_return::the_plus_menu_ignores_arrows_and_return_until_a_row_has_focus ... ok
test shell::tests::agent_picker_return::the_plus_menu_moves_focus_with_arrows_and_returns_on_the_focused_row ... ok
test shell::tests::effect_stop_binds::picker_resync_runs_in_picker_popup_not_unrelated_sections ... ok
test shell::tests::empty_slot_flows::a_drop_elsewhere_after_the_plus_picker_does_not_go_into_the_still_selected_slot ... ok
test shell::tests::effect_stop_binds::probe_stop_bind_after_a_picker_edit_survives_the_next_picker_edit ... ok
test skills::hook_delivery::tests::the_slots_hold_the_largest_set_the_picker_allows_however_its_lines_fall ... ok
test skills::picker_popover_tests::skills_popover_rows_hold_name_description_and_check_only ... ok
test skills::picker_popover_tests::the_budget_line_shows_only_near_or_over_a_limit ... ok
test skills::picker_popover_tests::thousands_groups_by_three ... ok
test shell::tests::overlays_chrome_layout::context_menu_and_layer_picker_rows_are_menu_items ... ok
test shell::tests::picker_libraries_tooltip::colour_picker_libraries_tab_shows_style_description_tooltip_only_when_non_empty ... ok
test shell::tests::empty_slot_flows::reset_after_filling_an_empty_slot_with_the_picker_brings_the_placeholder_back ... ok
test terminal::tests::the_install_check_reads_the_pickers_cached_probes_not_the_path ... ok
test widgets::picker::tests::active_color_edits_the_selected_stop ... ok
test widgets::picker::tests::css_and_hsl_roundtrip ... ok
test widgets::picker::tests::editing_oklch_and_oklab_channels_commits_srgb_rgba ... ok
test widgets::picker::tests::gradient_shape_switch_keeps_stops ... ok
test widgets::picker::tests::gradient_to_solid_takes_first_stop ... ok
test widgets::picker::tests::hsb_is_computed_in_gamma_space ... ok
test widgets::picker::tests::hue_survives_grey ... ok
test widgets::picker::tests::oklch_and_oklab_format_list_and_default ... ok
test widgets::picker::tests::oklch_state_preserves_typed_values_and_grey_hue ... ok
test widgets::picker::tests::parse_oklch_and_oklab_channel_rules_and_clamps ... ok
test widgets::picker::tests::procedural_png_presets_generate_valid_png_headers_and_distinct_textures ... ok
test widgets::picker::tests::red_shows_expected_oklch_and_oklab_cells ... ok
test shell::tests::effect_stop_binds::eyedropper_sample_closes_picker_and_commits_one_undo_step ... ok
test widgets::picker::tests::solid_to_gradient_keeps_color_as_first_stop ... ok
test widgets::picker::tests::stop_color_interpolates ... ok
test shell::tests::libraries::creating_a_style_from_the_picker_is_one_undo_step ... ok
test shell::tests::effect_stop_binds::unbound_gradient_picker_session_with_flip_stays_one_undo_step ... ok
test skills::tests::choosing_skills_in_the_picker_attaches_nothing_and_a_refused_send_keeps_run_json ... ok
test shell::tests::variables::bind_button_opens_picker_and_binds_a_fill ... ok
test shell::tests::components::empty_slot_placeholder_preferred_picker_and_stretch_on_insert_ui ... ok
test skills::tests::run_json_not_overwritten_by_page_sync_or_picker_open ... ok
test shell::tests::libraries::color_picker_libraries_tab_lists_library_styles_and_applies_one_undoably ... ok
test shell::tests::components::slot_preferred_picker_tells_same_named_components_apart_by_id ... ok
test shell::tests::components::slot_preferred_picker_checks_and_shows_every_selected_component ... ok
test shell::tests::libraries::library_variables_bind_through_the_picker_in_one_undo_step ... ok
test skills::tests::picker_scrolls_to_reach_all_100_skills_and_ephemeral_selection_preserves_settings ... ok
test shell::tests::libraries::library_variables_bind_to_a_style_through_the_picker_in_one_undo_step ... ok
test shell::tests::proto::popover_action_dropdown_switches_kind_and_destination_picker_lists_frames ... ok
test shell::tests::style_edit::style_picker_and_styles_list_show_description_tooltip_only_when_non_empty ... ok

test result: ok. 85 passed; 0 failed; 0 ignored; 0 measured; 3898 filtered out; finished in 5.96s
```

#### Acceptance 4b: `cargo test -p designos-app --bin designos --locked effect_stop_binds`

```text
running 16 tests
test shell::tests::effect_stop_binds::multiselect_bound_gradient_stop_locks_all_selected_layers_and_shows_mixed_pill ... ok
test shell::tests::effect_stop_binds::dangling_gradient_stop_binding_edit_is_kept ... ok
test shell::tests::effect_stop_binds::multiselect_stroke_gradient_bound_stop_preserves_bound_color_on_every_layer ... ok
test shell::tests::effect_stop_binds::probe_stop_bind_after_a_picker_edit_survives_the_next_picker_edit ... ok
test shell::tests::effect_stop_binds::dragging_gradient_stop_off_bar_deletes_at_most_one_stop_per_drag ... ok
test shell::tests::effect_stop_binds::picker_resync_runs_in_picker_popup_not_unrelated_sections ... ok
test shell::tests::effect_stop_binds::canvas_gradient_line_insert_remaps_stop_binding_and_undoes_in_one_step ... ok
test shell::tests::effect_stop_binds::gradient_stop_colors_bind_filter_scopes_detach_and_undo ... ok
test shell::tests::effect_stop_binds::bound_gradient_stop_locks_color_controls_until_detached ... ok
test shell::tests::effect_stop_binds::gradient_stop_bindings_follow_stop_across_flip_add_and_delete ... ok
test shell::tests::effect_stop_binds::effect_popover_opens_via_panel_flag_not_screenshot_chrome_side_effect ... ok
test shell::tests::effect_stop_binds::eyedropper_sample_closes_picker_and_commits_one_undo_step ... ok
test shell::tests::effect_stop_binds::dangling_only_gradient_stop_flip_session_stays_one_undo_step_and_remaps_key ... ok
test shell::tests::effect_stop_binds::unbound_gradient_picker_session_with_flip_stays_one_undo_step ... ok
test shell::tests::effect_stop_binds::deleting_effect_drops_its_bindings_and_shifts_later_effect_bindings_down ... ok
test shell::tests::effect_stop_binds::node_effect_rows_bind_filter_scopes_detach_and_undo ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 3967 filtered out; finished in 4.11s
```

#### Acceptance 4c: `cargo test -p designos-app --bin designos --locked layout_unchanged`

```text
running 20 tests
test shell::tests::layout_unchanged::panel_presets::cli_accepts_panel_comments ... ok
test shell::tests::layout_unchanged::cases_json_has_no_old_fixture ... ok
test shell::tests::layout_unchanged::cases_json_and_allow_json_match_spec_l_contract ... ok
test shell::tests::layout_unchanged::new_named_widget_is_reported_not_failed ... ok
test shell::tests::layout_unchanged::unblessed_cases_file_fails_the_gate ... ok
test shell::tests::layout_unchanged::prep_helpers_populate_missing_starter_states ... ok
test shell::tests::layout_unchanged::panel_presets::interaction_panel_with_no_reaction_opens_the_prototype_tab_only ... ok
test shell::tests::layout_unchanged::panel_presets::comments_panel_with_no_thread_opens_comment_mode_only ... ok
test shell::tests::layout_unchanged::panel_presets::comments_panel_opens_the_first_unresolved_thread_in_paint_order ... ok
test shell::tests::layout_unchanged::panel_presets::interaction_panel_without_a_reacting_selection_picks_the_first_reaction_in_paint_order ... ok
test shell::tests::layout_unchanged::panel_presets::interaction_panel_opens_the_selected_nodes_first_reaction ... ok
test shell::tests::layout_unchanged::h::layout_unchanged_h ... ok
test shell::tests::layout_unchanged::sim_loads_starter_and_seed_library_at_seeds_starter_not_showcase ... ok
test shell::tests::layout_unchanged::e::layout_unchanged_e ... ok
test shell::tests::layout_unchanged::k::layout_unchanged_k ... ok
test shell::tests::layout_unchanged::g::layout_unchanged_g ... ok
test shell::tests::layout_unchanged::d::layout_unchanged_d ... ok
test shell::tests::layout_unchanged::i::layout_unchanged_i ... ok
test shell::tests::layout_unchanged::j::layout_unchanged_j ... ok
test shell::tests::layout_unchanged::f::layout_unchanged_f ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 3963 filtered out; finished in 9.32s
```

#### Acceptance 4d: `cargo test -p designos-app --bin designos --locked parses_panel`

```text
running 1 test
test cli::tests::parses_panel ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3982 filtered out; finished in 0.00s
```

#### Acceptance 5a: `cargo fmt --all -- --check`

```text
(exit code 0, zero diff)
```

#### Acceptance 5b: `cargo clippy --workspace --all-targets --locked -- -D warnings`

```text
error: unneeded `return` statement
    --> crates/designos-app/src/agent_permissions.rs:1163:9
error: unneeded `return` statement
    --> crates/designos-app/src/agent_permissions.rs:1181:9
error: writing `&mut Vec` instead of `&mut [_]` involves a new object where a slice will do
   --> crates/designos-app/src/panels/components.rs:409:29
error: items after a test module
   --> crates/designos-app/src/panels/components.rs:263:1
error: unused return value of `egui::Context::run_ui` that must be used
   --> crates/designos-app/src/panels/components.rs:363:9
```
*(Exact 5 pre-existing `main` errors listed in the brief; 0 warnings/errors in `designos-doc` or `widgets/picker.rs`.)*

#### Acceptance 5c: `cargo test --workspace --locked --lib`

```text
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_diff-954071a99f0355be)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_doc-54b030362ab79b6a)
test result: ok. 677 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.90s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_eval-b0fb06508a5f1957)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_export-cee0f0ff60625584)
test result: ok. 153 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.21s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fig-e6b5e6d2fb73e410)
test result: ok. 143 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.01s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fonts-1cac0c57d5614e3e)
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_gpu-cf69135250c23b7e)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_library-6a5434ac6eccfb94)
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.18s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_mcp-d70a05cbf567a7e3)
test result: ok. 296 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.67s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_render-34b07dcd084a67a8)
test result: ok. 56 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 34.01s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_svg-d0464f00b06bb177)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_webcapture-60a1e05e125e03e8)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
```

#### Acceptance 5d: `python3 scripts/check_chrome_colour_literals.py`

```text
Chrome colour literal gate PASSED (29 files with findings, 199/199 baseline chrome sites).
```

#### Acceptance 5e: `python3 scripts/check_spacing_relationships.py`

```text
literal geometry outside theme.rs: offset_literals=1525, inset_literals=41, radius_literals=171, font_literals=23, nonconcentric_pairs=2, legacy_control_refs=6
note: 1 entries are below baseline; run --write-baseline to lock the gain in
PASS: no count rose above the baseline
```

#### Acceptance 5f: `python3 -m unittest discover -s scripts/tests`

```text
...............................................................................................................................................................................................................
----------------------------------------------------------------------
Ran 207 tests in 51.454s

OK
```

#### Acceptance 5g: `python3 scripts/zero_fake_ui_gate.py`

```text
============================================================================================================
 design:os studio — Zero Fake UI Enforcement Gate (DIR-28 · BR-31 · REQ-25 · D25)
============================================================================================================
ID    | STATUS | MODULE                           | INVARIANT & EVIDENCE
------------------------------------------------------------------------------------------------------------
Z01   | PASS   | terminal.rs + agent_composer.rs  | Live PTY send_to_agent in submit_chat_prompt; zero one-shot (-p / exec) calls
Z02   | PASS   | terminal.rs                      | set_active_runtime mutates tab.preset; DESIGNOS_AGENT_RUNTIME set at spawn; one-shot chat symbols removed
Z03   | PASS   | terminal.rs                      | dispatch_figma_ai_prompt sends executable command without # comment prefix
Z04   | PASS   | terminal.rs + agent_header.rs + agent_composer_toolbar.rs | Agents header and composer buttons have Sense::click() handlers; fake mic removed
Z05   | PASS   | specs.rs                         | specs-ask-agent-bar hosts editable TextEdit::singleline(ask_input)
Z06   | PASS   | specs.rs                         | Right Inspector dynamic active_section_toc & interactive SpecComment
Z07   | PASS   | specs.rs                         | Unwrapped draw_wrapped_prose & render_tab_blueprints renders pkg.ui_contract
Z08   | PASS   | panels/toolbar.rs + cmdk.rs      | Contextual sub-toolbar buttons mutate app.doc / app.tool; ctx-crop / ctx-remove-bg / cmdk Remove background stay removed
Z09   | PASS   | home/mod.rs                      | Act::ExportFig writes .fig to disk & Nav::AllFolders/Templates/Libraries filter distinctly
Z10   | PASS   | panels/right.rs                  | Share copies to clipboard & Dev Mode 2nd tab renders Dev Plugins inspector
Z11   | PASS   | panels/inspect.rs + properties.rs | Dev Mode Code Connect/MCP/Redlines/Keyframes & Copy to Slides mutate state/clipboard
Z12   | PASS   | widgets/picker.rs + panels/mod.rs + variables/view.rs | Mode::Image creates Paint::Image/doc.images & scrollbars support Sense::click_and_drag()
Z13   | PASS   | panels/menus.rs                  | Zoom menu rows are the allow-listed set on real Actions; no hard-coded checked state
Z14   | PASS   | updater.rs + tabs.rs             | Sample-manifest updater UI stays hidden (guard is code in tabs, sidebar card, modal) until a real manifest fetch exists
Z15   | PASS   | terminal.rs + cmdk.rs            | No fake template synthesis or canned Executed via transcript in Agents panel; synthesize_spec_from_prompt stays private to cmdk.rs
Z16   | PASS   | registered label helpers in designos-app/src | UX writing: action labels at registered button/menu helpers and label fns <=3 words, no count in them (ratchet vs ux_writing_baseline.json)
Z17   | PASS   | terminal.rs + mcp/server.rs      | No global /tmp/designos-live-* or designos-live-selection.spec.json fallback in shipped code
------------------------------------------------------------------------------------------------------------
 Gate Verdict: PASS (17/17 invariants satisfied)
============================================================================================================
```

Status: DONE
