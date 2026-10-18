# #152 Emit slots in codegen and in Dev Mode inspect

- Issue: https://github.com/jangtrinh/design-os-studio/issues/152 (task E3-9, epic #16, M / P2)
- Spec base: `main` at `767acf9`.
- Open PR: #496 (`task/152-slots-codegen-and-dev-inspect`, +476 −25). The release review, head `a5241ee`, found:
  1. MAJOR: the slot parameter and the slot's own default children both render;
  2. MAJOR: every named slot falls back to the same `children`, and instance export flattens every slot into one list;
  3. MAJOR: the SwiftUI snippet does not compile, and a slot named Body becomes `body()`;
  4. MINOR: Inspect counts slot items only among the instance's direct children.

  The owner ruled on 2026-10-18 that the release session fixes this PR itself.
- Files: `crates/designos-export/src/codegen.rs`, `react.rs`, `tailwind.rs`, `swiftui.rs`, `compose.rs`; `crates/designos-app/src/dev/data.rs`, `panels/inspect.rs`, `shell/tests/dev.rs`.

## 1. Summary and user story

As a developer reading Dev Mode for a slotted component, I want:
- generated code that exposes each slot as an idiomatic content parameter (React children or ReactNode props, SwiftUI `@ViewBuilder`, Compose `@Composable` lambdas);
- instance snippets that pass each slot's content to the right parameter;
- Inspect to tell me which layer is a slot and what it holds.

## 2. Current state

- FACT: `instance_args` (`designos-export/src/codegen.rs:285-302`) skips `PropValue::Slot` (`codegen.rs:298`: `(_, PropValue::Slot(_)) => continue`).
- FACT: there is no slot handling in `react.rs` (`react_for` at `react.rs:17`), `swiftui.rs` (`swiftui_for` at `:12`) or `compose.rs` (`compose_for` at `:12`).
- FACT: Dev Mode `component_rows` (`designos-app/src/dev/data.rs:161-177`) prints `PropValue::Slot(ids)` as `"{n} item(s)"` for instances. It shows nothing for a slot frame or a main component.
- FACT: Dev Mode code is produced through `Language::generate` (`dev/mod.rs:47`).

## 3. Behaviour rules

1. **Slot detection.** A node is a slot when `component.is_slot` is set, or it has an `OverrideField::SlotContent` binding (`is_slot_container`, `components.rs:208`). Its parameter name is the bound prop's base name (the text before `#`):
   - camelCase for React and Compose;
   - for SwiftUI, camelCase plus the suffix `Content` (rule 5).
2. **Slot set of a component.** The slot params, in document pre-order, de-duplicated by prop name.
3. **React and Tailwind, component definition.**
   - **Exactly one slot:** the signature has `{ children, ...props }` and the type `children?: React.ReactNode`. At the slot container, emit `{children ?? <>{DEFAULT}</>}`, where `DEFAULT` is the JSX of the slot's current children. If the slot has no children, emit `{children}`. Do not also emit the default children as siblings (MAJOR 1).
   - **Two or more slots:** one named prop per slot, `header?: React.ReactNode`, and so on. At each container emit `{header ?? <>{DEFAULT}</>}`. Never fall back to `children` (MAJOR 2).
4. **React and Tailwind, instance snippet.**
   - **One slot with overridden content:** `<Card ...props>{CONTENT}</Card>`.
   - **Several slots:** `<Card header={<>{CONTENT_H}</>} body={<>{CONTENT_B}</>} />`.
   - Content for each slot is the children of the instance's slot node for that prop, found by walking descendants and stopping at nested instances.
   - A slot whose content equals the main default is omitted.
5. **SwiftUI, component definition.**
   - Emit a struct with one generic parameter per slot, `struct Card<HeaderContent: View, BodyContent: View>: View`.
   - Stored properties are `let headerContent: HeaderContent`.
   - An init with `@ViewBuilder headerContent: () -> HeaderContent` assigns `self.headerContent = headerContent()`.
   - The slot container emits `headerContent`.
   - The identifier is always `<camel>Content` (`bodyContent`), never `body` (MAJOR 3).
   - Default content: emit a constrained extension init (`extension Card where BodyContent == <DefaultType>`) only when there is exactly one slot and its default is a single `Text`. Otherwise the parameter is required.
6. **SwiftUI, instance snippet.**
   - One slot: a trailing closure, `Card { CONTENT }`.
   - Several: labelled closures, `Card(headerContent: { ... }, bodyContent: { ... })`.
7. **Compose, component definition.** Parameters `header: @Composable () -> Unit = { DEFAULT }`, defaulting to `{}` when the slot is empty. The container calls `header()`.
8. **Compose, instance snippet.** One slot: a trailing lambda, `Card(...) { CONTENT }`. Several: named arguments, `Card(header = { ... }, body = { ... })`.
9. **CSS, UIKit and Android XML:** no slot construct. They emit the slot as a plain container, as today. Unchanged.
10. **Dev Mode Inspect.**
    - **Main component:** a `Slots` row group, one row per slot param, showing `name · Slot · {n} default item(s)`.
    - **Slot frame:** the layer subtitle shows `Slot`, and a row reads `Property: <name>`.
    - **Instance:** a row per slot showing `name · Slot · {n} item(s)`, where n counts the children of that slot node found by walking descendants, not only direct children (MINOR 4).
    - Rows are read-only text.

## 4. Data model / schema changes

None.

## 5. Contracts

In `codegen.rs`:

```rust
pub struct SlotParam { pub prop: String, pub ident: String, pub node: NodeId }
pub fn component_slots(doc: &Document, comp: NodeId) -> Vec<SlotParam>;
pub fn instance_slot_content(doc: &Document, inst: NodeId) -> Vec<(SlotParam, Vec<NodeId>)>; // grouped per slot
```

The existing `react_for`, `swiftui_for` and `compose_for` signatures are unchanged.

## 6. File-format mapping

None. Codegen reads the document model. RESOLVED.

## 7. UI

- Dev Mode Inspect rows use the existing row style: label 11 px Inter, value `theme::font_mono()` (P5). There are no new controls (P6).
- The code block shows the new output, and the copy action copies it.
- Evidence: a 1440 capture of Inspect on a slotted component and on an instance.

## 8. Edge cases

1. A Dialog with one slot `Body` whose default is the text `Title`.
   - React: `{children ?? <><p>Title</p></>}`, with no extra `<p>Title</p>` sibling.
   - Compose: `body: @Composable () -> Unit = { Text("Title") }`.
2. A Card with `Header` (`Title`) and `Body` (`Message`).
   - React signature `{ header, body }`, no `children`.
   - Instance `<Card header={<><p>Hi</p></>} />`, when only the header is overridden.
3. A slot named `Body` in SwiftUI gives `bodyContent`. The output compiles with `swiftc -typecheck` (test rule below).
4. A slot nested under a padding frame: the instance's Inspect row count reflects the slot's children (MINOR 4).
5. A slot prop name with spaces or `#` suffix (`Main content#12:3`): the ident is `mainContent`.
6. Two slots whose names camelCase to the same ident: the second gets a `2` suffix (`body2`).
7. A component with no slots: output identical to `main` (golden unchanged).

## 9. Non-goals

- Code Connect mappings.
- Slot constructs for CSS, UIKit and Android XML.
- Typed slot content (restricting to preferred components) in generated types.

## 10. Test plan

Seams: golden tests in `crates/designos-export` (`codegen.rs` fixture `slot_card` from the PR, extended) and `crates/designos-app/src/shell/tests/dev.rs`.

| Test | Expected (literal) | Fails before |
|---|---|---|
| `react_single_slot_uses_children_with_default_and_no_duplicate_siblings` | contains `{children ?? <><p>Title</p></>}`; `<p>Title</p>` occurs exactly once | yes on PR head |
| `react_two_slots_use_named_props_and_never_children` | signature `{ header, body }`; no `children` token | yes |
| `react_instance_groups_content_per_slot` | `header={<>…</>}` and `body={<>…</>}` | yes |
| `swiftui_slot_component_typechecks` | write output plus a stub `import SwiftUI` file, run `swiftc -typecheck` when `swiftc` is on PATH, skip with message otherwise | yes |
| `swiftui_body_slot_is_named_body_content` | contains `bodyContent`, not `var body: () ->` | yes |
| `compose_slots_are_named_lambdas_with_defaults` | `header: @Composable () -> Unit = { Text("Title") }` | yes |
| `dev_inspect_counts_nested_slot_children` | `Body · Slot · 2 item(s)` | yes on PR head |
| `golden_unchanged_for_components_without_slots` | byte-equal to `main` goldens | guard |

Golden assertions must compare whole strings, not `contains` only, for the slot fixtures (review: "the tests only `contains`-check").

## 11. Performance budgets

`CLAUDE.md` §4.1: codegen runs on demand. Dev Mode must not regenerate per frame. It regenerates only on `rev` or selection change, through the existing `dev_refresh` (`dev/mod.rs:147`).

## 12. Dependencies and build order

- Depends on #31 (merged).
- Benefits from #108 (per-property ids), which is not required.
- Independent of the fig queue. It can run in parallel with the slot UI tasks.

## 13. Owner rulings that apply

- R4 (brainstorm): native Dev Mode languages are CSS, SwiftUI, UIKit, Compose and Android XML, plus Studio's React and Tailwind exporters.
- A5 (follow Figma naming) applies to variable names, not slot identifiers.
- Owner ruling of 2026-10-18: the release session fixes PR #496.
