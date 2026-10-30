---
id: brainstorm-figma-ai-terminal-core
type: brainstorm
title: "es:brainstorm — 1:1 Figma AI Agent Surface + Native PTY Terminal Core (design:os studio)"
status: Approved
updated: 2026-10-15
owner_role: Product Owner
decisions: [A1, A2, A3, D12, D13]
---

# Phase 1: Product Brainstorm — 1:1 Figma AI Agent Suite Powered by Native PTY Terminal Core

> **Product:** **`design`:os studio** (always all-lowercase, bold **`design`**, regular `:os studio`)  
> **Feature Slug:** `designos-studio-v2` / `EP-11` (`US-11` · `REQ-13` · `S03`, `S06`, `S11`)  
> **Gate Status:** `es:brainstorm` Gate Cleared (`A1`, `A2`, `A3` Approved by Owner)

---

## 1. 6-Step Research Protocol & Evidence Synthesis

1. **Upstream Figma Desktop (`126.9.13`) Reverse-Engineering (`morluto/rea`)**:
   - Inspected `/Applications/Figma.app/Contents/Resources/app.asar` and `/Users/jangtrinh/Downloads/VSF - PCP.fig` (`fig-kiwi` container `v106`, 653 Kiwi definitions, 155,297 `NodeChange` records).
   - Identified Figma UI3's exact AI entry points: bottom-docked **`Actions (⌘K)`** popover (`All | AI | Actions | Libraries`), contextual selection AI menu, and 6 core AI actions (`Make Design / First Draft`, `Make Prototype`, `Rename Layers`, `Rewrite / Translate / Shorten Text`, `Populate Content`, `Design Gate Audit & Auto-Fix`).
2. **Existing Native PTY & Spec Architecture in `design:os studio`**:
   - `crates/designos-app/src/terminal.rs` provides a hardware-accelerated 120fps `portable-pty` + `vt100` TrueColor terminal emulator.
   - `crates/designos-app/src/cmdk.rs` and `specs.rs` provide the bottom-docked `Actions (⌘K)` palette and `NodeSpec` (`.spec.json`) declarative scene applicator (`apply_node_spec`).
3. **Key Architectural Insight (Why Native PTY Beats Cloud-Locked AI Wrappers)**:
   - Cloud-only AI wrappers lock designers into a single vendor API and cannot read the local repository, design system rules, or `.brv/` context tree.
   - By routing Figma UI3's 1:1 visual AI actions (`⌘K`, `Agents` Left Rail, Context Menu AI) directly into an active **Native PTY CLI Runtime** (`Claude Code`, `Codex`, `Antigravity (agy)`, `Grok`, `$SHELL`) with auto-injected `$DESIGNOS_*` environment variables and a `<50ms` live file watcher (`$DESIGNOS_LIVE_SPEC`), users get both 1-click Figma UI3 ergonomics AND full local coding-agent intelligence with zero extra API billing.

---

## 2. Approved Owner Decisions (`A1` .. `A3`)

| Decision ID | Decision Summary | Architectural Contract |
|---|---|---|
| **A1** | **Hybrid Dual-Mode Architecture (`D12`)** | Preserve 1:1 Figma UI3 AI surfaces (`Actions ⌘K`, `Agents` Left Rail, Context Menu AI: `Make Design / First Draft`, `Make Prototype`, `Rename Layers`, `Rewrite / Translate / Shorten`, `Populate Content`, `Design Gate Audit & Fix`) that automatically execute AND pipe structured prompts + live selection context into the active PTY Terminal session (`Claude Code`, `Codex`, `Antigravity (agy)`, `Grok`, `$SHELL`), alongside the interactive PTY Terminal where users log in (`claude`, `codex`, `agy`, `grok`) and run commands directly. |
| **A2** | **Live Hot-Apply (`<50ms`) + 1-Step Undo (`Cmd+Z`) + Diff Toast (`D13`)** | Auto-inject environment variables (`$DESIGNOS_BIN`, `$DESIGNOS_FILE`, `$DESIGNOS_MCP_BIN`, `$DESIGNOS_LIVE_SPEC`, `$DESIGNOS_SELECTION_JSON`) and a live file-watcher into the PTY runtime; whenever `Claude Code` / `Codex` / `Antigravity` writes a spec/patch or invokes CLI/MCP, the Canvas hot-applies it in `<50ms` as a single Undoable `History` step (`Cmd+Z`) with a diff toast notification. |
| **A3** | **Full 6-Action Figma AI Suite + 5 First-Class CLI Runtimes (`D12`, `D13`)** | Ship all 6 Figma AI Actions (`Make Design (First Draft)`, `Make Prototype (Auto-Flow Wiring)`, `Rename Layers`, `Rewrite / Translate / Shorten Text`, `Populate Realistic Data`, `Design Gate Audit & Auto-Fix`) and 5 CLI Runtime Presets (`Claude Code` `claude`, `Codex` `codex`, `Antigravity` `agy`, `Grok` `grok`, `$SHELL` `zsh`/`bash`) with live `INSTALLED` / `ACTIVE PTY` badges. |

---

## 3. Working Backwards — Amazon Press Release

### **design**:os studio Unveils 1:1 Figma AI Agent Suite Powered by a Native 120fps PTY Terminal Core for Claude Code, Codex, Antigravity, and Grok

**HO CHI MINH CITY — October 15, 2026** — Today, **`design`:os studio** announced the general availability of its **1:1 Figma AI Agent Suite & Native PTY Terminal Core (`S11` / `US-11`)**, uniting Figma UI3's familiar `Actions (⌘K)` AI workflows with a native Rust `portable-pty` terminal running local subscription CLI agents (`claude`, `codex`, `agy`, `grok`, and `$SHELL`).

For years, product teams faced a painful split: visual AI tools inside design apps could generate throwaway mockups but had zero awareness of the team's production Rust/TypeScript codebase, while terminal coding agents (`Claude Code`, `Codex`, `Antigravity`, `Grok`) understood the entire repository but could not directly manipulate a 120fps `.fig` v106 canvas.

With **`design`:os studio**, clicking **Make Design (First Draft)**, **Rename Layers**, or **Design Gate Audit & Auto-Fix** in the bottom-docked `Actions (⌘K)` bar (`S06`) or `Agents` Left Rail (`S11`) both executes instant deterministic canvas transformations AND pipes the live selection's `.spec.json` / `.fig` v106 context directly into the active PTY agent session (`S03`). When the CLI agent writes back to `$DESIGNOS_LIVE_SPEC` or calls `$DESIGNOS_MCP_BIN`, **`design`:os studio** hot-applies the diff onto the active project tab in under `50ms` as a single atomic `Cmd+Z` undo step—with a non-intrusive diff toast showing exactly which nodes changed.

> *"We wanted 100% of Figma UI3's speed and muscle memory—`⌘K` First Draft, semantic layer renaming, instant copywriting, and auto-prototype wiring—powered by the real CLI agents engineers already authenticate with in their terminal,"* said the Product Owner of **`design`:os studio**. *"No separate token subscriptions, no black-box cloud lock-in, and every AI mutation is reversible with a single `Cmd+Z`."*

---

## 4. User Stories Overview (`US-11` & Linked Stories)

1. **`US-11` (Primary — P0)**: As a **Product Designer, Systems Architect, and AI Engineer**, I want 1:1 Figma UI3 AI surfaces (`Actions ⌘K`, `Agents` Left Rail `S11`, and right-click Context Menu AI) wired to a 5-runtime Native PTY Terminal Core (`claude`, `codex`, `agy`, `grok`, `$SHELL`) with `<50ms` live hot-apply and 1-step `Cmd+Z` undo, so I can trigger Figma AI actions in 1 click or collaborate interactively with my authenticated CLI agents on the live `.fig` v106 canvas.
2. **`US-03` (Linked — P0)**: Embedded PTY Agent Terminal & `Copy for AI` Spec Reader citation bridge (`state.last_ai_copy`).
3. **`US-05` (Linked — P0)**: 1:1 Figma UI3 Chrome, `Type settings` per-tab filtering, Color Picker WCAG contrast bar, Dev Mode 5-tool bottom dock (`Select | Annotate | Measure | Comment | Actions`) + `Re-center ⇧2`, and Variables `/` group headers.
4. **`US-10` (Linked — P0)**: Multiple Tab Project Workspace (`36px` Desktop Tab Strip `S10`) with green `</>` Dev Mode tab badge and zero-clone per-frame tab sync (`es:rust` Law 10).
5. **`US-14` (Linked — P0 · `DIR-19`)**: Unified **Phosphor Icons** System (`https://phosphoricons.com/` · `@phosphor-icons/core` `viewBox="0 0 256 256"`, `D17`, `BR-22`) across all `Icon` enum variants, Left Rail, Left Side Panels (`Layers`, `Agents`, `Assets`), Bottom Floating Toolbar & Sub-Toolbar, Top Multi-Tab Bar, Right Inspector, `Actions (⌘K)`, Menus, Popovers, Home Screen, and `PRODUCT:OS Spec Driven v2` Reader.
6. **`US-15` (Linked — P0 · `DIR-21`)**: Pure Chat UI Wrapper Over Native PTY Terminal Shell (`Claude Code` `claude`, `Codex` `codex`, `Antigravity CLI` `agy` · `D18`, `BR-23`) inside `RailItem::Agents` (`S13`) with 1-click CLI runtime selector pill bar (`Claude Code | Codex | Antigravity`), structured User/Assistant chat turn bubbles (`AgentChatTurn` / `AgentChatRole`) streaming ANSI-stripped stdout (`sync_chat_from_pty` + `strip_ansi_codes`), inline `NodeSpec` detection & 1-click/auto Canvas apply badge inside chat bubbles, Selection + Pin-Comment context attachment (`+`), and instant toggle between the clean Chat thread view and the raw VT100 terminal screen.

---

## 5. MoSCoW Prioritization

| Tier | Scope Items |
|---|---|
| **Must Have** | • **6 Figma UI3 AI Actions** (`Make Design / First Draft`, `Make Prototype`, `Rename Layers`, `Rewrite / Translate / Shorten Text`, `Populate Realistic Data`, `Design Gate Audit & Auto-Fix`) accessible from `Actions (⌘K)` (`S06`), `Agents` Left Rail (`S11`), and Canvas Context Menu.<br>• **5 First-Class CLI Runtime Presets** (`Claude Code` `claude`, `Codex` `codex`, `Antigravity` `agy`, `Grok` `grok`, `$SHELL`) with binary auto-detection (`PATH` probe) and `INSTALLED` / `ACTIVE PTY` pills.<br>• **Auto-Injected Environment Contract**: `$DESIGNOS_BIN`, `$DESIGNOS_FILE`, `$DESIGNOS_MCP_BIN`, `$DESIGNOS_LIVE_SPEC`, `$DESIGNOS_SELECTION_JSON`.<br>• **Live Hot-Apply (`<50ms`) + 1-Step Undo (`Cmd+Z`) + Diff Toast**: File watcher on `$DESIGNOS_LIVE_SPEC` + stdout `NodeSpec` stream.<br>• **All-Lowercase Bold-Prefix Brand Wordmark**: Always render **`design`:os studio** (bold **`design`**, regular `:os studio`).<br>• **100% Unified Phosphor Icons System (`DIR-19` / `BR-22` / `D17`)**: Vendor official `@phosphor-icons/core` SVGs (`viewBox="0 0 256 256"`, `fill="currentColor"`) in `crates/designos-app/assets/icons/phosphor/*.svg` and render every UI icon via `resvg` with DPR-aware pixel snapping while preserving `Icon::Logo`.<br>• **Pure Chat UI Wrapper Over Live PTY Shell (`DIR-21` / `BR-23` / `D18`)**: Wrap `Claude Code (claude)`, `Codex (codex)`, and `Antigravity CLI (agy)` PTY sessions directly inside `S13` (`RailItem::Agents`) as structured User/Assistant chat bubbles (`AgentChatTurn` / `AgentChatRole`) streaming ANSI-stripped stdout (`sync_chat_from_pty` + `strip_ansi_codes`) with inline `NodeSpec` apply badges and instant Chat/Terminal toggle. |
| **Should Have** | • Prompt history & reusable team prompt templates inside `Actions (⌘K) -> AI` tab.<br>• One-click `Paste Spec Citation` button in `S03`/`S11` injecting `state.last_ai_copy` from the Spec Driven v2 Reader (`S02`). |
| **Could Have** | • Side-by-side before/after node count badge on the hot-apply diff toast (`+14 nodes · 3 styles updated · Undo ⌘Z`). |
| **Won't Have (Now)** | • Proprietary paid cloud LLM proxy or credit-metering billing wall (users use their own authenticated local CLI runtimes `claude`, `codex`, `agy`, `grok` at zero extra markup).<br>• Multi-user cloud WebSocket CRDT relay server (deferred to Wave 8 per `FC-001`). |

---

## 6. Appetite, Boundaries & No-Gos

- **Appetite**: Wave 4–7 sprints — deterministic Rust implementation inside `crates/designos-app/src/{icons.rs,cmdk.rs,terminal.rs,app.rs,panels/}` backed by unit/integration tests (`SCN-11-01..SCN-15-03`).
- **Hard No-Gos**:
  1. **No Cloud API Key Lock-In Required for Core Actions**: All 6 Figma AI Actions and the `S13` Pure Chat UI Wrapper must work over local PTY shells (`claude`, `codex`, `agy`) and deterministic local fallbacks without paid cloud API keys.
  2. **No Un-Undoable Canvas Mutations**: An AI or PTY hot-apply must never mutate `Document.nodes` without pushing a single coalesced `History` snapshot first (`Cmd+Z` restores exact prior state).
  3. **No Focus-Stealing Windows**: All automated `--screenshot` verification of `S03`, `S06`, `S11`, and `S13` must run with `with_visible(false).with_active(false)`.
  4. **No Uppercase Brand Strings**: Never display `"DESIGN:OS Studio"` or `"Design OS Studio"`; always use **`design`:os studio** (all-lowercase with bold **`design`**).
  5. **No Mixed Icon Families or Ad-Hoc Procedural Chrome Icons (`DIR-19`)**: Never mix legacy Lucide icons or ad-hoc `painter.line_segment`/`circle_stroke` chrome icons with Phosphor Icons; all non-logo `Icon` variants must load from `assets/icons/phosphor/*.svg`.
