# Completion Report — `#163` Slice B: Image Fill Adjustments Pipeline & Picker Controls

**Worktree:** `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-163b-image-adjustments-panel` (`AGENTS.md:1`)
**Branch:** `jangtrinh/agy-163b-image-adjustments-panel` (base `origin/main` at `8087735f4`, `plans/reports/agy-163b-image-adjustments-panel-evidence/red-tests.log:1`)
**Spec:** `docs/specs/v0.7/163-image-fill-adjustments-rotation-tile-scale-and-crop.md` (`docs/specs/v0.7/163-image-fill-adjustments-rotation-tile-scale-and-crop.md:1-147`)
**Evidence Directory:** `plans/reports/agy-163b-image-adjustments-panel-evidence/` (`plans/reports/agy-163b-image-adjustments-panel-evidence/gates.log:1`)
**Draft PR:** `https://github.com/jangtrinh/design-os-studio/pull/1595` (`plans/reports/agy-163b-image-adjustments-panel.md:7`)
**Branch Commits (`git log --oneline origin/main..HEAD`):** `f1e567c5f feat(images): exposure, contrast, saturation, temperature, tint, highlights and shadows on image fills, with picker controls (#163)` (`plans/reports/agy-163b-image-adjustments-panel.md:8`)

---

## 1. What Could NOT Be Verified

1. **Live Figma Desktop readback from Spec Section 10 (`docs/specs/v0.7/163-image-fill-adjustments-rotation-tile-scale-and-crop.md:113`):** The live Figma Desktop readback of a `#808080` rectangle with `Exposure = +100%` is an owner-manual check (`docs/specs/v0.7/163-image-fill-adjustments-rotation-tile-scale-and-crop.md:113`) because agents are forbidden from launching or automating the Figma desktop app (`CLAUDE.md:57`). Instead, we verified the exact 6-step shader extracted from Figma's `compiled_wasm.wasm` (`plans/reports/agy-163b-image-adjustments-panel-evidence/kit-paint-filters.log:2-24`), which maps `#808080` (`[128, 128, 128, 255]`) at `exposure = +1.0` to `[251, 251, 251, 255]` (`#FBFBFB`, within `±2/255` on both CPU and GPU, `crates/designos-gpu/tests/fidelity.rs:1022-1042`, `plans/reports/agy-163b-image-adjustments-panel-evidence/kit-render-head.log:9-12`).
2. **Live interactive macOS desktop window capture (`--open-id` / OS window screenshot):** Per the window capture rule, `ioreg -c IOHIDSystem` reported `HIDIdleTime = 23 s` (`< 300 s` threshold, `plans/reports/agy-163b-image-adjustments-panel-evidence/picker-window-capture-idle-check.log:1-3`), so spawning an interactive desktop window was marked `SKIPPED (owner active)` (`plans/reports/agy-163b-image-adjustments-panel-evidence/picker-window-capture-idle-check.log:3`). Headless offscreen `egui` + `tiny-skia` renders of the open `Mode::Image` picker were captured instead at `picker-image-adjustments-open.png` (`240x528`, `plans/reports/agy-163b-image-adjustments-panel-evidence/picker-window-capture-idle-check.log:4`) and `picker-image-adjustments-reset.png` (`240x528`, `plans/reports/agy-163b-image-adjustments-panel-evidence/picker-window-capture-idle-check.log:4`).
3. **Out-of-scope items deferred to Slice A (`docs/specs/v0.7/163-image-fill-adjustments-rotation-tile-scale-and-crop.md:126-135`):** Interactive canvas crop handles (`canvas/input.rs`, `canvas/overlays.rs`), `detail` / `vignette` / `brightness` rendering, and SVG/PDF export baking of adjusted images are not in this PR (`docs/specs/v0.7/163-image-fill-adjustments-rotation-tile-scale-and-crop.md:57,129`).

---

## 2. Skills Loaded & Behavioral Adjustments

| Skill (`SKILL.md:line`) | Key Rule Applied | Concrete Code/Verification Change Made |
|---|---|---|
| `/es:dev` -> `es:feature-dev` (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md:1-22`, `/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md:1-60`) | Classify -> Re-anchor facts -> Red tests -> Implement -> Adversarial mutants -> Gates | Executed Step 1 facts re-anchoring (`plans/reports/agy-163b-image-adjustments-panel-evidence/kit-paint-filters.log:1-32`) and recorded Red failures on base `8087735f4` (`plans/reports/agy-163b-image-adjustments-panel-evidence/red-tests.log:1-157`) before touching renderer or picker code. |
| `es:fable-thinking` (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md:1-95`) | Epistemic ledger (`FACT`/`INFERENCE`/`ASSUMPTION`), falsification via mutants, prediction before test | Resolved all 3 numeric holes in `docs/specs/v0.7/163-image-fill-adjustments-rotation-tile-scale-and-crop.md:45-55` from `compiled_wasm.wasm` (`plans/reports/agy-163b-image-adjustments-panel-evidence/kit-paint-filters.log:16-19`) and ran 3 targeted mutants with null control (`plans/reports/agy-163b-image-adjustments-panel-evidence/mutants.log:1-300`). |
| `es:rust` (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md:1-80`) | Zero unnecessary clones on hot paths; leave decoded image cache untouched | In `crates/designos-render/src/paint.rs:349-364`, `load_image` (`crates/designos-render/src/paint.rs:350`) remains unmutated; `apply_pixmap_adjustments` (`crates/designos-render/src/image_adjust.rs:194-219`) allocates an adjusted `Pixmap` only when `has_rendered_adjustments(&params.adjust)` (`crates/designos-render/src/image_adjust.rs:35-43`) is true. |
| `es:rust-gpu-canvas` (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md:1-90`) | Pack parameters into existing `Instance` slots (`64 B` stride); zero new render passes or offscreen `Program`s; preserve `FrameKey` reuse | Packed all 7 adjustment floats + active flag `q[1] = 1` into unused `Instance` fields (`c0[0..4]`, `c2[2..4]`, `t[3]`, `crates/designos-gpu/src/paintconv.rs:175-182`) with `progs=1` and `idle_stale_hits=0` (`plans/reports/agy-163b-image-adjustments-panel-evidence/perf.log:237`). |
| `es-designer` (`/Users/jangtrinh/.gemini/config/skills/es-designer/SKILL.md:1-75`) | Use semantic `theme.rs` tokens (`theme::BADGE_LG = 28.0`, `theme::SP_6`, `theme::SP_8`, `theme::TEXT_DIM`, `theme::ACCENT`) and concentric radii | Built `Mode::Image` rows in `crates/designos-app/src/widgets/picker.rs:831-860,1091-1301` using `ADJ_ROW_H = theme::BADGE_LG` (`28.0 px`, `crates/designos-app/src/widgets/picker.rs:833`), passing `zero_fake_ui_gate.py` (`17/17`, `plans/reports/agy-163b-image-adjustments-panel-evidence/gates.log:25`) and `check_spacing_relationships.py` (`plans/reports/agy-163b-image-adjustments-panel-evidence/gates.log:97`). |

---

## 3. Epistemic Ledger & Weakest Link

### Epistemic Ledger (`FACT` / `INFERENCE` / `ASSUMPTION`)

- **`FACT` — 6-step Figma shader pipeline and 3 numeric holes (`plans/reports/agy-163b-image-adjustments-panel-evidence/kit-paint-filters.log:10-28`):**
  - Inspected Figma's Brotli-compressed WebAssembly binary `compiled_wasm.wasm` (`53,350,481` bytes, SHA-256 `09c36427208226f5c8fd4ec07117007df1a2d1a64d6d020d46c2679313ce8477`, REA evidence `ev_6fdfebacf7bfa6ead63f5e1440af8bee87c3e534b44ecd88850eccdcf9e756e2`, `plans/reports/agy-163b-image-adjustments-panel-evidence/kit-paint-filters.log:2-8`) and JS bundle `figma_app-9b837f4ccec188a9.min.js` (`8,396,757` bytes, SHA-256 `48b375393b89dd446cd0440b56c777cc12af668ae3e8811f321b3f18291c5a23`, `plans/reports/agy-163b-image-adjustments-panel-evidence/kit-paint-filters.log:5-7`).
  - **Hole 1 (`offset 48564838`, `plans/reports/agy-163b-image-adjustments-panel-evidence/kit-paint-filters.log:16`):** Negative-exposure re-saturation in `ag(c, j)` scales `m = 3.0` by `1.0 + n * n * min(0.0, j) / (0.2 - min(0.0, j))` (`NEG_EXPOSURE_RESAT_PIVOT = 0.2` in `crates/designos-render/src/image_adjust.rs:23,124-126` and `crates/designos-gpu/src/shaders/draw.wgsl:230,291-293`).
  - **Hole 2 (`offset 48565100`, `plans/reports/agy-163b-image-adjustments-panel-evidence/kit-paint-filters.log:18`):** Positive-saturation lower bound in `ae(c, f)` is `q = n / (n - min(c.r, min(c.g, c.b)) + 1e-6)` (`crates/designos-render/src/image_adjust.rs:141` and `crates/designos-gpu/src/shaders/draw.wgsl:307`).
  - **Hole 3 (`offset 48565143`, `plans/reports/agy-163b-image-adjustments-panel-evidence/kit-paint-filters.log:19`):** Positive-saturation upper bound in `ae(c, f)` is `r = (1.0 - n) / (max(c.r, max(c.g, c.b)) - n + 1e-6)` with `o = f * max(0.0, min(2.0, min(q, r)) - 1.0) + 1.0` (`POS_SAT_MAX_SCALE = 2.0` in `crates/designos-render/src/image_adjust.rs:26,142-143` and `crates/designos-gpu/src/shaders/draw.wgsl:231,308-309`).
  - **Contrast storage range (`offset 3698950`, `plans/reports/agy-163b-image-adjustments-panel-evidence/kit-paint-filters.log:26-27`):** `figma_app-9b837f4ccec188a9.min.js` stores `contrast` in `[-0.3, +0.3]` (`CONTRAST_LIMIT = 0.3` in `crates/designos-render/src/image_adjust.rs:20,76` and `crates/designos-gpu/src/shaders/draw.wgsl:229,246`) while displaying `-100%..+100%` in the UI (`crates/designos-app/src/widgets/picker.rs:841,1199-1217`), whereas the other 6 fields (`exposure`, `vibrance`, `temperature`, `tint`, `highlights`, `shadows`) store `[-1.0, +1.0]` (`crates/designos-app/src/widgets/picker.rs:840-847`).
- **`FACT` — Re-anchored codebase line numbers:**
  - `ImageAdjust`: `crates/designos-doc/src/lib.rs:519-541` (`is_identity` at `crates/designos-doc/src/lib.rs:531-540`).
  - `ImageParams` & `ImageScaleMode`: `crates/designos-doc/src/lib.rs:509-517`, `crates/designos-doc/src/lib.rs:543-563`.
  - `image_uv_map`: `crates/designos-doc/src/lib.rs:2482-2548`.
  - `.fig` `paintFilter` import/export: `crates/designos-fig/src/import.rs:2123-2142` (`convert_image_adjust`), `crates/designos-fig/src/fig_export.rs:2084-2104`.
  - CPU image fill paint path: `crates/designos-render/src/paint.rs:344-419`.
  - GPU `Paint::Image` conversion: `crates/designos-gpu/src/paintconv.rs:146-184`.
  - GPU `Instance` struct (`64 B` stride): `crates/designos-gpu/src/pipeline.rs:39-57`.
  - WGSL `fs_image` shader: `crates/designos-gpu/src/shaders/draw.wgsl:338-359`.
  - Picker `Mode::Image`: `crates/designos-app/src/widgets/picker.rs:828-861,1084-1301`.
  - Properties panel fill picker caller: `crates/designos-app/src/panels/properties.rs:2399-2481`.
- **`FACT` — 3 Untitled UI PRO VARIABLES (v8.0) kit nodes with non-zero `paintFilter` (`plans/reports/agy-163b-image-adjustments-panel-evidence/kit-paint-filters.log:29-32`):**
  1. `1277:186` (`NodeId(41802)`, `"Section"`, `880.0x960.0`, `Fill`, `vibrance = -1.0`, `plans/reports/agy-163b-image-adjustments-panel-evidence/kit-paint-filters.log:30`).
  2. `1369:8096` (`NodeId(106660)`, `"Metrics"`, `1216.0x340.0`, `Fill`, `exposure = -0.46769068`, `plans/reports/agy-163b-image-adjustments-panel-evidence/kit-paint-filters.log:31`).
  3. `1369:8211` (`NodeId(106786)`, `"Metrics"`, `343.0x788.0`, `Fill`, `exposure = -0.46769068`, `plans/reports/agy-163b-image-adjustments-panel-evidence/kit-paint-filters.log:32`).
- **`INFERENCE` — Bilinear texel order vs. per-fragment evaluation:**
  - CPU (`crates/designos-render/src/paint.rs:351-355`) adjusts decoded source texels prior to `tiny-skia` bilinear sampling (`FilterQuality::Bilinear`, `crates/designos-render/src/paint.rs:390,415`), whereas GPU (`crates/designos-gpu/src/shaders/draw.wgsl:339-349`) evaluates `adjust_image` per fragment after `textureSampleLevel`. On solid colors and low-frequency regions they match within `≤ 1/255` (`0.0000%` mismatch at tolerance `2`, `plans/reports/agy-163b-image-adjustments-panel-evidence/kit-render-head.log:9-12,14-28`); on high-frequency downscaled photo edges (`kit_0`, `kit_1`, `kit_2`) the CPU/GPU diff actually *decreases* from `5.56% / 4.08% / 7.73%` (unadjusted base) to `4.15% / 3.94% / 7.52%` (`plans/reports/agy-163b-image-adjustments-panel-evidence/kit-render-head.log:15-28`).
- **`ASSUMPTION` — Multi-node selection frame dimensions for `Fill`/`Fit` -> `Crop`:**
  - When opening the fill picker from the properties panel, `pk.state.image_frame` is populated from the first selected node's `(node.size.x, node.size.y)` (`crates/designos-app/src/panels/properties.rs:2400-2404`), which matches single-node selection and provides a deterministic fallback frame size if multiple nodes are selected.

### Weakest Link (One Line)

CPU `apply_pixmap_adjustments` (`crates/designos-render/src/paint.rs:351-355`) transforms source image texels before bilinear filtering while GPU `fs_image` (`crates/designos-gpu/src/shaders/draw.wgsl:339-349`) transforms sampled fragments after bilinear filtering, which can differ by `1–3` levels along high-contrast downscaled texel boundaries (`plans/reports/agy-163b-image-adjustments-panel-evidence/kit-render-head.log:15-28`).

---

## 4. Acceptance Criteria & Verification Table

| # | Requirement (`docs/specs/v0.7/163-image-fill-adjustments-rotation-tile-scale-and-crop.md`) | Implementation (`file:line`) | Verification Test / Log (`log-file:line`) | Verdict |
|---|---|---|---|---|
| 1 | `#808080` at `exposure = +1.0` brightens to `[251, 251, 251, 255]` (`#FBFBFB`) on CPU and GPU within `±2/255` (`docs/specs/v0.7/163-image-fill-adjustments-rotation-tile-scale-and-crop.md:112-113`) | `crates/designos-render/src/image_adjust.rs:113-130,148-190`, `crates/designos-gpu/src/shaders/draw.wgsl:281-298,313-359` | `exposure_plus_one_brightens_a_literal_pixel_equally_on_cpu_and_gpu` (`crates/designos-gpu/tests/fidelity.rs:1022-1042`, `plans/reports/agy-163b-image-adjustments-panel-evidence/green-tests.log:89-93`, `plans/reports/agy-163b-image-adjustments-panel-evidence/kit-render-head.log:9-10`) | **PASS** |
| 2 | Each of the 7 steps (`exposure`, `contrast`, `vibrance`, `temperature`, `tint`, `highlights`, `shadows`) at `±0.5` matches `f64` reference within `±2/255` on CPU and GPU (`docs/specs/v0.7/163-image-fill-adjustments-rotation-tile-scale-and-crop.md:45-56,114`) | `crates/designos-render/src/image_adjust.rs:61-178`, `crates/designos-gpu/src/shaders/draw.wgsl:233-336`, `crates/designos-gpu/src/paintconv.rs:175-182` | `adjustment_unit_tests_per_step` (`crates/designos-gpu/tests/fidelity.rs:1044-1116`, `plans/reports/agy-163b-image-adjustments-panel-evidence/green-tests.log:94-166`) | **PASS** |
| 3 | Switching `Fill`/`Fit` -> `Crop` writes `params.transform` reproducing the framing (with `rotation` folded in); switching `Crop` -> `Fill`/`Fit` keeps `transform` stored (`docs/specs/v0.7/163-image-fill-adjustments-rotation-tile-scale-and-crop.md:42-43,115`) | `crates/designos-render/src/image_adjust.rs:225-253`, `crates/designos-app/src/widgets/picker.rs:1108-1116` | `fill_to_crop_switch_reproduces_the_framing` (`crates/designos-gpu/tests/fidelity.rs:1118-1185`, `plans/reports/agy-163b-image-adjustments-panel-evidence/green-tests.log:167-172`) | **PASS** |
| 4 | All-zero `ImageAdjust` skips pipeline (`q[1] == 0` on GPU, no pixmap clone on CPU) and is byte-identical to unadjusted render (`docs/specs/v0.7/163-image-fill-adjustments-rotation-tile-scale-and-crop.md:56,95,116`) | `crates/designos-render/src/image_adjust.rs:35-43`, `crates/designos-render/src/paint.rs:351-355`, `crates/designos-gpu/src/paintconv.rs:175-182` | `all_zero_adjustments_are_byte_identical_to_the_previous_render` (`crates/designos-gpu/tests/fidelity.rs:1187-1222`, `plans/reports/agy-163b-image-adjustments-panel-evidence/green-tests.log:96-114`) | **PASS** |
| 5 | Transparent pixels (`alpha == 0`), `Fit` letterbox bars, and out-of-bounds `Crop` regions remain `[0, 0, 0, 0]`; `alpha in (0, 255)` un-premultiplies RGB and preserves `alpha`; blend mode & opacity apply after adjustments (`docs/specs/v0.7/163-image-fill-adjustments-rotation-tile-scale-and-crop.md:97-99`) | `crates/designos-render/src/image_adjust.rs:181-219`, `crates/designos-render/src/paint.rs:349-364`, `crates/designos-gpu/src/shaders/draw.wgsl:346-358` | `fit_letterbox_and_out_of_bounds_crop_remain_transparent_under_positive_adjustments` (`crates/designos-render/tests/render.rs:978-1016`), `blend_mode_and_opacity_apply_after_image_adjustments` (`crates/designos-render/tests/render.rs:1018-1047`, `plans/reports/agy-163b-image-adjustments-panel-evidence/green-tests.log:68-72`) | **PASS** |
| 6 | Picker `Mode::Image` renders `"Rotate"` (`24 px` left/right `90°` buttons + `font_mono` angle), `"Tile scale"` (`Tile` mode only), and `"Adjustments"` (`7` rows at `28 px` `theme::BADGE_LG` + conditional `"Reset"`), committing one undo step per release (`docs/specs/v0.7/163-image-fill-adjustments-rotation-tile-scale-and-crop.md:83-91,117`) | `crates/designos-app/src/widgets/picker.rs:831-860,1091-1301`, `crates/designos-app/src/panels/properties.rs:2400-2404,2477-2480` | `picker_adjustment_row_edits_through_the_paint_path` (`crates/designos-app/src/widgets/picker.rs:3828-3921`), `picker_rotate_buttons_step_by_90_degrees` (`crates/designos-app/src/widgets/picker.rs:3924-3945`), `picker_tile_scale_row_shown_only_in_tile_mode_and_edits_scale` (`crates/designos-app/src/widgets/picker.rs:3948-3981`, `plans/reports/agy-163b-image-adjustments-panel-evidence/green-tests.log:364-369`) | **PASS** |
| 7 | Kit nodes `1277:186` (`vibrance = -1.0`), `1369:8096` (`exposure = -0.46769068`), and `1369:8211` (`exposure = -0.46769068`) visibly render their adjustments on CPU and GPU (`docs/specs/v0.7/163-image-fill-adjustments-rotation-tile-scale-and-crop.md:27,118`) | `crates/designos-render/src/paint.rs:349-364`, `crates/designos-gpu/src/paintconv.rs:175-182`, `crates/designos-gpu/src/shaders/draw.wgsl:338-359` | `kit_nodes_with_adjustments_render_on_cpu_and_gpu` (`crates/designos-gpu/tests/fidelity.rs:1224-1306`, `plans/reports/agy-163b-image-adjustments-panel-evidence/kit-render-head.log:13-31`) | **PASS** |
| 8 | Interactive frame budget holds (`< 8 ms` compile budget, `< 64` `Program`s, `FrameKey` reuse `stale_hits == 0` when idle) | `crates/designos-gpu/src/paintconv.rs:175-182` | `image_adjustments_interactive_frame_budget_holds` (`crates/designos-gpu/tests/fidelity.rs:1308-1402`, `plans/reports/agy-163b-image-adjustments-panel-evidence/perf.log:236-240`) | **PASS** |

---

## 5. Step-by-Step Evidence & Pasted Outputs

### 5.1 Red Tests on Base `8087735f4` (`plans/reports/agy-163b-image-adjustments-panel-evidence/red-tests.log:1-157`)

Before implementing the adjustment pipeline and picker controls, the new tests were executed against base `8087735f4` (`plans/reports/agy-163b-image-adjustments-panel-evidence/red-tests.log:1`), failing at the exact missing features:

```text
thread 'exposure_plus_one_brightens_a_literal_pixel_equally_on_cpu_and_gpu' (94263632) panicked at crates/designos-gpu/tests/fidelity.rs:1092:9:
CPU channel 0 was not brightened above 0x80: [128, 128, 128, 255]
test exposure_plus_one_brightens_a_literal_pixel_equally_on_cpu_and_gpu ... FAILED (red-tests.log:14-17)

thread 'adjustment_unit_tests_per_step' (94263697) panicked at crates/designos-gpu/tests/fidelity.rs:1151:13:
exposure_+0.5_grey: adjust_rgba [128, 128, 128, 255] != expected [150, 150, 150, 255] on channel 0
test adjustment_unit_tests_per_step ... FAILED (red-tests.log:36-39)

thread 'fill_to_crop_switch_reproduces_the_framing' (94263739) panicked at crates/designos-gpu/tests/fidelity.rs:1230:38:
Fill -> Crop must write params.transform
test fill_to_crop_switch_reproduces_the_framing ... FAILED (red-tests.log:60-63)

thread 'widgets::picker::tests::picker_adjustment_row_edits_through_the_paint_path' (94267497) panicked at crates/designos-app/src/widgets/picker.rs:3630:13:
missing label "Adjustments" in image picker texts: ["Rectangle", "Position", "Alignment", "Position", "X", "0", "Y", "0", "Rotation", "0", "°", "Layout", "Dimensions", "W", "100", "H", "100", "Appearance", "100", "%", "0", "0", "%", "iOS", "Fill", "Image", "100", "%", "AI", "Stroke", "Effects", "Export", "Custom", "Libraries", "Image fill", "Scale: Fill", "Studio Grid", "Gradient Mesh", "Checkerboard", "Choose from disk…"]
test widgets::picker::tests::picker_adjustment_row_edits_through_the_paint_path ... FAILED (red-tests.log:99-102)

thread 'widgets::picker::tests::picker_rotate_buttons_step_by_90_degrees' (94268729) panicked at crates/designos-app/src/widgets/picker.rs:3705:9:
missing Rotate row label in ["Custom", "Libraries", "Image fill", "Scale: Fill", "Studio Grid", "Gradient Mesh", "Checkerboard", "Choose from disk…"]
test widgets::picker::tests::picker_rotate_buttons_step_by_90_degrees ... FAILED (red-tests.log:122-125)

thread 'widgets::picker::tests::picker_tile_scale_row_shown_only_in_tile_mode_and_edits_scale' (94269726) panicked at crates/designos-app/src/widgets/picker.rs:3740:9:
Tile scale must be visible in Tile mode: ["Custom", "Libraries", "Image fill", "Scale: Tile", "Studio Grid", "Gradient Mesh", "Checkerboard", "Choose from disk…"]
test widgets::picker::tests::picker_tile_scale_row_shown_only_in_tile_mode_and_edits_scale ... FAILED (red-tests.log:145-148)
```

### 5.2 Green Tests on HEAD (`plans/reports/agy-163b-image-adjustments-panel-evidence/green-tests.log:1-370`)

All `11` new tests across `designos-render` (`2` tests, `plans/reports/agy-163b-image-adjustments-panel-evidence/green-tests.log:68-72`), `designos-gpu` (`6` tests, `plans/reports/agy-163b-image-adjustments-panel-evidence/green-tests.log:81-191`), and `designos-app` (`3` tests, `plans/reports/agy-163b-image-adjustments-panel-evidence/green-tests.log:364-369`) pass:

```text
running 2 tests
test blend_mode_and_opacity_apply_after_image_adjustments ... ok
test fit_letterbox_and_out_of_bounds_crop_remain_transparent_under_positive_adjustments ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 41 filtered out; finished in 0.01s (green-tests.log:68-72)

running 6 tests
IMAGE_ADJUST_PERF: base_pan(20f) p50=0.101ms p95=0.116ms max=0.118ms draws=42 progs=1 | live_adjust_drag(20f) p50=0.328ms p95=0.368ms max=0.421ms draws=48 progs=1 | idle_stale_hits=0
test image_adjustments_interactive_frame_budget_holds ... ok
test exposure_plus_one_brightens_a_literal_pixel_equally_on_cpu_and_gpu ... ok
test all_zero_adjustments_are_byte_identical_to_the_previous_render ... ok
test adjustment_unit_tests_per_step ... ok
test fill_to_crop_switch_reproduces_the_framing ... ok
test kit_nodes_with_adjustments_render_on_cpu_and_gpu ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 30 filtered out; finished in 29.64s (green-tests.log:81-191)

running 3 tests
test widgets::picker::tests::picker_tile_scale_row_shown_only_in_tile_mode_and_edits_scale ... ok
test widgets::picker::tests::picker_rotate_buttons_step_by_90_degrees ... ok
test widgets::picker::tests::picker_adjustment_row_edits_through_the_paint_path ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 4325 filtered out; finished in 0.20s (green-tests.log:364-369)
```

### 5.3 Adversarial Falsification — 3 Mutants (`plans/reports/agy-163b-image-adjustments-panel-evidence/mutants.log:1-300`)

1. **Step 0 — Green null control (`plans/reports/agy-163b-image-adjustments-panel-evidence/mutants.log:1-89`):** `adjustment_unit_tests_per_step` and `fill_to_crop_switch_reproduces_the_framing` pass (`2 passed; 0 failed`, `plans/reports/agy-163b-image-adjustments-panel-evidence/mutants.log:88`).
2. **Step 1 — Mutant (i): GPU skips saturation step (`img_saturation`) in `crates/designos-gpu/src/shaders/draw.wgsl:333` (`plans/reports/agy-163b-image-adjustments-panel-evidence/mutants.log:91-134`):**
   - **Prediction:** `adjustment_unit_tests_per_step` will fail on `saturation_+0.5_warm` because GPU pixel remains `[200, 120, 60, 255]` instead of `[205, 118, 44, 255]`.
   - **Observed (`plans/reports/agy-163b-image-adjustments-panel-evidence/mutants.log:122-125`):**
     ```text
     thread 'adjustment_unit_tests_per_step' (96319460) panicked at crates/designos-gpu/tests/fidelity.rs:1112:13:
     saturation_+0.5_warm: GPU pixel [200, 120, 60, 255] != expected [205, 118, 44, 255] on channel 0
     test adjustment_unit_tests_per_step ... FAILED
     ```
3. **Step 2 — Mutant (ii): CPU scales alpha channel by `0.5` in `crates/designos-render/src/image_adjust.rs:189` (`plans/reports/agy-163b-image-adjustments-panel-evidence/mutants.log:136-165`):**
   - **Prediction:** `adjustment_unit_tests_per_step` will fail on the semi-transparent alpha preservation check (`a = 128`).
   - **Observed (`plans/reports/agy-163b-image-adjustments-panel-evidence/mutants.log:151-156`):**
     ```text
     thread 'adjustment_unit_tests_per_step' (96320872) panicked at crates/designos-gpu/tests/fidelity.rs:1091:5:
     assertion `left == right` failed: semi-transparent alpha must remain 128
       left: 64
      right: 128
     test adjustment_unit_tests_per_step ... FAILED
     ```
4. **Step 3 — Mutant (iii): `Fill` -> `Crop` writes identity transform `[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]` in `crates/designos-render/src/image_adjust.rs:234` (`plans/reports/agy-163b-image-adjustments-panel-evidence/mutants.log:167-205`):**
   - **Prediction:** `fill_to_crop_switch_reproduces_the_framing` will fail on `params.transform` (`expected [1.0, 0.0, 0.0, 0.0, 0.5, 0.25]`).
   - **Observed (`plans/reports/agy-163b-image-adjustments-panel-evidence/mutants.log:193-196`):**
     ```text
     thread 'fill_to_crop_switch_reproduces_the_framing' (96322327) panicked at crates/designos-gpu/tests/fidelity.rs:1151:21:
     expected transform [1.0, 0.0, 0.0, 0.0, 0.5, 0.25], got [1.0, 0.0, 0.0, 0.0, 1.0, 0.0]
     test fill_to_crop_switch_reproduces_the_framing ... FAILED
     ```
5. **Step 4 — Post-revert green verification (`plans/reports/agy-163b-image-adjustments-panel-evidence/mutants.log:207-300`):** Reverted all mutants and re-ran both tests (`2 passed; 0 failed`, `plans/reports/agy-163b-image-adjustments-panel-evidence/mutants.log:299`).

### 5.4 Visual Evidence Inspection (CPU & GPU PNGs)

Every generated PNG in `plans/reports/agy-163b-image-adjustments-panel-evidence/` was visually inspected:

| PNG File (`plans/reports/agy-163b-image-adjustments-panel-evidence/`) | Log Citation (`log-file:line`) | Visual Inspection Summary (1–2 lines) |
|---|---|---|
| `solid-808080-exp1-base-cpu.png` & `solid-808080-exp1-base-gpu.png` | `kit-render-base.log:20-21`, `kit-render-head.log:11-12` | Unadjusted `16x16` mid-grey `#808080` (`[128, 128, 128, 255]`) square; CPU and GPU match with `0.0000%` mismatch (`SSIM 1.0000`). |
| `solid-808080-exp1-head-cpu.png` & `solid-808080-exp1-head-gpu.png` | `kit-render-head.log:9-10` | Adjusted `16x16` square at `exposure = +1.0` rendered as near-white `#FBFBFB` (`[251, 251, 251, 255]`); CPU and GPU match with `0.0000%` mismatch (`SSIM 1.0000`) and differ from base on `93.75%` of scene pixels (`kit-render-base.log:20`). |
| `kit-node-0-base-cpu.png` & `kit-node-0-base-gpu.png` | `kit-render-base.log:11-13` | Node `1277:186` (`"Section"`, `vibrance = -1.0`) rendered on base `8087735f4`: the portrait photo in the hero card is warm full-color peach/brown because `vibrance = -1.0` was ignored. |
| `kit-node-0-head-cpu.png` & `kit-node-0-head-gpu.png` | `kit-render-head.log:14-18` | Node `1277:186` (`"Section"`, `vibrance = -1.0`) rendered on HEAD: the portrait photo is completely desaturated into neutral B&W greyscale (`R=G=B` luma) on both CPU and GPU (`SSIM 0.9945`), while surrounding card UI chrome is unchanged. |
| `kit-node-1-base-cpu.png` & `kit-node-1-base-gpu.png` | `kit-render-base.log:14-16` | Node `1369:8096` (`"Metrics"`, `1216x340`, `exposure = -0.46769068`) on base `8087735f4`: background photo behind white metric KPI overlays is bright, reducing white-text contrast. |
| `kit-node-1-head-cpu.png` & `kit-node-1-head-gpu.png` | `kit-render-head.log:19-23` | Node `1369:8096` (`"Metrics"`, `exposure = -0.46769068`) on HEAD: background photo is visibly darkened by `-46.8%` exposure on both CPU and GPU (`SSIM 0.9943`), while crisp white metric typography (`"400+"`, `"600%"`, `"10k"`, `"200+"`) stays untouched at `#FFFFFF`. |
| `kit-node-2-base-cpu.png` & `kit-node-2-base-gpu.png` | `kit-render-base.log:17-19` | Node `1369:8211` (`"Metrics"`, `343x788` mobile card, `exposure = -0.46769068`) on base `8087735f4`: unadjusted bright background photo behind stacked mobile metric cards. |
| `kit-node-2-head-cpu.png` & `kit-node-2-head-gpu.png` | `kit-render-head.log:24-28` | Node `1369:8211` (`"Metrics"`, `exposure = -0.46769068`) on HEAD: background photo is darkened identically on CPU and GPU (`SSIM 0.9808`), matching the desktop banner treatment while preserving white overlay text. |
| `picker-image-adjustments-open.png` | `picker-window-capture-idle-check.log:4`, `crates/designos-app/src/widgets/picker.rs:3890-3905` | Headless `240x528` capture of `Mode::Image` in `Tile` mode with non-zero adjustments (`exposure = +0.5`, `contrast = -0.15` displayed as `-50%`): shows `"Scale: Tile"`, `"Rotate"` row with `24 px` left/right `90°` buttons and `"0°"` in `font_mono`, `"Tile scale"` slider (`100%`), `"Adjustments"` header with `"Reset"` button, and all `7` adjustment rows (`28 px` each) with centered `0` tick and active accent fill. |
| `picker-image-adjustments-reset.png` | `picker-window-capture-idle-check.log:4`, `crates/designos-app/src/widgets/picker.rs:3907-3920` | Headless `240x528` capture of `Mode::Image` after clicking `"Reset"`: all `7` adjustment sliders return to `0%`, and the `"Reset"` button is hidden (`any_adj == false`). |

### 5.5 Performance Benchmark (`plans/reports/agy-163b-image-adjustments-panel-evidence/perf.log:236-240`)

Measured on `Apple M4 Max (Metal)` in `release` profile (`crates/designos-gpu/tests/fidelity.rs:1308-1402`, `plans/reports/agy-163b-image-adjustments-panel-evidence/perf.log:233-240`) across a `16`-image adjusted grid (`4x4` nodes):
- **`20`-frame pan with unadjusted images (`base_pan`):** `p50 = 0.029 ms`, `p95 = 0.032 ms`, `max = 0.033 ms`, `draws = 42`, `progs = 1` (`plans/reports/agy-163b-image-adjustments-panel-evidence/perf.log:237`).
- **`20`-frame live adjustment slider drag (`live_adjust_drag`):** `p50 = 0.072 ms`, `p95 = 0.085 ms`, `max = 0.157 ms` (`< 8.0 ms` interactive frame budget), `draws = 48`, `progs = 1` (`< 64` cap) (`plans/reports/agy-163b-image-adjustments-panel-evidence/perf.log:237`).
- **Idle `FrameKey` reuse check:** `idle_stale_hits = 0` (`plans/reports/agy-163b-image-adjustments-panel-evidence/perf.log:237`).

```text
running 1 test
IMAGE_ADJUST_PERF: base_pan(20f) p50=0.029ms p95=0.032ms max=0.033ms draws=42 progs=1 | live_adjust_drag(20f) p50=0.072ms p95=0.085ms max=0.157ms draws=48 progs=1 | idle_stale_hits=0
test image_adjustments_interactive_frame_budget_holds ... ok (perf.log:236-238)
```

### 5.6 Repository & Workspace Gates (`plans/reports/agy-163b-image-adjustments-panel-evidence/gates.log:1-10863`)

| Gate Command | Result | Log Citation (`plans/reports/agy-163b-image-adjustments-panel-evidence/gates.log:line`) |
|---|---|---|
| `python3 scripts/zero_fake_ui_gate.py` | `PASS (17/17 invariants satisfied)` | `gates.log:1-26` |
| `python3 scripts/design_os_self_evolve.py` | `G01..G08 PASS` (`35/35` `theme.rs` tokens, `0` Purple Ban hits, `17/17` Zero-Fake-UI, `78` DS components, `0` a11y failures, `6/6` specimen families); `G09` reports `6` pre-existing HTML errors in `plans/261102-project-docs-workspace/ui-mockups/*.html` outside our scope | `gates.log:27-93` |
| `python3 scripts/check_spacing_relationships.py` | `PASS: no count rose above the baseline` (`2` entries below baseline) | `gates.log:94-97` |
| `python3 scripts/check_ds_ontology_parity.py` | `RESULT: PASS (P1, P2, P3, P4, P5, P6, P7, P9)` | `gates.log:98-110` |
| `python3 -m unittest discover -s scripts/tests -p 'test_*.py'` | `Ran 2207 tests in 75.685s — OK` | `gates.log:111-152` |
| `cargo fmt --all -- --check` | `fmt_rc=0` | `gates.log:153-154` |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | `clippy_rc=0` (`0` warnings, `0` errors) | `gates.log:155-597` |
| `cargo test -p designos-doc --locked` | `774 + 3 + 5 + 2 = 784 passed; 0 failed` | `gates.log:598-1429` |
| `cargo test -p designos-render --locked` | `56 + 4 + 3 + 7 + 9 + 1 + 7 + 3 + 9 + 12 + 5 + 5 + 10 + 32 + 11 + 5 + 6 + 43 + 8 + 5 + 21 + 1 + 15 + 1 + 7 + 2 + 5 + 3 + 23 = 319 passed; 0 failed` | `gates.log:1430-1950` |
| `cargo test -p designos-gpu --locked -- --skip large_file_compile_benchmarks` | `5 + 1 + 4 + 36 + 8 + 14 + 5 + 12 + 19 + 6 + 2 + 15 + 5 + 2 = 134 passed; 0 failed` | `gates.log:1951-2192` |
| `cargo test -p designos-fig --locked` | `146 + 422 integration = 568 passed; 0 failed` | `gates.log:2193-3064` |
| `cargo test -p designos-export --locked` | `168 + 192 integration = 360 passed; 0 failed` | `gates.log:3065-3559` |
| `cargo test -p designos-mcp --locked` | `327 + 261 integration = 588 passed; 0 failed` | `gates.log:3560-4396` |
| `cargo test --workspace --locked --lib` | `13 + 774 + 1 + 168 + 146 + 30 + 5 + 37 + 327 + 56 + 2 + 8 = 1,567 passed; 0 failed` | `gates.log:4397-6099` |
| `cargo test -p designos-app --bin designos --locked` (+ isolated reruns of known parallel/TMPDIR-length tests) | `4,313 passed` in bulk run (`gates.log:10485`); `vecops::tests::two_deferred_text_commands_both_convert_in_order` and `shell::tests::close_human_dirty::library_file_close_save_writes_the_edit_and_closes_not_path_required` passed on isolated rerun (`2 passed; 0 failed`, `gates.log:10488-10503`); `agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root` (which hit the `104`-byte Unix socket limit when `TMPDIR` was `/var/folders/...` at `130` bytes, `gates.log:10472`) passed with short canonical `/private/tmp/x.XXXXXX` (`1 passed; 0 failed`, `gates.log:10505-10863`) | `gates.log:6100-10863` |

### 5.7 Cleanup of `out/cargo` and `out/kit-clone.fig`

Both `out/cargo` and `out/kit-clone.fig` were removed (`out/` itself was preserved):

```text
$ ls out/cargo
ls: out/cargo: No such file or directory
ls_out_cargo_rc=1
```

---

Status: Ready for controller review
