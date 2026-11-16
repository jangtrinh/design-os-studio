# #1019 The canvas is the focused element when no widget has focus (macOS, VoiceOver, automation)

| Field | Value |
|---|---|
| Issue | #1019 (epic #124; bug, P2; backlog; moved v0.6.2 → v0.6.3) |
| Slice | S1, wave 2, leg G |
| Size | 8 builder hours |
| Tier | Claude Sonnet (keyboard routing across the whole shell; 10 focus-query sites; regression risk on every shortcut) |
| Depends on | #935 merged (`shell.rs`), #828 merged (`agent_a11y.rs`, `widgets/mod.rs` `labelled_state`). Owner answer **Q6** (spoken name) |
| Touches | `crates/designos-app/src/agent_a11y.rs` (`canvas_id` `:68`, `non_canvas_has_focus` `:74`; new `ensure_canvas_focus`), `crates/designos-app/src/canvas/mod.rs` (canvas response `:399`), `crates/designos-app/src/shell.rs` (call site at the end of the frame; `:1319`), and the 9 focus-query sites in Scope 4. New `crates/designos-app/src/shell/tests/canvas_focus_fallback.rs` |

**Code baseline:** `origin/main@d22dad970`. Re-anchor against HEAD first; list any new `egui_wants_keyboard_input` or `focused()` caller found at HEAD.

## User-visible outcome

When you are not typing in a field, macOS reports a focused element in the Studio window: the canvas, named "Canvas" (Q6). As a result:
- VoiceOver lands on it;
- automation tools (`orca computer`, the accessibility API) can send keys and clicks to Studio instead of failing with "window_not_focused";
- every canvas shortcut keeps working exactly as today, including arrow nudge, Tab sibling walk, Escape and undo.

## Current state (FACT)

1. egui builds the AccessKit tree focus as `memory.focused()` or the root (`egui-0.35.0/src/context.rs:2604-2607`): `focus_id = self.memory.focused().map_or(root_id, …)`. There is no app hook to override it.
2. The root is `Role::Window`, and `accesskit_macos` excludes it from focus: `can_be_focused` is `filter(node) == Include && node.role() != Role::Window` (`accesskit_macos-0.26.3/src/node.rs:288-290`). So with no egui focus, macOS sees no focused element.
3. Studio already has a canvas focus id. `canvas_id()` (`agent_a11y.rs:68`) is the id of the canvas response (`canvas/mod.rs:399`, `ui.interact(rect, ui.id().with("canvas"), Sense::click_and_drag())`).
   - The Agents shortcut Ctrl+Alt+J gives it focus (`agent_a11y.rs:459`).
   - `non_canvas_has_focus(ctx)` (`:74`) is the "typing somewhere" test used by `canvas/interact.rs:347`, `shell.rs:382`, `:1239`, `:2081` and `terminal.rs:6797`.
4. When a widget has focus and no event filter is set, egui's `Focus::begin_pass` moves focus on plain arrows and Tab, and drops it on Escape (`egui-0.35.0/src/memory/mod.rs:560-591`). So a focused canvas **without** a lock filter would lose focus on the first nudge key. This also affects today's Ctrl+Alt+J path (INFERENCE: latent bug).
5. The only fix that avoids patching egui is that the canvas holds egui focus whenever nothing else does (FACT 1). Then `egui_wants_keyboard_input()` (= `memory.focused().is_some()`, `egui-0.35.0/src/context.rs:2884-2886`) becomes **true** while you are not typing. Every site below would then think a field is focused.

## Scope

1. **Fallback focus.** At the end of each frame, after all panels have drawn, call `agent_a11y::ensure_canvas_focus(ctx, app)`. It runs only while AccessKit is active (`agent_a11y::is_accesskit_active()`, `:44`), and when:
   - `memory.focused()` is `None`;
   - a design or board document tab is active, not Home or a Project tab;
   - no modal dialog or `app.ui.menu` is open;
   - the terminal is not capturing the keyboard.

   It requests focus on `canvas_id()` and sets the lock filter `EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true }`, so egui never moves or drops canvas focus on those keys (FACT 4). Apply the same filter in the Ctrl+Alt+J path (`agent_a11y.rs:459`).
2. **Name it.** Label the canvas node with `widgets::labelled_state(ui, canvas_id(), Role::Group, "Canvas", …)` from #828. With Q6 (b) the name is `"Canvas, {page name}"`.
3. **Clicks still win.**
   - A press on the canvas still surrenders another widget's focus (`canvas/interact.rs:402-408`), then the fallback takes it.
   - A click into a field still focuses the field.
   - Tab on the canvas keeps today's sibling walk (`shell.rs:1642-1650`), which surrenders focus first; the fallback takes it back next frame.
4. **Move every "is the user typing?" check to `non_canvas_has_focus`.** Each of these sites currently treats any focus as typing. With the canvas focused, each would silently lose a key:

   | Site (baseline) | Today | Would lose |
   |---|---|---|
   | `shell.rs:1319` (`handle_terminal_undo_keys`) | `ctx.egui_wants_keyboard_input()` | Cmd+Z / Shift+Cmd+Z to the document while the terminal holds keys |
   | `canvas/textsession.rs:770` (`text_keys`) | `egui_wants_keyboard_input()` | Tab and Shift+Tab nesting in a text session |
   | `agent_picker.rs:333` | `!egui_wants_keyboard_input()` | Escape closes the agent picker |
   | `libraries/modal.rs:152` | same | Escape closes the libraries modal |
   | `libraries/modal.rs:410` | same | Escape closes the libraries sub-view |
   | `libraries/review.rs:80` | same | Escape closes the review |
   | `panels/properties.rs:1806` | same | X toggles auto spacing while hovering the align row |
   | `project_tab/tree.rs:718` | same | arrow, Enter and F2 navigation in the project tree. Only if a Project tab can coexist with canvas focus; per Scope 1 it cannot, but keep it safe |
   | `variables/view.rs:280` | `typing = egui_wants_keyboard_input()` | variables view shortcuts |
   | `widgets/picker.rs:2343` | `!ui.memory(m.focused().is_some())` | Delete removes a gradient stop |

   Sites that **surrender** focus (`canvas/interact.rs:406`, `shell.rs:1643`, `terminal.rs:6644`) need no change, because the fallback restores canvas focus next frame. Test-only sites (`field_tab.rs:319`, `menu.rs:1291`) are unaffected. `field_tab.rs:105,138,148` look up the focused id among registered fields; the canvas is not one, so they are unaffected.
5. Add `pub fn typing(ctx) -> bool` as an alias of `non_canvas_has_focus`, if the builder wants a clearer name at call sites. There must be one definition.

## Acceptance criteria

- **AC-1** With AccessKit enabled, a design document open and nothing focused, run one frame. The `TreeUpdate.focus` is the AccessKit id of `canvas_id()`, and that node has role `Group` and name "Canvas" (or Q6 b). Red on the baseline (focus = root).
- **AC-2** After clicking into the X field: focus is the field. After Escape (from #935): focus returns to the canvas within one frame.
- **AC-3** Each of the 10 sites in Scope 4 has a test showing its key still works with the canvas focused: Cmd+Z (terminal case), Tab in a text session, Escape for the picker, modal, sub-view and review, X on the align row, Delete on a gradient stop, and one variables-view shortcut. Mutation check: reverting any one site makes its test red.
- **AC-4** Arrow-key nudge, Shift+arrow, Tab and Shift+Tab sibling walk, Enter and Shift+Enter, and Escape (deselect) all work with the canvas focused. The existing `shell/tests.rs` shortcut tests (for example `shortcuts_duplicate_group_delete` `:423` and `view_toggles_via_shortcuts` `:1203`) and `panel_keys.rs` stay green.
- **AC-5** An AccessKit `Action::Focus` request on the canvas node gives it focus, and keys sent afterwards reach the canvas (issue AC-2). Test by injecting `Event::AccessKitActionRequest`, then `Key::V`: the tool becomes Move.
- **AC-6** Home, a Project tab, an open dialog and an open menu do not get canvas fallback focus.
- **AC-7** Idle CPU stays 0.00%. The fallback requests no repaint and does not bump `TileView::rev` (CLAUDE.md §4 invariant 1). It requests focus only when focus is `None`, so after the first idle frame it does nothing.
- **AC-8** On a macOS build, `orca computer` (or the Accessibility Inspector) reports a focused element in the Studio window with nothing selected. Attach a screenshot of the Accessibility Inspector to the PR (owner-manual if the builder cannot run it).

## Test steps

1. `shell/tests/canvas_focus_fallback.rs`: AC-1, AC-2, AC-5 and AC-6, through `Sim.accesskit_update` (`shell/tests.rs:153`) after `enable_accesskit`.
2. One test per Scope 4 row (AC-3), in the module that owns the site's existing tests where one exists.
3. `cargo test -p designos-app --lib` (whole crate: this touches shared routing).
4. Idle probe: `ps -p <pid> -o time=` over 20 s on `target/release/designos <fixture> --gpu` after settle. Expect dCPU = 0.000 s (method from CLAUDE.md §4).
5. Visual: an Accessibility Inspector capture showing the focused "Canvas" element.

## Edge cases

- Text editing on the canvas (`app.canvas.text`): the text session is not an egui widget. Canvas focus stays, and `text_keys` must now run (Scope 4 row 2).
- The agent composer is idle-focused (`composer_idle_focus`, `shell.rs:1316-1318`): the composer has focus, and the fallback does nothing.
- The window loses OS focus: egui keeps `memory.focused()`, and there is no change.
- A modal opens over a focused canvas: the modal's own Escape logic now uses `non_canvas_has_focus`, so Escape closes the modal.

## Non-goals

- **Home, Project tabs, open dialogs and open menus.** They get no fallback element in this leg (AC-6), so on those screens automation can still report `window_not_focused`.
  - The issue's acceptance criterion names the canvas as the example, but it says "a focused element in the Studio window".
  - So the PR says "Refs #1019", not "Fixes", unless the controller first files a follow-up for those screens. The same pattern would apply there: a `Role::Group` node named after the view.
- The canvas accessibility tree: layers, selection path and announcements. That is #386.
- F6 region cycling. That is #387.

## Parity facts

- Native macOS apps always expose a focused UI element while key. AccessKit's macOS adapter explicitly refuses a focused `Window` (FACT 2), so a focusable child is needed.

## Decisions recorded

- The fallback runs only while AccessKit is active. The macOS tree, and so a reported focus, exists only then: egui fills `accesskit_update` only `if let Some(state)` (`egui-0.35.0/src/context.rs:2594-2595`). Sessions with no assistive tech or automation attached keep today's focus behaviour exactly. The 10 site changes in Scope 4 still ship for every session, because they are correct either way. The Ctrl+Alt+J lock-filter fix applies always.
- `Role::Group` for the canvas, named "Canvas". The spoken page name waits for Q6.
- The lock filter claims Tab, arrows and Escape for the canvas. The shell already gives them canvas meaning.
- The fallback runs at end of frame, so a widget that took focus this frame always wins.

## Rules

- Open one PR that references #1019. Use "Fixes #1019" only when every acceptance criterion is met. Do not merge, tag or close.
- Re-anchor every cited line against HEAD before writing code, and report drift first.
- Work on a branch or worktree from `main`, never on `main`.
- Merge with squash only, after `gh pr update-branch <n> --rebase`.
- Blocking CI is `Test (macos-14)`, `Clippy` and `Rustfmt`. Ubuntu and Windows CI are advisory (CLAUDE.md, Git Workflow).
- Test first: each new test is red on the base commit, and that red output goes in the PR body.
- Visual evidence goes in the PR: an app capture, or a terminal render for test-only proof.
- Keep plan ids, issue numbers and finding codes out of code comments and test names. Use conventional commits with no AI trailers.
- Re-measure idle CPU after the change with the CLAUDE.md §4 method, and paste the numbers in the PR.
