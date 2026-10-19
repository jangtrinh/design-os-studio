# #570 JSON scene format: importer and exporter at schema 2.21 with a full version-migration chain

| Field | Value |
|---|---|
| Issue | #570 (shipping bug; R&D adoption task 2026-10-18; moved to v0.6 AI design) |
| Size / priority | L / P0 (E15-7 lists it as an external prerequisite) |
| Depends on | the coordinator's step list and schema notes (see "Open questions for the coordinator"); no other v0.6 task |
| Unblocks | E15-7 (in-progress flag maps to `placeholder: true`), every part-2 task that exchanges scenes with an agent |
| Touches | `crates/designos-fig/src/pen.rs` (1,589 lines today: split, see rule 12), new `crates/designos-fig/src/pen/` modules, `crates/designos-fig/src/lib.rs` (exports, `:40`), `crates/designos-fig/tests/` (new `pen_*` test files and fixtures under `crates/designos-fig/tests/fixtures/`), `crates/designos-doc` (only if a field the format needs has no home, see section 4) |

Code baseline: `origin/main@01d48f2` (0.5.0). **Which code this is.** The issue calls it a "third-party JSON scene-format importer/exporter". In this repository it is the
Pencil.dev `.pen` module, `crates/designos-fig/src/pen.rs` ("Pencil.dev (`.pen`) JSON format importer and exporter", `pen.rs:1`), re-exported as `export_pen_json`,
`import_pen_json`, `import_pen_json_with_warnings` (`lib.rs:40`). Everything below is based on that code. Fixtures are written by us; no third-party sample file is used
(acceptance 4) and no file of the owner is copied.

## 1. Summary and user story

As an agent or a tool that exchanges design scenes as JSON with design:os, a file I export declares the current schema version (2.21) and opens correctly in tools that
read 2.21, and an older file (1.0, 2.8, 2.13, 2.17) imports with all geometry, instances, fills, strokes, variables and themes mapped, with every unsupported part reported
and preserved instead of dropped.

## 2. Current state (FACT)

- **Export writes the legacy version.** `export_pen_json` inserts `"version": "1.0"` (`pen.rs:1035`) and emits only the fields listed in section 6; nothing else.
- **Import ignores `version`.** `import_pen_json_budgeted` reads `name`, `themes`, `variables`, `pages` / `children`, `background` (`:41-97`); no code reads the version, so there
  is no migration chain.
- **Current consumers.** `import_pen_json` / `export_pen_json` are used by tests only (`tests/stack_wrap_and_text_wrap.rs:186`, `tests/pen_component_depth_limits.rs`); the app
  has no `.pen` open path and the library probe does not know the extension (no hit outside `designos-fig` in a search for `"pen"` / `.pen`).
- **Mis-mapped cases the issue lists, as the code does them today:**
  - *Instances.* A `ref` node's size defaults to 100 x 100 when it sets none (`parse_dimension(.., 100.0)`, `:682-689`); fills, stroke and corner radius are read from the
    instance's own keys only (`:699`, `:864`, `:870`). After import `resolve_pen_refs` clones the component's children into an instance with no children of its own and applies
    `descendants` overrides, but only `name`, `enabled`/`visible`, `width`, `height`, `content`/`characters` and `fill`/`fills` (`apply_descendant_override`, `:993-1030`).
    There is no way to replace `children` (slot filling) and no override of stroke, padding, font or icon.
  - *Frames.* A frame with no `layout` key and none of `gap`, `padding`, `justifyContent`, `alignItems` gets **no** auto layout (`parse_frame_props`, `:624-631`, the `None => None`
    arm), and absent `width`/`height` become fixed 100 (`:300-326`).
  - *Fills.* Only `"linear"|"linear_gradient"|"linear-gradient"` and `"radial"...` object types are read, with handles fixed to defaults and stops from `stops[].position|offset`
    / `color` (`parse_paints`, `:451-468`, `:492-508`); there is no `gradient` type with `gradientType` and `colors`, no angular gradient (the document has
    `PaintKind::AngularGradient`, `designos-doc/src/paint.rs:200`), and an image fill stores the **raw URL string** as `image_ref` with `ImageScaleMode::Fill` (`:469-472`) although
    `image_ref` keys into `Document.images` (`paint.rs:208`).
  - *Strokes.* `thickness` / `weight` as a number or a `{top,right,bottom,left}` object, `align` of `center` / `outside` (anything else inside), and a stroke `fill` / `color` / `paints`
    (`parse_stroke`, `:510-547`). No `strokeWidth`, no `strokeAlignment`, no line cap or join (the model has `Stroke.cap`, `Stroke.join`, `paint.rs:272-273`).
  - *Icon nodes.* `icon_font` is imported as a text node whose content is `iconFontName` (`:734-737`): the layer shows the glyph's name as text.
  - *Variables.* Only `fontSize` is bound (`:738-741`); `fontWeight`, `lineHeight` and `letterSpacing` are read as literals. The model binds exactly `fontSize`, `fontWeight`,
    `lineHeight`, `letterSpacing` among text numbers (`designos-doc/src/variables_apply.rs:26`, `:273`); a font family has no bindable key.
  - *Default theme.* `default_mode = modes[0].id` where `modes` is filled from `themes`: an array in order, but for an object it iterates the map (`:101-121`). `serde_json` is
    built without `preserve_order` (`Cargo.toml`: `serde_json = "1"`), so object keys iterate alphabetically; the first axis and the first value of a themes object are therefore
    not the file's first. Variable values given as an object of `mode: value` iterate the same way (`:196-204`). Export hard-codes one axis named `mode` (`:1048-1049`, `:1068`).
    All axes land in one collection `pen:tokens` (`:153-164`).
  - *Library `imports`.* No code reads `imports`; a `ref` whose target is not in the same file is skipped silently (`resolve_pen_refs`, `:910-912`).
  - *Other fields.* `reusable` is honoured only for `frame` and the unknown-type fallback (`:703-709`, `:822-827`); `slot`, `placeholder`, ellipse arcs (always `ArcData::default()`,
    `:714`), path `viewBox`, `layoutPosition`, flips, underline, strikethrough, `href`, `blendMode` are not read; rotation is read, flips are not.
  - *Unknown node kinds.* Any other `type` falls into the last match arm and becomes a `Frame` (or `Component` when `reusable`) (`:821-827`): `prompt`, `context`, `script` and `browser`
    nodes silently turn into empty frames, and their own fields are lost. `NodeKind::Unknown { type_name }` exists for exactly this ("kept so nothing is silently lost",
    `designos-doc/src/node.rs:500-506`) and is unused by this module.
- **Warnings.** The only warning is the clone-budget message (`import_pen_json_with_warnings` returns `Result<(Document, Option<String>), String>`, `pen.rs:37`); everything else is dropped without a record.
- **Limits already in place** (must stay): `MAX_PEN_REF_CLONES = 2_000_000` (`:896`), iterative cloning and deep-tree safety (`clone_component_subtree`, `:974-991`; `serde_json` feature
  `unbounded_depth`), ref cycle errors (`:913-915`). Tests: `pen_component_depth_limits.rs`; unit tests in `pen.rs:1357-1589` (round trip, ref depth, budgets).
- Document model homes that already exist: `Node.source_id` (the file's own id, `node.rs:69`), `Node.plugin_data` (survives `.fig` round trips, `node.rs:146`),
  `Node.layout_child.absolute` (`node.rs:1069`), `Node.hyperlink` (`node.rs:143`), `Node.blend`, `TextStyle.decoration` (`TextDecoration::{None, Underline, Strikethrough}`,
  `paint.rs:432-437`), `ArcData { start, end, inner_radius }` (`node.rs:624-628`), component slots (`ComponentPropDef` with `PropKind::Slot`, `is_slot`, `is_slot_content`,
  `OverrideField::SlotContent`, `node.rs:168-290`), instance override flags (`OverrideField::{Name, Visible, Size, Fills, Stroke, CornerRadii, Characters, TextStyle, Layout, ...}`).

## 3. Behaviour rules

Legend: **[issue]** names the key or behaviour in the issue text. **[coordinator]** needs input the coordinator holds (see the last section); the rule states what is decided and what is not.

1. **Version chain on import [issue].** `import` reads the root `version` (a string `MAJOR.MINOR`; a missing or unparsable value is treated as `1.0` and reported as a warning). A chain of
   migration steps, one function per hop, runs on the parsed `serde_json::Value` in place from the file's version up to 2.21, before any mapping; each step may add a warning. After the
   chain the mapping code sees exactly one shape (2.21). A version above 2.21 imports as 2.21 with a warning "newer schema, unknown fields reported". The step list and what
   each hop changes are **[coordinator]**: this spec fixes the mechanism (a `Vec<Step { from, to, apply }>` table in a new `pen/migrate.rs`, tested hop by hop and end to end), not the hops.
2. **Export always writes the current version [issue].** `"version": "2.21"`. Only fields the 2.21 schema defines are written. Which of today's output keys are still valid in 2.21
   (`enabled`, `reusable`, `fill`, `stroke{fill,thickness}`, `layout`, `padding`, `justifyContent`, `alignItems`, `clip`, `cornerRadius`, `polygonCount`, `innerRadius`, `geometry`,
   `content`, `fontFamily`, `fontSize`, `fontWeight`, `fontStyle`, `descendants`) is **[coordinator]**.
3. **Instances [issue].**
   1. Inheritance: for `size`, `fill`, `stroke` and `cornerRadius`, a key the instance does not set is taken from the component at resolve time (the component's solved size, paints,
      stroke, radii). "Does not set" is tracked per key while parsing the instance (an `explicit: BTreeSet<&str>`), because today absent and default are indistinguishable (`:682-689`).
      A key the instance sets stays and is recorded as the matching `OverrideField` (`Size`, `Fills`, `Stroke`, `CornerRadii`) on the instance root.
   2. Descendant replacement: a `descendants` entry may carry `children` (a list of node objects): the targeted layer's children are replaced by the imported list (slot filling),
      marked `OverrideField::SlotContent` on the target (ties to #582) and `is_slot_content` on the new nodes; if the target is a slot container (`is_slot`) the nodes become that slot's content.
   3. Descendant overrides gain: stroke (width, paints, alignment, cap, join), padding, font (family, size, weight, style as bound or literal) and icon (name, family), each recorded as the
      matching `OverrideField` (`Stroke`, `Layout`, `TextStyle`) and exported back in the same shape. Key names inside overrides follow the node-level keys of rules 5 to 8.
   4. An unresolvable `ref` (missing target) keeps the instance with `component_key = ref`, size from the instance (or 100 x 100 as today when it sets none) and a warning naming the id.
4. **Frames [issue].** When `layout` is absent a frame is a horizontal auto-layout frame, and when `width` / `height` are absent they are `fit_content` (hug), not fixed 100. Explicit
   `layout: "none"` stays "no auto layout". Rules for non-frame nodes (rectangles, ellipses, text: default width/height when absent) keep today's defaults unless the coordinator says
   otherwise **[coordinator]**. The export writes `layout` and sizing explicitly so a re-import never depends on defaults.
5. **Fills [issue].**
   1. `gradient` fill with `gradientType` (linear, radial, angular) and `colors`: mapped to `PaintKind::LinearGradient`, `RadialGradient`, `AngularGradient` with stops from `colors`. The element
      shape of `colors` (stop position field, colour string form, alpha) and how gradient geometry (angle, centre, handles) is expressed are **[coordinator]**; today's `linear` / `radial` object
      types and `stops` keep working through the migration chain.
   2. Image fill with `mode` and a URL: `mode` maps onto `ImageScaleMode::{Fill, Fit, Crop, Tile}` (the value vocabulary is **[coordinator]**). A relative URL is resolved **inside the project folder only**
      (rule 9); the bytes go into `Document.images` and `image_ref` becomes that key. A remote `http(s)` or `data:` URL is not fetched: the fill keeps its URL in `plugin_data["pen:image-url"]`, renders
      as a missing image, and a warning is reported.
6. **Strokes [issue].** `strokeWidth` (a single number, or per side), `strokeAlignment` (`inner` -> `Inside`, `center` -> `Center`, `outer` -> `Outside`), line cap and join (to `Stroke.cap`,
   `Stroke.join`; key names and values **[coordinator]**), and a stroke given as a list of fills (all become `Stroke.paints`, not only the first: the exporter today writes the first solid paint
   only, `:1253-1264`). Per-side widths use `Stroke.side_weights` as today (`:529-535`). Export writes every field it reads.
7. **Icon nodes [issue].** An icon node renders its glyph instead of an empty frame or the glyph's name as text. How the glyph is drawn (which icon fonts exist in the app, what happens when the
   family is not bundled) is **[coordinator]**; the rule that fixes the bug is that no icon node may import as a text layer showing its name. Until the coordinator answers, the importer maps an icon
   node to a vector node when the glyph is resolvable and otherwise to a placeholder frame carrying `plugin_data["pen:icon"] = {family, name}` plus a warning, so nothing is lost and it round-trips.
8. **Variables and themes [issue].**
   1. Font and number properties bound to `$variables`: `fontSize` (already), `fontWeight`, `lineHeight`, `letterSpacing` and padding/gap/radius/opacity (already) bind through
      `node.variables.bindings` (keys of `variables_apply.rs:26`). A font-family binding has no bindable key in the model: it is imported as the resolved family and reported as a warning
      (keeping the `$name` in `plugin_data["pen:bindings"]` for round trip).
   2. The default theme value is the **first** theme value in the file's own order, never alphabetical: the importer parses `themes` and `variables` values with an order-preserving reader
      (the map is read as `Vec<(String, Value)>` using `serde_json::value::RawValue` or a streaming pass; the `serde_json/preserve_order` crate feature is not an option, because it would change the key order of every `Map` in the process;
      the exact reader is a technical choice recorded in the PR).
   3. Multi-axis themes (several axes such as `mode` and `density`) are one variable collection per axis, in file order, each with its own default mode; a variable value that names several
      axes applies to the matching combination. Node-level `theme` selects the mode per axis for that subtree (`node.variables.modes`, as `:878-882` does for one axis). The exact value shape
      of a multi-axis value and of a node-level `theme` are **[coordinator]**.
   4. Export writes each axis under its own name instead of the hard-coded `mode`.
9. **Library `imports` [issue] and safe paths.** `imports` entries are resolved to files **inside the project folder**: the importer takes `PenImportOptions { base_dir: Option<PathBuf> }`. A relative
   path is joined to `base_dir`, canonicalised, and refused (warning, not an error) when it is absolute, contains `..` that leaves `base_dir`, or resolves through a symlink outside it. A resolved
   file is imported with the same pipeline (depth limit 8, cycle detection by canonical path, shared clone budget) and its reusable nodes become referenceable as `<alias>:<id>`; a missing
   or refused import leaves every instance of it unexpanded with `component_key` set and one warning per import. Without `base_dir` no file is read and imports are reported as unresolved. The
   `imports` and `ref` key shapes are **[coordinator]**.
10. **Other nodes and fields [issue].**
    - `reusable` on any node type: a rectangle, ellipse, text and so on with `reusable: true` becomes a component whose single child is that node (the model's `NodeKind::Component` carries frame
      props only), exported back as the same node with `reusable: true`.
    - `slot` lists and `placeholder`: a frame with a `slot` list defines slot properties (`ComponentPropDef` with `PropKind::Slot`, `is_slot`) whose preferred components come from the list;
      `placeholder: true` is stored as `plugin_data["pen:placeholder"] = "true"` on the node (`Document` has no `in_progress` field on `origin/main@01d48f2`, `designos-doc/src/lib.rs:142-195`; E15-7 adds it). E15-7 then maps `pen:placeholder` to its marker with `run_id: None`; this task does not wait for that field and adds no `Document` field. List and flag shapes: **[coordinator]**.
    - Ellipse arcs: to `ArcData { start, end, inner_radius }` (units and key names **[coordinator]**).
    - Path `viewBox`: path data is scaled from the viewBox to the node's box (today `geometry` is used unscaled, `:726-732`).
    - `layoutPosition: absolute` -> `layout_child.absolute = true`; exported when set.
    - Flips: mirrored transforms (negative scale on the node's `Affine`), exported as flips (key names **[coordinator]**).
    - Underline and strikethrough -> `TextStyle.decoration`; `href` -> `Node.hyperlink`; `blendMode` -> `Node.blend` (vocabulary **[coordinator]**).
11. **Unsupported node kinds [issue]: `prompt`, `context`, `script`, `browser` and any other unknown `type`.** They import as `NodeKind::Unknown { type_name }` (children kept, not drawn), with the node's
    whole JSON object minus `children` stored compactly in `plugin_data["pen:raw"]`, and are written back verbatim by the exporter (id, type, name and all fields, children re-exported normally).
    Every one is listed in the import report: kind, id and name. The `_ => Frame` arm is removed. Nothing is dropped silently.
12. **Module split and API.** `pen.rs` (1,589 lines) becomes `pen/mod.rs` plus `migrate.rs`, `variables.rs`, `paint.rs`, `node.rs`, `instances.rs`, `export.rs`, each under 200 lines where
    practical, with the existing tests moved beside what they test. New entry point
    `import_pen_json_report(raw: &str, opts: &PenImportOptions) -> Result<PenImport, String>` where `PenImport { doc: Document, warnings: Vec<PenWarning>, source_version: String }` and
    `PenWarning { code, node_id: Option<String>, message }`. `import_pen_json` and `import_pen_json_with_warnings` keep their signatures and call it (the second returns the clone-budget warning as
    before). Warnings are also printed to stderr as today for `import_pen_json`.

## 4. Data model / schema changes

- No new `Document` or `Node` field is required: every target above has a home in section 2 (`plugin_data` for what has none: `pen:raw`, `pen:icon`, `pen:bindings`, `pen:image-url`, `pen:placeholder`, `pen:root`). `Node.plugin_data` is a `BTreeMap<String, String>` (`node.rs:146`), so every `pen:*` value is a **string**: JSON text for structured values (`pen:raw`, `pen:icon`, `pen:bindings`), the literal `true` for `pen:placeholder`.
  `plugin_data` is a `.fig` round-trip carrier (`node.rs:146`), so these survive an export to `.fig` and back (checked by the round-trip tests).
- New: `PenImportOptions`, `PenImport`, `PenWarning` (public, in `designos-fig`).
- If the coordinator's schema notes show a field with no home (for example gradient geometry richer than three handles), the worker stops and asks; no field is invented here.

## 5. Contracts

- Public: `import_pen_json_report`, `PenImportOptions`, `PenImport`, `PenWarning`; existing functions unchanged in signature.
- Exported files declare `"version": "2.21"`, validated by a shape check in the test suite (acceptance 2): required keys per node type and no key outside the 2.21 set, using a
  schema table written by us from the coordinator's description (`tests/pen_schema_2_21.rs`).
- No MCP tool is added by this task (no tool consumes the format today).

## 6. File-format field mappings (summary; the rules above are normative)

| Format field | Document target | Today | Rule |
|---|---|---|---|
| `version` | none | ignored on import, `"1.0"` on export (`:1035`) | 1, 2 |
| `themes`, `variables`, `$name` | `variable_collections`, `variables`, `node.variables.bindings`, `node.variables.modes` | one collection, alphabetical default for objects (`:101-165`) | 8 |
| `width`, `height` (`number`, `fill_container`, `fit_content`, `$var`) | `node.size`, `layout_child.horizontal/vertical` | default 100 fixed (`:300-326`) | 3, 4 |
| `layout`, `gap`, `padding`, `justifyContent`, `alignItems`, `wrap`, `clip` | `FrameProps.auto_layout`, `clips_content` | no layout when absent (`:624-631`) | 4 |
| `fill` (colour, `$var`, array, `linear`/`radial` object) | `node.fills`, `bindings["fills/i"]` | `:427-508` | 5 |
| `gradient` + `gradientType` + `colors` | `PaintKind::{Linear,Radial,Angular}Gradient` | absent | 5 |
| image fill (`mode`, relative URL) | `PaintKind::Image`, `Document.images` | raw URL as key (`:469-472`) | 5, 9 |
| `stroke` (`thickness`/`weight`, `align`, `fill`/`color`/`paints`) | `node.stroke` | `:510-547` | 6 |
| `strokeWidth`, `strokeAlignment`, cap, join, list of fills | `Stroke.weight/side_weights/align/cap/join/paints` | absent | 6 |
| `cornerRadius` (number, array of 4, `$var`) | `node.corner_radii`, bindings | `:592-616` | 3 |
| `effect` / `effects` | `node.effects` | `:549-590` | unchanged |
| `type: frame` + `reusable` | `NodeKind::Component` | `:703-709` | 10 |
| `reusable` on other types | component wrapping the node | ignored | 10 |
| `type: ref`, `ref`, `descendants` | `NodeKind::Instance`, `component_key`, overrides | `:806-811`, `:993-1030` | 3 |
| `descendants[..].children` | slot content | absent | 3.2 |
| `slot` list | slot props | absent | 10 |
| `placeholder` | `plugin_data["pen:placeholder"]` (E15-7 maps it to its marker) | absent | 10 |
| `imports` | resolved library files | absent | 9 |
| `type: icon_font`, `iconFontName`, `iconFontFamily` | glyph node | text of the name (`:734-737`) | 7 |
| `type: ellipse` arc fields | `ArcData` | default only | 10 |
| path `geometry`/`d`, `viewBox`, `fillRule` | `NodeKind::Vector.paths` | unscaled (`:725-733`) | 10 |
| `layoutPosition` | `layout_child.absolute` | absent | 10 |
| flips | node transform | absent | 10 |
| `underline`, `strikethrough`, `href`, `blendMode` | `TextStyle.decoration`, `Node.hyperlink`, `Node.blend` | absent | 10 |
| `prompt`, `context`, `script`, `browser`, other types | `NodeKind::Unknown` + `plugin_data["pen:raw"]` | become frames (`:821-827`) | 11 |

Key names marked **[coordinator]** in section 3 are not in this table on purpose.

## 7. UI

None. (There is no `.pen` open path in the app; the app and MCP wiring that expose the format are separate tasks. Visual evidence for this task is a terminal capture and a rendered
comparison, see section 10.)

## 8. Edge cases

1. A file with no `version`: treated as 1.0 with a warning.
2. `version: "2.21"` with a hand-edited unknown top-level key: kept as JSON text in the first page node's `plugin_data["pen:root"]` (the root has no node of its own), reported.
3. An instance of a component defined later in the file: resolved after the whole tree is read (as today, `pending_refs`).
4. Two nodes with the same `id`: the later one wins in the id map (as today) and a warning names the duplicate.
5. A `ref` to itself or to an ancestor: error, as today (`:913-915`).
6. An image path with `..`, absolute, a symlink out of the folder, a missing file, an unreadable file, a non-image file, a file over the size cap (initial 64 MiB per image, 256 MiB per import):
   each is one warning and the fill keeps its URL in `plugin_data`.
7. Import of 100,000 nodes: the migration chain and mapping stay linear; no per-node `Value` clones.
8. A theme object with a single axis and a single value: one collection, one mode.
9. Round trip of an unknown node with children: the children are re-exported normally inside the preserved raw object.
10. Export of a document that was never imported from the format (a Studio-made design): every node type the exporter cannot express (connector, sticky variants, boolean ops, component sets) keeps
    today's mapping (`:1140-1156`) and the unmapped ones are listed in an export warning instead of silently becoming frames.

## 9. Non-goals

- Fetching remote URLs or data URIs.
- Rendering icon fonts that the app does not bundle (see rule 7).
- A new UI or MCP tool for opening `.pen` files.
- Reading any third-party sample file (acceptance 4); the owner's files are never used.
- Changing the `.fig` or REST importers.

## 10. Test plan (red first)

Seams: `designos-fig` integration tests with hand-written, content-free fixtures under `crates/designos-fig/tests/fixtures/pen/` (one logical design encoded at 1.0, 2.8, 2.13, 2.17 and 2.21),
unit tests per migration hop, the shape check for the exported file.

| Case | Expected (fails before) |
|---|---|
| `same_design_five_versions_import_field_equal` | the five fixtures import to documents that are field-equal for every supported field (compare through a helper that matches nodes by `source_id` and ignores `NodeId`s) |
| `round_trip_is_field_equal_at_each_version` | for each fixture: import -> export -> re-import: field-equal (acceptance 1) |
| `export_declares_2_21_and_only_2_21_fields` | `"version": "2.21"`; the shape check passes; no key outside the 2.21 table (acceptance 2) |
| `each_migration_hop_alone` | one test per `Step { from, to }` on a minimal input |
| `missing_version_is_1_0_with_a_warning` | warning code `version-missing` |
| `newer_version_imports_with_a_warning` | `3.0` imports as 2.21 shape, warning |
| `instance_takes_size_fill_stroke_radius_from_the_component` | instance sets none: copies; sets some: keeps them and records `OverrideField`s (fail-before: 100 x 100, no fill) |
| `instance_children_replacement_fills_a_slot` | `descendants[..].children` replaces the target's children; slot marked |
| `instance_overrides_stroke_padding_font_icon` | each override applied and exported back |
| `frame_defaults_to_horizontal_and_fit_content` | absent keys: horizontal auto layout, hug sizing (fail-before: no layout, fixed 100) |
| `gradient_linear_radial_angular` | three fixtures; paint kinds and stops (fail-before: no `gradient` type) |
| `image_fill_resolves_a_relative_path_inside_the_project_folder` | bytes in `Document.images`, `image_ref` is that key; `mode` mapped |
| `image_paths_outside_the_folder_are_refused` | `..`, absolute and symlink-out cases: warning, no read |
| `stroke_width_alignment_cap_join_and_fill_list` | `strokeWidth` single and per side, the three alignments, all paints kept (fail-before: `strokeWidth` ignored) |
| `icon_node_is_not_a_text_layer_of_its_name` | (fail-before: text layer with the glyph name) |
| `text_number_bindings_font_weight_line_height_letter_spacing` | bound keys present (fail-before: only `fontSize`) |
| `font_family_binding_is_reported_not_dropped` | warning plus `plugin_data["pen:bindings"]` |
| `default_theme_is_the_first_in_file_order` | `themes` / values objects whose first key sorts last alphabetically: the default is the first in file order (fail-before: alphabetical) |
| `multi_axis_themes_one_collection_per_axis` | two axes in order; node-level `theme` selects per axis |
| `library_imports_resolve_present_and_report_missing` | present file: instances expand; missing: unexpanded plus one warning; cycle: refused |
| `reusable_on_any_node_type` | rectangle with `reusable` becomes a component wrapping it, exported back |
| `slot_lists_and_placeholder` | slot props created; `placeholder: true` stored in `plugin_data["pen:placeholder"]` and exported back as `placeholder: true` |
| `ellipse_arc_viewbox_absolute_flip_text_decorations_href_blend` | one case each |
| `unsupported_kinds_are_preserved_and_listed` | `prompt`, `context`, `script`, `browser` round trip byte-equal (JSON-equal) and appear in `warnings` (fail-before: empty frames, no warning) |
| `existing_pen_tests_and_depth_limits_still_pass` | `pen_component_depth_limits.rs`, `stack_wrap_and_text_wrap.rs`, the moved unit tests |

Fixture authoring rule: each fixture is written by hand from the schema notes the coordinator hands over, contains invented content only (names such as "Card", "Hero"), and is reviewed against
acceptance 4. Until the notes arrive, only the fixtures of cases that need no unknown key can be written.

Run: `cargo test -p designos-fig` (all `pen_*` files). Evidence: red/green terminal capture per case group; a rendered comparison of one fixture design before and after (headless render of the
imported document with `designos --screenshot`, showing the instance with its inherited size and look, the gradient, and an icon node), as the visual proof for a task with no UI.

## 11. Performance budgets

- Import time for a synthetic 100,000-node file (generator in the large-fixture pattern, `crates/designos-gpu/tests/large_file_perf.rs`): within 1.25x of the current importer on the same input
  (measure and record before and after); the version chain is a single in-place pass per hop.
- Memory: no full-tree `Value` clone per migration step; refs still bounded by `MAX_PEN_REF_CLONES`.
- Image reading: sequential, bounded by the caps in edge case 6; never on the UI thread (no UI path exists yet).
- Deep nesting stays iterative (`clone_component_subtree`); the existing depth tests stay green on a small stack.

## 12. Dependencies and build order

- Needs the coordinator's step list and schema notes before the migration hops and the unnamed key shapes can be written; the mechanism (rule 1), module split (rule 12), safe paths (rule 9),
  unknown-kind preservation (rule 11), instance inheritance (rule 3.1), default theme order (rule 8.2) and frame defaults (rule 4) do not wait for them.
- `designos-fig/src/pen.rs` is a hot file only for this task; no other v0.6 task edits it. E15-7 consumes `placeholder` after this lands (its scene-format test is gated on this issue).
- Instance slot filling touches the same marks as #582; land #582 first or share the helper `slot_content_marked`.

## 13. Owner rulings

- Full scope, no MVP (R&D adoption task header).
- Fixtures are ours; no third-party samples; no owner files.
- Nothing vanishes silently (delivery law 1): unsupported kinds and unresolved parts are preserved and reported.

## Open questions for the coordinator

The issue says the coordinator holds the internal reference notes and passes them on at dispatch. These are **not filled in here**; each is needed before the matching rule can be finished.

1. The **migration step list** from 1.0 to 2.21, and for each hop what changed (renamed keys, moved structures, changed value vocabularies), with the versions that carry a hop at 2.8, 2.13 and 2.17.
2. The **2.21 key names and value shapes** the issue does not name:
   - gradient: the shape of `colors` entries (position field, colour string form, alpha) and how angle, centre or handles are expressed;
   - image fill: the vocabulary of `mode`;
   - stroke: line cap and line join key names and values, the per-side shape of `strokeWidth`, and the form of a stroke given as a list of fills;
   - icon node: its key names (family, name, size, weight) and which icon families exist;
   - ellipse arc key names and units; flip key names; underline, strikethrough and `blendMode` key names and the blend-mode vocabulary;
   - `slot` list and `placeholder` shapes; `imports` and library `ref` key shapes; the value shape of multi-axis theme values and of node-level `theme`.
3. Which of today's export keys remain valid in 2.21 (rule 2), and the default `width` / `height` when absent for non-frame nodes (rule 4).
4. How a `reusable` non-frame node should map (rule 10: wrapping component is this spec's technical choice; confirm it matches the format's intent).
5. How an icon glyph is to be drawn when its icon font is not bundled (rule 7).

## Could not ground

- Any consumer of the format other than tests: the app has no `.pen` open path and no MCP tool exposes it on `origin/main@01d48f2`, although the E15 specs expect agents to exchange scenes through it.
- The public schema description the issue says to write fixtures from: it was not available to this spec; fixtures wait for the coordinator's notes.
- The pre-existing meaning of `enabled` versus `visible` and of `fill_container(px)` / `fit_content(px)` fallback numbers in 2.21 (they come from the current code, not from a 2.21 description).
