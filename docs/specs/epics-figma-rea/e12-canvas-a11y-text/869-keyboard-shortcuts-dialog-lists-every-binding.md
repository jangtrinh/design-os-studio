# #869 The Keyboard shortcuts list shows every binding, in the platform's notation, with no overlap

| Field | Value |
|---|---|
| Issue | #869 (epic #124; bug, P2; backlog; moved v0.6.2 → v0.6.3); details `plans/reports/scout-261022-1800-test-round-3.md` §4 |
| Slice | S1, wave 3, leg H |
| Size | 8 builder hours (option Q1 b: about 14) |
| Tier | Claude Sonnet (UI layout and copy; es-designer; platform notation across menus) |
| Depends on | #904 and #944 merged (`panels/menus.rs`, `widgets/menu.rs`, `panels/toolbar.rs`), #828 and #1019 merged (`agent_a11y.rs`). Owner answers **Q1–Q4** |
| Touches | new `crates/designos-app/src/shortcuts.rs` (collect + format), `crates/designos-app/src/home/dialog.rs` (`show_shortcuts` `:191-211`; `show` measure and draw `:243-300`), `crates/designos-app/src/panels/menus.rs` (read-only walk of `main_nodes` `:490`, the zoom rows `:751-757`, `tool_nodes` `:645`; `leaf` `:384` gets the formatter if Q2 a/b), `crates/designos-app/src/canvas/tools.rs` (`shortcut()` `:78`, `from_key_board` `:132`), `crates/designos-app/src/agent_a11y.rs` (`FOCUS_SHORTCUT_LABEL` `:18`), `crates/designos-app/src/shell.rs` (only if Q3 a: one key binding). With Q2 (a) or (b) also: `crates/designos-app/src/widgets/menu.rs` (`MenuItem` shortcut drawing), `crates/designos-app/src/panels/toolbar.rs` (tooltips "More actions   Ctrl+K" `:207`, "Actions   Ctrl+K" `:295`, `:426`) and `crates/designos-app/src/cmdk.rs` (palette rows already show macOS glyphs on every OS, e.g. `"⌥⌘D"` `:154`). Tests in `crates/designos-app/src/shell/tests/shortcuts_dialog.rs` |

**Code baseline:** `origin/main@d22dad970`. Re-anchor against HEAD first.

## User-visible outcome

Help → Keyboard shortcuts (and Ctrl+Shift+? if Q3 a) opens a list that:
- shows **every** shortcut the menus and the toolbar show, grouped under the menu that owns it (File, Edit, View, Object, Text, Vector, Help…) plus a Tools group;
- shows the tools of the open file: design tools in a design file, board tools (Sticky S, Connector X, Stamp E, …) in a board file;
- uses the platform's keys: on macOS ⌘ ⇧ ⌥ ⌃ (Q2 a) or Cmd/Shift/Option (Q2 b), on Windows and Linux Ctrl/Shift/Alt;
- fits any window: a 1024×768 window shows the list scrollable, with no row drawn over another.

## Current state (FACT)

1. `App::show_shortcuts` builds 13 hand-written key and action pairs plus one note (`home/dialog.rs:191-211`). All of them say "Ctrl". The agents-focus row comes from `agent_a11y::FOCUS_SHORTCUT_LABEL = "Ctrl+Alt+J"` (`agent_a11y.rs:18`).
2. The dialog draws key text at `x0` and the action at `x0 + 190` (`dialog.rs:288-289`). Its height is the sum of row heights with **no cap** (`dialog.rs:249-262`). Above about 20 rows it would grow past a 768 px window.
3. The menus declare their accelerators as strings in `leaf(label, shortcut, act)` (`menus.rs:384`). There are about 100 `leaf(` calls, and the issue counts 59 with a shortcut, all "Ctrl+…".
   - The exception is the Zoom pill menu, which shows "⌘+", "⌘-", "⌘0" and "⌘'" (`menus.rs:751-757`).
   - The same action shows two labels: View → "Zoom in" is "Ctrl+=" (`menus.rs:538`), while the Zoom pill says "⌘+". The binding is `Key::Equals | Key::Plus` (`shell.rs:1502`).
   - Toolbar tooltips also say Ctrl ("Actions   Ctrl+K", `toolbar.rs:295`).
   - egui maps Cmd on macOS to `modifiers.command` (`plans/reports/grok-261103-1840-pr-1561.md:32`, citing `egui-winit-0.35.0/src/lib.rs:468-484`), so the bindings are Cmd on macOS while the labels say Ctrl.
4. Design tool keys: `Tool::shortcut()` (`tools.rs:78-104`). Board keys: `Tool::from_key_board` (`tools.rs:130-147`). The issue counts 15 design and 10 board keys.
5. The Help menu leaf "Keyboard shortcuts" has no accelerator (`menus.rs:612`). It is also opened from the rail (`rail.rs:160`) and `right.rs:179`.
6. Existing tests that read the dialog: `dev_mode_shortcut.rs:136` (`the_shortcuts_dialog_lists_shift_d`), `home.rs:481` and `agent_panel_keyboard/overlay_escape.rs:101,261`.
7. A UI audit already flagged the Ctrl-only copy on macOS and recommended platform-derived labels (`plans/reports/ui-audit-040-evidence/ledgers/critique-a-home-states.md`, HS-27). It was not ruled on, hence Q2.

## Scope

1. **One source.** `shortcuts::collect(app) -> Vec<Group>` walks:
   - the menu tree `main_nodes(app)`, recursing into submenus and ignoring the disabled state;
   - the zoom rows;
   - the tool keys of the active file kind (design or board);
   - the agents focus row.

   It keeps every leaf with a non-empty shortcut as `(group, action label, shortcut)` and removes only exact duplicates (same group, label and key). "Ctrl+K" appears twice in Help (`menus.rs:610-611`) with two labels: both rows stay, so no new string is made.
2. **One formatter.** `shortcuts::display(s: &str, os) -> String` turns the canonical "Ctrl+Shift+S" into the platform notation (Q2). The dialog uses it. With Q2 (a) or (b) the menu `leaf` rendering, the Zoom rows and the toolbar tooltips use it too. The Zoom pill's "⌘+" becomes the canonical "Ctrl+=" that the View menu already uses, so one action has one label. Store canonical strings only.
3. **The dialog (Q1 a).**
   - Title "Keyboard shortcuts".
   - Rows grouped under a small group header.
   - Two columns as today: keys in mono at `x0`, action at `x0 + 190` (`dialog.rs:288-289`). Widen the key column only if a formatted key does not fit, and record the new width in the PR.
   - The body scrolls inside a max height of `screen.height − 2 × 48 px`.
   - The footer note (the agent-terminal undo note) stays last.
   - Fixed width as today (`W`).
   - Rows are 22 px on the 4 px rhythm (design/principles.md).

   Q1 (b) replaces this with a bottom panel with category tabs. Re-scope the size if chosen.
4. **Open key (Q3 a).** Bind Ctrl+Shift+? to `Action::Shortcuts` and show it on the Help leaf.
   - Figma uses **Control** (not Cmd) on macOS, so on macOS match `modifiers.ctrl`. Elsewhere match `modifiers.command`.
   - The canonical string needs a way to say "Control even on macOS" (for example a `Ctl+` prefix), and the formatter must render it ⌃.
5. **Canvas-only keys (Q4).** Default (a): not listed. With (b), add a "Canvas" group from a hand-kept table, plus a test that each listed key is handled by `Shell::key` or `Tool::from_key*`.

## Acceptance criteria

- **AC-1** Set test: the set of `(action, shortcut)` pairs the dialog shows equals the set collected from the menu tree, zoom rows and toolbar tools of the active file kind. Built independently in the test from `main_nodes` and the tool tables. Fails if a menu leaf with a shortcut is missing from the dialog.
- **AC-2** On macOS (cfg or injected OS) no row shows "Ctrl+" for a `command` binding. Each row shows the Q2 notation. On Windows and Linux rows show "Ctrl+".
- **AC-3** A board file lists the board tool keys (Sticky S, Connector X, Stamp E, Marker P, Highlighter Shift+P, Shape R, Text T, Section Shift+S, Move V, Hand H) and not the design-only tool keys (Frame F, Pen P…). A design file lists the reverse.
- **AC-4** No overlap: in a `Sim` window of 1024×768, every drawn row rect (recorded through `widgets::record` and read with `last_pass_names()`) is inside the dialog body clip, and no two row rects intersect. The last row is reachable by scrolling. The red baseline adds 30 synthetic rows to the old dialog to show that it overflows.
- **AC-5** With Q3 (a): Ctrl+Shift+? (macOS: ⌃⇧?) opens the dialog from the canvas, and the Help leaf shows that key.
- **AC-6** The existing tests in FACT 6 pass (update `the_shortcuts_dialog_lists_shift_d` if the label text changes).
- **AC-7** Copy follows the ux-writing rule: action labels are the menu labels themselves (no new strings), group headers are the menu names, and the note is 8 words or fewer if rewritten.
- **AC-8** Visual: 1440 captures of the dialog in a design file and in a board file, plus a 1024×768 capture scrolled to the end (responsive check requested by the issue itself).

## Test steps

1. Unit: `shortcuts::display` table test. Canonical "Ctrl+Shift+S", "Ctrl+Alt+Enter", "Shift+A", "Ctrl+=" and "Ctrl+\\" map to macOS and other-OS output.
2. `shell/tests/shortcuts_dialog.rs`: AC-1, AC-3, AC-4 and AC-5 with `Sim`.
3. `cargo test -p designos-app --lib shortcuts menus dev_mode_shortcut home overlay_escape`
4. `python3 scripts/zero_fake_ui_gate.py` (no fake rows: every listed key must be bound, which AC-1 proves for menus and tools).

## Edge cases

- A menu leaf whose shortcut is context-only, like "Enter" for Edit vertices (`menus.rs:604`): listed under its menu (it is a menu shortcut).
- A Project tab: the main menu greys file actions but still lists them (disabled state is ignored).
- A long action label: elide it at the column width. The full label shows on hover.
- Duplicate key with different actions in design and board (S = Slice vs Sticky): only the active file kind's tool is listed.

## Non-goals

- A searchable list, "used" highlighting (Figma), or a Layout tab.
- Remapping shortcuts.
- Canvas-only keys unless Q4 (b).

## Parity facts

- Figma opens its shortcuts panel with Control+Shift+? on macOS and Ctrl+Shift+? on Windows. It opens along the bottom with category tabs, you can keep working while it is open, and used shortcuts are highlighted. Source: [Figma help, "Keyboard shortcuts"](https://help.figma.com/hc/en-us/articles/360040328653), read 2026-11-06 through search.
- Figma's own menus on macOS show glyphs (⌘ ⇧ ⌥). This is a platform convention, not re-verified by REA in this run.

## Decisions recorded

- Canonical shortcut strings stay "Ctrl+…" in source, and only display is per platform. This avoids two spellings in code and keeps the existing tests and string search working.
- Disabled menu state is ignored when collecting: the list documents keys, not current availability.

## Rules

- Open one PR that references #869. Use "Fixes #869" only when every acceptance criterion is met. Do not merge, tag or close.
- Re-anchor every cited line against HEAD before writing code, and report drift first.
- Work on a branch or worktree from `main`, never on `main`.
- Merge with squash only, after `gh pr update-branch <n> --rebase`.
- Blocking CI is `Test (macos-14)`, `Clippy` and `Rustfmt`. Ubuntu and Windows CI are advisory (CLAUDE.md, Git Workflow).
- Test first: each new test is red on the base commit, and that red output goes in the PR body.
- Visual evidence goes in the PR: an app capture, or a terminal render for test-only proof.
- Keep plan ids, issue numbers and finding codes out of code comments and test names. Use conventional commits with no AI trailers.
- UI leg: load `es-designer` before UI code. New or changed strings follow the PCP ux-writing rule (actions of 3 words or fewer, messages of about 8 words).
