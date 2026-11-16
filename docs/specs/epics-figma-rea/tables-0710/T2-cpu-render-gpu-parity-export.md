# T2: draw tables on CPU and GPU, export them to PNG, SVG and PDF

| Field | Value |
|---|---|
| Issues | #317 AC-1 ("Merged 2x2 cells render as one"), #1231 test step 3 (export includes the table, crate level) |
| Size | 9 h |
| Tier | Claude Sonnet build, Opus review (render/GPU invariants) |
| Depends on | T1 merged |
| Runs in parallel with | T1b, T3a |
| Blocks | T4, T5 (visual evidence), T6 perf |
| Branch | `jangtrinh/tables-t2-render` |

Read first: `CLAUDE.md` §4 (performance and safety invariants 1-4), `.brv/context-tree/architecture/crate-map.md` (`SceneIndex`, `SceneCompiler`), `crates/designos-render/src/board.rs:1-8` (the lowering contract).

## Goal

A table draws the same in the CPU reference renderer and the GPU renderer, and PNG, SVG and PDF exports include it:
- background;
- cell fills;
- grid lines that skip the inside of merged blocks;
- cell text with padding.

## Approach: the board lowering path, no new renderer branch

FACT: both renderers draw `is_board_item()` nodes as `board::lower(doc, n)`:
- CPU: `crates/designos-render/src/lib.rs:494-499`.
- GPU: `crates/designos-gpu/src/compile.rs:891-896`.
- Geometry: `crates/designos-render/src/geom.rs:497-500`.

T1 makes `Table` and `TableCell` board items, so T2 only adds two arms to `board::lower` (`crates/designos-render/src/board.rs:173-235`):

1. **`TableCell` lowers to a `Rectangle` body** with the cell's fills, no stroke and no corner radius.
   - Plus one synthetic `Text` child (slot 0) holding the cell text, laid inside the cell minus `TABLE_CELL_PADDING`.
   - Vertical alignment follows OQ-2 (recommended: top).
   - Text wraps at the cell width with `TextAutoResize::Height`. It never shrinks to fit, unlike stickies (`fit_into`).
2. **`Table` lowers to a `Frame` body** with the table's fills.
   - `clips_content = false`, so the lowering opens no clip program (invariant 3/4).
   - `children` = the table's real cell ids (in `n.children` order), followed by one synthetic `Vector` child (slot 0): the grid lines.
   - Grid lines: the outer border, plus every interior row and column boundary segment that is not inside a merged block. The stroke paint and weight come from the table node's `stroke`, `StrokeAlign::Center`.
   - Real child ids resolve through `board::get` as document nodes (`board.rs:38-44`), and each cell lowers itself when it is drawn.

**Bug to fix in the same PR (FACT plus INFERENCE).** `lowered_document` (`board.rs:237-257`) re-inserts every id in `body.children` from `get(doc, c)`.
- For a real child id, that returns the unlowered document node.
- Depending on iteration order, it overwrites a cell that was already lowered, so SVG and PDF (`designos-export/src/svg.rs:42`, `pdf.rs:90`) would meet a raw `TableCell`.
- Fix: insert only ids where `is_synthetic(c)` is true. PNG is unaffected: `designos-export/src/raster.rs:200` goes through the renderer.

**Invariants kept:**
- No new offscreen `Program`: no clip, mask, blur or blend in the lowering.
- Text greeking below 3.5 px and sub-pixel culling apply to the synthetic texts as to any text, because both stay behind `interactive()` (`compile.rs` `fn interactive`).
- Headless reference compiles (`budget = None`) are untouched, so `cargo test -p designos-gpu` stays bit-exact.
- `board::lower` runs per drawn cell per compile. The `SYNTH` registry clears at 50,000 entries (`board.rs:226-229`); a 50×50 table registers 2,501 entries per full draw, well under that (FACT: the arithmetic; perf measured in T6).

## Input from the real file (T0, 2026-11-07)

The approach is unchanged: T1 keeps cells as real child nodes (T1 re-check). The owner's FigJam board gives the reference look for the OQ-1 / OQ-2 mocks (FACT, one file): cells 192 × 64, text inset 20 px on every side, Inter Medium 16, white cells, grid lines black 20 % weight 1, and the table rounded 12 at its four outer corners. If the owner picks that look, the outer corners are drawn by giving the four corner cells' lowered rectangles a corner radius on their outer corners and rounding the grid's outer border path, never by a clip on the table body (`clips_content = false` stays, invariants 3 and 4).

## Files T2 may modify

- `crates/designos-render/src/board.rs`
- `crates/designos-render/src/geom.rs`: only the board-item arm list at `:497`, if T1's forced arm did not already add the kinds
- `crates/designos-render/tests/table_render.rs` (new)
- `crates/designos-gpu/tests/table_parity.rs` (new)
- `crates/designos-export/tests/table_export.rs` (new)

## Acceptance criteria (red at base, green at head)

1. CPU render, 3×3 table with a 2×2 merge at the top left (`table_render.rs`):
   - The pixel at the centre of the merged block is the anchor fill.
   - Along the would-be interior line at x = col0 width, inside the merge, there are no grid-line pixels: the pixel equals the anchor fill.
   - The interior line between columns 1 and 2 in row 2 is present: the pixel equals the stroke colour.
   - This is #317 AC-1.
2. CPU vs GPU parity for the same scene plus a table with Vietnamese text and a cell fill: `cargo test -p designos-gpu table_parity` passes with the same tolerance the existing parity tests use. Find the helper the other `crates/designos-gpu/tests/*.rs` parity tests call; [UNVERIFIED] name.
3. `SceneIndex::bounds_of(cell)` equals the cell's world rect from `relayout` for all 9 cells of a 3×3 table.
4. SVG export of a table node (`designos_export::svg::export_svg`, `svg.rs:40`) contains every cell's text string and one path for the grid. This needs the `lowered_document` fix; a test whose document order puts a cell before its table catches the overwrite.
5. PNG export (`raster::export_png`, `raster.rs:200`) of the table is non-empty and its pixel at a cell centre equals that cell's fill. PDF export contains the cell text (text-extract or byte search, whichever the existing PDF tests use).
6. An interactive GPU compile of a page holding one 50×50 table creates no offscreen program beyond what an empty page creates. Assert through the compiled frame's program count; [UNVERIFIED] accessor, found by the builder in `compile.rs`, cited in the PR.
7. Existing suites stay green: `cargo test -p designos-render`, `cargo test -p designos-gpu`, `cargo test -p designos-export`.
8. **The grid survives a registry clear in the middle of a draw.**
   - The failure mode is new with tables:
     - Today a lowered body's synthetic children are fetched before any other `lower()` runs on the thread.
     - A table body's grid child (slot 0) is drawn after its real cells, and each cell calls `lower()`.
     - `lower()` clears `SYNTH` once it holds more than 50,000 entries (`board.rs:226-229`). If a cell's lowering triggers that clear, `board::get(grid_id)` returns `None` and the grid silently disappears.
     - The registry grows over a session, because ids of deleted items are never reused.
   - The test:
     - Pre-fill the registry to 49,999 entries through a test-only helper, or by lowering 49,999 throwaway items.
     - Render a 3×3 table on CPU and GPU.
     - Assert that grid-line pixels are present on both.
   - The builder picks the fix and states it in the PR. Two candidates:
     - clear only at a compile or frame boundary;
     - never clear while a lowered body's children are still pending.
   - Either way, invariant 3 (headless bit-exact) holds.

## Test seam

- Headless CPU render and GPU parity tests.
- Export unit tests.
- The interactive compile is driven with `budget = Some(8ms)` in a test.

## Non-goals

- Editing chrome: selection, handles, hover (T4).
- Live text cursor drawing (existing text session).
- Dev Mode code generation for tables (owner OQ-10).
- Whiteboard export toggle WB-11 (#1226, v0.7.8).

## Gates

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p designos-render
cargo test -p designos-gpu
cargo test -p designos-export
```

## Visual evidence

1. `designos --screenshot` (CPU) and `--gpu` captures of a native fixture document: a 3×3 table with a 2×2 merge, Vietnamese text and a cell fill, plus a 2x4 table rebuilt natively to match the owner's board (`evidence/t0-jam-thumbnail.png`), shown side by side with that thumbnail. Once T3a has merged, use the imported golden `figjam-table-2x4.jam` instead of the rebuilt 2x4.
2. The exported PNG and SVG opened in a browser, captured. Keep the display awake (`caffeinate -u -d`) per project memory.

## Risks

| Risk | L×I | Mitigation |
|---|---|---|
| `lowered_document` overwrite: half-lowered SVG/PDF | H×H | AC-4 with an adversarial order |
| Per-frame `lower` cost on 2,500 cells | M×M | T6 perf test. If over budget, cache lowered cells keyed by `(id, rev)` in a follow-up; not in this phase unless AC-6 or T6 fails. |
| Text wrap differs CPU vs GPU | M×H | AC-2 includes multi-line Vietnamese text |
| A `SYNTH` clear during a nested cell lowering drops the table grid | M×H | AC-8 |

## Rollback

Revert the commit. Tables then draw nothing (board lowering returns `None`), but documents stay intact.
