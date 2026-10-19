# E15-3 Bind fills, strokes, numbers and styles to document variables from apply_spec (`var:` references)

| Field | Value |
|---|---|
| Epic / pack | E15 / v0.6 AI design |
| Size / priority | L / P0 |
| Depends on | none (E15-11/E15-12 consume its output) |
| Unblocks | E15-1 overrides with `var:`, T7.9 `var:` paths, part-2 style archetypes |
| Touches | `crates/designos-doc/src/spec.rs` (`resolve_easeui_color`, `apply_easeui_preset`), new `crates/designos-doc/src/spec_bind.rs` |

## 1. Summary and user story

As an agent, I write `fill: "var:Color/Primary"`, `gap: "var:Spacing/4"` or
`text_style: "Heading L"`, and the engine creates real variable bindings and style links against the
open document, so a theme or mode switch restyles my layers like hand-made ones.

## 2. Current state (FACT)

- Spec colours go through `resolve_easeui_color` (`crates/designos-doc/src/spec.rs:145-170`): a
  fixed table of 19 `$easeui.*` tokens to RGB, then `Color::from_hex`. Unknown strings return `None`
  and the fill is silently skipped (`spec.rs:485-487`, stroke `spec.rs:491-493`).
- Presets write raw colours and numbers (`apply_easeui_preset`, `spec.rs:201-355`).
- Numeric spec fields are plain `f32` (`spec.rs:59-98`, `LayoutSpec` `spec.rs:19-42`); there is no
  `line_height` field.
- Bindings model: `NodeVariables { bindings: property key -> variable id, modes: collection id ->
  mode id }` (`crates/designos-doc/src/variables.rs:135-140`); explicit mode lookup
  `variable_mode` (`variables.rs:154`) and `resolve_variable` (`variables.rs:172`).
- Property keys and kinds: `property_kind` (`crates/designos-doc/src/variables_apply.rs:21-32`),
  scope check `scope_allows` (`variables_apply.rs:36`), value writer `set_bound_property`
  (`variables_apply.rs:153`).
- Styles: `Style` (`crates/designos-doc/src/lib.rs:782-787`), names in `StyleMeta.name`
  (`lib.rs:770-778`), node links in `Node.style_refs` (`crates/designos-doc/src/node.rs:109-110`),
  value application `apply_style_values` (`crates/designos-doc/src/library_ops.rs:975`).

## 3. Behaviour rules

### 3.1 Variable reference syntax

`var:<path>` or `var:id:<variable id>`. `var:id:` names one variable exactly and is never ambiguous.
A `<path>` resolves in this order (precedence: qualified, then unqualified, then library); the first
step that matches stops the search, and more than one match inside a step is `AMBIGUOUS_VARIABLE`:

1. Qualified, `<collection>/<variable name>`. `<collection>` is a collection **id** or an exact
   collection **name**; either may itself contain `/` (the part-2 style compiler's collection is named
   `Style / Color`, its id is `designos-style:color`), so every collection whose id or name `c`
   satisfies `path` starts with `c` + `/` is a candidate and the longest `c` wins. The remainder is the
   variable name, matched exactly inside that collection (the name may contain `/`). A hit ends
   resolution: a variable with the same name in another collection is never considered. Two candidates
   of the same length (two collections with one name, or one collection's id equal to another's name)
   is ambiguous. If a collection matches but holds no such variable, resolution continues with step 2,
   and step 2 compares the whole path, not the remainder, so a typo after a qualified prefix cannot
   bind a variable of another collection.
2. Unqualified: the whole path equals a variable name in exactly one collection. Two collections
   holding that name is `AMBIGUOUS_VARIABLE`; an agent that needs one of them writes step 1 with the
   collection id, or `var:id:`.
3. Enabled library variables, same two steps; a hit is imported with the library import command in the
   same trial document.

Examples (collections `Color`, id `c1`, holding `Primary`; and a collection with id `c2` holding a
variable named `Color/Primary`): `var:Color/Primary` is step 1, collection `Color`, the `c1` variable,
`c2`'s is never considered; `var:c2/Color/Primary` is step 1 by id and names the `c2` variable.

Producers (lint fixes in E15-10, E15-11 and E15-13, search results in T7.9, `export_spec`) emit the name form `var:<collection name>/<variable name>` only when it resolves back to that same variable under the three steps above; otherwise they emit `var:<collection id>/<variable name>`, or `var:id:<id>` when even that is ambiguous. A reference a producer wrote therefore never fails with `AMBIGUOUS_VARIABLE`.

No match: `UNKNOWN_VARIABLE` with the 5 closest `collection/name` strings by Levenshtein distance
(ties broken by collection order, then name); a candidate whose collection name occurs twice is listed
with its collection id instead of its name.

### 3.2 Bindable spec fields

| Spec field | Property key(s) | Kind |
|---|---|---|
| `fill` (non-text) / `fill` or `text_color` (text) | `fills/0` | Color |
| `stroke` | `strokes/0` | Color |
| `layout.gap` / `layout.counter_gap` | `itemSpacing` / `counterAxisSpacing` | Number |
| `layout.padding[i]` | `paddingTop/Right/Bottom/Left` | Number |
| `corner_radius` | all four `*Radius` keys | Number |
| `corner_radii[i]` | `topLeftRadius` .. `bottomLeftRadius` | Number |
| `stroke_weight` | `strokeWeight` | Number |
| `font_size` / `line_height` (new) | `fontSize` / `lineHeight` | Number |
| `opacity` | `opacity` | Number |

Each numeric field accepts a number or a `var:` string (`NumberOrVar`). The binding is inserted into
`node.variables.bindings` and the resolved value is written with `set_bound_property`, so the layer
renders correctly before any mode switch.

Checks, all before allocation:
- kind: `property_kind(key)` must equal `Variable.kind` (an alias is checked on its resolved kind)
  → else `VARIABLE_TYPE_MISMATCH` (expected and actual kind).
- scope: `scope_allows(scopes, key, node kind)` → else `VARIABLE_SCOPE_MISMATCH`, listing the
  variable's scopes and, as `valid_options`, up to 5 in-scope variables of the same kind.

### 3.3 Style references

- `fill: "style:Brand/Primary"` and `stroke: "style:..."` link a colour (Fill) style.
- `text_style: "Heading L"` on text nodes links a Text style; `effect_style: "Elevation/2"` links an
  Effect style. The `style:` prefix is optional on these two fields.
- Name match is exact on `StyleMeta.name`; library styles are imported like variables. The style
  values are applied with `apply_style_values` and the key goes into `style_refs`.
- Wrong style type for the field: `STYLE_TYPE_MISMATCH`. Unknown: `UNKNOWN_STYLE` (5 closest).
- A field may carry a style or a variable or a literal, never two; explicit `font_size` together
  with `text_style` overrides the style's size and drops the link with a `STYLE_DETACHED_BY_SPEC`
  warning.

### 3.4 Modes

`mode: {"<collection name>": "<mode name>"}` on any container sets
`node.variables.modes[collection id] = mode id`, inherited by descendants. Unknown collection or mode:
`UNKNOWN_VARIABLE_MODE` listing the collection's modes.

### 3.5 EaseUI aliases (back-compat)

- `$easeui.*` tokens and `easeui-*` presets keep working. On first use in a document, the engine
  inserts a collection named `EaseUI (deprecated)` with one mode `Default` and one Color variable per
  canonical token, holding exactly the RGB in the existing table (`spec.rs:148-166`). Alias spellings
  (`$easeui.panel` = `$easeui.bg.panel`) bind to the same variable.
- Every token use binds that variable instead of writing a raw colour. Preset colours bind the same
  way; preset numbers stay raw (the table has no number tokens).
- Each use adds a warning `DEPRECATED_EASEUI_TOKEN` with the replacement `var:EaseUI (deprecated)/
  bg/panel` and the spec path. Rendering is bit-identical to today because the bound value equals the
  old literal.
- `transparent` / `none` keep clearing the paint list (`spec.rs:483-484`); they are not tokens.

### 3.6 Literal values

Hex stays allowed. A hex equal to exactly one document colour variable is reported by the issues block
(E15-12, rule `hardcoded-value` from E15-11) with a Bind fix; an ambiguous match is reported without a
fix (R2.7). Unparseable colour strings stop being silently skipped: `INVALID_COLOR` with the accepted
forms.

## 4. Data model / schema changes

- `NodeSpec`: numeric bindable fields change from `f32`/`Option<f32>` to
  `NumberOrVar`/`Option<NumberOrVar>` (`#[serde(untagged)] enum NumberOrVar { Num(f32), Ref(String) }`);
  JSON numbers deserialize unchanged. New fields `line_height: Option<NumberOrVar>`,
  `text_style: Option<String>`, `effect_style: Option<String>`, `mode: BTreeMap<String, String>`.
- `LayoutSpec.gap`, `counter_gap` become `NumberOrVar`; `padding` becomes `[NumberOrVar; 4]` (a JSON
  array of numbers still parses).
- `export_spec` emits `var:` strings for bound properties, `style:`/`text_style` for linked styles and
  `mode` for explicit modes, so export → apply keeps bindings. Today it exports raw values only
  (`spec.rs:614-709`).
- The `EaseUI (deprecated)` collection is ordinary document data (`variable_collections`), persisted
  in `.designos` and exported to `.fig` like any local collection.

## 5. Contracts

Tools: `designos_apply_spec` and the `apply_spec` op of `designos_apply_batch`.

Reply additions:

```json
{"bindings_created": 14, "styles_linked": 3,
 "warnings": [{"code": "DEPRECATED_EASEUI_TOKEN", "path": "spec.children[0].fill",
               "token": "$easeui.bg.panel", "replacement": "var:EaseUI (deprecated)/bg/panel"}]}
```

Errors (envelope per E15-5; `safe_to_retry: true`):

| Code | `valid_options` | Corrected example |
|---|---|---|
| `UNKNOWN_VARIABLE` | 5 closest paths | same field with the closest path |
| `AMBIGUOUS_VARIABLE` | each `var:id:<id>` with collection | `var:id:` form of the first |
| `VARIABLE_TYPE_MISMATCH` | variables of the expected kind (5) | first one |
| `VARIABLE_SCOPE_MISMATCH` | in-scope variables (5) | first one |
| `UNKNOWN_STYLE` / `STYLE_TYPE_MISMATCH` | 5 closest of the right type | first one |
| `UNKNOWN_VARIABLE_MODE` | mode names | first mode |
| `INVALID_COLOR` | `["#RRGGBB", "#RRGGBBAA", "var:...", "style:..."]` | none |

## 6. File-format mapping

- `.designos`: bindings and modes are existing fields; nothing new.
- `.fig`: node paints export `colorVar` already (`crates/designos-fig/src/fig_export.rs:664-688`);
  number bindings and modes follow the existing variable export. Round-trip test only.

## 7. UI

No new surface. Bound layers show the variable chip in the existing properties panel
(`crates/designos-app/src/panels/var_bind.rs`), which is the visual evidence for the PR.

## 8. Edge cases

1. Variable is an alias chain: kind and scope are checked on the resolved target; the binding points
   at the referenced variable, not the target.
2. Variable has no value in the node's effective mode: binding is created, value falls back to the
   collection default mode, warning `VARIABLE_MODE_FALLBACK`.
3. `corner_radius: "var:Radius/md"` on a text node: text has no radius keys
   (`bindable_keys`, `variables_apply.rs:68-80`) → `OVERRIDE_FIELD_NOT_APPLICABLE`.
4. Document already contains a user collection named `EaseUI (deprecated)` with different values:
   the engine uses a collection with the reserved id `easeui-deprecated`; a name clash appends
   ` (2)` and never rebinds to the user's collection.
5. `mode` on a frame whose collection has one mode: accepted; the explicit mode is stored even
   though it equals the default, so a mode added later does not change this frame.
6. A literal and a `var:` for the same property in a preset plus explicit field: the explicit field
   wins, as today (`spec.rs:416-418` applies presets first).

## 9. Non-goals

- Creating variables or styles from a spec (part-2 style archetype tasks).
- Grid styles.
- Removing the EaseUI alias path (a later deprecation release decides).

## 10. Test plan

Seam: designos-doc unit tests in `spec_bind.rs`; CPU render snapshot for the alias path
(`designos-render` `render_node`, bit compare).

Fixture `fixtures::tokens_doc()`: collection `Color` (modes Light, Dark) with `Primary` (#0D99FF /
#3582FF), collection `Spacing` with `4` = 16, text style `Heading L` (24/700), effect style
`Elevation/2`.

| Case | Input | Expected |
|---|---|---|
| `binds_fill_and_mode_switch_recolours` | rect `fill:"var:Color/Primary"`, frame `mode:{"Color":"Dark"}` | `bindings["fills/0"]` = Primary id; resolved fill #3582FF |
| `unknown_variable_lists_5_closest` | `var:Color/Primery` | `UNKNOWN_VARIABLE`, `valid_options[0] == "Color/Primary"`, len ≤ 5, doc unchanged |
| `number_binding_gap` | `layout.gap:"var:Spacing/4"` | `itemSpacing` bound, value 16 |
| `text_style_link` | `text_style:"Heading L"` | `style_refs["text"]` set; font size 24 |
| `easeui_alias_bit_identical` | existing test spec from `spec.rs:804` (`easeui-screen`, `$easeui.*`) | render bytes equal to a golden produced on main before the change; one `DEPRECATED_EASEUI_TOKEN` warning per use |
| `type_mismatch` | `font_size:"var:Color/Primary"` | `VARIABLE_TYPE_MISMATCH` |
| `qualified_by_collection_id` | collections `Style / Color` (id `designos-style:color`) with `color/primary` and `Brand` (id `b1`) with `color/primary`; `fill:"var:designos-style:color/color/primary"` | binds the `Style / Color` variable; the `Brand` one is untouched |
| `qualified_name_with_slash` | same document; `fill:"var:Style / Color/color/primary"` | binds the `Style / Color` variable (longest collection prefix) |
| `unqualified_same_name_is_ambiguous` | same document; `fill:"var:color/primary"` | `AMBIGUOUS_VARIABLE`, `valid_options` has the id-qualified form of both, doc unchanged |
| `qualified_miss_does_not_cross_collections` | same document; `fill:"var:designos-style:color/color/primry"` | `UNKNOWN_VARIABLE`; no binding to the `Brand` variable |
| `producer_emits_id_form_when_name_form_is_ambiguous` | two collections both named `Brand`, each with `Color/Primary`; a lint `bind` fix targeting the second | `to` is `var:<second collection id>/Color/Primary`; applying it binds the second; with unique names the fix keeps the name form |
| `qualified_ignores_other_collection_same_name` | collection `Color` with `Primary` and collection `X` with a variable named `Color/Primary`; `var:Color/Primary` | binds `Color`'s `Primary`, no ambiguity |
| `export_apply_keeps_bindings` | export then apply | equal binding maps |

Fail-before: `binds_fill_and_mode_switch_recolours` fails today (fill silently skipped,
`spec.rs:485`).

## 11. Performance budgets

One name index for variables and one for styles per call (O(variables + styles)). Budget: 1,000
bindings in one spec on a document with 5,000 variables apply in under 20 ms release. No
`apply_bound_variables` full-document clone (`variables_apply.rs:306`) on the write path.

## 12. Dependencies and build order

No upstream dependency. Build in parallel with E15-1 (different module); E15-1 overrides reuse
`spec_bind::resolve_colour_ref` for `fill`/`stroke`.

## 13. Owner rulings

- R2.7 (D-E3-3): token suggestions use exact value matches only; ambiguous matches are reported
  without a fix (applies to 3.6).
- Full scope, never MVP: variables, styles, modes and the alias path ship together.
- All AI through the user's agent CLI: engine-side resolution, no model.
