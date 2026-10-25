# #639 Codegen: a grid child with counter-axis Fill exports its solved size

| Field | Value |
|---|---|
| Issue | #639 (found in the #548 review; exists on main, not a regression) |
| Size / priority | S / P2 |
| Depends on | #578 (same function, `sizes_along`; queue after it) |
| Unblocks | none |
| Touches | `crates/designos-export/src/codegen.rs` (`sizes_along`, fixtures, tests), tests in `css.rs`, `tailwind.rs`, `swiftui.rs`, `compose.rs`. No change to `designos-doc`. |

Code baseline: `origin/main@01d48f2` (0.5.0).

## 1. Summary and user story

As a developer exporting a grid, a child whose vertical sizing is Fill (an imported `.fig` STRETCH
child, or a child set to fill height in a grid) exports the pixel height the solver gave it. Today
SwiftUI prints `.frame(width: 30, maxHeight: .infinity)` for a child the solver sized 30 x 20 on an
auto row, so the exported view grows beyond the row.

## 2. Current state (FACT)

- `sizes_along` (`crates/designos-export/src/codegen.rs:380-423`) computes `counter_horiz` only for
  a non-grid flow parent: `parent.al.filter(|_| in_flow && !grid_parent).map(..)` (`:394`). For a
  grid child `effective_sizing(false, None)` therefore returns `Fill` only when the child's own
  `layout_child.vertical == Sizing::Fill` (`designos-doc/src/layout_solver_types.rs:118-125`), and
  `fill_h` is true (`:405`), so `h = Dim::Fill` (`:413-414`).
- #548 made `AlignSelf::Stretch` alone export the solved size in a grid: the comment at
  `codegen.rs:388-392` says codegen "never turns `AlignSelf::Stretch` into a fill for a grid child and
  exports the solved pixels". The Fill case was left out; the fixture parameter that reproduces it
  already exists but no test sets it: `fixture::grid_stretch_child(row_track, imported_fill)`
  (`codegen.rs:747-765`), `imported_fill = true` stores `Sizing::Fill` on the height. All existing
  callers pass `false` (`swiftui.rs:613`, `compose.rs:522`, `css.rs:1009`, `codegen.rs:1116`).
- `Dim::Fill` prints: SwiftUI `maxHeight: .infinity` (`swiftui.rs:211-213`), Compose
  `fillMaxHeight()` (`compose.rs:195-205`), CSS nothing for the length (the grid branch only emits
  `align-self: stretch` when `align_self == Stretch`, `css.rs:66-70`), Tailwind nothing for a grid
  parent (`tailwind.rs:433` skips `self-stretch` when `grid_parent`).
- The solver sizes the grid's auto row from the children and then stretches a Fill child to the row:
  a 30 x 20 child on an auto row solves to 30 x 20; with a fixed 100 row track it solves to 30 x 100
  (the expectations of `grid_stretch_child_solved_sizes`, `codegen.rs:1114-1121`).
- A grid runs along the width in the solver and in the web generators (`web_sizes`,
  `codegen.rs:361-378`); the stack generators emit a grid as a column (`sizes`, `:354-359`).

## 3. Behaviour rules

1. For a child of a grid parent, a vertical axis that stretches (`lc.vertical == Sizing::Fill`, with or
   without `AlignSelf::Stretch`) exports `Dim::Fixed(n.size.y)`, the size the solver wrote. The
   existing `AlignSelf::Stretch` rule is unchanged.
2. The width axis of a grid child is untouched: horizontal Fill keeps meaning "fill the cell" in every
   generator (SwiftUI `maxWidth: .infinity`, no CSS width), and `grow` keeps its per-generator meaning
   (`codegen.rs:361-378`).
3. Web output for the new case prints the pixel height and the stretch declaration the Stretch case
   prints today: CSS `height: <n>px;` plus `align-self: stretch;` when the node also stores
   `AlignSelf::Stretch`; Tailwind `h-[<n>px]`. A Fill-only child (no Stretch) prints the height only.
4. All six generators change through `sizes_along`; UIKit and Android XML print the fixed length
   instead of a hugging-priority line / `match_parent` and are covered by tests.
5. A grid whose row tracks are flexible (`1fr`) still exports the solved pixels for the child, the
   same trade-off #548 made for Stretch; the grid frame's own `1fr` template is unchanged.

## 4. Data model / schema changes

None.

## 5. Contracts

No signature change. Output change: a grid child with vertical Fill prints a pixel height.

## 6. File-format mapping

None.

## 7. UI

None.

## 8. Edge cases

1. Auto row, 30 x 20 child, vertical Fill: `height: 20px` / `.frame(width: 30, height: 20)` /
   `.size(width = 30.dp, height = 20.dp)` (acceptance).
2. Fixed 100 row track: 100.
3. Imported shape: Fill and Stretch stored together: same pixels; CSS also keeps `align-self: stretch`.
4. Grid child that spans two rows: the solved `n.size.y` is the spanned height; printed as is.
5. Grid child with vertical Hug: unchanged (`Dim::Hug` / content height).
6. A non-grid flow parent: unchanged (handled by #578).
7. Exporting a child on its own (no flow parent): unchanged; tests export the grid, as the #548 tests
   do (`swiftui.rs:611-612`).

## 9. Non-goals

- Horizontal Fill inside a grid cell.
- Emitting `fr`-aware or relative heights.
- Changing grid track export (T-grid codegen, #146).

## 10. Test plan (red first)

Seams: unit tests in `designos-export`, reusing `fixture::grid_stretch_child(track, imported_fill)`;
add `fixture::grid_fill_child(track)` (Fill height, no `AlignSelf`) so the Fill-only case is tested
separately from the imported pair. Synthetic data only.

| Case | Steps | Expected (fails before the fix) |
|---|---|---|
| `swiftui_grid_fill_child_exports_solved_height` | tracks `(None, 20)`, `(Fixed(100), 100)`; `swiftui_for(grid)` | contains `width: 30, height: {h}`; no `maxHeight` |
| `compose_grid_fill_child_...` | same | `width = 30.dp, height = {h}.dp`; no `fillMax` / `weight` |
| `css_grid_fill_child_...` | same, `css_for(child)` | `height: {h}px;`, `width: 30px;`, no `flex` |
| `css_grid_imported_fill_plus_stretch` | `imported_fill = true` | as above plus `align-self: stretch;` |
| `tailwind_grid_fill_child_...` | same | `h-[{h}px]` |
| `uikit_...` / `android_xml_...` | same | fixed height constraint / `layout_height="{h}dp"` |
| `grid_child_horizontal_fill_unchanged` | child with horizontal Fill | still no width in CSS, `maxWidth: .infinity` in SwiftUI |

Run: `cargo test -p designos-export`. Evidence: red run before, green after; one headless-Chrome
render of the exported 2-column grid HTML showing the child at its solved height.

## 11. Performance budgets

None (constant work inside an existing function).

## 12. Dependencies and build order

After #578 (same function). Independent of every other v0.6 task.

## 13. Owner rulings

- Codegen exports what the layout solver computed (#532 ruling carried through #548).
- Visual evidence mandatory: the PR carries the render.
