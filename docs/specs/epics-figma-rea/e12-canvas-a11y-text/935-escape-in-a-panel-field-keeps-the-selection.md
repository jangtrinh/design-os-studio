# #935 Escape in a panel field only leaves the field (and a deliberate retype is kept)

| Field | Value |
|---|---|
| Issue | #935 (epic #124; backlog; moved v0.6.2 → v0.6.3); includes the 10-22 comment (PR #894 re-review R1-1, `widgets/field.rs` about 62) |
| Slice | S1, wave 1, leg D |
| Size | 4 builder hours |
| Tier | Claude Sonnet (shared field state used by every inspector field; agent-concurrency edge) |
| Depends on | none. **Blocks** #1019 (`shell.rs`) |
| Touches | `crates/designos-app/src/widgets/field_tab.rs` (raw-input stamp next to `take_tab_press` `:100`), `crates/designos-app/src/shell.rs` (raw-input hook `:2077-2080`; Escape guard arm `:1589` only), `crates/designos-app/src/widgets/field.rs` (edited flag `:46-115`), new `crates/designos-app/src/shell/tests/field_escape.rs` |

**Code baseline:** `origin/main@d22dad970`. Re-anchor against HEAD first.

## User-visible outcome

1. You type in a Design panel field (X, a hex colour, a layer name) and press Escape. The edit is cancelled and the field is left. The canvas selection and the active tool stay as they were.
   - A second Escape, with no field focused, works as today: it cancels a tool, then clears the selection.
2. An agent changes a field's value while you are typing in it. You retype the original value and press Enter. Your value is written; today it is ignored.

## Current state (FACT)

1. The shell's key handler runs **before** the panels draw (`widgets/menu.rs:614-616` documents the order). egui drops widget focus on Escape before app code runs (`menu.rs:668`, comment "egui clears keyboard focus on Escape in Focus::begin_pass").
   - So by the time the Escape arm at `shell.rs:1590-1613` runs, no field has focus.
   - The arm then runs the canvas chain: cancel the interaction, the pen, the eyedropper, a menu, prototype editing, find, the layer filter, the tool, then `clear_selection()` (`:1608`).
2. Two owners already take Escape first with a guard arm (`shell.rs:1589`): `app.comments.captures_escape()` and `dropdown_menu_was_open(ctx)` (`menu.rs:618-621`, a pass-number stamp).
3. Fields register as Tab stops through `field_tab::register` (`field_tab.rs:49`). `take_tab_press` already inspects the raw input **before** egui (`field_tab.rs:100-123`; called from `shell.rs:2079`), while `memory.focused()` still holds the field.
4. Field edit state (`field.rs:46-70`): a buffer equal to its seed, or to the live value, counts as "not edited" and follows the live value (`:60-68`). On focus loss without Escape it commits only when `buf != display && buf != seed` (`:109-110`).
   - So if an agent moved the live value away and you retype the seed value, `buf == seed`, and your retype is dropped. This is the R1-1 comment.

## Scope

1. **Field Escape stamp.** In the raw-input hook (`shell.rs:2077-2080`), call a new `field_tab::note_field_escape(ctx, raw)`. It runs next to `take_tab_press`, before egui reads the input. It stamps the current pass when:
   - the raw events hold an Escape press;
   - `memory.focused()` is a field or dropdown registered in the last frame (`Stops.drawn` / `last`).

   Add `pub fn escape_was_for_field(ctx) -> bool`, true for the stamped pass only. This is the same pattern as `dropdown_menu_was_open`.
2. **Shell guard.** Extend the guard arm at `shell.rs:1589` with `|| crate::widgets::field_tab::escape_was_for_field(ctx)`. The field's own Escape handling (`field.rs:109`: Escape means no commit) is unchanged, so the edit is cancelled.
3. **Edited flag.** Replace the "edited = differs from seed" test with an explicit flag:
   - Set it when the `TextEdit` reports `changed()` (typing, paste, stepping).
   - Clear it on focus gain and on commit.
   - Commit on Enter or focus loss when the flag is set and Escape was not pressed, even if the text equals the seed.
   - While the flag is clear, the buffer keeps following the live value (current rule `:60-68`).
4. Covers every field drawn through `field::edit_text_hint`, `edit_num_text` and the dropdowns registered through `field_tab::register`. Builder: list any field kind that does not register.

## Acceptance criteria

- **AC-1** With one layer selected and the Move tool, click into the X field, type "12" and press Escape. The X value is unchanged, the field has no focus, the selection still holds the layer, and the tool is still Move. Red on the baseline (the selection clears).
- **AC-2** The same for a hex colour field and a text field (the layer name in the inspector, or the typography content field, whichever is registered).
- **AC-3** A second Escape (no field focused) clears the selection, as today.
- **AC-4** Tool other than Move (Rectangle), Escape in a field: the tool stays Rectangle. Second Escape: Move.
- **AC-5** Agent race (R1-1): focus X (value 10) and type "1". An agent update sets X to 20 (live edit). Type "0" so the buffer is "10", press Enter. X is 10. Red on the baseline (stays 20).
- **AC-6** No regression: a field that was focused but not typed in follows a live change and commits nothing on blur (`field.rs:60-63` behaviour). Covered by the existing field tests plus one new test.
- **AC-7** Escape while a dropdown menu is open, and Escape in a comment draft: unchanged (existing tests in `shell/tests/agent_panel_keyboard/overlay_escape.rs` stay green).

## Test steps

1. `shell/tests/field_escape.rs`: AC-1 to AC-5 with `Sim`. Focus a field by clicking its recorded rect, type with `Event::Text`, press Escape with `Event::Key`.
2. Run red on the baseline (paste the output), then green.
3. Run `cargo test -p designos-app --lib field panel_keys overlay_escape`.
4. Visual: a 1440 capture after AC-1, showing the selection box still on the layer.

## Edge cases

- Escape during IME composition in a field: the IME takes it. No stamp unless egui sees `Key::Escape` pressed.
- A field inside a popover (Type settings): the same rule. The popover's own Escape close order is unchanged (the dropdown guard runs first).
- The rename field in the Layers panel (`app.layers.renaming`): it is not a registered field. Its Escape behaviour is unchanged; the builder confirms this with a test.
- The agent composer (`composer_id`): not a registered field. Unchanged.

## Non-goals

- Changing what the canvas Escape chain does with no field focused.
- `escape_clears_selection()` (`shell.rs:1417`): stays `true`.

## Parity facts

- The issue states: "In Figma, Escape in a field only leaves the field." (Owner-filed, PR #894 review.) Not re-verified by REA in this run.

## Decisions recorded

- Use a raw-input stamp rather than reading egui focus in the shell. Focus is already gone by then (FACT 1).
- An explicit edited flag replaces the seed comparison. The live-follow rule is kept.

## Rules

- Open one PR that references #935. Use "Fixes #935" only when every acceptance criterion is met. Do not merge, tag or close.
- Re-anchor every cited line against HEAD before writing code, and report drift first.
- Work on a branch or worktree from `main`, never on `main`.
- Merge with squash only, after `gh pr update-branch <n> --rebase`.
- Blocking CI is `Test (macos-14)`, `Clippy` and `Rustfmt`. Ubuntu and Windows CI are advisory (CLAUDE.md, Git Workflow).
- Test first: each new test is red on the base commit, and that red output goes in the PR body.
- Visual evidence goes in the PR: an app capture, or a terminal render for test-only proof.
- Keep plan ids, issue numbers and finding codes out of code comments and test names. Use conventional commits with no AI trailers.
