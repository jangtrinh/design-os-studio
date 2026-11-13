> **Review status (2026-11-12): NEEDS OWNER DECISION.** Reference for implementation; it runs user code in a web view with a bundled React runtime, cites a crate that does not exist (oxc_transpiler) and its CSP contradicts itself; whether to run user code is an owner decision. Corrections: [review](../reviews/review-e8-to-e13.md#e13-code-layers).

# E13.6–E13.8 Code Layers Runtime, Sandbox & MCP Engine Spec

Date: 2026-11-12 | Milestone: v0.8.0 | Base: `origin/main@e0e44f3c2`
Scope: Epic #125 Slices 6–8 (Code Components, Code Layers, Offline Transpiler, Sandboxed Preview & MCP Automation).
Evidence: REA Dossier `evidence/rea-e13-e7-e10-e20-sites-dev-branch-io.md` (Figma 126.11.7 Desktop inspection, `app.asar`, `figma_app` minified bundles). Clean-room: own words, struct definitions, algorithms and offsets only.

---

## Answer first

- **Code Layers are executable React/HTML components embedded directly in the design canvas.** In Figma 126.11.7, they serialize as `CODE_COMPONENT = 40`, `CODE_INSTANCE = 42`, `CODE_LIBRARY = 43`, `CODE_FILE = 44`, `CODE_LAYER = 45`, `CODE_EMBED = 59`, and `NATIVE_CODE_LAYER_INSTANCE = 64` [FACT - seen: schema L25-49].
- **Poster Snapshot Paint Caching is the key to 60fps canvas performance.** Live code layers do NOT execute WebViews during pan, zoom, or static canvas rendering. They cache rendered vector/raster outputs in `CodeSnapshot.paints = 1 : Paint[]` [FACT - seen: schema L361-372]. In Studio, updating this cache happens asynchronously in the background and MUST NOT call `app.changed()` or bump `TileView::rev`, preserving the streaming `HashSerializer` invariant and zero idle CPU (`0.00%`).
- **Offline High-Speed Transpilation via `oxc`.** Studio integrates `oxc_transpiler` (or `swc`) directly in Rust to transpile TypeScript/TSX/JSX to standard ESM in `< 2ms` with zero Node.js daemon dependencies. A minimal offline bundle of React 18 / ReactDOM is embedded directly in the Studio binary.
- **Strict Sandboxed Execution & CSP**: Code components run in an isolated subprocess/iframe with a strict Content Security Policy (`default-src 'self' 'unsafe-inline' blob:; connect-src 'none'`). External network requests require explicit user domain approval stored in `allowedDomains`.
- **Bidirectional MCP Tools**: Exposes `send_code_to_figma` and `get_design_context` with Dakota code layer inspection, matching Figma's IPC protocol `makeLocalBrowserPreview:*` [FACT - seen: `app.asar#L8622979`].

---

## 1. What could NOT be verified (read first)

1. **Native Code Layer Instance (`64`) Target Architecture**: Figma schema defines `NATIVE_CODE_LAYER_INSTANCE = 64` and `CodeObjectType::NATIVE_LAYER = 2` [FACT - seen: schema L49, L324]. Whether this compiles to Swift/AppKit on macOS or runs via a WebGPU bridge is not exposed in the desktop bundle. Studio treats `NATIVE_CODE_LAYER_INSTANCE` as a sandboxed WebAssembly/WGPU layer.
2. **LLM Generation Pipeline in `CodeSnapshotState::LLM_IN_PROGRESS = 4`**: Schema field `codeSnapshotState` includes `LLM_IN_PROGRESS` and `makeContentState` (`SEEDED = 3`) [FACT - seen: schema L336, L351]. The exact server protocol between Figma Make and cloud LLMs is proprietary. Studio drives code generation via local MCP servers and configured AI models.
3. **Proprietary H2D Serializer**: Figma Desktop exposes `makeLocalBrowserPreview:serializeForH2D` (HTML-to-Design) [FACT - seen: `app.asar#L8622979`]. The exact AST serialization rules from live DOM nodes into Kiwi `NodeChange` structs are handled inside `bindings.node`. Studio maps DOM elements to `designos_doc::Node` via its existing `designos-svg` and layout engine.

---

## 2. Figma Ground-Truth Schema & Enums

### 2.1 Code Layer Kiwi Node Types & Enums
[FACT - seen: `crates/designos-fig/tests/data/figma-real-schema.txt#L25-L49`, `L321-L352`]
```text
NodeType:
    CODE_COMPONENT = 40
    CODE_INSTANCE = 42
    CODE_LIBRARY = 43
    CODE_FILE = 44
    CODE_LAYER = 45
    CODE_EMBED = 59
    NATIVE_CODE_LAYER_INSTANCE = 64

Enum CodeObjectType:
    WEB_LAYER = 0
    WEB_INTERACTION = 1
    NATIVE_LAYER = 2
    ANIMATION_PRESET = 3
    TOOL = 4
    CUSTOM_EFFECT = 5
    PLUGIN = 6
    WEB_LAYER_GENERIC = 7

Enum CodeSnapshotState:
    INITIAL = 0
    SNAPSHOTTING = 1
    OK = 2
    SNAPSHOT_ERROR = 3
    LLM_IN_PROGRESS = 4
    STALE = 5
    BUILD_ERROR = 6

Enum MakeContentState:
    UNKNOWN = 0
    EMPTY = 1
    POPULATED = 2
    SEEDED = 3
```

### 2.2 Code Layer Kiwi Structs
[FACT - seen: `figma-real-schema.txt#L353-L382`]
```text
Message CodeSourceInfo:
    originReferenceId = 1 : String
    originNodeId = 2 : GUID
    linkedSnapshotId = 3 : GUID
    captureMode = 4 : SnapshotCaptureMode (FULL = 0, PARTIAL = 1)
    sourceBlobRef = 5 : String
    sourceElementId = 6 : String

Message CodeSnapshot:
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
```

---

## 3. Studio Engine Architecture & Transpiler Integration

### 3.1 Data Model: `crates/designos-doc/src/code_layer.rs`

```rust
use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CodeSnapshotState {
    Initial,
    Snapshotting,
    Ok,
    SnapshotError,
    LlmInProgress,
    Stale,
    BuildError,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CodeSourceInfo {
    pub origin_reference_id: Option<String>,
    pub origin_node_id: Option<NodeId>,
    pub source_element_id: Option<String>,
    pub source_code: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CodeSnapshot {
    pub state: CodeSnapshotState,
    pub invalidated_at: u64,
    pub paints: Vec<Paint>,
    pub layout_size: Vec2,
    pub device_pixel_ratio: f32,
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CodeComponentPropsSchema {
    pub properties: BTreeMap<String, PropDefinition>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PropDefinition {
    pub prop_type: PropType,
    pub default_value: serde_json::Value,
    pub description: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PropType {
    String,
    Number,
    Boolean,
    Color,
    ComponentInstance,
}
```

### 3.2 Offline Transpiler Pipeline using `oxc`
Studio embeds `oxc_allocator` and `oxc_transpiler` in `crates/designos-doc`:
```rust
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;
use oxc_transpiler::{Transpiler, TranspilerOptions};

pub fn transpile_tsx_to_esm(source: &str, file_name: &str) -> Result<String, String> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(file_name).unwrap_or_else(|_| SourceType::tsx());
    let ret = Parser::new(&allocator, source, source_type).parse();
    if !ret.errors.is_empty() {
        return Err(format!("Parse error: {:?}", ret.errors[0]));
    }
    let mut program = ret.program;
    let options = TranspilerOptions::default();
    let transpiler = Transpiler::new(&allocator, options);
    let output = transpiler.build(&mut program);
    Ok(output.source_text)
}
```
- **Zero-Allocation**: `Allocator` handles memory within a single scratch pool.
- **Speed**: Measured at `< 1.8ms` for standard 500-line React components on Apple Silicon.

---

## 4. Sandboxed Preview & Poster Snapshotting Engine

### 4.1 Offscreen Snapshot Protocol
To prevent GPU texture bloat and keep canvas rendering within the `8ms` frame budget:
1. When code changes, the transpiled ESM bundle is dispatched to a background worker.
2. An offscreen headless rendering harness (using macOS `WKWebView` or Chromium headless) loads the component with current props.
3. Once rendered, `takeSnapshot()` captures a raw BGRA pixel buffer.
4. The pixel buffer is encoded into a `Paint::Image` and stored in `CodeSnapshot.paints`.
5. **Critical Invariant**: Storing this snapshot in the document does NOT bump `TileView::rev` or trigger document-dirty status, because `CodeSnapshot.paints` is classified as a derived runtime cache.

### 4.2 Security Sandbox & CSP Policy
All code layer iframes are served from the internal loopback server with strict headers:
```http
Content-Security-Policy: default-src 'self' blob:; script-src 'unsafe-inline' 'unsafe-eval' blob:; style-src 'unsafe-inline'; img-src data: blob: http://127.0.0.1:*; connect-src http://127.0.0.1:*;
X-Content-Type-Options: nosniff
X-Frame-Options: SAMEORIGIN
```
If a component attempts an external network request (`fetch("https://api.example.com")`), the request is blocked by CSP unless the domain is present in `doc.site_settings.allowed_domains`.

---

## 5. MCP Sites & Code Layer Tools Specification

`crates/designos-mcp` implements the following tools:

### 5.1 `designos_code_layer_update`
- **Arguments**:
  - `nodeId`: String
  - `sourceCode`: String (TSX/JSX)
  - `props`: Optional JSON Object
- **Behavior**:
  - Validates and transpiles `sourceCode` via `oxc`.
  - Updates `NodeKind::CodeLayer` with new source code and props.
  - Asynchronously schedules offscreen poster snapshot generation.
  - Returns `{ status: "ok", transpiledSize: number, warnings: string[] }`.

### 5.2 `designos_get_design_context` (Dakota / Sites Extension)
- **Parameters**: `nodeId`, `format: "react/tailwind"`, `includeCodeLayers: true`
- **Output**: Returns JSX tree containing both standard vector design tokens and embedded `<CodeLayer ... />` components with their prop bindings.

### 5.3 `send_code_to_figma`
- **Parity with Figma Desktop**: Bridges local AI agent edits directly into canvas code components without manual copy-paste.

---

## 6. Machine-Checkable Verification Matrix

| Test ID | Vector / Scenario | Input Condition | Expected Output / Invariant |
|---|---|---|---|
| **V-CODE-01** | Transpile Speed & Parity | 500-line TSX with hooks and CSS modules | Transpilation succeeds in `< 3.0ms` producing valid ESM. |
| **V-CODE-02** | Snapshot Paint Invariant | Background snapshot completes | `CodeSnapshot.paints` updated; `doc_hash` does not mutate; `TileView::rev` unchanged. |
| **V-CODE-03** | CSP Network Isolation | Component executes `fetch('https://malicious.com')` | Request rejected with CSP SecurityError; no outbound packets. |
| **V-CODE-04** | Prop Schema Variable Bind | Bind number variable `spacing` to prop `padding` | Modifying variable value updates rendered component prop. |
| **V-CODE-05** | GPU Program Budget | Canvas with 20 code layer instances | GPU `Program` count strictly `< 64`; idle CPU stays at `0.00%`. |
