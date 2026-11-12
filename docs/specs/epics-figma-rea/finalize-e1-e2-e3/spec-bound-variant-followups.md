# Bound variant follow-ups: what Figma does, and the fix spec for Studio

Date 2026-11-12, milestone v0.7.14. Base `origin/main@e0e44f3c2`. Research and spec only: no product code changed, nothing committed.
Evidence: `evidence/re/` (sources and hashes in `00-sources-and-builds.txt`). Clean room: own words, offsets, identifiers and UI strings only. No Figma code copied.

## Answer first

- **Figma stores one variant binding per instance, as a map with one entry per property.** Unbinding a property removes only its entry. A hand pick on another, unbound property swaps the variant and leaves the map alone. So a pick on `State` does not stop `Size` from following its variable (seen, Q1).
- **Figma's inspector cannot hand-pick a bound property.** A bound row shows a variable pill with a "Detach property" button instead of the dropdown. Nested instance rows use the same row component as top-level rows, so they get the same pill, "Apply variable" and "Detach property" (seen, Q1 and Q5).
- **Unknown:** what a plain value written through the plugin API does to an existing binding (Q2). The JS hands the call straight to the engine. Studio keeps the owner's existing ruling, "a pick holds until reset", and applies it per property and the same way at both levels.
- **Inferred, not proven:** Figma re-solves layout in passes until nothing is dirty (Q3). The prototype player carries the full variant-binding resolver, the nested override transfer and its own "set variable mode" runner (Q4).
- **The Studio fixes, in build order:**
  1. Record picks per property (#1901 and #1902 together).
  2. Lay out again after the bound-property resync inside the same history step (#1903).
  3. Run the resync in the player (#1904).
  4. Nested rows get the pill and bind control (#1905; the UI design is a separate step).

## 1. What could NOT be verified (read first)

1. **Q2, the API.** Unknown: what Figma's plugin `setProperties({Size: "Large"})` does on an instance whose `Size` is bound. Three outcomes are possible: it detaches, it is overruled at once, or it holds until the next mode change. The JS layer passes the call straight to the engine (`39902-…js` @3773186). The engine strings name no rule. Settling it needs a runtime probe (section 3, V-Q2).
2. **What "another setting" is.** Figma's pill can say "Value overruled by another setting". The UI shows that state when the bound value is a valid option but the instance shows a different value for that property (seen, condition @100596). What causes that state is not in the JS. Candidates are an instance-swap property on a parent, a missing variant combination, or an API write. None was confirmed.
3. **Q3 and Q4 are inferred from strings.** A string's presence proves the code is in the binary, not that a given call path reaches it. No anonymous wasm function was traced: no symbol names, and it is out of the 40-minute budget per question.
4. **No live Figma observation.** The brief forbids opening Figma or making a file. No figma.com page was fetched in this round; the public quotes come from the #1824 RE (`re-1824-nivb-evidence/03`).
5. **Studio was read, not run.** No cargo build or test was run for this spec (spec only). The line numbers are for `origin/main@e0e44f3c2`.
6. **Selecting a nested instance directly in Figma.** Whether its own top section shows the rows (`onlyInstances` vs `onlyInstanceSublayers`, `49300` @189737) was not settled. The rows facts in Q5 cover the "Nested instances" blocks of the outer instance's panel.

## 2. Figma facts

### Q1. A hand pick on a property of an instance whose variant properties are bound

Status: **seen** (storage, unbind, pick path, panel). The engine re-applying the binding after a pick is **inferred**.

- F1 (seen). The binding of all variant properties of an instance is one expression on the instance's `VARIANT_PROPERTIES` field. It is the function `RESOLVE_VARIANT`, whose first argument is a map from property name to variable. String variables go in directly. Boolean and number variables are wrapped in a to-string conversion. Evidence: `49300-…js` @98835–@99060, `figma_app-6070…js` @3851645 (module 441252). Engine: "Variant binding does not have a VariableMap first argument." (prototype-lib wasm @27492144).
- F2 (seen). Binding property P writes only P's entry. "Detach property" deletes only P's entry. When the map is empty, the whole binding is cleared. Evidence: @98915. Undo labels: `set-variant-variable-binding` and `clear-variant-variable-binding`.
- F3 (seen). The pick handler takes the instance's current property values, replaces the picked one, finds that variant and swaps the instance to it (`change-variant-prop`). It never reads or writes the binding. Evidence: @97142–@97553.
- F4 (seen). A bound row shows the pill and "Detach property" in place of the dropdown. A bound property therefore cannot be hand-picked in the panel; the only route is to detach that one property, then pick. Evidence: @100596–@102300.
- F5 (inferred, medium). The resolver needs the main the instance shows ("No backing symbol available to Variant Binding function", editor wasm 49989820). So it lays the bound entries over the variant the instance shows, and a pick on an unbound `State` keeps `Size` following. This fits F1–F3. The call path is not traced.
- F6 (seen; the trigger is unknown). Figma has a state "bound, valid, but not shown", with the tooltip "Value overruled by another setting". Evidence: @100596 and `02-ui-strings.txt`.

### Q2. Setting a bound property by value through the plugin or REST API

Status: **unknown** for a plain value. **Seen** for binding through the API.

- Seen: `setProperties` accepts `string | boolean | VariableAlias` for each property. The alias helpers say they are "used with functions such as node.setProperties() to assign component properties to variables". `componentProperties[...].boundVariables.value` is read-only. Source: `typings/plugin-api.d.ts`, GitHub `figma/plugin-typings`.
- Seen: the engine has failure messages for binding a variant property through this path: "Attempting to bind variant property %s failed: variable … does not resolve to a string / not found / is invalid" (50070079, 50205825, 50220987), and "Received invalid value for variant property %s" (49897360).
- Unknown: what a plain string does to an existing entry. Nothing in the typings, the JS or the strings says. REST is read-only for this, so it is not applicable.

### Q3. After a variable-driven variant switch, when ancestors' auto layout is solved

Status: **inferred** (same update).

- Editor and player wasm both hold `autolayout_nodes_dirty_after_layout` (49752276), `max_layout_passes_exceeeded` (50256931, sic) and `nodesThatNeedBottomUpRelayout` (49752203). Variant resolution sits in instance updating: `symbolUpdaterGetModeIDForSet` (49800653), `flushAsyncUpdatersAndUpdateSymbolInstancesDescendedFrom` (50021605).
- Reading: one update applies the instance change, then runs layout passes up to a cap, and counts nodes left dirty as a fault. A hug ancestor that lags one edit would be such a fault. Not traced.

### Q4. Whether the prototype player re-resolves bound variant properties on a mode change or a variable change

Status: **inferred** (yes).

- Seen: the player runs a separate wasm (`prototype-lib/compiled_wasm.wasm`, 29.4 MB). It contains the variant-binding resolver (`RESOLVE_VARIANT` 27451105, "No backing symbol available to Variant Binding function" 27293451, the map-argument checks 27492144), the nested override transfer (`transferNivbOverrides` 27232618, `FGOverrideTransferForNivb.cpp` 27283253, the W01 drop warning 27220541), `setVariableMode` and `runSetVariableMode` (27389659/75), and the mode dependency units (`FGVariableSetDirtyRootBackingSetEffect.cpp` 27271163, `InstanceRematerializer` 27251576).
- Seen (#1824 RE, staff quote): bound nested variants switch on "changes in the actual binding, the variable value, or mode context". That covers both a mode change and a variable value change.
- Reading: a player that never switched bound variants would not need the nested override transfer, so the player switches them. The call path is not traced.

### Q5. The inspector row of a nested instance's bound variant property

Status: **seen** for the rows. The label "design editor" for `67215`'s panel is **inferred** by elimination: in the cached build only two call sites pass `forBubbledProps:!0`, and the other one is the Slides panel (`6db9f521`). `67215` also holds Slides and Make strings, so its name alone does not prove it.

- The "Nested instances" blocks of the outer instance's panel draw each exposed nested instance's variant properties with the same row component as the instance's own rows (`ej` → module 119393; `49300` @74749). `forBubbledProps` changes only the analytics source (`bubbled_sidebar`) and the override path. The design editor panel (`67215-…js` @81057) passes no `hideIcon` to these blocks, so the bind and detach icons show. The Slides panel (`6db9f521-…js` @293131) passes `hideIcon:true` to both top and nested rows, so Slides shows neither.
- The three props that switch the binding control off (`hideBindingsButton`, `keepControlWhenBound`, `variableOnlyBinding`) reach typed-property rows only, not variant rows (@82364, @87998, @88708).
- Row states and strings (from `02-ui-strings.txt`):

| State | Right side | Icon button | Tooltip / label |
|---|---|---|---|
| Not bound | dropdown; a toggle for a two-value boolean-like property | bind icon | "Apply variable" (opens the picker; internal option "Assign variable…") |
| Bound, value is an option and shown | pill with the variable's name and a value thumbnail | detach icon | "Detach property" (aria-label and tooltip) |
| Bound, value not an option | pill marked invalid | detach icon | "Variable value doesn't map to variant" |
| Bound, value valid but the instance shows another | pill marked invalid | detach icon | "Value overruled by another setting" |
| Bound, variable deleted | pill marked deleted | detach icon | — |
| Mixed selection | pill reads "Mixed" | detach icon | — |
| Component playground | — | no bind icon | — |

- Picker filter: a toggle-type property asks for BOOLEAN variables; any other variant property asks for STRING or FLOAT variables. Option matching is case-insensitive in the panel's checks.
- Nested blocks: the header shows the nested instance's layer name only. Hovering highlights its nodes on canvas. Error boxes are hidden (`hideErrors:true`). The pill tooltips above are not error boxes and still apply.

## 3. Test vectors from Figma

Common setup: a set `Button` with `Size` (Small, Large) × `State` (Default, Hover), all four variants. A string variable `size` in collection `Theme`: light → "Small", dark → "Large". Wrapper main `W-main` holds nested instance `N` of `Button` with `Size` bound to `size`. Instance `W` of `W-main` sits in a host frame `H`.

| # | Setup / action | Figma's result | Status |
|---|---|---|---|
| V-Q1a | On W's nested block, pick `State = Hover` (Size bound, H light) | N shows Size=Small, State=Hover; Size's entry stays in the map | seen (F3 + F2) |
| V-Q1b | Then switch H to dark | N shows Size=Large, State=Hover | inferred (F5) |
| V-Q1c | On W's nested block, try to change Size | no dropdown; pill "size" with "Detach property" | seen (F4) |
| V-Q1d | Click "Detach property" on Size | Size's entry removed; the State entry (if bound) stays; the dropdown returns | seen (F2) |
| V-Q2 | Plugin: `N_copy.setProperties({Size: "Large"})` with H light, then switch H dark, then light | unknown. Probe: read `componentProperties.Size.boundVariables` and the shown variant after each step | unknown |
| V-Q3 | H is a hug auto-layout frame holding a top-level `Button` instance bound to `size` (Small 100 wide, Large 200), padding 4: switch H to dark | H is 208 wide after the same update | inferred |
| V-Q4 | Prototype: a button with "Set variable mode Theme → dark" on a frame holding W; present and click | N shows Size=Large | inferred |
| V-Q5 | Select W; look at its "Nested instances" block for N | Size row shows pill "size" + detach icon; State row shows dropdown + "Apply variable" icon | seen |

## 4. Per issue

Studio facts below are read from `origin/main@e0e44f3c2`.

### A hand pick on one nested property stops the other bound properties following (#1901)

**Cause in Studio.**
- One whole-instance flag decides whether the bindings apply: `nested_variant_follows_binding` (`crates/designos-doc/src/component_prop_variables.rs:233`) is false as soon as the copy holds a `MainComponent` override.
- A hand pick records that override: `swap_recorded` (`components.rs:2640`) via `set_nested_instance_prop` (`components.rs:2705`).
- `sync_pair` then skips the bound choices (`components.rs:2214`, `:2229`), and `apply_prop_resync` skips every variant property of the copy (`nested_variant_is_picked`, `component_prop_variables.rs:365`, used at `:410`).

**Specified behaviour (follows F1, F2, F3, F5).** The pick is recorded per property. A bound property follows its variable unless that same property was picked by hand. The variant the copy shows starts from the picked variant (its `MainComponent` override, as now). Every bound, unpicked property is laid over it from its variable.

### A variant picked through the API on a top-level bound instance reverts at the next mode change (#1902)

**Cause in Studio.**
- `set_instance_prop` (`components.rs:2659`) writes the pick into `component.values`, the same slot `apply_prop_resync` writes the variable's value into (`component_prop_variables.rs:430–433`). A pick and a resync cannot be told apart.
- `nested_variant_is_picked` is false for every top instance (`:366`), so the next resync overwrites the pick.
- `swap_instance` (`components.rs:2625`) records `MainComponent`, but a top instance's variant comes from `variant_target(values)` (`components.rs:2225–2228`), which the resync rewrites.
- Pinned today by `a_bound_top_instance_returns_to_its_variable_at_the_next_mode_change_after_a_pick` (`component_nested_variant_binding_tests.rs:659`).

**Specified behaviour.**
- The rule stays the owner's existing ruling: "a variant picked by hand wins over the binding until it is reset". Sources: #1824 issue body; ruling A in `son-1824.md` fix round 1; #1902 Expected, "one rule for both".
- It now holds per property, the same way on top-level and nested instances.
- **Studio-only decision, no Figma evidence (Q2 unknown).** Figma's panel never creates this state (F4). A held pick on a bound property is the "Value overruled by another setting" state (F6), so the pill already has a Figma-shaped way to show it.
- The alternative, "a pick detaches that one property" (Figma's only UI route, F2/F4), would reverse the ruling. Raise it only if the owner wants it (Unresolved 1).

**Mechanism for both issues (one change).**
1. Add the field `ComponentLink::variant_picks: BTreeSet<String>` (`crates/designos-doc/src/node.rs`, next to `own_variant_bindings` at :376).
   - Serde: `default`, `skip_serializing_if = "BTreeSet::is_empty"`.
   - Add it to `ComponentLink::is_empty` (:402).
   - It holds the bound variant properties a person set by hand on this instance (see the Writers filter).
2. Writers. A pick is recorded only for a property that is **bound on that instance when the pick is made**. For an unbound property the shown variant already holds the pick (the `MainComponent` override on a copy, `component.values` on a top instance). Restricting it this way keeps the field off every instance `apply_spec` creates with `"variant"` (`spec_instance.rs:1110`, `:1317`), so no `field:variantPicks` entry goes into `.fig` files and no golden churns.
   - `set_instance_prop` with `Some(PropValue::Variant)` on a bound property inserts the name. With `None` it removes the name.
   - `set_nested_instance_prop` on a bound property inserts the name. It keeps recording `MainComponent` as now. A copy's binding includes the one it inherits from the main (`follow_reference_variant_bindings`), so an inherited binding counts as bound.
   - `swap_instance` to a sibling variant of the same set inserts each **bound** variant property whose value differs between the old and new main. A swap to a main of another set inserts each bound variant property the new main also has (a whole-main choice).
   - **Top instance swap must also write `component.values`.** On a top instance, `swap_instance` writes the new main's value into `component.values` for every variant property the swap changed, bound or not. Without it, the next `sync_one` reads the old stored value through `variant_target` (`components.rs:2225`) and moves the instance back (the resync of a second bound property is such a sync).
   - `bind_prop_variables` (`component_prop_variables.rs:246`) removes the name for every property it binds or unbinds. Binding means "follow from now on", as Figma's "Apply variable" does.
   - Trade-off of the filter, accepted: a copy picked on an unbound `State` that later inherits a new `State` binding from its main follows that binding. Today it stays frozen by the whole-instance override. Figma: unknown.
3. Releasers:
   - `reset_instance` (`components.rs:1089`) clears the set.
   - `reset_overrides` with `MainComponent` (`components.rs:1042`) clears the set.
4. Readers:
   - Replace `nested_variant_is_picked` with "variant property and `variant_picks` contains its name", for top and nested alike.
   - In `sync_pair` (`components.rs:2229`), drop the `nested_variant_follows_binding` gate. Always compute `bound_variant_choices`, minus picked names, and lay them over `desired`. `desired` stays `cur` while `MainComponent` is recorded and the reference's main otherwise.
   - `nested_variant_follows_binding` then has no reader except the import release (step 6). Remove it or rename it to what it checks.
5. Resync scope:
   - `collect_prop_resync_scope` (`component_prop_variables.rs:327`) also adds a node whose `variant_picks` changed. Without this, a reset of a top instance would not re-apply the variable until the next mode change.
   - In `apply_prop_resync`, a top instance's picked property is skipped, so its stored value stays the pick.
6. `.fig` round trip, through the existing carrier:
   - Add `"variantPicks"` to `CARRIED_FIELDS` (`fig_export/carrier.rs:15`).
   - Write it in `fig_export.rs` beside `ownVariantBindings` (:3296) for override entries (nested copies), and in the node's own carrier entries for top instances.
   - Pass it through in `fig.rs:2662`.
   - Restore it in `rest_import.rs` beside :3172.
   - `release_bound_variants_the_file_stored` (`component_prop_variables.rs:465`): skip a node whose picks came from the carrier.
   - For a Figma-authored file (no carrier), keep today's whole-target test. When it does not release, set `variant_picks` = the bound properties whose shown value differs from the variable's value. Bound properties the file shows at the variable's value keep following.

**Acceptance tests (write red first; doc crate unless named).**
1. `a_pick_on_state_leaves_a_bound_size_following_the_mode`: nested copy, Size bound, pick State=Hover, host dark → Size=Large, State=Hover; light → Size=Small, State=Hover. Red today (stays Small).
2. `a_pick_on_the_bound_property_holds_only_that_property`: Size and State both bound to two variables; pick Size=Large by hand; switch both variables' modes → Size stays Large, State follows.
3. `reset_gives_every_picked_property_back_to_its_variable`: after test 2, `reset_overrides(MainComponent)` → both follow.
4. `a_top_level_pick_on_a_bound_property_holds_until_reset`: replaces the assertions of `a_bound_top_instance_returns_to_its_variable_at_the_next_mode_change_after_a_pick`. `set_instance_prop(Size=Large)`, dark, light → Large. `reset_instance` → Small at once, without a mode change.
5. `a_top_level_swap_to_a_sibling_variant_holds_the_properties_it_changed`: Size and State both bound; swap Small→Large; a mode change that flips State keeps Size=Large and follows State (red until the swap also writes `component.values`). `component.values["Size"]` reads Large after the swap.
10. `a_pick_on_an_unbound_property_records_no_variant_picks`: `set_instance_prop` and `set_nested_instance_prop` on an unbound property leave `variant_picks` empty; an `apply_spec` with `"variant"` on an unbound set exports a `.fig` with no `field:variantPicks` entry.
6. `binding_a_picked_property_makes_it_follow_again`: pick, then `bind_prop_variable` on the same property → follows; `variant_picks` no longer holds it.
7. `variant_picks_survive_a_native_save_and_load`, and in designos-fig `variant_picks_survive_a_fig_reopen_on_a_top_instance_and_a_nested_copy`.
8. designos-fig: update `a_pick_on_another_property_than_the_bound_one_survives_a_fig_reopen_and_a_mode_switch` (`fig_nested_bound_variant_picks.rs:117`). After dark the copy must show `Button/Size=Large, State=Hover`. Today the test pins the bug, `Size=Small`.
9. One undo restores variant, picks and stored values for each of tests 1, 2 and 4.

**Files touched.** `designos-doc`: `node.rs`, `components.rs`, `component_prop_variables.rs`, the tests in `component_nested_variant_binding_tests.rs`. `designos-fig`: `fig_export.rs`, `fig_export/carrier.rs`, `fig.rs`, `rest_import.rs`, and the tests.

**Must NOT change.**
- `a_manual_pick_on_the_nested_instance_wins_until_the_override_is_reset` (:547) stays green.
- `a_swap_to_another_set_with_the_same_property_survives_a_fig_reopen_and_a_mode_switch` stays green.
- `a_stored_swap_that_differs_from_the_variable_stays_a_pick_after_a_fig_reopen` stays green.
- R5, "unbinding an inherited binding holds the copy until reset", stays as it is.
- The manual three-level switch keeps today's behaviour (#1909 is separate).
- The variable-driven switch at depths 2–4 keeps its overrides.
- No `OverrideField` variant is added (older builds could not open the file).
- Instances with no bound variant property never carry `variant_picks` (no native-JSON or `.fig` change for them).
- Unbound instances behave exactly as today.

### A hug frame keeps its old size for one edit after a variable-driven switch (#1903)

**Cause in Studio.**
- Editor path: `App::edit_nodes` (`crates/designos-app/src/ops.rs:387`) writes the mode and lays out the edited node and its ancestors inside the transaction (`relayout_around`, :408). Only then does `Tx::commit` → `History::exec` run the resync, inside `apply_with_table_relayout` (`crates/designos-doc/src/history.rs:865–877`). The swapped instance changes size after the layout pass, and nothing in the step solves its ancestors again.
- MCP: `designos-mcp/src/tools/batch.rs:1405–1414` seeds its post-commit relayout from the batch's own ops, which do not include the resync-swapped instances.
- `set_variables` / `SetVariables` steps take the same path.

**Specified behaviour (corroborated by Q3, inferred).** The step that switches a variant also re-solves the auto-layout ancestors of every instance the switch resized. After the step, every hug frame on that chain hugs its content. Studio's own invariant, "a hug frame hugs its content after every committed step", needs no Figma evidence on its own.

**Mechanism.**
- Add one doc-crate helper used by every resync caller: resync, then solve upward from the parent of each instance whose size the resync changed (`Document::relayout_seeds_upward`, `layout_apply.rs:26`).
  - Collect the resized instances from the resync's own inverse: Update commands whose node's size differs now.
  - Callers: `apply_with_table_relayout` and `settle_bound_props_after` (the MCP trial, `component_prop_variables.rs:355`). The MCP trial and the committed document must stay equal, or batch.rs's debug assertion "committed document hash must equal trial document hash" fires.
- Undo:
  - The solve bumps `Document::layout_epoch` (`layout_apply.rs:217`). The editor's `mark_last_undo_as_layout_recorded` (`ops.rs:350`) then declines to mark the step. Undo and redo therefore re-solve from the step's seeds (`apply_and_relayout`, `history.rs:955`). That restores the hug size.
  - Rely on that, and pin it with test 2.
  - Do NOT mark such a step layout-recorded by another route: its stored inverse would not hold the ancestors' sizes.

**Acceptance tests.**
1. doc `a_hug_host_resizes_in_the_same_step_as_a_variable_driven_switch_of_a_top_instance`: H hug, padding 4, top instance bound; `History::exec` the mode Update → H width 208 right after the step (today 108).
2. Same, then `undo` → 108 and Small; `redo` → 208 and Large.
3. Same for a nested copy inside W inside a hug H.
4. designos-app shell test through `set_node_mode` (the editor path that marks steps layout-recorded): 208 after the call; undo → 108.
5. designos-mcp: `set_variables` batch on a hug host → reply and document show 208; no debug hash assertion. The existing test `a_set_variables_batch_that_moves_a_plain_top_level_bound_instance_commits_what_the_trial_showed` stays green.

**Files touched.** `designos-doc/src/history.rs`, `component_prop_variables.rs` (helper, trial caller), tests. Possibly `designos-mcp/src/tools/batch.rs`, only if test 5 is still red after the doc change.

**Must NOT change.**
- Steps with no variant switch keep their layout-recorded marking.
- Viewport-only actions never bump `TileView::rev` (CLAUDE.md §4).
- No relayout of frames off the resized chain (siblings keep positions).
- Table relayout order and behaviour stay as they are.

### A mode change in the prototype player does not re-resolve bound variants (#1904)

**Cause in Studio.**
- `Player::set_variable_mode` (`crates/designos-app/src/proto/player.rs:909`) sets the collection's default mode and clears every node's mode for that collection (:918–920). It then calls `refresh_bound` (:927).
- `refresh_bound` re-applies node-field bindings through `variables::reflow_bound` (`crates/designos-app/src/variables/mod.rs:625`), which knows nothing of component-property bindings. No history step runs, so `apply_prop_resync` never runs.
- `Player::set_variable` (:881) has the same gap for a variable value change.

**Specified behaviour (follows Q4, inferred; and the staff quote "binding, value, or mode context").** In the player, a "Set variable mode" or "Set variable" action switches every instance whose bound variant property now resolves differently: top-level instances and nested copies, through the same switch the editor uses, overrides carried as in the editor. Hug ancestors resize in the same action.

**Mechanism.**
- Add a doc-crate public function that resyncs bound properties for a scope with no undo step. It reuses `apply_prop_resync` plus the #1903 relayout and discards the returned inverse.
- Do NOT reuse `settle_bound_props` (`component_prop_variables.rs:459`). Its import-only release would turn real picks back into followers.
- Call it at the start of `refresh_bound`, before `reflow_bound`, with scope "all bound instances". The player has just cleared per-node modes for the collection, so a subtree scope would miss nodes.

**Acceptance tests (designos-app, `proto/player.rs` tests).**
1. `set_variable_mode_switches_a_bound_top_level_variant`: dark → instance shows Size=Large.
2. `set_variable_mode_switches_a_bound_nested_variant_and_keeps_its_text_override`.
3. `set_variable_switches_a_variant_bound_to_that_variable`.
4. `a_picked_property_stays_held_in_the_player` (after #1901/#1902).
5. The editor document is unchanged after the player runs: `f.doc` equals its copy from before.

**Files touched.** `crates/designos-app/src/proto/player.rs`; `designos-doc` (the new public function, `lib.rs` export).

**Must NOT change.**
- The player never writes the editor document.
- `set_variable_mode_switches_modes_for_every_layer` (player.rs:2123) stays green.
- Unknown modes are still reported, not applied.
- Player frame cost: the resync scans only nodes holding a property binding.

### No pill or bind control on a nested instance's variant rows (#1905), UI facts only

**Studio today.**
- `render_instance_prop_rows` (`crates/designos-app/src/panels/components.rs:332`) sets `bind_kind = None` for nested rows (`nested_scope.is_some()`). There is no `pill_field` (`panels/var_bind.rs:861`) and no `hover_button` (:386) on them.
- A nested copy selected on its own gets no component rows (`panels/components.rs:21`, the `_` branch).
- The data side already exists:
  - binding on a copy is `CopyBinding::Own`;
  - unbinding an inherited binding is `CopyBinding::FrozenUnbound` (`component_prop_variables.rs:246–305`);
  - `prop_binding_problem` gives the invalid reasons.

**Figma UI facts to design against (Q5, seen).**
- Nested rows are the same row as top-level rows. No variant row is "nested-only".
- Unbound: control + bind icon, tooltip "Apply variable".
- Bound: pill (variable name, value thumbnail) in place of the control + detach icon, tooltip and accessible name "Detach property". It removes this one property's binding.
- Invalid tooltips: "Variable value doesn't map to variant"; "Value overruled by another setting". The second maps to Studio's held pick from #1901/#1902: bound, valid, picked.
- Picker types: toggle-like variant property → boolean variables; other variant properties → string or number variables.
- Hidden: the bind/detach icons in the component playground (and in Slides).
- Nested block header: the nested layer's name only. Hover highlights the layer.

**Hand-off for the UI step.** The UI design goes through es-designer and the project design read order. Every write behind the controls exists already: bind → `bind_prop_variables` on the copy; detach → `bind_prop_variables(None)`. Acceptance for the build step: a nested bound row shows a pill with the variable's name and a detach control; detach removes only that property's binding; an unbound nested row offers the bind control; the "overruled" state shows for a held pick. Screenshots are required by the project rules.

## 5. Build order (where files overlap)

1. Per-property picks, #1901 + #1902. One PR: `node.rs`, `components.rs`, `component_prop_variables.rs`, fig carrier files. Everything else builds on `variant_picks`.
2. Layout in the same step, #1903: `history.rs`, `component_prop_variables.rs` (helper). Starts after 1 merges (same file).
3. Player resync, #1904: `proto/player.rs` + the doc function. Uses the helper from 2, so it starts after 2.
4. Nested pill, #1905: `panels/components.rs` (+ `var_bind.rs` if the pill needs the nested key). UI design step first. It needs 1 for the "overruled" state; the rest is independent and can be designed in parallel.

## 6. Side findings (recorded, not specced)

- Figma matches a variable's value to variant options case-insensitively in the panel checks (@100596). Studio's `resolve_for_def` uses `options.contains` (case-sensitive, `component_prop_variables.rs:119`).
- Figma lets boolean and number variables drive a variant property, as text (F1). Studio's `bindable_prop_kind` takes string variables only for variants (:37).

## 7. Epistemic ledger

| Claim | Tier | Evidence |
|---|---|---|
| One binding per instance, a map keyed by property | FACT | `49300` @98835–@99060; module 441252; prototype wasm 27492144 |
| Detach removes one entry; empty map clears the binding | FACT | `49300` @98915 |
| A hand pick never touches the binding | FACT | `49300` @97142–@97553 |
| The panel cannot pick a bound property | FACT | `49300` @100596–@102300 |
| A pick on an unbound property keeps the other bound properties following | INFERENCE (medium-high) | F2 + F3 + "No backing symbol available to Variant Binding function" |
| Nested rows show pill / bind / detach like top rows | FACT | `49300` @74749, @87998; `67215` @81057 |
| A plain-value API write on a bound property | UNKNOWN | JS passes through (`39902` @3773186); no string or doc |
| The trigger of "Value overruled by another setting" | UNKNOWN | only the condition is seen (@100596) |
| Ancestors re-laid out in the same update | INFERENCE (medium) | layout-pass strings |
| The player re-resolves bound variants | INFERENCE (medium) | prototype-lib strings; staff quote |
| Studio causes (all five) | FACT (code read) | file:line above; not run |
| "A pick holds until reset" is an owner ruling | FACT | `son-1824.md` round 1, ruling A; #1824 and #1902 issue text |

Assumptions:

1. **The editor's `set_node_mode` step is the one marked layout-recorded** (ops.rs:350). Confidence high, from a code read. A shell test showing an unmarked step would flip it; the spec holds either way.
2. **A swap to another set should hold every property.** Confidence medium; this is a Studio-only decision. An owner wish for "only the shared properties" would flip it.

## Unresolved

1. **Owner decision (optional).** Keep the ruling "a hand pick on a bound property holds until reset" (specified here). The alternative is Figma's UI route: "a pick on a bound property detaches that one property". With it, the pick cannot be undone by reset, top-level and nested behave the same, and no pick record is needed for bound properties. Recommendation: keep the ruling, because nothing here contradicts it and Figma has a matching "overruled" state.
2. **V-Q2 runtime probe** (about 5 minutes in a scratch file, owner's OK needed): it settles Figma's API behaviour.
3. **Selecting a nested instance alone:** Figma's own-section rows are not settled (section 1, item 6). It affects #1905's second entry point only.
