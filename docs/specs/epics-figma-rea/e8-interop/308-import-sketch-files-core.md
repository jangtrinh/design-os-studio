# #308 Import Sketch files: pages, artboards, layers, shapes, booleans, text, images

| Field | Value |
|---|---|
| Issue | #308 (task T8.22, L / P2; epic #21; milestone v0.26 Sketch import). Owner kept Sketch import in the 2026-10-30 roadmap trim (`plans/261030-1700-roadmap-trim/decisions.md`, row "Sketch, Lucid/Mural import: Keep both"). |
| Code baseline | `origin/main@d22dad970` |
| Verdict | **Still real.** No Sketch code in the workspace (`grep -rli sketch crates/*/src`: 0 files; the 2026-10-17 plan records "[F] No Sketch or PPTX code", `full-scope-e7-e8-e10.md:277`). |
| Size | L: 20–28 h builder |
| Tier | Claude Sonnet (new crate, geometry and text mapping judgment, many layer classes) |
| Touches | new crate `crates/designos-interop` with module `sketch/` (decided in `full-scope-e7-e8-e10.md:322`: "A new `designos-interop` crate (`pptx/`, `sketch/`, `webcapture/`)"; web capture already lives in its own crate `designos-webcapture` and stays there); `Cargo.toml` workspace members; `designos-library` (route `ImportKind::Sketch` from #307); `crates/designos-app/src/io.rs` (same). |
| Depends on | #307 (probe). Owner answer Q-308 (fixture source). |

## 1. Format facts (FACT, source cited; nothing else assumed)

Source A: developer.sketch.com/file-format (fetched 2026-11-06). Source B: `github.com/sketch-hq/sketch-file-format`, `packages/file-format/schema/*.schema.yaml` (HEAD, release tag `@sketch-hq/sketch-file@1.1.5`, fetched 2026-11-06).

- Container: "Sketch documents are stored as ZIP archives containing JSON encoded data" (A). Entries: `document.json` (shared styles, page refs), `meta.json`, `user.json`, `pages/<id>.json` (one per page), `images/`, `previews/preview.png` (A).
- Format introduced in Sketch 43 (A). `meta.json` `version` enum 121–146, `compatibilityVersion` const 99 (B `meta.schema.yaml`).
- `document.json` (B `abstract-document.schema.yaml`): `pages` (file refs), `layerStyles`, `layerTextStyles`, `layerSymbols`, `sharedSwatches`, `foreignSymbols`, `foreignLayerStyles`, `foreignTextStyles`, `foreignSwatches`, `fontReferences`, `colorSpace`.
- Layer classes present as schema files (B `layers/`): `artboard`, `bitmap`, `group`, `oval`, `page`, `polygon`, `rectangle`, `shape-group`, `shape-path`, `slice`, `star`, `symbol-instance`, `symbol-master`, `text`, `triangle`, `hotspot`. Booleans: `boolean-operation` enum (B).
- Text: `attributedString { string, attributes: [stringAttribute { location, length, attributes { MSAttributedStringFontAttribute (font-descriptor), MSAttributedStringColorAttribute, kerning, paragraphStyle, textStyleVerticalAlignmentKey } }] }` (B `objects/attributed-string`, `objects/string-attribute`).
- Everything else (geometry encoding such as `curve-point` / `point-string`, `frame` rect, style `fills` / `borders` / `shadows` / `blur`, `resizingConstraint`, `exportOptions`) → builder reads the schema file for each before mapping. **No mapping detail in this spec is to be taken as a format fact unless it cites B.** Mark anything else UNKNOWN in code comments and the import report.

## 2. User-visible outcome

A designer with an old Sketch file drops it on Home (or File > Open). It opens as a Studio design file: same pages and artboard names, layers in the same order with the same names, shapes, boolean groups, text with its fonts and colours, and images. Anything Studio cannot bring across is listed in an import summary instead of silently missing.

## 3. Mapping (decided; each row cites the schema file it rests on)

| Sketch `_class` (B) | Studio (`NodeKind`, `crates/designos-doc/src/node.rs`) |
|---|---|
| `page` | `Page` |
| `artboard` | `Frame` (clips content; background colour if `hasBackgroundColor`) |
| `group` | `Group` |
| `rectangle` | `Rectangle` (corner radius from its points / `fixedRadius`, per schema) |
| `oval` | `Ellipse` |
| `polygon`, `star`, `triangle` | `Polygon` / `Star` when parameters map 1:1, else `Vector` from points |
| `shapePath` | `Vector` |
| `shapeGroup` with child `booleanOperation` | `BooleanOp` (union / subtract / intersect / exclude as the enum allows) |
| `text` | `Text` with runs from `attributedString` |
| `bitmap` | `Rectangle` with an image fill from `images/` |
| `symbolMaster` / `symbolInstance` | **#309.** In this issue: instance imported as a detached copy of its master's layers (picture right), counted as "symbols detached" |
| `slice`, `hotspot` | skipped, counted |

Styles per layer: fills, borders → strokes, shadows / inner shadows, blur, opacity, blend mode — map what has a Studio field; count the rest by name in the report.

## 4. Behaviour rules (decided; technical)

- R1 Pure function `designos_interop::sketch::import(bytes) -> Result<(Document, SketchImportReport), SketchError>`; no I/O beyond the byte slice; ZIP read under the same inflate budget pattern as `designos-fig/src/container.rs` (entry cap, per-read cap, ratio cap).
- R2 Report: counts per unsupported class / property, fonts referenced but not available, images missing. Shown with the existing import warning path (`append_import_warning`, `io.rs`).
- R3 Coordinates: Sketch frames are parent-relative (INFERENCE from schema `frame: rect`; builder confirms with a fixture whose child is offset in a moved group and renders it).
- R4 Supported versions: `meta.version` 121–146 import; > 146 import with a warning "made with a newer Sketch"; non-ZIP (< Sketch 43) handled by #307's message.
- R5 Text: font descriptor `name` (PostScript name) → family + weight via `designos-fonts` lookup; unknown → keep family string, weight 400, count "font substituted".

## 5. Acceptance criteria (red then green)

- **AC-1** Fixture set (see Q-308) imports; per page, layer count and the ordered list of layer names equal the source's (from the fixture JSON, computed by the test, not hand-typed).
- **AC-2** Every unsupported class/property in the fixtures appears in the report with a count; the test asserts the exact counts.
- **AC-3** Boolean fixture (union, subtract, intersect, difference) renders within 0.5 % mismatched pixels of `previews/preview.png` when the fixture carries one (real-file fixtures only; synthetic fixtures assert geometry numerically).
- **AC-4** Text fixture with two runs (different colour and weight) keeps both runs at the right ranges.
- **AC-5** Image layer shows its bitmap; a missing `images/` entry counts "image missing" and draws the placeholder.
- **AC-6** A ZIP bomb fixture (high ratio) is refused with an error, no panic.
- **AC-7** Visual evidence: Studio window with an imported fixture next to the Sketch `preview.png` (1440 capture).

## 6. Test steps

`cargo test -p designos-interop`; app open of each fixture through `io.rs`; screenshot.

## 7. Edge cases

- Nested groups with rotation and flips; masks (`hasClippingMask`) → Studio mask where the model has it, else counted.
- Text with `textBehaviour` fixed vs auto width → Studio text auto-resize mode.
- Shared style refs (`sharedStyleID`) → values applied, link left to #309.
- Huge files: `pages/*.json` parsed one page at a time.

## 8. Non-goals

Symbols as components, shared styles as styles, libraries (#309). Sketch export. Sketch clipboard (#321). Prototyping links (`flow`) → counted.

## 9. User-visible details not settled by the issue

- Fixture source / which real Sketch files define "done" → **Q-308**.
- Where the report shows: decided — the existing import warning toast + the same text in the file's import log; no new panel.

## 10. Rollback

New crate behind the probe route; revert the route commit to disable (files then show #307's "coming later" message).
