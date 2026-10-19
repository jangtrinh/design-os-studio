# #162 Cap effect counts in the "+" control without trimming imported effects

| Field | Value |
|---|---|
| Issue | #162 (task E5-2, epic #18; milestone v0.7 Effects & Export; the issue body still says "Pack: v0.6 Effects & Export", a stale label) |
| Size | S |
| Depends on | none |
| Unblocks | none; shares `panels/effects.rs` with #161 (blend modes) and #37 (progressive blur), which add their rows after this task's cap table exists |
| Touches | new `crates/designos-app/src/panels/effect_caps.rs` (cap table, counting, reason text), `crates/designos-app/src/panels/effects.rs` (`header` `:124-155`, `section` `:157-`), `crates/designos-app/src/widgets/menu.rs` (`MenuItem` `:12-22`, `dropdown_checked` `:514-`), new `crates/designos-app/src/shell/tests/effect_caps.rs` |

Code baseline: `origin/main@0e8a9a5` (`docs(specs): v0.6 task specs, part 4 (agent terminal UX and constraint solver) (#697)`). v0.7 opens after v0.6 is tagged. The worker re-anchors every cited line against HEAD before writing code and reports drift first.

## 1. Summary and user story

As a designer, I cannot add a ninth drop shadow to a layer: the row is disabled and a tooltip says why, the same way Figma limits it. As a designer who opens a Figma file that already has more effects than that, I keep every effect: nothing is trimmed, hidden or dropped, and I can still edit and delete them.

## 2. Current state (FACT)

1. The "+" button opens a menu of the seven kinds (`KINDS`, `panels/effects.rs:33-41`) and a disabled "Shader / Soon" row (`:138`). A pick pushes one new effect onto every selected layer (`:141-150`). Nothing counts the effects first.
2. Each effect row has a type dropdown that converts the row in place (`dropdown(... ("fx-kind", i) ...)`, `panels/effects.rs:175-183`; `convert`, `:97`). No count check either.
3. The inspector hands `section` the effects of the first selected layer only (`properties.rs:959`: `&n0.effects`), while the "+" and the type dropdown edit all selected layers (`app.edit_nodes(sel, ...)`).
4. `MenuItem` has `disabled`, `shortcut` and `icon` but no tooltip field (`widgets/menu.rs:12-22`). A disabled row is hover-only (`Sense::hover()`, `menu.rs:394`) and keyboard navigation skips it (`next_enabled`, `menu.rs:210-227`). `dropdown` and `dropdown_checked` build items from label and check mark only (`menu.rs:507-533`).
5. There is no cap anywhere: a search of `designos-doc`, `designos-fig` and `designos-app` for an effect limit, `truncate` or `take(` on effects finds none. The `.fig` and REST importers push every effect they read (search run on this commit; the search found no hit in `designos-fig/src/fig.rs` or `rest_import.rs`).
6. Figma's limit (Figma help page "Apply effects to layers", read 2026-10-19): per layer, 8 drop shadows, 8 inner shadows, 1 layer blur, 2 noise effects, 1 texture, 1 background blur, 1 glass. The page does not say what the UI does at the limit (no mention of a disabled button, a tooltip or an error). **The issue lists four numbers (8/8/1/1); the page lists seven.**
7. Other code that pushes an effect: `preset_popup` (`properties.rs:2250`, the `--panel` screenshot aid) adds one effect to a layer at `:2336` (only when the layer has none) and `:2382`. They are not user paths.
8. The agent path sets at most one shadow through the spec `shadow` key (`designos-doc/src/spec.rs:239`, a replace). The issue keeps limits "in UI only"; this spec keeps that.
9. `Sim` tests drive the inspector by control id and click (`shell/tests/effect_grid_styles.rs:16-41`). UNKNOWN: whether `Sim` can read an egui hover tooltip (a search of `shell/tests.rs` and `shell/tests/*.rs` for `tooltip`, `hover_text` and `on_hover` finds nothing). Check: read the `Sim` helper before writing the tooltip test; if it cannot, test the reason string as a pure function and the disabled state through the menu.

## 3. Behaviour rules

1. **Caps** (one table in `effect_caps.rs`, indexed by `KINDS` order): Inner shadow 8, Drop shadow 8, Layer blur 1, Background blur 1, Noise 2, Texture 1, Glass 1. Source: Figma help page (fact 6). Whether noise, texture and glass are capped is open question 1.
2. **Count** = number of effects of that kind in the layer's `effects` list, hidden ones included (open question 3). A layer linked to an effect style is counted by the values it holds.
3. **The "+" menu.** A kind at or above its cap is a disabled row. For several selected layers, a kind is disabled when any selected layer is at or above its cap, because the pick edits all of them. A disabled pick does nothing and writes nothing.
4. **The type dropdown.** A row may not convert to a kind that is at or above its cap in any selected layer; its own current kind stays enabled and checked.
5. **Reason text.** Hover on a disabled row shows one sentence: "A layer can have up to 8 drop shadows." (cap 1: "A layer can have only one layer blur."). With several selected layers: "A selected layer already has 8 drop shadows." When the count is above the cap: "This layer has 10 drop shadows; the limit is 8." The right-hand column of the disabled row shows "Max 8" (open question 2).
6. **Never trim.** No code path removes, reorders, hides or rewrites an effect because of a cap: not import, open, save, undo, redo, duplicate, paste, an edit of another property, or deleting a different effect. A layer with 12 drop shadows keeps 12. Its rows stay editable and removable. Its "Drop shadow" row in the menu stays disabled until the count is below 8.
7. The cap applies to Studio's own inspector controls only. MCP, spec and agent writes are not capped (the issue's ruling: "enforced in UI only").
8. `Shader` stays a disabled "Soon" row.

## 4. Data model / schema changes

None. `MenuItem` gains `hint: String` (default empty; shown as the hover text of the row, also when the row is disabled).

## 5. Contracts

```rust
pub fn effect_cap(kind: usize) -> usize                       // index into KINDS
pub fn blocked_kinds(layers: &[&[Effect]]) -> [Option<Blocked>; 7]
pub struct Blocked { pub cap: usize, pub count: usize, pub several: bool }
pub fn reason_text(kind: usize, b: &Blocked) -> String
```

Pure functions, no `egui` types. `blocked_kinds` takes the effect list of every selected layer.

## 6. File-format mapping

None. `.fig` and REST keep every effect they hold (rule 6).

## 7. UI

Described in words. Checked against `design/principles.md` and `design/soul.md`; every colour, gap and size is a `crates/designos-app/src/theme.rs` constant; `ui ds context --dir .` is run before the PR.

- The "+" menu keeps its width (168 px, `panels/effects.rs:139`) and row height. A disabled kind uses the existing disabled treatment (`TEXT_DISABLED`, `menu.rs:405`), with "Max N" in the same right-hand column the "Soon" label of the Shader row uses.
- Hover over a disabled row shows the sentence of rule 5 in the standard tooltip. The type dropdown's disabled rows look and behave the same.
- No new colour, no animation, no timer. `design/soul.md:19` bans item counts inside labels such as "Layers (4)". "Max 8" is a limit, not an item count, and sits in its own right-hand column, as "Soon" does; the label text itself never changes.
- Evidence: desktop captures at 1440 for: the menu with every row enabled; the menu with a layer at 8 drop shadows (Drop shadow row disabled, tooltip visible); a layer with 10 imported drop shadows (all rows listed, row still disabled); the type dropdown with a full kind. Responsive: SKIPPED (owner default). Load `es-designer` before UI code. Update the Figma frames from the verified captures first, then run the remaining gates (owner rule).

## 8. Edge cases

1. Selection of several layers where one has 8 shadows and another has 2: Drop shadow disabled with the "A selected layer ..." text; the other kinds work.
2. Hiding an effect does not free a slot (rule 2).
3. A layer with 9 inner shadows and a click on its type dropdown: the row may convert to any kind below its cap, and the 9th row stays an inner shadow until the user changes it.
4. Undo after adding the 8th shadow: the count is 7 on the next frame and the row is enabled again.
5. A style-linked layer: adding leaves the style and copies its values (existing behaviour, `panels/effects.rs:146`); the count uses those values.
6. Progressive blur (#37) is a new kind. UNKNOWN: whether Figma counts it against the one layer blur or separately. Check: owner-manual test in Figma on a layer that has a layer blur; #37's spec records the answer and adds its row to the cap table.
7. A kind added to `KINDS` without a cap entry fails a test (the table length equals `KINDS.len()`).

## 9. Non-goals

- Capping agent, MCP or spec writes, or trimming imported data.
- Blend modes on shadows (#161) and progressive blur (#37).
- Any change to how many effects the renderer draws.

## 10. Test plan (red first)

Seams: pure-function tests for the cap table and reason text; `Sim` tests in `shell/tests/effect_caps.rs` for the menu and the dropdown; a document test and a `.fig` round trip for the never-trim rule.

| Case | Expected | Fails before |
|---|---|---|
| `cap_table_matches_figma_and_covers_every_kind` | seven caps `8,8,1,1,2,1,1` in `KINDS` order; table length equals `KINDS.len()` | yes (module absent) |
| `ninth_drop_shadow_row_is_disabled_and_writes_nothing` | layer with 8 drop shadows: open "+", click Drop shadow: still 8 effects, undo stack unchanged | yes (a 9th is pushed) |
| `reason_text_per_case` | the four sentences of rule 5, with `several` and `count > cap` | yes |
| `any_selected_layer_at_cap_blocks_the_kind` | two layers (8 and 2 shadows): row disabled; Inner shadow works | yes |
| `kind_dropdown_blocks_a_full_target_kind` | row of a layer with 8 inner shadows: converting a drop shadow to inner is disabled; its own kind stays checked | yes |
| `removing_below_the_cap_enables_the_row` | 8 shadows, remove one: row enabled next frame | yes |
| `hidden_effects_count_toward_the_cap` | 8 shadows, 1 hidden: row disabled | yes |
| `imported_effects_over_the_cap_are_kept` | document with 12 drop shadows: unrelated edit, save and reload as `.designos`, and a `.fig` export then import (the existing export and import pair of `designos-fig`) each leave 12 | guard (passes today; if the `.fig` leg fails it is a data-loss bug and is fixed at source in this task) |
| `over_cap_layer_stays_editable` | 12 shadows: change the radius of the 11th, delete the 12th: works, count 11, menu row still disabled | guard |
| `menu_item_hint_defaults_empty_and_existing_menus_unchanged` | all existing `menu.rs` tests stay green | guard |
| `zero_fake_ui_gate_passes` | `python3 scripts/zero_fake_ui_gate.py` | guard |

Run: `cargo test -p designos-app effect_caps`, `cargo test -p designos-app shell::tests::effect_grid_styles`, `cargo test -p designos-fig`, `python3 scripts/zero_fake_ui_gate.py`.

## 11. Performance budgets

- `blocked_kinds` runs once per painted frame while the Effects section is visible: one pass over at most the effects of the selected layers, under 0.05 ms for 12 effects, and no allocation beyond a fixed-size array.
- No extra repaint request, no timer (idle CPU invariant, CLAUDE.md section 4). The inspector must not call `app.changed()` for painting a disabled row.

## 12. Dependencies and build order

No dependency. Order with the other effect tasks: this task first if possible, because #161 and #37 add rows to the same menu and the cap table. If #161 lands first, its PR leaves `KINDS` unchanged (it adds a field to shadows), so the order does not matter for correctness. `panels/effects.rs` is 571 lines: the new logic goes in `effect_caps.rs`, the call sites in `effects.rs`.

## 13. Owner rulings

- Owner ruling 2026-10-19: cap all seven effect kinds with Figma's numbers. A disabled menu row shows a 'Max 8'-style tooltip, hidden effects count toward the cap, and imported effects are never trimmed.

- Limits are enforced in the UI only, and imported data is never dropped (issue text; the delivery workflow's first law: nothing vanishes silently).
- The 9th drop shadow is disabled with a tooltip; an imported file with more effects keeps them all (issue acceptance criteria).
- Desktop capture at 1440 in the PR; responsive checks skipped by default (owner default, 2026-10-10).

## Open questions for the owner (recommended first)

1. **Which kinds are capped.**
   - A (recommended): all seven, using the numbers on Figma's help page (8 drop, 8 inner, 1 layer blur, 1 background blur, 2 noise, 1 texture, 1 glass), because "match Figma" is the pack's headline.
   - B: only the four in the issue (8/8/1/1); noise, texture and glass stay unlimited.
2. **Reason shown without a mouse.** Recommended: the disabled row also shows "Max 8" in its right-hand column, because the keyboard skips disabled rows and cannot reach a hover tooltip. Alternative: tooltip only.
3. **Hidden effects.** Recommended: they count toward the cap (the row exists in the list). This is an ASSUMPTION about Figma; the check is an owner-manual test in Figma (hide the eighth drop shadow, then try to add one).
