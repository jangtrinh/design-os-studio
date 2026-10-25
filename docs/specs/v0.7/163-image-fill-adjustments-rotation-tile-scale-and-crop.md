# #163 Image fill adjustments, rotation, tile scale and crop (E5-3)

| Field | Value |
|---|---|
| Issue | #163 (task E5-3; parent #18; Size L / P1) |
| Size | L (three PRs are sensible: model and round trip, CPU and GPU transform, adjustments and panel) |
| Depends on | hot-file queue position 20 (#161) merged first (section 12) |
| Unblocks | #521 (its crop frames and its variable-mode slice build on the fields and the paint model added here) |
| Touches | `crates/designos-doc/src/paint.rs` (`PaintKind::Image` `:208-212`, `ImageScaleMode` `:179-185`), `crates/designos-render/src/paint.rs` (`:320-365`), `crates/designos-gpu/src/paintconv.rs` (`:162-189`), `crates/designos-gpu/src/shaders/draw.wgsl` (`fs_image`), `crates/designos-gpu/src/frame.rs` (`Instance` `:104-121`), `crates/designos-fig/src/fig.rs` (`convert_paint` `:2510-2519`), `crates/designos-fig/src/fig_export.rs` (`encode_paint` `:1858-1873`), `crates/designos-fig/src/rest_import.rs` (`parse_paint` `:86-94`), `crates/designos-fig/src/export.rs`, `crates/designos-app/src/widgets/picker.rs` (image section `:830-895`), the 38 files that name `PaintKind::Image`. HOT FILES: `fig.rs`, `fig_export.rs`, `rest_import.rs`. |

Code baseline: `origin/main@0e8a9a5`. Evidence: REA 2026-10-19 (Figma fullscreen wasm shader source for the adjustment pipeline; plugin typings 1.142.0; kiwi schema in `crates/designos-fig/src/figma_schema.txt`). Formulas are written in the spec's own words; no Figma code or shader text is reproduced. The issue text says "Pack: v0.6 Effects & Export" and cites `paint.rs:207-210`; the milestone is v0.7 and the lines are now `:208-212` (conflicts noted in README-part2).

## 1. Summary and user story

As a designer, I want an image fill to keep its exposure, contrast, saturation, temperature, tint, highlights and shadows, its rotation, its tile scale and its crop, as in Figma, so a photo looks the same after import and survives a round trip.

## 2. Current state (FACT)

- `PaintKind::Image { image_ref, scale_mode }` only (`designos-doc/src/paint.rs:208-212`); `ImageScaleMode` is Fill, Fit, Crop, Tile (`:179-185`).
- Import drops everything but the hash and the mode: `convert_paint` reads `image`, `imageHash` or the image name and `imageScaleMode` (`fig.rs:2510-2519`); `transform` (7), `rotation` (11), `scale` (12), `paintFilter` (14), `filterColorAdjust` (13), `imageShouldColorManage` (22) and the original size (19, 20) are never read. `parse_paint` maps REST `STRETCH` to Crop (`rest_import.rs:86-94`) and export writes Crop back as `STRETCH` (`fig_export.rs:1866-1872`).
- Render: Crop is drawn exactly like Fill (`designos-render/src/paint.rs:325-328`; GPU `paintconv.rs:166-169`): a centred cover scale. Tile ignores any scale: `(1.0, 0.0, 0.0)` (`render paint.rs:333`; GPU `paintconv.rs:174`). The CPU feeds a pattern transform (`render paint.rs:353-363`); the GPU feeds a local-to-UV matrix, but only its diagonal and translation (`paintconv.rs:186-187`).
- The GPU instance already carries a full 2 x 2 matrix plus translation for images (`Instance.c1`, `c2`, `frame.rs:113-116`) and `fs_image` evaluates the full affine (`shaders/draw.wgsl`, `fs_image`); mode flags are `IMG_PAD`, `IMG_FIT`, `IMG_TILE` in `Instance.q[2]` (`frame.rs:97-101`). For an image draw, `c0`, `c2.zw` and `t.w` are unused (INFERENCE from `frame.rs:104-121` and `paintconv.rs:179-188`; the executor fills `q[3]`).
- Panel: the image section of the paint picker has a "Scale: Fill / Fit / Crop / Tile" cycle button, three procedural generators and "Choose from disk" (`widgets/picker.rs:830-895`); gate `Z12` requires `Mode::Image` to create a real image paint (`scripts/zero_fake_ui_gate.py`).
- Blast radius: 66 sites name `PaintKind::Image` in 38 files; 41 of them mention `image_ref` in a literal or pattern.
- FACT (kiwi schema, `Message Paint`; REA 2026-10-19): `transform = 7` (a `Matrix`, m00 to m12), `imageScaleMode = 10` (STRETCH 0, FIT 1, FILL 2, TILE 3), `rotation = 11`, `scale = 12`, `filterColorAdjust = 13` (the legacy struct: tint, shadows, highlights, detail, exposure, vignette, temperature, vibrance), `paintFilter = 14` (a `PaintFilterMessage`), `imageShouldColorManage = 22`, `originalImageWidth = 19`, `originalImageHeight = 20`, `imageVar = 31`, `paintVar = 46`. `PaintFilterMessage` ids: tint 1, shadows 2, highlights 3, detail 4, exposure 5, vignette 6, temperature 7, vibrance 8, contrast 9, brightness 10. A crop is stored as STRETCH plus a transform.
- FACT (REA 2026-10-19, plugin typings 1.142.0): `transform` holds the plugin's image transform, mapping node UV to image UV; tile `scale` ranges from 0.01 to 100; the UI turns `rotation` in 90-degree steps; the UI slider order is exposure, contrast, saturation (vibrance), temperature, tint, highlights, shadows; contrast ranges over +/-0.3 and every other adjustment over +/-1; the default is 0.
- FACT (REA 2026-10-19, Figma fullscreen wasm shader source): the adjustment pipeline of section 3 rule 3.
- INFERENCE: the REST and plugin key names are `imageTransform`, `scalingFactor`, `rotation` and a `filters` object with `exposure`, `contrast`, `saturation`, `temperature`, `tint`, `highlights`, `shadows` (from the plugin typings' `ImagePaint`; confirm against the pinned typings package in the first commit). Saturation is kiwi `vibrance`.
- UNKNOWN: the unit and direction of kiwi `rotation` (the UI uses 90-degree steps). Closing check: owner-manual live Figma read (below).
- UNKNOWN: the default of the shadows-and-highlights threshold, the exact exposure correction for negative exposure, and the two headroom ratios of the positive-saturation rule (named only as q and r in section 3). Closing check: the same live read, as five gray-ramp exports (below), or an owner-approved REA re-read of those three steps recorded as a table of sample pixels.
- UNKNOWN: what Figma draws outside the image for a crop whose transform maps part of the node outside [0, 1]. Closing check: the live read (a crop zoomed out).
- UNKNOWN: whether the UI contrast slider shows +/-100% over the stored +/-0.3 range. Closing check: the live read (drag contrast to the maximum and read the plugin value).

## 3. Behaviour rules

1. **Model.** An image paint carries: scale mode, an optional 2 x 3 image transform M (default identity), a rotation, a tile scale (default 1), an adjustment set (all 10 `PaintFilterMessage` values, default 0), and the pass-through values `image_should_color_manage` and the original image size. Exposure, contrast, saturation, temperature, tint, highlights and shadows are rendered; detail, vignette and brightness are stored and not rendered (not in Figma's UI list).
2. **Crop (STRETCH plus a transform).** A node-local position (x, y) has node UV (u, v) = (x / width, y / height). The image UV is uv_img = M . [u, v, 1] (a 2 x 3 affine). With the default M the image is stretched to the node box. Switching a Fill or Fit paint to Crop in the panel writes the M that reproduces the current framing, so the picture does not jump.
3. **Adjustments** act on the resampled colour (after the image is sampled, un-premultiplied), in six steps, in the order tint and shadows and highlights, Lab, exposure, saturation, with the gamma 2.2 power curve and luma weights 0.2126, 0.7152, 0.0722:
   1. Linear colour = rgb^2.2.
   2. Shadows and highlights offset: h = luma - T^2.2 (T the threshold, default UNKNOWN); the offset is 0.75 h / sqrt(1 + 4 h^2) times highlights when h > 0, and times -0.999 x shadows otherwise; call it ai.
   3. Lab (D65, plain cube root): add 30 x tint to a and 30 x temperature to b. Contrast pivots on L_c = 100 T: for c < 0, L' = mix(50 + 0.2 (L - L_c), L, c + 1); for c >= 0, L' = mix(L, 100 x smootherstep(L_c - 30, L_c + 30, L), c). Clamp to 0..1.
   4. Exposure with j = exposure + ai and k = 10^(2j) - 1: desaturate 75% toward the luma; if j < 0 scale by 1 + j / 16; map each channel x to x (k + 1) / (1 + k x); re-saturate by a factor of 3 with a correction for negative j (exact form UNKNOWN, see section 2); clamp.
   5. Saturation p (kiwi vibrance): for p <= 0, c += (c - luma) x p; for p > 0, scale the chroma by min(mix(1, min(q, r), p), 2) (q and r UNKNOWN, see section 2).
   6. Output = result^(1 / 2.2).
4. **Rotation** turns the image in 90-degree steps (other values are stored and rendered as the nearest step, INFERENCE) about the centre of the fitted image rectangle for Fill and Fit, and about each tile's centre for Tile (INFERENCE; closing check in section 10). For odd quarter turns Fill and Fit use the image's swapped dimensions when choosing the cover or contain scale. The unit and direction are one constant pair in one function until the live read decides.
5. **Tile scale.** For Tile the tile is the image size times the tile scale, starting at the node's top-left (today's origin, `render paint.rs:333`), repeating in both directions.
6. **Import and export.** `imageScaleMode`, `transform`, `rotation`, `scale`, `paintFilter` and the three pass-through values round trip through `.fig` and REST; the legacy `filterColorAdjust` is read into the same adjustment set when no `paintFilter` is present and is written back as `paintFilter` (ids above).
7. **Variables.** `imageVar` and `paintVar` are #521's; this task does not read them (as today) and #521 counts what it does not resolve.
8. **CPU and GPU** use the same transform and the same adjustment function; CPU is the reference.

## 4. Data model / schema changes

- `ImageParams { transform: Option<[f32; 6]>, rotation: f32, tile_scale: f32, adjust: ImageAdjust, color_manage: Option<bool>, original_size: Option<(u32, u32)> }` with `Default`, in `designos-doc/src/paint.rs`; `ImageAdjust` holds the 10 floats. `PaintKind::Image` gains `params: ImageParams` with `#[serde(default, skip_serializing_if = "ImageParams::is_default")]`, so files without any of it stay byte-identical.
- Mechanical first commit (the 41 `image_ref` sites get `params: Default::default()` or `..`), then the behaviour commits, as in #161.
- No change to `Document` or `Node`.

## 5. Contracts

- `.designos`: one optional `params` object on an image paint; an old Studio ignores it (serde unknown-field tolerance; `PaintKind` is not `deny_unknown_fields`).
- `.fig` fields: 7, 10, 11, 12, 13 (read only), 14, 19, 20, 22; REST: `imageTransform`, `rotation`, `scalingFactor`, `filters` (INFERENCE, section 2).
- The GPU instance layout: seven adjustment values and the pass-through flags use `c0`, `c2.zw` and `t.w`, and the executor still owns `q[3]`; if a field is not free the implementer adds a second per-draw vec4, with a fidelity test unchanged. The 64-program cap and the VRAM budget are not touched (no new program: adjustments run inside `fs_image`).
- `Z12` of the zero-fake gate stays green.

## 6. File-format mapping

| Studio | Kiwi (`Paint`) | REST / plugin |
|---|---|---|
| `scale_mode` Fill, Fit, Tile | `imageScaleMode` (10): FILL 2, FIT 1, TILE 3 | `FILL`, `FIT`, `TILE` |
| Crop | STRETCH (0) plus `transform` (7) | `STRETCH` plus `imageTransform` |
| `params.transform` | `transform` (7), m00 m01 m02 / m10 m11 m12 | `imageTransform` (2 x 3) |
| `params.rotation` | `rotation` (11) | `rotation` |
| `params.tile_scale` | `scale` (12) | `scalingFactor` |
| `params.adjust` (10 values) | `paintFilter` (14); legacy `filterColorAdjust` (13) read only | `filters` (7 keys; saturation = vibrance) |
| `params.color_manage`, `original_size` | 22; 19 and 20 | not carried to REST |

## 7. UI

Words only. Checked against `design/principles.md` (P3, P4, P5, P6, P10) and `design/soul.md`; tokens from `theme.rs`; the image section is part of the paint picker (`widgets/picker.rs`).

- Below the "Image fill" header row the picker gains an "Adjustments" group: seven rows in Figma's order (Exposure, Contrast, Saturation, Temperature, Tint, Highlights, Shadows), each a label and a numeric field with a slider at `CONTROL_STANDARD` (28 px), values in `font_mono` with `%`. The scale of the contrast row (stored range +/-0.3) is UNKNOWN (section 2); until the live read the row shows the stored value times 100 with `%`. A "Reset" text action in the group header appears only when any value is non-zero.
- For Tile the section also shows a "Tile scale" row; for every mode a "Rotate" row with two 24 px icon buttons (rotate left, rotate right by 90 degrees) and the current angle in `font_mono`.
- The "Scale: Crop" mode label stays; there is no crop-editing handle in this task (non-goal): Crop is import, round trip and render. The cycle button keeps its four modes.
- The picker is a popover of fixed width; adding rows grows it vertically only; no new colour or size literal; no count in a label; detail, vignette and brightness have no row (stored, not shown).
- Desktop capture at 1440 in the PR: the picker with the Adjustments group, a photo at exposure +1 beside the original, a 90-degree rotation, a tile at scale 2. Responsive: SKIPPED (owner default). Load es-designer before UI code; update Figma from the verified capture first.

## 8. Edge cases

- All adjustments 0: the pipeline is skipped entirely (byte-identical to today's render).
- Contrast outside the stored range: clamp to +/-0.3 (INFERENCE from the range above).
- Transparent pixels: the colour is un-premultiplied only where alpha is above 0; alpha is never changed by the pipeline.
- A crop whose transform is singular (determinant 0): draw nothing and report once; never divide by zero.
- A crop that maps outside [0, 1]: transparent outside (ASSUMPTION; Figma's rule is UNKNOWN).
- Rotation not a multiple of 90: stored verbatim, drawn at the nearest quarter turn.
- Tile scale below 0.01 or above 100: clamp to the range.
- Very large images: the CPU keeps its decode and downscale cache (`load_image`, `render paint.rs:137-165`); adjustments run on the sampled output, so the cache is shared by all adjustment values.
- Fit mode letterbox stays transparent (`render paint.rs:354-362`); adjustments never touch the letterbox.
- A blend mode or opacity on the paint: applied after the adjustments, as today for paints.
- `imageVar` and `paintVar` present: untouched here (see rule 7).
- Export to SVG and PDF keep drawing image fills as today (no adjustments, rotation, tile scale or crop; the SVG exporter returns a `String` and has no warnings channel, `svg.rs:39`). This is a documented gap for the PR and the release notes, not a regression; baking is a follow-up (non-goal).

## 9. Non-goals

- Crop-editing handles on the canvas and in the panel.
- Detail, vignette and brightness rendering.
- Image variables and paint variables (#521).
- Baking adjustments into SVG or PDF output.
- Colour management (`imageShouldColorManage` is carried, not applied).

## 10. Test plan (red first)

Seams: pixel tests in `designos-render/tests/render.rs`, CPU = GPU in `designos-gpu/tests/fidelity.rs` (tolerance 8, strict mismatch under 2%; a literal pixel within 2 levels), round trips in `designos-fig/tests/` (patterns of `fig_roundtrip.rs` and `rest_import.rs`).

Literal pixel for the first acceptance criterion: a solid `#808080` image (128, 128, 128) on a node of the same size with exposure +1 and every other value 0. By the pipeline above with j = 1 and k = 99, the linear value 0.2195 maps to 0.9657 and back through the 1/2.2 curve to 251 (hand-computed from the rules as written; INFERENCE until the live read). The CPU and the GPU both produce 251 within 2 levels; the test recomputes it with an f64 reference function and uses that value.

| Case | Expected | Fails before |
|---|---|---|
| `exposure_plus_one_brightens_a_literal_pixel_equally_on_cpu_and_gpu` | the pixel is 251 (+/- 2) on both | yes |
| `all_zero_adjustments_are_byte_identical_to_the_previous_render` | identical | guard |
| `rotation_90_is_exact_on_a_2x2_image` | image A B / C D on a 2 x 2 node, Fill, rotation one quarter turn: the four pixels are the exact permutation for the chosen direction (a constant); CPU = GPU exactly | yes |
| `tile_scale_2_doubles_the_tile_size` | a 4 x 4 image tiled on a 16 x 16 node at scale 2 shows 2 repeats per row, not 4 | yes |
| `crop_with_identity_transform_stretches_the_image` | stretched to the node box (no cover scale) | yes (drawn as Fill) |
| `crop_transform_maps_uv_through_the_affine` | a transform scaling u by 0.5 shows the left half of the image | yes |
| `fill_to_crop_switch_reproduces_the_framing` | pixels equal before and after the mode switch | yes |
| `singular_crop_transform_draws_nothing` | no panic, empty fill | yes |
| `adjustment_unit_tests_per_step` | f64 reference values for tint, temperature, contrast (c < 0 and c >= 0), shadows and highlights at fixed T (the test fixes T as a constant until the read) | yes |
| `fig_round_trip_keeps_transform_rotation_scale_filters_and_flags` | fields 7, 11, 12, 14, 19, 20, 22 survive import, export, re-import | yes |
| `legacy_filter_color_adjust_reads_into_the_adjustment_set` | values equal; export writes `paintFilter` | yes |
| `rest_round_trip_keeps_image_transform_scaling_factor_and_filters` | keys survive | yes |
| `old_designos_file_loads_with_default_params_and_saves_unchanged` | JSON identical | guard |
| `picker_adjustment_row_edits_through_the_paint_path` (app) | one undo step per drag release | yes |
| `zero_fake_ui_gate` | `Z12` and the rest green | guard |

Run: `cargo test -p designos-doc`, `cargo test -p designos-render`, `cargo test -p designos-gpu --test fidelity`, `cargo test -p designos-fig`, `cargo test -p designos-app picker`. Evidence: red/green capture, CPU and GPU renders of the exposure and rotation fixtures, the 1440 picker capture.

Owner-manual check (flagged, never claimed by an agent): **live Figma read.** In a scratch Figma file, (1) apply each of the seven adjustments at the minimum, the maximum and one value between, to a 256-step gray ramp image and a colour chart, export at 1x and read pixels (closes the threshold default, the negative-exposure correction and the positive-saturation rule); (2) rotate an image fill by each 90-degree step in Fill, Fit and Tile and read the plugin `rotation` value and the stored `.fig` value (closes the unit, direction and tile rotation centre); (3) crop an image, zoom the crop out so it leaves the node, export and read the outside (closes the outside rule); (4) read the contrast slider and its plugin value at the maximum (closes the UI scale).

## 11. Performance budgets

- Adjustments off: zero added cost (the code path is skipped).
- GPU: adjustments are per-pixel arithmetic in `fs_image` (a few dozen ALU operations and one cube-root pair per pixel for the Lab step); no new program, no extra texture; a page of large image fills stays within the 8 ms frame budget on the reference machine (measured on the existing large-file perf fixture, recorded in the PR).
- CPU reference: an adjusted fill renders into a scratch layer of the fill's bounding box then composites; one extra allocation per adjusted fill per render, bounded by `MAX_REGION_PIXELS`; the decode cache is unchanged.
- Dragging a slider: one repaint per frame, no document hash on the UI thread (CLAUDE.md section 4); idle 0.00%.

## 12. Dependencies and build order

Hot-file queue position 21 (issue text): claim only after position 20 (#161) merged. Then #521 (position 22 in README-part2). Commits: mechanical `params` field, `.fig` and REST round trip, CPU and GPU transform (crop, rotation, tile scale), adjustments (CPU, GPU), panel. The render commits touch no hot file and may merge after the round-trip commit without waiting for the panel.

## 13. Owner rulings

- Acceptance (issue): exposure +1 brightens a literal pixel equally on CPU and GPU; a 90-degree rotation is exact; exposure, contrast, saturation, temperature, tint, highlights, shadows, rotation, tile scale and crop round trip.
- Delivery law 1: detail, vignette, brightness and the colour-management flag are carried, not dropped; the SVG and PDF gap is written down in the PR.
- The "non-AI crop" gap item of the issue context is covered by the Crop rules.

## Open questions for the owner (recommended first)

1. Run the live Figma read (four items above, about 45 minutes). Recommended: yes; it closes six UNKNOWNs and the three numeric holes of the pipeline.
2. Crop editing (handles on the canvas). Recommended: a follow-up task; this task renders and round trips crops from Figma files.
3. Adjustments on the sampled colour (this spec) versus on the decoded image before resampling. Recommended: sampled colour, as it matches how a shader-based renderer does it; the difference shows only on hard edges.
4. Contrast slider scale. Recommended: show the stored value times 100 until the read shows Figma's mapping.

## 14. Facts resolved by REA 2 (2026-10-19)

Static reading of Figma's fullscreen wasm and app bundle. No Figma code was copied. Where a line above says UNKNOWN or INFERENCE and this section says FACT, this section wins.

- FACT: `highlightShadowThreshold` is the constant 0.5, not a stored property, so the contrast pivot is Lc = 50.
- FACT: the UI/plugin contrast equals the stored value divided by 0.3, clamped to ±1, so ±100% in the UI means ±0.3 in kiwi. Exposure, tint, temperature, highlights and shadows map 1:1. Saturation is kiwi `vibrance`. UNKNOWN: whether the renderer rescales contrast again.
- FACT: kiwi image `rotation` is in degrees, read back mod 360, and only for Fill, Fit and Tile. Stretch is exposed as Crop with a transform and has no rotation. UNKNOWN: the rotation origin (INFERENCE: the image centre).
