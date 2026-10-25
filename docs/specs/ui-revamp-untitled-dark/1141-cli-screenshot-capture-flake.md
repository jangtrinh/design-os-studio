# R `--screenshot` either captures or says why: no silent "screenshot was not captured"

| Field | Value |
|---|---|
| Issue | #1141 (epic #1118, spec key R) |
| Size | S. One to two days: a re-request loop with a timeout, a reasoned failure message, an always-on-top capture window, a wall-clock watchdog, a test-only fault hook, seven unit tests, and a repeat-run script for the PR evidence. |
| Depends on | none |
| Unblocks | L (#1126): its baseline and contact sheets come from `--screenshot`. M (docs captures). Every spec whose PR evidence is a CLI capture. |
| Touches | `crates/designos-app/src/shell.rs`:<br>- `Shot` `:50-69`, `SHOT_GPU_SETTLE_FRAMES` `:73`, `SHOT_OK` `:23`<br>- the `Shot` built in `Shell::build` `:394-405`<br>- `drive_screenshot` `:409-486`<br>- `request_screenshot` `:492-508`<br>- call sites in `draw` `:1364` and `draw_window` `:1378`<br>`crates/designos-app/src/main.rs`: viewport for screenshot mode `:150-167`, exit `:187-190`<br>`crates/designos-app/Cargo.toml` (direct `log` dependency, already in `Cargo.lock` through egui)<br>New test module `crates/designos-app/src/shell/tests/screenshot_capture.rs`<br>New `scripts/screenshot_repeat.sh` |

Code baseline: `origin/main@205c72ef`. All file:line citations were read there with `git show`. `origin/main` has since moved to `7e15ddae`, which adds 4 lines at `shell.rs:103`. Every `shell.rs` line above 102 is the same there, and every line below it is +4 (for example, `drive_screenshot` is at `:413`); `main.rs` is unchanged. egui citations refer to the locked `eframe 0.35.0`, `egui-wgpu 0.35.0` and `egui 0.35.0` sources. Re-anchor against HEAD and report drift before writing code. Scratch evidence: `$SCRATCH/agents/w5/r/`, where `$SCRATCH` = `/private/tmp/claude-501/-Users-jangtrinh--claude-profiles-work-spares-spare-AW68Nz/a6445c24-c40e-4254-a3e2-5e0bafafd0f4/scratchpad`.

## 1. Summary and user story

As the PM running spec L's baseline and every revamp PR's evidence through `designos <file> --screenshot out.png`, I need each capture to either write the PNG or exit with a reason I can act on, such as "no image after 5 requests" or "window not drawn for 30 s". Today a run sometimes exits 1 with only `designos: screenshot was not captured`. A retry passes, so the failure is random and silent.

## 2. Current state (labels per claim)

**Reported failures (FACT, as reported; not reproduced by this writer):**
- #1141, 2026-10-25 12:57–13:00, no other load, build from `ec47da26`: a fresh 156-node `.designos` made with `--apply-spec` failed 3 times in a row at `--size 1600x1000`, with or without `--select`, and also at `1600x1100`. It had passed a minute earlier. The same frame rendered fine through MCP `designos_render_png`.
- While other agents built in the same checkout, the spec writers saw 5 failures: "5 failures in 16 captures" per the #1141 body, and "5 of 37 runs" per the controller's brief for this spec (the counts differ by source; both are as reported). The failing cases were D-06 (zoom menu), D-09 (Dev Inspect), E-03b, E-04 and F-03. Every retry exited 0.

**This writer's runs (FACT, 2026-10-25 at about 21:38, no other load, binary built from `ec47da26`):** the screenshot code there is identical to `205c72ef` (`diff` of `shell.rs:403-502` against `:409-508`, and of `main.rs:150-160` against `:184-194`). The flake did **not** reproduce:
- `designos --fixture showcase --select none --size 1440x900 --no-toast --screenshot run-N.png`: 20 of 20 exited 0 (`$SCRATCH/agents/w5/r/runs.log`).
- The same with `--gpu --size 1600x1000`: 15 of 15 exited 0 (`gpu-runs.log`).

**The capture state machine (FACT, `shell.rs` at `205c72ef`):**
- `drive_screenshot` runs at the top of every `draw_window` (`:1378`), and `request_screenshot` runs after the frame has drawn (`:1364`).
- Frame 1 sets `pixels_per_point` and sends `ViewportCommand::InnerSize` (`:413-419`).
- A capture becomes due when `!requested` and either `frames >= 12` with the content rect within 1.5 pt of the requested size, or `frames >= 60` (`:478-482`).
- `request_screenshot` sends `ViewportCommand::Screenshot` once and sets `requested = true` (`:504-507`). On a GPU canvas that is not at rest, it waits until frame 180 (`SHOT_GPU_SETTLE_FRAMES`, `:73`, `:501`).
- While `requested` is true, nothing asks again (`:495-499`). The only re-request is for an image captured from a capped frame that is not the last try (`:423-427`).
- After 240 frames without an image, the window closes (`:483-485`). `main` then finds `SHOT_OK` false and exits 1 with `designos: screenshot was not captured` (`main.rs:187-189`).
- Other exits:
  - An image whose bytes do not fit `RgbaImage::from_raw` closes the window without any message (`:433`, falling through to `:468`).
  - A failed PNG write prints `cannot write …` (`:464`). That is a different symptom.
- The screenshot window is built with `.with_active(false)` and no window level (`main.rs:159-160`).

**What eframe and egui-wgpu do with a screenshot request (FACT, crate sources):**
- eframe moves every pending `Screenshot` action into the paint call of the frame that sees it (`eframe/src/native/wgpu_integration.rs:724-741`).
- `egui_wgpu::winit::Painter::paint_and_update_textures` returns early, **before** capturing, when the surface texture cannot be acquired (`egui-wgpu/src/winit.rs:605-628`). The early returns cover `Outdated` (reconfigure), `Lost` (recreate), `Occluded` and `Timeout` (skip frame) (`egui-wgpu/src/lib.rs:386-406`).
- It also returns early when the surface is missing (`winit.rs:563-564`) or a recreate fails (`:519-522`).
- So a request that rides on a skipped frame is dropped. No `Event::Screenshot` ever arrives for it.
- egui-wgpu reports these frames only through `log::warn!`, `log::debug!` and `log::trace!`. `designos-app` installs no `log` backend: there is no `log`, `env_logger` or `tracing` in its `Cargo.toml` and no `set_logger` in `src`. The drop is therefore invisible.
- When winit reports the window occluded or minimised, `ViewportInfo::visible()` is false (`egui/src/data/input/viewport_info.rs:95-101`). eframe then neither paints nor runs the UI (`wgpu_integration.rs:648-660`, `:721`), so `frames` stops counting and the run waits instead of failing.

**Root-cause hypotheses:**

| # | Hypothesis | Label | Why |
|---|---|---|---|
| H1 | The one screenshot request is dropped by a skipped paint (`Outdated` after the frame-1 resize, `Timeout` under GPU or CPU contention, or an `Occluded` status that arrives before winit's occlusion event), and Studio never asks again, so the run closes at frame 241. | INFERENCE (strongest) | Every link is FACT above: the early return, no re-request, the close at 241, no log. The trigger cannot be seen without a logger, so it is unproven. It explains a failure that is random, more frequent under load and cured by a retry. |
| H2 | On a GPU canvas that has not settled, the request goes out at frame 180 and leaves 60 frames for the image. One dropped request then ends the run. | INFERENCE | Follows from `:501`, `:506` and `:483`. It narrows H1's window and adds no new cause. |
| H3 | The image arrives but `RgbaImage::from_raw` rejects it, and the run closes silently. | ASSUMPTION (unlikely) | egui's `ColorImage` keeps `pixels.len() == w*h`, so `from_raw` should accept it. The branch is still silent and must say why. |
| H4 | Window occlusion makes the run exit 1. | INFERENCE: ruled out as a direct cause | An occluded window stops the UI, so the run hangs rather than exiting 1. Occlusion can still feed H1 through the `Occluded` surface status. |
| H5 | Another agent's `cargo build` replaces the binary while it runs. | INFERENCE: ruled out | A running process keeps its mapped image. A different binary would not print this message. |

**UNKNOWN:** which surface status, if any, each reported failure hit. Closing check: test 6 below. The new trace prints the status, and the repeat script (test 7) runs under load.

## 3. Behaviour rules

1. **A lost request is asked again.**
   - After a `Screenshot` command is sent, if no `Event::Screenshot` arrives within `SHOT_IMAGE_WAIT_FRAMES` = 10 frames, the request counts as lost.
   - `requested` is cleared and the capture becomes due again under the existing rules: at rest, or `frames >= 180` on the GPU path.
   - At most `SHOT_MAX_REQUESTS` = 5 requests per run. An image that arrives late, after a re-request, is accepted: the first image wins and the window closes.
2. **The give-up frame moves with the requests.** The run gives up when 5 requests are lost, or at frame 240 + 10 × (number of re-requests), whichever comes first. A request is never cut off by the old fixed frame-240 close.
3. **Every failure says why.** On failure, stderr gets exactly one line, `designos: screenshot was not captured: <reason>`, and the exit code is 1. The prefix and the exit code are unchanged, so spec L's capture script still classifies the run as INFRA (L rule 11: exit 1 plus that text means retry, then exit 3). Reasons:
   - `no image after <n> requests (frames <f1>, <f2>, …); last surface status <status>`
   - `the window was not drawn for 30 s (occluded or minimised)`, from rule 5
   - `the image was malformed (<w>x<h>, <bytes> bytes)`, from the H3 branch
   - `the window closed before the capture` (any other close)
   A failed PNG write keeps its own message, `cannot write <path>: <error>`, and also exits 1.
4. **The capture window is never covered.** In screenshot mode the viewport gets `.with_window_level(egui::WindowLevel::AlwaysOnTop)` (egui `viewport.rs:655`), next to the existing `.with_active(false)` (`main.rs:160`). It floats above other windows for the 1–3 s of a capture and does not take focus.
5. **No run waits forever: a frame heartbeat, not a load timer.** In screenshot mode, `main` starts a watchdog thread before `run_native`, but it is **armed only by the first `drive_screenshot` frame**, so loading a large file before the first frame never trips it. The kit takes 15–150 s to load (spec N). Every frame resets it. If 30 s of wall time pass with no frame after the first one, it prints the rule-3 reason `the window was not drawn for 30 s (occluded or minimised)` and calls `std::process::exit(1)`. This covers the occluded or minimised window, where eframe runs no frames (section 2).
6. **The drop becomes visible.** In screenshot mode only, `main` installs a minimal `log` backend that writes `warn` and above from the `egui_wgpu` and `wgpu` targets to stderr, prefixed `designos: wgpu:`. The shell keeps the last such status in the `Shot` for the rule-3 reason. Normal launches install nothing.
7. **Optional trace.** `DESIGNOS_SHOT_TRACE=1` prints one line per frame: `frame`, `sized`, `at_rest`, `requested`, `visible`, and `images seen`. It is off by default.
8. **Test-only fault hook.** In debug builds only (`cfg(debug_assertions)`), `DESIGNOS_SHOT_DROP_IMAGES=<n>` makes `drive_screenshot` ignore the first `n` images, as if their requests were lost. It does not exist in release builds.
9. **Micro-sequence captures** (`shot.seq`, `:436-457`) follow rules 1–3 for each frame of the sequence.

## 4. Data model / schema changes

None to documents. `Shot` gains `requests: u32`, `requested_at: Option<u32>` (frame), `lost_frames: Vec<u32>`, `last_surface_status: Option<String>`, `drop_images: u32` (debug builds only) and `outcome: ShotOutcome { Pending, Written, Failed(String) }`. `SHOT_OK` stays as is, and a new `SHOT_FAILURE: OnceLock<String>` carries the reason to `main`.

## 5. Contracts

- Exit codes are unchanged: 0 when the PNG is written, 1 when it is not. The stderr prefix `designos: screenshot was not captured` is unchanged, with `: <reason>` appended.
- Spec L's capture script contract is unchanged: exit 1 plus that prefix means INFRA, retried twice, then exit 3. After this spec, the remaining INFRA cases name their reason.
- New environment variables: `DESIGNOS_SHOT_TRACE` (all builds) and `DESIGNOS_SHOT_DROP_IMAGES` (debug builds only). No new CLI flags.

## 6. File-format mapping

None.

## 7. UI

None in the product. The capture window is always on top while it exists (rule 4). No chrome changes, so `es-designer` is not needed.

## 8. Edge cases

- **An image arrives after it was counted lost.** The first image wins. A second image that is already queued is ignored, because the window is closing.
- **A capped GPU image arrives after a re-request.** The existing at-rest rule applies (`:423-427`): ask again unless it is the last try.
- **The window cannot reach the requested size** (larger than the screen): `sized` stays false, and the capture is due at frame 60 as today. The image is resized with Lanczos3 (`:435`), as today.
- **A cut-off frame from a run that gave up is never written.** A run either writes a whole PNG or fails with a reason.
- **`--screenshot` in a directory that cannot be written:** the `cannot write` message, as today.

## 9. Non-goals

- Changing eframe or egui-wgpu.
- Headless rendering of the window chrome without a window (spec L's gate already uses the headless `Sim`).
- Making `--screenshot` work in CI without a GUI session.

## 10. Test plan (red first)

The `Sim` cases (1–5) live in `crates/designos-app/src/shell/tests/screenshot_capture.rs`. They build a `Sim` from `--fixture showcase --select none --size 1600x1088 --screenshot <tempdir>/s.png` (the `Sim` screen is 1600x1088, `shell/tests.rs:103`) and read the commands from each frame's `FullOutput.viewport_output`.

| # | Case | Expected | Fails before |
|---|---|---|---|
| 1 | `lost_screenshot_request_is_sent_again` | Run frames until one `ViewportCommand::Screenshot` is sent, then inject no image for 12 frames. A second `Screenshot` command is sent. | yes: the request stays pending (`:495-499`) |
| 2 | `capture_is_written_after_a_lost_request` | After case 1, inject `Event::Screenshot` with a 1600x1088 `ColorImage`. The PNG exists, and `outcome == Written`. | yes |
| 3 | `capture_gives_up_with_a_reason_after_five_lost_requests` | Never inject. `outcome == Failed` and the reason starts with `no image after 5 requests`. | yes: today it fails without a reason |
| 4 | `failure_line_keeps_the_infra_prefix` | Pure `fn failure_line(&ShotOutcome) -> String` starts with `designos: screenshot was not captured: `. | yes: no such fn |
| 5 | `malformed_image_is_reported` | Pure image-to-buffer conversion given a mismatched size returns `Err` naming `<w>x<h>` and the byte count. | yes: silent today |
| 6 | `screenshot_viewport_is_always_on_top` | Pure `fn screenshot_viewport(w, h) -> ViewportBuilder` (moved out of `main`): `window_level == Some(AlwaysOnTop)` and `active == Some(false)`. | yes |
| 6b | `watchdog_is_not_armed_before_the_first_frame` | Pure watchdog state: `arm` is never called, and 60 s pass in a fake clock: no fire. Armed, then no frame for 31 s: fire, with the occluded reason. Frames every 1 s for 60 s: no fire. | yes (no watchdog) |
| 7 | Local, real binary, deterministic: `DESIGNOS_SHOT_DROP_IMAGES=1 target/debug/designos --fixture showcase --select none --size 1440x900 --screenshot x.png` | exit 0 and the PNG exists | red on the **hook-only commit** (build order step 1: the hook exists, the re-request does not): exit 1. Paste that run in the PR. |
| 8 | Local stress: `scripts/screenshot_repeat.sh 50` | 50 runs each of: showcase CPU, showcase `--gpu`, `--menu main`, `--panel picker`, and a `.designos` written by `--apply-spec` at `1600x1000`, once with no load and once while `cargo build -p designos-app` runs in another checkout. 0 failures, or every failure names its reason. | not yet reproduced: this writer saw 35/35 pass with no load (section 2). It is an evidence run, not a red test. |

Run:
- `cargo test -p designos-app --bin designos screenshot_`
- `cargo test --workspace --locked`
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- Cases 7 and 8 locally, with their terminal output captured into the PR. #1141 asks for visual evidence of N repeated runs.

## 11. Performance budgets

- No cost outside screenshot mode: no logger, no watchdog, no new per-frame work.
- In screenshot mode, the re-requests add at most 4 × 10 frames to a failing run. A passing run is unchanged: 12 frames, or 180+ on an unsettled GPU canvas.
- Watchdog: one sleeping thread, for the life of a capture process.

## 12. Dependencies and build order

Independent. Land it before L's bless run if possible. L does not wait for it (L rule 11).

Order:
1. Fault hook `DESIGNOS_SHOT_DROP_IMAGES`, on its own commit. Run case 7 on it: exit 1 is the red.
2. Refactor `screenshot_viewport` and `failure_line` (tests 4, 6).
3. `ShotOutcome` and the reasons (3, 5).
4. Re-request loop (1, 2). Case 7 now exits 0.
5. Logger, trace, frame-heartbeat watchdog (6b).
6. Repeat script and the stress evidence (8).
7. PR (squash).

## Acceptance

1. Tests 1–6 and 6b are green in `cargo test --workspace --locked`, and each was seen failing before its fix (pasted in the PR).
2. Case 7 exits 1 on the hook-only commit (build order step 1) and 0 on the PR build; both runs pasted.
3. Case 8 shows 0 unexplained failures in 250 runs under load and 250 runs without load: every failure line carries a reason (terminal capture in the PR).
4. Spec L's capture script still classifies a failed run as INFRA (exit 1 plus the unchanged prefix).
5. No colour is involved, so the token-contract C-1 pairs (`specs/token-contract.md` section (f)) do not apply.

## 13. Owner rulings that apply

- Spec L rule 11 (2026-10-25): a missing screenshot is INFRA (exit 1 plus the prefix, retried, then exit 3), never a layout failure. This spec keeps that contract.
- 2026-09-22: visual evidence in every PR (case 8 terminal capture).
- "Nothing vanishes silently" (delivery workflow law 1): every failure names its reason (rule 3).
- Standing: `main` is protected (PR, green `Test (macos-14)`, Clippy, Rustfmt, squash).

## Decisions on the open questions (writer, 2026-10-25)

1. **Retry inside Studio, or only in the caller?** Inside Studio. The request is lost below Studio (egui-wgpu), and only Studio knows it is still waiting. The caller's retry (L) stays as a second guard.
2. **Keep exit 1, or add a new exit code?** Keep exit 1 and the prefix, so L's contract and any existing caller keep working. The reason goes in the message.
3. **Always-on-top capture window?** Yes. A capture window that another window covers either stops (occluded) or drops frames. It does not take focus (`with_active(false)` stays), so it does not interrupt typing.
4. **A wall-clock limit as well as the frame limit?** Yes, 30 s, because an occluded window runs no frames and the frame limit never fires.

## Open questions for the owner

None. These are technical calls about a CLI capture path, and the visible product does not change.
