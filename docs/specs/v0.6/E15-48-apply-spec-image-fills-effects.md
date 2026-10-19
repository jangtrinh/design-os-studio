# E15-48 Support image fills and effects in apply_spec (NodeSpec)

- Issue: #638 (task E15-48, epic #585, M / P0)
- Spec base: `main` at `767acf9`.
- Files: `crates/designos-doc/src/spec.rs` (new `fills` and `effects` keys, export), `crates/designos-doc/src/spec_paint.rs` (new: `PaintSpec`, `EffectSpec`, validation), `crates/designos-doc/src/variables_apply.rs` (effect binding keys, image binding key), `crates/designos-doc/src/image_key.rs` (shared with E15-39), `crates/designos-mcp/src/engine.rs` (`apply_spec` workspace-relative image paths, schema description).

## 1. Summary and user story

As an agent, I want one `apply_spec` call to create a card with an image fill and a drop shadow, with colours and numbers optionally bound to variables, instead of many follow-up calls or skipping them.

## 2. Current state

- FACT: `NodeSpec` sets fills only through `fill: Option<String>` (hex or `$easeui.*` token) and strokes through `stroke` (`crates/designos-doc/src/spec.rs:86-92`); it has no image fill and no effects key. Effects are set only by EaseUI presets (one drop shadow in `apply_easeui_preset`, `spec.rs:240-246`).
- FACT: `NodeSpec` does not reject unknown keys (no `deny_unknown_fields`, `spec.rs:44-46`), so a misspelt key is silently ignored today.
- FACT: `export_spec` keeps only the first solid fill and stroke and no effects (`spec.rs:621-628`), so image fills and effects do not round-trip through specs.
- FACT: paint and effect types: `PaintKind::Image { image_ref, scale_mode }`, `ImageScaleMode::{Fill, Fit, Crop, Tile}` (`crates/designos-doc/src/paint.rs:179-213`); `Effect::{DropShadow, InnerShadow, LayerBlur, BackgroundBlur, Noise(NoiseEffect), Texture(TextureEffect), Glass(GlassEffect)}` with their fields (`paint.rs:284-381`).
- FACT: variable binding keys for effects exist only for opacity (`effects/N/opacity`, `crates/designos-doc/src/variables_apply.rs:13,83-96`).
- FACT: the MCP server confines input paths to the workspace (`crates/designos-mcp/src/server.rs:319-328`).
- FACT: E15-1 (#586) instances, E15-3 (#588) `var:` references, E15-5 (#590) named errors with corrected examples, E15-39 asset jobs and SHA-1 image keys.

## 3. Behaviour rules

1. **New keys.** `fills: Vec<PaintSpec>` and `effects: Vec<EffectSpec>` on `NodeSpec`. `fill` (string) stays for back-compat; giving both `fill` and `fills` is `spec_conflicting_keys`. Paints apply bottom to top in array order (Figma order).
2. **PaintSpec** (`type` tagged):
   - `{ "type": "solid", "color": "#RRGGBB[AA]" | "var:<name>", "opacity": 0..1 }`;
   - `{ "type": "image", "asset": "<image key>" | "path": "<workspace-relative path>" | "job": "<asset job id>", "scale_mode": "fill" | "fit" | "crop" | "tile", "opacity": 0..1, "image_var": "var:<String variable>" }` — exactly one of `asset`, `path`, `job`.
3. **Image sources.**
   - `asset`: must exist in `Document.images` (`image_not_found` otherwise).
   - `path`: resolved inside the MCP workspace with the server's input path rules; file ≤ 64 MiB; PNG and JPEG stored as is, WebP and GIF converted to PNG (the renderer decodes PNG and JPEG only); stored under the SHA-1 key (E15-39 rule 4). Errors: `image_path_invalid` (outside workspace, missing, not a file), `image_unsupported_format`, `image_too_large`. On the CLI (`designos --apply-spec`), paths resolve relative to the spec file's folder with the same containment rule.
   - `job`: the paint is created when the E15-39 job completes; until then the node carries the E15-39 pending state, and the paint array keeps a slot reserved at that index (the job's completion writes into the slot in its own undo step). A job id that does not exist → `asset_job_unknown`.
4. **Image variable binding** (`image_var`). A String variable whose value per mode is an image key; binding key `fills/<i>/imageRef`. On mode change the paint's `image_ref` follows the variable (resolved like other bindings in `variables_apply`). Values that are not existing image keys resolve to no change and a lint warning `image_var_unresolved`. This is a design:os extension: Figma has no image variables, so `.fig` export writes the currently resolved image and drops the binding (listed in the export notes).
5. **EffectSpec** (`type` tagged), defaults in brackets:
   - `drop_shadow { color ["#00000040"], x [0], y [4], blur [12], spread [0], visible [true], show_behind [false] }`;
   - `inner_shadow { color, x, y, blur, spread, visible }`;
   - `layer_blur { radius [8], visible }`, `background_blur { radius [16], visible }`;
   - `noise { mode: "mono" | "duo" | "multi" ["mono"], color, secondary, size [1], density [0.5], opacity [0.2], blend ["normal"], seed [0], visible }`;
   - `texture { size [4], radius [2], clip_to_shape [true], seed [0], visible }`;
   - `glass { light_angle [45], light_intensity [0.5], refraction [0.3], depth [8], dispersion [0.1], frost [12], visible }`.
   Field names map one to one onto `Effect` fields (`x`/`y` → `offset`, `blur` → `radius`). Ranges: blur and radius ≥ 0; fractions in 0..1; values are never clamped, and an out-of-range value is an error (`spec_value_out_of_range`, details name the field and range).
6. **Effect variable bindings.** Any colour or number field may be `"var:<name>"`. Binding keys: `effects/<i>/color`, `effects/<i>/radius` (blur), `effects/<i>/spread`, `effects/<i>/offsetX`, `effects/<i>/offsetY` (Figma's effect `boundVariables` names), plus existing `effects/<i>/opacity`. `variables_apply` gains resolve and apply for these keys with scopes `EFFECT_COLOR` (color) and `EFFECT_FLOAT` (numbers). AC2: a bound shadow colour follows a mode switch.
7. **Strict keys.** `NodeSpec`, `PaintSpec` and `EffectSpec` reject unknown keys. An unknown key returns `spec_unknown_key` with `details: { path: "children[2].effects[0].blurr", allowed: [...] }` and a corrected example that replaces the key with the closest allowed one (Levenshtein ≤ 2, using `strsim`, already in `Cargo.lock`), per the E15-5 error contract. Owner ruling 2026-10-19: strict from v0.6 everywhere, with no warning release. Unknown keys at any level (top-level `NodeSpec`, `children[*]`, `fills`, `effects`, layout and text sub-objects) are errors in v0.6. Consequences:
   - `export_spec` must emit only keys `apply_spec` accepts (round-trip test below), so an exported spec always re-applies.
   - Keys added by later tasks (E15-21 `surface`, SC-4 agent fields) land in the same PR as the key's parser; a spec using a key from an unmerged task fails with `spec_unknown_key`, which is the intended signal.
   - The changelog for v0.6 lists the change as breaking for agents that relied on ignored keys, with the error example.
   - The agent guide (`designos_guide`, E15-15) states that unknown keys are rejected and lists the allowed keys per object.
8. **Atomicity.** All of this happens inside the existing trial-document `apply_spec` path (`crates/designos-mcp/src/engine.rs:262-268`): any error leaves the document unchanged (AC3), and success is one undo step (AC1), including image bytes added to `Document.images`.
9. **Export.** `export_spec` writes `fills` (solid paints and image paints with their `asset` key and scale mode) and `effects` (all types, with `var:` for bound fields) whenever the node has more than one exportable paint, a non-solid exportable (image) paint, or any effect, so a node with one solid and one gradient exports as `fill`, not a one-item `fills`; `fill` is kept for the single-solid case so old specs stay identical. **Gradients are omitted** from `export_spec`: `PaintSpec` has only the `Solid` and `Image` variants (strict, rule 7), so a `{ "type": "gradient" }` entry would be an unknown variant that `apply_spec` rejects, and an exported spec must always re-apply. Each omitted gradient is listed in the export notes (`{ node_id, path: "fills[<i>]", reason: "gradient_not_exportable" }`), never dropped silently. If every paint of a node is a gradient the node exports with no `fill` and no `fills` key. A later task that adds a gradient `PaintSpec` variant (parser, validation and export in one PR) lifts the omission.

## 4. Data model / schema

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PaintSpec {
    Solid { color: String, #[serde(default = "one")] opacity: f32 },
    Image { #[serde(default)] asset: Option<String>, #[serde(default)] path: Option<String>, #[serde(default)] job: Option<String>,
            #[serde(default)] scale_mode: ScaleModeSpec, #[serde(default = "one")] opacity: f32, #[serde(default)] image_var: Option<String> },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum EffectSpec { DropShadow(ShadowSpec), InnerShadow(ShadowSpec), LayerBlur(BlurSpec), BackgroundBlur(BlurSpec),
    Noise(NoiseSpec), Texture(TextureSpec), Glass(GlassSpec) }
pub struct ShadowSpec { pub color: ColorOrVar, pub x: NumOrVar, pub y: NumOrVar, pub blur: NumOrVar, pub spread: NumOrVar,
    pub visible: bool, pub show_behind: bool }
pub enum NumOrVar { Num(f32), Var(String) }   // untagged: number or "var:…" string
pub enum ColorOrVar { Hex(String), Var(String) }
// NodeSpec
#[serde(default, skip_serializing_if = "Option::is_none")] pub fills: Option<Vec<PaintSpec>>,
#[serde(default, skip_serializing_if = "Option::is_none")] pub effects: Option<Vec<EffectSpec>>,
```

Back-compat: specs without the new keys behave exactly as before; `export_spec` output for single-solid nodes is unchanged (rule 9).

## 5. Contracts

AC1 example (`designos_apply_spec`):

```json
{ "parent_id": 2, "spec": {
  "kind": "frame", "name": "Card", "width": 320, "height": 360, "corner_radius": 16,
  "layout": { "direction": "column", "gap": 12, "padding": [0, 0, 16, 0] },
  "fills": [ { "type": "solid", "color": "var:designos-style:color/color/card" } ],
  "effects": [ { "type": "drop_shadow", "color": "var:designos-style:color/color/shadow", "y": 8, "blur": 24 } ],
  "children": [
    { "kind": "rectangle", "name": "Cover", "width": 320, "height": 180,
      "fills": [ { "type": "image", "path": "assets/cover.jpg", "scale_mode": "fill" } ] },
    { "kind": "text", "name": "Title", "text": "Quiet desk", "font_size": 20 } ] } }
```

Errors (SC-1), each with a corrected example:
- `spec_unknown_key` — `{"type":"drop_shadow","blurr":24}` → corrected `{"type":"drop_shadow","blur":24}`;
- `spec_conflicting_keys` — `fill` and `fills` together → corrected keeps `fills`;
- `image_path_invalid` — `"path": "../outside.png"` → corrected `"path": "assets/cover.jpg"` (an existing workspace image when one exists, else a placeholder note in `message`);
- `image_not_found`, `image_unsupported_format`, `image_too_large`, `asset_job_unknown`;
- `image_source_ambiguous` — two of `asset`/`path`/`job`;
- `spec_value_out_of_range` — `"density": 1.4` → corrected `"density": 1.0`;
- `variable_not_found` and `variable_kind_mismatch` from E15-3 for bad `var:` references.
All are `safe_to_retry: true` (nothing was written).

## 6. File-format mapping

| Spec | Document | `.fig` export |
|---|---|---|
| `image` paint | `PaintKind::Image`, bytes in `images` under SHA-1 key | `IMAGE` paint with image hash (`fig_export.rs:1727-1733`) |
| `image_var` | binding `fills/i/imageRef` | dropped; resolved image written; export note |
| effect `var:` fields | bindings `effects/i/color|radius|spread|offsetX|offsetY` | Figma effect `boundVariables` (written by the variables export path; E1-9 #138 owns the JSON side) |

## 7. UI

None. Studio shows the result like any node; bound effect fields show the variable chip in the existing effects panel when it supports bindings (no change required here).

## 8. Edge cases

1. Image paint on a text node: allowed (text fills accept image paints in Figma); rendered by the existing text paint path.
2. `opacity: 0` image: kept (invisible but present), no warning.
3. Same image path used twice in one spec: read once, one key.
4. Huge image (8,000 × 8,000 PNG under 64 MiB): stored as is; render uses existing image decoding paths.
5. `job` paint whose job fails: slot removed in the failure handling and the node keeps the E15-39 failed state; the paint array shrinks by one with a warning in the job status.
6. Effects on a group: allowed as in Figma (effects apply to the group's composite).

## 9. Non-goals

- Authoring custom shaders (E5-5, #170).
- Generating images (E15-39/40/41).
- Gradient authoring and gradient export in specs (omitted until a `PaintSpec` variant round-trips them, rule 9).

## 10. Test plan

Seam: `cargo test -p designos-mcp apply_spec` plus doc round-trip tests in `designos-doc`.

Fail-before: `card_with_image_and_shadow_one_undo` fails on `main` (keys ignored, no image, no effect).

| Test | Input | Expected |
|---|---|---|
| `card_with_image_and_shadow_one_undo` (AC1) | §5 example with a fixture JPEG in the workspace | cover has an image paint (SHA-1 key present in `images`); card has one drop shadow; undo depth +1; one undo restores the original hash |
| `bound_shadow_colour_follows_mode` (AC2) | `color/shadow` Light `#00000040`, Dark `#00000099`; switch the card's mode to Dark | resolved shadow colour equals the Dark value |
| `invalid_image_path_refused_unchanged` (AC3) | `"path": "../x.png"` | `image_path_invalid`; document hash unchanged; no image added |
| `unknown_effect_key_named_error` | `blurr` | `spec_unknown_key` with corrected `blur` |
| `unknown_top_level_key_rejected` | `{"type":"frame","widht":100}` | `spec_unknown_key`, path `widht`, corrected `width`; document unchanged |
| `export_spec_reapplies_strict` | export every fixture node from `crates/designos-doc` test fixtures that has no gradient paint (nodes with a gradient paint are not in this set, rule 9), re-apply | zero `spec_unknown_key`; equal paints and effects |
| `export_spec_omits_gradients_and_lists_them` | a rectangle with a gradient fill and a solid fill; a rectangle with only a gradient fill | the first exports its solid paint only, the second exports no `fill` or `fills` key; both gradients appear in the export notes with `gradient_not_exportable` and the paint index; the exported specs re-apply with zero `spec_unknown_key` |
| `fill_and_fills_conflict` | both keys | `spec_conflicting_keys` |
| `export_spec_round_trip` | apply §5, export, re-apply into a new doc | equal node paints and effects (image keys equal) |
| `single_solid_export_unchanged` | old-style spec | export identical to `main` output |
| `all_effect_types_map` | one spec per effect type | resulting `Effect` fields equal the spec values |
| `image_var_switches_image` | String variable with two image keys per mode | paint `image_ref` follows the mode |

Visual evidence: CPU render of the §5 card in Light and Dark modes.

## 11. Performance budgets

- Image file read and SHA-1 run in the MCP server before the trial apply; 10 MB image ≤ 80 ms.
- `apply_spec` cost otherwise unchanged; one `Command::Insert` (CLAUDE.md §4 unaffected).

## 12. Dependencies and build order

- Needs E15-1 (#586). Uses E15-3 (#588), E15-5 (#590), E15-39 (job source; the `job` variant lands with E15-39 if it merges later).
- Merges first in the `spec.rs` queue (README-part2). Unblocks E15-21 (its `surface` key lands in the strict key list after this task).

## 13. Owner rulings that apply

- Full scope (owner ruling 2026-10-18): image fills from asset id, path or asset tools, all four scale modes, optional variable binding; drop shadow, inner shadow, layer blur, background blur, noise, texture and glass, each bindable.
- Named errors with a corrected example (E15 error contract).
- Unknown keys are rejected from v0.6, at every level (owner ruling 2026-10-19).
- Colour is Display P3 per document: hex values convert into the document profile (SC-6).
