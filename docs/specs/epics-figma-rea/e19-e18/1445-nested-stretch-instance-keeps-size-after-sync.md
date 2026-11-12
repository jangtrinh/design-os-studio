# #1445 A nested Stretch instance keeps its constrained size after a main sync

| Field | Value |
|---|---|
| Issue | #1445 (epic #1487, E19 Components & instances) |
| Code baseline | `origin/main@d22dad970` (2026-11-06). Re-anchor every line below against HEAD before coding; report drift first. |
| Verdict | **Mostly fixed / in flight.** Close when draft PR #1673 merges and the issue's exact App-path repro passes on `main`. No new build leg. |
| Size | 1.5 h (review of #1673 + one App-path regression test + close) |
| Tier | Claude Sonnet for the test; Opus for the #1673 review (controller) |
| Touches | `crates/designos-app/src/comp_ops.rs` (tests module only). #1673 owns `crates/designos-doc/src/components.rs`. |
| Depends on | PR #1673 (draft, closes #1656). PR #1672 also edits `components.rs` (merge order: whichever lands second rebases). |

## 1. Still real?

- FACT: PR #1655 (merged 2026-11-05) carried contributor fix #1641: a nested Stretch/Fill instance keeps its derived size when its enclosing instance syncs. Body says "Part of #1445" (gh PR #1655).
- FACT: The general case moved to #1656 ("Main component sync does not re-apply constraints"): Stretch, Scale, Max, Center on plain layers; nested instance inside a plain frame; swapped nested instance. #1656 says "Related: #1445".
- FACT: PR #1673 (`jangtrinh/agy-1656-sync-constraints`, draft, no review yet, CI green except advisory `Test (windows-latest)`) re-runs `plan_constraint_pass` + `apply_constrained_geometry` and the auto-layout solve after each synced container (`plans/reports/agy-1656-sync-constraints.md` §1) and adds 8 red-first tests, incl. Scale, Max, Center, nested-in-plain-frame, swapped nested, single undo.
- FACT: The resize path still strips `Size`/`Transform` overrides of children after an instance resize: `crates/designos-app/src/comp_ops.rs:74-79`. That is intended (a constraint relayout is not a user override); the sync must recompute constrained geometry instead, which is what #1673 does.
- INFERENCE: After #1673 merges, the issue's own repro (Card main opacity edit triggers snap 280 → 80, via `App::set_instance_prop` then `App::edit_nodes`, issue comment 2026-10-31) is covered at the doc layer but has no App-path test. One App-path test closes the loop.

## 2. User-visible outcome

Widen a Card instance so its nested Stretch button is 280 px. Edit anything on the Card or Button main (opacity, colour). The nested button stays 280 px; Scale children keep their scaled geometry; pinned children keep their distances. One undo restores the state before the sync.

## 3. Scope

1. Review #1673 against #1656's expected list (controller, Opus).
2. Add one App-path regression test reproducing the issue comment exactly.
3. After merge: rerun the test on `main`, close #1445 with evidence.

## 4. Acceptance criteria

- AC-1 (red on `d22dad970`, green after #1673): App-path test `nested_stretch_instance_keeps_width_after_unrelated_main_edit` in `comp_ops.rs` tests: Card main 100 wide, nested Btn instance 80 wide, Stretch L+R; Card instance resized to 300 (Btn 280); `App::edit_nodes` sets Card main opacity 0.5; after sync, nested Btn width == 280 ± 0.01 and x unchanged.
- AC-2: same test with the nested Btn swapped (#1427 path) gives the same 280.
- AC-3: one `undo` after the opacity edit restores opacity and keeps 280.
- AC-4: `cargo test -p designos-doc --lib` and `cargo test -p designos-app --lib` green on macOS after merge; terminal capture saved as evidence (visual evidence rule).

## 5. Test steps

1. On `origin/main`: add AC-1 test → run `cargo test -p designos-app --lib nested_stretch_instance_keeps_width` → expect FAIL (280 → 80).
2. Rebase onto #1673 head (or after merge) → PASS.
3. Studio manual check (optional, owner-visible): build, repeat the repro on canvas, screenshot before/after the main edit.

## 6. Edge cases

- Nested instance with Fixed constraints must still follow its main's size (#1655 kept this; assert it unchanged).
- Nested instance in an auto-layout parent with Fill: covered by #1655; do not regress.
- Instance resized smaller than main (negative delta): Stretch shrinks; covered by #1673 tests? If not, add one case to AC-1's test table.

## 7. Non-goals

Rewriting the sync; changing `record_overrides` (`comp_ops.rs:60-95`).

## 8. Figma-parity facts

- FACT (issue #1445 expected): in Figma a nested instance inside a resized instance keeps the size its constraints give it after any main edit. Static source not cited in the issue; behaviour is standard Figma constraint semantics. Live check if wanted: in Figma, resize an instance holding a Stretch nested instance, edit the main's opacity, read the nested width with `figma-agent inspect`.

## 9. Undecided user-visible details

None. The issue states the expected result.

## 10. Builder tier

Claude Sonnet writes AC-1..3 (App test harness is judgment-light but sits on a sensitive path). Controller (Opus) reviews #1673.

## Epistemic ledger

- FACT: #1655 merged; #1673 open draft; #1656 open (gh).
- INFERENCE: #1673 covers the issue's doc-level cases (its test list matches #1656's list).
- ASSUMPTION (medium): #1673 needs no further round. Flips if the Opus review finds a failing case.
