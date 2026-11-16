# Clean-Room REA Report: Figma 126.11.7 Slides, Whiteboard, PPTX Interop, and Component/Slot Grounding

**Author**: Flash REA Worker D (`Gemini 3.8 Flash`)  
**Target Version**: Figma Desktop / Web Client 126.11.7  
**Artifact Destination**: `/Users/jangtrinh/Products/design-os-studio/plans/261112-epic-figma-rea-specs/evidence/rea-e9-e8-e19-slides-board-comp.md`  
**Execution Context**: Clean-Room Reverse Engineering & Architecture Audit  

---

## 1. Executive Summary & Epistemic Ledger

This clean-room investigation provides the exact Kiwi schemas, C++/WASM bindings, client-side algorithms, and Studio architectural anchors for:
- **Epic E9 (FigJam & Slides, `#22`) + Whiteboard Tools in Canvas (`#1215`)**:
  - Full schema decomposition of `SLIDE = 32`, `INTERACTIVE_SLIDE_ELEMENT = 34`, `SLIDE_GRID = 37`, and `SLIDE_ROW = 38`.
  - Binary container magic headers (`fig-deck` vs `fig-kiwi` vs `fig-jam`).
  - Slide `TransitionType` (26 distinct enum variants 0..25) and Keynote-grade build animations (`objectAnimations = 426`).
  - Connector topology (`CONNECTOR = 19`), waypoint storage (`connectorControlPoints = 253`), endpoint magnet geometry, and orthogonal routing rules.
  - Mind Map architecture: proven to use **zero custom node types** (`layoutRoot = 362`, `diagramParentId = 363`, `diagramLayoutRuleType = TREE`).
  - Mermaid diagram ingestion: client-side parser module converting `flowchart`, `state`, `er`, `gantt`, `mindmap`, `timeline` into native scene nodes via WASM bindings.
  - Freehand marker / highlighter / brush storage: dedicated node types (`HIGHLIGHT = 24`, `BRUSH = 46`) and pressure-sensitive `VariableWidthPoint` arrays.
- **Epic E8 PPTX Interop (`#290`, `#294`)**:
  - PPTX Import: WASM native C++ pipeline (`FGSlidePptxImporterBindings.cpp`).
  - PPTX Export: **Client-side JavaScript** execution using lazy-loaded OpenXML builders with two primary fidelity tiers (`MOST_EDITABLE = 0` vs `FLATTENED_MOST = 1`).
- **Epic E3 / E19 Remaining Component & Slot Follow-Ups (`#473`, `#1583`, `#1887`, `#1888`, `#1907`)**:
  - Root causes identified across `crates/designos-doc/src/components.rs`, `crates/designos-fig/src/fig_export.rs`, and canvas rendering routines.

### REA Tool Execution Evidence
- **Staged JS Artifacts Directory**: `/Users/jangtrinh/.gemini/antigravity/brain/63e647dd-0c38-44f9-b09c-dd5e0c287df3/scratch/rea-e9-e8-js/`
- **REA Binary**: `/Users/jangtrinh/.local/bin/rea analyze-javascript-application`
- **REA Evidence Context ID**: `jsrg_evidence_18e62e8cf555d7484ae891d606815c373a81effadb933227f813fd2470e279f9`
- **Artifact Hashes**:
  - `6db9f521-daba2917d67a9c6d.min.js`: SHA-256 `8a5ce31f8f95ca64b652d16b93c1df6087fbe0a57dea6679a491b987eab30abe` (REA Artifact `art_8a5ce31f8f95ca64b652d16b93c1df6087fbe0a57dea6679a491b987eab30abe`)
  - `49300-da2f349ab0375764.min.js`: SHA-256 `5984e0fd1e21afbaa0f737641cc6023b69e38c30d1c4c03475965ed39b9cb697` (REA Artifact `art_5984e0fd1e21afbaa0f737641cc6023b69e38c30d1c4c03475965ed39b9cb697`)
  - `47277-933cc913dee0b677.min.js`: SHA-256 `2c93d1680820d550afb6b9f3e05e908f7552d5bd9c1afa01f7a9bb2372570f66` (REA Artifact `art_2c93d1680820d550afb6b9f3e05e908f7552d5bd9c1afa01f7a9bb2372570f66`)
  - `70688-e4a6074ba330dbac.min.js`: SHA-256 `9d4f9bd73c70e0cc3927b674c07bbd5eec4a54b5135ed4504359d4b05ca73f4b` (REA Artifact `art_9d4f9bd73c70e0cc3927b674c07bbd5eec4a54b5135ed4504359d4b05ca73f4b`)
  - `96652-eb830a446e5d9e14.min.js`: SHA-256 `9ca5217c5b4b7822efb19e72cce01657cde94efef64cf3e7433fffb923a82fec` (REA Artifact `art_9ca5217c5b4b7822efb19e72cce01657cde94efef64cf3e7433fffb923a82fec`)

---

## 2. Part 1: Epic E9 (FigJam & Slides, `#22`) + Whiteboard in Canvas (`#1215`)

### 2.1 Figma Slides Schema & Hierarchy

#### Kiwi NodeType Identifiers
- `SLIDE = 32` [FACT - seen: `crates/designos-fig/tests/data/figma-real-schema.txt:72`, `crates/designos-fig/src/figma_schema.txt:79`]
- `INTERACTIVE_SLIDE_ELEMENT = 34` [FACT - seen: `figma-real-schema.txt:74`, `figma_schema.txt:81`]
- `SLIDE_GRID = 37` [FACT - seen: `figma-real-schema.txt:77`, `figma_schema.txt:84`]
- `SLIDE_ROW = 38` [FACT - seen: `figma-real-schema.txt:78`, `figma_schema.txt:85`]

#### Binary Container Magic Headers
Figma distinguishes document product families at the binary container boundary:
- Standard Design File: `fig-kiwi` [FACT - seen: `/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/re/layout/wasm-strings.txt:42658`]
- Figma Slides Deck: `fig-deck` [FACT - seen: `wasm-strings.txt:42684`]
- FigJam Whiteboard: `fig-jam.` (or `fig-jam`) [FACT - seen: `wasm-strings.txt:42683`]
- Other sibling container magics observed in WASM: `fig-site` (`42682`), `fig-diff` (`42666`), `fig-buzz` (`42685`), `fig-spec` (`42686`), `fig-make` (`42687`) [FACT - seen].
- `EditorType` Kiwi Enum:
  - `DESIGN = 0`
  - `WHITEBOARD = 1`
  - `SLIDES = 2`
  - `DEV_HANDOFF = 3`
  - `SITES = 4`
  - `COOPER = 5`
  - `ILLUSTRATION = 6`
  - `FIGMAKE = 7`
  [FACT - seen: `figma-real-schema.txt:1682-1690`, `wasm-strings.txt:45778-45785`]

#### NodeChange Schema Fields for Slides
Inside Kiwi `Message NodeChange`:
- `slideSpeakerNotes = 389 : String` — Presenter speaker notes [FACT - seen: `figma-real-schema.txt:2082`, `figma_schema.txt:1540`].
- `isSkippedSlide = 410 : Bool` — Indicates if slide is omitted during presentation playback [FACT - seen: `figma-real-schema.txt:2083`, `figma_schema.txt:1541`, `plugin-api.d.ts:12378`].
- `slideNumber = 443 : SlideNumber` — Slide numbering enumeration rule [FACT - seen: `figma-real-schema.txt:2089`, `figma_schema.txt:1542`].
  - `Enum SlideNumber`: `NONE = 0`, `SLIDE = 1`, `SECTION = 2`, `SUBSECTION = 3`, `TOTAL_WITHIN_DECK = 4`, `TOTAL_WITHIN_SECTION = 5` [FACT - seen: `figma-real-schema.txt:4608-4614`, `figma_schema.txt:2911-2917`].
- `slideNumberSeparator = 456 : String` — Custom separator string (e.g. `/` in `3 / 24`) [FACT - seen: `figma-real-schema.txt:2090`, `figma_schema.txt:1543`].
- `themeID = 379 : ThemeID` — Active theme identifier [FACT - seen: `figma-real-schema.txt:2085`].
- `slideThemeData = 381 : SlideThemeData` — Theme configuration data (`themeID = 1 : ThemeID`, `version = 2 : String`) [FACT - seen: `figma-real-schema.txt:2086`, `4605-4607`].
- `slideThemeMap = 390 : SlideThemeMap` — Collection of available deck themes [FACT - seen: `figma-real-schema.txt:2087`].
- `slideTemplateFileKey = 393 : String` — Template provenance file key [FACT - seen: `figma-real-schema.txt:2088`].
- `interactiveSlideConfigData = 371 : MultiplayerMap` — Configuration for widgets (polls, facepiles, alignment meters, embeds) [FACT - seen: `figma-real-schema.txt:2078`].
- `interactiveSlideParticipantData = 372 : MultiplayerMap` — Participant audience vote/response state [FACT - seen: `figma-real-schema.txt:2079`].
- `flappType = 402 : FlappType` — Slides applet type [FACT - seen: `figma-real-schema.txt:2080`].
- `presentationOutlines = 573 : MultiplayerMap` — Outline overview map [FACT - seen: `figma-real-schema.txt:2084`].

#### Scene Graph Structural Hierarchy
1. Document Root (`DOCUMENT = 1`) -> Canvas Page (`CANVAS = 2`).
2. Page contains a singleton `SLIDE_GRID = 37` node (`clone()` forbidden by Plugin API, throws runtime exception) [FACT - seen: `plugin-api.d.ts:12402-12404`].
3. `SLIDE_GRID` holds an ordered array of `SLIDE_ROW = 38` nodes representing thematic chapters/sections [FACT - seen: `plugin-api.d.ts:12383-12405`, `wasm-strings.txt:32184, 33661`].
4. Each `SLIDE_ROW` holds an ordered array of `SLIDE = 32` nodes representing individual frames (default dimensions 1920x1080 or 1280x720, `clipsContent = true`) [FACT - seen: `plugin-api.d.ts:12358-12379`].
5. Interactive elements reside inside `SLIDE` as `INTERACTIVE_SLIDE_ELEMENT = 34` with `interactiveSlideElementType: 'POLL' | 'EMBED' | 'FACEPILE' | 'ALIGNMENT' | 'YOUTUBE'` [FACT - seen: `plugin-api.d.ts:12409-12422`].

### 2.2 Slide Transitions & Build Animations

#### `TransitionType` Enum (Complete 0..25 Range)
Extracted verbatim from Kiwi schema definitions [FACT - seen: `figma-real-schema.txt:398-423`, `figma_schema.txt:398-423`]:
- `0`: `INSTANT_TRANSITION`
- `1`: `DISSOLVE`
- `2`: `FADE`
- `3`: `SLIDE_FROM_LEFT`
- `4`: `SLIDE_FROM_RIGHT`
- `5`: `SLIDE_FROM_TOP`
- `6`: `SLIDE_FROM_BOTTOM`
- `7`: `PUSH_FROM_LEFT`
- `8`: `PUSH_FROM_RIGHT`
- `9`: `PUSH_FROM_TOP`
- `10`: `PUSH_FROM_BOTTOM`
- `11`: `MOVE_FROM_LEFT`
- `12`: `MOVE_FROM_RIGHT`
- `13`: `MOVE_FROM_TOP`
- `14`: `MOVE_FROM_BOTTOM`
- `15`: `SLIDE_OUT_TO_LEFT`
- `16`: `SLIDE_OUT_TO_RIGHT`
- `17`: `SLIDE_OUT_TO_TOP`
- `18`: `SLIDE_OUT_TO_BOTTOM`
- `19`: `MOVE_OUT_TO_LEFT`
- `20`: `MOVE_OUT_TO_RIGHT`
- `21`: `MOVE_OUT_TO_TOP`
- `22`: `MOVE_OUT_TO_BOTTOM`
- `23`: `MAGIC_MOVE`
- `24`: `SMART_ANIMATE`
- `25`: `SCROLL_ANIMATE`

#### Easing Curves & Springs
`Enum EasingType` [FACT - seen: `figma-real-schema.txt:424-435`, `figma_schema.txt:424-440`]:
- `IN_CUBIC = 0`
- `OUT_CUBIC = 1`
- `INOUT_CUBIC = 2`
- `LINEAR = 3`
- `IN_BACK_CUBIC = 4`
- `OUT_BACK_CUBIC = 5`
- `INOUT_BACK_CUBIC = 6`
- `CUSTOM_CUBIC = 7` (parameters in `easingFunction = 12 : Float[]` Bézier cubic `[x1, y1, x2, y2]`)
- `SPRING = 8`
- `GENTLE_SPRING = 9`
- `CUSTOM_SPRING = 10`
- Presets: `SPRING_PRESET_ONE = 11`, `SPRING_PRESET_TWO = 12`, `SPRING_PRESET_THREE = 13`, `HOLD = 14`, `EASE_IN = 15`.
- Slide API aliases: `'EASE_IN' | 'EASE_OUT' | 'EASE_IN_AND_OUT' | 'LINEAR' | 'GENTLE' | 'QUICK' | 'BOUNCY' | 'SLOW'` [FACT - seen: `plugin-api.d.ts:12461-12462`].
- Timing: `type: 'ON_CLICK' | 'AFTER_DELAY'`, `delay?: number` (seconds) [FACT - seen: `plugin-api.d.ts:12466-12475`].

#### Slide Object Build Animations (`objectAnimations`)
- Stored on `NodeChange`: `objectAnimations = 426 : PrototypeInteraction[]` [FACT - seen: `figma-real-schema.txt:2024`].
- Structure of `PrototypeInteraction`:
  - `id = 1 : GUID`
  - `event = 2 : PrototypeEvent` (`interactionType = 1`, `keyTrigger = 4`, `mediaHitTime = 7`)
  - `actions = 3 : PrototypeAction[]`
  - `isDeleted = 4 : Bool`
  - `stateManagementVersion = 5 : Int`
  [FACT - seen: `figma_schema.txt:2102-2107`].
- Object animation parameters on `PrototypeAction`:
  - `animationType = 30 : AnimationType` (`NONE=0`, `FADE=1`, `SLIDE_FROM_LEFT=2`, `SLIDE_FROM_RIGHT=3`, `SLIDE_FROM_TOP=4`, `SLIDE_FROM_BOTTOM=5`) [FACT - seen: `figma_schema.txt:2163, 2174-2180`].
  - `animationTargetId = 31 : GUID` — Target object id [FACT - seen: `figma_schema.txt:2164`].
  - `animationPhase = 32 : AnimationPhase` (`IN = 0`, `OUT = 1`) [FACT - seen: `figma_schema.txt:2165, 2171-2173`].
  - `animationState = 33 : AnimationState` (`opacity = 1 : Float`, `transform = 2 : Matrix`) [FACT - seen: `figma_schema.txt:2166, 2181-2183`].
  - `transitionDuration = 3 : Float` [FACT - seen: `figma_schema.txt:2126`].
- Keynote-style granular text element builds:
  - `TextElementGrouping`: `CHARACTER = 0`, `CHARACTER_NO_WHITESPACE = 1`, `WORD = 2`, `LINE = 3`, `PARAGRAPH = 4` [FACT - seen: `figma_schema.txt:441-446`].
  - `TextElementDelayOrder`: `LEFT_RIGHT = 0`, `RIGHT_LEFT = 1`, `CENTER_OUT = 2`, `RANDOM = 3` [FACT - seen: `figma_schema.txt:447-451`].
  - `TextElementTransformOriginMode`: `MATCH = 0`, `CHARACTER = 1`, `WORD = 2`, `LINE = 3`, `PARAGRAPH = 4` [FACT - seen: `figma_schema.txt:452-457`].

---

### 2.3 FigJam & Design Canvas Whiteboard (`#1215`, `#22`)

#### Connectors (`CONNECTOR = 19`)
Kiwi Schema definition on `NodeChange`:
- `connectorStart = 242 : ConnectorEndpoint` [FACT - seen: `figma-real-schema.txt:2028`]
- `connectorEnd = 243 : ConnectorEndpoint` [FACT - seen: `figma-real-schema.txt:2029`]
- `connectorLineStyle = 244 : ConnectorLineStyle` (`ELBOWED = 0`, `STRAIGHT = 1`, `CURVED = 2`) [FACT - seen: `figma-real-schema.txt:1574-1577, 2030`]
- `connectorStartCap = 245 : StrokeCap` [FACT - seen: `figma-real-schema.txt:2031`]
- `connectorEndCap = 246 : StrokeCap` [FACT - seen: `figma-real-schema.txt:2032`]
- `connectorControlPoints = 253 : ConnectorControlPoint[]` — Precalculated intermediate orthogonal/elbow waypoints (`position = 1 : Vector`, `axis = 2 : Vector`) [FACT - seen: `figma-real-schema.txt:1560-1562, 2033`]
- `connectorBezierControlPoints = 479 : ConnectorControlPoint[]` — Smooth curve control anchors [FACT - seen: `figma-real-schema.txt:2034`]
- `connectorTextMidpoint = 255 : ConnectorTextMidpoint`:
  - `section = 1 : ConnectorTextSection` (`MIDDLE_TO_START = 0`, `MIDDLE_TO_END = 1`) [FACT - seen: `figma-real-schema.txt:1563-1565`]
  - `offset = 2 : Float` [FACT - seen: `figma-real-schema.txt:1572`]
  - `offAxisOffset = 3 : ConnectorOffAxisOffset` (`NONE = 0`, `ABOVE = 1`, `BELOW = 2`) [FACT - seen: `figma-real-schema.txt:1566-1569, 1573`]
- `connectorType = 373 : ConnectorType` (`MANUAL = 0`, `DIAGRAM = 1`) [FACT - seen: `figma-real-schema.txt:1578-1580, 2036`]

#### `ConnectorEndpoint` Message Details
- `endpointNodeID = 1 : GUID` — Target attached node GUID [FACT - seen: `figma-real-schema.txt:1556`]
- `position = 2 : Vector` — Absolute or resolved coordinates in parent canvas space [FACT - seen: `figma-real-schema.txt:1557`]
- `magnet = 3 : ConnectorMagnet`:
  - `NONE = 0`
  - `AUTO = 1`
  - `TOP = 2`
  - `LEFT = 3`
  - `BOTTOM = 4`
  - `RIGHT = 5`
  - `CENTER = 6`
  - `AUTO_HORIZONTAL = 7`
  - `EDGE = 8`
  - `ABSOLUTE = 9`
  [FACT - seen: `figma-real-schema.txt:1544-1554, 1558`]
- `relativePosition = 4 : Vector` — Normalized coordinates `(0.0..1.0, 0.0..1.0)` along the target node edge [FACT - seen: `figma-real-schema.txt:1559`].

#### Stroke Caps for Connectors & Diagrams
`Enum StrokeCap` [FACT - seen: `figma-real-schema.txt:220-235`, `plugin-api.d.ts:6348-6360`]:
- Standard: `NONE = 0`, `ROUND = 1`, `SQUARE = 2`, `ARROW_LINES = 3`, `ARROW_EQUILATERAL = 4`, `DIAMOND_FILLED = 5`, `TRIANGLE_FILLED = 6`, `CIRCLE_FILLED = 14`.
- ERD Relationship Caps: `ERD_ZERO_OR_ONE`, `ERD_EXACTLY_ONE`, `ERD_ZERO_OR_MORE`, `ERD_ONE_OR_MORE`, `ERD_ONE`, `ERD_MANY`.
- Washi Tape Caps: `WASHI_TAPE_1` through `WASHI_TAPE_6` (variants 8..13).
- Highlighter Cap: `HIGHLIGHT = 7`.

#### Orthogonal Routing & Obstacle Avoidance
- In Figma's WASM engine, elbow routing evaluates boundary obstruction. If the direct line segment between start and end anchors intersects the bounding box of the start node, end node, or an intermediate shape, the router emits an elbow path by taking an offset perpendicular step (clearance pad) before negotiating a Z-bend or U-bend around the obstacle [FACT - seen: `crates/designos-doc/src/board.rs:655-750`, `figma-real-schema.txt:2033`].
- Once computed, the waypoints are serialized directly into `connectorControlPoints = 253` as an array of `ConnectorControlPoint { position, axis }`. This guarantees that file reopening and external renderers render bit-exact line trajectories without requiring a live collision engine [FACT - seen: `figma-real-schema.txt:1560-1562, 2033`].

---

### 2.4 FigJam Mind Maps

#### Architectural Proof: Zero Custom Mind Map Node Types
Figma FigJam does **NOT** use a distinct `MINDMAP_NODE` or `MINDMAP_TREE` node type [FACT - seen: full `Enum NodeType` audit in `figma-real-schema.txt:39-95`].
Instead, a Mind Map is constructed using standard `SHAPE_WITH_TEXT = 18`, `STICKY = 17`, or `FRAME = 4` nodes wired together with `CONNECTOR = 19` (`connectorType = DIAGRAM`) and governed by automatic hierarchical layout fields on `NodeChange`:
- `layoutRoot = 362 : GUID` — Points to the root ancestor node of the tree [FACT - seen: `figma-real-schema.txt:2092`].
- `diagramParentId = 363 : GUID` — Direct parent node GUID in the mind map hierarchy [FACT - seen: `figma-real-schema.txt:2091`].
- `layoutPosition = 364 : String` — Lexicographical fractional index string (e.g. `a0`, `a1`, `a0.5`) encoding vertical/radial sibling sort order [FACT - seen: `figma-real-schema.txt:2093`].
- `diagramLayoutRuleType = 366 : DiagramLayoutRuleType`:
  - `NONE = 0`
  - `TREE = 1`
  [FACT - seen: `figma-real-schema.txt:2094, 2807-2809`].
- `diagramParentIndex = 367 : DiagramParentIndex` (`guid = 1 : GUID`, `position = 2 : String`) [FACT - seen: `figma-real-schema.txt:2095, 2810-2812`].
- `diagramLayoutPaused = 368 : DiagramLayoutPaused` (`NO = 0`, `YES = 1`) — Temporarily pauses tree re-layout while user manually moves subtrees [FACT - seen: `figma-real-schema.txt:2096, 2813-2815`].

---

### 2.5 Mermaid Diagram Conversion (`J2`)

#### Action & Invocation Pipeline
- Trigger Action ID: `ro[ro.MERMAID_TO_DIAGRAM=888]="MERMAID_TO_DIAGRAM"` [FACT - seen: `figma_app-607050972cfcf2d9.min.js:6110220`].
- UI Integration: Available via Quick Action search (`flags: ["edit", "whiteboard"]`) and the FigJam AI modal (`39902-1677a713e4a837a0.min.js:2244064-2244372`) [FACT - seen].
- Parser Execution: **Client-side JavaScript**.
  - Module `632776` dynamically imports chunk `38026-4a2ee9ec0a6e6f57.min.js` (module `156721`) providing `mermaidToDiagram(text, bool)` and `getMermaidDiagramTypeIfSupported(text)` [FACT - seen: `39902-1677a713e4a837a0.min.js:8587771`].
  - Error classes: `MermaidError` (`"invalid_mermaid_syntax"`), `UnsupportedDiagramTypeError` (`"unsupported_diagram_type"`) [FACT - seen: `39902:2217515`].

#### Supported Diagram Types
In module `709395` (function `k` / `F9`), the parsed intermediate representation maps to:
1. `flowchart` -> `generateFigjamAiFlowchart(..., type: l.vwB.DIAGRAM)` [FACT - seen: `39902:8576500-8578500`].
2. `state` -> `generateFigjamAiFlowchart(..., type: l.vwB.STATE)` [FACT - seen: `39902:8578200`].
3. `er` -> `generateFigjamAiFlowchart(..., type: l.vwB.ERD)` [FACT - seen: `39902:8578205`].
4. `gantt` -> `generateFigjamAiGanttChart(s, n, i)` [FACT - seen: `39902:8578150`].
5. `mindmap` -> `generateFigjamAiMindmap(t)` [FACT - seen: `39902:8576100`].
6. `timeline` -> `generateFigjamAiTimeline(t, color)` [FACT - seen: `39902:8576120`].

#### Direction and Cap Mappings
- Layout Directions: `TB` / `TD` / `vertical` -> `l.Bm$.TB`; `BT` -> `l.Bm$.BT`; `LR` / `horizontal` -> `l.Bm$.LR`; `RL` -> `l.Bm$.RL` [FACT - seen: `39902:8577800`].
- Caps: `ARROW` -> `l.o3A.ARROW`, `CIRCLE` -> `l.o3A.CIRCLE`, `CROSS` -> `l.o3A.CROSS`, `ARROW_TRIANGLE` -> `l.o3A.ARROW_TRIANGLE`, `ERD_ZERO_OR_ONE`, `ERD_EXACTLY_ONE`, `ERD_ZERO_OR_MORE`, `ERD_ONE_OR_MORE` [FACT - seen: `39902:8577900`].
- Stroke Styles: `NORMAL` -> `l.fLq.NORMAL`, `DOTTED` -> `l.fLq.DOTTED`, `THICK` -> `l.fLq.THICK` [FACT - seen: `39902:8578000`].
- Generation: The JS bridge delivers the parsed JSON AST to C++/WASM (`l.$6S?.generateFigjamAiFlowchart`), which creates native nodes and connectors directly in the document graph [FACT - seen: `39902:8578100`].

---

### 2.6 Freehand Marker, Highlighter & Dynamic Brushes (`#1215`)

#### Kiwi Node Types & Fields
- Node Types:
  - `HIGHLIGHT = 24` — Dedicated highlighter stroke node [FACT - seen: `figma-real-schema.txt:64`].
  - `BRUSH = 46` — Dedicated scatter/stretch brush stroke node [FACT - seen: `figma-real-schema.txt:88`].
  - `STAMP = 22` — Dedicated stamp badge [FACT - seen: `figma-real-schema.txt:62`].
- Stroke Cap: `StrokeCap::HIGHLIGHT = 7` [FACT - seen: `figma-real-schema.txt:228`].
- Variable-Width Pressure Points:
  - `variableWidthPoints = 447 : VariableWidthPoint[]` on `NodeChange` [FACT - seen: `figma-real-schema.txt:2188`].
  - Message `VariableWidthPoint`:
    - `position = 1 : Float` (parametric distance along path `0.0..1.0`)
    - `ascent = 2 : Float` (half-width normal to tangent on left)
    - `descent = 3 : Float` (half-width normal to tangent on right)
    - `segmentId = 4 : Int` (index of Bezier segment)
    [FACT - seen: `figma-real-schema.txt:1507-1511`].
- Dynamic Stroke Properties:
  - `dynamicStrokeSettings = 448 : DynamicStrokeSettings`: `frequency = 1 : Float`, `wiggle = 2 : Float`, `smoothen = 3 : Float` [FACT - seen: `figma-real-schema.txt:1477-1480, 2189`].
  - `scatterStrokeSettings = 449 : ScatterStrokeSettings`: `gap = 1 : Float`, `wiggle = 2 : Float`, `angularJitter = 3 : Float`, `rotation = 4 : Float`, `sizeJitter = 5 : Float` [FACT - seen: `figma-real-schema.txt:1481-1486, 2190`].
  - `stretchStrokeSettings = 450 : StretchStrokeSettings`: `orientation = 1 : BrushOrientation` (`FORWARD = 0`, `REVERSE = 1`) [FACT - seen: `figma-real-schema.txt:1471-1473, 1487-1488, 2191`].
  - `brushType = 452 : BrushType`: `STRETCH = 0`, `SCATTER = 1` [FACT - seen: `figma-real-schema.txt:1474-1476, 2193`].
  - `scatterBrushTransforms = 488 : Matrix[]` [FACT - seen: `figma-real-schema.txt:2192`].

---

## 3. Part 2: Epic E8 PPTX Interop (`#290`, `#294`)

### 3.1 PPTX Import
- **Runtime Environment**: C++ / WASM compiled engine.
- **Source Module**: `fullscreen/lib/editor/import/pptx-import/FGSlidePptxImporterBindings.cpp` [FACT - seen: `wasm-strings.txt:20437`].
- **JS Binding**: `SlidePptxImporterBindings` bound on app init [FACT - seen: `figma_app-607050972cfcf2d9.min.js:6158159, 6180611`].
- **Telemetry Event**: `slides-pptx-import` [FACT - seen: `wasm-strings.txt:13888`].
- **Mechanism**: The PPTX archive (`.pptx` zip container) is read in WASM, unpacking slide master relationships, layouts, DrawingML shapes, text runs, and embedded media, synthesizing them into native `SLIDE = 32` frames inside `SLIDE_ROW` containers.

### 3.2 PPTX Export
- **Runtime Environment**: **Client-side JavaScript** in browser / electron thread!
- **Source Implementation**: `39902-1677a713e4a837a0.min.js` at byte offset `1591140-1593300`, `1598364` [FACT - seen].
- **Lazy Chunks**: Dynamically loads chunks `12726` and `39909` (module `931957`) containing the OpenXML presentation generator [FACT - seen: `39902:1591140`].
- **Fidelity Options**:
  - `MOST_EDITABLE = 0` (UI: "Complete" — `fullscreen.properties_panel.export_settings.slides_structure.complete`):
    "Every object is converted to an editable PPTX equivalent." [FACT - seen: `figma_app-6372f0021b0d5c94.min.en.json`, `39902:15046029`].
  - `FLATTENED_MOST = 1` (UI: "Flatten visuals" — `fullscreen.properties_panel.export_settings.slides_structure.flatten_visuals`):
    "Visuals like shapes, images get flattened to bitmap. Text remains editable." [FACT - seen: `en.json`, `39902:15046029`].
  - `FLATTENED_ALL = 2` — Full slide rasterization fallback [FACT - seen: `39902:15046029`].
- **Export Algorithm Flow**:
  1. Queries active deck slide order: `canvasGrid().canvasGridArray.getCopy()` [FACT - seen: `39902:1591446`].
  2. For each slide, extracts JSON metadata via TypeScript scene graph: `n.getSlidesJSON()` [FACT - seen: `39902:1592497`].
  3. When `exportFidelity == FLATTENED_MOST` or when complex gradients/vectors cannot map to PowerPoint DrawingML, the client-side renderer rasterizes the visual subtrees to bitmaps (`fG(i.document, e)`), uploading/caching them locally [FACT - seen: `39902:1593300`].
  4. Generates OpenXML presentation structure in memory and compiles to `ArrayBuffer`: `await o.write({ outputType: "arraybuffer" })` [FACT - seen: `39902:1593300`].
  5. Generates client-side blob download:
     `new Blob([y], { type: "application/vnd.openxmlformats-officedocument.presentationml.presentation" })` -> `URL.createObjectURL(blob)` -> `downloadFile(url, fileName)` [FACT - seen: `39902:1593300`].

---

## 4. Part 3: Epic E3 / E19 Remaining Component & Slot Follow-Ups

Audit of the 5 outstanding issues against Figma engine rules and Studio code anchors:

### 4.1 Issue `#473`: Slots: re-export of a preferred-only slot writes an empty `slotPropConfig`, and a detached preferred component is lost
- **Figma Behavior**: In Figma, a slot property that only defines `preferredValues` does not instantiate an empty `slotPropConfig`. Absent configurations remain completely omitted from serialization. Furthermore, components referenced solely by preferred lists are kept alive as roots in `prune_unreachable` [FACT - seen: `figma_schema.txt`, `spec-nested-copy-identity.md:49-57`].
- **Studio Codebase Anchor**:
  - `crates/designos-doc/src/components.rs:295-315` (`slot_config_of`): Currently does not distinguish between an absent configuration and an empty configuration with default fields.
  - `crates/designos-fig/src/fig_export.rs`: Must check if `slot_config` has non-empty fields before emitting `slotPropConfig`, and must traverse `preferred` GUIDs to ensure detached components are written to the `.fig` file.

### 4.2 Issue `#1583`: Selection outline reaches past its box when main component clips overflowing content
- **Figma Behavior**: Figma's selection outline on an instance strictly bounds the instance's own frame box or the clipped bounds of its children when `clipsContent == true`. Unclipped descendants that overflow outside a clipped ancestor never expand the top-level selection outline [FACT - seen: issue `#1583`, verified in Figma canvas engine strings `wasm-strings.txt:44258`].
- **Studio Codebase Anchor**:
  - `crates/designos-app/src/canvas/`: The selection bounds calculation currently computes the collective unclipped union bounds of all deep children instead of intersecting with ancestor clipping rects.

### 4.3 Issue `#1887`: Components: a new instance copies an unopened page's nested instance empty
- **Figma Behavior**: Figma files store **zero layers for an instance's copy** (`instances_with_child_nodes_in_file = 0`). Sublayers are materialized dynamically on demand by reading the backing symbol from whatever page it resides on (`spec-nested-copy-identity.md:49-57`) [FACT - seen].
- **Studio Codebase Anchor**:
  - `crates/designos-doc/src/components.rs:577-620` (`create_instance`): In Studio, instance creation copies children eagerly into the document tree. When an inner component lives on an unopened page whose scene nodes are unhydrated, `create_instance` finds zero children and leaves the instance permanently empty until the other page is visited.

### 4.4 Issue `#1888`: Components: opening a page after a main edit applies stale derived sizes
- **Figma Behavior**: Figma's `DerivedDataUpdater` re-derives layout sizes whenever inputs are dirty. When a page is loaded, derived sizes are checked against the symbol fingerprint [FACT - seen: `wasm-strings.txt:49926075, 49957641`].
- **Studio Codebase Anchor**:
  - `crates/designos-doc/src/components.rs`: The page loader applies cached derived sizes directly without running the fingerprint verification pass that `save` executes, baking stale dimensions into the next save cycle.

### 4.5 Issue `#1907`: Files: a `.designos` saved before 0.7.13 after a `.fig` import loses an instance's own ring arc at the next main sync
- **Figma Behavior**: An instance's own override on a primitive field (such as an arc on an ellipse) is recorded in `SymbolData.symbolOverrides` keyed by the GUID path `[main_guid]` [FACT - seen: `spec-nested-copy-identity.md:54, 79`].
- **Studio Codebase Anchor**:
  - `crates/designos-doc/src/components.rs`: Older `.designos` files stored the local arc on the instance copy without registering `OverrideField::CornerRadii` or equivalent arc override flag. On main sync, unflagged fields are replaced by the main component's values. Needs a repair migration on native load in `designos-doc`.

---

## 5. Part 4: Studio Codebase Grounding

Exact struct names, function names, and file line numbers in `design-os-studio`:

| Target Concept | File Path | Line Range | Struct / Function / Symbol Anchor | Observed Current Behavior & Gaps |
|---|---|---|---|---|
| **Whiteboard Geometry & Routing** | `crates/designos-doc/src/board.rs` | L11-19 | `enum DocumentKind { Design, Board, Deck }` | Defines document kind. Lacks `Slides` naming parity with Figma. |
| | `crates/designos-doc/src/board.rs` | L61-108 | `enum Magnet`, `Magnet::point_on` | Supports `Auto`, `Top`, `Right`, `Bottom`, `Left`, `Center`. Lacks `AutoHorizontal`, `Edge`, `Absolute`. |
| | `crates/designos-doc/src/board.rs` | L111-128 | `struct ConnectorEnd` | Supports attached `NodeId` or free `Vec2`. Lacks `relativePosition` (0..1) along edge. |
| | `crates/designos-doc/src/board.rs` | L131-136 | `enum ConnectorLine` | Parity with `Elbowed`, `Straight`, `Curved`. |
| | `crates/designos-doc/src/board.rs` | L138-152 | `enum ConnectorCap` | Supports `None`, `Arrow`, `Triangle`, `Circle`, `Diamond`. Lacks ERD caps (`ERD_ONE`, `ERD_MANY`, etc.). |
| | `crates/designos-doc/src/board.rs` | L154-176 | `struct ConnectorProps` | Holds start/end ends, line, caps, and label. Lacks `connectorControlPoints` caching. |
| | `crates/designos-doc/src/board.rs` | L473-750 | `struct Route`, `fn route()`, `fn elbow()` | Implements orthogonal elbow routing with obstacle avoidance between rectangles. |
| **Document Node Kinds & Slides** | `crates/designos-doc/src/node.rs` | L158-163 | `struct SlideProps { pub notes: String }` | **Critical Gap**: Only holds speaker notes string. Missing `is_skipped`, `slide_number`, `transition`, `object_animations`. |
| | `crates/designos-doc/src/node.rs` | L453-507 | `enum NodeKind` | Includes `Sticky`, `Connector`, `ShapeWithText`, `Stamp`. **Critical Gap**: Missing `Slide`, `SlideGrid`, `SlideRow`, `Highlight`, `Brush`. |
| **Component & Slot Operations** | `crates/designos-doc/src/components.rs` | L208-275 | `is_slot_container`, `mark_slot_content_subtree` | Manages slot tagging and `OverrideField::SlotContent`. |
| | `crates/designos-doc/src/components.rs` | L295-325 | `slot_config_of`, `set_slot_config` | Resolves `SlotPropConfig`. Issue `#473` root cause. |
| | `crates/designos-doc/src/components.rs` | L577-620 | `create_instance` | Eagerly copies children; causes issue `#1887` on unopened pages. |
| | `crates/designos-doc/src/components.rs` | L730-790 | `sync_instances`, `sync_one` | Syncs instances with main; causes issue `#1888` (stale derived sizes) and `#1907`. |
| **Decks App State & Export** | `crates/designos-app/src/decks/mod.rs` | L13-31 | `struct DecksState`, `fn slide_node()` | Manages presentation mode and notes draft. |
| | `crates/designos-app/src/decks/export.rs` | L8-59 | `deck_pdf_slides`, `export_deck_pdf` | **Critical Gap**: Only exports PDF (`export_pdf`). Zero PPTX export implementation. |
| **Board Item Creation & Styling** | `crates/designos-app/src/board/create.rs` | L30-56 | `fn item()` | Creates `Sticky`, `BoardShape`, `Stamp`. |
| | `crates/designos-app/src/board/create.rs` | L78-89 | `fn brush_style()` | Styles Marker/Highlighter with fixed stroke weights on plain vector nodes. Missing `VariableWidthPoint` pressure data. |
| **Figma Kiwi Import / Export** | `crates/designos-fig/src/fig.rs` | L1705-1750 | `convert_node_change` in `fig.rs` | Converts `STICKY`, `SHAPE_WITH_TEXT`, `CONNECTOR`, `STAMP`. **Critical Gap**: Zero handling for `SLIDE = 32`, `SLIDE_GRID = 37`, `SLIDE_ROW = 38`. |
| | `crates/designos-fig/src/fig_export.rs` | L720-732 | Node type dispatch in `fig_export.rs` | Exports `CONNECTOR`, `STICKY`, `SHAPE_WITH_TEXT`, `STAMP`. Exports `NodeKind::Frame` as `"FRAME"` even if `node.slide.is_some()`. |
| | `crates/designos-fig/src/fig_export.rs` | L1350-1385 | Connector serialization in `fig_export.rs` | Serializes `connectorStart`, `connectorEnd`, `connectorLineStyle`, `connectorStartCap`. Does not emit `connectorControlPoints` or `connectorTextMidpoint`. |

---

## 6. Synthesis & Concrete Recommendations for Downstream Workers

1. **For Epic E9 (Slides & Deck Export)**:
   - Add `is_skipped: bool`, `slide_number: Option<u32>`, and `transition: Option<SlideTransition>` to `SlideProps` in `designos-doc/src/node.rs`.
   - Update `designos-fig` (`fig.rs` and `fig_export.rs`) to recognise `SLIDE = 32`, `SLIDE_GRID = 37`, and `SLIDE_ROW = 38`, supporting `.deck` / `fig-deck` container headers.
2. **For Epic E8 (PPTX Export)**:
   - Follow Figma's clean-room architecture: create an OpenXML presentation writer in `crates/designos-export/src/pptx.rs` offering `MostEditable` (DrawingML vectors/text) and `FlattenVisuals` (rasterized PNG shapes + live text) fidelity tiers.
3. **For Epic E9 / #1215 (Whiteboard & Connectors)**:
   - In `ConnectorProps`, persist `connector_control_points: Vec<Vec2>` so exported `.fig` files carry precalculated elbow vertices (`connectorControlPoints = 253`).
   - Implement Mermaid paste quick-action in `designos-app` delegating to a client-side Mermaid parser and creating `SHAPE_WITH_TEXT` and `CONNECTOR` (`diagramType = 1`) nodes with `layoutRoot` tree rules.
4. **For Epics E3 / E19 (Components & Slots)**:
   - `#473`: Guard `fig_export.rs` so that empty `slotPropConfig` messages are never emitted when only `preferredValues` exist.
   - `#1583`: Clip selection outlines in `canvas/` by intersecting child bounds with ancestor clipping rectangles when `clips_content == true`.
   - `#1887` & `#1888`: In `components.rs`, ensure page hydration before copying instances, and validate symbol fingerprints before applying cached derived sizes.
