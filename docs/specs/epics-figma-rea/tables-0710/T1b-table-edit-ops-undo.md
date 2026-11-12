# T1b: table edit operations as single undo steps

| Field | Value |
|---|---|
| Issues | #331 AC-2 (doc half), #1231 ("undo of row delete") |
| Size | 6 h |
| Tier | Claude Sonnet build, Opus review |
| Depends on | T1 merged |
| Runs in parallel with | T2, T3a (disjoint files) |
| Blocks | T4, T5, T5b |
| Branch | `jangtrinh/tables-t1b-ops` |

## Goal

There is one pure, tested set of table operations in `designos-doc`, shared by the UI (T4, T5) and MCP (T5b), so the rules live in one place (DRY).

Each operation:
- returns one `Command::Batch` (`crates/designos-doc/src/history.rs:76`) that includes the `table::relayout` updates;
- executes as one undo step through `History::exec_user` / `exec_agent` (`history.rs:374-381`);
- leaves every table it touches whole at the end of the step, because the T1 guard (decision 8) checks positions once per undo step and rolls back otherwise. Order inside the batch is free. A refused op is a bug in the op, never something the caller works around.

## Operations (`crates/designos-doc/src/table_ops.rs`, new)

Every function has the shape `fn op(doc: &Document, table: NodeId, …) -> Result<Command, TableOpError>` and only builds the command. The caller executes it.

| Op | Behaviour |
|---|---|
| `create_table(doc, parent, at, rows, cols, style)` | `Insert` of the table plus `rows×cols` cells, with fresh `TrackId`s from `next_track` |
| `insert_rows(table, at_index, count)` / `insert_cols` | New tracks of the default size (owner OQ-1). New empty cells, which inherit the neighbour track's cell fill (INFERENCE from Figma behaviour; flag in PR). A merge that spans the insertion point grows its span. |
| `delete_rows(table, ids)` / `delete_cols` | Removes the tracks and their cells; spans shrink. Deleting the last row or column deletes the table. |
| `move_row(table, id, to_index)` / `move_col` | Reorders the tracks. If the move would split a merge, the op is refused (`TableOpError::SplitsMerge`). |
| `resize_row(table, id, size)` / `resize_col` | Size clamped to the minimum (OQ-1) |
| `resize_table(table, new_size)` | Scales every track proportionally (OQ-7 recommended); text is not scaled |
| `merge(table, rect: (row_from, col_from, row_to, col_to))` | Refused if it partially overlaps an existing merge. The covered cells are removed. Their text is appended to the anchor, one line per covered cell (ruling OQ-5 (a), 2026-11-07). |
| `unmerge(table, anchor_cell)` | Removes the merge entry; the covered positions get fresh empty cells |
| `set_cell_text(table, row_index, col_index, text)` | Text update on the anchor at that position (MCP needs this; the UI uses the text session) |
| `next_cell(table, cell, backwards) -> Option<NodeId>` | Row-major order over anchors; a pure query for Tab (T4) |

Errors (`TableOpError`):
- `NotATable`
- `IndexOutOfRange`
- `SplitsMerge`
- `OverlapsMerge`
- `TooLarge` (rows or cols > 500; the limit is INFERENCE, a guard against runaway agent calls; flag in PR)

Each error carries a human message that names the next action (Law 2), for example "Unmerge the cells in rows 2-3 first".

## Files T1b may modify

- `crates/designos-doc/src/table_ops.rs` (new)
- `crates/designos-doc/src/table_ops_tests.rs` (new)
- `crates/designos-doc/src/lib.rs` (one `mod` line and re-export)

## Acceptance criteria (red at base, green at head)

1. For every op in the table, a test applies the command with `History::exec_user` and then `undo()`. The test asserts that the document equals the before-state (`Document: PartialEq`, or a node-by-node compare if `Document` is not `PartialEq`), then `redo()` equals the after-state. This makes one op exactly one undo step: `undo_label` changes once.
2. Undo of row delete, the #1231 edge case: delete row 2 of a 3×3 table whose cells hold text, then one undo. The texts, track ids, order and sizes are back, and the cells have the same NodeIds as before.
3. Merge then unmerge on a 3×3 table:
   - `merge((0,0),(1,1))` leaves 6 cell nodes, and the anchor text follows OQ-5.
   - `unmerge` leaves 9 cell nodes, and the three new cells are empty.
4. `move_row` across a merge returns `Err(SplitsMerge)` and leaves the document untouched. `insert_rows` inside a merged block grows `row_span` by `count`.
5. `create_table(…, 50, 50, …)` builds 2,500 cells. The command applies in < 150 ms in a `--release` test on the controller's M4 Max (`#[ignore]` perf test, run in the gates list). Undo removes all 2,501 nodes in one step.
6. `next_cell` walks the anchors of a 3×3 table with a 2×2 merge in order: anchor, (0,2), (1,2), (2,0), (2,1), (2,2). It returns `None` after the last cell. What Tab does at the end is T4's call (OQ-4).

## Test seam

Pure doc tests in `table_ops_tests.rs`, using `History` directly.

## Non-goals

- UI.
- MCP.
- Clipboard copy and paste of rows (not requested).
- Sorting and formulas.

## Gates

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo fmt --all -- --check
cargo clippy -p designos-doc --all-targets -- -D warnings
cargo test -p designos-doc
cargo test -p designos-doc --release -- --ignored table_ops_perf
```

## Visual evidence

A terminal capture of the test run, including the AC-5 timing line.

## Risks and rollback

- Merge-split rules are the easiest place to break invariants. Mitigation: a randomized test applies 1,000 random ops on a 6×6 table with a fixed seed, and after each op asserts the T1 invariants (`validate_and_repair` reports zero fixes).
- Rollback: revert; nothing else depends on it until T4, T5 and T5b merge.

> Amended 2026-11-07 (R-T1-2): call `check_table_positions` at the commit points outside `History` (`Tx::cmd`, live-patch replay) and test a rollback through them.

> Amended 2026-11-07 (R-T1-9): test that an undo refused by the position check through the auto-layout path leaves the layout pass undone too.
