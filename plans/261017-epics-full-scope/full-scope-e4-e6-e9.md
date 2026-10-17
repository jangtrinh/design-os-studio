# E4 Draw & Vector, E6 Motion & Media export, E9 FigJam & Slides: FULL SCOPE

Brainstormer revision, 2026-10-17, under the owner ruling: full scope, no MVP. Status: owner decisions pending. Supersedes draft-e4-e6-e9.md.

**Revised for the full-scope ruling.** Every gap in gap-list.md and in the Figma catalog for E4, E6 and E9 is now a task. Packs only set the delivery order. The only exclusions left are the ones that are out of the product by nature, and each is an owner question.

### Epistemic Ledger
- **FACT:** everything cited with file:line was read this session. The catalog sections I read for coverage:
  - §3 Draw (`figma-catalog-2026-10.md:26-39`)
  - §9 Motion (:90-100)
  - §12 export (:126-131)
  - §16 FigJam (:163-170)
  - §17 Slides (:172-179)
- **INFERENCE:** sizes, pack grouping, and crate behaviour I know but did not check this session.
- **ASSUMPTION:** the version for each pack; licences of crates that are not in `Cargo.lock`; Figma behaviour that REA has not read yet.
- **Not verified:**
  - Nothing was compiled.
  - Licences of crates outside `Cargo.lock` are unchecked. The repo has no `deny.toml` (FACT).
  - Rows marked "verify first" may already exist in the code.
  - I made 5 `gh api` calls.

---

## Shared groundwork (needed before all three epics)

**What exists today (FACT):**
- **Import keeps Draw/Motion fields only as strings.** `extract_fig_metadata` (`designos-fig/src/fig.rs:609-753`) stores an allowlist of fields as JSON strings, including `timelineDefinitions` :735 and `scatterStrokeSettings` :731.
- **Large files keep almost none of it.** When a file has more than 5,000 entries, nearly everything is skipped (`fig.rs:285`, `:610-620`).
- **Export never writes these fields back.** The hand-coded `append_fig_metadata` (`fig_export.rs:1740`) never emits them: grep finds 0 hits for timeline, scatter, variableWidth or transformModifiers in `fig_export.rs`, `kiwi.rs` and `rest_import.rs`.
- **Unknown node types are kept as placeholders.** They load as `NodeKind::Unknown` (`rest_import.rs:964`), so TABLE, SLIDE, KEYFRAME, TRANSFORM and TEXT_PATH all end up there.
- **The GPU reuses the CPU stroke outline** (`designos-gpu/src/geom.rs:286`).
- **Easing lives in the app crate** (`designos-app/src/proto/easing.rs:65-125`), not in `designos-doc`.

| ID | Task | Size/Pri | Touches | Test seam | Acceptance | Deps |
|---|---|---|---|---|---|---|
| X1 | Typed kiwi passthrough. Keep every unmapped field and node type as typed kiwi values and re-emit them on export, on large files too | L / P1 | `fig.rs`, `fig_export.rs`, `kiwi.rs` | designos-fig testkit | (1) Draw/Motion/TABLE/SLIDE fixture fields are equal after a round-trip; (2) the same holds on a file with more than 5,000 entries; (3) existing round-trip tests stay green | — |
| X2 | Move easing (cubic, spring, presets) into `designos-doc`; add REA's 15 `EasingType` values | S / P1 | `prototype.rs`, `proto/easing.rs` | unit | (1) prototype player tests unchanged; (2) preset values match REA r5 | — |
| X3 | Licence gate: `deny.toml` that allows MIT/Apache/BSD/ISC/Zlib and flags GPL/LGPL | S / P1 | root config | `cargo deny check licenses` | (1) the current tree passes; (2) adding a GPL crate fails the gate | — |

Rule: only one task at a time may touch `fig.rs`, `fig_export.rs` or `rest_import.rs`.

---

## E4 Draw & Vector (#17)

### 1. What exists today (FACT)
- **Stroke model:** one stroke per node (`node.rs:92`).
  - `Stroke` (`paint.rs:254-264`) has no variable width, offset or brush.
  - `StrokeAlign` is Inside/Center/Outside only (`:230-235`).
- **Paints:** no PATTERN paint (`paint.rs:188-212`). An image Tile mode exists (`:184`) and the GPU has an `IMG_TILE` path (`paintconv.rs:174,182`).
- **Node kinds:** no TransformGroup or TextPath (`node.rs:356-407`).
- **Vector edit tools:** Move, Bend and Bucket (`canvas/vector.rs:25-33`). The paint bucket is tested (`shell/tests/vector.rs:367`).
- **Vector commands:** flatten, outline stroke, simplify and boolean (`vecops.rs:47,88,146,186`), plus the shape builder (`vectorops.rs:453-506`).
- **Brushes:** a BRUSH node imports as a plain VECTOR (`fig.rs:833`). Only `brushType` survives (`fig_export.rs:554`).
- **Freehand tools:** Pencil, Marker and Highlighter (`tools.rs:20,31-32`).

### 2. Full scope
Every Draw item in the catalog (§3 :31-36): variable-width and dynamic strokes, stretch and scatter brushes, pattern fills, repeaters through transform groups (linear and radial, plus symmetry and skew), eraser, cut, offset, text on a path, and stroke sampling. REA adds path trim, multi-stroke with stroke offset, and the vector bounding box for multiple points.

On top of that the plan includes:
- the Draw toolbar mode;
- round-tripping every Draw field;
- SVG, PDF and codegen output for all of it.

**Shaders** are listed in catalog §3 but belong to E5. They are not planned here.

**Excluded by nature, owner to decide:** Vectorize, because Figma's version is an AI model (see Q2).

**Packs (dependency order):**

| Pack | Headline | Follow-ups |
|---|---|---|
| D1 Pro strokes | Variable width | Path trim, Draw mode, bounding box, outline export |
| D2 Repeat & pattern | Transform groups | Symmetry/skew, pattern fill |
| D3 Edit vectors | Eraser | Cut, offset, multi-stroke, stroke sampling |
| D4 Expressive strokes | Brushes | Dynamic stroke, brush library, custom brushes |
| D5 Type on path | Text on path | — |
| D6 | Vectorize | Only if the owner keeps it (Q2) |

### 3. Owner decisions
1. **Brush artwork source?**
   - **(a) Recommended: ship our own original brush set and also render brushes from imported `.fig` files.** Figma's behaviour without any IP risk.
   - (b) Copy Figma's 25 built-in brushes from Figma.app. Exact look, but legal/IP risk.
   - (c) Render imported brushes only, with no built-in library. Cheapest, but Draw feels incomplete.
2. **Vectorize (an AI model in Figma, so excluded by nature)?**
   - **(a) Recommended: build an offline, non-AI image tracer** (e.g. `vtracer`; MIT licence is an ASSUMPTION to check). The feature exists offline. Trade-off: weaker than AI on photos.
   - (b) Route it through the Studio Agent runtime (the user's own CLI). Quality depends on that CLI's image support.
   - (c) Exclude it. Zero cost, but one catalog gap stays open.
3. **Which pack ships first?**
   - **(a) Recommended: D1 Pro strokes.** The geometry is shared by both renderers and all REA fields are known.
   - (b) D2 Repeat & pattern. More visual, but adds a new node kind and a cache.
   - (c) D3 Edit vectors. Used daily, but blocked on an REA read of the eraser/offset geometry.

**Technical decisions taken:**
- Draw gets a toolbar mode, matching Figma.
- All stroke, brush and repeater geometry is built as fill paths in `designos-render`, so the GPU inherits it with no new `Program`.
- Variable width is disabled on dashed strokes, as in Figma.
- SVG, PDF and codegen export these as outlined paths.
- Pattern tiles go in a byte-capped LRU texture cache drawn through `IMG_TILE`.
- Repeaters are tessellated once and drawn as transformed copies.
- Dynamic stroke and scatter use a seeded deterministic PRNG.

### 4. Tasks

| ID | Pack | Task | Size/Pri | Touches | Test seam | Acceptance | Deps |
|---|---|---|---|---|---|---|---|
| E4-1 | D1 | Typed doc fields + `.fig`/REST mapping for every Draw field: 446-452, 482, 571, 554, 542, 455, 432; BRUSH=46, TRANSFORM=48, TEXT_PATH=41; PATTERN=8 | L / P1 | `node.rs`, `paint.rs`, `fig.rs`, `fig_export.rs`, `rest_import.rs` | fig round-trip | (1) all fields survive `.fig` → `.designos` → `.fig`; (2) old `.designos` files load | X1 |
| E4-2 | D1 | Variable-width outline geometry | L / P1 | `vectorops.rs`, `outline.rs` | area + GPU diff | (1) TAPER area within ±1% of the analytic value; (2) headless CPU = GPU, bit-exact; (3) dashed strokes ignore the width profile | E4-1 |
| E4-3 | D1 | Variable-width tool: 7 presets, on-canvas width handles, stroke panel (es-designer) | M / P1 | `properties.rs`, `canvas/handles.rs` | shell + 1440 screenshot | (1) a handle drag is one undo step; (2) preset changes render the same frame | E4-2 |
| E4-4 | D1 | Static path trim (render + panel); E6 reuses it | M / P1 | render, gpu `geom.rs`, properties | pixel diff | (1) 0.25→0.75 draws half the path; (2) CPU = GPU | E4-1 |
| E4-5 | D1 | Draw toolbar mode | M / P1 | `shell.rs`, `tools.rs` | shell + screenshot | (1) Draw mode shows only Draw tools; (2) shortcut matches Figma | — |
| E4-6 | D1 | Vector bounding box for multiple selected points (verify first) | S / P2 | `canvas/vector.rs` | shell test | (1) scaling the selected points' box scales only those points | — |
| E4-7 | D1 | SVG/PDF/codegen emit Draw geometry as outlines (extended as features land) | M / P1 | `svg.rs`, `pdf.rs`, codegen | golden tests | (1) a variable-width stroke exports as a filled path; (2) no Draw data is silently dropped (a warning is listed) | E4-2, PR #88 merged |
| E4-8 | D2 | `TransformGroup` node + REPEAT linear/radial (count 2–40): render, index, hit-testing | L / P1 | `node.rs`, `index.rs`, render, gpu `compile.rs` | render + hit tests | (1) radial ×12: CPU = GPU; (2) 40×40 copies pan within 8 ms on the VSF page; (3) no new `Program` | E4-1 |
| E4-9 | D2 | SYMMETRY + SKEW, stacking up to 2 modifiers | M / P1 | same as E4-8 | render tests | (1) symmetry mirrors live edits; (2) a 3rd modifier is refused | E4-8 |
| E4-10 | D2 | Transform group UI: create from selection, modifier panel, handles (es-designer) | M / P1 | properties, canvas | shell + screenshot | (1) changing the count is one undo step; (2) ungroup bakes the copies | E4-9 |
| E4-11 | D2 | Pattern paint render (RECT/H-HEX/V-HEX, spacing, scale, alignment) with a capped cache | L / P1 | render `paint.rs`, gpu `paintconv.rs` | pixel diff | (1) hex rows offset correctly; (2) the cache stays under its cap during scroll | E4-1 |
| E4-12 | D2 | Pattern fill UI: source picker, tile options | M / P1 | fill panel | shell + screenshot | (1) editing the source updates every pattern; (2) round-trip | E4-11 |
| E4-13 | D3 | Eraser tool | M / P1 | `canvas/vector.rs`, `vectorops.rs` | area tests | (1) an erase across a shape splits it; (2) one undo step | REA |
| E4-14 | D3 | Cut (scissors) tool | M / P1 | `canvas/vector.rs` | network tests | (1) cutting a closed path opens it at the cut point | REA |
| E4-15 | D3 | Offset path command | M / P1 | `vecops.rs`, `vectorops.rs` | area tests | (1) +10 on a 100 px square ≈ (120)² minus the corners; (2) a negative offset shrinks the shape | REA |
| E4-16 | D3 | Multi-stroke model (`stroke` becomes `strokes: Vec`) + OFFSET align | L / P1 | `node.rs`, all renderers, codegen, MCP | full suite | (1) old files migrate; (2) a single stroke renders exactly as before | E4-1 |
| E4-17 | D3 | Multi-stroke UI: add, reorder, offset value | M / P1 | stroke panel | shell + screenshot | (1) 3 strokes render in order; (2) round-trip | E4-16 |
| E4-18 | D3 | Stroke sampling (semantics need REA) | S–M / P2 | TBD | TBD | Defined after REA | REA |
| E4-19 | D4 | Dynamic stroke geometry (frequency/wiggle/smoothen, seed) | M / P1 | `vectorops.rs` | determinism | (1) same seed → identical output; (2) CPU = GPU | E4-2 |
| E4-20 | D4 | Dynamic stroke UI | S / P1 | stroke panel | shell | (1) slider ranges match Figma (0.01–20, 0–1) | E4-19 |
| E4-21 | D4 | Stretch brush engine (warp artwork along the path) | L / P1 | render | pixel diff | (1) an imported STRETCH brush renders without being flattened | E4-1 |
| E4-22 | D4 | Scatter brush engine (gap, wiggle, angle/size jitter, rotation) | L / P1 | render | determinism + diff | (1) seeded and stable across re-renders | E4-1 |
| E4-23 | D4 | Built-in brush library + brush picker (es-designer) | M / P1 | assets, panel | screenshot | (1) all brushes from Q1 apply; (2) work offline | Q1, E4-21/22 |
| E4-24 | D4 | Custom brush from a selection (whether Figma supports this is an ASSUMPTION; needs REA) | M / P2 | doc, panel | shell | (1) a new brush appears in the picker and round-trips | E4-23, REA |
| E4-25 | D5 | Text-on-path layout | L / P1 | `text/`, `textmeasure.rs` | glyph tests | (1) baseline follows the tangent; (2) `forward=false` reverses direction | E4-1 |
| E4-26 | D5 | Text-on-path tool: attach, drag start point, flip, edit | M / P1 | canvas, `textedit.rs` | shell + screenshot | (1) text stays editable after attaching to a path | E4-25 |
| E4-27 | D6 | Vectorize, per Q2 | M–L / P3 | new module | golden | (1) a raster logo becomes vector paths offline | Q2 |

### 5. Risks and unknowns
- **REA still needed:**
  - width interpolation between profile points;
  - the dynamic-stroke noise function;
  - the brush stamp and warp model;
  - eraser, cut and offset semantics;
  - hex pattern alignment;
  - stroke sampling;
  - custom brushes;
  - tablet pressure.
- **E4-16 multi-stroke has the widest blast radius in E4.** It changes every renderer, codegen and MCP.
- **E4-7 conflicts with PR #88** in `pdf.rs`. Wait for #88 to merge.

---

## E6 Motion & Media export (#19)

### 1. What exists today (FACT)
- **Prototype easing and transitions:** easing (`prototype.rs:65-80`) and 7 `TransitionKind` values (`:83-91`).
- **Smart Animate:** `proto/smart.rs:1-4`.
- **Spring solver:** `proto/easing.rs:65,77`.
- **No Motion model:** there is no timeline, track or keyframe model.
- **Video is decode-only:** Motion-JPEG decoding, with `openh264` behind an optional feature (`video.rs:1-13`, `designos-app/Cargo.toml:28`).
- **Export formats:** `ExportFormat` is Png/Jpg/Svg/Pdf (`designos-export/src/lib.rs:48-53`).
- **Vector PDF:** PR #88 (closes #40) is open and not merged.
- **Already in `Cargo.lock`:** `gif` 0.14.1, `color_quant` 1.1.0, `image-webp` 0.2.4, `zip` 9.0.1, `png` 0.17.16, `openh264` 0.9.8.

### 2. Full scope
- **Motion editing** (catalog §9 :98-100; REA r5 Motion):
  - timeline: duration, playback ONCE/LOOP/BOOMERANG, autoplay, keyframe snapping, quantized playback, nested timelines, per-layer offset;
  - keyframes and tracks: SET/SCALE/OFFSET operations, track loop mode;
  - track targets: translation, rotation, scale, shear, opacity, corner radius, stroke weight, width/height, path trim, the 21 effect fields, text, and color with OKLAB/OKLCH/HSL/linear interpolation;
  - all 15 easing types, including springs and custom cubic;
  - anchor point and motion path;
  - first-party presets and the ~60 composite animation styles;
  - animation styles in libraries;
  - text animation with stagger;
  - easing and timing variables;
  - audio on the timeline;
  - the Motion toolbar mode.
- **Export:** CSS, JSON, React, MP4, WebM, animated SVG, GIF, Lottie and dotLottie, with their export settings (frame rate, loop count, background, duration, quality).
- **Static WebP export.**
- **Vector PDF** is already covered by #40 / PR #88; it is a dependency here, not re-planned.
- **Overlaps, noted only:** `get_motion_context` MCP (E7), `figma.motion` Plugin API (E8).
- **Excluded by nature:** none.

**Encoders and dependencies.** All work offline. The project is MIT/Apache (FACT). X3 enforces the licence list.

| Format | Choice |
|---|---|
| GIF | `gif` + `color_quant` NeuQuant (both already in the lock). **Reject `imagequant` (GPL-3).** |
| PNG sequence | `png` |
| Lottie / dotLottie | `serde_json` / `zip` |
| SVG, CSS, JSON, React | String emitters, no new crates |
| Static WebP | `image-webp`. INFERENCE: encodes lossless only. |
| WebM | `rav1e` AV1 (BSD-2 + royalty-free AOM patent licence; ASSUMPTION) + an in-house EBML muxer |
| MP4 | Per Q1. Muxer: in-house ISO-BMFF writer, or the `mp4` crate (MIT, ASSUMPTION) |
| Audio decode | `symphonia`. **MPL-2.0 (file-level copyleft): X3 must allow it explicitly (ASSUMPTION it is acceptable).** |
| Audio playback | `cpal` (Apache-2.0) |
| Audio encode | Opus via libopus (BSD, C) for WebM; AAC per Q1 |

All video and audio code sits behind a `motion-video` cargo feature, so CI stays pure Rust.

**Packs (dependency order):**

| Pack | Headline | Follow-ups |
|---|---|---|
| M1 Animate a frame | Timeline + keyframes + playback | Export settings, PNG sequence, GIF, WebP |
| M2 Motion toolkit | Presets and animation styles | Anchor point, motion path, path trim tracks, effect tracks, text animation, library styles, variables |
| M3 Ship to web | Lottie | dotLottie, animated SVG, CSS, JSON, React |
| M4 Video | MP4 + WebM | Export progress UI |
| M5 Sound | Audio on the timeline | Audio inside MP4 and WebM |

### 3. Owner decisions
1. **How to encode MP4 (H.264 + AAC audio)?**
   - **(a) Recommended: the OS encoders (VideoToolbox/AudioToolbox on macOS, Media Foundation on Windows), with AV1 + Opus on Linux.** Licence-clean and plays everywhere. Cost: code per platform.
   - (b) Compile `openh264` in. One code path, but Cisco's patent cover applies only to Cisco's downloaded binary, which needs the network.
   - (c) Use the user's ffmpeg when installed. Tiny effort, but the feature is missing for most users.
   - (d) AV1 everywhere. Pure Rust, but QuickTime and PowerPoint playback is poor.
2. **Which exports go in M1?**
   - **(a) Recommended: GIF + PNG sequence + WebP.** All their dependencies are already in the lock, so M1 ships soonest.
   - (b) Lottie in M1. Developers want it, but it makes M1 L-sized.
3. **What should the React export emit?**
   - **(a) Recommended: Figma's exact flavour, after an REA read, with CSS-in-JSX as the fallback.** Parity rule.
   - (b) Always use the `motion` (framer-motion) library. Idiomatic, but adds a dependency to the user's project.

**Technical decisions taken:**
- The model mirrors Figma's field IDs.
- The evaluator is a pure function, `(doc, t_us) → overrides`, applied to a scratch copy, the same way `video.rs:5-7` handles video.
- **Playback never calls `app.changed()` or bumps `TileView::rev`.**
- Tracks that change only transform or opacity reuse cached geometry.
- Export renders on the headless CPU renderer, on a worker thread.
- Motion lives next to Smart Animate; it does not replace it.

### 4. Tasks

| ID | Pack | Task | Size/Pri | Touches | Test seam | Acceptance | Deps |
|---|---|---|---|---|---|---|---|
| E6-1 | M1 | Motion model: timeline def incl. parent/nested, assignments, tracks, keyframes, interpolation, color space, snap/quantize, track loop | M / P1 | `designos-doc` new module, `node.rs` | serde | (1) `.designos` round-trip; (2) a doc without Motion serializes byte-identical | X2 |
| E6-2 | M1 | Evaluator: SET/SCALE/OFFSET, ONCE/LOOP/BOOMERANG, track loop, quantize, nested timelines | M / P1 | doc | sampling tests | (1) BOOMERANG at 1.5d equals the value at 0.5d; (2) spring matches `spring_at` | E6-1 |
| E6-3 | M1 | Property applier: transform/shear, opacity, radius, stroke weight, size, color in 5 color spaces | M / P1 | doc, render | unit + diff | (1) OKLCH midpoint matches a reference value; (2) applying never mutates `doc` | E6-2 |
| E6-4 | M1 | Motion kiwi import/export: nodes 56-58; fields 445, 505-509, 535, 552, 553, 591, 599 | L / P1 | `fig.rs`, `fig_export.rs`, `kiwi.rs` | round-trip | (1) a Figma Motion file round-trips times, values and easings | X1, E6-1 |
| E6-5 | M1 | Canvas, Present and prototype autoplay playback | M / P1 | `canvas/gpu.rs`, `present.rs` | shell + perf probe | (1) `rev` does not change during playback; (2) a transform-only track causes no re-tessellation | E6-3 |
| E6-6 | M1 | Motion mode + timeline panel: tracks, diamonds, scrubber, zoom, snapping (es-designer) | L / P1 | new `panels/timeline*.rs`, `shell.rs` | shell + screenshot | (1) add/move/delete keyframe is one undo step each; (2) scrub updates the same frame | E6-2 |
| E6-7 | M1 | Easing editor: 15 types, bezier handles, spring parameters | M / P1 | panels | shell + screenshot | (1) dragging a handle updates the curve preview and the playback | E6-6 |
| E6-8 | M1 | Export settings model + panel: frameRate/loopCount/background/duration/svgAnimated/quality; ImageType 4-8 round-trip | M / P1 | `designos-export` `lib.rs`, `fig.rs` | unit + round-trip | (1) defaults: GIF 15 fps, video 30 fps; (2) `.fig` export settings survive | E6-1, X1 |
| E6-9 | M1 | Frame-sequence renderer (worker thread) + PNG sequence; CLI + MCP | M / P1 | export, CLI, MCP | export tests | (1) frame t equals a single render at t; (2) the UI thread never blocks | E6-3 |
| E6-10 | M1 | GIF export: NeuQuant palette, loop count, fps {8,12,15,24,30} | M / P1 | export | export tests | (1) 1 s at 15 fps gives 15 frames; (2) loop 0 = forever | E6-9 |
| E6-11 | M1 | Static WebP export (lossless) | S / P2 | export | export test | (1) decoded output equals the PNG pixels | — |
| E6-12 | M2 | First-party presets (FADE, MOVE, ROTATE, SCALE, PATH_DRAW, PATH_TRAVEL, …) + preset instance node 58 | M / P1 | doc, timeline UI | unit | (1) FADE creates opacity 0→1 with the default bezier | E6-6 |
| E6-13 | M2 | ~60 composite animation styles | L / P2 | doc, assets | golden | (1) each style produces REA's recipe | REA, E6-12 |
| E6-14 | M2 | Anchor point | M / P1 | doc, canvas | unit + diff | (1) rotation pivots around the anchor | E6-3 |
| E6-15 | M2 | Motion path (path travel) | L / P1 | doc, canvas | unit | (1) the layer follows the path at t | REA, E6-3 |
| E6-16 | M2 | Path-trim tracks + PATH_DRAW/ERASE/TRAVEL recipes | S / P1 | doc | unit | (1) draw-on animates trim 0→1 | E4-4, E6-12 |
| E6-17 | M2 | Effect tracks (21 fields) | M / P2 | doc, render | diff | (1) animated blur radius renders identically on CPU and GPU at t | E6-3 |
| E6-18 | M2 | Text animation: TEXT_DATA track, stagger by char/word/line, order, delay | L / P1 | doc, `text/` | unit + diff | (1) per-word stagger offsets each word by the delay | E6-3 |
| E6-19 | M2 | Animation styles in libraries (StyleType ANIMATION=7, `styleIdsForAnimation` 582) | M / P2 | `styles.rs`, `fig.rs` | round-trip | (1) a published style applies in a consumer file, read-only (A13) | E1 styles, E6-12 |
| E6-20 | M2 | Easing/timing variables (`durationUsVar`, easing vars) | M / P2 | `variables*.rs` | unit | (1) changing the variable re-times bound tracks | E6-2 |
| E6-21 | M3 | Lottie export + lint (reject angular/diamond gradients, shaders, 3D, video; rasterize shadow/glass/noise) | L / P1 | export | schema + golden | (1) passes the lottie-web schema; (2) unsupported features get a named warning | E6-8 |
| E6-22 | M3 | dotLottie | S / P1 | export | zip test | (1) the archive opens in a dotLottie player | E6-21 |
| E6-23 | M3 | Animated SVG | M / P1 | export | golden | (1) SMIL/CSS timing matches the keyframes | E6-8 |
| E6-24 | M3 | CSS keyframes | M / P1 | export | golden | (1) % stops = keyframe times; (2) cubic-bezier values match | E6-8 |
| E6-25 | M3 | JSON motion export (Figma schema) | S–M / P2 | export | golden | (1) matches the REA-captured sample | REA |
| E6-26 | M3 | React export | M / P1 | export, codegen | golden | (1) the emitted component compiles (tsc) | Q3, E6-24 |
| E6-27 | M4 | WebM: rav1e + EBML muxer, quality LOW/MED/HIGH | L / P1 | export | container parse | (1) plays in Chrome and Firefox (screenshot); (2) default build has no rav1e | E6-9, X3 |
| E6-28 | M4 | MP4 per Q1 | L–XL / P1 | export, platform shims | per-OS smoke | (1) plays in QuickTime; (2) cancel leaves no file | Q1 |
| E6-29 | M4 | Video export UX: progress, cancel, size estimate (es-designer) | M / P1 | export panel | shell + screenshot | (1) cancel within 1 frame; (2) UI stays responsive | E6-27 |
| E6-30 | M5 | Audio on the timeline: import, playback, waveform track | L / P2 | doc, timeline UI | unit + manual | (1) audio stays in sync with the scrubber (within 1 frame) | E6-6, X3 |
| E6-31 | M5 | Audio muxing: Opus in WebM, AAC per Q1 | M / P2 | export | container parse | (1) the A/V duration delta is at most 1 frame | E6-27/28/30 |

### 5. Risks and unknowns
- **REA still needed:**
  - motion-path coordinate space;
  - the React and JSON export formats;
  - recipes for the ~60 styles;
  - stagger details;
  - variable bindings through `parameterConsumptionMap`;
  - the audio data model.
- **Preview can differ from export (second-order effect).** While playing, the idle full-quality refine never runs, so masks and blur look worse in the live preview than in the export.
- **Biggest strategic risk:** patents and licences for H.264, AAC and MPL code (Q1 and X3).

---

## E9 FigJam & Slides (#22)

### 1. What exists today (FACT)
- **Board items:** sticky, shape-with-text, connector (with routing) and stamp (`designos-doc/src/board.rs:34-1001`).
- **Board tools:** marker and highlighter (`board/create.rs:81`).
- **Sections:** `NodeKind::Section` exists (`node.rs:362`).
- **Missing on boards:** grep found no table, mind map or Mermaid code.
- **Home "Templates":** only a filename filter, not a template system (`home/mod.rs:243-249`).
- **Decks:**
  - slides are frames with `SlideProps{notes}` only (`node.rs:159-162`);
  - add, duplicate, delete, move and notes work (`decks/mod.rs:112-204`);
  - deck PDF export is raster (`decks/export.rs:1-2`);
  - Present navigates slides with the arrow keys (`present.rs:350-365`).
- **File formats:** no `.deck` or `.jam` references in the code.

### 2. Full scope
- **FigJam** (catalog :164-170):
  - sections;
  - mind maps;
  - quick create;
  - templates;
  - timer;
  - tables with merged cells;
  - arrows;
  - Mermaid paste (every diagram type Figma supports);
  - `generate_diagram`;
  - ERD;
  - AI summaries;
  - Mural/Lucid/Jamboard import;
  - widgets;
  - `.jam` files.
- **Slides** (catalog :173-179):
  - decks and design mode;
  - libraries/themes;
  - templates;
  - sections;
  - interactive prototypes;
  - object animations;
  - transitions;
  - presenter notes and presenter view;
  - skipped slides;
  - slide numbers;
  - `.deck` files;
  - the agent in FigJam and Slides.
- **Overlaps, noted only:** PPTX belongs to E8 (keep Figma's transition and animation enums so they map 1:1 to `p:transition`/`p:timing`). MCP `use_figma` belongs to E7.
- **Excluded by nature, owner to decide (Q1):**
  - multi-user-only features: voting, spotlight, cursor chat, live slide polls;
  - AI image tools, which need a remote image model;
  - an iPad app.

**Packs (dependency order):**

| Pack | Headline | Follow-ups |
|---|---|---|
| S1 Present like Keynote | Transitions | Skipped slides, slide numbers, presenter view, `.deck` |
| S2 Build the story | Object animations | Sections, interactive prototypes, templates infrastructure, themes |
| J1 Tables | Tables | Table editing UI |
| J2 Diagrams | Mermaid | All Mermaid types, ERD, `generate_diagram` |
| J3 Think together | Mind maps | Quick create, FigJam templates, timer, `.jam`, agent and summaries |
| J4 Bring boards in | Foreign import | Widgets |

### 3. Owner decisions
1. **Exclusions by nature: confirm (multi-select)?**
   - **(a) Recommended: exclude voting, spotlight, cursor chat, live polls, AI image tools and the iPad app.** They contradict offline-first and no-remote-model rulings, or are a different platform.
   - (b) Plan solo variants: local voting tallies, and spotlight as presenter-follow in Present.
   - (c) Keep all as P3 placeholder tasks.
2. **AI summaries and "Agent in FigJam/Slides"?**
   - **(a) Recommended: run them through the existing Studio Agent runtime (the user's CLI) in board and deck modes.** No model is built into the app, and it reuses the runtime.
   - (b) Exclude them as remote AI.
3. **Templates while offline?**
   - **(a) Recommended: "Save as template" plus a small set of original bundled templates.** Works offline, no IP risk.
   - (b) "Save as template" only. Empty on first run.
   - (c) Recreate Figma Community templates. Familiar, but IP risk.
4. **Board import from other tools?**
   - **(a) Recommended: Lucid + Mural in J4, and drop Jamboard.** INFERENCE: Google discontinued Jamboard, so it only exports PDF.
   - (b) Move these importers to E8 (interop).
   - (c) Also import Jamboard PDFs as images.

**Technical decisions taken:**
- Pack order puts S1 first: all its pieces are S-sized, REA's fields are complete, and it reuses `proto/geom.rs:151-233`.
- Slide transitions map Figma's 26 types onto `TransitionKind`, extended only where a type has no equivalent.
- Object animations use the shared easing (X2), not the E6 timeline, matching Figma.
- Mermaid uses an in-house subset parser and a layered layout, drawn with `ShapeWithText` and `board::route`.
- The mind map model follows REA (parity rule).
- Table cells reuse `TextProps`.

### 4. Tasks

| ID | Pack | Task | Size/Pri | Touches | Test seam | Acceptance | Deps |
|---|---|---|---|---|---|---|---|
| E9-1 | S1 | Typed slide fields + `.fig` round-trip: SLIDE 32, SLIDE_GRID 37, SLIDE_ROW 38; fields 389, 410, 443, 426; transition | M / P1 | `node.rs`, `fig.rs`, `fig_export.rs` | round-trip | (1) a Figma Slides file round-trips; (2) old decks load | X1 |
| E9-2 | S1 | Transitions: 26 types + duration, picker + Present playback (es-designer) | M / P1 | `decks/ui.rs`, `present.rs`, `proto/geom.rs` | shell + screenshot | (1) Push-left over 300 ms plays on Next; (2) one undo step | E9-1 |
| E9-3 | S1 | Skipped slides in Present and PDF; dimmed thumbnail | S / P1 | `present.rs`, `decks/export.rs` | unit | (1) Next jumps over a skipped slide; (2) the PDF excludes it | E9-1 |
| E9-4 | S1 | Slide numbers (field 443 + number layer) | S / P1 | `decks/` | unit | (1) numbers renumber on reorder and skip skipped slides, as Figma does | E9-1 |
| E9-5 | S1 | Presenter view: notes, next slide, timer, on a second display (verify first) | M / P1 | `present.rs` | shell + screenshot | (1) notes show the current slide's notes; (2) the audience window has no notes | — |
| E9-6 | S1 | `.deck` file import/export | M / P2 | `designos-library`, `fig.rs` | fixture | (1) a `.deck` opens as a deck | REA, E9-1 |
| E9-7 | S2 | Object animations: FADE/SLIDE_FROM_*, IN/OUT, click order | M / P1 | `decks/`, `present.rs` | player tests | (1) click order is respected; (2) OUT runs before the transition | X2, E9-1 |
| E9-8 | S2 | Sections in the slide grid | S–M / P1 | `decks/` | shell | (1) moving a section moves its slides in one undo step | E9-1 |
| E9-9 | S2 | Interactive prototype slides (flows play inside Present) | M / P2 | `present.rs`, `proto/player.rs` | player tests | (1) a hotspot on a slide navigates in-slide; (2) Next still advances | E9-1 |
| E9-10 | S2 | Template infrastructure: save, browse, insert, bundled set (for boards and decks) | M / P1 | `home/`, `designos-library` | library tests | (1) works offline; (2) inserts with fresh IDs | Q3 |
| E9-11 | S2 | Slide themes and libraries in design mode | M / P2 | `styles.rs`, `decks/` | unit | (1) a theme change restyles every slide; (2) undo | E1 styles, E9-10 |
| E9-12 | J1 | Table model + render + `.fig` (TABLE 29, TABLE_CELL 30, fields 308-311, 538) | L / P1 | `node.rs`, `board.rs`, render, `fig.rs` | doc + render + round-trip | (1) merged 2×2 cells render as one; (2) round-trip | X1, REA |
| E9-13 | J1 | Table UI: tool, cell text, add/remove/reorder/resize, merge/unmerge (es-designer) | L / P1 | `board/*` | shell + screenshot | (1) Tab moves between cells; (2) every operation is one undo step | E9-12 |
| E9-14 | J2 | Mermaid flowchart paste → shapes and connectors | M / P1 | `clipboard.rs`, `board/create.rs` | parser golden | (1) `A-->B-->C` gives 3 shapes and 2 connectors; (2) invalid input changes nothing | — |
| E9-15 | J2 | Remaining Mermaid types Figma supports (sequence, state, class, gantt, …, per REA) | L / P2 | same | golden per type | (1) each type renders as editable native layers | REA, E9-14 |
| E9-16 | J2 | ERD (Mermaid erDiagram → entity tables) | M / P2 | board | golden | (1) entities become tables with attached relationship connectors | E9-12, E9-15 |
| E9-17 | J2 | `generate_diagram` MCP tool (Mermaid → board) | S / P2 | `designos-mcp` | MCP test | (1) the tool call creates the same layers as a paste | E9-14 |
| E9-18 | J3 | Mind map model + auto layout | L / P1 | doc, board | layout tests | (1) adding a child re-lays out its siblings; (2) round-trip | REA |
| E9-19 | J3 | Mind map UI: Tab/Enter to add, collapse, branch colours (es-designer) | M / P1 | `board/*` | shell + screenshot | (1) keyboard-only creation works | E9-18 |
| E9-20 | J3 | Quick create (hover arrows spawn a connected shape; verify first) | M / P2 | `board/*` | shell | (1) the new shape is auto-connected | — |
| E9-21 | J3 | Facilitator timer (local) | S / P3 | `board/toolbar.rs` | shell | (1) the timer runs while the canvas is in use | — |
| E9-22 | J3 | `.jam` file import/export | M / P2 | `designos-library`, `fig.rs` | fixture | (1) a `.jam` opens as a board | REA, X1 |
| E9-23 | J3 | Agent panel + summaries in board and deck modes | M / P2 | `terminal.rs`, `panels/rail.rs` | shell | (1) the selection is sent as context; (2) the summary inserts as a sticky | Q2 |
| E9-24 | J4 | Lucid import | L / P3 | importer | fixture | (1) shapes and connectors map to native items | Q4 |
| E9-25 | J4 | Mural import | L / P3 | importer | fixture | (1) stickies map to native stickies | Q4 |
| E9-26 | J4 | FigJam widgets | XL / P3 | plugin runtime | E8 harness | (1) a sample widget renders and keeps its synced state | E8 Plugin API |

### 5. Risks and unknowns
- **REA still needed:**
  - the table cell → row/col binding field;
  - the mind map model;
  - the names of TransitionType 0–25;
  - which Mermaid types Figma supports;
  - the slide-section model;
  - the `.deck` and `.jam` containers.
- **E9-26 depends on E8's Plugin API (XL).**
- **Second-order risk:** adding non-Figma transition kinds would make PPTX (E8) lossy.

---

## Pack sequencing across epics (INFERENCE)
- **Order:** X1–X3 first, then S1 → D1 → M1 → S2 → D2 → M2 → J1 → D3 → M3 → J2 → D4 → M4 → J3 → D5 → M5 → J4 (→ D6 if the owner keeps Vectorize).
- **Versions (ASSUMPTION):** E1–E3 take v0.7–v0.8, so S1 would land around v0.9.
- **Rule:** never run two packs whose tasks touch the hot files (`fig.rs` / `fig_export.rs` / `rest_import.rs`) at the same time.
- **Total:** 3 shared tasks + 27 (E4) + 31 (E6) + 26 (E9) = 87 tasks.

## Unresolved questions (for the controller to ask through AskUserQuestion)
- E4: Q1 brush artwork, Q2 Vectorize, Q3 first pack.
- E6: Q1 MP4/AAC encoder, Q2 exports in M1, Q3 React flavour.
- E9: Q1 exclusions, Q2 agent and summaries, Q3 templates, Q4 board import.
- Whether the MPL-2.0 `symphonia` crate is acceptable under X3.

**Assumption disclosure:**
- **Pack-to-version mapping:** low confidence. Changes if the owner reorders the epics.
- **X1 assumes the export schema can encode the passthrough fields:** medium confidence. If not, X1 becomes XL.

Status: DONE_WITH_CONCERNS
Summary: Every E4, E6 and E9 gap in gap-list.md and the catalog is now one of 87 tasks across 3 shared tasks and 16 packs (plus D6 if Vectorize is kept). The only exclusions left are by nature and are owner questions.
Concerns/Blockers:
- About 14 tasks are blocked on REA reads.
- Crate licences outside `Cargo.lock` are unverified until X3 exists.
- E4-7 has to wait for PR #88 to merge.
- E6-19 and E9-11 depend on E1's style infrastructure; E9-26 depends on E8's Plugin API.