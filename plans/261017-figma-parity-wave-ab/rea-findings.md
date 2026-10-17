# REA findings (Figma desktop 126.9.13, local) — implementation facts

Method:
- Kiwi schema dumped from a real `.fig` (653 defs; `NodeChange` has 616 fields).
- Editor bundles taken from the local Electron cache, then static `rg` and REA.
- Field = `name = kiwi field id`.
- The full schema text is kept outside the repo at the controller's scratchpad, `figma-126-kiwi-schema.kiwi`.

## Clipboard (S1), FACT from `figma_app-df4fa2cfed8484d4.min.js`
- The writer puts two items on the clipboard, via `ClipboardItem`:
  - `text/plain`: the plain text of the selection.
  - `text/html`, assembled as:
    ```
    <meta charset="utf-8"><div><span data-metadata="<!--(figmeta)" + base64(utf8(JSON metadata)) + "(/figmeta)-->"></span><span data-buffer="<!--(figma)" + base64(buffer) + "(/figma)-->"></span></div><span style="white-space:pre-wrap;">TEXT</span>
    ```
- `buffer` = the wasm `encodeClipboardData(...)` output. Decoding goes `decodeMessage(decodeClipboardData(bytes))`.
- INFERENCE: the buffer is a fig-kiwi archive with an embedded schema, in the same container family as `.fig`. `designos-fig` `container::open` + kiwi decode are expected to read it. Verify with a synthesized fixture built from a real `.fig` archive.
- The reader also accepts the HTML-entity-escaped markers `&lt;!--(figmeta)` … `(/figmeta)--&gt;` and `(/figma)--&gt;`, because browsers and the macOS pasteboard may escape attribute content. Parse both forms.
- Message fields used for paste (`Message` def):
  - `nodeChanges`, `blobs`;
  - `pasteID`, `pasteFileKey`, `pastePageId`;
  - `pasteIsPartiallyOutsideEnclosingFrame`;
  - `pasteBranchSourceFileKey = 26`.
- metadata JSON, INFERENCE from public knowledge: `{ fileKey, pasteID, dataType: "scene" }`.
- Placement: not verified in Figma's code. Use the brainstorm assumption A2: center in the viewport, single undo.

## Shared styles (S2)
- `enum StyleType { NONE=0 FILL=1 STROKE=2 TEXT=3 EFFECT=4 EXPORT=5 GRID=6 ANIMATION=7 }`.
- A style is a normal `NodeChange` carrying these fields:
  - `styleType=163`, `styleDescription=191`, `key=319`, `version=171`, `sortPosition=320`;
  - `isSoftDeletedStyle=176`;
  - `sharedStyleMasterData=172`, `sharedStyleReference=173`.
- Values on the style node:
  - EFFECT uses `effects=43`;
  - GRID uses `layoutGrids=47`;
  - FILL and STROKE use `fillPaints=38`.
- Consumer refs (`StyleId { guid=1, assetRef=2 }`): `styleIdForFill=332`, `styleIdForStrokeFill=333`, `styleIdForText=334`, `styleIdForEffect=335`, `styleIdForGrid=336`.
- Legacy GUID refs: `inheritEffectStyleID=169`, `inheritGridStyleID=170`, `inheritFillStyleID=158`, `inheritStrokeStyleID=162`, `inheritTextStyleID=167`.
- Repo today:
  - `fig_export.rs:446` already exports EFFECT/GRID style nodes and refs.
  - `fig.rs:1003` maps the refs.
- Repo gaps:
  - `fig.rs:915/966` read `effects`/`layoutGrids` from the consumer's own copy, not from the style node (fills are swapped from the style node at `fig.rs:885-896`; mirror that).
  - Legacy `inherit*StyleID` refs are never read.
  - `isSoftDeletedStyle` is not filtered (`fig.rs:536` only checks `isSoftDeleted`).
  - `rest_import.rs:1128-1138` seeds `doc.styles` effect/grid from the first consumer.
  - The app `styles.rs:25` `style_entries` lists only Text/Fill.
  - The doc model already has `Style::{Effect, Grid}` (`designos-doc/src/lib.rs:731`).

## Auto layout (S4)
- `StackMode { NONE=0 HORIZONTAL=1 VERTICAL=2 GRID=3 }` = `stackMode=105`.
- `StackJustify { MIN=0 CENTER=1 MAX=2 SPACE_EVENLY=3 SPACE_BETWEEN=4 SPACE_AROUND=5 SPACE_EVENLY_CSS=6 }`.
  - Field `stackPrimaryAlignItems=230` (legacy field `stackJustify=121`).
  - The UI offers SPACE_BETWEEN, SPACE_EVENLY_CSS (label "Space evenly") and SPACE_AROUND.
  - A lint flags legacy `SPACE_EVENLY` with `stackLayoutVersion<1`.
  - Import maps both 3 and 6 to SpaceEvenly.
  - Export writes 6 when `layout_version >= 1`, else 3.
- `stackWrap=323 { NO_WRAP=0 WRAP=1 }`: wrap is valid for both HORIZONTAL and VERTICAL.
- `stackCounterSpacing=324` is the gap between wrapped lines.
- `stackCounterAlignContent=343 { AUTO=0 SPACE_BETWEEN=1 }`.
- `stackCounterAlignItems=231` uses `StackAlign { MIN CENTER MAX BASELINE }`.
- `stackChildAlignSelf=236`.
- `stackLayoutVersion=574 (uint)`: ≥1 is the 2026 CSS-aligned layout, where the border is always inside the box.
- Child margins: `stackChildMarginTop/Right/Bottom/Left=606..609`.
- Repo gaps:
  - `fig.rs:1350` folds SPACE_EVENLY into SpaceBetween.
  - `rest_import.rs:2223` only knows SpaceBetween.
  - `stackLayoutVersion`, `stackCounterAlignContent` and the margins are unread.
- Semantics, INFERENCE from the CSS naming: SPACE_AROUND = CSS `space-around` (each item gets equal space on both sides, so the edge gap is half the inner gap); SPACE_EVENLY_CSS = CSS `space-evenly` (all gaps equal, including the edges).

## Text wrap (S4)
- `textWrapStyle=594 { AUTO=0 BALANCE=1 PRETTY=2 }`, node-level.
- Semantics follow CSS `text-wrap: balance | pretty` (INFERENCE):
  - balance: minimize the variance of line widths within the same line count;
  - pretty: avoid a short or one-word last line.

## Variables (S5)
- `NodeChange.codeSyntax: CodeSyntaxMap = 358` sits on the VARIABLE node.
  - `CodeSyntaxMap { entries: CodeSyntaxMapEntry[] = 1 }`.
  - `CodeSyntaxMapEntry { platform = 1, value: string = 2 }`.
  - `CodeSyntaxPlatform { WEB=0 ANDROID=1 iOS=2 }`: note this order and the lowercase "i" in `iOS`.
- Composed opacity:
  - `Paint.opacityVar: VariableData = 38` sits beside `colorVar=21`.
  - Effects use `Effect.effectOpacityVar = 53` beside `colorVar=10`.
  - Scope `VariableScope.COLOR_OPACITY = 23` is used on FLOAT variables.
  - Resolve as `paint.opacity × float`. Multiply vs replace is not verified; use multiply and flag it.
- Repo gaps:
  - `variables.rs:182` writes an empty `codeSyntax`.
  - No `opacityVar` support anywhere.

## Codegen (S5)
- The R4 report on native Dev Mode languages is pending.
- Vue and HTML targets follow the existing `react.rs` / `css.rs` patterns.

## Later-wave field index
- Slots:
  - frame `isSlot=463`;
  - `isSlotContent=495`;
  - `ComponentPropType.SLOT=7`;
  - `SlotPropConfig{stretchChildOnInsert, displayByDefault, minChildren, maxChildren, allowPreferredValuesOnly}`;
  - `ComponentPropNodeField.SLOT_CONTENT_ID=4`.
- Variable-width stroke: `variableWidthPoints=447 {position, ascent, descent, segmentId}`.
- Brushes: `brushType=452`, `dynamicStrokeSettings=448`, `scatterStrokeSettings=449`, `stretchStrokeSettings=450`.
- Text on path: TEXT_PATH=41, `textPathStart=432 {tValue, forward}`.
- Repeat and symmetry: `transformModifiers=455` (REPEAT/SYMMETRY/SKEW; count, offset, LINEAR/RADIAL); TRANSFORM=48.
- Paint types:
  - `PaintType.PATTERN=8`, with spacing, tile type and alignment;
  - `NOISE=9`;
  - `CUSTOM=10` (a code-component shader).
- Progressive blur:
  - `Effect.blurOpType=19 {NORMAL, PROGRESSIVE}`;
  - `startOffset=20`, `endOffset=28`, `startRadius=21`, `radius=4`.
- Glass: `refractionRadius=30`, `specularAngle=31`, `specularIntensity=32`, `bevelSize=33`, `chromaticAberration=34`, `reflectionDistance=35`, `refractionIntensity=36`.
- Table: TABLE=29, TABLE_CELL=30; row/column positions 308/309; heights/widths 310/311; `tableMergedCells=538`.
- Slides: SLIDE=32; `slideSpeakerNotes=389`, `isSkippedSlide=410`; transitions reuse the prototype fields (`transitionType=154`, `transitionDuration=155`).
- Motion: KEYFRAME=56, KEYFRAME_TRACK=57; timeline fields 506–513, 552, 553.
- Extended collections: `VariableSetMode.parentVariableSetId=4`, `parentModeId=5`; `isCollectionExtendable=385`.
- Desktop MCP:
  - The shell `lazy/external/codegen_mcp_server.js` serves `127.0.0.1:<port>/mcp` (Streamable HTTP).
  - The tools are defined in editor chunks.
  - `figma_agent` is a font/URL helper, not MCP.

## R4: Dev Mode / Code Connect / MCP (FACT unless marked)
- **Native Dev Mode codegen languages:** WEB (CSS), IOS (SwiftUI), IOS_UIKIT (UIKit), ANDROID (Compose), ANDROID_XML (XML). Codegen plugins add more.
  - Vue, HTML, Svelte and Web Components are Code Connect labels only, NOT native codegen.
  - Tailwind exists only via MCP `format: react/tailwind`.
- **Language → codeSyntax platform:** WEB and CSSBUILDER → WEB; ANDROID and ANDROID_XML → ANDROID; IOS and IOS_UIKIT → iOS.
- **CSS printing of a bound variable:**
  - a `codeSyntax.WEB` string that contains `var(--` is printed verbatim, keeping the user fallback;
  - any other `codeSyntax.WEB` string is printed verbatim;
  - otherwise print `var(--<sanitize(name)>, <resolved value>)`;
  - a definition line is `--<name>: <value>`;
  - the MCP path prints `codeSyntax.WEB`, else `var(--<name with [^a-zA-Z0-9-] removed>)`.
- **Name sanitizer (OX):**
  1. Strip non-ASCII characters, quotes and newlines; trim.
  2. Replace runs of `[\s/]+` with `-`.
  3. If the result matches `^[-A-Za-z0-9_]*$`, keep it as is, case preserved.
  4. Otherwise convert camelCase to lower kebab-case and strip leading non-alphanumerics.
  5. An empty result becomes `unnamed`.
- **Dev status:**
  - `SectionStatus{NONE=0, BUILD=1 (Ready for dev), COMPLETED=2}`.
  - `sectionStatusInfo=355 {status, lastUpdateUnixTimestamp, description, userId, prevStatus}`, `handoffStatusMap=361`.
  - "Changed" is derived (status set + an edited-since flag), not stored.
- **Annotations:** `annotations=369 {label (Lexical|plain), properties[] (33-value AnnotationPropertyType), labelV2, categoryId}`, `measurements=384`, document `annotationCategories=453` (5 presets + 8 colors + custom).
- **Desktop MCP:**
  - `codegen_mcp_server.js` only proxies to the editor.
  - Local tools: `get_design_context`, `get_variable_defs`, `get_screenshot`, `get_metadata`, `get_figjam`, `get_motion_context` (gated), the code connect tools (get/add map, suggestions, send mappings), and `set_codegen_config`.
  - `get_design_context` returns code, hints and a PNG. Above 40k chars it falls back to sparse XML metadata.
- **Code Connect:** a server-side entity `{id, nodeId, fileKey, figmadoc, instanceFigmadocs, label, type, userRules}`. The resolved mapping is `{componentName, source, snippet, snippetImports, snippetOptions, version}`.
- **MCP parity gaps in studio, ranked:**
  1. `get_screenshot` returning image/png base64;
  2. `get_design_context` (code + hints + screenshot);
  3. XML `get_metadata`;
  4. `codeSyntax` in `get_variables`;
  5. Code Connect store + 4 tools.
