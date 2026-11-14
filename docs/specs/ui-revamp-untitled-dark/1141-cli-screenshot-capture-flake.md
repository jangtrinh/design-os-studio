# R `--screenshot` either captures or says why: no silent "screenshot was not captured"

| Field | Value |
|---|---|
| Issue | #1141 (epic #1118, spec key R) |
| Size | M. About two days: a surface-status hook, a time-aware re-request loop with spaced retries and a give-up floor, reasoned failure messages, an always-on-top capture window, a frame-aware watchdog, rules for micro-sequences, a test-only fault hook, Sim cases on the existing window harness, and an opt-in repeat script for the PR evidence. |
| Depends on | none |
| Unblocks | L (#1126): its baseline and contact sheets come from `--screenshot`. M (docs captures). Every spec whose PR evidence is a CLI capture. |
| Touches | `crates/designos-app/src/shell.rs`:<br>- `SHOT_OK` `:23`, `Shot` `:50-69`, `SHOT_GPU_SETTLE_FRAMES` `:73`<br>- the `Shot` built in `Shell::build` `:398-409`<br>- `drive_screenshot` `:413-490`<br>- `request_screenshot` `:496-512`<br>- call sites: `draw` → `request_screenshot` `:1372`, `draw_window` → `drive_screenshot` `:1386`<br>`crates/designos-app/src/main.rs`: screenshot viewport `:150-167`, `NativeOptions` `:167`, exit `:183-190`<br>New `crates/designos-app/src/shot_status.rs` (`SurfaceLog`, watchdog state)<br>`crates/designos-app/src/shell/tests/gpu_screenshot_wait.rs` (new cases and `run_window_with`; its existing assertions stay green unchanged)<br>New `scripts/screenshot_repeat.sh` |

Code baseline: `origin/main@649da124` (newest on 2026-10-25). Every `shell.rs` and `main.rs` line above was re-anchored there from `205c72ef` with `git show` and a line map. `shell.rs` moved twice in between: +4 lines at `:103`, and +4 more near `:994` (toast code, `7e15ddae`), so lines after about `:994` are +8 (`draw` is at `:1372`, `draw_window` at `:1386`). `main.rs` and `gpu_screenshot_wait.rs` are unchanged. egui citations are to the locked `eframe`, `egui`, `egui-wgpu` 0.35.0 and `wgpu`, `wgpu-core`, `wgpu-hal` 29.0.4 sources (`Cargo.lock`), read in `~/.cargo/registry`. Re-anchor against HEAD and report drift before writing code. Scratch evidence: `$SCRATCH/agents/w5/r/`, where `$SCRATCH` = `/private/tmp/claude-501/-Users-jangtrinh--claude-profiles-work-spares-spare-AW68Nz/a6445c24-c40e-4254-a3e2-5e0bafafd0f4/scratchpad`.

## 1. Summary and user story

As the PM running spec L's baseline and every revamp PR's evidence through `designos <file> --screenshot out.png`, I need each capture to either write the PNG or exit with a reason I can act on, such as "no image after 5 requests; last surface status Occluded" or "window not drawn for 30 s". Today a run sometimes exits 1 with only `designos: screenshot was not captured`. A retry passes, so the failure is random and silent.

## 2. Current state (labels per claim)

**Reported failures (FACT, as reported; not reproduced by this writer):**
- #1141, 2026-10-25 12:57–13:00, no other load, build from `ec47da26`: a fresh 156-node `.designos` made with `--apply-spec` failed 3 times in a row at `--size 1600x1000`, with or without `--select`, and also at `1600x1100`. It had passed a minute earlier. The same frame rendered fine through MCP `designos_render_png`.
- While other agents built and captured in the same checkout, the spec writers saw 5 failures: "5 failures in 16 captures" per the #1141 body, and "5 of 37 runs" per the controller's brief for this spec (the counts differ by source; both are as reported). The failing cases were D-06 (zoom menu), D-09 (Dev Inspect), E-03b, E-04 and F-03. Every retry exited 0.

**This writer's runs (FACT, 2026-10-25 at about 21:38, no other load, binary built from `ec47da26`):** the screenshot code there is identical to `205c72ef` and `649da124` apart from line numbers. The flake did **not** reproduce:
- `designos --fixture showcase --select none --size 1440x900 --no-toast --screenshot run-N.png`: 20 of 20 exited 0 (`$SCRATCH/agents/w5/r/runs.log`).
- The same with `--gpu --size 1600x1000`: 15 of 15 exited 0 (`gpu-runs.log`).
- These runs had no concurrent capture windows (see H6).

**The capture state machine (FACT, `shell.rs` at `649da124`):**
- `drive_screenshot` runs near the top of every `draw_window` (`:1386`), and `request_screenshot` runs after the whole frame has drawn (`:1372`). `drive_screenshot` asks for a repaint every frame (`:416`).
- Frame 1 sets `pixels_per_point` and sends `ViewportCommand::InnerSize` (`:417-423`).
- A capture becomes due when `!requested` and either `frames >= 12` with the content rect within 1.5 pt of the requested size, or `frames >= 60` (`:482-486`).
- `request_screenshot` sends `ViewportCommand::Screenshot` once and sets `requested = true` (`:508-511`). On a GPU canvas that is not at rest, it waits until frame 180 (`SHOT_GPU_SETTLE_FRAMES`, `:73`, `:505`).
- While `requested` is true, nothing asks again (`:499-503`). The only re-request is for an image captured from a capped frame that is not the last try (`:427-431`).
- After 240 frames without an image, the window closes (`:487-489`), except in a micro-sequence (`&& shot.seq.is_none()`). `main` then finds `SHOT_OK` false and exits 1 with `designos: screenshot was not captured` (`main.rs:187-189`).
- An image is accepted whenever it arrives: `drive_screenshot` never checks `requested` (`:424-426`).
- Other exits:
  - An image whose bytes do not fit `RgbaImage::from_raw` closes the window without any message (`:437`, falling through to `:472`).
  - A failed PNG write prints `designos: cannot write …` (`:468`). `SHOT_OK` stays false, so `main` also prints the bare `screenshot was not captured` line and exits 1. Spec L classifies that as INFRA.
- Micro-sequences (`--panel <slug>:seq:<n>`, `:383-396`): each sequence frame resets `frames = 12` (`:455`), so a sequence never reaches the 240-frame close and can hang. A failed `frame-NNNN.jpg` write only logs (`:444-446`) and the sequence continues.
- The screenshot window is built with `.with_active(false)` and no window level (`main.rs:159-160`). `NativeOptions` is `{ viewport, ..Default::default() }` (`:167`), so `wgpu_options` is eframe's default: `WgpuConfiguration::default()` with `SurfaceConfig::LOW_LATENCY` (`eframe/src/epi.rs:453-455`).

**What eframe and egui-wgpu do with a screenshot request (FACT, crate sources):**
- A `Screenshot` command emitted by frame n's UI reaches the viewport's `actions_requested` through `handle_viewport_output` (`eframe/src/native/wgpu_integration.rs:777`), which runs **after** frame n's paint. It is moved into the paint of frame n+1 (`:724-741`). So the request rides the next frame's paint.
- That paint only happens when the viewport is visible (`:721`). When it is not, the command stays queued, and the UI is not run either (`:660`), so `frames` stops counting and the run waits instead of failing.
- `egui_wgpu::winit::Painter::paint_and_update_textures` returns early, **before** capturing, when the surface texture cannot be acquired (`egui-wgpu/src/winit.rs:605-628`). It first calls `WgpuConfiguration.on_surface_status` with the status (`:612`). A request that rides on that paint is dropped: no `Event::Screenshot` ever arrives for it.
- It also returns early when the surface is missing (`winit.rs:563-564`) or a recreate fails (`:519-522`).
- `on_surface_status` is a public field, `Arc<dyn Fn(&wgpu::CurrentSurfaceTexture) -> SurfaceErrorAction + Send + Sync>` (`egui-wgpu/src/lib.rs:346-347`), reached through `eframe::NativeOptions.wgpu_options` (`eframe/src/epi.rs:369`). It is not called for `Success` or `Suboptimal` (`lib.rs:340-345`).
- The default hook (`lib.rs:386-406`) returns `Reconfigure` for `Outdated`, `RecreateSurface` for `Lost` and `SkipFrame` for everything else. It logs `Outdated` and `Occluded` at `trace`, `Lost` at `debug`, and the rest (`Timeout`, `Validation`) at `warn`.
- `wgpu::CurrentSurfaceTexture` has `Success`, `Suboptimal`, `Timeout`, `Occluded`, `Outdated`, `Lost` and `Validation` (`wgpu/src/api/surface_texture.rs:55-86`).
- On macOS, wgpu-hal's Metal `acquire_texture` returns only two errors: `Occluded`, from a synchronous check of the window's occlusion state that does not wait for winit's occlusion event (`wgpu-hal/src/metal/surface.rs:151`), and `Timeout`, when `nextDrawable` returns nil (`:166`). The engineering review read wgpu-core's one-to-one mapping (`present.rs:255-269`); this writer re-read only the two hal lines. So `Outdated` and `Lost` cannot come from acquiring a texture on Metal.
- Two more drop paths, after a successful acquire, log at `error` and lose the image: an unsupported surface format (`egui-wgpu/src/capture.rs:200-205`) and a failed `map_async` (`:207-211`).
- `designos-app` installs no `log` backend: no `set_logger`, `env_logger`, `tracing_subscriber` or `simplelog` in `crates/`. Even with one, a `warn` logger would not see `Occluded`.

**Root-cause hypotheses:**

| # | Hypothesis | Label | Why |
|---|---|---|---|
| H1 | On macOS, the paint that carries the one screenshot request fails to acquire a texture: `Occluded` (the window is covered or off its Space at that moment, before winit reports occlusion) or `Timeout` (`nextDrawable` returned nil under compositor or GPU contention). Studio never asks again, so the run closes at frame 241. | INFERENCE (strongest) | Every link is FACT above: the early return, the two Metal statuses, no re-request, the close at 241, no visible log. It explains a failure that is random, more frequent under load and cured by a retry. Which status each failure hit is UNKNOWN. |
| H1b | The acquire succeeds but the image is still lost: an unsupported surface format or a failed `map_async` (`capture.rs:200-211`), or an image delivered while eframe is not running the UI (`wgpu_integration.rs:660`; Studio implements only `ui`). | INFERENCE | These paths exist in the sources. None of them calls `on_surface_status`, so only the loss timer (rule 1) catches them. |
| H2 | On a GPU canvas that has not settled, the request goes out at frame 180 and leaves 60 frames for the image. One dropped request then ends the run. | INFERENCE | Follows from `:505`, `:510` and `:487`. It narrows H1's window and adds no new cause. |
| H3 | The image arrives but `RgbaImage::from_raw` rejects it, and the run closes silently. | ASSUMPTION (unlikely) | egui's `ColorImage` keeps `pixels.len() == w*h`, so `from_raw` should accept it. The branch is still silent and must say why. |
| H4 | Window occlusion makes the run exit 1 directly. | INFERENCE: ruled out as a direct cause | An occluded window stops the UI, so the run hangs rather than exiting 1. Occlusion still feeds H1 through the `Occluded` status. |
| H5 | Another agent's `cargo build` replaces the binary while it runs. | INFERENCE: ruled out | A running process keeps its mapped image. A different binary would not print this message. |
| H6 | Two capture runs at once (parallel agents, or L's Tier B) open same-size windows at the same default position and cover each other. With rule 4 both float, so the lower one can still be fully covered, and Metal returns `Occluded`. | INFERENCE (plausible) | The reported failures came while other agents were active; this writer's 35 clean runs had no concurrent captures. Closing check: test 13 (20 concurrent pairs, before and after). |

**UNKNOWN:** which surface status, if any, each reported failure hit. Closing check: the status hook (rule 6) puts the status in the failure line and the trace, and tests 12 and 13 run under load and in concurrent pairs.

## 3. Behaviour rules

1. **A lost request is asked again.** After a `Screenshot` command is sent at frame n, it counts as **lost** when no image has arrived and either:
   - (event) the status hook (rule 6) recorded a skipped paint after the request's baseline. The baseline is the `SurfaceLog` skip count read at the start of frame n+1's `drive_screenshot`, because the command rides the paint of frame n+1 (section 2); or
   - (time-aware) **both** 10 frames **and** 250 ms of input time (`ctx.input(|i| i.time)`, wall time since start in a real run) have passed since it was sent.

   A lost request clears `requested`, and its frame and status are kept for the reason line.
2. **Spaced retries, a cap, and a floor before giving up.**
   - After the k-th lost request, the capture becomes due again no earlier than `10 << (k − 1)` frames later (10, 20, 40, 80), and then under the existing due rules (at rest, or `frames >= 180` on the GPU path).
   - At most `SHOT_MAX_LOST` = 5 lost requests per capture. No sixth request is sent.
   - Only lost requests count. The existing at-rest re-ask for an image captured from a capped frame (`:427-431`) is not a lost request.
   - The run gives up at the first frame where all hold: `frames > 240`, at least 4 s of input time since frame 1, and no request is outstanding. An outstanding request is waited out to its loss window (rule 1) first, so a request is never cut off by the frame count.
   - INFERENCE: a failing run therefore lasts at least the later of 241 frames and 4 s, plus at most one loss window and one retry spacing. Today's run gives up at frame 241 whatever the time; on the CPU path, a bare "5 lost requests" rule would have given up near frame 62.
3. **Every failure says why, with the INFRA prefix.** On failure, stderr gets exactly one line, `designos: screenshot was not captured: <reason>`, and the exit code is 1. The prefix and the exit code are unchanged, so spec L's capture script still classifies the run as INFRA (L rule 12; its exception for a case owned by the PR's `--only-spec` key still exits 1 there). Reasons:
   - `no image after <n> requests (sent at frames <f1>, <f2>, …); last surface status <status or "none reported">`
   - `the window was not drawn for 30 s (occluded or minimised)`, from rule 5
   - `the image was malformed (<w>x<h>, <bytes> bytes)`, from the H3 branch
   - `cannot write <path>: <error>`, for a failed PNG or sequence-frame write. Today's separate `designos: cannot write …` line is replaced by this one, so a full disk or an unwritable directory is INFRA in L, never a layout failure.
   - `the window closed before the capture` (any other close)
4. **The capture window floats.** In screenshot mode the viewport gets `.with_window_level(egui::WindowLevel::AlwaysOnTop)` (egui `viewport.rs:655`), next to the existing `.with_active(false)` (`main.rs:160`). The builder moves into a pure `fn screenshot_viewport(w, h) -> ViewportBuilder`. The window does not take focus, but it does float over whatever the user is doing for the 1–3 s of a capture (see rule 11).
5. **No run waits forever: a frame-aware watchdog.** In screenshot mode, `main` starts a watchdog thread before `run_native`. It beats twice per frame: `begin` at the top of `drive_screenshot` and `end` in `request_screenshot`, at the end of the UI pass. The GPU canvas compile, the slow part of a large-page frame, runs between the two (`canvas/gpu.rs:283`). The watchdog:
   - is not armed until the first `begin`, so loading a large file before the first frame never trips it (the kit takes 15–150 s to load, spec N);
   - never fires while a frame is in progress (after `begin`, before `end`), however long that frame takes;
   - fires when 30 s of wall time pass after an `end` with no new `begin`. It prints exactly `designos: screenshot was not captured: the window was not drawn for 30 s (occluded or minimised)` and calls `std::process::exit(1)`.

   INFERENCE: the paint after `end` uploads and draws what the UI pass compiled, so it is not expected to take 30 s. A paint that does would be reported as not drawn.
6. **The surface status becomes visible, through the hook, not a logger.** In screenshot mode only, `main` sets `NativeOptions.wgpu_options.on_surface_status` to a closure that records the status into a shared `Arc<SurfaceLog>` (a skip counter and the last status name, from `{status:?}`), then returns exactly what the default hook returns. The closure wraps the one in `eframe::NativeOptions::default().wgpu_options.on_surface_status`, and the options start from `NativeOptions::default()` with only the viewport and this field changed. eframe's own default builds `wgpu_options` as `WgpuConfiguration::default().with_surface_config(SurfaceConfig::LOW_LATENCY)` (`eframe/src/epi.rs:453-455`), so starting from `WgpuConfiguration::default()` would silently drop that setting. The same `Arc` is handed to the shell after it is built; the `Shot` reads it. Normal launches keep the default options. No `log` dependency or logger is added: the `error`-level capture drops (H1b) stay unlogged, and the reason line says `none reported` for them.
7. **Optional trace.** `DESIGNOS_SHOT_TRACE=1` prints one line per frame: `frame`, `time`, `sized`, `at_rest`, `requested`, `visible`, `images seen`, and the `SurfaceLog` skip count and last status. It is off by default.
8. **Test-only fault hook.** In debug builds only (`cfg(debug_assertions)`), `DESIGNOS_SHOT_DROP_IMAGES=<n>` makes `drive_screenshot` ignore the first `n` images, as if their requests were lost. It does not exist in release builds.
9. **Micro-sequences.** Each sequence frame is its own capture:
   - The loss counters, the retry spacing and the floor of rules 1–2 reset when a sequence frame starts (where `frames = 12` is set, `:455`).
   - A sequence gains a give-up, by the same floor measured from that sequence frame's start. Its reason names the frame: `no image for sequence frame <k> after <n> requests …`.
   - A failed `frame-NNNN.jpg` write fails the run with the `cannot write` reason, instead of logging and continuing.
10. **Compatibility with spec L.** An image that arrives with no outstanding request is still accepted, as today: L's test 8b injects an unsolicited 1×1 image after frame 8. `outcome = Written` is set at the same point where the PNG is saved, which is where L writes `--layout-json`.
11. **Stress runs are opt-in.** `scripts/screenshot_repeat.sh` refuses to start unless `DESIGNOS_SHOT_STRESS_OK=1` is set. It then prints how many windows it will open and the estimated minutes before the first one. It serialises its own captures with a `mkdir`-based lock (`$TMPDIR/designos-capture.lock`) except in the concurrent-pairs mode of test 13.

## 4. Data model / schema changes

None to documents. In-memory only:
- `Shot` gains `outcome: ShotOutcome { Pending, Written, Failed(String) }`, `started_at: Option<f64>` (input time of frame 1), `outstanding: Option<OutstandingRequest { sent_frame, sent_time, skips_baseline: Option<u32> }>`, `lost: Vec<(u32, Option<String>)>` (frame, status), `next_due_frame: u32`, `surface: Arc<SurfaceLog>`, and `drop_images: u32` (debug builds only).
- New `shot_status.rs`: `SurfaceLog { skips: AtomicU32, last: Mutex<Option<String>> }` with `record(&wgpu::CurrentSurfaceTexture)`, and the watchdog state (`begin`, `end`, `poll(now) -> Option<&'static str>`), written against an injectable clock.
- `SHOT_OK` stays. A new `SHOT_FAILURE: OnceLock<String>` carries the reason to `main`. Only the close path writes it, and no test reads it or `SHOT_OK`: tests assert `Shot.outcome`, because the existing tests already set `SHOT_OK` from parallel threads.

## 5. Contracts

- Exit codes are unchanged: 0 when the PNG is written, 1 when it is not. The stderr prefix `designos: screenshot was not captured` is unchanged, with `: <reason>` appended. A failed write now uses that prefix too (rule 3).
- Spec L's capture-script contract is unchanged: exit 1 plus that prefix means INFRA, retried twice, then exit 3, except L's owned cases, which exit 1 (L rule 12). After this spec, every INFRA line names its reason.
- New environment variables: `DESIGNOS_SHOT_TRACE` (all builds), `DESIGNOS_SHOT_DROP_IMAGES` (debug builds only), and `DESIGNOS_SHOT_STRESS_OK` (the repeat script only). No new CLI flags.

## 6. File-format mapping

None.

## 7. UI

None in the product. The capture window floats above other windows while it exists (rule 4). No chrome changes, so `es-designer` is not needed.

## 8. Edge cases

- **An image arrives after its request was counted lost.** The first image wins, whichever request it answers. A second image already queued is ignored, because the window is closing.
- **An image arrives with no request outstanding** (L test 8b): accepted (rule 10).
- **A capped GPU image arrives after a re-request.** The existing at-rest rule applies (`:427-431`): ask again unless it is the last try. That re-ask is not a lost request (rule 2).
- **Fast frames during occlusion.** After an `Occluded` acquire, egui-wgpu returns before `present`, so frames are not paced by vsync until winit reports the occlusion (INFERENCE from `winit.rs:626-628`). The 250 ms half of rule 1 keeps such frames from burning the loss window in milliseconds.
- **Slow frames under `Timeout`.** `nextDrawable` can block for up to 1 s per frame (INFERENCE). The 10-frame half of rule 1 keeps a slow but working window from being counted lost after 250 ms.
- **The window cannot reach the requested size** (larger than the screen): `sized` stays false, and the capture is due at frame 60 as today. The image is resized with Lanczos3 (`:439`), as today.
- **A long legitimate frame** (an uncapped idle-refine compile or a relayout slice on a page above 5,000 nodes): the watchdog sees a frame in progress and does not fire (rule 5).
- **Two concurrent capture runs** (H6): both float, and one can still cover the other. The repeat script serialises its runs; other callers are not serialised by this spec (non-goal; see test 13).
- **A cut-off frame from a run that gave up is never written.** A run either writes a whole PNG or fails with a reason.

## 9. Non-goals

- Changing eframe, egui-wgpu or wgpu.
- Headless rendering of the window chrome without a window (spec L's gate already uses the headless `Sim`).
- Making `--screenshot` work in CI without a GUI session.
- Serialising or offsetting capture windows inside Studio for every caller. If test 13 shows H6, a follow-up issue proposes an in-product lock or a per-run window position.
- A `log` backend.

## 10. Test plan (red first)

The Sim cases live in `crates/designos-app/src/shell/tests/gpu_screenshot_wait.rs`, next to the harness that already models eframe: `run_window` (`:74-114`) sends the image of a request issued at frame n from the paint of n+1 and delivers it `lag` frames later. This spec adds `run_window_with(s, shot, limit, dt, deliver, before_frame)`, where `deliver(request_index, sent_frame) -> Option<usize>` returns the frame whose input carries the image (`None` = dropped) and `dt` is the input time per frame. `run_window` becomes `run_window_with(…, DT, |_, n| Some(n + 1 + lag), …)`, so every existing test keeps its call and its assertions unchanged, including `requests.len() == 1`, "no retries once the settle limit has passed" and `closed_at + 3 < 240` (`:166`, `:176-177`, `:187-188`, `:265-266`). Tests assert `s.shell.shot`'s `outcome`, never `SHOT_OK` or `SHOT_FAILURE`.

Build order step 1 (section 12) adds the seam with no behaviour change: `ShotOutcome` and `Shot.outcome` (set to `Written` where `SHOT_OK` is set, and to `Failed` with an empty reason at today's frame-241 close), `failure_line(&ShotOutcome)` returning today's text, the `surface` field, `run_window_with`, and the fault hook. Every Sim case below then compiles and is red on behaviour.

| # | Case | Expected | Fails before |
|---|---|---|---|
| 1 | `a_lost_request_is_sent_again` | CPU canvas; `deliver` drops request 0 and delivers the rest after `DELIVERY_LAG`. Two requests are sent, the PNG is saved, `outcome == Written`. | yes: one request, close at frame 241, `outcome == Failed` |
| 2 | `an_image_that_arrives_after_frame_241_is_saved` | `deliver` drops request 0, sends request 1's image at frame 260, drops the rest. No `Close` before frame 260; the PNG exists; `outcome == Written`. | yes: `Close` at 241 |
| 3 | `the_run_gives_up_with_a_reason_after_five_lost_requests` | `deliver` drops everything; `limit` 1,000. Exactly 5 requests; the close comes after frame 240 and at least 4 s of input time; the reason starts with `no image after 5 requests`. | yes: one request |
| 4 | `a_skipped_paint_counts_the_request_as_lost_at_once` | `deliver` drops request 0; `before_frame` records an `Occluded` status in the shot's `SurfaceLog` before frame `requests[0] + 2`. The second request is sent at most 12 frames after the first (event plus 10-frame spacing), not after the 25-frame time window; the kept loss names `Occluded`. | yes: one request |
| 5 | `an_image_inside_the_loss_window_is_not_asked_again` | For `(dt, lag)` in `(0.025, 2)`, `(0.2, 3)`, `(0.001, 12)`: one request, PNG saved. This pins the "both 10 frames and 250 ms" rule. | guard (green today) |
| 6 | `a_failed_write_keeps_the_infra_prefix` | `--screenshot <tmp>/blocker/shot.png`, where `<tmp>/blocker` is a regular file. `outcome == Failed("cannot write …")`, and `failure_line(&outcome)` starts with `designos: screenshot was not captured: cannot write `. | yes: `outcome` stays `Pending` |
| 7 | `a_malformed_image_is_reported` | After `frames_until_screenshot`, one frame whose input carries an `Event::Screenshot` with a 1600×1088 `ColorImage` whose `pixels` is one short. `outcome == Failed("the image was malformed (1600x1088, …)")`. | yes: `Pending`, silent close |
| 8 | `a_sequence_frame_retries_a_lost_request_and_finishes` | `--panel <slug>:seq:3` with a slug from `apply_micro_interaction_continuous_frame`; `deliver` drops the first image of sequence frame 2. Three `frame-NNNN.jpg` files exist and the run closes with `outcome == Written` within `limit` 1,500. | yes: the sequence never closes |
| 9 | `screenshot_viewport_is_always_on_top` | Pure `screenshot_viewport(w, h)`: `window_level == Some(AlwaysOnTop)`, `active == Some(false)`. | not red-first: a new pure function moved out of `main`; it guards the refactor |
| 10 | `watchdog_fires_only_when_no_frame_runs` | Fake clock. Never armed, 60 s pass: no fire. `begin` at t, no `end` until t+40: no fire. `end` at t+40, nothing for 31 s: fires with the exact rule-5 line. `begin`/`end` every 1 s for 60 s: no fire. | not red-first: no watchdog exists today; behaviour is pinned from the first commit |
| 11 | Local, real binary, deterministic: `DESIGNOS_SHOT_DROP_IMAGES=1 target/debug/designos --fixture showcase --select none --size 1440x900 --screenshot x.png` | exit 0 and the PNG exists | red on the **seam commit** (build order step 1): exit 1. Paste that run in the PR. |
| 12 | Local stress, opt-in (rule 11): `DESIGNOS_SHOT_STRESS_OK=1 scripts/screenshot_repeat.sh 50` | 50 runs each of: showcase CPU, showcase `--gpu`, `--menu main`, `--panel picker`, and a `.designos` written by `--apply-spec` at `1600x1000`, once with no load and once while `cargo build -p designos-app` runs in another checkout. 0 failures, or every failure names its reason. | evidence run, not a red test: this writer saw 35/35 pass with no load |
| 13 | Concurrent pairs, opt-in: `DESIGNOS_SHOT_STRESS_OK=1 scripts/screenshot_repeat.sh --pairs 20` | 20 pairs of showcase captures started together, without the lock, on the base build (0.6.2, `$SCRATCH/tools/designos-v062`, per the owner ruling that every Studio CLI run uses 0.6.2) and on the PR build. The PR build has 0 unexplained failures; every failure line names its status. A base-build failure rate above 0 supports H6. | evidence run for H6 |

Run:
- `cargo test -p designos-app --bin designos gpu_screenshot_wait`
- `cargo test -p designos-app --bin designos shot_status`
- `cargo test --workspace --locked`
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `bash -n scripts/screenshot_repeat.sh`
- Case 11 locally. Cases 12 and 13 only when the controller has scheduled them and told the owner (each opens always-on-top windows: case 12 about 500 windows, roughly 10–25 min; case 13, 80 windows), with their terminal output captured into the PR. #1141 asks for visual evidence of N repeated runs.

## 11. Performance budgets

- No cost outside screenshot mode: default `wgpu_options`, no watchdog, no new per-frame work.
- In screenshot mode, a passing run is unchanged: 12 frames, or 180+ on an unsettled GPU canvas. The hook adds one atomic increment and one short lock per skipped paint.
- A failing run lasts at least the later of 241 frames and 4 s (rule 2); it no longer gives up earlier than today.
- Watchdog: one sleeping thread, for the life of a capture process.

## 12. Dependencies and build order

Independent. Land it before L's bless run if possible. L does not wait for it (L rule 12).

**Cross-spec with L.** R and L both change the write block of `drive_screenshot`: L writes `--layout-json` on the PNG frame. Whichever lands second rebases onto the other. R keeps rule 10 (unsolicited images accepted, `outcome = Written` at the save), which L's test 8b relies on. L's capture script may adopt the repeat script's lock (rule 11); that is a note for L's owner, not part of this spec.

Order:
1. The seam, on its own commit: `ShotOutcome`, `Shot.outcome`, `failure_line` (today's text), `Shot.surface`, `run_window_with`, the fault hook. No behaviour change. Run case 11 on it: exit 1 is the red.
2. Cases 1–8 written and seen red (case 5 green).
3. Reasons and the write and malformed branches (6, 7).
4. Time-aware loss, spacing, cap and floor (1, 2, 3, 5).
5. The status hook in `main` and the event-driven loss (4).
6. Micro-sequence rules (8).
7. `screenshot_viewport` with `AlwaysOnTop` (9), the watchdog (10), the trace.
8. The repeat script with the lock and the opt-in; cases 12 and 13 when scheduled.
9. PR (squash).

## Acceptance

1. Cases 1–4 and 6–8 are green in `cargo test --workspace --locked`, and each was seen failing **on behaviour** against the seam commit (output pasted in the PR). Cases 5, 9 and 10 are green.
2. Every existing test in `gpu_screenshot_wait.rs` passes without an edit to its body.
3. Case 11 exits 1 on the seam commit and 0 on the PR build; both runs pasted.
4. When scheduled: case 12 shows 0 unexplained failures in 250 runs under load and 250 runs without load, and case 13 reports its pair results for both builds (terminal captures in the PR). Every failure line carries a reason and, where the hook saw one, a status.
5. Spec L's capture script still classifies every failed run, including a failed write, as INFRA (exit 1 plus the unchanged prefix).
6. No colour is involved, so the token-contract C-1 pairs (`specs/token-contract.md` section (f)) do not apply.

## 13. Owner rulings that apply

- Spec L rule 12 (2026-10-25): a missing screenshot is INFRA (exit 1 plus the prefix, retried, then exit 3), never a layout failure; a case owned by the PR's `--only-spec` key exits 1. This spec keeps that contract and extends it to failed writes.
- 2026-09-22: visual evidence in every PR (cases 11–13 terminal captures).
- "Nothing vanishes silently" (delivery workflow law 1): every failure names its reason (rule 3).
- Standing: `main` is protected (PR, green `Test (macos-14)`, Clippy, Rustfmt, squash).

## Decisions on the open questions (writer, 2026-10-25)

1. **Retry inside Studio, or only in the caller?** Inside Studio. The request is lost below Studio (egui-wgpu), and only Studio knows it is still waiting. The caller's retry (L) stays as a second guard.
2. **Keep exit 1, or add a new exit code?** Keep exit 1 and the prefix, so L's contract and any existing caller keep working. The reason goes in the message.
3. **Always-on-top capture window?** Yes. A covered capture window either stops (occluded) or drops paints. It does not take focus (`with_active(false)` stays). It is visible over the user's work, which is why the stress runs are opt-in (rule 11).
4. **A wall-clock limit as well as the frame limit?** Yes, 30 s with no frame in progress, because an occluded window runs no frames and the frame limit never fires. It never fires inside a frame, so a slow compile is not killed.
5. **Logger or status hook?** The hook. A `warn` logger cannot see `Occluded`, `Outdated` or `Lost`, which egui-wgpu logs at `trace` or `debug`, and `Occluded` is the most likely status on macOS. The hook sees every status and lets the shell count a request as lost at once.
6. **Frame counts or time for the loss window?** Both. Frames are not paced while Metal skips paints, and `Timeout` frames are slow, so neither measure alone works (section 8).

## Open questions for the owner

None. These are technical calls about a CLI capture path, and the visible product does not change. Scheduling the opt-in stress runs (cases 12 and 13) is a controller task: they put always-on-top windows over the owner's desktop.

## Changelog

- 2026-10-25, RS review R1 (blocker): rule 6 is now an `on_surface_status` hook (`egui-wgpu/src/lib.rs:346-347`, through `eframe::NativeOptions.wgpu_options`, `epi.rs:369`), checked in the locked 0.35.0 source; the name is as the review gave it. It records each status and delegates to the default closure taken from `NativeOptions::default()`, so eframe's `LOW_LATENCY` surface setting (`epi.rs:453-455`) is kept. The `log` dependency is dropped. H1 is rewritten for Metal (`Occluded`, `Timeout`), H1b covers the `map_async`, format and not-running-UI drops, and the reason line and trace carry the status.
- 2026-10-25, RS review R2: a failed write is now a rule-3 reason with the INFRA prefix (`cannot write <path>: <error>`), tested by case 6. "L rule 11" became "L rule 12", with L's owned-case exception (rule 3, §5, §12, §13).
- 2026-10-25, RS review R3: loss needs both 10 frames and 250 ms, or a skipped paint reported by the hook; retries are spaced 10/20/40/80 frames; only lost requests count toward the cap of 5; the run never gives up before the later of frame 240 and 4 s (rules 1–2, cases 2, 3, 5).
- 2026-10-25, RS review R4: the Sim cases moved into `gpu_screenshot_wait.rs` on `run_window_with`; that file is in Touches and its existing assertions stay unchanged. A seam commit makes the cases red on behaviour; case 2 is now "an image after frame 241", which is red today. Tests assert `Shot.outcome`, never the globals. The new pure tests (9, 10) are labelled not red-first.
- 2026-10-25, RS review R5: the watchdog beats at frame begin and end and never fires inside a frame; its exact line is in rule 5; case 10 covers the in-frame case.
- 2026-10-25, RS review R6: micro-sequences reset the counters per sequence frame, gain a give-up, and fail on a frame-write error (rule 9, case 8).
- 2026-10-25, RS review R7: H6 (concurrent capture windows) added, with case 13 (20 pairs, before and after) and a lock in the repeat script; an in-product lock is a follow-up only if H6 is confirmed.
- 2026-10-25, RS review R8: the stress runs are opt-in (`DESIGNOS_SHOT_STRESS_OK=1`), print their window count first, and run only when the controller has scheduled them and told the owner.
- 2026-10-25, RS review R9: the cross-spec note with L is in §12, and rule 10 keeps L's unsolicited-image test and JSON write point working.
- 2026-10-25, controller amendment 1: the base build for case 13 is 0.6.2 (owner ruling: every Studio CLI and MCP run uses 0.6.2). The writer's 35 runs in §2 were on an `ec47da26` build and are labelled so.
- 2026-10-25, review of rule 6: the hook wraps the closure from `eframe::NativeOptions::default()`, whose `wgpu_options` carries `SurfaceConfig::LOW_LATENCY` (`epi.rs:453-455`); §2 says the same.
- 2026-10-25, drift: re-anchored from `205c72ef` to `origin/main@649da124`. Lines after about `shell.rs:994` move +8, not +4 (`draw` `:1372`, `draw_window` `:1386`).
