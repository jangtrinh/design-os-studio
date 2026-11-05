# Image Fill Crop, Rotation, Tile Scale, and Import-Export Preservation Report

## 0. What Could NOT Be Verified First

1. **Full-suite `cargo test -p designos-app --bin designos --locked` exit code `0` in a single un-filtered invocation under `8`-thread full CPU load `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:7227-7247)`:**
   - In the full binary run (`4,247 passed; 2 failed; 13 ignored` in `391.51s`, `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:7244)`), `2` known slow-font timing tests in `vecops::tests` timed out waiting for background font arrival under CPU contention:
     1. `vecops::tests::text_to_vector_queues_missing_font_without_blocking_and_respects_failed_and_outline_scope` (`crates/designos-app/src/vecops.rs:1480:9`, `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:7229-7233)`).
     2. `vecops::tests::two_deferred_text_commands_both_convert_in_order` (`crates/designos-app/src/vecops.rs:1921:9`, `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:7234-7238)`).
   - Both passed when rerun in `vecops::tests` (`34 passed; 0 failed` in `1.53s`, `exit_code=0`, `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:8861-8908)`).
2. **Follow-up items explicitly outside this scope `(crates/designos-doc/src/paint.rs:206-230)`:**
   - Rendering the `10` `ImageAdjust` color-adjustment filters on CPU and GPU, the image picker panel controls (`exposure`, `contrast`, `saturation`, `temperature`, `tint`, `highlights`, `shadows`, `rotation`, `tile_scale`), and on-canvas crop handles are deferred to the next follow-up work and were not rendered here.
3. **Non-macOS CI targets (`ubuntu` / `windows`):**
   - Verified locally on macOS `aarch64` (`Apple M4 Max`, Metal backend, `(plans/reports/agy-163-image-crop-rotation-tile-evidence/red-tests.log:263)`); `ubuntu` and `windows` CI runs are delegated to GitHub Actions on Draft PR `https://github.com/jangtrinh/design-os-studio/pull/1585`.

---

## 1. Conclusion

`ImageAdjust` (`10` `f32` fields matching `PaintFilterMessage` fields `1..10`, `(crates/designos-doc/src/paint.rs:206-230)`) and `ImageParams` (`transform`, `rotation`, `tile_scale`, `adjust`, `color_manage`, `original_size`, `(crates/designos-doc/src/paint.rs:248-276)`) are now stored on `PaintKind::Image` as `params` with `#[serde(default, skip_serializing_if = "ImageParams::is_default")]` `(crates/designos-doc/src/paint.rs:309-314)`, keeping existing `.designos` files byte-for-byte identical when `params` is default `(crates/designos-doc/src/paint.rs:782-789)`. `.fig` binary import/export `(crates/designos-fig/src/fig.rs:2755-2781, crates/designos-fig/src/fig_export.rs:2134-2198)` and REST JSON import/export `(crates/designos-fig/src/rest_import.rs:86-144, crates/designos-fig/src/export.rs:65-108)` preserve `transform` / `imageTransform`, `rotation`, `scale` / `scalingFactor`, `paintFilter` (and legacy `filterColorAdjust` on import, written back as `paintFilter` on export), `imageShouldColorManage`, and `(originalImageWidth, originalImageHeight)` `(crates/designos-fig/tests/fig_roundtrip.rs:4805-5028)`. Both CPU (`tiny-skia`, `(crates/designos-render/src/paint.rs:320-433)`) and GPU (`wgpu`, `(crates/designos-gpu/src/paintconv.rs:162-246)`) now apply the 2x3 crop `transform` in `ImageScaleMode::Crop` (with transparent pixels outside `[0, 1]^2` and no draw on singular transforms), clockwise `90°` quarter-turn `rotation` about `(0.5, 0.5)` in `Fill`, `Fit`, and `Tile` (swapping image dimensions on odd quarter turns), and `tile_scale` in `Tile`, passing all `15` guard and feature tests across `designos-doc`, `designos-fig`, `designos-render`, and `designos-gpu` `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:538,1352,1369,1381,2046-2068,2266-2278)`.

---

## 2. Step 1 Facts: Re-Anchored Locations, Kit Counts, and Rotation/REST Verification

### 2.1 Re-Anchored File Locations and `PaintKind::Image` Site Count at Base (`1e46e34f2bf93eea3b2374a672a71c7f8097e267`)

| Area | Original Anchor (`0e8a9a5`) | Current Anchor (`1e46e34f` / `fefa8ead`) | Verified Details |
|---|---|---|---|
| `ImageScaleMode` and `PaintKind::Image` | `crates/designos-doc/src/paint.rs:179-212` | `crates/designos-doc/src/paint.rs:185-314` | `ImageScaleMode` enum (`Fill`, `Fit`, `Crop`, `Tile`) at `(crates/designos-doc/src/paint.rs:185-192)`; `ImageAdjust` at `(crates/designos-doc/src/paint.rs:206-230)`; `ImageParams` at `(crates/designos-doc/src/paint.rs:248-276)`; `PaintKind::Image` at `(crates/designos-doc/src/paint.rs:309-314)`. |
| CPU image loading and `with_paint` | `crates/designos-render/src/paint.rs:137-165`, `320-365` | `crates/designos-render/src/paint.rs:137-178`, `320-433` | `load_image` downscale cache at `(crates/designos-render/src/paint.rs:137-178)`; `PaintKind::Image` arm in `with_paint` at `(crates/designos-render/src/paint.rs:320-358)`; `rotate_uv_quarter_turns`, `image_quarter_turns`, and `image_uv_affine` at `(crates/designos-render/src/paint.rs:365-433)`. |
| GPU image instance packing | `crates/designos-gpu/src/paintconv.rs:162-189` | `crates/designos-gpu/src/paintconv.rs:162-246` | `PaintKind::Image` arm in `fill_paint` at `(crates/designos-gpu/src/paintconv.rs:162-176)`; `rotate_uv_quarter_turns`, `image_quarter_turns`, and `image_uv_affine` at `(crates/designos-gpu/src/paintconv.rs:184-246)`. |
| GPU instance layout and shader | `crates/designos-gpu/src/frame.rs:98-121`, `crates/designos-gpu/src/shaders/draw.wgsl:226-242` | `crates/designos-gpu/src/frame.rs:97-121`, `crates/designos-gpu/src/shaders/draw.wgsl:226-242` | `IMG_PAD = 0`, `IMG_FIT = 1`, `IMG_TILE = 2` at `(crates/designos-gpu/src/frame.rs:98-101)`; `Instance` struct (`c1`, `c2`) at `(crates/designos-gpu/src/frame.rs:106-121)`; `fs_image` fragment shader at `(crates/designos-gpu/src/shaders/draw.wgsl:226-242)`. |
| `.fig` import (`convert_paint`) | `crates/designos-fig/src/fig.rs:2510-2519` | `crates/designos-fig/src/fig.rs:2720-2785` | `convert_paint` function at `(crates/designos-fig/src/fig.rs:2720-2785)`; `"IMAGE"` arm at `(crates/designos-fig/src/fig.rs:2755-2781)`. |
| `.fig` export (`encode_paint_full`) | `crates/designos-fig/src/fig_export.rs:1858-1873` | `crates/designos-fig/src/fig_export.rs:2082-2214` | `encode_paint` / `encode_paint_full` at `(crates/designos-fig/src/fig_export.rs:2082-2214)`; `PaintKind::Image` arm at `(crates/designos-fig/src/fig_export.rs:2134-2198)`. |
| REST JSON import (`parse_paint`) | `crates/designos-fig/src/rest_import.rs:86-94` | `crates/designos-fig/src/rest_import.rs:86-153` | `parse_image_params` at `(crates/designos-fig/src/rest_import.rs:86-125)`; `parse_paint` `"IMAGE"` arm at `(crates/designos-fig/src/rest_import.rs:135-144)`. |
| REST JSON export (`paint`) | `crates/designos-fig/src/export.rs:65-78` | `crates/designos-fig/src/export.rs:49-115` | `paint` function at `(crates/designos-fig/src/export.rs:49-115)`; `PaintKind::Image` arm at `(crates/designos-fig/src/export.rs:65-108)`. |
| Picker panel and UI gate | `crates/designos-app/src/widgets/picker.rs:830-895`, `scripts/zero_fake_ui_gate.py:773-790` | `crates/designos-app/src/widgets/picker.rs:1093-1099`, `1122-1184`, `scripts/zero_fake_ui_gate.py:773-790` | `Mode::Image` tab handler at `(crates/designos-app/src/widgets/picker.rs:1093-1099)` and `mode == Mode::Image` body at `(crates/designos-app/src/widgets/picker.rs:1122-1184)`; `check_z12` in `(scripts/zero_fake_ui_gate.py:773-790)`. |
| Kiwi schema definitions | `crates/designos-fig/src/figma_schema.txt:176-180`, `815-882` | `crates/designos-fig/src/figma_schema.txt:176-180`, `815-882` | `Enum ImageScaleMode` at `(crates/designos-fig/src/figma_schema.txt:176-180)`, `Struct FilterColorAdjust` at `(crates/designos-fig/src/figma_schema.txt:815-823)`, `Message PaintFilterMessage` at `(crates/designos-fig/src/figma_schema.txt:824-834)`, `Message Paint` at `(crates/designos-fig/src/figma_schema.txt:835-882)`. |

- **Total `PaintKind::Image` occurrences across `crates/` at base**: `117` lines across `57` files (`108` code sites and `9` comment/string-literal sites), of which `77` mention `image_ref` and `56` code sites across `39` files required explicit `params` or `..` in Commit 1 (`8f8f734b`, `(crates/designos-doc/src/paint.rs:309-314)`).

### 2.2 Image Paint Counts on APFS Clone of Untitled UI Kit (`out/kit-clone.fig`)

All counts below were measured directly on `out/kit-clone.fig` (`119,748` `nodeChanges` records) and recorded in `(plans/reports/agy-163-image-crop-rotation-tile-evidence/kit-image-paints.log:1-14)`:

| Metric | Count | Citation |
|---|---:|---|
| Total `nodeChanges` records | `119,748` | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/kit-image-paints.log:1)` |
| Total `Paint` records with `type == "IMAGE"` (`fillPaints`) | `1,902` | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/kit-image-paints.log:2-3)` |
| `imageScaleMode == "FILL"` | `1,867` | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/kit-image-paints.log:4)` |
| `imageScaleMode == "FIT"` | `0` | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/kit-image-paints.log:4)` |
| `imageScaleMode == "STRETCH"` (Crop) | `23` | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/kit-image-paints.log:4)` |
| `imageScaleMode == "TILE"` | `12` | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/kit-image-paints.log:4)` |
| `imageScaleMode` unset | `0` | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/kit-image-paints.log:4)` |
| Non-identity `transform` (`1,902` carry `transform`) | `119` (`99` in `FILL`, `18` in `STRETCH`, `2` in `TILE`) | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/kit-image-paints.log:5-6)` |
| Non-zero `rotation` (`1,902` carry `rotation`) | `10` (`1` at `90°`, `1` at `450°`, `8` at `360°`) | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/kit-image-paints.log:7,18-21,43-44,55-58)` |
| `scale != 1.0` across all `IMAGE` paints / within `TILE` paints | `1,899` total (`4` at `0.25`, `4` at `0.40`, `1,891` at `0.50`, `3` at `1.00`) / `9` within `TILE` (`4` at `0.25`, `4` at `0.40`, `1` at `0.50`) | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/kit-image-paints.log:8-9,88-102)` |
| `paintFilter` present (and non-zero) | `3` (`1277:186` with `vibrance: -1.0`; `1369:8096` and `1369:8211` with `exposure: -0.46769068`) | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/kit-image-paints.log:10,66,80-81)` |
| `filterColorAdjust` present | `0` | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/kit-image-paints.log:11)` |
| `imageShouldColorManage` present / `originalImageWidth` & `Height` present | `1,902` / `1,902` | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/kit-image-paints.log:12-13)` |

Five kit nodes selected for headless CPU and GPU before/after verification:
1. `1266:28497` (`FRAME` `"Section"`, `720x960` on page `134:3146` `"      ↳ Log in and sign up pages"`): `FILL`, `rotation = 90.0`, image `cbc660a11bbd27b8a000b61df9f4de05f617a13d` (`1440x960`) `(plans/reports/agy-163-image-crop-rotation-tile-evidence/kit-image-paints.log:18,63)`.
2. `1280:220` (`FRAME` `"Section"`, `720x960` on page `134:3146` `"      ↳ Log in and sign up pages"`): `FILL`, `rotation = 450.0` (`450 mod 360 = 90`), image `e6760cdca970932a116753a41bdebc4df91d983b` (`1440x960`) `(plans/reports/agy-163-image-crop-rotation-tile-evidence/kit-image-paints.log:21,67)`.
3. `1362:95` (`FRAME` `"Image"`, `160x240` on page `645:15070` `"      ↳ CTA sections"`): `STRETCH`, `transform = [0.6557377, 0.0, 0.04098361, 0.0, 0.6557377, 0.23751928]`, image `086675e1194fea8498195cac5336b1fa679e82c2` `(plans/reports/agy-163-image-crop-rotation-tile-evidence/kit-image-paints.log:29,75)`.
4. `1362:92` (`FRAME` `"Image"`, `160x240` on page `645:15070` `"      ↳ CTA sections"`): `STRETCH`, `transform = [0.58726346, 0.0, 0.15415666, 0.0, 0.58726346, 0.10849105]`, image `5429041b86654d042b1c5b566ab1873ad747482a` `(plans/reports/agy-163-image-crop-rotation-tile-evidence/kit-image-paints.log:28,74)`.
5. `12297:615641` (`FRAME` `"Transparency slider"`, `232x12` on page `12206:573373` `"      ↳ Color pickers"`): `TILE`, `scale = 0.25`, image `a5c91063d71a310ecd471de02ba853189a276125` (`32x32`) `(plans/reports/agy-163-image-crop-rotation-tile-evidence/kit-image-paints.log:50,98)`.

### 2.3 Rotation Direction, Origin, and REST Key Verification

1. **Figma plugin/REST typings (`@figma/plugin-typings@1.142.0`)**:
   - Inspected `/Users/jangtrinh/.npm/_npx/0009fb9fb9fb8409/node_modules/@figma/plugin-typings/plugin-api.d.ts`:
     - `interface ImageFilters` `(plugin-api.d.ts:4720-4728)` defines `exposure?: number`, `contrast?: number`, `saturation?: number`, `temperature?: number`, `tint?: number`, `highlights?: number`, `shadows?: number`.
     - `interface ImagePaint` `(plugin-api.d.ts:4807-4839)` defines `type: 'IMAGE'`, `scaleMode: 'FILL' | 'FIT' | 'CROP' | 'TILE'`, `imageHash: string | null`, `imageTransform?: Transform`, `scalingFactor?: number`, `rotation?: number`, `filters?: ImageFilters`, where `type Transform = [[number, number, number], [number, number, number]]` `(plugin-api.d.ts:3940)`.
2. **Figma Desktop cached bundle inspection via `/opt/homebrew/bin/rea inspect-artifact`**:
   - Decompressed two cached Figma Desktop bundles from `/Users/jangtrinh/Library/Application Support/Figma/DesktopProfile/v43/Cache/Cache_Data`:
     - `figma_app-9b837f4ccec188a9.min.js` (`8,396,757` bytes, sha256 `48b375393b89dd446cd0440b56c777cc12af668ae3e8811f321b3f18291c5a23`, REA evidence `ev_9d687e8b9a3b7611e37f0f1ba78d8543b9ca8546b076152b2832d1d4067a1894`):
       - `onRotateClockwiseClick` (`"image-rotate-clockwise"`, byte offset `3681627`) calls `(0, K.Lt)(this.props.paint, 90)`.
       - `function H(e, t)` (`K.Lt`, byte offset `2306266`) increments `rotation: n.rotation + t` for `FILL` / `FIT` / `TILE` and pre-multiplies `translate(0.5, 0.5) * rotate(-t * pi / 180) * translate(-0.5, -0.5)` onto `n.transform` for `STRETCH`.
       - `function c(e)` (byte offset `3667257`) rotates a `STRETCH` `transform` by `90°` clockwise via `{m00: t.m10, m01: t.m11, m02: t.m12, m10: -t.m00, m11: -t.m01, m12: 1 - t.m02}`.
     - `viewer-f24e2d24df683ef5.min.js` (`2,054,120` bytes, sha256 `d32ac3d9fa7346176f1669c12b28a35c8bb732647f07317e997f762fc83f91e6`, REA evidence `ev_ef93ea7471381a8d03a050c59d780f59ae09fe9ee0adbc74991ef4a6474435ac`):
       - `allocateBoundsToImageRatio(e, t)` (byte offset `461487`) rotates the image dimensions by `_rotationDeg` and takes `abs()`, swapping `(iw, ih)` on odd `90°` turns (`90°`, `270°`) before computing `nodeSize / rotatedImageSize`.
       - `allocateImageTransformForGeometrySize(e)` (byte offset `462336`, with `t = this._rotationDeg * (Math.PI / 180)`):
         - `FILL`: `translatedBy2(0.5, 0.5).rotatedBy(-t).scaledBy3(n.x()/n.maxComponent(), n.y()/n.maxComponent()).translatedBy2(-0.5, -0.5)`
         - `FIT`: `translatedBy2(0.5, 0.5).rotatedBy(-t).scaledBy3(n.x()/n.minComponent(), n.y()/n.minComponent()).translatedBy2(-0.5, -0.5)`
         - `STRETCH`: `copyFrom(this.transform())`
         - `TILE`: `translatedBy2(0.5, 0.5).rotatedBy(-t).translatedBy2(-0.5, -0.5).scaledBy2(1 / this.scalingFactor()).scaledBy1(n)`
       - `imagePaintViewer` (byte offset `722373`) sets `SHADER_NAME_U_EMULATE_WRAP_ADDRESS = 1` only for `TILE` and `SHADER_NAME_U_EMULATE_BORDER_ADDRESS = 1` for `FILL`, `FIT`, and `STRETCH`, making UV outside `[0, 1]` transparent in `STRETCH` (Crop) as well as `FIT`.
   - **Verified convention**: Positive `rotation` (`90°`, `180°`, `270°`, mod `360°`) rotates the displayed image **clockwise on screen (`y` down)** about the centre `(0.5, 0.5)` of the fitted image rectangle for `Fill` and `Fit`, and about each tile's centre `(0.5, 0.5)` in tile UV for `Tile` `(crates/designos-render/src/paint.rs:365-383, crates/designos-gpu/src/paintconv.rs:184-202)`.

---

## 3. Skills Loaded and Rulings Made

| Skill | Path (`file:line`) | Changes Driven by Skill |
|---|---|---|
| `es:dev` / `es:feature-dev` | `/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md:1-39`, `/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md:1-120` | Read `.brv/context-tree/session-handoffs/latest.md:1-106`, `.brv/context-tree/architecture/crate-map.md:1-120`, and `.brv/context-tree/architecture/figma-vs-designos-engine.md:1-120` before editing; split the work into the `3` scoped commits (`8f8f734b`, `21c7df1c`, `fefa8ead`) and kept `fig.rs`, `fig_export.rs`, `rest_import.rs`, and `export.rs` strictly confined to the image paint functions. |
| `es:rust` | `/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md:1-120` | Made `ImageAdjust` and `ImageParams` `Copy` (`40` and `72` bytes on stack, zero heap allocations, `(crates/designos-doc/src/paint.rs:206,248)`) with epsilon-bounded `is_default(&self)` (`1e-6`, `(crates/designos-doc/src/paint.rs:233,279)`) and `#[serde(default, skip_serializing_if = "ImageParams::is_default")]` `(crates/designos-doc/src/paint.rs:313)`. |
| `es:rust-gpu-canvas` | `/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md:1-120` | Packed the full 2x3 node-space-to-UV affine directly into existing `Instance.c1 = [u.0, v.0, u.1, v.1]` and `Instance.c2 = [u.2, v.2, 0.0, 0.0]` `(crates/designos-gpu/src/paintconv.rs:172-173)` and mapped `ImageScaleMode::Crop` to `IMG_FIT` in `inst.q[2]` `(crates/designos-gpu/src/paintconv.rs:167-170)`, requiring zero WGSL shader edits, zero extra textures, and zero changes to `interactive()` guards in `compile.rs`. |
| `es:astra-work` | `/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md:1-130` | Captured red tests before implementation `(plans/reports/agy-163-image-crop-rotation-tile-evidence/red-tests.log:1-404)`, killed all `3` required mutants at runtime `(plans/reports/agy-163-image-crop-rotation-tile-evidence/mutant-1.log:1-62, mutant-2.log:1-43, mutant-3.log:1-44)`, and ran all `10` mandatory gates on `fefa8ead` `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:1-8908)`. |
| `es:fable-thinking` | `/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md:1-140` | Falsified the hypothesis that all `STRETCH` (Crop) images on CPU need the `1 px` transparent border pad by checking whether any of the `4` node corners maps outside `[0, 1]^2` (`crop_leaves_unit_uv`, `(crates/designos-render/src/paint.rs:341-355)`), eliminating border-pad edge bleed on identity-stretched and zoomed-in crops while keeping out-of-bounds UV transparent. |
| `reverse-engineer-anything` | `/Users/jangtrinh/.gemini/config/skills/reverse-engineer-anything/SKILL.md:1-120` | Executed `/opt/homebrew/bin/rea inspect-artifact` on cached Figma Desktop bundles `figma_app-9b837f4ccec188a9.min.js` and `viewer-f24e2d24df683ef5.min.js` (`Section 2.3`), resolving the clockwise rotation direction, `(0.5, 0.5)` pivot, odd-turn dimension swap, and `STRETCH` transparent border clamp as `FACT`. |

---

## 4. Acceptance Summary Table

| # | Acceptance Check | Red State (Base) | Green State (`fefa8ead`) | Evidence Citation |
|---|---|---|---|---|
| 1a | Guard: `old_designos_file_loads_with_default_params_and_saves_unchanged` `(crates/designos-doc/src/paint.rs:783-789)` | `ok` (`exit_code=0`) | `ok` (`exit_code=0`) | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/red-tests.log:1-13)`, `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:538)` |
| 1b | Guard: `image_fill_modes` `(crates/designos-render/tests/render.rs:510-535)` | `ok` (`exit_code=0`) | `ok` (`exit_code=0`) | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/red-tests.log:14-28)`, `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:2050)` |
| 1c | `fig_round_trip_keeps_transform_rotation_scale_filters_and_flags` `(crates/designos-fig/tests/fig_roundtrip.rs:4805-4862)` | `FAILED` (`exit_code=101`) | `ok` (`exit_code=0`) | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/red-tests.log:29-55)`, `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:1352)` |
| 1d | `legacy_filter_color_adjust_reads_into_the_adjustment_set` `(crates/designos-fig/tests/fig_roundtrip.rs:4865-4935)` | `FAILED` (`exit_code=101`) | `ok` (`exit_code=0`) | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/red-tests.log:56-83)`, `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:1381)` |
| 1e | `rest_round_trip_keeps_image_transform_scaling_factor_and_filters` `(crates/designos-fig/tests/fig_roundtrip.rs:4938-5028)` | `FAILED` (`exit_code=101`) | `ok` (`exit_code=0`) | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/red-tests.log:84-111)`, `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:1369)` |
| 1f | `rotation_90_is_exact_on_a_2x2_image` (CPU & GPU, `0.0000%` mismatch) `(crates/designos-render/tests/render.rs:851-873, crates/designos-gpu/tests/fidelity.rs:737-766)` | `FAILED` (`exit_code=101`) | `ok` (`exit_code=0`) | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/red-tests.log:112-139,248-280)`, `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:2062,2270)` |
| 1g | `tile_scale_2_doubles_the_tile_size` (CPU & GPU, `0.0000%` mismatch) `(crates/designos-render/tests/render.rs:876-888, crates/designos-gpu/tests/fidelity.rs:769-785)` | `FAILED` (`exit_code=101`) | `ok` (`exit_code=0`) | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/red-tests.log:140-165,281-311)`, `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:2068,2278)` |
| 1h | `crop_with_identity_transform_stretches_the_image` (CPU & GPU, `0.0000%` mismatch) `(crates/designos-render/tests/render.rs:891-910, crates/designos-gpu/tests/fidelity.rs:788-815)` | `FAILED` (`exit_code=101`) | `ok` (`exit_code=0`) | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/red-tests.log:166-191,312-342)`, `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:2049,2266)` |
| 1i | `crop_transform_maps_uv_through_the_affine` (CPU & GPU, `0.0000%` mismatch) `(crates/designos-render/tests/render.rs:913-927, crates/designos-gpu/tests/fidelity.rs:818-840)` | `FAILED` (`exit_code=101`) | `ok` (`exit_code=0`) | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/red-tests.log:192-219,343-373)`, `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:2046,2267)` |
| 1j | `singular_crop_transform_draws_nothing` (CPU & GPU, `0.0000%` mismatch) `(crates/designos-render/tests/render.rs:930-941, crates/designos-gpu/tests/fidelity.rs:843-862)` | `FAILED` (`exit_code=101`) | `ok` (`exit_code=0`) | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/red-tests.log:220-247,374-404)`, `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:2063,2272)` |
| 2a | Null control on `fefa8ead` before mutants | — | `0` failed (`exit_code=0`) | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:1394,2076,2294)` |
| 2b | Mutant 1: `ImageScaleMode::Crop` routed to `Fill` in `(crates/designos-render/src/paint.rs:323)` | — | Killed by `3` crop tests (`exit_code=101`) | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/mutant-1.log:27-62)` |
| 2c | Mutant 2: `tile_scale` hardcoded to `1.0` in `(crates/designos-render/src/paint.rs:405)` | — | Killed by `tile_scale_2_doubles_the_tile_size` (`exit_code=101`) | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/mutant-2.log:26-43)` |
| 2d | Mutant 3: `.fig` import drops `transform` in `(crates/designos-fig/src/fig.rs:2764-2766)` | — | Killed by `fig_round_trip_keeps_transform_rotation_scale_filters_and_flags` (`exit_code=101`) | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/mutant-3.log:25-44)` |
| 3 | Kit evidence: `20` headless CPU/GPU before/after PNG crops on `1266:28497` (`98.32%` change, SSIM `0.9939`), `1280:220` (`95.81%` change, SSIM `0.9965`), `1362:95` (`99.51%` change, SSIM `0.9890`), `1362:92` (`99.66%` change, SSIM `0.9844`), `12297:615641` (`49.68%` change, SSIM `0.9751`) | Base renders | Head renders | `plans/reports/agy-163-image-crop-rotation-tile-evidence/kit-*-{before,after}-{cpu,gpu}.png`, `(plans/reports/agy-163-image-crop-rotation-tile-evidence/kit-image-paints.log:1-107)` |
| 4a | `cargo fmt --all -- --check` | — | `exit_code=0` | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:1-2)` |
| 4b | `cargo clippy --workspace --all-targets --locked` | — | `exit_code=0` (`0` warnings in modified crates) | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:3-100)` |
| 4c | `cargo test -p designos-doc --locked` | — | `exit_code=0` (`746 + 3 + 5 + 2` passed, `0` failed) | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:101-893)` |
| 4d | `cargo test -p designos-fig --locked` | — | `exit_code=0` (`143` lib + `52` `fig_roundtrip` + all integration tests passed, `0` failed) | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:894-1671)` |
| 4e | `cargo test -p designos-render --locked` | — | `exit_code=0` (`327` lib + `39` `render` + all integration tests passed, `0` failed) | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:1672-2223)` |
| 4f | `cargo test -p designos-gpu --locked -- --skip large_file_compile_benchmarks` | — | `exit_code=0` (`5` lib + `28` `fidelity` + all integration suites passed; `large_file_perf` `0 passed; 1 filtered out`) | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:2224-2440)` |
| 4g | `cargo test -p designos-export --locked` | — | `exit_code=0` (`164` lib + all integration tests passed, `0` failed) | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:2441-2912)` |
| 4h | `cargo test -p designos-app --locked --bin designos` + isolated `vecops::tests` rerun | — | `4,247 passed; 2 failed` under full load + `34 passed; 0 failed` (`exit_code=0`) in `vecops::tests` rerun | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:2940-7247,8861-8908)` |
| 4i | `cargo test --workspace --lib --locked` | — | `exit_code=0` (`1,531` lib tests passed across all `11` crates, `0` failed) | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:7248-8860)` |
| 4j | `python3 -B scripts/zero_fake_ui_gate.py` | — | `PASS (17/17 invariants satisfied)` (`exit_code=0`) | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:2913-2939)` |
| 5 | Draft PR open (`#1585`), `git log --oneline origin/main..HEAD`, and `out/cargo` & `out/kit-clone.fig` deleted (`ls out/cargo` failing) | — | `https://github.com/jangtrinh/design-os-studio/pull/1585`, `ls_out_cargo_rc=1` | `(plans/reports/agy-163-image-crop-rotation-tile-evidence/ls-out-cargo.log:1-2)` |

---

## 5. Pasted Command Outputs

### 5.1 Guard and Red Tests on Base `(plans/reports/agy-163-image-crop-rotation-tile-evidence/red-tests.log:1-404)`

```text
=== cargo test -p designos-doc --locked --lib -- old_designos_file_loads_with_default_params_and_saves_unchanged ===
running 1 test
test paint::tests::old_designos_file_loads_with_default_params_and_saves_unchanged ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 745 filtered out; finished in 0.01s
exit_code=0

=== cargo test -p designos-render --locked --test render -- image_fill_modes ===
running 1 test
test image_fill_modes ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 38 filtered out; finished in 0.00s
exit_code=0

=== cargo test -p designos-fig --locked --test fig_roundtrip -- fig_round_trip_keeps_transform_rotation_scale_filters_and_flags ===
running 1 test
test fig_round_trip_keeps_transform_rotation_scale_filters_and_flags ... FAILED
thread 'fig_round_trip_keeps_transform_rotation_scale_filters_and_flags' (87411070) panicked at crates/designos-fig/tests/fig_roundtrip.rs:4851:5:
export writes paintFilter
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 51 filtered out; finished in 0.03s
exit_code=101

=== cargo test -p designos-fig --locked --test fig_roundtrip -- legacy_filter_color_adjust_reads_into_the_adjustment_set ===
running 1 test
test legacy_filter_color_adjust_reads_into_the_adjustment_set ... FAILED
thread 'legacy_filter_color_adjust_reads_into_the_adjustment_set' (87411089) panicked at crates/designos-fig/tests/fig_roundtrip.rs:4912:5:
assertion `left == right` failed
  left: ImageAdjust { tint: 0.0, shadows: 0.0, highlights: 0.0, detail: 0.0, exposure: 0.0, vignette: 0.0, temperature: 0.0, vibrance: 0.0, contrast: 0.0, brightness: 0.0 }
 right: ImageAdjust { tint: 0.25, shadows: -0.5, highlights: 0.75, detail: 0.125, exposure: -0.375, vignette: 0.625, temperature: -0.25, vibrance: 0.5, contrast: 0.0, brightness: 0.0 }
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 51 filtered out; finished in 0.01s
exit_code=101

=== cargo test -p designos-fig --locked --test fig_roundtrip -- rest_round_trip_keeps_image_transform_scaling_factor_and_filters ===
running 1 test
test rest_round_trip_keeps_image_transform_scaling_factor_and_filters ... FAILED
thread 'rest_round_trip_keeps_image_transform_scaling_factor_and_filters' (87411107) panicked at crates/designos-fig/tests/fig_roundtrip.rs:4998:5:
assertion `left == right` failed
  left: Null
 right: Array [Array [Number(0.5), Number(0.0), Number(0.25)], Array [Number(0.0), Number(0.75), Number(0.125)]]
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 51 filtered out; finished in 0.00s
exit_code=101

=== cargo test -p designos-render --locked --test render -- rotation_90_is_exact_on_a_2x2_image ===
running 1 test
test rotation_90_is_exact_on_a_2x2_image ... FAILED
thread 'rotation_90_is_exact_on_a_2x2_image' (87411129) panicked at crates/designos-render/tests/render.rs:872:5:
assertion `left == right` failed
  left: [255, 0, 0, 255]
 right: [0, 0, 255, 255]
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 38 filtered out; finished in 0.00s
exit_code=101

=== cargo test -p designos-render --locked --test render -- tile_scale_2_doubles_the_tile_size ===
running 1 test
test tile_scale_2_doubles_the_tile_size ... FAILED
thread 'tile_scale_2_doubles_the_tile_size' (87411148) panicked at crates/designos-render/tests/render.rs:895:5:
x=15 is still inside the doubled red half: [0, 0, 255, 255]
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 38 filtered out; finished in 0.00s
exit_code=101

=== cargo test -p designos-render --locked --test render -- crop_with_identity_transform_stretches_the_image ===
running 1 test
test crop_with_identity_transform_stretches_the_image ... FAILED
thread 'crop_with_identity_transform_stretches_the_image' (87411169) panicked at crates/designos-render/tests/render.rs:921:5:
left edge of stretched crop shows red band: [0, 255, 0, 255]
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 38 filtered out; finished in 0.00s
exit_code=101

=== cargo test -p designos-render --locked --test render -- crop_transform_maps_uv_through_the_affine ===
running 1 test
test crop_transform_maps_uv_through_the_affine ... FAILED
thread 'crop_transform_maps_uv_through_the_affine' (87411188) panicked at crates/designos-render/tests/render.rs:942:5:
assertion `left == right` failed: UV < 0 outside cropped image is transparent over white page
  left: [255, 0, 0, 255]
 right: [255, 255, 255, 255]
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 38 filtered out; finished in 0.00s
exit_code=101

=== cargo test -p designos-render --locked --test render -- singular_crop_transform_draws_nothing ===
running 1 test
test singular_crop_transform_draws_nothing ... FAILED
thread 'singular_crop_transform_draws_nothing' (87411208) panicked at crates/designos-render/tests/render.rs:962:5:
assertion `left == right` failed: singular crop transform draws nothing
  left: [255, 0, 0, 255]
 right: [255, 255, 255, 255]
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 38 filtered out; finished in 0.00s
exit_code=101

=== cargo test -p designos-gpu --locked --test fidelity -- rotation_90_is_exact_on_a_2x2_image ===
running 1 test
test rotation_90_is_exact_on_a_2x2_image ... FAILED
GPU: Apple M4 Max (Metal)
rotation_90_2x2: mismatch 0.0000% (0 px of 4) at tolerance 0, ssim 1.0000
thread 'rotation_90_is_exact_on_a_2x2_image' (87411945) panicked at crates/designos-gpu/tests/fidelity.rs:769:5:
assertion `left == right` failed
  left: [255, 0, 0, 255]
 right: [0, 0, 255, 255]
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 28 filtered out; finished in 0.10s
exit_code=101

=== cargo test -p designos-gpu --locked --test fidelity -- tile_scale_2_doubles_the_tile_size ===
running 1 test
test tile_scale_2_doubles_the_tile_size ... FAILED
GPU: Apple M4 Max (Metal)
tile_scale_2: mismatch 0.0000% (0 px of 800) at tolerance 8, ssim 1.0000
thread 'tile_scale_2_doubles_the_tile_size' (87412004) panicked at crates/designos-gpu/tests/fidelity.rs:800:5:
assertion `left == right` failed
  left: [0, 0, 255, 255]
 right: [255, 0, 0, 255]
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 28 filtered out; finished in 0.06s
exit_code=101

=== cargo test -p designos-gpu --locked --test fidelity -- crop_with_identity_transform_stretches_the_image ===
running 1 test
test crop_with_identity_transform_stretches_the_image ... FAILED
GPU: Apple M4 Max (Metal)
crop_identity_stretch: mismatch 0.0000% (0 px of 800) at tolerance 8, ssim 1.0000
thread 'crop_with_identity_transform_stretches_the_image' (87412049) panicked at crates/designos-gpu/tests/fidelity.rs:834:5:
assertion `left == right` failed
  left: [0, 255, 0, 255]
 right: [255, 0, 0, 255]
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 28 filtered out; finished in 0.07s
exit_code=101

=== cargo test -p designos-gpu --locked --test fidelity -- crop_transform_maps_uv_through_the_affine ===
running 1 test
test crop_transform_maps_uv_through_the_affine ... FAILED
GPU: Apple M4 Max (Metal)
crop_affine_uv: mismatch 0.0000% (0 px of 1600) at tolerance 8, ssim 1.0000
thread 'crop_transform_maps_uv_through_the_affine' (87412102) panicked at crates/designos-gpu/tests/fidelity.rs:863:5:
assertion `left == right` failed
  left: [255, 0, 0, 255]
 right: [255, 255, 255, 255]
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 28 filtered out; finished in 0.06s
exit_code=101

=== cargo test -p designos-gpu --locked --test fidelity -- singular_crop_transform_draws_nothing ===
running 1 test
test singular_crop_transform_draws_nothing ... FAILED
GPU: Apple M4 Max (Metal)
singular_crop: mismatch 0.0000% (0 px of 1600) at tolerance 8, ssim 1.0000
thread 'singular_crop_transform_draws_nothing' (87412130) panicked at crates/designos-gpu/tests/fidelity.rs:891:5:
assertion `left == right` failed
  left: [255, 0, 0, 255]
 right: [255, 255, 255, 255]
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 28 filtered out; finished in 0.07s
exit_code=101
```

### 5.2 Three Mutants Killed at Runtime `(plans/reports/agy-163-image-crop-rotation-tile-evidence/mutant-1.log:1-62, mutant-2.log:1-43, mutant-3.log:1-44)`

```text
=== Mutant 1: ImageScaleMode::Crop routed to Fill arm in crates/designos-render/src/paint.rs ===
running 3 tests
test crop_transform_maps_uv_through_the_affine ... FAILED
test singular_crop_transform_draws_nothing ... FAILED
test crop_with_identity_transform_stretches_the_image ... FAILED

thread 'crop_transform_maps_uv_through_the_affine' (87475294) panicked at crates/designos-render/tests/render.rs:923:5:
assertion `left == right` failed: UV < 0 outside cropped image is transparent over white page
  left: [255, 0, 0, 255]
 right: [255, 255, 255, 255]

thread 'singular_crop_transform_draws_nothing' (87475296) panicked at crates/designos-render/tests/render.rs:940:5:
assertion `left == right` failed: singular crop transform draws nothing
  left: [255, 0, 0, 255]
 right: [255, 255, 255, 255]

thread 'crop_with_identity_transform_stretches_the_image' (87475295) panicked at crates/designos-render/tests/render.rs:908:5:
left edge of stretched crop shows red band: [0, 255, 0, 255]

test result: FAILED. 0 passed; 3 failed; 0 ignored; 0 measured; 36 filtered out; finished in 0.01s
exit_code=101

=== Mutant 2: tile_scale hardcoded to 1.0 in crates/designos-render/src/paint.rs ===
running 1 test
test tile_scale_2_doubles_the_tile_size ... FAILED

thread 'tile_scale_2_doubles_the_tile_size' (87476465) panicked at crates/designos-render/tests/render.rs:886:5:
x=7 is inside the doubled red band (0..10): [0, 0, 255, 255]

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 38 filtered out; finished in 0.00s
exit_code=101

=== Mutant 3: transform dropped in convert_paint in crates/designos-fig/src/fig.rs ===
running 1 test
test fig_round_trip_keeps_transform_rotation_scale_filters_and_flags ... FAILED

thread 'fig_round_trip_keeps_transform_rotation_scale_filters_and_flags' (87470679) panicked at crates/designos-fig/tests/fig_roundtrip.rs:4857:5:
assertion `left == right` failed
  left: ImageParams { transform: None, rotation: 90.0, tile_scale: 0.4, adjust: ImageAdjust { tint: 0.1, shadows: -0.2, highlights: 0.3, detail: 0.4, exposure: -0.5, vignette: 0.6, temperature: -0.7, vibrance: 0.8, contrast: -0.9, brightness: 0.25 }, color_manage: Some(true), original_size: Some((1440, 960)) }
 right: ImageParams { transform: Some([0.6, 0.0, 0.15, 0.0, 0.75, 0.1]), rotation: 90.0, tile_scale: 0.4, adjust: ImageAdjust { tint: 0.1, shadows: -0.2, highlights: 0.3, detail: 0.4, exposure: -0.5, vignette: 0.6, temperature: -0.7, vibrance: 0.8, contrast: -0.9, brightness: 0.25 }, color_manage: Some(true), original_size: Some((1440, 960)) }

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 51 filtered out; finished in 0.03s
exit_code=101
```

### 5.3 Mandatory Gates on Final Code Commit `fefa8ead` `(plans/reports/agy-163-image-crop-rotation-tile-evidence/green-gates.log:1-8908)`

```text
=== cargo fmt --all -- --check ===
exit_code=0

=== cargo clippy --workspace --all-targets --locked ===
warning: unneeded `return` statement
    --> crates/designos-app/src/agent_permissions.rs:1176:9
warning: unneeded `return` statement
    --> crates/designos-app/src/agent_permissions.rs:1194:9
warning: very complex type used. Consider factoring parts into `type` definitions
    --> crates/designos-app/src/shell/tests/inspector_var_binds.rs:1182:21
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 47.41s
exit_code=0

=== cargo test -p designos-doc --locked ===
running 746 tests
test paint::tests::old_designos_file_loads_with_default_params_and_saves_unchanged ... ok
test result: ok. 746 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.70s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
exit_code=0

=== cargo test -p designos-fig --locked ===
running 143 tests
test result: ok. 143 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.13s
     Running tests/fig_roundtrip.rs (out/cargo/debug/deps/fig_roundtrip-59464b98b5f1161b)
running 52 tests
test fig_round_trip_keeps_transform_rotation_scale_filters_and_flags ... ok
test rest_round_trip_keeps_image_transform_scaling_factor_and_filters ... ok
test legacy_filter_color_adjust_reads_into_the_adjustment_set ... ok
test result: ok. 52 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.59s
exit_code=0

=== cargo test -p designos-render --locked ===
running 327 tests
test result: ok. 327 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.72s
     Running tests/render.rs (out/cargo/debug/deps/render-5ddf387f7aa22575)
running 39 tests
test crop_transform_maps_uv_through_the_affine ... ok
test crop_with_identity_transform_stretches_the_image ... ok
test image_fill_modes ... ok
test rotation_90_is_exact_on_a_2x2_image ... ok
test singular_crop_transform_draws_nothing ... ok
test tile_scale_2_doubles_the_tile_size ... ok
test result: ok. 39 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
exit_code=0

=== cargo test -p designos-gpu --locked -- --skip large_file_compile_benchmarks ===
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/fidelity.rs (out/cargo/debug/deps/fidelity-e9d67385509b7a81)
running 28 tests
test crop_with_identity_transform_stretches_the_image ... ok
test crop_transform_maps_uv_through_the_affine ... ok
test rotation_90_is_exact_on_a_2x2_image ... ok
test singular_crop_transform_draws_nothing ... ok
test tile_scale_2_doubles_the_tile_size ... ok
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.77s
     Running tests/large_file_perf.rs (out/cargo/debug/deps/large_file_perf-c19c8fdc40311fbf)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s
exit_code=0

=== cargo test -p designos-export --locked ===
running 164 tests
test result: ok. 164 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.53s
exit_code=0

=== python3 -B scripts/zero_fake_ui_gate.py ===
Z12   | PASS   | widgets/picker.rs + panels/mod.rs + variables/view.rs | Mode::Image creates Paint::Image/doc.images & scrollbars support Sense::click_and_drag()
 Gate Verdict: PASS (17/17 invariants satisfied)
exit_code=0

=== cargo test -p designos-app --locked --bin designos -- --test-threads=8 ===
failures:
    vecops::tests::text_to_vector_queues_missing_font_without_blocking_and_respects_failed_and_outline_scope
    vecops::tests::two_deferred_text_commands_both_convert_in_order
test result: FAILED. 4247 passed; 2 failed; 13 ignored; 0 measured; 0 filtered out; finished in 391.51s
exit_code=101

=== cargo test -p designos-app --locked --bin designos -- vecops::tests ===
running 34 tests
test vecops::tests::text_to_vector_queues_missing_font_without_blocking_and_respects_failed_and_outline_scope ... ok
test vecops::tests::two_deferred_text_commands_both_convert_in_order ... ok
test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 4228 filtered out; finished in 1.53s
exit_code=0

=== cargo test --workspace --lib --locked ===
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 746 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.80s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 164 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.51s
test result: ok. 143 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.95s
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.24s
test result: ok. 327 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.68s
test result: ok. 56 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 33.05s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
exit_code=0
```

### 5.4 Draft PR URL, `git log --oneline origin/main..HEAD`, and `ls out/cargo` Failing `(plans/reports/agy-163-image-crop-rotation-tile-evidence/ls-out-cargo.log:1-2)`

- **Draft PR URL**: `https://github.com/jangtrinh/design-os-studio/pull/1585`
- **`git log --oneline origin/main..HEAD`**:

```text
fefa8ead feat(render,gpu): apply crop transform, 90-degree rotation, and tile scale to image fills
21c7df1c feat(fig): round-trip image transform, rotation, tile scale, and adjustments
8f8f734b feat(doc): add ImageParams and ImageAdjust to PaintKind::Image
```

- **`ls out/cargo` after `rm -rf out/cargo out/kit-clone.fig` `(plans/reports/agy-163-image-crop-rotation-tile-evidence/ls-out-cargo.log:1-2)`**:

```text
ls: out/cargo: No such file or directory
ls_out_cargo_rc=1
```

---

## 6. Epistemic Ledger (`FACT` / `INFERENCE` / `ASSUMPTION`), Assumptions & Weakest Link

### 6.1 Epistemic Ledger
- **`FACT`**: Kiwi `Paint` carries `transform` (field `7`, `Matrix`), `imageScaleMode` (field `10`, `STRETCH = 0`, `FIT = 1`, `FILL = 2`, `TILE = 3`), `rotation` (field `11`, `float`), `scale` (field `12`, `float`), legacy `filterColorAdjust` (field `13`, `FilterColorAdjust`), `paintFilter` (field `14`, `PaintFilterMessage`), `originalImageWidth` (field `19`, `uint`), `originalImageHeight` (field `20`, `uint`), and `imageShouldColorManage` (field `22`, `bool`) `(crates/designos-fig/src/figma_schema.txt:176-180,815-882)`.
- **`FACT`**: In Figma Desktop (`figma_app-9b837f4ccec188a9.min.js` byte offset `2306266`, `viewer-f24e2d24df683ef5.min.js` byte offsets `461487`, `462336`, `722373`), positive `rotation` (`90°`, `180°`, `270°` mod `360°`) rotates the image clockwise on screen (`y` down) about `(0.5, 0.5)` of the fitted rectangle for `Fill` and `Fit` (swapping image dimensions on odd `90°` turns) and about `(0.5, 0.5)` of each tile for `Tile`, while `STRETCH` (Crop) uses `transform` directly and sets `SHADER_NAME_U_EMULATE_BORDER_ADDRESS = 1` so UV outside `[0, 1]^2` is transparent `(crates/designos-render/src/paint.rs:365-433, crates/designos-gpu/src/paintconv.rs:184-246)`.
- **`FACT`**: All `1,902` `IMAGE` paints in `out/kit-clone.fig` carry `scale` (`1,891` with `0.5`) and `transform` (`119` non-identity across `99` `FILL`, `18` `STRETCH`, `2` `TILE`); only `ImageScaleMode::Tile` applies `params.tile_scale` and only `ImageScaleMode::Crop` applies `params.transform` during rendering, while `.fig` and REST import/export preserve all fields on `ImageParams` regardless of `scale_mode` `(plans/reports/agy-163-image-crop-rotation-tile-evidence/kit-image-paints.log:5-9, crates/designos-render/src/paint.rs:391-432)`.
- **`INFERENCE`**: When `ImageScaleMode::Crop` is used with `params.transform == None`, `image_uv_affine` defaults the 2x3 transform to identity `[1.0, 0.0, 0.0, 0.0, 1.0, 0.0]` (stretching the image to the node box) and treats UV outside `[0, 1]^2` as transparent `(crates/designos-render/src/paint.rs:411-432, crates/designos-gpu/src/paintconv.rs:230-245)` — confirmed against `viewer-f24e2d24df683ef5.min.js` byte offsets `462336` and `722373`.
- **`ASSUMPTION`**: None remaining (`0` unverified assumptions in this scope).

### 6.2 Weakest Link
- On CPU `(crates/designos-render/src/paint.rs:341-355)`, `ImageScaleMode::Crop` enables the `1 px` transparent border pad when any of the `4` node corners maps outside `[0, 1]^2` (`crop_leaves_unit_uv`), which relies on `tiny-skia`'s `SpreadMode::Pad` clamping to that `1 px` transparent border rather than hardware border-color sampling.

---

## 7. Unresolved Questions

None for this scope; the exact `PaintFilterMessage` shader constants (`T` threshold default, negative-exposure re-saturation correction, and positive-saturation `q, r` clamp constants) and UI contrast slider scale remain deferred to the follow-up adjustments and picker panel work `(crates/designos-doc/src/paint.rs:206-230)`.

Status: DONE
