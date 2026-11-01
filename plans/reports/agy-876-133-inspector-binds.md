# Report: Always-Visible Variable Bind Icon & Typography Bind Entry Points (`#876`, `#133`)

- **Branch:** `jangtrinh/agy-876-133-inspector-binds`
- **Base:** `origin/main` (`de6b81ff` in Round 3; `64f2dd07` in Round 2; `0c894123` in Round 1)
- **Issues:** Closes `#876` · Part of `#133` (inspector half)

## Not Verified / Known Limitations

1. **Interactive macOS Window Server Hover / Cursor Verification:** All layout, hit-testing, colour, and proof-sheet checks (`r2-typography.png`) were rendered headlessly via `Sim::shapes` and `designos-render` software rasterization; live OS window-server cursor changes on hover were not manually exercised in a GUI window session.
2. **Style-Token Undo Coverage (`#1448`):** Binding a typography variable on a node with a linked `StyleKind::Text` detaches the text style inside the same `Tx` (`"Bind Variable"` / `"Bind Library Variable"`), so a single Undo restores both the style ref and the previous binding state (`#1448` is out of scope for this PR).
3. **Legacy `fontWeight` Variable Picker Creation:** Legacy `fontWeight` (`VariableKind::Number`) bindings on text nodes render as bound pills in the weight slot and can be detached via the pill `×` button, while new bindings from the weight dropdown's `"Apply variable…"` item bind `fontStyle` (`VariableKind::String`).
4. **Pre-existing Clippy Lints Outside Scope:** On `origin/main` (`de6b81ff`), `crates/designos-app/src/agent_permissions.rs` (lines 1176 and 1194) has 2 pre-existing `clippy::needless_return` errors under `-D warnings` (`crates/designos-app/src/panels/components.rs` has 0 warnings). `agent_permissions.rs` is outside this PR's allowed file scope; all files touched by this PR pass `cargo clippy` with zero warnings.

## Re-Anchor Drift

- [`crates/designos-app/src/panels/var_bind.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/var_bind.rs): `625` lines on `origin/main` (`a5c7b385` and `ca2f621b`) → `774` lines after Round 1 (`0c894123` base) → `769` lines after merging `64f2dd07` → `779` lines in Round 2 (`7a3cfae0`) → `768` lines in Round 3.
  - Replaced Round 2's 13-slot `AvailabilityTable` (which walked all local and library variables 13 times per pass via `SCOPE_SLOTS.iter().enumerate()` and merged `FRAME_FILL` / `SHAPE_FILL` / `TEXT_FILL` into one coarse `"fills/0"` slot) with a single-walk `ScopeCensus` (`unscoped: [bool; 4]`, `scoped: [u32; 4]` over the 24 Figma scope names in `SCOPE_NAMES`) built once per egui pass (`pass_nr`, `local_vars_len`, `enabled_len`, `lib_stamp`) and evaluated via `scope_allows(prop, scope, target_kind)`.
  - Changed `node_target_kind` and `style_target_kind` to return `&'static NodeKind` (`text_target_kind()`, `&NodeKind::Section`, `&NodeKind::Rectangle`) so zero `NodeKind` clones or heap allocations occur per field check.
  - Changed `draw_hover_button` signature to accept `record_name: impl FnOnce() -> String` and call `widgets::record_with` so zero `format!` allocations occur when layout snapshot recording is off.
  - Centred the `24×24` (`theme::TARGET_MIN`) bind icon vertically on `rect.center().y` (`Rect::from_center_size(Pos2::new(rect.right() - theme::TARGET_MIN / 2.0, rect.center().y), Vec2::splat(theme::TARGET_MIN))`).
  - Delegated non-text-style keys (`!is_text_style_key(&key0)`) in `bind_picker` to `app.bind_variable` / `app.bind_library_variable` and removed the redundant `if key0 == "fontFamily"` font-loading block (`textsession::requesting_new_faces` already loads newly requested faces).
- [`crates/designos-app/src/panels/typography.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/typography.rs): `1,010` lines in Round 2 and Round 3.
  - During an active text editing session (`in_session`), bound fields (`fontFamily`, `fontStyle`/`fontWeight`, `fontSize`, `lineHeight`, `letterSpacing`, `fills/0`, `paragraphIndent`, `paragraphSpacing`) still render their bound `var_bind::pill_field` so bound properties remain locked; only unbound fields suppress `value_rect` narrowing and `hover_button` during `in_session`.
  - Removed the `25 px` bind icon reserve and `hover_button` from the 88 px weight dropdown (`wt_r`) so weight names (`Semi Bold`, `Extra Bold`, etc.) display in full; restored `.checked(w.0 == cur)` on weight menu items (`WEIGHTS.iter().map(|w| MenuItem::new(w.0).checked(w.0 == cur))`); added `"Apply variable…"` after a separator at the bottom of the weight dropdown menu via `dropdown_items` when `!in_session` and a matching `fontStyle` (`VariableKind::String`) variable exists.
- `value_rect` / `style_value_rect` call sites across `crates/designos-app/src/panels/`: `13` call sites (`2` in `var_bind.rs`, `3` in `style_edit.rs`, `3` in `typography.rs`, `2` in `effects.rs`, `2` in `properties.rs`, plus `2` fn definitions in `var_bind.rs` = `15` occurrences).
- `hover_button` / `style_hover_button` call sites across `crates/designos-app/src/panels/`: `12` call sites (`4` in `var_bind.rs`, `3` in `style_edit.rs`, `2` in `effects.rs`, `3` in `typography.rs`, `1` in `properties.rs`, plus `2` fn definitions in `var_bind.rs` = `14` occurrences).
- Layout baselines & allow-list: `49` `.layout.json` files under `crates/designos-app/tests/layout-baseline/`, [`allow.json`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/tests/layout-baseline/allow.json) (deleted 21 `micro-*` entries and 2 zero-drift `canvas` `h_base: 24` entries), and [`crates/designos-app/src/layout_snapshot.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/layout_snapshot.rs) (`2,680` lines; `check_stale_allow_entries_for_run` checks the union of specs in a run using `main`'s rule that an entry is stale only when it matches 0 widgets, and reports `name_prefix`, `h_base`, and `reason`).

## Assumptions

1. `egui::Context::cumulative_pass_nr()` advances once per egui layout pass, and `(pass_nr, local_vars_len, enabled_len, lib_stamp)` in `ScopeCensus` uniquely identifies the variable scope census within a pass, including same-pass local variable additions (`local_vars_len`) and library metadata refreshes (`lib_stamp`).
2. Subscribed library variable metadata only changes when `lib_meta_stamp(app)` changes (`u64::MAX` when no libraries are enabled), making `Arc<[(VariableKind, Vec<String>)]>` safe to reuse across passes without re-cloning.
3. On the 88 px weight dropdown (`wt_r`), `"Apply variable…"` is appended after a separator via `dropdown_items` only when `!in_session` and `var_bind::has_matching_var(app, ui.ctx(), "fontStyle", VariableKind::String, &NodeKind::Text(t.clone()))` is true, opening `var_bind::open_picker(app, sel, &["fontStyle"], VariableKind::String, wt_r)`.

## Skills Loaded

- `es:astra-work`
- `es:fable-thinking`
- `es:rust`
- `es:rust-gpu-canvas`
- `es:dev`
- `es:feature-dev`

---

## Round 3

### What Could Not Be Verified

- **Interactive OS Window-Server Cursor Hover:** All Round 3 acceptance checks, RED-on-`7a3cfae0` / GREEN-on-HEAD tests, 33/33 mutant kills, opt-level 2 kit 697-variable frame-time and allocation benchmarks against `origin/main`, and workspace gates were verified directly by command execution below. Only live OS window-server cursor shape changes remain headless-only.

### 1. Controller Rulings & Verification Matrix

| Ruling | Fix Summary | File : Line | Test Name(s) | Status |
|---|---|---|---|---|
| **1 — MAJOR-B & MINOR-A** | Replaced `AvailabilityTable` (which walked all variables 13 times per pass and collapsed `FRAME_FILL` / `SHAPE_FILL` / `TEXT_FILL` into one `"fills/0"` slot) with `ScopeCensus` (`unscoped: [bool; 4]`, `scoped: [u32; 4]` over the 24 `SCOPE_NAMES`). `pass_scope_census` visits each local and enabled library variable once per pass (`census_add`, recording `(pass_nr, visits)` under `Id::new("var-bind-pass-visits")` in `#[cfg(test)]`), and `has_matching_var` checks `scope_allows(prop, scope, target_kind)` over the active scope bitmask (`0` mismatches across 4 kinds × 24 scopes × 7 target `NodeKind`s × 38 keys). | [`var_bind.rs:146-248`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/var_bind.rs#L146-L248) | `inspector_var_binds_scope_census_visits_once_per_pass_and_refreshes_on_local_or_library_update`, `inspector_var_binds_scope_oracle_and_per_target_fill_scopes` | **PASS** |
| **2 — MINOR-C** | Restored `.checked(w.0 == cur)` on the weight dropdown menu items (`WEIGHTS.iter().map(|w| MenuItem::new(w.0).checked(w.0 == cur)).collect()`) so the current weight displays a checkmark as on `origin/main`. | [`typography.rs:223`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/typography.rs#L223) | `inspector_var_binds_weight_menu_checks_current_weight_and_guards_apply_variable` | **PASS** |
| **3 — MINOR-D** | Added tests covering `"Apply variable…"` suppression during an active text edit session (`N05`) and when no `fontStyle`-compatible String variable exists (`N06`), selecting `Black` (`menu-row-0-8` → weight `900`, `menu-check-0-8` checked, no bind picker opened, `N07`), same-pass local variable addition (`N01`), library v1 → v2 update adding a Number variable (`N03`, `N04`), and per-target fill scopes (`FRAME_FILL`, `SHAPE_FILL`, `TEXT_FILL`, `ALL_FILLS` on Frame, Rectangle, and Text, `N13`). | [`inspector_var_binds.rs:917-1240`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/shell/tests/inspector_var_binds.rs#L917-L1240) | `inspector_var_binds_scope_census_visits_once_per_pass_and_refreshes_on_local_or_library_update`, `inspector_var_binds_scope_oracle_and_per_target_fill_scopes`, `inspector_var_binds_weight_menu_checks_current_weight_and_guards_apply_variable` | **PASS** |
| **4 — MINOR-6 / Q1 & LOW-G** | Kept the 23 deleted `allow.json` entries deleted; reverted the stricter drift-bound stale rule in `layout_snapshot::compare` so an `allow.json` entry is stale only when it matches 0 widgets (matching `origin/main`); enriched the stale error message in `check_stale_allow_entries_for_run` with `name_prefix={:?}, h_base={:?}, reason='{}'`. | [`layout_snapshot.rs:766-795`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/layout_snapshot.rs#L766-L795), [`layout_snapshot.rs:820-835`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/layout_snapshot.rs#L820-L835), [`layout_snapshot.rs:1636-1688`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/layout_snapshot.rs#L1636-L1688) | `layout_snapshot::tests::stale_check_counts_covering_entries_and_runs_over_union_of_run` | **PASS** |
| **5 — MINOR-F & MINOR-E** | Removed all review codes, issue numbers, and mutant IDs from comments, test names, and assertion messages across all touched files; rewrote inaccurate Round 1 / Round 2 items in this report from the actual code and logs. | [`var_bind.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/var_bind.rs), [`typography.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/typography.rs), [`layout_snapshot.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/layout_snapshot.rs), [`inspector_var_binds.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/shell/tests/inspector_var_binds.rs), [`agy-876-133-inspector-binds.md`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds.md) | Verified by `git diff origin/main -- crates/designos-app/src/` grep (`0` matches) | **PASS** |

### 2. Acceptance Criterion 1: RED on `7a3cfae0`, GREEN on HEAD, and 33/33 Mutants Killed

#### 2.1 RED on `7a3cfae0` ([`r3-red-on-7a3cfae0.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/r3-red-on-7a3cfae0.log))

Command:

```sh
git checkout 7a3cfae0 -- crates/designos-app/src/panels/var_bind.rs crates/designos-app/src/panels/typography.rs crates/designos-app/src/layout_snapshot.rs
cargo test -p designos-app --bin designos --locked -- \
  inspector_var_binds_scope_census_visits_once_per_pass_and_refreshes_on_local_or_library_update \
  inspector_var_binds_scope_oracle_and_per_target_fill_scopes \
  inspector_var_binds_weight_menu_checks_current_weight_and_guards_apply_variable \
  stale_check_counts_covering_entries_and_runs_over_union_of_run
```

Output:

```text
running 4 tests

thread 'layout_snapshot::tests::stale_check_counts_covering_entries_and_runs_over_union_of_run' (59896963) panicked at crates/designos-app/src/layout_snapshot.rs:1662:14:
covering entry for existing widgets with 0 drift must not be flagged as stale: "stale allow.json[0] entry (case='d-editor-default', region='right_panel', spec='F'): matched 0 widgets"
test layout_snapshot::tests::stale_check_counts_covering_entries_and_runs_over_union_of_run ... FAILED

thread 'shell::tests::inspector_var_binds::inspector_var_binds_weight_menu_checks_current_weight_and_guards_apply_variable' (59896966) panicked at crates/designos-app/src/shell/tests/inspector_var_binds.rs:1201:5:
weight menu must check the current weight Semi Bold at menu-check-0-5
test shell::tests::inspector_var_binds::inspector_var_binds_weight_menu_checks_current_weight_and_guards_apply_variable ... FAILED

thread 'shell::tests::inspector_var_binds::inspector_var_binds_scope_oracle_and_per_target_fill_scopes' (59896965) panicked at crates/designos-app/src/shell/tests/inspector_var_binds.rs:1030:13:
assertion `left == right` failed: scope FRAME_FILL on Frame(Welcome): expected bind:fills/0 visible=true
  left: false
 right: true
test shell::tests::inspector_var_binds::inspector_var_binds_scope_oracle_and_per_target_fill_scopes ... FAILED

thread 'shell::tests::inspector_var_binds::inspector_var_binds_scope_census_visits_once_per_pass_and_refreshes_on_local_or_library_update' (59896964) panicked at crates/designos-app/src/shell/tests/inspector_var_binds.rs:945:5:
assertion `left == right` failed: scope census must visit each local (4) and enabled library (2) variable exactly once per pass
  left: None
 right: Some(6)
test shell::tests::inspector_var_binds::inspector_var_binds_scope_census_visits_once_per_pass_and_refreshes_on_local_or_library_update ... FAILED

test result: FAILED. 0 passed; 4 failed; 0 ignored; 0 measured; 4045 filtered out; finished in 1.02s
```

#### 2.2 GREEN on HEAD ([`r3-green.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/r3-green.log))

Command:

```sh
cargo test -p designos-app --bin designos --locked -- \
  inspector_var_binds_scope_census_visits_once_per_pass_and_refreshes_on_local_or_library_update \
  inspector_var_binds_scope_oracle_and_per_target_fill_scopes \
  inspector_var_binds_weight_menu_checks_current_weight_and_guards_apply_variable \
  stale_check_counts_covering_entries_and_runs_over_union_of_run
```

Output:

```text
running 4 tests
test layout_snapshot::tests::stale_check_counts_covering_entries_and_runs_over_union_of_run ... ok
test shell::tests::inspector_var_binds::inspector_var_binds_scope_census_visits_once_per_pass_and_refreshes_on_local_or_library_update ... ok
test shell::tests::inspector_var_binds::inspector_var_binds_weight_menu_checks_current_weight_and_guards_apply_variable ... ok
test shell::tests::inspector_var_binds::inspector_var_binds_scope_oracle_and_per_target_fill_scopes ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 4045 filtered out; finished in 3.26s
```

#### 2.3 All 33 Reviewer Mutants (`M01..M18` + `N01..N15`, [`r3-mutants.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/r3-mutants.log))

Every mutant from `rev-r1-report.md` (`M01`–`M18`) and `rev-r2-report.md` (`N01`–`N15`) was applied one at a time, tested with `cargo test -p designos-app --bin designos --locked -- inspector_var_binds effect_stop_binds stale_check_counts_covering_entries_and_runs_over_union_of_run`, and reverted immediately:

| # | Mutant ID | File | One-Line Diff | Status | Killing Test(s) |
|---|---|---|---|---|---|
| 1 | `M01_predicate_local_drop_kind` | `var_bind.rs` | Drop `var.kind == kind` check on local variables in `has_matching_var` / `bind_picker` | **KILLED** | `inspector_var_binds_hidden_when_no_matching_local_or_library_variable_exists`, `inspector_var_binds_scope_oracle_and_per_target_fill_scopes`, `effect_stop_binds::node_effect_rows_bind_filter_scopes_detach_and_undo` |
| 2 | `M02_predicate_local_drop_scope` | `var_bind.rs` | Replace `scope_allows(prop, s, target_kind)` with `true` in `has_matching_var` | **KILLED** | `inspector_var_binds_scope_oracle_and_per_target_fill_scopes`, `inspector_var_binds_hidden_when_no_matching_local_or_library_variable_exists`, `inspector_var_binds_weight_menu_checks_current_weight_and_guards_apply_variable` |
| 3 | `M03_predicate_library_drop_scope` | `var_bind.rs` | Ignore library variable scopes (`&[][..]` in `census_add` for library vars) | **KILLED** | `inspector_var_binds_hidden_when_no_matching_local_or_library_variable_exists` |
| 4 | `M04_narrow_zero_width` | `var_bind.rs` | `let reserve = 0.0;` in `narrow_for_bind` | **KILLED** | `inspector_var_binds_no_overlap_between_value_field_and_bind_hitbox`, `inspector_var_binds_scope_census_visits_once_per_pass_and_refreshes_on_local_or_library_update`, `inspector_var_binds_hidden_when_no_matching_local_or_library_variable_exists`, `inspector_var_binds_scope_oracle_and_per_target_fill_scopes` |
| 5 | `M05_narrow_always_reserve` | `var_bind.rs` | `if true && rect.width() > reserve + 24.0` in `narrow_for_bind` | **KILLED** | `inspector_var_binds_hidden_when_no_matching_local_or_library_variable_exists`, `inspector_var_binds_scope_census_visits_once_per_pass_and_refreshes_on_local_or_library_update`, `inspector_var_binds_scope_oracle_and_per_target_fill_scopes` |
| 6 | `M06_hover_btn_20px_hitbox` | `var_bind.rs` | Shrink `draw_hover_button` hitbox from `theme::TARGET_MIN` (`24.0`) to `20.0` | **KILLED** | `inspector_var_binds_no_overlap_between_value_field_and_bind_hitbox`, `inspector_var_binds_always_visible_at_rest_and_opens_picker_on_single_click` |
| 7 | `M07_local_detach_separate_tx` | `var_bind.rs` | Skip `style_refs.remove(StyleKind::Text.ref_key())` inside local `Tx` | **KILLED** | `inspector_var_binds_style_linked_text_detaches_in_same_undo_step_local_and_library` |
| 8 | `M08_lib_detach_separate_tx` | `var_bind.rs` | Skip `style_refs.remove(StyleKind::Text.ref_key())` inside library `Tx` | **KILLED** | `inspector_var_binds_style_linked_text_detaches_in_same_undo_step_local_and_library` |
| 9 | `M09_weight_menu_binds_number` | `typography.rs` | Open weight bind picker with `VariableKind::Number` instead of `VariableKind::String` | **KILLED** | `inspector_var_binds_style_linked_text_detaches_in_same_undo_step_local_and_library`, `inspector_var_binds_typography_fields_bind_detach_legacy_weight_and_font_load` |
| 10 | `M10_stroke_side_order_R_L_swapped` | `stroke.rs` | Swap `"strokeRightWeight"` and `"strokeLeftWeight"` in per-side stroke grid | **KILLED** | `inspector_var_binds_per_side_stroke_weights_bind_and_detach` |
| 11 | `M11_stop_hover_always_visible` | `var_bind.rs` | Pass `always_visible = true` in `picker_stop_hover_button` | **KILLED** | `inspector_var_binds_always_visible_at_rest_and_opens_picker_on_single_click` |
| 12 | `M12_always_vis_icon_secondary` | `var_bind.rs` | Use `theme::ICON_SECONDARY` instead of `theme::ICON_QUATERNARY` at rest | **KILLED** | `inspector_var_binds_always_visible_at_rest_and_opens_picker_on_single_click` |
| 13 | `M13_paragraph_rows_ignore_session` | `typography.rs` | Pass `false` instead of `in_session` to `numeric_in_session` in `details_popover` | **KILLED** | `inspector_var_binds_suppressed_during_active_text_edit_session` |
| 14 | `M14_effects_num_full_rc` | `effects.rs` | Pass un-narrowed `rc` instead of `val_r` to `field::num_field` in `effect_num_row` | **KILLED** | `inspector_var_binds_no_overlap_between_value_field_and_bind_hitbox` |
| 15 | `M15_style_editor_ignores_reserved_rect` | `style_edit.rs` | Pass un-narrowed `rc` instead of `val_r` to `field_box` on `fontFamily` | **KILLED** | `inspector_var_binds_no_overlap_between_value_field_and_bind_hitbox` |
| 16 | `M16_skip_requesting_new_faces` | `var_bind.rs` | Skip `textsession::load_for_with(&app.doc, &app.fonts, new_faces)` on `PickResult::Picked` | **KILLED** | `inspector_var_binds_typography_fields_bind_detach_legacy_weight_and_font_load` |
| 17 | `M17_hover_btn_not_always_visible` | `var_bind.rs` | `let show = hovered \|\| is_open;` in `draw_hover_button` | **KILLED** | `inspector_var_binds_always_visible_at_rest_and_opens_picker_on_single_click` (+ 8 other `inspector_var_binds` tests) |
| 18 | `M18_is_text_style_key_drop_fontStyle` | `var_bind.rs` | Remove `"fontStyle"` from `is_text_style_key` | **KILLED** | `inspector_var_binds_style_linked_text_detaches_in_same_undo_step_local_and_library` |
| 19 | `N01_avail_key_drop_local_len` | `var_bind.rs` | Ignore `t.local_vars_len == local_vars_len` in `pass_scope_census` cache hit check | **KILLED** | `inspector_var_binds_scope_census_visits_once_per_pass_and_refreshes_on_local_or_library_update` |
| 20 | `N02_avail_key_drop_pass_nr` | `var_bind.rs` | Ignore `t.pass_nr == pass_nr` in `pass_scope_census` cache hit check | **KILLED** | `inspector_var_binds_hidden_when_no_matching_local_or_library_variable_exists`, `inspector_var_binds_scope_census_visits_once_per_pass_and_refreshes_on_local_or_library_update`, `inspector_var_binds_scope_oracle_and_per_target_fill_scopes` |
| 21 | `N03_lib_cache_ignore_stamp` | `var_bind.rs` | Replace `c.stamp == stamp` with `!c.vars.is_empty()` in `cached_library_var_metas` | **KILLED** | `inspector_var_binds_scope_census_visits_once_per_pass_and_refreshes_on_local_or_library_update` |
| 22 | `N04_lib_stamp_drop_pending` | `var_bind.rs` | Omit `app.libs.pending.len()` from `lib_meta_stamp` | **KILLED** | `inspector_var_binds_scope_census_visits_once_per_pass_and_refreshes_on_local_or_library_update` |
| 23 | `N05_weight_menu_allow_in_session` | `typography.rs` | Drop `!in_session` guard on `"Apply variable…"` in weight dropdown | **KILLED** | `inspector_var_binds_weight_menu_checks_current_weight_and_guards_apply_variable` |
| 24 | `N06_weight_menu_always_offer_apply` | `typography.rs` | Drop `var_bind::has_matching_var` guard on `"Apply variable…"` in weight dropdown | **KILLED** | `inspector_var_binds_weight_menu_checks_current_weight_and_guards_apply_variable` |
| 25 | `N07_weight_menu_off_by_one` | `typography.rs` | Change `if i < WEIGHTS.len()` to `if i + 1 < WEIGHTS.len()` in weight dropdown | **KILLED** | `inspector_var_binds_weight_menu_checks_current_weight_and_guards_apply_variable` |
| 26 | `N08_fill_pill_hidden_in_session` | `typography.rs` | Gate `fills/0` `var_bind::pill_field` on `!in_session` | **KILLED** | `inspector_var_binds_suppressed_during_active_text_edit_session` |
| 27 | `N09_line_height_pill_hidden_in_session` | `typography.rs` | Gate `lineHeight` `var_bind::pill_field` on `!in_session` | **KILLED** | `inspector_var_binds_suppressed_during_active_text_edit_session` |
| 28 | `N10_weight_pill_hidden_in_session` | `typography.rs` | Gate `fontStyle` `var_bind::pill_field` on `!in_session` | **KILLED** | `inspector_var_binds_suppressed_during_active_text_edit_session` |
| 29 | `N11_family_pill_hidden_in_session` | `typography.rs` | Gate `fontFamily` `var_bind::pill_field` on `!in_session` | **KILLED** | `inspector_var_binds_suppressed_during_active_text_edit_session` |
| 30 | `N12_paragraph_indent_pill_hidden_in_session` | `var_bind.rs` | Gate `pill_field` inside `numeric_in_session` on `!in_session` | **KILLED** | `inspector_var_binds_suppressed_during_active_text_edit_session` |
| 31 | `N13_avail_merge_text_fill_slot` | `var_bind.rs` | Map `"TEXT_FILL"` to `"FRAME_FILL"` in `SCOPE_NAMES` | **KILLED** | `inspector_var_binds_scope_oracle_and_per_target_fill_scopes` |
| 32 | `N14_stale_check_omit_rule_details` (plus `N14_stale_check_old_semantics` note) | `layout_snapshot.rs` | Omit `name_prefix={:?}, h_base={:?}, reason='{}'` from `check_stale_allow_entries_for_run` (note: `rev-r2-report.md`'s original `N14_stale_check_old_semantics` mutated `covered_with_drift == 0` → `covered == 0`, which Ruling 4 / `MINOR-6 / Q1` adopted as canonical production behavior; `stale_check_counts_covering_entries_and_runs_over_union_of_run` kills both the `7a3cfae0` `covered_with_drift == 0` rule and omitting `LOW-G` rule details) | **KILLED** | `layout_snapshot::tests::stale_check_counts_covering_entries_and_runs_over_union_of_run` |
| 33 | `N15_hover_btn_top_aligned` | `var_bind.rs` | Top-align bind icon (`Pos2::new(rect.right() - theme::TARGET_MIN, rect.top())`) instead of vertical centring | **KILLED** | `inspector_var_binds_no_overlap_between_value_field_and_bind_hitbox`, `inspector_var_binds_always_visible_at_rest_and_opens_picker_on_single_click` |

### 3. Acceptance Criterion 2: Frame Cost with the Kit's 697 Variables (Text Selected, `opt-level=2`) & Allocation Baseline vs `origin/main`

Measured with `RUSTFLAGS="-C opt-level=2"` on both `origin/main` (`de6b81ff`) and Round 3 HEAD using the Untitled UI PRO VARIABLES v8.0 kit's `697` variables (`3` collections: `_Primitives`, `1. Color modes`, `2. Containers, spacing`) extracted to `$S/tmp/kit-vars.json` (outside the repository), across 20 steady-state frames after 3 warm-up frames:

- **Text selected (`"Welcome back"`, `697` kit variables, `opt-level=2`):**
  - `origin/main` (`de6b81ff`): `min = 227 µs` (`0.227 ms`), `med = 292 µs` (`0.292 ms`), `allocs = 1,958 min / 1,959 med`
  - Round 3 HEAD: `min = 245 µs` (`0.245 ms`, **`+7.9%` vs `main`**, within `<= +15%`), `med = 273 µs` (`0.273 ms`, **`-6.5%` vs `main`**), `allocs = 1,973 min / 1,974 med` (`+0.77%` vs `main`)
  - Round 2 (`7a3cfae0`, before `ScopeCensus` + `&'static NodeKind`): `min = 500 µs` (`+120%` vs `main`), `allocs = 5,113` (`+161%` vs `main`)
- **Frame selected (`"Welcome"`, `697` kit variables, `opt-level=2`):**
  - `origin/main` (`de6b81ff`): `min = 285 µs`, `med = 310 µs`, `allocs = 3,163`
  - Round 3 HEAD: `min = 283 µs` (`-0.7%` vs `main`), `med = 301 µs` (`-2.9%` vs `main`), `allocs = 3,151` (`-0.38%` vs `main`)
- **Rectangle selected (`"Hero"`, `697` kit variables, `opt-level=2`):**
  - `origin/main` (`de6b81ff`): `min = 225 µs`, `med = 235 µs`, `allocs = 1,649`
  - Round 3 HEAD: `min = 255 µs` (`+13.3%` vs `main`), `med = 271 µs`, `allocs = 1,657` (`+0.49%` vs `main`)
- **700-Variable Subscribed Library Allocation Comparison (`origin/main` vs Round 3 HEAD):**
  - `"Welcome back"` (Text): `origin/main` `1,958` (0 vars) / `2,100` (700 lib vars) vs Round 3 HEAD `1,943` (0 vars) / `2,090` (700 lib vars; **`-0.48%` vs `main`**, `+7.57%` vs 0-var HEAD)
  - `"Hero"` (Rectangle): `origin/main` `1,649` (0 vars) / `1,748` (700 lib vars) vs Round 3 HEAD `1,642` (0 vars) / `1,749` (700 lib vars; **`+0.06%` vs `main`**, `+6.52%` vs 0-var HEAD)

#### Pasted Output — `origin/main` (`de6b81ff`, [`r3-kitcost-main.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/r3-kitcost-main.log))

```text
running 1 test
KIT_COST target=Welcome back vars=kit697 allocs min=1958 med=1959 time_us min=227 med=292
KIT_COST target=Welcome vars=kit697 allocs min=3163 med=3163 time_us min=285 med=310
designos: --select "Hero" matched 2 layers; selected id 5 (first in page order; use id:<n> for another)
KIT_COST target=Hero vars=kit697 allocs min=1649 med=1649 time_us min=225 med=235
BASE_COST target=Welcome back vars=none allocs min=1958 med=1958 time_us min=244 med=250
designos: --select "Hero" matched 2 layers; selected id 5 (first in page order; use id:<n> for another)
BASE_COST target=Hero vars=none allocs min=1649 med=1649 time_us min=227 med=239
LIB700_COST target=Welcome back vars=lib700 allocs min=2100 med=2100 time_us min=249 med=263
designos: --select "Hero" matched 2 layers; selected id 5 (first in page order; use id:<n> for another)
LIB700_COST target=Hero vars=lib700 allocs min=1748 med=1748 time_us min=205 med=215
test shell::tests::inspector_var_binds::kit_cost_and_alloc_probe ... ok
```

#### Pasted Output — Round 3 HEAD ([`r3-kitcost-head.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/r3-kitcost-head.log))

```text
running 1 test
KIT_COST target=Welcome back vars=kit697 allocs min=1973 med=1974 time_us min=245 med=273
KIT_COST target=Welcome vars=kit697 allocs min=3151 med=3151 time_us min=283 med=301
designos: --select "Hero" matched 2 layers; selected id 5 (first in page order; use id:<n> for another)
KIT_COST target=Hero vars=kit697 allocs min=1657 med=1657 time_us min=255 med=271
BASE_COST target=Welcome back vars=none allocs min=1943 med=1943 time_us min=213 med=226
designos: --select "Hero" matched 2 layers; selected id 5 (first in page order; use id:<n> for another)
BASE_COST target=Hero vars=none allocs min=1642 med=1642 time_us min=196 med=203
LIB700_COST target=Welcome back vars=lib700 allocs min=2090 med=2090 time_us min=225 med=243
designos: --select "Hero" matched 2 layers; selected id 5 (first in page order; use id:<n> for another)
LIB700_COST target=Hero vars=lib700 allocs min=1749 med=1749 time_us min=202 med=234
test shell::tests::inspector_var_binds::kit_cost_and_alloc_probe ... ok
```

### 4. Acceptance Criterion 3: Verification Gates on Final Commit

#### 4.1 `cargo fmt --all -- --check` ([`r3-cargo-fmt.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/r3-cargo-fmt.log))

```sh
cargo fmt --all -- --check
```

```text
cargo fmt exit: 0
```

#### 4.2 `cargo clippy --workspace --all-targets --locked --keep-going -- -D warnings` ([`r3-cargo-clippy.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/r3-cargo-clippy.log))

```sh
cargo clippy --workspace --all-targets --locked --keep-going -- -D warnings
```

```text
   Compiling designos-mcp v0.7.6 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-mcp)
    Checking designos-app v0.7.6 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app)
error: unneeded `return` statement
    --> crates/designos-app/src/agent_permissions.rs:1176:9
     |
1176 |         return std::env::var_os("HOME").filter(|p| !p.is_empty()).map(|home| PathBuf::from(home).join("Library").join("Application Support"));
     |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
     = note: `-D clippy::needless-return` implied by `-D warnings`

error: unneeded `return` statement
    --> crates/designos-app/src/agent_permissions.rs:1194:9
     |
1194 |         return std::env::var_os("HOME").filter(|p| !p.is_empty()).map(|home| PathBuf::from(home).join("Library").join("Caches"));
     |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
```

*(Only `main`'s 2 pre-existing `clippy::needless_return` lints in untouched `crates/designos-app/src/agent_permissions.rs:1176,1194`; zero lints in any touched file.)*

#### 4.3 `cargo test -p designos-app --bin designos --locked -- inspector typography var_bind layout_unchanged` ([`r3-tests-app.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/r3-tests-app.log))

```sh
cargo test -p designos-app --bin designos --locked -- inspector typography var_bind layout_unchanged
```

```text
running 95 tests
...
test shell::tests::inspector_var_binds::inspector_var_binds_always_visible_at_rest_and_opens_picker_on_single_click ... ok
test shell::tests::inspector_var_binds::inspector_var_binds_frame_allocations_with_700_library_variables_within_ten_percent ... ok
test shell::tests::inspector_var_binds::inspector_var_binds_hidden_when_no_matching_local_or_library_variable_exists ... ok
test shell::tests::inspector_var_binds::inspector_var_binds_no_overlap_between_value_field_and_bind_hitbox ... ok
test shell::tests::inspector_var_binds::inspector_var_binds_per_side_stroke_weights_bind_and_detach ... ok
test shell::tests::inspector_var_binds::inspector_var_binds_scope_census_visits_once_per_pass_and_refreshes_on_local_or_library_update ... ok
test shell::tests::inspector_var_binds::inspector_var_binds_scope_oracle_and_per_target_fill_scopes ... ok
test shell::tests::inspector_var_binds::inspector_var_binds_style_linked_text_detaches_in_same_undo_step_local_and_library ... ok
test shell::tests::inspector_var_binds::inspector_var_binds_suppressed_during_active_text_edit_session ... ok
test shell::tests::inspector_var_binds::inspector_var_binds_typography_fields_bind_detach_legacy_weight_and_font_load ... ok
test shell::tests::inspector_var_binds::inspector_var_binds_weight_menu_checks_current_weight_and_guards_apply_variable ... ok
test shell::tests::inspector_var_binds::inspector_var_binds_wrap_counter_axis_spacing_binds_and_detaches ... ok
...
test result: ok. 95 passed; 0 failed; 0 ignored; 0 measured; 3954 filtered out; finished in 24.78s
```

#### 4.4 `cargo test --workspace --locked --lib` ([`r3-tests-lib.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/r3-tests-lib.log))

```sh
cargo test --workspace --locked --lib
```

```text
designos-diff:       test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
designos-doc:        test result: ok. 708 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.66s
designos-eval:       test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
designos-export:     test result: ok. 153 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.51s
designos-fig:        test result: ok. 143 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.95s
designos-fonts:      test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
designos-gpu:        test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
designos-library:    test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.35s
designos-mcp:        test result: ok. 322 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.69s
designos-render:     test result: ok. 56 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 32.47s
designos-svg:        test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
designos-webcapture: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
```

*(Total: `1,476 passed; 0 failed; 3 ignored` across all 12 workspace library targets.)*

#### 4.5 `python3 scripts/check_chrome_colour_literals.py` ([`r3-chrome-colour-literals.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/r3-chrome-colour-literals.log))

```sh
python3 scripts/check_chrome_colour_literals.py
```

```text
Chrome colour literal gate PASSED (29 files with findings, 199/199 baseline chrome sites).
```

#### 4.6 `python3 scripts/zero_fake_ui_gate.py` ([`r3-zero-fake-ui.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/r3-zero-fake-ui.log))

```sh
python3 scripts/zero_fake_ui_gate.py
```

```text
Gate Verdict: PASS (17/17 invariants satisfied)
```

### 5. Acceptance Criterion 4: `git log --oneline origin/main..HEAD`

```text
fa72f214 fix(inspector): use single-pass scope census and restore weight menu check
31c333ba Merge remote-tracking branch 'origin/main' into jangtrinh/agy-876-133-inspector-binds
7a3cfae0 fix(app): address PR #1452 review rulings for inspector variable binds (#876, #133)
ca2f621b Merge remote-tracking branch 'origin/main' into jangtrinh/agy-876-133-inspector-binds
a5c7b385 feat(inspector): always-visible variable bind icon and typography bind entry points (#876, #133)
5192a40c test(inspector): add variable bind icon and typography bind tests (#876, #133)
```

### 6. Epistemic Ledger

- **FACT:** In Round 2 (`7a3cfae0`), `pass_availability_table` looped `for (slot, &(prop, kind, ref target_kind)) in SCOPE_SLOTS.iter().enumerate()` (`13` iterations) and called `slot_matches_any_var` on each slot, visiting all `V` variables up to 13 times per pass (`9,061` visits for the kit's `697` variables), while `node_target_kind` cloned `NodeKind::Text(TextData)` on every `value_rect` / `hover_button` call.
- **FACT:** Replacing `AvailabilityTable` with `ScopeCensus` (`pass_scope_census`) + `&'static NodeKind` in `node_target_kind` / `style_target_kind` visits each local and enabled library variable once per pass (`visits == 6` for `4` local + `2` library variables in `inspector_var_binds_scope_census_visits_once_per_pass_and_refreshes_on_local_or_library_update`), matches `scope_allows` with `0` mismatches across `4 × 24 × 7 × 38 = 25,536` `(kind, scope, target_kind, key)` combinations (`inspector_var_binds_scope_oracle_and_per_target_fill_scopes`), and brings text-selected frame cost with the kit's `697` variables at `opt-level=2` from `500 µs` (`5,113` allocs) down to `245 µs` min / `273 µs` med (`1,973` allocs) vs `origin/main`'s `227 µs` min / `292 µs` med (`1,958` allocs).
- **FACT:** Restoring `.checked(w.0 == cur)` on `typography.rs:223` sets a positive `menu-check-0-5` rect when `Semi Bold` (`600`) is current and `menu-check-0-8` when `Black` (`900`) is selected (`inspector_var_binds_weight_menu_checks_current_weight_and_guards_apply_variable`).
- **INFERENCE:** Because `ScopeCensus` records exact bitmasks over the 24 Figma scope names in `SCOPE_NAMES` and delegates the property/target check to `scope_allows`, adding new inspector bind keys cannot silently alias onto another key's scope slot.
- **ASSUMPTION:** Any future Figma scope token added to `scope_allows` will also be added to `SCOPE_NAMES` (enforced by the `SCOPE_NAMES` exhaustive oracle in `inspector_var_binds_scope_oracle_and_per_target_fill_scopes`).

---

## Round 2 (Corrected for MINOR-E)

### 1. Controller Rulings & Verification Matrix

| Ruling | Fix Summary | File : Line | Test Name(s) | Status |
|---|---|---|---|---|
| **BLOCKER-1** | Keep `var_bind::pill_field` active during `in_session` across all 8 bound typography fields (`fontFamily`, `fontStyle`/`fontWeight`, `fontSize`, `lineHeight`, `letterSpacing`, `fills/0`, `paragraphIndent`, `paragraphSpacing`) so bound values stay locked; hide only the unbound bind icon and reserve during `in_session`. | [`typography.rs:202-238`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/typography.rs#L202-L238), [`typography.rs:365-384`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/typography.rs#L365-L384), [`var_bind.rs:587-604`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/var_bind.rs#L587-L604) | `inspector_var_binds_suppressed_during_active_text_edit_session` | **PASS** |
| **MAJOR-2** | Build one per-pass scope census (`ScopeCensus` in Round 3; `AvailabilityTable` in Round 2) per egui pass (`pass_nr`) and cache library `(VariableKind, Vec<String>)` behind an `Arc` keyed by `lib_meta_stamp`; pass lazy `FnOnce() -> String` into `widgets::record_with` so zero `format!` runs when layout recording is off. | [`var_bind.rs:146-248`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/var_bind.rs#L146-L248), [`var_bind.rs:414-434`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/var_bind.rs#L414-L434) | `inspector_var_binds_frame_allocations_with_700_library_variables_within_ten_percent`, `inspector_var_binds_scope_census_visits_once_per_pass_and_refreshes_on_local_or_library_update` | **PASS** |
| **MAJOR-3** | Weight dropdown (`wt_r`, 88 px) has no bind icon and no `25 px` reserve so `Semi Bold` / `Extra Bold` show in full; binding is triggered via `"Apply variable…"` after a separator at the bottom of the weight dropdown (`dropdown_items`), gated on `!in_session` and matching `fontStyle` (`VariableKind::String`) availability. Bound `fontStyle` or legacy `fontWeight` still renders the bound pill in `wt_r`. | [`typography.rs:220-238`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/typography.rs#L220-L238) | `inspector_var_binds_typography_fields_bind_detach_legacy_weight_and_font_load`, `inspector_var_binds_weight_menu_checks_current_weight_and_guards_apply_variable` | **PASS** |
| **MINOR-4** | Vertically centre the `24×24` (`theme::TARGET_MIN`) bind icon on `rect.center().y` (`top + 4.0` on 32 px rows) via `Rect::from_center_size(Pos2::new(rect.right() - theme::TARGET_MIN / 2.0, rect.center().y), Vec2::splat(theme::TARGET_MIN))`. | [`var_bind.rs:425-428`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/var_bind.rs#L425-L428) | `inspector_var_binds_always_visible_at_rest_and_opens_picker_on_single_click`, `inspector_var_binds_no_overlap_between_value_field_and_bind_hitbox` | **PASS** |
| **MINOR-5** | Strengthened `inspector_var_binds` assertions to kill all reviewer mutants (`M01`–`M18` and `N01`–`N15`): painted `field_rect` vs icon rect checks on inspector, effect, popover, and style-editor rows; `ICON_QUATERNARY` at-rest vs `ICON` hover stroke colour check; gradient-stop hover-only check; `FONT_SIZE`-only variable excluded from `lineHeight` picker; undo after each bind; font-loading check on `fontFamily` bind; Color-only doc check on Number fields. | [`inspector_var_binds.rs:1-1240`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/shell/tests/inspector_var_binds.rs#L1-L1240) | All 12 tests in `inspector_var_binds` (`33/33` mutants killed) | **PASS** |
| **MINOR-6** | Updated layout baselines for centred `bind:*` y-coordinates and full-width `font-weight`; deleted 21 `micro-*` entries and 2 stale `canvas` `h_base: 24` entries from `allow.json`; updated `check_stale_allow_entries_for_run` to validate across the union of specs in a run and report rule details when an entry matches 0 widgets. | [`layout_snapshot.rs:766-850`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/layout_snapshot.rs#L766-L850), [`allow.json`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/tests/layout-baseline/allow.json), `tests/layout-baseline/*.layout.json` | `layout_snapshot::tests::stale_check_counts_covering_entries_and_runs_over_union_of_run` | **PASS** |
| **MINOR-7** | In `bind_picker` (`BindTarget::Node`), delegate keys where `!is_text_style_key(&key0)` directly to `app.bind_variable` / `app.bind_library_variable` instead of duplicating the inline `Tx` block. | [`var_bind.rs:722-763`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/var_bind.rs#L722-L763) | `inspector_var_binds_always_visible_at_rest_and_opens_picker_on_single_click`, `inspector_var_binds_per_side_stroke_weights_bind_and_detach`, `inspector_var_binds_wrap_counter_axis_spacing_binds_and_detaches` | **PASS** |
| **MINOR-8** | Removed the redundant `if key0 == "fontFamily" { ... }` font-loading block in `var_bind.rs`; `textsession::requesting_new_faces` already detects and loads newly bound font families. | [`var_bind.rs:713-766`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/var_bind.rs#L713-L766) | `inspector_var_binds_typography_fields_bind_detach_legacy_weight_and_font_load` (`M16_skip_requesting_new_faces`) | **PASS** |
| **MINOR-9** | Corrected Round 1 lib test total (`1,416` in R1; `1,474` in R2; `1,476` in R3), removed inaccurate `LineHeight::Px or Auto` / `LetterSpacing::Px` qualifiers and "read-only" wording for legacy `fontWeight` pills, and added `Not Verified / Known Limitations`, `Re-Anchor Drift`, `Assumptions`, `Skills Loaded`, `Round 2`, and `Round 3` sections to this report. | [`agy-876-133-inspector-binds.md`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds.md) | N/A (report accuracy) | **PASS** |

### 2. Visual Verification of [`r2-typography.png`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/r2-typography.png)

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

## Round 1 Summary

1. **Always-Visible Variable Bind Icon When Matching Variables Exist (`#876`):**
   - Added `has_matching_var`, `value_rect`, and `style_value_rect` in [`crates/designos-app/src/panels/var_bind.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/var_bind.rs) so unbound fields reserve `theme::TARGET_MIN + theme::BORDER_WIDTH` (`25.0 px`) on the right when a matching local or enabled subscribed library variable exists, and keep full width otherwise.
   - Updated `draw_hover_button` so `hover_button` and `style_hover_button` render the `24×24` (`theme::TARGET_MIN`) hexagonal bind icon flush with `rect.right()` at rest in `theme::ICON_QUATERNARY` with no background fill, and highlight with `theme::BG_TERTIARY` + `theme::ICON` on hover or when the picker is open. Kept `picker_stop_hover_button` hover-or-open-only inside the color picker stop list.
2. **Complete `#876` Coverage on Auto-Layout & Per-Side Stroke Weight Fields:**
   - Wrapped `al-counter-gap` in [`crates/designos-app/src/panels/properties.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/properties.rs) (`wrap_lines_ui`) in `var_bind::numeric(app, ui, pane, id, r, &["counterAxisSpacing"], ...)`.
   - Wrapped individual per-side stroke weight fields in [`crates/designos-app/src/panels/stroke.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/stroke.rs) in `var_bind::numeric` with `&["strokeTopWeight"]`, `&["strokeRightWeight"]`, `&["strokeBottomWeight"]`, and `&["strokeLeftWeight"]`.
3. **Typography Variable Bind Entry Points (`#133` Inspector Half):**
   - Wired all 8 typography properties in [`crates/designos-app/src/panels/typography.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/typography.rs):
     - `fontFamily` (`VariableKind::String`, `var_bind::pill_field` + `var_bind::value_rect` + `var_bind::hover_button`)
     - `fontStyle` (`VariableKind::String` via `"Apply variable…"` in the weight dropdown menu, with `var_bind::pill_field` rendering and `×` detach support for both `fontStyle` and legacy `fontWeight` `VariableKind::Number` bindings)
     - `fontSize` (`VariableKind::Number`, `var_bind::pill_field` + `var_bind::value_rect` + `var_bind::hover_button`)
     - `lineHeight` (`VariableKind::Number`, `var_bind::pill_field` + `var_bind::value_rect` + `var_bind::hover_button`)
     - `letterSpacing` (`VariableKind::Number`, `var_bind::pill_field` + `var_bind::value_rect` + `var_bind::hover_button`)
     - `fills/0` (`VariableKind::Color` on the text color row, narrowing `text-hex` via `var_bind::value_rect`)
     - `paragraphIndent` and `paragraphSpacing` (`VariableKind::Number`, `var_bind::numeric_in_session` inside `details_popover`)
4. **Scope Filtering & Subscribed Library Variables:**
   - Verified `scope_allows` mapping across all typography, stroke, layout, corner, opacity, fill, and effect keys, including subscribed library variables.
5. **Linked Text Style Detach on Typography Variable Bind:**
   - When `is_text_style_key(&key0)` (`fontFamily`, `fontStyle`, `fontWeight`, `fontSize`, `lineHeight`, `letterSpacing`, `paragraphSpacing`, `paragraphIndent`) is bound on a node with a linked `StyleKind::Text` ref, `n.style_refs.remove(StyleKind::Text.ref_key())` runs inside the same `Tx` (`"Bind Variable"` / `"Bind Library Variable"`), so one Undo restores both the text style ref and the previous binding state.
6. **Font Loading on `fontFamily` Variable Bind:**
   - Wrapped `PickResult::Picked` in `textsession::requesting_new_faces` and called `textsession::load_for_with(&app.doc, &app.fonts, new_faces)` so newly bound font families trigger font loading immediately.

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

Status: DONE
