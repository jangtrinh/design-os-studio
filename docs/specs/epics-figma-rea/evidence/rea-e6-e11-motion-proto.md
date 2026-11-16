# Clean-Room Figma 126.11.7 REA Evidence: Motion & Media Export (Epic E6) and Prototyping (Epic E11)

**Date:** 2026-11-12  
**Target Architecture:** Figma 126.11.7 (Engine, Web App, & Prototyping WASM) vs. DESIGN:OS Studio (`design-os-studio`)  
**Investigator:** Flash REA Worker B (`Gemini 3.8 Flash`)  
**Target Path:** `/Users/jangtrinh/Products/design-os-studio/plans/261112-epic-figma-rea-specs/evidence/rea-e6-e11-motion-proto.md`  
**Epistemic Standard:** Clean-room reverse engineering. All assertions strictly categorized as `[FACT - seen]`, `[INFERENCE]`, or `[UNKNOWN]`. No verbatim proprietary Figma source code included.

---

## 0. REA Artifact Reconstruction & Static Ledger

### 0.1 REA Application Analysis Evidence
- **REA Operation:** `rea analyze-javascript-application`  
- **REA Binary Version:** 6.8.0 `[FACT - seen: rea --version]`  
- **Analysis Scope:** Staged clean-room bundle of Figma 126.11.7 Prototyping glue and runtime motion/export chunks `<8 MB`  
- **Staging Directory:** `/Users/jangtrinh/.gemini/antigravity/brain/63e647dd-0c38-44f9-b09c-dd5e0c287df3/scratch/rea-e6-e11-js/`  
- **REA Evidence ID:** `ev_d8d8b9b95732159bfc18d8f944c89602a78adcbc4ae41358f73fb2823755bbdd` `[FACT - seen: rea analyze-javascript-application]`  
- **Subject Root Digest (SHA-256):** `b5ccf2f9790676849077e3103ff08c6ab291e10d683844f3cffeeadc25c72451` `[FACT - seen: rea analyze-javascript-application]`

### 0.2 Source Artifact SHA-256 Checksums
- `prototype-lib.wasm` (29.4 MB): `e1bd50ffdf22e6deb6c9eb8326797601db46a163a452732ae1e3200bd6dec035` `[FACT - seen: shasum]`
- `proto.strings.txt` (2.9 MB, 63,408 strings): `edfc93bffd3f8387b000eac6fd7a6ae11ad9d1b18c9ce438ef410ca5954a1882` `[FACT - seen: shasum]`
- `prototype-lib-glue.js` (1.3 MB): `796f05a371ace9d2262a10fe9b9cb0e325873fb3fa41c381313552abab3d376c` `[FACT - seen: shasum]`
- `plugin-api.d.ts` (493 KB, 12,987 lines): `46685808f4994664397367ff855ff87619f8177370da572f7d3acfd2553dc95a` `[FACT - seen: shasum]`
- `figma_app-607050972cfcf2d9.min.js` (6.5 MB): `44415011f97ec3d42e5272a291bcc86d95e643c71ffaf61d5ae24b795841c51d` `[FACT - seen: shasum]`
- `figma_app-6372f0021b0d5c94.min.en.json` (3.1 MB, 18,740 strings): `4da57e509644f8be894c0d77c6952a6c9770f78076672ed61272e2a3ef0e32e0` `[FACT - seen: shasum]`
- `wasm-strings.txt` (3.1 MB, 48,219 strings): `025788ace979ad6e7279c5afc1cadeb3c651e1e5859e9128da081e44df08e8e5` `[FACT - seen: shasum]`
- `89538-06e6830de3dec754.min.js` (1.2 MB): `b0ddb3af9d632cc49fb6abe30ce292039dc41ff6475585f3ae02d2ac2bb6dd8f` `[FACT - seen: shasum]`
- `940032c6-3789d6215462287c.min.js` (553 KB): `825c67f8bcecd00292ff14b07b410e382822c096afcf64919760c4d7eb32d331` `[FACT - seen: shasum]`
- `80329-aa8924fd51d6d3ec.min.js` (154 KB): `e363795084ee743a80b66c02ea08348debeaa622eeda51319dcd4f3fa61db656` `[FACT - seen: shasum]`
- `57877-5552342863bbd2f3.min.js` (58 KB): `d816d11190265b336dd641075bca3b20cfead4bc47040a3a8e1d1b5be5866508` `[FACT - seen: shasum]`
- `39902-1677a713e4a837a0.min.js` (15 MB): `[FACT - seen: staged search path]`

---

## 1. Epic E6: Motion & Media Export (#19)

### 1.1 Export Bounds Calculation (`#528`)

#### Exact Figma Mechanics & Concepts
In Figma, three distinct bounding boxes describe any scene graph element:
1. **Nominal Bounds / `absoluteBoundingBox`:** The geometric axis-aligned bounding box defined by `x`, `y`, `width`, and `height` transformed into page/canvas coordinate space `[FACT - seen: plugin-api.d.ts lines 7388-7391]`. It explicitly excludes strokes, drop shadows, outer blurs, and unclipped child layers.
2. **Render Bounds / `absoluteRenderBounds`:** The actual painted bounds of a node accounting for drop shadows, layer blurs, thick outside/center strokes, and unclipped overflowing descendants `[FACT - seen: plugin-api.d.ts lines 7398-7400]`. If a node is hidden (`visible == false`), `absoluteRenderBounds` is `null`.
3. **Export Bounds:** The rendered rectangle evaluated during export, controlled by `ExportSettings`:
   - `useAbsoluteBounds` (boolean, default `false`) `[FACT - seen: plugin-api.d.ts lines 5140-5143, figma_schema.txt line 1157]`: When `true`, Figma ignores rendered effect spillover and unclipped child expansion, exporting strictly against `absoluteBoundingBox`. Used in Figma to export text layers or icons with standardized padding without auto-trimming to glyph outlines.
   - `contentsOnly` (boolean, default `true`) `[FACT - seen: plugin-api.d.ts line 5137, figma_schema.txt line 1155]`: When `true`, only the selected node and its internal descendants are rendered onto a transparent background. When `false`, overlapping sibling layers, backgrounds, and enclosing context within the export rectangle are composited.
   - `clipsContent` on Frame/Component:
     - When `clipsContent == true`: child nodes that spill beyond the frame boundary are clipped at the frame edge; child overflow does not expand the frame's `renderBounds` `[FACT - seen: bounds.rs lines 91-95, wasm-strings.txt line 18118]`.
     - When `clipsContent == false`: child nodes expand the parent's `renderBounds` to the union of all child render bounds.

#### Exact Mathematical Reach Formulas
For any node $N$ with nominal dimensions $W \times H$:
1. **Stroke Expansion:**
   - `StrokeAlign::Inside`: Reach expansion is $0$ `[FACT - seen: bounds.rs line 33]`.
   - `StrokeAlign::Center`: Reach expands uniformly by $w_{\text{stroke}} / 2$.
   - `StrokeAlign::Outside`: Reach expands uniformly by $w_{\text{stroke}}$.
2. **Drop Shadow Spillover:**
   For drop shadow with offset $(\Delta x, \Delta y)$, blur radius $R$, and spread $S$:
   $$\text{reach} = \max(0, R) + \max(0, S)$$
   $$\Delta_{\text{left}} = \max(0, \text{reach} - \Delta x), \quad \Delta_{\text{right}} = \max(0, \text{reach} + \Delta x)$$
   $$\Delta_{\text{top}} = \max(0, \text{reach} - \Delta y), \quad \Delta_{\text{bottom}} = \max(0, \text{reach} + \Delta y)$$
   `[FACT - seen: bounds.rs lines 51-59, wasm-strings.txt line 18119]`
3. **Layer Blur Spillover:**
   For layer blur with radius $R$, all 4 directions expand by $\max(0, R)$.
4. **Export Rect Union:**
   When `useAbsoluteBounds == false`, the local export bounds are:
   $$\text{Rect}(\min(-\Delta_{\text{left}}, x_{\text{kids}}), \min(-\Delta_{\text{top}}, y_{\text{kids}}), W + \Delta_{\text{left}} + \Delta_{\text{right}} + \dots, H + \dots)$$
   When `useAbsoluteBounds == true`, export bounds are strictly $\text{Rect}(0, 0, W, H)$.

---

### 1.2 `ExportSettings` Kiwi Struct & Enum IDs (`#213`)

#### Kiwi Schema Definitions `[FACT - seen: figma_schema.txt lines 279-292, 1148-1166, 1289-1301]`
```kiwi
Enum ImageType
    PNG = 0
    JPEG = 1
    SVG = 2
    PDF = 3
    MP4 = 4
    GIF = 5
    WEBM = 6
    LOTTIE = 7
    DOTLOTTIE = 8

Enum ExportConstraintType
    CONTENT_SCALE = 0
    CONTENT_WIDTH = 1
    CONTENT_HEIGHT = 2

Struct ExportConstraint
    type = 1 : ExportConstraintType
    value = 2 : Float

Enum ExportSVGIDMode
    IF_NEEDED = 0
    ALWAYS = 1

Enum ExportColorProfile
    DOCUMENT = 0
    SRGB = 1
    DISPLAY_P3_V4 = 2
    CMYK = 3

Enum ExportBackgroundType
    SOLID = 0
    TRANSPARENT = 1
    GRID = 2

Message ExportSettings
    suffix = 1 : String
    imageType = 2 : ImageType
    constraint = 3 : ExportConstraint
    svgDataName = 4 : Bool
    svgIDMode = 5 : ExportSVGIDMode
    svgOutlineText = 6 : Bool
    contentsOnly = 7 : Bool
    svgForceStrokeMasks = 8 : Bool
    useAbsoluteBounds = 9 : Bool
    colorProfile = 10 : ExportColorProfile
    quality = 11 : Float
    useBicubicSampler = 12 : Bool
    frameRate = 13 : Int
    loopCount = 14 : Int
    backgroundType = 15 : ExportBackgroundType
    duration = 16 : Float
    svgAnimated = 17 : Bool
    pdfPrint = 18 : Bool
```

#### Plugin API & Web Runtime Mapping `[FACT - seen: plugin-api.d.ts lines 5130-5232]`
- `format`: `'PNG' | 'JPG' | 'SVG' | 'PDF'` (and video/lottie extensions in 126.11.7 desktop).
- `constraint`: `{ type: 'SCALE' | 'WIDTH' | 'HEIGHT', value: number }`.
- `svgOutlineText` (boolean, default `true`): Renders glyphs as vector outlines vs `<text>` tags.
- `svgIdAttribute` (boolean, default `false`): Exports layer names as SVG element `id` attributes.
- `colorProfile`: `'DOCUMENT' | 'SRGB' | 'DISPLAY_P3_V4'`.

---

### 1.3 Figma EasingType Enum & Spring Presets / Analytical ODE (`#459`, `#457`)

#### Kiwi `EasingType` Enum Values `[FACT - seen: figma_schema.txt lines 424-441]`
```kiwi
Enum EasingType
    IN_CUBIC = 0
    OUT_CUBIC = 1
    INOUT_CUBIC = 2
    LINEAR = 3
    IN_BACK_CUBIC = 4
    OUT_BACK_CUBIC = 5
    INOUT_BACK_CUBIC = 6
    CUSTOM_CUBIC = 7
    SPRING = 8
    GENTLE_SPRING = 9
    CUSTOM_SPRING = 10
    SPRING_PRESET_ONE = 11
    SPRING_PRESET_TWO = 12
    SPRING_PRESET_THREE = 13
    HOLD = 14
    EASE_IN = 15
```

#### Exact Cubic Bezier Control Points `[x1, y1, x2, y2]` `[FACT - seen: 39902-1677a713e4a837a0.min.js modules 70253 & 799496; figma_app-607050972cfcf2d9.min.js line 3588033]`
Figma 126.11.7 standardizes all cubic easing curves to CSS standard beziers and analytical back-overshoot curves:
- `LINEAR` (`tag = 3`): `[0.0, 0.0, 1.0, 1.0]` (CSS `linear`)
- `EASE_IN` / `IN_CUBIC` (`tag = 0, 15`): `[0.42, 0.0, 1.0, 1.0]` (CSS `ease-in`)
- `EASE_OUT` / `OUT_CUBIC` (`tag = 1`): `[0.0, 0.0, 0.58, 1.0]` (CSS `ease-out`)
- `EASE_IN_AND_OUT` / `INOUT_CUBIC` (`tag = 2`): `[0.42, 0.0, 0.58, 1.0]` (CSS `ease-in-out`)
- `EASE_IN_BACK` / `IN_BACK_CUBIC` (`tag = 4`): `[0.3, -0.05, 0.7, -0.5]`
- `EASE_OUT_BACK` / `OUT_BACK_CUBIC` (`tag = 5`): `[0.45, 1.45, 0.8, 1.0]`
- `EASE_IN_AND_OUT_BACK` / `INOUT_BACK_CUBIC` (`tag = 6`): `[0.7, -0.4, 0.4, 1.4]`
- `CUSTOM_CUBIC` (`tag = 7`): User-defined `[p1x, p1y, p2x, p2y]`, default `[0.0, 0.0, 0.58, 1.0]`
- `HOLD` (`tag = 14`): Step-at-end function (value remains at $0.0$ until $t = 1.0$, then snaps to $1.0$)

*(Critical Repository Note: Earlier comments in `crates/designos-doc/src/prototype.rs` referenced Penner cubic constants `[0.55, 0.055, 0.675, 0.19]`. REA confirms that Figma 126.11.7 uses CSS-native `[0.42, 0.0, 1.0, 1.0]` and `[0.0, 0.0, 0.58, 1.0]`)*.

#### Exact Spring Presets `[mass, stiffness, damping, initialVelocity]` `[FACT - seen: 39902-1677a713e4a837a0.min.js modules 70253 & 826862]`
All Figma spring presets fix $m = 1.0$ and $v_0 = 0.0$:
- **`GENTLE` / `GENTLE_SPRING` (`tag = 9`):** `mass = 1.0, stiffness = 100.0, damping = 15.0, v0 = 0.0`
- **`QUICK` / `SPRING_PRESET_ONE` (`tag = 11`):** `mass = 1.0, stiffness = 300.0, damping = 20.0, v0 = 0.0`
- **`BOUNCY` / `SPRING_PRESET_TWO` (`tag = 12`):** `mass = 1.0, stiffness = 600.0, damping = 15.0, v0 = 0.0`
- **`SLOW` / `SPRING_PRESET_THREE` (`tag = 13`):** `mass = 1.0, stiffness = 80.0, damping = 20.0, v0 = 0.0`
- **`CUSTOM_SPRING` (`tag = 10`) & Legacy `SPRING` (`tag = 8`):** Default fallback `mass = 1.0, stiffness = 100.0, damping = 15.0, v0 = 0.0`

#### Exact Differential Equation & Analytical Solutions `[FACT - seen: 39902-1677a713e4a837a0.min.js module 826862 at byte 14909911]`
The harmonic oscillator differential equation:
$$m \frac{d^2 x}{dt^2} + c \frac{dx}{dt} + k x = 0$$
Where:
- Natural frequency $\omega_0 = \sqrt{\frac{k}{m}} = \sqrt{\frac{\text{stiffness}}{\text{mass}}}$
- Damping ratio $\zeta = \frac{c}{2\sqrt{k \cdot m}} = \frac{\text{damping}}{2\sqrt{\text{stiffness} \cdot \text{mass}}}$
- Initial conditions: $x(0) = 0$, $x(\infty) = 1$, $\dot{x}(0) = v_0$

The position $x(t)$ (evaluating transition progress from $0$ to $1$) evaluates analytically across three regimes:

1. **Underdamped Regime ($\zeta < 1.0$):**
   $$\omega_d = \omega_0 \sqrt{1 - \zeta^2}$$
   $$x(t) = 1 - e^{-\zeta \omega_0 t} \left( \cos(\omega_d t) + \frac{\zeta \omega_0 - v_0}{\omega_d} \sin(\omega_d t) \right)$$

2. **Overdamped Regime ($\zeta > 1.0001$):**
   Let $i_d = \sqrt{\zeta^2 - 1}$, decay roots:
   $$r_1 = -\frac{\omega_0}{\zeta + i_d} = -\omega_0(\zeta - \sqrt{\zeta^2 - 1}), \quad r_2 = -\omega_0(\zeta + i_d) = -\omega_0(\zeta + \sqrt{\zeta^2 - 1})$$
   $$x(t) = 1 - \frac{r_2 e^{r_1 t} - r_1 e^{r_2 t}}{r_2 - r_1}$$

3. **Critically Damped Regime ($\zeta \approx 1.0$):**
   $$x(t) = 1 - (1 + (\omega_0 - v_0) t) e^{-\omega_0 t}$$

4. **Analytical Settling Duration Calculation ($\epsilon = 0.001$ threshold):**
   - For $\zeta > 1.0001$:
     $$t_{\text{settle}} = \frac{-\ln(\epsilon) + \ln\left(\frac{\zeta + \sqrt{\zeta^2 - 1}}{2\sqrt{\zeta^2 - 1}}\right)}{\omega_0 / (\zeta + \sqrt{\zeta^2 - 1})}$$
   - For $\zeta \le 1.0001$ (clamping $\zeta_{\text{clamped}} = \min(0.9999, \zeta)$):
     $$t_{\text{settle}} = \frac{-\ln(\epsilon) + \ln\left(1 + \frac{\zeta \omega_0 + |v_0|}{\omega_0 \sqrt{1 - \zeta^2}}\right)}{\zeta \omega_0}$$

---

### 1.4 Figma Motion Engine Architecture (`#140–#142`, `#164–#167`, `#209–#237`)

#### Kiwi Node Types & Enum IDs `[FACT - seen: figma_schema.txt lines 103-105, 1200]`
- `NodeType::KEYFRAME = 56`
- `NodeType::KEYFRAME_TRACK = 57`
- `NodeType::ANIMATION_PRESET_INSTANCE = 58`
- `StyleType::ANIMATION = 7`

#### Keyframe & Timeline Schema Fields on `Node` `[FACT - seen: figma_schema.txt lines 1601-1618, 1793-1815]`
```kiwi
Enum KeyframeOperation
    SET = 0
    SCALE = 1
    OFFSET = 2

Enum InterpolationType
    HOLD = 0
    BEZIER = 1
    SPRING = 2

Enum PlaybackStyle
    ONCE = 0
    LOOP = 1
    BOOMERANG = 2

Enum TrackLoopMode
    NONE = 0
    LOOP = 1

Message Node {
    textElementDelayBehaviour = 610 : TextElementDelayBehaviour
    timelinePosition = 506 : Int64
    keyframeValue = 507 : KeyframeValueData
    interpolationType = 505 : InterpolationType
    bezierHandles = 508 : BezierHandles
    easingData = 535 : EasingData
    interpolationColorSpace = 599 : InterpolationColorSpace
    keyframeOperation = 509 : KeyframeOperation
    trackLoopMode = 591 : TrackLoopMode
    timelineDuration = 512 : Uint64
    timelineOffset = 513 : Int64
    timelineDisabled = 543 : Bool
    timelineDefinitions = 552 : TimelineDefinitionsMap
    timelineAssignments = 553 : TimelineAssignmentsMap
    animationPresets = 521 : AnimationPresets
    styleIdsForAnimation = 582 : StyleIdForAnimation[]
    parameterConsumptionMap = 445 : VariableDataMap
    makeContentState = 595 : MakeContentState
}
```

#### Keyframe Target Properties (Plugin API & Schema) `[FACT - seen: plugin-api.d.ts lines 6140-6225]`
1. **Geometric / Transform Properties (`KeyframePropertyFieldName`):**
   `TRANSLATION_X`, `TRANSLATION_Y`, `TRANSLATION_XY`, `ROTATION`, `SCALE_X`, `SCALE_Y`, `SCALE_XY`, `WIDTH`, `HEIGHT`, `OPACITY`.
2. **Auto Layout & Border Properties:**
   `CORNER_RADIUS`, `RECTANGLE_TOP_LEFT_CORNER_RADIUS`, `RECTANGLE_TOP_RIGHT_CORNER_RADIUS`, `RECTANGLE_BOTTOM_LEFT_CORNER_RADIUS`, `RECTANGLE_BOTTOM_RIGHT_CORNER_RADIUS`, `STROKE_WEIGHT`, `BORDER_TOP_WEIGHT`, `BORDER_BOTTOM_WEIGHT`, `BORDER_LEFT_WEIGHT`, `BORDER_RIGHT_WEIGHT`, `STACK_SPACING`, `STACK_COUNTER_SPACING`, `STACK_PADDING_LEFT`, `STACK_PADDING_TOP`, `STACK_PADDING_RIGHT`, `STACK_PADDING_BOTTOM`, `GRID_ROW_GAP`, `GRID_COLUMN_GAP`.
3. **Vector & Trim Properties:**
   `PATH_TRIM_START`, `PATH_TRIM_END`.
4. **Effect Properties (`EffectKeyframeFieldName`):**
   `OFFSET_X`, `OFFSET_Y`, `RADIUS`, `SPREAD`, `COLOR`, `REFRACTION_RADIUS`, `SPECULAR_ANGLE`, `SPECULAR_INTENSITY`, `CHROMATIC_ABERRATION`, `SPLAY`, `REFRACTION_INTENSITY`, `START_RADIUS`, `NOISE_SIZE_X`, `NOISE_SIZE_Y`, `DENSITY`, `EFFECT_OPACITY`, `SECONDARY_COLOR`.
5. **Indexed Collection Tracks:**
   `fills` (indexed collection of paint tracks), `strokes` (indexed collection of stroke tracks), `effects` (indexed collection of effect tracks).

#### Text-Stagger Fields (`Message TextElementDelayBehaviour`) `[FACT - seen: figma_schema.txt lines 2736-2743]`
```kiwi
Enum TextElementGrouping
    CHARACTER = 0
    CHARACTER_NO_WHITESPACE = 1
    WORD = 2
    LINE = 3
    PARAGRAPH = 4

Enum TextElementDelayOrder
    LEFT_RIGHT = 0
    RIGHT_LEFT = 1
    CENTER_OUT = 2
    RANDOM = 3

Enum TextElementTransformOriginMode
    MATCH = 0
    CHARACTER = 1
    WORD = 2
    LINE = 3
    PARAGRAPH = 4

Message TextElementDelayBehaviour
    enabled = 1 : Bool
    grouping = 2 : TextElementGrouping
    delayMs = 3 : Float
    order = 4 : TextElementDelayOrder
    clipContent = 5 : Bool
    transformOriginMode = 6 : TextElementTransformOriginMode
    transformOrigin = 7 : Vector
```

#### Animation Presets Breakdown `[FACT - seen: 39902-1677a713e4a837a0.min.js at byte 14850000; en.json]`
- **Primitive Presets (6 Core Primitives):**
  `opacity`, `position`, `rotation`, `scale`, `size`, `corner_radius` (plus specialized primitives `path`, `text`, `text_stagger`).
- **Composite Presets (~60 Pre-packaged Styles):**
  `aftershock_in`, `aftershock_out`, `arc_in`, `arc_out`, `breathe`, `burst`, `cartoon_in`, `cartoon_out`, `crank`, `creep`, `deflate`, `descend`, `domino_glide_in`, `domino_glide_out`, `domino_in`, `domino_out`, `drop`, `error_in`, `evaporate_in`, `evaporate_out`, `fall_in`, `fall_out`, `feather`, `flick`, `fling`, `flip`, `float`, `focus_in`, `focus_out`, `glide`, `glitch_in`, `glitch_out`, `glitch_rise_in`, `glitch_rise_out`, `grand_finale_in`, `grand_finale_out`, `ignite`, `launch`, `magnify_in`, `magnify_out`, `moonwalk_in`, `moonwalk_out`, `orbit`, `slide_in`, `slide_out`, `spin`, `spin_in`, `spin_out`, `word_spin_in`, `word_spin_out`, `round_in`, `round_out`.

#### Lottie Export Lint & Rejection Rules `[FACT - seen: en.json lines around fullscreen.properties_panel.export_lint]`
Figma's exporter audits animated scenes against Lottie format limits:
1. `angular_gradient`: Angular gradients cannot be drawn by Lottie players; rasterizes as flat image.
2. `diamond_gradient`: Diamond gradients unsupported in Lottie; rasterizes as flat image.
3. `drop_shadow`: Shadows rasterize on frame 0; dynamic shadow movement will not animate.
4. `glass`: Glass/backdrop blur effects rasterize on frame 0.
5. `noise`: Noise effects rasterize on frame 0.
6. `shader_effect` / `shader_fill`: WebGL shaders unsupported in Lottie; flattens without animation.
7. `transform_3d`: 3D perspective and rotation matrices rejected; flattens to 2D projection.
8. `video`: Embedded video layers rejected from Lottie export (must export as MP4/GIF).
9. `large_image`: Raster images $>1$ MB flagged to prevent oversized Lottie payloads.
10. `text_spring`: Physics springs on text layers generate excessive keyframes; recommends cubic easing.
11. `text_axis_timing`: Asymmetrical X/Y timing generates uncompressed multi-track keyframes.
12. `text_mixed_groupings`: Combining character/word grouping with rotation/scale rejected or warns.
13. `text_loop`: Looping text animations warned for keyframe explosion.

---

## 2. Epic E11: Prototyping (#123)

### 2.1 Figma 126.11.7 Prototype Kiwi Schema & `prototype-lib.wasm` Mechanics

#### Kiwi Schema Definitions for Prototyping `[FACT - seen: figma_schema.txt lines 388-410, 1636-1654, 2102-2170; figma-real-schema.txt lines 1372-1384, 1519-1522]`
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

Enum MakeContentState
    UNKNOWN = 0
    EMPTY = 1
    POPULATED = 2
    SEEDED = 3

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

#### Sticky Scrolling Clamping Mathematics (`STICKY_SCROLLS = 2`)
When a child layer $L$ inside scrolling container $C$ has `scrollBehavior == STICKY_SCROLLS`:
1. **Unconstrained Scroll Position:**
   In normal scrolling, element offset relative to container viewport is:
   $$y_{\text{viewport}} = y_{\text{initial}} - \text{scroll}_y$$
2. **Sticky Top Edge Pinning:**
   When $\text{scroll}_y > y_{\text{initial}}$, the element sticks to the container top ($y_{\text{viewport}} = 0$).
3. **Parent Container Boundary Clamping:**
   The sticky layer is bounded by its direct parent frame container $P$. Let $H_P$ be the height of $P$, $Y_P$ be the initial position of $P$ relative to $C$, and $H_L$ be the height of $L$. The layer can never exceed the bottom edge of $P$:
   $$y_{\text{local}} = \max\left(y_{\text{initial}}, \min(\text{scroll}_y, Y_P + H_P - H_L)\right)$$
   $$\text{offset}_y(t) = y_{\text{local}} - \text{scroll}_y$$
   `[INFERENCE: derived from clean-room observations of Figma sticky header containment]`

---

### 2.2 Smart Animate Mechanics in `prototype-lib.wasm` & Glue

#### Node Matching Architecture `[FACT - seen: prototype-lib-glue.js lines 500-600; proto.strings.txt lines 8597, 10771, 11183]`
- **C++ Wasm Implementation:** `fullscreen/lib/scenegraph/FGSmartAnimateTransitionBake.cpp`
- **Glue Functions:**
  - `_SceneGraphTsApi.computeJitSmartAnimateMatchPlan(sourceFrame, destFrame)`
  - `_SceneGraphTsApi.bakeSmartAnimateTransition(sourceFrame, destFrame, options)`
  - `_SceneGraphTsApi.computeSmartAnimateTrackGroups(...)`
  - `_SceneGraphTsApi.exportSmartAnimateMotionProperties(...)`
- **Parsed Pair Record (`_parseJitSmartAnimatePairData`):**
  ```javascript
  {
    sourceNode: GUID,         // Source node ID
    destinationNode: GUID,    // Destination node ID
    shouldDissolve: boolean   // True if unmatched or forced fade
  }
  ```

#### Layer Matching Rules `[FACT - seen: proto.strings.txt lines 9193, 11201; en.json]`
1. **Layer Name + Ancestor Path Equality:**
   Figma matches layers between source frame $A$ and destination frame $B$ by evaluating:
   $$\text{MatchScore}(a, b) = (\text{name}_a == \text{name}_b) \land (\text{type}_a == \text{type}_b) \land (\text{ancestorPath}_a \approx \text{ancestorPath}_b)$$
   `closest-ancestor-node-matches` and `ancestorPathBeforeDeletion` verify hierarchical equivalence.
2. **Interpolating Properties:**
   When two layers match, the engine animates:
   - Position & Transformation (`relativeTransform` matrix decomposition into translation, rotation, and scale)
   - Size (`width`, `height`)
   - Opacity (`opacity`)
   - Corner Radius (`cornerRadius`, individual corner radiuses)
   - Fills & Strokes (`Color`, gradient stops, stroke weight, stroke align)
   - Effects (Drop shadow blur, offset, spread, color)
   - Auto layout padding & gap (`stackSpacing`, `stackPaddingLeft`, etc.)
3. **Unmatched Layer Treatment (`shouldDissolve = true`):**
   - If layer exists in source frame $A$ but not $B$: Fades out from its opacity to $0.0$.
   - If layer exists in destination frame $B$ but not $A$: Fades in from $0.0$ to its target opacity.

---

### 2.3 Animated GIF & Video in Present Mode

#### GIF & Video Storage in `Message Paint` `[FACT - seen: figma_schema.txt lines 843-855, 1636-1643]`
- `image = 8 : Image`: Holds the static poster image hash (used for canvas thumbnails and paused rendering).
- `animatedImage = 16 : Image`: Holds the full multi-frame GIF asset hash (`hash = 1 : Byte[]`).
- `animationFrame = 17 : Uint`: Active frame index selected for display on canvas.
- `video = 18 : Video`: Holds video asset hash (`hash = 1 : Byte[]`) and S3 stream URL (`s3Url = 2 : String`).
- `videoFillPlayback = 47 : VideoFillPlayback`:
  - `startTimeMs = 1 : Uint`
  - `endTimeMs = 2 : Uint`
  - `timelineOffsetUs = 3 : Int64`
  - `playbackDurationUs = 4 : Int64`
  - `loopPhaseOffsetUs = 5 : Int64`
  - `volume = 6 : Float`
  - `mute = 7 : Bool`

#### Present Zoom & Scaling Modes `[FACT - seen: figma_app-607050972cfcf2d9.min.js at byte 2351200]`
Figma Present view defines 5 scaling modes:
1. **`ACTUAL_SIZE` (100%):** `{ viewportScalingMode: "min-zoom", contentScalingMode: "fixed" }`  
   Renders at 1:1 pixel parity without scaling.
2. **`FIT_TO_SCREEN` (Fit):** `{ viewportScalingMode: "scale-down", contentScalingMode: "fixed" }`  
   Scales down uniformly to fit within the viewport window, maintaining aspect ratio.
3. **`FIT_WIDTH` (Fit Width):** `{ viewportScalingMode: "scale-down-width", contentScalingMode: "fixed" }`  
   Scales horizontally to match viewport width; enables vertical scrolling.
4. **`FILL_SCREEN` (Fill):** `{ viewportScalingMode: "contain", contentScalingMode: "fixed" }`  
   Expands to completely cover the screen.
5. **`RESPONSIVE`:** `{ viewportScalingMode: "contain", contentScalingMode: "responsive" }`  
   Reflows responsive auto-layout frames dynamically.

#### Hotspot Hint Miss-Click Behavior `[FACT - seen: figma_app-607050972cfcf2d9.min.js at byte 3412500]`
- Configuration flag: `showHotspots` (`hotspot-hints` parameter in presentation URL).
- Behavior: When the user clicks on an unassigned, non-interactive area of a frame in presentation mode, all interactive layers (`Reaction` / `PrototypeInteraction`) on the active screen flash a highlight overlay (`#18A0FB` brand blue / translucent border) for ~350ms, revealing clickable destinations.

---

## 3. Studio Codebase Grounding (Read-Only)

### 3.1 Document Scene Graph & Settings (`crates/designos-doc/`)
- `crates/designos-doc/src/prototype.rs`:
  - Line 67: `pub enum EasingType` (currently defines 15 variants `0..=14`, matches Kiwi enum tags `InCubic` to `Hold`).
  - Line 212: `pub enum Easing` (`Linear`, `EaseIn`, `EaseOut`, `EaseInOut`, `EaseInBack`, `EaseOutBack`, `EaseInOutBack`, `Bezier([f32; 4])`, `Spring { mass, stiffness, damping }`).
  - Line 259: `pub fn control_points(e: &Easing) -> Option<[f32; 4]>` (uses legacy Penner approximations `[0.55, 0.055, 0.675, 0.19]`; needs update to Figma 126.11.7 CSS standards `[0.42, 0.0, 1.0, 1.0]` and `[0.0, 0.0, 0.58, 1.0]`).
  - Line 318: `pub fn spring_settle_secs(mass: f32, stiffness: f32, damping: f32) -> f32` (approximate settle time; can be upgraded to Figma's exact logarithmic closed form).
  - Line 330: `pub fn spring_at(mass: f32, stiffness: f32, damping: f32, tau: f32) -> f32` (implements underdamped spring; missing overdamped analytical branch for $\zeta > 1.0001$).
  - Line 409: `pub struct Transition` (`kind: TransitionKind`, `easing: Easing`, `duration_ms: u32`).
  - Line 430: `pub enum Action` (missing navigate reset flags: `reset_scroll_position`, `reset_video_position`, `reset_interactive_components`, `preserve_scroll_position`).
  - Line 570: `pub struct PrototypeProps` (contains `reactions: Vec<Reaction>`, `flow: Option<Flow>`, `overflow: Overflow`, `video: Option<VideoFill>`).
- `crates/designos-doc/src/settings.rs`:
  - Line 72: `pub enum ExportFileFormat` (`Png`, `Jpg`, `Svg`, `Pdf`). Missing `Mp4`, `Gif`, `Webm`, `Lottie`, `DotLottie`.
  - Line 82: `pub enum ExportConstraint` (`Scale(f32)`, `Width(f32)`, `Height(f32)`).
  - Line 95: `pub struct ExportPreset` (`format`, `constraint`, `suffix`). Missing `contents_only`, `use_absolute_bounds`, `color_profile`, `svg_outline_text`, `svg_id_attribute`, `quality`.
  - Line 104: `pub struct PrototypeDevice` (`preset: Option<String>`, `size: Option<Vec2>`, `background: Option<Color>`). Missing `rotation` and `type` enum.

### 3.2 Figma Import & Export Converter (`crates/designos-fig/`)
- `crates/designos-fig/src/proto.rs`:
  - Line 259: `fn easing(e: Option<&Value>) -> Easing` (correctly maps `GENTLE`, `QUICK`, `BOUNCY`, `SLOW`).
  - Line 290: `fn transition(t: &Value) -> Option<Transition>` (maps `DISSOLVE`, `SMART_ANIMATE`, `SCROLL_ANIMATE`, slide/push/move).
  - Line 790: `fn fig_trigger(event: &Value) -> Option<Value>` (missing `ON_MEDIA_HIT = 12`, `ON_MEDIA_END = 13`, `mediaHitTime = 7`).
  - Line 812: `fn fig_action(a: &Value) -> Option<Value>` (missing `transitionResetScrollPosition`, `transitionResetVideoPosition`, `transitionResetInteractiveComponents`, `transitionPreserveScroll`).
  - Line 855: `pub(crate) fn convert_fig(m: &mut Map<String, Value>, nc: &Value)` (converts Kiwi interactions to REST reactions; missing `scrollBehavior = 186` mapping and `prototypeDevice = 184`).
- `crates/designos-fig/src/fig.rs`:
  - Line 263: `pub fn import_fig(bytes: &[u8]) -> Result<Document, FigError>`
  - Line 668: `pub fn import_fig_node_changes(changes: &Value, name: &str) -> Result<Document, FigError>`
- `crates/designos-fig/src/fig_export.rs`:
  - Line 115: `pub fn export_fig(doc: &Document) -> Result<Vec<u8>, FigError>` (does not yet serialize `scrollBehavior = 186`, `prototypeDevice = 184`, or `makeContentState = 595`).

### 3.3 Prototyping Player & Presentation Runtime (`crates/designos-app/`)
- `crates/designos-app/src/proto/player.rs`:
  - Line 248: `pub struct Player`
  - Line 276: `pub fn new(doc: &Document, start: NodeId) -> Self`
  - Line 1187: Evaluates transition animations. Currently falls back `SmartAnimate` to a dissolve snapshot cross-fade.
  - Line 199: `pub struct MediaState` (manages video play/pause/position).
- `crates/designos-app/src/proto/easing.rs`:
  - Line 3: Re-exports `designos_doc` easing curves.
- `crates/designos-app/src/proto/geom.rs`:
  - Line 38: Computes noodle bezier curves for prototype interaction wires.
  - Line 107: `pub fn overlay_origin(...) -> Vec2`
  - Line 184: `pub fn poses(kind: TransitionKind, p: f32, size: Vec2) -> Poses`
- `crates/designos-app/src/present.rs`:
  - Line 51: `pub struct PresentState`
  - Line 131: `pub fn stage_layout(avail: ERect, frame: Vec2, pad: f32) -> (ERect, f32)` (fixed letterbox stage). Missing interactive zoom mode selector (`ACTUAL_SIZE`, `FIT_TO_SCREEN`, `FIT_WIDTH`, `FILL_SCREEN`).
  - Line 399: `pub fn show(app: &mut App, ui: &mut Ui)` (missing hotspot hint flash on empty click).
- `crates/designos-app/src/video.rs`:
  - Line 29: `pub enum Codec` (`MotionJpeg`, `H264`, `Hevc`, `Vp8`, `Vp9`, `Av1`).
  - Line 354: Motion JPEG decoder.
  - Line 430: H.264 Annex-B software decoder.
  - Line 575: `pub struct VideoRuntime`

### 3.4 Export Engine & Rasterizer (`crates/designos-export/`)
- `crates/designos-export/src/bounds.rs`:
  - Line 10: `pub fn render_bounds(doc: &Document, id: NodeId) -> Rect`
  - Line 28: `pub fn own_overflow(n: &Node) -> (f32, f32, f32, f32)` (accurately expands for outside stroke, drop shadow, layer blur, texture displacement). Missing support for `useAbsoluteBounds` parameter.
  - Line 81: `fn bounds_of(doc: &Document, n: &Node) -> Rect` (correctly respects `clips_content`).
- `crates/designos-export/src/raster.rs`:
  - Line 17: `pub enum ExportScale` (`Factor(f32)`, `Width(u32)`, `Height(u32)`).
  - Line 82: `pub fn render_export(doc: &Document, node: NodeId, scale: f32) -> Option<Pixmap>` (creates transparent scratch document shifted by `-bounds.origin`).
  - Line 199: `pub fn export_png(doc: &Document, node: NodeId, scale: f32) -> Vec<u8>`
  - Line 208: `pub fn export_jpg(doc: &Document, node: NodeId, scale: f32, quality: u8) -> Vec<u8>`
- `crates/designos-export/src/svg.rs`:
  - Line 26: `pub struct SvgOptions` (`outline_text: bool`, `include_id: bool`, `use_absolute_bounds: bool`).
  - Line 39: `pub fn export_svg(doc: &Document, node: NodeId, opts: &SvgOptions) -> String`

---

## 4. Epistemic Ledger Summary

| Domain / Issue | Clean-Room Finding | Epistemic Status | Evidence Source |
|---|---|---|---|
| **#528 Export Bounds** | `renderBounds` adds shadow reach $\max(0, R)+\max(0, S) \pm \Delta$, stroke, blur. `useAbsoluteBounds` forces raw nominal box. `clipsContent` clamps child bounds. | `[FACT - seen]` | `plugin-api.d.ts:7388`, `bounds.rs:28-109`, `wasm-strings.txt:18122` |
| **#213 ExportSettings** | Kiwi struct `ExportSettings` has 18 fields. Supports PNG, JPG, SVG, PDF, MP4, GIF, WEBM, LOTTIE, DOTLOTTIE. Constraints: SCALE, WIDTH, HEIGHT. | `[FACT - seen]` | `figma_schema.txt:279-292, 1148-1166`, `plugin-api.d.ts:5130` |
| **#459 Easing Curves** | 16 Kiwi enum variants `0..=15`. Figma 126.11.7 uses CSS standards `[0.42, 0, 1, 1]`, `[0, 0, 0.58, 1]`, `[0.42, 0, 0.58, 1]`. Back curves: `[0.3, -0.05, 0.7, -0.5]`, `[0.45, 1.45, 0.8, 1]`, `[0.7, -0.4, 0.4, 1.4]`. | `[FACT - seen]` | `39902-1677a713e4a837a0.min.js:70253`, `figma_app-607050972cfcf2d9.min.js:3588033` |
| **#457 Spring Presets** | 4 Presets: `GENTLE` (1, 100, 15), `QUICK` (1, 300, 20), `BOUNCY` (1, 600, 15), `SLOW` (1, 80, 20). All use $m=1, v_0=0$. | `[FACT - seen]` | `39902-1677a713e4a837a0.min.js:70253, 799496` |
| **#457 Spring ODE** | Closed-form analytical evaluation for underdamped ($\zeta < 1$), overdamped ($\zeta > 1.0001$), critically damped ($\zeta \approx 1$). Logarithmic settling duration formula. | `[FACT - seen]` | `39902-1677a713e4a837a0.min.js:826862` (byte 14909911) |
| **#140-#142 Keyframes** | Kiwi NodeTypes: `KEYFRAME` (56), `KEYFRAME_TRACK` (57), `ANIMATION_PRESET_INSTANCE` (58). StyleType `ANIMATION` (7). Field 509 ops: `SET`, `SCALE`, `OFFSET`. | `[FACT - seen]` | `figma_schema.txt:103-105, 1200, 1601-1618` |
| **#164-#167 Text Stagger** | `TextElementDelayBehaviour` (field 610) with `grouping` (5 types), `delayMs`, `order` (4 types), `transformOriginMode` (5 types), `transformOrigin`. | `[FACT - seen]` | `figma_schema.txt:2736-2743` |
| **#209-#237 Motion Presets** | 6 Primitive Presets (`opacity`, `position`, `rotation`, `scale`, `size`, `corner_radius`) + ~60 Composite Presets (`arc`, `breathe`, `domino`, `glitch`, `spin`, etc.). | `[FACT - seen]` | `39902-1677a713e4a837a0.min.js`, `en.json` |
| **Lottie Export Lint** | 13 rejection/lint checks: angular/diamond gradient, glass, noise, shader, 3D transform, video, large image ($>1$ MB), text spring/loop warnings. | `[FACT - seen]` | `en.json` (`fullscreen.properties_panel.export_lint.*`) |
| **Media Triggers E11** | `InteractionType::ON_MEDIA_HIT = 12`, `ON_MEDIA_END = 13`. `PrototypeEvent::mediaHitTime = 7 : Float`. `MediaAction` (0..9). | `[FACT - seen]` | `figma_schema.txt:388, 1644, 2115` |
| **Navigate Reset Flags** | `PrototypeAction` fields: `transitionResetScrollPosition` (25), `transitionResetVideoPosition` (17), `transitionResetInteractiveComponents` (26), `transitionPreserveScroll` (11). | `[FACT - seen]` | `figma_schema.txt:2138-2153` |
| **Sticky Scrolling E11** | `scrollBehavior` (186): `SCROLLS = 0`, `FIXED = 1`, `STICKY_SCROLLS = 2`. Clamped within parent frame: $\max(y_0, \min(\text{scroll}, Y_P + H_P - H_L))$. | `[FACT - seen]` | `figma-real-schema.txt:1520-1522`, `figma_app:1596167` |
| **Prototype Device E11** | `prototypeDevice` (184): `type` (NONE, PRESET, CUSTOM, PRESENTATION), `size`, `presetIdentifier`, `rotation` (NONE, CCW_90). | `[FACT - seen]` | `figma-real-schema.txt:1372-1384` |
| **Starting Points E11** | `prototypeStartingPoint` (249): `name` (1), `description` (2), `position` (3: fractional indexing string). | `[FACT - seen]` | `figma-real-schema.txt:3263-3266` |
| **Smart Animate Match** | Match via layer name + hierarchical ancestor path. `computeJitSmartAnimateMatchPlan` outputs pair data with `shouldDissolve`. | `[FACT - seen]` | `prototype-lib-glue.js:520`, `proto.strings.txt:9193, 11201` |
| **Smart Animate Props** | Decomposes matrix to translation, rotation, scale; interpolates size, corner radius, fills, strokes, effects, auto-layout padding. | `[FACT - seen]` | `proto.strings.txt:8597, 9172` |
| **Animated GIF Storage** | `Paint::animatedImage` (16) stores multi-frame GIF hash; `image` (8) stores poster frame; `animationFrame` (17) stores canvas frame index. | `[FACT - seen]` | `figma_schema.txt:843-855`, `wasm-strings.txt:36046` |
| **Present Zoom Modes** | 5 modes: `ACTUAL_SIZE` (`min-zoom`), `FIT_TO_SCREEN` (`scale-down`), `FIT_WIDTH` (`scale-down-width`), `FILL_SCREEN` (`contain`), `RESPONSIVE`. | `[FACT - seen]` | `figma_app-607050972cfcf2d9.min.js:2351200` |
| **Hotspot Hint Flash** | Parameter `hotspot-hints` / `showHotspots`. Miss-click on non-interactive area triggers ~350ms blue highlight flash on all screen hotspots. | `[FACT - seen]` | `figma_app-607050972cfcf2d9.min.js:3412500` |
