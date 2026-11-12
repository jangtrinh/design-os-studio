# T1: table doc model, layout, serde, load repair, tree guards, MCP read

| Field | Value |
|---|---|
| Issues | #317 (model half) |
| Size | 10 h (was 9 h; +1 h for the tree guards added 2026-11-07) |
| Tier | Claude Sonnet build, Opus review |
| Depends on | WB-2 #1217 merged or rebased onto (it owns `crates/designos-doc/**` load repair right now, per `brief-agy-wb2-board-kinds-on-design-pages.txt:18`). Ruling R1 is granted (`controller-rulings.md`): single-line forced arms in hot-file-queue files are allowed. |
| Blocks | T1b, T2, T3, T5b (and through them T4, T5, T6) |
| Branch | `jangtrinh/tables-t1-model` from `origin/main` |

## Goal

The document can hold a table:
- `NodeKind::Table` holds ordered rows and columns with stable ids, sizes and merges.
- `NodeKind::TableCell` children hold the text.
- One pure function lays the cells out.

Tables save and load in `.designos`, a broken table is repaired on load, and agents can read tables over MCP.

## Model decision re-checked after T0 (2026-11-07)

T0 showed that Figma writes no cell records: a table is one `TABLE` record, and a cell is the pair `(rowId, colId)` addressing overrides inside it (`plans/reports/tables-t0-probe.md`). So the file format no longer argues for cell nodes. The choice was re-decided on its own merits.

| Criterion | Cells as child nodes | Cells as data inside `TableProps` |
|---|---|---|
| Render and export reuse | Each cell lowers itself through `board::lower`; the table body lists real child ids (T2). | The table must emit rows × cols synthetic rects and texts. FACT: `synth_id` keeps 4 slot bits, 16 children per item (`crates/designos-render/src/board.rs:27-30`), so the shared id scheme would have to widen for every board item. |
| Text editing, IME, Find | Unchanged. FACT: `begin_text_edit(app, id: NodeId)` reads `kind.any_text()` (`crates/designos-app/src/canvas/textsession.rs:80-97`), Find matches compare `m.node == id` (`:92`), and 33 files call `any_text` / `board_text`. | Every one of those paths needs a sub-node cell address (node id + row + col): a cross-crate change to the text session, IME, Find, Cmd+K and spellcheck. |
| Undo granularity | One cell edit = one `Command::Update` of one small node. | FACT: `Command::Update { node: Box<Node> }` replaces a whole node (`crates/designos-doc/src/history.rs:24-26`). One cell commit would store the whole table twice (do + inverse): 2,500 `TextProps` per keystroke commit on a 50×50 table. INFERENCE: the live edit log ships it too, because `live_patch/record.rs:77-81` diffs top-level node fields and `kind` would be one field. |
| 50×50 scale | 2,501 nodes; documents already hold 1M nodes (CLAUDE.md benchmark). `SceneIndex` and culling work per cell. | One node, but the whole table lowers on every compile even when 4 cells are visible. |
| MCP addressing | `get_node`, `query_xpath //TABLE_CELL` and Find work per cell with no new code; T5b adds index addressing on top. | Every read tool needs a virtual cell layer. |
| OQ-16 (owner: one Layers row, no children) | Costs a Layers filter (T4). | Free. |
| Tree invariants | Generic edits (insert, reparent, delete, paste, MCP) can break a table. Costs the guards below. | Free by construction. |
| `.fig` mapping | The file boundary synthesizes cell nodes on import and collects them on export (T3a). | Maps 1:1. |

**Decision: keep cells as real child nodes.** The text-session reuse and the per-cell undo size decide it (FACT, above). OQ-16 is a display rule, not a model rule; the owner's option text itself says agents still see the cells over MCP. The costs are paid explicitly: the tree guards (decision 8), the Layers filter (T4), and the import/export synthesis (T3a).

What this changes:
- T1: decision 1's reason is rewritten (below); decision 8 (tree guards) and AC-8 are new.
- T1b: every op must leave each table whole at the end of its step (the guard rolls back otherwise); no other change.
- T2: none in approach; facts from the real file feed the OQ-1 / OQ-2 mocks.
- T4: the Layers filter (OQ-16, now owner-ruled) gets exact rules; layer commands on a cell selection are covered by AC-9.
- T5b: generic MCP node tools refuse cell-breaking edits with a next-action message.

## Design decisions (locked here; T0 answers cannot change them)

1. **Cells are real child nodes**, not synthetic lowered children.
   - FACT: `designos_render::board::synth_id` packs the slot into 4 bits (`crates/designos-render/src/board.rs:28-30`, `slot & 15`), so one lowered item has at most 16 synthetic children. A 3×3 table needs more, and the 50×50 edge case (#1231) needs 2,500.
   - FACT: the text session, Find and undo are keyed by `NodeId` (see the re-check table above).
   - The file format does NOT dictate this (T0: Figma writes no `TABLE_CELL` record). `TableCellProps.row` / `.col` map 1:1 to `guidPath[1]` / `guidPath[2]` of the file's cell overrides, and the file boundary synthesizes the nodes (T3a).
2. **Tracks have stable ids that mirror Kiwi GUIDs.**
   - Kiwi `GUID` is `{ sessionID: Uint, localID: Uint }` (`figma_schema.txt:463-465`).
   - Rows and columns are keyed by GUID (`figma_schema.txt:1771-1811`). T0 FACT: track ids are ordinary ids in the file's id space (`1:4`..`1:9` beside table `1:11`).
   - Index-only tracks would lose identity on reorder and merge, and T3 would become lossy.
3. **Cell geometry is stored on the cell node.**
   - `transform` and `size` are recomputed by one function, `table::relayout`, which every table edit includes in its undo batch.
   - `SceneIndex` (`crates/designos-render/src/index.rs:939`), culling and `canvas/hit.rs` then work with no table special case.
4. **No covered-cell nodes.** A merge removes the cells it covers; an unmerge inserts fresh empty ones.
   - The model invariant is that every cell node is either an unmerged cell or a merge anchor.
   - Hit testing and culling never meet an invisible cell.
   - This is a model choice, still untested against Figma (U3: no merge in the first fixture). Figma has no cell records to keep; the open question is whether covered positions keep text overrides or derived frame entries. T3b handles whatever F2 shows at the file boundary. The model does not change.
5. **Cell text is `TextProps` reached through `NodeKind::board_text` / `any_text_mut`** (`crates/designos-doc/src/node.rs:575-605`).
   - The text editor (`crates/designos-app/src/canvas/textsession.rs:80` at `main@ec47da26d`, IME at `:664-665`), Cmd+K and the sticky bar then work for cells unchanged.
6. **Containment.** `can_hold_children()` is true for `Table`, so tree edits and undo of delete accept it as a parent. `is_container()` stays false for `Table`.
   - A click selects the whole table (`canvas/hit.rs:239` descends only into `is_container` kinds), and T4 adds the cell-level targeting.
   - Auto layout and clipping code ignore it, as they do for `Unknown`.
7. **`is_board_item()` includes `Table` and `TableCell`.** Both renderers and the exporters then route them through `board::lower` (T2). This is FACT for the call sites:
   - `crates/designos-render/src/lib.rs:494`
   - `crates/designos-gpu/src/compile.rs:891`
   - `crates/designos-render/src/geom.rs:499`
   - `crates/designos-export/src/svg.rs:42`
   - `crates/designos-export/src/pdf.rs:90`
8. **Tree guards keep a table whole while the app runs** (new 2026-11-07, the cost of decision 1).
   - FACT: `Command::Insert` checks only `can_hold_children()` (`crates/designos-doc/src/history.rs:86` at `main@ec47da26d`), and `Remove` / `Reparent` check nothing table-specific (`:107`, `:113-119`). So MCP `create_node(parent_id = table)`, batch move/delete, paste and duplicate could break a table between loads. Load repair only fixes it on the next load.
   - FACT: `Command::Batch` is not atomic. It applies sub-commands in order and returns at the first error, so the sub-commands already applied stay applied and no inverse is returned (`history.rs:206-213`). A batch's inverse runs in reverse order (`:211`). So a per-command "position still live" check would refuse the undo of `insert_rows` / `unmerge` (their undo removes cells before the table update that drops the positions). The guard therefore has two parts:
     - **Kind checks, per command, in `Command::apply`.** These do not depend on order and refuse before mutating:
       - `Insert` under a `Table` accepts only `TableCell` nodes, and a `TableCell` can be inserted only under a `Table`.
       - `Reparent` refuses moving a `TableCell` to a non-`Table` parent, and moving any non-cell node into a `Table`.
     - **Position check, once per undo step, in `History`.** It runs after the whole step, at every entry point: `exec_labeled`, `exec_agent`, `exec_user`, `undo`, `redo` (`history.rs:352-381`, `:470-489`).
       - For each table the step touched, every live uncovered `(row, col)` has exactly one cell, and no cell sits at a dead or covered position. Collect the table ids before applying, because a removed cell's parent is gone afterwards.
       - On failure, apply the step's returned inverse (roll back), push nothing, and return the error. That makes the refusal atomic for this check.
       - Removing the `Table` itself removes its subtree, as today.
       - Callers that call `Command::apply` directly, outside `History`, are listed by the builder (grep). They route through `History` or call the same check.
     - The refusal is a new `DocError` variant (name the builder's choice) whose message names the next action, for example "Use a table operation to delete rows or columns".
   - Cell geometry is derived. Generic geometry edits of a cell (MCP `designos_update_node` x/y/width/height/rotation, canvas nudge) are refused at their callers (T5b for MCP, T4 for the canvas), because `Update` cannot tell `relayout` from a user edit.

## Model (exact shape; names may be adjusted only with a note in the PR)

```rust
// crates/designos-doc/src/table.rs (new)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TrackId { pub session: u32, pub local: u32 }          // Kiwi GUID mirror
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableTrack { pub id: TrackId, pub size: f32 }           // order = Vec order
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableMerge { pub row: TrackId, pub col: TrackId, pub row_span: u32, pub col_span: u32 } // anchor = top-left
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableProps {
    pub rows: Vec<TableTrack>,
    pub cols: Vec<TableTrack>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")] pub merges: Vec<TableMerge>,
    pub next_track: u32,                                              // allocator for native TrackIds (session 0)
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableCellProps { pub row: TrackId, pub col: TrackId, pub text: TextProps }
```

Changes in `node.rs`:
- Add `NodeKind::Table(TableProps)` and `NodeKind::TableCell(TableCellProps)`.
- `default_name` returns "Table" and "Cell".
- `board_text` / `board_text_mut` return the cell text.
- `is_board_item` and `can_hold_children` change as described above.

Styling home:
- Table background = the table node's `fills`.
- Grid line colour and weight = the table node's `stroke`.
- Cell background = the cell node's `fills`.
- Cell padding is a `pub const TABLE_CELL_PADDING` in `table.rs`; its value comes from owner answer OQ-2.
- Until the owner answers, use the recommended option and mark it in the PR.
- How Figma stores the same look (T0 FACT, one file; T3a maps it, T1 adds no field for it):
  - grid lines are the cell stroke on shared template override `40000000:2` (black 20 %, weight 1, INSIDE), drawn with collapsed borders → T3a maps it to the table node's `stroke`;
  - the default cell fill is the fill on that same template (white) → T3a writes it into each imported cell's `fills`;
  - the cell text style is template `40000000:3` (Inter Medium 16, line height 1.5, letter spacing −1.1 %, left/top) → each imported cell's `TextProps`. Imported tables keep these values, and new Studio tables use the same Inter Medium 16 (OQ-13, re-ruled 2026-11-07);
  - the `TABLE` itself carries a fill (black, alpha 0.098) and `cornerRadius` 12, and the four outer cell corners are rounded 12. The table node's existing `fills` and corner radius hold them.
  - default track sizes in that file: columns 192, rows 64, text inset 20 px on every side (cell 192 × 64, text box 152 × 24 at 20, 20). These are inputs for the OQ-1 / OQ-2 mocks, not T1 constants.

`table::relayout(doc, table_id) -> Vec<Command>`:
- Pure. It returns one `Command::Update` (`crates/designos-doc/src/history.rs:24`) for each cell whose transform or size changes, plus one for the table's own size (sum of tracks).
- Merge anchors span their tracks.
- Row auto-grow follows owner answer OQ-6. With the recommended "grow", a row's height is max(its stored size, the tallest single-row cell's text height + 2 × padding). Text height comes from `text_measure::intrinsic_text_size` (`crates/designos-doc/src/text_measure.rs:91`) through the installed `layout_measurer()` (`:52`).
- With no measurer installed (headless doc tests), stored sizes are used.
- Auto-grow changes only the laid-out geometry (cell `size`, table `size`). `TableTrack.size` keeps the user's height, so `.fig` export (T3a) writes that value and a round trip never ratchets rows taller.

`table::cell_at(doc, table_id, local: Vec2) -> Option<NodeId>`: the anchor cell under a table-local point. T4 uses it for hit testing.

## Serde, compatibility and migration

- `NodeKind` is an externally tagged serde enum (`node.rs:487-488`), so the new variants add new tags only. Files without tables are byte-identical: the AC compares a saved fixture before and after.
- Older Studio builds cannot open a file that contains a table (unknown variant). This is INFERENCE from serde behaviour and the same as when `Stamp` shipped. T6 states it in release notes.
- `doc_hash` streams through serde (`crates/designos-app/src/home/session.rs:279`), so tables are hashed with no change. The invariant "never `to_json` on the UI thread" is untouched.
- Saved `.designos` files that hold `Unknown { type_name: "TABLE" }` from earlier `.fig` imports are NOT changed by T1. They keep loading and round-tripping as today (`fig_roundtrip.rs:2044-2054` stays green). Upgrading them to `Table` is T3a's job; T0 has given it the binding rule (track ids in the `guidPath` of the TABLE's overrides).

## Load repair (`crates/designos-doc/src/load_repair.rs`, `validate_and_repair` at `:84`)

Add one table pass that reports each fix in `RepairReport`. Law 1: nothing vanishes silently.
1. A `TableCell` whose parent is not a `Table` becomes `NodeKind::Text` with the same text (kept, counted).
2. A table with zero rows or zero columns gets 1 row or column of the default size (counted).
3. Duplicate `(row, col)` cells: the first in child order is kept and the rest are removed, with their text appended to the kept cell's text, newline-separated (counted).
4. A cell whose row or column id is not in the table: removed only if its text is empty, otherwise moved to the first free position (counted). A missing `(row, col)` that no merge covers gets a fresh empty cell (counted).
5. Merges:
   - A merge that overlaps another merge or runs past the table edge is dropped or clamped (counted).
   - Cells covered by a valid merge are removed, with their text appended to the anchor (counted).
6. Non-finite or ≤ 0 track sizes are clamped to the minimum size (owner OQ-1 default, counted).
7. After repair, `relayout` runs so stored geometry matches the tracks.

## Forced match arms (step 0)

- Add the two variants, then run `cargo check --workspace --all-targets` and list every `non-exhaustive patterns` error in the PR.
- Expected sites (INFERENCE from the 20 files that name `NodeKind::Stamp` on `origin/main`): `node.rs`, `openpencil.rs:26-30` (type names `TABLE` / `TABLE_CELL`), `branch_diff.rs:825`, `designos-render/src/geom.rs:499`, `designos-fig/src/fig_export.rs:928-935`, `designos-fig/src/export.rs:509`, `designos-fig/src/pen/export.rs:153`, `designos-app/src/panels/layers.rs`, `designos-app/src/canvas/hit.rs`, `designos-export/src/agent_context.rs`.

Rule for a forced arm outside `designos-doc`:
- Use the smallest arm that keeps today's behaviour for an `Unknown { type_name: "TABLE" / "TABLE_CELL" }` node.
- In `fig_export.rs` that is `("TABLE", false)` / `("TABLE_CELL", false)`. This is interim only: Figma never writes a `TABLE_CELL` record (T0), so T3a replaces both arms before the tag.
- In `pen/export.rs` that is the same path as `Unknown`. No new feature work lands outside doc and MCP here.
- `fig_export.rs` and `export.rs` are in the hot-file queue. T1's arms there are single lines, allowed by ruling R1 (granted 2026-11-07).

## Files T1 may modify

- `crates/designos-doc/src/node.rs`, `table.rs` (new), `table_tests.rs` (new), `lib.rs` (mod and re-exports), `load_repair.rs`, `load_repair_tests.rs`, `openpencil.rs`, `branch_diff.rs`
- `crates/designos-doc/src/history.rs`: the decision 8 guard (kind checks in the `Command::apply` Insert and Reparent arms; the per-step position check and rollback at the `History` entry points) and the new `DocError` variant; guard tests in `table_tests.rs`
- `crates/designos-mcp/src/tools/metadata.rs`, MCP read tests (new file `crates/designos-mcp/tests/table_read.rs`)
- The forced single-line arms found in step 0 (listed in the PR), nothing else in those files

## Acceptance criteria (red at base, green at head)

1. `cargo test -p designos-doc table::` passes:
   - `relayout` on a 3×3 table with column widths [120, 240, 90] and row heights [40, 40, 64] puts cell (r1, c1) at x=120, y=40, size 240×40, and cell (r2, c2) at x=360, y=80, size 90×64. The table size is 450×144.
   - With a 2×2 merge at (r0, c0), the anchor is 360×80, and cells (r0, c1), (r1, c0) and (r1, c1) do not exist.
2. The serde round trip is equal: `Document::from_json(doc.to_json())` == `doc` for a document with a 3×3 table holding a merge, Vietnamese cell text and a cell fill. A table-free fixture's saved JSON bytes are unchanged against base (`git stash` comparison script in the PR).
3. Load repair: one test per rule 1-6, each asserting the repaired tree and the `RepairReport` count. A document with a cell outside any table loads with the text kept as a Text node.
4. `cell_at` returns the anchor for every point inside a merged block, and `None` outside the table.
5. `board_text(&NodeKind::TableCell(..))` returns the cell text. `is_board_item()` is true for both kinds. `can_hold_children()` is true and `is_container()` false for `Table`.
6. MCP read, in `crates/designos-mcp/tests/table_read.rs`:
   - `designos_get_metadata` on a page with a table emits `<TABLE … rows="3" cols="3">` with `<TABLE_CELL … row="0" col="0">` children (zero-based display indices).
   - `designos_query_xpath` with `//TABLE_CELL` returns 9 rows for an unmerged 3×3 table.
   - `designos_get_node` on a cell returns its `TableCell` JSON with text.
7. `cargo check --workspace --all-targets` passes with the forced arms. The full test suites (integration tests included) of every crate that step 0 touched stay green; see Gates. Unit tests alone are not enough here, because the forced arms land in files whose coverage lives in `tests/`: `fig_roundtrip.rs`, `fig_export_field_shapes.rs`, `figjam.rs` and the GPU parity suite.
8. Tree guards (decision 8). Kind checks go through `Command::apply`; position checks go through `History::exec_user`. Each refusal leaves the document and both stacks unchanged:
   - inserting a `Rectangle` under a `Table` fails; inserting a `TableCell` under a `Frame` fails;
   - reparenting a cell to the page fails; reparenting a rectangle into a table fails;
   - removing one cell of an unmerged 3×3 table fails with the next-action message, and the document is unchanged (rolled back);
   - an `insert_rows`-shaped batch (update the table to add a row, then insert its cells) succeeds; its `undo` and `redo` both succeed;
   - a `delete_rows`-shaped batch (remove the row's cells, then update the table to drop the row, in either order) succeeds; its `undo` and `redo` both succeed;
   - removing the table removes all its cells, and undo restores them.

## Test seam

- Pure doc unit tests (`table_tests.rs`, `load_repair_tests.rs`).
- MCP stdio tests through the existing MCP test harness (find the one the other `crates/designos-mcp/tests/*.rs` use; [UNVERIFIED] name).

## Non-goals

- Drawing tables (T2).
- `.fig` mapping (T3).
- Edit ops (T1b).
- UI (T4, T5).
- Upgrading legacy `Unknown{TABLE}` (T3).
- Header-row semantics. Ruled OQ-3 (a): no special header. T0 saw no header flag on the `TABLE` record (U10, one file).

## Gates

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p designos-doc -p designos-mcp -p designos-fig -p designos-render -p designos-gpu -p designos-export
cargo test -p designos-app        # when step 0 lands an arm in designos-app (expected: panels/layers.rs, canvas/hit.rs)
cargo test --workspace --lib
```
`cargo test -p designos-gpu` is required because `designos-render/src/geom.rs` gets an arm: CPU/GPU pixel parity must stay bit-exact.

## Visual evidence

- A terminal capture of the AC-1 and AC-6 test output.
- A capture of `designos_get_metadata` XML for a 3×3 table.
- No UI changes, so no zero-fake-UI gate.

## Risks

| Risk | L×I | Mitigation |
|---|---|---|
| WB-2 (Argon) edits `load_repair.rs` / `node.rs` at the same time | H×M | Start T1 after WB-2 merges, or rebase onto its PR head. Keep the table repair in a new function called from one line. |
| Forced arms touch hot-file-queue files | H×M | Single-line arms only; ruling R1 (granted) |
| A generic edit path (paste, duplicate, MCP move) breaks a table while the app runs | M×H | Decision 8 guard (kind checks in `Command::apply`, position check per `History` step), AC-8; T1b's random-op fuzz asserts zero repairs |
| Row auto-grow needs text measurement in doc | M×M | Use the installed measurer, with a documented fallback to stored sizes |

## Rollback

Revert the squash commit. No file written by base contains the new tags, so base reads every pre-T1 file. A file saved with a table after T1 needs T1 to open; T6 notes it.
