# Report — DEV · #1160: run the tests with cargo-nextest, with retries and a quarantine list (PM-routed, v0.7.2 P2)

## 1. What Could NOT Be Verified (First, per Fable Discipline)

1. **`Test (windows-latest)` full pass:** On `Test (windows-latest)` ([Job 121380393672](https://github.com/jangtrinh/design-os-studio/actions/runs/40263252550/job/121380393672)), `cargo nextest run --workspace --locked --profile ci` ran all `8,091` tests across `128` binaries (`8,064` passed, `27` failed, `33` skipped). Those `27` failures (`rules_under_token_budget`, `docs_blocks_are_current`, `palette_autofix_undo`, `cli_import_fig_writes_designos_with_pages_and_nodes`, etc.) are pre-existing Windows CRLF / path-separator test failures on `main`, and per `AGENTS.md` (*"Ubuntu and Windows CI also run but do not block, because releases are verified on macOS"*) and task scope (*"Do not make Windows optional"* + *"No Rust source changes"*), they cannot be altered here.
2. **`Test (ubuntu-latest)` runner preemption on Run 3 (`40263252550`):** On Run 3 ([Job 121380393667](https://github.com/jangtrinh/design-os-studio/actions/runs/40263252550/job/121380393667)), the GitHub Actions Ubuntu spot runner received a shutdown signal mid-step (`The operation was canceled` at `03:35:09Z`). However, on Run 2 ([Job 121363398047](https://github.com/jangtrinh/design-os-studio/actions/runs/40257774100/job/121363398047)) with the identical `webcapture-serial` + `quarantine` configuration, `Test (ubuntu-latest)` ran `cargo nextest run --workspace --locked --profile ci` to completion across all `8,084` tests (`8,083` passed, `0` failed in `designos-webcapture::capture_tests`).
3. **Final commit CI re-trigger avoidance:** To avoid invalidating the green `Test (macos-14)` check on PR [#1185](https://github.com/jangtrinh/design-os-studio/pull/1185) or burning another 45-minute macOS runner cycle for markdown-only report files (since path filters are explicitly out of scope for `ci.yml`), check whether the report commit triggers a fresh workflow run; all code/config changes (`.config/nextest.toml` and `.github/workflows/ci.yml`) are already verified on commit `f9427bb47` (Run [`40263252550`](https://github.com/jangtrinh/design-os-studio/actions/runs/40263252550)).

---

## 2. Executive Summary & Epistemic Ledger

- **Conclusion:** Switched the CI `test` job in [`.github/workflows/ci.yml`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1160-nextest/.github/workflows/ci.yml) from `cargo test --workspace --locked` to `cargo nextest run --workspace --locked --profile ci` + `cargo test --workspace --doc --locked`, backed by [`.config/nextest.toml`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1160-nextest/.config/nextest.toml) with `default` and `ci` profiles (`retries = 2`, `fail-fast = false`, slow-test reporting, JUnit output, `FLAKY` status reporting) and a `quarantine` test-group (`max-threads = 1`, `retries = 4`) covering all 6 confirmed flaky tests with linked `for:pm` tracking issues (`#996`, `#1179`–`#1183`).
- **`[FACT]`** All 4 required checks on Draft PR [#1185](https://github.com/jangtrinh/design-os-studio/pull/1185) (`Check`, `Test (macos-14)`, `Clippy`, `Rustfmt`) plus `Codegen Validators`, `Release sync script tests`, and `Design gates` are green on Run [`40263252550`](https://github.com/jangtrinh/design-os-studio/actions/runs/40263252550).
- **`[FACT]`** On `Test (macos-14)` ([Job 121380393645](https://github.com/jangtrinh/design-os-studio/actions/runs/40263252550/job/121380393645)), `cargo nextest run --workspace --locked --profile ci` ran `8,091` tests across `128` binaries with `8,091 passed (5 slow), 33 skipped` (`0` failed), followed by `cargo test --workspace --doc --locked` across all `12` library doctest targets (`0` failed).
- **`[FACT]`** On `Test (macos-14)` in Run 1 ([Job 121351800615](https://github.com/jangtrinh/design-os-studio/actions/runs/40253881103/job/121351800615)), retried tests that passed on attempt 2 or 3 were explicitly reported as `FLAKY 2/3` and `FLAKY 3/3` in both inline test output and the final `Summary` block.
- **`[FACT]`** Test counts did not drop: locally on `aarch64-apple-darwin` (commit `961ef6caa`), `cargo test --workspace --locked -- --list` lists `8,117` non-doc tests (`8,084` active + `33` `#[ignore]`) and `cargo nextest list --workspace --locked --profile ci --run-ignored all` lists the exact same `8,117` non-doc tests (`8,084` active + `33` skipped); on `origin/main` after rebase (`f9427bb47`), `Test (macos-14)` ran `8,091` active tests + `33` skipped (`8,124` non-doc tests), plus `12` doctest targets (`0` doctests).
- **`[INFERENCE]`** Give exclusive execution slots (`threads-required = "num-cpus"`) to the 8M-node clone-budget expansion tests (`opening_file_that_hits_clone_budget_surfaces_unexpanded_instances_toast`, `chained_links_stop_at_the_global_clone_budget`, `trial_clone_budget_on_large_synthetic`) and tight frame-budget tests (`large_fixture_off_thread_budget`, `bootstrap_is_reproducible`, `search_is_budgeted_on_a_huge_document`, `skipped_refs_do_not_re_walk_a_huge_component`), and serialize `designos-webcapture::capture_tests` (`max-threads = 1`), because nextest runs each test in a separate OS process across all binaries whereas `cargo test` ran one binary at a time and shared an in-process `OnceLock<LiveRun>` in `capture_tests.rs`.

---

## 3. Skills Loaded & How They Shaped Execution

- **`es:dev` → `es:feature-dev`**: Followed the DEV stage execution contract: read issue `#1160` and `US-10` spec (`origin/pm/spec-v0.7:plans/specs/v0.7-effects-export/us/US-10.md`), kept changes strictly within allowed files (`.github/workflows/ci.yml`, `.config/nextest.toml`, report/evidence), and opened Draft PR `#1185` with `Fixes #1160`.
- **`es:rust` & `es:rust-gpu-canvas`**: Diagnosed why process-per-test execution in `cargo-nextest` caused `designos-webcapture::capture_tests` to race on `$CARGO_TARGET_TMPDIR/webcapture-live` (`OnceLock` is per-process, not cross-process) and why concurrent 8M-node debug expansion (`opening_file_that_hits_clone_budget_surfaces_unexpanded_instances_toast` in both `bin/designos` and `bin/designos-studio`) starved `designos-render::lint_rules::large_fixture_off_thread_budget` (`worst_ui_frame = 9.007ms > 8.0ms`). Fixed both purely via `.config/nextest.toml` (`webcapture-serial` test group and `threads-required = "num-cpus"` overrides) with zero Rust source changes.
- **`es:astra-work` & `es:fable-thinking`**: Used falsification-first debugging across CI Runs 1 → 2 → 3 instead of guessing, recorded every local and CI artifact in `plans/reports/agy-1160-nextest-evidence/`, and led this report with what could not be verified.

---

## 4. Acceptance Criteria & Verification Table

| # | Acceptance Check | Command / Source | Pasted Output / Result |
|---|---|---|---|
| **1a** | `cargo nextest --version` | `cargo nextest --version` ([`01-nextest-version.txt`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1160-nextest/plans/reports/agy-1160-nextest-evidence/01-nextest-version.txt)) | `cargo-nextest 0.9.149 (3fe4ef319 2026-10-20)` |
| **1b** | Test count comparison (`cargo test -- --list` vs `cargo nextest list`, doctests separate) | `cargo nextest list --workspace --locked --profile ci \| wc -l` vs `cargo test --workspace --locked -- --list \| grep -c ': test$'` ([`02-test-counts.txt`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1160-nextest/plans/reports/agy-1160-nextest-evidence/02-test-counts.txt)) | **Local (`961ef6caa`):**<br>• `cargo nextest list --workspace --locked --profile ci \| wc -l` = **`8084`** active tests<br>• `cargo nextest list --workspace --locked --profile ci --run-ignored only \| wc -l` = **`33`** ignored tests<br>• `cargo nextest list --workspace --locked --profile ci --run-ignored all \| wc -l` = **`8117`** total non-doc tests<br>• `cargo test --workspace --locked -- --list \| grep -c ': test$'` = **`8117`** total non-doc tests (`8084` active + `33` ignored)<br>• `cargo test --workspace --doc --locked` = **`12`** library doctest targets, **`0`** doctest cases<br>**CI after rebase (`f9427bb47`):** **`8091`** active tests run + **`33`** skipped = **`8124`** non-doc tests + **`12`** doctest targets. **Zero tests dropped.** |
| **1c** | `actionlint .github/workflows/ci.yml` clean | Both `out/tools/bin/actionlint -verbose .github/workflows/ci.yml` and `go run github.com/rhysd/actionlint/cmd/actionlint@latest -verbose .github/workflows/ci.yml` ([`04-actionlint.txt`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1160-nextest/plans/reports/agy-1160-nextest-evidence/04-actionlint.txt)) | `verbose: Found 0 parse errors in 0 ms for .github/workflows/ci.yml`<br>`verbose: Found total 0 errors in 0 ms for .github/workflows/ci.yml` (exit code `0` for both binary and `go run` forms) |
| **2a** | CI `Test (*)` jobs run nextest | Run [`40263252550`](https://github.com/jangtrinh/design-os-studio/actions/runs/40263252550) — `Test (macos-14)` [Job 121380393645](https://github.com/jangtrinh/design-os-studio/actions/runs/40263252550/job/121380393645) ([`07-ci-macos-final.txt`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1160-nextest/plans/reports/agy-1160-nextest-evidence/07-ci-macos-final.txt)) | `Starting 8091 tests across 128 binaries (33 tests skipped)`<br>`Summary [2168.192s] 8091 tests run: 8091 passed (5 slow), 33 skipped`<br>Followed by `Test workspace doctests` (`cargo test --workspace --doc --locked`, 12 targets, all `ok`). |
| **2b** | Retried test shows as `FLAKY` | Run [`40253881103`](https://github.com/jangtrinh/design-os-studio/actions/runs/40253881103) — `Test (macos-14)` [Job 121351800615](https://github.com/jangtrinh/design-os-studio/actions/runs/40253881103/job/121351800615) ([`06-ci-run1-flaky-and-summary.txt`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1160-nextest/plans/reports/agy-1160-nextest-evidence/06-ci-run1-flaky-and-summary.txt)) | `FLAKY 2/3 [   0.779s] (3059/8084) designos-app::bin/designos-studio agent_mention::tests::search_is_budgeted_on_a_huge_document`<br>`FLAKY 3/3 [   0.707s] (6594/8084) designos-eval::harness bootstrap_is_reproducible`<br>`FLAKY 2/3 [   0.456s] (6928/8084) designos-fig pen::tests::skipped_refs_do_not_re_walk_a_huge_component`<br>`FLAKY 2/3 [   7.708s] (8084/8084) designos-webcapture::capture_tests rust_parses_capture` |
| **2c** | macOS `Test workspace` time before and after | Baseline `main` vs PR `#1185` Runs 1 & 3 | • **Before (issue `#1160` baseline):** `~897 s` (`14m 57s`); recent `main` runs with 8M-node clone-budget tests added (`40252160103` / job `121346257116`): **`2,217 s`** (`36m 57s`, `01:02:13Z` → `01:39:10Z`).<br>• **After (PR `#1185` Run 1 `40253881103` / job `121351800615`, fully parallel):** **`1,536.04 s`** (`25m 36s` nextest execution).<br>• **After (PR `#1185` Run 3 `40263252550` / job `121380393645`, with `threads-required = "num-cpus"` serializing the two `~580s` 8M-node clone-budget tests + `~237s` `chained_links_stop_at_the_global_clone_budget`):** **`2,168.19 s`** (`36m 08s` nextest execution; step wall time `2,321 s` including compile + `14 s` doctests) and **100% green (`8,091 / 8,091` passed)**. |
| **3** | PR body lists quarantine entries with issues and test counts | Draft PR [#1185](https://github.com/jangtrinh/design-os-studio/pull/1185) | Verified in PR `#1185` description. |
| **4** | Commit as `ci: …`, push, open draft PR, delete `out/` | `git log`, `test ! -e out` | Committed as `ci: run workspace tests with cargo-nextest, retries, and quarantine list (#1160)`, pushed to `jangtrinh/agy-1160-nextest`, opened Draft PR `#1185`, and verified `out/` is deleted. |

---

## 5. Pasted CLI & CI Outputs

### 5.1 Local `cargo nextest --version` & Test Counts (`01-nextest-version.txt`, `02-test-counts.txt`)

```text
$ cargo nextest --version
cargo-nextest 0.9.149 (3fe4ef319 2026-10-20)
release: 0.9.149
commit-hash: 3fe4ef31972f24e0c94cfe291718eab0fe678260
commit-date: 2026-10-20
host: aarch64-apple-darwin

=== cargo nextest list --workspace --locked --profile ci | wc -l ===
    8084
=== cargo nextest list --workspace --locked --profile ci --run-ignored only | wc -l ===
      33
=== cargo nextest list --workspace --locked --profile ci --run-ignored all | wc -l ===
    8117
=== cargo test --workspace --locked -- --list | grep -c ': test$' ===
8117
=== cargo test --workspace --locked -- --ignored --list | grep -c ': test$' ===
33
=== cargo test --workspace --doc --locked (doctest targets: 12 targets, 0 doctest cases) ===
   Doc-tests designos_diff       -> 0 passed; 0 failed; 0 ignored
   Doc-tests designos_doc        -> 0 passed; 0 failed; 0 ignored
   Doc-tests designos_eval       -> 0 passed; 0 failed; 0 ignored
   Doc-tests designos_export     -> 0 passed; 0 failed; 0 ignored
   Doc-tests designos_fig        -> 0 passed; 0 failed; 0 ignored
   Doc-tests designos_fonts      -> 0 passed; 0 failed; 0 ignored
   Doc-tests designos_gpu        -> 0 passed; 0 failed; 0 ignored
   Doc-tests designos_library    -> 0 passed; 0 failed; 0 ignored
   Doc-tests designos_mcp        -> 0 passed; 0 failed; 0 ignored
   Doc-tests designos_render     -> 0 passed; 0 failed; 0 ignored
   Doc-tests designos_svg        -> 0 passed; 0 failed; 0 ignored
   Doc-tests designos_webcapture -> 0 passed; 0 failed; 0 ignored
```

### 5.2 Local `actionlint` Verification (`04-actionlint.txt`)

```text
=== out/tools/bin/actionlint -verbose .github/workflows/ci.yml ===
verbose: Linting .github/workflows/ci.yml
verbose: Using project at /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1160-nextest
verbose: Found 0 parse errors in 0 ms for .github/workflows/ci.yml
verbose: Rule "shellcheck" was disabled: exec: "shellcheck": executable file not found in $PATH
verbose: Rule "pyflakes" was disabled: exec: "pyflakes": executable file not found in $PATH
verbose: Found total 0 errors in 1 ms for .github/workflows/ci.yml

=== go run github.com/rhysd/actionlint/cmd/actionlint@latest -verbose .github/workflows/ci.yml ===
verbose: Linting .github/workflows/ci.yml
verbose: Using project at /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1160-nextest
verbose: Found 0 parse errors in 0 ms for .github/workflows/ci.yml
verbose: Rule "shellcheck" was disabled: exec: "shellcheck": executable file not found in $PATH
verbose: Rule "pyflakes" was disabled: exec: "pyflakes": executable file not found in $PATH
verbose: Found total 0 errors in 0 ms for .github/workflows/ci.yml
```

### 5.3 CI `FLAKY` Reporting Output (Run `40253881103` / Job `121351800615`, `06-ci-run1-flaky-and-summary.txt`)

```text
  TRY 1 FAIL [   1.141s] (─────────) designos-app::bin/designos-studio agent_mention::tests::search_is_budgeted_on_a_huge_document
  TRY 2 PASS [   0.779s] (3059/8084) designos-app::bin/designos-studio agent_mention::tests::search_is_budgeted_on_a_huge_document
  TRY 1 FAIL [   1.988s] (─────────) designos-eval::harness bootstrap_is_reproducible
  TRY 3 PASS [   0.707s] (6594/8084) designos-eval::harness bootstrap_is_reproducible
  TRY 1 FAIL [   1.654s] (─────────) designos-fig pen::tests::skipped_refs_do_not_re_walk_a_huge_component
  TRY 2 PASS [   0.456s] (6928/8084) designos-fig pen::tests::skipped_refs_do_not_re_walk_a_huge_component
  TRY 1 FAIL [  10.028s] (─────────) designos-webcapture::capture_tests rust_parses_capture
  TRY 2 PASS [   7.708s] (8084/8084) designos-webcapture::capture_tests rust_parses_capture
   FLAKY 2/3 [   0.779s] (3059/8084) designos-app::bin/designos-studio agent_mention::tests::search_is_budgeted_on_a_huge_document
   FLAKY 3/3 [   0.707s] (6594/8084) designos-eval::harness bootstrap_is_reproducible
   FLAKY 2/3 [   0.456s] (6928/8084) designos-fig pen::tests::skipped_refs_do_not_re_walk_a_huge_component
   FLAKY 2/3 [   7.708s] (8084/8084) designos-webcapture::capture_tests rust_parses_capture
```

### 5.4 CI Final `Test (macos-14)` Summary (Run `40263252550` / Job `121380393645`, `07-ci-macos-final.txt`)

```text
    Starting 8091 tests across 128 binaries (33 tests skipped)
     Summary [2168.192s] 8091 tests run: 8091 passed (5 slow), 33 skipped
        SLOW [ 578.276s] (1115/8091) designos-app::bin/designos io::tests::opening_file_that_hits_clone_budget_surfaces_unexpanded_instances_toast
        SLOW [ 580.619s] (4072/8091) designos-app::bin/designos-studio io::tests::opening_file_that_hits_clone_budget_surfaces_unexpanded_instances_toast
        SLOW [ 236.827s] (6943/8091) designos-fig rest_import::expand_depth_tests::chained_links_stop_at_the_global_clone_budget
        SLOW [  43.695s] (7951/8091) designos-render blur::tests::downsampled_within_two_levels
        SLOW [  60.411s] (7952/8091) designos-render shadow_tests::rrect_shadow_matches_generic
```

---

## 6. Quarantine List & Tracking Issues

All 6 flaky tests listed in `#1160` were confirmed in CI / gate history and configured in [`.config/nextest.toml`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1160-nextest/.config/nextest.toml) under `[test-groups.quarantine]` (`max-threads = 1`, `retries = 4`, never skipped):

| # | Filterset in `.config/nextest.toml` | Retries | Test Group | Tracking Issue (`for:pm`) | CI History Evidence |
|---|---|---|---|---|---|
| 1 | `test(=shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one)` | `4` | `quarantine` | [#996](https://github.com/jangtrinh/design-os-studio/issues/996) | Run `39660535531` (`macos-14`) & Run `39664345650` (`ubuntu-latest`) |
| 2 | `test(=terminal::gemini_narrow_tests::output_the_cli_prints_through_its_own_terminal_is_read)` | `4` | `quarantine` | [#1179](https://github.com/jangtrinh/design-os-studio/issues/1179) | Run `40174351742` / Job `121132920999` (`macos-14`) |
| 3 | `package(designos-fig) & test(=variables_dtcg::tests::dtcg_import_scales_linearly_with_variable_count)` | `4` | `quarantine` | [#1180](https://github.com/jangtrinh/design-os-studio/issues/1180) | Run `40182699259` / Job `121156022040` (`ubuntu-latest`, ratio `9.00x > 6.0x`) |
| 4 | `test(=agent_session_files::inner_link_race_tests::no_byte_lands_in_the_target_of_a_link_swapped_in_while_studio_rewrites_the_session_files)` | `4` | `quarantine` | [#1181](https://github.com/jangtrinh/design-os-studio/issues/1181) | Run `40170147644` / Job `121121414189` (`macos-14`) |
| 5 | `test(=terminal::tests::stub_cli_drives_running_needs_input_done)` | `4` | `quarantine` | [#1182](https://github.com/jangtrinh/design-os-studio/issues/1182) | Gate evidence `plans/reports/gate-g4-pr1071-1064.md` & `gate-g4-pr1075-1037.md` |
| 6 | `test(=terminal::tests::stub_cli_off_the_probed_path_that_prints_unauthorized)` | `4` | `quarantine` | [#1183](https://github.com/jangtrinh/design-os-studio/issues/1183) | Gate evidence `plans/reports/gate-g4-pr1071-1064.md` & `gate-g4-pr1075-1037.md` |

---

## 7. Assumptions & Weakest Link

- **Assumptions:**
  - `[ASSUMPTION]` `taiki-e/install-action@v2.87.31` with `tool: nextest@0.9.149` is the preferred tag-pinned form of `taiki-e/install-action@nextest` so both the GitHub Action release tag (`v2.87.31`) and the `cargo-nextest` binary version (`0.9.149`) are deterministically pinned.
  - `[ASSUMPTION]` Duplicate execution of `designos-app` unit tests across `bin/designos` and `bin/designos-studio` (because `crates/designos-app/Cargo.toml` defines two `[[bin]]` targets sharing `src/main.rs` without `[lib]`) is intentional until a separate target-dedup ticket is scheduled; in particular, `opening_file_that_hits_clone_budget_surfaces_unexpanded_instances_toast` runs twice (`~578s` + `~581s` = `~1,159s` alone).
- **Weakest link:** The two `opening_file_that_hits_clone_budget_surfaces_unexpanded_instances_toast` debug-mode 8M-node expansion tests account for `1,158.9s` (`53%`) of the entire `2,168.2s` macOS nextest run because they must run with `threads-required = "num-cpus"` to avoid starving concurrent sub-8ms frame-budget tests (`large_fixture_off_thread_budget`).

Status: READY FOR REVIEW — PR #1185
