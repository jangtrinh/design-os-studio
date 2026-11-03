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
3. **Correction 3 — `crates/designos-fig/tests/data/figma-real-schema.txt` line numbers** (corrected again in Round 3; the Round 2 ranges `359-381` and `457-460` were wrong and its citation log printed the lines without comparing text):
   - Round 1 cited shifted line ranges (`404-426`, `502-505`, `1986-2009`).
   - The actual line numbers, text-checked in `plans/reports/agy-1478-dash-cap-evidence/r3-citation-grep.log`:
     - `crates/designos-fig/tests/data/figma-real-schema.txt:220-241`: `Enum StrokeCap` (`NONE = 0`, `ROUND = 1`, `SQUARE = 2`, ... `ERD_MANY = 20`).
     - `crates/designos-fig/tests/data/figma-real-schema.txt:276-278`: `Enum DashMode` (`CLIP = 0`, `STRETCH = 1`).
     - `crates/designos-fig/tests/data/figma-real-schema.txt:1503`: `dashPattern = 11 : Float[]` in the `Stroke` message.
     - `crates/designos-fig/tests/data/figma-real-schema.txt:1890`: `dashPattern = 13 : Float[]` in the `NodeChange` message.
     - `crates/designos-fig/tests/data/figma-real-schema.txt:1906`: `strokeCap = 30 : StrokeCap` in the `NodeChange` message.
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
   - **Superseded in Round 3: the butt clamp described in this bullet was deleted; both fallbacks stroke with `s.cap` again.**
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
| **A1 (GREEN on HEAD — `.fig`/REST)** | `cargo test -p designos-fig --test fig_roundtrip -- rest_instance_overrides_skips_layout_cleared rest_export_import_preserves_dash_cap fig_instance_stroke_override_preserves_align_join_and_miter fig_export_import_preserves_dash_cap fig_instance_stroke_override_clears_dashes` (the Round 2 log does not record its filter; this command is rebuilt from the five names it printed and rerun in Round 3) | `0` | `ok. 5 passed; 0 failed` (`plans/reports/agy-1478-dash-cap-evidence/r2-green-tests.log:52-59`) |
| **A2 (Null Control)** | `cargo test -p designos-render ... && cargo test -p designos-gpu ...` | `0` / `0` | `4 passed` CPU (`null_render_rc=0`), `4 passed` GPU (`null_gpu_rc=0`, `plans/reports/agy-1478-dash-cap-evidence/r2-mutants.log:1-31`) |
| **A2 (Mutant 1: `node_stroke_style` consults `vertex_caps`)** | `cargo test -p designos-gpu --test fidelity -- solid_open_line_round_vertex_cap_renders_round_gpu` | `101` | Killed by `solid_open_line_round_vertex_cap_renders_round_gpu` (`1 failed`, `mutant1_gpu_rc=101`, `plans/reports/agy-1478-dash-cap-evidence/r2-mutants.log:32-107`) |
| **A2 (Mutant 2: `None` fallback removed)** | `cargo test -p designos-render --test render -- thin_inside_dashed_rect_keeps_stroke && cargo test -p designos-gpu --test fidelity -- thin_inside_dashed_rect_keeps_stroke_gpu` | `101` / `101` | Killed by `thin_inside_dashed_rect_keeps_stroke` (`mutant2_render_rc=101`) and `thin_inside_dashed_rect_keeps_stroke_gpu` (`mutant2_gpu_rc=101`, `plans/reports/agy-1478-dash-cap-evidence/r2-mutants.log:108-219`) |
| **Gate 1 (`cargo fmt`)** | `cargo fmt --all -- --check` | `0` | `fmt_rc=0` (`plans/reports/agy-1478-dash-cap-evidence/r2-gates.log:1-2`) |
| **Gate 2a (`clippy --workspace`)** | `cargo clippy --workspace --all-targets --locked -- -D warnings` | `101` | Pre-existing `main` errors in `designos-app` only (`clippy_ws_rc=101`, `plans/reports/agy-1478-dash-cap-evidence/r2-gates.log:3-57`) |
| **Gate 2b (`clippy` touched crates)** | `cargo clippy -p designos-render -p designos-gpu -p designos-fig --all-targets --locked -- -D warnings` | `0` | `clippy_touched_rc=0` (`plans/reports/agy-1478-dash-cap-evidence/r2-gates.log:58-66`) |
| **Gate 3 (`designos-render` tests)** | `cargo test -p designos-render --locked` | `0` | `283 passed; 0 failed; 5 ignored` (corrected in Round 4: the Round 2 report printed 265; this is the sum of the 29 `test result` lines in the cited range; `test_render_rc=0`, `plans/reports/agy-1478-dash-cap-evidence/r2-gates.log:67-534`) |
| **Gate 4 (`designos-gpu` tests)** | `cargo test -p designos-gpu --locked -- --skip large_file_compile_benchmarks` | `0` | `104 passed; 0 failed` (corrected in Round 4: the Round 2 report printed 103; sum of the 16 `test result` lines in the cited range; `test_gpu_rc=0`, `plans/reports/agy-1478-dash-cap-evidence/r2-gates.log:535-745`) |
| **Gate 5 (`designos-fig` tests)** | `cargo test -p designos-fig --locked` | `0` | `525 passed; 0 failed; 6 ignored` (corrected in Round 4: the Round 2 report printed 692; sum of the 47 `test result` lines in the cited range; `test_fig_rc=0`, `plans/reports/agy-1478-dash-cap-evidence/r2-gates.log:746-1565`) |
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

2. **Acceptance 1 — GREEN on HEAD (`plans/reports/agy-1478-dash-cap-evidence/r2-green-tests.log:12-18,32-43,52-61`, pasted as printed)**:
   ```text
   running 4 tests
   test solid_open_line_all_butt_vertex_caps_stay_butt ... ok
   test thin_inside_dashed_rect_keeps_stroke ... ok
   test dashed_open_line_uses_stroke_cap_on_every_dash ... ok
   test dashed_rect_round_dash_cap_renders_round_dash_ends ... ok

   test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 29 filtered out; finished in 0.00s

       Finished `test` profile [unoptimized + debuginfo] target(s) in 9.04s
        Running tests/fidelity.rs (out/cargo/debug/deps/fidelity-e9d67385509b7a81)

   running 4 tests
   test thin_inside_dashed_rect_keeps_stroke_gpu ... ok
   test solid_open_line_round_vertex_cap_renders_round_gpu ... ok
   test dashed_open_line_uses_stroke_cap_on_every_dash_gpu ... ok
   test dashed_rect_round_dash_cap_renders_round_dash_ends_gpu ... ok

   test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.16s

   gpu_rc=0

   running 5 tests
   test rest_instance_overrides_skips_layout_cleared ... ok
   test rest_export_import_preserves_dash_cap ... ok
   test fig_instance_stroke_override_preserves_align_join_and_miter ... ok
   test fig_export_import_preserves_dash_cap ... ok
   test fig_instance_stroke_override_clears_dashes ... ok

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

- **Superseded in Round 3: this assumption (the butt clamp) no longer applies.** For thin inside-aligned dashed rectangles (`w >= n.size.x` or `w >= n.size.y`) where the inset centerline collapses (`dashed_rect_center_path` returns `None`) and rendering falls back to the `2 * w` clipped stroke along the outer perimeter, clamping `st.line_cap` to `sk::LineCap::Butt` when `width > s.weight` in `node_stroke_style` (`crates/designos-render/src/outline.rs:84-86`) is intentional so `tiny-skia` does not scale round/square cap protrusion to `w` (`2 * (w / 2)`) and bridge across dash gaps.

Status: COMPLETED (All Round 2 rulings 1–6 implemented and verified; RED->GREEN tests, mutation kills, citation checks, and gates logged in `plans/reports/agy-1478-dash-cap-evidence/r2-*.log`; `out/cargo` removed).

## Round 3

Status of the code: head of the code under test is `6540b6ec` (`d1dc75b2` removes the butt clamp, `6540b6ec` moves the thin rectangle probes). `origin/main` had moved by one commit (`9a294a1b`); it was merged first (`fdf066b4`, no conflicts, merge not rebase). Round 3 evidence is in `plans/reports/agy-1478-dash-cap-evidence/r3-*`.

### What could not be verified, deviations, and predictions that were wrong

1. **The ruling's example dashes `[8, 20]` were replaced by `[8, 60]` in the thin rectangle tests.** On a 4px tall rectangle the stroke at twice the weight covers the full height, so the dashes on the bottom edge reach the same rows. With `[8, 20]` (period 28, outline 408) a bottom dash covered the probe at `(20, 11)`, and the thin test stayed green with the clamp put back, so it could not kill the mutant. I predicted red and observed green; that run's log was not kept (the mutant log was rewritten afterwards), so this is observed, not logged. With `[8, 40]` the probe `(19, 11)` was also reached by the bottom dash's start cap (center `(22, 14)`, radius 4, distance about 3.5), so the cap probe was not tied to the dash end above it. With `[8, 60]` (period 68) the first bottom dash sits at x = 66..74 and the probe is reached only by the top dash's end cap. The gap, 60, is longer than twice the weight, as ruling 2 requires.
2. **The GPU ellipse test asserts the anti-aliasing-forgiven ratio.** My first run failed on the strict ratio (that failing run's log was not kept; the final null-control log prints the same ratios): round cap `3.1000%` (310 px of 10000), butt cap `2.7200%` (272 px), while the forgiven ratio is `0.0000%` (`r3-null-green.log:19,23-25`). I had predicted green on the strict `< 0.02` gate; the curve edges differ by anti-aliasing only. The assert now follows the existing curve test at `crates/designos-gpu/tests/fidelity.rs:33` (`report_aa.ratio < 0.005`), and the probed pixels are still read exactly from the GPU image.
3. **"2.6px past the dash end" is the distance along the arc** (pixel `(68, 83)`, arc length 42.65 against a dash end at 40). Its straight-line distance from the dash end point `(71.6, 83.7)` is about 3.1 px, inside the cap radius of 4 (largest pixel-corner distance 3.67).
4. **Probe `(19, 11)` on the thin rectangle** is, with `[8, 60]`, reached only by the top dash's end cap; I computed this from the geometry, I did not render a cap-only image.
5. **The two old-source runs match because the stroke code is identical:** `git diff --stat 9dba68d2 56869cad -- crates/designos-render/src crates/designos-gpu/src` is empty (`r3-old-source.log:1-2`). Both runs check out only `designos-render/src/{lib,outline,vectorops}.rs` from the named commit and keep everything else at HEAD.
6. **Scope of the gates:** exactly the five in the brief. I did not run the full `designos-fig` suite, the `designos-app` binary tests or `zero_fake_ui_gate.py` in Round 3; the five `fig_roundtrip` tests named in the review were rerun alone (`r3-fig-green-names.log`).
7. **The stamps at the top of `r3-mutant.log` and `r3-old-source.log` print `git status` after the mutation or checkout**, so they show the changed files on purpose; the lines after each restore print an empty status.
8. **Report-only commit after the gates:** the gates ran on `6540b6ec`; the report commit changes only `plans/reports/**` (`git diff --name-only 6540b6ec HEAD` is pasted in the final message).

### Rulings

| Ruling | Result | Evidence |
|---|---|---|
| 1. Delete the butt clamp in `node_stroke_style`; no cap override there | Done. `node_stroke_style` (`crates/designos-render/src/outline.rs:66-91`) copies `s.cap` (line 71) and `if has_dashes(s)` (line 83) only builds the dash array. Both `2 * weight` fallbacks stroke with `s.cap`. | `git show d1dc75b2 -- crates/designos-render/src/outline.rs` deletes the three clamp lines; mutant diff at `r3-mutant.log:7-20` |
| 2. Thin rectangle tests: gap longer than twice the weight; dash, cap and gap probes; red on `9dba68d2` | Done, with `[8, 60]` (deviation 1). CPU `crates/designos-render/tests/render.rs:802-822`, GPU `crates/designos-gpu/tests/fidelity.rs:653-688`. Probes `(14, 12)` painted, `(19, 11)` painted cap, `(40, 12)` white gap. | green `r3-null-green.log:7,21`; red on `9dba68d2` source `r3-old-source.log:16,19,29-34`; red on `56869cad` source `r3-old-source.log:55,58,68-73` |
| 3. Inside dashed ellipse, round cap, gap longer than twice the weight, CPU and GPU | Done. CPU `render.rs:825-848`, GPU `fidelity.rs:691-720`. Circle of radius 40, weight 4, dashes `[40, 20]`; probes `(83, 68)` body, `(68, 83)` round cap past the dash end, `(61, 86)` gap; butt contrast `(68, 83)` white. | passes on `56869cad` and `9dba68d2` sources (`r3-old-source.log:17,38,56,77`); fails with the clamp put back (`r3-mutant.log:25,31-34,52-56`) |
| 4. Report fixes | `Enum StrokeCap` is `figma-real-schema.txt:220-241`, `Enum DashMode` is `:276-278`; green `fig_roundtrip` names pasted as printed; Round 2 text that the clamp made false is marked superseded | `r3-citation-grep.log`, `r3-fig-green-names.log:6-11` |

### Mutant (after a green null control)

Null control on the committed tree, then the clamp put back (`outline.rs` taken from `bd099a3a:crates/designos-render/src/outline.rs`, not from `HEAD`), then restored with `git checkout HEAD -- crates/designos-render/src/outline.rs`. The mutant run kills four tests: the thin rectangle and the ellipse, each on the CPU and the GPU.

Null control (`r3-null-green.log:4-29`):
```text
   # null control: the committed tree, no mutation
   ## designos-render
   test dashed_line_and_caps ... ok
   test thin_inside_dashed_rect_keeps_stroke ... ok
   test dashed_open_line_uses_stroke_cap_on_every_dash ... ok
   test dashed_rect_round_dash_cap_renders_round_dash_ends ... ok
   test inside_dashed_ellipse_round_cap_extends_past_dash_end ... ok
   test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 29 filtered out; finished in 0.00s
   render_rc=0
   ## designos-gpu (Metal)
   thin_inside_dashed_rect: mismatch 0.7955% (42 px of 5280) at tolerance 8, ssim 0.9991
   inside_dashed_ellipse_round_cap: mismatch 3.1000% (310 px of 10000) at tolerance 8, ssim 0.9962
   thin_inside_dashed_rect: 0.0000% with AA pixels forgiven (0 px)
   test thin_inside_dashed_rect_keeps_stroke_gpu ... ok
   test dashed_open_line_uses_stroke_cap_on_every_dash_gpu ... ok
   inside_dashed_ellipse_round_cap: 0.0000% with AA pixels forgiven (0 px)
   inside_dashed_ellipse_butt_cap: mismatch 2.7200% (272 px of 10000) at tolerance 8, ssim 0.9969
   inside_dashed_ellipse_butt_cap: 0.0000% with AA pixels forgiven (0 px)
   test inside_dashed_ellipse_round_cap_extends_past_dash_end_gpu ... ok
   test dashed_rect_round_dash_cap_renders_round_dash_ends_gpu ... ok
   test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 19 filtered out; finished in 0.20s
   gpu_rc=0
```

Mutant (`r3-mutant.log:5-36,44-60`):
```text
   # mutant: node_stroke_style forces butt on dashed strokes wider than the weight (outline.rs taken from bd099a3a)
   ## mutation (git diff against the committed tree)
   diff --git a/crates/designos-render/src/outline.rs b/crates/designos-render/src/outline.rs
   index 2015b8f5..e0d7edec 100644
   --- a/crates/designos-render/src/outline.rs
   +++ b/crates/designos-render/src/outline.rs
   @@ -81,6 +81,9 @@ pub fn node_stroke_style(n: &Node, width: f32) -> sk::Stroke {
            dash: None,
        };
        if has_dashes(s) {
   +        if width > s.weight {
   +            st.line_cap = sk::LineCap::Butt;
   +        }
            let mut v = s.dashes.clone();
            if v.len() % 2 == 1 {
                v.extend_from_within(..);
   ## designos-render
   test dashed_line_and_caps ... ok
   test dashed_open_line_uses_stroke_cap_on_every_dash ... ok
   test thin_inside_dashed_rect_keeps_stroke ... FAILED
   test inside_dashed_ellipse_round_cap_extends_past_dash_end ... FAILED
   test dashed_rect_round_dash_cap_renders_round_dash_ends ... ok
   thread 'thin_inside_dashed_rect_keeps_stroke' (76637311) panicked at crates/designos-render/tests/render.rs:820:5:
   assertion `left == right` failed: thin inside dashed rectangle paints the round cap just past the dash end at (19, 11)
     left: [255, 255, 255, 255]
    right: [255, 0, 0, 255]
   thread 'inside_dashed_ellipse_round_cap_extends_past_dash_end' (76637310) panicked at crates/designos-render/tests/render.rs:841:5:
   assertion `left == right` failed: round cap is painted 2.6px past the dash end at (68, 83)
     left: [255, 255, 255, 255]
    right: [255, 0, 0, 255]
   test result: FAILED. 3 passed; 2 failed; 0 ignored; 0 measured; 29 filtered out; finished in 0.00s
   render_rc=101
   thread 'thin_inside_dashed_rect_keeps_stroke_gpu' (76639205) panicked at crates/designos-gpu/tests/fidelity.rs:677:5:
   assertion `left == right` failed: GPU thin inside dashed rectangle paints the round cap just past the dash end at (19, 11)
     left: [255, 255, 255, 255]
    right: [255, 0, 0, 255]
   inside_dashed_ellipse_round_cap: mismatch 2.7200% (272 px of 10000) at tolerance 8, ssim 0.9969
   test dashed_open_line_uses_stroke_cap_on_every_dash_gpu ... ok
   test thin_inside_dashed_rect_keeps_stroke_gpu ... FAILED
   inside_dashed_ellipse_round_cap: 0.0000% with AA pixels forgiven (0 px)
   thread 'inside_dashed_ellipse_round_cap_extends_past_dash_end_gpu' (76639204) panicked at crates/designos-gpu/tests/fidelity.rs:711:5:
   assertion `left == right` failed: GPU round cap painted 2.6px past the dash end at (68, 83)
     left: [255, 255, 255, 255]
    right: [255, 0, 0, 255]
   test inside_dashed_ellipse_round_cap_extends_past_dash_end_gpu ... FAILED
   test dashed_rect_round_dash_cap_renders_round_dash_ends_gpu ... ok
   test result: FAILED. 2 passed; 2 failed; 0 ignored; 0 measured; 19 filtered out; finished in 0.24s
   gpu_rc=101
   # after restore: git status --short -- crates (empty = clean):
```

### Red on the old sources (`r3-old-source.log`)

```text
   # git diff --stat 9dba68d2 56869cad -- crates/designos-render/src crates/designos-gpu/src (empty = identical stroke code):

   # new tests run against designos-render/src/{lib,outline,vectorops}.rs of 9dba68d2; everything else at HEAD
   # files differing from HEAD:
    crates/designos-render/src/lib.rs       |  8 ++------
    crates/designos-render/src/outline.rs   | 15 ++++++++-------
    crates/designos-render/src/vectorops.rs |  2 +-
    3 files changed, 11 insertions(+), 14 deletions(-)
   ## designos-render
   test thin_inside_dashed_rect_keeps_stroke ... FAILED
   test inside_dashed_ellipse_round_cap_extends_past_dash_end ... ok
   thread 'thin_inside_dashed_rect_keeps_stroke' (76640655) panicked at crates/designos-render/tests/render.rs:819:5:
   assertion `left == right` failed: thin inside dashed rectangle paints the first dash at (14, 12)
     left: [255, 255, 255, 255]
    right: [255, 0, 0, 255]
   test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 32 filtered out; finished in 0.00s
   render_rc=101
   thin_inside_dashed_rect: mismatch 0.0000% (0 px of 5280) at tolerance 8, ssim 1.0000
   thin_inside_dashed_rect: 0.0000% with AA pixels forgiven (0 px)
   thread 'thin_inside_dashed_rect_keeps_stroke_gpu' (76642639) panicked at crates/designos-gpu/tests/fidelity.rs:671:5:
   assertion `left == right` failed: GPU thin inside dashed rectangle paints the first dash at (14, 12)
     left: [255, 255, 255, 255]
    right: [255, 0, 0, 255]
   inside_dashed_ellipse_round_cap: mismatch 3.1000% (310 px of 10000) at tolerance 8, ssim 0.9962
   test thin_inside_dashed_rect_keeps_stroke_gpu ... FAILED
   inside_dashed_ellipse_round_cap: 0.0000% with AA pixels forgiven (0 px)
   inside_dashed_ellipse_butt_cap: mismatch 2.7200% (272 px of 10000) at tolerance 8, ssim 0.9969
   inside_dashed_ellipse_butt_cap: 0.0000% with AA pixels forgiven (0 px)
   test inside_dashed_ellipse_round_cap_extends_past_dash_end_gpu ... ok
   test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 21 filtered out; finished in 0.20s
   gpu_rc=101
   # new tests run against designos-render/src/{lib,outline,vectorops}.rs of 56869cad; everything else at HEAD
   # files differing from HEAD:
    crates/designos-render/src/lib.rs       |  8 ++------
    crates/designos-render/src/outline.rs   | 15 ++++++++-------
    crates/designos-render/src/vectorops.rs |  2 +-
    3 files changed, 11 insertions(+), 14 deletions(-)
   ## designos-render
   test thin_inside_dashed_rect_keeps_stroke ... FAILED
   test inside_dashed_ellipse_round_cap_extends_past_dash_end ... ok
   thread 'thin_inside_dashed_rect_keeps_stroke' (76643802) panicked at crates/designos-render/tests/render.rs:819:5:
   assertion `left == right` failed: thin inside dashed rectangle paints the first dash at (14, 12)
     left: [255, 255, 255, 255]
    right: [255, 0, 0, 255]
   test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 32 filtered out; finished in 0.00s
   render_rc=101
   thin_inside_dashed_rect: mismatch 0.0000% (0 px of 5280) at tolerance 8, ssim 1.0000
   thin_inside_dashed_rect: 0.0000% with AA pixels forgiven (0 px)
   thread 'thin_inside_dashed_rect_keeps_stroke_gpu' (76645492) panicked at crates/designos-gpu/tests/fidelity.rs:671:5:
   assertion `left == right` failed: GPU thin inside dashed rectangle paints the first dash at (14, 12)
     left: [255, 255, 255, 255]
    right: [255, 0, 0, 255]
   inside_dashed_ellipse_round_cap: mismatch 3.1000% (310 px of 10000) at tolerance 8, ssim 0.9962
   test thin_inside_dashed_rect_keeps_stroke_gpu ... FAILED
   inside_dashed_ellipse_round_cap: 0.0000% with AA pixels forgiven (0 px)
   inside_dashed_ellipse_butt_cap: mismatch 2.7200% (272 px of 10000) at tolerance 8, ssim 0.9969
   inside_dashed_ellipse_butt_cap: 0.0000% with AA pixels forgiven (0 px)
   test inside_dashed_ellipse_round_cap_extends_past_dash_end_gpu ... ok
   test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 21 filtered out; finished in 0.31s
   gpu_rc=101
   # after restore: git status --short -- crates (empty = clean):
```

### Citation check (`r3-citation-grep.log`)

Each corrected cite is checked by comparing the quoted words with the text at that line (`awk 'NR==n'` plus a substring test), not by printing the line. The two old wrong cites (`:359` `Enum StrokeCap`, `:457` `Enum DashMode`) are included as negative rows and print `FAIL`, which proves the check reads the text. `cite_unexpected=0` closes the log.
```text
   # text-at-line check: PASS = the quoted words are on that line. HEAD 6540b6ec
   PASS expect=PASS ok        crates/designos-fig/tests/data/figma-real-schema.txt:220 needle=[Enum StrokeCap] line=[Enum StrokeCap]
   PASS expect=PASS ok        crates/designos-fig/tests/data/figma-real-schema.txt:221 needle=[NONE = 0] line=[    NONE = 0]
   PASS expect=PASS ok        crates/designos-fig/tests/data/figma-real-schema.txt:222 needle=[ROUND = 1] line=[    ROUND = 1]
   PASS expect=PASS ok        crates/designos-fig/tests/data/figma-real-schema.txt:223 needle=[SQUARE = 2] line=[    SQUARE = 2]
   PASS expect=PASS ok        crates/designos-fig/tests/data/figma-real-schema.txt:241 needle=[ERD_MANY = 20] line=[    ERD_MANY = 20]
   PASS expect=PASS ok        crates/designos-fig/tests/data/figma-real-schema.txt:242 needle=[Enum StrokeJoin] line=[Enum StrokeJoin]
   PASS expect=PASS ok        crates/designos-fig/tests/data/figma-real-schema.txt:276 needle=[Enum DashMode] line=[Enum DashMode]
   PASS expect=PASS ok        crates/designos-fig/tests/data/figma-real-schema.txt:277 needle=[CLIP = 0] line=[    CLIP = 0]
   PASS expect=PASS ok        crates/designos-fig/tests/data/figma-real-schema.txt:278 needle=[STRETCH = 1] line=[    STRETCH = 1]
   PASS expect=PASS ok        crates/designos-fig/tests/data/figma-real-schema.txt:1503 needle=[dashPattern = 11] line=[    dashPattern = 11 : Float[]]
   PASS expect=PASS ok        crates/designos-fig/tests/data/figma-real-schema.txt:1890 needle=[dashPattern = 13] line=[    dashPattern = 13 : Float[]]
   PASS expect=PASS ok        crates/designos-fig/tests/data/figma-real-schema.txt:1906 needle=[strokeCap = 30] line=[    strokeCap = 30 : StrokeCap]
   FAIL expect=FAIL ok        crates/designos-fig/tests/data/figma-real-schema.txt:359 needle=[Enum StrokeCap] line=[Enum StackPositioning]
   FAIL expect=FAIL ok        crates/designos-fig/tests/data/figma-real-schema.txt:457 needle=[Enum DashMode] line=[    PARAGRAPH = 4]
   FAIL expect=FAIL ok        crates/designos-fig/tests/data/figma-real-schema.txt:460 needle=[STRETCH = 1] line=[    HORIZONTAL = 1]
   cite_unexpected=0
```

### Green `fig_roundtrip` names as printed (`r3-fig-green-names.log:5-12`)

```text
   running 5 tests
   test rest_instance_overrides_skips_layout_cleared ... ok
   test rest_export_import_preserves_dash_cap ... ok
   test fig_instance_stroke_override_preserves_align_join_and_miter ... ok
   test fig_export_import_preserves_dash_cap ... ok
   test fig_instance_stroke_override_clears_dashes ... ok
   test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 42 filtered out; finished in 0.02s
   fig_rc=0
```

### Gates on `6540b6ec` (`r3-gates.log`)

| Gate | Result | Log line |
|---|---|---|
| `cargo fmt --all -- --check` | `fmt_rc=0` | `r3-gates.log:5` |
| `cargo clippy -p designos-render -p designos-gpu --all-targets --locked -- -D warnings` | `clippy_rc=0` | `r3-gates.log:16` |
| `cargo test -p designos-render --locked` | 284 passed, 0 failed, 5 ignored (sum of the 29 `test result` lines); `test_render_rc=0` | `r3-gates.log:76` |
| `cargo test -p designos-gpu --locked -- --skip large_file_compile_benchmarks` | 105 passed, 0 failed (sum of the 16 `test result` lines); `test_gpu_rc=0` | `r3-gates.log:110` |
| `cargo test --workspace --locked --lib` | 1501 passed, 0 failed, 3 ignored (sum of the 12 `test result` lines); `test_ws_lib_rc=0` | `r3-gates.log:143` |

### Visual proof

`plans/reports/agy-1478-dash-cap-evidence/r3-proof-sheet.png` (CPU and GPU renders from the passing run, 4x, probes boxed: green expects painted, blue expects white). The ellipse with a round cap shows round dash ends and the butt contrast shows flat ones; the thin rectangle shows the dashes painted. The per-render PNGs are `r3-thin_inside_dashed_rect-{cpu,gpu}.png`, `r3-inside_dashed_ellipse_round_cap-{cpu,gpu}.png`, `r3-inside_dashed_ellipse_butt_cap-cpu.png`.

### Assumptions

- Closing the clamp question as "main's behaviour": an inside or outside dashed non-rectangle stroked at twice the weight keeps its own cap. Confidence high; evidence: the ellipse probe passes on the `56869cad` and `9dba68d2` stroke code (`r3-old-source.log:17,38,56,77`). It flips if the reviewer shows a rendering reference where the cap radius is `weight` rather than `2 * weight / 2`.

Status: DONE (code and report; PR not marked ready, not merged, not commented)

## Round 4

CI `Test (macos-15)` failed on `0ffe4f36`, deterministically: `designos-export` `pdf_vector_fidelity` `randomised_frames_of_supported_features_are_vector_and_match_the_canvas`, seeds 6, 17, 49, 69, 83. Rounds 1-3 never ran `designos-export`. `origin/main` had moved to `e7ace20b` and was merged first (`a0b4f32b`, merge not rebase, no conflicts). Code under test is `6aabc599`. Round 4 evidence is in `plans/reports/agy-1478-dash-cap-evidence/r4-*`.

### What could not be verified, deviations, and open concerns

1. **The CI job was not re-run locally.** The local run is `cargo test -p designos-export --locked --test pdf_vector_fidelity` on macOS here; the CI result is in the final message, not in this report.
2. **Repro and red runs used `a0b4f32b`, not `0ffe4f36`.** `a0b4f32b` is `0ffe4f36` plus the `origin/main` merge; the export, render, gpu and doc source is the same (`r4-base.log:2-3` is an empty diff stat), the merge only touched `designos-fig` (`r4-base.log:4-6`). The local run reproduces CI exactly: same five seeds, same counts (`r4-repro-head.log:53-58`).
3. **"Passes on `origin/main`" was checked without a scratch worktree.** I took the head tree and put `crates/designos-render/src/{lib,outline,vectorops}.rs` back to `e7ace20b`; the export, gpu and doc source already equal main's. The `git diff --stat` printed in `r4-repro-main.log:2-4` lists two test files only, so its trailing "empty = identical" label is misleading: the non-test source of the compared crates matches `e7ace20b`. Result: `49 passed; 0 failed` (`r4-repro-main.log:6`).
4. **One line outside `crates/designos-export`:** `crates/designos-render/src/lib.rs` re-exports `dashed_rect_center_path` (a public API addition, `lib.rs:39`), so the PDF writer and the canvas share one helper instead of a second copy of the inset outline rules.
5. **Same bug class in two other consumers, not fixed here (the task scopes the fix to the PDF failure):**
   - `crates/designos-export/src/svg.rs:597-626` strokes an inside or outside dashed rectangle at `w * 2.0` on the boundary under a clip or mask, like the PDF writer did.
   - `crates/designos-render/src/vectorops.rs:380-382,391` (`outline_stroke`, the "Outline stroke" action) expands closed aligned shapes with `st_double` (`w * 2.0`) and clips, so a dashed inside or outside rectangle outlines differently from the canvas.
   - I read both and did not render either; a test for each would show the divergence the same way `pdf_vector_fidelity` did.
6. **Residual differences after the fix.** Seeds 17 and 49 still show 7 and 17 differing samples (threshold 48), in a bounding box that is identical to `origin/main` before this PR: `r4-residual.log` lists `main-pre-PR 7/9600` and `17/9600` with the same boxes and maxima. They are not caused by this PR.
7. **Proof-sheet numbers come from two sources.** "126" and "202" are the Rust test's counts (`r4-repro-head.log:54-58`); "0 after" is a Python recount of the dumped PNGs; the same Python recount of the before images gives 121 and 201 (`r4-residual.log`), a few samples different from the test's own counts because of how the canvas picture is composited. The recount is labelled as such on the sheet.
8. **Round 2 gate counts were wrong against their own log** and are corrected in the Round 2 table above: render `283` (printed `265`), gpu `104` (printed `103`), fig `525` (printed `692`).
9. **Workspace clippy is not clean, by the known lints only** (`r4-gates.log:7-18`): `crates/designos-app/src/agent_permissions.rs:1176`, `:1194` and `crates/designos-app/src/shell/tests/inspector_var_binds.rs:1181`. None of those files is changed by this PR (`r4-clippy-scope.log:20-21` is empty). `designos-app/src/panels/stroke.rs` is changed and reports no lint. Scoped clippy on render, gpu, export and fig is clean (`clippy_scoped_rc=0`, `r4-clippy-scope.log:23`).
10. **A test-file helper was added:** the randomised test now also writes the page raster next to the canvas picture when `DESIGNOS_PDF_DUMP_DIR` is set (`pdf_vector_fidelity.rs`, the dump block of `random_frames_agree`). It changes no assertion.

### Root cause

The canvas and the PDF stroke a dashed inside or outside rectangle on different outlines at different widths.

- Canvas (`crates/designos-render/src/lib.rs:916` in `0ffe4f36`, `draw_strokes`): for a dashed rectangle-like shape with `Inside` or `Outside` alignment it strokes the inset or outset outline from `outline::dashed_rect_center_path` (`outline.rs:221`) at the stroke's own weight `w`, clipped to one side. The GPU does the same through `stroke_plan` (`outline.rs:279` in `0ffe4f36`). Commit `9dba68d2` introduced `dashed_rect_center_path` (`r4-base.log:12-13`).
- PDF (`crates/designos-export/src/pdf.rs:1107` and `:1123` in `0ffe4f36`, `stroke_line`): width `weight * 2.0` for every inside or outside stroke, stroking `path_cmds(&sh.paths)`, the node boundary, under the same clip.
- Effect: a PDF dash cap has radius `w` instead of `w / 2`, so round and square caps overlap the gaps and the dashes run together (the middle panel of the sheet below); the dash phase also differs because the boundary and the inset outline have different lengths, which is why butt caps differ too (seed 69, outside, no cap).
- On `origin/main` both sides used the doubled boundary, which is why main passes.
- The failing nodes are dashed rectangles with `Inside` or `Outside` alignment: seed 83 node 6 (inside, round cap, bbox of the differing samples x 121..189, y 75..111, equal to the node bounds), seed 69 node 6 (outside, no cap), seeds 6 and 17 (inside), seed 49 (outside, rounded and rotated). Centered dashed strokes and the thin-rectangle fallback already agreed.

### Fix

`crates/designos-export/src/pdf.rs:1098-1136` (`stroke_line`): for a rectangle-like shape with a valid dash pattern and `Inside` or `Outside` alignment, when `dashed_rect_center_path` returns an outline, stroke that outline at the stroke's own weight (clip unchanged); otherwise keep the doubled boundary, which is what the canvas does for the thin-rectangle fallback.

### Test-first

Two focused tests in `crates/designos-export/tests/pdf_vector_fidelity.rs`: `dashed_inside_and_outside_rectangle_strokes_match_the_canvas` (inside round, inside butt, inside square with rounded corners, outside square, outside butt with rounded corners; each also asserts the canvas paints the first dash) and `thin_dashed_rectangle_stroke_matches_the_canvas` (a regression guard for the unchanged fallback, green before and after).

Red on the export code of the failing head (`r4-red.log`):
```text
   # new pdf_vector_fidelity tests against the export code of 0ffe4f36 (crates/designos-export/src unchanged; only tests edited). git status --short -- crates:
    M crates/designos-export/tests/pdf_vector_fidelity.rs
   # ---
   test thin_dashed_rectangle_stroke_matches_the_canvas ... ok
   test dashed_inside_and_outside_rectangle_strokes_match_the_canvas ... FAILED
   test randomised_frames_of_supported_features_are_vector_and_match_the_canvas ... FAILED
   thread 'dashed_inside_and_outside_rectangle_strokes_match_the_canvas' (77632215) panicked at crates/designos-export/tests/pdf_vector_fidelity.rs:507:5:
   Inside Round dashed rectangle: 206 of 6000 sampled pixels differ from the canvas
   Inside None dashed rectangle: 250 of 6000 sampled pixels differ from the canvas
   Inside Square dashed rectangle: 31 of 6000 sampled pixels differ from the canvas
   Outside Square dashed rectangle: 170 of 6000 sampled pixels differ from the canvas
   Outside None dashed rectangle: 218 of 6000 sampled pixels differ from the canvas
   thread 'randomised_frames_of_supported_features_are_vector_and_match_the_canvas' (77632216) panicked at crates/designos-export/tests/pdf_vector_fidelity.rs:1258:5:
   5 of 150 random frames differ from the canvas:
   seed 6: 65/9600 sampled pixels differ on the Vector page
   seed 17: 70/9600 sampled pixels differ on the Vector page
   seed 49: 105/9600 sampled pixels differ on the Vector page
   seed 69: 202/9600 sampled pixels differ on the Vector page
   seed 83: 126/9600 sampled pixels differ on the Vector page
   test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 48 filtered out; finished in 5.60s
```

Green on `6aabc599` (`r4-green.log`):
```text
   # green: the committed tree
   test dashed_inside_and_outside_rectangle_strokes_match_the_canvas ... ok
   test thin_dashed_rectangle_stroke_matches_the_canvas ... ok
   test randomised_frames_with_unsupported_features_still_match_the_canvas ... ok
   test randomised_frames_of_supported_features_are_vector_and_match_the_canvas ... ok
   test result: ok. 51 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.61s
   export_rc=0
```

Mutant: `crates/designos-export/src/pdf.rs` taken from `a0b4f32b`, then restored (`r4-mutant.log`); the two tests fail with the same numbers as the red run:
```text
   # mutant: crates/designos-export/src/pdf.rs taken from a0b4f32b (export code of the failing head)
   ## mutation (git diff against the committed tree)
    crates/designos-export/src/pdf.rs | 17 +++++------------
    1 file changed, 5 insertions(+), 12 deletions(-)
   test dashed_inside_and_outside_rectangle_strokes_match_the_canvas ... FAILED
   test thin_dashed_rectangle_stroke_matches_the_canvas ... ok
   test randomised_frames_with_unsupported_features_still_match_the_canvas ... ok
   test randomised_frames_of_supported_features_are_vector_and_match_the_canvas ... FAILED
   thread 'dashed_inside_and_outside_rectangle_strokes_match_the_canvas' (77649501) panicked at crates/designos-export/tests/pdf_vector_fidelity.rs:507:5:
   Inside Round dashed rectangle: 206 of 6000 sampled pixels differ from the canvas
   Inside None dashed rectangle: 250 of 6000 sampled pixels differ from the canvas
   Inside Square dashed rectangle: 31 of 6000 sampled pixels differ from the canvas
   Outside Square dashed rectangle: 170 of 6000 sampled pixels differ from the canvas
   Outside None dashed rectangle: 218 of 6000 sampled pixels differ from the canvas
   ---- randomised_frames_of_supported_features_are_vector_and_match_the_canvas stdout ----
   thread 'randomised_frames_of_supported_features_are_vector_and_match_the_canvas' (77649530) panicked at crates/designos-export/tests/pdf_vector_fidelity.rs:1258:5:
   seed 6: 65/9600 sampled pixels differ on the Vector page
   seed 17: 70/9600 sampled pixels differ on the Vector page
   seed 49: 105/9600 sampled pixels differ on the Vector page
   seed 69: 202/9600 sampled pixels differ on the Vector page
   seed 83: 126/9600 sampled pixels differ on the Vector page
       dashed_inside_and_outside_rectangle_strokes_match_the_canvas
       randomised_frames_of_supported_features_are_vector_and_match_the_canvas
   test result: FAILED. 49 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.65s
   export_rc=101
   # after restore: git status --short -- crates (empty = clean):
```

### Before and after (synthetic test frames only)

`r4-before-after.png`: canvas, PDF page before the fix and PDF page after the fix for seed 83 (inside, round cap) and seed 69 (outside, butt cap), cropped to the node and magnified 3x. `r4-seed83-canvas.png`, `r4-seed83-pdf-before.png` and `r4-seed83-pdf-after.png` are the full frames (480 x 320). Differing samples per seed, before the fix, after, and on `origin/main` before this PR (`r4-residual.log`):
```text
   seed 6: before-fix 65/9600 bbox=[187.0, 87.0, 223.0, 149.0] max=135 | after-fix 0/9600 bbox=- max=0 | main-pre-PR 0/9600 bbox=- max=0
   seed 17: before-fix 72/9600 bbox=[123.0, 21.0, 233.0, 159.0] max=64 | after-fix 7/9600 bbox=[123.0, 21.0, 233.0, 35.0] max=64 | main-pre-PR 7/9600 bbox=[123.0, 21.0, 233.0, 35.0] max=64
   seed 49: before-fix 105/9600 bbox=[139.0, 15.0, 239.0, 67.0] max=71 | after-fix 17/9600 bbox=[139.0, 15.0, 237.0, 49.0] max=48 | main-pre-PR 17/9600 bbox=[139.0, 15.0, 237.0, 49.0] max=48
   seed 69: before-fix 201/9600 bbox=[67.0, 65.0, 137.0, 121.0] max=232 | after-fix 0/9600 bbox=- max=0 | main-pre-PR 0/9600 bbox=- max=0
   seed 83: before-fix 121/9600 bbox=[121.0, 75.0, 189.0, 111.0] max=166 | after-fix 0/9600 bbox=- max=0 | main-pre-PR 0/9600 bbox=- max=0

```

### Gates on `6aabc599` (`r4-gates.log`, `r4-clippy-scope.log`)

| Gate | Result | Log line |
|---|---|---|
| `cargo fmt --all -- --check` | `fmt_rc=0` | `r4-gates.log:5` |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | `clippy_ws_rc=101`, only the three known lints above (finding 9) | `r4-gates.log:7-18` |
| scoped clippy: render, gpu, export, fig | `clippy_scoped_rc=0` | `r4-clippy-scope.log:23` |
| `cargo test -p designos-render --locked` | 284 passed, 0 failed, 5 ignored (29 result lines); `test_render_rc=0` | `r4-gates.log:49` |
| `cargo test -p designos-gpu --locked -- --skip large_file_compile_benchmarks` | 105 passed, 0 failed (16 result lines); `test_gpu_rc=0` | `r4-gates.log:67` |
| `cargo test -p designos-export --locked` | 341 passed, 0 failed, 5 ignored (16 result lines); `test_export_rc=0` | `r4-gates.log:85` |
| `cargo test -p designos-fig --locked` | 527 passed, 0 failed, 6 ignored (48 result lines); `test_fig_rc=0` | `r4-gates.log:136` |
| `cargo test --workspace --locked --lib` | 1501 passed, 0 failed, 3 ignored (12 result lines); `test_ws_lib_rc=0` | `r4-gates.log:157` |

Report-only commits follow the gated commit `6aabc599`; the final message pastes `git diff --name-only 6aabc599 HEAD`.

### Assumptions

- The canvas (`designos-render` CPU) is the reference and is right: the GPU renderer already follows the same outline through `stroke_plan`, and the PR's purpose is the dash cap on that outline. Confidence high. Evidence that would flip it: a Figma reference where an inside dashed rectangle's caps are drawn at radius `w`.

Status: DONE_WITH_CONCERNS (PDF fix and tests are done; the SVG exporter and Outline stroke have the same divergence, see finding 5)

## Round 5

The canvas strokes an inside or outside dashed rectangle along its inset or outset outline at the stroke's own weight (`dashed_rect_center_path`). After round 4 two consumers still stroked the node boundary at twice the weight, so their dash phase and cap size differed from the canvas: SVG export and the Outline stroke action. Both now follow the canvas. `origin/main` (`f0123d99`) was merged first (`a7da7e00`, merge not rebase, no conflicts; it only deletes two `out/evidence` PNGs). Code is `ddf87421`; evidence is in `plans/reports/agy-1478-dash-cap-evidence/r5-*`.

### One rule in one place

`outline::dashed_rect_stroke_path(n, sh)` (`crates/designos-render/src/outline.rs:229`, re-exported at `lib.rs:39`) returns the inset or outset outline when the canvas draws the node's aligned dashed stroke that way, else `None`. It checks the shape (`Shape::is_rect`, `closed`), the dash pattern, per-side weights, precomputed stroke outlines and thin rectangles. Consumers:
- canvas `draw_strokes` (`lib.rs:915`) and GPU `stroke_plan` (`outline.rs:294`): switched. `designos-render` (289 passed) and `designos-gpu` (105 passed) tests are unchanged and green, so the canvas path stays bit-identical on everything they render;
- PDF (`pdf.rs:1102`): round 4's own gating replaced by the helper (same decisions; the dash check was already the canvas rule);
- SVG export (`svg.rs:604`) and Outline stroke (`vectorops.rs:384`): new.

`dashed_rect_center_path` is private again; round 4's re-export now carries the new name (nothing outside the crate used the old one).

### What changed

- SVG export (`svg.rs:601-615` and `:678`): for such a node the stroke element is the helper's outline as `<path>` at weight `w`, still under the node's clip (Inside) or mask (Outside), as the canvas, GPU and PDF keep their clip. Everything else is byte for byte as before. Shape and closedness come from `outline_of`, like the canvas, not from `Geo::is_rect` (which is false whenever `fill_geometry` exists, while the canvas treats a rectangle layer as a rectangle regardless).
- Outline stroke: new `vectorops::outline_node_stroke(doc, n)` (`vectorops.rs:378`). The app's call site (`designos-app/src/vecops.rs:433`) passes the node. `outline_stroke(paths, stroke, network)` itself is unchanged. The wrapper takes the helper's decision (node kind, not geometry); a rectangle-shaped Vector keeps the doubled boundary stroke.
- The Outline stroke result keeps the stroker's exact curves whenever the inset or outset stroke already lies on its side of the shape (checked with `leaves_its_side`, slack 0.2). Only where it crosses the boundary (a corner radius below half the weight with a miter join) does it clip with the polygon overlay and refit curves as every aligned outline stroke does. Reason: the refit is imprecise on round caps (measured up to 0.4 doc px), which showed as 0.3% of pixels differing; with the exact path the straight-corner cases match the canvas to 1 level on every pixel (`max=1`).

### Tests, red on `efe2d977` and green after

New: `svg.rs` (6 tests), `crates/designos-svg/tests/dashed_rectangle_stroke_alignment.rs` (3, resvg vs canvas, the repo's existing SVG raster path), `crates/designos-render/tests/outline_stroke_of_dashed_rectangles.rs` (5), `designos-app` `vecops::tests::outline_stroke_of_a_dashed_inside_rectangle_matches_the_canvas_stroke` (1). `golden_inside_stroke_uses_clip_and_double_width` is now a solid stroke (clip + 2w), so it is also the solid guard; the dashed case is new, with literal path data (`M4 4 L96 4 L96 56 L4 56 L4 4 Z` at weight 8 for a 100 x 60 inside rectangle, `M-4 -4 ... Z` outside).

Null control (`r5-null.log`): all green on `ddf87421`. Red (`r5-red.log`), each consumer on the behaviour of `efe2d977` (SVG: `efe2d977` non-test code of `svg.rs`; Outline stroke: `efe2d977` render crate plus a wrapper that delegates to `outline_stroke`, the call the app made; app: the `efe2d977` call site):
```text
svg.rs: 4 failed (inside, outside, rounded corners, imported fill geometry); the thin-rectangle, non-rectangle and solid-guard tests stay green
designos-svg raster: Inside svg: its round cap: expected ink at (46.5, 34.0), found [227, 142, 142];  Outside Round: 1692 of 96000 pixels differ
designos-render: 3 failed: Inside Round 3459, Outside Round 4197, Outside None (radius 12) 3458 of 96000 pixels differ; gap probe; tight-corner test
designos-app: the outline covers what the canvas strokes: 3279 pixels differ strongly (left: 3279, right: 0)
```
Pictures: `r5-before-after.png` (canvas | SVG | Outline stroke, inside and outside, before and after; recount: 7406 and 7440 of 216000 pixels differ before for inside, 8967 and 9004 for outside, 0 after on all four).

### Mutants (`r5-mutants.log`, all after the green null control)

- (a) SVG export strokes dashed rectangles on the boundary at 2w again: killed, same 4 `svg.rs` tests and 2 raster tests fail.
- (b) the Outline stroke wrapper decides by geometry (any single closed path counts as a rectangle): killed by `a_rectangle_shaped_vector_keeps_the_doubled_boundary_stroke` alone (the other four stay green).
- (c) extra: the Outline stroke never clips to its side: killed by `outline_of_a_dashed_inside_stroke_is_clipped_at_a_tight_rounded_corner` (ink at (129.7, 30.3), outside the rounded corner).

### Gates on `ddf87421` (`r5-gates.log`)

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | rc=0 |
| `cargo clippy --workspace --all-targets --locked` | rc=0; only the three known `designos-app` lints (`agent_permissions.rs:1176`, `:1194`, `shell/tests/inspector_var_binds.rs:1181`) |
| `cargo test -p designos-render --locked` | 289 passed, 0 failed, 5 ignored |
| `cargo test -p designos-gpu --locked -- --skip large_file_compile_benchmarks` | 105 passed, 0 failed |
| `cargo test -p designos-export --locked` | 347 passed, 0 failed, 5 ignored (includes the PDF fidelity tests that now use the helper) |
| `cargo test -p designos-fig --locked` | 527 passed, 0 failed, 6 ignored |
| `cargo test -p designos-svg --locked` | 24 passed, 0 failed, 1 ignored |
| `cargo test -p designos-app --bin designos --locked` | 4209 passed, 1 failed, 13 ignored; the failure is `vecops::tests::two_deferred_text_commands_both_convert_in_order` ("both texts still waiting for slow fonts", the known slow-font timing test under load); alone it passes twice (1.52 s, 1.51 s) |
| `cargo test --workspace --locked --lib` | 1507 passed, 0 failed, 3 ignored |

### Deviations, residual differences, open concerns

1. Files outside round 4's list were changed with the controller's grants: `designos-app/src/vecops.rs` (call site, import, one test), `designos-render/src/{outline,lib}.rs`, `designos-export/src/pdf.rs` (helper swap), new `designos-svg/tests/dashed_rectangle_stroke_alignment.rs`.
2. With round corners the dashes along the curves sit a fraction of a pixel apart from the canvas: tiny-skia's dash and stroke approximation depends on the stroker scale (the canvas uses the zoom, the outline 1:1; resvg has its own). Outside, no cap, radius 12: 480 of 96000 pixels differ by more than 24 against the canvas at 2x. The tests allow 1% for rounded corners and 0.1% for straight corners; the straight cases measure 0. I tried a finer stroker scale for the new path (4 and 8): it moved the mismatch between cases (Inside Square 74 to 404 and 380 pixels, Outside None 480 to 49 and 242) instead of removing it, so it stays at the existing 1.0.
3. `leaves_its_side` ignores overflow below 0.2 doc px (a corner radius within 0.2 of half the weight). That tip is under 0.05 square px.
4. The Outline stroke wrapper and the SVG exporter compute the node shape once more (`node_shape` / `outline_of`); for a boolean group with an aligned stroke that is a second boolean evaluation. Both are one-off exports or an explicit action, not a frame path.
5. The Outline stroke action still ignores per-side weights and precomputed stroke outlines (unchanged); the helper returns `None` for them, so they take the old path.
6. Red for the Outline stroke wrapper is measured against a stub that delegates to `outline_stroke` because the wrapper did not exist on `efe2d977`; the app test is red against the real `efe2d977` call.
7. Visual evidence is synthetic rectangles only (`r5-before-after.png`); no real file was used.

Status: DONE_WITH_CONCERNS (all rulings applied; the one gate failure is the known timing test, green alone)
