# #150 Show empty-slot placeholders, a preferred-instance picker and stretch-on-insert

- Issue: https://github.com/jangtrinh/design-os-studio/issues/150 (task E3-7, epic #16, M / P1)
- Spec base: `main` at `767acf9`.
- Open PR: #508 (`task/150-empty-slot-placeholder-picker-stretch`, +917 −19). The release review, head `2ddfeb7`, found four MAJORs:
  1. a selected slot captures Assets drops (`comp_ops.rs:333`);
  2. `slot_config_of` resolves a nested slot to the outer component;
  3. chrome collection stops at depth 3 on documents with more than 10k nodes, so a normal screen's slot draws no placeholder or `+`;
  4. Reset does not restore slot content.

  The owner ruled on 2026-10-18 that the release session fixes this PR itself.
- This spec also corrects a semantic error in the PR: `stretchChildOnInsert` is counter-axis fill, not primary-axis fill (Figma Plugin API).
- Files: `crates/designos-doc/src/components.rs`; `crates/designos-app/src/canvas/draw.rs`, `canvas/mod.rs`, `comp_ops.rs`, `ops.rs`, `panels/components.rs`, `shell/tests/components.rs`; `crates/designos-render/src/index.rs`.

## 1. Summary and user story

As a designer using a slotted component:
- an empty slot shows a visible placeholder with a `+` button, in the main component and, when the author chose so, in instances;
- `+` offers the slot's preferred instances first, or only those when the author restricted it;
- an inserted child fills the slot across its counter axis when the author enabled "Stretch child on insert".

## 2. Current state

- FACT: the canvas draws slot chrome when `fast_is_slot_container` (`canvas/draw.rs:30-37`) is true. It requires an own or source `SlotContent` override or binding, then calls `designos_doc::is_slot_container`. `component.is_slot` is not consulted.
- FACT: the scene index (`crates/designos-render/src/index.rs:131`) applies the same predicate for chrome nodes.
- FACT: chrome collection is depth-capped on large documents. The review cites `canvas/draw.rs:206` and `:355`: depth 3 when `doc.nodes.len() > 10_000`. Selected or hovered slots get an extra pass (`draw.rs:261-271`).
- FACT: there is no placeholder, `+` picker or stretch logic on `main`.
- FACT: `SlotPropConfig.display_by_default` and `stretch_child_on_insert` are stored (`node.rs:226-231`).

## 3. Behaviour rules

1. **Empty.** A slot is empty when its child list has no visible children.
2. **Placeholder visibility.**
   - In a main component (or a variant), an empty slot always shows the placeholder while `show_ui` is on (editing, not Present or prototype).
   - In an instance, an empty slot shows the placeholder when `display_by_default != Some(false)`. Both `None` and `Some(true)` show it, which matches Figma's `displayEmptyByDefault` highlight on empty instances.
   - A selected or hovered empty slot always shows the placeholder.
   - Never in Present or prototype mode.
3. **Placeholder drawing.**
   - A 1 px dashed border inset 0.5 px inside the slot bounds, in the component accent `#06B6D4` from `theme.rs`, at 60% alpha.
   - A centred `+` button: a 24×24 px screen-space circle, the same accent, with a white Phosphor `Plus` icon.
   - The placeholder is overlay chrome (UI layer), never document content, so it is excluded from export and from the CPU and GPU reference renders.
4. **Collection without the depth cap.** Collect empty-slot placeholders from the `SceneIndex` visible set: every slot node intersecting the viewport, at any depth. The painter and the hit-test overlay use the same id list (MAJOR 3).
5. **Picker.** Clicking `+` (or the inspector `Add instance…`) opens a menu.
   - `allow_preferred_values_only == Some(true)` and `preferred` non-empty: only the preferred components, in `preferred` order.
   - `preferred` non-empty and not restricted: the preferred components first, then a submenu `More components…` with all local components, sorted by name.
   - `preferred` empty: all local components, sorted by name.
   - Recommended ids from the R&D note on the issue are the same list as `preferred`. No separate field.
6. **Insert from the picker** goes through #34's gate (`check_slot_change`) and `insert_instance_into_slot(comp, slot)`. The instance is appended as the last child. `SlotContent` is recorded on an instance slot. Everything is one undo step.
7. **Stretch-on-insert.** When `stretch_child_on_insert == Some(true)` and the slot has auto layout, an inserted child gets `Sizing::Fill` on the slot's **counter axis**:
   - width for a vertical stack;
   - height for a horizontal stack.

   The primary axis keeps the child's sizing. For a slot without auto layout, the child's counter-axis size is set to the slot's inner size along that axis, with `Constraint::Stretch` on that axis. Applies on every insert path in #34 rule 2. Source: Figma Plugin API `SlotSettings.stretchChildOnInsert`, "when a layer is inserted into the slot, apply counter-axis fill".
8. **Config resolution** uses #34's `enclosing_instance_slot` / owner walk. It never takes the outer component's only config (MAJOR 2).
9. **Insert target and Reset** behave as #34 rules 7 and 8. Shared code, fixed once.
10. **Round trip.** Filling a slot is a descendant override that replaces the slot's children (R&D note). It survives:
    - `.designos` (children plus `OverrideField::SlotContent`);
    - the JSON scene format (the `NodeSpec` / `--apply-spec` scene JSON already carries children);
    - `.fig` (instance slot children plus `isSlotContent=true`).

## 4. Data model / schema changes

None. Every flag exists.

## 5. Contracts

```rust
pub fn should_show_empty_slot_placeholder(doc: &Document, slot: NodeId, show_ui: bool, hovered_or_selected: bool) -> bool;
pub fn slot_picker_entries(doc: &Document, slot: &SlotRef) -> SlotPicker; // { preferred: Vec<NodeId>, others: Option<Vec<NodeId>> }
pub fn apply_slot_stretch_on_insert(doc: &Document, slot: NodeId, child: NodeId) -> Option<Command>;
impl App { pub fn insert_instance_into_slot(&mut self, comp: NodeId, slot: NodeId) -> Option<NodeId>; }
```

## 6. File-format mapping (RESOLVED)

- `SlotPropConfig.displayByDefault = 2` is the Plugin API's `displayEmptyByDefault`.
- `stretchChildOnInsert = 1` means counter-axis fill.
- `isSlotContent = 495` on inserted children in `.fig` export.
- Figma editor symbols: `EMPTY_SLOT_CONTENTS_ID`, `SLOT_PREFERRED_INSTANCE_INSERT`, `SlotPreferredValuePicker`, `showSlotPreferredContentPicker`. These confirm that the picker exists as a dedicated surface.

## 7. UI

- **Placeholder.** As in rule 3, on the canvas overlay layer.
- **The `+` control.** It has a 24×24 hit target in screen space, independent of zoom (P3).
  - Hover darkens the accent by one step.
  - Focus uses the brand orange ring `#E36323` (P2).
- **Picker menu.** Reuses the context-menu widget, with rows 28 px high.
  - Preferred rows show a small component icon in cyan.
  - The `More components…` submenu appears only when rule 5 allows it.
- **Inspector.** In the instance slot section (#34), `Add instance…` opens the same picker.
- **Zero fake UI.** The `+` is always wired. It is not drawn where inserting is impossible: in a slot already at max, the placeholder is not shown, because the slot is not empty, and the inspector's `Add instance…` is disabled with a reason.
- **Evidence.** A 1440 capture of an empty slot in a main component and in an instance, with the picker open.

## 8. Edge cases

1. On a 723k-record page (the VSF-scale benchmark), an empty slot at depth 4 (Section → Frame → Instance → Slot) shows the placeholder and its `+` works (MAJOR 3).
2. An instance with `display_by_default == Some(false)` hides the placeholder until it is selected.
3. A vertical auto-layout slot 320 wide, padding 16, with stretch on. An inserted Icon 24×24 Fixed becomes width Fill, so the inner width is 288; its height stays 24.
4. A horizontal slot with stretch on: height Fill, width kept.
5. Stretch off: the inserted child keeps its sizing.
6. Preferred-only with an empty `preferred`: treated as not restricted (all components). Matches rule 5's precondition.
7. A nested icon slot inside a button inside a card: the picker lists the button slot's preferred components (MAJOR 2).
8. Present mode: no placeholder.

## 9. Non-goals

- Figma's non-blocking violation badges.
- A slot thumbnail preview in the picker.
- Library components in the picker (local and enabled-library copies only).

## 10. Test plan

Seams: `crates/designos-doc/src/slot_model_tests.rs` and `crates/designos-app/src/shell/tests/components.rs`.

| Test | Expected | Fails before |
|---|---|---|
| `empty_slot_shows_placeholder_in_main_and_in_instance_by_default` | true / true | yes |
| `instance_hides_placeholder_when_display_by_default_false` | false, true when selected | yes |
| `placeholder_and_plus_work_on_large_document_at_depth_four` | 12k-node doc, slot at depth 4: painter list contains it; click on `+` opens picker | yes on PR head |
| `picker_lists_preferred_only_when_restricted` | `[Icon]`, no `others` | yes |
| `picker_lists_preferred_first_then_more` | `preferred [Icon]`, `others` = all sorted | yes |
| `stretch_on_insert_fills_counter_axis_vertical` | vertical slot: width Fill, height kept | yes (PR head fills primary axis) |
| `stretch_on_insert_fills_counter_axis_horizontal` | horizontal slot: height Fill | yes |
| `nested_slot_picker_uses_inner_config` | | yes on PR head |
| `filled_slot_round_trips_native_and_fig` | children plus `isSlotContent` back | yes |

## 11. Performance budgets

- `CLAUDE.md` §4.1: no `app.changed()` and no `TileView::rev` increment from hover or placeholder painting. The placeholder is overlay-only.
- The P8 budget: placeholder collection reads the `SceneIndex` visible set, O(visible). It never walks the page tree per frame.
- §4.3 and §4.4: no new GPU `Program`, because the placeholder is drawn with egui painter shapes.

## 12. Dependencies and build order

- Depends on #32 and #34 (shared gate, Reset and insert-target fixes). Merge after #34.

## 13. Owner rulings that apply

- Owner default: desktop 1440 capture; responsive skipped.
- Owner ruling of 2026-10-18: the release session fixes PR #508.
- Design principles P1–P3 and P6.
