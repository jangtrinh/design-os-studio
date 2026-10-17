# New epics E11 Prototyping, E12 Canvas/a11y/text, E13 Sites & code layers, E14 iPad/mobile: FULL SCOPE

Brainstormer draft, 2026-10-17. Owner decisions D11.1–D14.3 pending.

Coordinator technical rulings:
- Split E5-4 into E5-4a (OS video decode, shared) and E5-4b (video fill model and round-trip).
- The Sites preview server is a runtime product feature. It uses an OS-assigned port plus a token on 127.0.0.1 and is not subject to the dev-server fixed-port contract.

**Next step:** the controller asks the 11 owner decisions (D11.1–D14.3, at most 3 per epic) through AskUserQuestion, then files the four epic issues. No E11–E14 epic issues exist on GitHub yet (FACT, from the issue list: the epics stop at #23 E10).

### Epistemic Ledger
- **FACT:** I read the four planning files, the REA r5 notes, and the code listed with file:line below. I used 2 GitHub REST calls; the first was rejected with HTTP 422.
- **INFERENCE:** whether the code can be moved to other targets (WASM build, iOS build). This comes from the dependency lists only. No cargo was run, as instructed.
- **ASSUMPTIONS:**
  - What Figma's "Smart symbols" does (low confidence).
  - That eframe runs on iOS (low).
  - That cosmic-text can apply arbitrary variable-font axes (low).
  - That Safari on iPad ships WebGPU (medium).
- **Not verified:**
  - Whether each prototype trigger actually works at runtime. I only counted matches in `player.rs`.
  - Whether a bulk "Export…" dialog exists. A grep found none.
  - Whether the panels are visible to VoiceOver.

**Fixes needed outside these epics, before filing:**
1. **Split E5-4** into E5-4a (OS video decode, shared) and E5-4b (video fill model and round-trip). E11-5, E12-8, E13-12 and E14-13 all consume E5-4a. Today E5-4's acceptance criterion (3) "Present plays it" overlaps E11-5.
2. **E6-30 (audio on the timeline)** should use the audio output from E11-6.
3. **E13-6 (site compiler) needs two E7/E8 tasks first:** T7.29 (HTML codegen) and T8.7 (embedded webview host, required by E13-17 and E11-16).
4. **Issue #40 (vector PDF)** stays as it is. E12-18 links to it rather than duplicating it.
5. **Hot-file queue.** Tasks marked 🔥 touch `fig.rs` and join the single merged queue after X1: E11-3, E11-7, E11-17, E12-9, E12-13, E12-16, E12-18, E13-3, E13-16.
6. **Already owned elsewhere** (not repeated here): text wrap Balance/Pretty is E2, text on path is E4, text animation is E6, WebP export is E6-11, Figma's 15 easing types are X2.

---

## E11 Prototyping

### 1. What exists today (FACT)
- **Data model** (`designos-doc/src/prototype.rs`):
  - 11 triggers (:24-62) and 13 actions (:115-176), including video controls (:179-191), conditionals and set-variable.
  - 7 transition kinds (:83), easing with bezier and spring (:65), overlays (:97).
  - Scroll direction (`Overflow`, :204), fixed children (:260), and a video fill stored alongside an image paint (`VideoFill`, :222).
- **Player and Present:**
  - The player is `proto/player.rs` (2,591 lines); Present is `present.rs` (981 lines).
  - Present has device letterboxing (:141, :524), a flow switcher and Restart (:447-462), a variables panel (:790), "unsupported codec" badges on videos (:766) and a URL confirm bar (:889).
  - It always fits the frame to the window (`stage_layout`, :131) and renders on the CPU every frame (:162-217).
- **Devices:** a hard-coded list of 8, iPhone 15 era (`proto/mod.rs:36-45`). No rotation.
- **Video:**
  - Only Motion-JPEG decodes by default. H.264 needs the off-by-default `openh264` feature (`designos-app/Cargo.toml:21-28`), which conflicts with the OS-decoder ruling.
  - Every frame is encoded to PNG and written into `doc.images` (`video.rs:612-616`). This is very slow.
  - There is no audio output: the player only tracks a `muted` flag (`player.rs:201`) and the app has no audio crate.
- **GIF:** rejected when placed (`canvas/interact.rs:1829`, test :2255).
- **.fig mapping** (`designos-fig/src/proto.rs`):
  - Sticky scroll is collapsed into fixed (:216).
  - Interaction types it doesn't know, including the media triggers, are dropped (:245).
  - `prototypeStartingPoint` is kept only as an opaque value (`fig.rs:730`). `prototypeDevice` is not imported at all (no matches).
- **"Make prototype":** a rule-based auto-wire (`cmdk.rs:1066`). No AI is involved.
- **Comments** are pinned to a page or node only (`comments.rs:9-13`). Present has no comment code.
- **Make embeds:** `makeContentState` is only passed through as opaque data (`fig.rs:739`).

### 2. Full scope
| Capability | State | Evidence |
|---|---|---|
| Triggers: click, drag, hover, press, mouse ×4, delay, key, gamepad | DONE (model and .fig) | prototype.rs:24, proto.rs:218 |
| Media triggers (video ends, video reaches a time) | MISSING | proto.rs:245 |
| Navigate options: reset scroll, video and component state | MISSING | no matches |
| Sticky scroll | PARTIAL | proto.rs:216 |
| Overlays, variables, conditionals, interactive components | DONE | prototype.rs:115-176 |
| Device presets, rotation, device models, .fig device import | PARTIAL | mod.rs:36 |
| Present zoom modes (fit/fill/100%/width) and hotspot hints | MISSING | present.rs:131 |
| Video in prototypes (OS decode, audio, smooth playback) | PARTIAL | video.rs:9-13, 612 |
| GIF in prototypes | MISSING | interact.rs:1829 |
| Comments on prototypes | MISSING | comments.rs:9 |
| Mobile view | MISSING | no web or mobile player |
| Offline present | DONE | native Present |
| AI "Add interactions" (through the agent CLI) | PARTIAL | cmdk.rs:1066, rule-based |
| Embedded Make/code prototypes | MISSING | fig.rs:739 passthrough only |
| Embedding prototypes elsewhere (local equivalent: HTML player export) | MISSING | — |
| MCP tools to read and write prototypes | MISSING (E7 lists none) | — |

### 3. Tasks
| ID | Title | Size/Pri | Pack | Touches | Test seam | Acceptance criteria | Deps |
|---|---|---|---|---|---|---|---|
| E11-1 | **REA:** prototype fields in 126.9.13: media trigger types and fields, Navigate reset flags, STICKY_SCROLLS, prototypeDevice (rotation/model), whether GIFs play on the canvas, hotspot hints, `makeContentState` | S/P1 | P1 | — | findings doc | (1) every field has an id and type; (2) anything unknown is labelled UNKNOWN | — |
| E11-2 | Move Player, smart animate, easing and the document-typed geometry into a new egui-free crate `designos-proto` | M/P1 | P1 | proto/*, present.rs | existing player tests | (1) `cargo tree -p designos-proto` shows no egui; (2) all player tests pass unchanged; (3) Present screenshots match before/after | — |
| E11-3 | Model and .fig mapping: media triggers, reset flags, sticky, device rotation/model, typed starting point and device | M/P1 | P1 | prototype.rs, settings.rs, proto.rs 🔥fig.rs | round-trip | (1) a Figma fixture round-trips; (2) old `.designos` files load; (3) sticky stays distinct from fixed | E11-1, X1 |
| E11-4 | Player runtime for E11-3: sticky scroll, resets, media triggers firing | M/P1 | P1 | designos-proto | player unit tests | (1) "video ends" fires once per play-through; (2) reset scroll restores 0,0 | E11-2, E11-3 |
| E11-5 | Present video pipeline: decoded frames go straight to a texture (no PNG step), seek and loop accuracy, WebM/VP9/AV1 where the OS supports them; drop `openh264` | M/P1 | P2 | video.rs, present.rs | perf probe | (1) 1080p30 H.264 plays at ≥30 fps on the reference Mac; (2) no `encode_png` call in the frame path; (3) the default build has no openh264 | E5-4a |
| E11-6 | Shared audio output service (OS audio; cpal on Linux): video audio plus all mute ops | M/P1 | P2 | new media module | sync test | (1) A/V drift ≤1 frame over 60 s; (2) Mute, Unmute and Toggle behave correctly | E11-5, X3 |
| E11-7 | Animated GIF: accept on placement, keep a poster frame plus an animation record, play in Present with the file's frame delays and loop count, .fig round-trip | M/P1 | P2 | interact.rs, prototype.rs, 🔥fig.rs | decode + round-trip | (1) frame timing matches the file within ±1 frame; (2) editor and exports show the poster; (3) the GIF rejection test is inverted | E11-1 |
| E11-8 | Present options: fit/fill/100%/fit-width, hotspot hints when a click hits nothing, rotation, a current device catalogue with bezels (es-designer) | M/P1 | P3 | present.rs, mod.rs | shell + screenshot | (1) each zoom mode matches a golden screenshot; (2) a miss-click flashes every reactive area for ≤500 ms | E11-3 |
| E11-9 | Comments on prototypes: comment mode in Present, pinned to frame/node/offset, same thread store as canvas comments with a "Prototype" filter | M/P1 | P3 | comments/*, comments.rs, present.rs | doc + shell | (1) a comment made in Present shows on the canvas on the same node; (2) resolving works from either side; (3) it survives save | — |
| E11-10 | AI "Add interactions" through the agent CLI: sends a selection spec and receives a structured "reactions patch"; diff preview; applied as one undo step; the rule-based auto-wire stays as the labelled offline fallback | M/P1 | P4 | cmdk.rs, terminal.rs, specs.rs | patch parsing with a recorded fixture | (1) the schema is validated and a bad destination is rejected with a named error; (2) one undo step; (3) no bundled model (cargo tree) | E11-11 |
| E11-11 | MCP tools: get/set reactions, flows and device | S/P1 | P4 | designos-mcp | MCP tests | (1) a write round-trips through the doc; (2) the tools are added to the T7.6 list | — |
| E11-12 | WASM build of `designos-render` plus `designos-proto` (rayon and system fonts behind features; fonts embedded from the doc) | L/P1 | P5 | render, fonts, proto Cargo | wasm test + pixel diff | (1) fixture frames match the native CPU render within the documented tolerance; (2) size within the budget set by D11.1 | E11-2 |
| E11-13 | "Export prototype as HTML" (local equivalent of embed and share links): offline player with flows, restart and device frame; CLI and MCP | L/P1 | P5 | new `designos-web-player` | scripted click path | (1) opens from file:// in Safari, Chrome and Firefox (screenshots); (2) the same clicks reach the same frames as Present; (3) zero network requests (CSP) | E11-12, D11.1 |
| E11-14 | Mobile view in the web player: tap/swipe/long-press, fit, safe areas, rotation, offline install (PWA manifest) | M/P1 | P5 | web-player | device run | (1) runs on a Z Fold 7 and iPhone Safari (screenshots); (2) double-tap does not zoom the browser | E11-13 |
| E11-15 | Video, GIF and audio in the web player (browser decoding, composited by layer order) | M/P2 | P5 | web-player | browser test | (1) the video draws in correct layer order under an overlay; (2) media triggers fire | E11-13, E11-5, E11-7 |
| E11-16 | Code/Make prototypes embedded and live in Present (webview) and in the HTML player (iframe); a postMessage bridge lets code trigger Navigate or Set variable | L/P2 | P6 | present.rs, web-player | shell + browser | (1) a button inside a code layer triggers Navigate; (2) variables sync both ways | E13-17, T8.7 |
| E11-17 | Import Make embeds (`makeContentState`, `codeSourceInfo`) as code layers, otherwise passthrough plus poster | M/P2 | P6 | 🔥fig.rs | round-trip | (1) nothing lost on round-trip; (2) mapped embeds render their poster | E11-1, E13-16 |
| E11-18 | Prototype conformance suite: scripted runs over REA-captured Figma fixtures | S/P1 | P3 | tests | CI | (1) frame sequences match the expected list | E11-4 |

### 4. Owner decisions
- **D11.1 How exported prototypes render.**
  - **(a) Recommended: a WASM build of our own renderer.** It is pixel-identical to Present, keeps smart animate and variables, and is the same code the phone preview uses. Cost: a file of a few MB (INFERENCE).
  - (b) Pre-rendered PNG frames plus hotspots. Small files, but no smart animate, variables or scrolling.
- **D11.2 Export packaging.**
  - **(a) Recommended: one self-contained `.html` file by default, with a folder option above about 100 MB.** One file to send or embed, and it works offline.
  - (b) Folder only.

### 5. Technical decisions and risks
- **Decisions:**
  - Prototype player and site export use different runtimes: a pixel-exact WASM player for prototypes, semantic HTML for sites (E13).
  - GIF reuses the `VideoFill` sidecar pattern, so renderers stay untouched (the existing decision at prototype.rs:213).
  - Present stays on CPU rendering; E11-5 only removes the PNG round-trip.
- **Risks:**
  - rayon inside WASM without threads (INFERENCE: put it behind a feature).
  - Web-player video layer ordering.
  - REA build 126.9.13 may predate some fields.

---

## E12 Canvas, accessibility & text

### 1. What exists today (FACT)
- **Keyboard:** Enter / Shift+Enter move into and out of the hierarchy, Tab cycles siblings (`shell.rs:710-745`).
- **Screen reader:** AccessKit is compiled in (`Cargo.lock` has `accesskit_winit`), but the app calls it zero times. INFERENCE: the canvas and custom-painted panels are invisible to screen readers.
- **Theme:** compile-time constants (`theme.rs:22-31`), referenced 2,895 times across 96 files. Dark theme only; no contrast mode.
- **Eyedropper:** canvas only. It renders one pixel of the page (`panels/properties.rs:1579-1587`).
- **Video on canvas:** shows the poster frame only, by design (`video.rs:1-7`).
- **Fonts:**
  - Installed system fonts load (`render/text.rs:73`), plus Fontsource download with a cache.
  - No way to import a font file.
  - SF Pro and the other Apple families are always swapped for Inter (`fonts/subst.rs:5-11`).
  - No missing-fonts UI.
- **Variable fonts:**
  - Axis values are stored per text style (`paint.rs:446`).
  - The UI offers only 4 fixed axes, with no ranges read from the font (`typography.rs:839-877`).
  - The renderer uses only wght, wdth (snapped to 9 stretch buckets), ital and slnt (`text.rs:405-423`). opsz and custom axes are ignored.
- **Already done in text:** OpenType features (`text.rs:436-444`), indent, hanging punctuation and lists, leading trim (`textmodel.rs:130-140`), small caps (`paint.rs:399`).
- **Decoration:** none, underline or strikethrough only (`paint.rs:403`).
- **Export:**
  - Custom suffix is done (`panels/export.rs:84-92`, :218).
  - Outline text exists in the export crate (`svg.rs:27`) but has no UI.
  - No colour profile, simplify stroke or include-id options.
  - PDF output is raster; vector PDF is issue #40.

### 2. Full scope
| Capability | State | Evidence |
|---|---|---|
| Keyboard hierarchy navigation | DONE | shell.rs:710 |
| Keyboard create/resize, focus regions, shortcut panel | MISSING | — |
| Screen reader: chrome | PARTIAL | AccessKit compiled in, not used |
| Screen reader: canvas layers | MISSING | — |
| Enhanced-contrast mode | MISSING | theme.rs constants |
| Eyedropper anywhere on screen | MISSING | properties.rs:1580 |
| On-canvas video playback, 300 MB uploads | MISSING | video.rs:1-7, kept in memory in `images` |
| Installed local fonts | DONE | text.rs:73 |
| Uploaded/imported fonts, shared font folder, embedding in documents | MISSING | — |
| Apple fonts | PARTIAL | subst.rs:5 |
| Variable font axes | PARTIAL | text.rs:405, typography.rs:839 |
| Missing-fonts panel | MISSING | — |
| Smart symbols | MISSING (behaviour UNKNOWN) | — |
| Decoration style, thickness, offset, colour | MISSING | paint.rs:403 |
| Export suffix | DONE | export.rs:84 |
| Outline text option in the UI | PARTIAL | svg.rs:27 |
| Colour profile, simplify stroke, include id, contents-only | MISSING | — |
| Bulk export dialog | MISSING (ASSUMPTION) | no matches |
| Tab groups, muting, agent window (catalog §1, an orphan) | MISSING | — |
| Layer-type labels (an orphan) | MISSING | — |

### 3. Tasks
| ID | Title | Size/Pri | Pack | Touches | Test seam | Acceptance criteria | Deps |
|---|---|---|---|---|---|---|---|
| E12-1 | **REA:** accessible-canvas key map and announcements; enhanced-contrast palette; smart symbols; ExportSettings fields (colour profile, outline text, id, simplify stroke, contents only); font-upload model; on-canvas video controls | S/P1 | A0 | — | findings doc | (1) every behaviour has evidence or is labelled UNKNOWN | — |
| E12-2 | Runtime theme palette: scripted switch from constants to lookups, plus a contrast mode in `design.tokens.json` | L/P1 | A1 | all UI (single PR in a quiet window) | token parity test + screenshot diff | (1) zero pixel diff on 5 surfaces in standard mode; (2) the theme.rs ⇄ tokens parity test passes | — |
| E12-3 | Enhanced-contrast mode: toggle, plus following the OS "Increase contrast" setting (es-designer, curator `ui ds a11y`) | M/P1 | A1 | theme.rs, prefs | a11y gate | (1) text ≥4.5:1 and UI ≥3:1 everywhere; (2) screenshots | E12-2, D12.1 |
| E12-4 | Accessibility info for every custom widget, via the shared widget helpers | L/P1 | A2 | widgets/* | AccessKit tree snapshot | (1) every control in the properties panel has a role and label; (2) VoiceOver pass (screen recording) | — |
| E12-5 | Canvas accessibility tree: visible layers plus the selection path, focus synced, actions announced | L/P1 | A2 | canvas/* | tree test + perf probe | (1) ≤1 ms per frame on the 723k-node page; (2) no `rev` bump and no `to_json` (§4) | E12-4 |
| E12-6 | Full keyboard canvas control per REA: focus regions (F6), keyboard create and resize, shortcut reference | M/P1 | A2 | shell.rs | shell tests | (1) a rectangle can be created and resized without a pointer; (2) focus cycles through 4 regions | E12-1 |
| E12-7 | Eyedropper anywhere on screen: NSColorSampler (macOS), desktop pixel read (Windows), xdg-desktop-portal PickColor (Linux); canvas sampler as fallback | M/P1 | A3 | properties.rs, new platform module | per-OS smoke | (1) picks a colour from another app (screenshot); (2) the value is stored in the document's P3 space; (3) Esc cancels | E1 P3 |
| E12-8 | On-canvas video playback: play controls on hover or selection; frames go into a dynamic GPU texture slot with no FrameKey or `rev` change | L/P1 | A3 | canvas/gpu.rs, designos-gpu | perf probe + headless suite | (1) 1080p plays on the 723k-node page; (2) idle refine is unaffected; (3) CPU and headless paths show the poster and stay bit-exact | E5-4a, E11-5 |
| E12-9 | Large media storage: stream video from file entries instead of memory; 300 MB limit | L/P1 | A3 | doc images, io.rs, 🔥fig.rs | RSS probe + round-trip | (1) opening a 300 MB video adds <50 MB RSS; (2) save/load round-trip; (3) `doc_hash` cost unchanged | D12.3 |
| E12-10 | Font import (TTF/OTF/WOFF/WOFF2) into a user font library, plus embedding in documents that respects the font's embedding permission (OS/2 fsType) | M/P1 | T1 | designos-fonts, fonts.rs, save format | font tests | (1) renders immediately and after restart; (2) a restricted font refuses embedding with a named reason; (3) an embedded font renders on a machine without it | D12.2 |
| E12-11 | Team font folder (synced-folder root, per the async-channel ruling) | S/P2 | T1 | designos-fonts | unit | (1) a font added to the folder appears without restart | E12-10 |
| E12-12 | Apple fonts on macOS: resolve SF Pro/Compact/Mono/New York to the system files, with optical sizes; other OSes keep the substitution plus a missing indicator | M/P1 | T1 | subst.rs, text.rs | glyph comparison | (1) macOS renders the system SF; (2) the Linux CI result is unchanged | E12-14 |
| E12-13 | Full variable-font support: read the font's axes and named instances, a slider for every axis, all axes applied in shaping and outlines, CSS `font-variation-settings`, .fig `fontVariations` | L/P1 | T2 | typography.rs, text.rs, css.rs, 🔥fig.rs | pixel diff + round-trip | (1) changing opsz or a custom axis changes the outlines; (2) CPU and GPU stay bit-exact; (3) .fig round-trip | — |
| E12-14 | Missing-fonts panel: list, replace across the document, badge on affected layers | M/P1 | T1 | panels, fonts.rs | shell | (1) one replace is one undo step across all runs | — |
| E12-15 | Smart symbols, per REA | S/P2 | T2 | textedit.rs | unit | (1) matches REA cases | E12-1 |
| E12-16 | Decoration style, thickness, offset and colour (CPU, GPU, SVG/CSS, .fig) | M/P2 | T2 | paint.rs, render, export, 🔥fig.rs | diff + round-trip | (1) wavy underline is identical on CPU and GPU; (2) round-trip | — |
| E12-17 | Text parity audit against catalog §4 and REA: font picker previews, forced small caps, sub/superscript | S/P2 | T2 | — | audit report | (1) every finding becomes a filed task | E12-1 |
| E12-18 | Export settings popover: outline text, include id, simplify stroke, colour profile, JPG quality, contents only; .fig ExportSettings | M/P1 | X1 | export.rs, settings.rs, 🔥fig.rs | round-trip + snapshot | (1) .fig export settings survive; (2) every toggle changes the output (snapshot) | E12-1 |
| E12-19 | Embed ICC profiles (sRGB / Display P3) in PNG and JPG, converting colours when needed | M/P1 | X1 | raster.rs | ColorSync check | (1) macOS Preview shows Display P3 on a P3 export (screenshot); (2) sRGB exports stay bit-exact | E1 P3 |
| E12-20 | Simplify stroke and include id in SVG, per REA semantics | M/P2 | X1 | svg.rs | golden SVG | (1) matches the REA sample | E12-1 |
| E12-21 | Bulk export dialog (Cmd+Shift+E) | M/P2 | X1 | panels | shell + screenshot | (1) lists every exportable layer; exports the chosen subset | — |
| E12-22 | Tab groups, tab muting, agent terminal in its own window (egui multi-viewport) | M/P3 | X2 | tabs.rs, terminal.rs | shell | (1) a group survives restart; (2) the agent window can be detached and reattached | — |
| E12-23 | Layer-type labels in the Layers panel | S/P3 | X2 | layers.rs | screenshot | (1) the label shows for each node type | — |

### 4. Owner decisions
- **D12.1 How contrast mode turns on.**
  - **(a) Recommended: a manual toggle plus following the OS "Increase contrast" setting.** Users who already set it in the OS get it with no action.
  - (b) Manual toggle only, as in Figma.
- **D12.2 Embedding fonts in documents.**
  - **(a) Recommended: embed when the font's licence flags allow it.** Files stay portable offline; this is the local stand-in for Figma's shared fonts.
  - (b) Never embed; fonts only come from the user's library.
- **D12.3 Where large videos are stored.**
  - **(a) Recommended: inside the file, streamed rather than held in memory.** Nothing goes missing when a file moves.
  - (b) Linked external file. Smaller documents, but the link breaks on move.

### 5. Technical decisions and risks
- **Decisions:**
  - The `record()` widget helper is the single place to attach accessibility info (INFERENCE).
  - Video frames go into an indirect GPU texture slot rather than triggering a recompile, which honours §4.1 and §4.4.
  - The theme refactor is one scripted PR.
- **Risks:**
  - E12-2 touches 96 files and will conflict with every UI branch.
  - cosmic-text may not support arbitrary variation axes, which would force a fallback to ttf-parser outlines (ASSUMPTION).
  - WOFF2 decoding needs brotli; its licence goes through the X3 gate.

---

## E13 Sites & code layers

### 1. What exists today (FACT)
- No Sites code at all (inventory).
- **Foundations to reuse:**
  - Whole-tree React, CSS and Tailwind codegen (`export/src/react.rs:24`, `css.rs:9`, `tailwind.rs:12`).
  - Variables to CSS custom properties (`codegen.rs:158`).
  - Auto-layout and constraint models (`node.rs:628`, :672).
  - The existing loopback server pattern (`plugins/runtime.rs:34-75`).
  - `codeSourceInfo` and `makeContentState` are kept as opaque data (`fig.rs:731`, :739).
- **Planned elsewhere:** HTML codegen is T7.29; the webview host is T8.7. There is no JS engine or webview today.

### 2. Full scope (all MISSING unless noted)
1. Site file type and site mode.
2. Pages and page settings: slug, SEO, social image, favicon, noindex.
3. Breakpoints: a primary frame with overrides per breakpoint, edits flowing down, a resize preview.
4. Responsive compile to HTML/CSS. PARTIAL foundation in the codegen above.
5. Blocks and the 50 templates (ruling #20, with the licence gate).
6. Interactions: hover/press states, links, page transitions, sticky.
7. Scroll effects: appear on scroll, parallax, marquee.
8. Video backgrounds.
9. CMS: collections, CSV, list and detail pages.
10. Local preview.
11. Local "publish" as a static folder: sitemap, robots, 404, optimised assets. Custom domains, password protection and analytics are hosted features and are EXCLUDED by ruling 14.
12. Accessible HTML output.
13. .fig import of Sites files.
14. Code layers: from a frame, a repo or a prompt; poster on the canvas; live in Present and in sites; code editor; props; sandbox.
15. MCP tools and agent generation (through the agent CLI).

### 3. Tasks
| ID | Title | Size/Pri | Pack | Touches | Test seam | Acceptance criteria | Deps |
|---|---|---|---|---|---|---|---|
| E13-1 | **REA:** Sites in 126.9.13: editor type, responsive sets/breakpoints and their overrides, page settings, scroll-effect fields, CMS, `codeSourceInfo`, `makeContentState`. Code layers shipped 2026-10-15, after this build: design our own, then align (same pattern as ruling #12) | M/P1 | S0 | — | findings doc | (1) every field has an id or is labelled UNKNOWN | — |
| E13-2 | Site document type, Home "New site", site toolbar with pages and breakpoint bar (es-designer) | L/P1 | S1 | home/, shell.rs, doc | shell + screenshot | (1) a site is created, saved and reopened | — |
| E13-3 | Breakpoint model: overrides stored per breakpoint on top of the primary, edits flowing down | XL/P1 | S1 | node.rs, 🔥fig.rs | doc tests | (1) a primary edit reaches breakpoints that have no override; (2) an override sticks; (3) one undo step | E13-1 |
| E13-4 | Breakpoint canvas UX: frames side by side, drag-width reflow preview, override markers, reset | L/P1 | S1 | canvas/* | perf probe + screenshot | (1) the reflow drag stays within the 8 ms budget; (2) §4 perf invariants hold | E13-3 |
| E13-5 | Page settings panel | M/P1 | S1 | panels | round-trip | (1) every field appears in the exported `<head>` | E13-2 |
| E13-6 | Site compiler: semantic HTML, flex/grid/constraints, breakpoints become media queries, variables become CSS properties, styles become classes, effects, inline SVG | XL/P1 | S2 | export `site/` | browser pixel diff + html-validate | (1) 375/768/1440 renders match the Studio render within threshold (screenshots); (2) html-validate passes; (3) static layout works with no JS | T7.29, E13-3 |
| E13-7 | Asset pipeline: WebP and srcset, WOFF2 fonts self-hosted when the licence allows, SVG optimisation | M/P1 | S2 | export | unit | (1) a restricted font falls back to a CSS font stack with a warning | E12-10, E6-11 |
| E13-8 | Export site (menu, CLI, MCP): sitemap, robots, 404, relative links, OG tags | M/P1 | S2 | io.rs, CLI, MCP | hash test | (1) re-exporting gives byte-identical output; (2) works from file:// | E13-6 |
| E13-9 | Local preview: loopback server on an OS-assigned port with a token, live reload off the UI thread | M/P1 | S2 | plugins runtime pattern | integration | (1) an edit shows in the browser in ≤1 s; (2) the server stops with the window | E13-8 |
| E13-10 | Site interactions: variant states become `:hover`/`:active`, links, View Transitions, sticky | L/P1 | S3 | site/ | browser test | (1) hover state matches the variant (screenshot) | E13-6, E11-3 |
| E13-11 | Scroll effects (appear, parallax, marquee, scroll-linked), honouring `prefers-reduced-motion`, with an editor preview | L/P2 | S3 | doc, site/ | browser test | (1) reduced-motion turns effects off | E13-1 |
| E13-12 | Video backgrounds in export (poster, muted autoplay loop) | M/P2 | S3 | site/ | browser test | (1) plays in Safari and Chrome | E5-4a |
| E13-13 | Block library, the 50 templates, "Save as block/template" | L/P2 | S4 | assets, home | screenshot | (1) passes the licence gate (ruling #20) | licence gate |
| E13-14 | CMS model: collections, typed fields, items, CSV in/out | L/P2 | S5 | doc | round-trip | (1) a CSV of 1k rows round-trips | — |
| E13-15 | CMS binding: list layers, detail templates, slug pages | XL/P2 | S5 | doc, site/ | export test | (1) one page per item; (2) the editor previews real items | E13-14, E13-6 |
| E13-16 | Code layer model (source files, entry, props schema, poster, sizing), .designos and .fig (`codeSourceInfo`) | L/P1 | S6 | node.rs, 🔥fig.rs | round-trip | (1) lossless round-trip; (2) old files load | E13-1 |
| E13-17 | Code runtime: offscreen webview snapshot becomes the canvas poster (async, cached, not a doc mutation); live in Present | XL/P1 | S6 | canvas, present | perf probe | (1) the poster arrives without a `rev` bump; (2) §4 invariants hold | T8.7, E13-16 |
| E13-18 | Offline TSX→JS transpile in-process (swc/oxc) plus React ESM shipped with the app (no npm) | L/P1 | S6 | new crate | unit | (1) the fixture component runs with no network; (2) passes the X3 licence gate | X3 |
| E13-19 | Code editor panel with runtime errors, and a props inspector generated from the props schema with variable binding (es-designer) | L/P1 | S6 | panels | shell + screenshot | (1) a prop edit re-renders the poster | E13-17 |
| E13-20 | Create a code layer from a frame (react.rs), a repo file (repo wins, same as D7.2) or a prompt (agent CLI through MCP) | L/P1 | S6 | cmdk, MCP | integration | (1) a repo edit syncs into the doc; (2) "Edit in code" is shown | E13-19, T7 Code Connect |
| E13-21 | Sandbox: CSP, no network except allowedDomains with consent (ruling #17), whitelisted bridge messages | M/P1 | S6 | runtime | security test | (1) fetch to a domain not on the list is blocked | T8.15 |
| E13-22 | Code layers in exports: real components on sites, imports in React codegen, poster (labelled) in PNG/SVG/PDF | M/P1 | S6 | export | browser + golden | (1) a live component runs on the exported site | E13-6, E13-17 |
| E13-23 | MCP tools: sites, pages, breakpoints, CMS CRUD, code layers, export | M/P1 | S6 | designos-mcp | MCP tests | (1) add to the T7.6 list | — |
| E13-24 | Agent "generate section/site" through the agent CLI and MCP | M/P2 | S6 | cmdk | fixture | (1) no bundled model | E13-23 |

### 4. Owner decisions
- **D13.1 Where sites live.**
  - **(a) Recommended: their own file type in Home.** Matches Figma, and keeps CMS and pages out of design files.
  - (b) A mode inside any design file.
- **D13.2 Code layer languages.**
  - **(a) Recommended: React/TSX only.** Figma parity, and one runtime to sandbox.
  - (b) React plus plain HTML/JS.
  - (c) Option (b) plus Vue and Svelte.
- **D13.3 JavaScript in exported sites.**
  - **(a) Recommended: zero JS by default; small scripts only where an interaction needs them.** Fast, works without JS, good for SEO.
  - (b) Always ship one runtime bundle.

### 5. Technical decisions and risks
- **Decisions:**
  - The preview server binds 127.0.0.1 with an OS-assigned port and a token. This is a product feature at runtime, not a project dev server, so the fixed-port contract doesn't apply (INFERENCE; the controller should confirm).
  - CMS content is baked in at export time (hosted services are excluded).
- **Risks:**
  - The REA build predates code layers.
  - The breakpoint override model (XL) touches the instance-override code paths.
  - Webview snapshots on Linux depend on WebKitGTK.

---

## E14 iPad / mobile app

### 1. What exists today (FACT)
- **No iOS target.** `packaging/` contains linux, macos and windows only.
- **Desktop-only dependencies in `designos-app/Cargo.toml`:**
  - `portable-pty` (:51). iOS cannot spawn the agent CLI processes, so the agent terminal can't run there.
  - `rfd`, `arboard`. INFERENCE: neither has an iOS backend.
- **Core crates look portable** (INFERENCE from their dependencies): doc, render, gpu (wgpu 29), fig and export.
- **UI sizing:** the UI uses 24 px targets (principles). Touch needs 44 pt.

### 2. Full scope (all MISSING)
- iPad editor with full feature parity.
- Touch, Apple Pencil (pressure, tilt, hover, squeeze), hardware keyboard, trackpad.
- IME (Vietnamese, CJK).
- Files / iCloud integration.
- App lifecycle and memory handling.
- Touch-adapted layout.
- AI features (with the agent constraint above).
- Plugins (App Store rule 2.5.2).
- Prototype preview on phones: LAN web player, native player, mirror.
- Distribution.

### Approaches (feasibility)
| | Code reuse | Native feel | Cost | Verdict |
|---|---|---|---|---|
| **C. Thin Swift UIKit host + egui-wgpu on a CAMetalLayer** (no eframe) | High: one UI codebase | Pencil, IME and Files done natively | L–XL | **Recommended; the E14-1 spike decides** |
| A. eframe on iOS through winit | Highest | Weak: IME, no Pencil hover, no file picker | M | Fallback if C is blocked |
| B. SwiftUI shell + UniFFI core | Canvas only | Best | 2× UI work, permanent drift | Rejected (DRY) |
| D. WASM editor in Safari as a PWA | High | Weak: memory caps, file access | L | Fails the 723k-node page (INFERENCE) |

### 3. Tasks
| ID | Title | Size/Pri | Pack | Touches | Test seam | Acceptance criteria | Deps |
|---|---|---|---|---|---|---|---|
| E14-1 | Spike C vs A on a real iPad: 723k-node fixture fps/RSS, Pencil, IME, background/foreground | M/P1 | I0 | spike branch | report + recording | (1) go/no-go table with numbers; (2) a ceiling on document size that iOS memory limits allow | — |
| E14-2 | Platform trait: file dialogs, clipboard, PTY, updater, open-URL, fonts, notifications | L/P1 | I0 | io.rs, clipboard.rs, terminal.rs, updater.rs | CI target check | (1) `cargo check --target aarch64-apple-ios` passes; (2) desktop behaviour unchanged | E14-1 |
| E14-3 | Fonts and caches on iOS (bundled fonts, system fonts, app container) | S/P1 | I0 | designos-fonts | device test | (1) SF and bundled fonts render | E14-2 |
| E14-4 | Xcode host, Rust xcframework build, signing, CI `.ipa` | M/P1 | I1 | packaging/ios | CI | (1) a signed build installs on the device | E14-2 |
| E14-5 | Input bridge: gestures, Pencil (pressure, tilt, azimuth, hover, squeeze), UIKeyCommand shortcuts, pointer | L/P1 | I1 | host + shell input | device test | (1) Pencil pressure reaches the E4 brush; (2) pinch zoom at 120 fps with no `rev` bump | E14-4 |
| E14-6 | UITextInput IME bridge into canvas text editing | L/P1 | I1 | host, textsession.rs | device test | (1) Vietnamese Telex and Japanese compose correctly | E14-4 |
| E14-7 | Files/iCloud document browser, autosave on background, share sheet | M/P1 | I1 | host, io | device test | (1) no data loss when the app is killed in the background | E14-4 |
| E14-8 | Lifecycle and memory: surface rebuild, dropping caches on memory warnings, a VRAM refine budget per device (§4) | M/P1 | I1 | canvas/gpu.rs | device probe | (1) survives 10 background/foreground cycles; (2) the budget applies | E14-4 |
| E14-9 | Touch layout profile with 44 pt targets and a "touch" token mode (es-designer, curator) | XL/P1 | I2 | theme, panels | rubric + screenshots | (1) passes the curator rubric at 11"/13", portrait and landscape | E12-2, E14-5 |
| E14-10 | Parity pass: every desktop surface reachable, checklist generated from the `menus.rs` action list | L/P1 | I2 | panels | checklist | (1) 100% of actions reachable | E14-9 |
| E14-11 | AI on iPad per D14.2: relay to the paired desktop's agent runtime (pairing, encryption, token) | L/P2 | I2 | host, terminal | integration | (1) "Add interactions" works through the relay; disabled with a named reason when there's no relay | D14.2, E11-10 |
| E14-12 | "Preview on device" from desktop: LAN web player, QR code with a one-time token, live update, follow-selection mirror, auto-stop | M/P1 | I3 | web-player, server | device test | (1) the Z Fold 7 scans the QR and the prototype runs; (2) the mirror follows within 500 ms; (3) stopping revokes the token | E11-13, E11-14 |
| E14-13 | Native prototype player on iPhone and iPad (designos-proto + GPU, AVFoundation video, prototype comments) | L/P1 | I3 | host | device test | (1) matches the desktop Present click path | E14-4, E11-2, E11-9, E5-4a |
| E14-14 | Bonjour discovery and native mirror | M/P2 | I3 | host | device test | (1) finds the desktop with no manual IP | E14-12 |
| E14-15 | Distribution: App Store/TestFlight pipeline, privacy manifest, export compliance | M/P1 | I4 | packaging | review checklist | (1) passes TestFlight processing | D14.1 |
| E14-16 | Plugins on iPad through WKWebView/JavaScriptCore instead of QuickJS (rule 2.5.2) | M/P2 | I4 | plugin host | device test | (1) a plugin from the corpus runs | T8.5, D14.1 |

### 4. Owner decisions
- **D14.1 Distribution.**
  - **(a) Recommended: the App Store.** It's the only normal way users install iPad apps. Costs $99/yr and review rules apply (plugins must run in WebKit).
  - (b) TestFlight only.
  - (c) EU sideload only.
- **D14.2 AI on iPad.** iPadOS can't run the agent CLI.
  - **(a) Recommended: relay to the user's paired desktop agent runtime.** This keeps the "agent CLI, no bundled model" ruling.
  - (b) Disable AI on iPad.
- **D14.3 Native platforms.**
  - **(a) Recommended: iPadOS plus iPhone as one universal app.** Android phones get the LAN web player (E14-12) at no extra cost.
  - (b) Add a native Android app now.

### 5. Technical decisions and risks
- **Decisions:**
  - Approach C, pending the spike.
  - The same `designos-proto` crate drives Present, the web player and the iOS player.
- **Risks:**
  - iOS memory limits on very large documents.
  - egui IME on iOS.
  - The LAN server's attack surface (mitigated by a token, user-started only, auto-stop).
  - Apple Developer account costs and review delays.

---

**Assumptions** (confidence; what would change them):
- Smart symbols means auto-substituting typed sequences (low). E12-1 REA settles it.
- eframe and egui-wgpu run on iOS (low/medium). E14-1 settles it.
- `rayon` can be put behind a feature for WASM (medium). An E11-12 build settles it.
- Treating the preview server as a runtime feature exempt from the port contract (medium). The controller's ruling settles it.

**Weakest link:** the E14 feasibility and the E11-12 WASM build are inferred from dependency lists only; no cargo build ran.

**Open questions for the controller:**
- Split E5-4 into 4a/4b?
- Ask D13.1 as a confirm-only question, given the "follow Figma" rule was not found in the decisions ledger?

Status: DONE_WITH_CONCERNS
Summary: Full-scope drafts for E11 (18 tasks), E12 (23), E13 (24) and E14 (16), with code evidence, packs, dependencies and 11 owner decisions. A shared egui-free `designos-proto` crate plus a WASM player connects E11 embedding, mobile view and E14 preview.
Concerns/Blockers: Cargo was not run, so E14 feasibility and the WASM build are unproven until the E11-12 build and the E14-1 spike. E5-4 overlaps E11-5 and E12-8 until it is split. The REA build 126.9.13 predates code layers.