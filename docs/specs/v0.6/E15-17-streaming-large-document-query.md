# E15-17 Query large documents with a streaming visitor: paging, node budget and compact rows

| Field | Value |
|---|---|
| Epic / pack | E15 / v0.6 AI design |
| Size / priority | M / P0 |
| Depends on | E15-9 |
| Unblocks | part-2 E15-29 (S3 edits on large files), E15-16 follow-up calls |
| Touches | `crates/designos-mcp/src/engine.rs` (`designos_query_xpath` dispatch only), new `crates/designos-mcp/src/tools/query.rs`, new `crates/designos-doc/src/query/` (streaming evaluator built from the legacy XPath parser), `crates/designos-mcp/src/server.rs` (`validate_document` layer limit) |

## 1. Summary and user story

As an agent on a very large file, I ask targeted questions ("all text with a truncation problem on
this page", "frames named Card with hard-coded fills") and page through the answers with a cursor,
each page bounded in nodes scanned and rows returned, with compact rows and honest scanned/remaining
counts.

## 2. Current state (FACT)

- MCP `designos_query_xpath` uses an ad-hoc matcher: `//TAG`, `//TAG[@name='x']`,
  `contains(@name, ...)`, or a name substring; it collects every match with no limit or cursor
  (`crates/designos-mcp/src/engine.rs:307-349`). It is not declared in `tools()` (T7.6 declares it).
- designos-doc has a real XPath subset (child and descendant axes, node tests, positional and
  attribute predicates with `and`/`or`, `contains`, comparisons): `parse_xpath`, `node_attr`,
  `eval_predicate_expr`, `query_xpath(doc, page, expr, limit)` (lines 108-560 of the legacy module in
  `crates/designos-doc/src/`). It materialises a candidate vector per step (whole-subtree `walk` for
  `//`), so memory and time scale with the document, and it has no cursor.
- `designos_open_file` refuses documents over 100,000 layers (`crates/designos-mcp/src/server.rs:572-574`),
  so the large benchmark (112,474 scene nodes on the 7-page subset; 1,082,324 in the full file) cannot
  be opened by the standalone MCP today.

## 3. Behaviour rules

### 3.1 Streaming evaluation

- Parse with the existing parser (moved into `designos_doc::query`, legacy wrapper kept).
- Evaluate in one depth-first pre-order pass over the scope with an NFA over steps: each visited node
  carries the set of step indices its ancestors satisfied; a child-axis step `i` can match only if the
  parent satisfied `i-1`, a descendant-axis step if any ancestor did. A node matching the last step is
  emitted. Each node is visited once, so results are unique and ordered by document pre-order.
- Positional predicates (`[2]`) are evaluated against siblings of the same type, as today.

### 3.2 Paging and budgets

- `limit`: rows per page (1..=500, default 100).
- `max_scanned`: hard node budget per call (1,000..=200,000, default 50,000). The pass stops at
  whichever comes first; the cursor captures the DFS stack `(node id, next child index)` and NFA
  states.
- Cursor: opaque base64 of `{v, doc_revision, scope_root, expr_hash, stack}`. A cursor is valid only
  for the same expression and document revision; otherwise `CURSOR_STALE` (`safe_to_retry: true`,
  `next_step`: restart without cursor).
- Reply counts: `scanned` (this call), `scanned_total` (across the cursor chain), `scope_size` (nodes in
  the scope, computed once on the first page and carried in the cursor), `remaining = scope_size −
  scanned_total`, `done: bool`.

### 3.3 Predicates added

| Predicate | Meaning |
|---|---|
| `has-problem('<rule>')` | E15-9 finding of that rule on the node (ignored findings excluded) |
| `@problems` | number of findings (preset `agent`, pixel rules excluded unless named) |
| `@inProgress` | E15-7 flag (bool) |
| `is-bound('<property key>')` | property bound to a variable (e.g. `fills/0`, `itemSpacing`) |
| `has-hardcoded('<property key or *>')` | E15-11 `hardcoded-value` finding on that property |
| `@visible`, `@locked`, existing `node_attr` attributes | unchanged |

Problem predicates evaluate rules per candidate node through `lint_subtree` restricted to the named
rules, with the text env created on the MCP thread. Nodes cost 1 budget unit each, plus 20 units per
node for a predicate needing text shaping and 200 for a pixel rule, so budgets stay meaningful.

### 3.4 Rows

`format: "rows"` (default) returns a header and arrays:
`columns: ["id","name","type","x","y","w","h","problems","in_progress"]` with page-space bounds rounded
to 0.01; `problems` = rule ids found (only when computed, else `null`). `format: "objects"` returns the
same fields as objects. `with_problems: true` computes problems for every returned row (budgeted).

### 3.5 Large documents in the standalone MCP

`validate_document`'s layer limit rises from 100,000 to 1,200,000 (covers the full benchmark), because
reads are now bounded. The validation pass itself is O(n · depth) with depth ≤ 256 and runs once per
open. The 64 MiB input-file limit (`server.rs:395-397`) rises to 512 MiB for the same reason; the size
of the synthetic large fixture file is recorded in the PR so the limit is checked against real data. Writes on such documents keep E15-4's single whole-document trial clone; E15-4's
`trial_clone_budget_on_large_synthetic` test gates its time and memory on 112,474 and 1,200,000
synthetic nodes.

## 4. Data model / schema changes

- `designos_doc::query::{Query, QueryCursor, QueryPage, run(doc, scope, &Query, &mut Budget,
  Option<QueryCursor>, env: Option<&dyn LintEnv>)}`.
- No document change.

## 5. Contracts

Tool `designos_query_xpath` (declared, `readOnlyHint:true, idempotentHint:true`; alias `query_nodes`
kept for dispatch):

```json
{"type":"object","required":["selector"],"additionalProperties":false,"properties":{
  "selector":{"type":"string","minLength":1,"maxLength":2048},
  "page_id":{"type":"integer","minimum":1},
  "node_id":{"type":"integer","minimum":1},
  "limit":{"type":"integer","minimum":1,"maximum":500,"default":100},
  "max_scanned":{"type":"integer","minimum":1000,"maximum":200000,"default":50000},
  "cursor":{"type":"string"},
  "format":{"enum":["rows","objects"],"default":"rows"},
  "with_problems":{"type":"boolean","default":false}}}
```

Worked examples (in the description, E15-5): `//TEXT[has-problem('text-truncated')]`,
`//FRAME[@name='Card']//*[has-hardcoded('fills/0')]`, `//*[@inProgress]`.

Output:

```json
{"columns":["id","name","type","x","y","w","h","problems","in_progress"],
 "rows":[[88,"Plan title","TEXT",24,96,180,20,["text-truncated"],false]],
 "scanned":50000,"scanned_total":50000,"scope_size":112474,"remaining":62474,"done":false,
 "cursor":"eyJ2IjoxLCJkb2NfcmV2aXNpb24iOjQx..."}
```

Errors: `INVALID_SELECTOR` (parser message, `path:"/selector"`, `corrected_example` when the fix is a
known pattern, e.g. a missing closing bracket), `CURSOR_STALE`, `NODE_NOT_FOUND`, `EXACTLY_ONE_SCOPE`,
`UNKNOWN_RULE` (in `has-problem`). The old reply shape (`nodes`, `total` with no limit) is gone.

## 6. File-format mapping

None.

## 7. UI

None.

## 8. Edge cases

1. Expression matching the scope root itself (`/*`): roots are never emitted (consistent with today's
   `depth > 0` rule in the legacy engine).
2. Document edited between pages: `CURSOR_STALE`; the agent restarts (results would otherwise skip or
   repeat).
3. Budget runs out with zero rows: valid page (`rows: []`, `done: false`, cursor set).
4. Selector with only positional predicates on huge sibling lists: sibling counting is O(siblings) per
   candidate; capped by budget units (each sibling scanned costs 1 unit).
5. Problem predicate when no text engine exists: `has-problem` on text rules fails with
   `RULE_UNAVAILABLE` naming the reason (never silently false).
6. Scoped Studio canvas: results cover what the session holds; reply `scoped: true`.

## 9. Non-goals

- Full XPath 1.0 (axes beyond child/descendant, functions beyond those listed).
- Write operations from queries.

## 10. Test plan

Seams: MCP test on the large fixture plus perf record; designos-doc unit tests for the evaluator.

| Case | Expected |
|---|---|
| `paging_returns_each_text_once` | synthetic large fixture (README); `//TEXT` with limit 500, max_scanned 50,000, follow cursors to `done` → ids unique, count equals the number of text nodes from a direct count |
| `clipped_text_query_matches_linter` | fixture with 12 truncated texts → `//TEXT[has-problem('text-truncated')]` ids == E15-9 `text-truncated` finding ids |
| `no_page_exceeds_budget` | every page reports `scanned ≤ max_scanned` (in units) and `rows ≤ limit` |
| `cursor_stale_after_edit` | page 1, edit, page 2 with cursor → `CURSOR_STALE` |
| `legacy_selectors_still_work` | `//FRAME[@name='Card']`, `//*[contains(@name,'btn')]` return the same ids as the legacy `query_xpath` |
| `large_open_accepted` | open a 112,474-node synthetic `.designos` → success (fails today at 100,000) |

## 11. Performance budgets

- Plain selectors: ≥ 2 million nodes scanned per second release; a 50,000-node page in ≤ 30 ms.
- Text-shaping predicates: bounded by budget units (≈ 2,500 text nodes per default page).
- Memory per call O(depth + rows), never O(document).
- Opening the large fixture in the standalone MCP: ≤ 2 s including validation (recorded).

## 12. Dependencies and build order

After E15-9 (problem predicates) and alongside T7.6 (tool declared). Before part-2 E15-29.

## 13. Owner rulings

- Full scope: problem and hard-coded predicates ship with paging.
- The eval never uses client files: tests use the synthetic generator; the local benchmark file is
  used only for manual perf recording.
