# Report — DEV · #1095: the Export panel writes a PNG silently to `~` with no dialog and no toast

## 1. What Could NOT Be Verified
- **Post-click panel Export toast in a headless `designos --screenshot` CLI run:** `designos --screenshot` (`crates/designos-app/src/shell.rs:94-406`, `crates/designos-app/src/panels/properties.rs:2273`) loads the document, applies startup flags (`--select`, `--panel`, `--menu`), renders until `shot.frames >= 12`, and saves the frame. It has no CLI flag or `--panel` preset that adds an Export setting and clicks `Export <layer>` in the Properties panel during a `--screenshot` run, and the task's `Files you may modify` boundary prohibits adding new CLI flags in `cli.rs` or `properties.rs`. Per Acceptance 5 (*"If the toast cannot be captured by `--screenshot`, say why and attach the test output"*), we captured the 1440×900 `--screenshot` on a scratch copy of the Untitled UI kit (`plans/reports/agy-1095-panel-export-png-evidence/panel-export-capture.png`) and verified the full post-click panel Export toast, `Show in folder` (`toast-reveal`) button rect, and `#937` 10-second stacking behavior end-to-end in `Sim` (`plans/reports/agy-1095-panel-export-png-evidence/green.log`).
- **Live native OS file dialog (`rfd::FileDialog`) and OS file manager (`open -R` / `xdg-open` / `explorer /select,`) windows:** Per the hermetic test rules, `cargo test` stubs out native `rfd` modal windows (`DialogOutcome::Unavailable` in `SystemDialogs`) and file-manager windows (`App::reveal_file`). Dialog-picked single-file and folder exports are verified via `Picked` / `ScriptedDialogs` tests, and `Show in folder` click dispatch is verified via a stub `App::reveal_file` callback in `Sim`.

---

## 2. Epistemic Ledger, Weakest Link & Skills Loaded

### Epistemic Ledger
- **`FACT` (`crates/designos-app/src/io/fallback_save.rs:13-20`):** Prior to this fix, `fallback_dir_in(home: &Path)` returned `home.to_path_buf()` (`~`) whenever `home.join("Downloads").is_dir()` was `false`. On headless Linux Xvfb (`#1095` QA report `V-06`) and in clean `$HOME` test environments (`mkdir -p "$S/home"`), `~/Downloads` did not exist yet, so `fallback_dir_in` silently returned `~` and wrote `~/Pasted image.png` (`~/Hero.png`) directly into `~`.
- **`FACT` (`crates/designos-app/src/panels/export.rs:515-521`, `crates/designos-app/src/io/file_dialog.rs:173-182`, `crates/designos-app/src/ops.rs:2277-2280`):** Prior to this fix, `run_export_with`, `save_without_dialog`, and `export_selection_with` called `app.show_toast(...)` instead of attaching `toast.reveal = Some(path)` and `theme::TOAST_REVEAL_SECONDS` (`10.0` seconds), so their toasts had no `Show in folder` button and expired after `4.0` seconds.
- **`FACT` (`crates/designos-app/src/panels/export.rs:507`, `crates/designos-app/src/io/file_dialog.rs:178`, `crates/designos-app/src/ops.rs:2279`):** Prior to this fix, write errors formatted `std::io::Error` directly (`"Export failed: {e}"`), which in Rust's standard library contains only the OS error message without the failed file path.
- **`FACT` (`crates/designos-app/src/app.rs:338-357`, `crates/designos-app/src/shell.rs:997-1031`):** Prior to this fix, `App` had a single `toast` slot with no queue, so any subsequent plain `show_toast` call within 10 seconds of an export toast overwrote the active export toast and its `Show in folder` button (`#937`).
- **`INFERENCE`:** Ensuring `fallback_dir_in(home)` always targets `home.join("Downloads")` (creating `Downloads` when missing and never falling back to `~`), routing write errors through `App::show_export_write_error`, attaching `reveal` via `App::show_toast_with_reveal_status`, and queueing plain toasts behind an active unexpired reveal toast (`#937`) eliminates all three failure modes across both single-file and multi-setting exports.
- **`ASSUMPTION`:** None remaining; all four `Sim` acceptance scenarios were falsified on the base (`red.log`) and verified green on the head (`green.log`).

### Weakest Link
- **`Sink::Dir` multi-file `Show in folder` target:** When multiple export settings (e.g., `PNG` and `SVG`) are written to a folder (`~/Downloads` or a dialog-chosen folder), `toast.reveal` holds the first written file (`written[0]`), so clicking `Show in folder` selects that file inside the destination folder on macOS/Windows (`open -R` / `explorer /select,`) and opens its parent folder (`~/Downloads`) on Linux (`xdg-open`).

### How the Existing Helper Handles Several Files at Once (PNG + SVG)
- In `crates/designos-app/src/panels/export.rs:463-529`, `plan_export` builds one `ExportJob` per `(node, export_setting)` pair (merging selected frames on a shared `PDF` row into a single multi-page PDF job).
- When `jobs.len() == 1` (`[job]`), `run_export_with` requests a single-file save dialog (`FileRequest::save(job.file.as_str()).filter(...)`).
- When `jobs.len() > 1` (for example, `PNG` and `SVG` settings on the same layer, or multiple selected layers), `run_export_with` requests a folder dialog (`FileRequest::folder("Choose export folder")`).
  - If a folder is picked, `Sink::Dir { dir, keep_existing: false }` writes each job into `dir` and shows `"Exported <N> files to <dir>"` with `Show in folder` (`toast.reveal = Some(written[0].clone())`).
  - If no dialog can open (`DialogOutcome::Unavailable`), it resolves `dialogs.fallback_dir()` (`~/Downloads`, never `~`), writes each file via `crate::io::write_new_in(&dir, &job.file, &bytes)` (never overwriting existing files), and shows `"No save dialog. Saved <N> files to <dir>"` with `Show in folder` (`toast.reveal = Some(written[0].clone())`).

### Skills Loaded & Changes Made Because of Them
- **`/es:dev` -> `es:feature-dev`:** Followed the DEV phase directly from the controller-approved spec without stopping for brainstorm/plan approval or spawning a verifier.
- **`es:fable-thinking` & `es:astra-work`:** Formulated explicit `PREDICT` checks before each command, built the `FACT`/`INFERENCE`/`ASSUMPTION` ledger, and verified red-first `Sim` failures (`red.log`) on the base product logic before implementing the fix (`green.log`).
- **`es:rust` & `es:rust-gpu-canvas`:** Used a thread-local test home override (`SystemDialogs::with_test_home`) instead of mutating process-global `HOME` across multi-threaded Rust tests, kept `VecDeque<Toast>` zero-overhead on idle frames, and preserved all GPU/canvas invariants untouched.
- **`es:designer`:** Preserved the exact toast layout, `Show in folder` (`REVEAL_LABEL`) action button geometry, and `TOAST_REVEAL_SECONDS` (10s) timing tokens from `theme.rs` and `toast_layout.rs`.
- **`es:orca-native-orchestration`:** Sent periodic Orca heartbeats and checked the Orca inbox at every natural checkpoint.

---

## 3. Acceptance Table

| # | Check | Command | Result |
|---|---|---|---|
| 1 | Red-first `Sim` tests (`red.log` on base, `green.log` on head) | `env HOME="$S/home" TMPDIR="$S/" ... cargo test -p designos-app --bin designos --locked export` | **PASS** (`red.log`: 4 `shell::tests::panel_export::*` tests failed on base; `green.log`: `93 passed; 0 failed`) |
| 2 | Format & Clippy | `cargo fmt --all -- --check && cargo clippy -p designos-app --all-targets --locked -- -D warnings` | **PASS** (exit `0`, 0 warnings) |
| 3 | Full binary tests under harness | `env HOME="$S/home" TMPDIR="$S/" ... cargo test -p designos-app --bin designos --locked` | **PASS** (`2947 passed; 0 failed; 8 ignored` in `341.33s`) |
| 4 | Zero-fake-UI gate | `python3 scripts/zero_fake_ui_gate.py` | **PASS** (`17/17 invariants satisfied`) |
| 5 | Visual evidence (`1440×900` on copy of Untitled UI kit + test output) | `designos plans/reports/agy-1095-panel-export-png-evidence/untitled-ui-copy.fig --select "Text" --size 1440x900 --screenshot plans/reports/agy-1095-panel-export-png-evidence/panel-export-capture.png` | **PASS** (`PNG image data, 1440 x 900, 8-bit/color RGBA, non-interlaced`, scratch `.fig` copy removed) |

### Pasted Output — Acceptance 1 (`red.log` excerpt on base)
```text
failures:

---- shell::tests::panel_export::panel_export_failed_write_shows_error_toast_with_path_and_reason stdout ----

thread 'shell::tests::panel_export::panel_export_failed_write_shows_error_toast_with_path_and_reason' (55206971) panicked at crates/designos-app/src/shell/tests/panel_export.rs:161:9:
failed write to Downloads must never fall back to ~

---- shell::tests::panel_export::panel_export_multiple_settings_writes_all_files_to_fallback_and_shows_reveal_toast stdout ----

thread 'shell::tests::panel_export::panel_export_multiple_settings_writes_all_files_to_fallback_and_shows_reveal_toast' (55206972) panicked at crates/designos-app/src/shell/tests/panel_export.rs:206:9:
both PNG and SVG settings must be written to ~/Downloads

---- shell::tests::panel_export::panel_export_never_overwrites_an_existing_file stdout ----

thread 'shell::tests::panel_export::panel_export_never_overwrites_an_existing_file' (55207136) panicked at crates/designos-app/src/shell/tests/panel_export.rs:141:9:
assertion `left == right` failed
  left: None
 right: Some("/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.INeaci/designos_panel_export_13873_no_overwrite/Downloads/Hero 2.png")

---- shell::tests::panel_export::panel_export_without_dialog_writes_to_fallback_folder_not_home_and_shows_success_toast_with_full_path stdout ----

thread 'shell::tests::panel_export::panel_export_without_dialog_writes_to_fallback_folder_not_home_and_shows_success_toast_with_full_path' (55208282) panicked at crates/designos-app/src/shell/tests/panel_export.rs:93:9:
panel Export must never write silently to ~ (/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.INeaci/designos_panel_export_13873_fallback_not_home/Hero.png)

test result: FAILED. 89 passed; 4 failed; 1 ignored; 0 measured; 2861 filtered out; finished in 2.91s
```

### Pasted Output — Acceptance 1 (`green.log` excerpt on head)
```text
test shell::tests::panel_export::panel_export_failed_write_shows_error_toast_with_path_and_reason ... ok
test shell::tests::panel_export::panel_export_never_overwrites_an_existing_file ... ok
test shell::tests::panel_export::panel_export_multiple_settings_writes_all_files_to_fallback_and_shows_reveal_toast ... ok
test shell::tests::panel_export::panel_export_without_dialog_writes_to_fallback_folder_not_home_and_shows_success_toast_with_full_path ... ok

test result: ok. 93 passed; 0 failed; 1 ignored; 0 measured; 2861 filtered out; finished in 1.56s
```

### Pasted Output — Acceptance 2 (`cargo fmt` & `cargo clippy`)
```text
$ cargo fmt --all -- --check
$ cargo clippy -p designos-app --all-targets --locked -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 51.40s
```

### Pasted Output — Acceptance 3 (Full `designos` test binary under harness)
```text
running 2955 tests
...
test shell::tests::panel_export::panel_export_failed_write_shows_error_toast_with_path_and_reason ... ok
test shell::tests::panel_export::panel_export_multiple_settings_writes_all_files_to_fallback_and_shows_reveal_toast ... ok
test shell::tests::panel_export::panel_export_never_overwrites_an_existing_file ... ok
test shell::tests::panel_export::panel_export_without_dialog_writes_to_fallback_folder_not_home_and_shows_success_toast_with_full_path ... ok
...
test result: ok. 2947 passed; 0 failed; 8 ignored; 0 measured; 0 filtered out; finished in 341.33s
```

### Pasted Output — Acceptance 4 (`python3 scripts/zero_fake_ui_gate.py`)
```text
------------------------------------------------------------------------------------------------------------
 Gate Verdict: PASS (17/17 invariants satisfied)
============================================================================================================
```

---

## 4. Changes Per File
- **`crates/designos-app/src/io/fallback_save.rs`:** Updated `fallback_dir_in(home: &Path)` to always return `home.join("Downloads")` (creating `Downloads` when missing and never falling back to `~`), and updated its unit test `the_fallback_is_always_downloads_and_never_home`.
- **`crates/designos-app/src/io/file_dialog.rs`:** Made `SystemDialogs::pick` and `SystemDialogs::pick_many` return `DialogOutcome::Unavailable` under `cfg!(test)` (so `Sim` tests never open blocking native OS modal dialogs), added `#[cfg(test)] SystemDialogs::with_test_home` for thread-safe per-test home directories, added shared `App::show_export_write_error`, and updated `App::save_without_dialog` to attach `Show in folder` (`show_toast_with_reveal_status`) on write and report the target path + OS reason on failure.
- **`crates/designos-app/src/app.rs`:** Added `toast_queue: VecDeque<Toast>` so plain toasts arriving while an action toast (`reveal.is_some()`) is active on screen queue behind it instead of replacing it (`#937`), and added `App::show_toast_with_reveal_status`.
- **`crates/designos-app/src/shell.rs`:** Popped queued toasts in `Shell::toast` when the active toast expires, and cleared `self.app.toast` before reporting a failed `Show in folder` click so the error toast displays immediately.
- **`crates/designos-app/src/ops.rs`:** Updated `App::export_selection_with` to use `show_toast_with_reveal` on success and `show_export_write_error` on failure.
- **`crates/designos-app/src/panels/export.rs`:** Recorded `"title-action:Export"`, `"export-add"`, and `"export-btn"` via `widgets::record`, returned `(PathBuf, std::io::Error)` from `Sink::write` to call `app.show_export_write_error(without_dialog, &path, &e)` on failure, and used `app.show_toast_with_reveal_status` for single-file and multi-file export success toasts.
- **`crates/designos-app/src/shell/tests.rs` & `crates/designos-app/src/shell/tests/panel_export.rs`:** Added end-to-end `Sim` tests for the Properties panel's Export section covering fallback `Downloads` writes (not `~`), non-overwrite numbering, failed write error toasts with path and reason, multi-setting (`PNG` + `SVG`) exports, `Show in folder` (`toast-reveal`), and `#937` toast lifetime/stacking.

---

## 5. Every User-Facing String (Verbatim)
- `"No save dialog. "`
- `"No save dialog. Saved to {}"`
- `"Saved to {}{skipped}"` (prefixed with `"No save dialog. "` when `without_dialog` is true -> `"No save dialog. Saved to {}{skipped}"`)
- `"Exported {}{skipped}"`
- `"Saved {} files to {}{skipped}"` (prefixed with `"No save dialog. "` when `without_dialog` is true -> `"No save dialog. Saved {} files to {}{skipped}"`)
- `"Exported {} files to {}{skipped}"`
- `"Exported {} files{skipped}"`
- `"{lead}Export failed: {}: {err}"` (`"No save dialog. Export failed: {}: {err}"` or `"Export failed: {}: {err}"`)
- `"Show in folder"` (existing `REVEAL_LABEL` in `crates/designos-app/src/shell.rs`)

Status: DONE
