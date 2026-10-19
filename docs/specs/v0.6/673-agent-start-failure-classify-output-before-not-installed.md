# #673 Agent start failure: classify unknown-flag and not-logged-in from the output before "not installed"

| Field | Value |
|---|---|
| Issue | #673 (found in the v0.5.0 release review of #665; non-blocking) |
| Size / priority | S / P2 |
| Depends on | none |
| Unblocks | #652 (picker install state), #679 (more CLIs reuse the probe list) |
| Touches | `crates/designos-app/src/agent_start_failure.rs` (`diagnose`), `crates/designos-app/src/terminal.rs` (`runtime_binary_available`, `configure_pty_env_with_session`, the `diagnose` call at `:1349`), tests in both files |

Code baseline: `origin/main@01d48f2` (0.5.0). Facts about installed CLIs below were observed on the dev
machine on 2026-10-19 (versions in `plans/reports/release-060-evidence/cli-help-261019/versions.txt`).

## 1. Summary and user story

As a user whose agent CLI is installed in its own folder (Grok in `~/.grok/bin`), when that CLI starts and
quits within five seconds because I am not signed in, the failure panel says "Grok Build is not signed in"
and offers "Run login", not "is not installed" with a link to the install page.

## 2. Current state (FACT)

- `diagnose(preset, installed, exit_code, raw_output)` (`agent_start_failure.rs:116-148`) picks the cause
  with `if !installed || has(NOT_INSTALLED)` first (`:133`). `installed` is decisive: a false value wins
  over any output.
- `installed` comes from `tab.spawn_preset.is_installed()` at the call
  (`terminal.rs:1349`), i.e. `runtime_binary_available(binary_name)` (`terminal.rs:193-198`, `:202-240`).
- That probe checks Studio's own `PATH`, then `~/.cargo/bin`, `~/.local/bin`, `~/.antigravity/bin`,
  `/opt/homebrew/bin`, `/usr/local/bin` (`terminal.rs:212-228`). A GUI app does not inherit the login
  shell's `PATH`, while the tab runs the CLI through `$SHELL -l -i -c "exec <cli>"`
  (`build_agent_shell_cmd`, `terminal.rs:2112-2127`) and the login shell does.
- Observed on this machine: `grok` resolves to `~/.grok/bin/grok` (`command -v grok`), which is not in the
  probe list; `agy` is also installed under `~/.antigravity/antigravity/bin/agy`, a folder that is not
  `~/.antigravity/bin`; `opencode` is in `~/.opencode/bin`; `claude`, `codex`, `gemini` are npm links in
  `/opt/homebrew/bin` (probed). Grok's own `grok du` help describes `~/.grok` as its home.
- The child's `PATH` gets the same shortened list prepended (`terminal.rs:2316-2334`), so the two lists
  can drift.
- A start failure counts when the CLI exits within `START_FAILURE_WINDOW` (5 s) and not with code 0
  (`counts_as_failed_start`, `:67-71`).
- Output needles today: `NOT_INSTALLED` (`command not found`, `: not found`,
  `is not recognized as an internal`), `UNKNOWN_FLAG`, `NOT_LOGGED_IN` (`not logged in`, `please log in`,
  `/login`, `unauthorized`, `sign in`, `invalid api key`, ...) (`:118-131`).
- Existing tests: `unknown_flag_output_of_the_real_clis_is_recognised`, `missing_binary_offers_the_install_page`,
  `signed_out_cli_offers_its_own_login` (all in the `tests` module of `agent_start_failure.rs`), and the
  end-to-end stub in `terminal.rs` (`spawn_exiting_stub_agent`, `:544-548`, assertion near `:5650`).

## 3. Behaviour rules

1. Precedence in `diagnose`: first the causes the CLI's own output proves, then install state:
   1. `UnknownFlag` (output contains an unknown-flag needle);
   2. `NotLoggedIn` (output contains a signed-out needle);
   3. `NotInstalled` when the output has a not-installed needle, or when `!installed` and the output
      names no other cause;
   4. otherwise `ExitedEarly`.
2. `installed` stays an input, now only a fallback signal. A signed-out message from a CLI Studio could not
   find is therefore "not signed in": the shell ran it.
3. The probe learns each CLI's own install folder. A new `AgentCliPreset::install_dirs()` returns the
   verified folders: Grok `~/.grok/bin`; Antigravity `~/.antigravity/bin` (kept) and
   `~/.antigravity/antigravity/bin`; the shared user folders already probed. One function feeds both
   `runtime_binary_available` (`terminal.rs:220-226`) and the `PATH` prepend in
   `configure_pty_env_with_session` (`:2316-2323`) so they cannot drift.
4. Windows: unchanged. The probe reads `HOME` only (`terminal.rs:220`) and keeps its `.exe`/`.cmd`/`.bat`
   suffix rule (`:236-238`); the new folders are probed under the same `HOME` and not verified on Windows
   (releases are verified on macOS).
5. The fix button follows the cause as today (`NotLoggedIn` -> `RunLogin`, `NotInstalled` -> install page,
   `UnknownFlag` -> shell).
6. No new cause, copy or UI. The `explanation()` for `NotInstalled` says "could not find ... on your PATH"
   and stays true because a missing output cause is the only way to reach it.

## 4. Data model / schema changes

None. `StartFailureCause` and `FixAction` are unchanged.

## 5. Contracts

`diagnose`'s signature is unchanged (`installed: bool` stays). Behavioural contract: output-based causes
win. `runtime_binary_available(bin)` gains folders, nothing is removed.

## 6. File-format mapping

None.

## 7. UI

None. The failure panel (`agent_start_failure::paint`) shows the corrected headline and fix; capture the
panel for the signed-out case in the PR (desktop 1440, responsive SKIPPED).

## 8. Edge cases

1. Grok in `~/.grok/bin`, Studio launched from Finder, signed out: output has `unauthorized` ->
   `NotLoggedIn`, fix "Run login" (acceptance).
2. Truly absent CLI: the login shell prints `command not found` -> `NotInstalled` through the output
   needle, and with a silent exit through `!installed`.
3. CLI present but a flag is rejected: `UnknownFlag` even if the probe said not installed.
4. Output with both a signed-out and an unknown-flag needle: `UnknownFlag` wins (existing order between
   those two is kept).
5. Exit code 0 within five seconds: not a failure (unchanged).
6. A model name or file named like a needle (for example a line containing `: not found` printed by the
   CLI for a missing file) is classified by the first rule it matches; the precedence above keeps a
   genuine signed-out line ahead of it. Known limit, not fixed here.

## 9. Non-goals

- Probing arbitrary custom folders or asking the shell (`command -v`) for install state; the picker
  (#652) decides whether to run a login-shell probe.
- New causes (network error, outdated version).
- Changing the five-second window.

## 10. Test plan (red first)

Seams: pure unit tests on `diagnose`; one stub-process test through the real tab.

| Case | Steps | Expected (fails before) |
|---|---|---|
| `signed_out_output_beats_a_failed_install_probe` | `diagnose(Grok, false, Some(1), "Error: unauthorized\nrun grok login")` | cause `NotLoggedIn`, fix `RunLogin("grok login")` |
| `unknown_flag_output_beats_a_failed_install_probe` | `diagnose(Antigravity, false, Some(2), "flags provided but not defined: -mcp-config")` | `UnknownFlag` |
| `silent_exit_of_an_unfound_binary_is_not_installed` | `diagnose(Codex, false, Some(127), "")` | `NotInstalled` |
| `command_not_found_output_is_not_installed` | `diagnose(Claude, true, Some(127), "zsh: command not found: claude")` | `NotInstalled` |
| `stub_cli_off_the_probed_path_that_prints_unauthorized` | write an executable script `stub-agent` printing `unauthorized` and exiting 1 into a temp dir that is neither on `PATH` nor in `install_dirs()`; start a tab with it through `spawn_with_session_cmd` (command override by absolute path); poll `poll_live_agent_bridge` | `tab.start_failure.cause == NotLoggedIn`; `is_installed()` for the stub's name is false |
| `install_dirs_include_each_clis_own_folder` | `Grok.install_dirs()` | contains `$HOME/.grok/bin` |
| `probe_and_child_path_use_one_list` | compare the two lists | equal (same function) |

Run: `cargo test -p designos-app agent_start_failure` and the `terminal` stub test. Evidence: red/green
capture and a screenshot of the failure panel for the signed-out stub (`--panel agents-start-failure`
fixture pattern, `shell.rs` `agents-start-failure`).

## 11. Performance budgets

`diagnose` is called once per failed tab; the probe adds at most six `is_file` checks. No change to the
UI frame budget.

## 12. Dependencies and build order

Independent. `terminal.rs` is the known hot spot (see README-part3): keep this change to the two
functions named in Touches.

## 13. Owner rulings

- #650: a failed start shows the real cause and one fix action; honesty over a generic message.
- `GROK_FOLDER_TRUST=0` stays (unrelated, unchanged).
