# Review of the "True Figma" specs E8 to E13 (2026-11-12)

Checked against `origin/main` at b731e4359 (2026-11-12). The specs say they are written against e0e44f3c2; main is 10 commits ahead, none of them touch the areas below. Read-only review: no repo edits, no cargo, no GitHub writes. Pull requests 1946, 1947 and 1950 were merged after this review was written and are not reflected below.

How to read the counts. Code references = existing-code claims the reviewer could identify in the spec (path, type, function, field, crate, line), not things the spec proposes as new. Column order: found exactly / found elsewhere or renamed / not found. Figma facts = statements about what Figma stores or does. SOURCED = the reviewer checked it (repo schema `crates/designos-fig/tests/data/figma-real-schema.txt`, or a string in the installed Figma 126.11.7 app). ASSERTED = no checkable source. CONTRADICTED = conflicts with repo code or docs, or with the spec's own evidence file.

## Verdict table

| Spec | Verdict | One-line reason | Code refs (exact / elsewhere / missing) | Figma facts (sourced / asserted / contradicted) |
|---|---|---|---|---|
| E8 PPTX import and export | READY AFTER FIXES | Export core is sound; the "probe" slice rests on a file and enum that do not exist; transition table unsourced | 9 / 7 / 4 | 4 / 6 / 0 |
| E9 connectors, mind map, Mermaid, freehand | NEEDS OWNER DECISION | Native `CONNECTOR` export in `.fig` contradicts the standing vector-export rule; most connector work shipped in 0.7.10; new node kinds vs "no file format change" | 18 / 2 / 1 | 11 / 5 / 1 |
| E9 slides, transitions, builds, deck round trip | READY AFTER FIXES | Invents parallel `TransitionType` and `PrototypeEasing` next to shipped `Transition`, `TransitionKind`, `Easing`; probe premise wrong | 9 / 3 / 2 | 6 / 3 / 1 |
| E10 three-way diff, merge, git driver | SUPERSEDED (engine); UI and CLI parts need a rewrite | Diff engine and merge with id remap shipped (#343, #344, #345); `branch_diff.rs` already exists, 1,900 lines | 2 / 6 / 5 | 1 / 3 / 0 |
| E11 proto crate, triggers, sticky, Smart Animate | READY AFTER FIXES | Crate split and Kiwi mapping match open #362 and #363 once aligned; sticky math and match threshold are upgraded from inference to fact | 9 / 1 / 1 | 3 / 3 / 2 |
| E11 video, audio, GIF, Present UX, MCP | NEEDS OWNER DECISION | Native video decoders and a wgpu path that Present does not use; duplicates E12 video (#389) | 5 / 1 / 0 | 3 / 3 / 0 |
| E12 canvas a11y, contrast, eyedropper, video | READY AFTER FIXES (video slice: NEEDS OWNER DECISION) | Keyboard facts wrong for Windows and Linux; AccessKit snippet does not fit egui; nudge and resize already shipped | 7 / 8 / 2 | 3 / 5 / 2 |
| E12 variable fonts, rich text, decorations, smart symbols | READY AFTER FIXES | Variable fonts, rich-text round trip and skip-ink flag already on main; only decoration style, thickness, offset, fills are new | 11 / 1 / 1 | 4 / 4 / 1 |
| E13 code layers, sandbox, MCP | NEEDS OWNER DECISION | Runs user code in a web view with a bundled React runtime; cites a crate that does not exist; CSP contradicts itself | 6 / 1 / 3 | 3 / 3 / 0 |
| E13 Sites, breakpoints, scroll, CMS, compiler | READY AFTER FIXES (compiler and preview server: NEEDS OWNER DECISION) | Schema work is well grounded; a new loopback server and WOFF2 encoder are decisions | 4 / 1 / 0 | 3 / 3 / 0 |

Concrete defects found (all verified): decimal axis-tag values in the E12 text spec are all wrong (hex is right); `ttf-parser` 0.25.1 has no `fs_type` field (use `permissions()`); crate `oxc_transpiler` does not exist on crates.io (the real crates are `oxc_transformer` and `oxc_codegen`); E9 README gives table node numbers 35 and 36 where the schema has 29 and 30.

## What the review could not verify

1. The raw Figma artefacts the specs cite by line (`wasm-strings.txt`, `proto.strings.txt`, `prototype-lib-glue.js`, `plugin-api.d.ts`, `en.json`) are not in the repository and were not available to the reviewer. The only Figma 126.11.7 material searched was the installed app's `app.asar` and the 81 MB V8 code cache of JavaScript. The WASM and the brotli HTTP cache were not decoded.
2. So these stay unchecked: the container magic `fig-deck`, the PPTX importer binding names, `FGSmartAnimateTransitionBake`, the hotspot flash duration and colour, every `en.json` UI string (branching, PPTX dialog), and the numeric action id 888 for the Mermaid action.
3. The dossiers in [evidence](../evidence/) are the specs' own working notes, not Figma-authored sources. The reviewer used them only to compare labels (for example, the sticky formula is labelled inference there) and did not treat them as proof.
4. PowerPoint markup (direction attribute meaning for wipe, push, cover, pull; the morph element namespace) was judged from memory, not from ECMA-376 or MS-PPTX.
5. Compile-ability of every snippet was judged by reading; nothing was built.
6. Hot-file queue positions (33 to 36 in E9) were not reconciled with the queue in the open issue bodies (#363 says position 46, #397 says 50).
7. The reviewer did not read the [index](../EPIC-FIGMA-SPECS-INDEX.md) or the e1 to e7 and e19 to e20 specs for this part.
8. Counts are the reviewer's own enumeration and can differ by one or two from another reader's.

---

<a id="e8-pptx"></a>

## 1. PPTX import and export in E8 ([`e8-interop-pptx/e8-pptx-import-export-spec.md`](../e8-interop-pptx/e8-pptx-import-export-spec.md))

**a. What it says.** Six slices. Slice PPTX-1: a zip probe that tells PPTX, Sketch and zipped `.fig` apart (#307 territory). Slice PPTX-2: import a PPTX into a deck (#290): slide size from EMU, shapes, text, notes. Slice PPTX-3: an OpenXML package writer (#294). Slice PPTX-4: shapes and text as DrawingML. Slice PPTX-5: two fidelity modes, "most editable" and "flatten visuals" (visuals rasterised, text kept live). Slice PPTX-6: menu entry and fidelity dropdown. Claims Figma exports PPTX in the client, and maps its 26 transitions to PowerPoint transition tags.

**b. Code references (9 exact, 7 elsewhere or renamed, 4 missing).**
- Missing: `crates/designos-library/src/header.rs` (and `probe.rs`); the `FileKind` enum; `zip` in designos-library (zip 9 lives in designos-fig only); `crates/designos-app/src/menu.rs` (the File menu is `MenuKind::FileMenu` in `panels/menus.rs`).
- Elsewhere or renamed: "8 KB header probe" is `Library::file_kind` in `designos-library/src/store.rs:769`, and it reads the stored `.designos` JSON `kind`, not magic bytes; `render_to_pixmap` is `render_node` / `render_page` in designos-render; `Paint::Solid(c)` is `Paint { kind: PaintKind::Solid(c) }`; `to_hex_rgb` is `Color::to_hex` (`paint.rs:140`); `html_escape` is `esc` / `escape_html` in designos-export; `SlideProps` is at `node.rs:169`, not 158-163; direct `rfd::FileDialog` is not how decks export (they use the `FileDialogs` trait, with an `Unavailable` branch).
- Exact: `decks/export.rs`, `deck_pdf_slides`, `safe_filename`, `export_doc`, `show_toast`, `node.transform.tx`, `node.size`, `TextProps` and `TextStyle` fields, slide = `Frame` plus `node.slide`.

**c. Figma facts (4 sourced, 6 asserted, 0 contradicted).**
- SOURCED: fidelity names `MOST_EDITABLE`, `FLATTENED_MOST`, `FLATTENED_ALL` are in the installed Figma JS; the PPTX MIME string and `notesSlide` are in the same code (consistent with client-side export); `SLIDE = 32`, `slideSpeakerNotes = 389` are in the schema.
- ASSERTED: the transition-to-tag table; the "Complete" / "Flatten visuals" strings (not found); the `FGSlidePptxImporterBindings` module and the telemetry key; the JS function names `getSlidesJSON`, `canvasGridArray`; the EMU divisor 9525 (arithmetic is right, Figma use unsourced); numeric values 0, 1, 2 of the fidelity enum.
- Internal gap: README-style claim "26 variants map 1:1" but the table covers 21 (nothing for the four move-out variants or scroll animate). The morph row uses `p16:morph`; the reviewer believes PowerPoint's morph is `p159:morph` inside an alternate-content wrapper. Verify before coding.

**d. Already on main.** Deck PDF export (`decks/export.rs`) and notes (`slide.notes`). No PPTX code, no XML writer. `roxmltree` 0.20 is already an app dependency (usable for import). Decks have no transitions yet, so the V-PPTX-5 test depends on the E9 slides work.

**e. Issues.** Issue #290 is open "Import PPTX into a deck", #294 open "Export decks to PPTX", #21 open (epic E8), #307 and #308 open (Sketch). Titles match. Milestones are not v0.8: #290 and #294 sit in "Backlog: AI design 2 roadmap & overflow", Sketch in "v0.26".

**f. Roadmap and risks.** No roadmap conflict. IP: the spec quotes two Figma UI sentences verbatim while claiming own-words clean room; write own labels. New dependency: the zip writer in designos-export (zip is MIT or Apache-2.0, already in the workspace), no copyleft. AGENTS.md section 4: raster path uses designos-render, off the UI thread is not stated; the export runs on the UI thread in the spec's sample, which would stall on a big deck.

**g. Verdict: READY AFTER FIXES.** Corrections: (1) drop slice PPTX-1 as written; PPTX detection belongs in `designos-fig/src/container.rs::open` (today it returns "missing fig-kiwi magic or zip header" for any zip that has no `canvas.fig`), coordinated with #307; (2) use `FileDialogs` and the `FileRequest::save` pattern from `export_deck_pdf_with`; (3) rename the six wrong symbols above; (4) mark the transition table and morph markup as to-verify, or gate on a real PowerPoint file; (5) run export off the UI thread; (6) own wording for the two fidelity labels.

---

<a id="e9-connectors-mermaid"></a>

## 2. Connectors, mind maps, Mermaid, freehand in E9 ([`e9-1215-connectors-mindmap-mermaid-freehand-spec.md`](../e9-slides-whiteboard/e9-1215-connectors-mindmap-mermaid-freehand-spec.md), plus README)

**a. What it says.** Slice CONN-1: grow `Magnet` from 6 to 10 states, add `relative_position`, `control_points`, `is_diagram`, and a 15-variant `ConnectorCap`. Slice CONN-2: write `CONNECTOR = 19` with fields 242 to 255 into `.fig`. Slice CONN-3: obstacle-avoiding elbow router with 20 px clearance, 256-cell budget. Slices MIND-1/2: tree layout from `layoutRoot`, `diagramParentId`, `layoutPosition`, with a codec. Slices MERM-1/2: a hand-written parser for six Mermaid types plus paste and a quick action. Slices FREE-1/2/3: `NodeKind::Highlight` and `Brush`, pressure widths, Catmull-Rom tessellation, `HIGHLIGHT = 24` and `BRUSH = 46` codec. Issues named: #22, #1215.

**b. Code references (18 exact, 2 renamed, 1 missing).**
- Missing: `crates/designos-app/src/canvas/tool_draw.rs`.
- Renamed: `ConnectorCap` exists but with other variants (`None, Arrow, Triangle, Circle, Diamond`, 5 total); `Node::connector(id, from, to)` is `Node::connector(id, props)`.
- Exact includes `board.rs` (doc and render), `Magnet`, `ConnectorEnd`, `ConnectorProps`, `ConnectorLine`, `Node::shape_with_text`, `outward()`, `SceneIndex` (in `designos-render/src/index.rs`), `Tool::Highlighter`, `BlendMode::Multiply`, `App::paste`, `fig.rs`, `fig_export.rs`.
- Omitted by the spec: an existing codec `crates/designos-fig/src/board.rs` that already maps `STICKY`, `SHAPE_WITH_TEXT`, `CONNECTOR`, `STAMP` and connector endpoints.

**c. Figma facts (11 sourced, 5 asserted, 1 contradicted).**
- SOURCED (schema, exact): node types 17, 18, 19, 22, 24, 46; connector fields 242 to 255 and 373; `ConnectorEndpoint` and the 10 magnets; `StrokeCap` 0 to 20 (ERD caps are 15 to 20); mind-map fields 362, 363, 364, 366, 367, 368; `VariableWidthPoint`; stroke settings 448 to 452; no mind-map node type exists in the schema; `fig-jam` magic in installed Figma (Studio already has `MAGIC_JAM`); action name `MERMAID_TO_DIAGRAM` in the installed JS.
- ASSERTED: that waypoints are saved so reopening needs no recalculation (the earlier connector spec lists this as unknown); mind-map composition and fractional-index semantics; Mermaid action number 888 and the six supported types (strings for flowchart, mindmap, gantt exist; state, er, timeline not found); multiply blend and 45 degree chisel for highlighters; router clearance and budget (labelled inference by the spec).
- CONTRADICTED: emitting `CONNECTOR` for design files contradicts the closing rule of the connector-in-design-canvas spec of 2026-11-04 (not in the repository): design `.fig` export stays a vector with a carrier until an owner-manual check shows Figma Design keeps a `CONNECTOR`. That spec also keeps the raw magnet name and `relativePosition` in `fig_metadata` instead of widening the enum.
- Internal inconsistency: V-CONN-3 says children sit 160 px right with a 24 px gap; the algorithm says 120 px gap.

**d. Already on main.** Live connectors in design files shipped in 0.7.10 (tool X, magnets, re-route, `connect` batch op, cached routes, `.fig` export as vector with a marker, import back to live). An elbow router with a coordinate-grid search, endpoint-box avoidance and `ELBOW_STUB = 24` exists in `designos-doc/src/board.rs` (`elbow_with_steps`). Slices CONN-1 (partly), CONN-3 (only intermediate obstacles are new), CONN-2 (board codec exists) overlap. Marker and Highlighter tools exist (vector strokes, 24 px wide, alpha 0.4, round cap, no pressure, no multiply). Nothing exists for mind maps, Mermaid, `Highlight`/`Brush` kinds, or variable-width strokes. Connector home is the nearest common frame (`connector_home.rs`); the spec never mentions this roadmap rule.

**e. Issues.** Issue #22 is open (epic E9), #1215 open "[PM] Epic: Whiteboard tools in the design canvas". Related: #332 to #335 (Mermaid, v0.23), #1527 and #1574 (Mermaid renderer spike and Rust 1.95 bump). Titles match.

**f. Roadmap and risks.** Honours "whiteboard merges into the design canvas" in words. Not addressed: Arrow and Connector as separate tools, and the nearest-common-frame parent rule. Issue #1215 states "no file format change or migration"; renaming `ConnectorCap` variants and adding two `NodeKind`s needs a serde alias plan. The non-goal list of #1215 names Mermaid for v0.23, and #1527 says spike the renderer first (native subset, existing crate, or JS); the spec's hand-written parser pre-empts that. The IP rule in #1215 (original design:os names) applies. No new dependency.

**g. Verdict: NEEDS OWNER DECISION.** Decisions: native `CONNECTOR` export in design `.fig`; new `Highlight` and `Brush` node kinds and cap renames versus "no format change"; Mermaid parser versus the open spike. Fixes regardless: drop the 10-state enum rewrite in favour of `fig_metadata`, rebase CONN-3 on the shipped router (add intermediate obstacles only), add the nearest-common-frame rule to Mermaid lowering.

README notes: it says tables are Kiwi `TABLE = 35`, `TABLE_CELL = 36`; the schema has 29 and 30 (contradicted). Its slot positions 33 to 36 were not reconciled.

---

<a id="e9-slides-transitions"></a>

## 3. Slides, transitions, builds, deck round trip in E9 ([`e9-slides-transitions-builds-deck-roundtrip-spec.md`](../e9-slides-whiteboard/e9-slides-transitions-builds-deck-roundtrip-spec.md))

**a. What it says.** Slice SLIDE-1: widen `SlideProps` (skip flag, number mode and separator, theme, transition, builds) and add 26-variant `TransitionType`, build enums and `ObjectAnimation`. Slice SLIDE-2: detect `fig-deck` in an 8 KB probe and open as a deck. Slice SLIDE-3: `.fig` codec for slide, grid, row and interactive element nodes and fields 389, 410, 443, 456, 381, 426. Slice SLIDE-4: all 26 transitions and spring presets in the player. Slice SLIDE-5: keyboard-stepped text builds by character, word, line, paragraph. Issue named: #22.

**b. Code references (9 exact, 3 renamed, 2 missing).**
- Missing: `header.rs` and `FileKind` (same premise error as E8); `encode_prototype_interactions`.
- Renamed: `PrototypeEasing` is `Easing` / `EasingType` in `designos-doc/src/prototype.rs`; `SlideProps` at `node.rs:169`; `en(...)` and `msg(...)` live in `designos-fig/src/testkit.rs`.
- Exact: `DocumentKind::Deck` (`board.rs:18`), `Node::frame`, `NodeKind::Section`, `clips_content`, the five fig and doc files, `prototype.rs`, `decks/mod.rs`, `decks/ui.rs`, `render/text.rs`.
- Self-contradiction: `ObjectAnimation.animation_type` is typed `TransitionType`, but the schema's `animationType = 30` is a separate 6-value enum (`NONE, FADE, SLIDE_FROM_*`).

**c. Figma facts (6 sourced, 3 asserted, 1 contradicted).**
- SOURCED (schema, exact lines): node types 32, 34, 37, 38; fields 389, 410, 443, 456, 381, 390, 393, 371, 372, 402, 573, 426; `SlideNumber` 0 to 5; `TransitionType` 0 to 25; `EasingType` 0 to 15; `AnimationType`, `AnimationPhase`; the three `TextElement*` enums.
- ASSERTED: container magic `fig-deck` (absent from the installed JS, wasm not checkable); hierarchy details (singleton grid, default sizes); default duration 0.35 s; glosses on each transition.
- CONTRADICTED: spring names "Gentle, Quick, Bouncy" and damping 0.75, 0.85, 0.60 appear as "Verified" in test V-SLIDE-4, while the spec's own README says damping was not decompiled.

**d. Already on main.** Decks exist (new document, add slide, notes, Present, PDF export). `Transition { kind: TransitionKind, easing, duration_ms }` with kinds Dissolve, SmartAnimate, MoveIn, MoveOut, Push, SlideIn, SlideOut and a `Direction`; all 16 `EasingType` values, including the three spring presets; a Smart Animate planner (`proto/smart.rs`); `.fig` mapping in `designos-fig/src/proto.rs` (maps scroll animate onto smart animate; no instant, fade or magic move). A slide node imported from `.fig` today becomes `Unknown`. `DocumentKind::Deck` is never set from a `.fig` import. So SLIDE-1 and SLIDE-4 re-specify the transition model; the new parts are Instant, Fade, MagicMove, ScrollAnimate, skip flag, numbering, theme, builds and the codec.

**e. Issues.** Issue #22 is open. No per-slice issue numbers are named. Related open: #328 (play prototype flows inside slides).

**f. Roadmap and risks.** "Slides extras: keep" is honoured. Hot-file serial slot 33 is stated (checked against e1/e2 README only). Fade-through-colour and interactive widgets would introduce no new dependency. Present must not bump `TileView::rev`; the spec does not state it for the build sequencer (the E11 README does).

**g. Verdict: READY AFTER FIXES.** Corrections: (1) extend `Transition` / `TransitionKind` / `Easing` instead of adding `TransitionType` and `PrototypeEasing`; map the 26 values in `fig/proto.rs`; (2) type build animations with the 6-value animation enum; (3) replace slice SLIDE-2: the `.fig` magic list belongs in `designos-fig/src/container.rs` (`MAGIC_KIWI`, `MAGIC_JAM` already there) after `fig-deck` is sourced; (4) set spring damping from evidence or mark as Studio choice; (5) fix README table node numbers.

---

<a id="e10-branching"></a>

## 4. Three-way diff, branch merge, git driver in E10 ([`e10-three-way-diff-branch-merge-git-driver-spec.md`](../e10-versions-branching/e10-three-way-diff-branch-merge-git-driver-spec.md))

**a. What it says.** A new `branch_diff.rs` with `ThreeWayDiffReport`, `LayerDiff`, `MergeConflict`; FNV-1a subtree hashes to prune; node id remap with reference fixups; a merge review dialog (`home/branch_review.rs`) with "Pick Branch" and "Pick Main"; a `designos merge-driver` and `designos diff` CLI in a `designos-cli` crate; four conflict categories from Figma. Issue named: #23.

**b. Code references (2 exact, 6 renamed or elsewhere, 5 missing).**
- Missing: crate `designos-cli`; a `hash` field on `Node`; `rayon` in designos-doc; `measurement.from_node` / `to_node`; `VariableAlias`.
- Elsewhere: `crates/designos-doc/src/branch_diff.rs` exists with different content; `instance.component_id` (is `NodeKind::Instance { component }`); `action.destination`; `bound_variables` (is `NodeVariables`); `style_id`; `text_style_id`.
- Exact: schema lines 3656 to 3678 (`DiffChunk` and `DiffPayload`), core `Document` and `Node`.

**c. Figma facts (1 sourced, 3 asserted, 0 contradicted).** SOURCED: `DiffType`, `DiffPayload`, `DiffChunk` fields. ASSERTED: the four conflict buckets and every UI string (not found without `en.json`); the "measured under 420 ms on 723k nodes" figure (no such code or benchmark exists); that Figma merges at layer granularity only.

**d. Already on main (this is the main finding).** `branch_diff.rs` (1,900 lines): per-layer Added, Removed, Moved, Changed, MovedAndChanged, 27 property groups, longest-common-subsequence move detection, conflicts, id collisions, entity diffs (styles, variables, collections, flows, comments), a streaming hash. `branch_merge.rs`: `merge(...)` with id remap and a report; `designos-library/src/branches.rs`: create, load, save, archive, restore, delete, `merge_branch`, `update_branch_from_main`. CHANGELOG 0.6.0 says "Branches in the library, engine only". No UI, no CLI, no MCP for branches. The spec's types do not exist and would collide in role with `Diff3`, `Conflict`, `SideChanges`.

**e. Issues.** Issues #343, #344, #345 are CLOSED (storage, diff, merge). Open: #346 branch UI, #347 review-before-merge screen, #348, #349, #350, #355 "Add designos merge-driver and designos diff CLIs", #356 ".gitattributes for the merge driver". Epic #23 open. The spec cites none of these.

**f. Roadmap and risks.** "In-file branches: keep all" honoured. IP: "Pick Branch / Pick Main / Update from main file" are Figma strings; Studio already says "Before update from main". Use own words. Moving the diff to rayon adds a dependency to designos-doc (MIT or Apache-2.0); the shipped engine needs none. Review thumbnails render offscreen; they must not touch `TileView::rev`.

**g. Verdict: SUPERSEDED for the engine (sections 3 and 4 of the spec).** Rewrite as UI and CLI work on the shipped `Diff3` and `merge` API, mapped to #347, #346, #355, #356.

---

<a id="e11-proto-crate"></a>

## 5. Proto crate, Kiwi triggers, sticky scrolling, Smart Animate in E11 ([`e11-proto-crate-triggers-sticky-smart-animate-spec.md`](../e11-prototyping/e11-proto-crate-triggers-sticky-smart-animate-spec.md), plus README)

**a. What it says.** Slice P1: new crate `designos-proto`, egui-free, holding player, graph, Smart Animate, sticky and media clock. Slice P2: Kiwi round trip for media triggers (`ON_MEDIA_HIT`, `ON_MEDIA_END`, `mediaHitTime`), `MediaAction`, navigate reset flags, sticky scroll, device, starting point, `MakeContentState`. Slice P3: Smart Animate matching and a transform decomposition.

**b. Code references (9 exact, 1 elsewhere, 1 missing).**
- Missing: `Affine` as a tuple (`m.0[4]`); the real type has named fields `a, b, c, d, tx, ty`.
- Elsewhere: Smart Animate already exists at `designos-app/src/proto/smart.rs` (504 lines, `SmartPlan`); the spec writes a new `smart.rs`.
- Exact: `prototype.rs`, `designos-fig/src/proto.rs`, `present.rs`, `proto/player.rs`, `Reaction`, `Player`, `TileView::rev`, `AGENTS.md`, designos-doc types.
- Premise check: the player is already almost UI-free (2,605 lines), but `proto/geom.rs` imports `egui::Rect` and `crate::variables` is used; the crate split must deal with both.
- The sample `Cargo.toml` uses `tracing`, which is not in the workspace, and non-workspace dependency lines.

**c. Figma facts (3 sourced, 3 asserted, 2 contradicted).**
- SOURCED (schema): `InteractionType` (media at 12, 13), `MediaAction` 0 to 9, `ScrollBehavior` including `STICKY_SCROLLS = 2`, `PrototypeDevice` 184, `PrototypeStartingPoint` 249, `MakeContentState` 595, all `PrototypeAction` field numbers.
- ASSERTED: the names `computeJitSmartAnimateMatchPlan` and `bakeSmartAnimateTransition`; `shouldDissolve`; the behaviours in V-NAV and V-MEDIA tests.
- CONTRADICTED: the sticky clamp formula and tests V-STICKY-01/02 are tagged FACT, but the evidence dossier labels the formula "INFERENCE: derived from clean-room observations". The match score "ancestor distance at most 2" is not in the dossier, which says only that ancestor paths must be equivalent.

**d. Already on main.** Player, easing, Smart Animate and Present exist. Not on main: sticky (`proto.rs:216` collapses sticky into fixed), media triggers (dropped at `proto.rs:245`), device import, starting point (opaque, `fig.rs:730`).

**e. Issues.** Issue #123 is open (epic E11). The slices map to open #362 (crate split), #363 (Kiwi mapping, hot file), #364 (run sticky, resets, media triggers in the player). Titles match. Issue #362 says easing already moved to designos-doc and "designos-proto adds no easing copy"; the spec says P1 moves the easing, a conflict. Issue #363 puts its codec at hot-file position 46; the spec gives no position.

**f. Roadmap and risks.** v0.8 target is fine. Invariant 1 (no rev bumps in Present) is restated correctly. Voice trigger stays a stub (no recogniser, no network).

**g. Verdict: READY AFTER FIXES for P1 and P2; NEEDS EVIDENCE for sticky and the Smart Animate match rule (P3).** Corrections: (1) align P1 with #362 (acceptance: `cargo tree -p designos-proto` shows no egui; move `smart.rs`, do not rewrite); (2) fix `Affine` access; (3) relabel sticky formula and match threshold as inference; source them (decode the prototype WASM, or observe a Figma sticky file) before coding; (4) add the hot-file position.

---

<a id="e11-video-present"></a>

## 6. Video, audio, GIF, Present UX, MCP in E11 ([`e11-video-audio-gif-present-ux-mcp-spec.md`](../e11-prototyping/e11-video-audio-gif-present-ux-mcp-spec.md))

**a. What it says.** Slice P4: decode video with `VideoToolbox` and `MediaFoundation` straight into `wgpu::Texture`; audio-led master clock with at most one frame drift over 60 s; animated GIF via `animatedImage`. Slice P5: five present scaling modes; a 350 ms blue flash on miss-clicks. Slice P6: six `designos_proto_*` MCP tools.

**b. Code references (5 exact, 1 elsewhere, 0 missing).** `video.rs` (does contain Cisco OpenH264 behind an off-by-default feature and a PNG encode per frame, as the spec says), `present.rs`, `designos-mcp/src/lib.rs`, the `designos_*` tool naming, `Color`. Elsewhere: Present draws through tiny-skia into egui `TextureHandle`s (`ctx.load_texture`), not wgpu textures, so the sample `wgpu::Texture` path has no consumer in Present.

**c. Figma facts (3 sourced, 3 asserted, 0 contradicted).** SOURCED: `Paint` fields `image` 8, `animatedImage` 16, `animationFrame` 17, `video` 18, `videoFillPlayback` 47 and its struct (lines 883 to 895, 2600 to 2607; the spec cites 843 to 855); the viewport and content scaling mode names (`scale-down`, `scale-down-width`, `viewportScalingMode`, `contentScalingMode`, `ACTUAL_SIZE`, `FILL_SCREEN`) in the installed JS; the `hotspot-hints` / `showHotspots` Present parameter. ASSERTED: the scale formula per mode (note `FILL_SCREEN` is listed as `contain` but uses `max`, which is cover); the hotspot colour 18A0FB, the 350 ms, the alpha values (18A0FB appears in Figma only inside a generic colour table); "no DRM".

**d. Already on main.** Video fills with Motion JPEG decode in pure Rust, H.264 behind a feature, poster frame and badge; Present with letterboxed stage only; no GIF animation, no scaling modes, no hotspot flash, no prototype MCP tools.

**e. Issues.** Issue #123 is open. Related: #366 "Add Present zoom modes, hotspot hints, rotation and a current device catalogue" (open, v0.36), #374 (play video, GIF and audio in the web player), #389 (video on the canvas, E12). Titles match.

**f. Roadmap and risks.** New platform FFI (VideoToolbox, MediaFoundation; the sample uses a raw `c_void`) is new unsafe surface and new crates. Cisco OpenH264 is BSD-2 but ships a patent-licensed binary; check before moving to the default build. The spec says E11 video and E12 on-canvas video both decode video, in two crates (`designos-app/src/video.rs` and `designos-gpu/src/video.rs`) with different decoders (VideoToolbox versus ffmpeg).

**g. Verdict: NEEDS OWNER DECISION.** Decide one video pipeline for Present and the canvas (#389), the decoder per platform, and whether native FFI is acceptable. Scaling modes and MCP tools could be implemented now under #366 after: removing the colour and 350 ms as facts (own values, own colour), fixing `FILL_SCREEN`.

---

<a id="e12-canvas-a11y"></a>

## 7. Canvas accessibility, contrast, eyedropper, video in E12 ([`e12-canvas-a11y-contrast-eyedropper-video.md`](../e12-text-fonts-a11y/e12-canvas-a11y-contrast-eyedropper-video.md), plus README)

**a. What it says.** A viewport-culled AccessKit tree from `SceneIndex::query_rect`; F6 cycling through five regions; nudge 1 and 10 px, resize with Cmd; enhanced-contrast tokens with 7:1 text; a macOS `NSColorSampler` eyedropper; on-canvas video in an indirect texture slot (300 MB cap).

**b. Code references (7 exact, 8 elsewhere or renamed, 2 missing).**
- Missing: the `handle_key_events` function; `cocoa`, `objc`, `dispatch` crates (the app uses `objc2` with a trimmed feature list).
- Elsewhere: `designos-render/src/scene_index.rs` is `index.rs`; `query_rect` takes `(f32, f32, f32, f32)` and an out vector; `accesskit` is used as `egui::accesskit` and egui owns the `TreeUpdate`; `doc.selection` is `app.selection`; `doc.pages` holds `NodeId`s, not nodes; `NodeKind::Ellipse` is a tuple variant; `nudge_selection` / `resize_selection` are `nudge` and `resize_selection_by`; `Document::media` is `Document::images`.
- Exact: `agent_a11y.rs` (it covers the Agents panel; no canvas tree), `shell.rs`, `theme.rs`, `design/design.tokens.json`, `SceneCompiler`, `wgpu::TexelCopyTextureInfo` (wgpu 29), `TileView::rev`.

**c. Figma facts (3 sourced, 5 asserted, 2 contradicted).**
- SOURCED: F6 cycling exists (Mac: F6 and Shift+F6; other platforms: Ctrl+F6); `prefers-contrast` appears in the installed JS; the name `isDecorativeImage`; keyboard nudge settings `nudgeAmount` and `bigNudge` exist.
- ASSERTED: the order of five regions (the handler only posts a message to the parent window); the enhanced-contrast colour values (they are Studio design values tagged FACT, and `#1E1E1E` is not a Studio canvas token); that Figma uses `NSColorSampler` (not found); the 300 MB cap; ffmpeg.
- CONTRADICTED: "F6 on macOS and Windows" versus Figma's own code and the spec's dossier (Ctrl+F6 outside Mac); five regions versus open #387, which says "F6 cycles focus through 4 regions".

**d. Already on main.** Keyboard nudge and resize shortcuts with tests (`ops.rs`, `interact.rs`); Agents-panel screen-reader support; an in-canvas eyedropper (#388 is the screen-wide version); Display P3. Not on main: canvas AccessKit tree, F6 regions, enhanced contrast mode, native sampler, canvas video.

**e. Issues.** Issue #124 is open (epic E12). Related: #387 (keyboard, F6), #388 (screen eyedropper; already says NSColorSampler on macOS), #389 (video), #385, #828, #1019. No issue numbers are named in the spec itself.

**f. Roadmap and risks.** Invariants stated (no `to_json`, no rev bump on focus) are right. ffmpeg is LGPL or GPL depending on build: a copyleft decision. New crates (`cocoa`, `objc`, `dispatch`) duplicate `objc2`. The AccessKit sample cannot call a raw `Tree::new` while egui owns the adapter.

**g. Verdict: READY AFTER FIXES; the video slice needs the owner (see section 6).** Corrections: F6 per platform; region count agreed with #387; the tree through `ctx.accesskit_node_builder`; correct field and function names; use `objc2-app-kit` for the sampler; treat contrast values as Studio design values and run them through the existing contrast checker; drop nudge and resize (shipped).

---

<a id="e12-fonts-rich-text"></a>

## 8. Variable fonts, rich text, decorations, smart symbols, font gate in E12 ([`e12-variable-fonts-rich-text-decorations-smart-symbols.md`](../e12-text-fonts-a11y/e12-variable-fonts-rich-text-decorations-smart-symbols.md))

**a. What it says.** Variable-font axes and automatic `opsz`; mixed inline text through `styleOverrideTable`; wavy and dotted decorations with skip-ink; a smart-symbol table; an `OS/2 fsType` gate on font upload.

**b. Code references (11 exact, 1 elsewhere, 1 missing).** Exact: `paint.rs` and the `TextStyle` shown (accurate, including `features` and `variation_axes`), `render/text.rs`, `css.rs`, `textedit.rs`, `panels/typography.rs`, `designos-fonts/src/lib.rs`, `home/dialog.rs`, `TextRun`, `fig.rs`, `fig_export.rs`. Elsewhere: `ttf_parser` is a dependency of designos-render, not designos-fonts. Missing: `os2.fs_type` (ttf-parser 0.25.1 offers `permissions()`).

**c. Figma facts (4 sourced, 4 asserted, 1 contradicted).**
- SOURCED: `FontVariation` and `fontVariations = 260`; `TextData` fields; decoration fields 35, 411, 412, 413, 415, 416 and the two enums; smart-quote and smart-text-substitution toggle actions exist in the installed JS.
- ASSERTED: "1-based index, 0 is base"; Dev Mode output form; every row of the substitution table; the `fsType` gate (nothing in Figma evidence).
- CONTRADICTED: automatic `opsz` tracking is tagged FACT in the spec; the dossier says inference.
- Defect: the decimal values of the five axis tags are all wrong (for example `wght` is 2003265652, not 1998084468); the hex values are right.

**d. Already on main.** Variable axes (`variation_axes`, typography panel with wght, wdth, opsz, slnt; `opsz` defaults to font size), CSS `font-variation-settings`, `.fig` import and export of `fontVariations`; mixed runs and the override-table round trip (`fig_export.rs:4232`); `decoration_skip_ink` stored and round-tripped. Not on main: decoration style, thickness, offset, fills (#397), smart symbols, any font-embedding feature (fonts are bundled, system, or fetched; there is no upload that stores a file in a document), a missing-fonts panel (#393).

**e. Issues.** Issue #124 is open. Related: #397 "Add text decoration style, thickness, offset and colour" (hot file position 50), #393 missing-fonts panel, #394 (system fonts with optical sizes). No smart-symbol issue.

**f. Roadmap and risks.** No roadmap conflict. A font licence gate has nothing to gate today. The wavy-underline pixel parity (CPU and GPU) must stay behind the `interactive()` guard (#397 acceptance).

**g. Verdict: READY AFTER FIXES for decorations only.** Corrections: strip slices that already exist; correct axis decimals; use `permissions()`; mark `opsz` as inference. Smart symbols: NEEDS EVIDENCE (the pair table is unsourced). Font gate: NEEDS OWNER DECISION (is document font embedding wanted at all).

---

<a id="e13-code-layers"></a>

## 9. Code layers, sandbox, MCP in E13 ([`e13-code-layers-runtime-sandbox-mcp-spec.md`](../e13-sites-code-layers/e13-code-layers-runtime-sandbox-mcp-spec.md))

**a. What it says.** Slice 6: `CodeSnapshot`, `CodeSourceInfo` model and an offline TSX to ESM transpiler. Slice 7: a loopback preview server, an offscreen web view that snapshots a poster into `CodeSnapshot.paints`, a CSP and a domain allow-list. Slice 8: MCP tools `designos_code_layer_update`, an extended design-context tool, `send_code_to_figma`. Issue named: #125.

**b. Code references (6 exact, 1 elsewhere, 3 missing).** Missing: crate `oxc_transpiler` (not on crates.io); `NodeKind::CodeLayer`; `doc.site_settings.allowed_domains` (no such field, and the spec defines none). Elsewhere: `send_code_to_figma` is already `implemented` in `docs/mcp-parity.md` as `designos_import_web_capture`, with a different meaning (code to design, not agent edits into a code component). Exact: `designos-mcp` `engine.rs` and `server.rs`, `designos-gpu/src/compile.rs`, `designos_get_design_context`, `designos-svg`, the rev and hash invariants.

**c. Figma facts (3 sourced, 3 asserted, 0 contradicted).** SOURCED: node types 40 to 45, 59, 64; `CodeObjectType`, `CodeSnapshotState`, `MakeContentState`, `CodeSourceInfo`, `CodeSnapshot` (the spec's line numbers 25 to 49 and 321 to 382 are wrong; content is right); `makeLocalBrowserPreview:sendMouseEvent` and `:serializeForH2D` exist in the installed `app.asar` (the cited offset 8622979 points at a neighbouring name). ASSERTED: that Figma never runs a web view while panning; that the native layer is a WebGPU bridge; the transpile times ("measured under 1.8 ms", no code exists).
Self-contradictions: the opening says `connect-src 'none'`; section 4.2 allows `connect-src http://127.0.0.1:*` and `'unsafe-eval'`; test V-CODE-03 says "no outbound packets". The snapshot is said not to change `doc_hash`, but it is stored in the document, so the hasher must skip it explicitly (not stated).

**d. Already on main.** None of it. The native Plugins panel that ships already runs downloaded bundles in a browser panel; its sandbox rules are the closest precedent. A manifest-import remnant was slated for removal (#1410).

**e. Issues.** Issue #125 is open (epic E13). Related open: #375 (REA study), #407, #425, #426 (MCP tools for sites and code layers). Titles match.

**f. Roadmap and risks.** The owner kept "Sites, code layers (E13)" on 2026-10-30 and dropped the plugin platform. Executing user code in a web view with a bundled React 18 (MIT), oxc crates (MIT), a loopback HTTP listener, and macOS WKWebView or headless Chromium is a runtime and a network surface the owner has not seen. Needs an explicit decision that this is not the dropped plugin runtime.

**g. Verdict: NEEDS OWNER DECISION.** Decide: run user code at all, which engine (system web view versus bundled Chromium), the dependency set, the CSP. Fix: crate names (`oxc_transformer`, `oxc_codegen`, `oxc_parser`, plus `oxc_semantic`), the missing `allowed_domains` model, the CSP contradiction, remove "measured" numbers.

---

<a id="e13-sites"></a>

## 10. Sites, breakpoints, scroll behaviours, CMS, compiler in E13 ([`e13-sites-breakpoints-scroll-cms-compiler-spec.md`](../e13-sites-code-layers/e13-sites-breakpoints-scroll-cms-compiler-spec.md), plus README)

**a. What it says.** Slices 1 to 5. Schema and model for `WEBPAGE`, `RESPONSIVE_SET`, primary and secondary breakpoints with override diffs; site settings; behaviours (appear, parallax, marquee, sticky); CMS collections and repeaters; a zero-JavaScript HTML and CSS compiler with media queries; a local preview server with a token and live reload.

**b. Code references (4 exact, 1 elsewhere, 0 missing).** `designos-export/src/html.rs` (exists, 1,121 lines, single-node HTML5 generator; the spec treats it as a base), "existing bit-exact CSS and Tailwind compiler", `session.rs` edit detection, the layout engine. Elsewhere: "existing `NodeChange` override resolution" is a Figma term; Studio's `NodeChange` is the branch-diff enum, and the override machinery is the instance override code.

**c. Figma facts (3 sourced, 3 asserted, 0 contradicted).** SOURCED: `EditorType::SITES = 4`, `WEBPAGE = 55`, `RESPONSIVE_SET = 39`, `RESPONSIVE_NODE_SET`, `CMS_RICH_TEXT`, `REPEATER`; breakpoint fields 398, 457, 458, 462, 500, 501, 522 and `DerivedBreakpointData`; `ResponsiveSetSettings`, `NodeBehaviors` and the behaviour messages; `WEBPAGE`, `isPrimaryBreakpoint`, `ignoreReducedMotion`, `addBypassLinks` appear in the installed JS. ASSERTED: that secondary breakpoints never fork a tree; that Figma Sites deploys on Cloudflare Workers or Fastly (nothing in the dossier); the Dakota translation internals.

**d. Already on main.** Nothing for Sites. `scrollBehavior` exists only through the prototype codec (sticky collapsed to fixed). Single-frame HTML export exists.

**e. Issues.** Issue #125 is open. Related: #407 (breakpoints), #425 (code layers in exports), #426 (MCP tools), #375 (REA study). Titles match.

**f. Roadmap and risks.** Sites kept by the owner. New: a loopback server with a token and server-sent events (a network listener; the fixed localhost port rule applies: one fixed port per project, one process); a WOFF2 encoder (no crate in the workspace; licence to be named); multi-page output. The `<= 8 ms` and rev invariants hold for a compiler that runs off the canvas path.

**g. Verdict: READY AFTER FIXES for slices 1 to 4 (model and codecs, hot file serial); NEEDS OWNER DECISION for the preview server and the WOFF2 dependency.** Corrections: relabel the Cloudflare claim as unknown; define how overrides resolve with the instance machinery; add a hot-file position; fix schema line citations.

---

<a id="owner-decisions"></a>

## Owner decisions these specs need

1. Design-file `.fig` export of connectors: keep the vector with carrier, or write native `CONNECTOR` (needs the owner-manual check named in the connector spec of 2026-11-04). Also: widen the `Magnet` enum, or keep raw names in `fig_metadata`.
2. New node kinds `Highlight` and `Brush` and a renamed `ConnectorCap`, against the #1215 rule "no file format change".
3. Mermaid: hand-written parser (E9 spec) versus the spike in #1527 (native subset, existing crate, or JS) and the Rust 1.95 bump (#1574).
4. One video pipeline for Present and the canvas: VideoToolbox and MediaFoundation FFI, ffmpeg (LGPL or GPL), or the existing OpenH264 feature. Binds E11 video and E12 video (#389).
5. Whether Studio documents embed fonts at all (the `fsType` gate has nothing to gate otherwise).
6. Whether running user code in code layers (React bundle, oxc crates, web view, loopback server) is acceptable given plugins were dropped; which engine; the CSP.
7. A loopback preview server for Sites: allowed, and with which port rule.
8. F6 landmark count: 4 (#387) or 5 (spec), and the per-platform key.
9. IP wording: own labels for PPTX fidelity and branch review actions.
10. Whether the E10 spec is closed as superseded and its UI and CLI parts re-filed under #346, #347, #355, #356.

## Suggested first slices

1. PPTX export, most-editable mode (E8 slices 3 and 4, issue #294): new module in `designos-export` only, no hot file, no UI dependence; add a zip writer; verify by opening in PowerPoint or Keynote.
2. Crate split of the prototype player (#362): a move with no behaviour change; acceptance in the issue; unblocks #363 and #364.
3. Review-before-merge screen (#347) on the shipped `Diff3` and `merge` API; no engine work.
4. Text decoration style, thickness, offset, colour (#397): schema fields are sourced; CPU and GPU parity test; hot-file position 50.
5. Slide metadata (skip, number, theme) and the `.fig` slide codec, after extending `Transition` instead of adding a parallel enum; needs the `fig-deck` magic sourced first.
