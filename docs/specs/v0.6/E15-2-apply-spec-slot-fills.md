# E15-2 Fill component slots from apply_spec, validated against each slot's recommended components

| Field | Value |
|---|---|
| Epic / pack | E15 / v0.6 AI design |
| Size / priority | M / P0 |
| Depends on | #150 (E3-7, recommended components per slot), E15-1, E15-9 (preset and test edit, section 12) |
| Unblocks | part-2 surface templates, E15-12 (slot lint finding) |
| Touches | `crates/designos-doc/src/spec.rs`, `crates/designos-doc/src/spec_instance.rs` |

## 1. Summary and user story

As an agent placing a Card instance, I fill its `Content` slot with my own layers (or with
instances of the components the slot recommends), and I can switch off an unused slot, without the
instance being detached.

## 2. Current state (FACT)

- Slot model: `PropKind::Slot` (`crates/designos-doc/src/node.rs:210`), `PropValue::Slot(Vec<NodeId>)`
  (`node.rs:200-201`), `SlotPropConfig` with `preferred`, `allow_preferred_values_only`,
  `min_children`, `max_children`, `display_by_default`, `stretch_child_on_insert`
  (`node.rs:225-243`); `ComponentLink.is_slot`, `is_slot_content`, `slot_configs`
  (`node.rs:262-270`); `OverrideField::SlotContent` (`node.rs:188-189`).
- `is_slot_container(doc, id)` (`crates/designos-doc/src/components.rs:208`) identifies slot frames.
- Test coverage of the model: `crates/designos-doc/src/slot_model_tests.rs`.
- `NodeSpec` has no `slots` field (`crates/designos-doc/src/spec.rs:46-124`); E15-1 adds instance
  placement but rejects `children` on an instance root.
- #150 adds recommended components per slot in the picker (open, v0.5).

## 3. Behaviour rules

1. `slots` is a map from slot property name to either a list of `NodeSpec` (fill) or the literal
   `false` (disable). Slot names are the `ComponentPropDef.name` of `PropKind::Slot` definitions on
   the instance's component (or set).
2. Fill: the slot container descendant is located through the slot property binding
   (`ComponentLink.bindings` with `OverrideField::SlotContent` → name) or, for files that mark the
   frame only, `is_slot == true` with a matching layer name. Its children are replaced by the built
   specs; each built root gets `is_slot_content = true`; the container records
   `OverrideField::SlotContent`; the instance root's `component.values[slot] = PropValue::Slot(ids)`.
3. Validation against recommendations (`SlotPropConfig.preferred`):
   - a fill root that is an instance of a preferred component (or of a variant inside a preferred
     set) passes;
   - any other root passes with a lint finding `slot-non-recommended` (severity warning, E15-12
     issues block), naming the recommended components;
   - when `allow_preferred_values_only == Some(true)` (strict slot), a non-recommended root fails the
     whole call with `SLOT_FILL_NOT_ALLOWED`.
4. Child count: fewer than `min_children` or more than `max_children` fails with
   `SLOT_CHILD_COUNT` (bounds in the message). Unset bounds do not constrain.
5. Disable: `false` hides the slot container in this instance (`visible = false`, records
   `OverrideField::Visible`) and keeps its default content untouched underneath; the instance stays
   linked.
6. `stretch_child_on_insert == Some(true)`: every fill root gets `width_sizing = fill` on the slot's
   primary axis unless the spec set width or width_sizing explicitly.
7. Unknown slot name fails with `UNKNOWN_SLOT`, listing the slot names of that component.
8. Filling a slot of a nested instance uses the E15-1 override path: `overrides:[{"path":"Body",
   "slots":{"Content":[...]}}]`. `OverrideSpec` gains the same `slots` field.

## 4. Data model / schema changes

```rust
// NodeSpec and OverrideSpec
#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
pub slots: BTreeMap<String, SlotFillSpec>,

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum SlotFillSpec { Disabled(FalseLiteral), Fill(Vec<NodeSpec>) }
```

`FalseLiteral` deserializes only from JSON `false` (a `true` value is `INVALID_SLOT_VALUE`). No
persisted model change: slot content is stored with the existing fields listed in section 2.
`export_spec` of an instance with filled slots emits `slots`, so export → apply round-trips.

## 5. Contracts

Tool: `designos_apply_spec` (and the `apply_spec` op of `designos_apply_batch`, E15-4).

Example input:

```json
{"parent_id": 1, "spec": {"kind": "instance", "component": "Card",
  "slots": {"Content": [{"kind": "instance", "component": "List item"},
                        {"kind": "text", "text": "Footnote"}],
            "Actions": false}}}
```

Output additions: `name_to_id` includes slot content as `"Card/Content/List item"`; the issues
block (E15-12) carries `slot-non-recommended` findings.

Errors (envelope per E15-5, all `safe_to_retry: true`):

| Code | `valid_options` |
|---|---|
| `UNKNOWN_SLOT` | slot names of the component |
| `SLOT_FILL_NOT_ALLOWED` | recommended component names (strict slot) |
| `SLOT_CHILD_COUNT` | none; message gives `min..max` and received count |
| `INVALID_SLOT_VALUE` | `["false", "[NodeSpec...]"]` |
| `SLOT_CONTAINER_NOT_FOUND` | none; the component declares the slot but no frame is bound to it |

## 6. File-format mapping

- `.designos`: existing `ComponentLink` fields; round-trip test only.
- JSON scene format (#570): slot fill = descendant override replacing `children` of the slot node;
  `slot` lists map to `SlotPropConfig.preferred`. Owned by #570; this task adds the round-trip test
  `card_slot_fill_round_trips_scene_format` gated on #570 being merged.
- `.fig`: slot content uses `isSlotContent` (tag 495) and `slot-content-id` (`node.rs:267-272`),
  already round-tripped by the existing slot model tests.

## 7. UI

No new surface. A filled slot shows its content in the instance on canvas; a disabled slot is hidden
like any hidden layer. The Components panel's slot picker (#150) is unchanged.

## 8. Edge cases

1. Slot defined on a component set but bound in only some variants: filling resolves the container
   in the chosen variant; if absent there, `SLOT_CONTAINER_NOT_FOUND` names the variant.
2. Empty list `[]`: clears the slot (valid when `min_children` is unset or 0).
3. Fill root is itself an instance of the same Card: `INSTANCE_CYCLE` (E15-1).
4. Disable then fill the same slot in one spec: rejected, `INVALID_SLOT_VALUE` (one value per key by
   JSON map construction; a later override entry re-enabling it records `Visible` again).
5. Preferred list empty and slot not strict: every fill passes with no warning.
6. A preferred component lives in a disabled library: it is still accepted by id match, and the
   warning says the library is disabled.

## 9. Non-goals

- The slot placeholder and picker UI (#150).
- Editing `SlotPropConfig` from a spec.

## 10. Test plan

Seam: designos-doc unit tests using the synthetic slot fixture from E3-15 (`slot_model_tests.rs`
helpers): component "Card" with slots `Content` (preferred: "List item", min 1, max 5) and `Actions`
(strict, preferred: "Button").

| Case | Input | Expected |
|---|---|---|
| `fills_content_and_round_trips_native` | Content = [List item instance, text] | container children are the 2 new roots, `is_slot_content` true, `SlotContent` override; `to_json`→`from_json` equal |
| `non_recommended_fill_warns` | Content = [rectangle] | success; issues contains `slot-non-recommended` naming `List item` |
| `strict_slot_refuses` | Actions = [rectangle] | `SLOT_FILL_NOT_ALLOWED`, doc hash unchanged |
| `disable_hides_without_detach` | Actions = false | container `visible == false`, instance kind still `Instance`, root link intact |
| `child_count_bounds` | Content = 6 items | `SLOT_CHILD_COUNT` |
| `nested_slot_via_override_path` | overrides path `Body`, slots Content | fill applied in nested instance |

Fail-before: `fills_content_and_round_trips_native` fails today (`slots` unknown to NodeSpec, silently
ignored).

## 11. Performance budgets

Per slot: O(children of the slot container + fill size). Budget: a 5-slot, 100-node fill applies in
under 10 ms release. No document-wide walk.

## 12. Dependencies and build order

After E15-1 (shares `spec_instance.rs`), #150 (recommended components) and E15-9 (the PR edits E15-9's
`agent` preset, so the preset and its test exist first; both are wave 2, E15-9 merges first). Can be built in parallel
with E15-3 if E15-3 does not touch `spec_instance.rs`. The PR that registers `slot-non-recommended`
also appends it to E15-9's built-in `agent` preset and extends `presets_name_only_registered_rules`.

## 13. Owner rulings

- Full scope, never MVP: strict slots, bounds and disable ship together.
- #150 R&D extension (2026-10-18): filling a slot from an instance is a descendant override that
  replaces `children` and must round-trip native and JSON scene format.
- Parallel agents hard-locked to frames (RD-3): enforced by RD-3 at the parent level.
