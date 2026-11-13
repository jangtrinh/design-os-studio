# REA Evidence & Codebase Anchor Dossier: Epics E13, E7, E10, E20
**Target**: Figma 126.11.7 Clean-Room Extraction & DESIGN:OS Studio Architecture Anchors  
**Scope**:
- **Epic E13**: Sites & Code Layers (`#125`)
- **Epic E7**: Dev Mode Annotations, Measurements & Code Connect (`#20`)
- **Epic E10**: Versions & Branching (`#23`)
- **Epic E20**: Document Save, Load & Quit Reliability (`#1788`)

**REA Tool & Environment Evidence IDs**:
- `rea` CLI version: `6.8.0` (`/Users/jangtrinh/.local/bin/rea`) [FACT - seen: `rea --version`]
- `desktop_rust.node` Artifact Inspection: `evidence_id`: `ev_6e46a40427aaa73337a20f19d15178ba177de305369b3678052e2b449f09c899`, SHA-256: `9500c73119d7bdb3954c7784f80db2509e2143c0a9b476528b85b43dc7213dd7` [FACT - seen: REA inspect_artifact]
- `desktop_rust.node` Rust Dependency Inspection: `evidence_id`: `ev_0a3ecf805b5fd8f1997b86600e9ff72590df827cac94b671732c1219ac0169a6`, Rustc commit: `48a229ceaefd4985c50990b14116b6d856af0985` (file offset 2016239) [FACT - seen: REA inspect_rust_binary]
- `bindings.node` Artifact Inspection: `evidence_id`: `ev_eec532e1d3df92bcc1d09805e1286af0cc46b640eb71a15b911d79a2715edb3c`, SHA-256: `eed4ef3964fb7ae5faed1994a1ba54f86d471ff603555c3541984a32c5a49cdf` [FACT - seen: REA inspect_artifact]
- Staged Figma 126.11.7 JS Subgraph Analysis: `evidence_id`: `ev_909ce46afcf8f2ddc345a89052d503e9feb28b88515010e0394d853b80ec2cd4`, SHA-256: `75fb6690cafb963cad2d5131a1b8218cff8a04a1ce47b02c090f004ea708f4a4` [FACT - seen: REA analyze_javascript_application]

---

## Part 1: Epic E13 — Sites & Code Layers (`#125`)

### 1.1 Sites & Responsive Breakpoints: Exact Kiwi Schema Definitions

#### Exact Kiwi `NodeType` Enum Values in Figma 126.11.7
[FACT - seen: `crates/designos-fig/tests/data/figma-real-schema.txt#L79-L105`]
- `RESPONSIVE_SET = 39`
- `CODE_COMPONENT = 40`
- `TEXT_PATH = 41`
- `CODE_INSTANCE = 42`
- `CODE_LIBRARY = 43`
- `CODE_FILE = 44`
- `CODE_LAYER = 45`
- `BRUSH = 46`
- `MANAGED_STRING = 47`
- `TRANSFORM = 48`
- `CMS_RICH_TEXT = 49`
- `REPEATER = 50`
- `JSX = 51`
- `EMBEDDED_PROTOTYPE = 52`
- `REACT_FIBER = 53`
- `RESPONSIVE_NODE_SET = 54`
- `WEBPAGE = 55`
- `KEYFRAME = 56`
- `KEYFRAME_TRACK = 57`
- `ANIMATION_PRESET_INSTANCE = 58`
- `CODE_EMBED = 59`
- `BINARY_FILE = 60`
- `SPEC_BLOCK = 61`
- `TOOL_INSTANCE = 62`
- `CUSTOM_EFFECT_INSTANCE = 63`
- `NATIVE_CODE_LAYER_INSTANCE = 64`

#### EditorType Enum
[FACT - seen: `crates/designos-fig/tests/data/figma-real-schema.txt#L4104-L4114`]
```text
Enum EditorType
    DESIGN = 0
    WHITEBOARD = 1
    SLIDES = 2
    DEV_HANDOFF = 3
    SITES = 4
    COOPER = 5
    ILLUSTRATION = 6
    FIGMAKE = 7
    FIGSPEC = 8
```
Editor type for Figma Sites is explicitly `SITES = 4`.

#### Sites View State
[FACT - seen: `crates/designos-fig/tests/data/figma-real-schema.txt#L3452-L3458`]
```text
Enum SitesViewState
    FILE = 0
    CODE = 1
    DAKOTA = 2
    SETTINGS = 3
    INSERT = 4
    VARIABLES = 5
```
- `DAKOTA = 2` is Figma's internal engineering codename for Figma Sites canvas runtime.
- `NodeChange` field: `sitesViewState = 41 : SitesViewState`.

#### Breakpoint Edits & Propagation Mechanics (`Desktop` -> `Tablet` -> `Mobile`)
[FACT - seen: `crates/designos-fig/tests/data/figma-real-schema.txt#L1050-L1053`, `L2220-L2226`]
In `NodeChange`:
- `isPrimaryBreakpoint = 458 : Bool`
- `primaryResponsiveNodeId = 457 : GUID`
- `defaultResponsiveSetId = 398 : GUID`
- `multiEditGlueId = 462 : GUID`
- `breakpointMinWidth = 501 : Float`
- `isBreakpointInFocus = 522 : Bool`
- `derivedBreakpointData = 500 : DerivedBreakpointData`

In `Message DerivedBreakpointData`:
```text
Message DerivedBreakpointData
    overrides = 1 : NodeChange[]
```
[INFERENCE based on schema]: Figma does **not** create disconnected duplicate frames for secondary breakpoints. Secondary breakpoint nodes (`Tablet`, `Mobile`) carry `isPrimaryBreakpoint = false`, point their `primaryResponsiveNodeId = 457` to the `Desktop` primary breakpoint node GUID, and record differential edits in `derivedBreakpointData.overrides = 1 : NodeChange[]`. This directly reuses Figma's core symbol/instance override engine (`NodeChange[]` delta list) rather than introducing a separate ad-hoc cascading CSS engine.
[FACT - seen: `wasm-strings.txt#L44661`]: Canvas node hierarchy constraint: `"Cannot move node. Webpages must be direct children of the canvas"`. All `WEBPAGE` (`NodeType = 55`) nodes are top-level children of `CANVAS` (`NodeType = 2`).

#### Page Settings (`Message ResponsiveSetSettings`)
[FACT - seen: `crates/designos-fig/tests/data/figma-real-schema.txt#L2455-L2479`]
```text
Message ResponsiveSetSettings
    title = 1 : String
    description = 2 : String
    scalingMode = 3 : ResponsiveScalingMode
    scalingMinFontSize = 4 : Float
    scalingMaxFontSize = 5 : Float
    scalingMinLayoutWidth = 6 : Float
    scalingMaxLayoutWidth = 7 : Float
    lang = 8 : String
    faviconHash = 9 : String
    socialImageHash = 10 : String
    googleAnalyticsID = 11 : String
    blockSearchIndexing = 12 : Bool
    customCodeHeadStart = 13 : String
    customCodeHeadEnd = 14 : String
    customCodeBodyStart = 15 : String
    customCodeBodyEnd = 16 : String
    faviconID = 17 : GUID
    socialImageID = 18 : GUID
    addBypassLinks = 19 : Bool
    ignoreReducedMotion = 20 : Bool
    cookieBanner = 21 : CookieBannerSettings

Enum ResponsiveScalingMode
    REFLOW = 0
    SCALE = 1
```
- Custom code injection slots: `customCodeHeadStart`, `customCodeHeadEnd`, `customCodeBodyStart`, `customCodeBodyEnd`.
- SEO & indexing: `title`, `description`, `lang`, `faviconHash`/`faviconID`, `socialImageHash`/`socialImageID` (OpenGraph preview), `googleAnalyticsID`, `blockSearchIndexing`.
- Accessibility & Motion: `addBypassLinks = 19 : Bool` (skip-to-content accessibility link), `ignoreReducedMotion = 20 : Bool` (controls whether CSS animations bypass `prefers-reduced-motion` media queries).

---

### 1.2 Scroll Effects & Animation Behaviors

[FACT - seen: `crates/designos-fig/tests/data/figma-real-schema.txt#L4428-L4510`]
In `NodeChange`: `behaviors = 404 : NodeBehaviors`.

```text
Message NodeBehaviors
    link = 1 : LinkBehavior
    appear = 2 : AppearBehavior
    hover = 3 : HoverBehavior
    press = 4 : PressBehavior
    focus = 5 : FocusBehavior
    scrollParallax = 6 : ScrollParallaxBehavior
    scrollTransform = 7 : ScrollTransformBehavior
    cursor = 8 : CursorBehavior
    marquee = 9 : MarqueeBehavior
    code = 10 : CodeBehavior[]
```

#### Appear on Scroll (`AppearBehavior`)
```text
Enum AppearBehaviorTrigger
    PAGE_LOAD = 1
    THIS_LAYER_IN_VIEW = 2
    OTHER_LAYER_IN_VIEW = 3
    SCROLL_DIRECTION = 4

Enum RelativeDirection
    UP = 1
    DOWN = 2
    LEFT = 3
    RIGHT = 4

Message AppearBehavior
    trigger = 1 : AppearBehaviorTrigger
    direction = 2 : RelativeDirection
    otherLayer = 3 : GUID
    enterTransition = 4 : BehaviorTransition
    enterState = 5 : NodeChange
    exitTransition = 6 : BehaviorTransition
    exitState = 7 : NodeChange
    playsOnce = 8 : Bool
    playsOnceVar = 9 : VariableData
    isDeleted = 10 : Bool
```

#### Parallax (`ScrollParallaxBehavior`)
```text
Message ScrollParallaxBehavior
    axis = 1 : ScrollDirection
    speed = 2 : Float
    relativeToPage = 3 : Bool
    speedVar = 4 : VariableData
    isDeleted = 5 : Bool
```
`ScrollDirection` enum values: `NONE = 0`, `HORIZONTAL = 1`, `VERTICAL = 2`, `BOTH = 3`.

#### Scroll Transform (`ScrollTransformBehavior`)
```text
Enum ScrollTransformBehaviorTrigger
    PAGE_HEIGHT = 1
    THIS_LAYER_IN_VIEW = 2
    OTHER_LAYER_IN_VIEW = 3

Message ScrollTransformBehavior
    trigger = 1 : ScrollTransformBehaviorTrigger
    otherLayer = 2 : GUID
    transition = 3 : BehaviorTransition
    fromState = 4 : NodeChange
    toState = 5 : NodeChange
    playsOnce = 6 : Bool
    playsOnceVar = 7 : Bool
    playsOnceVar2 = 8 : VariableData
    isDeleted = 9 : Bool
```

#### Marquee / Ticker (`MarqueeBehavior`)
```text
Message MarqueeBehavior
    direction = 1 : RelativeDirection
    speed = 2 : Float
    shouldLoopInfinitely = 3 : Bool
    speedVar = 4 : VariableData
    shouldLoopInfinitelyVar = 5 : VariableData
    pauseOnHover = 6 : VariableData
    isDeleted = 7 : Bool
```

#### Sticky Headers
[FACT - seen: `crates/designos-fig/tests/data/figma-real-schema.txt#L1519-L1522`, `L2145`]
In `NodeChange`: `scrollBehavior = 186 : ScrollBehavior`.
```text
Enum ScrollBehavior
    SCROLLS = 0
    FIXED_WHEN_CHILD_OF_SCROLLING_FRAME = 1
    STICKY_SCROLLS = 2
```
Sticky positioning on sites is specified by `scrollBehavior = 2` (`STICKY_SCROLLS`).

---

### 1.3 CMS & Code Layers

#### CMS Collection, Filter & Consumption Model
[FACT - seen: `crates/designos-fig/tests/data/figma-real-schema.txt#L2480-L2530`]
- `NodeChange` fields:
  - `cmsSelector = 419 : CMSSelector`
  - `cmsConsumptionMap = 420 : CMSConsumptionMap`
  - `cmsRichTextStyleMap = 460 : CMSRichTextStyleMap`
  - `cmsTarget = 5 : CMSItemPageTarget`
  - `repeaterCmsOverrideData = 540 : RepeaterCmsOverrideData`
- Structs:
```text
Message CMSSelector
    cmsCollectionId = 1 : String
    filterCriteria = 2 : CMSFilterCritera
    sorts = 3 : CMSSelectorSort[]
    limit = 4 : Uint

Message CMSFilterCritera
    matchType = 1 : CMSFilterCriteriaMatchType (MATCH_ALL = 0, MATCH_ANY = 1)
    filters = 2 : CMSSelectorFilter[]

Message CMSSelectorFilter
    cmsFieldId = 1 : String
    op = 2 : CMSSelectorFilterOperator (EQUALS = 0)
    comparisonValue = 3 : String

Message CMSSelectorSort
    cmsFieldId = 1 : String
    orderBy = 2 : CMSFieldOrderBy (ASCENDING = 0, DESCENDING = 1)

Message CMSConsumptionMap
    entries = 1 : CMSConsumptionMapEntry[]

Message CMSConsumptionMapEntry
    consumptionField = 1 : CMSConsumptionField (MISSING = 0, TEXT_DATA = 1)
    cmsFieldId = 2 : String

Message CMSRichTextStyleMap
    entries = 1 : CMSRichTextStyleEntry[]

Message CMSRichTextStyleEntry
    styleClass = 1 : CMSRichTextStyleClass
    textDescriptor = 2 : CMSRichTextDescriptor

Enum CMSRichTextStyleClass
    HEADING1 = 0
    HEADING2 = 1
    HEADING3 = 2
    HEADING4 = 3
    HEADING5 = 4
    HEADING6 = 5
    PARAGRAPH = 6
    LINK = 7
    BLOCKQUOTE = 8

Message RepeaterCmsOverrideData
    overrides = 1 : NodeChange[]
```

#### Code Layers & Sandboxed Preview Architecture
[FACT - seen: `crates/designos-fig/tests/data/figma-real-schema.txt#L558-L590`, `L2230-L2280`, `L2370-L2405`]
- `NodeChange` Code Layer Fields:
  - `sourceCode = 414 : String`
  - `collaborativeSourceCode = 444 : CollaborativePlainText`
  - `sourceCodeCollaborativeTextVersion = 534 : CollaborativeTextOpID[]`
  - `belongsToCodeLibraryId = 427 : CodeLibraryId`
  - `importedCodeFiles = 467 : ImportedCodeFiles`
  - `codeFileCanvasNodeId = 468 : CanvasNodeId`
  - `isEntrypointCodeFile = 498 : Bool`
  - `backingCodeComponentId = 429 : CodeComponentId`
  - `isMainCodeComponent = 487 : Bool`
  - `codeSnapshotState = 431 : CodeSnapshotState`
  - `codeSnapshot = 459 : CodeSnapshot`
  - `codeSnapshotLayers = 589 : CodeSnapshotLayers`
  - `codeObjectType = 516 : CodeObjectType`
  - `codeBehaviorData = 478 : CodeBehaviorData`
  - `codeEmbedInfo = 532 : CodeEmbedInfo`
  - `codeSourceInfo = 558 : CodeSourceInfo`
  - `makeContentState = 595 : MakeContentState`
  - `isCodePreviewPlayingOnCanvas = 527 : Bool`

- Enums & Structs:
```text
Enum CodeObjectType
    WEB_LAYER = 0
    WEB_INTERACTION = 1
    NATIVE_LAYER = 2
    ANIMATION_PRESET = 3
    TOOL = 4
    CUSTOM_EFFECT = 5
    PLUGIN = 6
    WEB_LAYER_GENERIC = 7

Enum CodeSnapshotState
    INITIAL = 0
    SNAPSHOTTING = 1
    OK = 2
    SNAPSHOT_ERROR = 3
    LLM_IN_PROGRESS = 4
    STALE = 5
    BUILD_ERROR = 6

Enum CachedPreviewStatus
    UNKNOWN = 0
    PENDING = 1
    COMPLETE = 2
    FAILED = 3
    ERROR = 4

Enum MakeContentState
    UNKNOWN = 0
    EMPTY = 1
    POPULATED = 2
    SEEDED = 3

Message CodeSourceInfo
    originReferenceId = 1 : String
    originNodeId = 2 : GUID
    linkedSnapshotId = 3 : GUID
    captureMode = 4 : SnapshotCaptureMode (FULL = 0, PARTIAL = 1)
    sourceBlobRef = 5 : String
    sourceElementId = 6 : String

Message CodeSnapshot
    state = 6 : CodeSnapshotState
    invalidatedAt = 7 : Uint
    cachedPreviewStatus = 8 : CachedPreviewStatus
    commitSha = 9 : String
    capturedRoute = 10 : String
    paints = 1 : Paint[]
    offset = 2 : Vector
    layoutSize = 3 : Vector
    canvasSize = 5 : Vector
    devicePixelRatio = 4 : Uint

Message CodeOutputLayer
    node = 1 : NodeChange
    sourceNodeId = 2 : GUID

Message CodeSnapshotLayers
    state = 1 : CodeSnapshotState
    invalidatedAt = 2 : Uint
    cachedPreviewStatus = 4 : CachedPreviewStatus
    layers = 3 : CodeOutputLayer[]
```
[INFERENCE on poster image caching]: `CodeSnapshot.paints = 1 : Paint[]` stores rendered raster/vector snapshot paints of the code layer output directly in Kiwi format. When canvas rendering is idle or in test mode, the renderer draws `paints` without spinning up the sandboxed web preview.

#### Sandboxed Iframe / Bundler IPC Channels
[FACT - seen: `app.asar` byte offset 8622979, and JS bundle 70678-a0ad44f0702a3649.min.js]:
Figma Desktop 126.11.7 communicates with the local bundler / preview process via Electron IPC under `makeLocalBrowserPreview:*`:
- `makeLocalBrowserPreview:createLocalBrowserPreview`
- `makeLocalBrowserPreview:loadComponentDescriptor`
- `makeLocalBrowserPreview:invoke`
- `makeLocalBrowserPreview:reattachPickerAfterReload`
- `makeLocalBrowserPreview:prefetchElementPicker`
- `makeLocalBrowserPreview:stopElementPicker`
- `makeLocalBrowserPreview:clearElementPickerSelection`
- `makeLocalBrowserPreview:toggleDevTools`
- `makeLocalBrowserPreview:pointAndEditControl`
- `makeLocalBrowserPreview:sendKeyEvent`
- `makeLocalBrowserPreview:sendMouseEvent`
- `makeLocalBrowserPreview:sendWheelEvent`
- `makeLocalBrowserPreview:setToolbarOverlayActive`
- `makeLocalBrowserPreview:setCornerRadius`
- `makeLocalBrowserPreview:focusTextEditor`
- `makeLocalBrowserPreview:executeAnnotationScript`
- `makeLocalBrowserPreview:serializeForH2D` (HTML-to-Design serialization)
Associated runtime flags: `FIGCODE_BROWSER_PREVIEW_MCP_ENABLED`, `FIGCODE_FIGMA_MCP_PROXY_MODE`, `FIGCODE_MAKE_LOCAL_DEVSERVER_MCP_TOOLS_ENABLED`.

---

## Part 2: Epic E7 — Dev Mode, Annotations, Measurements & Code Connect (`#20`)

### 2.1 Exact 33 AnnotationPropertyType Enum Values
[FACT - seen: `crates/designos-fig/tests/data/figma-real-schema.txt#L1581-L1614`]
```text
Enum AnnotationPropertyType
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
*Gap Analysis with Studio Codebase*: `crates/designos-doc/src/dev.rs` currently defines `ANNOTATION_PROPERTIES` with 22 properties. Missing in Studio: `COMPONENT (23)`, `FONT_STYLE (24)`, `GRID_ROW_GAP (25)`, `GRID_COLUMN_GAP (26)`, `GRID_ROW_COUNT (27)`, `GRID_COLUMN_COUNT (28)`, `GRID_ROW_ANCHOR_INDEX (29)`, `GRID_COLUMN_ANCHOR_INDEX (30)`, `GRID_ROW_SPAN (31)`, `GRID_COLUMN_SPAN (32)`, `STACK_MODE (20)`, `STACK_ALIGNMENT (21)`.

### 2.2 Annotation Categories Schema
[FACT - seen: `crates/designos-fig/tests/data/figma-real-schema.txt#L1617-L1648`]
```text
Enum AnnotationCategoryPreset
    NONE = 0
    ACCESSIBILITY = 1
    BEHAVIOR = 2
    CONTENT = 3
    DEVELOPMENT = 4
    INTERACTION = 5

Enum AnnotationCategoryColor
    YELLOW = 0
    ORANGE = 1
    RED = 2
    PINK = 3
    VIOLET = 4
    BLUE = 5
    TEAL = 6
    GREEN = 7

Message AnnotationCategoryCustom
    color = 1 : AnnotationCategoryColor
    customColor = 2 : Color
    label = 3 : String

Message AnnotationCategory
    id = 1 : GUID
    preset = 2 : AnnotationCategoryPreset
    custom = 3 : AnnotationCategoryCustom

Message AnnotationCategories
    version = 1 : Uint
    items = 2 : AnnotationCategory[]

Message Annotation
    label = 1 : String
    properties = 2 : AnnotationProperty[]
    labelV2 = 3 : String
    categoryId = 4 : GUID
```

### 2.3 Measurements Schema (`MEASUREMENT`)
[FACT - seen: `crates/designos-fig/tests/data/figma-real-schema.txt#L1649-L1664`]
```text
Enum AnnotationMeasurementNodeSide
    TOP = 0
    BOTTOM = 1
    LEFT = 2
    RIGHT = 3

Message AnnotationMeasurement
    id = 1 : GUID
    fromNode = 2 : GUID
    toNode = 3 : GUID
    fromNodeSide = 4 : AnnotationMeasurementNodeSide
    toSameSide = 5 : Bool
    innerOffsetRelative = 6 : Float
    outerOffsetFixed = 7 : Float
    toNodeStablePath = 8 : GUIDPath
    freeText = 9 : String
```

### 2.4 DevStatus & Lifecycle
[FACT - seen: `crates/designos-doc/src/dev.rs#L16-L30`, `wasm-strings.txt#L13460`, `L44086-L44090`]
- Figma states: `READY_FOR_DEV`, `STATUS_COMPLETED`, `DEV_MODE_FOCUS_VIEW`, `CODE_CONNECT`.
- Studio status enum: `DevStatus::ReadyForDev`, `DevStatus::Completed`, `DevStatus::Changed`.
- Lifecycle in Studio:
  - `mark_ready(doc, id, now, snapshot_version)`: sets `DevStatus::ReadyForDev`, computes `subtree_fingerprint(doc, id)` and stores baseline `snapshot`.
  - `mark_completed(doc, id, now)`: sets `DevStatus::Completed`.
  - `refresh_statuses(doc, now)`: compares current subtree fingerprint with baseline; flips `ReadyForDev` -> `Changed` if contents changed, or back on undo.

### 2.5 Figma Desktop MCP & Code Connect Tools
[FACT - seen: `figma_app-607050972cfcf2d9.min.js#L6479789`, `app.asar#L3193473`]:
Figma Desktop MCP server exposes:
1. `get_design_context`:
   - Parameters: `fileKey` (string), `nodeId` (string), `format` (`"react/tailwind"` | `"json"`), `query` (optional XPath-like selector when `format="json"`), `depth` (number), `limit` (number), `forceCode` (boolean), `useCodeConnect` (boolean), `includeScreenshot` (boolean), `inlineScreenshot` (boolean), `maxDimension` (number, default 1024), `dirForAssetWrites` (string path).
   - Behavior: When `format="react/tailwind"`, returns generated React code with Code Connect snippet substitutions. When `format="json"`, returns structured `{ nodes, matchCount, truncated }`.
2. `get_metadata`: returns canvas structure and layer IDs without full vector/rendering payload.
3. `get_screenshot`: returns raster rendering of selected node.
4. `send_code_to_figma`: updates code layers in Figma Make / Dakota.
5. Internal MCP bridge methods: `internal_get_tools`, `internal_get_resources_list`, `internal_get_prompts_list`, `internal_get_resource`, `internal_get_prompt`.

---

## Part 3: Epic E10 — Versions & Branching (`#23`)

### 3.1 Exact Kiwi DiffPayload & DiffChunk Schema
[FACT - seen: `crates/designos-fig/tests/data/figma-real-schema.txt#L3656-L3678`]
```text
Enum DiffType
    BRANCHING = 0
    NODE_CHANGES_ONLY = 1

Message DiffPayload
    nodeChanges = 1 : NodeChange[]
    blobs = 2 : Blob[]
    diffChunks = 3 : DiffChunk[]
    diffBasis = 4 : NodeChange[]
    basisParentNodeChanges = 5 : NodeChange[]
    parentNodeChanges = 6 : NodeChange[]
    diffType = 7 : DiffType

Message DiffChunk
    nodeChanges = 1 : Uint[]
    phase = 2 : NodePhase
    displayNode = 3 : NodeChange
    canvasId = 4 : GUID
    canvasName = 5 : String
    canvasIsInternal = 6 : Bool
    chunksAffectingThisChunk = 7 : Uint[]
    basisParentHierarchy = 8 : NodeChange[]
    parentHierarchy = 9 : NodeChange[]
    basisParentHierarchyGuids = 10 : GUID[]
    parentHierarchyGuids = 11 : GUID[]
```

### 3.2 Branch Conflict Granularity & Resolution UI
[FACT - seen: `figma_app-6372f0021b0d5c94.min.en.json`]:
- **Conflict Categorization**:
  - `collaboration.conflict_multipage`: `"Multipage"`
  - `collaboration.conflict_local_styles`: `"Styles"`
  - `collaboration.conflict_local_variables`: `"Variables"`
  - `collaboration.conflict_annotation_categories`: `"Annotation categories"`
- **Diff Chunk Details**:
  - `branching.chunk_details.added`: `"Added"`
  - `branching.chunk_details.removed`: `"Removed"`
  - `branching.chunk_details.property_changed`: `"Property changed"`
  - `branching.chunk_details.property_added`: `"Property added"`
  - `branching.chunk_details.property_removed`: `"Property removed"`
  - `branching.chunk_details.component_set_properties_changed`: `"Component set properties changed"`
  - `branching.chunk_details.guid_property_modified`: `"{propertyLabel} modified"`
- **Resolution Options**:
  - Per-conflict selection: `collaboration.branching_from_source.pick_branch` (`"Pick branch"`), `collaboration.branching_from_source.pick_main_file` (`"Pick main file"`).
  - Bulk resolution: `collaboration.branching_force.merge_from_source_merge_conflict_radio_label` (`"Pick {option} changes for all conflicts"` with option `branch` or `main`), `collaboration.branching_from_source.resolve_all` (`"Resolve all"`).
  - Workflow directions:
    - `Update from main file` (`collaboration.branching_merge_modal.update_from_main_file`): rebase branch against updated main.
    - `Merge to main file` (`collaboration.branching_to_source.conflicts_must_be_resolved_to_merge_this_branch`): blocked if branch is behind main: `"Couldn't proceed with merge. Please accept updates from main before merging your branch."`

---

## Part 4: Epic E20 — Document Save, Load & Quit Reliability (`#1788`)

### 4.1 Sub-Issues Analysis Under Epic #1788
[FACT - seen: `gh issue view 1788` & sub-issues]:
1. **`#1648` (macOS quit hardening)**: Follow-up to PR `#1645`. In `crates/designos-app/src/macos_terminate.rs`:
   - If Studio is hidden with `Cmd+H` and user quits from Dock or logs out, window must unhide to display the unsaved changes modal.
   - Unwritten library edits currently cancel logout silently; must write or prompt.
   - `on_exit` callback runs twice per close on all platforms (`crates/designos-app/src/shell.rs`).
   - `applicationShouldTerminate:` callback lacks `catch_unwind`; panic must fall back to `NSTerminateNow`.
2. **`#1686` (Keep unopened pages compact in `.designos`)**:
   - Issue: Saving to `.designos` currently loads every page and expands all component instances (`#1670` fix), ballooning file size (e.g., Untitled UI kit explodes from 1.67 GB to 4.95 GB, save time jumps from 3.5s to 17s, instance count from 207k to 877k).
   - Expected: Preserve unexpanded compact page representations in `.designos` for pages never visited, expanding on demand upon first visit.
3. **`#1712` (Streaming reopen CPU)**:
   - Issue: Streaming JSON parser cut peak memory from 11.6 GB to 6.6 GB on large kits, but doubled reopen CPU time (33-36s vs 17-20s).
   - Options: Memory-mapped JSON parsing (`mmap` with `unsafe` block) or format change from `#1686`.
4. **`#1715` (Clone during page hydration saves empty pages)**:
   - In `crates/designos-doc/src/lib.rs` (page hydrator lock): cloning while hydrator is locked returns early without copying deferred page list, causing subsequent saves to write empty pages.
5. **`#1716` (Dropped copy blocks original save)**:
   - Dropping a temporary cloned document that loaded pages leaves the original refusing native saves with `"Another copy loaded these pages"`.
6. **`#1717` (Quit dropping queued work)**:
   - One-frame race window: `Cmd+S` queued work followed immediately by Dock Quit.
7. **`#1718` (Save to library copies whole doc on UI thread without size guard)**:
   - Ground anchor: `crates/designos-app/src/home/session.rs:958` calls `thumb::refresh_in_background(lib, e.id, self.export_doc().into_owned())`. Lines 579, 616, and 739 have `nodes.len() <= 25_000` guards; line 958 omits it, causing 5.9 GB memory spike.
8. **`#1719` (Queued Save writes whichever tab is active)**:
   - In `crates/designos-app/src/ops.rs`: queued save reads active tab at execution time instead of the tab active when `Cmd+S` was pressed.

### 4.2 Figma Desktop Save, Load & Quit Recovery Mechanisms
[FACT - seen: `wasm-strings.txt`, `app.asar`, `figma_app-6372f0021b0d5c94.min.en.json`]:
- **Lazy Page Loading**:
  - `loadSceneAndCanvasAsset`: Figma loads the scene index and active `CANVAS` asset first. Inactive `CANVAS` nodes are loaded on demand when user switches page (`Setting current page to new canvas`).
  - Webpages and layers on other canvases are not deserialized into active DOM/WASM scene memory until navigated to.
- **Autosave & Background Sync**:
  - `autosaveSubscribeToContainingPages`: autosave listeners attach only to pages containing modified nodes.
  - Offline / disk failure prompts: `autosave.unable_to_leave_document.unsaved_changes_save_in_background` (`"The current document has unsaved changes. Figma will continue trying to save in the background. Please try again later."`).
  - Unsafe operation blocking: `visual_bell.unsafe_with_unsaved_content` (`"This action is unsafe to perform with unsaved content. Allow document saving to complete, then try again"`).
- **Quit & Tab Close Protection**:
  - `desktop.dialogs.close_tab_unsaved_changes.title`: `"Close tab with unsaved changes?"`.
  - `desktop.dialogs.close_window_unsaved_changes.body`: `"Changes are currently being saved. Your changes will be lost if you close the window now."`.

---

## Part 5: DESIGN:OS Studio Codebase Anchors Matrix

| Area / Concern | File Anchor | Current Implementation Status | Gap / Required Epic Action |
|---|---|---|---|
| **Sites & Breakpoints (E13)** | `crates/designos-fig/src/fig.rs` | Reads `NodeType::RESPONSIVE_SET (39)` & `WEBPAGE (55)`. | Add `DerivedBreakpointData` parsing (`NodeChange[]` overrides) and `isPrimaryBreakpoint` linking. |
| **Sites Settings (E13)** | `crates/designos-doc/src/lib.rs` | Document-level settings lack web publishing metadata. | Implement `ResponsiveSetSettings` struct (SEO, head/body code injection, reduced motion, scaling mode). |
| **Scroll Effects (E13)** | `crates/designos-doc/src/lib.rs` | `NodeBehaviors` missing `appear`, `scrollParallax`, `scrollTransform`, `marquee`. | Map `AppearBehavior`, `ScrollParallaxBehavior`, and `MarqueeBehavior` into doc scene node metadata. |
| **Code Layers (E13)** | `crates/designos-doc/src/lib.rs` | `NodeType::CODE_COMPONENT (40)`, `CODE_LAYER (45)` registered in schema. | Implement `CodeSnapshot` (poster paint caching) and sandboxed iframe bridge matching `makeLocalBrowserPreview:*`. |
| **Annotations (E7)** | `crates/designos-doc/src/dev.rs:40` | `ANNOTATION_PROPERTIES` contains 22 properties. | Expand to all 33 `AnnotationPropertyType` enum values (grid gap/spans, font style, component). |
| **Measurements (E7)** | `crates/designos-doc/src/dev.rs` | No measurement overlay struct. | Add `AnnotationMeasurement` (`fromNode`, `toNode`, `AnnotationMeasurementNodeSide`, `innerOffsetRelative`, `outerOffsetFixed`). |
| **Dev Mode Status (E7)** | `crates/designos-doc/src/dev.rs:16` | `DevStatus` (`ReadyForDev`, `Completed`, `Changed`) fully implemented with subtree fingerprinting. | Parity complete; connect to `designos-mcp`'s `designos_set_dev_status`. |
| **MCP Tools (E7)** | `crates/designos-mcp/src/engine.rs:1` | 29 tools dispatched (`get_design_context`, `get_metadata`, `get_screenshot`, `query_nodes`, `lint`). | Ensure `get_design_context` supports `format: "json"` XPath query option and asset export directory. |
| **Diff & Branching (E10)** | `crates/designos-library/src/versions.rs` | Linear version snapshots (`FileVersion`). | Implement `DiffPayload` / `DiffChunk` schema for branching, conflict detection across styles/variables/pages. |
| **Autosave Loop (E20)** | `crates/designos-app/src/home/session.rs:730` | `Autosave` with zero-allocation `doc_hash` and `DiskWatcher`. | Solid base; resolve #1718 missing size guard on line 958. |
| **Page Hydration (E20)** | `crates/designos-doc/src/lib.rs:235` | `page_hydrator: Option<Arc<Mutex<dyn PageHydrator>>>`. | Address #1715 (cloning during hydration) and #1716 (dropped copy tracking). |
| **macOS Quit & Flush (E20)** | `crates/designos-app/src/macos_terminate.rs` | AppKit delegate `applicationShouldTerminate:`. | Address #1648 (hidden app unhide on quit, panic guard, double `on_exit`). |
| **Queued Save Tab Affinity (E20)** | `crates/designos-app/src/tabs.rs` / `ops.rs` | Queued saves resolve active tab at execution. | Address #1719 by capturing originating `TabId` at queue time. |
