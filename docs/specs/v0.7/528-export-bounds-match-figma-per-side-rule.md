# #528 Export bounds: PNG and SVG export size matches Figma's render bounds on the 10 kit frames

| Field | Value |
|---|---|
| Issue | #528 (bug; found by the kit render audit of 2026-10-18; owner ruling 2026-10-18: exports match Figma's render bounds) |
| Size | M |
| Depends on | none for causes 1 to 3; cause 5 needs the live Figma read in section 10; the rounding change needs calibration (section 3 rule 7) |
| Unblocks | #37 (its progressive-blur reach line in `own_overflow` lands on the rewritten function) |
| Touches | `crates/designos-export/src/bounds.rs` (`own_overflow`, `:29-79`), `crates/designos-export/src/svg.rs` (call site `:687`, test only), `crates/designos-export/tests/` (one ignored kit test), `crates/designos-export/Cargo.toml` (dev-dependency on `designos-fig`), and, after calibration only, `crates/designos-render/src/lib.rs` (`checked_pixmap_size` `:147-157`) and `crates/designos-export/src/raster.rs` (`:91`). No hot file. |

Code baseline: `origin/main@0e8a9a5` (`docs(specs): v0.6 task specs, part 4 ... (#697)`). Evidence: REA 2026-10-19 (Figma export size read through the Ease Design Figma Agent for the 10 frames; Figma's render-bounds function was not found in the wasm, so the rule below is fitted, not read from code).

## 1. Summary and user story

As a designer exporting a frame with shadows and strokes, I want the PNG and SVG to have the pixel size Figma gives, so a hand-off or a diff against Figma's export lines up to the pixel.

## 2. Current state (FACT)

- Export already grows to the render bounds: `render_bounds` (`bounds.rs:8`) feeds `render_export` (`raster.rs:86`), `export_svg_plain` (`svg.rs:51`), and the export panel preview (`panels/export.rs:171`). `own_overflow` (`bounds.rs:29`) is also the SVG shadow filter region (`svg.rs:687`), so one fix reaches PNG, SVG and the filter box.
- `own_overflow` keeps four running maxima `(l, t, r, b)` and ceils each side at the end (`bounds.rs:30`, `:78`).
  - Stroke: gated by `side_weights.is_some() || weight > 0` (`:39`), outset taken from `n.stroke.weight` only (`:41-45`: Inside 0, Center w/2, Outside w), then merged with `l.max(out)` and the same for the other three sides (`:46-49`).
  - Drop shadow: `reach = radius.max(0) + spread.max(0)` (`:54`), then `l.max(reach - offset.x)`, `r.max(reach + offset.x)`, `t.max(reach - offset.y)`, `b.max(reach + offset.y)` (`:55-58`).
  - Layer blur and a non-clipped texture add their radius to every side (`:60-74`).
  - Inner shadow, background blur, glass and noise add nothing (they paint inside the box).
- Three causes of mismatch follow from those lines (I re-derived the two data points by hand, below).
  1. Negative spread is clamped to 0 (`:54`).
  2. The stroke outset and the shadow reach are merged with `max` (`:46-49`, `:55-58`) instead of added.
  3. The stroke gate uses `side_weights` (`:39`) but the outset uses `weight` (`:43-44`): a stroke that only has per-side weights has outset 0.
- Hand check of the issue's table, from the lines above:
  - `_Date picker menu` 328 x 540, shadow offset (0, 20), blur 24, spread -4, 1 px outside stroke: Studio left/right = max(1, 24) = 24, top = max(1, 24-20) = 4, bottom = max(1, 24+20) = 44, so 376 x 588 (the issue's Studio value). Figma's 370 x 582 equals the additive rule in section 3 (21, 1, 21, 41).
  - `_Footer social proof badge` 304 x 36, shadow (0, 1), blur 2, spread 0, 1 px outside stroke: Studio 308 x 40 (the issue's value); additive rule gives sides (3, 2, 3, 4) so 310 x 42, Figma's value.
- PNG pixel size: `checked_pixmap_size` does `ceil(b.w * scale)` and `ceil(b.h * scale)` (`designos-render/src/lib.rs:147-157`). Two entry points use it: `render_export` after the bounds (`raster.rs:91`) and `render_node` for an unchanged box (`lib.rs:164`; `render_export` returns it at `raster.rs:88-90`). `designos-render/tests/pixmap_size_cap.rs` and `designos-export/tests/pixmap_size_cap.rs` cover the cap, not the rounding.
- Existing tests that must keep passing: `shadow_extends_bounds_with_offset` (116 x 56) and `outside_stroke_and_clipping` (`bounds.rs:125-151`), `png_includes_shadow_overflow` (`designos-export/src/lib.rs:180-200`). All of them use non-negative spread and either a shadow or a stroke, never both, so the additive rule leaves them unchanged (I checked each by hand).
- `designos-export` has no dependency on `designos-fig` (`crates/designos-export/Cargo.toml`), and `designos-fig` depends on `designos-doc` only (`crates/designos-fig/Cargo.toml:14`, `:25`), so a dev-dependency from export to fig adds no cycle.
- FACT (REA 2026-10-19, disassembly of the Figma export path): Figma sizes a PNG as round-half-away-from-zero of (scale x the integer 1x size).
- UNKNOWN: how Figma derives that integer 1x size from its fractional bounds (floor, ceil or round of each side). Closing check: the calibration in section 10 (owner-manual).
- UNKNOWN: Figma's render-bounds code was not found; the rule is fitted from two shadowed frames. Closing check: the live read in section 10.
- Not reproduced: 8 of the 10 frames could not be reproduced (the kit file was not on the research machine and the Figma plugin was not connected).

## 3. Behaviour rules

Root causes and the acceptance of each (hypotheses until the kit test and the live read run; frames are from the issue table):

| # | Cause (FACT in code) | Location | Frames it should explain | Status | Acceptance |
|---|---|---|---|---|---|
| 1 | negative spread clamped to 0 | `bounds.rs:54` | `_Date picker menu` `11627:254713` (+6 x +6); probably `_Video timestamp indicator` `9264:578567` and `Settings pages 02 Mobile` `6277:293415` (INFERENCE) | hand-checked on the date picker | the 328 x 540 fit frame exports 370 x 582 |
| 2 | stroke and shadow merged with `max` instead of added | `bounds.rs:46-49`, `:55-58` | `_Footer social proof badge` `12469:575669` (-2 x -2) | hand-checked | the 304 x 36 fit frame exports 310 x 42 |
| 3 | stroke gate reads `side_weights`, outset reads `weight` | `bounds.rs:39` vs `:43-44` | none known | untested | per-side weights give a non-zero outset |
| 4 | `ceil` instead of round-half-away on the PNG size | `raster.rs:91`, `designos-render/src/lib.rs:151` | fractional scales only | needs the calibration | after calibration, 0.5x and 1.5x on odd sizes equal Figma's integers |
| 5 | unknown | none | `Dashboards 01 Desktop` `8085:173260` (-84 x -1), `Context menu` `11547:540792` (-9), `Text highlight` `9291:558536`, `_Text editor icon` `9291:551743` (+3), `Informational pages 01 Desktop` `1689:439089` and `Settings pages 01 Desktop` `1673:402973` (+2) | hypotheses: text glyph overflow, rotation, masks, clip semantics | the per-descendant read names one cause per frame before any code |

The fitted rule, in words: for each of the four sides, take the stroke outset on that side plus the shadow reach on that side, clamp the shadow part at zero, and keep the largest value over all effects; then union with the children.

1. **Stroke outset** (the same on every side): Inside 0, Center half the weight, Outside the full weight. The weight is the largest of `side_weights` when it is set, else `weight` (cause 3). The gate is "a visible stroke paint and a positive outset weight", so the gate and the outset read the same number.
2. **Shadow reach per side**: `blur + spread - offset` for left and top, `blur + spread + offset` for right and bottom, using the signed offset on that axis. The spread is not clamped. The sum is then clamped at 0 once, per side: `max(0, blur + spread -/+ offset)` (cause 1).
3. **Add, do not merge** the two: `side = stroke_outset + shadow_reach` (cause 2). With several drop shadows the sides take the largest shadow reach (union); strokes are counted once.
4. **Layer blur and texture** keep their current radius rule on every side and combine by `max` with the shadow part, then add the stroke outset like any other effect (INFERENCE: same additive shape; no fit data for blur; closing check in section 10).
5. **Per-side ceil is kept** (`bounds.rs:78`) so the box grows in whole pixels per side.
6. **Children**: a clipped frame counts its own effects only; an unclipped frame unions its children's bounds as today (`bounds.rs:93-104`). A rotated child contributes the bounding box of its transformed bounds (unchanged).
7. **PNG rounding stays `ceil` until calibration.** After the calibration of section 10 the size rule changes in `checked_pixmap_size` to Figma's measured one, in its own commit, with its own tests. Not before: the integer 1x size is unknown, so changing only the final rounding would be a guess.
8. **Cause 5 is not fixed by a guess.** The six frames with an unexplained difference get a named cause from the per-descendant read in section 10 before any code changes (hypotheses on record: text glyph overflow, rotation, masks, clip semantics).

## 4. Data model / schema changes

None. `own_overflow` keeps its signature `(l, t, r, b)` (it is `pub` and used by `svg.rs:687`).

## 5. Contracts

- `render_bounds` and `own_overflow` stay the single source for PNG, SVG, the SVG shadow filter box and the export preview.
- Public export functions keep their signatures; only the numbers change.
- `#37` adds one `match` arm to `own_overflow` for the progressive blur reach after this task; keep the match shape.

## 6. File-format mapping

None (export only). SVG viewBox and the shadow filter region follow `own_overflow` automatically.

## 7. UI

No UI change. The export panel preview and size readout (`panels/export.rs:171`) show the new size; no new control. Desktop capture at 1440 in the PR: the export panel for the `_Date picker menu` fixture before (376 x 588) and after (370 x 582). Responsive: SKIPPED (owner default). Load es-designer only if a UI file changes (none expected).

## 8. Edge cases

- Negative spread larger than the blur: `max(0, ...)` gives 0 on that side; the box does not shrink below the node box.
- A shadow whose offset pushes it away from a side: that side gets only the stroke outset (the `max(0, ...)` case).
- Stroke align Inside with a shadow: outset 0, shadow part only.
- Per-side weights `(1, 4, 2, 8)` with Outside alignment: every side uses 8 under rule 1 (ASSUMPTION; Figma may use each side's own weight, see the open question).
- Two shadows: sides are the largest of the two reaches, not their sum.
- Node with no stroke paint but a stored weight: no outset.
- Fractional values: per-side ceil happens after the additive sum, so 0.4 + 0.4 = 0.8 rounds up to 1 once, not twice.
- A clipped frame: its own shadow counts, its children's do not; an Outside stroke of a clipped frame's child never counts.
- Page nodes: `bounds_of` skips `own_overflow` for a page (`bounds.rs:82-88`); unchanged.

## 9. Non-goals

- Changing how the image is painted inside the new box (shadow shape, blur kernel): #520.
- Inner shadow, background blur, glass and noise reach (they stay inside the box).
- Figma's SVG export syntax (`svg.rs` structure); only the viewBox and filter region size are in scope.
- A fix for the six frames of cause 5 before their cause is named.

## 10. Test plan (red first)

Seams: `designos-export` unit tests in `bounds.rs`; one ignored kit test; the shared synthetic fixtures below.

| Case | Expected | Fails before |
|---|---|---|
| `date_picker_fit_328x540_negative_spread` | 328 x 540, shadow `0 20 24 -4`, 1 px outside stroke, exports 370 x 582 | yes (376 x 588) |
| `footer_badge_fit_304x36_additive_stroke_and_shadow` | 304 x 36, shadow `0 1 2 0`, 1 px outside stroke, exports 310 x 42 | yes (308 x 40) |
| `negative_spread_larger_than_blur_adds_nothing` | blur 4, spread -6: sides equal the stroke outset only | yes |
| `each_stroke_alignment_adds_0_half_or_full` | Inside 0, Center w/2, Outside w, no shadow | guard |
| `stroke_and_shadow_add_not_max` | 1 px outside stroke plus shadow `0 0 8 0`: every side 9 | yes |
| `two_shadows_take_the_union_not_the_sum` | shadows `0 4 8 0` and `0 0 12 0`: every side is 12 (not 24, not 12 + 8) | guard |
| `per_side_stroke_weights_gate_and_outset_agree` | `side_weights = (1, 4, 2, 8)`, weight 0, Outside: outset 8 on every side (gate true, outset non-zero) | yes (gate true, outset 0) |
| `clipped_frame_counts_own_shadow_not_children` | clipped frame with own shadow and a child with a big shadow: bounds equal own shadow only | guard |
| `child_rotated_30_degrees_contributes_its_bounding_box` | rotated child's shadow grows the union by the bounding box | guard |
| `scales_0_5_and_1_5_on_odd_sizes_keep_ceil` | 101 x 51 at 0.5 and 1.5: sizes follow `ceil` (this pins today's rounding until calibration) | guard |
| `shadow_extends_bounds_with_offset`, `outside_stroke_and_clipping`, `png_includes_shadow_overflow` | unchanged | guard |
| `render_bounds_svg_filter_region_matches_own_overflow` | the SVG shadow filter box equals the node box plus `own_overflow` | guard |
| kit test (ignored): the 10 frames | each exports within 1 px of the Figma size recorded in the issue table, 1x, PNG and SVG | yes |

Kit test: `crates/designos-export/tests/` file that reads the kit through the same `reference()` search as `designos-fig/tests/real_files.rs:12` (`DESIGNOS_REFERENCE_DIR`, then `reference/` of the checkout or an ancestor, then `$HOME/Downloads`); it fails when the variable is set and the file is missing, and prints a reason and returns when the variable is unset.

Run: `cargo test -p designos-export`, then `cargo test -p designos-export --test <kit file> -- --ignored` with the kit available. Evidence: red/green capture, the before/after export sizes of the two fitted frames, and the kit test table for the frames that pass.

Owner-manual checks (flagged, never claimed by an agent). Both need a live Figma file and the Untitled UI kit:

- **Per-descendant bounds read (closes cause 5 and the INFERENCE in rule 4).** For each of the 10 frames, read the render bounds of every descendant through the plugin API and diff against Studio's `bounds_of`. About 10 minutes per frame. The result names the cause of the six unexplained frames.
- **Calibration rectangles (closes rule 7 and the 100 x 40 case).** About a dozen rectangles with known sizes, strokes and shadows, including a 100 x 40 rectangle with one drop shadow at offset (0, 4) and blur 8 (the issue's own acceptance case; the rule predicts 116 x 56, INFERENCE), exported at 0.5x, 1x and 1.5x. This writes nodes into a Figma file, so it needs the owner's approval first.

## 11. Performance budgets

- `own_overflow` runs once per node per export; no allocation; the change is a few additions. No measurable change in `render_bounds` on a 100,000-node page (recorded in the PR with `cargo test -p designos-export` timing, not averaged).
- Export memory: unchanged except where the box now shrinks (a negative spread frame is smaller than before).
- No UI-thread work added: the export panel already calls `render_bounds` per fingerprint change (`panels/export.rs:171`).

## 12. Dependencies and build order

No task dependency for causes 1 to 3. Land in two commits: the additive per-side rule with its unit tests, then the ignored kit test. Rounding (rule 7) and cause 5 follow the owner-manual reads and are separate PRs or commits. Land before #37's bounds line (README-part2). No hot file.

## 13. Owner rulings

- Owner ruling 2026-10-18: exports match Figma's render bounds (issue).
- A unit test pins the shadow reach to Figma's rule; the existing `bounds.rs` tests keep passing (issue acceptance criteria).
- Delivery law 3 (count before you target): the headline "10 frames" is a hypothesis; causes 1 and 2 explain two named frames, and the other frames are expected, not proven, until the kit test runs.

## Open questions for the owner (recommended first)

1. Approve the calibration rectangles (a dozen nodes written into a scratch Figma file). Recommended: yes, one scratch page, deleted afterwards; it is the only way to pin the rounding rule and the 100 x 40 case.
2. Per-side stroke weights with Outside alignment: use the largest side weight on every side (this spec), or each side's own weight. Recommended: the largest, until the live read shows otherwise; per-side outset with different weights is rare.
3. If cause 5 frames turn out to need a text-glyph or mask rule that touches the renderer, split them into a new task. Recommended: yes, file it with the read's evidence rather than stretching this one.

## 14. Facts resolved by REA 2 (2026-10-19)

Static reading of Figma's fullscreen wasm and app bundle. No Figma code was copied. Where a line above says UNKNOWN or INFERENCE and this section says FACT, this section wins.

- FACT: render bounds = own box (only when the node has a visible fill or stroke) ∪ true stroke outline (vector-like nodes) ∪ text ink bounds ∪ children (opacity ≤ 0 skipped; clipped by a clipping container) ∪ effect rects. The result is then intersected with every clipping ancestor. A hidden node, or a node with an ancestor at opacity ≤ 0, has no bounds (null).
- FACT: a drop shadow rect is the source bounds moved by the offset and inflated by radius + spread on every side, unclamped, then unioned with the source. Stroke and shadow therefore add up. This rule reproduces 370×582 and 310×42 exactly.
- FACT: background blur inflates by its radius (for a progressive blur, the larger of radius and start radius), and is rotation- and scale-aware. Texture inflates by its radius. Inner shadow, noise, glass and custom effects do not inflate the bounds.
- Cause 5 candidates (INFERENCE, to confirm on the kit): fill-less non-clipping frames that drop their own box, children clipped by a clipping container, children at opacity 0, text measured by ink, and strokes measured by the true outline.
- UNKNOWN: how the integer 1x size is derived from fractional bounds. PNG pixel size stays round-half-away(scale × integer 1x).
