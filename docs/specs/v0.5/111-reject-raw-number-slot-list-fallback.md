# #111 Slots: reject the raw-number fallback in the slot-list parser

- Issue: https://github.com/jangtrinh/design-os-studio/issues/111 (audit E3-5, epic #16, S / P1)
- Spec base: `main` at `767acf9`.
- Open PR: #492 (`task/111-reject-slot-raw-number-fallback`, +27 −6). No review comment yet.
- Hot files: `crates/designos-fig/src/rest_import.rs`, `crates/designos-fig/src/fig_export.rs` (serial queue).

## 1. Summary and user story

As a user who opens a `.fig` or REST JSON file from anyone, I must not have a slot in my document silently bound to an unrelated layer. Today the file can do that by writing a bare number that happens to match one of our internal `NodeId`s.

## 2. Current state

- FACT: `rest_import.rs:1543-1557` (`resolve_swaps`, `SLOT:` branch) splits the CSV and resolves each part with:

  ```rust
  self.ids.get(part).copied().or_else(|| part.parse::<u64>().ok().map(NodeId))
  ```

  A part that is not a file id, but parses as a number, becomes our internal id as-is.
- FACT: the `SLOT:` CSV comes from `componentProperties[*].value` of type `SLOT`, at `rest_import.rs:1237-1240`.
- FACT: the export writes `PropValue::Slot(ids)` at `fig_export.rs:1049`. An id with no guid in `cx.node_guids` falls back to writing its raw number. PR #492 removes that fallback; this spec keeps that change.

## 3. Behaviour rules

1. A slot-list entry resolves only through `self.ids` (file id to `NodeId`). Any other entry is dropped.
2. Every dropped entry adds 1 to `ImportNotes.dropped_slot_refs` (see §4). Nothing vanishes silently (delivery law 1).
3. Export writes only ids that have a guid in `cx.node_guids`. Unmapped ids are omitted. The exporter has no report type today, and this task does not add one. The omission is enforced and covered by a test.
4. Order is preserved for the entries that do resolve. Duplicates are kept as the file wrote them, because Figma's order is meaningful.
5. An empty CSV gives `PropValue::Slot(vec![])`, as today.

## 4. Data model / schema changes

- FACT: `designos-fig` returns no import report today. `lib.rs:37-42` exports `import_fig` and `import_rest_json`, which return only a `Document`. The one precedent is `pen::import_pen_json_with_warnings` (`lib.rs:40`).
- New public type, following that precedent:

  ```rust
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
  pub struct ImportNotes { pub dropped_slot_refs: usize }
  ```

  It is not serialized, so `.designos` is unchanged. #109 and #107 add their own counters to this same struct later.
- No `.fig` field change.

## 5. Contracts

- `pub fn import_rest_json_with_notes(json: &str) -> Result<(Document, ImportNotes), FigError>` and `pub fn import_fig_with_notes(bytes: &[u8]) -> Result<(Document, ImportNotes), FigError>`. They take the same arguments as `import_rest_json` (`rest_import.rs:500`) and `import_fig` (`fig.rs:173`). `import_rest_json` and `import_fig` become thin wrappers that discard the notes, so every caller keeps compiling.
- Internal: `fn resolve_slot_csv(&self, csv: &str) -> (Vec<NodeId>, usize /* dropped */)`, extracted from `resolve_swaps` so it can be unit-tested.
- No MCP or CLI change. A user-facing toast for this counter is out of scope here. The paste path (#109) already shows its own.

## 6. File-format mapping

- REST: `componentProperties["<name>#<id>"] = { "type": "SLOT", "value": "<id>,<id>,..." }`. This is our own transport form, carried through `fig.rs` `component_props` at `fig.rs:1671-1676`.
- `.fig`: slot children of an instance live in the real file as the instance's own children and `isSlotContent`. The CSV is an internal hop. No Figma fact is needed. RESOLVED.

## 7. UI

None. The counter is exposed through the API in §5. Wiring it into an app notice is a follow-up issue to file when this lands.

## 8. Edge cases

1. `"12"`, where `12` is an existing internal `NodeId` of an unrelated node: dropped, counter = 1, and the slot does not contain node 12.
2. `"1:2"`, a valid file id in `ids`: resolved.
3. `"1:2, 99:99"`, with the second id not in the file: the result is `[id(1:2)]` and counter = 1.
4. `"-5"`, `"1e3"` or `""` entries: dropped, with no panic.
5. 10,000 entries: linear time, no quadratic `contains`.
6. Export of a slot that holds an id deleted from the document: omitted, so the file stays valid.

## 9. Non-goals

- Changing where slot content is stored in Figma's format (#108).
- Remapping slot ids on paste (#109).

## 10. Test plan

Seam: `crates/designos-fig/tests/fig_slots.rs`, plus a unit test on `resolve_slot_csv`.

- `raw_numeric_slot_entries_are_rejected_and_do_not_bind_unrelated_nodes` (the PR #492 test, kept):
  1. Build a REST document where node id `5` exists internally;
  2. give the instance the slot value `"5"`;
  3. assert `PropValue::Slot(vec![])` and `dropped_slot_refs == 1`.
  - Fails before: yes.
- `mixed_slot_list_keeps_order_of_resolvable_ids`: the value `"1:3,7,1:2"` gives `[id(1:3), id(1:2)]` and `dropped == 1`.
- `export_omits_slot_ids_without_a_guid`:
  1. Put `PropValue::Slot([NodeId(999_999)])` on an instance, with no node 999999;
  2. export, then re-import;
  3. assert an empty slot.
  - Fails before: yes on `main` (the raw number is written).

## 11. Performance budgets

None of the `CLAUDE.md` §4 invariants is touched. The import path stays O(n) in slot entries.

## 12. Dependencies and build order

- Depends on #31 (merged).
- Runs in the fig hot-file queue before #112, #107, #108 and #110. It is the smallest diff, so it goes first.

## 13. Owner rulings that apply

- Delivery-workflow law 1 ("nothing vanishes silently") applies to the dropped-entry counter.
- Brainstorm no-gos: no loosened assertions.
- No specific owner ruling exists for this task.
