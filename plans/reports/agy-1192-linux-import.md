# DEV Report — `#1192` (P0, `v0.6.3`): Home › Import on Linux opens nothing and says nothing

## What Could Not Be Verified Here

- **Real Linux desktop chooser execution (portal running vs. portal stopped):** This worker runs on macOS (`aarch64-apple-darwin`), so live D-Bus `org.freedesktop.portal.Desktop` session-bus owner checks via `libdbus-1.so.3` and live `xdg-desktop-portal` / `zenity` window creation could not be executed against a running Linux X11/Wayland session here.
- **How it was verified instead:** The Linux readiness & timeout pipeline (`check_chooser_ready_bounded` and `dialog_outcome_bounded` in [`crates/designos-app/src/io/dialog_probe.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1192-linux-import/crates/designos-app/src/io/dialog_probe.rs)) is exercised directly on macOS through deterministic unit and `Sim` tests (`ScriptedDialogs::answering(DialogOutcome::Unavailable)`, `ScriptedDialogs::hanging()`, `portal_without_session_bus_owner_is_unavailable_and_records_backend_error`, `hanging_portal_check_times_out_within_bound_and_never_calls_show`, and `DESIGNOS_FORCE_NO_FILE_DIALOG`), and the exact Linux QA steps (portal present, then portal stopped) are stated in the draft PR body.

---

## Conclusion

Fixed `#1192` across [`crates/designos-app/src/io/dialog_probe.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1192-linux-import/crates/designos-app/src/io/dialog_probe.rs), [`crates/designos-app/src/io/file_dialog.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1192-linux-import/crates/designos-app/src/io/file_dialog.rs), [`crates/designos-app/src/home/mod.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1192-linux-import/crates/designos-app/src/home/mod.rs), [`crates/designos-app/src/home/main_view.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1192-linux-import/crates/designos-app/src/home/main_view.rs), [`crates/designos-app/src/home/dialog.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1192-linux-import/crates/designos-app/src/home/dialog.rs), [`crates/designos-app/src/ops.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1192-linux-import/crates/designos-app/src/ops.rs), [`crates/designos-app/src/shell.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1192-linux-import/crates/designos-app/src/shell.rs), and [`crates/designos-app/src/app.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1192-linux-import/crates/designos-app/src/app.rs):

1. **Never silent (`AC-1`, `AC-2`, `AC-4`, Ruling 1):**
   - Normalized `SystemDialogs::pick_many` (`rfd_dialog(request).pick_files().filter(|v| !v.is_empty())`) so `rfd 0.17.2`'s `Some(vec![])` return on Linux portal `ResponseCode::Other (2)` or non-zero `zenity` exit is classified through `dialog_outcome` instead of being mistaken for `DialogOutcome::Picked(vec![])`.
   - Added a pre-call bounded readiness check (`check_chooser_ready_bounded` / `dialog_outcome_bounded`, bounded by `MAX_DIALOG_OPEN_WAIT = 1500 ms` `< 2 s`) before calling `rfd` on Linux. On Linux (`linux_portal_ready`), it checks via `libdbus-1.so.3` (already used by `rfd`, zero new crates) with a 500 ms per-call timeout and `no_auto_start` that `org.freedesktop.portal.Desktop` has an active owner on the session bus (`org.freedesktop.DBus.NameHasOwner`) and exposes a responsive `org.freedesktop.portal.FileChooser` (`org.freedesktop.DBus.Properties.Get`), or falls back when `zenity` is on `PATH` without a session bus.
   - When a dialog cannot open (`DialogOutcome::Unavailable`), `App::show_no_file_dialog_toast` emits a `warn` log line to stderr (`designos: warn: file dialog unavailable: <reason>`, also stored in `app.last_dialog_warn`) and shows `NO_FILE_DIALOG` (`"No file dialog. Install zenity or xdg-desktop-portal."`) within `< 2 s`. A user's real cancel (`DialogOutcome::Cancelled`) remains completely silent (`AC-4`).
2. **In-app path fallback (`AC-3`, `AC-5`, Ruling 2):**
   - Added the `"Enter a path…"` action (`"toast-enter-path"`) to the `NO_FILE_DIALOG` toast in `Shell::toast`, reusing `toast_layout::layout` and existing tokens.
   - Clicking `"Enter a path…"` dismisses the toast and opens `Dialog::prompt(Intent::ImportPath, "Import file", "", "Import")`, reusing the existing modal prompt in `home/dialog.rs` with zero new visual style.
   - Submitting the prompt trims surrounding whitespace/quotes (preserving internal spaces and UTF-8 / Vietnamese diacritics) and calls `app.open_path(...)` — the exact same code path as CLI `designos <file>` (`AC-5`) — transitioning from Home into a new editor tab on success, or showing the existing open/parse error toast on failure.
3. **One dialog at a time (Ruling 3) & platform scope (Ruling 4):**
   - Added per-window `App::try_begin_dialog` (`dialog_opening: Arc<AtomicBool>`), single-frame `Act::Import` deduplication in `home::show`, and `!r.double_clicked()` on `"home-import"` so double-clicking Import or clicking again while a chooser is opening opens nothing new.
   - Non-Linux platforms (`macOS`, `Windows`) keep their native `rfd` dialogs unchanged unless `DESIGNOS_FORCE_NO_FILE_DIALOG` is explicitly set for testing.

---

## Epistemic Ledger (`H1`, `H2`, `H3`)

| ID | Claim / Hypothesis | Status | Evidence |
|---|---|---|---|
| **H3** | Home's Import does not go through `picked_path` at all. | **DISPROVEN (`FACT`)** | `"home-import"` in [`crates/designos-app/src/home/mod.rs:1286`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1192-linux-import/crates/designos-app/src/home/mod.rs#L1286) and [`crates/designos-app/src/home/main_view.rs:46`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1192-linux-import/crates/designos-app/src/home/main_view.rs#L46) pushes `Act::Import` -> `home::apply` calls `import_files_dialog` ([`home/mod.rs:580`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1192-linux-import/crates/designos-app/src/home/mod.rs#L580)) -> `app.picked_paths(outcome)` ([`file_dialog.rs:158`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1192-linux-import/crates/designos-app/src/io/file_dialog.rs#L158)). |
| **H1** | The portal call never returns (or blocks for 25 s on D-Bus activation), so the dialog hangs or misclassifies after timeout and nothing shows `NO_FILE_DIALOG`. | **CONFIRMED (`FACT`)** | In `rfd-0.17.2` (`src/backend/xdg_desktop_portal/portal/mod.rs:35,56,130-144` and `libdbus.rs:125,137`), `conn.send_and_block(&Message::open_file(opts))` calls `dbus_connection_send_with_reply_and_block(..., -1, ...)` without `NO_AUTO_START` (25 s D-Bus activation block when `org.freedesktop.portal.Desktop` has no owner), and `wait_for_response` loops on `conn.read_write(-1)` indefinitely if the `FileChooser` backend never emits `Response`. On the base code, `dialog_probe::classify` called `show()` before checking any backend and treated any `None` after `>= 300 ms` (`MIN_HUMAN_DIALOG`) when `.portal` files existed on disk as `DialogOutcome::Cancelled` (silent). |
| **H2** | `rfd` returns an empty result at once and `dialog_outcome` reads it as a cancel/pick, so no toast is shown. | **CONFIRMED VARIANT (`FACT`)** | Unlike `rfd::FileDialog::pick_file()` (`if res.is_empty() { None }` at `xdg_desktop_portal.rs:99`), `rfd::FileDialog::pick_files()` (called by `SystemDialogs::pick_many` for `Home › Import` at `xdg_desktop_portal.rs:131-138` and `zenity.rs:98`) returns `Some(vec![])` when the portal returns `ResponseCode::Other (2)` or `zenity` exits non-zero. Base `classify` matched `Some(vec![])` as `DialogOutcome::Picked(vec![])`, bypassing the `None` / `Unavailable` branch completely, and `import_into_library(&[], ...)` did nothing and showed no toast. |
| **A1** | Stripping optional surrounding `"` or `'` quotes on the in-app path prompt input is desirable when users paste paths copied from a terminal or file manager. | **`ASSUMPTION`** | Implemented in `home::dialog::finish` for `Intent::ImportPath`: strips only outer whitespace and a matching pair of outer `"..."` or `'...'` quotes while preserving all internal spaces and UTF-8 / Vietnamese diacritics. |

---

## Acceptance Table & Verification Evidence

| # | Requirement | Command / Artifact | Result |
|---|---|---|---|
| **1** | **Red first (`red.log`) then Green (`green.log`)** | `cargo test -p designos-app --bin designos --locked io::file_dialog_flow_tests` (saved to [`red.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1192-linux-import/plans/reports/agy-1192-linux-import-evidence/red.log) and [`green.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1192-linux-import/plans/reports/agy-1192-linux-import-evidence/green.log)) | **PASS** (`red.log`: 5 new tests failed on base; `green.log`: 35 passed, 0 failed) |
| **2** | **Unit & UI tests for all 5 cases** | `io::file_dialog_flow_tests` + `io::dialog_probe::tests` + `shell::tests::home::import_without_a_dialog_opens_path_prompt_from_toast_and_imports_file` | **PASS** (35 + 16 + 1 passed) |
| **3a** | **`cargo fmt --all -- --check`** | `cargo fmt --all -- --check` under isolated harness | **PASS** (exit `0`) |
| **3b** | **`cargo clippy --workspace --all-targets --locked -- -D warnings`** | `cargo clippy --workspace --all-targets --locked -- -D warnings` under isolated harness | **PASS** (exit `0`, 0 warnings) |
| **3c** | **`cargo test --workspace --locked --lib`** | `cargo test --workspace --locked --lib` under isolated harness | **PASS** (exit `0`, all workspace lib test suites passed) |
| **3d** | **`cargo test -p designos-app --bin designos --locked`** | `cargo test -p designos-app --bin designos --locked` under isolated harness | **PASS** (`3274 passed; 0 failed; 11 ignored`) |
| **4** | **1440x900 captures of error toast + action and path field** | [`toast-no-file-dialog-1440.png`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1192-linux-import/plans/reports/agy-1192-linux-import-evidence/toast-no-file-dialog-1440.png) and [`dialog-enter-path-1440.png`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1192-linux-import/plans/reports/agy-1192-linux-import-evidence/dialog-enter-path-1440.png) | **PASS** (`1440x900` PNGs captured & visually inspected) |

### Pasted Command Outputs

#### 1. `red.log` vs `green.log` (`io::file_dialog_flow_tests`)

```text
# Red phase (plans/reports/agy-1192-linux-import-evidence/red.log):
failures:
    io::file_dialog_flow_tests::other_dialog_sites::home_import_double_click_opens_one_chooser
    io::file_dialog_flow_tests::other_dialog_sites::home_import_fallback_path_opens_fixture_like_cli
    io::file_dialog_flow_tests::other_dialog_sites::home_import_fallback_path_with_spaces_and_vietnamese_diacritics_works
    io::file_dialog_flow_tests::other_dialog_sites::home_import_hanging_chooser_times_out_with_toast_and_warns
    io::file_dialog_flow_tests::other_dialog_sites::home_import_says_so_and_imports_nothing

test result: FAILED. 30 passed; 5 failed; 0 ignored; 0 measured; 3247 filtered out; finished in 0.39s

# Green phase (plans/reports/agy-1192-linux-import-evidence/green.log):
running 35 tests
test io::file_dialog_flow_tests::other_dialog_sites::home_import_double_click_opens_one_chooser ... ok
test io::file_dialog_flow_tests::other_dialog_sites::home_import_says_so_and_imports_nothing ... ok
test io::file_dialog_flow_tests::other_dialog_sites::home_import_is_silent_on_cancel ... ok
test io::file_dialog_flow_tests::other_dialog_sites::home_import_hanging_chooser_times_out_with_toast_and_warns ... ok
test io::file_dialog_flow_tests::other_dialog_sites::home_import_fallback_path_with_spaces_and_vietnamese_diacritics_works ... ok
test io::file_dialog_flow_tests::other_dialog_sites::home_import_fallback_path_opens_fixture_like_cli ... ok
...
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 3250 filtered out; finished in 0.19s
```

#### 2. `io::dialog_probe::tests` & `shell::tests::home::import_without_a_dialog_opens_path_prompt_from_toast_and_imports_file`

```text
running 16 tests
test io::dialog_probe::tests::portal_without_session_bus_owner_is_unavailable_and_records_backend_error ... ok
test io::dialog_probe::tests::hanging_portal_check_times_out_within_bound_and_never_calls_show ... ok
...
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 3269 filtered out; finished in 0.10s

running 1 test
test shell::tests::home::import_without_a_dialog_opens_path_prompt_from_toast_and_imports_file ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3284 filtered out; finished in 0.25s
```

#### 3. Mandatory macOS Gates (`fmt`, `clippy`, `--workspace --lib`, `--bin designos`, `zero_fake_ui_gate.py`, `check_spacing_relationships.py`)

```text
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo fmt --all -- --check
(exit 0)

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings
(exit 0)

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test --workspace --locked --lib
test result: ok. (all workspace lib crates passed, exit 0)

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked
running 3285 tests
...
test result: ok. 3274 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 362.27s

$ python3 scripts/zero_fake_ui_gate.py
Gate Verdict: PASS (17/17 invariants satisfied)

$ python3 scripts/check_spacing_relationships.py
PASS: no count rose above the baseline
```

---

## How the Forced-Unavailable Captures Were Produced on macOS

We added an environment test hook `DESIGNOS_FORCE_NO_FILE_DIALOG` in [`crates/designos-app/src/io/dialog_probe.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1192-linux-import/crates/designos-app/src/io/dialog_probe.rs#L35-L42) and [`crates/designos-app/src/shell.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1192-linux-import/crates/designos-app/src/shell.rs#L182-L188):
- Setting `DESIGNOS_FORCE_NO_FILE_DIALOG=toast` makes `dialog_outcome` return `DialogOutcome::Unavailable` (recording `"forced unavailable via DESIGNOS_FORCE_NO_FILE_DIALOG"`) and invokes `crate::home::import_files_dialog(&mut app, &crate::io::SystemDialogs)` on `--home` startup:
  ```bash
  DESIGNOS_FORCE_NO_FILE_DIALOG=toast out/cargo/debug/designos --home --size 1440x900 --screenshot plans/reports/agy-1192-linux-import-evidence/toast-no-file-dialog-1440.png
  ```
- Setting `DESIGNOS_FORCE_NO_FILE_DIALOG=dialog` opens the `"Import file"` fallback prompt (`app.open_import_path_dialog()`) on `--home` startup:
  ```bash
  DESIGNOS_FORCE_NO_FILE_DIALOG=dialog out/cargo/debug/designos --home --size 1440x900 --screenshot plans/reports/agy-1192-linux-import-evidence/dialog-enter-path-1440.png
  ```

---

## Skills Loaded & How They Changed the Work

1. **`es:dev` / `es:feature-dev`**: Enforced reading `.project-agent.md`, `CONTEXT.md`, and `.brv/context-tree/session-handoffs/latest.md` first, plus keeping all filesystem/dlopen probing inside allowlisted modules (`crates/designos-app/src/io/dialog_probe.rs`) so `agent_session_files::fs_sites_scan_tests` stays green.
2. **`es:rust` & `es:rust-gpu-canvas`**: Kept `dialog_opening` per-`App` instance (`Arc<AtomicBool>`) and `LAST_BACKEND_ERROR` thread-local instead of a process-global lock after observing parallel test contention in `io::file_dialog_flow_tests`.
3. **`es:designer`**: Reused `toast_layout::layout` (with `"Enter a path…"`, `<= 3` words per `design/ux-writing.md` / `Z16`) and `home::Dialog::prompt` with zero new spacing/radius/font literals so `zero_fake_ui_gate.py` (17/17) and `check_spacing_relationships.py` pass without baseline drift.
4. **`es:fable-thinking` & `es:astra-work`**: Used falsification-first root-cause analysis on `rfd 0.17.2` and `dialog_probe.rs` (proving both H1's D-Bus hang/25s timeout misclassification and H2's `pick_files() -> Some(vec![])` silent return), ran Red-first (`red.log`) before enabling the production fix (`green.log`), and re-modeled `dialog_opening` from global to per-window when parallel tests exposed contention.

---

## Weakest Link

- **Weakest link:** On Linux, if `org.freedesktop.portal.Desktop` is owned on the session bus and answers `Properties.Get("org.freedesktop.portal.FileChooser", "version")` within 500 ms, `rfd` proceeds to call `OpenFile` and then waits in `conn.read_write(-1)` for the `org.freedesktop.portal.Request.Response` signal (which cannot be given a 2 s wall-clock timeout without cutting off a human browsing files in a working chooser). If a portal backend crashes *after* answering `Properties.Get` and *after* returning a `Request` handle without ever emitting `Response`, `rfd`'s `wait_for_response` would still wait on the signal.
- **Mitigation:** Checking both `NameHasOwner("org.freedesktop.portal.Desktop")` and `Properties.Get("org.freedesktop.portal.FileChooser", "version")` with `no_auto_start` and a 500 ms timeout before calling `rfd` catches missing/stopped portals and missing/unregistered `FileChooser` backends up front, and normalizing `pick_files()`'s `Some(vec![])` catches portals that answer `OpenFile` with `ResponseCode::Other (2)`.

---

## Unresolved Questions

- None.

Status: READY FOR REVIEW (all macOS gates green; red/green logs and 1440px captures saved; Linux live-portal QA checklist in draft PR body).
