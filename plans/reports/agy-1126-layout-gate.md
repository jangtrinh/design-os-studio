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
