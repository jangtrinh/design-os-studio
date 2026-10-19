# E15-19 Compile a style archetype straight into variables, modes and styles (`designos_apply_style`)

- Issue: #606 (task E15-19, epic #585, L / P0)
- Spec base: `main` at `767acf9`.
- Files: `crates/designos-doc/src/style_compile/{mod.rs,names.rs,derive.rs,tests.rs}` (new), `crates/designos-doc/src/lib.rs` (`Document.applied_style`), the MCP tool module `designos-mcp/src/tools/style.rs` (new) and its registration in `engine.rs` `tools()`.

## 1. Summary and user story

As an agent, I want one call that writes a resolved style (E15-18) into the document as variable collections with Light and Dark modes, text styles and effect styles, so I build with `var:` references and never transcribe values.

As a user, I want to switch the whole design to another style in one undo step, and get CSS-compatible names in code export.

## 2. Current state

- FACT: variables are `Variable { id, name, collection, kind, values: BTreeMap<mode id, VariableValue>, description, scopes, hidden_from_publishing, key, code_syntax }` (`crates/designos-doc/src/variables.rs:94-110`); collections hold `modes`, `default_mode` and an ordered `variables` list (`variables.rs:20-31`); values include `Alias` (`variables.rs:52-59`).
- FACT: shared styles are `Style::{Fill, Text, Effect, Grid}` in `Document.styles` keyed by string, with names and metadata in `style_meta` (`crates/designos-doc/src/lib.rs:150-153`, `lib.rs:782-787`).
- FACT: a style cannot hold a variable alias yet; #131 (E1-4, moved into this pack) adds aliases to colour, effect and text styles.
- FACT: CSS code export names a variable `Color/Primary` as `var(--Color-Primary)` (`crates/designos-export/src/codegen.rs:119-160`, `css_var_name` / `css_var`), and prints a WEB `code_syntax` verbatim when present (`codegen.rs:184-189`).
- FACT: the MCP engine applies document edits as `Command`s with an undo history; `Command::Batch(Vec<Command>)` exists (`crates/designos-doc/src/history.rs:74`).
- FACT: DTCG export is #138 (E1-9, moved into this pack).

## 3. Behaviour rules

1. **Input** is a `ResolvedStyle` from E15-18 (`archetype` + all eight params). The tool resolves the params itself; unresolved params fail with the E15-18 errors.
2. **Collections written** (fixed ids, so re-application finds them):

| Collection id | Name | Modes | Variables |
|---|---|---|---|
| `designos-style:color` | `Style / Color` | `light` "Light" (default), `dark` "Dark" | `color/<role>` for every role in §3.4, `color/shadow` |
| `designos-style:shape` | `Style / Shape` | `value` "Value" | `radius/none,sm,md,lg,xl,full` |
| `designos-style:space` | `Style / Space` | `value` | `space/<n>` for each scale step (`space/0`, `space/8`, …) |
| `designos-style:type` | `Style / Type` | `value` | `font/heading,body,caption,data` (String), `font-weight/<slot>` (Number), `font-size/<step>` and `line-height/<step>` (Number) |

3. **Variable ids** are deterministic: `designos-style/<name>` (for example `designos-style/color/primary`). Scopes: colours `ALL_FILLS`, `STROKE_COLOR`, `EFFECT_COLOR`; radius `CORNER_RADIUS`; space `GAP`, `WIDTH_HEIGHT`; font family `FONT_FAMILY`; weight `FONT_WEIGHT`; size `FONT_SIZE`; line height `LINE_HEIGHT`.
4. **Role derivation** (so every archetype writes the same names). Missing optional roles take, in order:
   - `card` ← `background`; `card-foreground` ← `foreground`; `popover` ← `card`; `popover-foreground` ← `card-foreground`;
   - `secondary` ← `muted` if present else mix(`background`, `foreground`, 0.06); `secondary-foreground` ← `foreground`;
   - `muted` ← mix(`background`, `foreground`, 0.06); `muted-foreground` ← mix(`foreground`, `background`, 0.40), then darkened/lightened in 2% OKLCH lightness steps until it reaches 4.5:1 on `muted`;
   - `accent` ← `secondary`; `accent-foreground` ← `secondary-foreground`;
   - `destructive` ← `#DC2626` light / `#EF4444` dark; `destructive-foreground` ← whichever of `#FFFFFF` and `#0A0A0A` has the higher contrast;
   - `success` `#16A34A`, `warning` `#D97706`, `info` `#0284C7` (same both modes); `chart-1…5` ← `primary` then hue rotations of +60°, +120°, +180°, +240° in OKLCH at the primary's lightness and chroma;
   - `shadow` ← `#000000`.
   `mix(a, b, t)` interpolates in OKLab. A derived alias is written as a real colour value, not an alias, so modes stay independent.
5. **Names are CSS-custom-property compatible.** Each variable name uses only `[a-z0-9-/]`; `code_syntax[WEB] = "var(--<name with / replaced by ->)"`, for example `var(--color-primary)`, `var(--radius-md)`, `var(--font-heading)`. Code export prints these verbatim (§2 FACT), so names survive export.
6. **Text styles** (keys `designos-style:text/<Name>`, for example `designos-style:text/Body Large`; the style's `StyleMeta.name` is `<Name>` itself, so `text_style: "Body Large"` resolves by E15-3's exact-name rule): `Display`, `H1`, `H2`, `H3`, `H4`, `Body Large`, `Body`, `Body Strong`, `Caption`, `Data`. Each binds `fontFamily` to the slot variable (heading slot for Display–H4, body for Body*, caption, data), `fontSize` and `lineHeight` to the step variables, `fontWeight` to the slot weight variable (Body Strong uses the body slot's highest weight). Step mapping: `Display`→`display`, `H1`→`h1`, `H2`→`h2`, `H3`→`h3`, `H4`→`h4`, `Body Large`→`body-lg`, `Body` and `Body Strong`→`body`, `Caption`→`caption`; `Data` uses the `body` step (E15-18 §3.8 has no `data` step, so there is no `font-size/data` variable). Line height = `round(size × 1.2)` for Display–H2, `round(size × 1.3)` for H3–H4, `round(size × 1.5)` for body, body-lg, caption and Data.
7. **Effect styles** (keys `designos-style:effect/elevation-<n>`, `StyleMeta.name` `Elevation/<n>`, so `effect_style: "Elevation/2"` resolves by E15-3's exact-name rule): always three, `Elevation/1`, `Elevation/2` and `Elevation/3` (the set's levels `"1"`..`"3"`, E15-18), including a level that has no layers, which is written with an empty effect list (`Style::Effect(vec![])`, so a template that binds `Elevation/2` never meets `UNKNOWN_STYLE`; an empty list renders no shadow). Each layer becomes `Effect::DropShadow` or `Effect::InnerShadow` (`paint.rs:284-298`). The colour binds to `color/shadow` with the layer's alpha applied through an alias-with-alpha when #131 supports it; otherwise the colour is written as a value and the style records `bound: false` in the report.
8. **Determinism and idempotency.**
   - Collections, variables and styles are written in a fixed order (table order, then name order).
   - Re-applying the same resolved style produces no change: the tool compares the would-be state with the current one and returns `changed: false` with no undo entry (AC1).
   - Applying another archetype rewrites values under the same ids and names. Nodes bound with `var:` references therefore re-theme (AC2).
   - Variables the compiler did not create are never touched. A user-made variable whose name collides with a compiler name in a different collection is left alone; the compiler's variable lives in its own collection.
9. **One undo step.** All writes are one `Command::Batch`. `dry_run: true` returns the full report without changing the document and uses the E15-4 (#589) transaction preview shape.
10. **Active style record.** `Document.applied_style = Some(AppliedStyle { archetype, params, catalogue_version })` (exactly the §4 struct; no revision or timestamp field, so a re-apply of the same style leaves the document hash unchanged) is written in the same batch. E15-16 bootstrap and E15-20 read it.
11. **References.** Agents and templates address every name this compiler writes through E15-3's collection-qualified form: the collection id, then `/`, then the variable name. Examples: `var:designos-style:color/color/primary`, `var:designos-style:shape/radius/md`, `var:designos-style:space/space/16`, `var:designos-style:type/font-size/h1`. E15-3 §3.1 step 1 resolves it by collection id and never considers a same-named variable in another collection, so a user variable also called `color/primary` cannot capture the binding (edge case 1). The unqualified `var:color/primary` still works while exactly one collection holds that name; with a user variable of the same name it is `AMBIGUOUS_VARIABLE` (E15-3 step 2).
12. **Mode selection.** Optional `mode: "light" | "dark"` sets the explicit mode of the `Style / Color` collection on the given `scope_node_id` (default: the active page) through `NodeVariables.modes` (`variables.rs:135-141`).

## 4. Data model / schema

```rust
// designos-doc/src/lib.rs, on Document
#[serde(default, skip_serializing_if = "Option::is_none")]
pub applied_style: Option<AppliedStyle>,

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppliedStyle { pub archetype: String, pub params: StyleParams, pub catalogue_version: String }

pub fn compile_style(doc: &Document, style: &ResolvedStyle) -> StylePlan;   // pure
pub struct StylePlan { pub commands: Vec<Command>, pub report: StyleReport }
pub struct StyleReport { pub collections: Vec<String>, pub variables_created: u32, pub variables_updated: u32,
    pub text_styles: u32, pub effect_styles: u32, pub unbound_effect_layers: u32, pub changed: bool }
```

Back-compat: old documents have no `applied_style` (`None`). Nothing else in the format changes; collections, variables and styles use existing types.

## 5. Contracts

`designos_apply_style` input:
```json
{ "archetype": "quiet-editorial",
  "params": { "palette": "ink-on-paper", "heading": "fraunces", "radius": "crisp", "elevation": "flat-hairline" },
  "mode": "light", "scope_node_id": 12, "dry_run": false }
```
Output:
```json
{ "changed": true, "dry_run": false,
  "applied": { "archetype": "quiet-editorial", "params": { "…all eight…" }, "catalogue_version": "1.0.0" },
  "report": { "collections": ["designos-style:color", "designos-style:shape", "designos-style:space", "designos-style:type"],
              "variables_created": 66, "variables_updated": 0, "text_styles": 10, "effect_styles": 3, "unbound_effect_layers": 0 },
  "undo_group": 1 }
```
Errors (SC-1): E15-18's `style_archetype_unknown`, `style_param_invalid`, `style_param_unknown`; `style_needs_params` (details: the `open` lists; corrected example fills each open group with its first option; `safe_to_retry: true`); `style_collection_locked` when a `designos-style:*` collection is a subscribed library copy (`VariableCollection::is_library`, `variables.rs:38`), `safe_to_retry: false`.

## 6. File-format mapping

- `.designos`: native fields above.
- `.fig` export: collections, variables, text and effect styles export through the existing variable and style paths; `applied_style` is design:os-only and is not exported.
- DTCG (#138): the result exports as `color.primary` with `$type: "color"` and one `$extensions` mode map per #138's format. AC3 validates it.

## 7. UI

None (E15-20 adds the picker). The existing Variables panel shows the four `Style / …` collections.

## 8. Edge cases

1. A document already holding user variables named `color/primary` in another collection: both coexist; the report lists the name collision as a warning. Bindings written with the qualified form (rule 11) address the compiler's variable only; an unqualified `var:color/primary` is `AMBIGUOUS_VARIABLE` and the corrected example is the qualified form.
2. Re-apply after a user edited a compiler variable's value: the compiler overwrites it (the style owns those names) and the report counts it under `variables_updated`.
3. Archetype switch where the new radius scale lacks a key: impossible by schema (fixed key set); a test enforces it.
4. Font not installed locally: variables still written; `designos-fonts` fetches on render. The report lists fonts not yet cached.
5. `mode: "dark"` on a node inside a library instance: refused with `style_scope_invalid` (corrected example: the page id).
6. Dry run on an unchanged document: `changed: false`, empty command list.
7. Very large document (1M nodes): compile cost is independent of node count; re-theme cost is the existing variable-resolve path.

## 9. Non-goals

- Re-binding existing hard-coded values to the new variables (that is lint + auto-fix, E15-11/E15-13).
- Per-frame different styles (E15-36 adds extra modes for that).
- Fill styles: colour variables cover fills.

## 10. Test plan

Seam: `cargo test -p designos-doc style_compile` plus a render snapshot through `designos-render`.

Fail-before: `apply_twice_is_identical` fails on `main` (no compiler).

| Test | Input | Expected |
|---|---|---|
| `apply_twice_is_identical` (AC1) | apply A, hash; apply A again | hashes equal (zero-allocation document hash), second call `changed: false`, undo depth +1 total |
| `frame_rethemes_on_switch` (AC2) | frame with `fill: "var:designos-style:color/color/primary"`, apply A then B | resolved fill equals B's `primary`; render snapshot differs from A's at the frame |
| `qualified_ref_ignores_user_variable_with_same_name` | document with a user collection holding `color/primary` (another colour); frame binds `var:designos-style:color/color/primary`; apply A then B | the binding points at the compiler variable (id `designos-style/color/primary`), the frame re-themes, the user variable is untouched; unqualified `var:color/primary` returns `AMBIGUOUS_VARIABLE` |
| `empty_elevation_level_still_has_a_style` | archetype whose elevation set has `"1": []` and `"3": []`; apply, then bind `effect_style: "Elevation/1"` through `apply_spec` | `Elevation/1`, `Elevation/2`, `Elevation/3` all exist; level 1 has an empty effect list; the bind succeeds and draws no shadow; `effect_styles` is 3 |
| `dtcg_export_validates` (AC3) | apply A, export DTCG (#138) | validates against the DTCG schema fixture used by #138 |
| `derivation_fills_every_role` | palette with only required roles | every role of E15-18 §3.6 (27 names, `shadow` included) present in both modes |
| `muted_foreground_reaches_aa` | derived roles | `muted-foreground/muted ≥ 4.5` |
| `names_are_css_safe` | every compiler name | matches `^[a-z0-9-]+(/[a-z0-9-]+)*$`; `code_syntax[WEB]` equals `var(--…)` form |
| `single_undo_step` | apply then undo | document equals the pre-apply hash |
| `dry_run_changes_nothing` | dry run | document hash unchanged, report non-empty |

Visual evidence: render snapshot PNGs of one sample frame under two archetypes, side by side.

## 11. Performance budgets

- Compile ≤ 5 ms; apply on a 112k-node document ≤ 50 ms plus the existing re-resolve cost, measured in the test log.
- No `doc.to_json()` for comparisons: equality uses the zero-allocation document hash (CLAUDE.md §4.1).
- Viewport state untouched; only the variable revision changes, so the canvas repaints once.

## 12. Dependencies and build order

- Needs E15-18 (#604), #131 (E1-4), #138 (E1-9), E15-4 (#589, dry-run shape), E15-3 (#588; AC2 binds a fill with the qualified `var:designos-style:color/color/primary`, and the text and effect style links resolve by E15-3's name rule).
- Unblocks E15-20 (#608), E15-22 (#610), E15-36.

## 13. Owner rulings that apply

- Full scope: every collection, mode and style type listed.
- Variables JSON is Figma-native plus DTCG.
- Colour is Display P3 per document: palette colours are converted to the document profile on write (SC-6).
