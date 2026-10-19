# E15-18 Model style archetypes with typed parameters and ship our own catalogue

- Issue: #604 (task E15-18, epic #585, L / P0)
- Spec base: `main` at `767acf9`.
- Files: `assets/styles/catalogue.json` (new), `assets/styles/archetypes/<id>.json` (new, one per archetype), `crates/designos-doc/src/style_archetype/{mod.rs,schema.rs,resolve.rs,catalogue.rs,tests.rs}` (new), the `get_guidelines` tool module created by #248 (`designos-mcp/src/tools/guidelines.rs` in the issue text).

## 1. Summary and user story

As an agent, I want to pick a style by name and get back a prose brief plus concrete, validated values (fonts, a role-keyed palette, radius scale, elevation set, imagery recipe), so I never invent colours or type pairings.

As a user, I want every bundled style to pass contrast checks and use fonts we are allowed to embed.

Colour and type choices are where models are weakest. Briefs plus concrete values remove those choices.

## 2. Current state

- FACT: the only built-in styling today is the hard-coded EaseUI preset and token set in `NodeSpec`: `preset` (`crates/designos-doc/src/spec.rs:54-58`), `$easeui.*` colour tokens resolved by `resolve_easeui_color` (`spec.rs:145`) and `apply_easeui_preset` (`spec.rs:201`).
- FACT: there is no `get_guidelines` tool yet. The MCP tool list is built in `crates/designos-mcp/src/engine.rs:705` (`tools()`) and `server.rs:620`. There is no `tools/` module directory in `designos-mcp/src` today; #248 (T7.40) introduces `get_guidelines`, and its R&D extension names "parameterised style archetypes: a prose brief plus typed parameters (4 font slots, a role-keyed colour palette such as `surface.primary` / `accent.*`, roundness, elevation, imagery)".
- FACT: `designos-fonts` bundles Inter and fetches Fontsource / Google Fonts on demand (CLAUDE.md §2 crate map).
- FACT: the source knowledge is MIT-licensed (`/Users/jang/Products/ease-design/LICENSE`, "Copyright (c) 2026 ease-design contributors"): `knowledge/personas/personas.json` holds 26 personas, each with typography, a colour philosophy, a radius scale, spacing base, shadow intensity and anti-patterns; `knowledge/token-taxonomy.md` §"The paired semantic convention" defines the `{role}` / `{role}-foreground` pairs; `knowledge/color-science.md`, `knowledge/design-soul.md`.
- FACT: some persona fonts are not embeddable (for example the `liquid-glass` persona names "SF Pro Display").

## 3. Behaviour rules

1. **Archetype = brief + parameter sources.** The brief has seven fixed sections: `core`, `identity`, `layout`, `token_map`, `decoration`, `hierarchy`, `typography`. Each is 1–6 sentences of our own prose, written for design:os. No section is empty.
2. **Parameters.** Five typed parameter groups, each a list of options:
   - `fonts`: four slots, `heading`, `body`, `caption`, `data`. Each slot lists font options.
   - `palette`: role-keyed palettes with `light` and `dark` modes.
   - `radius`: radius scales.
   - `elevation`: elevation sets (shadow layers per level).
   - `imagery`: imagery recipes; may be the single option `none`.
3. **Resolution.**
   - No archetype given: return the catalogue (id, name, one-line summary, surfaces, status). E15-20 later adds the preset ids to this listing.
   - Archetype given, some params missing: every group with exactly one option is filled automatically. If any group still has more than one option and no value, return `status: "needs_params"` with the option lists for the open groups. This is a success result, not an error.
   - A supplied value that is not in its group's option list fails with `style_param_invalid`, listing the allowed values and a corrected example.
   - All groups resolved: return the brief, the resolved values and a `resolved_params` object that `designos_apply_style` (E15-19) accepts verbatim.
4. **Catalogue content.** One archetype per persona in `ease-design/knowledge/personas/personas.json` (26 on 2026-10-18), rewritten for design:os in our own words. Owner ruling 2026-10-19: ship all 26 now; the owner's blind review later removes or reworks the ones that miss, so each archetype carries `status: "candidate"` until reviewed. Each archetype offers at least 2 palettes, 2 radius scales and 2 elevation sets, so the explorer (E15-36) can vary style inside one archetype.
5. **Font licence rule.** A font option is valid only when its `licence` is one of `OFL-1.1`, `Apache-2.0`, `MIT`, `UFL-1.0`, and its `source` is `bundled` (Inter) or `fontsource`. A persona font that fails this rule is replaced by a licensed font with a similar role (for example Inter for a system UI face) and the replacement is noted in the archetype's `source_notes`.
6. **Palette roles.** Every palette defines, in both modes, the required roles `background`, `foreground`, `primary`, `primary-foreground`, `border`, `ring`. Optional roles: `card`, `card-foreground`, `popover`, `popover-foreground`, `secondary`, `secondary-foreground`, `muted`, `muted-foreground`, `accent`, `accent-foreground`, `destructive`, `destructive-foreground`, `success`, `warning`, `info`, `chart-1` … `chart-5`, `shadow`. Missing optional roles are derived at compile time by the fixed rules in E15-19 §3.4, so every compiled document has the same variable names.
7. **Contrast gate (AC2).** For every palette and mode:
   - every declared text pair `[fg, bg]` must reach 4.5:1;
   - every declared non-text pair (`ring` on `background`, `border` is exempt) must reach 3:1;
   - ratios use the E15-10 (#595) contrast function on colours converted to the document profile (SC-6), with alpha composited over `background`.
   Default pairs, always checked when both roles exist: `foreground/background`, `card-foreground/card`, `popover-foreground/popover`, `primary-foreground/primary`, `secondary-foreground/secondary`, `muted-foreground/muted`, `accent-foreground/accent`, `destructive-foreground/destructive`, plus non-text `ring/background`.
8. **Type scale.** Each archetype declares `type_scale { base_px, ratio, steps }`. Steps are named `caption`, `body`, `body-lg`, `h4`, `h3`, `h2`, `h1`, `display`; size = `round_to_half(base_px × ratio^k)` with k from −1 (caption) to 6 (display). `ratio ∈ {1.125, 1.2, 1.25, 1.333, 1.5}` (the persona `type_scale_contrast` mapping flat 1.125, moderate 1.25, dramatic 1.5 plus two in between).
9. **Spacing.** `spacing { base_px ∈ {4, 8}, scale }` where `scale` is a strictly increasing list of multiples of `base_px` starting at 0.
10. **Catalogue version.** `catalogue.json` carries `version` (semver). Any archetype edit bumps it. E15-20 thumbnails and E15-19 `AppliedStyle` record this version.

## 4. Data model / schema

Archetype file (`assets/styles/archetypes/quiet-editorial.json`, abridged):

```json
{
  "schema": 1,
  "id": "quiet-editorial",
  "status": "candidate",
  "name": "Quiet editorial",
  "summary": "Paper-like calm, serif headlines, generous margins.",
  "surfaces": ["landing-page", "slides", "web-app"],
  "source_notes": "From ease-design persona quiet-luxury-editorial; heading face swapped to an OFL serif.",
  "brief": { "core": "…", "identity": "…", "layout": "…", "token_map": "…", "decoration": "…", "hierarchy": "…", "typography": "…" },
  "type_scale": { "base_px": 16, "ratio": 1.333 },
  "spacing": { "base_px": 8, "scale": [0, 8, 16, 24, 32, 48, 64, 96, 128] },
  "params": {
    "fonts": {
      "heading": [{ "id": "fraunces", "family": "Fraunces", "weights": [400, 600], "licence": "OFL-1.1", "source": "fontsource" }],
      "body":    [{ "id": "inter", "family": "Inter", "weights": [400, 500, 600], "licence": "OFL-1.1", "source": "bundled" }],
      "caption": [{ "id": "inter", "family": "Inter", "weights": [500], "licence": "OFL-1.1", "source": "bundled" }],
      "data":    [{ "id": "jetbrains-mono", "family": "JetBrains Mono", "weights": [400], "licence": "OFL-1.1", "source": "fontsource" }]
    },
    "palette": [{
      "id": "ink-on-paper", "space": "srgb",
      "modes": {
        "light": { "background": "#FAF8F3", "foreground": "#1C1B19", "primary": "#1C1B19", "primary-foreground": "#FAF8F3", "border": "#E4DFD4", "ring": "#6B5E45" },
        "dark":  { "background": "#141311", "foreground": "#EDE9E0", "primary": "#EDE9E0", "primary-foreground": "#141311", "border": "#2E2B26", "ring": "#B9A77F" }
      },
      "extra_pairs": [["muted-foreground", "background"]]
    }],
    "radius": [{ "id": "crisp", "values": { "none": 0, "sm": 2, "md": 4, "lg": 6, "xl": 8, "full": 9999 } }],
    "elevation": [{ "id": "flat-hairline", "levels": { "1": [], "2": [{ "x": 0, "y": 1, "blur": 2, "spread": 0, "color": "shadow", "alpha": 0.08, "inner": false }], "3": [] } }],
    "imagery": [{ "id": "none", "kind": "none" }]
  }
}
```

Rust (`designos-doc/src/style_archetype/schema.rs`), all `#[serde(deny_unknown_fields)]`:

```rust
pub struct Archetype { pub schema: u32, pub id: String, pub status: ArchetypeStatus, pub name: String, pub summary: String,
    pub surfaces: Vec<String>, pub source_notes: String, pub brief: Brief,
    pub type_scale: TypeScale, pub spacing: Spacing, pub params: ParamSources }
#[serde(rename_all = "kebab-case")] pub enum ArchetypeStatus { Candidate, Reviewed }   // owner ruling: all 26 ship as `candidate`; the owner's blind review later promotes or removes
pub struct Brief { pub core: String, pub identity: String, pub layout: String, pub token_map: String,
    pub decoration: String, pub hierarchy: String, pub typography: String }
pub struct ParamSources { pub fonts: FontSlots, pub palette: Vec<Palette>, pub radius: Vec<RadiusScale>,
    pub elevation: Vec<ElevationSet>, pub imagery: Vec<ImageryRecipe> }
pub struct FontSlots { pub heading: Vec<FontOption>, pub body: Vec<FontOption>, pub caption: Vec<FontOption>, pub data: Vec<FontOption> }
pub struct FontOption { pub id: String, pub family: String, pub weights: Vec<u16>, pub licence: FontLicence, pub source: FontSource }
pub struct Palette { pub id: String, pub space: ColorSpaceTag, pub modes: PaletteModes, #[serde(default)] pub extra_pairs: Vec<[String; 2]> }
pub struct PaletteModes { pub light: BTreeMap<String, String>, pub dark: BTreeMap<String, String> }   // role -> "#RRGGBB" or "#RRGGBBAA"
pub struct RadiusScale { pub id: String, pub values: BTreeMap<String, f32> }  // keys: none, sm, md, lg, xl, full
pub struct ElevationSet { pub id: String, pub levels: BTreeMap<String, Vec<ShadowLayer>> } // keys "1".."3"
pub struct ShadowLayer { pub x: f32, pub y: f32, pub blur: f32, pub spread: f32, pub color: String, pub alpha: f32, pub inner: bool }
pub struct ImageryRecipe { pub id: String, pub kind: ImageryKind, #[serde(default)] pub prompt: String,
    #[serde(default)] pub aspect_ratios: Vec<String>, #[serde(default)] pub avoid: Vec<String> }
pub enum ImageryKind { None, Photo, Illustration, Pattern, Abstract3d }
pub struct StyleParams { pub heading: Option<String>, pub body: Option<String>, pub caption: Option<String>, pub data: Option<String>,
    pub palette: Option<String>, pub radius: Option<String>, pub elevation: Option<String>, pub imagery: Option<String> }
pub struct ResolvedStyle { pub archetype: String, pub catalogue_version: String, pub params: StyleParams /* all Some */,
    pub brief: Brief, pub fonts: [FontOption; 4], pub palette: Palette, pub radius: RadiusScale,
    pub elevation: ElevationSet, pub imagery: ImageryRecipe, pub type_scale: Vec<(String, f32)>, pub spacing: Spacing }
pub fn catalogue() -> &'static Catalogue;                  // parsed once from include_str! data, OnceLock
pub fn resolve(id: &str, params: &StyleParams) -> Result<Resolution, StyleError>;
pub enum Resolution { NeedsParams { open: BTreeMap<String, Vec<String>>, filled: StyleParams }, Resolved(ResolvedStyle) }
```

The data files are embedded with `include_str!` through a generated list in `catalogue.rs`; a test fails when a file in `assets/styles/archetypes/` is missing from the list. No document format change in this task.

## 5. Contracts

`designos_get_guidelines` (tool added by #248) gains `category: "style"`:

Input:
```json
{ "category": "style", "archetype": "quiet-editorial", "params": { "palette": "ink-on-paper", "heading": "fraunces" } }
```

Output, needs params:
```json
{ "status": "needs_params", "archetype": "quiet-editorial",
  "filled": { "body": "inter", "caption": "inter", "data": "jetbrains-mono", "imagery": "none" },
  "open": { "radius": ["crisp", "soft"], "elevation": ["flat-hairline", "lifted"] } }
```

Output, resolved: `{ "status": "resolved", "resolved_params": {…all eight keys…}, "brief": {…}, "values": { "fonts": […], "palette": {…}, "radius": {…}, "elevation": {…}, "imagery": {…}, "type_scale": [["caption", 12], …], "spacing": {…} }, "catalogue_version": "1.0.0" }`.

Output, no archetype: `{ "status": "catalogue", "catalogue_version": "1.0.0", "archetypes": [{ "id", "name", "summary", "surfaces", "status" }] }`. E15-20 adds a top-level `presets` list.

Errors (SC-1):
- `style_archetype_unknown`, details `{ "allowed": [ids] }`, corrected example `{ "category": "style", "archetype": "quiet-editorial" }`, `safe_to_retry: true`.
- `style_param_invalid`, details `{ "param": "heading", "value": "Comic Sans", "allowed": ["fraunces"] }`, corrected example with the first allowed value, `safe_to_retry: true`.
- `style_param_unknown` for a key outside the eight, same shape.

## 6. File-format mapping

None. The catalogue is application data, not document data.

## 7. UI

None in this task (the gallery is E15-20).

## 8. Edge cases

1. A group with zero options in a data file fails the schema test (it can never resolve).
2. Palette colour with alpha (`#RRGGBBAA`): composited over the mode's `background` before contrast.
3. `display-p3` palette in an sRGB document: converted with gamut clipping (SC-6); the contrast check runs on the converted value.
4. Same option id in two slots (Inter as body and caption) is allowed; ids are unique within a slot only.
5. Params supplied for a single-option group with the only valid value: accepted silently; with another value: `style_param_invalid`.
6. Archetype file whose `id` differs from its file stem: schema test failure.
7. Unicode in `family` names: allowed; `id` must match `^[a-z0-9-]+$`.

## 9. Non-goals

- Compiling values into the document (E15-19).
- The gallery and thumbnails (E15-20).
- Generating imagery (E15-39); this task stores only the recipe.
- Prose guides per surface (E15-21).

## 10. Test plan

Seam: `cargo test -p designos-doc style_archetype` plus a schema check in CI (the same test, run in the existing `cargo test --workspace` job).

Fail-before: `catalogue_has_every_persona` fails on `main` (no catalogue).

| Test | Input | Expected |
|---|---|---|
| `every_archetype_validates` | all files | 0 schema errors, ids unique, ids equal file stems |
| `every_palette_passes_declared_pairs` (AC2) | all palettes × 2 modes | every text pair ≥ 4.5, ring ≥ 3.0 |
| `catalogue_has_every_persona` | persona slugs from a frozen copy of `personas.json` in `tests/fixtures/` | one archetype per slug (or a recorded substitution) |
| `no_param_call_lists_options` (AC3) | `resolve("quiet-editorial", {})` | `NeedsParams`, `open` contains every multi-option group |
| `bad_font_slot_fails_with_allowed_list` (AC3) | `heading: "comic-sans"` | `style_param_invalid`, `allowed == ["fraunces"]` |
| `single_option_groups_autofill` | `{}` on an archetype whose imagery has one option | `filled.imagery == "none"` |
| `every_archetype_ships_candidate` | all files | `status == "candidate"` for all 26 (owner ruling 2026-10-19) |
| `font_licences_are_embeddable` | all font options | licence in the allowed set, source in `{bundled, fontsource}` |
| `type_scale_formula` | base 16, ratio 1.25 | caption 16·1.25⁻¹ = 12.8 → 13, body 16, body-lg 20, h1 16·1.25⁵ = 48.83 → 49 |

Visual evidence: terminal capture of the test run plus `designos_get_guidelines` output for one archetype.

## 11. Performance budgets

- `catalogue()` parses once (`OnceLock`); parse of all archetypes ≤ 20 ms in a debug build.
- `resolve` ≤ 1 ms. No UI-thread work: the tool runs in `designos-mcp`.
- CLAUDE.md §4 invariants are untouched (no canvas, render or autosave code changes).

## 12. Dependencies and build order

- Needs #248 (the `get_guidelines` tool) and E15-10 (#595, contrast function).
- Unblocks E15-19 (#606), E15-20 (#608), E15-36, E15-39.

## 13. Owner rulings that apply

- Full scope: the whole persona set, not a sample.
- Content written for design:os from the ease-design knowledge (MIT, owner's own).
- Colour is Display P3 per document: palettes declare their space and are converted on write.
- Fonts limited to licences that allow embedding.
