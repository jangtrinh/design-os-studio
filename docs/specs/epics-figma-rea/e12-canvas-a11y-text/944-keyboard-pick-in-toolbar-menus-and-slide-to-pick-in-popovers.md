# #944 Keyboard pick in a keyboard-opened toolbar menu, and slide-to-pick in popover dropdowns

| Field | Value |
|---|---|
| Issue | #944 (epic #124; backlog; moved v0.6.2 → v0.6.3); source `plans/reports/release-060-evidence/review-905.md` R1-1, R1-2 |
| Slice | S1, wave 2, leg F |
| Size | 5 builder hours |
| Tier | Claude Sonnet (keyboard-focus routing between the toolbar and an open menu; shared dropdown widget) |
| Depends on | #904 merged (both edit `widgets/menu.rs`). If this leg must edit `shell.rs`, rebase after #1019 merges |
| Touches | `crates/designos-app/src/panels/toolbar.rs` (chevron `:239-283`), `crates/designos-app/src/widgets/menu.rs` (`dropdown_items` `:624-685`; `popup_tree` key handling `:322-`), new `crates/designos-app/src/shell/tests/toolbar_menu_keyboard.rs` |

**Code baseline:** `origin/main@d22dad970`. Re-anchor against HEAD first.

## User-visible outcome

1. Tab to a toolbar chevron and press Space: the tool menu opens. Press ArrowDown to move the highlight, then Enter or Space to pick that tool. Escape closes the menu with no pick.
2. In a popover (for example Type settings), press on a dropdown field, slide onto an option and let go: that option is picked. This already works for fields in the right panel.

## Current state (FACT)

1. The chevron toggles the menu on a primary press in its hit rect, or on a non-pointer `clicked()` (keyboard Space or Enter, or a screen-reader activation) (`toolbar.rs:273-278`).
   - While the menu is open, the chevron keeps egui focus.
   - So Enter on it toggles the menu **closed** before the menu sees the key. ArrowDown already reaches the menu (issue text).
2. `popup_tree` handles Escape (`menu.rs:383-389`) and arrow keys for its highlight. Builder: confirm where Enter picks in `popup_tree` and whether it consumes the key.
3. Dropdown fields (`dropdown`, `dropdown_checked` and `dropdown_items`, `menu.rs:597-685`) open on `r.clicked()` (`:664`). That is a release, so the menu is not open during the press.
   - The menu (`popup_menu`) uses `primary_gesture`, whose `covered` rule ignores a press over another Foreground layer (`menu.rs:394-400`).
   - A popover is a Foreground area. INFERENCE: a press that starts on a field inside a popover is "covered" and never arms the menu, so a slide cannot pick.
4. The review claims right-panel dropdowns already support press-and-slide. **UNVERIFIED**: the shared code opens on click (FACT 3). The first test decides it.

## Scope

1. **Keyboard routing while a chevron menu is open.** While `app.ui.menu == Some(MenuKind::ToolGroup(gi))` and the menu has a highlighted row:
   - Enter and Space go to the menu and pick the highlighted tool.
   - The chevron's `clicked()` toggle ignores keyboard activation while its own menu is open.
   - Escape still closes the menu (unchanged) and returns focus to the chevron.
2. **Dropdown press-and-slide.** Make the dropdown open on the primary **press** in its rect, the same way the chevron does (`primary_pressed_in`, `toolbar.rs:117`). The press then arms the menu, and a release on a row picks it.
   - The opener's own layer, including a popover layer, must not count as covering for that press.
   - Keep keyboard opening (`clicked()` without the pointer) as today.
3. Apply the change in the shared `dropdown_items`, so the right panel and popovers behave the same.

## Acceptance criteria

- **AC-1** Focus the Shape group chevron by keyboard and press Space: the menu is open. Press ArrowDown, then Enter: `app.tool` is the highlighted tool, the menu is closed and focus returns to the chevron. Red on the baseline (closes with no pick).
- **AC-2** The same with Space as the final key.
- **AC-3** Open by keyboard, then Escape: closed, tool unchanged.
- **AC-4** A dropdown inside the Type settings popover: press on the field, drag to the 3rd option, release. Option 3 is picked. Red on the baseline.
- **AC-5** The same gesture on a right-panel dropdown picks. Record whether it was red or green on the baseline in the PR. This settles the review's claim.
- **AC-6** A plain click on a dropdown field opens it and leaves it open (no pick). A second click on a row picks.
- **AC-7** The existing tests stay green: the `widgets/menu.rs` tests, the toolbar tests (`toolbar.rs:730-760`) and the panel dropdown tests.

## Test steps

1. `shell/tests/toolbar_menu_keyboard.rs`: AC-1 to AC-3 with `Sim`. Give focus to the chevron id stored at `toolbar.rs:258` (`test-chevron-id`).
2. Add AC-4 to AC-6 with raw pointer events, using `test-dd-list` rects (`menu.rs:629-640`).
3. `cargo test -p designos-app --lib toolbar menu dropdown typography`
4. Visual: a 1440 capture of the open tool menu with a keyboard highlight.

## Edge cases

- The chevron menu opened by mouse, then keyboard Enter: it picks the highlighted row, or does nothing when nothing is highlighted.
- A dropdown inside a popover that is clipped by the window edge (#889 U-13): out of scope, but the release must not pick a clipped row that is not drawn.
- Two popovers stacked: the press arms only the menu opened by it.

## Non-goals

- New keyboard shortcuts.
- Typeahead in menus.

## Parity facts

macOS pop-up buttons and Figma dropdowns support press-and-drag-to-pick, and keyboard Space/Enter to pick the highlighted item. (Platform convention; not re-verified by REA in this run.)

## Decisions recorded

- Opening on press is shared by every dropdown. If AC-5 shows the right panel was already green through another path, the builder keeps one path and deletes the other.

## Rules

- Open one PR that references #944. Use "Fixes #944" only when every acceptance criterion is met. Do not merge, tag or close.
- Re-anchor every cited line against HEAD before writing code, and report drift first.
- Work on a branch or worktree from `main`, never on `main`.
- Merge with squash only, after `gh pr update-branch <n> --rebase`.
- Blocking CI is `Test (macos-14)`, `Clippy` and `Rustfmt`. Ubuntu and Windows CI are advisory (CLAUDE.md, Git Workflow).
- Test first: each new test is red on the base commit, and that red output goes in the PR body.
- Visual evidence goes in the PR: an app capture, or a terminal render for test-only proof.
- Keep plan ids, issue numbers and finding codes out of code comments and test names. Use conventional commits with no AI trailers.
- UI leg: load `es-designer` before UI code. New or changed strings follow the PCP ux-writing rule (actions of 3 words or fewer, messages of about 8 words).
