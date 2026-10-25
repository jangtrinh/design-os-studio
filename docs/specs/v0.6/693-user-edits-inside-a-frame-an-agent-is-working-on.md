# #693 User edits inside a frame an agent is working on: show who is working there and offer Make a copy or Edit anyway

| Field | Value |
|---|---|
| Issue | #693 (parent #585; owner ruling 2026-10-19: a banner with two choices) |
| Size | M |
| Depends on | #592 (in-progress flag), #572 (frame ownership per run), #677 (turn ledger and summary card); soft: #682 (agent colour) |
| Unblocks | none |
| Touches | new `crates/designos-app/src/agent_busy_frames.rs` (busy lookup, dismissals, free-spot placement, both-changed record), new `crates/designos-app/src/canvas/agent_banner.rs` (banner painting), `crates/designos-app/src/ops.rs` (a `duplicate_into_free_spot` beside `duplicate_selection` `:518-552`), `crates/designos-app/src/live_patch_apply.rs` (`:17-68`), `crates/designos-app/src/agent_turn_card.rs` (one summary line, from #677), `crates/designos-app/src/canvas/mod.rs` (one call), tests |

Code baseline: `origin/main@9f3f7a5`.

## 1. Summary and user story

As a designer, when I click into a frame an agent is working on, I want to be told who is working there, and to choose: edit a copy while the agent finishes the original, or edit the original anyway. Nothing I or the agent do may be dropped silently.

## 2. Current state (FACT)

- While the user is in the middle of an edit, an agent write waits: `user_edit_in_progress` is true for a canvas drag, a text session, a vector point drag, a property scrub, an open colour picker or an open builder (`live_patch_apply.rs:17-25`); `apply_pending_live_patches` returns early while it holds (`:50-52`). The only sign is the status line "Agent edit waits until you finish the current edit" (`shell.rs:1317-1320`; the issue cites `:1313`).
- There is no notion of a busy frame on `main`: no in-progress flag, no agent metadata on a node (a search of `designos-doc`, `designos-mcp` and `designos-app` for `in_progress`, `AgentMeta`, `agent_runs`, `SetInProgress` finds only `edit_in_progress`). #592 and #572 are open.
- Patches carry no run id (`LivePatch { v, base, ops }`, `designos-doc/src/live_patch/mod.rs:27-35`).
- `App::duplicate_selection` copies the selection through `clipboard::capture` and `instantiate`, offsets the copy by `PASTE_OFFSET` unless the parent is auto-layout, inserts it right after the original and selects it (`ops.rs:518-552`). No function finds a free spot to the right; a search of `crates/designos-app/src` for `free_spot`, `nearest_free`, `find_free`, `free_slot`, `next_free` finds nothing (UNKNOWN whether an equivalent exists under another name).
- The canvas draws editor overlays through `draw::overlays` (`canvas/mod.rs:387`); floating pills exist (`tidy_up_pill_rect`, `canvas/draw.rs:671`).
- Agent edits to a field are visible as `LiveOp::Update { id, set, unset }` with the field names (`mod.rs:143-149`); a user edit to a node is a `Command::Update { node }` whose before and after are diffable.
- #677's ledger (not on `main`) records the layers a turn touched and a fingerprint of each.
- UNKNOWN: the exact API of #592 and #572 for "this frame is busy, owned by run X". Check: read the merged code of both before starting; this spec names the lookup `busy_info(doc, node) -> Option<BusyInfo { agent_label, run }>` and the implementer maps it.

## 3. Behaviour rules

1. **Busy frame.** A frame is busy when it has the in-progress flag (#592) or is owned by an agent run that has not ended (#572). `busy_info` walks the selected node's ancestors (the node itself and up) and returns the nearest busy frame.
2. **Trigger.** When the user selects a node, or starts an edit on a node (a drag, a text session, a property scrub), and `busy_info` finds a busy frame, a banner shows at the top of the canvas: "<agent name> is working here". The banner shows once per busy frame until dismissed or the run ends.
3. **Make a copy.** Duplicates the busy frame into the same parent, selects the copy, and leaves the original to the agent. The copy is a plain frame: it carries no in-progress flag and no agent ownership, so the agent's later writes land only on the original ids.
   - Placement: the nearest free spot to the right of the original in the parent's space: start at the original's right edge plus a gap of 40 px (initial value), and while the rectangle of the copy overlaps any sibling's bounds, move to that sibling's right edge plus the gap. When the parent is auto-layout, the copy goes in as the next sibling in flow and no offset is applied.
   - One undo step (one `Tx`), the same way `duplicate_selection` commits.
4. **Edit anyway.** Hides the banner for that frame until the run ends. Edits then follow today's rule: an agent write waits while the user is mid-gesture, then applies.
5. **No silent drops.** If an agent write and a user edit touch the same field of the same node, the later one wins. The per-turn summary (#677) lists that field as "changed by you and the agent". Field names come from the agent's `Update.set` keys and the diff of the user's `Command::Update`.
6. **Gone when over.** The banner disappears when the run ends or is stopped (#684), when the busy frame is deleted, or when the user selects nothing inside it.
7. No banner for frames no agent works on; no banner for the agent's own frames when nobody selects inside them.
8. Several agents (#572): the banner names the agent that owns the nearest busy frame; two busy frames nested: the nearest wins.
9. The banner never moves the view and never changes the selection by itself. Only "Make a copy" selects (the copy).

## 4. Data model / schema changes

- In memory: `dismissed: BTreeSet<(NodeId, RunKey)>`, `both_changed: BTreeMap<(NodeId, String), TurnNo>`. No file or log change.
- `RunKey` is #572's run id, or #677's turn number before #572.

## 5. Contracts

- `busy_info` (rule 1) is the only place that reads #592 and #572 state for this task.
- #677's summary type gains `both_changed: Vec<(NodeId, String)>`; its card shows one caption line when it is not empty.

## 6. File-format mapping

None. The copy is an ordinary frame in `.designos` and `.fig` export.

## 7. UI

Described in words; checked against `design/principles.md` P1 to P10 and `design/soul.md`.

- The banner is a floating pill at the top centre of the canvas, `RADIUS_FRAME`, fill `BG_SECONDARY`, `BORDER` hairline, inset `INSET_FRAME`, `SPACE_INLINE` (8 px) between parts, inside the canvas rect at `INSET_PANEL` below its top edge and clear of the ruler strip.
- Left: an 8 px dot in the agent's colour (#682's token; `TEXT_SECONDARY` before #682). Then the sentence "<agent name> is working here" in body type, `TEXT`, elided to the pill width.
- Right: two buttons of `CONTROL_STANDARD` (28 px), `RADIUS_CONTROL`. "Make a copy" is the primary action: `BG_BRAND` with a dark label (it is the safe choice and the canvas view's one primary action). "Edit anyway" is neutral: `BG_TERTIARY` fill, `TEXT` label. Each is a pointer target of at least `TARGET_MIN`.
- No animation, no timer; the banner repaints on events only. It never overlaps the selection handles' hit area: it sits above the canvas content area and the user can still click through the rest of the canvas.
- Desktop capture at 1440: banner on a busy frame, the copy in free space beside it, the summary line "changed by you and the agent". Responsive: SKIPPED (owner default). Load es-designer before UI code; update the Figma frames from the captures first.

## 8. Edge cases

1. Selecting the busy frame itself: banner shows; the copy is of that frame.
2. Selecting a child two levels deep: the busy ancestor is found; the copy is of the busy frame, not of the child.
3. The user starts a drag in a busy frame without having seen the banner (a click and drag in one gesture): the banner shows at once and the drag continues; the agent write waits as today.
4. The agent finishes while the banner is open: it disappears; no copy is made.
5. "Make a copy" in a locked or slot-protected parent: the duplicate is refused with the app's normal message, as `duplicate_selection` refuses (`slot_insert_allowed`, `ops.rs:529-533`).
6. The sibling bounds overlap in a way that never frees (a crowded row): the loop ends after the number of siblings and places the copy past the right-most sibling.
7. Two agents on one frame: the banner names the run that owns it; the other is listed in the receipt.
8. Undo of "Make a copy": removes the copy and restores the previous selection.
9. A top-level frame on a page and a nested frame behave the same; only auto-layout parents change placement.

## 9. Non-goals

- Locking frames against the user. Merging a copy back. A third choice ("wait"). Showing the banner for comments or variable edits.

## 10. Test plan (red first)

Seams: pure tests for the free-spot function and `busy_info`; the `Sim` harness with a fake busy flag and a stub patch.

| Case | Expected | Fails before |
|---|---|---|
| `selecting_a_node_in_a_busy_frame_shows_the_banner_with_the_agent_name` | banner text "Claude is working here" | yes |
| `make_a_copy_creates_one_duplicate_in_free_space_selected` | one new frame, no overlap with siblings, selected, one undo step | yes |
| `later_agent_writes_land_only_on_the_original` | the stub patch changes the original; the copy's hash is unchanged | yes |
| `the_copy_carries_no_busy_flag_or_owner` | flag and owner absent | yes |
| `edit_anyway_hides_the_banner_until_the_run_ends` | hidden; shown again for the next run | yes |
| `no_banner_for_frames_no_agent_works_on` | none | yes |
| `banner_clears_when_the_run_ends_or_stops` | none | yes |
| `free_spot_skips_overlapping_siblings` | table: empty row, one blocker, crowded row | yes |
| `auto_layout_parent_inserts_the_copy_as_the_next_sibling` | index and no offset | yes |
| `same_field_edits_are_listed_as_changed_by_both` | summary lists the field | yes |
| `banner_never_moves_the_view_or_the_selection` | `view` and selection equal | guard |

Run: `cargo test -p designos-app agent_busy_frames shell`. Evidence: red/green capture, the 1440 captures.

## 11. Performance budgets

- `busy_info`: one ancestor walk per selection or edit-start event, O(depth); nothing per frame beyond reading a cached `Option`.
- Free spot: O(siblings) with a cap; under 1 ms for 1,000 siblings.
- No `to_json`, no timer, no `app.changed()` from the banner itself (the duplicate is a normal edit and does call it).

## 12. Dependencies and build order

After #592, #572 and #677. Land with or after #682 so the dot has its colour. `ops.rs` is shared with nothing else in v0.6 known to this index. `agent_turn_card.rs` is #677's file: add the one line in a small follow-up commit after it merges. Merge order is in README-part4.

## 13. Owner rulings

- Show a banner with two choices (2026-10-19): "Make a copy" and "Edit anyway". Concurrent edits are never silently dropped; the later write wins and the summary lists the field. The banner goes when the run ends or stops. (Issue text.)

## Open questions for the owner (recommended first)

1. The gap between a copy and its original. Recommended: 40 px, one named constant; or the spacing the page's other frames use.
2. Which button is the primary one. Recommended: "Make a copy" in orange, because it is the choice that cannot lose work.
