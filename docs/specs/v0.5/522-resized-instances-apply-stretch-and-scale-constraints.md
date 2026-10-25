# #522 Resized instances keep the main component's size for Stretch/Scale children (pie charts draw small and off-centre)

- Issue: https://github.com/jangtrinh/design-os-studio/issues/522 (bug, Untitled UI kit render audit 2026-10-18)
- Spec base: `main` at `767acf9`.
- Open PR: #538 (`task/522-resized-instance-constraints`, +210 −1). It applies constraints in `expand_instance` when the instance size differs from the component's, keeps rotated child centres, and ignores sentinel `_overrideKey`. No review yet at spec time.
- Hot file: `crates/designos-fig/src/rest_import.rs` (serial queue).

## 1. Summary and user story

As a designer importing a kit, a component instance I resized in Figma must lay out its children the way Figma does. Today, children with Stretch or Scale constraints stay at the main component's size unless Figma's derived data happens to list them. 640 children in 159 frames on 15 pages are affected; the pie charts are the visible case.

## 2. Current state

- FACT: `expand_instance` (`rest_import.rs:1843-1888`) copies the component's children verbatim. It never compares the instance size with the component size.
- FACT: resized-instance child geometry comes only from `derivedSymbolData`, through `apply_overrides` (`rest_import.rs:1904-1925`) and `apply_fast_derived` (`:1927`). Children without a derived entry keep the component geometry.
- FACT: the editor's resize rule is `apply_constraints` (`crates/designos-app/src/layout.rs:167-219`):
  - Min: unchanged;
  - Max: `x += dw`;
  - Center: `x += dw/2`;
  - Stretch: `w += dw`;
  - Scale: `x *= rx; w *= rx`.

  It skips rotated children (`transform.b` or `transform.c` ≠ 0, `layout.rs:179-181`), skips auto-layout frames and groups (`:169-171`), and recurses into resized children (`:213-215`).
- FACT: `Constraint` is `{Min, Max, Center, Stretch, Scale}` (`designos-doc/src/node.rs:990-997`).
- FACT from the issue: in pie chart `1084:114`, the "Background" arc follows the instance and the six "Series" arcs stay at 160×160.

## 3. Behaviour rules

1. **When.** After `expand_instance` copies the children of component `C` into instance `I`, and before overrides and derived data apply: if `|I.size − C.size| > 1e-4` on either axis, and `I` is not auto-layout, run the constraint pass with `old = C.size` and `new = I.size`.
2. **Constraint pass** (shared math). Move the editor's per-child computation into `designos-doc`, as `pub fn constrain_child(child: &Node, old: Vec2, new: Vec2) -> (Affine, Vec2)`. Let `dw = new.x − old.x`, `dh = new.y − old.y`, `rx = new.x / old.x` (1 if `old.x` is 0) and `ry` likewise.
   - Horizontal: Min keeps `x, w`; Max `x += dw`; Center `x += dw / 2`; Stretch `w += dw`; Scale `x *= rx`, `w *= rx`.
   - Vertical: the same with `y`, `h`, `dh`, `ry`.
   - `w = max(w, 1)` and `h = max(h, 1)`.
   - Both `apply_constraints` (app) and the importer call it. This fixes at the shared layer (delivery law 4).
3. **Rotated children.** Apply the rule to the child's centre and size in the parent's axes:
   - Position: the centre `c = transform · (w/2, h/2)` moves by the horizontal and vertical rules applied to the centre. Min is unchanged; Max `+dw`; Center `+dw/2`; Stretch `+dw/2`; Scale `c.x *= rx`. The same applies vertically.
   - Size, uniform case (`rx == ry` within 1e-4) with both axes Stretch or both Scale: the child's local size scales by `rx`.
   - Size, otherwise: the local size is unchanged. UNRESOLVED (Q522-1): how Figma resizes a rotated child under non-uniform Stretch. Searched on 2026-10-18 with no answer; see §6 Facts.
   - The editor keeps skipping rotated children (`layout.rs:179`) until Q522-1 is answered. Changing editor behaviour is out of scope.
4. **Recursion.** When a child's size changed and it is a non-auto-layout container, recurse with `(old_child, new_child)`, as the editor does.
5. **Stale geometry.** When a child's size changes, drop its pre-baked `fill_geometry` and `stroke_geometry`, the imported Figma paths. They were baked for the old size; the renderer then rebuilds from the shape. Arc data (`arcData`, ellipse start and end angles) is size-relative and is kept.
6. **Derived data wins.** `derivedSymbolData` entries still apply after the pass and override its result for the nodes they list (Figma's computed values, `rest_import.rs:1912-1916`).
7. **Sentinel key.** Entries whose `overrideKey` is `4294967295:4294967295` are ignored, so they cannot block the match of real derived entries. This is shared with #518 rule 1. Whichever lands first implements it, and the other rebases.

## 4. Data model / schema changes

None.

## 5. Contracts

`pub fn constrain_child(child: &Node, old: Vec2, new: Vec2) -> (Affine, Vec2)` in `designos-doc` (for example `layout_constraints.rs`), exported from `lib.rs`. `designos-app/src/layout.rs` `apply_constraints` calls it, with unchanged behaviour for unrotated children.

## 6. File-format mapping (RESOLVED)

- `NodeChange.derivedSymbolData = 125`, `overrideKey = 213`.
- Constraints: `horizontalConstraint` and `verticalConstraint` (the existing mapping in `fig.rs`; MIN, MAX, CENTER, STRETCH, SCALE).
- Size and transform as imported.
- Kiwi ids confirmed against Figma's cached fullscreen wasm schema (REA 2026-10-18):
  - `NodeChange.derivedSymbolData = 125`, `guidPath = 111`, `size = 11`, `transform = 12`, `horizontalConstraint = 28`, `verticalConstraint = 37`;
  - `SymbolData { symbolID = 1; symbolOverrides = 2; uniformScaleFactor = 3 }`.

### Facts (REA 2026-10-19): Q522-1 RESOLVED

#522 shipped in v0.5.0 with rules 2-3 as first written (local-axis resize, unrounded deltas, `max(w, 1)`). The measured Figma rules below replace them; the follow-up that brings import and editor to them is #691.

- Method: Figma's constraint solver was read from its cached fullscreen wasm with WABT (static disassembly only, no Figma code executed or copied). The result was checked against Figma's own `derivedSymbolData` in four saved kits.
- Agreement: 3,634 resized-instance child rows (non-auto-layout, `uniformScaleFactor` 1, no group-like parent), max size error 1.2e-4. The one real rotated case (45°, uniform Scale/Scale) matches to 3e-7.
- FACT, rule for every child, rotated or not:
  1. Take the child's axis-aligned bounds in the parent's axes.
  2. Move its edges per axis: Max `+ r(P′) − r(P)`; Center `+ r(P′/2) − r(P/2)`; Stretch moves only the max edge by `r(P′) − r(P)`; Scale multiplies by `P′/P` without rounding. `r` rounds half away from zero.
  3. Map the whole child with the resulting per-axis scale (scale 1 when the old extent is under 2⁻¹²).
  4. Split the new matrix into size `(w·|col0|, h·|det|/|col0|)` and a transform that keeps any skew.
- FACT: there is no minimum-size clamp. Sub-1 and zero-thickness sizes survive.
- INFERENCE, high confidence (code-only, no saved reference): a rotated child under non-uniform resize becomes a skewed parallelogram.
- Owner-manual check (flagged, never claimed by an agent): a 100×40 rectangle rotated 30°, Stretch/Stretch, in a 200×200 component; instance resized to 300×200. Predicted derived size is about 175.130 × 44.266, with skew in the transform.
- Open (Q522-2, in #691): how group-like parents (Group, BooleanOp, resize-to-fit frames) refit their children after the pass.
- Edge case 4 revisited: pie arcs rotated 60° under Stretch/Stretch 160 → 240 scale ×1.366 (218.56), not ×1.5. Under Scale/Scale they scale ×1.5. Check which constraint the kit's arcs use.

## 7. UI

None. Evidence: Studio versus Figma render of pie chart `1084:114`, and of the dashboard frames listed in the issue, through the kit render audit.

## 8. Edge cases

1. A 160×160 component with a Stretch/Stretch child at (0,0,160,160), instanced at 280×280, with no derived data: the child is (0,0,280,280). This is the issue's AC test.
2. The same with a Scale/Scale child at (20,20,120,120): the child is (35,35,210,210).
3. A Max/Max child at (140,140,20,20), instanced at 200×200: the child is at (180,180), still 20×20.
4. Pie Series arcs rotated 60°, Stretch/Stretch, instance 160 → 240, uniform: the arcs scale ×1.5 about the moved centre, concentric with the Background.
5. A derived entry exists for one arc: that arc takes Figma's values, and the others take the constraint result.
6. An auto-layout instance resized: no constraint pass (the layout owns positions).
7. Instance size equals component size: no change (byte-identical import).
8. `old.x == 0`: `rx = 1`, with no division by zero.

## 9. Non-goals

- Editor handling of rotated children (Q522-1).
- Auto-layout re-solve on import (separate).

## 10. Test plan

Seams: `crates/designos-fig/tests/fig_roundtrip.rs`, the `designos-doc` unit tests for `constrain_child`, and `tests/real_files.rs` (kit, ignored).

| Test | Expected | Fails before |
|---|---|---|
| `resized_instance_applies_stretch_and_scale_constraints_to_children_without_derived_data` | edge cases 1–3 exact | yes |
| `rotated_children_scale_uniformly_about_moved_centre` | edge case 4: centre (120,120), size ×1.5 | yes |
| `derived_entry_overrides_constraint_result` | edge case 5 | guard |
| `constrain_child_matches_editor_for_unrotated_children` | table of Min/Max/Center/Stretch/Scale × 2 axes against the current `apply_constraints` outputs | guard (shared layer) |
| `untitled_ui_pie_chart_resized_instances_match` (ignored, kit) | the Series arcs' world bounds equal the Background's bounds ±0.5 px | yes |

## 11. Performance budgets

- No `CLAUDE.md` §4 invariant applies to import.
- The pass is O(children of resized instances). It runs only when the sizes differ.

## 12. Dependencies and build order

- No issue dependency.
- Hot file: serialise after #518 (shared sentinel rule) and before #525.

## 13. Owner rulings that apply

- Hot-file rule (`docs/releasing.md`).
- Delivery law 4 ("fix at the shared layer") motivates the shared `constrain_child`.
