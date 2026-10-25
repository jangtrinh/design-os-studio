# E15-27 Add suite S1 recreate with fidelity metrics

- Issue: #616 (task E15-27, epic #585, L / P0)
- Spec base: `main` at `767acf9`.
- Files: `crates/designos-eval/suites/s1/{tasks.toml,split.lock,README.md}` (new), `crates/designos-eval/src/metrics/fidelity.rs` (new), `crates/designos-eval/src/metrics/{blocks.rs,assign.rs,ciede2000.rs,inpaint.rs}` (new), `crates/designos-eval/src/judge/clip.rs` (new, feature `aesthetic`), `crates/designos-eval/tools/s1_reference_blocks.py` (new, offline helper), `crates/designos-eval/tests/fixtures/s1_golden.json` (new).

## 1. Summary and user story

As the team, we want to measure how faithfully an agent recreates a reference web page on the canvas, with the published Design2Code-style scores (block match, text, position, colour, CLIP), so fidelity changes are visible release to release.

## 2. Current state

- FACT: no fidelity metric exists. `designos-diff` compares two images pixel-wise and reports `mismatched`, `ratio` and `ssim` (`crates/designos-diff/src/lib.rs:40-60,301-308`); it has no notion of text blocks.
- FACT: the Design2Code dataset (`SALT-NLP/Design2Code`, revision `e2f61f6…`) is ODC-BY and its metric code (`NoviScl/Design2Code`) is MIT (E15-23 register; Design2Code approved as `eval-only-cache` by owner ruling 2026-10-19, so it is fetched at run time and never committed or shipped).
- FACT: text nodes in a document have exact world bounds, characters and resolved fills, so candidate blocks need no OCR.

## 3. Behaviour rules

1. **Tasks.** 40 reference pages drawn from the Design2Code test split with seed `20261018`: shuffle the sorted page ids with xoshiro256** (SC-7) and take the first 40. The list is frozen in `tasks.toml`; the draw is re-checkable with `designos-eval lock --suite s1`.
2. **Task brief.** The agent gets the reference screenshot (PNG in the cache, `inputs = ["ref/<id>.png"]`) and the instruction to recreate it as one frame named `Recreation` whose width equals the screenshot width, height free. The reference HTML is never shown to the agent.
3. **Reference blocks** are computed once per page offline by `tools/s1_reference_blocks.py`, which runs the pinned Design2Code reference code to extract text blocks (text, bounding box, text colour) from the reference HTML. Output `ref/<id>.blocks.json` lives in the user cache next to the screenshot (class `eval-only-cache`, approved). It is never committed: no `ref/<id>.blocks.json` file is ever added to the repository.
4. **Candidate blocks** come from the output frame: every visible text node becomes one block per laid-out line group (consecutive lines of one text node merge into one block, as in the reference method), with world bounds relative to the frame, characters, and the resolved fill colour of its first run (alpha composited over the backdrop colour sampled from the CPU render under the block centre).
5. **Normalisation.** Both sides' coordinates are divided by their page width and height (reference: screenshot size; candidate: rendered frame size), so positions are in [0, 1].
6. **Text similarity** between two strings: `sim(a, b) = 2·M / (|a| + |b|)` where `M` is the number of matched characters from Ratcliff/Obershelp longest-common-substring matching (the same definition as Python `difflib.SequenceMatcher.ratio()`); strings are lower-cased and whitespace-collapsed first.
7. **Block matching.** Build the cost matrix `C[i][j] = −sim(r_i, c_j)` over reference blocks `r` and candidate blocks `c`; solve the rectangular linear assignment with the Jonker–Volgenant (shortest augmenting path) algorithm; drop pairs with `sim < 0.5`. Before matching, the reference method's merge step is applied on both sides: adjacent blocks whose union improves the best pairing are merged, following the pinned reference code exactly (the golden test, rule 13, pins this behaviour).
8. **Block-Match score** = `Σ_{(i,j)∈P} (S(r_i) + S(c_j)) / (Σ_i S(r_i) + Σ_j S(c_j))` where `P` is the matched set and `S(·)` is the block area in normalised units. Missing and invented blocks lower the score.
9. **Text score** = mean over `P` of `sim(r_i, c_j)`; 0 when `P` is empty.
10. **Position score** = mean over `P` of `1 − max(|x_r − x_c|, |y_r − y_c|)` using normalised block centres; 0 when `P` is empty.
11. **Colour score** = mean over `P` of `1 − ΔE₀₀(rgb_r, rgb_c) / 100` with CIEDE2000 on sRGB-converted colours (D65); 0 when `P` is empty.
12. **CLIP score** = cosine similarity of CLIP ViT-B/32 image embeddings of the reference screenshot and the candidate render, both with text regions inpainted (each side's own blocks) by the Telea fast-marching method (radius 3 px), resized with bicubic sampling to 224×224 after a centre crop of the shorter side, normalised with the CLIP mean and standard deviation. Inference runs in `designos-eval` with the `aesthetic` feature, from the CLIP ViT-B/32 weights (register id `clip-vit-b32`, approved by the owner as `eval-only-cache` on 2026-10-19, same terms as UIClip) downloaded to the user cache by `designos-eval fetch`; never in the app.
13. **Overall** = mean of the five scores. Each score is also written separately. With the `aesthetic` feature and the weights fetched, all five exist and `overall` is the mean of five. Without the feature (or without the weights) there is no CLIP score and `overall` is `null`, never the mean of four, so a number is never compared with one built from a different set.
14. **Ceiling (AC1).** Scoring the reference against itself (reference blocks vs themselves, screenshot vs itself) must give 1.0 for the four deterministic scores always, and for the CLIP score and `overall` when built with `--features aesthetic` with the approved weights fetched (±1e-6 for all five). The harness also records an importer ceiling when the web-capture importer (#277) is available: the reference HTML imported through it and scored as a candidate; this value is stored in `suites/s1/README.md` and reported next to S1 results as context, not used to rescale.
15. **Reference parity (AC2).** For 3 golden samples (fixed reference/candidate pairs whose blocks and images are committed as small synthetic fixtures we made, not dataset pages), our metric code reproduces the pinned Python reference output: block-match, text, position and colour within 1e-4; CLIP within 0.01 (different inference runtime; asserted only with `--features aesthetic` and the weights fetched).

## 4. Data model / schema

```rust
pub struct Block { pub text: String, pub bbox: [f64; 4] /* x, y, w, h normalised */, pub color: [u8; 3] }
pub struct FidelityScores { pub block_match: f64, pub text: f64, pub position: f64, pub color: f64,
    pub clip: Option<f64> /* None without the aesthetic feature */, pub overall: Option<f64>,
    pub matched: u32, pub ref_blocks: u32, pub cand_blocks: u32 }
pub fn score(reference: &[Block], candidate: &[Block], ref_png: &RgbaImage, cand_png: &RgbaImage, clip: Option<&ClipModel>) -> FidelityScores;
```

JSONL `run.metrics` keys: `fidelity.block_match`, `fidelity.text`, `fidelity.position`, `fidelity.color`, `fidelity.clip`, `fidelity.overall`. Without the `aesthetic` feature, `clip` and `overall` are `null` and `notes` says why.

## 5. Contracts

- `designos-eval run --suite s1 …` (E15-25) computes these scores per run.
- `designos-eval fetch --suite s1` downloads the pinned dataset revision and the CLIP weights (`clip-vit-b32`, approved in the register) into the cache after checking the register (`source_not_approved` otherwise).
- `designos-eval fidelity --ref-blocks a.json --ref-png a.png --doc out.designos --frame Recreation` prints `FidelityScores` as JSON.

## 6. File-format mapping

None.

## 7. UI

None.

## 8. Edge cases

1. Candidate with no text: block-match 0, text/position/colour 0, CLIP still computed.
2. Candidate frame taller than the reference: normalisation per side keeps positions comparable; extra content lowers block-match through unmatched area.
3. Text drawn as vectors (outlined): not a text block; counts as missing text. Documented in the suite README.
4. Reference page whose screenshot exceeds 4096 px tall: rendered and compared at the cached size; the candidate render uses scale 1.
5. Weights missing and no network: CLIP `null`, run continues, `notes` says `clip_weights_missing`.
6. Duplicate identical blocks (repeated nav items): assignment handles them; ties broken by lowest index for determinism.

## 9. Non-goals

- Pixel diff as a fidelity score (`designos-diff` covers pixel regressions elsewhere).
- Showing reference HTML to the agent.

## 10. Test plan

Seam: `cargo test -p designos-eval metrics::fidelity` (CLIP tests behind `--features aesthetic`) plus one live run recorded in the PR.

Fail-before: the module does not exist.

| Test | Input | Expected |
|---|---|---|
| `self_score_is_ceiling` (AC1) | synthetic reference vs itself | the four deterministic scores 1.0 ± 1e-6 (always); with `--features aesthetic` and the weights fetched, CLIP and `overall` 1.0 ± 1e-6 as well |
| `overall_is_null_without_clip` | build without `aesthetic` | `clip` and `overall` are `None`, `notes` says why; never a mean of four |
| `golden_matches_reference_impl` (AC2) | 3 golden samples | four scores within 1e-4 always; with `--features aesthetic` and the weights fetched, CLIP within 0.01 of `s1_golden.json` |
| `missing_block_lowers_block_match` | 4 equal-area reference blocks, candidate has 3 of them | block-match (3+3)/(4+3) = 0.857 |
| `colour_score_uses_ciede2000` | `#000000` vs `#FFFFFF` text | ΔE₀₀ = 100 → colour score 0.0 |
| `position_score_formula` | centres (0.2, 0.2) vs (0.25, 0.3) | 1 − max(0.05, 0.1) = 0.9 |
| `assignment_is_deterministic` | duplicated blocks | same pairing on repeat |
| `task_draw_is_frozen` | seed 20261018 | the 40 ids equal `tasks.toml` |

Visual evidence: terminal capture of the golden test plus, for the live run, the reference and candidate PNGs side by side with the five scores.

## 11. Performance budgets

- Fidelity without CLIP ≤ 100 ms per page (≤ 500 blocks per side, O(n³) assignment).
- CLIP inference ≤ 1.5 s per image pair on CPU; weights loaded once per process.
- No app or MCP code path links the CLIP code (`cargo tree -p designos-app` and `-p designos-mcp` show no inference crate).

## 12. Dependencies and build order

- Needs E15-25, E15-23 (Design2Code and the CLIP ViT-B/32 weights are both approved as `eval-only-cache` by owner rulings of 2026-10-19, so `fetch` can download both and the CLIP score is computed). Optional: #277 for the importer ceiling.
- Parallel with E15-28 and E15-31.

## 13. Owner rulings that apply

- R4.13: CLIP weights live in the user cache and are used only by the eval, never bundled in the app.
- The eval never uses client files.
- Full scope: all five fidelity scores.
