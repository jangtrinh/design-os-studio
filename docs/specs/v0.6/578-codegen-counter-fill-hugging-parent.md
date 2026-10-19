# #578 Codegen: a counter-axis Fill child in a hugging parent prints its solved length

| Field | Value |
|---|---|
| Issue | #578 (follow-up of #532 / PR #548) |
| Size / priority | S / P2 |
| Depends on | none |
| Unblocks | #639 (same function; land #578 first so #639 builds on the new `keeps_*` rule) |
| Touches | `crates/designos-export/src/codegen.rs` (`sizes_along`, fixtures, unit tests), tests in `css.rs`, `tailwind.rs`, `swiftui.rs`, `compose.rs`. No change to `designos-doc`. |

Code baseline for every reference below: `origin/main@01d48f2` (0.5.0). The issue's references
(`effective_sizing`, the `Dim::Fill` length skip) are still where #548 left them; line numbers are given
fresh.

## 1. Summary and user story

As a developer exporting a frame to CSS, Tailwind, SwiftUI, UIKit, Compose or Android XML, a row whose
children all fill the counter axis exports a stable height instead of collapsing: a hugging row holding
two Fill-height children of stored size 20 and 100 prints 100 for both, the size the layout solver
gives them.

## 2. Current state (FACT)

- Sizing for every generator comes from one function, `sizes_along`
  (`crates/designos-export/src/codegen.rs:380-423`). `sizes` (stack generators: SwiftUI, UIKit,
  Compose, Android XML, `:356`) and `web_sizes` (CSS, Tailwind, `:366`) both call it.
- #548 added `cross_stretch` (`codegen.rs:398-401`): an `AlignSelf::Stretch` child in a non-grid flow
  parent whose `counter_sizing == Sizing::Hug` keeps its own cross length (`keeps_w` / `keeps_h`,
  `:402-403`), so `fill_w` / `fill_h` (`:404-405`) are false and `w` / `h` stay `Dim::Fixed`.
- That condition ends with `... != Sizing::Fill` (`:401`): a child whose counter axis is
  `Sizing::Fill` is deliberately excluded. It therefore gets `Dim::Fill`
  (`effective_sizing` returns Fill when the own sizing is Fill, `designos-doc/src/layout_solver_types.rs:118-125`).
- `Dim::Fill` on a cross axis prints no length: CSS pushes only `align-self: stretch`
  (`css.rs:48-49`, stretch flag `:74-90`, emission `:99-110`), Tailwind `self-stretch` (`tailwind.rs:428-433`), SwiftUI
  `maxHeight: .infinity` (`swiftui.rs:207-213`), Compose `fillMaxHeight()` (`compose.rs:195-205`).
  With a hugging parent there is nothing for the child to fill, so a browser sizes the line from the
  children's content, which is 0 for an empty shape. Imported `.fig` STRETCH children store both
  `Sizing::Fill` and `AlignSelf::Stretch` (comment in `layout_solver_types.rs:127-133`).
- The solver treats Fill and `AlignSelf::Stretch` as the same stretch (`ChildIn::stretches_cross`,
  `designos-doc/src/layout_solver_types.rs:101`; used at `layout_solver.rs:193` and `:323`). In a hugging counter axis without margins a
  stretching child "keeps its natural margin box as the line size when none [of the children] does",
  so an all-stretch row does not fold to its padding (comment and loop at `layout_solver.rs:181-203`).
- Existing coverage of the Stretch twin: `stretch_child_keeps_its_cross_size_when_the_parent_hugs` in
  `css.rs:982`, `tailwind.rs:775`, `swiftui.rs:598`, `compose.rs:507`; fixture
  `fixture::row_with_stretch_children` (`codegen.rs:771`).

## 3. Behaviour rules

1. A flow child (not a grid child, not absolute) whose counter axis stretches counts as a
   `cross_stretch` child when the parent's `counter_sizing` is `Hug`. "Stretches" means
   `lc.effective_sizing(counter_is_horizontal, Some(..)) == Sizing::Fill`, which covers `Sizing::Fill`
   alone, `AlignSelf::Stretch` alone, and both stored together (imported `.fig`). The `!= Sizing::Fill`
   exclusion at `codegen.rs:401` is removed.
2. For such a child the counter-axis `Dim` is `Dim::Fixed(n.size)` on that axis, where `n.size` is the
   size the solver wrote (the document is solved before export; `relayout_subtree` is the caller's
   precondition, as in #548). The main axis is unchanged.
3. Web output keeps the stretch declaration next to the length: CSS `align-self: stretch` with an
   explicit `height` or `width` (the explicit length wins in a browser, which is the intended solved
   size), Tailwind `self-stretch` with `h-[..]` / `w-[..]` (the `sz.cross_stretch` branches at
   `css.rs:87-89` and `tailwind.rs:437-439` already do this for Stretch).
4. A parent whose counter axis is `Fixed` is untouched: the child keeps `Dim::Fill` and prints no
   length (existing test `stretch_child_leaves_the_cross_size_to_a_fixed_parent`, `css.rs:998`).
5. A main-axis Fill (`grow`, or Fill along the parent's primary axis) is untouched.
6. Because the rule lives in `sizes_along`, UIKit and Android XML change too. Their output for the new
   case is the fixed length (`heightAnchor.constraint(equalToConstant:)`, `layout_height="Ndp"`) instead
   of the hugging-priority line / `match_parent`; the tests below pin that so it is intentional.

## 4. Data model / schema changes

None. `Sizes.cross_stretch` already exists (`codegen.rs:351`); its doc comment is updated from
"`AlignSelf::Stretch` on a non-Fill child" to "a stretching child of a hugging flex parent".

## 5. Contracts

No public function signature changes. Public output change: Fill-height / Fill-width children of a
hugging flex parent print a pixel length (listed in section 10). The output of Fixed and Hug parents,
of grids and of main-axis Fill does not change.

## 6. File-format mapping

None.

## 7. UI

None.

## 8. Edge cases

1. Row hugging its height, children stored 20 and 100, both `Sizing::Fill` on the vertical axis, no
   `AlignSelf`: after the solve both are 100 and both export 100 (the acceptance case).
2. The same, with imported `Fill + Stretch` stored together: same output.
3. One plain (non-stretching) sibling of height 60 and one Fill child: the solver sizes the line to 60
   (`layout_solver.rs:181-203`); the Fill child exports 60.
4. Vertical stack hugging its width: symmetric (`keeps_w`).
5. Child with cross margins: the solver only holds the line open by the child's minimum plus margins;
   the exported length is still the solved `n.size`.
6. An unsolved document (a caller skipped `relayout_subtree`): the output is the stored size, which is
   the pre-existing behaviour for every Fixed axis; not a regression and not handled here.
7. Wrapped flow parent (`wrap: true`) with hugging counter axis: the rule applies per child the same
   way; the line height the solver chose is `n.size`.

## 9. Non-goals

- Changing the layout solver or the stored `stretched` bookkeeping.
- Grid children (#639).
- Making codegen solve the layout itself.

## 10. Test plan (red first)

Seams: unit tests inside `designos-export`, using `fixture::row_with_stretch_children` extended with a
`counter_fill: bool` parameter (children get `layout_child.vertical = Sizing::Fill`, optionally also
`AlignSelf::Stretch`). All fixtures are synthetic.

| Case | Steps | Expected (fails before the fix) |
|---|---|---|
| `css_hugging_row_all_fill_children_print_the_solved_height` | row Hug, kids `[20, 100]` Fill-height; solve; `css_for` each kid | `height: 100px;` and `align-self: stretch;` |
| `tailwind_hugging_row_all_fill_children` | same, `tailwind` classes | `h-[100px]` and `self-stretch` |
| `swiftui_..._` / `compose_..._` | same, `swiftui_for(row)`, `compose_for(row)` | `height: 100`, no `maxHeight`; `height = 100.dp`, no `fillMaxHeight` |
| `uikit_...` / `android_xml_...` | same | fixed height constraint / `layout_height="100dp"` |
| `imported_fill_plus_stretch_same_output` | kids with Fill and Stretch | identical to the Fill-only output |
| `fixed_parent_stays_fill_only` | same row with `counter_sizing: Fixed` | `align-self: stretch`, no `height:` (existing behaviour) |
| `plain_sibling_sets_the_line` | kids `[60 plain, 20 Fill]` | Fill child prints `60` |
| `vertical_stack_symmetric` | column Hug width | `width: 100px` on both |

Run: `cargo test -p designos-export`. Evidence: the failing run before the change and the passing run
after (terminal capture), plus one rendered comparison: the exported HTML of the 20/100 fixture in
headless Chrome (a screenshot of the two children at equal height) next to the pre-fix collapsed one.

## 11. Performance budgets

None: a constant number of comparisons per node inside an existing function.

## 12. Dependencies and build order

Independent of every other v0.6 task. Land before #639; both edit `sizes_along`, so they are one
worker's two commits or two PRs queued, not parallel.

## 13. Owner rulings

- Codegen matches the layout solver (#532 ruling carried by #548: "export the solved size").
- Visual evidence is mandatory: PR carries the before/after render.
