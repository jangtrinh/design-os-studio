---
id: glossary
type: glossary
title: "Domain Glossary & Banned Wording"
updated: 2026-10-15
---

## Terms

| Term | Anchor | Definition | Source |
|---|---|---|---|
| Spec Format v2 | spec-format-v2 | Canonical 11-stage specification bundle (`spec.manifest.json`, `pipeline.yaml`, `po-spec.md`, `clarifications.md`, `FC-001.md`, `PRD.md`, `US-01..14.md`, `W-01.md`, `P-01.md`, `tokens.md`, `test-plan.md`, `ui-contract.md`, `READY.md`, `review/gates/*.md`). | D3, D5 |
| Copy for AI | copy-cho-ai | Deterministic Markdown citation block carrying exact source file path, line range, section breadcrumb, reader view, and project revision. | D4 |
| UI Contract | ui-contract | 12-column behavioral selector table mapping `Screen × State` (`S01..S13` × 6 states = 78 rows) to `US`, `Element`, `Components`, and `SCN`. | D5 |
| Kiwi v106 | kiwi-v106 | Figma binary container format (`fig-kiwi` header + DEFLATE schema + Zstd/DEFLATE message) preserving unknown schema fields in `raw_fields` across load/save cycles. | D1, D7 |
| 2D CSS Grid | css-grid-2d | Figma 2025+ Auto-Layout mode (`LayoutDirection::Grid`, `stackMode: "GRID"`) supporting `fr`, `px`, `auto` track sizing and `col_span` / `row_span` placement. | D1, D6 |
| Figma UI3 Tokens | figma-ui3-tokens | Canonical sRGB dark theme palette of `com.figma.Desktop` (`#1E1E1E` canvas, `#2C2C2C` panel, `#383838` input well, `#444444` border, `#0D99FF` selection blue, `#9747FF` component purple, `#24CB71` Dev Mode green). | D6 |
| REA Pipeline | rea-pipeline | Static and binary reverse-engineering workflow (`morluto/rea`) inspecting `/Applications/Figma.app` (`app.asar`) and `.fig` archives to document Kiwi schemas and UI3 tokens in `research/`. | D7 |
| Headless Background Capture | headless-capture | Non-intrusive verification mode (`with_visible(false).with_active(false)`) that renders `--screenshot` PNGs in the background without stealing macOS window focus. | D8 |
| Dual-Layer Auto-Update | dual-layer-auto-update | Two-tier evergreen update engine (`updater.rs` + `designos update`): Layer 1 atomic self-updater with SHA-256 verification and `.bak` rollback; Layer 2 passive read-only `/Applications/Figma.app` (`Info.plist` / `app.asar` / Kiwi `v106`) schema watchdog. | D9 |
| **design**:os studio Wordmark | brand-wordmark | Canonical brand typography rule: always all-lowercase **`design`:os studio** (or **`design`:os**) with **`design`** rendered in Bold (`700`) and `:os studio` rendered in Regular (`400`). | D10 |
| Multi-Tab Project Workspace | multi-tab-workspace | Top `36px` Figma UI3 Desktop Tab Strip (`S10`) managing multiple simultaneously open projects (`ProjectTab`) with isolated `Document`, `History` undo/redo, `Page`, `Selection`, `View` zoom/pan, `Dev Mode`, dirty dot `•`, and pin state. | D11 |
| Hybrid Dual-Mode Figma AI Suite | figma-ai-suite | 6 canonical Figma UI3 AI actions (`Make Design`, `Make Prototype`, `Rename Layers`, `Rewrite/Translate/Shorten`, `Populate Realistic Data`, `Design Gate Audit & Auto-Fix`) backed by 5 Native PTY CLI Runtimes (`claude`, `codex`, `agy`, `grok`, `$SHELL`) and `<50ms` live `.spec.json` hot-apply (`S11`). | D12, D13 |
| 5-Pillar Figma Parity Meter | figma-parity-meter | Deterministic weighted benchmark (`0.25*P1 + 0.20*P2 + 0.20*P3 + 0.20*P4 + 0.15*P5`) audited by `scripts/figma_parity_meter.py`, stored in `research/parity-scorecard.json`, and displayed in `S12` / `--parity-report`. | D14 |
| Figma UI3 `Agents` Left Side Panel | agents-side-panel | 1:1 Figma UI3 `Agents` panel (`S13`, `240px–320px`, 2nd item on Left Rail below `File`) with `< New chat / Shared` + `[AI]` + `[↗]` + `•••`, `#26354A` credit banner, `"What's next?"`, 3 suggestion groups, inline VT100 PTY `>_ Terminal`, and `#0D99FF` `"Ask for changes"` composer (`DIR-17`, `DIR-18`). | D15, D16 |
| Unified Phosphor Icons System | phosphor-icons-system | Official `@phosphor-icons/core` (`https://phosphoricons.com/`, `viewBox="0 0 256 256"`, `fill="currentColor"`) SVG icon suite vendored in `crates/designos-app/assets/icons/phosphor/*.svg` and rasterized via `resvg` with DPR-aware `(Icon, physical_px)` caching and `round_to_pixel` snapping across all `crate::icons::Icon` variants (preserving `Icon::Logo` for the brand mark) (`DIR-19`). | D17 |
| Agent Chat Wrapper | agent-chat-wrapper | Pure native Chat UI wrapper (`AgentChatTurn` / `AgentChatRole`) in `RailItem::Agents` (`S13`) layered directly on top of the active `portable-pty` + `vt100` shell of `Claude Code (claude)`, `Codex (codex)`, and `Antigravity CLI (agy)` with 1-click CLI runtime selector pills, `sync_chat_from_pty` + `strip_ansi_codes` live stdout streaming, inline `NodeSpec` detection & 1-click/auto Canvas apply badge, `+` Selection + Pin-Comment context attachment, and instant toggle between Chat thread view and raw VT100 terminal screen (`DIR-21`). | D18 |
| Cross-Platform Release Packaging | cross-platform-release-packaging | Unified installer & release packaging workflow (`scripts/build-release.sh`, `scripts/package-macos.sh`, `scripts/package-windows.ps1`, `scripts/package-linux.sh`, `.github/workflows/release.yml`) producing `.app`/`.dmg` (macOS), NSIS `-setup.exe`/`install.ps1` (Windows), and `.deb`/`.tar.gz` (Linux) with `dist/release-manifest.json` + `SHA256SUMS` (`DIR-22`). | D19 |
| 16-Capture Visual Proof Suite | visual-proof-suite | Deterministic 16-capture Retina 2x (`3200×2000`) headless screenshot suite (`scripts/capture-visual-proof.sh` → `docs/assets/screenshots/*.png`) covering `S01..S13` + 3 Spec Reader Role Views (`PO View`, `Dev View`, `QA & Coverage`), showcased in the `design-os-github-page` 12-Section AI-Ready `README.md` and 9-Tier `docs/index.html` GitHub Pages site (`DIR-23`). | D20 |

## Ban list

| Banned | Replacement | Reason |
|---|---|---|
| Emoji icons in UI chrome | `crate::icons::Icon` unified Phosphor Icons (`@phosphor-icons/core` SVGs) | Violates `es:designer` craft-floor §2 & `BR-22` (`DIR-19`) |
| Mixed icon families (Lucide/Feather/custom paths) or ad-hoc procedural chrome icons | Official Phosphor Icons (`https://phosphoricons.com/`) in `crates/designos-app/assets/icons/phosphor/*.svg` via `icons::paint` | Violates `BR-22` (`D17` / `DIR-19`) icon consistency rule |
| Raw unstripped ANSI escape codes or cloud-only non-PTY mock chat inside `Agents` (`S13`) | `AgentChatTurn` / `AgentChatRole` wrapping live PTY stdout via `sync_chat_from_pty` + `strip_ansi_codes` | Violates `BR-23` (`D18` / `DIR-21`) pure PTY chat wrapper rule |
| Nested cards-in-cards | `1px` hairline divider (`#2A2B31` / `#3C3C3C`) or `#26272C` table header row | Violates `es:designer` craft-floor §2 |
| Unverified "seamless" claim | Cite concrete `SCN-xx` test and `CURRENT` gate digest | No unmeasured claims |
| Focus-stealing screenshot window | `ViewportBuilder::default().with_visible(false).with_active(false)` | Violates `BR-11` (`D8`) user screen protection rule |
| Unverified in-place binary overwrite | Atomic `.staged` download + `SHA-256` verify + `.bak` backup before rename | Violates `BR-13` (`D9`) zero-brick update safety rule |
| Uppercase or title-case brand (`DESIGN:OS Studio`, `Design OS Studio`, `DESIGN:OS`) | All-lowercase **`design`:os studio** (bold **`design`**, regular `:os studio`) | Violates `BR-14` (`D10`) brand typography directive |
| Shared global Undo/Selection across open tabs | Per-tab `ProjectTab` state isolation (`Document`, `History`, `Page`, `Selection`, `View`) | Violates `BR-15` (`D11`) multi-tab project isolation rule |

