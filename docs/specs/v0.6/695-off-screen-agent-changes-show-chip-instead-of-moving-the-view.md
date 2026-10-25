# #695 When an agent turn ends with changes off-screen, show an "N changes → Show" chip instead of moving the view

| Field | Value |
|---|---|
| Issue | #695 (parent #585; owner ruling 2026-10-19: never move the user's view automatically) |
| Size | S |
| Depends on | #677 (turn end and changed layers), #682 (agent colour and the touched-ids list) |
| Unblocks | none |
| Touches | new `crates/designos-app/src/canvas/agent_changes_chip.rs` (chip state, placement, painting), `crates/designos-app/src/app.rs` (`View::fit_with_margin`, beside `View::fit` `:44-50`), `crates/designos-app/src/live_patch_apply.rs` (feed the turn's changed ids), `crates/designos-app/src/canvas/mod.rs` (one call after `draw::overlays`, `:387`), tests |

Code baseline: `origin/main@9f3f7a5`.

## 1. Summary and user story

As a designer, when an agent finishes a turn that changed things I cannot see, I want a small chip at the edge of the canvas that says how many changes are off-screen and takes me there when I click it. My view must never move on its own.

## 2. Current state (FACT)

- After agent edits, the selection (not the view) jumps to the inserted roots: `app.selection = total.inserted_roots.clone()` (`live_patch_apply.rs:95-96`). The view is untouched by patches.
- One exception moves the view: `replace_live_document`, used when the agent switches to another document, calls `app.fit_all()` (`live_patch_apply.rs:42`).
- Changes elsewhere on the canvas are invisible: replay reports only inserted roots (`designos-doc/src/live_patch/replay.rs:61-73`). #682's spec adds a `touched` list of changed ids; #677 adds turn marks and a per-turn ledger. Neither is on `main`.
- Fit helpers: `View::fit(bounds, size)` pads by the constant `FIT_PADDING` 64 px (`app.rs:44-50`, `theme.rs:511`); `App::fit_selection` and `fit_all` set `self.view` directly (`ops.rs:1742-1752`). Setting `app.view` is viewport-only: it does not call `app.changed()` or bump `TileView::rev` (CLAUDE.md section 4).
- World bounds of a node come from `canvas::fast_world_aabb(doc, index, id)` (`ops.rs:127`). The canvas rect is `app.canvas_rect`.
- Floating canvas overlays are painted in `canvas::draw::overlays` (`canvas/mod.rs:387`).
- `design/soul.md` forbids counts concatenated into labels: "counts render in a separate `Badge/StatusCount` pill (`D85`)". The issue writes the chip text as `N changes → Show`.

## 3. Behaviour rules

1. **When.** When a turn ends (#677's end mark, or the heuristic end for Codex) and at least one layer the turn inserted, updated or moved lies outside the viewport, a chip appears. Removed layers are not counted (nothing to show). A turn whose changes are all visible shows no chip.
2. **Off-screen** means the layer's world bounds do not intersect the canvas rect in page space. A partly visible layer counts as visible.
3. **Count N** is the number of distinct layers of that turn that are off-screen at the moment the chip is computed. It is recomputed when the user pans or zooms (rule 6), never per frame.
4. **Place.** At the canvas edge nearest to the off-screen layers: take the centre of their union and clamp the chip to the canvas rect at the edge in that direction, inset `INSET_PANEL`, centred along the edge as far as it fits. One chip per turn.
5. **Click.** Pans and zooms to fit all layers the turn changed (the union of their bounds) with a 10% margin on each side. It selects nothing and does not change the selection. The click uses a new `View::fit_with_margin(bounds, size, 0.10)`; the zoom stays within `ZOOM_MIN` to `ZOOM_MAX`; a zero-size union is treated as 1 x 1.
6. **Goes away** when: the user clicks it; the next turn starts; the user pans or zooms so that none of its layers is off-screen any more; the document is switched; the tab closes.
7. **Never moves the view.** Nothing in this task or in patch replay changes `app.view` for a normal patch. The document-switch fit (`:42`) is the one existing exception and stays (open question 2).
8. **Viewport-only.** Showing, hiding and clicking the chip call no `app.changed()` and do not bump `TileView::rev`.
9. The chip has the agent's colour (#682).
10. Several agents (#572): one chip per finished turn, each in its agent's colour; chips at the same edge stack with `SPACE_INLINE` between them, newest nearest the corner.

## 4. Data model / schema changes

In memory: `Chip { agent, layers: Vec<NodeId>, turn }`. No file or log change.

## 5. Contracts

- `View::fit_with_margin(bounds: Rect, size: Vec2, margin_fraction: f32) -> View` (pure, tested).
- Input from #677 and #682: the ordered list of `(NodeId, TouchKind)` per turn and the turn end event.

## 6. File-format mapping

None.

## 7. UI

Described in words; checked against `design/principles.md` P1 to P10 and `design/soul.md`.

- The chip is a pill of `CONTROL_STANDARD` (28 px), `radius_full`, filled with the agent's colour with a dark label (soul: a dark label on an accent fill). It holds, left to right: a small count pill (`BG_TERTIARY` fill, the number in `font_mono`, so the count is not inside a label), the word "changes", and "Show" with an arrow icon. The arrow icon points toward the changes. `SPACE_FACT` (4 px) separates the parts of one fact; the pill is a single pointer target of at least `TARGET_MIN`.
- Hover steps the fill to the agent colour's hover tone. Focus uses the brand-orange ring.
- No animation; no timer; the chip repaints on events only.
- Desktop capture at 1440: a chip on the right edge with three changes, the view after the click showing all of them, no chip for an on-screen turn. Responsive: SKIPPED (owner default). Load es-designer before UI code; update the Figma frames from the captures first.

## 8. Edge cases

1. A single changed layer off-screen: "1 change" (singular word).
2. N above 999: the pill shows "999+".
3. The user zooms out so the layers become visible: the chip clears at once.
4. The turn changed only comments or variables: no layers, no chip.
5. The agent deletes the changed layer before the click: the layer drops from the list; if none is left, the chip clears.
6. Changed layers on two opposite sides: the chip goes to the edge nearest the union centre; the click fits all.
7. The document switches while a chip shows: cleared.
8. Huge turn (5,000 layers): bounds from `fast_world_aabb`, capped at 10,000 layers for the union; the pill says "9,999+".
9. The window is smaller than the chip: it elides "changes" first.
10. Codex turns end by heuristic and can end twice: a second end for the same turn replaces the chip's layers, it does not add a second chip.

## 9. Non-goals

- Moving the view during a turn. The cursor and flash (#682). Following the agent. Selecting the changed layers. A list of changes (#677's card).

## 10. Test plan (red first)

Seams: pure tests for `View::fit_with_margin`, the off-screen test and the placement; the `Sim` harness with a stub turn.

| Case | Expected | Fails before |
|---|---|---|
| `a_turn_that_edits_an_off_screen_frame_shows_the_chip_with_the_count` | chip with N equal to the off-screen layers | yes |
| `click_brings_the_frame_into_view_with_a_ten_percent_margin` | the union plus 10% fits the canvas rect; the frame is inside | yes |
| `the_view_never_moves_without_a_click` | `app.view` equal on every frame of the turn | guard (patches do not move it today) |
| `an_on_screen_only_turn_shows_no_chip` | none | yes |
| `click_changes_no_selection` | selection equal | yes |
| `chip_clears_on_next_turn_start_pan_to_visible_and_document_switch` | none after each | yes |
| `chip_never_bumps_the_tile_revision_or_calls_changed` | `tiles.rev()` equal | yes |
| `placement_uses_the_edge_nearest_the_changes` | table of four directions and a corner | yes |
| `fit_with_margin_clamps_zoom_and_handles_a_point` | within bounds; no NaN | yes |
| `two_agents_two_chips_stack` | two chips, two colours, no overlap | yes |

Run: `cargo test -p designos-app agent_changes_chip app`. Evidence: red/green capture, the 1440 captures.

## 11. Performance budgets

- On turn end: one `fast_world_aabb` per touched layer (capped at 10,000), under 2 ms for 10,000 on the large fixture.
- On view change: re-check only the chip's own layer list, under 0.5 ms. Nothing per frame while the view is still.
- No `app.changed()`, no `rev` bump, no timer. Idle CPU unchanged at 0.00%.

## 12. Dependencies and build order

After #677 and #682 (the touched-ids list is shared, see #682). It touches `app.rs`, `canvas/mod.rs` and `live_patch_apply.rs` at call sites only. Merge order is in README-part4: last of the canvas pieces.

## 13. Owner rulings

- Never move the user's view automatically; offer a chip. The chip says how many changes and takes the user there on click; it goes away on click, on the next turn, or when the changes are visible; no chip when everything is visible; viewport-only. (Issue text, 2026-10-19.)

## Open questions for the owner (recommended first)

1. The count inside the chip. Recommended: a separate count pill inside the chip, because the design soul bans counts in labels; the words stay "changes" and "Show".
2. The document-switch fit (`live_patch_apply.rs:42`). Recommended: keep it, since a newly opened document has no view to protect; the owner may prefer a chip there too.
