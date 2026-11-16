> **Review status (2026-11-12): READY AFTER FIXES.** Reference for implementation; inside and outside strokes, mask types and offset path are already on main, and the GPU stencil claim is false (the renderer has no stencil; each inside or outside stroke clip opens a mask Program). Corrections: [review](../reviews/review-e4-e5-e6-e7.md#e4-basic-shapes).

# True Figma Implementation Spec: Basic Shapes, Multi-Strokes & Vector Tools

**Epic:** E4 — Draw & Vector (#17)  
**Issues:** #952, #953, #196, #197, #1506, #193, #194, #499  
**Status:** Build-Ready  
**Date:** 2026-11-12  
**Target Milestone:** v0.8.0  
**Evidence Source:** REA Analysis `ev_6f56bed2b1984cdf5d746450517171fe32bc951557c54a88dce368fd3db6b704` (SHA-256: `e24f67a66fea09b8e13275a2bf75dcab4e9675a22de1b2b6c2b08deab41029b1`)  
**Studio Grounding:** `crates/designos-doc/src/paint.rs`, `crates/designos-doc/src/node.rs`, `crates/designos-render/src/outline.rs`, `crates/designos-render/src/mask.rs`, `crates/designos-fig/src/fig.rs`, `crates/designos-fig/src/fig_export.rs`

---

## Answer first

- **Inside/Outside Stencil Parity (#952)**: Figma generates stroke geometry centered on the path, expands the stroke width to $2w$, and applies a 1-bit or 8-bit clip mask derived from the shape's fill region:
  - `INSIDE`: Kept only where the shape fill covers the plane (`DestinationIn` / stencil test).
  - `OUTSIDE`: Kept only where the shape fill does *not* cover the plane (`DestinationOut` / outside stencil test).
  - `CENTER`: No clipping; standard width $w$ centered along the path normal.
  - In `designos-render/src/outline.rs`, this is synthesized into `StrokePlan { outline, clip }` so both CPU (`tiny-skia`) and GPU (`lyon` in `designos-gpu/src/geom.rs`) execute the identical stencil clip pipeline without creating any new GPU `Program`s.
- **Shape Parity & Resize Math (#953)**:
  - `ELLIPSE`: Stored as `ArcData` (Kiwi `arcData = 195 : ArcData` on `NodeChange`) with fields `startingAngle: Float`, `endingAngle: Float`, `innerRadius: Float` ($[0.0, 1.0]$). A complete ellipse has `startingAngle = 0.0`, `endingAngle = 2\pi`, `innerRadius = 0.0`. Partial arcs and donut holes render via parametric cubic bezier approximations.
  - `STAR`: Stored as `starInnerScale = 24 : Float` on `NodeChange` (ratio of inner star radius to outer vertex radius; default 0.382 for a standard 5-point star) and `pointCount = 23 : Uint`.
  - `LINE`: Stored with `size = { x: length, y: 0.0 }`. Angle is strictly stored in `relativeTransform` (2x3 affine matrix). Canvas resizing updates `size.x` and the rotation matrix; `strokeWeight` is completely decoupled.
- **Multi-Stroke Architecture (#196, #197)**:
  - Stored in Kiwi as `strokeData = 571 : StrokeData` (`strokes = 1 : Stroke[]`, `version = 2 : Uint`).
  - `StrokeAlign` enum: `CENTER = 0`, `INSIDE = 1`, `OUTSIDE = 2`, `OFFSET = 3`.
  - When `align == StrokeAlign::OFFSET`, the path is shifted along its vertex normals by `strokeOffset = 13 : Float` before stroke tessellation.
  - Compositing order: All fills render first (index 0 to $N-1$), followed by all strokes rendered sequentially in ascending index order (index 0 at bottom, index $M-1$ on top).
  - Independent per-side border weights (`borderTopWeight = 295`, `borderRightWeight = 298`, `borderBottomWeight = 296`, `borderLeftWeight = 297`, `borderStrokeWeightsIndependent = 299 : Bool`).
- **Vector Mask Types (#1506)**:
  - Stored in Kiwi as `mask = 16 : Bool`, `maskIsOutline = 18 : Bool`, `maskType = 317 : MaskType` (`ALPHA = 0`, `OUTLINE = 1`, `LUMINANCE = 2`).
  - `OUTLINE` (plugin API `'VECTOR'`): Uses the closed geometric fill silhouette of the shape. Stroke paints and stroke outlines are completely stripped (`n.stroke.paints.clear()`) unless the mask node has an empty fill, in which case the stroke outline alone acts as the solid cutting stencil.
  - `LUMINANCE`: Evaluates grey luminance per pixel: $Y = 0.2126R + 0.7152G + 0.0722B$, applying the resulting luminance as the mask alpha.
- **Interactive Vector Tools (#193, #194, #499)**:
  - **Eraser (#193)**: Circular eraser stamp radius $R$ subtracted from path contours using `i_overlay` boolean difference operations (`Overlay::difference`), inserting new endpoints and splitting curves.
  - **Scissors/Cut (#194)**: Splits cubic bezier segments at projected parametric coordinate $t \in [0.0, 1.0]$ using de Casteljau subdivision, breaking closed loops into open paths or splitting multi-segment networks.
  - **Offset Path (#499)**: Polylines and bezier paths are buffered using `i_overlay` contour dilation/erosion by distance $D \in [-100.0, 100.0]$ with `Round` or `Square` joins. Variable-width strokes are automatically outlined to standard vector fills before offset, triggering user visual bell toast `"Variable width stroke outlined for offset"`.

---

## 1. What could NOT be verified (read first)

1. **WASM Internal Numerical Tolerance for Path Clipping**: The precise floating-point epsilon used in Figma's C++ WASM geometry engine (`FGPathTrim.cpp`, `OffsetPathBehavior`) for bezier flattening is unexported. Studio specifies a chordal error threshold of $\epsilon = 0.25\,\text{px}$.
2. **REST API Exposure of `StrokeAlign::OFFSET`**: Figma's REST API currently reports `strokeAlign` as `'INSIDE' | 'OUTSIDE' | 'CENTER'`. The `OFFSET` alignment is serialized in binary Kiwi files (`fig-kiwi`) but has not yet been documented in the public REST schema. Studio imports and exports `OFFSET` via Kiwi and maps to `CENTER` in public JSON exports.
3. **Line Cap Behavior on Zero-Length Path Segments**: When an open path has zero length and `StrokeCap::Round` is selected, Figma renders a circle of diameter $w$. Studio verifies this matches `tiny-skia`'s native line cap implementation.

---

## 2. Epistemic Ledger

- `[FACT - seen: figma-real-schema.txt:215-219, 1498, 1767]` `StrokeAlign` enum has 4 variants: `CENTER = 0`, `INSIDE = 1`, `OUTSIDE = 2`, `OFFSET = 3`. `strokeOffset = 13 : Float` on `Message Stroke` and `strokeOffset = 554 : Float` on `NodeChange`.
- `[FACT - seen: figma-real-schema.txt:1523-1526, 1762]` `ArcData` contains `startingAngle = 1 : Float`, `endingAngle = 2 : Float`, `innerRadius = 3 : Float`. Field ID on `NodeChange` is `arcData = 195 : ArcData`.
- `[FACT - seen: figma-real-schema.txt:1762]` `STAR` node stores `starInnerScale = 24 : Float` and `pointCount = 23 : Uint` on `NodeChange`.
- `[FACT - seen: figma-real-schema.txt:1489-1506, 1779]` Multi-stroke is stored in `strokeData = 571 : StrokeData`.
- `[FACT - seen: figma-real-schema.txt:1692-1695, 1754-1758]` Mask properties: `mask = 16 : Bool`, `maskIsOutline = 18 : Bool`, `maskType = 317 : MaskType` (`ALPHA = 0`, `OUTLINE = 1`, `LUMINANCE = 2`).
- `[FACT - seen: figma_app en.json, wasm-strings.txt]` Vector tools action IDs: `fullscreen_actions.set-tool-eraser`, `fullscreen_actions.set-tool-cut`, `fullscreen_actions.set-tool-offset-path`, `fullscreen_actions.outlined-vws-for-offset-visual-bell`.
- `[INFERENCE]` When `maskType == OUTLINE`, stroke geometry is stripped because vector mask semantics dictate that only the closed path silhouette serves as a clipping stencil.
- `[ASSUMPTION]` De Casteljau subdivision at chordal tolerance $\epsilon = 0.25\,\text{px}$ produces visually indistinguishable curves across canvas zoom scales $10\% \dots 3200\%$.

---

## 3. Shape & Stroke Parity (#952, #953)

### 3.1 Inside & Outside Stencil Clipping Math (#952)
For any closed shape path $\mathcal{P}_{\text{fill}}$ with stroke weight $w$:
1. A centerline stroke path is expanded with doubled weight $w_{\text{expanded}} = 2.0 \cdot w$.
2. The stroke geometry is clipped against the raster or vector representation of $\mathcal{P}_{\text{fill}}$:
   $$\mathcal{S}_{\text{inside}} = \text{Stroke}\left(\mathcal{P}_{\text{fill}}, 2w\right) \cap \mathcal{P}_{\text{fill}}$$
   $$\mathcal{S}_{\text{outside}} = \text{Stroke}\left(\mathcal{P}_{\text{fill}}, 2w\right) \setminus \mathcal{P}_{\text{fill}}$$
3. **CPU Pipeline (`tiny-skia`)**:
   In `crates/designos-render/src/outline.rs`:
   ```rust
   let (width, clip) = match align {
       d::StrokeAlign::Center => (w, StrokeClip::None),
       d::StrokeAlign::Inside => (w * 2.0, StrokeClip::Inside),
       d::StrokeAlign::Outside => (w * 2.0, StrokeClip::Outside),
       d::StrokeAlign::Offset => (w, StrokeClip::None), // normal shifted
   };
   ```
   When rendering `StrokeClip::Inside`:
   - Draw shape fill into a mask pixmap or clip buffer.
   - Draw doubled stroke with blend mode `SourceIn` (or stencil mask test `ref == 1`).
   When rendering `StrokeClip::Outside`:
   - Draw doubled stroke with blend mode `SourceOut` (or stencil mask test `ref == 0`).
4. **GPU Pipeline (`designos-gpu/src/geom.rs`)**:
   - Lyon generates vertex mesh for the doubled stroke.
   - The shape fill is rendered into the hardware stencil buffer (stencil ref `1`).
   - `INSIDE` stroke pipeline sets `stencil_state: StencilFaceState { compare: CompareFunction::Equal, ... }` with ref `1`.
   - `OUTSIDE` stroke pipeline sets `stencil_state: StencilFaceState { compare: CompareFunction::NotEqual, ... }` with ref `1`.
   - This executes bit-exact clipping **with zero new offscreen GPU `Program`s allocated**, preserving the $< 64$ cap!

### 3.2 Ellipse ArcData & Star Geometry Math (#953)
1. **Ellipse Arc Representation**:
   Given bounding box dimensions $(W, H)$, center $(C_x, C_y) = (W/2, H/2)$, radii $(R_x, R_y) = (W/2, H/2)$:
   - Outer arc spans angles $\theta \in [\theta_{\text{start}}, \theta_{\text{end}}]$.
   - Inner arc (if $r_{\text{inner}} > 0$) spans angles $\theta \in [\theta_{\text{end}}, \theta_{\text{start}}]$ at radii $(r_{\text{inner}} R_x, r_{\text{inner}} R_y)$.
   - Each arc quadrant is approximated using cubic beziers with control point tangent length:
     $$k = \frac{4}{3} \tan\left(\frac{\Delta\theta}{4}\right)$$
   - When $\theta_{\text{end}} - \theta_{\text{start}} < 2\pi$ or $r_{\text{inner}} > 0$, radial straight line segments connect the outer arc endpoints to the inner arc or center point.
2. **Star Geometry**:
   Given outer radius $R_{\text{outer}} = \min(W, H) / 2$, inner radius $R_{\text{inner}} = R_{\text{outer}} \cdot \text{starInnerScale}$, vertex count $N = \text{pointCount}$:
   - Vertex angle step: $\Delta\theta = \frac{2\pi}{2N} = \frac{\pi}{N}$.
   - For $i \in [0, 2N - 1]$:
     $$r_i = \begin{cases} R_{\text{outer}} & \text{if } i \text{ is even} \\ R_{\text{inner}} & \text{if } i \text{ is odd} \end{cases}$$
     $$(x_i, y_i) = (C_x + r_i \sin(i \Delta\theta), C_y - r_i \cos(i \Delta\theta))$$
3. **LINE Geometry & Resize Invariant**:
   - Stored with `size = (length, 0.0)`.
   - Centerline path: `(0, 0)` to `(length, 0)`.
   - The affine matrix `transform` encodes translation $(t_x, t_y)$ and rotation angle $\phi$.
   - Interactive resizing: Moving endpoint $(X_2, Y_2)$ recomputes:
     $$\text{length} = \sqrt{(X_2 - X_1)^2 + (Y_2 - Y_1)^2}$$
     $$\phi = \text{atan2}(Y_2 - Y_1, X_2 - X_1)$$
     `node.size.x = length`, `node.size.y = 0.0`, `node.transform = Affine::translation(X_1, Y_1) * Affine::rotation(phi)`.
     `stroke.weight` remains unchanged.

---

## 4. Multi-Stroke Architecture (#196, #197)

### 4.1 Data Model Extensions (`crates/designos-doc/src/paint.rs`)
```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrokeAlign {
    #[default]
    Inside,
    Center,
    Outside,
    /// Offset by explicit normal distance (Kiwi StrokeAlign::OFFSET = 3)
    Offset,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StrokeItem {
    pub id: u32,
    pub paints: Vec<Paint>,
    pub weight: f32,
    pub align: StrokeAlign,
    pub offset: f32,
    pub cap: StrokeCap,
    pub join: StrokeJoin,
    pub miter_limit: f32,
    pub dashes: Vec<f32>,
    pub path_trim: Option<Vec2>, // x = start [0..1], y = end [0..1]
    pub visible: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    /// Active/primary stroke parameters (for single-stroke fast path & backward compatibility)
    pub paints: Vec<Paint>,
    pub weight: f32,
    pub side_weights: Option<[f32; 4]>,
    pub align: StrokeAlign,
    pub cap: StrokeCap,
    pub join: StrokeJoin,
    pub miter_limit: f32,
    pub dashes: Vec<f32>,
    pub offset: f32,
    /// Multi-stroke collection (Kiwi strokeData = 571)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<StrokeItem>,
}
```

### 4.2 Multi-Stroke Rendering Sequence
In `crates/designos-render/src/paint.rs` and `designos-gpu/src/geom.rs`:
1. Check if `stroke.items` is non-empty:
   - If empty, render legacy single stroke (`stroke.paints`).
   - If non-empty, iterate through `stroke.items` in ascending index order ($0 \dots M-1$):
     - Filter out items with `!item.visible || item.weight <= 0.0 || item.paints.is_empty()`.
     - Synthesize `StrokePlan` for the item using its specific `align`, `offset`, `weight`, and `path_trim`.
     - Paint stroke item outlines over the destination raster using `item.paints`.

### 4.3 Offset Stroke Math (`StrokeAlign::Offset`)
For a 2D polyline or bezier curve with normal $\mathbf{n}(t)$:
1. Shift vertex position:
   $$\mathbf{p}_{\text{offset}}(t) = \mathbf{p}(t) + \mathbf{n}(t) \cdot \text{strokeOffset}$$
2. Apply standard centerline stroke expansion with weight $w$ on the shifted curve $\mathbf{p}_{\text{offset}}(t)$.

---

## 5. Vector Mask Types (#1506)

### 5.1 Mask Type Data Model (`crates/designos-doc/src/node.rs`)
```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaskType {
    #[default]
    Alpha = 0,
    Outline = 1,
    Luminance = 2,
}
```
Add to `Node`:
```rust
pub struct Node {
    // ...
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_mask: bool,
    #[serde(default, skip_serializing_if = "is_default_mask_type")]
    pub mask_type: MaskType,
    // ...
}
```

### 5.2 Mask Evaluation Pipeline (`crates/designos-render/src/mask.rs`)
```rust
pub fn evaluate_mask_alpha(mask_pm: &sk::Pixmap, mask_type: MaskType) -> Vec<u8> {
    let pixels = mask_pm.data();
    let mut alpha_channel = Vec::with_capacity(mask_pm.width() as usize * mask_pm.height() as usize);
    for chunk in pixels.chunks_exact(4) {
        let (r, g, b, a) = (chunk[0] as f32 / 255.0, chunk[1] as f32 / 255.0, chunk[2] as f32 / 255.0, chunk[3] as f32 / 255.0);
        let coverage = match mask_type {
            MaskType::Alpha => a,
            MaskType::Outline => if a > 0.001 { 1.0 } else { 0.0 },
            MaskType::Luminance => {
                // Rec. 709 Luminance formula
                (0.2126 * r + 0.7152 * g + 0.0722 * b) * a
            }
        };
        alpha_channel.push((coverage.clamp(0.0, 1.0) * 255.0).round() as u8);
    }
    alpha_channel
}
```
For `MaskType::Outline`:
- Stroke paints and strokes are completely stripped from the mask node (`mask_node.stroke.paints.clear()`).
- Only the closed fill geometry is rasterized as an opaque stencil ($A = 1.0$).
- If `mask_node.fills.is_empty()` and only an open stroke exists, the stroke outline is synthesized as the cutting stencil.

---

## 6. Interactive Vector Tools (#193, #194, #499)

### 6.1 Eraser Tool (#193)
- **UI State**: `Tool::Eraser` with brush radius $R \in [1.0, 100.0]$ (default $10.0\,\text{px}$).
- **Cursor**: `cursor-eraser.svg`.
- **Interaction**:
  1. On mouse drag, sample points along stroke trajectory $\mathbf{x}_i$.
  2. Synthesize swept circle polygon $\mathcal{E} = \bigcup_i \mathcal{B}(\mathbf{x}_i, R)$.
  3. Transform $\mathcal{E}$ into the local coordinate system of each selected `NodeKind::Vector`.
  4. Perform boolean subtraction using `i_overlay`:
     - For filled regions: $\mathcal{F}_{\text{new}} = \mathcal{F}_{\text{old}} \setminus \mathcal{E}$.
     - For stroke segments: Intersect segment with circle boundary, insert split vertices at entry/exit points, remove vertices lying inside $\mathcal{E}$.
  5. Rebuild `VectorNetwork` with updated vertices, segments, and regions.

### 6.2 Scissors / Cut Tool (#194)
- **UI State**: `Tool::Cut`. Action name `"Cut tool"`.
- **Interaction**:
  1. Raycast mouse position against all segments of active vector network within hit-test tolerance ($4.0\,\text{px}$).
  2. For hit cubic bezier segment $\mathbf{B}(t) = (1-t)^3 \mathbf{P}_0 + 3(1-t)^2 t \mathbf{P}_1 + 3(1-t) t^2 \mathbf{P}_2 + t^3 \mathbf{P}_3$:
     - Compute closest parameter $t_{\text{cut}} \in (0.0, 1.0)$.
     - Apply de Casteljau split at $t_{\text{cut}}$, yielding left sub-curve $(\mathbf{P}_0, \mathbf{L}_1, \mathbf{L}_2, \mathbf{C})$ and right sub-curve $(\mathbf{C}, \mathbf{R}_1, \mathbf{R}_2, \mathbf{P}_3)$.
  3. Insert new vertex at $\mathbf{C}$.
  4. Replace original segment with two new segments. If segment was part of a closed loop, the loop opens; if it was part of an open path, the path splits into two independent paths.
  5. Provide menu action `fullscreen_actions.break-vector-network-into-paths` to break all disconnected subgraphs into separate `Node`s.

### 6.3 Offset Path Tool (#499)
- **UI Modal**: Toolbar slider `"Offset"` ($[-100.0, 100.0]\,\text{px}$), Join type picker (`Round` / `Square`), `"Done"`, `"Cancel"`.
- **Algorithm**:
  1. Check if source vector contains variable width points (`variableWidthPoints.len() > 0`). If present:
     - Convert variable width stroke to outline fill geometry.
     - Emit visual bell warning toast: `"Variable width stroke outlined for offset"`.
  2. Extract closed contour polygons from `VectorNetwork`.
  3. Execute polygon offsetting using `i_overlay` contour expansion:
     - Positive offset ($D > 0$): Outward polygon dilation.
     - Negative offset ($D < 0$): Inward polygon erosion.
     - Join handling: `Round` generates arc vertices along normal fan; `Square` extends miter up to limit $2.0 \cdot |D|$.
  4. On `"Done"`, commit new geometry as a `NodeKind::Vector` containing the offset paths.

---

## 7. Kiwi Schema & Serialization Diffs

### 7.1 Kiwi Encoding (`crates/designos-fig/src/fig_export.rs`)
```rust
// In encode_node_change:
if let Some(arc) = &ellipse_arc {
    fields.push(("arcData", msg(vec![
        ("startingAngle", fl(arc.starting_angle)),
        ("endingAngle", fl(arc.ending_angle)),
        ("innerRadius", fl(arc.inner_radius)),
    ])));
}

if node.is_mask {
    fields.push(("mask", V::Bool(true)));
    fields.push(("maskIsOutline", V::Bool(node.mask_type == MaskType::Outline)));
    let mask_type_val = match node.mask_type {
        MaskType::Alpha => 0,
        MaskType::Outline => 1,
        MaskType::Luminance => 2,
    };
    fields.push(("maskType", V::Uint(mask_type_val)));
}

// Multi-stroke export (strokeData = 571):
if !node.stroke.items.is_empty() {
    let stroke_msgs: Vec<V> = node.stroke.items.iter().enumerate().map(|(i, s)| {
        msg(vec![
            ("strokeId", V::Int(s.id as i32)),
            ("strokeWeight", fl(s.weight)),
            ("strokeAlign", en(stroke_align_name(s.align))),
            ("strokeOffset", fl(s.offset)),
            ("strokeCap", en(stroke_cap_name(s.cap))),
            ("strokeJoin", en(stroke_join_name(s.join))),
            ("miterLimit", fl(s.miter_limit)),
            ("strokePaint", arr(s.paints.iter().map(encode_paint).collect())),
            ("isDeleted", V::Bool(false)),
        ])
    }).collect();
    fields.push(("strokeData", msg(vec![
        ("strokes", arr(stroke_msgs)),
        ("version", V::Uint(2)),
    ])));
}
```

### 7.2 Kiwi Decoding (`crates/designos-fig/src/fig.rs`)
```rust
// In convert_node:
if let Some(arc) = nc.get("arcData") {
    let s_angle = fnum(arc, "startingAngle").unwrap_or(0.0);
    let e_angle = fnum(arc, "endingAngle").unwrap_or(2.0 * std::f32::consts::PI);
    let i_radius = fnum(arc, "innerRadius").unwrap_or(0.0);
    node.kind = NodeKind::Ellipse(ArcData {
        starting_angle: s_angle,
        ending_angle: e_angle,
        inner_radius: i_radius,
    });
}

if let Some(sd) = nc.get("strokeData") {
    if let Some(strokes_arr) = sd.get("strokes").and_then(Value::as_array) {
        for s in strokes_arr {
            // parse individual stroke items into node.stroke.items
        }
    }
}
```

---

## 8. Test Vectors from Figma

| Test Vector ID | Setup / Action | Input Parameters | Expected Ground-Truth Result | Verification Gate |
|---|---|---|---|---|
| **V-E4-01** | Donut Ellipse with 90° cutout | `startingAngle: 0.0`, `endingAngle: 4.712389` ($270^\circ$), `innerRadius: 0.5` | Center hole of radius $0.5R$, quadrant $[270^\circ, 360^\circ]$ open, boundary sealed by straight radial edges | Visual pixmap comparison bit-exact with Figma export |
| **V-E4-02** | 5-Point Star Inner Ratio | `pointCount: 5`, `starInnerScale: 0.382`, size $100 \times 100$ | Inner vertices at radius $19.1\,\text{px}$, outer vertices at $50.0\,\text{px}$ | Geometry vertex match within $10^{-5}$ tolerance |
| **V-E4-03** | Inside Stroke Stencil on Ellipse | Ellipse $100 \times 100$, Stroke weight $10.0$, `StrokeAlign::Inside` | Outer edge of stroke strictly at $R = 50.0\,\text{px}$; no pixels rendered at $R > 50.0\,\text{px}$ | Pixmap bounding box is exactly $100 \times 100$ |
| **V-E4-04** | Multi-Stroke Layering | Rectangle $200 \times 200$, Stroke 0: Black weight 20 Center; Stroke 1: White weight 10 Center | White stroke renders directly on top of black stroke, yielding $5\,\text{px}$ black borders on either side of white stroke | Pixel sample at offset $7.5\,\text{px}$ from center is black; at $2.5\,\text{px}$ is white |
| **V-E4-05** | Offset Stroke (`StrokeAlign::Offset`) | Path from $(0, 0)$ to $(100, 0)$, weight 4, `strokeOffset: 15.0` | Centerline of rendered stroke is at $Y = 15.0$; stroke footprint covers $Y \in [13.0, 17.0]$ | Pixmap raster matches centerline offset |
| **V-E4-06** | Vector Mask Outline | Frame masked by star with red fill and thick yellow stroke (`MaskType::Outline`) | Yellow stroke is completely ignored; clipped child layers strictly bounded by star's inner/outer fill boundary | Child content outside star fill is $100\%$ transparent |
| **V-E4-07** | Vector Mask Luminance | Pure white square ($Y = 1.0$) with gradient to black ($Y = 0.0$), `MaskType::Luminance` | Target content alpha linearly ramps from $1.0$ at top to $0.0$ at bottom | Alpha gradient bit-exact with formula $0.2126R + 0.7152G + 0.0722B$ |
| **V-E4-08** | Offset Path Variable Width Fallback | Path with wedge variable width profile, click Offset Vector | Toast `"Variable width stroke outlined for offset"` displayed; path outlined to solid polygon, offset applied | `NodeKind::Vector` created with filled polygon contours |
