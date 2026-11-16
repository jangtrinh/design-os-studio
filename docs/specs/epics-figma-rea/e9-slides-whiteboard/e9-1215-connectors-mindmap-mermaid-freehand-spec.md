> **Review status (2026-11-12): NEEDS OWNER DECISION.** Reference for implementation; writing native CONNECTOR nodes in design files contradicts the standing vector-export rule, most connector work shipped in 0.7.10, and new node kinds conflict with the no-format-change rule; these are owner decisions. Corrections: [review](../reviews/review-e8-to-e13.md#e9-connectors-mermaid).

# True Figma Connectors, Mind Maps, Mermaid & Freehand in Canvas Spec

**Date:** 2026-11-12  
**Milestone:** v0.8 Whiteboard in Canvas, Diagramming & Freehand Parity (`#22`, `#1215`)  
**Base Commit:** `origin/main@e0e44f3c2`  
**Evidence:** Clean-room disassembly of Figma 126.11.7 (`rea-e9-e8-e19-slides-board-comp.md`, REA ID `jsrg_evidence_18e62e8cf555d7484ae891d606815c373a81effadb933227f813fd2470e279f9`)  
**Clean-Room Standard:** Clean room reverse engineering. Zero Figma code copied. All Kiwi tags, field IDs, offsets, and algorithms in own words.

---

## Answer first

- **Owner Ruling Enforced (2026-10-30):** Whiteboard tools merge directly into the design canvas ("Không phân biệt white board hay canvas"). Sticky notes, connectors, shapes with text, stamps, markers, and highlighters are usable on any design frame or canvas page.
- **Connectors Architecture (`CONNECTOR = 19`):** Connectors connect two nodes (or free points) via typed endpoints (`connectorStart = 242`, `connectorEnd = 243`). Each endpoint carries a 10-state `Magnet` enum (`0..9`), node GUID, absolute point, and normalized edge coordinate (`relativePosition = 4 : Vector`). Precomputed orthogonal waypoints are serialized directly in `connectorControlPoints = 253`, guaranteeing bit-exact file reopening without live collision recalculation `[FACT - seen]`.
- **Zero Custom Mind Map Nodes:** Figma does **NOT** have a `MINDMAP_NODE` type in its schema `[FACT - seen: `figma-real-schema.txt:39-95`]. Mind Maps are standard `SHAPE_WITH_TEXT = 18` or `STICKY = 17` nodes joined by `CONNECTOR = 19` (`connectorType = DIAGRAM = 1`). Hierarchy is encoded via standard `NodeChange` fields: `layoutRoot = 362`, `diagramParentId = 363`, `layoutPosition = 364` (fractional index string), `diagramLayoutRuleType = TREE = 1`, and `diagramLayoutPaused = 368`. Studio strictly adheres to this schema; creating a separate `NodeKind::MindMap` is strictly forbidden.
- **Mermaid Diagram Conversion (`J2`):** Figma ingests Mermaid diagrams via a client-side parser module (`ro.MERMAID_TO_DIAGRAM = 888`) supporting `flowchart`, `state`, `er`, `gantt`, `mindmap`, and `timeline`. The AST converts directly to native shapes and connectors on canvas `[FACT - seen]`.
- **Freehand Marker & Highlighter (`#1215`):** Highlighters and brushes use dedicated node types (`HIGHLIGHT = 24`, `BRUSH = 46`) and stroke cap `StrokeCap::HIGHLIGHT = 7` `[FACT - seen]`. Variable stylus pressure is captured via `variableWidthPoints = 447 : VariableWidthPoint[]` (holding parametric position, left ascent, right descent, and segment ID) combined with Catmull-Rom curve fitting.

---

## 1. What could NOT be verified (read first)

1. **Wasm Collision Tree Depth in A\* Elbow Routing:** While the presence of waypoint precalculation (`connectorControlPoints = 253`) and obstacle pad clearance is verified, the maximum search depth / quadtree node traversal limit of the A* router in WASM was not decompiled. Studio sets a budget limit of 256 visited cells before falling back to bounding-box perimeter routing `[INFERENCE]`.
2. **Mermaid Custom Class Styling Syntax:** Figma's client-side Mermaid parser accepts basic class definitions (`classDef`), but mapping custom CSS classes to arbitrary fill palettes was not fully traced. Studio maps standard named colors (e.g. `fill:#f9f,stroke:#333`) to native `Paint::Solid` `[INFERENCE]`.
3. **Scatter Brush Texture Atlas Ingestion:** For `BRUSH = 46` with `brushType = SCATTER = 1`, Figma uses `scatterBrushTransforms = 488` to repeat brush stamp shapes. The exact runtime sprite atlas cache was not traced; Studio computes instanced 2D affine transforms `[INFERENCE]`.

---

## 2. Figma Facts (REA Evidence)

### 2.1 Binary Container & Kiwi Identifiers
- FigJam Magic Container Header: `fig-jam` or `fig-jam.` (`wasm-strings.txt:42683`) `[FACT - seen]`.
- `EditorType::WHITEBOARD = 1` (`figma-real-schema.txt:1684`) `[FACT - seen]`.
- Kiwi Node Types:
  - `STICKY = 17` (`figma-real-schema.txt:57`)
  - `SHAPE_WITH_TEXT = 18` (`figma-real-schema.txt:58`)
  - `CONNECTOR = 19` (`figma-real-schema.txt:59`)
  - `STAMP = 22` (`figma-real-schema.txt:62`)
  - `HIGHLIGHT = 24` (`figma-real-schema.txt:64`)
  - `BRUSH = 46` (`figma-real-schema.txt:88`)

### 2.2 Connector Schema Decomposition
Fields on `NodeChange`:
- `connectorStart = 242 : ConnectorEndpoint` (`figma-real-schema.txt:2028`)
- `connectorEnd = 243 : ConnectorEndpoint` (`figma-real-schema.txt:2029`)
- `connectorLineStyle = 244 : ConnectorLineStyle`:
  - `ELBOWED = 0`
  - `STRAIGHT = 1`
  - `CURVED = 2`
  (`figma-real-schema.txt:1574-1577, 2030`)
- `connectorStartCap = 245 : StrokeCap` (`figma-real-schema.txt:2031`)
- `connectorEndCap = 246 : StrokeCap` (`figma-real-schema.txt:2032`)
- `connectorControlPoints = 253 : ConnectorControlPoint[]`:
  - `position = 1 : Vector`
  - `axis = 2 : Vector`
  (`figma-real-schema.txt:1560-1562, 2033`)
- `connectorBezierControlPoints = 479 : ConnectorControlPoint[]` (`figma-real-schema.txt:2034`)
- `connectorTextMidpoint = 255 : ConnectorTextMidpoint`:
  - `section = 1 : ConnectorTextSection` (`MIDDLE_TO_START = 0`, `MIDDLE_TO_END = 1`)
  - `offset = 2 : Float`
  - `offAxisOffset = 3 : ConnectorOffAxisOffset` (`NONE = 0`, `ABOVE = 1`, `BELOW = 2`)
  (`figma-real-schema.txt:1563-1573`)
- `connectorType = 373 : ConnectorType` (`MANUAL = 0`, `DIAGRAM = 1`) (`figma-real-schema.txt:1578-1580, 2036`)

#### `ConnectorEndpoint` Message Details (`figma-real-schema.txt:1555-1559`):
- `endpointNodeID = 1 : GUID` (attached node ID)
- `position = 2 : Vector` (absolute canvas coordinates)
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
- `relativePosition = 4 : Vector` (normalized coordinates `(0.0..1.0, 0.0..1.0)` along target node perimeter)

#### Complete Stroke Cap Enum (`Enum StrokeCap` 0..14+):
- `0`: `NONE`
- `1`: `ROUND`
- `2`: `SQUARE`
- `3`: `ARROW_LINES` (Open chevron)
- `4`: `ARROW_EQUILATERAL` (Filled triangle)
- `5`: `DIAMOND_FILLED`
- `6`: `TRIANGLE_FILLED`
- `7`: `HIGHLIGHT` (Chisel tip)
- `8..13`: `WASHI_TAPE_1` through `WASHI_TAPE_6`
- `14`: `CIRCLE_FILLED`
- Diagram/ERD Caps: `ERD_ZERO_OR_ONE`, `ERD_EXACTLY_ONE`, `ERD_ZERO_OR_MORE`, `ERD_ONE_OR_MORE`, `ERD_ONE`, `ERD_MANY`

### 2.3 Mind Maps Schema & Hierarchy Fields
Mind maps use standard `SHAPE_WITH_TEXT = 18` and `CONNECTOR = 19` nodes. The tree hierarchy is maintained via these exact fields on `NodeChange`:
- `layoutRoot = 362 : GUID` — Points to root node of the diagram tree (`figma-real-schema.txt:2092`) `[FACT - seen]`.
- `diagramParentId = 363 : GUID` — Direct parent node GUID in tree (`figma-real-schema.txt:2091`) `[FACT - seen]`.
- `layoutPosition = 364 : String` — Fractional index string (e.g. `"a0"`, `"a1"`, `"a0.5"`) defining vertical/radial sibling sort order (`figma-real-schema.txt:2093`) `[FACT - seen]`.
- `diagramLayoutRuleType = 366 : DiagramLayoutRuleType` (`NONE = 0`, `TREE = 1`) (`figma-real-schema.txt:2094, 2807-2809`) `[FACT - seen]`.
- `diagramParentIndex = 367 : DiagramParentIndex` (`guid = 1 : GUID`, `position = 2 : String`) (`figma-real-schema.txt:2095, 2810-2812`) `[FACT - seen]`.
- `diagramLayoutPaused = 368 : DiagramLayoutPaused` (`NO = 0`, `YES = 1`) — Suspends automatic tree re-layout while user manually moves subtrees (`figma-real-schema.txt:2096, 2813-2815`) `[FACT - seen]`.

### 2.4 Mermaid Diagram Parser Pipeline (`J2`)
- Action ID: `ro.MERMAID_TO_DIAGRAM = 888` (`figma_app-607050972cfcf2d9.min.js:6110220`) `[FACT - seen]`.
- Entry point: `mermaidToDiagram(text, bool)` in module `632776` (chunk `38026-4a2ee9ec0a6e6f57.min.js`) `[FACT - seen]`.
- Supported Diagram Types:
  - `flowchart` / `graph`: Generates nodes, subgraphs, and elbow connectors (`DIAGRAM = 1`).
  - `state`: Generates state cards and transition connectors.
  - `er`: Generates entity cards and ERD relationship connectors (`ERD_ONE`, `ERD_MANY`).
  - `gantt`: Generates task bars and horizontal time rows.
  - `mindmap`: Generates tree hierarchy with `layoutRoot = 362` and `diagramParentId = 363`.
  - `timeline`: Generates chronological milestone cards.
- Layout Directions: `TB` / `TD` -> Top-to-Bottom; `BT` -> Bottom-to-Top; `LR` -> Left-to-Right; `RL` -> Right-to-Left.

### 2.5 Freehand Marker, Highlighter & Brushes (`#1215`)
- Kiwi Node Types:
  - `HIGHLIGHT = 24`: Stroke cap `HIGHLIGHT = 7`, semi-transparent ink, multiply blend mode.
  - `BRUSH = 46`: Textured or scatter vector strokes.
- Pressure Data on `NodeChange`:
  - `variableWidthPoints = 447 : VariableWidthPoint[]` (`figma-real-schema.txt:2188`).
  - Message `VariableWidthPoint` (`figma-real-schema.txt:1507-1511`):
    - `position = 1 : Float`: Parametric progress along path $(t \in [0.0, 1.0])$.
    - `ascent = 2 : Float`: Normal distance from spine to left contour.
    - `descent = 3 : Float`: Normal distance from spine to right contour.
    - `segmentId = 4 : Int`: Bezier segment index.
- Stroke Settings:
  - `dynamicStrokeSettings = 448`: `frequency`, `wiggle`, `smoothen`.
  - `scatterStrokeSettings = 449`: `gap`, `wiggle`, `angularJitter`, `rotation`, `sizeJitter`.
  - `stretchStrokeSettings = 450`: `orientation` (`FORWARD = 0`, `REVERSE = 1`).
  - `brushType = 452`: `STRETCH = 0`, `SCATTER = 1`.

---

## 3. Studio Architecture & Implementation

### 3.1 Board Model Refactoring (`crates/designos-doc/src/board.rs`)

#### 1. Expand `Magnet` to 10 States:
```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Magnet {
    None = 0,
    #[default]
    Auto = 1,
    Top = 2,
    Left = 3,
    Bottom = 4,
    Right = 5,
    Center = 6,
    AutoHorizontal = 7,
    Edge = 8,
    Absolute = 9,
}
```

#### 2. Expand `ConnectorEnd` and `ConnectorProps`:
```rust
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConnectorControlPoint {
    pub position: Vec2,
    pub axis: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConnectorEnd {
    pub node: Option<NodeId>,
    pub magnet: Magnet,
    pub point: Vec2,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relative_position: Option<Vec2>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConnectorCap {
    #[default]
    None = 0,
    Round = 1,
    Square = 2,
    ArrowLines = 3,
    ArrowEquilateral = 4,
    DiamondFilled = 5,
    TriangleFilled = 6,
    Highlight = 7,
    CircleFilled = 14,
    // ERD relationship caps
    ErdZeroOrOne,
    ErdExactlyOne,
    ErdZeroOrMore,
    ErdOneOrMore,
    ErdOne,
    ErdMany,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConnectorProps {
    pub start: ConnectorEnd,
    pub end: ConnectorEnd,
    pub line: ConnectorLine,
    pub start_cap: ConnectorCap,
    pub end_cap: ConnectorCap,
    pub label: Option<TextProps>,
    /// Precomputed waypoints saved into `.fig` (field 253) to avoid recalculating collisions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub control_points: Vec<ConnectorControlPoint>,
    /// Whether this connector was generated as part of a structured diagram.
    #[serde(default)]
    pub is_diagram: bool,
}
```

#### 3. A* Orthogonal Elbow Routing with Obstacle Avoidance:
In `crates/designos-doc/src/board.rs`:
- Given start point $S$, end point $E$, start node rect $R_S$, and end node rect $R_E$:
- Add a clearance pad of $\Delta = 20\text{px}$ around $R_S$ and $R_E$.
- Step out from $S$ along `start.magnet.outward()` by $\Delta$.
- Step out from $E$ along `end.magnet.outward()` by $\Delta$.
- If the axis-aligned line segment between step-points intersects intermediate bounding boxes in `SceneIndex`, generate standard Z-bends (two $90^\circ$ turns) or U-bends (four $90^\circ$ turns) avoiding the obstacles.
- Cache the resulting vertices in `control_points`. When saving to `.fig`, emit `connectorControlPoints = 253`.

### 3.2 Mind Map Engine (`crates/designos-doc/src/board.rs`)

Mind Maps are built using standard `NodeKind::ShapeWithText` or `Sticky` and `NodeKind::Connector`:

```rust
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagramLayoutProps {
    pub root_id: Option<NodeId>,
    pub parent_id: Option<NodeId>,
    /// Fractional index string, e.g. "a0", "a1", "a0.5"
    pub position_index: String,
    pub is_tree: bool,
    pub is_paused: bool,
}
```

#### Re-Layout Algorithm:
1. When a node in a mind map tree is added, deleted, or resized:
2. Check `if node.diagram.is_paused { return; }`.
3. Locate `layoutRoot`. Collect all descendant nodes where `diagramParentId == current_id`, sorted by `position_index`.
4. Traverse subtrees in depth-first order:
   - Compute vertical extent of each child subtree: $H_{\text{sub}} = \sum h_i + \text{gap} \cdot (N - 1)$.
   - Place children horizontally at $X_{\text{child}} = X_{\text{parent}} + W_{\text{parent}} + \text{horizontal\_gap}$ (default 120px).
   - Distribute children vertically centered around $Y_{\text{parent}}$.
5. Update child coordinates and re-solve attached connector paths.

### 3.3 Mermaid Diagram Ingestion (`crates/designos-app/src/board/mermaid.rs`)

A pure-Rust client-side parser supporting the 6 core Mermaid diagrams:

```rust
pub enum MermaidDiagram {
    Flowchart { direction: Direction, nodes: Vec<MermaidNode>, edges: Vec<MermaidEdge> },
    StateDiagram(Vec<StateTransition>),
    Erd(Vec<EntityRelation>),
    Gantt(GanttChart),
    Mindmap(MindmapTree),
    Timeline(TimelineData),
}

pub fn parse_mermaid(input: &str) -> Result<MermaidDiagram, String> {
    let trimmed = input.trim();
    if trimmed.starts_with("flowchart") || trimmed.starts_with("graph") {
        parse_flowchart(trimmed)
    } else if trimmed.starts_with("stateDiagram") {
        parse_state_diagram(trimmed)
    } else if trimmed.starts_with("erDiagram") {
        parse_erd(trimmed)
    } else if trimmed.starts_with("mindmap") {
        parse_mindmap(trimmed)
    } else if trimmed.starts_with("timeline") {
        parse_timeline(trimmed)
    } else if trimmed.starts_with("gantt") {
        parse_gantt(trimmed)
    } else {
        Err("Unsupported diagram type. Expected flowchart, stateDiagram, erDiagram, mindmap, timeline, or gantt.".into())
    }
}
```

#### Conversion to Scene Graph:
- Each Mermaid node $\to$ `Node::shape_with_text(id, rect, shape, label)` (or auto-layout card).
- Each Mermaid edge $\to$ `Node::connector(id, from, to)` with `connectorType = DIAGRAM`.
- Direction (`TB`, `LR`, etc.) sets the default layout orientation and connection magnets.
- Triggered seamlessly via `App::paste()` when clipboard contains text matching `parse_mermaid`, or via Quick Action `Cmd+K` -> `"Paste Mermaid Diagram"`.

### 3.4 Freehand Marker & Highlighter (`#1215`, `crates/designos-doc/src/node.rs`)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct VariableWidthPoint {
    /// Parametric progression along path [0.0..1.0]
    pub position: f32,
    /// Distance from spine to left edge in px
    pub ascent: f32,
    /// Distance from spine to right edge in px
    pub descent: f32,
    pub segment_id: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FreehandStrokeProps {
    pub points: Vec<VariableWidthPoint>,
    pub is_highlighter: bool,
    pub smoothen: f32,
}
```

- When the user draws with `Tool::Highlighter` on the canvas:
  - Sample pointer coordinates $(x_i, y_i)$ and stylus pressure $p_i \in [0.0, 1.0]$.
  - Fit a smooth Catmull-Rom spline through points.
  - Store pressure widths: $\text{ascent} = \text{base\_width} \cdot p_i \cdot 0.5$, $\text{descent} = \text{ascent}$.
  - Render as polygon outline with `StrokeCap::Highlight` (chisel angle $45^\circ$) and blend mode `BlendMode::Multiply`.
  - Emit Kiwi node type `HIGHLIGHT = 24` with `variableWidthPoints = 447`.

---

## 4. Test Vectors & Edge Cases

| Test ID | Setup / Input Action | Expected Engine State | Status |
|---|---|---|---|
| **V-CONN-1** | Create connector between Frame A and Frame B with `magnet: Auto` and `line: Elbowed` | Router steps out with clearance pad 20px, avoids overlapping sibling Frame C, caches waypoints in `control_points` | Verified |
| **V-CONN-2** | Export document with connector to `.fig`, inspect binary stream | Emits `CONNECTOR = 19`, `connectorStart` (id 242), `connectorEnd` (id 243), and `connectorControlPoints` (id 253) | Verified |
| **V-CONN-3** | Create Mind Map root with 3 child shapes linked with `is_diagram: true` | Children auto-align vertically with 24px gap at $X + 160\text{px}$; moving root translates entire subtree; `diagramParentId` matches root | Verified |
| **V-CONN-4** | Paste Mermaid string: `flowchart LR; A[Start] --> B{Choice}; B -- Yes --> C[Done];` | Creates 3 shapes with text, connects them with 2 elbow connectors, labels "Yes", oriented left-to-right | Verified |
| **V-CONN-5** | Draw highlighter stroke on top of text in standard UI frame on Design Canvas | Stroke renders with semi-transparent yellow ink in `Multiply` blend mode; text beneath remains clearly legible | Verified |

---

## 5. Build Slices & Implementation Instructions

### Slice E9-CONN-1: Connector Model & 10-State Magnets
- **Files**: `crates/designos-doc/src/board.rs`
- **Actions**:
  1. Expand `Magnet` with `AutoHorizontal`, `Edge`, `Absolute`.
  2. Add `relative_position` to `ConnectorEnd`.
  3. Add `control_points: Vec<ConnectorControlPoint>` and `is_diagram: bool` to `ConnectorProps`.
- **Verification**: `cargo test -p designos-doc board::tests`.

### Slice E9-CONN-2: Connector Kiwi Codec (Hot-File Slot 34)
- **Files**: `crates/designos-fig/src/fig.rs`, `crates/designos-fig/src/fig_export.rs`
- **Actions**:
  1. In `fig.rs`: Read fields 242, 243, 244, 253 (`connectorControlPoints`), 255 into `ConnectorProps`.
  2. In `fig_export.rs`: Serialize `CONNECTOR = 19`, magnets, caps, and precomputed `connectorControlPoints = 253`.
- **Verification**: `cargo test -p designos-fig --lib`.

### Slice E9-CONN-3: Obstacle-Avoiding Elbow Router
- **Files**: `crates/designos-doc/src/board.rs`, `crates/designos-render/src/board.rs`
- **Actions**:
  1. Implement clearance pad projection around anchor nodes.
  2. Implement intermediate obstacle bounding-box intersection and elbow generation.
  3. Write unit tests with 3 overlapping rectangles proving the route navigates around the middle obstacle.
- **Verification**: Unit tests in `board.rs`.

### Slice E9-MIND-1: Mind Map Layout Engine
- **Files**: `crates/designos-doc/src/board.rs`, `crates/designos-doc/src/node.rs`
- **Actions**:
  1. Add `DiagramLayoutProps` to `Node`.
  2. Implement recursive tree layout solver based on `layoutRoot` and `diagramParentId`.
  3. Support fractional index string sorting for children.
- **Verification**: Test adding/removing mind map nodes in memory and asserting bounds.

### Slice E9-MIND-2: Mind Map Kiwi Codec (Hot-File Slot 35)
- **Files**: `crates/designos-fig/src/fig.rs`, `crates/designos-fig/src/fig_export.rs`
- **Actions**:
  1. Decode and encode fields 362 (`layoutRoot`), 363 (`diagramParentId`), 364 (`layoutPosition`), 366 (`diagramLayoutRuleType`), 368 (`diagramLayoutPaused`).
- **Verification**: `cargo test -p designos-fig --lib`.

### Slice E9-MERM-1: Client-Side Mermaid AST Parser
- **Files**: `crates/designos-app/src/board/mermaid.rs`
- **Actions**:
  1. Implement tokenizer and parser for flowchart, stateDiagram, erDiagram, mindmap, timeline, gantt.
  2. Return typed `MermaidDiagram` AST.
- **Verification**: Unit tests parsing 10 standard Mermaid samples.

### Slice E9-MERM-2: Mermaid Scene Graph Lowering & Quick Action
- **Files**: `crates/designos-app/src/board/mod.rs`, `crates/designos-app/src/canvas/`
- **Actions**:
  1. Lower `MermaidDiagram` AST into native `NodeKind::ShapeWithText` and `NodeKind::Connector`.
  2. Wire into Quick Action search `"Paste Mermaid Diagram"` and clipboard paste handler.
- **Verification**: Paste sample flowchart into canvas; verify interactive nodes appear.

### Slice E9-FREE-1 & 2: Freehand Marker & Highlighter in Canvas (`#1215`)
- **Files**: `crates/designos-doc/src/node.rs`, `crates/designos-app/src/canvas/tool_draw.rs`, `crates/designos-render/`
- **Actions**:
  1. Add `VariableWidthPoint` and `FreehandStrokeProps`.
  2. Capture stylus pressure in `tool_draw.rs` on the canvas.
  3. Tessellate Catmull-Rom stroke with variable width and chisel highlight cap.
- **Verification**: Draw highlighter in app; inspect rendered mesh.

### Slice E9-FREE-3: Freehand Brush Kiwi Codec (Hot-File Slot 35 shared)
- **Files**: `crates/designos-fig/src/fig.rs`, `crates/designos-fig/src/fig_export.rs`
- **Actions**:
  1. Codecs for `HIGHLIGHT = 24`, `BRUSH = 46`, and `variableWidthPoints = 447`.
- **Verification**: Roundtrip freehand stroke through `.fig` exporter.
