# Report — `#1163` (Part 1): Fork-Safe PR Gate & `workflow_run` Labeller

- **Issue:** [`#1163`](https://github.com/jangtrinh/design-os-studio/issues/1163) (PM-routed, `v0.7.2` P1, US-12 Part 1)
- **Draft PR:** [`https://github.com/jangtrinh/design-os-studio/pull/1184`](https://github.com/jangtrinh/design-os-studio/pull/1184) (`jangtrinh/agy-1163-fork-gate` -> `main`, `Refs #1163`)
- **Evidence Directory:** `plans/reports/agy-1163-fork-gate-evidence/`

---

## 1. What Could NOT Be Verified (Stated First)

1. **Live cross-repository fork PR trigger (`pull_request` from an external fork):**
   - No external fork repository was available in this environment; PR [`#1184`](https://github.com/jangtrinh/design-os-studio/pull/1184) was opened from a branch on the same repository (`jangtrinh/design-os-studio`).
   - **Why fork behaviour is identical (`AC-01`):** `.github/workflows/pr-gate.yml` uses `on: pull_request` (never `pull_request_target`), declares top-level `permissions: contents: read`, and references zero `secrets.*`. On our same-repo PR runs ([`40253727791`](https://github.com/jangtrinh/design-os-studio/actions/runs/40253727791), [`40254805190`](https://github.com/jangtrinh/design-os-studio/actions/runs/40254805190), [`40256484143`](https://github.com/jangtrinh/design-os-studio/actions/runs/40256484143)), the runner's `GITHUB_TOKEN Permissions` block shows only `Contents: read` and `Metadata: read`. By GitHub Actions' platform rules for `on: pull_request` from public forks, a fork PR receives the exact same read-only `GITHUB_TOKEN` (`Contents: read`, `Metadata: read`) and has all repository secrets stripped by the platform.
2. **Automatic pre-merge `on: workflow_run` trigger on GitHub Actions:**
   - By GitHub Actions' security model, `on: workflow_run` workflows only register and auto-trigger once the workflow file (`.github/workflows/pr-labeller.yml`) and its sparse-checked-out helper (`scripts/pr_labels.py`) exist on the repository's default branch (`main`).
   - Before PR [`#1184`](https://github.com/jangtrinh/design-os-studio/pull/1184) is merged into `main`, we verified `.github/workflows/pr-labeller.yml`'s exact shell step and `scripts/pr_labels.py` against all three live `PR Gate` workflow runs on GitHub Actions:
     - Run [`40253727791`](https://github.com/jangtrinh/design-os-studio/actions/runs/40253727791) (missing artifact / early gate failure -> exits `0` cleanly and applies no labels),
     - Run [`40254805190`](https://github.com/jangtrinh/design-os-studio/actions/runs/40254805190) (`400` lines changed across `4` files touching `.github/` -> downloads artifact `12622676825`, validates fields with `jq -e` and `scripts/pr_labels.py`, applies `sensitive-path` and NOT `size/L` to PR [`#1184`](https://github.com/jangtrinh/design-os-studio/pull/1184)),
     - Run [`40256484143`](https://github.com/jangtrinh/design-os-studio/actions/runs/40256484143) (`801` lines changed across `5` files after adding temporary 401-line fixture -> downloads artifact `12622329393`, validates fields, and applies `size/L` + `sensitive-path` to PR [`#1184`](https://github.com/jangtrinh/design-os-studio/pull/1184)).
3. **Part 2 (`CONTRIBUTING.md` Appendix A and `.github/pull_request_template.md` Appendix B):**
   - Explicitly deferred to Part 2 per task prompt ("Part 2 (CONTRIBUTING Appendix A and PR template Appendix B) comes later, once the PM provides the text. Do not write those texts yourself.").

---

## 2. Conclusion & Epistemic Ledger

- **Conclusion:** `.github/workflows/pr-gate.yml`, `.github/workflows/pr-labeller.yml`, `scripts/pr_labels.py`, and `scripts/test_pr_labels.py` are implemented, linted clean with `actionlint`, unit-tested red-then-green (`6/6` tests passing), and verified against live GitHub Actions `PR Gate` runs on Draft PR [`#1184`](https://github.com/jangtrinh/design-os-studio/pull/1184) without modifying `.github/workflows/ci.yml` or any required check names (`Check`, `Test (macos-14)`, `Clippy`, `Rustfmt`).
- **Epistemic Ledger:**
  - `FACT:` `python3 -B -m unittest scripts/test_pr_labels.py` failed with `ModuleNotFoundError: No module named 'pr_labels'` before `scripts/pr_labels.py` was created (`plans/reports/agy-1163-fork-gate-evidence/red-test-pr-labels.log`) and passed all `6` unit tests (`Ran 6 tests in 0.044s — OK`) after implementation (`plans/reports/agy-1163-fork-gate-evidence/green-test-pr-labels.log`).
  - `FACT:` `actionlint -verbose .github/workflows/pr-gate.yml .github/workflows/pr-labeller.yml` returned `Found 0 errors in 2 files` (exit `0`, `plans/reports/agy-1163-fork-gate-evidence/actionlint.log`).
  - `FACT:` On `PR Gate` runs [`40253727791`](https://github.com/jangtrinh/design-os-studio/actions/runs/40253727791), [`40254805190`](https://github.com/jangtrinh/design-os-studio/actions/runs/40254805190), and [`40256484143`](https://github.com/jangtrinh/design-os-studio/actions/runs/40256484143), the runner log `GITHUB_TOKEN Permissions` block shows only `Contents: read` and `Metadata: read` (`plans/reports/agy-1163-fork-gate-evidence/pr-gate-permissions.log`).
  - `FACT:` `PR Gate` run [`40254805190`](https://github.com/jangtrinh/design-os-studio/actions/runs/40254805190) passed all 4 jobs (`PR Gate (rustfmt)`, `PR Gate (clippy)`, `PR Gate (lib tests)`, `PR Gate (result artifact)`) at `400` lines / `4` files and uploaded artifact `12622676825` (`plans/reports/agy-1163-fork-gate-evidence/pr-gate-400-result.json`); the labeller logic applied `sensitive-path` (and not `size/L`) to PR [`#1184`](https://github.com/jangtrinh/design-os-studio/pull/1184) (`plans/reports/agy-1163-fork-gate-evidence/labeller-400-sensitive-path.log`).
  - `FACT:` `PR Gate` run [`40256484143`](https://github.com/jangtrinh/design-os-studio/actions/runs/40256484143) (with temporary 401-line file `plans/reports/agy-1163-fork-gate-evidence/temp-401-lines.txt`) passed all 4 jobs at `801` lines / `5` files and uploaded artifact `12622329393` (`plans/reports/agy-1163-fork-gate-evidence/pr-gate-801-result.json`); the labeller logic applied `size/L` and `sensitive-path` to PR [`#1184`](https://github.com/jangtrinh/design-os-studio/pull/1184) (`plans/reports/agy-1163-fork-gate-evidence/labeller-801-size-l.log`).
  - `INFERENCE:` Once PR [`#1184`](https://github.com/jangtrinh/design-os-studio/pull/1184) merges to `main`, GitHub Actions will automatically trigger `.github/workflows/pr-labeller.yml` via `on: workflow_run` whenever `PR Gate` completes on any future same-repo or fork PR.
- **Weakest Link:** Pre-merge `on: workflow_run` registration cannot be auto-triggered by GitHub Actions until `.github/workflows/pr-labeller.yml` lands on `main`; mitigated by executing `.github/workflows/pr-labeller.yml`'s exact shell step against all three live `PR Gate` runs (`40253727791`, `40254805190`, `40256484143`) and validating with `actionlint`.
- **Assumptions:** None remaining.
- **Unresolved Questions:** None for Part 1 (Part 2 awaits PM Appendix A & B copy).

---

## 3. Acceptance Table (Commands, Outputs & Run Links)

| # | Acceptance Check | Command / Run Link | Result |
|---|---|---|---|
| **1a** | Unit tests red first (missing `scripts/pr_labels.py`) | `python3 -B -m unittest scripts/test_pr_labels.py` | **PASS (Red):** `ModuleNotFoundError: No module named 'pr_labels'` (`plans/reports/agy-1163-fork-gate-evidence/red-test-pr-labels.log`) |
| **1b** | Unit tests green (`400` vs `401` lines, `10` vs `11` files, rename into sensitive path, missing artifact, malformed artifact) | `python3 -B -m unittest scripts/test_pr_labels.py -v` | **PASS (Green):** `Ran 6 tests in 0.044s — OK` (`plans/reports/agy-1163-fork-gate-evidence/green-test-pr-labels.log`) |
| **2** | `actionlint` clean on both workflows (`AC-04`) | `$HOME/.local/bin/actionlint -verbose .github/workflows/pr-gate.yml .github/workflows/pr-labeller.yml` | **PASS:** `Found 0 errors in 2 files` (exit `0`, `plans/reports/agy-1163-fork-gate-evidence/actionlint.log`) |
| **3a** | `PR Gate` runs on PR with read-only `GITHUB_TOKEN` (`AC-01`) | [`https://github.com/jangtrinh/design-os-studio/actions/runs/40253727791/job/121350963382`](https://github.com/jangtrinh/design-os-studio/actions/runs/40253727791/job/121350963382), [`https://github.com/jangtrinh/design-os-studio/actions/runs/40254805190`](https://github.com/jangtrinh/design-os-studio/actions/runs/40254805190) | **PASS:** `Contents: read`, `Metadata: read` (`plans/reports/agy-1163-fork-gate-evidence/pr-gate-permissions.log`) |
| **3b-i** | Labeller exits `0` and does nothing when `pr-gate-result` artifact is missing (early gate failure) | Run [`40253727791`](https://github.com/jangtrinh/design-os-studio/actions/runs/40253727791) -> `plans/reports/agy-1163-fork-gate-evidence/labeller-missing-artifact.log` | **PASS:** `::notice::pr-gate-result artifact is missing for run 40253727791; nothing to label.` (exit `0`) |
| **3b-ii** | Labeller adds `sensitive-path` (and not `size/L` at `400` lines) (`AC-02`) | Run [`40254805190`](https://github.com/jangtrinh/design-os-studio/actions/runs/40254805190) (artifact `12622676825`) -> PR [`#1184`](https://github.com/jangtrinh/design-os-studio/pull/1184) | **PASS:** `{"labels":["sensitive-path"],"number":1184}` (`plans/reports/agy-1163-fork-gate-evidence/labeller-400-sensitive-path.log`) |
| **3c** | Temporary 401-line file commit triggers `size/L` (`AC-03`), then temporary file is removed in a new commit | Run [`40256484143`](https://github.com/jangtrinh/design-os-studio/actions/runs/40256484143) (artifact `12622329393`, commit `bc500b2b05b3cd9baef545cff81b407600c391ea`) -> PR [`#1184`](https://github.com/jangtrinh/design-os-studio/pull/1184) | **PASS:** `{"labels":["sensitive-path","size/L"],"number":1184}` (`plans/reports/agy-1163-fork-gate-evidence/labeller-801-size-l.log`) |

---

## 4. Pasted Commands & Verbatim Outputs

### 4.1 Acceptance 1 — Red-First & Green Unit Tests (`scripts/test_pr_labels.py`)

**Red (before `scripts/pr_labels.py` existed):**
```text
$ python3 -B -m unittest scripts/test_pr_labels.py
E
======================================================================
ERROR: test_pr_labels (unittest.loader._FailedTest.test_pr_labels)
----------------------------------------------------------------------
ImportError: Failed to import test module: test_pr_labels
Traceback (most recent call last):
  File "/opt/homebrew/Cellar/python@3.14/3.14.6/Frameworks/Python.framework/Versions/3.14/lib/python3.14/unittest/loader.py", line 137, in loadTestsFromName
    module = __import__(module_name)
  File "/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1163-fork-gate/scripts/test_pr_labels.py", line 10, in <module>
    import pr_labels  # noqa: E402
    ^^^^^^^^^^^^^^^^
ModuleNotFoundError: No module named 'pr_labels'


----------------------------------------------------------------------
Ran 1 test in 0.000s

FAILED (errors=1)
```

**Green (after implementing `scripts/pr_labels.py`):**
```text
$ python3 -B -m unittest scripts/test_pr_labels.py -v
test_10_files_adds_no_size_label_while_11_files_adds_size_l (scripts.test_pr_labels.PrLabelsTests.test_10_files_adds_no_size_label_while_11_files_adds_size_l) ... ok
test_400_lines_adds_no_size_label_while_401_lines_adds_size_l (scripts.test_pr_labels.PrLabelsTests.test_400_lines_adds_no_size_label_while_401_lines_adds_size_l) ... ok
test_malformed_artifact_is_rejected (scripts.test_pr_labels.PrLabelsTests.test_malformed_artifact_is_rejected) ... ok
test_missing_artifact_exits_zero_and_outputs_nothing (scripts.test_pr_labels.PrLabelsTests.test_missing_artifact_exits_zero_and_outputs_nothing) ... ok
test_rename_into_sensitive_path_triggers_sensitive_path_label (scripts.test_pr_labels.PrLabelsTests.test_rename_into_sensitive_path_triggers_sensitive_path_label) ... ok
test_sensitive_paths_match_all_configured_categories (scripts.test_pr_labels.PrLabelsTests.test_sensitive_paths_match_all_configured_categories) ... ok

----------------------------------------------------------------------
Ran 6 tests in 0.044s

OK
```

### 4.2 Acceptance 2 — `actionlint` on Both Workflows (`AC-04`)

```text
$ $HOME/.local/bin/actionlint -verbose .github/workflows/pr-gate.yml .github/workflows/pr-labeller.yml
verbose: Linting 2 files
verbose: Linting .github/workflows/pr-labeller.yml
verbose: Using project at /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1163-fork-gate
verbose: Linting .github/workflows/pr-gate.yml
verbose: Using project at /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1163-fork-gate
verbose: Found 0 parse errors in 0 ms for .github/workflows/pr-labeller.yml
verbose: Found 0 parse errors in 0 ms for .github/workflows/pr-gate.yml
verbose: Rule "shellcheck" was disabled: exec: "shellcheck": executable file not found in $PATH
verbose: Rule "shellcheck" was disabled: exec: "shellcheck": executable file not found in $PATH
verbose: Rule "pyflakes" was disabled: exec: "pyflakes": executable file not found in $PATH
verbose: verbose: Found total 0 errors in 0 ms for .github/workflows/pr-labeller.yml
Rule "pyflakes" was disabled: exec: "pyflakes": executable file not found in $PATH
verbose: Found total 0 errors in 0 ms for .github/workflows/pr-gate.yml
verbose: Found 0 errors in 2 files
```

### 4.3 Acceptance 3a — `GITHUB_TOKEN Permissions` on `PR Gate` (`AC-01`)

From [`https://github.com/jangtrinh/design-os-studio/actions/runs/40253727791/job/121350963382`](https://github.com/jangtrinh/design-os-studio/actions/runs/40253727791/job/121350963382) (and identical on [`40254805190`](https://github.com/jangtrinh/design-os-studio/actions/runs/40254805190) and [`40256484143`](https://github.com/jangtrinh/design-os-studio/actions/runs/40256484143)):
```text
2026-10-26T01:20:20.2952588Z ##[group]GITHUB_TOKEN Permissions
2026-10-26T01:20:20.2954330Z Contents: read
2026-10-26T01:20:20.2954816Z Metadata: read
2026-10-26T01:20:20.2955387Z ##[endgroup]
```
*(A fork PR triggered via `on: pull_request` receives the exact same read-only `GITHUB_TOKEN` permissions (`Contents: read`, `Metadata: read`) and has no access to repository secrets by GitHub's rules.)*

### 4.4 Acceptance 3b — Missing Artifact Edge Case & `sensitive-path` Label at `400` Lines (`AC-02`)

**Early gate failure / missing artifact (`RUN_ID=40253727791`, [`https://github.com/jangtrinh/design-os-studio/actions/runs/40253727791`](https://github.com/jangtrinh/design-os-studio/actions/runs/40253727791)):**
```text
::notice::pr-gate-result artifact is missing for run 40253727791; nothing to label.
```

**400-line boundary run (`RUN_ID=40254805190`, [`https://github.com/jangtrinh/design-os-studio/actions/runs/40254805190`](https://github.com/jangtrinh/design-os-studio/actions/runs/40254805190), artifact `12622676825`):**
```text
{
  "pr_number": 1184,
  "head_sha": "dc892a8026a04aff966544bd299f1b2a3e96a11e",
  "additions": 400,
  "deletions": 0,
  "lines_changed": 400,
  "changed_files": 4,
  "paths": [
    ".github/workflows/pr-gate.yml",
    ".github/workflows/pr-labeller.yml",
    "scripts/pr_labels.py",
    "scripts/test_pr_labels.py"
  ]
}
labels_output=sensitive-path
{"labels":["sensitive-path"],"number":1184}
```

### 4.5 Acceptance 3c — Temporary 401-Line Fixture Adds `size/L` (`AC-03`)

**801-line run with temporary 401-line file (`RUN_ID=40256484143`, [`https://github.com/jangtrinh/design-os-studio/actions/runs/40256484143`](https://github.com/jangtrinh/design-os-studio/actions/runs/40256484143), artifact `12622329393`, commit `bc500b2b05b3cd9baef545cff81b407600c391ea`):**
```text
artifact_id=12622329393
{
  "pr_number": 1184,
  "head_sha": "bc500b2b05b3cd9baef545cff81b407600c391ea",
  "additions": 801,
  "deletions": 0,
  "lines_changed": 801,
  "changed_files": 5,
  "paths": [
    ".github/workflows/pr-gate.yml",
    ".github/workflows/pr-labeller.yml",
    "plans/reports/agy-1163-fork-gate-evidence/temp-401-lines.txt",
    "scripts/pr_labels.py",
    "scripts/test_pr_labels.py"
  ]
}
jq_validation=ok
labels_output=size/L,sensitive-path
{"labels":["sensitive-path","size/L"],"number":1184}
```

---

## 5. Exact Sensitive-Path Prefix Patterns (`SENSITIVE_PATH_PREFIXES` in `scripts/pr_labels.py`)

Any touched path (including both the source and destination paths of a `git diff --numstat` rename such as `old => new` or `prefix/{old => new}/suffix`) starting with one of the following prefixes triggers `sensitive-path`:

1. `.github/`
2. `packaging/`
3. `scripts/build-release.sh`
4. `scripts/package.sh`
5. `scripts/package-macos.sh`
6. `scripts/release-smoke-macos.sh`
7. `scripts/release-sync-pages.py`
8. `crates/designos-mcp/`
9. `crates/designos-eval/src/sandbox.rs`
10. `crates/designos-app/src/agent_permissions.rs`
11. `crates/designos-app/src/agent_session` *(matches `agent_sessions.rs`, `agent_session_files.rs`, `agent_session_failure.rs`, and `agent_session*`)*
12. `crates/designos-app/src/skills/hook_delivery.rs`
13. `crates/designos-app/src/terminal` *(matches `terminal.rs`, `terminal/`, and `terminal_undo_chord.rs`)*
14. `crates/designos-app/src/shell/tests/terminal_undo_chord.rs`
15. `skills/vendor/`

---

## 6. Skills Loaded & What Changed Because of Them

1. **`es:dev` & `es:feature-dev`** (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`):
   - Enforced red-first TDD (`scripts/test_pr_labels.py` written and run before `scripts/pr_labels.py` existed) and strict file-scope discipline (leaving `.github/workflows/ci.yml`, `CONTRIBUTING.md`, and `.github/pull_request_template.md` untouched).
2. **`es:rust` & `es:rust-gpu-canvas`** (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`):
   - Diagnosed why `cargo test --workspace --lib --locked` hit OOM (`SIGKILL`, signal 9) on `ubuntu-latest` during `designos-fig`'s `chained_links_stop_at_the_global_clone_budget` test (`8,000,000` `Node` clones in debug mode) and fixed `PR Gate (lib tests)` by provisioning a 16 GB swapfile and running `RUST_TEST_THREADS=1`.
3. **`es:astra-work`** (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`):
   - Guided post-mutation verification on live GitHub Actions runs (`40253727791`, `40254805190`, `40256484143`) and caught a `jq` pipe-scoping bug in our ad-hoc test harness while confirming `.github/workflows/pr-labeller.yml`'s `jq -e` validator passed cleanly (`jq_validation=ok`).
4. **`es:fable-thinking`** (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`):
   - Enforced `PREDICT:` before consequential tool calls, structured the Epistemic Ledger (`FACT` / `INFERENCE` / `ASSUMPTION`), and placed `What Could NOT Be Verified` (fork-only trigger & pre-merge `on: workflow_run` platform rule) at the very top of this report.

---

## Round 2

### R2.1 What Could NOT Be Verified (Stated First)

1. **Live cross-repository fork PR trigger (`pull_request` from an external fork):**
   - No external fork repository was available in this environment; PR [`#1184`](https://github.com/jangtrinh/design-os-studio/pull/1184) is on `jangtrinh/design-os-studio`.
   - Under Round 2, even when a fork rewrites `.github/workflows/pr-gate.yml` on its merge commit to forge `pr_number`, forge `paths`/`additions`/`deletions`, upload an oversized artifact, or skip uploading the artifact entirely, `.github/workflows/pr-labeller.yml` resolves the PR and its changed files/counts directly from GitHub's REST API (`commits/{head_sha}/pulls`, `pulls/{n}`, and `pulls/{n}/files`) verified against `workflow_run.head_sha` and `workflow_run.head_repository.full_name`.
2. **Automatic pre-merge `on: workflow_run` trigger on GitHub Actions:**
   - By GitHub Actions' platform rules, `on: workflow_run` workflows only register and fire automatically once `.github/workflows/pr-labeller.yml` and `scripts/pr_labels.py` are merged into the default branch (`main`).
   - Verified pre-merge via a live read-only dry run against PR [`#1184`](https://github.com/jangtrinh/design-os-studio/pull/1184) using the exact `gh api` and `scripts/pr_labels.py` commands from `.github/workflows/pr-labeller.yml` (`plans/reports/agy-1163-fork-gate-evidence/r2-dry-run.log`).

### R2.2 Conclusion, Epistemic Ledger, Weakest Link & Assumptions

- **Conclusion:** `.github/workflows/pr-labeller.yml`, `scripts/pr_labels.py`, and `scripts/test_pr_labels.py` have been updated per all 6 controller rulings for Round 2 (leaving `.github/workflows/pr-gate.yml` untouched). The labeller now resolves the PR number and all line/file counts and paths from GitHub's REST API (`commits/{head_sha}/pulls`, `pulls/{n}`, `pulls/{n}/files`), treats the artifact `pr_number` solely as a fallback candidate verified against `pulls/{n}` (`head.sha` and `head.repo.full_name`), never reads counts or paths from the artifact, applies both `size/L` and `sensitive-path` when `pulls/{n}/files` is truncated below `changed_files` (the 3,000-file API cap), still labels PRs found via `commits/{head_sha}/pulls` when the artifact is missing, and bounds candidate artifact downloads to `<= 64 KiB` (`65,536` bytes).
- **Epistemic Ledger:**
  - `FACT:` All new Round 2 unit tests in `scripts/test_pr_labels.py` failed on the Round 1 head (`2d3012ca1ea064fd03638753139fe56e614ac07c`, `FAILED (failures=1, errors=9)` in `plans/reports/agy-1163-fork-gate-evidence/r2-red.log`) and passed after updating `scripts/pr_labels.py` (`Ran 12 tests in 0.095s — OK` in `plans/reports/agy-1163-fork-gate-evidence/r2-green.log`).
  - `FACT:` `$HOME/.local/bin/actionlint -verbose .github/workflows/pr-gate.yml .github/workflows/pr-labeller.yml` passed with `Found 0 errors in 2 files` (`plans/reports/agy-1163-fork-gate-evidence/r2-actionlint.log`).
  - `FACT:` In the live read-only dry run against PR [`#1184`](https://github.com/jangtrinh/design-os-studio/pull/1184) (`headRefOid = 2d3012ca1ea064fd03638753139fe56e614ac07c`, `additions = 1387`, `deletions = 0`, `changed_files = 16`), Rule 1's lookup (`commits/{head_sha}/pulls`) resolved `1184`, `scripts/pr_labels.py` produced `{"pr_number": 1184, "labels": ["size/L", "sensitive-path"]}`, and passing a hand-written conflicting artifact (`pr_number: 1`, `paths: ["README.md"]`, `lines_changed: 1`) still resolved `1184` and produced `{"pr_number": 1184, "labels": ["size/L", "sensitive-path"]}` (`plans/reports/agy-1163-fork-gate-evidence/r2-dry-run.log`).
  - `INFERENCE:` Once PR [`#1184`](https://github.com/jangtrinh/design-os-studio/pull/1184) merges to `main`, fork PRs cannot cross-target another issue/PR (Attack A), skip labels by forging or omitting the artifact (Attack B), or wedge the runner with an oversized artifact.
- **Weakest Link:** Pre-merge `on: workflow_run` auto-triggering requires `.github/workflows/pr-labeller.yml` on `main`; mitigated by running the exact workflow commands live against PR `#1184`'s GitHub API payloads and unit-testing all 7 fallback/attack branches.
- **Assumptions:** None. (`.github/workflows/pr-gate.yml` required no changes because all 6 rulings operate in `.github/workflows/pr-labeller.yml` and `scripts/pr_labels.py`.)
- **Unresolved Questions:** None.

### R2.3 Round 2 Acceptance Table

| # | Acceptance Check | Evidence File | Result |
|---|---|---|---|
| **1a** | Red first: new Round 2 tests fail on Round 1 head (`2d3012ca1ea064fd03638753139fe56e614ac07c`) | `plans/reports/agy-1163-fork-gate-evidence/r2-red.log` | **PASS (Red):** `Ran 12 tests in 0.024s — FAILED (failures=1, errors=9)` |
| **1b** | Green: all 7 required Round 2 tests + 5 boundary/sensitive/size-cap tests pass | `plans/reports/agy-1163-fork-gate-evidence/r2-green.log` | **PASS (Green):** `Ran 12 tests in 0.095s — OK` |
| **2** | `actionlint` clean on `.github/workflows/pr-gate.yml` and `.github/workflows/pr-labeller.yml` | `plans/reports/agy-1163-fork-gate-evidence/r2-actionlint.log` | **PASS:** `Found 0 errors in 2 files` (exit `0`) |
| **3** | Live read-only dry run against PR `#1184` (resolves `1184` from `commits/{sha}/pulls`, labels from `pulls/1184` + `pulls/1184/files`, and ignores conflicting `pr_number: 1` / `README.md` artifact) | `plans/reports/agy-1163-fork-gate-evidence/r2-dry-run.log` | **PASS:** `{"pr_number": 1184, "labels": ["size/L", "sensitive-path"]}` both without and with the conflicting artifact |

### R2.4 Skills Loaded in Round 2 & What Changed Because of Them

1. **`es:dev` & `es:feature-dev`** (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`):
   - Executed red-first TDD against the Round 1 head before editing `scripts/pr_labels.py`, kept seam tests at the public `evaluate_pr_labels` / CLI boundary, and respected the allowed file scope (`.github/workflows/pr-labeller.yml`, `scripts/pr_labels.py`, `scripts/test_pr_labels.py`, report, and evidence).
2. **`es:rust` & `es:rust-gpu-canvas`** (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`):
   - Loaded per owner instruction; no Rust crates or `.github/workflows/pr-gate.yml` Rust jobs were modified in Round 2.
3. **`es:astra-work` & `es:fable-thinking` & `es:orca-native-orchestration`** (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-orca-native-orchestration/SKILL.md`):
   - Applied `PREDICT:` before tool calls, verified multi-page `gh api --paginate` JSON array handling in `_load_json_file`, enforced the 64 KiB cap in both the workflow (`size_in_bytes <= 65536` + `head -c 65537`) and `scripts/pr_labels.py` (`MAX_ARTIFACT_BYTES = 65536`), and placed `What Could NOT Be Verified` first.

### R2.5 Pasted Commands & Verbatim Outputs (Round 2)

#### 1. Red-First Unit Tests on Round 1 Head (`plans/reports/agy-1163-fork-gate-evidence/r2-red.log`)

```text
$ python3 -B -m unittest scripts/test_pr_labels.py -v
test_10_files_adds_no_size_label_while_11_files_adds_size_l (scripts.test_pr_labels.PrLabelsTests.test_10_files_adds_no_size_label_while_11_files_adds_size_l) ... ERROR
test_400_lines_adds_no_size_label_while_401_lines_adds_size_l (scripts.test_pr_labels.PrLabelsTests.test_400_lines_adds_no_size_label_while_401_lines_adds_size_l) ... ERROR
test_candidate_number_with_different_head_repo_is_refused (scripts.test_pr_labels.PrLabelsTests.test_candidate_number_with_different_head_repo_is_refused) ... ERROR
test_candidate_number_with_different_head_sha_is_refused (scripts.test_pr_labels.PrLabelsTests.test_candidate_number_with_different_head_sha_is_refused) ... ERROR
test_files_list_shorter_than_changed_files_gives_both_labels (scripts.test_pr_labels.PrLabelsTests.test_files_list_shorter_than_changed_files_gives_both_labels) ... ERROR
test_labels_come_from_files_list_ignoring_conflicting_artifact (scripts.test_pr_labels.PrLabelsTests.test_labels_come_from_files_list_ignoring_conflicting_artifact) ... ERROR
test_no_pr_found_and_no_artifact_gives_no_labels_and_no_error (scripts.test_pr_labels.PrLabelsTests.test_no_pr_found_and_no_artifact_gives_no_labels_and_no_error) ... ERROR
test_oversized_artifact_over_64_kib_is_refused (scripts.test_pr_labels.PrLabelsTests.test_oversized_artifact_over_64_kib_is_refused) ... FAIL
test_pr_found_with_no_artifact_is_still_labelled (scripts.test_pr_labels.PrLabelsTests.test_pr_found_with_no_artifact_is_still_labelled) ... ERROR
test_rename_into_github_counts_as_one_file_and_is_sensitive (scripts.test_pr_labels.PrLabelsTests.test_rename_into_github_counts_as_one_file_and_is_sensitive) ... ERROR
test_rename_into_sensitive_path_triggers_sensitive_path_label (scripts.test_pr_labels.PrLabelsTests.test_rename_into_sensitive_path_triggers_sensitive_path_label) ... ok
test_sensitive_paths_match_all_configured_categories (scripts.test_pr_labels.PrLabelsTests.test_sensitive_paths_match_all_configured_categories) ... ok

======================================================================
ERROR: test_10_files_adds_no_size_label_while_11_files_adds_size_l (scripts.test_pr_labels.PrLabelsTests.test_10_files_adds_no_size_label_while_11_files_adds_size_l)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1163-fork-gate/scripts/test_pr_labels.py", line 70, in test_10_files_adds_no_size_label_while_11_files_adds_size_l
    pr_labels.evaluate_pr_labels(
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
AttributeError: module 'pr_labels' has no attribute 'evaluate_pr_labels'

======================================================================
ERROR: test_400_lines_adds_no_size_label_while_401_lines_adds_size_l (scripts.test_pr_labels.PrLabelsTests.test_400_lines_adds_no_size_label_while_401_lines_adds_size_l)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1163-fork-gate/scripts/test_pr_labels.py", line 51, in test_400_lines_adds_no_size_label_while_401_lines_adds_size_l
    pr_labels.evaluate_pr_labels(
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
AttributeError: module 'pr_labels' has no attribute 'evaluate_pr_labels'

======================================================================
ERROR: test_candidate_number_with_different_head_repo_is_refused (scripts.test_pr_labels.PrLabelsTests.test_candidate_number_with_different_head_repo_is_refused)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1163-fork-gate/scripts/test_pr_labels.py", line 121, in test_candidate_number_with_different_head_repo_is_refused
    pr_labels.evaluate_pr_labels(
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
AttributeError: module 'pr_labels' has no attribute 'evaluate_pr_labels'

======================================================================
ERROR: test_candidate_number_with_different_head_sha_is_refused (scripts.test_pr_labels.PrLabelsTests.test_candidate_number_with_different_head_sha_is_refused)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1163-fork-gate/scripts/test_pr_labels.py", line 110, in test_candidate_number_with_different_head_sha_is_refused
    pr_labels.evaluate_pr_labels(
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
AttributeError: module 'pr_labels' has no attribute 'evaluate_pr_labels'

======================================================================
ERROR: test_files_list_shorter_than_changed_files_gives_both_labels (scripts.test_pr_labels.PrLabelsTests.test_files_list_shorter_than_changed_files_gives_both_labels)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1163-fork-gate/scripts/test_pr_labels.py", line 187, in test_files_list_shorter_than_changed_files_gives_both_labels
    pr_labels.evaluate_pr_labels(
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
AttributeError: module 'pr_labels' has no attribute 'evaluate_pr_labels'

======================================================================
ERROR: test_labels_come_from_files_list_ignoring_conflicting_artifact (scripts.test_pr_labels.PrLabelsTests.test_labels_come_from_files_list_ignoring_conflicting_artifact)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1163-fork-gate/scripts/test_pr_labels.py", line 143, in test_labels_come_from_files_list_ignoring_conflicting_artifact
    pr_labels.evaluate_pr_labels(
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
AttributeError: module 'pr_labels' has no attribute 'evaluate_pr_labels'

======================================================================
ERROR: test_no_pr_found_and_no_artifact_gives_no_labels_and_no_error (scripts.test_pr_labels.PrLabelsTests.test_no_pr_found_and_no_artifact_gives_no_labels_and_no_error)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1163-fork-gate/scripts/test_pr_labels.py", line 199, in test_no_pr_found_and_no_artifact_gives_no_labels_and_no_error
    pr_labels.evaluate_pr_labels(pulls=[], head_sha=SHA_A, head_repo=REPO),
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
AttributeError: module 'pr_labels' has no attribute 'evaluate_pr_labels'

======================================================================
ERROR: test_pr_found_with_no_artifact_is_still_labelled (scripts.test_pr_labels.PrLabelsTests.test_pr_found_with_no_artifact_is_still_labelled)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1163-fork-gate/scripts/test_pr_labels.py", line 222, in test_pr_found_with_no_artifact_is_still_labelled
    pr_labels.evaluate_pr_labels(
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
AttributeError: module 'pr_labels' has no attribute 'evaluate_pr_labels'

======================================================================
ERROR: test_rename_into_github_counts_as_one_file_and_is_sensitive (scripts.test_pr_labels.PrLabelsTests.test_rename_into_github_counts_as_one_file_and_is_sensitive)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1163-fork-gate/scripts/test_pr_labels.py", line 171, in test_rename_into_github_counts_as_one_file_and_is_sensitive
    paths = pr_labels.extract_file_paths(files)
            ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
AttributeError: module 'pr_labels' has no attribute 'extract_file_paths'

======================================================================
FAIL: test_oversized_artifact_over_64_kib_is_refused (scripts.test_pr_labels.PrLabelsTests.test_oversized_artifact_over_64_kib_is_refused)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1163-fork-gate/scripts/test_pr_labels.py", line 262, in test_oversized_artifact_over_64_kib_is_refused
    self.assertEqual(proc.returncode, 1)
    ~~~~~~~~~~~~~~~~^^^^^^^^^^^^^^^^^^^^
AssertionError: 2 != 1

----------------------------------------------------------------------
Ran 12 tests in 0.024s

FAILED (failures=1, errors=9)
```

#### 2. Green Unit Tests (`plans/reports/agy-1163-fork-gate-evidence/r2-green.log`)

```text
$ python3 -B -m unittest scripts/test_pr_labels.py -v
test_10_files_adds_no_size_label_while_11_files_adds_size_l (scripts.test_pr_labels.PrLabelsTests.test_10_files_adds_no_size_label_while_11_files_adds_size_l) ... ok
test_400_lines_adds_no_size_label_while_401_lines_adds_size_l (scripts.test_pr_labels.PrLabelsTests.test_400_lines_adds_no_size_label_while_401_lines_adds_size_l) ... ok
test_candidate_number_with_different_head_repo_is_refused (scripts.test_pr_labels.PrLabelsTests.test_candidate_number_with_different_head_repo_is_refused) ... ok
test_candidate_number_with_different_head_sha_is_refused (scripts.test_pr_labels.PrLabelsTests.test_candidate_number_with_different_head_sha_is_refused) ... ok
test_files_list_shorter_than_changed_files_gives_both_labels (scripts.test_pr_labels.PrLabelsTests.test_files_list_shorter_than_changed_files_gives_both_labels) ... ok
test_labels_come_from_files_list_ignoring_conflicting_artifact (scripts.test_pr_labels.PrLabelsTests.test_labels_come_from_files_list_ignoring_conflicting_artifact) ... ok
test_no_pr_found_and_no_artifact_gives_no_labels_and_no_error (scripts.test_pr_labels.PrLabelsTests.test_no_pr_found_and_no_artifact_gives_no_labels_and_no_error) ... ok
test_oversized_artifact_over_64_kib_is_refused (scripts.test_pr_labels.PrLabelsTests.test_oversized_artifact_over_64_kib_is_refused) ... ok
test_pr_found_with_no_artifact_is_still_labelled (scripts.test_pr_labels.PrLabelsTests.test_pr_found_with_no_artifact_is_still_labelled) ... ok
test_rename_into_github_counts_as_one_file_and_is_sensitive (scripts.test_pr_labels.PrLabelsTests.test_rename_into_github_counts_as_one_file_and_is_sensitive) ... ok
test_rename_into_sensitive_path_triggers_sensitive_path_label (scripts.test_pr_labels.PrLabelsTests.test_rename_into_sensitive_path_triggers_sensitive_path_label) ... ok
test_sensitive_paths_match_all_configured_categories (scripts.test_pr_labels.PrLabelsTests.test_sensitive_paths_match_all_configured_categories) ... ok

----------------------------------------------------------------------
Ran 12 tests in 0.095s

OK
```

#### 3. `actionlint` on Both Workflows (`plans/reports/agy-1163-fork-gate-evidence/r2-actionlint.log`)

```text
$ $HOME/.local/bin/actionlint -verbose .github/workflows/pr-gate.yml .github/workflows/pr-labeller.yml
verbose: Linting 2 files
verbose: Linting .github/workflows/pr-gate.yml
verbose: Using project at /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1163-fork-gate
verbose: Linting .github/workflows/pr-labeller.yml
verbose: Using project at /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1163-fork-gate
verbose: Found 0 parse errors in 0 ms for .github/workflows/pr-labeller.yml
verbose: Found 0 parse errors in 0 ms for .github/workflows/pr-gate.yml
verbose: Rule "shellcheck" was disabled: exec: "shellcheck": executable file not found in $PATH
verbose: Rule "shellcheck" was disabled: exec: "shellcheck": executable file not found in $PATH
verbose: Rule "pyflakes" was disabled: exec: "pyflakes": executable file not found in $PATH
verbose: Rule "pyflakes" was disabled: exec: "pyflakes": executable file not found in $PATH
verbose: Found total 0 errors in 0 ms for .github/workflows/pr-labeller.yml
verbose: Found total 0 errors in 0 ms for .github/workflows/pr-gate.yml
verbose: Found 0 errors in 2 files
```

#### 4. Live Read-Only Dry Run Against PR `#1184` (`plans/reports/agy-1163-fork-gate-evidence/r2-dry-run.log`)

```text
+zsh:3> GH_REPO=jangtrinh/design-os-studio 
+zsh:4> EXPECTED_HEAD_SHA=+zsh:4> gh pr view 1184 --json headRefOid --jq .headRefOid
+zsh:4> EXPECTED_HEAD_SHA=2d3012ca1ea064fd03638753139fe56e614ac07c 
+zsh:5> EXPECTED_HEAD_REPO=jangtrinh/design-os-studio 
+zsh:6> work_dir=+zsh:6> mktemp -d
+zsh:6> work_dir=/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.aQTB2FRbhJ 
+zsh:7> trap 'rm -rf "$work_dir"' EXIT
+zsh:8> pulls_json=/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.aQTB2FRbhJ/commits-pulls.json 
+zsh:9> pull_json=/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.aQTB2FRbhJ/pull.json 
+zsh:10> files_json=/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.aQTB2FRbhJ/files.json 
+zsh:11> lying_artifact_json=/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.aQTB2FRbhJ/lying-artifact.json 
+zsh:13> gh api --paginate repos/jangtrinh/design-os-studio/commits/2d3012ca1ea064fd03638753139fe56e614ac07c/pulls
+zsh:14> candidate_pr=+zsh:15> python3 -B scripts/pr_labels.py --resolve-pr --pulls /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.aQTB2FRbhJ/commits-pulls.json --head-sha 2d3012ca1ea064fd03638753139fe56e614ac07c --head-repo jangtrinh/design-os-studio
+zsh:14> candidate_pr=1184 
+zsh:21> echo 'resolved_candidate_pr=1184'
resolved_candidate_pr=1184
+zsh:23> gh api repos/jangtrinh/design-os-studio/pulls/1184
+zsh:24> jq '{number, head_sha: .head.sha, head_repo: .head.repo.full_name, additions, deletions, changed_files}' /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.aQTB2FRbhJ/pull.json
{
  "number": 1184,
  "head_sha": "2d3012ca1ea064fd03638753139fe56e614ac07c",
  "head_repo": "jangtrinh/design-os-studio",
  "additions": 1387,
  "deletions": 0,
  "changed_files": 16
}
+zsh:25> gh api --paginate repos/jangtrinh/design-os-studio/pulls/1184/files
+zsh:27> python3 -B scripts/pr_labels.py --pulls /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.aQTB2FRbhJ/commits-pulls.json --pull /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.aQTB2FRbhJ/pull.json --files /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.aQTB2FRbhJ/files.json --head-sha 2d3012ca1ea064fd03638753139fe56e614ac07c --head-repo jangtrinh/design-os-studio
{"pr_number": 1184, "labels": ["size/L", "sensitive-path"]}
+zsh:34> jq -n --arg sha 2d3012ca1ea064fd03638753139fe56e614ac07c $'{\n    pr_number: 1,\n    head_sha: $sha,\n    additions: 1,\n    deletions: 0,\n    lines_changed: 1,\n    changed_files: 1,\n    paths: ["README.md"]\n  }'
+zsh:43> cat /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.aQTB2FRbhJ/lying-artifact.json
{
  "pr_number": 1,
  "head_sha": "2d3012ca1ea064fd03638753139fe56e614ac07c",
  "additions": 1,
  "deletions": 0,
  "lines_changed": 1,
  "changed_files": 1,
  "paths": [
    "README.md"
  ]
}
+zsh:45> python3 -B scripts/pr_labels.py --resolve-pr --pulls /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.aQTB2FRbhJ/commits-pulls.json --head-sha 2d3012ca1ea064fd03638753139fe56e614ac07c --head-repo jangtrinh/design-os-studio --artifact /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.aQTB2FRbhJ/lying-artifact.json
1184
+zsh:52> python3 -B scripts/pr_labels.py --pulls /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.aQTB2FRbhJ/commits-pulls.json --pull /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.aQTB2FRbhJ/pull.json --files /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.aQTB2FRbhJ/files.json --head-sha 2d3012ca1ea064fd03638753139fe56e614ac07c --head-repo jangtrinh/design-os-studio --artifact /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.aQTB2FRbhJ/lying-artifact.json
{"pr_number": 1184, "labels": ["size/L", "sensitive-path"]}
+zsh:1> rm -rf /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.aQTB2FRbhJ
```

Status: DONE
