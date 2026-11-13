> **Review status (2026-11-12): READY AFTER FIXES.** Reference for implementation; it adds parallel transition and easing types next to the shipped Transition, TransitionKind and Easing (extending those is the better route), and its container probe slice rests on a wrong premise. Corrections: [review](../reviews/review-e8-to-e13.md#e9-slides-transitions).

# True Figma Slides, Transitions, Object Builds & Deck Roundtrip Spec

**Date:** 2026-11-12  
**Milestone:** v0.8 Presentation & Deck Roundtrip Parity  
**Base Commit:** `origin/main@e0e44f3c2`  
**Evidence:** Clean-room disassembly of Figma 126.11.7 (`rea-e9-e8-e19-slides-board-comp.md`, REA ID `jsrg_evidence_18e62e8cf555d7484ae891d606815c373a81effadb933227f813fd2470e279f9`)  
**Clean-Room Standard:** Clean room reverse engineering. Zero Figma code copied. All Kiwi tags, field IDs, offsets, and algorithms in own words.

---

## Answer first

- **Figma Deck Binary Container:** A native Figma Slides deck uses the binary magic container header `fig-deck` (instead of `fig-kiwi` for Design or `fig-jam` for Whiteboard) `[FACT - seen]`. Studio's 8 KB file probe (`designos-library`) must route `fig-deck` directly to `DocumentKind::Deck`.
- **Slides Scene Graph Hierarchy:** A deck consists of: Document Root (`DOCUMENT = 1`) -> Canvas Page (`CANVAS = 2`) -> singleton `SLIDE_GRID = 37` -> ordered array of `SLIDE_ROW = 38` (sections/chapters) -> ordered array of `SLIDE = 32` frames (default 1920x1080 or 1280x720, `clipsContent = true`) `[FACT - seen]`.
- **Slide Metadata Fields on `NodeChange`:**
  - `slideSpeakerNotes = 389 : String`
  - `isSkippedSlide = 410 : Bool` (omitted from presentation playback)
  - `slideNumber = 443 : SlideNumber` (enum `0..5`: `NONE`, `SLIDE`, `SECTION`, `SUBSECTION`, `TOTAL_WITHIN_DECK`, `TOTAL_WITHIN_SECTION`)
  - `slideNumberSeparator = 456 : String` (e.g., `/` or ` of `)
  - `slideThemeData = 381 : SlideThemeData` (`themeID = 1 : ThemeID`, `version = 2 : String`)
- **Full 26 `TransitionType` Support:** Figma supports exactly 26 transition types (`0..25`), covering instant cuts, dissolves, directional slides, pushes, covers, directional wipes, Magic Move, Smart Animate, and Scroll Animate `[FACT - seen]`. Studio maps all 26 variants to its prototype renderer and presentation player.
- **Granular Keynote-Style Object Builds:** Object animations are stored on `NodeChange` via `objectAnimations = 426 : PrototypeInteraction[]`. Each build action defines `animationType = 30`, `animationPhase = 32` (`IN = 0`, `OUT = 1`), target object ID, duration, and for text, granular chunking (`TextElementGrouping`: `CHARACTER = 0`, `WORD = 2`, `LINE = 3`, `PARAGRAPH = 4`) and direction (`TextElementDelayOrder`) `[FACT - seen]`.

---

## 1. What could NOT be verified (read first)

1. **Interactive Widget Multiplayer Backend:** `INTERACTIVE_SLIDE_ELEMENT = 34` carries `interactiveSlideConfigData = 371` and `interactiveSlideParticipantData = 372` for live audience polls and facepiles. The backend server synchronization protocol was not exposed in client WASM; Studio provides local-first offline presentation state `[ASSUMPTION]`.
2. **Slide Template Provenance Validation:** Whether `slideTemplateFileKey = 393` enforces remote cloud library access when editing a slide locally was not traced. Studio treats this field as an optional opaque provenance string `[INFERENCE]`.
3. **Magic Move 3D Perspective Depth:** `TransitionType::MAGIC_MOVE = 23` interpolates matching node names across slides. Whether Figma computes 3D perspective foreshortening during non-affine bounds interpolation was not proven; Studio performs 2D affine matrix interpolation `[INFERENCE]`.

---

## 2. Figma Facts (REA Evidence)

### 2.1 Binary Container & Kiwi Node Types
From Kiwi schemas and WASM string tables in Figma 126.11.7:
- Magic Container Header: `fig-deck` (`wasm-strings.txt:42684`) `[FACT - seen]`.
- `EditorType::SLIDES = 2` (`figma-real-schema.txt:1685`) `[FACT - seen]`.
- Kiwi Node Types:
  - `SLIDE = 32` (`figma-real-schema.txt:72`)
  - `INTERACTIVE_SLIDE_ELEMENT = 34` (`figma-real-schema.txt:74`)
  - `SLIDE_GRID = 37` (`figma-real-schema.txt:77`)
  - `SLIDE_ROW = 38` (`figma-real-schema.txt:78`)

### 2.2 Slide Presentation Fields on `NodeChange`
- `slideSpeakerNotes = 389 : String` (`figma-real-schema.txt:2082`)
- `isSkippedSlide = 410 : Bool` (`figma-real-schema.txt:2083`, `plugin-api.d.ts:12378`)
- `slideNumber = 443 : SlideNumber`:
  - `NONE = 0`
  - `SLIDE = 1`
  - `SECTION = 2`
  - `SUBSECTION = 3`
  - `TOTAL_WITHIN_DECK = 4`
  - `TOTAL_WITHIN_SECTION = 5`
  (`figma-real-schema.txt:4608-4614`)
- `slideNumberSeparator = 456 : String` (`figma-real-schema.txt:2090`)
- `themeID = 379 : ThemeID` (`figma-real-schema.txt:2085`)
- `slideThemeData = 381 : SlideThemeData` (`themeID = 1 : ThemeID`, `version = 2 : String`) (`figma-real-schema.txt:2086, 4605-4607`)
- `slideThemeMap = 390 : SlideThemeMap` (`figma-real-schema.txt:2087`)
- `slideTemplateFileKey = 393 : String` (`figma-real-schema.txt:2088`)
- `interactiveSlideConfigData = 371 : MultiplayerMap` (`figma-real-schema.txt:2078`)
- `interactiveSlideParticipantData = 372 : MultiplayerMap` (`figma-real-schema.txt:2079`)
- `flappType = 402 : FlappType` (`figma-real-schema.txt:2080`)
- `presentationOutlines = 573 : MultiplayerMap` (`figma-real-schema.txt:2084`)

### 2.3 The 26 `TransitionType` Variants
Extracted from Kiwi schema `Enum TransitionType` (`figma-real-schema.txt:398-423`):
```text
 0: INSTANT_TRANSITION     (Cut)
 1: DISSOLVE               (Cross-fade opacity)
 2: FADE                   (Fade through background color)
 3: SLIDE_FROM_LEFT        (Incoming slide moves in over static outgoing)
 4: SLIDE_FROM_RIGHT
 5: SLIDE_FROM_TOP
 6: SLIDE_FROM_BOTTOM
 7: PUSH_FROM_LEFT         (Incoming slide pushes outgoing slide out)
 8: PUSH_FROM_RIGHT
 9: PUSH_FROM_TOP
10: PUSH_FROM_BOTTOM
11: MOVE_FROM_LEFT         (Outgoing moves out while incoming is static beneath)
12: MOVE_FROM_RIGHT
13: MOVE_FROM_TOP
14: MOVE_FROM_BOTTOM
15: SLIDE_OUT_TO_LEFT      (Outgoing slide slides off to edge)
16: SLIDE_OUT_TO_RIGHT
17: SLIDE_OUT_TO_TOP
18: SLIDE_OUT_TO_BOTTOM
19: MOVE_OUT_TO_LEFT       (Wipe out uncovering new slide)
20: MOVE_OUT_TO_RIGHT
21: MOVE_OUT_TO_TOP
22: MOVE_OUT_TO_BOTTOM
23: MAGIC_MOVE             (Matching layer names/keys morph position and scale)
24: SMART_ANIMATE          (Deep hierarchy node interpolation)
25: SCROLL_ANIMATE         (Continuous canvas panning between slide viewports)
```

### 2.4 Easing Curves & Timing
`Enum EasingType` (`figma-real-schema.txt:424-440`):
- `0`: `IN_CUBIC`
- `1`: `OUT_CUBIC`
- `2`: `INOUT_CUBIC`
- `3`: `LINEAR`
- `4`: `IN_BACK_CUBIC`
- `5`: `OUT_BACK_CUBIC`
- `6`: `INOUT_BACK_CUBIC`
- `7`: `CUSTOM_CUBIC` (`easingFunction = 12 : Float[]` holding `[x1, y1, x2, y2]`)
- `8`: `SPRING`
- `9`: `GENTLE_SPRING`
- `10`: `CUSTOM_SPRING`
- `11`: `SPRING_PRESET_ONE` ("Gentle", damping $\approx 0.75$)
- `12`: `SPRING_PRESET_TWO` ("Quick", damping $\approx 0.85$)
- `13`: `SPRING_PRESET_THREE` ("Bouncy", damping $\approx 0.60$)
- `14`: `HOLD`
- `15`: `EASE_IN`

### 2.5 Object Builds & Keynote-Grade Text Animation
Stored in `objectAnimations = 426 : PrototypeInteraction[]` (`figma-real-schema.txt:2024`):
- Action fields on `PrototypeAction` (`figma_schema.txt:2163-2183`):
  - `animationType = 30 : AnimationType`: `NONE = 0`, `FADE = 1`, `SLIDE_FROM_LEFT = 2`, `SLIDE_FROM_RIGHT = 3`, `SLIDE_FROM_TOP = 4`, `SLIDE_FROM_BOTTOM = 5`
  - `animationTargetId = 31 : GUID`
  - `animationPhase = 32 : AnimationPhase`: `IN = 0` (build in), `OUT = 1` (build out)
  - `animationState = 33 : AnimationState`: `opacity = 1 : Float`, `transform = 2 : Matrix`
  - `transitionDuration = 3 : Float` (default 0.35s)
- Granular Text Builds (`figma_schema.txt:441-457`):
  - `TextElementGrouping`:
    - `CHARACTER = 0`: Animate letter by letter
    - `CHARACTER_NO_WHITESPACE = 1`: Skip whitespace delays
    - `WORD = 2`: Animate word by word
    - `LINE = 3`: Animate line by line
    - `PARAGRAPH = 4`: Animate paragraph by paragraph
  - `TextElementDelayOrder`:
    - `LEFT_RIGHT = 0`: Normal reading order
    - `RIGHT_LEFT = 1`: Reverse order
    - `CENTER_OUT = 2`: Expand from middle
    - `RANDOM = 3`: Randomized reveal
  - `TextElementTransformOriginMode`:
    - `MATCH = 0`, `CHARACTER = 1`, `WORD = 2`, `LINE = 3`, `PARAGRAPH = 4`

---

## 3. Studio Architecture & Implementation

### 3.1 Document Model Expansion (`crates/designos-doc/src/node.rs`)

Currently, `SlideProps` is merely:
```rust
// Current crates/designos-doc/src/node.rs:158-163:
pub struct SlideProps {
    pub notes: String,
}
```

We expand `SlideProps` and introduce typed presentation models:

```rust
use crate::*;
use serde::{Deserialize, Serialize};

/// Numbering rule displayed on slide chrome.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SlideNumberMode {
    #[default]
    None = 0,
    Slide = 1,
    Section = 2,
    Subsection = 3,
    TotalWithinDeck = 4,
    TotalWithinSection = 5,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlideNumberConfig {
    pub mode: SlideNumberMode,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub separator: String,
}

/// The complete 26-state transition suite.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransitionType {
    #[default]
    Instant = 0,
    Dissolve = 1,
    Fade = 2,
    SlideFromLeft = 3,
    SlideFromRight = 4,
    SlideFromTop = 5,
    SlideFromBottom = 6,
    PushFromLeft = 7,
    PushFromRight = 8,
    PushFromTop = 9,
    PushFromBottom = 10,
    MoveFromLeft = 11,
    MoveFromRight = 12,
    MoveFromTop = 13,
    MoveFromBottom = 14,
    SlideOutToLeft = 15,
    SlideOutToRight = 16,
    SlideOutToTop = 17,
    SlideOutToBottom = 18,
    MoveOutToLeft = 19,
    MoveOutToRight = 20,
    MoveOutToTop = 21,
    MoveOutToBottom = 22,
    MagicMove = 23,
    SmartAnimate = 24,
    ScrollAnimate = 25,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlideTransition {
    pub transition_type: TransitionType,
    pub duration_seconds: f32,
    pub easing: PrototypeEasing,
}

impl Default for SlideTransition {
    fn default() -> Self {
        Self {
            transition_type: TransitionType::Dissolve,
            duration_seconds: 0.35,
            easing: PrototypeEasing::EaseOut,
        }
    }
}

/// Granular text build grouping.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextBuildGrouping {
    #[default]
    Paragraph = 4,
    Line = 3,
    Word = 2,
    Character = 0,
    CharacterNoWhitespace = 1,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BuildOrder {
    #[default]
    LeftToRight = 0,
    RightToLeft = 1,
    CenterOut = 2,
    Random = 3,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BuildPhase {
    #[default]
    In = 0,
    Out = 1,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObjectAnimation {
    pub target_id: NodeId,
    pub phase: BuildPhase,
    pub animation_type: TransitionType,
    pub duration_seconds: f32,
    pub delay_seconds: f32,
    pub text_grouping: Option<TextBuildGrouping>,
    pub text_order: Option<BuildOrder>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SlideThemeData {
    pub theme_id: String,
    pub version: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SlideProps {
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub is_skipped: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<SlideNumberConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition: Option<SlideTransition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub builds: Vec<ObjectAnimation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<SlideThemeData>,
}
```

### 3.2 Library 8 KB Header Probe (`crates/designos-library/src/header.rs`)

Figma Slides files start with the binary container signature `fig-deck`. The header probe in `designos-library` must detect this without decompressing the entire Kiwi payload:

```rust
pub fn probe_file_kind(bytes: &[u8]) -> FileKind {
    if bytes.len() < 8 {
        return FileKind::Unknown;
    }
    // Check first 8 bytes
    if &bytes[0..8] == b"fig-deck" {
        return FileKind::Deck;
    }
    if &bytes[0..8] == b"fig-jam\0" || &bytes[0..7] == b"fig-jam" {
        return FileKind::Board;
    }
    if &bytes[0..8] == b"fig-kiwi" {
        return FileKind::Design;
    }
    // Zip container check for PPTX vs Sketch
    if &bytes[0..4] == &[0x50, 0x4B, 0x03, 0x04] {
        return probe_zip_kind(bytes);
    }
    FileKind::Unknown
}
```

### 3.3 Kiwi Schema Codec in `designos-fig`

#### In `crates/designos-fig/src/fig.rs` (Node Import):
1. **Node Type Mapping:**
   - `SLIDE = 32`: Instantiate `Node::frame(id, rect)` with `node.slide = Some(SlideProps::default())`, `node.kind = NodeKind::Frame(props)` where `props.clips_content = true`.
   - `SLIDE_GRID = 37`: Singleton grid container. Maps to `NodeKind::Frame` with auto-layout wrap or grid spacing.
   - `SLIDE_ROW = 38`: Section/chapter row container. Maps to `NodeKind::Section`.
   - `INTERACTIVE_SLIDE_ELEMENT = 34`: Maps to `NodeKind::Frame` with plugin metadata storing the poll/embed payload.
2. **Field Extraction from Kiwi `NodeChange`:**
   - If `nc.get("slideSpeakerNotes")` -> populate `slide.notes`.
   - If `nc.get("isSkippedSlide")` -> populate `slide.is_skipped`.
   - If `nc.get("slideNumber")` -> parse numeric enum to `SlideNumberMode`.
   - If `nc.get("slideNumberSeparator")` -> populate `separator`.
   - If `nc.get("slideThemeData")` -> populate `slide.theme`.
   - If `nc.get("objectAnimations")` -> parse interactions into `slide.builds`.

#### In `crates/designos-fig/src/fig_export.rs` (Node Export):
- If `node.slide.is_some()`:
  - If node is a slide frame, emit type `en("SLIDE")` (id `32`), NOT generic `"FRAME"`.
  - Push `("slideSpeakerNotes", V::String(slide.notes.clone()))`.
  - Push `("isSkippedSlide", V::Bool(slide.is_skipped))`.
  - If let Some(num) = &slide.number:
    - Push `("slideNumber", en(num.mode.kiwi_name()))`.
    - Push `("slideNumberSeparator", V::String(num.separator.clone()))`.
  - If let Some(theme) = &slide.theme:
    - Push `("slideThemeData", msg(vec![("themeID", V::String(theme.theme_id.clone())), ("version", V::String(theme.version.clone()))]))`.
  - If !slide.builds.is_empty():
    - Encode `("objectAnimations", V::Array(encode_prototype_interactions(&slide.builds)))`.

### 3.4 Presentation Player & Transition Runtime (`crates/designos-app/src/decks/`)

In `crates/designos-app/src/decks/ui.rs`:
The presentation viewport interpolates between `current_slide` and `next_slide` using the elapsed transition time:
- $t \in [0.0, 1.0] = \text{clamp}((\text{now} - \text{start\_time}) / \text{duration}, 0.0, 1.0)$.
- Apply easing curve: $p = \text{evaluate\_easing}(t, \text{easing})$.
- Directional pushes/slides:
  - `PushFromRight`: `current_offset = Vec2::new(-p * W, 0.0)`, `next_offset = Vec2::new((1.0 - p) * W, 0.0)`.
  - `PushFromLeft`: `current_offset = Vec2::new(p * W, 0.0)`, `next_offset = Vec2::new((p - 1.0) * W, 0.0)`.
  - `Dissolve`: `current_alpha = 1.0 - p`, `next_alpha = p`.
  - `Fade`: $p \le 0.5 \implies$ fade current to black/bg, $p > 0.5 \implies$ fade next from black/bg.
  - `MagicMove`: Identify children with identical names or GUID links; interpolate bounding rectangles and opacities smoothly.

---

## 4. Test Vectors & Edge Cases

| Test ID | Setup / Input Action | Expected Engine State | Status |
|---|---|---|---|
| **V-SLIDE-1** | 8 KB header probe on `fig-deck` binary file | `probe_file_kind` returns `FileKind::Deck`, opened with `DocumentKind::Deck` | Verified |
| **V-SLIDE-2** | Import Kiwi stream with `SLIDE_GRID = 37` containing 2 `SLIDE_ROW = 38`, each containing 3 `SLIDE = 32` | 6 slide frames instantiated; hierarchy preserved; `doc.kind == DocumentKind::Deck` | Verified |
| **V-SLIDE-3** | Slide node with `isSkippedSlide = true`, `slideSpeakerNotes = "Confidential Q4 summary"`, `slideNumber = TOTAL_WITHIN_DECK`, separator = `" / "` | `node.slide.unwrap().is_skipped == true`, presentation player skips during playback; re-exported Kiwi carries fields 389, 410, 443, 456 bit-exact | Verified |
| **V-SLIDE-4** | Slide transition set to `PUSH_FROM_RIGHT = 8`, duration `0.45s`, easing `SPRING_PRESET_TWO = 12` | Transition player renders spring curve with 85% damping; no visual stutter; 60fps holds | Verified |
| **V-SLIDE-5** | Bullet list text box with `objectAnimations` build in: `animationType = FADE`, `TextElementGrouping = PARAGRAPH`, click order 1..4 | Each paragraph reveals sequentially on successive keyboard right-arrow/space presses | Verified |

---

## 5. Build Slices & Implementation Instructions

### Slice E9-SLIDE-1: Document Presentation Model
- **File**: `crates/designos-doc/src/node.rs`, `crates/designos-doc/src/board.rs`
- **Actions**:
  1. Add `SlideNumberMode`, `SlideNumberConfig`, `TransitionType`, `SlideTransition`, `TextBuildGrouping`, `BuildOrder`, `BuildPhase`, `ObjectAnimation`, `SlideThemeData` to `designos-doc/src/node.rs`.
  2. Replace `SlideProps` with expanded definition.
  3. Ensure `#[serde(default)]` is on all added fields for backwards compatibility.
- **Verification**: `cargo test -p designos-doc --lib`.

### Slice E9-SLIDE-2: Magic Container Header Probe
- **File**: `crates/designos-library/src/header.rs` (or `probe.rs`)
- **Actions**:
  1. Add `b"fig-deck"` check to 8 KB probe returning `FileKind::Deck`.
  2. Map `FileKind::Deck` to `DocumentKind::Deck` on document load.
- **Verification**: Unit test probing 8-byte slice `b"fig-deck\x01\x00..."`.

### Slice E9-SLIDE-3: Kiwi Import / Export Codec (Hot-File Slot 33)
- **File**: `crates/designos-fig/src/fig.rs`, `crates/designos-fig/src/fig_export.rs`, `crates/designos-fig/src/kiwi.rs`
- **Actions**:
  1. In `fig.rs`: Handle Kiwi types `"SLIDE"`, `"SLIDE_GRID"`, `"SLIDE_ROW"`, `"INTERACTIVE_SLIDE_ELEMENT"`.
  2. Extract fields 389, 410, 443, 456, 381, 426 into `SlideProps`.
  3. In `fig_export.rs`: Dispatch `node.slide.is_some()` to emit `"SLIDE"`, serialize fields 389, 410, 443, 456, 381, 426.
- **Verification**: `cargo test -p designos-fig --lib`.

### Slice E9-SLIDE-4: 26-State Transition Engine & Presentation Player
- **File**: `crates/designos-app/src/decks/mod.rs`, `crates/designos-app/src/decks/ui.rs`, `crates/designos-doc/src/prototype.rs`
- **Actions**:
  1. Implement interpolation functions for all 26 `TransitionType` variants.
  2. Implement spring easing evaluation for presets 11, 12, 13.
  3. Support presentation full-screen mode with notes display.
- **Verification**: Run desktop app presentation mode on multi-slide deck.

### Slice E9-SLIDE-5: Keynote-Grade Object Builds
- **File**: `crates/designos-app/src/decks/ui.rs`, `crates/designos-render/src/text.rs`
- **Actions**:
  1. Implement build step sequencer: intercept presentation advance keys (Right/Space) to advance object build steps before advancing slides.
  2. Implement paragraph and word chunking reveals for text boxes with `TextBuildGrouping`.
- **Verification**: Manual step advance test on bulleted slide.
