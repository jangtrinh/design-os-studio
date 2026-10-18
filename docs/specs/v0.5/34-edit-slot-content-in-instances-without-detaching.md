# #34 Slots: edit slot content in instances without detaching

- Issue: https://github.com/jangtrinh/design-os-studio/issues/34 (epic #16)
- Spec base: `main` at `767acf9`.
- Open PR: #503 (`task/34-edit-slot-content-in-instances`). Release review, head `8371ee2`, requested changes:
  - MAJOR 1: canvas drag, alt-drag, draw, pen and paste-to-replace bypass the limits;
  - MAJOR 2: a selected slot captures every new instance, including Assets drops elsewhere;
  - MAJOR 3: Reset does not restore slot content;
  - MINOR 4: a rejected drop still marks the slot as changed.
  - The owner ruled on 2026-10-18 that the release session fixes this PR itself.
- Files: `crates/designos-doc/src/components.rs`, `slot_model_tests.rs`; `crates/designos-app/src/comp_ops.rs`, `ops.rs`, `panels/components.rs`, `canvas/interact.rs`, `canvas/create.rs`, `shell/tests/components.rs`.

## 1. Summary and user story

As a designer using a component with a slot, I add, remove and reorder layers inside the slot of an instance, and the instance stays linked to its main component. Later edits to the main component still reach every non-slot part. The slot's min, max and preferred-only settings are enforced, and I am told why when an edit is refused.

## 2. Current state

- FACT: `sync_pair` (`components.rs:858-956`) returns before rebuilding children whenever any of these holds:
  - the instance node overrides `SlotContent`;
  - the main node binds `SlotContent`;
  - either node is a slot container (`components.rs:905-911`).

  So slot children are never rebuilt from the main component, even after a Reset.
- FACT: `reset_overrides` (`components.rs:473`) clears override flags and re-syncs.
- FACT: `insert_instance` (`comp_ops.rs:330-345`) parents into `container_at(&self.doc, self.page, at, &[])`. On `main` it does not special-case a selected slot. The release review describes the PR head's change at `comp_ops.rs:333`.
- FACT: no doc function checks `SlotPropConfig` limits on main. `rg check_slot_insert` finds nothing in `crates/`.
- FACT, Figma: Plugin API slot edits that break limits "don't throw errors". The `SlotNode.limitViolations` getter reports `BELOW_MIN`, `ABOVE_MAX` and `HAS_NON_PREFERRED` (developers.figma.com `SlotNode`, read 2026-10-18). The editor bundle has `SlotLimitsBindings_computeLimitViolations`, `selectNonPreferredChildren` and `SlotsBindingsWeb_onSlotLimitViolations`.

  The issue AC asks Studio to enforce max with a toast. Studio refuses the edit; Figma flags it. This is a deliberate difference; see §13.

## 3. Behaviour rules

1. **Slot resolution.** For a node `p` inside an instance, `enclosing_instance_slot(doc, p) -> Option<SlotRef>` returns the slot container when `p` itself is a slot container in an instance. A slot container is `is_slot` or `is_slot_container`.
   `SlotRef { slot: NodeId, instance: NodeId, prop: String, cfg: SlotPropConfig }`.
   - `prop` comes from the slot's (or its `source`'s) `SlotContent` binding.
   - `cfg` comes from the owner component's `slot_configs[prop]`. The owner is found by walking `component.source` to the unlinked main slot, then to its enclosing component or set.
   - It never takes "the only config on some outer component" (the #150 review MAJOR on nested instances).
2. **One structural gate.** Every structural edit whose source or target parent is an instance slot calls `check_slot_change(doc, slot, op) -> Result<(), SlotConstraintError>` before mutating. On `Err` it shows an error toast and does nothing. The edits are:
   - insert, paste, paste over selection, duplicate;
   - Assets drop, canvas drag-reparent, alt-drag duplicate;
   - shape draw, pen commit, text create;
   - delete, reorder, move to parent.

   The gate code paths are `ops.rs` (insert, paste, duplicate, delete, reorder, `move_to_parent`), `canvas/interact.rs` (`reparent_after_move`, `start_move` alt-duplicate, `start_draw`), `canvas/create.rs` (pen commit) and `ops.rs` paste-to-replace.
3. **Constraint rules,** with `n` = current child count of the slot:
   - insert k children: `max_children = Some(m)` and `n + k > m` → `Err(AboveMax { max: m })`;
   - remove k children: `min_children = Some(m)` and `n - k < m` → `Err(BelowMin { min: m })`;
   - insert a node X while `allow_preferred_values_only == Some(true)` → `Err(NotPreferred)` unless X is an `Instance` whose component, or that component's set, is in `cfg.preferred`;
   - reorder never violates.
4. **Toast texts** (error toasts):
   - `AboveMax` → `"This slot holds at most {m} layers"`;
   - `BelowMin` → `"This slot needs at least {m} layers"`;
   - `NotPreferred` → `"This slot only accepts its preferred instances"`.
5. **Linked, with override.** A structural change that actually changes the slot's child list (membership or order) records `OverrideField::SlotContent` on the instance slot node, in the same transaction. A refused or no-op edit records nothing (MINOR 4).
6. **Non-slot parts still sync.** `sync_pair` skips only the children of an instance slot node that has the `SlotContent` override. A main slot (binding) with no instance override is rebuilt from the main component as normal children. The early return becomes:

   ```rust
   if inode.component.overrides.contains(&OverrideField::SlotContent) { return; }
   ```

   Slot detection by binding no longer suppresses the rebuild.
7. **Reset.** Clearing `SlotContent` with `reset_overrides` then `sync_one` rebuilds the slot's children from the main slot. Inserted layers are removed, and the main component's default slot content returns (MAJOR 3). One undo step.
8. **Insert target.** `insert_instance(comp, at)` parents into a selected slot only when `at` lies inside that slot's world bounds. Otherwise it uses `container_at` (MAJOR 2). The inspector's add action passes the slot centre as `at`.
9. **Slot content inspector section.** Shown when the selection is an instance slot. It lists the children in order. Each row has `Move down` and `Remove` (ids `slot-child-down:<id>` and `slot-child-del:<id>`). An `Add instance…` dropdown lists components: preferred first, others only when `allow_preferred_values_only != Some(true)`. Each action goes through rule 2.

## 4. Data model / schema changes

None. `OverrideField::SlotContent` (`node.rs:189`) and `PropValue::Slot` exist. `.fig` and REST mapping are unchanged by this task. The instance's slot children are ordinary nodes under the instance's slot frame, stored in `.designos` like any child.

## 5. Contracts

```rust
pub struct SlotRef { pub slot: NodeId, pub instance: NodeId, pub prop: String, pub cfg: SlotPropConfig }
pub enum SlotOp<'a> { Insert(&'a [NodeId]), Remove(usize), Reorder }
pub enum SlotConstraintError { AboveMax { max: u32 }, BelowMin { min: u32 }, NotPreferred }
pub fn enclosing_instance_slot(doc: &Document, parent: NodeId) -> Option<SlotRef>;
pub fn check_slot_change(doc: &Document, slot: &SlotRef, op: SlotOp) -> Result<(), SlotConstraintError>;
pub fn mark_slot_content_changed(doc: &Document, slot: NodeId, before: &[NodeId]) -> Option<Command>; // None when the child list is unchanged
```

`SlotConstraintError` implements `Display` with the toast texts of rule 4.

## 6. File-format mapping

None new. FACT: in Figma, slot content of an instance is the instance's own children of the slot frame, flagged `isSlotContent` (495). Studio reads and writes that flag already (`rest_import.rs:1020`, `fig_export.rs:644-646`). RESOLVED.

## 7. UI

- **Slot content section.** It sits in the right panel under the instance's `Component properties` when an instance slot is selected.
  - The title `Slot content` has a count badge, rendered separately (P5).
  - Rows are 28 px with ≥24 px icon buttons: `Move down` and `Remove`. `Remove` uses `Icon::Trash` (`icons.rs:98`). `Move down` needs Phosphor `arrow-down`, which is not vendored: `Icon` has `ArrowUp` but no `ArrowDown` (`icons.rs:15-140`). Vendor it the way `ArrowUp` is vendored (CONTEXT.md, Unified Phosphor Icons System).
  - `Add instance…` uses the existing instance-swap dropdown.
- **Override chip.** The existing "Changed: slot content" chip and the Reset action appear only when the override exists.
- **Toasts.** Error style, using the existing toast system.
- **Rules.** Zero fake UI: disabled rows give a reason in the tooltip. For example, Remove at min shows `"This slot needs at least {m} layers"`.
- **Evidence.** 1440 capture before and after, with a child added to an instance slot.

## 8. Edge cases

1. `max_children = 2`, the slot has 2: drag a rectangle in, alt-drag a child, or draw a frame inside. All are refused with a toast, and the document is unchanged (MAJOR 1).
2. `min_children = 1`, the slot has 1: drag the last child out, or delete it. Refused.
3. Preferred-only with `[Icon]`: an Avatar instance dropped in is refused; an Icon instance is accepted; a plain frame is refused.
4. Slot selected, Assets drop on empty canvas: the instance lands on the canvas, not in the slot (MAJOR 2).
5. Add a child, then Reset: the child is removed, the default content is back, and the chip is gone (MAJOR 3).
6. A preferred-only drop is refused: no override chip appears (MINOR 4).
7. A main component edit (fill of a non-slot sibling) after slot edits: the instance's sibling updates, and the slot content is kept.
8. A nested instance (icon slot on a button inside a card): the limits come from the button's slot config, not the card's (rule 1).

## 9. Non-goals

- The empty-slot placeholder, the `+` picker and stretch (#150).
- Figma-style non-blocking violation badges. A follow-up can add them if the owner prefers Figma's behaviour.
- `.fig` export changes.

## 10. Test plan

Seams: `crates/designos-doc/src/slot_model_tests.rs` (doc rules) and `crates/designos-app/src/shell/tests/components.rs` (app paths).

| Test | Fails before |
|---|---|
| `adding_a_child_to_an_instance_slot_keeps_the_instance_linked` | yes |
| `max_children_refuses_insert_with_toast_on_every_path` (parameterised over insert_instance, paste, duplicate, canvas reparent, alt-drag, draw, pen) | yes for the canvas paths on PR head `8371ee2` |
| `min_children_refuses_remove_and_drag_out` | yes |
| `preferred_only_refuses_non_preferred_instance_and_plain_frame` | yes |
| `assets_drop_away_from_selected_slot_lands_under_pointer` | yes on PR head |
| `reset_slot_content_restores_main_component_children_in_one_undo` | yes on PR head |
| `refused_drop_records_no_slot_content_override` | yes on PR head |
| `main_component_update_reaches_non_slot_siblings_but_keeps_slot_content` | yes |
| `nested_instance_slot_uses_inner_component_config` | yes |

Fixture: a component `Card` (200×120, vertical auto layout) with a slot frame `Body` bound to prop `Body`; config `{min 1, max 2, preferred_only true, preferred [Icon]}`; components `Icon` (24×24) and `Avatar` (32×32); one `Card` instance.

## 11. Performance budgets

- `CLAUDE.md` §4.1: no `doc.to_json` or `doc_hash` on the UI thread, and the gate checks are O(children).
- The canvas drag path calls `enclosing_instance_slot` once on release, not per pointer move.
- The P8 frame budget (8 ms build) is unaffected.

## 12. Dependencies and build order

- Depends on #31 (merged), #32 and #112.
- #150 depends on this. Both edit `sync_pair` and `insert_instance`. Fix MAJORs 2 and 3 once, here, and rebase #150 onto it (release review note).

## 13. Owner rulings that apply

- The issue AC ("a max limit is enforced with a toast") is the owner-filed scope.
- Figma reports limit violations without blocking. Studio blocks, per the AC. Recorded as a known difference; not an owner question unless Figma parity is preferred later.
- Owner ruling of 2026-10-18: the release session fixes PR #503.
- Owner default: desktop 1440 capture only.
