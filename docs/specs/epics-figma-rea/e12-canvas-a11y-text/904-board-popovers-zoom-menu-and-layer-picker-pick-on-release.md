# #904 Board toolbar popovers, Zoom menu and Cmd+right-click layer picker pick on release

| Field | Value |
|---|---|
| Issue | #904 (epic #124; bug, P2; backlog; moved v0.6.2 → v0.6.3); includes the PR #905 review NIT 6 comment (layer picker) |
| Slice | S1, wave 1, leg C |
| Size | 4 builder hours |
| Tier | Argon (port to an existing tested helper; sim tests) |
| Depends on | none. **Blocks** #944 (`widgets/menu.rs`) and #869 (`panels/menus.rs`) |
| Touches | `crates/designos-app/src/board/toolbar.rs` (`popup` `:253`, cell picks `:295`, `:315`), `crates/designos-app/src/panels/menus.rs` (`show_zoom_menu` `:767`, row pick `:834` only), `crates/designos-app/src/panels/context.rs` (layer picker rows `:272`), `crates/designos-app/src/widgets/menu.rs` (make `primary_gesture` `:151` crate-visible, or wrap it; no behaviour change), new `crates/designos-app/src/shell/tests/release_pick_menus.rs` |

**Code baseline:** `origin/main@d22dad970`. Re-anchor against HEAD first.

## User-visible outcome

In these menus you can press, slide to a row or cell, and let go to pick it, however far or slowly the pointer travelled. This already works in the main menu, the toolbar tool dropdown and every `popup_tree` menu:
- the brush, shape and stamp grids in the board toolbar;
- the Zoom / View menu;
- the Cmd+right-click "select layer" picker.

## Current state (FACT)

1. Every menu built on `popup_tree` (`widgets/menu.rs:322`) picks on the release of a press, through `primary_gesture` (`:151`). Its tests are at `:965-1100`. Examples:
   - `a_release_over_a_row_picks_it_whatever_happened_since_the_press`;
   - `a_release_whose_press_the_menu_never_saw_picks_nothing`;
   - `a_release_over_a_row_covered_by_a_higher_layer_picks_nothing`.
2. Three menus still pick on egui's `clicked()`, which only fires for a release within 6 px and 0.8 s on the widget that took the press:
   - board popover cells: `board/toolbar.rs:295` (brush and shape kinds) and `:315` (stamps);
   - Zoom menu rows: `menus.rs:834` (`row_resp.clicked()`, inside `show_zoom_menu` `:767`);
   - layer picker rows: `context.rs:272` (`resp.clicked()`; the menu is opened by Cmd+right-click).
3. `primary_gesture` is a private `fn` (`menu.rs:151`).

## Scope

1. Give the three menus the same release rule. Reuse `primary_gesture`, or a thin crate-visible wrapper, with the same `covered` rule: a release over a higher layer picks nothing. Do not copy its logic.
2. A press the menu never saw (one that started outside, or before it opened) does not pick on its release. The same applies to the press that opened the menu if it is released over the opener.
3. The press that opens the board popover or the Zoom menu may slide onto a cell or row and pick it on release, the same way the toolbar tool dropdown already works (`toolbar.rs:273-278`, opens on press).
4. Keep keyboard activation (Enter or Space on a focused cell or row, and a screen reader's activation) working through `clicked()` when it is not a primary-button click. This is the same split `toolbar.rs:276` uses.

## Acceptance criteria

- **AC-1** For each of the 3 menus: a press on a row, a 40 px drag, a 1.2 s hold and a release on another row picks the row under the release. Red on the baseline.
- **AC-2** For each: open with a press, slide to a row and release in the same gesture: picks it.
- **AC-3** A release over a toast or tooltip layer above the menu picks nothing (the `covered` rule).
- **AC-4** Escape and a press outside still close each menu without a pick.
- **AC-5** Enter on a keyboard-focused row or cell still picks it (no pointer).
- **AC-6** The existing `widgets/menu.rs` tests and `shell/tests/board.rs` (for example `toolbar_buttons_pick_tools_and_shortcuts_follow_the_board_keys`, `:78`) stay green.

## Test steps

1. `shell/tests/release_pick_menus.rs`: one test per menu per AC-1, AC-2 and AC-4. Use `Sim` raw pointer events, with frames in between for the hold.
2. Run red on the baseline, then green after the fix.
3. Run `cargo test -p designos-app --lib menu board context`.
4. Visual: a 1440 capture of the board brush popover open over the canvas (no visual change expected; it shows the menu under test).

## Edge cases

- The board popover closes after a pick (current behaviour) and does not re-open from the same release.
- The Zoom menu's checked rows (Pixel grid, `menus.rs:757`) toggle once per release.
- Layer picker: the release lands on the row of a layer that an agent has deleted in the meantime. Pick nothing and close.

## Non-goals

- Moving these menus onto `popup_tree`.
- Any visual change.
- The dropdown fields inside popovers. That is #944, item 2.

## Parity facts

macOS menus (and Figma's) pick on release after press-and-slide. Same family as the fixes for #842, #843, #852 and #855 (issue text).

## Decisions recorded

- Widen the visibility of `primary_gesture` to `pub(crate)` rather than re-implement it. That changes no behaviour of `popup_tree`.

## Rules

- Open one PR that references #904. Use "Fixes #904" only when every acceptance criterion is met. Do not merge, tag or close.
- Re-anchor every cited line against HEAD before writing code, and report drift first.
- Work on a branch or worktree from `main`, never on `main`.
- Merge with squash only, after `gh pr update-branch <n> --rebase`.
- Blocking CI is `Test (macos-14)`, `Clippy` and `Rustfmt`. Ubuntu and Windows CI are advisory (CLAUDE.md, Git Workflow).
- Test first: each new test is red on the base commit, and that red output goes in the PR body.
- Visual evidence goes in the PR: an app capture, or a terminal render for test-only proof.
- Keep plan ids, issue numbers and finding codes out of code comments and test names. Use conventional commits with no AI trailers.
