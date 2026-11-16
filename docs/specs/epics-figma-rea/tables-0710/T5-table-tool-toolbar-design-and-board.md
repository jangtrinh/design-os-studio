# T5: Table tool, Shift+T, toolbar, Design and Board files

| Field | Value |
|---|---|
| Issues | #1231 (WB-16, AC-12 "Table tool creates and edits tables in Design and Board files"; test steps 1-3; edge case 50×50) |
| Size | 8 h |
| Tier | Claude Sonnet build (loads `es-designer` first), Opus review, curator visual pass |
| Depends on | T4 merged. WB-3 #1218 merged (toolbar "Whiteboard" group; Argon task `task_bedf6eba9a0a`). OQ-1 (default look) answered by the owner with mocks. OQ-2 as for T4. OQ-3, OQ-10, OQ-12, OQ-13 and OQ-14 are ruled (`controller-rulings.md`). |
| Runs in parallel with | T3a, T3b, T5b |
| Blocks | T6 |
| Branch | `jangtrinh/tables-t5-tool` |

## Goal

The Table tool exists in Design and Board files:
- Shift+T, or the toolbar button, then a click creates the default 3×3 table at the click point (#1231 step 1).
- Editing then works through T4.
- Export of a selection that holds a table works from the export panel (#1231 step 3).

## Behaviour

1. `Tool::Table` is added to `crates/designos-app/src/canvas/tools.rs:10-36`.
   - `label` = "Table".
   - `shortcut` = "Shift+T".
   - `from_key`: `(T, true) => Tool::Table` (Design map, `tools.rs` `from_key`).
   - `from_key_board`: `(T, true) => Tool::Table` (Board map).
   - FACT: neither map binds Shift+T today; only `(T, false)` exists, mapping to Text in both.
2. Shift+T is also used inside vector edit mode: `shell/tests/vector.rs:491` asserts "Shift+T kept square" (FACT). Its meaning there is not read; INFERENCE: a vector-edit transform key.
   - The tool key must not fire in vector edit or text edit modes.
   - A conflict test asserts both, and the keyboard-shortcuts dialog lists Table (#869 rule: the dialog lists every binding).
3. A capability method `wb_tables_enabled()` is added in `crates/designos-app/src/board/caps.rs`, returning true for Design and Board, like `wb_connectors_enabled` (`caps.rs:31-33`).
   - `tool_offered` (`caps.rs:43-45`) offers `Tool::Table` only where it is true.
   - Slides files: not offered (ruling OQ-13 (a)).
4. Click creates the default table (OQ-1 size and look, OQ-2 padding, ruling OQ-3: no special header row) using `table_ops::create_table`, then selects it.
   - Click-drag: ruling OQ-12 (a): a drag sets the table's size; rows and columns stay 3×3, tracks are scaled.
   - The tool returns to Move after one table, as other board tools do (INFERENCE from board tools; verify against Sticky's behaviour in `board/create.rs`).
5. Toolbar:
   - In Design files, the Table button joins WB-3's Whiteboard group.
   - In Board files, it joins the board toolbar (`crates/designos-app/src/board/toolbar.rs`).
   - The icon is original design:os art. No FigJam or Figma assets, names or icons (epic #1215 IP rule; WB-3 `figjam` grep gate).
6. Export: the export panel enables PNG, SVG and PDF for a selected table and for a frame that contains one. The pixels come from T2.

## Files T5 may modify

- `crates/designos-app/src/canvas/tools.rs`
- `crates/designos-app/src/board/caps.rs`
- `crates/designos-app/src/board/create.rs`
- `crates/designos-app/src/board/toolbar.rs`
- The WB-3 Whiteboard group file (its path is known once #1218 merges; [UNVERIFIED])
- The tool-press creation site in `crates/designos-app/src/canvas/interact.rs`. This is the creation branch only. It is serial after T4, so there is no conflict.
- The keyboard shortcuts dialog source (#869; [UNVERIFIED] path)
- The icon asset for Table (the folder the other board tool icons use)
- `crates/designos-app/src/shell/tests/table_tool.rs` (new)

## Acceptance criteria (red at base, green at head)

1. In a Design file, Shift+T then a click at (400, 300) creates one `Table`: 3×3 and 9 `TableCell`s, with size and look per OQ-1, at the click per OQ-12. It is selected and is one undo step. The same happens in a Board file (#1231 steps 1-2).
2. Typing after creation goes into a cell per OQ-1/OQ-4 (recommended: the first cell is in edit mode right after creation). Then T4's Tab, add and remove behaviours work. One shell test drives the full #1231 step 1: create, type, add a row, remove a column, resize. It runs in both file kinds.
3. Shift+T in vector edit mode keeps its existing behaviour: the existing tests at `shell/tests/vector.rs:491-576` stay green, and no table is created. Shift+T while editing text types "T".
4. The Table button exists in the Design Whiteboard group and the Board toolbar, and acts. `python3 scripts/zero_fake_ui_gate.py` passes, and the WB-3 `figjam` grep gate passes.
5. 50×50 (#1231 edge case): create a 50×50 table with `table_ops` in a shell test, press Tab 10 times, then delete row 25. Each step completes within the test with no panic. The perf budget is T6.
6. Export (#1231 step 3): with a table selected, the export panel's PNG and SVG produce files. The SVG contains the cell texts, and the PNG has the cell fill at a cell centre.

## Test seam

Shell tests: key events, clicks, and export panel calls.

## Non-goals

- MCP op (T5b).
- Table templates.
- CSV paste (not requested).
- The WB-11 export toggle (#1226).

## Gates

Same as T4:
- `cargo fmt`
- `clippy -D warnings`
- `cargo test -p designos-app`
- `zero_fake_ui_gate.py`
- `design_os_self_evolve.py`

## Visual evidence ("Figma first, gates after")

1. 1440 captures in a Design file and a Board file:
   - the toolbar with the Table button;
   - Shift+T cursor state;
   - a fresh 3×3 table;
   - the same table after the full #1231 step 1.
2. The exported PNG, and the SVG opened in a browser.
3. Captures go to the controller ("screens ready") for Figma, then gates.

## Risks

| Risk | L×I | Mitigation |
|---|---|---|
| WB-3 slips, so the Design toolbar has no group | M×H | Critical-path item in README. If WB-3 is not merged by the time T4 merges, T5 waits (ruling OQ-14 (a): wait for #1218). |
| Shift+T conflicts | M×M | AC-3 |

## Rollback

Revert. The tool disappears. Tables in files still render and edit (T2, T4).
