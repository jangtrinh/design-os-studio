---
id: test-plan
type: tdd
title: "Stage 9 TDD & Automated Scenario Verification Plan (SCN-01-01..SCN-15-03)"
status: Passed
updated: 2026-10-15
owner_role: QC
trace:
  us: [US-01, US-02, US-03, US-04, US-05, US-06, US-07, US-08, US-09, US-10, US-11, US-12, US-13, US-14, US-15]
---

This document maps all **46 acceptance scenarios (`SCN-01-01` .. `SCN-15-03`)** of the 15 User Stories (`US-01..US-15`) to automated tests (`cargo test`) and background verification screenshot commands (`--screenshot` / `--parity-report` with `with_active(false)`).

---

## 1. Automated Test Scenario Matrix (`SCN-01-01` .. `SCN-15-03`)

| Scenario ID | User Story & AC | Screen(s) | Method | Status | Automated Test Target / CLI Verification Command |
|---|---|---|---|---|---|
| **SCN-01-01** | `US-01` · `AC-01` | `S01` | `auto` | `passed` | `cargo test -p designos-fig --test fig_roundtrip` (`raw_fields` & Kiwi v106 lossless R/W) |
| **SCN-01-02** | `US-01` · `AC-02` | `S01` | `auto` | `passed` | `cargo test -p designos-app layout::tests` (`LayoutDirection::Grid`, `fr`/`px`/`auto`, `col_span`/`row_span`) |
| **SCN-01-03** | `US-01` · `AC-03` | `S01`, `S02` | `auto` | `passed` | `cargo test -p designos-app specs::tests::summarizes_nodespec_json_metrics` |
| **SCN-02-01** | `US-02` · `AC-01` | `S02` | `auto` | `passed` | `cargo test -p designos-app specs::tests::loads_workspace_spec_packages_and_json_blueprints` |
| **SCN-02-02** | `US-02` · `AC-02` | `S02`, `S03` | `auto` | `passed` | `cargo test -p designos-app specs::tests::formats_copy_cho_ai_citation_markdown` |
| **SCN-02-03** | `US-02` · `AC-03` | `S02` | `auto` | `passed` | `cargo test -p designos-app specs::tests::parses_us_01_frontmatter_and_ac_table_and_ui_contract_and_gates` |
| **SCN-03-01** | `US-03` · `AC-01` | `S03` | `auto` | `passed` | `cargo test -p designos-app terminal::tests` (PTY spawn, ANSI VT100 screen buffer, 120fps non-blocking) |
| **SCN-03-02** | `US-03` · `AC-02` | `S02`, `S03` | `auto` | `passed` | `cargo test -p designos-app terminal::tests` (`last_ai_copy` citation injection into PTY stdin) |
| **SCN-03-03** | `US-03` · `AC-03` | `S01`, `S03` | `auto` | `passed` | `cargo test -p designos-app terminal::tests` (live stdout `NodeSpec` extraction & canvas sync) |
| **SCN-04-01** | `US-04` · `AC-01` | `S04` | `auto` | `passed` | `cargo test -p designos-app home::` (`Drafts`, `All folders`, `Resources`, `Starred`, `Trash` navigation & badges) |
| **SCN-04-02** | `US-04` · `AC-02` | `S04` | `auto` | `passed` | `cargo test -p designos-app home::` (search, type filter, sort order, Grid/List toggle) |
| **SCN-04-03** | `US-04` · `AC-03` | `S04` | `auto` | `passed` | `cargo test -p designos-app shell::tests` (file card context menu: Star, Duplicate, Rename, Trash/Restore) |
| **SCN-05-01** | `US-05` · `AC-01` | `S05` | `auto` | `passed` | `cargo test -p designos-app panels::properties::tests` (`Type settings` 3-tab `Basic \| Details \| Variable` & OpenType) |
| **SCN-05-02** | `US-05` · `AC-02` | `S05`, `S06` | `auto` | `passed` | `cargo test -p designos-app cmdk::tests` & `panels::menus::tests` (Bottom `Cmd+K` & `Zoom/View` `%` input) |
| **SCN-05-03** | `US-05` · `AC-03` | `S07`, `S08` | `auto` | `passed` | `cargo test -p designos-app variables::` & `dev::tests` (`matches_scope` picker & Dev Mode Box Model + 5-lang codegen) |
| **SCN-06-01** | `US-06` · `AC-01` | `S01` | `auto` | `passed` | `cargo test -p designos-app canvas::hit::tests` & `canvas::interact::tests` (17 `NodeKind` hover/select & double-click drill-down) |
| **SCN-06-02** | `US-06` · `AC-02` | `S01` | `auto` | `passed` | `cargo test -p designos-app canvas::handles::tests` & `canvas::snap::tests` (resize, corner radius, pink padding/gap handles, `Alt` redlines) |
| **SCN-06-03** | `US-06` · `AC-03` | `S01` | `auto` | `passed` | `cargo test -p designos-app canvas::grids::tests` & `layout::tests` (dotted 2D CSS Grid cell overlay & track header pills) |
| **SCN-07-01** | `US-07` · `AC-01` | `S01`, `S07` | `auto` | `passed` | `cargo test -p designos-fig` (`fig-kiwi` v106 schema, 653 definitions, `parameterConsumptionMap` id=445) |
| **SCN-07-02** | `US-07` · `AC-02` | `S01` | `auto` | `passed` | `cargo test -p designos-fig --test fig_roundtrip` (`symbolOverrides`, stale binding clear, `overriddenSymbolID` swap) |
| **SCN-07-03** | `US-07` · `AC-03` | `S01`, `S07` | `auto` | `passed` | `cargo test -p designos-app icons::tests` & UI3 token verification in `plans/specs/designos-studio-v2/research/` |
| **SCN-08-01** | `US-08` · `AC-01` | `S01..S13` | `auto` | `passed` | `cargo test -p designos-app cli::tests` (`with_visible(false).with_active(false)` background screenshot capture) |
| **SCN-08-02** | `US-08` · `AC-02` | `S01..S13` | `auto` | `passed` | `cargo test -p designos-app specs::tests::renders_specs_reader_and_left_panel_without_lint_issues` |
| **SCN-08-03** | `US-08` · `AC-03` | `S01..S13` | `auto` | `passed` | `cargo test --workspace` (full workspace regression + 99% Figma UI3 visual parity verification) |
| **SCN-09-01** | `US-09` · `AC-01` | `S09`, `S04` | `auto` | `passed` | `cargo test -p designos-app updater::tests` (`Stable \| Beta \| Nightly` channel manifest check & UI3 Update Pill) |
| **SCN-09-02** | `US-09` · `AC-02` | `S09` | `auto` | `passed` | `cargo test -p designos-app updater::tests` (SHA-256 verification, `.staged` → `.bak` atomic swap & automatic rollback) |
| **SCN-09-03** | `US-09` · `AC-03` | `S09`, `S01` | `auto` | `passed` | `cargo test -p designos-app updater::tests` (passive read-only `/Applications/Figma.app` `Info.plist` & Kiwi `v106` watchdog) |
| **SCN-10-01** | `US-10` · `AC-01` | `S10`, `S04` | `auto` | `passed` | `cargo test -p designos-app tabs::tests` (`36px` Desktop Tab Strip, **`design`:os studio** lowercase bold-prefix wordmark, `Home` tab + project tabs) |
| **SCN-10-02** | `US-10` · `AC-02` | `S10`, `S01` | `auto` | `passed` | `cargo test -p designos-app tabs::tests` (100% per-tab isolation of `Document`, `History` undo/redo, `Page`, `Selection`, `View` zoom/pan & `Dev Mode`) |
| **SCN-10-03** | `US-10` · `AC-03` | `S10` | `auto` | `passed` | `cargo test -p designos-app tabs::tests` (`Cmd+T`, `Cmd+W`, `Ctrl+Tab`, `Cmd+1..9` & context menu `Duplicate tab`, `Pin/Unpin tab`, `Close other tabs`) |
| **SCN-11-01** | `US-11` · `AC-01` | `S03`, `S11` | `auto` | `passed` | `cargo test -p designos-app terminal::tests` (5 CLI Runtime presets `claude`/`codex`/`agy`/`grok`/`$SHELL` & `$DESIGNOS_*` env contract) |
| **SCN-11-02** | `US-11` · `AC-02` | `S06`, `S11` | `auto` | `passed` | `cargo test -p designos-app cmdk::tests` (6 Figma AI Actions in `Actions ⌘K`, `Agents` Rail & Context Menu) |
| **SCN-11-03** | `US-11` · `AC-03` | `S01`, `S11` | `auto` | `passed` | `cargo test -p designos-app terminal::tests` (`<50ms` `$DESIGNOS_LIVE_SPEC` hot-apply, Diff Toast & 1-step `Cmd+Z` atomic undo) |
| **SCN-11-04** | `US-11` · `AC-04` | `S01`, `S05..S10` | `auto` | `passed` | `cargo test -p designos-app` & `designos-fig` (Figma `126.9.13` Functional Gaps A–F closure) |
| **SCN-12-01** | `US-12` · `AC-01` | `S12` | `auto` | `passed` | `python3 scripts/figma_parity_meter.py` (deterministic 5-pillar weighted parity calculation & `research/parity-scorecard.json`) |
| **SCN-12-02** | `US-12` · `AC-02` | `S12` | `auto` | `passed` | `cargo test -p designos-app cli::tests` & `designos --parity-report` (headless CLI 5-pillar parity table output) |
| **SCN-12-03** | `US-12` · `AC-03` | `S02`, `S12` | `auto` | `passed` | `cargo test -p designos-app specs::tests` (Live 5-Pillar Figma Parity Meter card & `Copy Parity Report cho AI`) |
| **SCN-13-01** | `US-13` · `AC-01` | `S13` | `auto` | `passed` | `cargo test -p designos-app panels::left::tests` (`DIR-17` 1:1 Figma UI3 `Agents` Left Side Panel, inline `>_ Terminal` PTY & `#0D99FF` composer) |
| **SCN-13-02** | `US-13` · `AC-02` | `S01`, `S06`, `S07`, `S13` | `auto` | `passed` | `cargo test -p designos-app panels::` & `cmdk::tests` & `dev::tests` (`DIR-18` 9 Visual Deltas verification) |
| **SCN-13-03** | `US-13` · `AC-03..05` | `S01`, `S04`, `S10`, `S13` | `auto` | `passed` | `cargo test -p designos-app shell::tests` (`DIR-18` / `DIR-20` End-to-End Userflows `UF-01..UF-08` state machine verification) |
| **SCN-14-01** | `US-14` · `AC-01` | `S01..S13` | `auto` | `passed` | `cargo test -p designos-app icons::tests::every_phosphor_svg_parses_cleanly` (`DIR-19` 100% `Icon` variants map to valid `@phosphor-icons/core` `256×256` SVGs) |
| **SCN-14-02** | `US-14` · `AC-02` | `S01..S13` | `auto` | `passed` | `cargo test -p designos-app icons::tests::every_phosphor_svg_rasterizes_nonzero_alpha_at_standard_sizes` (DPR-aware `resvg` rasterization & `round_to_pixel` snapping) |
| **SCN-14-03** | `US-14` · `AC-03` | `S01..S13` | `auto` | `passed` | `cargo test -p designos-app icons::tests` & `specs::tests` (100% unified Phosphor Icons across all studio surfaces, preserving `Icon::Logo`) |
| **SCN-15-01** | `US-15` · `AC-01` | `S13`, `S03` | `auto` | `passed` | `cargo test -p designos-app terminal::tests` (`DIR-21` 1-click CLI runtime selector pill bar `Claude Code \| Codex \| Antigravity` spawning/switching `claude`/`codex`/`agy` PTY shells) |
| **SCN-15-02** | `US-15` · `AC-02` | `S13`, `S03` | `auto` | `passed` | `cargo test -p designos-app terminal::tests` (`AgentChatTurn` / `AgentChatRole` live PTY stdout synchronization via `sync_chat_from_pty` + `strip_ansi_codes` & `+` Selection/Comments attachment) |
| **SCN-15-03** | `US-15` · `AC-03` | `S13`, `S03`, `S01` | `auto` | `passed` | `cargo test -p designos-app terminal::tests` (inline `NodeSpec` detection & 1-click/auto Canvas apply badge inside chat bubbles + instant Chat/VT100 terminal toggle) |

---

## 2. TDD Process & Non-Intrusive Background Verification (`with_active(false)`)

1. **Red → Green → Refactor**: Every new behaviour in `ui-contract.md` must be declared as `SCN-xx-yy` before the UI code is written, and verified with `cargo test`.
2. **Background proof screenshots & Update / Parity checks (Zero Focus Stealing)**:
   ```bash
   cargo run --release -p designos-app -- "/Users/jangtrinh/Downloads/VSF - PCP.fig" --screenshot /tmp/designos_s01_canvas.png
   cargo run --release -p designos-app -- --panel specs --screenshot /tmp/designos_s02_specs.png
   cargo run --release -p designos-app -- --panel terminal --screenshot /tmp/designos_s03_terminal.png
   cargo run --release -p designos-app -- --parity-report
   cargo run --release -p designos-app -- update --check
   ```
   All the commands above run with `ViewportBuilder::default().with_visible(false).with_active(false)` so a window never appears over the user's screen.

