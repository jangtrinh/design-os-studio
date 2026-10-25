# E15-34 Critique designs against a rubric checklist with bounding boxes, never by the generator

- Issue: #623 (task E15-34, epic #585, L / P0)
- Spec base: `main` at `767acf9`.
- Files: `crates/designos-mcp/src/tools/critique.rs` (new), `crates/designos-doc/src/agent_meta.rs` (SC-4: authorship fields), `crates/designos-doc/src/critique.rs` (new: stored critiques), `crates/designos-doc/src/lib.rs` (`Document.agent_runs`, `Document.critiques`), `crates/designos-render/src/annotate.rs` (new: numbered boxes overlay for headless PNGs), `engine.rs` tool list.

## 1. Summary and user story

As an agent acting as a critic, I want a scaffold for one frame (rubric axes, the surface checklist, lint findings and a screenshot), then to submit findings tied to node ids or region boxes with a severity and a repair target, validated by the engine.

As a user, I want critique never done by the agent run that made the design, and findings I can see as numbered boxes or as canvas comments.

Unstructured screenshot loops do not improve quality; rubric-scoped critique does.

## 2. Current state

- FACT: no critique tool exists. Comments exist: `designos_add_comment` takes `text`, `author`, `node_id`, `page_id`, `x`, `y` (`crates/designos-mcp/src/engine.rs:154,645-654`).
- FACT: RD-3 (#572) adds parallel agents with enforced frame ownership and per-agent runs ("Each agent run is an atomic undo block"); it defines run identity.
- FACT: T7.1 (#238, extended for E15) returns screenshots as MCP image content with `region` and `max_long_edge` (default 1024).
- FACT: E15-7 (#592) flags frames in progress; E15-9 (#594) is the linter; E15-21 (#609) the surface profiles; E15-31 the rubric file (`rubric.toml`, our condensed wording of the ease-design taste rubric's seven axes).
- FACT: nodes carry no authorship today (`Node` fields, `crates/designos-doc/src/node.rs:66-161`).

## 3. Behaviour rules

1. **Authorship (SC-4).** Every node created or modified by an MCP write records the writing run: `Node.agent.author: Option<u32>` (index into `Document.agent_runs`), updated on create and on every property change made by an agent run. `Document.agent_runs: BTreeMap<u32, AgentRunRecord { run_id, agent_label, model, started_at }>` holds one entry per run; a user edit in Studio sets `author = None` for the edited node (human-authored). The run id comes from the RD-3 session (or, for a plain MCP session without RD-3, a run id generated at session start and reported by `designos_get_document`).
2. **Scaffold.** `designos_critique({ frame_id, post_comments?, include_image? })`:
   - refuses frames flagged in progress (E15-7) with `frame_in_progress`;
   - refuses when the calling run authored any node in the frame (`critic_is_generator`, AC1), listing up to 10 such node ids;
   - creates a stored critique `{ critique_id, frame_id, critic_run, created_at, status: "open" }` and returns: the frame bounds (page space), the rubric axes that apply (Motion excluded for static frames), the surface checklist (the frame's E15-21 profile rules in plain words plus the prose's Checks section), the current lint findings for the frame (E15-9 rows), a compact outline (as E15-31), and a screenshot via #238 (`max_long_edge` 1024) when `include_image` (default true).
3. **Submission.** `designos_submit_critique({ critique_id, findings[], scores? })`. Each finding:
   - `target`: `{ "nodes": [ids] }` (each must be the frame or a descendant) or `{ "region": { "x", "y", "w", "h" } }` in page coordinates;
   - `region` must lie inside the frame bounds (tolerance 0.5 px on each edge) with `w, h ≥ 4` (AC2: outside → `finding_out_of_bounds`);
   - `basis`: `{ "axis": "spacing" }`, `{ "check": "<surface rule id>" }`, or `{ "lint": "<finding id>" }`;
   - `severity`: `blocker | major | minor`;
   - `message` (≤ 400 chars), `repair_target` (≤ 400 chars, what the fixed state looks like), optional `suggestion` (≤ 400 chars).
   Validation is all-or-nothing: one invalid finding rejects the submission with `critique_invalid` and per-finding reasons. On success findings get stable ids `F1…Fn` and `status: "open"`.
4. **Scores.** Optional `scores` per applicable axis 0–10; the critique stores them; `passed` = every applicable axis ≥ 7 (the rubric's floor) and no `blocker` finding.
5. **Annotated image.** The submit result includes a PNG (MCP image content) of the frame render at the same scale as the scaffold image, with each finding drawn as a 2 px box in `#FE5938` (finding bounds: the union of target nodes' world bounds, or the region) and a numbered label (white text on a 16 px circle). Box pixel coordinates are `round((bounds − frame origin) × scale)`; AC3: within 1 px of the finding bounds at that scale. The overlay is drawn by `designos-render`'s `annotate` helper on the CPU path; nothing is added to the document.
6. **Comments.** With `post_comments: true`, each finding becomes a comment thread through the existing comment path: `node_id` = the first target node or the frame; `x, y` = the finding bounds' top-left in page space; text `F<n> [<severity>] <message> → <repair_target>`; author = the critic's agent label. The comment id is stored on the finding.
7. **Lint-backed resolution.** A finding whose `basis` is `lint` is auto-resolved when that lint rule no longer fires on those nodes at the next `designos_critique` of the frame. Other findings are resolved by a later critic run (any run except the frame's authors) through `designos_resolve_findings({ critique_id, finding_ids, note })`.
8. **Storage.** `Document.critiques` keeps critiques with their findings and statuses so E15-35 repairs can link to them and history survives save and reload. A critique older than the frame's last structural change is marked `stale` in listings.

## 4. Data model / schema

```rust
// agent_meta.rs (SC-4; this task adds `author`)
pub struct AgentMeta { pub surface: Option<SurfaceKind>, #[serde(default, skip_serializing_if = "Option::is_none")] pub author: Option<u32> }
// lib.rs on Document
#[serde(default, skip_serializing_if = "BTreeMap::is_empty")] pub agent_runs: BTreeMap<u32, AgentRunRecord>,
#[serde(default, skip_serializing_if = "Vec::is_empty")] pub critiques: Vec<Critique>,
pub struct AgentRunRecord { pub run_id: String, pub agent_label: String, pub model: Option<String>, pub started_at: String }
pub struct Critique { pub id: String, pub frame: NodeId, pub critic_run: u32, pub created_at: String,
    pub status: CritiqueStatus, pub scores: BTreeMap<String, f32>, pub passed: Option<bool>, pub findings: Vec<CritiqueFinding> }
pub struct CritiqueFinding { pub id: String, pub target: FindingTarget, pub basis: FindingBasis, pub severity: Severity,
    pub message: String, pub repair_target: String, pub suggestion: Option<String>, pub status: FindingStatus,
    pub comment_id: Option<String>, pub repair: Option<RepairLink> /* E15-35 */ }
pub enum FindingTarget { Nodes(Vec<NodeId>), Region(Rect) }
pub enum FindingStatus { Open, Repaired, Resolved, Rejected }
```

Back-compat: all new fields default to empty. `.fig` export ignores them.

## 5. Contracts

- `designos_critique` input `{ "frame_id": 120, "post_comments": false, "include_image": true }` → `{ "critique_id": "c-7", "frame": { "id": 120, "bounds": [0, 0, 1440, 900] }, "axes": ["layout","typography","spacing","iconography","depth","consistency"], "checklist": [ { "id": "surface.web-app.body-min-size", "text": "Body text is at least 13 px" } ], "lint": [ … ], "outline": [ … ], "image": <image content> }`.
- `designos_submit_critique` input `{ "critique_id": "c-7", "findings": [ { "target": { "region": { "x": 40, "y": 120, "w": 600, "h": 80 } }, "basis": { "axis": "spacing" }, "severity": "major", "message": "Section header crowds the cards.", "repair_target": "24 px between header and grid, matching other sections." } ], "scores": { "spacing": 6 } }` → `{ "critique_id": "c-7", "findings": [ { "id": "F1", … } ], "passed": false, "image": <annotated image> }`.
- `designos_resolve_findings` → `{ "resolved": ["F1"] }`.
- Errors (SC-1): `frame_in_progress` (`safe_to_retry: true` after the flag clears), `critic_is_generator` (`safe_to_retry: false`; corrected example: none, details explain that another agent run must critique), `finding_out_of_bounds` (corrected example clamps the region to the frame), `critique_invalid`, `critique_unknown`, `finding_unknown`.

## 6. File-format mapping

`.designos` native. `.fig`: not exported; comments posted by rule 6 export as ordinary comments.

## 7. UI

No new panel. Findings posted as comments show in the existing comments UI. A finding's numbered box image is returned to the agent; Studio shows the same boxes only through comments.

## 8. Edge cases

1. A frame partly authored by the critic run (it fixed one node earlier): refused; the rule is strict by design.
2. A human-only frame (no agent authorship): any run may critique.
3. Target node deleted before submission: `critique_invalid` naming the node.
4. Region exactly on the frame edge: allowed (0.5 px tolerance).
5. Rotated frame: bounds are the frame's world axis-aligned bounds; regions are checked against them.
6. Very large frame (10k nodes): outline truncated to 400 rows with `truncated: true`.
7. Save and reload between scaffold and submit: the critique id persists; submission works.

## 9. Non-goals

- Applying repairs (E15-35).
- Running the critic agent (any agent CLI can call these tools; E15-36/E15-37 orchestrate runs).

## 10. Test plan

Seam: `cargo test -p designos-mcp critique` with two stub agent runs (two MCP sessions with distinct run ids on one document), plus a screenshot of the annotated PNG.

Fail-before: the tools do not exist.

| Test | Input | Expected |
|---|---|---|
| `authoring_run_refused` (AC1) | run A creates frame F; run A calls `designos_critique(F)` | `critic_is_generator`; no critique stored |
| `other_run_allowed` | run B critiques F | scaffold returned |
| `out_of_bounds_region_rejected` (AC2) | region x = frame.right − 10, w = 40 | `finding_out_of_bounds`; nothing stored |
| `annotated_boxes_match_bounds` (AC3) | 3 findings, scale 0.5 | detected box edges (scan for the box colour) within 1 px of `round(bounds × 0.5)` |
| `in_progress_frame_skipped` | F flagged in progress | `frame_in_progress` |
| `comments_posted` | `post_comments: true`, 2 findings | 2 threads at the finding corners with `F1`, `F2` prefixes |
| `lint_finding_auto_resolves` | finding based on a contrast lint id; fix the colour; critique again | finding status `resolved` |
| `authorship_survives_reload` | save, reload | `agent.author` and `agent_runs` equal |

Visual evidence: the annotated PNG from `annotated_boxes_match_bounds`, attached to the PR.

## 11. Performance budgets

- Authorship recording adds ≤ 1% to write tool time (one `u32` per touched node).
- Scaffold for a 10k-node frame ≤ 300 ms excluding the screenshot.
- Annotated render uses the CPU path in `designos-mcp`; never on the Studio UI thread (CLAUDE.md §4).

## 12. Dependencies and build order

- Needs #572 (RD-3), #238 (T7.1), E15-7 (#592), E15-9 (#594), E15-21 (#609), E15-31.
- Unblocks E15-35, E15-37, E15-47 (critique skill).

## 13. Owner rulings that apply

- R4.13: the critic is the user's agent CLI; the engine only validates and records.
- Parallel agents are hard-locked to their frames (RD-3); critique reads freely but writes nothing except stored critiques and optional comments.
- Generator is never grader.
