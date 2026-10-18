# RD-3 Parallel designer agents with enforced frame ownership

| Field | Value |
|---|---|
| Issue | #572 (moved from v0.19 Desktop MCP to v0.6 AI design; E15 link 2026-10-18) |
| Size / priority | L / P0 (pack headline dependency) |
| Depends on | E15-4 (transactions), E15-5 (errors), E15-16 (prefetch); uses E15-7 (in-progress) when present |
| Unblocks | part-2 E15-34 (critique refuses self-review), E15-35 (repair leases), E15-36 (variant agents) |
| Touches | new `crates/designos-mcp/src/tools/agents.rs`, new `crates/designos-mcp/src/tools/lease.rs`, `crates/designos-mcp/src/server.rs` (write guard), `crates/designos-doc/src/live_patch/` (`run_id` on patches, replay guard), `crates/designos-app/src/terminal.rs` (spawning, run tracking), canvas owner badges in `crates/designos-app/src/canvas/draw.rs`, Agents panel |

## 1. Summary and user story

As a lead agent (or a user), I start several designer agents at once, each owning specific frames:
split one design into sections, build N independent versions side by side, or keep iterating new
variations until stopped. Each agent can read everything but can only change its own frames; the
engine refuses anything else. Each agent's work undoes as its own step, and a failed run rolls back
only that agent's changes.

## 2. Current state (FACT)

- Studio runs one PTY agent per terminal tab, each with its own session directory
  (`tab_session_id`, `crates/designos-app/src/terminal.rs:668-745`), and launches the user's own CLIs
  (`AgentCliPreset`, `terminal.rs:62-176`; installed check `terminal.rs:167`).
- Each agent CLI starts its own `designos-mcp` process (stdio, `crates/designos-mcp/src/main.rs:5-17`)
  editing a copy loaded from the session's `canvas.designos` and sending edits to Studio through its
  own `doc-patch.jsonl` (`crates/designos-mcp/src/server.rs:19-94`, `252-267`). Studio replays each log
  into the one open document, remapping colliding ids (`crates/designos-doc/src/live_patch/replay.rs:15-26`).
- There is no ownership concept: any agent may write anywhere. Agent status per tab comes from hooks
  writing `agent-status.json` (`terminal.rs:863-930`).
- `LivePatch` has no run identity (`crates/designos-doc/src/live_patch/mod.rs:27-35`).

## 3. Behaviour rules

### 3.1 Runs and leases

- A run = one agent process started by Studio, id `DESIGNOS_AGENT_RUN_ID` (`<doc-instance>-<tab>-<n>`),
  exported in its environment together with `DESIGNOS_LEAD_RUN_ID` when spawned by a lead.
- `Lease { run_id, frames: Vec<NodeId>, purpose: split | side_by_side | iterate | repair | variant,
  lead_run_id: Option<String>, granted_ms }`. Frames are root frames (E15-7 definition). A frame is
  leased to at most one run.
- Studio owns the lease table and writes it to `leases.json` in the session directory of every agent
  tab of that document whenever it changes (atomic write).

### 3.2 Enforcement (hard lock, two layers)

1. MCP write guard: before any mutating call (the `mutating` list plus batch, set_in_progress and
   later write tools), the session reloads `leases.json` if its mtime changed and computes the call's
   affected set: every node it updates, deletes or moves (old and new parent), and the parent of every
   insert. If the caller's run holds a lease, each affected node must be a descendant-or-self of one of
   its frames. If the caller holds no lease, it may not touch any node inside a frame leased to another
   run. Violation → `OUTSIDE_OWNED_FRAMES`, document unchanged, `valid_options` = the caller's frame
   ids, `safe_to_retry: true`.
2. Studio replay guard (backstop): `LivePatch` gains `run_id`; on replay, each op is checked against
   the same rule; violating ops are skipped with the reason in `LivePatchReport.skipped`, and the run
   is flagged `violated` in the Agents panel. Writes that bypass the MCP never land.
- Reads are unrestricted for every run.
- Document-level writes (variables, styles) by a leased agent are allowed only when the lease purpose
  is `split` and the lead granted `allow_shared: true`; otherwise `OUTSIDE_OWNED_FRAMES` with
  `path` naming the variables/styles op.

### 3.3 Spawning and modes

Only a run with no lead (a lead) or the user can spawn. Studio receives spawn requests from the lead's
MCP through `<session_dir>/spawn-requests.jsonl` (one JSON line per request; Studio watches the session
directory off the UI thread, as `DiskWatcher` does) and answers in `spawn-results.jsonl`.

| Mode | Behaviour |
|---|---|
| `split` | each entry names its frames (sections of one design); Studio grants the leases and spawns one run per entry |
| `side_by_side` | `count` runs with the same prompt; Studio creates `count` empty frames next to the anchor frame (same size, 80 px gap), leases one to each run |
| `iterate` | like side_by_side with count 1, repeated: when a run finishes, a new frame and run start, until the lead or user stops it or `max_rounds` is reached; mode `same_style` keeps the previous round's archetype parameters, `new_style` asks for a different one in the prompt |

Each run's first turn gets the E15-16 bootstrap with its frames as targets, the frames' computed layout
and the document variables. Runtime per entry: one of the installed CLIs; an uninstalled runtime fails
with `RUNTIME_NOT_INSTALLED` (`valid_options` = installed runtimes). No model is bundled.

### 3.4 Undo and rollback

- Every batch keeps its own undo step (E15-4) in Studio; the History panel groups steps by run.
- `Undo run` on a run's group reverts that run selectively: the inverse of all its commands in reverse
  order, applied as one new step. This is well-defined because no other run could touch its frames.
- A failed run (process exit status ≠ 0, cancelled by the lead or user, or flagged `violated`) is
  rolled back automatically with `Undo run`, and the Agents panel says what was reverted. A run that
  ends normally keeps its work.
- Every node an agent creates or updates records `plugin_data["designos:run"] = <run_id>` in the same
  command (so it survives undo/redo and `.fig` round trips); part-2 critique uses it to refuse
  self-review.

## 4. Data model / schema changes

- `Lease`, `LeaseTable` and `fn allows(&self, run, doc, affected) -> Result<(), Violation>` (serde) live
  in new `crates/designos-doc/src/live_patch/lease.rs`, so the MCP guard (`tools/lease.rs`) and Studio's
  replay guard run the same check.
- `LivePatch.run_id: Option<String>` (skipped when `None`).
- `Node.plugin_data["designos:run"]` convention; no new node field.

## 5. Contracts

Tools (all `destructiveHint:false` except stop):

`designos_spawn_agents`:

```json
{"type":"object","required":["mode","agents"],"additionalProperties":false,"properties":{
  "mode":{"enum":["split","side_by_side","iterate"]},
  "agents":{"type":"array","minItems":1,"maxItems":8,"items":{"type":"object","required":["prompt"],"additionalProperties":false,
    "properties":{"prompt":{"type":"string","maxLength":20000},"frames":{"type":"array","items":{"type":"integer","minimum":1}},
                  "runtime":{"enum":["claude","codex","antigravity","grok"]},"label":{"type":"string","maxLength":64},
                  "allow_shared":{"type":"boolean","default":false}}}},
  "count":{"type":"integer","minimum":1,"maximum":8},
  "anchor_frame":{"type":"integer","minimum":1},
  "max_rounds":{"type":"integer","minimum":1,"maximum":20},
  "style_mode":{"enum":["same_style","new_style"]},
  "idempotency_key":{"type":"string","maxLength":128}}}
```

Output: `{"runs":[{"run_id":"d7-3-1","label":"Header","runtime":"claude","frames":[812],"status":"starting"}]}`.

`designos_assign_frames` `{run_id, frames}` (lead or user only; replaces the lease; frames held by a
running other agent fail `FRAME_LEASED` unless that run is stopped first).
`designos_list_agents` `{}` → runs with status (`starting|working|waiting|done|failed|stopped`), frames,
edit counts, last activity.
`designos_stop_agents` `{run_ids?: [...], all?: bool}` → stopped runs (their work kept unless
`rollback: true`).

Errors: `OUTSIDE_OWNED_FRAMES`, `FRAME_LEASED`, `NOT_LEAD`, `SPAWN_REQUIRES_STUDIO` (standalone MCP,
no session directory), `RUNTIME_NOT_INSTALLED`, `TOO_MANY_AGENTS` (more than 8 concurrent).

## 6. File-format mapping

Leases are session state, never saved in the document. `designos:run` plugin data round-trips through
`.designos` and `.fig`.

## 7. UI (load es-designer before UI code; desktop capture at 1440, responsive SKIPPED)

- Canvas: each leased frame shows an owner pill after its name label (same helper as Dev status,
  `crates/designos-app/src/canvas/draw.rs:321-331`): agent label and a status dot (`Running`, `Idle`,
  `Exited` colours from the terminal palette, P7). Owner colours come from a fixed 6-colour list built
  from the four semantic accents and two Zinc-compatible tints; never purple (P2).
- Agents panel: a `Runs` list (28 px rows): label, runtime, frames (click selects and zooms), status,
  edit count, and Stop / Undo run buttons (24 px targets). A violated run shows a red status text with
  the count of refused ops.
- History panel: steps grouped under each run.
- All controls are wired to real runs (Zero fake UI).

## 8. Edge cases

1. Lead tries to edit a frame leased to a running agent: refused; must `designos_assign_frames` or stop
   it first.
2. Agent inserts into a frame it owns but moves a node out of it: refused (new parent outside).
3. User edits a leased frame by hand: allowed (leases bind agents, not the user); the agent sees the
   change on its next canvas reload.
4. Two agents create nodes at the same time: id collisions handled by replay remap; each agent sees its
   own ids until reload (existing behaviour).
5. Frame deleted by the user while leased: lease dropped; the agent's next write there fails
   `NODE_NOT_FOUND`.
6. Studio closes while runs are active: runs are stopped, work kept, leases discarded.
7. `iterate` with no `max_rounds`: stops on user/lead stop only; each round's frame is a new frame, so
   earlier rounds stay intact.

## 9. Non-goals

- Agents on different documents, remote agents, or a bundled model.
- Merging competing versions (E15-37 ranks and merges variants).

## 10. Test plan

Seams: MCP guard unit tests, live-patch replay tests, shell test with a stub agent command, screenshot.

| Case | Expected |
|---|---|
| `write_outside_lease_refused` | lease frames [10]; `update_node` on a node under frame 20 → `OUTSIDE_OWNED_FRAMES`, revision unchanged |
| `replay_guard_skips_foreign_ops` | hand-written patch line with `run_id` A touching B's frame → op skipped with reason; document unchanged |
| `three_agents_concurrently_no_lost_edits` | 3 stub agents each writing 50 batches into their own frame (interleaved logs) → final document has all 150 batches' nodes |
| `each_run_undoes_as_its_own_step` | after the 3 runs, `Undo run` on B → only B's nodes reverted; A and C intact |
| `failed_run_rolls_back` | stub agent exits with status 1 after 5 batches → its 5 batches reverted automatically |
| `side_by_side_creates_frames` | count 3 → 3 new frames, each leased to one run |
| `authorship_recorded` | nodes created by run A carry `designos:run = A` |

## 11. Performance budgets

- Guard check per write: O(affected nodes × depth) ancestor walk, ≤ 0.2 ms for typical batches; lease
  reload only on mtime change.
- Studio replay guard: O(ops); no full-document work.
- Up to 8 concurrent runs; Studio's watcher is a single off-UI-thread directory watch (0% idle CPU when
  nothing changes).

## 12. Dependencies and build order

After E15-4, E15-5 and E15-16. The guard lands before any part-2 multi-agent task.

## 13. Owner rulings

- Parallel agents are hard-locked to their frames (2026-10-18): edits outside assigned frames are
  refused with a named error; reads unrestricted; the lead can reassign.
- All AI through the user's agent CLI (R4.13): runs are the user's installed CLIs.
- E15 link (2026-10-18): the lease mechanism is reused for repair leases (E15-35) and variant agents
  (E15-36); authorship per node enables refusing self-review (E15-34).
- Purple Ban (P2) for owner colours.
