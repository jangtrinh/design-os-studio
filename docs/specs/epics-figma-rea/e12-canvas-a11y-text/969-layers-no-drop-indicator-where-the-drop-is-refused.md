# #969 Layers panel: no drop line on a row that refuses the drop

| Field | Value |
|---|---|
| Issue | #969 (epic #124; bug, P3; backlog; moved v0.6.2 → v0.6.3) |
| Slice | S1, wave 1, leg A, after #951 (same builder, same file) |
| Size | 3 builder hours |
| Tier | Argon (pure predicate plus sim tests; machine-checkable) |
| Depends on | #951 (owns `panels/layers.rs` first). Owner answer **Q5** (feedback on a refused drop) |
| Touches | `crates/designos-app/src/ops.rs` (new pure predicate next to `move_to_parent` `:1996`; both branches call it), `crates/designos-app/src/panels/layers.rs` (drop bookkeeping `:477-481`, indicator `:742-758`, new per-drag cache on the Layers state), `crates/designos-app/src/shell/tests/layers_drop_refusal.rs` |

**Code baseline:** `origin/main@d22dad970`. Re-anchor against HEAD first.

## User-visible outcome

You drag a layer over a place it cannot go. For example, an instance of main A onto main B when A already holds an instance of B. The blue drop line or box does not appear there, so the panel never promises a drop that will do nothing.

## Current state (FACT)

1. While dragging, `app.layers.drop` is computed from the pointer row only (`drop_for`, `layers.rs:479-481`). The indicator draws for any `Some(drop)` (`:742-758`).
2. The refusal happens only at the drop, inside `App::move_to_parent` (`ops.rs:1996`). Both branches filter each dragged id with the same four conditions:
   - the same-parent branch at `:2016-2022`;
   - the cross-parent branch at `:2052-2061`.

   The four conditions:
   - `id != new_parent`;
   - `!doc.is_ancestor(id, new_parent)`;
   - `!reparent_would_hold_itself(&doc, &[id], new_parent)` (`designos-doc/src/components.rs:629`);
   - `slot_move_allowed(&[id], new_parent)` (`ops.rs:669`).
3. `slot_move_allowed` takes `&mut self` and **shows a toast** on refusal (`ops.rs:669-680`, through `report_slot_error`). It cannot be called every frame from the indicator. The pure part is `slot_edit_error(&doc, …)` (called at `:678`).
4. The issue asks that the check stay cheap. Walking the dragged subtree and its nested mains on every frame is too slow, so it must be cached per drag.

## Scope

1. Add a pure predicate on the document, used by both `move_to_parent` branches and by the Layers panel: `fn reparent_blocked(doc: &Document, id: NodeId, new_parent: NodeId) -> Option<RefuseReason>`.
   - It covers the four conditions above and calls `slot_edit_error` directly, without the toast.
   - `move_to_parent` keeps its current toast for a slot refusal by mapping the reason, so its behaviour does not change.
2. Resolve the target parent with `resolve_drop` (`layers.rs:134`). A drop is **refused** when every dragged id is blocked for that parent. If at least one id can move, `move_to_parent` moves it, so the line still shows.
3. Cache per drag: key `(drag ids, target parent)` → `refused: bool`, kept on the Layers state and cleared when the drag ends (`:764-775`) and when `doc_rev` changes. Each hover over a new target parent computes the predicate at most once.
4. A refused target sets `app.layers.drop = None` for drawing. The release then drops nothing, which is the current visible result. The feedback in rule 5 depends on Q5.
5. Q5 default (a): no line and no other feedback. With (b) the panel sets `CursorIcon::NotAllowed` while over a refused target. With (c), add to (b) a toast on release naming the reason in about 8 words, for example "Can't put a main inside itself".

## Acceptance criteria

- **AC-1** Setup: main A holds an instance of main B, and an instance of A is dragged over B's row (Inside, Before and After zones). No drop indicator is painted. Assert this through a recorded rect or a test hook on the indicator. Red on the baseline.
- **AC-2** The same drag over a row that accepts it still shows the line, and the drop moves the layer (regression).
- **AC-3** The indicator test and `move_to_parent` call the same predicate. A test proves it by mutation: a table of 6 cases (cycle, self, ancestor, slot max, slot min, allowed) gives the same answer from the predicate as from a real `move_to_parent` (moved or not).
- **AC-4** Over one target the predicate runs once per drag, not once per frame. Use a counter in tests: 30 frames hovering one row means 1 evaluation.
- **AC-5** Hovering a slot-refused target shows no toast. Only a real drop can toast.
- **AC-6** Q5 option-specific: (a) no cursor change; (b) `NotAllowed` cursor over a refused row; (c) (b) plus one toast on release.

## Test steps

1. `shell/tests/layers_drop_refusal.rs`: build mains A and B with the cross-instance, drag by `Sim` raw events over B's row, and step frames. Assert the indicator is absent and the tree is unchanged after release.
2. Run the table test for AC-3 in `ops.rs` tests, next to `move_to_parent_keeps_world_position` (`ops.rs:2922`).
3. `cargo test -p designos-app --lib layers ops::`

## Edge cases

- A multi-selection where some ids are refused and some are allowed: the line shows and only the allowed ids move (current drop behaviour).
- An agent edit changes the document mid-drag: `doc_rev` clears the cache.
- Filtering mode (`start_row_drag` refuses drags while filtering, `:414`): unchanged.
- A 160k-node document: the predicate runs on hover changes only (AC-4).

## Non-goals

- Changing which drops are refused.
- Canvas drag-into-frame refusal.

## Parity facts

- Figma (desktop, layers panel) shows no insertion line where a component cannot be nested. This is UNVERIFIED in this run (no REA capture). Option (a) matches the issue text.

## Decisions recorded

- Cache key `(Vec<NodeId>, NodeId)` plus `doc_rev`. No per-frame subtree walk.
- The pure predicate lives in the app crate next to `move_to_parent`, because `slot_edit_error` is an app concern. If the builder finds `slot_edit_error` in `designos-doc`, the predicate goes there instead.

## Rules

- Open one PR that references #969. Use "Fixes #969" only when every acceptance criterion is met. Do not merge, tag or close.
- Re-anchor every cited line against HEAD before writing code, and report drift first.
- Work on a branch or worktree from `main`, never on `main`.
- Merge with squash only, after `gh pr update-branch <n> --rebase`.
- Blocking CI is `Test (macos-14)`, `Clippy` and `Rustfmt`. Ubuntu and Windows CI are advisory (CLAUDE.md, Git Workflow).
- Test first: each new test is red on the base commit, and that red output goes in the PR body.
- Visual evidence goes in the PR: an app capture, or a terminal render for test-only proof.
- Keep plan ids, issue numbers and finding codes out of code comments and test names. Use conventional commits with no AI trailers.
