# E15-31 Judge designs with checklists through the agent CLI, with position swap, plus a local aesthetic score

- Issue: #620 (task E15-31, epic #585, L / P0)
- Spec base: `main` at `767acf9`.
- Files: `crates/designos-eval/src/judge/{mod.rs,checklist.rs,pairwise.rs,bradley_terry.rs,prompts.rs,parse.rs,aesthetic.rs}` (new; `aesthetic.rs` behind feature `aesthetic`), `crates/designos-eval/src/judge/prompts/{checklist.md,pairwise.md}` (new, our own text), `crates/designos-eval/src/judge/rubric.toml` (new).

## 1. Summary and user story

As the team, we want a judge we can trust on taste-adjacent questions: it answers concrete checklist items per task, compares two designs with positions swapped to cancel order bias, never judges its own model's work, and is complemented by a local aesthetic score that is one signal and never a gate.

The judge library is shared: E15-37 ranks variants with it inside `designos-mcp`.

## 2. Current state

- FACT: no judge exists. The ease-design taste rubric (MIT, owner's own) defines seven axes (Layout, Typography, Spacing, Motion, Iconography, Depth / Surface, Consistency) scored 0–10 with a pass floor of 7 per axis (`ease-design/knowledge/taste-rubric.md` §"The Critique Gate", §"Pass thresholds"), and states "the maker never grades its own work" (§"The Excellence Tier" 2).
- FACT: the agent CLIs available through the Studio terminal are Claude Code, Codex CLI, Antigravity, Grok Build (`crates/designos-app/src/terminal.rs:62-101`); E15-25 runs any configured command.
- FACT: UIClip weights are published under an MIT tag and are approved in the E15-23 register as `eval-only-cache` (owner ruling 2026-10-19): downloaded at run time into the user cache, never committed or shipped, so E15-25 accepts them as a source. The CLIP ViT-B/32 weights (register id `clip-vit-b32`, consumer E15-27) are approved on the same terms by the owner ruling of the same date.

## 3. Behaviour rules

1. **All model calls go through an agent job** (SC-5 contract, owned by E15-39): the judge writes a prompt file and image files, runs the configured judge profile's command, and reads a JSON answer from the command's output file or stdout. No HTTP client, no model library in default features.
2. **Checklist judge.**
   - Input: one design (CPU render PNG at scale 1, capped to a 2048 px long edge by downscaling, plus a compact node outline of the frame: id, kind, name, bounds, text, component name; ≤ 400 rows, deepest rows dropped first), the task brief and its checklist items, and the rubric's applicable axis descriptions (`rubric.toml`, our own condensed wording of the seven axes; Motion is excluded for static frames).
   - The prompt (`prompts/checklist.md`, our own text) asks for one answer per item: `yes`, `no` or `n/a`, with a one-sentence evidence string naming visible elements, plus optional axis scores 0–10.
   - Answer schema: `{ "items": [ { "i": 1, "answer": "yes|no|n/a", "evidence": "…" } ], "axes": { "layout": 8, … } }`. Parsing is strict: wrong item count, unknown answer values or missing evidence make the sample invalid; one retry with the parse error appended; a second failure records `judge_invalid` for that sample.
   - Samples: 3 per design (independent calls). Per item, the majority answer wins; a three-way split is `n/a` and counted in `disagreement`.
   - `checklist_pass_rate` = yes / (yes + no) over items; `null` if all items are `n/a`.
3. **Pairwise judge.**
   - Input: designs A and B for the same brief, rendered at the same scale, placed as two images labelled `First` and `Second`.
   - Order swap: 3 samples with A first, 3 with B first (6 calls). Each answer: `{ "winner": "first|second|tie", "reason": "…" }`, mapped back to A/B.
   - `p_A = (wins_A + 0.5·ties) / 6`. `order_disagreement` = share of A-first/B-first sample pairs (matched by index) whose mapped winners differ.
   - Tie rule: `|p_A − 0.5| ≤ 1/6` is reported as `tie`. AC1: identical inputs must give a tie under this rule.
4. **Bradley–Terry** (used by E15-37 for more than two designs): strengths from the matrix of `wins + 0.5·ties` by the minorisation–maximisation update `p_i ← W_i / Σ_{j≠i} (n_ij / (p_i + p_j))`, start `p_i = 1`, normalised to `Σ p = n` each step, stop at max relative change < 1e-9 or 1,000 iterations; output `p_i / max p`.
5. **Judge ≠ generator (AC2).** Every result records `generator_model` (from the run's agent profile) and `judge_model` (from the judge profile). Models are compared after normalisation (lower case, provider prefix and date suffix removed: `claude-sonnet-5-20261001` → `claude-sonnet-5`). Equal → `judge_is_generator`, the run is refused before any call. A profile without a model name cannot be used as a judge (`judge_model_unknown`).
6. **Aesthetic score.**
   - Feature `aesthetic` only. Model: UIClip-style CLIP ViT-B/32 fine-tune, weights from the register, downloaded once to the user cache; after that it runs with no network (AC3).
   - Score: image embedding `e` compared with two text embeddings of fixed prompts that state a well-designed and a poorly designed UI screenshot of the same description; `aesthetic = softmax(τ·[cos(e, t_good), cos(e, t_bad)])[0]` with τ from the checkpoint's logit scale. The exact prompts and preprocessing follow the pinned upstream inference code; a parity test against 3 upstream outputs (within 0.01) pins them.
   - Reported as `aesthetic` in [0, 1]. It is never used in a CI gate or as a sole ranking key (E15-33, E15-37 enforce this by weight caps).
7. **Determinism.** Judge outputs are not deterministic; every raw answer is stored (JSONL `judge` records) so aggregate scores can be recomputed without new calls.

## 4. Data model / schema

```rust
pub struct JudgeProfile { pub agent: AgentProfile /* E15-25 */, pub model: String }
pub struct ChecklistResult { pub items: Vec<ItemVerdict>, pub pass_rate: Option<f64>, pub axes: BTreeMap<String, f64>,
    pub samples: u32, pub invalid_samples: u32, pub disagreement: u32 }
pub struct PairwiseResult { pub p_a: f64, pub verdict: PairVerdict /* A | B | Tie */, pub order_disagreement: f64, pub samples: Vec<PairSample> }
pub fn judge_checklist(job: &dyn AgentJobRunner, input: &ChecklistInput) -> Result<ChecklistResult, JudgeError>;
pub fn judge_pair(job: &dyn AgentJobRunner, a: &DesignInput, b: &DesignInput, brief: &str) -> Result<PairwiseResult, JudgeError>;
pub fn bradley_terry(wins: &[Vec<f64>]) -> Vec<f64>;
#[cfg(feature = "aesthetic")] pub fn aesthetic(model: &UiClip, png: &RgbaImage) -> f64;
pub enum JudgeError { JudgeIsGenerator { model: String }, JudgeModelUnknown, AgentFailed(String), Invalid(String) }
```

JSONL additions: `{"type":"judge","task":…,"run":…,"kind":"checklist|pairwise","judge_model":…,"generator_model":…,"raw":[…],"result":{…}}` and `run.metrics.checklist_pass_rate`, `run.metrics.aesthetic`.

## 5. Contracts

- `designos-eval run --suite s2 --agent claude --judge codex` (judge profile name from the same config file).
- `designos-eval judge --pair a.png b.png --brief brief.txt --judge codex` and `designos-eval judge --checklist task-id --png out.png --judge codex` for ad-hoc use.
- Errors: `judge_is_generator` (exit 3), `judge_model_unknown` (exit 2), `aesthetic_weights_missing` (exit 1 with the fetch command).

## 6. File-format mapping

None.

## 7. UI

None.

## 8. Edge cases

1. Judge CLI cannot read images: the profile flag `images = false` makes the judge send the outline only and record `vision: false`; pairwise judging requires images and is refused with `judge_needs_vision`.
2. Very tall designs: downscaled to the 2048 px long edge; the scale is passed in the prompt.
3. Judge answers in prose around JSON: the parser takes the last fenced or bare JSON object in the output; anything else is invalid.
4. Timeouts: per call 120 s default; a timed-out sample counts as invalid.
5. Same model family, different size (sonnet vs opus): allowed; only normalised equality is refused. The report shows both names.
6. Weights checksum mismatch in the cache: re-download once, else `aesthetic_weights_corrupt`.

## 9. Non-goals

- Calibration against humans (E15-32).
- Critique findings with boxes (E15-34 uses its own schema; it may reuse the job runner).

## 10. Test plan

Seam: unit tests with a stub judge command (a script that reads the prompt and answers per fixture rules), plus one live run recorded in the PR.

Fail-before: the module does not exist.

| Test | Input | Expected |
|---|---|---|
| `swap_on_identical_inputs_ties` (AC1) | stub judge that always prefers `first`; A = B | `p_A = 0.5`, verdict `tie`, `order_disagreement = 1.0` |
| `consistent_judge_wins` | stub judge that always prefers the image with more red | `p_A = 1.0` when A is red |
| `judge_equal_generator_refused` (AC2) | generator `claude-sonnet-5`, judge `claude-sonnet-5-20261001` | `judge_is_generator` before any call (stub call counter 0) |
| `aesthetic_offline_after_cache` (AC3, feature) | cached weights, network disabled by pointing the fetcher at an unroutable address | score in [0, 1]; no network attempt counted |
| `aesthetic_parity` (feature) | 3 images with upstream outputs | within 0.01 |
| `checklist_majority_and_split` | samples yes/yes/no and yes/no/n/a | `yes`; `n/a` with disagreement 1 |
| `invalid_answer_retried_once` | stub returns bad JSON then good JSON | result valid, `invalid_samples = 1` |
| `bradley_terry_known` | A beats B 6/6, B beats C 6/6, A beats C 6/6 | ranking A > B > C; strengths normalised to max 1 |

Visual evidence: terminal capture of the stub tests and, for the live run, the pair of images with the stored judge answers.

## 11. Performance budgets

- Library overhead per call ≤ 50 ms (excluding the agent).
- Bradley–Terry for n ≤ 8 ≤ 1 ms.
- Aesthetic inference ≤ 1.5 s per image on CPU.
- Default-feature builds link no inference crate (checked in CI with `cargo tree -p designos-mcp`).

## 12. Dependencies and build order

- Needs E15-25, E15-23. Uses the agent job runner `designos_eval::agent_job` (SC-5, contract specified in E15-39); whichever of E15-31 and E15-39 lands first creates the module.
- Unblocks E15-32, E15-34 (critic job runner reuse), E15-37.

## 13. Owner rulings that apply

- R4.13: every judge call goes through the user's agent CLI; aesthetic weights stay in the user cache, eval only.
- The rubric reuses the ease-design taste rubric (owner's own knowledge set).
- Generator is never grader.
