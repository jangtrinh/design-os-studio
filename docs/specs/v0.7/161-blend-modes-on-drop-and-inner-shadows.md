# #161 Blend modes on drop and inner shadows (E5-1)

| Field | Value |
|---|---|
| Issue | #161 (task E5-1; parent #18; Size S / P1 in the issue; raised to M here, see below) |
| Size | M (the field is small; 51 files construct or match the shadow variants, 3 render paths composite a shadow, one panel gains a row) |
| Depends on | hot-file queue position 19 (#39) merged first (section 12); no other task |
| Unblocks | #520 (its knockout fix lands on the same composite code) |
| Touches | `crates/designos-doc/src/paint.rs` (`Effect::DropShadow` / `InnerShadow` `:296-310`), `crates/designos-render/src/effects.rs` (`:135`, `:168`), `crates/designos-render/src/shadow.rs` (`:285`), `crates/designos-render/src/lib.rs` (`:446-453`, `:474-478`), `crates/designos-gpu/src/compile.rs` (`:962-966`, `:1293`, `inner_shadow` `:1297-1348`), `crates/designos-fig/src/fig.rs` (`:2533-2536`), `crates/designos-fig/src/fig_export.rs` (`encode_effect` `:1887-1960`), `crates/designos-fig/src/rest_import.rs` (`parse_effect` `:109-159`), `crates/designos-fig/src/export.rs` (`:94`), `crates/designos-app/src/panels/effects.rs`, `crates/designos-app/src/widgets/picker.rs` (shared blend dropdown), and the 51 files that construct or match the shadow variants. HOT FILES: `fig.rs`, `fig_export.rs`, `rest_import.rs` (one at a time). |

Code baseline: `origin/main@0e8a9a5`. Evidence: REA 2026-10-19 (Figma kiwi schema in `crates/designos-fig/src/figma_schema.txt`; plugin typings 1.142.0; Figma fullscreen wasm shader source for the blend composite). The issue body says "Pack: v0.6 Effects & Export"; the milestone is v0.7 Effects & Export (conflict noted in README-part2).

## 1. Summary and user story

As a designer, I want a drop shadow or an inner shadow to carry a blend mode (Multiply, Overlay, ...) as in Figma, so a "multiply" shadow darkens what lies under it, and the mode survives a `.fig` and REST round trip.

## 2. Current state (FACT)

- Shadows have no blend field: `Effect::DropShadow { color, offset, radius, spread, visible, show_behind }` and `InnerShadow { color, offset, radius, spread, visible }` (`designos-doc/src/paint.rs:296-310`). `NoiseEffect` has one (`blend`, `:353-364`).
- CPU: a drop shadow is composited source-over (`effects.rs:135`; the analytic fast path `shadow.rs:285`), an inner shadow likewise (`effects.rs:168`).
- GPU: the shadow is drawn as a `CompositeOp` with `mode: CompMode::Over` (`compile.rs:1293`); the inner shadow similarly in `inner_shadow` (`compile.rs:1297-1348`). `CompMode` already has `Blend(BlendMode)` that reads a snapshot of the destination (`designos-gpu/src/frame.rs:169-177`). `DrawBlend` has only `Over` and `Erase` (`frame.rs:124-129`).
- CPU layer blend exists and is reusable: `blend_mode()` maps a document mode to tiny-skia (`designos-render/src/paint.rs:16-45`), linear burn and linear dodge go through `custom_blend` and `blend_layer` (`paint.rs:52-63`); `Normal` and `PassThrough` both draw source-over (`paint.rs:19`).
- Import: kiwi to REST-shaped JSON already copies a `blendMode` key on every effect (`fig.rs:2533-2536`), but `parse_effect` never reads it for shadows (`rest_import.rs:118-119`), `encode_effect` never writes it for shadows (`fig_export.rs:1889-1905`), and the REST exporter writes the constant `"NORMAL"` (`export.rs:94`). `rest_import::blend` maps unknown names, including UNSET, to Normal (`rest_import.rs:43-66`); `blend_name` writes the names (`fig_export.rs:2344`).
- Panel: the shadow settings popover has 6 rows (drop) and 5 (inner): X, Y, Blur, Spread, Color, and Show behind (`panels/effects.rs:304-318`, `:330-370`). The paint picker has a private 16-mode blend dropdown (`widgets/picker.rs:1294`).
- Blast radius: `Effect::DropShadow {` appears at 81 sites in 51 files (counted by search on this commit), mostly tests and fixtures.
- FACT (kiwi schema): `Effect.blendMode = 6`, enum `BlendMode` PASS_THROUGH 0, NORMAL 1, DARKEN 2, MULTIPLY 3, LINEAR_BURN 4, COLOR_BURN 5, LIGHTEN 6, SCREEN 7, LINEAR_DODGE 8, COLOR_DODGE 9, OVERLAY 10, SOFT_LIGHT 11, HARD_LIGHT 12, DIFFERENCE 13, EXCLUSION 14, HUE 15, SATURATION 16, COLOR 17, LUMINOSITY 18, UNSET 19 (`figma_schema.txt`, `Message Effect`, `Enum BlendMode`).
- FACT (REA 2026-10-19, plugin typings 1.142.0): the default blend mode of a shadow is NORMAL.
- INFERENCE (REA 2026-10-19, Figma fullscreen wasm shader source): a drop shadow blends with the backdrop behind the layer, and an inner shadow blends with the layer's own content; the blend composite is a separate step applied last.
- UNKNOWN: whether PASS_THROUGH and UNSET are valid for a shadow (probably not). Closing check: owner-manual live Figma read (set a shadow's blend through the plugin API and read it back).
- UNKNOWN: inside an isolated layer (opacity below 1, a blended parent), whether Figma blends the shadow with the layer's content so far or with the page behind. Closing check: the same live read, one frame with a 50% opacity parent.

## 3. Behaviour rules

1. **Model.** Each drop shadow and inner shadow has a blend mode, default Normal. The field name is `blend`, the same as `NoiseEffect.blend`.
2. **Composite step.** The shadow is rendered as today into its own region, then composited with the blend mode: a drop shadow against what is already painted under the node (the backdrop of this layer), an inner shadow against the node's own content just drawn (the fill, drawn before the inner shadow at `lib.rs:473-478`).
3. **Normal and PassThrough** composite source-over (today's behaviour, byte for byte). PassThrough is stored verbatim for round trip and drawn as Normal.
4. **Other modes** use the same math as layer blends: the 14 tiny-skia modes through `blend_mode()`, linear burn and linear dodge through `blend_layer`, on the CPU; `CompMode::Blend` on the GPU.
5. **Old files** (no field) are Normal. A file whose value is UNSET is read as Normal and written back as NORMAL (the only value that changes; UNSET means "no value", so nothing is lost, INFERENCE).
6. **Round trip.** Kiwi `blendMode` and the REST `blendMode` on shadows are read and written with the existing `blend()` and `blend_name()` mappings. The REST exporter stops writing the constant NORMAL (`export.rs:94`) and writes the stored mode.
7. **Effect styles** carry the mode (they hold `Vec<Effect>`).
8. **Show behind** and the knockout (`effects.rs:129-134`) apply before the blend: the knocked-out shadow is what gets blended.
9. **The mode applies to the shadow only**, never to the node: the node's own blend (layer blend) is unchanged.

## 4. Data model / schema changes

- `Effect::DropShadow` and `Effect::InnerShadow` gain `blend: BlendMode` with `#[serde(default)]` (default Normal) and `skip_serializing_if` default, so `.designos` files written before this change load unchanged and files without a blend stay byte-identical when saved.
- Land the field in two commits: first a mechanical commit that adds `blend: BlendMode::Normal` at every construction site and `..` at every pattern (no behaviour change, compiles everywhere), then the behaviour commit. This keeps the hot-file diffs small and the 51-file churn out of the review of the real change.
- No change to `Document` or to the node struct.

## 5. Contracts

- `.designos` JSON: one optional key per shadow, additive. Old Studio reads a new file by ignoring the key (serde unknown-field tolerance is not assumed: check that `Effect` is not `deny_unknown_fields`; it is not on this commit).
- `.fig`: `blendMode` field 6 on the effect message, existing enum names.
- REST: `blendMode` on the effect, existing names.
- The MCP tools and `apply_spec` keep their shapes: a shadow without `blend` is Normal (`designos-doc/src/spec.rs:239` and `designos-mcp/src/engine.rs:901` get the default only).

## 6. File-format mapping

| Studio | Kiwi (`Effect`) | REST / plugin |
|---|---|---|
| `DropShadow.blend`, `InnerShadow.blend` | `blendMode` (6), enum `BlendMode` | `blendMode` on the effect |
| default Normal | absent or NORMAL (1) | absent or NORMAL |
| PassThrough | PASS_THROUGH (0), kept and drawn as Normal | PASS_THROUGH |
| UNSET (19) | read as Normal, written as NORMAL | UNSET read as Normal |

## 7. UI

Words only. Checked against `design/principles.md` (P3 spacing ladder, P4 concentric radii, P5 type roles, P6 Zero Fake UI, P10 inheritance) and `design/soul.md`.

- The shadow settings popover gains one row, "Blend", under "Color" for both drop and inner shadows (rows become 7 and 6; update `effect_rows`). The row holds a dropdown at `CONTROL_STANDARD` (28 px) listing the same modes as the paint picker, Normal first, without Pass through. The popover and the effect-style editor share the row (`FxTarget::Nodes` and `FxTarget::Style`).
- The dropdown is the paint picker's blend dropdown extracted into a shared widget (it takes a `BlendMode`, not a `Paint`); the picker keeps its behaviour. No new colour or size literal: tokens from `theme.rs` only.
- The row label is the word "Blend" in body type; the current mode name is the dropdown's value. No count in a label.
- Nothing is added to the effect row in the section list: a non-Normal blend is visible in the popover only.
- Desktop capture at 1440 in the PR: the popover open on a Multiply drop shadow, and the canvas showing a darkened backdrop under it. Responsive: SKIPPED (owner default). Load es-designer before UI code; update Figma from the verified capture first.

## 8. Edge cases

- Blend inside an isolated layer (opacity under 1, parent blend): blends against the layer content painted so far, because the shadow is drawn into the layer target. Figma may differ (UNKNOWN above); the PR notes it.
- A shadow blend with a Multiply mode and a transparent backdrop: the result is the shadow colour at its alpha (blend math with `ab = 0` reduces to source-over).
- Multiple shadows with different modes: each composites in list order, as today.
- Noise effect `blend` is untouched (`NoiseEffect.blend`).
- Hidden shadow: no composite, no cost.
- Inner shadow with Show behind: not applicable (inner shadows have no such flag).
- Text nodes: the shadow silhouette is the glyph paths; the blend applies the same way.
- The 64-program cap in `interactive()` mode (CLAUDE.md section 4): a blended GPU shadow costs one snapshot plus the existing programs; past the cap the shadow falls back to source-over, not to nothing.

## 9. Non-goals

- Blend modes on blur, glass or noise composites (noise already has one).
- Changing the shadow shape, kernel or knockout (#520, #37).
- A per-shadow opacity separate from the colour alpha.
- Variable binding for the blend mode.

## 10. Test plan (red first)

Seams: pixel tests in `designos-render/tests/render.rs`, CPU = GPU in `designos-gpu/tests/fidelity.rs`, round trips in `designos-fig/tests/` (the pattern of `fig_effect_grid_styles.rs`).

Literal pixel for the first acceptance criterion: a backdrop rectangle `#C8C8C8`; a node 40 x 40 at (10, 10); a drop shadow colour `#808080` at alpha 1, offset (20, 0), blur 0, spread 0, Multiply. At the pixel (60, 30), outside the node (the node ends at x = 50) and inside the shadow (x 30 to 70), Normal gives 128 and Multiply gives 200 x 128 / 255 = 100.4, so 100 within 1.

| Case | Expected | Fails before |
|---|---|---|
| `multiply_drop_shadow_darkens_the_backdrop_at_a_literal_pixel` | (60, 30) is 100 +/- 1 (Normal would be 128) | yes (the field does not exist) |
| `multiply_inner_shadow_blends_with_the_node_content` | node fill `#C8C8C8`, inner shadow `#808080` hard-edged inside; a pixel in the shadowed band is 100 +/- 1 | yes |
| `normal_default_is_byte_identical_to_today` | render of a fixture without the field equals the previous render | guard |
| `cpu_and_gpu_agree_on_multiply_screen_overlay_shadows` | within the fidelity tolerance (tolerance 8, strict mismatch under 2%) | yes |
| `linear_burn_and_linear_dodge_shadows_use_the_custom_blend` | CPU matches `blend_layer`; GPU matches CPU | yes |
| `fig_round_trip_keeps_shadow_blend` | kiwi `blendMode` MULTIPLY on a drop and an inner shadow survive import, export, re-import, including inside an effect style | yes |
| `rest_round_trip_keeps_shadow_blend` | REST export no longer writes NORMAL for a Multiply shadow | yes |
| `old_designos_file_without_blend_loads_as_normal_and_saves_unchanged` | JSON identical after load and save | guard |
| `pass_through_is_kept_and_drawn_as_normal` | stored value PassThrough, pixels equal Normal | yes |
| `unset_reads_as_normal_and_writes_normal` | NORMAL after export | yes |
| `blend_row_changes_the_effect_through_edit_nodes` (app test) | one undo step, style-linked layer behaves as for other rows | yes |
| `zero_fake_ui_gate` | `python3 scripts/zero_fake_ui_gate.py` green | guard |

Run: `cargo test -p designos-doc`, `cargo test -p designos-render`, `cargo test -p designos-gpu --test fidelity`, `cargo test -p designos-fig`, `cargo test -p designos-app effects`. Evidence: red/green capture, the CPU and GPU renders of the literal-pixel fixture, the 1440 popover capture.

Owner-manual check (flagged, never claimed by an agent): **live Figma read** for the two UNKNOWNs in section 2: set a Multiply drop shadow and an Overlay inner shadow through the plugin API on a layer inside a 50% opacity frame, export at 1x, and read back the stored `blendMode` for PASS_THROUGH.

## 11. Performance budgets

- Normal shadows: zero added cost (the code path is unchanged when the mode is Normal).
- Blended shadow, GPU: one extra destination snapshot per blended shadow; counted against the 64-program cap and the VRAM budget; measured with the existing `refine_vram_budget` test fixture.
- Blended shadow, CPU: one extra `draw_pixmap` with a blend mode over the shadow region (already a region-sized pixmap), no extra allocation.
- No change to idle CPU (0.00%); no `app.changed()`.

## 12. Dependencies and build order

Hot-file queue position 20 (issue text): claim only after position 19 (#39) merged. Within v0.7 part 2 the order on `fig.rs`, `fig_export.rs`, `rest_import.rs` is #38, #37, #39, #161, #163, #521 (README-part2). The mechanical "field with default everywhere" commit touches non-hot files and may be pushed as its own PR early, rebased on the last merge. Render files: land after #37, before #520's fix.

## 13. Owner rulings

- Acceptance (issue): a Multiply shadow darkens the backdrop at a literal pixel; `.fig` and REST round trip keep the mode; old files default to Normal.
- Owner ruling R2.8 does not apply here (it is about shaders).
- Delivery law 1: nothing vanishes silently; PASS_THROUGH is kept verbatim, and the one value that changes (UNSET, "no value") is named in rule 5.

## Open questions for the owner (recommended first)

1. PASS_THROUGH on a shadow: keep it verbatim and draw as Normal (this spec), or normalise to Normal on import. Recommended: keep verbatim; it is lossless and costs nothing.
2. Run the live Figma read for the isolated-layer case. Recommended: yes, with the same session as #520's read (one scratch file).
3. Size: the issue says S; the spec says M because of the 51-file churn. Recommended: keep M and use the mechanical-commit split.
