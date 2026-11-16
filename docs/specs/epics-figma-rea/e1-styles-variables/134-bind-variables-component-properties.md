# #134 Bind variables to component properties (boolean, text, variant) (E1-5c)

| Field | Value |
|---|---|
| Issue | #134 (E1-5c, P2). Milestone on GitHub: Backlog; body says v0.8 |
| Still real? | Yes. No binding path for component properties exists on main (evidence below) |
| Size | L: about 12–16 builder hours. That includes a 2 h REA spike, the doc model and resolution, the `.fig` round trip, and the instance-panel bind entry points |
| Tier | Claude Sonnet. The instance-sync and override code is judgment-heavy, and the work sits on the hot-file queue |
| Depends on | REA spike (step 0); #1664 merged (hot files `fig.rs`, `fig_export.rs`) |
| Ready after owner answers? | Yes for the doc, `.fig` and resolution scope. The variant leg waits on the REA spike (parity fact, not an owner question) |

Code baseline: `origin/main@d22dad970`.

## 1. Outcome

On an instance, a designer binds a component property to a variable:
- a boolean property ("Show icon") to a boolean variable;
- a text property ("Label") to a string variable;
- a variant property ("Size") to a string variable, if Figma allows it (step 0).

Switching the variable mode flips or relabels the instance. The binding survives `.fig` save and reopen and `.designos` save.

## 2. Re-anchor (origin/main)

- FACT: `PropValue` is `Bool | Text | Variant | Instance | Slot`, with no variable reference (`crates/designos-doc/src/node.rs:227-236`). Instance values are set by `set_instance_prop` (`crates/designos-doc/src/components.rs:2055`) and applied in `apply_instance_props_with_ctx` (`crates/designos-doc/src/spec_instance.rs:1023`).
- FACT: `property_kind` has no component-property key (`crates/designos-doc/src/variables_apply.rs:21-44`).
- FACT (schema): `ComponentPropAssignment { defID=1, value=2, varValue=3: VariableData }` and `ComponentPropDef.varValue=9` (`crates/designos-fig/src/figma_schema.txt`, the `ComponentPropAssignment` and `ComponentPropDef` messages, about lines 1825-1840). `VariableData { value: VariableAnyValue, dataType, resolvedDataType }`, and `VariableAnyValue.alias=4` (schema:2620-2630, :2744-2747).
- FACT: import reads `varValue.value` only through literal branches (`boolValue`, `textValue`, `guidValue`, `symbolIdValue`) in `component_props` / `held` (`crates/designos-fig/src/fig.rs:2071-2087`). An `alias` in `varValue` is dropped silently. `fig_export.rs` writes `varValue` with literals only (`:1371`, `:1403`, `:1419`).
- FACT (Figma Plugin API, developers.figma.com `InstanceNode`, read 2026-11-06): `setProperties` takes `string | boolean | VariableAlias`. The node-level `boundVariables` has `componentProperties?: { [propertyName]: VariableAlias }`. The docs do not say which property types accept an alias.
- UNKNOWN: whether Figma lets a VARIANT property bind to a string variable, and which variable types each property type accepts. Closing check: REA step 0.

## 3. Scope and rules

0. **REA spike (about 2 h, `rea` per the global rule)** on the Figma editor bundle and on a scratch Figma file the owner allowed. Answer three questions:
   - (a) Which property types show a bind affordance: BOOLEAN, TEXT, VARIANT? INSTANCE_SWAP is expected to show none.
   - (b) Where the alias is stored in kiwi: assignment `varValue` with `dataType=ALIAS`, or something else.
   - (c) What happens to a variant binding whose string matches no option.

   Write the facts into `plans/reports/re-figma-<date>-component-prop-variable-binds.md`.
1. **Model.** The binding lives in the instance's `variables.bindings` under the key `componentProperties/<prop name>`, mirroring Figma's `boundVariables.componentProperties`. `property_kind` maps it by the property definition's kind: Boolean → `Boolean`, Text → `String`, Variant → `String` (only if step 0 confirms). No new `PropValue` variant is needed, so serde and old files are unchanged.
2. **Resolution.** In `resolve_node` order, a bound property's resolved value replaces the stored `PropValue` before `apply_instance_props_with_ctx` runs. The stored value stays the literal fallback, the way other bindings keep their stored value.
3. **Edit rule.** Setting the property by hand in the instance panel removes its binding in the same undo step, as for other bound fields.
4. **Variant with no matching option.** Follow the step 0 result. Until it is known, the builder keeps the current variant and records a lint issue, never a crash.
5. **.fig round trip.** Import an `alias` in `ComponentPropAssignment.varValue` as a binding. Export a binding as `varValue { dataType: ALIAS, value { alias } }` plus the literal fallback in `value`, as Figma writes it (step 0 b).
6. **Inspector.** Each boolean, text and variant row in the instance properties panel gets the always-visible bind icon (#876 rule) and a pill when bound. The variable list is filtered by kind.
7. **MCP.** `designos_update_node` / `apply_batch` accept the binding key the same way as other binding keys. The builder checks the existing binding write path accepts any key `property_kind` knows.

## 4. Files likely touched

`crates/designos-doc/src/variables_apply.rs`, `components.rs`, `spec_instance.rs`; `crates/designos-fig/src/fig.rs` (`component_props`), `fig_export.rs` (the `varValue` writers); `crates/designos-app/src/panels/` (instance properties panel; builder locates the file) and `panels/var_bind.rs`.

## 5. Acceptance criteria

- **AC-1** A Button component has a boolean prop "Show icon". An instance binds it to a boolean variable with Light = true and Dark = false. Under Dark, the icon layer of the instance is hidden, and the main is unchanged. This is red on main.
- **AC-2** A text prop bound to a string variable shows that mode's string in the instance's text layer. Switching the mode re-measures hug text.
- **AC-3** `.fig` round trip: an instance with both bindings is saved and reopened, and both bindings are present and resolve per mode. A Figma-written fixture whose assignment has `varValue.dataType = ALIAS` imports as a binding. This is red on main, where the alias is dropped.
- **AC-4** `.designos` save/reopen keeps the binding. A document without component bindings serializes byte-identical to main.
- **AC-5** Editing the property by hand removes the binding in one undo step, and undo restores it.
- **AC-6** (UI) A desktop capture at 1440 of the instance panel shows the bind icon and a bound pill on a boolean row and a text row. Zero Fake UI gate passes.
- **AC-7** (variant, only if REA confirms) A variant bound to a string variable switches the variant per mode.

## 6. Test steps

`cargo test -p designos-doc components`, `cargo test -p designos-fig` (new `fig_component_property_variable_bindings_round_trip.rs`), `cargo test -p designos-app --bin designos`, then the workspace gates and the 1440 capture.

## 7. Edge cases

- A nested instance whose outer instance exposes the property: the binding sits on the instance that owns the assignment.
- A deleted variable: the binding is dangling. The stored fallback is used, and the lint flags it, as for other dangling bindings.
- A library variable: aliasing goes through the existing library variable import.
- Swapping the instance to another component without the property: drop the binding with the property. Nothing silently remains, and the lint has nothing stale to report.
- A component set whose variants define the property through `parentPropDefId` (`fig.rs:2069-2070`).

## 8. Non-goals

- INSTANCE_SWAP and SLOT binding (the Plugin API throws for SLOT).
- Binding on the main component's property *definition* default (`ComponentPropDef.varValue`). It is read and written verbatim if present. If step 0 shows it is needed, file it separately.

## 9. Figma parity

Checked: schema fields and the Plugin API surface (above). Needs a live Figma check: step 0 (a)–(c).

## 10. Builder rules

Hot-file queue position after #132 and #133-A. `Closes #134`. Visual evidence: terminal captures plus the 1440 capture.
