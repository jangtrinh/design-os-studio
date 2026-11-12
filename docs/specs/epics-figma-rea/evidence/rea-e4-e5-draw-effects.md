# Clean-Room Figma 126.11.7 REA Evidence & Studio Codebase Anchors: Epic E4 (Draw & Vector) & Epic E5 (Effects & Shaders)

**Status:** Complete  
**Date:** 2026-11-12  
**Target File:** `/Users/jangtrinh/Products/design-os-studio/plans/261112-epic-figma-rea-specs/evidence/rea-e4-e5-draw-effects.md`  
**REA Analysis Evidence ID:** `ev_6f56bed2b1984cdf5d746450517171fe32bc951557c54a88dce368fd3db6b704`  
**REA Subject Digest (SHA-256):** `e24f67a66fea09b8e13275a2bf75dcab4e9675a22de1b2b6c2b08deab41029b1`  
**Inspected Artifacts:**
- Kiwi Schema: `crates/designos-fig/tests/data/figma-real-schema.txt` (SHA-256: `660bf4f8352614ddf466b8cfbcf761a2bfadcb6aebe01ee69fc73418c8bfa2e0`)
- TypeScript Typing: `/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/re/bound-variants/typings/plugin-api.d.ts`
- WASM Text & Binary Strings: `/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/re/layout/wasm-strings.txt`
- UI Localization Dictionary: `/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/re/layout/figma_app-6372f0021b0d5c94.min.en.json`
- Analyzed JS Chunk Set:
  - `37794-21d439e52cb0e66c.min.js`: SHA-256 `f4f8cbcd9fa1dcfd7041347e69dc0a4f0d7e1fceaf96b23f740c6e8191f92442`
  - `57877-5552342863bbd2f3.min.js`: SHA-256 `d816d11190265b336dd641075bca3b20cfead4bc47040a3a8e1d1b5be5866508`
  - `97228-1903339c9ebd9deb.min.js`: SHA-256 `6fd7752f35655a66c174a3f1225a35273835af1b95811419109488255ae57c50`
  - `b2835def-9bf46100524c15bb.min.js`: SHA-256 `097a8d8c4994ba94643328e554c86c14378394051823fadb33b2c7e05a849101`

---

## Part 1: Epic E4 — Draw & Vector (#17)

### 1.1 Shape & Stroke Parity (#952, #953, #196, #197, #193, #194, #499, #1506)

#### Inside/Outside Stroke Math & Storage
- `[FACT - seen: figma-real-schema.txt:215-219, 1498, 1767]` Stroke alignment enum is `StrokeAlign { CENTER = 0, INSIDE = 1, OUTSIDE = 2, OFFSET = 3 }`. Field ID on `Stroke` is `strokeAlign = 6 : StrokeAlign`; field ID on `NodeChange` is `strokeAlign = 29 : StrokeAlign`.
- `[FACT - seen: plugin-api.d.ts:8753-8766]` In Figma, inside and outside strokes are implemented by doubling the stroke weight (`weight * 2.0`) and clipping (masking) the expanded stroke path against the shape boundary:
  - `INSIDE`: Stroke is masked by the shape fill region (`DestinationIn` / stencil intersection).
  - `OUTSIDE`: Stroke is masked by the inverted shape fill region (`DestinationOut` / outside stencil).
  - `strokeGeometry`: Always exported as center-aligned paths relative to the shape coordinate system regardless of `strokeAlign`.
- `[FACT - seen: figma-real-schema.txt:1523-1526, 1762]` `ELLIPSE` with partial arcs or donuts stores `ArcData`:
  - `Message ArcData { startingAngle = 1 : Float, endingAngle = 2 : Float, innerRadius = 3 : Float }`.
  - Field ID on `NodeChange` is `arcData = 195 : ArcData`.
  - `innerRadius`: Normalized float in range `[0.0, 1.0]` representing the inner hole radius relative to outer radius.
- `[FACT - seen: figma-real-schema.txt:1762]` `STAR` geometry stores:
  - `starInnerScale = 24 : Float` on `NodeChange`. Represents the ratio of inner vertex distance to outer vertex distance (default 0.382 for a 5-point star).
  - Number of vertices is governed by `pointCount` on polygon/star base.
- `[INFERENCE]` Resizing Behavior across Containers:
  - Resizing direct shape via bounding box: Updates `size.x` and `size.y`. `arcData.innerRadius` and `starInnerScale` are dimensionless scale ratios and remain constant. `strokeWeight` remains invariant in document pixels.
  - Inside a `GROUP`: Figma groups have no local bounds. Resizing a group calculates a 2D affine transformation on child nodes, updating child translation and `size` (unless using Scale Tool `K`, which also multiplies `strokeWeight` by the scale factor).
  - Inside a `FRAME`: When child has `SCALE` constraints, resizing the parent frame scales child `x, y, width, height` by parent dimension ratio. `strokeWeight` is preserved unless vector vertices are converted.
  - `LINE`: Stored as `size: { x: length, y: 0.0 }`. Rotation is encoded in `relativeTransform` (2x3 affine matrix). Canvas resize adjusts `size.x` and rotation in `relativeTransform`; `strokeWeight` is decoupled.

#### Multi-Stroke Model & Per-Side Weights
- `[FACT - seen: figma-real-schema.txt:1489-1506, 1779]` Multi-stroke architecture is stored in `strokeData = 571 : StrokeData` on `NodeChange`:
  - `Message StrokeData { strokes = 1 : Stroke[], version = 2 : Uint }`
  - `Message Stroke`:
    - `strokeId = 1 : Int`
    - `strokeWeight = 2 : Float`
    - `strokeWeightVar = 3 : VariableData`
    - `strokePaint = 4 : Paint[]`
    - `styleIdForStrokeFill = 5 : StyleId`
    - `strokeAlign = 6 : StrokeAlign` (CENTER=0, INSIDE=1, OUTSIDE=2, OFFSET=3)
    - `strokeCap = 7 : StrokeCap`
    - `strokeCapSize = 8 : Number`
    - `strokeJoin = 9 : StrokeJoin`
    - `miterLimit = 10 : Float`
    - `dashPattern = 11 : Float[]`
    - `pathTrim = 12 : OptionalVector`
    - `strokeOffset = 13 : Float`
    - `isDeleted = 14 : Bool`
- `[FACT - seen: figma-real-schema.txt:1774, 2195]` Legacy single-stroke model on `NodeChange`:
  - `strokePaints = 39 : Paint[]`
  - `strokeWeight = 26 : Float`
  - `strokeAlign = 29 : StrokeAlign`
  - `strokeOffset = 554 : Float`
- `[FACT - seen: figma-real-schema.txt:1781-1790]` Independent per-side border weights on `NodeChange`:
  - `borderTopWeight = 295 : Float`
  - `borderRightWeight = 298 : Float`
  - `borderBottomWeight = 296 : Float`
  - `borderLeftWeight = 297 : Float`
  - `borderStrokeWeightsIndependent = 299 : Bool`
  - `borderTopHidden = 287 : Bool`, `borderBottomHidden = 288 : Bool`, `borderLeftHidden = 289 : Bool`, `borderRightHidden = 290 : Bool`
  - `bordersTakeSpace = 294 : Bool`
- `[FACT - seen: plugin-api.d.ts:8708-8714, designos-render/src/lib.rs:771-796]` Painting order:
  - Fills render first, strokes render second.
  - In `strokePaints` and `strokeData.strokes`, elements are evaluated and rendered in index order (`0` rendered first at the bottom, higher indices painted on top).

#### Vector Mask Outline Rules (#1506)
- `[FACT - seen: figma-real-schema.txt:1692-1695, 1754-1758]` Mask storage on `NodeChange`:
  - `mask = 16 : Bool`
  - `maskIsOutline = 18 : Bool`
  - `maskType = 317 : MaskType`
  - `Enum MaskType { ALPHA = 0, OUTLINE = 1, LUMINANCE = 2 }`
- `[FACT - seen: plugin-api.d.ts:5556, 7638-7642]` Plugin API maps `MaskType` to:
  - `'ALPHA'`: Alpha channel of the rendered node (including transparency of fills, strokes, and child layers).
  - `'VECTOR'`: Maps to Kiwi `OUTLINE = 1`. Uses the solid geometric path boundary / fill silhouette of the shape.
  - `'LUMINANCE'`: Greyscale brightness formula ($Y = 0.2126R + 0.7152G + 0.0722B$).
- `[FACT - seen: designos-render/src/mask.rs:24-32]` Does a `VECTOR` mask include stroke geometry?
  - When masking by vector outline (`VECTOR`/`OUTLINE`), the mask stencil uses the closed fill boundary geometry of the shape. Stroke paints and stroke expansions are stripped (`n.stroke.paints.clear()`), unless the node has no fill and only an open stroke, in which case outline stroke geometry is synthesized as an opaque cutting stencil.

#### Tool Strings & WASM Geometry Symbols (#193, #194, #499)
- `[FACT - seen: figma_app en.json, wasm-strings.txt]` UI Strings, Action IDs, and WASM Symbols:
  - **Eraser (#193)**:
    - Action ID: `fullscreen_actions.set-tool-eraser` -> `"Eraser"`
    - Vector Eraser Action: `fullscreen_actions.set-tool-vector-eraser` -> `"Erase"`
    - WASM Behavior: `VectorEraserVectorBehavior`, `vector-eraser-tool`, `eraser_use`
    - Action Logging: `illustration.vector_eraser_used`
    - Icons: `cursor-eraser.svg`, `cursor-eraser-ui3.svg`
  - **Scissors / Cut (#194)**:
    - Action ID: `fullscreen_actions.set-tool-cut` -> `"Cut"`
    - Tool Name: `fullscreen_actions.cut_tool` -> `"Cut tool"`
    - Vector Split Actions: `fullscreen_actions.split_vector` -> `"Split vector"`, `fullscreen_actions.break-vector-network-into-paths` -> `"Break branching vector network into paths"`
    - Join Actions: `fullscreen_actions.join-selection` -> `"Join selection"`, `fullscreen_actions.smooth-join-selection` -> `"Smooth join selection"`
  - **Offset Path (#499)**:
    - Action ID: `fullscreen_actions.set-tool-offset-path` -> `"Offset vector"`
    - Toolbar Slider: `fullscreen.toolbar.offset-path-slider-label` -> `"Offset"`, `fullscreen.toolbar.offset-path-slider-tooltip` -> `"Amount"`
    - Join Types: `fullscreen.toolbar.offset-path-join-type-round` -> `"Round"`, `fullscreen.toolbar.offset-path-join-type-square` -> `"Square"`
    - Buttons: `fullscreen.toolbar.offset-path-apply-button` -> `"Done"`, `fullscreen.toolbar.offset-path-cancel-button` -> `"Cancel"`
    - Warnings: `fullscreen_actions.outlined-vws-for-offset-visual-bell` -> `"Variable width stroke outlined for offset"`, `fullscreen_actions.vws-removed-for-offset-visual-bell` -> `"Variable width stroke removed for offset"`
    - WASM C++ Symbols:
      - `int Figma::OffsetPathState_Internal___getProperty_offset(int)`
      - `int Figma::OffsetPathState_Internal___getProperty_maxSliderOffset(int)`
      - `int Figma::OffsetPathState_Internal___getProperty_minSliderOffset(int)`
      - `int Figma::OffsetPathState_Internal___getProperty_joinType(int)`
      - `int Figma::AppStateTsApi_offsetPathState()`
      - `MutableOffsetPathState`, `OffsetPathBehavior`, `MutableWritableObservableValue_OffsetPathJoinType_`
      - `illustration.offset_path`

---

### 1.2 Figma Draw Kiwi Schema & Engine (#179–#206)

#### Variable Width Points & Width Profile Presets
- `[FACT - seen: figma-real-schema.txt:1507-1511, 2188]` Schema definition:
  - `Message VariableWidthPoint`:
    - `position = 1 : Float` (position $t \in [0.0, 1.0]$ along stroke path)
    - `ascent = 2 : Float` (left side width / offset)
    - `descent = 3 : Float` (right side width / offset)
    - `segmentId = 4 : Int` (index of segment in vector network)
  - Field ID on `NodeChange`: `variableWidthPoints = 447 : VariableWidthPoint[]`
- `[FACT - seen: plugin-api.d.ts:8825-8851]` Width Profile Presets:
  - `UNIFORM`
  - `WEDGE`
  - `TAPER`
  - `QUARTER_TAPER`
  - `EYE`
  - `MIRRORED_TAPER`
  - `CUSTOM` (requires explicit `variableWidthPoints` array)
- `[FACT - seen: figma_app en.json, plugin-api.d.ts:8976-8978]` Dashed Stroke & Branching Exclusion Rules:
  - Error Toast: `fullscreen_actions.dashed_stroke_variable_width_disabled` -> `"Can't use variable width on dashed strokes"`
  - Dynamic Stroke Conflict: `fullscreen_actions.dynamic_stroke_variable_width_disabled` -> `"Can't use variable width on dynamic strokes"`
  - Branching Vector Warning: `fullscreen.visual_bell.branching_vector_network_warning` -> `"Branching vectors don't support variable width"`
  - WASM Engine Invariant: `VariableWidthPointsInvariantEnforcer` / `VARIABLE_WIDTH_POINTS_INVARIANT_ENFORCER`

#### Path Trim
- `[FACT - seen: figma-real-schema.txt:1504, 2194, 4127-4128]` Schema definition:
  - `Message OptionalVector { value = 1 : Vector }`
  - Field ID on `Stroke`: `pathTrim = 12 : OptionalVector`
  - Field ID on `NodeChange`: `pathTrim = 542 : OptionalVector`
  - Coordinate Representation: Stored as `Vector { x, y }` where `x` = start trim ($0.0 \dots 1.0$), `y` = end trim ($0.0 \dots 1.0$).
- `[FACT - seen: wasm-strings.txt, figma-real-schema.txt:2698-2699]` Engine Implementation:
  - C++ source symbol in WASM: `fullscreen/lib/geometry/FGPathTrim.cpp`
  - Function symbols: `applyPathTrim`, `applyOffsetPathAnimation`
  - Motion Variable Fields: `PATH_TRIM_START = 54`, `PATH_TRIM_END = 55`

#### Transform Modifiers / TRANSFORM Group
- `[FACT - seen: figma-real-schema.txt:88, 1292-1306, 1777]` Schema definition:
  - `NodeType::TRANSFORM = 48`
  - Field ID on `NodeChange`: `transformModifiers = 455 : TransformModifier[]`
  - `Enum TransformModifierType { REPEAT = 0, SYMMETRY = 1, SKEW = 2 }`
  - `Message TransformModifier`:
    - `type = 1 : TransformModifierType`
    - `offset = 2 : Vector`
    - `visible = 3 : Bool`
    - `count = 4 : Uint` (linear/radial repeat count: integer range $2 \dots 40$)
    - `repeatType = 5 : RepeatType` (`LINEAR = 0`, `RADIAL = 1`)
    - `axis = 6 : EffectAxis` (`X = 0`, `Y = 1`, `X_AND_Y = 2`)
    - `unitType = 7 : UnitType`
    - `order = 8 : RepeatOrder`
    - `skewX = 9 : Float`
    - `skewY = 10 : Float`
- `[FACT - seen: rea-findings.md:113]` Modifier Cap: Maximum of 2 concurrent modifiers per transform group.

#### Pattern Fill (PaintType::PATTERN)
- `[FACT - seen: figma-real-schema.txt:166, 906-911, 938-945]` Schema definition:
  - `PaintType::PATTERN = 8`
  - `sourceNodeId = 26 : GUID` (GUID of canvas frame/component acting as pattern tile)
  - `patternTileType = 28 : PatternTileType` (`RECTANGULAR = 0`, `HORIZONTAL_HEXAGONAL = 1`, `VERTICAL_HEXAGONAL = 2`)
  - `scale = 12 : Float` (or `scalingFactor` in plugin API)
  - `spacing = 27 : Float` (legacy scalar) & `patternSpacing = 37 : Vector` (modern 2D spacing)
  - `verticalAlignment = 29 : PatternAlignment` (`START = 0`, `CENTER = 1`, `END = 2`)
  - `horizontalAlignment = 30 : PatternAlignment` (`START = 0`, `CENTER = 1`, `END = 2`)

#### Brush Node & Settings
- `[FACT - seen: figma-real-schema.txt:86, 1471-1488, 2189-2193]` Schema definition:
  - `NodeType::BRUSH = 46`
  - Field on Node: `brushType = 452 : BrushType` (`STRETCH = 0`, `SCATTER = 1`)
  - Field on Node: `strokeBrushGuid = 446 : GUID` (points to `BRUSH` node 46)
  - Field on Node: `strokeSeed = 482 : Uint64` (deterministic procedural variation seed)
  - `Message DynamicStrokeSettings` (field `dynamicStrokeSettings = 448`):
    - `frequency = 1 : Float` (range $0.01 \dots 20$)
    - `wiggle = 2 : Float` (amplitude $\ge 0.0$)
    - `smoothen = 3 : Float` (smoothing factor $0.0 \dots 1.0$)
  - `Message ScatterStrokeSettings` (field `scatterStrokeSettings = 449`):
    - `gap = 1 : Float` (spacing between stamps, $\ge 0.25$)
    - `wiggle = 2 : Float` (offset jitter $\ge 0.0$)
    - `angularJitter = 3 : Float` (angle jitter in degrees, $-180^\circ \dots 180^\circ$)
    - `rotation = 4 : Float` (base stamp angle, $-180^\circ \dots 180^\circ$)
    - `sizeJitter = 5 : Float` (size variance $0 \dots 3$)
  - `Message StretchStrokeSettings` (field `stretchStrokeSettings = 450`):
    - `orientation = 1 : BrushOrientation` (`FORWARD = 0`, `REVERSE = 1`)
- `[FACT - seen: plugin-api.d.ts:8871-8922]` Standard Presets:
  - Scatter Brushes (10): `BUBBLEGUM`, `WITCH_HOUSE`, `SHOEGAZE`, `HONKY_TONK`, `SCREAMO`, `DRONE`, `DOO_WOP`, `SPOKEN_WORD`, `VAPORWAVE`, `OI` (+ `CUSTOM`).
  - Stretch Brushes (15): `HEIST`, `BLOCKBUSTER`, `GRINDHOUSE`, `BIOPIC`, `SPAGHETTI_WESTERN`, `SLASHER`, `HARDBOILED`, `VERITE`, `EPIC`, `SCREWBALL`, `ROM_COM`, `NOIR`, `PROPAGANDA`, `MELODRAMA`, `NEW_WAVE` (+ `CUSTOM`).

#### Text on Path (NodeType::TEXT_PATH)
- `[FACT - seen: figma-real-schema.txt:81, 1038-1040, 2134]` Schema definition:
  - `NodeType::TEXT_PATH = 41`
  - Field on Node: `textPathStart = 432 : TextPathStart`
  - `Message TextPathStart`:
    - `tValue = 1 : Float` (parameter $t \in [0.0, 1.0]$ along start segment)
    - `forward = 2 : Bool` (tangent traversal direction / flip)
- `[FACT - seen: plugin-api.d.ts:1351-1355, 11074-11132]` Geometry and Text Properties:
  - Bound to an underlying `VectorNetwork` representing the baseline curve.
  - Text alignment: `textAlignHorizontal: 'LEFT' | 'CENTER' | 'RIGHT' | 'JUSTIFIED'` controls distribution along the curve.
  - Text alignment: `textAlignVertical: 'TOP' | 'CENTER' | 'BOTTOM'` controls baseline vertical shift (above, centered, or below the curve).
  - Actions: `fullscreen_actions.combine-text-path-layers` -> `"Convert to text on path"`, `fullscreen_actions.separate-text-path-layers` -> `"Create separate layers"`.

---

## Part 2: Epic E5 — Effects & Shaders (#18)

### 2.1 Effects & Image Fill Parity (#521, #37, #38, #1328)

#### Image Fill (PaintType::IMAGE)
- `[FACT - seen: figma-real-schema.txt:875-905]` Schema fields on `Paint`:
  - `type = 1 : PaintType` (`IMAGE = 5`)
  - `image = 8 : Image` (holds `hash = 1 : Byte[]`)
  - `imageScaleMode = 10 : ImageScaleMode` (`STRETCH = 0`, `FIT = 1`, `FILL = 2`, `TILE = 3`)
  - `transform = 7 : Matrix` (`imageTransform` 2x3 affine matrix for `CROP` mode)
  - `scale = 12 : Float` (`scalingFactor` for `TILE` mode)
  - `rotation = 11 : Float` (rotation in 90-degree steps: $0^\circ, 90^\circ, 180^\circ, 270^\circ$)
  - `imageVar = 31 : VariableData` (variable binding for switching image per mode scope, #521)
  - `opacityVar = 38 : VariableData` (opacity variable binding)
- `[FACT - seen: figma-real-schema.txt:864-874, plugin-api.d.ts:4720-4728]` Paint Filter 7 Sliders (`PaintFilterMessage` / `ImageFilters`):
  - Values default to `0.0`, with slider range `[-1.0, 1.0]`:
    1. `exposure = 5 : Float` (Kiwi 5)
    2. `contrast = 9 : Float` (Kiwi 9)
    3. `vibrance = 8 : Float` (Kiwi 8, mapped to `saturation` in plugin API)
    4. `temperature = 7 : Float` (Kiwi 7)
    5. `tint = 1 : Float` (Kiwi 1)
    6. `highlights = 3 : Float` (Kiwi 3)
    7. `shadows = 2 : Float` (Kiwi 2)
    - Secondary Kiwi fields: `detail = 4 : Float`, `vignette = 6 : Float`, `brightness = 10 : Float`.
- `[FACT - seen: wasm-strings.txt:GLSL-uniform-block]` Shader Transfer Formulas (Clean-Room Mathematical Reconstruction):
  1. **Linearization & Luminance**:
     - Convert sRGB to linear RGB: $C_{\text{lin}} = C^{2.2}$.
     - Rec. 709 Luminance weights: $w_L = [0.2126, 0.7152, 0.0722]$.
     - Luminance scalar: $L = \text{dot}(C_{\text{lin}}, w_L)$.
  2. **Highlights & Shadows**:
     - Luminance delta from midpoint: $\Delta = L - (\text{threshold})^{2.2}$.
     - Soft falloff factor: $S = \frac{0.75 \cdot \Delta}{\sqrt{1.0 + 4.0 \Delta^2}}$.
     - Highlight/Shadow bias: $B_{\text{hl}} = \begin{cases} S \cdot \text{highlights} & \text{if } \Delta > 0 \\ -S \cdot 0.999 \cdot \text{shadows} & \text{if } \Delta \le 0 \end{cases}$.
  3. **Temperature, Tint & Contrast in CIELAB space**:
     - Matrix transform $C_{\text{lin}} \to \text{CIE XYZ}$ (D65 standard illuminant).
     - Non-linear cube root $f(t) = t^{1/3}$ with white point normalization ($X / 0.95047, Z / 1.08883$).
     - Compute CIELAB coordinates: $L^* = 116 f(Y) - 16$, $a^* = 500 [f(X) - f(Y)]$, $b^* = 200 [f(Y) - f(Z)]$.
     - Tint offset: $a^* \mathrel{+}= 30.0 \cdot \text{tint}$.
     - Temperature offset: $b^* \mathrel{+}= 30.0 \cdot \text{temperature}$.
     - Contrast adjustment: Smoothstep sigmoid scaling centered on $L^*$ pivot.
     - Inverse CIELAB to XYZ, then matrix transform XYZ to linear RGB clamped to $[0, 1]$.
  4. **Exposure Tone Curve**:
     - Exponential multiplier: $k = 10^{2.0 \cdot (\text{exposure} + B_{\text{hl}})} - 1.0$.
     - Tone curve mapping: $C_{\text{exp}} = C \cdot \frac{k + 1.0}{C \cdot k + 1.0}$.
  5. **Saturation / Vibrance**:
     - When desaturating ($\text{sat} \le 0$): Linear lerp towards monochrome $L$: $C \mathrel{-}= (L - C) \cdot \text{sat}$.
     - When boosting vibrance ($\text{sat} > 0$): Per-channel headroom ratio scales less-saturated channels more aggressively:
       $q = \frac{L}{L - \min(R,G,B) + \epsilon}$, $r = \frac{1.0 - L}{\max(R,G,B) - L + \epsilon}$, $k_{\text{vib}} = \min(2.0, \text{mix}(1.0, \min(q, r), \text{sat}))$.
       $C \mathrel{+}= (L - C) \cdot (1.0 - k_{\text{vib}})$.
  6. **Gamma Encoding**: $C_{\text{out}} = C^{1.0 / 2.2}$.

#### Progressive Blur (#37)
- `[FACT - seen: figma-real-schema.txt:1218-1220, 1252-1255]` Kiwi Schema definitions on `Message Effect`:
  - `blurOpType = 19 : BlurOpType` (`NORMAL = 0`, `PROGRESSIVE = 1`)
  - `startRadius = 21 : Float`
  - `radius = 4 : Float` (serves as the end radius of the progressive blur)
  - `startOffset = 20 : Vector`
  - `endOffset = 28 : Vector`
  - Bound Variables: `startRadiusVar = 45`, `startOffsetXVar = 46`, `startOffsetYVar = 47`, `endOffsetXVar = 48`, `endOffsetYVar = 49`
- `[FACT - seen: plugin-api.d.ts:4465-4482]` Coordinate units and falloff:
  - Start and end offsets are in **normalized object space (UV $[0.0, 1.0]$)** relative to the node's bounding box ($(0, 0)$ is top-left, $(1, 1)$ is bottom-right).
  - Blur radius interpolates linearly along the vector between `startOffset` and `endOffset`, evaluating variable Gaussian blur kernel diameters.

#### Glass, Noise & Texture (#38)
- `[FACT - seen: figma-real-schema.txt:174-184, 1234-1291]` Kiwi Schema definitions on `Message Effect`:
  - `type = 1 : EffectType` (`GRAIN = 6`, `NOISE = 7`, `GLASS = 8`)
  - `blendMode = 6 : BlendMode` (Kiwi field 6!)
  - `showShadowBehindNode = 8 : Bool` (Kiwi field 8!)
  - **Glass Effect Fields**:
    - `refractionRadius = 30 : Float` (UI/REST label: `depth`, range $\ge 1.0$, default 10.0)
    - `refractionIntensity = 36 : Float` (UI/REST label: `refraction`, range $[0.0, 1.0]$)
    - `specularAngle = 31 : Float` (UI/REST label: `lightAngle`, range $[0^\circ, 360^\circ]$)
    - `specularIntensity = 32 : Float` (UI/REST label: `lightIntensity`, range $[0.0, 1.0]$)
    - `bevelSize = 33 : Float` (bevel width in px)
    - `chromaticAberration = 34 : Float` (UI/REST label: `dispersion`, range $[0.0, 1.0]$)
    - `reflectionDistance = 35 : Float` (UI/REST label: `splay`, range $[0.0, 1.0]$, variable binding `splayVar = 41`)
    - `radius = 4 : Float` (frost / background blur radius in px)
  - **Noise & Texture (Grain) Fields**:
    - `noiseType = 26 : NoiseType` (`MULTITONE = 0`, `MONOTONE = 1`, `DUOTONE = 2`)
    - `noiseSize = 22 : Vector` (2D vector size; scalar access uses `noiseSize.x`)
    - `density = 25 : Float` ($0.0 \dots 1.0$)
    - `opacity = 27 : Float` ($0.0 \dots 1.0$)
    - `seed = 29 : Uint` (pseudo-random seed)
    - `color = 2 : Color` (primary noise color)
    - `secondaryColor = 24 : Color` (secondary noise color for `DUOTONE`)
    - `clipToShape = 23 : Bool` (constrains noise/texture displacement to shape silhouette)

#### Background Blur & Backdrop Alpha Compositing (#1328)
- `[FACT - seen: figma-real-schema.txt:1241, plugin-api.d.ts:4381-4383, designos-render/src/fx.rs:230-245]` Backdrop Alpha Compositing:
  - `showShadowBehindNode = 8 : Bool` on `Message Effect`: Controls whether drop shadow geometry is masked out beneath translucent or transparent node fills.
  - Standard Drop Shadow behavior (`showShadowBehindNode == false`): The node's silhouette is subtracted from the shadow raster (`DestinationOut`), preventing the drop shadow from darkening the interior of semi-transparent or translucent layers.
  - When `showShadowBehindNode == true`: The drop shadow is rendered fully behind the node without silhouette clipping.
  - Background Blur (`BACKGROUND_BLUR` / `GLASS`):
    - Reads the composited backdrop behind the layer.
    - Applies Gaussian blur / refraction.
    - Fills of the current layer composite *over* the blurred backdrop using Porter-Duff `SourceOver` (or layer blend mode).
    - Strokes render last on top of fills.

---

### 2.2 Video Fill, Custom Shaders & 3D (#168–#176, #571)

#### Video Fill (PaintType::VIDEO)
- `[FACT - seen: figma-real-schema.txt:165, 843-845, 894-895, 2600-2607]` Kiwi Schema fields on `Paint`:
  - `type = 1 : PaintType` (`VIDEO = 7`)
  - `video = 18 : Video` (`hash = 1 : Byte[]`, `s3Url = 2 : String`)
  - `image = 8 : Image` (poster frame image hash)
  - `videoFillPlayback = 47 : VideoFillPlayback`:
    - `startTimeMs = 1 : Uint`
    - `endTimeMs = 2 : Uint`
    - `timelineOffsetUs = 3 : Int64`
    - `playbackDurationUs = 4 : Int64`
    - `loopPhaseOffsetUs = 5 : Int64`
    - `volume = 6 : Float`
    - `mute = 7 : Bool`
  - On `MEDIA` node: `Message VideoPlayback` has `autoplay = 1 : Bool`, `mediaLoop = 2 : Bool`, `muted = 3 : Bool`, `showControls = 4 : Bool`, `playbackRate = 7 : Float`.

#### Custom Shaders & Mesh Gradients (#39, #170, #571)
- `[FACT - seen: figma-real-schema.txt:168, 184, 917-918, 1279-1280, 2828-2862]` Kiwi Schema definitions:
  - `PaintType::CUSTOM = 10`
  - `EffectType::CUSTOM = 9`
  - Fields on `Paint`:
    - `customEffectId = 39 : CodeComponentId` (`guid = 1 : GUID`, `assetRef = 2 : AssetRef`)
    - `componentPropAssignments = 40 : ComponentPropAssignment[]`
  - Fields on `Effect`:
    - `customEffectId = 43 : CodeComponentId`
    - `componentPropAssignments = 44 : ComponentPropAssignment[]`
  - Uniform Schema (`Message ComponentPropAssignment`):
    - `defID = 1 : GUID`
    - `value = 2 : ComponentPropValue`:
      - `boolValue = 1 : Bool`
      - `textValue = 2 : TextData`
      - `guidValue = 3 : GUID`
      - `floatValue = 4 : Float`
      - `easingData = 5 : EasingData`
      - `vectorValue = 6 : Vector`
      - `lineValue = 7 : Line`
      - `circleValue = 8 : Circle`
      - `rotation3DValue = 9 : Rotation3D`
      - `circlePointValue = 10 : CirclePoint`
      - `gradientValue = 11 : Gradient` (mesh gradient control points)
      - `colorPointValue = 12 : ColorPoint`
      - `colorArrayValue = 13 : ColorArray`
      - `paintValue = 14 : Paint`
    - `varValue = 3 : VariableData` (variable binding to shader parameter)

#### 3D Transforms & Perspective
- `[FACT - seen: figma-real-schema.txt:1307-1365, 1778, 1780]` Kiwi Schema definitions:
  - Fields on `NodeChange`:
    - `transform3d = 570 : Transform3d`
    - `scene3d = 572 : Scene3d`
  - `Enum TransformSchemaType { NONE = 0, FIXED_ORDER = 1, FREEFORM = 2 }`
  - `Message Transform3d`:
    - `type = 1 : TransformSchemaType`
    - `value = 2 : TransformSchemaValue`
    - `backfaceHidden = 3 : Bool`
    - `visible = 4 : Bool`
  - `Message FixedOrderTransform3d`:
    - `perspective = 1 : Float`
    - `translateZ = 2 : Float`
    - `rotateX = 3 : Float`
    - `rotateY = 4 : Float`
    - `rotateZ = 5 : Float`
  - `Struct Matrix4f`: 16 floats (`m00 = 1` through `m33 = 16`)
  - `Message Scene3d`:
    - `perspective = 1 : Float`
    - `perspectiveOrigin = 2 : NumberVector2D`
    - `preserve3d = 3 : Bool`
    - `visible = 4 : Bool`

---

## Part 3: Studio Codebase Grounding

### Exact Source File Anchors & Parity Gaps

| Area | File Path | Existing Line Numbers & Symbols | Parity Gap & Implementation Requirement |
|---|---|---|---|
| **Paint Model** | `crates/designos-doc/src/paint.rs` | Lines 189–214: `pub enum PaintKind`<br>Lines 217–222: `pub struct Paint` | Lacks `PaintKind::Pattern`, `PaintKind::Shader`, `PaintKind::Video`, and filter adjustments (`FilterColorAdjust`). |
| **Stroke Model** | `crates/designos-doc/src/paint.rs` | Lines 242–248: `pub enum StrokeAlign`<br>Lines 267–277: `pub struct Stroke` | `StrokeAlign` missing `Offset = 3`. `Stroke` missing `stroke_offset`, `path_trim`, `variable_width_points`, `brushes`. |
| **Effect Model** | `crates/designos-doc/src/paint.rs` | Lines 297–328: `pub enum Effect`<br>Lines 380–395: `pub struct GlassEffect` | Lacks `ProgressiveBlur`, `ShaderEffect`. `GlassEffect` lacks `splay` (`reflectionDistance`) and `bevel`. `NoiseEffect` only supports scalar size. |
| **Node Scene Graph** | `crates/designos-doc/src/node.rs` | Lines 65–120: `pub struct Node`<br>Lines 453–508: `pub enum NodeKind` | `Node.is_mask` is boolean; missing `mask_type: MaskType` (`Alpha`, `Vector`, `Luminance`). `NodeKind` missing `TextPath (41)`, `Brush (46)`, `Transform (48)`. |
| **Figma Importer** | `crates/designos-fig/src/fig.rs` | Lines 1530–1540: `strokePaints`<br>Lines 2692–2735: `fn convert_effect` | `convert_effect` maps `FOREGROUND_BLUR`, `GRAIN`, `GLASS`, but omits `blurOpType` (progressive), `splay`, `bevel`, and multi-stroke `strokeData (571)`. |
| **Figma Exporter** | `crates/designos-fig/src/fig_export.rs` | Lines 2025–2120: `fn encode_effect_full` | Encodes legacy strokes only; does not export `strokeData (571)`, `pathTrim (542)`, `variableWidthPoints (447)`, or progressive blur. |
| **REST Importer** | `crates/designos-fig/src/rest_import.rs` | Lines 109–159: `fn parse_effect` | Parses `DROP_SHADOW`, `INNER_SHADOW`, `LAYER_BLUR`, `BACKGROUND_BLUR`, `NOISE`, `TEXTURE`, `GLASS`. Omits `PROGRESSIVE` blur and `splay`. |
| **Render Outline** | `crates/designos-render/src/outline.rs` | Lines 98–114: `StrokeClip`, `StrokePlan`<br>Lines 163–211: `per_side_stroke_path`<br>Lines 215–239: `stroke_plan` | Centers on center/inside/outside clipping. Lacks `StrokeAlign::Offset` distance shifting and variable width tessellation. |
| **Render Stencils** | `crates/designos-render/src/mask.rs` | Lines 24–33: `pub fn mask_stencil` | Strips all strokes (`n.stroke.paints.clear()`). Needs handling for `VECTOR` mask type vs `ALPHA` vs `LUMINANCE`. |
| **Render Paint** | `crates/designos-render/src/paint.rs` | Lines 395–418: `pub fn stroke_paths` | Evaluates strokes sequentially; lacks shader evaluation for 7 image filter sliders and pattern tiling. |
| **GPU Shaders** | `crates/designos-gpu/src/shaders/fx.wgsl` | Lines 123–140: `fn fs_displace`<br>Lines 146–178: `fn fs_glass` | `fs_glass` lacks `splay` refraction displacement. Missing progressive blur shader and 7-slider color transfer pipeline. |
| **GPU Geometry** | `crates/designos-gpu/src/geom.rs` | Lines 1–60: `PaintCache`, `IdHasher` | GPU tessellator lacks variable width stroke mesh synthesis and multi-stroke layered mesh generation. |
| **GPU Paint Conv** | `crates/designos-gpu/src/paintconv.rs` | Lines 1–40: `PaintInst`, `PaintCache` | Converts solid and gradient ramps; lacks texture pattern sampler and video playback texture binding. |
