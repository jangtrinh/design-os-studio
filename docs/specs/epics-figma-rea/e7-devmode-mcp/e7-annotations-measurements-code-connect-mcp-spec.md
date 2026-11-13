> **Review status (2026-11-12): READY AFTER FIXES.** Reference for implementation; the 33-value annotation enum is sourced, but three REST names are wrong, the Code Connect model contradicts issue #259 and the rule that repo files win, and the .fig annotation codec is missing. Corrections: [review](../reviews/review-e4-e5-e6-e7.md#e7-annotations).

# E7 Dev Mode, Annotations, Measurements & Code Connect MCP Spec

Date: 2026-11-12 | Milestone: v0.8.0 | Base: `origin/main@e0e44f3c2`
Scope: Epic #20 (Dev Mode Parity, 33 Annotation Properties, Categories, Measurements, Real Code Connect, MCP Tools).
Evidence: REA Dossier `evidence/rea-e13-e7-e10-e20-sites-dev-branch-io.md` (Figma 126.11.7 Kiwi schema lines 1581–1664, `app.asar`, `figma_app` JS). Clean-room: own words, struct definitions, algorithms and offsets only.

---

## Answer first

- **Figma Dev Mode defines exactly 33 `AnnotationPropertyType` enum values (`0..32`).** Studio's `crates/designos-doc/src/dev.rs` currently implements 22 properties [FACT - seen: `dev.rs:50-73`]. We expand this to all 33 values, adding the 11 missing grid, min/max, layout, and component properties [FACT - seen: schema L1581-1614].
- **Annotation Categories with Presets & Colors**: Supports category tagging with 5 system presets (`ACCESSIBILITY`, `BEHAVIOR`, `CONTENT`, `DEVELOPMENT`, `INTERACTION`) and 8 preset colors (`YELLOW` through `GREEN`), plus custom labels and colors [FACT - seen: schema L1617-1648].
- **Precise Measurement Overlays (`MEASUREMENT`)**: Serializes `AnnotationMeasurement` with `fromNode`, `toNode`, `AnnotationMeasurementNodeSide` (`TOP=0`, `BOTTOM=1`, `LEFT=2`, `RIGHT=3`), `toSameSide`, `innerOffsetRelative`, `outerOffsetFixed`, `toNodeStablePath`, and `freeText` [FACT - seen: schema L1649-1664].
- **Replace the Inspect Stub with First-Class Code Connect**: Replace the placeholder in `crates/designos-app/src/panels/inspect.rs:501-552` (which encoded Code Connect into annotation text strings) with a dedicated `crates/designos-doc/src/code_connect.rs` registry in `Document`.
- **Full MCP Parity**: Enhance `designos_get_design_context` with `format: "json"` XPath/query selector support, and add 4 dedicated Code Connect MCP tools (`designos_code_connect_map`, `designos_code_connect_resolve`, `designos_code_connect_list`, `designos_code_connect_lint`).

---

## 1. What could NOT be verified (read first)

1. **GUID Path Serialization in Measurement (`toNodeStablePath`)**: Schema field `toNodeStablePath = 8 : GUIDPath` [FACT - seen: schema L510] records a path through component and nested instance hierarchies. While its top-level GUIDs are verified, the resolution behavior when an intermediate component is detached or renamed is inferred to fall back to direct `toNode` GUID resolution.
2. **Figma Cloud Code Connect CLI Handshake**: Figma's `@figma/code-connect` CLI uploads AST definitions to Figma cloud servers. In Studio, Code Connect mappings are stored directly in `Document` (`Document.code_connect`) and can be synced with local codebases via the MCP bridge or Git worktrees.

---

## 2. Figma Ground-Truth Schema & Enums

### 2.1 Complete 33 `AnnotationPropertyType` Enum (`0..32`)
[FACT - seen: `crates/designos-fig/tests/data/figma-real-schema.txt#L1581-L1614`]
```text
Enum AnnotationPropertyType:
    FILL = 0
    STROKE = 1
    WIDTH = 2
    HEIGHT = 3
    MIN_WIDTH = 4
    MIN_HEIGHT = 5
    MAX_WIDTH = 6
    MAX_HEIGHT = 7
    STROKE_WIDTH = 8
    CORNER_RADIUS = 9
    EFFECT = 10
    TEXT_STYLE = 11
    TEXT_ALIGN_HORIZONTAL = 12
    FONT_FAMILY = 13
    FONT_SIZE = 14
    FONT_WEIGHT = 15
    LINE_HEIGHT = 16
    LETTER_SPACING = 17
    STACK_SPACING = 18
    STACK_PADDING = 19
    STACK_MODE = 20
    STACK_ALIGNMENT = 21
    OPACITY = 22
    COMPONENT = 23
    FONT_STYLE = 24
    GRID_ROW_GAP = 25
    GRID_COLUMN_GAP = 26
    GRID_ROW_COUNT = 27
    GRID_COLUMN_COUNT = 28
    GRID_ROW_ANCHOR_INDEX = 29
    GRID_COLUMN_ANCHOR_INDEX = 30
    GRID_ROW_SPAN = 31
    GRID_COLUMN_SPAN = 32
```

### 2.2 Annotation Categories & Measurement Schema
[FACT - seen: `figma-real-schema.txt#L1617-L1664`]
```text
Enum AnnotationCategoryPreset:
    NONE = 0
    ACCESSIBILITY = 1
    BEHAVIOR = 2
    CONTENT = 3
    DEVELOPMENT = 4
    INTERACTION = 5

Enum AnnotationCategoryColor:
    YELLOW = 0
    ORANGE = 1
    RED = 2
    PINK = 3
    VIOLET = 4
    BLUE = 5
    TEAL = 6
    GREEN = 7

Message AnnotationMeasurement:
    id = 1 : GUID
    fromNode = 2 : GUID
    toNode = 3 : GUID
    fromNodeSide = 4 : AnnotationMeasurementNodeSide (TOP=0, BOTTOM=1, LEFT=2, RIGHT=3)
    toSameSide = 5 : Bool
    innerOffsetRelative = 6 : Float
    outerOffsetFixed = 7 : Float
    toNodeStablePath = 8 : GUIDPath
    freeText = 9 : String
```

---

## 3. Studio Engine Implementation

### 3.1 Expanded Data Model: `crates/designos-doc/src/dev.rs`

Replace `ANNOTATION_PROPERTIES: [(&str, &str); 22]` with the exhaustive 33-entry array:
```rust
/// All 33 Figma annotation property types, matching Kiwi enum 0..32 exactly.
pub const ANNOTATION_PROPERTIES: [(&str, &str); 33] = [
    ("Fill", "fills"),
    ("Stroke", "strokes"),
    ("Width", "width"),
    ("Height", "height"),
    ("Min width", "minWidth"),
    ("Min height", "minHeight"),
    ("Max width", "maxWidth"),
    ("Max height", "maxHeight"),
    ("Stroke weight", "strokeWeight"),
    ("Corner radius", "cornerRadius"),
    ("Effects", "effects"),
    ("Text style", "textStyleId"),
    ("Text align", "textAlignHorizontal"),
    ("Font", "fontFamily"),
    ("Font size", "fontSize"),
    ("Font weight", "fontWeight"),
    ("Line height", "lineHeight"),
    ("Letter spacing", "letterSpacing"),
    ("Gap", "itemSpacing"),
    ("Padding", "padding"),
    ("Layout mode", "layoutMode"),
    ("Alignment", "primaryAxisAlignItems"),
    ("Opacity", "opacity"),
    ("Component", "componentId"),
    ("Font style", "fontStyle"),
    ("Grid row gap", "gridRowGap"),
    ("Grid column gap", "gridColumnGap"),
    ("Grid row count", "gridRowCount"),
    ("Grid column count", "gridColumnCount"),
    ("Grid row anchor", "gridRowAnchorIndex"),
    ("Grid column anchor", "gridColumnAnchorIndex"),
    ("Grid row span", "gridRowSpan"),
    ("Grid column span", "gridColumnSpan"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnnotationCategoryPreset {
    None,
    Accessibility,
    Behavior,
    Content,
    Development,
    Interaction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnnotationCategoryColor {
    Yellow,
    Orange,
    Red,
    Pink,
    Violet,
    Blue,
    Teal,
    Green,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnnotationCategory {
    pub id: String,
    pub preset: AnnotationCategoryPreset,
    pub color: AnnotationCategoryColor,
    pub custom_color: Option<Color>,
    pub label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeasurementSide {
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Measurement {
    pub id: String,
    pub from_node: NodeId,
    pub to_node: NodeId,
    pub from_side: MeasurementSide,
    pub to_same_side: bool,
    pub inner_offset_relative: f32,
    pub outer_offset_fixed: f32,
    pub free_text: Option<String>,
}
```

### 3.2 Real Code Connect Model: `crates/designos-doc/src/code_connect.rs`

```rust
use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CodeConnectMapping {
    pub node_id: NodeId,
    pub component_name: String,
    pub source_path: String,
    pub template: String,
    pub props: BTreeMap<String, CodeConnectPropMapping>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CodeConnectPropMapping {
    pub code_prop_name: String,
    pub design_prop_name: String,
    pub prop_kind: CodeConnectPropKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CodeConnectPropKind {
    Value,
    Boolean,
    Enum,
    Children,
}
```

### 3.3 Inspect Panel Integration (`crates/designos-app/src/panels/inspect.rs`)
Remove lines 501–534 that append synthetic annotations to `n.dev.annotations`. Instead:
1. Query `doc.code_connect.get(&id)`.
2. If mapped, display status badge `"Connected"` with the real `source_path` and `component_name`.
3. If unmapped, display `"Not started"` with one-click action `"Connect Component"` which opens the Code Connect template editor.

---

## 4. MCP Tools Specification

### 4.1 `designos_get_design_context` Enhancements
- **XPath / JSON Query Support**: When `format: "json"` is supplied, query specific subtrees with XPath selectors (e.g. `//Button[@variant='primary']`).
- **Asset Directory Writing**: When `dirForAssetWrites` is provided, automatically exports embedded SVG/PNG assets to disk and substitutes relative file paths into generated code.
- **Code Connect Snippet Injection**: Automatically substitutes mapped Code Connect templates into generated React/Tailwind/SwiftUI code.

### 4.2 Dedicated Code Connect MCP Tools
1. **`designos_code_connect_map`**:
   - Parameters: `nodeId` (String), `componentName` (String), `sourcePath` (String), `template` (String), `props` (Object).
   - Registers a durable Code Connect binding in the document.
2. **`designos_code_connect_resolve`**:
   - Parameters: `nodeId` (String).
   - Returns the rendered code snippet with real design tokens and variant values substituted.
3. **`designos_code_connect_list`**:
   - Returns all active Code Connect mappings across the file.
4. **`designos_code_connect_lint`**:
   - Scans components for missing Code Connect mappings, mismatched prop types, or stale source file paths.

---

## 5. Machine-Checkable Verification Matrix

| Test ID | Vector / Scenario | Input Condition | Expected Output / Invariant |
|---|---|---|---|
| **V-DEV-01** | All 33 Annotation Properties | Parse `.fig` with Grid row/col and Component properties | All 33 properties imported with bit-exact type IDs `0..32`. |
| **V-DEV-02** | Measurement Round-Trip | Measurement with `innerOffsetRelative = 0.5`, `outerOffsetFixed = 16.0` | Imported, serialized, and exported with matching coordinates. |
| **V-DEV-03** | Category Color Presets | Category with preset `ACCESSIBILITY` and color `TEAL` | Displays correct badge and persists to `.fig` / `.designos`. |
| **V-DEV-04** | Code Connect Real Storage | Map component via `designos_code_connect_map` | Stored in `doc.code_connect`; annotations array remains clean. |
| **V-DEV-05** | MCP Context with Code Connect | Request `designos_get_design_context` with `useCodeConnect: true` | Emits custom `<Button variant="primary" />` instead of raw div/css. |
