# E1-9 Import and export variables as Figma JSON and W3C DTCG tokens

| Field | Value |
|---|---|
| Issue | #138 (moved from v0.8 Variables everywhere to v0.6 AI design) |
| Size / priority | M / P2 |
| Depends on | none (REA step E1-f for Figma's native variables JSON) |
| Unblocks | part-2 E15-19 (styles compiled to variables), E15-42 (CSS/Tailwind themes to variables) |
| Touches | `crates/designos-fig/src/variables.rs`, new `crates/designos-fig/src/variables_dtcg.rs`, `crates/designos-app/src/variables/` (import/export menu) |

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
| `figma-native` | Figma's own variables export file; schema from REA E1-f (UNKNOWN until the REA report lands; the task starts with that step) | same schema |
| `dtcg` | W3C Design Tokens Format Module 2025.10 (stable) | same |

Format detection on import: REST if `meta.variables` present; `figma-native` by the REA-identified
top-level keys; DTCG if any object carries `$value`/`$type` or `$schema` points at the DTCG format.
Unknown → `UNKNOWN_TOKENS_FORMAT` naming the three formats.

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

No document model change. `VariablesFormat` enum and `import_variables(doc, text, format, opts) ->
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
6. Figma-native fields the model lacks (found by REA): preserved in collection/variable extension
   storage if they matter for round trip, else reported.

## 9. Non-goals

- Tokens Studio or other vendor-specific token formats.
- CSS/Tailwind theme import (E15-42).

## 10. Test plan

Seam: fixture round trips in designos-fig (fixtures written by us from the public specifications; never
client files).

| Case | Expected |
|---|---|
| `figma_native_import_lossless` | REA-derived fixture → every variable, mode, alias, scope and code syntax equals the expected model |
| `figma_export_reimports_in_figma` | manual check: export, import into Figma, screenshot attached to the PR (owner-manual leg, flagged) |
| `dtcg_export_validates` | export of the tokens fixture → validator reports 0 errors; `$type`, colour objects and aliases correct |
| `dtcg_round_trip_lossless` | export DTCG (manifest + per-mode files) → import into an empty doc → variables equal (ids via extensions) |
| `rest_round_trip_unchanged` | existing REST tests still pass |
| `merge_not_replace` | import into a doc with extra variables → extras kept |

## 11. Performance budgets

5,000 variables x 4 modes import or export ≤ 200 ms. Off the UI thread when triggered from the menu.

## 12. Dependencies and build order

REA E1-f first. Independent of the E15 write protocol; before part-2 E15-19 and E15-42.

## 13. Owner rulings

- R3.10 (D-E1-2): Figma-native and W3C DTCG design tokens.
- REA rule: Figma's schema via `rea` (morluto/rea).
- Delivery law: owner-manual legs (re-import into Figma) are flagged, never claimed.
