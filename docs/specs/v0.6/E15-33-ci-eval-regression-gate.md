# E15-33 Gate CI on deterministic eval metrics

- Issue: #622 (task E15-33, epic #585, M / P0)
- Spec base: `main` at `767acf9`.
- Files: `crates/designos-eval/src/gate.rs` (new), `crates/designos-eval/baselines/<release>/` (new: transcripts and baseline JSONL), `crates/designos-eval/gate.toml` (new), `.github/workflows/ci.yml` (new `eval-gate` job), `crates/designos-eval/tests/gate.rs` (new).

## 1. Summary and user story

As the team, we want every pull request to fail when the engine makes agent outputs measurably worse on the deterministic metrics, without needing an agent CLI in CI: CI replays recorded tool-call transcripts against the current engine and compares the metrics with the release baseline.

## 2. Current state

- FACT: CI jobs are `test` (macOS, Ubuntu, Windows matrix), `lint` (clippy) and `fmt` (`.github/workflows/ci.yml:17,54,78`). Only `Test (macos-14)`, `Clippy` and `Rustfmt` block merges (CLAUDE.md "Git Workflow").
- FACT: E15-25 records transcripts and replays them (`designos-eval replay`), E15-26 computes deterministic metrics, E15-29 adds drift, E15-30 consistency.

## 3. Behaviour rules

1. **Baseline set.** `baselines/<release>/` holds, per suite S2, S3 and S4: the transcripts of one recorded live run per task (dev and held-out), the start documents' generator parameters, and `baseline.jsonl` with the deterministic metrics produced by replaying those transcripts on the release commit. S1 is included when its CLIP-free scores are available (fidelity without CLIP is deterministic). S5 has no transcripts.
2. **Gate run.** `designos-eval gate --baseline baselines/v0.6.0 --out gate/`:
   - replays every baseline transcript on the current build (E15-25 replay, CPU renders);
   - computes the gated metrics (rule 3) per task;
   - compares per task with the baseline (paired);
   - writes `gate/report.jsonl` and `gate/summary.md`, exit 0 (pass) or 1 (regression) or 2 (replay diverged on more than 5% of tasks).
3. **Gated metrics and direction** (`gate.toml`):

| Metric | Better | Practical margin ε |
|---|---|---|
| `render_success` | higher | 0.01 |
| `token_coverage` | higher | 0.02 |
| `instance_ratio` | higher | 0.02 |
| `lint_per_100.total` | lower | 0.5 |
| `contrast_failures` | lower | 0.5 |
| `clipped_text` | lower | 0.5 |
| `off_grid_pct` | lower | 1.0 |
| `auto_layout_coverage` | higher | 0.02 |
| `drift.free` (S3) | higher | 0.0 (zero tolerance: any per-task drop fails, rule 4) |
| `consistency.overall` (S4) | higher | 0.02 |
| `fidelity.block_match`, `.text`, `.position`, `.color` (S1) | higher | 0.01 |

   Judge-derived and aesthetic metrics are never gated (E15-31, E15-32).
4. **Significance test** (paired bootstrap). For metric `m`, over the common task set `T` (tasks whose replay did not diverge in either baseline or current), let `d_t = sign · (m_cur(t) − m_base(t))` with `sign = +1` when higher is better and `−1` when lower is better (so negative `d` is a regression).
   - `D̄ = mean_t d_t`; bootstrap the mean of `d` over tasks with B = 10,000 (seed `sha1("gate" ‖ metric)` per SC-7); `U` = 97.5th percentile (one-sided 97.5% upper bound at α = 0.025).
   - **Regression** when `U < 0` (the whole interval is below zero) **and** `D̄ < −ε`.
   - **Zero tolerance.** For a metric with `epsilon = 0` the bootstrap is skipped: it is a regression when any common task has `d_t < 0`. With ε = 0 the bootstrap cannot do this job: one failing task among 96 unchanged ones gives `D̄ = −1/96`, but about 37% of resamples omit that task, so `U = 0` and the test would pass.
   - Because replays are deterministic, a metric whose `d_t` are all zero passes exactly; for ε > 0 a single-task change below ε passes.
5. **Divergence.** A replay that stops with `replay_diverged` (E15-25 §3.11) is excluded from the comparison and listed. More than 5% diverged tasks fails the gate with exit 2, since the engine changed the tool contract in a way the baseline cannot measure; the fix is a new baseline (rule 7) or a contract fix.
6. **CI job.** `eval-gate` on `macos-14`, after `test`:
   - `cargo build --release -p designos-eval -p designos-mcp`;
   - generate the needed fixtures with `designos-eval gen` (cached by key `gen-<kit>-<nodes>-<seed>-<generator version>`; 1M fixtures are excluded from PR runs and replayed nightly);
   - `designos-eval gate`; upload `gate/` as an artefact; post `summary.md` to the job summary.
   - The job is required for merge once three consecutive green runs on `main` are recorded (AC2); until then it reports but does not block. Making it required is an owner action on the ruleset (owner-manual, recorded in the PR). Owner ruling 2026-10-19: yes, required after three consecutive green runs.
7. **Baseline updates.** Live-agent runs happen locally (`designos-eval run … --record-baseline`). A new baseline lands only through a reviewed commit that adds `baselines/<release>/` and states the reason; CI never writes baselines.
8. **Seeded regression check (AC1).** A test patches the engine through a test-only switch (`DESIGNOS_TEST_FORCE_CLIP=1`, compiled only with `cfg(test)` or the `gate-selftest` feature) that makes `apply_spec` give every text node a fixed height equal to half its measured text height, so text clips; the gate on the S2 baseline subset must exit 1 and name `clipped_text`.

## 4. Data model / schema

```toml
# gate.toml
alpha = 0.025
bootstrap = 10000
max_diverged_share = 0.05
[metric.clipped_text]
better = "lower"
epsilon = 0.5
```

Report line: `{"type":"gate","metric":"clipped_text","n_tasks":96,"mean_delta":-1.42,"upper":-0.88,"epsilon":0.5,"result":"regression","worst_tasks":[{"task":"s2-0012-shadcn","base":0,"cur":3}]}`.

## 5. Contracts

`designos-eval gate --baseline <dir> [--suites s2,s3,s4] [--out dir]`; exit codes 0/1/2 as above; `summary.md` lists per metric: base mean, current mean, mean delta, upper bound, verdict, and the five worst tasks with links to their artefacts.

## 6. File-format mapping

None.

## 7. UI

None (CI job summary only).

## 8. Edge cases

1. A task added after the baseline: not in `T`; reported as `new`, not gated.
2. A metric `null` on one side: the task is excluded for that metric and counted.
3. Ubuntu/Windows runners: the gate runs on macOS only (merge-blocking platform); other platforms do not run it.
4. Fonts: the CI image installs the fonts used by fixtures from the `designos-fonts` cache artefact, so renders match the baseline machine; a font fetch failure fails the job with `fonts_unavailable`, not a silent metric change.
5. Flaky timing: no timing metric is gated.

## 9. Non-goals

- Running live agents in CI.
- Gating on judge or aesthetic scores.

## 10. Test plan

Seams: `cargo test -p designos-eval gate` and the CI job itself.

Fail-before: no gate exists.

| Test | Input | Expected |
|---|---|---|
| `seeded_clipping_fails_gate` (AC1) | S2 subset baseline; forced-clip switch on | exit 1; `clipped_text` regression; worst tasks listed |
| `unchanged_engine_passes` (AC2) | same commit as baseline | exit 0; every `mean_delta = 0` |
| `zero_tolerance_single_drop_fails` | one task's `drift.free` falls from 1 to 0 among 95 unchanged | exit 1; `drift.free` regression naming that task |
| `small_change_below_margin_passes` | one task's `lint_per_100.total` +0.3 | pass |
| `diverged_share_fails` | 6% of transcripts made unreplayable | exit 2 |
| `bootstrap_reproducible` | same inputs twice | identical report bytes |
| CI: three consecutive green `eval-gate` runs on `main` (AC2) | n/a | run links recorded in the PR enabling the required check |

Visual evidence: CI job summary screenshot for a pass and for the seeded failure.

## 11. Performance budgets

- PR gate (S2, S3 at 100k, S4; S1 without CLIP) ≤ 15 min on `macos-14`, fixtures cached.
- Nightly gate with 1M fixtures ≤ 60 min.

## 12. Dependencies and build order

- Needs E15-26, E15-29 (and E15-25, E15-30, E15-27 for their metrics).
- Last task of the eval track.

## 13. Owner rulings that apply

- R4.13: CI runs no model; it replays transcripts.
- The eval never uses client files.
- Full scope: every deterministic metric is gated.
