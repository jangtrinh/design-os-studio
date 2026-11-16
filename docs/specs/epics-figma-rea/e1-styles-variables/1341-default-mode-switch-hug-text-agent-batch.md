# #1341 Switching a collection's default mode leaves bound hug text at its old size (agent batch path)

| Field | Value |
|---|---|
| Issue | #1341 (bug, P2). Milestone: Backlog |
| Still real? | Yes on main. Contributor **draft PR #1357** (NamAIEngineer, head `bae7ef735`, base `f1baf5b7`, last updated 2026-10-30, no reviews) implements the issue's own fix |
| Size | S: about 1–2 h (rebase, review, carry) |
| Tier | Grok build-fast for the rebase and a local test run (small, reversible, with a named test). Review by Claude or Codex, never by Grok (owner rule). Carry with credit |
| Depends on | None. Files are disjoint from the other E1 items (`designos-mcp` only) |
| Ready after owner answers? | Yes |

Code baseline: `origin/main@d22dad970`.

## 1. Outcome

An agent switches a collection's default mode through `designos_apply_batch`. A label bound to that collection's size variable and its hug-sized button now resize to match what the canvas draws, the same as when a person switches the mode in Studio. Undo restores both.

## 2. Re-anchor (origin/main)

- FACT: the batch path snapshots only the written variables: `VariablesBefore::capture(&trial, &written)` (`crates/designos-mcp/src/tools/batch.rs:2181`, type at `crates/designos-mcp/src/tools/text_remeasure.rs:36-42`). Then `remeasure_after` (`text_remeasure.rs:119`, called at `batch.rs:2189`) runs. A collection whose `modes` or `default_mode` changed, with no variable record written, yields an empty changed set.
- FACT: the editor's rule already covers this. `changed_variables` treats every variable of a collection whose `modes` or `default_mode` changed as changed, plus their aliases (`crates/designos-app/src/variables/mod.rs:556-575`).
- FACT: PR #1357 adds a collection snapshot and marks number/string variables (plus aliases) as changed when a collection's modes or default mode differ. It touches `batch.rs` (+2/−1), `text_remeasure.rs` (+25/−5) and the test `crates/designos-mcp/tests/hug_text_relayout.rs` (+38).

## 3. Rules for the carry

1. Rebase #1357 on main. The base is a week old; `mergeable` reports clean, but the rebase is still needed so CI runs against current main.
2. Review point (DRY): the MCP rule must match the app rule in `changed_variables`, including aliases. Builder decision: keep the MCP copy, since the app helper takes the app's `VariableStore` type. Add a doc comment that names the twin, so the two stay in step. Moving both to `designos-doc` is a follow-up issue, not this PR.
3. Colour and boolean variables do not reflow text, so they are excluded as in the PR. Keep that.

## 4. Acceptance criteria

- **AC-1** A label is bound to a number variable whose modes are 14 and 28, inside a hug button. A batch `set_variables` switches the default mode. Then the stored label size equals the drawn size (about 87x34 from the issue's probe), and the button equals padding plus the label. This is red on main.
- **AC-2** Undo restores the 44x17 label and the 68 px button. Redo reapplies the new sizes.
- **AC-3** Changing only a collection's mode *name* (same ids, same default) triggers no remeasure. A test asserts no extra commands.
- **AC-4** An alias chain (label → alias → size variable): the switch remeasures the label.
- **AC-5** Mutation check: revert the snapshot, and AC-1 fails.

## 5. Test steps

`gh pr checkout 1357`, rebase on main, `cargo test -p designos-mcp --test hug_text_relayout`, revert check, then `cargo test -p designos-mcp`, clippy and fmt. Evidence: a terminal capture.

## 6. Edge cases

- The batch both writes a variable and switches the mode: the union of both sets is remeasured once.
- A collection is deleted in the same batch: its bindings are baked and nothing reflows (the app rule).
- The batch runs with `dry_run=true`: no remeasure commands are published (existing contract).

## 7. Non-goals

- The app-side mode switch (already correct).
- Moving `changed_variables` into `designos-doc`.

## 8. Builder rules

Carry with credit. The commit author stays NamAIEngineer, the same as #1616. Use `Closes #1341`.
