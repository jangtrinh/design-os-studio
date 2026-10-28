# DEV Report · #1141 (P0, US-17): `designos --screenshot` sometimes exits 1 with "screenshot was not captured"

- **Branch:** `jangtrinh/agy-1141-screenshot-capture` (based on `origin/release/v0.6.4` / `6f0c3bb2`)
- **Commits:**
  1. `d575ddf2`: `test(screenshot): add debug DESIGNOS_SHOT_DROP_IMAGES fault hook`
  2. `4f766ace`: `test(screenshot): add red unit tests for capture retry, outcome, viewport, and watchdog`
  3. `HEAD`: `fix(screenshot): re-request lost captures, add frame watchdog, and report failure reasons (#1141)`

---

## 1. What Could NOT Be Verified (First)

1. **Case 8 (`mode=load`, 250 runs under CPU load): `PENDING (screen)`**
   - Deferred by explicit controller instruction on 2026-10-28 (`"run NO screenshot binary, loop, window or load generator until I send 'screen free' (the case 8 load half stays PENDING; keep the idle 250 result you already have, with its log)"`).
   - The `mode=idle` 250-run suite completed before the deferral and passed 250/250 (`summary: mode=idle total=250 passed=250 failed=0 unexplained=0`, logged in [`case8.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1141-screenshot-capture/plans/reports/agy-1141-screenshot-capture-evidence/case8.log)).
2. **AC-04 real `#1126` layout-gate capture script execution**
   - The `#1126` layout-gate capture script lives on a `main`-based branch and does not exist in our `6f0c3bb2` base.
   - Per Controller Ruling 2, AC-04 was verified via the unit test `shell::tests::screenshot_capture::failure_line_keeps_the_infra_prefix` asserting that every failure path outputs the exact `designos: screenshot was not captured: <reason>` prefix and exits with code `1`. The controller will run the real script against this build on `main` later.

---

## 2. Spec-to-HEAD Drift Table (`origin/main@205c72ef` vs Base `6f0c3bb2`)

| Symbol / Anchor | Spec (`origin/main@205c72ef`) | Base (`6f0c3bb2`) | Post-Fix (`HEAD`) |
|---|---|---|---|
| `SHOT_OK` | `crates/designos-app/src/shell.rs:23` | `crates/designos-app/src/shell.rs:23` (unchanged) | `crates/designos-app/src/shell.rs:23` |
| `Shot` struct | `crates/designos-app/src/shell.rs:50-69` | `crates/designos-app/src/shell.rs:53-73` (+3 lines) | `crates/designos-app/src/shell.rs:259-292` |
| `SHOT_GPU_SETTLE_FRAMES` | `crates/designos-app/src/shell.rs:73` | `crates/designos-app/src/shell.rs:77` (+4 lines) | `crates/designos-app/src/shell.rs:296` |
| `Shot` init in `Shell::build` | `crates/designos-app/src/shell.rs:394-405` | `crates/designos-app/src/shell.rs:414-426` (+20 lines) | `crates/designos-app/src/shell.rs:629-650` |
| `Shell::drive_screenshot` | `crates/designos-app/src/shell.rs:409-486` | `crates/designos-app/src/shell.rs:430-507` (+21 lines) | `crates/designos-app/src/shell.rs:654-796` |
| — Frame 1 `set_pixels_per_point` + `InnerSize` | `shell.rs:413-419` | `shell.rs:434-440` | `shell.rs:670-676` |
| — Capped GPU frame re-request (`!captured_at_rest && !last_try`) | `shell.rs:423-427` | `shell.rs:444-449` | `shell.rs:688-697` |
| — `RgbaImage::from_raw` | `shell.rs:433` | `shell.rs:454` | `shell.rs:700` (via `image_to_rgba` at `shell.rs:201-204`) |
| — Lanczos3 resize | `shell.rs:435` | `shell.rs:456` | `shell.rs:709` |
| — Micro-sequence (`shot.seq`) | `shell.rs:436-457` | `shell.rs:457-478` | `shell.rs:710-740` |
| — PNG write error `cannot write` | `shell.rs:464` | `shell.rs:485` | `shell.rs:715, 752` |
| — `ViewportCommand::Close` after image | `shell.rs:468` | `shell.rs:489` | `shell.rs:738, 758` |
| — `due` calculation | `shell.rs:478-482` | `shell.rs:499-503` | `shell.rs:788` |
| — Give-up frame close | `shell.rs:483-485` | `shell.rs:504-506` | `shell.rs:789-795` |
| `Shell::request_screenshot` | `crates/designos-app/src/shell.rs:492-508` | `crates/designos-app/src/shell.rs:513-529` (+21 lines) | `crates/designos-app/src/shell.rs:802-832` |
| `request_screenshot` call in `draw` | `crates/designos-app/src/shell.rs:1364` | `crates/designos-app/src/shell.rs:1411` (+47 lines) | `crates/designos-app/src/shell.rs:1726` |
| `drive_screenshot` call in `draw_window` | `crates/designos-app/src/shell.rs:1378` | `crates/designos-app/src/shell.rs:1425` (+47 lines) | `crates/designos-app/src/shell.rs:1740` |
| `main.rs` screenshot viewport setup | `crates/designos-app/src/main.rs:150-167` | `crates/designos-app/src/main.rs:163-180` (+13 lines) | `crates/designos-app/src/main.rs:163-185` (`shell::screenshot_viewport` at `shell.rs:206-214`) |
| `main.rs` post-run exit check | `crates/designos-app/src/main.rs:187-190` | `crates/designos-app/src/main.rs:205-208` (+18 lines) | `crates/designos-app/src/main.rs:208-221` |
| `Sim` default screen size `1600x1088` | `crates/designos-app/src/shell/tests.rs:103` | `crates/designos-app/src/shell/tests.rs:103` (unchanged) | `crates/designos-app/src/shell/tests.rs:103` |

---

## 3. Root Cause & Epistemic Ledger (`FACT` / `INFERENCE` / `ASSUMPTION`)

1. **`[FACT]` Single-shot `ViewportCommand::Screenshot` dropped on surface error (`crates/designos-app/src/shell.rs:513-529`)**:
   - In base `6f0c3bb2`, `Shell::request_screenshot` sent `egui::ViewportCommand::Screenshot` once and latched `shot.requested = true`. It never re-requested unless an image arrived from a capped GPU frame (`shell.rs:444-449`).
   - `eframe` drains pending `Screenshot` commands into `painter.paint_and_update_textures` for that single frame (`eframe-0.35.0/src/native/wgpu_integration.rs:731-752`).
   - `egui_wgpu::winit::Painter::paint_and_update_textures` (`egui-wgpu-0.35.0/src/winit.rs:605-628`) calls `surface.get_current_texture()` and returns early **before** allocating `capture_buffer` whenever the surface status is `Outdated` (e.g., after Frame 1's `ViewportCommand::InnerSize` at `shell.rs:439`), `Lost`, `Occluded`, or `Timeout`. The drained `screenshot_commands` vector for that frame is dropped permanently, so no `Event::Screenshot` ever arrives and `drive_screenshot` eventually closes at frame 240 (`shell.rs:504-506`) with a bare `"designos: screenshot was not captured"` (`main.rs:205-208`).
2. **`[FACT]` macOS Metal surface occlusion when launching inactive (`wgpu-hal-29.0.4/src/metal/surface.rs:124-156`)**:
   - On macOS Metal, `get_current_texture()` checks `[NSWindow occlusionState] & NSWindowOcclusionStateVisible` and returns `SurfaceError::Occluded` if the window is covered.
   - Base `main.rs:172-173` built the screenshot viewport with `.with_active(false)` (to avoid stealing focus) but without `WindowLevel::AlwaysOnTop`. When `eframe` called `window.set_visible(true)` (`[window makeKeyAndOrderFront:nil]`, `winit-0.30.13/src/platform_impl/macos/window_delegate.rs:905`) after Frame 1 (`epi_integration.rs:326`) while `[NSApp isActive] == NO`, AppKit ordered the window behind active windows.
   - Setting `.with_window_level(egui::WindowLevel::AlwaysOnTop)` in [`screenshot_viewport`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1141-screenshot-capture/crates/designos-app/src/shell.rs#L206-L214) and calling `[window orderFrontRegardless]` on `NSApp.windows` (`order_screenshot_window_front_regardless` at [`shell.rs:144-153`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1141-screenshot-capture/crates/designos-app/src/shell.rs#L144-L153)) keeps the capture window at the front of `kCGFloatingWindowLevel` without activating the app or stealing keyboard focus.
3. **`[FACT]` Secondary hang when `--screenshot` closes a file path dirtied by a CLI preset (`crates/designos-app/src/shell.rs:1745-1752`)**:
   - When `--screenshot` was invoked on a real file path alongside a mutating preset (such as `fixture.designos --panel picker --screenshot out.png`), `preset_popup` mutated `app.doc` after `open_path`, marking the document dirty. When `drive_screenshot` wrote the PNG and sent `ViewportCommand::Close`, `Shell::draw_window` called `!self.app.may_quit(&ctx)` and sent `ViewportCommand::CancelClose` to show the interactive `"Save changes?"` dialog. Bypassing `may_quit` / `flush_autosave` on `close_requested()` when `self.shot.is_some()` ([`shell.rs:1745-1751`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1141-screenshot-capture/crates/designos-app/src/shell.rs#L1745-L1751)) ensures headless screenshot runs always exit cleanly once the capture completes or fails.
4. **`[FACT]` Controller Ruling 1 (`DESIGNOS_SHOT_DROP_IMAGES` gating choice)**:
   - The spec did not require `DESIGNOS_SHOT_DROP_IMAGES` in release builds, so per Controller Ruling 1 we gated it with `#[cfg(debug_assertions)]` ([`shell.rs:645-646`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1141-screenshot-capture/crates/designos-app/src/shell.rs#L645-L646), [`shell.rs:682-686`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1141-screenshot-capture/crates/designos-app/src/shell.rs#L682-L686)), committed it first in `d575ddf2`, and ran Case 7 against the debug build (`out/cargo/debug/designos`).

---

## 4. Skills Loaded & What Changed Because of Them

- **`/es:dev` & `es:feature-dev`**: Enforced red-first commit sequencing (`d575ddf2` fault hook -> `4f766ace` failing unit tests -> `d1f4bbb9` implementation fix) and scoped file ownership.
- **`/es:rust`**: Kept `on_surface_status` zero-cost outside `--screenshot` mode by accessing `eframe::egui_wgpu::wgpu::CurrentSurfaceTexture` directly without adding `wgpu` to `crates/designos-app/Cargo.toml`, and used lock-free atomics / `OnceLock` (`SHOT_OK`, `SHOT_WRITE_FAILED`, `SHOT_FAILURE`, `SHOT_OPEN_ERROR`) across the `eframe::run_native` boundary.
- **`/es:rust-gpu-canvas`**: Preserved the `SHOT_GPU_SETTLE_FRAMES` / `captured_at_rest` GPU refine handshake so a re-requested capture after a capped GPU frame does not count against the 5 lost-request budget (`shot.requests = shot.requests.saturating_sub(1)`).
- **`es:astra-work` & `es:fable-thinking`**: Used prediction-before-action and falsification when `fixture-panel-picker` timed out during Case 8 diagnosis, proving within one 2-second test that `v3.png` had already been written (`size = 246811`) before `Shell::draw_window` cancelled the window close via `may_quit(&ctx)` -> `ViewportCommand::CancelClose`.

---

## 5. Acceptance Evidence (All Gates Run on Final Code Commit `HEAD`)

| Gate / Criterion | Status | Evidence File / Command & Output |
|---|---|---|
| **Tests 1–6 & 6b (Red first)** | PASS | [`red.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1141-screenshot-capture/plans/reports/agy-1141-screenshot-capture-evidence/red.log) on `4f766ace`: `FAILED. 0 passed; 7 failed` (`lost_screenshot_request_is_sent_again`, `capture_is_written_after_a_lost_request`, `capture_gives_up_with_a_reason_after_five_lost_requests`, `failure_line_keeps_the_infra_prefix`, `malformed_image_is_reported`, `screenshot_viewport_is_always_on_top`, `watchdog_is_not_armed_before_the_first_frame`). |
| **Tests 1–6, 6b & edge case (Green)** | PASS | [`green.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1141-screenshot-capture/plans/reports/agy-1141-screenshot-capture-evidence/green.log): `ok. 8 passed; 0 failed` (includes `late_image_after_lost_request_wins_and_second_image_is_ignored`). |
| **Case 7 (`DESIGNOS_SHOT_DROP_IMAGES=1` on hook commit `d575ddf2`)** | PASS | [`case7-red.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1141-screenshot-capture/plans/reports/agy-1141-screenshot-capture-evidence/case7-red.log): `exit_code=1`, `png_exists=false`, `stderr=designos: screenshot was not captured`. |
| **Case 7 (`DESIGNOS_SHOT_DROP_IMAGES=1` on fix)** | PASS | [`case7-green.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1141-screenshot-capture/plans/reports/agy-1141-screenshot-capture-evidence/case7-green.log): `exit_code=0`, `png_exists=true`, `png_bytes=249968`, `stderr=`. |
| **Case 8 (`scripts/screenshot_capture_loop.sh`, 250 idle + 250 load)** | PARTIAL (`load` PENDING screen) | [`case8.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1141-screenshot-capture/plans/reports/agy-1141-screenshot-capture-evidence/case8.log):<br>`mode=idle`: `total=250 passed=250 failed=0 unexplained=0`<br>`mode=load`: `PENDING (screen)` per controller instruction. |
| **`cargo fmt --all -- --check`** | PASS | Exited `0` (`fmt_exit=0`). |
| **`cargo clippy --workspace --all-targets --locked -- -D warnings`** | PASS | Exited `0` (`Finished dev profile [unoptimized + debuginfo] target(s) in 1m 22s`, `clippy_exit=0`). |
| **`cargo test --workspace --locked --lib`** | PASS | Exited `0` (`workspace_lib_exit=0`). |
| **`cargo test -p designos-app --bin designos --locked`** | PASS | Exited `0` (`test result: ok. 3343 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 344.03s`, zero reruns needed). |
| **`python3 scripts/zero_fake_ui_gate.py`** | PASS | `Gate Verdict: PASS (17/17 invariants satisfied)` |
| **`python3 scripts/check_spacing_relationships.py`** | PASS | `PASS: no count rose above the baseline` |

### Pasted Command Outputs

#### 1. Red & Green Unit Tests (`red.log` / `green.log`)
```text
# red.log (on 4f766ace):
running 7 tests
test shell::tests::screenshot_capture::failure_line_keeps_the_infra_prefix ... FAILED
test shell::tests::screenshot_capture::malformed_image_is_reported ... FAILED
test shell::tests::screenshot_capture::screenshot_viewport_is_always_on_top ... FAILED
test shell::tests::screenshot_capture::watchdog_is_not_armed_before_the_first_frame ... FAILED
test shell::tests::screenshot_capture::lost_screenshot_request_is_sent_again ... FAILED
test shell::tests::screenshot_capture::capture_is_written_after_a_lost_request ... FAILED
test shell::tests::screenshot_capture::capture_gives_up_with_a_reason_after_five_lost_requests ... FAILED
test result: FAILED. 0 passed; 7 failed; 0 ignored; 0 measured; 3346 filtered out; finished in 1.29s

# green.log (on HEAD):
running 8 tests
test shell::tests::screenshot_capture::screenshot_viewport_is_always_on_top ... ok
test shell::tests::screenshot_capture::malformed_image_is_reported ... ok
test shell::tests::screenshot_capture::watchdog_is_not_armed_before_the_first_frame ... ok
test shell::tests::screenshot_capture::failure_line_keeps_the_infra_prefix ... ok
test shell::tests::screenshot_capture::lost_screenshot_request_is_sent_again ... ok
test shell::tests::screenshot_capture::capture_gives_up_with_a_reason_after_five_lost_requests ... ok
test shell::tests::screenshot_capture::late_image_after_lost_request_wins_and_second_image_is_ignored ... ok
test shell::tests::screenshot_capture::capture_is_written_after_a_lost_request ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 3346 filtered out; finished in 1.77s
```

#### 2. Case 7 Fault Hook (`case7-red.log` / `case7-green.log`)
```text
# case7-red.log (on d575ddf2):
commit=d575ddf2
cmd=DESIGNOS_SHOT_DROP_IMAGES=1 out/cargo/debug/designos --fixture showcase --select none --size 1440x900 --no-toast --screenshot $S/shot.png
exit_code=1
png_exists=false
png_bytes=0
stderr=designos: screenshot was not captured

# case7-green.log (with fix):
commit=4f766ace+fix
cmd=DESIGNOS_SHOT_DROP_IMAGES=1 out/cargo/debug/designos --fixture showcase --select none --size 1440x900 --no-toast --screenshot $S/shot.png
exit_code=0
png_exists=true
png_bytes=249968
stderr=
```

#### 3. Case 8 Loop (`case8.log`)
```text
=== Case 8 suite: mode=idle runs=250 jobs=1 ===
summary: mode=idle total=250 passed=250 failed=0 unexplained=0
=== Case 8 suite: mode=load runs=250 jobs=1 ===
status: PENDING (screen — deferred per controller instruction until 'screen free')
```

#### 4. Harness Gates (`fmt`, `clippy`, `--workspace --lib`, `--bin designos`, Python gates)
```text
$ python3 scripts/zero_fake_ui_gate.py
 Gate Verdict: PASS (17/17 invariants satisfied)

$ python3 scripts/check_spacing_relationships.py
literal geometry outside theme.rs: offset_literals=1668, inset_literals=45, radius_literals=257, font_literals=25, nonconcentric_pairs=2
note: 14 entries are below baseline; run --write-baseline to lock the gain in
PASS: no count rose above the baseline

$ cargo fmt --all -- --check
fmt_exit=0

$ cargo clippy --workspace --all-targets --locked -- -D warnings
...
    Checking designos-library v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1141-screenshot-capture/crates/designos-library)
    Checking designos-eval v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1141-screenshot-capture/crates/designos-eval)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 22s
clippy_exit=0

$ cargo test --workspace --locked --lib
...
test result: ok. 55 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 33.77s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
workspace_lib_exit=0

$ cargo test -p designos-app --bin designos --locked
...
test result: ok. 3343 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 344.03s
bin_test_exit=0
```

---

## 6. Files Modified & Files Outside Scope

- **Files modified (all within scope):**
  - [`crates/designos-app/src/main.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1141-screenshot-capture/crates/designos-app/src/main.rs)
  - [`crates/designos-app/src/shell.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1141-screenshot-capture/crates/designos-app/src/shell.rs)
  - [`crates/designos-app/src/shell/tests.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1141-screenshot-capture/crates/designos-app/src/shell/tests.rs)
  - [`crates/designos-app/src/shell/tests/screenshot_capture.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1141-screenshot-capture/crates/designos-app/src/shell/tests/screenshot_capture.rs)
  - [`scripts/screenshot_capture_loop.sh`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1141-screenshot-capture/scripts/screenshot_capture_loop.sh)
  - [`plans/reports/agy-1141-screenshot-capture.md`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1141-screenshot-capture/plans/reports/agy-1141-screenshot-capture.md)
  - `plans/reports/agy-1141-screenshot-capture-evidence/{red.log,green.log,case7-red.log,case7-green.log,case8.log}`
- **Files outside scope:** None.

---

## 7. Assumptions & Weakest Link

- **Assumptions:** `[ASSUMPTION]` On macOS, `std::time::Instant` (`mach_absolute_time`) pauses while the host hardware is in system sleep; `ShotWatchdog` therefore measures 30 seconds of awake wall-clock time after the first screenshot frame is drawn.
- **Weakest link:** Case 8's 250-run `mode=load` stress suite has not yet been executed because screen/window/load execution is deferred (`PENDING (screen)`) until the controller sends `screen free`.

Status: DONE


---

## Round 2

- **Branch:** `jangtrinh/carry-1141-screenshot-capture-main` (carry onto `origin/main` `8dfaee93`, base `818b6249`)
- **Commits:**
  1. `37eb0da2`: `test(screenshot): add round 2 failing tests for frame watchdog, write failure, and exit contract`
  2. `HEAD`: `fix(screenshot): guard watchdog during frames, stop on write failure, and pin exit contract (#1141)`

### 1. What Could NOT Be Verified (First)

1. **Case 8 (`mode=load`, 250 runs under CPU load): `PENDING (screen)`**
   - Per Round 2 controller spec (`"No window this round. Case 8's load half waits until the controller posts 'screen free'; #1126's Tier B holds the screen now"`), no screenshot binary or window was launched in Round 2.
2. **Real 30+ second dense CPU kit tile paint under `spawn_screenshot_watchdog()`**
   - Because no window is permitted in Round 2 and opening client/Untitled kits is prohibited by the harness rules, the in-frame guard and post-frame clock re-arm were verified deterministically through `ShotWatchdog` / `watchdog_exit_outcome` unit tests (`watchdog_does_not_fire_while_a_frame_is_running`, `watchdog_fires_after_30s_without_frames_once_the_frame_ends`, `watchdog_does_not_exit_after_the_png_was_written`).

### 2. Spec-to-HEAD Drift Table (`818b6249` vs Post-Fix `HEAD`)

| Citation in Round 2 Spec | Spec Says (`818b6249`) | Carry Base (`818b6249`) | Post-Fix (`HEAD`) |
|---|---|---|---|
| `Shot` init `open_failed` | `shell.rs` `Shot` init | `crates/designos-app/src/shell.rs:713` | `crates/designos-app/src/shell.rs:761` |
| `beat_screenshot_watchdog()` call in `drive_screenshot` | `shell.rs:729` | `crates/designos-app/src/shell.rs:729` (exact) | `crates/designos-app/src/shell.rs:777` |
| `spawn_screenshot_watchdog` loop & `check` | `shell.rs:98-118` | `crates/designos-app/src/shell.rs:98-120` (exact) | `crates/designos-app/src/shell.rs:146-168` |
| Unlimited tile budget in screenshot mode | `tiles.rs:158` | `crates/designos-app/src/canvas/tiles.rs:158` (exact) | `crates/designos-app/src/canvas/tiles.rs:158` (unchanged) |
| Sequence-frame write error arm (`rgb.save`) | `shell.rs:788` | `crates/designos-app/src/shell.rs:788` (exact) | `crates/designos-app/src/shell.rs:836-845` |
| Single-PNG write error arm (`buf.save`) | `shell.rs:825` | `crates/designos-app/src/shell.rs:825` (exact) | `crates/designos-app/src/shell.rs:876-883` |
| `Shell::draw` entry & `request_screenshot` | `shell.rs:1796-1810` | `crates/designos-app/src/shell.rs:1796-1810` (exact) | `crates/designos-app/src/shell.rs:1850-1865` |
| `main.rs` screenshot exit path & `exit_code` | `main.rs:224-254` | `crates/designos-app/src/main.rs:224-254` (exact) | `crates/designos-app/src/main.rs:224-259` |

### 3. Root Cause & Epistemic Ledger (`FACT` / `INFERENCE` / `ASSUMPTION`)

1. **`[FACT]` Watchdog measured time since frame start rather than time between frames, and exited without re-checking completion flags under the lock (`crates/designos-app/src/shell.rs:98-120`, `729`)**:
   - In `818b6249`, `beat_screenshot_watchdog()` armed `last_frame_at` at the top of `drive_screenshot` (`shell.rs:729`), before `canvas::show` (`shell.rs:1976`). In `--screenshot` mode, `tiles.rs:158` renders all visible tiles synchronously on the UI thread with `TileBudget::unlimited()`. If a single frame took `>= 30 s` to paint, the watchdog thread saw `start.elapsed() - last_frame_at >= 30 s` while that frame was still running and exited with `SHOT_OCCLUDED_REASON`.
   - Furthermore, once `w.check(start.elapsed())` returned `Some`, the watchdog thread dropped the `SHOT_WATCHDOG` lock and called `std::process::exit(1)` without re-checking `w.done`, `SHOT_OK`, or `SHOT_WRITE_FAILED` under the lock.
   - **Fix**: Added `SHOT_IN_FRAME: AtomicBool` (`shell.rs:39`) and `ShotFrameGuard` (`shell.rs:121-144`) instantiated at the top of `Shell::draw` (`shell.rs:1851`). `ShotFrameGuard::enter` sets `SHOT_IN_FRAME = true` and `w.begin_frame()`, and `Drop::drop` calls `w.end_frame(start.elapsed())` (clearing `in_frame` and re-arming `last_frame_at` at frame end) before clearing `SHOT_IN_FRAME`. `ShotWatchdog::check` (`shell.rs:101-111`) and `spawn_screenshot_watchdog` (`shell.rs:159-166`) never fire while a frame is running, and immediately before `process::exit`, `watchdog_exit_outcome` (`shell.rs:114-119`) re-checks `w.done`, `SHOT_OK`, and `SHOT_WRITE_FAILED` under the `SHOT_WATCHDOG` lock.
2. **`[FACT]` Write errors in `drive_screenshot` left `shot.outcome == ShotOutcome::Pending` (`crates/designos-app/src/shell.rs:788`, `825`)**:
   - Both the sequence-frame save error (`shell.rs:788`) and the single-PNG save error (`shell.rs:825`) set `SHOT_WRITE_FAILED = true` and sent `ViewportCommand::Close`, but left `shot.outcome` as `ShotOutcome::Pending`. Consequently, `request_screenshot` at the end of `Shell::draw` and any subsequent `drive_screenshot` pass before window teardown continued running.
   - **Fix**: Set `shot.due = false` and `shot.outcome = ShotOutcome::Failed(format!("cannot write {}: {e}", ...))` in both write-error arms (`shell.rs:837-845` and `shell.rs:876-883`) before `ViewportCommand::Close`, so `drive_screenshot` (`shell.rs:778`) and `request_screenshot` (`shell.rs:926`) return early on all subsequent passes.
3. **`[FACT]` Merged exit contract in `main.rs` was inline and unpinned by unit tests (`crates/designos-app/src/main.rs:229-254`)**:
   - Factored the stderr message selection into pure `pub(crate) fn screenshot_exit_line` (`main.rs:242-249`) paired with `pub(crate) fn exit_code` (`main.rs:253-259`), and pinned all 4 exit cases (`--select` miss, write failure, not captured, written) in `shell::tests::screenshot_capture::screenshot_exit_contract` (`screenshot_capture.rs:314-340`).

### 4. Skills Loaded & What Changed Because of Them

- **`/es:dev` & `es:feature-dev`**: Followed red-first commit sequencing (`37eb0da2` failing tests -> `HEAD` fix) and kept edits strictly within the allowed files (`shell.rs`, `main.rs`, `screenshot_capture.rs`, and the report/evidence folder).
- **`/es:rust`**: Used a RAII `Drop` guard (`ShotFrameGuard`) that re-arms `ShotWatchdog` under the mutex before clearing `SHOT_IN_FRAME`, eliminating any race window between frame completion and the 30 s idle timer, and kept `ShotWatchdog::check` and `watchdog_exit_outcome` pure so parallel `cargo test` threads cannot interfere with unit tests.
- **`/es:rust-gpu-canvas`**: Preserved all GPU/CPU canvas rendering paths untouched while ensuring long unlimited-budget CPU tile frames (`tiles.rs:158`) are never killed by the occlusion watchdog.
- **`es:astra-work` & `es:fable-thinking`**: Caught and eliminated an unused-method clippy error (`check` vs `check_in_frame`) under `-D warnings` by unifying the check into `ShotWatchdog::check` and calling it directly in `spawn_screenshot_watchdog`.

### 5. Acceptance Table & Pasted Outputs (Final Commit)

| Gate / Criterion | Status | Evidence File / Summary |
|---|---|---|
| **Round 2 Red tests (5 new tests fail first)** | PASS | [`red-r2.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1141-screenshot-capture/plans/reports/agy-1141-screenshot-capture-evidence/red-r2.log) & [`red.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1141-screenshot-capture/plans/reports/agy-1141-screenshot-capture-evidence/red.log) on `37eb0da2`: `FAILED. 8 passed; 5 failed` (`watchdog_does_not_fire_while_a_frame_is_running`, `watchdog_fires_after_30s_without_frames_once_the_frame_ends`, `watchdog_does_not_exit_after_the_png_was_written`, `write_failure_stops_further_capture_passes`, `screenshot_exit_contract`). |
| **Round 2 Green tests (all 13 tests pass)** | PASS | [`green-r2.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1141-screenshot-capture/plans/reports/agy-1141-screenshot-capture-evidence/green-r2.log) & [`green.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1141-screenshot-capture/plans/reports/agy-1141-screenshot-capture-evidence/green.log) on `HEAD`: `ok. 13 passed; 0 failed`. |
| **`cargo fmt --all -- --check`** | PASS | Exited `0` (`FMT_EXIT=0`). |
| **`cargo clippy --workspace --all-targets --locked -- -D warnings`** | PASS | Exited `0` (`CLIPPY_EXIT=0`). |
| **`cargo test --workspace --locked --lib`** | PASS | Exited `0` (`WORKSPACE_LIB_EXIT=0`, all 12 lib crates green). |
| **`cargo test -p designos-app --bin designos --locked`** | PASS (after 3/3 rerun of known flake) | Suite: `3506 passed; 1 failed; 12 ignored` (the 1 failure was known PTY timing flake `terminal::zoom_scrollback_tests::agents_approval_open_tab_has_no_scrollback_and_agents_scrollback_preset_is_scrolled`, which passed 3/3 alone: `0.40s`, `0.89s`, `0.32s`). |
| **`python3 scripts/zero_fake_ui_gate.py`** | PASS | `Gate Verdict: PASS (17/17 invariants satisfied)` |
| **`python3 scripts/check_spacing_relationships.py`** | PASS | `PASS: no count rose above the baseline` |

#### Exact Commands & Pasted Outputs

##### 1. Red & Green Unit Tests (`red-r2.log` / `green-r2.log`)
```text
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked screenshot_capture
# On 37eb0da2 (red-r2.log):
running 13 tests
test shell::tests::screenshot_capture::watchdog_is_not_armed_before_the_first_frame ... ok
test shell::tests::screenshot_capture::malformed_image_is_reported ... ok
test shell::tests::screenshot_capture::screenshot_viewport_is_always_on_top ... ok
test shell::tests::screenshot_capture::failure_line_keeps_the_infra_prefix ... ok
test shell::tests::screenshot_capture::watchdog_fires_after_30s_without_frames_once_the_frame_ends ... FAILED
test shell::tests::screenshot_capture::watchdog_does_not_fire_while_a_frame_is_running ... FAILED
test shell::tests::screenshot_capture::screenshot_exit_contract ... FAILED
test shell::tests::screenshot_capture::watchdog_does_not_exit_after_the_png_was_written ... FAILED
test shell::tests::screenshot_capture::lost_screenshot_request_is_sent_again ... ok
test shell::tests::screenshot_capture::capture_gives_up_with_a_reason_after_five_lost_requests ... ok
test shell::tests::screenshot_capture::write_failure_stops_further_capture_passes ... FAILED
test shell::tests::screenshot_capture::late_image_after_lost_request_wins_and_second_image_is_ignored ... ok
test shell::tests::screenshot_capture::capture_is_written_after_a_lost_request ... ok

failures:
    shell::tests::screenshot_capture::screenshot_exit_contract
    shell::tests::screenshot_capture::watchdog_does_not_exit_after_the_png_was_written
    shell::tests::screenshot_capture::watchdog_does_not_fire_while_a_frame_is_running
    shell::tests::screenshot_capture::watchdog_fires_after_30s_without_frames_once_the_frame_ends
    shell::tests::screenshot_capture::write_failure_stops_further_capture_passes

test result: FAILED. 8 passed; 5 failed; 0 ignored; 0 measured; 3506 filtered out; finished in 1.88s

# On HEAD (green-r2.log):
running 13 tests
test shell::tests::screenshot_capture::watchdog_does_not_fire_while_a_frame_is_running ... ok
test shell::tests::screenshot_capture::watchdog_fires_after_30s_without_frames_once_the_frame_ends ... ok
test shell::tests::screenshot_capture::watchdog_does_not_exit_after_the_png_was_written ... ok
test shell::tests::screenshot_capture::screenshot_exit_contract ... ok
test shell::tests::screenshot_capture::malformed_image_is_reported ... ok
test shell::tests::screenshot_capture::watchdog_is_not_armed_before_the_first_frame ... ok
test shell::tests::screenshot_capture::failure_line_keeps_the_infra_prefix ... ok
test shell::tests::screenshot_capture::screenshot_viewport_is_always_on_top ... ok
test shell::tests::screenshot_capture::capture_gives_up_with_a_reason_after_five_lost_requests ... ok
test shell::tests::screenshot_capture::lost_screenshot_request_is_sent_again ... ok
test shell::tests::screenshot_capture::late_image_after_lost_request_wins_and_second_image_is_ignored ... ok
test shell::tests::screenshot_capture::capture_is_written_after_a_lost_request ... ok
test shell::tests::screenshot_capture::write_failure_stops_further_capture_passes ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 3506 filtered out; finished in 1.39s
```

##### 2. Format, Clippy & Python Gates
```text
$ cargo fmt --all -- --check
FMT_EXIT=0

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1141-screenshot-capture/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1141-screenshot-capture/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.7.0 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1141-screenshot-capture/crates/designos-mcp)
    Checking designos-app v0.7.0 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1141-screenshot-capture/crates/designos-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 13.96s
CLIPPY_EXIT=0

$ python3 scripts/zero_fake_ui_gate.py
 Gate Verdict: PASS (17/17 invariants satisfied)

$ python3 scripts/check_spacing_relationships.py
literal geometry outside theme.rs: offset_literals=1655, inset_literals=45, radius_literals=253, font_literals=25, nonconcentric_pairs=2
note: 17 entries are below baseline; run --write-baseline to lock the gain in
PASS: no count rose above the baseline
```

##### 3. Workspace `--lib` & Binary Test Suite + 3x Flake Reruns
```text
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test --workspace --locked --lib
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_diff-3e4ac7f4239bdf70)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_doc-a602db38a46fff85)
test result: ok. 671 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.83s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_eval-bcfed33cc2424327)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_export-9fdb08ecda921559)
test result: ok. 153 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 1.05s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fig-23ef0ffaaa2e5d17)
test result: ok. 143 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.83s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fonts-b47e8236505f86b6)
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_gpu-c1d967374849bc9c)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_library-83f15dba30acfe90)
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.34s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_mcp-b0d745b70e7b1837)
test result: ok. 255 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.32s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_render-864d59a31f5c5170)
test result: ok. 56 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 48.82s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_svg-0e64ba5bab47cd7e)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_webcapture-fb56851b77895abd)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s
WORKSPACE_LIB_EXIT=0

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked
test result: FAILED. 3506 passed; 1 failed; 12 ignored; 0 measured; 0 filtered out; finished in 453.16s
failures:
    terminal::zoom_scrollback_tests::agents_approval_open_tab_has_no_scrollback_and_agents_scrollback_preset_is_scrolled

# 3x standalone reruns of the known PTY timing flake:
=== Rerun 1/3 ===
running 1 test
test terminal::zoom_scrollback_tests::agents_approval_open_tab_has_no_scrollback_and_agents_scrollback_preset_is_scrolled ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3518 filtered out; finished in 0.40s
=== Rerun 2/3 ===
running 1 test
test terminal::zoom_scrollback_tests::agents_approval_open_tab_has_no_scrollback_and_agents_scrollback_preset_is_scrolled ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3518 filtered out; finished in 0.89s
=== Rerun 3/3 ===
running 1 test
test terminal::zoom_scrollback_tests::agents_approval_open_tab_has_no_scrollback_and_agents_scrollback_preset_is_scrolled ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3518 filtered out; finished in 0.32s
```

### 6. Assumptions & Weakest Link

- **Assumptions:** `[ASSUMPTION]` In a `--screenshot` process, `Shell::draw` runs exclusively on the main UI thread, so a process-global `SHOT_IN_FRAME: AtomicBool` paired with `ShotWatchdog::in_frame` accurately reflects whether the screenshot window is currently inside a frame.
- **Weakest link:** Case 8's 250-run `mode=load` stress suite remains `PENDING (screen)` until the controller posts `"screen free"`.

Status: DONE
