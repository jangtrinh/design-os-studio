# Figma gap list: design-os-studio vs Figma (Oct 2026)

Inputs: `figma-catalog-2026-10.md` (Sonnet, official sources up to 2026-10-15) and `inventory-design-os-studio.md` (Haiku scan). The diff was done by Sonnet with rg verification. The controller spot-checked both P0 rows (FACT).

Scope: Figma Design, Dev Mode, Draw, Motion, Slides, FigJam, Sites. Admin and Weave are skipped.

Priority:
- P0: daily pro use and a parity blocker for an offline-first solo/small team.
- P1: frequent.
- P2: nice to have.
- P3: skip (cloud-only or out of scope).

Effort: S < 1 day · M 1–3 days · L 1–2 weeks · XL multi-week. Priority and effort are estimates (INFERENCE).

## Summary
Core 2024-era Figma Design is mostly present. The gaps cluster in two places:
- the 2025–26 additions: Draw, Motion, slot settings, grid 2.0, the agent surface, Code Connect, extended collections;
- ecosystem and interop: the Figma plugin runtime, Sketch, PPTX.

| Area | HAVE | PARTIAL | MISSING |
|---|---|---|---|
| Canvas & tools | 5 | 3 | 2 |
| Layers & selection | 5 | 2 | 0 |
| Shapes/vector/Draw | 8 | 0 | 11 |
| Text | 5 | 2 | 2 |
| Auto layout/grid | 5 | 1 | 2 |
| Components/slots | 4 | 2 | 1 |
| Styles & variables | 4 | 0 | 5 |
| Effects & paints | 5 | 1 | 4 |
| Prototyping | 4 | 1 | 5 |
| Motion | 0 | 0 | 5 |
| Dev Mode/Code Connect/MCP | 6 | 3 | 4 |
| Collab | 2 | 0 | 3 |
| Import/export | 4 | 1 | 5 |
| Files/libraries/versions | 3 | 0 | 3 |
| Plugins/API | 1 | 0 | 4 |
| AI | 2 | 2 | 3 |
| FigJam | 4 | 0 | 7 |
| Slides | 4 | 1 | 3 |
| Sites | 0 | 0 | 5 |

## Gaps by priority

### P0
- **Effect styles and grid styles: MISSING (M).** `designos-app/src/styles.rs:25` `style_entries` covers Text and Fill only (controller-verified). `LayoutGrid` is modeled at `paint.rs:520`.
- **PDF export of frames/pages: PARTIAL (S).** `designos-export/src/lib.rs:42` `ExportFormat` is Png/Jpg/Svg. `export_pdf` is wired only from `decks/export.rs:32` (controller-verified).

### P1
- **Slots, authoring side: PARTIAL (L).** `PropKind::Slot` exists (`node.rs:207`, `components.rs:346-382`, `fig_export.rs:916-939`), but there are no settings (min/max/preferred), no slot child frame and no `SlotNode`.
- **Check designs: PARTIAL (M).** Extend the design-gate audit (`cmdk.rs:894`) with a scan for hard-coded values vs variables/styles, WCAG checks and one-click fixes.
- **Vertical wrap: PARTIAL (S).** The UI only sets horizontal wrap (`properties.rs:1036-1040`). The engine is axis-generic, but only a horizontal test exists.
- **Auto spacing Around / Evenly: MISSING (S).** `LayoutAlign` (`node.rs:610-617`) has no SpaceAround/SpaceEvenly.
- **CSS-aligned auto layout version flag: MISSING (M).** Semantics unknown, so reverse-engineer first.
- **Grid 2.0: PARTIAL (M).** `grid_col_tracks: [GridTrack; 8]` is a hard cap. Auto-position, auto rows and track drag-reorder are missing.
- **Text wrap Balance/Pretty: MISSING (S).** Line breaker change in `textmeasure.rs`.
- **Progressive blur: MISSING (M).** Needs both the CPU and the GPU path.
- **Variable code syntax: MISSING (S).** Only an empty `codeSyntax {}` is written (`fig/variables.rs:182`).
- **Composed color opacity variables: MISSING (S).** `variables_apply.rs:9` handles percent only.
- **Codegen for Vue/HTML/Web Components/Flutter: PARTIAL (S).** Currently 5 targets.
- **Slides PPTX import/export: MISSING (L).**
- **Figma Plugin API runtime: MISSING (XL).** Ecosystem lock-in. A strategic decision is needed early.

### P2
- **Vector/Draw:**
  - variable-width/dynamic stroke (L);
  - brushes (XL);
  - repeaters/transform groups (L);
  - eraser (M; paint bucket already exists);
  - cut/offset path (M);
  - text on path (L);
  - pattern fill (M).
- **Effects and color:**
  - custom shaders (XL);
  - extended collections (L);
  - color spaces P3/OKLCH (L).
- **Dev Mode:**
  - real Code Connect (L; today a stub at `inspect.rs:499-538`);
  - focus view (M);
  - MCP parity with Figma's 41 tools (L).
- **Motion:**
  - timeline, keyframes and animation styles (XL);
  - Lottie/MP4/WebM/GIF/React export (XL; no encoder today).
- **Workspace and versions:**
  - Draw/Motion toolbar modes (M);
  - branching and merging (XL).
- **Import:** Sketch import (L); web capture (L).
- **Slides:** transitions, animations, sections, templates (L).
- **FigJam:** tables (M); mind map and templates (M); Mermaid (M).
- **AI:** an on-canvas agent with real generation (L).
- **Export:** WebP (S).

### P3 (skip / later)
- **Out of offline-first scope:**
  - real-time multiplayer, cursor chat, spotlight;
  - REST API and webhooks;
  - Sites (only a static-HTML export slice is plausible);
  - code layers.
- **Different product surfaces:**
  - VS Code extension;
  - widgets;
  - generative plugins and skills (the PTY agent covers this);
  - mobile prototype preview.
- **Needs a remote AI model:** AI image tools, vectorize.
- **Minor / low priority:**
  - 3D transforms (early access in Figma);
  - component playground;
  - tab groups and nested folders;
  - screen-wide eyedropper;
  - library guidelines files.

## Epics for REA brainstorm (target: local /Applications/Figma.app 126.9.13)
| # | Epic | Gaps | REA targets |
|---|---|---|---|
| E1 | Style and variable system | effect/grid styles, code syntax, composed opacity, extended collections, color spaces | how kiwi serializes `styleType` EFFECT/GRID; extended-collection fields in VARIABLE_SET; document color profile; COLOR_OPACITY encoding |
| E2 | Auto layout 2026 and grid 2.0 | vertical wrap, Around/Evenly, CSS-aligned flag, more than 8 tracks, auto-position/rows/reorder, Balance/Pretty | wrap algorithm on the vertical axis; legacy/new layout flag field; `stackJustify` enum values; `gridAutoTracks` / `gridItemsPositioning`; balance/pretty line breaking |
| E3 | Slots and design-system authoring | slots (settings, SlotNode), check designs, playground | slot node type and `SLOT_CONTENT_ID` overrides; min/max/preferred fields; check-designs rule set |
| E4 | Draw and vector | variable width, dynamic stroke, brushes, text on path, repeaters, pattern, eraser/cut/offset | `variableWidthPoints`, `complexStrokeProperties`, `TEXT_PATH`, `TRANSFORM_GROUP`, pattern paint fields; cut/offset algorithms; brush stamp model |
| E5 | Effects and shaders | progressive blur, shaders, 3D | progressive blur effect fields and renderer blending; shader node format; 3D matrix storage |
| E6 | Motion and media export | timeline, keyframes, animation styles, Lottie/MP4/GIF/React export, frame PDF/WebP | motion data model; Lottie mapping; video export settings fields |
| E7 | Dev Mode, Code Connect, MCP | Code Connect store, Vue/HTML codegen, focus view, MCP parity, code syntax | Code Connect storage in .fig; focus view semantics; the 41 MCP tool schemas |
| E8 | Interop and ecosystem | Plugin API runtime, Sketch, PPTX, web capture, Figma clipboard paste | plugin sandbox (JS runtime, `figma` global, manifest); API surface used by top-20 plugins; Figma clipboard payload format |
| E9 | FigJam and Slides depth | tables, mind map, templates, Mermaid, slide transitions/animations, PPTX | board table/mind-map node types; slide transition/animation fields |
| E10 | Versions and async collaboration | branching/merge, nested folders, multi-author comments | branch metadata in .fig; merge granularity; named-version snapshot format |

Suggested order (INFERENCE): E1 (holds the P0), then E2 and E3, then E4 + E5, then E6–E10 depending on product direction.

## Overstated or inconsistent repo claims (FACT)
1. README says 487/487 at :67, :410 and :423, while the scorecard JSON and README :10/:26 say 387/387.
2. The parity meter checks only that a regex matches. It passes M36/M37/M41 (Outlines, Pixel preview, Annotations), which the code removed as fake (`menus.rs:1034`).
3. The MCP tool count is given three ways: README 22, CLAUDE.md 29, rg finds 24 distinct names.
4. "Code Connect" is a snippet generator for a non-existent `@designos/code-connect` package. "Connected" is decided by matching an annotation string (`inspect.rs:499-538`).
5. "100%" parity is measured against a frozen list for Figma 126.9.13 that predates Config 2026. It excludes brushes, slot settings, Motion, grid auto-positioning, check designs and extended collections.
6. "Make Design" is template synthesis with no LLM client (`cmdk.rs:615-858`). There is no JA rewrite mode, although the meter claims one.

## Open questions for the owner
- Figma Plugin API runtime: support it (XL) or not?
- Is Figma clipboard paste interop (copy from Figma.app, paste into studio) P0?
- Real-time multiplayer stays P3 (offline-first), with branching/merge as the substitute?
- Which epic goes first in the REA brainstorm?

## Owner decisions (2026-10-17, AskUserQuestion)
- REA brainstorm covers ALL epics E1–E10.
- Figma Plugin API runtime: P1, will be built.
- Figma clipboard paste interop: P0 (new row, part of E8).
- REA runs via `npx rea-agents`, read-only on /Applications/Figma.app.
