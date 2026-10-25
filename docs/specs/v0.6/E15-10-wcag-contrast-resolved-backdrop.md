# E15-10 Check WCAG contrast of text against its resolved backdrop

| Field | Value |
|---|---|
| Epic / pack | E15 / v0.6 AI design |
| Size / priority | M / P0 |
| Depends on | E15-9, #158 (thresholds) |
| Unblocks | E15-13 (contrast stays suggest-only), part-2 critique metrics |
| Touches | new `crates/designos-doc/src/lint/rules/contrast.rs`, `crates/designos-render/src/lint_env.rs` (backdrop sampling), new `render_backdrop` in `crates/designos-render/src/lib.rs` |

## 1. Summary and user story

As a designer or agent, I learn whether text is readable against what is actually painted behind it
(gradients, images, translucent overlays, stacked layers, blend modes), with the measured worst-case
ratio, the required ratio, and a passing colour variable when one exists.

## 2. Current state (FACT)

- Legacy 17-rule linter: `low-contrast-text` compares the text's top solid fill against the first
  ancestor solid fill with alpha × opacity ≥ 0.5, else the page background, else white
  (`effective_background_color`, line 729-748 of the legacy module in `crates/designos-doc/src/`);
  luminance uses the 0.03928 threshold (line 713). Gradients and images are ignored.
- Check designs contrast (`crates/designos-app/src/check_designs/contrast.rs:1-21`) models ancestor
  solid fills and group opacity exactly but skips text whenever a gradient, image, non-normal blend,
  a sibling painted behind, or a stroke band is involved ("neither reported nor fixed"). It uses the
  0.04045 threshold (`contrast.rs:35-41`) and `required_ratio` 3:1 for ≥ 24 px or ≥ 18.66 px bold,
  else 4.5:1 (`contrast.rs:56-62`).
- The CPU renderer can render any page region (`render_page` with a viewport offset,
  `crates/designos-render/src/lib.rs:91`; used that way in `crates/designos-export/src/raster.rs:94-96`).

## 3. Behaviour rules

### 3.1 Formulas

- Channel linearisation (sRGB, WCAG 2.x): `c' = c / 12.92` if `c ≤ 0.04045`, else
  `((c + 0.055) / 1.055)^2.4`, with `c` in 0..1. (The legacy 0.03928 constant gives identical results
  for every 8-bit value; 0.04045 is used everywhere from now on.)
- Relative luminance `L = 0.2126 R' + 0.7152 G' + 0.0722 B'`.
- Contrast `(L_light + 0.05) / (L_dark + 0.05)`, range 1..21.
- Large text: font size ≥ 24 px, or ≥ 18.66 px with weight ≥ 700.
- Required ratio: AA 4.5 (large 3.0); AAA 7.0 (large 4.5). Level from config `contrast_level`
  (`AA` default, `AAA`), per #158.

### 3.2 Measuring the backdrop

For each visible text node with any visible fill:

1. Fast path (no rendering): if every layer that paints under the text's box is a solid fill with
   normal blending fully covering the box, and no ancestor uses a non-normal blend, the backdrop is
   the analytic composite of those fills (the current Check designs model). Text fill solid →
   one ratio.
2. Pixel path (`LintEnv::backdrop`), otherwise. Sampling rect = the text's page-space box. Sampling
   scale = 1 sample per page unit, reduced so the sample grid is at most 256 x 256. Three CPU renders
   of that rect, all of the full page in paint order:
   - `B`: everything except the text node (backdrop as seen around the text, overlays above included);
   - `C`: everything (what the user sees);
   - `M`: the text node alone on transparent, giving glyph coverage `alpha`.
   For every sample with `M.alpha ≥ 0.75` (glyph interior), ratio_i = contrast(C_i, B_i), both
   flattened onto opaque (they are full-page renders, so they are opaque wherever the page paints).
3. Worst case = minimum ratio_i over glyph-interior samples. Fewer than 4 interior samples (tiny or
   empty text): fall back to all samples with `alpha > 0`.
4. Text fills that are gradients or images are covered automatically by `C`.

`render_backdrop(doc, page, hide: &[NodeId], rect, scale) -> Option<Pixmap>` is added to
designos-render; it renders `rect` of `page` with the given nodes skipped, using the same code path as
`render_page`, so results are bit-exact with exports.

### 3.3 Finding

Rule id `contrast` (alias `low-contrast-text`), category `accessibility`, severity error for AA
failure, warning for AAA-only failure when level is AAA. Fields beyond E15-9's: `measured_ratio`
(2 decimals), `required_ratio`, `level`, `large`, `method` (`analytic` | `pixels`),
`worst_sample` (page-space point), `foreground_hex`, `backdrop_hex` (colours at the worst sample).

### 3.4 Passing variable pair

When the text fill is bound to (or exactly equals, per E15-11) a colour variable, the rule searches
colour variables of the same collection (all modes resolved in the node's effective mode) for one
that passes against the worst-sample backdrop. Choice: smallest CIE76 ΔE from the current colour
(Lab conversion as the legacy analyser, line 1211-1247). When the backdrop's top fill is also bound,
the pair search additionally tries backdrop variables with the text colour fixed, and reports the
better of the two as `suggested_fix: {kind:"bind", property:"fills/0", to:"var:..."}`. The fix is
suggest-tier only (E15-13: contrast never auto-applies).

## 4. Data model / schema changes

- Finding extension fields above (serde, only present on `contrast` findings).
- `LintConfig.contrast_level: ContrastLevel` (default AA).
- No document change.

## 5. Contracts

Surfaces through `designos_lint` (E15-9), the issues block (E15-12) and Check designs. Example
finding:

```json
{"rule":"contrast","severity":"error","node_id":301,"node_name":"Hero caption",
 "measured_ratio":2.87,"required_ratio":4.5,"level":"AA","large":false,"method":"pixels",
 "worst_sample":{"x":412,"y":88},"foreground_hex":"#C8C8C8","backdrop_hex":"#E9E2DA",
 "message":"Contrast 2.87:1 is below AA 4.5:1 over the image behind it.",
 "suggested_fix":{"kind":"bind","property":"fills/0","from":"#C8C8C8","to":"var:Color/Text strong"},
 "fixable":false}
```

## 6. File-format mapping

None.

## 7. UI

Check designs lists contrast rows as today, now including text over gradients and images, and shows
`measured / required` in the row message. The existing one-click colour fix applies only to the
analytic path as today; pixel-path findings show the suggested variable as a fix button (applies a
bind, one undo step, user-initiated).

## 8. Edge cases

1. Text fully covered by an opaque layer above it: `C` equals `B` in glyph interiors → ratio 1:1,
   reported as `contrast` with message "text is covered by <layer name>" (it is unreadable).
2. Text with opacity 0 or inside a hidden ancestor: skipped (not painted).
3. Transparent page background with nothing behind: backdrop is the page colour (renderers paint it).
4. Emoji or colour glyphs: their pixels are part of `C`; worst case may be driven by the emoji;
   accepted, message names the sample point.
5. Rotated text: the sampling rect is the rotated box's AABB; coverage mask restricts samples to
   glyphs.
6. Text larger than 256 x 256 samples: downsampled grid; thin strokes may lose interior samples,
   handled by rule 3.2.3 fallback.
7. Mixed runs with different colours: covered pixel-wise; the worst run sets the result.

## 9. Non-goals

- Non-text contrast (icons, borders, WCAG 1.4.11).
- APCA or WCAG 3 metrics.

## 10. Test plan

Seam: designos-render integration tests (`crates/designos-render/tests/lint_contrast.rs`) with rendered
fixtures; analytic fast-path unit tests in designos-doc.

| Case | Fixture | Expected |
|---|---|---|
| `grey_on_gradient_fails_where_old_check_passed` | frame fills `[solid #000000, linear gradient #FFFFFF→#DDDDDD on top]`, text `#CCCCCC` 14 px | legacy rule passes (picks black, 13:1); new `contrast` fails with ratio < 1.5 |
| `overlay_on_image_within_0_05` | image fill (2x2 checker #3060A0/#F0E0C0 scaled), overlay rect `#000000` at 40% above, text `#FFFFFF` 16 px | worst ratio equals the analytic reference for the lighter checker cell under 40% black, within 0.05 |
| `large_text_uses_3_to_1` | text 24 px `#949494` on `#FFFFFF` (ratio 3.03) | passes AA (3:1); the same text at 23 px fails (needs 4.5) |
| `bold_18_66_is_large` | 18.66 px weight 700 | required 3.0 |
| `aaa_level` | same as large case with level AAA | warning, required 4.5 |
| `variable_pair_suggested` | text bound to `Color/Text muted` failing; collection has `Text strong` passing | `suggested_fix.to == "var:Color/Text strong"` |
| `analytic_path_no_render` | solid backgrounds only | `method == "analytic"` and the env's render counter is 0 |

Reference values are computed in the test from the formulas in 3.1, not from the renderer.

## 11. Performance budgets

- Analytic path: under 10 µs per text node.
- Pixel path: three renders at ≤ 256 x 256 samples, under 4 ms per text node release.
- Per scan, at most 2,000 pixel-path evaluations per `Budget` slice; the rest continue in the next
  slice (E15-9 cursor), so the off-thread scan of the large fixture keeps its recorded budget and the
  UI thread is never involved.

## 12. Dependencies and build order

After E15-9 (registry, env trait) and #158 (thresholds and level setting). Parallel with E15-11. The
PR that registers `contrast` also appends it to E15-9's built-in `agent` preset and extends
`presets_name_only_registered_rules`.

## 13. Owner rulings

- Auto-fix (2026-10-18): contrast changes alter visible colour, so they are suggest-tier only and are
  never auto-applied (E15-13 acceptance 4).
- Full scope: gradients, images, overlays, blend modes and the variable pair ship together.
