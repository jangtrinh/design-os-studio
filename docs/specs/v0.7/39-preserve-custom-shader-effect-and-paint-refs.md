# #39 Preserve custom shader effect and paint references on round trip

| Field | Value |
|---|---|
| Issue | #39 (task; roadmap; full-scope amendment 2026-10-17; owner ruling R2.8) |
| Size | M |
| Depends on | hot-file queue position 18 (#37) merged first (section 12); #174 (X1, merged) is node-level only, see section 2 |
| Unblocks | #161 (queue position 20); the user-authored WGSL shaders of pack v0.24 (E5-5, E5-6) which add rendering on top of this carrier |
| Touches | `crates/designos-doc/src/paint.rs` (`Effect` `:296-319`, `PaintKind` `:188-214`, `Paint::silhouette_alpha` `:228-234`), `crates/designos-fig/src/fig.rs` (`convert_paint` `:2483-2523`, `convert_effect` `:2525-2568`), `crates/designos-fig/src/fig_export.rs` (`encode_paint` `:1841-1885`, `encode_effect` `:1887-1960`), `crates/designos-fig/src/rest_import.rs` (`parse_paint` `:70-103`, `parse_effect` `:109-159`), `crates/designos-fig/src/export.rs`, `crates/designos-render/src/lib.rs` and `crates/designos-gpu/src/compile.rs` (skip arms), `crates/designos-app/src/panels/effects.rs`, `crates/designos-app/src/panels/properties.rs`. HOT FILES: `fig.rs`, `fig_export.rs`, `rest_import.rs`. |

Code baseline: `origin/main@0e8a9a5`. Evidence: REA 2026-10-19 (kiwi schema in `crates/designos-fig/src/figma_schema.txt`; plugin typings 1.142.0; Figma fullscreen wasm shader source for the fallback question).

## 1. Summary and user story

As a designer opening a Figma file that uses shader effects or shader fills, I want Studio to keep them untouched and write them back on export, and to tell me honestly that it does not render them, so I never lose a shader by opening and saving a file.

## 2. Current state (FACT)

- Shader effects and fills are dropped on import, silently. `convert_effect` passes an unknown type through (`fig.rs:2526-2530`, `other => other`) but `parse_effect` returns `None` for every type it does not know (`rest_import.rs:157`), and the effect list is built with `filter_map`, so the effect disappears. `convert_paint` returns `None` for any paint type other than SOLID, GRADIENT and IMAGE (`fig.rs:2520`), so a CUSTOM paint disappears too. Export cannot write what the model does not hold: `encode_effect` and `encode_paint` match the model enums (`fig_export.rs:1849-1951`).
- The model has no shader variant: `Effect` (`paint.rs:296-319`) and `PaintKind` (`:188-214`).
- X1 (#174, merged) keeps unmapped fields of a node, not of a paint or an effect: `extract_fig_metadata` reads node-level keys (`fig.rs:954`), stored in `Node.fig_metadata: BTreeMap<String, String>` (`designos-doc/src/node.rs:155`) and re-emitted by `append_fig_metadata` (`fig_export.rs:2086`). It therefore does not carry a CUSTOM effect inside `effects` or a CUSTOM fill inside `fillPaints`. The typed JSON-to-kiwi conversion it uses is reusable: `Schema::field_from_json` (`kiwi.rs:788`) and `kiwi::to_json` (`:1004`), where floats print as their shortest f32 round-trip form (`clean_f32`, `:1164`).
- The add-effect menu already lists a disabled "Shader" entry with the shortcut text "Soon" (`panels/effects.rs:138`); it is the only shader surface and is honest (disabled, labelled).
- `Node.extra` already stores arbitrary `serde_json::Value` in the model (`node.rs:247`), so a JSON value in the document model has precedent.
- FACT (kiwi schema; REA 2026-10-19): effect type `CUSTOM = 9` (`EffectType`), paint type `CUSTOM = 10` (`PaintType`); effect fields `customEffectId = 43` (a `CodeComponentId`: `guid = 1` a `GUID {sessionID, localID}`, `assetRef = 2` an `AssetRef {key, version}`) and `componentPropAssignments = 44` (a `ComponentPropAssignment[]`: `defID`, `value` (a `ComponentPropValue`, which may itself hold a `Paint`), `varValue`, `legacyDerivedTextData`); paint fields `customEffectId = 39` and `componentPropAssignments = 40`.
- FACT (REA 2026-10-19, plugin typings 1.142.0): the plugin API calls it SHADER and creates one with `importShaderById`; the shader source lives server-side, so a file carries only the reference.
- UNKNOWN: what Figma draws when the shader cannot be resolved (a missing asset). Closing check: owner-manual live Figma read (below).
- UNKNOWN: whether any local file or the kit contains a shader effect or fill. Closing check: owner supplies a Figma file that uses one (or the kit scan reports zero); the fixture below is synthetic until then.

## 3. Behaviour rules

1. **Opaque passthrough.** A shader effect and a shader fill are carried whole: every field present on the decoded kiwi message is kept verbatim (not only the four named fields), so an unknown future field survives too. They are never interpreted.
2. **Render nothing.** CPU, GPU, export (PNG, SVG, PDF) and codegen skip a shader effect and a shader paint. They draw no pixel, allocate no program and add no export overflow.
3. **Fills keep their other properties.** A shader paint keeps `opacity`, `visible` and `blend` like any paint, and stays in its slot in the fill list (order preserved).
4. **Silhouette.** A shader paint contributes no coverage to shadow, blur-mask or glass silhouettes: `silhouette_alpha` returns 0 for it (INFERENCE: nothing is drawn, so nothing is inked; Figma's rule is UNKNOWN).
5. **Round trip.** Import keeps the raw message; export writes it back through the schema with the same field ids; effect styles carry shader effects. The REST and plugin name for the type is SHADER (INFERENCE from the plugin typings); the kiwi enum is CUSTOM.
6. **Honest UI.** The inspector shows the label "Shader (not rendered)" (section 7). It is a status text, not a control.
7. **No creation.** Studio cannot create a shader effect or fill; the disabled "Shader / Soon" menu entry stays as is (owner ruling R2.8: custom shaders are user-authored WGSL in pack v0.24; imported Figma shaders stay preserve-only here).
8. **No silent loss.** If a shader message cannot be re-encoded (rare: the file was written with another schema than the one `export_fig` writes), export returns a `FigError` that names the layer and the next action; it never drops the shader silently. `export_fig` has no warnings channel (it returns `Result<Vec<u8>, FigError>`, `fig_export.rs:115`).

## 4. Data model / schema changes

- `ShaderRef { raw: serde_json::Value }` in `designos-doc/src/paint.rs`: the decoded message minus its `type`, as a JSON object (enums by name, bytes as hex, as `kiwi::to_json` writes them).
- `Effect::Shader { shader: ShaderRef, visible: bool }` and `PaintKind::Shader(ShaderRef)`. Their serde shape is `{ "raw": ... }`; old files never contain them.
- Every exhaustive `match` on `Effect` and `PaintKind` gets a Shader arm (the compiler lists them). Do this in a mechanical first commit (arms that do nothing), then the behaviour commit, as in #161 and #37.
- `Effect::visible()` and `panels/effects.rs::set_visible` read and write `visible` for the shader effect; the raw message's own `visible` is rewritten from it on export.

## 5. Contracts

- `.designos` JSON: a new variant that an old Studio cannot read (`Effect` and `PaintKind` are tagged enums, so serde rejects the unknown variant; whether the old Studio refuses the whole file or only that node is not verified). This is acceptable because only files that already hold a shader are affected; the release notes say so. (The alternative, a side map, is not used because it breaks order.)
- `.fig`: fields 39, 40 (paint) and 43, 44 (effect), plus every other field of the message, round trip.
- REST: type SHADER with the raw object under a `shader` key (internal shape; REST itself has no such paint, INFERENCE).
- Export bounds, SVG, PDF and codegen ignore the new variants explicitly (a test per consumer).

## 6. File-format mapping

| Studio | Kiwi | REST / plugin |
|---|---|---|
| `Effect::Shader` | effect type CUSTOM (9), `customEffectId` (43), `componentPropAssignments` (44), all other fields kept | SHADER, raw under `shader` |
| `PaintKind::Shader` | paint type CUSTOM (10), `customEffectId` (39), `componentPropAssignments` (40), all other fields kept | SHADER, raw under `shader` |
| `Paint.opacity`, `visible`, `blend` of a shader paint | `opacity` (3), `visible` (4), `blendMode` (5) | same names |

## 7. UI

Words only. Checked against `design/principles.md` (P3, P5, P6 Zero Fake UI, P10) and `design/soul.md`.

- **Effects section.** A shader effect row shows, in the place of the kind dropdown, the static text "Shader (not rendered)" in caption type with `TEXT_SECONDARY`. It is not a dropdown and not clickable. The effect settings button is absent (nothing to edit). The eye (visibility) and minus (remove) buttons stay and work (removal is one undo step). The row height is the same as the other effect rows.
- **Fill section.** A shader fill row shows the same text in the place of the colour swatch and value, with the opacity, visibility and remove buttons working as for other fills.
- No new colour or size literal; tokens from `theme.rs`. No badge colour is spent on this (the text is neutral); no count in a label.
- The add menu keeps its disabled "Shader" entry with "Soon" (unchanged, still honest).
- Dev Mode inspect: nothing is listed for a shader (no CSS equivalent); the layer's other properties are unchanged.
- Zero Fake UI gate: the text is status, not a control; `python3 scripts/zero_fake_ui_gate.py` stays green.
- Desktop capture at 1440 in the PR: the Effects section and the Fill section of a layer that has a shader effect and a shader fill (synthetic fixture), with the label visible. Responsive: SKIPPED (owner default). Load es-designer before UI code; update Figma from the verified capture first.

## 8. Edge cases

- A node whose only fill is a shader: the layer draws no fill; its shadows come from its other paints, strokes or children as for a fill-less node (see #520 for that rule).
- A shader effect on a layer inside an instance or component: carried like any effect; instance overrides of the list keep the raw message.
- Hidden shader: stays hidden, round trips as hidden.
- Duplicate or copy-paste of a layer: the raw message is cloned with the layer. Pasting into another file keeps the reference; the shader asset may not resolve there (nothing is rendered either way).
- A shader inside `componentPropAssignments` of another paint (`paintValue`): kept inside the raw message of its owner; not interpreted.
- A message that fails to re-encode (schema mismatch): the export fails with a `FigError` naming the layer (rule 8), not a silent drop.
- A very large raw message: the carrier sets no limit of its own and adds no copy at render time.
- Undo: removing a shader effect restores the exact raw message.

## 9. Non-goals

- Rendering Figma shaders (source is server-side) and user-authored WGSL shaders (E5-5, E5-6, pack v0.24).
- Creating a shader in Studio, editing its properties, or resolving its asset.
- A typed model of `ComponentPropAssignment`.
- Shader paints as variables (`paintVar`).

## 10. Test plan (red first)

Seams: `designos-fig` testkit round trip (pattern of `fig_effect_grid_styles.rs` and `testkit.rs`), render pixel tests, one panel test.

Fixture (synthetic, written by the test): a frame with a CUSTOM effect (`customEffectId` with a `guid` and an `assetRef`, two `componentPropAssignments` (a float and a bool)), a CUSTOM fill with the same fields, an effect style holding a CUSTOM effect, and a hidden CUSTOM effect.

| Case | Expected | Fails before |
|---|---|---|
| `shader_effect_and_fill_survive_fig_import_export_reimport` | the decoded message of every shader equals the original (JSON equal, every field) | yes (dropped) |
| `shader_message_bytes_are_equal_after_round_trip` | encoding the re-imported shader message gives the same bytes as encoding the original message through the same schema (the issue's "byte-equal for those fields") | yes |
| `shader_in_effect_style_round_trips` | style keeps the effect | yes |
| `shader_fill_keeps_opacity_visible_blend_and_slot_order` | fill index and properties unchanged | yes |
| `unknown_field_on_a_shader_message_is_kept` | an extra schema field present on the message survives | yes |
| `shader_renders_nothing_cpu_and_gpu` | pixels equal the render of the node without the shader | yes |
| `shader_paint_adds_no_shadow_silhouette` | a shadow of a node whose only fill is a shader has no fill-derived coverage | yes |
| `shader_adds_no_export_overflow_and_no_svg_or_pdf_output` | bounds and SVG equal the node without it | yes |
| `re_encode_failure_returns_a_fig_error_naming_the_layer` | `FigError` text contains the layer name | yes |
| `old_designos_file_without_shaders_loads_and_saves_unchanged` | JSON identical | guard |
| `designos_json_round_trip_keeps_shader` | save and load keep the raw message | yes |
| `shader_row_shows_the_honest_label_and_no_settings_button` (app) | text "Shader (not rendered)"; no dropdown, no gear; eye and minus work | yes |
| `zero_fake_ui_gate` | green | guard |

Run: `cargo test -p designos-doc`, `cargo test -p designos-fig`, `cargo test -p designos-render`, `cargo test -p designos-gpu`, `cargo test -p designos-export`, `cargo test -p designos-app effects`. Evidence: red/green capture and the 1440 capture of the honest label.

Owner-manual check (flagged, never claimed by an agent): **live Figma read.** Supply or create a Figma file with a shader effect and a shader fill (and, if the shader asset can be made unavailable, one whose asset is missing), and read what Figma draws for the missing case. This closes the fallback UNKNOWN; the code does not change either way (render nothing), only the PR note does.

## 11. Performance budgets

- Render: zero cost; a shader effect or paint is skipped before any program is allocated (the 64-program cap and the VRAM budget are not touched).
- Import and export: the raw message is converted once per shader; no per-frame work; no UI-thread `to_json`.
- Memory: one JSON object per shader.
- Idle: 0.00%.

## 12. Dependencies and build order

Hot-file queue position 19 (issue text), after #37 (position 18); then #161 (position 20). Two commits: mechanical arms, then behaviour. If a user-authored WGSL task (pack v0.24) lands first it replaces the skip arms; this task does not wait for it.

## 13. Owner rulings

- Owner ruling R2.8 (2026-10-17, recorded in `plans/261017-epics-full-scope/owner-decisions-and-cross-check.md`, Round 2 item 8): custom shaders are user-authored WGSL; imported Figma shaders are preserve-only here.
- Acceptance (issue): a round trip is byte-equal for those fields; the inspector shows the honest label "Shader (not rendered)".
- Delivery law 1 and 2: nothing vanishes silently; errors name the next action (the `FigError` text).

## Open questions for the owner (recommended first)

1. Carry the whole message verbatim (this spec) or only the four named fields. Recommended: the whole message; it also keeps fields Studio has never heard of.
2. Provide a Figma file that contains a shader so the fixture can be checked against a real one. Recommended: yes if one exists; otherwise the synthetic fixture stands and the real check waits.
3. An old Studio cannot open a `.designos` file that holds a shader. Recommended: accept, and say so in the release notes.

## 14. Facts resolved by REA 2 (2026-10-19)

Static reading of Figma's fullscreen wasm and app bundle. No Figma code was copied. Where a line above says UNKNOWN or INFERENCE and this section says FACT, this section wins.

- FACT: when the pre-rendered custom-effect texture is missing, Figma logs an error and draws nothing for that effect or paint. A CUSTOM effect also counts as ineffective while a feature flag is on.
- INFERENCE: an unresolved shader adds no silhouette and no coverage. This matches the spec's render-nothing rule.
