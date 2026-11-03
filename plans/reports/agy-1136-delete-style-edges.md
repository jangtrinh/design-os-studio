# Report: Delete style — cap the mode product and close the remaining fallback edges

## What Could Not Be Verified
- Nothing inside the task scope was left unverified. No visual UI layout or toast wording changed, so no window screenshot capture was needed (`plans/261030-2041-e1-wave-1/brief-agy-1136-delete-style-edges.txt:5`).

---

## Step 1: Facts First (Re-Anchoring on `aa5bf311` & Mode Combination Sizing)

### 1. Re-anchoring the four items on `aa5bf311` (`aa5bf311d6deabd1c47ca21568eb717e21eb569e`)
- **Item 1 — Uncapped mode product (`FACT`):**
  - `collections_and_modes` is defined at `(crates/designos-doc/src/styles_ops.rs:448-497)` on `aa5bf311`.
  - `classify_own_binding_override` is defined at `(crates/designos-doc/src/styles_ops.rs:502-588)` on `aa5bf311`.
  - The collection lookup `let colls = collections_and_modes(doc, &[own_var_id.as_str(), style_var]);` is at `(crates/designos-doc/src/styles_ops.rs:524)` on `aa5bf311`.
  - The Cartesian product vector allocation (`let mut combos: Vec<Vec<(&str, &str)>> = ...`) is at `(crates/designos-doc/src/styles_ops.rs:529-540)` on `aa5bf311`, and the walk over `&combos` is at `(crates/designos-doc/src/styles_ops.rs:545-555)` on `aa5bf311`.
- **Item 2 — Delete-time view when the style value does not resolve (`FACT`):**
  - In `classify_own_binding_override` `(crates/designos-doc/src/styles_ops.rs:502-588)` on `aa5bf311`, when `style_won_vals` has at least two resolved values that disagree `(crates/designos-doc/src/styles_ops.rs:566-571)`, the delete-time check is at `(crates/designos-doc/src/styles_ops.rs:573-578)`:
    - `own_wins_at_delete` is computed at `(crates/designos-doc/src/styles_ops.rs:573)`.
    - When `!own_wins_at_delete` is true `(crates/designos-doc/src/styles_ops.rs:574)` but `doc.resolve_variable(id, style_var)` returns `None` `(crates/designos-doc/src/styles_ops.rs:575)`, execution falls through to the default-mode check at `(crates/designos-doc/src/styles_ops.rs:579-586)` and then to `StyleBindingOverride::SomeModesBake { val: first_val.clone(), fallback: true }` at `(crates/designos-doc/src/styles_ops.rs:587)`.
    - `delete_style_with_fallbacks` `(crates/designos-doc/src/styles_ops.rs:723-778)` then bakes `val` onto `copy` via `set_bound_property(&mut copy, &node_key, &val)` at `(crates/designos-doc/src/styles_ops.rs:758)`, changing the layer's delete-time appearance.
- **Item 3 — No alias row in the acceptance table (`FACT`):**
  - The 23-case acceptance table in `table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types` is at `(crates/designos-doc/src/styles_ops.rs:2150-2479)` (`cases` at `(crates/designos-doc/src/styles_ops.rs:2171-2369)`, `vars` at `(crates/designos-doc/src/styles_ops.rs:2375-2424)`) on `aa5bf311`, and the round-5 multi-mode tests `r5_row1..r5_row6` are at `(crates/designos-doc/src/styles_ops.rs:2504-3198)`. None of those rows binds a style property to a variable whose value is `VariableValue::Alias`.
- **Item 4 — Neighbour-only `Vec::dedup` in `style_picker.rs` (`FACT`):**
  - `delete_linked_style` in `(crates/designos-app/src/panels/style_picker.rs:32-52)` on `aa5bf311` collects `NodeId`s from `fallbacks` at `(crates/designos-app/src/panels/style_picker.rs:43)` and calls `ids.dedup()` at `(crates/designos-app/src/panels/style_picker.rs:44)` before returning `ids.len()` at `(crates/designos-app/src/panels/style_picker.rs:45)`.

### 2. Mode Combination Sizing (`FACT`)
- **Formula for $k$ collections with $m$ modes each:**
  - Full Cartesian product walk `(crates/designos-doc/src/styles_ops.rs:529-540)`: visits $m^k$ combinations (for $k = 3, m = 8$: $8^3 = 512$ combinations; for $k = 2, m = 8$: $8^2 = 64$ combinations).
  - Per-collection walk when variables do not share a collection: visits $k \cdot m$ modes (for $k = 3, m = 8$: $3 \times 8 = 24$ modes; for $k = 2, m = 8$: $2 \times 8 = 16$ modes).
- **Largest real case in the Untitled UI kit (APFS clone `out/evidence/untitled-clone.fig`):**
  - Imported variables and collections from the APFS clone were exported via `designos out/evidence/untitled-clone.fig --export-variables out/evidence/untitled-vars.json --format figma-rest` and recorded in `(plans/reports/agy-1136-delete-style-edges-evidence/untitled-kit-collections.log:1-15)`.
  - Total collections in the imported document: `10`, total variables: `697` `(plans/reports/agy-1136-delete-style-edges-evidence/untitled-kit-collections.log:1)`.
  - Collection mode counts `(plans/reports/agy-1136-delete-style-edges-evidence/untitled-kit-collections.log:2-11)`:
    - `VariableCollectionId:13039:911` (`1. Color modes`): `2` modes (`5256:0:Light mode`, `5353:0:Dark mode`), `2` variables `(plans/reports/agy-1136-delete-style-edges-evidence/untitled-kit-collections.log:3)`.
    - `VariableCollectionId:5256:372339` (`1. Color modes`): `2` modes (`5256:0:Light mode`, `5353:0:Dark mode`), `273` variables `(plans/reports/agy-1136-delete-style-edges-evidence/untitled-kit-collections.log:5)`.
    - 8 single-mode collections (`1` mode each): `VariableCollectionId:13039:909` (`_Primitives`, `3` vars), `VariableCollectionId:5248:377685` (`_Primitives`, `343` vars), `VariableCollectionId:6283:327807` (`5. Containers`, `3` vars), `VariableCollectionId:6298:45304` (`4. Widths`, `12` vars), `VariableCollectionId:6298:45383` (`3. Spacing`, `17` vars), `VariableCollectionId:6298:46388` (`2. Radius`, `11` vars), `VariableCollectionId:7403:16248` (`6. Typography`, `32` vars), `VariableCollectionId:7856:138521` (`6. Typography`, `1` var) `(plans/reports/agy-1136-delete-style-edges-evidence/untitled-kit-collections.log:2,4,6-11)`.
  - Across all `10` collections in the document: mode sum = `12`, mode product = `4` ($2 \times 2 \times 1^8$) `(plans/reports/agy-1136-delete-style-edges-evidence/untitled-kit-collections.log:12)`.
  - Largest reachable `(own_var, style_var)` pair in the kit: `4` collections (`VariableCollectionId:5256:372339` -> `VariableCollectionId:5248:377685` paired with `VariableCollectionId:13039:911` -> `VariableCollectionId:13039:909`), with mode sum = `6` ($2 + 1 + 2 + 1$) and mode product = `4` ($2 \times 1 \times 2 \times 1$) `(plans/reports/agy-1136-delete-style-edges-evidence/untitled-kit-collections.log:15)`.

---

## Step 2: Implementation of Rulings 1–5

1. **Ruling 1 — Bounded walk (`FACT`):**
   - Defined `MAX_STYLE_MODE_COMBOS: usize = 4096` at `(crates/designos-doc/src/styles_ops.rs:449)` and added `StyleBindingOverride::KeepBindingWithFallback` at `(crates/designos-doc/src/styles_ops.rs:451-456)`.
   - Added `VarCollectionModes` `(crates/designos-doc/src/styles_ops.rs:458-462)` and `collections_for_var` `(crates/designos-doc/src/styles_ops.rs:464-516)` to collect per-variable collections, modes, alias hop prefixes, and multi-axis (`+`) mode flags.
   - In `classify_own_binding_override` `(crates/designos-doc/src/styles_ops.rs:520-677)`:
     - Computed `shares_collection` at `(crates/designos-doc/src/styles_ops.rs:554)`.
     - When `!shares_collection` `(crates/designos-doc/src/styles_ops.rs:562-603)`: classifies `own_colls` and `style_colls` per collection (sum of modes, not product) using a single reusable `probe: Vec<(&str, &str)>` `(crates/designos-doc/src/styles_ops.rs:563)`, stopping at `MAX_STYLE_MODE_COMBOS` (`4,096`) and returning `StyleBindingOverride::KeepBindingWithFallback` `(crates/designos-doc/src/styles_ops.rs:569-571,587-589)`.
     - When `shares_collection` `(crates/designos-doc/src/styles_ops.rs:604-643)`: iterates the Cartesian product in-place via an odometer (`indices: Vec<usize>` of length `k` and `combo: Vec<(&str, &str)>` of length `k` at `(crates/designos-doc/src/styles_ops.rs:605-607)`, with `first_val: Option<VariableValue>` and `style_vals_agree: bool` at `(crates/designos-doc/src/styles_ops.rs:558-559)`), so zero heap allocation grows with the product. When `visits == MAX_STYLE_MODE_COMBOS` (`4,096`), returns `StyleBindingOverride::KeepBindingWithFallback` at `(crates/designos-doc/src/styles_ops.rs:609-611)`.
   - In `delete_style_with_fallbacks` `(crates/designos-doc/src/styles_ops.rs:823-876)`, `StyleBindingOverride::KeepBindingWithFallback` keeps the binding on `copy.variables.bindings` (without baking) and pushes `(id, node_key)` into `fallbacks` at `(crates/designos-doc/src/styles_ops.rs:853-861)`.
2. **Ruling 2 — Delete-time look never changes (`FACT`):**
   - In `classify_own_binding_override` at `(crates/designos-doc/src/styles_ops.rs:661-667)`, when `!own_wins_at_delete`, `match doc.resolve_variable(id, style_var)` returns `StyleBindingOverride::SomeModesBake { val, fallback: true }` on `Some(val)` `(crates/designos-doc/src/styles_ops.rs:664)` and `StyleBindingOverride::None` on `None` `(crates/designos-doc/src/styles_ops.rs:665)`, keeping the layer's binding and never baking the default or first value when the delete-time mode itself does not resolve.
3. **Ruling 3 — Alias row (`FACT`):**
   - Added case `fill_opacity_alias_with_mixed_opaque_light_translucent_dark_own_fill` at `(crates/designos-doc/src/styles_ops.rs:2467-2474)` with `v_op_alias` defined via `VariableValue::Alias("v_op".into())` at `(crates/designos-doc/src/styles_ops.rs:2507)` in `table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types` `(crates/designos-doc/src/styles_ops.rs:2248-2585)`.
4. **Ruling 4 — Dedup by set (`FACT`):**
   - Replaced neighbour-only `Vec::dedup` in `delete_linked_style` `(crates/designos-app/src/panels/style_picker.rs:37-53)` with `fallback_layer_count` `(crates/designos-app/src/panels/style_picker.rs:32-35)`, which deduplicates `NodeId`s via `std::collections::BTreeSet<NodeId>`.
5. **Ruling 5 — Nothing else changes (`FACT`):**
   - All single-collection and single-mode deletes and all existing rows in `table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types` `(crates/designos-doc/src/styles_ops.rs:2248-2585)` and `r5_row1..r5_row6` `(crates/designos-doc/src/styles_ops.rs:2606-3300)` retain their exact behavior.

---

## Step 3: Acceptance Evidence (Red -> Green, Mutants, and Gates)

### Acceptance 1: Red on `aa5bf311`, Green with Fix, and Null Control
- **Tests added:**
  - **(a) Unrelated 3x8 collections visit sum (`24`), not product (`512`):** `styles_ops::tests::delete_style_walks_mode_sum_across_unrelated_eight_mode_collections` `(crates/designos-doc/src/styles_ops.rs:3459-3575)`, using test-only visit counter `take_style_mode_visits` `(crates/designos-doc/src/styles_ops.rs:39-47)`.
  - **(b) Shared collection mode product past `4,096` stops at `4,096`, keeps binding, and counts in toast:** `styles_ops::tests::delete_style_caps_shared_collection_mode_product_at_4096_and_keeps_binding` `(crates/designos-doc/src/styles_ops.rs:3578-3751)` (testing both a 4-axis shared collection with $9^4 = 6,561$ combinations at `(crates/designos-doc/src/styles_ops.rs:3581-3670)` and a single shared collection with `4,100` modes at `(crates/designos-doc/src/styles_ops.rs:3672-3750)`) and `shell::tests::effect_grid_styles::deleting_a_style_past_mode_product_cap_keeps_binding_and_counts_layer_in_toast` `(crates/designos-app/src/shell/tests/effect_grid_styles.rs:486-558)`.
  - **(c) Unresolved delete-time style value keeps binding and identical delete-time canvas render:** `styles_ops::tests::delete_style_with_unresolved_delete_time_style_value_keeps_binding_and_look` `(crates/designos-doc/src/styles_ops.rs:3754-3860)` and `shell::tests::effect_grid_styles::deleting_a_style_with_unresolved_delete_time_value_keeps_binding_and_canvas_render` `(crates/designos-app/src/shell/tests/effect_grid_styles.rs:561-645)`.
  - **(d) Alias row in acceptance table:** `fill_opacity_alias_with_mixed_opaque_light_translucent_dark_own_fill` in `styles_ops::tests::table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types` `(crates/designos-doc/src/styles_ops.rs:2467-2474,2507)`.
  - **(e) Interleaved fallback node deduplication by set:** `shell::tests::effect_grid_styles::style_picker_deduplicates_interleaved_fallback_nodes_by_set` `(crates/designos-app/src/shell/tests/effect_grid_styles.rs:648-655)`.
- **Red run on `aa5bf311` (`plans/reports/agy-1136-delete-style-edges-evidence/red-on-base.log`):**
  - `styles_ops::tests::delete_style_walks_mode_sum_across_unrelated_eight_mode_collections`: FAILED (`left: 512`, `right: 24`) `(plans/reports/agy-1136-delete-style-edges-evidence/red-on-base.log:35,77-83)`.
  - `styles_ops::tests::delete_style_caps_shared_collection_mode_product_at_4096_and_keeps_binding`: FAILED (`left: 6561`, `right: 4096`) `(plans/reports/agy-1136-delete-style-edges-evidence/red-on-base.log:34,69-75)`.
  - `styles_ops::tests::delete_style_with_unresolved_delete_time_style_value_keeps_binding_and_look`: FAILED (`left: None`, `right: Some("v_op")`) `(plans/reports/agy-1136-delete-style-edges-evidence/red-on-base.log:36,84-90)`.
  - `shell::tests::effect_grid_styles::deleting_a_style_past_mode_product_cap_keeps_binding_and_counts_layer_in_toast`: FAILED (`left: None`, `right: Some("v_fx_op")`) `(plans/reports/agy-1136-delete-style-edges-evidence/red-on-base.log:455,467-473)`.
  - `shell::tests::effect_grid_styles::deleting_a_style_with_unresolved_delete_time_value_keeps_binding_and_canvas_render`: FAILED (`left: 18135` differing bytes, `right: 0`) `(plans/reports/agy-1136-delete-style-edges-evidence/red-on-base.log:457,475-480)`.
  - `shell::tests::effect_grid_styles::style_picker_deduplicates_interleaved_fallback_nodes_by_set`: FAILED (`left: 3`, `right: 2`) `(plans/reports/agy-1136-delete-style-edges-evidence/red-on-base.log:462,482-488)`.
  - Null control on `aa5bf311`: `styles_ops::tests::table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types` passed `(plans/reports/agy-1136-delete-style-edges-evidence/red-on-base.log:64)`, along with all `36` pre-existing `styles_ops::tests` `(plans/reports/agy-1136-delete-style-edges-evidence/red-on-base.log:97)` and all `11` pre-existing `effect_grid_styles` tests `(plans/reports/agy-1136-delete-style-edges-evidence/red-on-base.log:495)`.
- **Green run with fix (`plans/reports/agy-1136-delete-style-edges-evidence/green.log`):**
  - `designos-doc` (`styles_ops::tests`): `39 passed; 0 failed` `(plans/reports/agy-1136-delete-style-edges-evidence/green.log:17-19,47,50)`.
  - `designos-app` (`shell::tests::effect_grid_styles`): `14 passed; 0 failed` `(plans/reports/agy-1136-delete-style-edges-evidence/green.log:76,78,83,86)`.

### Acceptance 2: Mutant Kills After Green Null Control
1. **Mutant 1 — Per-collection split removed (product again) (`plans/reports/agy-1136-delete-style-edges-evidence/mutant-1-no-per-collection-split.log`):**
   - Forcing `shares_collection = true` at `(crates/designos-doc/src/styles_ops.rs:554)`:
   - Null control `styles_ops::tests::table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types` passed (`ok`) `(plans/reports/agy-1136-delete-style-edges-evidence/mutant-1-no-per-collection-split.log:47)`.
   - Killed by `styles_ops::tests::delete_style_walks_mode_sum_across_unrelated_eight_mode_collections` (`left: 512`, `right: 24`; `38 passed; 1 failed`) `(plans/reports/agy-1136-delete-style-edges-evidence/mutant-1-no-per-collection-split.log:18,52-64)`.
2. **Mutant 2 — Fallback bakes the default value again (`plans/reports/agy-1136-delete-style-edges-evidence/mutant-2-bake-default-on-unresolved.log`):**
   - Restoring the `if let Some(val) = doc.resolve_variable(id, style_var)` fall-through at `(crates/designos-doc/src/styles_ops.rs:662-667)`:
   - Null control `styles_ops::tests::table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types` passed (`ok`) `(plans/reports/agy-1136-delete-style-edges-evidence/mutant-2-bake-default-on-unresolved.log:47)`.
   - Killed by `styles_ops::tests::delete_style_with_unresolved_delete_time_style_value_keeps_binding_and_look` (`left: None`, `right: Some("v_op")`; `38 passed; 1 failed`) `(plans/reports/agy-1136-delete-style-edges-evidence/mutant-2-bake-default-on-unresolved.log:19,52-64)` and `shell::tests::effect_grid_styles::deleting_a_style_with_unresolved_delete_time_value_keeps_binding_and_canvas_render` (`left: 18135`, `right: 0`; `13 passed; 1 failed`) `(plans/reports/agy-1136-delete-style-edges-evidence/mutant-2-bake-default-on-unresolved.log:93,103-115)`.
3. **Mutant 3 — Alias-skipping resolver in `classify_own_binding_override` (`plans/reports/agy-1136-delete-style-edges-evidence/mutant-3-skip-aliases.log`):**
   - Skipping `VariableValue::Alias` when resolving `style_var` at `(crates/designos-doc/src/styles_ops.rs:619)`:
   - Killed by `styles_ops::tests::table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types` on `case=fill_opacity_alias_with_mixed_opaque_light_translucent_dark_own_fill deleted_in=light: resolve_node mismatch in mode 'light'` (`opacity: 1.0` vs `opacity: 0.5`) `(plans/reports/agy-1136-delete-style-edges-evidence/mutant-3-skip-aliases.log:11-23)`.

### Acceptance 3: Verification Gates
- **`cargo fmt --all -- --check; echo fmt_rc=$?`:** `fmt_rc=0` `(plans/reports/agy-1136-delete-style-edges-evidence/gate-fmt.log:1)`.
- **`cargo clippy --workspace --all-targets --locked`:** `clippy_rc=0` `(plans/reports/agy-1136-delete-style-edges-evidence/gate-clippy.log:57-58)`.
- **`cargo test -p designos-doc --locked`:** `733 passed` (lib) + `3 passed` (`auto_layout_wrap_fields_compat`) + `5 passed` (`changelog_pillars`) + `2 passed` (`shadow_blend`) = `743 passed; 0 failed`, `designos_doc_rc=0` `(plans/reports/agy-1136-delete-style-edges-evidence/gate-designos-doc.log:743,752,763,771,779)`.
- **`cargo test -p designos-app --bin designos --locked`:**
  - Full parallel run (`4,260` tests): `4244 passed; 3 failed; 13 ignored` `(plans/reports/agy-1136-delete-style-edges-evidence/gate-designos-app.log:4328)`.
  - The 3 unrelated failures under full-suite parallel load (`agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root` at `(plans/reports/agy-1136-delete-style-edges-evidence/gate-designos-app.log:4306-4310)` due to `/var/folders/...` socket path length, `vecops::tests::text_to_vector_queues_missing_font_without_blocking_and_respects_failed_and_outline_scope` at `(plans/reports/agy-1136-delete-style-edges-evidence/gate-designos-app.log:4312-4315)`, and `vecops::tests::two_deferred_text_commands_both_convert_in_order` at `(plans/reports/agy-1136-delete-style-edges-evidence/gate-designos-app.log:4317-4320)`) were rerun alone in `(plans/reports/agy-1136-delete-style-edges-evidence/gate-designos-app-rerun.log:1-40)` and all 3 passed (`1 passed; 0 failed` at lines `10-12`, `23-25`, and `36-38`; `designos_app_rerun_rc=0` at line `40`).
- **`cargo test --workspace --locked --lib`:** All workspace library unit test suites passed (`workspace_lib_rc=0`) `(plans/reports/agy-1136-delete-style-edges-evidence/gate-workspace-lib.log:1590,1654,1661,1675,1677)`.
- **`python3 -B scripts/zero_fake_ui_gate.py`:** `Gate Verdict: PASS (17/17 invariants satisfied)`, `zero_fake_ui_gate_rc=0` `(plans/reports/agy-1136-delete-style-edges-evidence/gate-zero-fake-ui.log:24-26)`.

---

## Step 4: Target Cleanup (`out/cargo`)
- Executed `rm -rf out/cargo` and verified `ls out/cargo` fails `(plans/reports/agy-1136-delete-style-edges-evidence/clean-out-cargo.log:1-2)`:
```text
ls: out/cargo: No such file or directory
ls_out_cargo_rc=1
```
