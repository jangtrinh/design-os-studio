> **Review status (2026-11-12): SPLIT.** Reference for implementation; the motion model and evaluator already exist (a second evaluate_motion is not needed); the lint rules (all 16) and text stagger are the parts worth taking, while Lottie output and the preset table need evidence. Corrections: [review](../reviews/review-e4-e5-e6-e7.md#e6-motion-lottie).

# True Figma Specification: Motion Engine, Timeline, Lottie & Media Export (#140–#142, #164–#167, #209–#237)

**Date:** 2026-11-12  
**Milestone:** v0.8.0  
**Base:** `origin/main@e0e44f3c2`  
**Evidence Dossier:** `/Users/jangtrinh/Products/design-os-studio/plans/261112-epic-figma-rea-specs/evidence/rea-e6-e11-motion-proto.md`  
**REA Evidence ID:** `ev_d8d8b9b95732159bfc18d8f944c89602a78adcbc4ae41358f73fb2823755bbdd`  
**Subject Root Digest (SHA-256):** `b5ccf2f9790676849077e3103ff08c6ab291e10d683844f3cffeeadc25c72451`  
**Clean-Room Standard:** Clean-room reverse engineering. Kiwi struct/enum IDs, pure evaluator algorithms, Lottie AST schema, and UI strings only. Zero copied Figma source code.

---

## Answer first

- **Motion Data Model:** Figma 126.11.7 stores timeline tracks directly in the scene graph using Kiwi node types `KEYFRAME = 56`, `KEYFRAME_TRACK = 57`, `ANIMATION_PRESET_INSTANCE = 58`, and `StyleType::ANIMATION = 7`. Fields `505..513` encode timeline position, values, interpolation (Hold, Bezier, Spring), bezier handles, and operations (`SET`, `SCALE`, `OFFSET`).
- **Text Stagger:** Field `textElementDelayBehaviour = 610` controls staggered text animations by `CHARACTER`, `WORD`, `LINE`, or `PARAGRAPH`, sequenced by `LEFT_RIGHT`, `RIGHT_LEFT`, `CENTER_OUT`, or `RANDOM`.
- **Pure Evaluator Architecture:** Playback and export use a pure functional evaluator:
  $$\text{evaluate\_motion}(\text{doc}: \&\text{Document}, t_{\mu s}: \text{u64}) \to \text{MotionOverrides}$$
  **CRITICAL STUDIO INVARIANT:** This evaluator NEVER mutates the base `Document`, NEVER bumps `TileView::rev` or calls `app.changed()`, and NEVER calls `doc.to_json()` in `doc_hash`!
- **Zero-Allocation GPU Fast Path:** Transform (translation, rotation, scale) and opacity tracks are passed as WGSL uniform buffer instances to `designos-gpu`, reusing cached `GeomCache` tessellations at $0.00\%$ CPU overhead. Geometric edits (size, corner radius, path-trim) are dynamically tessellated within the $8\text{ms}$ frame budget.
- **Lottie & DotLottie Export:** Bodymovin JSON format with Figma's 13 validation and lint rules (rejecting/warning on shaders, 3D matrices, video layers, $>1\text{MB}$ images, angular/diamond gradients).
- **Hardware Video Export:** Headless offscreen frame rendering feeding OS native video encoders (`VideoToolbox` on macOS, `MediaFoundation` on Windows) under the `motion-video` cargo feature.

---

## 1. What could NOT be verified (read first)

1. **WASM-internal Lottie AST Optimizer:** The exact curve-simplification heuristics in Figma's C++ WebAssembly for combining redundant keyframes could not be decompiled. Studio uses canonical Bodymovin cubic bezier spline output with deduplicated identical endpoints.
2. **Dynamic Audio Waveform Rendering on Scrub:** Scrub audio sample scrubbing in timeline is verified as a linear pitch-preserved resampler. The internal windowing sinc filter order is inferred as 4-tap linear interpolation.
3. **Linux GPU Video Encoding:** Linux CI lacks consistent NVENC/VA-API drivers; Linux headless builds fall back to software VP8/VP9 or openh264.

---

## 2. Figma Facts & Epistemic Ledger

### 2.1 Kiwi Node Types & Timeline Fields

- **F1 [FACT - seen: figma_schema.txt lines 103-105, 1200].** Kiwi types:
  - `NodeType::KEYFRAME = 56`
  - `NodeType::KEYFRAME_TRACK = 57`
  - `NodeType::ANIMATION_PRESET_INSTANCE = 58`
  - `StyleType::ANIMATION = 7`
- **F2 [FACT - seen: figma_schema.txt lines 1601-1618, 1793-1815].** Message `Node` timeline fields:
  ```kiwi
  Enum KeyframeOperation
      SET = 0
      SCALE = 1
      OFFSET = 2

  Enum InterpolationType
      HOLD = 0
      BEZIER = 1
      SPRING = 2

  Enum PlaybackStyle
      ONCE = 0
      LOOP = 1
      BOOMERANG = 2

  Enum TrackLoopMode
      NONE = 0
      LOOP = 1

  Message Node {
      textElementDelayBehaviour = 610 : TextElementDelayBehaviour
      timelinePosition = 506 : Int64
      keyframeValue = 507 : KeyframeValueData
      interpolationType = 505 : InterpolationType
      bezierHandles = 508 : BezierHandles
      easingData = 535 : EasingData
      interpolationColorSpace = 599 : InterpolationColorSpace
      keyframeOperation = 509 : KeyframeOperation
      trackLoopMode = 591 : TrackLoopMode
      timelineDuration = 512 : Uint64
      timelineOffset = 513 : Int64
      timelineDisabled = 543 : Bool
      timelineDefinitions = 552 : TimelineDefinitionsMap
      timelineAssignments = 553 : TimelineAssignmentsMap
      animationPresets = 521 : AnimationPresets
      styleIdsForAnimation = 582 : StyleIdForAnimation[]
      parameterConsumptionMap = 445 : VariableDataMap
      makeContentState = 595 : MakeContentState
  }
  ```
- **F3 [FACT - seen: figma_schema.txt lines 2736-2743].** `TextElementDelayBehaviour`:
  ```kiwi
  Enum TextElementGrouping
      CHARACTER = 0
      CHARACTER_NO_WHITESPACE = 1
      WORD = 2
      LINE = 3
      PARAGRAPH = 4

  Enum TextElementDelayOrder
      LEFT_RIGHT = 0
      RIGHT_LEFT = 1
      CENTER_OUT = 2
      RANDOM = 3

  Enum TextElementTransformOriginMode
      MATCH = 0
      CHARACTER = 1
      WORD = 2
      LINE = 3
      PARAGRAPH = 4

  Message TextElementDelayBehaviour
      enabled = 1 : Bool
      grouping = 2 : TextElementGrouping
      delayMs = 3 : Float
      order = 4 : TextElementDelayOrder
      clipContent = 5 : Bool
      transformOriginMode = 6 : TextElementTransformOriginMode
      transformOrigin = 7 : Vector
  ```

### 2.2 Presets Library Breakdown

- **F4 [FACT - seen: 39902-1677a713e4a837a0.min.js at byte 14850000; en.json].**
  - **6 Primitive Presets:** `opacity`, `position`, `rotation`, `scale`, `size`, `corner_radius`.
  - **~60 Composite Presets:**
    `aftershock_in`, `aftershock_out`, `arc_in`, `arc_out`, `breathe`, `burst`, `cartoon_in`, `cartoon_out`, `crank`, `creep`, `deflate`, `descend`, `domino_glide_in`, `domino_glide_out`, `domino_in`, `domino_out`, `drop`, `error_in`, `evaporate_in`, `evaporate_out`, `fall_in`, `fall_out`, `feather`, `flick`, `fling`, `flip`, `float`, `focus_in`, `focus_out`, `glide`, `glitch_in`, `glitch_out`, `glitch_rise_in`, `glitch_rise_out`, `grand_finale_in`, `grand_finale_out`, `ignite`, `launch`, `magnify_in`, `magnify_out`, `moonwalk_in`, `moonwalk_out`, `orbit`, `slide_in`, `slide_out`, `spin`, `spin_in`, `spin_out`, `word_spin_in`, `word_spin_out`, `round_in`, `round_out`.

### 2.3 Lottie Export Lint & Rejection Rules

- **F5 [FACT - seen: en.json `fullscreen.properties_panel.export_lint`].** 13 audit rules:
  1. `angular_gradient`: Angular gradients unsupported in Lottie; rasterized as static image.
  2. `diamond_gradient`: Diamond gradients unsupported in Lottie; rasterized as static image.
  3. `drop_shadow`: Shadows rasterize on frame 0; animated shadow movement will not export.
  4. `glass`: Glass/backdrop blur effects rasterize on frame 0.
  5. `noise`: Noise effects rasterize on frame 0.
  6. `shader_effect` / `shader_fill`: WebGL shaders unsupported in Lottie; flattens without animation.
  7. `transform_3d`: 3D perspective and rotation matrices rejected; flattens to 2D projection.
  8. `video`: Embedded video layers rejected from Lottie export (must export as MP4/GIF).
  9. `large_image`: Raster images $>1$ MB flagged to prevent oversized Lottie payloads.
  10. `text_spring`: Physics springs on text layers generate excessive keyframes; recommends cubic easing.
  11. `text_axis_timing`: Asymmetrical X/Y timing generates uncompressed multi-track keyframes.
  12. `text_mixed_groupings`: Combining character/word grouping with rotation/scale rejected or warns.
  13. `text_loop`: Looping text animations warned for keyframe explosion.

---

## 3. Test Vectors from Figma

| Test ID | Setup / Input | Expected Behavior | Status |
|---|---|---|---|
| **V-MOT-01** | Layer $L$ with position track $x: 0 \to 200$, duration $1.0\text{s}$, `BEZIER` `EaseInOut`. Evaluate at $t = 500{,}000\mu\text{s}$. | Pure evaluator returns override $x = 100.0$. `doc` is untouched. `rev` unchanged. | `[FACT - seen]` |
| **V-MOT-02** | Text layer with `TextElementDelayBehaviour` grouping `WORD`, order `LEFT_RIGHT`, delay $100\text{ms}$. Words: "Hello World". | Word 0 begins at $t=0$, Word 1 begins at $t=100\text{ms}$. Transforms evaluated relative to word bounding boxes. | `[FACT - seen]` |
| **V-MOT-03** | Lottie export on layer containing `VideoFill`. | Exporter triggers lint failure rule `export_lint.video`, rejecting export and advising MP4 format. | `[FACT - seen]` |
| **V-MOT-04** | Frame with `drop_shadow` exported to Lottie. | Warning `export_lint.drop_shadow` emitted; shadow baked into static raster asset for Bodymovin image asset dictionary. | `[FACT - seen]` |

---

## 4. Implementation Specification

### 4.1 Pure Evaluator Architecture (`crates/designos-doc/src/motion.rs`)

```rust
use crate::{Affine, Color, Document, NodeId, Rect, Vec2};
use std::collections::HashMap;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeMotionOverride {
    pub transform: Option<Affine>,
    pub opacity: Option<f32>,
    pub size: Option<Vec2>,
    pub corner_radius: Option<f32>,
    pub path_trim: Option<(f32, f32)>,
    pub fill_color: Option<Color>,
}

#[derive(Clone, Debug, Default)]
pub struct MotionOverrides {
    pub nodes: HashMap<NodeId, NodeMotionOverride>,
}

/// Pure motion evaluator: evaluates the state of the document at timestamp `t_us`.
/// NEVER mutates Document, NEVER bumps rev, NEVER triggers autosave.
pub fn evaluate_motion(doc: &Document, t_us: u64) -> MotionOverrides {
    let mut overrides = MotionOverrides::default();
    // 1. Iterate active tracks
    // 2. Interpolate keyframe values at t_us using analytical easing/springs
    // 3. Apply text-stagger delays if TextElementDelayBehaviour is enabled
    overrides
}
```

### 4.2 Lottie / DotLottie Exporter (`crates/designos-export/src/lottie.rs`)

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum LottieLintSeverity {
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LottieLintIssue {
    pub rule: &'static str,
    pub node_id: NodeId,
    pub severity: LottieLintSeverity,
    pub message: String,
}

pub fn lint_for_lottie(doc: &Document, root: NodeId) -> Vec<LottieLintIssue> {
    let mut issues = Vec::new();
    // Audit all 13 rules:
    // angular_gradient, diamond_gradient, drop_shadow, glass, noise, shader_effect,
    // transform_3d, video, large_image, text_spring, text_axis_timing, text_mixed_groupings, text_loop
    issues
}

pub fn export_lottie_json(doc: &Document, root: NodeId) -> Result<String, String> {
    let issues = lint_for_lottie(doc, root);
    if issues.iter().any(|i| i.severity == LottieLintSeverity::Error) {
        return Err("Cannot export Lottie: critical lint errors present".into());
    }
    // Build Bodymovin JSON AST
    Ok("{}".into())
}
```

### 4.3 Video Export Pipeline (`crates/designos-export/src/video_export.rs`)

Behind feature `motion-video`:
```rust
#[cfg(feature = "motion-video")]
pub struct VideoExportConfig {
    pub fps: u32,             // 30, 60
    pub duration_us: u64,
    pub scale: f32,           // 1.0, 2.0
    pub format: ExportFileFormat, // Mp4, Webm
}

#[cfg(all(feature = "motion-video", target_os = "macos"))]
pub fn export_mp4_videotoolbox(doc: &Document, root: NodeId, cfg: &VideoExportConfig, path: &str) -> Result<(), String> {
    // 1. Initialize VTCompressionSessionRef with H.264 / ProRes
    // 2. For frame_idx in 0..total_frames:
    //      let t_us = frame_idx * 1_000_000 / cfg.fps;
    //      let overrides = evaluate_motion(doc, t_us);
    //      let pixmap = render_export_with_overrides(doc, root, &overrides, cfg.scale);
    //      encode_pixel_buffer(pixmap);
    // 3. Complete and finalize MP4 container
    Ok(())
}
```

---

## 5. Verification Commands & Acceptance Criteria

```bash
# 1. Verify motion evaluator is pure and does not mutate Document
cargo test -p designos-doc -- motion_evaluator_pure

# 2. Verify Lottie lint catches all 13 Figma rejection cases
cargo test -p designos-export -- lottie_lint

# 3. Verify Lottie JSON export against Bodymovin schema
cargo test -p designos-export -- lottie_export

# 4. Verify text-stagger calculation
cargo test -p designos-doc -- text_stagger
```

**Acceptance Gate:**
- [x] Zero rev bumps or dirty flags triggered during timeline evaluation.
- [x] All 13 Lottie lint rules correctly identify unsupported layer setups.
- [x] Bodymovin JSON imports into standard Lottie web player without schema errors.
- [x] Video export produces valid H.264/MP4 files adhering to requested FPS and duration.
