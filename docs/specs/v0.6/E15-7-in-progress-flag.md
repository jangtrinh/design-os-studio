# E15-7 Flag frames as in progress while an agent works and clear the flag when it finishes

| Field | Value |
|---|---|
| Epic / pack | E15 / v0.6 AI design |
| Size / priority | M / P0 |
| Depends on | E15-4, #570 (JSON scene format schema 2.21) |
| Unblocks | E15-17 (in-progress predicate), T7.2 extension, part-2 critique and variant ranking |
| Touches | `crates/designos-doc` (document model, live patch), JSON scene format module in `crates/designos-fig/src/`, `crates/designos-app/src/canvas/draw.rs`, `crates/designos-app/src/terminal.rs`, `crates/designos-mcp/src/tools/` |

## 1. Summary and user story

As a reviewer (human, critic agent or another agent), I can see which frames an agent is still
building: they carry an "Agent working" badge on the canvas and an `in_progress` flag in every MCP
read. The flag disappears when the agent finishes the frame, when its run ends or exits, or after a
stale timeout, so a crashed agent never leaves frames marked forever.

## 2. Current state (FACT)

- No node or document field records work state (`Node` fields, `crates/designos-doc/src/node.rs:65-155`;
  `Document` fields, `crates/designos-doc/src/lib.rs:142-196`).
- Agent run state exists only as a file: the agent CLI hooks write `{"state": "working" | "waiting" |
  "done"}` to `<session_dir>/agent-status.json` (`crates/designos-app/src/terminal.rs:863-930`);
  PTY liveness is `TerminalTab::is_alive` (`terminal.rs:556`).
- Canvas badges next to top-level frame labels already exist for Dev status
  (`dev_status_badge_rect`, `crates/designos-app/src/canvas/draw.rs:321-331`; `badge`,
  `draw.rs:197`; label rects `draw.rs:112-173`).
- Agent edits reach Studio as `LivePatch` lines (`crates/designos-doc/src/live_patch/mod.rs:27-35`).
- Node plugin metadata survives `.fig` round trips (`Node.plugin_data`, `node.rs:144-146`).
- Idle invariant: no periodic UI-thread repaint when idle (CLAUDE.md §4, Autosave rule).

## 3. Behaviour rules

### 3.1 Model

The flag is work state, not design content, so it is not an undo step: undoing a batch must not
resurrect a cleared flag, and clearing at run end must not create an undo entry. It is stored in the
document (so it persists and round-trips) but outside `History`:

```rust
// Document
#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
pub in_progress: BTreeMap<NodeId, InProgress>,

pub struct InProgress {
    pub run_id: Option<String>,   // None = imported or set by a user
    pub agent: Option<String>,    // "Claude Code", "Codex", ... for the badge
    pub since_ms: u64,            // Unix ms
    pub stale_after_s: Option<u32>,
}
```

Entries whose node no longer exists are pruned on read and on save.

### 3.2 Setting

- `NodeSpec.in_progress: Option<bool>` on a spec root, the batch op
  `{"op":"set_in_progress","nodes":[...],"value":true}` (E15-4 op list), and the tool
  `designos_set_in_progress`.
- Allowed targets: frames, sections, components and instances that are direct children of a page or a
  section ("root frames"). Others fail `IN_PROGRESS_TARGET_NOT_ROOT` naming the root ancestor to use.
- `run_id` = `DESIGNOS_AGENT_RUN_ID` from the environment (set by Studio per agent spawn,
  `<session_id>-<tab>-<n>`); `agent` = `DESIGNOS_AGENT_RUNTIME` label (`terminal.rs:270`).
- A node already flagged by another live run: `NODE_IN_PROGRESS_BY_OTHER_RUN` unless `force: true`.
  The flag is not a lock (RD-3 owns write locks).

### 3.3 Clearing

1. Agent: `value:false`, or the run's last batch sets `in_progress:false` on the frame it finished.
2. Run end: Studio clears every entry with that `run_id` when `agent-status.json` turns `done` or the
   PTY exits (`is_alive` false). The terminal bridge already watches both.
3. Stale: an entry older than `stale_after_s` (default from the user setting
   `agent.in_progress_stale_minutes`, default 15, range 1..=1440) is treated as cleared everywhere it
   is read, and is removed at the next save or patch. Entries with `run_id: None` never go stale and
   are cleared only explicitly.
4. Studio wakes for expiry with one `request_repaint_after` aimed at the earliest expiry, at most one
   pending at a time, never periodic; with no flags there is no timer (0% idle CPU preserved).

### 3.4 Live sync

New live-patch op `LiveOp::SetInProgress { id, value: Option<InProgress> }`, replayed by Studio
outside its undo history. It does not bump the document revision that drives tile re-render
(`TileView::rev`); the badge is overlay chrome and only needs a repaint.

### 3.5 Readers

- `designos_get_node`, `list_nodes`, `query_xpath` rows (E15-17) and metadata (T7.2) include
  `in_progress: {run_id, agent, since_ms}` or omit it.
- Lint (E15-9) still runs on flagged frames (agents need their issues); part-2 critique and variant
  ranking skip them and say so.

## 4. Data model / schema changes

- `Document.in_progress` and `InProgress` as above; native format back-compat by `serde(default)`.
- `LiveOp::SetInProgress`; older editors skip unknown ops with a reason (`LivePatchReport.skipped`,
  `crates/designos-doc/src/live_patch/replay.rs:71-72`).
- `NodeSpec.in_progress: Option<bool>`.

## 5. Contracts

Tool `designos_set_in_progress` (annotations: `readOnlyHint:false, destructiveHint:false,
idempotentHint:true`):

```json
{"type":"object","required":["node_ids","value"],"additionalProperties":false,"properties":{
  "node_ids":{"type":"array","items":{"type":"integer","minimum":1},"minItems":1,"maxItems":200},
  "value":{"type":"boolean"},
  "stale_after_s":{"type":"integer","minimum":60,"maximum":86400},
  "force":{"type":"boolean","default":false},
  "idempotency_key":{"type":"string","maxLength":128}}}
```

Output: `{"updated":[{"node_id":12,"in_progress":true}], "changed":true}`. No undo entry; an
`designos_undo` after it undoes the previous design edit, not the flag.

Errors: `IN_PROGRESS_TARGET_NOT_ROOT` (`valid_options` = the root ancestor id), `NODE_NOT_FOUND`,
`NODE_IN_PROGRESS_BY_OTHER_RUN` (`next_step`: pass `force:true` only when the other run is known
dead). All `safe_to_retry: true`.

## 6. File-format mapping

| Format | Mapping |
|---|---|
| `.designos` | `in_progress` map (above) |
| JSON scene format (#570) | not exported. `placeholder` is an existing field of that format with its own meaning (component placeholder), and #570 exports only fields the current schema defines, so the flag is never written to it and never inferred from `placeholder`. The export report lists the flagged roots left out (count and ids); import ignores `placeholder` for this purpose |
| `.fig` | `plugin_data["designos:in_progress"]` = compact JSON of the entry, so nothing is dropped on a Figma round trip; import restores it |

## 7. UI

Follows `design/principles.md` and `crates/designos-app/src/theme.rs`:
- A pill badge right after the frame name label, using the same geometry helper pattern as
  `dev_status_badge_rect`. Text `Agent working` (or `<agent> working`), Inter 10 px caption, Selection
  Blue info tint (`#0D99FF` text on its translucent background, the existing `STATUS_INFO_*`
  constants), 1 px border; no animation (idle CPU).
- Hover tooltip: agent, run id, started time, stale time.
- The badge is real state (Zero Fake UI): it renders only from `Document.in_progress`.
- Desktop capture at 1440 in the PR; responsive SKIPPED (owner default). Load es-designer before UI
  code.

## 8. Edge cases

1. Agent flags a frame, user deletes the frame: entry pruned; no badge, no error.
2. User undoes the batch that created a flagged frame: node gone, entry pruned; redo restores the node
   without the flag (work state is not history).
3. Studio restarts while flags exist: entries load; stale rule applies with the persisted `since_ms`;
   run-end clearing applies only if the same run is still alive (it is not), so entries with a
   `run_id` from a dead run are cleared on load.
4. Two agents (RD-3) each flag their own frames: badges show each agent name.
5. Clock skew (since_ms in the future after a file comes from another machine): treated as now.
6. A flagged component: the badge shows on the component label; instances are not flagged.

## 9. Non-goals

- Using the flag as a write lock (RD-3).
- A Layers-panel indicator (may follow later).

## 10. Test plan

Seams: designos-doc unit tests, a shell test with a stub agent command, and a screenshot.

| Case | Steps | Expected |
|---|---|---|
| `batch_flag_and_clear_on_run_end` | stub agent sets flag via batch; write `{"state":"done"}` to agent-status.json | entry present, badge rect computed; after status change entry removed |
| `killed_agent_flag_goes_stale` | flag with `stale_after_s: 60`, kill PTY child, advance injected clock 61 s | flag absent from readers; removed at next save |
| `scene_format_does_not_carry_flag` | export a flagged root to the scene format, re-import; also import a node with `placeholder: true` | no `placeholder` written from the flag; export report names the omitted flag; re-import has no entry; the imported `placeholder: true` node gets no `in_progress` entry |
| `fig_round_trip_keeps_flag` | export_fig then import | plugin data restores the entry |
| `undo_does_not_touch_flag` | flag, then undo previous edit | flag unchanged |
| `not_root_refused` | flag a text inside a frame | `IN_PROGRESS_TARGET_NOT_ROOT` with root id |

Clock is injected (`fn now_ms()` trait) so tests do not sleep.

## 11. Performance budgets

- Badge painting iterates `in_progress` entries (typically under 20) intersected with visible label
  rects; under 0.1 ms per frame. No tree walk.
- No timers when no flags exist; with flags, at most one pending wake.
- Pruning on save is O(entries).

## 12. Dependencies and build order

After E15-4 (batch op) and #570 (scene format exporter, needed only by the omitted-flag report and
`scene_format_does_not_carry_flag`). The `.designos` and `.fig` parts do not wait for #570.

## 13. Owner rulings

- Parallel agents hard-locked to frames (RD-3): the flag complements, never replaces, RD-3 locks.
- Nothing vanishes silently (delivery law 1): the `.fig` plugin-data mapping keeps the flag.
- Zero fake UI: the badge shows only real state.
