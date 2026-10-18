# E15-29 Add suite S3 edits on large self-generated files

- Issue: #618 (task E15-29, epic #585, M / P0)
- Spec base: `main` at `767acf9`.
- Files: `crates/designos-eval/suites/s3/{tasks.toml,split.lock,README.md}` (new), `crates/designos-eval/src/metrics/drift.rs` (new), `crates/designos-eval/src/metrics/node_hash.rs` (new), `crates/designos-eval/tests/fixtures/s3/` (new drift fixtures).

## 1. Summary and user story

As the team, we want to measure whether an agent can edit a big existing file correctly without disturbing anything it was not asked to touch, because editing complex files is weaker than greenfield generation.

## 2. Current state

- FACT: E15-24 generates 100k and 1M node documents deterministically; E15-17 (#602) adds a streaming query with paging and a node budget so agents can read large documents.
- FACT: documents are `BTreeMap<NodeId, Node>` (`crates/designos-doc/src/lib.rs:147`); nodes reference variables, styles and components by id (`node.rs:110,122,128`).
- FACT: CLAUDE.md §4.1 forbids `to_json` for hashing large documents and mandates a zero-allocation streaming hasher (`HashSerializer` in `crates/designos-app/src/home/session.rs`).

## 3. Behaviour rules

1. **Tasks.** 30 edit tasks, each run on both the 100k and the 1M fixture of the same profile and seed (`start = "gen:default:<seed>"` with `nodes = 100000` and `1000000`), so 60 task instances (AC2). Task kinds (5 each): rename and restyle a component variant; change one screen's layout (convert a section to a 3-column grid); fix every contrast failure inside one named frame; replace hard-coded colours with variables on one page; add a new state frame next to a named screen; update a variable value in one mode only and fix the frames it breaks.
2. **Scope.** Each task declares `scope`: a list of frame names, component names, variable names or page names that may change. The scope closure is computed on the start document: the named nodes and their descendants, plus for components every instance of them, plus for variables every node bound to them (directly or through styles), plus new nodes created inside scope containers.
3. **Structural drift.** For every node outside the scope closure, compare a per-node hash before and after. `node_hash` streams the node's serialisable fields into a 64-bit FNV-1a hasher through a `serde::Serializer` that writes bytes, never building a JSON string. Added or removed nodes outside scope also count. `structural_drift` = count of out-of-scope nodes changed, added or removed. Variables, styles and components outside scope are hashed the same way (`token_drift`).
4. **Pixel drift.** Top-level frames on every page are classified:
   - in scope → skipped;
   - out of scope and (structurally changed, or depending on a changed variable, style or component) → rendered before and after at scale 0.25 on the CPU path and compared with `designos-diff::compare` at tolerance 0 with anti-alias detection off;
   - every other out-of-scope frame → a canary sample of 20 frames per page chosen with the task seed is rendered and compared the same way.
   `pixel_drift_frames` = frames with any mismatched pixel.
5. **Drift-free.** `drift_free = 1` when `structural_drift == 0`, `token_drift == 0` and `pixel_drift_frames == 0`; else 0. AC1: an edit that touches an unrelated frame is caught.
6. **Task success.** Each task also has a deterministic `check` (a small predicate in `tasks.toml`, evaluated on the result): for example `frame("Billing / Settings").layout.grid_columns == 3`, or `lint(frame("Invoices")).contrast == 0`. Predicates use a tiny expression language over a read-only document API: `frame(name)`, `component(name)`, `variable(name).value(mode)`, `lint(node).<family>`, `count(selector)`, comparisons and `and`/`or`. `task_success ∈ {0,1}`.
7. **Read discipline metric.** `read_before_write` = 1 when the transcript shows a read tool call (E15-17 query, get_node, screenshot) touching the edited node before its first write; reported, not gated.
8. **Timeouts.** 1M tasks use `timeout_s = 1800`.

## 4. Data model / schema

```toml
[[task]]
id = "s3-0004"
kind = "layout"
brief = "In the screen 'Billing / Overview', turn the 'Plans' section into a three-column grid of plan cards. Change nothing else."
scope = ["frame:Billing / Overview"]
check = "frame('Billing / Overview').child('Plans').layout.grid_columns == 3"
sizes = [100000, 1000000]
seed = 4242
```

```rust
pub struct DriftReport { pub structural_drift: u32, pub token_drift: u32, pub pixel_drift_frames: u32,
    pub drifted: Vec<DriftItem> /* first 50, with node id, name, kind */, pub drift_free: bool }
pub fn drift(before: &Document, after: &Document, scope: &ScopeClosure, seed: u64) -> DriftReport;
```

JSONL `run.metrics`: `drift.structural`, `drift.token`, `drift.pixel_frames`, `drift.free`, `task_success`, `read_before_write`.

## 5. Contracts

- `designos-eval drift --before a.designos --after b.designos --scope "frame:Billing / Overview"` prints `DriftReport` JSON.
- `designos-eval run --suite s3 …` per E15-25.

## 6. File-format mapping

None.

## 7. UI

None.

## 8. Edge cases

1. Auto layout reflow inside scope moves a node whose parent is out of scope: impossible by construction (a child's position belongs to its parent); a scope frame resizing its out-of-scope parent's layout counts as drift, which is intended.
2. The agent saves with a different page order: page order is part of the document hash outside scope and counts as structural drift.
3. Undo history differences are ignored (not part of node hashes).
4. Font fallback differences between machines: renders compare before/after on the same machine and run, so fonts are constant.
5. Scope name not found in the start document: the task fails to load with `scope_unresolved` (fixture bug), never silently empty.
6. The agent creates a node at the page root outside any scope frame: counted as an added out-of-scope node.

## 9. Non-goals

- Measuring the quality of the edit beyond its check predicate (the judge may score it later).
- Generating the fixtures (E15-24).

## 10. Test plan

Seam: `cargo test -p designos-eval metrics::drift` with drift fixtures, and a stub run over the suite.

Fail-before: the module does not exist.

| Test | Input | Expected |
|---|---|---|
| `unrelated_frame_edit_is_caught` (AC1) | 10k generated doc; stub edits frame A (scope) and renames a text in frame B | `structural_drift ≥ 1`, `pixel_drift_frames ≥ 1`, `drift_free = false`, B named in `drifted` |
| `in_scope_edit_is_clean` | stub edits only frame A | `drift_free = true` |
| `variable_change_reaches_dependants` | change a variable in scope; frames bound to it are out of scope | those frames rendered and reported |
| `every_task_runs_on_both_sizes` (AC2) | tasks.toml | 30 tasks, each with sizes 100000 and 1000000 |
| `node_hash_streams_without_json` | hash a 100k doc | equals a reference hash; allocation counter shows no buffer ≥ 1 MiB |
| `check_predicates_parse` | every task | predicate parses and evaluates to false on the start document |

Visual evidence: terminal capture of the drift fixture result and the before/after crops of the drifted frame.

## 11. Performance budgets

- Drift on 1M nodes: hashing ≤ 4 s; pixel checks ≤ 60 s (bounded by the canary sample and changed frames).
- Hashing never allocates a document-sized buffer (CLAUDE.md §4.1).

## 12. Dependencies and build order

- Needs E15-24, E15-25, E15-17 (#602).
- Unblocks E15-33 (CI gate replays S3).

## 13. Owner rulings that apply

- The eval never uses client files; only self-generated large files.
- Full scope: every task on both large sizes.
