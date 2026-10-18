# #151 Migrate instance-swap or boolean regions to slots, keeping instance content

- Issue: https://github.com/jangtrinh/design-os-studio/issues/151 (task E3-8, epic #16, M / P2)
- Spec base: `main` at `767acf9`.
- Open PR: #533 (`task/151-migrate-to-slots`, +841 −1). The release review, head `1c1117b`, found two MAJORs:
  1. migrating one variant rewrites the shared set property and breaks the other variants;
  2. a hidden boolean region's children are deleted instead of kept as slot content.

  The owner ruled on 2026-10-18 that the release session fixes this PR itself.
- Files: `crates/designos-doc/src/components.rs`; `crates/designos-app/src/comp_ops.rs`, `panels/components.rs`, `shell/tests/components.rs`.

## 1. Summary and user story

As a design-system maintainer adopting slots, I want to turn an existing region into a slot without losing what every existing instance shows. The region is either driven by an instance-swap property (an icon) or by a boolean visibility property (an optional footer).

## 2. Current state

- FACT: instance swap is `PropKind::InstanceSwap { preferred }` (`node.rs:209`), bound by `OverrideField::MainComponent` → prop name. `sync_pair` swaps on `PropValue::Instance` (`components.rs:868-873`).
- FACT: boolean visibility is `PropKind::Boolean`, bound by `OverrideField::Visible`, and applied by `apply_bindings` (`components.rs:763`).
- FACT: property definitions of a component set live on the set (`prop_owner`, `comp_ops.rs:411`).
- FACT: there is no migrate command on `main`.
- REA (E3-c), Figma 126.9.13 editor bundle:
  - slot commands present: `CONVERT_TO_SLOT`, `GROUP_SELECTION_INTO_SLOT`, `RESET_SLOT_ASSIGNMENT`, `canConvertSelectedSlotsToFrame`, `createSlotAssignment`, `generatePrimaryInstanceAssigningSlotItem`;
  - no string `migrate`, `migrateToSlots` or `Migrate to slots` in any slot-bearing chunk (0 hits).
  - Conclusion (INFERENCE): Figma has no dedicated "migrate region" command in this build. The semantics below are Studio's own, built from Figma's primitives (convert-to-slot plus slot assignment per instance).

## 3. Behaviour rules

1. **Supported region shapes.** The selection is exactly one node `R` inside a main component or a variant, and `R` is one of:
   - (A) an `Instance` bound `MainComponent → P`, with `P` an InstanceSwap prop;
   - (B) a `Frame` whose only child is an `Instance` bound as in (A);
   - (C) a `Frame` bound `Visible → P`, with `P` a Boolean prop.

   Anything else is a no-op with an honest message (rule 7).
2. **Variants: all or nothing.** When `P` lives on a `ComponentSet`, find the matching region in every variant: the node with the same binding (`MainComponent → P` or `Visible → P`) and the same shape class.
   - If any variant has no matching region, or has more than one, return `Err(VariantMismatch { variant: <name> })` and leave the document unchanged.
   - Otherwise migrate every variant's region in the same command (MAJOR 1).
3. **Main-side transform, per region** (one `Command::Batch` for everything):
   - Shape (A): wrap `R` in a new frame `"<P name>"` with the same transform and size, so that frame becomes the slot. `R`'s default instance stays inside as default slot content.
   - Shapes (B) and (C): the frame itself becomes the slot.
   - The slot frame gets `is_slot = true` and the binding `SlotContent → P'`.
   - The old binding (`MainComponent` or `Visible`) is removed from the region.
   - The owner's prop `P` becomes `P'` = `ComponentPropDef { name: P.name, kind: PropKind::Slot, default: PropValue::Slot(vec![]) }`, the same name, so existing references keep reading.
   - Owner `slot_configs[P.name]` = `SlotPropConfig { preferred: <InstanceSwap preferred>, ..Default::default() }` for (A) and (B), or default for (C).
4. **Instance-side content, per instance** of every migrated component or variant (direct and nested):
   - (A)/(B): the instance's current content in the region becomes slot content. That is the swapped instance, or the default one, with its overrides. Each moved node is marked `is_slot_content = true`.
   - The instance slot node gets `OverrideField::SlotContent` when its content differs from the main default. That is the case when the instance swapped (`PropValue::Instance(x)` with `x` ≠ default) or overrode a descendant.
   - (C), visible in the instance: same as (A), with the region's current children.
   - (C), **hidden** in the instance (`values[P] == Bool(false)`): the children are **kept** as slot content (`is_slot_content = true`). The slot frame keeps `visible = false` as an instance override (`OverrideField::Visible`), so the instance renders as before (MAJOR 2).
   - `values[P]` on the instance root is replaced by `PropValue::Slot(<ids of the slot content>)`.
5. **One undo step.** One `Tx::commit`. Undo restores the props, bindings, values, nodes and visibility exactly.
6. **Inspector.** `Migrate to slot` shows in the `Component properties` row of an InstanceSwap or Boolean prop, and on the context menu of a supported region. It is enabled only when `can_migrate_region_to_slot` is `Ok`. Otherwise it is disabled, and the tooltip carries the error text.
7. **Honest messages** (error toast on a forced call, tooltip on the disabled control):
   - `NotInMainComponent` → `"Select a layer inside a main component"`;
   - `UnsupportedShape` → `"Only an instance-swap layer, a frame around one, or a frame shown by a boolean property can become a slot"`;
   - `VariantMismatch { variant }` → `"Variant \"{variant}\" has no matching layer, so nothing was changed"`;
   - `InsideInstance` → `"Edit the main component to migrate"`.

## 4. Data model / schema changes

None. It uses `PropKind::Slot`, `PropValue::Slot`, `slot_configs`, `is_slot`, `is_slot_content` and `OverrideField::SlotContent`, all serialized already.

## 5. Contracts

```rust
pub enum MigrateToSlotError { NotInMainComponent, UnsupportedShape, VariantMismatch { variant: String }, InsideInstance, Missing }
pub fn can_migrate_region_to_slot(doc: &Document, node: NodeId) -> Result<MigratePlan, MigrateToSlotError>;
pub fn migrate_region_to_slot(doc: &mut Document, node: NodeId) -> Result<Command, MigrateToSlotError>;
impl App { pub fn migrate_selection_to_slot(&mut self) -> bool; }
```

`MigrateToSlotError` implements `Display` with the texts of rule 7.

## 6. File-format mapping

Export after migration uses the existing slot paths: `isSlot=463`, `isSlotContent=495`, `ComponentPropType.SLOT=7` and `slotPropConfig=12` (specs 107, 108, 110). No new facts are needed. RESOLVED. REA E3-c is recorded in §2.

## 7. UI

- **Placement.** The action is a text button `Migrate to slot`, 28 px high, under the property's row in Component properties. No icon is needed, and none of the vendored icons (`icons.rs`) fits. The same action is in the region's context menu.
- **Rules.** Disabled state with a reason, toast on error, and theme tokens only.
- **Evidence.** A 1440 capture of the before state (swap property), the after state (slot property), and an instance showing the same content.

## 8. Edge cases

1. Button set Primary/Ghost with `Icon` on the set: both variants are migrated, and both variants' instances keep their icons (MAJOR 1).
2. A set where Ghost has no icon layer: `VariantMismatch("Ghost")`, and the document hash is unchanged.
3. Card footer bound to `Show footer`; an instance turned it off after editing the label to `Custom footer`. After migration that instance's slot holds the `Custom footer` label, the slot is hidden, and the render is identical (MAJOR 2).
4. An instance that swapped Icon to Avatar: its slot holds an Avatar instance, and the override flag is set.
5. A nested instance of the component inside another component: migrated too.
6. A plain rectangle, a text layer, an unbound frame, a page, a component root, or a layer inside an instance: no-op with a message, and the document is unchanged.
7. Undo: byte-identical `to_json` before and after (test only; never on the UI thread).

## 9. Non-goals

- Migrating a whole library in one command.
- Migrating TEXT or VARIANT props.
- Exposing the command through MCP (follow-up).

## 10. Test plan

Seams: `crates/designos-doc/src/components.rs` tests (or `slot_model_tests.rs`) and `crates/designos-app/src/shell/tests/components.rs`.

| Test | Fails before |
|---|---|
| `migrate_instance_swap_region_to_slot_keeps_swapped_and_overridden_content_and_undoes_in_one_step` | yes |
| `migrate_in_component_set_migrates_every_variant_and_their_instances` | yes on PR head `1c1117b` |
| `migrate_in_component_set_with_missing_variant_region_is_a_noop_with_message` | yes |
| `migrate_boolean_region_keeps_hidden_instance_content_as_hidden_slot_content` (replaces the PR's `..._empties_hidden`) | yes on PR head |
| `unsupported_shapes_are_noops_with_honest_message` (rect, text, unbound frame, page, component root, inside instance) | yes |
| `render_of_every_instance_is_unchanged_after_migration` (CPU render hash per instance before and after) | guard |

## 11. Performance budgets

`CLAUDE.md` §4.1: one commit and one `rev` bump. No `to_json` in the app path. The migration is O(instances × region size).

## 12. Dependencies and build order

- Depends on #32 (convert-to-slot primitives) and #34 (slot-content override semantics in `sync_pair`).
- Merge after #34.

## 13. Owner rulings that apply

- The full-scope ruling (`owner-decisions-and-cross-check.md`: all epics in full scope, no MVP) puts this task in scope.
- No owner ruling exists on migrate semantics. Figma has no equivalent command in 126.9.13 (REA above), so the rules here are a technical decision, recorded for review.
- Owner ruling of 2026-10-18: the release session fixes PR #533.
