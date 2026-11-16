# E9 FigJam, Slides & Whiteboard in Canvas: Master Plan & Hot-File Schedule

**Date:** 2026-11-12  
**Milestone:** v0.8 Presentation, Whiteboard & Interactive Canvas Parity  
**Base Commit:** `origin/main@e0e44f3c2`  
**Evidence Dossier:** `plans/261112-epic-figma-rea-specs/evidence/rea-e9-e8-e19-slides-board-comp.md`  
**Clean-Room Standard:** Clean room reverse engineering. Zero proprietary Figma code copied. All Kiwi tags, JS/WASM offsets, schema IDs, and UI strings in own words.

---

## Answer first

- **Epic `#22` (FigJam & Slides) + `#1215` (Whiteboard in Canvas) Parity:** Clean-room reverse engineering of Figma 126.11.7 settles the exact data structures and binary formats for Slides, FigJam, and Canvas Whiteboard tools.
- **Owner Ruling Enforced (2026-10-30):** Whiteboard tools merge directly into the design canvas ("Không phân biệt white board hay canvas"). A sticky, connector, shape with text, marker, or highlighter is a first-class citizen of any page, whether the document is `DocumentKind::Design`, `Board`, or `Deck`.
- **Zero Custom Mind Map Nodes:** Figma does **NOT** invent a `MINDMAP_NODE`. Mind maps are composed entirely of standard `SHAPE_WITH_TEXT = 18`, `STICKY = 17`, and `CONNECTOR = 19` nodes governed by hierarchical layout fields on `NodeChange`: `layoutRoot = 362`, `diagramParentId = 363`, `layoutPosition = 364`, `diagramLayoutRuleType = TREE = 1`, and `diagramLayoutPaused = 368`. Studio's engine preserves this invariant.
- **Figma Slides Architecture:** Slides are organized into a strict hierarchy: Page -> `SLIDE_GRID = 37` -> `SLIDE_ROW = 38` -> `SLIDE = 32`. Each slide frame carries presentation metadata (`slideSpeakerNotes = 389`, `isSkippedSlide = 410`, `slideNumber = 443`, `slideNumberSeparator = 456`, `slideThemeData = 381`).
- **Full 26-State Transition Engine & Granular Builds:** All 26 Figma `TransitionType` enum variants (`0..25`), easing curves (including cubic presets and springs), and Keynote-grade object animations (`objectAnimations = 426` with `TextElementGrouping` 0..4) are mapped to Studio's prototype and presentation player.
- **Client-Side Mermaid Parsing:** Mermaid diagrams are ingested via client-side AST conversion (`MERMAID_TO_DIAGRAM = 888`) mapping `flowchart`, `state`, `er`, `gantt`, `mindmap`, and `timeline` into native vector shapes, auto-layout cards, and diagram connectors.
- **Strict Hot-File Queue Integration:** All slices modifying `crates/designos-fig/src/fig.rs`, `crates/designos-fig/src/fig_export.rs`, and `crates/designos-fig/src/kiwi.rs` are scheduled strictly serially in slots #33, #34, #35, and #36, continuing the queue established in `specs/e1-styles-variables/` and `specs/e1-e2-rea-resolutions/`.

---

## 1. What could NOT be verified (read first)

1. **Interactive Slide Widget Multiplayer Sync Protocol:** While the Kiwi fields `interactiveSlideConfigData = 371` and `interactiveSlideParticipantData = 372` are verified on `NodeChange`, the live WebSocket multiplayer protocol for audience polling and live reactions (`flappType = 402`) relies on Figma server-side infrastructure (`fig-slides-service`). Studio implements local and offline presentation state for interactive widgets `[FACT - seen schema / ASSUMPTION runtime]`.
2. **Mermaid Subgraph Auto-Grouping in Nested Frames:** The client-side chunk `38026-4a2ee9ec0a6e6f57.min.js` parses subgraphs into grouped bounding boxes, but exact padding defaults for nested subgraphs with horizontal layout (`LR`) were not exposed in WASM string logs. Studio sets a default 32px padding `[INFERENCE]`.
3. **Exact Spring Dampening Defaults for Slide Presets:** Kiwi `EasingType` variants `SPRING_PRESET_ONE = 11`, `SPRING_PRESET_TWO = 12`, and `SPRING_PRESET_THREE = 13` correspond to UI aliases "Gentle", "Quick", and "Bouncy". Their exact damping ratio ($\zeta$) and stiffness ($k$) values in WASM were not decompiled; Studio adopts standard UI spring values ($\zeta = 0.75, 0.85, 0.60$) `[INFERENCE]`.
4. **Hardware Acceleration Scissor Capping for Highlighter Blend Modes:** Figma highlighters (`HIGHLIGHT = 24`, `StrokeCap::HIGHLIGHT = 7`) render with multiply blend mode and soft edges. Whether the GPU shader greeking applies to highlighters zoomed past 12800% was not traced. Studio falls back to CPU reference tessellation outside viewport bounds `[ASSUMPTION]`.

---

## 2. REA Evidence & Checksum Index

Evidence extracted from clean-room disassembly of Figma 126.11.7 artifacts:

| Artifact Name | SHA-256 Digest | REA Artifact Context ID | Primary Subject |
|---|---|---|---|
| `6db9f521-daba2917d67a9c6d.min.js` | `8a5ce31f8f95ca64b652d16b93c1df6087fbe0a57dea6679a491b987eab30abe` | `art_8a5ce31f8f95ca64b652d16b93c1df6087fbe0a57dea6679a491b987eab30abe` | Slides UI, transition panel, deck state |
| `49300-da2f349ab0375764.min.js` | `5984e0fd1e21afbaa0f737641cc6023b69e38c30d1c4c03475965ed39b9cb697` | `art_5984e0fd1e21afbaa0f737641cc6023b69e38c30d1c4c03475965ed39b9cb697` | Properties panel, component rows, slide numbers |
| `39902-1677a713e4a837a0.min.js` | `9ca5217c5b4b7822efb19e72cce01657cde94efef64cf3e7433fffb923a82fec` | `art_9ca5217c5b4b7822efb19e72cce01657cde94efef64cf3e7433fffb923a82fec` | Mermaid parser, PPTX export, Mind Map layout |
| `47277-933cc913dee0b677.min.js` | `2c93d1680820d550afb6b9f3e05e908f7552d5bd9c1afa01f7a9bb2372570f66` | `art_2c93d1680820d550afb6b9f3e05e908f7552d5bd9c1afa01f7a9bb2372570f66` | Whiteboard toolbar, stamps, highlighter tools |
| `70688-e4a6074ba330dbac.min.js` | `9d4f9bd73c70e0cc3927b674c07bbd5eec4a54b5135ed4504359d4b05ca73f4b` | `art_9d4f9bd73c70e0cc3927b674c07bbd5eec4a54b5135ed4504359d4b05ca73f4b` | Connector routing, waypoints, endpoint magnets |
| Unified Extraction Ledger | `18e62e8cf555d7484ae891d606815c373a81effadb933227f813fd2470e279f9` | `jsrg_evidence_18e62e8cf555d7484ae891d606815c373a81effadb933227f813fd2470e279f9` | Complete cross-module call graph |

---

## 3. Scope & Architectural Synthesis

### 3.1 Interplay with Tables (`specs/tables-0710/`)
Epic `#22` encompasses FigJam and Slides tables as well as whiteboard and deck topologies. `specs/tables-0710/` specifies cell auto-layout, column/row sizing, borders, and Kiwi `TABLE = 35` and `TABLE_CELL = 36` nodes. This roadmap builds on top of `tables-0710`, ensuring table components function seamlessly inside both `SLIDE = 32` containers and freeform canvas whiteboards.

### 3.2 Design Canvas & Whiteboard Unification (`#1215`)
Following the owner's ruling (2026-10-30), whiteboard tools do not live in a gated mode or separate sandbox. Designers can drop sticky notes, draw connectors between regular auto-layout frames, sketch with pressure-sensitive highlighters, and stamp approval badges directly on high-fidelity UI screens or on dedicated whiteboard pages.

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                          DESIGN:OS Canvas Space                             │
│                                                                             │
│  ┌───────────────────────┐                    ┌──────────────────────────┐  │
│  │   UI Screen Frame     │◄───────────────────┤      Sticky Note         │  │
│  │   (NodeKind::Frame)   │   CONNECTOR=19     │   (NodeKind::Sticky)     │  │
│  │                       │   (Elbowed,        │   Author: "Jang"         │  │
│  │   [Button] [Input]    │    Magnet::Left)   │   "Review padding here"  │  │
│  └───────────────────────┘                    └──────────────────────────┘  │
│             ▲                                                               │
│             │ CONNECTOR=19 (Curved, ERD_ONE to ERD_MANY)                    │
│             ▼                                                               │
│  ┌───────────────────────┐                    ┌──────────────────────────┐  │
│  │   Database Table      │                    │     Mind Map Branch      │  │
│  │   (NodeKind::Table)   │                    │  (NodeKind::ShapeWithText│  │
│  │   Col A | Col B       │                    │   layoutRoot: 362)       │  │
│  └───────────────────────┘                    └──────────────────────────┘  │
│             │                                                               │
│             ▼ Highlighter Stroke (NodeKind::Highlight, cap: HIGHLIGHT=7)    │
│  ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~  │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Hot-File Queue Position Integration & Serial Schedule

To maintain strict zero-conflict Git hygiene, any modification to the four primary serialization hot files (`crates/designos-fig/src/fig.rs`, `crates/designos-fig/src/fig_export.rs`, `crates/designos-fig/src/kiwi.rs`, and `crates/designos-fig/src/rest_import.rs`) MUST occur in strict serial sequence.

```text
[Position 32]  Grid 2.0: Dynamic Tracks Expansion (E2-REA-3)
       │
       ▼
[Position 33]  E9-SLIDES-1: Slides & Decks Kiwi Codecs (fig.rs / fig_export.rs / kiwi.rs)
       │
       ▼
[Position 34]  E9-BOARD-1: Connectors, Magnets & Waypoints Codecs (fig.rs / fig_export.rs)
       │
       ▼
[Position 35]  E9-BOARD-2: Mind Map Layout & Freehand Brush Codecs (fig.rs / fig_export.rs)
       │
       ▼
[Position 36]  E19-SLOT-1: Issue #473 Preferred-Only Slot Codec Guard (fig_export.rs)
```

---

## 5. Full Slices Execution Table

| Slice ID | Target Issue | Scope & Files Owned | Est. (h) | Model Tier | Hot-File Slot | Dependencies |
|---|---|---|---|---|---|---|
| **E9-SLIDE-1** | `#22` Slides Doc | `SlideProps` expansion (`is_skipped`, `slide_number`, `theme`), `DocumentKind::Deck` (`designos-doc/src/node.rs`, `board.rs`) | 4 | Flash | None (Doc only) | None |
| **E9-SLIDE-2** | `#22` .deck Header | Library 8 KB magic header probe for `fig-deck` and `fig-jam` (`designos-library/src/header.rs`, `probe.rs`) | 3 | Flash | None (Library) | None |
| **E9-SLIDE-3** | `#22` Kiwi Slides | Import/export for `SLIDE=32`, `SLIDE_GRID=37`, `SLIDE_ROW=38`, `slideSpeakerNotes`, `slideNumber` (`designos-fig/src/fig.rs`, `fig_export.rs`, `kiwi.rs`) | 8 | Flash | **Pos 33** | E9-SLIDE-1, E9-SLIDE-2 |
| **E9-SLIDE-4** | `#22` Transitions | All 26 `TransitionType` (0..25) + Easing/Spring engine in presentation player (`designos-doc/src/prototype.rs`, `designos-app/src/decks/`) | 8 | Argon | None (App/Player) | E9-SLIDE-1 |
| **E9-SLIDE-5** | `#22` Object Builds | Granular Keynote-style text element builds (`CHARACTER`, `WORD`, `LINE`, `PARAGRAPH`) & click order (`designos-doc/src/prototype.rs`, `designos-app/src/decks/`) | 7 | Argon | None (App/Player) | E9-SLIDE-4 |
| **E9-CONN-1** | `#22` & `#1215` | `ConnectorProps` expansion: 10 magnets, `relativePosition`, `connectorControlPoints` waypoint cache (`designos-doc/src/board.rs`) | 5 | Flash | None (Doc only) | None |
| **E9-CONN-2** | `#22` & `#1215` | Kiwi codec for `CONNECTOR=19`, waypoints array 253, midpoint text (`designos-fig/src/fig.rs`, `fig_export.rs`) | 6 | Flash | **Pos 34** | E9-CONN-1 |
| **E9-CONN-3** | `#22` & `#1215` | Obstacle-avoiding A* orthogonal elbow routing with pad clearance (`designos-doc/src/board.rs`, `crates/designos-render/src/board.rs`) | 8 | Argon | None (Routing) | E9-CONN-1 |
| **E9-MIND-1** | `#22` Mind Maps | Tree layout engine using `layoutRoot=362`, `diagramParentId=363`, `diagramLayoutRuleType=TREE` (`designos-doc/src/board.rs`, `node.rs`) | 8 | Argon | None (Doc layout) | E9-CONN-1 |
| **E9-MIND-2** | `#22` Mind Maps | Kiwi codec for mindmap hierarchy fields 362..368 (`designos-fig/src/fig.rs`, `fig_export.rs`) | 4 | Flash | **Pos 35** | E9-MIND-1 |
| **E9-MERM-1** | `#22` Mermaid | Client-side Mermaid syntax tokenizer & AST parser for 6 diagram types (`designos-app/src/board/mermaid.rs`) | 8 | Argon | None (App parser) | E9-CONN-1 |
| **E9-MERM-2** | `#22` Mermaid | Node & connector synthesiser + quick action paste handler (`designos-app/src/board/mod.rs`, `create.rs`) | 6 | Flash | None (App UI) | E9-MERM-1, E9-MIND-1 |
| **E9-FREE-1** | `#1215` Freehand | `NodeKind::Highlight`, `NodeKind::Brush`, `VariableWidthPoint` pressure data model (`designos-doc/src/node.rs`, `board.rs`) | 6 | Flash | None (Doc only) | None |
| **E9-FREE-2** | `#1215` Freehand | Catmull-Rom smoothing, pressure stroke polygon tessellator, canvas tool interaction (`designos-app/src/canvas/tool_draw.rs`, `designos-render/`) | 8 | Argon | None (Render/App) | E9-FREE-1 |
| **E9-FREE-3** | `#1215` Freehand | Kiwi codec for `HIGHLIGHT=24`, `BRUSH=46`, `variableWidthPoints=447`, brush settings (`designos-fig/src/fig.rs`, `fig_export.rs`) | 6 | Flash | **Pos 35** (shared) | E9-FREE-1 |

---

## 6. Subagent Cost-Optimization & Model-Tier Routing Policy

In adherence to user-defined runtime rules:
- **`Gemini 4 Carbon` (Coordinator / Spec Architect)**: Writes concrete specs, reviews implementation proofs, manages hot-file serial locks, and independently verifies test gates. Does **NOT** execute routine coding turns.
- **`Model: "flash"` (`Gemini 3.8 Flash`)**: Assigned to all strictly defined schema codecs, Kiwi message definitions, serialization adapters, library file probes, and unit tests.
- **`Model: "pro"` (`Gemini 4 Argon`)**: Assigned to complex mathematical and architectural modules: A* obstacle-avoiding elbow router, Catmull-Rom spline tessellator, Mind Map fractional-index tree re-balancer, Mermaid graph AST lowering, and the 26-state presentation transition engine.

---

## 7. Revert Safety & Quality Gates

1. **Additive Compatibility:** All newly introduced fields on `SlideProps`, `ConnectorProps`, and `NodeChange` are wrapped in `#[serde(default, skip_serializing_if = "...")]`. Older `.designos` files load without migration errors.
2. **Deterministic Fallbacks:** If a `.fig` file containing unsupported widget participants (`flappType = 402`) is imported, it falls back to an `Unknown` or standard `Frame` container without corrupting the surrounding scene graph.
3. **Verification Command Matrix:**
   ```bash
   # Gate 1: Workspace unit tests
   cargo test -p designos-doc --lib
   cargo test -p designos-fig --lib
   cargo test -p designos-render --lib
   cargo test -p designos-app --lib

   # Gate 2: GPU reference parity (headless Bit-exactness)
   cargo test -p designos-gpu

   # Gate 3: Connector routing & Mind Map layout benchmarks
   cargo test -p designos-doc board::tests
   ```
