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

---

## Round 3

### 1. What Could NOT Be Verified (Stated First)

- **Section 12 Steps 8–9 (`scripts/layout_baseline_capture.py`, Tier B PNG/wireframe run) and the `*.layout.json` / `cases.json.bless_sha` bless were NOT run or committed in Round 3 (`FACT`).** By explicit controller ruling in the Round 3 task prompt, Steps 8–9 and the baseline bless stay in Round 4.
- **Nothing in Round 3's scope (Part 1's two hermetic review fixes + Part 2's Section 12 Steps 4–7 and Section 10 tests) was left unverified (`FACT`).** Windows/Linux CI targets run on GitHub Actions rather than locally on this macOS (`aarch64-apple-darwin`) worker.

### 2. Conclusion & Epistemic Ledger (`FACT` / `INFERENCE` / `ASSUMPTION`)

- **Conclusion (`FACT`):** Both Round 2 Grok review findings (Part 1) and Spec L Section 12 Steps 4–7 (Part 2) are implemented with red-before-green commit discipline (`plans/reports/agy-1126-layout-gate-evidence/r3-red.log` and `r3-green.log`) and verified green across all workspace and binary test suites (`3,008 passed; 0 failed` in `designos` bin, `0` clippy warnings, `17/17` zero-fake UI gate).
- **Epistemic Ledger:**
  - `FACT` — Merging `origin/main` (`5ef7cf189`, bringing `#1203` picker tooltip) into `5d8c7d1d` produced clean merge commit `9a05cccf6`.
  - `FACT` — Part 1 Finding 1 (Major): `tab_designos_env_map` (`crates/designos-app/src/terminal.rs:4496-4557`) now builds the tab's `DESIGNOS_*` map in the exact child-PTY precedence order: parent `DESIGNOS_*` first, then forced `DESIGNOS_MCP_BIN` (`designos_mcp_binary()`), `DESIGNOS_WORKSPACE`, `DESIGNOS_FILE`, and default session variables (`DESIGNOS_LIVE_CANVAS=1`, `DESIGNOS_LIVE_DOC_SYNC=1`, `DESIGNOS_LIVE_DOC_PATCH=1`, `DESIGNOS_AGENT_RUNTIME`), and finally the live `session.env_vars()` overlay. `configure_pty_env_with_session_and_hermetic` (`terminal.rs:5093-5119`) iterates `tab_designos_env_map` directly so child PTY env and hermetic `$DESIGNOS_*` argv expansion share one source of truth and cannot drift. Verified red (`25e5f5c59`) -> green (`88490f092`) by `cli_resolve::tests::hermetic_codex_expansion_uses_tab_env_not_parent_env`.
  - `FACT` — Part 1 Finding 2 (Minor): `HERMETIC_DIR: OnceLock<Option<PathBuf>>` (`crates/designos-app/src/cli_resolve.rs:28`) stores the canonical `PathBuf` validated during `enforce_hermetic_startup_os` (marker `.designos-hermetic` included). `hermetic_agent_cli_dir()` returns the pinned path without re-reading or re-canonicalizing `DESIGNOS_AGENT_CLI_DIR`, `resolve_in_hermetic` requires any passed `dir` to match the pinned path when `HERMETIC_DIR` is active (or satisfy `validate_and_canonicalize_hermetic_dir`), the `or_else(canonicalize)` fallback in `terminal.rs` is deleted so an unresolvable/retargeted dir fails with a clear error, and PTY `PATH` uses the pinned canonical dir directly. Verified red (`25e5f5c59`) -> green (`88490f092`) by `cli_resolve::tests::hermetic_startup_dir_is_pinned_after_symlink_retarget`.
  - `FACT` — Part 2 Step 4: Added `crates/designos-app/src/layout_snapshot.rs` (`LayoutSnapshot`, `RegionSnapshot`, `Rect4`, `ScrollState`, `WidgetEntry`, `AllowList`, `AllowEntry`, `AnchorX`, `AnchorY`, `CompareReport`, `capture`, `parse_allow_list`, `validate_allow_entries`, `compare`, `check_stale_allow_entries`, `compare_with_stale_check`, `load_and_compare_baseline`) and converted `widgets::record` / `widgets::record_with` (`crates/designos-app/src/widgets/mod.rs`) to transform layer-local rects to global viewport coordinates via `ui.ctx().layer_transform_to_global(ui.layer_id())`. Verified red (`44427cc8b`) -> green (`54cb5018`) across all 12 unit tests (`1`, `3`–`7`, `4b`–`4e`, `6b`, `6c`).
  - `FACT` — Part 2 Step 5: Added `showcase::seed_library_at(root, modified)` (`crates/designos-app/src/showcase/mod.rs:94-138`) with `T_BASE = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000)` and 4 seeded projects (`T_BASE - 120s`, `- 3_600s`, `- 86_400s`, `- 259_200s`), added `Home::fixed_now: Option<i64>` and `Home::now_ms(&self)` (`crates/designos-app/src/home/mod.rs:288-295`) wired across all 7 `wall_ms()` display sites in `home/mod.rs` and `home/main_view.rs`, and added the Tier A `opts.agent_cli_dir` seam + state-driven `agents-chat` (`shell.rs:151-169, 229-235, 331-335, 399-402`). Verified red (`3a053adf`) -> green (`18770487`) by tests `8c` (`agent_cases_do_not_depend_on_installed_clis`) and `8d` (`home_is_deterministic_with_fixed_clock`).
  - `FACT` — Part 2 Step 6: Added `crates/designos-app/src/shell/tests/layout_unchanged/{mod.rs,d.rs..k.rs}`, `crates/designos-app/tests/layout-baseline/cases.json` (74 headless cases + 2 kit cases, no `bless_sha` in Round 3), and `crates/designos-app/tests/layout-baseline/allow.json` (`[]`). When `cases.json.bless_sha` is absent (pre-bless in Round 3), each `layout_unchanged_<spec>` test runs every case owned by that spec through `Sim::from_cli_with_stubs`, captures its `LayoutSnapshot`, and verifies structural invariants plus self-comparison under `G1`–`G5`. Verified red (`3e8ac83b`) -> green (`694d3467`) across all 9 `layout_unchanged` tests.
  - `FACT` — Part 2 Step 7: Added `--layout-json OUT.json` (`Options::layout_json: Option<PathBuf>`, `cli.rs:109, 341-346, 353-355, 384`) requiring `--screenshot`, enabled `layout_snapshot::RECORDING` during `Shot` frames (`shell.rs:468`), and wrote the JSON snapshot immediately after the PNG write in `drive_screenshot` (`shell.rs:521-541`). Verified red (`7ab958e1`) -> green (`5e8790ac`) by tests `8` (`layout_json_requires_screenshot`) and `8b` (`layout_json_matches_headless_snapshot`).
  - **Weakest link (one line):** Until Round 4 commits the blessed `*.layout.json` files and `cases.json.bless_sha`, `layout_unchanged_d..k` verify live capture + self-comparison (`compare(&snap, &snap, ...)`) rather than cross-commit baseline diffing (which is locked by unit test `7` `missing_baseline_fails` on `load_and_compare_baseline`).

### 3. Skills Loaded & Changes Made Because of Them

- `es:astra-work` & `es:fable-thinking`: Re-anchored every file/line citation against `9a05cccf6` after merging `origin/main` (`#1203`), maintained an explicit `FACT`/`INFERENCE`/`ASSUMPTION` ledger, stated `PREDICT` before every test/gate run, and verified all acceptance checks on the final code commit (`5e8790ac`).
- `es:orca-native-orchestration`: Checked coordinator inbox via `orca orchestration check --terminal term_42306ff4-b832-4e39-8751-792d2125bfd7 --json` at natural checkpoints and sent heartbeats with both `--task-id` and `--dispatch-id`.
- `es:dev` / `es:feature-dev`: Enforced strict Red -> Green commit pairs for Part 1 and each of Steps 4, 5, 6, and 7.
- `es:rust`: Used `OnceLock<Option<PathBuf>>` for zero-reallocation startup dir pinning, `is_none_or` and `const { assert!(...) }` for clean Rust 2024 / Clippy compliance, and kept `RECORDING` thread-local overhead zero when disabled.
- `es:rust-gpu-canvas`: Kept `designos-gpu` and `designos-render` completely untouched; hooked `--layout-json` into `shell::drive_screenshot` on the exact `Event::Screenshot` frame so Tier A headless `Sim` and Tier B `--screenshot` capture identical 2-frame geometry.

### 4. Round-3 Drift Table (Spec L Citations Re-Anchored Against `9a05cccf6` / `5e8790ac`)

| Spec L Citation | Spec L Section / Step | Actual Location on Branch (`9a05cccf6` -> `5e8790ac`) | Nature of Drift & Impact |
|---|---|---|---|
| `crates/designos-app/src/cli.rs:5-103` (`Options`), `:141-199` (`PANELS`), `:228-340` (`parse`), `:354-372` (`USAGE`) | Step 7 (`--layout-json`) | `Options` at `:5-110`, `PANELS` (`[&str; 58]`, `#1203` added `"fill-picker-libraries"`) at `:144-203`, `parse` at `:231-356`, `USAGE` at `:366-385` | Plain line shift + `#1203` added `"fill-picker-libraries"` to `PANELS`. Non-breaking; `--layout-json` added cleanly. |
| `crates/designos-app/src/shell.rs:149` (`Shell::build`), `:207-212` (`opts.home`), `:303-405` (`match opts.panel.as_deref()`), `:422-500` (`Shot` / `drive_screenshot`) | Steps 5 & 7 | `Shell::build` at `:149`, Tier A seam at `:151-169`, `opts.home` + `fixed_now` at `:229-235`, `match opts.panel.as_deref()` at `:325-431` (`agents-chat` at `:331-335`, `version-history` at `:399-402`), `Shot` / `drive_screenshot` at `:444-545` | Plain line shift from Step 5 additions + `#1203` arm (`"fill-picker-libraries"` at `:423-428`). Non-breaking. |
| `crates/designos-app/src/home/mod.rs:1451, 1651, 1900, 2232, 2673` & `main_view.rs:203, 496` (`wall_ms()`) | Step 5 (`Home::now_ms`) | `home/mod.rs:288-295` (`now_ms`), display sites at `home/mod.rs:1463, 1663, 1912, 2244, 2685` and `home/main_view.rs:203, 496` | Exact match on `main_view.rs`; +12 line shift in `home/mod.rs` from adding `fixed_now` field and `now_ms(&self)`. |
| `crates/designos-app/src/showcase/mod.rs:87-128` (`seed_library`) | Step 5 (`seed_library_at`) | `seed_library` at `:87-91` delegates to `seed_library_at(root, None)` at `:94-138` | Exact line anchor `:87`. Non-breaking refactor. |
| `crates/designos-app/src/terminal.rs:4496-4528` (`tab_designos_env_map`), `:5085-5097` (`configure_pty_env_with_session_and_hermetic`) | Part 1 Findings 1 & 2 | `tab_designos_env_map` at `:4496-4557`; `configure_pty_env_with_session_and_hermetic` at `:5093-5119` | Unified into single precedence helper + pinned canonical `PathBuf`. |
| `crates/designos-app/src/shell/tests.rs:37-105` (`Sim`), `:1166` (module tail) | Steps 5 & 6 | `Sim` at `:37-115`; `#[cfg(unix)] mod layout_unchanged;` registered at `:1168` | Plain line shift (+2 lines from `#1203` test). |

### 5. Red -> Green Commit Pairs & Acceptance Verification

| Step / Scope | Red Commit | Green Commit | Tests Verified (Red -> Green) |
|---|---|---|---|
| **Merge `origin/main`** | — | `9a05cccf6` | Merged `origin/main` (`5ef7cf189`, `#1203`) into `5d8c7d1d` |
| **Part 1 (Hermetic fixes 1 & 2)** | `25e5f5c59` | `88490f092` | `hermetic_codex_expansion_uses_tab_env_not_parent_env`, `hermetic_startup_dir_is_pinned_after_symlink_retarget` (`0/2` -> `19/19`) |
| **Part 2 Step 4 (`layout_snapshot.rs` + `G1`–`G5`)** | `44427cc8b` | `54cb5018` | Unit tests `1`, `3`–`7`, `4b`–`4e`, `6b`, `6c` (`0/12` -> `12/12`) |
| **Part 2 Step 5 (Tier A seams + fixed clock)** | `3a053adf` | `18770487` | Unit tests `8c` (`agent_cases_do_not_depend_on_installed_clis`), `8d` (`home_is_deterministic_with_fixed_clock`) (`0/2` -> `2/2`) |
| **Part 2 Step 6 (`layout_unchanged` + `cases.json` + `allow.json`)** | `3e8ac83b` | `694d3467` | `cases_json_and_allow_json_match_spec_l_contract` + `layout_unchanged_d..k` (`0/9` -> `9/9`) |
| **Part 2 Step 7 (`--layout-json` CLI + screenshot hook)** | `7ab958e1` | `5e8790ac` | Unit tests `8` (`layout_json_requires_screenshot`), `8b` (`layout_json_matches_headless_snapshot`) (`0/2` -> `2/2`) |

---

#### Acceptance 1 — Red (`r3-red.log`) & Green (`r3-green.log`) Summaries

Full logs are committed at `plans/reports/agy-1126-layout-gate-evidence/r3-red.log` and `plans/reports/agy-1126-layout-gate-evidence/r3-green.log`.

```text
=== r3-red.log summary ===
Part 1 (25e5f5c59): test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 2989 filtered out; finished in 0.01s
  - cli_resolve::tests::hermetic_codex_expansion_uses_tab_env_not_parent_env ... FAILED
  - cli_resolve::tests::hermetic_startup_dir_is_pinned_after_symlink_retarget ... FAILED
Step 4 (44427cc8b): test result: FAILED. 0 passed; 12 failed; 0 ignored; 0 measured; 2991 filtered out; finished in 0.31s
  - layout_snapshot::tests::{centred_bar_widening_by_12_passes_with_anchor_x_centre, cumulative_shift_is_derived_not_a_range, equally_specific_entries_fail_to_parse, growing_a_row_without_allow_fails_and_with_allow_passes, inspector_overflow_does_not_fail_g2, missing_baseline_fails, palette_growing_above_bottom_anchor_passes_with_allow, region_wide_wildcard_entry_is_rejected, reordering_two_buttons_fails_g3, snapshot_reports_every_region_of_the_editor, stale_allow_entry_fails, widening_the_left_panel_fails_g1} ... FAILED
Step 5 (3a053adf):  test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 3003 filtered out; finished in 1.38s
  - layout_snapshot::tests::agent_cases_do_not_depend_on_installed_clis ... FAILED
  - layout_snapshot::tests::home_is_deterministic_with_fixed_clock ... FAILED
Step 6 (3e8ac83b):  test result: FAILED. 0 passed; 9 failed; 0 ignored; 0 measured; 3005 filtered out; finished in 0.00s
  - shell::tests::layout_unchanged::{cases_json_and_allow_json_match_spec_l_contract, d..k} ... FAILED
Step 7 (7ab958e1):  test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 3014 filtered out; finished in 0.54s
  - layout_snapshot::tests::layout_json_requires_screenshot ... FAILED
  - layout_snapshot::tests::layout_json_matches_headless_snapshot ... FAILED

=== r3-green.log summary ===
Part 1 (88490f092): test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 2972 filtered out; finished in 0.33s
Step 4 (54cb5018):  test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 2991 filtered out; finished in 0.23s
Step 5 (18770487):  test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 3003 filtered out; finished in 32.71s
Step 6 (694d3467):  test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 3005 filtered out; finished in 6.58s
Step 7 (5e8790ac):  test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 3014 filtered out; finished in 0.55s
```

---

#### Acceptance 2 & 3 — Clean `PATH` Proof + Full Mandatory Test Harness on `5e8790ac`

```text
$ export PATH="/usr/bin:/bin:/usr/sbin:/sbin:$HOME/.cargo/bin"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
$ export CARGO_TARGET_DIR="$PWD/out/cargo"
$ for c in claude codex gemini grok agy opencode; do command -v $c; done
(empty output — exit 1 on all 6 agent CLIs)

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo fmt --all -- --check
(exit 0)

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
    Checking designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.64s

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test --workspace --locked --lib
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 653 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.46s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 134 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.14s
test result: ok. 121 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 36.12s
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.41s
test result: ok. 224 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.83s
test result: ok. 55 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 32.16s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked
running 3016 tests
...
test result: ok. 3008 passed; 0 failed; 8 ignored; 0 measured; 0 filtered out; finished in 212.30s

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

#### Acceptance 4 — `git diff origin/main -- Cargo.lock`

```diff
$ git diff origin/main -- Cargo.lock
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

#### Acceptance 5 — Real-CLI Check Once With Normal `PATH` (`DESIGNOS_STUB_LOG="$PWD/out/invocations.log"`)

```text
$ DESIGNOS_STUB_LOG="$PWD/out/invocations.log" CARGO_TARGET_DIR="$PWD/out/cargo" cargo test -p designos-app --bin designos --locked layout_unchanged
running 9 tests
test shell::tests::layout_unchanged::cases_json_and_allow_json_match_spec_l_contract ... ok
test shell::tests::layout_unchanged::e::layout_unchanged_e ... ok
test shell::tests::layout_unchanged::h::layout_unchanged_h ... ok
test shell::tests::layout_unchanged::k::layout_unchanged_k ... ok
test shell::tests::layout_unchanged::i::layout_unchanged_i ... ok
test shell::tests::layout_unchanged::d::layout_unchanged_d ... ok
test shell::tests::layout_unchanged::g::layout_unchanged_g ... ok
test shell::tests::layout_unchanged::j::layout_unchanged_j ... ok
test shell::tests::layout_unchanged::f::layout_unchanged_f ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 3007 filtered out; finished in 5.83s

$ cat "$PWD/out/invocations.log"
/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app/tests/layout-baseline/stubs/agy --mcp-config /nonexistent/mcp.json
```

### 6. Unresolved Questions

- None.

Status: DONE — Round 3 Part 1 (both hermetic fixes) and Part 2 (Spec L Section 12 Steps 4–7) implemented and verified green (3008/0 in designos bin, 0 clippy warnings, 17/17 zero-fake UI gate).

---

## Round 4

### 1. What Could NOT Be Verified (Stated First)

- **Section 12 Steps 8–9 (`scripts/layout_baseline_capture.py`, Tier B PNG/wireframe run) and the `*.layout.json` / `cases.json.bless_sha` bless were NOT run or committed in Round 4 (`FACT`).** By explicit controller ruling in the Round 4 task prompt, Round 4 fixes the 12 comparer/fixture review findings only; Steps 8–9 and the baseline bless move to Round 5.
- **Nothing in Round 4's scope (all 12 findings from Grok's Round 3 review checked against the canonical PM blob `origin/pm/spec-ui-revamp:plans/specs/ui-revamp/detail/1126-layout-unchanged-gate-baseline-capture.md`) was left unverified (`FACT`).** Windows/Linux CI targets run on GitHub Actions rather than locally on this macOS (`aarch64-apple-darwin`) worker.

### 2. Conclusion & Epistemic Ledger (`FACT` / `INFERENCE` / `ASSUMPTION`)

- **Conclusion (`FACT`):** All 11 actionable findings from Grok's Round 3 review (`review-1126-pr1206-r3-grok.md`) have been checked against the canonical PM blob (`origin/pm/spec-ui-revamp:plans/specs/ui-revamp/detail/1126-layout-unchanged-gate-baseline-capture.md`, sections 3, 5, and 10), implemented with red-before-green commit discipline (`plans/reports/agy-1126-layout-gate-evidence/r4-red.log` and `r4-green.log`), and verified green across all workspace and binary test suites (`1,281 passed; 0 failed` in `--workspace --lib`, `3,019 passed; 0 failed` in `designos` bin, `0` clippy warnings, `0` fmt diffs, `17/17` zero-fake UI gate, `0` spacing regressions).
- **Epistemic Ledger:**
  - `FACT` — Merging `origin/main` (`cb69322fa`, bringing `#1204` canvas move state) into `c9844487` produced clean merge commit `1c99d2d70`.
  - `FACT` — Finding 1 (Major 1): PM blob §3.3 rule 8 (`h_base`) defines row-box clustering (`|y_a - y_b| <= 2.0`, bounding box height `[h_base - 1, h_base + 2]`) over all widgets in the region and requires applying the cumulative `dh` sum (`sum(max_y(curr_row) - max_y(base_row))`) to widgets on rows below. Previously `expected_y` only included `cum_dh` inside the named-widget `G4` branch (`layout_snapshot.rs:647`). Both named and unnamed `G4` paths now compute `row_cum_dh(base_r[1], &row_shifts)` (`layout_snapshot.rs:762-846`) and check unnamed widgets against `expected_y = base_r[1] + cum_dh + gap_shift` (plus any covering `allow.json` entry). Verified red (`db075f221`) -> green (`fd3c115ed`) by `layout_snapshot::tests::unnamed_rows_shift_by_cumulative_row_box_dh`.
  - `FACT` — Finding 2 (Major 2): PM blob §3.2 (`gap_dy`) specifies allowed change in vertical gap *between* adjacent rows in the covered prefix (`expected_y += gaps_above * gap_dy` with `0.5pt` tolerance). Previously `gap_dy.abs()` was added to `tol_y`. Now `gaps_above = count_row_gaps_above(base_r[1], &row_shifts)` counts distinct row boxes strictly above the widget's row, shifts `expected_y` by `gaps_above as f32 * gap_dy`, and keeps `tol_y = 0.5` (`layout_snapshot.rs:782-826`). Verified red (`073eb69cc`) -> green (`bc5fc8c1a`) by `layout_snapshot::tests::gap_dy_shifts_expected_y_per_row_gap_with_half_pt_tolerance`.
  - `FACT` — Finding 3 (Major 3): PM blob §3.1 (`G3 Inventory`) and §3.2 (`inventory_max_delta`) state that `inventory_max_delta` forgives only a virtualized widget count delta (`|curr_n - base_n| <= max_delta`) while still comparing the shared prefix (`leading min(n)`). Previously `compare_region` returned early after checking `count_delta <= max_delta`. Now `compare_region` records the count-delta check and continues through `G3` and `G4` on the shared prefix `0..base_reg.widgets.len().min(curr_reg.widgets.len())` (`layout_snapshot.rs:584-628, 725-757`). Verified red (`12354bceb`) -> green (`20fc39d3e`) by `layout_snapshot::tests::inventory_max_delta_forgives_count_only_and_compares_shared_prefix`.
  - `FACT` — Finding 4 (Major 4): PM blob §3.3 rule 9 states an `allow.json` entry is stale only when it matched zero widgets across the whole test run (`"*"` is stale only when no case in the run matched it), and an entry covers a widget whenever `best_allow_entry` selects it for a widget (or region in `G1`) that exists in the snapshot, even if its current delta is `0`. Updated `compare_region` (`layout_snapshot.rs:480-482, 766-768, 829-831`) to record `matched_allow_indices` whenever `best_allow_entry` covers an existing region/widget, added `check_stale_allow_entries_for_run(cases_in_run, allow, matched_across_run)` (`layout_snapshot.rs:413-444`), and wired `shell/tests/layout_unchanged/mod.rs` to check staleness over the union of cases run by the spec suite. Verified red (`6cbc2cede`) -> green (`6393cc0f4`) by `layout_snapshot::tests::stale_check_counts_covering_entries_and_runs_over_union_of_run`.
  - `FACT` — Finding 5 (Minor 5): PM blob §3 (`regions.overlays`) specifies `overlay:{order}/{index}` ordered by `ctx.memory(|m| m.areas().order())`, omitting `Order::Background`, `Order::Debug`, zero-sized rects, and internal egui `Id` hashes. In `egui 0.35.0`, `Areas::order()` is `pub(crate)` while `Memory::layer_ids()` is the public iterator over `self.areas.order()`. Updated `capture` (`layout_snapshot.rs:256-289`) to iterate `m.layer_ids()` with per-`Order` counters (`overlay:Middle/0`, `overlay:Foreground/0`, …) and updated `G1` overlay comparison (`layout_snapshot.rs:536-573`) to compare ordered `(name, rect)` pairs directly. Verified red (`1b3fba3da`) -> green (`ae30d657e`) by `layout_snapshot::tests::overlay_names_use_areas_order_and_omit_egui_id`.
  - `FACT` — Finding 6 (Minor 6): PM blob §3 (`RegionSnapshot.scroll`) specifies detecting scrollbars via `WidgetType::ScrollBar` with axis `"y"` (`h > w`) / `"x"` (`w >= h`) and `maybe: false` when `widget_info` is enabled, and falling back to narrow `Sense::DRAG` bars matching `spacing.scroll.bar_width` with `maybe: true` when `widget_info` is absent on the release `--layout-json` path (`layout_snapshot.rs:198-253`). Verified red (`d2de4a508`) -> green (`9dd077692`) by `layout_snapshot::tests::scroll_detection_uses_widget_type_scrollbar_and_maybe_in_release`.
  - `FACT` — Finding 7 (Minor 7): PM blob §3.1 (`G1 Region geometry`) keeps region `x` and `w` exact (`<= 0.5pt`) unless `allow.json` explicitly covers `x`/`w`, while running region `y` and `h` through `check_rect_with_allow` when an `allow.json` entry with `name_prefix: None` covers `(case, region)` (`layout_snapshot.rs:475-534`). Verified red (`e8087c33b`) -> green (`133a181c3`) by `layout_snapshot::tests::g1_allows_covered_region_y_and_h_while_keeping_x_and_w_exact`.
  - `FACT` — Finding 8 (Minor 8): PM blob §10 Test `8c` (`agent_cases_do_not_depend_on_installed_clis`) requires running all 6 `agents-*` cases twice under `ScopedCliDirs`: once with an empty host `PATH=""` and once with fake `claude` and `codex` scripts (`exit 99`) on `PATH`, asserting identical `LayoutSnapshot`s and that `DESIGNOS_STUB_LOG` records only paths inside the stub directory. Rewrote `8c` (`layout_snapshot.rs:1330-1431`) to run each `PATH` setup in an isolated child test process (`DESIGNOS_TEST_8C_CHILD=empty|fake`) so `std::env::set_var("PATH", ...)` never races other threads, and verified after Minor 9's fix that all 6 `agents-*` cases run in `~0.98s` total. Committed at `2a651d6b1`.
  - `FACT` — Finding 9 (Minor 9): Removed the 50 × 100 ms (`5s`) `std::thread::sleep` loop in `open_failed_start_fixture` (`crates/designos-app/src/terminal.rs:5840-5859`) per PM blob §11 and §3.3 rule 4; `Sim::from_cli_with_stubs`'s 2 s `is_alive` poll (`shell/tests.rs:103-112`) is the single wait. Verified red (`e44df6c69`, took `5.09s`) -> green (`872db2b1c`, took `0.10s`) by `layout_snapshot::tests::open_failed_start_fixture_does_not_block_waiting_for_exit`.
  - `FACT` — Finding 10 (Nit 10): PM blob §2 Decision 9 and §3.2 (`min_dw` / `min_dh`) state that negative `dw` or `dh` is only legal when `reason` cites rule `R5` (intentional removal of dead padding). Added validation in `validate_allow_entries` (`layout_snapshot.rs:342-357`). Verified red (`1cb899eee`) -> green (`e0244c8e9`) by `layout_snapshot::tests::negative_dw_or_dh_requires_r5_in_reason`.
  - `FACT` — Finding 11 (Nit 11): Per controller instruction in the Round 4 task prompt, `no change` (`--layout-json` overwriting the path it was given matches the PNG save).
  - `FACT` — Finding 12 (Nit 12): Checked PM blob §3.1 (`G3 Inventory and reading order`) against Grok's review: the PM blob explicitly states *"Cluster widgets into rows by `|y_a - y_b| <= 2.0`, sort each row by `x`, and compare the sequence of `name` values (using `"_"` for unnamed widgets) per row."* Implemented `row_name_sequences` (`layout_snapshot.rs:703-759`) to enforce per-row `x`-sorted `name.unwrap_or("_")` sequence equality in `G3`. Verified red (`fa69c29cd`) -> green (`343bd6d8f`) by `layout_snapshot::tests::g3_checks_row_name_sequence_with_underscore_for_unnamed`.
  - **Weakest link (one line):** Until Round 5 runs `scripts/layout_baseline_capture.py` and commits the blessed `*.layout.json` files and `cases.json.bless_sha`, `layout_unchanged_d..k` verify live capture + self-comparison (`compare(&snap, &snap, ...)`) rather than cross-commit baseline diffing.

### 3. Skills Loaded & Changes Made Because of Them

- `es:astra-work` & `es:fable-thinking`: Verified all 12 findings against the canonical PM blob (`origin/pm/spec-ui-revamp:plans/specs/ui-revamp/detail/1126-layout-unchanged-gate-baseline-capture.md`) before writing tests or code, stated `PREDICT` before every test/gate run, isolated Test `8c`'s `PATH` mutation into child processes so parallel tests cannot race on process environment, and ran the full acceptance suite on the final code commit (`343bd6d8f`).
- `es:orca-native-orchestration`: Checked coordinator inbox via `orca orchestration check --terminal term_d34859f1-22a9-4304-a9b1-ccf9f9039391 --json` at natural checkpoints and sent heartbeats every 5 minutes with both `--task-id task_3e73174a163c` and `--dispatch-id ctx_770f92a4f012`.
- `es:dev` / `es:feature-dev`: Enforced strict Red (`test(app): …`) -> Green (`fix(app): …`) commit pairs for every fixed finding (`r4-red.log` and `r4-green.log`).
- `es:rust`: Extracted `type RectPair = ([f32; 4], [f32; 4]);` to satisfy `clippy::type_complexity` under `-D warnings`, used `egui::Memory::layer_ids()` and `ctx.global_style()` cleanly in `egui 0.35.0`, and removed the 5 s blocking sleep loop from `open_failed_start_fixture`.
- `es:rust-gpu-canvas`: Kept `designos-gpu` and `designos-render` untouched while preserving identical 2-frame snapshot capture across Tier A headless `Sim` and Tier B `--screenshot --layout-json`.

### 4. Round 4 Findings Table (All 12 Findings Checked Against PM Blob)

| # | Severity | Finding Summary | PM Blob Check (`origin/pm/spec-ui-revamp:...`) | Status | Red Commit | Green Commit |
|---|---|---|---|---|---|---|
| **1** | Major 1 | Unnamed widgets never get cumulative row-box `dh` shift (`h_base`, rule 8) | Matches PM blob §3.3 rule 8: row boxes cluster all widgets in the region and shift subsequent rows (named and unnamed alike) by `sum(dh)`. | `fixed` | `db075f221` | `fd3c115ed` |
| **2** | Major 2 | `gap_dy` added to `tol_y` instead of `expected_y += gaps_above * gap_dy` with `0.5pt` tolerance | Matches PM blob §3.2: `gap_dy` is per-row-gap vertical shift (`expected_y += gaps_above * gap_dy`), tolerance stays `0.5pt`. | `fixed` | `073eb69cc` | `bc5fc8c1a` |
| **3** | Major 3 | `inventory_max_delta` skips every check in the region instead of comparing shared prefix `min(n)` | Matches PM blob §3.1 & §3.2: `inventory_max_delta` forgives count delta only; shared prefix `0..min(n)` still runs `G3` and `G4`. | `fixed` | `12354bceb` | `20fc39d3e` |
| **4** | Major 4 | Stale check scoped per case and misses 0-drift covered widgets | Matches PM blob §3.3 rule 9: an entry counts as matched when it covers an existing widget/region and is stale only when it matched nothing across the whole run. | `fixed` | `6cbc2cede` | `6393cc0f4` |
| **5** | Minor 5 | Overlay names include egui `Id` and sort lexicographically instead of `overlay:{order}/{index}` in `areas().order()` | Matches PM blob §3: `overlay:{order}/{index}` in `Memory::layer_ids()` (`areas().order()`) order with no egui `Id`. | `fixed` | `1b3fba3da` | `ae30d657e` |
| **6** | Minor 6 | Scroll detection uses `v`/`h` heuristics instead of `WidgetType::ScrollBar`, axis `y`/`x`, and `maybe: true` in release | Matches PM blob §3: uses `WidgetType::ScrollBar` (`axis: "y"\|"x"`, `maybe: false`) when widget info is present and narrow `Sense::DRAG` bar fallback (`maybe: true`) when absent. | `fixed` | `d2de4a508` | `9dd077692` |
| **7** | Minor 7 | `G1` checks region `y` and `h` strictly (`0.5pt`) even when covered by an `allow.json` entry | Matches PM blob §3.1: `G1` keeps `x` and `w` exact (unless `dx`/`dw` allowed) and runs `y`/`h` through allow bounds when a region-level entry covers `(case, region)`. | `fixed` | `e8087c33b` | `133a181c3` |
| **8** | Minor 8 | Test `8c` only runs once under current `PATH` instead of empty host `PATH` vs fake `claude`/`codex` on `PATH` + stub log check | Matches PM blob §10 Test `8c`: runs all 6 `agents-*` cases under `PATH=""` and `PATH="<fake_cli_dir>:..."` with `ScopedCliDirs`, asserting identical snapshots and stub-only `DESIGNOS_STUB_LOG`. | `fixed` | `e44df6c69` (with #9) | `2a651d6b1` |
| **9** | Minor 9 | `open_failed_start_fixture` (`--panel agents-start-failure`) has a 5 s sleep loop | Matches PM blob §11 & §3.3 rule 4: removed the 50 × 100 ms sleep loop in `terminal.rs`; `Sim`'s 2 s `is_alive` poll is the only wait. | `fixed` | `e44df6c69` | `872db2b1c` |
| **10** | Nit 10 | Negative `min_dw`/`max_dw`/`min_dh`/`max_dh` does not require `"R5"` in `reason` | Matches PM blob §2 Decision 9 & §3.2: negative `dw`/`dh` rejected by `validate_allow_entries` unless `reason` contains `"R5"`. | `fixed` | `1cb899eee` | `e0244c8e9` |
| **11** | Nit 11 | `--layout-json` overwrites output path (matches PNG save) | Controller ruling: `no change`. | `no change` | — | — |
| **12** | Nit 12 | `G3` only checks named widgets' relative order instead of per-row `x`-sorted name sequence with `"_"` for unnamed widgets | PM blob §3.1 explicitly requires clustering rows by `\|y_a - y_b\| <= 2.0`, sorting each row by `x`, and comparing the `name` sequence (with `"_"` for unnamed widgets) per row. | `fixed` | `fa69c29cd` | `343bd6d8f` |

---

### 5. Acceptance Verification (Run on Final Code Commit `343bd6d8f`)

#### Acceptance 1 — Findings Table
See Section 4 above.

#### Acceptance 2 — Red (`r4-red.log`) and Green (`r4-green.log`) Logs
Full logs are committed at `plans/reports/agy-1126-layout-gate-evidence/r4-red.log` and `plans/reports/agy-1126-layout-gate-evidence/r4-green.log`.

```text
=== r4-red.log (each test failing on its RED commit before its fix) ===
Finding 1 (db075f221): test layout_snapshot::tests::unnamed_rows_shift_by_cumulative_row_box_dh ... FAILED
  panicked at crates/designos-app/src/layout_snapshot.rs:909:60:
  unnamed rows shifting by cumulative row-box dh (+0, +8, +16) must pass G4: "G4 unnamed widget #2 rect drift in case f-inspector region 'right_panel': baseline=[1210.0, 90.0, 200.0, 24.0], current=[1210.0, 98.0, 200.0, 32.0]"
  test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3017 filtered out; finished in 0.01s

Finding 2 (073eb69cc): test layout_snapshot::tests::gap_dy_shifts_expected_y_per_row_gap_with_half_pt_tolerance ... FAILED
  panicked at crates/designos-app/src/layout_snapshot.rs:994:9:
  widget with 0 rows above it must not be allowed to move +8pt under gap_dy=8
  test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3018 filtered out; finished in 0.01s

Finding 3 (12354bceb): test layout_snapshot::tests::inventory_max_delta_forgives_count_only_and_compares_shared_prefix ... FAILED
  panicked at crates/designos-app/src/layout_snapshot.rs:1056:14:
  moving a widget in the shared prefix under inventory_max_delta must fail G4: CompareReport { new_names: [], scroll_changes: [], matched_allow_indices: {0} }
  test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3019 filtered out; finished in 0.01s

Finding 4 (6cbc2cede): test layout_snapshot::tests::stale_check_counts_covering_entries_and_runs_over_union_of_run ... FAILED
  panicked at crates/designos-app/src/layout_snapshot.rs:1086:14:
  covering entry for existing widgets with 0 drift must not be flagged as stale: "stale allow.json[0] entry (case='d-editor-default', region='right_panel', spec='F'): matched 0 widgets"
  test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3020 filtered out; finished in 0.00s

Finding 5 (1b3fba3da): test layout_snapshot::tests::overlay_names_use_areas_order_and_omit_egui_id ... FAILED
  panicked at crates/designos-app/src/layout_snapshot.rs:1195:9:
  assertion `left == right` failed: overlay names must follow areas().order() as overlay:{order}/{index} with no egui Id
    left: ["Foreground:Id::new(\"popup-fg-beta\")", "Foreground:Id::new(\"popup-fg-gamma\")", "Middle:Id::new(\"popup-mid-alpha\")"]
   right: ["overlay:Middle/0", "overlay:Foreground/0", "overlay:Foreground/1"]
  test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3021 filtered out; finished in 0.01s

Finding 6 (d2de4a508): test layout_snapshot::tests::scroll_detection_uses_widget_type_scrollbar_and_maybe_in_release ... FAILED
  panicked at crates/designos-app/src/layout_snapshot.rs:1565:9:
  assertion `left == right` failed: with widget_info enabled, only the overflowing ScrollArea in right_panel should emit ScrollGeom(y, maybe=false); got [ScrollGeom { region: "left_panel", axis: "v", maybe: false }]
  test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3022 filtered out; finished in 0.02s

Finding 7 (e8087c33b): test layout_snapshot::tests::g1_allows_covered_region_y_and_h_while_keeping_x_and_w_exact ... FAILED
  panicked at crates/designos-app/src/layout_snapshot.rs:1653:60:
  covered region y/h changes must pass G1: "G1 region mismatch in case c-title-bar on tab_bar.h: baseline=40, current=44"
  test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3023 filtered out; finished in 0.01s

Finding 9 (e44df6c69): test layout_snapshot::tests::open_failed_start_fixture_does_not_block_waiting_for_exit ... FAILED
  panicked at crates/designos-app/src/layout_snapshot.rs:1713:9:
  open_failed_start_fixture must not block in a 5s sleep loop; took 5.088045292s
  test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3024 filtered out; finished in 5.17s

Finding 10 (1cb899eee): test layout_snapshot::tests::negative_dw_or_dh_requires_r5_in_reason ... FAILED
  panicked at crates/designos-app/src/layout_snapshot.rs:1832:10:
  negative min_dh without R5 in reason must fail validate_allow_entries
  test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3025 filtered out; finished in 0.00s

Finding 12 (fa69c29cd): test layout_snapshot::tests::g3_checks_row_name_sequence_with_underscore_for_unnamed ... FAILED
  panicked at crates/designos-app/src/layout_snapshot.rs:1900:53:
  swapping named and unnamed widget order on a row must fail G3 Order: CompareReport { new_names: [], scroll_changes: [], matched_allow_indices: {0, 1} }
  test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3026 filtered out; finished in 0.01s

=== r4-green.log (each test passing on its GREEN commit after its fix) ===
Finding 1 (fd3c115ed):  test layout_snapshot::tests::unnamed_rows_shift_by_cumulative_row_box_dh ... ok (1 passed; 0 failed; finished in 0.01s)
Finding 2 (bc5fc8c1a):  test layout_snapshot::tests::gap_dy_shifts_expected_y_per_row_gap_with_half_pt_tolerance ... ok (1 passed; 0 failed; finished in 0.01s)
Finding 3 (20fc39d3e):  test layout_snapshot::tests::inventory_max_delta_forgives_count_only_and_compares_shared_prefix ... ok (1 passed; 0 failed; finished in 0.01s)
Finding 4 (6393cc0f4):  test layout_snapshot::tests::stale_check_counts_covering_entries_and_runs_over_union_of_run ... ok (1 passed; 0 failed; finished in 0.00s)
Finding 5 (ae30d657e):  test layout_snapshot::tests::overlay_names_use_areas_order_and_omit_egui_id ... ok (1 passed; 0 failed; finished in 0.01s)
Finding 6 (9dd077692):  test layout_snapshot::tests::scroll_detection_uses_widget_type_scrollbar_and_maybe_in_release ... ok (1 passed; 0 failed; finished in 0.01s)
Finding 7 (133a181c3):  test layout_snapshot::tests::g1_allows_covered_region_y_and_h_while_keeping_x_and_w_exact ... ok (1 passed; 0 failed; finished in 0.01s)
Finding 9 (872db2b1c):  test layout_snapshot::tests::open_failed_start_fixture_does_not_block_waiting_for_exit ... ok (1 passed; 0 failed; finished in 0.10s)
Finding 8 (2a651d6b1):  test layout_snapshot::tests::agent_cases_do_not_depend_on_installed_clis ... ok (1 passed; 0 failed; finished in 1.00s)
Finding 10 (e0244c8e9): test layout_snapshot::tests::negative_dw_or_dh_requires_r5_in_reason ... ok (1 passed; 0 failed; finished in 0.00s)
Finding 12 (343bd6d8f): test layout_snapshot::tests::g3_checks_row_name_sequence_with_underscore_for_unnamed ... ok (1 passed; 0 failed; finished in 0.01s)
```

#### Acceptance 3 & 4 — Clean `PATH` Proof + Full Mandatory Test Harness on `343bd6d8f`

```text
$ export PATH="/usr/bin:/bin:/usr/sbin:/sbin:$HOME/.cargo/bin"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
$ export CARGO_TARGET_DIR="$PWD/out/cargo"
$ for c in claude codex gemini grok agy opencode; do command -v $c; done
(empty output — 0 bytes in r4-clean-path.log)

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo fmt --all -- --check
(exit 0 — 0 bytes in r4-fmt.log)

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-mcp)
    Checking designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 11.66s

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test --workspace --locked --lib
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 653 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.67s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 134 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 121 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 57.46s
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.37s
test result: ok. 224 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.68s
test result: ok. 55 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 33.25s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked
running 3027 tests
...
test video::tests::posters ... ok
test widgets::menu::tests::scrolling_to_the_end_reveals_the_last_row_and_a_click_picks_it ... ok
test widgets::menu::tests::arrow_keys_keep_the_highlighted_row_in_view ... ok
test video::tests::garbage_is_not_a_video_and_never_panics ... ok
test widgets::menu::tests::rows_scrolled_out_of_the_panel_cannot_be_clicked ... ok
test terminal::tests::touched_layers_do_not_bleed_into_other_active_tab_turn ... ok
test terminal::tests::pty_injection_fixture_real_shell ... ok

test result: ok. 3019 passed; 0 failed; 8 ignored; 0 measured; 0 filtered out; finished in 202.54s

$ python3 scripts/zero_fake_ui_gate.py
------------------------------------------------------------------------------------------------------------
 Gate Verdict: PASS (17/17 invariants satisfied)
============================================================================================================

$ python3 scripts/check_spacing_relationships.py
literal geometry outside theme.rs: offset_literals=1675, inset_literals=45, radius_literals=260, font_literals=25, nonconcentric_pairs=2
note: 11 entries are below baseline; run --write-baseline to lock the gain in
PASS: no count rose above the baseline
```

### 6. Assumptions & Unresolved Questions

- **Assumptions:**
  - `ASSUMPTION` (verified against `egui 0.35.0` source): `egui::Memory::layer_ids()` yields the exact slice from `self.areas.order()` (`Areas::order` is `pub(crate)` in `egui 0.35.0`), so iterating `m.layer_ids()` satisfies the spec's `ctx.memory(|m| m.areas().order())` ordering contract.
  - `ASSUMPTION` (verified by `scroll_detection_uses_widget_type_scrollbar_and_maybe_in_release`): On desktop without a touchscreen, `egui::ScrollArea` sets `DragScroll::OnTouch` (`Sense::empty()`) on the viewport while the scrollbar handle itself carries `Sense::DRAG` with cross-axis equal to `spacing.scroll.bar_width`. Matching narrow `Sense::DRAG` rects against `bar_width` when `widget_info` is absent detects overflowing scrollbars in release `--layout-json` builds without false-positive hits on sliders or drag-value widgets.
- **Unresolved Questions:**
  - None.

Status: DONE — Round 4 comparer and fixture fixes (11 fixed, 1 no-change per controller ruling) verified red-to-green with 0 fmt diffs, 0 clippy warnings, 1,281/0 workspace lib tests, 3,019/0 designos bin tests, 17/17 zero-fake UI gate, and 0 spacing regressions.

---

## Round 5

### 1. What Could NOT Be Verified (Stated First)

- **Section 12 Steps 8–9 (`scripts/layout_baseline_capture.py`, Tier B PNG/wireframe run) and the `*.layout.json` / `cases.json.bless_sha` bless were NOT run or committed in Round 5 (`FACT`).** By explicit controller instruction in the Round 5 task prompt, Round 5 fixes the Round 4 Grok review findings only; Section 12 Steps 8–9 and the baseline bless move to Round 6.
- **Nothing in Round 5's scope (Items 1–7 from Grok's Round 4 review checked against the canonical PM blob `origin/pm/spec-ui-revamp:plans/specs/ui-revamp/detail/1126-layout-unchanged-gate-baseline-capture.md`, sections 8 and 9) was left unverified (`FACT`).** Windows/Linux CI targets run on GitHub Actions rather than locally on this macOS (`aarch64-apple-darwin`) worker.

### 2. Conclusion & Epistemic Ledger (`FACT` / `INFERENCE` / `ASSUMPTION`)

- **Conclusion (`FACT`):** All 6 actionable findings/test-gaps from Grok's Round 4 review (`plans/reports/review-1126-pr1206-r4-grok.md`) have been implemented with red-before-green commit discipline (`plans/reports/agy-1126-layout-gate-evidence/r5-red.log` and `r5-green.log`), and Item 7 is documented as `no change` per spec rule 5 item 3. Every gate passed on the final code commit (`2b909a5d4`): `0` fmt diffs, `0` clippy warnings, `1,281 passed; 0 failed` in `--workspace --lib`, `3,021 passed; 0 failed` in `designos` bin, `17/17` zero-fake UI gate, and `0` spacing regressions.
- **Epistemic Ledger:**
  - `FACT` — `git fetch origin` at session start confirmed `origin/main` had 0 new commits relative to `4ff960279` (`git rev-list --count HEAD..origin/main == 0`).
  - `FACT` — **Item 1 (Major A — row's own `dh` is range-checked):** PM blob §8 (`G4/G5`) states *"Each row's own `dh` must also lie in its covering entry's `[min_dh, max_dh]`."* Previously only individual widget `dh` was checked (`layout_snapshot.rs:449-452`), while `compute_row_shifts` (`layout_snapshot.rs:604-612`) summed `row_dh = c_h - b_h` without range-checking the row bounding box itself. Added `RowShift { base_min_y, base_max_y, dh, label, covering_idx }` and a row bounding-box `dh` check in `compare_region` (`layout_snapshot.rs:762-795`) before per-widget rect drift and G3 order checks: any G3 row with `|row_dh| > 0.5` whose `row_dh` lies outside its covering entry's `[min_dh - 0.5, max_dh + 0.5]` (or has no covering entry) fails with an error naming the row (`G3/G4 row #0 [A, B] dh=16 out of allowed [8, 8] ...`). Verified red (`ec8e25ba8`) -> green (`2607ec1bf`) with Grok's scenario (`A [0,0,40,24] -> [0,0,40,32]`, `B [0,10,40,24] -> [0,18,40,32]` under `{h_base:24, min_dh:8, max_dh:8}`, where row `dh` is `+16`).
  - `FACT` — **Item 2 (Major B — derived cumulative shift covers `anchor_x: left`, `anchor_x: right`, `anchor_y: bottom`, and `anchor_y: top`):** PM blob §8 (`G4/G5`) states *"For `anchor_x: left` it is the sum of the observed `dw` of the widgets to the left in the same row. `bottom`/`right` sum from the other end. The sum is not a range."* Replaced `row_cum_dh` + `count_row_gaps_above` with `DerivedShifts { sum_dw_left, sum_dw_right, sum_dh_above, sum_dh_below, gaps_above, gaps_below }` (`layout_snapshot.rs:604-725`) and updated `check_rect_with_allow` (`layout_snapshot.rs:365-439`) so:
    - `AnchorX::Left` checks `cur[0]` against `base[0] + sum_dw_left + dx`;
    - `AnchorX::Right` checks `cur[0]` against `base[0] + base[2] - cur[2] - sum_dw_right + dx`;
    - `AnchorY::Top` checks `cur[1]` against `base[1] + sum_dh_above + gaps_above * gap_dy + dy`;
    - `AnchorY::Bottom` checks `cur[1]` against `base[1] + base[3] - cur[3] - sum_dh_below - gaps_below * gap_dy + dy`;
    - `AnchorX::Centre` / `AnchorY::Centre` keep the existing centre-point rule.
    Verified red (`dd0516f39`) -> green (`134b90f36`) by `layout_snapshot::tests::derived_shift_covers_left_right_and_bottom_anchors` (left button `32->40` + right button `x +8` passes; right button `x +12` fails on its edge; right-anchored mirror passes/fails; bottom-anchored stack mirror passes/fails).
  - `FACT` — **Item 3 & Item 6b (Major C — `"*"` entry stale-checked in any spec run that applies it + stale test passes spec filter `Some("F")`):** PM blob §9 states *"An entry that matched nothing in a run is stale and fails, with its index."* Previously `check_stale_allow_entries_for_run` skipped `e.case == "*"` whenever `e.spec != spec_key` (`layout_snapshot.rs:825-828`), even though `compare` applies `e.case == "*"` regardless of `e.spec`. Removed `e.case == "*"` from the `spec_filter` skip in `check_stale_allow_entries_for_run` (`layout_snapshot.rs:922-925`) so wildcard entries are stale-checked against the cases that ran in any spec run. Extended `stale_check_counts_covering_entries_and_runs_over_union_of_run` with Grok's scenario (`check_stale_allow_entries_for_run(..., Some("F"))` with `{case:"*", spec:"D", region:"right_panel", name_prefix:"prop-row-", max_dh:8}` whose only match is an F case: fails as stale when unmatched and passes when matched). Verified red (`3efaae491`) -> green (`5a637742e`).
  - `FACT` — **Item 4 (Minor, finding 6 — scroll detection agrees on both paths for 16×24 drag-only splitter):** In `egui 0.35.0` (`scroll_area.rs:1324-1336`), `ScrollArea` registers `WidgetType::ScrollBar` with `Sense::CLICK | Sense::DRAG` and cross-axis thickness `scroll_style.bar_width` (`10.0`). On the release fallback path (`!has_any_widget_info`), `is_drag_only || ((w - bar_width).abs() <= 0.5 && ...)` previously classified a `16×24` drag-only splitter as `ScrollGeom { maybe: true }` while the widget-info path left it in `unnamed`. Tightened the release fallback in `capture` (`layout_snapshot.rs:230-252`) to require `(w - bar_width).abs() <= 0.5 && h >= 24.0` (vertical) or `(h - bar_width).abs() <= 0.5 && w >= 24.0` (horizontal), so both paths classify the `16×24` drag-only splitter into `unnamed` and only emit `ScrollGeom` for the scrollbar. Verified red (`c8ebbfd49`) -> green (`0f4449b34`) by `scroll_detection_uses_widget_type_scrollbar_and_maybe_in_release`.
  - `FACT` — **Item 5 (Nit — match `R5` as a token in `reason` so `R50` does not pass):** Updated `validate_allow_entries` (`layout_snapshot.rs:348-353`) to tokenize `e.reason` on non-`[A-Za-z0-9_]` characters (`e.reason.split(|c: char| !c.is_ascii_alphanumeric() && c != '_').any(|tok| tok == "R5")`). Verified red (`24c012610`) -> green (`678e3e5bf`) by `negative_dw_or_dh_requires_r5_in_reason`.
  - `FACT` — **Item 6a (Test gap — inventory test builds a count delta above `inventory_max_delta` and fails with G4 count error):** Extended `inventory_max_delta_forgives_count_only_and_compares_shared_prefix` (`layout_snapshot.rs:1487-1499`) to add 3 unnamed widgets (`count_delta = 3 > inventory_max_delta = 2`) and assert `compare` fails with the `G4 unnamed widget count mismatch` error. Committed at `2b909a5d4`.
  - `FACT` — **Item 7 (No change — `agy` spawn wait at `terminal.rs:2948-2957`):** Inspected `crates/designos-app/src/terminal.rs:2945-2960` and PM blob §3.3 rule 5 item 3 (`git show origin/pm/spec-ui-revamp:plans/specs/ui-revamp/detail/1126-layout-unchanged-gate-baseline-capture.md`). The 20 × 10 ms (`200ms` max) `try_wait` poll in `terminal.rs:2948-2957` is explicitly cited by spec rule 5 item 3 (`terminal.rs:2947-2953`) as the intentional non-PTY `agy` immediate-exit check (`exit 127` in `~20ms` when the stub exits immediately), not an unupheld sleep loop. No code change required.
  - **Weakest link (one line):** Until Round 6 runs `scripts/layout_baseline_capture.py` and commits the blessed `*.layout.json` baselines and `cases.json.bless_sha`, `layout_unchanged_d..k` verify live capture + self-comparison (`compare(&snap, &snap, ...)`) rather than cross-commit baseline diffing.

### 3. Skills Loaded & Changes Made Because of Them

- `es:astra-work` & `es:fable-thinking`: Checked each of Grok's Round 4 findings against the canonical PM blob (`origin/pm/spec-ui-revamp:plans/specs/ui-revamp/detail/1126-layout-unchanged-gate-baseline-capture.md`, §8 and §9), stated `PREDICT` before every test and gate command, verified every red commit failed for the exact expected reason (`r5-red.log`) before applying its fix (`r5-green.log`), and ran the full acceptance suite on the final code commit (`2b909a5d4`).
- `es:orca-native-orchestration`: Checked coordinator inbox via `orca orchestration check --terminal term_7216dbdb-2fde-4248-b764-20e7f5c4bd94 --json` at natural checkpoints and sent heartbeats every 5 minutes with `--task-id task_7adc1e178ee7` and `--dispatch-id ctx_369d0dde9849`.
- `es:dev` / `es:feature-dev`: Enforced strict Red (`test(app): …`) -> Green (`fix(app): …`) commit pairs for Items 1–5 plus dedicated test commit `2b909a5d4` for Item 6a.
- `es:rust`: Used zero-allocation token splitting (`split(|c: char| !c.is_ascii_alphanumeric() && c != '_')`) for `R5` token matching, structured `DerivedShifts` (`Copy`) to pass clean borrowed value bundles into `check_rect_with_allow` without triggering `clippy::too_many_arguments`, and inspected `egui-0.35.0/src/containers/scroll_area.rs:1314-1336` to verify `ScrollBar`'s exact `Sense` and `bar_width` invariants.
- `es:rust-gpu-canvas`: Preserved all `designos-gpu` and `designos-render` invariants untouched.

### 4. Round 5 Findings Table (Items 1–7 Checked Against PM Blob)

| # | Severity | Finding Summary | PM Blob Check (`origin/pm/spec-ui-revamp:...`) | Status | Red Commit | Green Commit |
|---|---|---|---|---|---|---|
| **1** | Major A | Row's own bounding-box `dh` (`c_h - b_h`) is never range-checked against its covering entry's `[min_dh, max_dh]` | PM blob §8 (`G4/G5`): *"Each row's own `dh` must also lie in its covering entry's `[min_dh, max_dh]`."* Rejects any row whose `dh` (`\|row_dh\| > 0.5`) falls outside `[min_dh - 0.5, max_dh + 0.5]` with an error naming the row. | `fixed` | `ec8e25ba8` | `2607ec1bf` |
| **2** | Major B | Derived shift covers only `anchor_y: top` instead of `anchor_x: left`, `anchor_x: right`, and `anchor_y: bottom` | PM blob §8 (`G4/G5`): *"For `anchor_x: left` it is the sum of the observed `dw` of the widgets to the left in the same row. `bottom`/`right` sum from the other end. The sum is not a range."* | `fixed` | `dd0516f39` | `134b90f36` |
| **3** | Major C (Finding 4) | `"*"` entry is skipped by per-spec stale check when `e.spec != spec_key`, even though `compare` applies it | PM blob §9: *"An entry that matched nothing in a run is stale and fails, with its index."* Removed `e.case == "*"` from the `spec_filter` skip so wildcard entries are stale-checked against the cases that ran. | `fixed` | `3efaae491` | `5a637742e` |
| **4** | Minor (Finding 6) | Release-path scroll detection classifies a `16×24` drag-only splitter as `scroll {maybe: true}` while widget-info path keeps it in `unnamed` | PM blob §3 rule 3: tightened release-path scrollbar fallback to require `(w - bar_width).abs() <= 0.5 && h >= 24.0` (or horizontal mirror), so both paths keep the `16×24` splitter in `unnamed`. | `fixed` | `c8ebbfd49` | `0f4449b34` |
| **5** | Nit | `"R5"` substring check in `reason` lets `"R50"` pass | Matches `"R5"` as an alphanumeric/underscore-delimited token in `validate_allow_entries`, rejecting `"R50"`. | `fixed` | `24c012610` | `678e3e5bf` |
| **6a** | Test gap | Inventory test did not build a count delta above `inventory_max_delta` and assert the G4 count error | Added count delta `+3 > inventory_max_delta (2)` assertion in `inventory_max_delta_forgives_count_only_and_compares_shared_prefix`. | `fixed` | — (test-only gap) | `2b909a5d4` |
| **6b** | Test gap | Stale test did not pass a spec filter `Some("F")` with a `case: "*", spec: "D"` entry | Added `check_stale_allow_entries_for_run(..., Some("F"))` with `{case:"*", spec:"D", region:"right_panel", name_prefix:"prop-row-", max_dh:8}` in `stale_check_counts_covering_entries_and_runs_over_union_of_run`. | `fixed` | `3efaae491` | `5a637742e` |
| **7** | Note | `agy` spawn wait at `crates/designos-app/src/terminal.rs:2948-2957` | Explicitly cited by PM blob §3.3 rule 5 item 3 (`terminal.rs:2947-2953`) as the intentional 20 × 10 ms non-PTY `try_wait` check for immediate `exit 127`. | `no change` | — | — |

---

### 5. Acceptance Verification (Run on Final Code Commit `2b909a5d4`)

#### Acceptance 1 — Findings Table
See Section 4 above.

#### Acceptance 2 — Red (`r5-red.log`) and Green (`r5-green.log`) Logs
Full logs are committed at `plans/reports/agy-1126-layout-gate-evidence/r5-red.log` and `plans/reports/agy-1126-layout-gate-evidence/r5-green.log`.

```text
=== r5-red.log (each test failing on its RED commit before its fix) ===
Major A (ec8e25ba8): test layout_snapshot::tests::row_own_dh_is_range_checked_against_covering_entry ... FAILED
  panicked at crates/designos-app/src/layout_snapshot.rs:1087:9:
  row dh failure must name the row and report dh out of range, got: G3 named widget 'B' rect drift in case f-inspector (region 'right_panel'): baseline=[0.0, 10.0, 40.0, 24.0], current=[0.0, 18.0, 40.0, 32.0]
  test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3027 filtered out; finished in 0.01s

Major B (dd0516f39): test layout_snapshot::tests::derived_shift_covers_left_right_and_bottom_anchors ... FAILED
  panicked at crates/designos-app/src/layout_snapshot.rs:1216:14:
  left button widening 32->40 and right button shifting x +8 must pass: "G3 named widget 'btn-right' rect drift in case f-inspector (region 'right_panel'): baseline=[1250.0, 60.0, 32.0, 24.0], current=[1258.0, 60.0, 32.0, 24.0]"
  test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3028 filtered out; finished in 0.01s

Major C & 6b (3efaae491): test layout_snapshot::tests::stale_check_counts_covering_entries_and_runs_over_union_of_run ... FAILED
  panicked at crates/designos-app/src/layout_snapshot.rs:1509:14:
  spec D wildcard entry that compare can apply in Some("F") run must fail as stale when unmatched: ()
  test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3028 filtered out; finished in 0.01s

Minor 4 (c8ebbfd49): test layout_snapshot::tests::scroll_detection_uses_widget_type_scrollbar_and_maybe_in_release ... FAILED
  panicked at crates/designos-app/src/layout_snapshot.rs:2035:9:
  assertion `left == right` failed: when widget info is absent, fallback scroll detection must emit only right_panel axis='y', maybe=true; got [ScrollGeom { region: "left_panel", axis: "y", maybe: true }, ScrollGeom { region: "right_panel", axis: "y", maybe: true }]
  test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3028 filtered out; finished in 0.02s

Nit 5 (24c012610): test layout_snapshot::tests::negative_dw_or_dh_requires_r5_in_reason ... FAILED
  panicked at crates/designos-app/src/layout_snapshot.rs:2176:10:
  R50 must not satisfy the R5 token requirement in reason: AllowList { entries: [AllowEntry { case: "f-inspector", region: "right_panel", name_prefix: Some("prop-row-0"), h_base: None, anchor_x: None, anchor_y: None, min_dw: None, max_dw: None, min_dh: Some(-4.0), max_dh: None, dx: None, dy: None, gap_dy: None, inventory_max_delta: None, spec: "F", reason: "R50: tighten inspector row" }] }
  test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3028 filtered out; finished in 0.00s

=== r5-green.log (each test passing on its GREEN commit after its fix) ===
Major A (2607ec1bf):      test layout_snapshot::tests::row_own_dh_is_range_checked_against_covering_entry ... ok (1 passed; 0 failed; finished in 0.01s)
Major B (134b90f36):      test layout_snapshot::tests::derived_shift_covers_left_right_and_bottom_anchors ... ok (1 passed; 0 failed; finished in 0.01s)
Major C & 6b (5a637742e): test layout_snapshot::tests::stale_check_counts_covering_entries_and_runs_over_union_of_run ... ok (1 passed; 0 failed; finished in 0.00s)
Minor 4 (0f4449b34):      test layout_snapshot::tests::scroll_detection_uses_widget_type_scrollbar_and_maybe_in_release ... ok (1 passed; 0 failed; finished in 0.02s)
Nit 5 (678e3e5bf):        test layout_snapshot::tests::negative_dw_or_dh_requires_r5_in_reason ... ok (1 passed; 0 failed; finished in 0.00s)
Test Gap 6a (2b909a5d4):  test layout_snapshot::tests::inventory_max_delta_forgives_count_only_and_compares_shared_prefix ... ok (1 passed; 0 failed; finished in 0.01s)
```

#### Acceptance 3 & 4 — Clean `PATH` Proof + Full Mandatory Test Harness on `2b909a5d4`

```text
$ export PATH="/usr/bin:/bin:/usr/sbin:/sbin:$HOME/.cargo/bin"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
$ export CARGO_TARGET_DIR="$PWD/out/cargo"
$ for c in claude codex gemini grok agy opencode; do command -v $c; done
(empty output — 0 bytes in r5-clean-path.log)

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo fmt --all -- --check
(exit 0 — 0 bytes in r5-fmt.log)

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-mcp)
    Checking designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 52.42s

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test --workspace --locked --lib
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 653 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.96s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 134 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.54s
test result: ok. 121 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 42.39s
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.59s
test result: ok. 224 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.61s
test result: ok. 55 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 32.96s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked
running 3029 tests
...
test video::tests::posters ... ok
test video::tests::garbage_is_not_a_video_and_never_panics ... ok
test terminal::tests::sim_approval_card_and_permissions_modal_pass_layout_lint ... ok
test widgets::menu::tests::rows_scrolled_out_of_the_panel_cannot_be_clicked ... ok
test terminal::tests::touched_layers_do_not_bleed_into_other_active_tab_turn ... ok
test terminal::tests::pty_injection_fixture_real_shell ... ok

test result: ok. 3021 passed; 0 failed; 8 ignored; 0 measured; 0 filtered out; finished in 192.27s

$ python3 scripts/zero_fake_ui_gate.py
------------------------------------------------------------------------------------------------------------
 Gate Verdict: PASS (17/17 invariants satisfied)
============================================================================================================

$ python3 scripts/check_spacing_relationships.py
literal geometry outside theme.rs: offset_literals=1675, inset_literals=45, radius_literals=260, font_literals=25, nonconcentric_pairs=2
note: 11 entries are below baseline; run --write-baseline to lock the gain in
PASS: no count rose above the baseline
```

### 6. Assumptions & Unresolved Questions

- **Assumptions:**
  - `ASSUMPTION` (verified against `egui-0.35.0/src/containers/scroll_area.rs:1314-1336`): On the release `--layout-json` path (`!has_any_widget_info`), `egui::ScrollArea`'s scrollbar response always has cross-axis dimension equal to `spacing.scroll.bar_width` (`10.0pt`) and main-axis `>= 24.0pt`, whereas panel/inspector splitters use wider drag rects (e.g. `16×24`). Requiring `(w - bar_width).abs() <= 0.5 && h >= 24.0` (or horizontal mirror) on the release fallback makes both paths classify `16×24` drag-only splitters into `unnamed`.
- **Unresolved Questions:**
  - None.

Status: DONE — Round 5 comparer and test-gap fixes (Items 1–6 fixed red-to-green, Item 7 documented no-change per spec rule 5 item 3) verified on `2b909a5d4` with 0 fmt diffs, 0 clippy warnings, 1,281/0 workspace lib tests, 3,021/0 designos bin tests, 17/17 zero-fake UI gate, and 0 spacing regressions.

---

## Round 6 (Two Comparer Fixes & One Load-Sensitive Test on `4b6dbb5a` -> `efc19e7e`)

### 1. What Could NOT Be Verified First

- **NOT VERIFIED (Deferred by Task Scope to Round 7):** Section 12 steps 8–9 (`scripts/layout_baseline_capture.py` Tier B screenshot/canvas diffing across the 74 headless + 4 kit cases) and the baseline bless (`tests/layout-baseline/*.layout.json` and `cases.json.bless_sha`) were not executed in Round 6 per the explicit task constraint: *"Fixes only. Section 12 steps 8–9 and the bless are round 7. No `*.layout.json` baselines and no `cases.json.bless_sha`."*

### 2. Conclusion & Epistemic Ledger

- **Conclusion:** Round 6 resolves the two accepted comparer findings from `plans/reports/review-1126-pr1206-r5-grok.md` (Major 1 and Minor 1) and the load-sensitive `open_failed_start_fixture_does_not_block_waiting_for_exit` test (Item 3), while preserving the rejected Major 2 ruling and all Round 5 / Findings 1–12 behaviors unchanged. Each comparer fix was landed with a failing `test(app): …` commit first (`r6-red.log`) followed by its `fix(app): …` commit (`r6-green.log`), and Item 3 was verified to fail when the removed 5 s `poll_live_agent_bridge` sleep loop is restored in `terminal.rs` (`bridge_polled=true, elapsed=5.076707375s`) and pass without it (`0.12s`). All acceptance gates pass on `efc19e7e`.
- **Epistemic Ledger:**
  - `FACT` — **Controller Ruling on Major 2 (`rejected (ruling)`, no code change):** Per controller ruling against PM blob §8: *"Expected shift. It is the entry's exact `dx`/`dy` (default 0), plus a derived cumulative shift."* It applies to each widget on each axis, so a widget with no covering entry and an unchanged size still gets the derived shift from the rows above (which is how a row below a grown row is allowed to move). Preserved both `None if !size_changed` arms in `crates/designos-app/src/layout_snapshot.rs:766-768` and `789-791`.
  - `FACT` — **Item 1 (Major 1 — intersect per-widget best allow ranges on row `dh`, `layout_snapshot.rs:715-736`):** Previously `row_covering` took a single winner across the row's widgets via `max_by_key` and chained `best_matching_entry(..., None, row_b_rect)`, letting one widget's wider `max_dh` (`[8, 32]` on `B`) become the whole row's range even when `A`'s best entry was `[8, 8]`. Updated `compare` to take only each widget's best (most specific) entry (`best_matching_entry(allow, case_id, &reg, w.name, w.b_rect)`) and intersect their `[min_dh, max_dh]` ranges (`min_dh = min_dh.max(e.min_dh.unwrap_or(0.0))`, `max_dh = max_dh.min(e.max_dh.unwrap_or(0.0))`, with an unset side defaulting to `0.0`). A one-widget row still uses that widget's best entry so `equally_specific_entries_fail_to_parse` (`name_prefix must beat h_base`) stays green, and Grok's A/B geometry (`A: [0,0,40,24]->[0,0,40,32]`, `B: [0,10,40,24]->[0,18,40,32]` under `{h_base:24, min_dh:8, max_dh:8}` + `{name_prefix:"B", min_dh:8, max_dh:32, dy:8}`) fails with `G3/G4 row #0 [A, B] dh=16 out of allowed [8, 8]`. Verified red (`361656ab`) -> green (`399dfc4a`).
  - `FACT` — **Item 2 (Minor 1 — restore `gaps_above * gap_dy` on `centre` and `bottom` `y` anchors, `layout_snapshot.rs:475-480`):** PM blob §8 states *"The derived shift also adds the covering entry's `gap_dy` once for each row gap above the widget."* Added `(shifts.gaps_above as f32) * gap_dy` back to `AnchorY::Centre` and `AnchorY::Bottom` while preserving `- shifts.sum_dh_below` on `AnchorY::Bottom`. Verified red (`d897874e`) -> green (`397f5be0`) by `gap_dy_shifts_expected_y_per_row_gap_with_half_pt_tolerance` (`anchor_y: centre` and `anchor_y: bottom` with `gap_dy: 8`, one row above, and an unchanged-size widget moving `+8` passes; `+16` fails).
  - `FACT` — **Item 3 (Load-sensitive test `open_failed_start_fixture_does_not_block_waiting_for_exit`, `layout_snapshot.rs:2165-2189`):** Replaced the load-sensitive `< 500 ms` assertion (which failed at `1.686 s` during a full-suite parallel run on a loaded machine) with: (a) a time-independent check seeding `terminal.deferred_bridge_actions` and asserting `!terminal.deferred_bridge_actions.is_empty()` after `open_failed_start_fixture(None)` returns (since `poll_live_agent_bridge()` drains `deferred_bridge_actions` via `std::mem::take` on its first call), and (b) `elapsed < Duration::from_secs(4)` (well below the 5 s `START_FAILURE_WINDOW` sleep loop and well above loaded parallel spawn time). Verified in `r6-red.log` that restoring the 5 s sleep loop in `terminal.rs:2537` fails the test (`bridge_polled=true, elapsed=5.076707375s`) and restoring `terminal.rs` passes in `0.12s` (`efc19e7e`).
  - `FACT` — **Item 4 (No change to Major 2, Round 5 items 3–4, or findings 1–12):** Verified via `git diff 4b6dbb5a..efc19e7e` that only the row `dh` range intersection, `AnchorY` `gap_dy` addition, and the 3 unit tests in `crates/designos-app/src/layout_snapshot.rs` were touched.
  - **Weakest link (one line):** Until Round 7 runs `scripts/layout_baseline_capture.py` and commits the blessed `*.layout.json` baselines and `cases.json.bless_sha`, `layout_unchanged_d..k` verify live capture + self-comparison (`compare(&snap, &snap, ...)`) rather than cross-commit baseline diffing.

### 3. Skills Loaded & Changes Made Because of Them

- `es:astra-work` & `es:fable-thinking`: Verified every item against PM blob §8 (`origin/pm/spec-ui-revamp:plans/specs/ui-revamp/detail/1126-layout-unchanged-gate-baseline-capture.md`) and controller rulings, stated `PREDICT` before each test and gate run, verified red failures (`r6-red.log`) before green fixes (`r6-green.log`), tested putting the 5 s sleep loop back in `terminal.rs` to prove Item 3 catches it, and ran all acceptance gates on the final code commit (`efc19e7e`).
- `es:orca-native-orchestration`: Checked coordinator inbox via `orca orchestration check --terminal term_5782569b-a2f3-46b0-89a4-5654a49d8546 --json` at natural checkpoints and sent heartbeats every 5 minutes with `--task-id task_4141f3c043a9` and `--dispatch-id ctx_45d906e0c1c0`.
- `es:dev` / `es:feature-dev`: Enforced strict Red (`test(app): …`) -> Green (`fix(app): …`) commits for Items 1 and 2 plus `test(app): …` commit for Item 3.
- `es:rust`: Removed unused `b_min_x`/`b_max_x`/`b_w` row-width temporaries and the unused `DerivedShifts::gaps_below` field when switching `AnchorY::Bottom` to `gaps_above * gap_dy`, ensuring `cargo clippy --workspace --all-targets --locked -- -D warnings` passes with 0 warnings.
- `es:rust-gpu-canvas`: Preserved all `designos-gpu` and `designos-render` invariants untouched.

### 4. Round 6 Items Table (Items 1–4 & Major 2 Ruling)

| # | Item / Review Finding | Summary | Status | Red Commit | Green / Final Commit |
|---|---|---|---|---|---|
| **Major 2** | Uncovered unchanged-size widget inherits derived shift (`layout_snapshot.rs:766-768, 789-791`) | PM blob §8: *"Expected shift. It is the entry's exact `dx`/`dy` (default 0), plus a derived cumulative shift."* Applies to each widget on each axis so a row below a grown row is allowed to move; kept `None if !size_changed` arms per controller ruling. | `rejected (ruling)` | — | — |
| **1** | **Major 1:** One widget's wider range becomes the whole row's range (`layout_snapshot.rs:715-736`) | Take only each widget's best (most specific) entry in the row and intersect their `[min_dh, max_dh]` ranges (±0.5; unset side is `0.0`). One-widget rows keep `name_prefix > h_base`; Grok's A/B geometry fails naming the row and `dh=16`. | `fixed` | `361656ab` | `399dfc4a` |
| **2** | **Minor 1:** `gap_dy` was dropped for `centre` and `bottom` (`layout_snapshot.rs:475-480`) | Added `(shifts.gaps_above as f32) * gap_dy` back to `AnchorY::Centre` and `AnchorY::Bottom` while keeping `- shifts.sum_dh_below` on `AnchorY::Bottom`. Tested `anchor_y: centre` and `bottom` with `gap_dy: 8`, 1 row above, and unchanged-size widget moving `+8`. | `fixed` | `d897874e` | `397f5be0` |
| **3** | **Load-sensitive test:** `open_failed_start_fixture_does_not_block_waiting_for_exit` (`layout_snapshot.rs:2165-2189`) | Asserts both `!bridge_polled` (`deferred_bridge_actions` sentinel not drained by `poll_live_agent_bridge()`) and `elapsed < 4s`. Proven to fail (`bridge_polled=true, elapsed=5.076707375s`) when the 5 s sleep loop is restored in `terminal.rs`. | `fixed` | `r6-red.log` (with 5 s loop restored in `terminal.rs`) | `efc19e7e` |
| **4** | Preserve Major 2, Round 5 items 3–4, and findings 1–12 | Untouched (`git diff 4b6dbb5a..efc19e7e`). | `no change` | — | — |

---

### 5. Acceptance Verification (Run on Final Code Commit `efc19e7e`)

#### Acceptance 1 — Items 1–4 Table
See Section 4 above.

#### Acceptance 2 — Red (`r6-red.log`) and Green (`r6-green.log`) Logs
Full logs are committed at `plans/reports/agy-1126-layout-gate-evidence/r6-red.log` and `plans/reports/agy-1126-layout-gate-evidence/r6-green.log`.

```text
=== r6-red.log (each test failing before its fix / with the 5s sleep loop restored) ===
Major 1 (361656ab): test layout_snapshot::tests::row_own_dh_is_range_checked_against_covering_entry ... FAILED
  thread 'layout_snapshot::tests::row_own_dh_is_range_checked_against_covering_entry' (76510932) panicked at crates/designos-app/src/layout_snapshot.rs:1219:9:
  row dh failure must name the row and dh=16, got: G3 row order mismatch in case f-inspector region 'right_panel': baseline=[["A", "B"]], current=[["A"], ["B"]]
  test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3028 filtered out; finished in 0.01s

Minor 1 (d897874e): test layout_snapshot::tests::gap_dy_shifts_expected_y_per_row_gap_with_half_pt_tolerance ... FAILED
  thread 'layout_snapshot::tests::gap_dy_shifts_expected_y_per_row_gap_with_half_pt_tolerance' (76551899) panicked at crates/designos-app/src/layout_snapshot.rs:1418:37:
  anchor_y=Centre with gap_dy=8 and 1 row above moving +8 must pass: G3 named widget 'prop-row-1' rect drift in case f-inspector (region 'right_panel'): baseline=[1210.0, 90.0, 200.0, 24.0], current=[1210.0, 98.0, 200.0, 24.0]
  test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3028 filtered out; finished in 0.01s

Item 3 (5s sleep loop put back in terminal.rs): test layout_snapshot::tests::open_failed_start_fixture_does_not_block_waiting_for_exit ... FAILED
  thread 'layout_snapshot::tests::open_failed_start_fixture_does_not_block_waiting_for_exit' (76594410) panicked at crates/designos-app/src/layout_snapshot.rs:2186:9:
  open_failed_start_fixture must not block in a 5s poll_live_agent_bridge sleep loop: bridge_polled=true, elapsed=5.076707375s
  test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3028 filtered out; finished in 5.16s

=== r6-green.log (each test passing after its fix) ===
Major 1 (399dfc4a):
  test layout_snapshot::tests::row_own_dh_is_range_checked_against_covering_entry ... ok (1 passed; 0 failed; finished in 0.00s)
  test layout_snapshot::tests::equally_specific_entries_fail_to_parse ... ok (1 passed; 0 failed; finished in 0.01s)
Minor 1 (397f5be0):
  test layout_snapshot::tests::gap_dy_shifts_expected_y_per_row_gap_with_half_pt_tolerance ... ok (1 passed; 0 failed; finished in 0.01s)
  test layout_snapshot::tests::derived_shift_covers_left_right_and_bottom_anchors ... ok (1 passed; 0 failed; finished in 0.02s)
Item 3 (efc19e7e):
  test layout_snapshot::tests::open_failed_start_fixture_does_not_block_waiting_for_exit ... ok (1 passed; 0 failed; finished in 0.12s)
```

#### Acceptance 3 & 4 — Clean `PATH` Proof + Full Mandatory Test Harness on `efc19e7e`

```text
$ export PATH="/usr/bin:/bin:/usr/sbin:/sbin:$HOME/.cargo/bin"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
$ export CARGO_TARGET_DIR="$PWD/out/cargo"
$ for c in claude codex gemini grok agy opencode; do command -v $c; done
(empty output — 0 bytes in r6-clean-path.log)

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo fmt --all -- --check
(exit 0 — 0 bytes in r6-fmt.log)

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-mcp)
    Checking designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 27.03s

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test --workspace --locked --lib
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 653 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.52s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 134 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.21s
test result: ok. 121 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 41.66s
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.31s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.27s
test result: ok. 224 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.31s
test result: ok. 55 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 33.25s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked
running 3029 tests
...
test widgets::menu::tests::scrolling_to_the_end_reveals_the_last_row_and_a_click_picks_it ... ok
test widgets::menu::tests::arrow_keys_keep_the_highlighted_row_in_view ... ok
test terminal::tests::sim_approval_card_and_permissions_modal_pass_layout_lint ... ok
test video::tests::garbage_is_not_a_video_and_never_panics ... ok
test widgets::menu::tests::rows_scrolled_out_of_the_panel_cannot_be_clicked ... ok
test terminal::tests::touched_layers_do_not_bleed_into_other_active_tab_turn ... ok
test terminal::tests::pty_injection_fixture_real_shell ... ok

test result: ok. 3021 passed; 0 failed; 8 ignored; 0 measured; 0 filtered out; finished in 169.17s

$ python3 scripts/zero_fake_ui_gate.py
------------------------------------------------------------------------------------------------------------
 Gate Verdict: PASS (17/17 invariants satisfied)
============================================================================================================

$ python3 scripts/check_spacing_relationships.py
literal geometry outside theme.rs: offset_literals=1675, inset_literals=45, radius_literals=260, font_literals=25, nonconcentric_pairs=2
note: 11 entries are below baseline; run --write-baseline to lock the gain in
PASS: no count rose above the baseline
```

### 6. Assumptions & Unresolved Questions

- **Assumptions:**
  - `ASSUMPTION` (verified against `crates/designos-app/src/terminal.rs:3175-3178`): `TerminalState::poll_live_agent_bridge` unconditionally drains `self.deferred_bridge_actions` via `std::mem::take` on its first statement, while `spawn_tab_running` never touches `deferred_bridge_actions`. Seeding a sentinel in `deferred_bridge_actions` before `open_failed_start_fixture(None)` therefore provides a clock-independent proof that `open_failed_start_fixture` did not enter the removed `poll_live_agent_bridge` loop, alongside the `< 4 s` bound against raw sleep loops.
- **Unresolved Questions:**
  - None.

Status: DONE — Round 6 comparer and test fixes (Major 1 and Minor 1 fixed red-to-green, Item 3 made load-resilient and proven to fail when the 5 s sleep loop is put back, Major 2 rejected per controller ruling) verified on `efc19e7e` with 0 fmt diffs, 0 clippy warnings, 1,281/0 workspace lib tests, 3,021/0 designos bin tests, 17/17 zero-fake UI gate, and 0 spacing regressions.


---

## Round 7 — Load-Proof Tests, Tier B Capture Script (`scripts/layout_baseline_capture.py`), and Local 74-Case Tier B Verification (`#1126` Step 8)

### 1. What Could NOT Be Verified (First Section)

1. **Step 9 Bless (`cases.json.bless_sha` and `crates/designos-app/tests/layout-baseline/*.layout.json`):**
   - Explicitly excluded from Round 7 per controller brief: *"Not in this round: the bless (step 9). No `*.layout.json` baselines and no `cases.json.bless_sha`. Rule 11 needs `bless_sha` to be the parent of the squash commit, with nothing merging in between, so the controller runs the bless itself in a merge window agreed with the PM."*
2. **Kit Cases (`kit-editor` and `kit-agents`):**
   - Skipped during the headless local Tier B run (`SKIP kit-editor: no --kit` and `SKIP kit-agents: no --kit`) because `--kit` was omitted in the headless run (`Cases: 74 captured, 2 skipped (76 total)`).
3. **Spec vs. Brief Differences Identified & Documented:**
   - **Difference 1 (`--seed-library` in `scripts/layout_baseline_capture.py` vs. Spec L §5):** Spec L §5 shows `designos --seed-library "$RUN/lib"` inside the `env -i` block where `DESIGNOS_AGENT_CLI_DIR="$RUN/stubs"` is already exported. However, `cli_resolve::enforce_hermetic_startup_os()` (Rule 5) denies `DESIGNOS_AGENT_CLI_DIR` whenever `--screenshot` is not present on the command line (returning exit code `2` with `DESIGNOS_AGENT_CLI_DIR requires --screenshot`). Therefore `scripts/layout_baseline_capture.py` runs `designos --seed-library "$RUN/lib"` under `env -i` with `HOME`, `PATH`, `TMPDIR`, `DESIGNOS_LIBRARY` first, re-verifies `<run>/lib/.designos-hermetic`, and then executes the case capture under the exact 6-variable `env -i` block (`HOME`, `PATH`, `TMPDIR`, `DESIGNOS_LIBRARY`, `DESIGNOS_AGENT_CLI_DIR`, `DESIGNOS_STUB_LOG`).
   - **Difference 2 (`cargo test -p designos-app --bin designos --locked` environment vs. Acceptance Table command):** Exporting `DESIGNOS_AGENT_CLI_DIR="$PWD/crates/designos-app/tests/fixtures/agent-stubs"` into the parent `cargo test --bin designos` process activates `cli_resolve::hermetic_mode_active()` (`std::env::var_os("DESIGNOS_AGENT_CLI_DIR").is_some()`), while `HERMETIC_AGENT_CLI_DIR: OnceLock<PathBuf>` is only populated by `enforce_hermetic_startup_os()` inside `main()`, which the Rust test harness does not call. Non-gate unit tests that call `build_agent_shell_cmd_checked_for_tab` (`terminal.rs:4414`) then fail with `Err("hermetic mode: startup-validated DESIGNOS_AGENT_CLI_DIR is missing")`. Running `cargo test -p designos-app --bin designos --locked` under the clean `PATH="/usr/bin:/bin:/usr/sbin:/sbin:$HOME/.cargo/bin"` without parent-shell `DESIGNOS_AGENT_CLI_DIR` (letting each hermetic test scope `DESIGNOS_AGENT_CLI_DIR` via `ScopedCliDirs` or child `Command::env`) passes all `3,021` bin tests with `0` failures.
   - **Difference 3 (macOS AMFI/syspolicy vnode cache warm-up for `"spawns": true` cases):** Copying stub scripts to a fresh temporary directory per case (`<run>/stubs`) creates new inodes on macOS `/var/folders/...`; on the first execution of a fresh script inode, macOS `syspolicy_check` can delay `/bin/sh` startup past the 12-frame headless screenshot window before `stubs/claude` appends its invocation line to `$DESIGNOS_STUB_LOG`. For `"spawns": true` cases (`i-agents-chat`, `i-agents-start-failure`), `copy_stubs_with_marker` warms the copied stub inodes by running `<stub> --version` under `env -i PATH=/usr/bin:/bin` (without `DESIGNOS_STUB_LOG` set) before launching `designos`.

---

### 2. Part A — Complete Audit of `Instant` / `Duration` Bounds Under 5 s in `git diff origin/main...HEAD`

Committed in `7b0d108e9` (`test(app): make layout gate and cli resolver tests load-proof`). Every `Instant` / `Duration` occurrence in `git diff origin/main...HEAD` was audited:

| # | Location | Original Bound / Usage | Action Taken in Round 7 | Why |
|---|---|---|---|---|
| 1 | `crates/designos-app/src/cli_resolve.rs:916` (`non_hermetic_catalog_path_prepends_install_and_extra_before_host_path`) | `SystemCommandRunner.run(..., Duration::from_secs(2))` | **Changed to `Duration::from_secs(30)`** | Under full-bin parallel load, `fork`/`exec` of `/bin/sh` exceeded 2 s and timed out with exit code `None`. `run()` returns as soon as the child exits, so 30 s makes the PATH check load-proof. |
| 2 | `crates/designos-app/src/layout_snapshot.rs:1773-1792` (`agent_cases_do_not_depend_on_installed_clis` 8c child test) | `assert!(build_elapsed < Duration::from_millis(600), ...)` | **Replaced with time-independent state assertions** right after `Shell::build` before any frame runs (`pending_send == Some("Create a SaaS analytics card in Claude Code")`, `notice == Some("Starting Claude Code\u{2026} your message will be sent")`, `active_tab().is_some_and(\|t\| t.last_usage_status.is_none())`), plus set `app.terminal.active_runtime = AgentCliPreset::Claude` in `crates/designos-app/src/shell.rs:336` (`Some("agents-chat")`). | A 600 ms wall-clock bound cannot distinguish an 800 ms sleep loop from a loaded machine. If the old 800 ms `poll_live_agent_bridge()` loop in `Shell::build` is put back, `pending_send` is drained (`None`) and `notice` is cleared (`None`) during `Shell::build`, failing the new state check deterministically (`plans/reports/agy-1126-layout-gate-evidence/r7-red.log` lines 1–56). |
| 3 | `crates/designos-app/src/layout_snapshot.rs:1824-1848` (8c test `i-agents-start-failure` wait) and `crates/designos-app/src/shell/tests/layout_unchanged/mod.rs:109-132` (`run_spec_cases` `i-agents-start-failure` wait) | `Duration::from_secs(2)` deadline loop without post-loop exit assertion | **Changed deadline to `Duration::from_secs(15)`**, added `assert!(exited, "case {}: stub child did not exit within 15s deadline", ...)`, refreshed `t.spawned_at = Instant::now()` before the 2 diagnostic frames so `counts_as_failed_start` (`<= START_FAILURE_WINDOW`) is immune to slow OS spawn, and asserted `active_tab().is_some_and(\|t\| t.start_failure.is_some())`. | Prevents snapshotting a half-started state under heavy load while still breaking out of the poll loop immediately once the stub exits. |
| 4 | `crates/designos-app/src/layout_snapshot.rs:2173-2203` (`open_failed_start_fixture_does_not_block_waiting_for_exit`) | `elapsed < Duration::from_secs(4)` combined with `!bridge_polled` | **Removed `Instant::now()` / `Duration::from_secs(4)` wall-clock check**; replaced with deterministic state check `!bridge_polled && unpolled_tab` (`!terminal.deferred_bridge_actions.is_empty()` and `active_tab().is_some_and(\|t\| t.start_failure.is_none() && t.last_usage_status.is_none())`). | Eliminates wall-clock sensitivity while directly verifying that `open_failed_start_fixture` neither drained `deferred_bridge_actions` nor polled the tab's exit status during fixture open. |
| 5 | `crates/designos-app/src/layout_snapshot.rs:1833` and `crates/designos-app/src/shell/tests/layout_unchanged/mod.rs:118` | `std::thread::sleep(Duration::from_millis(10))` | **Left unchanged** | This is the 10 ms polling sleep interval inside the 15 s child-exit wait loop, not a timeout or deadline assertion. |
| 6 | `crates/designos-app/src/showcase/mod.rs:101-130` | `Duration::from_secs(1_700_000_000)`, `Duration::from_secs(120)`, `Duration::from_secs(3_600)`, `Duration::from_secs(86_400)`, `Duration::from_secs(259_200)` | **Left unchanged** | Fixed epoch constants (`T_BASE - ...`) used by `--seed-library` for deterministic `modified` timestamps (Rule 7), not wall-clock bounds. |

---

### 3. Part B & Part C — Commits & Evidence Files

- **Commits on `jangtrinh/agy-1126-layout-gate`:**
  1. `b01caef60` — `Merge remote-tracking branch 'origin/main' into jangtrinh/agy-1126-layout-gate`
  2. `7b0d108e9` — `test(app): make layout gate and cli resolver tests load-proof`
  3. `71a3c8744` — `test(scripts): add unit tests for layout_baseline_capture.py`
  4. Final commit — `feat(scripts): add Tier B layout baseline capture and comparison script`
- **Evidence Files in `plans/reports/agy-1126-layout-gate-evidence/`:**
  - `r7-red.log` — Part A (8c test failing with the 800 ms `poll_live_agent_bridge` loop restored in `Shell::build`) + Part B (`python3 -m unittest discover -s scripts/tests -v` failing with `ModuleNotFoundError: No module named 'layout_baseline_capture'` before `scripts/layout_baseline_capture.py` existed).
  - `r7-green.log` — Part A & Part B green runs (`LayoutBaselineCaptureTest` 9/9 passing, full `scripts/tests` 52/52 passing, and load-proof Rust tests passing).
  - `r7-tierb.log` — Full 74-case local Tier B capture (`Cases: 74 captured, 2 skipped (76 total)`) and self-compare (`G6: 0 mismatches.`).
  - `wireframe-d-window-chrome.png` & `r7-wireframe-d-window-chrome.png` — `d-window-chrome` wireframe overlay sheet (`7,544` bytes).
  - `colour-sheet.png` & `r7-colour-sheet.png` — 8-column colour contact sheet (`974,173` bytes = `951 KB`, `< 1 MB`).
  - `invocations.log` & `r7-invocations.log` — Stub invocation log from the 74-case Tier B capture (`2` lines: `.../run/stubs/claude --session-id ...` and `.../run/stubs/agy --mcp-config /nonexistent/mcp.json`).

---

### 4. Acceptance Table (All 8 Gates with Pasted Output)

| Gate | Command | Verdict | Output Summary |
|---|---|---|---|
| **1. Clean `PATH`** | `for c in claude codex gemini grok agy opencode; do command -v $c; done` | **PASS** | Prints nothing (`0` lines) |
| **2. Format** | `cargo fmt --all -- --check` | **PASS** | Exit `0` |
| **3. Clippy** | `cargo clippy --workspace --all-targets --locked -- -D warnings` | **PASS** | Exit `0`, `0` warnings |
| **4. Workspace lib** | `cargo test --workspace --locked --lib` | **PASS** | `1,281 passed; 0 failed` across 12 lib crates |
| **5. Bin suite** | `cargo test -p designos-app --bin designos --locked` | **PASS** | `3,021 passed; 0 failed; 8 ignored` (incl. `non_hermetic_catalog_path_prepends_install_and_extra_before_host_path`, `agent_cases_do_not_depend_on_installed_clis`, `open_failed_start_fixture_does_not_block_waiting_for_exit`) |
| **6. Zero-fake & spacing** | `python3 scripts/zero_fake_ui_gate.py && python3 scripts/check_spacing_relationships.py` | **PASS** | `17/17 invariants satisfied` & `PASS: no count rose above the baseline` |
| **7. Script unit tests (Test 11)** | `python3 -m unittest discover -s scripts/tests -v` | **PASS** | `Ran 52 tests in 11.084s — OK` (`9/9` in `LayoutBaselineCaptureTest`) |
| **8. Local Tier B (Test 10)** | `scripts/layout_baseline_capture.py` capture + `--compare` self-compare | **PASS** | `74 captured, 2 skipped (76 total)`, all 3 post-run checks passed, `--compare` exited `0` (`G6: 0 mismatches.`) |

#### Pasted Gate Outputs

```text
=== 1. Clean PATH ===
$ export PATH="/usr/bin:/bin:/usr/sbin:/sbin:$HOME/.cargo/bin"
$ for c in claude codex gemini grok agy opencode; do command -v $c; done
(empty output — 0 lines)

=== 2. Format ===
$ cargo fmt --all -- --check
(exit 0 — 0 lines)

=== 3. Clippy ===
$ cargo clippy --workspace --all-targets --locked -- -D warnings
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1126-layout-gate/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.34s

=== 4. Workspace lib ===
$ cargo test --workspace --locked --lib
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 653 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.24s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 134 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.25s
test result: ok. 121 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 36.11s
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.49s
test result: ok. 224 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.53s
test result: ok. 55 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 35.15s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

=== 5. Bin suite ===
$ cargo test -p designos-app --bin designos --locked
running 3029 tests
...
test cli_resolve::tests::non_hermetic_catalog_path_prepends_install_and_extra_before_host_path ... ok
test layout_snapshot::tests::agent_cases_do_not_depend_on_installed_clis ... ok
test layout_snapshot::tests::open_failed_start_fixture_does_not_block_waiting_for_exit ... ok
...
test result: ok. 3021 passed; 0 failed; 8 ignored; 0 measured; 0 filtered out; finished in 155.22s

=== 6. Zero-fake & spacing ===
$ python3 scripts/zero_fake_ui_gate.py && python3 scripts/check_spacing_relationships.py
------------------------------------------------------------------------------------------------------------
 Gate Verdict: PASS (17/17 invariants satisfied)
============================================================================================================
literal geometry outside theme.rs: offset_literals=1675, inset_literals=45, radius_literals=260, font_literals=25, nonconcentric_pairs=2
note: 11 entries are below baseline; run --write-baseline to lock the gain in
PASS: no count rose above the baseline

=== 7. Script unit tests (Test 11) ===
$ python3 -m unittest discover -s scripts/tests -v
test_clean_run_exits_0_and_writes_sheets (test_layout_baseline_capture.LayoutBaselineCaptureTest.test_clean_run_exits_0_and_writes_sheets) ... ok
test_compare_structural_drift_exits_3_and_prints_both_g1_g5_and_g6 (test_layout_baseline_capture.LayoutBaselineCaptureTest.test_compare_structural_drift_exits_3_and_prints_both_g1_g5_and_g6) ... ok
test_corrupt_screenshot_png_exits_1 (test_layout_baseline_capture.LayoutBaselineCaptureTest.test_corrupt_screenshot_png_exits_1) ... ok
test_log_line_outside_stubs_exits_2 (test_layout_baseline_capture.LayoutBaselineCaptureTest.test_log_line_outside_stubs_exits_2) ... ok
test_missing_hermetic_active_file_exits_2 (test_layout_baseline_capture.LayoutBaselineCaptureTest.test_missing_hermetic_active_file_exits_2) ... ok
test_missing_kit_skips_kit_cases_and_exits_0 (test_layout_baseline_capture.LayoutBaselineCaptureTest.test_missing_kit_skips_kit_cases_and_exits_0) ... ok
test_missing_screenshot_exits_1_with_message (test_layout_baseline_capture.LayoutBaselineCaptureTest.test_missing_screenshot_exits_1_with_message) ... ok
test_only_spec_unknown_key_exits_2 (test_layout_baseline_capture.LayoutBaselineCaptureTest.test_only_spec_unknown_key_exits_2) ... ok
test_spawns_true_with_no_log_line_exits_2 (test_layout_baseline_capture.LayoutBaselineCaptureTest.test_spawns_true_with_no_log_line_exits_2) ... ok
...
----------------------------------------------------------------------
Ran 52 tests in 11.084s

OK

=== 8. Local Tier B (Test 10) ===
$ python3 scripts/layout_baseline_capture.py \
    --bin "$PWD/out/cargo/release/designos" \
    --cases crates/designos-app/tests/layout-baseline/cases.json \
    --stubs crates/designos-app/tests/fixtures/agent-stubs \
    --out "$PWD/out/layout-tierb/993afe38"
SKIP kit-editor: no --kit
SKIP kit-agents: no --kit
Every case has its acknowledgement.
Every spawns case logged a stub line.
Every $0 in invocations.log starts with the stub dir (/private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/designos-tierb-run-2an_jetj/run/stubs/).
Cases: 74 captured, 2 skipped (76 total), wall time 437.87s

$ python3 scripts/layout_baseline_capture.py \
    --bin "$PWD/out/cargo/release/designos" \
    --cases crates/designos-app/tests/layout-baseline/cases.json \
    --stubs crates/designos-app/tests/fixtures/agent-stubs \
    --out "$PWD/out/layout-tierb/993afe38-selfcheck" \
    --compare "$PWD/out/layout-tierb/993afe38"
SKIP kit-editor: no --kit
SKIP kit-agents: no --kit
G6: 0 mismatches.
```

---

### 5. Assumptions & Skills Loaded

- **Assumptions:**
  - `FACT`: `cli_resolve::enforce_hermetic_startup_os()` rejects `DESIGNOS_AGENT_CLI_DIR` when `--screenshot` is not present, so `--seed-library` must execute before `DESIGNOS_AGENT_CLI_DIR` is added to the per-case `env -i` environment.
  - `FACT`: `Shell::build` for `--fixture agents-chat` previously left `app.terminal.active_runtime` at `AgentCliPreset::default()` (`Auto`), which resolves the first alphabetical stub (`agy`) instead of `claude` and omits the `"Starting Claude Code… your message will be sent"` notice asserted by the 8c test. Setting `app.terminal.active_runtime = AgentCliPreset::Claude` in `Some("agents-chat")` aligns both Tier A and Tier B (`invocations.log` records `.../run/stubs/claude --session-id ...` for `i-agents-chat` and `.../run/stubs/agy --mcp-config /nonexistent/mcp.json` for `i-agents-start-failure`).
- **Skills Loaded:**
  - `es:dev`, `es:feature-dev`, `es:rust`, `es:rust-gpu-canvas`, `es:astra-work`, `es:fable-thinking`, `es:orca-native-orchestration`.

Status: READY FOR CONTROLLER REVIEW (ROUND 7)
