#!/usr/bin/env python3
"""Build master-tasks.json for design:os studio epics E1-E14 (owner rulings applied, rounds 1-10)."""
import json, collections, re, os

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "master-tasks.json")
EPIC_NO = {"E1": 14, "E2": 15, "E3": 16, "E4": 17, "E5": 18, "E6": 19, "E7": 20, "E8": 21, "E9": 22, "E10": 23}
# E11-E14 have no epic issue yet; the filer creates them from out["new_epics"].
SRC = {
    "A": "plans/261017-epics-full-scope/full-scope-audit-e1-e2-e3-e5.md",
    "B": "plans/261017-epics-full-scope/full-scope-e4-e6-e9.md",
    "C": "plans/261017-epics-full-scope/full-scope-e7-e8-e10.md",
    "D": "plans/261017-epics-full-scope/owner-decisions-and-cross-check.md",
    "E": "plans/261017-epics-full-scope/full-scope-e11-e12-e13-e14.md",
}

# ---------------------------------------------------------------- packs
PACKS = [
    ("v0.4 Layout 2.0", "Headline: Grid 2.0 and auto layout v1 (existing milestone). Adds grid track editing, cell alignment, layout guide parity, real-track grid codegen, presets. Infra: X1 typed kiwi passthrough (first in the hot-file queue). #106 is eligible to ship earlier as a v0.4.x hotfix."),
    ("v0.5 Components", "Headline: Slots (existing milestone). Adds slot canvas UX, migrate-to-slots, slots in codegen, Check designs numbers/libraries/scale/rule parity, component playground. Infra: E3-15 verify the owner's VSF - PCP.fig slot data."),
    ("v0.6 Effects & Export", "Headline: progressive blur and glass/noise/texture parity incl. .fig round trip (existing milestone). Adds shadow blend modes, effect count limits, image fill adjustments/crop. Vector PDF (#40) stays here."),
    ("v0.7 Agent design context", "Headline: agents get Figma-MCP-grade design context (screenshot, metadata, get_design_context, selection default configurable per ruling R1.4). Follow-ups: parity matrix vs Figma's 41 tools, whoami/create_new_file/get_libraries/create_design_system_rules, Vue/HTML/Web Components/Flutter codegen. Infra: X3 licence gate, plugin corpus approval, non-shipping plugin runtime spike T8.3. Needs live-sync issues #75-#81 settled."),
    ("v0.8 Variables everywhere", "Headline: variables inside styles and bindable on every Figma-bindable property. Follow-ups: delete/order/describe styles, extended collections, Display P3 + OKLCH, Figma JSON + DTCG tokens, Figma paste merges styles/variables (T8.4)."),
    ("v0.9 Present like Keynote", "Headline: 26 slide transitions in Present. Follow-ups: typed slide fields, skipped slides, slide numbers, presenter view, spotlight as presenter-follow (solo), PPTX import and export."),
    ("v0.10 Pro strokes", "Headline: variable-width strokes. Follow-ups: typed Draw fields round trip, path trim, Draw toolbar mode, multi-point bounding box, outlined Draw geometry in SVG/PDF/codegen."),
    ("v0.11 Animate a frame", "Headline: Motion timeline + keyframes + playback. Follow-ups: easing editor, export settings, PNG sequence, GIF, static WebP. Infra: X2 easing moved into designos-doc."),
    ("v0.12 Run Figma plugins", "Headline: run real open-source Figma plugins imported from a manifest (QuickJS host, embedded webview UI). Follow-ups: core nodes/events/fonts/images/export API, compatibility harness gate, MCP use_figma parity (designos_run_plugin_script)."),
    ("v0.13 Branches", "Headline: branch, review and merge inside a file (per-layer conflicts, archived merged branches). Follow-ups: update from main, compare versions, local review state, comment merge, multi-author identity."),
    ("v0.14 Build the story", "Headline: object animations on slides. Follow-ups: slide sections, interactive prototype slides, templates (copied Figma templates per R5.20), slide themes. Infra: X4 licence-review gate for copied Figma assets."),
    ("v0.15 Repeat & pattern", "Headline: transform groups (repeat linear/radial). Follow-ups: symmetry/skew, transform-group UI, pattern fills."),
    ("v0.16 Code Connect", "Headline: real Code Connect (model, Inspect snippets, CLI where repo files win per R6.21). Follow-ups: prop helpers, MCP Code Connect tools incl. get_context_for_code_connect, keep Figma GUIDs, connect-component UI."),
    ("v0.17 Motion toolkit", "Headline: presets and animation styles. Follow-ups: anchor point, motion path, path-trim/effect/text tracks, animation styles model + libraries, easing/timing variables, MCP get_motion_context."),
    ("v0.18 Full Plugin API", "Headline: 100% of @figma/plugin-typings minus hosted exclusions. Follow-ups: styles/variables/components, teamLibrary, clientStorage/parameters, network consent, FigJam/Slides editor types, Dev Mode codegen plugins, local stubs for hosted APIs."),
    ("v0.19 Desktop MCP", "Headline: Streamable HTTP MCP on the project's fixed loopback port. Follow-ups: download_assets, upload_assets, search_design_system, set_codegen_config, get_figjam."),
    ("v0.20 FigJam tables", "Headline: tables with merged cells on boards (model, render, .fig, editing UI)."),
    ("v0.21 Edit vectors", "Headline: eraser. Follow-ups: cut tool, offset path, multi-stroke with OFFSET align, stroke sampling."),
    ("v0.22 Motion for the web", "Headline: Lottie export. Follow-ups: dotLottie, animated SVG, CSS keyframes, JSON motion, React (Figma flavour per R6.23)."),
    ("v0.23 Mermaid diagrams", "Headline: paste Mermaid into boards as native layers. Follow-ups: every Figma-supported Mermaid type, ERD, generate_diagram MCP tool."),
    ("v0.24 Custom shaders", "Headline: user-authored WGSL shaders (GPU, cached readback on CPU/headless per R2.8). Follow-ups: the 5 Figma shader MCP tools."),
    ("v0.25 3D transforms", "Headline: our own 3D layer transforms, informed by an REA study of Spline, Rive and the Figma 126.9.13 schema (R3.12). Not waiting for Figma GA."),
    ("v0.26 Sketch import", "Headline: open .sketch files (layers, text, images, symbols to components, shared styles). Follow-up: zip-entry file probe."),
    ("v0.27 Annotate & inspect", "Headline: Figma annotations (33 property types, categories, measurements) with .fig round trip. Follow-ups: status activity log, focus view, local status inbox, specs authoring."),
    ("v0.28 Brushes", "Headline: stretch and scatter brushes with Figma's 25 built-in brushes (R5.20, behind X4). Follow-ups: dynamic stroke, custom brushes, text on a path."),
    ("v0.29 Video & sound export", "Headline: MP4 and WebM export via OS codecs (R3.11). Follow-ups: export progress UX, audio on the timeline, audio muxing, shared OS video decode (E5-4a), video fills (E5-4b), Present video pipeline without the PNG step, shared audio output service (E11-6, also used by timeline audio)."),
    ("v0.30 Web capture", "Headline: capture a live web page into editable frames (R6.22: Studio importer allowed). Follow-ups: Chrome extension, responsive capture, MCP code-to-canvas."),
    ("v0.31 Think together", "Headline: mind maps. Follow-ups: quick create, facilitator timer, local vote tallies (solo variant per R4.15)."),
    ("v0.32 AI tools through your agent CLI", "Headline: AI image tools routed through the user's agent CLI, no bundled model (R4.13). Follow-ups: vectorize via agent CLI, agent panel + summaries in board and deck modes."),
    ("v0.33 Work as a team without a server", "Headline: git merge driver for .designos files. Follow-ups: synced-folder library root, @mentions/reactions, nested folders + preview, tab groups, VS Code extension, Jira/Google Chat/Slack opt-in posts, local status hook."),
    ("v0.34 Widgets & agent-built plugins", "Headline: FigJam widgets run locally (also closes E9 widget scope). Follow-ups: widget synced state + .fig round trip, generative plugins scaffolded by the agent."),
    ("v0.35 Bring boards in", "Headline: Lucid and Mural import (no Jamboard, R6.24). Follow-ups: .jam/.deck import, copy Studio to Figma clipboard, paste from Sketch/Illustrator."),
    ("v0.36 Prototype 2.0", "Headline: sticky scroll, media triggers and Navigate resets in prototypes, on a new egui-free designos-proto crate. Follow-ups: animated GIF in prototypes, prototype conformance suite over REA-captured Figma fixtures."),
    ("v0.37 Present prototypes", "Headline: Present zoom modes (fit/fill/100%/width), hotspot hints, rotation and a current device catalogue. Follow-ups: comments on prototypes, AI 'Add interactions' through the agent CLI (R4.13), MCP prototype tools."),
    ("v0.38 Share prototypes as one file", "Headline: export a prototype as one self-contained offline .html (R8.28; folder option above ~100 MB), rendered by our own WASM build (R8.27). Follow-ups: mobile view, video/GIF/audio in the web player."),
    ("v0.39 Accessible Studio", "Headline: enhanced-contrast mode with a manual toggle that also follows the OS setting (R8.29), on a runtime theme palette. Follow-ups: screen reader info on every widget, canvas accessibility tree, full keyboard canvas control."),
    ("v0.40 Video on the canvas", "Headline: videos play on the canvas, stored inside the file and streamed, not held in RAM (R9.31). Follow-up: eyedropper anywhere on screen."),
    ("v0.41 Fonts & type", "Headline: import fonts and embed them in files when the font licence allows (R8.30). Follow-ups: team font folder, Apple system fonts, missing-fonts panel, every variable-font axis, smart symbols, decoration style, text parity audit."),
    ("v0.42 Export settings", "Headline: export settings popover (outline text, include id, simplify stroke, colour profile, JPG quality, contents only) with .fig round trip. Follow-ups: ICC profiles in PNG/JPG, SVG simplify/id, bulk export dialog, tab muting + detached agent window, layer-type labels. Vector PDF stays #40."),
    ("v0.43 Sites", "Headline: sites as their own file type in Home (R9.32) with pages and breakpoints (overrides flow down from the primary frame). Follow-ups: breakpoint canvas UX, page settings."),
    ("v0.44 Publish a static site", "Headline: compile a site to semantic HTML/CSS with zero JS by default (R9.34) and export it as a static folder. Follow-ups: asset pipeline, local preview server (OS-assigned port + token, runtime feature)."),
    ("v0.45 Sites in motion", "Headline: site interactions (hover/press states, links, View Transitions, sticky). Follow-ups: scroll effects honouring reduced motion, video backgrounds."),
    ("v0.46 Site content", "Headline: CMS collections with list and detail pages, baked in at export. Follow-ups: CSV in/out, block library and the 50 templates (behind X4)."),
    ("v0.47 Code layers", "Headline: React/TSX code layers (R9.33) with a canvas poster, live in Present and in exported sites, sandboxed. Follow-ups: offline TSX transpile, code editor + props inspector, create from frame/repo/prompt, MCP tools, agent site generation, live code in prototypes, Make embed import."),
    ("Unscheduled: external trigger", "Not a release pack. Tasks waiting on an external event (Figma 3D GA). Move into the next open pack when the trigger fires."),
    ("Later", "Backlog, not a release pack. E14 iPad / mobile app, every task Status Backlog (R10.35). Owner decisions D14.1 distribution, D14.2 AI on iPad and D14.3 native platforms stay open on the E14 epic issue."),
]
P = {p[0].split(" ")[0]: p[0] for p in PACKS}
P["U"] = "Unscheduled: external trigger"
assert P["U"] in [p[0] for p in PACKS]

# ---------------------------------------------------------------- hot-file queue (merge order; pack order per docs/releasing.md "Merging")
HOT = [
    "X1", "#106", "#24", "#25", "#26", "#29", "#71", "E2-2", "E2-4", "E2-5",      # v0.4
    "#31", "#107", "#108", "#109", "#110", "#111",                                  # v0.5 (PR #49 fixes first)
    "#38", "#37", "#39", "E5-1", "E5-3",                                            # v0.6
    "E1-4", "E1-5a", "E1-5b", "E1-5c", "E1-2", "E1-6", "E1-7", "T8.4",              # v0.8
    "E9-1",                                                                          # v0.9
    "E4-1",                                                                          # v0.10
    "E6-4", "E6-8",                                                                  # v0.11
    "T7.19",                                                                         # v0.16
    "E1-10", "E6-19",                                                                # v0.17
    "E9-12",                                                                         # v0.20
    "E4-16",                                                                         # v0.21
    "E5-8",                                                                          # v0.25
    "T7.23",                                                                         # v0.27
    "E5-4b",                                                                         # v0.29 (E5-4a decode is not hot)
    "E9-18",                                                                         # v0.31
    "T8.20c",                                                                        # v0.34
    "T8.29", "T8.30",                                                                # v0.35
    "E11-3", "E11-7",                                                                # v0.36
    "E12-9",                                                                         # v0.40
    "E12-13", "E12-16",                                                              # v0.41
    "E12-18",                                                                        # v0.42
    "E13-3",                                                                         # v0.43
    "E13-16", "E11-17",                                                              # v0.47 (E11-17 needs E13-16)
]
HOTPOS = {h: i + 1 for i, h in enumerate(HOT)}

TASKS = []


def t(id, epic, pack, title, size, prio, touches, seam, ac, deps=(), ctx="", rul="", rea="", src="A", ui=False, status=""):
    TASKS.append(dict(id=id, epic=epic, pack=P[pack], title=title, size=size, prio=prio, touches=touches,
                      seam=seam, ac=ac, deps=list(deps), ctx=ctx, rul=rul, rea=rea, src=src, ui=ui, status=status))


# ================================================================ E1 Styles & Variables
t("E1-1", "E1", "v0.8", "Delete a style and detach its linked layers, keeping their values", "S", "P1",
  "designos-doc/src/styles_ops.rs, designos-app style_picker.rs", "designos-doc unit test",
  "Linked layers keep their values and lose the style ref in one undo step|A library style refuses deletion with the A13 wording|.fig export writes no orphan style node",
  ctx="No delete op exists in styles_ops.rs (only history.rs:39 mentions delete). Figma keeps values and detaches [A, REA confirm].")
t("E1-2", "E1", "v0.8", "Order styles and group them into folders with sortPosition round trip", "M", "P1",
  "designos-doc lib.rs StyleMeta, style_picker.rs, fig.rs, fig_export.rs", "doc test + .fig round trip",
  "Order survives native save and .fig (sortPosition=320)|Renaming a folder renames every member in one undo step|Old files load",
  ctx="StyleMeta holds name/key/description/property only (lib.rs:742-750).", rea="E1-a: sortPosition and folder semantics.")
t("E1-3", "E1", "v0.8", "Edit style descriptions and show them on hover in the style picker", "S", "P2",
  "style_edit.rs, style_picker.rs", "shell test + 1440 screenshot",
  "Edit, save, then the description shows on hover|Disabled for library styles (A13)", ui=True,
  ctx="Field exists (lib.rs:747); no UI in style_edit.rs.")
t("E1-4", "E1", "v0.8", "Let colour, effect and text styles hold variable aliases", "L", "P1",
  "designos-doc lib.rs Style, styles.rs, fig.rs:927,957, fig_export.rs, variables_apply.rs", "doc mode-switch test + .fig round trip",
  "A mode switch recolours layers linked to a variable-bound colour style|.fig style node colorVar round-trips|Check designs counts the layer as bound",
  ctx="Style::Fill(Vec<Paint>) and Paint carry no binding (lib.rs:754, rest_import.rs:97-101).",
  rea="E1-b: colorVar on style nodes and text-style variable fields.")
t("E1-5a", "E1", "v0.8", "Bind variables to gradient stops, effect parameters and glass parameters", "M", "P1",
  "variables_apply.rs, fig.rs:1168-1260, fig_export.rs:721,1362, variable bind picker", "doc + .fig round trip",
  "Each key resolves per mode|Scopes EFFECT_FLOAT/EFFECT_COLOR are enforced in the picker|Round trip keeps every binding",
  deps=["#37", "#38"], ctx="property_kind covers only the keys at variables_apply.rs:21-31; effects bind colour + opacity only.")
t("E1-5b", "E1", "v0.8", "Bind variables to min/max size, font family/style and paragraph spacing/indent", "M", "P1",
  "variables_apply.rs, fig.rs, fig_export.rs, text model", "doc + .fig round trip",
  "A string variable switches the font family per mode|Bound min/max feeds the shared solver|Round trip keeps every binding",
  deps=["#30"], ctx="Needs the shared solver from #30 / PR #47.")
t("E1-5c", "E1", "v0.8", "Bind variables to component properties (boolean, text, variant)", "M", "P2",
  "components.rs, variables_apply.rs, fig.rs", "doc test + .fig round trip",
  "A boolean variable toggles the prop under a mode switch|Round trip keeps the binding", rea="E1-c: component-prop variable binding path.")
t("E1-6", "E1", "v0.8", "Support extended variable collections with per-variable overrides", "L", "P2",
  "variables.rs, variables_edit.rs, variables/view.rs, designos-fig/src/variables.rs, fig.rs", "doc resolution test + .fig round trip",
  "A parent change flows to non-overridden values|Reset restores the parent value|parentVariableSetId=4 / parentModeId=5 / isCollectionExtendable=385 round-trip, incl. a library parent",
  ui=True, ctx="VariableCollection has no parent fields (variables.rs:20-29). Not plan-gated.",
  rea="E1-d: how VARIABLE_SET extension overrides are stored.")
t("E1-7", "E1", "v0.8", "Store and render Display P3 colour per document", "L", "P2",
  "designos-doc lib.rs, paint.rs, fig.rs, fig_export.rs, designos-render, designos-gpu, designos-export", "pixel diff, CPU = GPU",
  "A P3 file renders like Figma on a P3 display (screenshot)|sRGB files stay bit-exact|documentColorProfile round-trips; PNG export carries the P3 tag",
  rul="R3.9: store and render Display P3 per document; sRGB files stay bit-exact.",
  ctx="Color is straight sRGB (paint.rs:4-11); documentColorProfile only kept in metadata (fig.rs:663).",
  rea="E1-e: documentColorProfile enum and colour encoding in P3 files.")
t("E1-8", "E1", "v0.8", "Add OKLCH/OKLAB picker modes and emit oklch() and color(display-p3) CSS", "M", "P2",
  "widgets/picker.rs, designos-export/src/css.rs", "unit conversion tests + screenshot",
  "Known-value conversions within 1/255|A P3 document emits color(display-p3 ...) and oklch() where chosen|sRGB documents emit unchanged CSS",
  deps=["E1-7"], rul="R3.9 (OKLCH/OKLAB picker modes and CSS oklch()/display-p3).", ui=True)
t("E1-9", "E1", "v0.8", "Import and export variables as Figma JSON and W3C DTCG tokens", "M", "P2",
  "designos-fig/src/variables.rs, variables/mod.rs", "fixture round trip",
  "A Figma-exported variables JSON imports losslessly|Our Figma-format export re-imports into Figma (manual check, screenshot)|DTCG export validates against the DTCG format and re-imports losslessly",
  rul="R3.10: Figma-native and W3C DTCG design tokens.",
  ctx="JSON import/export exists (variables.rs:158,215) but its Figma-format parity is an unverified ASSUMPTION.",
  rea="E1-f: Figma variables JSON schema.")
t("E1-10", "E1", "v0.17", "Model animation styles (StyleType ANIMATION) with picker and .fig round trip", "M", "P2",
  "designos-doc lib.rs Style, styles.rs, fig.rs, fig_export.rs", ".fig round trip",
  "ANIMATION=7 styles and styleIdsForAnimation (582) round-trip|Apply links the style to a layer's timeline|Old files load",
  deps=["E6-1"], ctx="Absorbs the model/.fig half of the old E6-19 draft (duplicate removed). E6-19 keeps library publish/consume.", src="A+B")
t("E1-11", "E1", "v0.17", "Add easing and timing variables bound to transitions and Motion tracks", "M", "P2",
  "variables.rs, variables_apply.rs, prototype.rs, motion module", "doc test",
  "A bound easing switches per mode|Changing a durationUsVar variable re-times bound tracks|Round trip of parameterConsumptionMap bindings",
  deps=["E6-2", "X2"], ctx="Merged with the duplicate draft E6-20 (durationUsVar, easing vars). VariableKind today = Color/Number/String/Boolean.",
  rea="Variable bindings through parameterConsumptionMap.", src="A+B")

# ================================================================ E2 Auto layout & Grid
t("E2-1", "E2", "v0.4", "Edit grid tracks on canvas: insert, delete, resize and drag-reorder", "M", "P1",
  "canvas/grids.rs, canvas/draw.rs, panels/properties.rs", "shell test with literal track lists + screenshot",
  "Moving track 2 to 4 moves the items anchored there in one undo step|Deleting a track re-flows auto-placed items|Labels match Figma",
  deps=["#24", "#25"], ui=True, ctx="#24 lists reorder as a non-goal; canvas only labels tracks (canvas/grids.rs:31-47).")
t("E2-2", "E2", "v0.4", "Align grid children within their cell horizontally and vertically", "M", "P1",
  "node.rs, layout_solver_grid.rs, fig.rs, fig_export.rs, rest_import.rs", "solver parity with literal positions + .fig round trip",
  "A centred child sits centred in a 100x100 cell|gridChildHorizontal/VerticalAlign round-trip|Old files load with Start/Start",
  deps=["#30"], ctx="gridChildVerticalAlign is metadata-only (fig.rs:661); LayoutChild has no field (node.rs:699-713).",
  rea="E2-a: gridChildHorizontal/VerticalAlign enums.")
t("E2-3", "E2", "v0.4", "Generate CSS, Tailwind and React grid code from real tracks and placement", "M", "P1",
  "designos-export/src/css.rs:43-97, codegen.rs", "golden export tests",
  "[1fr, 120px, Hug] emits 1fr 120px auto|Explicit anchors emit grid-column: n / span k|Gaps emit from the model",
  deps=["#24", "#63"], ctx="css.rs:97 always writes repeat(n, minmax(0,1fr)).")
t("E2-4", "E2", "v0.4", "Reach layout guide parity: alignment, fixed width, auto count, per-guide visibility", "M", "P1",
  "paint.rs:520 LayoutGrid, panels/layoutguide.rs, fig.rs, fig_export.rs encode_grid, render", "render pixel test + .fig round trip",
  "Centred 12 x 60 px columns draw at literal x|A hidden guide is not drawn but round-trips|Multiple guides per frame; grid styles carry the new fields; old files load",
  ui=True, rea="E2-b: alignment, sectionSize, count = -1 for auto.")
t("E2-5", "E2", "v0.4", "Support per-child align-self in auto layout version 1", "S", "P2",
  "node.rs, layout solver, fig.rs", "solver parity + .fig round trip",
  "MAX/CENTER align-self children sit at literal positions|Round trip keeps the value|If REA shows no version-1 align-self UI, close with the REA evidence",
  deps=["#30", "#26"], rea="E2-c: align-self UI in version 1 and Figma's default layout version for new frames.",
  ctx="Import reads STRETCH only (fig.rs:1016).")
t("E2-6", "E2", "v0.4", "Refresh frame and device presets to Figma's current list", "S", "P2",
  "designos-app proto/mod.rs:38, frame tool presets", "unit test of the preset list",
  "iPhone 17 family present with Figma's sizes|Every preset in Figma's current list exists with the same name and size",
  rea="E2-d: preset list.", ctx="Only iPhone 15 / 15 Pro Max today. Cross-check gap item 8: device frames belong to E2.")

# ================================================================ E3 Slots & Check designs
t("E3-7", "E3", "v0.5", "Show empty-slot placeholders, a preferred-instance picker and stretch-on-insert", "M", "P1",
  "canvas, panels/components.rs", "shell test + screenshot",
  "An empty slot shows a placeholder in the main component and in instances (displayByDefault)|The + picker lists preferred instances only when set|An inserted child stretches when the flag is on",
  deps=["#32", "#34"], ui=True)
t("E3-8", "E3", "v0.5", "Migrate instance-swap or boolean regions to slots, keeping instance content", "M", "P2",
  "comp_ops.rs, components.rs", "doc test",
  "Instance content becomes slot content in one undo step|Unsupported shapes are a no-op with an honest message",
  deps=["#32"], rea="E3-c: migrate-to-slots command semantics.")
t("E3-9", "E3", "v0.5", "Emit slots in codegen and in Dev Mode inspect", "M", "P2",
  "designos-export/src/codegen.rs, react.rs, swiftui, compose", "golden exports",
  "React emits children/ReactNode, SwiftUI @ViewBuilder, Compose slot lambda|Dev Mode inspect shows the slot", deps=["#31"])
t("E3-10", "E3", "v0.5", "Check designs: flag hard-coded numbers that exactly match number variables", "M", "P1",
  "check_designs/hardcoded.rs, fix.rs", "literal fixtures",
  "A 16 px gap matching space/4 is flagged with a Bind fix|Gap, padding, radius, stroke, font size and line height are covered|Ambiguous matches are reported without a fix",
  deps=["#35"], rul="R2.7 (D-E3-3): exact value matches only; ambiguous matches reported without a fix.",
  ctx="Index holds colour variables only (task/35 hardcoded.rs:33).")
t("E3-11", "E3", "v0.5", "Check designs: suggest variables and styles from enabled libraries", "M", "P2",
  "check_designs/hardcoded.rs, libraries/", "fixture with an enabled library",
  "A library variable is suggested with its library name|The fix imports and binds in one undo step|Disabled libraries are never suggested",
  deps=["#35", "E3-10"], rul="R2.7: exact matches only.")
t("E3-12", "E3", "v0.5", "Check designs: scan whole pages incrementally off the UI thread", "M", "P1",
  "check_designs/mod.rs, state.rs", "perf test on the 723k-node fixture (budgets unchanged)",
  "The full page completes with no frame over the 8 ms budget|Progress is shown|No layer cut-off (MAX_SCANNED_LAYERS removed)",
  deps=["#35"], ctx="MAX_SCANNED_LAYERS = 20_000 (task/35 check_designs/mod.rs:30).")
t("E3-13", "E3", "v0.5", "Check designs: match Figma's rule set, thresholds and dismiss/ignore", "M", "P2",
  "check_designs/*", "fixtures per rule",
  "Every rule in the REA list exists with Figma's thresholds (AAA, large text)|Dismiss/ignore persists in the document|Ignored findings never reappear after rescan",
  deps=["#35"], rea="E3-d: Check designs rule list, number tokens, AAA/large text, ignore list, library sources.")
t("E3-14", "E3", "v0.5", "Add a read-only component playground in Dev Mode", "M", "P2",
  "designos-app/src/dev/", "shell test + screenshot",
  "Prop toggles on the scratch instance never mutate the document (doc_hash unchanged)|Codegen updates live",
  rul="D-E3-2: build it (owner round 7 note: full-scope ruling answers it).", ui=True)
t("E3-15", "E3", "v0.5", "Verify the owner's VSF - PCP.fig carries slot data and derive a synthetic slot fixture", "S", "P1",
  "designos-fig testkit fixtures (synthetic only)", "local scan + fixture test",
  "Report counts of isSlot (463), isSlotContent (495) and SLOT props found in /Users/jang/Downloads/VSF - PCP.fig|If zero: stop and report to the owner (no fixture invented)|If present: a minimal synthetic fixture reproduces the structure with no owner content; any test that reads the real file is skipped when the path is absent",
  deps=["#31"], rul="R2.6: VSF - PCP.fig is the slot reference; verify first; never commit the file (owner's private file, 124 MB).",
  ctx="Feeds #107 (preferredValues shape) and #110 (Figma numbering, Figma.app open). 0 slot hits in 6,045 cached files per the PR #49 review.", src="D")

# ================================================================ E5 Effects & Shaders
t("E5-1", "E5", "v0.6", "Add blend modes to drop and inner shadows", "S", "P1",
  "paint.rs, designos-render, designos-gpu, fig.rs, fig_export.rs, rest_import.rs, panels/effects.rs", "pixel test (multiply), CPU = GPU, round trip",
  "A multiply shadow darkens the backdrop at a literal pixel|.fig and REST round trip keep the blend mode|Old files default to Normal",
  ui=True, rea="E5-c: Effect.blendMode field id.")
t("E5-2", "E5", "v0.6", "Cap effect counts in the + control without trimming imported effects", "S", "P2",
  "panels/effects.rs", "shell test",
  "The 9th drop shadow (8/8/1/1 limits) is disabled with a tooltip|An imported file with more effects keeps them all",
  ctx="Limits enforced in UI only; imported data is never dropped (Law 1).", ui=True)
t("E5-3", "E5", "v0.6", "Add image fill adjustments, rotation, tile scale and crop", "L", "P1",
  "paint.rs, render paint, gpu paintconv.rs, fig.rs, fig_export.rs, rest_import.rs, image paint UI", "pixel tests, CPU = GPU, round trip",
  "Exposure +1 brightens a literal pixel equally on CPU and GPU|A 90 degree rotation is exact|Exposure/contrast/saturation/temperature/tint/highlights/shadows, rotation, tile scale and crop round-trip",
  ui=True, ctx="PaintKind::Image{image_ref, scale_mode} only (paint.rs:207-210). Covers non-AI crop (cross-check gap item 8).",
  rea="E5-d: image paint filter and transform fields.")
t("E5-4a", "E5", "v0.29", "Decode video through OS codecs in one shared media module", "M", "P1",
  "new shared media decode module (platform shims next to E6-28's encoders), designos-app Cargo.toml (openh264 feature removed)", "per-OS decode smoke test + seek fixture",
  "H.264/HEVC decode via VideoToolbox/AVFoundation on macOS, Media Foundation on Windows, AV1 on Linux, each delivering RGBA frames to a caller-supplied sink|Seek to a timestamp lands on the right frame (fixture) and loop restarts at 0|The default build has no openh264 (cargo tree)|An unsupported codec returns a named error, never a silent blank frame",
  deps=["E6-28"], rul="R3.11: OS codecs - VideoToolbox/AVFoundation/AudioToolbox on macOS, Media Foundation on Windows, AV1 + Opus on Linux.",
  ctx="Split from the old E5-4 (coordinator ruling 2026-10-17). Shared by E5-4b, E11-5 (Present), E12-8 (canvas), E13-12 (site video backgrounds) and E14-13 (iOS player). Today only Motion-JPEG decodes by default; H.264 needs the off-by-default openh264 feature (designos-app/Cargo.toml:21-28), which conflicts with R3.11.", src="A+E")
t("E5-4b", "E5", "v0.29", "Support video fills: keep on import, poster frame on canvas, round trip", "M", "P2",
  "paint.rs, render, fig.rs, rest_import.rs", ".fig/REST round trip + poster render test",
  "Video paint round-trips losslessly (no more drop at rest_import.rs:95)|The canvas shows the first frame as the poster on CPU and GPU|Old files load unchanged",
  deps=["E5-4a"], rul="R3.11: OS codecs.", rea="Video paint fields.",
  ctx="Split from the old E5-4. Playback in Present moved to E11-5; playback on the canvas is E12-8.", src="A+E")
t("E5-5", "E5", "v0.24", "Author custom WGSL shaders and render them on the GPU", "XL", "P2",
  "new designos-doc shader asset, designos-gpu programs, app code panel", "gpu render test + screenshot",
  "A user WGSL gradient renders|A compile error shows inline and never crashes|Obeys the 64-program cap and the 512 MiB refine VRAM budget",
  deps=["#39"], ui=True, rul="R2.8 (D-E5-1): user-authored WGSL, GPU renders. Figma's shader source is server-side, so imported Figma shaders stay preserve-only (#39).",
  rea="CUSTOM uniform schema.")
t("E5-6", "E5", "v0.24", "Render shader nodes on CPU and headless paths from a cached GPU readback", "L", "P2",
  "designos-render, designos-gpu readback, parity test lists", "CPU/GPU diff + exclusion list",
  "CPU/headless output equals the cached GPU readback for the same parameters|No cache and no GPU: render the fallback fill and emit a named warning (never silent)|Shader nodes are on the bit-exact exclusion list and the parity suite stays green",
  deps=["E5-5"], rul="R2.8: CPU and headless use a cached GPU readback; shader nodes excluded from the CPU=GPU bit-exact suite.")
t("E5-7", "E5", "v0.25", "REA study: 3D in Spline, Rive and the Figma 126.9.13 kiwi schema", "M", "P2",
  "plans/reports only (no product code)", "REA evidence report",
  "Spline and Rive desktop apps inspected with rea (parallel workers, disjoint scopes) with cited evidence|Every 3D field in the 126.9.13 kiwi schema listed with id and type|A recommended 3D transform model + render approach for E5-8",
  rul="R3.12: REA Spline/Rive + schema 126.9.13, build our own, do not wait for Figma GA. Global rule: reverse engineering uses morluto/rea.", src="D")
t("E5-8", "E5", "v0.25", "Design and implement 3D layer transforms", "XL", "P2",
  "node.rs, designos-render, designos-gpu, fig.rs, fig_export.rs, transform UI", "render tests + .fig round trip + screenshot",
  "rotateX/rotateY + perspective render CPU = GPU (or a documented exclusion)|126.9.13 3D fields round-trip through .fig|Old files load unchanged|SVG/PDF/Lottie exports emit a named warning for 3D",
  deps=["E5-7", "X1"], ui=True, rul="R3.12: our own implementation informed by the REA study.", src="D")
t("E5-9", "E5", "U", "Align 3D transforms with Figma once its 3D reaches GA", "M", "P3",
  "same as E5-8", "REA diff + .fig fixtures from Figma GA",
  "A Figma GA 3D file opens with every 3D field kept|Differences between our model and Figma GA are migrated or documented",
  deps=["E5-8"], rul="R3.12 task 3: follow-up when Figma 3D reaches GA.", ctx="Trigger: Figma 3D GA. Stays unscheduled until then.", src="D")
t("E5-10", "E5", "v0.32", "Route AI image tools through the agent CLI: remove background, isolate, expand, erase", "M", "P2",
  "designos-app terminal.rs / agent runtime bridge, image paint UI", "shell test with a stub agent command + screenshot",
  "Each command sends the image and instruction to the user's Studio Agent runtime and inserts the returned image in one undo step|No agent configured: commands are disabled with an explanation|The original image is kept (nothing vanishes)|cargo tree shows no bundled model crate",
  ui=True, rul="R4.13: all AI through the agent CLI; no model bundled, not even segmentation.", src="D")

# ================================================================ Shared groundwork
t("X1", "E4", "v0.4", "Keep every unmapped kiwi field and node type as typed passthrough and re-emit it on export", "L", "P1",
  "designos-fig fig.rs, fig_export.rs, kiwi.rs", "designos-fig testkit round trip",
  "Draw/Motion/TABLE/SLIDE fixture fields are equal after a round trip|The same holds on a file with more than 5,000 entries|Existing round-trip tests stay green",
  ctx="Shared groundwork for E4/E6/E9 (also helps #39, T8.20c). Today extract_fig_metadata keeps an allowlist as JSON strings (fig.rs:609-753), skips nearly all on >5,000 entries (fig.rs:285), and append_fig_metadata never re-emits them. Filed under E4 as first consumer.",
  rea="Confirm the export schema can encode passthrough fields (if not, X1 becomes XL).", src="B")
t("X2", "E6", "v0.11", "Move easing (cubic, spring, presets) into designos-doc and add the 15 EasingType values", "S", "P1",
  "designos-doc prototype.rs, designos-app proto/easing.rs", "unit tests",
  "Prototype player tests unchanged|Preset values match REA r5|All 15 EasingType values exist",
  ctx="Shared by E6 Motion, E9 object animations, E1-11.", src="B")
t("X3", "E6", "v0.7", "Gate dependencies on licence with cargo-deny", "S", "P1",
  "deny.toml, CI workflow", "cargo deny check licenses",
  "The current tree passes|Adding a GPL/LGPL crate fails the gate|MPL-2.0 passes only through the explicit exception list (symphonia)",
  rul="R4.16: MPL-2.0 allowed as an explicit exception.", ctx="Allow MIT/Apache/BSD/ISC/Zlib; flag GPL/LGPL. Lands before new crates (rquickjs, wry, rav1e, symphonia). The repo has no deny.toml today.", src="B")
t("X4", "E9", "v0.14", "Gate releases on a licence review of copied Figma brushes and templates", "M", "P1",
  "assets provenance ledger, release checklist script, About/attribution UI", "script test + release-task evidence",
  "A provenance ledger lists every bundled brush and template: source, licence (Community files usually CC BY 4.0; built-in brushes are Figma proprietary), attribution|The release check fails when an asset has no ledger row|Attribution renders where the licence requires it|Real legal blockers are reported to the owner before shipping; the gate never reverses the decision",
  rul="R5.20: copy Figma's 25 brushes and templates, knowingly; coordinator adds this licence-review gate before release.",
  ctx="Cross-epic: E9-10 (templates, v0.14) and E4-23 (brushes, v0.28) depend on it.", src="D")

# ================================================================ E4 Draw & Vector
t("E4-1", "E4", "v0.10", "Map every Draw field to typed doc fields with .fig and REST round trip", "L", "P1",
  "node.rs, paint.rs, fig.rs, fig_export.rs, rest_import.rs", ".fig round trip",
  "Fields 446-452, 482, 571, 554, 542, 455, 432; BRUSH=46, TRANSFORM=48, TEXT_PATH=41; PATTERN=8 survive .fig -> .designos -> .fig|Old .designos files load",
  deps=["X1"], src="B")
t("E4-2", "E4", "v0.10", "Build variable-width stroke outline geometry", "L", "P1",
  "vectorops.rs, outline.rs", "area test + GPU diff",
  "TAPER area within 1% of the analytic value|Headless CPU = GPU bit-exact|Dashed strokes ignore the width profile",
  deps=["E4-1"], rea="Width interpolation between profile points.", src="B")
t("E4-3", "E4", "v0.10", "Add the variable-width tool with presets, width handles and stroke panel", "M", "P1",
  "properties.rs, canvas/handles.rs", "shell test + 1440 screenshot",
  "A handle drag is one undo step|7 presets; a preset change renders on the same frame", deps=["E4-2"], ui=True, src="B")
t("E4-4", "E4", "v0.10", "Render static path trim with a panel control", "M", "P1",
  "designos-render, designos-gpu geom.rs, properties", "pixel diff",
  "Trim 0.25-0.75 draws half the path|CPU = GPU", deps=["E4-1"], ctx="E6-16 reuses it.", src="B")
t("E4-5", "E4", "v0.10", "Add the Draw toolbar mode", "M", "P1",
  "shell.rs, tools.rs", "shell test + screenshot",
  "Draw mode shows only Draw tools|The shortcut matches Figma", ui=True, src="B")
t("E4-6", "E4", "v0.10", "Scale multiple selected vector points with a bounding box", "S", "P2",
  "canvas/vector.rs", "shell test",
  "Scaling the selected points' box scales only those points|If it already exists, close with a test proving it", ctx="Verify first.", src="B")
t("E4-7", "E4", "v0.10", "Export Draw geometry as outlines in SVG, PDF and codegen", "M", "P1",
  "designos-export svg.rs, pdf.rs, codegen", "golden tests",
  "A variable-width stroke exports as a filled path|No Draw data is silently dropped; a warning is listed",
  deps=["E4-2", "#40"], ctx="Conflicts with PR #88 in pdf.rs; wait for it to merge. Extended as later Draw features land.", src="B")
t("E4-8", "E4", "v0.15", "Add the TransformGroup node with linear and radial repeat", "L", "P1",
  "node.rs, index.rs, designos-render, designos-gpu compile.rs", "render + hit tests",
  "Radial x12 renders CPU = GPU|40x40 copies pan within 8 ms on the VSF page|No new GPU Program",
  deps=["E4-1"], src="B")
t("E4-9", "E4", "v0.15", "Add symmetry and skew modifiers, stacking up to two", "M", "P1",
  "node.rs, index.rs, render, gpu compile.rs", "render tests",
  "Symmetry mirrors live edits|A third modifier is refused", deps=["E4-8"], src="B")
t("E4-10", "E4", "v0.15", "Build the transform group UI: create from selection, modifier panel, handles", "M", "P1",
  "properties, canvas", "shell test + screenshot",
  "Changing the count is one undo step|Ungroup bakes the copies", deps=["E4-9"], ui=True, src="B")
t("E4-11", "E4", "v0.15", "Render pattern paints with a byte-capped tile cache", "L", "P1",
  "designos-render paint.rs, designos-gpu paintconv.rs", "pixel diff",
  "RECT/H-HEX/V-HEX rows offset correctly with spacing, scale and alignment|The cache stays under its cap during scroll",
  deps=["E4-1"], rea="Hex pattern alignment.", src="B")
t("E4-12", "E4", "v0.15", "Add the pattern fill UI: source picker and tile options", "M", "P1",
  "fill panel", "shell test + screenshot",
  "Editing the source updates every pattern|Round trip keeps the options", deps=["E4-11"], ui=True, src="B")
t("E4-13", "E4", "v0.21", "Add the eraser tool", "M", "P1",
  "canvas/vector.rs, vectorops.rs", "area tests",
  "Erasing across a shape splits it|One undo step", rea="Eraser semantics.", src="B")
t("E4-14", "E4", "v0.21", "Add the cut (scissors) tool", "M", "P1",
  "canvas/vector.rs", "vector network tests",
  "Cutting a closed path opens it at the cut point|One undo step", rea="Cut semantics.", src="B")
t("E4-15", "E4", "v0.21", "Add the offset path command", "M", "P1",
  "vecops.rs, vectorops.rs", "area tests",
  "+10 on a 100 px square gives about 120^2 minus the corners|A negative offset shrinks the shape", rea="Offset semantics.", src="B")
t("E4-16", "E4", "v0.21", "Turn the single stroke into a stroke list with OFFSET alignment", "L", "P1",
  "node.rs, every renderer, codegen, designos-mcp, fig.rs/fig_export.rs (stroke encode)", "full workspace suite",
  "Old files migrate|A single stroke renders exactly as before|Multiple strokes round-trip through .fig",
  deps=["E4-1"], ctx="Widest blast radius in E4 (every renderer, codegen, MCP).", src="B")
t("E4-17", "E4", "v0.21", "Add the multi-stroke UI: add, reorder, offset value", "M", "P1",
  "stroke panel", "shell test + screenshot",
  "3 strokes render in order|Round trip keeps order and offsets", deps=["E4-16"], ui=True, src="B")
t("E4-18", "E4", "v0.21", "Add stroke sampling per REA semantics", "M", "P2",
  "TBD after REA", "TBD after REA",
  "Behaviour matches the REA definition with a fixture|Round trip keeps the setting", rea="Stroke sampling semantics.", src="B")
t("E4-19", "E4", "v0.28", "Build dynamic stroke geometry (frequency, wiggle, smoothen, seed)", "M", "P1",
  "vectorops.rs", "determinism test",
  "The same seed gives identical output|CPU = GPU", deps=["E4-2"], rea="Dynamic-stroke noise function.", src="B")
t("E4-20", "E4", "v0.28", "Add the dynamic stroke UI", "S", "P1",
  "stroke panel", "shell test + screenshot", "Slider ranges match Figma (0.01-20, 0-1)", deps=["E4-19"], ui=True, src="B")
t("E4-21", "E4", "v0.28", "Build the stretch brush engine that warps artwork along a path", "L", "P1",
  "designos-render", "pixel diff",
  "An imported STRETCH brush renders without being flattened|CPU = GPU", deps=["E4-1"], rea="Brush stamp and warp model.", src="B")
t("E4-22", "E4", "v0.28", "Build the scatter brush engine (gap, wiggle, angle/size jitter, rotation)", "L", "P1",
  "designos-render", "determinism + diff",
  "Seeded and stable across re-renders|CPU = GPU", deps=["E4-1"], src="B")
t("E4-23", "E4", "v0.28", "Ship Figma's 25 built-in brushes with a brush picker", "M", "P1",
  "assets, brush panel", "screenshot + offline test",
  "All 25 brushes extracted from Figma.app (via rea) apply|They work offline|Each brush has a provenance row in the X4 ledger",
  deps=["X4", "E4-21", "E4-22"], ui=True, rul="R5.20: copy Figma's 25 built-in brushes; X4 licence gate before release.",
  rea="Extract brush artwork and parameters from Figma.app with rea.", src="B+D")
t("E4-24", "E4", "v0.28", "Create a custom brush from a selection", "M", "P2",
  "designos-doc, brush panel", "shell test",
  "A new brush appears in the picker and round-trips|If REA shows Figma lacks the feature, close with evidence",
  deps=["E4-23"], rea="Whether Figma supports custom brushes.", src="B")
t("E4-25", "E4", "v0.28", "Lay out text along a path", "L", "P1",
  "text/, textmeasure.rs", "glyph tests",
  "The baseline follows the tangent|forward=false reverses direction", deps=["E4-1"], src="B")
t("E4-26", "E4", "v0.28", "Add the text-on-path tool: attach, drag start point, flip, edit", "M", "P1",
  "canvas, textedit.rs", "shell test + screenshot",
  "Text stays editable after attaching to a path|Start-point drag and flip are one undo step each", deps=["E4-25"], ui=True, src="B")
t("E4-27", "E4", "v0.32", "Vectorize raster images through the agent CLI", "M", "P3",
  "agent runtime bridge, image context menu", "shell test with a stub agent command",
  "The agent CLI returns SVG and it inserts as editable vector paths in one undo step|No agent configured: disabled with an explanation|No tracer or model crate bundled",
  rul="R4.13: vectorize goes through the agent CLI; no bundled tracer (replaces the draft's vtracer recommendation).", src="B+D")

# ================================================================ E6 Motion & Media export
t("E6-1", "E6", "v0.11", "Add the Motion model: timelines, tracks, keyframes, interpolation", "M", "P1",
  "designos-doc new motion module, node.rs", "serde tests",
  ".designos round trip incl. nested timelines, snap/quantize and track loop|A doc without Motion serializes byte-identical", deps=["X2"], src="B")
t("E6-2", "E6", "v0.11", "Evaluate Motion timelines (SET/SCALE/OFFSET, ONCE/LOOP/BOOMERANG, nesting)", "M", "P1",
  "designos-doc", "sampling tests",
  "BOOMERANG at 1.5d equals the value at 0.5d|Springs match spring_at|Pure function (doc, t_us) -> overrides", deps=["E6-1"], src="B")
t("E6-3", "E6", "v0.11", "Apply Motion values to transform, opacity, radius, stroke, size and colour", "M", "P1",
  "designos-doc, designos-render", "unit + diff",
  "OKLCH midpoint matches a reference value (5 colour spaces)|Applying never mutates doc", deps=["E6-2"], src="B")
t("E6-4", "E6", "v0.11", "Import and export Motion through .fig kiwi", "L", "P1",
  "fig.rs, fig_export.rs, kiwi.rs", ".fig round trip",
  "Nodes 56-58 and fields 445, 505-509, 535, 552, 553, 591, 599 round-trip|A Figma Motion file keeps times, values and easings", deps=["X1", "E6-1"], src="B")
t("E6-5", "E6", "v0.11", "Play Motion on canvas, in Present and on prototype autoplay", "M", "P1",
  "canvas/gpu.rs, present.rs", "shell test + perf probe",
  "TileView::rev does not change during playback and app.changed() is never called|A transform-only track causes no re-tessellation", deps=["E6-3"], src="B")
t("E6-6", "E6", "v0.11", "Add Motion mode and the timeline panel", "L", "P1",
  "new panels/timeline*.rs, shell.rs", "shell test + screenshot",
  "Add/move/delete keyframe are one undo step each|Scrubbing updates the same frame|Tracks, diamonds, zoom and snapping work", deps=["E6-2"], ui=True, src="B")
t("E6-7", "E6", "v0.11", "Add the easing editor for all 15 easing types", "M", "P1",
  "panels", "shell test + screenshot",
  "Dragging a bezier handle updates the curve preview and the playback|Spring parameters edit live", deps=["E6-6"], ui=True, src="B")
t("E6-8", "E6", "v0.11", "Model export settings and round-trip them through .fig", "M", "P1",
  "designos-export lib.rs, fig.rs, export panel", "unit + round trip",
  "Defaults: GIF 15 fps, video 30 fps|frameRate/loopCount/background/duration/svgAnimated/quality and ImageType 4-8 survive .fig", deps=["E6-1", "X1"], ui=True, src="B")
t("E6-9", "E6", "v0.11", "Render frame sequences on a worker thread and export PNG sequences", "M", "P1",
  "designos-export, CLI, designos-mcp", "export tests",
  "Frame t equals a single render at t|The UI thread never blocks|Available from CLI and MCP", deps=["E6-3"], src="B")
t("E6-10", "E6", "v0.11", "Export animated GIF with NeuQuant palette, loop count and fps", "M", "P1",
  "designos-export", "export tests",
  "1 s at 15 fps gives 15 frames|Loop 0 means forever|fps in {8,12,15,24,30}", deps=["E6-9"], ctx="gif + color_quant already in Cargo.lock; reject imagequant (GPL-3).", src="B")
t("E6-11", "E6", "v0.11", "Export static WebP (lossless)", "S", "P2",
  "designos-export", "export test", "Decoded output equals the PNG pixels", ctx="image-webp already in the lock.", src="B")
t("E6-12", "E6", "v0.17", "Add first-party Motion presets and the preset instance node", "M", "P1",
  "designos-doc, timeline UI", "unit tests",
  "FADE creates opacity 0->1 with the default bezier|FADE/MOVE/ROTATE/SCALE/PATH_DRAW/PATH_TRAVEL exist; node 58 round-trips", deps=["E6-6"], src="B")
t("E6-13", "E6", "v0.17", "Add the ~60 composite animation styles", "L", "P2",
  "designos-doc, assets", "golden tests", "Each style produces REA's recipe", deps=["E6-12"], rea="Recipes for the ~60 styles.", src="B")
t("E6-14", "E6", "v0.17", "Add the Motion anchor point", "M", "P1",
  "designos-doc, canvas", "unit + diff", "Rotation pivots around the anchor|Anchor edits are one undo step", deps=["E6-3"], src="B")
t("E6-15", "E6", "v0.17", "Animate a layer along a motion path", "L", "P1",
  "designos-doc, canvas", "unit tests", "The layer follows the path at t|Round trip keeps the path binding", deps=["E6-3"], rea="Motion-path coordinate space.", src="B")
t("E6-16", "E6", "v0.17", "Add path-trim tracks and PATH_DRAW/ERASE/TRAVEL recipes", "S", "P1",
  "designos-doc", "unit tests", "Draw-on animates trim 0->1", deps=["E4-4", "E6-12"], src="B")
t("E6-17", "E6", "v0.17", "Animate the 21 effect fields", "M", "P2",
  "designos-doc, designos-render", "diff", "Animated blur radius renders identically on CPU and GPU at t", deps=["E6-3"], src="B")
t("E6-18", "E6", "v0.17", "Animate text with stagger by character, word or line", "L", "P1",
  "designos-doc, text/", "unit + diff",
  "Per-word stagger offsets each word by the delay|TEXT_DATA track, order and delay round-trip", deps=["E6-3"], rea="Stagger details.", src="B")
t("E6-19", "E6", "v0.17", "Publish and consume animation styles through libraries", "M", "P2",
  "styles.rs, fig.rs, library publish", "round trip + library test",
  "A published animation style applies in a consumer file, read-only (A13)|Library updates propagate", deps=["E1-10", "E6-12"],
  ctx="Model/.fig half moved to E1-10 (duplicate resolved). E6-20 (easing/timing variables) merged into E1-11.", src="B")
t("E6-21", "E6", "v0.22", "Export Lottie with a feature lint", "L", "P1",
  "designos-export", "schema + golden",
  "Output passes the lottie-web schema|Angular/diamond gradients, shaders, 3D and video get a named warning; shadow/glass/noise are rasterized", deps=["E6-8"], src="B")
t("E6-22", "E6", "v0.22", "Export dotLottie", "S", "P1", "designos-export", "zip test", "The archive opens in a dotLottie player", deps=["E6-21"], src="B")
t("E6-23", "E6", "v0.22", "Export animated SVG", "M", "P1", "designos-export", "golden", "SMIL/CSS timing matches the keyframes", deps=["E6-8"], src="B")
t("E6-24", "E6", "v0.22", "Export CSS keyframes", "M", "P1", "designos-export", "golden",
  "Percent stops equal keyframe times|cubic-bezier values match", deps=["E6-8"], src="B")
t("E6-25", "E6", "v0.22", "Export Motion as Figma-schema JSON", "M", "P2", "designos-export", "golden",
  "Output matches the REA-captured sample", rea="JSON export format.", src="B")
t("E6-26", "E6", "v0.22", "Export Motion as React in Figma's exact flavour", "M", "P1", "designos-export, codegen", "golden + tsc",
  "The emitted component compiles (tsc)|Output matches Figma's flavour from the REA read; CSS-in-JSX is the interim fallback",
  deps=["E6-24"], rul="R6.23: exactly Figma's flavour after an REA read; CSS-in-JSX interim.", rea="React export format.", src="B+D")
t("E6-27", "E6", "v0.29", "Export WebM with rav1e AV1 and an EBML muxer", "L", "P1", "designos-export (motion-video feature)", "container parse",
  "Plays in Chrome and Firefox (screenshot)|Quality LOW/MED/HIGH|The default build has no rav1e (feature-gated)", deps=["E6-9", "X3"],
  rul="R3.11: Linux uses AV1 + Opus.", src="B")
t("E6-28", "E6", "v0.29", "Export MP4 through OS encoders", "XL", "P1", "designos-export, platform shims (macOS/Windows)", "per-OS smoke test",
  "Plays in QuickTime (macOS) and the Windows player (screenshots)|Cancel leaves no file|Linux offers AV1 + Opus with a named note",
  deps=["E6-9", "X3"], rul="R3.11: VideoToolbox/AudioToolbox on macOS, Media Foundation on Windows, AV1 + Opus on Linux. No openh264 compiled in.", src="B+D")
t("E6-29", "E6", "v0.29", "Add video export progress, cancel and size estimate", "M", "P1", "export panel", "shell test + screenshot",
  "Cancel within 1 frame|The UI stays responsive", deps=["E6-27"], ui=True, src="B")
t("E6-30", "E6", "v0.29", "Put audio on the timeline: import, playback, waveform track", "L", "P2", "designos-doc, timeline UI", "unit + manual",
  "Audio stays in sync with the scrubber within 1 frame|Waveform renders|Playback goes through the shared audio output service (E11-6); no second audio stack", deps=["E6-6", "X3", "E11-6"],
  rul="R4.16: symphonia (MPL-2.0) allowed via X3 exception.", rea="Audio data model.",
  ctx="Audio output is the shared E11-6 service (same pack), so Present video audio and timeline audio use one OS audio path.", src="B+E")
t("E6-31", "E6", "v0.29", "Mux audio into exports: Opus in WebM, AAC via OS encoders in MP4", "M", "P2", "designos-export", "container parse",
  "The A/V duration delta is at most 1 frame", deps=["E6-27", "E6-28", "E6-30"], rul="R3.11.", src="B")

# ================================================================ E7 Dev Mode & MCP
t("T7.1", "E7", "v0.7", "Return node screenshots as MCP image content (designos_get_screenshot)", "M", "P2", "designos-mcp server.rs, engine.rs tools()", "cargo test -p designos-mcp",
  "Returns image/png base64 for a node at scale 0.25-4|Size equals bounds x scale within 1 px|Above the pixel cap a named error; nothing truncated silently",
  ctx="render_png only writes a file today (server.rs:465-488).", src="C")
t("T7.2", "E7", "v0.7", "Return sparse XML node metadata (designos_get_metadata)", "S", "P2", "designos-mcp", "unit test",
  "The XML parses|Node count equals the subtree up to the depth limit|Hidden nodes are flagged", src="C")
t("T7.3", "E7", "v0.7", "Move design-context data into designos-export so app and MCP share it", "M", "P2", "designos-app/src/dev/data.rs, designos-export", "existing dev/mod.rs tests",
  "No logic duplicated|Inspect panel output unchanged", src="C")
t("T7.4", "E7", "v0.7", "Return code, hints and a screenshot from designos_get_design_context", "L", "P2", "designos-mcp, designos-export", "MCP fixture test",
  "A fixture card yields CSS, a var(--Color-Primary, ...) hint and one image|Above 40k chars: XML fallback with truncated:true|Bounded and off the UI thread on the 723k-node file",
  deps=["T7.1", "T7.2", "T7.3"], src="C")
t("T7.5", "E7", "v0.7", "Target the editor selection by default, configurable per user and session", "M", "P2", "live_canvas_scope.rs, designos-mcp server.rs:194, settings", "live_sync_tests.rs",
  "Calls without node_id use the configured default: selection, require node_id, or page|The default is settable per user and per session; every tool and option stays exposed|A stale selection gives a clear error; detached sessions require node_id",
  deps=["T7.1", "#75", "#76", "#77", "#78", "#80", "#81"], rul="R1.4: like Figma MCP (selection default when attached) but more open: configurable, expose everything.", src="C+D")
t("T7.6", "E7", "v0.7", "Build the MCP parity matrix against Figma's 41 tools and snapshot the tool list", "S", "P2", "designos-mcp tests, README, CLAUDE.md", "snapshot test of tools()",
  "Every one of Figma's 32 + 9 Weave tools maps to a studio tool, a task ID, or a named exclusion|The snapshot fails when the tool list drifts|README and CLAUDE.md quote the generated list (fixes 22 vs 29 drift)",
  rul="R4.13: generate_image goes through the agent CLI (studio side = upload_assets T7.37). R4.14: Weave and other hosted tools are named exclusions. send_code_to_figma maps to T7.14.", src="C+D")
t("T7.35", "E7", "v0.7", "Add the whoami MCP tool returning the local user", "S", "P2", "designos-mcp", "MCP test",
  "Returns a stable local identity and display name (T10.14 identity when present)|Plan/seat fields report single local user, free", rul="R4.14: hosted identity replaced by a local equivalent.", src="D")
t("T7.36", "E7", "v0.7", "Add create_new_file without replacing the open document", "S", "P2", "designos-mcp, designos-library store.rs", "MCP test",
  "Creates a library file and returns its id|The open document is untouched|Optional template/source argument", deps=["#43"],
  ctx="Only new_document exists and it replaces the open document (#43).", src="D")
t("T7.38", "E7", "v0.7", "Add get_libraries listing local and enabled libraries", "S", "P2", "designos-mcp, designos-library", "MCP test",
  "Lists every library with enabled state, origin and component/style/variable counts|Disabled libraries are flagged, not hidden", src="D")
t("T7.40", "E7", "v0.7", "Add the create_design_system_rules MCP prompt", "S", "P2", "designos-mcp prompts", "MCP prompt test",
  "Produces a rules file from the document's styles, variables (code_syntax) and components|Output names every token source", deps=["T7.3"], src="D")
t("T7.28", "E7", "v0.7", "Add the Vue codegen target", "S", "P1", "designos-export/src/vue.rs, Language::ALL, MCP language list", "snapshot + vue-tsc",
  "The fixture card output validates with vue-tsc|Bound variables print code_syntax (A5, A7)", src="C")
t("T7.29", "E7", "v0.7", "Add the HTML codegen target", "S", "P1", "designos-export/src/html.rs, Language::ALL, MCP language list", "snapshot + html-validate",
  "The fixture card output passes html-validate|Bound variables print code_syntax (A5, A7)", src="C")
t("T7.30", "E7", "v0.7", "Add the Web Components codegen target", "S", "P1", "designos-export/src/webcomponents.rs, Language::ALL, MCP language list", "snapshot + tsc",
  "The fixture card output compiles with tsc|Bound variables print code_syntax (A5, A7)", src="C")
t("T7.31", "E7", "v0.7", "Add the Flutter codegen target", "M", "P1", "designos-export/src/flutter.rs, Language::ALL, MCP language list", "snapshot + flutter analyze",
  "The fixture card output passes flutter analyze|Bound variables print code_syntax (A5, A7)", src="C")
t("T7.13", "E7", "v0.12", "Run Figma Plugin API scripts from MCP (designos_run_plugin_script)", "M", "P2", "designos-mcp, plugin host", "MCP + JS script test",
  "A script mutates the document in one History step|Unsupported calls return the named error|Mirrors Figma use_figma", deps=["T8.8", "T8.9"], src="C")
t("T7.15", "E7", "v0.16", "Model Code Connect mappings in the document", "M", "P2", "new designos-doc/src/code_connect.rs", "native round trip",
  "Multiple {label, component_name, source, snippet, imports, prop_map} per component round-trip|Legacy files load|Deleting a component archives its mappings and reports them", src="C")
t("T7.16", "E7", "v0.16", "Replace the Code Connect stub in Inspect with real snippets", "M", "P2", "panels/inspect.rs:501-552", "rewritten :997 test + screenshot",
  "No @designos/code-connect string remains|Instance props appear in the snippet; label tabs|Unmapped components show the empty state; repo-backed mappings show Edit in code",
  deps=["T7.15"], ui=True, rul="R6.21: repo files win; UI shows Edit in code for repo mappings.", src="C")
t("T7.17", "E7", "v0.16", "Add Code Connect prop-mapping helpers", "M", "P2", "code_connect.rs", "fixtures per helper",
  "enum/boolean/string/instance/children/textContent helpers each covered by a fixture|Variant changes update the snippet", deps=["T7.15"], src="C")
t("T7.18", "E7", "v0.16", "Add MCP Code Connect tools: get map, add map, suggestions, send mappings", "M", "P2", "designos-mcp", "MCP tests",
  "Add then get round-trips|Suggestions skip mapped components|One undo step per add", deps=["T7.15"], src="C")
t("T7.39", "E7", "v0.16", "Add get_context_for_code_connect to MCP", "M", "P2", "designos-mcp, code_connect.rs", "MCP test",
  "Returns props, variants, slots and existing mappings for a component|Repo-backed mappings are marked read-only", deps=["T7.15"], src="D")
t("T7.19", "E7", "v0.16", "Keep Figma node GUIDs on import and re-export", "M", "P2", "fig.rs, kiwi.rs, rest_import.rs, fig_export.rs", ".fig/REST round trip",
  "The GUID survives .fig and REST import and re-export|A Figma node URL resolves to a studio node", src="C")
t("T7.20", "E7", "v0.16", "Ship the designos code-connect CLI where repo files win", "L", "P2", "designos-app cli.rs, code_connect parser", "Figma sample repo test",
  "Figma's sample repo publishes every mapping (*.figma.tsx, .swift, .kt, html)|Re-running is idempotent|Parse errors name file and line",
  deps=["T7.15", "T7.19"], rul="R6.21: repo files win; the document keeps a synced copy.", src="C")
t("T7.21", "E7", "v0.16", "Add the connect-component UI with name-matched source suggestions", "M", "P2", "Dev Mode panel", "shell test + screenshot",
  "Picking a source file maps an unmapped component|Suggestions are name-matched", deps=["T7.16", "T7.18"], ui=True, src="C")
t("T7.12", "E7", "v0.17", "Return Motion context from MCP (get_motion_context)", "M", "P2", "designos-mcp", "MCP test",
  "Returns timelines, tracks, keyframes and easings for a node|Matches the Figma tool's shape", deps=["E6-1"], src="C")
t("T7.7", "E7", "v0.19", "Serve MCP over Streamable HTTP on the project's fixed loopback port", "M", "P2", "designos-mcp main.rs/server.rs, .project-agent.md, port registry", "transport tests",
  "Binds only 127.0.0.1 on the assigned port (fixed-port contract, strict)|A request without the session token gets 401|stdio still works",
  ctx="Port allocated through .project-agent.md + ~/.config/project-localhost-ports.json. [A] rmcp supports Streamable HTTP.", src="C")
t("T7.8", "E7", "v0.19", "Download a subtree's images and vectors into the workspace (download_assets)", "M", "P2", "designos-mcp", "MCP test",
  "Every image fill and exportable vector is written once (hash-deduplicated)|Paths stay confined to the workspace", src="C")
t("T7.37", "E7", "v0.19", "Upload images and files into the document from MCP (upload_assets)", "M", "P2", "designos-mcp, image store", "MCP test",
  "Uploaded images become image fills or placed layers in one undo step|Hash-deduplicated; oversize input gets a named error|Serves as the studio side of generate_image (agent makes the image)",
  rul="R4.13: AI image generation happens in the agent CLI.", src="D")
t("T7.9", "E7", "v0.19", "Search components, styles and variables across libraries (search_design_system)", "M", "P2", "designos-mcp, designos-library", "MCP test",
  "Ranked results include library origin|Disabled libraries are excluded", src="C")
t("T7.10", "E7", "v0.19", "Persist per-file codegen language and units (set_codegen_config)", "S", "P2", "designos-mcp, designos-doc", "MCP test",
  "The setting persists in native save|generate_code and get_design_context honour it", src="C")
t("T7.11", "E7", "v0.19", "Return board content from MCP (get_figjam)", "M", "P2", "designos-mcp", "board fixture test",
  "Returns stickies, connectors, sections and shapes|Connector endpoints resolve", src="C")
t("T7.41", "E7", "v0.24", "Add the shader MCP tools: get, list, list_file, create, update", "M", "P2", "designos-mcp, shader asset", "MCP tests",
  "get_shader, list_shaders, list_file_shaders, create_shader and update_shader exist|create/update compile errors are returned, never crash|One undo step per write",
  deps=["E5-5", "T7.6"], src="D")
t("T7.14", "E7", "v0.30", "Import web captures from MCP (designos_import_web_capture / generate_design)", "S", "P2", "designos-mcp, designos-interop/webcapture", "MCP test",
  "A capture payload or URL becomes editable frames via MCP|Same output as the T8.27 importer|Covers Figma's send_code_to_figma / generate_design rows in T7.6",
  deps=["T8.27"], rul="R6.22: Studio's web-capture importer stays in scope (html-to-figma ban covers only the Figma write workflow).", src="C+D")
t("T7.22", "E7", "v0.27", "Build the annotation model and UI: 33 property types, categories, measurements", "L", "P2", "designos-doc/src/dev.rs, app/src/dev/annot.rs, sections.rs", "shell test + screenshot",
  "All 33 types are selectable|Category colours render (5 presets + 8 colours + custom)|Measurements draw and update when nodes move",
  ui=True, rea="33 AnnotationPropertyType names; measurements shape.", src="C")
t("T7.23", "E7", "v0.27", "Round-trip Dev data through .fig: annotations, categories, measurements, status, resources", "L", "P2", "fig.rs, kiwi.rs, fig_export.rs, designos-fig/src/dev.rs, rest_import.rs", "testkit round trip",
  "A Figma-authored fixture round-trips with equal fields|annotationCategories leaves the fig.rs:737 skip list|sectionStatusInfo=355 and dev resources round-trip", deps=["T7.22"], src="C")
t("T7.24", "E7", "v0.27", "Log Dev status changes (prevStatus, timestamp, description)", "S", "P2", "designos-doc/src/dev.rs", "unit test",
  "Each change appends an entry|The log round-trips via T7.23", deps=["T7.23"], src="C")
t("T7.25", "E7", "v0.27", "Add focus view with interactive inspection", "L", "P2", "app/src/dev/*", "shell test + screenshot",
  "Opens from the Dev list and renders only the subtree|Variant switches and prototype interactions play|Esc returns", deps=["T7.24"], ui=True, rea="Focus view semantics and strings.", src="C")
t("T7.26", "E7", "v0.27", "Add a local inbox for Dev status notifications", "M", "P2", "designos-app, designos-library", "shell test",
  "A status change in file A shows while file B is open|Mark-read persists|Optional OS notification", rul="R4.14: hosted notifications replaced by a local inbox.", ui=True, src="C")
t("T7.27", "E7", "v0.27", "Author specs linked to frames with an MCP get_specs tool", "M", "P2", "app/src/specs.rs, designos-mcp", "shell + MCP test",
  "A spec links to a frame and survives save|MCP returns the specs for a node", ui=True, src="C")
t("T7.32", "E7", "v0.33", "Ship a VS Code extension backed by designos-mcp", "L", "P3", "new vscode extension package", "extension test + screenshot",
  "Installs from a .vsix|Shows the design context of the selected node; Code Connect view", deps=["T7.4", "T7.7"], rul="R4.14: opt-in local add-on.", src="C")
t("T7.33", "E7", "v0.33", "Post Dev status to Jira, Google Chat and Slack (opt-in)", "M", "P3", "integrations module, settings", "integration tests with local stub servers",
  "Off by default|No request before per-integration consent|Jira link previews; Google Chat and Slack status posts",
  rul="R4.14: opt-in Jira / Google Chat / Slack / VS Code add-ons (Slack added per cross-check gap 8).", src="C+D")
t("T7.34", "E7", "v0.33", "Add a local outgoing status hook (user-configured POST)", "S", "P3", "integrations module", "unit test with a loopback server",
  "Off by default|Failures are logged and visible", rul="R4.14: outgoing status hook as the local webhook equivalent.", src="C")

# ================================================================ E8 Interop & Plugins
t("T8.32", "E8", "v0.7", "Propose the 8-plugin compatibility corpus and get owner approval", "S", "P1", "plans/ corpus record", "owner approval recorded",
  "8 open-source Figma plugins listed with repo, licence and API surface covered|Owner approves via AskUserQuestion; the list is recorded|T8.11 reads this list",
  rul="R1.1: coordinator proposes 8 plugins, owner approves.", src="D")
t("T8.3", "E8", "v0.7", "Spike the plugin runtime and write its design doc (behind a flag)", "M", "P1", "plugin host spike (flagged), design doc", "headless run",
  "One corpus plugin runs end to end headless|The doc fixes sandbox limits and the security model|Ships nothing user-visible",
  deps=["T8.32", "X3"], rea="R3 retry with rea: jsvm-cpp.wasm globals, ES level, timers, figma injection, manifest validation.", src="C")
t("T8.4", "E8", "v0.8", "Merge styles, variables, prototype links and library refs on Figma paste", "M", "P1", "designos-fig/src/clipboard.rs, fig.rs, app/src/clipboard.rs", "clipboard tests",
  "A same-key style links; otherwise a local style is created|Repeated pastes create no duplicates|Variables and prototype links survive paste", src="C")
t("T8.1", "E8", "v0.9", "Import PPTX into a deck", "L", "P1", "new designos-interop/pptx, decks/mod.rs, designos-library store.rs probe", "fixture decks (PowerPoint, Keynote, Google Slides)",
  "Slide count and text match|Unsupported elements are counted in the import report|100 slides import in under 2 s [A]", src="C")
t("T8.2", "E8", "v0.9", "Export decks to PPTX", "L", "P1", "designos-interop/pptx", "export -> re-import + Keynote/PowerPoint screenshot",
  "Opens with no repair prompt|Text stays editable; transitions map 1:1 to p:transition|Fallbacks are listed", deps=["T8.1", "E9-2"], src="C")
t("T8.5", "E8", "v0.12", "Host plugins in QuickJS on a worker thread with limits", "L", "P1", "plugins/ host (rquickjs)", "host tests",
  "An infinite loop is killed at the limit with a toast|Memory overrun is contained|Close cancels pending mutations", deps=["T8.3"], src="C")
t("T8.6", "E8", "v0.12", "Import Figma plugins from a manifest in a local folder or repo", "M", "P1", "plugins/store.rs (new source kind)", "manifest tests",
  "An invalid field is named in the error|Menu entries appear|main/ui/editorType/menu/parameters/networkAccess parsed",
  rul="R1.2: Import plugin from manifest from a local folder or repo.", src="C")
t("T8.7", "E8", "v0.12", "Show plugin UIs in an embedded webview with a postMessage bridge", "L", "P1", "plugins/runtime.rs, wry webview", "bridge tests + screenshot",
  "Messages round-trip in under 50 ms|CSP is kept|Falls back to the system browser where no webview exists",
  deps=["T8.5"], ui=True, rul="R1.3: embedded webview (wry/WKWebView), system browser fallback. Linux needs WebKitGTK.", src="C")
t("T8.8", "E8", "v0.12", "Implement the core nodes Plugin API", "L", "P1", "plugin host API", "JS script suite",
  "Every create/read/write call for FRAME, RECT, ELLIPSE, LINE, POLYGON, STAR, VECTOR, TEXT, GROUP, SECTION, BOOLEAN is covered by a script test|selection, currentPage, viewport, notify, closePlugin work", deps=["T8.5"], src="C")
t("T8.9", "E8", "v0.12", "Implement plugin events and undo grouping", "M", "P1", "plugin host API", "JS script suite",
  "selectionchange, documentchange, currentpagechange, run, close fire|One plugin run is one History step", deps=["T8.8"], src="C")
t("T8.10", "E8", "v0.12", "Implement plugin fonts, images and export", "M", "P1", "plugin host API", "JS script suite",
  "loadFontAsync, createImage and exportAsync work offline", deps=["T8.8"], src="C")
t("T8.11", "E8", "v0.12", "Run the plugin corpus headless in CI and report coverage", "M", "P1", "CI harness", "CI report",
  "Reports typings coverage and unsupported calls per corpus plugin|It is the gate for every later E8 pack", deps=["T8.6", "T8.7", "T8.8", "T8.9", "T8.10", "T8.32"], src="C")
t("T8.12", "E8", "v0.18", "Implement styles, variables, components and component-props Plugin APIs", "L", "P1", "plugin host API", "JS script suite",
  "Every call covered by a script test|Corpus coverage rises in the T8.11 report", deps=["T8.8"], src="C")
t("T8.13", "E8", "v0.18", "Map the teamLibrary API to local published libraries", "M", "P1", "plugin host API, designos-library", "JS script suite",
  "teamLibrary calls return local published libraries", deps=["T8.12"], src="C")
t("T8.14", "E8", "v0.18", "Implement clientStorage, parameters/quick actions and relaunch data", "M", "P1", "plugin host API", "JS script suite",
  "clientStorage persists locally per plugin|Parameters show in quick actions|Relaunch data round-trips", deps=["T8.6"], src="C")
t("T8.15", "E8", "v0.18", "Enforce plugin network policy with one-time consent", "S", "P1", "plugin runtime, consent UI", "network tests + screenshot",
  "Only manifest allowedDomains are reachable after consent|No request before consent|Consent is per plugin and revocable",
  deps=["T8.7"], ui=True, rul="R5.17: manifest allowedDomains only, one-time consent per plugin.", src="C")
t("T8.16", "E8", "v0.18", "Implement FigJam and Slides editor-type Plugin APIs", "L", "P1", "plugin host API", "JS script suite",
  "Sticky, connector, shape-with-text and slide APIs covered by script tests", deps=["T8.8"], src="C")
t("T8.17", "E8", "v0.18", "Run Dev Mode codegen and inspect plugins in the Plugins & Codegen panel", "L", "P1", "plugin host, panels/right.rs:186", "plugin test + screenshot",
  "A codegen plugin's output appears as a language tab|Inspect plugins render in the panel", deps=["T8.8"], ui=True, src="C")
t("T8.18", "E8", "v0.18", "Implement the remaining Plugin API surface (text review, util, timer)", "L", "P1", "plugin host API", "T8.11 report",
  "The T8.11 report shows 100% of @figma/plugin-typings minus the hosted exclusions", deps=["T8.12", "T8.13", "T8.14", "T8.15", "T8.16", "T8.17"], src="C")
t("T8.19", "E8", "v0.18", "Stub hosted Plugin APIs as single local user, free", "S", "P1", "plugin host API", "JS script suite",
  "Payments report free; activeUsers/currentUser report the single local user|Plugins calling them keep running|Community install/publish and REST are named exclusions",
  rul="R4.14: hosted services excluded; payments and multiplayer APIs return single local user, free.", src="C")
t("T8.22", "E8", "v0.26", "Import Sketch files: pages, layers, shapes, booleans, text, images", "L", "P2", "designos-interop/sketch", "fixture .sketch files",
  "Layer count and names match|Unsupported layers are counted in the report", deps=["T8.24"], src="C")
t("T8.23", "E8", "v0.26", "Import Sketch symbols as components with overrides, shared styles and libraries", "L", "P2", "designos-interop/sketch", "fixture .sketch files",
  "Symbols become components with overrides|Shared styles become styles", deps=["T8.22"], src="C")
t("T8.24", "E8", "v0.26", "Probe zip entries to route .sketch and zipped .fig imports", "S", "P2", "designos-library store.rs, Home import flow", "probe tests",
  ".sketch and zipped .fig both route correctly", src="C")
t("T8.25", "E8", "v0.30", "Write the web capture script (computed styles, boxes, text runs, images)", "M", "P2", "designos-interop/webcapture", "Playwright on local fixtures",
  "Captures boxes, computed styles, text runs and data-URL images from a fixture page", rul="R6.22: Studio's own web-capture importer stays in scope.", src="C")
t("T8.26", "E8", "v0.30", "Ship a Chrome extension that posts captures to Studio with a token", "M", "P2", "chrome extension package", "extension test",
  "A token is required|Captures land on the open file", deps=["T8.25"], src="C")
t("T8.27", "E8", "v0.30", "Import web captures as auto layout frames with editable text", "L", "P2", "designos-interop/webcapture", "screenshot diff vs source page",
  "Flex maps to auto layout; text stays editable|The screenshot diff against the source page is within threshold", deps=["T8.25"], src="C")
t("T8.28", "E8", "v0.30", "Capture several breakpoints into sibling frames", "M", "P2", "designos-interop/webcapture", "Playwright fixture",
  "Each breakpoint becomes a sibling frame", deps=["T8.27"], src="C")
t("T8.20a", "E8", "v0.34", "Run and render FigJam widgets on the QuickJS host", "L", "P3", "plugin host, widget renderer", "E8 harness",
  "A sample widget renders", deps=["T8.8"], ctx="Absorbs the duplicate draft E9-26 (FigJam widgets).", src="C")
t("T8.20b", "E8", "v0.34", "Add widget synced state, property menu and interaction (single user)", "M", "P3", "widget runtime", "E8 harness",
  "A sample widget keeps its synced state across reload|Property menu actions work", deps=["T8.20a"], src="C")
t("T8.20c", "E8", "v0.34", "Import and export widget nodes through .fig", "M", "P3", "fig.rs, kiwi.rs, fig_export.rs", ".fig round trip",
  "widgetSyncedState and widgetMetadata round-trip", deps=["T8.20b", "X1"], src="C")
t("T8.21", "E8", "v0.34", "Let the agent scaffold and run generative plugins", "M", "P3", "agent tooling, designos-mcp", "end-to-end test",
  "The agent creates and runs a plugin from a prompt with no manual steps", deps=["T8.11", "T7.13"], src="C")
t("T8.29", "E8", "v0.35", "Copy from Studio to Figma via the fig-kiwi HTML clipboard payload", "M", "P2", "fig_export.rs, clipboard.rs", "Figma.app paste screenshot",
  "The payload pastes into Figma.app with layers intact", src="C")
t("T8.30", "E8", "v0.35", "Import .jam and .deck files", "M", "P2", "designos-library store.rs probe, fig.rs", "fixtures",
  "A .jam opens as a board and a .deck as a deck|Board and slide nodes map to native types",
  deps=["E9-1", "E9-12", "E9-18", "X1"], ctx="Absorbs the duplicate drafts E9-6 (.deck) and E9-22 (.jam).", rea=".deck and .jam containers.", src="C+D")
t("T8.31", "E8", "v0.35", "Paste from Sketch and Illustrator clipboards", "M", "P3", "app/src/clipboard.rs, designos-interop", "clipboard fixtures",
  "Sketch and Illustrator clipboard flavours paste as native layers", deps=["T8.22"], src="C")

# ================================================================ E9 FigJam & Slides
t("E9-1", "E9", "v0.9", "Type slide fields and round-trip Figma Slides through .fig", "M", "P1", "node.rs, fig.rs, fig_export.rs", ".fig round trip",
  "SLIDE 32, SLIDE_GRID 37, SLIDE_ROW 38, fields 389/410/443/426 and transitions round-trip|Old decks load", deps=["X1"], src="B")
t("E9-2", "E9", "v0.9", "Add 26 slide transitions with picker and Present playback", "M", "P1", "decks/ui.rs, present.rs, proto/geom.rs", "shell test + screenshot",
  "Push-left over 300 ms plays on Next|Choosing a transition is one undo step|Figma's 26 types map onto TransitionKind (1:1 for PPTX)",
  deps=["E9-1"], ui=True, rea="TransitionType 0-25 names.", src="B")
t("E9-3", "E9", "v0.9", "Skip slides in Present and PDF with a dimmed thumbnail", "S", "P1", "present.rs, decks/export.rs", "unit tests",
  "Next jumps over a skipped slide|The PDF excludes it", deps=["E9-1"], ui=True, src="B")
t("E9-4", "E9", "v0.9", "Number slides (field 443 plus a number layer)", "S", "P1", "decks/", "unit tests",
  "Numbers renumber on reorder and skip skipped slides, as in Figma", deps=["E9-1"], src="B")
t("E9-5", "E9", "v0.9", "Add presenter view: notes, next slide, timer, second display", "M", "P1", "present.rs", "shell test + screenshot",
  "Notes show for the current slide|The audience window has no notes|If it already exists, close with a test proving it", ui=True, ctx="Verify first: present.rs has no presenter-view code (cross-check FACT).", src="B")
t("E9-27", "E9", "v0.9", "Spotlight as presenter-follow in Present", "S", "P3", "present.rs", "shell test + screenshot",
  "The audience window follows the presenter's slide and zoom/pan focus within 1 frame|Toggle on/off|No network", deps=["E9-5"], ui=True,
  rul="R4.15: solo variant, spotlight as presenter-follow.", src="D")
t("E9-7", "E9", "v0.14", "Add slide object animations with click order", "M", "P1", "decks/, present.rs", "player tests",
  "Click order is respected|OUT runs before the transition|FADE/SLIDE_FROM_* IN/OUT", deps=["X2", "E9-1"], src="B")
t("E9-8", "E9", "v0.14", "Add sections to the slide grid", "M", "P1", "decks/", "shell test",
  "Moving a section moves its slides in one undo step", deps=["E9-1"], ui=True, rea="Slide-section model.", src="B")
t("E9-9", "E9", "v0.14", "Play interactive prototype flows inside slides", "M", "P2", "present.rs, proto/player.rs", "player tests",
  "A hotspot on a slide navigates in-slide|Next still advances", deps=["E9-1"], src="B")
t("E9-10", "E9", "v0.14", "Add templates for boards and decks: save, browse, insert, bundled Figma set", "M", "P1", "home/, designos-library", "library tests + screenshot",
  "Works offline|Inserts with fresh IDs|The bundled set is Figma's templates, each with an X4 provenance row",
  deps=["X4"], ui=True, rul="R5.20: copy Figma's templates, behind the X4 licence gate.", src="B+D")
t("E9-11", "E9", "v0.14", "Apply slide themes and libraries in design mode", "M", "P2", "styles.rs, decks/", "unit tests",
  "A theme change restyles every slide|Undo reverts it", deps=["E9-10"], src="B")
t("E9-12", "E9", "v0.20", "Model, render and round-trip FigJam tables with merged cells", "L", "P1", "node.rs, board.rs, render, fig.rs", "doc + render + .fig round trip",
  "Merged 2x2 cells render as one|TABLE 29, TABLE_CELL 30, fields 308-311, 538 round-trip", deps=["X1"], rea="Table cell -> row/col binding field.", src="B")
t("E9-13", "E9", "v0.20", "Add the table editing UI", "L", "P1", "board/*", "shell test + screenshot",
  "Tab moves between cells|Add/remove/reorder/resize and merge/unmerge are one undo step each", deps=["E9-12"], ui=True, src="B")
t("E9-14", "E9", "v0.23", "Paste Mermaid flowcharts as shapes and connectors", "M", "P1", "clipboard.rs, board/create.rs", "parser golden tests",
  "A-->B-->C gives 3 shapes and 2 connectors|Invalid input changes nothing", src="B")
t("E9-15", "E9", "v0.23", "Support every Mermaid diagram type Figma supports", "L", "P2", "clipboard.rs, board/create.rs", "golden per type",
  "Each type renders as editable native layers", deps=["E9-14"], rea="Which Mermaid types Figma supports.", src="B")
t("E9-16", "E9", "v0.23", "Turn Mermaid erDiagram into ERD entity tables", "M", "P2", "board", "golden",
  "Entities become tables with attached relationship connectors", deps=["E9-12", "E9-15"], src="B")
t("E9-17", "E9", "v0.23", "Add the generate_diagram MCP tool", "S", "P2", "designos-mcp", "MCP test",
  "The tool call creates the same layers as a paste", deps=["E9-14"], src="B")
t("E9-18", "E9", "v0.31", "Model mind maps with auto layout and .fig round trip", "L", "P1", "designos-doc, board, fig.rs", "layout tests + round trip",
  "Adding a child re-lays out its siblings|Round trip keeps the mind map", deps=["X1"], rea="Mind map model.", src="B")
t("E9-19", "E9", "v0.31", "Add the mind map UI: Tab/Enter, collapse, branch colours", "M", "P1", "board/*", "shell test + screenshot",
  "Keyboard-only creation works|Collapse persists", deps=["E9-18"], ui=True, src="B")
t("E9-20", "E9", "v0.31", "Add quick create from hover arrows", "M", "P2", "board/*", "shell test",
  "The new shape is auto-connected|If it already exists, close with a test proving it", ui=True, ctx="Verify first.", src="B")
t("E9-21", "E9", "v0.31", "Add a local facilitator timer to boards", "S", "P3", "board/toolbar.rs", "shell test",
  "The timer runs while the canvas is in use", ui=True, src="B")
t("E9-28", "E9", "v0.31", "Tally votes locally on boards", "M", "P3", "board/*", "shell test + screenshot",
  "Start a vote, place vote dots on stickies, see the tally|Results insert as a sticky or table|No network", ui=True,
  rul="R4.15: solo variant, local vote tallies.", src="D")
t("E9-23", "E9", "v0.32", "Run the agent panel and summaries in board and deck modes", "M", "P2", "terminal.rs, panels/rail.rs", "shell test",
  "The selection is sent to the user's agent CLI as context|The summary inserts as a sticky|No model bundled", ui=True,
  rul="R4.13: AI summaries and the agent in FigJam/Slides run through the agent CLI.", src="B+D")
t("E9-24", "E9", "v0.35", "Import Lucid boards", "L", "P3", "designos-interop importer", "fixtures",
  "Shapes and connectors map to native items|Unsupported items are counted", rul="R6.24: Lucid and Mural; Jamboard dropped.", src="B")
t("E9-25", "E9", "v0.35", "Import Mural boards", "L", "P3", "designos-interop importer", "fixtures",
  "Stickies map to native stickies|Unsupported items are counted", rul="R6.24.", src="B")

# ================================================================ E10 Versions & Branching
t("T10.1", "E10", "v0.13", "Store branch metadata and branch copies inside a file", "M", "P2", "designos-library store.rs FileEntry, versions.rs", "library tests",
  "Creating a branch saves a Branch point version plus a copy with the same NodeIds|Legacy indexes load",
  rul="R5.19: branches live inside the file.", src="C")
t("T10.2", "E10", "v0.13", "Build a three-way diff engine for nodes, styles, variables and components", "L", "P2", "new designos-doc/src/branch_diff.rs", "pure fixtures",
  "Add, remove, move and property changes classified correctly|Conflicts reported per layer only when both sides changed the same property|723k-node diff within budget off-thread [A]",
  rul="R5.18: conflicts resolved per layer.", src="C")
t("T10.3", "E10", "v0.13", "Merge branches with ID remap and per-layer conflict resolution", "L", "P2", "designos-doc, designos-library", "merge fixtures",
  "Every reference resolves after remapping (instances, prototype links, style refs, variable bindings, comment anchors)|The chosen side applies per layer|Versions exist before and after; one undo reverts",
  deps=["T10.1", "T10.2"], rul="R5.18 per layer; R5.19 merged branch archived, read-only, restorable.", src="C")
t("T10.4", "E10", "v0.13", "Add the branch UI: title-bar switcher, create, rename, archive, Home count", "M", "P2", "designos-app", "shell test + screenshot",
  "Switching never dirties the document|Archived branches show read-only and restorable|Home shows the branch count",
  deps=["T10.1"], ui=True, rul="R5.19.", src="C")
t("T10.5", "E10", "v0.13", "Add the review-before-merge screen", "M", "P2", "designos-app", "shell test + screenshot",
  "Before/after thumbnails and a conflict list|Merge stays disabled until every conflict is resolved", deps=["T10.2", "T10.4"], ui=True, src="C")
t("T10.6", "E10", "v0.13", "Show branch checkpoints in version history", "S", "P2", "app/src/history/*", "shell test",
  "Created, merged and updated entries appear", deps=["T10.1"], src="C")
t("T10.7", "E10", "v0.13", "Update a branch from main", "M", "P2", "designos-doc, designos-library", "merge fixtures",
  "Main changes flow into the branch with per-layer conflicts|One undo reverts", deps=["T10.3"], src="C")
t("T10.8", "E10", "v0.13", "Compare any two versions", "S", "P2", "app/src/history/*", "shell test",
  "Any two versions show their diff", deps=["T10.2"], src="C")
t("T10.14", "E10", "v0.13", "Give comment authors a stable identity, display name and colour", "S", "P2", "designos-doc/src/comments.rs", "migration test",
  "Legacy author strings migrate|Identity is stable per machine", src="C")
t("T10.9", "E10", "v0.13", "Add a local review request state to branches", "M", "P2", "designos-library, designos-app", "shell test",
  "In review / Approved / Changes requested persist with reviewer name|The merge screen shows the state; comments anchor to changes",
  deps=["T10.5", "T10.14"], rul="R4.15: async channel (branch review).", src="C")
t("T10.10", "E10", "v0.13", "Merge comment threads across branches", "S", "P2", "comments.rs, merge", "merge fixtures",
  "Branch comments survive merge with remapped anchors", deps=["T10.3"], rul="R4.15: async channel (comment merge).", src="C")
t("T10.11", "E10", "v0.33", "Add designos merge-driver and designos diff CLIs", "M", "P2", "app/src/cli.rs", "temp git repo test",
  "A clean merge exits 0|A conflict exits non-zero with a report and never corrupts the file", deps=["T10.2", "T10.3"], rul="R4.15: git merge driver.", src="C")
t("T10.12", "E10", "v0.33", "Install .gitattributes for the merge driver and document it", "S", "P2", "cli.rs, docs", "CLI test",
  "The installer writes the attributes idempotently|Docs show the setup", deps=["T10.11"], src="C")
t("T10.13", "E10", "v0.33", "Support a shared library root on a synced folder with conflict detection", "M", "P2", "designos-library store.rs", "library tests",
  "Conflict copies (iCloud, Dropbox, NAS) are surfaced, never silently overwritten|A lock file guards concurrent writes", rul="R4.15: synced-folder library root.", src="C")
t("T10.15", "E10", "v0.33", "Add @mentions, reactions and author filters to comments", "M", "P2", "comments.rs, comments UI", "shell test + screenshot",
  "Mentions resolve to identities|Reactions persist|Filter by author works", deps=["T10.14"], ui=True, src="C")
t("T10.16", "E10", "v0.33", "Nest folders 10 deep with folder previews", "M", "P3", "designos-library store.rs, Home", "library + shell test",
  "Depth 10 enforced with a clear error at 11|Previews render", ui=True, ctx="Folder.parent already exists (store.rs:26-30).", src="C")
t("T10.17", "E10", "v0.33", "Add tab groups", "M", "P3", "designos-app tabs.rs", "shell test + screenshot",
  "Tabs group, collapse and persist across restart", ui=True, src="C")

# ================================================================ E11 Prototyping
R_AI = "R4.13: all AI goes through the user's agent CLI; no bundled model."
R_D111 = "R8.27 (D11.1): prototypes export with our own WASM build of designos-render + designos-proto, pixel-identical to Present."
R_D112 = "R8.28 (D11.2): one self-contained .html file; above about 100 MB the user can choose a folder instead."
t("E11-1", "E11", "v0.36", "REA study: prototype fields in Figma 126.9.13", "S", "P1",
  "plans/reports only (no product code)", "findings doc",
  "Every field (media trigger types and fields, Navigate reset flags, STICKY_SCROLLS, prototypeDevice rotation/model, GIF playback on canvas, hotspot hints, makeContentState) has an id and type|Anything unknown is labelled UNKNOWN",
  rea="All listed prototype fields in 126.9.13; note the build may predate some.", src="E")
t("E11-2", "E11", "v0.36", "Move the prototype player and smart animate into a new egui-free crate designos-proto", "M", "P1",
  "designos-app proto/*, present.rs, new crates/designos-proto", "existing player tests",
  "cargo tree -p designos-proto shows no egui|All player tests pass unchanged|Present screenshots match before/after|Easing comes from designos-doc (X2); designos-proto adds no easing copy",
  deps=["X2"], ctx="Player is proto/player.rs (2,591 lines); Present is present.rs (981 lines). The same crate later drives Present, the WASM web player (E11-12) and the iOS player (E14-13). Easing already moved to designos-doc by X2, so it is not moved again here.", src="E")
t("E11-3", "E11", "v0.36", "Model and .fig mapping for media triggers, Navigate resets, sticky scroll, device rotation/model and starting point", "M", "P1",
  "designos-doc prototype.rs, settings.rs, designos-fig proto.rs, fig.rs", "round trip",
  "A Figma fixture round-trips every new field|Old .designos files load|Sticky stays distinct from fixed",
  deps=["E11-1", "X1"], ctx="proto.rs:216 collapses sticky into fixed; unknown interaction types incl. media triggers are dropped (proto.rs:245); prototypeStartingPoint is opaque (fig.rs:730); prototypeDevice is not imported.", src="E")
t("E11-4", "E11", "v0.36", "Run sticky scroll, Navigate resets and media triggers in the player", "M", "P1",
  "designos-proto", "player unit tests",
  "'Video ends' fires once per play-through|Reset scroll restores 0,0|Reset video and reset component state behave per E11-1",
  deps=["E11-2", "E11-3"], src="E")
t("E11-5", "E11", "v0.29", "Play video in Present straight to a texture, with accurate seek and loop", "M", "P1",
  "designos-app video.rs, present.rs", "perf probe",
  "1080p30 H.264 plays at 30 fps or more on the reference Mac|No encode_png call in the frame path|The default build has no openh264|WebM/VP9/AV1 play where the OS supports them; others show the existing 'unsupported codec' badge",
  deps=["E5-4a"], rul="R3.11: OS codecs only.",
  ctx="Today every frame is encoded to PNG and written into doc.images (video.rs:612-616). Present stays on CPU rendering; this only removes the PNG round trip. Took over 'Present plays it' from the old E5-4.", src="E")
t("E11-6", "E11", "v0.29", "Add a shared audio output service for video audio and the mute actions", "M", "P1",
  "new shared media audio module (designos-app), player.rs mute ops", "A/V sync test",
  "A/V drift at most 1 frame over 60 s|Mute, Unmute and Toggle behave correctly|The service is a reusable API that E6-30 timeline audio consumes",
  deps=["E11-5", "X3"], rul="R3.11: OS audio (AudioToolbox on macOS, Media Foundation on Windows); cpal on Linux behind X3.",
  ctx="No audio output exists: the player only tracks a muted flag (player.rs:201) and the app has no audio crate.", src="E")
t("E11-7", "E11", "v0.36", "Play animated GIFs in prototypes with their own frame delays and loop count", "M", "P1",
  "canvas/interact.rs, designos-doc prototype.rs, fig.rs", "decode + round trip",
  "Frame timing matches the file within 1 frame|Editor and exports show the poster frame|The GIF rejection test (interact.rs:2255) is inverted|.fig round trip keeps the animation",
  deps=["E11-1", "E11-3"], ctx="GIF is rejected on placement today (interact.rs:1829). Reuses the VideoFill sidecar pattern (prototype.rs:213) so renderers stay untouched.", src="E")
t("E11-8", "E11", "v0.37", "Add Present zoom modes, hotspot hints, rotation and a current device catalogue", "M", "P1",
  "present.rs, proto/mod.rs", "shell test + golden screenshots",
  "Each zoom mode (fit/fill/100%/fit-width) matches a golden screenshot|A miss-click flashes every reactive area for at most 500 ms|Rotation swaps the device frame|The device list reuses E2-6's preset list (one source); bezels are added here",
  deps=["E11-3", "E2-6"], ui=True,
  ctx="Present always fits (stage_layout, present.rs:131); devices are a hard-coded iPhone 15 era list of 8 (proto/mod.rs:36-45). Device presets belong to E2 (E2-6), so this task consumes that list instead of keeping a second one.", src="E")
t("E11-9", "E11", "v0.37", "Comment on prototypes in Present, sharing the canvas comment threads", "M", "P1",
  "comments/*, designos-doc comments.rs, present.rs", "doc + shell test",
  "A comment made in Present shows on the canvas on the same node|Resolving works from either side|It survives save|Threads have a 'Prototype' filter",
  ui=True, ctx="Comments are pinned to a page or node only (comments.rs:9-13); Present has no comment code.", src="E")
t("E11-10", "E11", "v0.37", "AI 'Add interactions' through the agent CLI with a diff preview", "M", "P1",
  "cmdk.rs, terminal.rs, specs.rs", "patch parsing with a recorded fixture",
  "The reactions patch schema is validated and a bad destination is rejected with a named error|Applying the patch is one undo step|No bundled model (cargo tree)|The rule-based auto-wire stays as the labelled offline fallback",
  deps=["E11-11"], ui=True, rul=R_AI, ctx="'Make prototype' is a rule-based auto-wire today (cmdk.rs:1066).", src="E+D")
t("E11-11", "E11", "v0.37", "Add MCP tools to read and write reactions, flows and the device", "S", "P1",
  "designos-mcp", "MCP tests",
  "A write round-trips through the document|The tools are added to the T7.6 parity list and tool snapshot",
  rul="R1.4: every tool and option exposed.", src="E")
t("E11-12", "E11", "v0.38", "Build designos-render and designos-proto for WASM", "L", "P1",
  "designos-render, designos-fonts, designos-proto Cargo features", "wasm test + pixel diff",
  "Fixture frames match the native CPU render within the documented tolerance|rayon and system fonts sit behind features; fonts come embedded from the document|The .wasm size stays within a budget recorded in the PR",
  deps=["E11-2"], rul=R_D111, ctx="Risk: rayon without threads in WASM (INFERENCE: put it behind a feature). No cargo build has run for wasm yet.", src="E+D")
t("E11-13", "E11", "v0.38", "Export a prototype as one offline HTML file (CLI and MCP too)", "L", "P1",
  "new crates/designos-web-player, export menu, CLI, MCP", "scripted click path",
  "Opens from file:// in Safari, Chrome and Firefox (screenshots)|The same clicks reach the same frames as Present|Zero network requests (CSP)|The default output is one self-contained .html; above ~100 MB the dialog offers a folder",
  deps=["E11-12"], ui=True, rul=R_D111 + " " + R_D112,
  ctx="Local equivalent of Figma's embed and share links (hosted services excluded, R4.14). Prototypes use the pixel-exact WASM player; sites (E13) use semantic HTML.", src="E+D")
t("E11-14", "E11", "v0.38", "Add a mobile view to the web player: touch, safe areas, rotation, offline install", "M", "P1",
  "designos-web-player", "device run",
  "Runs on a Z Fold 7 and iPhone Safari (screenshots)|Double-tap does not zoom the browser|A PWA manifest allows offline install",
  deps=["E11-13"], src="E")
t("E11-15", "E11", "v0.38", "Play video, GIF and audio in the web player in layer order", "M", "P2",
  "designos-web-player", "browser test",
  "Video draws in correct layer order under an overlay|Media triggers fire",
  deps=["E11-13", "E11-5", "E11-7"], ctx="Browser decoding; risk: video layer ordering over the WASM canvas.", src="E")
t("E11-16", "E11", "v0.47", "Run code layers live in Present and the HTML player, with a bridge to prototype actions", "L", "P2",
  "present.rs, designos-web-player", "shell + browser test",
  "A button inside a code layer triggers Navigate|Variables sync both ways through the postMessage bridge",
  deps=["E13-17", "T8.7"], rul="R9.33 (D13.2): code layers are React/TSX only. R1.3: embedded webview (wry/WKWebView).", src="E+D")
t("E11-17", "E11", "v0.47", "Import Figma Make embeds as code layers, otherwise keep them with a poster", "M", "P2",
  "fig.rs", "round trip",
  "Nothing is lost on round trip|Mapped embeds render their poster",
  deps=["E11-1", "E13-16"], ctx="makeContentState and codeSourceInfo are opaque passthrough today (fig.rs:731, :739).", src="E")
t("E11-18", "E11", "v0.36", "Add a prototype conformance suite over REA-captured Figma fixtures", "S", "P1",
  "designos-proto tests, CI", "CI",
  "Scripted runs produce the expected frame sequences for every fixture",
  deps=["E11-4"], src="E")

# ================================================================ E12 Canvas, accessibility & text
t("E12-1", "E12", "v0.39", "REA study: accessible canvas, contrast palette, smart symbols, ExportSettings, font upload, on-canvas video", "S", "P1",
  "plans/reports only (no product code)", "findings doc",
  "Every behaviour (canvas key map and announcements, enhanced-contrast palette, smart symbols, ExportSettings fields, font-upload model, on-canvas video controls) has evidence or is labelled UNKNOWN",
  rea="All listed behaviours in Figma 126.9.13.", src="E")
t("E12-2", "E12", "v0.39", "Switch the theme from compile-time constants to a runtime palette with a contrast mode", "L", "P1",
  "theme.rs and every UI file using it (96 files), design/design.tokens.json", "token parity test + screenshot diff",
  "Zero pixel diff on 5 surfaces in standard mode|The theme.rs to design.tokens.json parity test passes|Lands as one scripted PR in an announced quiet window",
  ui=True, ctx="theme.rs:22-31 constants are referenced 2,895 times across 96 files. Risk: conflicts with every open UI branch, so all earlier-pack UI work merges first.", src="E")
t("E12-3", "E12", "v0.39", "Add enhanced-contrast mode: manual toggle that also follows the OS setting", "M", "P1",
  "theme.rs, prefs", "ui ds a11y gate + screenshots",
  "Text at least 4.5:1 and UI at least 3:1 everywhere|Turning on the OS 'Increase contrast' setting switches the mode with no user action|The manual toggle overrides the OS setting and persists",
  deps=["E12-2"], ui=True, rul="R8.29 (D12.1): a manual toggle that also follows the OS 'Increase contrast' setting.", src="E+D")
t("E12-4", "E12", "v0.39", "Give every custom widget accessibility info through the shared widget helpers", "L", "P1",
  "designos-app widgets/*", "AccessKit tree snapshot",
  "Every control in the properties panel has a role and label|VoiceOver pass (screen recording)",
  deps=["E12-2"], ui=True, ctx="AccessKit is compiled in (accesskit_winit in Cargo.lock) but called zero times. The record() helper is the single attach point (INFERENCE).", src="E")
t("E12-5", "E12", "v0.39", "Expose a canvas accessibility tree: visible layers, selection path, focus, announcements", "L", "P1",
  "canvas/*", "tree test + perf probe",
  "At most 1 ms per frame on the 723k-node page|No rev bump and no to_json (CLAUDE.md section 4)",
  deps=["E12-4"], src="E")
t("E12-6", "E12", "v0.39", "Control the canvas fully from the keyboard: focus regions, create and resize, shortcut reference", "M", "P1",
  "shell.rs", "shell tests",
  "A rectangle can be created and resized without a pointer|F6 cycles focus through 4 regions|The shortcut reference lists every binding",
  deps=["E12-1", "E12-2"], ui=True, ctx="Enter / Shift+Enter / Tab hierarchy navigation is done (shell.rs:710-745).", src="E")
t("E12-7", "E12", "v0.40", "Pick colours anywhere on screen with the eyedropper", "M", "P1",
  "panels/properties.rs, new platform colour-sampler module", "per-OS smoke test",
  "Picks a colour from another app (screenshot)|The value is stored in the document's colour space (Display P3 per E1-7)|Esc cancels|NSColorSampler on macOS, desktop pixel read on Windows, xdg-desktop-portal PickColor on Linux; the canvas sampler stays as fallback",
  deps=["E1-7"], ui=True, ctx="Canvas-only today (properties.rs:1579-1587).", src="E")
t("E12-8", "E12", "v0.40", "Play video on the canvas through a dynamic GPU texture slot", "L", "P1",
  "canvas/gpu.rs, designos-gpu", "perf probe + headless suite",
  "1080p plays on the 723k-node page|Idle refine is unaffected|CPU and headless paths show the poster and stay bit-exact|No FrameKey or rev change per video frame",
  deps=["E5-4a", "E11-5"], ui=True, ctx="The canvas shows only the poster by design today (video.rs:1-7). An indirect texture slot honours CLAUDE.md section 4.1 and 4.4.", src="E")
t("E12-9", "E12", "v0.40", "Store large video inside the file and stream it instead of holding it in memory (300 MB)", "L", "P1",
  "doc images, io.rs, fig.rs", "RSS probe + round trip",
  "Opening a 300 MB video adds less than 50 MB RSS|Save/load round trip|doc_hash cost unchanged",
  rul="R9.31 (D12.3): large video stored inside the file and streamed, not loaded into RAM.", src="E+D")
t("E12-10", "E12", "v0.41", "Import fonts into a user font library and embed them in documents when the licence allows", "M", "P1",
  "designos-fonts, fonts.rs, save format", "font tests",
  "TTF/OTF/WOFF/WOFF2 render immediately and after restart|A restricted font (OS/2 fsType) refuses embedding with a named reason|An embedded font renders on a machine without it",
  deps=["X3"], rul="R8.30 (D12.2): embed when the font licence allows it (OS/2 fsType); forbidden fonts are refused with a named reason.",
  ctx="No way to import a font file today. WOFF2 needs brotli; its licence goes through X3.", src="E+D")
t("E12-11", "E12", "v0.41", "Add a team font folder on the synced-folder root", "S", "P2",
  "designos-fonts", "unit",
  "A font added to the folder appears without restart",
  deps=["E12-10", "T10.13"], rul="R4.15: synced-folder library root as the async channel.", src="E+D")
t("E12-12", "E12", "v0.41", "Use Apple system fonts on macOS with optical sizes", "M", "P1",
  "fonts/subst.rs, render text.rs", "glyph comparison",
  "macOS renders the system SF Pro/Compact/Mono/New York|The Linux CI result is unchanged|Other OSes keep the substitution with a missing indicator",
  deps=["E12-14"], ctx="SF Pro and other Apple families are always swapped for Inter (fonts/subst.rs:5-11).", src="E")
t("E12-13", "E12", "v0.41", "Support every variable-font axis and named instance", "L", "P1",
  "typography.rs, render text.rs, export css.rs, fig.rs", "pixel diff + round trip",
  "Changing opsz or a custom axis changes the outlines|CPU and GPU stay bit-exact|CSS emits font-variation-settings|.fig fontVariations round-trip",
  ui=True, ctx="UI offers 4 fixed axes (typography.rs:839-877); renderer uses wght, wdth (9 buckets), ital, slnt only (text.rs:405-423). Risk: cosmic-text may not take arbitrary axes (ASSUMPTION); fallback is ttf-parser outlines.", src="E")
t("E12-14", "E12", "v0.41", "Add a missing-fonts panel with document-wide replace", "M", "P1",
  "panels, fonts.rs", "shell test",
  "Lists every missing font with affected layer count|One replace is one undo step across all runs|Affected layers show a badge",
  ui=True, src="E")
t("E12-15", "E12", "v0.41", "Add smart symbols per the REA findings", "S", "P2",
  "textedit.rs", "unit",
  "Matches the REA cases from E12-1",
  deps=["E12-1"], ctx="Behaviour UNKNOWN until E12-1 (assumed: auto-substitution of typed sequences, low confidence).", src="E")
t("E12-16", "E12", "v0.41", "Add text decoration style, thickness, offset and colour", "M", "P2",
  "paint.rs, render, export, fig.rs", "pixel diff + round trip",
  "A wavy underline is identical on CPU and GPU|SVG/CSS emit the decoration|.fig round trip",
  ui=True, ctx="Decoration is none/underline/strikethrough only (paint.rs:403).", src="E")
t("E12-17", "E12", "v0.41", "Audit text parity against the feature catalog and REA", "S", "P2",
  "plans/reports only", "audit report",
  "Every finding becomes a filed task|Findings already owned elsewhere are linked, not refiled: text wrap Balance/Pretty (E2, done), text on path (E4-25/E4-26), text animation (E6-18)",
  deps=["E12-1"], ctx="Scope: font picker previews, forced small caps, sub/superscript and any other gap.", src="E")
t("E12-18", "E12", "v0.42", "Add the export settings popover with .fig ExportSettings round trip", "M", "P1",
  "panels/export.rs, settings.rs, fig.rs", "round trip + output snapshots",
  ".fig export settings survive a round trip|Every toggle (outline text, include id, simplify stroke, colour profile, JPG quality, contents only) changes the output (snapshot)",
  deps=["E12-1"], ui=True, ctx="Custom suffix is done (export.rs:84-92); outline text exists in svg.rs:27 with no UI. Vector PDF is not duplicated here: it stays #40 (v0.6), linked from this task.", src="E")
t("E12-19", "E12", "v0.42", "Embed sRGB / Display P3 ICC profiles in PNG and JPG exports", "M", "P1",
  "designos-export raster.rs", "ColorSync check",
  "macOS Preview shows Display P3 on a P3 export (screenshot)|sRGB exports stay bit-exact",
  deps=["E1-7"], src="E")
t("E12-20", "E12", "v0.42", "Simplify stroke and include id in SVG export", "M", "P2",
  "designos-export svg.rs", "golden SVG",
  "Matches the REA sample from E12-1",
  deps=["E12-1", "E12-18"], src="E")
t("E12-21", "E12", "v0.42", "Add the bulk export dialog (Cmd+Shift+E)", "M", "P2",
  "panels", "shell test + screenshot",
  "Lists every exportable layer|Exports the chosen subset",
  ui=True, ctx="No bulk export dialog found (grep, ASSUMPTION).", src="E")
t("E12-22", "E12", "v0.42", "Add tab muting and an agent terminal in its own window", "M", "P3",
  "tabs.rs, terminal.rs", "shell test",
  "A muted tab stays muted across restart|The agent window can be detached and reattached (egui multi-viewport)",
  deps=["T10.17"], ui=True, ctx="Tab groups are already T10.17 (v0.33); this task builds on them and does not re-implement groups.", src="E+C")
t("E12-23", "E12", "v0.42", "Show layer-type labels in the Layers panel", "S", "P3",
  "panels/layers.rs", "screenshot",
  "The label shows for each node type|The virtualised Layers panel keeps its frame budget",
  ui=True, src="E")

# ================================================================ E13 Sites & code layers
R_D131 = "R9.32 (D13.1): sites are their own file type in Home."
R_D132 = "R9.33 (D13.2): code layers are React/TSX only."
R_D133 = "R9.34 (D13.3): zero JS by default in exported sites; small scripts only where an interaction needs them."
R_PORT = "Coordinator ruling 2026-10-17: the preview server is a runtime product feature on 127.0.0.1 with an OS-assigned port and a token; the dev-server fixed-port contract does not apply."
t("E13-1", "E13", "v0.43", "REA study: Sites and code-layer fields in Figma 126.9.13", "M", "P1",
  "plans/reports only (no product code)", "findings doc",
  "Every field (editor type, responsive sets/breakpoints and overrides, page settings, scroll effects, CMS, codeSourceInfo, makeContentState) has an id or is labelled UNKNOWN",
  rea="Sites fields. Code layers shipped 2026-10-15, after build 126.9.13: design our own, then align (same pattern as R3.12).", src="E")
t("E13-2", "E13", "v0.43", "Add the site file type: Home 'New site', site toolbar with pages and breakpoint bar", "L", "P1",
  "home/, shell.rs, designos-doc, designos-library file kind", "shell test + screenshot",
  "A site is created, saved and reopened from Home|Design files never show site mode|The library file-kind probe recognises sites",
  ui=True, rul=R_D131, src="E+D")
t("E13-3", "E13", "v0.43", "Model breakpoints: per-breakpoint overrides on top of the primary frame", "XL", "P1",
  "designos-doc node.rs, fig.rs", "doc tests",
  "A primary edit reaches breakpoints with no override|An override sticks|Each edit is one undo step|Old files load",
  deps=["E13-1", "E13-2"], ctx="Risk: touches the instance-override code paths.", src="E")
t("E13-4", "E13", "v0.43", "Show breakpoints side by side with a drag-width reflow preview", "L", "P1",
  "canvas/*", "perf probe + screenshot",
  "The reflow drag stays within the 8 ms budget|CLAUDE.md section 4 invariants hold|Override markers and reset work",
  deps=["E13-3"], ui=True, src="E")
t("E13-5", "E13", "v0.43", "Add the page settings panel: slug, SEO, social image, favicon, noindex", "M", "P1",
  "panels", "round trip",
  "Every field round-trips and appears in the exported <head>",
  deps=["E13-2"], ui=True, src="E")
t("E13-6", "E13", "v0.44", "Compile a site to semantic HTML and CSS with breakpoints as media queries", "XL", "P1",
  "designos-export site/", "browser pixel diff + html-validate",
  "375/768/1440 renders match the Studio render within threshold (screenshots)|html-validate passes|Static layout works with no JS|Variables become CSS properties, styles become classes, effects and inline SVG are emitted",
  deps=["T7.29", "T8.7", "E13-3"], rul=R_D133, ctx="Reuses React/CSS/Tailwind codegen (react.rs:24, css.rs:9, tailwind.rs:12), variables to CSS (codegen.rs:158). Needs T7.29 HTML codegen and T8.7 webview host first.", src="E+D")
t("E13-7", "E13", "v0.44", "Optimise site assets: WebP + srcset, self-hosted WOFF2 when allowed, SVG optimisation", "M", "P1",
  "designos-export site/", "unit",
  "A restricted font falls back to a CSS font stack with a warning|Images ship as WebP with srcset|SVGs are optimised",
  deps=["E13-6", "E12-10", "E6-11"], rul="R8.30 (D12.2): fonts are embedded only when the licence allows.", ctx="WebP encoding is E6-11; not re-implemented here.", src="E+D")
t("E13-8", "E13", "v0.44", "Export a site as a static folder (menu, CLI, MCP)", "M", "P1",
  "io.rs, cli.rs, designos-mcp", "hash test",
  "Re-exporting gives byte-identical output|Works from file://|Sitemap, robots, 404, relative links and OG tags are present",
  deps=["E13-6"], rul="R4.14: hosted features (custom domains, password, analytics) are excluded; publish = a local static folder.", src="E+D")
t("E13-9", "E13", "v0.44", "Preview a site locally with live reload", "M", "P1",
  "plugins runtime loopback pattern (plugins/runtime.rs:34-75), site preview module", "integration test",
  "An edit shows in the browser within 1 s|The server stops with the window|It binds 127.0.0.1 on an OS-assigned port and rejects requests without the token|Reload runs off the UI thread",
  deps=["E13-8"], rul=R_PORT, src="E+D")
t("E13-10", "E13", "v0.45", "Export site interactions: hover/press states, links, View Transitions, sticky", "L", "P1",
  "designos-export site/", "browser test",
  "Hover state matches the variant (screenshot)|Interactions work with JS disabled where CSS can express them; any script is per-interaction and small",
  deps=["E13-6", "E11-3"], rul=R_D133, src="E+D")
t("E13-11", "E13", "v0.45", "Add scroll effects: appear, parallax, marquee, scroll-linked", "L", "P2",
  "designos-doc, designos-export site/", "browser test",
  "prefers-reduced-motion turns effects off|The editor previews each effect",
  deps=["E13-1", "E13-6"], rul=R_D133, src="E+D")
t("E13-12", "E13", "v0.45", "Export video backgrounds: poster plus muted autoplay loop", "M", "P2",
  "designos-export site/", "browser test",
  "Plays in Safari and Chrome|The poster shows before playback and with video disabled",
  deps=["E5-4a", "E13-6"], src="E")
t("E13-13", "E13", "v0.46", "Add the block library, the 50 templates and 'Save as block/template'", "L", "P2",
  "assets, home/", "screenshot",
  "Passes the X4 licence gate (every template has a provenance row)|Save as block/template works",
  deps=["X4", "E13-2"], ui=True, rul="R5.20: copied Figma templates behind the X4 licence-review gate.", src="E+D")
t("E13-14", "E13", "v0.46", "Model CMS collections, typed fields and items with CSV in/out", "L", "P2",
  "designos-doc", "round trip",
  "A CSV of 1k rows round-trips",
  deps=["E13-2"], ctx="CMS content is baked in at export time (hosted services excluded, R4.14).", src="E")
t("E13-15", "E13", "v0.46", "Bind CMS to list layers, detail templates and slug pages", "XL", "P2",
  "designos-doc, designos-export site/", "export test",
  "One page per item|The editor previews real items",
  deps=["E13-14", "E13-6"], ui=True, src="E")
t("E13-16", "E13", "v0.47", "Model code layers (source files, entry, props schema, poster, sizing) with .fig codeSourceInfo", "L", "P1",
  "designos-doc node.rs, fig.rs", "round trip",
  "Lossless .designos and .fig round trip|Old files load|Only React/TSX sources are accepted; others are refused with a named reason",
  deps=["E13-1"], rul=R_D132, src="E+D")
t("E13-17", "E13", "v0.47", "Render code layers: offscreen webview snapshot as canvas poster, live in Present", "XL", "P1",
  "canvas, present.rs, webview host", "perf probe",
  "The poster arrives without a rev bump (async, cached, not a doc mutation)|CLAUDE.md section 4 invariants hold",
  deps=["T8.7", "E13-16", "E13-18"], ctx="Needs E13-18 because the runtime executes transpiled TSX. Risk: Linux snapshots depend on WebKitGTK.", src="E")
t("E13-18", "E13", "v0.47", "Transpile TSX to JS in-process and ship React ESM with the app", "L", "P1",
  "new transpile crate (swc or oxc)", "unit",
  "The fixture component runs with no network|Passes the X3 licence gate|No npm at runtime",
  deps=["X3"], rul=R_D132, src="E+D")
t("E13-19", "E13", "v0.47", "Add the code editor panel and a props inspector with variable binding", "L", "P1",
  "panels", "shell test + screenshot",
  "A prop edit re-renders the poster|Runtime errors show in the panel|Props come from the props schema and bind to variables",
  deps=["E13-17"], ui=True, src="E")
t("E13-20", "E13", "v0.47", "Create a code layer from a frame, a repo file or a prompt", "L", "P1",
  "cmdk.rs, designos-mcp", "integration test",
  "A repo edit syncs into the document|'Edit in code' is shown for repo-backed layers|Prompt creation goes through the agent CLI",
  deps=["E13-19", "T7.15", "T7.20"], rul="R6.21: repo files win, the document keeps a synced copy. " + R_AI, src="E+D")
t("E13-21", "E13", "v0.47", "Sandbox code layers: CSP, allowedDomains with consent, whitelisted bridge messages", "M", "P1",
  "code-layer runtime", "security test",
  "fetch to a domain not on the list is blocked|Allowed domains need a one-time consent|Unknown bridge messages are dropped with a named log entry",
  deps=["T8.15", "E13-17"], rul="R5.17: only allowedDomains with one-time consent.", src="E+D")
t("E13-22", "E13", "v0.47", "Ship code layers in exports: live on sites, imports in React codegen, labelled poster elsewhere", "M", "P1",
  "designos-export", "browser + golden",
  "A live component runs on the exported site|React codegen imports the component|PNG/SVG/PDF use the poster with a label",
  deps=["E13-6", "E13-17"], rul=R_D133 + " A live code layer is an interaction that needs JS.", src="E+D")
t("E13-23", "E13", "v0.47", "Add MCP tools for sites, pages, breakpoints, CMS, code layers and site export", "M", "P1",
  "designos-mcp", "MCP tests",
  "Each CRUD tool round-trips through the document|Tools are added to the T7.6 parity list",
  deps=["E13-8", "E13-16"], rul="R1.4: every tool and option exposed.", src="E+D")
t("E13-24", "E13", "v0.47", "Generate a section or site through the agent CLI and MCP", "M", "P2",
  "cmdk.rs", "recorded fixture",
  "A recorded agent response becomes site layers in one undo step|No bundled model (cargo tree)",
  deps=["E13-23"], rul=R_AI, src="E+D")

# ================================================================ E14 iPad / mobile app (Later, Backlog)
OPEN_D141 = "OPEN owner decision D14.1 (deferred, R10.35): distribution - (a) App Store (recommended; plugins must run in WebKit), (b) TestFlight only, (c) EU sideload only."
OPEN_D142 = "OPEN owner decision D14.2 (deferred, R10.35): AI on iPad - (a) relay to the paired desktop agent runtime (recommended; keeps R4.13), (b) disable AI on iPad."
OPEN_D143 = "OPEN owner decision D14.3 (deferred, R10.35): native platforms - (a) iPadOS + iPhone as one universal app (recommended; Android gets the LAN web player), (b) add a native Android app now."
BL = "Backlog"
t("E14-1", "E14", "Later", "Spike the iPad host: thin UIKit + egui-wgpu on CAMetalLayer vs eframe/winit", "M", "P1",
  "spike branch only", "report + recording",
  "Go/no-go table with numbers: 723k-node fixture fps and RSS, Pencil, IME, background/foreground|A document-size ceiling that iOS memory limits allow",
  rul=OPEN_D143, ctx="No iOS target today (packaging/ has linux, macos, windows). Recommended approach C (UIKit host + egui-wgpu); A (eframe) is the fallback. ASSUMPTION: eframe/egui-wgpu run on iOS (low/medium).", status=BL, src="E+D")
t("E14-2", "E14", "Later", "Add a platform trait for file dialogs, clipboard, PTY, updater, open-URL, fonts and notifications", "L", "P1",
  "io.rs, clipboard.rs, terminal.rs, updater.rs", "CI target check",
  "cargo check --target aarch64-apple-ios passes|Desktop behaviour unchanged",
  deps=["E14-1"], ctx="portable-pty (Cargo.toml:51) cannot spawn processes on iOS; rfd and arboard have no iOS backend (INFERENCE).", status=BL, src="E")
t("E14-3", "E14", "Later", "Load fonts and caches on iOS", "S", "P1",
  "designos-fonts", "device test",
  "SF and bundled fonts render|Caches live in the app container",
  deps=["E14-2"], status=BL, src="E")
t("E14-4", "E14", "Later", "Build the Xcode host, Rust xcframework, signing and a CI .ipa", "M", "P1",
  "packaging/ios", "CI",
  "A signed build installs on the device",
  deps=["E14-2"], rul=OPEN_D143, status=BL, src="E+D")
t("E14-5", "E14", "Later", "Bridge touch, Apple Pencil, hardware keyboard and trackpad input", "L", "P1",
  "iOS host + shell input", "device test",
  "Pencil pressure, tilt, azimuth, hover and squeeze reach the E4 brush engine|Pinch zoom runs at 120 fps with no rev bump|UIKeyCommand shortcuts work",
  deps=["E14-4"], status=BL, src="E")
t("E14-6", "E14", "Later", "Bridge UITextInput IME into canvas text editing", "L", "P1",
  "iOS host, textsession.rs", "device test",
  "Vietnamese Telex and Japanese compose correctly",
  deps=["E14-4"], status=BL, src="E")
t("E14-7", "E14", "Later", "Add the Files/iCloud document browser, background autosave and share sheet", "M", "P1",
  "iOS host, io", "device test",
  "No data loss when the app is killed in the background",
  deps=["E14-4"], status=BL, src="E")
t("E14-8", "E14", "Later", "Handle iOS lifecycle and memory: surface rebuild, cache drops, per-device VRAM refine budget", "M", "P1",
  "canvas/gpu.rs", "device probe",
  "Survives 10 background/foreground cycles|The per-device refine VRAM budget applies (CLAUDE.md section 4)",
  deps=["E14-4"], status=BL, src="E")
t("E14-9", "E14", "Later", "Add a touch layout profile with 44 pt targets and a touch token mode", "XL", "P1",
  "theme, panels", "curator rubric + screenshots",
  "Passes the curator rubric at 11 and 13 inch, portrait and landscape",
  deps=["E12-2", "E14-5"], ui=True, ctx="Desktop UI uses 24 px targets (design/principles.md).", status=BL, src="E")
t("E14-10", "E14", "Later", "Make every desktop action reachable on iPad", "L", "P1",
  "panels", "generated checklist",
  "100% of actions in the menus.rs action list are reachable",
  deps=["E14-9"], ui=True, status=BL, src="E")
t("E14-11", "E14", "Later", "AI on iPad through a relay to the paired desktop agent runtime", "L", "P2",
  "iOS host, terminal", "integration test",
  "'Add interactions' works through the relay|With no relay, AI commands are disabled with a named reason",
  deps=["E11-10"], rul=OPEN_D142 + " " + R_AI, ctx="iPadOS cannot run the agent CLI. Do not start until D14.2 is answered.", status=BL, src="E+D")
t("E14-12", "E14", "Later", "Preview a prototype on a phone from the desktop over LAN with a QR code", "M", "P1",
  "designos-web-player, LAN server", "device test",
  "The Z Fold 7 scans the QR code and the prototype runs|The mirror follows the selection within 500 ms|Stopping revokes the token; the server auto-stops",
  deps=["E11-13", "E11-14"], rul=OPEN_D143 + " Server: one-time token, user-started only, auto-stop.",
  ctx="Same runtime-server shape as E13-9 (OS-assigned port + token); the coordinator's port ruling named only the Sites preview, so confirm it covers this server too.", status=BL, src="E+D")
t("E14-13", "E14", "Later", "Native prototype player on iPhone and iPad", "L", "P1",
  "iOS host", "device test",
  "Matches the desktop Present click path|Video plays via AVFoundation|Prototype comments work",
  deps=["E14-4", "E11-2", "E11-9", "E5-4a"], rul=OPEN_D143, status=BL, src="E+D")
t("E14-14", "E14", "Later", "Find the desktop with Bonjour and mirror natively", "M", "P2",
  "iOS host", "device test",
  "Finds the desktop with no manual IP",
  deps=["E14-12"], status=BL, src="E")
t("E14-15", "E14", "Later", "Set up distribution: App Store/TestFlight pipeline, privacy manifest, export compliance", "M", "P1",
  "packaging/ios", "review checklist",
  "Passes TestFlight processing",
  deps=["E14-4"], rul=OPEN_D141, ctx="Do not start until D14.1 is answered.", status=BL, src="E+D")
t("E14-16", "E14", "Later", "Run plugins on iPad through WKWebView/JavaScriptCore instead of QuickJS", "M", "P2",
  "plugin host", "device test",
  "A plugin from the T8.32 corpus runs",
  deps=["T8.5", "E14-4"], rul=OPEN_D141 + " App Store rule 2.5.2 applies if (a).", status=BL, src="E+D")

# ================================================================ new epics (no issue yet; the filer creates them)
NEW_EPICS = [
    ("E11", "[Epic] E11 Prototyping",
     "Full scope: plans/261017-epics-full-scope/full-scope-e11-e12-e13-e14.md (E11).\n\n"
     "### Scope\nMedia triggers, Navigate resets, sticky scroll, device rotation/model and their .fig mapping; an egui-free `designos-proto` crate; video in Present without the PNG step and a shared audio output service; animated GIF; Present zoom modes, hotspot hints and a current device catalogue; comments on prototypes; AI 'Add interactions' through the agent CLI; MCP prototype tools; offline HTML prototype export with a WASM player, mobile view and media; live code layers and Make embeds in prototypes.\n\n"
     "### Owner rulings\n- R8.27 (D11.1): our own WASM build of designos-render + designos-proto, pixel-identical to Present.\n- R8.28 (D11.2): one self-contained .html file; folder option above about 100 MB.\n- R4.13: AI only through the agent CLI. R3.11: OS codecs. R4.14: hosted embed/share links become local HTML export.\n\n"
     "### Packs\nv0.29 (E11-5, E11-6), v0.36 Prototype 2.0, v0.37 Present prototypes, v0.38 Share prototypes as one file, v0.47 Code layers (E11-16, E11-17).\n\n"
     "### Cross-epic\nConsumes E5-4a (OS video decode), X1, X2 (easing stays in designos-doc), E2-6 (device presets), T8.7 (webview), E13-16/E13-17 (code layers). E6-30 uses the E11-6 audio service."),
    ("E12", "[Epic] E12 Canvas, accessibility & text",
     "Full scope: plans/261017-epics-full-scope/full-scope-e11-e12-e13-e14.md (E12).\n\n"
     "### Scope\nRuntime theme palette and enhanced contrast; AccessKit info on every widget and a canvas accessibility tree; full keyboard canvas control; screen-wide eyedropper; video playback on the canvas and large media streamed from the file; font import/embedding, team font folder, Apple fonts, missing-fonts panel, full variable-font axes, smart symbols, decoration style; export settings popover, ICC profiles, SVG simplify/id, bulk export; tab muting, detached agent window, layer-type labels.\n\n"
     "### Owner rulings\n- R8.29 (D12.1): contrast mode = manual toggle that also follows the OS 'Increase contrast' setting.\n- R8.30 (D12.2): embed fonts when the licence allows (OS/2 fsType); refuse others with a named reason.\n- R9.31 (D12.3): large video stored inside the file and streamed, not loaded into RAM.\n\n"
     "### Packs\nv0.39 Accessible Studio, v0.40 Video on the canvas, v0.41 Fonts & type, v0.42 Export settings.\n\n"
     "### Not here (owned elsewhere)\nText wrap Balance/Pretty (E2, done), text on path (E4-25/26), text animation (E6-18), WebP (E6-11), easing (X2), tab groups (T10.17), vector PDF (#40, linked from E12-18)."),
    ("E13", "[Epic] E13 Sites & code layers",
     "Full scope: plans/261017-epics-full-scope/full-scope-e11-e12-e13-e14.md (E13).\n\n"
     "### Scope\nA local Figma Sites: site file type, pages and page settings, breakpoints with overrides, responsive compile to semantic HTML/CSS, asset pipeline, static-folder export, local preview, interactions, scroll effects, video backgrounds, blocks and 50 templates, CMS with CSV; code layers (model, webview runtime, offline TSX transpile, editor and props, create from frame/repo/prompt, sandbox, exports, MCP tools, agent generation). Custom domains, password protection and analytics are hosted features and excluded (R4.14).\n\n"
     "### Owner rulings\n- R9.32 (D13.1): sites are their own file type in Home.\n- R9.33 (D13.2): code layers are React/TSX only.\n- R9.34 (D13.3): zero JS by default; small scripts only where an interaction needs them.\n- Coordinator: the preview server is a runtime feature (127.0.0.1, OS-assigned port + token); the fixed-port contract does not apply.\n- R5.20 + X4: templates behind the licence gate. R5.17: code-layer network = allowedDomains + consent. R6.21: repo files win.\n\n"
     "### Packs\nv0.43 Sites, v0.44 Publish a static site, v0.45 Sites in motion, v0.46 Site content, v0.47 Code layers.\n\n"
     "### Cross-epic\nE13-6 needs T7.29 (HTML codegen) and T8.7 (webview host). E13-17 also feeds E11-16."),
    ("E14", "[Epic] E14 iPad / mobile app",
     "Full scope: plans/261017-epics-full-scope/full-scope-e11-e12-e13-e14.md (E14).\n\n"
     "**Status: Backlog, pack Later** (owner R10.35, 2026-10-17: \"Đưa vào backlog đã\").\n\n"
     "### Scope\niPad editor with full parity: touch, Apple Pencil, keyboard, trackpad, IME, Files/iCloud, lifecycle/memory, touch layout, AI, plugins; prototype preview on phones (LAN web player, native player, mirror); distribution. Recommended approach C (thin UIKit host + egui-wgpu on CAMetalLayer), decided by the E14-1 spike.\n\n"
     "### Open owner decisions (deferred)\n- " + OPEN_D141 + "\n- " + OPEN_D142 + "\n- " + OPEN_D143 + "\n\n"
     "### Cross-epic\nReuses designos-proto (E11-2), the web player (E11-13/14), E5-4a, E12-2 (runtime theme) and T8.5."),
]

# ================================================================ amendments
AMEND = [
    (25, "Owner-ruled full scope (2026-10-17): also map `gridAutoTracks` (fig.rs:729, metadata-only today) to the model and back. Acceptance: (1) auto-track sizes round-trip through .fig; (2) REA confirms the gridAutoTracks string format. HOT FILE: position {#25} in the single hot-file queue."),
    (26, "Owner ruling R2.5 (D-E2-1, 2026-10-17): NEW frames default to auto layout version 1 (CSS-aligned, border inside the box). Imported frames keep their own version. Acceptance add: (1) a frame created in Studio has version 1; (2) an imported version-0 frame stays version 0 until 'Update layout'. HOT FILE: position {#26}."),
    (31, "Hot-file queue (2026-10-17): the PR #49 review fixes (H1, L1, M1) are position {#31}, the first v0.5 hot-file merge. Follow-ups are filed as #107-#112; slot reference file is the owner's VSF - PCP.fig, verified by E3-15 (never committed)."),
    (37, "Full-scope audit (2026-10-17): acceptance must say the round trip is through `.fig` kiwi (`convert_effect` fig.rs:1849 + `encode_effect` fig_export.rs:1640), that effect styles carry progressive blur, and record the offset units (REA E5-a). HOT FILE: position {#37}, after #38."),
    (38, "Full-scope audit (2026-10-17): acceptance must name the `.fig` kiwi round trip for splay and the noise/texture fields, and add independent noise/texture X/Y size (`size: f32` scalar today, paint.rs:328). The base data loss is fixed by #106; this issue builds on it. Fixture: NOISE/GRAIN/GLASS keep every field through .fig; effect styles carry them. REA E5-b (noise X/Y fields, PaintType NOISE=9 vs NOISE effect). HOT FILE: position {#38}, after #106 merges."),
    (39, "Owner ruling R2.8 (2026-10-17): custom shaders are user-authored WGSL (E5-5/E5-6, pack v0.24). Figma shader source is server-side, so imported Figma shaders stay preserve-only here. May use X1 typed passthrough if it lands first. HOT FILE: position {#39}."),
    (107, "Owner ruling R2.6 (2026-10-17): use the real `preferredValues` shape found in the owner's `VSF - PCP.fig` once E3-15 verifies it contains slot data. Never commit that file; tests use the synthetic fixture from E3-15."),
    (110, "Owner ruling R2.6 (2026-10-17): the Figma.app open check uses the owner's `VSF - PCP.fig` as the slot reference, after E3-15 confirms it carries slot data (isSlot=463 / isSlotContent=495 / SLOT props). If it has none, E3-15 reports to the owner first. Never commit the file."),
    (43, "Blocks T7.36 (MCP create_new_file, pack v0.7 Agent design context): create_new_file must create a library file without replacing the open document."),
    (75, "Blocks T7.5 (selection default for MCP tools, pack v0.7 Agent design context, ruling R1.4). Proposed: move to milestone v0.7."),
    (76, "Blocks T7.5 (pack v0.7 Agent design context, ruling R1.4). Proposed: move to milestone v0.7."),
    (77, "Blocks T7.5 (pack v0.7 Agent design context, ruling R1.4). Proposed: move to milestone v0.7."),
    (78, "Blocks T7.5 (pack v0.7 Agent design context, ruling R1.4). Proposed: move to milestone v0.7."),
    (80, "Blocks T7.5 (pack v0.7 Agent design context, ruling R1.4): two Studio instances sharing tab-1 breaks a per-session selection default. Proposed: move to milestone v0.7."),
    (81, "Blocks T7.5 (pack v0.7 Agent design context, ruling R1.4). Proposed: move to milestone v0.7."),
    (14, "Owner rulings 2026-10-17: Display P3 per document + OKLCH/OKLAB (R3.9); variables JSON in Figma-native and W3C DTCG (R3.10). E1-10 animation styles and E1-11 easing/timing variables absorb the duplicate E6 drafts (E6-19 model half, E6-20) and ship in v0.17 Motion toolkit. Main E1 pack: v0.8 Variables everywhere."),
    (15, "Owner ruling R2.5: new frames use auto layout version 1; imported frames keep theirs. New tasks E2-1..E2-6 join v0.4 Layout 2.0; device presets (iPhone 17) belong to E2."),
    (16, "Owner rulings: slot reference file = owner's VSF - PCP.fig, verify first, never commit (R2.6, task E3-15); Check designs number suggestions are exact matches only (R2.7); component playground is built (E3-14)."),
    (17, "Owner rulings: copy Figma's 25 built-in brushes behind the X4 licence-review gate (R5.20); vectorize goes through the agent CLI, no bundled tracer (R4.13). X1 typed kiwi passthrough is filed here as first consumer."),
    (18, "Owner rulings: custom shaders = user-authored WGSL, GPU render, CPU/headless via cached GPU readback, excluded from bit-exact suite (R2.8); video fills via OS codecs (R3.11), split into E5-4a (shared OS video decode, also used by E11/E12/E13/E14) and E5-4b (video fill model and round trip); 3D = REA of Spline/Rive + 126.9.13 schema, our own implementation, align at Figma GA (R3.12, E5-7/8/9); AI image tools through the agent CLI, no bundled model (R4.13, E5-10)."),
    (19, "Owner rulings: MP4 via OS encoders, Linux AV1 + Opus (R3.11); MPL-2.0 symphonia allowed via X3 (R4.16); React motion export = Figma's flavour after REA, CSS-in-JSX interim (R6.23). E6-20 merged into E1-11; E6-19 now covers library publish/consume only. E6-30 timeline audio plays through the shared E11-6 audio output service."),
    (20, "Owner rulings: MCP defaults to the editor selection when attached, configurable per user/session (selection / require node_id / page), every tool and option exposed (R1.4); Code Connect repo files win (R6.21); hosted services become local equivalents incl. Slack (R4.14). Added MCP tools: whoami, create_new_file, upload_assets, get_libraries, get_context_for_code_connect, create_design_system_rules, 5 shader tools."),
    (21, "Owner rulings: plugin corpus = 8 plugins proposed by the coordinator, owner-approved (R1.1, T8.32); import from manifest (R1.2); embedded webview with browser fallback (R1.3); network = manifest allowedDomains + one-time consent (R5.17); hosted APIs stubbed as single local user, free (R4.14); web capture stays (R6.22). E8 owns widgets (T8.20a-c) and .jam/.deck import (T8.30)."),
    (22, "Owner rulings: multiplayer-only features become async channels + solo variants (spotlight as presenter-follow E9-27, local vote tallies E9-28) (R4.15); templates copied from Figma behind X4 (R5.20); Lucid + Mural import, Jamboard dropped (R6.24); agent/summaries via agent CLI (R4.13). Duplicates removed: E9-6/E9-22 -> E8 T8.30, E9-26 widgets -> E8 T8.20a-c."),
    (23, "Owner rulings: conflicts resolved per layer (R5.18); branches live inside the file with a title-bar switcher and Home count, merged branches archived read-only and restorable (R5.19); multiplayer replaced by async channels: branch review, git merge driver, synced-folder library root, comment merge (R4.15)."),
]

# ---------------------------------------------------------------- body builder
def fmt_deps(ds):
    return ", ".join(ds) if ds else "none"


def body(x):
    hp = HOTPOS.get(x["id"])
    ep_ref = f"#{EPIC_NO[x['epic']]}" if x["epic"] in EPIC_NO else "epic issue not filed yet"
    lines = [f"**Epic:** {x['epic']} ({ep_ref}) | **Pack:** {x['pack']} | **Size/Prio:** {x['size']} / {x['prio']}"]
    if x["status"]:
        lines += [f"**Status:** {x['status']} (owner ruling R10.35)"]
    lines += [""]
    if x["ctx"]:
        lines += ["### Context", x["ctx"], ""]
    lines += ["### Acceptance criteria"] + [f"{i}. {a.strip()}" for i, a in enumerate(x["ac"].split("|"), 1)]
    lines += ["", "### Test seam", x["seam"]]
    if x["ui"]:
        lines += ["Load es-designer before UI code; desktop capture at 1440 goes in the PR (responsive: SKIPPED, owner default)."]
    lines += ["", "### Touches", x["touches"]]
    if hp:
        lines += [f"HOT FILE: merge position {hp} in the single hot-file queue (fig.rs / fig_export.rs / kiwi.rs / rest_import.rs). Claim only after position {hp-1} merged." if hp > 1 else "HOT FILE: merge position 1 in the single hot-file queue."]
    lines += ["", "### Deps", fmt_deps(x["deps"])]
    if x["rul"]:
        lines += ["", "### Owner rulings", x["rul"]]
    if x["rea"]:
        lines += ["", "### REA needs", x["rea"] + " Use rea (morluto/rea) per the global reverse-engineering rule."]
    srcs = [SRC[s] for s in x["src"].split("+")]
    lines += ["", "### Source", "; ".join(srcs)]
    lines += ["", "Visual evidence (screenshot or terminal capture) is required in the PR."]
    return "\n".join(lines)


ids = [x["id"] for x in TASKS]
dups = [i for i, c in collections.Counter(ids).items() if c > 1]
assert not dups, dups
known = set(ids)
for x in TASKS:
    for d in x["deps"]:
        assert d.startswith("#") or d in known, (x["id"], d)
for h in HOT:
    assert h.startswith("#") or h in known, h

out = {
    "packs": [{"title": p[0], "description": p[1]} for p in PACKS],
    "tasks": [dict({"id": x["id"], "epic": x["epic"], "title": x["title"], "body": body(x), "prio": x["prio"], "size": x["size"],
                    "touches": x["touches"], "pack": x["pack"], "deps": x["deps"]}, **({"status": x["status"]} if x["status"] else {}))
              for x in TASKS],
    "amend": [{"issue": n, "add_text": re.sub(r"\{(#\d+)\}", lambda m: str(HOTPOS[m.group(1)]), s)} for n, s in AMEND],
    "hot_file_queue": HOT,
    "new_epics": [{"code": c, "title": ti, "body": b} for c, ti, b in NEW_EPICS],
}
json.dump(out, open(OUT, "w"), indent=1, ensure_ascii=False)

# ---------------------------------------------------------------- report
ep = collections.Counter(x["epic"] for x in TASKS)
pk = collections.Counter(x["pack"] for x in TASKS)
print("TOTAL", len(TASKS), "AMEND", len(AMEND))
print("EPIC", dict(sorted(ep.items(), key=lambda kv: int(kv[0][1:]))))
for p in PACKS:
    print(f"{pk.get(p[0],0):3d}  {p[0]}")
# pack order sanity: a dep must be in same or earlier pack
order = {p[0]: i for i, p in enumerate(PACKS)}
pmap = {x["id"]: x["pack"] for x in TASKS}
bad = [(x["id"], d, pmap[d]) for x in TASKS for d in x["deps"] if d in pmap and order[pmap[d]] > order[x["pack"]]]
for b in bad:
    print("PACK-ORDER VIOLATION", *b)
# hot queue monotonic by pack
last, bad_hot = -1, []
for h in HOT:
    if h in pmap:
        o = order[pmap[h]]
        if o < last:
            bad_hot.append(h)
            print("HOT ORDER NOT PACK-MONOTONIC", h)
        last = max(last, o)
# every 🔥 fig.rs task in E11-E13 is in the queue; every new-epic code has a task; no empty pack
hot_new = [x["id"] for x in TASKS if x["epic"] in ("E11", "E12", "E13") and "fig.rs" in x["touches"]]
missing_hot = [i for i in hot_new if i not in HOTPOS]
empty = [p[0] for p in PACKS if not any(x["pack"] == p[0] for x in TASKS)]
assert {c for c, _, _ in NEW_EPICS} <= {x["epic"] for x in TASKS}
assert not bad and not bad_hot and not missing_hot and not empty, (bad, bad_hot, missing_hot, empty)
print("CHECKS OK: no dup ids, deps resolve, no forward-pack deps, hot queue pack-monotonic,", len(HOT), "hot entries")
