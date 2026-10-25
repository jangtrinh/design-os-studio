# #160 Verify the owner's VSF - PCP.fig carries slot data and derive a synthetic slot fixture

- Issue: https://github.com/jangtrinh/design-os-studio/issues/160 (task E3-15, epic #16, S / P1)
- Spec base: `main` at `767acf9` (2026-10-18).
- Open PR: #495 (`task/160-verify-vsf-pcp-slots-fixture`). Release review asked for changes (one MAJOR, one MINOR). On 2026-10-18 the owner ruled that the release session fixes this PR itself; other workers must not push to it.
- Hot files: none in production code. The test lives in `crates/designos-fig/tests/fig_slots.rs`.

## 1. Summary and user story

As the maintainer of the slot import and export, I need to know whether the owner's reference file contains real slot records. This decides whether #107 and #110 can be checked against Figma-written data, or only against a synthetic fixture shaped like the real schema. The owner's file must never enter the repo.

## 2. Current state

- FACT: the slot model has been on `main` since PR #49:
  - `SlotPropConfig` at `crates/designos-doc/src/node.rs:225-247`;
  - `ComponentLink.slot_configs / is_slot / is_slot_content / slot_content_id` at `node.rs:261-273`.
- FACT: the import reads `isSlot` and `isSlotContent` at `crates/designos-fig/src/fig.rs:1193`, and `jsonSerialized["slot-content-id"]` at `fig.rs:1199`. `rest_import.rs:1019-1021` maps them onto the node.
- FACT: the synthetic export schema numbers `ComponentPropType` as `BOOL, TEXT, INSTANCE_SWAP, VARIANT, SLOT` (0..4) at `crates/designos-fig/src/testkit.rs:162`. It numbers `ComponentPropDef` fields differently from Figma (`type=3`, `preferredValues=6` as a GUID list, `slotPropConfig=7`) at `testkit.rs:400-407`.
- FACT, measured on issue #160 (comment of 2026-10-17 22:38), on the owner's file of 124,453,334 bytes and 153,220 `nodeChanges`:

  | Construct | Count |
  |---|---|
  | `isSlot` true | 0 |
  | `isSlotContent` true | 0 |
  | `SLOT` prop defs | 0 |
  | `slotPropConfig` | 0 |
  | `SLOT_CONTENT_ID` refs | 0 |
  | `slotContentIdValue` assignments | 0 |
  | `INSTANCE_SWAP` defs with a non-empty `preferredValues` | 15 |

- FACT: `/Users/jang/Downloads/VSF - PCP.fig` on this machine is 124,453,334 bytes, the same size as the file measured above. It is a ZIP whose first entry is `canvas.fig` (header `PK\x03\x04`, name `canvas.fig`). I did not decode it again for this spec.
- FACT, from PR #495 review: the test asserts only the schema tags and `total_nodes > 0`, so it passes whatever the slot counts are. It decodes with `decode_message`, which keeps multi-gigabyte heavy fields; `kiwi.rs:391` documents those, and the import path drops them.

## 3. Behaviour rules

1. The scan reads the file only when it exists at `$HOME/Downloads/VSF - PCP.fig`, or at the path in `DESIGNOS_SLOT_REFERENCE`. When the file is absent the test returns early and prints `skipped: reference file absent`. CI never has the file.
2. Decode with `Schema::decode_message_for_import` (skip-heavy), never with `decode_message`.
3. Count these over every `nodeChanges` record and over every nested `symbolData.symbolOverrides[]` and `derivedSymbolData[]` entry:
   - `isSlot == true`;
   - `isSlotContent == true`;
   - `componentPropDefs[].type == SLOT`;
   - `componentPropDefs[].slotPropConfig` present;
   - `componentPropRefs[].componentPropNodeField == SLOT_CONTENT_ID`;
   - `componentPropAssignments[].varValue.value.slotContentIdValue` present;
   - `componentPropDefs[].varValue.value.slotContentIdValue` present;
   - `jsonSerialized` present (expected 0, because the real schema has no such field);
   - `INSTANCE_SWAP` defs whose `preferredValues.instanceSwapValues` is non-empty.
4. When the file is present, the test asserts the measured counts as literals. A count of zero is the AC2 report, kept in source.
5. If any slot count is non-zero, AC3 applies:
   - add a content-free synthetic fixture that reproduces the structure;
   - add a round-trip test for that structure;
   - change the literal counts in the test.
   No owner names, text or images may enter the fixture.
6. Independently of the owner file, add a synthetic fixture numbered like the real schema. It must contain:
   - one component with a `SLOT` prop;
   - a `slotPropConfig` with all five fields;
   - `preferredValues` written as an object;
   - one instance with a `SLOT_CONTENT_ID` assignment.

   #107, #108 and #110 consume this fixture. It is built in code (`testkit`), not committed as a binary blob.

## 4. Data model / schema changes

None in the document model. In tests only: `testkit::figma_real_slot_schema()` (new). It returns the subset of Figma's schema listed in §6 with Figma's real field ids, so fixtures encode the way Figma does.

## 5. Contracts

- Test seam: `crates/designos-fig/tests/fig_slots.rs`.
  - `fn scan_slot_counts(bytes: &[u8]) -> SlotCounts`
  - `struct SlotCounts { is_slot, is_slot_content, slot_defs, slot_configs, slot_content_refs, slot_content_assignments, slot_content_def_values, json_serialized, swap_defs_with_preferred: usize, total_nodes: usize }`
- Errors: a decode failure fails the test with the `KiwiError` text. It is never swallowed.

## 6. File-format mapping (all RESOLVED)

Source: the embedded kiwi schema in the Figma 126.9.13 editor bundle (local Electron code cache), confirmed by the issue #160 report.

| Fact | Value |
|---|---|
| `NodeChange.isSlot` | 463 (`bool`) |
| `NodeChange.isSlotContent` | 495 (`bool`) |
| `NodeChange.componentPropDefs` | 266 |
| `NodeChange.componentPropRefs` | 267 |
| `NodeChange.componentPropAssignments` | 268 |
| `NodeChange.symbolData` | 113 (`SymbolData{symbolID=1, symbolOverrides=2, uniformScaleFactor=3}`) |
| `NodeChange.derivedSymbolData` | 125 |

- `ComponentPropDef{id=1, name=2, initialValue=3, sortPosition=4, parentPropDefId=5, type=6, isDeleted=7, preferredValues=8, varValue=9, parameterConfig=10, description=11, slotPropConfig=12, colorArrayConfig=13, visibleWhen=14}`
- `ComponentPropAssignment{defID=1, value=2, varValue=3, legacyDerivedTextData=4}`
- `ComponentPropRef{nodeField=1, defID=2, zombieFallbackName=3, componentPropNodeField=4, isDeleted=5}`
- `ComponentPropType{BOOL=0, TEXT=1, COLOR=2, INSTANCE_SWAP=3, VARIANT=4, NUMBER=5, IMAGE=6, SLOT=7, EASING=8, ...}`
- `ComponentPropNodeField{VISIBLE=0, TEXT_DATA=1, OVERRIDDEN_SYMBOL_ID=2, INHERIT_FILL_STYLE_ID=3, SLOT_CONTENT_ID=4}`
- `ComponentPropPreferredValues{stringValues=1 (deprecated), instanceSwapValues=2, parameterValues=3}`
- `InstanceSwapPreferredValue{type=1, key=2}`, with `InstanceSwapPreferredValueType{COMPONENT=0, STATE_GROUP=1}`.
- `SlotPropConfig{stretchChildOnInsert=1, displayByDefault=2, minChildren=3, maxChildren=4, allowPreferredValuesOnly=5}`
- `VariableData{value=1, dataType=2, resolvedDataType=3}`
- `VariableAnyValue.slotContentIdValue=18`, with `SlotContentId{guid=1}`.
- `VariableDataType.SLOT_CONTENT_ID=18` and `VariableResolvedDataType.SLOT_CONTENT_ID=12`.
- There is no `SLOT` value in `NodeType` (0..64). A slot is a `FRAME` (4) with `isSlot=true`.
- The real `NodeChange` has no `jsonSerialized` field (0 occurrences in the embedded schema text).

## 7. UI

None.

## 8. Edge cases

1. The file is absent (CI): skip with a message, never fail.
2. The file is present but is another version with different counts: the literal assertion fails, and the failure message prints the whole `SlotCounts` so the maintainer can update it.
3. Slot data appears only inside `symbolOverrides` (an instance override): it is counted by rule 3.
4. The ZIP container holds `canvas.fig` plus images: only `canvas.fig` is decoded.
5. Memory: the skip-heavy decode keeps the process well under the 2 MB-per-field heavy limit the import already uses. A full decode is forbidden.
6. `DESIGNOS_SLOT_REFERENCE` points at a non-`.fig` file: fail with `KiwiError`, because the user asked for that file explicitly.

## 9. Non-goals

- Committing the owner's file, or any byte or string taken from it (ruling R2.6).
- Changing the import or export code. That belongs to #107, #108 and #110.
- Opening an export in Figma.app. That belongs to #110.

## 10. Test plan

- `vsf_pcp_slot_scan_reports_literal_counts_when_present` (rule 4). Expected on the 124,453,334-byte file: `is_slot 0, is_slot_content 0, slot_defs 0, slot_configs 0, slot_content_refs 0, slot_content_assignments 0, slot_content_def_values 0, json_serialized 0, swap_defs_with_preferred 15, total_nodes 153220`.
  - Fails before the change: yes. The current test asserts no counts, so it is green on any file.
- `synthetic_real_numbered_slot_fixture_decodes_with_figma_field_ids`: build the rule 6 fixture with `figma_real_slot_schema()`, decode it, and assert:
  - `type == 7`;
  - the `slotPropConfig` fields 1..5 read back `true, false, 1, 3, true`;
  - `preferredValues.instanceSwapValues[0] == {type: COMPONENT, key: "K_ICON"}`;
  - `slotContentIdValue.guid == 7:42`.
  - Fails before: yes (no such schema helper).
- Mutation check: deleting the counting loop makes the first test fail on the machine that has the file.

## 11. Performance budgets

No `CLAUDE.md` §4 invariant applies, since no app or GPU code changes. Test budget: the scan finishes in under 60 s in a release build on the owner's machine. If it is slower, the test is `#[ignore]`d with a reason and run by hand.

## 12. Dependencies and build order

- Depends on #31 (merged as PR #49).
- Blocks #107 and #110, which take the synthetic real-numbered fixture from here.
- Runs first in the v0.5 slot queue.

## 13. Owner rulings that apply

- R2.6 / D-E3-1 (`plans/261017-epics-full-scope/owner-decisions-and-cross-check.md` round 2, item 6):
  - `VSF - PCP.fig` is the slot reference;
  - verify it first;
  - if it has no slots, report to the owner;
  - never commit it.
- AC2 has already fired: the counts are 0. The report is on the issue. Whether the owner supplies another file is still open; see the README's unresolved list.
- Brainstorm no-gos (`plans/specs/figma-parity-2026/brainstorm.md`): no network calls, no loosened assertions.
