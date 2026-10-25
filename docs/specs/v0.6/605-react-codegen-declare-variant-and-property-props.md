# #605 React codegen: declare variant and property props on generated components

| Field | Value |
|---|---|
| Issue | #605 (follow-up of #496, which fixed the same gap for Compose and SwiftUI) |
| Size / priority | M / P2 |
| Depends on | none |
| Unblocks | E15-45 (verify generated code) can reuse the opt-in type-check test |
| Touches | `crates/designos-export/src/slots.rs` (`react_slot_signature`, `prop_params`, `slot_ident`), `crates/designos-export/src/react.rs` (`react_with`), `crates/designos-export/src/tailwind.rs` (`instance_attrs`, `:521`), `compose.rs` and `swiftui.rs` (shared renamer only), tests in `crates/designos-export/tests/slot_codegen.rs` |

Code baseline: `origin/main@01d48f2` (0.5.0).

## 1. Summary and user story

As a developer copying React code for a component and for an instance of it, every attribute the
instance passes (a variant choice, a boolean, a text, a swapped icon) is a declared, typed prop of the
generated component, so the TSX compiles and the instance's values reach the component.

## 2. Current state (FACT)

- `react_with` emits the function header through `react_slot_signature`
  (`crates/designos-export/src/react.rs:25-37`, `slots.rs:429-446`). That header lists only slot
  props, typed `React.ReactNode`; with no slot it is `export function Name() {` (`slots.rs:432-434`).
- An instance passes its non-default values and every variant selection as attributes
  (`instance_args`, `crates/designos-export/src/codegen.rs:296-314`; written by `instance_attrs`,
  `tailwind.rs:521-532`: `name`, `name={false}`, `name="text"`, `name={<Icon />}`). Those names are
  not declared on the definition.
- The non-slot properties of a definition are already modelled for the other two languages:
  `prop_params` (`slots.rs:315-340`, camelCase name, `PropDefault::{Bool, Text, Instance}`) feeds
  `slot_function` in Compose (`compose.rs:25-40`) and `slot_view` in SwiftUI (`swiftui.rs:29-57`).
  The #496 tests are `compose_function_declares_every_argument_its_instance_passes` and
  `swiftui_init_declares_every_argument_its_instance_passes`
  (`crates/designos-export/tests/slot_codegen.rs:305-358`). React has no twin.
- Name safety exists only for slots: `slot_ident` (`slots.rs:167-179`) appends `Slot` to JS, Swift or
  Kotlin reserved words. `prop_params` names are raw `camel(clean_prop_name(..))`; the Compose
  function always declares `modifier` itself (`compose.rs:27`), so a property named `modifier`
  produces a duplicate parameter.
- `prop_defs(doc, comp)` returns a component's own properties plus its set's
  (`designos-doc/src/components.rs:359-370`). A set root therefore lists the set-level properties only;
  a property defined on one variant (`defined_on_variant`, `components.rs:1656`) is passed by
  instances of that variant but missing on the set definition.
- A TSX parse check exists but is opt-in: `generated_react_parses`
  (`tests/slot_codegen.rs:370-402`) pipes the output to `npx --yes esbuild --loader=tsx`; it is
  `#[ignore]` and needs network. No test or CI job type-checks anything: `.github/workflows/ci.yml`
  only runs cargo (build, test, clippy, fmt) and `tsc` is not installed on the dev machine.

## 3. Behaviour rules

1. The React header is built from the definition's full parameter list: non-slot properties
   (`prop_params`, in declaration order) followed by slots, in one destructured props object with a
   matching type literal:
   `export function Button({ label = 'Save', disabled = false, size = 'S', icon = <Star />, children }: { label?: string; disabled?: boolean; size?: 'S' | 'L'; icon?: React.ReactNode; children?: React.ReactNode }) {`
   (line breaks per `Out` conventions; shown on one line here).
2. Types: Boolean property `boolean`; Text `string`; Variant the union of its options as string
   literals (`'S' | 'L'`; `string` when the options list is empty); Instance-swap `React.ReactNode`;
   Slot `React.ReactNode`. Every prop is optional (`?`) because each has a default.
3. Defaults come from `PropDefault`: booleans and text as literals, a variant's selected option, an
   instance swap as `<Default />` or omitted (`undefined`) when the default names no component.
4. A component with properties and no slots gets the same destructured header; today it exports a
   bare `export function Name() {`. A component with neither keeps `export function Name() {`
   byte for byte (existing goldens `golden_inline_card`, `golden_tailwind_card` stay green).
5. Set root: the declared properties are the union (by cleaned name, first definition wins) of the
   set's own properties and the properties defined on each of its variant components. Variant
   components inside the set export the same header as their set.
6. Name safety through one shared function `prop_ident(name, lang, taken)` in `slots.rs`:
   - a JS reserved word (`JS_RESERVED`, `slots.rs`), `key` or `ref` (React-reserved prop names) gets the
     suffix `Prop`; Compose: `modifier` and the Kotlin keywords; Swift: its keywords (the lists
     `slot_ident` already uses);
   - a name equal to an earlier property or to a slot identifier gets a numeric suffix (`size2`), in
     declaration order, so the result is deterministic;
   - the definition and every instance call compute the names from the same ordered list
     (`prop_params(comp)` plus `slot_infos(comp)`), so `instance_attrs`, the Compose call arguments and
     the SwiftUI call use the identical identifier. Compose and SwiftUI adopt `prop_ident` only for
     this collision handling; their output for non-colliding names does not change.
7. Instance output: attributes are written under the renamed identifier (`<Dialog classProp="x" />`).
8. The destructured defaults and the instance attributes never use `children` unless a slot owns that
   name (`react_prop`, `slots.rs:420-427`): a property named `children` is renamed `childrenProp`.

## 4. Data model / schema changes

None. `PropParam` gains no field; a new `PropParam::ts_type(&self)` helper (or an enum of option
lists carried from `PropKind::Variant { options }`) feeds the type literal. `Lang::React` already
exists.

## 5. Contracts

Public functions `react_for` and `react_with` keep their signatures; the generated text changes only
for components with properties (section 3). The Dev Mode code panel and the MCP codegen tool
(`designos_generate_code`, `designos-mcp/src/engine.rs:112`) show the new text.

## 6. File-format mapping

None.

## 7. UI

None (the Dev Mode code panel renders the string; check it in the visual evidence).

## 8. Edge cases

1. Variant property with a single option: `size?: 'S'`.
2. A variant property name containing a space or `#id` suffix (`Size#2:2`): cleaned and camelCased
   (`size`), as `instance_args` does.
3. Two properties whose cleaned names collide (`Label` and `label`): `label`, `label2`; the instance uses
   the same two names in the same order.
4. A property named like a slot (`header` text prop and `header` slot): property keeps `header`, slot
   becomes `header2` through the shared taken-list (slots keep `slot_ident` suffix `Slot` first when
   reserved).
5. Component set with a property only on the second variant: appears in the set header; the first
   variant's instance does not pass it.
6. Instance of a component with no properties and no slots: no attributes, unchanged.
7. Properties plus Tailwind output (`ReactOptions { tailwind: true }`): same header; `className` is not a
   property name collision because instance classes are written separately (`tailwind.rs:549`); a
   property named `className` is renamed `classNameProp`.
8. Text default containing a quote or backslash: JS string escaping (`js_value`, `react.rs:62`).

## 9. Non-goals

- Controlled-component logic, state, or event props.
- A published `.d.ts` or a package.
- Adding a TypeScript toolchain to CI.

## 10. Test plan (red first)

Seams: always-on string tests in `crates/designos-export/tests/slot_codegen.rs` using the existing
builders (`dialog`, `two_slot_dialog`, `add_props`), plus one opt-in type-check test.

| Case | Steps | Expected (fails before) |
|---|---|---|
| `react_function_declares_every_argument_its_instance_passes` | `two_slot_dialog` + `add_props`; `react_for(comp)` and `react_for(inst)` | the instance's attribute names (`label`, `disabled`, `size`, `icon`, `header`, `body`) are each a key of the definition's destructured object and type literal, in order |
| `react_component_with_props_and_no_slots_declares_them` | component with Text + Boolean props only | header has both, not `Name()` |
| `react_variant_prop_is_a_literal_union` | Variant with options `S`, `L` | `size?: 'S' \| 'L'` |
| `react_set_root_declares_variant_level_props` | set with a property on one variant only | union in the set header |
| `react_reserved_and_colliding_prop_names_are_renamed` | props named `class`, `key`, `children`, `header` (slot too) | `classProp`, `keyProp`, `childrenProp`, `header` + `header2`; instance attributes match |
| `compose_prop_named_modifier_is_renamed` | property named `modifier` | no duplicate `modifier:` parameter; call uses the renamed name |
| `swiftui_prop_keyword_is_renamed` | property named `class` | renamed in init and call |
| `goldens_unchanged` | existing `golden_*` tests | still pass untouched |
| `generated_react_type_checks` (opt-in, `#[ignore]`) | write each output to a temp dir with a synthetic `react.d.ts` stub (declares `namespace React { type ReactNode = unknown }` and `JSX.IntrinsicElements`), run `npx --yes -p typescript@5.5.4 tsc --noEmit --jsx preserve --strict` on definition plus instance in one file | exit 0 |

Run: `cargo test -p designos-export`; opt-in: `cargo test -p designos-export -- --ignored`.
Evidence: red/green terminal capture, the ignored type-check run pasted into the PR (owner-manual or
worker-run with network; it is not claimed as CI), and a Dev Mode screenshot of the code panel for a
component with properties.

## 11. Performance budgets

Codegen is called on demand; the added work is one pass over a component's property list. Budget:
no measurable change on a 1,000-layer component (record `cargo test` time before and after).

## 12. Dependencies and build order

Independent. `slots.rs` is shared with other codegen work; serialize with any change to the slot
emitters.

## 13. Owner rulings

- Developer-facing output only; no UI ruling needed.
- Delivery law 2 (honest reporting): the PR does not claim a CI type-check that does not exist.

## Could not ground

- A TSX type-check gate in CI. None exists on `origin/main@01d48f2`, and adding Node to the Rust CI
  workflow is outside this task, so the type-check is an opt-in `#[ignore]` test like
  `generated_react_parses`, run and pasted by the worker. The pinned `typescript` version
  (`5.5.4`) is a placeholder; the worker confirms it against the npm registry before use.
