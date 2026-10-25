# #37 Progressive blur effect (model, CPU and GPU render, `.fig` round trip)

| Field | Value |
|---|---|
| Issue | #37 (task; roadmap; full-scope amendment 2026-10-17) |
| Size | L |
| Depends on | hot-file queue position 17 (#38) merged first (section 12); #528 lands before the bounds line of section 5 |
| Unblocks | #39 (queue position 19); the blur-radius calibration that decides the uniform-blur kernel question of #520 |
| Touches | `crates/designos-doc/src/paint.rs` (`Effect::LayerBlur` / `BackgroundBlur` `:312-319`), `crates/designos-render/src/effects.rs` (`blur_rect` `:206-220`, `background_blur` `:175-203`) with a new `crates/designos-render/src/progressive_blur.rs`, `crates/designos-render/src/lib.rs` (`:346-376`, `:454-456`), `crates/designos-gpu/src/compile.rs` (`:769-815`, `:1350`), `crates/designos-gpu/src/frame.rs` (`Post`), `crates/designos-gpu/src/exec.rs` (`:686`), `crates/designos-gpu/src/shaders/blur.wgsl` (or a new shader file), `crates/designos-gpu/src/pipelines.rs`, `crates/designos-fig/src/fig.rs` (`convert_effect` `:2525-2568`), `crates/designos-fig/src/fig_export.rs` (`encode_effect` `:1906-1907`), `crates/designos-fig/src/rest_import.rs` (`parse_effect` `:120-121`), `crates/designos-fig/src/export.rs`, `crates/designos-app/src/panels/effects.rs`, `crates/designos-export/src/bounds.rs` (one arm). HOT FILES: `fig.rs`, `fig_export.rs`, `rest_import.rs` (one at a time). |

Code baseline: `origin/main@0e8a9a5`. Evidence: REA 2026-10-19 (Figma fullscreen wasm shader source; plugin typings 1.142.0; kiwi schema in `crates/designos-fig/src/figma_schema.txt`). Formulas below are written in the spec's own words; no Figma code or shader text is reproduced.

## 1. Summary and user story

As a designer, I want a progressive blur (a blur whose strength ramps along a line, for a fade-to-blur header or footer) on a layer and on the background behind it, imported from Figma and exported back, rendered the same on the CPU reference and the GPU canvas.

## 2. Current state (FACT)

- Blur is uniform only: `LayerBlur { radius, visible }` and `BackgroundBlur { radius, visible }` (`designos-doc/src/paint.rs:312-319`).
- CPU layer blur: the node is drawn into an isolated layer and `blur_rect` blurs it with a Gaussian of standard deviation radius x scale / 2 (`lib.rs:346-376`; `effects.rs:206-220`, sigma set at `lib.rs:372`). Several layer blurs on a node take the largest radius (`lib.rs:348-355`; GPU `compile.rs:769-777`).
- CPU background blur: `background_blur` copies the backdrop, blurs it with the same Gaussian with exact edges, masks it to the node shape and replaces the backdrop inside the shape (`effects.rs:175-203`, sigma `:176`). The Gaussian itself is `effects.rs:121` for the shadow case (same rule, radius / 2).
- GPU: layer blur is `Post::Blur { sigma, clamp }` on the layer program (`compile.rs:787-815`), background blur is a backdrop op (`compile.rs:1350`); the shader is a separable Gaussian with a uniform sigma (`shaders/blur.wgsl`, `BlurParams { dir, size, radius, clamp_edges, sigma }`). `Post` has `Blur` and `Displace` only (`frame.rs:233-239`).
- Import and export drop the fields: `convert_effect` copies `radius` but not `blurOpType`, `startRadius`, `startOffset` or `endOffset` (`fig.rs:2533-2536`, no such keys); `parse_effect` builds `LayerBlur { radius, visible }` (`rest_import.rs:120-121`); `encode_effect` writes `FOREGROUND_BLUR` / `BACKGROUND_BLUR` with `radius` and `visible` only (`fig_export.rs:1906-1907`). A progressive blur imported today becomes a uniform blur at the end radius, silently.
- Effect variable bindings other than colour and effect opacity are not read (`fig.rs:1722-1741`), so the bindings of the new fields are dropped like those of `radius` already are.
- There is no import report or export warnings channel on `main`: `import_fig` returns `Result<Document, FigError>` (`fig.rs:206`) and `export_fig` returns `Result<Vec<u8>, FigError>` (`fig_export.rs:115`). v0.6's #5 specifies an `ImportReport` returned by `import_fig_with_report` (`docs/specs/v0.6/5-import-global-clone-budget-and-review-backlog.md`, rule 5; not on `main` yet).
- Panel: the blur popover has one row, "Blur" (`panels/effects.rs:304-318`, `:371-380`); the add menu lists seven kinds (`KINDS` `:33-41`).
- Export bounds: layer blur adds its radius to every side (`designos-export/src/bounds.rs:60-66`).
- FACT (kiwi schema, `Message Effect`): `blurOpType = 19` (enum `BlurOpType`: NORMAL 0, PROGRESSIVE 1), `radius = 4` (the end radius), `startRadius = 21`, `startOffset = 20` and `endOffset = 28` (both `Vector`, so these are field ids, not values), variable bindings `startRadiusVar = 45`, `startOffsetXVar = 46`, `startOffsetYVar = 47`, `endOffsetXVar = 48`, `endOffsetYVar = 49`.
- FACT (REA 2026-10-19, plugin typings 1.142.0): the plugin shape is `{blurType, radius, startRadius, startOffset, endOffset}`; the offsets are in normalized object space, (0, 0) the top-left of the layer's box and (1, 1) its bottom-right. This closes the issue's note "offset units unverified".
- FACT (REA 2026-10-19, Figma fullscreen wasm shader source): the ramp parameter is the clamped projection of the pixel onto the line from start to end; the radius count is the start count plus (end count - start count) x that parameter, held constant outside the offsets; if start equals end the start radius applies everywhere; the count has a floor of 0.001.
- FACT (same source): the kernel is a separable tent filter, not a Gaussian: weight at integer offset k is the larger of (count - |k|) and 0, so the centre weight equals the count; it runs horizontally then vertically with bilinear tap pairing, and the count is evaluated per pixel.
- FACT (same source): in the clamped variant, taps outside the clamp rectangle get weight 0 and the result is renormalised, so edges do not darken.
- FACT (same source): the blurred result is mixed with the sharp source by the count b: for b below 1 the output is the sharp source; otherwise the blurred share is (b^2 - 1)^2 / ((b^2 - 1)^2 + b^2) and the sharp share is the rest (0 at b = 1, 0.69 at b = 2, 0.88 at b = 3; I recomputed 0.6923 and 0.8767).
- INFERENCE (same source): the layer blur uses the unclamped variant and spills outside the node; the background blur uses the clamped variant over the backdrop rectangle, masked to the node shape.
- INFERENCE: the count is about the radius times the device scale, i.e. the tent half-width equals the radius (the analytic shadow shader normalises by the blur radius over a support of [-1, 1]).
- UNKNOWN: the exact radius-to-count mapping. Closing check: owner-manual live Figma read (below): a hard vertical edge blurred at radius 10, 20 and 40, fitted against the tent model.
- UNKNOWN: the default start radius and offsets of a new progressive blur; the UI labels of the type switch; how two layer blurs on one node combine. Closing check: the same live read (add the effect through the UI and read the stored values).
- UNKNOWN: whether the REST JSON names match the typings (`blurType`, `startRadius`, `startOffset`, `endOffset`). Closing check: the first REST export of a file containing a progressive blur.

## 3. Behaviour rules

1. **Model.** A blur effect (layer or background) is uniform or progressive. A progressive blur has a start radius, an end radius (the existing `radius`), a start point and an end point in normalized object space.
2. **Ramp.** For a pixel at normalized position p, the parameter is t = clamp((p - s) . (e - s) / |e - s|^2, 0, 1). The blur count is c(t) = max(0.001, c_start + (c_end - c_start) x t), with c_start the count of the start radius and c_end the count of the end radius. If s = e, c = c_start everywhere.
3. **Radius to count.** One function `blur_count(radius, device_scale)` is the only place that knows the mapping. Until the calibration, it returns radius x device_scale (INFERENCE above).
4. **Kernel.** Progressive blur is a separable tent filter, horizontal then vertical, with the per-pixel count of rule 2: tap weights max(c - |k|, 0), normalised to sum 1. Uniform layer blur, background blur and shadows use the same tent kernel and count mapping (owner ruling 2026-10-19).
5. **Edges.** Layer blur: nothing is clamped; pixels outside the layer count as transparent and the result spills past the node box. Background blur: taps outside the backdrop rectangle get weight 0 and the weights are renormalised, then the result is masked to the node shape and replaces the backdrop inside it, as today (`effects.rs:196-202`).
6. **Crossfade.** The output is the blurred share w(b) x blur + (1 - w(b)) x source, with b = c, w(b) = 0 for b < 1, else (b^2 - 1)^2 / ((b^2 - 1)^2 + b^2).
7. **Where it applies.** Layer blur: to the node's own rendered content including children (as today's `has_kids && !clips` rule, `lib.rs:374`). Background blur: to what is painted behind the node.
8. **Combining.** Uniform layer blurs keep the "largest radius wins" rule (`lib.rs:348-355`). A progressive layer blur is applied as its own pass after the uniform one (UNKNOWN in Figma, see section 2; the PR records the choice).
9. **Round trip.** `.fig` and REST carry all five fields; effect styles carry progressive blur; a file with `blurOpType` NORMAL or absent loads as uniform.
10. **Variable bindings** of the new fields are not resolved (non-goal) and are counted in `ImportReport.dropped_variable_bindings` (section 5), not dropped silently.

## 4. Data model / schema changes

- `ProgressiveBlur { start_radius: f32, start: Vec2, end: Vec2 }` in `designos-doc/src/paint.rs`; `progressive: Option<ProgressiveBlur>` on `LayerBlur` and `BackgroundBlur`, `#[serde(default, skip_serializing_if = "Option::is_none")]`. A uniform blur serialises exactly as today.
- As in #161, add the field in a mechanical first commit (every construction site gets `progressive: None`, patterns get `..`), then the behaviour commit.
- No change to `Document` or `Node`.

## 5. Contracts

- Old `.designos` files and old Studio: an old reader ignores the key; a new reader treats a missing key as uniform.
- `.fig`: `blurOpType` (19), `startRadius` (21), `startOffset` (20), `endOffset` (28) written only for a progressive blur; `radius` (4) keeps meaning "end radius" for a progressive one and "the radius" for a uniform one. Round trip through `convert_effect` (`fig.rs:2525`) and `encode_effect` (`fig_export.rs:1887`) is the acceptance seam (issue amendment).
- REST: plugin names `blurType`, `radius`, `startRadius`, `startOffset`, `endOffset` (INFERENCE that REST matches; see section 2).
- Import report: `ImportReport.dropped_variable_bindings: usize` counts the bindings above. It lives on the `ImportReport` of v0.6's #5; if #5 has not landed when this task starts, this task adds `import_fig_with_report` and the struct in the shape #5's rule 5 gives (`import_fig` stays a wrapper) and #5 rebases onto it (README-part2).
- Export bounds: `own_overflow` adds an arm so a progressive layer blur reaches the larger of the start and end radius on every side (INFERENCE; the uniform blur rule is `bounds.rs:60-66`). Lands after #528.
- GPU: `Post` gains a progressive variant; the 64-program cap and the VRAM budget of CLAUDE.md section 4 count it like `Post::Blur`.

## 6. File-format mapping

| Studio | Kiwi (`Effect`) | REST / plugin |
|---|---|---|
| uniform `LayerBlur` | type FOREGROUND_BLUR, `radius` (4), `blurOpType` NORMAL or absent | `LAYER_BLUR`, `blurType` NORMAL |
| uniform `BackgroundBlur` | type BACKGROUND_BLUR, `radius` (4) | `BACKGROUND_BLUR` |
| `progressive` present | `blurOpType` PROGRESSIVE (19), `startRadius` (21), `startOffset` (20), `endOffset` (28) | `blurType` PROGRESSIVE, `startRadius`, `startOffset`, `endOffset` |
| `radius` of a progressive blur | `radius` (4), the end radius | `radius` |
| not carried | `startRadiusVar` (45), `startOffsetXVar` / `YVar` (46, 47), `endOffsetXVar` / `YVar` (48, 49) | counted in `ImportReport.dropped_variable_bindings` |

## 7. UI

Words only. Checked against `design/principles.md` (P3, P4, P5, P6, P10) and `design/soul.md`; tokens from `crates/designos-app/src/theme.rs`.

- The Layer blur and Background blur popovers gain a type switch as their first row, "Type", a segmented control with two tiles ("Uniform", "Progressive"; wording UNKNOWN, see the open question), 24 px high as segmented strips are. The existing "Blur" row follows and is relabelled "End blur" only when progressive is on.
- When progressive is on, rows follow in this order: "Start blur", "End blur", "Start X", "Start Y", "End X", "End Y". Numbers use the shared numeric field (`CONTROL_STANDARD` 28 px, `font_mono`); the four position fields show a percent of the layer box with the `%` suffix; blur values have no suffix (as the existing blur row). Row count of the popover becomes 2 (uniform) and 7 (progressive) in `effect_rows`.
- Switching to progressive keeps the radius as the end blur and writes the defaults of rule: start blur 0, start point (50%, 0%), end point (50%, 100%) (Studio's own default, ASSUMPTION; the Figma default is UNKNOWN). Switching back drops the extra fields (one undo step restores them).
- No canvas handle for the ramp in this task (non-goal; open question 3). No count in a label; no new colour or size literal. The effect section row keeps its present look; the kind dropdown still says "Layer blur" or "Background blur".
- Desktop capture at 1440 in the PR: the popover with progressive on, and a fade-to-blur header over a photo, before and after. Responsive: SKIPPED (owner default). Load es-designer before UI code; update Figma from the verified capture first.

## 8. Edge cases

- Start equals end: uniform at the start radius (rule 2).
- Reversed points (start after end along the axis): the ramp runs the other way; no special case.
- End radius 0 or start radius 0: count floor 0.001 gives the sharp source where b is below 1.
- Both radii below the count that gives any blur (b below 1 everywhere): no visible effect, no GPU pass allocated.
- A diagonal ramp: t is the projection onto the diagonal in normalized space, so a non-square layer skews the iso-lines (as Figma's normalized offsets imply; INFERENCE).
- Rotated or flipped nodes: the ramp is in the node's own object space, so it rotates with the node; the blur kernel itself runs in device pixels.
- Zoom: counts scale with the device scale; at low zoom the count falls below 1 and the effect vanishes smoothly.
- Hidden effect: no work. A progressive blur on a node with `clips_content`: as today, the blur covers the node box only (`lib.rs:374`).
- Tiles: the layer padding is the larger count, so a tile render equals the full render (guard).
- Texture or noise on the same node: unchanged order (blur after the node body, as today).

## 9. Non-goals

- Canvas handles for the start and end points.
- Variable bindings of the new fields.
- Progressive blur on shadows, glass or noise.
- Motion blur or any other `BlurOpType` beyond NORMAL and PROGRESSIVE.

## 10. Test plan (red first)

Seams: CPU pixel tests in `designos-render/tests/` (new file `progressive_blur.rs`), CPU = GPU in `designos-gpu/tests/fidelity.rs` (tolerance 8, strict mismatch under 2%), round trip in `designos-fig/tests/` (pattern of `fig_effect_grid_styles.rs`).

| Case | Expected | Fails before |
|---|---|---|
| `blur_count_function_is_the_only_mapping` | `blur_count(10, 2.0) == 20.0` until calibration | yes |
| `ramp_parameter_is_clamped_projection` | t is 0 at and before the start, 1 at and after the end, 0.5 midway, for a vertical, a horizontal and a diagonal line | yes |
| `equal_start_and_end_use_the_start_radius_everywhere` | uniform output | yes |
| `tent_weights_for_count_2_5` | normalised taps -2..2 are 0.0769, 0.2308, 0.3846, 0.2308, 0.0769 (+/- 1e-4) | yes |
| `crossfade_weight_table` | w(0.5) = 0, w(1) = 0, w(2) = 0.6923, w(3) = 0.8767 (+/- 1e-4) | yes |
| `layer_ramp_is_monotonic_along_the_axis` | a hard edge placed across the ramp: edge width never decreases from start to end (CPU) | yes |
| `layer_blur_spills_outside_the_node_box` | non-zero alpha outside the box on the blurred side | yes |
| `background_blur_clamped_edge_does_not_darken` | a white backdrop region touching the backdrop border keeps 255 at the border | yes |
| `background_blur_is_masked_to_the_node_shape` | pixels outside the shape equal the unblurred backdrop | yes |
| `cpu_and_gpu_agree_for_layer_and_background_progressive_blur` | within the fidelity tolerance | yes |
| `tile_render_equals_full_render_for_a_progressive_blur` | identical | guard |
| `fig_round_trip_keeps_progressive_blur_fields_for_layer_and_background_and_styles` | five fields survive import, export, re-import, also inside an effect style | yes |
| `fig_normal_blur_op_type_loads_uniform` | no `progressive` | guard |
| `rest_round_trip_keeps_progressive_blur` | `blurType`, `startRadius`, offsets survive | yes |
| `old_designos_file_loads_uniform_and_saves_unchanged` | JSON identical | guard |
| `progressive_blur_import_counts_dropped_variable_bindings` | the report counter is 2 for a fixture with two bound fields | yes |
| `own_overflow_adds_progressive_reach` | bounds grow by the larger radius | yes (after #528) |
| `type_switch_row_edits_through_edit_nodes` (app) | one undo step; switching back restores via undo | yes |
| `zero_fake_ui_gate` | green | guard |

Literal numbers come from the model in section 3; the `blur_count` expectations change only when the calibration below says so (the test then moves with the one function).

Run: `cargo test -p designos-doc`, `cargo test -p designos-render`, `cargo test -p designos-gpu --test fidelity`, `cargo test -p designos-fig`, `cargo test -p designos-app effects`. Evidence: red/green capture, the CPU and GPU renders of a vertical ramp over a stripe pattern, the 1440 popover capture.

Owner-manual check (flagged, never claimed by an agent): **live Figma read.** In a scratch Figma file: (1) a hard black/white vertical edge with a uniform layer blur at radius 10, 20 and 40, exported at 1x and 2x (closes the radius-to-count mapping and the uniform kernel question); (2) a progressive layer blur and a progressive background blur created through the UI, with the stored start radius, offsets and defaults read through the plugin API (closes the defaults, labels and REST names); (3) a node with two layer blurs (closes the combining rule).

## 11. Performance budgets

- CPU reference: cost is the region pixels times the count per pass; the region cap of 48 million pixels (`effects.rs:16`) stays; a count above 64 uses a half-resolution pass (FACT, REA 2026-10-19, Figma fullscreen wasm shader source: Figma renders large radii on a downscaled tile; the threshold is UNKNOWN, so Studio chooses it by measurement and records it in the PR).
- GPU: two post passes (horizontal, vertical) per progressive blur; counted against the 64-program cap and the VRAM refine budget (512 MiB default); a count above a threshold measured on the reference machine (start at 24, recorded in the PR) is evaluated on a half-resolution copy. Measured with the existing GPU perf fixture, not averaged.
- Idle: 0.00% CPU; a progressive blur adds no timer and no `app.changed()`; a pan or zoom does not rebuild the ramp (it is analytic).
- Frame budget while dragging a handle value in the popover: one extra pass, within the 8 ms budget on the reference machine (measured and recorded).

## 12. Dependencies and build order

Hot-file queue position 18 (issue text): after #38 (position 17), before #39 (position 19). Order inside the task: model plus mechanical commit; CPU render; GPU render; `.fig` and REST; panel; bounds arm (after #528). Render files: before #161 and #520's fix (README-part2).

## 13. Owner rulings

- Owner ruling 2026-10-19: every blur moves to Figma's tent kernel: progressive blur, uniform layer blur, background blur and drop/inner shadows, on CPU and GPU. The radius-to-count mapping comes from the calibration step (live Figma read) and is shared by all of them. CPU and GPU golden images are re-baselined in the same PR, with a before/after diff attached.

- Acceptance (issue): the model and panel controls work (screenshot); CPU and GPU parity; round trip through `.fig` kiwi (`convert_effect` and `encode_effect`), effect styles carry progressive blur, and the offset units are recorded (now FACT: normalized).
- Delivery law 1: bindings of the new fields are counted, not dropped quietly.
- CLAUDE.md section 4: interactive paths stay gated by `interactive()`, programs capped at 64, idle 0.00%.

## Open questions for the owner (recommended first)

1. Run the live Figma read (three items above, about 30 minutes). Recommended: yes; it also closes #520's kernel question.
2. Should uniform blur and shadows move from the Gaussian (radius / 2) to the tent kernel after the calibration? Recommended: not in this task; keep progressive on the tent and decide for uniform when the fit shows a visible difference.
3. Canvas handles for the ramp start and end. Recommended: a follow-up task; numeric fields in percent ship now.
4. Wording of the type switch: "Uniform" / "Progressive". Recommended: yes, until the live read shows Figma's labels.

## 14. Facts resolved by REA 2 (2026-10-19)

Static reading of Figma's fullscreen wasm and app bundle. No Figma code was copied. Where a line above says UNKNOWN or INFERENCE and this section says FACT, this section wins.

- FACT: the blur count is measured in device pixels of the blur tile and is capped at 64 taps.
- FACT: R_px = radius × the node-to-canvas scale; for a non-uniform scale, use the larger axis.
- FACT: for large radii the count is compressed by 1 / (1 + 2·R_px·G/W), and the tile is downscaled by count / R_px.
- FACT: in a progressive blur the end with the larger radius takes the count; the other end takes count × minR/maxR, linear in radius.
- FACT: layer blur and background blur use one shared tile-blur path. Rectangle shadows use a separate analytic shader that takes the blur radius directly.
- INFERENCE: below compression, `count = min(R·scale, 64)`. UNKNOWN: the tile constants W and G, so the radius where compression starts. Closing check: calibrate on live Figma.
- FACT: a new progressive blur defaults to start radius 0 and runs from (50%, 0%) to (50%, 100%). On a LINE node it runs from (0, −0.5) to (1, −0.5). Both radii are capped at 100.
