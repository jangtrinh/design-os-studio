# #112 Slots: doc-op semantics and unknown SlotPropConfig keys

- Issue: https://github.com/jangtrinh/design-os-studio/issues/112 (audit E3-6, epic #16, S / P2)
- Spec base: `main` at `767acf9`.
- Open PR: #493 (`task/112-slots-doc-ops-and-unknown-keys`). Release review found one MAJOR: Duplicate and Paste go through `clipboard::instantiate`, which still keeps `slot_content_id`. The owner ruled on 2026-10-18 that the release session takes this fix itself.
- Hot files: `crates/designos-doc/src/components.rs`, `crates/designos-doc/src/node.rs`, `crates/designos-fig/src/rest_import.rs`, `crates/designos-app/src/clipboard.rs`.

## 1. Summary and user story

As a designer who duplicates, pastes and edits slotted components, I expect:
- each copy to be its own slot content, never sharing an identity with the original;
- a slot-content flag set on a main component to reach its instances;
- slot settings I did not author, or that a newer Figma wrote, to survive a save in our format.

## 2. Current state

- FACT: `copy_rec` (`components.rs:96-121`), used by `duplicate_subtree` (`components.rs:417-419`) and `create_instance`, clones the node with `orig.clone()`. It clears `source_id`, and when `link` is set it also clears `overrides`, `props` and `slot_configs`. It never clears `component.slot_content_id`, so a duplicate shares the guid.
- FACT: `clipboard::instantiate_mapped` (`crates/designos-app/src/clipboard.rs:261-290`) clones each node and clears `source_id`. It keeps `slot_content_id`. Duplicate and Paste use it (`ops.rs` per the PR #493 review: `ops.rs:413` and `:472`).
- FACT: `apply_reference` (`components.rs:713-760`) mirrors `bindings` and `is_slot` onto non-top instance nodes at `components.rs:751-754`. It does not mirror `is_slot_content`.
- FACT: `SlotPropConfig` (`node.rs:225-247`) has exactly five optional scalars plus `preferred`. `rest_import.rs:1197-1210` reads only those five keys, so any other `slotPropConfig` key is dropped.

## 3. Behaviour rules

1. **Copy clears the slot content identity.** Every node produced by these paths has `component.slot_content_id = None`:
   - `copy_rec` (duplicate and create-instance);
   - `clipboard::instantiate_mapped` (Duplicate, Paste, Paste over selection, alt-drag duplicate, Figma paste).

   The node's other slot fields are copied as they are: `is_slot`, `is_slot_content`, `slot_configs`, and on instance roots `values` with `PropValue::Slot`.
2. **Undo** of a duplicate or paste restores nothing extra, because the original is untouched.
3. **`apply_reference` mirrors `is_slot_content`** next to `is_slot`, for non-top nodes, and with the same guard (`!top`).
4. **Unknown keys are preserved in the native format.** `SlotPropConfig` gets an `extra` map that keeps every `slotPropConfig` key the importer does not model, with its JSON value untouched.
   - `.designos` save and load keep `extra` byte-for-byte as a JSON value.
   - `.fig` export cannot carry keys the kiwi schema lacks. `extra` is not written to `.fig`. That is a known limit, stated in the export doc comment.
5. `extra` never overrides a modelled field. If a key is modelled (one of the five), it goes into the typed field and never into `extra`.

## 4. Data model / schema changes

```rust
pub struct SlotPropConfig {
    // ... existing five Option fields + preferred, unchanged ...
    /// `slotPropConfig` keys this version does not model, by the key name the file used;
    /// each value is the compact JSON text of the original value. Native format only.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, String>,
}
```

- Serde name `extra`. It is a nested object, not flattened, so no key can collide with the typed snake_case fields.
- Value type `String` (JSON text), not `serde_json::Value`, because `SlotPropConfig` derives `Eq` (FACT, `node.rs:224`) and `Value` is not `Eq`. PR #493 used `#[serde(flatten)] BTreeMap<String, Value>`. Replace it with this form.
- Back-compat:
  - Old `.designos` files have no `extra`, so `default` gives an empty map.
  - Older builds that read a newer file ignore the unknown `extra` key, because the struct has no `deny_unknown_fields` (FACT, `node.rs:224-225`).
  - Empty maps are skipped on write, so files without unknown keys come out byte-identical.

## 5. Contracts

- No public function signature changes.
- `rest_import.rs` SLOT branch: after reading the five keys, every remaining `(k, v)` in `cfg` goes to `extra.insert(k, v.to_string())`.
- `export_rest_json` does not write component properties today (FACT, from the PR #512 review: "REST export still omits component properties entirely"). So `extra` has no REST export path yet. No new path is added here.

## 6. File-format mapping

- Real `.fig` `SlotPropConfig{stretchChildOnInsert=1, displayByDefault=2, minChildren=3, maxChildren=4, allowPreferredValuesOnly=5}`. Source: embedded editor schema, see spec 160 §6. RESOLVED.
- A `.fig` decoded through its own embedded schema can carry a field with a higher id that a newer Figma added. `fig.rs` `component_props` copies the whole `slotPropConfig` object (`fig.rs:1663-1665`), so such a key reaches `rest_import` and lands in `extra`.

## 7. UI

None.

## 8. Edge cases

1. Duplicate an instance whose `slot_content_id = "9:9"`: the copy has `None`, the original keeps `"9:9"`.
2. Paste the same instance twice: both copies have `None`.
3. Copy a main component with a slot frame (`is_slot = true`): the copy keeps `is_slot = true`, because it is a real slot of the new component.
4. A main component's slot child flips `is_slot_content = true`, then sync runs: every instance mirror flips too. A top instance root is not changed.
5. `slotPropConfig: {"minChildren": 1, "wrapMode": "WRAP", "future": {"a": [1, 2]}}`: `min_children = Some(1)`, and `extra = {"future": "{\"a\":[1,2]}", "wrapMode": "\"WRAP\""}`. A native round trip is equal.
6. `slotPropConfig: {}`: the config exists and every field is `None` with an empty `extra`. This is the existing "settings present" behaviour from `fig.rs:1663`.

## 9. Non-goals

- Making `slot_content_id` per property (#108).
- Writing `extra` to `.fig`.
- REST export of component properties.

## 10. Test plan

Seams: `crates/designos-doc/src/slot_model_tests.rs`, `crates/designos-fig/tests/fig_slots.rs`, and `crates/designos-app/src/ops.rs` tests.

| Test | Input | Expected | Fails before |
|---|---|---|---|
| `duplicate_subtree_clears_slot_content_id_so_copies_do_not_share_a_guid` | instance with `slot_content_id = Some("12:34")`; `duplicate_subtree` | copy `None`, source `Some("12:34")` | yes |
| `duplicate_and_paste_through_the_clipboard_clear_slot_content_id` | app with an instance `slot_content_id = Some("9:9")`; run `duplicate_selection`, then copy and `paste` | both new nodes `None`; one undo removes each copy | yes (the MAJOR from the release review) |
| `sync_instances_propagates_is_slot_content_flipped_on_the_component` | set `is_slot_content = true` on the component's body child; `sync_instances` | instance mirror `is_slot_content == true` | yes |
| `unknown_slot_prop_config_keys_are_preserved_in_native_format` | REST with `"slotPropConfig": {"minChildren": 1, "wrapMode": "WRAP"}`, import, `to_json`, `from_json` | `extra["wrapMode"] == "\"WRAP\""`, `min_children == Some(1)` | yes |
| `modelled_keys_never_land_in_extra` | the five keys present | `extra.is_empty()` | guard |

## 11. Performance budgets

No `CLAUDE.md` §4 invariant applies. `doc_hash` streams with `HashSerializer`. The new `extra` map is serialized through serde like any other field, so nothing calls `to_json`.

## 12. Dependencies and build order

- Depends on #31 (merged).
- Order in the fig queue: #111, then #112, then #107.
- Touches `clipboard.rs`, which #109 also edits. Run #112 before #109, and rebase #109.

## 13. Owner rulings that apply

- PR #49 coordinator acceptance ruling (PR #49 comment, 2026-10-17): "unknown fields preserved" was accepted narrowly then. Carrying unknown `SlotPropConfig` keys in the native format is this follow-up.
- Owner ruling of 2026-10-18: the release session fixes PR #493 itself.
