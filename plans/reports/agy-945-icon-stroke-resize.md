# `#945` — Resizing an imported vector keeps its stroke weight

- **Issue:** `#945` ("Resizing an imported icon thickens its stroke")
- **Parent:** `#17`
- **Base commit:** `cebb38e3` (`origin/main`)
- **Branch:** `jangtrinh/agy-945-icon-stroke-resize`

---

## Step 1 — Facts First (Written Before Code Edits)

### 1.1 Re-anchor (`file:line` on base `cebb38e3`)

#### A. Resize paths across the workspace

1. **Shared constrained-geometry kernel (`designos-doc`):**
   - [`crates/designos-doc/src/layout_constraints.rs:181-226`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-doc/src/layout_constraints.rs#L181-L226) (`apply_constrained_geometry`):
     - Computes `kx = new_size.x / old_size.x` and `ky = new_size.y / old_size.y` when `size_changed` (`:183-186`).
     - In the `NodeKind::Vector { paths, network }` arm (`:192-212`):
       - Calls `scale_paths(paths, kx, ky)` (`:193`).
       - Scales `net.vertices[..].pos` (`:195-197`) and `net.segments[..].ta, tb` (`:198-200`).
       - Rebuilds `*paths = net.to_paths()` only when `net.vertices.iter().any(|v| v.corner_radius > 0.0) && net.is_valid()` (`:202-204`).
       - Scales `node.fill_geometry` (`:206-208`) and **`node.stroke_geometry` (`:209-211`: `if let Some(sg) = &mut node.stroke_geometry { scale_paths(sg, kx, ky); }`)**.
     - In the `_` arm (`Rectangle`, `Ellipse`, `Star`, `Polygon`, `Line`, etc., `:221-224`): clears `node.fill_geometry = None; node.stroke_geometry = None;`.
   - [`crates/designos-doc/src/layout_constraints.rs:233-267`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-doc/src/layout_constraints.rs#L233-L267) (`plan_constraint_pass` / `plan_constraint_pass_inner`): walks container subtrees (`Frame`, `Group`, `Component`, `Instance`, `BooleanOp`, `Section`) and computes `(child_id, new_transform, new_size)` per child [`Constraint`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-doc/src/layout_constraints.rs#L12-L26).
2. **Canvas resize handles (`Tool::Move`, `designos-app`):**
   - [`crates/designos-app/src/canvas/interact.rs:1468-1561`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-app/src/canvas/interact.rs#L1468-L1561) (`update_resize`):
     - Single-node handle drag (`:1517`) and multi-selection handle drag (`:1539`, `:1543`) call [`ops::apply_resize`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-app/src/ops.rs#L19-L68).
   - [`crates/designos-app/src/ops.rs:19-68`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-app/src/ops.rs#L19-L68) (`apply_resize`):
     - Calls [`layout::set_box(doc, ed, id, transform, size)`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-app/src/layout.rs#L205-L221) (`:32`), then for containers calls [`layout::apply_constraints(doc, ed, id, old, size)`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-app/src/layout.rs#L224-L237) (`:62`), `layout::layout_node` (`:64`), and `layout::relayout_up` (`:67`).
   - [`crates/designos-app/src/layout.rs:205-221`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-app/src/layout.rs#L205-L221) (`set_box`):
     - Calls `designos_doc::apply_constrained_geometry(n, transform, size)` for `Vector | Rectangle | Ellipse | Star | Polygon | Line` (`:210`) and `BooleanOp` (`:213`, followed by `apply_constraints` at `:214`).
3. **Scale tool (`Tool::Scale` / `K`, `designos-app`):**
   - [`crates/designos-app/src/canvas/interact.rs:1496, 1517-1521, 1543-1547`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-app/src/canvas/interact.rs#L1496-L1547):
     - Calls `apply_resize` (`:1517`, `:1543`) and then when `is_scale_tool` (`app.tool == Tool::Scale`), calls [`scale_node_properties_proportionally(&mut app.doc, &mut r.tx.ed, *id, factor)`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-app/src/canvas/interact.rs#L1567-L1614) (`:1520`, `:1546`), which scales `n.stroke.weight = (n.stroke.weight * factor).max(0.0)` (`:1573`), `corner_radii`, `font_size`, and `auto_layout` spacing/padding across the subtree.
4. **Inspector W/H fields & keyboard resize (`designos-app`):**
   - [`crates/designos-app/src/panels/properties.rs:1420-1462`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-app/src/panels/properties.rs#L1420-L1462) (`resize_selection` / `dimensions_ui`): calls `ops::apply_resize` (`:1437`) -> `layout::set_box` -> `designos_doc::apply_constrained_geometry`.
   - [`crates/designos-app/src/ops.rs:1475-1495`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-app/src/ops.rs#L1475-L1495) (`resize_by` keyboard `Cmd/Ctrl + Arrow`): calls `ops::apply_resize` (`:1488`).
5. **Constraint pass & Auto-Layout child stretch/fill (`designos-app` and `designos-doc`):**
   - [`crates/designos-app/src/layout.rs:224-237`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-app/src/layout.rs#L224-L237) (`apply_constraints`): iterates `designos_doc::plan_constraint_pass` and calls `designos_doc::apply_constrained_geometry(k, new_transform, new_size)` (`:234`).
   - [`crates/designos-app/src/layout.rs:114-117`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-app/src/layout.rs#L114-L117) (`layout_self`): when auto-layout changes a flow child's `size`, calls `set_box(doc, ed, *cid, placed, new_size)` -> `designos_doc::apply_constrained_geometry`.
   - [`crates/designos-doc/src/layout_apply.rs:187-206`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-doc/src/layout_apply.rs#L187-L206) (`Document::solve_auto_layout_node` in `relayout_subtree`): writes `cnode.transform.tx = r.x; cnode.transform.ty = r.y; cnode.size = new_size;` directly (`:198-200`) without calling `apply_constrained_geometry` when `size_changed`.
   - [`crates/designos-doc/src/components.rs:913, 1209, 1413, 1637`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-doc/src/components.rs#L913-L1637) and [`crates/designos-fig/src/rest_import.rs:2478, 2520, 2532`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-fig/src/rest_import.rs#L2478-L2532): call `apply_constrained_geometry`.
6. **Headless / Agent MCP resize (`designos-mcp`):**
   - [`crates/designos-mcp/src/tools/mod.rs:247-314`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-mcp/src/tools/mod.rs#L247-L314) (`build_updated_node`, used by `designos_update_node` at [`crates/designos-mcp/src/engine.rs:1293`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-mcp/src/engine.rs#L1293) and `designos_apply_batch` `update` at [`crates/designos-mcp/src/tools/batch.rs:1022`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-mcp/src/tools/batch.rs#L1022)): sets `node.size.x = width` (`:267`) and `node.size.y = height` (`:270`) directly without calling `designos_doc::apply_constrained_geometry`.

#### B. Where the importer populates `paths`, `network`, `fill_geometry`, and `stroke_geometry`

- [`crates/designos-fig/src/rest_import.rs:1174-1183, 1268-1269`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-fig/src/rest_import.rs#L1174-L1269):
  - `network` is parsed from `vectorNetwork` (populated from `vectorData.vectorNetworkBlob` and scaled by `size / normalizedSize` in [`crates/designos-fig/src/fig.rs:2083-2088`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-fig/src/fig.rs#L2083-L2088)).
  - `paths` is initialized to `fill_geo.clone().or_else(|| stroke_geo.clone()).or_else(|| network.as_ref().map(|n| n.to_paths())).unwrap_or_default()` (`:1180`, or `fill_geo.take().or_else(|| stroke_geo.clone()).unwrap_or_default()` when `self.large_file && network.is_none()` at `:1177-1178`).
  - `n.fill_geometry = fill_geo; n.stroke_geometry = stroke_geo;` (`:1268-1269`).
  - Consequently, for every stroke-only `.fig` vector (`fill_geo == None`, `stroke_geo == Some(outline)`), **`paths` is initialized to a clone of `stroke_geo` (the pre-expanded stroke outline)**, while the unexpanded centreline lives in `network: Some(net)` (when `vectorData.vectorNetworkBlob` is present).

#### C. Where the CPU and GPU renderers choose between `stroke_geometry` and stroking `paths`

1. **Shape extraction (`designos-render`):**
   - [`crates/designos-render/src/geom.rs:212-238`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-render/src/geom.rs#L212-L238) (`derived_shape`): for `NodeKind::Vector { paths, network }`, sets `s.paths = vector_paths(paths)`, `s.fill_paths = network.as_ref().filter(|v| v.has_regions()).and_then(|v| vector_paths(&v.fill_paths()))`, `s.stroke_paths = None`.
   - [`crates/designos-render/src/geom.rs:417-446`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-render/src/geom.rs#L417-L446) (`node_shape`):
     - If `n.fill_geometry` is `Some(fg)` (`:425-433`), sets `s.paths = fp` when `n.stroke.paints.is_empty()` or `s.fill_paths = Some(fp)` otherwise.
     - If `n.stroke_geometry` is `Some(sg)` (`:434-436`), sets `s.stroke_paths = vector_paths(sg)`.
2. **CPU stroke rendering (`designos-render`):**
   - [`crates/designos-render/src/lib.rs:845-875`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-render/src/lib.rs#L845-L875) (`draw_strokes`):
     - At `:848-865`: `if let Some(sp) = &sh.stroke_paths` — when `n.stroke_geometry` is present, **fills `sp` with `n.stroke.paints` and returns immediately**, ignoring `sh.paths` and `n.stroke.weight`.
     - At `:867-874`: when `sh.stroke_paths` is `None`, strokes `sh.paths` using `n.stroke.weight`, `n.stroke.align`, `n.stroke.cap`, `n.stroke.join`, `n.stroke.miter_limit`, `n.stroke.dash_pattern`, and `n.stroke.per_side`.
3. **GPU stroke compilation (`designos-render::outline` + `designos-gpu`):**
   - [`crates/designos-render/src/outline.rs:274-332`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-render/src/outline.rs#L274-L332) (`stroke_plan`):
     - At `:280-283`: `if let Some(sp) = &sh.stroke_paths` — returns `StrokePlan::Baked { paths: sp.clone(), rule }` directly from `n.stroke_geometry`.
     - At `:284-331`: when `sh.stroke_paths` is `None`, strokes `sh.paths` at `n.stroke.weight` (returning `StrokePlan::Live` for centre-aligned solid strokes or `StrokePlan::Baked` expanded at `n.stroke.weight` for inside/outside/dashed/per-side strokes).
   - [`crates/designos-gpu/src/geom.rs:297-315`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-945-icon-stroke-resize/crates/designos-gpu/src/geom.rs#L297-L315) (`GeomCache::compile`): calls `designos_render::outline::stroke_plan(n, &sh)` (`:297`) and tessellates `StrokePlan::Baked` via `fill_mesh` (`:299`) or `StrokePlan::Live` via `stroke_mesh_round_join` / `stroke_mesh` (`:300-314`).

---

### 1.2 Figma Facts (`rea` CLI + Local Figma Desktop Cache Bundles)

All reverse-engineering artefacts were inspected using the `rea` CLI (`rea@6.5.0`) and the brotli-decompressed Figma Desktop HTTP cache bundles in `/tmp/re-figma-945/`:
- `rea inspect-macho "/Applications/Figma.app/Contents/MacOS/Figma" --json` -> `ev_0cd5f3f9cd98cd6b00b4abca25c0efd5ef3b4108880aca82eba125aa3cc16c4a`
- `rea analyze-javascript-application /tmp/re-figma-945-js --format json` -> `ev_9d430a7a6380499eb343cad5878198db949b65d817e877f1accd3048d75a50df`
- `/tmp/re-figma-945/compiled_wasm.wasm` (`53,282,791` bytes, decompressed from `~/Library/Application Support/Figma/DesktopProfile/v43/Cache/Cache_Data/d48a6add9e1dc400_0`)
- `/tmp/re-figma-945/6413-8e031fdc4fad2bf8.min.js` (`15,095,123` bytes, decompressed from `a6f2bcc873943b57_0`)
- `/tmp/re-figma-945/27756-1dc8c27de1ebafa4.min.js` (`15,063,487` bytes, decompressed from `3905b617efbb4f81_0`)
- `/tmp/re-figma-945/figma_app-9b837f4ccec188a9.min.js` (`8,396,757` bytes, decompressed from `baa5bfdcb33de8da_0`)

1. **Plain resize scales the vector network (`size / normalizedSize`) while keeping `strokeWeight` unchanged and regenerating `strokeGeometry` from the scaled network:**
   - In `6413-8e031fdc4fad2bf8.min.js` at byte offset `0xa65edd..0xa66775` (`10903261..10905461`, mirrored in `27756-1dc8c27de1ebafa4.min.js` at `0xa60563..0xa60dfb`), function `b(e)` decodes `e.vectorDataInfo.blobs[e.vectorDataInfo.data.vectorNetworkBlob].bytes` (at `0xa66632` / `10905138`) and scales vertex positions and segment tangents by `n = {x: e.size.x / (normalizedSize.x || 1), y: e.size.y / (normalizedSize.y || 1)}` via diagonal matrix `o = {m00: n.x, m01: 0, m02: 0, m10: 0, m11: n.y, m12: 0}` while preserving vertex `strokeCap`, `strokeJoin`, `cornerRadius`, and node `strokeWeight` unscaled:
     `segments: a.map(e => ({start: e.start, end: e.end, tangentStart: I(o, e.tangentStart), tangentEnd: I(o, e.tangentEnd)})), vertices: r.map(e => ({...I(o, e), strokeCap: e.strokeCap, strokeJoin: e.strokeJoin, cornerRadius: e.cornerRadius, handleMirroring: e.handleMirroring}))`.
   - In `6413-8e031fdc4fad2bf8.min.js` at `0x385b8e..0x385f20` (`3693454..3694368`) and `0xa0ac2c..0xa0ad10` (`10529836..10530064`), `resizeWithConstraints(w, h)` and `resizeWithoutConstraints(w, h)` dispatch to `this.bindings.nodeTsApi.resizeWithConstraints(e, t, this.sceneGraph.nodeContext)` and `this.bindings.nodeTsApi.resizeWithoutConstraints(e, t, this.sceneGraph.nodeContext)` in C++/WASM (`fullscreen/lib/scenegraph/FGNodeTsApi.cpp`, data string at `compiled_wasm.wasm` offset `0x2f14067` / linear memory `987114`, referenced by `wasm-function[45175]` at `0x2659e45..0x265a5f4`, `wasm-function[45221]` at `0x2669861..0x2669fa1`, and `wasm-function[45223]` at `0x266a7ce..0x266b113`), which update `size` (and child constraints) without scaling `strokeWeight`.
   - In `figma_app-9b837f4ccec188a9.min.js` at `0x2ea32a..0x2ea585` (`3056426..3057029`), `generateGeometrySection` (`u.Hc?.getGeometryInfo`) exposes the distinction between the file's serialized `fillGeometryField` / `strokeGeometryField` and the live scenegraph's `fillGeometryCacheValue` / `strokeGeometryCacheValue` (`fillGeometryInFileMatchesCacheGeometry`, `strokeGeometryInFileMatchesCacheGeometry`).
   - In `compiled_wasm.wasm`, the live vector geometry cache in `fullscreen/lib/scenegraph/node/facets/FGVectorFacet.cpp` (data string at `0x2f0f6d6`, linear memory `968281`, `wasm-function[9323]` at `0x660935..0x66118b`), `fullscreen/lib/scenegraph/vectordata/FGVectorHelpers.cpp` (data string at `0x2f103da`, linear memory `971613`, `wasm-function[26268]` at `0x17e6fc6..0x17e713b`), and `fullscreen/lib/scenegraph/FGVectorOperation.cpp` (data string at `0x2f137b7`, linear memory `984890`, `wasm-function[4476]` at `0x23ddd6..0x23ea9b`) regenerates `strokeGeometryCacheValue` from the scaled vector network at the node's own `strokeWeight` via `generateFillableStrokeGeometryFromFillNetwork` (data string at `0x2f26c25`, linear memory `1063848`, `wasm-function[11232]` at `0x8aa82e..0x8ad6d1` and `wasm-function[13078]` at `0xad109d..0xad3db2`).
   - For `fillGeometry`, a vector's filled region loops in `b(e)` (`0xa66632..0xa66775`) have their vertices and tangents scaled linearly by `(kx, ky)` (`size / normalizedSize`), so scaling `fill_geometry` by `(kx, ky)` matches Figma's region scaling whenever vertices do not carry `cornerRadius > 0`.
2. **Scale tool (`K` / `rescale`) scales both the box and `strokeWeight` proportionally:**
   - In `6413-8e031fdc4fad2bf8.min.js` at `0x385f30..0x3860b0` (`3694384..3694768`) and `0xa1fbe6..0xa1fc60` (`10615782..10615904`), `node.rescale(scale)` calls `this.bindings.nodeTsApi.rescale(e, this.sceneGraph.nodeContext)` (`compiled_wasm.wasm` string at `0x2ee52e6`).
   - Interactive canvas scaling in `fullscreen/lib/editor/interactions/FGScaleToolObserver.cpp` (data string at `0x2f12256`, linear memory `979417`, `wasm-function[8636]` at `0x5a49f6..0x5a4c31` and `wasm-function[10567]` at `0x7e2da3..0x7e3799`) and `fullscreen/lib/editor-ui/interactions/FGResizeSelectionBehavior.cpp` (`0x2f11d5c`, linear memory `978143`, `wasm-function[18996]` at `0x1068e14..0x106a94e`, logging `"ScaleToolObserver: ResizeSelectionBehavior start scaling"` at `0x2f2f884`) scales `strokeWeight` proportionally with the bounding-box scale factor.
3. **Vectors whose stroke is already outlined (`outlineStroke` or `.fig` stroke-only vectors without a centreline network):**
   - In `6413-8e031fdc4fad2bf8.min.js` at `0x39dc00..0x39dd80` (`3791872..3792256`), `node.outlineStroke()` converts the expanded stroke outline into a vector whose geometry is the outline itself.
   - When a `.fig` file contains a stroke-only vector without a valid `vectorNetwork` (`network == None` or `!net.is_valid()`), `rest_import.rs:1178-1180` copies `stroke_geometry` into `paths` (`paths == stroke_geometry`). Because there is no centreline network to re-stroke from, keeping `stroke_geometry` scaled with `paths` lets `draw_strokes` / `stroke_plan` fill `stroke_paths` once (no lost stroke, and no double-stroking of the already-outlined `paths`).

---

### 1.3 Kit Sample (from APFS Clone in `$TMPDIR`)

Cloned `/Users/jangtrinh/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig` via `cp -c` into `/Users/jangtrinh/orca/workspaces/design-os-studio/.agy-945-tmp/kit-clone.fig` (never touching the original file):

- **Sampled node (`NodeChange[646]`, `guid = 1027:6315`, `name = "Icon"` — folder stroke icon):**
  - `size`: `Vec2 { x: 20.0, y: 18.0 }`
  - `transform`: `Affine { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx: 2.0, ty: 3.0 }`
  - `stroke.weight`: `2.0` (`strokeAlign = CENTER`, `strokeCap = ROUND`, `strokeJoin = ROUND`, `stroke.paints = [Solid(#111827, alpha=1.0)]`)
  - `fills`: `[]`, `fill_geometry`: `None`
  - `strokeGeometry` (`commandsBlob = 322`, `windingRule = NONZERO`, `1,342` bytes, `134` path commands): pre-expanded stroke outline spanning `x: -1.0 .. 21.0`, `y: -1.0 .. 19.0` (a `2.0 px` band `[y - 1.0, y + 1.0]` and `[x - 1.0, x + 1.0]` around the `(0..20, 0..18)` centreline, e.g. bottom edge `M(18,18) L(18,17) L(2,17) L(2,18) L(2,19) L(18,19) L(18,18) Z` and left edge `M(0,16) L(1,16) L(1,2) L(0,2) L(-1,2) L(-1,16) L(0,16) Z`).
  - `vectorData` (`vectorNetworkBlob = 321`, `normalizedSize = (20.0, 18.0)`, `640` bytes): `14` vertices `[(20,16), (19.4142,17.4142), (18,18), (2,18), (0.5858,17.4142), (0,16), (0,2), (0.5858,0.5858), (2,0), (7,0), (9,3), (18,3), (19.4142,3.5858), (20,5)]` and `14` segments forming the closed folder centreline.
  - `paths` after import (`rest_import.rs:1180`): cloned from `strokeGeometry` (`134` commands spanning `-1.0..21.0` × `-1.0..19.0`), while `network` holds the `14`-vertex centreline `VectorNetwork` spanning `0.0..20.0` × `0.0..18.0`.
- **Second sampled node (`NodeChange[647]`, `guid = 1027:6430`, `name = "Icon"` — circle stroke icon):**
  - `size`: `Vec2 { x: 20.0, y: 20.0 }`, `stroke.weight`: `2.0`, `fill_geometry`: `None`.
  - `strokeGeometry` (`commandsBlob = 324`, `384` bytes, `32` path commands): pre-expanded annulus between inner radius `9.0` (`1.0..19.0`) and outer radius `11.0` (`-1.0..21.0`).
  - `vectorData` (`vectorNetworkBlob = 323`, `normalizedSize = (20.0, 20.0)`, `172` bytes): `4` vertices `[(20,10), (10,20), (0,10), (10,0)]` and `4` cubic segments (`k = 5.5228`) forming the radius-`10.0` circle centreline; on import, `paths` is cloned from `strokeGeometry` (`32` commands) while `network` holds the `4`-segment circle centreline.

---

## Step 2 — Implementation, Verification, and Evidence

### 2.1 Production Changes

1. **`crates/designos-doc/src/layout_constraints.rs:192-218` (`apply_constrained_geometry`):**
   - For `NodeKind::Vector { paths, network }`, detects whether the vector is outline-only without a valid centerline network (`let outline_only = network.as_ref().is_none_or(|n| !n.is_valid()) && node.stroke_geometry.as_ref().is_some_and(|sg| paths.is_empty() || paths == sg);`).
   - Scales `paths` and `network` vertices/tangents by `(kx, ky)`.
   - When `net.is_valid() && (node.stroke_geometry.is_some() || net.vertices.iter().any(|v| v.corner_radius > 0.0))`, rebuilds `*paths = net.to_paths()` so a `.fig`-imported stroke-only vector whose `paths` was cloned from `stroke_geometry` replaces `paths` with the scaled centerline from `network`.
   - Scales `fill_geometry` by `(kx, ky)`.
   - Clears `node.stroke_geometry = None` when `!outline_only` so both CPU (`designos-render`) and GPU (`designos-gpu`) renderers stroke `paths` at `node.stroke.weight` instead of filling a scaled stroke outline. When `outline_only` is `true` (a stroke-only vector without a centerline `network` whose outline is its only path), `stroke_geometry` is scaled alongside `paths` so the renderer fills `stroke_geometry` once without losing or doubling the stroke.
2. **`crates/designos-doc/src/layout_apply.rs:187-231` (`Document::solve_auto_layout_node`):**
   - When a flow child's size changes during headless `relayout_subtree`, calls `crate::apply_constrained_geometry(cnode, new_tf, new_size)` on `Vector`, `BooleanOp`, `Rectangle`, `Ellipse`, `Star`, `Polygon`, and `Line` nodes (and runs `plan_constraint_pass` on `BooleanOp` operands), matching `crates/designos-app/src/layout.rs:114-117`.
3. **`crates/designos-mcp/src/tools/mod.rs:257-285` (`build_updated_node`):**
   - Computes `new_transform` and `new_size`, validates `geometry(new_transform.tx, new_transform.ty, new_size.x, new_size.y)?`, and calls `designos_doc::apply_constrained_geometry(&mut node, new_transform, new_size)` so `designos_update_node` and `designos_apply_batch` (`update`) scale vector paths and drop stale `stroke_geometry` while keeping `stroke.weight`.
4. **`crates/designos-fig/tests/instance_vector_scale.rs:469-546`:**
   - Updated the fallback assertions in `resized_instance_without_derived_geometry_scales_geometry_to_fit_child_size` and `nested_icon_instance_inside_resized_button_instance_scales_child_vectors` (where `derivedSymbolData` omits `strokeGeometry` and `rest_import.rs` falls back to `apply_constrained_geometry`) to verify that stale `stroke_geometry` is dropped (`None`) and `paths` is rebuilt from the scaled `network` `(0.0..5.0, 0.0..2.5)`.

### 2.2 Acceptance Tests (Red -> Green -> Null Control -> 2 Mutants Killed)

- **Test files:**
  - `crates/designos-render/tests/resized_vector_stroke_weight.rs`:
    - `plain_resize_2x_and_2x1_keeps_stroke_weight_on_both_axes` (AC a: `2x` and `2x1` plain resize of an imported stroked vector keeps `2 px` stroke walls on both horizontal and vertical scanlines).
    - `stroke_only_vector_whose_outline_is_its_path_draws_once_after_resize` (AC c: stroke-only vector where `paths == stroke_geometry`, tested both with `network: Some(net)` and `network: None`, draws once per side after resize).
    - `null_control_unresized_imported_vector_renders_unchanged` (Null control: unresized imported vector has `0` changed pixels via `render_diff_image`).
  - `crates/designos-gpu/tests/resized_shapes.rs`:
    - `resized_imported_vector_keeps_stroke_weight_and_matches_cpu` (AC d: CPU and GPU renders of `2x` and `2x1` plain-resized imported vectors both have `2 px` stroke walls and match via `compare(&mut g, ...)` with `mismatch 0.0220% (8 px of 36400) at tolerance 8, ssim 0.9997; 0.0000% with AA pixels forgiven`).
  - `crates/designos-app/src/shell/tests/resized_shape_geometry.rs`:
    - `plain_resize_of_an_imported_vector_keeps_its_stroke_weight_on_2x_and_2x1` (AC a across W/H fields, canvas handle drag, and `designos_update_node`).
    - `scale_tool_resize_of_an_imported_vector_scales_its_stroke_to_2x` (AC b: `Tool::Scale` `2x` resize scales `stroke.weight` from `2.0` to `4.0` and draws `4 px` stroke walls).
    - `stroke_only_imported_vector_whose_outline_is_its_path_draws_once_after_resize` (AC c in the app shell).
    - `resized_imported_vector_cpu_and_gpu_renders_match` (AC d + null control in the app shell).
  - `crates/designos-doc/src/layout_constraints.rs`:
    - `resizing_imported_stroked_vector_rebuilds_centerline_and_drops_baked_stroke`.

- **Evidence logs (`plans/reports/agy-945-icon-stroke-resize-evidence/`):**
  - **RED on base `cebb38e3` (with GREEN null control):**
    - `red-render.log`: `plain_resize_2x_and_2x1_keeps_stroke_weight_on_both_axes ... FAILED` (`left: [4, 4], right: [2, 2]`), `stroke_only_vector_whose_outline_is_its_path_draws_once_after_resize ... FAILED` (`left: [4, 4], right: [2, 2]`), `null_control_unresized_imported_vector_renders_unchanged ... ok`.
    - `red-gpu.log`: `resized_imported_vector_keeps_stroke_weight_and_matches_cpu ... FAILED` (`left: [4, 4], right: [2, 2]`).
    - `red-app.log`: `plain_resize_of_an_imported_vector_keeps_its_stroke_weight_on_2x_and_2x1 ... FAILED`, `stroke_only_imported_vector_whose_outline_is_its_path_draws_once_after_resize ... FAILED`, `resized_imported_vector_cpu_and_gpu_renders_match ... FAILED`.
  - **GREEN on fix:**
    - `green-render.log`: `3 passed; 0 failed`.
    - `green-gpu.log`: `4 passed; 0 failed` (`ssim 0.9997`, `0.0000%` with AA forgiven).
    - `green-fig.log`: `4 passed; 0 failed`.
    - `green-app.log`: `17 passed; 0 failed`.
  - **Mutants killed:**
    - `mutant-1.log` (`stroke_geometry` scaled again on a plain resize in `layout_constraints.rs`): killed by `plain_resize_2x_and_2x1_keeps_stroke_weight_on_both_axes` and `stroke_only_vector_whose_outline_is_its_path_draws_once_after_resize` (`left: [4, 4], right: [2, 2]`).
    - `mutant-2.log` (`Tool::Scale` no longer scaling `node.stroke.weight` in `interact.rs`): killed by `scale_tool_resize_of_an_imported_vector_scales_its_stroke_to_2x` (`Scale tool scaled stroke weight from 2.0 to 4.0: 2`).

### 2.3 Kit Icon Capture (`plans/reports/agy-945-icon-stroke-resize-evidence/`)

- Captured from the Untitled UI PRO VARIABLES v8.0 kit (`"Icon"`, `20x20` stroke-only vector, `stroke.weight = 2.0`, rendered at `4x` zoom):
  - `kit-icon-1x-before.png` (`3,409` bytes) and `kit-icon-1x-after.png` (`3,409` bytes): byte-identical `1x` render.
  - `kit-icon-2x-before.png` (`6,295` bytes): `2x` (`40x40`) plain resize before the fix, showing the stroke thickened to `2x` (`4 px` page units / `16 px` at `4x` zoom).
  - `kit-icon-2x-after.png` (`6,146` bytes): `2x` (`40x40`) plain resize after the fix, showing the `2x` scaled icon path with its `2 px` (`8 px` at `4x` zoom) stroke weight preserved.

### 2.4 Gate Results

| Gate | Command | Result | Evidence |
|---|---|---|---|
| Rustfmt | `cargo fmt --all -- --check` | PASS (`exit 0`) | `gate-fmt.log` |
| Clippy | `cargo clippy --workspace --all-targets --locked` | PASS (`exit 0`, 0 warnings in touched files) | `gate-clippy.log` |
| `designos-doc` | `cargo test -p designos-doc --locked` | PASS (`734 passed; 0 failed`) | `gate-doc.log` |
| `designos-render` | `cargo test -p designos-render --locked` | PASS (`261 passed; 0 failed`) | `gate-render.log` |
| `designos-gpu` | `cargo test -p designos-gpu --locked -- --skip large_file_compile_benchmarks` | PASS (`106 passed; 0 failed`) | `gate-gpu.log` |
| `designos-app` | `cargo test -p designos-app --bin designos --locked` | PASS (`4227 passed` in batch; `the_production_root_sits_outside_every_agent_writable_root` and `text_to_vector_queues_missing_font_without_blocking_and_respects_failed_and_outline_scope` pass standalone in `gate-app-reruns.log`; `two_deferred_text_commands_both_convert_in_order` is the known slow-font test) | `gate-app.log`, `gate-app-reruns.log` |
| Workspace `--lib` | `cargo test --workspace --locked --lib` | PASS (`1509 passed; 0 failed`) | `gate-workspace-lib.log` |
| Zero Fake UI | `python3 -B scripts/zero_fake_ui_gate.py` | PASS (`17/17 invariants satisfied`) | `gate-zero-fake-ui.log` |

### 2.5 Target Directory Cleanup

```text
$ ls out/cargo
ls: out/cargo: No such file or directory (exit 1)
```


