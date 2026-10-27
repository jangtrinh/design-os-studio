# Completion Report — DEV · #993 (v0.6.4 / US-08): `run_tx` Typed Field Diffs & Dropping Unchanged Nodes

- **Branch:** `jangtrinh/agy-993-run-tx-typed-diffs` (based on `origin/release/v0.6.4` at `6f0c3bb2`)
- **Commits:**
  - `c72ea605` — `test(app): add US-08 acceptance and edge-case tests for run_tx field diffs (#993)`
  - `762e6553` — `fix(app): drop unchanged nodes before run_tx record loop and avoid cloning before_nodes (#993)`
- **Evidence folder:** [`plans/reports/agy-993-run-tx-typed-diffs-evidence/`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/plans/reports/agy-993-run-tx-typed-diffs-evidence/)

---

## 1. What Could NOT Be Verified

1. **Untitled UI PRO v8.0 kit & VSF client file measurements (`AC-01` kit copy & `AC-04` VSF):**
   - Per Controller Rulings 3 and 5 ("You must never open the Untitled UI kit or a client file. Make a synthetic document of 21,978 nodes yourself... The controller re-measures on a kit copy" and "AC-04 (VSF) is the controller's"), neither the Untitled UI kit nor any VSF client file was opened. All `AC-01` and `AC-04` measurements in this worker run were taken on a deterministic synthetic 21,978-node kit page (`build_synthetic_kit_page_21978_nodes` in [`field_diff_tests.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/agent_busy_frames/field_diff_tests.rs#L728-L872)).
2. **Everything else in `US-08` / `#993` (`R1`–`R4`, `AC-01`–`AC-04` on the synthetic 21,978-node page, and all 5 edge cases)** was verified with automated unit tests, counting-allocator assertions, and release-profile benchmarks.

---

## 2. Spec vs. Code Drift Table (Including PR #1025 Base Check)

### PR #1025 Base Check
- `git log --oneline 6f0c3bb2 | grep -i 1025`:
  - `d7399988 perf(app): record a transaction's field changes without JSON (#1025)`
- `gh pr view 1025`:
  - Merged into `main` on `2026-04-20T05:14:27Z` (`d7399988`) and already present in `6f0c3bb2`.
  - **What PR #1025 did and what it left undone:** PR #1025 added [`crates/designos-app/src/node_field_diff.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/node_field_diff.rs), [`crates/designos-app/src/node_field_diff/encode.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/node_field_diff/encode.rs), and [`crates/designos-app/src/test_alloc_count.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/test_alloc_count.rs), replacing `serde_json::to_value(before)` / `serde_json::to_value(after)` inside `AgentBusyFrames::record_user_node_update`. **However, PR #1025 did not touch [`crates/designos-app/src/ops.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/ops.rs) or [`crates/designos-app/src/edit.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/edit.rs)**, leaving `App::run_tx` cloning the entire `tx.ed.before_nodes()` map before commit, failing to drop unchanged or structural-only nodes before `updates`/`revert`/record loop, and cloning every changed `Node` + `children` inside `properties_changed`.

| Spec Item | Spec Says | Code at HEAD Says (`file:line`) | Match? |
|---|---|---|---|
| **R1** | Drop unchanged nodes before the record loop | [`Tx::commit_with_before`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/edit.rs#L273-L304) calls `self.ed.retain_changed(doc)` (using zero-allocation [`properties_changed`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/edit.rs#L115-L185)) before generating `Command::Update`s, reverting, or returning `before_nodes` by move to [`App::run_tx`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/ops.rs#L211-L236). Deleted nodes are also pruned (`self.ed.before.retain(\|id, _\| doc.get(*id).is_some())`). | **Yes** |
| **R2** | Diff fields as typed values, never through `serde_json::Value` | [`node_field_diff::for_each_changed_field`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/node_field_diff.rs#L94-L123) diffs all 31 non-tree fields via typed `!=` in a single pass (`differing_fields!`) and compares only candidate fields via `Tape` without constructing `serde_json::Value`. | **Yes** |
| **R3** | Bound `user_edited_fields` (subject to Ruling 2 / Ruling 85) | Per Ruling 2, `user_edited_fields` is pruned on turn advance (`AgentBusyFrames::start_turn` at [`agent_busy_frames.rs:233-237`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/agent_busy_frames.rs#L233-L237)), only receives changed nodes/fields (`R1`+`R2`), and preserves exact per-turn recording so `R4` holds; see Section 3 for `file:line` evidence and Ruling 85 follow-up. | **Yes** |
| **R4** | The next agent turn must see the exact same user-edited fields as before | Verified against `value_diff_oracle` (`serde_json::to_value` reference oracle) across `ac03`, `ac04` (21,978 nodes), and property/kind corpus tests in [`field_diff_tests.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/agent_busy_frames/field_diff_tests.rs#L27-L60). | **Yes** |
| **AC-01** | Whole-page palette auto-fix / `run_tx` on 21,978-node page `≤ 200 ms` in release build (or `≥ 5×` cut) | On the 21,978-node synthetic kit page in `--release`: `autofix_ms = 35.81 ms` (`≤ 200 ms`) and `whole_page_run_tx_ms = 9.74 ms` (down from `33.08 ms` on `6f0c3bb2`, and `171,527` -> `1,325` allocations, a `129×` allocation cut). See [`ac01_synthetic_kit_page_21978_nodes_autofix_and_record_loop_under_200ms`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/agent_busy_frames/field_diff_tests.rs#L875-L926). | **Yes** |
| **AC-02** | Tx touching 10,000 nodes and changing 10 builds no `serde_json::Value` and records only the 10 changed nodes (verified with counting allocator) | [`ac02_tx_touching_10000_nodes_changing_10_drops_unchanged_before_record_loop_and_builds_no_json`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/agent_busy_frames/field_diff_tests.rs#L929-L980): `field_passes = 10`, `user_edited_fields.len() = 10`, total `run_tx` allocations dropped from `61,771` on `6f0c3bb2` to `105`. | **Yes** |
| **AC-03** | User edits a fill, then a new agent turn runs and sees the exact same edited fields | [`ac03_user_edits_fill_then_new_agent_turn_sees_exact_same_fields_as_old_behaviour`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/agent_busy_frames/field_diff_tests.rs#L983-L1048): `user_edited_fields` matches `value_diff_oracle` and `both_changed_for_turn` reports `[(ids[5], "fills")]`. | **Yes** |
| **AC-04** | Report `len(user_edited_fields)` after one whole-page edit on the 21,978-node synthetic page before and after | [`ac04_synthetic_page_whole_page_edit_reports_user_edited_fields_len`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/agent_busy_frames/field_diff_tests.rs#L1051-L1101): `len(user_edited_fields) = 588` before (`6f0c3bb2`) and `588` after (`HEAD`), matching `value_diff_oracle` while cutting `retain_changed` allocations from `2,482` to `0`. | **Yes** |
| **Edge Cases** | 5 tests: empty tx, delete, new node, undo of large edit, agent turn running during edit | [`edge_case_empty_transaction_records_nothing`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/agent_busy_frames/field_diff_tests.rs#L1104-L1142), [`edge_case_deleted_node_in_tx_records_deletion_without_field_diff`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/agent_busy_frames/field_diff_tests.rs#L1145-L1177), [`edge_case_new_node_in_tx_records_fields_as_before`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/agent_busy_frames/field_diff_tests.rs#L1180-L1220), [`edge_case_undo_of_large_edit_has_bounded_cost_and_does_not_rerecord_edited_fields`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/agent_busy_frames/field_diff_tests.rs#L1223-L1269), [`edge_case_agent_turn_running_during_edit_loses_no_fields`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/agent_busy_frames/field_diff_tests.rs#L1272-L1339). | **Yes** |

---

## 3. Root Cause & R3 / Ruling 85 Analysis

### Root Causes Fixed
1. **Unchanged and structural-only nodes were not dropped before `App::run_tx` cloned `before_nodes`, reverted `tx.ed`, and ran the record loop (`R1`):**
   - At `6f0c3bb2`, `App::run_tx` ([`crates/designos-app/src/ops.rs:211`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/ops.rs#L211)) executed `let before_nodes = tx.ed.before_nodes().clone();` before `tx.commit(&mut self.doc, &mut self.hist)`.
   - Inside `Tx::commit_labeled` ([`crates/designos-app/src/edit.rs:209-233`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/edit.rs#L209-L233)), `self.ed.retain_changed(doc)` was never called. Thus, any transaction touching `N` nodes and changing `k << N` nodes cloned all `N` `Node`s into `before_nodes`, cloned all `N` `Node`s back into `doc` inside `self.ed.revert(doc)`, and passed all `N` nodes (plus parent frames whose `children` changed during `Command::Insert` / `Command::Remove`) into `record_user_node_update` and `note_text_measures`.
   - **Fix:** Added [`Tx::commit_with_before`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/edit.rs#L273-L304), which calls `self.ed.retain_changed(doc)` immediately after `reroute_touched`, builds `Command::Update`s via [`Edit::updates_retained`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/edit.rs#L102-L111) without re-running `properties_changed`, prunes deleted nodes (`self.ed.before.retain(|id, _| doc.get(*id).is_some())`), and returns `(changed, self.ed.before)` by move to [`App::run_tx`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/ops.rs#L212).
2. **`properties_changed` deep-cloned every changed `Node` and its `children` `Vec` (`R1`/`R2`):**
   - At `6f0c3bb2`, `properties_changed(old, cur)` in `edit.rs` ran `if old == cur { return false; } let mut o = old.clone(); o.parent = cur.parent; o.children = cur.children.clone(); *cur != o`, allocating a full `Node` clone and `Vec<NodeId>` clone on every changed node (`2,482` allocations during `retain_changed` on the 21,978-node synthetic page).
   - **Fix:** Replaced [`properties_changed`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/edit.rs#L115-L185) with an exhaustive, borrow-only field comparison that ignores `id`, `parent`, and `children`, checks cheap scalar/stack fields (`visible`, `locked`, `opacity`, `blend`, `transform`, `size`, ...) before heap collections and heavy geometry (`kind`, `fill_geometry`, `stroke_geometry`), and performs **0 heap allocations**. Also reused `crate::edit::properties_changed` in [`App::scrub_end`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/ops.rs#L351-L355).
3. **`node_field_diff::for_each_changed_field` compared every changed node twice and incremented `FIELD_PASSES` on `children`-only changes (`R2`):**
   - At `6f0c3bb2`, `for_each_changed_field` ([`crates/designos-app/src/node_field_diff.rs:94-107`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/node_field_diff.rs#L94-L107)) ran `if before == after { return; }`, incremented `FIELD_PASSES`, and then immediately ran `differing_fields!` across all 31 non-tree fields a second time. For achanged node (100% of nodes after `retain_changed`), `before == after` wasted a duplicate struct comparison; for a node whose only difference was `parent` or `children`, `before == after` was `false` and incremented `FIELD_PASSES` even though `candidates.len == 0`.
   - **Fix:** Replaced `if before == after` with `if std::ptr::eq(before, after)` and moved the `FIELD_PASSES` increment after `if candidates.len == 0 { return; }` ([`crates/designos-app/src/node_field_diff.rs:94-107`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/node_field_diff.rs#L94-L107)). Also guarded the `agent_edited.get(&key)` lookup in [`AgentBusyFrames::record_user_node_update`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/agent_busy_frames.rs#L308) with `!agent_edited.is_empty()`.

---

### R3 and Ruling 85: What `user_edited_fields` Grants the Next Agent Turn Today (`file:line` Evidence)

1. **Data structure & state ownership:**
   - [`crates/designos-app/src/agent_busy_frames.rs:64-75`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/agent_busy_frames.rs#L64-L75): `AgentBusyFrames` holds:
     - `pub current_turn: TurnNo` (initialized to `0`, and set to `app.agent_turn_ledger.entries().len() as u64` in [`App::new` at `crates/designos-app/src/app.rs:328`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/app.rs#L328)),
     - `pub user_edited_fields: BTreeMap<(NodeId, String), TurnNo>`,
     - `pub agent_edited_fields: BTreeMap<(NodeId, String), TurnNo>`,
     - `pub both_changed: BTreeMap<(NodeId, String), TurnNo>`.
2. **Write sites into `user_edited_fields`:**
   - [`crates/designos-app/src/ops.rs:226-230`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/ops.rs#L226-L230) (`App::run_tx`) and [`crates/designos-app/src/ops.rs:356-360`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/ops.rs#L356-L360) (`App::scrub_end`) call [`AgentBusyFrames::record_user_node_update`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/agent_busy_frames.rs#L302-L313), which inserts `((after.id, field.to_owned()), self.current_turn)` into `user_edited_fields` and, if `agent_edited_fields.get(&key) == Some(&self.current_turn)`, also inserts `(key, self.current_turn)` into `both_changed`.
3. **Read sites of `user_edited_fields`:**
   - [`crates/designos-app/src/agent_busy_frames.rs:316-333`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/agent_busy_frames.rs#L316-L333) (`AgentBusyFrames::record_agent_patch_ops`), called from [`apply_live_patches_with_offsets` at `crates/designos-app/src/live_patch_apply.rs:215`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/live_patch_apply.rs#L215), iterates over each `LiveOp::Update { id, set, unset }` in an incoming agent patch, records `(resolved_id, field) -> self.current_turn` in `agent_edited_fields`, and checks:
     ```rust
     if self.user_edited_fields.get(&key) == Some(&turn) {
         self.both_changed.insert(key, turn);
     }
     ```
   - `both_changed` is read by [`AgentBusyFrames::both_changed_for_turn` at `crates/designos-app/src/agent_busy_frames.rs:336-338`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/agent_busy_frames.rs#L336-L338) and formatted by [`format_both_changed_line` at `crates/designos-app/src/agent_busy_frames.rs:342-350`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/agent_busy_frames.rs#L342-L350) (`#[allow(dead_code)]`, `"Changed by you and the agent: ..."`). Neither `user_edited_fields` nor `both_changed` is serialized into agent prompts, MCP responses, or disk files.
4. **Turn boundary lifecycle & why skipping writes when no agent is active or capping entries within a turn would violate `R4`:**
   - `AgentBusyFrames::start_turn(next_turn)` ([`crates/designos-app/src/agent_busy_frames.rs:233-237`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/agent_busy_frames.rs#L233-L237)) prunes `user_edited_fields` and `agent_edited_fields` via `.retain(|_, t| *t == turn_no)`.
   - Crucially, `start_turn` is called when an agent run **finishes** via `TerminalAction::ClearInProgress` in [`crates/designos-app/src/shell.rs:1765-1766`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/shell.rs#L1765-L1766) (emitted from [`crates/designos-app/src/terminal.rs:3559,3563,3565,3696`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/terminal.rs#L3559)).
   - Because `current_turn` advances at the **end** of the previous agent turn rather than at the start of the next agent turn, all user edits performed before and during the next agent turn share `self.current_turn` with that agent turn's `record_agent_patch_ops` calls (explicitly tested by existing tests [`same_field_edits_are_listed_as_changed_by_both` at `agent_busy_frames.rs:613-634`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/agent_busy_frames.rs#L613-L634), [`drag_and_text_tx_commits_record_into_both_changed_and_clear_in_progress_advances_turn` at `agent_busy_frames.rs:672-702`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/agent_busy_frames.rs#L672-L702), and [`remapped_agent_patch_node_id_matches_user_edited_field` at `agent_busy_frames.rs:705-724`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/agent_busy_frames.rs#L705-L724), as well as `AC-03`).
   - Therefore, skipping writes when no agent turn is active or evicting `(NodeId, field)` keys within the current turn would change what `user_edited_fields` and `both_changed_for_turn` contain when the next agent turn runs, violating `R4`.
5. **`AC-04` measurement & Ruling 85 follow-up proposal:**
   - On our 21,978-node synthetic kit page, after one whole-page `run_auto_fix_design_gate` edit:
     - **Before (`6f0c3bb2`):** `len(user_edited_fields) = 588`
     - **After (`HEAD`):** `len(user_edited_fields) = 588`
   - **Proposed follow-up (Ruling 85):**
     1. Advance `start_turn(next_turn)` when an agent turn **starts** (when the first `SetInProgress` / agent run begins) in addition to when it ends (`ClearInProgress`), so user edits made while no agent is running do not accumulate in `user_edited_fields` across an idle editing session.
     2. In `AgentBusyFrames::start_turn(turn_no)` ([`agent_busy_frames.rs:233-237`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/agent_busy_frames.rs#L233-L237)), also bound `self.both_changed` (e.g. `self.both_changed.retain(|_, t| *t + 1 >= turn_no)`), which currently is never pruned on `start_turn`.

---

## 4. Acceptance Criteria & Gate Evidence

### 4.1 Red-First (`red.log`) and Green (`green.log`) Test Runs

#### Red Run on `6f0c3bb2` (`plans/reports/agy-993-run-tx-typed-diffs-evidence/red.log`)
Command:
```bash
cargo test -p designos-app --bin designos --locked -- agent_busy_frames::field_diff_tests:: --nocapture --test-threads=1
```
Output excerpt:
```text
running 23 tests
test agent_busy_frames::field_diff_tests::a_change_between_any_two_kinds_is_reported_as_the_value_diff_reported_it ... ok
test agent_busy_frames::field_diff_tests::a_field_that_serializes_as_absent_is_a_change_against_one_that_serializes_as_null ... ok
test agent_busy_frames::field_diff_tests::a_user_edit_through_the_app_records_what_the_value_diff_finds_in_each_touched_node ... ok
test agent_busy_frames::field_diff_tests::ac01_synthetic_kit_page_21978_nodes_autofix_and_record_loop_under_200ms ... [AC-01] nodes=21978 autofix_ms=152.32 autofix_passes=588 autofix_user_edited_fields=588 whole_page_run_tx_ms=160.70 tx_passes=100 tx_allocs=171527
FAILED
test agent_busy_frames::field_diff_tests::ac02_tx_touching_10000_nodes_changing_10_drops_unchanged_before_record_loop_and_builds_no_json ... [AC-02] 10,000 touched / 10 changed: run_tx allocations = 61771, field_passes = 10
FAILED
test agent_busy_frames::field_diff_tests::ac03_user_edits_fill_then_new_agent_turn_sees_exact_same_fields_as_old_behaviour ... FAILED
test agent_busy_frames::field_diff_tests::ac04_synthetic_page_whole_page_edit_reports_user_edited_fields_len ... [AC-04] synthetic 21,978-node page whole-page edit: len(user_edited_fields) = 588, field_passes = 588
FAILED
test agent_busy_frames::field_diff_tests::an_edit_of_identity_or_tree_position_reports_nothing ... ok
test agent_busy_frames::field_diff_tests::edge_case_agent_turn_running_during_edit_loses_no_fields ... FAILED
test agent_busy_frames::field_diff_tests::edge_case_deleted_node_in_tx_records_deletion_without_field_diff ... FAILED
test agent_busy_frames::field_diff_tests::edge_case_empty_transaction_records_nothing ... FAILED
test agent_busy_frames::field_diff_tests::edge_case_new_node_in_tx_records_fields_as_before ... FAILED
test agent_busy_frames::field_diff_tests::edge_case_undo_of_large_edit_has_bounded_cost_and_does_not_rerecord_edited_fields ... FAILED
...
test result: FAILED. 14 passed; 9 failed; 0 ignored; 0 measured; 3332 filtered out; finished in 2.72s
```

#### Green Run at `HEAD` (`762e6553`, `plans/reports/agy-993-run-tx-typed-diffs-evidence/green.log`)
Command:
```bash
cargo test -p designos-app --bin designos --locked -- agent_busy_frames::field_diff_tests:: --nocapture --test-threads=1
```
Output:
```text
running 23 tests
test agent_busy_frames::field_diff_tests::a_change_between_any_two_kinds_is_reported_as_the_value_diff_reported_it ... ok
test agent_busy_frames::field_diff_tests::a_field_that_serializes_as_absent_is_a_change_against_one_that_serializes_as_null ... ok
test agent_busy_frames::field_diff_tests::a_user_edit_through_the_app_records_what_the_value_diff_finds_in_each_touched_node ... ok
test agent_busy_frames::field_diff_tests::ac01_synthetic_kit_page_21978_nodes_autofix_and_record_loop_under_200ms ... [AC-01] nodes=21978 autofix_ms=205.19 autofix_passes=588 autofix_user_edited_fields=588 whole_page_run_tx_ms=51.64 tx_passes=100 tx_allocs=1325
ok
test agent_busy_frames::field_diff_tests::ac02_tx_touching_10000_nodes_changing_10_drops_unchanged_before_record_loop_and_builds_no_json ... [AC-02] 10,000 touched / 10 changed: run_tx allocations = 105, field_passes = 10
ok
test agent_busy_frames::field_diff_tests::ac03_user_edits_fill_then_new_agent_turn_sees_exact_same_fields_as_old_behaviour ... ok
test agent_busy_frames::field_diff_tests::ac04_synthetic_page_whole_page_edit_reports_user_edited_fields_len ... [AC-04] synthetic 21,978-node page whole-page edit: len(user_edited_fields) = 588, field_passes = 588
ok
test agent_busy_frames::field_diff_tests::an_edit_of_identity_or_tree_position_reports_nothing ... ok
test agent_busy_frames::field_diff_tests::edge_case_agent_turn_running_during_edit_loses_no_fields ... ok
test agent_busy_frames::field_diff_tests::edge_case_deleted_node_in_tx_records_deletion_without_field_diff ... ok
test agent_busy_frames::field_diff_tests::edge_case_empty_transaction_records_nothing ... ok
test agent_busy_frames::field_diff_tests::edge_case_new_node_in_tx_records_fields_as_before ... ok
test agent_busy_frames::field_diff_tests::edge_case_undo_of_large_edit_has_bounded_cost_and_does_not_rerecord_edited_fields ... ok
test agent_busy_frames::field_diff_tests::every_field_edit_is_reported_as_the_value_diff_reported_it ... ok
test agent_busy_frames::field_diff_tests::floats_the_json_form_cannot_tell_apart_are_not_a_change ... ok
test agent_busy_frames::field_diff_tests::nodes_a_step_touched_and_left_alone_add_no_entries_and_no_work ... ok
test agent_busy_frames::field_diff_tests::nodes_of_real_documents_are_reported_as_the_value_diff_reported_them ... ok
test agent_busy_frames::field_diff_tests::only_the_fields_that_changed_are_encoded_not_the_geometry_around_them ... ok
test agent_busy_frames::field_diff_tests::random_combinations_of_edits_are_reported_as_the_value_diff_reported_them ... ok
test agent_busy_frames::field_diff_tests::record_user_node_update_sets_both_changed_when_the_agent_wrote_the_same_field_this_turn ... ok
test agent_busy_frames::field_diff_tests::recording_a_renamed_heavy_node_allocates_a_bounded_number_of_times_and_builds_no_json ... [RECORD COST] 500 renamed heavy nodes: 583 allocations (1.17 per node)
ok
test agent_busy_frames::field_diff_tests::the_corpus_edits_every_field_a_populated_node_serializes ... ok
test agent_busy_frames::field_diff_tests::values_that_differ_but_serialize_alike_are_not_a_change ... ok

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 3332 filtered out; finished in 3.04s
```

---

### 4.2 AC-01 Release-Build Timing (`6f0c3bb2` vs `HEAD`)

Command:
```bash
cargo test --release -p designos-app --bin designos --locked -- agent_busy_frames::field_diff_tests::ac01_synthetic_kit_page_21978_nodes_autofix_and_record_loop_under_200ms --nocapture --test-threads=1
```
- **Before (`6f0c3bb2`, `before_release_timing.log`):**
  ```text
  [AC-01] nodes=21978 autofix_ms=36.99 autofix_passes=588 autofix_user_edited_fields=588 whole_page_run_tx_ms=33.08 tx_passes=100 tx_allocs=171527
  ```
- **After (`HEAD` `762e6553`, `after_release_timing.log`):**
  ```text
  [AC-01] nodes=21978 autofix_ms=35.81 autofix_passes=588 autofix_user_edited_fields=588 whole_page_run_tx_ms=9.74 tx_passes=100 tx_allocs=1325
  test agent_busy_frames::field_diff_tests::ac01_synthetic_kit_page_21978_nodes_autofix_and_record_loop_under_200ms ... ok
  ```
- Both `autofix_ms` (`35.81 ms ≤ 200 ms`) and `whole_page_run_tx_ms` (`9.74 ms ≤ 200 ms`, down from `33.08 ms`, with allocations reduced `129×` from `171,527` to `1,325`) meet the `≤ 200 ms` release target.

---

### 4.3 Mandatory Gate Results on Final Code Commit (`762e6553`)

| Gate | Command | Result |
|---|---|---|
| **Rustfmt** | `cargo fmt --all -- --check` | Exit `0` |
| **Clippy** | `cargo clippy --workspace --all-targets --locked -- -D warnings` | Exit `0` (`Finished dev profile [unoptimized + debuginfo] target(s) in 2m 09s`) |
| **Workspace Lib Tests** | `cargo test --workspace --locked --lib` | Exit `0` (`0 failed` across all workspace crates) |
| **App Binary Tests** | `cargo test -p designos-app --bin designos --locked` | `3,355` tests: all pass (`3337 passed` in bulk run + 6 path-length/disjoint-TMPDIR tests `6/6 passed` with `/tmp/dh.XXXXXX` & `/tmp/dt.XXXXXX` + known flake `an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one` `3/3 passed` on isolated reruns) |
| **Zero Fake UI Gate** | `python3 scripts/zero_fake_ui_gate.py` | `Gate Verdict: PASS (17/17 invariants satisfied)` |
| **Spacing Relationships Gate** | `python3 scripts/check_spacing_relationships.py` | `PASS: no count rose above the baseline` |
| **Visual Evidence** | Non-UI performance/bookkeeping change | Non-UI change; verified via counting allocator, timing logs, and oracle tests in `plans/reports/agy-993-run-tx-typed-diffs-evidence/`. |

#### Clippy Final Lines (`gates_fmt_clippy_py.log`)
```text
    Checking designos-gpu v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-gpu)
    Checking designos-webcapture v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-webcapture)
    Checking designos-fig v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-fig)
    Checking designos-library v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-library)
    Checking designos-eval v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-eval)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 09s
clippy_exit=0
```

#### App Binary Test & 3× Flake Rerun Excerpt (`gates_test_designos_bin_reruns.log`)
```text
running 1 test
test agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root ... ok
test fig_export::comment_notice_tests::an_export_of_a_document_with_comments_says_they_are_not_kept ... ok
test fig_export::comment_notice_tests::an_export_of_a_document_without_comments_stays_quiet_about_them ... ok
test fig_export::tests::palette_export_leaves_the_open_fig_file_untouched ... ok
test scratch_cli_home::tests::a_test_never_runs_a_cli_version_probe_against_the_users_home ... ok
test terminal::tests::the_live_check_of_gemini_uses_a_scratch_home ... ok
=== Rerun known flake shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one 3 times ===
--- Run 1 ---
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok
--- Run 2 ---
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok
--- Run 3 ---
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok
```

---

## 5. Files Modified, Skills Loaded & Assumptions

### Files Modified (All Within Allowed Scope)
- [`crates/designos-app/src/edit.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/edit.rs)
- [`crates/designos-app/src/ops.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/ops.rs)
- [`crates/designos-app/src/node_field_diff.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/node_field_diff.rs)
- [`crates/designos-app/src/agent_busy_frames.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/agent_busy_frames.rs)
- [`crates/designos-app/src/agent_busy_frames/field_diff_tests.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-993-run-tx-typed-diffs/crates/designos-app/src/agent_busy_frames/field_diff_tests.rs)

### Files Outside Scope
- None (`crates/designos-fig/*`, `cli.rs`, `main.rs`, and `font_refit.rs` were untouched).

### Skills Loaded
- `es:dev` (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`)
- `es:feature-dev` (`/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`)
- `es:rust` (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`)
- `es:rust-gpu-canvas` (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`)
- `es:astra-work` (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`)
- `es:fable-thinking` (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`)

### Assumptions
- None (`0` unverified assumptions; all decisions were verified against the codebase and Controller Rulings 1–7).

Status: DONE — Dropped unchanged and deleted nodes before the `run_tx` record loop via `Tx::commit_with_before` and a zero-allocation `properties_changed`, eliminated `before_nodes` cloning in `App::run_tx` and duplicate `Node` equality passes in `node_field_diff::for_each_changed_field`, and verified R1–R4, AC-01..AC-04 (`35.81 ms` auto-fix / `9.74 ms` whole-page `run_tx` in release on a 21,978-node synthetic kit page; `105` allocations for 10,000 touched / 10 changed nodes), and all 5 edge cases.
