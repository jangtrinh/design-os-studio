# #645 Migrate to slot: an unbound instance in one variant must not leave a set-level slot only one variant has

| Field | Value |
|---|---|
| Issue | #645 (follow-up of the #533 review) |
| Size / priority | S / P2 |
| Depends on | none |
| Unblocks | none |
| Touches | `crates/designos-doc/src/components.rs` (`classify_slot_region`, `classify_slot_migration`, tests at the end of the file), `crates/designos-app/src/comp_ops.rs` (toast only, `migrate_node_to_slot`) |

Code baseline: `origin/main@01d48f2` (0.5.0). The issue cites `upgrade_owner_prop_to_slot` at
`components.rs:1298`; it is `components.rs:1715` now.

## 1. Summary and user story

As a designer migrating an instance region inside a variant of a component set to a slot, I never end
up with a slot property on the whole set that exists on only one variant. For an unbound region in one
variant, the new slot stays scoped to the variant I started in.

## 2. Current state (FACT)

- `classify_slot_region` (`crates/designos-doc/src/components.rs:1589-1660`) classifies one region.
  For an `Instance` region with neither a `MainComponent` nor a `Visible` binding it invents a
  property name: `unique_slot_prop_name(doc, name_owner, &n.name)` (`:1627`, in the
  instance branch), where `name_owner` is the component set when the variant sits in one, else the
  component.
- It then decides the owner: `defined_on_variant` is true only when the variant component already
  holds a property of that name (`:1656`); owner = the variant (`comp`) in that case, else
  `name_owner` (`:1657`). A freshly invented name is never defined on the variant, so the owner is
  the set.
- `classify_slot_migration` (`:1664-1693`) looks for matching regions in the other variants only when
  `set_defines_prop` is true: `owner != comp && owner.props.any(|p| p.name == prop_name)`
  (`:1667`). The invented name is not on the set yet, so the guard never runs and `regions` holds the
  selected region alone.
- `migrate_region_to_slot` (`:1992-2016`) migrates the listed regions and calls
  `upgrade_owner_prop_to_slot(&mut c, plan.owner, ..)` (`:1715`), which pushes a `PropKind::Slot`
  property on the owner when the name is not found (`:1729`). With the set as owner the result is
  a set-level slot property that only the selected variant's region is bound to.
- Existing behaviour for related shapes:
  - a property shared by the set where a variant lacks a matching region is refused with an honest
    message (`migrate_region_in_a_component_set_is_a_noop_when_a_variant_has_no_matching_region`,
    `components.rs:3152`);
  - a property a variant defines itself migrates that variant only
    (`migrate_region_whose_property_a_variant_defines_itself_leaves_other_variants_alone`, `:3223`);
  - a shared bound region migrates every variant
    (`migrate_instance_swap_region_in_a_component_set_migrates_every_variant_and_undoes_in_one_step`, `:3108`,
    fixture `set_with_icon_in_each_variant`).
- UI entry: the "Migrate to slot" action shown when `can_migrate_region_to_slot` is true
  (`designos-app/src/panels/components.rs:622`), run by `App::migrate_node_to_slot`
  (`comp_ops.rs:571-585`), which toasts the refusal text or "Migrated to slot".

## 3. Behaviour rules

Owner ruling 2026-10-19: the new slot belongs to that variant only (what earlier drafts called outcome A). Rules:

1. Applies only to an `InstanceLayer` region with no `MainComponent` and no `Visible` binding whose
   component sits in a component set.
2. The invented property name makes the variant the property's owner (`owner = comp`), the same owner the `defined_on_variant` branch picks (`components.rs:1656-1657`). Only the selected variant's region
   migrates; no property lands on the set; other variants and their instances are untouched. The toast stays "Migrated to slot".
3. After the migration there is no slot property on the set that is bound in fewer variants than the set has variants, unless the property was already defined on the set before the migration (existing
   shared-property behaviour, unchanged).
4. A region that is bound, or any region of a component that is not in a set, behaves as today.
5. One undo step, as today.

## 4. Data model / schema changes

None. The migration writes the property into the variant's `component.props`, a place the model and the
`.fig` exporter already handle (variant-level properties, see #605 and `prop_defs`,
`components.rs:359-370`).

## 5. Contracts

No signature change. `migrate_region_to_slot` and `can_migrate_region_to_slot` keep their types; the
result for the unbound set case is the variant-scoped slot of rule 2.

## 6. File-format mapping

| Format | Mapping |
|---|---|
| `.designos` | `Slot` property in the variant node's `component.props` |
| `.fig` | slot properties already export per owner (`fig_export.rs`, `componentPropDefs`); no new mapping |

## 7. UI

No new control. The existing "Migrate to slot" row appears or hides per `can_migrate_region_to_slot`
and shows its toast. No new copy. Follow `design/principles.md`; no visual change otherwise.

## 8. Edge cases

1. A set of three variants, unbound instance named "Icon" in each, migrate in the first: variant 1 gets the slot property and slot; variants 2 and 3 keep their instances.
2. The variant is not in a set: unchanged; the property lives on the component.
3. The invented name collides with a set property of another kind: `unique_slot_prop_name` already
   suffixes (`Icon 2`); the owner is chosen after the name is final.
4. Instances of other variants: their `component.values` and children are byte-identical
   before and after (the assertion of the existing test at `:3223`).
5. Undo restores the variant's props, the region and every instance (existing one-step guarantee).

## 9. Non-goals

- Heuristic matching of unbound regions across variants (none is attempted).
- Moving an existing variant property to the set.
- Bound regions and boolean containers (unchanged).

## 10. Test plan (red first)

Seam: `designos-doc` unit tests next to `set_with_icon_in_each_variant`; add a fixture
`set_with_unbound_icon_in_each_variant()` (two variants, each with one instance of `f.comp` and no
property bindings, plus one instance of each variant on the page).

| Case | Steps | Expected (fails before) |
|---|---|---|
| `unbound_region_in_one_variant_creates_no_set_level_slot` | migrate the small variant's icon | `set.component.props` has no `Slot` entry; the small variant has exactly one; the large variant's region and its instance are unchanged |
| `unbound_region_in_a_standalone_component_unchanged` | component without a set | property on the component, as today |
| `undo_restores_everything` | undo after the migration | previous props and regions |

`cargo test -p designos-doc`. Evidence: red/green terminal capture; a Properties-panel screenshot of the
small variant after the migration (Components panel, Dev capture at 1440).

## 11. Performance budgets

No added cost: only the selected variant is touched.

## 12. Dependencies and build order

Independent. `components.rs` is a hot file (also #582, #583 touch the doc crate): rebase, do not
hand-merge.

## 13. Owner rulings

- Owner ruling 2026-10-19 (`plans/reports/release-060-evidence/owner-rulings-v06.md`): the slot belongs to that variant only.
- #533: migration keeps instance content and is one undo step; a shared property migrates every
  variant that holds a matching region.
- Nothing vanishes silently: a refusal names the next action.

## Open questions for the owner

None. The question of the first draft was answered by the owner on 2026-10-19: the slot belongs to that variant only.
