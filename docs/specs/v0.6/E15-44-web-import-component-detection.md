# E15-44 Detect repeated subtrees in web imports and turn them into components

- Issue: #634 (task E15-44, epic #585, L / P0)
- Spec base: `main` at `767acf9`.
- Files: the web-capture importer from #277: new `component_detect` step (`signature.rs`, `grouping.rs`, `promote.rs`), capture script addition for framework component names (#276 format v2, shared with E15-43).

## 1. Summary and user story

As a designer importing a page, I want repeated cards, rows and buttons to become one main component with instances, with differing text and images as overrides, named after the page's own component names when available, so the import is editable as a system instead of loose copies.

## 2. Current state

- FACT: no web capture exists on `main`; #276 captures boxes, computed styles, text runs and images; #277 imports them as auto-layout frames with editable text, screenshot diff within threshold.
- FACT: components and instances are part of the document model (`Node.component: ComponentLink`, `crates/designos-doc/src/node.rs:122`); E15-1 (#586) places instances and variants from `apply_spec`; instance overrides are tracked per field (`OverrideField`, `node.rs:164-169`).

## 3. Behaviour rules

1. **Capture addition.** For each element the capture records `framework_name` when available: the nearest React component display name from the element's fiber (`__reactFiber$*` key, `type.displayName || type.name`), the Vue component `name` (`__vueParentComponent.type.name`), or a `data-component` attribute. Absent → `null`.
2. **Signature** (computed bottom-up on the imported node tree, before promotion):
   - `sig(node) = H(kind, tag, sorted class list with digits-bearing utility tokens kept, layout shape, style key, [sig(child) for child in children])` with H = 64-bit FNV-1a over a canonical byte encoding;
   - `layout shape` = auto-layout direction, wrap, alignment, child count;
   - `style key` = fills, strokes, radii, effects, font family, size and weight of text runs; **text characters and image bytes are excluded** (they become overrides);
   - size is not part of the signature; it is checked by tolerance (rule 3).
3. **Groups.** Nodes with equal signatures form a candidate group when:
   - the group has ≥ 2 members (setting `import.component_min_repeats`, default 2) and each member has ≥ 3 nodes in its subtree;
   - member widths and heights are within ±8 px or ±10% (whichever is larger) of the group median;
   - no member is an ancestor of another member.
4. **Selection.** Candidate groups are processed by descending subtree size; members of a selected group are marked; a smaller group is skipped when all of its members are inside members of an already selected group **and** it repeats only within them in the same positions (it will be part of the main component). A smaller group whose members also appear elsewhere becomes its own component and is nested: the larger main component contains instances of it.
5. **Promotion.**
   - The first member in document order becomes the main component, moved to an `Imported components` section placed to the right of the imported page frame (100 px gap, stacked vertically with 40 px gaps), and replaced in place by an instance of itself.
   - Every other member is replaced by an instance at the same position and size; differing text characters become text overrides; differing image fills become fill overrides; size differences within tolerance become instance resizes (resized instances follow #522 behaviour).
   - Names: `framework_name` when all members share it (`ProductCard`); else the most common `framework_name` among members; else a name derived from the root's tag and first class (`Card`, `Li item`); duplicates get ` 2`, ` 3`.
6. **Verification.** After promotion, render the imported page and the pre-promotion import on the CPU path and compare with `designos-diff` (tolerance 8, AA detection on): mismatched ratio ≤ 0.5%. A larger difference rolls back promotion of the group that caused it (groups are promoted one at a time in selection order with a check after each) and reports `component_promotion_reverted` with the group name.
7. **One undo step** for the whole import, including promotions (part of #277's import step).

## 4. Data model / schema

No document format change. Importer types:

```rust
pub struct CandidateGroup { pub signature: u64, pub members: Vec<NodeId>, pub subtree_size: u32, pub framework_name: Option<String> }
pub struct ComponentDetectReport { pub components: u32, pub instances: u32, pub nested: u32, pub reverted: Vec<String>,
    pub groups_skipped_size: u32 }
```

## 5. Contracts

The #277 import tool and CLI gain `detect_components: bool` (default true) and `component_min_repeats: u32` (default 2), and report `ComponentDetectReport` fields.

## 6. File-format mapping

Promoted components and instances are ordinary components; `.fig` export writes them as such.

## 7. UI

No new UI. The import report sheet lists components found and any reverted groups. The `Imported components` section is visible on the canvas next to the page.

## 8. Edge cases

1. Members with different hover or focus states captured: states are not captured by #276 (static capture); identical static styles group together.
2. A list where one item has an extra badge child: different signature; it stays a loose copy (or forms its own group with other badged items).
3. Text-only repeats (`<span>` labels): skipped by the ≥ 3 node rule.
4. Images that are the same bytes in every member: no override needed; image stays in the main.
5. Very large pages (5,000 repeated rows): grouping is linear in nodes (hash map by signature); promotion of 5,000 instances is one batch.
6. Members split across two auto-layout parents: allowed; instances keep their parents.

## 9. Non-goals

- Variant detection (grouping similar components into component sets with properties); a follow-up.
- Token mapping (E15-43).

## 10. Test plan

Seam: capture fixture test with local fixture pages (Playwright, as #276) plus importer unit tests on stored captures, plus a screenshot.

Fixtures (ours): `six-cards.html` (a grid of 6 identical product cards with different titles, prices and images, rendered by a tiny React component `ProductCard`; the fixture folder vendors a pinned React UMD build, MIT, recorded in the E15-23 register, so the page works offline), `badge-variant.html`, `nested-buttons.html` (cards containing a button that also appears in the header).

Fail-before: no detection step.

| Test | Input | Expected |
|---|---|---|
| `six_cards_one_component_six_instances` (AC1) | six-cards.html | 1 main component named `ProductCard`; 6 instances (the main's original position holds an instance) |
| `overrides_match_source_and_diff_within_threshold` (AC2) | same | each instance's title and price text equals the source page; image overrides present; CPU render vs source screenshot within #277's threshold and vs pre-promotion render ≤ 0.5% mismatched |
| `badge_item_not_grouped_with_plain` | badge-variant.html | badged item is a loose frame or in its own group |
| `nested_button_component` | nested-buttons.html | `Button` component; card main contains a `Button` instance; header button is an instance |
| `reverted_group_reported` | fixture where promotion changes rendering (forced through a test hook) | group reverted; report names it |

Visual evidence: capture of the imported page with the `Imported components` section and the layers panel showing instances.

## 11. Performance budgets

- Signature and grouping ≤ 100 ms for 10,000 nodes; promotion plus per-group verification ≤ 2 s for 20 groups on a 1440-wide page.
- Runs in the importer, off the UI thread; one batch at the end (CLAUDE.md §4.1).

## 12. Dependencies and build order

- Needs #277 (T8.27), E15-1 (#586). Merge after E15-43 (same importer file).

## 13. Owner rulings that apply

- R6.22: Studio's own web-capture importer is in scope.
- Full scope: nested components, overrides and framework names.
