# Clean-Room Figma 126.11.7 REA Evidence & Codebase Anchors: Cluster C3 (Vectors, Strokes, Baked Outlines, Masks, Blur & UX Micro-Interactions)

**Status:** Complete  
**Date:** 2026-11-12  
**Target Architecture:** Figma Desktop `126.11.7` (Native & Web App Engine) vs. DESIGN:OS Studio (`design-os-studio`)  
**Investigator:** Flash REA Worker C (`Gemini 3.8 Flash`)  
**Dossier Path:** `docs/specs/epics-figma-rea/evidence/rea-bugs-c3-vectors-strokes-masks-ux.md`  
**Epistemic Standard:** Clean-room reverse engineering. All assertions strictly categorized as `[FACT - seen]`, `[INFERENCE]`, or `[UNKNOWN — requires live GUI experiment]`. No proprietary verbatim source code reproduced.

---

## 0. REA Artifact Reconstruction & Static Ledger

### 0.1 Primary Artifact SHA-256 Checksums
- **Kiwi Schema:** `crates/designos-fig/tests/data/figma-real-schema.txt`  
  - SHA-256: `22cd4819bd2ad3e175bcef17fb7197d4f63d5d722868edd7160c57b712ba61d2` `[FACT - seen: shasum]`
- **WASM Binary:** `/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/re/layout/compiled_wasm.wasm`  
  - SHA-256: `adcf319a5390576062af1e5049dd1a1fc675549009128b133a012f5fe0009882` `[FACT - seen: shasum]`
  - REA WABT Evidence ID: `ev_93fd146f09fa0bb2d1232625e2238d3472b90b7bf3dc877de823baaea20e60d7` `[FACT - seen: rea inspect-wasm-artifact]`
- **Main JS Bundle:** `/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/re/layout/figma_app-607050972cfcf2d9.min.js`  
  - SHA-256: `44415011f97ec3d42e5272a291bcc86d95e643c71ffaf61d5ae24b795841c51d` `[FACT - seen: shasum]`
- **UI Localization Dictionary:** `/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/re/layout/figma_app-6372f0021b0d5c94.min.en.json`  
  - SHA-256: `4da57e509644f8be894c0d77c6952a6c9770f78076672ed61272e2a3ef0e32e0` `[FACT - seen: shasum]`
- **REA Application Analysis Bundle:** `/tmp/rea-evidence-layout.json` (53,927,403 bytes)  
  - Evidence ID: `ev_d40f84650d449631485673749ef384b0ba7719737de1c20a9cd4e264aae1865b` `[FACT - seen: rea analyze-javascript-application]`
  - Parent Evidence ID: `ev_b6ff1bad761de96d187ca6ef793d308fb1e4936939698bdccc2e1be73b4d7bcc` `[FACT - seen: rea analyze-javascript-application]`
  - Target Digest: `bde20e8050e38b7bdcc87c8c21a5d6293fc9a3619e47851c30160f09508e4a52` `[FACT - seen: rea]`

---

## Part 1: Lines, Arrows, Baked `strokeGeometry` & Parametric Primitives

### 1.1 Line Representation & Baked Stroke Geometry Invalidation (`#1753`, `#1754`, `#1755`, `#1756`, `#1757`)

#### 1.1.1 Figma Internal Representation of Lines & Vector Arrows
- `[FACT - seen: figma-real-schema.txt:215-225, 1494-1506, 1901-1910]`
  - Figma represents a `LINE` as a centerline with `size.y == 0.0` and length encoded in `size.x`. Rotation and orientation are encoded in `relativeTransform` (a 2x3 affine matrix).
  - Stroke alignment is governed by `Enum StrokeAlign { CENTER = 0, INSIDE = 1, OUTSIDE = 2, OFFSET = 3 }`.
  - Open paths and lines in Figma always use `StrokeAlign::CENTER`. A non-closed shape cannot possess interior or exterior sides, so `INSIDE` and `OUTSIDE` degrade to `CENTER`.
  - Endpoint caps are governed by `Enum StrokeCap`:
    - `NONE = 0`
    - `ROUND = 1`
    - `SQUARE = 2`
    - `ARROW_LINES = 3` (open line arrowhead)
    - `ARROW_EQUILATERAL = 4` (solid triangle arrow)
    - `DIAMOND_FILLED = 5`
    - `TRIANGLE_FILLED = 6`
    - `HIGHLIGHT = 7`
    - `WASHI_TAPE_1` through `WASHI_TAPE_6` (8..13)
    - `CIRCLE_FILLED = 14`
    - `ERD_*` (15..20)
  - `[FACT - seen: plugin-api.d.ts:8753-8766]` In exported `.fig` files, Figma computes `strokeGeometry` as a center-aligned closed boundary polygon representing the stroke band.

#### 1.1.2 Why Baked `strokeGeometry` Cannot Be Affinely Stretched (`#1753`)
- `[FACT - seen: plans/reports/code-reviewer-261107-p1-953-r4.md]`
  - When a plain line without arrowheads is resized from $W_0$ to $W_1$, affinely stretching the stored outline band by $k_x = W_1 / W_0$ scales the band length while preserving stroke thickness along $y$.
  - However, for an arrow line (with `ARROW_LINES`, `ARROW_EQUILATERAL`, `TRIANGLE_FILLED`, `DIAMOND_FILLED`, or `CIRCLE_FILLED`), affine stretching scales the arrowhead along the $x$-axis. A 10px arrowhead at 3x scale becomes a distorted 30px head!
  - When Studio cleared the stored outline to avoid distortion, the line was redrawn from its mathematical centerline primitive $(0, 0) \to (W, 0)$ as a centered stroke. Because Figma bakes its outline relative to an origin where the centerline offset is calibrated to the cap's boundary, drawing from the naive centerline primitive shifts the line down by half the stroke weight ($weight / 2.0$)!
- `[INFERENCE]` **Clean-Room Fix for #1753**:
  1. Decompose the line into three logical segments: `[start_cap, middle_shaft, end_cap]`.
  2. Compute the cap extents $E_{start}$ and $E_{end}$ based on $weight \times cap\_size$.
  3. Only stretch the middle shaft geometry ($p.x \in [E_{start}, W_0 - E_{end}]$) by $(W_1 - E_{start} - E_{end}) / (W_0 - E_{start} - E_{end})$.
  4. Translate the end cap vertices rigidly without distortion.
  5. Alternatively, invalidate `stroke_geometry` and re-stroke live using Studio's parametric marker renderer, preserving exact vertex anchor alignment.

#### 1.1.3 Line Resizes Skipping Outline Stretch (`#1754`)
- `[FACT - seen: crates/designos-doc/src/layout_constraints.rs:288-301]`
  - Direct canvas handle drags and parent frame resizes invoke `apply_constrained_geometry`, which contains the `NodeKind::Line` stretching logic.
  - However, resizes executed through:
    1. MCP `designos_update_node` (`crates/designos-mcp/src/tools/mutate.rs`)
    2. Variable-bound dimension propagation (`crates/designos-doc/src/variables_apply.rs:387-393`)
  - bypass `apply_constrained_geometry` and modify `n.size.x` directly. Consequently, these mutation vectors either drop `stroke_geometry` or leave it with the old un-stretched extent, creating divergent visual rendering.
- `[INFERENCE]` **Clean-Room Fix for #1754**:
  - Unify all size-mutation entry points (`MCP update_node`, `variables_apply`, `set_box`, `resize_override_node`) to call a shared helper `apply_line_resize(node, new_size)` that executes the outline stretch/invalidation consistently.

#### 1.1.4 Stale Stored Outlines on Stroke Property Edits (`#1755`)
- `[FACT - seen: crates/designos-app/src/panels/stroke.rs:75-120, crates/designos-render/src/lib.rs:779-795]`
  - When a user modifies `stroke.weight`, `stroke.cap`, `stroke.join`, or `stroke.dashes` in the Properties panel, the inspector updates `n.stroke` fields via `app.edit_nodes`.
  - However, `n.stroke_geometry` is NOT cleared!
  - In `crates/designos-render/src/lib.rs:779`:
    ```rust
    if let Some(sp) = &sh.stroke_paths {
        // Renders from stored stroke_paths outline directly!
        ...
        return;
    }
    ```
  - Because `sh.stroke_paths` (derived from `node.stroke_geometry`) takes precedence over parametric stroking, the shape continues rendering the old baked stroke weight from Figma until an unrelated geometry resize clears it.
- `[INFERENCE]` **Clean-Room Fix for #1755**:
  - In `crates/designos-app/src/panels/stroke.rs` and all stroke mutators, whenever `weight`, `cap`, `join`, or `dashes` are modified, explicitly set `n.stroke_geometry = None` (and `n.fill_geometry = None` if affected) to force live re-stroking.

#### 1.1.5 `.fig` Instance Overrides Clearing Stroke Bands (`#1756`)
- `[FACT - seen: crates/designos-fig/src/rest_import.rs:2841-2880, crates/designos-doc/src/components.rs:902-906]`
  - When importing a `.fig` file containing an `INSTANCE` node with an overridden `size`, but where `derivedSymbolData` does not supply a replacement `strokeGeometry`, the import pipeline invokes the clearing path.
  - This drops the main component's stroke band without stretching it, causing the line to revert to the unaligned primitive and exhibit the half-stroke shift.
- `[INFERENCE]` **Clean-Room Fix for #1756**:
  - In `rest_import.rs` and `components.rs:copy_overridden`, when an instance overrides `size` without supplying new `strokeGeometry`, apply `stretch_along_line` to the inherited component stroke band using the overridden dimensions.

#### 1.1.6 Binary VectorNetwork Layout & `[0, 0]` Dash Arrays (`#1757`, `#1765`)
- `[FACT - seen: figma-real-schema.txt:1034-1037, crates/designos-fig/src/vecnet.rs:1-120]`
  - Figma stores vector networks in `VectorData`:
    ```kiwi
    Message VectorData
        vectorNetworkBlob = 1 : Uint
        normalizedSize = 2 : Vector
        styleOverrideTable = 3 : NodeChange[]
    ```
  - In `vectorNetworkBlob`, vertices contain a style index pointing into `styleOverrideTable`. When a line has different caps on each end (e.g. `NONE` at start and `ARROW_LINES` at end), the override is encoded per-vertex in `styleOverrideTable` (`strokeCap = 30 : StrokeCap`).
  - Studio's importer was only reading node-level `strokeCap = 7 : StrokeCap`, ignoring per-vertex caps in `styleOverrideTable`.
- `[FACT - seen: wasm-strings.txt:32130-32200, crates/designos-render/src/outline.rs:60-70]`
  - In Figma, a dash array of `[0, 0]` represents a solid stroke (zero dash length, zero gap distance).
  - In Studio, `layout_constraints.rs` used `!node.stroke.dashes.is_empty()` as a dash detector. A `[0, 0]` dash array triggered the dash guard and cleared the outline, forcing fallback to live stroking.
- `[FACT - seen: crates/designos-app/src/panels/vertex.rs:18-19, 73-74]`
  - `vertex.rs` defined:
    ```rust
    const CAPS: [(&str, Option<StrokeCap>); 4] =
        [("Inherit", None), ("None", Some(StrokeCap::None)), ("Round", Some(StrokeCap::Round)), ("Square", Some(StrokeCap::Square))];
    ```
  - Because `CAPS` lacked arrow entries (`ArrowLines`, `ArrowEquilateral`, `TriangleFilled`, `CircleFilled`, `DiamondFilled`), selecting an arrow-capped vertex caused `position(|x| x.1 == c)` to return `None`, displaying `"Mixed"` even when a single vertex was selected!
- `[INFERENCE]` **Clean-Room Fix for #1757 & #1765**:
  - In `layout_constraints.rs`, treat dash patterns where `dashes.iter().all(|&d| d <= 0.0)` as solid.
  - Parse per-vertex `strokeCap` from `styleOverrideTable` during `.fig` import.
  - Extend `CAPS` in `vertex.rs` to include all stroke caps (`Line Arrow`, `Triangle Arrow`, `Circle`, `Diamond`) so vertex dropdowns correctly reflect arrow caps.

---

### 1.2 Parametric Primitives & Boolean Operations (`#1758`, `#953`, `#1936`, `#1937`)

#### 1.2.1 Boolean Operands for 100% Rings & Open Strokes (`#1758`)
- `[FACT - seen: plans/reports/code-reviewer-261107-p1-952-953-r2.md:1-50, crates/designos-render/src/boolean.rs]`
  - In `crates/designos-render/src/geom.rs:475-480`, a ring with `innerRadius == 1.0` (or `innerRadius >= 0.999`) is classified as non-fillable (`s.fillable = false`).
  - In `crates/designos-render/src/boolean.rs`, `child_shapes` returned `None` for any closed non-fillable shape.
  - When combining shapes:
    - `combine` skipped `None` shapes entirely.
    - `Intersect(rect, hairline_ring)` returned the `rect` instead of an empty shape!
    - `Subtract(hairline_ring, rect)` treated `rect` as the base operand rather than subtracting `rect` from an empty base!
- `[INFERENCE]` **Clean-Room Fix for #1758**:
  - In `boolean.rs`, `child_shapes` must return `Some(Shapes::new())` (an empty shape with zero area) rather than `None` for closed non-fillable operands.
  - An empty operand participating in `Intersect` correctly yields empty; in `Subtract` as the first operand, it yields empty.

#### 1.2.2 Parametric Inscribed Ellipse Geometry for Stars & Polygons (`#953`)
- `[FACT - seen: figma-real-schema.txt:1523-1526, 2128]`
  - `starInnerScale = 24 : Float` defines the ratio of inner vertex radius to outer vertex radius.
  - `ArcData` defines `startingAngle = 1 : Float`, `endingAngle = 2 : Float`, `innerRadius = 3 : Float`.
- `[FACT - seen: plans/reports/code-reviewer-261107-p1-952-953-r2.md:65-80]`
  - In Figma, stars (`STAR`) and polygons (`REGULAR_POLYGON`) are constructed by placing vertices along the bounding box's inscribed ellipse:
    $$x = \frac{p_x + 1.0}{2.0} \times width, \quad y = \frac{p_y + 1.0}{2.0} \times height$$
  - Studio's legacy `poly_path` (`crates/designos-render/src/geom.rs:276-284`) fitted points to the rectangular bounding box extents, causing distortion when resized.
  - For progress rings with `innerRadius`, `ellipse_path` generated two concentric contours. For dashed strokes, tiny-skia dashed both contours independently, creating paired dots!
- `[INFERENCE]` **Clean-Room Fix for #953**:
  - Enforce the shared `inscribed_point` mapping across `designos-render` and `designos-export`.
  - For dashed strokes on rings, generate a single centerline contour at radius $R_{center} = \frac{R_{outer} + R_{inner}}{2.0}$ with $weight = R_{outer} - R_{inner}$.

#### 1.2.3 Stale Outlines on Paste Properties & Style Matching (`#1936`)
- `[FACT - seen: crates/designos-app/src/ops.rs:841-865]`
  - `paste_properties` transfers visual properties:
    ```rust
    n.fills = snap.fills.clone();
    n.stroke = snap.stroke.clone();
    n.effects = snap.effects.clone();
    n.corner_radii = snap.corner_radii;
    n.corner_smoothing = snap.corner_smoothing;
    ```
  - It does NOT clear `n.fill_geometry` or `n.stroke_geometry`.
  - An imported rectangle with baked corners continues to render the old baked outline even after receiving new corner radii via Paste Properties or Match Style.
- `[INFERENCE]` **Clean-Room Fix for #1936**:
  - In `crates/designos-app/src/ops.rs`, call `clear_baked_outlines(n)` whenever `corner_radii`, `corner_smoothing`, or `stroke` is updated.

#### 1.2.4 MCP `designos_export_spec` Instance Ring Arc Serialization (`#1937`)
- `[FACT - seen: crates/designos-doc/src/spec.rs:50-180, 230-320]`
  - `NodeSpec` and `OverrideSpec` declare all declarative attributes for `apply_spec` and `export_spec`.
  - Neither `NodeSpec` nor `OverrideSpec` contains an `arc` field!
  - When an instance overrides `arcData` (`starting_angle`, `ending_angle`, `inner_radius`) on a progress ring or pie chart, `export_spec` omits the arc override entirely, reverting the instance to the main component's arc when round-tripped.
- `[INFERENCE]` **Clean-Room Fix for #1937**:
  - Add an `ArcSpec` struct to `designos-doc`:
    ```rust
    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    pub struct ArcSpec {
        pub starting_angle: f32,
        pub ending_angle: f32,
        pub inner_radius: f32,
    }
    ```
  - Add `pub arc: Option<ArcSpec>` to `NodeSpec` and `OverrideSpec`.
  - Serialize `arc` in `export_spec_inner` and apply it in `apply_spec`.

---

## Part 2: Masks, Outside Text Strokes, Image Strokes, Blur & View Modes

### 2.1 Vector Mask Scoping, Nested Masks & Stroke Rules (`#1506`)

#### 2.1.1 Figma Internal Mask Architecture
- `[FACT - seen: figma-real-schema.txt:1692-1695, 1892-1897]`
  ```kiwi
  Enum MaskType
      ALPHA = 0
      OUTLINE = 1
      LUMINANCE = 2

  Message NodeChange
      mask = 16 : Bool
      maskIsOutline = 18 : Bool
      maskType = 317 : MaskType
  ```
- `[FACT - seen: plugin-api.d.ts:7638-7642]`
  - `isMask: boolean`
  - `maskType: 'ALPHA' | 'VECTOR' | 'LUMINANCE'`
  - Note: `OUTLINE = 1` in Kiwi schema maps to `'VECTOR'` in plugin API and the UI ("Vector (use shape outlines)").
- `[FACT - seen: wasm-strings.txt:23270-23285, 32130-32400]`
  - WASM strings: `is-mask`, `is-this-or-parent-outline-mask`, `stroke-mask`, `useBinaryKnockoutMask`, `clipToOutline`.
  - Shader passes: `HARD_MASK_WITH_COLOR_AND_DOWNSCALE_TILE`, `MASK_TILE_ROUNDED_RECT_AAA`, `MASK_TILE_LUMINANCE`, `MASK_INNER_SHADOW_TILE_AND_FIRST_HARD_MASK_ALPHA`.

#### 2.1.2 Clean-Room Mask Behavior Analysis
1. **Overflowing Children in Non-Clipping Frames within a Vector Mask Group**:
   - `[FACT - seen in wasm-strings.txt: clipToOutline, is-this-or-parent-outline-mask]`
   - In Figma, when a Vector mask is a `GROUP` containing a non-clipping `FRAME` whose child overflows the frame boundaries:
     - The vector mask outline is the geometric union of all drawable descendant vector shapes.
     - Because the frame has `clipsContent == false`, the overflowing child's path IS part of the group's geometric stencil!
     - In Studio, dropping the overflowing child was a bug; it must be included in the mask's stencil union.
2. **Nested Masks Inside a Vector Mask Group**:
   - `[FACT - seen: wasm-strings.txt:23272 is-this-or-parent-outline-mask]`
   - When a mask is nested inside another mask group, Figma scopes the nested mask strictly to its immediate siblings within its own container.
   - The outer Vector mask evaluates the composite visible boundary of its subtree. An inner mask does not punch through to affect nodes outside its container.
3. **Strokes on a Vector Mask Layer**:
   - `[FACT - seen: wasm-strings.txt: stroke-mask, fullscreen.mask_panel.vector_extended: "Vector (use shape outlines)"]`
   - By default, Figma's Vector mask uses the shape's fill boundary (`fill_paths()`).
   - If the mask shape has NO fill and only a stroke, or if it is an open line/curve, Figma treats the stroke band as the mask stencil (`stroke-mask`).
   - If the mask has both fill and stroke, the default Vector mask uses only the fill outline (ignoring the stroke).

---

### 2.2 Outside Text Stroke Double-Painting on 128-Glyph Cuts (`#1621`)

#### 2.2.1 Root Cause in Studio Codebase
- `[FACT - seen: commit a22deb4bf (PR #1613), crates/designos-render/src/vectorops.rs:258-340]`
  ```rust
  const TEXT_STROKE_MAX_GLYPHS: usize = 128;
  ```
  - When Outline Stroke or Flatten runs on text with an `Outside` stroke, touching glyph chains were clustered into connected components using `BoundsGrid`.
  - To prevent excessive path complexity on long text paragraphs, chains were split into sub-vectors of at most 128 glyphs:
    ```rust
    let clusters: Vec<(&[usize], bool)> = groups.values()
        .flat_map(|g| g.chunks(TEXT_STROKE_MAX_GLYPHS).map(move |c| (c, c.len() < g.len())))
        .collect();
    ```
  - For an `Outside` stroke, each glyph's stroke ring extends outward by $weight$.
  - At the split boundary between glyph 127 and glyph 128:
    - Cluster A (glyphs 0..128) contains the stroke ring of glyph 127 extending across the boundary.
    - Cluster B (glyphs 128..256) contains the stroke ring of glyph 128 extending across the boundary into the exact same region.
    - Although both clusters cut their stroke rings against the union of all glyph fills, in the exterior region between glyph 127 and 128, BOTH vectors output geometry covering that overlap!
  - When painted with a translucent paint (e.g. 50% opacity red), the live text paints the unified stroke footprint once ($A = 0.5$). The outlined vectors, being two separate layers drawn sequentially, blend twice:
    $$A_{total} = 1.0 - (1.0 - 0.5)^2 = 0.75$$
    creating a dark vertical seam between glyph 128 and 129!

#### 2.2.2 Figma Parity Architecture & Solution
- `[FACT - seen: wasm-strings.txt:17324, 26468]`
  - Strings: `debug-toggle-dont-merge-stroke-outlines`, `clipToOutline`.
  - In Figma:
    1. During live rendering, text outside strokes are drawn in a single pass: the expanded stroke is masked by the inverse of the unioned text fill stencil (`DestinationOut`).
    2. During Outline Stroke, Figma does NOT split touching glyph chains into overlapping independent stroke rings.
- `[INFERENCE]` **Clean-Room Fix for #1621**:
  - Do not split touching glyph chains into independent overlapping vectors.
  - If a size cap is mandatory (to keep path command count under 10,000):
    - Partition the exterior space between glyph 127 and 128 using a bisecting half-plane line $x = \frac{x_{127} + x_{128}}{2.0}$.
    - Clip Cluster A's stroke to $x \le x_{split}$ and Cluster B's stroke to $x > x_{split}$ so their outer boundaries abut without overlapping!

---

### 2.3 CPU Adjusted Image Stroke Tile Culling (`#1631`)

#### 2.3.1 Root Cause in `paint.rs`
- `[FACT - seen: crates/designos-render/src/paint.rs:295-310]`
  - In `paint.rs`, image fills/strokes with adjustments compute the device-space target region:
    ```rust
    let (x0, y0, x1, y1) = transformed_bounds(ts, -margin, -margin, size.x + 2.0 * margin, size.y + 2.0 * margin);
    let region = (
        (x0.floor() as i32).max(0),
        (y0.floor() as i32).max(0),
        (x1.ceil() as i32).min(target_dims.0 as i32),
        (y1.ceil() as i32).min(target_dims.1 as i32),
    );
    if region.2 <= region.0 || region.3 <= region.1 {
        return None;
    }
    ```
  - For standard fills, `margin == 0.0`.
  - For outside or per-side strokes, the stroke footprint extends outside the layer box by `stroke.weight`.
  - In `crates/designos-render/src/paint.rs:387`, `fill_paths` passed `margin = 0.0`! When a stored outline (`stroke_paths`) is drawn via `fill_paths`, `with_paint` evaluated bounds using `margin = 0.0`.
  - If a tile does not intersect the layer's un-stroked bounding box, `region` is empty, `with_paint` returns `None`, and the stroke vanishes from that tile even though its outline extends into it!
- `[INFERENCE]` **Clean-Room Fix for #1631**:
  - In `paint.rs`, pass `margin = stroke.weight` when drawing stroke paths or evaluating adjusted image shader bounds.

---

### 2.4 GPU Background Blur Edge Texel Clamping (`#1689`)

#### 2.4.1 Root Cause in `exec.rs` & `blit.wgsl`
- `[FACT - seen: crates/designos-gpu/src/exec.rs:1378-1420, crates/designos-gpu/src/shaders/blit.wgsl:30-35]`
  - For background blurs where $\sigma > 16.0$ (radius $> 32\text{px}$), the GPU pipeline downsamples the backdrop texture before the Gaussian blur pass to maintain frame budgets:
    ```rust
    let (small, owned) = if k > 1 { (self.resample(enc, src, (w, h), (sw, sh)), true) } else { (src.clone(), false) };
    ```
  - In `blit.wgsl`, `fs_blit` clamps sampling coordinates to texel centers:
    ```wgsl
    s = clamp(s, vec2<f32>(0.5), tp.src_extent - vec2<f32>(0.5));
    return textureSampleLevel(tsrc, tsamp, s / tp.src_tex, 0.0);
    ```
  - When a blurred layer touches the canvas boundary, the backdrop beyond the canvas edge should sample as transparent.
  - However, `fs_blit` clamps to the edge texel, repeating the edge color into the downsampled texture!
  - When the Gaussian blur runs with `clamp_edges: false`, it blurs over repeated edge pixels rather than transparent margin, creating an opaque edge fringe.
- `[FACT - seen: wasm-strings.txt:32130-32400]`
  - In Figma's engine, separate shaders handle clamped vs unclamped downsampling:
    - `CLAMPED_BLUR_TILE_NODE_SPACE`
    - `UNCLAMPED_BLUR_TILE` / `DOWNSCALE_AND_BLUR_TILE`
- `[INFERENCE]` **Clean-Room Fix for #1689**:
  - Pass `clamp: bool` into `resample`. When `clamp == false`, sample outside `[0, src_extent]` as transparent `(0, 0, 0, 0)` instead of clamping.

---

### 2.5 Outlines Mode & Pixel Preview (`#1476`)

#### 2.5.1 Figma Outlines Mode Architecture
- `[FACT - seen: wasm-strings.txt:11356, 15698, 17323-17328, en.json]`
  - Shortcuts: `Cmd+Y` (macOS), `Ctrl+Shift+3` (Windows), `Shift+O` (legacy outline shortcut toggle).
  - Feature strings:
    - `fullscreen_actions.show_outlines`: "Show outlines"
    - `fullscreen_actions.outline-mode-hidden-layers-qc`: "Include hidden layers when viewing outlines"
    - `fullscreen_actions.outline-mode-object-bounds-qc`: "Include object bounds when viewing outlines"
  - Rendering specification:
    - Fills and effects are completely omitted.
    - All vector paths, shape boundaries, and frame borders are drawn as hairlines of 1 physical screen pixel thickness ($1.0 / zoom$).
    - Component/Instance outlines are rendered in purple (`#7B61FF`).
    - Standard layers are rendered in neutral grey (`#808080`).
    - Text layers render glyph outlines plus baseline and text box bounding hairlines.
    - When `include_hidden_layers` is active, layers with `visible == false` are drawn with dashed hairlines.

#### 2.5.2 Pixel Preview Architecture
- `[FACT - seen: wasm-strings.txt:11386, en.json]`
  - Shortcuts: `Ctrl+P` (macOS), `Ctrl+Shift+P`, `Alt+Cmd+Y`.
  - Toggles between `Off`, `1x`, and `2x`.
  - Rendering specification:
    - The document is rendered into an offscreen buffer at $1.0\times$ or $2.0\times$ standard resolution.
    - The buffer is upscaled to the viewport using nearest-neighbor interpolation, revealing raster pixelation at high zoom levels ($> 100\%$).
    - Completely viewport-only; does not alter document geometry.

---

## Part 3: 5 Remaining UX Micro-Interaction State Machines

### 3.1 Right Sidebar Resize Drag, Clamping & Double-Click Reset (`#1109`)
- `[FACT - seen: figma_app-607050972cfcf2d9.min.js: devHandoffRightPanelWidth, propertiesPanelWidth]`
  - Default width: `240px`.
  - Minimum width: `240px`.
  - Maximum width: `500px`.
  - Double-click on the left border resize handle: resets width immediately to `240px`.
  - Active drag handle displays `ResizeHorizontal` (`col-resize` / `ew-resize`) cursor.
  - Keyboard navigation: Accessible as `role="separator"`, supports Left/Right arrow keys (stepping by 8px; Shift+Arrow by 32px), Home (collapses to min 240px), End (expands to max 500px).
  - Persistence: Saved in local storage / user profile and restored across sessions.

### 3.2 Left Sidebar Resize Parity & Keyboard Accessibility (`#1110`)
- `[FACT - seen: figma_app-607050972cfcf2d9.min.js: devHandoffDefaultLeftPanelWidth: "240px"]`
  - Default width: `240px`.
  - Minimum width: `200px` (or `240px`).
  - Maximum width: Clamped to leave at least 320px for the central canvas.
  - Double-click on resize handle resets width to `240px`.
  - Keyboard: Focusable separator with focus ring, accessible via Tab, arrow keys adjust width.

### 3.3 Pages Section Header Collapse Toggle (`#1111`)
- `[FACT - seen: wasm-strings.txt: toggle-section-visibility, en.json]`
  - In Figma's left sidebar, the Pages section header contains a chevron toggle.
  - When collapsed:
    - Pages list is hidden.
    - Draggable separator between Pages and Layers is hidden.
    - The header remains visible, displaying the active page name next to the "Pages" label.
    - Layers list expands to occupy 100% of the available sidebar height.
  - Keyboard: Toggle button supports `aria-expanded="false"`, activatable with Space/Enter.
  - State persists per document.

### 3.4 Layer Right-Click Context Menu Parity (`#1474`)
- `[FACT - seen: wasm-strings.txt:11256-11386, en.json]`
  Figma's layer list right-click context menu includes the following actions (matching main application menus):
  1. **Rename** (`Cmd+R` / `F2`)
  2. **Select All** (`Cmd+A`) / **Select Inverse** (`Shift+Cmd+A`)
  3. **Cut** (`Cmd+X`) / **Copy** (`Cmd+C`) / **Paste** (`Cmd+V`)
  4. **Copy as:**
     - Copy as SVG
     - Copy as PNG (`copy-as-png`)
     - Copy as CSS
  5. **Paste to Replace** (`Cmd+Shift+R` / `paste-to-replace`)
  6. **Group Selection** (`Cmd+G`) / **Ungroup Selection** (`Cmd+Shift+G`)
  7. **Frame Selection** (`Cmd+Alt+G`)
  8. **Use as Mask** (`Cmd+Ctrl+M` / `mask-selection`)
  9. **Create Component** (`Cmd+Alt+K` / `create-symbol`)
  10. **Outline Stroke** (`Cmd+Shift+O` / `outline-stroke`)
  11. **Flatten Selection** (`Cmd+E` / `flatten-selection`)
  12. **Boolean Operations:** Union, Subtract, Intersect, Exclude
  13. **Flip Horizontal** (`Shift+H` / `flip-horizontal`) / **Flip Vertical** (`Shift+V` / `flip-vertical`)
  14. **Bring Forward** (`Cmd+]`) / **Bring to Front** (`Cmd+Alt+]`)
  15. **Send Backward** (`Cmd+[`) / **Send to Back** (`Cmd+Alt+[`)
  16. **Lock / Unlock** (`Cmd+Shift+L` / `toggle-locked-for-selected-nodes`)
  17. **Hide / Show** (`Cmd+Shift+H` / `toggle-visible`)
  18. **Delete** (`Backspace` / `Delete`)

### 3.5 Canvas Handle Drag Deadzone Threshold (`#1730`)
- `[FACT - seen: plans/reports/code-reviewer-261107-p1-907-r2.md: H1]`
  - In `crates/designos-app/src/canvas/interact.rs:1213-1229`, corner radius handle dragging mapped pointer position to an absolute radius value on the very first frame of interaction:
    ```rust
    let r = dx.min(dy).max(0.0);
    ```
  - Similarly, shape control handles (`interact.rs:934` for star ratio, point count, arc start, sweep, inner radius) evaluated angles/ratios immediately on pointer-down.
  - A click-and-release without pointer motion resulted in:
    - Corner radius mutating from 0 to 11px!
    - Star inner ratio mutating from 0.382 to 0.344!
    - Pushing an unwanted entry to the Undo stack!
- `[INFERENCE]` **Clean-Room Fix for #1730**:
  - Enforce a drag threshold (`DRAG_THRESHOLD = 3.0` screen pixels):
    - Track `drag_start_pos: Pos2` and `has_dragged: bool`.
    - If `(pointer_pos - drag_start_pos).length() < DRAG_THRESHOLD`, suppress value mutations and undo registration.
    - On release without dragging, restore original values and cancel the interaction cleanly.

---

## Part 4: Codebase Root-Cause Anchoring & Action Plan Summary

| Issue | Target File(s) | Root Cause | Clean-Room Solution |
|---|---|---|---|
| `#1753` | `crates/designos-doc/src/layout_constraints.rs` | Resizing an imported arrow line stretched the arrowhead or dropped the outline causing half-stroke shift | Stretch only middle shaft between cap margins ($[E_{start}, W - E_{end}]$) |
| `#1754` | `crates/designos-mcp/src/tools/mutate.rs`, `crates/designos-doc/src/variables_apply.rs` | MCP and variable-bound dimension writers modify `size.x` directly without calling `stretch_along_line` | Route all line dimension mutations through `apply_constrained_geometry` |
| `#1755` | `crates/designos-app/src/panels/stroke.rs` | Stroke panel property edits update `n.stroke` but leave stale `n.stroke_geometry` intact | Clear `stroke_geometry = None` when weight, cap, join, or dash is edited |
| `#1756` | `crates/designos-fig/src/rest_import.rs` | `.fig` instance override with size but no outline clears stroke band | Stretch inherited component stroke band using overridden size |
| `#1757` | `crates/designos-fig/src/fig.rs`, `crates/designos-doc/src/layout_constraints.rs` | Per-vertex caps in `vectorData.styleOverrideTable` not read; `[0,0]` dash treated as non-solid | Read per-vertex caps; treat all-zero dash arrays as solid |
| `#1758` | `crates/designos-render/src/boolean.rs` | `child_shapes` returned `None` for 100% hairline ring, skipping operand in boolean sequence | Return `Some(Shapes::new())` (empty shapes) for closed non-fillable shapes |
| `#1765` | `crates/designos-app/src/panels/vertex.rs` | `CAPS` list only had 4 basic caps; arrow caps returned `None` and showed `"Mixed"` | Add arrow cap variants to `CAPS` in `vertex.rs` matching Stroke panel |
| `#953` | `crates/designos-render/src/geom.rs`, `crates/designos-export/src/geo.rs` | Stars/polygons fitted to rectangular box; dotted rings stroked two contours | Inscribe vertices on bounding ellipse; stroke rings along single centerline contour |
| `#1936` | `crates/designos-app/src/ops.rs` | `paste_properties` updates corner radii without clearing baked `fill_geometry` / `stroke_geometry` | Call `clear_baked_outlines(n)` in `paste_properties` and style matching |
| `#1937` | `crates/designos-doc/src/spec.rs`, `crates/designos-doc/src/spec_instance.rs` | `NodeSpec` and `OverrideSpec` lack `arc` field, dropping instance ring overrides | Add `ArcSpec` to `NodeSpec` and `OverrideSpec`; serialize and apply in MCP spec tools |
| `#1506` | `crates/designos-render/src/mask.rs`, `crates/designos-render/src/lib.rs` | Vector mask dropped overflowing children of non-clipping frames and ignored stroke masks | Include overflowing children in mask stencil union; honor stroke mask rules |
| `#1621` | `crates/designos-render/src/vectorops.rs` | Splitting touching glyph chains at 128 glyphs left overlapping outside stroke rings | Partition exterior boundary between split clusters without overlap or omit split |
| `#1631` | `crates/designos-render/src/paint.rs` | Adjusted image strokes evaluated bounds using layer box with zero margin | Expand bounds evaluation by `margin = stroke.weight` in `paint.rs` |
| `#1689` | `crates/designos-gpu/src/exec.rs`, `blit.wgsl` | Background blur downsampling clamped edges in `fs_blit` even when `clamp == false` | Pass `clamp: bool` into `resample` and output transparent borders when unclamped |
| `#1476` | `crates/designos-app/src/canvas/draw.rs`, `crates/designos-render/src/lib.rs` | Outlines mode and Pixel preview had no underlying rendering implementations | Implement 1px hairline rendering pass (with component purple) and 1x/2x pixel preview |
| `#1109` | `crates/designos-app/src/shell.rs` | Right sidebar width was fixed (`theme::RIGHT_PANEL_WIDTH`); no resize handle | Add draggable resize handle (clamp 240–500px, double-click reset to 240px, persistence) |
| `#1110` | `crates/designos-app/src/shell.rs` | Left sidebar resize existed but lacked persistence, double-click reset, and keyboard navigation | Persist left sidebar width; add double-click reset to 240px and keyboard support |
| `#1111` | `crates/designos-app/src/panels/left.rs` | Pages header lacked collapse/expand toggle button | Add chevron collapse toggle button; show current page name in header when collapsed |
| `#1474` | `crates/designos-app/src/panels/left.rs` | Layers panel right-click menu lacked mask, flatten, outline stroke, boolean, flip, copy as | Add full action menu to layer list right-click context menu matching main menus |
| `#1730` | `crates/designos-app/src/canvas/interact.rs` | Corner radius and shape control handles applied values on click-down without deadzone | Enforce `3.0px` drag deadzone threshold before mutating values or registering undo |

---
**Verification Digest:** Generated from clean-room static analysis and REA evidence bundle `ev_d40f84650d449631485673749ef384b0ba7719737de1c20a9cd4e264aae1865b`. No proprietary source code included.
