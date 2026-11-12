# #309 Sketch symbols become components (with overrides); shared styles become styles; library symbols come along

| Field | Value |
|---|---|
| Issue | #309 (task T8.23, L / P2; epic #21; milestone v0.26 Sketch import) |
| Code baseline | `origin/main@d22dad970` |
| Verdict | **Still real** (no Sketch code; depends on #308). |
| Size | L: 16–22 h builder |
| Tier | Claude Sonnet (component/override model mapping; sensitive `components.rs` semantics) |
| Touches | `crates/designos-interop/src/sketch/` (new, from #308): `symbols.rs`, `styles.rs`. Reads only from `designos-doc` component APIs; no edit to `crates/designos-doc/src/components.rs` expected (if needed, flag before coding — it is shared with open PRs #1673 / #1672). |
| Depends on | #308. Owner answers Q-309 (foreign library symbols). |

## 1. Format facts (FACT, source: `github.com/sketch-hq/sketch-file-format` `packages/file-format/schema/`, fetched 2026-11-06)

- `symbolInstance` (`layers/symbol-instance.schema.yaml`): `symbolID` (uuid), `overrideValues: [overrideValue]`, `scale`, `preservesSpaceWhenHidden`.
- `overrideValue` (`objects/override-value.schema.yaml`): `overrideName`, `value` = string | uuid | file-ref | data-ref.
- `overrideName` (`objects/override-name.schema.yaml`): pattern `<layer UUID>(/<layer UUID>)*` followed by one of `_stringValue`, `_symbolID`, `_image`, `_layerStyle`. So an override path is a chain of layer `do_objectID`s, and four override kinds exist: text, nested symbol swap, image, layer style.
- `document.json` (`abstract-document.schema.yaml`): `layerSymbols` (symbol container), `layerStyles` (`shared-style-container`), `layerTextStyles` (`shared-text-style-container`), `sharedSwatches`, `foreignSymbols: [MSImmutableForeignSymbol { libraryID, sourceLibraryName, symbolPrivate, originalMaster, symbolMaster }]`, `foreignLayerStyles`, `foreignTextStyles`, `foreignSwatches`.
- `sharedStyle` (`objects/shared-style.schema.yaml`): `do_objectID`, `name`, `value: style`.
- UNKNOWN: where `symbolMaster` layers live when not on a "Symbols" page — builder checks `symbol-master.schema.yaml` + fixtures; Studio puts masters where they are found.

## 2. User-visible outcome

A Sketch file with a design system opens in Studio with real components: each symbol is a main component, each symbol instance is an instance of it, and text, image, nested-symbol and style overrides show as instance overrides (editing the main updates the instances). Shared layer and text styles appear in Studio's styles list and stay linked; swatches become colour variables.

## 3. Behaviour rules (decided; technical)

- R1 `symbolMaster` → `Component` (Studio `NodeKind::Component`); `symbolInstance` → `Instance` linked to it; `scale` → instance scale (the uniform-scale path used for `.fig`).
- R2 Overrides, by `overrideName` suffix: `_stringValue` → text characters override; `_symbolID` → nested instance swap; `_image` → image fill override; `_layerStyle` → style ref override. The UUID chain resolves through masters' `do_objectID`s to Studio node ids; an unresolved chain is counted, never guessed.
- R3 `layerStyles` → Studio fill/stroke/effect styles (split by which properties the style sets); `layerTextStyles` → text styles; `sharedStyleID` on a layer → `style_refs`.
- R4 `sharedSwatches` → one colour collection "Sketch swatches" with one mode; swatch refs → variable bindings.
- R5 Foreign (library) symbols/styles (`foreign*`): embedded `symbolMaster` is present in the file, so it imports as a local component; library identity (`libraryID`, `sourceLibraryName`) kept in the component's metadata. How it is presented → **Q-309**.
- R6 Report extends #308's: overrides unresolved, foreign items imported, styles merged.

## 4. Acceptance criteria (red then green)

- **AC-1** Fixture with one symbol and three instances: Studio has 1 component, 3 instances; editing the main's fill updates all three.
- **AC-2** Each override kind (text, nested swap, image, layer style) on a fixture instance survives as an override, and the instance's other properties follow the main.
- **AC-3** `layerStyles` / `layerTextStyles` count equals Studio style count from that file; a layer linked to a shared style shows that style name in the Inspector.
- **AC-4** Swatches: a fill bound to a swatch becomes a variable binding; changing the variable recolours it.
- **AC-5** Foreign symbol fixture imports per Q-309 answer; report counts it.
- **AC-6** #308's ACs stay green (instances no longer "detached" in its count).

## 5. Test steps

`cargo test -p designos-interop sketch::symbols`; open fixture in Studio; capture Inspector showing an instance with overrides and the Styles list (1440).

## 6. Edge cases

- Override chain through a swapped nested symbol (override targets the swapped-in master's layers).
- Instance of a master that is missing (deleted symbol) → group with a counted "missing symbol".
- Recursive symbol (master contains an instance of itself, invalid in Sketch) → refused for that instance, counted.
- Text style + per-run attribute overrides inside a text with a shared text style.

## 7. Non-goals

Updating from a Sketch library file on disk; Sketch "smart layout" (`groupLayout`) → counted, not mapped (INFERENCE: closest Studio feature is auto layout; out of scope without owner ask).

## 8. User-visible details not settled by the issue

- Q-309: how library (foreign) symbols appear.

## 9. Rollback

Revert to #308 behaviour (detached copies) by removing the symbols pass; file format untouched.
