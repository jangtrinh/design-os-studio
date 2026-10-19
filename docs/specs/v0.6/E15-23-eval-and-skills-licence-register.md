# E15-23 Review dataset, model-weight and skill licences for the eval and skills pack

- Issue: #611 (task E15-23, epic #585, S / P0)
- Spec base: `main` at `767acf9`.
- Files: `docs/eval-licences.md` (new register), `THIRD_PARTY_LICENSES.md` (new), `scripts/check_eval_licences.py` (new), `.github/workflows/ci.yml` (one step).

## 1. Summary and user story

As the owner, I want every dataset, model weight, design kit and skill used by `designos-eval` and the skills pack recorded with its licence, allowed use, attribution text and pinned version, so nothing we ship or run is legally doubtful and no client file is ever used.

## 2. Current state

- FACT: no register exists; there is no `THIRD_PARTY_LICENSES` file at the repo root (`ls` on `767acf9`).
- FACT: CI runs `test`, `lint` (clippy) and `fmt` jobs (`.github/workflows/ci.yml:17,54,78`).
- FACT (checked 2026-10-18 through the GitHub and Hugging Face APIs):

| Source | Pinned version | Licence as published | Notes found |
|---|---|---|---|
| Design2Code dataset (`SALT-NLP/Design2Code`, Hugging Face) | revision `e2f61f6e9d9b5ee201b81d8e0ae4d37aa13f8f25` | `odc-by` (dataset tag) | pages derive from real websites; page content belongs to the original sites |
| Design2Code metric code (`NoviScl/Design2Code`, GitHub) | pin a commit at implementation | MIT | reference implementation for E15-27 |
| WebSight (`HuggingFaceM4/WebSight`) | revision `b11f8172f89c992b56ac702319e02c428cca4a4e` | `cc-by-4.0` | synthetic pages; check the source of embedded photos |
| UICrit (`google-research-datasets/uicrit`) | pin a commit at implementation | CC BY 4.0 (README) | screenshots come from the RICO dataset, which has its own terms |
| UIClip weights (`biglab/uiclip_jitteredwebsites-2-224-paraphrased` and siblings) | pin a revision | `mit` (model tag) | check training-data terms |
| CLIP ViT-B/32 weights (Hugging Face `openai/clip-vit-base-patch32`; code `openai/CLIP`) | revision `3d74acf9a28c67741b2f4f2ea7635f0aaf6f0268` (field `sha` of `https://huggingface.co/api/models/openai/clip-vit-base-patch32`, read 2026-10-19; last modified 2024-02-29) | MIT (GitHub licence of `openai/CLIP`, read through the API 2026-10-19); the Hugging Face model carries no licence tag | owner-approved `eval-only-cache` 2026-10-19, same terms as UIClip; the implementer re-reads the revision before the first fetch and records any change |
| shadcn/ui (`shadcn-ui/ui`) | pin a commit | MIT | open kit for S2 and the generator (owner ruling 2026-10-19: S2 uses shadcn/ui + IBM Carbon) |
| IBM Carbon (`carbon-design-system/carbon`) | pin a commit | Apache-2.0 | open kit for S2 |
| `jakubkrehel/skills` | commit `4c66b1fa86e9ab70187b43d7da9b6fdb29f2ef63` (pushed 2026-10-13) | MIT | adopted by E15-46 |
| transitions.dev skills | none | no licence file found | excluded |
| ease-design knowledge (`/Users/jang/Products/ease-design`) | pin a commit | MIT ("Copyright (c) 2026 ease-design contributors") | owner's own; source for E15-18, E15-21, E15-31, E15-47 |

## 3. Behaviour rules

1. **One register.** `docs/eval-licences.md` has a human table and one fenced `toml` block that is the machine source. The check script parses only the TOML block; a test asserts the table rows equal the TOML entries (ids, licences, use).
2. **Entry fields:** `id`, `name`, `kind` (`dataset | weights | kit | skills | knowledge | font`), `url`, `pinned` (commit SHA or Hugging Face revision), `licence` (SPDX id, or `NONE`), `use` (`eval-only-cache | eval-redistribute | ship-in-app | ship-in-skills | excluded`), `attribution` (exact text), `review` (`approved | pending | rejected`), `reviewer`, `reviewed_on`, `notes`, `consumers` (task ids).
3. **Use classes.**
   - `eval-only-cache`: fetched at run time into the user cache (`~/Library/Caches/designos-eval/<id>/<pinned>/` on macOS, the platform cache dir elsewhere); never committed, never shipped.
   - `eval-redistribute`: small derived fixtures may be committed under `crates/designos-eval/suites/` with attribution.
   - `ship-in-app` / `ship-in-skills`: files go into the app bundle or the skills folder with the licence text and attribution in `THIRD_PARTY_LICENSES.md`.
   - `excluded`: no task may reference it.
4. **Exclusion rule.** `licence = "NONE"` forces `use = "excluded"` (owner ruling). A `pending` review blocks every consumer except the review itself: `designos-eval` refuses to fetch or run a task whose source is `pending` or `rejected` (E15-25 reads this file).
5. **Licence review required before `approved`:** Owner ruling 2026-10-19: Design2Code, UICrit, RICO screenshots, UIClip weights and CLIP ViT-B/32 weights are approved as `eval-only-cache` only: downloaded at run time into the user cache by `designos-eval fetch`, never committed, never shipped. Their entries are filed with `review = "approved"`, `reviewer = "owner"`, `reviewed_on = "2026-10-19"`, and the RICO terms are still recorded verbatim in `notes`. Derived metadata (S1 reference blocks) stays in the cache too. Other sources still need the review below.
   - Design2Code: ODC-BY covers the dataset; page text and images are third-party web content. Required: confirm eval use of the screenshots and HTML; attribution text; no redistribution of pages in this repo (class `eval-only-cache`). Our committed artefacts are task ids only: page screenshots, HTML and derived block metadata (S1 reference blocks) stay in the user cache and are never committed (owner ruling above).
   - WebSight: CC BY 4.0 allows redistribution with attribution; confirm the licence of any embedded photos before committing rendered fixtures.
   - UICrit: CC BY 4.0 for critiques and ratings; RICO screenshot terms must be read and recorded separately (separate entry `rico-screenshots`). If RICO terms do not allow redistribution, screenshots stay `eval-only-cache`.
   - UIClip weights: MIT tag; record the paper citation as attribution; class `eval-only-cache` (weights never enter the app, R4.13).
   - CLIP ViT-B/32 weights: approved by the owner on 2026-10-19 as `eval-only-cache`, the same terms as UIClip (`review = "approved"`, `reviewer = "owner"`, `reviewed_on = "2026-10-19"`); MIT; weights never enter the app, R4.13. Pin: a real Hugging Face revision, never a made-up one. The entry takes the `sha` that `https://huggingface.co/api/models/openai/clip-vit-base-patch32` returns (`3d74acf9a28c67741b2f4f2ea7635f0aaf6f0268` on 2026-10-19). If that call cannot be made when the entry is written, `pinned` stays `"to be pinned by the implementer"` and the register check treats that literal as unpinned (`schema` fails for an `approved` entry until it is a revision).
   - shadcn/ui (MIT) and Carbon (Apache-2.0): we re-create tokens and component structures as our own design documents; record attribution; Apache-2.0 requires keeping the NOTICE text if any file is copied.
   - `jakubkrehel/skills`: MIT; class `ship-in-skills`; full licence text in `skills/vendor/` (E15-46) and in `THIRD_PARTY_LICENSES.md`.
   - Fonts used by E15-18 archetypes: one `font` entry per family with its licence (OFL-1.1 etc.), class `ship-in-app` only for Inter (bundled); others are fetched by `designos-fonts`.
6. **Client-file ban (AC3).** The check script fails when any path under `crates/designos-eval/` or any eval task file references:
   - a file name containing `VSF`, `PCP` (case-insensitive, whole token), or `Untitled UI`;
   - an absolute path under a user home directory (`/Users/`, `/home/`, `C:\Users\`);
   - a `.fig` or `.designos` file not produced by the E15-24 generator (generated files are recognised by the `gen-` name prefix and their profile sidecar).
   The ban list lives in the TOML block as `[ban] patterns = [...]` so it can grow without code changes.
7. **Attribution output.** `THIRD_PARTY_LICENSES.md` is generated from the TOML block for entries with a `ship-*` class (script `--write`), and the check fails when it is stale.

## 4. Data model / schema

```toml
[[source]]
id = "design2code"
name = "Design2Code test set"
kind = "dataset"
url = "https://huggingface.co/datasets/SALT-NLP/Design2Code"
pinned = "e2f61f6e9d9b5ee201b81d8e0ae4d37aa13f8f25"
licence = "ODC-By-1.0"
use = "eval-only-cache"
attribution = "Design2Code (Si et al., 2024), SALT-NLP, ODC-BY 1.0."
review = "approved"
reviewer = "owner"
reviewed_on = "2026-10-19"
notes = "Pages derive from third-party websites. Owner ruling 2026-10-19: eval-only-cache, never committed or shipped."
consumers = ["E15-27"]

[[source]]
id = "clip-vit-b32"
name = "CLIP ViT-B/32 weights"
kind = "weights"
url = "https://huggingface.co/openai/clip-vit-base-patch32"
pinned = "3d74acf9a28c67741b2f4f2ea7635f0aaf6f0268"
licence = "MIT"
use = "eval-only-cache"
attribution = "CLIP (Radford et al., 2021), OpenAI, MIT."
review = "approved"
reviewer = "owner"
reviewed_on = "2026-10-19"
notes = "Owner ruling 2026-10-19: eval-only-cache, same terms as UIClip. Licence is that of the openai/CLIP repository; the Hugging Face model has no licence tag. Revision read from the Hugging Face API on 2026-10-19."
consumers = ["E15-27"]

[ban]
patterns = ['(?i)\bvsf\b', '(?i)\bpcp\b', '(?i)untitled ui', '^/Users/', '^/home/', '^[A-Za-z]:\\Users\\']
```

## 5. Contracts

`python3 scripts/check_eval_licences.py [--write]`:
- exit 0: register valid, every consumer reference resolves, no banned path, `THIRD_PARTY_LICENSES.md` current;
- exit 1: findings, one per line, `<check>: <what to fix>`, for example `unlicensed-in-use: transitions-dev is referenced by skills/vendor/... but has licence NONE; remove the reference`;
- exit 2: the register cannot be parsed.

Checks: `schema`, `none-means-excluded`, `consumer-exists`, `task-source-registered` (every `source = "<id>"` in `crates/designos-eval/suites/**/tasks.toml` exists in the register), `client-file-ban`, `third-party-current`, `table-matches-toml`.

## 6. File-format mapping

None.

## 7. UI

None.

## 8. Edge cases

1. A source with two licences (code MIT, data CC BY): two entries with distinct ids.
2. Hugging Face revision moved upstream: we pin a revision SHA, so the fetch stays reproducible; a missing revision fails the fetch with the id and SHA.
3. The ban regex must not match legitimate words (`spec`, or `pcp` inside a longer word such as `pcpx`): whole-token matching, where a token is bounded by non-alphanumeric characters, so a hyphenated name that contains the token (`pcp-free.md`) does match. A test feeds `specs/pcpx.md` (must pass) and `fixtures/VSF - PCP.fig` and `specs/pcp-free.md` (must fail).
4. Register edits without a review date for an `approved` entry: schema failure.
5. Windows path separators in task files: normalised before matching.

## 9. Non-goals

- Legal advice. The register records the review; the owner approves.
- Fetching data (E15-25 does it, reading this register).

## 10. Test plan

Seam: `python3 scripts/check_eval_licences.py` in CI (new step in the `test` job before `cargo test`), plus `python3 -m unittest scripts/test_check_eval_licences.py`.

Fail-before: running the script on `main` exits 2 (no register).

| Test | Input | Expected |
|---|---|---|
| `every_eval_source_registered` (AC1) | suites referencing `design2code`, `uicrit` | exit 0 when registered; exit 1 `task-source-registered` when an id is removed |
| `unlicensed_marked_excluded` (AC2) | entry `transitions-dev` with `licence = "NONE"`, `use = "eval-only-cache"` | exit 1 `none-means-excluded` |
| `client_fixture_path_fails` (AC3) | `crates/designos-eval/suites/s3/tasks.toml` containing `path = "fixtures/VSF - PCP.fig"` | exit 1 `client-file-ban` |
| `home_path_fails` | `path = "/Users/x/Downloads/kit.fig"` | exit 1 |
| `word_containing_pcp_passes` | `specs/pcpx.md` | exit 0 |
| `hyphenated_pcp_token_fails` | `specs/pcp-free.md` | exit 1 `client-file-ban` |
| `unpinned_approved_entry_fails` | entry `clip-vit-b32` with `review = "approved"` and `pinned = "to be pinned by the implementer"` | exit 1 `schema` (an approved entry needs a commit SHA or revision) |
| `third_party_file_stale` | edit an attribution | exit 1 `third-party-current` |

Visual evidence: terminal capture of a passing run and of the three failing fixtures.

## 11. Performance budgets

Script run ≤ 2 s on the full repo. No effect on CLAUDE.md §4 invariants.

## 12. Dependencies and build order

- No dependencies. First task of the pack's eval track.
- Unblocks E15-25, E15-27, E15-28, E15-31, E15-32, E15-46.

## 13. Owner rulings that apply

- The eval never uses client files; only open kits and self-generated large files.
- Skills: adopt MIT `jakubkrehel/skills` with attribution; skip anything without a licence.
- R4.13: model weights used by the eval stay in the user cache and never ship in the app.
