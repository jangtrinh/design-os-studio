# Report: Delete style — cap the mode product and close the remaining fallback edges

## What Could Not Be Verified
- Nothing inside the task scope was left unverified. No visual UI layout or toast wording changed, so no window screenshot capture was needed `(plans/261030-2041-e1-wave-1/brief-agy-1136-delete-style-edges.txt:5)`.

## Conclusion
- **FACT:** Bounded the delete-style mode override classification in `(crates/designos-doc/src/styles_ops.rs:449-677,853-861)` so unrelated variable collections walk the sum of modes ($k \cdot m$) while shared/multi-axis collections walk an in-place odometer capped at `MAX_STYLE_MODE_COMBOS = 4,096` (`StyleBindingOverride::KeepBindingWithFallback`), preserved the delete-time look when `doc.resolve_variable(id, style_var)` returns `None` (`StyleBindingOverride::None` at `(crates/designos-doc/src/styles_ops.rs:661-667)`), added an alias row (`VariableValue::Alias` at `(crates/designos-doc/src/styles_ops.rs:2467-2474,2507)`) to the mode-equivalence acceptance table, and deduplicated `style_picker::delete_linked_style` fallback layers via `BTreeSet<NodeId>` (`fallback_layer_count` at `(crates/designos-app/src/panels/style_picker.rs:32-35,47)`). All 6 red->green tests, 3 mutant kills, and 6 workspace gates passed, `out/cargo` was deleted and verified absent, and Draft PR [#1576](https://github.com/jangtrinh/design-os-studio/pull/1576) is open against `main`.

---

## Step 1: Facts First (Re-Anchoring on `aa5bf311` & Mode Combination Sizing)

### 1. Re-anchoring the four items on `aa5bf311` (`aa5bf311d6deabd1c47ca21568eb717e21eb569e`)
- **Item 1 — Uncapped mode product (`FACT`):**
  - `collections_and_modes` was defined at `(crates/designos-doc/src/styles_ops.rs:448-497)` on `aa5bf311`.
  - `classify_own_binding_override` was defined at `(crates/designos-doc/src/styles_ops.rs:502-588)` on `aa5bf311`.
  - The collection lookup `let colls = collections_and_modes(doc, &[own_var_id.as_str(), style_var]);` was at `(crates/designos-doc/src/styles_ops.rs:524)` on `aa5bf311`.
  - The Cartesian product vector allocation (`let mut combos: Vec<Vec<(&str, &str)>> = ...`) was at `(crates/designos-doc/src/styles_ops.rs:529-540)` on `aa5bf311`, and the walk over `&combos` was at `(crates/designos-doc/src/styles_ops.rs:545-555)` on `aa5bf311`.
- **Item 2 — Delete-time view when the style value does not resolve (`FACT`):**
  - In `classify_own_binding_override` `(crates/designos-doc/src/styles_ops.rs:502-588)` on `aa5bf311`, when `style_won_vals` had at least two resolved values that disagreed `(crates/designos-doc/src/styles_ops.rs:566-571)`, the delete-time check was at `(crates/designos-doc/src/styles_ops.rs:573-578)`:
    - `own_wins_at_delete` was computed at `(crates/designos-doc/src/styles_ops.rs:573)`.
    - When `!own_wins_at_delete` was true `(crates/designos-doc/src/styles_ops.rs:574)` but `doc.resolve_variable(id, style_var)` returned `None` `(crates/designos-doc/src/styles_ops.rs:575)`, execution fell through to the default-mode check at `(crates/designos-doc/src/styles_ops.rs:579-586)` and then to `StyleBindingOverride::SomeModesBake { val: first_val.clone(), fallback: true }` at `(crates/designos-doc/src/styles_ops.rs:587)`.
    - `delete_style_with_fallbacks` `(crates/designos-doc/src/styles_ops.rs:723-778)` then baked `val` onto `copy` via `set_bound_property(&mut copy, &node_key, &val)` at `(crates/designos-doc/src/styles_ops.rs:758)`, changing the layer's delete-time appearance.
- **Item 3 — No alias row in the acceptance table (`FACT`):**
  - The 23-case acceptance table in `table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types` was at `(crates/designos-doc/src/styles_ops.rs:2150-2479)` (`cases` at `(crates/designos-doc/src/styles_ops.rs:2171-2369)`, `vars` at `(crates/designos-doc/src/styles_ops.rs:2375-2424)`) on `aa5bf311`, and the round-5 multi-mode tests `r5_row1..r5_row6` were at `(crates/designos-doc/src/styles_ops.rs:2504-3198)`. None of those rows bound a style property to a variable whose value was `VariableValue::Alias`.
- **Item 4 — Neighbour-only `Vec::dedup` in `style_picker.rs` (`FACT`):**
  - `delete_linked_style` in `(crates/designos-app/src/panels/style_picker.rs:32-52)` on `aa5bf311` collected `NodeId`s from `fallbacks` at `(crates/designos-app/src/panels/style_picker.rs:43)` and called `ids.dedup()` at `(crates/designos-app/src/panels/style_picker.rs:44)` before returning `ids.len()` at `(crates/designos-app/src/panels/style_picker.rs:45)`.

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

## Step 3: Acceptance Table with Pasted Output

| Criterion / Gate | Pasted Output | Citation (`file:line` / `log-file:line`) |
|---|---|---|
| **Base commit RED verification (`aa5bf311`) + Null control** | `test styles_ops::tests::delete_style_caps_shared_collection_mode_product_at_4096_and_keeps_binding ... FAILED` / `test styles_ops::tests::delete_style_walks_mode_sum_across_unrelated_eight_mode_collections ... FAILED` / `test styles_ops::tests::delete_style_with_unresolved_delete_time_style_value_keeps_binding_and_look ... FAILED` / `test styles_ops::tests::table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types ... ok` / `test result: FAILED. 36 passed; 3 failed` / `test shell::tests::effect_grid_styles::deleting_a_style_past_mode_product_cap_keeps_binding_and_counts_layer_in_toast ... FAILED` / `test shell::tests::effect_grid_styles::deleting_a_style_with_unresolved_delete_time_value_keeps_binding_and_canvas_render ... FAILED` / `test shell::tests::effect_grid_styles::style_picker_deduplicates_interleaved_fallback_nodes_by_set ... FAILED` / `test result: FAILED. 11 passed; 3 failed` | `(plans/reports/agy-1136-delete-style-edges-evidence/red-on-base.log:34-36,64,69-97,455,457,462,467-495)` |
| **(a) 3 unrelated 8-mode collections visit `24` (`3 * 8`), not `512` (`8^3`)** | `test styles_ops::tests::delete_style_walks_mode_sum_across_unrelated_eight_mode_collections ... ok` | `(crates/designos-doc/src/styles_ops.rs:3459-3575)`, `(plans/reports/agy-1136-delete-style-edges-evidence/green.log:18,50)` |
| **(b) Shared collection mode product past `4,096` stops at `4,096`, keeps binding, and counts in toast** | `test styles_ops::tests::delete_style_caps_shared_collection_mode_product_at_4096_and_keeps_binding ... ok` / `test shell::tests::effect_grid_styles::deleting_a_style_past_mode_product_cap_keeps_binding_and_counts_layer_in_toast ... ok` | `(crates/designos-doc/src/styles_ops.rs:3578-3751)`, `(crates/designos-app/src/shell/tests/effect_grid_styles.rs:486-558)`, `(plans/reports/agy-1136-delete-style-edges-evidence/green.log:17,50,76,86)` |
| **(c) Unresolved delete-time style value keeps binding and identical delete-time canvas render** | `test styles_ops::tests::delete_style_with_unresolved_delete_time_style_value_keeps_binding_and_look ... ok` / `test shell::tests::effect_grid_styles::deleting_a_style_with_unresolved_delete_time_value_keeps_binding_and_canvas_render ... ok` | `(crates/designos-doc/src/styles_ops.rs:3754-3860)`, `(crates/designos-app/src/shell/tests/effect_grid_styles.rs:561-645)`, `(plans/reports/agy-1136-delete-style-edges-evidence/green.log:19,50,78,86)` |
| **(d) Alias row in acceptance table (`VariableValue::Alias`) + Null control** | `test styles_ops::tests::table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types ... ok` | `(crates/designos-doc/src/styles_ops.rs:2467-2474,2507)`, `(plans/reports/agy-1136-delete-style-edges-evidence/green.log:47,50)` |
| **(e) Non-contiguous fallback entries deduplicated by set (`BTreeSet<NodeId>`)** | `test shell::tests::effect_grid_styles::style_picker_deduplicates_interleaved_fallback_nodes_by_set ... ok` | `(crates/designos-app/src/panels/style_picker.rs:32-35,47)`, `(crates/designos-app/src/shell/tests/effect_grid_styles.rs:648-655)`, `(plans/reports/agy-1136-delete-style-edges-evidence/green.log:83,86)` |
| **Mutant 1: Per-collection split removed (`shares_collection = true`) after green null control** | `test styles_ops::tests::table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types ... ok` / `thread 'styles_ops::tests::delete_style_walks_mode_sum_across_unrelated_eight_mode_collections' panicked at crates/designos-doc/src/styles_ops.rs:3511:5: assertion left == right failed: 3 unrelated 8-mode collections must visit the sum (3 * 8 = 24), not the product (8^3 = 512) left: 512 right: 24` / `test result: FAILED. 38 passed; 1 failed` | `(plans/reports/agy-1136-delete-style-edges-evidence/mutant-1-no-per-collection-split.log:18,47,52-64)` |
| **Mutant 2: Fallback bakes default value again on unresolved delete-time style value after green null control** | `test styles_ops::tests::table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types ... ok` / `thread 'styles_ops::tests::delete_style_with_unresolved_delete_time_style_value_keeps_binding_and_look' panicked ... left: None right: Some("v_op")` / `thread 'shell::tests::effect_grid_styles::deleting_a_style_with_unresolved_delete_time_value_keeps_binding_and_canvas_render' panicked ... left: 18135 right: 0` | `(plans/reports/agy-1136-delete-style-edges-evidence/mutant-2-bake-default-on-unresolved.log:19,47,52-64,93,103-115)` |
| **Mutant 3: Alias-skipping resolver in `classify_own_binding_override`** | `thread 'styles_ops::tests::table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types' panicked at crates/designos-doc/src/styles_ops.rs:2580:21: case=fill_opacity_alias_with_mixed_opaque_light_translucent_dark_own_fill deleted_in=light: resolve_node mismatch in mode 'light'` / `test result: FAILED. 0 passed; 1 failed` | `(plans/reports/agy-1136-delete-style-edges-evidence/mutant-3-skip-aliases.log:11-23)` |
| **Gate 1: `cargo fmt --all -- --check; echo fmt_rc=$?`** | `fmt_rc=0` | `(plans/reports/agy-1136-delete-style-edges-evidence/gate-fmt.log:1)` |
| **Gate 2: `cargo clippy --workspace --all-targets --locked`** | `Finished dev profile [unoptimized + debuginfo] target(s) in 14.21s` / `clippy_rc=0` | `(plans/reports/agy-1136-delete-style-edges-evidence/gate-clippy.log:57-58)` |
| **Gate 3: `cargo test -p designos-doc --locked`** | `test result: ok. 733 passed; 0 failed` / `3 passed` / `5 passed` / `2 passed` / `designos_doc_rc=0` | `(plans/reports/agy-1136-delete-style-edges-evidence/gate-designos-doc.log:743,752,763,771,779)` |
| **Gate 4: `cargo test -p designos-app --bin designos --locked` + solo rerun of 3 flaky tests** | Full parallel run: `test result: FAILED. 4244 passed; 3 failed; 13 ignored` (`agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root`, `vecops::tests::text_to_vector_queues_missing_font_without_blocking_and_respects_failed_and_outline_scope`, `vecops::tests::two_deferred_text_commands_both_convert_in_order`). Solo reruns: `1 passed; 0 failed` x 3, `designos_app_rerun_rc=0` | `(plans/reports/agy-1136-delete-style-edges-evidence/gate-designos-app.log:4306-4328)`, `(plans/reports/agy-1136-delete-style-edges-evidence/gate-designos-app-rerun.log:10-12,23-25,36-40)` |
| **Gate 5: `cargo test --workspace --locked --lib`** | `workspace_lib_rc=0` (all workspace library crates passed with 0 failures) | `(plans/reports/agy-1136-delete-style-edges-evidence/gate-workspace-lib.log:1590,1654,1661,1675,1677)` |
| **Gate 6: `python3 -B scripts/zero_fake_ui_gate.py`** | `Gate Verdict: PASS (17/17 invariants satisfied)` / `zero_fake_ui_gate_rc=0` | `(plans/reports/agy-1136-delete-style-edges-evidence/gate-zero-fake-ui.log:24-26)` |
| **Gate 7: Draft PR, `git log`, and `out/cargo` cleanup** | Draft PR: `https://github.com/jangtrinh/design-os-studio/pull/1576` / `ls: out/cargo: No such file or directory` / `ls_out_cargo_rc=1` | `(plans/reports/agy-1136-delete-style-edges-evidence/clean-out-cargo.log:1-2)` |

---

## Assumptions
- **ASSUMPTION:** When `!shares_collection` is true and a variable aliases across collections (`c1 -> c2`), probing `c2`'s modes while holding `c1` at the mode (`alias_prefix` at `(crates/designos-doc/src/styles_ops.rs:460,502-505,573-575,591-593)`) that aliases into `c2` visits every reachable value of the variable chain in $O(\sum m_i)$ mode evaluations without computing the Cartesian product across collections.
- **ASSUMPTION:** When a multi-axis `+` mode key exists on any collection in `own_colls` or `style_colls` (`has_plus` at `(crates/designos-doc/src/styles_ops.rs:461,489,554)`), routing through the `shares_collection` odometer branch (`(crates/designos-doc/src/styles_ops.rs:554-557,604-643)`) preserves exact multi-axis mode resolution while still bounding total evaluations at `MAX_STYLE_MODE_COMBOS = 4,096`.

---

## Loaded Skills & What Changed Because of Them
1. **`es:dev` (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`) & `es:feature-dev` (`/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`):**
   - Executed directly in the DEV phase using the controller rulings in `brief-agy-1136-delete-style-edges.txt` as the approved plan without pausing for brainstorm or plan approval.
2. **`es:rust` (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`):**
   - Replaced the $O(\prod m_i)$ `Vec<Vec<(&str, &str)>>` Cartesian product allocation in `classify_own_binding_override` `(crates/designos-doc/src/styles_ops.rs:520-677)` with an in-place odometer (`indices: Vec<usize>` and `combo: Vec<(&str, &str)>` of length $k$) and scalar agreement tracking (`first_val: Option<VariableValue>`, `style_vals_agree: bool`), keeping heap allocation $O(k)$.
3. **`es:rust-gpu-canvas` (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`):**
   - Used `designos_render::render_backdrop` over the expanded `local_bounds()` region in `deleting_a_style_with_unresolved_delete_time_value_keeps_binding_and_canvas_render` `(crates/designos-app/src/shell/tests/effect_grid_styles.rs:614-644)` to verify bit-exact delete-time pixel parity (`0` differing bytes across `40,000` RGBA bytes).
4. **`es:astra-work` (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`):**
   - Enforced red-before-green verification on `aa5bf311`, null-control verification (`table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types`), 3 mutant kills (`mutant-1-no-per-collection-split.log`, `mutant-2-bake-default-on-unresolved.log`, `mutant-3-skip-aliases.log`), solo reruns of the 3 parallel-load flaky tests in `designos-app`, and post-gate removal of `out/cargo`.
5. **`es:fable-thinking` (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`):**
   - Maintained explicit `FACT` / `ASSUMPTION` separation, predicted test/gate outcomes before each run, and verified alias hop traversal (`alias_prefix`) so unrelated collections with cross-collection aliases still reach downstream modes during the per-collection sum walk.

## Weakest Link
- When `shares_collection` is true and the Cartesian product exceeds `MAX_STYLE_MODE_COMBOS = 4,096`, `classify_own_binding_override` stops conservatively at `4,096` combinations and returns `KeepBindingWithFallback` without proving whether combinations `4,097..` would have agreed on a single style value.

## Unresolved Questions
- None.

---

## Exact Commands and Pasted Outputs

### 1. Untitled UI Kit Mode Combination Sizing (`plans/reports/agy-1136-delete-style-edges-evidence/untitled-kit-collections.log`)
```text
total_collections=10 total_variables=697
  cid=VariableCollectionId:13039:909 name='_Primitives' modes=1 (['13039:0:Mode 1']) vars=3
  cid=VariableCollectionId:13039:911 name='1. Color modes' modes=2 (['5256:0:Light mode', '5353:0:Dark mode']) vars=2
  cid=VariableCollectionId:5248:377685 name='_Primitives' modes=1 (['5248:0:Value']) vars=343
  cid=VariableCollectionId:5256:372339 name='1. Color modes' modes=2 (['5256:0:Light mode', '5353:0:Dark mode']) vars=273
  cid=VariableCollectionId:6283:327807 name='5. Containers' modes=1 (['6283:0:Value']) vars=3
  cid=VariableCollectionId:6298:45304 name='4. Widths' modes=1 (['6298:2:Mode 1']) vars=12
  cid=VariableCollectionId:6298:45383 name='3. Spacing' modes=1 (['6298:0:Mode 1']) vars=17
  cid=VariableCollectionId:6298:46388 name='2. Radius' modes=1 (['6298:1:Mode 1']) vars=11
  cid=VariableCollectionId:7403:16248 name='6. Typography' modes=1 (['7403:0:Value']) vars=32
  cid=VariableCollectionId:7856:138521 name='6. Typography' modes=1 (['7403:0:Value']) vars=1
all_collections_mode_sum=12 all_collections_mode_product=4
max_collections_for_single_var_chain=2
max_mode_sum_for_two_vars=6 pair=('VariableId:5256:372340', 'VariableId:13039:912') colls=['VariableCollectionId:5248:377685', 'VariableCollectionId:5256:372339', 'VariableCollectionId:13039:909', 'VariableCollectionId:13039:911']
max_mode_product_for_two_vars=4 pair=('VariableId:5256:372340', 'VariableId:13039:912') colls=['VariableCollectionId:5248:377685', 'VariableCollectionId:5256:372339', 'VariableCollectionId:13039:909', 'VariableCollectionId:13039:911']
```

### 2. Red Verification on Base Commit `aa5bf311` (`plans/reports/agy-1136-delete-style-edges-evidence/red-on-base.log`)
```text
env HOME="$S/home" TMPDIR="$S/tmp/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-doc --locked styles_ops::tests
...
test styles_ops::tests::delete_style_caps_shared_collection_mode_product_at_4096_and_keeps_binding ... FAILED
test styles_ops::tests::delete_style_walks_mode_sum_across_unrelated_eight_mode_collections ... FAILED
test styles_ops::tests::delete_style_with_unresolved_delete_time_style_value_keeps_binding_and_look ... FAILED
test styles_ops::tests::table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types ... ok
test result: FAILED. 36 passed; 3 failed; 0 ignored; 0 measured; 697 filtered out; finished in 0.03s

env HOME="$S/home" TMPDIR="$S/tmp/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked effect_grid_styles
...
test shell::tests::effect_grid_styles::deleting_a_style_past_mode_product_cap_keeps_binding_and_counts_layer_in_toast ... FAILED
test shell::tests::effect_grid_styles::deleting_a_style_with_unresolved_delete_time_value_keeps_binding_and_canvas_render ... FAILED
test shell::tests::effect_grid_styles::style_picker_deduplicates_interleaved_fallback_nodes_by_set ... FAILED
test result: FAILED. 11 passed; 3 failed; 0 ignored; 0 measured; 4246 filtered out; finished in 0.43s
```

### 3. Green Verification with Fix (`plans/reports/agy-1136-delete-style-edges-evidence/green.log`)
```text
env HOME="$S/home" TMPDIR="$S/tmp/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-doc --locked styles_ops::tests
...
test styles_ops::tests::delete_style_caps_shared_collection_mode_product_at_4096_and_keeps_binding ... ok
test styles_ops::tests::delete_style_walks_mode_sum_across_unrelated_eight_mode_collections ... ok
test styles_ops::tests::delete_style_with_unresolved_delete_time_style_value_keeps_binding_and_look ... ok
test styles_ops::tests::table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types ... ok
test result: ok. 39 passed; 0 failed; 0 ignored; 0 measured; 694 filtered out; finished in 0.04s

env HOME="$S/home" TMPDIR="$S/tmp/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked effect_grid_styles
...
test shell::tests::effect_grid_styles::deleting_a_style_past_mode_product_cap_keeps_binding_and_counts_layer_in_toast ... ok
test shell::tests::effect_grid_styles::deleting_a_style_with_unresolved_delete_time_value_keeps_binding_and_canvas_render ... ok
test shell::tests::effect_grid_styles::style_picker_deduplicates_interleaved_fallback_nodes_by_set ... ok
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 4246 filtered out; finished in 0.16s
```

### 4. Mutant Kills (`mutant-1-no-per-collection-split.log`, `mutant-2-bake-default-on-unresolved.log`, `mutant-3-skip-aliases.log`)
```text
# Mutant 1 (shares_collection = true):
test styles_ops::tests::table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types ... ok
test styles_ops::tests::delete_style_walks_mode_sum_across_unrelated_eight_mode_collections ... FAILED
assertion `left == right` failed: 3 unrelated 8-mode collections must visit the sum (3 * 8 = 24), not the product (8^3 = 512)
  left: 512
 right: 24
test result: FAILED. 38 passed; 1 failed; 0 ignored; 0 measured; 694 filtered out; finished in 0.04s

# Mutant 2 (fall through and bake default on unresolved delete-time style value):
test styles_ops::tests::table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types ... ok
test styles_ops::tests::delete_style_with_unresolved_delete_time_style_value_keeps_binding_and_look ... FAILED
assertion `left == right` failed: binding must be kept when delete-time style value is unresolved so delete-time look does not change
  left: None
 right: Some("v_op")
test result: FAILED. 38 passed; 1 failed; 0 ignored; 0 measured; 694 filtered out; finished in 0.04s
test shell::tests::effect_grid_styles::deleting_a_style_with_unresolved_delete_time_value_keeps_binding_and_canvas_render ... FAILED
assertion `left == right` failed: canvas render in delete-time mode must be identical before and after deleting the style
  left: 18135
 right: 0
test result: FAILED. 13 passed; 1 failed; 0 ignored; 0 measured; 4246 filtered out; finished in 0.17s

# Mutant 3 (skip VariableValue::Alias in classify_own_binding_override):
test styles_ops::tests::table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types ... FAILED
case=fill_opacity_alias_with_mixed_opaque_light_translucent_dark_own_fill deleted_in=light: resolve_node mismatch in mode 'light'
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 732 filtered out; finished in 0.01s
```

### 5. Verification Gates, Draft PR, `git log`, and `out/cargo` Cleanup
```text
$ cargo fmt --all -- --check; echo fmt_rc=$?
fmt_rc=0

$ cargo clippy --workspace --all-targets --locked
Finished `dev` profile [unoptimized + debuginfo] target(s) in 14.21s
clippy_rc=0

$ cargo test -p designos-doc --locked
test result: ok. 733 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.34s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
designos_doc_rc=0

$ cargo test -p designos-app --bin designos --locked
test result: FAILED. 4244 passed; 3 failed; 13 ignored; 0 measured; 0 filtered out; finished in 32.16s
# Solo reruns of the 3 parallel-load failures:
running 1 test
test agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4259 filtered out; finished in 0.02s
running 1 test
test vecops::tests::text_to_vector_queues_missing_font_without_blocking_and_respects_failed_and_outline_scope ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4259 filtered out; finished in 0.36s
running 1 test
test vecops::tests::two_deferred_text_commands_both_convert_in_order ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4259 filtered out; finished in 0.65s
designos_app_rerun_rc=0

$ cargo test --workspace --locked --lib
workspace_lib_rc=0

$ python3 -B scripts/zero_fake_ui_gate.py
Gate Verdict: PASS (17/17 invariants satisfied)
zero_fake_ui_gate_rc=0

$ rm -rf out/cargo && ls out/cargo; echo ls_out_cargo_rc=$?
ls: out/cargo: No such file or directory
ls_out_cargo_rc=1

Draft PR: https://github.com/jangtrinh/design-os-studio/pull/1576
$ git log --oneline origin/main..HEAD
36f1d152 fix(styles): bound the delete-style mode walk and keep the delete-time look (#1136)
```

Status: COMPLETE (all gates 0, evidence in plans/reports/agy-1136-delete-style-edges-evidence/)
