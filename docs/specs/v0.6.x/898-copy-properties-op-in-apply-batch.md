# #898 Copy and paste properties between layers from MCP (copy_properties op in designos_apply_batch)

| Field | Value |
|---|---|
| Issue | #898 (enhancement; from the Figma plugin study, owner pack ruling 2026-10-22: next-patch work) |
| Size | M, about two days (issue: S). Acceptance criteria 1 and 2 force one shared module and a visible change to the editor's Paste properties; see section 1. |
| Depends on | none |
| Unblocks | an agent "make B look like A" in one undo step, with `@alias` refs to layers made earlier in the same batch |
| Touches | new `crates/designos-doc/src/copy_properties.rs` (+ tests file) and `lib.rs` re-export; `crates/designos-app/src/ops.rs` (`:821-872`), `app.rs` (`CopiedStyle` `:128-138`, field `:171`, init `:261`); `crates/designos-render/src/text.rs` (new `fit_text_size` beside `text_geometry` `:761`) and `designos-app/src/layout.rs:19-25` (delegates); `crates/designos-mcp/src/tools/batch.rs` (schema `:41-89`, description `:137`, parse `:583-811`, exec `:1813` neighbourhood, reply `:1939-1950`); generated: `tests/snapshots/tools.json`, `generated/rules.md` (no change expected), README/CLAUDE/SKILL tool blocks, `docs/mcp-parity.md` |

Code baseline: `origin/main@42a0d027`. Re-anchor every cited line against HEAD and report drift before writing code. Evidence: Figma plugin `jangtrinh/design-os-figma-plugin` @ `7c1ef8bb`, `cli/src/commands/clone-traits.ts` and `plugin/src/main/executor-clone-traits.ts`. No plugin code is copied; the Figma Plugin API is not used.

## 1. Summary and user story

As an agent, I say `{"op":"copy_properties","source":A,"targets":[B,C],"traits":["fills","radius"]}` and B and C take A's look in the same undo step as the rest of my batch, keeping variable and style bindings. The issue asks for the editor's Paste properties logic to be shared. Reading it shows that logic drops bindings (section 2), so sharing it as is would fail acceptance criterion 2. The shared module therefore carries bindings, and the editor's Paste properties changes with it (Open question 1).

## 2. Current state (FACT unless labelled)

- Plugin `clone-traits --source --target --traits` (`clone-traits.ts:4-10`; `command-catalog.ts:213-216`). Groups `layout, fills-variables, typography, spacing, text` (`executor-clone-traits.ts:3-6`). An unknown trait is `E_INVALID_ARGS` (`:14-17`); `text` must be asked for explicitly (`:18-20`). `typography` and `text` throw unless source and target are both TEXT (`:56-58`, `:97-99`). A field absent on either node is skipped silently (`:40-41`); `skipped` lists only fields whose assignment threw (`:42-47`). Reply `{sourceId,targetId,traits,applied,skipped}` (`:105`). The groups copy raw fields (`:83-94`); the plugin reports no bindings.
- Studio editor `copy_properties` / `paste_properties` (`ops.rs:821-872`): `CopiedStyle` (`app.rs:128-138`) holds `fills, stroke, effects, opacity, blend_mode, corner_radii, corner_smoothing, text_style`. Paste writes those to every selected node in one `Tx` (`:841-872`); for text it applies the whole style to the base and every run with `richtext::apply_all` (`:859-860`) and re-fits the box with `layout::fit_text` (`:862-867`). Entry points: `panels/menus.rs:199-200`, `panels/context.rs:43-44`, `shell.rs:692-693`. Tests: `ops.rs:2416-2453`, `shell/tests.rs:1127-1131`.
- Paste never touches `style_refs` or `variables.bindings` (`ops.rs:823-872`). A node keeps style links in `Node.style_refs` (slots `fill, stroke, text, effect, grid`, `node.rs:108-110`) and variable bindings in `Node.variables.bindings`, key to variable id (`node.rs:126-128`, `variables.rs:163-168`; keys `variables_apply.rs:21-44`). `resolve_node` applies a node's own bindings over its stored values at read time (`variables_apply.rs:540-572`). INFERENCE: pasting a raw fill onto a node whose `fills/0` is bound leaves the old variable on top, so the paste looks like a no-op; closing check: the red test `paste_properties_replaces_the_targets_bound_fill`.
- Text colour lives in `TextStyle.fills` (`paint.rs:464`); the editor copies the node-level `n.fills` (`ops.rs:827`), unused for text. INFERENCE: pasting from a text layer onto a rectangle clears the rectangle's fill; closing check: the red test `text_fill_moves_to_a_rectangle_and_back`.
- Existing group copies are private to `components.rs`: `copy_style_ref_key` (`:849-858`), `binding_override_field` (`:860-883`, no `effects/` arm, so `effects/N/...` bindings are never moved), `copy_variable_bindings_for_field` (`:885-893`), `copy_field` (`:895-957`). `copy_field(TextStyle)` copies the source's `runs` (`:932-941`), which are byte ranges of the source text, so it must not be reused for typography.
- MCP batch: one trial clone (`batch.rs:1437`), ops run in order, commands collected and committed as one `Command::Batch` (`:1961-1962`), so one undo step. Flat op schema with `additionalProperties:false` (`:82-89`), `op` enum (`:41-45`), node refs `id | $prev | @alias[/path]` (`:30-36`). `set_in_progress` is the multi-target model: `nodes`/`node` (`:397-420`), at most 200 (`:760`), alias and `$prev` bind the first target (`:1813-1891`). Lease guard `check_lease_guard` (`:937-956`). Unsupported-op error lists valid ops (`:772-781`; it omits `create` and `set_in_progress` today). Reply `{dry_run, created, created_count, name_to_id, changed, touched_roots, touched_bounds, undo_label}` (`:1939-1950`); `changed` is `ChangedCounts` (`:465-473`).
- MCP edits to a layer inside an instance do not record overrides (no `record_override` in `designos-mcp`); `lint_fix` does it by hand (`lint/fix.rs:245-250`); the editor does it in `Tx::commit` (`edit.rs:162`, `comp_ops.rs:60`). INFERENCE: without it the next component sync reverts the change.
- MCP never re-fits text boxes: a content update replaces `content` only (`tools/mod.rs:258-266`) and the document layout solver has no text measure (no `auto_resize` in `layout_solver*.rs`). The fit rule lives in the app (`layout.rs:19-25`, `textmeasure.rs:16-31`) over `designos_render::text_geometry` (`text.rs:761`); `designos-mcp` depends on `designos-render` (`Cargo.toml`).
- Rules pack: the `designos_apply_batch` purpose is cut at 106 chars inside the op list (`generated/rules.md:187-188`, `rules.rs:475-492`) and the pack is 23,968 of 24,000 chars. Appending an op name after `set_in_progress` in `batch.rs:137` changes nothing in the pack.
- UNKNOWN: whether any lint rule or export reads `style_refs`/`bindings` in a way a moved binding could break. Closing check: `cargo test --workspace --lib` after the change.

## 3. Behaviour rules

1. **Op.** `copy_properties` in `designos_apply_batch`: `source` (node ref), `targets` (1..=200 node refs, `nodes`/`node_ids` not accepted), `traits` (optional), `as`. An op, not a new tool, so it joins the batch's single undo step, can name layers created earlier (`@card`), and costs the rules pack nothing. `as` and `$prev` refer to the first target.
2. **Traits** (8 names; the issue lists 7, `appearance` is added for editor parity, Open question 2). Omitted `traits` means the editor's set `fills, strokes, effects, appearance, radius, typography`; `layout` and `spacing` are opt-in; `[]` is invalid.

| Trait | Property fields | Style link | Variable bindings that travel |
|---|---|---|---|
| `fills` | node fills; for text the base and every run's `style.fills` | `fill` | `fills/*` |
| `strokes` | `stroke` (paints, weights, align, cap, join, dashes) | `stroke` | `strokes/*`, `strokeWeight`, `stroke{Top,Right,Bottom,Left}Weight` |
| `effects` | `effects` | `effect` | `effects/*` |
| `appearance` | `opacity`, `blend` | none | `opacity` |
| `radius` | `corner_radii`, `corner_smoothing` | none | `topLeftRadius`, `topRightRadius`, `bottomRightRadius`, `bottomLeftRadius`, `cornerRadius` |
| `typography` | `TextStyle` minus `fills` (family, weight, italic, size, line height, letter and paragraph spacing, align, case, decoration, features, axes) | `text` | `fontSize`, `fontFamily`, `fontStyle`, `fontWeight`, `lineHeight`, `letterSpacing`, `paragraphSpacing` |
| `layout` | frame `clips_content`, `auto_layout` without spacing, `grids`; `layout_child`; `constraints` | `grid` | none |
| `spacing` | `item_spacing`, `counter_spacing`, `padding` of an auto layout | none | `itemSpacing`, `counterAxisSpacing`, `padding{Left,Right,Top,Bottom}` |

3. **Match, not merge.** For each applied trait the target ends with the source's value, including "none": a source without effects clears the target's effects and its `effects/*` bindings and `effect` link. Bindings and the style link of a trait that was not asked for are left alone.
4. **Bindings stay bound.** For each applied trait the target's bindings and link of that trait are replaced by the source's. A binding is carried only if `scope_allows(variable.scopes, key, target.kind)` (`variables_apply.rs:48-86`) holds; otherwise its raw value still copies and the skip list names the key (`binding_out_of_scope`). Modes need no handling: bindings resolve in the target's own mode at read time (`variables_apply.rs:540-572`).
5. **Snapshot at run time.** The op snapshots the source from the trial document when it runs, so it sees earlier ops of the same batch.
6. **Typography** (no `richtext.rs` move). The target's base style takes the source's typography fields; every run takes them too but keeps its own `fills`; runs equal to the base are dropped and equal neighbours merged. For the editor's default set this should equal today's `apply_all` result (base := source style, runs cleared); INFERENCE from reading `richtext.rs:63-84` and `:128-144`, pinned by a test that uses `richtext::apply_all` as the oracle.
7. **Incompatible pairs are skipped and reported, never silent.** Per target and trait: `source_unsupported`, `target_unsupported` (typography needs `any_text()` on both; radius is unsupported on Text, Line, Group, Vector (the inspector's list, `panels/properties.rs:908`) and Page; layout and spacing need frame-like kinds, `layout_child`/`constraints` any non-page node), `no_auto_layout_source` and `no_auto_layout_target` (spacing only), `same_node`, `binding_out_of_scope`. `layout` never changes an existing target's spacing; a target without auto layout takes the source's whole auto layout. Canonical order is trait order of the table, so `layout` runs before `spacing`.
8. **Nothing applies at all** (every trait skipped on every target) fails the batch: `BATCH_OP_FAILED` with cause `OVERRIDE_FIELD_NOT_APPLICABLE` at `/ops/<i>/traits`, `valid_options` = traits the source can supply. An unknown trait name is `INVALID_ARGUMENTS` at `/ops/<i>/traits/<j>` with `valid_options` and a corrected example.
9. **Text boxes.** The shared function reports `refit_text`; the caller re-fits. Editor: `layout::fit_text` as today. MCP: new `designos_render::fit_text_size(props, size)` (the app's `fit_text` rule moved next to `text_geometry`; `layout::fit_text` delegates), then `relayout_for_node` (`batch.rs:993`).
10. **Guards and instances.** Each target passes `check_lease_guard`; the source is read-only. A target with `component.source.is_some()` gets `record_override(before, after)` added to its overrides, as `lint/fix.rs:245-250` does.
11. **Commit.** Each changed target becomes one `Command::Update`, live-patched through `live_ops_for_command` like `update`; `changed.updated` counts changed targets; `dry_run` works as for every op.
12. **Reply.** New optional `copied` array, one entry per op: `{op, source, traits, targets:[{node_id, applied, changed, skipped:[{trait,reason,key?,kind?}], variables, styles}]}`. `changed` lists the names among `fills, stroke, effects, opacity, blend, corner_radii, corner_smoothing, text_style, clips_content, auto_layout, grids, layout_child, constraints, item_spacing, counter_spacing, padding, variable_bindings, style_refs` whose value differs after the copy; `variables` and `styles` list the binding keys and style slots now on the target for the applied traits.
13. **Editor.** `copied_style` becomes `Option<PropertiesSnapshot>` and Paste properties is `apply` with the default set, then its existing `Tx`, toast and text re-fit. Toasts do not change.

## 4. Data model / schema changes

None to the document or its files. New in `designos-doc`: `CopyTrait` (`ALL`, `DEFAULT`, `name`, `from_name`), `PropertiesSnapshot` (small, no geometry), `CopyReport`, `CopySkip`, `SkipReason`. `CopiedStyle` is removed.

## 5. Contracts

- `pub fn PropertiesSnapshot::of(source: &Node) -> Self`; `pub fn PropertiesSnapshot::can_supply(&self, t: CopyTrait) -> bool`; `pub fn PropertiesSnapshot::apply(&self, target: &mut Node, traits: &[CopyTrait], binding_in_scope: &dyn Fn(&str, &str, &NodeKind) -> bool) -> CopyReport` (the closure takes variable id, binding key, target kind, so the editor can pass one built before it borrows the document mutably). `CopyReport { applied, changed, skipped, variables, styles, refit_text }`. The file stays under 200 lines; tests in `copy_properties_tests.rs`.
- MCP op fields added to `op_properties`: `source` (node ref), `targets` (array of node refs, 1..=200), `traits` (array of the 8 names, unique); `op` enum and the description gain `copy_properties` appended after `set_in_progress`. Success schema gains optional `copied`.
- Example: `{"ops":[{"op":"copy_properties","source":12,"targets":["@card","$prev"],"traits":["fills","radius","typography"]}]}` replies `"copied":[{"op":0,"source":12,"traits":["fills","radius","typography"],"targets":[{"node_id":31,"applied":["fills","radius"],"changed":["fills"],"skipped":[{"trait":"typography","reason":"target_unsupported","kind":"RECTANGLE"}],"variables":["fills/0"],"styles":["fill"]}]}]`.
- No new error code. Codes used: `INVALID_ARGUMENTS`, `MISSING_FIELD`, `UNKNOWN_REF`, `NODE_NOT_FOUND`, `OVERRIDE_FIELD_NOT_APPLICABLE`, `OUTSIDE_OWNED_FRAMES`, `VALUE_OUT_OF_RANGE`, `BATCH_OP_FAILED`. Add `copy_properties` to the valid-ops list at `batch.rs:780`.
- Generated output: `cargo run -p designos-mcp --bin gen-tool-docs`; commit what it changes.

## 6. File-format mapping

None. `style_refs` and `variables.bindings` already round-trip (`boundVariables` keys are the binding keys, `variables_apply.rs:1-8`).

## 7. UI

No new UI and no copy change; Cmd+Alt+C, Cmd+Alt+V, the context menu items and their toasts stay. What a user can see change is Paste properties itself: it now moves bindings and links (a bound fill pasted over a bound fill takes the new variable), text colour follows the paste rules of section 3, and a source without radius no longer zeroes a target's radius. `es-designer` is not required (no panel or token change); if a panel file is touched, load it and follow CLAUDE.md section 5.

## 8. Edge cases

- Source or target missing: `NODE_NOT_FOUND` at `/ops/<i>/source` or `/targets/<j>`. Duplicate targets collapse. The source among the targets: skip `same_node`.
- A target inside an instance: overrides recorded (rule 10). A target in a component main: it changes the main and the instances follow at the next sync (INFERENCE from `sync_instances`, `components.rs`).
- A variable deleted after the editor's copy: the stale binding resolves to nothing and the raw value shows (`resolve_node` skips unresolved ids); lint reports a broken style link. No extra handling.
- Text with runs: runs keep their own fills under `typography` (rule 6); under `fills` every run takes the source colour.
- `layout` onto a frame with children when the source has no auto layout: auto layout is removed and children keep their solved positions.
- `$prev` after the op is the first target; with `as`, `@alias` is the first target.
- Replays: with an `idempotency_key` the stored reply replays, `copied` included.
- 200 targets with large text: the text re-fit runs once per text target (budget in section 11).

## 9. Non-goals

- Characters (`text` trait of the plugin), name, visibility, lock, size, position, transform, mask, export settings, prototype data, component properties, variants and overrides, variable modes.
- Copy between documents or from a library; a standalone `designos_copy_properties` tool; the system clipboard.
- Creating styles or variables; changing a style's own values.
- Fixing `binding_override_field` (no `effects/` arm) for instance sync: a follow-up, because it changes component sync (this spec does not call it).
- Recording overrides for the existing `update` op.

## 10. Test plan (red first)

Seams: pure tests in `designos-doc` (`copy_properties_tests.rs`), editor tests in `ops.rs`, op tests in `batch.rs` `mod tests` (model `batch_is_one_undo_step` `:2286`). All "fails before" are yes unless marked: no module, op or behaviour exists.

| Case | Expected | Fails before |
|---|---|---|
| `default_traits_equal_the_old_paste_for_unbound_nodes` | fills, stroke, effects, opacity, blend, radii, smoothing, text style as `ops.rs:841-872` wrote them | yes |
| `typography_matches_richtext_apply_all` (app; oracle = old code path) | text with runs: base := source style, runs cleared | yes |
| `paste_properties_replaces_the_targets_bound_fill` | bound `fills/0` becomes the source's binding, or none | yes (closes the INFERENCE) |
| `bound_variables_and_linked_styles_stay_bound` | every table binding key and every `fill/stroke/effect/text/grid` link arrives on the target | yes |
| `effect_bindings_move_with_effects` | `effects/0/color` moves (the gap in `binding_override_field`) | yes |
| `none_is_copied_too` | source without stroke clears target stroke, `strokes/*` and `stroke` link | yes |
| `unrequested_traits_keep_their_bindings` | `traits:["radius"]` leaves fills and bindings untouched | yes |
| `text_fill_moves_to_a_rectangle_and_back` | `style.fills` is read and written; a rectangle fill is not wiped | yes |
| `radius_skips_text_line_group_vector_and_page` | skip reasons, targets unchanged | yes |
| `spacing_needs_auto_layout_on_both_and_layout_keeps_spacing` | skip reasons; `layout` keeps target padding; order layout then spacing | yes |
| `binding_out_of_scope_is_dropped_and_named` | raw value copied, key in `skipped` | yes |
| `paste_properties_is_one_undo_step_and_refits_text` (app) | one undo restores; `fit_text_size` result applied | guard |
| `paste_properties_equals_the_mcp_op` (app) | same pair through `copy_properties()`/`paste_properties()` and through `PropertiesSnapshot::apply` gives equal nodes | yes |
| `copy_properties_op_is_one_undo_step_with_the_batch` | one `undo` reverts all ops; `changed.updated` counts targets | yes |
| `copy_properties_default_traits_equal_paste_properties` | op without `traits` equals the editor result on the same fixture | yes |
| `copy_properties_reply_lists_changed_and_skipped_per_target` | `copied` as in section 5 | yes |
| `copy_properties_nothing_applies_is_a_named_error` | `BATCH_OP_FAILED`, cause `OVERRIDE_FIELD_NOT_APPLICABLE`, `valid_options`, document identical | yes |
| `copy_properties_unknown_trait_lists_valid_options` | `INVALID_ARGUMENTS` `/ops/0/traits/1`, 8 options | yes |
| `copy_properties_accepts_alias_prev_and_refs_to_new_layers` | `@card` and `$prev` resolve | yes |
| `copy_properties_records_overrides_on_instance_sublayers` | a sync after the op keeps the change | yes |
| `copy_properties_respects_owned_frames` | `OUTSIDE_OWNED_FRAMES` for a target outside the lease | yes |
| `copy_properties_dry_run_changes_nothing` | no commit, reply present | yes |
| `rules_under_token_budget`, `tool_list_matches_snapshot` | green; pack unchanged in size | guard |

Run: `cargo test -p designos-doc copy_properties`, `cargo test -p designos-render fit_text`, `cargo test -p designos-app ops::`, `cargo test -p designos-app --lib shell`, `cargo test -p designos-mcp copy_properties`, `cargo test -p designos-mcp rules::`, `cargo test -p designos-mcp --test tools_snapshot`, `cargo run -p designos-mcp --bin gen-tool-docs`, `cargo test --workspace --lib`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`. Evidence (PR): a terminal capture of the batch call and its `copied` reply; before and after screenshots of a bound fill pasted in Studio (`designos --screenshot`) and of the MCP result rendered with `designos_render_png`.

## 11. Performance budgets

- Per target: a clone of the fills, effects and text runs of the snapshot; no geometry. 200 targets of plain nodes under 50 ms (ASSUMPTION; measure). The batch already clones the whole document once (`batch.rs:1437`); the op adds no clone of it.
- Text re-fit is shaping: one `fit_text_size` per text target, budget 2 ms each (ASSUMPTION; measure on a 200-target batch of two-line labels).
- Editor: Paste properties stays one `Tx` and one `run_tx`; it is not a viewport operation and never runs per frame. No `doc.to_json()`; the snapshot is a field clone. On the "VSF - PCP" benchmark (112,474 nodes) paste onto 100 selected layers must not stall a frame (CLAUDE.md section 4); record the frame time.

## 12. Dependencies and build order

Independent of #896 and #897. Order: `copy_properties.rs` with its tests; `fit_text_size` and the `layout::fit_text` delegate; the editor switch and its tests; the batch op and its tests; `gen-tool-docs`; check the pack is still 23,968 chars (the op name goes after `set_in_progress` in the description, past the 106-char cut); evidence; PR (squash).

## 13. Owner rulings that apply

- 2026-10-22 (pack): "Small items (#896, #897, #898) are next-patch work."
- Issue #898, acceptance criterion 1: the traits copied match the editor's Paste properties for the same pair (shared code, one test for both).
- Standing: new MCP params go through the generated docs and the drift test; Studio is "source-available", not "open source".
- 2026-09-22: "Làm cái gì trên toàn bộ project cũng phải chụp hình chứng minh" (visual evidence in every PR).

## Decisions on the open questions (controller, 2026-10-22)

The owner delegated these calls ("Orchestrate it yourself and answer it yourself", relayed by the release controller on 2026-10-22). Each question below was settled with its recommended answer, so they are binding for the implementer. The reasons are kept so the owner can reverse any of them.

1. **Does the editor's Paste properties carry bindings and links from now on?** Recommended: yes. Criterion 1 (shared code) plus criterion 2 (bindings stay bound) cannot both hold otherwise, and today a bound target ignores a paste. Alternative: MCP-only carrying, which breaks criterion 1.
2. **Add `appearance` (opacity and blend) as an eighth trait?** Recommended: yes; the editor's paste copies both today and the issue's seven names do not cover them. Alternative: fold them into `effects`, which misnames them.
3. **`traits` omitted means the editor's set?** Recommended: yes (agents get Cmd+Alt+V by default; `layout`/`spacing` stay opt-in). Alternative: require `traits` as the plugin does.
4. **Move the text-fit rule into `designos-render` so MCP re-fits text?** Recommended: yes (about 20 lines, one rule for both callers). Alternative: MCP skips the fit and a text box can come out the wrong size.
5. **Out-of-scope variable bindings: drop and report, or fail the op?** Recommended: drop and report, so a mixed target list still works.
