# Report — DEV · #1196: Export tells the user where the file went and never overwrites silently

## 1. What Could NOT Be Verified (First)
- **Live native OS file dialog (`rfd::FileDialog`) and native OS file-manager windows (`open -R` on macOS, `explorer /select,` on Windows, `xdg-open` on Linux):** Per the hermetic test rules, `cargo test` stubs out native `rfd` modal windows (`DialogOutcome::Unavailable` in `SystemDialogs`) and refuses to spawn a real file manager in `fig_export::reveal_file` (`if cfg!(test) { return Err(...) }`). Dialog-picked single-file and folder exports are verified via `ScriptedDialogs::answering(DialogOutcome::Picked(...))`, OS command construction is verified in `fig_export::tests::reveal_command_selects_the_file_in_each_file_manager`, and clicking `Show in folder` (`toast-reveal`) is verified via stubbed `App::reveal_file` callbacks in `Sim`.

---

## 2. Which ACs the Carried Commits Already Met vs. What Was Added

### Carried Commits (`d3960d2f` -> `b65b4d78`, `9c83a9ff` -> `73563b5c4`, `2589eb8f` -> `e5810170`)
- **AC-1 (Already met by `d3960d2f` + `9c83a9ff`):**
  - `run_export_with` (`crates/designos-app/src/panels/export.rs`), `save_without_dialog` (`crates/designos-app/src/io/file_dialog.rs`), and `export_selection_with` (`crates/designos-app/src/ops.rs`) show a toast immediately after export naming the saved path (`"Exported <path>"` or `"No save dialog. Saved to <path>"`).
  - `9c83a9ff` starts a toast's countdown on its first paint (`Toast.armed` / `Toast.duration` in `crates/designos-app/src/shell.rs:1088-1092`), holds the toast during `--screenshot` capture (`hold_for_capture`), and adds `--panel export-toast` (`crates/designos-app/src/cli.rs`, `crates/designos-app/src/panels/properties.rs:2388-2403`).
- **AC-2 (Mostly met by `d3960d2f`; Linux folder reveal edge case added in `09885e3a`):**
  - `show_export_toast` attaches `toast.reveal = Some(path)` (`Show in folder` / `toast-reveal` widget in `crates/designos-app/src/shell.rs:1169`), wired through `App::reveal_file` (`fig_export::reveal_file`), which builds `open -R <file>` on macOS, `explorer /select,<file>` on Windows, and `xdg-open <dir>` on Linux (`crates/designos-app/src/fig_export.rs:72-80`), and shows an error toast (`"Cannot open the folder: …"`) without panicking if the launcher fails.
  - **Added in `09885e3a`:** When multiple files are exported at once, `toast.reveal` is the destination directory `dir` (`panels/export.rs:532`). On Linux, `reveal_command("linux", file)` previously passed `file.parent().unwrap_or(file)` to `xdg-open`, which opened the parent of `dir` when `file` was already an existing directory. Updated `reveal_command` (`crates/designos-app/src/fig_export.rs:77-79`) to pass `file` directly when `file.is_dir()`.
- **AC-3 (Already met by `d3960d2f`):**
  - `App::show_export_write_error` (`crates/designos-app/src/io/file_dialog.rs:154-160`) shows an error toast (`error = true`, `reveal = None`) with `"{lead}Export failed: {}: {err}"` naming both the path tried and the OS error reason.
- **Several exports at once (Already met by `d3960d2f`):**
  - `run_export_with` (`crates/designos-app/src/panels/export.rs:522-533`) shows one summary toast (`"Exported <N> files to <dir>"` or `"No save dialog. Saved <N> files to <dir>"`) with `toast.reveal = Some(dir)`.

### Added in `cdece794` (`test(export)`) + `09885e3a` (`fix(export)`)
- **AC-4 (Numeric suffix `Card-1.png`, `Card-2.png`, … and never overwriting on any export path):**
  - `d3960d2f` only avoided overwriting in the no-dialog fallback folder (`Sink::Dir { dir, keep_existing: true }`), where `write_new_in` used space-separated suffixes starting at 2 (`Card 2.png`, `Card 3.png`), while dialog-picked single-file exports (`Sink::File(path)`) and dialog-picked folder exports (`Sink::Dir { dir, keep_existing: false }`) called `std::fs::write` and overwrote existing files silently.
  - Added `write_new_dashed_in` and `write_new_dashed_path` in `crates/designos-app/src/io/fallback_save.rs:58-70` (using `step = 0` -> `"{stem}.{ext}"`, `step = n` -> `"{stem}-{n}.{ext}"`, i.e., `Card.png` -> `Card-1.png` -> `Card-2.png`) and routed both `Sink::File(path)` and `Sink::Dir(dir)` in `crates/designos-app/src/panels/export.rs:433-438` through them so panel exports never overwrite existing files and always report the final suffixed path in the toast.
- **Unsafe file names (`/` and `:` become `-`):**
  - Added `sanitize_export_name` in `crates/designos-app/src/io/fallback_save.rs:37-41` (mapping `'/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'` and control characters to `'-'` before `designos_export::sanitize_file_name`) and wired it into `preset_file_name`, `plan_export`, and `write_new_dashed_in`.
- **Pre-frame reveal toast preservation (`crates/designos-app/src/app.rs:375`):**
  - Removed the `self.now > 0.0` guard when queueing a plain toast behind an unarmed reveal toast (`!t.armed || self.now < t.until`) so a plain toast raised before Frame 1 sets `self.app.now` (such as `poll_fonts` or `--panel export-toast` startup) cannot overwrite an active export reveal toast.

---

## 3. Drift (`spec says / HEAD says`)
- **Spec says:** `Properties › Export › Export <name> writes the file (e.g. ~/Card.png, with correct content at 300×182) straight to $HOME. There is no save dialog, toast or path shown, so the user can't find the file.`
  - **HEAD says:** Prior to `#1095` (`7e15ddaef`), `fallback_dir_in` (`crates/designos-app/src/io/fallback_save.rs:13-19`) fell back to `$HOME` when `~/Downloads` did not exist, and `run_export_with` (`crates/designos-app/src/panels/export.rs`) did not attach `toast.reveal`. On `release/v0.7.0` (`1b33582b8`) + carried commits (`b65b4d78`, `73563b5c4`, `e5810170`), fallback exports go to `~/Downloads` and attach `Show in folder`, and on final commit `09885e3a`, `Sink::File` and `Sink::Dir` both write via `write_new_dashed_path` / `write_new_dashed_in` (`Card-1.png`, `Card-2.png`) and sanitize `/` and `:` to `-`.
- **Spec says:** `AC-5 (optional, decide at G2): a save dialog or a remembered export folder setting.`
  - **HEAD says:** Per controller ruling, AC-5 is out of scope (no new save dialog or setting).

---

## 4. Root Cause (`file:line` Evidence), Epistemic Ledger & Weakest Link

### Root Cause
1. **`crates/designos-app/src/panels/export.rs:446-464` (before `09885e3a`):** `Sink::File(path)` and `Sink::Dir { dir, keep_existing: false }` called `std::fs::write` directly, silently overwriting existing files whenever a file or folder path was chosen, while `Sink::Dir { dir, keep_existing: true }` called `crate::io::write_new_in`, which formatted collisions as `"{stem} {n}.{ext}"` (`n >= 2`, e.g. `Card 2.png`) instead of `Card-1.png`, `Card-2.png`.
2. **`crates/designos-app/src/panels/export.rs:86-94` & `crates/designos-export/src/lib.rs:148-156`:** `preset_file_name` delegated directly to `designos_export::sanitize_file_name`, which replaces `/` and `:` with `'_'` instead of `'-'`.
3. **`crates/designos-app/src/fig_export.rs:77` (before `09885e3a`):** `reveal_command("linux", file)` always passed `file.parent().unwrap_or(file)` to `xdg-open`, even when `file` was already the destination directory (`dir`) of a multi-file export summary toast.
4. **`crates/designos-app/src/app.rs:375` (before `09885e3a`):** `App::show_toast` checked `self.now > 0.0` before queueing a plain toast behind an unarmed reveal toast (`!t.armed`), allowing a plain toast raised at `self.now == 0.0` (before the first `draw_window` frame) to overwrite a preset or startup export toast.

### Epistemic Ledger
- **`FACT` (`crates/designos-app/src/io/fallback_save.rs:37-70`, `crates/designos-app/src/panels/export.rs:86-94,425-440`):** Observed in `red.log` (`37 passed; 7 failed`) that prior to `09885e3a`, panel exports produced `Alpha 2.png` / `Beta 2.png` / `Hero 2.png` and `Card_A_B.png`, and `DialogOutcome::Picked` overwrote `Card.png`. Observed in `green.log` (`44 passed; 0 failed`) that after `09885e3a`, existing files are never overwritten and receive `-1`, `-2` suffixes (`Card-1.png`, `Card-2.png`, `Hero-1.png`, `Hero-2.png`), `/` and `:` become `-` (`Card-A-B.png`, `Hero-Sub-1.png`), and the toast displays and reveals the final path.
- **`FACT` (`plans/reports/agy-1196-evidence/export-toast-1440.png`):** Observed at 1440×900 on a temporary copy of `crates/designos-app/assets/starter/designos-studio-starter.designos` (`HIDIdleTime` = 213s and 217s >= 120s) that the export toast renders above the bottom toolbar beside the open Design inspector panel with the full saved path and `Show in folder` button.
- **`INFERENCE`:** Routing all `panels/export.rs` writes through `write_new_dashed_path` / `write_new_dashed_in` (`OpenOptions::new().write(true).create_new(true)`) guarantees atomic non-overwrite behavior across both dialog-picked and fallback exports while keeping `fs_sites_allowlist.txt` in sync.
- **`ASSUMPTION`:** The instruction line `Append ## Round 2 to plans/reports/agy-backmerge-065-into-070.md` in the worker template was a leftover from a prior backmerge task template (that file does not exist in `plans/reports/`); the canonical report path for this task is `plans/reports/agy-1196-export-feedback.md`.

### Weakest Link
- Native OS file-manager execution (`open -R` / `explorer /select,` / `xdg-open`) is verified at the command-construction and callback-dispatch boundary rather than by spawning a live Finder window during automated tests.

---

## 5. Skills Loaded & What Changed Because of Them
- **`/es:dev` -> `es:feature-dev`:** Executed the DEV phase directly against the approved spec and controller rulings without stopping for brainstorm/plan approval or spawning a verifier.
- **`es:fable-thinking` & `es:astra-work`:** Stated explicit `PREDICT` expectations before consequential commands, verified all 7 new/updated assertions red-first (`red.log`) before writing production code (`green.log`), and traced the duplicate-layer batch interaction between `plan_export` and `write_new_dashed_in` so two layers named `Card` exported into a folder that already has `Card.png` produce `Card-1.png` and `Card-2.png` rather than `Card-1-1.png`.
- **`es:rust` & `es:rust-gpu-canvas`:** Kept `OpenOptions::new().write(true).create_new(true)` consolidated in `io/fallback_save.rs::write_unique_in` so `fs_sites_scan_tests` stays exact after removing direct `std::fs::write` calls from `panels/export.rs`.
- **`es:designer`:** Verified the 1440×900 visual capture (`plans/reports/agy-1196-evidence/export-toast-1440.png` and `before-no-toast-1440.png`) against the spacing and zero-fake-UI gates (`17/17 invariants satisfied`).
- **`es:orca-native-orchestration`:** Checked Orca mail at every checkpoint, acknowledged coordinator instructions (`delivery_541e24cbdcb9`), and sent periodic heartbeats.

---

## 6. Acceptance Table & Pasted Command Output

| # | Check | Command | Result |
|---|---|---|---|
| 1 | Red-first tests (`plans/reports/agy-1196-evidence/red.log`) | `env HOME="$S/home" TMPDIR="$S/" ... cargo test -p designos-app --bin designos --locked -- export_toast panel_export panels::export::tests reveal_command_selects_the_file_in_each_file_manager` | **PASS (Red verified)** (`37 passed; 7 failed` on `cdece794`) |
| 2 | Green tests (`plans/reports/agy-1196-evidence/green.log`) | `env HOME="$S/home" TMPDIR="$S/" ... cargo test -p designos-app --bin designos --locked -- export_toast panel_export panels::export::tests reveal_command_selects_the_file_in_each_file_manager` | **PASS** (`44 passed; 0 failed` on `09885e3a`) |
| 3 | `cargo fmt --all -- --check` | `env HOME="$S/home" TMPDIR="$S/" ... cargo fmt --all -- --check` | **PASS** (exit `0`) |
| 4 | `cargo clippy --workspace --all-targets --locked -- -D warnings` | `env HOME="$S/home" TMPDIR="$S/" ... cargo clippy --workspace --all-targets --locked -- -D warnings` | **PASS** (exit `0`, 0 clippy warnings) |
| 5 | `cargo test --workspace --locked --lib` | `env HOME="$S/home" TMPDIR="$S/" ... cargo test --workspace --locked --lib` | **PASS** (all 10 lib test suites passed, 0 failed) |
| 6 | `cargo test -p designos-app --bin designos --locked` | `env HOME="$S/home" TMPDIR="$S/" ... cargo test -p designos-app --bin designos --locked` | **PASS** (`3474 passed; 0 failed; 12 ignored` in `299.69s`) |
| 7 | `python3 scripts/zero_fake_ui_gate.py` & `python3 scripts/check_spacing_relationships.py` | `python3 scripts/zero_fake_ui_gate.py && python3 scripts/check_spacing_relationships.py` | **PASS** (`17/17 invariants satisfied`; `PASS: no count rose above the baseline`) |
| 8 | Visual evidence at 1440×900 on copy of starter (`HIDIdleTime` = 213s, 217s) | `out/cargo/debug/designos "$S/starter-copy.designos" --size 1440x900 --panel export-toast --screenshot plans/reports/agy-1196-evidence/export-toast-1440.png` | **PASS** (`1440 x 900, 8-bit/color RGBA`) |

### Pasted Output — 1. Red-First (`plans/reports/agy-1196-evidence/red.log`)
```text
failures:
    fig_export::tests::reveal_command_selects_the_file_in_each_file_manager
    panels::export::tests::export_toast_panel_dialog_picked_file_and_folder_never_overwrite_and_use_dash_suffix
    panels::export::tests::export_toast_panel_one_file_without_a_dialog_offers_reveal_and_keeps_a_slash_name_inside
    panels::export::tests::one_file_in_the_fallback_folder_never_overwrites
    panels::export::tests::several_files_without_a_dialog_all_land_in_the_fallback_folder
    shell::tests::export_toast::export_toast_preset_survives_a_plain_toast_before_the_first_frame
    shell::tests::panel_export::panel_export_never_overwrites_an_existing_file

test result: FAILED. 37 passed; 7 failed; 0 ignored; 0 measured; 3442 filtered out; finished in 3.41s
```

### Pasted Output — 2. Green (`plans/reports/agy-1196-evidence/green.log`)
```text
running 44 tests
test panels::export::tests::every_format_is_in_the_dropdown_and_round_trips_to_the_stored_format ... ok
test fig_export::tests::reveal_command_selects_the_file_in_each_file_manager ... ok
test panels::export::tests::fingerprint_changes_with_the_subtree ... ok
test panels::export::tests::format_labels_fit_their_dropdown ... ok
test io::file_dialog_flow_tests::export_toast_menu_read_only_folder_names_the_path_and_the_reason ... ok
test panels::export::tests::export_cancel_is_silent_and_writes_nothing ... ok
test panels::export::tests::export_without_a_dialog_or_a_fallback_folder_still_says_so ... ok
test panels::export::tests::multi_selection_shows_and_edits_shared_rows_only ... ok
test io::file_dialog_flow_tests::export_toast_menu_picked_path_offers_reveal ... ok
test panels::export::tests::oversized_frame_in_a_pdf_row_reports_an_error ... ok
test panels::export::tests::export_toast_panel_read_only_folder_names_the_path_and_the_reason ... ok
test io::file_dialog_flow_tests::export_toast_menu_without_a_dialog_offers_reveal ... ok
test panels::export::tests::scale_strings_round_trip_through_constraints ... ok
test panels::export::tests::export_toast_panel_one_file_without_a_dialog_offers_reveal_and_keeps_a_slash_name_inside ... ok
test panels::export::tests::export_toast_panel_dialog_file_offers_reveal ... ok
test panels::export::tests::one_file_without_a_save_dialog_lands_in_the_fallback_folder_and_says_so ... ok
test panels::export::tests::pdf_rows_mix_with_image_rows_and_repeat_rows_add_no_page ... ok
test panels::export::tests::a_very_long_layer_name_still_saves_to_the_fallback_folder ... ok
test panels::export::tests::one_file_in_the_fallback_folder_never_overwrites ... ok
test panels::export::tests::selected_frames_with_pdf_rows_export_as_one_multi_page_file ... ok
test panels::export::tests::export_writes_where_the_dialog_picked ... ok
test panels::export::tests::export_toast_panel_many_files_offer_one_folder_reveal ... ok
test panels::export::tests::several_files_without_a_dialog_all_land_in_the_fallback_folder ... ok
test panels::export::tests::export_toast_panel_dialog_picked_file_and_folder_never_overwrite_and_use_dash_suffix ... ok
test panels::export::tests::plus_adds_a_row_and_shows_preview_and_button ... ok
test panels::export::tests::rows_add_edit_remove_are_undoable_and_survive_save_and_load ... ok
test shell::tests::fig_export_toast::show_in_folder_works_on_the_home_screen_after_an_export ... ok
test shell::tests::export_toast::export_toast_panel_preset_parses_and_the_toast_has_a_reveal_path ... ok
test shell::tests::export_toast::toast_clock_plain_toast_survives_a_30s_block_then_lasts_toast_seconds ... ok
test variables::tests::export_toast_keeps_counts_out_of_parentheses_and_agrees_in_number ... ok
test shell::tests::fig_export_toast::a_long_path_toast_stays_inside_the_window ... ok
test shell::tests::fig_export_toast::show_in_folder_reveals_the_exported_file ... ok
test shell::tests::export_toast::toast_clock_reveal_toast_survives_a_30s_block_then_lasts_reveal_seconds ... ok
test shell::tests::fig_export_toast::a_failed_reveal_says_so ... ok
test shell::tests::export_toast::export_toast_panel_toast_is_visible_beside_the_open_properties_panel ... ok
test shell::tests::export_toast::export_toast_preset_survives_a_plain_toast_before_the_first_frame ... ok
test shell::tests::fig_export_toast::a_click_on_a_plain_toast_reaches_the_canvas_below ... ok
test shell::tests::panel_export::panel_export_failed_write_shows_error_toast_with_path_and_reason ... ok
test shell::tests::panel_export::panel_export_never_overwrites_an_existing_file ... ok
test shell::tests::panel_export::panel_export_multiple_settings_writes_all_files_to_fallback_and_shows_reveal_toast ... ok
test shell::tests::panel_export::panel_export_without_dialog_writes_to_fallback_folder_not_home_and_shows_success_toast_with_full_path ... ok
test shell::tests::export_toast::toast_clock_export_preset_is_painted_on_the_screenshot_frame ... ok
test shell::tests::fig_export_toast::a_click_on_the_toast_body_reaches_the_canvas_below ... ok
test shell::tests::text_styles::page_panel_export_plus_adds_a_page_export_row_and_exports_the_page ... ok

test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 3442 filtered out; finished in 1.99s
```

### Pasted Output — 3, 4, 5 & 7. `cargo fmt`, `cargo clippy`, `cargo test --workspace --locked --lib`, and Python Gates
```text
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo fmt --all -- --check
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 47.30s
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test --workspace --locked --lib
    Finished `test` profile [unoptimized + debuginfo] target(s) in 29.16s
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 657 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.03s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 137 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.37s
test result: ok. 143 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.96s
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.28s
test result: ok. 255 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.43s
test result: ok. 56 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 33.04s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
$ python3 scripts/zero_fake_ui_gate.py
------------------------------------------------------------------------------------------------------------
 Gate Verdict: PASS (17/17 invariants satisfied)
============================================================================================================
$ python3 scripts/check_spacing_relationships.py
literal geometry outside theme.rs: offset_literals=1655, inset_literals=45, radius_literals=253, font_literals=25, nonconcentric_pairs=2
note: 17 entries are below baseline; run --write-baseline to lock the gain in
PASS: no count rose above the baseline
```

### Pasted Output — 6. `cargo test -p designos-app --bin designos --locked`
```text
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked
...
test terminal::zoom_scrollback_tests::agents_approval_open_tab_has_no_scrollback_and_agents_scrollback_preset_is_scrolled ... ok
test terminal::tests::touched_layers_do_not_bleed_into_other_active_tab_turn ... ok
test terminal::zoom_scrollback_tests::edge_cases_streaming_scrolled_history_alt_mouse_min_clamp_and_post_zoom_spawn ... ok
test terminal::tests::sim_approval_card_and_permissions_modal_pass_layout_lint ... ok
test terminal::unwritable_hook_folder_tests::a_hook_that_cannot_write_its_folder_ends_at_once_with_success_and_says_nothing ... ok
test terminal::hook_output_tests::a_link_or_a_hard_link_at_any_name_a_hook_wrote_is_never_written_through_by_any_hook_of_any_cli ... ok
test terminal::zoom_scrollback_tests::ac02_scrollbar_drag_click_and_thumb_across_5000_lines_and_min_size ... ok
test terminal::tests::pty_injection_fixture_real_shell ... ok

test result: ok. 3474 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 299.69s
```

### Pasted Output — 8. Visual Capture (`1440×900` on Copy of Shipped Starter)
```text
HIDIdleTime before window 1: 213
HIDIdleTime before window 2: 217
plans/reports/agy-1196-evidence/before-no-toast-1440.png: PNG image data, 1440 x 900, 8-bit/color RGBA, non-interlaced
plans/reports/agy-1196-evidence/export-toast-1440.png:    PNG image data, 1440 x 900, 8-bit/color RGBA, non-interlaced
```

---

## 7. Assumptions
- `write_new_in` (`" 2"`, `" 3"` suffix) is retained for `.fig` exports (`fig_export::export_new`) and `save_without_dialog` (`io/file_dialog.rs`) so their existing `#892`/`#1017` test suites remain unchanged, while `panels/export.rs` (Properties › Export › Export `<name>`, `#1196`) uses `write_new_dashed_in` / `write_new_dashed_path` (`-1`, `-2` suffix) and `sanitize_export_name` (`/` and `:` -> `-`).
- No files outside `crates/designos-app/` were modified.

Status: DONE
