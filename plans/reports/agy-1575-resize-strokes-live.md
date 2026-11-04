# `#1575` — Resized vectors keep their stroke weight when the stroke mixes alignments or joins

## What Could Not Be Verified

- Full 31-page interactive GUI window frame-time profiling was not run because the headless worker contract forbids opening GUI windows (`brief-agy-1575-resize-strokes-live.txt:29`); all CPU and GPU renders and kit captures were verified headlessly through `designos-render` (`crates/designos-render/src/lib.rs:840-925`) and `designos-gpu` (`crates/designos-gpu/src/geom.rs:266-277`).
- Within the 45-minute reverse-engineering budget (`brief-agy-1575-resize-strokes-live.txt:6`) on `compiled_wasm.wasm`, the exact branch instruction inside stripped `wasm-function[11231]` / `wasm-function[13077]` (`generateFillableStrokeGeometryFromFillNetwork` at `0x8aa82e..0x8ad6d1` and `0xad109d..0xad3db2`) was corroborated via the unminified JS style-table decoder in `6413-8e031fdc4fad2bf8.min.js` (`0xa65edd..0xa66775`) and Figma's own serialized `strokeGeometry` blobs on the kit's `83` open-chain `Inside`-stroke vectors and `63` mixed-join arrow vectors (`step1-kit-initial-counts.log:11-12`, `step1-kit-initial-counts.log:18-42`, `step1-kit-counts.log:394-421`), together with Ruling 1 and Ruling 2 (`brief-agy-1575-resize-strokes-live.txt:19-20`).

---

## Conclusion

All six rulings (`brief-agy-1575-resize-strokes-live.txt:18-24`) are implemented and verified across `designos-doc`, `designos-fig`, `designos-render`, and `designos-gpu`:
1. **Per-subpath stroke alignment (`Ruling 1`):** Closed sub-paths keep `StrokeAlign::Inside` or `StrokeAlign::Outside` while open chains stroke `StrokeAlign::Center`, shared between CPU (`Renderer::draw_strokes` at `crates/designos-render/src/lib.rs:894-915`) and GPU (`outline::stroke_plan` at `crates/designos-render/src/outline.rs:300-303`, calling `vectorops::has_mixed_subpath_align` at `crates/designos-render/src/vectorops.rs:482-487` and `vectorops::mixed_subpath_stroke_outline` at `crates/designos-render/src/vectorops.rs:512-520`).
2. **Per-vertex stroke joins (`Ruling 2`):** `expand_subpath` (`crates/designos-render/src/vectorops.rs:298-363`) and `expand_node_subpaths` (`crates/designos-render/src/vectorops.rs:490-509`) split multi-segment sub-paths at segment midpoints (`split_cubic_half` at `crates/designos-render/src/vectorops.rs:253-262`) and stroke each corner piece with its vertex's `StrokeJoin` override (`v.join.unwrap_or(stroke.join)` at `crates/designos-render/src/vectorops.rs:310-312`) on both CPU (`crates/designos-render/src/lib.rs:894-915`) and GPU (`crates/designos-render/src/outline.rs:309-312`).
3. **Bake drop predicate (`Ruling 3`):** `live_stroke_reproduces_bake` (`crates/designos-doc/src/layout_constraints.rs:188-197`) drops `stroke_geometry` on plain resize for `Inside`/`Outside` strokes with open chains and for per-vertex join overrides while preserving the scaled bake for side weights, brushes, arrow cap markers (`meta["strokeCap"]`), differing vertex caps (`v.cap != stroke.cap`), and dashed `Inside`/`Outside` strokes.
4. **Arrow cap REST round-trip (`Ruling 4`):** `export_node` (`crates/designos-fig/src/export.rs:542-550, 682-691`) and `unmodelled_stroke_cap` (`crates/designos-fig/src/export.rs:810-812`) write `fig_metadata["strokeCap"]` back onto the node's `"strokeCap"` when `node.stroke.cap == StrokeCap::None` (`export.rs:545`) and onto the first vertex without a `"strokeCap"` in `vectorNetwork.vertices` when `node.stroke.cap != StrokeCap::None` (`export.rs:684-690`).
5. **Unified equal-cap path (`Ruling 5`):** `vertex_caps` (`crates/designos-render/src/vectorops.rs:442-473`) and `outline_stroke_along` (`crates/designos-render/src/vectorops.rs:558`) only take the separate polygon-cap path when at least one vertex has `v.cap.is_some_and(|c| c != stroke.cap)` (`vectorops.rs:446, 558`), so open strokes whose per-vertex caps equal `n.stroke.cap` draw through the native stroker on both CPU and GPU.
6. **Unresized vector pixel parity (`Ruling 6`):** All `318` sampled unresized kit vectors render with identical FNV-1a pixel hashes on base (`54f0c883`) and head (`DIFF_EXIT=0` at `kit-hashes-diff.log:12-15`).

---

## Step 1 — Facts First (Written Before Editing Code)

### 1.1 Re-anchor on Base `54f0c883` (`file:line`)

1. **Where an open chain forces `Center`:**
   - `crates/designos-render/src/geom.rs:358-410` (`vector_paths`): initializes `let mut closed = true;` (`geom.rs:360`) and sets `closed = false` (`geom.rs:369`, `geom.rs:403`) whenever any subpath in `paths` does not end with `PathCmd::Close`.
   - `crates/designos-render/src/lib.rs:846` (`Renderer::draw_strokes`): `let align = if sh.closed { n.stroke.align } else { designos_doc::StrokeAlign::Center };` forces `StrokeAlign::Center` for the entire shape (including closed subpaths) when `sh.closed` is `false`.
   - `crates/designos-render/src/outline.rs:233` (`dashed_rect_stroke_path`) and `crates/designos-render/src/outline.rs:279` (`stroke_plan`, called by `NodeGeom::stroke_mesh` at `crates/designos-gpu/src/geom.rs:273`): `let align = if sh.closed { n.stroke.align } else { d::StrokeAlign::Center };` forces `StrokeAlign::Center` (`width = w`, `StrokeClip::None` at `outline.rs:304-308`) across all subpaths when `sh.closed` is `false`.
   - `crates/designos-doc/src/layout_constraints.rs:197` (base `54f0c883`, `live_stroke_reproduces_bake`): `&& (centered || net.open_chain_ends().is_empty())` refuses to drop `stroke_geometry` on resize whenever `stroke.align != StrokeAlign::Center` and the vector network has open chains.
   - Note: `crates/designos-render/src/vectorops.rs:553-622` (`outline_stroke_along`) already separates closed subpaths (`vectorops.rs:574-580`, expanded at `2.0 * w` and clipped against `fill` with `OverlayRule::Intersect` for `Inside` and `OverlayRule::Difference` for `Outside` at `vectorops.rs:597-619`) from open subpaths (`vectorops.rs:582-595`, expanded centered at `w`).

2. **Where the node join wins over per-vertex joins:**
   - `crates/designos-render/src/outline.rs:66-91` (`node_stroke_style`): constructs a single `sk::Stroke` whose `line_join` is mapped from `n.stroke.join` (`outline.rs:76-80`); `stroke_plan` (`outline.rs:313-315`) and `Renderer::make_stroke` (`crates/designos-render/src/lib.rs:832-838`) stroke the entire node with that single `sk::Stroke`, ignoring `Vertex::join` (`crates/designos-doc/src/vector.rs:44`).
   - `crates/designos-render/src/vectorops.rs:218-246` (`sk_stroke` / `sk_stroke_with_join`): base `sk_stroke` built `st_center` and `st_double` solely from `stroke.join`, ignoring `v.join` on `VectorNetwork::vertices`.
   - `crates/designos-doc/src/layout_constraints.rs:195` (base `54f0c883`, `live_stroke_reproduces_bake`): `&& net.vertices.iter().all(|v| v.cap.is_none_or(|c| c == stroke.cap) && v.join.is_none_or(|j| j == stroke.join))` kept the scaled bake whenever any vertex had `v.join` differing from `stroke.join`.

3. **Where the arrow cap name is recorded on import and dropped on REST export:**
   - Recorded on REST import: `crates/designos-fig/src/rest_import.rs:28-34` (`unmodelled_stroke_cap`, inspecting `v["strokeCap"]` and `v["vectorNetwork"]["vertices"][..]["strokeCap"]`) and `crates/designos-fig/src/rest_import.rs:1317-1321` (recording `n.fig_metadata.insert("strokeCap".into(), Value::String(cap.into()))` while `rest_import.rs:1312-1316` maps `n.stroke.cap` to `StrokeCap::None` and `crates/designos-fig/src/vecnet.rs:14-21, 77` maps `vx.cap` to `None`).
   - Recorded on `.fig` import and preserved on `.fig` export: `crates/designos-fig/src/fig.rs:1098-1100, 1129-1140` (`extract_fig_metadata` records node `"strokeCap"` or `vectorData.styleOverrideTable[..]["strokeCap"]` into `meta["strokeCap"]`) and `crates/designos-fig/src/fig_export.rs:1451-1457` (`crates/designos-fig/src/fig_export/carrier.rs:15-16`) writes it back.
   - Dropped on REST export on base `54f0c883`: `crates/designos-fig/src/export.rs:542-550` wrote `"NONE" | "ROUND" | "SQUARE"` from `node.stroke.cap` without checking `node.fig_metadata.get("strokeCap")`, and `crates/designos-fig/src/export.rs:682-684` / `crates/designos-fig/src/vecnet.rs:109-137` (`network_to_json`) only wrote `"strokeCap"` from `v.cap: Option<StrokeCap>`.

4. **Where CPU and GPU pick vertex caps versus the node cap:**
   - `crates/designos-render/src/vectorops.rs:442-473` (`vertex_caps`): on base `54f0c883`, returned `Some(...)` whenever `net.vertices.iter().any(|v| v.cap.is_some())`, even when every vertex cap equalled `n.stroke.cap` (and `outline_stroke_along` similarly checked `n.vertices.iter().any(|v| v.cap.is_some())`).
   - `crates/designos-render/src/lib.rs:832-838` (`Renderer::make_stroke`) and `crates/designos-render/src/lib.rs:916-921` (`Renderer::draw_strokes`): on CPU, when `vertex_caps(n).is_some()`, forces `st.line_cap = sk::LineCap::Butt` (`lib.rs:835`) and fills 48-gon polygon caps from `vertex_caps(n)` (`lib.rs:916-921`).
   - `crates/designos-render/src/outline.rs:66-91, 278-316` (`node_stroke_style` / `stroke_plan`) and `crates/designos-gpu/src/geom.rs:266-277` (`NodeGeom::stroke_mesh`): on GPU, `stroke_plan` uses `node_stroke_style(n, width)` (`n.stroke.cap` directly on `sk::Stroke`) when `vertex_caps(n)` is `None`.

---

### 1.2 Kit Counts (`out/kit-clone.fig`)

Cloned `/Users/jangtrinh/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig` via `cp -c` to `out/kit-clone.fig`:

- **Initial 7-page import counts (`plans/reports/agy-1575-resize-strokes-live-evidence/step1-kit-initial-counts.log:9-15`):**
  - `initial_total_vectors = 33325` (`step1-kit-initial-counts.log:9`)
  - `initial_valid_net_and_bake = 17556` (`step1-kit-initial-counts.log:10`)
  - **`initial_open_chain_aligned_count = 83`** (`step1-kit-initial-counts.log:11`, `step1-kit-counts.log:183, 190-272`): `51` `"Earmark"` fold vectors inside the `"File type icon"` component set (`NodeId(12749)`, `source_id = "4916:411695"`) and `32` open-arc vectors inside `"Company logo"` (`NodeId(2738)`, `source_id = "7531:71186"`).
  - **`initial_mixed_vertex_joins_count = 63`** (`step1-kit-initial-counts.log:12`; `118` across all `31` hydrated pages in `step1-kit-counts.log:184, 275-392`): `"Hand-drawn arrow"` vectors mixing `Some(StrokeJoin::Miter)` on shaft vertices with `Some(StrokeJoin::Round)` on arrowhead vertices while `n.stroke.join == StrokeJoin::Round`.
  - `initial_arrow_cap_meta = 732` (`step1-kit-initial-counts.log:13`; `3125` across all `31` hydrated pages in `step1-kit-counts.log:185`).
  - `initial_vertex_caps_equal_node = 6352` and `initial_vertex_caps_differ = 0` (`step1-kit-initial-counts.log:14-15`; `39700` and `0` across all `31` hydrated pages in `step1-kit-counts.log:186-187`).

- **Three named `"File type"` glyphs (`step1-kit-counts.log:222, 223, 237`, `step1-kit-initial-counts.log:16-87`):**
  1. `NodeId(12753)` (`source_id = "4916:411445"`, `name = "Earmark"`, inside `NodeId(12751)` `"Page"` `4916:411443` of `NodeId(12750)` `"File type=Image / IMG, Type=Default"` `4916:411661`, paired with closed outline `NodeId(12752)` `"Page background"` `4916:411444`, `align = Inside, weight = 1.5`) (`step1-kit-initial-counts.log:16-42`).
  2. `NodeId(12771)` (`source_id = "4916:411433"`, `name = "Earmark"`, inside `NodeId(12769)` `"Page"` `4916:411431` of `NodeId(12768)` `"File type=Image / JPG, Type=Default"` `4916:411659`, paired with closed outline `NodeId(12770)` `"Page background"` `4916:411432`, `align = Inside, weight = 1.5`) (`step1-kit-initial-counts.log:43-65`).
  3. `NodeId(13029)` (`source_id = "4902:412664"`, `name = "Earmark"`, inside `NodeId(13027)` `"Page"` `4902:412662` of `NodeId(13026)` `"File type=Simple / PDF, Type=Default"` `4916:411568`, paired with closed outline `NodeId(13028)` `"Page background"` `4902:412663`, `align = Inside, weight = 1.5`) (`step1-kit-initial-counts.log:66-88`).

- **Three named `"Arrow"` illustrations (`step1-kit-counts.log:275-277, 403-421`):**
  1. `NodeId(43238)` (`source_id = "2628:400953"`, `name = "Vector"`, inside `NodeId(43237)` `"Arrow=16"` `2628:400996` of `NodeId(43236)` `"Hand-drawn arrow"` `2628:400997`, `size = (245.07, 170.64)`, `align = Center, weight = 3.0, cap = Round, join = Round`, `v[0..=4] join = Some(Miter)`, `v[5..=7] join = Some(Round)`) (`step1-kit-counts.log:275, 403-421`).
  2. `NodeId(43240)` (`source_id = "2628:400923"`, `name = "Vector"`, inside `NodeId(43239)` `"Arrow=21"` `2628:400966` of `NodeId(43236)` `"Hand-drawn arrow"` `2628:400997`, `size = (229.27, 122.08)`, `align = Center, weight = 3.0, cap = Round, join = Round`, `v[0..=5] join = Some(Miter)`, `v[6..=8] join = Some(Round)`) (`step1-kit-counts.log:276`).
  3. `NodeId(43242)` (`source_id = "2628:400951"`, `name = "Vector"`, inside `NodeId(43241)` `"Arrow=34"` `2628:400994` of `NodeId(43236)` `"Hand-drawn arrow"` `2628:400997`, `size = (185.52, 158.38)`, `align = Center, weight = 3.0, cap = Round, join = Round`, `v[0..=3] join = Some(Miter)`, `v[4..=6] join = Some(Round)`) (`step1-kit-counts.log:277`).

---

### 1.3 Figma's Rule (REA + Local Figma Desktop Cache + Kit `strokeGeometry`)

- `rea inspect-macho "/Applications/Figma.app/Contents/MacOS/Figma" --format json` -> evidence bundle `ev_0cd5f3f9cd98cd6b00b4abca25c0efd5ef3b4108880aca82eba125aa3cc16c4a`.
- **Per-vertex `strokeJoin` overrides the node's `strokeJoin`:**
  - In `out/re-figma-1575/6413-8e031fdc4fad2bf8.min.js` at byte offset `0xa65edd..0xa66775` (`10903261..10905461`), `b(e)` seeds style `0` with `{strokeCap: e.strokeCap, strokeJoin: e.strokeJoin, cornerRadius: e.cornerRadius, handleMirroring: e.handleMirroring}` (`0xa65f65` / `10903397`), merges `styleOverrideTable` entries via `strokeJoin: "strokeJoin" in e ? e.strokeJoin : d.strokeJoin` (`0xa66022..0xa66130` / `10903586..10903856`), and assigns each vertex `t.get(styleID) || t.get(0)` (`0xa6640d..0xa6645a` / `10904589..10904666`).
  - Confirmed on `NodeId(43238)` (`"Arrow=16"`, `2628:400953`, `n.stroke.join = Round`, `n.stroke.weight = 3.0`) in `step1-kit-counts.log:403-421`: shaft vertices `v[1] = (95.40, 16.11)`, `v[2] = (38.33, 57.58)`, `v[3] = (140.79, 96.03)` (`join = Some(Miter)`) produce triangular miter bevel/join wedges (`MoveTo(95.400024, 16.108881), LineTo(96.76679, 15.490868), LineTo(96.76671, 15.490671), Close` at `step1-kit-counts.log:406`), whereas arrowhead tip vertex `v[6] = (242.64, 170.64)` (`join = Some(Round)`) produces a cubic circular arc join (`MoveTo(242.64001, 170.63889), LineTo(242.2086, 172.0755), CubicTo(..), CubicTo(..), Close` at `step1-kit-counts.log:409`).
- **Stroke alignment per subpath (closed regions keep `Inside`/`Outside`; open chains stroke `Center`):**
  - In `out/re-figma-1575/compiled_wasm.wasm`, `fullscreen/lib/geometry/FGVectorNetwork.cpp` (`0x2f13ec0`, linear memory `986691`: `getRunChains` `wasm-function[8084]` at `0x545531..0x546aeb`, `findTopologyDataInto` `wasm-function[18240]` at `0xfb0d5b..0xfb268f`, `visitSegments` `wasm-function[31294]` at `0x1e003a3..0x1e00f78`) and `generateFillableStrokeGeometryFromFillNetwork` (`0x2f26c25`, linear memory `1063848`, `wasm-function[11231]` at `0x8aa82e..0x8ad6d1` and `wasm-function[13077]` at `0xad109d..0xad3db2`) stroke closed region loops with the node's `Inside`/`Outside` alignment and open chains with `Center` alignment.
  - Confirmed on `NodeId(12753)` (`"Earmark"`, `4916:411445`, open L-chain `(0,0) -> (0,11.5) -> (11.5,11.5)`, `align = Inside, weight = 1.5`) in `step1-kit-initial-counts.log:35-42`: Figma's baked `strokeGeometry` spans `x: -0.75 .. +0.75` around the vertical leg `x = 0.0` and `y: 10.75 .. 12.25` around the horizontal leg `y = 11.5` (`±0.75 = ±weight/2`, i.e., `Center`-aligned), and on `NodeId(2988)` (`"Shape"`, `7531:71436`, open semicircle of radius `10.0`, `align = Inside, weight = 5.0`) in `step1-kit-counts.log:394-401`, `strokeGeometry` spans radius `7.5 .. 12.5` (`10.0 ± 2.5 = 10.0 ± weight/2`, i.e., `Center`-aligned).

---

## Epistemic Ledger & Skills Loaded

### Skills Loaded (`brief-common-argon.txt:27-30, 39`)
- `es:dev` (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`) -> `es:feature-dev` (`/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`): executed directly in the DEV phase with the round brief as the accepted plan.
- `es:fable-thinking` (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`): enforced `FACT` / `INFERENCE` / `ASSUMPTION` tracking, `PREDICT` before every consequential action, red-first test verification on base `54f0c883`, and 3-mutant falsification after green null control.
- `es:rust` (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`): applied zero-clone slice borrowing (`std::slice::from_ref`) and allocation-minimal subpath iteration in `expand_subpath` (`crates/designos-render/src/vectorops.rs:298-363`).
- `es:rust-gpu-canvas` (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`): unified the mixed-subpath alignment and per-vertex join stroke expansion through `outline::stroke_plan` (`crates/designos-render/src/outline.rs:300-315`) so `NodeGeom::stroke_mesh` (`crates/designos-gpu/src/geom.rs:273`) and `Renderer::draw_strokes` (`crates/designos-render/src/lib.rs:894-915`) share one geometry plan without touching `SceneCompiler::interactive()`.

### Epistemic Ledger
- **`FACT`:** On base `54f0c883`, `live_stroke_reproduces_bake` (`crates/designos-doc/src/layout_constraints.rs:195-197`) kept `stroke_geometry` whenever `stroke.align != StrokeAlign::Center && !net.open_chain_ends().is_empty()` or any vertex had `v.join != Some(stroke.join)`, causing all `83` open-chain `Inside`-stroke vectors and `63` mixed-join arrow vectors on the initial 7 pages of the Untitled UI kit (`step1-kit-initial-counts.log:11-12`) to thicken their strokes 2x when resized 2x (`kit-base-render.log:16-21`).
- **`FACT`:** On base `54f0c883`, `export_rest_json` (`crates/designos-fig/src/export.rs:542-550, 682-684`) dropped `fig_metadata["strokeCap"]` (`"ARROW_LINES"`, `"ARROW_EQUILATERAL"`), causing a REST round-tripped arrow vector to lose its marker (`red-fig.log:21-26`) and drop its arrowhead bake on resize.
- **`FACT`:** On base `54f0c883`, `vertex_caps` (`crates/designos-render/src/vectorops.rs:442-473`) returned `Some(...)` whenever `v.cap.is_some()` even if every vertex cap equalled `n.stroke.cap` (`6352` vectors on initial 7 pages, `39700` across all 31 pages in `step1-kit-counts.log:186`), sending CPU through 48-gon cap polygons while GPU used `node_stroke_style` (`red-render.log:24-25`, `red-gpu.log:10-11`).
- **`INFERENCE`:** Splitting each cubic curve of a multi-segment subpath at $t = 0.5$ via de Casteljau subdivision (`split_cubic_half` at `crates/designos-render/src/vectorops.rs:253-262`) and stroking each two-half-segment corner piece with `sk_stroke_with_join(..., StrokeCap::None, joins[k])` (`vectorops.rs:318-344`) plus outward endpoint caps (`push_outward_cap` at `vectorops.rs:264-294, 345-361`) isolates each interior vertex's `StrokeJoin` without disturbing adjacent corners. Confirmed by pixel-exact Miter/Round corner checks (`green-render.log:10`, `green-gpu.log:10`) and Mutant 2 (`mutant-2.log:15-16, 50-51`).
- **`ASSUMPTION`:** Matching subpath corner points `cs[k][0]` to `VectorNetwork::vertices` within Euclidean distance `1e-2` (`crates/designos-render/src/vectorops.rs:310-311`) uniquely identifies the vertex for that corner on valid vector networks.
- **Weakest link:** Distance-based vertex lookup (`hypot <= 1e-2` in `expand_subpath` at `crates/designos-render/src/vectorops.rs:311`) could pick the first matching vertex if a degenerate self-intersecting subpath places two vertices with different `StrokeJoin` overrides within `0.01` document units of each other.

---

## Files Modified (`gate-01-fmt.log`, `git diff --stat`)

| File | Lines Changed | Purpose |
|---|---|---|
| [`crates/designos-doc/src/layout_constraints.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1575-resize-strokes-live/crates/designos-doc/src/layout_constraints.rs#L183-L197) | `183-197` (`+6 / -7`) | Extend `live_stroke_reproduces_bake` to drop `stroke_geometry` for `Inside`/`Outside` strokes with open chains and for per-vertex join overrides. |
| [`crates/designos-fig/src/export.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1575-resize-strokes-live/crates/designos-fig/src/export.rs#L542-L812) | `542-550`, `682-691`, `810-812` (`+15 / -1`) | Preserve unmodelled arrow cap names from `fig_metadata["strokeCap"]` on the node and on `vectorNetwork.vertices` during REST export. |
| [`crates/designos-render/src/vectorops.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1575-resize-strokes-live/crates/designos-render/src/vectorops.rs#L218-L622) | `218-363`, `442-520`, `558-615` (`+170 / -18`) | Add `sk_stroke_with_join`, `split_cubic_half`, `push_outward_cap`, `expand_subpath`, `has_per_vertex_joins`, `has_mixed_subpath_align`, `expand_node_subpaths`, `mixed_subpath_stroke_outline`; restrict `vertex_caps` and `outline_stroke_along` to caps that differ from `n.stroke.cap`. |
| [`crates/designos-render/src/outline.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1575-resize-strokes-live/crates/designos-render/src/outline.rs#L300-L316) | `300-316` (`+8 / -0`) | Route mixed-subpath-alignment and per-vertex-join strokes through `mixed_subpath_stroke_outline` and `expand_node_subpaths` in `stroke_plan`. |
| [`crates/designos-render/src/lib.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1575-resize-strokes-live/crates/designos-render/src/lib.rs#L894-L915) | `894-915` (`+22 / -0`) | Share `outline::stroke_plan` in `Renderer::draw_strokes` when `has_mixed_subpath_align` or `has_per_vertex_joins` is true. |
| [`crates/designos-fig/tests/resized_vector_stroke_model.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1575-resize-strokes-live/crates/designos-fig/tests/resized_vector_stroke_model.rs#L156-L187) | `156-187` (`+22 / -4`) | Update `a_vertex_cap_of_its_own_and_dashes_inside_keep_the_scaled_bake_while_a_vertex_join_drops_it` and add `an_arrow_cap_survives_a_rest_round_trip_and_keeps_its_bake_on_a_resize`. |
| [`crates/designos-render/tests/resized_vector_stroke_weight.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1575-resize-strokes-live/crates/designos-render/tests/resized_vector_stroke_weight.rs#L350-L504) | `350-504` (`+139 / -21`) | Update `an_inside_or_outside_stroke_of_a_network_with_an_open_chain_drops_its_bake_and_strokes_live` and add `file_type_network_resized_2x_strokes_closed_outline_inside_and_open_fold_centered`, `mixed_miter_and_round_join_path_resized_2x_keeps_both_joins`, and `equal_cap_open_stroke_draws_through_the_same_path_as_node_cap`. |
| [`crates/designos-gpu/tests/resized_shapes.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1575-resize-strokes-live/crates/designos-gpu/tests/resized_shapes.rs#L337-L491) | `337-491` (`+140 / -0`) | Add GPU=CPU parity tests `resized_inside_and_outside_strokes_with_an_open_chain_drop_their_bake_and_stroke_live_like_cpu`, `file_type_network_resized_2x_strokes_outline_inside_and_fold_centered_like_cpu`, `mixed_miter_and_round_join_path_resized_2x_keeps_both_joins_like_cpu`, and `equal_cap_open_stroke_draws_identically_on_cpu_and_gpu`. |

---

## Acceptance Verification (`brief-agy-1575-resize-strokes-live.txt:26-41`)

### Acceptance 1 — Tests (RED on Base `54f0c883`, GREEN on Head, Null Controls)

#### 1A. RED Runs on Base `54f0c883`

- **`designos-fig` (`plans/reports/agy-1575-resize-strokes-live-evidence/red-fig.log:17-38`):**
```text
thread 'a_vertex_cap_of_its_own_and_dashes_inside_keep_the_scaled_bake_while_a_vertex_join_drops_it' (88656060) panicked at crates/designos-fig/tests/resized_vector_stroke_model.rs:164:5:
vertex join that differs from the node's: the stale outline is dropped
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

thread 'an_arrow_cap_survives_a_rest_round_trip_and_keeps_its_bake_on_a_resize' (88656063) panicked at crates/designos-fig/tests/resized_vector_stroke_model.rs:182:9:
assertion `left == right` failed: node arrow cap: the arrow cap survives the REST round trip
  left: None
 right: Some("ARROW_LINES")
test a_vertex_cap_of_its_own_and_dashes_inside_keep_the_scaled_bake_while_a_vertex_join_drops_it ... FAILED
test an_arrow_cap_survives_a_rest_round_trip_and_keeps_its_bake_on_a_resize ... FAILED
...
test result: FAILED. 10 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.55s
```

- **`designos-render` (`plans/reports/agy-1575-resize-strokes-live-evidence/red-render.log:20-52`):**
```text
thread 'mixed_miter_and_round_join_path_resized_2x_keeps_both_joins' (88647562) panicked at crates/designos-render/tests/resized_vector_stroke_weight.rs:484:5:
mixed-join path drops its bake on 2x resize

thread 'equal_cap_open_stroke_draws_through_the_same_path_as_node_cap' (88647560) panicked at crates/designos-render/tests/resized_vector_stroke_weight.rs:522:5:
equal per-vertex caps do not trigger the separate cap polygon path

thread 'an_inside_or_outside_stroke_of_a_network_with_an_open_chain_drops_its_bake_and_strokes_live' (88647558) panicked at crates/designos-render/tests/resized_vector_stroke_weight.rs:375:9:
Inside: the baked outline is dropped on resize

thread 'file_type_network_resized_2x_strokes_closed_outline_inside_and_open_fold_centered' (88647561) panicked at crates/designos-render/tests/resized_vector_stroke_weight.rs:433:5:
file-type network drops its bake on 2x resize
...
test result: FAILED. 7 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

- **`designos-gpu` (`plans/reports/agy-1575-resize-strokes-live-evidence/red-gpu.log:10-56`):**
```text
thread 'equal_cap_open_stroke_draws_identically_on_cpu_and_gpu' (88648223) panicked at crates/designos-gpu/tests/resized_shapes.rs:491:5:
equal per-vertex caps use the node cap path

thread 'mixed_miter_and_round_join_path_resized_2x_keeps_both_joins_like_cpu' (88648225) panicked at crates/designos-gpu/tests/resized_shapes.rs:455:5:
mixed-join path drops its bake on 2x resize

thread 'resized_inside_and_outside_strokes_with_an_open_chain_drop_their_bake_and_stroke_live_like_cpu' (88648229) panicked at crates/designos-gpu/tests/resized_shapes.rs:343:9:
Inside: the baked outline is dropped

thread 'file_type_network_resized_2x_strokes_outline_inside_and_fold_centered_like_cpu' (88648224) panicked at crates/designos-gpu/tests/resized_shapes.rs:402:5:
file-type network drops its bake on 2x resize
...
test result: FAILED. 5 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
```

#### 1B. GREEN Runs on Head

- **`designos-fig` (`plans/reports/agy-1575-resize-strokes-live-evidence/green-fig.log:7-21`):**
```text
running 12 tests
test the_kit_icon_drops_its_bake_and_draws_its_own_weight ... ok
test an_arrow_cap_on_the_node_keeps_the_scaled_bake ... ok
test an_arrow_cap_on_a_vertex_keeps_the_scaled_bake ... ok
test fig_icon_without_an_arrow_cap_drops_its_bake ... ok
test a_vertex_cap_equal_to_the_nodes_and_dashes_on_a_centered_stroke_still_drop_the_bake ... ok
test a_brush_and_side_weights_keep_the_scaled_bake ... ok
test a_vertex_cap_of_its_own_and_dashes_inside_keep_the_scaled_bake_while_a_vertex_join_drops_it ... ok
test fig_arrow_caps_on_the_node_and_on_a_vertex_keep_the_scaled_bake ... ok
test an_arrow_cap_survives_a_rest_round_trip_and_keeps_its_bake_on_a_resize ... ok
test a_saved_fig_file_of_a_plain_icon_carries_no_cap_marker_and_still_drops_its_bake ... ok
test an_arrow_cap_survives_a_saved_fig_file_and_the_reopened_vector_keeps_its_bake_on_a_resize ... ok
test an_arrow_vector_in_an_instance_keeps_its_bake_when_the_instance_is_resized ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.57s
```

- **`designos-render` (`plans/reports/agy-1575-resize-strokes-live-evidence/green-render.log:7-20`):**
```text
running 11 tests
test unresized_kit_vector_renders_are_hashed_into_a_file ... ok
test unresized_imported_vector_renders_unchanged ... ok
test mixed_miter_and_round_join_path_resized_2x_keeps_both_joins ... ok
test stroke_follows_the_network_while_the_fill_keeps_the_outline ... ok
test syncing_the_main_keeps_the_plain_resize_of_an_instance_child_with_a_size_override ... ok
test equal_cap_open_stroke_draws_through_the_same_path_as_node_cap ... ok
test plain_resize_2x_and_2x1_keeps_stroke_weight_on_both_axes ... ok
test stroke_only_vector_whose_outline_is_its_path_draws_once_after_resize ... ok
test an_inside_stroke_of_a_closed_network_still_drops_its_bake_and_stays_inside ... ok
test an_inside_or_outside_stroke_of_a_network_with_an_open_chain_drops_its_bake_and_strokes_live ... ok
test file_type_network_resized_2x_strokes_closed_outline_inside_and_open_fold_centered ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

- **`designos-gpu` (`plans/reports/agy-1575-resize-strokes-live-evidence/green-gpu.log:7-18`):**
```text
running 9 tests
test file_type_network_resized_2x_strokes_outline_inside_and_fold_centered_like_cpu ... ok
test resized_imported_vector_keeps_stroke_weight_and_matches_cpu ... ok
test mixed_miter_and_round_join_path_resized_2x_keeps_both_joins_like_cpu ... ok
test equal_cap_open_stroke_draws_identically_on_cpu_and_gpu ... ok
test resized_vector_strokes_its_network_and_fills_its_outline_like_cpu ... ok
test resized_flattened_star_matches_cpu ... ok
test resized_union_matches_cpu ... ok
test resized_imported_ellipse_and_star_match_cpu ... ok
test resized_inside_and_outside_strokes_with_an_open_chain_drop_their_bake_and_stroke_live_like_cpu ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.27s
```

#### 1C. Null Controls
- **`designos-render --test vectorops` (`plans/reports/agy-1575-resize-strokes-live-evidence/null-control-vectorops.log:8-33`):** `23 passed; 0 failed`.
- **`designos-gpu --test fidelity` (`plans/reports/agy-1575-resize-strokes-live-evidence/null-control-fidelity.log:8-33`):** `23 passed; 0 failed`.
- **Unresized kit vector pixel hash diff (`plans/reports/agy-1575-resize-strokes-live-evidence/kit-hashes-diff.log:7-15`):**
```text
running 1 test
test unresized_kit_vector_renders_are_hashed_into_a_file ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 32.37s

DIFF_EXIT=0
     318 /Users/jangtrinh/orca/workspaces/design-os-studio/.agy-1575-tmp/tmp/base-kit-hashes.txt
     318 /Users/jangtrinh/orca/workspaces/design-os-studio/.agy-1575-tmp/tmp/head-kit-hashes.txt
     636 total
```

---

### Acceptance 2 — Three Mutants Killed After Green Null Control

1. **Mutant 1 — Open chains forced back to the node alignment (`plans/reports/agy-1575-resize-strokes-live-evidence/mutant-1.log:15-42, 58-103`):**
   - Mutation: `closed_subs.push(sub)` unconditionally when `aligned` in `outline_stroke_along` (`crates/designos-render/src/vectorops.rs:574`).
   - Killed by `4` tests (`2` in `designos-render` at `mutant-1.log:39-42`, `2` in `designos-gpu` at `mutant-1.log:100-103`):
```text
thread 'an_inside_or_outside_stroke_of_a_network_with_an_open_chain_drops_its_bake_and_strokes_live' (88943519) panicked at crates/designos-render/tests/resized_vector_stroke_weight.rs:368:9:
assertion `left == right` failed: Inside: one 2px centered band on the open chain
  left: []
 right: [2]
thread 'file_type_network_resized_2x_strokes_closed_outline_inside_and_open_fold_centered' (88943522) panicked at crates/designos-render/tests/resized_vector_stroke_weight.rs:424:5:
assertion `left == right` failed: fold leg is 1.5px centered, not 3px at x = 58
  left: [139, 142, 147, 255]
 right: [255, 255, 255, 255]
...
thread 'resized_inside_and_outside_strokes_with_an_open_chain_drop_their_bake_and_stroke_live_like_cpu' (88947521) panicked at crates/designos-gpu/tests/resized_shapes.rs:349:9:
assertion `left == right` failed: CPU Inside: one 2px centered band on the open chain
  left: []
 right: [2]
thread 'file_type_network_resized_2x_strokes_outline_inside_and_fold_centered_like_cpu' (88947516) panicked at crates/designos-gpu/tests/resized_shapes.rs:406:5:
CPU: 1.5px centered stroke on fold leg
```

2. **Mutant 2 — Node join used for every vertex (`plans/reports/agy-1575-resize-strokes-live-evidence/mutant-2.log:15-34, 50-95`):**
   - Mutation: `expand_subpath` (`crates/designos-render/src/vectorops.rs:298`) always strokes with `sk_stroke(stroke, width, base_cap)` (`stroke.join`).
   - Killed by `2` tests (`1` in `designos-render` at `mutant-2.log:32-34`, `1` in `designos-gpu` at `mutant-2.log:93-95`):
```text
thread 'mixed_miter_and_round_join_path_resized_2x_keeps_both_joins' (88974176) panicked at crates/designos-render/tests/resized_vector_stroke_weight.rs:471:5:
Miter corner keeps its sharp outer tip at (14, 14)
...
thread 'mixed_miter_and_round_join_path_resized_2x_keeps_both_joins_like_cpu' (88975658) panicked at crates/designos-gpu/tests/resized_shapes.rs:458:5:
CPU: Miter corner sharp at (14, 14), Round corner rounded at (53, 14)
```

3. **Mutant 3 — REST export drops the cap name (`plans/reports/agy-1575-resize-strokes-live-evidence/mutant-3.log:14-37`):**
   - Mutation: `unmodelled_stroke_cap` (`crates/designos-fig/src/export.rs:810`) returns `None`.
   - Killed by `an_arrow_cap_survives_a_rest_round_trip_and_keeps_its_bake_on_a_resize` (`mutant-3.log:14-37`):
```text
thread 'an_arrow_cap_survives_a_rest_round_trip_and_keeps_its_bake_on_a_resize' (88992015) panicked at crates/designos-fig/tests/resized_vector_stroke_model.rs:182:9:
assertion `left == right` failed: node arrow cap: the arrow cap survives the REST round trip
  left: None
 right: Some("ARROW_LINES")
...
test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.58s
```

---

### Acceptance 3 — Kit Evidence (Headless CPU and GPU Before/After 2x Resize Crops)

All 6 step-2 kit nodes were resized 2x via `apply_constrained_geometry` and rendered headlessly at `zoom = 1.0` on base (`54f0c883`, `kit-base-render.log:15-24`) and head (`kit-head-render.log:9-18`), then composited side by side (`before` left, 4px divider, `after` right; `kit-crops-summary.log:1-12`):

```text
base file-type-img: bg_id=NodeId(12752) bg_bake=false ear_id=NodeId(12753) ear_bake=true weight=1.5
base file-type-jpg: bg_id=NodeId(12770) bg_bake=false ear_id=NodeId(12771) ear_bake=true weight=1.5
base file-type-pdf: bg_id=NodeId(13028) bg_bake=false ear_id=NodeId(13029) ear_bake=true weight=1.5
base arrow-16: id=NodeId(43238) bake=true weight=3 size=(490.14008, 341.27777)
base arrow-21: id=NodeId(43240) bake=true weight=3 size=(458.54004, 244.16774)
base arrow-34: id=NodeId(43242) bake=true weight=3 size=(371.03198, 316.7594)
---
head file-type-img: bg_id=NodeId(12752) bg_bake=false ear_id=NodeId(12753) ear_bake=false weight=1.5
head file-type-jpg: bg_id=NodeId(12770) bg_bake=false ear_id=NodeId(12771) ear_bake=false weight=1.5
head file-type-pdf: bg_id=NodeId(13028) bg_bake=false ear_id=NodeId(13029) ear_bake=false weight=1.5
head arrow-16: id=NodeId(43238) bake=false weight=3 size=(490.14008, 341.27777)
head arrow-21: id=NodeId(43240) bake=false weight=3 size=(458.54004, 244.16774)
head arrow-34: id=NodeId(43242) bake=false weight=3 size=(371.03198, 316.7594)
---
file-type-img-cpu: crop=(0, 0, 80, 96) size=(164, 96) full_ink=(508->596) crop_ink=(508->596)
file-type-img-gpu: crop=(0, 0, 80, 96) size=(164, 96) full_ink=(505->593) crop_ink=(505->593)
file-type-jpg-cpu: crop=(0, 0, 80, 96) size=(164, 96) full_ink=(508->596) crop_ink=(508->596)
file-type-jpg-gpu: crop=(0, 0, 80, 96) size=(164, 96) full_ink=(505->593) crop_ink=(505->593)
file-type-pdf-cpu: crop=(0, 0, 80, 96) size=(164, 96) full_ink=(508->596) crop_ink=(508->596)
file-type-pdf-gpu: crop=(0, 0, 80, 96) size=(164, 96) full_ink=(505->593) crop_ink=(505->593)
arrow-16-cpu: crop=(142, 115, 262, 235) size=(244, 120) full_ink=(6847->3902) crop_ink=(850->480)
arrow-16-gpu: crop=(142, 115, 262, 235) size=(244, 120) full_ink=(6598->3662) crop_ink=(819->454)
arrow-21-cpu: crop=(6, 116, 126, 236) size=(244, 120) full_ink=(5387->3056) crop_ink=(1281->726)
arrow-21-gpu: crop=(6, 116, 126, 236) size=(244, 120) full_ink=(5206->2868) crop_ink=(1220->667)
arrow-34-cpu: crop=(142, 161, 262, 281) size=(244, 120) full_ink=(4653->2665) crop_ink=(994->571)
arrow-34-gpu: crop=(142, 161, 262, 281) size=(244, 120) full_ink=(4511->2499) crop_ink=(959->521)
```

| Kit Node | CPU Side-by-Side Crop (`before` \| `after`) | GPU Side-by-Side Crop (`before` \| `after`) |
|---|---|---|
| `NodeId(12752)` + `NodeId(12753)` (`File type=Image / IMG`) | [`kit-file-type-img-cpu-before-after.png`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1575-resize-strokes-live/plans/reports/agy-1575-resize-strokes-live-evidence/kit-file-type-img-cpu-before-after.png) | [`kit-file-type-img-gpu-before-after.png`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1575-resize-strokes-live/plans/reports/agy-1575-resize-strokes-live-evidence/kit-file-type-img-gpu-before-after.png) |
| `NodeId(12770)` + `NodeId(12771)` (`File type=Image / JPG`) | [`kit-file-type-jpg-cpu-before-after.png`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1575-resize-strokes-live/plans/reports/agy-1575-resize-strokes-live-evidence/kit-file-type-jpg-cpu-before-after.png) | [`kit-file-type-jpg-gpu-before-after.png`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1575-resize-strokes-live/plans/reports/agy-1575-resize-strokes-live-evidence/kit-file-type-jpg-gpu-before-after.png) |
| `NodeId(13028)` + `NodeId(13029)` (`File type=Simple / PDF`) | [`kit-file-type-pdf-cpu-before-after.png`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1575-resize-strokes-live/plans/reports/agy-1575-resize-strokes-live-evidence/kit-file-type-pdf-cpu-before-after.png) | [`kit-file-type-pdf-gpu-before-after.png`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1575-resize-strokes-live/plans/reports/agy-1575-resize-strokes-live-evidence/kit-file-type-pdf-gpu-before-after.png) |
| `NodeId(43238)` (`Arrow=16`, `2628:400953`) | [`kit-arrow-16-cpu-before-after.png`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1575-resize-strokes-live/plans/reports/agy-1575-resize-strokes-live-evidence/kit-arrow-16-cpu-before-after.png) | [`kit-arrow-16-gpu-before-after.png`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1575-resize-strokes-live/plans/reports/agy-1575-resize-strokes-live-evidence/kit-arrow-16-gpu-before-after.png) |
| `NodeId(43240)` (`Arrow=21`, `2628:400923`) | [`kit-arrow-21-cpu-before-after.png`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1575-resize-strokes-live/plans/reports/agy-1575-resize-strokes-live-evidence/kit-arrow-21-cpu-before-after.png) | [`kit-arrow-21-gpu-before-after.png`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1575-resize-strokes-live/plans/reports/agy-1575-resize-strokes-live-evidence/kit-arrow-21-gpu-before-after.png) |
| `NodeId(43242)` (`Arrow=34`, `2628:400951`) | [`kit-arrow-34-cpu-before-after.png`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1575-resize-strokes-live/plans/reports/agy-1575-resize-strokes-live-evidence/kit-arrow-34-cpu-before-after.png) | [`kit-arrow-34-gpu-before-after.png`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1575-resize-strokes-live/plans/reports/agy-1575-resize-strokes-live-evidence/kit-arrow-34-gpu-before-after.png) |

---

### Acceptance 4 — Gates (`brief-agy-1575-resize-strokes-live.txt:30-40`)

1. **`cargo fmt --all -- --check; echo fmt_rc=$?` (`plans/reports/agy-1575-resize-strokes-live-evidence/gate-01-fmt.log:1`):**
```text
fmt_rc=0
```

2. **`cargo clippy --workspace --all-targets --locked; echo clippy_rc=$?` (`plans/reports/agy-1575-resize-strokes-live-evidence/gate-02-clippy.log:1-54`):**
```text
warning: `designos-app` (bin "designos") generated 2 warnings (2 duplicates)
warning: `designos-app` (bin "designos-studio") generated 2 warnings (run `cargo clippy --fix --bin "designos-studio" -p designos-app` to apply 2 suggestions)
warning: `designos-app` (bin "designos" test) generated 3 warnings (2 duplicates)
warning: `designos-app` (bin "designos-studio" test) generated 3 warnings (3 duplicates)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 40.37s
clippy_rc=0
```
*(Zero warnings in `designos-doc`, `designos-fig`, `designos-render`, `designos-gpu`, or `designos-mcp`; only `main`'s pre-existing `designos-app` warnings at `crates/designos-app/src/agent_permissions.rs:1176, 1194` and `crates/designos-app/src/shell/tests/inspector_var_binds.rs:1182`, `gate-02-clippy.log:17-52`.)*

3. **`cargo test -p designos-doc --locked` (`plans/reports/agy-1575-resize-strokes-live-evidence/gate-03-doc.log:766, 775, 786, 794, 800`):**
```text
test result: ok. 746 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.68s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

4. **`cargo test -p designos-fig --locked` (`plans/reports/agy-1575-resize-strokes-live-evidence/gate-04-fig.log:159, 740, 861, 867`):**
```text
test result: ok. 144 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.87s
...
     Running tests/resized_vector_stroke_model.rs (out/cargo/debug/deps/resized_vector_stroke_model-87b6562e1729e120)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.59s
...
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

5. **`cargo test -p designos-render --locked` (`plans/reports/agy-1575-resize-strokes-live-evidence/gate-05-render.log:72, 350, 489, 495`):**
```text
test result: ok. 56 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 32.87s
...
     Running tests/resized_vector_stroke_weight.rs (out/cargo/debug/deps/resized_vector_stroke_weight-236926c4c166f7cb)
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
...
     Running tests/vectorops.rs (out/cargo/debug/deps/vectorops-30cf93ca5695e39d)
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
```

6. **`cargo test -p designos-gpu --locked -- --skip large_file_compile_benchmarks` (`plans/reports/agy-1575-resize-strokes-live-evidence/gate-06-gpu.log:21, 67, 83-87, 176, 217`):**
```text
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
...
     Running tests/fidelity.rs (out/cargo/debug/deps/fidelity-e9d67385509b7a81)
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.18s
...
     Running tests/large_file_perf.rs (out/cargo/debug/deps/large_file_perf-c19c8fdc40311fbf)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s
...
     Running tests/resized_shapes.rs (out/cargo/debug/deps/resized_shapes-f43e2264302b4cce)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.41s
```

7. **`cargo test -p designos-mcp --locked` (`plans/reports/agy-1575-resize-strokes-live-evidence/gate-07-mcp.log:451, 643, 841, 847`):**
```text
test result: ok. 327 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.89s
...
     Running tests/resized_vector_via_update.rs (out/cargo/debug/deps/resized_vector_via_update-fe25817e7a75a15d)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
...
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
```

8. **`cargo test -p designos-app --bin designos --locked` (`plans/reports/agy-1575-resize-strokes-live-evidence/gate-08-app.log:4402-4420`) and single-test reruns (`plans/reports/agy-1575-resize-strokes-live-evidence/gate-08-app-reruns.log:9-25`):**
   - Full suite result: `4251 passed; 2 failed; 13 ignored` (`gate-08-app.log:4420`). The 2 failures were two of the brief's named known-flaky/long-TMPDIR tests (`brief-agy-1575-resize-strokes-live.txt:38`): `agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root` (`gate-08-app.log:4404-4407`) and `vecops::tests::two_deferred_text_commands_both_convert_in_order` (`gate-08-app.log:4410-4413`).
   - Both passed when rerun alone (`gate-08-app-reruns.log:9-25`):
```text
running 1 test
test agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4265 filtered out; finished in 0.00s
...
running 1 test
test vecops::tests::two_deferred_text_commands_both_convert_in_order ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4265 filtered out; finished in 1.48s
```

9. **`cargo test --workspace --locked --lib` (`plans/reports/agy-1575-resize-strokes-live-evidence/gate-09-workspace-lib.log:805, 833, 875, 1050, 1200, 1236, 1247, 1290, 1623, 1687, 1694, 1708`):**
```text
designos-doc:        746 passed; 0 failed; 0 ignored
designos-diff:        12 passed; 0 failed; 0 ignored
designos-eval:        27 passed; 0 failed; 0 ignored
designos-export:     164 passed; 0 failed; 1 ignored
designos-fig:        144 passed; 0 failed; 0 ignored
designos-fonts:       30 passed; 0 failed; 0 ignored
designos-gpu:          5 passed; 0 failed; 0 ignored
designos-library:     37 passed; 0 failed; 0 ignored
designos-mcp:        327 passed; 0 failed; 0 ignored
designos-render:      56 passed; 0 failed; 2 ignored
designos-svg:          1 passed; 0 failed; 0 ignored
designos-webcapture:   8 passed; 0 failed; 0 ignored
```

10. **`python3 -B scripts/zero_fake_ui_gate.py` (`plans/reports/agy-1575-resize-strokes-live-evidence/gate-10-zero-fake-ui.log:24-26`):**
```text
------------------------------------------------------------------------------------------------------------
 Gate Verdict: PASS (17/17 invariants satisfied)
============================================================================================================
zero_fake_ui_rc=0
```

---

### Acceptance 5 — Draft PR, Commit Log & `out/cargo` Cleanup (`brief-agy-1575-resize-strokes-live.txt:41`)

- **Draft PR URL:** `https://github.com/jangtrinh/design-os-studio/pull/1588`
- **`git log --oneline origin/main..HEAD`:**
```text
$ git log --oneline origin/main..HEAD
138d01f8 fix(vector): resized vectors keep their stroke weight when the stroke mixes alignments or joins (#1575)
```
- **`ls out/cargo` failing after cleanup (`plans/reports/agy-1575-resize-strokes-live-evidence/cleanup-out-cargo.log:1-2`):**
```text
$ ls out/cargo
ls: out/cargo: No such file or directory
ls_out_cargo_rc=1
```

---

## Assumptions & Unresolved Questions

- **Assumptions:**
  - When a vector node has `node.stroke.cap != StrokeCap::None` and carries a vertex-level unmodelled arrow cap in `node.fig_metadata["strokeCap"]` (`crates/designos-fig/src/rest_import.rs:28-34, 1317-1321`), writing that cap name onto the first vertex in `vectorNetwork.vertices` without a `"strokeCap"` (`crates/designos-fig/src/export.rs:684-690`) suffices for `import_rest_json` to recover `node.fig_metadata["strokeCap"]` on re-import while preserving the modelled `node.stroke.cap` (`crates/designos-fig/tests/resized_vector_stroke_model.rs:172-187`).
  - Matching subpath corner vertices by Euclidean distance `<= 1e-2` in document coordinates (`crates/designos-render/src/vectorops.rs:311`) accurately associates `Path` corner points with `VectorNetwork::vertices` entries.
- **Unresolved Questions:** None.

Status: COMPLETE
