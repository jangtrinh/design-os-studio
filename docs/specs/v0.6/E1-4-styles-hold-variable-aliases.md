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
   - Fill styles: `fills/<i>` (colour) and `fills/<i>/opacity` (number);
   - Text styles: `fontSize`, `lineHeight`, `letterSpacing`, `paragraphSpacing`, `fontWeight` (number),
     `fontFamily`, `fontStyle` (string);
   - Effect styles: `effects/<i>/color`, `effects/<i>/radius`, `effects/<i>/spread`,
     `effects/<i>/offsetX`, `effects/<i>/offsetY`.
   Kind checks reuse `property_kind` semantics (`variables_apply.rs:21-32`) extended with the new keys;
   a mismatch is refused by the editing APIs.
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
| `.fig` (kiwi) | style node paints: `colorVar` per paint (same encoding as node paints); text style variable fields and effect variable fields: field names to be confirmed by the REA step E1-b with `rea` (morluto/rea) on Figma's schema before coding — UNKNOWN until then |
| Figma REST JSON | `boundVariables` on style-defining nodes imported into `bindings` when present |

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
| `text_style_size_binding` | text style `fontSize` bound → linked text resolves size per mode (after REA confirms export fields, the `.fig` round trip of this case is added) |

## 11. Performance budgets

Resolution cost per linked node: O(bindings of its styles); styles are looked up by key (BTreeMap).
No extra full-document pass; the large-fixture render budget is unchanged (recorded once in the PR).

## 12. Dependencies and build order

Hot-file queue position 22. The REA step precedes `.fig` coding. Before part-2 E15-19.

## 13. Owner rulings

- REA rule: reverse-engineering of Figma fields uses `rea` (morluto/rea).
- Full scope: colour, text and effect style bindings ship together.
- Nothing vanishes silently: broken bindings are reported, not dropped.
