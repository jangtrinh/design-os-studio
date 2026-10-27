# Completion Report — `release: 0.6.5`: release notes and workspace version bump on `release/v0.6.5`

- **Branch:** `jangtrinh/agy-r065-release-notes`
- **Base:** `origin/release/v0.6.5` (`5bedbbe78c97eaf56e7b4d857f03c266f1e72aea`)
- **Date:** 2026-10-27
- **Evidence directory:** `plans/reports/agy-r065-release-notes-evidence/`

---

## 1. What could NOT be verified

1. **PRs `#1254` (`#1154`, hug text measurement), `#1251` (`#1043`, Kiwi 64-bit varint 9-byte rule), and `#1256` (`#1242`, resized instance vector geometry scaling) have not merged into `origin/release/v0.6.5` yet.**
   - At completion time (`git fetch origin && gh pr view <n> --json number,state,mergedAt,headRefOid`), PR `#1254` (`origin/jangtrinh/agy-1154-nodespec-text-measure` @ `c78e0e35`), PR `#1251` (`origin/jangtrinh/carry-1043-kiwi-varint` @ `91d66bb7`), and PR `#1256` (`origin/jangtrinh/agy-1242-instance-vector-scale` @ `cce12370`) are `OPEN` (`mergedAt: null`), and `origin/release/v0.6.5` is at `5bedbbe78` (containing the 13 merged PRs `#1213`, `#1236`, `#1237`, `#1238`, `#1239`, `#1247`, `#1249`, `#1252`, `#1253`, `#1255`, `#1257`, `#1258`, `#1259`).
   - Per Controller Ruling 1 and Ruling 4, `CHANGELOG.md` and `README.md` include the release-notes lines for `#1254`, `#1251`, and `#1256` written directly from their PR heads (`origin/jangtrinh/agy-1154-nodespec-text-measure`, `origin/jangtrinh/carry-1043-kiwi-varint`, and `origin/jangtrinh/agy-1242-instance-vector-scale`), and the three `#1254`-specific backticked spans (`RenderTextMeasure`, `TEXT_MEASURED_WITH_FALLBACK_FONT`, `measure_text_box`) are listed in the backtick-span table as composed on `5bedbbe7` with their exact `file:line` locations on `origin/jangtrinh/agy-1154-nodespec-text-measure` (all backticked spans in the `#1251` and `#1256` bullets already exist verbatim on `5bedbbe7`).
   - Test-only PR `#1246` (`#1205`) is also `OPEN` and, per the task spec, gets no `CHANGELOG.md` line.
2. **Live model turns and non-macOS interactive desktop sessions were not run.**
   - Live Claude, Gemini CLI, Grok, and Antigravity sessions, as well as interactive Linux and Windows desktop sessions, were not executed during this release-notes and version-bump task; those known limitations remain explicitly listed in `### Known issues` of `## [0.6.5]`.

---

## 2. Conclusion, Epistemic Ledger, Weakest Link & Skills Loaded

### Conclusion
`CHANGELOG.md` now has `## [0.6.5] - 2026-10-27` (covering all 13 merged PRs since `v0.6.3` plus the 3 open PRs `#1254`, `#1251`, and `#1256` grouped under `### Added`, `### Changed`, and `### Fixed`, and an updated `### Known issues` section) and `## [0.6.4] - 2026-10-27` directly below it explaining in one sentence that `0.6.4` was not published because the tag was pushed before the version bump and that its fixes ship in `0.6.5`. `README.md` updates `What's new` to `0.6.5`, adds `<a id="new-in-v065"></a>` and `### 0.6.5` (6 bullets drawn from `CHANGELOG.md`) above `### 0.6.3`, and updates the Windows test suite advisory note in `### Known issues` to `0.6.5`. The workspace version in `Cargo.toml` and `Cargo.lock` is bumped from `0.6.3` to `0.6.5` in its own commit (`release: 0.6.5`), verified via `cargo metadata`, `cargo build -p designos-diff --locked`, `designos-diff --version`, and `cargo fmt --all -- --check`.

### Epistemic Ledger
- `FACT`: `git log --oneline --no-merges v0.6.3..origin/release/v0.6.5` contains 13 distinct PRs (`#1213`, `#1236`, `#1237`, `#1238`, `#1239`, `#1247`, `#1249`, `#1252`, `#1253`, `#1255`, `#1257`, `#1258`, `#1259`; with `#1236`, `#1237`, `#1238`, `#1239` appearing twice across the two `0.6.4` lineages). PRs `#1254` (`c78e0e35`), `#1251` (`91d66bb7`), and `#1256` (`cce12370`) are open against `release/v0.6.5`.
- `FACT`: Every GitHub issue referenced in `0.6.3`'s `### Known issues` (`#1097`, `#1074`, `#1066`, `#1048`, `#1047`, `#1059`, `#1101`, `#930`, `#1096`, `#1073`, `#1158`, `#1061`, `#1068`, `#1069`, `#1058`, `#1102`) and `README.md`'s `### Known issues` (`#834`, `#977`, `#1158`) was checked with `gh issue view`; none of the open issues listed there (`#1097`, `#1074`, `#1066`, `#1048`, `#1047`, `#1059`, `#1101`, `#930`, `#1096`, `#1073`, `#1158`, `#1102`, `#834`, `#977`) were closed or fixed by a `0.6.5` PR, so all lines are retained and the `#1158` Windows advisory line is updated from `0.6.3` to `0.6.5`.
- `FACT`: All 148 unique backticked spans in `## [0.6.5]` of `CHANGELOG.md` were checked with `git grep -n -F -e`: 144 are found verbatim on `HEAD` (`5bedbbe7`), 3 are found verbatim in PR `#1254` (`origin/jangtrinh/agy-1154-nodespec-text-measure`), and 1 is a composed CLI example (`designos FILE --export-variables OUT.json --format dtcg --mode MODE`) whose flags and structure exist in `crates/designos-app/src/cli.rs:430-450`.
- `FACT`: `cargo metadata --format-version 1 --no-deps --locked` reports version `{'0.6.5'}` across all 13 workspace crates, and `designos-diff --version` built under the isolated harness prints `designos-diff 0.6.5`.
- `INFERENCE`: Once PRs `#1254`, `#1251`, and `#1256` merge into `release/v0.6.5` and `origin/release/v0.6.5` is merged into `jangtrinh/agy-r065-release-notes`, the 3 PR `#1254` backticked spans will also match directly on `HEAD`.
- `ASSUMPTION`: None.

### Weakest Link
PRs `#1254`, `#1251`, and `#1256` are still open against `release/v0.6.5`; if any of those three PRs changes its user-facing identifiers before merging (or does not land in `0.6.5`), its corresponding bullet in `CHANGELOG.md` and `README.md` must be updated when merging `origin/release/v0.6.5` per Ruling 4.

### Skills Loaded & Changes Made Because of Them
- `es:dev` (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`) & `es:feature-dev` (`/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`): Loaded `.project-agent.md`, `CONTEXT.md`, and `.brv/context-tree/session-handoffs/latest.md` before editing, kept scope strictly limited to the allowed files, and structured the 3 commits in the exact order specified by Ruling 5.
- `es:rust` (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`) & `es:rust-gpu-canvas` (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`): Verified workspace package version propagation through `Cargo.toml` and `Cargo.lock` (`cargo update -w --offline`) and confirmed no Rust source or GPU/render paths were modified.
- `es-astra-work` (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`) & `es:fable-thinking` (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`): Applied falsification-first `git grep -n -F` verification on every backticked span in `## [0.6.5]`, which caught that the DTCG provenance key in `crates/designos-fig/src/variables_dtcg.rs:4817` is `alias_of` (snake_case) rather than `aliasOf` and that the Kiwi varint methods in `crates/designos-fig/src/kiwi.rs:242,973` are named `var_u64`; updated `CHANGELOG.md` before finalizing Commit 1 so those spans match `HEAD` verbatim.

---

## 3. Acceptance Gates & Verbatim Outputs

| # | Gate | Command | Result |
|---|---|---|---|
| 1a | Allowed files only (`origin/release/v0.6.5..HEAD`) | `git diff --stat origin/release/v0.6.5..HEAD` | **PASS** — Only `CHANGELOG.md`, `README.md`, `Cargo.toml`, `Cargo.lock`, plus this report and evidence in Commit 3 |
| 1b | Bump commit isolation (`release: 0.6.5`) | `git diff --stat fedb6e67..7c5ece98` (`git diff --stat HEAD~1` on Commit 2) | **PASS** — Only `Cargo.toml` and `Cargo.lock` |
| 2 | Backtick-span verification (`## [0.6.5]`) | `git grep -n -F -e <span> -- crates/ scripts/ docs/ skills/` | **PASS** — 148 unique spans: 144 found verbatim on `HEAD`, 3 found verbatim in PR `#1254` (composed on `5bedbbe7`), 1 composed CLI example |
| 3 | Workspace crate versions | `cargo metadata --format-version 1 --no-deps --locked \| python3 -c 'import json,sys;print({p["version"] for p in json.load(sys.stdin)["packages"]})'` | **PASS** — Prints `{'0.6.5'}` |
| 4a | Isolated harness build | `cargo build -p designos-diff --locked` (under isolated `HOME`/`TMPDIR`) | **PASS** — Exit `0` |
| 4b | Binary version check | `"$CARGO_TARGET_DIR/debug/designos-diff" --version` | **PASS** — Prints `designos-diff 0.6.5` |
| 4c | Rustfmt check | `cargo fmt --all -- --check` | **PASS** — Exit `0` |
| 4d | Temporary directory cleanup | `rm -rf "$PWD/out" "$S" && test ! -e "$PWD/out"` | **PASS** — `out/` and `$S` removed |

### 3.1 `git diff --stat` Outputs

Before Commit 3 (docs commit `fedb6e67` and version bump commit `7c5ece98`):
```text
$ git diff --stat origin/release/v0.6.5..7c5ece98
 CHANGELOG.md | 57 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 Cargo.lock   | 26 +++++++++++++-------------
 Cargo.toml   |  2 +-
 README.md    | 14 ++++++++++++--
 4 files changed, 83 insertions(+), 16 deletions(-)
```

On the `release: 0.6.5` bump commit (`7c5ece98`):
```text
$ git diff --stat fedb6e67..7c5ece98
 Cargo.lock | 26 +++++++++++++-------------
 Cargo.toml |  2 +-
 2 files changed, 14 insertions(+), 14 deletions(-)
```

### 3.2 Backtick-Span Table for `## [0.6.5]` (`CHANGELOG.md`)

All 148 unique backticked spans in `## [0.6.5]` of `CHANGELOG.md` (raw output saved in `plans/reports/agy-r065-release-notes-evidence/backtick-spans-grep.txt`):

| # | Backticked span | Classification | `git grep -n -F` match or composition source |
|---|---|---|---|
| 1 | `#RRGGBB` | Found | `crates/designos-app/src/canvas/draw.rs:1262` |
| 2 | `#RRGGBBAA` | Found | `crates/designos-app/src/widgets/picker.rs:331` |
| 3 | `$extensions` | Found | `crates/designos-fig/src/variables_dtcg.rs:289` |
| 4 | `%XX` | Found | `crates/designos-app/src/home/dialog.rs:588` |
| 5 | `(2)` | Found | `crates/designos-app/src/agent_busy_frames/field_diff_tests.rs:98` |
| 6 | `--apply-spec` | Found | `crates/designos-app/assets/specs/easeui-agent-console.spec.json:9` |
| 7 | `--collection` | Found | `crates/designos-app/src/cli.rs:383` |
| 8 | `--color-format` | Found | `crates/designos-app/src/cli.rs:385` |
| 9 | `--edit-text` | Found | `crates/designos-app/src/cli.rs:327` |
| 10 | `--fixture` | Found | `crates/designos-app/Cargo.toml:25` |
| 11 | `--home` | Found | `crates/designos-app/src/cli.rs:142` |
| 12 | `--inspect-json` | Found | `crates/designos-app/src/agent_cli_select_tests.rs:110` |
| 13 | `--panel agents-scrollback` | Found | `crates/designos-app/src/terminal/zoom_scrollback_tests.rs:625` |
| 14 | `--resolve-aliases` | Found | `crates/designos-app/src/cli.rs:384` |
| 15 | `--seed-library` | Found | `crates/designos-app/Cargo.toml:25` |
| 16 | `--select` | Found | `crates/designos-app/src/agent_cli_select_tests.rs:1` |
| 17 | `--select ""` | Found | `crates/designos-app/src/select_target/error.rs:7` |
| 18 | `--version` | Found | `crates/designos-app/src/agent_cli_catalog.rs:110` |
| 19 | `--zoom fit` | Found | `crates/designos-app/src/cli.rs:29` |
| 20 | `.fig` | Found | `crates/designos-app/src/agent_busy_frames.rs:119` |
| 21 | `.gemini` | Found | `crates/designos-app/src/agent_composer.rs:365` |
| 22 | `.gemini/settings.json` | Found | `crates/designos-app/src/agent_session_files/fs_sites_scan_tests/patterns.rs:53` |
| 23 | `.spec.json` | Found | `crates/designos-app/assets/specs/easeui-agent-console.spec.json:482` |
| 24 | `.tokens.json` | Found | `crates/designos-app/src/main.rs:466` |
| 25 | `.zip` | Found | `crates/designos-app/src/agent_busy_frames/field_diff_tests.rs:585` |
| 26 | `/path` | Found | `crates/designos-app/src/agent_sessions.rs:687` |
| 27 | `536870912` | Found | `crates/designos-mcp/tests/session.rs:320` |
| 28 | `About this file` | Found | `crates/designos-app/assets/starter/designos-studio-starter.designos:1` |
| 29 | `App::run_tx` | Found | `crates/designos-app/src/cmdk.rs:1059` |
| 30 | `DESIGN-OS Studio Starter` | Found | `crates/designos-app/assets/starter/designos-studio-starter.designos:1` |
| 31 | `DESIGN-OS Studio Starter (1)` | Found | `crates/designos-app/src/home/mod.rs:2780` |
| 32 | `Design Token Sheet` | Found | `crates/designos-app/src/home/mod.rs:2678` |
| 33 | `Done` | Found | `crates/designos-app/src/agent_a11y.rs:234` |
| 34 | `ESC[3J` | Found | `crates/designos-app/src/terminal.rs:108` |
| 35 | `Enter` | Found | `crates/designos-app/src/agent_composer.rs:3` |
| 36 | `Enter a path…` | Found | `crates/designos-app/src/app.rs:147` |
| 37 | `Esc` | Found | `crates/designos-app/src/agent_approval_card.rs:210` |
| 38 | `FILE_TOO_LARGE` | Found | `crates/designos-mcp/generated/rules.md:156` |
| 39 | `Getting started` | Found | `crates/designos-app/src/home/mod.rs:990` |
| 40 | `Height` | Found | `crates/designos-app/assets/starter/designos-studio-starter.designos:1` |
| 41 | `Help and account` | Found | `crates/designos-app/src/panels/menus.rs:571` |
| 42 | `INVALID_NAME` | Found | `crates/designos-mcp/generated/rules.md:132` |
| 43 | `Import` | Found | `crates/designos-app/src/app.rs:263` |
| 44 | `Launch Brainstorm` | Found | `crates/designos-app/src/shell/tests/home.rs:770` |
| 45 | `MAX_IMPORT_FILE_BYTES` | Found | `crates/designos-mcp/src/server.rs:626` |
| 46 | `Marlow — Mobile Banking Kit` | Found | `crates/designos-app/src/shell/tests/home.rs:791` |
| 47 | `NAME_TOO_LONG` | Found | `crates/designos-mcp/generated/rules.md:132` |
| 48 | `Needs approval` | Found | `crates/designos-app/src/agent_a11y.rs:222` |
| 49 | `No, suggest changes` | Found | `crates/designos-app/src/gemini_dialog.rs:5` |
| 50 | `NodeSpec` | Found | `crates/designos-app/assets/specs/easeui-agent-console.spec.json:9` |
| 51 | `Open starter sample` | Found | `crates/designos-app/src/home/mod.rs:1470` |
| 52 | `Open tab` | Found | `crates/designos-app/src/agent_approval_card.rs:36` |
| 53 | `PATH_OUTSIDE_WORKSPACE` | Found | `crates/designos-mcp/generated/rules.md:132` |
| 54 | `Product Exploration` | Found | `crates/designos-app/src/shell/tests/home.rs:770` |
| 55 | `Product UI Kit` | Found | `crates/designos-app/src/demo.rs:32` |
| 56 | `REMOVED` | Found | `crates/designos-app/src/agent_session_files/no_follow/remove.rs:37` |
| 57 | `RenderTextMeasure` | Composed on `5bedbbe7` (Found in PR `#1254`) | `origin/jangtrinh/agy-1154-nodespec-text-measure:crates/designos-app/src/main.rs:132` |
| 58 | `Resources & Libraries` | Found | `crates/designos-app/src/home/mod.rs:381` |
| 59 | `Show prompt` | Found | `crates/designos-app/src/agent_approval_card.rs:36` |
| 60 | `Starter screens` | Found | `crates/designos-app/assets/starter/designos-studio-starter.designos:1` |
| 61 | `Stop` | Found | `crates/designos-app/src/agent_a11y.rs:240` |
| 62 | `TEXT_MEASURED_WITH_FALLBACK_FONT` | Composed on `5bedbbe7` (Found in PR `#1254`) | `origin/jangtrinh/agy-1154-nodespec-text-measure:crates/designos-doc/src/spec.rs:696` |
| 63 | `TOAST_REVEAL_SECONDS` | Found | `crates/designos-app/src/app.rs:379` |
| 64 | `TOAST_SECONDS` | Found | `crates/designos-app/src/app.rs:371` |
| 65 | `Tab` | Found | `crates/designos-app/assets/starter/designos-studio-starter.designos:1` |
| 66 | `TabTurnTracker` | Found | `crates/designos-app/src/agent_stop.rs:1261` |
| 67 | `Templates & Specs` | Found | `crates/designos-app/src/home/mod.rs:380` |
| 68 | `ToolPermission` | Found | `crates/designos-app/src/agent_turn_status.rs:751` |
| 69 | `Type the full path to the file, starting with / or ~/.` | Found | `crates/designos-app/src/home/dialog.rs:547` |
| 70 | `Unnamed effect style` | Found | `crates/designos-app/src/panels/style_picker.rs:340` |
| 71 | `Unnamed fill style` | Found | `crates/designos-doc/src/lib.rs:760` |
| 72 | `Unnamed grid style` | Found | `crates/designos-doc/src/lib.rs:763` |
| 73 | `Unnamed text style` | Found | `crates/designos-doc/src/lib.rs:761` |
| 74 | `Usage: designos-mcp --workspace DIRECTORY | --version | --list-lint-rules | --help` | Found | `crates/designos-mcp/src/main.rs:29` |
| 75 | `Welcome` | Found | `crates/designos-app/src/cmdk.rs:1421` |
| 76 | `WidthAndHeight` | Found | `crates/designos-app/assets/starter/designos-studio-starter.designos:1` |
| 77 | `Working…` | Found | `crates/designos-app/src/agent_open_flow.rs:49` |
| 78 | `agents-approval-background-tab` | Found | `crates/designos-app/src/cli.rs:161` |
| 79 | `alias_of` | Found | `crates/designos-doc/src/spec.rs:2108` |
| 80 | `aliases_resolved_via_default_mode` | Found | `crates/designos-app/src/main.rs:520` |
| 81 | `apply_constrained_geometry` | Found | `crates/designos-app/src/layout.rs:194` |
| 82 | `ask_user` | Found | `crates/designos-app/src/gemini_dialog.rs:16` |
| 83 | `before_nodes` | Found | `crates/designos-app/src/agent_busy_frames/field_diff_tests.rs:545` |
| 84 | `cosmic-text` | Found | `crates/designos-app/Cargo.toml:45` |
| 85 | `crates/designos-app/assets/starter/designos-studio-starter.designos` | Found | `crates/designos-app/src/home/session.rs:16` |
| 86 | `derivedSymbolData` | Found | `crates/designos-app/assets/starter/designos-studio-starter.designos:1` |
| 87 | `designos` | Found | `crates/designos-app/Cargo.toml:2` |
| 88 | `designos FILE --export-variables OUT.json --format dtcg --mode MODE` | Composed | Composed CLI invocation from `crates/designos-app/src/cli.rs:430-450` (`designos [FILE] ... --export-variables PATH --format dtcg --mode NAME`) |
| 89 | `designos-mcp` | Found | `crates/designos-app/Cargo.toml:39` |
| 90 | `designos-studio` | Found | `crates/designos-app/Cargo.toml:7` |
| 91 | `designos: warn: import path` | Found | `crates/designos-app/src/home/dialog.rs:565` |
| 92 | `designos_apply_batch` | Found | `crates/designos-mcp/generated/rules.md:10` |
| 93 | `designos_apply_spec` | Found | `crates/designos-app/src/agent_a11y.rs:514` |
| 94 | `designos_create_node` | Found | `crates/designos-app/src/agent_session_files/fs_sites_allowlist.txt:93` |
| 95 | `designos_export_fig_file` | Found | `crates/designos-app/src/agent_turn_status.rs:501` |
| 96 | `designos_get_document` | Found | `crates/designos-app/src/agent_mention.rs:925` |
| 97 | `designos_import_web_capture` | Found | `crates/designos-mcp/generated/rules.md:153` |
| 98 | `designos_open_file` | Found | `crates/designos-app/src/agent_turn_status.rs:498` |
| 99 | `designos_render_png` | Found | `crates/designos-app/src/agent_turn_status.rs:502` |
| 100 | `designos_save_file` | Found | `crates/designos-app/src/agent_turn_status.rs:499` |
| 101 | `designos_search_design_system` | Found | `crates/designos-mcp/generated/rules.md:39` |
| 102 | `designos_specs` | Found | `crates/designos-app/src/agent_session_files/fs_sites_allowlist.txt:21` |
| 103 | `designos_update_node` | Found | `crates/designos-app/src/agent_session_files/fs_sites_allowlist.txt:93` |
| 104 | `examples/` | Found | `crates/designos-app/src/agent_session_files/fs_sites_allowlist.txt:21` |
| 105 | `exit_plan_mode` | Found | `crates/designos-app/src/gemini_dialog.rs:16` |
| 106 | `fig:<figma id>` | Found | `crates/designos-app/src/cli.rs:1` |
| 107 | `file://` | Found | `crates/designos-app/src/dev/mod.rs:351` |
| 108 | `fillGeometry` | Found | `crates/designos-doc/src/node.rs:111` |
| 109 | `fixtures` | Found | `crates/designos-app/Cargo.toml:25` |
| 110 | `font_family` | Found | `crates/designos-app/assets/starter/designos-studio-starter.designos:1` |
| 111 | `guid` | Found | `crates/designos-app/assets/starter/designos-studio-starter.designos:1` |
| 112 | `hex` | Found | `crates/designos-app/assets/icons/lucide/hexagon.svg:3` |
| 113 | `hooks` | Found | `crates/designos-app/src/agent_approval_channel.rs:4` |
| 114 | `id:<n>` | Found | `crates/designos-app/src/cli.rs:1` |
| 115 | `import_size_gate` | Found | `crates/designos-mcp/src/server.rs:628` |
| 116 | `isSoftDeleted` | Found | `crates/designos-fig/FIELDS.md:90` |
| 117 | `isSoftDeletedStyle` | Found | `crates/designos-fig/src/fig.rs:819` |
| 118 | `is_orphan_style` | Found | `crates/designos-app/src/panels/style_picker.rs:398` |
| 119 | `is_table_style` | Found | `crates/designos-app/src/panels/properties.rs:266` |
| 120 | `measure_text_box` | Composed on `5bedbbe7` (Found in PR `#1254`) | `origin/jangtrinh/agy-1154-nodespec-text-measure:crates/designos-mcp/src/engine.rs:4029` |
| 121 | `name:<text>` | Found | `crates/designos-app/src/cli.rs:1` |
| 122 | `next_step` | Found | `crates/designos-app/src/agent_session_files.rs:16` |
| 123 | `none` | Found | `crates/designos-app/assets/icons/lucide/align-center-horizontal.svg:8` |
| 124 | `object` | Found | `crates/designos-app/src/agent_busy_frames/field_diff_tests.rs:302` |
| 125 | `on_process_exit` | Found | `crates/designos-app/src/agent_turn_status.rs:869` |
| 126 | `overrideKey` | Found | `crates/designos-fig/FIELDS.md:74` |
| 127 | `permission_prompt` | Found | `crates/designos-app/src/agent_turn_status.rs:132` |
| 128 | `plan_constraint_pass` | Found | `crates/designos-app/src/layout.rs:216` |
| 129 | `record_user_node_update` | Found | `crates/designos-app/src/agent_busy_frames.rs:302` |
| 130 | `resolve_pty_heuristic_status` | Found | `crates/designos-app/src/terminal.rs:764` |
| 131 | `safe_to_retry: false` | Found | `crates/designos-mcp/src/server.rs:3437` |
| 132 | `selection` | Found | `crates/designos-app/assets/specs/easeui-agent-console.spec.json:334` |
| 133 | `source_id` | Found | `crates/designos-app/assets/starter/designos-studio-starter.designos:1` |
| 134 | `strokeGeometry` | Found | `crates/designos-doc/src/node.rs:116` |
| 135 | `strokeSeed` | Found | `crates/designos-fig/src/figma_schema.txt:1583` |
| 136 | `style_refs` | Found | `crates/designos-app/assets/starter/designos-studio-starter.designos:1` |
| 137 | `tokens.manifest.json` | Found | `crates/designos-app/src/main.rs:468` |
| 138 | `tools.allowed` | Found | `docs/specs/v0.6/656-agents-ask-permission-mode-with-in-panel-approvals.md:135` |
| 139 | `tools.discoveryCommand` | Found | `docs/specs/v0.6/656-agents-ask-permission-mode-with-in-panel-approvals.md:138` |
| 140 | `tty` | Found | `crates/designos-app/src/agent_permissions.rs:209` |
| 141 | `validate_output_name` | Found | `crates/designos-mcp/src/engine.rs:1554` |
| 142 | `values_from_default_mode` | Found | `crates/designos-app/src/main.rs:521` |
| 143 | `var_u64` | Found | `crates/designos-fig/src/kiwi.rs:242` |
| 144 | `workspace-write` | Found | `crates/designos-app/src/agent_approval_channel.rs:6` |
| 145 | `{…}` | Found | `crates/designos-fig/src/variables_dtcg.rs:4803` |
| 146 | `~` | Found | `crates/designos-app/src/agent_approval_channel.rs:11` |
| 147 | `~/…` | Found | `crates/designos-app/src/home/dialog.rs:620` |
| 148 | `…` | Found | `crates/designos-app/src/agent_approval_card.rs:203` |

### 3.3 `cargo metadata`, Harness Build, `designos-diff --version` & `cargo fmt` Outputs

```text
$ cargo metadata --format-version 1 --no-deps --locked | python3 -c 'import json,sys;print({p["version"] for p in json.load(sys.stdin)["packages"]})'
{'0.6.5'}

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo build -p designos-diff --locked
   Compiling designos-diff v0.6.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-r065-release-notes/crates/designos-diff)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.08s

$ "$CARGO_TARGET_DIR/debug/designos-diff" --version
designos-diff 0.6.5

$ cargo fmt --all -- --check
(exit 0, no output)
```

---

## 4. Changes per File

- `CHANGELOG.md`: Added `## [0.6.5] - 2026-10-27` above `## [0.6.3] - 2026-10-26` with `### Added` (`Starter file` for `#1212` PR `#1252`; `Import and export` for `#1140` PR `#1253`), `### Changed` (`Canvas` for `#993` PR `#1249`), `### Fixed` (`Agents panel` for `#1171` PR `#1236`, `#1194` PR `#1238`, `#1195` PR `#1247`; `Canvas` for `#1193` PR `#1237`, `#1197` PR `#1239`, `#1154` PR `#1254`; `Import and export` for `#1192` PR `#1213`, `#1043` PR `#1251`, `#1242` PR `#1256`; `Styles` for `#1165` PR `#1255`; `CLI and MCP` for `#1121` PR `#1257`, `#1003` PR `#1258`, `#1127` PR `#1259`), and `### Known issues` (carried over from `0.6.3` with the `#1158` Windows test suite advisory line updated to `0.6.5`), plus `## [0.6.4] - 2026-10-27` directly below `## [0.6.5]`.
- `README.md`: Updated `The newest is 0.6.3.` to `The newest is 0.6.5.` in `## What's new`, added `<a id="new-in-v065"></a>` and `### 0.6.5` (6 bullets drawn from `CHANGELOG.md`) above `<a id="new-in-v063"></a>`, and updated the `#1158` Windows test suite advisory bullet in `### Known issues` from `0.6.3` to `0.6.5`.
- `Cargo.toml`: Bumped `[workspace.package]` `version` from `"0.6.3"` to `"0.6.5"`.
- `Cargo.lock`: Updated workspace crate versions from `0.6.3` to `0.6.5` via `cargo update -w --offline`.
- `plans/reports/agy-r065-release-notes.md` & `plans/reports/agy-r065-release-notes-evidence/`: Added this report and raw verification outputs.

---

## 5. Verbatim Release Notes Added

### 5.1 `CHANGELOG.md` — `## [0.6.5] - 2026-10-27` and `## [0.6.4] - 2026-10-27`

````markdown
## [0.6.5] - 2026-10-27

Patch for **AI design** ([milestone v0.6.5](https://github.com/jangtrinh/design-os-studio/milestone/52)): Studio ships the `DESIGN-OS Studio Starter` file, exports a single variable mode as one self-contained DTCG `.tokens.json` file with alias resolution and hex colors, addresses CLI nodes by name, id or Figma id, measures hug text with the renderer, and speeds up whole-page edits.

### Added
- **Starter file.**
  - New libraries and upgrades from 0.4.0–0.6.4 seed a `Getting started` project with the bundled `DESIGN-OS Studio Starter` file (`crates/designos-app/assets/starter/designos-studio-starter.designos`, with pages `Starter screens` and `About this file`) without touching existing user files. `Open starter sample` in Home (`Templates & Specs` and `Resources & Libraries`) and `Help and account` › `Open starter sample` open a new copy in the current project (`DESIGN-OS Studio Starter (1)`, `(2)`, … when the name is taken), and launching with no file argument opens Home. The older sample builders (`Marlow — Mobile Banking Kit`, `Launch Brainstorm`, `Product Exploration`, `Design Token Sheet`, `Product UI Kit`, `Welcome`), `--seed-library`, `--fixture` (in release builds without the `fixtures` feature) and the `examples/` folder are removed (#1212, PR #1252).
- **Import and export.**
  - `designos FILE --export-variables OUT.json --format dtcg --mode MODE` exports one variable mode across the file's collections (or the collections named with `--collection`) as a single self-contained DTCG `.tokens.json` file. When a token has no value in the chosen mode, Studio falls back to its collection's default mode (`values_from_default_mode`) and inlines cross-collection alias targets so every `{…}` reference resolves inside the file; an alias whose target has no value in any mode stops the export before writing. `--resolve-aliases` replaces `{…}` alias strings with resolved literal values while keeping `alias_of` under `$extensions` (`aliases_resolved_via_default_mode` counts default-mode hops), and `--color-format` (`hex` or `object`) writes `#RRGGBB` / `#RRGGBBAA` strings when set to `hex`. Multi-file DTCG folder and `.zip` exports give same-named local and library collections unique group and file names and round-trip them through `tokens.manifest.json` (#1140, PR #1253).

### Changed
- **Canvas.**
  - Whole-page and multi-node edits (`App::run_tx`, such as command-palette auto-fix on a large design-system page) record only the nodes whose content changed (`record_user_node_update`) and compare typed fields directly instead of cloning `before_nodes` and serializing every touched node to JSON twice (#993, PR #1249).

### Fixed
- **Agents panel.**
  - When an agent CLI (such as Antigravity) repaints its screen on a UI zoom or terminal resize with `ESC[3J` (erase scrollback), Studio clears that tab's scrollback buffer instead of appending another copy of the screen, and computes the scrollbar thumb and wheel range from the real scrollback length (`--panel agents-scrollback` previews a 120-line scrollback) (#1171, PR #1236).
  - A Codex tab whose turn ends quietly decays from `Working…` to `Done` after 1.5 s without output (`resolve_pty_heuristic_status`), closes the turn in `TabTurnTracker`, hides the `Stop` button once no turn is running, and passes a PTY process exit to `on_process_exit` (#1194, PR #1238).
  - When the tab you are viewing is waiting for approval in its terminal, the approval banner shows `Show prompt` instead of `Open tab` and focuses that tab's terminal so `Enter` and `Esc` reach the CLI prompt directly. When another tab is waiting, `Open tab` switches to that waiting tab (or the oldest waiting tab when several tabs wait), and both buttons are keyboard-reachable with `Tab` and `Enter` (`agents-approval-background-tab`) (#1195, PR #1247).
- **Canvas.**
  - Renaming a component or frame from an agent session through `designos-mcp` repaints the canvas frame label when the live patch lands. Clicking a single-shape component on the canvas selects the component while double-clicking enters the shape, and renaming a component or its child shape keeps their names independent while updating unrenamed instances (#1193, PR #1237).
  - Frame, component and instance name labels on the canvas are measured with the app's own fonts and cut to fit with `…` inside the frame's on-screen width minus a 4.5 px right inset, the way Figma does; when a component or instance glyph plus `…` does not fit, only `…` is drawn (#1197, PR #1239).
  - Hug (`WidthAndHeight`) and fixed-width (`Height`) text layers in `NodeSpec` (`--apply-spec`, `designos_apply_spec`, `designos_create_node`, `designos_update_node` and `designos_apply_batch`) are measured with the renderer's `cosmic-text` shaper (`RenderTextMeasure` / `measure_text_box`) instead of a character-count width estimate, honouring `font_family` and returning `TEXT_MEASURED_WITH_FALLBACK_FONT` when a requested font falls back (#1154, PR #1254).
- **Import and export.**
  - Home › `Import` › `Enter a path…` trims whitespace and one pair of quotes, expands `~` and `~/…`, decodes `file://` links (`%XX`), refuses a relative path with `Type the full path to the file, starting with / or ~/.`, and logs `designos: warn: import path` on stderr when a typed open fails. Toast lifetimes (`TOAST_SECONDS`, `TOAST_REVEAL_SECONDS`) start on the toast's first painted frame so a slow frame cannot expire a toast before it is shown (#1192, PR #1213).
  - Kiwi 64-bit varint decoding and encoding (`var_u64`) treat the ninth byte as a full 8-bit byte with no continuation flag, fixing `.fig` imports where a `strokeSeed` with bit 63 set desynced the stream and preventing 10-byte varints on export (#1043, PR #1251).
  - Resized component instances in `.fig` imports index fast `derivedSymbolData` entries by both `guid` and `overrideKey`, apply derived `strokeGeometry` to stroke-only vectors without wiping open paths when `fillGeometry` is empty, and scale vector and boolean geometry through `apply_constrained_geometry` (`plan_constraint_pass` across nested containers, shallowest path first) while keeping stroke weight unchanged (#1242, PR #1256).
- **Styles.**
  - Deleted, `REMOVED`, `isSoftDeleted` / `isSoftDeletedStyle` and consumer-only orphan style records (`is_orphan_style` / `is_table_style`) are hidden from style pickers, Assets, library publishing, layer style chips, `designos_get_document` and `designos_search_design_system`, while layers keep their raw values and `style_refs` for `.fig` round-trip. Listed styles whose name is empty or whitespace-only display as `Unnamed fill style`, `Unnamed text style`, `Unnamed effect style` or `Unnamed grid style`, with the raw ID in the tooltip (#1165, PR #1255).
- **CLI and MCP.**
  - `designos_open_file` and `designos_import_web_capture` share one 512 MiB (`536870912` bytes, `MAX_IMPORT_FILE_BYTES`) import size gate (`import_size_gate`). Files over the limit fail with `FILE_TOO_LARGE` at `/path`, `safe_to_retry: false`, and a `next_step` that points to DESIGN:OS Studio, and `designos-mcp` usage errors list `--version` (`Usage: designos-mcp --workspace DIRECTORY | --version | --list-lint-rules | --help`) (#1121, PR #1257).
  - `designos_save_file`, `designos_render_png`, `designos_export_fig_file` and the shared MCP output-path validator (`validate_output_name`) return `INVALID_NAME` on control characters, bidi-override characters or whitespace-only file/folder names and stems, and `NAME_TOO_LONG` on path components over 255 UTF-8 bytes without an overwrite hint, before falling back to missing-folder or `PATH_OUTSIDE_WORKSPACE` errors on blank paths (#1003, PR #1258).
  - `designos` and `designos-studio` `--select` accepts `id:<n>`, `fig:<figma id>`, `name:<text>`, `none` or a layer name (trying a bare number as a layer name before a node id, and expanding lazy pages once on a miss), supports `--zoom fit` to frame the selection, reports `--inspect-json` `selection` bounds in page coordinates with `source_id` on screens, rejects `--select ""` and `--select` with `--home` at parse time, and on a `--select` or `--edit-text` miss prints what was searched on stderr, exits 2, and writes no PNG or file change (#1127, PR #1259).

### Known issues
- Esc on the Other CLI menu or form also reaches Codex and puts it in rewind mode (#1097).
- A document with no path gives `designos-mcp` the whole disk as its workspace (#1074). In Ask mode each `designos-mcp` call asks first.
- A prompt that contains `No, suggest changes` can hold the automatic Enter, and refuse sends, in a Gemini Ask tab until the line scrolls off (#1066).
- Antigravity, Gemini CLI and Grok start MCP servers from copies of the session config that an agent can write, so a server command added there could run outside any sandbox (#1048). It needs an agent that can already write its session folder without asking, such as Codex under your own `workspace-write` setting. Studio checks and rewrites these copies when a tab starts. It does not stop an agent that writes later, and Grok may read an edited MCP config while a session runs [unverified: from Grok's source, not run]. Studio moves aside the sources that Grok's and Antigravity's docs and source name, and a later version of either CLI may read more. Workaround: in a window where an agent can write without asking, do not run a Grok or Antigravity tab in Ask mode. Planned for a later release: run Grok and Antigravity from a folder only that tab can write.
- Gemini CLI in Ask mode reads `tools.allowed` and other settings from `.gemini/settings.json` in the session folder, which an agent can write (#1047). Some keys there skip Gemini's confirmation prompt, and `hooks` and `tools.discoveryCommand` run a command when the next Gemini tab starts. Gemini 0.35 has no switch that ignores workspace settings for one run. Studio rewrites the file when a tab starts, and an Ask-mode Gemini tab refuses to start when the rewrite fails, so a read-only `.gemini` no longer keeps an agent's file. Planned fix: #1059.
- Ask covers agent tabs started from Studio. The `designos` CLI, which you run yourself, and a `designos-mcp` started by another host are not gated by Ask: that host approves its own MCP calls (#1101). A real MCP-level approval gate is planned for a later release.
- Codex, Grok, Antigravity and Gemini CLI ask for approval in their own terminal menu, and the panel shows `Open tab`; Approve and Deny in the panel exist for Claude only (#930). Clicking an option in Codex's own approval menu does nothing, so use the arrow keys and Enter (#1096).
- Gemini CLI and Grok in Ask mode: the status line may not show a waiting state for every approval prompt. For Gemini, the state comes from the confirmation dialog on the tab's screen or from Gemini's `ToolPermission` notification. The `ask_user` and `exit_plan_mode` dialogs draw no `No, suggest changes` option, and a panel narrower than 29 columns hides a cut label, so a wait there starts only from a hook record. The screen check is on for Gemini 0.35.x and 0.65.x, read from source only. For another Gemini version, a No answer leaves `Needs approval` standing until you type a prompt. Where Studio writes no Gemini hooks file (Windows, or Gemini already has system defaults), a wait starts only from the screen. For Grok, the state comes from its `permission_prompt` notification, and Studio never checked that payload against a real Grok [unverified]. No real Gemini or Grok approval prompt was run.
- Export Frame as `.spec.json` writes into `designos_specs` in the temporary folder when Studio finds no source tree, by path, so an agent that can write the temporary folder could plant a link at the file name. It needs you to run the export (#1073).
- The Windows test suite is advisory for 0.6.5. Releases are verified on macOS, and known Windows test failures are tracked in #1158.
- Windows (releases are verified on macOS only):
  - Stop on Windows is not checked by any test. The Stop tests fail there because their Python stub imports `tty`, which Python does not provide on Windows (#1066).
  - Palette auto-fix undo is unverified on Windows: eight tests show different auto-layout sizes after one undo, and the cause is not known (#1066).
  - The link checks for session files are not race-free on Windows (#1061, #1068), an Ask-mode Grok or Antigravity tab refuses an entry that is not Studio's instead of moving it (#1069), and no hooks file is written for Gemini, so its `Needs approval` comes from the screen only (#1058, #1066).
  - Windows was not built or run by hand for these changes.
- Not checked against a live model turn: the Ask-mode Claude launch (skill from a private plugin, Studio's hooks and MCP config from private copies), the Gemini approval state, and the Ask-mode Grok and Antigravity checks were tested with stubs and source reading, not with a real Claude, Gemini, Grok or Antigravity turn.
- Of the 38 GUI fixes from 0.6.1 listed in #1102, 16 were re-checked by tests. The rest are not re-tested yet, and U-17 (a new design file is named "Untitled") is still open (#1102).

## [0.6.4] - 2026-10-27

Not published: the tag was pushed before the version bump, so no packages were built, and its fixes ship in 0.6.5.
````

### 5.2 `README.md` — `## What's new` (`0.6.5`) and `### Known issues`

````markdown
<a id="whats-new"></a>
## What's new

Notes for every release are in the [CHANGELOG](CHANGELOG.md). The newest is 0.6.5. [v0.6.0](https://github.com/jangtrinh/design-os-studio/releases/tag/v0.6.0) has captures from the release build, and older releases are short lists.

<a id="new-in-v065"></a>
### 0.6.5

- **Starter file.** Fresh libraries and upgrades from 0.4.0–0.6.4 seed a `Getting started` project with `DESIGN-OS Studio Starter` (`Starter screens` and `About this file`) without touching existing user files, `Open starter sample` in Home and `Help and account` opens a new copy, launching with no file argument opens Home, and the older sample builders, `--seed-library`, `--fixture` (in release builds) and `examples/` are removed.
- **Variables.** `designos FILE --export-variables OUT.json --format dtcg --mode MODE` exports one variable mode across the file's collections (or `--collection`) as a single self-contained DTCG `.tokens.json` file with default-mode fallback, `--resolve-aliases`, and `--color-format` (`hex` or `object`), and multi-file DTCG exports give same-named local and library collections unique group and file names.
- **Agents panel.** Tabs clear their scrollback on `ESC[3J` so a UI zoom no longer repeats output, quiet Codex turns decay from `Working…` to `Done` after 1.5 s and close the turn in the ledger, and the approval banner shows `Show prompt` to focus the current waiting tab or `Open tab` to switch to another waiting tab.
- **Canvas and `.fig` import.** Frame, component and instance labels repaint after an agent rename and cut to fit with `…` like Figma; hug and fixed-width `NodeSpec` text is measured with the renderer's `cosmic-text` shaper instead of a character-count estimate; Kiwi 64-bit varints decode and encode the ninth byte as a full 8 bits; and resized instances in `.fig` imports scale and apply derived vector geometry while keeping stroke weight.
- **Styles and performance.** Deleted, `REMOVED` and orphan style records are hidden from style pickers, Assets, library publishing, layer chips and MCP listings, while unnamed listed styles show `Unnamed fill style`, `Unnamed text style`, `Unnamed effect style` or `Unnamed grid style` with the raw ID in the tooltip; whole-page and multi-node edits diff only changed nodes using typed field comparisons instead of serializing every touched node to JSON.
- **CLI and MCP.** `--select` accepts `id:<n>`, `fig:<figma id>`, `name:<text>`, `none` or a layer name, supports `--zoom fit`, reports `--inspect-json` `selection` bounds in page coordinates, and exits 2 without writing a PNG or changing the file when a selector matches nothing; Home › `Import` › `Enter a path…` expands `~/` and `file://` paths and starts toast timers on first paint; MCP imports share one 512 MiB `FILE_TOO_LARGE` gate with `safe_to_retry: false`, `designos-mcp` usage errors list `--version`, and MCP file writers return `INVALID_NAME` and `NAME_TOO_LONG` on invalid output names.
````

````markdown
### Known issues

- Saving a `.fig` from Studio drops numbered-list formatting and writes inherited or variable-bound paints on instances as fixed values ([#834](https://github.com/jangtrinh/design-os-studio/issues/834)). Keep the original `.fig`, or save as `.designos`, until it is fixed. v0.5.0 behaves the same.
- On macOS, Quit from the Dock, logout and shutdown still skip the Save changes question ([#977](https://github.com/jangtrinh/design-os-studio/issues/977)). Cmd+Q and Quit in the app menu ask.
- The Windows test suite is advisory for 0.6.5. Releases are verified on macOS, and known Windows test failures are tracked in [#1158](https://github.com/jangtrinh/design-os-studio/issues/1158).
- The macOS app is ad-hoc signed, so the first launch asks for a one-time Open Anyway in System Settings > Privacy & Security.
````

---

## 6. Unresolved Questions

- PRs `#1254` (`#1154`), `#1251` (`#1043`), and `#1256` (`#1242`) are still open against `release/v0.6.5`. Ready to merge `origin/release/v0.6.5` into `jangtrinh/agy-r065-release-notes` as soon as the controller notifies that they have landed.

Status: DONE
