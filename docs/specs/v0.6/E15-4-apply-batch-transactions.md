# E15-4 Apply batched agent writes as one transaction: dry-run, one undo group, created ids and a name-to-id map

| Field | Value |
|---|---|
| Epic / pack | E15 / v0.6 AI design |
| Size / priority | L / P0 |
| Depends on | none |
| Unblocks | E15-5, E15-6, E15-7, E15-8, E15-12, RD-3 |
| Touches | new `crates/designos-mcp/src/tools/mod.rs`, new `crates/designos-mcp/src/tools/batch.rs`, `crates/designos-mcp/src/engine.rs` (dispatch only), `crates/designos-mcp/src/server.rs` (mutating list), `crates/designos-doc/src/live_patch/mod.rs` (`label`) |

## 1. Summary and user story

As an agent, I send one `designos_apply_batch` with several operations (build a section, recolour
two nodes, delete a placeholder). Either all of it lands as one undo step, or nothing changes and I
get the failing op and why. With `dry_run: true` I see the ids, bounds and issues the commit would
produce, without committing.

## 2. Current state (FACT)

- Every tool call is its own undo step: `Engine::exec` pushes one command per call
  (`crates/designos-mcp/src/engine.rs:413-418`); `History` is a linear stack of inverse commands with
  no labels (`crates/designos-doc/src/history.rs:205-226`). `Command::Batch(Vec<Command>)` exists
  (`history.rs:74`).
- `designos_apply_spec` already builds on a trial clone and commits one `Insert`
  (`engine.rs:262-268`); create/update/delete have no trial (`engine.rs:129-140`, `439-539`).
- No tool returns a name-to-id map; `apply_spec` returns only `root_node_id` and the root node
  (`engine.rs:269`).
- The session marks a fixed list of tools as mutating for live-patch publishing and rollback
  (`crates/designos-mcp/src/server.rs:508-546`). A failed publish rolls the edit back
  (`server.rs:271-285`, `531-534`).
- One `LivePatch { v, base, ops }` line is replayed by the editor as one transaction
  (`crates/designos-doc/src/live_patch/mod.rs:27-35`, `live_patch/replay.rs:75-76`). The editor may
  remap agent ids that collide with ids it created meanwhile (`replay.rs:16-17`).
- `engine.rs` is 1,194 lines; `crates/designos-mcp/src/` has no `tools/` module yet.

## 3. Behaviour rules

### 3.1 Operations

`ops` is an ordered list (1..=200 entries). Each op has `op` plus fields:

| `op` | Fields | Engine effect |
|---|---|---|
| `apply_spec` | `parent`, `spec`, `index?` | E15-1/2/3 spec build under `parent` |
| `insert` | `parent`, `kind`, `name?`, `text?`, `x,y,width,height`, `index?` | same validation as `designos_create_node` (`engine.rs:439-486`) |
| `update` | `node`, any `designos_update_node` field | same validation as `engine.rs:488-539` |
| `move` | `node`, `parent`, `index?` | `Command::Reparent`; refuses page roots and cycles |
| `delete` | `node` | `Command::Remove`; refuses page roots |
| `set_variables` | `upsert_collections[]`, `upsert_variables[]`, `delete_variables[]` | one `Command::SetVariables` |
| `set_style` | `upsert[]`, `delete[]`, `apply:[{node, style}]` | `Command::SetStyles` + style links |

`parent` / `node` accept a numeric id or a reference string:
- `"@<as>"`: the root created by an earlier op that declared `"as": "<as>"` (unique per batch);
- `"@<as>/<name path>"`: a named descendant inside that root, same path rules as E15-1 3.3.

A reference to a later op or an unknown `as` fails `UNKNOWN_REF` before anything runs.

### 3.2 Transaction

1. Parse and validate every op's arguments (types, bounds, references). Any failure: reply with the op
   index and nothing runs.
2. Clone the document once into a trial (`Document::clone`, as `engine.rs:262` does today) and run the
   ops in order on the trial, collecting the `Command` each op executes and settling layout after each
   op exactly as the single tools do (`relayout`, `engine.rs:395-398`).
3. Any op error aborts: the trial is dropped, the real document and history are untouched. The reply
   names `failed_op` (index), its `op` and the error (E15-5 envelope).
4. Dry run: stop here and return the reply computed from the trial (ids, map, bounds, issues block per
   E15-12). Nothing is committed and nothing is published.
5. Commit: execute `Command::Batch(collected)` on the real document through `History::exec` (one undo
   entry), relayout the touched roots, publish one `LivePatch` line labelled
   `Agent: <run label>` (3.4). Determinism requirement: the committed document hash equals the trial
   hash (debug assertion plus a test), so dry-run ids equal commit ids (ids come from
   `try_alloc_id`, derived from the node set, `engine.rs:471-473`).
6. If publishing to the editor fails, the existing rollback (`server.rs:531-534`) undoes the whole
   batch, one step.

### 3.3 Reply

- `created`: ids of every op root, in op order, plus `created_count` (all nodes including
  descendants).
- `name_to_id`: every named node the batch created, keyed by `"<as or root name>/<descendant path>"`.
  Duplicate sibling names get `[n]` suffixes as in E15-1.
- `changed`: per-kind counts `{inserted, updated, moved, deleted, variables, styles}`.
- `touched_roots`: for each op, the topmost node it changed (for delete: the former parent), with its
  page-space bounds `{x, y, width, height}` after the batch (deleted nodes: before). Union bounds of
  all touched roots: `touched_bounds`.
- `undo_label`, `dry_run`, and the issues block (E15-12) when enabled.

### 3.4 Undo group label

`LivePatch` gains `label: Option<String>` (skipped when `None`, so old editors and logs parse). The
editor uses it as the history entry name in its History panel. Label = `Agent: <run>` where `<run>` is
`DESIGNOS_AGENT_RUN_ID` from the agent runtime environment when set, else `designos-mcp`. The MCP-side
`History` stays label-free (its entries are internal).

### 3.5 Module layout

`crates/designos-mcp/src/tools/mod.rs` declares `batch`. `Engine::call` adds one match arm
`"designos_apply_batch" => tools::batch::apply(self, args)`; all logic lives in `batch.rs`. The
single-node logic in `create`/`update` is factored into functions in `tools/` that both the single
tools and the batch call (no duplicated validation).

## 4. Data model / schema changes

- `LivePatch.label: Option<String>` (`#[serde(default, skip_serializing_if = "Option::is_none")]`).
- No document format change.

## 5. Contracts

Tool `designos_apply_batch`, annotations `readOnlyHint:false, destructiveHint:true,
idempotentHint:false` (true idempotency comes from E15-6 keys).

Input schema (JSON Schema, `additionalProperties:false` at every level):

```json
{"type":"object","required":["ops"],"properties":{
  "ops":{"type":"array","minItems":1,"maxItems":200,"items":{"type":"object","required":["op"],
    "properties":{"op":{"enum":["apply_spec","insert","update","move","delete","set_variables","set_style"]},
                  "as":{"type":"string","pattern":"^[A-Za-z0-9_-]{1,64}$"}}}},
  "dry_run":{"type":"boolean","default":false},
  "idempotency_key":{"type":"string","maxLength":128},
  "diff_image":{"type":"boolean","default":false}}}
```

(`idempotency_key` is E15-6, `diff_image` is E15-8; both are declared here so the schema is defined
once.)

Example:

```json
{"ops":[
  {"op":"apply_spec","as":"hero","parent":1,"spec":{"kind":"frame","name":"Hero","layout":{"direction":"column","gap":16},
    "children":[{"kind":"text","name":"Title","text":"Plan your week"}]}},
  {"op":"update","node":"@hero/Title","opacity":0.9},
  {"op":"delete","node":42}],
 "dry_run":true}
```

Output:

```json
{"dry_run":true,"created":[901],"created_count":2,
 "name_to_id":{"hero":901,"hero/Title":902},
 "changed":{"inserted":2,"updated":1,"moved":0,"deleted":1,"variables":0,"styles":0},
 "touched_roots":[{"op":0,"node_id":901,"bounds":{"x":0,"y":0,"width":200,"height":56}},
                  {"op":1,"node_id":902,"bounds":{"x":0,"y":0,"width":118,"height":20}},
                  {"op":2,"node_id":1,"bounds":{"x":0,"y":0,"width":1440,"height":900}}],
 "touched_bounds":{"x":0,"y":0,"width":1440,"height":900},
 "undo_label":"Agent: run-7","issues":[],"issues_total":0}
```

Errors (E15-5 envelope): `BATCH_OP_FAILED` wraps the inner error with `failed_op` and `op`;
`UNKNOWN_REF`, `DUPLICATE_AS`, `BATCH_TOO_LARGE` (over 200 ops or over 20,000 created nodes). All are
`safe_to_retry: true` (nothing written). An editor-publish failure keeps today's message and is
`safe_to_retry: true` because the rollback restored the state (verified by hash in a test).

## 6. File-format mapping

None. The edit log line format gains the optional `label`.

## 7. UI

Studio's History panel shows the batch as one entry named `Agent: <run>`; undo and redo move it as
one step. No other surface.

## 8. Edge cases

1. An op deletes a node created by an earlier op in the same batch: allowed; it is absent from
   `name_to_id` and counted in `deleted`.
2. `move` into its own descendant: `MOVE_CYCLE`, batch aborts.
3. Two ops update the same node: applied in order; one `Command::Update` per op inside the batch.
4. Live session where the editor remaps a colliding id on replay (`replay.rs:16-17`): the reply holds
   the session ids; the agent sees the editor's ids after the next canvas reload, as for single tools
   today. Documented in the tool description.
5. Empty effective change (all updates equal current values): commit is skipped, `changed` all zero,
   no undo entry, no patch line (mirrors `engine.rs:531-533`).
6. Batch over the edit-log line limit (`MAX_LIVE_PATCH_LINE_BYTES`, `server.rs:663-675`): commit is
   rolled back with the existing "split it into smaller calls" error, `safe_to_retry: true`.

## 9. Non-goals

- Partial commit or savepoints.
- Undo inside the MCP session for individual ops of a batch.
- Scripting (T7.13, later pack).

## 10. Test plan

Seam: `cargo test -p designos-mcp` (engine-level tests in `tools/batch.rs`, plus one stdio test in
`crates/designos-mcp/tests/stdio.rs`).

| Case | Input | Expected |
|---|---|---|
| `failed_last_op_leaves_document_identical` | 3 ops, op 2 `update` with `opacity: 2` | `failed_op == 2`; `Document::to_json()` before == after, byte for byte (test-only use; never on a UI thread); history depth unchanged |
| `batch_is_one_undo_step` | 3 valid ops then `designos_undo`, then `designos_redo` | after undo the nodes, variables and styles equal the pre-batch ones (relayout writes excepted, per the `History::undo` contract, `history.rs:218-225`); after redo equal the post-batch ones; undo depth +1 only |
| `dry_run_matches_commit` | same batch with `dry_run:true` then `false` | identical `created`, `name_to_id`, `touched_roots`; document unchanged after dry run |
| `name_map_resolves_every_named_node` | spec with 12 named descendants | every name path in `name_to_id` resolves via `designos_get_node` to a node with that name |
| `refs_to_earlier_ops` | op 1 uses `@hero/Title` | update applied to the created text |
| `unknown_ref_rejected_before_run` | `@nope` | `UNKNOWN_REF`, nothing ran |
| `one_patch_line_with_label` | live session dir in a temp folder | exactly one new `doc-patch.jsonl` line with `"label":"Agent: ..."` |
| `trial_clone_budget_on_large_synthetic` | synthetic documents of 100,000, 112,474 and 1,200,000 nodes; one 200-op batch each | clone plus ops within the §11 budgets; times and memory growth recorded; the document is unchanged after a dry run |

Fail-before: every case fails today (`unknown tool: designos_apply_batch`, `engine.rs:388`).

## 11. Performance budgets

- One whole-document trial clone per batch, never per op. v0.6 keeps this clone (a scoped trial is
  not specified, so no other spec may assume one). The standalone limit today is 100,000 nodes
  (`server.rs:572-574`); E15-17 raises it to 1,200,000 layers (owner answer Q2, 2026-10-18), so the
  clone is budgeted on three synthetic sizes (README generator, never the owner file), clone plus 200
  ops, release build, reference machine:

  | Nodes | Budget |
  |---|---|
  | 100,000 | under 400 ms |
  | 112,474 (the benchmark scene) | under 450 ms (the 100,000 budget scaled linearly) |
  | 1,200,000 | measured and recorded in the PR; must finish under 5 s with peak memory growth of at most one document size |

  The measured times and the memory growth are written into the PR. If the 1,200,000 case misses its
  limit, the PR stops and reports an owner question (scoped trial, or a lower write-only limit); it
  does not decide that. In Studio sessions the agent document is
  the scoped canvas (`crates/designos-app/src/live_canvas_scope.rs:11-14`: 128 top-level frames,
  5,000 full-subtree layers), so typical batches clone a few thousand nodes.
- No `to_json` of the whole document anywhere in the batch runtime path (CLAUDE.md §4.1); only tests
  serialize to compare bytes.

## 12. Dependencies and build order

First in the write-protocol track. E15-5 (errors), E15-6 (keys), E15-7 (in-progress), E15-8 (diff
images) and E15-12 (issues) all extend `tools/batch.rs`; build them after this lands.

## 13. Owner rulings

- Full scope, never MVP: all seven op types ship.
- Parallel agents hard-locked to frames (RD-3): every op's target is checked by the RD-3 lease guard
  inside the transaction; a refusal aborts the batch like any op error.
- All AI through the user's agent CLI: engine-side only.
