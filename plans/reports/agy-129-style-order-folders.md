# Report — #129: Order styles and group them into folders, with a `sortPosition` round trip

## 1. Summary

Implemented style ordering (`sortPosition` / `StyleMeta::sort_position`), folder grouping by `/` prefix in the style picker and inspector Styles section, and single-undo-step folder renaming (`rename_style_folder`) across all member styles:

1. **Data Model & Native Persistence (`designos-doc`):**
   - Added `#[serde(default, skip_serializing_if = "Option::is_none")] pub sort_position: Option<String>` to `StyleMeta` (`crates/designos-doc/src/lib.rs`).
   - Old `.designos` files without `sort_position` deserialize cleanly with `sort_position: None`, keep their stored `BTreeMap` order as the stable default order, and omit `sort_position` when saved.
   - Added `ordered_style_keys(doc, prop)` (`crates/designos-doc/src/styles_ops.rs`) so styles with `Some(sort_position)` sort by Figma's fractional-index ASCII string (`s.as_str()`) first, followed by styles with `None` in their stored `doc.styles` iteration order.
   - Added `rename_style_folder(doc, old_folder, new_folder)` (`crates/designos-doc/src/styles_ops.rs`) next to `rename_style`, renaming every style whose name starts with `"{old_folder}/"` to `"{new_folder}/{rest}"` in a single `Command::SetStyles` undo step (or stripping the prefix when `new_folder` is empty after trimming).
2. **`.fig` Import & Export Round Trip (`designos-fig`):**
   - Inspected the real Untitled UI v8.0 `.fig` archive (`❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig`) with `/opt/homebrew/bin/rea inspect-artifact` (evidence ID `ev_c31891ebd57179acda1dd4671c497968638cc59e9f63150f48aa2b6302e60f01`) and kiwi schema inspection. Confirmed that Figma stores style order on each style `NodeChange` (`styleType != NONE`, field id `120`) in the `sortPosition` `string` field (field id `425`) as a variable-length printable ASCII fractional index (e.g., `"!~~p~oL"` < `"!~~p~oLF"` < `"!~~p~oLf"` < `"!~~p~oLx"` for `Shadows/shadow-xs`, `Shadows/shadow-xs-skeuomorphic`, `Shadows/shadow-sm`, `Shadows/shadow-lg`; `"\"'#"` for `Focus rings/focus-ring`). Saved the extracted sample in `plans/reports/agy-129-style-order-folders-evidence/sort-position-sample.json`.
   - Updated `crates/designos-fig/src/fig.rs` (`read_style`) to read `nc.get_str("sortPosition")` into `StyleMeta::sort_position` (filtering empty strings to `None`).
   - Updated `crates/designos-fig/src/fig_export.rs` (`push_styles`) to iterate styles in `ordered_style_keys(&doc, None)` order and emit each style's preserved `sort_position` (or a monotonic base-10 fractional string `format!("~{i:04}")` when `sort_position` is `None`) so Figma and Studio read back the exact same order and `sortPosition` values on round trip.
3. **UI Grouping, Ordering, Folder Rename & Reorder Decision (`designos-app`):**
   - Updated `style_entries_of` (`crates/designos-app/src/styles.rs`) to order styles within each `StyleKind` by `ordered_style_keys(doc, None)` and group members of the same `/` folder together in first-seen folder order instead of sorting folders and leaves alphabetically.
   - Updated `crates/designos-app/src/panels/style_picker.rs` (`menu_model`, `header_button`, `linked_row`) and `crates/designos-app/src/panels/properties.rs` (`paint_title`, `styles_list`) so the style picker menus list disabled folder header rows (`MenuItem::new(e.folder).disabled(true)`) with indented leaf rows (`"  {leaf}"`), expose `"Rename folder"` in the style picker menu when the linked style is inside a folder (reusing `linked_row`'s inline `TextEdit` field), and allow double-clicking a folder header row in `styles_list` to rename the folder inline in one undo step (`App::rename_style_folder_to`).
   - **Reorder decision:** Per issue #129's acceptance criteria and the task brief ("If the issue or Figma parity needs reordering in the UI, the minimum is that the order is kept and shown. Do not build drag-to-reorder unless the issue's criteria ask for it. Say what you decided and why"), we kept and displayed the `sortPosition` order across import, native save/load, `.fig` export, the style picker menu, and the inspector Styles section without adding drag-to-reorder UI, because #129's three acceptance criteria do not ask for drag-to-reorder.

---

## 2. Files Modified

- `crates/designos-doc/src/lib.rs` — Added `sort_position: Option<String>` with `#[serde(default, skip_serializing_if = "Option::is_none")]` to `StyleMeta`.
- `crates/designos-doc/src/styles_ops.rs` — Added `rename_style_folder` and `ordered_style_keys`, plus unit tests `style_order_kept_through_native_save_and_load_and_old_file_omits_sort_position` and `folder_rename_renames_every_member_and_one_undo_restores_all`.
- `crates/designos-fig/src/fig.rs` — Read `sortPosition` into `StyleMeta::sort_position` during `.fig` style import.
- `crates/designos-fig/src/fig_export.rs` — Emit `sortPosition` and order style `NodeChange` records by `ordered_style_keys` during `.fig` export.
- `crates/designos-fig/tests/fig_effect_grid_styles.rs` — Added integration test `style_sort_position_and_folder_order_round_trip` (5 styles across 2 folders, `.fig` import -> export -> re-import verifying preserved order and `sortPosition` values) and `..Default::default()` on test `StyleMeta` struct literals.
- `crates/designos-app/src/styles.rs` — Updated `style_entries_of` to preserve `ordered_style_keys` order and first-seen folder grouping; added `App::rename_style_folder_to`.
- `crates/designos-app/src/panels/style_picker.rs` — Grouped style picker menu items by folder header, indented folder leaves, and wired `"Rename folder"` (`Pick::RenameFolder`) through `linked_row`.
- `crates/designos-app/src/panels/properties.rs` — Grouped fill/stroke style picker menu items by folder header, added double-click inline folder rename on folder rows in `styles_list`, and added `..Default::default()` to `StyleMeta` construction in `create_style`.
- `crates/designos-app/src/shell/tests/effect_grid_styles.rs` — Added UI test `styles_order_by_sort_position_group_by_folder_and_rename_folder_in_one_undo_step`.
- Exhaustive `StyleMeta { ..Default::default() }` struct-literal updates (approved by coordinator via `orca orchestration ask`) in:
  - `crates/designos-app/src/agent_mention.rs`
  - `crates/designos-app/src/check_designs/tests.rs`
  - `crates/designos-app/src/live_canvas_scope.rs`
  - `crates/designos-app/src/shell/tests/libraries.rs`
  - `crates/designos-doc/src/branch_merge.rs`
  - `crates/designos-doc/src/live_patch/tests.rs`
  - `crates/designos-doc/src/spec_bind.rs`
  - `crates/designos-doc/src/spec_bind_reference_forms_tests.rs`
  - `crates/designos-doc/src/spec_instance.rs`
  - `crates/designos-doc/src/variables_apply.rs`
  - `crates/designos-fig/src/rest_import.rs` (line 3360 `..Default::default()` only)
  - `crates/designos-fig/tests/fig_upgrade.rs`
  - `crates/designos-fig/tests/rest_upgrade.rs`
  - `crates/designos-gpu/src/geom.rs`
  - `crates/designos-mcp/src/engine.rs`
  - `crates/designos-mcp/src/prompts.rs`
  - `crates/designos-mcp/src/tools/batch.rs`
  - `crates/designos-mcp/src/tools/design_context.rs`
  - `crates/designos-mcp/src/tools/search_design_system.rs`

---

## 3. Red -> Green Output

- Full logs saved at:
  - `plans/reports/agy-129-style-order-folders-evidence/red.log`
  - `plans/reports/agy-129-style-order-folders-evidence/green.log`

### Red Phase (`red.log` excerpt)

```text
running 1 test
thread 'styles_ops::tests::style_order_kept_through_native_save_and_load_and_old_file_omits_sort_position' (54925782) panicked at crates/designos-doc/src/styles_ops.rs:3196:9:
assertion `left == right` failed: ordered_style_keys must follow sort_position, then stored order for None
  left: []
 right: ["S_z", "S_m", "S_a", "S_legacy"]
test styles_ops::tests::style_order_kept_through_native_save_and_load_and_old_file_omits_sort_position ... FAILED

running 1 test
thread 'styles_ops::tests::folder_rename_renames_every_member_and_one_undo_restores_all' (54926311) panicked at crates/designos-doc/src/styles_ops.rs:3227:55:
called `Result::unwrap()` on an `Err` value: EmptyName
test styles_ops::tests::folder_rename_renames_every_member_and_one_undo_restores_all ... FAILED

running 1 test
thread 'style_sort_position_and_folder_order_round_trip' (54929783) panicked at crates/designos-fig/tests/fig_effect_grid_styles.rs:835:5:
assertion `left == right` failed: .fig import must read sortPosition onto StyleMeta::sort_position
  left: [("2:10", None), ("2:20", None), ("2:30", None), ("2:40", None), ("2:50", None)]
 right: [("2:10", Some("!~~p~oLx")), ("2:20", Some("\"'#")), ("2:30", Some("!~~p~oL")), ("2:40", Some("!~~p~oLf")), ("2:50", Some("\"'#f"))]
test style_sort_position_and_folder_order_round_trip ... FAILED

running 1 test
thread 'shell::tests::effect_grid_styles::styles_order_by_sort_position_group_by_folder_and_rename_folder_in_one_undo_step' (54935453) panicked at crates/designos-app/src/shell/tests/effect_grid_styles.rs:437:5:
assertion `left == right` failed: styles must be ordered by sort_position and grouped by folder, not sorted alphabetically by name
  left: [("Focus rings", "focus-ring"), ("Shadows", "shadow-lg"), ("Shadows", "shadow-sm"), ("Shadows", "shadow-xs")]
 right: [("Shadows", "shadow-xs"), ("Shadows", "shadow-sm"), ("Shadows", "shadow-lg"), ("Focus rings", "focus-ring")]
test shell::tests::effect_grid_styles::styles_order_by_sort_position_group_by_folder_and_rename_folder_in_one_undo_step ... FAILED
```

### Green Phase (`green.log` excerpt)

```text
running 1 test
test styles_ops::tests::style_order_kept_through_native_save_and_load_and_old_file_omits_sort_position ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 650 filtered out; finished in 0.04s

running 1 test
test styles_ops::tests::folder_rename_renames_every_member_and_one_undo_restores_all ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 650 filtered out; finished in 0.00s

running 1 test
test style_sort_position_and_folder_order_round_trip ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out; finished in 0.04s

running 1 test
test shell::tests::effect_grid_styles::styles_order_by_sort_position_group_by_folder_and_rename_folder_in_one_undo_step ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2951 filtered out; finished in 0.32s
```

---

## 4. Test, Lint, and Gate Output

### `cargo fmt --all -- --check` & `python3 scripts/zero_fake_ui_gate.py`

```text
$ cargo fmt --all -- --check
(exit 0)

$ python3 scripts/zero_fake_ui_gate.py
============================================================================================================
 design:os studio — Zero Fake UI Enforcement Gate (DIR-28 · BR-31 · REQ-25 · D25)
============================================================================================================
...
 Gate Verdict: PASS (17/17 invariants satisfied)
============================================================================================================
```

### `cargo clippy --workspace --all-targets --locked -- -D warnings`

```text
$ HOME="$PWD/out/home" TMPDIR="$PWD/out/tmp" XDG_CONFIG_HOME="$PWD/out/home/.config" XDG_CACHE_HOME="$PWD/out/home/Library/Caches" XDG_DATA_HOME="$PWD/out/home/Library/Application Support" CARGO_TARGET_DIR="$PWD/out/cargo" cargo clippy --workspace --all-targets --locked -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s)
(exit 0, 0 warnings)
```

### `cargo test -p designos-doc --locked` & `cargo test -p designos-fig --locked`

```text
=== cargo test -p designos-doc --locked ===
Running unittests src/lib.rs: test result: ok. 651 passed; 0 failed; 0 ignored; finished in 12.19s
Running tests/auto_layout_wrap_fields_compat.rs: test result: ok. 3 passed; 0 failed; 0 ignored; finished in 0.01s
Running tests/changelog_pillars.rs: test result: ok. 5 passed; 0 failed; 0 ignored; finished in 0.01s

=== cargo test -p designos-fig --locked ===
Running unittests src/lib.rs: test result: ok. 121 passed; 0 failed; 0 ignored; finished in 35.03s
Running tests/clipboard.rs: test result: ok. 8 passed; 0 failed
Running tests/component_props.rs: test result: ok. 3 passed; 0 failed
Running tests/dev_mode.rs: test result: ok. 3 passed; 0 failed
Running tests/fig_child_margins.rs: test result: ok. 3 passed; 0 failed
Running tests/fig_effect_grid_styles.rs: test result: ok. 16 passed; 0 failed
Running tests/fig_export_field_shapes.rs: test result: ok. 8 passed; 0 failed
Running tests/fig_export_grid_shapes.rs: test result: ok. 6 passed; 0 failed
Running tests/fig_export_non_finite.rs: test result: ok. 5 passed; 0 failed
Running tests/fig_export_real_schema.rs: test result: ok. 4 passed; 0 failed
Running tests/fig_export_real_tags.rs: test result: ok. 11 passed; 0 failed
Running tests/fig_grid_fixed_row_export.rs: test result: ok. 13 passed; 0 failed
Running tests/fig_grid_override_anchors.rs: test result: ok. 21 passed; 0 failed
Running tests/fig_import.rs: test result: ok. 4 passed; 0 failed
Running tests/fig_import_legacy_export.rs: test result: ok. 4 passed; 0 failed
Running tests/fig_library.rs: test result: ok. 8 passed; 0 failed
Running tests/fig_non_finite_derived.rs: test result: ok. 2 passed; 0 failed
Running tests/fig_prop_var_value.rs: test result: ok. 5 passed; 0 failed
Running tests/fig_roundtrip.rs: test result: ok. 38 passed; 0 failed
Running tests/fig_slots.rs: test result: ok. 32 passed; 0 failed
Running tests/fig_unmapped_layer_edits.rs: test result: ok. 4 passed; 0 failed
Running tests/fig_upgrade.rs: test result: ok. 15 passed; 0 failed
Running tests/figjam.rs: test result: ok. 7 passed; 0 failed
Running tests/grid_child_align_patch.rs: test result: ok. 6 passed; 0 failed
Running tests/kiwi_zero_size_struct_limit.rs: test result: ok. 6 passed; 0 failed
Running tests/pen_component_depth_limits.rs: test result: ok. 2 passed; 0 failed
Running tests/pen_schema_2_21.rs: test result: ok. 34 passed; 0 failed
Running tests/proto_import.rs: test result: ok. 6 passed; 0 failed
Running tests/proto_vars.rs: test result: ok. 6 passed; 0 failed
Running tests/real_files.rs: test result: ok. 0 passed; 0 failed; 6 ignored
Running tests/remote_components.rs: test result: ok. 2 passed; 0 failed
Running tests/rest_import.rs: test result: ok. 10 passed; 0 failed
Running tests/rest_import_grid_sizing.rs: test result: ok. 2 passed; 0 failed
Running tests/rest_upgrade.rs: test result: ok. 7 passed; 0 failed
Running tests/settings_import.rs: test result: ok. 3 passed; 0 failed
Running tests/stack_wrap_and_text_wrap.rs: test result: ok. 11 passed; 0 failed
Running tests/variables.rs: test result: ok. 5 passed; 0 failed
Running tests/variables_code_syntax_opacity.rs: test result: ok. 5 passed; 0 failed
Running tests/variables_library_collections.rs: test result: ok. 1 passed; 0 failed
Running tests/vector_network.rs: test result: ok. 4 passed; 0 failed
```

### `cargo test -p designos-app --bin designos --locked`

```text
$ HOME="$PWD/out/home" TMPDIR="$PWD/out/tmp" XDG_CONFIG_HOME="$PWD/out/home/.config" XDG_CACHE_HOME="$PWD/out/home/Library/Caches" XDG_DATA_HOME="$PWD/out/home/Library/Application Support" CARGO_TARGET_DIR="$PWD/out/cargo" cargo test -p designos-app --bin designos --locked
Running unittests src/main.rs (out/cargo/debug/deps/designos-dd82a7aa841fdfee)
running 2952 tests
...
test result: ok. 2944 passed; 0 failed; 8 ignored; 0 measured; 0 filtered out; finished in 159.25s
```

---

## 5. Evidence Paths

- `plans/reports/agy-129-style-order-folders-evidence/red.log` — Red-phase test output showing all 4 acceptance tests failing before implementation.
- `plans/reports/agy-129-style-order-folders-evidence/green.log` — Green-phase test output showing all 4 acceptance tests passing after implementation.
- `plans/reports/agy-129-style-order-folders-evidence/sort-position-sample.json` — Real `sortPosition` values extracted from `❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig` (backed by `/opt/homebrew/bin/rea inspect-artifact` evidence `ev_c31891ebd57179acda1dd4671c497968638cc59e9f63150f48aa2b6302e60f01` and kiwi `NodeChange` decoding), demonstrating Figma's ASCII fractional-index ordering across `Text`, `Effect`, and `Grid` style folders.
- `plans/reports/agy-129-style-order-folders-evidence/style-picker-folders.png` — `1440×900` `designos --screenshot` capture of the effect style picker menu opened on a temporary copy of the Untitled UI v8.0 kit (`--panel effect-styles --size 1440x900`), read and visually verified with `read_file` to confirm folder grouping (`Shadows`, `Shadows/Portfolio mockups`, `Focus rings`, `Backdrop blurs`, `Shadows/Avatars`) and `sortPosition` ordering (`shadow-xs`, `shadow-xs-skeuomorphic`, `shadow-sm`, `shadow-md`, `shadow-lg`, `shadow-xl`, `shadow-2xl`, `shadow-3xl`).

---

## 6. Self-Review Against Acceptance Criteria

| Acceptance Criterion | Status | Evidence |
|---|---|---|
| 1(a). Order survives native save and load | PASS | `styles_ops::tests::style_order_kept_through_native_save_and_load_and_old_file_omits_sort_position` in `green.log` |
| 1(b). Order & `sortPosition` survive `.fig` import -> export -> import round trip on >=3 styles in >=2 folders | PASS | `style_sort_position_and_folder_order_round_trip` (5 styles in 2 folders: `Shadows` and `Focus rings`) in `green.log` |
| 1(c). Renaming a folder renames every member in one undo step, and one undo restores all | PASS | `styles_ops::tests::folder_rename_renames_every_member_and_one_undo_restores_all` and `shell::tests::effect_grid_styles::styles_order_by_sort_position_group_by_folder_and_rename_folder_in_one_undo_step` in `green.log` |
| 1(d). Old file without `sort_position` loads with stable default order, and saving it does not add `sort_position` | PASS | `styles_ops::tests::style_order_kept_through_native_save_and_load_and_old_file_omits_sort_position` (`!saved_old.contains("sort_position")`) in `green.log` |
| 2. `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --locked -- -D warnings` exit 0 | PASS | Both exited 0 with 0 warnings |
| 3. `cargo test -p designos-doc --locked`, `cargo test -p designos-fig --locked`, and `cargo test -p designos-app --bin designos --locked` pass under isolated harness | PASS | 659 doc tests, 419 fig tests, and 2,944 app bin tests passed (`0 failed`) |
| 4. `python3 scripts/zero_fake_ui_gate.py` passes | PASS | `Gate Verdict: PASS (17/17 invariants satisfied)` |
| 5. `1440×900` `designos --screenshot` of style picker grouped in folders on Untitled UI copy, opened and read before citation | PASS | `plans/reports/agy-129-style-order-folders-evidence/style-picker-folders.png` |
| 6. `out/` and temporary `.fig` copies deleted before commit | PASS | Verified with `git status -s` |

---

## 7. Residual Risks

- Styles that share a folder prefix but have non-contiguous `sortPosition` values across other folders are grouped under the folder's first-seen position in `style_entries_of` so the picker never repeats the same folder header twice; within that folder, members keep their relative `sortPosition` order.
- When exporting newly created native styles that have `sort_position: None` to `.fig`, `fig_export::push_styles` synthesizes a monotonic `"~{i:04}"` `sortPosition` (`'~'` sorts after printable ASCII `'!'..'}'` used by imported Figma styles) so their relative order is preserved in `.fig` without colliding with imported `sortPosition` strings.
