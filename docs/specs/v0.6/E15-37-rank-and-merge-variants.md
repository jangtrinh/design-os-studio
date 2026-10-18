# E15-37 Rank variants by lint, checklist and pairwise scores and merge the winner

- Issue: #626 (task E15-37, epic #585, M / P0)
- Spec base: `main` at `767acf9`.
- Files: `crates/designos-mcp/src/tools/variants.rs` (rank, merge, archive, restore), `crates/designos-eval/src/judge/` (used as a library with default features), `crates/designos-doc/src/explorations.rs` (`VariantScores`), `crates/designos-mcp/Cargo.toml` (dependency on `designos-eval`, `default-features = false`).

## 1. Summary and user story

As a designer, I want the variants of an exploration ranked by a transparent score (deterministic quality, checklist and head-to-head comparisons), then to merge or keep the winner, with the others archived and restorable, never deleted.

Selecting the best of several attempts beats one attempt.

## 2. Current state

- FACT: E15-36 stores explorations and variant records in `Document.explorations`.
- FACT: E15-26 computes deterministic metrics; E15-31 provides checklist and pairwise judging and Bradley–Terry strengths; E15-32 marks judge configurations trusted or untrusted; E15-34 critique scores per axis may exist for variant frames.
- FACT: T10.3 (#345) merges branches; merged branches are archived, read-only and restorable (owner R5.19).
- FACT: `designos-mcp` depends on no eval crate today (`crates/designos-mcp/Cargo.toml`).

## 3. Behaviour rules

1. **Deterministic score `D`** ∈ [0, 1] per variant frame (E15-26 metrics on the frame):
   `D = 0.25·render_success + 0.20·token_coverage + 0.15·auto_layout_coverage + 0.20·(1 − min(1, lint_per_100.total / 10)) + 0.10·[contrast_failures = 0] + 0.10·[clipped_text = 0]`; `null` metrics count as 0.
2. **Checklist score `C`** ∈ [0, 1]: E15-31 `checklist_pass_rate` on the exploration's checklist. The checklist is the brief's checklist when given, otherwise generated once per exploration by the judge from the brief (stored, shown to the user, identical for all variants).
3. **Pairwise score `P`** ∈ [0, 1]: Bradley–Terry strengths (E15-31) normalised to max 1, from position-swapped pairwise judging (6 samples per pair). For `n ≤ 4` every pair is judged; for `n > 4` the top 4 by `(D + C) / 2` play a round robin and the others get `P = 0` with `pairwise: "not_played"`.
4. **Total** `S = 0.4·D + 0.3·C + 0.3·P` (weights in settings `variants.rank_weights`, each ≥ 0, sum 1; the aesthetic score may be added with weight ≤ 0.1, default 0, and only when the eval's `aesthetic` feature is available, which it never is inside `designos-mcp`).
5. **Trust rule.** When the judge configuration is not `trusted` in `judge-trust.toml` (E15-32), or no judge profile is configured, `C` and `P` are not computed and `S = D`; the result says `ranking: "deterministic-only"` and why.
6. **Judge ≠ generator.** A judge model equal to any variant's generator model is refused (`judge_is_generator`, E15-31 rule 5). With mixed models, the user picks a judge model that generated none of the variants.
7. **Ordering.** Sort by `S` descending; ties by `D`, then by `C`, then by variant index ascending. AC1: the ranking is recomputed from stored scores with no new judge call (`designos_rank_variants({ …, rescore: false })` default after the first scoring).
8. **Storage.** Every raw judge answer and every component score is stored in `VariantRecord.scores` with the judge model, judge config id and weights used.
9. **Merge winner.**
   - Frames placement: the winner frame is renamed `<Brief title>` (the variant suffix removed) and its in-progress and variant tags cleared; the other variant frames move into a section `Archive · <brief title> · <date>` placed below the row, locked, with `archived = true` in their records. Per-variant style modes (E15-36 rule 7) stay so archived frames still render correctly; the winner's modes become the defaults of the style collections (other modes remain).
   - Branches placement: the winner branch merges into its base with T10.3 (one undo step, versions before and after); loser branches are archived (read-only, restorable) without merging.
   - AC2: losers are archived and restorable; nothing is deleted.
10. **Keep.** `action: "keep"` keeps the winner frame where it is without renaming or archiving others (for users who only want the ranking).
11. **Restore.** `designos_restore_variant({ exploration_id, index })` unlocks the archived frame and moves it back next to the winner (frames), or un-archives the branch (branches). One undo step.

## 4. Data model / schema

```rust
pub struct VariantScores { pub d: f64, pub c: Option<f64>, pub p: Option<f64>, pub total: f64,
    pub metrics: DeterministicMetrics, pub checklist: Option<ChecklistResult>, pub pairwise_strength: Option<f64>,
    pub judge_model: Option<String>, pub judge_config_id: Option<String>, pub weights: [f64; 3], pub ranking_mode: RankingMode }
pub enum RankingMode { Full, DeterministicOnly { reason: String } }
```

## 5. Contracts

- `designos_rank_variants({ exploration_id, judge_model?, rescore? })` → `{ "exploration_id": "x-2", "ranking": "full", "ranked": [ { "index": 2, "total": 0.81, "d": 0.88, "c": 0.80, "p": 1.0 }, { "index": 1, "total": 0.66, … }, { "index": 3, … } ], "judge_model": "gpt-5.6-sol", "weights": [0.4, 0.3, 0.3] }`.
- `designos_merge_variant({ exploration_id, index, action: "merge" | "keep" })` → `{ "merged": true, "winner": 2, "archived": [1, 3], "undo_label": "Merge variant 2" }`.
- `designos_restore_variant({ exploration_id, index })` → `{ "restored": true, "frame_id": 610 }`.
- Errors: `exploration_unknown`, `variants_not_done` (some variant still running; `safe_to_retry: true`), `judge_is_generator`, `variant_index_invalid`, `merge_conflict` (branches: T10.3 reports conflicts because main moved; details list layers; the user resolves per layer through the branches UI).

## 6. File-format mapping

None beyond E15-36 storage.

## 7. UI

The ranking view is E15-38. Archive sections appear on the canvas as locked sections named as above.

## 8. Edge cases

1. A failed variant (no frame content): `D = 0`, excluded from pairwise; listed last with `state: failed`.
2. Rescore after the user edited a variant: `rescore: true` recomputes all scores; stored scores record the document revision they were computed on and a listing marks stale scores.
3. Merging when the winner frame was moved to another page by the user: allowed; the archive section goes to the winner's page.
4. Branch merge conflicts because main changed during exploration: `merge_conflict`; nothing merged; losers not archived until the merge completes.
5. Restoring a variant whose archive section was deleted by the user: the frame was deleted with it; `variant_index_invalid` with the reason "frame no longer exists" (nothing to restore, and the deletion was the user's own undoable action).

## 9. Non-goals

- Calibrating the judge (E15-32).
- Automatic merging without a user or agent request.

## 10. Test plan

Seams: `cargo test -p designos-eval judge` (scores) and `cargo test -p designos-mcp variants_rank` (MCP).

Fail-before: the tools do not exist.

| Test | Input | Expected |
|---|---|---|
| `ranking_reproducible_from_stored_scores` (AC1) | score once with a stub judge; rank again with `rescore: false` and the stub disabled | identical ranking; stub call count 0 on the second call |
| `merge_archives_losers` (AC2) | frames placement, winner 2 | variants 1 and 3 in a locked archive section, `archived = true`; `designos_restore_variant(1)` brings 1 back; one undo restores the pre-merge hash |
| `branch_merge_archives_loser_branches` | branches placement | winner merged, losers archived and restorable |
| `untrusted_judge_falls_back` | judge config untrusted | `ranking: deterministic-only`, `S = D` |
| `deterministic_score_formula` | metrics fixture (1, 0.5, 1, lint 5, 0, 0) | D = 0.25 + 0.10 + 0.15 + 0.10 + 0.10 + 0.10 = 0.80 |
| `tie_breaks_by_d_then_index` | equal S | order by D, then index |
| `mcp_links_no_inference_crate` | `cargo tree -p designos-mcp` | no ML inference crate listed |

Visual evidence: canvas capture after merge showing the winner and the archive section, plus the ranking JSON.

## 11. Performance budgets

- Deterministic scoring ≤ 300 ms per variant frame (≤ 10k nodes).
- Judge calls run on worker threads in `designos-mcp`; Studio's UI thread never blocks (CLAUDE.md §4).

## 12. Dependencies and build order

- Needs E15-36, E15-26, E15-31, E15-34 (and E15-32 for trust).
- Unblocks E15-38.

## 13. Owner rulings that apply

- R4.13: judges run through the user's agent CLI.
- Merged-away and losing variants are archived, never deleted (R5.19 and "nothing vanishes").
- Generator is never grader.
