# #108 Slots: slot content id per property (format change and migration)

- Issue: https://github.com/jangtrinh/design-os-studio/issues/108 (audit E3-2, epic #16, M / P1)
- Spec base: `main` at `767acf9`.
- Open PR: #512 (`task/108-slot-content-id-per-property`). Release reviews:
  - Round 1: two MAJORs.
  - Round 2: two MAJORs.
  - Release fix round 2, head `4bc2bb8`: the node's own id round-trips untouched, per-property ids come only from the property fields, `delete_prop` no longer touches the node id, and the legacy `.designos` migration was dropped.
  - The owner ruled on 2026-10-18 that the release session fixes this PR itself.
- Hot files: `node.rs`, `components.rs`, `fig.rs`, `fig_export.rs`, `rest_import.rs`, `testkit.rs`.

## 1. Summary and user story

As a designer with a component that has two slots (Header and Body), each instance must keep separate content per slot through save, `.fig` export and re-import. Today one `slot_content_id` per node cannot represent two slot properties.

## 2. Current state

- FACT: `ComponentLink.slot_content_id: Option<String>` (`node.rs:270-273`) is the only slot content identity, one per node.
- FACT: the import reads it only from `jsonSerialized["slot-content-id"].guid` (`fig.rs:1198-1201`), mapped at `rest_import.rs:1021`. The export writes it back as `jsonSerialized` (`fig_export.rs:647-649`).
- FACT: the real kiwi `NodeChange` has no `jsonSerialized` field (embedded-schema scan, see spec 160 §6). The real schema carries a slot content id as `VariableAnyValue.slotContentIdValue = 18` (`SlotContentId{guid=1}`), reached through:
  - `ComponentPropAssignment.varValue` (3), on instances;
  - `ComponentPropDef.varValue` (9), the default on a component.
- FACT: `component_props` (`fig.rs:1624-1680`) reads `holder.varValue.value` only for bool, text, guid and symbolId values (`fig.rs:1630-1640`). `slotContentIdValue` is ignored.
- FACT, from the PR #49 coordinator review M1: "Read the slot content id from the real schema path first". That was not done on `main`. The `jsonSerialized` path is the only one implemented.

## 3. Behaviour rules

1. **Two identities, kept apart.**
   - `slot_content_id` (existing) = the node's own `slot-content-id` as read from the legacy `jsonSerialized` bag. It is opaque and round-trips unchanged when present. No code derives anything from it.
   - `slot_content_ids` (new) = property name → content guid, from the real schema: `ComponentPropAssignment.varValue.value.slotContentIdValue.guid` for instances, and `ComponentPropDef.varValue.value.slotContentIdValue.guid` for component defaults.
2. **Import.**
   - For every assignment whose def is type `SLOT` (7) and whose `varValue.dataType == SLOT_CONTENT_ID` (18), or whose `varValue.value.slotContentIdValue` is set: insert `slot_content_ids[prop name] = "<session>:<local>"`.
   - Same for a def's `varValue`, stored on the component node.
   - A guid-only assignment stores the id and does not invent a `PropValue::Slot` value.
3. **Export.** For each `PropKind::Slot` def, write `varValue{ value{ slotContentIdValue{ guid } }, dataType: SLOT_CONTENT_ID, resolvedDataType: SLOT_CONTENT_ID }` only when `slot_content_ids` has that property name.
   - Never fall back to `slot_content_id` for a property.
   - `jsonSerialized` is written only when `slot_content_id` is `Some`, as on `main`. #110 removes it from `.fig` export, because Figma's schema lacks it.
4. **Rename** (`rename_prop`, `components.rs:1011`) moves the `slot_content_ids` key on the owner and on every instance. **Delete** (`delete_prop`, `components.rs:1050`) removes the key on owner and instances, and never touches `slot_content_id`.
5. **Copy** (duplicate, paste, create instance) clears `slot_content_ids` exactly like `slot_content_id` (rule 1 of #112).
6. **Legacy `.designos` files** keep loading and exporting exactly as on `main`. There is no migration: a legacy `slot_content_id` is the node's own id, and mapping it onto one property would be a guess. This was a release-session decision of 2026-10-18 on PR #512; see the README's unresolved list for owner confirmation of AC2.

## 4. Data model / schema changes

```rust
// ComponentLink (node.rs)
/// Slot content guid per slot property name (`slotContentIdValue` on the property's varValue).
#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
pub slot_content_ids: BTreeMap<String, String>,
```

- Serde name `slot_content_ids`. Default empty. `ComponentLink::is_empty` (`node.rs:296-312`) includes it.
- Back-compat: old files have no key, so the map is empty. Files that use no slot ids stay byte-identical.
- `slot_content_id` stays as it is.
- `.fig`: no `NodeChange` field change. New use of `ComponentPropAssignment.varValue` (3) and `ComponentPropDef.varValue` (9).
- Testkit: add `VariableData{value=1, dataType=2, resolvedDataType=3}`, `VariableAnyValue{slotContentIdValue=18}` (with other members as the exporter needs), `SlotContentId{guid=1}`, and the enums `VariableDataType{…SLOT_CONTENT_ID=18}` and `VariableResolvedDataType{…SLOT_CONTENT_ID=12}`. Add `varValue` to `ComponentPropDef` (9) and `ComponentPropAssignment` (3).

## 5. Contracts

- `designos_doc::rename_prop` and `delete_prop` keep their signatures. The new behaviour is in rule 4.
- No MCP or CLI change.

## 6. File-format mapping (RESOLVED)

| Path | Id |
|---|---|
| `NodeChange.componentPropAssignments` | 268 |
| `ComponentPropAssignment.varValue` | 3 |
| `VariableData.value` | 1 |
| `VariableAnyValue.slotContentIdValue` | 18 |
| `SlotContentId.guid` | 1 |
| `NodeChange.componentPropDefs` | 266 |
| `ComponentPropDef.varValue` | 9 |
| `VariableData.dataType` | 2, `VariableDataType.SLOT_CONTENT_ID = 18` |
| `VariableData.resolvedDataType` | 3, `VariableResolvedDataType.SLOT_CONTENT_ID = 12` |
| `NodeChange.componentPropRefs` | 267 |
| `ComponentPropRef.componentPropNodeField` | 4, `ComponentPropNodeField.SLOT_CONTENT_ID = 4` (binds a slot frame to its property; already handled at `fig.rs:1688`) |

Source: Figma 126.9.13 embedded schema (local editor bundle) and the issue #160 report. Not verified against a Figma-written file with slots, because none exists (#160 counts are 0).

## 7. UI

None.

## 8. Edge cases

1. A component with `Body` and `Footer` slots. The instance assigns `Body → 7:1` and `Footer → 7:2`. Native save/load, `.fig` round trip and rename `Body → Content` keep both ids separate.
2. A migrated or legacy node with `slot_content_id = "7:42"` and an empty map. Export writes no `varValue` on any slot property. `jsonSerialized` is unchanged (it is removed only by #110).
3. Deleting `Body` while `slot_content_id = "7:42"` and `slot_content_ids["Body"] = "8:2"`: the map loses `Body`, and `slot_content_id` stays `"7:42"` (round 2 MAJOR 2).
4. An assignment with `varValue.dataType = SLOT_CONTENT_ID` but no guid: ignored.
5. An assignment whose def id is unknown: skipped, as on `main` (`fig.rs:1674`).
6. Two instances of the same component: each keeps its own map. Sync never copies the map from the component to instances (instances have their own assignments).

## 9. Non-goals

- Guessing the property for a legacy `slot_content_id`.
- Editing slot content in instances (#34).
- Writing `slot_content_ids` to REST JSON (REST export writes no component properties).

## 10. Test plan

Seams: `crates/designos-doc/src/slot_model_tests.rs` and `crates/designos-fig/tests/fig_slots.rs`.

| Test | Fails before |
|---|---|
| `two_slot_props_on_one_node_keep_separate_content_ids_through_native_json` | yes |
| `two_slot_props_keep_separate_content_ids_through_fig_round_trip` (assignments `Body→7:1`, `Footer→7:2` through `export_fig` + `import_fig`) | yes |
| `slot_content_id_is_read_from_assignment_var_value` (hand-built real-numbered record; `slot_content_ids["Body"] == "7:1"`) | yes |
| `legacy_node_slot_content_id_round_trips_unchanged_and_is_never_assigned_to_a_property` (`slot_content_id = "7:42"`, two slots, export: no `varValue` with `slotContentIdValue` on either def or assignment) | yes on PR head `d3bd50d` |
| `renaming_a_slot_property_moves_its_content_id_on_owner_and_instances` | yes |
| `deleting_a_slot_with_its_own_map_entry_keeps_the_node_slot_content_id` | yes on PR head `5c9b551` |
| `duplicate_clears_slot_content_ids` | yes |

## 11. Performance budgets

No `CLAUDE.md` §4 invariant applies. The map is serde-skipped when empty, and `doc_hash` streams it with the zero-allocation serializer.

## 12. Dependencies and build order

- Depends on #31 (merged), #112 (copy semantics) and #107 (shares `component_props` in `fig.rs`).
- Fig queue: #107, then #108, then #110. #152 (codegen) and #34 read `slot_content_ids` once both land.

## 13. Owner rulings that apply

- PR #49 coordinator review (2026-10-17): "Keep the model at one id per node for now. One id per property changes the saved format and is a follow-up." This task is that follow-up.
- Owner ruling of 2026-10-18: the release session fixes PR #512.
- The legacy-migration drop is a release-session decision, not an owner ruling. It changes the reading of AC2 ("old files migrate" becomes "old files keep working unchanged"). It needs owner confirmation; see the README's unresolved list.
