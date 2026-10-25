# #107 Slots: real preferredValues object for SLOT and INSTANCE_SWAP

- Issue: https://github.com/jangtrinh/design-os-studio/issues/107 (audit E3-1, epic #16, M / P1)
- Spec base: `main` at `767acf9`.
- Open PR: #510 (`task/107-real-preferred-values-object`). Release reviews:
  - Round 1, head `3b8fdb9`: one MAJOR (an instance steals the key). Fixed on the branch.
  - Round 2, head `c035285`: two new MAJORs (the key is not stored on the component; an instance key overwrites the owner on export).
  - The owner ruled on 2026-10-18 that the release session fixes this PR itself.
- Hot files: `fig.rs`, `fig_export.rs`, `rest_import.rs`, `testkit.rs` (serial queue).

## 1. Summary and user story

As a designer importing a Figma file whose slot and instance-swap properties list preferred components, I need:
- the "preferred" list in Studio to show those components;
- the same list to come back after I save to `.fig` and reopen;
- the list to survive in a second file that uses the same library key.

Today real files give empty lists, because Figma stores the list as an object of library keys, not as a guid list.

## 2. Current state

- FACT: `fig.rs:1657-1662` (`component_props`) reads `preferredValues` as an array of GUIDs (`guid_str`). In a real file the field is a message, so `farr` yields nothing and the list is empty for both prop types.
- FACT: `rest_import.rs:1188-1195` queues `PREF:<id-or-key>` for INSTANCE_SWAP. `rest_import.rs:1211-1215` queues `SLOTPREF:<id-or-key>` for SLOT. `resolve_swaps` (`rest_import.rs:1521-1541`) resolves them only through `self.ids`, the file node ids. A library key never matches.
- FACT: `fig_export.rs:1004-1009` and `:1024-1028` write `preferredValues` as `V::Array` of guids.
- FACT: the synthetic schema declares `ComponentPropDef.preferredValues` as `GUID[]` at id 6 (`testkit.rs:406`).
- FACT: component publish keys exist only in transient maps:
  - `component_keys` (REST `components` map, `rest_import.rs:557`, `:663`);
  - `library_keys` (`rest_import.rs:762`, `:860`).

  A `Component` or `ComponentSet` node does not store its key. `fig.rs:1179` copies `componentKey` into the REST bag. The exporter invents `comp:{session}:{local}` when no key is known (per review, `fig_export.rs:257` on the PR head).
- FACT: on the owner's file, 15 INSTANCE_SWAP defs carry a non-empty `preferredValues` (issue #160 report). Real data for INSTANCE_SWAP exists; real data for SLOT does not.

## 3. Behaviour rules

1. **Import, object form.** For a `ComponentPropDef` of type `INSTANCE_SWAP` (3) or `SLOT` (7) whose `preferredValues` is a message, take each `instanceSwapValues[i] = {type, key}`.
   - `type = COMPONENT` (0) resolves `key` against component publish keys.
   - `type = STATE_GROUP` (1) resolves `key` against component-set publish keys.
2. **Key index (import).** Built once per import, in document order, with first-owner-wins:
   - (a) `SYMBOL` nodes: `componentKey` (216), falling back to `key` (319). These map to the component.
   - (b) `FRAME` nodes with `isStateGroup` (state groups): `componentKey`, falling back to `key`. These map to the component set.
   - (c) REST `components[id].key` and `componentSets[id].key` maps.
   - (d) Only when no (a)-(c) owner exists for a key: an `INSTANCE` carrying `componentKey`, mapped to its `symbolData.symbolID` component, never to the instance.

   An instance never overwrites an owner.
3. **Key persistence.** Every `Component` and `ComponentSet` whose key is known from 2(a)-(c) stores it in `fig_metadata["componentKey"]`. Preferred entries resolve to `NodeId`s for the model. The publish key stays on the target node, so export can write it back unchanged.
4. **Unresolved keys** (no owner in this file, for example a library not loaded) are not dropped silently:
   - store them on the prop as `unresolved_preferred: Vec<PreferredKey>`, see §4;
   - write them back on export;
   - count them in `ImportNotes.unresolved_preferred_keys`.
5. **Legacy form.** A `preferredValues` that is an array of GUIDs, or of strings (our older exports and REST), is still imported through the existing `PREF:` / `SLOTPREF:` path.
6. **`stringValues`** (deprecated, field 1) is read as keys of type COMPONENT when `instanceSwapValues` is empty.
7. **`parameterValues`** (field 3) is ignored on import. The model cannot hold it, and it is written as an empty array (FACT, from the PR #510 review "checked" list).
8. **Export.** For each preferred `NodeId`, write `{type, key}`:
   - `type = STATE_GROUP` if the target is a `ComponentSet`, else `COMPONENT`;
   - `key = target.fig_metadata["componentKey"]`. If the target has no key, use the synthetic `comp:<session>:<local>` key that the exporter already writes onto that same component's `componentKey`, so a re-import resolves it.

   Then append `unresolved_preferred` verbatim. Write the field as a `ComponentPropPreferredValues` message, never as a guid array.
9. **Export key ownership.** When the exporter seeds `componentKey` onto components, a key already owned by a component (from rule 3) is never reassigned to another component from an instance's `component_key`. Seeding from an instance is `or_insert` only if no component owns that key.

## 4. Data model / schema changes

- `designos-doc`:
  - `PropKind::InstanceSwap { preferred: Vec<NodeId> }` (`node.rs:209`) is unchanged.
  - `SlotPropConfig.preferred: Vec<NodeId>` (`node.rs:245-246`) is unchanged.
  - New, on `ComponentPropDef`:
    ```rust
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unresolved_preferred: Vec<PreferredKey>,
    ```
    ```rust
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct PreferredKey { pub kind: PreferredKeyKind, pub key: String }
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub enum PreferredKeyKind { Component, StateGroup }
    ```
  - Back-compat: absent means empty. Old files load unchanged.
- `fig_metadata["componentKey"]` on Component and ComponentSet. It is a string field in the existing bag (`node.rs:154-155`), so there is no schema change.
- `designos-fig` testkit:
  - `ComponentPropDef.preferredValues` becomes `ComponentPropPreferredValues` (message).
  - Add the defs `ComponentPropPreferredValues`, `InstanceSwapPreferredValue` and `InstanceSwapPreferredValueType`.
  - Field ids follow the real schema (§6). #110 renumbers the whole schema. This task already uses real ids for the three new defs.
- `ImportNotes` (introduced by #111) gains `unresolved_preferred_keys: usize`.

## 5. Contracts

- `fn resolve_preferred(cx: &Ctx, v: &InstanceSwapPreferredValue) -> Result<NodeId, PreferredKey>`
- `fn encode_preferred_values(cx: &ExportCx, preferred: &[NodeId], unresolved: &[PreferredKey]) -> V`
- No MCP or CLI change. MCP `get_component` (if it lists props) shows `preferred` ids as today.

## 6. File-format mapping (RESOLVED)

| Message / field | Id |
|---|---|
| `ComponentPropDef.preferredValues` | 8 (`ComponentPropPreferredValues`) |
| `ComponentPropPreferredValues.stringValues` | 1 (`string[]`, deprecated) |
| `ComponentPropPreferredValues.instanceSwapValues` | 2 (`InstanceSwapPreferredValue[]`) |
| `ComponentPropPreferredValues.parameterValues` | 3 (`VariableData[]`) |
| `InstanceSwapPreferredValue.type` | 1 |
| `InstanceSwapPreferredValue.key` | 2 (`string`) |
| `InstanceSwapPreferredValueType` | `COMPONENT=0`, `STATE_GROUP=1` |
| `NodeChange.componentKey` | 216 |
| `NodeChange.key` | 319 |
| `NodeChange.isStateGroup` | 225 (`bool`) |
| `NodeChange.symbolData` | 113, with `symbolID=1` |

Source: embedded kiwi schema in the Figma 126.9.13 editor bundle, and the issue #160 schema report. REST: `componentPropertyDefinitions[*].preferredValues = [{ "type": "COMPONENT" | "COMPONENT_SET", "key": "<key>" }]`. That is REST's public shape; our importer already accepts the `{key}` form at `rest_import.rs:1191`.

RESOLVED, Q107-1 (REA 2026-10-18). REST writes `"COMPONENT_SET"`, both as the node `type` of a component set and as `preferredValues[*].type` for set entries. `STATE_GROUP` is a `.fig` (kiwi) name only. The importer maps REST `COMPONENT_SET` → `PreferredKeyKind::StateGroup`. Still accepting `"STATE_GROUP"` on REST import is harmless (tolerant input), but no Figma REST payload produces it. Our REST export does not write component properties, so this only affects import.

### Facts (REA 2026-10-18)

1. FACT. In `figma/rest-api-spec` `openapi/openapi.yaml` (version 0.45.0, fetched 2026-10-18):
   - `InstanceSwapPreferredValue.type` has `enum: [COMPONENT, COMPONENT_SET]`, described as "Type of node for this preferred value", with `key` being the "Key of this component or component set".
   - `ComponentSetNode.type` has `enum: [COMPONENT_SET]`.
   - The node discriminator maps `COMPONENT_SET: "#/components/schemas/ComponentSetNode"`.
2. FACT. `STATE_GROUP` occurs 0 times in that openapi.yaml, and 0 times in `@figma/plugin-typings` 1.142.0 `plugin-api.d.ts`. The Plugin API uses `'COMPONENT' | 'COMPONENT_SET'` too.
3. FACT. Kiwi side, from the schema compiled into Figma's cached fullscreen wasm (`static.figma.com/fullscreen/c36343e1…/fullscreen-wasm/compiled_wasm.wasm.br` in `~/Library/Application Support/Figma/DesktopProfile/v43/Cache/Cache_Data`; three other cached builds are identical):
   ```
   message InstanceSwapPreferredValue { InstanceSwapPreferredValueType type = 1; string key = 2; }
   enum InstanceSwapPreferredValueType { COMPONENT = 0; STATE_GROUP = 1; }
   ```
   This matches §6. `NodeChange.isStateGroup = 225` and `ComponentPropDef.preferredValues = 8` are unchanged.
4. Method. `rea` 6.7.0 (`npx -y rea-agents`) was used for the JS chunks. Its wasm lane was unavailable (WABT not installed), so the binary schema was decoded directly from the wasm data section at the `MessageType` definition.

## 7. UI

None here. The preferred list is shown by #32 (slot settings) and #150 (the `+` picker).

## 8. Edge cases

1. An icon `SYMBOL` with key `K` precedes a button that contains an instance repeating `K`. Preferred `{COMPONENT, K}` resolves to the symbol (the round 1 MAJOR).
2. A key carried only by an instance: it resolves to that instance's `symbolID` component.
3. A master with only `key` (319) and no `componentKey`: it resolves, and the round trip keeps `K` (the round 2 MAJOR 1).
4. A REST file with `components[id].key = "K_ICON"` and no instance: it resolves, and a `.fig` save writes `componentKey: K_ICON` on Icon and `{COMPONENT, K_ICON}` in preferred.
5. A stand-in fixture: Icon owns `K_ICON`, and a later instance repeats `K_ICON` with `symbolID` pointing at StandIn. Export writes `K_ICON` only on Icon, and the re-import still points preferred at Icon (the round 2 MAJOR 2).
6. A key with no owner: it is kept in `unresolved_preferred`, round-trips, and is counted.
7. A state-group key resolves to the `ComponentSet` node. A `COMPONENT`-typed entry whose key belongs to a set is a mismatch: keep it unresolved.
8. A legacy guid list and an object in the same file: each def is decoded by its own shape.

## 9. Non-goals

- Fetching library files to resolve keys (no network: brainstorm no-go).
- Modelling `parameterValues`.
- The slot content id (#108) and schema renumbering beyond the three defs (#110).

## 10. Test plan

Seam: `crates/designos-fig/tests/fig_slots.rs`, using the real-numbered synthetic fixture from #160.

| Test | Fails before |
|---|---|
| `real_preferred_values_object_round_trips_for_slot_and_instance_swap_and_keeps_legacy_guid_list` | yes |
| `preferred_key_resolves_to_the_component_even_when_an_instance_of_it_comes_later` | yes |
| `preferred_key_carried_only_by_an_instance_resolves_to_the_instanced_component` | yes |
| `component_with_only_key_field_keeps_its_publish_key_through_fig_round_trip` (new; master `key=K_ICON`, no instance; import, `export_fig`, import; assert `fig_metadata["componentKey"] == "K_ICON"` and preferred = Icon) | yes on PR head `c035285` |
| `rest_component_key_survives_fig_save_when_no_instance_repeats_it` (new) | yes |
| `instance_key_never_reassigns_a_component_owned_key_on_export` (new; stand-in fixture through `export_fig`; preferred still Icon) | yes |
| `unresolved_preferred_key_round_trips_and_is_counted` (new; `{COMPONENT, "K_MISSING"}`; `unresolved_preferred == [..]`, `notes.unresolved_preferred_keys == 1`, export writes it back) | yes |

Fixture literals: keys `"K_ICON"`, `"K_AVATAR"`, `"K_SET"` and `"K_MISSING"`; GUIDs `1:10` (Icon), `1:11` (Avatar), `1:20` (set), `1:30` (instance).

## 11. Performance budgets

No `CLAUDE.md` §4 invariant applies (import and export only). The key index is built in one pass, O(n), with `HashMap<String, NodeId>`.

## 12. Dependencies and build order

- Depends on #31 (merged), #111 (introduces `ImportNotes`) and #160 (real-numbered fixture).
- Fig queue: #111, #112, #107, #108, #110.
- #109 (paste) leaves `unresolved_preferred` alone: these are publish keys, not node ids.

## 13. Owner rulings that apply

- R2.6 (owner-decisions round 2, item 6): use the real `preferredValues` shape from the owner's file once #160 verifies it. #160 found no SLOT data, but 15 INSTANCE_SWAP objects. The shape in §6 comes from the file's embedded schema, and tests use the synthetic fixture.
- Owner ruling of 2026-10-18: the release session fixes PR #510.
