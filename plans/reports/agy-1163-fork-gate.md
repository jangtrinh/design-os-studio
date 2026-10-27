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

---

## Round 3

### R3.1 What Could NOT Be Verified (Stated First)

1. **Automatic pre-merge `on: workflow_run` trigger on GitHub Actions:**
   - By GitHub Actions' platform rules, `on: workflow_run` workflows only register and auto-trigger after `.github/workflows/pr-labeller.yml` and `scripts/pr_labels.py` land on the default branch (`main`).
   - Verified pre-merge via a live read-only dry run (no POST) using the workflow's exact `gh api` and `scripts/pr_labels.py` commands against both real open fork PR [`#1175`](https://github.com/jangtrinh/design-os-studio/pull/1175) (`NamAIEngineer:fix/issue-1086-apply-spec`, `c53c55060d51327fc6a3d1f0f02a8157a6b10e6a`, `NamAIEngineer/design-os-studio`, with no artifact) and same-repo PR [`#1184`](https://github.com/jangtrinh/design-os-studio/pull/1184) (`jangtrinh:jangtrinh/agy-1163-fork-gate`, `4a7a05aec8a692add373f1298cfe44956c1f52fa`, `jangtrinh/design-os-studio`, with no artifact) (`plans/reports/agy-1163-fork-gate-evidence/r3-dry-run.log`).

### R3.2 Conclusion, Epistemic Ledger, Weakest Link & Assumptions

- **Conclusion:** `.github/workflows/pr-labeller.yml`, `scripts/pr_labels.py`, and `scripts/test_pr_labels.py` have been updated per all 5 controller rulings for Round 3 while preserving everything from Round 2 (`4a7a05ae`):
  1. **First lookup:** `gh api -X GET "repos/${GH_REPO}/pulls" -f state=open -f head="${HEAD_OWNER}:${HEAD_BRANCH}" --paginate`, passing `HEAD_OWNER` (`github.event.workflow_run.head_repository.owner.login`) and `HEAD_BRANCH` (`github.event.workflow_run.head_branch`) via `env:` (never `${{ }}` in `run:`) and letting `-f` URL-encode query parameters, keeping only PRs whose `head.sha` and `head.repo.full_name` match the `workflow_run`.
  2. **Second lookup:** `commits/${EXPECTED_HEAD_SHA}/pulls`, unchanged.
  3. **Third lookup:** the `pr-gate-result` artifact candidate (`<= 64 KiB`), unchanged, accepted only after `pulls/{n}` matches `head.sha` and `head.repo.full_name`.
  4. **No-match notice:** When none of the lookups finds a matching PR and there is no artifact, `.github/workflows/pr-labeller.yml` exits `0` with a `::notice::` naming the lookups that ran (`pulls?state=open&head=${HEAD_OWNER}:${HEAD_BRANCH}`, `commits/${EXPECTED_HEAD_SHA}/pulls`, and `pr-gate-result` artifact).
  5. **Preserved Round 2 invariants:** Counts and paths come exclusively from GitHub (`pulls/{n}` and `pulls/{n}/files`), truncated file lists (`len(paths) < changed_files`) receive both `size/L` and `sensitive-path`, the 64 KiB artifact cap holds, and the label allowlist (`size/L`, `sensitive-path`) is unchanged.
- **Epistemic Ledger:**
  - `FACT:` On Round 2 head `4a7a05ae`, the 3 new Round 3 unit tests in `scripts/test_pr_labels.py` failed as predicted (`Ran 15 tests in 0.094s — FAILED (errors=3)` in `plans/reports/agy-1163-fork-gate-evidence/r3-red.log`), and after updating `scripts/pr_labels.py`, all 15 tests (`12` Round 2 + `3` Round 3) passed (`Ran 15 tests in 0.126s — OK` in `plans/reports/agy-1163-fork-gate-evidence/r3-green.log`).
  - `FACT:` `$HOME/.local/bin/actionlint -verbose .github/workflows/pr-gate.yml .github/workflows/pr-labeller.yml` passed with `Found 0 errors in 2 files` (`plans/reports/agy-1163-fork-gate-evidence/r3-actionlint.log`).
  - `FACT:` In the live read-only dry run (`plans/reports/agy-1163-fork-gate-evidence/r3-dry-run.log`), for fork PR [`#1175`](https://github.com/jangtrinh/design-os-studio/pull/1175) (`HEAD_OWNER="NamAIEngineer"`, `HEAD_BRANCH="fix/issue-1086-apply-spec"`, `EXPECTED_HEAD_SHA="c53c55060d51327fc6a3d1f0f02a8157a6b10e6a"`, `EXPECTED_HEAD_REPO="NamAIEngineer/design-os-studio"`, no artifact), `commits/c53c5506.../pulls` returned `[]` (`length == 0`) while `gh api -X GET repos/jangtrinh/design-os-studio/pulls -f state=open -f head=NamAIEngineer:fix/issue-1086-apply-spec --paginate` resolved `1175`, fetched `pulls/1175` (`additions: 101`, `deletions: 2`, `changed_files: 1`) and `pulls/1175/files` (`crates/designos-app/src/main.rs`), and printed `{"pr_number": 1175, "labels": []}`.
  - `FACT:` In the same live read-only dry run (`plans/reports/agy-1163-fork-gate-evidence/r3-dry-run.log`), for same-repo PR [`#1184`](https://github.com/jangtrinh/design-os-studio/pull/1184) (`HEAD_OWNER="jangtrinh"`, `HEAD_BRANCH="jangtrinh/agy-1163-fork-gate"`, `EXPECTED_HEAD_SHA="4a7a05aec8a692add373f1298cfe44956c1f52fa"`, `EXPECTED_HEAD_REPO="jangtrinh/design-os-studio"`, no artifact), the head-label lookup resolved `1184` and printed `{"pr_number": 1184, "labels": ["size/L", "sensitive-path"]}`.
  - `INFERENCE:` Once PR [`#1184`](https://github.com/jangtrinh/design-os-studio/pull/1184) merges to `main`, an open fork PR that deletes or skips the artifact upload in `.github/workflows/pr-gate.yml` is still resolved by `pulls?state=open&head=<owner>:<branch>` (verified against `head.sha` and `head.repo.full_name`) and labelled from GitHub's own PR metadata and file list.
- **Weakest Link:** If a fork PR is already closed AND its commit is not indexed by `commits/{sha}/pulls` AND it skipped uploading the artifact before closing, no lookup resolves it; that is the intended Ruling 4 no-op case (`PR was closed or its branch moved on`).
- **Assumptions:** None.
- **Unresolved Questions:** None.

### R3.3 Round 3 Acceptance Table

| # | Acceptance Check | Evidence File | Result |
|---|---|---|---|
| **1a** | Red first: 3 new Round 3 unit tests fail on Round 2 head (`4a7a05ae`) | `plans/reports/agy-1163-fork-gate-evidence/r3-red.log` | **PASS (Red):** `Ran 15 tests in 0.094s — FAILED (errors=3)` |
| **1b** | Green: all 15 unit tests (`12` Round 2 + `3` Round 3: fork head-label with empty commit lookup & no artifact, mismatched `head.sha`/`head.repo` ignored, lookup order head-label -> commit -> artifact) pass | `plans/reports/agy-1163-fork-gate-evidence/r3-green.log` | **PASS (Green):** `Ran 15 tests in 0.126s — OK` |
| **2** | `actionlint` clean on `.github/workflows/pr-gate.yml` and `.github/workflows/pr-labeller.yml` | `plans/reports/agy-1163-fork-gate-evidence/r3-actionlint.log` | **PASS:** `Found 0 errors in 2 files` (exit `0`) |
| **3** | Live read-only dry run (no POST) for fork PR `#1175` (`NamAIEngineer:fix/issue-1086-apply-spec`, no artifact) and same-repo PR `#1184` (`jangtrinh:jangtrinh/agy-1163-fork-gate`, no artifact) | `plans/reports/agy-1163-fork-gate-evidence/r3-dry-run.log` | **PASS:** `#1175` resolves `1175` with `{"pr_number": 1175, "labels": []}`; `#1184` resolves `1184` with `{"pr_number": 1184, "labels": ["size/L", "sensitive-path"]}` |

### R3.4 Skills Loaded in Round 3 & What Changed Because of Them

1. **`es:dev` & `es:feature-dev`** (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`):
   - Enforced red-first TDD on `4a7a05ae` (`r3-red.log`) before editing `scripts/pr_labels.py`, preserved all 12 Round 2 tests unchanged, and restricted edits to the allowed file scope.
2. **`es:rust` & `es:rust-gpu-canvas`** (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`):
   - Loaded per owner instruction; no Rust crates were modified in Round 3.
3. **`es:astra-work` & `es:fable-thinking` & `es:orca-native-orchestration`** (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-orca-native-orchestration/SKILL.md`):
   - Applied `PREDICT:` before every consequential step, verified URL-safe query encoding via `gh api -X GET ... -f state=open -f head="${HEAD_OWNER}:${HEAD_BRANCH}" --paginate` with `HEAD_OWNER` and `HEAD_BRANCH` passed strictly via `env:`, and verified live read-only resolution against real fork PR `#1175` and same-repo PR `#1184`.

### R3.5 Pasted Commands & Verbatim Outputs (Round 3)

#### 1. Red-First Unit Tests on `4a7a05ae` (`plans/reports/agy-1163-fork-gate-evidence/r3-red.log`)

```text
$ python3 -B -m unittest scripts/test_pr_labels.py -v
test_10_files_adds_no_size_label_while_11_files_adds_size_l (scripts.test_pr_labels.PrLabelsTests.test_10_files_adds_no_size_label_while_11_files_adds_size_l) ... ok
test_400_lines_adds_no_size_label_while_401_lines_adds_size_l (scripts.test_pr_labels.PrLabelsTests.test_400_lines_adds_no_size_label_while_401_lines_adds_size_l) ... ok
test_candidate_number_with_different_head_repo_is_refused (scripts.test_pr_labels.PrLabelsTests.test_candidate_number_with_different_head_repo_is_refused) ... ok
test_candidate_number_with_different_head_sha_is_refused (scripts.test_pr_labels.PrLabelsTests.test_candidate_number_with_different_head_sha_is_refused) ... ok
test_files_list_shorter_than_changed_files_gives_both_labels (scripts.test_pr_labels.PrLabelsTests.test_files_list_shorter_than_changed_files_gives_both_labels) ... ok
test_head_label_resolves_fork_pr_when_commit_lookup_empty_and_no_artifact (scripts.test_pr_labels.PrLabelsTests.test_head_label_resolves_fork_pr_when_commit_lookup_empty_and_no_artifact) ... ERROR
test_head_label_with_different_head_sha_or_head_repo_is_not_used (scripts.test_pr_labels.PrLabelsTests.test_head_label_with_different_head_sha_or_head_repo_is_not_used) ... ERROR
test_labels_come_from_files_list_ignoring_conflicting_artifact (scripts.test_pr_labels.PrLabelsTests.test_labels_come_from_files_list_ignoring_conflicting_artifact) ... ok
test_lookup_order_head_label_then_commit_then_artifact (scripts.test_pr_labels.PrLabelsTests.test_lookup_order_head_label_then_commit_then_artifact) ... ERROR
test_no_pr_found_and_no_artifact_gives_no_labels_and_no_error (scripts.test_pr_labels.PrLabelsTests.test_no_pr_found_and_no_artifact_gives_no_labels_and_no_error) ... ok
test_oversized_artifact_over_64_kib_is_refused (scripts.test_pr_labels.PrLabelsTests.test_oversized_artifact_over_64_kib_is_refused) ... ok
test_pr_found_with_no_artifact_is_still_labelled (scripts.test_pr_labels.PrLabelsTests.test_pr_found_with_no_artifact_is_still_labelled) ... ok
test_rename_into_github_counts_as_one_file_and_is_sensitive (scripts.test_pr_labels.PrLabelsTests.test_rename_into_github_counts_as_one_file_and_is_sensitive) ... ok
test_rename_into_sensitive_path_triggers_sensitive_path_label (scripts.test_pr_labels.PrLabelsTests.test_rename_into_sensitive_path_triggers_sensitive_path_label) ... ok
test_sensitive_paths_match_all_configured_categories (scripts.test_pr_labels.PrLabelsTests.test_sensitive_paths_match_all_configured_categories) ... ok

======================================================================
ERROR: test_head_label_resolves_fork_pr_when_commit_lookup_empty_and_no_artifact (scripts.test_pr_labels.PrLabelsTests.test_head_label_resolves_fork_pr_when_commit_lookup_empty_and_no_artifact)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1163-fork-gate/scripts/test_pr_labels.py", line 273, in test_head_label_resolves_fork_pr_when_commit_lookup_empty_and_no_artifact
    pr_labels.evaluate_pr_labels(
    ~~~~~~~~~~~~~~~~~~~~~~~~~~~~^
        head_pulls=head_pulls,
        ^^^^^^^^^^^^^^^^^^^^^^
    ...<5 lines>...
        artifact=None,
        ^^^^^^^^^^^^^^
    ),
    ^
TypeError: evaluate_pr_labels() got an unexpected keyword argument 'head_pulls'

======================================================================
ERROR: test_head_label_with_different_head_sha_or_head_repo_is_not_used (scripts.test_pr_labels.PrLabelsTests.test_head_label_with_different_head_sha_or_head_repo_is_not_used)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1163-fork-gate/scripts/test_pr_labels.py", line 318, in test_head_label_with_different_head_sha_or_head_repo_is_not_used
    pr_labels.resolve_pr_number(
    ~~~~~~~~~~~~~~~~~~~~~~~~~~~^
        head_pulls=mismatched_head_pulls,
        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    ...<2 lines>...
        head_repo=fork_repo,
        ^^^^^^^^^^^^^^^^^^^^
    )
    ^
TypeError: resolve_pr_number() got an unexpected keyword argument 'head_pulls'

======================================================================
ERROR: test_lookup_order_head_label_then_commit_then_artifact (scripts.test_pr_labels.PrLabelsTests.test_lookup_order_head_label_then_commit_then_artifact)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1163-fork-gate/scripts/test_pr_labels.py", line 344, in test_lookup_order_head_label_then_commit_then_artifact
    pr_labels.resolve_pr_number(
    ~~~~~~~~~~~~~~~~~~~~~~~~~~~^
        head_pulls=head_pulls_match,
        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    ...<3 lines>...
        artifact=artifact_candidate,
        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    ),
    ^
TypeError: resolve_pr_number() got an unexpected keyword argument 'head_pulls'

----------------------------------------------------------------------
Ran 15 tests in 0.094s

FAILED (errors=3)
```

#### 2. Green Unit Tests (`plans/reports/agy-1163-fork-gate-evidence/r3-green.log`)

```text
$ python3 -B -m unittest scripts/test_pr_labels.py -v
test_10_files_adds_no_size_label_while_11_files_adds_size_l (scripts.test_pr_labels.PrLabelsTests.test_10_files_adds_no_size_label_while_11_files_adds_size_l) ... ok
test_400_lines_adds_no_size_label_while_401_lines_adds_size_l (scripts.test_pr_labels.PrLabelsTests.test_400_lines_adds_no_size_label_while_401_lines_adds_size_l) ... ok
test_candidate_number_with_different_head_repo_is_refused (scripts.test_pr_labels.PrLabelsTests.test_candidate_number_with_different_head_repo_is_refused) ... ok
test_candidate_number_with_different_head_sha_is_refused (scripts.test_pr_labels.PrLabelsTests.test_candidate_number_with_different_head_sha_is_refused) ... ok
test_files_list_shorter_than_changed_files_gives_both_labels (scripts.test_pr_labels.PrLabelsTests.test_files_list_shorter_than_changed_files_gives_both_labels) ... ok
test_head_label_resolves_fork_pr_when_commit_lookup_empty_and_no_artifact (scripts.test_pr_labels.PrLabelsTests.test_head_label_resolves_fork_pr_when_commit_lookup_empty_and_no_artifact) ... ok
test_head_label_with_different_head_sha_or_head_repo_is_not_used (scripts.test_pr_labels.PrLabelsTests.test_head_label_with_different_head_sha_or_head_repo_is_not_used) ... ok
test_labels_come_from_files_list_ignoring_conflicting_artifact (scripts.test_pr_labels.PrLabelsTests.test_labels_come_from_files_list_ignoring_conflicting_artifact) ... ok
test_lookup_order_head_label_then_commit_then_artifact (scripts.test_pr_labels.PrLabelsTests.test_lookup_order_head_label_then_commit_then_artifact) ... ok
test_no_pr_found_and_no_artifact_gives_no_labels_and_no_error (scripts.test_pr_labels.PrLabelsTests.test_no_pr_found_and_no_artifact_gives_no_labels_and_no_error) ... ok
test_oversized_artifact_over_64_kib_is_refused (scripts.test_pr_labels.PrLabelsTests.test_oversized_artifact_over_64_kib_is_refused) ... ok
test_pr_found_with_no_artifact_is_still_labelled (scripts.test_pr_labels.PrLabelsTests.test_pr_found_with_no_artifact_is_still_labelled) ... ok
test_rename_into_github_counts_as_one_file_and_is_sensitive (scripts.test_pr_labels.PrLabelsTests.test_rename_into_github_counts_as_one_file_and_is_sensitive) ... ok
test_rename_into_sensitive_path_triggers_sensitive_path_label (scripts.test_pr_labels.PrLabelsTests.test_rename_into_sensitive_path_triggers_sensitive_path_label) ... ok
test_sensitive_paths_match_all_configured_categories (scripts.test_pr_labels.PrLabelsTests.test_sensitive_paths_match_all_configured_categories) ... ok

----------------------------------------------------------------------
Ran 15 tests in 0.126s

OK
```

#### 3. `actionlint` on Both Workflows (`plans/reports/agy-1163-fork-gate-evidence/r3-actionlint.log`)

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
verbose: Rule "pyflakes" was disabled: exec: "pyflakes": executable file not found in $PATH
verbose: Found total 0 errors in 0 ms for .github/workflows/pr-labeller.yml
verbose: Found total 0 errors in 0 ms for .github/workflows/pr-gate.yml
verbose: Found 0 errors in 2 files
```

#### 4. Live Read-Only Dry Run Against Fork PR `#1175` and Same-Repo PR `#1184` (`plans/reports/agy-1163-fork-gate-evidence/r3-dry-run.log`)

```text
+ GH_REPO=jangtrinh/design-os-studio
++ mktemp -d
+ work_dir=/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.F8LwB0ii0n
+ trap 'rm -rf "$work_dir"' EXIT
+ HEAD_OWNER=NamAIEngineer
+ HEAD_BRANCH=fix/issue-1086-apply-spec
+ EXPECTED_HEAD_SHA=c53c55060d51327fc6a3d1f0f02a8157a6b10e6a
+ EXPECTED_HEAD_REPO=NamAIEngineer/design-os-studio
+ head_pulls_json=/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.F8LwB0ii0n/1175-head-pulls.json
+ pulls_json=/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.F8LwB0ii0n/1175-commits-pulls.json
+ pull_json=/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.F8LwB0ii0n/1175-pull.json
+ files_json=/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.F8LwB0ii0n/1175-files.json
+ gh api -X GET repos/jangtrinh/design-os-studio/pulls -f state=open -f head=NamAIEngineer:fix/issue-1086-apply-spec --paginate
+ gh api --paginate repos/jangtrinh/design-os-studio/commits/c53c55060d51327fc6a3d1f0f02a8157a6b10e6a/pulls
+ jq length /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.F8LwB0ii0n/1175-commits-pulls.json
0
++ python3 -B scripts/pr_labels.py --resolve-pr --head-pulls /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.F8LwB0ii0n/1175-head-pulls.json --pulls /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.F8LwB0ii0n/1175-commits-pulls.json --head-sha c53c55060d51327fc6a3d1f0f02a8157a6b10e6a --head-repo NamAIEngineer/design-os-studio
+ candidate_pr=1175
+ echo resolved_candidate_pr_1175=1175
resolved_candidate_pr_1175=1175
+ gh api repos/jangtrinh/design-os-studio/pulls/1175
+ gh api --paginate repos/jangtrinh/design-os-studio/pulls/1175/files
+ jq '{number, head_sha: .head.sha, head_repo: .head.repo.full_name, additions, deletions, changed_files}' /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.F8LwB0ii0n/1175-pull.json
{
  "number": 1175,
  "head_sha": "c53c55060d51327fc6a3d1f0f02a8157a6b10e6a",
  "head_repo": "NamAIEngineer/design-os-studio",
  "additions": 101,
  "deletions": 2,
  "changed_files": 1
}
+ jq 'map({filename, status, additions, deletions})' /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.F8LwB0ii0n/1175-files.json
[
  {
    "filename": "crates/designos-app/src/main.rs",
    "status": "modified",
    "additions": 101,
    "deletions": 2
  }
]
+ python3 -B scripts/pr_labels.py --head-pulls /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.F8LwB0ii0n/1175-head-pulls.json --pulls /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.F8LwB0ii0n/1175-commits-pulls.json --pull /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.F8LwB0ii0n/1175-pull.json --files /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.F8LwB0ii0n/1175-files.json --head-sha c53c55060d51327fc6a3d1f0f02a8157a6b10e6a --head-repo NamAIEngineer/design-os-studio
{"pr_number": 1175, "labels": []}
+ HEAD_OWNER=jangtrinh
+ HEAD_BRANCH=jangtrinh/agy-1163-fork-gate
++ gh pr view 1184 --json headRefOid --jq .headRefOid
+ EXPECTED_HEAD_SHA=4a7a05aec8a692add373f1298cfe44956c1f52fa
+ EXPECTED_HEAD_REPO=jangtrinh/design-os-studio
+ head_pulls_json=/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.F8LwB0ii0n/1184-head-pulls.json
+ pulls_json=/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.F8LwB0ii0n/1184-commits-pulls.json
+ pull_json=/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.F8LwB0ii0n/1184-pull.json
+ files_json=/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.F8LwB0ii0n/1184-files.json
+ gh api -X GET repos/jangtrinh/design-os-studio/pulls -f state=open -f head=jangtrinh:jangtrinh/agy-1163-fork-gate --paginate
+ printf '[]\n'
++ python3 -B scripts/pr_labels.py --resolve-pr --head-pulls /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.F8LwB0ii0n/1184-head-pulls.json --pulls /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.F8LwB0ii0n/1184-commits-pulls.json --head-sha 4a7a05aec8a692add373f1298cfe44956c1f52fa --head-repo jangtrinh/design-os-studio
+ candidate_pr=1184
+ echo resolved_candidate_pr_1184=1184
resolved_candidate_pr_1184=1184
+ gh api repos/jangtrinh/design-os-studio/pulls/1184
+ gh api --paginate repos/jangtrinh/design-os-studio/pulls/1184/files
+ jq '{number, head_sha: .head.sha, head_repo: .head.repo.full_name, additions, deletions, changed_files}' /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.F8LwB0ii0n/1184-pull.json
{
  "number": 1184,
  "head_sha": "4a7a05aec8a692add373f1298cfe44956c1f52fa",
  "head_repo": "jangtrinh/design-os-studio",
  "additions": 2211,
  "deletions": 0,
  "changed_files": 20
}
+ python3 -B scripts/pr_labels.py --head-pulls /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.F8LwB0ii0n/1184-head-pulls.json --pulls /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.F8LwB0ii0n/1184-commits-pulls.json --pull /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.F8LwB0ii0n/1184-pull.json --files /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.F8LwB0ii0n/1184-files.json --head-sha 4a7a05aec8a692add373f1298cfe44956c1f52fa --head-repo jangtrinh/design-os-studio
{"pr_number": 1184, "labels": ["size/L", "sensitive-path"]}
+ rm -rf /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.F8LwB0ii0n
```

---

## Round 4

### R4.1 What Could NOT Be Verified (Stated First)

1. **Automatic pre-merge `on: workflow_run` trigger on GitHub Actions:**
   - By GitHub Actions' platform rules, `on: workflow_run` workflows only register and fire automatically after `.github/workflows/pr-labeller.yml` and `scripts/pr_labels.py` land on the default branch (`main`).
   - Verified pre-merge via a live read-only dry run (no POST) using the workflow's exact commands against open fork PR [`#1175`](https://github.com/jangtrinh/design-os-studio/pull/1175) (`NamAIEngineer:fix/issue-1086-apply-spec`, `c53c55060d51327fc6a3d1f0f02a8157a6b10e6a`, `NamAIEngineer/design-os-studio`, no artifact) and same-repo PR [`#1184`](https://github.com/jangtrinh/design-os-studio/pull/1184) (`jangtrinh:jangtrinh/agy-1163-fork-gate`, `0713e169710fa083055de6fd0c3c114f4853e472`, `jangtrinh/design-os-studio`, no artifact) (`plans/reports/agy-1163-fork-gate-evidence/r4-dry-run.log`).

### R4.2 Conclusion, Epistemic Ledger, Weakest Link & Assumptions

- **Conclusion:** `.github/workflows/pr-labeller.yml`, `scripts/pr_labels.py`, and `scripts/test_pr_labels.py` have been updated per all 5 controller rulings for Round 4 while preserving everything from Round 3 (`0713e169`):
  1. **Collect every match:** `resolve_prs_from_commits_pulls` and `resolve_pr_numbers` collect every matching PR whose `head.sha` and `head.repo.full_name` match the `workflow_run`, and `.github/workflows/pr-labeller.yml` (as well as `evaluate_all_pr_labels` / `--pulls-dir`) fetches and validates `pulls/{n}` and `pulls/{n}/files` for each matched PR independently.
  2. **Fallback order stays:** When `pulls?state=open&head=<owner>:<branch>` has no match, every match from `commits/{sha}/pulls` is used; only when both are empty is the `pr-gate-result` artifact candidate used, validated against `pulls/{n}`.
  3. **One failure stops the job:** If any matched PR fails validation (or API fetch/label post), the workflow and `evaluate_all_pr_labels` continue trying the remaining matched PRs first so no valid PR is left unlabelled, and then exit `1`.
  4. **Cap the list at 20:** `MAX_MATCHING_PRS = 20`. At most 20 matching PRs are labelled; if more than 20 match, the first 20 are labelled and the job exits `1` with an error naming the total matched count.
  5. **Tests:** Added 6 new Round 4 tests covering two head-label matches with different diffs (`README.md` -> `[]`, `.github/` 500 lines -> `["size/L", "sensitive-path"]`), one matching + one non-matching head-label entry, multi-match fallback order (`commits/{sha}/pulls` -> artifact), one failed PR validation still labelling the rest before failing, the 20-PR cap naming the count, and `parse_paginated_arrays` merging `][` and `]\n[` in order while refusing trailing junk or non-array values.
- **Epistemic Ledger:**
  - `FACT:` On Round 3 head `0713e169`, running `python3 -B -m unittest scripts/test_pr_labels.py -v` failed on all 6 new Round 4 tests while passing all 15 existing tests (`Ran 21 tests in 0.128s — FAILED (errors=6)` in `plans/reports/agy-1163-fork-gate-evidence/r4-red.log`), including `ValueError: pull number 2000 does not match expected 2001` when evaluating the second matched PR.
  - `FACT:` After updating `scripts/pr_labels.py`, all 21 unit tests (`15` existing + `6` new Round 4) passed (`Ran 21 tests in 0.190s — OK` in `plans/reports/agy-1163-fork-gate-evidence/r4-green.log`).
  - `FACT:` `$HOME/.local/bin/actionlint -verbose .github/workflows/pr-gate.yml .github/workflows/pr-labeller.yml` passed with `Found 0 errors in 2 files` (`plans/reports/agy-1163-fork-gate-evidence/r4-actionlint.log`).
  - `FACT:` In the live read-only dry run (`plans/reports/agy-1163-fork-gate-evidence/r4-dry-run.log`), fork PR [`#1175`](https://github.com/jangtrinh/design-os-studio/pull/1175) resolved `1175` with `{"pr_number": 1175, "labels": []}` (`{"results": [{"pr_number": 1175, "labels": []}], "errors": []}`), and same-repo PR [`#1184`](https://github.com/jangtrinh/design-os-studio/pull/1184) resolved `1184` with `{"pr_number": 1184, "labels": ["size/L", "sensitive-path"]}` (`{"results": [{"pr_number": 1184, "labels": ["size/L", "sensitive-path"]}], "errors": []}`).
  - `INFERENCE:` Once PR [`#1184`](https://github.com/jangtrinh/design-os-studio/pull/1184) merges to `main`, opening a second decoy PR from the same `owner:branch` into another base branch cannot shadow or suppress `size/L` or `sensitive-path` labels on the PR targeting `main`.
- **Weakest Link:** If a single head has more than 20 open PRs simultaneously, only the first 20 are labelled before the job exits `1` (by design per Ruling 4).
- **Assumptions:** None.
- **Unresolved Questions:** None.

### R4.3 Round 4 Acceptance Table

| # | Acceptance Check | Evidence File | Result |
|---|---|---|---|
| **1a** | Red first: 6 new Round 4 unit tests fail on Round 3 head (`0713e169`) while all 15 Round 2/3 tests pass | `plans/reports/agy-1163-fork-gate-evidence/r4-red.log` | **PASS (Red):** `Ran 21 tests in 0.128s — FAILED (errors=6)` |
| **1b** | Green: all 21 unit tests (`15` Round 2/3 + `6` Round 4) pass | `plans/reports/agy-1163-fork-gate-evidence/r4-green.log` | **PASS (Green):** `Ran 21 tests in 0.190s — OK` |
| **2** | `actionlint` clean on `.github/workflows/pr-gate.yml` and `.github/workflows/pr-labeller.yml` | `plans/reports/agy-1163-fork-gate-evidence/r4-actionlint.log` | **PASS:** `Found 0 errors in 2 files` (exit `0`) |
| **3** | Live read-only dry run (no POST) using the workflow's commands against fork PR `#1175` (no artifact) and same-repo PR `#1184` (no artifact) | `plans/reports/agy-1163-fork-gate-evidence/r4-dry-run.log` | **PASS:** `#1175` resolves `1175` -> `{"pr_number": 1175, "labels": []}`; `#1184` resolves `1184` -> `{"pr_number": 1184, "labels": ["size/L", "sensitive-path"]}` |

### R4.4 Skills Loaded in Round 4 & What Changed Because of Them

1. **`es:dev` & `es:feature-dev`** (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`):
   - Enforced red-first TDD on `0713e169` (`r4-red.log`) before editing `scripts/pr_labels.py`, preserved all 15 existing tests, and kept changes strictly within the allowed file scope.
2. **`es:rust` & `es:rust-gpu-canvas`** (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`):
   - Loaded per owner instruction; no Rust crates were modified in Round 4.
3. **`es:astra-work` & `es:fable-thinking`** (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`):
   - Guided adversarial verification of `evaluate_pr_labels` and `evaluate_all_pr_labels` (catching and removing a fallback that could have inspected `pulls_by_number` when `resolve_pr_numbers` returned `[]`, and tightening `parse_paginated_arrays` to reject empty/whitespace-only inputs as well as trailing junk and non-array JSON values).

### R4.5 Pasted Commands & Verbatim Outputs (Round 4)

#### 1. Red-First Unit Tests on `0713e169` (`plans/reports/agy-1163-fork-gate-evidence/r4-red.log`)

```text
$ python3 -B -m unittest scripts/test_pr_labels.py -v
test_10_files_adds_no_size_label_while_11_files_adds_size_l (scripts.test_pr_labels.PrLabelsTests.test_10_files_adds_no_size_label_while_11_files_adds_size_l) ... ok
test_400_lines_adds_no_size_label_while_401_lines_adds_size_l (scripts.test_pr_labels.PrLabelsTests.test_400_lines_adds_no_size_label_while_401_lines_adds_size_l) ... ok
test_candidate_number_with_different_head_repo_is_refused (scripts.test_pr_labels.PrLabelsTests.test_candidate_number_with_different_head_repo_is_refused) ... ok
test_candidate_number_with_different_head_sha_is_refused (scripts.test_pr_labels.PrLabelsTests.test_candidate_number_with_different_head_sha_is_refused) ... ok
test_cap_at_20_matching_prs_labels_first_20_and_fails_naming_count (scripts.test_pr_labels.PrLabelsTests.test_cap_at_20_matching_prs_labels_first_20_and_fails_naming_count) ... ERROR
test_fallback_uses_all_matches_from_commits_pulls_then_artifact (scripts.test_pr_labels.PrLabelsTests.test_fallback_uses_all_matches_from_commits_pulls_then_artifact) ... ERROR
test_files_list_shorter_than_changed_files_gives_both_labels (scripts.test_pr_labels.PrLabelsTests.test_files_list_shorter_than_changed_files_gives_both_labels) ... ok
test_head_label_list_with_one_matching_and_one_non_matching_labels_only_matching (scripts.test_pr_labels.PrLabelsTests.test_head_label_list_with_one_matching_and_one_non_matching_labels_only_matching) ... ERROR
test_head_label_resolves_fork_pr_when_commit_lookup_empty_and_no_artifact (scripts.test_pr_labels.PrLabelsTests.test_head_label_resolves_fork_pr_when_commit_lookup_empty_and_no_artifact) ... ok
test_head_label_with_different_head_sha_or_head_repo_is_not_used (scripts.test_pr_labels.PrLabelsTests.test_head_label_with_different_head_sha_or_head_repo_is_not_used) ... ok
test_labels_come_from_files_list_ignoring_conflicting_artifact (scripts.test_pr_labels.PrLabelsTests.test_labels_come_from_files_list_ignoring_conflicting_artifact) ... ok
test_lookup_order_head_label_then_commit_then_artifact (scripts.test_pr_labels.PrLabelsTests.test_lookup_order_head_label_then_commit_then_artifact) ... ok
test_no_pr_found_and_no_artifact_gives_no_labels_and_no_error (scripts.test_pr_labels.PrLabelsTests.test_no_pr_found_and_no_artifact_gives_no_labels_and_no_error) ... ok
test_one_failed_pr_validation_still_labels_rest_and_fails_job (scripts.test_pr_labels.PrLabelsTests.test_one_failed_pr_validation_still_labels_rest_and_fails_job) ... ERROR
test_oversized_artifact_over_64_kib_is_refused (scripts.test_pr_labels.PrLabelsTests.test_oversized_artifact_over_64_kib_is_refused) ... ok
test_paginated_parser_merges_adjacent_and_newline_arrays_and_refuses_junk_or_non_array (scripts.test_pr_labels.PrLabelsTests.test_paginated_parser_merges_adjacent_and_newline_arrays_and_refuses_junk_or_non_array) ... ERROR
test_pr_found_with_no_artifact_is_still_labelled (scripts.test_pr_labels.PrLabelsTests.test_pr_found_with_no_artifact_is_still_labelled) ... ok
test_rename_into_github_counts_as_one_file_and_is_sensitive (scripts.test_pr_labels.PrLabelsTests.test_rename_into_github_counts_as_one_file_and_is_sensitive) ... ok
test_rename_into_sensitive_path_triggers_sensitive_path_label (scripts.test_pr_labels.PrLabelsTests.test_rename_into_sensitive_path_triggers_sensitive_path_label) ... ok
test_sensitive_paths_match_all_configured_categories (scripts.test_pr_labels.PrLabelsTests.test_sensitive_paths_match_all_configured_categories) ... ok
test_two_head_label_matches_with_different_diffs_label_each_from_own_diff (scripts.test_pr_labels.PrLabelsTests.test_two_head_label_matches_with_different_diffs_label_each_from_own_diff) ... ERROR

======================================================================
ERROR: test_cap_at_20_matching_prs_labels_first_20_and_fails_naming_count (scripts.test_pr_labels.PrLabelsTests.test_cap_at_20_matching_prs_labels_first_20_and_fails_naming_count)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1163-fork-gate/scripts/test_pr_labels.py", line 635, in test_cap_at_20_matching_prs_labels_first_20_and_fails_naming_count
    results, errors = pr_labels.evaluate_all_pr_labels(
                      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
AttributeError: module 'pr_labels' has no attribute 'evaluate_all_pr_labels'. Did you mean: 'evaluate_pr_labels'?

======================================================================
ERROR: test_fallback_uses_all_matches_from_commits_pulls_then_artifact (scripts.test_pr_labels.PrLabelsTests.test_fallback_uses_all_matches_from_commits_pulls_then_artifact)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1163-fork-gate/scripts/test_pr_labels.py", line 573, in test_fallback_uses_all_matches_from_commits_pulls_then_artifact
    pr_labels.resolve_pr_numbers(
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
AttributeError: module 'pr_labels' has no attribute 'resolve_pr_numbers'. Did you mean: 'resolve_pr_number'?

======================================================================
ERROR: test_head_label_list_with_one_matching_and_one_non_matching_labels_only_matching (scripts.test_pr_labels.PrLabelsTests.test_head_label_list_with_one_matching_and_one_non_matching_labels_only_matching)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1163-fork-gate/scripts/test_pr_labels.py", line 547, in test_head_label_list_with_one_matching_and_one_non_matching_labels_only_matching
    pr_labels.resolve_pr_numbers(
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
AttributeError: module 'pr_labels' has no attribute 'resolve_pr_numbers'. Did you mean: 'resolve_pr_number'?

======================================================================
ERROR: test_one_failed_pr_validation_still_labels_rest_and_fails_job (scripts.test_pr_labels.PrLabelsTests.test_one_failed_pr_validation_still_labels_rest_and_fails_job)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1163-fork-gate/scripts/test_pr_labels.py", line 610, in test_one_failed_pr_validation_still_labels_rest_and_fails_job
    results, errors = pr_labels.evaluate_all_pr_labels(
                      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
AttributeError: module 'pr_labels' has no attribute 'evaluate_all_pr_labels'. Did you mean: 'evaluate_pr_labels'?

======================================================================
ERROR: test_paginated_parser_merges_adjacent_and_newline_arrays_and_refuses_junk_or_non_array (scripts.test_pr_labels.PrLabelsTests.test_paginated_parser_merges_adjacent_and_newline_arrays_and_refuses_junk_or_non_array)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1163-fork-gate/scripts/test_pr_labels.py", line 650, in test_paginated_parser_merges_adjacent_and_newline_arrays_and_refuses_junk_or_non_array
    pr_labels.parse_paginated_arrays('[{"id": 1}][{"id": 2}, {"id": 3}]'),
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
AttributeError: module 'pr_labels' has no attribute 'parse_paginated_arrays'

======================================================================
ERROR: test_two_head_label_matches_with_different_diffs_label_each_from_own_diff (scripts.test_pr_labels.PrLabelsTests.test_two_head_label_matches_with_different_diffs_label_each_from_own_diff)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1163-fork-gate/scripts/test_pr_labels.py", line 446, in test_two_head_label_matches_with_different_diffs_label_each_from_own_diff
    pr_labels.evaluate_pr_labels(
    ~~~~~~~~~~~~~~~~~~~~~~~~~~~~^
        head_pulls=head_pulls,
        ^^^^^^^^^^^^^^^^^^^^^^
    ...<4 lines>...
        head_repo=fork_repo,
        ^^^^^^^^^^^^^^^^^^^^
    ),
    ^
  File "/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1163-fork-gate/scripts/pr_labels.py", line 219, in evaluate_pr_labels
    v = validate_pull(
        pull, expected_pr_number=pr_number, expected_head_sha=expected_sha, expected_head_repo=expected_repo,
    )
  File "/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1163-fork-gate/scripts/pr_labels.py", line 130, in validate_pull
    raise ValueError(f"pull number {num!r} does not match expected {expected_pr_number}")
ValueError: pull number 2000 does not match expected 2001

----------------------------------------------------------------------
Ran 21 tests in 0.128s

FAILED (errors=6)
```

#### 2. Green Unit Tests (`plans/reports/agy-1163-fork-gate-evidence/r4-green.log`)

```text
$ python3 -B -m unittest scripts/test_pr_labels.py -v
test_10_files_adds_no_size_label_while_11_files_adds_size_l (scripts.test_pr_labels.PrLabelsTests.test_10_files_adds_no_size_label_while_11_files_adds_size_l) ... ok
test_400_lines_adds_no_size_label_while_401_lines_adds_size_l (scripts.test_pr_labels.PrLabelsTests.test_400_lines_adds_no_size_label_while_401_lines_adds_size_l) ... ok
test_candidate_number_with_different_head_repo_is_refused (scripts.test_pr_labels.PrLabelsTests.test_candidate_number_with_different_head_repo_is_refused) ... ok
test_candidate_number_with_different_head_sha_is_refused (scripts.test_pr_labels.PrLabelsTests.test_candidate_number_with_different_head_sha_is_refused) ... ok
test_cap_at_20_matching_prs_labels_first_20_and_fails_naming_count (scripts.test_pr_labels.PrLabelsTests.test_cap_at_20_matching_prs_labels_first_20_and_fails_naming_count) ... ok
test_fallback_uses_all_matches_from_commits_pulls_then_artifact (scripts.test_pr_labels.PrLabelsTests.test_fallback_uses_all_matches_from_commits_pulls_then_artifact) ... ok
test_files_list_shorter_than_changed_files_gives_both_labels (scripts.test_pr_labels.PrLabelsTests.test_files_list_shorter_than_changed_files_gives_both_labels) ... ok
test_head_label_list_with_one_matching_and_one_non_matching_labels_only_matching (scripts.test_pr_labels.PrLabelsTests.test_head_label_list_with_one_matching_and_one_non_matching_labels_only_matching) ... ok
test_head_label_resolves_fork_pr_when_commit_lookup_empty_and_no_artifact (scripts.test_pr_labels.PrLabelsTests.test_head_label_resolves_fork_pr_when_commit_lookup_empty_and_no_artifact) ... ok
test_head_label_with_different_head_sha_or_head_repo_is_not_used (scripts.test_pr_labels.PrLabelsTests.test_head_label_with_different_head_sha_or_head_repo_is_not_used) ... ok
test_labels_come_from_files_list_ignoring_conflicting_artifact (scripts.test_pr_labels.PrLabelsTests.test_labels_come_from_files_list_ignoring_conflicting_artifact) ... ok
test_lookup_order_head_label_then_commit_then_artifact (scripts.test_pr_labels.PrLabelsTests.test_lookup_order_head_label_then_commit_then_artifact) ... ok
test_no_pr_found_and_no_artifact_gives_no_labels_and_no_error (scripts.test_pr_labels.PrLabelsTests.test_no_pr_found_and_no_artifact_gives_no_labels_and_no_error) ... ok
test_one_failed_pr_validation_still_labels_rest_and_fails_job (scripts.test_pr_labels.PrLabelsTests.test_one_failed_pr_validation_still_labels_rest_and_fails_job) ... ok
test_oversized_artifact_over_64_kib_is_refused (scripts.test_pr_labels.PrLabelsTests.test_oversized_artifact_over_64_kib_is_refused) ... ok
test_paginated_parser_merges_adjacent_and_newline_arrays_and_refuses_junk_or_non_array (scripts.test_pr_labels.PrLabelsTests.test_paginated_parser_merges_adjacent_and_newline_arrays_and_refuses_junk_or_non_array) ... ok
test_pr_found_with_no_artifact_is_still_labelled (scripts.test_pr_labels.PrLabelsTests.test_pr_found_with_no_artifact_is_still_labelled) ... ok
test_rename_into_github_counts_as_one_file_and_is_sensitive (scripts.test_pr_labels.PrLabelsTests.test_rename_into_github_counts_as_one_file_and_is_sensitive) ... ok
test_rename_into_sensitive_path_triggers_sensitive_path_label (scripts.test_pr_labels.PrLabelsTests.test_rename_into_sensitive_path_triggers_sensitive_path_label) ... ok
test_sensitive_paths_match_all_configured_categories (scripts.test_pr_labels.PrLabelsTests.test_sensitive_paths_match_all_configured_categories) ... ok
test_two_head_label_matches_with_different_diffs_label_each_from_own_diff (scripts.test_pr_labels.PrLabelsTests.test_two_head_label_matches_with_different_diffs_label_each_from_own_diff) ... ok

----------------------------------------------------------------------
Ran 21 tests in 0.190s

OK
```

#### 3. `actionlint` on Both Workflows (`plans/reports/agy-1163-fork-gate-evidence/r4-actionlint.log`)

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
verbose: Rule "pyflakes" was disabled: exec: "pyflakes": executable file not found in $PATH
verbose: Found total 0 errors in 0 ms for .github/workflows/pr-labeller.yml
verbose: Found total 0 errors in 0 ms for .github/workflows/pr-gate.yml
verbose: Found 0 errors in 2 files
```

#### 4. Live Read-Only Dry Run Against Fork PR `#1175` and Same-Repo PR `#1184` (`plans/reports/agy-1163-fork-gate-evidence/r4-dry-run.log`)

```text
+ GH_REPO=jangtrinh/design-os-studio
++ mktemp -d
+ work_dir=/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU
+ trap 'rm -rf "/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU"' EXIT
++ gh pr view 1175 --json headRefOid --jq .headRefOid
+ SHA_1175=c53c55060d51327fc6a3d1f0f02a8157a6b10e6a
+ run_dry_for_head 1175 NamAIEngineer fix/issue-1086-apply-spec c53c55060d51327fc6a3d1f0f02a8157a6b10e6a NamAIEngineer/design-os-studio
+ local tag=1175
+ local HEAD_OWNER=NamAIEngineer
+ local HEAD_BRANCH=fix/issue-1086-apply-spec
+ local EXPECTED_HEAD_SHA=c53c55060d51327fc6a3d1f0f02a8157a6b10e6a
+ local EXPECTED_HEAD_REPO=NamAIEngineer/design-os-studio
+ local sub_dir=/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1175
+ mkdir -p /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1175
+ local head_pulls_json=/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1175/head-pulls.json
+ local pulls_json=/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1175/commits-pulls.json
+ gh api -X GET repos/jangtrinh/design-os-studio/pulls -f state=open -f head=NamAIEngineer:fix/issue-1086-apply-spec --paginate
+ printf '[]\n'
++ python3 -B scripts/pr_labels.py --resolve-pr --head-pulls /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1175/head-pulls.json --pulls /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1175/commits-pulls.json --head-sha c53c55060d51327fc6a3d1f0f02a8157a6b10e6a --head-repo NamAIEngineer/design-os-studio
+ candidate_prs=1175
+ '[' -z 1175 ']'
+ matched_prs=()
+ IFS=
+ read -r candidate_pr
+ '[' -z 1175 ']'
+ matched_prs+=("$candidate_pr")
+ IFS=
+ read -r candidate_pr
+ max_prs=20
+ prs_to_label=("${matched_prs[@]:0:$max_prs}")
+ for candidate_pr in '"${prs_to_label[@]}"'
+ pull_json=/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1175/pull-1175.json
+ files_json=/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1175/files-1175.json
+ gh api repos/jangtrinh/design-os-studio/pulls/1175
+ gh api --paginate repos/jangtrinh/design-os-studio/pulls/1175/files
+ python3 -B scripts/pr_labels.py --head-pulls /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1175/head-pulls.json --pulls /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1175/commits-pulls.json --pr-number 1175 --pull /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1175/pull-1175.json --files /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1175/files-1175.json --head-sha c53c55060d51327fc6a3d1f0f02a8157a6b10e6a --head-repo NamAIEngineer/design-os-studio
{"pr_number": 1175, "labels": []}
+ python3 -B scripts/pr_labels.py --head-pulls /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1175/head-pulls.json --pulls /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1175/commits-pulls.json --pulls-dir /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1175 --head-sha c53c55060d51327fc6a3d1f0f02a8157a6b10e6a --head-repo NamAIEngineer/design-os-studio
{"results": [{"pr_number": 1175, "labels": []}], "errors": []}
++ gh pr view 1184 --json headRefOid --jq .headRefOid
+ SHA_1184=0713e169710fa083055de6fd0c3c114f4853e472
+ run_dry_for_head 1184 jangtrinh jangtrinh/agy-1163-fork-gate 0713e169710fa083055de6fd0c3c114f4853e472 jangtrinh/design-os-studio
+ local tag=1184
+ local HEAD_OWNER=jangtrinh
+ local HEAD_BRANCH=jangtrinh/agy-1163-fork-gate
+ local EXPECTED_HEAD_SHA=0713e169710fa083055de6fd0c3c114f4853e472
+ local EXPECTED_HEAD_REPO=jangtrinh/design-os-studio
+ local sub_dir=/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1184
+ mkdir -p /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1184
+ local head_pulls_json=/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1184/head-pulls.json
+ local pulls_json=/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1184/commits-pulls.json
+ gh api -X GET repos/jangtrinh/design-os-studio/pulls -f state=open -f head=jangtrinh:jangtrinh/agy-1163-fork-gate --paginate
+ printf '[]\n'
++ python3 -B scripts/pr_labels.py --resolve-pr --head-pulls /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1184/head-pulls.json --pulls /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1184/commits-pulls.json --head-sha 0713e169710fa083055de6fd0c3c114f4853e472 --head-repo jangtrinh/design-os-studio
+ candidate_prs=1184
+ '[' -z 1184 ']'
+ matched_prs=()
+ IFS=
+ read -r candidate_pr
+ '[' -z 1184 ']'
+ matched_prs+=("$candidate_pr")
+ IFS=
+ read -r candidate_pr
+ max_prs=20
+ prs_to_label=("${matched_prs[@]:0:$max_prs}")
+ for candidate_pr in '"${prs_to_label[@]}"'
+ pull_json=/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1184/pull-1184.json
+ files_json=/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1184/files-1184.json
+ gh api repos/jangtrinh/design-os-studio/pulls/1184
+ gh api --paginate repos/jangtrinh/design-os-studio/pulls/1184/files
+ python3 -B scripts/pr_labels.py --head-pulls /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1184/head-pulls.json --pulls /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1184/commits-pulls.json --pr-number 1184 --pull /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1184/pull-1184.json --files /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1184/files-1184.json --head-sha 0713e169710fa083055de6fd0c3c114f4853e472 --head-repo jangtrinh/design-os-studio
{"pr_number": 1184, "labels": ["size/L", "sensitive-path"]}
+ python3 -B scripts/pr_labels.py --head-pulls /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1184/head-pulls.json --pulls /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1184/commits-pulls.json --pulls-dir /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU/1184 --head-sha 0713e169710fa083055de6fd0c3c114f4853e472 --head-repo jangtrinh/design-os-studio
{"results": [{"pr_number": 1184, "labels": ["size/L", "sensitive-path"]}], "errors": []}
+ rm -rf /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.MpiMYfxFUU
```

Status: DONE
