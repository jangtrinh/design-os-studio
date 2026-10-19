# E1-4 Let colour, effect and text styles hold variable aliases

| Field | Value |
|---|---|
| Issue | #131 (moved from v0.8 Variables everywhere to v0.6 AI design) |
| Size / priority | L / P1 |
| Depends on | none (REA step E1-b before `.fig` coding) |
| Unblocks | part-2 E15-19 (`designos_apply_style` compiles archetypes into variable-bound styles) |
| Touches | `crates/designos-doc/src/lib.rs` (`StyleMeta`), `crates/designos-doc/src/styles_ops.rs`, `crates/designos-doc/src/variables_apply.rs`, `crates/designos-fig/src/fig.rs`, `crates/designos-fig/src/fig_export.rs`, `crates/designos-fig/src/rest_import.rs` |
| Hot file | merge position 22 in the single hot-file queue (fig.rs / fig_export.rs / kiwi.rs / rest_import.rs); claim only after position 21 merged |

## 1. Summary and user story

As a designer (or E15-19 compiling a style archetype), I make a colour style whose paint is the
variable `Color/Primary`, a text style whose size is `Type/Body`, an effect style whose shadow colour is
`Color/Shadow`; switching modes restyles every layer linked to those styles, and the bindings survive
`.fig` round trips.

## 2. Current state (FACT)

- `Style` is `Fill(Vec<Paint>) | Text(TextStyle) | Effect(Vec<Effect>) | Grid(Vec<LayoutGrid>)`
  (`crates/designos-doc/src/lib.rs:782-787`); `Paint` carries no binding; `StyleMeta { name, key,
  description, property }` (`lib.rs:770-778`).
- Node bindings exist only on nodes (`NodeVariables.bindings`, `crates/designos-doc/src/variables.rs:135-140`)
  and are resolved per node and mode by `resolve_node` / `resolved_node` / `apply_bound_variables`
  (`crates/designos-doc/src/variables_apply.rs:283-306`).
- Linking a node to a style copies the style values into the node (`apply_style_values`,
  `crates/designos-doc/src/library_ops.rs:975`).
- `.fig` node paints export their `colorVar` (`crates/designos-fig/src/fig_export.rs:664-688`); style
  nodes' variable fields are not modelled. REST import builds styles from node values
  (`crates/designos-fig/src/rest_import.rs:1149-1156`). (The issue cites `fig.rs:927,957`; those lines
  have moved on current main and now hold unrelated metadata, so the exact import site is re-located
  during implementation.)

## 3. Behaviour rules

1. `StyleMeta.bindings: BTreeMap<String, String>`: property key → variable id. Keys:
   - Fill styles: `fills/<i>` (colour), `fills/<i>/stops/<j>` (colour of gradient stop `j` of fill `i`,
     section 6, edge 5) and `fills/<i>/opacity` (number);
   - Text styles: `fontSize`, `lineHeight`, `letterSpacing`, `paragraphSpacing`, `paragraphIndent`,
     `fontWeight` (number), `fontFamily`, `fontStyle` (string). `fontWeight` and `fontStyle` are two
     keys that pack into one kiwi `FONT_STYLE` entry (`asFloat` / `asString`, section 6);
   - Effect styles: `effects/<i>/color`, `effects/<i>/radius`, `effects/<i>/spread`,
     `effects/<i>/offsetX`, `effects/<i>/offsetY`.
   Kind checks reuse `property_kind` semantics (`variables_apply.rs:21-32`) extended with the new keys
   (`fills/<i>/stops/<j>` already falls under its `fills/` colour arm); a mismatch is refused by the
   editing APIs.
2. Style values remain stored (they are the default-mode resolution and the fallback when a variable is
   missing).
3. Resolution: for a node linked to a style (`Node.style_refs`), `resolve_node` resolves the style's
   bindings in the node's mode context (`variable_mode`, `variables.rs:154`) and applies them to the
   node's linked properties, unless the node itself binds the same property (node binding wins).
   Rendering, export and codegen use the resolved node as today.
4. Mode switch: changing a frame's explicit mode re-resolves its linked descendants on the next
   resolution; no document mutation is needed beyond the mode change.
5. Editing: `styles_ops` gains `bind_style_property(doc, key, property, variable) -> Command` and
   `unbind_style_property`; both are undoable through `Command::SetStyles` (it carries `style_meta`,
   `crates/designos-doc/src/history.rs:39-43`).
6. Alias semantics: a style binding to an alias variable resolves through the alias chain in the
   node's mode context, exactly like node bindings.
7. Check designs and the E15-11 matcher count a node linked to a variable-bound style as bound for the
   bound properties (no `hardcoded-value` finding).

## 4. Data model / schema changes

- `StyleMeta.bindings` with `#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]`: old files
  load unchanged; files without bindings save byte-identically.
- The `.fig` mapping (section 6) adds no document fields.

## 5. Contracts

- MCP: `designos_get_document` style entries gain `bindings` (`crates/designos-mcp/src/engine.rs:50-54`
  builds them from `style_meta`). The E15-4 `set_style` op accepts `bindings` on upsert.
- E15-3 `style:` references link styles; the bound behaviour follows automatically.

## 6. File-format mapping

| Format | Mapping |
|---|---|
| `.designos` | `StyleMeta.bindings` |
| `.fig` (kiwi) | RESOLVED (Facts below). Style nodes are ordinary `NodeChange`s with `styleType` (163) set. Fill style: `fillPaints` (38) `[i].colorVar` (21) → `fills/<i>`; gradient stops `[i].stopsVar` (23) `[j].colorVar` (2) → `fills/<i>/stops/<j>`; `[i].opacityVar` (38) → `fills/<i>/opacity`. Effect style: `effects` (43) `[i]` `colorVar` (10), `radiusVar` (9), `spreadVar` (11), `xVar` (12) → `offsetX`, `yVar` (13) → `offsetY`. Text style: `variableConsumptionMap` (307) entries `{variableField, variableData}` with `VariableField` `FONT_SIZE` (32), `LINE_HEIGHT` (36), `LETTER_SPACING` (34), `PARAGRAPH_SPACING` (2), `PARAGRAPH_INDENT` (3), `FONT_FAMILY` (28), `FONT_STYLE` (29), `FONT_VARIATIONS` (30); `fontWeight` and `fontStyle` both live in the one `FONT_STYLE` entry (`VariableFontStyle.asFloat` / `.asString`) |
| Figma REST JSON | RESOLVED. The `styles` map carries metadata only (`key`, `name`, `description`, `remote`, `styleType`). Values and bindings come from the style node (`GET /v1/files/:key/nodes?ids=<style node_id>`): `fills[i].boundVariables.color`, `fills[i].gradientStops[j].boundVariables.color`, `style.boundVariables.{fontFamily,fontSize,fontStyle,fontWeight,letterSpacing,lineHeight,paragraphSpacing,paragraphIndent}` (TEXT node `TypeStyle`), `effects[i].boundVariables.{color,radius,spread,offsetX,offsetY}` (shadows; blurs: `radius` only). Each value is `{"type":"VARIABLE_ALIAS","id":"VariableID:…"}` |

## 7. UI

The style editor (`crates/designos-app/src/panels/style_edit.rs`) shows a variable chip on bound style
properties using the existing variable-binding control (`crates/designos-app/src/panels/var_bind.rs`);
binding and unbinding are one undo step each. Load es-designer before UI code; desktop capture at 1440;
responsive SKIPPED.

## 8. Edge cases

1. Style bound to a variable deleted later: the stored style value is used; the binding is reported by
   a lint finding `broken-style-binding` (E15-9 registry addition, category tokens) and kept until the
   user unbinds (nothing vanishes silently).
2. Node overrides one property of its linked style (detaches that property): node value wins; the
   style link remains for other properties, as today.
3. Variable from a disabled library: resolves from the local library copy if present; else as 1.
4. Two modes with different font families bound in a text style: font loading follows the resolved
   family (existing font request path).
5. Gradient fill styles: only `fills/<i>` for solid paints and gradient stop colours `fills/<i>/stops/<j>`
   are bindable; other gradient parameters are not (Figma parity, confirmed by REA).
6. Instance children linked to a bound style: resolved like main components (mirrors follow).

## 9. Non-goals

- Grid style bindings.
- Creating variables from styles (E15-19).

## 10. Test plan

Seams: designos-doc mode-switch test and `.fig` round trip.

| Case | Expected |
|---|---|
| `mode_switch_recolours_style_linked_layer` | style `Brand` fill bound to `Color/Primary` (Light #0D99FF, Dark #3582FF); rect linked to `Brand` in a frame with mode Dark → resolved fill #3582FF; Light → #0D99FF |
| `node_binding_wins` | same rect also binds `fills/0` to another variable → node's variable used |
| `fig_style_colorvar_round_trips` | export_fig then import → `StyleMeta.bindings["fills/0"]` equals the variable (by key) |
| `check_designs_counts_bound` | linked rect produces no hard-coded finding for `fills/0` |
| `old_file_byte_identical` | load and save a file without bindings → identical bytes |
| `text_style_paragraph_indent_and_weight_round_trip` | text style with `paragraphIndent` bound to a number variable and `fontWeight` bound to a number variable and `fontStyle` to a string variable | `.fig` writes a `PARAGRAPH_INDENT` entry and one `FONT_STYLE` entry carrying `asFloat` and `asString`; re-import restores all three bindings; REST import of `paragraphIndent`, `fontWeight`, `fontStyle` `boundVariables` yields the same keys; nothing is dropped |
| `fill_style_gradient_stop_binding_round_trips` | gradient fill style with stop 1 bound to a colour variable (`fills/0/stops/1`) and stop 0 unbound | `.fig` writes `stopsVar[1].colorVar` only and re-imports `bindings["fills/0/stops/1"]`; REST import of `fills[0].gradientStops[1].boundVariables.color` yields the same key; an unbound stop stays unbound; with the variable bound in Dark the linked layer's stop 1 resolves per mode |
| `text_style_size_binding` | text style `fontSize` bound → linked text resolves size per mode, and the `.fig` round trip writes one `variableConsumptionMap` entry `{variableField: FONT_SIZE, variableData: ALIAS}` on the style node and reads it back |
| `effect_style_shadow_vars_round_trip` | effect style shadow with `color`, `radius`, `spread`, `offsetX`, `offsetY` bound → `.fig` writes `colorVar`/`radiusVar`/`spreadVar`/`xVar`/`yVar` and re-imports the same five bindings |

## 11. Performance budgets

Resolution cost per linked node: O(bindings of its styles); styles are looked up by key (BTreeMap).
No extra full-document pass; the large-fixture render budget is unchanged (recorded once in the PR).

## 12. Dependencies and build order

Hot-file queue position 22. The REA step precedes `.fig` coding. Before part-2 E15-19.

## 13. Owner rulings

- REA rule: reverse-engineering of Figma fields uses `rea` (morluto/rea).
- Full scope: colour, text and effect style bindings ship together.
- Nothing vanishes silently: broken bindings are reported, not dropped.

## 14. Facts (REA 2026-10-18)

Method. Read-only. Sources: the kiwi schema compiled into Figma's cached editor wasm, the cached editor JS, one Figma-saved community kit `.fig` (counts only; no content copied), and the public REST OpenAPI and plugin typings. `rea` (morluto/rea 6.7.0, `npx -y rea-agents`) ran `analyze-javascript-application` over the extracted JS chunks (evidence `ev_2624766…`). Its wasm lanes were unavailable on this machine: WABT and Ghidra are not installed. So the schema was decoded directly from the wasm data section with a 40-line kiwi `decodeBinarySchema` reader.

1. FACT. Schema source: `~/Library/Application Support/Figma/DesktopProfile/v43/Cache/Cache_Data`, entry `static.figma.com/fullscreen/c36343e1…/fullscreen-wasm/compiled_wasm.wasm.br` (brotli, 52 MB). The binary kiwi schema (653 definitions) starts at byte 49,396,956 with `MessageType`. Three other cached builds (`8a9f81ff…`, `065cabdf…`, `8d77463d…`) have identical ids for every field below.
2. FACT. Paint and stop fields:
   ```
   message Paint { … VariableData opacityVar = 38; VariableData colorVar = 21; ColorStopVar[] stopsVar = 23; … }
   message ColorStopVar { Color color = 1; VariableData colorVar = 2; float position = 3; }
   ```
3. FACT. Effect fields:
   ```
   message Effect { … VariableData radiusVar = 9; VariableData colorVar = 10;
     VariableData spreadVar = 11; VariableData xVar = 12; VariableData yVar = 13; … }
   ```
4. FACT. Text-style numeric bindings use the node's consumption map:
   ```
   NodeChange: VariableDataMap variableConsumptionMap = 307;  StyleType styleType = 163;
   message VariableDataMapEntry { uint nodeField = 1; VariableData variableData = 2; VariableField variableField = 3; }
   enum VariableField { PARAGRAPH_SPACING = 2; PARAGRAPH_INDENT = 3; FONT_FAMILY = 28; FONT_STYLE = 29; FONT_VARIATIONS = 30; FONT_SIZE = 32; LETTER_SPACING = 34; LINE_HEIGHT = 36; … }
   ```
   `VariableData { VariableAnyValue value = 1; VariableDataType dataType = 2; VariableResolvedDataType resolvedDataType = 3; }`. An alias is `VariableAnyValue.alias = 4` → `VariableID { GUID guid = 1; AssetRef assetRef = 2; }`.
5. FACT. Font weight and style: `VariableAnyValue.fontStyleValue = 9` → `VariableFontStyle { VariableData asString = 1; VariableData asFloat = 2; VariableData asVariations = 3; }`. The kiwi `VariableField` has no FONT_WEIGHT. A weight binding is the `FONT_STYLE` entry with `dataType = FONT_STYLE (8)` and `asFloat` = ALIAS of a FLOAT variable (seen in the kit below). INFERENCE: a string style binding uses `asString`.
6. FACT, observed in a Figma-saved kit (`fig-kiwi` version 106, 56,625 nodes). Its schema has the same ids as the wasm for `Effect`, `Paint` and `VariableDataMapEntry`; the wasm only adds newer fields.

   | Style | Style nodes | Bindings observed |
   |---|---|---|
   | TEXT (node type TEXT) | 334 | `variableConsumptionMap` entries: FONT_FAMILY ×332, FONT_SIZE ×332, FONT_STYLE ×290 (`asFloat` alias), LINE_HEIGHT ×176, LETTER_SPACING ×6 |
   | EFFECT (node type ROUNDED_RECTANGLE) | 49 | `colorVar` ×33, `radiusVar` ×43, `spreadVar`/`xVar`/`yVar` ×25 each |
   | FILL (node type ROUNDED_RECTANGLE) | 16 | `fillPaints[].colorVar` ×4 |

   All of them are `dataType = ALIAS`. `nodeField` was unset in every entry, so key on `variableField`.
7. FACT. REST (`figma/rest-api-spec` openapi.yaml 0.45.0) has these `boundVariables` keys. `SolidPaint`: `color`. `ColorStop`: `color`. `TypeStyle`: `fontFamily, fontSize, fontStyle, fontWeight, letterSpacing, lineHeight, paragraphSpacing, paragraphIndent`. `BaseShadowEffect`: `radius, spread, color, offsetX, offsetY`. `BaseBlurEffect`: `radius`. The `Style` metadata schema has `key, name, description, remote, styleType` and no values. `VariableAlias = {type: "VARIABLE_ALIAS", id}`.
8. FACT. Plugin API (`@figma/plugin-typings` 1.142.0) has these binding types. `TextStyle.boundVariables: {[VariableBindableTextField]?: VariableAlias}`, with the same eight text fields as REST. `PaintStyle.boundVariables: {paints?: VariableAlias[]}` and `EffectStyle.boundVariables: {effects?: VariableAlias[]}` are read-only. Per-paint and per-effect bindings: `VariableBindablePaintField = 'color'`, `VariableBindableColorStopField = 'color'`, `VariableBindableEffectField = 'color'|'radius'|'spread'|'offsetX'|'offsetY'`.
9. Gaps against rule 1, for the spec owner. Nothing was changed here.
   - Figma also binds `paragraphIndent`. RESOLVED in the 2026-10-19 review: rule 1 now lists it, with a round-trip test.
   - Figma has no separate `fontWeight` numeric field in kiwi (see 5).
   - `fills/<i>/opacity` exists in kiwi (`opacityVar`) but is exposed by neither REST nor the Plugin API.
