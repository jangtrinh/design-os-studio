# #644 Apply slot stretch on canvas drag, library drop and media placement into a slot

| Field | Value |
|---|---|
| Issue | #644 (follow-up of the #508 review) |
| Size / priority | M / P2 |
| Depends on | none (benefits from #582 only in sharing the "entered a slot" helper idea) |
| Unblocks | none |
| Touches | `crates/designos-app/src/canvas/interact.rs` (`reparent_after_move`, `place_image_bytes`, `place_video_bytes`), `crates/designos-app/src/libraries/mod.rs` (`drop_library_asset`), `crates/designos-app/src/clipboard.rs` (`place_svg`), `crates/designos-app/src/ops.rs` (shared helper next to `mark_slot_content_override`), tests in `crates/designos-app/src/shell/tests/` |

Code baseline: `origin/main@01d48f2` (0.5.0). The issue cites `interact.rs:1653` and
`libraries/mod.rs:621`; they are now `interact.rs:1629` and `:1653` (two `Command::Reparent` sites in
`reparent_after_move`, `:1617`) and `libraries/mod.rs:590-640` (`drop_library_asset`).

## 1. Summary and user story

As a designer who configured a slot to "stretch child on insert", every way of putting a layer into
that slot gives it the slot's width and Fill sizing, not only the `+` picker, paste and duplicate: I can
drag a layer in, drop a library component in, or place an image, a video or an SVG into the slot and get
the same result.

## 2. Current state (FACT)

- The rule is `apply_slot_stretch_on_insert(doc, slot, &mut Node)`
  (`designos-doc/src/components.rs:1442-1458`): when the slot's `SlotPropConfig` has
  `stretch_child_on_insert == Some(true)`, the child gets `layout_child.horizontal = Sizing::Fill`,
  `size.x = slot width - left/right padding` (minimum 1), `transform.tx = left padding` when the slot has
  no auto layout, and, for an instance or linked child, the `Size` and `Layout` overrides.
- It runs for: the picker (`comp_ops.rs:369`), paste (`ops.rs:542`, `:606`, `:653`) and moving
  through `move_to_parent` (`ops.rs:1464`; the stretch call is at `:1491`, only when `old_p != Some(new_parent)`).
- It does not run for:
  - canvas drag: `reparent_after_move` reparents with `Command::Reparent` at
    `canvas/interact.rs:1629` (reorder drop inside an auto-layout frame) and `:1653` (drop into another
    container) and then sets the transform (`:1654-1656`); the slot override is marked afterwards
    (`App::mark_slot_content_override`, `:1630-1632`, `:1670`);
  - library drop: `App::drop_library_asset` (`libraries/mod.rs:590`) builds the command with
    `import_asset`, validates with `slot_insert_allowed`, applies it and marks the slot, never stretching;
  - media: `place_image_bytes` (`canvas/interact.rs:2056`), `place_video_bytes` (`:2010`) and
    `place_svg` (`clipboard.rs:280`) each build one node (or a subtree), call `slot_insert_allowed` /
    `slot_edit_error`, insert, and mark the slot.
- Each path already has the slot as `parent` in hand (`container_at_with_index` / `container_at`).
- Shapes and text created with a draw tool inside a slot (`text_press`, rectangle and ellipse tools in
  `canvas/interact.rs`) are not in the issue; they also do not stretch today.
- Test rig for slots: `rig(max)` in `crates/designos-app/src/shell/tests/empty_slot_flows.rs:32-56`
  (instance "Get started" with an "Extra" slot configured `stretch_child_on_insert: Some(true)`),
  `App::insert_instance_into_slot`, `App::insert_instance`.

## 3. Behaviour rules

1. One helper, `App::stretch_into_slot_if_configured(doc: &mut Document, ed: &mut Edit, slot: NodeId, child: NodeId)`,
   in `ops.rs` next to `mark_slot_content_override`. It clones the child, runs
   `apply_slot_stretch_on_insert(doc, slot, &mut clone)`, and writes the clone back through
   `ed.node(doc, child)` so the change joins the caller's undo step (the pattern of `move_to_parent`, `ops.rs:1488-1495`).
   It does nothing unless the slot's config says stretch.
2. Every path in section 2 calls the helper after the node is in the slot and, for drag, after the
   world-to-local transform was assigned (so `tx = left padding` for a non-auto-layout slot is not
   overwritten).
3. Drag: only a move that changes the parent counts as entering. A reorder inside the same slot (the
   `m.reorder` branch, `:1621-1632`, `old_parent == r.parent`) does not stretch; a drag from one slot into
   another, or from the page into a slot, does. Alt-copies entering a slot stretch like any new content.
4. Library drop, image, video, SVG: the new root stretches; a video or image keeps its aspect ratio
   only through the existing Fill/size rules of `apply_slot_stretch_on_insert` (width follows the slot,
   height keeps the stored value). No new aspect-ratio logic.
5. Everything stays one undo step per user action (the drag's `Tx`, the drop's `Tx`).
6. A slot without the stretch setting, and any non-slot container, behaves exactly as today.
7. **Drawn layers keep their drawn size (owner ruling 2026-10-19).** A rectangle, ellipse, text or any other layer created with a draw tool inside a slot is not stretched: the user chose its size by drawing it.
   The helper is not called from the draw-tool paths (`text_press` and the shape tools in `canvas/interact.rs`).

## 4. Data model / schema changes

None.

## 5. Contracts

No public API change. `apply_slot_stretch_on_insert` keeps its signature.

## 6. File-format mapping

None.

## 7. UI

No new UI. Result in words: a layer dropped into a stretch-enabled slot spans the slot's inner width
at once, the same as when it is chosen from the `+` picker; Properties shows width as Fill with the
Size and Layout overrides on instances. The slot setting toggle already exists
(`panels/components.rs:571-575`). Desktop capture at 1440 of each path in the PR; responsive SKIPPED
(owner default). Load es-designer before touching UI code (no visual change expected beyond layout).

## 8. Edge cases

1. Slot with auto layout and left/right padding 12: width = slot width - 24.
2. Slot without auto layout: `transform.tx` becomes the left padding after the drag's transform fix-up.
3. Dragging several selected layers into the slot: each gets the stretch (`moving` loop, `:1646-1660`).
4. Drag refused by a slot limit (`slot_move_allowed`): nothing changes (the helper is never reached).
5. Library drop refused by `slot_insert_allowed`: images already copied are removed as today
   (`libraries/mod.rs:612-618`); no stretch.
6. Image placed into a slot narrower than the image: width shrinks to the slot, height keeps the stored
   value (rule 4); the user can change it.
7. A child that is a Text node: Fill width applies as for the picker.
8. Undo after a drag into a slot restores parent, transform, width and sizing in one step.

## 9. Non-goals

- Shapes and text created by draw tools inside a slot: they keep their drawn size (rule 7).
- Changing the stretch rule itself or its setting.
- Stretching on reorder inside the same slot.

## 10. Test plan (red first)

Seam: app-level tests with the `rig(max)` fixture (extend it with a small library fixture for the drop
case, as `shell/tests/libraries.rs:510` does for `insert_into_slot`). Synthetic documents only.

| Case | Steps | Expected (fails before) |
|---|---|---|
| `drag_into_stretch_slot_fills_the_slot` | create a rectangle on the page, drag-release it over the slot through the interaction test harness | rectangle `layout_child.horizontal == Fill`, `size.x == slot inner width` |
| `drag_between_slots_stretches_again` | move content from slot A to slot B (different widths) | width equals slot B inner width |
| `reorder_inside_same_slot_does_not_restretch` | reorder two children | sizes unchanged |
| `library_drop_into_stretch_slot_fills_the_slot` | `drop_library_asset` at a point over the slot | new instance Fill and slot width; override flags set |
| `image_place_into_stretch_slot` | `place_image_bytes` over the slot (small PNG built in the test) | Fill, slot width |
| `video_place_into_stretch_slot` | `place_video_bytes` with the tiny synthetic video fixture used by `place_video_makes_a_poster_layer_with_a_video_sidecar` (`interact.rs:2423`) | Fill, slot width |
| `svg_place_into_stretch_slot` | `place_svg` | Fill, slot width |
| `drawn_layers_keep_their_size` | draw a rectangle and a text layer inside a stretch slot | size and sizing unchanged (no Fill, no slot width) |
| `no_stretch_when_setting_off` | same drags with `stretch_child_on_insert: None` | sizes untouched |
| `one_undo_step_per_action` | each case then `undo` | state before the action |

Run: `cargo test -p designos-app`. Evidence: red/green capture and one desktop screenshot per path
(drag, library drop, image), of the resulting document (save the rig document from the test, then `designos --screenshot` on the saved file).

## 11. Performance budgets

The helper runs once per entered root: O(1) plus one node clone. Drag release latency unchanged (no
per-frame work; the helper runs on release only).

## 12. Dependencies and build order

Independent. `canvas/interact.rs` is large and shared; keep the change to the two call sites and
the new helper, and rebase rather than hand-merge.

## 13. Owner rulings

- #508 ruling: a slot configured to stretch gives inserted content the slot width.
- Owner ruling 2026-10-19 (`plans/reports/release-060-evidence/owner-rulings-v06.md`): drawn layers keep their drawn size and are not stretched.
- Figma-first (2026-10-13): UI change rendered locally goes to Figma before gates; here the capture is
  the evidence, there is no new frame.

## Open questions for the owner

None. The one question of the first draft was answered by the owner on 2026-10-19: drawn layers keep their drawn size (rule 7).
