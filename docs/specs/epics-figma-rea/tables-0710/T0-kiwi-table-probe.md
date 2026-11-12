# T0: Kiwi table probe and fixture

| Field | Value |
|---|---|
| Issues | #317 ("REA needs: table cell → row/col binding field") |
| Size | 3 h builder + about 15 min owner-manual (fixture) |
| Tier | Claude Sonnet (probe), owner (fixture) |
| Depends on | nothing; runs in parallel with T1 |
| Blocks | T3b only (since 2026-11-07). T3a starts on the first fixture's results. T1, T1b, T2, T4 and T5 do not wait. |
| Output | `plans/reports/tables-t0-probe.md` (the real report path) plus the committed fixtures |
| Status 2026-11-07 | First run DONE on the owner's 2x4 board (results at the end). Second run (re-probe of F2) open: it is T3b's gate. |

## Goal

Replace every UNKNOWN below with evidence from a real FigJam file and from the Figma app, so T3 maps fields by fact, not guess.

## What the repo already knows (FACT, schema text at `origin/main@d22dad970`)

| Item | Schema text | Where |
|---|---|---|
| `NodeType.TABLE = 29`, `NodeType.TABLE_CELL = 30` | enum entries | `crates/designos-fig/src/figma_schema.txt:76-77`; `crates/designos-fig/tests/data/figma-real-schema.txt:69-70` |
| `NodeChange.tableRowPositions = 308 : TableRowColumnPositionMap` | field | `figma_schema.txt:1535`; real `:2073` |
| `NodeChange.tableColumnPositions = 309 : TableRowColumnPositionMap` | field | `figma_schema.txt:1536`; real `:2074` |
| `NodeChange.tableRowHeights = 310 : TableRowColumnSizeMap` | field | `figma_schema.txt:1537`; real `:2075` |
| `NodeChange.tableColumnWidths = 311 : TableRowColumnSizeMap` | field | `figma_schema.txt:1538`; real `:2076` |
| `NodeChange.tableMergedCells = 538 : TableMergedCellMap` | field | `figma_schema.txt:1539`; real `:2077` |
| `TableRowColumnPositionMapEntry { id = 1 : GUID, position = 2 : String }` | message | `figma_schema.txt:1778-1781` |
| `TableRowColumnSizeMapEntry { id = 1 : GUID, size = 2 : Float }` | message | `figma_schema.txt:1808-1811` |
| `TableMergedCellMapEntry { rowId = 1 : GUID, colId = 2 : GUID, rowSpan = 3 : Int, colSpan = 4 : Int }` | message | `figma_schema.txt:1771-1777` |
| `TextSelection.tableCellRowId = 6 : GUID`, `tableCellColId = 7 : GUID` | on `TextSelection`, NOT on `NodeChange` | `figma_schema.txt:2289-2290` |

What the field names and shapes say, read literally (INFERENCE, not verified):
- Rows and columns have GUID identities. Their order comes from a `position` string per id, and their size from a float per id.
- A merge is keyed by one `(rowId, colId)` pair plus spans. The pair is probably the top-left anchor; see U4.
- Multiplayer text cursors address a cell by `(row GUID, column GUID)`. This suggests a cell's identity is the pair of track ids, but it does not say which `NodeChange` field stores that pair (U1).

What the repo does with these today (FACT):
- The importer has no `TABLE` mapping. `TABLE` / `TABLE_CELL` become `NodeKind::Unknown { type_name }`; `crates/designos-fig/src/rest_import.rs:1310` is the fallback and `crates/designos-doc/src/node.rs:559-566` documents tables as kept children.
- The X1 passthrough (#174) keeps fields 308-311 and 538 as JSON strings in `Node::fig_metadata` (`crates/designos-doc/src/node.rs:158`). `crates/designos-fig/tests/fig_roundtrip.rs:1919-1945` builds the table and `:2044-2051` asserts that the fields survive.
- That fixture is authored in the repo (synthetic NodeChanges), not Figma output. Its values (for example `gridChildVerticalAlign` on a cell, `fig_roundtrip.rs:1944`) are NOT evidence of what Figma writes.
- `.fig` export writes an `Unknown` node back under its own type name when the schema has that variant (`crates/designos-fig/src/fig_export.rs:935`).

Out of scope, so builders do not confuse them (FACT, real schema only):
- `TABLE = 14`, `TABLE_ROW = 15` and `TABLE_CELL = 16` sit in a text-block enum (`figma-real-schema.txt:3112-3114`).
- `TABLE = 62` and `TABLE_CELL = 94` sit in a role-like enum (`:4236`, `:4302`).
- `specBlockTableAlignment = 562`, `specBlockTableRowHeights = 578` and `specBlockTableColumnWidths = 579` (`:2336-2342`) belong to a different feature.
- None of these is the FigJam table node.

## UNKNOWN (never guess these in code; each names its probe)

| Id | Unknown | Resolved by |
|---|---|---|
| U1 | Which `NodeChange` field(s) bind a `TABLE_CELL` to its row and column. No `table*` field exists on `NodeChange` besides 308-311 and 538. | P1, P2 |
| U2 | Encoding and ordering of `position` strings (the same scheme as `parentIndex.position`? byte-wise compare?) | P1 (order in a file with a reordered row), P2 |
| U3 | Whether merged-away (covered) cells exist as `TABLE_CELL` nodes, and what happens to their text on merge | P1, P3 |
| U4 | Whether a merge entry's `(rowId, colId)` is the top-left anchor | P1 |
| U5 | Where cell text lives: `textData` on the `TABLE_CELL` itself, or a child `TEXT` node | P1 |
| U6 | Size units, and what a missing size entry means (a default size? its value?) | P1, P2 |
| U7 | Which fields carry the cell background, the table border colour and weight, and the text style | P1 |
| U8 | Whether the `TABLE` node's own `size` equals the sum of track sizes | P1 |
| U9 | Whether Figma (Design editor) and FigJam open a `.fig` whose `TABLE` was written by Studio, including from a Design file | P4 (after T3) |
| U10 | Header-row or other per-table flags, if any | P1 (diff of all keys on `TABLE`) |
| U11 | Whether row and column GUIDs share the node GUID namespace (unique per file) or are scoped per table. Native Studio ids could collide with node GUIDs (often `0:0` / `0:1` for document and page), and a duplicated table copies its track ids. | P1: compare every track GUID in F1 against every node GUID and against the other tables' track GUIDs; P2 for the rule |

Hint, external and not Kiwi evidence: Figma's public plugin API for FigJam exposes `TableNode` (`numRows`, `numColumns`, `cellAt`) and `TableCellNode` (`rowIndex`, `columnIndex`). It shapes what to look for. It does not say how the file stores it.

## Fixture F1 (owner-manual, about 15 min) — SUPERSEDED 2026-11-07

The owner authored a 2x4 board instead (golden `crates/designos-fig/tests/fixtures/figjam-table-2x4.jam`). What F1 asked for and the board does not hold (merge, reordered row, styled cell, Vietnamese) moved to fixture F2, specified in [T3b](T3b-fig-table-merges-second-fixture.md). The F1 text below is kept for the record; `figjam_tables.jam` is never created.

This leg is FLAGGED owner-manual: only the owner can author it in FigJam. Builders never author it, and never substitute client files.
- Do NOT use the `.jam` files found in iCloud (`…/HDBank/Process/*.jam`). They are client material and must never be committed, and nobody has checked that they hold tables.

Contents, in one new FigJam file `tables-fixture`, saved with "Save local copy":
1. Table A, 3×3:
   - Column widths changed to roughly 120 / 240 / 90 and one row made taller.
   - Text in every cell. Row 1 holds "Name", "Giá", "Qty". One cell holds Vietnamese with diacritics ("Bảng giá tháng 11") and one is empty.
   - One cell with a non-default background colour.
2. Table B, 4×4:
   - Before merging, type "a", "b", "c", "d" into the four top-left cells (this resolves U3).
   - Then merge that 2×2 block.
3. Table C, 3×2: after creating it, drag row 3 above row 1 (U2).
4. Table D, 1×1, untouched (defaults; U6, U8).

Commit: `crates/designos-fig/tests/fixtures/figjam_tables.jam`, owner-authored with no third-party content, so it can be committed. If the `.jam` exceeds 1 MB, ask the controller before committing.

Container support (INFERENCE): the `fig-jam.` magic already opens (`crates/designos-fig/src/container.rs:76`, `:113`, cited in `specs/e8-interop/README.md`). P1 confirms that `fig2json` reads it.

## Probes the builder runs

**P1, data dump** (resolves U1, U2, U4-U8 and U10). The first run used `figjam-table-2x4.jam`; the second uses `figjam-table-merge.jam`. The `jq` lines that select `TABLE_CELL` find nothing on real files (no such record); select the `TABLE` record and read its overrides instead, as the seed test does:
```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo run -p designos-fig --example fig2json -- crates/designos-fig/tests/fixtures/figjam_tables.jam > "$SCRATCH/tables-raw.json"
# every TABLE / TABLE_CELL NodeChange, full keys
jq '[.. | objects | select(.type? == "TABLE" or .type? == "TABLE_CELL")]' "$SCRATCH/tables-raw.json" > "$SCRATCH/tables-nodes.json"
# key set of a TABLE_CELL vs a STICKY (diff = candidate binding / style fields)
jq -r '[.. | objects | select(.type? == "TABLE_CELL")][0] | keys[]' "$SCRATCH/tables-raw.json" | sort > "$SCRATCH/cell-keys.txt"
jq -r '[.. | objects | select(.type? == "STICKY")][0] | keys[]' "$SCRATCH/tables-raw.json" | sort > "$SCRATCH/sticky-keys.txt"
diff "$SCRATCH/sticky-keys.txt" "$SCRATCH/cell-keys.txt"
```
- `fig2json` usage is FACT (`crates/designos-fig` fig2json harness lines 3-5).
- The `jq` paths assume the raw dump nests NodeChanges as objects with a `type` key. That is an ASSUMPTION; adjust after looking at the first 200 lines.
- The fixture needs one sticky for the diff. Add it if the owner's file has none, or diff against a TEXT node.
- Record each U with the exact JSON excerpt (guid, key, value).

**P2, Figma app semantics via `rea`** (global rule; resolves U1, U2, U6 where P1 is ambiguous):
- First `rea doctor`, then `rea analyze-javascript-application --help`. The exact flags are [UNVERIFIED]; `rea` 6.8.0 is installed at `~/.local/bin/rea`.
- Then run the JavaScript-application analysis on `/Applications/Figma.app` (present, FACT).
- Search the reconstructed bundle for the identifiers `tableRowPositions`, `tableColumnPositions`, `tableMergedCells`, `tableCellRowId`, and the strings `TABLE_CELL` / `TableCell`.
- Goal: the code that reads a cell's row and column, and the code that compares `position` strings.
- If the desktop app does not ship the editor code locally (it may load it from the web), say so. Do not fall back to logging in to figma.com without asking the controller.
- Record the Evidence ids `rea` returns.

**P3, merge text fate** (U3): the merged table in F2, read through P1. Rephrased after the first run: there are no cell records, so check whether the covered positions keep `[40000000:1, r, c]` text overrides and `[40000000:0, r, c]` frames. Either way the T1 model keeps no covered nodes; the finding decides what T3b reads and writes at those positions.

**P4, Figma opens Studio output** (U9): FLAGGED owner-manual, run after T3.
- The owner opens `target/tables-roundtrip.fig` (written by the T3 test or CLI) in FigJam and in Figma Design.
- Screenshot both. Without that screenshot, U9 stays open.

## Acceptance criteria

1. `plans/reports/tables-t0-probe.md` exists with one row per U1-U11: RESOLVED (with a JSON or `rea` Evidence excerpt) or STILL UNKNOWN (with the reason). U9 may stay open until T3.
2. The fixture is committed at `crates/designos-fig/tests/fixtures/figjam-table-2x4.jam` (PR pending). Its dump has exactly one `TABLE` record and zero `TABLE_CELL` records, and the count of `derivedImmutableFrameData.overrides` whose first guid is `40000000:0` equals rows × columns (8). Count those entries, not every `guidPath` (that gives 23). Pinned by `tests/figjam_table_raw_records.rs`. (Replaces the earlier `grep -c '"TABLE_CELL"'` = 29 / 32 check: Figma writes no such record.)
2b. Second run, on F2 (`figjam-table-merge.jam`): the U table is updated for U2 ordering, U3, U4, per-cell U7, U10 and U11 (duplicate table), each with a test in `figjam_table_raw_records.rs`. This closes T3b's gate.
3. Before code, the report gives T3 the exact cell-binding rule as a one-line statement plus excerpt. DONE: see "T3 binding rule" in the report.

## Non-goals

- No product code.
- No guesses written into code comments.
- No client files.

## Gates

None (no code). The controller reads the report on disk; idle is not completion.

## Visual evidence

- A screenshot of F1 open in FigJam (owner).
- A terminal capture of the P1 `diff` and of the `rea` search hits.

## Plugin-API facts from the owner's board (2026-11-07 01:20, read-only, figma-agent exec-js)
Board `e7weaiiyISx4ZWW1jfRmTN` ("Untitled", editorType figjam), evidence `evidence/board-table.json` + `evidence/board-walk.js`.
- FACT: one `TABLE` node `1:11`, 2 rows x 4 columns, 768 x 128.
- FACT: cells are `TABLE_CELL`, 192 x 64 each, text `A B C D / E "" "" ""`, solid fill.
- FACT: cell id = `T<table id>;<row id>;<column id>` (`T1:11;1:4;1:6`): rows `1:4`, `1:5`; columns `1:6`..`1:9`. Resolves U1 (a cell is keyed by row id + column id) and U11 (row and column ids are ordinary node ids in the file's id space) at the plugin level; the Kiwi field carrying them is still to confirm on a local file.
- NOT in this board: merged cells (U3, P3) and styled cells. Needs the owner to merge a 2x2 block and save a local copy.

## T0 results (2026-11-07, builder run on the owner's `Untitled.jam`)

Full table and proposed spec edits: `plans/reports/tables-t0-probe.md`. Evidence: `evidence/t0-jam-table-records.{txt,png}`. Fixture and seed test: commits `fdc508be9` + `0ef654759` on branch `jangtrinh/tables-t0-probe` (`crates/designos-fig/tests/fixtures/figjam-table-2x4.jam`, `tests/figjam_table_raw_records.rs`, 4 tests pass).

Headline (FACT, one 2x4 table, no merge):
- The file has NO `TABLE_CELL` records. `nodeChanges` = DOCUMENT, 2 CANVAS, 1 TABLE. The plugin API's `TABLE_CELL` nodes are synthesized.
- A cell is addressed by a `guidPath` of three GUIDs: `[template, rowId, colId]`. `rowId` / `colId` are the ids in `tableRowPositions` / `tableColumnPositions`; the template GUID is in the reserved session `40000000` (`0` = cell frame, `1` = cell text, `2` = shared fill + stroke, `3` = shared text style).
- Cell text: `TABLE.nodeGenerationData.overrides[].textData.characters`, keyed `[40000000:1, row, col]`. Empty cells have no entry.
- Layout: `TABLE.derivedImmutableFrameData.overrides` holds one frame (`40000000:0`) and one text (`40000000:1`) per cell, empty cells included.
- Merge (U3, U4), reordered rows (U2 ordering) and per-cell fill (U7): NOT in this file, still UNKNOWN. The file has no `tableMergedCells` (field 538).

Status per unknown after the first run (full evidence in the report):

| Id | Status | Next |
|---|---|---|
| U1 binding | RESOLVED: `guidPath` `[40000000:<role>, rowId, colId]` on the TABLE's overrides; no field on a cell, because no cell record exists | T3a |
| U2 positions | PARTIAL: same strings as `parentIndex.position`; ordering of a reordered track UNKNOWN | F2 re-probe |
| U3 covered cells | UNKNOWN (no merge); now asked as "do covered positions keep text overrides / derived frames?" | F2 re-probe |
| U4 merge anchor | UNKNOWN (no `tableMergedCells`) | F2 re-probe |
| U5 cell text | RESOLVED: `nodeGenerationData.overrides[].textData.characters` at `[40000000:1, r, c]`; empty cell = no entry | T3a |
| U6 sizes | PARTIAL: px; missing row height = 64 in this file (INFERENCE as a default); missing column width UNKNOWN | T3a uses derived frames first |
| U7 style | PARTIAL: shared fill/stroke on template `:2`, text style on `:3`; per-cell override UNKNOWN | F2 re-probe |
| U8 size = track sum | RESOLVED for this file | T3a |
| U9 Figma opens Studio output | UNKNOWN | P4 at the end of T3a, again in T3b |
| U10 header flag | none observed (one file) | F2 re-probe records it |
| U11 id space | PARTIAL: file id space, disjoint from the template session; duplicated table UNKNOWN | F2 re-probe (optional duplicate) |

Open from the first run, for the controller: P2 (`rea` on Figma.app) was not run; `grep` finds neither `tableMergedCells` nor `tableRowPositions` in `app.asar`, and logging in to figma.com needs a controller decision.
