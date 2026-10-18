# E15-8 Return before/after diff images for the region a write touched

| Field | Value |
|---|---|
| Epic / pack | E15 / v0.6 AI design |
| Size / priority | M / P0 |
| Depends on | T7.1 (#238), E15-4 |
| Unblocks | E15-15 (verify step), part-2 repair regression guard |
| Touches | `crates/designos-mcp/src/tools/batch.rs`, `crates/designos-diff/src/lib.rs`, `crates/designos-mcp/Cargo.toml` (add `designos-diff`), CPU render path via `crates/designos-mcp/src/tools/screenshot.rs` |

## 1. Summary and user story

As an agent, after a batch I ask for `diff_image: true` and get three small images of exactly the
area I changed (before, after, highlighted difference) plus the changed-pixel count and box, so I can
check the effect without a fresh full screenshot.

## 2. Current state (FACT)

- No write tool returns images; T7.1 adds `render_target` for inline screenshots (spec
  `T7.1-get-screenshot.md`).
- The pixel diff library exists: `DiffOptions { tolerance, detect_aa, ignore }`
  (`crates/designos-diff/src/lib.rs:45-58`), `DiffReport` with `mismatched`, `ratio`, `ssim` and no
  bounding box (`lib.rs:61-80`), `diff_image(a, b, opts) -> (DiffReport, RgbaImage)` (`lib.rs:308`),
  `crop` (`lib.rs:336`).
- designos-mcp does not depend on designos-diff (`crates/designos-mcp/Cargo.toml:12-20`).
- E15-4 keeps the pre-batch document untouched while ops run on a trial clone, and reports
  `touched_roots` with page-space bounds and their union `touched_bounds`.

## 3. Behaviour rules

1. Option `diff_image: true` on `designos_apply_batch` (declared in E15-4) and on single mutating
   tools (`create_node`, `update_node`, `delete_node`, `apply_spec`).
2. Region per page: union of touched-root bounds before the batch and after it (a moved node
   contributes both positions), grown by 8 page units on each side, clipped to the page's content
   bounds. If touched roots span several pages, one triple per page, at most 4 pages; beyond that the
   reply sets `diff_skipped: "more than 4 pages"` and returns none.
3. Before image: rendered from the pre-batch document; after image: from the committed document (or
   the trial for a dry run). Both through T7.1 `render_target` in region mode with the same
   `scale_used` (computed once from the union and `max_long_edge`, default 1024), on the CPU path, so
   they align pixel for pixel.
4. Diff: `diff_image(before, after, DiffOptions { tolerance: 0, detect_aa: false, ignore: [] })`.
   AA forgiveness is off because both renders are bit-exact on the same renderer; any changed pixel is
   real.
5. Changed box: new `DiffReport.changed_bbox: Option<Rect>` (min/max x,y over mismatched pixels),
   converted to page space as `region.origin + bbox / scale_used`.
6. No change: `changed_pixels == 0` → no images at all are returned, `changed_bbox: null`.
7. Images are MCP image content blocks in the order before, after, diff; each block's index and role
   are listed in `structuredContent.diff.images`.
8. If a render fails the T7.1 hard cap, the batch still succeeds and `diff_skipped` names
   `IMAGE_TOO_LARGE`; images never fail a write.

## 4. Data model / schema changes

- `designos_diff::DiffReport.changed_bbox: Option<Rect>` with `#[serde(default)]` (CLI JSON output
  stays back-compatible: new optional key).
- No document change.

## 5. Contracts

Request: `designos_apply_batch` with `"diff_image": true` (schema in E15-4).

Reply addition:

```json
{"diff":{"pages":[{"page_id":1,
   "region":{"x":92,"y":192,"width":416,"height":136},"scale_used":1.0,
   "changed_pixels":38400,"changed_ratio":0.678,
   "changed_bbox":{"x":100,"y":200,"width":400,"height":120},
   "images":[{"content_index":0,"role":"before"},{"content_index":1,"role":"after"},{"content_index":2,"role":"diff"}]}],
  "diff_skipped":null}}
```

Errors: none of its own; render problems go to `diff_skipped`. Idempotent replays re-render (E15-6
edge case 6).

## 6. File-format mapping

None.

## 7. UI

None (MCP only).

## 8. Edge cases

1. Delete only: after image shows the background; bbox = deleted node's painted area.
2. Change hidden by an opaque node above it: 0 changed pixels, honest result; `changed_pixels: 0`
   with no images even though `changed.updated > 0`.
3. Variable value change (E15-4 `set_variables`) that recolours nodes all over the page: touched roots
   are the bound nodes (E15-4 computes them from bindings), region may be the whole page; downscaled
   to the cap.
4. Huge region (whole 20,000 px page): `scale_used` ≈ 0.05; still valid; if below 0.01 →
   `diff_skipped`.
5. Dry run: before = current, after = trial; same output shape with `dry_run: true`.
6. Text edits with font fallback: same renderer both sides, so only real text changes show.

## 9. Non-goals

- GPU diffing or perceptual thresholds.
- Diffing against arbitrary historic versions (part-2 repair guard uses the same helper).

## 10. Test plan

Seam: `cargo test -p designos-mcp` with a pixel fixture.

| Case | Steps | Expected |
|---|---|---|
| `recolour_bbox_equals_node_bounds` | 400x120 rect at (100,200) on a white page, update fill red → blue, `diff_image:true` (scale 1, no downscale) | 3 image blocks; `changed_bbox` = (100,200,400,120) within 1 px each edge |
| `noop_batch_reports_zero` | update with current values | `changed_pixels: 0`, no image blocks |
| `move_includes_both_positions` | move rect by 300 px | region covers old and new; bbox covers both |
| `dry_run_diff_leaves_document` | dry run + diff | images returned; document unchanged |
| `diff_report_bbox_unit` (designos-diff) | two 10x10 images differing at (2,3) and (7,8) | `changed_bbox == Rect{x:2,y:3,w:6,h:6}` |

Fail-before: `recolour_bbox_equals_node_bounds` fails today (no `diff_image` option).

## 11. Performance budgets

Two region renders plus one diff at ≤ 1024 px long edge: under 400 ms release for a 2,000-node
region. Renders reuse per-subtree variable resolution (T7.1), never a whole-document clone.
`diff_image` is opt-in, so writes without it pay nothing.

## 12. Dependencies and build order

After T7.1 and E15-4. Independent of the lint track.

## 13. Owner rulings

- Full scope: multi-page diffs and dry-run diffs ship with the base feature.
- All AI through the user's agent CLI: images go only to the user's agent CLI.
