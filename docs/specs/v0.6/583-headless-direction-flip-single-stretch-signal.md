# #583 A headless direction change before the first solve leaves one stretch signal per child

| Field | Value |
|---|---|
| Issue | #583 (follow-up of the #548 review) |
| Size / priority | S / P2 |
| Depends on | none |
| Unblocks | none |
| Touches | new `crates/designos-doc/src/writer_command.rs` (the writer-boundary function shared with #582), `crates/designos-doc/src/live_patch/replay.rs` (call site, `:128-167`), `crates/designos-mcp/src/engine.rs` (`Engine::exec`, `:414-419`), `crates/designos-doc/src/layout_solver_types.rs` and `layout_apply.rs` (read only), tests in `crates/designos-doc` and `crates/designos-doc/src/live_patch/tests.rs` |

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

Where the rewrite lives, and why. `Command::apply` cannot host it: undo, redo, `Tx::cmd` (`designos-app/src/edit.rs:88-93`) and the main-component sync (`components.rs` builds
`Command::Update` for instance frames from the main's props) all run through `Command::apply`, so a rewrite inside it would run again on every undo, redo and sync, and an `Update` whose inverse
restores only the parent would leave rewritten children behind. #582 reached the same conclusion for the slot mark. Both therefore live at the **writer boundary**: one pure function in
`designos-doc`, `writer_command(doc: &Document, cmd: Command) -> Command`, that the headless writers call before they apply a command (MCP `Engine::exec`, live-patch replay). #583 adds its
step to it; #582 adds the slot-mark step. Whichever task lands first creates the file; the other adds its step.

1. **Direction step.** For a `Command::Update { node }` whose target frame's `auto_layout` changes horizontality (`AutoLayout::primary_is_horizontal` differs between the stored node and `node`), the
   function returns `WriterCommand { cmd: Command::Batch([Command::Update { parent }, Command::Update { child' } ...]), parts: [the same updates un-batched] }` (type shared with #582): the parent's update first, then one explicit `Update` per direct child that
   `apply_parent_direction_change(old_horiz, new_horiz)` changes, for the children whose `stretched.last_parent_horiz` is `None` (children with `Some(..)` are handled by the solver as today, so
   nothing is applied twice). The child updates are ordinary full-node updates, so `Command::apply`, its inverse, undo, redo and sync see plain commands. The parts are independent (each touches a different node and none reads another's changed fields), so `Engine::exec` records live ops with `live_ops_for_independent(&doc, &parts)` and never takes the document clone that `live_ops_for_command` makes for a `Batch` (`live_patch/record.rs:42-48`); for 10,000 children this is 10,000 per-node `field_changes` calls and no scene-graph copy.
2. **Same result as the inspector.** For a child with Fill on the old counter axis and `AlignSelf::Stretch`, Stretch is cleared; Fill stays; `last_parent_horiz` ends consistent with the new
   direction (`layout_solver_types.rs:134-146`).
3. A flip between two directions with the same horizontality (Horizontal to Grid) returns the command unchanged.
4. A node that is not a frame, a frame gaining or losing auto layout (no direction before or after), or a command that is not an `Update` returns unchanged.
5. Children without the duplicate pair are never touched (no child `Update` is emitted for them).
6. The main-component sync and the Studio inspector do not call `writer_command`: the inspector keeps its own rewrite (`ops.rs:212-228`) and sync builds its commands itself, so neither is affected.

## 4. Data model / schema changes

None. `stretched.last_parent_horiz` keeps its meaning; `writer_command` is a new public function in `designos-doc`.

## 5. Contracts

`writer_command` is the only new API. `Command::apply` and `History` are unchanged, so every existing inverse is still an inverse of what it undoes. `Engine::exec`
(`designos-mcp/src/engine.rs:414-419`) wraps each command before `live_ops_for` and `history.exec`, so the recorded live ops carry the child updates; the replay path wraps the
`Command::Update` it builds (`live_patch/replay.rs:167`). The one production `History::peek_undo` caller, `designos_undo` (`engine.rs:379`), is unaffected because inverses stay plain.

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
6. Undo then redo: plain inverses, so the pair is restored by undo and cleared again by redo, exactly as the forward edit did.
7. The main component's direction is flipped and an instance of it syncs: the sync does not call the boundary; the instance's children follow the main's already-normalised children.

## 9. Non-goals

- Adding a direction tool to MCP (E15-4 batch ops own the write surface).
- Changing the solver or `stretched` bookkeeping.
- Rewriting children of frames that were not changed.

## 10. Test plan (red first)

Seams: `designos-doc` unit test with a hand-built document; one replay test through `LiveOp::Update`.

| Case | Steps | Expected (fails before) |
|---|---|---|
| `flip_before_first_solve_leaves_one_signal` | horizontal frame, child `vertical = Fill` + `align_self = Stretch`, `stretched.last_parent_horiz == None`; `writer_command` on an `Update` that sets direction Vertical, apply, `relayout_subtree` | child `align_self == Auto`, `vertical == Fill` |
| `flip_back_is_symmetric` | another flip to Horizontal | one signal again, none lost |
| `undo_restores_pair` | undo the flip | Fill + Stretch both back, direction Horizontal |
| `redo_after_undo_matches_the_first_flip` | undo then redo | same final state as the first apply, history depth unchanged |
| `solved_child_not_double_rewritten` | solve once, then flip | same final state as the unsolved case |
| `live_patch_update_flip` | replay a `LiveOp::Update { set: kind with direction }` | same final state as the direct flip |
| `engine_exec_does_not_clone_the_document_for_a_flip` | flip a frame with 10,000 children on a 100,000-node document through `Engine::exec` | live ops equal the batch path's; no document clone; under the budget |
| `engine_exec_applies_the_boundary` | `Engine::exec` with a direction `Update` | recorded live ops include the child updates; replaying them gives the same document |
| `sync_after_a_main_direction_flip_is_unaffected` | flip the main component's frame through the boundary, then `sync_instances` | instance children equal the main's; no extra rewrite; sync's own batch unchanged |
| `same_horizontality_untouched` | Horizontal to Grid | command returned unchanged |
| `children_without_the_pair_emit_no_update` | plain children | the batch holds only the parent update |

Run: `cargo test -p designos-doc`. Evidence: red/green terminal capture; one screenshot of the child
width after a flip applied through a live patch (`designos --screenshot`).

## 11. Performance budgets

O(children of the changed frame) per direction change, with no document clone while recording live ops (rule 3.1); nothing on writes that do not change the direction. Budget: a flip of a frame with 10,000 children
through `Engine::exec` stays under 100 ms in a release build (record the number).

## 12. Dependencies and build order

Independent, but it shares `writer_command` with #582: whichever merges first creates `crates/designos-doc/src/writer_command.rs`, the other adds its step (rebase, no hand merge).

## 13. Owner rulings

- #532 ruling carried through #548: one stretch signal per child across direction flips.

## Could not ground

- A user-reachable MCP call that flips a direction: none exists on `origin/main@01d48f2`. The issue's
  "MCP direction flip" is covered here through the headless seams that do exist (live-patch `Update`
  and direct `Command::Update`). If a direction tool arrives with E15-4, its test reuses
  `flip_before_first_solve_leaves_one_signal`.
