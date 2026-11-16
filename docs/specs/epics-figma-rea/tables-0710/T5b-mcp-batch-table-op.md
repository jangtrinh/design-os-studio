# T5b: MCP `table` operation in `designos_apply_batch`

| Field | Value |
|---|---|
| Issues | #1231 ("Add `batch` table op") |
| Size | 6 h |
| Tier | Claude Sonnet build, Opus review |
| Depends on | T1b merged (ops). T2 is not needed for correctness, but `designos_get_screenshot` evidence needs it. |
| Runs in parallel with | T3a, T3b, T4, T5 (only `designos-mcp` files) |
| Blocks | T6 docs line for the tool table |
| Branch | `jangtrinh/tables-t5b-mcp` |

## Goal

An agent creates and edits tables in one atomic batch, using the same T1b operations as the UI. The whole batch is one undo group, as batches already are (`crates/designos-mcp/src/tools/batch.rs:181` description).

## Op shape

There is one new op name, `table`, with an `action`. Cells are addressed by zero-based display indices, not by names.

```json
{"op":"table","as":"t","action":"create","parent":1,"x":40,"y":40,"rows":3,"cols":3}
{"op":"table","node":"@t","action":"set_cell_text","row":0,"col":1,"text":"Giá"}
{"op":"table","node":"@t","action":"insert_rows","at":1,"count":2}
{"op":"table","node":"@t","action":"delete_cols","at":2,"count":1}
{"op":"table","node":"@t","action":"move_row","from":3,"to":0}
{"op":"table","node":"@t","action":"resize_col","at":1,"size":240}
{"op":"table","node":"@t","action":"merge","row":0,"col":0,"row_span":2,"col_span":2}
{"op":"table","node":"@t","action":"unmerge","row":0,"col":0}
{"op":"table","node":"@t","action":"resize","width":600,"height":200}
```

- `create` uses the OQ-1 defaults unless `width` / `height` / `col_width` / `row_height` are given.
- `create` replies with the table id and, under `name_to_id`, cell ids keyed `"r{row}c{col}"`.
- Errors map `TableOpError` to `ToolError::batch_op_failed(idx, "table", cause)`, the pattern at `batch.rs:979-988`. The cause text names the next action.
- `dry_run=true` works as for other ops: it computes ids and changes nothing.
- The op is allowed in Design and Board documents. In Slides documents it is refused with a message that names the next action (ruling OQ-13 (a): no Slides).

## Files T5b may modify

- `crates/designos-mcp/src/tools/batch_table.rs` (new): parse and apply. `batch.rs` is 5,598 lines (FACT), so no logic goes into it.
- `crates/designos-mcp/src/tools/batch.rs`: the `ParsedOp::Table` variant and its `op_name`/`alias` arms (`:608-650`), the schema enum string (`:75`), one dispatch arm, and one example in the description string (`:181`)
- `crates/designos-mcp/src/tools/mod.rs` (one `mod` line)
- `crates/designos-mcp/tests/table_batch.rs` (new)
- The generic node tools, one refusal each for cell-breaking edits (T1 decision 8): `designos_update_node` refuses x / y / width / height / rotation on a `TableCell`; `designos_create_node` with a `Table` parent and `designos_delete_node` on a `TableCell` surface the T1 guard error with its next-action text ("use the batch `table` op"). Files: wherever those tools live (`crates/designos-mcp/src/server.rs` / `engine.rs` name them at `main@ec47da26d`; the builder cites the exact lines).
- `crates/designos-mcp/tests/snapshots/*`: the tool description snapshot that `tests/tools_snapshot.rs` checks, regenerated and diff reviewed

## Acceptance criteria (red at base, green at head)

1. One batch with `create` 3×3, then 9 `set_cell_text`, then `merge` (0,0, 2×2), then `insert_rows` at 3, gives a table with 4 rows and one merge. `designos_undo` once removes the whole table, which proves one undo group.
2. `merge` overlapping an existing merge fails the whole batch atomically: the document is unchanged and the error names op index and cause. This is red at base, where `table` is an unknown op.
3. `create` 50×50 in one batch succeeds, and the reply's `name_to_id` has 2,500 cell keys. A `--release` run takes < 500 ms (`#[ignore]` perf test). Budget INFERENCE; adjust with measured numbers in the PR if the controller agrees.
4. `designos_get_metadata` after AC-1 shows the T1 `TABLE` / `TABLE_CELL` XML with the texts.
5. `cargo test -p designos-mcp tools_snapshot` passes with the updated snapshot. The diff shows only the new op.
6. Existing batch tests (`set_slot_batch.rs`, `session.rs`, `stdio.rs`) stay green.
7. `designos_update_node` with `x` on a cell, `designos_create_node` with `parent_id` = a table, and `designos_delete_node` on a cell each return an error naming the batch `table` op, and the document is unchanged.

## Test seam

MCP tests through the support harness in `crates/designos-mcp/tests/support`, as `set_slot_batch.rs` does.

## Non-goals

- `apply_spec` / NodeSpec `kind: "table"` (not requested; possible follow-up).
- `designos_create_node kind=table`.
- Code generation for tables (OQ-10).

## Gates

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo fmt --all -- --check
cargo clippy -p designos-mcp --all-targets -- -D warnings
cargo test -p designos-mcp
```

## Visual evidence

- A terminal capture of the AC-1 batch request and reply.
- The `designos_get_screenshot` PNG of the resulting table (after T2).

## Risks and rollback

- Risk: snapshot churn conflicts with other MCP PRs. Mitigation: rebase and regenerate last.
- Rollback: revert. The `table` op becomes unknown again, and tables stay editable in the UI.
