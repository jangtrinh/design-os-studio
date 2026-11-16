# Epic E4: Draw & Vector Engine Specs (#17)

Date: 2026-11-12 | Milestone: v0.8.0 | Base: `origin/main`  
Evidence: Clean-room REA Analysis Dossier `ev_6f56bed2b1984cdf5d746450517171fe32bc951557c54a88dce368fd3db6b704` (SHA-256: `e24f67a66fea09b8e13275a2bf75dcab4e9675a22de1b2b6c2b08deab41029b1`).  
Clean Room: Zero copied Figma source code. Verified Kiwi schema field numbers, TypeScript typings, WASM string exports, mathematical formulations, and Studio Rust codebase anchors.

---

## Answer first

- **Architecture Invariant (`AGENTS.md` §4)**: Every stroke expansion (inside, outside, center, offset, variable-width, and path-trimmed) must be synthesized as shared fill geometry in `designos-render/src/outline.rs` so that both the CPU renderer (`tiny-skia`) and the GPU renderer (`lyon` in `designos-gpu/src/geom.rs`) execute bit-exact identical raster silhouettes with **zero new offscreen GPU `Program` allocations**.
- **Multi-Stroke & Vector Stacking**: Figma 126.11.7 stores multi-strokes in `strokeData = 571 : StrokeData` (`Stroke[]`) on `NodeChange`. Fills render first (bottom-to-top), followed by strokes rendered sequentially in index order (`0` at bottom, `N-1` on top). Inside/outside alignments on closed shapes execute doubled-weight (`weight * 2.0`) stencil-masked clipping against the shape boundary (`DestinationIn` for inside, `DestinationOut` for outside). `StrokeAlign::OFFSET = 3` shifts the path normal by `strokeOffset = 13 : Float` before stroke tessellation.
- **Vector Mask Types (#1506)**: Kiwi `maskType = 317 : MaskType` supports `ALPHA = 0`, `OUTLINE = 1` (plugin API `'VECTOR'`), and `LUMINANCE = 2` ($Y = 0.2126R + 0.7152G + 0.0722B$). For `OUTLINE`, only the closed fill silhouette clips the target subtree; stroke paints and stroke expansions are stripped unless the mask node has an empty fill, in which case the stroke outline alone acts as an opaque binary cutter.
- **Interactive Vector Tools (#193, #194, #499)**:
  - Eraser (`#193`): Circle-brush stroke boundary subtracted from underlying VectorNetwork paths via `i_overlay` boolean difference.
  - Scissors/Cut (`#194`): Splits bezier segments at parametric $t \in [0, 1]$ projection, breaking closed loops into open segments or disconnecting network vertices (`split_vector`).
  - Offset Path (`#499`): Dual-contour polygon dilation/erosion via `i_overlay` winding offset with round/square join parameters, automatically flattening variable-width profiles with user notification bell.
- **Figma Draw Engine (#179–#206)**:
  - `variableWidthPoints = 447`: Array of `(position: Float, ascent: Float, descent: Float, segmentId: Int)`. Invariant enforcer forbids variable width on dashed strokes and branching vector networks.
  - `pathTrim = 542`: Arc-length parameter range $[t_{\text{start}}, t_{\text{end}}] \in [0.0, 1.0]$.
  - `NodeType::TRANSFORM = 48` & `transformModifiers = 455`: Max 2 modifiers (`REPEAT` linear/radial $2 \dots 40$, `SYMMETRY`, `SKEW`). Tessellated once on CPU/GPU and instanced via 2D affine matrices.
  - `PaintType::PATTERN = 8`: Clones referenced canvas node (`sourceNodeId = 26`) into an LRU byte-capped tile texture cache rendered through existing `IMG_TILE`.
  - `NodeType::BRUSH = 46`: Procedural stamps (`SCATTER`) or deformed ribbons (`STRETCH`) seeded by deterministic `strokeSeed = 482 : Uint64`.
  - `NodeType::TEXT_PATH = 41`: Binds text glyphs along arbitrary `VectorNetwork` baseline using cumulative arc-length parameterization.

---

## 1. What could NOT be verified (read first)

1. **WASM Bezier Subdivision Numerical Tolerance in Offset Path**: The exact epsilon threshold used inside Figma's C++ `FGPathTrim.cpp` / `OffsetPathBehavior` for arc-length re-parameterization is not exported as a symbol. Studio specifies a chordal tolerance of $\epsilon = 0.25\,\text{px}$, which delivers visual indistinguishability up to $800\%$ canvas zoom.
2. **Brush Deformation Mesh Density for STRETCH Brushes**: Figma's WASM engine deforms vector brush artwork along the spline skeleton. The exact tessellation quad count per segment is dynamic in WASM. Studio adopts adaptive curvature-dependent subdivision (min 8 quads, max 64 quads per cubic segment).
3. **Branching Network Partial Trimming**: When `pathTrim` is applied to a `VectorNetwork` with branching junctions (degree $> 2$), Figma's UI disables the control (`branching_vector_network_warning`). The underlying behavior if forged via plugin API is unverified; Studio follows the enforcer rule and treats degree $> 2$ networks as un-trimmable.
4. **No Live Figma Runtime Inspection**: In compliance with clean-room guidelines, no live Figma process was hooked or traced. All findings derive strictly from static REA artifacts (Kiwi schema, WASM string tables, TypeScript definition files, UI translation dictionaries).

---

## 2. Status & Roadmap Table

| Slice / Issue | Summary | Size (h) | Recommended Model Tier | Hot-File Lock | Dependencies | Acceptance Gate |
|---|---|---|---|---|---|---|
| **E4-P1.1 (#952)** | Inside/Outside Stroke Parity & Stencil Clipping on Ellipse & Polygons | M (8h) | `Gemini 3.8 Flash` | None (`outline.rs`, `geom.rs`) | None | Bit-exact tiny-skia & wgpu inside/outside strokes on Ellipse & Star |
| **E4-P1.2 (#953)** | `ArcData` (`startingAngle`, `endingAngle`, `innerRadius`), `starInnerScale`, `LINE` Matrix Math | S (4h) | `Gemini 3.8 Flash` | `fig.rs`, `fig_export.rs` (Slice 1) | None | Round-trip donut ellipses, partial arcs, 5-point stars, zero-height lines |
| **E4-P1.3 (#1506)** | Vector Mask Engine (`maskType` 0:ALPHA, 1:OUTLINE, 2:LUMINANCE) | M (6h) | `Gemini 3.8 Flash` | `fig.rs` (Slice 2) | None | Closed fill outline masking; luminance grey formula; stroke stripping |
| **E4-P2.1 (#196/#197)** | Multi-Stroke Architecture (`strokeData = 571`, `StrokeAlign::OFFSET`, `strokeOffset`, Side Weights) | L (14h) | `Gemini 4 Argon` | `fig.rs`, `fig_export.rs` (Slice 3) | E4-P1.1 | Bottom-to-top stroke stacking; offset stroke rendering; per-side weights |
| **E4-P2.2 (#193)** | Vector Eraser Tool (`i_overlay` boolean difference on active network) | M (8h) | `Gemini 4 Argon` | None (`canvas/tools/`, `vector.rs`) | E4-P1.1 | Erasing across open & closed paths; vertex insertion at cut points |
| **E4-P2.3 (#194)** | Vector Scissors / Cut Tool (`split_vector`, break network into paths) | M (6h) | `Gemini 3.8 Flash` | None (`canvas/tools/`, `vector.rs`) | None | Split segment at click point; join selection; disconnect junctions |
| **E4-P2.4 (#499)** | Offset Path Tool (`i_overlay` dilation/erosion, round/square joins) | M (8h) | `Gemini 4 Argon` | None (`canvas/tools/`, `geom.rs`) | E4-P2.1 | Offset slider range $[-100, 100]$; round/square joins; VWS outline toast |
| **E4-P3.1 (D1)** | Variable Width Strokes (`variableWidthPoints = 447`, 7 Presets, Enforcer) | L (16h) | `Gemini 4 Argon` | `fig.rs`, `fig_export.rs` (Slice 4) | E4-P1.1 | Variable width tessellation; dashed/branching conflict toasts; presets |
| **E4-P3.2 (D1)** | Path Trim Engine (`pathTrim = 542`, `Stroke.pathTrim = 12`, $[0..1]$ Arc-Length) | M (8h) | `Gemini 3.8 Flash` | `fig.rs`, `fig_export.rs` (Slice 5) | E4-P1.1 | Arc-length parameterization; start/end trim handles; dashed stroke trim |
| **E4-P3.3 (D2)** | Transform Modifiers (`TRANSFORM = 48`, `transformModifiers = 455`, Repeat 2..40) | L (14h) | `Gemini 4 Argon` | `fig.rs`, `fig_export.rs` (Slice 6) | E4-P1.1 | Linear & radial repeat; symmetry; skew; max 2 modifiers; GPU instancing |
| **E4-P3.4 (D2)** | Pattern Fills (`PaintType::PATTERN = 8`, Tile Types, LRU Cache) | M (10h) | `Gemini 3.8 Flash` | `fig.rs`, `fig_export.rs` (Slice 7) | None | Rect/Hexagonal pattern tiling; 2D spacing; dynamic node update in tile |
| **E4-P3.5 (D4)** | Procedural Brush Engine (`BRUSH = 46`, Scatter & Stretch, Seeded PRNG) | L (16h) | `Gemini 4 Argon` | `fig.rs`, `fig_export.rs` (Slice 8) | E4-P3.1 | Scatter jitter & gap; stretch ribbon deformation; deterministic seed |
| **E4-P3.6 (D4)** | Text on Path (`TEXT_PATH = 41`, `textPathStart = 432`, Arc-Length Projection) | L (14h) | `Gemini 4 Argon` | `fig.rs`, `fig_export.rs` (Slice 9) | E4-P3.2 | Glyph baseline alignment (L/C/R/J); vertical alignment (T/C/B); flip |

---

## 3. Order and Hot-File Queue Rules

### Serialization Hot-File Queue Rule (`AGENTS.md`)
The files `crates/designos-fig/src/fig.rs`, `crates/designos-fig/src/fig_export.rs`, `crates/designos-fig/src/kiwi.rs`, and `crates/designos-fig/src/rest_import.rs` are high-contention files. **Only one sub-agent may edit these files at any given time**.

The serialized slice queue for Epic E4 is:
1. **Slice 1 (E4-P1.2)**: `ArcData` (195), `starInnerScale` (24), `LINE` resize transform.
2. **Slice 2 (E4-P1.3)**: `maskType` (317), `maskIsOutline` (18).
3. **Slice 3 (E4-P2.1)**: Multi-stroke `strokeData` (571), `StrokeAlign::OFFSET` (3), `strokeOffset` (13), per-side weights (295–299).
4. **Slice 4 (E4-P3.1)**: `variableWidthPoints` (447).
5. **Slice 5 (E4-P3.2)**: `pathTrim` (542) on NodeChange and `Stroke.pathTrim` (12).
6. **Slice 6 (E4-P3.3)**: `NodeType::TRANSFORM` (48), `transformModifiers` (455).
7. **Slice 7 (E4-P3.4)**: `PaintType::PATTERN` (8), `sourceNodeId` (26), `patternTileType` (28), `patternSpacing` (37).
8. **Slice 8 (E4-P3.5)**: `NodeType::BRUSH` (46), `brushType` (452), `dynamicStrokeSettings` (448), `scatterStrokeSettings` (449), `strokeSeed` (482).
9. **Slice 9 (E4-P3.6)**: `NodeType::TEXT_PATH` (41), `textPathStart` (432).

### Parallelizable Streams (Zero Hot-File Lock)
The following tasks can proceed in parallel without touching the serialization hot files:
- **Stream A (Render Core)**: `designos-render/src/outline.rs` (Inside/Outside stencil clipping, offset strokes, variable width expansion, path trimming).
- **Stream B (GPU Tessellation)**: `designos-gpu/src/geom.rs` (Lyon variable width stroke builder, instance transform buffer updates).
- **Stream C (Interactive Tools)**: `designos-app/src/canvas/tools/` (Eraser `#193`, Scissors `#194`, Offset Path `#499`).

---

## 4. Rollback and Migration Strategy

- **Additive Serde Deserialization**: All newly added fields in `designos-doc` (`variable_width_points`, `path_trim`, `stroke_data`, `mask_type`, etc.) must be annotated with `#[serde(default, skip_serializing_if = "Option::is_none")]` or `#[serde(default, skip_serializing_if = "Vec::is_empty")]`. Older `.designos` files load without syntax errors.
- **Graceful Fallback**: If an older version of Studio opens a `.designos` file containing `NodeType::TRANSFORM` or `NodeType::BRUSH`, it falls back to rendering children via standard container traversal.
- **Git Reversion**: Every slice must be merged via squash commit. Rollback requires a single `git revert <commit>` with no cascading migration side-effects.
