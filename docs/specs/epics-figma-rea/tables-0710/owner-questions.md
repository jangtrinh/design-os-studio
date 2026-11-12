# Tables in 0.7.11: questions for the owner

These are user-visible details that #317, #331 and #1231 do not state.

How to ask:
- Use AskUserQuestion, at most 4 per round, recommended option first.
- Rounds 1-3 go before T4 starts, round 4 before T5.
- T0, T1, T1b, T2, T3 and T5b may start now. T1 and T1b use the recommended options and mark them in the PR, so an answer changes only a constant or one rule.
- Before asking a visual question (marked "visual"), show a mock or capture (owner rule: show screenshots before asking to judge anything visual). Build quick mocks in Studio once T2 renders tables; until then, describe in words.

Already decided by the owner, so not asked again:
- Responsive captures are skipped by default (owner 2026-10-10).
- The Purple Ban covers Studio chrome (`design/principles.md`).

Each question's id (for example OQ-1) is a tracking reference for the specs.

## Round 1: structure (blocks T4)

**What does a new table look like?** (OQ-1, visual) The issue fixes only 3×3.
- **(a) Recommended: dark, from the Studio Untitled UI dark tokens.** Reason: the controller's brief asks for Untitled UI dark tokens, and the result reads well on the default canvas. Values are FACT, from `crates/designos-app/src/theme.rs`:
  - 3×3, columns 160 px, rows 44 px (480×132 in total);
  - background `BG_SECONDARY` #171717;
  - grid lines 1 px `BORDER` #262626;
  - text 14 px Inter `TEXT_SECONDARY` #D4D4D4;
  - minimum track 24 px (pointer-target rule).
- (b) Light neutral: white background, #D4D4D4 lines, #171717 text, same sizes. Suits exports onto white paper and light kit content.
- (c) Follows the file: uses the file's own "surface / border / text" colour variables when it has them, otherwise (a). The most adaptive option, but also the most work and the least predictable.

**How does someone get from the table into a cell?** (OQ-15)
- **(a) Recommended:** the first click selects the whole table, like a frame. A second click on a cell selects that cell. Double-click, Enter, or typing on a selected cell starts editing it. Esc steps back up: text, then cell, then table. Reason: it matches how frames and groups are entered today, and moving a table never starts a cell edit by accident.
- (b) The first click selects the cell under the pointer directly. The whole table is selected by clicking its edge or from Layers.
- (c) The first click selects the table. Double-click goes straight into text editing. No cell-selection level exists, so merge works only from the right-click menu.

**What does Tab do in the last cell?** (OQ-4)
- **(a) Recommended:** add a new row and start typing in its first cell, as one undo step. Reason: you can keep typing a list without the mouse, as in FigJam and most spreadsheets.
- (b) Wrap to the first cell.
- (c) Stay in the cell and stop editing.

**What happens to text in cells that a merge covers?** (OQ-5)
- **(a) Recommended:** append it to the merged cell, one line per covered cell. Reason: no typed text is lost silently, and undo brings the cells back.
- (b) Keep only the top-left cell's text and discard the rest. Undo still restores it.
- (c) Refuse to merge while covered cells hold text.

## Round 2: editing (blocks T4)

**How does someone merge cells?** (OQ-8)
- **(a) Recommended:** select a block of cells (drag across them, or Shift+click), then choose "Merge cells" from the right-click menu or the floating bar. Reason: two discoverable places and no new shortcut.
- (b) The same as (a), plus the shortcut Cmd+Option+M. That shortcut is a proposal and needs a conflict check.
- (c) The right-click menu only.

**What padding and alignment does cell text get?** (OQ-2, visual)
- **(a) Recommended:** 12 px left/right, 8 px top/bottom, text at the top left. Reason: it follows the 4 px rhythm (`design/principles.md`), and top alignment keeps rows of different heights readable.
- (b) Same padding, text centred vertically.
- (c) 8 px on every side, top left.

**What happens when text is taller than its row?** (OQ-6)
- **(a) Recommended:** the row grows to fit, and never shrinks below the height the user set. Reason: no hidden text, and it matches FigJam.
- (b) Text is cut off at the cell edge.
- (c) The row grows, and shrinks back when the text gets shorter.

**Do cells appear as layers in the Layers panel?** (OQ-16)
- **(a) Recommended:** the table is one layer row. Its cells do not appear as child rows, and the table cannot be expanded. Reason: a 50×50 table would otherwise add 2,500 rows, and cells are reached on the canvas. Agents still see the cells over MCP.
- (b) The table expands to show its cells, named "Cell r1c1" and so on, collapsed by default.
- (c) The table expands to show its rows, each row expanding to its cells.

## Round 3: rows, columns and the table frame (blocks T4)

**Does a table have a header row?** (OQ-3)
- **(a) Recommended:** no special header in 0.7.11. People can bold or fill the first row themselves. Reason: the issues do not ask for one, and no header flag is known in the file fields. T0 (U10) checks this.
- (b) The first row is bold, on a darker fill, by default. This is styling only.
- (c) A real "header row" toggle that stays on top. This is new scope and needs its own issue.

**What do the handles on a selected table do?** (OQ-7, visual)
- **(a) Recommended:** corner and side handles scale every row and column proportionally, and text keeps its size. Reason: it behaves like resizing a frame.
- (b) The handles resize only the last row and column.
- (c) A selected table has no resize handles. Rows and columns are resized one at a time.

**What do row and column handles look like?** (OQ-9, visual)
- **(a) Recommended:** a small grip appears on hover over the left edge (rows) or the top edge (columns). Drag the grip to reorder, drag the line between two rows or columns to resize, and use the "+" on that line to insert. Reason: nothing shows until pointed at, and every action is direct.
- (b) Grips and "+" stay visible while the table is selected.
- (c) No grips. Reorder and insert from the right-click menu, and resize by dragging the lines.

**What does the right-click menu offer on a cell, row or column?** (OQ-11)
- **(a) Recommended:** Insert row above, Insert row below, Insert column left, Insert column right, Delete row, Delete column, Merge cells, Unmerge cells, Clear content. Reason: the actions the issues name, as short verb + object labels per the Studio copy rule.
- (b) Everything in (a), plus "Distribute rows evenly" and "Distribute columns evenly".
- (c) The same entries as (a), with the four insert entries grouped in an "Insert…" submenu.

## Round 4: the tool (blocks T5)

**What do click and click-drag do with the Table tool?** (OQ-12)
- **(a) Recommended:** a click places the default table, and a click-drag sets its size (still 3×3). Reason: it works the same as the other shape tools.
- (b) A click places the table. A drag draws a grid, and the drag picks the number of rows and columns.
- (c) A click only.

**Do tables behave differently in Board, Design and Slides files?** (OQ-13)
- **(a) Recommended:**
  - Board and Design files get the same tool, defaults and editing.
  - Text size follows the file's text default: 24 px in Board files (`wb_text_defaults`, `crates/designos-app/src/board/caps.rs`) and 14 px in Design files.
  - Slides files do not offer the tool in 0.7.11.

  Reason: one behaviour to learn, with sizes that fit each canvas.
- (b) Identical everywhere, with 14 px text.
- (c) Board files only in 0.7.11. This contradicts #1231 AC-12 and is listed only for completeness.

**How do exports and Dev Mode treat tables?** (OQ-10)
- **(a) Recommended:**
  - PNG, SVG and PDF draw tables (in scope, T2).
  - Dev Mode code gives an HTML `<table>` for CSS and Tailwind, and nothing for native targets.
  - A table counts as a whiteboard item for the "Include whiteboard items" export toggle (#1226, v0.7.8).

  Reason: tables are content people hand off, and HTML has a native table.
- (b) Exports draw tables, Dev Mode skips them, and they count as whiteboard items.
- (c) Exports draw tables, they are not whiteboard items, and Dev Mode skips them.

**What if the Whiteboard toolbar group (#1218) is not merged when the Table tool is ready?** (OQ-14)
- **(a) Recommended:** wait for the group. The 0.7.11 tag already waits for tables. Reason: no throwaway toolbar layout.
- (b) Put one Table button next to Text and Comment now, and move it into the group later.
- (c) Ship Shift+T and the command palette only, with no toolbar button until the group lands.

## Controller rulings needed (not owner questions)

These are in README §Rulings:
- R1: forced single-line match arms in hot-file-queue files during T1.
- R2: the hot-file queue order blocks T3.
