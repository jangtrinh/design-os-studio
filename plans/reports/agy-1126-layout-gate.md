# DEV Report — #1126 (Spec L, P0) · Round 1 of 3 (Section 12 Steps 1–3)

## 1. What Could NOT Be Verified (Stated First)

- **Section 12 Steps 4–9 and Bless (Round 2 & Round 3 scope):** Per the Round 1 dispatch instructions, this round's scope is strictly Section 12 Steps 1–3:
  - **Step 1:** `crates/designos-app/src/cli_resolve.rs` and the five resolvers routed through it, the hermetic predicate, the spawn arm, the PTY env, the startup check with its acknowledgement, and `warm_cli_version_cache`. Tests `H1–H7` and `H9–H11`.
  - **Step 2:** Checked-in stubs (`H8`) in `crates/designos-app/tests/layout-baseline/stubs/`, plus `stub_dir()` in test support.
  - **Step 3:** `ShellRegions`, `apply_launch_defaults` (`8e`), listable `record` (`1b`), and the `vector.rs` fix (`vector_toolbar_uses_widgets_record`).
  - Steps 4–7 (`layout_snapshot.rs`, comparer, `Sim` seams, per-spec gate tests, `--layout-json`) and Steps 8–9 (`layout_baseline_capture.py`, bless, `AC-01..AC-03`, `AC-06`) belong to Rounds 2 and 3 and were not executed in Round 1.
- **macOS GitHub Actions Hosted Runner (`Test (macos-14)`):** All local verification gates were executed under the isolated test harness on Apple Silicon (`macos`); GitHub Actions CI runs on the opened draft PR.

---

## 2. Drift Table (`7e15ddae` → `HEAD` `3c0818a6b`)

All cited files and lines in `plans/specs/ui-revamp/detail/1126-layout-unchanged-gate-baseline-capture.md` were re-anchored against `origin/main` (`3c0818a6b09112a53dbc2576bf725703a0298385`) before writing any code. Only plain line shifts (`+4` lines inside `mod tests` of `terminal.rs` from `#1174` adding `#[cfg_attr(windows, ignore = ...)]`) exist; zero behavioral drift.

| Cited File | Spec Line (`7e15ddae`) | HEAD Line (`3c0818a6b`) | Delta | Symbol / Context | Classification |
|---|---|---|---|---|---|
| `crates/designos-app/src/widgets/mod.rs` | `:245-283` | `:245-283` | `0` | `record` hook, `RECORD_DISABLED`, `RecordDisabled` guard | FACT — identical |
| `crates/designos-app/src/canvas/vector.rs` | `:1125` | `:1125` | `0` | Direct `test-rect` write for `vtool:<label>` | FACT — identical |
| `crates/designos-app/src/shell.rs` | `:94-96`, `:98`, `:102-105`, `:107-110`, `:121`, `:123` | `:94-96`, `:98`, `:102-105`, `:107-110`, `:121`, `:123` | `0` | `Shell::new` launch defaults, `pages_height`, `gpu.pref`, `tabs.always_visible` | FACT — identical |
| `crates/designos-app/src/shell.rs` | `:132`, `:214`, `:276`, `:277-321`, `:287-294`, `:327-329`, `:352` | `:132`, `:214`, `:276`, `:277-321`, `:287-294`, `:327-329`, `:352` | `0` | `Shell::build`, rulers default, panel match, agent branches, `agents-chat` 800ms loop, `terminal`, `libraries` | FACT — identical |
| `crates/designos-app/src/shell.rs` | `:413-490`, `:455-470` | `:413-490`, `:455-470` | `0` | `drive_screenshot` and PNG write | FACT — identical |
| `crates/designos-app/src/shell.rs` | `:1363`, `:1375`, `:1386`, `:1410`, `:1436-1442`, `:1447-1466`, `:1473-1488`, `:1509-1514`, `:1517-1522`, `:1559-1562`, `:3460` | `:1363`, `:1375`, `:1386`, `:1410`, `:1436-1442`, `:1447-1466`, `:1473-1488`, `:1509-1514`, `:1517-1522`, `:1559-1562`, `:3460` | `0` | `Shell::draw`, `draw_window`, title bar, tab bar, region rects, `left-panel-resize-handle`, `micro-seq-hud` | FACT — identical |
| `crates/designos-app/src/terminal.rs` | `:87`, `:250-276`, `:294-299`, `:328-329`, `:335-339`, `:341`, `:353` | `:87`, `:250-276`, `:294-299`, `:328-329`, `:335-339`, `:341`, `:353` | `0` | `FAILED_START_FIXTURE_COMMAND`, presets, `is_installed`, `extra_cli_path_dirs`, `runtime_binary_available[_in]` | FACT — identical |
| `crates/designos-app/src/terminal.rs` | `:620-624`, `:646`, `:656-661` | `:620-624`, `:646`, `:656-661` | `0` | `spawn_with_session_cmd`, `warm_cli_version_cache` call, Shell `-l` arm, test `sh -c 'sleep 5'` | FACT — identical |
| `crates/designos-app/src/terminal.rs` | `:1428`, `:1876-1888`, `:2542-2554`, `:2947-2953`, `:3031-3035` | `:1428`, `:1876-1888`, `:2542-2554`, `:2947-2953`, `:3031-3035` | `0` | Session temp dirs, failed-start wait, bounded `is_alive` poll, `refresh_tab_usage` | FACT — identical |
| `crates/designos-app/src/terminal.rs` | `:4345-4351`, `:4365-4391`, `:4870-4892` | `:4345-4351`, `:4365-4391`, `:4870-4892` | `0` | `default_shell_program`, `build_agent_shell_cmd`, `configure_pty_env_with_session` | FACT — identical |
| `crates/designos-app/src/terminal.rs` | `:8766` (`8743-8766`) | `:8770` (`8747-8770`) | `+4` | `gemini_live_check` (plain line shift from `#1174` `#[cfg_attr(windows, ignore)]` in `mod tests`) | FACT — plain line shift |
| `crates/designos-app/src/terminal.rs` | `:11259`, `:11270` (`11263`, `11274`) | `:11263`, `:11274` | `+4` | `pin_installed_catalog_clis`, `install_stub_claude` (plain line shift from `#1174` in `mod tests`) | FACT — plain line shift |
| `crates/designos-app/src/agent_cli_catalog.rs` | `:110-111`, `:200-215`, `:253`, `:434-466` | `:110-111`, `:200-215`, `:253`, `:434-466` | `0` | Probe args, `SystemCommandRunner::resolve_binary`, `run` PATH, `DefaultTestRunner` | FACT — identical |
| `crates/designos-app/src/agent_session_failure.rs` | `:543-590`, `:552`, `:560`, `:572-590` | `:543-590`, `:552`, `:560`, `:572-590` | `0` | `warm_cli_version_cache`, `probe_installed_cli_version`, `resolve_cli_executable` | FACT — identical |
| `crates/designos-app/src/agent_usage.rs` | `:563`, `:601-624`, `:609` | `:563`, `:601-624`, `:609` | `0` | `grok usage <sid>`, `resolve_grok_binary` | FACT — identical |
| `crates/designos-app/src/agent_composer.rs` | `:746-748` | `:746-748` | `0` | Composer auto-spawn `is_installed` check | FACT — identical |
| `crates/designos-app/src/agent_sessions.rs` | `:266-269`, `:464-501` | `:266-269`, `:464-501` | `0` | `designos-sessions` temp dir, resume commands | FACT — identical |
| `crates/designos-app/src/main.rs` | `:149`, `:180`, `:187-190` | `:149`, `:180`, `:187-190` | `0` | `spawn_startup_sweep`, `Shell::new`, screenshot exit 1 | FACT — identical |
| `crates/designos-app/src/cli.rs` | `:5-102`, `:219-337`, `:254-258`, `:353-371` | `:5-102`, `:219-337`, `:254-258`, `:353-371` | `0` | `Options`, `cli::parse`, `USAGE` | FACT — identical |
| `crates/designos-app/src/shell/tests.rs` | `:31-52`, `:53-125`, `:193-194`, `:212-219`, `:1114-1182` | `:31-52`, `:53-125`, `:193-194`, `:212-219`, `:1114-1182` | `0` | `stub_agent_cli_dir`, `Sim`, `rect_of`, `use_stub_agent_clis`, test module registrations | FACT — identical |

---

## 3. Build Order & Commit Log (Section 12 Steps 1–3, Red Before Fix)

Every step followed the strict red-then-green commit discipline on branch `jangtrinh/agy-1126-layout-gate`:

| Step | Phase | Commit | Summary | Test Result |
|---|---|---|---|---|
| **Step 1** | **RED** | `81c01923a` | `test(app): add red tests H1-H7 and H9-H11 for hermetic CLI resolution` | `0 passed; 10 failed` |
| **Step 1** | **GREEN** | `5abf34ca1` | `feat(app): add cli_resolve and hermetic agent CLI mode` | `10 passed; 0 failed` |
| **Step 2** | **RED** | `d0999a55b` | `test(app): add red test H8 for checked-in agent CLI stubs` | `0 passed; 1 failed` |
| **Step 2** | **GREEN** | `b0605984f` | `feat(app): check in hermetic agent CLI stubs and wire stub_dir` | `1 passed; 0 failed` |
| **Step 3** | **RED** | `409cd7340` | `test(app): add red tests 1b, 8e and vector record for regions and record hook` | `0 passed; 3 failed` |
| **Step 3** | **GREEN** | `db1ac784c` | `feat(app): add ShellRegions, apply_launch_defaults, listable record and vector record fix` | `3 passed; 0 failed` |

---

## 4. Clean `PATH` Verification (Ruling 3)

All red and green runs executed with `PATH="/usr/bin:/bin:/usr/sbin:/sbin:$HOME/.cargo/bin"` under an isolated `HOME` (`$TEST_HOME`), `TMPDIR` (`$TEST_HOME/tmp`), and `DESIGNOS_LIBRARY` (`$TEST_HOME/lib`).

```text
$ export PATH="/usr/bin:/bin:/usr/sbin:/sbin:$HOME/.cargo/bin"
$ for c in claude codex gemini grok agy opencode; do command -v $c; done
(empty output, exit 0 — plans/reports/agy-1126-layout-gate-evidence/clean-path-check.log)
```

---

## 5. Red and Green Test Logs (Section 12 Steps 1–3)

Full logs are saved in:
- `plans/reports/agy-1126-layout-gate-evidence/red.log`
- `plans/reports/agy-1126-layout-gate-evidence/green.log`

### Red Phase Excerpt (`red.log`)

```text
=== Step 1 RED (H1-H7, H9-H11) ===
running 10 tests
test cli_resolve::tests::scoped_guard_applies_only_inside_a_gate_case ... FAILED
test cli_resolve::tests::every_resolver_uses_resolve_in ... FAILED
test cli_resolve::tests::search_dirs_precedence_is_hermetic_scoped_explicit_host ... FAILED
test cli_resolve::tests::version_warmer_resolves_on_the_calling_thread ... FAILED
test cli_resolve::tests::hermetic_mode_is_denied_without_every_requirement ... FAILED
test cli_resolve::tests::hermetic_mode_writes_the_acknowledgement ... FAILED
test cli_resolve::tests::hermetic_mode_refuses_compound_or_unknown_commands ... FAILED
test cli_resolve::tests::hermetic_shell_cmd_has_no_login_or_interactive_flags ... FAILED
test cli_resolve::tests::hermetic_pty_env_is_stub_path_only ... FAILED
test cli_resolve::tests::hermetic_resolver_census ... FAILED
test result: FAILED. 0 passed; 10 failed; 0 ignored; 0 measured; 2965 filtered out; finished in 0.09s

=== Step 2 RED (H8) ===
running 1 test
test cli_resolve::tests::stub_scripts_behave ... FAILED
thread 'cli_resolve::tests::stub_scripts_behave' panicked at crates/designos-app/src/cli_resolve.rs:609:13:
missing checked-in stub .../crates/designos-app/tests/layout-baseline/stubs/claude
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 2975 filtered out; finished in 0.00s

=== Step 3 RED (1b, 8e, vector_toolbar_uses_widgets_record) ===
running 1 test
test widgets::tests::record_hook_lists_every_name_of_the_pass ... FAILED
assertion `left == right` failed: two calls must be listed and RecordDisabled call ignored
  left: []
 right: [("alpha", [[10.0 20.0] - [40.0 60.0]]), ("beta", [[50.0 60.0] - [120.0 140.0]])]
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 2978 filtered out; finished in 0.00s

running 1 test
test shell::tests::agent_a11y::shell_new_uses_launch_defaults_for_its_own_mode ... FAILED
Shell::new with screenshot: Some(..) must set screenshot_chrome = true
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 2978 filtered out; finished in 0.24s

running 1 test
test canvas::vector::tests::vector_toolbar_uses_widgets_record ... FAILED
vector::toolbar must record `vtool:Move` via widgets::record, got: []
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 2978 filtered out; finished in 0.02s
```

### Green Phase Excerpt (`green.log`)

```text
=== Step 1 GREEN (H1-H7, H9-H11) ===
running 10 tests
test cli_resolve::tests::search_dirs_precedence_is_hermetic_scoped_explicit_host ... ok
test cli_resolve::tests::hermetic_mode_refuses_compound_or_unknown_commands ... ok
test cli_resolve::tests::every_resolver_uses_resolve_in ... ok
test cli_resolve::tests::scoped_guard_applies_only_inside_a_gate_case ... ok
test cli_resolve::tests::version_warmer_resolves_on_the_calling_thread ... ok
test cli_resolve::tests::hermetic_pty_env_is_stub_path_only ... ok
test cli_resolve::tests::hermetic_mode_writes_the_acknowledgement ... ok
test cli_resolve::tests::hermetic_shell_cmd_has_no_login_or_interactive_flags ... ok
test cli_resolve::tests::hermetic_mode_is_denied_without_every_requirement ... ok
test cli_resolve::tests::hermetic_resolver_census ... ok
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 2965 filtered out; finished in 0.09s

=== Step 2 GREEN (H8) ===
running 1 test
test cli_resolve::tests::stub_scripts_behave ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2975 filtered out; finished in 2.89s

=== Step 3 GREEN (1b, 8e, vector_toolbar_uses_widgets_record) ===
running 1 test
test widgets::tests::record_hook_lists_every_name_of_the_pass ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2978 filtered out; finished in 0.01s

running 1 test
test shell::tests::agent_a11y::shell_new_uses_launch_defaults_for_its_own_mode ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2978 filtered out; finished in 0.40s

running 1 test
test canvas::vector::tests::vector_toolbar_uses_widgets_record ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2978 filtered out; finished in 0.01s
```

---

## 6. AC-05 Verification (Hermetic Startup Guard, Exit Code 2)

Verified both by unit tests `H4` (`hermetic_mode_is_denied_without_every_requirement`) and `H9` (`hermetic_mode_writes_the_acknowledgement`) and by manual CLI invocation (`plans/reports/agy-1126-layout-gate-evidence/ac05.log`):

```text
=== AC-05 Manual Check 1: DESIGNOS_AGENT_CLI_DIR set without --screenshot or markers ===
designos: hermetic agent mode denied: --screenshot is required; DESIGNOS_AGENT_CLI_DIR (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app/tests/layout-baseline/stubs) is missing .designos-hermetic; DESIGNOS_LIBRARY is unset; TMPDIR (/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.ARgHcnuuUW/tmp) is missing .designos-hermetic; HOME (/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.ARgHcnuuUW) is missing .designos-hermetic
exit_code=2
tmpdir_contents=
=== AC-05 Manual Check 2: DESIGNOS_AGENT_CLI_DIR set with --screenshot but missing TMPDIR marker ===
designos: hermetic agent mode denied: TMPDIR (/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/tmp.ARgHcnuuUW/tmp) is missing .designos-hermetic
exit_code=2
ack_exists=no
```

---

## 7. Developer Mac Real-CLI Check (Ruling 3)

Executed once after the hermetic predicate was green under the isolated `HOME`/`TMPDIR`/`DESIGNOS_LIBRARY` harness with the normal developer `PATH` where all six real CLIs (`claude`, `codex`, `gemini`, `grok`, `agy`, `opencode`) are installed (`plans/reports/agy-1126-layout-gate-evidence/real-cli-check.log`):

```text
=== Developer Mac Real-CLI Check (normal PATH) ===
claude -> /opt/homebrew/bin/claude
codex -> /opt/homebrew/bin/codex
gemini -> /opt/homebrew/bin/gemini
grok -> /Users/jangtrinh/.grok/bin/grok
agy -> /opt/homebrew/bin/agy
opencode -> /Users/jangtrinh/.opencode/bin/opencode

running 11 tests
test cli_resolve::tests::search_dirs_precedence_is_hermetic_scoped_explicit_host ... ok
test cli_resolve::tests::hermetic_mode_refuses_compound_or_unknown_commands ... ok
test cli_resolve::tests::every_resolver_uses_resolve_in ... ok
test cli_resolve::tests::version_warmer_resolves_on_the_calling_thread ... ok
test cli_resolve::tests::scoped_guard_applies_only_inside_a_gate_case ... ok
test cli_resolve::tests::hermetic_mode_writes_the_acknowledgement ... ok
test cli_resolve::tests::hermetic_shell_cmd_has_no_login_or_interactive_flags ... ok
test cli_resolve::tests::hermetic_pty_env_is_stub_path_only ... ok
test cli_resolve::tests::hermetic_mode_is_denied_without_every_requirement ... ok
test cli_resolve::tests::hermetic_resolver_census ... ok
test cli_resolve::tests::stub_scripts_behave ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 2968 filtered out; finished in 0.09s

=== Stub invocation log under normal PATH ===
/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app/tests/layout-baseline/stubs/claude --check-invocation
/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app/tests/layout-baseline/stubs/codex --check-invocation
/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app/tests/layout-baseline/stubs/agy --check-invocation
/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app/tests/layout-baseline/stubs/grok --check-invocation
/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app/tests/layout-baseline/stubs/gemini --check-invocation
/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app/tests/layout-baseline/stubs/opencode --check-invocation
VERIFIED: all 6 invocations in invocations.log start with /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app/tests/layout-baseline/stubs/
```

---

## 8. Verification Gates & Test Pass Counts (`origin/main` vs Round 1 Branch)

Full summary lines are saved in `plans/reports/agy-1126-layout-gate-evidence/test-counts.log`.

| Gate / Suite | `origin/main` (`3c0818a6b`) | Round 1 (`db1ac784c`) | Delta / Notes |
|---|---|---|---|
| `cargo test --workspace --locked --lib` | `1,281 passed; 0 failed; 3 ignored` | `1,281 passed; 0 failed; 3 ignored` | `+0` (identical) |
| `cargo test -p designos-app --bin designos --locked` | `2,965 total` (`2,957 passed; 0 failed; 8 ignored`) | `2,979 total` (`2,971 passed; 0 failed; 8 ignored`) | `+14` new tests (`H1–H11`, `1b`, `8e`, `vector_toolbar_uses_widgets_record`); zero existing expected values changed |
| `cargo fmt --all -- --check` | PASS | PASS (exit 0) | Clean formatting |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS | PASS (exit 0, 0 warnings) | Clean clippy |
| `python3 -m unittest discover -s scripts/tests -v` | PASS | PASS (exit 0) | All script unit tests green |
| `python3 scripts/zero_fake_ui_gate.py` | PASS | PASS (17/17 invariants) | Clean |
| `python3 scripts/check_spacing_relationships.py` | PASS | PASS (`no count rose above the baseline`) | Clean |
| `out/cargo` cleanup | N/A | Removed (`rm -rf out/cargo`) | Verified absent before push |

---

## Round 2

### 1. What Could NOT Be Verified (Stated First)

- **Nothing in Round 2's scope was left unverified (`FACT`).** All 8 controller rulings (Rulings 1–8), all 4 Majors and 4 Minors from Grok's Round 1 review (`review-1126-pr1206-r1-grok.md`), and all 6 Acceptance items were executed and verified locally on macOS (`aarch64-apple-darwin`).
- **Out-of-scope rounds (by explicit controller ruling):** Section 12 Steps 4–7 are deferred to Round 3, and Section 12 Steps 8–9 plus the baseline bless are deferred to Round 4. Windows/Linux CI targets are not executed locally on this macOS worker.

### 2. Conclusion & Epistemic Ledger (`FACT` / `INFERENCE` / `ASSUMPTION`)

- **Conclusion (`FACT`):** All Round 1 hermetic-mode escapes and clippy warnings on PR `#1206` are closed and locked by red-first regression tests (`e0e2f066`, `plans/reports/agy-1126-layout-gate-evidence/r2-red.log`) and green verification (`plans/reports/agy-1126-layout-gate-evidence/r2-green.log`).
- **Epistemic Ledger:**
  - `FACT` — Merging `origin/main` (`7f9000a29`, bringing `#1185` and `#1188`) into `0bd7e9ec` succeeded cleanly as merge commit `a03cb5144`.
  - `FACT` — Ruling 1: `cli_resolve::enforce_hermetic_startup_os` is now the very first call inside `run_startup_before_window` in `crates/designos-app/src/main.rs`, running before `--help`, `--version`, `missing_file_message`, `--seed-library`, and `run_agent_cli` (`--apply-spec`, etc.). It reads `std::env::vars_os()`; a non-UTF-8 `DESIGNOS_AGENT_CLI_DIR` returns `Err` (exit 2), while non-UTF-8 values of other variables are ignored without panicking.
  - `FACT` — Ruling 2: In hermetic mode (`hermetic_dir.is_some()`), `build_agent_shell_cmd_in` / `build_agent_shell_cmd_with_session_in` never invokes `/bin/sh -c`. It rejects `&`, `|`, `;`, `<`, `>`, `` ` ``, `$(`, `\n`, and `\r`, splits argv via `shell-words` `1.1.1` (already present in `Cargo.lock`, adding zero `[[package]]` entries), resolves `argv[0]` inside the canonical stub directory via `resolve_in_hermetic`, expands only `$DESIGNOS_*` and `${DESIGNOS_*}` from the tab's `DESIGNOS_*` environment, refuses any other `$` expansion, and constructs `CommandBuilder::new(canonical_stub_path)` directly. Outside hermetic mode (`hermetic_dir.is_none()`), `build_agent_shell_cmd` is byte-for-byte unchanged.
  - `FACT` — Ruling 3: `validate_and_canonicalize_hermetic_dir` requires `DESIGNOS_AGENT_CLI_DIR` to be an absolute path with no `.` or `..` components, canonicalizes it, and verifies `.designos-hermetic`. `hermetic_agent_cli_dir()`, `resolve_in_hermetic`, `resolve_in`, `build_agent_shell_cmd_with_session_in`, and `configure_pty_env_with_session_and_hermetic` all use the canonical path, and `resolve_in_hermetic` canonicalizes each candidate and refuses any target whose canonical parent is not the canonical stub directory (blocking symlinks pointing outside the stub dir, such as `/opt/homebrew/bin/claude`).
  - `FACT` — Ruling 4: Removed bare-name fallbacks across `agent_cli_catalog.rs` (`SystemCommandRunner::run`), `terminal.rs` (`refresh_tab_usage` grok binary lookup, `runtime_binary_available_in`, `agent_binary_reachable`, `failed_start` `installed` check, `gemini_live_check`), and `agent_usage.rs` (`run_grok_usage_with_timeout`), and widened the `H10` / `8(g)` census (`hermetic_resolver_census`) to scan `crates/designos-app/src/**/*.rs` for `var("PATH")`, `var_os("PATH")`, `split_paths(`, `Command::new("<agent>")`, and `PathBuf::from("<agent>")`.
  - `FACT` — Ruling 5: Restored the exact legacy non-hermetic `PATH` construction in `agent_cli_catalog.rs` via `cli_resolve::catalog_child_path` (`install_folders` + `extra_cli_path_dirs()` prepended to the raw process `PATH` in the old order) and locked it with `non_hermetic_catalog_path_prepends_install_and_extra_before_host_path`.
  - `FACT` — Ruling 6: Added `widgets::recording()` and lazy `widgets::record_with(ui, name_fn, rect)` in `crates/designos-app/src/widgets/mod.rs` and switched `crates/designos-app/src/canvas/vector.rs` to `widgets::record_with(ui, || format!("vtool:{}", t.label()), r)`, eliminating per-frame `String` allocation when widget recording is off.
  - `FACT` — Ruling 7: Moved `#[cfg(test)] pub fn stub_dir()` above `mod tests` in `cli_resolve.rs`, moved `#[cfg(test)] pub(crate) fn pin_installed_catalog_clis` above `mod ask_mode_tests` / `mod tests` in `terminal.rs` (introduced by `#1188` on `origin/main`), and replaced all three `field_reassign_with_default` sites in `crates/designos-app/src/shell/tests.rs` (`:1782`, `:1793`, `:1805`) with struct-update syntax. `cargo clippy --workspace --all-targets --locked -- -D warnings` exits `0`.
  - `ASSUMPTION` — When `build_agent_shell_cmd_in` is called in unit tests without a live `SessionEnv` (`session: None`), `$DESIGNOS_*` variables in preset launch commands expand from the process's `DESIGNOS_*` env vars overlaid on the default fallback tab `DESIGNOS_*` paths (the same fallback paths used by `configure_pty_env_with_session_and_hermetic`), and an unset `$DESIGNOS_<IDENT>` variable expands to `""` while any non-`DESIGNOS_*` `$` variable is refused with an error.
  - **Weakest link (one line):** `expand_hermetic_designos_vars` expands `$DESIGNOS_*` tokens after `shell_words::split` word tokenization, which relies on preset launch strings keeping `$DESIGNOS_*` outside single-quoted segments (true for all presets and `codex_mcp_overrides!`).

### 3. Skills Loaded & Changes Made Because of Them

- `/es:dev` -> `es:feature-dev`: Treated the Round 2 prompt and controller rulings as the approved spec; executed Red -> Green -> Verify without stopping for brainstorm/plan gates.
- `/es:rust`: Used `widgets::record_with(ui, || ..., r)` closure laziness so `vector.rs` performs zero heap allocation (`format!`) on normal non-recording frames (Ruling 6), and used `vars_os()` / `OsString` conversion to avoid panicking on non-UTF-8 environment variables (Ruling 1).
- `/es:rust-gpu-canvas`: Preserved all canvas/render paths untouched except the single `widgets::record_with` call in `crates/designos-app/src/canvas/vector.rs`.
- `es-designer`: Checked UI/toolbar touchpoints (`vector.rs`, `widgets/mod.rs`) to ensure zero visual or layout behavior changes while eliminating per-frame string allocation.
- `es:fable-thinking` & `es:astra-work`: Maintained `FACT`/`INFERENCE`/`ASSUMPTION` ledger, stated `PREDICT` before running gates, wrote red tests first (`e0e2f066`), and verified every artifact on the final commit before reporting.

### 4. Acceptance Criteria Verification & Pasted Outputs

| # | Acceptance Criterion | Verdict | Evidence File |
|---|---|---|---|
| **1** | `r2-red.log` shows Ruling 8 tests failing after merge (`a03cb5144`) and before fixes (`e0e2f066`); `r2-green.log` shows all 17 `cli_resolve::tests` passing | **PASS** | `plans/reports/agy-1126-layout-gate-evidence/r2-red.log`, `plans/reports/agy-1126-layout-gate-evidence/r2-green.log` |
| **2** | `git diff origin/main -- Cargo.lock` adds no `[[package]]` | **PASS** | `plans/reports/agy-1126-layout-gate-evidence/r2-cargo-lock-diff.log` |
| **3a** | Clean `PATH` check (`for c in claude codex gemini grok agy opencode; do command -v $c; done`) + `cargo fmt --all -- --check` exits 0 | **PASS** | `plans/reports/agy-1126-layout-gate-evidence/r2-clippy.log` |
| **3b** | `cargo clippy --workspace --all-targets --locked -- -D warnings` exits 0 | **PASS** | `plans/reports/agy-1126-layout-gate-evidence/r2-clippy.log` |
| **3c** | `cargo test --workspace --locked --lib` passes (`1,281 passed; 0 failed; 3 ignored`) | **PASS** | `plans/reports/agy-1126-layout-gate-evidence/r2-workspace-lib.log` |
| **3d** | `cargo test -p designos-app --bin designos --locked` passes (`2,980 passed; 0 failed; 8 ignored`) | **PASS** | `plans/reports/agy-1126-layout-gate-evidence/r2-bin-designos.log` |
| **3e** | `python3 scripts/zero_fake_ui_gate.py` (`17/17 PASS`) and `python3 scripts/check_spacing_relationships.py` (`PASS`) | **PASS** | `plans/reports/agy-1126-layout-gate-evidence/r2-python-gates.log` |
| **4** | Manual runs (`--apply-spec` with `/tmp/no-marker` + before/after SHA-256 hash, `--seed-library` with `/tmp/no-marker`, relative `DESIGNOS_AGENT_CLI_DIR=stubs`) each exit 2 and write nothing | **PASS** | `plans/reports/agy-1126-layout-gate-evidence/r2-manual.log` |
| **5** | Real-CLI check with normal `PATH`: `invocations.log` lists only stub paths | **PASS** | `plans/reports/agy-1126-layout-gate-evidence/r2-real-cli-check.log` |
| **6** | Conventional commits (`test(app): ...`, `fix(app): ...`), pushed to PR `#1206`, "Round 2" line added to PR body, `out/cargo` deleted | **PASS** | Git log & PR `#1206` |

---

#### Acceptance 1 — Red (`r2-red.log`) & Green (`r2-green.log`)

**Command:**
```sh
export PATH="/usr/bin:/bin:/usr/sbin:/sbin:$HOME/.cargo/bin"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked cli_resolve::tests -- --nocapture
```

**`r2-red.log` (on `e0e2f066`, after merging `origin/main` `7f9000a29` and before fixes):**
```text
running 17 tests
test cli_resolve::tests::search_dirs_precedence_is_hermetic_scoped_explicit_host ... ok
test cli_resolve::tests::hermetic_mode_refuses_compound_or_unknown_commands ... ok

thread 'cli_resolve::tests::hermetic_mode_refuses_background_redirect_and_unexpanded_vars' (70647744) panicked at crates/designos-app/src/cli_resolve.rs:727:28:
hermetic mode must refuse "claude & /usr/bin/true", got argv ["/bin/sh", "-c", "exec /private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.NwxslC/designos-1126-r2-8d-52528-8/claude & /usr/bin/true"]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
test cli_resolve::tests::scoped_guard_applies_only_inside_a_gate_case ... ok
test cli_resolve::tests::hermetic_mode_refuses_background_redirect_and_unexpanded_vars ... FAILED

thread 'cli_resolve::tests::hermetic_startup_refuses_relative_or_dotdot_dir' (70647751) panicked at crates/designos-app/src/cli_resolve.rs:711:74:
.. in DESIGNOS_AGENT_CLI_DIR must be denied: ()
test cli_resolve::tests::version_warmer_resolves_on_the_calling_thread ... ok
test cli_resolve::tests::every_resolver_uses_resolve_in ... ok

thread 'cli_resolve::tests::hermetic_mode_refuses_symlink_outside_stub_dir' (70647746) panicked at crates/designos-app/src/cli_resolve.rs:668:9:
assertion `left == right` failed: resolve_in in hermetic/stub dir must refuse a symlink pointing outside the stub dir
  left: Some("/private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.NwxslC/designos-1126-r2-8b-52528-13/stubs/claude")
 right: None
test cli_resolve::tests::hermetic_mode_refuses_symlink_outside_stub_dir ... FAILED
test cli_resolve::tests::hermetic_startup_refuses_relative_or_dotdot_dir ... FAILED
test cli_resolve::tests::hermetic_pty_env_is_stub_path_only ... ok

thread 'cli_resolve::tests::hermetic_shell_cmd_has_no_login_or_interactive_flags' (70647750) panicked at crates/designos-app/src/cli_resolve.rs:271:13:
assertion `left == right` failed: preset Claude argv must exec canonical binary directly without /bin/sh -c, -l, or -i
  left: ["/bin/sh", "-c", "exec /private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.NwxslC/designos-1126-h1-52528-3/claude --dangerously-skip-permissions"]
 right: ["/private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.NwxslC/designos-1126-h1-52528-3/claude", "--dangerously-skip-permissions"]
test cli_resolve::tests::hermetic_shell_cmd_has_no_login_or_interactive_flags ... FAILED
test cli_resolve::tests::hermetic_mode_writes_the_acknowledgement ... ok
test cli_resolve::tests::hermetic_mode_is_denied_without_every_requirement ... 
thread 'cli_resolve::tests::grok_usage_caller_with_no_grok_stub_spawns_nothing' (70647742) panicked at crates/designos-app/src/cli_resolve.rs:758:9:
refresh_tab_usage must not trigger grok usage subprocess when grok binary is absent from stub dir
ok
test cli_resolve::tests::grok_usage_caller_with_no_grok_stub_spawns_nothing ... FAILED
{
  "root_node_id": 2,
  "saved": true
}

thread 'cli_resolve::tests::startup_check_runs_before_seed_library_and_agent_cli' (70647755) panicked at crates/designos-app/src/cli_resolve.rs:611:49:
--apply-spec with invalid hermetic env must fail: ExitZero
test cli_resolve::tests::startup_check_runs_before_seed_library_and_agent_cli ... FAILED
test cli_resolve::tests::stub_scripts_behave ... ok

thread 'cli_resolve::tests::non_hermetic_catalog_path_prepends_install_and_extra_before_host_path' (70647752) panicked at crates/designos-app/src/cli_resolve.rs:786:9:
assertion `left == right` failed: non-hermetic catalog child PATH must prepend install_folders + extra_cli_path_dirs() before process PATH
  left: "/usr/bin:/bin:/usr/sbin:/sbin:/Users/jangtrinh/.cargo/bin:/private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.NwxslC/designos-1126-r2-8f-install-52528-11:/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.NwxslC/home/.cargo/bin:/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.NwxslC/home/.local/bin:/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.NwxslC/home/.antigravity/bin:/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.NwxslC/home/.antigravity/antigravity/bin:/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.NwxslC/home/.grok/bin:/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.NwxslC/home/.npm-global/bin:/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.NwxslC/home/.opencode/bin:/opt/homebrew/bin:/usr/local/bin"
 right: "/private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.NwxslC/designos-1126-r2-8f-install-52528-11:/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.NwxslC/home/.cargo/bin:/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.NwxslC/home/.local/bin:/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.NwxslC/home/.antigravity/bin:/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.NwxslC/home/.antigravity/antigravity/bin:/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.NwxslC/home/.grok/bin:/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.NwxslC/home/.npm-global/bin:/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.NwxslC/home/.opencode/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:/Users/jangtrinh/.cargo/bin"
test cli_resolve::tests::non_hermetic_catalog_path_prepends_install_and_extra_before_host_path ... FAILED

thread 'cli_resolve::tests::hermetic_resolver_census' (70647749) panicked at crates/designos-app/src/cli_resolve.rs:575:9:
found unallowed PATH or bare-agent lookup outside cli_resolve.rs:
terminal.rs:3038: let grok_bin = crate::agent_usage::resolve_grok_binary(&sources).unwrap_or_else(|| PathBuf::from("grok"));
terminal.rs:4971: let current_path = std::env::var("PATH").unwrap_or_default();
terminal.rs:8859: let mut list_cmd = std::process::Command::new("gemini");
test cli_resolve::tests::hermetic_resolver_census ... FAILED

failures:

failures:
    cli_resolve::tests::grok_usage_caller_with_no_grok_stub_spawns_nothing
    cli_resolve::tests::hermetic_mode_refuses_background_redirect_and_unexpanded_vars
    cli_resolve::tests::hermetic_mode_refuses_symlink_outside_stub_dir
    cli_resolve::tests::hermetic_resolver_census
    cli_resolve::tests::hermetic_shell_cmd_has_no_login_or_interactive_flags
    cli_resolve::tests::hermetic_startup_refuses_relative_or_dotdot_dir
    cli_resolve::tests::non_hermetic_catalog_path_prepends_install_and_extra_before_host_path
    cli_resolve::tests::startup_check_runs_before_seed_library_and_agent_cli

test result: FAILED. 9 passed; 8 failed; 0 ignored; 0 measured; 2971 filtered out; finished in 0.31s
```

**`r2-green.log` (after Round 2 fixes):**
```text
running 17 tests
test cli_resolve::tests::search_dirs_precedence_is_hermetic_scoped_explicit_host ... ok
test cli_resolve::tests::hermetic_mode_refuses_compound_or_unknown_commands ... ok
test cli_resolve::tests::every_resolver_uses_resolve_in ... ok
test cli_resolve::tests::hermetic_mode_refuses_symlink_outside_stub_dir ... ok
test cli_resolve::tests::scoped_guard_applies_only_inside_a_gate_case ... ok
test cli_resolve::tests::version_warmer_resolves_on_the_calling_thread ... ok
test cli_resolve::tests::hermetic_mode_writes_the_acknowledgement ... ok
test cli_resolve::tests::hermetic_startup_refuses_relative_or_dotdot_dir ... ok
test cli_resolve::tests::hermetic_mode_refuses_background_redirect_and_unexpanded_vars ... ok
test cli_resolve::tests::hermetic_pty_env_is_stub_path_only ... ok
test cli_resolve::tests::hermetic_mode_is_denied_without_every_requirement ... ok
test cli_resolve::tests::grok_usage_caller_with_no_grok_stub_spawns_nothing ... ok
test cli_resolve::tests::hermetic_shell_cmd_has_no_login_or_interactive_flags ... ok
test cli_resolve::tests::startup_check_runs_before_seed_library_and_agent_cli ... ok
test cli_resolve::tests::stub_scripts_behave ... ok
test cli_resolve::tests::hermetic_resolver_census ... ok
test cli_resolve::tests::non_hermetic_catalog_path_prepends_install_and_extra_before_host_path ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 2971 filtered out; finished in 0.42s
```

---

#### Acceptance 2 — `git diff origin/main -- Cargo.lock` (`r2-cargo-lock-diff.log`)

**Command:**
```sh
git diff origin/main -- Cargo.lock
```

**Output:**
```diff
diff --git a/Cargo.lock b/Cargo.lock
index abaa1df1..95cc6846 100644
--- a/Cargo.lock
+++ b/Cargo.lock
@@ -968,6 +968,7 @@ dependencies = [
  "roxmltree",
  "serde",
  "serde_json",
+ "shell-words",
  "toml",
  "unicode-segmentation",
  "ureq",
```

---

#### Acceptance 3 — Mandatory Harness Gates (`r2-clippy.log`, `r2-workspace-lib.log`, `r2-bin-designos.log`, `r2-python-gates.log`)

**Commands & Output (`cargo fmt --all -- --check` & `cargo clippy --workspace --all-targets --locked -- -D warnings`):**
```text
$ export PATH="/usr/bin:/bin:/usr/sbin:/sbin:$HOME/.cargo/bin"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
$ export CARGO_TARGET_DIR="$PWD/out/cargo"
$ for c in claude codex gemini grok agy opencode; do command -v "$c" || echo "$c: not found"; done
claude: not found
codex: not found
gemini: not found
grok: not found
agy: not found
opencode: not found
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo fmt --all -- --check
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-mcp)
    Checking designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 21.44s
```

**`cargo test --workspace --locked --lib` (`r2-workspace-lib.log`):**
```text
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test --workspace --locked --lib
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 653 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.40s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 134 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.20s
test result: ok. 121 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 40.90s
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.38s
test result: ok. 224 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.63s
test result: ok. 55 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 33.27s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
```

**`cargo test -p designos-app --bin designos --locked` (`r2-bin-designos.log`):**
```text
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked
running 2988 tests
...
test result: ok. 2980 passed; 0 failed; 8 ignored; 0 measured; 0 filtered out; finished in 197.25s
```

**Python Gates (`r2-python-gates.log`):**
```text
$ python3 scripts/zero_fake_ui_gate.py
------------------------------------------------------------------------------------------------------------
 Gate Verdict: PASS (17/17 invariants satisfied)
============================================================================================================
$ python3 scripts/check_spacing_relationships.py
literal geometry outside theme.rs: offset_literals=1675, inset_literals=45, radius_literals=260, font_literals=25, nonconcentric_pairs=2
note: 11 entries are below baseline; run --write-baseline to lock the gain in
PASS: no count rose above the baseline
```

---

#### Acceptance 4 — Manual CLI Runs Exiting 2 & Writing Nothing (`r2-manual.log`)

**Commands & Output:**
```text
=== Manual 1: --apply-spec with /tmp/no-marker ===
Before hash:
696498419d135377e53197baf34f8140c09b768dc45bdd637b21bcff534ae413  /tmp/r2-manual.qTlRQm/copy.designos
designos: hermetic agent mode denied: --screenshot is required; DESIGNOS_AGENT_CLI_DIR (/tmp/no-marker) is missing .designos-hermetic; DESIGNOS_LIBRARY is unset; TMPDIR (/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/) is missing .designos-hermetic; HOME (/Users/jangtrinh) is missing .designos-hermetic
Exit code: 2
After hash:
696498419d135377e53197baf34f8140c09b768dc45bdd637b21bcff534ae413  /tmp/r2-manual.qTlRQm/copy.designos

=== Manual 2: --seed-library with /tmp/no-marker ===
designos: hermetic agent mode denied: --screenshot is required; DESIGNOS_AGENT_CLI_DIR (/tmp/no-marker) is missing .designos-hermetic; DESIGNOS_LIBRARY is unset; TMPDIR (/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/) is missing .designos-hermetic; HOME (/Users/jangtrinh) is missing .designos-hermetic
Exit code: 2
Seed dir entries count: 0

=== Manual 3: relative DESIGNOS_AGENT_CLI_DIR=stubs ===
designos: hermetic agent mode denied: DESIGNOS_AGENT_CLI_DIR (stubs) must be an absolute path; DESIGNOS_LIBRARY is unset; TMPDIR (/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/) is missing .designos-hermetic; HOME (/Users/jangtrinh) is missing .designos-hermetic
Exit code: 2
Shot exists: no
```

---

#### Acceptance 5 — Real-CLI Check With Normal `PATH` (`r2-real-cli-check.log`)

**Commands & Output:**
```text
=== Developer Mac Real-CLI Check (normal PATH) ===
claude -> /opt/homebrew/bin/claude
codex -> /opt/homebrew/bin/codex
gemini -> /opt/homebrew/bin/gemini
grok -> /opt/homebrew/bin/grok
agy -> /opt/homebrew/bin/agy
opencode -> /Users/jangtrinh/.opencode/bin/opencode

running 17 tests
test cli_resolve::tests::hermetic_startup_refuses_relative_or_dotdot_dir ... ok
test cli_resolve::tests::hermetic_mode_refuses_symlink_outside_stub_dir ... ok
test cli_resolve::tests::search_dirs_precedence_is_hermetic_scoped_explicit_host ... ok
test cli_resolve::tests::hermetic_mode_refuses_background_redirect_and_unexpanded_vars ... ok
test cli_resolve::tests::every_resolver_uses_resolve_in ... ok
test cli_resolve::tests::hermetic_mode_refuses_compound_or_unknown_commands ... ok
test cli_resolve::tests::version_warmer_resolves_on_the_calling_thread ... ok
test cli_resolve::tests::scoped_guard_applies_only_inside_a_gate_case ... ok
test cli_resolve::tests::hermetic_mode_is_denied_without_every_requirement ... ok
test cli_resolve::tests::hermetic_mode_writes_the_acknowledgement ... ok
test cli_resolve::tests::startup_check_runs_before_seed_library_and_agent_cli ... ok
test cli_resolve::tests::hermetic_pty_env_is_stub_path_only ... ok
test cli_resolve::tests::hermetic_shell_cmd_has_no_login_or_interactive_flags ... ok
test cli_resolve::tests::grok_usage_caller_with_no_grok_stub_spawns_nothing ... ok
test cli_resolve::tests::stub_scripts_behave ... ok
test cli_resolve::tests::hermetic_resolver_census ... ok
test cli_resolve::tests::non_hermetic_catalog_path_prepends_install_and_extra_before_host_path ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 2971 filtered out; finished in 1.20s

=== .designos-hermetic-active ===
pid=85202
dir=/private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/r2-realcli.8JgCIT/stubs
=== Stub invocation log under normal PATH ===
/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app/tests/layout-baseline/stubs/claude --check-invocation
/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app/tests/layout-baseline/stubs/codex --check-invocation
/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app/tests/layout-baseline/stubs/agy --check-invocation
/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app/tests/layout-baseline/stubs/grok --check-invocation
/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app/tests/layout-baseline/stubs/gemini --check-invocation
/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app/tests/layout-baseline/stubs/opencode --check-invocation
/private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/r2-realcli.8JgCIT/stubs/agy --mcp-config /nonexistent/mcp.json
```

### 5. Unresolved Questions

- None.

Status: GREEN — all Round 2 hermetic-mode escapes closed, clippy exit 0, all gates verified.
