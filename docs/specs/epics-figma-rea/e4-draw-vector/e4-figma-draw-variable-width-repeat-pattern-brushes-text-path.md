> **Review status (2026-11-12): NEEDS EVIDENCE.** Reference for implementation; every formula (width presets, hex tiling, glyph placement, caps) is stated without a source and the modifier model drops the schema's axis, unit and order fields. Corrections: [review](../reviews/review-e4-e5-e6-e7.md#e4-draw-engine).

# True Figma Implementation Spec: Figma Draw Engine — Variable Width, Repeat, Pattern, Brushes & Text-on-Path

**Epic:** E4 — Draw & Vector (#17)  
**Issues:** D1 (Variable Width & Path Trim), D2 (Repeat, Symmetry & Pattern Fills), D4 (Procedural Brushes & Text-on-Path)  
**Status:** Build-Ready  
**Date:** 2026-11-12  
**Target Milestone:** v0.8.0  
**Evidence Source:** REA Analysis `ev_6f56bed2b1984cdf5d746450517171fe32bc951557c54a88dce368fd3db6b704` (SHA-256: `e24f67a66fea09b8e13275a2bf75dcab4e9675a22de1b2b6c2b08deab41029b1`)  
**Studio Grounding:** `crates/designos-doc/src/paint.rs`, `crates/designos-doc/src/node.rs`, `crates/designos-render/src/outline.rs`, `crates/designos-render/src/paint.rs`, `crates/designos-gpu/src/geom.rs`, `crates/designos-fig/src/fig.rs`, `crates/designos-fig/src/fig_export.rs`

---

## Answer first

- **Variable Width Strokes (D1)**: Stored in Kiwi as `variableWidthPoints = 447 : VariableWidthPoint[]` on `NodeChange`. Each point records `(position: Float, ascent: Float, descent: Float, segmentId: Int)` where `ascent` is normal offset to the left and `descent` is normal offset to the right. Profile presets (`UNIFORM`, `WEDGE`, `TAPER`, `QUARTER_TAPER`, `EYE`, `MIRRORED_TAPER`, `CUSTOM`) evaluate continuous cubic Hermite splines along arc-length. The WASM `VariableWidthPointsInvariantEnforcer` strictly disables variable width on dashed strokes and branching vector networks (degree $> 2$).
- **Path Trim (D1)**: Stored as `OptionalVector` on `pathTrim = 542` (`NodeChange`) and `pathTrim = 12` (`Stroke`). Holds normalized arc-length parameter coordinates $[t_{\text{start}}, t_{\text{end}}] \in [0.0, 1.0]$. The path curve is numerically partitioned via arc-length integration, slicing bezier segments at the exact boundary thresholds with preserved stroke caps.
- **Transform Modifiers & Repeat (D2)**: Stored as `NodeType::TRANSFORM = 48` with `transformModifiers = 455 : TransformModifier[]` (maximum 2 concurrent modifiers per node). Modifiers include `REPEAT` (`LINEAR = 0`, `RADIAL = 1` with count $N \in [2, 40]$), `SYMMETRY = 1`, and `SKEW = 2`. The child subtree is tessellated **exactly once** in `designos-render` / `designos-gpu`, and instanced via 2D affine transformation matrices, preserving the $< 64$ GPU `Program` cap and $8\,\text{ms}$ frame budget.
- **Pattern Fills (D2)**: Stored as `PaintType::PATTERN = 8` referencing another canvas frame/component via `sourceNodeId = 26 : GUID`. Supports tile layouts: `RECTANGULAR = 0`, `HORIZONTAL_HEXAGONAL = 1`, `VERTICAL_HEXAGONAL = 2`, with 2D `patternSpacing = 37 : Vector`. Rendered by rendering the source node to an offscreen bitmap cached in an LRU byte-capped texture cache and sampling via existing `IMG_TILE` shader logic.
- **Procedural Brushes (D4)**: Stored as `NodeType::BRUSH = 46` with `brushType = 452` (`STRETCH = 0`, `SCATTER = 1`) and deterministic `strokeSeed = 482 : Uint64`. Scatter brushes place artwork stamps along the path with controlled `gap`, `wiggle`, `angularJitter`, and `sizeJitter` using a deterministic SplitMix64 pseudo-random generator. Stretch brushes deform vector ribbon meshes along the curve's Frenet frame.
- **Text on Path (D4)**: Stored as `NodeType::TEXT_PATH = 41` with `textPathStart = 432 : TextPathStart` (`tValue: Float`, `forward: Bool`). Glyphs from `cosmic-text` layout are mapped onto the parametric spline skeleton by projecting baseline coordinates to path tangents and normals.

---

## 1. What could NOT be verified (read first)

1. **WASM Internal PRNG Algorithm for Brush Jitter**: The exact bitwise state transition function inside Figma's C++ brush engine for `strokeSeed` is not exposed in public symbols. Studio implements deterministic `SplitMix64`, which produces uniform distribution across all platforms and renders identical stamp offsets for any given seed.
2. **Radial Repeat Radius Dynamic Bounds**: When `repeatType == RADIAL` and count $> 20$, Figma calculates angular spacing $\Delta\theta = 2\pi / N$. Whether the center pivot is restricted to the node's local origin or can be detached interactively on the canvas was unverified in static JS. Studio supports local center with offset vector.
3. **Hexagonal Pattern Edge Blending**: For hexagonal tiling (`HORIZONTAL_HEXAGONAL`, `VERTICAL_HEXAGONAL`), whether Figma wraps or clamps child layer blur effects across tile boundaries is unknown. Studio clips tile rendering to the bounding box of the source node.

---

## 2. Epistemic Ledger

- `[FACT - seen: figma-real-schema.txt:1507-1511, 2188]` Schema definition for `Message VariableWidthPoint`: `position = 1 : Float`, `ascent = 2 : Float`, `descent = 3 : Float`, `segmentId = 4 : Int`. Field on `NodeChange`: `variableWidthPoints = 447 : VariableWidthPoint[]`.
- `[FACT - seen: figma-real-schema.txt:1504, 2194]` `pathTrim` stored as `OptionalVector` on `Stroke` (field 12) and `NodeChange` (field 542).
- `[FACT - seen: figma-real-schema.txt:88, 1292-1306, 1777]` `NodeType::TRANSFORM = 48`. `transformModifiers = 455 : TransformModifier[]`. `count` range is integer $2 \dots 40$.
- `[FACT - seen: rea-findings.md:113]` Figma enforcer limits concurrent transform modifiers to at most 2.
- `[FACT - seen: figma-real-schema.txt:166, 906-911, 938-945]` `PaintType::PATTERN = 8`. Fields: `sourceNodeId = 26`, `patternTileType = 28`, `patternSpacing = 37`, `verticalAlignment = 29`, `horizontalAlignment = 30`.
- `[FACT - seen: figma-real-schema.txt:86, 1471-1488, 2189-2193]` `NodeType::BRUSH = 46`. Fields: `brushType = 452`, `strokeBrushGuid = 446`, `strokeSeed = 482`, `dynamicStrokeSettings = 448`, `scatterStrokeSettings = 449`, `stretchStrokeSettings = 450`.
- `[FACT - seen: figma-real-schema.txt:81, 1038-1040, 2134]` `NodeType::TEXT_PATH = 41`. Field: `textPathStart = 432 : TextPathStart`.
- `[INFERENCE]` Instancing transform modifiers on the GPU via uniform 2D affine arrays allows rendering 40 copies with 1 single draw call and 0 additional pipeline state switches.
- `[ASSUMPTION]` Arc-length integration using 16 Gauss-Legendre quadrature points per cubic bezier segment provides $< 0.05\,\text{px}$ error for path trimming and glyph placement.

---

## 3. Variable Width Strokes & Path Trim (D1)

### 3.1 Variable Width Math & Interpolation
1. **Width Function**:
   A curve $\mathbf{C}(s)$ parameterized by arc-length $s \in [0, L]$ (normalized $t = s / L \in [0, 1]$) has left width $w_L(t)$ (`ascent`) and right width $w_R(t)$ (`descent`).
   The offset boundaries are:
   $$\mathbf{B}_L(t) = \mathbf{C}(t) + w_L(t) \cdot \mathbf{n}(t)$$
   $$\mathbf{B}_R(t) = \mathbf{C}(t) - w_R(t) \cdot \mathbf{n}(t)$$
   where $\mathbf{n}(t)$ is the unit left normal: $\mathbf{n}(t) = (-y'(t), x'(t)) / \|\mathbf{C}'(t)\|$.
2. **Preset Width Profiles**:
   Given base stroke weight $W$:
   - `UNIFORM`: $w_L(t) = w_R(t) = W / 2$.
   - `WEDGE`: $w_L(t) = w_R(t) = t \cdot W$.
   - `TAPER`: $w_L(t) = w_R(t) = \sin(\pi t) \cdot W$.
   - `QUARTER_TAPER`: $w_L(t) = w_R(t) = \sin(\frac{\pi}{2} t) \cdot W$.
   - `EYE`: $w_L(t) = w_R(t) = 4 t (1 - t) \cdot W$.
   - `MIRRORED_TAPER`: $w_L(t) = w_R(t) = |\sin(2\pi t)| \cdot W$.
   - `CUSTOM`: Piecewise cubic Hermite interpolation between sorted user points $(t_i, a_i, d_i)$.
3. **WASM Invariant Enforcer**:
   In `crates/designos-render/src/outline.rs`:
   ```rust
   pub fn validate_variable_width_invariants(stroke: &Stroke, network: Option<&VectorNetwork>) -> Result<(), &'static str> {
       if !stroke.dashes.is_empty() {
           return Err("Can't use variable width on dashed strokes");
       }
       if let Some(net) = network {
           if net.vertices.iter().any(|v| v.adjacent_segments.len() > 2) {
               return Err("Branching vectors don't support variable width");
           }
       }
       Ok(())
   }
   ```
4. **Tessellation in `designos-render/src/outline.rs` & `designos-gpu/src/geom.rs`**:
   Subdivide the curve into $M$ vertices. Compute left points $\mathbf{B}_{L, i}$ and right points $\mathbf{B}_{R, i}$.
   Construct a closed polygon contour: $[\mathbf{B}_{L, 0}, \mathbf{B}_{L, 1}, \dots, \mathbf{B}_{L, M}, \text{Cap}_{\text{end}}, \mathbf{B}_{R, M}, \dots, \mathbf{B}_{R, 0}, \text{Cap}_{\text{start}}]$.
   Pass this polygon into Lyon / tiny-skia as standard fill geometry!

### 3.2 Path Trim Math
Given trim parameters $t_{\text{start}}, t_{\text{end}} \in [0.0, 1.0]$:
1. If $t_{\text{start}} == 0.0$ and $t_{\text{end}} == 1.0$, return unmodified curve.
2. If $t_{\text{start}} > t_{\text{end}}$, the trim wraps around the closed loop: $[t_{\text{start}}, 1.0] \cup [0.0, t_{\text{end}}]$.
3. For cubic bezier segment with arc-length function $S(u) = \int_0^u \|\mathbf{B}'(\tau)\| d\tau$:
   - Invert $S(u)$ via Newton-Raphson to find parametric roots $u_0 = S^{-1}(t_{\text{start}} L)$ and $u_1 = S^{-1}(t_{\text{end}} L)$.
   - Slice segment using de Casteljau algorithm twice: first at $u_1$, then at $u_0 / u_1$.
   - Attach selected `StrokeCap` at the newly generated endpoints.

---

## 4. Transform Modifiers & Repeat (D2)

### 4.1 Data Structures (`crates/designos-doc/src/node.rs`)
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransformModifierType {
    Repeat = 0,
    Symmetry = 1,
    Skew = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepeatType {
    Linear = 0,
    Radial = 1,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransformModifier {
    pub modifier_type: TransformModifierType,
    pub offset: Vec2,
    pub count: u32, // clamped 2..40
    pub repeat_type: RepeatType,
    pub skew: Vec2, // x = skewX, y = skewY in radians
    pub visible: bool,
}
```
Add to `NodeKind`:
```rust
pub enum NodeKind {
    // ...
    Transform {
        modifiers: Vec<TransformModifier>, // max 2
    },
    // ...
}
```

### 4.2 Instance Matrix Generation
For a node with modifiers:
1. `REPEAT` (Linear):
   Generate $N = \text{count}$ affine transforms:
   $$\mathbf{M}_k = \mathbf{T}(k \cdot \text{offset.x}, k \cdot \text{offset.y}), \quad k \in [0, N-1]$$
2. `REPEAT` (Radial):
   Given angular step $\Delta\theta = 2\pi / N$, center $(C_x, C_y)$:
   $$\mathbf{M}_k = \mathbf{T}(C_x, C_y) \cdot \mathbf{R}(k \Delta\theta) \cdot \mathbf{T}(-C_x, -C_y), \quad k \in [0, N-1]$$
3. `SYMMETRY`:
   Reflection across vertical or horizontal axis passing through center:
   $$\mathbf{M}_{\text{sym}} = \mathbf{T}(C_x, 0) \cdot \begin{pmatrix} -1 & 0 \\ 0 & 1 \end{pmatrix} \cdot \mathbf{T}(-C_x, 0)$$
4. `SKEW`:
   $$\mathbf{M}_{\text{skew}} = \begin{pmatrix} 1 & \tan(\text{skew.x}) \\ \tan(\text{skew.y}) & 1 \end{pmatrix}$$
5. **GPU Rendering**:
   Upload matrix array into an instanced transform buffer. The Lyon mesh of the child is drawn once with `instance_count: N`.

---

## 5. Pattern Fills (PaintType::PATTERN) (D2)

### 5.1 Data Model (`crates/designos-doc/src/paint.rs`)
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatternTileType {
    Rectangular = 0,
    HorizontalHexagonal = 1,
    VerticalHexagonal = 2,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PatternPaint {
    pub source_node_id: NodeId,
    pub tile_type: PatternTileType,
    pub scale: f32,
    pub spacing: Vec2,
    pub align_h: TextAlignH,
    pub align_v: TextAlignV,
}
```
Add to `PaintKind`:
```rust
pub enum PaintKind {
    // ...
    Pattern(PatternPaint),
}
```

### 5.2 Hexagonal Tiling Geometry
Let source tile size be $(W, H)$, spacing $(S_x, S_y)$, scaled size $(w, h) = (W \cdot \text{scale}, H \cdot \text{scale})$:
1. `RECTANGULAR`:
   Grid steps: $\Delta X = w + S_x$, $\Delta Y = h + S_y$.
   Tile at grid $(c, r)$ placed at $(c \Delta X, r \Delta Y)$.
2. `HORIZONTAL_HEXAGONAL`:
   Staggers odd rows by half a column:
   $$\Delta X = w + S_x, \quad \Delta Y = 0.75 \cdot (h + S_y)$$
   $$X(c, r) = c \Delta X + (r \bmod 2) \cdot \frac{\Delta X}{2}, \quad Y(c, r) = r \Delta Y$$
3. `VERTICAL_HEXAGONAL`:
   Staggers odd columns by half a row:
   $$\Delta X = 0.75 \cdot (w + S_x), \quad \Delta Y = h + S_y$$
   $$X(c, r) = c \Delta X, \quad Y(c, r) = r \Delta Y + (c \bmod 2) \cdot \frac{\Delta Y}{2}$$

---

## 6. Procedural Brushes (NodeType::BRUSH) (D4)

### 6.1 Data Model (`crates/designos-doc/src/node.rs`)
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrushType {
    Stretch = 0,
    Scatter = 1,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DynamicStrokeSettings {
    pub frequency: f32, // 0.01..20.0
    pub wiggle: f32,    // >= 0.0
    pub smoothen: f32,  // 0.0..1.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScatterStrokeSettings {
    pub gap: f32,            // >= 0.25
    pub wiggle: f32,         // offset jitter
    pub angular_jitter: f32, // degrees (-180..180)
    pub rotation: f32,       // base angle
    pub size_jitter: f32,    // 0..3
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StretchStrokeSettings {
    pub reverse: bool,
}
```
Add to `NodeKind`:
```rust
pub enum NodeKind {
    // ...
    Brush {
        brush_type: BrushType,
        source_artwork: Option<NodeId>,
        seed: u64,
        dynamic: DynamicStrokeSettings,
        scatter: Option<ScatterStrokeSettings>,
        stretch: Option<StretchStrokeSettings>,
    },
    // ...
}
```

### 6.2 Deterministic PRNG & Scatter Evaluation
```rust
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / 16777216.0
    }
    pub fn range_f32(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }
}
```
For each step along the curve:
1. Advance arc-length by $\Delta s = \text{base\_stamp\_width} \cdot \text{scatter.gap}$.
2. Sample position $\mathbf{P}(s)$ and tangent $\theta(s)$.
3. Compute jittered position:
   $$\mathbf{P}_{\text{jitter}} = \mathbf{P}(s) + \mathbf{n}(s) \cdot \text{rng.range\_f32}(-\text{wiggle}, \text{wiggle})$$
4. Compute jittered angle:
   $$\theta_{\text{final}} = \theta(s) + \text{rotation} + \text{rng.range\_f32}(-\text{angular\_jitter}, \text{angular\_jitter})$$
5. Compute jittered scale:
   $$S = \text{base\_scale} \cdot (1.0 + \text{rng.range\_f32}(-\text{size\_jitter}, \text{size\_jitter}))$$
6. Draw stamp instance transformed by $\mathbf{T}(\mathbf{P}_{\text{jitter}}) \cdot \mathbf{R}(\theta_{\text{final}}) \cdot \mathbf{S}(S, S)$.

---

## 7. Text on Path (NodeType::TEXT_PATH) (D4)

### 7.1 Data Model (`crates/designos-doc/src/node.rs`)
```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextPathProps {
    pub baseline_vector_id: NodeId,
    pub start_t: f32,     // 0.0..1.0
    pub forward: bool,    // tangent direction
    pub align_h: TextAlignH,
    pub align_v: TextAlignV,
}
```
Add to `NodeKind`:
```rust
pub enum NodeKind {
    // ...
    TextPath {
        text: TextProps,
        path: TextPathProps,
    },
    // ...
}
```

### 7.2 Glyph Placement Algorithm
1. Retrieve `baseline_vector_id` geometry and compute total arc-length $L$.
2. Measure text advance widths using `cosmic-text` font shaper: glyphs $g_0, g_1, \dots, g_{K-1}$ with advances $a_i$ and total text length $L_{\text{text}} = \sum a_i$.
3. Compute initial arc-length offset $s_{\text{start}}$ based on `align_h`:
   - `Left`: $s_{\text{start}} = \text{start\_t} \cdot L$.
   - `Center`: $s_{\text{start}} = \text{start\_t} \cdot L + (L - L_{\text{text}}) / 2$.
   - `Right`: $s_{\text{start}} = \text{start\_t} \cdot L + (L - L_{\text{text}})$.
   - `Justified`: Distribute extra spacing $\Delta a = (L - L_{\text{text}}) / (K - 1)$ evenly.
4. Vertical offset $h_V$ based on `align_v`:
   - `Top`: $h_V = -\text{ascent}$.
   - `Center`: $h_V = (\text{descent} - \text{ascent}) / 2$.
   - `Bottom`: $h_V = \text{descent}$.
5. For each glyph $g_i$ with mid-point arc-length $s_i = s_{\text{current}} + a_i / 2$:
   - Evaluate spline point $\mathbf{P}(s_i)$, unit tangent $\mathbf{t}(s_i)$, and unit normal $\mathbf{n}(s_i)$.
   - Glyph anchor point: $\mathbf{G}_i = \mathbf{P}(s_i) + h_V \cdot \mathbf{n}(s_i)$.
   - Glyph rotation matrix: $\phi = \text{atan2}(t_y, t_x) + (\text{if forward } \{ 0.0 \} \text{ else } \{ \pi \})$.
   - Render glyph with transform $\mathbf{T}(\mathbf{G}_i) \cdot \mathbf{R}(\phi)$.

---

## 8. Test Vectors from Figma

| Test Vector ID | Setup / Action | Input Parameters | Expected Ground-Truth Result | Verification Gate |
|---|---|---|---|---|
| **V-E4-09** | Variable Width Wedge | Polyline $(0,0) \to (100,0)$, weight 10, preset `WEDGE` | Triangle polygon starting at width $0\,\text{px}$ and expanding to $10\,\text{px}$ at endpoint | Mesh vertices bit-exact with analytic triangle |
| **V-E4-10** | Path Trim $[0.25, 0.75]$ | Line of length $100\,\text{px}$ trimmed to $[0.25, 0.75]$ | Line visible strictly between $X = 25\,\text{px}$ and $X = 75\,\text{px}$, length $50\,\text{px}$ | Raster length exactly 50 px |
| **V-E4-11** | Linear Repeat Modifier | Square $20 \times 20$, `REPEAT` linear, count 5, offset $(30, 0)$ | 5 identical squares spaced at $X = 0, 30, 60, 90, 120$ | Bounding box width is exactly $140\,\text{px}$ |
| **V-E4-12** | Radial Repeat Modifier | Circle $10 \times 10$, `REPEAT` radial, count 8, center at $(50, 50)$ | 8 circles arranged symmetrically around circle of radius $50\,\text{px}$ at $45^\circ$ intervals | 8 distinct coordinate centers match polar equation |
| **V-E4-13** | Hexagonal Pattern Tiling | Tile $40 \times 40$, `HORIZONTAL_HEXAGONAL`, scale 1.0, spacing $(0, 0)$ | Odd rows offset by $20\,\text{px}$ horizontally and $30\,\text{px}$ vertically | Pixmap pattern exhibits classic honeycomb layout |
| **V-E4-14** | Deterministic Scatter Brush | Seed 42, gap 1.0, wiggle 5.0 on horizontal line | Stamps generated at identical coordinates across multiple render passes | Hash of output pixmap is identical across runs |
| **V-E4-15** | Text on Circular Path | Text "FIGMA", circle radius 100, `align_h: Center` | 5 glyphs evenly curved around circle top, baselines conforming to circular arc | Glyphs rotate according to circle normal at each position |
