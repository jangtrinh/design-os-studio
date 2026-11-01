# Report: Always-Visible Variable Bind Icon & Typography Bind Entry Points (`#876`, `#133`)

- **Branch:** `jangtrinh/agy-876-133-inspector-binds`
- **Base:** `origin/main` (`64f2dd07` in Round 2; `0c894123` in Round 1)
- **Issues:** Closes `#876` · Part of `#133` (inspector half)

## Not Verified / Known Limitations

1. **Interactive macOS Window Server Hover / Cursor Verification:** All Round 2 layout, hit-testing, colour, and proof-sheet checks (`r2-typography.png`) were rendered headlessly via `Sim::shapes` and `designos-render` software rasterization; live OS window-server cursor changes on hover were not manually exercised in a GUI window session.
2. **Style-Token Undo Coverage (`#1448`):** Binding a typography variable on a node with a linked `StyleKind::Text` detaches the text style inside the same `Tx` (`"Bind Variable"` / `"Bind Library Variable"`), so a single Undo restores both the style ref and the previous binding state (per controller ruling, `#1448` is out of scope for this PR).
3. **Legacy `fontWeight` Variable Picker Creation:** Legacy `fontWeight` (`VariableKind::Number`) bindings on text nodes render as bound pills in the weight slot and can be detached via the pill `×` button, while new bindings from the weight dropdown's `"Apply variable…"` item bind `fontStyle` (`VariableKind::String`).
4. **Pre-existing Clippy Lints Outside Scope:** On `origin/main` (`64f2dd07`), `crates/designos-app/src/agent_permissions.rs` (lines 1176 and 1194) has 2 pre-existing `clippy::needless_return` warnings under `-D warnings` and `crates/designos-app/src/panels/components.rs` (lines 68, 76, 179) has 3 pre-existing `collapsible_if` / `manual_Last` warnings. Both files are outside this PR's allowed file scope; all files touched by this PR pass `cargo clippy` with zero warnings.

## Re-Anchor Drift

- [`crates/designos-app/src/panels/var_bind.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/var_bind.rs): `774` lines at Round 1 start (`769` lines at Round 2 start after merging `origin/main` `64f2dd07`) → `779` lines in Round 2.
  - Replaced per-call library `Vec` cloning in `CachedLibVars` with a per-pass 64-bit `AvailabilityTable` (`pass_nr`, `local_vars_len`, `enabled_len`, `bits: [u32; 4]`) backed by `Arc<[(VariableKind, Vec<String>)]>` cached across passes by `lib_meta_stamp`.
  - Changed `draw_hover_button` signature to accept `record_name: impl FnOnce() -> String` and call `widgets::record_with` so zero `format!` allocations occur when layout snapshot recording is off.
  - Centred the `24×24` (`theme::TARGET_MIN`) bind icon vertically on `rect.center().y` (`Rect::from_center_size(Pos2::new(rect.right() - theme::TARGET_MIN / 2.0, rect.center().y), Vec2::splat(theme::TARGET_MIN))`).
  - Delegated non-text-style keys (`!is_text_style_key(&key0)`) in `bind_picker` to `app.bind_variable` / `app.bind_library_variable` and removed the redundant `if key0 == "fontFamily"` font-loading block (`textsession::requesting_new_faces` already loads newly requested faces).
- [`crates/designos-app/src/panels/typography.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/typography.rs): `1,010` lines in Round 2.
  - During an active text editing session (`in_session`), bound fields (`fontFamily`, `fontStyle`/`fontWeight`, `fontSize`, `lineHeight`, `letterSpacing`, `fills/0`, `paragraphIndent`, `paragraphSpacing`) still render their bound `var_bind::pill_field` so bound properties remain locked; only unbound fields suppress `value_rect` narrowing and `hover_button` during `in_session`.
  - Removed the `25 px` bind icon reserve and `hover_button` from the 88 px weight dropdown (`wt_r`) so weight names (`Semi Bold`, `Extra Bold`, etc.) display in full; added `"Apply variable…"` after a separator at the bottom of the weight dropdown menu via `dropdown_items` when `!in_session` and a matching `fontStyle` (`VariableKind::String`) variable exists.
- `value_rect` / `style_value_rect` call sites across `crates/designos-app/src/panels/`: `13` call sites (`2` in `var_bind.rs`, `3` in `style_edit.rs`, `3` in `typography.rs`, `2` in `effects.rs`, `2` in `properties.rs`, plus `2` fn definitions in `var_bind.rs` = `15` occurrences; reduced by 1 from Round 1 after removing the weight dropdown reserve in `MAJOR-3`).
- `hover_button` / `style_hover_button` call sites across `crates/designos-app/src/panels/`: `12` call sites (`4` in `var_bind.rs`, `3` in `style_edit.rs`, `2` in `effects.rs`, `3` in `typography.rs`, `1` in `properties.rs`, plus `2` fn definitions in `var_bind.rs` = `14` occurrences; reduced by 1 from Round 1 after removing the weight dropdown `hover_button` in `MAJOR-3`).
- Layout baselines & allow-list: `49` `.layout.json` files under `crates/designos-app/tests/layout-baseline/`, [`allow.json`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/tests/layout-baseline/allow.json) (deleted 21 `micro-*` entries and 2 zero-drift `canvas` `h_base: 24` entries), and [`crates/designos-app/src/layout_snapshot.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/layout_snapshot.rs) (`check_stale_allow_entries_for_run` now checks the union of specs in a run and flags any drift-bound entry whose `covered_with_drift == 0`).

## Assumptions

1. `egui::Context::cumulative_pass_nr()` monotonically advances per egui layout pass, and `(pass_nr, app.doc.variable_collections.len(), app.libraries.enabled.len())` uniquely identifies the variable availability state within a pass unless `invalidate_avail_cache(ctx)` is called on mid-pass mutations.
2. Subscribed library variable metadata only changes when `app.lib_meta_stamp` changes (`u64::MAX` when no libraries are enabled), making `Arc<[(VariableKind, Vec<String>)]>` safe to reuse across passes without re-cloning.
3. On the 88 px weight dropdown (`wt_r`), `"Apply variable…"` is appended after a separator via `dropdown_items` only when `!in_session` and `has_matching_var(app, ui.ctx(), "fontStyle", VariableKind::String, target_kind)` is true, opening the same `var_bind::open_bind_picker_for` popover anchored to `wt_r`.

## Skills Loaded

- `es:astra-work` — Evidence-backed execution loop, red-before-green discipline, and post-mutation verification.
- `es:fable-thinking` — Epistemic ledger (`FACT` / `INFERENCE` / `ASSUMPTION`), falsification-first root-cause analysis, and mutation testing.
- `es:rust` — Zero-allocation hot paths, bitset table caching, and `Arc` sharing across UI frames.
- `es:rust-gpu-canvas` — Headless `Sim::shapes` verification and `designos-render` proof-sheet rasterization.

---

## Round 2

### 1. Controller Rulings & Verification Matrix

| Ruling | Fix Summary | File : Line | Test Name(s) | Status |
|---|---|---|---|---|
| **BLOCKER-1** | Keep `var_bind::pill_field` active during `in_session` across all 8 bound typography fields (`fontFamily`, `fontStyle`/`fontWeight`, `fontSize`, `lineHeight`, `letterSpacing`, `fills/0`, `paragraphIndent`, `paragraphSpacing`) so bound values stay locked; hide only the unbound bind icon and reserve during `in_session`. | [`typography.rs:202-238`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/typography.rs#L202-L238), [`typography.rs:365-384`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/typography.rs#L365-L384), [`var_bind.rs:568-585`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/var_bind.rs#L568-L585) | `inspector_var_binds_typography_bind_picker_and_style_detach_undo` | **PASS** |
| **MAJOR-2** | Build one 64-bit `AvailabilityTable` (`[u32; 4]`) per egui pass (`pass_nr`) and cache library `(VariableKind, Vec<String>)` behind an `Arc` keyed by `lib_meta_stamp`; pass lazy `FnOnce() -> String` into `widgets::record_with` so zero `format!` runs when layout recording is off. | [`var_bind.rs:146-282`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/var_bind.rs#L146-L282), [`var_bind.rs:425-445`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/var_bind.rs#L425-L445) | `inspector_var_binds_allocation_budget_with_700_library_variables` | **PASS** (`1,646` base vs `1,747` lib, `+6.1%`) |
| **MAJOR-3** | Weight dropdown (`wt_r`, 88 px) has no bind icon and no `25 px` reserve so `Semi Bold` / `Extra Bold` show in full; binding is triggered via `"Apply variable…"` after a separator at the bottom of the weight dropdown (`dropdown_items`), gated on `!in_session` and matching `fontStyle` (`VariableKind::String`) availability. Bound `fontStyle` or legacy `fontWeight` still renders the bound pill in `wt_r`. | [`typography.rs:217-240`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/typography.rs#L217-L240), [`typography.rs:444-472`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/typography.rs#L444-L472), [`var_bind.rs:514-530`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/var_bind.rs#L514-L530) | `inspector_var_binds_typography_bind_picker_and_style_detach_undo` | **PASS** |
| **MINOR-4** | Vertically centre the `24×24` (`theme::TARGET_MIN`) bind icon on `rect.center().y` (`top + 4.0` on 32 px rows) via `Rect::from_center_size(Pos2::new(rect.right() - theme::TARGET_MIN / 2.0, rect.center().y), Vec2::splat(theme::TARGET_MIN))`. | [`var_bind.rs:436-439`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/var_bind.rs#L436-L439) | `inspector_var_binds_always_visible_at_rest_and_opens_picker_on_single_click`, `inspector_var_binds_no_overlap_between_value_field_and_bind_hitbox` | **PASS** |
| **MINOR-5** | Strengthened `inspector_var_binds` assertions to kill all 18 reviewer mutants (`M1`–`M18`): painted `field_rect` vs icon rect checks on inspector, effect, popover, and style-editor rows; `ICON_QUATERNARY` at-rest vs `ICON` hover stroke colour check; gradient-stop hover-only check; `FONT_SIZE`-only variable excluded from `lineHeight` picker; undo after each bind; font-loading check on `fontFamily` bind; Color-only doc check on Number fields. | [`inspector_var_binds.rs:1-914`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/shell/tests/inspector_var_binds.rs#L1-L914) | All 9 tests in `inspector_var_binds` (`18/18` mutants killed) | **PASS** |
| **MINOR-6** | Updated layout baselines for centred `bind:*` y-coordinates and full-width `font-weight`; deleted 21 `micro-*` entries and 2 stale `canvas` `h_base: 24` entries from `allow.json`; updated `check_stale_allow_entries_for_run` to validate across the union of specs in a run and reject drift-bound exceptions (`min_dx`/`max_dx`/`min_dy`/`max_dy`/`min_dw`/`max_dw`/`min_dh`/`max_dh`) when `covered_with_drift == 0`. | [`layout_snapshot.rs:766-850`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/layout_snapshot.rs#L766-L850), [`allow.json`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/tests/layout-baseline/allow.json), `tests/layout-baseline/*.layout.json` | `stale_check_counts_covering_entries_and_runs_over_union_of_run`, `layout_unchanged` (`20 passed`), `layout_snapshot` (`37 passed`) | **PASS** |
| **MINOR-7** | In `bind_picker` (`BindTarget::Node`), delegate keys where `!is_text_style_key(&key0)` directly to `app.bind_variable` / `app.bind_library_variable` instead of duplicating the inline `Tx` block. | [`var_bind.rs:733-774`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/var_bind.rs#L733-L774) | `inspector_var_binds_always_visible_at_rest_and_opens_picker_on_single_click`, `inspector_var_binds_per_side_stroke_and_counter_gap` | **PASS** |
| **MINOR-8** | Removed the redundant `if key0 == "fontFamily" { ... }` font-loading block in `var_bind.rs`; `textsession::requesting_new_faces` already detects and loads newly bound font families. | [`var_bind.rs:724-777`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/var_bind.rs#L724-L777) | `inspector_var_binds_typography_bind_picker_and_style_detach_undo` (verified by `M16_remove_requesting_new_faces`) | **PASS** |
| **MINOR-9** | Corrected Round 1 lib test total (`1,416` in R1; `1,474` in R2), removed inaccurate `LineHeight::Px or Auto` / `LetterSpacing::Px` qualifiers and "read-only" wording for legacy `fontWeight` pills, and added `Not Verified / Known Limitations`, `Re-Anchor Drift`, `Assumptions`, `Skills Loaded`, and `Round 2` sections to this report. | [`agy-876-133-inspector-binds.md`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds.md) | N/A (report accuracy) | **PASS** |

### 2. Round 2 RED & GREEN Gate Commands

All Round 2 evidence logs and the proof sheet are stored in [`plans/reports/agy-876-133-inspector-binds-evidence/`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/):

| Gate | Command | Result | Evidence |
|---|---|---|---|
| **Step 1 — RED Tests** (before R2 production edits) | `cargo test -p designos-app --bin designos --locked -- inspector_var_binds stale_check_counts_covering_entries_and_runs_over_union_of_run` | `7 failed; 3 passed` (expected RED on unedited R1 code) | [`r2-red.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/r2-red.log) |
| **Step 3 — 18 Reviewer Mutants** | `cargo test -p designos-app --bin designos --locked -- inspector_var_binds` across `M1`..`M18` | `18/18 KILLED` | [`r2-mutants.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/r2-mutants.log) |
| **Step 4 — Typography Proof Sheet** | `cargo test -p designos-app --bin designos --locked -- inspector_var_binds_generate_r2_typography_proof_sheet` | `2554×1188` PNG rendered via `Sim::shapes` + `designos-render` and visually inspected with `read_file` | [`r2-typography.png`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/r2-typography.png) |
| **Step 5.1 — Rustfmt** | `cargo fmt --all -- --check` | `PASS` (`fmt_exit=0`) | [`r2-green.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/r2-green.log) |
| **Step 5.2 — Clippy** | `cargo clippy --workspace --all-targets --locked --keep-going -- -D warnings` + `cargo clippy --workspace --all-targets --locked` | Zero warnings in all touched files (only the 2 pre-existing `needless_return` in untouched `agent_permissions.rs` and 3 pre-existing warnings in untouched `components.rs`) | [`r2-green.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/r2-green.log) |
| **Step 5.3 — App Inspector / Typography / Layout Tests** | `cargo test -p designos-app --bin designos --locked -- inspector typography var_bind layout_unchanged layout_snapshot` | `130 passed; 0 failed` (`1 + 1 + 128`) | [`r2-green.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/r2-green.log) |
| **Step 5.4 — Workspace Lib Tests** | `cargo test --workspace --locked --lib` | `1,474 passed; 0 failed` across all 12 workspace lib targets | [`r2-green.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/r2-green.log) |
| **Step 5.5 — Chrome Colour Literals** | `python3 scripts/check_chrome_colour_literals.py` | `PASSED` (`199/199` baseline chrome sites) | [`r2-green.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/r2-green.log) |
| **Step 5.6 — Zero Fake UI Gate** | `python3 scripts/zero_fake_ui_gate.py` | `PASS (17/17 invariants satisfied)` | [`r2-green.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/r2-green.log) |

### 3. Allocation Benchmark (`MAJOR-2`)

Measured in `inspector_var_binds_allocation_budget_with_700_library_variables` using the global counting allocator (`alloc_count()`) over 10 steady-state frames after 2 warm-up frames with `"Hero"` selected:

- **Baseline (`base_allocs`, 0 local variables, 0 library variables):** `1,646` median allocations/frame
- **With 700-variable enabled library (`lib_allocs`, 0 local variables, 700 library variables):** `1,747` median allocations/frame (`+101` allocations/frame, `+6.1%`, well inside the `<= base_allocs * 11 / 10 = 1,810` budget; down from `49,118` allocations/frame in Round 1)

### 4. 18 Reviewer Mutants Table (`r2-mutants.log`)

Every mutant was applied one at a time to the Round 2 production code, tested with `cargo test -p designos-app --bin designos --locked -- inspector_var_binds`, and reverted immediately:

| # | Mutant ID | File | One-Line Diff | Status | Killing Test(s) |
|---|---|---|---|---|---|
| 1 | `M1_has_matching_var_true` | `var_bind.rs` | `has_matching_var` body replaced with `true` | **KILLED** | `inspector_var_binds_always_visible_at_rest_and_opens_picker_on_single_click`, `inspector_var_binds_hidden_when_no_matching_variables_exist`, `inspector_var_binds_typography_bind_picker_and_style_detach_undo` |
| 2 | `M2_has_matching_var_false` | `var_bind.rs` | `has_matching_var` body replaced with `false` | **KILLED** | `inspector_var_binds_always_visible_at_rest_and_opens_picker_on_single_click`, `inspector_var_binds_enabled_library_variables_surface_bind_button`, `inspector_var_binds_no_overlap_between_value_field_and_bind_hitbox`, `inspector_var_binds_per_side_stroke_and_counter_gap`, `inspector_var_binds_style_edit_modal_and_effect_rows`, `inspector_var_binds_typography_bind_picker_and_style_detach_undo`, `inspector_var_binds_typography_details_popover_paragraph_fields` |
| 3 | `M3_scope_allows_true` | `var_bind.rs` | `scope_allows` body replaced with `true` | **KILLED** | `inspector_var_binds_scope_filtering_hides_and_shows_matching_fields`, `inspector_var_binds_typography_bind_picker_and_style_detach_undo` |
| 4 | `M4_value_rect_noop` | `var_bind.rs` | `value_rect` body replaced with `rect` | **KILLED** | `inspector_var_binds_always_visible_at_rest_and_opens_picker_on_single_click`, `inspector_var_binds_no_overlap_between_value_field_and_bind_hitbox`, `inspector_var_binds_per_side_stroke_and_counter_gap`, `inspector_var_binds_scope_filtering_hides_and_shows_matching_fields`, `inspector_var_binds_style_edit_modal_and_effect_rows`, `inspector_var_binds_typography_bind_picker_and_style_detach_undo`, `inspector_var_binds_typography_details_popover_paragraph_fields` |
| 5 | `M5_value_rect_always_shrink` | `var_bind.rs` | `value_rect` always shrinks by `25 px` | **KILLED** | `inspector_var_binds_always_visible_at_rest_and_opens_picker_on_single_click`, `inspector_var_binds_hidden_when_no_matching_variables_exist`, `inspector_var_binds_scope_filtering_hides_and_shows_matching_fields` |
| 6 | `M6_hover_button_noop` | `var_bind.rs` | `hover_button` body replaced with `{}` | **KILLED** | `inspector_var_binds_always_visible_at_rest_and_opens_picker_on_single_click`, `inspector_var_binds_enabled_library_variables_surface_bind_button`, `inspector_var_binds_no_overlap_between_value_field_and_bind_hitbox`, `inspector_var_binds_per_side_stroke_and_counter_gap`, `inspector_var_binds_scope_filtering_hides_and_shows_matching_fields`, `inspector_var_binds_style_edit_modal_and_effect_rows`, `inspector_var_binds_typography_bind_picker_and_style_detach_undo`, `inspector_var_binds_typography_details_popover_paragraph_fields` |
| 7 | `M7_always_visible_ignored` | `var_bind.rs` | `let show = hovered \|\| is_open;` in `draw_hover_button` | **KILLED** | `inspector_var_binds_always_visible_at_rest_and_opens_picker_on_single_click`, `inspector_var_binds_per_side_stroke_and_counter_gap`, `inspector_var_binds_style_edit_modal_and_effect_rows`, `inspector_var_binds_typography_bind_picker_and_style_detach_undo`, `inspector_var_binds_typography_details_popover_paragraph_fields` |
| 8 | `M8_typography_in_session_flipped` | `typography.rs` | `let in_session = true;` | **KILLED** | `inspector_var_binds_no_overlap_between_value_field_and_bind_hitbox`, `inspector_var_binds_scope_filtering_hides_and_shows_matching_fields`, `inspector_var_binds_typography_bind_picker_and_style_detach_undo` |
| 9 | `M9_typography_fills0_not_narrowed` | `typography.rs` | `let val_hex_r = hex_r;` on `fills/0` | **KILLED** | `inspector_var_binds_no_overlap_between_value_field_and_bind_hitbox`, `inspector_var_binds_typography_bind_picker_and_style_detach_undo` |
| 10 | `M10_stroke_side_wrong_key` | `stroke.rs` | `strokeRightWeight` changed to `strokeTopWeight` | **KILLED** | `inspector_var_binds_per_side_stroke_and_counter_gap` |
| 11 | `M11_stop_row_always_visible` | `var_bind.rs` | `picker_stop_hover_button` passes `always_visible = true` | **KILLED** | `inspector_var_binds_always_visible_at_rest_and_opens_picker_on_single_click` |
| 12 | `M12_icon_color_secondary_at_rest` | `var_bind.rs` | `ICON_QUATERNARY` changed to `ICON_SECONDARY` | **KILLED** | `inspector_var_binds_always_visible_at_rest_and_opens_picker_on_single_click` |
| 13 | `M13_detach_text_style_removed` | `var_bind.rs` | Removed `style_refs.remove(StyleKind::Text.ref_key())` | **KILLED** | `inspector_var_binds_typography_bind_picker_and_style_detach_undo` |
| 14 | `M14_effects_color_no_shrink` | `effects.rs` | `let val_r = rc;` on effect color row | **KILLED** | `inspector_var_binds_style_edit_modal_and_effect_rows` |
| 15 | `M15_style_edit_family_no_shrink` | `style_edit.rs` | `field_box(ui, rc)` instead of `field_box(ui, val_r)` on `fontFamily` | **KILLED** | `inspector_var_binds_style_edit_modal_and_effect_rows` |
| 16 | `M16_remove_requesting_new_faces` | `var_bind.rs` | Bypassed `textsession::requesting_new_faces` in `bind_picker` | **KILLED** | `inspector_var_binds_typography_bind_picker_and_style_detach_undo` |
| 17 | `M17_typography_pill_gated_on_in_session` | `typography.rs` | Gated `fontSize` `pill_field` on `!in_session` (`var_bind::numeric`) | **KILLED** | `inspector_var_binds_typography_bind_picker_and_style_detach_undo` |
| 18 | `M18_weight_dropdown_omits_apply_variable` | `typography.rs` | `let can_bind_style = false;` in `weight_dropdown` | **KILLED** | `inspector_var_binds_typography_bind_picker_and_style_detach_undo` |

### 5. Visual Verification of [`r2-typography.png`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/r2-typography.png)

Generated headlessly from `Sim::shapes` + `designos-render` (`2554×1188` composite of 4 panels) and inspected with `read_file`:

1. **Panel 1A & Panel 1B (Typography section at rest — `Semi Bold` and `Extra Bold`):**
   - The 88 px weight dropdown (`wt_r`) has no bind icon and no `25 px` reserve, displaying `Semi Bold` (Panel 1A) and `Extra Bold` (Panel 1B) in full without ellipsis or truncation.
   - `bind:fontFamily`, `bind:fontSize`, `bind:lineHeight`, `bind:letterSpacing`, and `bind:fills/0` are visible at rest in `theme::ICON_QUATERNARY` and vertically centred on their 32 px rows (`top + 4.0`, `cy == field.cy`).
2. **Panel 2 (Open weight dropdown menu):**
   - Clicking the weight dropdown opens the popup menu showing weights `Thin`, `Extra Light`, `Light`, `Regular`, `Medium`, `Semi Bold`, `Bold`, `Extra Bold`, `Black`, followed by a horizontal separator and `Apply variable…` at the bottom.
3. **Panel 3 (Bound weight pill):**
   - After selecting `Apply variable…` and picking `Type/Style` (`fontStyle`), the 88 px weight slot renders the bound variable pill (`Type/Style` with detach `×`), while `fontSize` alongside it keeps its numeric field + bind icon.
4. **Panel 4 (Active text-editing session `in_session = true`):**
   - While editing text on canvas, bound typography fields (`bound:fontStyle` = `Type/Style`, `bound:fontSize` = `Number/Step`, `bound:fills/0` = `Color/Brand`) still render their bound variable pills so bound values cannot be overwritten mid-session, whereas unbound fields (`Inter`, `Auto`, `0px`) render at full width with their bind icons hidden.

---

## Round 1 Summary (Updated for MINOR-9)

1. **R1 — Always-Visible Variable Bind Icon When Matching Variables Exist (`#876`):**
   - Added `has_matching_var`, `value_rect`, and `style_value_rect` in [`crates/designos-app/src/panels/var_bind.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/var_bind.rs) so unbound fields reserve `theme::TARGET_MIN + theme::BORDER_WIDTH` (`25.0 px`) on the right when a matching local or enabled subscribed library variable exists, and keep full width otherwise.
   - Updated `draw_hover_button` so `hover_button` and `style_hover_button` render the `24×24` (`theme::TARGET_MIN`) hexagonal bind icon flush with `rect.right()` at rest in `theme::ICON_QUATERNARY` with no background fill, and highlight with `theme::BG_TERTIARY` + `theme::ICON` on hover or when the picker is open. Kept `picker_stop_hover_button` hover-or-open-only inside the color picker stop list.
2. **R2 — Complete `#876` Coverage on Auto-Layout & Per-Side Stroke Weight Fields:**
   - Wrapped `al-counter-gap` in [`crates/designos-app/src/panels/properties.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/properties.rs) (`wrap_lines_ui`) in `var_bind::numeric(app, ui, pane, id, r, &["counterAxisSpacing"], ...)`.
   - Wrapped individual per-side stroke weight fields in [`crates/designos-app/src/panels/stroke.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/stroke.rs) in `var_bind::numeric` with `&["strokeTopWeight"]`, `&["strokeRightWeight"]`, `&["strokeBottomWeight"]`, and `&["strokeLeftWeight"]`.
3. **R3 — Typography Variable Bind Entry Points (`#133` Inspector Half):**
   - Wired all 8 typography properties in [`crates/designos-app/src/panels/typography.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/typography.rs):
     - `fontFamily` (`VariableKind::String`, `var_bind::pill_field` + `var_bind::value_rect` + `var_bind::hover_button`)
     - `fontStyle` (`VariableKind::String` via `"Apply variable…"` in the weight dropdown menu per `MAJOR-3`, with `var_bind::pill_field` rendering and `×` detach support for both `fontStyle` and legacy `fontWeight` `VariableKind::Number` bindings)
     - `fontSize` (`VariableKind::Number`, `var_bind::pill_field` + `var_bind::value_rect` + `var_bind::hover_button`)
     - `lineHeight` (`VariableKind::Number`, `var_bind::pill_field` + `var_bind::value_rect` + `var_bind::hover_button`)
     - `letterSpacing` (`VariableKind::Number`, `var_bind::pill_field` + `var_bind::value_rect` + `var_bind::hover_button`)
     - `fills/0` (`VariableKind::Color` on the text color row, narrowing `text-hex` via `var_bind::value_rect`)
     - `paragraphIndent` and `paragraphSpacing` (`VariableKind::Number`, `var_bind::numeric_in_session` inside `details_popover`)
4. **R4 — Scope Filtering & Subscribed Library Variables:**
   - Verified `scope_allows` mapping across all typography, stroke, layout, corner, opacity, fill, and effect keys, including subscribed library variables.
5. **R5 — Linked Text Style Detach on Typography Variable Bind:**
   - When `is_text_style_key(&key0)` (`fontFamily`, `fontStyle`, `fontWeight`, `fontSize`, `lineHeight`, `letterSpacing`, `paragraphSpacing`, `paragraphIndent`) is bound on a node with a linked `StyleKind::Text` ref, `n.style_refs.remove(StyleKind::Text.ref_key())` runs inside the same `Tx` (`"Bind Variable"` / `"Bind Library Variable"`), so one Undo restores both the text style ref and the previous binding state.
6. **R6 — Font Loading on `fontFamily` Variable Bind:**
   - Wrapped `PickResult::Picked` in `textsession::requesting_new_faces` and called `textsession::load_for_with(&app.doc, &app.fonts, new_faces)` so newly bound font families trigger font loading immediately.
7. **Round 1 Workspace Lib Test Count Correction (`MINOR-9`):**
   - In [`08-cargo-test-lib.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/08-cargo-test-lib.log), the 12 workspace `--lib` test binaries passed `13 + 706 + 1 + 148 + 143 + 30 + 4 + 37 + 269 + 56 + 1 + 8 = 1,416` tests (`0 failed`), and in Round 2 ([`r2-green.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/r2-green.log)) passed `13 + 706 + 1 + 153 + 143 + 30 + 4 + 37 + 322 + 56 + 1 + 8 = 1,474` tests (`0 failed`).

## Files Modified

- [`crates/designos-app/src/panels/var_bind.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/var_bind.rs)
- [`crates/designos-app/src/panels/typography.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/typography.rs)
- [`crates/designos-app/src/panels/stroke.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/stroke.rs)
- [`crates/designos-app/src/panels/properties.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/properties.rs)
- [`crates/designos-app/src/panels/effects.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/effects.rs)
- [`crates/designos-app/src/panels/style_edit.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/style_edit.rs)
- [`crates/designos-app/src/layout_snapshot.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/layout_snapshot.rs)
- [`crates/designos-app/src/shell/tests.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/shell/tests.rs)
- [`crates/designos-app/src/shell/tests/inspector_var_binds.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/shell/tests/inspector_var_binds.rs)
- [`crates/designos-app/tests/layout-baseline/allow.json`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/tests/layout-baseline/allow.json) & `crates/designos-app/tests/layout-baseline/*.layout.json`
- [`plans/reports/agy-876-133-inspector-binds.md`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds.md) & `plans/reports/agy-876-133-inspector-binds-evidence/*`
