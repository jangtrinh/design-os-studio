# Epic E6: Motion & Media Export (#19) — Master Architecture & Slice Roadmap

**Date:** 2026-11-12  
**Milestone:** v0.8.0  
**Target Architecture:** Figma 126.11.7 Clean-Room Parity vs. DESIGN:OS Studio (`design-os-studio`)  
**Base Revision:** `origin/main@e0e44f3c2`  
**Evidence Source:** `/Users/jangtrinh/Products/design-os-studio/plans/261112-epic-figma-rea-specs/evidence/rea-e6-e11-motion-proto.md`  
**REA Evidence ID:** `ev_d8d8b9b95732159bfc18d8f944c89602a78adcbc4ae41358f73fb2823755bbdd`  
**Clean-Room Standard:** Clean-room reverse engineering. Exact Kiwi struct/enum/field tags, schema byte offsets, analytical ODE closed forms, and UI strings in own words. Zero copied Figma source code.

---

## Answer first

1. **Epic E6 provides complete motion, timeline, animation export, and media export parity with Figma 126.11.7**, covering:
   - **#528 Export Bounds:** Distinction between `absoluteBoundingBox` (nominal geometric) and `absoluteRenderBounds` (painted effects spillover), with `useAbsoluteBounds = 9`, `contentsOnly = 7`, and `clipsContent` container clamping.
   - **#213 ExportSettings Round-Trip:** Full 18-field Kiwi `ExportSettings` message, supporting all image/media formats (PNG, JPG, SVG, PDF, MP4, GIF, WEBM, LOTTIE, DOTLOTTIE), constraints (SCALE, WIDTH, HEIGHT), and color profiles (DOCUMENT, SRGB, DISPLAY_P3_V4).
   - **#457 Lossless WebP Export:** Studio export addition of lossless WebP rasterization.
   - **#459 Analytical Easing & Spring ODEs:** Elimination of Penner approximations in favor of Figma's CSS standard beziers (`[0.42, 0, 1, 1]`, `[0, 0, 0.58, 1]`, `[0.42, 0, 0.58, 1]`, back curves), `HOLD` step function, and exact analytical closed-form ODE solutions for underdamped, overdamped, and critically damped springs with logarithmic $\epsilon = 0.001$ settle time.
   - **Motion Architecture & Presets:** Kiwi node types `KEYFRAME = 56`, `KEYFRAME_TRACK = 57`, `ANIMATION_PRESET_INSTANCE = 58`, pure evaluator `(doc, t_us) -> NodeOverrides`, 6 core primitives, ~60 composite presets, and `TextElementDelayBehaviour = 610` text-stagger engine.
   - **Lottie & Media Export:** Bodymovin/dotLottie AST generation, Figma's 13 export lint/rejection rules, animated SVG/CSS/React codegen, and hardware-accelerated video export (VideoToolbox / MediaFoundation).
2. **Studio Non-Negotiable Invariants (`AGENTS.md` §4):**
   - **Zero Rev Bumps in Playback:** Timeline scrubbing, motion playback, and frame export evaluation MUST NEVER call `app.changed()`, MUST NEVER bump `TileView::rev`, and MUST NEVER invoke `doc.to_json()` in `doc_hash`!
   - **GPU Tessellation Budget:** Transform and opacity tracks are applied via uniform buffers in WGSL, reusing cached `GeomCache` tessellations at $0.00\%$ CPU overhead. Path/stroke/radius tracks remain strictly within the `8ms` `frame_build_nanos` budget.
   - **Shared Math Location:** Pure easing and spring math lives in `designos-doc`, completely free of `egui` or GUI dependencies, shared across desktop, headless MCP, and export CLI.

---

## 1. What could NOT be verified (read first)

1. **WASM-internal Lottie AST optimization passes:** The exact node-reduction heuristics in Figma's C++ WebAssembly for combining multiple bezier tracks into single cubic splines could not be disassembled due to symbol stripping (`wasm-strings.txt`). Studio uses canonical Bodymovin cubic bezier spline output.
2. **Live GPU Hardware Video Encoding on Linux:** In Linux CI environments without NVENC or VA-API, video export must fall back to software VP8/VP9 or openh264. Verification was confirmed on macOS (VideoToolbox) and Windows (MediaFoundation).
3. **Figma Slides Video Duration Clamping:** Exact handling when an embedded video layer's `playbackDurationUs` exceeds the containing slide's timeline duration is inferred to truncate at the slide boundary.

---

## 2. Slice Order & Roadmap

The implementation is broken down into 4 serialized slices to maintain clean compiler stability and avoid merge hazards in hot files:

```
┌────────────────────────────────────────────────────────────────────────────────┐
│  Slice 1: P1 Export Bounds, Constraints & Lossless WebP (#528, #457)          │
│  - render_bounds vs absolute_bounding_box                                      │
│  - useAbsoluteBounds = 9, contentsOnly = 7, clipsContent containment           │
│  - Lossless WebP encoder integration                                           │
└──────────────────────────────────────┬─────────────────────────────────────────┘
                                       │
                                       ▼
┌────────────────────────────────────────────────────────────────────────────────┐
│  Slice 2: P2 Easing Standardization & Analytical Spring ODEs (#459)           │
│  - Replace Penner curves with Figma CSS beziers & back curves                  │
│  - Analytical spring ODE solver (underdamped, overdamped, critically damped)   │
│  - Logarithmic epsilon=0.001 settling duration                                 │
│  - Pure Rust module in designos-doc, zero egui dependency                     │
└──────────────────────────────────────┬─────────────────────────────────────────┘
                                       │
                                       ▼
┌────────────────────────────────────────────────────────────────────────────────┐
│  Slice 3: 18-Field ExportSettings Round-Trip & Kiwi Mapping (#213)            │
│  - ImageType (PNG, JPG, SVG, PDF, MP4, GIF, WEBM, LOTTIE, DOTLOTTIE)          │
│  - ExportColorProfile, ExportConstraint, SVG options                           │
│  - Kiwi schema decoding and encoding round-trip                                │
└──────────────────────────────────────┬─────────────────────────────────────────┘
                                       │
                                       ▼
┌────────────────────────────────────────────────────────────────────────────────┐
│  Slice 4: M1-M3 Motion Engine, Timeline UI, Lottie & Video Export             │
│  - Kiwi Keyframe & Timeline nodes (56, 57, 58) + StyleType::ANIMATION (7)     │
│  - Pure evaluator (doc, t_us) -> NodeOverrides (no rev bump!)                 │
│  - 6 primitives + ~60 composite presets + TextElementDelayBehaviour (610)     │
│  - Lottie / dotLottie exporter + 13 lint checks                                │
│  - Animated SVG, CSS, React motion codegen                                     │
│  - VideoToolbox / MediaFoundation MP4 & WebM export pipeline                  │
└────────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Hot-File Queue & Edit Serialization

To respect the single-writer principle on hot files, agents working on Epic E6 must serialize changes to shared files:

| File Path | Owning Slice | Lock Reason | Concurrent Edits Allowed? |
|---|---|---|---|
| `crates/designos-doc/src/prototype.rs` | Slice 2 | Easing enum & Spring math | **NO** — Complete Slice 2 first |
| `crates/designos-doc/src/settings.rs` | Slice 3 | `ExportSettings` & `ExportPreset` | **NO** — Complete Slice 3 first |
| `crates/designos-doc/src/node.rs` | Slice 4 | Keyframe & Timeline fields (505..610) | **NO** — Serialize with Slice 4 |
| `crates/designos-fig/src/kiwi.rs` | Slice 3, Slice 4 | Kiwi binary definitions | **NO** — Update once in Slice 3, append in Slice 4 |
| `crates/designos-fig/src/fig.rs` | Slice 3, Slice 4 | .fig deserializer | **NO** — Staged by slice |
| `crates/designos-fig/src/fig_export.rs` | Slice 3, Slice 4 | .fig serializer | **NO** — Staged by slice |
| `crates/designos-export/src/bounds.rs` | Slice 1 | Reach expansion formulas | Safe after Slice 1 |
| `crates/designos-export/src/raster.rs` | Slice 1, Slice 4 | Rasterizer & Pixmap allocation | Shared across slices |
| `crates/designos-export/src/svg.rs` | Slice 1, Slice 4 | SVG export & animation tags | Safe after Slice 1 |

---

## 4. Model-Tier Routing & Agent Delegation Contract

In strict compliance with the **Antigravity Sub-Agent Cost-Optimization Protocol (`GEMINI.md`)**:

- **Gemini 4 Carbon (`coordinator` / `architect`):**
  - High-level architecture, specification authoring, cross-epic review, and final gate verification.
  - NEVER performs bulk coding, test runs, or mechanical updates.
- **Gemini 4 Argon (`Model: "pro"`):**
  - Complex implementation tasks:
    - Pure analytical spring ODE solver and numerical stability tests.
    - Pure motion evaluator `(doc, t_us) -> NodeOverrides` and GPU uniform mapping.
    - Bodymovin/dotLottie AST serialization and Lottie lint engine.
    - VideoToolbox C/Metal pixel buffer pipeline.
- **Gemini 3.8 Flash (`Model: "flash"`):**
  - Machine-checkable, spec-bound tasks:
    - Kiwi struct/enum tag additions in `crates/designos-fig/src/kiwi.rs`.
    - Bounds expansion unit tests and SVG option flag plumbing.
    - 60 composite preset JSON table mappings.
    - Cargo test gate execution (`cargo test -p designos-export`, `cargo test -p designos-doc`).
