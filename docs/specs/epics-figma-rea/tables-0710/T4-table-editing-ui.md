# T4: table editing on the canvas

| Field | Value |
|---|---|
| Issues | #331 (AC-1 "Tab moves between cells"; AC-2 "add/remove/reorder/resize and merge/unmerge are one undo step each"), #1231 edge cases "IME in cells" and "undo of row delete" (UI half) |
| Size | 16 h |
| Tier | Claude Sonnet build (loads `es-designer` first, global rule), Opus review, curator visual pass |
| Depends on | T1b and T2 merged. Rulings in `controller-rulings.md` (OQ-3, -4, -5, -6, -8, -11, -15) and the owner's OQ-16 answer are in. Still open, asked with mocks once T2 renders: OQ-2 padding and alignment, OQ-7 table handles, OQ-9 row/column grips (OQ-1 default look for T5). |
| Runs in parallel with | T3a, T3b, T5b. Never with T5: they share canvas interaction and shell files. |
| Blocks | T5 |
| Branch | `jangtrinh/tables-t4-edit-ui` |

## Goal

In a Design or Board file, a user edits an existing table entirely on the canvas:
- select a table, a cell or a block of cells;
- type in a cell, IME included;
- move between cells with Tab and Shift+Tab;
- add, remove, reorder and resize rows and columns;
- merge and unmerge.

Every structural action is exactly one undo step.

## Behaviour (rulings and the owner's answers applied; OQ-2, OQ-7 and OQ-9 still show the recommended option)

Point 1 below is ruling OQ-15. **Layers panel, owner ruling OQ-16: a table is one row with no children.** Cells stay real child nodes in the model (T1 re-check), so `crates/designos-app/src/panels/layers.rs` gets a filter, for this purpose only:
- row building (`:75-81` at `main@ec47da26d`) does not descend into a `Table`, and a `Table` row reports `has_children = false` (no expand arrow);
- the auto-expand of the selection's ancestors (`:322-330`) never inserts a `Table` into `expanded`;
- selecting a cell on the canvas highlights its table's row;
- dropping a layer into a table is already impossible: `DropKind::Inside` is offered only for `is_container()` kinds (`:110`), and `Table` is not one (FACT). AC-8 asserts it stays so.

1. **Selection levels.**
   - A first click selects the table as a layer: `hit.rs` does not descend because `Table` is not `is_container` (T1 decision 6).
   - A click on a cell of an already-selected table selects that cell, using `table::cell_at`.
   - Shift+click or a drag across cells selects a rectangular block.
   - Esc steps up one level (text → cell → table).
2. **Text.**
   - Double-click a cell, Enter on a selected cell, or typing a printable key on a selected cell starts `canvas::textsession::begin_text_edit(app, cell_id)` (`crates/designos-app/src/canvas/textsession.rs:80` at `main@ec47da26d`).
   - IME preedit and commit flow through the existing handlers (`textsession.rs:664-665` at `main@ec47da26d`), because cell text is reached through `any_text_mut` (T1 decision 5).
   - When an edit commits, the caller appends the `relayout` commands to the same undo step, so a row that grew to fit the text (OQ-6) undoes with the text.
3. **Tab** (#331 AC-1). While editing a cell, Tab commits and starts editing the next anchor (`table_ops::next_cell`); Shift+Tab goes to the previous one.
   - At the last cell: ruling OQ-4 (a): append a row and edit its first cell, one undo step.
   - Today Tab is handled at `crates/designos-app/src/shell.rs:1521-1535` (selection cycling) and `:2086` (raw key event). The table branch must take precedence only while a table cell is being edited or selected; the shell test asserts the old behaviour elsewhere.
   - Sticky "Tab creates the next sticky" (`board/create.rs:93` `tab_next`) must not fire inside a table.
4. **Rows and columns** (OQ-9 handles, OQ-11 menu):
   - Hover grips on the table's left edge (rows) and top edge (columns).
   - Dragging a grip reorders (`move_row` / `move_col`). A drop that would split a merge is refused, the drop indicator does not show there, and a toast names the fix.
   - Dragging a boundary line resizes (`resize_row` / `resize_col`, min size from OQ-1). One drag is one undo step, with no intermediate steps (preview without commit, then one `exec_user` on release).
   - A "+" affordance at a boundary inserts there.
   - A context menu on a cell, row or column selection offers the OQ-11 entries.
5. **Merge and unmerge** (OQ-8): with a block selected, "Merge cells" sits in the context menu and in the contextual bar where WB-3 provides one. On a merged anchor, "Unmerge cells".
6. **Resizing the whole table** (OQ-7): the table's selection handles scale every track proportionally (`resize_table`). Text is not scaled.
7. **Delete.** Delete or Backspace with a whole row or column selected deletes it. With cells selected (not whole rows), it clears their text (OQ-11). With the table selected, it deletes the table.

All mutations call T1b ops and execute through `History::exec_user` (`crates/designos-doc/src/history.rs:381`). No mutation is built ad hoc in the UI (DRY).

## Files T4 may modify

- New module `crates/designos-app/src/board/table/`: `mod.rs`, `select.rs`, `grips.rs`, `menu.rs`, `overlay.rs`, each < 200 lines
- `crates/designos-app/src/board/mod.rs` (one `mod table;`)
- `crates/designos-app/src/panels/layers.rs` (OQ-16 cell-row filter only; T1 owns its forced arm, merged earlier)
- Hook lines only, each listed in the PR:
  - `crates/designos-app/src/canvas/interact.rs` (press, drag, double-click dispatch into `board::table`)
  - `crates/designos-app/src/canvas/textsession.rs` (Tab while editing a cell)
  - `crates/designos-app/src/shell.rs` (Tab precedence `:1521-1535`, `:2086`; context menu `:1731`)
  - the canvas overlay draw call site that draws selection chrome ([UNVERIFIED] file, found by the builder)
- `crates/designos-app/src/shell/tests/table_edit.rs` (new) and one `mod` line in `shell/tests.rs`

Chrome colours come only from `crates/designos-app/src/theme.rs` tokens. No new hex values.

## Acceptance criteria (shell tests, red at base, green at head)

1. Tab (#331 AC-1): on a 3×3 table with a 2×2 merge:
   - double-click the anchor, type "a", press Tab five times, typing a letter each time;
   - the cells, in `next_cell` order, hold "a", "b", "c", "d", "e", "f";
   - Shift+Tab returns to the previous cell;
   - Tab at the last cell behaves per OQ-4.
2. One undo step each (#331 AC-2). For each UI action, `undo_label` changes exactly once, and one Cmd+Z restores the pre-action document (node-by-node compare). The actions:
   - insert row
   - insert column
   - delete row
   - delete column
   - reorder row by grip drag
   - reorder column
   - resize row by boundary drag (with ≥ 5 intermediate pointer moves)
   - resize column
   - merge
   - unmerge
   - resize the whole table
3. Undo of row delete (#1231): delete row 2 with text via the context menu, then Cmd+Z. The texts and NodeIds are back. Cmd+Shift+Z deletes it again.
4. IME (#1231): in a cell, send `Event::Ime(Preedit "việ")` then `Commit "việt"` (pattern from `shell/tests/text_styles.rs:584-609`). The cell text is "việt", and Tab during an active preedit does not move cells.
5. Merge is refused for a block that partially overlaps a merge. The menu entry is disabled with a reason tooltip, so no dead control (Zero Fake UI).
6. Tab outside tables is unchanged: the existing Tab tests in `shell/tests` and the board sticky Tab test still pass.
7. `python3 scripts/zero_fake_ui_gate.py` passes: every new control acts.
8. Layers panel (owner ruling OQ-16): a 50×50 table shows as one row with no expand arrow and no cell rows; selecting a cell on the canvas highlights the table row and does not expand it. The row count before and after inserting the table differs by exactly 1. Dragging a layer onto the table row offers only Before / After.
9. Layer commands with a cell selected never break the table (T1 decision 8): Cmd+D, Cmd+G, Cmd+Alt+G, paste and arrow-key nudge each leave the table's cells and tracks unchanged and do not panic; a cell selection shows no move or resize handles. Each command either acts on the cell's text, acts on the whole table, or does nothing; the PR lists which, per command.

## Test seam

Shell tests (`s.key`, `s.frame(events)`, `s.cmd_key`), as in `crates/designos-app/src/shell/tests/*.rs`, using T1b `create_table` to set up.

## Non-goals

- The Table tool, toolbar and Shift+T (T5).
- MCP (T5b).
- Copy and paste of rows.
- Sorting.
- Cell-level style inspector beyond fill (not requested; owner may add).
- Responsive captures (owner default: SKIPPED).

## Gates

```bash
export PATH="/Users/jangtrinh/.local/bin:$HOME/.cargo/bin:$PATH"
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p designos-app table_edit
cargo test -p designos-app
python3 scripts/zero_fake_ui_gate.py
python3 scripts/design_os_self_evolve.py
```

## Visual evidence, in this order (global "Figma first, gates after" rule)

1. Rebuild the release app, then capture at 1440 a Design file and a Board file, each showing:
   - a selected cell block with grips;
   - a merged anchor;
   - the context menu;
   - a cell mid-IME-composition.
2. Send "screens ready" to the controller. The controller places the captures in Figma (figma-agent) before gates run.
3. Then run gates and CI. Report "responsive: SKIPPED (owner default)".

## Risks

| Risk | L×I | Mitigation |
|---|---|---|
| Tab precedence breaks existing Tab behaviour | M×H | AC-6; table branch gated on "a table cell is edited or selected" |
| Drag resize creates many undo steps | M×M | Preview without commit; one command on release (AC-2) |
| A generic layer command reaches a cell and the guard refuses it with an unhandled error | M×M | AC-9; the guard error is handled by the existing command error path, no panic |
| Hook lines collide with in-flight PRs (#1675 connector bar) | M×M | `gh pr view 1675 --json files` first; rebase; list shared files in the PR |

## Rollback

Revert. Tables stay viewable (T2), but can only be edited over MCP (T5b).

> Amended 2026-11-07 (R-T1-2): call `check_table_positions` at the commit points outside `History` (`Tx::cmd`, live-patch replay) and test a rollback through them.
