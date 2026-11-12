# E13 Sites & Code Layers: Build-Ready Parity Roadmap & Architecture (2026-11-12)

Epic #125. Base `origin/main@e0e44f3c2`. Clean-room specification synthesized from Figma 126.11.7 binary analysis and Kiwi schema ground truth (`crates/designos-fig/tests/data/figma-real-schema.txt`).
Evidence: `evidence/rea-e13-e7-e10-e20-sites-dev-branch-io.md` (REA IDs `ev_6e46a404...`, `ev_0a3ecf80...`, `ev_eec532e1...`, `ev_909ce46a...`).
Every load-bearing statement is tagged `[FACT - seen]`, `[INFERENCE]`, `[ASSUMPTION]`, or `[UNKNOWN]`. No Figma code copied.

---

## Answer first

- **Figma Sites is a first-class editor mode (`EditorType::SITES = 4`), not a plugin.** Canvas pages in Sites mode hold `WEBPAGE` (`55`) nodes as top-level children of `CANVAS` (`2`) [FACT - seen: `wasm-strings.txt#L44661`]. Each webpage encapsulates a responsive set of viewport frames (`RESPONSIVE_SET = 39`) with primary-to-secondary breakpoint override inheritance.
- **Breakpoint edits reuse Figma's native component override engine (`NodeChange[]` delta list).** Secondary breakpoints (`Tablet`, `Mobile`) do NOT fork separate layout trees; they link to the primary breakpoint via `primaryResponsiveNodeId = 457` and store diffs in `derivedBreakpointData.overrides = 1 : NodeChange[]` [FACT - seen: schema L93-96].
- **Code Layers are executable web/native components on canvas.** They serialize as `CODE_COMPONENT = 40`, `CODE_INSTANCE = 42`, `CODE_LAYER = 45`, `CODE_EMBED = 59`, and `NATIVE_CODE_LAYER_INSTANCE = 64`. They cache raster/vector snapshots in `CodeSnapshot.paints = 1 : Paint[]` [FACT - seen: schema L361-372] so canvas rendering remains bit-exact and sub-8ms without executing JavaScript on idle frames.
- **Studio Implementation Roadmap (8 Slices in strict build order):**
  1. **Slice 1 (Core Model & Schema)**: Add `NodeType::RESPONSIVE_SET (39)`, `WEBPAGE (55)`, `EditorType::SITES (4)`, and breakpoint linkage (`isPrimaryBreakpoint`, `primaryResponsiveNodeId`, `derivedBreakpointData`) into `designos-doc` and `designos-fig`.
  2. **Slice 2 (Page & Site Settings)**: Implement `ResponsiveSetSettings` in `designos-doc` (SEO, `scalingMode` REFLOW/SCALE, code injection slots, a11y bypass links, reduced motion override).
  3. **Slice 3 (Scroll & Animation Behaviors)**: Implement `NodeBehaviors` (`appear`, `scrollParallax`, `scrollTransform`, `marquee`, and `STICKY_SCROLLS = 2`).
  4. **Slice 4 (CMS Collections & Repeater Overrides)**: Implement `CMSSelector`, `CMSConsumptionMap`, `CMSRichTextStyleMap`, and CSV/JSON ingestion into `RepeaterCmsOverrideData`.
  5. **Slice 5 (Zero-JS Semantic HTML/CSS Site Compiler)**: High-speed compiler in `designos-export` emitting clean semantic HTML5, pure CSS media queries (`375px`, `768px`, `1440px`), CSS sticky positioning, and WOFF2/WebP asset optimization.
  6. **Slice 6 (Code Layers Model & Offline Transpiler)**: Transpiler bridge in `designos-doc`/`designos-app` using `oxc`/`swc` for instant TSX -> ESM compilation with bundled offline React.
  7. **Slice 7 (Sandboxed Preview & Poster Snapshot Caching)**: Loopback server (`127.0.0.1:0` with ephemeral token) + offscreen headless snapshotter updating `CodeSnapshot.paints` asynchronously without invalidating `TileView::rev` or leaking memory.
  8. **Slice 8 (MCP Sites & Code Layer Automation Tools)**: Expose Dakota/Sites MCP tools (`designos_sites_build`, `designos_code_layer_update`, `send_code_to_figma`) on `designos-mcp`.

---

## 1. What could NOT be verified (read first)

1. **Exact Client-Side CMS Filter Bytecode**: While Kiwi schema defines `CMSFilterCriteria` with `matchType` (`MATCH_ALL=0, MATCH_ANY=1`) and `op: EQUALS=0`, advanced filtering operators (regex, range comparisons, date parsing) are implemented in proprietary server endpoints. Studio will implement clean local evaluation for standard equality and substring matches.
2. **Proprietary Dakota Layout Engine Nuances**: Figma's internal web preview runtime (codename `DAKOTA = 2`) uses a custom layout-to-flexbox translator. The exact AST mapping logic is obfuscated inside `compiled_wasm.wasm`. Studio leverages its existing bit-exact CSS/Tailwind flex compiler in `designos-export`.
3. **Hardware-Accelerated Code Layer Canvas Blending**: Whether Figma macOS Desktop uses Metal layer sharing or shared memory IPC for live interactive code layers is inferred from `makeLocalBrowserPreview:sendMouseEvent` [FACT - seen: `app.asar` byte offset 8622979]. Studio will strictly use offscreen headless snapshotting to respect the `< 64` GPU `Program` cap and `0.00%` idle CPU invariant.

---

## 2. Slice Order, Sizing & Model-Tier Routing

In accordance with the **Antigravity Sub-Agent Cost-Optimization Hard Rule**:
- **Carbon** (`gemini-4-carbon`): Architect, spec author, coordinator, and independent final verifier.
- **Pro / Argon** (`Model: "pro"`): Bounded-judgment builder for complex engine integration (Slices 1, 3, 5, 6, 7).
- **Flash** (`Model: "flash"`): Cost-optimal worker for mechanical data models, schema serialization, tests, CLI flags, and docs (Slices 2, 4, 8).

| Slice | Scope & Deliverable | Builder Hours | Assigned Model Tier | Hot Files / Lock Ownership | Dependencies |
|---|---|---|---|---|---|
| **E13.1** | Schema: `RESPONSIVE_SET (39)`, `WEBPAGE (55)`, Breakpoint Linkage & Overrides | 8 h | **Argon** (`Model: "pro"`) | `crates/designos-fig/src/fig.rs`, `kiwi.rs` (Hot Queue #1) | Base repo |
| **E13.2** | `ResponsiveSetSettings`: SEO, Custom Code Injection, A11y & Motion Settings | 4 h | **Flash** (`Model: "flash"`) | `crates/designos-doc/src/site.rs`, `crates/designos-fig/src/fig.rs` | After E13.1 |
| **E13.3** | Scroll & Animation Behaviors: `appear`, `parallax`, `transform`, `marquee`, `sticky` | 10 h | **Argon** (`Model: "pro"`) | `crates/designos-doc/src/behaviors.rs`, `crates/designos-fig/src/fig.rs` | After E13.1 |
| **E13.4** | CMS Schema, CSV Ingestion & `RepeaterCmsOverrideData` Engine | 6 h | **Flash** (`Model: "flash"`) | `crates/designos-doc/src/cms.rs` | After E13.1 |
| **E13.5** | High-Performance HTML/CSS Compiler: Media queries (`375/768/1440`), Sticky & WOFF2 | 12 h | **Argon** (`Model: "pro"`) | `crates/designos-export/src/html.rs`, `site_compiler.rs` | After E13.2, E13.3, E13.4 |
| **E13.6** | Code Layer Node Model & Fast Offline `oxc`/`swc` TSX Transpiler | 10 h | **Argon** (`Model: "pro"`) | `crates/designos-doc/src/code_layer.rs`, `Cargo.toml` | After E13.1 |
| **E13.7** | Sandboxed Preview: `127.0.0.1:0` Loopback Server & Async Snapshot Cache | 14 h | **Argon** (`Model: "pro"`) | `crates/designos-app/src/preview/`, `crates/designos-gpu/src/compile.rs` | After E13.6 |
| **E13.8** | MCP Sites & Code Layer Tools (`get_design_context`, `send_code_to_figma`) | 6 h | **Flash** (`Model: "flash"`) | `crates/designos-mcp/src/engine.rs`, `server.rs` | After E13.5, E13.7 |

---

## 3. Hot-File Queue & Concurrency Invariants

Hot files in Studio (`fig.rs`, `fig_export.rs`, `kiwi.rs`) must be modified serially to prevent merge skew and parser regressions:
1. **Serial Execution on `crates/designos-fig`**:
   - `E13.1` updates Kiwi schema numbers (`39`, `40..45`, `55`, `59`, `64`) and `DerivedBreakpointData` in `fig.rs` + `kiwi.rs`.
   - Once `E13.1` tests pass, `E13.2` and `E13.3` serialize their respective import/export blocks.
2. **Parallel Isolation**:
   - `E13.5` (`designos-export`) can be implemented in parallel with `E13.6` (`oxc` transpiler in `designos-doc`) since they share no crate files.
3. **GPU & Memory Invariants (`AGENTS.md` §4)**:
   - Headless snapshotting of Code Layers updates an offscreen paint buffer (`CodeSnapshot.paints`). It MUST NOT call `app.changed()` or bump `TileView::rev` on idle canvas frames.
   - Live loopback preview binds strictly to `127.0.0.1:0` with an OS-allocated ephemeral port and a random 128-bit bearer token. No public interfaces (`0.0.0.0`) are permitted.
   - GPU Program count must remain `< 64` and tessellation frame budget within `8ms`.

---

## 4. Acceptance Criteria & Test Verification Gates

Each slice must provide automated test evidence before completion:
1. **Round-Trip Test**: Kiwi binary import/export of a `.fig` file containing `RESPONSIVE_SET`, `WEBPAGE`, `DerivedBreakpointData`, and `CodeSnapshot` without data loss or unknown field warnings.
2. **Override Integrity Test**: Modifying text or fill on a Primary breakpoint (`Desktop`) propagates down to `Tablet` and `Mobile` unless an explicit override exists in `derivedBreakpointData`.
3. **Compiler Determinism Test**: Compiling a 3-breakpoint page produces W3C-valid HTML5 and CSS without external dependencies or tracking scripts.
4. **Offline Code Layer Transpile Test**: Transpiling a React 18 component with hooks and Tailwind classes completes in `< 5ms` with `oxc`.
5. **Idle CPU & Memory Gate**: Running a document with 10 code layers maintains `0.00%` idle CPU after the initial snapshot and under `64` GPU programs.
