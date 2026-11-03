# Report — `#1478` Dash cap for dashed strokes

## Step 1 — Reverse Engineering & Re-anchor (Before Code Edits)

### 1.1 Reverse Engineering Figma (`rea` + Figma `v43` Bundle & `.fig` Schema Inspection)

1. **REA Evidence Bundles**:
   - Native binary inspection on `/Applications/Figma.app/Contents/MacOS/Figma`: `ev_0cd5f3f9cd98cd6b00b4abca25c0efd5ef3b4108880aca82eba125aa3cc16c4a` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:2`).
   - Extracted Figma `v43` JavaScript editor bundle inspection: `ev_063289d197f28b1354a60309c44d21a8b1f8e71775da9fbb91ab929b9755322e` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:3`).

2. **How Figma Stores Dash Cap in the `.fig` Kiwi Schema and REST API**:
   - **Figma `v43` Editor Kiwi Schema** (`~/Library/Application Support/Figma/DesktopProfile/v43/Cache/Cache_Data/3905b617efbb4f81_0` -> `27756-1dc8c27de1ebafa4.min.js.br`, decompressed `15063487` bytes, `sha256=2403ce35291fbd828f026f5670edf0284b5f9c5bec06b1eb2f9f8a21f4736980`, `plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:5-6`):
     - `dashCap` and `terminalCap` appear `0` times in the Kiwi schema bundle (`dashCap_count=0`, `terminalCap_count=0`, `plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:7`).
     - `enum StrokeCap { NONE=0; ROUND=1; SQUARE=2; ARROW_LINES=3; ... }` is defined at byte offset `13457308` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:8`).
     - `enum DashMode { CLIP=0; STRETCH=1; }` is defined at byte offset `13458035` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:9`).
     - `message Stroke` defines `StrokeCap strokeCap=7; Number strokeCapSize=8; StrokeJoin strokeJoin=9; float miterLimit=10; float[] dashPattern=11;` at byte offset `13479835` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:10`).
     - `message NodeChange` defines `float[] dashPattern=13; ... StrokeCap strokeCap=30; uint strokeCapTag=82[deprecated]; Number strokeCapSize=497; StrokeJoin strokeJoin=31;` at byte offset `13489978` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:11`).
   - **Untitled UI Kit `.fig` Embedded Kiwi Schema** (`untitled_schema.bin`, `75243` decompressed bytes, `sha256=f361538d53170dafb2e5ad8f6cd722e5cbca5697e1aa7c490caa098b27242643`, `plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:13-14`):
     - `dashCap` and `terminalCap` appear `0` times (`untitled_schema_dashCap_count=0`, `terminalCap_count=0`, `plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:15`).
     - `StrokeCap` is at byte offset `3047` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:16`), `DashMode` at `3783` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:17`), `dashPattern` at `19838` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:18`), `strokeCap` at `19779` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:19`), and `strokeCapSize` at `19792` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:20`).
     - Repository schema snapshots confirm the same layout: `crates/designos-fig/tests/data/figma-real-schema.txt:404-426` (`enum StrokeCap`), `crates/designos-fig/tests/data/figma-real-schema.txt:502-505` (`enum DashMode`), `crates/designos-fig/tests/data/figma-real-schema.txt:1986-2009` (`NodeChange.dashPattern = 13`, `NodeChange.strokeCap = 30`), and `crates/designos-fig/src/figma_schema.txt:257-266`, `crates/designos-fig/src/figma_schema.txt:1229-1233`, `crates/designos-fig/src/figma_schema.txt:1436-1443`.
   - **Figma Fullscreen Wasm JS Bindings** (`~/Library/Application Support/Figma/DesktopProfile/v43/Cache/Cache_Data/7d063ae82a1f1003_0` -> `compiled_wasm.js.br`, `2377189` decompressed bytes, `plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:22-23`):
     - `dashCap` appears `0` times (`dashCap_count=0`, `plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:23`).
     - Node C++ API exports `_NodeTsApi_getStrokeCap` at byte offset `1007067` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:24`), `_NodeTsApi_getStrokeCapOrMixed` at `1007186` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:25`), `_NodeTsApi_setStrokeCap` at `1007321` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:26`), `_NodeTsApi_getDashPattern` at `1010980` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:27`), `_NodeTsApi_setDashPattern` at `1011112` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:28`), and `_SceneGraphUiHelpers_updateSelectionProperties` at `1229331` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:29`).
   - **Figma Editor Stroke Settings UI & Derived Selection Property Bridge**:
     - In `Cache_Data/09f33c869f30e8a7_0` (`84430-eea6d6965fe6eeb8.min.js.br`, `357181` decompressed bytes, `plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:32`):
       - The cap segmented control defines `value: "NONE"` (`dash_cap_none.tooltip`), `value: "SQUARE"` (`dash_cap_square.tooltip`), and `value: "ROUND"` (`dash_cap_round.tooltip`) at byte offset `124471` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:33`).
       - `StrokeLineStyleSettings` reads `let M = (0, et.Hd)(e.dashCap)` at byte offset `140758` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:34`), constructs the `"dashCap"` input with `onChange: t => { e.onChange({dashCap: t}) }` and `label: (0, y.t)("fullscreen.properties_panel.stroke_settings.dash_cap")` at byte offset `144279` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:35`), and renders it only when `("SIMPLE_DASH" === ec || "CUSTOM_DASH" === ec) && void 0 !== M` at byte offset `145276` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:36`).
     - In `Cache_Data/0ffb76f013a6c928_0` (`57075-82fe18db0c55edd9.min.js.br`, `23831` decompressed bytes, `plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:37`):
       - Validator `P(e)` (`Hd`) accepts `"NONE" | "ROUND" | "SQUARE"` (or the mixed sentinel `a.oV`) at byte offset `18973` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:38`).
       - Dash style classifier `k(e)` returns `"LINE"` when `dashPattern` is empty or `[0, 0]`, `"SIMPLE_DASH"` when `e.length === 2`, and `"CUSTOM_DASH"` otherwise at byte offset `19034` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:39`).
     - In `Cache_Data/613e12ea20f89370_0` (`figma_app-7d9f1f40fd698362.min.js.br`, `8369519` decompressed bytes, `plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:41`):
       - `"terminalCap"` and `"dashCap"` are listed in the non-Kiwi derived selection property list `Q` at byte offset `2273536` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:42`) and routed to `SceneGraphUiHelpers.updateSelectionProperties(c, n, s, a === b.zk.YES, e)` at byte offset `2277874` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:44`), which updates `NodeChange.strokeCap` (`strokeCap = 30 : StrokeCap`) on the selected layer(s) (while open vector endpoint caps live in `vectorNetwork` vertex style overrides `strokeCap`).
       - In `Cache_Data/193b5f09212e6390_0` (`74515-63b499fb5ad21b5d.min.js.br`, `226628` decompressed bytes, `plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:45`), the `"dashCap"` recording key directly writes `{strokeCap: e}` at byte offset `79313` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:46`).

---

### 1.2 Re-anchor & Ruling Selection

1. **Document Model (`crates/designos-doc/src/paint.rs`)**:
   - `StrokeCap` (`crates/designos-doc/src/paint.rs:257-263`) defines `None`, `Round`, `Square`.
   - `Stroke` (`crates/designos-doc/src/paint.rs:268-283`) stores `pub cap: StrokeCap` (`crates/designos-doc/src/paint.rs:276`) and `pub dashes: Vec<f32>` (`crates/designos-doc/src/paint.rs:282`), with `Stroke::default()` (`crates/designos-doc/src/paint.rs:285-297`) setting `cap: StrokeCap::None` (`crates/designos-doc/src/paint.rs:291`) and `dashes: Vec::new()` (`crates/designos-doc/src/paint.rs:295`).
   - Open vector per-vertex endpoint caps are stored separately on `VectorVertex::cap: Option<StrokeCap>` (`crates/designos-doc/src/vector.rs:20-26`).

2. **CPU and GPU Renderers (`crates/designos-render` and `crates/designos-gpu`)**:
   - **CPU/GPU shared stroke setup**:
     - `has_dashes` (`crates/designos-render/src/outline.rs:61-63`) checks `!s.dashes.is_empty() && s.dashes.iter().all(|d| *d >= 0.0) && s.dashes.iter().sum::<f32>() > 0.0`.
     - `node_stroke_style` (`crates/designos-render/src/outline.rs:66-93`) builds a `tiny_skia::Stroke` from `n.stroke`, setting `line_cap` at `crates/designos-render/src/outline.rs:73-77` and `dash = sk::StrokeDash::new(v, 0.0)` at `crates/designos-render/src/outline.rs:85-91`.
     - `make_stroke` (`crates/designos-render/src/lib.rs:832-834`) delegates to `outline::node_stroke_style(n, width)`.
     - **Bug 1 Found (Open Vector with `VectorNetwork` + Dashes)**: In `vectorops::vertex_caps(n)` (`crates/designos-render/src/vectorops.rs:330-357`) and `vectorops::outline_stroke` (`crates/designos-render/src/vectorops.rs:367-382`), whenever a `NodeKind::Vector` had a `VectorNetwork` where any vertex had `v.cap.is_some()` (populated by `.fig` and REST imports from `styleOverrideTable` at `crates/designos-fig/src/fig.rs:2045-2049` and `crates/designos-fig/src/rest_import.rs:1951-1953`), `vertex_caps(n)` returned `Some(out)` even when the stroke was dashed (`has_dashes(&n.stroke)`), forcing `StrokeCap::None` (`LineCap::Butt`) on the base stroke and leaving every internal dash butt-capped instead of capping every dash with `Stroke::cap`.
     - **Bug 2 Found (`Inside` / `Outside` Aligned Dashed Rectangles)**: For `StrokeAlign::Inside` and `StrokeAlign::Outside`, stroking the outer boundary at `2.0 * w` and clipping to the shape interior/exterior produced half-clipped `2.0 * w`-diameter semicircles on the boundary (`radius = w` centered on the outer edge, creating a flat shelf along the outer edge) instead of a true `w`-diameter (`radius = w / 2`) round/square dash cap centered in the `w`-wide stroke band. Added `dashed_rect_center_path` (`crates/designos-render/src/outline.rs:223-260`), wired into `stroke_plan` (`crates/designos-render/src/outline.rs:280-286`) for GPU and `draw_strokes` (`crates/designos-render/src/lib.rs:911-920`) for CPU.

3. **Stroke Panel (`crates/designos-app/src/panels/stroke.rs`)**:
   - `popover_ui` (`crates/designos-app/src/panels/stroke.rs:135-213`) filters selection to stroked layers via `stroked_ids` (`crates/designos-app/src/panels/stroke.rs:119-126`), computes `common_cap = common(app, &ids, |n| n.stroke.cap)` (`crates/designos-app/src/panels/stroke.rs:143`) and `cap_label(common_cap)` (`crates/designos-app/src/panels/stroke.rs:128-133`, returning `"Mixed"` when selected stroked layers disagree on `Stroke::cap`), and renders row `5` (`"Dash cap"`, dropdown `"strokep-dash-cap"` with options `["None", "Round", "Square"]`) only when `show_dash_cap` (`crates/designos-app/src/panels/stroke.rs:144,199-208`) is true (`has_dashes(&n.stroke)`, `crates/designos-app/src/panels/stroke.rs:115-117`). Selecting a dash cap calls `app.edit_nodes(&ids, ...)` once (`crates/designos-app/src/panels/stroke.rs:202-205`), setting `n.stroke.cap = CAPS[i].1` and clearing `n.stroke_geometry = None` on all selected stroked layers in a single undo step.

4. **`.fig` and REST Import/Export (`crates/designos-fig`)**:
   - Extended `OverrideField::Stroke` serialization and import for component instances across `.fig` and REST:
     - `collect_instance_overrides` (`crates/designos-fig/src/fig_export.rs:1683-1715`) writes `strokeAlign`, `strokeJoin`, `strokeCap`, `miterLimit`, and `dashPattern`.
     - `convert_override` (`crates/designos-fig/src/fig.rs:2194-2205`) maps `strokeAlign`, `strokeCap`, `strokeJoin`, `strokeMiterAngle`, `miterLimit` -> `strokeMiterLimit`, and `dashPattern` -> `strokeDashes`.
     - `apply_override` (`crates/designos-fig/src/rest_import.rs:2922-2961`) and `override_field` (`crates/designos-fig/src/rest_import.rs:3619-3626`) apply and classify `strokeAlign`, `strokeCap`, `strokeJoin`, `strokeMiterLimit`, `strokeMiterAngle`, and `strokeDashes` as `OverrideField::Stroke` (clearing stale `stroke_geometry` when not overridden).
     - `instance_overrides` (`crates/designos-fig/src/export.rs:285-324,644-649`) emits REST `"overrides"` entries including `"strokeCap"` and `"strokeDashes"` for `OverrideField::Stroke` (`crates/designos-fig/src/export.rs:302`), while skipping `OverrideField::Layout` when `layout_cleared` is set on an `Instance` (`crates/designos-fig/src/export.rs:307`).

5. **Ruling Choice — Branch B (`Ruling 2` & `Ruling 3`)**:
   - Because Figma's `.fig` Kiwi schema (`NodeChange.strokeCap = 30 : StrokeCap`, `Stroke.strokeCap = 7 : StrokeCap`), C++ Wasm node API (`_NodeTsApi_getStrokeCap` / `_NodeTsApi_setStrokeCap`), and REST API (`strokeCap`) store a single node-level `strokeCap` field for the stroke cap while open vector endpoint caps live per-vertex in `VectorVertex::cap` (`vectorNetwork`), **Branch B** applies:
     - Do not add a duplicate field to `designos_doc::Stroke`.
     - The stroke panel's `"Dash cap"` control (`None`, `Round`, `Square`) is shown only while the stroke has a dash pattern (`crates/designos-app/src/panels/stroke.rs:144,199-208`), displays `"Mixed"` when selected stroked layers disagree on `Stroke::cap` (`crates/designos-app/src/panels/stroke.rs:128-133,143`), and writes `Stroke::cap` (clearing stale `stroke_geometry`) on every selected layer that has a stroke in one undo step (`crates/designos-app/src/panels/stroke.rs:119-126,202-205`).
     - Both CPU (`designos-render`) and GPU (`designos-gpu`) cap every dash with `Stroke::cap` (`crates/designos-render/src/outline.rs:66-93,223-286`, `crates/designos-render/src/lib.rs:832-834,911-920`, `crates/designos-render/src/vectorops.rs:330-382`).

---

## Acceptance 1 — Five Unit Tests (RED on Base `a98ec2b1` -> GREEN on Final Branch)

### 1.1 RED Runs on Unmodified Base Code (`a98ec2b1bcd6668092cbb4e97d8c82f305be8d54`)

Full log saved at `plans/reports/agy-1478-dash-cap-evidence/red-tests.log:1-140`:

1. **`dashed_rect_round_dash_cap_renders_round_dash_ends`** (`crates/designos-render/tests/render.rs:680-708`) and **`dashed_rect_round_dash_cap_renders_round_dash_ends_gpu`** (`crates/designos-gpu/tests/fidelity.rs:527-554`):
   - CPU failure (`plans/reports/agy-1478-dash-cap-evidence/red-tests.log:10,22-28`):
     ```text
     thread 'dashed_rect_round_dash_cap_renders_round_dash_ends' (73946583) panicked at crates/designos-render/tests/render.rs:701:5:
     assertion `left == right` failed: round dash cap extends +2px along midline y=24
       left: [255, 143, 143, 255]
      right: [255, 0, 0, 255]
     ```
   - GPU failure (`plans/reports/agy-1478-dash-cap-evidence/red-tests.log:46,59-68`):
     ```text
     thread 'dashed_rect_round_dash_cap_renders_round_dash_ends_gpu' (73946620) panicked at crates/designos-gpu/tests/fidelity.rs:545:5:
     assertion `left == right` failed: GPU round dash cap painted on midline (46, 24)
       left: [255, 128, 128, 255]
      right: [255, 0, 0, 255]
     ```
2. **`dashed_open_line_uses_stroke_cap_on_every_dash`** (`crates/designos-render/tests/render.rs:711-753`) and **`dashed_open_line_uses_stroke_cap_on_every_dash_gpu`** (`crates/designos-gpu/tests/fidelity.rs:557-597`):
   - CPU failure (`plans/reports/agy-1478-dash-cap-evidence/red-tests.log:9,14-20`):
     ```text
     thread 'dashed_open_line_uses_stroke_cap_on_every_dash' (73946582) panicked at crates/designos-render/tests/render.rs:751:5:
     assertion `left == right` failed: first dash start cap is round at midline (18, 20)
       left: [255, 255, 255, 255]
      right: [255, 0, 0, 255]
     ```
   - GPU failure (`plans/reports/agy-1478-dash-cap-evidence/red-tests.log:45,50-57`):
     ```text
     thread 'dashed_open_line_uses_stroke_cap_on_every_dash_gpu' (73946619) panicked at crates/designos-gpu/tests/fidelity.rs:596:5:
     CPU/GPU mismatch 2.333%
     ```
3. **`panels::stroke::tests::stroke_panel_dash_cap_visibility_mixed_and_multi_selection_single_undo`** (`crates/designos-app/src/panels/stroke.rs:240-339`):
   - Failure (`plans/reports/agy-1478-dash-cap-evidence/red-tests.log:87,91-95`):
     ```text
     thread 'panels::stroke::tests::stroke_panel_dash_cap_visibility_mixed_and_multi_selection_single_undo' (73947666) panicked at crates/designos-app/src/panels/stroke.rs:255:32:
     expected 'Dash cap' row when dashes are set, got [("Stroke settings", ...), ("Cap", ...), ("None", ...), ("Join", ...), ("Miter", ...), ("Miter angle", ...), ("Dash", ...), ("Gap", ...)]
     ```
4. **`fig_export_import_preserves_dash_cap`** (`crates/designos-fig/tests/fig_roundtrip.rs:4276-4352`):
   - Failure (`plans/reports/agy-1478-dash-cap-evidence/red-tests.log:114,126-132`):
     ```text
     thread 'fig_export_import_preserves_dash_cap' (73947884) panicked at crates/designos-fig/tests/fig_roundtrip.rs:4349:5:
     assertion `left == right` failed: instance child preserves overridden dash pattern across .fig
       left: [6.0, 4.0]
      right: [12.0, 6.0]
     ```
5. **`rest_export_import_preserves_dash_cap`** (`crates/designos-fig/tests/fig_roundtrip.rs:4355-4409`):
   - Failure (`plans/reports/agy-1478-dash-cap-evidence/red-tests.log:113,118-124`):
     ```text
     thread 'rest_export_import_preserves_dash_cap' (73947885) panicked at crates/designos-fig/tests/fig_roundtrip.rs:4416:5:
     assertion `left == right` failed: instance child preserves overridden Round dash cap across REST export + import + sync_instances
       left: None
      right: Round
     ```

### 1.2 GREEN Runs on Final Implementation

Full log saved at `plans/reports/agy-1478-dash-cap-evidence/green-tests.log:1-74`:

- **CPU (`designos-render`)** (`plans/reports/agy-1478-dash-cap-evidence/green-tests.log:10-17`):
  ```text
  Running tests/render.rs (out/cargo/debug/deps/render-5ddf387f7aa22575)
  test dashed_line_and_caps ... ok
  test dashed_open_line_uses_stroke_cap_on_every_dash ... ok
  test dashed_rect_round_dash_cap_renders_round_dash_ends ... ok
  test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 28 filtered out; finished in 0.00s
  ```
- **GPU (`designos-gpu`)** (`plans/reports/agy-1478-dash-cap-evidence/green-tests.log:30-36`):
  ```text
  Running tests/fidelity.rs (out/cargo/debug/deps/fidelity-e9d67385509b7a81)
  test dashed_open_line_uses_stroke_cap_on_every_dash_gpu ... ok
  test dashed_rect_round_dash_cap_renders_round_dash_ends_gpu ... ok
  test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.17s
  ```
- **Stroke Panel (`designos-app`)** (`plans/reports/agy-1478-dash-cap-evidence/green-tests.log:53-58`):
  ```text
  Running unittests src/main.rs (out/cargo/debug/deps/designos-22365f4a34dba6ca)
  test panels::stroke::tests::stroke_panel_dash_cap_visibility_mixed_and_multi_selection_single_undo ... ok
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4195 filtered out; finished in 0.02s
  ```
- **`.fig` & REST Round-Trip (`designos-fig`)** (`plans/reports/agy-1478-dash-cap-evidence/green-tests.log:67-73`):
  ```text
  Running tests/fig_roundtrip.rs (out/cargo/debug/deps/fig_roundtrip-59464b98b5f1161b)
  test rest_export_import_preserves_dash_cap ... ok
  test fig_export_import_preserves_dash_cap ... ok
  test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 40 filtered out; finished in 0.03s
  ```

---

## Acceptance 2 — Mutation Testing (Null Control + 2 Mutants Killed & Reverted)

Full log saved at `plans/reports/agy-1478-dash-cap-evidence/mutants.log:1-181`:

1. **Green Null Control (No Mutants)** (`plans/reports/agy-1478-dash-cap-evidence/mutants.log:1-39`):
   - `designos-render`: `3 passed; 0 failed` (`plans/reports/agy-1478-dash-cap-evidence/mutants.log:13`).
   - `designos-gpu`: `2 passed; 0 failed` (`plans/reports/agy-1478-dash-cap-evidence/mutants.log:25`).
   - `designos-app`: `1 passed; 0 failed` (`plans/reports/agy-1478-dash-cap-evidence/mutants.log:38`).

2. **Mutant 1 — Renderer ignores the dash cap (`crates/designos-render/src/outline.rs:91`)** (`plans/reports/agy-1478-dash-cap-evidence/mutants.log:41-136`):
   - Diff (`plans/reports/agy-1478-dash-cap-evidence/mutants.log:42-48`):
     ```diff
              v.extend_from_within(..);
          }
          st.dash = sk::StrokeDash::new(v, 0.0);
     +    st.line_cap = sk::LineCap::Butt; // MUTANT 1: renderer ignores dash cap
      }
      st
     ```
   - Killed by `designos-render` (`dashed_open_line_uses_stroke_cap_on_every_dash` and `dashed_rect_round_dash_cap_renders_round_dash_ends`, `2 failed`, `plans/reports/agy-1478-dash-cap-evidence/mutants.log:61-89`):
     ```text
     thread 'dashed_open_line_uses_stroke_cap_on_every_dash' (74020250) panicked at crates/designos-render/tests/render.rs:751:5:
     assertion `left == right` failed: first dash start cap is round at midline (18, 20)
       left: [255, 255, 255, 255]
      right: [255, 0, 0, 255]
     thread 'dashed_rect_round_dash_cap_renders_round_dash_ends' (74020251) panicked at crates/designos-render/tests/render.rs:701:5:
     assertion `left == right` failed: round dash cap extends +2px along midline y=24
       left: [255, 255, 255, 255]
      right: [255, 0, 0, 255]
     test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 28 filtered out; finished in 0.00s
     MUTANT 1 KILLED BY designos-render
     ```
   - Killed by `designos-gpu` (`dashed_open_line_uses_stroke_cap_on_every_dash_gpu` and `dashed_rect_round_dash_cap_renders_round_dash_ends_gpu`, `2 failed`, `plans/reports/agy-1478-dash-cap-evidence/mutants.log:102-136`):
     ```text
     thread 'dashed_open_line_uses_stroke_cap_on_every_dash_gpu' (74021540) panicked at crates/designos-gpu/tests/fidelity.rs:599:5:
     assertion `left == right` failed
       left: [255, 255, 255, 255]
      right: [255, 0, 0, 255]
     thread 'dashed_rect_round_dash_cap_renders_round_dash_ends_gpu' (74021541) panicked at crates/designos-gpu/tests/fidelity.rs:545:5:
     assertion `left == right` failed: GPU round dash cap painted on midline (46, 24)
       left: [255, 255, 255, 255]
      right: [255, 0, 0, 255]
     test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.18s
     MUTANT 1 KILLED BY designos-gpu
     ```
   - Mutant 1 reverted immediately after test run.

3. **Mutant 2 — Panel writes only the first selected layer (`crates/designos-app/src/panels/stroke.rs:202`)** (`plans/reports/agy-1478-dash-cap-evidence/mutants.log:138-181`):
   - Diff (`plans/reports/agy-1478-dash-cap-evidence/mutants.log:139-145`):
     ```diff
          if show_dash_cap {
              popover::row(ui, body, 5, "Dash cap", |ui, r| {
                  if let Some(i) = dropdown(ui, r, "strokep-dash-cap", cap_lbl, &caps) {
     -                app.edit_nodes(&ids, |n| {
     +                app.edit_nodes(&ids[..1], |n| { // MUTANT 2: panel writes only first selected layer
                          n.stroke.cap = CAPS[i].1;
                          n.stroke_geometry = None;
                      });
     ```
   - Killed by `designos-app` (`panels::stroke::tests::stroke_panel_dash_cap_visibility_mixed_and_multi_selection_single_undo`, `1 failed`, `plans/reports/agy-1478-dash-cap-evidence/mutants.log:162-181`):
     ```text
     thread 'panels::stroke::tests::stroke_panel_dash_cap_visibility_mixed_and_multi_selection_single_undo' (74024226) panicked at crates/designos-app/src/panels/stroke.rs:339:9:
     assertion `left == right` failed: r2 updated to Round
       left: Square
      right: Round
     test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 4195 filtered out; finished in 0.02s
     MUTANT 2 KILLED BY designos-app
     ```
   - Mutant 2 reverted immediately after test run.

---

## Acceptance 3 — GUI Proof (`HIDIdleTime >= 300`, Full & Cropped Captures)

Full metadata logged in `plans/reports/agy-1478-dash-cap-evidence/gui-capture.log:1-13`:

1. **Idle Guard & Kit Frame**:
   - Opened a `.designos` copy (`/tmp/untitled-1478-gui.designos`) of `/Users/jangtrinh/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig` on frame `Press logo` (`source_id=1345:407`) with dashed stroke on `Card` (`source_id=I1345:415;1345:61`, `dashes=[16.0, 12.0]`, `weight=6.0`, `plans/reports/agy-1478-dash-cap-evidence/gui-capture.log:12-13`).
   - Verified `ioreg -c IOHIDSystem` idle time before each capture:
     - Before `out/evidence/1478-dash-cap-none.png`: `IDLE_SECONDS=4610` (`>= 300`, `plans/reports/agy-1478-dash-cap-evidence/gui-capture.log:10`).
     - Before `out/evidence/1478-dash-cap-round.png`: `IDLE_SECONDS=4852` (`>= 300`, `plans/reports/agy-1478-dash-cap-evidence/gui-capture.log:11`).

2. **Full Window Screenshots (`out/evidence/`, local-only, not committed)**:
   - `out/evidence/1478-dash-cap-none.png`: `1600x1088` RGBA (`sha256=b3693b5247bc76612cb8b3f6dd96ec1f12d662711062426744dbe58207e561c8`, `plans/reports/agy-1478-dash-cap-evidence/gui-capture.log:1,5`), showing `"Stroke settings"` popover open with `"Dash"` `16`, `"Gap"` `12`, `"Dash cap"` `"None"`, and butt-ended dashes on the canvas.
   - `out/evidence/1478-dash-cap-round.png`: `1600x1088` RGBA (`sha256=4912e87f8ca786f4560e3e826df83323b7e08da359f3424a499e98dc5a662e77`, `plans/reports/agy-1478-dash-cap-evidence/gui-capture.log:2,6`), showing `"Stroke settings"` popover open with `"Dash"` `16`, `"Gap"` `12`, `"Dash cap"` `"Round"`, and rounded dash ends on the canvas.

3. **Cropped Stroke Panel Captures (Stroke Panel Only)**:
   - `plans/reports/agy-1478-dash-cap-evidence/stroke-panel-dash-cap-none.png`: `265x270` RGBA (`sha256=78c347259627de742c11d216ab8c8611b1c2386276991305ddacaacbb0e0f32a`, `plans/reports/agy-1478-dash-cap-evidence/gui-capture.log:3,7`).
   - `plans/reports/agy-1478-dash-cap-evidence/stroke-panel-dash-cap-round.png`: `265x270` RGBA (`sha256=8d7aad591985d21b74b964e09898735b0a05c47f420f10cec48b7beeeb6add87`, `plans/reports/agy-1478-dash-cap-evidence/gui-capture.log:4,8`).

---

## Acceptance 4 — All Gates Green & Cleanup Verification

Full gate outputs logged at `plans/reports/agy-1478-dash-cap-evidence/gates.log:1-8195`:

1. **`cargo fmt --all -- --check`** (`plans/reports/agy-1478-dash-cap-evidence/gates.log:1-4`):
   - Result: `=== EXIT: 0 ===` (`0` formatting diffs).

2. **`cargo clippy --workspace --all-targets --locked -- -D warnings`** (`plans/reports/agy-1478-dash-cap-evidence/gates.log:5-123`):
   - Workspace clippy (`plans/reports/agy-1478-dash-cap-evidence/gates.log:5-56`) reports only `main`'s 3 pre-existing errors outside our scope (`crates/designos-app/src/agent_permissions.rs:1176:9` and `1194:9` `clippy::needless_return` from `f680b8e9`, and `crates/designos-app/src/shell/tests/inspector_var_binds.rs:1181:21` `clippy::type_complexity` from `e4efa94e`, not modified per Wave 1 rules), and `0` warnings/errors in `crates/designos-app/src/panels/stroke.rs`.
   - `cargo clippy -p designos-render -p designos-gpu -p designos-fig --all-targets --locked -- -D warnings` (`plans/reports/agy-1478-dash-cap-evidence/gates.log:57-123`): `=== EXIT: 0 ===` (`0` warnings/errors across all touched library/test targets).

3. **`cargo test -p designos-render --locked`** (`plans/reports/agy-1478-dash-cap-evidence/gates.log:124-754`):
   - Result: `=== EXIT: 0 ===` (`233` passed across `26` test binaries, `0` failed; `tests/render.rs`: `31 passed; 0 failed` at `plans/reports/agy-1478-dash-cap-evidence/gates.log:607`).

4. **`cargo test -p designos-gpu --locked -- --skip large_file_compile_benchmarks`** (`plans/reports/agy-1478-dash-cap-evidence/gates.log:755-1036`):
   - Result: `=== EXIT: 0 ===` (`102` passed across `16` test binaries, `0` failed; `tests/fidelity.rs`: `20 passed; 0 failed` at `plans/reports/agy-1478-dash-cap-evidence/gates.log:890`).

5. **`cargo test -p designos-fig --locked`** (`plans/reports/agy-1478-dash-cap-evidence/gates.log:1037-1845`):
   - Result: `=== EXIT: 0 ===` (`684` passed across `35` test binaries, `0` failed; `tests/fig_roundtrip.rs`: `42 passed; 0 failed` at `plans/reports/agy-1478-dash-cap-evidence/gates.log:1502`, `tests/fig_upgrade.rs`: `10 passed; 0 failed` at `plans/reports/agy-1478-dash-cap-evidence/gates.log:1740`, `tests/fig_import.rs`: `65 passed; 0 failed` at `plans/reports/agy-1478-dash-cap-evidence/gates.log:1367`).

6. **`cargo test -p designos-app --bin designos --locked`** (`plans/reports/agy-1478-dash-cap-evidence/gates.log:1846-6334,8011-8195`):
   - Stroke panel suite (`cargo test -p designos-app --bin designos --locked panels::stroke`, `plans/reports/agy-1478-dash-cap-evidence/gates.log:8011-8028`): `test result: ok. 2 passed; 0 failed` (`=== EXIT: 0 ===`).
   - Full binary suite (`plans/reports/agy-1478-dash-cap-evidence/gates.log:1846-6334`) passed `4159` tests on the initial `/tmp/h-1478` run; the `24` tests that rejected `HOME` inside `/tmp` (an agent-writable temp root) or timed out under full-suite CPU load were re-run with non-`/tmp` short `HOME="/Users/jangtrinh/orca/workspaces/.h"` (`plans/reports/agy-1478-dash-cap-evidence/gates.log:8029-8195`): `47 passed` (`agent_permissions::tests::`, `gates.log:8090`), `28 passed` (`agent_turn_ledger::tests::`, `gates.log:8130`), `25 passed` (`terminal::ask_mode_tests::`, `gates.log:8167`), `1 passed` (`the_production_root_sits_outside_every_agent_writable_root`, `gates.log:8180`), and `1 passed` (`two_deferred_text_commands_both_convert_in_order`, `gates.log:8193`), `0 failed`, `=== EXIT: 0 ===` (`gates.log:8195`).

7. **`cargo test --workspace --locked --lib`** (`plans/reports/agy-1478-dash-cap-evidence/gates.log:6335-7981`):
   - Result: `=== EXIT: 0 ===` (`1498` passed across all workspace library crates: `13 + 720 + 1 + 158 + 143 + 30 + 5 + 37 + 326 + 56 + 1 + 8`, `0` failed).

8. **`python3 scripts/zero_fake_ui_gate.py`** (`plans/reports/agy-1478-dash-cap-evidence/gates.log:7982-8010`):
   - Result: `Gate Verdict: PASS (17/17 invariants satisfied)` (`=== EXIT: 0 ===`).

9. **Disk Cleanup Verification (`rm -rf out/cargo "$H" "$T"`, `ls out/cargo` failing)** (`plans/reports/agy-1478-dash-cap-evidence/gui-capture.log:9`):
   ```text
   $ rm -rf out/cargo "$H" "$T"
   $ ls out/cargo
   ls: out/cargo: No such file or directory
   ```

---

## Round 2

### What Could Not Be Verified & Pre-Existing / Environmental Exceptions

1. **Workspace-wide `cargo clippy --workspace --all-targets --locked -- -D warnings`** (`plans/reports/agy-1478-dash-cap-evidence/r2-gates.log:3-57`):
   - Exited with `clippy_ws_rc=101` solely due to 3 pre-existing errors on `main` in untouched files of `designos-app`: `crates/designos-app/src/agent_permissions.rs:1176` (`clippy::needless_return`), `crates/designos-app/src/agent_permissions.rs:1194` (`clippy::needless_return`), and `crates/designos-app/src/shell/tests/inspector_var_binds.rs:1181` (`clippy::type_complexity`).
   - Scoped clippy on all crates modified in Round 2 (`cargo clippy -p designos-render -p designos-gpu -p designos-fig --all-targets --locked -- -D warnings`, `plans/reports/agy-1478-dash-cap-evidence/r2-gates.log:58-66`) passed with `clippy_touched_rc=0`.
2. **Full-suite `cargo test -p designos-app --bin designos --locked` under 77-byte scratch `HOME`** (`plans/reports/agy-1478-dash-cap-evidence/r2-gates.log:1566-5998`):
   - Result: `4205 passed; 2 failed; 13 ignored` (`test_app_bin_rc=101`).
   - Named failure 1: `vecops::tests::two_deferred_text_commands_both_convert_in_order` (`plans/reports/agy-1478-dash-cap-evidence/r2-gates.log:5965,5985-5989`) timed out waiting for background font loading under 4220-test parallel CPU load; rerun alone (`plans/reports/agy-1478-dash-cap-evidence/r2-gates.log:7687-7703`), it passed (`1 passed; 0 failed`, `rerun_vecops_rc=0`).
   - Named failure 2: `agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root` (`plans/reports/agy-1478-dash-cap-evidence/r2-gates.log:5979-5984`) panicked at `crates/designos-app/src/agent_approval_channel.rs:286` because the 77-byte scratch `HOME` (`/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1478-dash-cap-scratch/home`) made the Unix socket path `148` bytes (`> 103`-byte macOS `SUN_LEN` limit). Rerun alone with that 77-byte `HOME` reproduces the 148-byte socket failure (`rerun_ap_long_home_rc=101`, `plans/reports/agy-1478-dash-cap-evidence/r2-gates.log:7704-7729`), and rerun alone with normal `HOME="/Users/jangtrinh"` (while keeping `TMPDIR` in the scratch directory) passes (`1 passed; 0 failed`, `rerun_ap_short_home_rc=0`, `plans/reports/agy-1478-dash-cap-evidence/r2-gates.log:7730-7744`).

---

### Ruling 6 — Round 1 Report Corrections (Verified Against Actual Files & Line Numbers)

1. **Correction 1 — Figma Desktop bundle offset `79313` (`endCap` vs `dashCap`)**:
   - In `Cache_Data/193b5f09212e6390_0` (`74515-63b499fb5ad21b5d.min.js.br`), offset `79313` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:46`) is:
     `label:(0,c.t)("fullscreen.properties_panel.stroke_settings.end_points"),hideNone:n,kind:"endCap"`
     which is the **`endCap` ("end_points")** control (it reuses `recordingKey:v.Pt(e,"dashCap")` for telemetry/recording, not for writing `dashCap`).
   - The actual **Dash cap** control is in `84430-eea6d6965fe6eeb8.min.js.br` at offset `144279` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:35`), which passes `value:e.dashCap,onChange:t=>{e.onChange({dashCap:t})}` and `kind:"dashCap"` under `fullscreen.properties_panel.stroke_settings.dash_cap`.
2. **Correction 2 — `NodeChange` byte offset `13489978` quote**:
   - The Kiwi schema string extracted from `Figma.app` at byte offset `13489978` (`plans/reports/agy-1478-dash-cap-evidence/step1-figma-re.log:11`) is:
     `StrokeCap strokeCap=30;uint strokeCapTag=82[deprecated];Number strokeCapSize=497;StrokeJoin strokeJoin=31;...`
   - That slice does **not** contain `dashPattern=13`; `dashPattern` is defined earlier in `NodeChange`.
3. **Correction 3 — `crates/designos-fig/tests/data/figma-real-schema.txt` line numbers**:
   - Round 1 cited shifted line ranges (`404-426`, `502-505`, `1986-2009`).
   - The actual line numbers in `crates/designos-fig/tests/data/figma-real-schema.txt` (verified in `plans/reports/agy-1478-dash-cap-evidence/r2-citation-grep.log`) are:
     - `crates/designos-fig/tests/data/figma-real-schema.txt:359-381`: `enum StrokeCap` (`NONE = 0`, `ROUND = 1`, `SQUARE = 2`, ...).
     - `crates/designos-fig/tests/data/figma-real-schema.txt:457-460`: `enum DashMode` (`CLIP = 0`, `STRETCH = 1`).
     - `crates/designos-fig/tests/data/figma-real-schema.txt:1503`: `Float[] dashPattern = 11;` inside `struct Stroke`.
     - `crates/designos-fig/tests/data/figma-real-schema.txt:1890`: `Float[] dashPattern = 13;` inside `message NodeChange`.
     - `crates/designos-fig/tests/data/figma-real-schema.txt:1906`: `StrokeCap strokeCap = 30;` inside `message NodeChange`.
4. **Correction 4 — Where per-vertex `v.cap` (`Vertex::cap`) is populated during `.fig` and REST import**:
   - Per-vertex `v.cap` is **not** populated at `crates/designos-fig/src/rest_import.rs:1951-1953` (where `mark_custom_instance_slots` resides at `crates/designos-fig/src/rest_import.rs:1952-1965`).
   - Instead, per-vertex `v.cap` is populated in `crates/designos-fig/src/vecnet.rs`:
     - For REST JSON / intermediate JSON vertices: `network_from_json` at `crates/designos-fig/src/vecnet.rs:77` (`vx.cap = x.get("strokeCap").and_then(Value::as_str).and_then(cap_from);`).
     - For `.fig` `styleOverrideTable` vertex styles: `apply_style_table` at `crates/designos-fig/src/vecnet.rs:207-228` (specifically `crates/designos-fig/src/vecnet.rs:214-216`: `if let Some(c) = entry.get("strokeCap").and_then(Value::as_str).and_then(cap_from) { v.cap = Some(c); }`), called from `convert_vector_network` in `crates/designos-fig/src/fig.rs:2083-2142`.

---

### Rulings 1–5 — Code Changes & Root-Cause Resolution

1. **Ruling 1 — Merge `origin/main` (`56869cad`)**:
   - Merged `origin/main` into `jangtrinh/agy-1478-dash-cap` (`56869cad`), keeping both `main`'s `proportionsConstrained` -> `preserveRatio` -> `OverrideField::LockAspect` arms and this branch's stroke override arms in:
     - `crates/designos-fig/src/fig.rs:2183-2211` (`convert_override`: lines `2194-2205` for stroke fields/`dashPattern` and lines `2206-2210` for `proportionsConstrained`).
     - `crates/designos-fig/src/rest_import.rs:2853-2965` (`apply_override`) and `crates/designos-fig/src/rest_import.rs:3609-3632` (`override_field`: line `3614` for `LockAspect` and lines `3622-3630` for `Stroke`).
     - `crates/designos-fig/src/export.rs:285-315` (`instance_overrides`: line `297` for `LockAspect` and line `303` for `Stroke`).
   - Verified `cargo test -p designos-fig --test fig_roundtrip -- fig_instance_lock_aspect_override_roundtrip` passes (`2 passed; 0 failed`, `lock_aspect_rc=0`, `plans/reports/agy-1478-dash-cap-evidence/r2-lock-aspect-roundtrip.log:1-19`).

2. **Ruling 2 (Major 1) — Solid open vectors render as on `main`**:
   - Removed the `vertex_caps` check from `node_stroke_style` (`crates/designos-render/src/outline.rs:66-94`), which is shared by both CPU and GPU (`stroke_plan`).
   - Applied the `st.line_cap = sk::LineCap::Butt` override for per-vertex caps exclusively on the CPU draw path in `Renderer::make_stroke` (`crates/designos-render/src/lib.rs:832-837`), where `draw_strokes` draws the polygon caps returned by `vertex_caps(n)`.
   - In `vectorops::vertex_caps` (`crates/designos-render/src/vectorops.rs:330-357`), kept the `crate::outline::has_dashes(&n.stroke)` early return (`crates/designos-render/src/vectorops.rs:332`) and returned `Some(out)` at `crates/designos-render/src/vectorops.rs:356` even when `out` is empty (when every open endpoint's effective cap is `StrokeCap::None`), so `make_stroke` still overrides `n.stroke.cap` to `Butt` on CPU when vertices explicitly set `cap = Some(StrokeCap::None)`.
   - Added CPU test `solid_open_line_all_butt_vertex_caps_stay_butt` (`crates/designos-render/tests/render.rs:766-799`) and GPU test `solid_open_line_round_vertex_cap_renders_round_gpu` (`crates/designos-gpu/tests/fidelity.rs:609-650`).

3. **Ruling 3 (Major 2) — Thin inside dashed rectangles keep their stroke**:
   - When `outline::dashed_rect_center_path(n, align)` returns `None` (because `w >= n.size.x` or `w >= n.size.y` on `StrokeAlign::Inside`), both `Renderer::draw_strokes` (`crates/designos-render/src/lib.rs:915-928`) and `outline::stroke_plan` (`crates/designos-render/src/outline.rs:281-296`) fall through to the `2 * w` clipped stroke path instead of returning early with no stroke.
   - In `outline::node_stroke_style` (`crates/designos-render/src/outline.rs:83-86`), when `has_dashes(s)` and `width > s.weight` (the `2 * w` fallback), `st.line_cap` is clamped to `sk::LineCap::Butt` so `tiny-skia` does not expand round/square caps with radius `w` (`= 2 * (w / 2)`) across `4px` gaps on thin inside-aligned dashed rectangles.
   - Added CPU test `thin_inside_dashed_rect_keeps_stroke` (`crates/designos-render/tests/render.rs:801-818`), GPU test `thin_inside_dashed_rect_keeps_stroke_gpu` (`crates/designos-gpu/tests/fidelity.rs:652-678`), and `StrokeAlign::Outside` probes in `dashed_rect_round_dash_cap_renders_round_dash_ends` (`crates/designos-render/tests/render.rs:709-719`) and `dashed_rect_round_dash_cap_renders_round_dash_ends_gpu` (`crates/designos-gpu/tests/fidelity.rs:555-564`).

4. **Ruling 4 — Instance stroke overrides (`.fig` and REST)**:
   - In `collect_instance_overrides` (`crates/designos-fig/src/fig_export.rs:1715`), `OverrideField::Stroke` now unconditionally emits `("dashPattern", V::Array(n.stroke.dashes.iter().map(|&d| fl(d)).collect()))`, so an instance that clears its component's dashed stroke to solid (`dashes = []`) writes `"dashPattern": []` into `symbolOverrides` instead of omitting `dashPattern` and inheriting the component's dashes on re-import.
   - Added 3 tests in `crates/designos-fig/tests/fig_roundtrip.rs`:
     - `fig_instance_stroke_override_preserves_align_join_and_miter` (`crates/designos-fig/tests/fig_roundtrip.rs:4475-4526`)
     - `fig_instance_stroke_override_clears_dashes` (`crates/designos-fig/tests/fig_roundtrip.rs:4528-4587`)
     - `rest_instance_overrides_skips_layout_cleared` (`crates/designos-fig/tests/fig_roundtrip.rs:4589-4641`)

5. **Ruling 5 (LOW) — Removed review/ruling wording from comments**:
   - Removed `"Under Branch B"` from the comment in `dashed_open_line_uses_stroke_cap_on_every_dash` (`crates/designos-render/tests/render.rs:724-725`).

---

### Round 2 Acceptance Table

| Check | Command | Exit Code | Result Summary & Log Citation |
|---|---|---|---|
| **A1 (RED on `9dba68d2` — CPU)** | `cargo test -p designos-render --test render -- solid_open_line_all_butt_vertex_caps_stay_butt thin_inside_dashed_rect_keeps_stroke` | `101` | `1 passed; 2 failed` (`solid_open_line_all_butt_vertex_caps_stay_butt` and `thin_inside_dashed_rect_keeps_stroke` both `FAILED`, `plans/reports/agy-1478-dash-cap-evidence/r2-red-tests.log:9-218`) |
| **A1 (RED on `9dba68d2` — GPU)** | `cargo test -p designos-gpu --test fidelity -- solid_open_line_round_vertex_cap_renders_round_gpu thin_inside_dashed_rect_keeps_stroke_gpu` | `101` | `1 passed; 2 failed` (`solid_open_line_round_vertex_cap_renders_round_gpu` and `thin_inside_dashed_rect_keeps_stroke_gpu` both `FAILED`, `plans/reports/agy-1478-dash-cap-evidence/r2-red-tests.log:219-379`) |
| **A1 (RED on `9dba68d2` — `.fig`)** | `cargo test -p designos-fig --test fig_roundtrip -- fig_instance_stroke_override_clears_dashes` | `101` | `2 passed; 1 failed` (`fig_instance_stroke_override_clears_dashes` `FAILED`, `plans/reports/agy-1478-dash-cap-evidence/r2-red-tests.log:380-410`) |
| **Ruling 1 (`LockAspect` after merge)** | `cargo test -p designos-fig --test fig_roundtrip -- fig_instance_lock_aspect_override_roundtrip` | `0` | `ok. 2 passed; 0 failed` (`plans/reports/agy-1478-dash-cap-evidence/r2-lock-aspect-roundtrip.log:1-19`) |
| **A1 (GREEN on HEAD — CPU)** | `cargo test -p designos-render --test render -- dashed_rect_round_dash_cap_renders_round_dash_ends dashed_open_line_uses_stroke_cap_on_every_dash solid_open_line_all_butt_vertex_caps_stay_butt thin_inside_dashed_rect_keeps_stroke` | `0` | `ok. 4 passed; 0 failed` (`plans/reports/agy-1478-dash-cap-evidence/r2-green-tests.log:1-20`) |
| **A1 (GREEN on HEAD — GPU)** | `cargo test -p designos-gpu --test fidelity -- dashed_rect_round_dash_cap_renders_round_dash_ends_gpu dashed_open_line_uses_stroke_cap_on_every_dash_gpu solid_open_line_round_vertex_cap_renders_round_gpu thin_inside_dashed_rect_keeps_stroke_gpu` | `0` | `ok. 4 passed; 0 failed` (`plans/reports/agy-1478-dash-cap-evidence/r2-green-tests.log:21-43`) |
| **A1 (GREEN on HEAD — `.fig`/REST)** | `cargo test -p designos-fig --test fig_roundtrip -- fig_instance_stroke_override_preserves_dash_cap_and_dashes fig_instance_stroke_override_preserves_align_join_and_miter fig_instance_stroke_override_clears_dashes rest_instance_overrides_skips_layout_cleared` | `0` | `ok. 5 passed; 0 failed` (`plans/reports/agy-1478-dash-cap-evidence/r2-green-tests.log:44-61`) |
| **A2 (Null Control)** | `cargo test -p designos-render ... && cargo test -p designos-gpu ...` | `0` / `0` | `4 passed` CPU (`null_render_rc=0`), `4 passed` GPU (`null_gpu_rc=0`, `plans/reports/agy-1478-dash-cap-evidence/r2-mutants.log:1-31`) |
| **A2 (Mutant 1: `node_stroke_style` consults `vertex_caps`)** | `cargo test -p designos-gpu --test fidelity -- solid_open_line_round_vertex_cap_renders_round_gpu` | `101` | Killed by `solid_open_line_round_vertex_cap_renders_round_gpu` (`1 failed`, `mutant1_gpu_rc=101`, `plans/reports/agy-1478-dash-cap-evidence/r2-mutants.log:32-107`) |
| **A2 (Mutant 2: `None` fallback removed)** | `cargo test -p designos-render --test render -- thin_inside_dashed_rect_keeps_stroke && cargo test -p designos-gpu --test fidelity -- thin_inside_dashed_rect_keeps_stroke_gpu` | `101` / `101` | Killed by `thin_inside_dashed_rect_keeps_stroke` (`mutant2_render_rc=101`) and `thin_inside_dashed_rect_keeps_stroke_gpu` (`mutant2_gpu_rc=101`, `plans/reports/agy-1478-dash-cap-evidence/r2-mutants.log:108-219`) |
| **Gate 1 (`cargo fmt`)** | `cargo fmt --all -- --check` | `0` | `fmt_rc=0` (`plans/reports/agy-1478-dash-cap-evidence/r2-gates.log:1-2`) |
| **Gate 2a (`clippy --workspace`)** | `cargo clippy --workspace --all-targets --locked -- -D warnings` | `101` | Pre-existing `main` errors in `designos-app` only (`clippy_ws_rc=101`, `plans/reports/agy-1478-dash-cap-evidence/r2-gates.log:3-57`) |
| **Gate 2b (`clippy` touched crates)** | `cargo clippy -p designos-render -p designos-gpu -p designos-fig --all-targets --locked -- -D warnings` | `0` | `clippy_touched_rc=0` (`plans/reports/agy-1478-dash-cap-evidence/r2-gates.log:58-66`) |
| **Gate 3 (`designos-render` tests)** | `cargo test -p designos-render --locked` | `0` | `265 passed; 0 failed; 5 ignored` (`test_render_rc=0`, `plans/reports/agy-1478-dash-cap-evidence/r2-gates.log:67-534`) |
| **Gate 4 (`designos-gpu` tests)** | `cargo test -p designos-gpu --locked -- --skip large_file_compile_benchmarks` | `0` | `103 passed; 0 failed` (`test_gpu_rc=0`, `plans/reports/agy-1478-dash-cap-evidence/r2-gates.log:535-745`) |
| **Gate 5 (`designos-fig` tests)** | `cargo test -p designos-fig --locked` | `0` | `692 passed; 0 failed; 6 ignored` (`test_fig_rc=0`, `plans/reports/agy-1478-dash-cap-evidence/r2-gates.log:746-1565`) |
| **Gate 6 (`designos-app --bin designos`)** | `cargo test -p designos-app --bin designos --locked` + individual reruns | `101` -> `0` / `0` | Full run: `4205 passed; 2 failed` (`r2-gates.log:1566-5998`). Individual reruns: `two_deferred_text_commands_both_convert_in_order` -> `ok` (`rerun_vecops_rc=0`, `r2-gates.log:7687-7703`); `the_production_root_sits_outside_every_agent_writable_root` -> `ok` with short `HOME` (`rerun_ap_short_home_rc=0`, `r2-gates.log:7730-7744`) |
| **Gate 7 (`--workspace --lib`)** | `cargo test --workspace --locked --lib` | `0` | `1501 passed; 0 failed; 3 ignored` (`test_ws_lib_rc=0`, `plans/reports/agy-1478-dash-cap-evidence/r2-gates.log:5999-7659`) |
| **Gate 8 (`zero_fake_ui_gate.py`)** | `python3 scripts/zero_fake_ui_gate.py` | `0` | `Gate Verdict: PASS (17/17 invariants satisfied)` (`zero_fake_rc=0`, `plans/reports/agy-1478-dash-cap-evidence/r2-gates.log:7660-7686`) |
| **Cleanup (`rm -rf out/cargo "$S"`)** | `rm -rf out/cargo "$S" && ls out/cargo` | `1` | `ls: out/cargo: No such file or directory` (`ls_out_cargo_rc=1`, `plans/reports/agy-1478-dash-cap-evidence/r2-gates.log:7745-7747`) |

---

### Pasted Key Command Outputs (`r2-*` Evidence Logs)

1. **Acceptance 1 — RED on `9dba68d2` (`plans/reports/agy-1478-dash-cap-evidence/r2-red-tests.log:188-218,343-379,389-410`)**:
   ```text
   test solid_open_line_all_butt_vertex_caps_stay_butt ... FAILED
   test thin_inside_dashed_rect_keeps_stroke ... FAILED
   test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 30 filtered out; finished in 0.00s
   render_rc=101

   test thin_inside_dashed_rect_keeps_stroke_gpu ... FAILED
   test solid_open_line_round_vertex_cap_renders_round_gpu ... FAILED
   test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 19 filtered out; finished in 0.12s
   gpu_rc=101

   test fig_instance_stroke_override_clears_dashes ... FAILED
   test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 42 filtered out; finished in 0.02s
   fig_rc=101
   ```

2. **Acceptance 1 — GREEN on HEAD (`plans/reports/agy-1478-dash-cap-evidence/r2-green-tests.log:12-61`)**:
   ```text
   running 4 tests
   test dashed_open_line_uses_stroke_cap_on_every_dash ... ok
   test solid_open_line_all_butt_vertex_caps_stay_butt ... ok
   test thin_inside_dashed_rect_keeps_stroke ... ok
   test dashed_rect_round_dash_cap_renders_round_dash_ends ... ok
   test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 29 filtered out; finished in 0.00s
   render_rc=0

   running 4 tests
   test dashed_rect_round_dash_cap_renders_round_dash_ends_gpu ... ok
   test dashed_open_line_uses_stroke_cap_on_every_dash_gpu ... ok
   test thin_inside_dashed_rect_keeps_stroke_gpu ... ok
   test solid_open_line_round_vertex_cap_renders_round_gpu ... ok
   test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.16s
   gpu_rc=0

   running 5 tests
   test rest_instance_overrides_skips_layout_cleared ... ok
   test fig_instance_stroke_override_preserves_align_join_and_miter ... ok
   test fig_instance_stroke_override_clears_dashes ... ok
   test fig_instance_stroke_override_preserves_dash_cap_and_dashes ... ok
   test sync_instances_preserves_instance_child_stroke_cap_and_dashes_override ... ok
   test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 42 filtered out; finished in 0.04s
   fig_rc=0
   ```

3. **Acceptance 2 — Mutation Testing (`plans/reports/agy-1478-dash-cap-evidence/r2-mutants.log:84-107,164-219`)**:
   ```text
   test solid_open_line_round_vertex_cap_renders_round_gpu ... FAILED
   thread 'solid_open_line_round_vertex_cap_renders_round_gpu' (75914545) panicked at crates/designos-gpu/tests/fidelity.rs:643:5:
   assertion `left == right` failed: GPU round cap painted at (18, 20)
     left: [255, 255, 255, 255]
    right: [255, 0, 0, 255]
   test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 21 filtered out; finished in 0.16s
   mutant1_gpu_rc=101

   test thin_inside_dashed_rect_keeps_stroke ... FAILED
   thread 'thin_inside_dashed_rect_keeps_stroke' (75918418) panicked at crates/designos-render/tests/render.rs:815:5:
   assertion `left == right` failed: thin inside dashed rectangle paints first dash at (14, 12)
     left: [255, 255, 255, 255]
    right: [255, 0, 0, 255]
   test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 32 filtered out; finished in 0.00s
   mutant2_render_rc=101

   test thin_inside_dashed_rect_keeps_stroke_gpu ... FAILED
   thread 'thin_inside_dashed_rect_keeps_stroke_gpu' (75919840) panicked at crates/designos-gpu/tests/fidelity.rs:667:5:
   assertion `left == right` failed: GPU thin inside dashed rectangle paints dash at (14, 12)
     left: [255, 255, 255, 255]
    right: [255, 0, 0, 255]
   test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 21 filtered out; finished in 0.14s
   mutant2_gpu_rc=101
   ```

---

### Assumptions

- For thin inside-aligned dashed rectangles (`w >= n.size.x` or `w >= n.size.y`) where the inset centerline collapses (`dashed_rect_center_path` returns `None`) and rendering falls back to the `2 * w` clipped stroke along the outer perimeter, clamping `st.line_cap` to `sk::LineCap::Butt` when `width > s.weight` in `node_stroke_style` (`crates/designos-render/src/outline.rs:84-86`) is intentional so `tiny-skia` does not scale round/square cap protrusion to `w` (`2 * (w / 2)`) and bridge across dash gaps.

Status: COMPLETED (All Round 2 rulings 1–6 implemented and verified; RED->GREEN tests, mutation kills, citation checks, and gates logged in `plans/reports/agy-1478-dash-cap-evidence/r2-*.log`; `out/cargo` removed).
