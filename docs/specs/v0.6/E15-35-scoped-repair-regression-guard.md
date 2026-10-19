# E15-35 Repair critique findings inside their region only, with a regression guard

- Issue: #624 (task E15-35, epic #585, L / P0)
- Spec base: `main` at `767acf9`.
- Files: `crates/designos-mcp/src/tools/critique.rs` (repair tools), the RD-3 ownership enforcement module from #572 (lease kind), `crates/designos-mcp/src/tools/repair_guard.rs` (new), `crates/designos-diff/src/lib.rs` (masked compare helper), `crates/designos-doc/src/critique.rs` (`RepairLink`).

## 1. Summary and user story

As an agent repairing a critique finding, I want a temporary write lease on exactly the nodes or region the finding names, so my fix cannot spill elsewhere; and if it does change anything outside, lint or pixels, the whole repair rolls back with a clear error.

As a user, I want each repair as its own undo step, linked to the finding it fixed.

## 2. Current state

- FACT: RD-3 (#572) enforces frame ownership for parallel agents: "An agent's edits outside its assigned frames are refused, with a named error. Reads are unrestricted."
- FACT: `designos-diff::compare(a, b, opts)` returns a `DiffReport` with `mismatched`, `ratio`, `ssim`; `DiffOptions` has `tolerance`, `detect_aa` and an `ignore: Vec<Rect>` list (`crates/designos-diff/src/lib.rs:40-60,301`).
- FACT: the MCP engine applies edits as `Command`s on a history (`crates/designos-mcp/src/engine.rs:413`, `exec`), and `apply_spec` already works on a trial clone before committing (`engine.rs:262-268`).
- FACT: E15-4 (#589) adds batched transactions with dry-run and one undo group; E15-34 stores critiques with findings.

## 3. Behaviour rules

1. **Begin.** `designos_begin_repair({ critique_id, finding_id })` grants a lease to the calling run:
   - any run may repair, including the run that generated the frame (repairing is generating; only judging is barred to authors). The call is refused only when another lease is active on an overlapping scope (`lease_conflict`) or the finding is not `open` (`finding_not_open`).
   - **Node target** → writable set = the target nodes and their descendants, plus new nodes created under them.
   - **Region target** → writable set = nodes whose world bounds lie fully inside the region, plus new nodes created under a writable container; the region is the pixel guard's allowed area.
   - The lease records the before-state: a document snapshot reference (copy-on-write clone of the document at begin), the lint findings of the whole page (E15-9), and a CPU render of the finding's top-level frame at scale 1.
   - Lease lifetime: it ends only by a successful `end` (the commit), by `cancel`, or by expiry, 10 minutes after the last write or the last failed `end` (each restarts the clock); expiry cancels (rule 5). A failed `end` does not close the lease.
2. **Writes during a lease** go to the lease's working copy, not the live document. Every write tool called by that run is checked by RD-3 enforcement extended with a lease kind: a write that touches a node outside the writable set is refused with `outside_lease` and the working copy is unchanged (AC1). Reads see the working copy for that run and the live document for others.
3. **End.** `designos_end_repair({ lease_id })` runs the guard on the working copy:
   - **Lint guard:** page findings outside the writable set (by node) before vs after, compared as multisets of `(rule id, node id)`; any finding added or removed outside the set is a regression. Findings inside the set may change freely.
   - **Pixel guard:** render the frame after, compare with the before render using `designos-diff` with `tolerance = 0`, `detect_aa = false` and `ignore` = the allowed area: for a region target the region expanded by 1 px; for a node target the union of the target nodes' before and after world bounds expanded by 1 px. Any mismatched pixel outside the ignore area is a regression (AC2).
   - **Pass:** the working copy's commands commit to the live document as one undo step labelled `Repair F<n>`; the finding becomes `repaired` with `RepairLink { lease_id, undo_label, run, at }`.
   - **Fail:** nothing is committed and the call returns `repair_regressed` with the list of lint changes and the bounding boxes of changed pixel clusters (connected components of mismatched pixels, up to 20). The working copy and the lease are kept, exactly as the agent left them, so the agent changes the repair in place and calls `end` again; the guard re-runs against the current working copy and the same before-state. The working copy is discarded only on `cancel`, on expiry, or after a successful commit.
4. **Re-critique (AC3).** After a successful repair, a lint-backed finding resolves automatically at the next `designos_critique` (E15-34 rule 7); other findings move to `resolved` when a critic run (not an author of the repair) confirms through `designos_resolve_findings`.
5. **Cancel and expiry.** `designos_cancel_repair({ lease_id })` and lease expiry discard the working copy and release the lease; the finding stays `open`. These two, and a successful commit, are the only events that discard the working copy (a `repair_regressed` result is not one of them).
6. **Live sync.** While a lease is active, Studio shows the live document (the working copy is not mirrored). On commit, the single undo step is published through the existing live patch path, like any tool call.
7. **Auto-fix interaction.** E15-13 auto-fix runs on the working copy after each write as usual; its fixes count toward the guard (a fix outside the set is a regression).

## 4. Data model / schema

```rust
pub struct RepairLease { pub id: String, pub run: u32, pub critique: String, pub finding: String,
    pub writable: WritableSet, pub allowed_area: Rect, pub base: Arc<Document>, pub working: Document,
    pub before_lint: Vec<(String, NodeId)>, pub before_png: RgbaImage, pub last_write: Instant }
pub enum WritableSet { Nodes(BTreeSet<NodeId>), Region { rect: Rect, nodes: BTreeSet<NodeId> } }
pub struct RepairLink { pub lease_id: String, pub undo_label: String, pub run: u32, pub at: String }
// designos-diff
pub fn compare_outside(a: &RgbaImage, b: &RgbaImage, allowed: &[Rect], tolerance: u8) -> (u64, Vec<Rect> /* clusters */);
```

Leases live in memory in the MCP server (not saved). `RepairLink` is stored on the finding (E15-34 `Document.critiques`).

## 5. Contracts

- `designos_begin_repair` → `{ "lease_id": "l-3", "finding": "F1", "writable": { "nodes": [301, 302] } | { "region": [40, 120, 600, 80], "nodes": [ … ] }, "expires_in_s": 600 }`.
- Any write outside → error `outside_lease`, details `{ "node_id": 77, "lease_id": "l-3", "writable_roots": [301, 302] }`, corrected example: the same call targeting a writable node, `safe_to_retry: true`.
- `designos_end_repair` → pass: `{ "committed": true, "undo_label": "Repair F1", "finding": { "id": "F1", "status": "repaired" } }`; fail: error `repair_regressed`, details `{ "lint_added": [ { "rule": "…", "node_id": 88 } ], "lint_removed": [], "pixel_clusters": [ [612, 300, 24, 18] ] }`, `safe_to_retry: true` (the working copy and the lease stay open so the agent can adjust it in place and end again).
- `designos_cancel_repair` → `{ "cancelled": true }`.
- Other errors: `lease_conflict`, `lease_unknown`, `lease_expired`, `finding_not_open`.

## 6. File-format mapping

None beyond E15-34's stored critiques.

## 7. UI

None. The undo history shows `Repair F<n>` entries.

## 8. Edge cases

1. Repair grows a node's height inside auto layout and pushes siblings outside the region: pixel guard fails, by design, and the working copy is kept; the agent shrinks the node again, or changes the layout inside the writable set, in the same lease and calls `end` again; to get more room it asks for a wider target through a new critique (cancel the lease first).
2. Repair edits a shared variable or style: the change reaches nodes outside the set (lint or pixels) and fails; if invisible outside (no other users), it passes. Variable and style writes are allowed only when the variable or style is used solely by writable nodes; otherwise `outside_lease` at write time.
3. Repair on a component main used by instances elsewhere: instances are outside the set; edits to the main are refused unless all instances are inside the set.
4. A render difference caused only by anti-aliasing at the allowed area edge: the 1 px expansion absorbs it; a test covers a moved edge.
5. Lease expiry mid-work: next write returns `lease_expired`; nothing committed.
6. Two leases on disjoint scopes by two runs: allowed concurrently; commits apply in end order; the second commit re-runs its guard against the updated live document before committing (rebase); a conflict there fails with `repair_regressed`.

## 9. Non-goals

- Choosing which finding to repair (agent's choice).
- Visual diff UI in Studio.

## 10. Test plan

Seam: `cargo test -p designos-mcp repair` with pixel fixtures built in test code.

Fail-before: the tools do not exist.

| Test | Input | Expected |
|---|---|---|
| `write_outside_lease_refused` (AC1) | lease on node 301; update node 77 | `outside_lease`; live document hash unchanged; working copy unchanged |
| `pixel_change_outside_rolls_back` (AC2) | region lease; repair also recolours a node outside via a shared parent fill | `repair_regressed` with one pixel cluster; live hash unchanged |
| `valid_repair_resolves_on_recritique` (AC3) | contrast finding on a text node; repair darkens the text | commit; next critique marks the finding resolved |
| `lint_change_outside_rolls_back` | repair creates a clipped text outside via resizing a parent | `repair_regressed`, `lint_added` lists it |
| `one_undo_step` | successful repair with 5 writes | one undo restores the pre-repair hash |
| `shared_variable_write_refused` | lease on one node; set a variable used elsewhere | `outside_lease` at write time |
| `regressed_end_keeps_working_copy_and_lease` | region lease; a repair that pushes a sibling outside the region; `end` fails with `repair_regressed`; then one more write inside the set that restores the sibling; `end` again | first `end`: error, live hash unchanged, `designos_get_node` on the run's reads still shows the working copy, lease still active; second `end`: commit, one undo step |
| `regressed_end_then_cancel_discards` | same first `end` failure; then `designos_cancel_repair` | `{cancelled: true}`; working copy gone; lease released; the finding is `open`; a new `designos_begin_repair` on the same scope succeeds |
| `regressed_end_then_expiry_discards` | same first `end` failure; clock advanced 10 minutes with no write | next write returns `lease_expired`; nothing committed; the scope is free again |
| `aa_edge_absorbed` | move a node's edge by 0.5 px inside a region | pass |

Visual evidence: before/after/diff PNGs of the pixel rollback fixture.

## 11. Performance budgets

- Begin: snapshot is a clone of the document (`Document: Clone`); on a 112k-node document ≤ 150 ms, measured in the test log.
- End guard: one frame render plus page lint; ≤ 500 ms for a 1440×900 frame of ≤ 5k nodes.
- No Studio UI-thread work; renders on the CPU path in `designos-mcp` (CLAUDE.md §4).

## 12. Dependencies and build order

- Needs E15-34, E15-4 (#589), #572.
- Unblocks the critique skill flow in E15-47.

## 13. Owner rulings that apply

- Parallel agents are hard-locked to their frames; leases narrow that lock further during repair.
- Auto-fix stays configurable and its fixes are inside the guard.
- Full scope: lint and pixel guards both.
