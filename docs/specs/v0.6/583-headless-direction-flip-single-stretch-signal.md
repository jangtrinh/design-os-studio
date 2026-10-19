# #583 A headless direction change before the first solve leaves one stretch signal per child

| Field | Value |
|---|---|
| Issue | #583 (follow-up of the #548 review) |
| Size / priority | S / P2 |
| Depends on | none |
| Unblocks | none |
| Touches | `crates/designos-doc/src/history.rs` (`Command::Update` apply, `:118-124`), `crates/designos-doc/src/layout_solver_types.rs` (`apply_parent_direction_change`, read only), `crates/designos-doc/src/layout_apply.rs` (read only), tests in `crates/designos-doc` and `crates/designos-doc/src/live_patch/tests.rs` |

Code baseline: `origin/main@01d48f2` (0.5.0). The issue cites `layout_apply.rs:100`; the check now sits at
`layout_apply.rs:150-155`.

## 1. Summary and user story

As an agent or script that flips an auto-layout frame between horizontal and vertical without Studio
having solved it yet, each child ends with exactly one stretch signal, the way the Studio inspector
leaves it, so the child does not stretch on both axes.

## 2. Current state (FACT)

- A child that fills the parent's counter axis can carry two signals for the same stretch:
  `Sizing::Fill` on that axis and `AlignSelf::Stretch` (imported `.fig` STRETCH children store both;
  `effective_sizing` treats them as one, `designos-doc/src/layout_solver_types.rs:118-125`).
- On a direction flip the rewrite is `LayoutChild::apply_parent_direction_change(old_horiz, new_horiz)`
  (`layout_solver_types.rs:134-146`): when the old counter axis was Fill and `AlignSelf::Stretch` is also
  set, it clears Stretch, because after the flip the Fill sits on the new primary axis and Stretch would
  stretch the new counter axis too.
- Callers:
  - the inspector path knows both directions: `App::edit_nodes` computes `old_horiz` / `new_horiz`
    around the edit (`designos-app/src/ops.rs:212-228`);
  - the solver paths use the stored `stretched.last_parent_horiz`:
    `Document::solve_auto_layout_node` (`designos-doc/src/layout_apply.rs:150-155`) and the app's
    `layout_self` (`designos-app/src/layout.rs:73-76`), both guarded by
    `if let Some(old_horiz) = ...last_parent_horiz`.
- `last_parent_horiz` is written only after a solve (`record_stretched`,
  `layout_solver_types.rs:195-203`) or by a previous flip. A frame that was never solved has `None`, so
  a direction write before the first solve skips the rewrite and the child keeps Fill + Stretch: after
  a horizontal-to-vertical flip its vertical Fill is now the primary axis and Stretch stretches the
  horizontal axis too ("stretches twice").
- Headless writers of a direction: a live-patch `LiveOp::Update` whose `set` carries the frame's
  `kind` (replay merges fields into a `Command::Update`, `designos-doc/src/live_patch/replay.rs:128-167`),
  and any script or test that edits `FrameProps.auto_layout.direction` and calls `relayout_subtree`.
  MCP has no direction tool: `UpdateArgs` has no layout fields (`designos-mcp/src/engine.rs:627-637`).

## 3. Behaviour rules

1. `Command::Update::apply` (`history.rs:118-124`) compares the node's previous and new
   `auto_layout` horizontality (`AutoLayout::primary_is_horizontal`). When it differs, it calls
   `apply_parent_direction_change(old_horiz, new_horiz)` on every direct child whose
   `stretched.last_parent_horiz` is `None` (children with `Some(..)` are handled by the solver as
   today, so nothing is applied twice).
2. The inverse of that update restores the child rewrites as well: it is a `Command::Batch` of the
   parent's `Update` and one `Update` per rewritten child (the current inverse restores only the parent,
   which would leave the children changed after an undo).
3. The result is the inspector's result: for a child with Fill on the old counter axis and Stretch,
   Stretch is cleared; Fill stays; `last_parent_horiz` ends consistent with the new direction.
4. A flip between two directions with the same horizontality (Horizontal to Grid) changes nothing.
5. A node that is not a frame, or a frame gaining or losing auto layout (no direction before or
   after), changes nothing.
6. Children without the duplicate pair are never touched.

## 4. Data model / schema changes

None. `stretched.last_parent_horiz` keeps its meaning.

## 5. Contracts

No API change. `Command::Update`'s inverse can now be a `Batch` when a direction flip rewrote children;
`History` already stores and applies `Batch` inverses (`history.rs:141-149`). Check callers of
`History::peek_undo` (`history.rs:230-236`) for any that assume the inverse of an `Update` is an `Update`.

## 6. File-format mapping

None. The rewrite changes `layout_child.align_self`; both fields already serialize.

## 7. UI

None.

## 8. Edge cases

1. Imported pair (Fill + Stretch), never solved, horizontal to vertical: Stretch cleared (acceptance).
2. Same frame after one solve (`last_parent_horiz == Some`): unchanged path, no double rewrite.
3. Vertical to horizontal: symmetric.
4. Child with Stretch only (no Fill): untouched; Stretch keeps stretching the new counter axis, which is
   the same meaning as before the flip.
5. Many children (10 000): one pass over the children list of the changed frame; no tree walk.
6. Undo then redo: the pair is restored, then cleared again.

## 9. Non-goals

- Adding a direction tool to MCP (E15-4 batch ops own the write surface).
- Changing the solver or `stretched` bookkeeping.
- Rewriting children of frames that were not changed.

## 10. Test plan (red first)

Seams: `designos-doc` unit test with a hand-built document; one replay test through `LiveOp::Update`.

| Case | Steps | Expected (fails before) |
|---|---|---|
| `flip_before_first_solve_leaves_one_signal` | horizontal frame, child `vertical = Fill` + `align_self = Stretch`, `stretched.last_parent_horiz == None`; `Command::Update` sets direction Vertical; `relayout_subtree` | child `align_self == Auto`, `vertical == Fill` |
| `flip_back_is_symmetric` | continue with another flip to Horizontal | one signal again, none lost |
| `undo_restores_pair` | undo the flip | Fill + Stretch both back, direction Horizontal |
| `solved_child_not_double_rewritten` | solve once, then flip | same final state as the unsolved case |
| `live_patch_update_flip` | replay a `LiveOp::Update { set: kind with direction }` | same final state as the direct `Command::Update` |
| `same_horizontality_untouched` | Horizontal to Grid | child unchanged |

Run: `cargo test -p designos-doc`. Evidence: red/green terminal capture; one screenshot of the child
width after a flip applied through a live patch (`designos --screenshot`).

## 11. Performance budgets

O(children of the changed frame) per direction change; nothing on writes that do not change the
direction.

## 12. Dependencies and build order

Independent. Edits `history.rs`; serialize with #582 only if it also edits `Command::apply` (it does
not: #582 adds a wrapper function).

## 13. Owner rulings

- #532 ruling carried through #548: one stretch signal per child across direction flips.

## Could not ground

- A user-reachable MCP call that flips a direction: none exists on `origin/main@01d48f2`. The issue's
  "MCP direction flip" is covered here through the headless seams that do exist (live-patch `Update`
  and direct `Command::Update`). If a direction tool arrives with E15-4, its test reuses
  `flip_before_first_solve_leaves_one_signal`.
