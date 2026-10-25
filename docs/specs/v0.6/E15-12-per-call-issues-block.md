# E15-12 Return a lint issues block from every mutating MCP call

| Field | Value |
|---|---|
| Epic / pack | E15 / v0.6 AI design |
| Size / priority | M / P0 |
| Depends on | E15-9, E15-4 |
| Unblocks | E15-13, E15-15, T7.13 link (script tool returns the same block) |
| Touches | new `crates/designos-mcp/src/tools/issues.rs`, new `crates/designos-mcp/src/tools/session_options.rs`, every mutating tool reply in `crates/designos-mcp/src/engine.rs` / `tools/` |

## 1. Summary and user story

As an agent, every write tells me, in the same reply, what is now wrong in the area I touched
("text 'Plan title' is truncated", "card overflows its parent"), with layer names, so I fix it in my
very next call while the context is fresh. I can turn the block down to errors only, or off.

## 2. Current state (FACT)

- Mutating replies carry the node and `changed` only, e.g. `create` returns
  `{"node", "parent_id", "changed"}` (`crates/designos-mcp/src/engine.rs:485`), `apply_spec` returns
  `{"root_node_id", "parent_id", "node", "changed"}` (`engine.rs:269`).
- Lint is a separate call with two rules (`engine.rs:350-375`); E15-9 replaces it with the registry
  and `lint_subtree` with a node/time budget.
- The session knows which tools mutate (`crates/designos-mcp/src/server.rs:508-520`); E15-4 adds
  `touched_roots` to batch replies.
- There is no per-session option store in designos-mcp.

## 3. Behaviour rules

### 3.1 Which calls

`designos_create_node`, `designos_update_node`, `designos_delete_node`, `designos_apply_spec`,
`designos_apply_batch` (including dry runs), the `set_style` batch op, and the part-2 style tools
(`designos_apply_style`) when they land. Read tools, comments, dev status, undo and redo do not carry
the block.

### 3.2 Lint scope

For each touched root `r` (E15-4 definition; single tools: the created or updated node, or for delete
the former parent):
1. Scope = the subtree of `r`'s parent container, so sibling overlap, overflow and hug collapse that the
   change caused are caught.
2. If that subtree exceeds 2,000 nodes: `r`'s subtree plus `r`'s direct siblings (not their
   descendants).
3. Scopes are deduplicated (a root inside another root's scope is merged).

Lint runs on the post-commit document (or the trial for a dry run), after layout is settled, with the
session's preset (default `agent`, E15-9 3.3) and a `RenderLintEnv` created on the MCP thread.

### 3.3 Budget and ordering

- At most `max_items` findings (default 20, 1..=100), sorted by severity, then rule order in the
  preset, then paint order.
- `issues_total` = all findings in scope after ignore filtering; `issues_truncated` when
  `issues_total > max_items`.
- Wall-time budget 50 ms per call (E15-9 `Budget`); when it runs out, `issues_partial: true` and
  `issues_scanned` say how far it got. A write never fails because of lint.

### 3.4 Item shape (compact)

```json
{"rule":"text-truncated","severity":"error","node_id":88,"node_name":"Plan title",
 "path":"Pricing/Card 2/Plan title","message":"Text needs 2 lines but shows 1.",
 "fix":{"kind":"set","property":"textAutoResize","to":"HEIGHT"},"auto_fixable":false}
```

`path` is the layer-name path from the touched root's frame; `fix` is E15-9's `suggested_fix` without
`from`; `auto_fixable` is `true` only for the E15-13 safe tier (`sizing-conflict`, `hardcoded-value`
exact binds) and is `false` for everything else, including `text-truncated` (a visible resize) and
until E15-13 lands.

### 3.5 Session options

Tool `designos_configure_session` sets options for this MCP session:
- `issues.mode`: `preset` (default), `errors_only`, `off`;
- `issues.preset`: any E15-9 preset or rule list (default `agent`);
- `issues.max_items`: 1..=100 (default 20);
- `auto_fix.*`: reserved for E15-13.

Options persist in `<session_dir>/session-options.json` when a session directory exists (survives
reconnect, like the E15-6 ledger) and reset on document switch. With `mode: off` the reply contains
none of the `issues*` keys, and nothing else in the reply changes.

## 4. Data model / schema changes

- `SessionOptions { issues: IssuesOptions { mode, preset, max_items }, auto_fix: AutoFixOptions }`
  (serde; unknown keys rejected).
- Reply keys added to mutating tools: `issues`, `issues_total`, `issues_truncated`, and when relevant
  `issues_partial`, `issues_scanned`. Their schemas join each tool's `outputSchema` (E15-5).

## 5. Contracts

`designos_configure_session` (`readOnlyHint:false, destructiveHint:false, idempotentHint:true`):

```json
{"type":"object","additionalProperties":false,"properties":{
  "issues":{"type":"object","additionalProperties":false,"properties":{
    "mode":{"enum":["preset","errors_only","off"]},
    "preset":{"type":"string"},
    "max_items":{"type":"integer","minimum":1,"maximum":100}}},
  "auto_fix":{"type":"object"}}}
```

Output: the full effective options object. Errors: `UNKNOWN_PRESET`, `UNKNOWN_RULE`,
`VALUE_OUT_OF_RANGE` (all `safe_to_retry: true`).

Mutating reply example (clean write): `{"node":{...},"changed":true,"issues":[],"issues_total":0,
"issues_truncated":false}`.

## 6. File-format mapping

None.

## 7. UI

None in Studio. (Studio's own Check designs panel is unaffected; E15-14 shows auto-fix reports.)

## 8. Edge cases

1. Write that deletes the last child of a hugging auto-layout frame: scope is the parent's parent
   subtree, so `collapsed-size` on the frame is reported.
2. Write on an ignored node: ignored findings are filtered (E15-9 3.5); `issues_total` excludes them.
3. Pre-existing problems in the scope that the write did not cause: reported too (the agent owns the
   area it touches); the item carries `preexisting: true` when the same finding existed before the
   call (compared against a pre-call lint of the same scope, only when the scope ≤ 500 nodes; otherwise
   the flag is omitted).
4. Idempotent replay (E15-6): the stored reply's issues are returned as stored (they describe the
   state right after the original call).
5. Text engine unavailable (headless without fonts): text rules are in `issues_skipped_rules`, never
   silently dropped.
6. Batch touching 40 roots on several pages: scopes per root, merged; the 50 ms budget applies to the
   whole call.

## 9. Non-goals

- Applying fixes (E15-13).
- Linting on read tools.

## 10. Test plan

Seam: MCP fixture tests (`cargo test -p designos-mcp`).

| Case | Steps | Expected |
|---|---|---|
| `clipped_text_named_in_reply` | `apply_spec` of a clipping 100x40 frame with text "Enterprise annual plan" 16 px fixed 200x20 | `issues` contains `clipped-partial` (or `text-truncated`) with `node_name` "Enterprise annual plan" |
| `clean_write_returns_empty_issues` | well-formed auto-layout card | `issues == []`, `issues_total == 0` |
| `off_removes_only_the_block` | same write with mode off vs preset on a fresh copy | replies equal after removing `issues*` keys |
| `errors_only_filters` | write producing a warning and an error | one item, the error |
| `budget_truncates_and_counts` | 60 findings, `max_items` 20 | 20 items, `issues_total` 60, truncated true |
| `options_survive_reconnect` | set mode errors_only, new Session on same dir | effective mode errors_only |

Fail-before: `clipped_text_named_in_reply` fails today (no `issues` key).

## 11. Performance budgets

Per-call lint ≤ 50 ms wall time (typically ≤ 25 ms for ≤ 2,000 nodes, E15-9 budget). Never a full-page
or full-document lint on the write path. The optional `preexisting` comparison runs only for scopes
≤ 500 nodes.

## 12. Dependencies and build order

After E15-9 (registry, `lint_subtree`) and E15-4 (touched roots). Before E15-13 (auto-fix reports ride
on the same reply) and E15-15 (rules pack describes the block).

## 13. Owner rulings

- Auto-fix (2026-10-18) configurability lives in the same session options (E15-13).
- All AI through the user's agent CLI: the block is designed for the agent CLIs' next turn.
