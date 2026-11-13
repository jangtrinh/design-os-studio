# T3b: `.fig` merged cells, reordered tracks and per-cell style (gated on fixture F2)

| Field | Value |
|---|---|
| Issues | #317 AC-2 (field 538 round trip), the merge half |
| Size | 5 h build, after a 1.5 h T0 re-probe of F2 |
| Tier | Claude Sonnet build, Opus review |
| **Gate (hard)** | **Not started until (1) the owner's F2 board is committed as `crates/designos-fig/tests/fixtures/figjam-table-merge.jam`, and (2) the T0 re-probe report on F2 resolves U2 ordering, U3, U4 and per-cell U7 (U10 and duplicate-table U11 recorded either way).** A builder who finds the gate open stops and reports `BLOCKED`. |
| Depends on | T3a merged (the mapping, the typing gate and the export writer it widens); T1b (merge ops for native tests) |
| Runs in parallel with | T4, T5, T5b |
| Blocks | T6, the 0.7.11 tag |
| Branch | `jangtrinh/tables-t3b-fig-merges` |

## Why it is gated

The first fixture has no merge, no reordered track and no styled cell (T0 FACT). Field 538's shape is known from the schema (`TableMergedCellMapEntry { rowId, colId, rowSpan, colSpan }`, `figma_schema.txt:1771-1777`), but its meaning is not: which corner the pair names, and what happens to covered positions' text overrides and derived frames. Writing that from the schema alone would be a guess in code (T0 non-goal), so T3b waits for evidence.

## Fixture F2 (owner-manual, about 10 min, asked 2026-11-07 morning)

One FigJam board, "Save local copy", nothing third-party:
1. A 4×4 table. Before merging, type "a", "b", "c", "d" into the four top-left cells; then merge that 2×2 block (U3, U4).
2. In the same or another table: drag the third row above the first, with texts that name each row ("row 1", "row 2", "row 3") (U2 ordering).
3. One cell with a non-default background and one with a changed text style (U7 per cell).
4. Vietnamese with diacritics in a cell ("Bảng giá tháng 11").
5. Optional: duplicate one table (U11, cross-table track ids).

Committed at `crates/designos-fig/tests/fixtures/figjam-table-merge.jam`. If it exceeds 1 MB, ask the controller first.

## Re-probe (T0 leg, before T3b code)

Extend `figjam_table_raw_records.rs` with tests that pin, from F2: the 538 entries; whether covered positions keep text overrides and derived frame entries; the order of the reordered row's position strings; the per-cell fill and text-style override shape; any new `TABLE` key (U10). Update the U table in `plans/reports/tables-t0-probe.md`. T3b copies each rule verbatim into its PR.

## Scope

- **Import:** type 538 into `TableMerge`. Widen T3a's typing gate to the override shapes F2 shows. Covered positions: whatever F2 shows, any non-empty covered text is appended to the anchor (ruling OQ-5: one line per covered cell) and counted in the import report (Law 1).
- **Export:** write 538 for every model merge, plus covered-position overrides and frames exactly as F2 shows. Remove T3a's "merges not written" path and its export-report counter.
- **Per-cell style:** write per-cell fill and text-style overrides in the F2 shape. Remove T3a's "per-cell style not written" counter.
- **Ordering:** confirm or replace the byte-wise comparator against F2's reordered row.

## Acceptance criteria (values come from the re-probe; slots named now)

1. Import F2: the merged table has one `TableMerge` with `row_span` 2 and `col_span` 2 at the anchor the re-probe names (`<U4 rule>`), and no cell nodes at covered positions (T1 decision 4). The anchor text follows the ruling OQ-5 rule applied to `<U3 finding>`.
2. The reordered table's rows import in the visual order: texts top to bottom are `<recorded by the re-probe>`.
3. The styled cell's fill and text style import onto that cell; other cells keep the template values. The Vietnamese text matches byte for byte.
4. Native export of a 3×3 table with a 2×2 merge, a cell fill and Vietnamese text: one `TABLE` with 538 holding 1 entry in the F2 shape, covered positions written per `<U3 finding>`, zero `TABLE_CELL` records; re-import equal in track order, sizes ±0.01, merges and texts.
5. F2 → import → `export_fig` → import gives the same tables (structural compare), raw track GUIDs equal F2's.
6. T3a's AC-6 negative tests flip: a merged table now types. The two T3a export-report counters are gone (their tests are replaced, not deleted silently).
7. P4 repeated on the AC-4 file: the owner opens it in FigJam and Figma Design; screenshots show the merge. The tag waits for this.

## Files T3b may modify

Same as T3a (`crates/designos-fig/src/{table.rs, fig.rs, fig_export.rs}`, `crates/designos-doc/src/table_kiwi.rs`, `crates/designos-fig/tests/{fig_table_roundtrip.rs, figjam_table_raw_records.rs}`), plus the F2 fixture. Serial after T3a, never parallel with it.

## Gates

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p designos-fig
cargo test -p designos-doc
```

## Risks

| Risk | L×I | Mitigation |
|---|---|---|
| The owner's F2 slips past the release window | M×H | The tag waits (owner ruling: tables ship in 0.7.11). The controller re-asks with the exact steps above; nothing is guessed in the meantime. |
| F2 shows Figma keeps covered cells' text | M×M | The model still has no covered nodes (T1 decision 4). Import appends the text per OQ-5; export writes the covered overrides F2 shows. |
| F2 shows a merge shape that does not fit `TableMerge` | L×H | Stop and ask the controller; the model change goes to T1 owners, not into T3b. |

## Rollback

Revert. Merged tables import as `Unknown` again (T3a's gate), and native merges export without 538, counted.
