# Epic E5: Effects & Shaders Engine Specs (#18)

Date: 2026-11-12 | Milestone: v0.8.0 | Base: `origin/main`  
Evidence: Clean-room REA Analysis Dossier `ev_6f56bed2b1984cdf5d746450517171fe32bc951557c54a88dce368fd3db6b704` (SHA-256: `e24f67a66fea09b8e13275a2bf75dcab4e9675a22de1b2b6c2b08deab41029b1`).  
Clean Room: Zero copied Figma source code. Verified Kiwi schema field numbers, TypeScript typings, GLSL/WGSL shader blocks, CIELAB transfer equations, and Studio Rust codebase anchors.

---

## Answer first

- **Architecture Invariants (`AGENTS.md` §4)**:
  - **GPU Program Budget**: Offscreen GPU `Program` allocations must strictly respect the `< 64` cap and the $8\,\text{ms}$ frame budget.
  - **Interactive LOD**: All LOD shortcuts (scissor clipping fallbacks, reduced Gaussian blur kernel radii, greeking) remain strictly isolated inside `self.interactive()` so that headless bit-exact test suites (`cargo test -p designos-gpu`) pass with zero divergence against `designos-render`.
  - **Tile and Video Texture Caches**: All image/video frames and custom shader passes use byte-capped LRU texture pools.
- **Image Filters & Color Grading (#521)**:
  - `PaintType::IMAGE = 5` stores 7 color sliders in `PaintFilterMessage`: `exposure = 5`, `contrast = 9`, `vibrance = 8` (`saturation`), `temperature = 7`, `tint = 1`, `highlights = 3`, `shadows = 2`.
  - Evaluation executes in a linear pipeline: sRGB linearization ($C^{2.2}$) $\to$ Rec. 709 Luminance ($Y = 0.2126R + 0.7152G + 0.0722B$) $\to$ Highlight/Shadow soft falloff ($S = \frac{0.75\Delta}{\sqrt{1 + 4\Delta^2}}$) $\to$ D65 CIELAB space for Tint ($a^* \pm 30$) and Temperature ($b^* \pm 30$) with sigmoid contrast $\to$ Exponential exposure tone curve $\to$ Vibrance headroom lerp $\to$ Gamma encoding ($C^{1/2.2}$).
- **Progressive Blur (#37)**:
  - Stored in Kiwi as `blurOpType = 19 : BlurOpType` (`NORMAL = 0`, `PROGRESSIVE = 1`), `startRadius = 21 : Float`, `radius = 4 : Float` (end radius), with normalized UV offsets `startOffset = 20 : Vector` and `endOffset = 28 : Vector`.
  - Radius interpolates continuously in UV space: $R(u, v) = \text{lerp}(r_{\text{start}}, r_{\text{end}}, \text{proj}_{\mathbf{u}}(u, v))$, evaluated in `fx.wgsl` via variable mip-chain sampling.
- **Glass, Noise & Texture Parity (#38)**:
  - Glass: `EffectType::GLASS = 8`, adding `reflectionDistance = 35 : Float` (UI `"splay"`, variable `splayVar = 41`), `refractionRadius = 30 : Float` (UI `"depth"`), `refractionIntensity = 36 : Float`, `specularAngle = 31 : Float`, `specularIntensity = 32 : Float`, `chromaticAberration = 34 : Float` (UI `"dispersion"`), `radius = 4 : Float` (frost).
  - Noise: `EffectType::NOISE = 7` & `GRAIN = 6`, `noiseType = 26` (`MULTITONE = 0`, `MONOTONE = 1`, `DUOTONE = 2`), 2D `noiseSize = 22 : Vector` (independent X/Y scaling).
  - UI Caps: Studio inspector enforces UI `+` creation limits: max 8 drop shadows, 8 inner shadows, 1 layer blur, 1 background/glass blur. Never trims imported files exceeding these caps.
- **Backdrop Alpha & Shadow Behind (#1328)**:
  - `showShadowBehindNode = 8 : Bool` on `Message Effect`.
  - When `false` (default): Node silhouette is subtracted from the shadow raster (`DestinationOut`), preventing translucent layers from becoming murky.
  - When `true`: Shadow is drawn completely behind the node without silhouette clipping.
- **Video Fills, Custom Shaders & 3D (#168–#176, #571)**:
  - Video: `PaintType::VIDEO = 7` with `video = 18` and `videoFillPlayback = 47`. Canvas renders poster image (`Image` hash) instantly; platform decoders (AVFoundation / MediaFoundation) run in prototype player.
  - Custom Shaders & Mesh Gradients: `PaintType::CUSTOM = 10` & `EffectType::CUSTOM = 9` preserve imported `customEffectId = 39`. Studio mesh gradients evaluate bicubic Coons patches tessellated into standard vertex meshes.
  - 3D Transforms: `transform3d = 570` (`FixedOrderTransform3d` with perspective, $t_z$, $r_x, r_y, r_z$) and `scene3d = 572` (`perspectiveOrigin`, `preserve3d`).

---

## 1. What could NOT be verified (read first)

1. **Exact Mipmap Kernel Level for Progressive Blur in WASM**: Figma's private shader uses an internal texture array or mip pyramid to accelerate variable-radius blurs without loop divergence. Studio implements a 4-level Gaussian mip pyramid with smooth linear interpolation.
2. **Video Audio Synchronization Clocks**: The exact microsecond clock synchronization logic between Figma's WASM engine and browser HTML5 video elements for looped timeline offsets is non-reconstructible from static wasm. Studio relies on standard system media clocks.
3. **Proprietary Custom Shader Bytecode**: Figma imports pre-compiled WebGL/WGSL binary blobs for certain internal effects (`customEffectId`). Studio preserves all Kiwi component property assignments and maps known effect IDs to native WGSL shaders in `designos-gpu`.

---

## 2. Status & Roadmap Table

| Slice / Issue | Summary | Size (h) | Recommended Model Tier | Hot-File Lock | Dependencies | Acceptance Gate |
|---|---|---|---|---|---|---|
| **E5-P1.1 (#521)** | Image Fills & 7 Filter Sliders (Exposure, Contrast, Vibrance, Tint, Temp, etc.) | L (14h) | `Gemini 4 Argon` | `fig.rs`, `fig_export.rs` (Slice 1) | None | Exact linear RGB, CIELAB & Rec.709 transfer curves in CPU & GPU |
| **E5-P1.2 (#37)** | Progressive Blur (`blurOpType = 1`, `startRadius`, `endRadius`, UV offsets) | M (10h) | `Gemini 4 Argon` | `fig.rs`, `fig_export.rs` (Slice 2) | None | Linear gradient blur falloff across bounding box in CPU and `fx.wgsl` |
| **E5-P1.3 (#38)** | Glass (`splay`, `dispersion`, `frost`) & Noise (2D `noiseSize`, `DUOTONE`) Parity | M (8h) | `Gemini 3.8 Flash` | `fig.rs`, `fig_export.rs` (Slice 3) | None | Complete `.fig` round-trip; splay displacement; UI count caps |
| **E5-P1.4 (#1328)** | Backdrop Alpha & `showShadowBehindNode` Silhouette Cutout Parity | S (4h) | `Gemini 3.8 Flash` | `fig.rs`, `fig_export.rs` (Slice 4) | None | Translucent fill shadow cutout; shadow-behind flag toggling |
| **E5-P2.1 (#168/#169)** | Video Fills (`PaintType::VIDEO = 7`, Playback Settings, Poster Fallback) | M (8h) | `Gemini 3.8 Flash` | `fig.rs`, `fig_export.rs` (Slice 5) | None | Poster frame canvas display; playback parameter round-trip |
| **E5-P2.2 (#39/#170)** | Custom WGSL Shader Bridge (`PaintType::CUSTOM`, `EffectType::CUSTOM`) | L (14h) | `Gemini 4 Argon` | `fig.rs`, `fig_export.rs` (Slice 6) | None | Uniform block mapping; $< 64$ Program cap preservation |
| **E5-P2.3 (#571)** | Mesh Gradients (Bicubic Coons Patch Tessellation) | L (14h) | `Gemini 4 Argon` | `fig.rs`, `fig_export.rs` (Slice 7) | None | Smooth color transitions across control point grid; SVG export |
| **E5-P2.4 (#173–#176)** | 3D Transforms (`transform3d = 570`, `scene3d = 572`, FixedOrder Math) | L (12h) | `Gemini 4 Argon` | `fig.rs`, `fig_export.rs` (Slice 8) | None | 4x4 perspective matrix transformation; backface culling |

---

## 3. Order and Hot-File Queue Rules

### Serialization Hot-File Queue Rule (`AGENTS.md`)
The files `crates/designos-fig/src/fig.rs`, `crates/designos-fig/src/fig_export.rs`, `crates/designos-fig/src/kiwi.rs`, and `crates/designos-fig/src/rest_import.rs` are high-contention files. **Only one sub-agent may edit these files at any given time**.

The serialized slice queue for Epic E5 is:
1. **Slice 1 (E5-P1.1)**: `PaintType::IMAGE` 7-slider filter fields on `PaintFilterMessage`.
2. **Slice 2 (E5-P1.2)**: Progressive blur fields on `Message Effect` (`blurOpType = 19`, `startRadius = 21`, `startOffset = 20`, `endOffset = 28`).
3. **Slice 3 (E5-P1.3)**: Glass `reflectionDistance = 35` (`splay`), `bevelSize = 33`, 2D `noiseSize = 22`.
4. **Slice 4 (E5-P1.4)**: `showShadowBehindNode = 8 : Bool`.
5. **Slice 5 (E5-P2.1)**: `PaintType::VIDEO = 7`, `video = 18`, `videoFillPlayback = 47`.
6. **Slice 6 (E5-P2.2)**: `PaintType::CUSTOM = 10`, `EffectType::CUSTOM = 9`.
7. **Slice 7 (E5-P2.3)**: Mesh gradient uniforms in `ComponentPropAssignment`.
8. **Slice 8 (E5-P2.4)**: `transform3d = 570`, `scene3d = 572`.

### Parallelizable Streams (Zero Hot-File Lock)
- **Shader Pipeline (`fx.wgsl`)**: Color grading fragment shader, progressive blur mip-chain sampling, splay refraction shader.
- **CPU Render Math (`designos-render/src/effects.rs`)**: Software CIELAB color grading, separable progressive Gaussian convolution, Coons patch subdivision.
- **Transform Pipeline (`designos-gpu/src/camera.rs`)**: 3D perspective matrix math ($4 \times 4$ projection).

---

## 4. Rollback and Migration Strategy

- **Additive Serde Enums**: Any new effect enum variant (`Effect::ProgressiveBlur`, `PaintKind::Video`, `PaintKind::Shader`) must deserialize gracefully. Unknown types degrade to harmless no-op rendering without corrupting the document.
- **Reversion**: Every PR is squashed to a single commit and can be reverted with `git revert <commit>`.
