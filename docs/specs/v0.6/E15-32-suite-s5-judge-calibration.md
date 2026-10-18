# E15-32 Add suite S5 judge calibration against UICrit and owner votes

- Issue: not filed yet (task E15-32, epic #585, M / P0)
- Spec base: `main` at `767acf9`.
- Files: `crates/designos-eval/suites/s5/{uicrit_pairs.toml,owner_pairs.toml,README.md}` (new), `crates/designos-eval/src/suites/s5.rs` (new), `crates/designos-eval/src/votes/{sheet.rs,import.rs}` (new), `crates/designos-eval/src/votes/sheet_template.html` (new), `crates/designos-eval/judge-trust.toml` (new, committed results).

## 1. Summary and user story

As the owner, I want a judge configuration (profile, model, prompt version) to be trusted only after it agrees with human judgement often enough, measured against the UICrit ratings and against my own pairwise votes, so untrusted judges never steer a gate or a ranking.

## 2. Current state

- FACT: UICrit holds critiques, bounding boxes and design quality ratings for 1,000 mobile UIs; its screenshots come from RICO; the critiques are CC BY 4.0 (README of `google-research-datasets/uicrit`; E15-23 register with a separate RICO entry).
- FACT: E15-31 provides checklist and pairwise judging with position swap and stores raw answers.
- FACT: there is no voting tool.

## 3. Behaviour rules

1. **UICrit pairs.** From the UICrit rows (licence permitting, `eval-only-cache`), build pairs of screens with the same app category whose mean human quality ratings differ by ≥ 2.0 points; draw 200 pairs with seed `20261018`; the higher-rated screen is the human winner. Pairs are frozen in `uicrit_pairs.toml` by screen id only (no images committed).
2. **Owner pairs.** 100 pairs of designs we own: outputs of S2/S4 runs and E15-36 variants (never client files), chosen to span easy and hard contrasts (50 pairs with a large deterministic-metric gap, 50 with a small one). Frozen in `owner_pairs.toml` with relative paths into the results store.
3. **Voting view (owner-manual leg).** `designos-eval vote-sheet --pairs owner_pairs.toml --out votes.html` writes one self-contained offline HTML file: images embedded as data URIs, one pair per screen, three buttons (`Left is better`, `Right is better`, `No difference`), keyboard shortcuts 1/2/3, left/right placement randomised per pair with a recorded seed, a progress count, and a `Download votes` button that saves `owner-votes.json` (pair id, choice mapped back to A/B, timestamp). No network, no server. `designos-eval vote-import owner-votes.json` validates and stores the votes in `suites/s5/owner_votes.json` (committed). This leg is flagged owner-manual: CI never fabricates votes, and a missing votes file marks owner agreement `not_measured`.
4. **Agreement metrics** for a judge configuration:
   - `uicrit_pair_accuracy` = share of UICrit pairs where the pairwise verdict (E15-31, ties count 0.5) matches the human winner;
   - `uicrit_kendall_tau_b` = Kendall τ_b between the judge's checklist-free axis mean (E15-31 axes) and the human mean rating over the screens in the pairs;
   - `owner_accuracy` = agreement with owner votes, excluding pairs the owner marked `No difference`; `owner_kappa` = Cohen's κ over the three-way labels (A, B, tie);
   - each with a 95% bootstrap CI over pairs (B = 10,000, seeded as E15-25).
5. **Trust rule.** A judge configuration is `trusted` when all hold:
   - `owner_accuracy ≥ 0.70` and its CI lower bound > 0.55;
   - `uicrit_pair_accuracy ≥ 0.65` and its CI lower bound > 0.50;
   - the mean `order_disagreement` across pairs ≤ 0.35.
   The thresholds are stored in `judge-trust.toml` under `[thresholds]` and can only change by a reviewed commit. A configuration with owner agreement `not_measured` is `untrusted`.
6. **Configuration identity.** `judge_config_id = sha1(profile command template ‖ model ‖ prompts/checklist.md ‖ prompts/pairwise.md ‖ rubric.toml)` (first 12 hex). Any prompt or rubric edit creates a new configuration that must be calibrated again.
7. **Effect of trust.** `judge-trust.toml` lists configuration ids with `trusted = true|false` and the metrics. E15-25 marks judge-derived metrics from an untrusted configuration with `judge_trusted: false`; E15-33 never gates on judge metrics; E15-37 uses judge scores in ranking only from a trusted configuration and otherwise ranks by deterministic metrics alone, saying so in its result (AC2).
8. **JSONL (AC1).** S5 runs write `{"type":"calibration","judge_config_id":…,"judge_model":…,"uicrit_pair_accuracy":…,"ci95":[…],"uicrit_kendall_tau_b":…,"owner_accuracy":…|null,"owner_kappa":…|null,"order_disagreement":…,"trusted":true|false,"thresholds":{…}}`.

## 4. Data model / schema

```toml
# judge-trust.toml
[thresholds]
owner_accuracy = 0.70
owner_accuracy_ci_low = 0.55
uicrit_pair_accuracy = 0.65
uicrit_pair_accuracy_ci_low = 0.50
max_order_disagreement = 0.35

[[config]]
id = "3f9a1c2b7d10"
judge_model = "gpt-5.6-sol"
trusted = false
measured_on = "2026-11-03"
uicrit_pair_accuracy = 0.61
owner_accuracy = "not_measured"
```

## 5. Contracts

- `designos-eval calibrate --judge <profile> [--uicrit-only]` runs S5 and updates `judge-trust.toml` (the file diff is the reviewable artefact).
- `designos-eval vote-sheet …`, `designos-eval vote-import …` as above. `vote-import` errors: `votes_pair_unknown`, `votes_duplicate`, `votes_sheet_mismatch` (seed or pair list differs from the sheet).

## 6. File-format mapping

None.

## 7. UI

The vote sheet is a local HTML file, outside Studio, so the Studio design system does not bind it; it still follows our basics: dark neutral background, one accent for focus, 16 px body text, buttons at least 44 px tall, no purple.

## 8. Edge cases

1. UICrit screens unavailable (RICO terms not approved): UICrit metrics `not_measured`; the trust rule then cannot pass, which is intended.
2. Owner votes `No difference` on more than 40% of pairs: agreement is computed on the rest and the report flags the low decision count.
3. Judge configuration changes between calibration and use: different `judge_config_id`, so it is untrusted until recalibrated.
4. Partial vote file (owner stopped halfway): imported; metrics computed on voted pairs with `n` reported; trust requires all 100 pairs voted.
5. Duplicate images in a pair: excluded at pair construction.

## 9. Non-goals

- Tuning prompts to pass the threshold on the calibration set (the held-out discipline of E15-25 applies: 30% of pairs are held out from prompt work).
- Collecting votes from people other than the owner.

## 10. Test plan

Seam: unit tests with stub judges; the owner vote session is flagged owner-manual and is not run in CI.

Fail-before: the suite does not exist.

| Test | Input | Expected |
|---|---|---|
| `agreement_written_to_jsonl` (AC1) | stub judge that always picks the higher-rated screen | calibration line with `uicrit_pair_accuracy = 1.0` |
| `untrusted_judge_marked_and_excluded` (AC2) | stub judge picking at random (seeded) | `trusted = false`; E15-25 marks its metrics `judge_trusted: false`; E15-33 gate input excludes them |
| `not_measured_owner_is_untrusted` | no owner_votes.json | `trusted = false`, reason `owner_votes_missing` |
| `vote_sheet_is_offline` | generated HTML | contains no `http://` or `https://` references |
| `vote_import_maps_sides` | sheet seed 5, vote "Left" on a pair where A was placed right | stored choice B |
| `config_id_changes_with_prompt` | edit pairwise.md | new id |
| `kappa_known_case` | 10 votes, judge agrees on 8, chance-expected agreement 0.5 | κ = (0.8 − 0.5)/(1 − 0.5) = 0.6 |

Visual evidence: terminal capture of a stub calibration and a screenshot of the vote sheet opened locally.

## 11. Performance budgets

- Vote sheet for 100 pairs ≤ 60 MB (images downscaled to a 1200 px long edge, JPEG quality 85).
- Calibration cost is 200 × 6 + 100 × 6 judge calls; the README states this before the run starts.

## 12. Dependencies and build order

- Needs E15-31, E15-23 (UICrit and RICO entries approved).
- Unblocks trusted use of judge scores in E15-37.

## 13. Owner rulings that apply

- The eval never uses client files: owner pairs come from our own runs.
- The owner vote leg is flagged owner-manual, never claimed by an agent.
- R4.13: judge calls go through the user's agent CLI.
