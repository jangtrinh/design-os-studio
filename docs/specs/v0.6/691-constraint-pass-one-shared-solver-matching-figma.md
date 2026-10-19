# #691 Constraint pass: one shared solver that matches Figma for rotated children, rounding and sub-1 sizes (import and editor)

| Field | Value |
|---|---|
| Issue | #691 (bug; follows #522, shipped in v0.5.0) |
| Size | L |
| Depends on | none |
| Unblocks | later import-fidelity work on resized instances; part 1's E1-4 reads the same importer |
| Touches | new `crates/designos-doc/src/layout_constraints.rs` (solver, exported from `lib.rs`), `crates/designos-fig/src/rest_import.rs` (hot file: `apply_instance_constraints` `:2106-2178`, call site `:2094-2101`), `crates/designos-app/src/layout.rs` (`apply_constraints` `:179-231`), `crates/designos-app/src/ops.rs` (call site `:60`), tests in `designos-doc`, `designos-fig/tests/fig_roundtrip.rs`, `designos-fig/tests/real_files.rs` |

Code baseline: `origin/main@9f3f7a5`. Measured Figma rules: from the issue body and from `docs/specs/v0.5/522-resized-instances-apply-stretch-and-scale-constraints.md` on `origin/docs/specs-v0.5`, section 6 "Facts (REA 2026-10-19): Q522-1 RESOLVED". They are used verbatim as the behaviour rules below.

## 1. Summary and user story

As a designer importing a kit or resizing a frame, I want children with Min, Max, Center, Stretch and Scale constraints to land where Figma puts them: rotated children included, rounding included, thin lines included. Import and editor must agree, because they are one rule.

## 2. Current state (FACT)

- Importer: after `expand_instance` copies a component's children, if the instance size differs from the component's by more than 1e-4 on an axis and `scales` has no entry for the instance (`uniformScaleFactor` absent), it runs `apply_instance_constraints(inst, old, new, 16)` (`rest_import.rs:2094-2101`). The function returns when `depth == 0` or `old.x <= 0` or `old.y <= 0` (`:2107-2109`), skips in-flow children of an auto-layout parent (`:2120-2122`), and computes per axis: Max `+dw`, Center `+dw/2`, Stretch `size + dw`, Scale `x * rx`, with `max(size, 1.0)` for Stretch and Scale (`:2127-2141`). A rotated child is treated through its centre: the **local** size changes along local axes and the centre moves (`:2123-2147`). When a size changed and the node is not a vector or boolean op, it drops `fill_geometry` and `stroke_geometry` (`:2151-2156`); it recurses into containers whose size changed (`:2157-2159`).
- Editor: `apply_constraints` returns for auto-layout frames and for `Group` (`layout.rs:180-183`), skips children whose `transform.b` or `.c` is non-zero (`:191-193`), applies plain `dw`, `dw/2`, `x *= rx`, clamps size to `max(.., 1.0)` (`:196-221`), writes through `Edit::node` (`:222-226`) and recurses when a child's size changed. It is called from the resize path (`ops.rs:60`) and tested at `layout.rs:586-597`.
- The two copies differ in more than the issue's table says: the importer recurses into every container, drops baked geometry and returns for a zero extent of the whole pass; the editor skips group parents and keeps baked geometry.
- Types: `Constraint { Min, Max, Center, Stretch, Scale }` (`designos-doc/src/node.rs:1101-1108`); `Affine { a, b, c, d, tx, ty }` in `f32`, applied as `(a x + c y + tx, b x + d y + ty)`, columns `(a, b)` and `(c, d)` (`designos-doc/src/geom.rs:47-54`, `:76-78`); `Mul` composes with the right operand first (`:102-105`). The matrix already carries skew.
- Measured on 2026-10-19 (522 Facts): 3,634 resized-instance child rows from four local kits agree with Figma's `derivedSymbolData` to a maximum size error of 1.2e-4; one real rotated case (45 degrees, uniform Scale) agrees to 3e-7. The non-uniform rotated case is predicted by analysis and has no saved Figma reference.
- I re-derived the issue's rotated example (100 x 40, 30 degrees, Stretch/Stretch, 200 x 200 to 300 x 200) from rules 1 to 5: the bounds are 106.60 x 84.64, the horizontal scale is 206.60 / 106.60, the new size is 175.13 x 44.27. This matches the issue's 175.130 x 44.266.
- Existing importer test for this area: `resized_instance_applies_stretch_and_scale_constraints_to_children_without_derived_data` (`designos-fig/tests/fig_roundtrip.rs:2446`). Private-kit tests use `#[ignore]` and the `reference()` helper that reads `DESIGNOS_REFERENCE_DIR` (`designos-fig/tests/real_files.rs:12`, `:34`).
- UNKNOWN: the file names of the four kits behind the measurement (not recorded in the repository). Check: ask the owner for the list from the REA report; the ignored test reads whatever the helper finds.
- UNKNOWN: whether `fig_export.rs` writes a skewed `transform` unchanged and whether selection handles and hit tests draw a skewed child correctly (the handle code takes four corners, `canvas/handles.rs:41-67`, but only rotation was exercised). Check: export a skewed child, re-import, and take a render.

## 3. Behaviour rules (the measured Figma rules, verbatim)

1. B = the axis-aligned bounds, in the parent's axes, of the child's local rectangle `(0, 0, w, h)` under its transform.
2. Per axis, with old parent extent P and new P', move B's min edge e0 and max edge e1:
   - Min: unchanged.
   - Max: `e + r(P') - r(P)`.
   - Center: `e + r(P'/2) - r(P/2)`.
   - Stretch: e0 unchanged, `e1 + r(P') - r(P)`.
   - Scale: `e * P' / P`, with no rounding.

   `r` rounds half away from zero.
3. Per-axis scale `s = new extent / old extent` (s = 1 when the old extent is under 2^-12). The map is `p -> B'.min + s * (p - B.min)`.
4. The new matrix M' is the map applied after the child's matrix. If det M' is 0 or not finite, leave the child unchanged.
5. Split M' into size and transform:
   - `kx = |col0|`, `ky = |det M'| / |col0|`.
   - New size `(w * kx, h * ky)`; the transform's columns are divided by `kx` and `ky`.
   - Skew is kept in the transform. `Affine` already carries a full 2 x 3 matrix.
6. Parents are solved before children; each child uses its immediate parent's new size. In-flow children of auto-layout parents get no pass. Absolute children do.
7. Group-like parents (Group, BooleanOp, frames with resize-to-fit) refit to their children after the pass. How the refit shifts the children's local coordinates is open. This task measures it on the kit files and documents it before coding it (spike S1). Until S1 is answered, each caller keeps its present behaviour for group-like parents; nothing else waits.
8. `derivedSymbolData` still overrides the result for the nodes it lists (importer only).

Implementation rules that follow from these:

9. `constrain_child(child: &Node, old: Vec2, new: Vec2) -> Option<(Affine, Vec2)>` returns the new transform and size, or `None` when rule 4 leaves the child unchanged. It computes in `f64` and stores `f32`. One shared walker `plan_constraint_pass(doc: &Document, parent: NodeId, old: Vec2, new: Vec2, depth: usize) -> Vec<(NodeId, Affine, Vec2)>` applies rule 6 and recursion, so both callers get the same order and skips; each caller applies the plan its own way (the importer writes the nodes, the editor writes through `Edit`).
10. The importer keeps its post-steps: dropping baked fill and stroke geometry when the size changed (not for vectors and boolean ops) and the `derivedSymbolData` pass. The editor keeps its recursion trigger. Neither post-step moves into the shared function.
11. The editor's skip for rotated children and its `max(.., 1.0)` clamps are removed. The importer's `max(.., 1.0)` and its whole-pass return for a zero extent are replaced by rules 3 and 4.

## 4. Data model / schema changes

None in the document model. A resized instance can now hold a child with a skewed transform and a sub-1 size; both are already representable.

## 5. Contracts

- `designos-doc` exports `constrain_child` and `plan_constraint_pass` from a new `layout_constraints.rs`. The two local copies (`rest_import.rs:2106-2178` body, `layout.rs:179-231` body) are deleted; the functions keep their names as thin callers.
- `designos-fig` and `designos-app` already depend on `designos-doc`; no new dependency.

## 6. File-format mapping

- `.fig` import reads `horizontalConstraint` (Kiwi id 28), `verticalConstraint` (37), `size` (11), `transform` (12), `derivedSymbolData` (125) as already mapped (522 section 6). No new field.
- `.fig` export writes size and transform as it does today. UNKNOWN: round-trip of a skewed transform (see section 2); check as stated there.
- `.designos`: the matrix is saved as is.

## 7. UI

None. No control or layout changes. Evidence is visual: a CPU render of a rotated child in a resized instance, before and after, beside the Figma screenshot of the same frame (`designos --screenshot`, CPU path), plus the kit test output. Responsive: SKIPPED (owner default).

## 8. Edge cases

1. A 90-degree child, 100 x 40, in a 200 x 200 parent resized to 300 x 200 with Stretch/Stretch: size 100 x 140, no skew (the issue's unit test).
2. Center rounding: parent 101 to 150 moves a centred child by `r(75) - r(50.5) = 75 - 51 = 24`, not 24.5.
3. A 0.5 px line keeps 0.5 under Stretch on the other axis; a zero-thickness line stays zero.
4. Old extent under 2^-12: scale 1 on that axis, no division by zero.
5. Det M' zero or non-finite (a collapsed parent extent): the child is left unchanged.
6. Scale constraint on a rotated child: `s` from rule 3 maps the whole child; a uniform Scale gives a uniform size change (the 45-degree measured case).
7. Group-like parent: unchanged from today until S1 (rule 7).
8. An instance with `uniformScaleFactor` set: no pass (importer guard unchanged).
9. An auto-layout parent: in-flow children skipped, absolute children solved.
10. Recursion depth: capped at 16 as today.
11. A resized instance whose child has a derived entry: derived values win (rule 8).

## 9. Non-goals

- Auto-layout re-solve on import. Changing which instances get a pass (the `scales` guard). Editor resize UX beyond the constraint rule. Group-like refit before S1.

## 10. Test plan (red first)

Seams: `designos-doc` unit tests for `constrain_child`; the importer test file; the ignored kit test.

| Case | Expected | Fails before |
|---|---|---|
| `rotated_90_stretch_stretch_gives_100_by_140_without_skew` | child 100 x 40 at 90 degrees in 200 x 200, parent to 300 x 200: size 100 x 140, `b*c` consistent with no skew | yes (no function; importer grows local axes) |
| `center_rounding_uses_rounded_extents` | 101 to 150: delta 24 | yes |
| `sub_one_child_keeps_its_size` | 0.5 px line keeps 0.5 | yes (clamped to 1) |
| `rotated_30_stretch_stretch_gives_175_130_by_44_266_with_skew` | size within 1e-3 of 175.130 x 44.266; transform has skew | yes |
| `det_zero_leaves_the_child_unchanged` | unchanged | yes |
| `old_extent_under_2_pow_minus_12_scales_by_one` | unchanged on that axis | yes |
| `half_away_from_zero_rounding_table` | `r(-0.5) = -1`, `r(0.5) = 1`, `r(2.5) = 3` | yes |
| `editor_and_importer_call_the_shared_pass` | both produce equal results for one fixture (a table of five constraint pairs, rotated and not) | yes |
| `editor_no_longer_skips_rotated_children` | a rotated child moves on resize | yes |
| `resized_instance_applies_stretch_and_scale_constraints_to_children_without_derived_data` | still green | guard |
| `derived_entry_overrides_the_pass` | derived values win | guard |
| kit test (ignored): resolve every resized instance child without `derivedSymbolData` overrides; compare to the stored derived values | max error under 2e-3; scope: non-auto-layout, `uniformScaleFactor` 1, no group-like parent | yes |
| `no_local_constraint_copies_remain` | grep: neither `rest_import.rs` nor `layout.rs` holds the per-axis `match` on `Constraint` | yes |

Run: `cargo test -p designos-doc`, `cargo test -p designos-fig`, `cargo test -p designos-app layout`; kit test with `cargo test -p designos-fig --test real_files -- --ignored`. Evidence: red/green capture, the before/after CPU render with the Figma screenshot, the kit test output.

Owner-manual check (flagged, never claimed by an agent): in Figma, a 100 x 40 rectangle rotated 30 degrees with Stretch/Stretch in a 200 x 200 component; resize an instance to 300 x 200; save. The file's derived size and transform for that child must match `rotated_30_stretch_stretch_gives_175_130_by_44_266_with_skew`.

## 11. Performance budgets

- The pass is O(children of resized containers) and runs only when sizes differ. Editor path: under 0.5 ms for a resize step of a frame with 1,000 children (measured; it runs on every resize frame, `ops.rs:60`).
- Importer: no measurable change in the kit import wall time (within noise, recorded before and after on the same kit).
- No allocation per child beyond the plan vector; `f64` math only inside the function. No UI-thread `to_json`; no idle cost.

## 12. Dependencies and build order

No task dependency. `rest_import.rs` is a hot file with a one-at-a-time rule (README-part3): #691 and part 4's #5 queue behind FU-669 and FU-582 and ahead of part 1's E1-4; do not interleave. `layout.rs` and `ops.rs` are shared with nothing else in v0.6 that this index knows of. Land the `designos-doc` function and its unit tests first, then the importer, then the editor, as three commits.

## 13. Owner rulings

- Owner ruling 2026-10-19: accept the editor behaviour change. Rotated children now move and resize with their frame, as in Figma, and children may become thinner than 1 px. Spike S1 on group-like parents runs first.

- The measured Figma rules (2026-10-19) are the behaviour; one shared function in `designos-doc` called by import and editor; the editor's rotated skip is removed; the owner-manual check is flagged and never claimed. (Issue text.)
- Fix at the shared layer (delivery law 4). Hot-file rule (`docs/releasing.md:21`).

## Open questions for the owner (recommended first)

(Answered by the owner 2026-10-19, see §13.) 1. The editor changes behaviour: rotated children now move and resize with their frame, and children may become thinner than 1 px. Recommended: accept, since that is what Figma does and the issue asks for it.
2. Group-like parents (rule 7). Recommended: run spike S1 on the kit files first and write the measured refit rule into this spec before the group-like code is written.
