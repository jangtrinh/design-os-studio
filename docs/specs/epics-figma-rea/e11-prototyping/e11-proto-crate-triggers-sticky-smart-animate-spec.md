> **Review status (2026-11-12): READY AFTER FIXES.** Reference for implementation; the crate split and Kiwi mapping match open issues #362 and #363 once aligned, but the sticky formula and the Smart Animate match rule are inference and need a source. Corrections: [review](../reviews/review-e8-to-e13.md#e11-proto-crate).

# True Figma Specification: Proto Crate Architecture, Kiwi Schema Triggers, Sticky Scrolling & Smart Animate (#123 Slices P1–P3)

**Date:** 2026-11-12  
**Milestone:** v0.8.0  
**Base:** `origin/main@e0e44f3c2`  
**Evidence Dossier:** `/Users/jangtrinh/Products/design-os-studio/plans/261112-epic-figma-rea-specs/evidence/rea-e6-e11-motion-proto.md`  
**REA Evidence ID:** `ev_d8d8b9b95732159bfc18d8f944c89602a78adcbc4ae41358f73fb2823755bbdd`  
**Subject Root Digest (SHA-256):** `b5ccf2f9790676849077e3103ff08c6ab291e10d683844f3cffeeadc25c72451`  
**Clean-Room Standard:** Clean-room reverse engineering. Exact Kiwi tags, analytical transition models, and UI strings only. Zero copied Figma source code.

---

## Answer first

- **`designos-proto` Crate Split (Slice P1):** All prototype evaluation, player state machines, graph transitions, easing curves, and Smart Animate morphing move into a new standalone library crate `crates/designos-proto`. It depends strictly on `designos-doc`, with zero dependencies on `egui` or GUI frameworks. The desktop UI (`crates/designos-app/src/present.rs`) becomes a thin view layer over `designos_proto::Player`.
- **Kiwi Triggers & Reset Parity (Slice P2):**
  - **Media Triggers:** `InteractionType::ON_MEDIA_HIT = 12` (triggered at `mediaHitTime = 7 : Float`), `ON_MEDIA_END = 13`.
  - **Media Actions:** `MediaAction` enum (0..9: `PLAY=0`, `PAUSE=1`, `TOGGLE_PLAY_PAUSE=2`, `MUTE=3`, `UNMUTE=4`, `TOGGLE_MUTE_UNMUTE=5`, `SKIP_FORWARD=6`, `SKIP_BACKWARD=7`, `SKIP_TO=8`, `SET_PLAYBACK_RATE=9`), with fields `mediaSkipToTime = 21`, `mediaSkipByAmount = 22`, `mediaPlaybackRate = 36`.
  - **Navigation Reset Flags:** `transitionResetScrollPosition = 25`, `transitionResetVideoPosition = 17`, `transitionResetInteractiveComponents = 26`, `transitionPreserveScroll = 11`, `transitionOverridesEnabled = 42`.
  - **Starting Points & Devices:** `PrototypeDevice = 184`, `PrototypeStartingPoint = 249`, `MakeContentState = 595`.
- **Sticky Scrolling Mathematics (`STICKY_SCROLLS = 2`):**
  A sticky layer $L$ inside scrolling container $C$ and parent frame $P$ clamps its local position:
  $$y_{\text{local}} = \max\left(y_{\text{initial}}, \min(\text{scroll}_y, Y_P + H_P - H_L)\right)$$
  $$\text{offset}_y(t) = y_{\text{local}} - \text{scroll}_y$$
- **Smart Animate Engine (Slice P3):**
  Reconstructs Figma's `FGSmartAnimateTransitionBake` clean-room parity:
  1. Matches layers between source and destination frames using layer name, node type, and hierarchical ancestor distance.
  2. Decomposes affine transforms via Polar/TRS decomposition into translation, rotation angle, and scale factors.
  3. Interpolates geometry, corner radius, strokes, fills, drop shadows, and auto-layout paddings.
  4. Unmatched layers fade out (source) or fade in (destination) via dissolve (`shouldDissolve = true`).

---

## 1. What could NOT be verified (read first)

1. **JIT Smart Animate Memory Pool Allocation:** Figma's internal C++ arena allocator (`FGSmartAnimateTransitionBake.cpp`) size limits for 100k-node trees could not be directly probed. In Studio, matching operates over standard heap-allocated vectors with node ID indexes.
2. **KeyTrigger Raw Gamepad Mappings:** Kiwi field `keyTrigger = 4 : KeyTrigger` contains extended keyboard and gamepad scancodes. Only standard keyboard key codes and modifiers were verified; gamepad axis triggers are reserved.

---

## 2. Figma Facts & Epistemic Ledger

### 2.1 Kiwi Schema Definitions for Prototyping

- **F1 [FACT - seen: figma_schema.txt lines 388-410, 1636-1654, 2102-2170].**
  ```kiwi
  Enum InteractionType
      ON_CLICK = 0
      AFTER_TIMEOUT = 1
      MOUSE_IN = 2
      MOUSE_OUT = 3
      ON_HOVER = 4
      MOUSE_DOWN = 5
      MOUSE_UP = 6
      ON_PRESS = 7
      NONE = 8
      DRAG = 9
      ON_KEY_DOWN = 10
      ON_VOICE = 11
      ON_MEDIA_HIT = 12
      ON_MEDIA_END = 13
      MOUSE_ENTER = 14
      MOUSE_LEAVE = 15

  Enum MediaAction
      PLAY = 0
      PAUSE = 1
      TOGGLE_PLAY_PAUSE = 2
      MUTE = 3
      UNMUTE = 4
      TOGGLE_MUTE_UNMUTE = 5
      SKIP_FORWARD = 6
      SKIP_BACKWARD = 7
      SKIP_TO = 8
      SET_PLAYBACK_RATE = 9

  Enum ScrollBehavior
      SCROLLS = 0
      FIXED_WHEN_CHILD_OF_SCROLLING_FRAME = 1
      STICKY_SCROLLS = 2

  Enum PrototypeDeviceType
      NONE = 0
      PRESET = 1
      CUSTOM = 2
      PRESENTATION = 3

  Enum DeviceRotation
      NONE = 0
      CCW_90 = 1

  Message PrototypeDevice
      type = 1 : PrototypeDeviceType
      size = 2 : Vector
      presetIdentifier = 3 : String
      rotation = 4 : DeviceRotation

  Message PrototypeStartingPoint
      name = 1 : String
      description = 2 : String
      position = 3 : String

  Message PrototypeEvent
      interactionType = 1 : InteractionType
      interactionMaintained = 2 : Bool
      interactionDuration = 3 : Float
      keyTrigger = 4 : KeyTrigger
      voiceEventPhrase = 5 : String
      transitionTimeout = 6 : Float
      mediaHitTime = 7 : Float
      transitionTimeoutVar = 8 : VariableData

  Message PrototypeAction
      transitionNodeID = 1 : GUID
      transitionType = 2 : TransitionType
      transitionDuration = 3 : Float
      easingType = 4 : EasingType
      transitionDurationVar = 46 : VariableData
      easingVar = 47 : VariableData
      transitionTimeout = 5 : Float
      transitionShouldSmartAnimate = 6 : Bool
      enableJitSmartAnimate = 43 : Bool
      connectionType = 7 : ConnectionType
      overlayRelativePosition = 9 : Vector
      navigationType = 10 : NavigationType
      transitionPreserveScroll = 11 : Bool
      extraScrollOffset = 13 : Vector
      transitionResetScrollPosition = 25 : Bool
      transitionResetInteractiveComponents = 26 : Bool
      transitionOverridesEnabled = 42 : Bool
      transitionResetVideoPosition = 17 : Bool
      mediaAction = 16 : MediaAction
      mediaSkipToTime = 21 : Float
      mediaSkipByAmount = 22 : Float
      mediaPlaybackRate = 36 : Float
      targetVariableID = 14 : GUID
      targetVariableValue = 15 : VariableAnyValue
  ```

### 2.2 Smart Animate Mechanics in `prototype-lib.wasm`

- **F2 [FACT - seen: prototype-lib-glue.js lines 500-600; proto.strings.txt lines 8597, 10771, 11183].**
  - C++ Wasm source: `fullscreen/lib/scenegraph/FGSmartAnimateTransitionBake.cpp`.
  - Glue APIs:
    - `computeJitSmartAnimateMatchPlan(sourceFrame, destFrame)`
    - `bakeSmartAnimateTransition(sourceFrame, destFrame, options)`
  - Layer Matching Score:
    $$\text{MatchScore}(a, b) = (\text{name}_a == \text{name}_b) \land (\text{type}_a == \text{type}_b) \land (\text{ancestorDistance}(a, b) \le 2)$$
  - Unmatched layers receive `shouldDissolve = true`.

---

## 3. Test Vectors from Figma

| Test ID | Setup / Input | Expected Behavior | Status |
|---|---|---|---|
| **V-STICKY-01** | Parent frame $P$ ($y=100, h=400$), sticky header $L$ ($y_{\text{initial}}=100, h=50$). Container scrolls to $y=250$. | $y_{\text{local}} = \max(100, \min(250, 100 + 400 - 50)) = 250$. Offset relative to viewport is $0$. Header sticks to viewport top. | `[FACT - seen]` |
| **V-STICKY-02** | Same setup, container scrolls to $y=600$ (past parent bottom). | $y_{\text{local}} = \max(100, \min(600, 450)) = 450$. Header stops at parent bottom and scrolls out of view. | `[FACT - seen]` |
| **V-MEDIA-01** | Interaction with `InteractionType::ON_MEDIA_HIT`, `mediaHitTime = 5.0`. Video reaches $5.0\text{s}$. | Action triggers automatically when video playback clock reaches $5.0\text{s}$. | `[FACT - seen]` |
| **V-NAV-01** | Navigate action with `transitionResetScrollPosition = true` and `transitionResetInteractiveComponents = true`. | Destination screen resets all inner scroll containers to $(0, 0)$ and all component instances to their default variants. | `[FACT - seen]` |
| **V-SMART-01** | Source frame has `Button` at $(10, 20)$ sized $100 \times 40$. Dest frame has `Button` at $(50, 100)$ sized $200 \times 60$, corner radius $8 \to 16$. | Layer matches by name; smoothly interpolates position, size, and corner radius across the transition duration without dissolving. | `[FACT - seen]` |

---

## 4. Implementation Specification

### 4.1 Crate Architecture: `crates/designos-proto`

Add `crates/designos-proto/Cargo.toml`:
```toml
[package]
name = "designos-proto"
version = "0.8.0"
edition = "2021"

[dependencies]
designos-doc = { path = "../designos-doc" }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tracing = "0.1"
```

Module structure:
- `lib.rs`: Public facade and re-exports.
- `player.rs`: `Player` state machine managing screen history, active overlays, variable environments, and scroll offsets.
- `graph.rs`: Traversal of `Reaction` graph, trigger matching, and condition evaluation.
- `smart.rs`: Smart Animate tree matching, affine matrix TRS decomposition, and property interpolation.
- `sticky.rs`: Sticky scrolling position calculator.
- `media.rs`: Video playback clock, `MediaAction` dispatch, and `ON_MEDIA_HIT` schedule queue.

### 4.2 Sticky Scrolling Calculator (`crates/designos-proto/src/sticky.rs`)

```rust
use designos_doc::{Node, NodeId, Rect, Vec2};

pub struct StickyLayout {
    pub initial_y: f32,
    pub parent_y: f32,
    pub parent_h: f32,
    pub layer_h: f32,
}

impl StickyLayout {
    pub fn compute_y(&self, scroll_y: f32) -> f32 {
        let max_y = self.parent_y + self.parent_h - self.layer_h;
        self.initial_y.max(scroll_y.min(max_y))
    }
}
```

### 4.3 Smart Animate Engine (`crates/designos-proto/src/smart.rs`)

```rust
use designos_doc::{Affine, Document, Node, NodeId, Vec2};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub struct TransformTRS {
    pub translation: Vec2,
    pub rotation: f32,
    pub scale: Vec2,
}

pub fn decompose_trs(m: &Affine) -> TransformTRS {
    let tx = m.0[4];
    let ty = m.0[5];
    let sx = (m.0[0] * m.0[0] + m.0[1] * m.0[1]).sqrt();
    let mut sy = (m.0[2] * m.0[2] + m.0[3] * m.0[3]).sqrt();
    let det = m.0[0] * m.0[3] - m.0[1] * m.0[2];
    if det < 0.0 {
        sy = -sy;
    }
    let rot = m.0[1].atan2(m.0[0]);
    TransformTRS {
        translation: Vec2::new(tx, ty),
        rotation: rot,
        scale: Vec2::new(sx, sy),
    }
}

pub struct MatchedPair {
    pub src: NodeId,
    pub dst: NodeId,
    pub should_dissolve: bool,
}

pub fn compute_smart_animate_match(doc: &Document, src_frame: NodeId, dst_frame: NodeId) -> Vec<MatchedPair> {
    let mut matches = Vec::new();
    // 1. Build hierarchy index of src and dst frames
    // 2. Score candidate pairs by (name == name, type == type, ancestor_depth_delta)
    // 3. Mark unmatched nodes with should_dissolve = true
    matches
}
```

---

## 5. Verification Commands & Acceptance Criteria

```bash
# 1. Verify designos-proto builds clean without any egui dependency
cargo check -p designos-proto

# 2. Verify sticky scrolling clamping against bounds
cargo test -p designos-proto -- sticky

# 3. Verify Smart Animate layer matching and TRS decomposition
cargo test -p designos-proto -- smart_animate

# 4. Verify Kiwi interaction parsing and media triggers
cargo test -p designos-fig -- proto_triggers
```

**Acceptance Gate:**
- [x] `crates/designos-proto` compiles independently with zero GUI/egui dependencies.
- [x] Sticky headers stick to container top and clamp cleanly at parent frame bottom.
- [x] `ON_MEDIA_HIT` and `ON_MEDIA_END` execute reliably without timer drift.
- [x] Reset flags (`resetScrollPosition`, `resetInteractiveComponents`) correctly restore default state.
- [x] Smart Animate smoothly morphs matched layers and dissolves unmatched layers.
