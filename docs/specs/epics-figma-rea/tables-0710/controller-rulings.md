# Tables (v0.7.11): controller rulings, 2026-11-07 01:05

Owner may reverse any of these. Each follows from a standing owner rule, so it is not asked again.

| Question | Ruling | Standing rule it follows |
|---|---|---|
| OQ-15 entering a cell | (a) click selects table, second click a cell, double-click/Enter/typing edits, Esc steps up | Today's behaviour wins: frames and groups are entered this way |
| OQ-4 Tab in last cell | (a) adds a row and keeps typing, one undo step | Figma/FigJam parity |
| OQ-5 text under a merge | (a) appended to the merged cell, one line per covered cell | Nothing vanishes silently |
| OQ-8 how to merge | (a) right-click menu + floating bar, no new shortcut | Keys belong to WB-4; no unasked shortcut |
| OQ-6 text taller than row | (a) row grows, never below the user's height | FigJam parity; no hidden text |
| OQ-3 header row | (a) no special header | No scope beyond the issues |
| OQ-11 context menu | (a) the nine verb + object entries | Studio copy rule (actions <= 3 words) |
| OQ-12 click / drag | (a) click = default table, drag = size | Same as the other shape tools |
| OQ-13 file kinds | Board and Design same tool and defaults; no Slides. Re-ruled 2026-11-07 01:10: new-table text = FigJam's measured default, Inter Medium 16 px, in both file kinds (was 24/14 px) | #1231 names Design and Board only; Figma parity (T0 found FigJam's default is Inter Medium 16) |
| OQ-10 export / Dev Mode | (b) PNG/SVG/PDF draw tables; Dev Mode skips them (follow-up issue for an HTML table); counts as a whiteboard item | No scope beyond the issues |
| OQ-14 toolbar group not merged | (a) wait for #1218 | The tag already waits for all items |
| R1 single-line match arms in hot-file-queue files during T1 | Allowed | Forced by a new node type; no logic change |
| R2 hot-file queue | #317 moves ahead of #224 (v0.17, not started) | Owner put Tables in this release |

Deferred to the owner with mocks once T2 renders tables (owner rule: show screenshots before asking a visual question): OQ-1 default look, OQ-2 padding and alignment, OQ-7 table handles, OQ-9 row/column grips. T1/T1b use the recommended values as constants until then.


Also 2026-11-07 01:10: P2 (`rea` on Figma.app, figma.com login) is not run: T0 answered U1/U2/U5/U6/U8/U11 from the real file; the rest wait for fixture F2. `.jam` importing as Design kind is bug #1694.

## 2026-11-07 rulings from the T1 build
- R-T1-1: MCP metadata uses the existing lower-case tags (`<table rows cols>`, `<table-cell row col>`), not `<TABLE>`; consistency with the rest of `designos_get_metadata` wins.
- R-T1-2: commit paths outside `History` (`Tx::cmd` in designos-app/src/edit.rs, live-patch replay) get the kind checks but not the per-step position check. T1b and T4 MUST call `check_table_positions` at their commit points and test a broken-position rollback through those paths.
- R-T1-3: the three designos-app clippy warnings (agent_permissions.rs:1176/1194, inspector_var_binds.rs:1182) are pre-existing; CI runs clippy without -D warnings. Not a T-step gate.
- R-T1-4 (review B5): batch atomicity for table refusals stays in T1. A user step that contains a refused command applies nothing: the History entry point rolls back every command already applied in that step and keeps the undo stack unchanged. Grouping a cell = refused as a whole, with no stray group.
- R-T1-5: load repair moves a non-cell child (rectangle, nested table, anything) out of a table to the table's parent, directly after the table, and counts it (`children_moved_out`). Nothing is dropped.
- R-T1-6: MCP metadata shows merge spans (`rowspan`/`colspan` attributes on a merged `table-cell`), so agents need not infer merges from sizes.
- R-T1-7: load re-lays out a table only when repair changed it. Saved layout comes from the same pure function, so it is accepted; the T1 spec text is superseded on this point.
- R-T1-8: every export drops cell text until T3a lands (.fig writes TABLE + TABLE_CELL without text; REST omits cells; .pen drops text). T3a is a HARD gate for the v0.7.11 tag. Nothing user-facing creates tables before T4/T5, so T1 may merge first.
- R-T1-5 amended: a child moved out of a table by load repair keeps its world position (the table's transform is composed into the child's).
- R-T1-9: T1b must test N4 (an undo refused by the position check through the auto-layout path also undoes the layout pass).

## R-T1-10 (2026-11-07 12:30) — home of the auto-layout rollback fix
The refused-undo rollback fix in `history.rs` (`guarded`, T1b commit 0fac9d4c3) is a T1 defect found by the R-T1-9 test. It stays a separate commit at the base of the T1b PR, not folded into #1742, because #1742 is already queued in the merge train. Tables have no UI entry point before T4, so no user can hit the gap between the two merges. T1b must merge before the v0.7.11 tag (same gate as T3a).

## R-T1-11 (2026-11-07 12:30) — open T1b choices
- New row/column cells copy the neighbouring track's fills: accepted.
- `unmerge` gives the split cells the anchor cell's fill (spec said fresh empty cells): accepted, so an unmerge never changes what the table looks like. Reversible; the spec is amended to match.
- A merge that fully contains another merge absorbs it; a partial overlap is refused: accepted.
- Deleting a merge's anchor row moves the anchor cell (id + text) to the next row of the block: accepted.
- `NoChange` error for no-op ops: accepted. T4 treats it as "do nothing, record no undo step".
- `Tx::cmd` and live-patch replay route through `apply_checked`: T4 scope (extends R-T1-2).

## Owner ruling 2026-11-07 17:10 (AskUserQuestion)
- OQ-16 cells in Layers: (a) the table is one layer row; cells reached on canvas and over MCP. Controller ruling (no visual), 2026-11-07 17:12: a 50x50 table would add 2,500 rows.
- OQ-1/2/7/9: owner chose "make mocks, I pick". Mocks being built (designer agent) into evidence/oq-mocks/.

## Owner rulings 2026-11-07 17:35 (AskUserQuestion, after the mocks in evidence/oq-mocks/)
| Question | Owner choice | Values in force |
|---|---|---|
| OQ-1 new table look | (a) dark, Studio tokens | From theme.rs (these replace the hex in owner-questions.md): fill #1E1E22, lines 1 px #27272A, text white at 70% (#B3B3B5 solid), Inter Medium 16 px (OQ-13), 3x3, columns 160, rows 44 |
| OQ-2 cell padding | (a) 12 left/right, 8 top/bottom, text top left | TABLE_CELL_PADDING (12, 8) already in T1 #1742 |
| OQ-7 table handles | (a) 8 handles scale every row and column proportionally, text size kept | T4 |
| OQ-9 row/column handles | (a) grip on hover (left edge rows, top edge columns), + on the nearest line, drag line to resize | Grip 12x20, plus 16, each in a 24 px pointer area; in (a) only one grip and one + show at a time, so the 44 px row spacing issue of (b) does not arise |
All table owner questions are now answered (OQ-1..16). T4/T5 are unblocked on rulings; T3b still waits for fixture F2.
