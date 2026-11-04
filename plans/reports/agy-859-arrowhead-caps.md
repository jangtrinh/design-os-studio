# Arrowhead Stroke Caps on Lines and Vectors (#859 — Slice A & #1590)

## Step 1 — Pre-Edit Analysis & Reverse Engineering

### 1.1 Re-anchor (`file:line` Inventory on Merged Base `d19906b09383c69f725c669eca9bbaac8866b546`)

We read `plans/reports/agy-1575-resize-strokes-live.md` (`plans/reports/agy-1575-resize-strokes-live.md:1-147`), merged `origin/jangtrinh/agy-1575-resize-strokes-live` (`d19906b09383c69f725c669eca9bbaac8866b546`, PR `#1588`), and anchored all stroke-cap model, import/export, CPU/GPU stroke geometry, and resize-bake sites:

- **Document Model (`crates/designos-doc`):**
  - `StrokeCap` enum (`None`, `Round`, `Square`, `ArrowLines`, `ArrowEquilateral`, `TriangleFilled`, `DiamondFilled`, `CircleFilled`) and `StrokeCap::is_arrow`: `crates/designos-doc/src/paint.rs:257-274`.
  - `Vertex::cap` (`pub cap: Option<StrokeCap>`): `crates/designos-doc/src/vector.rs:41` (and constructor `Vertex::xy` at `crates/designos-doc/src/vector.rs:51`).
  - `live_stroke_reproduces_bake(stroke: &Stroke, meta: &BTreeMap<String, String>, network: Option<&VectorNetwork>) -> bool`: `crates/designos-doc/src/layout_constraints.rs:190-204`, called by `scale_vector_paths` (`crates/designos-doc/src/layout_constraints.rs:206-222`) during `resize_node_with_constraints` (`crates/designos-doc/src/layout_constraints.rs:86`).
- **`.fig` & REST Import/Export (`crates/designos-fig`):**
  - Kiwi schema `enum StrokeCap`: `crates/designos-fig/src/figma_schema.txt:223-244` (`NodeChange.strokeCap` at `crates/designos-fig/src/figma_schema.txt:1229,1443`).
  - `.fig` import cap filter & metadata carrier: `is_modelled_stroke_cap` at `crates/designos-fig/src/fig.rs:1098-1100`, `extract_fig_metadata` at `crates/designos-fig/src/fig.rs:1132-1143`, and `copy(&mut m, nc, "strokeCap", "strokeCap")` at `crates/designos-fig/src/fig.rs:1575`.
  - `.fig` export cap serialization & metadata carrier: node `strokeCap` at `crates/designos-fig/src/fig_export.rs:935-940`, `figMetadata` carrier at `crates/designos-fig/src/fig_export.rs:1451-1457`, symbol override `strokeCap` at `crates/designos-fig/src/fig_export.rs:1735-1739`, and `encode_vector_network_blob` `styleOverrideTable` `strokeCap` at `crates/designos-fig/src/fig_export.rs:1990-1994`.
  - REST import cap mapping & metadata carrier: `StrokeCapSite` and `unmodelled_stroke_cap` at `crates/designos-fig/src/rest_import.rs:27-68`, `n.stroke.cap` at `crates/designos-fig/src/rest_import.rs:1339-1345`, and override `strokeCap` at `crates/designos-fig/src/rest_import.rs:3019-3025`.
  - REST export cap mapping & metadata carrier: `export_node` `strokeCap` at `crates/designos-fig/src/export.rs:543-552`, `vectorNetwork` fallback at `crates/designos-fig/src/export.rs:686-697`, and `unmodelled_stroke_cap` at `crates/designos-fig/src/export.rs:821-824`.
  - Vector network per-vertex cap conversion: `pub(crate) fn cap_from` at `crates/designos-fig/src/vecnet.rs:14-26`, `pub(crate) fn cap_name` at `crates/designos-fig/src/vecnet.rs:28-39`, `network_from_json` at `crates/designos-fig/src/vecnet.rs:87`, `network_to_json` at `crates/designos-fig/src/vecnet.rs:128-130`, and `apply_style_table` at `crates/designos-fig/src/vecnet.rs:224-226`.
  - `.pen` stroke import/export: `parse_stroke_cap` at `crates/designos-fig/src/pen/paint.rs:520-531` and `export_stroke_value_filtered` at `crates/designos-fig/src/pen/paint.rs:802-812`.
- **CPU, GPU & Export Stroke Rendering (`crates/designos-render`, `crates/designos-gpu`, `crates/designos-export`):**
  - `vectorops.rs`: `sk_stroke_with_join` at `crates/designos-render/src/vectorops.rs:222-251`, `split_cubic_at` / `cubic_eval` / `cubic_arc_len_and_t` / `trim_open_cubics` at `crates/designos-render/src/vectorops.rs:263-373`, `cap_setback` at `crates/designos-render/src/vectorops.rs:375-380`, `push_outward_cap` at `crates/designos-render/src/vectorops.rs:386-403`, `append_cap_outline` at `crates/designos-render/src/vectorops.rs:405-520`, `expand_open_subpath_with_caps` at `crates/designos-render/src/vectorops.rs:522-542`, `expand_subpath` at `crates/designos-render/src/vectorops.rs:544-613`, `circle_polygon` and `cap_polygons` at `crates/designos-render/src/vectorops.rs:645-689`, `vertex_caps` at `crates/designos-render/src/vectorops.rs:694-725`, `open_subpath_caps` and `has_arrow_caps` at `crates/designos-render/src/vectorops.rs:738-753`, `expand_node_subpaths` at `crates/designos-render/src/vectorops.rs:761-800`, and `outline_stroke_along` at `crates/designos-render/src/vectorops.rs:835-911`.
  - `outline.rs`: `node_stroke_style` at `crates/designos-render/src/outline.rs:66-96` and `stroke_plan` at `crates/designos-render/src/outline.rs:283-321`.
  - `lib.rs` (CPU renderer): `Renderer::draw_strokes` at `crates/designos-render/src/lib.rs:845-933`.
  - `geom.rs` (GPU tessellation): `NodeGeom::stroke_mesh` at `crates/designos-gpu/src/geom.rs:266-277` (consumes `designos_render::outline::stroke_plan` directly).
  - `index.rs` (spatial index bounds): `node_margin` at `crates/designos-render/src/index.rs:65-88`.
  - `svg.rs` & `pdf.rs` (vector exports): `has_custom_stroke_caps` / `write_strokes` at `crates/designos-export/src/svg.rs:599-684` and `stroke_cap` / `paint_node_strokes` at `crates/designos-export/src/pdf.rs:1071-1135`.

---

### 1.2 Figma's Cap Set (REA Inspection of `/Applications/Figma.app` + Project Schema)

We inspected `/Applications/Figma.app/Contents/MacOS/Figma` via `rea inspect-macho` (`plans/reports/agy-859-arrowhead-caps-evidence/rea-figma-macho.json:1-22`, evidence ID `ev_0cd5f3f9cd98cd6b00b4abca25c0efd5ef3b4108880aca82eba125aa3cc16c4a`) and disassembled the local Figma Desktop WebAssembly engine (`compiled_wasm.wasm`, 53,282,791 bytes) and workbench bundle (`6413-8e031fdc4fad2bf8.min.js`, 15,095,123 bytes), alongside `crates/designos-fig/src/figma_schema.txt:223-244`.

1. **Full Kiwi Binary `StrokeCap` Enum (`compiled_wasm.wasm` offset `0x303e711..0x303e7ff`, `6413-8e031fdc4fad2bf8.min.js` offset `13490466`, `crates/designos-fig/src/figma_schema.txt:223-244`):**
   - `NONE = 0`, `ROUND = 1`, `SQUARE = 2` (plain stroke caps)
   - `ARROW_LINES = 3`, `ARROW_EQUILATERAL = 4`, `DIAMOND_FILLED = 5`, `TRIANGLE_FILLED = 6`, `CIRCLE_FILLED = 14` (vector and line endpoint marker/arrowhead caps)
   - `HIGHLIGHT = 7`, `WASHI_TAPE_1 = 8`, `WASHI_TAPE_2 = 9`, `WASHI_TAPE_3 = 10`, `WASHI_TAPE_4 = 11`, `WASHI_TAPE_5 = 12`, `WASHI_TAPE_6 = 13` (FigJam scatter/washi-tape brush caps)
   - `ERD_ZERO_OR_ONE = 15`, `ERD_EXACTLY_ONE = 16`, `ERD_ZERO_OR_MORE = 17`, `ERD_ONE_OR_MORE = 18`, `ERD_ONE = 19`, `ERD_MANY = 20` (FigJam entity-relationship diagram connector caps)
2. **Vector & Line `StrokeCap` Set (`6413-8e031fdc4fad2bf8.min.js` offset `4169797` & `14961323`):**
   - REST / Kiwi schema names on `VECTOR` and `LINE` nodes (`eZ.StrokeCap` at `6413-8e031fdc4fad2bf8.min.js:4169797`):
     `["NONE", "ROUND", "SQUARE", "ARROW_LINES", "ARROW_EQUILATERAL", "DIAMOND_FILLED", "TRIANGLE_FILLED", "CIRCLE_FILLED"]`.
   - Plugin API aliases (`6413-8e031fdc4fad2bf8.min.js:14961323`): `"LINE_ARROW"` (for `"ARROW_LINES"`) and `"TRIANGLE_ARROW"` (for `"ARROW_EQUILATERAL"`), alongside `"DIAMOND_FILLED"`, `"CIRCLE_FILLED"`, and `"TRIANGLE_FILLED"`.
   - Therefore, the complete set of Figma vector/line `StrokeCap` variants is:
     - `None` (`"NONE"`)
     - `Round` (`"ROUND"`)
     - `Square` (`"SQUARE"`)
     - `ArrowLines` (`"ARROW_LINES"`, Plugin alias `"LINE_ARROW"`)
     - `ArrowEquilateral` (`"ARROW_EQUILATERAL"`, Plugin alias `"TRIANGLE_ARROW"`)
     - `TriangleFilled` (`"TRIANGLE_FILLED"`)
     - `DiamondFilled` (`"DIAMOND_FILLED"`)
     - `CircleFilled` (`"CIRCLE_FILLED"`)

---

### 1.3 Kit Counts (`Untitled UI Figma – PRO VARIABLES (v8.0)`)

We ran a read-only inventory (`plans/reports/agy-859-arrowhead-caps-evidence/kit-cap-inventory.log:1-82`) on an APFS clone of the Untitled UI kit (`119,748` raw Kiwi `nodeChanges` records, `61,798` blobs; `113` pages and `207,601` scene nodes after component/instance expansion in `designos_fig::import_fig`):

| Cap Type | Raw Kiwi `VECTOR`/`LINE` Node-Level | Raw Kiwi `styleOverrideTable` Entries (Vectors / Total Vertices) | Imported `Document` Vector/Line Node-Level (`import_fig`) | Imported `Document` Per-Vertex (Vectors / Total Vertices) | Three Named Kit Vectors (`source_id`, `NodeId`, `name`, `size`, `w`) |
|---|---:|---:|---:|---:|---|
| `NONE` | `17,593` (implicit default) | `538` (`513` vectors / `180,107` vertices) (`kit-cap-inventory.log:22,26,30`) | `18,922` (`kit-cap-inventory.log:35-36`) | `446` vectors / `180,155` vertices (`kit-cap-inventory.log:37-38`) | `1027:6187` sibling paths, standard icon vectors |
| `ROUND` | `2,067` (`2,065` `VECTOR` + `2` `LINE`) (`kit-cap-inventory.log:8-12,18`) | `2,387` (`1,698` vectors / `18,162` vertices) (`kit-cap-inventory.log:23,27,31`) | `15,700` (`kit-cap-inventory.log:35`) | `6,652` vectors / `55,617` vertices (`kit-cap-inventory.log:37-38`) | `1027:6187` (`"Icon"`, `18x20`, `w=2`), `1027:6315` (`"Icon"`, `20x18`, `w=2`), `1027:6430` (`"Icon"`, `20x20`, `w=2`) (`kit-cap-inventory.log:71-73`) |
| `SQUARE` | `3` (`3` `VECTOR`) (`kit-cap-inventory.log:14,19`) | `0` (`kit-cap-inventory.log:21-24`) | `11` (`kit-cap-inventory.log:35`) | `0` (`kit-cap-inventory.log:37-38`) | `1303:2169` (`"Vector"`, `20.91x24.93`, `w=1`), `1316:1066` (`"Vector"`, `23.00x27.42`, `w=1`), `1334:784` (`"vector"`, `13.44x13.48`, `w=95`) (`kit-cap-inventory.log:80-82`) |
| `ARROW_LINES` | `11` (`11` `VECTOR`) (`kit-cap-inventory.log:12,17`) | `0` (`kit-cap-inventory.log:21-24`) | `732` (`11` source components + `721` expanded instance clones across sizes `80x0`, `48x0`, `40x0`, `32x0`, `24x0`, `20x0`, `18x0`, `16x0`, `12x0`) (`kit-cap-inventory.log:36,39-59`) | `0` (`kit-cap-inventory.log:37-38`) | 1. `1143:83664` (`NodeId(111958)`, `"Line"`, `size=(80, 0)`, `w=1`) (`kit-cap-inventory.log:40,60`)<br>2. `8222:316128` (`NodeId(112139)`, `"Line"`, `size=(23.999998, 0)`, `w=1`) (`kit-cap-inventory.log:41,61`)<br>3. `7749:50847` (`NodeId(113459)`, `"Line"`, `size=(23.999998, 0)`, `w=1`) (`kit-cap-inventory.log:47,62`) |
| `ARROW_EQUILATERAL` | `0` (`kit-cap-inventory.log:16-20`) | `0` (`kit-cap-inventory.log:21-24`) | `0` (`kit-cap-inventory.log:35-36`) | `0` (`kit-cap-inventory.log:37-38`) | *(None in kit)* |
| `TRIANGLE_FILLED` | `0` (`kit-cap-inventory.log:16-20`) | `0` (`kit-cap-inventory.log:21-24`) | `0` (`kit-cap-inventory.log:35-36`) | `0` (`kit-cap-inventory.log:37-38`) | *(None in kit)* |
| `DIAMOND_FILLED` | `0` (`kit-cap-inventory.log:16-20`) | `0` (`kit-cap-inventory.log:21-24`) | `0` (`kit-cap-inventory.log:35-36`) | `0` (`kit-cap-inventory.log:37-38`) | *(None in kit)* |
| `CIRCLE_FILLED` | `0` (`kit-cap-inventory.log:16-20`) | `0` (`kit-cap-inventory.log:21-24`) | `0` (`kit-cap-inventory.log:35-36`) | `0` (`kit-cap-inventory.log:37-38`) | *(None in kit)* |

---

### 1.4 Figma's Head Geometry (WASM Disassembly + Kit `strokeGeometry` Cross-Verification)

We disassembled Figma's stroke-cap endpoint setback function `wasm-function[23138]` (`0x14eca31..0x14ed017`, `plans/reports/agy-859-arrowhead-caps-evidence/rea-wasm-cap-functions.log:105-742`), stroke-cap outline generator `wasm-function[31560]` (`0x1e4f1fb..0x1e52cb5`, `plans/reports/agy-859-arrowhead-caps-evidence/rea-wasm-cap-functions.log:743-6279`), and stroke network orchestrator `wasm-function[9009]` (`0x62c842..0x62d4f5`, `plans/reports/agy-859-arrowhead-caps-evidence/rea-wasm-cap-functions.log:6280-7825`).

Let:
- $w$ be the stroke weight (`strokeWeight`), and $r = \frac{1}{2}w$ (`rea-wasm-cap-functions.log:751-754`).
- $P = (P_x, P_y)$ be the open endpoint vertex position.
- $\hat{f} = (d_x, d_y)$ be the unit outward tangent at $P$ pointing away from the segment interior (`rea-wasm-cap-functions.log:6970-6978`, `756-786`).
- $\hat{n} = (-d_y, d_x)$ be the unit normal ($90^\circ$ CCW from $\hat{f}$, `rea-wasm-cap-functions.log:796-802`).
- $\kappa = 0.552284749831$ be the cubic Bézier quarter-circle handle constant (`rea-wasm-cap-functions.log:987,1550`).

#### Exact Formulas per Cap Type (`wasm-function[23138]` + `wasm-function[31560]`)

1. **`ArrowLines` (`ARROW_LINES`, internal enum `3`, `rea-wasm-cap-functions.log:1514-1881`):**
   - **Shaft setback:** $0$ (`wasm-function[23138]` `br_table` at `rea-wasm-cap-functions.log:166,371-378` leaves setback $0$; the shaft ends with a butt cap at $P \pm r\hat{n}$).
   - **Head centerline & outline:** Two $45^\circ$ backward wings of centerline length $9r = 4.5w$ from $P$ (`f64.const 9` multiplying $\frac{\hat{f} \pm \hat{n}}{\sqrt{2}}r$ at `rea-wasm-cap-functions.log:1602,1614`):
     $$W_1 = P + 4.5w\frac{-\hat{f} + \hat{n}}{\sqrt{2}}, \qquad W_2 = P + 4.5w\frac{-\hat{f} - \hat{n}}{\sqrt{2}}$$
     Along $\hat{f}$, each wing tip center lies at $-4.5w / \sqrt{2} = -3.18198051534w$ behind $P$ and $\pm 3.18198051534w$ perpendicular to $\hat{f}$.
   - **Join and wing caps:** Figma outlines the V-cap as a single closed contour of stroke width $w$ ($r = 0.5w$) with:
     - A $90^\circ$ round apex arc of radius $r$ centered at $P$ from $P + r\frac{\hat{f}+\hat{n}}{\sqrt{2}}$ to $P + r\frac{\hat{f}-\hat{n}}{\sqrt{2}}$ (`rea-wasm-cap-functions.log:1514-1598`, forward overhang $r = 0.5w$ at the apex of the arc).
     - Semicircular round caps of radius $r$ around $W_1$ and $W_2$ (`rea-wasm-cap-functions.log:1627-1730` and `1777-1880`; outer bounding extent $5r / \sqrt{2} = 3.53553390593w$, inner corner $4r / \sqrt{2} = 2.82842712475w$).
     - An inner mitred V-notch at $P - \sqrt{2}r\hat{f} = P - \frac{w}{\sqrt{2}}\hat{f} = P - 0.707106781187w\hat{f}$ (`rea-wasm-cap-functions.log:1731-1754`).

2. **`ArrowEquilateral` (`ARROW_EQUILATERAL`, internal enum `4`, `rea-wasm-cap-functions.log:345-361,435-449,1883-2226`):**
   - **Shaft setback:** The shaft is shortened by $4.5w$ (`local 11 = 5.0 * w - 0.5 * w = 4.5 * w` at `rea-wasm-cap-functions.log:345-355,435-446`), and the cap anchor is shifted back to $P_{\text{base}} = P - 5.0w\hat{f}$ (`rea-wasm-cap-functions.log:702-720`), giving a $0.5w$ overlap of the shaft inside the filled triangle base so no seam is visible.
   - **Head polygon (`strokeJoin != Round`, `rea-wasm-cap-functions.log:2159-2226`):** Forward-pointing equilateral triangle of altitude (length) $L = 10r = 5.0w$ and half-width $H = 5.7735026919r = \frac{5}{\sqrt{3}}w \approx 2.88675134595w$ (full width $\frac{10}{\sqrt{3}}w \approx 5.7735026919w$):
     $$\text{Tip} = P, \qquad B_{\pm} = P - 5.0w\hat{f} \pm \frac{5}{\sqrt{3}}w\hat{n}$$
   - **Rounded corners (`strokeJoin == Round`, `rea-wasm-cap-functions.log:1884-2122`):** The three $60^\circ$ vertices of the equilateral triangle are rounded with corner radius $r_c = \frac{2}{3}r = \frac{1}{3}w$.

3. **`DiamondFilled` (`DIAMOND_FILLED`, internal enum `5`, `rea-wasm-cap-functions.log:2228-2598`):**
   - **Shaft setback:** $0$ (shaft ends at $P$).
   - **Head polygon (`strokeJoin != Round`, `rea-wasm-cap-functions.log:2516-2598`):** Filled $45^\circ$-rotated square centered at $P$ with half-diagonal $D = 5.7735026919r = \frac{5}{\sqrt{3}}w \approx 2.88675134595w$ (total length $\frac{10}{\sqrt{3}}w$, total width $\frac{10}{\sqrt{3}}w$, shaft offset $+D$ forward and $-D$ backward):
     $$V_{\text{front}} = P + \frac{5}{\sqrt{3}}w\hat{f}, \quad V_{\text{right}} = P - \frac{5}{\sqrt{3}}w\hat{n}, \quad V_{\text{back}} = P - \frac{5}{\sqrt{3}}w\hat{f}, \quad V_{\text{left}} = P + \frac{5}{\sqrt{3}}w\hat{n}$$
   - **Rounded corners (`strokeJoin == Round`, `rea-wasm-cap-functions.log:2229-2488`):** The four $90^\circ$ corners of the diamond are rounded with corner radius $r_c = \frac{2}{3}r = \frac{1}{3}w$.

4. **`CircleFilled` (`CIRCLE_FILLED`, Kiwi enum `14`, internal enum `6`, `rea-wasm-cap-functions.log:2600-2630`):**
   - **Shaft setback:** $0$ (shaft ends at $P$).
   - **Head geometry:** Filled circle centered at $P$ with diameter $5.33333333333w = \frac{16}{3}w$ and radius $R = \frac{8}{3}w \approx 2.66666666667w$ (`rea-wasm-cap-functions.log:2605-2612`).

5. **`TriangleFilled` (`TRIANGLE_FILLED`, Kiwi enum `6`, internal enum `7`, UI "Reversed triangle", `rea-wasm-cap-functions.log:166,345-369,2632-2992`):**
   - **Shaft setback:** The shaft is shortened by $4.5w$ (`rea-wasm-cap-functions.log:166,345-369`) and the cap anchor is shifted back to $P_{\text{base}} = P - 5.0w\hat{f}$ (`rea-wasm-cap-functions.log:702-720`).
   - **Head polygon (`strokeJoin != Round`, `rea-wasm-cap-functions.log:2900-2992`):** Reversed triangle whose flat wide base of half-width $H = \frac{5}{\sqrt{3}}w \approx 2.88675134595w$ lies on the endpoint line through $P$ and whose backward-pointing sides meet the shaft edges $\pm 0.5w\hat{n}$ at $P_{\text{base}} = P - 5.0w\hat{f}$:
     $$V_1 = P - 5.0w\hat{f} + 0.5w\hat{n}, \quad V_2 = P + \frac{5}{\sqrt{3}}w\hat{n}, \quad V_3 = P - \frac{5}{\sqrt{3}}w\hat{n}, \quad V_4 = P - 5.0w\hat{f} - 0.5w\hat{n}$$
   - **Rounded corners (`strokeJoin == Round`, `rea-wasm-cap-functions.log:2633-2831`):** The two outer corners at $P \pm \frac{5}{\sqrt{3}}w\hat{n}$ are rounded with corner radius $r_c = \frac{1}{3}w$.

#### Cross-Verification & Residual Against Kit `strokeGeometry` (`kit-cap-inventory.log:60-64`)

Comparing the formula above against Figma's baked `strokeGeometry` in the Untitled UI kit on `1143:83664` (`NodeId(111958)`, `size=(80, 0)`, `w=1`), `8222:316128` (`NodeId(112139)`, `size=(23.999998, 0)`, `w=1`), `7749:50847` (`NodeId(113459)`, `size=(23.999998, 0)`, `w=1`), and resized instance clones `NodeId(152760)` (`size=(40, 0)`, `w=1`) and `NodeId(152828)` (`size=(48, 0)`, `w=1`):
- Formula start-cap points for $w = 1$, $P = (0, 0)$, $\hat{f} = (-1, 0)$, $\hat{n} = (0, -1)$:
  - Apex arc endpoints: $(-0.35355338, \mp 0.35355338)$ vs kit `(-0.35355338, -0.35355338)` and `(-0.35355338, 0.35355338)` (`kit-cap-inventory.log:60`) -> residual $0.0\text{ px}$ ($0\text{ ULP}$).
  - Apex arc cubic handles: $(-0.54881555, \mp 0.15829125)$ vs kit `(-0.54881555, -0.15829125)` and `(-0.54881555, 0.15829125)` (`kit-cap-inventory.log:60`) -> residual $0.0\text{ px}$ ($0\text{ ULP}$).
  - Wing outer/tip/inner points: $(2.8284271, \pm 3.5355339)$, $(3.5355339, \pm 3.5355339)$, $(3.5355339, \pm 2.8284271)$ vs kit `(2.828427, ±3.535534)`, `(3.535534, ±3.535534)`, `(3.535534, ±2.828427)` (`kit-cap-inventory.log:60`) -> residual $\le 1.2 \times 10^{-7}\text{ px}$ ($1\text{ f32 ULP}$).
  - Inner V-notch: $(0.70710677, 0.0)$ vs kit `(0.70710677, 0.0)` (`kit-cap-inventory.log:60`) -> residual $0.0\text{ px}$ ($0\text{ ULP}$).
- Across all 28 path commands of `1143:83664`, `8222:316128`, and `7749:50847`, the maximum coordinate residual between our closed-form WASM formula and Figma's baked `strokeGeometry` is $1.19 \times 10^{-7}\text{ px}$ ($1\text{ f32 ULP}$).

---

## Step 2 — Red Tests Before Production Edits

Before applying our production code changes on merged base `d19906b09383c69f725c669eca9bbaac8866b546` (with only minimal `todo!()` stubs on the new `StrokeCap` enum arms so the test binaries compile), we wrote and ran the Red tests across `designos-fig`, `designos-render`, and `designos-gpu`:

1. **`crates/designos-fig/tests/resized_vector_stroke_model.rs`** (`plans/reports/agy-859-arrowhead-caps-evidence/red-fig.log:1-145`):
   - Command: `cargo test -p designos-fig --test resized_vector_stroke_model --locked`
   - Result: `FAILED. 11 passed; 10 failed` (`red-fig.log:143`):
     - `all_five_arrow_caps_round_trip_through_rest_and_fig_on_node_and_vertex_and_plain_json_stays_byte_identical` (`red-fig.log:41`): panicked at `resized_vector_stroke_model.rs:703` (`assertion left == right failed` on `node.stroke.cap` for `ARROW_LINES`: `None` vs `ArrowLines`).
     - `an_arrow_cap_on_the_node_keeps_the_scaled_bake` (`red-fig.log:77`): panicked at `resized_vector_stroke_model.rs:142` (`an arrow cap is drawn live so its baked stroke geometry is dropped on resize`).
     - `an_arrow_cap_on_a_vertex_keeps_the_scaled_bake` (`red-fig.log:69`): panicked at `resized_vector_stroke_model.rs:159` (`a vertex arrow cap is drawn live so its baked stroke geometry is dropped on resize`).
     - `an_arrow_cap_survives_a_rest_round_trip_and_keeps_its_bake_on_a_resize` (`red-fig.log:84`): panicked at `resized_vector_stroke_model.rs:219`.
     - `an_arrow_cap_survives_a_saved_fig_file_and_the_reopened_vector_keeps_its_bake_on_a_resize` (`red-fig.log:91`): panicked at `resized_vector_stroke_model.rs:646`.
     - `an_arrow_vector_in_an_instance_keeps_its_bake_when_the_instance_is_resized` (`red-fig.log:99`): panicked at `resized_vector_stroke_model.rs:614`.
     - `an_instance_clone_of_an_arrow_icon_writes_the_arrow_back_onto_its_vertex_at_a_new_size` (`red-fig.log:107`): panicked at `resized_vector_stroke_model.rs:384`.
     - `fig_arrow_caps_on_the_node_and_on_a_vertex_keep_the_scaled_bake` (`red-fig.log:115`): panicked at `resized_vector_stroke_model.rs:514`.
     - `a_rest_export_keeps_a_vertex_arrow_off_a_plain_node` (`red-fig.log:55`) and `a_rest_export_writes_a_vertex_arrow_back_onto_its_vertex_when_the_node_has_a_plain_cap` (`red-fig.log:62`).

2. **`crates/designos-render/tests/resized_vector_stroke_weight.rs`** (`plans/reports/agy-859-arrowhead-caps-evidence/red-render.log:1-85`):
   - Command: `cargo test -p designos-render --test resized_vector_stroke_weight --locked`
   - Result: `FAILED. 16 passed; 4 failed` (`red-render.log:83`):
     - `arrow_lines_cap_matches_baked_stroke_geometry_and_keeps_constant_size_after_resize` (`red-render.log:44`): panicked at `resized_vector_stroke_weight.rs:882` (`ARROW_LINES must draw wider than the shaft: h=2.0 vs 2.0`).
     - `all_five_arrow_caps_render_expected_head_geometry_and_setback` (`red-render.log:37`): panicked at `resized_vector_stroke_weight.rs:948` (`ArrowLines head height 10.0 vs expected ~70.71`).
     - `per_vertex_mixed_arrow_caps_and_curved_endpoint_tangent_orient_heads_along_tangent` (`red-render.log:51`): panicked at `resized_vector_stroke_weight.rs:1032` (`CircleFilled start cap on open line must render wider than shaft: 8.0`).
     - `closed_subpath_ignores_arrow_caps_and_open_subpath_gets_them` (`red-render.log:58`): panicked at `resized_vector_stroke_weight.rs:1126` (`open subpath with ArrowEquilateral must render arrowhead: 6.0`).

3. **`crates/designos-gpu/tests/resized_shapes.rs`** (`plans/reports/agy-859-arrowhead-caps-evidence/red-gpu.log:1-82`):
   - Command: `cargo test -p designos-gpu --test resized_shapes --locked`
   - Result: `FAILED. 12 passed; 4 failed` (`red-gpu.log:80`):
     - `per_vertex_round_and_square_caps_match_cpu_and_extend_past_butt_cap_on_gpu` (`red-gpu.log:61`, `#1590`): panicked at `resized_shapes.rs:789` (`GPU must render per-vertex Round cap extension at x=17 across y=26..34`).
     - `arrow_lines_on_node_and_after_resize_matches_cpu_on_gpu` (`red-gpu.log:45`): panicked at `resized_shapes.rs:845` (`ArrowLines must paint wing pixels above and below the shaft on GPU`).
     - `all_five_arrow_caps_and_curved_endpoint_match_cpu_on_gpu` (`red-gpu.log:37`): panicked at `resized_shapes.rs:914` (`GPU ArrowEquilateral must paint wide arrowhead base`).
     - `per_vertex_arrow_caps_match_cpu_on_gpu` (`red-gpu.log:53`): panicked at `resized_shapes.rs:956` (`GPU must paint start CircleFilled and end ArrowEquilateral per-vertex caps`).

---

## Step 3 — Production Implementation & Green Tests

### 3.1 Production Changes (`file:line` Summary)

1. **`StrokeCap` Model & Resize Bake Rule (`crates/designos-doc`):**
   - `crates/designos-doc/src/paint.rs:257-274`: Added `ArrowLines`, `ArrowEquilateral`, `TriangleFilled`, `DiamondFilled`, and `CircleFilled` to `StrokeCap`, plus `pub fn is_arrow(self) -> bool`.
   - `crates/designos-doc/src/layout_constraints.rs:190-204`: Updated `live_stroke_reproduces_bake` so that modelled arrowhead caps (`stroke.cap.is_arrow()` or `v.cap.is_some_and(StrokeCap::is_arrow)`) on centered, non-dashed strokes return `true` (dropping the stale baked `stroke_geometry` on resize so the live renderer draws the arrowhead at the unstretched `stroke.weight` at the new endpoints), while preserving `d19906b0`'s rule that dashed strokes with custom per-vertex joins keep their scaled bake and unmodelled caps (`meta.contains_key("strokeCap")`) keep their scaled bake.

2. **`.fig`, REST, and `.pen` Import/Export (`crates/designos-fig`):**
   - `crates/designos-fig/src/vecnet.rs:14-39`: Made `cap_from` and `cap_name` `pub(crate)` and mapped `"ARROW_LINES"` | `"LINE_ARROW"` <-> `StrokeCap::ArrowLines`, `"ARROW_EQUILATERAL"` | `"TRIANGLE_ARROW"` <-> `StrokeCap::ArrowEquilateral`, `"TRIANGLE_FILLED"` <-> `StrokeCap::TriangleFilled`, `"DIAMOND_FILLED"` <-> `StrokeCap::DiamondFilled`, and `"CIRCLE_FILLED"` <-> `StrokeCap::CircleFilled`.
   - `crates/designos-fig/src/fig.rs:1098-1100,1575`: Updated `is_modelled_stroke_cap` to `c.is_empty() || crate::vecnet::cap_from(c).is_some()` and changed `enum_map(&mut m, nc, "strokeCap", ...)` to `copy(&mut m, nc, "strokeCap", "strokeCap")` so `.fig` node-level and vertex-level (`styleOverrideTable` -> `vecnet::apply_style_table` at `vecnet.rs:224-226`) arrow caps populate `node.stroke.cap` and `Vertex.cap` directly without polluting `fig_metadata`, while still-unmodelled caps (`"HIGHLIGHT"`, `"WASHI_TAPE_*"`) remain preserved in `fig_metadata`.
   - `crates/designos-fig/src/rest_import.rs:56-62,1340,3020`: Updated `unmodelled_stroke_cap` to `let unmodelled = |c: &&str| !c.is_empty() && crate::vecnet::cap_from(c).is_none();` and mapped node and instance-override `strokeCap` via `crate::vecnet::cap_from`, preserving `d19906b0`'s `StrokeCapSite` / `strokeCapSource` metadata carrier for still-unmodelled caps.
   - `crates/designos-fig/src/export.rs:543-552,686-697,821-824` & `crates/designos-fig/src/fig_export.rs:935-940,1737,1991-1993`: Exported all 8 modelled `StrokeCap` variants via `crate::vecnet::cap_name` on nodes, symbol overrides, and `styleOverrideTable` vertex entries while falling back to `unmodelled_stroke_cap` when an unmodelled cap was preserved in metadata.
   - `crates/designos-fig/src/pen/paint.rs:520-531,802-812`: Mapped the 5 arrowhead caps in `.pen` stroke import/export.

3. **Live CPU & GPU Stroke Rendering + SVG/PDF Outline Export (`crates/designos-render`, `crates/designos-gpu`, `crates/designos-export`):**
   - `crates/designos-render/src/vectorops.rs:263-542,645-911`:
     - Implemented `split_cubic_at`, `cubic_eval`, `cubic_arc_len_and_t`, and `trim_open_cubics` (`vectorops.rs:263-373`) to shorten open subpaths at their start/end by `cap_setback` ($4.5w$ for `ArrowEquilateral` and `TriangleFilled`, $0$ for all others) along the exact curve arc length and evaluate the outward tangent at the endpoint before trimming.
     - Implemented `append_cap_outline` (`vectorops.rs:405-520`) for all 5 arrowhead caps (`ArrowLines`, `ArrowEquilateral`, `TriangleFilled`, `DiamondFilled`, `CircleFilled`) using the exact WASM formulas from Step 1.4, including `StrokeJoin::Round` corner rounding ($r_c = \frac{1}{3}w$) on `ArrowEquilateral`, `DiamondFilled`, and `TriangleFilled`.
     - Implemented `expand_open_subpath_with_caps` (`vectorops.rs:522-542`), `open_subpath_caps` (`vectorops.rs:738-746`), and `has_arrow_caps` (`vectorops.rs:748-753`).
     - Preserved `d19906b0`'s `circle_polygon` and `cap_polygons` (`vectorops.rs:645-689`) for `Round` and `Square` caps in `outline_stroke_along` while delegating arrow caps to `append_cap_outline` and trimming open subpaths via `trim_open_cubics` (`vectorops.rs:881-903`).
     - Kept `has_per_vertex_joins` (`vectorops.rs:729-736`) with its exact name and signature (`pub fn has_per_vertex_joins(n: &Node) -> bool`).
   - `crates/designos-render/src/outline.rs:71-80,314-317` & `crates/designos-render/src/lib.rs:894-898`:
     - Routed `has_per_vertex_joins(n) || has_arrow_caps(n) || vertex_caps(n).is_some()` through `vectorops::expand_node_subpaths(n, sh, width)` in `outline::stroke_plan` (`outline.rs:314`) and `Renderer::draw_strokes` (`lib.rs:894-898`). Because `designos-gpu` (`crates/designos-gpu/src/geom.rs:266-277`) tessellates `outline::stroke_plan` directly, both CPU and GPU now draw all 5 arrowhead caps and per-vertex `Round`/`Square` caps (`Closes #1590`) from the same geometry plan.
   - `crates/designos-render/src/index.rs:75-76`: Extended `node_margin` stroke multiplier from `2.0` to `6.0` when `has_arrow_caps(n)` so spatial culling tiles include arrowhead overhangs up to $3.54w$.
   - `crates/designos-export/src/svg.rs:599-617` & `crates/designos-export/src/pdf.rs:1075-1079,1115-1124`: Routed strokes with `has_arrow_caps(n) || vertex_caps(n).is_some()` through `vectorops::outline_stroke` so SVG and PDF exports include arrowhead and per-vertex cap outlines.

### 3.2 Green Test Logs

- `plans/reports/agy-859-arrowhead-caps-evidence/green-fig.log:1-29`: `cargo test -p designos-fig --test resized_vector_stroke_model --locked` -> `test result: ok. 21 passed; 0 failed` (`rc=0`).
- `plans/reports/agy-859-arrowhead-caps-evidence/green-render.log:1-28`: `cargo test -p designos-render --test resized_vector_stroke_weight --locked` -> `test result: ok. 20 passed; 0 failed` (`rc=0`).
- `plans/reports/agy-859-arrowhead-caps-evidence/green-gpu.log:1-24`: `cargo test -p designos-gpu --test resized_shapes --locked` -> `test result: ok. 16 passed; 0 failed` (`rc=0`).

---

## Step 4 — Kit Non-Regression Proof (`318` Kit Vectors, `0` Diffs)

We ran the kit vector pixel-hash verification test on an APFS clone of the Untitled UI kit both on merged base `d19906b09383c69f725c669eca9bbaac8866b546` (`plans/reports/agy-859-arrowhead-caps-evidence/kit-hashes-base.log:1-318`) and on `HEAD` with our full production changes (`plans/reports/agy-859-arrowhead-caps-evidence/kit-hashes-head.log:1-318`), covering `318` real kit vectors across `4` groups:
1. `ARROW_LINES` vectors (`100` vectors, including `1143:83664` (`NodeId(111958)`), `8222:316128` (`NodeId(112139)`), `7749:50847` (`NodeId(113459)`), and resized instance clones `NodeId(152760)` and `NodeId(152828)`):
   - Rendered both with their original `.fig` baked `stroke_geometry` and live after dropping `stroke_geometry`: `0` pixel diffs out of `860..2,220` pixels (`0.00%` diff), and after resizing `1143:83664` from `80` to `40`, `stroke_geometry` is dropped (`live_Drop_bake=true`) and its live render matches the kit's own `40px` instance (`NodeId(152760)`) with `0` pixel diffs (`kit-hashes-head.log:1-100`).
2. Node-level `ROUND` vectors (`100` vectors, including `1027:6187`, `1027:6315`, `1027:6430`): identical FNV-1a pixel hashes before and after (`kit-hashes-head.log:101-200`).
3. Node-level `SQUARE` vectors (`11` vectors, all `11` in the kit including `1303:2169`, `1316:1066`, `1334:784`): identical FNV-1a pixel hashes before and after (`kit-hashes-head.log:201-211`).
4. Mixed per-vertex cap vectors (`107` vectors with `v.cap.is_some()`): identical FNV-1a pixel hashes before and after (`kit-hashes-head.log:212-318`).

Diff between `kit-hashes-base.log` and `kit-hashes-head.log` (`plans/reports/agy-859-arrowhead-caps-evidence/kit-hashes-diff.log`): **`0` lines (`0` bytes)** across all `318` vectors.

### 4.2 Headless CPU & GPU Side-by-Side Kit Renders (1x Bake vs Live, and Resized 2x Base vs Head)

For the three named kit `ARROW_LINES` vectors (`1143:83664`, `8222:316128`, `7749:50847`), headless CPU and GPU renders at `1x` (Figma baked `strokeGeometry` vs live `StrokeCap::ArrowLines` after dropping `stroke_geometry`: `0` pixel diffs) and after a `2x` horizontal resize (base scaled bake with stretched 2x arrowheads vs head live render with unstretched $w=1$ arrowheads on CPU and GPU):

- **`1143:83664` (`NodeId(111958)`, `"Line"`, `80x0`, `w=1`):**
  - 1x (Bake vs Live CPU/GPU): `plans/reports/agy-859-arrowhead-caps-evidence/kit-1143-83664-1x-bake-vs-live.png`
  - 2x Resize (Base Stretched Bake vs Head Live CPU/GPU): `plans/reports/agy-859-arrowhead-caps-evidence/kit-1143-83664-2x-base-vs-head.png`
- **`8222:316128` (`NodeId(112139)`, `"Line"`, `24x0`, `w=1`):**
  - 1x (Bake vs Live CPU/GPU): `plans/reports/agy-859-arrowhead-caps-evidence/kit-8222-316128-1x-bake-vs-live.png`
  - 2x Resize (Base Stretched Bake vs Head Live CPU/GPU): `plans/reports/agy-859-arrowhead-caps-evidence/kit-8222-316128-2x-base-vs-head.png`
- **`7749:50847` (`NodeId(113459)`, `"Line"`, `24x0`, `w=1`):**
  - 1x (Bake vs Live CPU/GPU): `plans/reports/agy-859-arrowhead-caps-evidence/kit-7749-50847-1x-bake-vs-live.png`
  - 2x Resize (Base Stretched Bake vs Head Live CPU/GPU): `plans/reports/agy-859-arrowhead-caps-evidence/kit-7749-50847-2x-base-vs-head.png`

---

## Step 5 — Mutation Falsification (3 Mutants Killed & Reverted)

We applied 3 independent single-line mutations one at a time, ran the corresponding test binary to verify the tests fail with `rc=101`, and reverted each mutation immediately:

1. **Mutant 1 (`crates/designos-fig/src/vecnet.rs:20`):** Mapped `"ARROW_EQUILATERAL"` -> `StrokeCap::None` in `cap_from`.
   - Command: `cargo test -p designos-fig --test resized_vector_stroke_model --locked`
   - Log: `plans/reports/agy-859-arrowhead-caps-evidence/mutant-1.log:1-101` (`rc=101`, `13 passed; 8 failed`, killed by `all_five_arrow_caps_round_trip_through_rest_and_fig_on_node_and_vertex_and_plain_json_stays_byte_identical`, `an_arrow_cap_on_the_node_keeps_the_scaled_bake`, `an_arrow_cap_on_a_vertex_keeps_the_scaled_bake`, `fig_arrow_caps_on_the_node_and_on_a_vertex_keep_the_scaled_bake`, etc.).
2. **Mutant 2 (`crates/designos-render/src/vectorops.rs:377`):** Returned `0.0` setback for `StrokeCap::ArrowEquilateral` in `cap_setback` (so the shaft pokes out to the tip instead of stopping $4.5w$ back).
   - Command: `cargo test -p designos-render --test resized_vector_stroke_weight --locked`
   - Log: `plans/reports/agy-859-arrowhead-caps-evidence/mutant-2.log:1-56` (`rc=101`, `18 passed; 2 failed`, killed by `all_five_arrow_caps_render_expected_head_geometry_and_setback` at `resized_vector_stroke_weight.rs:956` and `per_vertex_mixed_arrow_caps_and_curved_endpoint_tangent_orient_heads_along_tangent` at `resized_vector_stroke_weight.rs:1049`).
3. **Mutant 3 (`crates/designos-render/src/outline.rs:314`):** Removed `has_arrow_caps(n) || vertex_caps(n).is_some()` from `outline::stroke_plan` so GPU falls back to plain `node_stroke_style`.
   - Command: `cargo test -p designos-gpu --test resized_shapes --locked`
   - Log: `plans/reports/agy-859-arrowhead-caps-evidence/mutant-3.log:1-74` (`rc=101`, `12 passed; 4 failed`, killed by `per_vertex_round_and_square_caps_match_cpu_and_extend_past_butt_cap_on_gpu`, `arrow_lines_on_node_and_after_resize_matches_cpu_on_gpu`, `all_five_arrow_caps_and_curved_endpoint_match_cpu_on_gpu`, and `per_vertex_arrow_caps_match_cpu_on_gpu`).

---

## Step 6 — Verification Gates (All 11 Required Commands GREEN)

All 11 required gate commands were executed after the last code edit and logged under `plans/reports/agy-859-arrowhead-caps-evidence/`:

| # | Command | Log File | Exit Code & Summary |
|---|---|---|---|
| 1 | `cargo fmt --all -- --check` | `plans/reports/agy-859-arrowhead-caps-evidence/gate-01-fmt.log` | `fmt_rc=0` |
| 2 | `cargo clippy --workspace --all-targets -- -D warnings` | `plans/reports/agy-859-arrowhead-caps-evidence/gate-02-clippy.log` | `clippy_rc=0` (`0` warnings) |
| 3 | `cargo test -p designos-doc --locked` | `plans/reports/agy-859-arrowhead-caps-evidence/gate-03-doc.log` | `rc=0` (`747 + 3 + 5 + 2 passed; 0 failed`) |
| 4 | `cargo test -p designos-fig --locked` | `plans/reports/agy-859-arrowhead-caps-evidence/gate-04-fig.log` | `rc=0` (`0 failed` across all 45 test binaries) |
| 5 | `cargo test -p designos-render --locked` | `plans/reports/agy-859-arrowhead-caps-evidence/gate-05-render.log` | `rc=0` (`0 failed` across all 30 test binaries) |
| 6 | `cargo test -p designos-gpu --locked` | `plans/reports/agy-859-arrowhead-caps-evidence/gate-06-gpu.log` | `rc=0` (`0 failed` across all 16 test binaries) |
| 7 | `cargo test -p designos-mcp --locked` | `plans/reports/agy-859-arrowhead-caps-evidence/gate-07-mcp.log` | `rc=0` (`0 failed` across all 31 test binaries) |
| 8 | `cargo test -p designos-export --locked` | `plans/reports/agy-859-arrowhead-caps-evidence/gate-08-export.log` | `rc=0` (`0 failed` across all 10 test binaries) |
| 9 | `cargo test -p designos-app --bin designos --locked` | `plans/reports/agy-859-arrowhead-caps-evidence/gate-09-app.log` | `4,251 passed; 2 failed; 13 ignored` under 24-thread load; solo rerun of both known-flaky timing tests: `2 passed; 0 failed` (`rerun_rc=0`) |
| 10 | `cargo test --workspace --lib --locked` | `plans/reports/agy-859-arrowhead-caps-evidence/gate-10-workspace-lib.log` | `rc=0` (`0 failed` across all 12 workspace lib suites) |
| 11 | `python3 scripts/zero_fake_ui_gate.py` | `plans/reports/agy-859-arrowhead-caps-evidence/gate-11-zero-fake-ui.log` | `rc=0` (`17/17 PASS`) |

### Gate 9 (`designos-app`) Solo Rerun of Under-Load Timing Failures (`gate-09-app.log:4296-4313`)

Under full-suite parallel load (`4,266` tests), the two 800ms close-ack timeout tests in `shell::tests::close_human_dirty` timed out:
- `shell::tests::close_human_dirty::library_file_close_save_writes_the_edit_and_closes_not_path_required`
- `shell::tests::close_human_dirty::library_file_read_only_close_save_is_invalid_arguments_not_path_required`

Rerunning both tests alone (`gate-09-app.log:4296-4313`):
```text
running 2 tests
test shell::tests::close_human_dirty::library_file_read_only_close_save_is_invalid_arguments_not_path_required ... ok
test shell::tests::close_human_dirty::library_file_close_save_writes_the_edit_and_closes_not_path_required ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 4264 filtered out; finished in 1.70s

rerun_rc=0
```

### Draft PR, Commit Log (`0c5a1d27..HEAD`), and Scratch Cleanup

- **Draft PR URL:** https://github.com/jangtrinh/design-os-studio/pull/1591
- **`git log --oneline 0c5a1d27..HEAD`:**
```text
4e1cc7dd7 feat(strokes): arrowhead caps on lines and vectors, drawn live and kept through .fig and REST
d19906b09 docs(report): add red, green, mutant and gate logs for the Miter join and per-vertex end cap fixes
2145b877c fix(vector): keep a Miter join sharp in a piecewise stroke and draw the end cap of per-vertex caps outward
858d6fe82 docs(report): add red, green, mutant and gate logs for the end cap, dashed join and arrow cap fixes
50d555e15 fix(fig): write an arrow cap back to the node or vertex it was read from in REST export
f6e4d72f1 fix(vector): keep the baked outline of a dashed stroke whose vertex join differs from the node's
3fa59fe7d fix(vector): draw the end cap of an open stroke with per-vertex joins pointing outward
```
- **`ls out/cargo` (after deleting `out/cargo` and `out/kit-clone.fig`):**
```text
ls: out/cargo: No such file or directory
```
