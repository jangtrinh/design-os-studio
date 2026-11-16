# T3a: `.fig` import and export of tables without merges (the `TABLE` record and its cell overrides)

Revised 2026-11-07 from the T0 results (`plans/reports/tables-t0-probe.md`). T3 is split:
- **T3a (this file):** unmerged tables, both directions, golden = the owner's 2x4 board. Starts as soon as T1 and the fig PRs merge; it no longer waits for T0 or the owner.
- **[T3b](T3b-fig-table-merges-second-fixture.md):** merges, reordered-track ordering, per-cell style, Vietnamese golden. **Gated on fixture F2** (owner re-save). Not started before that gate clears.

| Field | Value |
|---|---|
| Issues | #317 AC-2 ("TABLE 29, TABLE_CELL 30, fields 308-311, 538 round-trip"), unmerged half |
| Size | 9 h (of the former 12 h; T3b is 5 h) |
| Tier | Claude Sonnet build, Opus review |
| Depends on | T1 merged. Open designos-fig PRs merged first or rebased onto: #1664, #1671, #1685 (open, FACT `gh pr list` 2026-11-06), #1672 per ledger. Ruling R2 granted (`controller-rulings.md`: #317 moves ahead of #224). Golden fixture PR merged (`crates/designos-fig/tests/fixtures/figjam-table-2x4.jam` + `tests/figjam_table_raw_records.rs`, commits `fdc508be9` + `0ef654759` on `jangtrinh/tables-t0-probe`; PR pending). |
| Runs in parallel with | T1b, T2, T4 (disjoint crates) |
| Blocks | T3b, T6 (round-trip evidence), P4 owner-manual check |
| Branch | `jangtrinh/tables-t3a-fig` |

## Goal

1. Opening a FigJam or Figma file with an unmerged table gives an editable `Table` with `TableCell` children, not `Unknown`.
2. Saving `.fig` writes one `TABLE` (29) record that Figma reads back: tracks 308-311 plus cell text in `nodeGenerationData`. No `TABLE_CELL` record.
3. Old `.designos` files that kept `Unknown{TABLE}` with passthrough fields upgrade to real tables on load.

## What the file holds (FACT, T0, one 2x4 table, no merge)

- One `TABLE` record (`1:11`, 768 × 128). **Zero `TABLE_CELL` records.** The plugin API's `TABLE_CELL` nodes are synthesized by Figma.
- Tracks: rows `1:4`, `1:5` in `tableRowPositions` (308); columns `1:6`..`1:9` in `tableColumnPositions` (309). Positions `!` `"` / `!`..`$`, the same fractional-index strings as `parentIndex.position`. Track ids sit in the file's id space.
- Sizes: `tableColumnWidths` (311) has every column (192). `tableRowHeights` (310) is empty; derived frames are 64 high.
- **Binding rule:** a cell is the pair `(rowId, colId)`. Its data are the entries of `TABLE.nodeGenerationData.overrides` / `derivedImmutableFrameData.overrides` whose `guidPath` is `[40000000:<role>, rowId, colId]` (role 1 = text, 0 = frame).
- Cell text: `nodeGenerationData.overrides[].textData.characters` at `[40000000:1, row, col]`. An empty cell has no entry.
- Shared style: single-guid template overrides `[40000000:2]` (cell fill white, stroke black 20 % weight 1 INSIDE, corner radius 0) and `[40000000:3]` (Inter Medium 16, line height 1.5, letter spacing −1.1 %, LEFT / TOP).
- The `TABLE` carries its own `fillPaints` (black, alpha 0.098), `cornerRadius` 12, `frameMaskDisabled` true.
- `derivedImmutableFrameData`: one frame + one text entry per cell, empty cells included, with glyph `derivedTextData` blobs. It is a layout cache written by Figma.

## Field mapping

| Kiwi | Doc model (T1) | Direction / status |
|---|---|---|
| `NodeChange.type = TABLE` (29) | `NodeKind::Table`; name, transform, size, fills, corner radius through the existing generic node mapping | both, FACT |
| `NodeChange.type = TABLE_CELL` (30) | none. Import synthesizes one `TableCell` per `(row, col)` in track order, row-major child order. Export writes no `TABLE_CELL`. | FACT (no such record) |
| 308 / 309 entries `{id, position}` | `TableProps.rows` / `.cols` order, `TrackId{session: id.sessionID, local: id.localID}`. Order by the existing byte-wise helper `grid_position_guids` (`crates/designos-fig/src/fig.rs:1217-1231`, same `{id, position}` shape; DRY). | Encoding FACT; byte-wise ordering INFERENCE until T3b checks a reordered row |
| 310 `tableRowHeights` | `TableTrack.size` for rows. Missing entry: the height of that row's derived frame (`[40000000:0, row, *]`) when present, else 64 (INFERENCE from one file). | import |
| 311 `tableColumnWidths` | `TableTrack.size` for cols. Missing entry: the width of that column's derived frame; if neither exists, the table stays `Unknown` (UNKNOWN default). | import |
| `nodeGenerationData` text override `[40000000:1, r, c]` | the `(r, c)` cell's `TextProps.content` | both, FACT |
| template override `[40000000:2]` | stroke → the table node's `stroke` (grid); fill → every imported cell's `fills` | both, FACT for the shape |
| template override `[40000000:3]` | every imported cell's `TextProps` font, size, line height, letter spacing, alignment | both, FACT for the shape |
| `derivedImmutableFrameData` (+ `Version`) | not typed. Dropped from the typed table and counted in the import report ("derived layout cache dropped"). Export does not write it; P4 decides whether Figma needs it. | UNKNOWN whether Figma recomputes it |
| 538 `tableMergedCells` | not in T3a (gate below) | T3b |

Rules:
- **Typing gate.** T3a types a `TABLE` only when it has no `tableMergedCells` entry and every `nodeGenerationData` override has one of the shapes the 2x4 golden shows (text at `[40000000:1, r, c]`, templates `:2` and `:3`; the builder pins the exact key sets in a test read from the golden). Anything else stays `Unknown{TABLE}` with every field in `fig_metadata`, and the import report names the reason. Nothing is guessed.
- **Unbound override.** A cell override whose row or column id is not in the track maps keeps the whole table `Unknown`, fields kept, counted.
- **Passthrough hygiene.** When a table is typed, remove 308-311, 538, `nodeGenerationData`, `derivedImmutableFrameData` and `derivedImmutableFrameDataVersion` from its `fig_metadata`, so export never writes stale glyph blobs for edited text. Keep the raw JSON of the two template overrides under one private `fig_metadata` key, and on export write the typed values over it, so template sub-fields T3a does not type survive.
- **Imported tables keep the file's look.** Text 16 px from template `:3` stays 16 px. Tables created in Studio use the same Inter Medium 16 (OQ-13, re-ruled 2026-11-07).
- **One mapping (DRY).** The mapping from table fields to `TableProps` + cells lives once, in `crates/designos-doc/src/table_kiwi.rs`. The importer produces today's `Unknown{TABLE}` + `fig_metadata` and then calls it; the legacy upgrade on load calls the same function.

### Export (one `TABLE` record)

- Fields: 308 / 309 in model order with positions from the same generator the exporter uses for `parentIndex.position` (`pos_str`, `crates/designos-fig/src/fig_export.rs:428` at `main@ec47da26d`); 310 with **every** row written explicitly from `TableTrack.size` (the user's height, never the auto-grown one, so round trips do not ratchet rows taller, and a 44 px Studio row never reads as Figma's 64); 311 with every column.
- `nodeGenerationData` with `useFineGrainedSyncing: true`, template `:2` (grid stroke + the most common cell fill), template `:3` (the most common cell text style), and one text override per non-empty cell at `[40000000:1, rowId, colId]`.
- A cell whose fill or text style differs from the template: the per-cell override shape is UNKNOWN, so T3a does not write it. The export report counts each such cell (Law 1). T3b writes it once F2 shows the shape.
- A native table with merges: T3a writes no 538 entry. The anchor keeps its text, covered positions are written empty, and the export report counts "merges not written: N". T3b replaces this. The tag waits for T3b, so this never ships.
- Track GUIDs (U11): imported ids are kept when they do not collide inside the file being written; otherwise, and for native `TrackId`s, each track gets a GUID from the exporter's node allocator (`alloc_guid`, `fig_export.rs:358` at `main@ec47da26d`). No node GUID and no track GUID may use the template session `40000000`.

## Files T3a may modify (hot-file queue files marked HQ)

- `crates/designos-fig/src/fig.rs` (HQ): typed `TABLE` hook next to the `STICKY` arm (re-anchor the line against HEAD)
- `crates/designos-fig/src/rest_import.rs` (HQ): typed construction next to the board arm (re-anchor)
- `crates/designos-fig/src/fig_export.rs` (HQ): replaces T1's single-line arm with the writer
- `crates/designos-fig/src/export.rs` (HQ): REST JSON export arm
- `crates/designos-fig/src/table.rs` (new): export writer and import hook
- `crates/designos-doc/src/table_kiwi.rs` (new): the one mapping, plus one call line in `load_repair.rs`
- `crates/designos-fig/tests/fig_table_roundtrip.rs` (new)
- `crates/designos-fig/tests/fig_roundtrip.rs`, the synthetic table: the `TABLE` + `TABLE_CELL` NodeChanges in `build_passthrough_changes` (`:1919-1945` at `main@ec47da26d`) and their assertions (`:2044-2054`, including the `CellR1C1` grid-alignment check that runs past `:2051`). **Flagged for replacement:** Figma never writes `TABLE_CELL`, so this emitter models a file that cannot exist. Replace it with a `TABLE` shaped like the golden (no cell record, text in `nodeGenerationData`), expect `NodeKind::Table`, and move the grid-alignment assertion onto a node type that really carries `gridChild*` fields, so that coverage is kept. Say so in the PR; it is an intentional behaviour change.

## Acceptance criteria (red at base, green at head)

1. **Import the golden** `crates/designos-fig/tests/fixtures/figjam-table-2x4.jam`:
   - one `NodeKind::Table`, 2 rows × 4 columns, track ids `1:4`, `1:5` / `1:6`..`1:9`, every column 192, every row 64, table size 768 × 128;
   - 8 `TableCell` children in row-major order; texts `A B C D` / `E "" "" ""` at the right `(row, col)`;
   - every cell fill is the template `:2` fill; the table stroke is black 20 % weight 1; every cell text is Inter Medium 16.
   - At base the kind is `Unknown`, so this is red.
2. The typed table's `fig_metadata` holds none of 308-311, 538, `nodeGenerationData`, `derivedImmutableFrameData`, `derivedImmutableFrameDataVersion`. The import report counts the dropped derived cache.
3. **Native export:** a 3×3 table built with T1b ops (column widths 120 / 240 / 90, row heights 44 / 80 / 44, texts including "Bảng giá tháng 11", one empty cell), then `export_fig` → `decode_fig_to_json`:
   - zero `TABLE_CELL` NodeChanges; one `TABLE` with 308 / 309 / 310 / 311 entry counts 3 / 3 / 3 / 3 and no 538;
   - 8 text overrides, each with `guidPath` `[40000000:1, rowId, colId]` matching the written track ids; templates `:2` and `:3` present;
   - no track GUID equals a node GUID; no GUID other than the template paths uses session `40000000`; two tables duplicated in Studio have disjoint track GUIDs;
   - re-import equals the source in track order, sizes ±0.01 and texts (byte-equal).
4. **No ratchet:** a row auto-grown by long text (laid-out height > stored height) exports its stored height, and export → import → export writes identical 310 values.
5. **File round trip:** golden → import → `export_fig` → import gives the same table as AC-1, and the raw track GUIDs equal the golden's.
6. **Gate holds:** a copy of the golden's decoded JSON with a `tableMergedCells` entry added, and one with an extra override shape, each import as `Unknown{TABLE}` with every field in `fig_metadata` and a named reason in the import report. A native table with a merge exports without 538 and the export report counts it.
7. **Legacy upgrade:** the golden imported at base (`Unknown{TABLE}` + `fig_metadata`), saved as `.designos`, reloads as the AC-1 table. An `Unknown{TABLE}` with children, or with an override that references a non-track id, stays `Unknown` and loses nothing (asserted).
8. A `Table` created in a Design file (`DocumentKind::Design`) exports with the same fields.
9. The replaced synthetic table in `fig_roundtrip.rs` passes with the new expectation. Existing suites stay green: `cargo test -p designos-fig` (including `fig_export_field_shapes.rs`, `figjam.rs`, `fig_unmapped_layer_edits.rs`, and the seed test `figjam_table_raw_records.rs`).
10. **P4, FLAGGED owner-manual, at the end of T3a:** the owner opens `target/tables-roundtrip.fig` (written by the AC-3 test, kept with an env flag) in FigJam and in Figma Design and sends screenshots. This also answers whether Figma needs `derivedImmutableFrameData`. If Figma rejects the file or shows empty cells, T3a is NOT done: the controller rules on writing derived frame entries (geometry only) before T3b starts. Without the screenshots U9 stays open, and the tag waits (Law 5).

## Non-goals

- Merges, reordered-track ordering checks, per-cell fill and text style, header flags: [T3b](T3b-fig-table-merges-second-fixture.md).
- `.jam` import as a product feature (#320). T0 observed the `.jam` importing as document kind `Design`; not changed here.
- `.pen` table fidelity.
- Figma REST API table import (no REST sample; follow-up issue).

## Gates

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p designos-fig
cargo test -p designos-doc
```

## Visual evidence

1. The golden opened in Studio (`designos --screenshot`, CPU and `--gpu` once T2 has merged) next to the owner's FigJam thumbnail (`evidence/t0-jam-thumbnail.png`).
2. P4 screenshots (AC-10).

## Risks

| Risk | L×I | Mitigation |
|---|---|---|
| Figma needs `derivedImmutableFrameData` (glyph `commandsBlob` indexes into `blobs`; writing it means producing text outlines) | M×H | P4 at the end of T3a, not after T3b, so the answer comes early. Fallback is a controller ruling with the P4 evidence. |
| Byte-wise track ordering is wrong for reordered tracks | M×M | Same comparator as the importer's grid tracks and `parentIndex`; T3b checks it on F2 |
| A Figma table with a shape T3a does not know imports as `Unknown` | H×L | Intended: the gate keeps every field; T3b widens it |
| Figma rejects Studio-written positions | M×H | P4 |
| Conflicts with #1664 / #1671 / #1685 in `fig.rs` / `fig_export.rs` | H×M | Rebase after they land; table code in the new `table.rs` with one-line hooks |

## Rollback

Revert. Tables then import as `Unknown` again, and the X1 passthrough still keeps their fields. Native tables then export as T1's bare arm: type only, no table fields. Note that in the revert PR.
