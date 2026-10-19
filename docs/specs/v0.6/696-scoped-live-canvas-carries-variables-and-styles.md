# #696 Scoped live canvas: carry variables and styles so var: and text_style resolve on large files

| Field | Value |
|---|---|
| Issue | #696 (owner ruling 2026-10-19: include variables and styles in the scoped canvas); found while fixing #670 (E15-3) |
| Size | M |
| Depends on | soft: #588 (E15-3, `var:` and `text_style` references in `apply_spec`) and #670 for the end-to-end test; none for the data copy and the merge-back ops |
| Unblocks | grounding (E15-1, E15-2, E15-3) on files over the scope threshold |
| Touches | `crates/designos-app/src/live_canvas_scope.rs` (`scoped_canvas` `:17-52`), `crates/designos-doc/src/live_patch/mod.rs` (`LiveOp` `:134-172`), `crates/designos-doc/src/live_patch/record.rs` (`:12-50`, the refusal), `crates/designos-doc/src/live_patch/replay.rs` (a replay arm and report counters), new `crates/designos-doc/src/live_patch/replay_tokens.rs`, tests in `live_sync_tests.rs` and the `live_patch` test files |

Code baseline: `origin/main@9f3f7a5`.

## 1. Summary and user story

As a designer with a large file, I want the agent to see my variables and styles and to be able to bind to them, and I want anything it changes there to land in my file once, with the same ids.

## 2. Current state (FACT)

- Above 25,000 nodes the editor shares a **scoped** canvas with the agent instead of the document (`terminal.rs:840-845`). `scoped_canvas` builds a fresh `Document`, copies the current page, the selected top-level frames in full (up to 5,000 layers) and the other top-level frames as empty shells (up to 128), plus the comment threads (`live_canvas_scope.rs:11-52`). It copies no `variables`, `variable_collections`, `styles` or `style_meta`; those are `BTreeMap`s on `Document` (`designos-doc/src/lib.rs:150-159`).
- Nodes carry their links by id: shared-style links in `style_refs: BTreeMap<String, String>` (`node.rs:110`) and variable bindings in a `bindings` map (`node.rs:286`; UNKNOWN which struct owns it. Check: read `node.rs:270-290`).
- `var:` and `text_style` references are **not on `main`**: `designos-doc/src/spec.rs` contains neither (searched on this commit); #588 is open. The status of #670 (named in the issue) is not visible from `main`; its report `plans/reports/release-060-evidence/fix-670-report.md` is not in the repository. UNKNOWN: what "the replay guard from #670" does. Check: read #670's merged code before starting.
- The agent's edits reach the editor as `LiveOp`s: `Insert`, `Update`, `Reparent`, `Remove`, `Comments` (`mod.rs:134-172`). There is no op for variables or styles. `live_ops_for_command` **refuses** style, variable, page, flow, library, guide and device commands with "<name> cannot be mirrored to the open editor" (`record.rs:12-14`, `:50`, names at `:93-106`). So today a variable or style edit on a live session is rejected, and the merge-back the issue asks for has no path.
- `Command::SetStyles { styles, style_meta }` and `Command::SetVariables { .. }` replace whole maps (`history.rs:40`, `:63`, apply at `:149-151`).
- Node ids of a scoped canvas are the editor's own; `reserve_ids` keeps the agent's new ids above them (`live_canvas_scope.rs:19`).
- Variable and style ids are strings (`Variable.id: String`, `variables.rs:95`), independent of node ids.
- The threshold (25,000), the 128 top-level frames and the 5,000-layer budget are the constants this issue says not to change (`live_canvas_scope.rs:12-15`, `terminal.rs:840`).
- `live_sync_tests.rs` has a fixture over the threshold that takes the scoped path (`big_fixture`, `:25`, assertion at `:74`).

## 3. Behaviour rules

1. **Copy the token data.** `scoped_canvas` copies every variable collection, variable, style and style meta of the full document into the scoped one: `variable_collections`, `variables`, `styles`, `style_meta`, whole, unchanged ids. Data only: no node is added, no node is copied differently, the scope threshold and node budgets do not change.
2. **Resolution.** Bindings and shared-style links on copied nodes already name ids; with the maps present they resolve on the agent's side. `var:<name>` and `text_style` references (#588) resolve against the copied maps and write the same ids a full-document call would.
3. **Merge back, binds.** An `Update` that sets a binding or style link carries ids; replay on the full document accepts it when the variable or style exists there (it does, by rule 1) and skips it with a reason ("variable not found") when it does not, counting it in `skipped`.
4. **Merge back, token edits.** Two new ops carry edits to the token maps as deltas by id:
   - `LiveOp::Variables { collections_set, collections_removed, variables_set, variables_removed }`;
   - `LiveOp::Styles { styles_set, styles_removed, meta_set, meta_removed }`.
   `live_ops_for_command` builds them from `SetVariables` and `SetStyles` by diffing the before and after maps (as the comment delta does); the refusal for these two commands is removed. Page, flow, library, guide and device commands stay refused.
5. **No duplicates.** Replay writes by id, last write wins; it never appends. An op whose value equals the full document's is counted in `duplicates`. A repeated delivery of the same patch changes nothing. Two entries with the same name but different ids are two entries (names are not keys).
6. **Ids unchanged.** Replay never remaps a variable or style id. (Node ids may be remapped by the existing machinery; token ids are not.)
7. **Undo.** A replayed token op commits as one undo step with the rest of its patch (one tool call, one step), through `SetVariables` and `SetStyles`.
8. **Cost.** Copying the maps is O(variables + styles). It runs where the snapshot is built (on the UI thread today, off it after #690).
9. Documents under the threshold are unaffected: they already share the whole document.
10. An older Studio reading a newer log reports the new op line as unreadable and goes on (the `LivePatch` line format keeps `v: 1`; Studio and `designos-mcp` ship together, CLAUDE.md section 3).

## 4. Data model / schema changes

- `LiveOp::Variables` and `LiveOp::Styles` (serde variants with `skip_serializing_if` on empty fields). `LivePatchReport` gains counters for token ops applied and skipped (`tokens_applied`, `tokens_duplicates`).
- No change to `Document` or the file formats.

## 5. Contracts

- Log line contract: two new op shapes, additive. The MCP tool schemas do not change (the agent still calls the same tools; only the live mirroring changes). The tool snapshot test (T7.6, part 1) is not regenerated by this task.
- `designos-mcp` mirrors `SetVariables` and `SetStyles` to the log instead of refusing them while a live editor is attached.

## 6. File-format mapping

None. The scoped canvas is a session-folder file (`canvas.designos`) written with the normal serializer; the added maps use the existing serde shape (`#[serde(default)]` keys).

## 7. UI

None. Evidence: a terminal capture of the round trip test output on the synthetic large file, and one 1440 screenshot of the editor after the agent bound a fill to the user's variable (the Properties panel shows the variable chip), taken from a stub agent run. Responsive: SKIPPED (owner default).

## 8. Edge cases

1. The user deletes a variable in the editor while the agent binds to it: replay skips the bind with "variable not found" and reports it in the toast; nothing dangles.
2. The agent creates a variable named like an existing one: two variables with distinct ids; the user sees both and can merge them.
3. The agent edits a variable's value for one mode: the delta carries the whole variable; replay overwrites it by id; other modes are the agent's copy of them, which were the user's at snapshot time (INFERENCE: a user edit to another mode between snapshot and replay would be overwritten; the test below checks it and the PR states the result).
4. A file with 5,000 variables and 800 styles: copied whole; the map clone is the only added cost.
5. The editor changes variables while the agent holds an old scoped canvas: the next published canvas carries the new maps; an agent write based on the old snapshot is a last-write-wins by id (same rule as nodes).
6. Library variables (from an enabled library) are not in `variables`; they are not copied and not changed here.
7. Replaying the same patch twice: second time all counted as duplicates.
8. A styles op on a style that a node still links to, deleted by the agent: the node's link dangles in the same way a deletion in the editor would; the existing detach logic applies.

## 9. Non-goals

- Changing the scope threshold, `MAX_TOP_LEVEL`, `FULL_SUBTREE_BUDGET` or how nodes are copied. Library variables. Page, flow, guide, device and library-state mirroring. The `var:` implementation itself (#588).

## 10. Test plan (red first)

Seams: `live_sync_tests.rs` (`big_fixture` over the threshold), `live_patch` unit tests, the MCP session tests in `designos-mcp/src/server.rs`.

| Case | Expected | Fails before |
|---|---|---|
| `scoped_canvas_carries_all_collections_variables_styles_and_meta` | counts and ids equal the full document's; node count equals the previous scoped result | yes |
| `a_bind_on_the_scoped_canvas_merges_with_unchanged_ids` | after replay, the full document's node holds the user's variable id | yes (needs #588 for the `var:` spelling; before it, bind through the binding field with the same ids) |
| `bind_then_merge_then_export_spec_round_trips` | `export_spec` of the merged node shows the same reference | yes |
| `a_variable_edit_on_the_scoped_canvas_merges_without_duplicates` | one variable, new value | yes (refused today) |
| `a_style_edit_on_the_scoped_canvas_merges_without_duplicates` | one style, new value | yes |
| `replaying_a_token_patch_twice_changes_nothing` | `duplicates` counted, document equal | yes |
| `a_bind_to_a_deleted_variable_is_skipped_with_a_reason` | `skipped` has the reason; no panic | yes |
| `token_ops_are_one_undo_step_with_their_patch` | history depth +1 | yes |
| `page_flow_library_commands_are_still_refused` | error text unchanged | guard |
| `scope_threshold_and_node_budgets_are_unchanged` | constants and node counts equal | guard |
| `old_reader_reports_the_new_op_as_unreadable` | line returned as an error | yes |

Run: `cargo test -p designos-app live_sync`, `cargo test -p designos-doc live_patch`, `cargo test -p designos-mcp`. Evidence: red/green capture and the round trip output.

## 11. Performance budgets

- Map copy: under 1 ms for 5,000 variables and 800 styles (measured with `test_timing::fastest_within`); this adds to the snapshot cost tracked by #690.
- Replay of a token op: O(entries in the delta). No document walk, no `to_json` on the UI thread.
- Idle: unchanged.

## 12. Dependencies and build order

The data copy and the ops need no other task. The end-to-end test needs #588 and #670. `live_patch/mod.rs` and `replay.rs` are shared with part 3's #677 (`TurnMark`), part 1's E15-4, E15-7, E15-14 and RD-3, and part 4's #682 (`touched`): serialize these edits and rebase (README-part4). `live_canvas_scope.rs` is shared with #690 only by call site.

## 13. Owner rulings

- Include variables and styles in the scoped canvas (2026-10-19). Data only, no extra nodes; ids unchanged; edits merge back without duplicates; a test on a synthetic large file covers bind, merge, `export_spec`; no change to the scope threshold or node copying. (Issue text.)

## Open questions for the owner (recommended first)

1. The issue speaks of "the replay guard from #670". Recommended: confirm it is the by-id, last-write-wins rule of rule 5; if #670 did something else, this task adopts #670's guard and drops its own.
2. Concurrent edits to other modes of one variable (edge case 3). Recommended: accept last-write-wins at the variable level, as nodes do.
