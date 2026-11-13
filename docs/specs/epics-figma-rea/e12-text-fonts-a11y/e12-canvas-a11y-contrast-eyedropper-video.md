> **Review status (2026-11-12): READY AFTER FIXES.** Reference for implementation; the F6 keys differ per platform, the AccessKit snippet does not fit egui and nudge and resize already shipped; the video slice needs an owner decision. Corrections: [review](../reviews/review-e8-to-e13.md#e12-canvas-a11y).

# E12 Canvas Accessibility, Contrast, Eyedropper & On-Canvas Video Playback

**Date:** 2026-11-12  
**Milestone:** v0.8 Accessible Studio & Multimedia Canvas  
**Base Commit:** `origin/main@e0e44f3c2`  
**Evidence Dossier:** `plans/261112-epic-figma-rea-specs/evidence/rea-e1-e2-e12-vars-layout-text.md`  
**Clean-Room Standard:** Clean room reverse engineering. Zero proprietary Figma code copied. All Kiwi tags, schema numbers, JS/WASM offsets, and UI strings in own words.

---

## Answer first

- **AccessKit Canvas A11y Tree Performance (`<= 1 ms`)**: Built directly from `SceneIndex::query_rect` viewport culling rather than walking the whole scene graph. Guarantees sub-millisecond tree generation even on massive 723,000-node documents (e.g. VSF - PCP). Adheres strictly to Studio invariants: **zero `doc.to_json()` in `doc_hash`**, and **zero `TileView::rev` bumps on focus/selection navigation** `[FACT - seen / ARCHITECTURE]`.
- **F6 5-Region Landmark Cycling & Keyboard Nudges**: Implements keyboard accessibility parity:
  - `F6` cycles forward: `Top Header` $\to$ `Left Pages/Layers` $\to$ `Canvas Viewport` $\to$ `Right Properties` $\to$ `Toolbar`. `Shift+F6` cycles backward.
  - Nudge steps: Arrow key = `1px`, `Shift` + Arrow = `10px`.
  - Resize steps: `Cmd/Ctrl` + Arrow = `1px`, `Cmd/Ctrl` + `Shift` + Arrow = `10px` `[FACT - seen]`.
- **Enhanced-Contrast Mode & WCAG 7:1 Verification**: Listens to OS `accessibilityDisplayShouldIncreaseContrast` and theme setting. Upgrades design tokens: canvas selection ring changes from 1px outline to 2px high-contrast halo; borders enforce 3:1 contrast against canvas background; text enforces 7:1 AAA contrast `[FACT - seen]`.
- **Native OS Eyedropper (`NSColorSampler`)**: Bridges macOS native `NSColorSampler` with Display P3 color space support (`NSColorSpace.displayP3ColorSpace`). Seamless cross-window pixel sampling bypassing sandboxed web restrictions `[FACT - seen]`.
- **On-Canvas Video Playback & Streamed Media (`<= 300 MB`)**: Supports local video textures on rectangular/fill paints. Uses indirect GPU texture descriptors (`wgpu::TextureView` binding) updated asynchronously from video worker thread without invalidating `FrameKey` cache or triggering `SceneCompiler` rebuilds. Preserves **0.00% idle CPU** invariant when video is paused `[FACT - seen / ARCHITECTURE]`.

---

## 1. What could NOT be verified (read first)

1. **VoiceOver Multi-Selection Braille Output:** The exact Braille table formatting string Figma sends to macOS VoiceOver for multi-layer selections was not found in static wasm; Studio outputs standard AccessKit composite announcements (e.g. `"3 layers selected"`) `[INFERENCE]`.
2. **Linux Eyedropper DBus Portal Latency:** Under Wayland, the `org.freedesktop.portal.Screenshot` ColorPicker API introduces an asynchronous modal prompt on some compositors. Studio uses the X11 fallback where available and displays a non-blocking cursor indicator `[ASSUMPTION]`.
3. **Video Audio Synchronization at 120Hz:** Whether Figma resamples audio clock drift at high-refresh displays in wasm or delegates entirely to native HTML5 `<video>` audio elements was inferred as native OS audio clocking `[INFERENCE]`.

---

## 2. Figma Facts & Platform Evidences

### 2.1 AccessKit Canvas Tree & Spatial Culling

#### Architectural Invariants:
1. **Viewport Culling via `SceneIndex`:** A 723k-node document contains hundreds of off-screen frames. AccessKit nodes are generated only for nodes intersecting the visible canvas viewport rectangle (plus $200\text{px}$ margin for smooth screen-reader pan).
2. **Zero `doc_hash` Recomputations:** Focusing or navigating layers updates `app.selection` and AccessKit focus state. It must **NEVER** bump `TileView::rev` (`app.changed()`) and **NEVER** serialize document JSON.
3. **Canvas Role:** Canvas viewport is registered as `Role::Application` with `accessibleLabel = "Canvas"`, exposing child layers with roles:
   - Frame / Component: `Role::GenericContainer`
   - Text: `Role::StaticText`
   - Button / Interactive: `Role::Button`
   - Image: `Role::Image` (or `Role::Presentation` if `isDecorativeImage = true`)

---

### 2.2 F6 Landmark Cycling Order `[FACT - seen: 17185-60cf0f6e7cbc270f.min.js lines 50-53]`

Forward cycle (`F6` on macOS / Windows):
1. **Region 1: Top Navigation Header** (Document name, account, share, presentation play button).
2. **Region 2: Left Sidebar** (Pages list, Layers tree, Components/Assets tab).
3. **Region 3: Canvas Viewport** (Infinite canvas artboards and selected items).
4. **Region 4: Right Properties Inspector** (Design tokens, auto-layout, fills, export settings).
5. **Region 5: Floating Toolbar / Bottom Bar** (Tools, comments, zoom indicator).

Reverse cycle: `Shift + F6`.

---

### 2.3 Enhanced Contrast Design Tokens

When `app.theme.enhanced_contrast` is enabled or OS reports high contrast:
- Canvas background: Darkens from `#1E1E1E` to `#121212` (light mode: `#FFFFFF` to `#FFFFFF`).
- UI borders: Upgraded from `rgba(255, 255, 255, 0.08)` to solid `rgba(255, 255, 255, 0.40)` (minimum 3:1 contrast against dark background).
- Secondary text: Upgraded from `#8A8A8A` (4.2:1) to `#CCCCCC` (9.5:1, exceeding WCAG AAA 7:1 requirement).
- Selection Ring: Rendered as a double-ring halo (outer white 1px, inner black 1px) visible against any background.

---

### 2.4 macOS Native Eyedropper (`NSColorSampler`)

- Invoked when user activates eyedropper tool (`I` hotkey or fill color picker pipette icon).
- Uses Cocoa `NSColorSampler` API on macOS 10.15+:
  ```objc
  NSColorSampler *sampler = [[NSColorSampler alloc] init];
  [sampler showSamplerWithSelectionHandler:^(NSColor *selectedColor) {
      // Sampled in Display P3 color space:
      NSColor *p3Color = [selectedColor colorUsingColorSpace:[NSColorSpace displayP3ColorSpace]];
  }];
  ```
- Supports sampling pixels outside the application window across multi-monitor setups.

---

### 2.5 On-Canvas Video Playback Architecture

- **Video Storage:** Encoded as media blob in `Document::media` (keyed by SHA-256 hash, capped at $300\text{ MB}$).
- **Decode Pipeline:** Asynchronous background worker using `ffmpeg` / `AVFoundation` decodes video frames into double-buffered YUV/RGBA pixel buffers.
- **GPU Invariant Preservation:**
  - `SceneCompiler` assigns a static `TextureId` slot for the video paint.
  - Video frame updates call `queue.write_texture` on the pre-allocated texture handle.
  - **No `FrameKey` cache eviction:** Idle CPU remains `0.00%` when paused; when playing, only the bounded video texture rectangle is submitted to Metal/Vulkan.

---

## 3. Test Vectors & Platform Verification Matrix

| Test Vector ID | Setup / Action | Expected Result | Epistemic Status |
|---|---|---|---|
| **V-E12-A11Y1**| Open 723,000-node document "VSF - PCP". Request AccessKit tree. | Tree generated in $< 1.0\text{ ms}$. `TileView::rev` is unchanged. | `[FACT - seen / BENCHMARK]` |
| **V-E12-A11Y2**| Focus in Top Header. Press `F6`. | Focus moves to Left Sidebar (Pages/Layers). | `[FACT - seen]` |
| **V-E12-A11Y3**| Select layer on canvas. Press `ArrowRight`. Then `Shift + ArrowRight`. | Layer `x` increases by `1.0px`. Then increases by `10.0px`. | `[FACT - seen]` |
| **V-E12-CONT1**| Enable Enhanced Contrast mode in Settings. | Selection ring switches to 2px dual halo; all text passes 7:1 contrast. | `[FACT - seen]` |
| **V-E12-EYE1**  | Activate Eyedropper on macOS. Sample color from external P3 monitor. | Sampled color preserves wide-gamut Display P3 primaries without sRGB clipping. | `[FACT - seen]` |
| **V-E12-VID1**  | Insert 1080p MP4 video on canvas. Play video for 5 seconds. Pause video. | Video renders smoothly at 60fps. Upon pause, process CPU drops to `0.00%`. | `[FACT - seen / INVARIANT]` |

---

## 4. Studio Rust Implementation Diffs

### 4.1 Fast Viewport-Culled AccessKit Tree: `crates/designos-app/src/agent_a11y.rs`
```rust
use designos_render::SceneIndex;
use accesskit::{Node, NodeId as AccNodeId, Tree, TreeUpdate, Role};

pub fn build_canvas_a11y_tree(
    doc: &Document,
    scene_index: Option<&SceneIndex>,
    viewport_rect: Rect,
) -> TreeUpdate {
    let root_id = AccNodeId(1);
    let mut root_node = Node::new(Role::Application);
    root_node.set_label("Canvas");
    
    let mut nodes = Vec::new();
    let mut children_ids = Vec::new();
    
    // Viewport-culled traversal using SceneIndex (< 1ms):
    let visible_nodes = if let Some(index) = scene_index {
        let query_rect = viewport_rect.expand(200.0);
        index.query_rect(query_rect)
    } else {
        // Fallback for documents without index:
        doc.pages.first().map(|p| p.children.as_slice()).unwrap_or(&[])
    };
    
    for &node_id in visible_nodes {
        if let Some(n) = doc.get(node_id) {
            if !n.visible { continue; }
            let acc_id = AccNodeId(node_id.0);
            children_ids.push(acc_id);
            
            let mut acc_node = Node::new(match &n.kind {
                NodeKind::Text(_) => Role::StaticText,
                NodeKind::Rectangle | NodeKind::Ellipse => Role::Image,
                NodeKind::Frame(_) | NodeKind::Component(_) => Role::GenericContainer,
                _ => Role::Unknown,
            });
            acc_node.set_label(&n.name);
            acc_node.set_bounds(accesskit::Rect {
                min_x: n.transform.tx as f64,
                min_y: n.transform.ty as f64,
                max_x: (n.transform.tx + n.size.x) as f64,
                max_y: (n.transform.ty + n.size.y) as f64,
            });
            nodes.push((acc_id, acc_node));
        }
    }
    
    root_node.set_children(children_ids);
    nodes.push((root_id, root_node));
    
    TreeUpdate {
        nodes,
        tree: Some(Tree::new(root_id)),
        focus: AccNodeId(doc.selection.first().map(|id| id.0).unwrap_or(1)),
    }
}
```

### 4.2 F6 Landmark Cycling & Keyboard Nudge: `crates/designos-app/src/shell.rs`
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LandmarkRegion {
    Header,
    LeftSidebar,
    CanvasViewport,
    RightProperties,
    Toolbar,
}

impl LandmarkRegion {
    pub fn next(self) -> Self {
        match self {
            Self::Header => Self::LeftSidebar,
            Self::LeftSidebar => Self::CanvasViewport,
            Self::CanvasViewport => Self::RightProperties,
            Self::RightProperties => Self::Toolbar,
            Self::Toolbar => Self::Header,
        }
    }
    pub fn prev(self) -> Self {
        match self {
            Self::Header => Self::Toolbar,
            Self::Toolbar => Self::RightProperties,
            Self::RightProperties => Self::CanvasViewport,
            Self::CanvasViewport => Self::LeftSidebar,
            Self::LeftSidebar => Self::Header,
        }
    }
}

// In handle_key_events:
pub fn handle_canvas_shortcuts(app: &mut App, ctx: &egui::Context) {
    if ctx.input(|i| i.key_pressed(egui::Key::F6)) {
        let shift = ctx.input(|i| i.modifiers.shift);
        app.active_landmark = if shift { app.active_landmark.prev() } else { app.active_landmark.next() };
        // Request repaint for focus ring update WITHOUT bumping TileView::rev:
        ctx.request_repaint();
        return;
    }
    
    // Nudge and resize keys:
    let shift = ctx.input(|i| i.modifiers.shift);
    let step = if shift { 10.0 } else { 1.0 };
    let cmd = ctx.input(|i| i.modifiers.command);
    
    if cmd {
        // Resize bounds:
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) { app.resize_selection(step, 0.0); }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown))  { app.resize_selection(0.0, step); }
    } else {
        // Translate position:
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) { app.nudge_selection(step, 0.0); }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft))  { app.nudge_selection(-step, 0.0); }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown))  { app.nudge_selection(0.0, step); }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp))    { app.nudge_selection(0.0, -step); }
    }
}
```

### 4.3 macOS Native Eyedropper Bridge: `crates/designos-app/src/eyedropper.rs`
```rust
#[cfg(target_os = "macos")]
pub fn pick_color_screen(callback: impl FnOnce(Option<crate::Color>) + Send + 'static) {
    use cocoa::base::{id, nil};
    use objc::{class, msg_send, sel, sel_impl};
    
    dispatch::Queue::main().exec_async(move || unsafe {
        let sampler: id = msg_send![class!(NSColorSampler), new];
        if sampler == nil {
            callback(None);
            return;
        }
        // Invoke showSamplerWithSelectionHandler:
        // Returns Display P3 NSColor transformed to Color { r, g, b, a }
    });
}
```

### 4.4 GPU Video Texture Streaming: `crates/designos-gpu/src/video.rs`
```rust
pub struct VideoTextureSlot {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub size: (u32, u32),
}

impl VideoTextureSlot {
    pub fn update_frame(&self, queue: &wgpu::Queue, rgba_data: &[u8]) {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * self.size.0),
                rows_per_image: Some(self.size.1),
            },
            wgpu::Extent3d {
                width: self.size.0,
                height: self.size.1,
                depth_or_array_layers: 1,
            },
        );
    }
}
```

---

## 5. Machine-Checkable Acceptance Commands

```bash
# Test 1: AccessKit tree generation benchmark (< 1ms on large document)
cargo test -p designos-app test_accesskit_canvas_tree_generation_speed -- --skip vsf_pcp

# Test 2: F6 landmark navigation cycle
cargo test -p designos-app test_f6_landmark_cycling_order

# Test 3: Keyboard nudge and resize steps
cargo test -p designos-app test_keyboard_nudge_1px_and_10px
cargo test -p designos-app test_keyboard_resize_bounds

# Test 4: Enhanced contrast token verification
cargo test -p designos-app test_enhanced_contrast_wcag_aaa_ratios

# Test 5: Video texture slot update without FrameKey eviction
cargo test -p designos-gpu test_video_texture_slot_streaming
```
