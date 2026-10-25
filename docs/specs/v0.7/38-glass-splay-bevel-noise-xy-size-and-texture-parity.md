# #38 Glass effect parameter parity (splay, bevel) and noise and texture parity (X/Y size, type rules)

| Field | Value |
|---|---|
| Issue | #38 (task; roadmap; full-scope amendment 2026-10-17: kiwi round trip, independent X/Y size, REA E5-b) |
| Size | L (two PRs: slice A model, round trip, panel; slice B render parity) |
| Depends on | #106 (merged: `convert_effect` and `encode_effect` carry noise, texture and glass today); hot-file queue position 16 merged first; slice B waits for the live Figma read of section 10 for numeric parity |
| Unblocks | #37 (queue position 18, slice A only) |
| Touches (slice A) | `crates/designos-doc/src/paint.rs` (`NoiseEffect` `:353-364`, `TextureEffect` `:367-376`, `GlassEffect` `:379-392`), `crates/designos-fig/src/fig.rs` (`convert_effect` `:2525-2568`), `crates/designos-fig/src/fig_export.rs` (`encode_effect` `:1908-1951`), `crates/designos-fig/src/rest_import.rs` (`parse_effect` `:109-159`), `crates/designos-fig/src/export.rs`, `crates/designos-app/src/panels/effects.rs` (`:382-515`), tests in `designos-fig/tests/fig_effect_grid_styles.rs` and `rest_import.rs`. HOT FILES: `fig.rs`, `fig_export.rs`, `rest_import.rs`. |
| Touches (slice B) | `crates/designos-render/src/fx.rs` (`noise` `:70`, `texture` `:167`, `glass` `:211`), `crates/designos-gpu/src/compile.rs` (`glass` `:1367-1394`, `noise` `:1396`, texture displace `:845-858`), `crates/designos-gpu/src/frame.rs` (`GlassParams` `:192-202`), `crates/designos-gpu/src/shaders/fx.wgsl`. No hot file. |

Code baseline: `origin/main@0e8a9a5`. Evidence: REA 2026-10-19 (Figma fullscreen wasm shader source; plugin typings 1.142.0; kiwi schema in `crates/designos-fig/src/figma_schema.txt`). Formulas are in the spec's own words; no Figma code or shader text is reproduced.

## 1. Summary and user story

As a designer importing a Figma file with Glass, Noise and Texture effects, I want every parameter to survive import and export, the panel to show Figma's labels, and the render to follow Figma's rules for splay, bevel, X/Y noise size and the three noise types.

## 2. Current state (FACT)

- Model: `NoiseEffect { mode: Mono | Duo | Multi, size: f32, density, opacity, blend, seed, visible }` (the scalar `size` is at `paint.rs:356`); `TextureEffect { size: f32, radius, clip_to_shape, seed, visible }` (`:369`); `GlassEffect { light_angle, light_intensity, refraction, depth, dispersion, frost, visible }` (`:379-392`). There is no splay, no bevel and no second size.
- Import (`.fig` to REST-shaped JSON): `convert_effect` maps GRAIN to TEXTURE (`fig.rs:2526-2530`), normalises MONO / DUO / MULTI (`:2538-2546`), and collapses a vector `noiseSize` to a single number: `fnum(ns, "x").or_else(|| fnum(ns, "y"))`, so a different Y is lost (`:2547-2553`). The glass loop copies five fields and never `reflectionDistance` or `bevelSize` (`:2554-2566`).
- `parse_effect` reads the same names with the same collapse (`rest_import.rs:115`), builds `Glass` from five fields plus `radius` as frost (`:148-156`) and accepts MONO / DUO / MULTI and the full names (`:126-130`).
- Export: `encode_effect` writes `noiseSize` as `vec2(size, size)` (`fig_export.rs:1911`, `:1936`), the noise type names MONOTONE, DUOTONE, MULTITONE (`:1918-1930`), GRAIN for texture (`:1935`) and five glass fields plus `radius` (`:1943-1949`); never splay or bevel.
- There is no import report or export warnings channel on `main`: `import_fig` returns `Result<Document, FigError>` (`fig.rs:206`) and `export_fig` returns `Result<Vec<u8>, FigError>` (`fig_export.rs:115`). v0.6's #5 specifies an `ImportReport` returned by `import_fig_with_report` (`docs/specs/v0.6/5-import-global-clone-budget-and-review-backlog.md`, rule 5; not on `main` yet).
- The round trip test of #106 exists and uses equal X and Y (`designos-fig/tests/fig_effect_grid_styles.rs:209-300`), so unequal sizes are untested.
- The issue item "Map GRAIN (=6) to Texture on import" is already done on `main` (`fig.rs:2528`, `rest_import.rs:141`); the work left is splay, bevel, X/Y size and the type rules.
- A fill of paint type NOISE (PaintType 9) is a different thing from the noise effect (EffectType NOISE 7) and is dropped by `convert_paint` (`fig.rs:2520`, `_ => return None`). Out of scope here (follow-up in README-part2).
- Render today: noise is a cell grain with a hash value noise (`fx.rs:70-157`, `cell = size x scale`), the glass is a blurred-mask bevel with a Gaussian frost (`fx.rs:211`; GPU `compile.rs:1367-1394`: `sigma_f = frost x scale / 2`, `sigma_m = max(depth / 3, 0.6)`), texture is a seeded value-noise displacement (`fx.rs:167`; GPU `Post::Displace` `frame.rs:238`).
- Panel: glass rows "Light angle", "Intensity", "Refraction", "Depth", "Dispersion", "Frost" (`panels/effects.rs:484-506`); noise rows "Mode", "Color" or "Color 1" and "Color 2", "Size", "Density", "Opacity" (all modes); texture rows "Size", "Radius", "Clip to shape". The popover row counts are fixed in `effect_rows` (`:304-318`). Seed has no control.
- FACT (kiwi schema, `Message Effect`; REA 2026-10-19): glass `refractionRadius` 30, `specularAngle` 31, `specularIntensity` 32, `bevelSize` 33, `chromaticAberration` 34, `reflectionDistance` 35, `refractionIntensity` 36, `radius` 4 (frost); glass variable bindings 37 to 42 (`splayVar` is 41); noise `noiseSize` 22 (a `Vector`), `seed` 29, `clipToShape` 23, `density` 25, `noiseType` 26 (enum MULTITONE 0, MONOTONE 1, DUOTONE 2), `opacity` 27, `color` 2, `secondaryColor` 24, `blendMode` 6; noise bindings 50 to 54; effect types GRAIN 6, NOISE 7, GLASS 8.
- FACT (REA 2026-10-19, plugin typings 1.142.0): plugin names are `depth` (kiwi `refractionRadius`, at least 1), `lightAngle` (degrees, `specularAngle`), `lightIntensity` (0 to 1, `specularIntensity`), `dispersion` (`chromaticAberration`), `refraction` (`refractionIntensity`); `reflectionDistance` (the shader calls it splay) and `bevelSize` have no plugin name (kiwi only).
- FACT (REA 2026-10-19, Figma fullscreen wasm shader source): the rules of sections 3.2 to 3.5 (glass, noise, texture).
- UNKNOWN: the unit of `specularAngle` in kiwi (degrees in the plugin; the shader works in radians). Studio reads and writes degrees today (`rest_import.rs:149`, `fig_export.rs:1944`). Closing check: owner-manual live Figma read (below).
- UNKNOWN: whether kiwi `noiseSize` is the grain size or its inverse, and in which direction X and Y act. INFERENCE (same source): the shader multiplies coordinates by `noiseSize`, so `noiseSize` is the inverse of the size the UI shows. Closing check: the same live read.
- UNKNOWN: defaults of glass, noise and blur (including the default splay, bevel and the density band edge softness). Closing check: the same live read (add each effect through the UI and read the stored values).
- UNKNOWN: the UI label of splay and bevel, and whether bevel is exposed in the UI at all. Closing check: the same read (screenshot of the Glass panel).

## 3. Behaviour rules

### 3.1 Model and round trip (slice A)

1. `GlassEffect` gains `splay: f32` (kiwi `reflectionDistance`) and `bevel: f32` (kiwi `bevelSize`). A file without them loads as 0 (INFERENCE: the schema default; the Figma default is UNKNOWN).
2. `NoiseEffect` and `TextureEffect` keep `size: f32` as the X size and gain `size_y: Option<f32>` (None means equal to X). A scalar `size` in an old `.designos` file loads as X = Y.
3. Import reads `noiseSize` as a vector and keeps X and Y. Export writes `vec2(x, y)`. The conversion between the kiwi value and Studio's pixel size lives in two functions, `noise_size_from_kiwi` and `noise_size_to_kiwi`, identity until the live read decides (so a round trip is lossless under either interpretation).
4. The specular angle conversion likewise lives in one pair of functions, degrees on both sides until the live read decides.
5. Glass fields map: `depth` is `refractionRadius`, `refraction` is `refractionIntensity`, `light_angle` is `specularAngle`, `light_intensity` is `specularIntensity`, `dispersion` is `chromaticAberration`, `splay` is `reflectionDistance`, `bevel` is `bevelSize`, `frost` is `radius`. The doc comment of `depth` (today "Bevel depth in px", `paint.rs:386`) is corrected: it is the refraction radius.
6. Noise `opacity` is kept for every type (lossless) but only the multitone type uses it when rendering (rule 3.3). Noise `blend` (`blendMode` 6), texture `clip_to_shape` and `seed` round trip as today.
7. Effect styles carry all of it. REST keys use the plugin names where they exist (`lightAngle`, `lightIntensity`, `refraction`, `depth`, `dispersion`) and the kiwi names for splay and bevel (`reflectionDistance`, `bevelSize`), as `convert_effect` already does for the five.
8. Variable bindings of fields 37 to 42 and 50 to 54 are not resolved (non-goal) and are counted in `ImportReport.dropped_variable_bindings` (the same counter as #37); they are not dropped quietly.

### 3.2 Glass render (slice B), rounded-rectangle shapes

Let d be the refraction radius, b the bevel size, eh the signed distance from the pixel to the shape edge (negative inside), g the refraction intensity.

1. Edge ramp: ei = clamp((eh + d + b) / d, 0, 1).
2. Edge displacement: el = clamp(ei^3.5 x d x g, -0.999 d, 0.999 d).
3. Bevel-band displacement: em = g x 0.2 x 0.999 d.
4. The two blend with a ramp over eh from -b to -0.3 b.
5. Direction: the edge-distance normal, with corner radii raised to at least max(b, d), mixed toward the radial direction from the shape centre by the splay amount.
6. The frosted backdrop (frost blur) is sampled at the pixel position minus the direction times the blended displacement.
7. Dispersion, when at least 0.001: eight triangular-weighted taps; for u from -0.5 to 0.5 the red channel is sampled at an extra shift of +0.5 + u, green at u, blue at u - 0.5, each times dispersion x direction; alpha is the largest of the three.
8. Specular: a band over eh from -b-0.001 to -0.2 b; the light angle is theta = lightAngle - node rotation + a quarter turn; two highlight terms (one on the side facing the light at full strength, one opposite at 0.6) proportional to the smoothstep of the normal-to-light dot product from 0.35 to 1, times the band times the light intensity; a third term from the direction-to-light dot product over a ramp from -d to -b, squared, times 0.05 times the intensity.
9. Non-rectangular shapes use a height-map variant in Figma (magnitude ((d - 1) / 2)^1.4). Studio keeps its blurred-mask approximation for them in this task (non-goal).

### 3.3 Noise render (slice B)

1. Gradient value noise per colour channel c, seeded with seed + c, evaluated at (pixel - shape centre) x noiseSize.
2. Three octaves, each doubling the frequency and halving the amplitude, with a smoothstep fade; the sum is normalised as (fbm + 1) / 2.
3. A density band is a window of the normalised value centred at 0.25 (the low band) or 0.75 (the high band) with half-width density / 4.
4. Monotone: j is the luma (weights 0.213, 0.715, 0.072) of the channel values; alpha = band(j) x colour alpha x source alpha; colour is the noise colour.
5. Duotone: the low band takes the secondary colour, the high band the primary colour.
6. Multitone: each channel is clamp(2n - 0.5); alpha = band(third channel) x opacity x source alpha.
7. The result is premultiplied and clipped to the source alpha.
8. The hash is float32-sin based, so the target is statistical parity (coverage and spectrum), not bit parity. Existing seeds therefore change their pattern; the seed value itself round trips.

### 3.4 Texture render (slice B)

Texture is a displacement, not an overlay (Studio already displaces: `fx.rs:167`, `Post::Displace`). Two independent noise channels give a vector v and two weights a0 and a1; the pixel is sampled at pos + v x radius x (a0^0.2 x a1^0.2), with X and Y noise sizes independent. The result is clipped to the shape when `clip_to_shape` is on.

### 3.5 Panel labels

The panel shows Figma's labels (section 7).

## 4. Data model / schema changes

- `GlassEffect.splay`, `GlassEffect.bevel` (`#[serde(default)]`, written only when non-zero).
- `NoiseEffect.size_y` and `TextureEffect.size_y: Option<f32>` (`#[serde(default, skip_serializing_if = "Option::is_none")]`).
- Mechanical first commit as in #161: every construction site (tests, `showcase`, `demo`, `check_designs` fixtures, `panels/effects.rs::default_effect` `:68-92`) gets the new fields; then the behaviour.
- No change to `Document` or `Node`.

## 5. Contracts

- `.designos`: additive keys; old files load unchanged and re-save byte-identical (a scalar `size` stays a scalar when `size_y` is None).
- Import report: the counter above lives on the `ImportReport` of v0.6's #5, created by whichever of #37 and #38 starts first if #5 has not landed (same wording as #37 section 5).
- `.fig`: fields 33 and 35 (glass), the full `noiseSize` vector (22) for noise and texture; every effect parameter listed in section 2 survives import, export and re-import (the issue's fixture acceptance).
- REST: as section 3.1 rule 7.
- The CPU renderer stays the reference; the GPU matches it within `designos-gpu/tests/fidelity.rs` tolerance for slice B, except noise, which is held to statistical parity.

## 6. File-format mapping

| Studio | Kiwi (`Effect`) | REST / plugin |
|---|---|---|
| glass `depth` | `refractionRadius` (30) | `depth` |
| glass `light_angle` | `specularAngle` (31) | `lightAngle` |
| glass `light_intensity` | `specularIntensity` (32) | `lightIntensity` |
| glass `bevel` | `bevelSize` (33) | kiwi name only |
| glass `dispersion` | `chromaticAberration` (34) | `dispersion` |
| glass `splay` | `reflectionDistance` (35) | kiwi name only |
| glass `refraction` | `refractionIntensity` (36) | `refraction` |
| glass `frost` | `radius` (4) | `radius` |
| noise type | `noiseType` (26): MULTITONE 0, MONOTONE 1, DUOTONE 2 | MONOTONE, DUOTONE, MULTITONE; MONO, DUO, MULTI accepted |
| noise `size`, `size_y` | `noiseSize` (22) vector x, y | number or `{x, y}` |
| noise `density`, `opacity`, `seed`, `color`, secondary colour, `blend` | 25, 27, 29, 2, 24, 6 | same names |
| texture (GRAIN, type 6) | `radius` (4), `noiseSize` (22), `clipToShape` (23), `seed` (29) | `TEXTURE` / `GRAIN` |
| not carried | bindings 37 to 42 and 50 to 54 | counted in `ImportReport.dropped_variable_bindings` |

## 7. UI

Words only. Checked against `design/principles.md` (P3, P4, P5, P6, P10) and `design/soul.md`; tokens from `theme.rs`.

- Glass popover rows, in order: "Light angle", "Light intensity" (today "Intensity"), "Refraction", "Depth", "Dispersion", "Splay", "Bevel", "Frost". Labels follow the plugin names in sentence case; the splay and bevel labels are UNKNOWN until the live read (open question 2). Row count 6 becomes 8 (`effect_rows`).
- Noise popover: "Mode" (Monotone, Duotone, Multitone), colour rows, "Size", "Density", and "Opacity" only when the mode is Multitone (the other modes keep the stored value but do not show or use it). When X and Y differ the single "Size" row becomes "Size X" and "Size Y" (a stored value is never hidden); when they are equal one field edits both.
- Texture popover: "Size" (or "Size X" and "Size Y" under the same rule), "Radius", "Clip to shape".
- Numeric fields use the shared numeric field (`CONTROL_STANDARD`, `font_mono`); units as today (`%` for 0..1 values shown as percent). No new colour or size literal; no count in a label.
- Desktop capture at 1440 in the PR: the three popovers (glass with splay and bevel, multitone noise with unequal size, texture), and a glass card over a stripe pattern before and after slice B. Responsive: SKIPPED (owner default). Load es-designer before UI code; update Figma from the verified capture first.

## 8. Edge cases

- Bevel 0: the bevel band is empty (treat bevel below 0.001 as none, INFERENCE); the edge ramp uses d alone.
- Depth below 1: clamp to 1 (plugin range, FACT above).
- Splay 0 and splay at the maximum: direction is the pure edge normal and the pure radial direction.
- Very large noise size or zero: clamp to a minimum size so the grain never falls under one pixel per octave (the existing `.max(0.05)` rule, `fx.rs:73`).
- Noise on a node with zero alpha: nothing (clipped to source alpha).
- Texture with `clip_to_shape` off: the export bounds already grow by the radius (`bounds.rs:68-74`).
- Rotated frame: the light angle subtracts the node rotation (rule 3.2.8; INFERENCE for the sign convention), so the highlight stays fixed in page space.
- A `.fig` with glass and no `reflectionDistance`: splay 0.
- Effect style with a glass or noise effect that has unequal X/Y: carried like a node effect.
- Seeds: a seed above `u32::MAX` clamps as today (`rest_import.rs:116`).

## 9. Non-goals

- The height-map glass variant for non-rectangular shapes.
- Variable bindings of the glass and noise fields.
- Paint-type NOISE fills (`PaintType` 9) and PATTERN fills.
- Bit-exact noise.
- A seed control in the panel.

## 10. Test plan (red first)

Seams: round trips in `designos-fig/tests/fig_effect_grid_styles.rs` and `rest_import.rs`; panel logic in `designos-app/src/panels/effects.rs` tests; render tests in `designos-render/tests/` and CPU = GPU in `designos-gpu/tests/fidelity.rs`.

Slice A:

| Case | Expected | Fails before |
|---|---|---|
| `fig_round_trip_keeps_glass_splay_and_bevel` | fields 35 and 33 equal after import, export, re-import, direct and via an effect style | yes |
| `fig_round_trip_keeps_noise_x_and_y_size` | `noiseSize` (3.0, 7.0) survives; today Y is lost | yes |
| `fig_round_trip_keeps_texture_x_and_y_size_clip_and_seed` | same for GRAIN | yes |
| `rest_round_trip_keeps_all_glass_noise_and_texture_fields` | every field of section 6 | yes |
| `both_noise_type_spellings_import_and_export_the_full_name` | MONO and MONOTONE give Mono; export writes MONOTONE | guard |
| `old_designos_noise_with_scalar_size_loads_as_x_equals_y_and_saves_unchanged` | JSON identical | guard |
| `missing_splay_and_bevel_load_as_zero` | 0 and 0 | yes |
| `size_and_angle_conversions_live_in_one_pair_of_functions` | grep: no other `noiseSize` or `specularAngle` arithmetic | yes |
| `dropped_variable_bindings_of_glass_and_noise_are_counted` | counter 3 for a fixture with 3 bound fields | yes |
| `glass_rows_use_the_figma_labels_in_order` (app) | the 8 labels of section 7 in order | yes |
| `noise_opacity_row_only_for_multitone` (app) | row absent for Mono and Duo, value kept | yes |
| `unequal_noise_size_shows_x_and_y_rows` (app) | two rows | yes |
| `zero_fake_ui_gate` | green | guard |

Slice B (numbers from section 3; hand-computed from the rules as written, INFERENCE until the live read):

| Case | Expected | Fails before |
|---|---|---|
| `glass_edge_ramp_and_displacement_table` | d = 10, b = 4, g = 1: eh = -14 gives displacement 0; eh = -9 gives 0.884 (ei = 0.5, 0.5^3.5 x 10); eh = -4 gives 9.99 (clamped to 0.999 d); bevel band displacement 1.998 | yes |
| `glass_splay_zero_is_edge_normal_and_splay_one_is_radial` | direction vectors at a corner | yes |
| `glass_dispersion_below_0_001_has_no_taps` | identical to no dispersion | yes |
| `monotone_alpha_is_band_times_colour_alpha_times_source_alpha` | table on a fixed noise value | yes |
| `multitone_ignores_colour_and_uses_opacity` | rgb is clamp(2n - 0.5), alpha scales with opacity | yes |
| `duotone_low_band_takes_secondary_colour` | colour of a low-band pixel | yes |
| `noise_coverage_rises_with_density_and_is_deterministic_per_seed` | monotone in density; equal for equal seed | yes |
| `texture_displacement_uses_independent_x_and_y_size` | X and Y sizes change the pattern independently | yes |
| `cpu_and_gpu_agree_on_glass_and_texture` | within the fidelity tolerance | yes |
| `cpu_and_gpu_noise_have_matching_coverage` | statistical parity (coverage within 0.03 of density-model) | yes |

Run: `cargo test -p designos-doc`, `cargo test -p designos-fig`, `cargo test -p designos-render`, `cargo test -p designos-gpu --test fidelity`, `cargo test -p designos-app effects`. Evidence: red/green capture, the three popovers at 1440, the glass and noise renders before and after.

Owner-manual check (flagged, never claimed by an agent): **live Figma read.** In a scratch Figma file add Glass, Noise (each of the three types) and Texture through the UI, with the Size field at 2 and then 4, light angle at 0 and 90 degrees, then (1) read the plugin values, (2) export the file as `.fig` and read the stored `specularAngle`, `noiseSize`, `reflectionDistance`, `bevelSize` and the defaults, (3) take a screenshot of each panel for the labels. This closes the angle unit, the noise size direction, the defaults and the labels.

## 11. Performance budgets

- Slice A: no render cost; import and export add a constant number of fields per effect.
- Slice B glass: the SDF evaluation is per pixel of the glass region only (the existing region and pad logic, `compile.rs:1367-1394`); dispersion costs 8 backdrop taps only when it is at least 0.001; the 64-program cap and the VRAM budget (`backdrop_bytes`) are unchanged.
- Noise: three octaves per channel per pixel on the GPU; CPU reference stays the existing region loop; no new allocation per frame.
- Idle: 0.00%; no `app.changed()` on viewport changes.

## 12. Dependencies and build order

Hot-file queue position 17 (issue text), after #106 (merged). Slice A uses the hot files and goes first; #37 follows (position 18). Slice B edits render files only: after #37 and #161 in `compile.rs`, and after the live read. Never run slice A in parallel with #37, #39, #161, #163 or #521 (README-part2).

## 13. Owner rulings

- Acceptance (issue): a fixture round trip keeps all parameters; the panel shows Figma labels; screenshot.
- Amendment 2026-10-17: independent X/Y noise and texture size; the kiwi round trip names splay and the noise and texture fields; effect styles carry them.
- Delivery law 3 and 6: the angle unit and the noise size direction are hypotheses with one conversion function each, not baked-in guesses.

## Open questions for the owner (recommended first)

1. Ship slice B (render parity) in this task or split it. Recommended: two PRs under this issue; slice A first, slice B after the live read.
2. Show a "Bevel" row and a "Splay" row. Recommended: yes, labelled "Bevel" and "Splay", provisional until the live read; a stored parameter without a control would hide it.
3. Run the live Figma read (about 20 minutes). Recommended: yes; it closes five UNKNOWNs.
4. Existing Studio glass and noise effects will look different after slice B (new math, new seeds' patterns). Recommended: accept; say so in the release notes.

## 14. Facts resolved by REA 2 (2026-10-19)

Static reading of Figma's fullscreen wasm and app bundle. No Figma code was copied. Where a line above says UNKNOWN or INFERENCE and this section says FACT, this section wins.

- FACT: kiwi `specularAngle` is in degrees and maps 1:1 to plugin `lightAngle`. It is clamped to −899…900 and defaults to −45. Studio stores degrees.
- FACT: glass defaults are refraction 0.8, depth 20, light intensity 0.8, angle −45, bevel 1, dispersion 0.5, splay 0 and frost radius 4. Plugin ranges are refraction 0–1, depth 1–100, intensity 0–1, dispersion 0–1, radius 0–100 and splay 0–1. Plugin writes always set bevel to 1.
- FACT: kiwi `noiseSize` equals the UI/plugin size, with X and Y independent. The renderer uses 1/size per axis, so UI 0.5 means a frequency of 2 per pixel.
- FACT: noise defaults are monotone, colour black at 25%, opacity 0.25, density 1, secondary colour white at 25%, size (0.5, 0.5) and a random seed. Texture defaults are size (4, 4), radius 4, clip-to-shape off and a random seed.
