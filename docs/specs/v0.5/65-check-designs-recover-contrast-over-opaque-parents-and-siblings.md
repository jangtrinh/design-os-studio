# #65 Check designs: recover contrast findings over opaque parents and solid sibling backgrounds

- Issue: https://github.com/jangtrinh/design-os-studio/issues/65 (PR #45 verifier follow-up, epic #16)
- Spec base: `main` at `767acf9`. PR #45 is merged.
- Open PR: #576 (`task/65-recover-contrast-findings-over-opaqu`, +522 −40).
  - Release review, head `c639e5e`: one MAJOR. Probe B was still silent, and a test locked that in.
  - Release fix round, head `2707f3c`: a plain opaque rect or frame that fully covers the text is now the backdrop. Probe B gives 1 finding with a fix. Timed on the Untitled UI kit: 71,896 layers, total 1,504 ms, worst page 36 ms (was 1,661 / 141 ms). Re-review is pending.
  - The owner ruled on 2026-10-18 that the release session fixes this PR.
- Files: `crates/designos-app/src/check_designs/contrast.rs`, `contrast/backdrop.rs`, `contrast/behind.rs`, `contrast/subtree.rs`, `fix.rs`, `edit.rs`, and tests under `check_designs/tests/`.

## 1. Summary and user story

As a designer, Check designs should flag real contrast failures in common layouts:
- light-grey text on a white card;
- a label on a white button rectangle.

Today the conservative "something is behind it, skip" rule hides most of them.

## 2. Current state

- FACT: `has_layer_behind(doc, id, occluder, bounds)` (`contrast/behind.rs:103`) skips the text when any visible earlier sibling of the text, or of an ancestor, paints within the text box. The module doc (`behind.rs:1-2`) says: "What they paint is not modelled, so text one of them reaches is not checked".
- FACT: `backdrop(doc, id)` (`contrast/backdrop.rs:108`) composites ancestors as isolated groups (premultiplied; opacity scales the group).
- FACT: the thresholds are AA 4.5:1 and large 3:1 (`contrast.rs:55-62`). The formula is WCAG relative luminance (`contrast.rs:35-53`).
- FACT: `Tx::cmds_all_or_none` (`edit.rs:93-111`) rolls back on failure. If a rollback inverse itself fails, it returns that error, but the transaction can still be committed by the caller (`edit.rs:120`).
- FACT: issue probes. Probe A: `#BBBBBB` text in an opaque white Card over a Background rect gives 0 findings. Probe B: a white rect plus a label in a button gives 0 findings.

## 3. Behaviour rules

1. **Opaque ancestor stops the walk.** While climbing ancestors for the backdrop and the behind-scan, stop at the first ancestor `A` that satisfies all of:
   - `opacity == 1`;
   - blend `Normal` or `PassThrough`;
   - no visible effects;
   - at least one visible solid fill with alpha 1, the topmost visible fill being solid and opaque;
   - `A`'s box, inset by `max(corner radii)` on every side, contains the text's page box.

   Layers behind `A` cannot show through, so they are not scanned.
2. **Covering opaque sibling becomes the backdrop.** Among earlier siblings of the text (and of each ancestor below the stopping ancestor), take the topmost one that intersects the text box. If it satisfies all of the following, use its top fill colour as the backdrop and stop:
   - it is a `Rectangle` or `Frame`;
   - its transform is axis-aligned (rotation ≡ 0 mod 90°);
   - no visible stroke;
   - no visible effect;
   - not a mask;
   - `opacity == 1` and blend `Normal`;
   - its topmost visible fill is solid with alpha 1;
   - its box, inset by its max corner radius, contains the text box.

   If that topmost sibling does not qualify (partial cover, translucent, rotated, stroked, effected or masked), the text is skipped, exactly as today. Its descendants are never composited (owner ruling 1A).
3. **No finding when nothing is visible:**
   - an ancestor with `opacity == 0`;
   - effective text alpha (`colour.a × paint.opacity × node.opacity × ancestor opacities`) ≤ 0;
   - the node blend is neither `Normal` nor `PassThrough`.
4. **Non-clipping frames** use the union of their subtree bounds for the "may reach" test (`contrast/subtree.rs`).
5. **Fix all never commits partial state.** If a rollback inside `cmds_all_or_none` fails, the `Tx` becomes poisoned. `commit` then returns `Err(DocError::TransactionPoisoned)` without pushing History, `check_designs_fix_all` cancels the transaction and returns 0, and the panel shows the error toast `"Fix all could not be applied; nothing changed"`.

## 4. Data model / schema changes

None. In-memory only: `Tx` gains `poisoned: bool`.

## 5. Contracts

- `pub fn cmds_all_or_none(&mut self, doc: &mut Document, cmds: Vec<Command>) -> Result<(), DocError>` keeps its signature. On rollback failure it sets `self.poisoned = true`.
- `pub fn commit(self, …) -> Result<bool, DocError>` returns `Err(DocError::TransactionPoisoned)` when poisoned. This is a new variant, `#[error("transaction rolled back incompletely")] TransactionPoisoned`. `DocError` (`designos-doc/src/lib.rs:95-102`) has only `NotFound`, `Cycle` and `NotAContainer`, and none of them fits.

## 6. File-format mapping

None. RESOLVED.

REA (context for #158), from the Figma 126.9.13 editor bundle: the shared colour-contrast module classifies text as `largeText`, `normalText` or `graphics`. It reports "unavailable" reasons: `TextInBackground`, `VideoInBackground`, `ImageInBackground`, `GradientInBackground`, `BlendModeInBackground`, `BlendModeInForeground`, `MixedBackgrounds`, `MixedStandards` and `Unavailable`. Figma therefore also declines to judge over images, gradients, blends and mixed backgrounds, consistent with rules 2 and 3.

## 7. UI

No new control. Recovered findings appear in the existing Contrast group. The new error toast appears only on a poisoned Fix all. Evidence: a 1440 capture of the panel on a fixture with probes A and B.

## 8. Edge cases

| Case | Expected |
|---|---|
| Probe A: `#BBBBBB` 14 px text in a white card (opaque, radius 8, text inset ≥ 8) over a dark Background rect | 1 finding (1.92:1 < 4.5), fix offered |
| Probe B: white rect 120×40, then a `#BBBBBB` 14 px label on top | 1 finding plus fix |
| Same as B but the rect rotated 15° | 0 findings (skipped) |
| Rect 50% opacity | skipped |
| Rect with a drop shadow | skipped |
| Text in a rounded card corner region (radius 24, text at x=4) | the card is not an occluder; the scan continues |
| Ancestor opacity 0 | 0 findings |
| Text paint alpha 0 | 0 findings |
| Text blend Multiply | 0 findings |
| `#777777` on `#808080` covering rect (1.13:1) | 1 finding (replaces the PR test that expected 0) |

## 9. Non-goals

- Compositing translucent or partial siblings or their descendants (ruling 1A).
- Images and gradients as backdrops.
- AAA and rule parity (#158).

## 10. Test plan

Seam: `crates/designos-app/src/check_designs/tests/` (`covering_sibling.rs`, `under_an_opaque_fill.rs`, `backdrop.rs`) plus `edit.rs` tests.

- Probe A, probe B, and every row of the §8 table: one named test each (for example `probe_b_white_rect_label_gives_one_finding_with_fix`).
  - Fail before: probes A and B, and the `#808080` row, fail on `main`.
- `poisoned_rollback_cancels_fix_all_and_commits_nothing`: drive the primitive with a command whose inverse fails. Assert that `commit` is `Err`, `hist.can_undo() == false`, and the document is unchanged.
  - Fails before: yes.
- Perf guard (release-only, `#[ignore]` with a reason in debug): the behind-scan over a synthetic page of 72k layers stays within the existing check-designs timing budget. Use `test_timing::fastest_within` (`test_timing.rs:12`), no new budget.

## 11. Performance budgets

- `CLAUDE.md` §4.1: the audit stays off any per-frame path. It runs once per `rev`.
- The measured kit timing must not regress: total ≤ 1,661 ms and worst page ≤ 141 ms, the pre-change numbers in the PR #576 comment, release build.

## 12. Dependencies and build order

- Depends on #35 (merged).
- Before #52 (shares `contrast/*`) and #157.

## 13. Owner rulings that apply

- **1A** (owner ruling on PR #45, 2026-10-18): contrast backdrop is skip-only, with no compositing of a covering child.
  - Rule 2 adopts a single opaque, solid, fully covering sibling as the backdrop. No compositing is involved, and its descendants and every partial or translucent case stay skip-only.
  - This reading was implemented by the release session (`2707f3c`) and is listed for owner confirmation in the README's unresolved questions.
- Owner default: 1440 capture only.
