# L Layout-unchanged gate: baseline geometry of every surface, and the check each revamp PR runs

| Field | Value |
|---|---|
| Issue | #1126 (epic #1118, spec key L) |
| Size | L. Four to five days. Work items:<br>- a snapshot module<br>- launch-faithful `Sim` defaults<br>- a hermetic agent mode shared by the test tier and the release binary<br>- six stub scripts<br>- a comparer that applies one rule to both axes<br>- one CLI flag<br>- the bless procedure<br>- a local capture script |
| Depends on | none. **Lands first**, before any revamp PR (A–K) merges. Does not wait on #1141 (rule 12). |
| Unblocks | D–K (each runs the gate), M (reuses the capture list), I (reuses the hermetic mode for its own captures), the re-bless at the end of the revamp |
| Touches | New `crates/designos-app/src/layout_snapshot.rs`<br>`crates/designos-app/src/widgets/mod.rs:245-283` (the `record` hook and its `RecordDisabled` guard; the hook becomes listable)<br>`crates/designos-app/src/canvas/vector.rs:1125` (writes `test-rect` directly; switch to `widgets::record`)<br>`crates/designos-app/src/shell.rs`:<br>- `Shell::new` launch defaults (`:98`, `:107-110`, `:121`, `:123`) → `Shell::apply_launch_defaults`<br>- `Shell::build` `:132`, panel match `:276`, agent branches `:277-321` (the wall-clock loop in `agents-chat` is at `:287-294`), `terminal` `:327-329`<br>- screenshot write `:455-470`; `drive_screenshot` is called at the top of `draw_window`, `:1386`<br>- title bar `:1410`, tab bar `:1436-1442`, region rects `:1473-1488`<br>`crates/designos-app/src/terminal.rs`:<br>- `build_agent_shell_cmd` `:4365-4391`<br>- the Shell-preset arm `:646`<br>- `configure_pty_env_with_session` PATH `:4870-4892`<br>- `runtime_binary_available` `:335-339` and `_in` `:341`<br>- `spawn_with_session_cmd` `:620-624`<br>- `pin_installed_catalog_clis` `:11259` (promoted to test support)<br>`crates/designos-app/src/agent_cli_catalog.rs:200-215` (`resolve_binary`)<br>`crates/designos-app/src/agent_session_failure.rs:543-590` (`warm_cli_version_cache`, `resolve_cli_executable`)<br>`crates/designos-app/src/main.rs` (hermetic startup check next to `:149`; module list)<br>`crates/designos-app/src/cli.rs` (`Options` `:5-102`, flag match `:219-337`, `USAGE` `:353-371`)<br>New test files under `crates/designos-app/src/shell/tests/layout_unchanged/` (one per spec key; registered next to `:1114-1182` of `shell/tests.rs`)<br>`Sim` in `crates/designos-app/src/shell/tests.rs:53-125`<br>New data dir `crates/designos-app/tests/layout-baseline/` (`cases.json`, `allow.json`, `<case>.layout.json`, `stubs/`)<br>New `scripts/layout_baseline_capture.py` |

Code baseline: `origin/main@7e15ddae`, fetched 2026-10-25. It includes:
- #1106 (`b598a499`): icon-only rail, `RAIL_WIDTH` 48
- #1108 (`07b961c8`): a draggable Pages/Layers separator (`panels/left_sep.rs`, `theme.rs:291-295`)
- `7e15ddae`: the export fallback

Every file:line was re-anchored from `205c72ef` with a line diff of each cited file (`git show` at both commits). Between them, `shell.rs` moved +4 after line 102 and +8 after about line 1000. `theme.rs` moved +8 after line 290, and `shell/tests.rs` moved +2 after line 1114. `terminal.rs`, the `agent_*` files, `cli.rs`, `main.rs`, `widgets/mod.rs`, `tabs.rs` and `designos-library` did not change. Re-anchor against HEAD and report drift before writing code.

egui citations refer to the locked `egui 0.35.0` source. This revision answers `reviews/L-engineering-review-r2.md` and applies `reviews/L-counsel-kongming.md`; the deviations from counsel, with evidence, are in the changelog at the end. Controller-only evidence (not needed to implement) is in `$SCRATCH/agents/w5/l/`, where `$SCRATCH` = `/private/tmp/claude-501/-Users-jangtrinh--claude-profiles-work-spares-spare-AW68Nz/a6445c24-c40e-4254-a3e2-5e0bafafd0f4/scratchpad`.

## 1. Summary and user story

As the PM merging the revamp PRs (D–K), I need proof that a PR changed colours, type and control sizes but not the layout. "Layout unchanged" means three things (plan.md, "Shared naming contract"): the same regions, the same panel widths, and the same control order. Heights and widths may change only where R5 changes a control or type size. Each such change is declared exactly and reviewed.

Pixel equality cannot prove this, because every colour changes by design. This spec splits the evidence into two tiers:
- **Tier A (CI gate):** a deterministic, host-independent geometry check of every surface. It runs in `cargo test` on every PR.
- **Tier B (local):** real-window PNGs of the same surfaces for side-by-side review, and a pixel freeze of the canvas.

Neither tier ever starts a real agent CLI or reads the owner's state.

## 2. Current state at `7e15ddae` (FACT unless labelled)

**Existing tools and why none of them is the gate.**

| Tool | What it does | Why it is not the gate |
|---|---|---|
| `scripts/measure_ui.py` | Crops fixed rects from `--screenshot` PNGs and pixel-diffs them against Figma captures | Pixel parity; hard-coded paths; gitignored references |
| `scripts/capture-visual-proof.sh` | 16 captures into `docs/images/`; also `--panel agents-chat` with the host `PATH` (it "starts a live agent session") | Overwrites docs, no comparison, not hermetic |
| `ui vr` (external) | Pixelmatch diff | Pixel-based; a missing baseline passes |
| `designos-diff compare/suite` (`crates/designos-diff/src/main.rs:16-48`, `suite.rs:11-31`) | Pixel diff with tolerance and ignore rects | The right tool for the canvas pixel freeze only (rule 10) |

There is no layout dump today.

**Named widget rects.**
- `widgets::record(ui, name, rect)` is a test hook (`widgets/mod.rs:245-252`). Under `cfg(test)` it stores the rect under `("test-rect", name)` unless a `RecordDisabled` guard is alive (`:248-250`, guard `:259-280`). In other builds it is a no-op (`:281-283`).
- About 510–528 call sites in about 70 files. The names are developer-written, for example `left-panel-resize-handle` (`shell.rs:1561`).
- Tests read them with `Sim::rect_of` (`shell/tests.rs:193-194`). The map cannot be listed.
- `canvas/vector.rs:1125` writes `test-rect` directly.

**Regions and window chrome.**
- `Shell::draw` (`shell.rs:1363`) → `draw_window` (`:1375`). That runs `drive_screenshot` (`:1386`), then draws:
  - the title bar, only when `tabs.always_visible && shot.is_none() && present.is_none()` (`:1410`)
  - the tab bar when `tabs.is_bar_visible` (`:1436-1442`; `always_visible || entries.len() > 1`, `tabs.rs:204`)
  - the regions (`:1473-1488`), plus the Home, Present and variables-full views (`:1447-1466`, `:1509-1514`)
- The canvas is one rect (`:1517-1522`).
- Widths: `RAIL_WIDTH` 48 (`theme.rs:263`), `LEFT_PANEL_WIDTH` 240 (`:276`), `RIGHT_PANEL_WIDTH` 240 (`:323`), `RULER_SIZE` 20 (`:532`).
- Mixed row heights in one panel: `SECTION_TITLE_HEIGHT` (`:347`), `FIELD_HEIGHT` (`:351`), `LINE_CAPTION` 14 (`:125`).

**The production launch path is `Shell::new`; there is no other.**
- `main.rs` always calls `Shell::new` (`main.rs:180`). It computes `screenshot = opts.screenshot.is_some()` (`shell.rs:98`). That flag gates:
  - the rail default, which opens the Agents panel when there is no panel and no file (`:107-110`)
  - the GPU preference (`:121`)
- `Shell::new` sets `tabs.always_visible = true` (`:123`); `Shell::build` (`:132`) does not.
- Rulers are on except under `cfg(test)` (`:214`).
- A library is opened only with `--home`, `--open-id` or `--library`, or when not in screenshot mode (`:94-96`).

**egui 0.35.0.**
- `prev_pass.widgets` is public (`context.rs:2644`, `:3861`). `layers()` yields `id`, `layer_id`, `rect`, `interact_rect`, `sense` and `enabled` (`widget_rect.rs:9-50`, `:120-129`).
- `by_layer` is a `HashMap` (`:96`). Every `Ui` has an entry (`:91`).
- `info(id)` (`:230`) returns a value only in debug builds with `style.debug.show_interactive_widgets` on (`context.rs:1500-1515`).
- An overflowing `ScrollArea` adds a `Sense::DRAG` area (`containers/scroll_area.rs:832-841`) and a scrollbar tagged `WidgetType::ScrollBar` (`:1325`, `:1334`, `:1336`).
- The workspace overrides only `[profile.dev.package."*"] opt-level` (round-2 review), so `debug_assertions` is on in `cargo test`.

**The headless harness.**
- `Sim` builds from `Options` (`shell/tests.rs:72-91`) and runs `ctx.run_ui` at 1600x1088 (`:103`, `:117`).
- It keeps egui shapes but never rasterises them, so it produces geometry only.
- Both binaries share `src/main.rs`.
- CI runs `cargo test --workspace --locked`. The app binary runs 1,565 tests in 63–64 s on `macos-14` (round-2 review).

**Agent launch and resolution (the source of round-2 blocker N1).**
- Agent tabs run `$SHELL` (`terminal.rs:4345-4351`). Outside `cfg(test)` they get `-l -i -c "exec <cli …>"` (`build_agent_shell_cmd`, `:4365-4391`, with `-l`/`-i` at `:4380-4381`). Shell-preset tabs also get `-l` (`:646`).
- **On this Mac, a login shell puts the real CLIs back on `PATH` whatever the parent set.**
  - `/etc/paths.d/homebrew` contains `/opt/homebrew/bin`.
  - `/etc/zprofile:10-11` and `/etc/profile:3-4` run `eval \`/usr/libexec/path_helper -s\``.
  - `claude`, `codex`, `agy` and `gemini` live in `/opt/homebrew/bin` (counsel ledger).
- The `PATH` override is `#[cfg(test)]` only (`:4870-4874`). Release builds prepend `path_prepend` and `extra_cli_path_dirs()` to the host `PATH` (`:4880-4891`).
- Every launch and resume command starts with the bare binary name: the presets at `:250-276` and the resume commands at `agent_sessions.rs:464-501`. The start-failure fixture is `"agy --mcp-config /nonexistent/mcp.json"` (`terminal.rs:87`); it spawns and busy-waits up to 5 s (`:2542-2554`).
- Four resolvers read the host:
  - `runtime_binary_available` (`:335-339`) is used by `AgentCliPreset::is_installed` (`:294-299`), **in test builds too**. The composer spawns a tab only when `is_installed` (`agent_composer.rs:746-748`), so `agents-chat` geometry depends on what the host has installed even in `Sim`.
  - `SystemCommandRunner::resolve_binary` (`agent_cli_catalog.rs:200-215`) and its probes `claude --version` and `claude auth status` (`:110-111`). Under `cfg(test)` the probe runner is `DefaultTestRunner`, with no process and every CLI reported installed (`:434-466`).
  - `warm_cli_version_cache` (`agent_session_failure.rs:543-567`) is called before every spawn (`terminal.rs:624`). It runs `<resolved> --version` on a thread, resolving through the host `PATH` (`:572-590`), **in test builds too**. Today's shell tests therefore run the real `claude --version` on a developer Mac.
- Test builds without `path_override` launch agent presets as `sh -c 'sleep 5'` (`terminal.rs:656-661`).
- **Owner state.**
  - `Library::default_root()` honours `DESIGNOS_LIBRARY` (`designos-library/src/store.rs:456-462`).
  - The fallback session dirs and the startup sweep live under `std::env::temp_dir()/designos-sessions` (`terminal.rs:1428`, `:1876-1888`, `agent_sessions.rs:266-269`), and `temp_dir()` follows `TMPDIR`.
  - **macOS sets `TMPDIR` for every login session.** This session's value is `/var/folders/43/…/T/`. So "`TMPDIR` is set" is always true and isolates nothing (see changelog, counsel deviation 1).

**Home.** The card text `edited(now, f.modified)` (`home/main_view.rs:453`) reads the library index field `modified`. The library writes that field as `now` (`store.rs:653`); it is not a file mtime.

**Screenshot path.**
- `drive_screenshot` (`shell.rs:413-490`) writes the PNG at `:455-470`.
- `main.rs:187-190` exits 1 with `screenshot was not captured` for every run that wrote no PNG.
- #1141 is open. Spec R adds a reason to that message and keeps the prefix and the exit code.

## 3. Behaviour rules

1. **One snapshot function, two callers, the same window.**
   - `layout_snapshot::capture(ctx, &ShellRegions) -> LayoutSnapshot` reads the named rects of the last completed pass and `prev_pass.widgets`.
   - Callers:
     - the Tier A gate, after its frames
     - the release app, on the PNG frame, when `--layout-json OUT.json` is given (`shell.rs:455-470`)
   - `Shell::draw` copies its region rects into `app.ui.regions` every frame, with no allocation.
   - **`Shell::apply_launch_defaults(&mut self, screenshot: bool)`** holds what `Shell::new` does today: `always_visible`, tab sync, the rail default when `!screenshot` (`:107-110`), and rulers on.
     - `Shell::new` calls `apply_launch_defaults(opts.screenshot.is_some())`. The `wgpu` preference stays in `new` (`:121`).
     - `screenshot == true` also sets `app.ui.screenshot_chrome = true`, and the title-bar condition (`:1410`) adds `&& !screenshot_chrome`.
     - There is no other production path.
     - `Shell::new` also loads `pages_height` from `Library::default_root()` (`shell.rs:102-105`, #1108). That load stays in `new`:
       - Tier A builds with `Shell::build` and never runs it.
       - Tier B neutralises it through `DESIGNOS_LIBRARY`.
       - A non-hermetic capture (the docs scripts) still reads the owner's setting.
   - The gate builds every case with `Shell::build`, then `apply_launch_defaults(true)`, except `d-window-chrome`, which uses `false` so that the title bar and tab bar draw as at launch. `d-window-chrome` therefore lands on the Agents panel (`:107-110`). Rule 6's seams apply to every case, so it probes nothing.
2. **The record hook can be listed.**
   - `record` keeps its temp-data write and its `RECORD_DISABLED` return.
   - It also pushes `(name, global rect)` to a per-pass list. At the **end** of `Shell::draw`, that list moves to `last_pass_names`, which is what `capture` reads.
   - Under `cfg(test)` the push is always on. Elsewhere it is on only while `layout_snapshot::RECORDING` (a relaxed `AtomicBool`) is set, which only `--layout-json` does.
   - The last rect wins, and the count is kept.
   - `vector.rs:1125` calls `record`.
3. **Snapshot content** (schema in section 5):
   - size and scale
   - regions: `title_bar`, `tab_bar`, `rail`, `left_panel`, `canvas`, `notes`, `right_panel`, `home`, `present`, `variables_full`; `null` when absent
   - overlays, in `ctx.memory(|m| m.areas().order())` order
   - `named`, with region by centre
   - `unnamed`: `sense.interactive()` widgets that are not named and not scroll internals, sorted by (region, y, x)
   - `scroll`: the scroll internals, as `{region, axis}`
   - floats rounded to 0.25 pt; keys sorted; no egui `Id`s
   - **Scroll internals.** In the gate, `Sim` turns on `style.debug.show_interactive_widgets` **after** `theme::apply`, and the gate asserts `cfg!(debug_assertions)`.
     - A scrollbar is a widget whose `info(id)` is `WidgetType::ScrollBar`.
     - A drag area is a `Sense::DRAG`-only widget whose rect contains a scrollbar's rect.
     - In `--layout-json` (no widget info in release), the same sense pattern is marked `"maybe": true`.
4. **The Tier A gate.**
   - It reads `cases.json` and runs every case's Tier A entry on the starter (rule 14). It skips a case only when rule 14 lists it as unreachable in Tier A.
   - **One `#[test]` per spec key** (`layout_unchanged_d`, `…_e`, …) so they run in parallel. They run only in the `designos` binary (`env!("CARGO_BIN_NAME")`).
   - Each case:
     - parses `args` with `cli::parse`
     - builds a `Sim` at 1440x900, scale 1, with rule 6's seams
     - calls `apply_launch_defaults` (rule 1)
     - runs 8 frames
     - captures the snapshot and compares it under G1–G5
   - A case may wait for a **stub's exit** (bounded at 2 s, using the bounded `is_alive` poll at `terminal.rs:2949-2951`). It never waits for an agent's output or for wall-clock time.
5. **Hermetic agent mode: one predicate, deny by default.**
   - **One resolver, two pure functions.** New module `crates/designos-app/src/cli_resolve.rs`:
     - `cli_search_dirs_with(&SearchSources) -> Vec<PathBuf>`
     - `resolve_in(dirs, bin) -> Option<PathBuf>`

     `SearchSources { hermetic, scoped, explicit, host_path, extra }` is filled by one wrapper, `cli_search_dirs(explicit)`. The wrapper is the only code that reads the environment, the `OnceLock` and the thread-local. Precedence, first match wins:
     1. `hermetic`: release with `DESIGNOS_AGENT_CLI_DIR` set (`hermetic_agent_cli_dir()`, a `OnceLock`) → exactly `[DIR]`.
     2. `scoped`, `cfg(test)` only: a thread-local `ScopedCliDirs` guard that **only the gate's `Sim` enters**, for the life of one case (rule 6) → exactly `[stub_dir()]`.
     3. `explicit`: a caller's own path list → only that list, as today. For example `runtime_binary_available_in(bin, Some(path_override), _)`; the start-failure wait at `terminal.rs:2947-2953` relies on it.
     4. Otherwise: the host `PATH` plus `extra_cli_path_dirs()`, as today.

     Outside a gate case, a test build resolves exactly as it does now. The 15 tests that set `path_override`, the `install_stub_claude` helpers (`terminal.rs:11270`), the composer's auto-spawn condition (`agent_composer.rs:746`) and `gemini_live_check` (`terminal.rs:8766`) keep their current behaviour (changelog R2).
   - **The five host resolvers all go through it:**
     - `runtime_binary_available_in` (`terminal.rs:341`; used by `AgentCliPreset::is_installed`, `:294-299`)
     - `SystemCommandRunner::resolve_binary` (`agent_cli_catalog.rs:200-215`)
     - `resolve_cli_executable` and the cache key next to it (`agent_session_failure.rs:552`, `:572-590`). Resolution happens on the calling thread, before the `thread::spawn` at `:560`, so the scoped guard applies (H11).
     - `resolve_grok_binary` (`agent_usage.rs:601-624`). It serves `grok usage <sid>` (`:563`), which `refresh_tab_usage` runs for any Grok tab with a session id (`terminal.rs:3031-3035`). Its `path_prepend` dirs and its fallbacks apply only at precedence 4. `extra_cli_path_dirs()` always adds `/opt/homebrew/bin` and `/usr/local/bin` (`terminal.rs:328-329`).
   - **Census.** A test fails if `split_paths(` or `var_os("PATH")` appears in `crates/designos-app/src` outside `cli_resolve.rs` and `configure_pty_env_with_session` (H10). Today's readers are:
     - `terminal.rs:336`, `:353`, `:4875`, `:4888`
     - `agent_cli_catalog.rs:206`, `:253`
     - `agent_session_failure.rs:552`, `:574`
     - `agent_usage.rs:609`
   - With `is_installed`, the picker probes, the `--version` warmer, `grok usage` and spawning all reading the same list, they agree by construction.
   - **Spawn in hermetic mode** (release with `DIR` set):
     - `build_agent_shell_cmd` and the Shell-preset arm use `/bin/sh` and ignore `$SHELL`, with **no `-l` and no `-i`**.
     - The command must be one simple command: after an optional leading `exec `, it contains none of `;`, `&&`, `||`, `|`, a backtick or `$(`. Its first word is a bare name, rewritten to `DIR/<name>`, and that file must exist. Anything else is refused: the tab fails to start with `hermetic mode: refused command <first word>`, and nothing falls back to another lookup.
     - `configure_pty_env_with_session` sets `PATH=DIR:/usr/bin:/bin`, removes `BASH_ENV` and `ENV`, and returns before the prepend at `:4880` (the same shape as the `cfg(test)` branch at `:4870-4874`).
   - **Deny by default at startup.** The check runs in `main.rs` **before** `agent_session_files::spawn_startup_sweep()` (`:149`), so a denied run touches nothing. If `DESIGNOS_AGENT_CLI_DIR` is set, the binary exits 2 unless all of these hold:
     - `--screenshot` is given
     - `DESIGNOS_LIBRARY`, `TMPDIR`, `HOME` and `DESIGNOS_AGENT_CLI_DIR` each name an existing directory that contains a `.designos-hermetic` marker file. The script creates the markers fresh per run.
       - `TMPDIR` needs a marker because macOS always sets it (section 2).
       - `HOME` needs one because `extra_cli_path_dirs()`, skills and transcript readers derive paths from it.

     The message names each missing requirement.
   - **Acknowledgement.** Once the check passes, and before the first frame, the binary writes `$TMPDIR/.designos-hermetic-active` with its pid and `DIR` (H9). The capture script accepts nothing else as proof that the binary it ran was hermetic. A binary that ignores the variable writes no acknowledgement, for example a build from before L, a stale `/Applications` app, or a wrong `--bin` (section 5).
   - **`warm_cli_version_cache`** resolves only through `cli_search_dirs()`, which closes today's `cfg(test)` leak.
   - **Stubs are the fake sessions.**
     - Six POSIX scripts (`claude`, `codex`, `agy`, `grok`, `gemini`, `opencode`) are checked in under `crates/designos-app/tests/layout-baseline/stubs/`. Both tiers use the same bytes: `stub_dir()` in test builds points at them, and a test asserts the old `stub_agent_cli_dir()` writer (`shell/tests.rs:31-50`) is replaced by them.
     - Behaviour:
       - `--version`: a fixed string, exit 0
       - `auth status` or `login status`: fixed, exit 0
       - `agy` with `--mcp-config`: one error line, exit 1 (the start-failure state)
       - anything else: append `"$0 $*"` to `$DESIGNOS_STUB_LOG` if set, then `exec /bin/cat`
     - Stubs print **no** agent output, so geometry never depends on output timing.
6. **Tier A seams** (instance-scoped, test only, applied to **every** case before `match opts.panel` at `shell.rs:276`). A `#[cfg(test)] Options.agent_cli_dir` sets:
   - `terminal.path_override = DIR`, the whole `PATH`, not prepended
   - the thread-local `ScopedCliDirs` guard (rule 5, precedence 2), entered for the case's lifetime and dropped after its snapshot
   - `picker.settings_root_override = <case temp dir>`
   - `app.libs.root = <case temp>/lib` (`libraries/mod.rs:114`), and `opts.library` = a seeded temp library for cases that need one
     - Without this, `App::library()` falls back to the owner's `Library::default_root()` (`libraries/mod.rs:190-196`, `store.rs:456-462`).
     - `k-libraries` (`shell.rs:352`) lists libraries through that root (`libraries/mod.rs:207-223`).
   - `pin_installed_catalog_clis(all six)`, promoted from `terminal.rs:11259` to test support
   - `bind_session(<case id>)` under a case temp dir
   - plus a `#[cfg(test)] Options.fixed_now` for Home

   Agent cases become state-driven:
   - `agents-chat` loses its 800 ms loop (`shell.rs:287-294`). Its gated state is "prompt queued, starting notice", which the silent stub never flushes.
   - `agents-start-failure` runs the stub `agy` with the fixture command.
   - Approval cases use their request files and `agent_status`, as today.
7. **Determinism.**
   - **Home:** the gate seeds its library with the shipped starter (what a new user gets, #1212) at fixed `modified` values (a test-only `seed_library_at(dir, modified)`; it never seeds the old showcase documents, rule 14) and a fixed `fixed_now`. Tier B cannot fix the clock, so the Home contact sheet masks the relative-time line and the version label rect.
   - **Every case:** at bless time the gate asserts that frame 8 equals frame 12, and the gate passes twice in a row with identical JSON. A case that fails either check is rewritten to a state-driven variant. It is never allow-listed.
8. **Comparison rules (hard failures; tolerance ±0.5 pt unless stated).**
   - **G1 Regions.** `x` and `w` are equal; `y` and `h` are equal unless an allow entry covers the region.
   - **G2 Inventory.**
     - A missing baseline name fails; a new name is reported.
     - The `unnamed` count per region or overlay is equal, unless an entry gives `inventory_max_delta`.
     - `scroll` entries are reported, never failed.
     - Virtualised regions compare the leading `min(n)` widgets.
   - **G3 Order.** Widgets are clustered into rows (centres within `max(4, 0.5·min(h₁,h₂))`) and sorted. The name sequence per row, with `_` for unnamed widgets, is equal.
   - **G4/G5 Axis rule (the same code for x and y).** For each widget on each axis:
     - **Size.** `d = size − size_base`. `|d| ≤ 0.5` passes. Otherwise the covering entry's `[min_d, max_d]` must contain it (`min_dw`/`max_dw` for x, `min_dh`/`max_dh` for y). A negative `d` additionally needs an R5 citation in `reason`.
     - **Anchor.** `anchor_x` is `left` (default), `right` or `centre`; `anchor_y` is `top` (default), `bottom` or `centre`. The anchored edge's observed shift must equal the **expected shift** ±0.5. The free edge moves by `d`, or by `±d/2` for `centre`.
     - **Expected shift.** It is the entry's exact `dx`/`dy` (default 0), plus a **derived cumulative shift**. For `anchor_y: top` that is the sum of the observed `dh` of the rows above in the same region (G3's rows). For `anchor_x: left` it is the sum of the observed `dw` of the widgets to the left in the same row. `bottom`/`right` sum from the other end. The sum is not a range.
     - A row's `dh` is the change in the height of the row's bounding box (the union of its widgets' rects).
     - The derived shift also adds the covering entry's `gap_dy` once for each row gap above the widget. It defaults to 0: B rule 9 keeps spacing unchanged, so `gap_dy` is only for a surface spec that changes the spacing between rows.
     - Each row's own `dh` must also lie in its covering entry's `[min_dh, max_dh]`. So a +16 header in a region allowed +8 per row fails on its own `dh`, and a widget that drifts more than the rows above it explain fails on its edge.
9. **Allow list (`allow.json`).**
   - Entry: `{"case","region","name_prefix"?,"h_base"?,"anchor_x"?,"anchor_y"?,"min_dw"?,"max_dw"?,"min_dh"?,"max_dh"?,"dx"?,"dy"?,"gap_dy"?,"inventory_max_delta"?,"spec","reason"}`.
   - `h_base` matches widgets by baseline height (±0.5). It is the only matcher that reaches unnamed widgets, for example every 24 pt field row.
   - **Precedence:** `name_prefix` > `h_base` > region-only. Two equally specific entries covering one widget make the file fail to parse.
   - `case` is one id. `"*"` is allowed only with `name_prefix` or `h_base`.
   - An entry without `spec` and `reason` fails to parse. An entry that matched nothing in a run is stale and fails, with its index.
   - `per_row` no longer exists; the derived cumulative shift replaces it.
   - Examples (spec D):
     - the command palette above the toolbar: `anchor_y: bottom`, `max_dh: 64`
     - the contextual bar: `anchor_y: bottom`, `dy: -6`; centred and widened 128 → about 140: `anchor_x: centre`, `max_dw: 12`
     - the re-centre pill: `dy: -4`
10. **The canvas must not change (G6, Tier B).**
    - `layout_baseline_capture.py --compare` crops each case's `canvas` region minus the rulers (`theme.rs:532`).
    - It runs `designos-diff compare --tolerance 0 --threshold 0` with every overlay and every in-canvas widget rect ignored.
    - A non-zero mismatch fails unless an entry with `region: "canvas"` allows it.
11. **Blessing (a procedure, not a chosen SHA).**
    - `bless_sha` is the `origin/main` commit that becomes the **parent of L's squash commit**. Procedure:
      1. `gh pr update-branch <L> --rebase`.
      2. `DESIGNOS_LAYOUT_BLESS=1 cargo test -p designos-app --bin designos layout_unchanged` on the rebased branch.
      3. Commit the JSON and `cases.json.bless_sha`.
      4. CI green.
      5. Merge at once. The PM lets nothing merge in between; if something does, rebase and bless again.

      Every UI change merged before L, such as #1108, is then in the baseline by construction.
    - **After L:**
      - Revamp PRs D–K never bless; they add allow entries.
      - The post-M PR re-blesses everything as the post-revamp guard.
      - A non-revamp UI PR that must change layout re-blesses **only the cases it changes**. It carries the `layout-change` label, and its reviewer reads the `*.layout.json` diff.
      - Branch protection ("up to date with main") makes every later PR carry that re-bless.
12. **Screenshot infrastructure failures (#1141).**
    - Tier A never takes a screenshot.
    - Tier B treats an exit of 1 with `screenshot was not captured` (with or without spec R's `: <reason>`) as INFRA: it retries twice, then exits 3.
    - **Exception:** a case that is INFRA on every try **and** is owned by the PR's `--only-spec` key exits 1, because the PR may have broken that capture.
    - A case whose `.layout.json` has an empty `named` map also fails (exit 1). That proves the release `RECORDING` path ran.
13. **The kit is never committed.** Tier B needs `--kit <path>`; the headless gate commits no kit-derived JSON.
14. **Document sources (owner ruling 2026-10-27; replaces the fixture-based case list).** Jang: drop the old built-in sample set entirely and verify with Untitled UI. Release builds since 0.6.5 (#1252) also reject `--fixture`: it exists only under `cfg(any(test, feature = "fixtures"))` (`cli.rs:238`, `:257`, `main.rs:46-168`), so the old list cannot run on a release binary anyway.
    - **Banned in both tiers:** `--fixture` of any name (`showcase`, `proto-vars`, `no-selection`, `variables`, `variables-library`, `comments`, `check-designs`, `board`, `board-empty`, `deck`), the `showcase`, `board::demo`, `decks::demo` and `demo` builders, and any document they produce. Other unit tests may keep them (#1212 R1/R3); this gate does not use them. A test asserts that `cases.json` has no `--fixture` argument.
    - **Tier B (every PNG anyone reviews): a copy of the Untitled UI PRO v8.0 kit.** `--kit` names the original `.fig`, which the script only reads. Once per run it imports the kit into `<run cache>/kit.designos`, outside the repo. Every case then gets its own APFS clone (`cp -c`), so no case sees another case's edits and nothing ever writes to the original. Kit layers keep their content and colours; nothing is recoloured or retouched (the `CLAUDE.md` marketing rule).
    - **Tier A (CI geometry, committed): the shipped starter** `crates/designos-app/assets/starter/designos-studio-starter.designos`. The kit cannot be committed: Jang's 2026-10-26 ruling in #1212 says Untitled UI is never redistributed in any form. At `origin/release/v0.6.5` the starter has 646 nodes: 3 screens (`01 Onboarding`, `02 Bio Age`, `03 Console - All studies`), an `About` page, 24 styles and 79 variables. It has no components, comment threads, prototype links or grid styles.
    - **States the document lacks are added on top of the copy, never taken from an old fixture.**
      - In Tier B the script adds them with the app's own headless commands (`--add-comment`, `--apply-spec`, `--import-variables`) or `designos-mcp`. They go on a page named `Layout baseline scratch` in the per-case clone, and are written to disk **before** the capture opens it. The capture therefore opens a clean, saved document; #1274 must still land first, because an imported file can open dirty.
      - In Tier A, `#[cfg(test)]` helpers in `layout_unchanged/` build the same states on an in-memory copy of the starter.
      - Each case lists its additions in `cases.json` (`"prep"`), so the run is reproducible.
      - Examples: a comment thread, a prototype link, a grid style, lint findings for Check designs, a published library, a component and an instance.
    - **Board and deck cases** start from Studio's own Blank board and Blank deck, which ship and are not legacy samples, plus content the script or helper adds.
    - **Selections** use `id:<n>` from `--inspect-json`, which is unique and deterministic, with the layer name recorded beside it. A name that matches more than one layer is never used as a selector.
    - **Case mapping is reviewed before any capture.** Every case keeps its id, its spec key and its `--panel`/`--menu`/`--tab` flags. Only the document, the selection and the `prep` change. The first step of the next round delivers one table: case → kit node → starter node → prep. The controller reviews that table before anything is captured or blessed.
    - **Unreachable state.** A case whose state cannot be reached in a tier by these means is listed as unreachable in that tier, with the reason, and the controller rules (drop it from that tier, or ask Jang). It never falls back to an old fixture.
    - **Cost on the owner's Mac.**
      - Before the case list is fixed, the round measures the load time and peak memory of the `.designos` copy against the `.fig`. Kit runs took about 50 s each in spec D; spec 1121 recorded 150 s and a 9.87 GB peak for an MCP import.
      - If a case needs more than 20 s to reach its first frame, the round proposes a reduced copy (kit pages only, unchanged) and the controller rules.
      - Tier B runs one GUI case at a time, checks `ioreg -c IOHIDSystem` HIDIdleTime first, and keeps the Mac awake with `caffeinate -dimsu`.

## 4. Data model / schema changes

None to documents.
- New in-memory types: `ShellRegions` (ten `Option<Rect>`s), `LayoutSnapshot` and `WidgetGeom`.
- New `app.ui.screenshot_chrome`.
- New module `cli_resolve.rs`: `SearchSources`, `cli_search_dirs_with`, `resolve_in`, the wrapper `cli_search_dirs(explicit)`, `hermetic_agent_cli_dir()`, and the `cfg(test)` guard `ScopedCliDirs`.
- New functions `hermetic_startup_check(env, &Options) -> Result<(), String>` and `write_hermetic_ack(tmpdir, dir, pid)`.
- Test-only `Options` fields: `agent_cli_dir` and `fixed_now`.
- No new release `Options` fields.

## 5. Contracts

**CLI.** `--layout-json OUT.json` is valid only with `--screenshot` (error: `--layout-json needs --screenshot`). `USAGE` gains `[--layout-json OUT.json]`. There is no `--agent-cli-dir` flag.

**Environment** (release; read only where needed):

| Variable | Meaning |
|---|---|
| `DESIGNOS_AGENT_CLI_DIR` | Turns on hermetic agent mode (rule 5). Deny by default: needs `--screenshot` plus markers in it, in `DESIGNOS_LIBRARY` and in `TMPDIR`, or the binary exits 2. |
| `DESIGNOS_LIBRARY` | Existing: library root (`store.rs:456-462`). Required, with a marker, in hermetic mode. |
| `TMPDIR` | Existing: temp root. Required, with a marker, in hermetic mode. |
| `DESIGNOS_STUB_LOG` | Read by the stubs, not by the app: the file each stub invocation is appended to. |

**Snapshot file** (`schema` 1; keys sorted; rects `[x, y, w, h]`, rounded to 0.25):

```json
{"schema":1,"case":"d-editor-default","size":[1440,900],"scale":1.0,
 "regions":{"title_bar":null,"tab_bar":[0,0,1440,40],"rail":[0,40,49,860],"left_panel":[49,40,241,860],"canvas":[290,40,909,860],"notes":null,"right_panel":[1199,40,241,860],"home":null,"present":null,"variables_full":null},
 "overlays":[{"name":"overlay:Foreground/0","rect":[8,40,218,384]}],
 "named":{"left-panel-resize-handle":{"region":"left_panel","rect":[287,40,6,860],"count":1}},
 "unnamed":[{"region":"rail","rect":[4,52,40,40],"sense":["click"]}],
 "scroll":[{"region":"right_panel","axis":"y"}]}
```

The numbers are illustrative; the real ones come from the bless run.

**Case list** (`cases.json`): entries `{"id","spec","args",["virtualized"],["spawns"]}` plus, per tier, the document, the `id:<n>` selection and `prep` (rule 14), and `"bless_sha"`. `args` keeps only the surface flags (`--panel`, `--menu`, `--tab`, `--dev`, `--present`, `--find`, `--rulers`, `--zoom`, `--gpu`); it never holds `--fixture`. **76 cases**, each in both tiers unless rule 14 lists it as unreachable in one of them. The groups below are the surfaces; the documents they used to name are replaced by rule 14:

| Group | Cases |
|---|---|
| Dry-run set | 42 distinct cases (section 10); `i-agents-chat` and `i-agents-start-failure` become state-driven (rule 6) |
| D | `d-window-chrome` (rule 1) |
| F / K | `edit-style-text`, `edit-style-effect`, `edit-style-grid`, `grid-styles`, `effect-noise`, `effect-texture`, `effect-glass`, `picker-page`, `fill-d9` |
| I | `agents-skills`, `agents-approval-queued`, `agents-approval-open-tab` |
| J | `check-designs-fixed`, `plugin-install`, `ai-image-menu`, `ai-image-expand`, `ai-image-erase`, `web-capture-report` |
| K / D | `present-variables` |
| Mixed | 12 `micro-*` panels: `micro-autolayout`, `micro-components-variables`, `micro-css-grid`, `micro-dev-boxmodel`, `micro-inspector-math`, `micro-layer-picker`, `micro-scale-tool`, `micro-selection-corner`, `micro-shape-controls`, `micro-smart-selection`, `micro-typography-multiedit`, `micro-vector-edit` |
| Kit views | `kit-editor` (`--zoom 100 --gpu`) and `kit-agents` (`--zoom 100 --panel agents --gpu`), each on a uniquely named kit frame chosen in the mapping table. They stay in the list, and their Tier A twins use the starter. |

`micro-seq-hud` is not a case. It is a painter layer id (`shell.rs:3460`), not a `--panel` value, and `cli::parse` rejects it (`cli.rs:254-258`).

**Not gated** (the owning spec carries the visual check): toasts (J; every capture uses `--no-toast`), modals and context menus without a `--panel` value (J). The title bar has no PNG; its geometry is gated by `d-window-chrome`.

**Baseline capture commands** (on the bless branch, rule 11):

```bash
export PATH="/Users/jangtrinh/.local/bin:$HOME/.cargo/bin:$PATH"
cargo build --release -p designos-app -p designos-diff
# Tier A: geometry baseline (committed)
DESIGNOS_LAYOUT_BLESS=1 cargo test -p designos-app --bin designos layout_unchanged
cargo test -p designos-app --bin designos layout_unchanged && cargo test -p designos-app --bin designos layout_unchanged
# Tier B: PNGs and --layout-json (not committed; zipped on the L PR)
python3 scripts/layout_baseline_capture.py --bin <build dir>/release/designos \
  --cases crates/designos-app/tests/layout-baseline/cases.json \
  --stubs crates/designos-app/tests/layout-baseline/stubs \
  --out ~/designos-layout-baseline/<bless sha> \
  --kit "$HOME/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig"   # read only; every case runs on its own clone (rule 14)
```

Per case, the script:
- creates a fresh run dir with `home/`, `lib/`, `tmp/` and a copy of the stubs, each holding a `.designos-hermetic` marker
- seeds `lib/` with `designos --seed-library` and replaces `{LIB}` in `args`
- runs `env -i HOME=<run>/home PATH=<stubs>:/usr/bin:/bin TMPDIR=<run>/tmp DESIGNOS_LIBRARY=<run>/lib DESIGNOS_AGENT_CLI_DIR=<run>/stubs DESIGNOS_STUB_LOG=<out>/invocations.log designos <args> --size 1440x900 --no-toast --screenshot <out>/<id>.png --layout-json <out>/<id>.layout.json`

Every case runs in hermetic mode, not only the agent cases. After each case the script checks, in this order:
1. `<run>/tmp/.designos-hermetic-active` exists and names that run's pid and stub dir. Otherwise the case fails, before the log is trusted.
2. A case marked `"spawns": true` in `cases.json` added at least one stub line to `invocations.log`. An empty log never passes. `spawns` is set on every case whose state starts an agent tab: at least `i-agents-chat`, `i-agents-start-failure` and `kit-agents`, plus any case where the bless run's log shows a stub line.
3. Every `$0` in `invocations.log` starts with the run's stub dir.

The log is attached to the PR. Exit codes: 0 pass, 1 layout, G6 or owned-INFRA failure, 3 INFRA only (rule 12). Tier B also draws a **wireframe sheet** (PIL, every rect, coloured by region) from any two `*.layout.json`, including Tier A's `d-window-chrome`.

**PR gate** (D–K, and A/B/C if they touch rendered UI):
1. CI `cargo test --workspace --locked` (required `Test (macos-14)`) runs Tier A.
2. Local: `layout_baseline_capture.py --compare ~/designos-layout-baseline/<bless sha> --only-spec <key> …`. The PR attaches the colour sheets, the wireframe sheets and `invocations.log`, and lists its allow entries with the R5 size each cites.

## 6. File-format mapping

None. `.designos` and `.fig` are untouched.

## 7. UI

None visible. No chrome changes, so `es-designer` is not needed. No colour is involved, so the token-contract C-1 pairs (`specs/token-contract.md` section (f)) do not apply.

## 8. Edge cases

- **Popover on the frame after build:** 8 frames, checked by frame 8 = frame 12 at bless time.
- **A widget straddles regions** (the resize handle, `shell.rs:1559-1562`): its centre decides; ties go left.
- **Clipped scroll areas:** use `rect`. Overflow adds `scroll` entries only.
- **R5 rows push others down:** the derived cumulative shift accounts for it exactly. Content below the fold stays in `named`/`unnamed`.
- **Hug-width widgets:** `dw` needs an entry. Later widgets in the row get the derived x shift.
- **A new command form** (custom CLI, compound command) in hermetic mode: refused (rule 5), and the case fails visibly rather than resolving anything on the host.
- **A stub missing from `DIR`:** spawn refused. `is_installed` is false through `cli_search_dirs()`, and the picker shows the not-installed row in both tiers alike.
- **#1141 INFRA:** retried, exit 3; owned cases exit 1 (rule 12).
- **Missing baseline file:** fails with the bless hint.

## 9. Non-goals

- Pixel equality of chrome; colour and token checks (A, B).
- Screenshots in CI; an egui rasteriser in tests.
- Transcript fixtures, a PTY-less tab type, an `--agent-cli-dir` flag.
- Fixing #1141 (spec R).
- Changing what existing `--panel` values show, beyond removing the two wall-clock waits.

## 10. Test plan (red first)

**Hermetic mode (own red-first tests). Each is a pure function, a function given explicit inputs, or a thread-local guard. None sets the process-wide `PATH` or the `OnceLock` (R8).**

| # | Case | Expected | Fails before |
|---|---|---|---|
| H1 | `hermetic_shell_cmd_has_no_login_or_interactive_flags` | `build_agent_shell_cmd`, given the hermetic dir as an explicit argument (the wrapper passes `hermetic_agent_cli_dir()`), gives argv `["/bin/sh","-c","exec <DIR>/claude …"]` with no `-l` or `-i`, for every preset and for the Shell preset | yes |
| H2 | `search_dirs_precedence_is_hermetic_scoped_explicit_host` | Pure `cli_search_dirs_with` over built `SearchSources` (no env, no `OnceLock`):
- hermetic set → `[DIR]`, even with scoped, explicit and a host `PATH` holding `claude`
- scoped only → `[stub_dir]`
- explicit only → only it
- nothing → host `PATH` + extra | yes (no function) |
| H3 | `hermetic_pty_env_is_stub_path_only` | `configure_pty_env_with_session`, given the hermetic dir explicitly, sets `PATH=DIR:/usr/bin:/bin` and removes `BASH_ENV` and `ENV` | yes |
| H4 | `hermetic_mode_is_denied_without_every_requirement` | `hermetic_startup_check` returns `Err` naming the missing item for each of: no `--screenshot`; `DESIGNOS_LIBRARY` unset; `TMPDIR` without a marker (set, as on macOS); `HOME` without a marker; `DIR` without a marker. `Ok` only with all of them. The environment is passed in as a map, never read from the process | yes |
| H5 | `hermetic_mode_refuses_compound_or_unknown_commands` | `"claude; rm -rf x"`, `"claude \|\| codex"`, `"$(x)"` and `"foo"` (not in `DIR`) are refused, with no fallback lookup | yes |
| H6 | `every_resolver_uses_resolve_in` | Pure `resolve_in(&[DIR], bin)` returns `DIR/bin` or `None`. `resolve_cli_executable` and `resolve_grok_binary`, called with `SearchSources{hermetic: DIR, host_path: <dir with fake claude and grok>}` passed explicitly, return `DIR` paths or `None`, never the host path. | yes: both read the host `PATH` today |
| H7 | `scoped_guard_applies_only_inside_a_gate_case` | Thread-local, deterministic: inside `ScopedCliDirs::enter(stub)`, `cli_search_dirs(None)` is `[stub]` and `AgentCliPreset::Claude.is_installed()` is true. After the guard drops it is the host list again. Another thread is unaffected. | yes (no guard) |
| H9 | `hermetic_mode_writes_the_acknowledgement` | After `hermetic_startup_check` passes, `write_hermetic_ack(tmp, DIR, pid)` writes `tmp/.designos-hermetic-active` with that pid and `DIR`. A denied check writes nothing. | yes |
| H10 | `hermetic_resolver_census` | Scans `crates/designos-app/src/**/*.rs`: `split_paths(` and `var_os("PATH")` appear only in `cli_resolve.rs` and `configure_pty_env_with_session` | yes: 9 readers today |
| H11 | `version_warmer_resolves_on_the_calling_thread` | Inside a scoped guard, `warm_cli_version_cache(Claude)` resolves to the stub path, as a test hook records before `thread::spawn` | yes |
| H8 | `stub_scripts_behave` | Each checked-in stub: `--version` exit 0 with a fixed string; `agy --mcp-config x` exit 1; anything else logs `$0 $*` to `DESIGNOS_STUB_LOG` and runs `cat` | yes (no stubs) |

**Gate and comparer:**

| # | Case | Expected | Fails before |
|---|---|---|---|
| 1 | `snapshot_reports_every_region_of_the_editor` | `d-editor-default` regions; rail `RAIL_WIDTH + BORDER_WIDTH`; `named` holds `left-panel-resize-handle` | yes |
| 1b | `record_hook_lists_every_name_of_the_pass` | two calls are listed; none under `RecordDisabled`; the next pass starts empty | yes |
| 2 | `layout_gate_is_deterministic` | twice in one process, and `cargo test` twice: identical | yes |
| 3 | `widening_the_left_panel_fails_g1` | +8 fails, naming `left_panel.w` | yes |
| 4 | `growing_a_row_without_allow_fails_and_with_allow_passes` | `dh +8` fails; `max_dh 8` passes | yes |
| 4b | `palette_growing_above_bottom_anchor_passes_with_allow` | bottom edge fixed, `dh +64`, `anchor_y bottom` passes; without the entry fails | yes |
| 4c | `cumulative_shift_is_derived_not_a_range` | rows 24→32 (+8 each, `h_base 24`, `[8,8]`): rows below shift by the derived sum and pass. A header +16 fails on its own `dh`. A widget 8 pt lower than the sum explains fails on its edge. | yes |
| 4d | `inspector_overflow_does_not_fail_g2` | 24→32 makes the right panel overflow: a `scroll` entry appears; G2 passes | yes |
| 4e | `centred_bar_widening_by_12_passes_with_anchor_x_centre` | each edge moves ±6; with the entry it passes; without it, G4 fails | yes |
| 5 | `reordering_two_buttons_fails_g3` | swap fails | yes |
| 6 | `stale_allow_entry_fails` | fails with its index | yes |
| 6b | `region_wide_wildcard_entry_is_rejected` | `"*"` without `name_prefix`/`h_base` fails to parse | yes |
| 6c | `equally_specific_entries_fail_to_parse` | two `h_base 24` entries on one region fail; `name_prefix` beats `h_base` | yes |
| 7 | `missing_baseline_fails` | fails with the bless hint | yes |
| 8 | `layout_json_requires_screenshot` | `cli::parse` error | yes |
| 8b | `layout_json_matches_headless_snapshot` | a `Sim` with `apply_launch_defaults(true)` and a `shot`; a synthetic `Event::Screenshot` (1×1) injected after frame 8; the written JSON equals `capture` in `regions` and `named` | yes |
| 8c | `agent_cases_do_not_depend_on_installed_clis` | Each agent case, `d-window-chrome` and `k-libraries` is snapshotted with the host `PATH` empty and with fake `claude`/`codex` on it. The JSON is identical. The stub log names only the stub dir. `app.libs.root` and every opened `Library` root lie under the case temp dir, never under `dirs::data_dir()`. | yes |
| 8d | `home_is_deterministic_with_fixed_clock` | `h-home` with `fixed_now = T` and with `fixed_now = T + 60 s` on a seed with fixed `modified`: the geometry is equal and the label text is the expected relative string; no wall-time wait | yes |
| 8e | `shell_new_uses_launch_defaults_for_its_own_mode` | `Options{screenshot: Some(..)}` → `screenshot_chrome` true, rail not opened; `Options::default()` → rail default applied | yes |
| 9 | `layout_unchanged_<spec>` (the gate) | green on the bless branch after bless, twice | yes |
| 10 | Local: `layout_baseline_capture.py --compare` against itself | G6 0 mismatches; `invocations.log` paths all under the stub dir | yes |
| 11 | Script unit tests | INFRA retry then exit 3. Owned INFRA exits 1. An empty `named` map fails. The hermetic env is built with `env -i` and four markers. A missing or foreign `.designos-hermetic-active` fails the case. A `spawns` case with an empty log fails. | yes |

Run:
- `cargo test -p designos-app --bin designos hermetic_` and the H-named tests (H1–H11)
- `cargo test -p designos-app --bin designos layout_` (twice)
- `cargo test --workspace --locked`
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `python3 scripts/zero_fake_ui_gate.py`
- `python3 -m unittest discover -s scripts/tests -v`

**Dry run (case 0), 2026-10-25, before #1106.** It proves the command list runs; it is **not** a baseline (rule 11).
- Binary built from `ec47da26`; 43/43 captured, 13.6 MB, 0.5–2.8 s each.
- It ran with the host `PATH`. Whether any agent case started a real CLI is UNKNOWN; rule 5 forbids that from now on.
- Two pairs were byte-identical: `d-editor-selected` = `f-design-tab`, and `f-picker` = `f-picker-gradient`. The worker drops `f-design-tab` and repoints `f-picker` at a solid fill, which leaves 42 distinct cases.

## 11. Performance budgets

- Tier A: 76 cases × 8 frames (rule 14), split into one `#[test]` per spec key so they run in parallel. Agent cases no longer wait 800 ms or 5 s. Budget: the app binary's CI test time grows by at most 30 s (INFERENCE; today it is 63–64 s; measure in the PR).
- Hermetic checks: one `OnceLock` read per resolution, with no cost when the variable is unset.
- `ShellRegions`: a copy of ten `Option<Rect>`s per frame. Zero idle cost.
- Committed baseline: one JSON file per Tier A case (76 at most), estimated under 3 MB (INFERENCE).

## 12. Dependencies and build order

Lands first. Order:
1. `cli_resolve.rs` and the five resolvers routed through it, the hermetic predicate, the spawn arm, the PTY env, the startup check with its acknowledgement, `warm_cli_version_cache`. Tests H1–H7, H9–H11.
2. The checked-in stubs (H8); `stub_dir()` in test support.
3. `ShellRegions`, `apply_launch_defaults` (8e), listable `record` (1b), `vector.rs` fix.
4. `layout_snapshot.rs` and the comparer (1, 3–7, 4b–4e, 6b, 6c).
5. `Sim` seams on every case; state-driven agent cases; `fixed_now` and the seed (8c, 8d).
6. The per-spec gate tests with `cases.json`.
7. `--layout-json` (8, 8b).
8. The script: hermetic env, invocation log, wireframes, G6, INFRA rules (10, 11).
9. Bless procedure (rule 11); determinism (2); the Tier B PNG run zipped on the PR.
10. PR (squash, merged right after the bless).

No revamp PR merges before this one.

## Acceptance

1. Tests H1–H11 were seen failing before their fix and pass after it (pasted in the PR).
2. Tests 1–11 are green. Each `layout_unchanged_<spec>` is green twice in a row on the bless branch on `macos-14` CI, and on a developer Mac with all six real CLIs installed, with identical JSON.
3. Tests 4b, 4c, 4d, 4e, 6c, 8b, 8c, 8d and 8e were seen failing before their fix.
4. The Tier B run attaches `invocations.log`:
   - every case has its `.designos-hermetic-active` acknowledgement
   - every `spawns` case logged at least one stub line
   - every `$0` starts with the run's stub dir

   With `DESIGNOS_AGENT_CLI_DIR` set and any of the four markers or `--screenshot` missing, the binary exits 2 and writes nothing (H4, H9, and one manual run pasted).
5. Every case in section 5 is in `cases.json` (76 cases), or is listed under "Not gated" with its owning spec. No case uses `--fixture` or an old sample document (rule 14). Every Tier B PNG shows a kit copy, and every Tier A snapshot comes from the starter. The controller has reviewed the mapping table, and every case unreachable in a tier carries the controller's ruling.
6. `cases.json.bless_sha` is the parent of L's squash commit.
7. `cargo test --workspace --locked` is green with the same pass count as `main`, plus exactly the tests this PR adds. No existing test's expected values change (changelog R2).

## 13. Owner rulings that apply

- R1 (2026-10-25): keep the layout and arrangement.
- R5: editor rows text-xs 12/18, titles text-sm 14/20, button xs 32, row 32, radius md 8; Home and dialogs sm/md. This is the only allowed source of size change.
- R7: spec-driven; the PM coordinates delivery (and enforces the merge-at-once bless, rule 11).
- 2026-09-22: visual evidence in every PR (colour sheets plus wireframe sheets).
- Standing: `main` is protected (PR, green `Test (macos-14)`, Clippy, Rustfmt, squash).
- 2026-10-26 (#1212): Untitled UI is never redistributed in any form, so the kit is never committed.
- 2026-10-27: drop the old built-in sample set entirely and verify with Untitled UI (rule 14).

## Decisions on the open questions (writer, 2026-10-25)

1. **Geometry source:** widget rects, not edge detection.
2. **CI or local:** geometry in CI (Tier A), pixels local (Tier B).
3. **Baselines:** JSON committed; PNGs zipped on the PR.
4. **Matching:** by `record` name; unnamed widgets by row, column and `h_base`.
5. **Who blesses:** L (by procedure), the post-M PR, and non-revamp PRs for their own cases under `layout-change`. D–K never.
6. **Agent cases:** in both tiers, hermetic by one predicate. Release capture is deny-by-default.
7. **#1141:** INFRA with its own exit code; owned cases exit 1.
8. **Title bar:** geometry through `d-window-chrome`; no PNG.
9. **Shrinking:** only through `min_dh`/`min_dw` with an R5 citation.
10. **`layout-change` for PRs outside the revamp:** a process call that the round-2 review left to the PM. This spec adopts counsel's procedure as the default (rule 11). The PM can veto it in review.

## Open questions for the owner

None. All the calls here are technical. The visible outcome, "layout unchanged", was decided by R1 and R5.

## Changelog

**Round 4 (2026-10-27): document sources.** Jang saw the first Tier B colour sheet. It was built on the old `showcase` fixture ("Balance card"), with 74 of its 76 cases on old fixtures. He restated the rule: drop the old set and verify with Untitled. This writer's round 3 broke it. Rule 14 replaces every old fixture: Tier B uses a kit copy, and Tier A uses the shipped starter. Missing states are added on top, and a reviewed mapping table comes before any capture. Rules 7 and 13, the case list, the capture command, acceptance 5 and section 13 are updated. The 2026-10-27 capture (`out/layout-tierb/993afe38`, built on fixtures) is discarded and not blessed.

**Round 3 (2026-10-25)** answers `reviews/L-engineering-review-r2.md` and applies `reviews/L-counsel-kongming.md`.

| Finding | Change |
|---|---|
| N1 (blocker); round-1 B3 (partly fixed) | Rules 5–6. Hermetic mode at one resolver, `cli_search_dirs()`. Release mode removes `-l`/`-i`, uses `/bin/sh`, rewrites the binary to an absolute path, and sets `PATH=DIR:/usr/bin:/bin`. It is deny-by-default at startup, with markers. The same six stubs serve both tiers. `warm_cli_version_cache` resolves inside the search dirs. Tests H1–H8 and acceptance 1 and 4. The `--agent-cli-dir` flag and the dead `terminal.rs:399` citation are dropped. |
| N2 | G4 and G5 are one axis rule, with `anchor_x`, an exact `dx`, `[min_dw, max_dw]`, and a derived x shift (rule 8). Test 4e. |
| N3; round-1 M6 (partly fixed) | Exact `dx`/`dy`, a derived cumulative shift, a per-row `[min, max]`, an `h_base` matcher for unnamed widgets, and precedence `name_prefix` > `h_base` > region; `per_row` removed (rules 8–9). Tests 4c and 6c. |
| N4; round-1 M5 (partly fixed) | Rule 11: `bless_sha` is the parent of L's squash commit (rebase, bless, merge at once). Non-revamp PRs re-bless their own cases under `layout-change`; D–K never bless. |
| N5 | Rule 1: `Shell::new` calls `apply_launch_defaults(opts.screenshot.is_some())`; `wgpu` stays in `new`. `d-window-chrome` gets the seams like every case. Test 8e. |
| N6 | Rule 12: an empty `named` map in `--layout-json` fails the case. |
| N7; round-1 M9 (partly fixed) | Rule 7: seed fixed `modified` (the library index field, `store.rs:653`), not mtimes; `fixed_now`. Test 8d uses two clock values, not wall time. |
| N7b | Rule 12: INFRA on every try in a case the PR owns exits 1. |
| N8 | Rule 4: one `#[test]` per spec key; 74 cases; budget restated (section 11). |
| N9 | `micro-seq-hud` dropped (74 cases). Citations fixed: the resize handle `record` is at `shell.rs:1561`; `use_stub_agent_clis` starts at `:212`; the agent branches are `:277-321` and `terminal` is `:327-329`. |
| Round-2 design notes | The gate asserts `cfg!(debug_assertions)`; the debug flag is set after `theme::apply`; a drag area is defined as a `DRAG`-only widget whose rect contains a ScrollBar rect (rule 3). |
| Re-anchor | `205c72ef` → `7e15ddae` (includes #1108); drift is stated in the header. |

**Counsel deviations, with evidence:**
1. **"`TMPDIR` set" is not a hermetic check on macOS.** `TMPDIR` is set for every login session; this session's is `/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/`. Rule 5 requires a `.designos-hermetic` marker in `TMPDIR`, `DESIGNOS_LIBRARY` and `DIR`, and the script uses `env -i`.
2. **Tier A was not host-independent as counsel wrote it.** `AgentCliPreset::is_installed` calls `runtime_binary_available`, which reads the host `PATH` (`terminal.rs:294-299`, `:335-339`), and the composer spawns only when it returns true (`agent_composer.rs:746-748`). A `path_override` on the session does not reach it. Round 3 first made `cli_search_dirs()` return the stub dir for every `cfg(test)` build. The round-3 review (R2) showed that this is too wide. It is now a thread-local guard that only the gate's `Sim` enters (rule 5, precedence 2). Test H7.
3. **Rewriting only the first word could still reach the host through a compound command.** `build_agent_shell_cmd` passes compound commands through unchanged (`terminal.rs:4387`), and custom CLIs are user-defined. Hermetic mode refuses any compound command, and any first word not found in `DIR` (rule 5, H5).
4. **Minor citations:** counsel's `terminal.rs:626` for the warmer call is `:624`, and its 2 s poll pattern (`:2941-2949`) is the `is_alive` loop at `:2949-2951`.

**Round 3 fixes (2026-10-25)** apply `reviews/L-engineering-review-r3.md` (APPROVE_WITH_FIXES):

| Finding | Change |
|---|---|
| R1 (major) | `resolve_grok_binary` (`agent_usage.rs:601-624`) is the fifth resolver routed through `cli_search_dirs()` (rule 5). Census test H10; H6 covers `resolve_grok_binary`. |
| R2 (major) | The blanket `cfg(test)` stub dir is gone. The stub dir is now precedence 2, a thread-local guard that only the gate's `Sim` enters. An explicit list (`path_override`) keeps resolving only in itself, and existing tests resolve as before (rule 5). Acceptance 7 adds the full-suite pass-count check. |
| R3 (major) | The Tier A seams set `app.libs.root` (and a seeded `opts.library` where needed) for every case (rule 6). 8c asserts that no library root lies under `dirs::data_dir()`. |
| R4 (major) | The binary writes `$TMPDIR/.designos-hermetic-active` after the startup check (H9). The script fails a case without it before trusting the log, and a `spawns` case with an empty log fails (section 5, test 11, acceptance 4). |
| R5 (minor) | A `HOME` marker is required, and the check runs before `spawn_startup_sweep()` (`main.rs:149`). |
| R6 (minor) | A row's `dh` is defined as the change in its bounding-box height. `gap_dy` was added to the entry schema (rules 8–9). |
| R7 (minor) | Rule 1 says the `pages_height` load (`shell.rs:102-105`) stays in `Shell::new`, and states how each tier is affected. |
| R8 (minor) | H2, H6 and H7 now test pure functions (`cli_search_dirs_with`, `resolve_in`) and a thread-local guard, with no process-wide `PATH` and no `OnceLock`. H9–H11 added. |

**Round 2 (2026-10-25)** answered `reviews/L-engineering-review.md` (round-1 B1, B2, M4, M7, M8 fixed; B3, M5, M6, M9 partly fixed, completed above).
