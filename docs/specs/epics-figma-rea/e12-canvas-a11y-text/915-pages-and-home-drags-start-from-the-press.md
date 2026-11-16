# #915 Page rows (and Home project and folder drags) keep a quick drag

| Field | Value |
|---|---|
| Issue | #915 (epic #124; bug, P3; backlog; moved v0.6.2 → v0.6.3) |
| Slice | S1, wave 1, leg B |
| Size | 3 builder hours (1 h if the red tests stay green) |
| Tier | Argon (test-first, mirrors the merged #855 / PR #908 pattern) |
| Depends on | none |
| Touches | `crates/designos-app/src/panels/left.rs` (page rows, `r.drag_started()` `:315`, and the page drop code that follows), `crates/designos-app/src/home/main_view.rs` (`:313`), `crates/designos-app/src/home/mod.rs` (`:2406`, `:2679`), new `crates/designos-app/src/shell/tests/pages_quick_drag.rs` |

**Code baseline:** `origin/main@d22dad970`. Re-anchor against HEAD first.

## User-visible outcome

A quick flick of a page row in the Pages list reorders the page, the same way a slow drag does. On Home, a quick drag of a project or folder card moves it.

## Current state (FACT)

1. Page rows start their drag on egui's `drag_started` (`left.rs:315`: `if r.drag_started() && !separator_press { app.layers.page_drag = Some(*pid); }`).
2. The Layers panel had the same problem. It was fixed by #855 / PR #908 with press and release events: `track_row_drag`, `layers.rs:352-405`.
   - The module comment at `:355-360` gives the egui rule. A press starts a drag only on a frame after the press, and a release inside the frame drops the pending drag.
   - So a move and release in the frame after the press, or all three in one frame, give no drag.
3. Home uses `resp.drag_started()` at `home/main_view.rs:313`, `home/mod.rs:2406` and `home/mod.rs:2679`.
4. The issue says it is **not reproduced**. The first step is a red test.

## Scope

1. Write the single-frame tests first:
   - `Sim::drag_in_one_frame` (`shell/tests.rs:219`) over a page row, onto another page row;
   - the "move and release in the next frame" variant;
   - the same two for one Home project card and one Home folder.
2. For every site that goes red, start the drag from the press and release events, the same way `track_row_drag` does.
   - Factor a small shared helper only if two or more sites need it, for example `widgets::row_drag::track(…)`.
   - The Layers code stays as it is: no refactor of a merged fix.
3. For every site that stays green, record the passing test as evidence, and leave the code alone.

## Acceptance criteria

- **AC-1** A one-frame flick of page 3 onto page 1 reorders the pages: page 3 becomes first. Red on the baseline if the bug is real.
- **AC-2** Press in one frame, then move and release in the next: the pages reorder.
- **AC-3** The same two cases for a Home project card and a Home folder move the item.
- **AC-4** A plain click on a page row still only switches the page, and a double-click still renames it (`left.rs:319-322`). Pressing a separator row does not start a drag (`separator_press`).
- **AC-5** The PR body says, for each of the 4 sites, whether it was red (with the test output) or green.

## Test steps

1. Add the tests to `shell/tests/pages_quick_drag.rs`. Run them on the baseline and paste the red or green output in the PR.
2. Fix the red sites and run `cargo test -p designos-app --lib pages_quick_drag home`.
3. Capture the Pages list after a scripted flick at 1440 (CLI screenshot) as visual evidence.

## Edge cases

- Dragging while a page is being renamed: no drag starts, as for layers (`start_row_drag` refuses while renaming, `layers.rs:414`).
- A drag that leaves the list and the release lands outside: no reorder.
- A Project tab (no pages): unchanged.

## Non-goals

- Changing drop zones or the drop-line look of the Pages list.

## Parity facts

Figma reorders pages by drag in the Pages list. Speed is not a factor in Figma (no frame-batching).

## Decisions recorded

- Test first. Code changes only at sites proven red.

## Rules

- Open one PR that references #915. Use "Fixes #915" only when every acceptance criterion is met. Do not merge, tag or close.
- Re-anchor every cited line against HEAD before writing code, and report drift first.
- Work on a branch or worktree from `main`, never on `main`.
- Merge with squash only, after `gh pr update-branch <n> --rebase`.
- Blocking CI is `Test (macos-14)`, `Clippy` and `Rustfmt`. Ubuntu and Windows CI are advisory (CLAUDE.md, Git Workflow).
- Test first: each new test is red on the base commit, and that red output goes in the PR body.
- Visual evidence goes in the PR: an app capture, or a terminal render for test-only proof.
- Keep plan ids, issue numbers and finding codes out of code comments and test names. Use conventional commits with no AI trailers.
