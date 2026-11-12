# Epic E11: Prototyping (#123) — Master Architecture & Slice Roadmap

**Date:** 2026-11-12  
**Milestone:** v0.8.0  
**Target Architecture:** Figma 126.11.7 Clean-Room Parity vs. DESIGN:OS Studio (`design-os-studio`)  
**Base Revision:** `origin/main@e0e44f3c2`  
**Evidence Source:** `/Users/jangtrinh/Products/design-os-studio/plans/261112-epic-figma-rea-specs/evidence/rea-e6-e11-motion-proto.md`  
**REA Evidence ID:** `ev_d8d8b9b95732159bfc18d8f944c89602a78adcbc4ae41358f73fb2823755bbdd`  
**Clean-Room Standard:** Clean-room reverse engineering. Exact Kiwi tags, analytical transition models, and UI strings only. Zero copied Figma source code.

---

## Answer first

1. **Epic E11 brings DESIGN:OS Studio into complete fidelity with Figma 126.11.7 prototyping and presentation**, covering:
   - **`designos-proto` Crate Extraction (Slice P1):** Decoupling prototyping graph traversal, reaction evaluation, interaction state machines, and media clock synchronization from `crates/designos-app` into an independent, `egui`-free library crate `crates/designos-proto`. This allows headless MCP tools, export CLI, and unit test suites to execute the exact same prototype state machine as the desktop presentation player.
   - **Kiwi Schema Parity (Slice P2):** Full round-trip support for media triggers (`ON_MEDIA_HIT = 12`, `ON_MEDIA_END = 13`, `mediaHitTime = 7`, `MediaAction 0..9`), navigate reset flags (`transitionResetScrollPosition = 25`, `transitionResetVideoPosition = 17`, `transitionResetInteractiveComponents = 26`, `transitionPreserveScroll = 11`, `transitionOverridesEnabled = 42`), sticky scrolling (`scrollBehavior = 186`), `PrototypeDevice = 184`, `PrototypeStartingPoint = 249`, and `MakeContentState = 595`.
   - **Smart Animate Engine (Slice P3):** Full hierarchical layer name and ancestor path matching (`FGSmartAnimateTransitionBake` clean-room parity), affine matrix TRS decomposition, seamless morphing of geometry, corner radius, strokes, fills, shadows, and auto-layout spacing, paired with dissolve transitions for unmatched elements.
   - **Hardware Video, Audio & Animated GIF (Slice P4):** Zero-PNG direct-to-GPU video decoding, monotonic audio master clock ($\le 1$ frame drift over 60 seconds), and Kiwi `animatedImage = 16` GIF playback.
   - **Present Mode UX & Hotspots (Slice P5):** 5 viewport scaling modes (`ACTUAL_SIZE`, `FIT_TO_SCREEN`, `FIT_WIDTH`, `FILL_SCREEN`, `RESPONSIVE`) and the authentic 350ms `#18A0FB` blue highlight flash on non-interactive miss-clicks.
   - **Prototype Comments & MCP Automation (Slice P6):** Headless prototype inspection and execution tools (`designos_proto_*`).
2. **Studio Non-Negotiable Invariants (`AGENTS.md` §4):**
   - **Zero Rev Bumps in Present Mode:** Prototype state transitions, interactive component swaps, and video frame steps MUST NEVER bump `TileView::rev` or call `app.changed()`, and MUST NEVER call `doc.to_json()` in `doc_hash`!
   - **GPU Budget:** Smart Animate transforms and opacities reuse cached `GeomCache` tessellations. Dynamic morphs are tessellated within the $8\text{ms}$ budget.

---

## 1. What could NOT be verified (read first)

1. **Voice Event Recognition (`ON_VOICE = 11`):** In Figma desktop, voice triggers interface with browser Web Speech APIs. The local C++ prototype engine does not contain an embedded ASR model; Studio specifies an extensible event stub for voice triggers without bundling an offline speech recognizer.
2. **Proprietary Video Streaming Cache (`video.s3Url`):** In web Figma, video frames can stream over S3 signed URLs. In offline desktop Studio, video playback strictly operates against local file assets and blob hashes (`video.hash = 1`).
3. **Exact JIT Smart Animate Curve Fitting:** The internal threshold for switching from quadratic to cubic bezier approximation during vector path morphing could not be disassembled; Studio employs standard cubic Catmull-Rom spline fitting.

---

## 2. Slice Order & Roadmap

```
┌────────────────────────────────────────────────────────────────────────────────┐
│  Slice P1: Crate Split — crates/designos-proto (egui-free)                     │
│  - Move Player, Easing, State Machine, Graph Traversal from designos-app      │
│  - Zero egui dependency, pure designos-doc consumer                            │
│  - Standalone tests runnable in cargo test -p designos-proto                   │
└──────────────────────────────────────┬─────────────────────────────────────────┘
                                       │
                                       ▼
┌────────────────────────────────────────────────────────────────────────────────┐
│  Slice P2: Kiwi Schema & Round-Trip Parity                                    │
│  - Media triggers (ON_MEDIA_HIT=12, ON_MEDIA_END=13, mediaHitTime=7)           │
│  - MediaAction enum (0..9) & navigate reset flags (25, 17, 26, 11, 42)        │
│  - scrollBehavior=186 (STICKY_SCROLLS=2) & clamping mathematics               │
│  - PrototypeDevice=184, PrototypeStartingPoint=249, MakeContentState=595       │
└──────────────────────────────────────┬─────────────────────────────────────────┘
                                       │
                                       ▼
┌────────────────────────────────────────────────────────────────────────────────┐
│  Slice P3: Smart Animate Tree Matcher & Morphing Engine                        │
│  - Layer Name + Ancestor Path hierarchical match algorithm                     │
│  - Matrix TRS decomposition (translation, rotation, scale)                    │
│  - Interpolation of geometry, radius, auto-layout, fills, strokes, effects     │
│  - Unmatched dissolve fade-in/fade-out handling                                │
└──────────────────────────────────────┬─────────────────────────────────────────┘
                                       │
                                       ▼
┌────────────────────────────────────────────────────────────────────────────────┐
│  Slice P4: Direct-to-GPU Video, Audio Service & Animated GIF                  │
│  - Replace OpenH264 PNG bounce with direct GPU texture upload                 │
│  - Monotonic system audio clock (<= 1 frame drift over 60s)                    │
│  - Animated GIF: Paint::animatedImage=16, image=8 poster, animationFrame=17   │
└──────────────────────────────────────┬─────────────────────────────────────────┘
                                       │
                                       ▼
┌────────────────────────────────────────────────────────────────────────────────┐
│  Slice P5: Present Mode UX & Hotspot Hints                                    │
│  - 5 Zoom modes: ACTUAL_SIZE, FIT_TO_SCREEN, FIT_WIDTH, FILL_SCREEN, RESPONSIVE│
│  - 350ms #18A0FB hotspot hint flash on empty clicks                            │
│  - Device bezels and presentation navigation bar                               │
└──────────────────────────────────────┬─────────────────────────────────────────┘
                                       │
                                       ▼
┌────────────────────────────────────────────────────────────────────────────────┐
│  Slice P6: Prototype Comments & MCP Automation Tools                           │
│  - Spatial comments attached to prototype frames and screen coordinates        │
│  - designos_proto_* MCP automation endpoints for headless agent verification  │
└────────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Hot-File Queue & Edit Serialization

| File Path | Owning Slice | Lock Reason | Concurrent Edits Allowed? |
|---|---|---|---|
| `crates/designos-proto/src/` | Slice P1 | New crate creation & core data structures | **NO** — Complete Slice P1 first |
| `crates/designos-doc/src/prototype.rs` | Slice P1, Slice P2 | Struct migrations & reset flags | **NO** — Staged by slice |
| `crates/designos-fig/src/proto.rs` | Slice P2 | .fig Kiwi interaction converter | **NO** — Staged by slice |
| `crates/designos-fig/src/kiwi.rs` | Slice P2 | Kiwi schema binary tags | Safe after Slice P2 |
| `crates/designos-app/src/proto/player.rs` | Slice P1, Slice P3 | Player delegation to `designos-proto` | **NO** — Complete Slice P1 first |
| `crates/designos-app/src/present.rs` | Slice P5 | Present mode UI & hotspot hints | Safe after Slice P5 |
| `crates/designos-app/src/video.rs` | Slice P4 | GPU video texture pipeline | Safe after Slice P4 |
| `crates/designos-mcp/src/lib.rs` | Slice P6 | MCP prototype tools | Safe after Slice P6 |

---

## 4. Model-Tier Routing & Agent Delegation Contract

In strict compliance with the **Antigravity Sub-Agent Cost-Optimization Protocol (`GEMINI.md`)**:

- **Gemini 4 Carbon (`coordinator` / `architect`):**
  - High-level architecture, crate boundary review, and final gate verification.
- **Gemini 4 Argon (`Model: "pro"`):**
  - Complex implementation tasks:
    - Smart Animate hierarchical tree matcher & matrix TRS decomposition (`crates/designos-proto/src/smart.rs`).
    - Direct-to-GPU video decoder and audio clock sync (`crates/designos-app/src/video.rs`).
    - Present stage layout algorithms and zoom reflow logic (`crates/designos-app/src/present.rs`).
- **Gemini 3.8 Flash (`Model: "flash"`):**
  - Machine-checkable, spec-bound tasks:
    - Moving pure files into `crates/designos-proto` and updating `Cargo.toml`.
    - Kiwi enum mappings in `crates/designos-fig/src/proto.rs`.
    - 350ms hotspot overlay timer and UI color styling.
    - Test execution (`cargo test -p designos-proto`, `cargo test -p designos-fig`).
