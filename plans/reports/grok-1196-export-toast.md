#1196 — export toast capture

## Capture

`--panel export-toast` parses, and the shell test shows the real export toast keeps a reveal path. The 1440×900 PNG was not written: `designos` exited before opening a window.

### Commands

1. `bash /private/tmp/claude-501/-Users-jangtrinh--claude-profiles-work-spares-spare-eMjL4g/4a19d7c5-37f7-497b-bf78-6cbf99e83d54/scratchpad/harness-cargo.sh test -p designos-app --bin designos --locked export_toast_panel_preset_parses_and_the_toast_has_a_reveal_path`

   Exit 0.

   ```
   running 1 test
   test shell::tests::export_toast::export_toast_panel_preset_parses_and_the_toast_has_a_reveal_path ... ok

   test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3344 filtered out; finished in 1.28s
   ```

2. `bash /private/tmp/claude-501/-Users-jangtrinh--claude-profiles-work-spares-spare-eMjL4g/4a19d7c5-37f7-497b-bf78-6cbf99e83d54/scratchpad/harness-cargo.sh fmt --all -- --check`

   Exit 0. No output.

3. `python3 scripts/zero_fake_ui_gate.py`

   Exit 0. `Gate Verdict: PASS (17/17 invariants satisfied)`.

4. `python3 scripts/check_spacing_relationships.py`

   Exit 0.

   ```
   literal geometry outside theme.rs: offset_literals=1668, inset_literals=45, radius_literals=257, font_literals=25, nonconcentric_pairs=2
   note: 14 entries are below baseline; run --write-baseline to lock the gain in
   PASS: no count rose above the baseline
   ```

5. `bash /private/tmp/claude-501/-Users-jangtrinh--claude-profiles-work-spares-spare-eMjL4g/4a19d7c5-37f7-497b-bf78-6cbf99e83d54/scratchpad/harness-cargo.sh clippy -p designos-app --all-targets --locked -- -D warnings`

   Exit 0. Finished the `dev` profile in 25.82s. Cargo printed the existing warning that `src/main.rs` is in both the `designos` and `designos-studio` bins. No clippy warnings.

6. `mkdir -p out` — exit 0.

7. `bash /private/tmp/claude-501/-Users-jangtrinh--claude-profiles-work-spares-spare-eMjL4g/4a19d7c5-37f7-497b-bf78-6cbf99e83d54/scratchpad/harness-cargo.sh build -p designos-app --locked`

   Exit 0. Finished the `dev` profile in 46.84s. Binary: `out/cargo/debug/designos`.

8. `cp "/Users/jangtrinh/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig" out/untitled-ui-copy.fig`

   Exit 0. No other `.fig` was opened or written.

9. Capture, run twice:

   `out/cargo/debug/designos out/untitled-ui-copy.fig --size 1440x900 --panel export-toast --screenshot plans/reports/grok-1196-evidence/export-toast-1440.png`

   Exit 1 both times (20:26:15 pid 49899, and 20:27:18 pid 66450):

   ```
   IOSurfaceSharedEventAddEventListener failed: 10000003
   designos: WGPU error: No suitable graphics adapter found; noop not requested, vulkan support not compiled in, metal found no adapters, dx12 support not compiled in, gl support not compiled in, webgpu not requested
   ```

### Not verified

- `plans/reports/grok-1196-evidence/export-toast-1440.png` was not created. The toast text, the full saved path, the Show in folder button, and the open Properties panel were not seen.
- The kit copy was not opened, so this run did not check whether its first frame fits the export pixmap cap (16384 px on a side, 2^28 pixels). An oversized frame would toast the render error and would not set a reveal path.
- `poll_fonts` can replace an export toast with a font-unavailable toast before the frame-12 screenshot. The window never started, so that path was not reached.

## Round 2

A toast's deadline was written when it was created. `App::show_toast` set `until = now + TOAST_SECONDS` (and `show_toast_with_reveal` set `TOAST_REVEAL_SECONDS`) at `crates/designos-app/src/app.rs` (before this round, the assignment was the `Toast { .. }` in `show_toast`, then line 363 for the reveal duration). `Shell::toast` then dropped the toast when `app.now > until`, before painting it (`crates/designos-app/src/shell.rs`, previously lines 1018-1021). `draw_window` assigns `app.now` from egui's `i.time` on the next frame (`shell.rs` around the `self.app.now = ctx.input(|i| i.time)` line). Any UI-thread block longer than the duration between those two — a slow export, a hung file dialog, or the capture waiting on a large file — moved `i.time` past `until`, so the first paint removed the toast unseen. That matches a capture whose PNG had no toast, including no `Opened <file>` toast.

The lifetime now starts at the first paint. `Toast` stores `duration` and `armed`. Creation still fills `until` so a pre-paint check such as `assert_offers_reveal` still sees a full duration. The first `Shell::toast` paint sets `armed` and rewrites `until = now + duration` (`shell.rs` 1020-1024) and only then applies the deadline (`shell.rs` 1030-1032). A toast replaced before that paint is still just replaced. Callers of `show_toast` and `show_toast_with_reveal` are unchanged. `show_no_file_dialog_toast` still asks for `TOAST_REVEAL_SECONDS`, now through `Toast::stay_for`, so the first paint keeps the longer duration (`io/file_dialog.rs`).

A `--screenshot` run can still spend longer than that duration after the first paint, while it waits for frame 12 or the GPU idle refine (up to 180 frames). While a shot is pending and not yet requested, `Shell::toast` does not expire the toast (`shell.rs` 1026-1032). `--no-toast` still returns first and paints nothing. The shell stand-in cannot take the GPU PNG, so `toast_clock_export_preset_is_painted_on_the_screenshot_frame` builds `--panel export-toast --screenshot --size 1600x1088`, jumps 30 s before the first frame, then jumps past `TOAST_REVEAL_SECONDS` before the screenshot request, and asserts the export toast text and Show in folder are painted on that frame.

### What changed

- `crates/designos-app/src/app.rs` — `Toast.duration` / `Toast.armed`, `Toast::stay_for`, `show_toast` stores the plain duration unarmed, `show_toast_with_reveal` stores the reveal duration.
- `crates/designos-app/src/shell.rs` — arm `until` on the first paint; hold the latest toast until the screenshot is requested.
- `crates/designos-app/src/io/file_dialog.rs` — the Enter a path toast sets its duration with `stay_for`.
- `crates/designos-app/src/shell/tests.rs` — `Sim::built` constructs the shell before any frame. `Sim::with` is that plus the same two frames as before.
- `crates/designos-app/src/shell/tests/export_toast.rs` — the three `toast_clock_*` tests, plus a process-wide lock so the two preset tests do not share `designos-export-toast-<pid>` at once.
- `plans/reports/grok-1196-export-toast.md` — this section.

### Commands

1. Red, before the production change. `bash …/harness-cargo.sh test -p designos-app --bin designos --locked toast_clock`

   Exit 101. 0 passed, 3 failed. Each panic is the toast already `None` on the first paint after the 30 s block.

   ```
   test shell::tests::export_toast::toast_clock_plain_toast_survives_a_30s_block_then_lasts_toast_seconds ... FAILED
   test shell::tests::export_toast::toast_clock_reveal_toast_survives_a_30s_block_then_lasts_reveal_seconds ... FAILED
   test shell::tests::export_toast::toast_clock_export_preset_is_painted_on_the_screenshot_frame ... FAILED

   the frame after a 30 s block paints the toast, got None
   the first paint after a 30 s load shows the export toast
   test result: FAILED. 0 passed; 3 failed; 0 ignored; 0 measured; 3345 filtered out; finished in 0.40s
   ```

2. Same command after the change, and again after the rustfmt line wrap. Exit 0 both times.

   ```
   test shell::tests::export_toast::toast_clock_reveal_toast_survives_a_30s_block_then_lasts_reveal_seconds ... ok
   test shell::tests::export_toast::toast_clock_plain_toast_survives_a_30s_block_then_lasts_toast_seconds ... ok
   test shell::tests::export_toast::toast_clock_export_preset_is_painted_on_the_screenshot_frame ... ok

   test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 3345 filtered out; finished in 0.49s
   ```

3. `bash …/harness-cargo.sh test -p designos-app --bin designos --locked toast`

   Exit 0. `ok. 45 passed; 0 failed; 1 ignored; 0 measured; 3302 filtered out; finished in 71.47s`. The ignored test is `pen_clone_budget_warning_reaches_the_caller_and_the_toast_text`.

4. `bash …/harness-cargo.sh test -p designos-app --bin designos --locked export`

   Exit 101. `100 passed; 3 failed; 1 ignored; 0 measured; 3244 filtered out; finished in 3.26s`. Every export-toast, fig-export toast, file-dialog export, and properties-panel export test passed. The three failures spawn a PTY and die before an assertion:

   ```
   agent_composer::tests::two_armed_claude_tabs_each_get_only_their_own_chunks_under_the_id_their_process_exports
   shell::tests::agent_panel_keyboard::overlay_escape::an_export_scale_menu_whose_section_disappeared_does_not_leave_the_terminal_deaf
   terminal::tests::export_selection_pastes_fixed_sentence_only

   openpty failed: failed to openpty: Os { code: 1, kind: PermissionDenied, message: "Operation not permitted" }
   ```

   Those files were not edited. The sandbox refuses `openpty`.

5. `bash …/harness-cargo.sh fmt --all -- --check` — exit 0 after wrapping the `show_toast` struct literal onto the line rustfmt asked for. The first check exited 1 on that literal only.

6. `bash …/harness-cargo.sh clippy -p designos-app --all-targets --locked -- -D warnings` — exit 0. Finished the `dev` profile in 16.72s. Cargo printed the existing warning that `src/main.rs` is in both the `designos` and `designos-studio` bins. No clippy warnings.

7. `python3 scripts/zero_fake_ui_gate.py` — exit 0. `Gate Verdict: PASS (17/17 invariants satisfied)`.

8. `python3 scripts/check_spacing_relationships.py` — exit 0. `PASS: no count rose above the baseline` (`offset_literals=1668, inset_literals=45, radius_literals=257, font_literals=25, nonconcentric_pairs=2`).

### Not verified

- The GPU capture PNG. This sandbox has no Metal adapter, and the task forbids running the capture here. The controller takes `out/cargo/debug/designos out/untitled-ui-copy.fig --size 1440x900 --panel export-toast --screenshot …`.
- The shell test uses a 1600×1088 sim surface, which is the size `Sim` already lays out, so the screenshot is requested at frame 12. It does not render the 1440×900 GPU frame.
- `poll_fonts` still runs in `eframe::App::ui` before `draw`. A font-unavailable toast can still replace the export toast. `Sim` calls `draw` directly, so the preset test does not cover that replacement.
- The three `export`-filter PTY tests above, because `openpty` is not permitted here.
