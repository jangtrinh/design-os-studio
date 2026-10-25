# #521 Image fills: the wrong image inside a variable-mode scope, and fill-mode crops

| Field | Value |
|---|---|
| Issue | #521 (bug; symptom bucket from the kit audit of 2026-10-18: 27 frames on 10 pages, not diagnosed) |
| Size | M (the crop slice is #163's work; this task owns the diagnosis, image-valued variables and the kit verification) |
| Depends on | #163 (hot-file queue position 21): the image transform, rotation, tile scale and adjustment fields and the crop render. Queue position 22 for this task (README-part2) |
| Unblocks | later work on `paintVar` (paint-valued variables) |
| Touches | `crates/designos-doc/src/variables.rs` (`VariableKind` `:43-49`, `VariableValue` `:51-59`), `crates/designos-doc/src/variables_apply.rs` (`set_bound_property` `:153-215`, `property_kind` `:21-32`), `crates/designos-fig/src/variables.rs` (`kind_of` `:52-60`, `parse_value` `:39-50`, `fig_value` `:395-404`, `type_name` `:72-79`), `crates/designos-fig/src/fig.rs` (`bound_variables` `:1650-1744`), `crates/designos-fig/src/fig_export.rs` (variables `:520-555`, `encode_paint` `:1841-1885`), `crates/designos-app/src/variables/` and `panels/var_bind.rs` (display of an image value), tests. HOT FILES: `fig.rs`, `fig_export.rs`. |

Code baseline: `origin/main@0e8a9a5`. Evidence: REA 2026-10-19 (kiwi schema in `crates/designos-fig/src/figma_schema.txt`; reading of the code; no Figma ground truth: the plugin was not connected and the kit file was not on the research machine, so the variable-mode cause is read from code only).

## 1. Summary and user story

As a designer opening a kit with light and dark pages, I want an image that is chosen by a variable mode (a dark map in a dark scope) to show the right image, and a cropped photo to show the crop Figma shows, so a login hero, team photos and a contact map look like the original.

## 2. Current state (FACT)

- Two sub-patterns from the issue: a different image is chosen inside a variable-mode scope (the contact page map is light in Studio, dark in Figma), and the fill-mode crop differs (the log-in hero "Section" and team photos). The image bytes are all present (the audit's image check), so it is selection or transform.
- **Crop** (root cause shared with #163): the importer reads only the hash and the mode (`fig.rs:2510-2519`), drops `transform`, `rotation`, `scale` and `paintFilter`, and `PaintKind::Image` has no field for them (`designos-doc/src/paint.rs:209-212`); Crop is drawn like Fill (`designos-render/src/paint.rs:325-328`, `designos-gpu/src/paintconv.rs:166-169`) and Tile ignores scale (`render paint.rs:333`). #163 adds the fields and the render; its spec holds the rules (UV = M applied to [u, v, 1]).
- **Variable mode.** `imageVar` (paint field 31) and `paintVar` (46) are never read: `bound_variables` collects only `colorVar` and `opacityVar` for fills and strokes (`fig.rs:1711-1721`) and `colorVar` and `effectOpacityVar` for effects (`:1722-1741`).
- The variable model has four kinds only: `VariableKind { Color, Number, String, Boolean }` and `VariableValue { Color, Number, String, Boolean, Alias }` (`designos-doc/src/variables.rs:43-59`). The `.fig` variable reader drops any variable whose resolved type is not one of them: `kind_of` returns `None` for IMAGE (`designos-fig/src/variables.rs:52-60`), `parse_value` and `fig_value` have no image arm (`:39-50`, `:395-404`), and the exporter writes only the four (`fig_export.rs:520-535`).
- Render-time resolution exists for the four kinds: `resolve_node` substitutes the value a bound variable resolves to in the mode in effect at the node (`variables_apply.rs:292-306`), through `Document::resolve_variable`, which walks the node's ancestors for `explicitVariableModes` (`designos-doc/src/variables.rs:154-189`). For a fill, `set_bound_property` accepts only a colour: `color_of(value)` then `PaintKind::Solid` (`variables_apply.rs:160-181`); a value of another kind is ignored ("mismatched types are ignored", `:151-152`). The bound-variable pill in the panel already shows no swatch for a non-colour value (`panels/var_bind.rs:35-38`).
- For `.fig` all image bytes of the archive are kept (`doc.images = d.archive.images`, `fig.rs:253`), so an image that only a variable value references is present in the document. (The REST path lists images by reference, `rest_import.rs:3624-3650`; REST has no image variables, INFERENCE.)
- FACT (kiwi schema; REA 2026-10-19): `Paint.imageVar = 31` and `paintVar = 46` are `VariableData`; `VariableDataType` has `IMAGE = 14` and `PAINT = 33`; `VariableResolvedDataType` has `IMAGE = 9` and `PAINT = 26`; `VariableAnyValue.imageValue = 14` holds an `ImageParameterValue { image, imageThumbnail, animatedImage, altText, originalImageHeight, originalImageWidth, animationFrame }`; `VariableData { value, dataType, resolvedDataType }`.
- INFERENCE (REA 2026-10-19, reading the code): the wrong-image cause is that the importer never reads `imageVar`, so a paint bound to an image variable keeps the image stored on the master (the default mode's), and the scope's mode is never applied. This explains "light map where Figma shows dark map".
- INFERENCE: the listed frames bind images through instances inside a frame that sets an explicit mode.
- UNKNOWN: that the listed fills actually carry `imageVar` and that the variable has per-mode image values. Closing check: owner-manual live Figma read plus a kit scan (below). If the fills carry no `imageVar`, the variable-mode cause is wrong and this task stops after the scan.
- UNKNOWN: which scale mode and which `transform` the listed crop fills carry (the issue says "the scale mode of each listed fill was not checked"). Closing check: the same scan.
- UNKNOWN: whether `paintVar` appears in the kit at all. Closing check: the same scan (count of paints with `imageVar` and with `paintVar`).
- Issue-vs-code: the issue links `paint.rs:209-212` and `render paint.rs:325-328` at an older commit; the lines are unchanged on this baseline. REST import ignores image variables by construction (no such REST type).

## 3. Behaviour rules

0. **Diagnose first.** Before code: the scan of section 10 reports, for the 8 listed frames, each image fill's scale mode, transform, `imageVar` and the per-mode image values of its variable, and the explicit modes on the node's ancestors. The variable slice proceeds only if that matches the inference.
1. **Image variables exist.** `VariableKind::Image` and `VariableValue::Image(String)` (the image reference, the hex hash, the same key as `PaintKind::Image.image_ref`). Import reads resolved type IMAGE and `imageValue.image` per mode; export writes them back (resolved type IMAGE, data type IMAGE, `imageValue`).
2. **Binding.** A paint's `imageVar` becomes a binding on the key `fills/N` or `strokes/N` (the same slots as a colour binding; a paint holds one or the other), alias-resolved like `colorVar`.
3. **Resolution.** `set_bound_property` accepts an image value on a `fills/N` or `strokes/N` key: the paint becomes `PaintKind::Image { image_ref, scale_mode, params }` with the paint's own scale mode and image parameters kept (from #163); if the paint was not an image, scale mode Fill. A colour value on a slot that holds an image paint behaves as today (becomes a solid).
4. **Mode scope.** The mode in effect is the one `resolve_variable` already computes from the node's own and its ancestors' explicit modes; no new mode logic.
5. **Round trip.** `imageVar` (31) round trips through `.fig`; bindings survive import, export, re-import; effect and style paints too.
6. **`paintVar` (46)** is not resolved (non-goal) and is counted in `ImportReport.dropped_variable_bindings` (see section 5) together with the other bindings Studio does not carry.
7. **Crop and fill-mode crops** follow #163's rules; this task adds the acceptance: the frames of section 10 match Figma by eye after #163, or the remaining difference is named by cause.
8. **No new cause by guess.** If the scan shows a third cause (for example `imageThumbnail` or `animatedImage` chosen instead of `image`, or a FILL paint with a non-identity transform), it is named in the PR and filed as its own task.

## 4. Data model / schema changes

- `VariableKind::Image`, `VariableValue::Image(String)` in `designos-doc/src/variables.rs`. Files without image variables are unchanged.
- Mechanical first commit: every exhaustive `match` on `VariableKind` and `VariableValue` gets an Image arm (32 non-test sites mention `VariableKind::Boolean`, a proxy count; the compiler lists the real ones), then the behaviour commit.
- No change to `Node` or `Paint` beyond what #163 adds.

## 5. Contracts

- `.designos` JSON: `Variable.kind` gains the value `Image` and a value `{ "Image": "<hash>" }`; an old Studio cannot read a file that holds an image variable (serde rejects the unknown variant; whether it refuses the whole file is not verified). Acceptable: only such files are affected.
- `.fig`: variable resolved type IMAGE (9), data type IMAGE (14), `imageValue` (14), paint `imageVar` (31). REST-shaped internal payload: `resolvedType` IMAGE and a value object `{ "imageRef": "<hash>" }` (internal; REST has no image variable type, INFERENCE).
- Import report: `ImportReport.dropped_variable_bindings` is the counter introduced by #37 or #38 (whichever started first) on the `ImportReport` of v0.6's #5; if none of them has landed, this task creates it in the shape #5's rule 5 gives.
- The Variables panel lists an image variable with its name and the kind label "Image"; its value cell shows the text "Image" and the last 8 characters of the hash in `font_mono`, not interactive (no fake control). Binding an image variable from the picker is a non-goal.

## 6. File-format mapping

| Studio | Kiwi | REST-shaped internal |
|---|---|---|
| `VariableKind::Image` | `variableResolvedType` IMAGE (`VariableResolvedDataType` 9) | `resolvedType` IMAGE |
| `VariableValue::Image(hash)` per mode | `variableDataValues.entries[].variableData`: dataType IMAGE (14), `value.imageValue.image.hash` | `valuesByMode[mode] = { imageRef }` |
| binding `fills/N`, `strokes/N` to an image variable | `Paint.imageVar` (31): dataType ALIAS, `value.alias` | `boundVariables.fills[N]` alias, as for colour |
| not carried | `Paint.paintVar` (46), `gradientVar` (43) | counted in `ImportReport.dropped_variable_bindings` |

## 7. UI

Words only. Checked against `design/principles.md` (P3, P5, P6, P10) and `design/soul.md`.

- No new control. The only UI change is how an image variable is shown, in the Variables panel and in the bound-variable pill: the pill shows the variable name with no colour swatch (already the behaviour for a non-colour value, `panels/var_bind.rs:35-38`); the panel row shows the kind label "Image" and the per-mode cell text described in section 5. Cells are not interactive; no thumbnail in this task.
- Desktop capture at 1440 in the PR: the contact-page frame (or the synthetic fixture) in a light scope and a dark scope showing the two images, and the Variables panel with an image variable. Responsive: SKIPPED (owner default). Load es-designer before UI code; update Figma from the verified capture first.

## 8. Edge cases

- An alias chain from an image variable to another image variable: resolved by `resolve_variable` (cycle depth cap unchanged, `MAX_ALIAS_DEPTH`).
- A mode with no value for the image variable: falls back to the collection's default mode value, as for the other kinds (`variables.rs:172-186`).
- An image variable whose hash is not in `doc.images`: the paint draws nothing (as an image with missing bytes does today) and the pill still shows the name.
- A paint bound to an image variable inside a component instance with overrides: the binding comes from the instance's clone; the resolution is per cloned node, so two instances in two scopes show two images.
- Text fills never take an image value (text fill slots ignore it, as `set_bound_property` ignores mismatched types).
- Stroke slots: accept the image value on the same terms as fills (Figma allows image strokes; INFERENCE).
- A colour variable and an image variable on the same node, different slots: both resolve.
- `.fig` written by Studio with an image variable and read by an old Figma: not verified; the exporter writes the same message shapes Figma writes.
- Variable library collections (`source_library_key`): image values behave like the other kinds (no special case).

## 9. Non-goals

- Paint-valued variables (`paintVar`, PAINT, gradient variables).
- Editing image variable values in the panel, binding an image variable from the picker, thumbnails in the Variables panel.
- The crop and rotation render themselves (#163).
- REST image variables (none exist).
- Any variable kind beyond Image.

## 10. Test plan (red first)

Seams: `designos-doc` unit tests (the pattern of `variables_apply.rs` tests), `designos-fig` round trip (the pattern of `variables.rs` and `fig_roundtrip.rs`), a CPU pixel test, one ignored kit test.

Fixture (written by the test): two 2 x 2 PNGs, "light" all `#FFFFFF` and "dark" all `#101010`; a collection with modes Light and Dark; an image variable whose Light value is the light image and Dark value the dark image; a frame with an explicit mode (Dark) holding a rectangle whose fill is bound to the variable (a stored paint with the light image, as a master would store it); a sibling frame in Light.

| Case | Expected | Fails before |
|---|---|---|
| `image_variable_resolves_the_dark_image_in_a_dark_scope` | the rectangle in the Dark frame resolves to the dark image; the Light frame to the light one | yes (no image kind) |
| `render_pixel_in_dark_scope_is_dark` | CPU render pixel (any inside) equals `#101010`; Light frame `#FFFFFF` | yes |
| `image_binding_keeps_scale_mode_and_image_params` | after resolution the paint keeps its scale mode and #163's params | yes |
| `colour_value_on_an_image_slot_becomes_a_solid_as_before` | unchanged | guard |
| `missing_mode_value_falls_back_to_the_default_mode` | default-mode image | yes |
| `missing_image_bytes_draw_nothing_and_keep_the_pill_name` | empty fill, name kept | yes |
| `fig_import_reads_image_variable_and_image_var_binding` | variable kind Image with both mode values; binding on `fills/0` | yes (dropped) |
| `fig_round_trip_keeps_image_variable_values_and_image_var` | equal after import, export, re-import | yes |
| `designos_json_round_trip_keeps_image_variables` | save and load equal | yes |
| `paint_var_is_counted_not_resolved` | `dropped_variable_bindings` counts it | yes |
| `old_documents_without_image_variables_save_unchanged` | JSON identical | guard |
| `variables_panel_lists_image_variable_with_kind_label_and_read_only_cells` (app) | label "Image", non-interactive cell | yes |
| `zero_fake_ui_gate` | green | guard |
| kit test (ignored, `DESIGNOS_REFERENCE_DIR`): scan of the 8 listed frames | prints per fill: scale mode, transform, `imageVar`, per-mode values, ancestor modes; and asserts nothing before the scan result is recorded in the PR | n/a |
| kit test (ignored): crop frames `1646:406365`, `6655:108055` and the contact map `1639:379127` | rendered pixel regions are compared against the exports the owner supplies (by eye is the acceptance; the test prints the paths) | n/a |

Run: `cargo test -p designos-doc`, `cargo test -p designos-fig`, `cargo test -p designos-render`, `cargo test -p designos-app variables`; kit tests with `cargo test -p designos-fig --test real_files -- --ignored`. Evidence: red/green capture, the light and dark renders, the scan output, the 1440 captures.

Owner-manual checks (flagged, never claimed by an agent):

- **Kit scan and live Figma read.** With the kit file, run the scan of rule 0 on the 8 listed frames (the ignored kit test prints it) and, in Figma, read for the contact-page map frame which image its fill shows under each mode (closes: the `imageVar` presence, the scale mode and transform of the crop fills, the `paintVar` count).
- **By-eye acceptance** of the two frames named in the issue after #163 and this task land: the owner compares the Studio render with Figma's export.

## 11. Performance budgets

- Resolution cost: one extra clone of a node with an image binding per render pass that already clones for other bindings (`resolve_node` borrows when there are no bindings, `variables_apply.rs:293-295`); no cost for documents without bindings.
- Import: reading `imageVar` is one field per paint; no extra decode (the archive images are already held).
- No UI-thread `to_json`; idle 0.00%; no `app.changed()` from the panel row.

## 12. Dependencies and build order

Needs #163 (queue position 21). This task is queue position 22 on `fig.rs`, `fig_export.rs` (README-part2). Order inside: scan (rule 0, no code), mechanical Image arms, resolution, `.fig` import and export, panel row. The crop acceptance is checked after #163 merges and needs no code from this task.

## 13. Owner rulings

- Acceptance (issue): the cause is named for each sub-pattern (variable-mode image selection, fill-mode crop transform); the log-in pages Desktop (`1646:406365`) and the contact-page map frame match Figma by eye.
- Delivery law 3 (count before you target): the "27 frames" are a bucket; the scan counts how many carry `imageVar`, a transform, or neither.
- Delivery law 4 (fix at the shared layer): the crop fix is #163's shared model, not a second implementation here.

## Open questions for the owner (recommended first)

1. Run the kit scan and the live read before any code. Recommended: yes; it takes about 15 minutes and decides whether the variable slice is needed.
2. Keep #521 separate from #163. Recommended: yes; #163 carries the crop fix, #521 carries variables and the verification.
3. Image variables in the Variables panel are read-only in this task. Recommended: yes; editing and binding are a follow-up.
