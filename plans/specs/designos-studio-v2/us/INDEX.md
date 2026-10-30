---
id: us-index
type: us-index
title: "US Spec Index — design:os studio v2 (US-01..US-18)"
status: Verified
updated: 2026-10-16
label: Story index
order: 0
---

Each file defines one independent User Story. `REQ` IDs link directly to the [PRD](../prd/PRD.md#4-requirements-and-rules-req-01req-18) and are collected in the [Master Backlog](../../../BACKLOG.md).

### E1 Figma 1:1, Multi-Tab Projects, Spec Reader v2, REA, Auto-Update, AI Agent Suite, Parity Scorecard, Unified Phosphor Icons, Pure PTY Chat Wrapper, 1M-Node Performance & Micro-Interactions (`US-01..US-18`)

| File | Story | Epic | REQ | Screens | Scenarios |
|---|---|---|---|---|---|
| [US-01.md](US-01.md) | 1:1 Figma `.fig` v106 Binary R/W & 2D CSS Grid | `EP-02` | `REQ-01, REQ-02` | `S01, S02, S03` | `SCN-01-01..03` |
| [US-02.md](US-02.md) | PRODUCT:OS Spec Driven v2 Reader & Role Views | `EP-07` | `REQ-03, REQ-04` | `S01, S02, S03` | `SCN-02-01..03` |
| [US-03.md](US-03.md) | Embedded PTY Agent Terminal `claude`/`codex` & Live Canvas Bridge | `EP-06` | `REQ-05` | `S01, S02, S03` | `SCN-03-01..03` |
| [US-04.md](US-04.md) | 1:1 Figma UI3 Home & File Management (`Drafts`, `All folders`, `Resources`, `Trash`, `Starred`) | `EP-05` | `REQ-06` | `S04` | `SCN-04-01..03` |
| [US-05.md](US-05.md) | 1:1 Figma UI3 Settings, Menus & Popovers (`Type settings`, `Cmd+K`, `Zoom/View`, `Color/Variables`, `Dev Mode`) | `EP-04` | `REQ-07` | `S05, S06, S07, S08` | `SCN-05-01..03` |
| [US-06.md](US-06.md) | 1:1 Figma Per-Element Canvas Interactions & 2D Grid Overlay (17 Node Kinds) | `EP-03` | `REQ-08, REQ-01` | `S01` | `SCN-06-01..03` |
| [US-07.md](US-07.md) | Figma Desktop (`app.asar`) & `.fig` Kiwi Binary Reverse Engineering (REA) Pipeline | `EP-01` | `REQ-09` | `S01, S07` | `SCN-07-01..03` |
| [US-08.md](US-08.md) | Headless Background Screenshot Verification (`with_active(false)`) & 99% Parity Gate | `EP-08` | `REQ-10` | `S01..S13` | `SCN-08-01..03` |
| [US-09.md](US-09.md) | Dual-Layer Auto-Update (Atomic Self-Updater `updater.rs` + Upstream Figma `app.asar`/Kiwi Watchdog) | `EP-09` | `REQ-11` | `S09, S04, S01` | `SCN-09-01..03` |
| [US-10.md](US-10.md) | Multiple Tab Project Workspace & Figma UI3 Desktop Tab Strip (`36px` Tab Bar, Per-Tab Isolation & Brand Wordmark) | `EP-10` | `REQ-12` | `S10, S04, S01` | `SCN-10-01..03` |
| [US-11.md](US-11.md) | 1:1 Figma AI Agent Suite & Native PTY Terminal Core (`Claude`, `Codex`, `Antigravity`, `Grok`) | `EP-11` | `REQ-13, REQ-03` | `S03, S06, S11` | `SCN-11-01..04` |
| [US-12.md](US-12.md) | Quantitative 1:1 Figma Parity Benchmark & Live Telemetry Scorecard (`%` match with Figma meter) | `EP-12` | `REQ-14, REQ-04` | `S12, S02, S03` | `SCN-12-01..03` |
| [US-13.md](US-13.md) | 1:1 Figma UI3 `Agents` Left Side Panel (`DIR-17`), 9 Visual Deltas & 5 End-to-End Userflows Audit (`UF-01..UF-05` · `DIR-18`) | `EP-13` | `REQ-15, REQ-16` | `S13, S01, S03, S04, S06, S07, S10, S11` | `SCN-13-01..03` |
| [US-14.md](US-14.md) | Unified Phosphor Icons System Across Entire Studio (`@phosphor-icons/core` SVG Asset Pipeline & DPR-Aware Raster Cache · `DIR-19`) | `EP-14` | `REQ-17, REQ-07` | `S01..S13` | `SCN-14-01..03` |
| [US-15.md](US-15.md) | Pure Chat UI Wrapper Over Native PTY Terminal Shell (`Claude Code`, `Codex`, `Antigravity CLI` · `DIR-21`) | `EP-15` | `REQ-18, REQ-15, REQ-05` | `S13, S03, S01` | `SCN-15-01..03` |
| [US-16.md](US-16.md) | 1:1 Figma C++/WASM 1M-Node Performance Engine (`6.25s` Load, Sub-ms Hit-Test/Drag/Layout · `DIR-25`) | `EP-16` | `REQ-01, REQ-08, REQ-09` | `S01, S02, S12` | `SCN-16-01..03` |
| [US-17.md](US-17.md) | 1:1 Figma Micro-Interactions, Keyboard Stepping, Scrubbing & Direct Manipulation (`MI-01..MI-20`) | `EP-03` | `REQ-21, REQ-08` | `S01, S02, S03, S04` | `SCN-17.1..17.6` |
| [US-18.md](US-18.md) | Deep Figma Feature & Micro-Interaction Parity Wave 3 (On-Canvas Direct Manipulation Handles, Multi-Edit, Variable Expressions & Conditional Prototyping · `DIR-30`) | `EP-18` | `REQ-01, REQ-07, REQ-08, REQ-09` | `S01, S05, S07, S08` | `SCN-18-01..03` |
