# E4 Draw & Vector, E6 Motion & Media export, E9 FigJam & Slides

Draft from the brainstormer agent, 2026-10-17. Status: DRAFT; owner rulings pending. Owner ruling after drafting: FULL SCOPE, no MVP, so every 'MVP', 'later' and 'non-goal' line below must become a task or an owner decision.

### Epistemic Ledger
- **FACT:** what I read this session. That covers the 6 planning docs, the code listed below (with file:line), issues #17, #19 and #22, issue #40, and PR #88: open, not merged, vector PDF.
- **INFERENCE:** sizes, how features split into packs, and crate behaviour I know from memory but did not check this session.
- **ASSUMPTION:** pack-to-version mapping, crate licences and maintenance status, and Figma behaviour that REA did not determine.
- **UNKNOWN:** the fields that REA R5 lists as "Not determined" (`rea-r5-motion-draw-effects.md:5-12`).

**Not verified:**
- Nothing was compiled or run.
- Licences for crates that are not in `Cargo.lock` (rav1e, mp4, objc2-video-toolbox) are ASSUMPTION. Check them with `cargo deny` before adding any. The repo has no `deny.toml` (FACT).
- I made 5 `gh api` calls (#17, #19, #22, #40, PR 88).

---

## Shared groundwork (blocks all three epics)

**FACT, three findings that shape every task below:**
1. **The `.fig` round-trip loses Draw, Motion and Slides data today.**
   - `extract_fig_metadata` (`designos-fig/src/fig.rs:609-753`) saves only an allowlist of fields, as JSON strings. `timelineDefinitions` (:735) and `scatterStrokeSettings` (:731) are in it.
   - Files with more than 5,000 entries skip almost all of it (`fig.rs:285`, `:610-620`).
   - `append_fig_metadata` (`fig_export.rs:1740`) writes back a hand-coded key list. It never writes these fields: 0 hits for timeline, scatter, variableWidth or transformModifiers in `fig_export.rs`, `kiwi.rs` and `rest_import.rs`.
   - Unknown node types become `NodeKind::Unknown{type_name}` (`rest_import.rs:964`). TABLE, SLIDE, KEYFRAME, TRANSFORM and TEXT_PATH land there.
2. **New stroke geometry can be shared by both renderers.** The GPU tessellates the CPU's `stroke_outline` (`designos-gpu/src/geom.rs:286`; `designos-render/src/outline.rs:1-6`). INFERENCE: any stroke shape written as fill geometry in `designos-render` gets CPU/GPU parity and needs no new GPU `Program`.
3. **Easing lives in the app crate,** at `designos-app/src/proto/easing.rs:65-125`: `ease`, `spring_at` and `PRESETS`. Headless export, CLI and MCP cannot use it from there.

| ID | Task | Size/Pri | Touches | Test seam | Acceptance | Deps |
|---|---|---|---|---|---|---|
| X1 | Typed kiwi field passthrough: unknown/unmapped node fields and node types are kept as typed kiwi values and written back unchanged, including on files with more than 5,000 entries | L / P1 | `fig.rs`, `fig_export.rs`, `kiwi.rs` | `designos-fig` testkit round-trip | (1) import → export → re-import of a fixture with variableWidthPoints, timelineDefinitions, TABLE and SLIDE is byte-equal per field; (2) same result on a file with more than 5,000 entries; (3) the existing round-trip tests still pass | — |
| X2 | Move easing (bezier, spring, presets) into `designos-doc` and add REA's cubic and spring presets | S / P1 | `prototype.rs`, `proto/easing.rs` | unit tests | (1) no behaviour change in the prototype player tests; (2) GENTLE/Quick/Bouncy/Slow and IN/OUT/INOUT(_BACK) match REA's values | — |

Technical decision taken: **only one task may touch `fig.rs`, `fig_export.rs` or `rest_import.rs` at a time.** These files are 2.1k, 2.1k and 3.3k lines, and parallel edits would conflict.

---

## E4 Draw & Vector (#17)

### 1. What exists today (FACT)
- **Stroke model:** `Stroke` has paints, weight, side_weights, align, cap, join, miter and dashes (`paint.rs:254-264`).
  - `StrokeAlign` is Inside/Center/Outside, with no OFFSET (`:230-235`).
  - A node has exactly one stroke (`node.rs:92`), so there is no multi-stroke.
- **Paints:** `PaintKind` is Solid, Linear/Radial/Angular/Diamond gradient and Image (`paint.rs:188-212`). There is no PATTERN.
  - `ImageScaleMode::Tile` exists (`paint.rs:184`), and the GPU has an `IMG_TILE` path (`designos-gpu/src/paintconv.rs:174,182`).
- **Node kinds:** no TransformGroup and no TextPath (`node.rs:356-407`).
- **Vector edit tools:** Move, Bend and Bucket (`canvas/vector.rs:25-33`). The paint bucket is tested (`shell/tests/vector.rs:367`). There is no eraser or cut.
- **Object vector ops:** flatten, outline stroke, simplify and boolean (`vecops.rs:47,88,146,186`). Geometry comes from `vectorops.rs` (`outline_stroke` :299; arrange/shape builder :453-506; i_overlay). There is no offset.
- **Brushes:** a BRUSH node imports as VECTOR (`fig.rs:833`). Only `brushType` survives, and it is written back (`fig_export.rs:554`).
- **Freehand:** Pencil, Marker and Highlighter tools exist (`tools.rs:20,31-32`; `board/create.rs:81`).

### 2. Proposed scope

| Pack | Headline | Follow-ups |
|---|---|---|
| **D1 "Pro strokes"** | Variable-width strokes: presets UNIFORM, WEDGE, TAPER, QUARTER_TAPER, EYE, MIRRORED_TAPER and CUSTOM, plus width handles on the canvas | Static path trim (start/end); Draw toolbar mode; Draw-field round-trip (needs X1) |
| **D2 "Repeat & pattern"** | Transform groups (REPEAT linear/radial, count 2–40; SYMMETRY; SKEW; at most 2 modifiers) | Pattern fill (RECT, H-HEX, V-HEX; spacing, scale, alignment) |
| **D3 "Edit vectors"** | Eraser | Cut (scissors), offset path, dynamic stroke (frequency/wiggle/smoothen plus seed), multi-stroke with OFFSET align |
| **D4 "Brushes & type"** | Stretch and scatter brushes | Text on path |

- **Non-goals:**
  - Figma's own brush artwork (see Q1).
  - Raster painting.
  - AI vectorize (P3, needs a remote model).
  - Pressure/tablet input. ASSUMPTION: not needed for parity in the MVP; REA should confirm.

### 3. Owner decisions
1. **Brush artwork source?**
   - **(a) Recommended: ship our own original brush set and render the brushes that come inside imported `.fig` files.** Same behaviour as Figma, no IP risk.
   - (b) Render imported brushes only, with no brush authoring. Cheapest, but Draw feels half-built.
   - (c) Copy Figma's 25 built-in brushes out of Figma.app. Exact visual parity, but a legal/IP risk.
2. **Headline for the first Draw pack?**
   - **(a) Recommended: D1 Pro strokes.** All of it is shared geometry, REA fields are complete, and it is the lowest risk.
   - (b) D2 Repeat & pattern. More eye-catching, but it adds a new node kind and a tile cache.
   - (c) D3 Edit tools. Daily use, but REA has not determined the eraser/offset geometry.

**Technical decisions taken:**
- Draw gets its own toolbar mode, as in Figma (owner rule: follow Figma's behaviour).
- Every Draw stroke is built as fill geometry in `designos-render`. Both renderers inherit it, with no new `Program`.
- Variable width is rejected on dashed strokes, as in Figma.
- SVG, PDF and codegen export variable-width, brush and repeater content as outlined paths. INFERENCE: this matches Figma.
- Pattern tiles are rasterised by the CPU into an LRU texture cache with a hard byte cap counted next to the VRAM budget, and drawn through the existing `IMG_TILE` path.
- Repeater copies are tessellated once and placed per copy with transforms. Worst case is 40 × 40 copies; `SceneIndex` culls them.
- Dynamic stroke uses a seeded deterministic PRNG, so headless CPU/GPU diffs stay bit-exact.

### 4. Task breakdown

| ID | Pack | Task | Size/Pri | Touches | Test seam | Acceptance | Deps |
|---|---|---|---|---|---|---|---|
| E4-1 | D1 | Map the Draw fields to typed doc fields: variableWidthPoints, pathTrim, strokeData/strokeOffset, transformModifiers, PATTERN, TEXT_PATH, dynamic/scatter/stretch settings, strokeSeed | M / P1 | `fig.rs`, `fig_export.rs`, `rest_import.rs`, `paint.rs`, `node.rs` | fig round-trip | (1) every listed field survives `.fig` → `.designos` → `.fig`; (2) older `.designos` files still load | X1 |
| E4-2 | D1 | Variable-width outline geometry (width profile along the path → fill path) | L / P1 | `vectorops.rs`, `outline.rs` | area tests + GPU pixel diff | (1) TAPER on a 100 px line has the analytic area ±1%; (2) headless CPU vs GPU is bit-exact; (3) a dashed stroke ignores the profile | E4-1 |
| E4-3 | D1 | Variable-width UI: profile picker in the stroke panel and width handles on the canvas (es-designer) | M / P1 | `panels/properties.rs`, `canvas/handles.rs` | shell tests + 1440 screenshot | (1) dragging a width handle is one undo step; (2) choosing a preset updates the canvas in the same frame | E4-2 |
| E4-4 | D1 | Static path trim: render and panel (E6 reuses it) | M / P1 | render, gpu `geom.rs`, properties | pixel diff | (1) start=0.25/end=0.75 draws half the path; (2) CPU = GPU | E4-1 |
| E4-5 | D1 | Draw toolbar mode (Design / Draw / Dev) | M / P2 | `shell.rs`, `tools.rs` | shell test + screenshot | (1) the mode shows only Draw tools; (2) the shortcut matches Figma | — |
| E4-6 | D2 | `NodeKind::TransformGroup`: REPEAT, SYMMETRY, SKEW; bounds, hit-testing, SceneIndex | L / P1 | `node.rs`, `index.rs`, render, gpu `compile.rs` | render + hit tests | (1) radial ×12 renders identically on CPU and GPU; (2) 40×40 copies pan within the 8 ms budget on the VSF fixture page; (3) no new `Program` | E4-1 |
| E4-7 | D2 | Pattern paint (RECT/HEX) with a capped tile cache | L / P1 | `paint.rs`, render `paint.rs`, gpu `paintconv.rs` | pixel diff | (1) H-HEX offsets alternate rows; (2) the cache stays under its cap during scroll | E4-1 |
| E4-8 | D3 | Eraser VTool (sweep a region → i_overlay subtract on the network) | M / P2 | `canvas/vector.rs`, `vectorops.rs` | area tests | (1) erasing across a filled shape splits it into two regions; (2) one undo step | REA |
| E4-9 | D3 | Cut + offset path | M / P2 | `vecops.rs`, `vectorops.rs` | area tests | (1) offset +10 on a 100 px square gives area (120)² minus the corner rounding | REA |
| E4-10 | D3 | Dynamic stroke (seeded) | M / P2 | `vectorops.rs` | determinism test | (1) same seed gives identical paths; (2) CPU = GPU | E4-2 |
| E4-11 | D3 | Multi-stroke + OFFSET align (`stroke` → `strokes: Vec`) | L / P2 | `node.rs` and every renderer, codegen | full suite | (1) old files migrate; (2) the first stroke renders exactly as before | E4-1 |
| E4-12 | D4 | Brushes: stretch along the path, scatter stamps | XL / P3 | render, `fig.rs` | pixel diff | (1) an imported BRUSH node renders without flattening; (2) scatter is deterministic | REA, Q1 |
| E4-13 | D4 | Text on path (glyphs placed along a curve, `textPathStart`) | L / P3 | `text/`, `textmeasure.rs` | glyph-position tests | (1) the glyph baseline follows the path tangent; (2) `forward=false` reverses direction | E4-1 |

### 5. Risks and unknowns
- **REA still needed:**
  - how width is interpolated between points;
  - the dynamic-stroke noise function (needed for exact visual parity);
  - the brush stamp model;
  - eraser/offset semantics;
  - hex pattern alignment maths;
  - pressure input.
- **E4-11 multi-stroke has a wide blast radius.** It changes the stroke field of every node and touches all renderers, codegen and MCP. Keep it last.
- **Repeaters on huge pages.** Up to 1,600 copies per group could break the 8 ms `GeomCache` budget. Mitigate with stale_hits deferral and culling (INFERENCE).

---

## E6 Motion & Media export (#19)

### 1. What exists today (FACT)
- **Prototype easing:** Linear, the In/Out/InOut(_Back) cubics, custom Bezier and Spring (`prototype.rs:65-80`).
- **Transitions:** `TransitionKind` has 7 kinds (`prototype.rs:83-91`).
- **Smart Animate:** interpolates transform, size, opacity, radius, fills and strokes (`proto/smart.rs:1-4`).
- **Spring solver:** `proto/easing.rs:65,77`.
- **No Motion support:** there is no timeline, keyframe or track model. On import, `timelineDefinitions` is kept as an opaque string and dropped on export (see Shared groundwork).
- **Video is decode-only:** Motion-JPEG playback in Present; H.264 decoding sits behind the optional `openh264` feature (`video.rs:1-13`, `designos-app/Cargo.toml:28`).
- **Export formats:** `ExportFormat` is Png/Jpg/Svg/Pdf (`designos-export/src/lib.rs:48-53`). `pdf.rs` is raster on main; vector PDF is PR #88, still open.
  - The export crate's `image` has png/jpeg only (`designos-export/Cargo.toml:13`).
  - The app crate enables `webp` (`designos-app/Cargo.toml:40`).
- **Already in `Cargo.lock`:** `gif` 0.14.1, `color_quant` 1.1.0, `image-webp` 0.2.4, `zip` 9.0.1, `png` 0.17.16, `openh264` 0.9.8.

### 2. Proposed scope

| Pack | Headline | Follow-ups |
|---|---|---|
| **M1 "Animate a frame"** | Timeline + keyframes on opacity, X/Y, rotation, scale, size, corner radius, stroke weight and path trim, with playback on the canvas and in Present | Motion toolbar mode; lossless kiwi import/export of Motion data; GIF + PNG-sequence export; **static WebP export** (S, standalone, can ship in any pack) |
| **M2 "Ship to web"** | Lottie and dotLottie export | Animated SVG, CSS keyframes, React; animation presets FADE/MOVE/ROTATE/SCALE/PATH_DRAW/PATH_TRAVEL; text stagger |
| **M3 "Video"** | MP4 + WebM export (settings: fps, quality, background, duration, loop) | Progress bar and cancel, encoding off the UI thread |
| Later | — | Motion path, anchor point, the ~60 composite animation styles, animation styles in libraries (StyleType ANIMATION=7, E1), easing variables, audio track (P3) |

**Encoder and dependency list.** The project is MIT/Apache-2.0 (FACT). Everything below works with no network.

| Format | Choice | Notes |
|---|---|---|
| GIF | `gif` 0.14 (MIT/Apache) + `color_quant` NeuQuant (MIT) | Both already in the lock. **Reject `imagequant`** (GPL-3 or commercial). |
| PNG sequence | `png` | Already in the lock. |
| Lottie / dotLottie | `serde_json` / `zip` | Already in the lock. |
| Animated SVG, CSS, React | String emitters | No new dependencies. |
| Static WebP | `image-webp` | INFERENCE: its encoder is lossless only. Lossy needs libwebp (C), so it is a later follow-up. |
| WebM | `rav1e` AV1 (BSD-2 + royalty-free AOM patent licence, pure Rust; ASSUMPTION, verify) + a small in-house EBML/WebM muxer | |
| MP4 | Decided by owner Q1. Muxer `mp4` crate (MIT, ASSUMPTION) or an in-house ISO-BMFF writer | |

All video encoders go behind a `motion-video` cargo feature, following the existing `video` feature precedent, so the default build and CI stay pure Rust.

- **Non-goals:**
  - Animated WebP.
  - Audio export.
  - Anything Lottie's lint rejects: angular/diamond gradients, shaders, 3D, video. Like Figma, these are blocked with a warning.
  - Server-side rendering.

### 3. Owner decisions
1. **How should MP4 (H.264) be encoded?**
   - **(a) Recommended: the OS encoder (VideoToolbox on macOS, Media Foundation on Windows), with AV1-in-MP4 on Linux.** Licence-clean, plays everywhere (Keynote, PowerPoint, social). The cost is per-OS code.
   - (b) Compile `openh264` in. One code path, but Cisco's patent cover applies only to Cisco's downloaded binary, which needs the network. That conflicts with offline-first.
   - (c) Use the user's ffmpeg when it is installed. Tiny effort, but the feature is missing for most users.
   - (d) AV1 only, for both MP4 and WebM. Pure Rust, but QuickTime and PowerPoint playback is poor.
2. **Which export ships in the first Motion pack?**
   - **(a) Recommended: GIF + PNG sequence.** Its dependencies are already in the lock, so the pack ships soonest.
   - (b) Lottie first. Developers want it, but the mapping is L-sized.
   - (c) Hold M1 until video is ready. Biggest headline, but XL and blocked on Q1.
3. **Animation presets in the first pass?**
   - **(a) Recommended: the 6 first-party primitives first, and the ~60 composite styles after REA dumps their recipes.** Every primitive is fully specified by REA.
   - (b) Reproduce all ~60 now. Full parity, but blocked on REA.
4. **What should the React export emit?**
   - **(a) Recommended: whatever Figma emits, after an REA read, with plain CSS-in-JSX as the fallback.** Parity rule.
   - (b) Always use `motion` (framer-motion) code. Most idiomatic, but adds a third-party dependency to users' projects.

**Technical decisions taken:**
- The Motion data model mirrors Figma's IDs: `Timeline{duration_us, playback, autoplay, snap_fps}` and `Track{field, op, keyframes}`.
- The evaluator is a pure function `(doc, t_us) → per-node overrides` applied on a scratch copy, the same pattern Present uses for video (`video.rs:5-7`).
- **Playback never calls `app.changed()` or bumps `TileView::rev`** (`CONTEXT.md`).
- Tracks that change only transform or opacity reuse the cached geometry. Only size, radius and path-trim tracks re-tessellate, within the 8 ms budget.
- Export uses the headless CPU renderer (budget=None, full quality, deterministic) and encodes on a worker thread.
- Motion lives next to prototype Smart Animate, as in Figma, and does not replace it.

### 4. Task breakdown

| ID | Pack | Task | Size/Pri | Touches | Test seam | Acceptance | Deps |
|---|---|---|---|---|---|---|---|
| E6-1 | M1 | Motion model in `designos-doc` (timeline, assignments, tracks, keyframes, interpolation HOLD/BEZIER/SPRING, color space) | M / P1 | `designos-doc` new module, `node.rs` | serde tests | (1) round-trips through `.designos`; (2) a file without Motion data is byte-identical to before | X2 |
| E6-2 | M1 | Evaluator (SET/SCALE/OFFSET ops, ONCE/LOOP/BOOMERANG playback) | M / P1 | doc | unit sampling tests | (1) BOOMERANG at 1.5×duration equals the value at 0.5; (2) a spring track matches `spring_at` | E6-1 |
| E6-3 | M1 | Motion kiwi import/export (KEYFRAME=56, KEYFRAME_TRACK=57, ANIMATION_PRESET_INSTANCE=58, fields 552/553/445/505-509/535/599) | L / P1 | `fig.rs`, `fig_export.rs`, `kiwi.rs` | fig round-trip | (1) a Figma Motion file round-trips with the same keyframe times and values; (2) an unknown easing value is kept | X1, E6-1 |
| E6-4 | M1 | Canvas and Present playback | M / P1 | `canvas/gpu.rs`, `present.rs` | shell test + perf probe | (1) `rev` does not change during playback; (2) a transform-only track does no re-tessellation (GeomCache counter) | E6-2 |
| E6-5 | M1 | Timeline panel and Motion mode: tracks, keyframe diamonds, scrubber, easing picker (es-designer) | L / P1 | new `panels/timeline*.rs`, `shell.rs` | shell tests + screenshot | (1) add/move/delete a keyframe, each one undo step; (2) scrubbing updates the canvas the same frame | E6-2 |
| E6-6 | M1 | Frame renderer + GIF/PNG-sequence export (UI, CLI flag, MCP) | M / P1 | `designos-export` (`lib.rs` ExportFormat), CLI | export tests | (1) a 1 s animation at 15 fps gives 15 GIF frames, looping forever; (2) the frame at t matches a PNG render at t | E6-2 |
| E6-7 | M1 | WebP static export | S / P2 | `designos-export` | export test | (1) decoded output equals the PNG export pixel-for-pixel (lossless) | — |
| E6-8 | M2 | Lottie / dotLottie exporter with the lint rules | L / P1 | `designos-export` | JSON schema + golden files | (1) the output passes the lottie-web schema; (2) a rejected feature gives a named warning | E6-2 |
| E6-9 | M2 | Animated SVG + CSS keyframes + React | M / P2 | `designos-export` | golden tests | (1) the CSS keyframe % values match keyframe times; (2) the cubic-bezier values match the easing | E6-2, Q4 |
| E6-10 | M2 | Animation presets + text stagger | M / P2 | doc, timeline UI | unit tests | (1) FADE applied adds an opacity track 0→1 with the default bezier [0,0,.58,1] | E6-5 |
| E6-11 | M3 | WebM (rav1e + muxer) behind `motion-video` | L / P2 | `designos-export` | ffprobe-free container parse test | (1) the file opens in Chrome and Firefox (manual, screenshot); (2) the default build has no rav1e | E6-6 |
| E6-12 | M3 | MP4 according to owner Q1 | L–XL / P2 | `designos-export`, platform shims | per-OS smoke test | (1) plays in QuickTime (macOS); (2) cancel leaves no partial file | Q1 |

### 5. Risks and unknowns
- **REA still needed:**
  - motion-path coordinate space;
  - React export flavour;
  - composite-style recipes;
  - text-element stagger details;
  - whether `parameterConsumptionMap` binds tracks to variables.
- **Preview can differ from export.** During playback the `FrameKey` changes every frame, so the idle full-quality refine never runs. Masks and blur on heavy pages will look worse in the preview than in the export. This is a second-order effect; document it.
- **Encoder licences/patents.** The H.264 patent position is the one strategic legal risk (Q1).

---

## E9 FigJam & Slides (#22)

### 1. What exists today (FACT)
- **Board mode:** sticky, shape-with-text, connector (with routing) and stamp node kinds (`designos-doc/src/board.rs:34-1001`). Marker and Highlighter tools (`board/create.rs:81`).
  - grep found no table, mind map or Mermaid code.
- **"Templates" on Home is a filename filter,** not a template system (`home/mod.rs:243-249`).
- **Decks:** slides are frames carrying `SlideProps{notes}` and nothing else (`node.rs:159-162`).
  - Add/duplicate/delete/move slide and notes (`decks/mod.rs:112-204`).
  - Raster deck PDF (`decks/export.rs:1-2`).
  - Present has slide navigation keys (`present.rs:350-365`).
- **Missing in decks:** transitions, skipped slides, sections, object animations.
- No `.deck` or `.jam` references in the library, fig or io code.
- TABLE and SLIDE import as `Unknown` (see Shared groundwork).

### 2. Proposed scope

| Pack | Headline | Follow-ups |
|---|---|---|
| **S1 "Present like Keynote"** | Slide transitions: Figma's TransitionType 0–25 plus duration, stored as an ON_CLICK interaction on the slide | Skipped slides (Present and PDF skip them); slide numbers; sections (SLIDE_GRID/ROW); slide-field round-trip |
| **S2** | Object animations (FADE, SLIDE_FROM_*; IN/OUT; click order) | Slide templates / themes |
| **J1 "Tables"** | FigJam tables (TABLE/TABLE_CELL, fractional row/col order, sizes, merged cells) | — |
| **J2 "Diagrams from text"** | Paste Mermaid → native shapes and connectors | Board templates |
| **J3** | Mind map | Blocked on REA |

- **Non-goals:** PPTX (E8). Meeting tools (timer, voting, cursor chat), widgets, AI summaries and Mural/Lucid import (P3).
- **Overlap with E8 (note only):** keep slide transitions and object animations in Figma's enums, so the E8 PPTX mapper can translate them 1:1 to `p:transition` and `p:timing`.

### 3. Owner decisions
1. **Which pack comes first?**
   - **(a) Recommended: S1 "Present like Keynote".** Its pieces are all S-sized, REA's fields are complete, and it reuses the prototype transition code (`proto/geom.rs:151-233`).
   - (b) J1 Tables. High FigJam value, but blocked on the table cell → row/col field that REA has not determined.
2. **Templates while offline?**
   - **(a) Recommended: local "Save as template" plus about 6 original bundled templates.** Works offline with no IP risk.
   - (b) Local "Save as template" only. Cheapest, but empty on first run.
   - (c) Recreate Figma Community templates. Familiar to users, but IP risk and lots of content work.
3. **Mermaid scope for the first release?**
   - **(a) Recommended: flowchart only.** It covers most real pastes and reuses connector routing.
   - (b) Flowchart + sequence diagrams. More complete, but sequence needs a lifeline layout.
   - (c) Every type Figma supports. Full parity, but those types are not yet confirmed by REA.
4. **Mind maps?**
   - **(a) Recommended: wait for REA.** The parity rule says match Figma's model.
   - (b) Build our own model now. Faster, but `.fig`/`.jam` round-trip breaks later.

**Technical decisions taken:**
- Slide transitions reuse the prototype `TransitionKind` and player. Figma's 26 types map to it, and `TransitionKind` grows only where a type has no equivalent.
- Object animations use the shared easing (X2), not the E6 timeline (Figma keeps the two separate).
- Mermaid is parsed by an in-house subset parser, laid out with a simple layered layout, and drawn with the existing `ShapeWithText` and `board::route`.
- Table cell text reuses `TextProps`.

### 4. Task breakdown

| ID | Pack | Task | Size/Pri | Touches | Test seam | Acceptance | Deps |
|---|---|---|---|---|---|---|---|
| E9-1 | S1 | Slide fields in the doc and `.fig`: isSkippedSlide=410, slideNumber=443, transition, SLIDE/SLIDE_GRID/SLIDE_ROW | M / P1 | `node.rs`, `fig.rs`, `fig_export.rs` | round-trip | (1) a Figma Slides file round-trips skip, number and transition; (2) old decks load | X1 |
| E9-2 | S1 | Transition picker + Present playback (es-designer) | M / P1 | `decks/ui.rs`, `present.rs`, `proto/geom.rs` | shell test + screenshot | (1) Push-left over 300 ms plays on Next; (2) one undo step | E9-1 |
| E9-3 | S1 | Skipped slides: Present and PDF skip them; dimmed thumbnail | S / P1 | `present.rs`, `decks/export.rs` | unit test | (1) Next jumps over a skipped slide; (2) PDF page count excludes it | E9-1 |
| E9-4 | S1 | Sections in the slide grid | S–M / P2 | `decks/` | shell test | (1) moving a section moves its slides in one undo step | E9-1 |
| E9-5 | S2 | Object animations (field 426): build in/out on click | M / P2 | `decks/`, `present.rs` | player tests | (1) click order is respected; (2) OUT runs before the slide transition | X2, E9-1 |
| E9-6 | J1 | Table node: cells, resize, add/remove row/col, merge (es-designer) | L / P2 | `node.rs`, `board.rs`, `board/*`, render | doc + render tests | (1) merging 2×2 cells renders one cell; (2) `.fig` round-trip | X1, REA |
| E9-7 | J2 | Mermaid flowchart paste → shapes and connectors | M / P2 | `clipboard.rs`, `board/create.rs` | parser golden tests | (1) `A-->B-->C` gives 3 shapes and 2 attached connectors; (2) invalid input shows an error and changes nothing | Q3 |
| E9-8 | J2/S2 | Local templates: save, insert, bundled set | M / P2 | `home/`, `designos-library` | library tests | (1) works offline; (2) a template inserts with fresh IDs | Q2 |
| E9-9 | J3 | Mind map | L / P3 | TBD | TBD | TBD | REA, Q4 |

### 5. Risks and unknowns
- **REA still needed:**
  - table cell → row/col binding;
  - the mind map model;
  - Figma's names for TransitionType 0–25;
  - which Mermaid diagram types Figma supports;
  - the slide-section model;
  - whether `.deck`/`.jam` files share the `.fig` container. INFERENCE: yes.
- **A second-order risk with E8:** if E9 adds non-Figma transition kinds, PPTX mapping becomes lossy.

---

## Simplest viable path overall (INFERENCE)
Build X1 and X2 first, then ship one pack per epic in this order:
1. **S1:** smallest, REA-complete.
2. **D1:** shared geometry, no new GPU programs.
3. **M1:** GIF/PNG only, no new dependencies.

Video (M3) and brushes (D4) are the two XL tails. Keep them last and behind owner decisions.

Pack versions are ASSUMPTION: E1–E3 take v0.7–v0.8, so the controller would map S1, D1 and M1 to v0.9 and later.

## Unresolved questions (for the controller to put to the owner via AskUserQuestion)
- E4 Q1, Q2
- E6 Q1, Q2, Q3, Q4
- E9 Q1, Q2, Q3, Q4
- Whether X1 may add a `deny.toml` licence gate (I recommend it).

Status: DONE_WITH_CONCERNS
Summary: The brainstorm and executor-ready task breakdown for E4, E6 and E9 are ready, grounded at file:line, with 2 shared prerequisites (typed kiwi passthrough, easing moved into `designos-doc`) and 10 owner questions.
Concerns/Blockers:
- Crate licences outside `Cargo.lock` (rav1e, mp4, objc2-video-toolbox) and image-webp being lossless-only are unverified (ASSUMPTION/INFERENCE).
- Several tasks (E4-8/9/12, E9-6/9, E6-9) are blocked on REA reads.
- Assumption disclosure:
  - **Pack-to-version mapping (low confidence).** Changes if the owner's roadmap puts E4/E6/E9 earlier.
  - **X1 assumes `fig_export` can encode passthrough fields with its export schema (medium confidence).** If it only knows a fixed schema subset, X1 grows to XL.