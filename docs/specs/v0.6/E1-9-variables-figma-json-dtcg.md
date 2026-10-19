# E1-9 Import and export variables as Figma JSON and W3C DTCG tokens

| Field | Value |
|---|---|
| Issue | #138 (moved from v0.8 Variables everywhere to v0.6 AI design) |
| Size / priority | M / P2 |
| Depends on | none (REA step E1-f for Figma's native variables JSON) |
| Unblocks | part-2 E15-19 (styles compiled to variables), E15-42 (CSS/Tailwind themes to variables) |
| Touches | `crates/designos-doc/src/variables.rs` (the `extensions` fields), `crates/designos-fig/src/variables.rs`, new `crates/designos-fig/src/variables_dtcg.rs`, `crates/designos-app/src/variables/` (import/export menu) |

## 1. Summary and user story

As a designer or developer, I import a variables file exported from Figma, or W3C design tokens, into a
document losslessly, and export ours in both forms so Figma and token tooling read them back without
loss.

## 2. Current state (FACT)

- designos-fig reads and writes the Figma REST `GET /v1/files/:key/variables/local` shape:
  `import_variables_value` replaces `doc.variables` and `doc.variable_collections`
  (`crates/designos-fig/src/variables.rs:90-164`), `import_variables_json` (`variables.rs:168-174`),
  `export_variables_meta` (`variables.rs:200`), `export_variables_json` (`variables.rs:241-243`),
  values including aliases as `{"type":"VARIABLE_ALIAS","id":...}` (`variables.rs:184-197`).
- Whether this equals the file Figma's own variables export produces is an unverified ASSUMPTION (issue
  context). There is no DTCG support.
- Model: `VariableCollection { id, name, modes, default_mode, variables, key, source_library_key }`,
  `Variable { id, name, collection, kind, values by mode, description, scopes, hidden_from_publishing,
  key, code_syntax }`, values `Color | Number | String | Boolean | Alias`
  (`crates/designos-doc/src/variables.rs:14-110`).

## 3. Behaviour rules

### 3.1 Formats

| Id | Import | Export |
|---|---|---|
| `figma-rest` | existing `variables/local` body | existing |
| `figma-native` | RESOLVED (§14). This is Figma's own Variables-modal export, a DTCG dialect: one `<mode name>.tokens.json` per mode (MIME `application/design-tokens+json`), with Figma metadata under `$extensions["com.figma.*"]`. Importing several files at once makes one mode per file; the mode name is the file name minus `.tokens.json` | Same: one `<mode>.tokens.json` per mode. All modes of a collection go in `<collection name>.zip` |
| `dtcg` | W3C Design Tokens Format Module 2025.10 (stable) | same |

Format detection on import:
- REST: `meta.variables` is present.
- `figma-native`: the top-level `$extensions["com.figma.modeName"]` is present, or any token carries `$extensions["com.figma.variableId"]`. Checked before `dtcg`, because the file is DTCG-shaped.
- DTCG: any object carries `$value`/`$type`, or `$schema` points at the DTCG format.
- Unknown → `UNKNOWN_TOKENS_FORMAT` naming the three formats.

### 3.2 DTCG mapping

- Groups: collection → top-level group; variable name `/` segments → nested groups (`Color/Primary` →
  `{"Color": {"Primary": {...}}}` inside the collection group).
- Types: Color → `$type: "color"` with the 2025.10 colour object `{colorSpace: "srgb", components:
  [r,g,b], alpha, hex}`; Number → `"number"`, or `"dimension"` with `{value, unit: "px"}` when the
  variable's scopes are all geometric (`WIDTH_HEIGHT`, `GAP`, `CORNER_RADIUS`, `STROKE_FLOAT`,
  `FONT_SIZE`, `LINE_HEIGHT`, `LETTER_SPACING`, `PARAGRAPH_SPACING`); String with `FONT_FAMILY` scope →
  `"fontFamily"`, other strings → `$type` omitted, value string with `$extensions` marker;
  Boolean → `$extensions` (DTCG has no boolean type).
- Aliases: `{Collection.Group.Name}` curly-brace references.
- Modes: DTCG has no modes in the format module. Export writes one token file per mode
  (`<collection>.<mode>.tokens.json`) plus `tokens.manifest.json` listing collections, modes and files;
  the default mode's file is the canonical one. Lossless metadata goes in
  `$extensions["org.designos"]`: `id`, `scopes`, `code_syntax`, `hidden_from_publishing`, `key`, and
  for booleans/plain strings the original kind. Import of a manifest restores modes; import of a single
  file creates one mode named after the file.
- `$description` ↔ `description`. `$deprecated` is preserved in `$extensions` on round trip.

### 3.3 Import semantics

- Import merges by default: collections matched by `id` (or name when ids are absent) update their
  variables; new ones are added; nothing is deleted. Option `replace: true` keeps today's replace
  behaviour for REST.
- One undo step (`Command::SetVariables`, `crates/designos-doc/src/history.rs:63-66`).
- Report: counts added/updated/unchanged, and every value that could not be mapped (with the token path
  and reason) — never dropped silently.

## 4. Data model / schema changes

One additive model change, needed so `figma-native` round-trips Figma fields the model has no slot for
(8.6). Fields in `crates/designos-doc/src/variables.rs`, all on existing structs; none changes a field
that exists today (`Variable.values` stays `BTreeMap<String, VariableValue>`, `variables.rs:100-101`).

```rust
pub type FigmaExtensions = BTreeMap<String, serde_json::Value>; // key -> verbatim JSON

pub struct VariableCollection {
    // ...existing fields...
    /// Collection-level Figma keys. Today: `com.figma.parentVariableSetId`,
    /// `com.figma.parentVariableSetName` (extended collections, file-root `$extensions`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: FigmaExtensions,
}

pub struct Variable {
    // ...existing fields...
    /// Variable-level keys. Today exactly one, ours: `org.designos.figmaTokenType`
    /// (`"easing"` or `"duration"`), set when a figma-native token of that `$type` is imported
    /// because the model has only Color/Number/String/Boolean (easing -> `String` variable holding the
    /// serialized easing; duration -> `Number` variable in ms). Export reads it to write `$type`
    /// back. Not a Figma key and never written into a token's `$extensions`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: FigmaExtensions,
    /// Per-mode Figma keys: outer key = a mode id of this variable's collection (the same keys as
    /// `values`), inner map = the `com.figma.*` keys that mode's token carried. A mode that carried
    /// none has no entry.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub mode_extensions: BTreeMap<String, FigmaExtensions>,
}
```

Serde: `default` plus `skip_serializing_if` on all three fields, so old files load unchanged and a
document without extensions saves byte-identically. `BTreeMap` keeps the output ordered
(deterministic). Every `Variable { .. }` and `VariableCollection { .. }` struct literal in the
workspace gains `extensions: Default::default()` (and `mode_extensions: Default::default()` on
`Variable`); the compiler finds them all. Keys are stored verbatim as Figma wrote them. An unknown
`com.figma.*` token key lands in `mode_extensions[mode]` (never in the variable-level map), so a
difference between modes is never merged away. The key of `mode_extensions` is always the key `values` uses
for that mode: when the import merges into an existing collection (§3.3) and matches the mode by name,
it is that mode's existing id; for a new mode it is the id the import assigns (a native file carries no
mode id, so the import uses the mode name, unique within the collection).

**Where each Figma key lands** (figma-native token `$extensions` and file-root `$extensions`):

| Figma key | Level | Lands in |
|---|---|---|
| `com.figma.variableId` | variable | `Variable.id` |
| `com.figma.scopes` | variable | `Variable.scopes` (REST names; native `STROKE` is accepted and stored as `STROKE_COLOR`, native export writes `STROKE`, INFERENCE per §14.8; `COLOR_OPACITY` follows the existing REST handling) |
| `com.figma.codeSyntax` | variable | `Variable.code_syntax` |
| `com.figma.hiddenFromPublishing` | variable | `Variable.hidden_from_publishing` |
| `com.figma.type` (`"boolean"`, `"string"`) | variable | not stored: derived from `Variable.kind` on export |
| `com.figma.modeName` (file root) | mode | `VariableMode.name` |
| `com.figma.parentVariableSetId`, `com.figma.parentVariableSetName` (file root) | collection | `VariableCollection.extensions` (same value in every mode file; export writes it into every file root) |
| `com.figma.isOverride` | **per mode** | `Variable.mode_extensions[mode]["com.figma.isOverride"]` |
| `com.figma.aliasData` | **per mode** | `Variable.mode_extensions[mode]["com.figma.aliasData"]` |
| `com.figma.paint` | **per mode** | `Variable.mode_extensions[mode]["com.figma.paint"]` (the value itself is a `Color`) |
| `com.figma.paintAliasData` | **per mode** | `Variable.mode_extensions[mode]["com.figma.paintAliasData"]` |
| `com.figma.composedColor` | **per mode** | `Variable.mode_extensions[mode]["com.figma.composedColor"]` (the value itself is the literal `Color`) |
| `$type: "easing"` / `"duration"` | variable | `Variable.extensions["org.designos.figmaTokenType"]` plus the `String` / `Number` value |
| any other `com.figma.*` | per mode | `Variable.mode_extensions[mode][key]` |

Cross-collection alias (the one case where §14.6 exports a literal): import keeps the token's literal as
that mode's `VariableValue` and the `com.figma.aliasData` object in `mode_extensions[mode]`; it does not
become a model `Alias`, because the file does not say which target mode the literal came from. The
import report lists each one (`cross-collection-alias-kept-as-literal`, token path). Export writes the
literal and the stored `aliasData` unchanged. A model `Alias` to a variable in another collection (made
in Studio) exports the target resolved the way `Document::resolve_variable` does (the exporting mode
id, else the target collection's default mode, `variables.rs:150-166`) plus an `aliasData` built from
the target (`targetVariableId` = `Variable.id`, `targetVariableName`, `targetVariableSetId`,
`targetVariableSetName`; a library target uses its publish `key` for the id). ASSUMPTION (confidence
medium): Figma resolves that literal the same way; it flips if the manual re-import leg shows
otherwise.

Edits that keep the per-mode map consistent: removing a mode (`variables_edit.rs:296`) and the
case where deleting a variable leaves an alias with no literal, so its mode value is removed
(`variables_edit.rs:482`), also remove `mode_extensions[mode]` for that mode;
deleting a variable drops everything. Editing a value in Studio clears only the keys that describe how
the old literal was derived: that mode's `com.figma.aliasData` and `com.figma.composedColor`. It keeps
`com.figma.paint` and `com.figma.paintAliasData` (the marker that the variable is a PAINT, not a COLOR,
must stay identical in every mode file) and `com.figma.isOverride` (an edited value in an extended
collection is still an override); unrelated keys stay.

The extensions are written by `figma-native` export and by `.designos` save. REST, `.fig` and `dtcg`
exports do not carry them: each export report lists every extension key it left out (count and
variable path), never silently. `VariablesFormat` enum and `import_variables(doc, text, format, opts) ->
ImportReport`, `export_variables(doc, format) -> ExportBundle { files: Vec<(name, bytes)> }`.

## 5. Contracts

- MCP: the E15-4 `set_variables` op is unchanged; no new MCP tool in this task (E15-42 adds theme
  import).
- CLI: `designos --import-variables <file> --format auto|figma-rest|figma-native|dtcg` and
  `--export-variables <dir> --format ...` on the existing `cli.rs` (non-interactive).

## 6. File-format mapping

As 3.1-3.2. DTCG files validate against the format rules implemented by a validator module (type/value
shape, alias syntax, group/token separation, name restrictions: no `{`, `}`, `.` and no leading `$`).

## 7. UI

The Variables panel menu (`crates/designos-app/src/variables/`) gains `Import variables…` and `Export
variables…` with a format choice (`Figma (REST)`, `Figma`, `Design tokens (DTCG)`), a result toast
`Imported 120 variables (4 skipped — details)`, and the details listing skipped tokens. Load es-designer
before UI code; desktop capture at 1440; responsive SKIPPED.

## 8. Edge cases

1. DTCG token name containing `.` from a variable name like `1.5x`: exported with the dot replaced by
   `_` and the original name in `$extensions`; import restores it.
2. Alias to a variable in another collection: DTCG reference spans groups; resolved on import.
3. Alias cycle in imported tokens: cycle members imported with their last literal value if any; cycle
   reported.
4. Colour in a non-sRGB colour space (DTCG `display-p3`): converted to sRGB with clamping; reported.
5. Same variable id in two imported files with different values per mode: merged by mode; conflicts in
   the same mode reported, first wins.
6. Figma-native fields the model has no first-class slot for: stored in `extensions` (§4) and written
   back by `figma-native` export, so the export re-imports into Figma unchanged. Any other
   `com.figma.*` key found on import is stored the same way (never dropped), and keys a target format
   cannot carry are listed in that export's report. REA found these (§14):
   - `com.figma.isOverride`, `com.figma.aliasData`, `com.figma.paint`, `com.figma.paintAliasData`,
     `com.figma.composedColor`: per mode (§4 table); two modes of one variable may carry different
     ones, or none;
   - the parent-collection ids (extended collections): on the collection;
   - `easing` and `duration` tokens (TIMING and EASING variables): variable-level marker (§4).

## 9. Non-goals

- Tokens Studio or other vendor-specific token formats.
- CSS/Tailwind theme import (E15-42).

## 10. Test plan

Seam: fixture round trips in designos-fig (fixtures written by us from the public specifications; never
client files).

| Case | Expected |
|---|---|
| `figma_native_import_lossless` | REA-derived fixture → every variable, mode, alias, scope and code syntax equals the expected model, and every `com.figma.*` key of the §4 table is present at the level and mode the table names |
| `figma_native_round_trip_json_equal` | fixture with a PAINT, an EASING, a TIMING, a composed colour, a cross-collection alias with `aliasData`, an `isOverride` token and an extended collection → import then `figma-native` export | every exported `<mode>.tokens.json` is JSON-equal to the fixture file |
| `two_mode_extensions_differ_per_mode` | fixture `variables-two-mode-extensions` (written by us): collection `Theme`, modes `Light` and `Dark`; `Brand/Fill`: Light carries `isOverride: true` and a cross-collection `aliasData`, Dark carries neither; `Brand/Paint` (PAINT): Light has `paint {paintType:"SOLID", blendMode:"MULTIPLY"}` and `paintAliasData`, Dark has `paint {paintType:"SOLID"}` only; `Brand/Tint`: Dark carries `composedColor`, Light does not; the collection carries the two parent keys | import: `mode_extensions[Light]` and `mode_extensions[Dark]` hold exactly those keys, no more; collection `extensions` holds the parent keys; export: `Light.tokens.json` and `Dark.tokens.json` are each JSON-equal to the fixture (a key never leaks into the other mode's file); parent keys appear in both file roots |
| `mode_extensions_follow_mode_edits` | on the document from the previous case: edit the `Light` value of `Brand/Fill` and of `Brand/Paint`; then delete mode `Dark` | after the edits `Brand/Fill` Light has no `aliasData` but still has `isOverride`, `Brand/Paint` Light still has `paint` and `paintAliasData`, and an edited `Brand/Tint` mode has no `composedColor`; after the delete no `mode_extensions` entry for `Dark` remains |
| `native_merge_keys_extensions_by_existing_mode_id` | document with collection `Theme` whose mode `Light` has id `1:0`; import `Light.tokens.json` carrying `isOverride` | `mode_extensions["1:0"]` holds the key; no entry keyed `Light` |
| `extensions_serde_default` | load a `.designos` written before this change; save a document with no extensions | loads with empty maps; the saved JSON has none of the three new keys and is byte-identical to today's for a document without extensions |
| `non_native_exports_report_extensions` | same document exported as `dtcg`, REST and `.fig` | `dtcg` output has no `com.figma.` key; each export report lists the left-out extension keys with counts; nothing dropped without a report entry |
| `figma_export_reimports_in_figma` | manual check: export, import into Figma, screenshot attached to the PR (owner-manual leg, flagged) |
| `dtcg_export_validates` | export of the tokens fixture → validator reports 0 errors; `$type`, colour objects and aliases correct |
| `dtcg_round_trip_lossless` | export DTCG (manifest + per-mode files) → import into an empty doc → variables equal (ids via extensions) |
| `dtcg_export_has_no_figma_keys` | DTCG export of a fixture imported from a `figma-native` file → no `com.figma.` key anywhere; booleans in `$extensions["org.designos"]` |
| `figma_native_export_matches_figma_dialect` | `figma-native` export of a boolean and a string variable → `$type: "number"` 0/1 with `com.figma.type: "boolean"`, and `$type: "string"` |
| `rest_round_trip_unchanged` | existing REST tests still pass |
| `merge_not_replace` | import into a doc with extra variables → extras kept |

## 11. Performance budgets

5,000 variables x 4 modes import or export ≤ 200 ms. Off the UI thread when triggered from the menu.

## 12. Dependencies and build order

REA E1-f first. Independent of the E15 write protocol; before part-2 E15-19 and E15-42.

## 13. Owner rulings

- R3.10 (D-E1-2): Figma-native and W3C DTCG design tokens.
- Owner ruling 2026-10-19: **two separate formats**, never one hybrid file.
  - `figma-native` follows Figma's own dialect exactly (booleans as `number` 0/1 with `com.figma.type`, `$type: "string"`, `com.figma.*` metadata, one `<mode>.tokens.json` per mode, zip per collection), so the export re-imports into Figma unchanged.
  - `dtcg` is strict W3C DTCG 2025.10: only DTCG types, our metadata only under `$extensions["org.designos"]`, multi-mode as the manifest of §3.2. It must pass the DTCG validator with zero errors and must not carry `com.figma.*` keys.
  - The import menu and CLI keep the two as separate choices; `auto` detection (§3.1) picks one, never mixes.
- REA rule: Figma's schema via `rea` (morluto/rea).
- Delivery law: owner-manual legs (re-import into Figma) are flagged, never claimed.

## 14. Facts (REA 2026-10-18)

Method. Read-only. Sources:
- Figma's cached editor JS: `~/Library/Application Support/Figma/DesktopProfile/v43/Cache/Cache_Data`, chunk `webpack-artifacts/assets/3187-2ffbb1f1e33f7509.min.js.br` (brotli). Five other cached builds of the same chunk exist.
- The cached English strings: `figma_app-*.min.en.json.br`.
- The kiwi schema in the cached fullscreen wasm.
- Public docs: `figma/rest-api-spec` openapi.yaml 0.45.0 and `@figma/plugin-typings` 1.142.0.

`rea` 6.7.0 (`npx -y rea-agents analyze-javascript-application`, evidence `ev_2624766…`) indexed the extracted chunks. The facts below come from reading the decoded chunk directly. They are restated in our own words; no Figma code is copied.

1. FACT. A native format exists, and it is not a plugin-API or REST feature.
   - The Variables modal has `Import variables`, a mode-header menu with `Import mode` / `Export mode`, and a collection menu with `Export modes`. Strings: `variables.authoring_modal.import_variables`, `….mode_context_menu.export_mode`, `….sidebar.context_menu.export_modes`.
   - `@figma/plugin-typings` 1.142.0 has no variables export method.
   - REST offers only `GET /v1/files/:key/variables/local`, the shape `figma-rest` already implements.
2. FACT. Files.
   - Each export file is named `${name}.tokens.json`, written with `JSON.stringify(json, null, 2)`, MIME `application/design-tokens+json`.
   - "Export modes" zips one file per mode into `${collection.name}.zip`.
   - "Export mode" downloads one file. INFERENCE (strong): that file is named after the mode, the same key the importer reads back.
   - Import accepts `application/json` and `application/design-tokens+json`, several files at once. It derives each mode name from the file name by dropping `.tokens.json` (also `.tokens (N).json`), otherwise just the extension. It then calls `create-variables-from-design-tokens` with `jsonByModeName`.
3. FACT. Document shape, one mode per file.
   - Nested groups follow the variable name split on `/`.
   - Each token is `{ $type, $value, $description?, $extensions }`.
   - The file root carries `$extensions: { "com.figma.modeName": <mode>, "com.figma.parentVariableSetId"?, "com.figma.parentVariableSetName"? }`. The two parent keys appear only for extended collections.
   - A variable whose name equals a group name becomes that group's `$root` token, and aliases to it are rewritten to `{path.$root}`.
   - Soft-deleted variables and modes with no value are skipped.
4. FACT. Type mapping, export:

   | Figma variable | `$type` | `$value` | Extra `$extensions` |
   |---|---|---|---|
   | COLOR | `color` | `{colorSpace:"srgb", components:[r,g,b], alpha, hex:"#RRGGBB"}` | none |
   | FLOAT | `number` | number | none |
   | FLOAT (TIMING) | `duration` | `{value: <ms, rounded>, unit:"ms"}` | none |
   | BOOLEAN | `number` | `0` / `1` | `com.figma.type: "boolean"` |
   | STRING | `string` (not a DTCG type) | string | `com.figma.type: "string"` |
   | EASING | `easing` (not a DTCG type) | serialized easing, `cubic-bezier(…)` / `spring(…)` / named | none |
   | PAINT (solid only) | `color` | colour object, alpha = paint opacity | `com.figma.paint: {paintType:"SOLID", blendMode?}`, `com.figma.paintAliasData?: {opacity: …}` |

5. FACT. Extensions on every token:
   - `com.figma.variableId` (always);
   - `com.figma.scopes` (scope names, only when non-empty; `COLOR_OPACITY` is dropped unless colour-opacity is enabled);
   - `com.figma.codeSyntax: {WEB?, ANDROID?, iOS?}` (kiwi `CodeSyntaxPlatform` WEB=0, ANDROID=1, iOS=2);
   - `com.figma.hiddenFromPublishing: true` (only for local, unpublishable variables);
   - `com.figma.isOverride: true` (a value overridden in an extended collection).
6. FACT. Aliases.
   - Same collection: `$value: "{Group.Sub.Name}"`, the target name with `/` → `.`, typed by the target's resolved type.
   - Cross collection: the resolved literal value plus `com.figma.aliasData: {targetVariableId, targetVariableName, targetVariableSetId, targetVariableSetName}`. Library targets use the publish key instead of the local id.
   - A COMPOSE_COLOR expression exports as a `color` token with `com.figma.composedColor`.
7. FACT. Import accepts more than it exports:
   - `color` in `srgb` or `hsl`; `dimension` in `px` only;
   - `fontFamily` as a string only (not an array); `string`;
   - `number` (BOOLEAN when `com.figma.type` is `"boolean"`); `duration` in `ms` or `s`;
   - `easing` (named, `cubic-bezier(…)`, `spring(…)`).

   Tokens without `$type` infer their type. The named errors are `unsupported-color-space`, `unsupported-dimension-unit`, `unsupported-font-family-value-type`, `unsupported-duration-unit` and `unsupported-easing-token-value`. The import toast counts tokens imported, updated, overridden and in error.
8. FACT. Scope names differ between kiwi and REST.
   - Kiwi `VariableScope` (0–23): `ALL_SCOPES, TEXT_CONTENT, CORNER_RADIUS, WIDTH_HEIGHT, GAP, ALL_FILLS, FRAME_FILL, SHAPE_FILL, TEXT_FILL, STROKE, STROKE_FLOAT, EFFECT_FLOAT, EFFECT_COLOR, OPACITY, FONT_STYLE, FONT_FAMILY, FONT_SIZE, LINE_HEIGHT, LETTER_SPACING, PARAGRAPH_SPACING, PARAGRAPH_INDENT, FONT_VARIATIONS, TRANSFORM, COLOR_OPACITY`.
   - REST uses `STROKE_COLOR` instead of `STROKE`, adds `FONT_WEIGHT`, and has no `TRANSFORM`.
   - INFERENCE: the export writes the editor enum names (the same enum object that also carries `TRANSFORM`). A `figma-native` importer should therefore accept both `STROKE` and `STROKE_COLOR`.
9. For the spec owner; R3.10 and §3.2 are unchanged here. The `dtcg` mapping in §3.2 differs from Figma's own dialect in four places:
   - booleans: `$extensions` vs Figma's `number` 0/1 + `com.figma.type`;
   - plain strings: `$type` omitted vs Figma's `$type: "string"`;
   - metadata namespace: `org.designos` vs `com.figma.*`;
   - multi-mode packaging: manifest vs zip of per-mode files.

   `figma-native` must follow Figma's choices exactly to re-import into Figma. Resolved by the owner ruling in §13: the two formats stay separate and each keeps its own choices.
