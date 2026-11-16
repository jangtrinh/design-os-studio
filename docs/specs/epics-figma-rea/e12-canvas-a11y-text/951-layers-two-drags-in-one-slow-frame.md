# #951 Two row drags in one slow frame: the first drops where it was released

| Field | Value |
|---|---|
| Issue | #951 (epic #124; backlog "AI design 2 roadmap & overflow"; moved v0.6.2 → v0.6.3 on 10-25) |
| Slice | S1 Input you can trust, wave 1, leg A (same builder then does #969) |
| Size | 1 builder hour |
| Tier | Grok build-fast (one guard plus one sim test; a Claude reviewer reviews it, never Grok) |
| Depends on | none |
| Touches | `crates/designos-app/src/panels/layers.rs` (`track_row_drag` `:360-405`) and a new test in `crates/designos-app/src/shell/tests/layers_drop_refusal.rs`. #969 later adds to the same test file. |

**Code baseline:** `origin/main@d22dad970`. Re-anchor every line against HEAD before writing code, and report any drift first.

## User-visible outcome

You drag layer A and let go, then in the same moment flick layer B. That happens when Studio is busy and one frame takes long. Today A lands where B was released. After the fix, A lands where you released A, and B is handled by the next frame's logic.

## Current state (FACT)

1. `track_row_drag` walks this frame's primary button events in order (`layers.rs:360-405`). On every release it computes:
   - `under_way = app.layers.drag.is_some()`;
   - `began` from the stored press.

   When either is true, it sets `frame.at = Some(pos)` and `frame.ended = true` (`:385-394`).
2. `app.layers.drag` stays `Some` until the drop runs after the rows are drawn (`:764-775`).
   - So when release A, press B and release B share a frame, B's release still sees `under_way == true`.
   - It overwrites `frame.at` with B's release point, and the drop at `:764` uses B's point.
3. The reviewer measured this in probe p13 (`plans/reports/release-060-evidence/review-908.md`, "Re-review (fix round 2)"): A landed at index 29 instead of 34.

## Scope

- In the release branch of `track_row_drag`, ignore any release after the first one that ended a drag in this frame. The guard the reviewer suggested is `if (under_way || began) && !frame.ended`.
- A press after that release is still stored as `app.layers.press`, so B can start a drag in a later frame.
- B's own drop is not replayed in this frame. One drop per frame is the rule.

## Acceptance criteria

- **AC-1** One frame holds A's drag move, A's release, B's press, a move beyond the click reach and B's release. Layer A ends at the index under A's release point, not B's. Red on the baseline, green after.
- **AC-2** The existing quick-drag tests (`drag_in_one_frame` users in `shell/tests/fast_drag_*.rs` and the Layers fast-drag tests from #908) stay green.
- **AC-3** After AC-1's frame, B is not dropped and no drag is under way: `app.layers.drag.is_none()` after the next idle frame.

## Test steps

1. Port probe p13 into `shell/tests/layers_drop_refusal.rs` as `two_drags_in_one_frame_drop_the_first_at_its_own_release`. Use `Sim` with raw `PointerButton` and `PointerMoved` events in one `step`, the same way `Sim::drag_in_one_frame` builds events (`shell/tests.rs:219`).
2. Run it on the baseline and record the red output (index 29 vs 34, or equivalent).
3. Apply the guard and run it again: green.
4. Run `cargo test -p designos-app --lib layers` and `cargo test -p designos-app --lib fast_drag`.

## Edge cases

- Three releases in one frame: only the first one that ended a drag counts.
- A plain click (press and release within reach) before a real drag in the same frame. The click ends nothing, so the later drag release still counts. The existing comment at `:346-348` describes this case, so the guard must not change it.
- The list is hidden for a pass mid-gesture (`:372-378`): unchanged.

## Non-goals

- Replaying B as a second drop.
- Any change to canvas drags.

## Parity facts

Figma desktop drops every completed layer drag where its pointer was released. Studio drops one per frame; that is acceptable because a human cannot do two drags in under ~30 ms without a stall. (INFERENCE, not measured in Figma.)

## Decisions recorded (invisible, technical)

- One drop per frame. The later drag in the same frame is dropped silently and not replayed. A replay would need a queue and could move a layer the user never saw move.

## Rules

- Open one PR that references #951. Use "Fixes #951" only when every acceptance criterion is met. Do not merge, tag or close.
- Re-anchor every cited line against HEAD before writing code, and report drift first.
- Work on a branch or worktree from `main`, never on `main`.
- Merge with squash only, after `gh pr update-branch <n> --rebase`.
- Blocking CI is `Test (macos-14)`, `Clippy` and `Rustfmt`. Ubuntu and Windows CI are advisory (CLAUDE.md, Git Workflow).
- Test first: each new test is red on the base commit, and that red output goes in the PR body.
- Visual evidence goes in the PR: an app capture, or a terminal render for test-only proof.
- Keep plan ids, issue numbers and finding codes out of code comments and test names. Use conventional commits with no AI trailers.
- Built by Grok build-fast: a Claude reviewer reviews the PR. Grok never reviews its own change.
