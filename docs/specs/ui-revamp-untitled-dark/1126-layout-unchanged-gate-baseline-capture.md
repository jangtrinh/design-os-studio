# L Layout-unchanged gate: baseline geometry of every surface, and the check each revamp PR runs

| Field | Value |
|---|---|
| Issue | #1126 (epic #1118, spec key L) |
| Size | M. Two to three days: a snapshot module, one CLI flag, a headless test with a case list, a comparer, the baseline commit, a local capture script. |
| Depends on | none. **Lands first**, before any revamp PR (A–K) merges. |
| Unblocks | D, E, F, G, H, I, J, K (each runs the gate), M (reuses the capture list), and the re-bless at the end of the revamp |
| Touches | New `crates/designos-app/src/layout_snapshot.rs`<br>`crates/designos-app/src/widgets/mod.rs:243-250` (the `record` hook can be listed)<br>`crates/designos-app/src/shell.rs`:<br>- region rects `:1383-1398`, recorded into the app state<br>- screenshot write `:445-460`, to also write `--layout-json`<br>`crates/designos-app/src/cli.rs` (`Options` `:5-96`, `parse` `:208-322`, `USAGE` `:326-338`)<br>`crates/designos-app/src/main.rs` (module list)<br>New test `crates/designos-app/src/shell/tests/layout_unchanged.rs` (registered next to `:1017-1048` of `shell/tests.rs`)<br>`Sim` in `crates/designos-app/src/shell/tests.rs:41-104` (configurable screen size)<br>New data dir `crates/designos-app/tests/layout-baseline/` (`cases.json`, `allow.json`, `<case>.layout.json`)<br>New `scripts/layout_baseline_capture.py` |

Code baseline: `origin/main@ec47da26`. Re-anchor every cited line against HEAD and report drift before writing code. Scratch evidence for this spec, outside the repo: `$SCRATCH/agents/w5/l/`. `$SCRATCH` is the controller's scratchpad, `/private/tmp/claude-501/-Users-jangtrinh--claude-profiles-work-spares-spare-AW68Nz/a6445c24-c40e-4254-a3e2-5e0bafafd0f4/scratchpad`.

## 1. Summary and user story

As the PM merging the revamp PRs (D–K), I need proof that a PR changed colours, type and control sizes but not the layout. "Layout unchanged" means three things (plan.md, "Shared naming contract"):
- the same regions
- the same panel widths
- the same control order

Heights may grow only where R5 raises a control or type size.

Pixel equality cannot prove this, because every colour changes by design. This spec records the geometry of every surface before the revamp starts, and adds a deterministic geometry check that runs in `cargo test` on every PR. It also records screenshots of the same surfaces for side-by-side review.

## 2. Current state (FACT unless labelled)

**What exists and why none of it is enough on its own:**

| Tool | What it does | Why it is not the gate |
|---|---|---|
| `scripts/measure_ui.py` (94 lines) | Crops fixed rectangles out of `--screenshot` PNGs and pixel-diffs them against Figma reference captures with `designos-diff compare` (`:24-31`, `:79`) | Pixel parity with Figma. Hard-codes `target/debug/designos` (`:20-21`) and needs gitignored reference PNGs. |
| `scripts/capture-visual-proof.sh` (91 lines) | 16 `--screenshot` captures into `docs/images/` from the `showcase` fixture | Overwrites docs images, uses `/tmp`, no comparison |
| `ui vr diff/gate/accept` (external `ui` CLI, ease-design) | Pixelmatch diff of PNG pairs; `gate` diffs a baseline dir against a current dir | Pixel-based. A missing baseline is "not a failure". Not in this repo or in CI. |
| `designos-diff compare/suite` (`crates/designos-diff/src/main.rs:16-48`, `suite.rs:11-31`) | Pixel diff with tolerance, threshold and ignore rects; manifest suites with crop regions | Pixel-based, but in the workspace. It is the right tool for the one region whose pixels must not change: the canvas (rule 7). |

**There is no layout dump today.** Nothing in `crates/designos-app/src` writes widget geometry to a file.

**The pieces a dump can be built from:**
- **Named widget rects (the strongest source).**
  - `widgets::record(ui, name, rect)` is a test hook that "remembers where a named widget was drawn so UI tests can find it" (`widgets/mod.rs:243-247`). Under `cfg(test)` it stores the rect in egui temp data under `("test-rect", name)`. In other builds it is an inlined no-op (`:248-250`).
  - It has **444 call sites in about 65 files**, covering every surface the revamp touches: `panels/left.rs`, `right.rs`, `inspect.rs`, `properties.rs`, `typography.rs`, `toolbar.rs`, `rail.rs`, `tabs.rs`, `home/*`, `terminal.rs`, `agent_*`, `cmdk.rs`, `find/panel.rs`, `comments/*`, `variables/*`, `libraries/*`, `history/*`, `dev/*`, `specs.rs`, `present.rs` and others.
  - The names are semantic and written by developers, for example `left-panel-resize-handle` (`shell.rs:1492`), `agents-composer-send` and `agent-turn-row-{idx}`.
  - Tests read them with `Sim::rect_of(name)` (`shell/tests.rs:159-160`).
  - The temp map cannot be listed, so today a test can only look up a name it already knows.
- **Shell regions.** `Shell::draw` computes every region rect each frame (`shell.rs:1383-1398`):
  - `rail_rect`, `left_rect`, `right_rect`, `canvas_rect`, `notes_rect`
  - inside, `main_canvas_rect` and `dock_rect` (`:1424-1429`)
  - the tab bar, an `egui::Panel::top("project_tabs_bar")` of `TOP_TAB_BAR_HEIGHT` (`tabs.rs:896`)
  - the widths come from `theme::RAIL_WIDTH` 56 (`theme.rs:263`), `LEFT_PANEL_WIDTH` 240 (`:279`) and `RIGHT_PANEL_WIDTH` 240 (`:318`)
- **Widget rects.**
  - egui is **0.35** (`crates/designos-app/Cargo.toml:40`, `:51`; `CLAUDE.md` says 0.31, which is stale).
  - In 0.35, `Context::viewport(|vp| …)` is public and `ViewportState.prev_pass.widgets: WidgetRects` is public. After a pass ends, `prev_pass` holds that pass (`egui-0.35.0/src/context.rs:2644`, `:3861`).
  - `WidgetRects::layers()` yields every widget per layer in paint order: `id`, `layer_id`, `rect`, `interact_rect`, `sense`, `enabled` (`widget_rect.rs:9-50`, `:120-129`).
  - `Context::layer_transform_to_global` maps them to screen space (`context.rs:2976`).
  - `WidgetInfo` labels are only stored in debug builds with a debug flag (`context.rs:1503-1509`), so they cannot be relied on.
  - AccessKit nodes (`pass_state.rs:71-74`, `:226`) carry role and label, but only for widgets that report them. This spec does not depend on them.
- **The headless test harness.**
  - `Sim` builds the whole shell from CLI `Options` (`Shell::build(&ctx, opts)`, `shell/tests.rs:56-74`, `shell.rs:120`) and runs frames with `ctx.run_ui` at a fixed 1600x1088 (`tests.rs:79-104`). No window and no GPU, so it runs in CI.
  - CI runs `cargo test --workspace --locked` on `macos-14`, a required check (`.github/workflows/ci.yml`, step "Test workspace").
  - Both binaries share `src/main.rs`, so shell tests run once per binary (`crates/designos-app/Cargo.toml:9-15`).
- **Screenshot path.**
  - `drive_screenshot` saves the PNG when `Event::Screenshot` arrives (`shell.rs:403-480`).
  - `main.rs:153-156` exits 1 when no PNG was written.
  - Rulers are on in every non-test run (`shell.rs:202`) and off in `Sim` unless `--rulers` is passed.

**UNKNOWNs:**
- How many named rects each case records. Some interactive widgets have no `record` call. Closing check: the bless run prints `named/unnamed` per case. Unnamed widgets are still counted and ordered (rule 4, G2 and G3).
- Whether every case is deterministic across two runs: terminal prompt text, Home "edited … ago" text, agent CLIs on PATH. Closing check: the gate runs twice on the same commit with 0 differences (test plan, case 2).

## 3. Behaviour rules

1. **One snapshot function, two callers.** `layout_snapshot::capture(ctx: &egui::Context, regions: &ShellRegions) -> LayoutSnapshot` reads the named rects of the last pass and `prev_pass.widgets`. It is called by:
   - the headless gate test, after the case's frames have run
   - the real app, on the frame that writes a `--screenshot` PNG, when `--layout-json OUT.json` is given (`shell.rs:452-458`)

   `Shell::draw` stores the rects it already computes in `app.ui.regions: ShellRegions` every frame. This is a plain copy, with no allocation per frame.
2. **The record hook can be listed.**
   - `widgets::record` keeps its current temp-data write.
   - It also pushes `(name, rect)` onto a per-pass list held in egui temp data under one fixed `Id`. The list is cleared at the start of each pass, by `Shell::draw` before drawing.
   - Under `cfg(test)` it is always on. In other builds it is on only while `layout_snapshot::RECORDING` (a relaxed `AtomicBool`) is set, which only `--layout-json` does. Normal launches pay one relaxed atomic load per call and allocate nothing (CLAUDE.md section 4).
   - Rects are converted to global coordinates with the recording `ui`'s layer transform.
   - When a name is recorded twice in one pass, the last rect wins, as the temp-data map does today, and the duplicate count is kept.
3. **Snapshot content.** Schema in section 5:
   - The window size and scale.
   - The regions: `tab_bar`, `rail`, `left_panel`, `canvas`, `canvas_main`, `dock`, `notes`, `right_panel`, plus `home`, `present` and `variables_full` when those views replace the editor (`shell.rs:1357-1376`, `:1416-1421`).
   - The bounds of each open overlay layer: menus, popovers, modals, toasts. Overlay layers are named `overlay:<egui Order>/<n>`, where `n` is the paint order among overlays.
   - **`named`**: every recorded name with its global `rect`, rounded to 0.25 pt, and its region. The region is the first region whose rect contains the rect's centre, or its overlay.
   - **`unnamed`**: every interactive widget from `prev_pass.widgets` whose rect does not equal a named rect, with `rect`, `sense` and region.
   - Egui `Id`s are not stored. They are hashes that change when code changes, so they cannot identify a widget across the revamp.
4. **The headless gate.** `layout_unchanged` iterates `crates/designos-app/tests/layout-baseline/cases.json`. For each case it:
   - parses the case's `args` with `cli::parse`
   - builds a `Sim` at 1440x900 points, scale 1
   - runs 8 frames
   - captures the snapshot
   - compares it with `<case>.layout.json` under rules G1–G5

   Agent cases use the stub agent CLIs the Agents tests already use (`stub_agent_cli_dir`, `shell/tests.rs:19-38`). The Home case uses a library seeded by `showcase::seed_library` into a temp dir. No case reads the user's library or home directory.
5. **Comparison rules (hard failures).** Tolerances are in points.
   - **G1 Regions.** Every region's `x` and `w` are equal (±0.5). `y` and `h` are equal unless `allow.json` allows growth for that region.
   - **G2 Inventory.** Every baseline name is still recorded. A missing name fails. A new name is reported, not failed, because a revamp widget may add a `record` call. The count of `unnamed` widgets per region or overlay is equal.
   - **G3 Order.**
     - Within a region, named and unnamed widgets are clustered into rows: two widgets share a row when their vertical centres differ by at most `max(4, 0.5·min(h₁, h₂))`.
     - Rows are sorted by top edge, and widgets within a row by left edge.
     - The sequence of names, with unnamed widgets as `_`, is equal row by row.
   - **G4 Horizontal.** Each named rect's left and right edges are within ±2 of the baseline. For unnamed widgets, those at the same row and column position are compared. A difference beyond ±2 needs an `allow.json` entry with `grow:"width"` for that region and case.
   - **G5 Vertical.** No widget moves up (`dy ≥ −0.5`) and none shrinks (`dh ≥ −0.5`). Any `dy > 0.5` or `dh > 0.5` needs an `allow.json` entry with `grow:"height"` for that region, and that entry must name the R5 size that causes it.
6. **Allow list.** `allow.json` is a reviewed list: `[{"case":"<id>|*","region":"<name>","grow":["height"|"width"],"spec":"E","reason":"R5: editor rows 24→32 (row 32)"}]`.
   - An entry without `spec` and `reason` fails to parse.
   - An entry that matched nothing in a run is reported as stale, and fails, so the list cannot rot.
7. **Blessing.**
   - `DESIGNOS_LAYOUT_BLESS=1 cargo test -p designos-app layout_unchanged` rewrites every `<case>.layout.json` and prints a summary.
   - Only two PRs bless: this spec's PR, which records the pre-revamp state, and the PR after spec M, which records the post-revamp state as the new guard. Every PR in between leaves the baseline files untouched.
   - A revamp PR that changes a `*.layout.json` fails review. Its PR body must instead list its `allow.json` additions.
8. **The canvas must not change (G6, local).** Document rendering is not chrome: the page background and content do not change, and canvas overlays keep their Figma-parity colours (plan.md, "Controller defaults").
   - `scripts/layout_baseline_capture.py --compare` crops each case's `canvas_main` region minus the rulers (`RULER_SIZE` 20, `theme.rs:525`) from the baseline and current PNGs.
   - It runs `designos-diff compare --tolerance 0 --threshold 0`, with `--ignore` rects for every overlay layer and every widget rect that lies inside the canvas (the floating toolbar, canvas pills).
   - A non-zero mismatch fails, unless the case is listed in `allow.json` with `region:"canvas"` and a reason.
9. **Screenshots for people.** The same script captures each case as a real `--screenshot` PNG (and `--layout-json`) at 1440x900, scale 1. It writes a side-by-side contact sheet of baseline and current per case. Every revamp PR attaches the sheets of the cases its spec owns (the `spec` field in `cases.json`).
10. **Kit cases are local only.** Cases marked `"kit": true` run only when `--kit <path>` is given. CI has no kit, and the kit is a paid file that is never committed (plan.md, "Facts", and the marketing rule in `CLAUDE.md`).

## 4. Data model / schema changes

None to documents. New in-memory types: `ShellRegions` (copy of eight `Rect`s), `LayoutSnapshot`, `WidgetGeom`. New committed data files in `crates/designos-app/tests/layout-baseline/` (section 5).

## 5. Contracts

**CLI.** `--layout-json OUT.json`, valid only with `--screenshot`. Without it, the parser rejects with `--layout-json needs --screenshot`. The file is written on the same frame as the PNG; if it cannot be written, the process exits 1 as for the PNG. `USAGE` gains `[--layout-json OUT.json]`.

**Snapshot file** (`schema` 1; one file per case; keys sorted; rects `[x, y, w, h]` in points, rounded to 0.25):

```json
{"schema":1,"case":"d-editor-default","size":[1440,900],"scale":1.0,
 "regions":{"tab_bar":[0,0,1440,40],"rail":[0,40,57,860],"left_panel":[57,40,241,860],"canvas":[298,40,901,860],"canvas_main":[298,40,901,860],"dock":null,"notes":null,"right_panel":[1199,40,241,860]},
 "overlays":[{"name":"overlay:Foreground/0","rect":[8,40,218,384]}],
 "named":{"left-panel-resize-handle":{"region":"left_panel","rect":[295,40,6,860],"count":1}},
 "unnamed":[{"region":"rail","rect":[8,52,40,40],"sense":["click"]}]}
```

The numbers are illustrative. The real ones come from the bless run.

**Case list** (`cases.json`, committed). Each entry is `{"id","spec","args",["kit"]}`. The list dry-run in this session (section 10, case 0) is the starting set: **43 cases, 42 after dropping a duplicate** (section 10), covering D (shell, menus, present, board, deck), E (layers, no selection), F (Design and Prototype tabs, pickers, effects, strokes, type details, styles), G (Dev mode, annotate, compare, spec reader), H (Home), I (Agents panel states, permissions, terminal), J (command palette, menus, Find, comments, version history, plugins, Check designs) and K (assets, variables full and floating, libraries). The worker adds the two kit cases:
- `kit-editor`: `<kit> --select "Page header" --zoom 100 --gpu`
- `kit-agents`: `<kit> --select "Avatar group" --zoom 100 --panel agents --gpu`

They match the README capture commands.

**Baseline capture commands** (the exact list, run once on the commit before the first revamp PR, `designos` built from that commit):

```bash
export PATH="/Users/jangtrinh/.local/bin:$HOME/.cargo/bin:$PATH"
cargo build --release -p designos-app -p designos-diff
# 1. Geometry baseline (committed): headless, CI-equivalent
DESIGNOS_LAYOUT_BLESS=1 cargo test -p designos-app --bin designos layout_unchanged
# 2. Determinism check: the same test without BLESS must pass twice in a row
cargo test -p designos-app --bin designos layout_unchanged && cargo test -p designos-app --bin designos layout_unchanged
# 3. Pixel and real-window baseline (not committed; attached to the L PR as a zip, see Decisions 3)
python3 scripts/layout_baseline_capture.py --bin <cargo build dir>/release/designos \
  --cases crates/designos-app/tests/layout-baseline/cases.json --out ~/designos-layout-baseline/<sha> [--kit ~/untitled-ui-kit.fig]
```

The script runs, per case:

```text
designos <args> --size 1440x900 --no-toast --screenshot <out>/<id>.png --layout-json <out>/<id>.layout.json
```

A seeded library is created first with `designos --seed-library <out>/lib`, and `{LIB}` in `args` is replaced by that path.

**PR gate** (every revamp PR, D–K, and A/B/C if they touch rendered UI):
1. CI: `cargo test --workspace --locked` includes `layout_unchanged`. It is a required check (`Test (macos-14)`).
2. Local, pasted in the PR:

   ```bash
   python3 scripts/layout_baseline_capture.py --bin <designos built from the PR> --cases …/cases.json --out <dir> --compare ~/designos-layout-baseline/<baseline sha> --only-spec <key>
   ```

   It prints a G6 verdict per case and writes `<dir>/sheets/<id>.png` (baseline | current). The PR attaches the sheets for its spec's cases. The PR body lists its `allow.json` additions, with the R5 size each one cites.

## 6. File-format mapping

None. `.designos` and `.fig` are untouched.

## 7. UI

None visible. The flag writes a file, and the test is headless. No chrome changes, so `es-designer` is not needed.

## 8. Edge cases

- **A case opens a popover on the frame after build.** Many `--panel` popups do. 8 frames is enough for all 43 dry-run cases to reach their final state (closing check: the bless run's snapshot of frame 8 equals frame 12 for every case; the test asserts this once at bless time).
- **A widget straddles two regions**, such as the left panel's resize handle (`shell.rs:1491-1494`). It belongs to the region containing its centre. The handle is 6 pt wide around the boundary, so its centre is on the boundary line, and ties go to the left region.
- **A scroll area is clipped.** Use `rect`, not `interact_rect`, so rows hidden below the fold still count. Virtualised lists (Layers) only allocate visible rows, so the count depends on panel height. Heights grow under R5, so fewer rows fit, and G2 would fail. For regions whose rows are virtualised (`left_panel` Layers list, Assets grid, variables table), the comparison counts **rows visible in the baseline that are still visible**, plus the order of those rows. The worker marks these regions `"virtualized": true` in `cases.json`, and G2 then compares the leading `min(n_base, n_cur)` widgets.
- **An R5 height change pushes a row out of view** in a non-virtualised panel. G2 fails. The PR must either keep the content reachable (scroll) and add an allow entry with `grow:"height"` and a reason, or fix the layout.
- **Text-width-driven widgets** (a button whose width hugs its label) change width when the type scale changes (R2/R5). G4 fails without an allow entry for `width`. That is intended: the reviewer sees each such change.
- **Agents cases** start a stub agent process through the terminal. The stubs never exit (`shell/tests.rs:9-16`), so tab titles are stable.
- **Missing baseline file for a case** (a new case added later): the test fails with "no baseline; run with DESIGNOS_LAYOUT_BLESS=1 in a PR that adds the case". Unlike `ui vr gate`, a missing baseline is a failure.

## 9. Non-goals

- Pixel equality of chrome (colours change by design).
- Checking colours, type sizes or tokens: specs A and B and their gates (`design_os_self_evolve.py` G01, `theme.rs` tests) do that.
- Running real-window screenshots in CI. CI uses the headless snapshot, which needs no GPU or window.
- Keeping `scripts/measure_ui.py` or `scripts/capture-visual-proof.sh` (untouched, see spec M).
- A layout gate for document content or the canvas renderer beyond G6.

## 10. Test plan (red first)

| # | Case | Expected | Fails before |
|---|---|---|---|
| 0 | **Dry run of the baseline command list (done in this session; see below)** | 43 PNGs at 1440x900 | — |
| 1 | `snapshot_reports_every_region_of_the_editor` | `d-editor-default` has the eight regions. The rail is `RAIL_WIDTH + BORDER_WIDTH` wide. Left and right panels are `LEFT_PANEL_WIDTH`/`RIGHT_PANEL_WIDTH` plus the border. `named` holds `left-panel-resize-handle`. | yes (no module) |
| 1b | `record_hook_lists_every_name_of_the_pass` | two `record` calls in one pass: both names listed. The next pass starts empty. | yes |
| 2 | `layout_gate_is_deterministic` | snapshot twice in one process, plus `cargo test` twice: identical | yes |
| 3 | `widening_the_left_panel_fails_g1` | test-only mutation: `left_panel_width` + 8 gives G1 failure naming `left_panel.w` | yes |
| 4 | `growing_a_row_without_allow_fails_g5_and_with_allow_passes` | synthetic snapshots (pure comparer test): `dh = +8` fails; with an allow entry it passes | yes |
| 5 | `reordering_two_buttons_fails_g3` | synthetic: swap two widgets in a row, G3 fails | yes |
| 6 | `stale_allow_entry_fails` | an allow entry that matched nothing fails with its index | yes |
| 7 | `missing_baseline_fails` | a case without a file fails with the bless hint | yes |
| 8 | `layout_json_requires_screenshot` | `cli::parse(["--layout-json","x.json"])` is an error | yes |
| 9 | `layout_unchanged` (the gate) | green on the pre-revamp commit after bless | yes |
| 10 | Local: `layout_baseline_capture.py --compare` against itself | G6: 0 mismatches on all canvas cases | yes (no script) |

Run:
- `cargo test -p designos-app --bin designos layout_` (twice)
- `cargo test --workspace --locked`
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `python3 scripts/zero_fake_ui_gate.py`
- `python3 -m unittest discover -s scripts/tests -v`, if the capture script gets unit tests for its crop and compare helpers (recommended: `scripts/tests/test_layout_baseline_capture.py`)

**Dry run (case 0), this session.** The baseline command list was run with today's flags. HEAD has no `--layout-json` yet, so PNGs only.
- Runner: `$SCRATCH/agents/w5/tools/capture_layout_baseline.py`, with the cases in `$SCRATCH/agents/w5/l/layout-baseline-cases.json`.
- Command: `python3 -I capture_layout_baseline.py layout-baseline-cases.json baseline-dryrun seed-lib -- cargo run -q --release -p designos-app --bin designos --`.
- Output: `$SCRATCH/agents/w5/l/baseline-dryrun/`.
- Binary: `designos` installed from the HEAD of this session (`ec47da26`, clean `crates/`, toolchain 1.93.1) with `cargo install --locked --path crates/designos-app --bin designos --root $SCRATCH/agents/w5/bin`, so the run did not wait on the shared build lock.

**Dry-run result (2026-10-25):** `captured 43/43; total 13.6 MB`. Every case exited 0 in 0.5–2.8 s. PNGs are 57 KB (`x-present`) to 1.22 MB (`i-agent-permissions`). Excerpt of the log:

```text
seed: 0 …/w5/l/seed-lib
d-editor-default               exit=0   1.3s    250362 B
f-picker                       exit=0   1.3s    361801 B
g-dev-compare                  exit=0   1.4s    925814 B
h-home                         exit=0   0.9s    139140 B
i-agent-permissions            exit=0   2.4s   1221523 B
k-libraries                    exit=0   0.5s    422401 B
x-deck                         exit=0   0.8s    257603 B
captured 43/43; total 13.6 MB
```

The files (`ls baseline-dryrun`): `d-editor-default`, `d-editor-selected`, `d-menu-main`, `d-menu-main-file`, `d-menu-shape`, `d-menu-zoom`, `e-layers-selected`, `e-no-selection`, `f-design-tab`, `f-prototype-tab`, `f-picker`, `f-picker-gradient`, `f-effects-menu`, `f-stroke-settings`, `f-type-details`, `f-effect-styles`, `f-edit-style-color`, `g-dev-react`, `g-dev-annotate`, `g-dev-compare`, `g-spec-reader`, `h-home`, `i-agents`, `i-agents-chat`, `i-agents-picker`, `i-agents-start-failure`, `i-agents-approval-command`, `i-agent-permissions`, `i-terminal`, `j-palette`, `j-cmdk`, `j-find`, `j-comments`, `j-version-history`, `j-plugins`, `j-check-designs`, `k-assets`, `k-variables`, `k-variables-floating`, `k-libraries`, `x-present`, `x-board`, `x-deck` (`.png` each). `f-picker.png` was viewed: a real capture of the Marlow showcase, with the colour picker open on "Balance card".

**Findings that change the case list:**
- Two pairs are byte-identical (`shasum`):
  - `d-editor-selected` = `f-design-tab`. `--tab design` is the default tab.
  - `f-picker` = `f-picker-gradient`. The "Balance card" fill is already a gradient, so both flags open the same picker.
- The worker drops `f-design-tab`, and points `f-picker` at a layer with a solid fill (pick one from `--inspect-json` of the `showcase` fixture). That leaves **42 distinct cases** before the two kit cases.
- The 13.6 MB total confirms Decision 3: PNGs are not committed.

## 11. Performance budgets

- The headless gate: 43 cases × 8 frames. Budget: under 60 s of CI time on `macos-14` (Sim frames run without rendering; INFERENCE from the existing shell tests, closing check: measure in the PR).
- `ShellRegions` copy: eight `Rect`s per frame, no allocation. The snapshot runs only in tests or on the one screenshot frame. Zero idle cost (CLAUDE.md section 4: nothing new on the UI thread at rest).
- Committed baseline size: 43 JSON files. Estimate under 2 MB in total (INFERENCE: a few hundred widgets × about 120 bytes per case). Measured in the PR.

## 12. Dependencies and build order

Lands first. Order:
1. `ShellRegions` recorded in `Shell::draw`.
2. `layout_snapshot.rs` and its unit tests (cases 1, 4–6).
3. `Sim` screen size parameter.
4. The `layout_unchanged` test with `cases.json`.
5. Bless on the pre-revamp commit.
6. Determinism check (case 2).
7. `--layout-json` CLI flag and case 8.
8. `scripts/layout_baseline_capture.py` with G6.
9. The baseline PNG run, zipped and attached to the PR.
10. PR (squash).

No revamp PR merges before this one. The PM blocks D–K on it.

## 13. Owner rulings that apply

- R1 (2026-10-25): keep the current layout and arrangement (regions, panel order, panel widths, where each control sits).
- R5: editor panels use text-xs 12/18 rows and labels, text-sm 14/20 titles, button xs 32, row 32, radius md 8. Home and dialogs use sm/md. This is the only allowed source of height growth.
- R7: spec-driven; the PM coordinates delivery.
- 2026-09-22: visual evidence in every PR (the contact sheets).
- Standing: `main` is protected (PR, green `Test (macos-14)`, Clippy, Rustfmt, squash).

## Decisions on the open questions (writer, 2026-10-25)

1. **Geometry source: screenshots with edge detection, or widget rects?** Widget rects: the named `widgets::record` rects, egui's pass-state rects and the shell's own region rects. Edge detection breaks exactly when colours and hairline contrast change, which is what the revamp does. egui 0.35 exposes the rects publicly, and the shell already computes the regions.
2. **Gate in CI or local only?** Geometry in CI, through the headless `Sim` that already drives the whole shell from CLI options. Pixels (G6) stay local, because CI cannot render a real window reliably and the kit cases cannot run there.
3. **Where baselines live.**
   - Geometry JSON is committed in `crates/designos-app/tests/layout-baseline/`: small, text, reviewable.
   - PNGs are not committed. 43 full-window captures would add several MB to a public repo on every bless, and they are only needed locally for G6 and the contact sheets.
   - They are zipped and attached to the L PR (and to the post-M re-bless PR), and the capture script can regenerate them from any commit.
4. **Match widgets by id, label or position?** By the developer-written `widgets::record` name first: 444 call sites already name the widgets the UI tests click. Unnamed widgets are matched by row and column position. egui `Id`s are code-derived hashes and change with the refactor. `WidgetInfo` labels are debug-only. AccessKit labels exist only for widgets that report them.
5. **Who may bless?** Only L's PR and the post-M PR. Every growth in between is an `allow.json` line with a spec key and an R5 reason, so each one is reviewed rather than silently absorbed.

## Open questions for the owner

None. All the calls here are technical. The visible outcome, "layout unchanged", was decided by R1 and R5.
