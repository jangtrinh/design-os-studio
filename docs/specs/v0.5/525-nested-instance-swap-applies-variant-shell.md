# #525 Button variant not applied: link and secondary buttons draw as filled primary pills

- Issue: https://github.com/jangtrinh/design-os-studio/issues/525 (bug, Untitled UI kit render audit 2026-10-18)
- Spec base: `main` at `767acf9`.
- Open PR: #540 (`task/525-nested-instance-swap-shell`, +229 −5).
  - Root cause: `swap_nested` retargets the component and rebuilds the children, but keeps the old variant's shell (fills, stroke, effects, radii, auto-layout props) on the instance node, and does not re-apply its props.
  - The release review, head `37d6d5a`, found one MAJOR: the kit detector does not find every affected instance. It hydrates one page, checks only `Hierarchy=Link`, and returns green when the file is absent.
  - The owner ruled on 2026-10-18 that the release session fixes this PR.
- Hot file: `crates/designos-fig/src/rest_import.rs`; tests in `tests/fig_roundtrip.rs` and `tests/real_files.rs`.

## 1. Summary and user story

As a designer importing a kit, a nested button swapped to the "Link" or "Secondary" variant (by instance swap or a variant property) must look like that variant, with no leftover purple pill from the default "Primary" variant.

## 2. Current state

- FACT: `swap_nested(d, target, …)` (`rest_import.rs:1792-1842`) returns early when `current == target` or the target is not a Component (`:1797-1799`), expands the target (`expand_nested`), and retargets `d`'s `component`. Per the PR #540 root cause, it does not copy the target's own node properties onto `d`, and does not call `apply_props` for `d`'s values on the new children.
- FACT: on `main`, the component pointer is replaced at `rest_import.rs:1804-1806`, which also clears `component_key`.
- FACT: `component_props` reads `symbolIdValue` (`fig.rs:1635`). `OVERRIDDEN_SYMBOL_ID` maps to `mainComponent` (`fig.rs:1557`, `:1687`).

## 3. Behaviour rules

1. **Shell copy on swap.** When `swap_nested` retargets instance `d` from component `A` to `B`, it copies from `B` onto `d` every field that `d` does not override (`d.component.overrides`):
   - `FrameProps` (clip, auto layout, grids);
   - `size`, unless `d` overrides Size or derived data sets it later;
   - `fills`, `stroke`, `effects`, `opacity`, `blend`, `is_mask`, `corner_radii`, `corner_smoothing`;
   - `fill_geometry` and `stroke_geometry`;
   - `style_refs`, `variables.bindings`, `component.variant_props`.
2. **Then rebuild children** from `B`, as today.
3. **Re-apply instance props.** After the rebuild, apply `d.component.values` to the new subtree (`apply_props`), so assignments such as `Icon leading = false` or label text survive.
4. **Then the same-entry symbol overrides** on `d` apply (they win), then derived data. This is the existing order in `apply_override` (per the review: "Same-entry symbol overrides still win").
5. **Fields overridden on `d`** in the file keep `d`'s values.

## 4. Data model / schema changes

None.

## 5. Contracts

None public.

## 6. File-format mapping (RESOLVED)

- `NodeChange.symbolData = 113`, with `symbolID=1` and `symbolOverrides=2`; `overriddenSymbolID` is the swap target in an override entry.
- `componentPropAssignments = 268`, with `symbolIdValue` (`VariableAnyValue.symbolIdValue = 8`) for INSTANCE_SWAP values.
- `ComponentPropNodeField.OVERRIDDEN_SYMBOL_ID = 2`.

## 7. UI

None. Evidence: Studio versus Figma renders of `1685:415210` (the "View integration" link buttons) and the other frames listed in the issue.

## 8. Edge cases

1. Default Primary (purple fill, white text) swapped to Link (no fill, purple text, hidden icon): no fill, no stroke, the icon is hidden by the prop value, and the text is "View integration".
2. Swapped to Secondary (white fill, grey 1 px stroke, shadow): the fill, stroke and effect equal Secondary's.
3. `d` overrides its fill in the file: `d` keeps the override fill after the swap.
4. A swap to the same component: no-op (the early return is kept).
5. A swap target that is not a Component (missing or library-absent): no-op, and the instance keeps the old shell. Counted as an unresolved swap in `ImportNotes` (from #111) as `unresolved_swaps`.
6. A nested swap inside a swapped instance: each level applies rules 1–4 in pre-order.

## 9. Non-goals

- Variant switching in the editor (already handled by `sync_pair` → `swap_main`).
- Library-only targets that are not in the file.

## 10. Test plan

Seams: `tests/fig_roundtrip.rs` and `tests/real_files.rs`.

- `nested_instance_swap_updates_shell_fills_strokes_effects_and_reapplies_props` (PR #540): synthetic Section footer → Button (Primary: fill `#7F56D9`), swapped to Link (no fill), with `Icon leading=false` and text `View integration`. Expect an empty fill, the icon hidden and the text kept.
  - Fails before: yes.
- `swap_keeps_instance_level_fill_override`: edge case 3.
- `swap_to_secondary_copies_stroke_and_effect`: edge case 2.
- `untitled_ui_swapped_link_and_secondary_buttons_match_variant_shell` (ignored, kit). Fixes the review MAJOR:
  - hydrate **every** page (`Document::hydrate_all_pages`, `designos-doc/src/lib.rs:326`);
  - for every instance whose component's `variant_props["Hierarchy"]` is `Link` or `Secondary`, assert that its fill, stroke and effects equal its component's, unless the field is in `component.overrides`;
  - print the count checked and the count failing;
  - keep the explicit check of `1685:415210` "View integration".
  - Skipping is only allowed when `DESIGNOS_REFERENCE_DIR` is unset, with a printed reason. With the variable set and the file missing, the test fails.

## 11. Performance budgets

No `CLAUDE.md` §4 invariant applies. The shell copy is O(1) per swap. `hydrate_all_pages` runs only in the ignored kit test.

## 12. Dependencies and build order

- No issue dependency.
- Hot file: after #518 and #522.

## 13. Owner rulings that apply

- Hot-file rule (`docs/releasing.md`).
- Owner ruling of 2026-10-18: the release session fixes PR #540.
