# Review of the "True Figma Implementation Specs" (E4 draw, E5 effects, E6 motion and export, E7 Dev Mode)

Date 2026-11-12. Read-only review. Code checked against `origin/main` at `b731e4359`. Evidence files opened: the repo Kiwi schema `crates/designos-fig/tests/data/figma-real-schema.txt` (4804 lines), the extracted Figma app files (`plugin-api.d.ts`, `figma_app-6372f0021b0d5c94.min.en.json`, `wasm-strings.txt`, `bound-variants/js/39902-1677a713e4a837a0.min.js`), and the dossiers in [evidence](../evidence/). Issue states from `gh issue view` on 2026-11-12. Pull requests 1946, 1947 and 1950 were merged after this review was written and are not reflected below.

Counts are of references the reviewer checked one by one. "Proposed new" names (types the spec asks you to add) are not counted as not found. Grades: SOURCED = the reviewer opened the evidence and it says it; ASSERTED = no checkable source; CONTRADICTED = repo code, tests, docs, or the evidence itself disagrees.

## Verdict table

| Spec | Verdict | One-line reason | Code refs: found / elsewhere-renamed / not found | Figma facts: sourced / asserted / contradicted |
|---|---|---|---|---|
| E4 basic shapes, multi-stroke, vector tools | READY AFTER FIXES | Three of eight slices already shipped (inside/outside strokes, mask types, offset path); the GPU "stencil, zero programs" claim is false | 14 / 6 / 7 | 15 / 8 / 3 |
| E4 draw engine (variable width, trim, repeat, pattern, brush, text on path) | NEEDS EVIDENCE | Kiwi fields are real, but every formula (width presets, hex tiling, glyph placement, caps) has no source | 9 / 2 / 3 | 14 / 14 / 1 |
| E5 image filters, progressive blur, glass, noise, shadow-behind | SUPERSEDED except progressive blur (READY AFTER FIXES) | Image filters, glass splay and bevel, noise size, shadow-behind are on main; #521 and #1328 are the wrong issues | 9 / 3 / 3 | 14 / 6 / 3 |
| E5 video, custom shaders, mesh gradient, 3D | NEEDS EVIDENCE | Conflicts with the shipped video-fill record and shader reference; "36 programs" arithmetic is meaningless; Euler order unknown | 6 / 2 / 3 | 12 / 8 / 2 |
| E6 export bounds, export settings, easing and springs | READY AFTER FIXES | Schema and easing curves are sourced; bounds "facts" cite Studio's own code, settle-time and test numbers are wrong | 9 / 4 / 4 | 14 / 5 / 4 |
| E6 motion, timeline, Lottie, video export | SUPERSEDED for the model and evaluator; READY AFTER FIXES for lint and stagger; NEEDS EVIDENCE for Lottie output | `evaluate_motion` and the motion model already exist with different types | 5 / 3 / 3 | 12 / 5 / 2 |
| E7 annotations, measurements, Code Connect, MCP | READY AFTER FIXES | The 33-value enum is sourced; three REST names are wrong; contradicts issue #259 and ruling R6.21 | 8 / 2 / 1 | 14 / 6 / 3 |

Slice order comes from the READMEs: E4 queue is 9 slices on the Kiwi hot files, E5 is 8, E6 is 4. No README names an owner for the hot-file queue position that #281 already holds (position 40).

## What the review could not verify

1. Any "Figma does X" that rests on `plugin-api.d.ts` lines the reviewer did not open (inside/outside stroke doubling at 8753-8766, filter sliders at 4720-4728, wasm-strings line 18119). The lines for progressive blur, variable width, export bounds, and the UI strings were opened.
2. The REA evidence IDs (`ev_6f56...`, `ev_d8d8...`) and the "Figma 126.11.7" version. The dossiers state them; the REA store was not available to the reviewer.
3. The minified-JS offsets the specs quote (14850000, 14909911). The same code was found by searching, not at those exact offsets.
4. Whether Figma caps repeat count at 2..40 and stacks at most two modifiers. The cited `rea-findings.md:113` does not say either (issue #189 says "up to two", which is the only support).
5. The "max 8 drop shadows, 1 layer blur" inspector caps. Not in the dossier.
6. No cargo was run. Every statement about GPU cost (programs, passes, command buffers, idle CPU) is read from code, not measured.
7. No Figma-authored file with these features (variable width, brushes, progressive blur, annotations, motion) was used by any spec. The "Test Vectors from Figma" tables are values derived from the specs' own formulas, not exported from Figma.
8. Line numbers: the specs cite `figma-real-schema.txt` lines. About half do not match the repo copy (its checksum differs from the dossier's `660bf4f8...`), so the writers used another copy. Field numbers all match; only the line numbers drift.

## Cross-cutting findings

- The dossiers say main lacks features it already has (mask type, glass splay, image adjust). They were written against a stale picture of the code, and the specs inherit that.
- The Kiwi layer is the strong part: 130 of 131 schema field numbers and enum values that were grepped match exactly. The one miss is `pointCount = 23` (not a NodeChange field in the repo schema; the importer reads `count` or `pointCount`).
- The spec text calls itself "Build-Ready" and "True Figma" while about a third of the formulas are inventions presented under a FACT label.
- Several specs reuse the phrase "zero new offscreen GPU Programs, < 64 cap". In `compile.rs` a Program is a per-frame offscreen target (`self.progs`), counted by `prog_cap_hit(48)`; at idle the refine compile swaps the count for a 512 MiB VRAM budget. A static count of shader pipelines (E5 "8 + 12 + 16 = 36") says nothing about that cap.

---

<a id="e4-basic-shapes"></a>

## 1. Basic shapes, multi-stroke, vector tools in E4 ([`e4-basic-shapes-strokes-vector-tools-spec.md`](../e4-draw-vector/e4-basic-shapes-strokes-vector-tools-spec.md))

### a. What it says to build
- Inside and outside stroke parity (#952, slice P1.1): double the weight and clip against the fill.
- Ellipse arc data, star inner scale, line resize (#953, slice P1.2).
- Vector mask types alpha, outline, luminance (#1506, slice P1.3).
- Multi-stroke list with OFFSET alignment, per-side weights (#196, #197, slice P2.1).
- Eraser tool (#193, slice P2.2), cut tool (#194, slice P2.3), offset path tool (#499, slice P2.4).
- Kiwi encode and decode code for arc, mask, strokeData.

### b. Code references
Found exactly: `crates/designos-render/src/outline.rs` with `StrokePlan { outline, clip }` and `StrokeClip` (lines 110-125), `NodeKind::Ellipse(ArcData)` (node.rs:531), `Node.is_mask`, `n.stroke.paints.clear()` in `mask_stencil` (mask.rs), `encode_paint`, `fl`, `VectorNetwork`, `NodeKind::Vector`, `Stroke` fields, `designos-render/src/paint.rs`, `designos-gpu/src/geom.rs`, `kiwi.rs`, `rest_import.rs`, AGENTS.md section 4.
Found elsewhere or renamed: `canvas/tools/` (it is the file `canvas/tools.rs`), `ArcData` fields (`start`, `end`, `inner_radius`, not `starting_angle`), `MaskType::Outline` (the repo name is `MaskType::Vector`), `is_default_mask_type` (repo: `MaskType::is_default`), `convert_node` (exists in fig.rs but returns a JSON map, not a Node), `fnum` (rest_import.rs, `pub(crate)`), `msg`/`en` (testkit).
Not found: `encode_node_change`, `stroke_align_name`, `stroke_cap_name`, `evaluate_mask_alpha`, `adjacent_segments` on vertices, `Overlay::difference` (i_overlay 9 uses `OverlayRule` through `SingleFloatOverlay`), `split_vector` as a Rust symbol (it is a Figma action id), `cursor-eraser.svg` (a Figma asset name).
The inspect-panel line ranges belong to E7, not here.

### c. Figma facts
- SOURCED: `StrokeAlign` 0..3, `strokeOffset` 13 on Stroke and 554 on NodeChange, `arcData` 195, `starInnerScale` 24, `strokeData` 571 with `Stroke` fields, mask fields 16, 18, 317, per-side weights 295-299, tool action ids and strings ("Eraser", "Cut tool", "Offset", "Round", "Square", "Variable width stroke outlined for offset", all found in `en.json`).
- ASSERTED: the eraser result (insert vertices at cut points), cut-tool split at nearest t, the offset-stroke formula (shift along the normal), the 0.382 default star ratio, "Figma stores LINE as size (length, 0)", the 0.25 px chordal tolerance (the spec admits it is Studio's own choice).
- CONTRADICTED: (1) "OUTLINE mask strips strokes" is flagged INFERENCE by the spec, but the repo already does it and tests it. (2) "Luminance is exactly 0.2126R + 0.7152G + 0.0722B, bit-exact with Figma" against `mask.rs` line 9: "this formula ... is to be confirmed against a Figma render". The spec turns an open question into a FACT. (3) The GPU claim below.

### d. What already exists
| Slice | Main today |
|---|---|
| P1.1 inside/outside (#952, CLOSED, PR #1734) | Shipped. The spec's code block in section 3.1 is nearly verbatim `outline.rs` lines 315-317. SUPERSEDED. |
| P1.2 arc, star, line (#953, OPEN, v0.7.14) | Mostly shipped: arc handles and instance arc overrides (0.7.13, #1727), line stroke band on resize (PR #1752). Gap is what #953 says: resized imported stars, rings and lines differing from Figma. The spec does not describe that gap. |
| P1.3 mask types (#1506, OPEN) | `MaskType {Alpha, Vector, Luminance}` is on the node, imported and exported (`maskType` "OUTLINE"), and rendered (`mask.rs`). The open issue is "outline rules for groups, nested masks and strokes", which the spec does not touch. SUPERSEDED for the types. |
| P2.1 multi-stroke (#196, #197) | None: `Stroke` has no list. `strokeData` is only kept as a passthrough key (`own_entry_keys.rs`). Per-side weights already exist (`Stroke.side_weights`). Real work, but the spec lists per-side weights as new. |
| P2.2 eraser (#193), P2.3 cut (#194) | None; `Tool` enum has no Eraser or Cut. |
| P2.4 offset path (#499, #195) | Shipped in 0.5.0 (#195 CLOSED): `offset_paths(paths, delta, join, miter_limit)` in `vectorops.rs`, `offset_path_selection_with` in `designos-app/src/vecops.rs`, with tests. Open #499 is "stroke-only contours, open-path caps, multi-selection reflow and side weights". Studio joins are Round, Bevel, Miter; the spec says Round and Square (that is the Figma UI label, but the mapping is not stated). |

### e. Issue state
Issue #952 is CLOSED (title matches the symptom, not the "stencil" design). Issue #953 is OPEN v0.7.14. Issue #1506 is OPEN v0.7.14. Issues #196, #197, #193, #194, #499 are OPEN, milestone "v0.21 Edit vectors". Epic #17 is OPEN. The spec's milestone "v0.8.0" matches none of them.

### f. Risks against the invariants
- The GPU section is wrong. `designos-gpu` has `depth_stencil: None` in every pipeline (`pipelines.rs`, `present.rs`). Inside and outside strokes use `mask_clip(...)` in `compile.rs` (`draw_strokes`, lines ~1250-1280), which opens an offscreen mask Program per stroked node, with a fallback to no clip when `prog_cap_hit(48)` or at low zoom. A multi-stroke list with M inside or outside items would open M mask Programs per node, so it hits the cap faster and degrades to unclipped strokes under load. This contradicts "zero new Programs" and needs a design (one shared mask per node for all items, or CPU-tessellated inside geometry).
- Metal command-buffer limit: each extra mask Program is extra render passes; the spec does not count them.
- Eraser and cut change vector networks on the UI thread: keep them as one `Command` per gesture; do not call `app.changed()` per mouse move.
- `doc_hash`: the new `StrokeItem` list must be added to the streaming `HashSerializer`, not hashed through JSON.

### g. Verdict: READY AFTER FIXES
1. Drop P1.1 and the mask-type part of P1.3 (shipped); keep #1506's real scope (groups, nested masks).
2. Rename to repo names (`MaskType::Vector`, `ArcData.start/end`) or state the rename.
3. Replace the GPU stencil paragraph with the mask-Program accounting above.
4. Mark luminance as unconfirmed, as `mask.rs` does.
5. Use the repo's decode path (`copy(&mut m, nc, ...)` on JSON maps) instead of constructing `Node` in `fig.rs`.
6. Add the issue numbers and sizes to the multi-stroke slice only after #196's acceptance criteria are read.

---

<a id="e4-draw-engine"></a>

## 2. Draw engine in E4 ([`e4-figma-draw-variable-width-repeat-pattern-brushes-text-path.md`](../e4-draw-vector/e4-figma-draw-variable-width-repeat-pattern-brushes-text-path.md))

### a. What it says to build
Variable-width strokes with seven presets and an enforcer (D1); path trim (D1); transform group with repeat, symmetry, skew, at most two modifiers (D2); pattern fills with three tile layouts and a tile cache (D2); scatter and stretch brushes with a seeded PRNG (D4); text on path (D4). Issue labels are "D1, D2, D4", not numbers. The README points at #179-#206.

### b. Code references
Found: `Node`, `NodeKind`, `PaintKind` (enum at paint.rs:412), `TextAlignH`, `TextAlignV`, `TextProps`, `IMG_TILE` (`designos-gpu/src/frame.rs`), `GeomCache`, `frame_build_nanos`, `lyon` (via `mesh.rs`), cosmic-text 0.19, `NodeId`.
Elsewhere or renamed: `Stroke.dashes` (exists; the enforcer snippet is proposed code), `Easing`-style names not used here.
Not found: `adjacent_segments` (vertices carry no adjacency list; use segment degree), `designos-gpu/src/camera.rs` is cited by E5, not here. Everything else (TransformModifier, PatternPaint, BrushType, SplitMix64, TextPathProps) is proposed new.

### c. Figma facts
- SOURCED (schema checked): `variableWidthPoints` 447 (`position`, `ascent`, `descent`, `segmentId`), `pathTrim` 12 and 542 (`OptionalVector`), `TRANSFORM` 48, `transformModifiers` 455, pattern fields 26, 28, 29, 30, 37, `BRUSH` 46, `brushType` 452, `strokeSeed` 482, the three settings messages, `TEXT_PATH` 41, `textPathStart` 432. Width-profile names and the invariant strings are real (`plugin-api.d.ts:8836`, `en.json`, `FGPathTrim`, `VariableWidthPointsInvariantEnforcer`, `OffsetPathBehavior` are all in `wasm-strings.txt`).
- CONTRADICTED: the schema `TransformModifier` also has `axis` (X, Y, X_AND_Y), `unitType` (PIXELS, RELATIVE) and `order` (FORWARD, REVERSE). The spec's struct drops all three and the symmetry maths ignores the axis. The plugin typing says a variable-width point is `{position, width}` where width is a fraction of the stroke weight; the spec's "ascent = normal offset left, descent = right" meaning is a guess about the Kiwi pair.
- ASSERTED: preset width curves (wedge = t*W, taper = sin(pi t)*W, eye = 4t(1-t)*W and the rest), Hermite interpolation for custom points, repeat count 2..40, the cap of two modifiers, hexagonal tiling offsets (0.75 row factor), scatter stepping "gap times stamp width", glyph placement and justify rules, 16-point Gauss-Legendre arc length, "GPU instancing draws 40 copies in one call". The spec itself says the PRNG is Studio's (fine). The dossier lists the scatter and stretch brush preset names; none feed a formula.

### d. What already exists
Nothing of the six features. `strokeData`, `variableWidthPoints`, `pathTrim`, `transformModifiers`, `textPathStart` survive only as passthrough keys (#174 CLOSED). The only brush hit is an unrelated REST key. Corresponding issues are all open: #180, #182 (variable width), #184 (trim), #188-#190 (transform group), #191, #192 (pattern), #199-#204 (brushes), #205, #206 (text on path); #179 (typed fields) and #198 (stroke sampling) are prerequisites the spec skips.

### e. Issue state
All of #179-#206 that matter are OPEN, milestones v0.10, v0.15, v0.28 or Backlog. Items #181 and #183 in that range are unrelated merged PRs. Issues #186 and #195 are CLOSED.

### f. Risks
- Repeat of 40 children with group opacity or blend would open up to 40 offscreen Programs if each copy is its own group; the spec says "tessellated once, drawn with instance_count N" but `exec.rs` draws one instance per call (`inst..inst + 1`). Needs a design that keeps the 48-Program interactive cap.
- Pattern fill "renders the source node offscreen into an LRU tile": on the GPU that is an offscreen Program per distinct source; the refine VRAM budget (512 MiB) applies.
- Variable-width and brush geometry are built into the mesh cache; they must respect the 8 ms `frame_build_nanos` budget with deferral through `stale_hits`, which the spec only names.
- Text on path: glyph shaping must not run on the UI thread each frame.

### g. Verdict: NEEDS EVIDENCE
Source first: width-profile curves and the meaning of ascent and descent (needs a Figma-authored file with known presets, or the plugin docs), repeat limits, hex tile geometry, scatter stepping, text-on-path alignment. Add `axis`, `unitType`, `order` to the modifier model. Name the issues (#180-#206) per slice.

---

<a id="e5-image-filters"></a>

## 3. Image filters, progressive blur, glass, noise, shadow-behind in E5 ([`e5-image-filters-progressive-blur-glass-noise-spec.md`](../e5-effects-shaders/e5-image-filters-progressive-blur-glass-noise-spec.md))

### a. What it says to build
Seven image-filter sliders with a CIELAB pipeline (#521); progressive blur with start and end radius and normalized offsets (#37); glass splay and bevel, 2D noise size, effect creation caps (#38); drop-shadow silhouette cut-out controlled by `showShadowBehindNode` (#1328).

### b. Code references
Found: `designos-gpu/src/shaders/fx.wgsl` (`fs_glass` at line 146, `bilinear`, `texel`), `designos-render/src/effects.rs`, `designos-app/src/panels/effects.rs`, `designos-doc/src/paint.rs`, `GlassEffect`, `NoiseEffect`, `rest_import.rs`, `convert_effect`, `encode_effect_full`, `parse_effect`.
Renamed: `NoiseEffect.size` is an f32 with `size_y: Option<f32>` (not a `Vec2`), `NoiseMode` is `Mono/Duo/Multi` (the spec's field `mode: NoiseMode` matches the name only).
Not found: `can_add_effect`, `EffectKind` (proposed new), `Effect::ProgressiveBlur` (proposed new), `hex_to_bytes`/`vec2` helper names in the fig_export snippets (`vec2` exists in testkit).

### c. Figma facts
- SOURCED: all schema fields (`blurOpType` 19, `startRadius` 21, `startOffset` 20, `endOffset` 28, `radius` 4, glass 30-36, `noiseSize` 22, `noiseType` 26, `showShadowBehindNode` 8, `splayVar` 41, `PaintFilterMessage` ids). Normalized offsets: `plugin-api.d.ts:4465-4482` says "normalized object space, (0,0) top-left, (1,1) bottom-right".
- CONTRADICTED: (1) The spec says the filter message has "7 sliders"; the schema message has ten (adds `detail` 4, `vignette` 6, `brightness` 10), and main already models all ten (`ImageAdjust`). (2) The spec's pipeline is a thinner copy of what `image_adjust.rs` already implements: main clamps contrast to +-0.3 (`CONTRAST_LIMIT`), mixes toward luminance and re-saturates in the exposure step (`apply_exposure_linear`), none of which the spec has; the spec's test V-E5-01 (0.18 to 0.957) would not match main's exposure step. (3) Splay: main documents it as "how far the bending direction turns from the edge normal toward the shape centre"; the spec's shader treats it as extra displacement distance along the normal (`splay_dist = splay * depth * 1.5 * t`), an invented formula.
- ASSERTED: `[FACT - seen: wasm-strings.txt:GLSL-uniform-block]` has no real locator (the same constants are in `image_adjust.rs` with the comment "used by Figma's fragment shader", so the source was real earlier); the 4-level mip pyramid and `lambda = log2(1 + R/2)` (the spec admits it is Studio's design); inspector creation caps 8/8/1/1; "2.2 gamma matches within 0.1%".

### d. What already exists
| Slice | Main today |
|---|---|
| #521 filters | Shipped on CPU (`image_adjust.rs`) and GPU (`adjust_image` in `draw.wgsl`), ten fields. SUPERSEDED. Open #521 is a different bug (wrong image in a variable-mode scope). |
| #37 progressive blur | None (`blurOpType` appears nowhere). Real work. |
| #38 glass and noise | `GlassEffect.splay` and `.bevel`, noise and texture `size_y` exist and round-trip through `.fig` and REST (0.7.x, PR #1593). Not rendered: `fx.wgsl` has no splay. The open gap is the render, not the model. |
| #1328 shadow behind | `DropShadow.show_behind` exists; silhouette cut-out is in the renderer. #1328 is CLOSED and was about background blur with no fill (PR #1624). Wrong issue, shipped feature. |

### e. Issue state
Issue #521 is OPEN v0.7.14 (wrong subject). Issue #37 is OPEN Backlog. Issue #38 is OPEN Backlog (stale: shipped except render). Issue #1328 is CLOSED (wrong subject). Epic #18 is OPEN, milestone "v0.6 AI design".

### f. Risks
- A 4-level Gaussian pyramid per blurred layer is four or more extra passes and targets per layer. The project already records that "a blurred layer alone is 4 passes" against the Metal 2,047-passes-per-submission limit; the spec does not budget this or say how the interactive and refine compiles differ. Keep it behind `interactive()`.
- Splay and glass shader changes must keep the headless CPU-versus-GPU pixel-diff tests exact.
- Moving the effect-count caps into the inspector is UI-only and safe, but its source is unverified.

### g. Verdict: SUPERSEDED, except progressive blur: READY AFTER FIXES
Keep only #37: doc model, `.fig` and REST round trip, CPU render, then GPU. Use `startOffset` as normalized (sourced). Say what the GPU pass budget is. Drop the filter, glass-model, noise-model and shadow-behind sections or restate them as "render splay in `fs_glass`" with the right semantics and a source.

---

<a id="e5-video-shaders"></a>

## 4. Video, custom shaders, mesh gradient, 3D in E5 ([`e5-video-fills-custom-wgsl-shaders-3d-transforms.md`](../e5-effects-shaders/e5-video-fills-custom-wgsl-shaders-3d-transforms.md))

### a. What it says to build
Video fills with poster fallback (#168, #169); a custom WGSL shader registry (#39, #170); Coons-patch mesh gradients (#571); 3D transforms with perspective, backface hiding, scene settings (#173, #175, #176).

### b. Code references
Found: `designos-doc/src/paint.rs`, `node.rs`, `designos-render/src/paint.rs`, `designos-gpu/src/geom.rs`, `compile.rs`, `IMG_TILE`, `Document::images`.
Not found: `designos-gpu/src/camera.rs`, `PaintKind::Video`, `PaintKind::CustomShader`, `Transform3d`, `Scene3d` (all proposed new except the file path, which is cited as grounding).
Elsewhere: `PaintKind::Shader(ShaderRef)` and `Effect::Shader` already exist (paint.rs:401-439); `VideoFill` lives in `prototype.rs:692`.

### c. Figma facts
- SOURCED: `PaintType::VIDEO` 7, `video` 18, `image` 8, `videoFillPlayback` 47 and its seven fields, `PaintType::CUSTOM` 10, `EffectType::CUSTOM` 9, `customEffectId` 39 and 43, `componentPropAssignments` 40 and 44, `transform3d` 570, `scene3d` 572, `FixedOrderTransform3d` 1-5, `backfaceHidden`, `perspectiveOrigin`, `preserve3d`.
- CONTRADICTED: "`image = 8` is the poster frame" is the spec's reading; the schema only says the paint has an image. And the budget claim "8 + 12 + 16 = 36 programs, satisfies the < 64 cap" contradicts what a Program is (see cross-cutting).
- ASSERTED: Euler order (the spec flags it unknown, good), Coons patch tessellation at 8x8, "$d/(d+t_z)$ shrinks to 0.6667" (the formula follows from the matrix the spec chose, not from Figma), backface culling rule on screen-space winding, "hardware decoders in prototype mode only", the video poster being the whole canvas behaviour. The test vector V-E5-08 expects a centre colour of (0.5, 0.5, 0.25) for red, green, blue, yellow corners; that is the plain bilinear average, and the spec gives no Figma source that mesh gradients blend that way.

### d. What already exists
- Video: Studio already decided a design: the poster is a normal `PaintKind::Image` paint, and a sidecar `VideoFill` record says it is really a video; video bytes sit in `Document::images`; it plays only in Present (`prototype.rs:685-705`, `designos-app/src/video.rs`). The spec's `PaintKind::Video(VideoPaint)` is a second, competing model. `.fig` import and export of `PaintType::VIDEO` is the open part (#169).
- Custom shaders: `.fig`, REST and `.designos` round trip as `ShaderRef { raw }`; the inspector shows "Shader (not rendered)" (#39 CLOSED, PR #1602). The spec's `CustomShaderPaint { effect_id, uniforms }` replaces the raw reference with a typed one; fine, but it must keep unknown shaders as raw.
- Mesh gradient, 3D: none.

### e. Issue state
Issue #168 is OPEN (decode through OS codecs, v0.29). Issue #169 is OPEN (video fills, v0.29). Issue #39 is CLOSED. Issue #170 is OPEN (author WGSL shaders, v0.24). Issue #571 is OPEN (v0.24). Issue #173 is OPEN (a REA study, not an implementation), #175 OPEN (v0.25), #176 OPEN (no milestone; "align with Figma once 3D is GA"). Issue #174 in that range is an unrelated CLOSED passthrough issue.

### f. Risks
- Video: a decoder thread per playing fill and an LRU texture pool must stay off the idle path (0.00% idle CPU). Poster-only canvas is safe; Present-mode decoding needs an explicit stop that cancels repaint requests.
- Custom WGSL: user shaders compile at runtime; the headless CPU path has no equivalent, so the CPU-versus-GPU pixel-diff gate cannot cover them.
- 3D with `preserve3d` and depth sorting is a new render path; offscreen Program use per 3D layer is unbudgeted.
- Marketing rule: any demo of these features must use the Untitled UI kit (video and 3D demos included); the spec does not say how a demo would be produced.

### g. Verdict: NEEDS EVIDENCE
Reconcile video with the existing `VideoFill` design (extend it, do not add a second model). Source the mesh-gradient colour model and the 3D rotation order from a Figma-authored file. Remove the 36-programs arithmetic. Note #176 and #173 are parked or study items.

---

<a id="e6-export-easing"></a>

## 5. Export bounds, export settings, easing and springs in E6 ([`e6-export-bounds-settings-easing-springs-spec.md`](../e6-motion-export/e6-export-bounds-settings-easing-springs-spec.md))

### a. What it says to build
- Slice 1: export bounds options (`useAbsoluteBounds`, `contentsOnly`) and a lossless WebP format (#528, #457).
- Slice 2: replace the Penner easing curves with CSS curves, add analytic spring settle time (#459).
- Slice 3: model all 18 `ExportSettings` fields and round-trip them through `.fig` (#213).

### b. Code references
Found: `designos-export/src/bounds.rs` (`render_bounds`, `own_overflow`, `bounds_of`), `designos-doc/src/settings.rs` (`ExportFileFormat`, `ExportConstraint`, `ExportPreset`), `prototype.rs` (`control_points`, `spring_settle_secs`, `spring_at`, `Easing`, `EasingType::Hold = 14`), `designos-export/src/raster.rs`, `svg.rs`, `kiwi.rs`, `fig.rs`, `fig_export.rs`.
Renamed: the spec says `prototype.rs / easing.rs`; `easing.rs` does not exist (it is `prototype.rs`). `ExportSettings` is a proposed new name for `ExportPreset`.
Not found: `ExportBoundsOptions`, `render_bounds_with_opts` (proposed new).
Wrong line numbers: the Figma facts F2-F4 cite "bounds.rs lines 51-59, 39-50, 91-95"; on main the stroke reach is at ~lines 62-69 and the drop-shadow reach at ~71-76.

### c. Figma facts
- SOURCED: all 18 `ExportSettings` fields and the five enums match the schema exactly (lines 279-292, 1409-1439). Easing curves: `39902-...min.js` module 70253 has `IN_CUBIC: [.42,0,1,1]`, `OUT_CUBIC: [0,0,.58,1]`, `INOUT_CUBIC: [.42,0,.58,1]`, `LINEAR`, the three back curves exactly as the spec lists, and the plugin names `EASE_IN` to `IN_CUBIC`. Spring presets: `GENTLE [1,100,15]`, `QUICK/ONE [1,300,20]`, `BOUNCY/TWO [1,600,15]`, `SLOW/THREE [1,80,20]` are in the same module (the names GENTLE, QUICK, BOUNCY, SLOW are in the switch). The spring step response and the over-damped settle formula match module 826862 (underdamped, over-damped, critical, and `-ln(eps) + ln((z+i)/(2i))` over `w0/(z+i)`). `useAbsoluteBounds` and `contentsOnly` exist in the plugin typing (`plugin-api.d.ts:5139-5143`).
- CONTRADICTED:
  1. Critical-damping settle `9.23341/w0`. Figma's code does not branch: for `zeta <= 1.0001` it uses the under-damped formula with `zeta` capped at 0.9999, and `AD` clamps `zeta` into [0.03, 0.9999] unless a "physical" spring is used. The Lambert-W root is correct maths (checked: 9.2334) but it is Studio's invention, not Figma's number.
  2. Test V-EAS-02: with the spec's own formulas, Gentle at t = 0.2 s gives x = 0.700 (not 0.724) and settle = 1.022 s (not 0.92 s). The Penner value of test V-EAS-01 is 0.145, not 0.165.
  3. The facts F2, F3 and F4 (shadow reach, stroke reach, clip containment) are labelled "FACT - seen" with `bounds.rs` as the source. That is Studio's own code. Open #528 says Studio's export bounds differ from Figma by 1 to 84 px on 10 kit frames, so these formulas are the known-wrong ones. The tests V-EXP-01 to V-EXP-04 are computed from them, not from Figma exports.
  4. "`EASE_IN / IN_CUBIC (tag = 0, 15)`" merges two Kiwi tags the repo keeps apart on purpose (`EasingType::EaseIn = 15` doc comment: "Not the stronger InCubic curve"; test at prototype.rs:780 pins Penner for tag 0). The Figma JS supports the merge (both tags map to the same bezier), so the spec is right and the repo's earlier decision is the one to revisit, with the tests changed deliberately.
- ASSERTED: CMYK conversion profile (US Web Coated SWOP), WebP quality 100 means lossless, `t_settle.clamp(0.05, 5.0)`, nominal text bounds when `useAbsoluteBounds`.

### d. What already exists
- Lossless WebP encode exists: `export_webp` in `raster.rs`, tested equal to PNG (#215 CLOSED, 0.5.0). Missing: the `ExportFormat` menu entry and `ExportFileFormat::Webp` (#457 OPEN is exactly that). The spec's `Webp = 9` extends an enum documented as "Kiwi parity" (tags 0..8); WebP has no Kiwi `ImageType`, so a `.fig` write must map it to PNG or keep a sidecar.
- Easing: 16 Kiwi values, `Hold`, `spring_at` (already the same step response) and `spring_settle_secs` (envelope 6.9/rate, capped at 4 s) exist (#139 CLOSED). Only the curve constants and the settle formula change. Open #459 as filed is a different task: "make `EasingType::to_easing` / `easing_type` round-trip (Hold, Spring, CustomSpring)"; `Hold` currently maps to `Easing::Linear` and loses its identity.
- Export settings: `ExportPreset` has format, constraint, suffix. `fig.rs` already imports `exportSettings` (4 formats) and `fig_export.rs:1746` writes it. The spec's struct keeps 10 of the 18 fields but its acceptance gate claims all 18 round-trip "without loss"; the eight it drops (`svgDataName`, `svgForceStrokeMasks`, `useBicubicSampler`, `frameRate`, `loopCount`, `duration`, `svgAnimated`, `pdfPrint`) would be lost.
- Bounds: PNG and SVG render bounds exist (#528, PR #1261); `useAbsoluteBounds` and `contentsOnly` are absent.

### e. Issue state
Issue #528 is OPEN v0.7.14 (the real defect is measured error against Figma, not missing options). Issue #213 is OPEN Backlog. Issue #457 is OPEN v0.7.14. Issue #459 is OPEN v0.7.14 (different task, see above). Issues #215 and #139 are CLOSED. Epic #19 is OPEN, no milestone. The spec's slice 2 swaps #459's subject; file a new issue or amend #459.

### f. Risks
Pure maths in `designos-doc`, no GPU or UI work, so the render invariants are untouched. Risk is correctness: changing curve constants moves every saved prototype transition and Motion track; the existing pins at `prototype.rs:780` must change in the same slice, with a note in the CHANGELOG. Keep `Easing::EaseIn` (Penner) distinct only if a file already saved with it must not move; say which.

### g. Verdict: READY AFTER FIXES
1. Fix the settle formula to match the JS (cap `zeta` at 0.9999 and drop the 9.23341 branch), or label it a Studio improvement with a flag.
2. Recompute V-EAS-01 and V-EAS-02 from the formulas.
3. Re-source bounds from Figma exports of kit frames (#528 has the measurements); delete F2-F4 as facts.
4. Decide the 18-field storage: keep unmodeled fields as a typed passthrough, or model all.
5. Keep WebP as a Studio-only format, with a stated `.fig` mapping.
6. Rename or file the easing issue; update the `prototype.rs` tests in the same slice.

---

<a id="e6-motion-lottie"></a>

## 6. Motion, timeline, Lottie, video export in E6 ([`e6-motion-timeline-lottie-video-export-spec.md`](../e6-motion-export/e6-motion-timeline-lottie-video-export-spec.md))

### a. What it says to build
Motion node types and fields; text stagger; a pure `evaluate_motion(doc, t_us) -> MotionOverrides` in `crates/designos-doc/src/motion.rs`; a Lottie exporter with 13 lint rules; hardware video export (VideoToolbox, MediaFoundation) behind a `motion-video` feature; about 60 composite presets.

### b. Code references
Found: `designos-doc/src/motion.rs` (the file exists, 448 lines), `designos-export/src/raster.rs`, `svg.rs`, `render_export`, `Document`, `NodeId`.
Renamed: `evaluate_motion` already exists in `designos-doc/src/motion_eval.rs:54` as `evaluate_motion(doc, t_us: i64) -> Vec<PropertyOverride>`; the spec proposes `u64` and `MotionOverrides` with a `HashMap`. `motion.rs` is the data model, not the evaluator.
Not found: `crates/designos-export/src/lottie.rs`, `video_export.rs`, `motion-video` feature, `MotionOverrides`, `NodeMotionOverride`, `render_export_with_overrides` (proposed new).

### c. Figma facts
- SOURCED: node types 56, 57, 58, `StyleType::ANIMATION` 7, every timeline field id and enum (505-513, 535, 543, 552, 553, 591, 599, 610; `TextElementDelayBehaviour` 1-7 with all enums), all matching the schema. Preset name families appear in `en.json` (32 of 32 name prefixes tried match). The Lottie lint rules come from real `export_lint.*` strings.
- CONTRADICTED: the lint rule count and wording. `en.json` has 16 lint descriptions: angular gradient, diamond gradient, drop shadow, glass, large audio, large image, noise, shader effect, shader fill, text axis timing, text combined animations, text loop, text mixed groupings, text spring, transform 3D, video. The spec lists 13 (merging the two shader rules, calling it "large_image") and omits `large_audio` and `text_combined_animations`. The spec also calls them "rejection rules" and has V-MOT-03 "rejecting export" for video; Figma's text for video is "Lottie can't read or display videos. You can export to MP4 or GIF", i.e. an issue to review, with several (shaders, gradients) exporting as a flat image rather than failing. Severity is not evidenced.
- ASSERTED: "about 60" composite presets and "6 primitives" (names found, count not), the Lottie AST optimizer behaviour (spec says unverifiable), 4-tap linear audio scrub, V-MOT-01's x = 100 at half time with "EaseInOut" (the true value at 0.5 for ease-in-out is 0.5, so 100 is right, but it is a derivation).

### d. What already exists
Most of slice 4's data side is shipped: Motion model (#140, 0.7.11), evaluation of timelines with SET/SCALE/OFFSET, ONCE/LOOP/BOOMERANG and nesting (#142), `.fig` import and export of timelines and tracks (#209), animation styles (#141, 0.7.12), easing and duration variable binding (#143). The text-stagger message is only a passthrough key. Open and real: applying motion values to render (#164), frame sequence export (#165), MP4 export (#167, with #168 decode), the `cargo-deny` licence gate for codec dependencies (#166), timeline UI, Lottie export, presets.

### e. Issue state
Issues #140, #141, #142 and #209 are CLOSED. Issues #164 and #165 are OPEN Backlog. Issue #166 is OPEN. Issue #167 is OPEN v0.29. Issues #164-#167 are the issues for slice 4's remaining work; the spec cites a range "#140-#142, #164-#167, #209-#237" without mapping slices to them.

### f. Risks
- Playback needs a repaint loop while playing; the spec says "0.00% CPU overhead" but never states that the loop stops (no `request_repaint` when paused or ended) or that scrubbing never calls `app.changed()` for the evaluator output. Idle CPU must stay 0.00% once playback stops.
- Transform and opacity tracks as uniforms: fine. Size, radius and path-trim tracks re-tessellate and fall under the 8 ms budget; do not do that for 100,000-node pages.
- Video encode on a background thread with its own render path: must not share the live `GeomCache`; use `render_node_into` style headless render.
- New codec dependencies: #166 licence gate first.

### g. Verdict: mixed
SUPERSEDED for the data model and evaluator (do not create a second `evaluate_motion`). READY AFTER FIXES for the lint rules (use all 16, state severity as unknown until a Figma export shows it) and for text stagger (schema sourced). NEEDS EVIDENCE for Lottie JSON output (no Figma-authored Lottie sample) and for the preset table (needs an actual preset definition file). Map each remaining item to #164-#167.

---

<a id="e7-annotations"></a>

## 7. Annotations, measurements, Code Connect, MCP in E7 ([`e7-annotations-measurements-code-connect-mcp-spec.md`](../e7-devmode-mcp/e7-annotations-measurements-code-connect-mcp-spec.md))

### a. What it says to build
Expand `ANNOTATION_PROPERTIES` from 22 to 33; add categories (5 presets, 8 colours, custom) and measurements; replace the Code Connect stub in the Inspect panel with a document registry (`Document.code_connect`, `code_connect.rs`); enhance `designos_get_design_context` (JSON format with an XPath query, asset directory, Code Connect snippets); add four Code Connect MCP tools. No issue numbers are named, only Epic #20.

### b. Code references
Found: `dev.rs:50-73` (`ANNOTATION_PROPERTIES` with exactly 22 entries, a correct claim), `Annotation` (dev.rs:42, text, properties labels, `category: Option<String>`), `designos-app/src/panels/inspect.rs` with the stub (Section B starts at line 512; the note is built at 535-541; the spec says lines 501-552, which contains it, and "remove lines 501-534 that append annotations", which stops 6 lines before the `annotations.push`), `designos_get_design_context` (`designos-mcp/src/tools/design_context.rs`, with a `code_connect` output key and a "Code Connect stub").
Renamed: nothing.
Not found: `code_connect.rs` (proposed), the four MCP tools (proposed), parameters `format`, `useCodeConnect`, `dirForAssetWrites` (today it takes `node_id`, `language`, `react_tailwind`, `response_format`, `include_screenshot`).
One extra check: `designos_query_xpath` already exists as a separate tool, so "XPath support in get_design_context" duplicates it.

### c. Figma facts
- SOURCED: the 33 `AnnotationPropertyType` values, categories (preset, colour, custom), `AnnotationMeasurement` (fields 1-9), `Annotation` (`labelV2`, `categoryId`), all matching the schema (lines 1581-1664, which also matches the spec's line numbers). Figma's desktop MCP tool parameters (`format`, `query`, `useCodeConnect`, `dirForAssetWrites`) are in the dossier with a source offset; That asar was not opened.
- CONTRADICTED: the REST-style names the spec assigns to the 11 new properties. Figma's public REST docs list 25 annotation types: `alignItems`, `mainComponent`, `layoutMode` and the 22 Studio already has. The spec maps "Alignment" to `primaryAxisAlignItems` and "Component" to `componentId`; the docs say `alignItems` and `mainComponent`. The grid names (`gridRowGap` and the rest) are not in the public REST list at all (Kiwi-only), so a REST export must drop or rename them.
- ASSERTED: that Code Connect belongs in the Figma file. In Figma, Code Connect mappings are published from a repo to Figma's cloud, not stored in the document; "store it in `Document`" is a Studio decision, which is fine if called one.

### d. What already exists
- Annotations with 22 property labels and a free-text `category` string exist, with REST JSON import and export (`designos-fig/src/dev.rs`). No Kiwi (`.fig`) annotation import or export: `annotationCategories` is only in the own-entry key table, and `fig.rs` never reads `annotations`.
- Dev Mode panels in `designos-app/src/dev/` including `annot.rs` (the UI home for #280).
- Code Connect: only the Inspect stub and a stub key in the MCP reply.
- Measurements: none.

### e. Issue state
The spec names none. The matching issues: #280 (annotation model and UI: 33 types, categories, measurements; OPEN, v0.27), #281 (`.fig` round trip of annotations, categories, measurements; OPEN; it sits at hot-file queue position 40 and depends on #280), #259 (model Code Connect mappings in the document; OPEN), #260 (replace the stub), #262 (MCP map tools), #261, #263, #264, #265, #266 (all OPEN, v0.16 Code Connect).
Conflicts with those issues:
- Issue #259 requires "multiple `{label, component_name, source, snippet, imports, prop_map}` per component" and archive on component delete; the spec's `CodeConnectMapping` is one mapping per node with a different field set.
- Issue #265 records owner ruling R6.21, "repo files win; the document keeps a synced copy". The spec never says which side wins and its lint tool ("stale source file paths") implies the document is the authority.
- Issue #262 names the MCP tools as "get map, add map, suggestions, send mappings"; the spec's four tool names differ.

### f. Risks
`designos_get_design_context` already carries a 5,000-node and 40,000-character fallback; adding asset writes to disk from an MCP read tool changes its risk tier (see the published tool risk metadata, #1287). Measurement overlays on the canvas must not call `app.changed()` per pan or zoom. Visual proof for #280 needs a desktop 1440 capture from the Untitled UI kit, not a self-built file.

### g. Verdict: READY AFTER FIXES
1. Replace the REST names for the new properties (`alignItems`, `mainComponent`) and mark the grid ones as Kiwi-only.
2. Reconcile the Code Connect model with #259 and R6.21 (multiple mappings per component, repo wins).
3. Name the issues (#280, #281, #259, #260, #262) and keep the hot-file queue position for #281.
4. Add the `.fig` encode and decode for annotations (the spec shows only the Rust model).
5. State where categories and measurements live (document level or per node) and how `Annotation.category: Option<String>` becomes an id.
6. Drop XPath-in-get_design_context in favour of the existing `designos_query_xpath`, or justify the duplicate.

---

## Suggested first slices (at most five)

1. **Easing curves and spring settle (#459 or a new E6 issue).** Files: `crates/designos-doc/src/prototype.rs` and its tests only. Safe: pure maths, no hot file, curves and spring presets are sourced from Figma's own JS. Do it with the fixed settle formula and recomputed test numbers; update the Penner pin at `prototype.rs:780` deliberately and note it in the CHANGELOG.
2. **Progressive blur, round trip first (#37).** Files: `designos-doc/src/paint.rs` (new effect), then `fig.rs`, `fig_export.rs`, `rest_import.rs` in the hot-file queue, then `designos-render/src/effects.rs`; GPU in a later PR. Safe: the Kiwi fields and normalized-offset meaning are sourced, and nothing exists yet; the first PR touches no GPU code.
3. **Annotation model: 33 types and categories (#280).** Files: `designos-doc/src/dev.rs`, `designos-app/src/dev/annot.rs`, `sections.rs`. Safe: the enum is sourced from the schema, no hot file until #281; use `alignItems` and `mainComponent` for the REST names. Needs a 1440 kit-based screenshot.
4. **Lossless WebP in the export menu (#457).** Files: `designos-export/src/lib.rs` (`ExportFormat`), `designos-doc/src/settings.rs` (`ExportFileFormat`). Safe: the encoder and its test already exist; the slice is a menu entry and an enum variant with an explicit `.fig` mapping (no Kiwi tag 9).
5. **Code Connect mapping model (#259).** Files: new `crates/designos-doc/src/code_connect.rs` plus a `Document` field with `#[serde(default, skip_serializing_if)]`. Safe: a new file, no hot file; build it to #259's acceptance (multiple mappings, archive on delete) and R6.21, not the spec's single-mapping struct. The Inspect stub replacement (#260) follows in a second PR.

Held back on purpose: multi-stroke (#196) needs the GPU mask-Program design first; export bounds (#528) needs Figma-measured values, not the spec's formulas; everything in E4 draw and E5 video, shaders, mesh and 3D needs the evidence listed above.
