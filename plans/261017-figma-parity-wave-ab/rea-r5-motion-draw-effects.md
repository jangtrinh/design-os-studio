# REA R5: Motion, Draw, effects, FigJam/Slides, versions (Figma 126.9.13)

Evidence extracts are kept in the controller scratchpad (`rea/work/r5`). Unlabelled = FACT; INF = inference.

## Not determined
- Shader source: it is a server-side code asset.
- Units for the progressive blur offsets.
- The table cell → row/col binding field.
- The mind map model.
- The motion path coordinate space.
- Brush, eraser and offset geometry.
- Branch internals: server REST only.

## Motion (E6)
**Timeline.** `timelineDefinitions=552` is a `TimelineData`:

| Field | Id | Notes |
|---|---|---|
| `durationUs` | 1 | microseconds |
| `defaultTimeline` | 2 | |
| `parentTimelineDefId` | 3 | |
| `playbackStyle` | 4 | ONCE, LOOP or BOOMERANG |
| `autoplay` | 5 | |
| `keyframeSnappingFps` | 6 | |
| `quantizePlayback` | 7 | |
| `durationUsVar` | 8 | |

- `timelineAssignments=553` holds per-node `{offsetUs, disabled, autoplay}`.

**Node types.** KEYFRAME=56, KEYFRAME_TRACK=57, ANIMATION_PRESET_INSTANCE=58.
- Binding goes through `parameterConsumptionMap=445`.
- A track node has `keyframeOperation=509` (SET, SCALE or OFFSET) and `trackLoopMode=591`.
- A keyframe node has:
  - `timelinePosition` (int64 µs) = 506;
  - `keyframeValue` = 507;
  - `interpolationType` = 505 (HOLD, BEZIER or SPRING);
  - `bezierHandles` = 508;
  - `easingData` = 535;
  - `interpolationColorSpace` = 599 (DEFAULT, OKLAB, OKLCH, HSL or SRGB_LINEAR).

**Track targets (VariableField).**
- Translation: MOTION_TRANSLATION_X=46, MOTION_TRANSLATION_Y=47, combined XY=69.
- Rotation: MOTION_ROTATION=48.
- Scale: MOTION_SCALE_X=49, MOTION_SCALE_Y=50, combined XY=70.
- Shear: MOTION_SHEAR=51.
- Path trim: PATH_TRIM_START=54, PATH_TRIM_END=55.
- Other: OPACITY=31, CORNER_RADIUS=1, STROKE_WEIGHT=4, WIDTH=12, HEIGHT=13, TEXT_DATA=11.
- Effect tracks have 21 fields.

**Easing.** `EasingType` has 15 values (0–14): cubic presets, springs (GENTLE, Quick, Bouncy, Slow), CUSTOM_CUBIC and HOLD.

| Spring preset | mass / stiffness / damping |
|---|---|
| GENTLE | 1 / 100 / 15 |
| Quick | 1 / 300 / 20 |
| Bouncy | 1 / 600 / 15 |
| Slow | 1 / 80 / 20 |

| Cubic preset | Control points |
|---|---|
| IN | [.55, .055, .675, .19] |
| OUT | [.215, .61, .355, 1] |
| INOUT | [.645, .045, .355, 1] |
| IN_BACK | [.3, -.05, .7, -.5] |
| OUT_BACK | [.45, 1.45, .8, 1] |
| INOUT_BACK | [.7, -.4, .4, 1.4] |

- The default bezier is [0, 0, .58, 1].

**Animation styles.** `styleIdsForAnimation=582`, `animationPresets=521`.
- First-party presets: FADE, MOVE, ROTATE, SCALE, PATH_DRAW, PATH_TRAVEL and others.
- About 60 named composite styles.
- Text stagger: `textElementDelayBehaviour=610` (grouping by char, word or line; order; delayMs).

**Path trim.**
- `pathTrim` = 542 on the node and 12 on Stroke. INF: x = start, y = end, both 0..1.
- The PATH preset has keyframe recipes for draw, erase and travel.

**Export.**
- `ImageType` adds MP4=4, GIF=5, WEBM=6, LOTTIE=7 and DOTLOTTIE=8.
- New `ExportSettings` fields: `frameRate=13`, `loopCount=14`, `backgroundType=15`, `duration=16`, `svgAnimated=17`.

| Format | Defaults and allowed values |
|---|---|
| MP4 / WebM | fps 30, quality LOW, MEDIUM or HIGH; plugin fps {12, 24, 30, 60} |
| GIF | fps 15, loop 0 = forever; plugin fps {8, 12, 15, 24, 30} |
| Lottie | ≤ 60 fps |

- Lottie lint rejects angular/diamond gradients, shaders, 3D, video and audio. Shadow, glass and noise are rasterized from the first frame.

**MVP (L).**
1. Lossless import of the timeline, keyframes and tracks.
2. Model: `Timeline{duration_us, playback, autoplay, snap_fps}` and `Track{field, op, keyframes[t_us, value, interp]}`.
3. An evaluator for opacity, transform, corner radius, stroke weight, size and path trim.
4. A headless PNG sequence.

Export effort: animated SVG M, CSS keyframes M, Lottie L, MP4/WebM XL.

## Draw (E4)
- **Variable width.** `variableWidthPoints=447` holds `{position, ascent, descent, segmentId}`.
  - Presets: UNIFORM, WEDGE, TAPER, QUARTER_TAPER, EYE, MIRRORED_TAPER and CUSTOM.
  - Not allowed on dashed strokes.
- **Dynamic stroke.** `dynamicStrokeSettings=448` holds `{frequency 0.01–20, wiggle, smoothen 0–1}`, with `strokeSeed=482`.
- **Brushes.** `brushType=452` (STRETCH or SCATTER).
  - `strokeBrushGuid=446` points to a BRUSH node (46).
  - `scatterStrokeSettings=449` holds `{gap, wiggle, angularJitter, rotation, sizeJitter}`.
  - 10 scatter and 15 stretch built-in brushes.
- **Multi-stroke.** `strokeData=571` holds Stroke[] with `strokeOffset=554` and `StrokeAlign.OFFSET=3`.
- **Pattern fill.** Paint PATTERN=8.
  - `sourceNodeId=26`, `patternTileType=28` (RECT, H-HEX or V-HEX).
  - `patternSpacing=37`, `scale=12`, alignments 29/30.
- **Transform group.** TRANSFORM=48 with `transformModifiers=455`.
  - At most 2 modifiers: REPEAT (count 2–40, LINEAR or RADIAL), SYMMETRY or SKEW.
- **Text on path.** TEXT_PATH=41 with `textPathStart=432` `{tValue, forward}`.
- **MVP (M):** pathTrim, variable width, strokeOffset, REPEAT and pattern fill, then dynamic stroke. Round-trip every Draw field first. Defer brushes (L), text on path (L) and eraser/offset (L).

## Effects (E5)
- **Progressive blur.** `blurOpType=19` set to PROGRESSIVE.
  - `radius=4` is the end radius; `startRadius=21`.
  - `startOffset=20` and `endOffset=28` are Vectors.
  - Plugin form: `{blurType:'PROGRESSIVE', radius, startRadius, startOffset, endOffset}`.
- **Glass.** Kiwi fields: refraction 36, depth 30, specularAngle 31, specularIntensity 32, dispersion 34, splay 35, bevel 33.
  - Plugin names: refraction 0–1, depth 1–100, lightAngle, lightIntensity 0–1, dispersion 0–1, splay 0–1.
  - The studio's GlassEffect lacks splay.
- **Noise and texture.** `noiseType` (MONO, DUO or MULTI), `noiseSize`, `density`, `secondaryColor`, `opacity`, `seed`.
  - Texture is GRAIN=6, with `clipToShape=23`.
- **Shader.** Effect CUSTOM=9 or Paint CUSTOM=10, through a code-component ref plus prop assignments as uniforms. XL; keep the refs only.
- **MVP (S–M):** add glass splay, progressive blur, and map GRAIN to Texture.

## FigJam / Slides (E9)
- **Table.** TABLE=29, TABLE_CELL=30.
  - Row/column positions 308/309 (fractional index).
  - Heights/widths 310/311.
  - `tableMergedCells=538` `{rowId, colId, rowSpan, colSpan}`.
- **Slides.**
  - Node types SLIDE=32, SLIDE_GRID=37, SLIDE_ROW=38.
  - Fields `slideSpeakerNotes=389`, `isSkippedSlide=410`, `slideNumber=443`.
  - Transition = one `prototypeInteractions` ON_CLICK entry on the SLIDE, using `TransitionType` 0–25 and a duration.
  - Object animations: `objectAnimations=426`, type FADE or SLIDE_FROM_*, phase IN or OUT.
- **MVP (M):** slide transitions (S), skipped slides (S), section status (S), object animations (M), tables (M–L). Defer mind maps.

## Versions (E10)
- Kiwi has no branch fields. Branching goes through server REST: create, sync, merge requests, checkpoint.
- In-schema: `DiffPayload` (diffType BRANCHING or NODE_CHANGES_ONLY), `Message.originFileKey=47`.
- **MVP (S):** a local copy-on-write branch plus a DiffPayload-style diff and branch metadata. Hosted merge/review is out of scope.
