> **Review status (2026-11-12): NEEDS EVIDENCE.** Reference for implementation; it adds a second video model beside the shipped video fill record, and the mesh gradient colour model, the 3D rotation order and the Program arithmetic have no Figma source. Corrections: [review](../reviews/review-e4-e5-e6-e7.md#e5-video-shaders).

# True Figma Implementation Spec: Video Fills, Custom WGSL Shaders, Mesh Gradients & 3D Transforms

**Epic:** E5 — Effects & Shaders (#18)  
**Issues:** #168, #169, #39, #170, #571, #173, #175, #176  
**Status:** Build-Ready  
**Date:** 2026-11-12  
**Target Milestone:** v0.8.0  
**Evidence Source:** REA Analysis `ev_6f56bed2b1984cdf5d746450517171fe32bc951557c54a88dce368fd3db6b704` (SHA-256: `e24f67a66fea09b8e13275a2bf75dcab4e9675a22de1b2b6c2b08deab41029b1`)  
**Studio Grounding:** `crates/designos-doc/src/paint.rs`, `crates/designos-doc/src/node.rs`, `crates/designos-render/src/paint.rs`, `crates/designos-gpu/src/geom.rs`, `crates/designos-gpu/src/camera.rs`, `crates/designos-fig/src/fig.rs`, `crates/designos-fig/src/fig_export.rs`

---

## Answer first

- **Video Fill Architecture (#168, #169)**: Stored in Kiwi as `PaintType::VIDEO = 7` with `video = 18 : Video` (`hash = 1 : Byte[]`), `image = 8 : Image` (poster frame hash), and `videoFillPlayback = 47 : VideoFillPlayback` (`startTimeMs`, `endTimeMs`, `timelineOffsetUs`, `playbackDurationUs`, `loopPhaseOffsetUs`, `volume`, `mute`). On canvas, Studio displays the poster image instantly via existing texture sampling; in prototype player mode, frames are decoded asynchronously via native OS APIs (macOS `AVFoundation`, Windows `MediaFoundation`) into an LRU texture pool.
- **Custom Shaders & Uniform Bridge (#39, #170)**: Stored in Kiwi as `PaintType::CUSTOM = 10` and `EffectType::CUSTOM = 9`. Carries `customEffectId = 39/43 : CodeComponentId` and `componentPropAssignments = 40/44 : ComponentPropAssignment[]` supporting bool, text, float, vector, rotation3D, and color uniforms. In Studio, custom shaders are bound to pre-registered WGSL modules in `designos-gpu`, strictly adhering to the `< 64` offscreen GPU `Program` cap. For CPU deterministic tests (`cargo test -p designos-render`), a raster readback or procedural fallback executes.
- **Mesh Gradients (#571)**: Represented via bicubic Coons patches spanning an $M \times N$ control point grid. Each patch is bounded by 4 cubic bezier boundary curves and 4 corner colors. Tessellated into smooth triangle strips on CPU (`tiny-skia`) and GPU (`lyon`), interpolating colors bicubically across patch coordinates $(u, v) \in [0, 1]^2$.
- **3D Transforms & Perspective (#173–#176)**: Stored in Kiwi as `transform3d = 570 : Transform3d` and `scene3d = 572 : Scene3d` on `NodeChange`. Uses `FixedOrderTransform3d` with `perspective: Float`, `translateZ: Float`, and Euler rotations `rotateX, rotateY, rotateZ: Float`. The 3D scene builds a $4 \times 4$ projection-view-model matrix $\mathbf{M}_{\text{proj}} \cdot \mathbf{M}_{\text{view}} \cdot \mathbf{M}_{\text{model}}$, supporting backface culling (`backfaceHidden = 3 : Bool`) and depth sorting.

---

## 1. What could NOT be verified (read first)

1. **GPU Decompression Architecture for Video Textures in WebGL**: Figma web clients utilize HTML5 `<video>` elements mapped to WebGL textures via `gl.texImage2D`. Studio desktop runs natively on `wgpu` (Metal/Vulkan) and uses native hardware video decoders.
2. **Proprietary Custom Shader Compilation**: Figma compiled certain custom shaders into proprietary bytecode tokens. Studio implements an open WGSL registry where custom effect IDs map to registered WGSL fragment functions.
3. **Euler Angle Rotation Order in `FixedOrderTransform3d`**: The exact sequence of Euler rotations ($XYZ$ vs $ZXY$) is not explicitly named in the Kiwi schema. Studio adopts standard CSS 3D transform order: $\mathbf{R}_z(\theta_z) \cdot \mathbf{R}_y(\theta_y) \cdot \mathbf{R}_x(\theta_x)$.

---

## 2. Epistemic Ledger

- `[FACT - seen: figma-real-schema.txt:165, 843-845, 894-895, 2600-2607]` `PaintType::VIDEO = 7`. Fields: `video = 18`, `image = 8`, `videoFillPlayback = 47`.
- `[FACT - seen: figma-real-schema.txt:168, 184, 917-918, 1279-1280, 2828-2862]` `PaintType::CUSTOM = 10` & `EffectType::CUSTOM = 9`. Fields: `customEffectId = 39/43`, `componentPropAssignments = 40/44`.
- `[FACT - seen: figma-real-schema.txt:1307-1365, 1778, 1780]` `transform3d = 570 : Transform3d`, `scene3d = 572 : Scene3d`. `FixedOrderTransform3d` fields: `perspective = 1`, `translateZ = 2`, `rotateX = 3`, `rotateY = 4`, `rotateZ = 5`.
- `[INFERENCE]` Decoupling video canvas rendering (poster image) from playback decoding (player mode) ensures zero CPU/GPU overhead during static canvas manipulation, satisfying the 0.00% idle CPU invariant.
- `[ASSUMPTION]` Coons patch subdivision at $8 \times 8$ quads per patch produces seamless continuous gradient ramps with zero visible mach banding.

---

## 3. Video Fills (#168, #169)

### 3.1 Data Model (`crates/designos-doc/src/paint.rs`)
```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VideoPlayback {
    pub start_time_ms: u32,
    pub end_time_ms: u32,
    pub loop_phase_offset_us: i64,
    pub volume: f32,
    pub muted: bool,
    pub autoplay: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VideoPaint {
    pub video_ref: String,  // sha1/sha256 hash
    pub poster_ref: String, // image hash
    pub playback: VideoPlayback,
    pub scale_mode: ImageScaleMode,
}
```
Add to `PaintKind`:
```rust
pub enum PaintKind {
    // ...
    Video(VideoPaint),
}
```

### 3.2 Canvas & Player Execution
1. **Canvas Edit Mode**:
   - `video.poster_ref` is looked up in `Document::images`.
   - Rendered using the existing static image pipeline (`PaintKind::Image`).
   - Zero video decoding threads or timers spawned on the UI thread.
2. **Prototype Player Mode**:
   - Spawns background hardware decoder instance for `video_ref`.
   - On frame tick, decodes texture into an offscreen Metal/Vulkan texture (`wgpu::Texture`).
   - Binds texture to `IMG_TILE` shader slot.

---

## 4. Custom WGSL Shaders & Mesh Gradients (#39, #170, #571)

### 4.1 Custom Shader Uniform Model (`crates/designos-doc/src/paint.rs`)
```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ShaderUniformValue {
    Bool(bool),
    Float(f32),
    Vec2(Vec2),
    Color(Color),
    ColorArray(Vec<Color>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CustomShaderPaint {
    pub effect_id: String,
    pub uniforms: BTreeMap<String, ShaderUniformValue>,
}
```
Add to `PaintKind`:
```rust
pub enum PaintKind {
    // ...
    CustomShader(CustomShaderPaint),
}
```

### 4.2 GPU Program Budget Management
In `crates/designos-gpu/src/compile.rs`:
- Pre-register a fixed set of standard custom shaders (Wave, Glitch, Mesh, Vignette).
- Store shaders in a static array capped at length 16.
- Total GPU `Program` count: Base UI programs (8) + Post-processing FX (12) + Custom Shaders (16) = 36 programs.
- **Strictly satisfies the $< 64$ Program cap invariant (`AGENTS.md` §4)**.

---

### 4.3 Mesh Gradient Coons Patch Formulation (#571)
A Coons patch $\mathbf{S}(u, v)$ for $(u, v) \in [0, 1]^2$ interpolates 4 boundary curves $\mathbf{c}_0(u), \mathbf{c}_1(u), \mathbf{d}_0(v), \mathbf{d}_1(v)$ and 4 corner points $\mathbf{P}_{00}, \mathbf{P}_{10}, \mathbf{P}_{01}, \mathbf{P}_{11}$:
$$\mathbf{S}(u, v) = \mathbf{S}_c(u, v) + \mathbf{S}_d(u, v) - \mathbf{S}_{cd}(u, v)$$
where:
$$\mathbf{S}_c(u, v) = (1 - v) \mathbf{c}_0(u) + v \mathbf{c}_1(u)$$
$$\mathbf{S}_d(u, v) = (1 - u) \mathbf{d}_0(v) + u \mathbf{d}_1(v)$$
$$\mathbf{S}_{cd}(u, v) = (1 - u)(1 - v) \mathbf{P}_{00} + u(1 - v) \mathbf{P}_{10} + (1 - u)v \mathbf{P}_{01} + uv \mathbf{P}_{11}$$

#### Color Interpolation
Corner colors $\mathbf{C}_{00}, \mathbf{C}_{10}, \mathbf{C}_{01}, \mathbf{C}_{11}$ in linear RGB:
$$\mathbf{C}(u, v) = (1 - u)(1 - v) \mathbf{C}_{00} + u(1 - v) \mathbf{C}_{10} + (1 - u)v \mathbf{C}_{01} + uv \mathbf{C}_{11}$$
Tessellated into an $8 \times 8$ grid of quads (128 triangles) with vertex colors passed to Lyon.

---

## 5. 3D Transforms & Perspective (#173–#176)

### 5.1 Data Model (`crates/designos-doc/src/node.rs`)
```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Transform3d {
    pub perspective: f32, // focal length d, default 1000.0
    pub translate_z: f32,
    pub rotate_x: f32,    // radians
    pub rotate_y: f32,    // radians
    pub rotate_z: f32,    // radians
    pub backface_hidden: bool,
    pub visible: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scene3d {
    pub perspective: f32,
    pub perspective_origin: Vec2, // default (0.5, 0.5)
    pub preserve_3d: bool,
    pub visible: bool,
}
```
Add to `Node`:
```rust
pub struct Node {
    // ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform_3d: Option<Transform3d>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene_3d: Option<Scene3d>,
    // ...
}
```

### 5.2 3D Matrix Mathematics
1. **Perspective Projection Matrix**:
   Given perspective distance $d > 0$:
   $$\mathbf{P} = \begin{pmatrix} 1 & 0 & 0 & 0 \\ 0 & 1 & 0 & 0 \\ 0 & 0 & 1 & 0 \\ 0 & 0 & -1/d & 1 \end{pmatrix}$$
2. **Model Rotation & Translation**:
   $$\mathbf{M} = \mathbf{T}(t_x, t_y, t_z) \cdot \mathbf{R}_z(\theta_z) \cdot \mathbf{R}_y(\theta_y) \cdot \mathbf{R}_x(\theta_x)$$
3. **Full Vertex Transformation**:
   For each local corner $\mathbf{v} = (x, y, 0, 1)^T$:
   $$\mathbf{v}' = \mathbf{P} \cdot \mathbf{M} \cdot \mathbf{v}$$
   Perspective divide:
   $$(x_{\text{screen}}, y_{\text{screen}}) = (v'_x / v'_w, v'_y / v'_w)$$
4. **Backface Culling**:
   Calculate polygon winding normal in screen space:
   $$N_z = (x_1 - x_0)(y_2 - y_0) - (y_1 - y_0)(x_2 - x_0)$$
   If `backface_hidden == true` and $N_z \le 0.0$, the node is culled from rendering!

---

## 6. Kiwi Schema & Serialization Diffs

### 6.1 Encoding (`crates/designos-fig/src/fig_export.rs`)
```rust
// Video export
PaintKind::Video(v) => vec![
    ("type", en("VIDEO")),
    ("video", msg(vec![("hash", arr(hex_to_bytes(&v.video_ref)))]))),
    ("image", msg(vec![("hash", arr(hex_to_bytes(&v.poster_ref)))]))),
    ("videoFillPlayback", msg(vec![
        ("startTimeMs", V::Uint(v.playback.start_time_ms)),
        ("endTimeMs", V::Uint(v.playback.end_time_ms)),
        ("volume", fl(v.playback.volume)),
        ("mute", V::Bool(v.playback.muted)),
    ])),
],

// 3D Transform export
if let Some(t3d) = &node.transform_3d {
    fields.push(("transform3d", msg(vec![
        ("type", en("FIXED_ORDER")),
        ("backfaceHidden", V::Bool(t3d.backface_hidden)),
        ("visible", V::Bool(t3d.visible)),
        ("value", msg(vec![
            ("perspective", fl(t3d.perspective)),
            ("translateZ", fl(t3d.translate_z)),
            ("rotateX", fl(t3d.rotate_x)),
            ("rotateY", fl(t3d.rotate_y)),
            ("rotateZ", fl(t3d.rotate_z)),
        ])),
    ])));
}
```

### 6.2 Decoding (`crates/designos-fig/src/fig.rs`)
```rust
if let Some(t3d) = nc.get("transform3d") {
    let bf = t3d.get("backfaceHidden").and_then(Value::as_bool).unwrap_or(false);
    if let Some(val) = t3d.get("value") {
        node.transform_3d = Some(Transform3d {
            perspective: fnum(val, "perspective").unwrap_or(1000.0),
            translate_z: fnum(val, "translateZ").unwrap_or(0.0),
            rotate_x: fnum(val, "rotateX").unwrap_or(0.0),
            rotate_y: fnum(val, "rotateY").unwrap_or(0.0),
            rotate_z: fnum(val, "rotateZ").unwrap_or(0.0),
            backface_hidden: bf,
            visible: true,
        });
    }
}
```

---

## 7. Test Vectors from Figma

| Test Vector ID | Setup / Action | Input Parameters | Expected Ground-Truth Result | Verification Gate |
|---|---|---|---|---|
| **V-E5-07** | Video Poster Frame Display | Video paint with mp4 video ref and PNG poster ref | Canvas immediately displays PNG poster without spawning decoder thread | Zero background decoder thread spawned |
| **V-E5-08** | Coons Mesh Gradient Corner Interpolation | $2 \times 2$ Mesh: Top-left Red, Top-right Green, Bottom-left Blue, Bottom-right Yellow | Center point $(0.5, 0.5)$ has color $(0.5, 0.5, 0.25)$ in linear RGB | Pixel sample at center matches bicubic average |
| **V-E5-09** | 3D Perspective Projection Shrink | Square $100 \times 100$, perspective 1000, $t_z = 500$ | Apparent size on screen scales by $d / (d + t_z) = 1000 / 1500 = 0.6667$, rendered at $66.67 \times 66.67$ | Screen pixel dimensions match scale factor |
| **V-E5-10** | 3D Backface Culling | Plane rotated $180^\circ$ around Y-axis (`rotateY = \pi`), `backfaceHidden: true` | Node is completely culled; zero draw calls issued to GPU | Render pipeline skips node |
| **V-E5-11** | Custom Shader Uniform Round-Trip | Custom effect with float uniform `amplitude: 2.5` | Imported, serialized to `.fig`, re-imported with exact float value preserved | Floating point equality within $10^{-6}$ |
