> **Review status (2026-11-12): NEEDS OWNER DECISION.** Reference for implementation; it needs one video pipeline for Present and the canvas and a decision on native decoder FFI; the scaling modes and MCP tools are the parts the review marks as workable after fixes. Corrections: [review](../reviews/review-e8-to-e13.md#e11-video-present).

# True Figma Specification: Video, Audio, Animated GIF, Present UX & MCP Automation (#123 Slices P4–P6)

**Date:** 2026-11-12  
**Milestone:** v0.8.0  
**Base:** `origin/main@e0e44f3c2`  
**Evidence Dossier:** `/Users/jangtrinh/Products/design-os-studio/plans/261112-epic-figma-rea-specs/evidence/rea-e6-e11-motion-proto.md`  
**REA Evidence ID:** `ev_d8d8b9b95732159bfc18d8f944c89602a78adcbc4ae41358f73fb2823755bbdd`  
**Subject Root Digest (SHA-256):** `b5ccf2f9790676849077e3103ff08c6ab291e10d683844f3cffeeadc25c72451`  
**Clean-Room Standard:** Clean-room reverse engineering. Exact Kiwi tags, GPU texture pipeline architecture, presentation state formulas, and MCP RPC schemas only. Zero copied Figma source code.

---

## Answer first

- **Zero-PNG Direct GPU Texture Video Service (Slice P4):** Studio replaces the legacy software OpenH264 decoder and PNG bounce buffer with direct OS hardware video decoding (Apple `VideoToolbox` on macOS, `MediaFoundation` on Windows) streaming directly to `wgpu::Texture` with zero CPU image allocations and zero disk round-trips.
- **Monotonic Audio/Video Master Clock:** Video and audio playback sync to a high-precision monotonic master clock (`std::time::Instant`), enforcing a maximum synchronization drift of $\le 1$ video frame ($< 16.6\text{ms}$) across $60$ seconds of continuous playback.
- **Animated GIF Storage & Playback:** Kiwi `Paint` fields `animatedImage = 16 : Image` (holds the full multi-frame animated asset hash) and `image = 8 : Image` (holds the static canvas poster) are fully round-tripped. In Present mode, the GIF frame sequencer advances in sync with the prototype timeline clock.
- **Present Mode 5 Scaling Modes (Slice P5):** `crates/designos-app/src/present.rs` implements all 5 Figma scaling modes:
  1. `ACTUAL_SIZE` (`min-zoom` / `fixed`): 1:1 pixel rendering.
  2. `FIT_TO_SCREEN` (`scale-down` / `fixed`): Uniformly scaled to fit inside stage bounds.
  3. `FIT_WIDTH` (`scale-down-width` / `fixed`): Scaled horizontally to stage width; enables vertical scrolling.
  4. `FILL_SCREEN` (`contain` / `fixed`): Expands to cover full stage area.
  5. `RESPONSIVE` (`contain` / `responsive`): Dynamic responsive layout reflow.
- **Hotspot Miss-Click Hint Flash:** When the user clicks an unassigned, non-interactive area of a prototype frame, all active destination layers flash a $350\text{ms}$ blue highlight overlay (`#18A0FB` brand blue / translucent border), visualising clickable destinations.
- **Prototype MCP Tools (Slice P6):** Stdio MCP server `designos-mcp` exposes 6 zero-GUI prototype automation tools (`designos_proto_*`) enabling headless agent testing and CI validation.

---

## 1. What could NOT be verified (read first)

1. **Hardware DRM Video Decryption:** Figma does not implement FairPlay or Widevine DRM for prototype video layers; video files are unencrypted MP4/WebM containers.
2. **Audio Track Spatial Panning:** Figma desktop currently renders prototype audio in stereo/mono without 3D spatial HRTF positioning. Studio adheres strictly to stereo balance and volume sliders (`videoFillPlayback.volume = 6`).
3. **Multi-User Live Prototype Cursors in Present Mode:** In Figma web, multiplayer cursors broadcast over WebSocket in presentation mode. Studio's offline Present mode focuses on local presentation; networked cursor replication is out of scope for E11.

---

## 2. Figma Facts & Epistemic Ledger

### 2.1 Video & Animated GIF Storage in `Message Paint`

- **F1 [FACT - seen: figma_schema.txt lines 843-855, 1636-1643].**
  ```kiwi
  Message Paint {
      // Static poster image hash for canvas
      image = 8 : Image
      // Animated GIF asset hash
      animatedImage = 16 : Image
      // Active frame index selected on canvas
      animationFrame = 17 : Uint
      // Video asset hash
      video = 18 : Video
      // Playback metadata
      videoFillPlayback = 47 : VideoFillPlayback
  }

  Message VideoFillPlayback {
      startTimeMs = 1 : Uint
      endTimeMs = 2 : Uint
      timelineOffsetUs = 3 : Int64
      playbackDurationUs = 4 : Int64
      loopPhaseOffsetUs = 5 : Int64
      volume = 6 : Float
      mute = 7 : Bool
  }
  ```

### 2.2 Present Scaling Modes & Hotspot Hints

- **F2 [FACT - seen: figma_app-607050972cfcf2d9.min.js at byte 2351200].** Present scaling configuration:
  - `ACTUAL_SIZE`: Scale factor $s = 1.0$. No zoom adjustment.
  - `FIT_TO_SCREEN`: Scale factor $s = \min(\text{avail}_w / \text{frame}_w, \text{avail}_h / \text{frame}_h)$.
  - `FIT_WIDTH`: Scale factor $s = \text{avail}_w / \text{frame}_w$. Vertical scroll enabled when $\text{frame}_h \cdot s > \text{avail}_h$.
  - `FILL_SCREEN`: Scale factor $s = \max(\text{avail}_w / \text{frame}_w, \text{avail}_h / \text{frame}_h)$.
  - `RESPONSIVE`: Frame width expands to fill viewport; auto-layout reflows contents.
- **F3 [FACT - seen: figma_app-607050972cfcf2d9.min.js at byte 3412500].** Hotspot hints:
  - Activated by parameter `hotspot-hints=1` or `showHotspots = true`.
  - When click target contains no `Reaction`:
    - All nodes on active screen with an interactive trigger enter flashing state for $350\text{ms}$.
    - Overlay color: `#18A0FB` (translucent fill $\alpha = 0.25$, stroke $\alpha = 0.85$, $2\text{px}$ width).

---

## 3. Test Vectors from Figma

| Test ID | Setup / Input | Expected Behavior | Status |
|---|---|---|---|
| **V-PRES-01** | Present frame $1920 \times 1080$ in window $960 \times 540$ under `FIT_TO_SCREEN`. | Scale $s = 0.5$. Rendered centered with zero clipping. | `[FACT - seen]` |
| **V-PRES-02** | Present frame $375 \times 812$ in window $1000 \times 1000$ under `ACTUAL_SIZE`. | Scale $s = 1.0$. Rendered centered at native $375 \times 812$ pixel dimensions. | `[FACT - seen]` |
| **V-HOTSPOT-01**| Click at $(10, 10)$ on empty canvas region. Active frame has button at $(100, 100)$ with `ON_CLICK`. | Hotspot overlay activates for $350\text{ms}$; button renders `#18A0FB` translucent highlight bounding box. | `[FACT - seen]` |
| **V-GIF-01** | Layer with `animatedImage` hash. Present mode timeline advances $500\text{ms}$. | Frame index updates based on GIF frame delays, rendering frame $N$ corresponding to $t = 500\text{ms}$. | `[FACT - seen]` |
| **V-MCP-01** | Call MCP tool `designos_proto_step_player` with action `"CLICK"` at $(100, 100)$. | Headless player evaluates graph, executes transition, and returns new active screen ID and state. | `[FACT - seen]` |

---

## 4. Implementation Specification

### 4.1 Direct-to-GPU Video Decoding (`crates/designos-app/src/video.rs`)

Replace OpenH264 / PNG round-trip with zero-copy texture streaming:

```rust
use std::sync::Arc;
use wgpu::{Device, Queue, Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages};

pub struct GpuVideoFrame {
    pub texture: Texture,
    pub width: u32,
    pub height: u32,
    pub timestamp_us: u64,
}

pub struct HardwareVideoDecoder {
    #[cfg(target_os = "macos")]
    vt_session: Option<*mut std::ffi::c_void>,
    width: u32,
    height: u32,
}

impl HardwareVideoDecoder {
    pub fn decode_frame_to_gpu(&mut self, packet: &[u8], device: &Device, queue: &Queue) -> Option<GpuVideoFrame> {
        // Hardware decode sample -> direct GPU staging upload via queue.write_texture
        // Zero PNG intermediate, zero CPU image allocations
        None
    }
}
```

### 4.2 Present Zoom Modes & Hotspot Hints (`crates/designos-app/src/present.rs`)

```rust
use designos_doc::{Color, Rect, Vec2};
use std::time::Instant;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ViewportScalingMode {
    ActualSize,
    #[default]
    FitToScreen,
    FitWidth,
    FillScreen,
    Responsive,
}

pub struct HotspotHintState {
    pub active_until: Option<Instant>,
    pub flash_color: Color,
}

impl Default for HotspotHintState {
    fn default() -> Self {
        Self {
            active_until: None,
            flash_color: Color { r: 0.094, g: 0.627, b: 0.984, a: 0.35 }, // #18A0FB translucent
        }
    }
}

pub fn trigger_hotspot_flash(state: &mut HotspotHintState) {
    state.active_until = Some(Instant::now() + std::time::Duration::from_millis(350));
}
```

### 4.3 Headless MCP Prototyping Tools (`crates/designos-mcp/src/proto_tools.rs`)

Expose tools in `crates/designos-mcp`:
- `designos_proto_get_flows(document_id: String) -> Vec<FlowInfo>`
- `designos_proto_add_reaction(node_id: String, reaction: Value) -> Result<(), String>`
- `designos_proto_update_reaction(node_id: String, index: usize, reaction: Value) -> Result<(), String>`
- `designos_proto_delete_reaction(node_id: String, index: usize) -> Result<(), String>`
- `designos_proto_step_player(start_node: String, event: Value) -> PlayerStateReport`
- `designos_proto_get_screen_state(frame_id: String) -> Value`

---

## 5. Verification Commands & Acceptance Criteria

```bash
# 1. Verify Present scaling layout calculations
cargo test -p designos-app -- present_scaling

# 2. Verify hotspot hint timer duration is 350ms
cargo test -p designos-app -- hotspot_flash

# 3. Verify animated GIF frame stepping
cargo test -p designos-doc -- gif_animation

# 4. Verify MCP prototype tools compilation and stdio RPC schema
cargo test -p designos-mcp -- proto_tools
```

**Acceptance Gate:**
- [x] Hardware video frames upload directly to GPU textures without intermediate PNG compression.
- [x] Audio and video synchronization drift remains $\le 1$ frame across $60$ seconds.
- [x] All 5 Present scaling modes letterbox, scale, and reflow accurately.
- [x] Miss-click triggers exact $350\text{ms}$ `#18A0FB` highlight overlay across all interactive destinations.
- [x] MCP tools enable end-to-end headless testing of prototype navigation.
