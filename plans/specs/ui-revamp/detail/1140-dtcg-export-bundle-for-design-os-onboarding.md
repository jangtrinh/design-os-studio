---
id: detail-1140-dtcg-export-bundle-for-design-os-on
type: note
title: "P A one-file, alias-resolved DTCG export per mode, so `ui ds import` onboards a kit without Figma"
updated: "2026-10-25"
status: draft
---

## Nguồn
Bản sao từ nhánh `docs/specs-untitled-ui-dark` (`docs/specs/ui-revamp-untitled-dark/1140-dtcg-export-bundle-for-design-os-onboarding.md`), lấy ngày 25/10/2026 để pack tự đủ. Nguồn chuẩn chi tiết cho US tương ứng.


| Field | Value |
|---|---|
| Issue | #1140 (epic #1118, spec key P) |
| Size | M. One to two days: unique bundle names (a data-loss fix), a single-file `--mode` export with alias resolution and a hex colour option, CLI wiring, eleven tests. |
| Depends on | none |
| Unblocks | Spec A (onboard Untitled UI dark with the design:os CLI, no Figma: R3). Any other kit onboarding that goes `designos --export-variables` → `ui ds import`. |
| Touches | `crates/designos-fig/src/variables_dtcg.rs`:<br>- `export_dtcg` `:844-1014` (group names `:848-853`, cross-collection alias refs `:921-933`, file name `:982`, manifest `:986-1011`)<br>- `export_variables` `:614-628`<br>- `resolve_literal_in_mode` `:541-567` (reused)<br>- `color_to_dtcg_obj` `:491`<br>- `zip_entries` `:109-121`<br>- tests `:2373+`<br>`crates/designos-app/src/cli.rs` (`Options` `:91-93`, `parse` `:317-324`, `USAGE`)<br>`crates/designos-app/src/main.rs` (export branch `:426-461`) |

Code baseline: `origin/main@35a65796`, also valid at `205c72ef` (newest `origin/main` on 2026-10-25): between the two, `crates/designos-mcp`, `crates/designos-fig/src` and `designos-app` `main.rs`/`cli.rs` did not change (only a new test file `designos-fig/tests/fig_effect_grid_styles.rs` was added). `variables_dtcg.rs` is identical between local `ec47da26` and `35a65796`. The `designos-app` lines were re-anchored with `git show 35a65796:<path>`. Re-anchor against HEAD and report drift before writing code. Scratch evidence for this spec, outside the repo: `$SCRATCH/agents/w5/p/`. `$SCRATCH` is the controller's scratchpad, `/private/tmp/claude-501/-Users-jangtrinh--claude-profiles-work-spares-spare-AW68Nz/a6445c24-c40e-4254-a3e2-5e0bafafd0f4/scratchpad`. The runs used a `designos` binary built from `ec47da26` (`cargo install --path crates/designos-app --bin designos --root $SCRATCH/agents/w5/bin`).

## 1. Summary and user story

As the designer agent onboarding the Untitled UI kit into `design/` without Figma (R3), I run one Studio command that writes the kit's Dark mode as one self-contained DTCG file. `ui ds import` then gets every colour role with a real value, and `ui ds a11y` measures real contrast pairs instead of reporting 0 pairs.

**Diagnosis.** The brief's gap is real (cross-file aliases dangle). Reproducing it exposed a worse bug:
- The DTCG export **silently overwrites collections that share a name**.
- The kit holds a library copy and a local copy of `_Primitives`, `1. Color modes` and `6. Typography`.
- So 4 of its 13 mode files are lost in a directory export, and the zip export fails outright.

Both are fixed here: first the data loss, then the single-file export.

## 2. Current state (FACT unless labelled)

- **Export path.**
  - `designos <file> --export-variables <dir|x.zip|x.json> --format dtcg` calls `designos_fig::export_variables` (`main.rs:426-428`).
  - It writes a zip for `.zip`; a single file only when the bundle has exactly one file (`:436`); otherwise **a directory**, even for a path ending in `.json` (`:442`).
  - Each bundle file is written with `std::fs::write`, so a second file with the same name overwrites the first (`:442-450`).
- **`export_dtcg`** (`variables_dtcg.rs:844-1014`):
  - One file per collection × mode, named `{group}.{mode}.tokens.json` (`:982`). The group is the sanitised collection name (`:848-853`).
  - Tokens sit under that group key (`:978-979`). A cross-collection alias is written as `{<target group>.<path>}`, a reference into another file (`:921-933`).
  - `tokens.manifest.json` maps each collection id to its files (`:986-1011`).
  - **Nothing makes group names or file names unique.** The figma-native exporter does (`:816-821`, `:826-833`).
- **The importer.** It walks the manifest and reads each mode file under the manifest's `group` key (`:1697-1750`). Two collections sharing a group name therefore share one file and one alias namespace.
- **Alias resolution exists.** `resolve_literal_in_mode` follows an alias chain across collections, up to 16 hops, matching modes by id, then by name, then the default (`:541-567`). The figma-native exporter already uses it for cross-collection aliases (`:751`).
- **Colours** are written as DTCG 2025.10 objects `{alpha, colorSpace, components, hex}` (`color_to_dtcg_obj`, `:491`).

**Reproduction (scratch, 2026-10-25, kit copy `$SCRATCH/uui-ws/untitled-ui.fig`):**

```text
$ designos untitled-ui.fig --export-variables dtcg --format dtcg      # summary piped through python
files listed: 13
duplicate names: {'_Primitives.Style.tokens.json': 2, '1_ Color modes.Light mode.tokens.json': 2, '1_ Color modes.Dark mode.tokens.json': 2, '6_ Typography.Value.tokens.json': 2}
$ ls dtcg | wc -l
       9                                                      # 12 mode files + manifest planned, 9 on disk
$ python3 dtcg_bundle_probe.py dtcg "1_ Color modes.Dark mode.tokens.json" bundle   # collection ids found per file
1_ Color modes.Dark mode.tokens.json: 273 tokens, collection_ids=['VariableCollectionId:5256:372339']
_Primitives.Style.tokens.json: 343 tokens, collection_ids=['VariableCollectionId:5248:377685']
6_ Typography.Value.tokens.json: 1 tokens, collection_ids=['VariableCollectionId:7856:138521']
        # the manifest also lists VariableCollectionId:13039:909, 13039:911 and 7403:16248: their files were overwritten
$ designos untitled-ui.fig --export-variables dtcg.zip --format dtcg
designos: Cannot build zip: not a fig file: zip start_file _Primitives.Style.tokens.json: invalid Zip archive: Duplicate filename: EscapeAscii { .. }
exit=2                                                        # no dtcg.zip; the message also mislabels it "not a fig file"

$ ui ds import "dtcg/1_ Color modes.Dark mode.tokens.json" --dir ds-one
ds import: 295 token(s) [273 color, 22 number] across 54 categories → …/ds-one/design
  82 roles recognized, 8 gaps: card, popover, secondary, muted, accent, input, info, neutral
  skipped 132 un-typeable token(s): 1_ Color modes-Colors-Effects-Shadows-shadow-skeumorphic-inner-$extensions-org.designos.collection_id, …
$ ui ds a11y --dir ds-one
⚠ a11y: 0 contrast pairs measurable — this store has no recognized roles and no {role}/{role}-foreground pairs; nothing was checked (false-green risk).
exit=0
```

**Which file shape does `ui` (0.6.0) accept?** Built in scratch from the export above with `$SCRATCH/agents/w5/tools/dtcg_bundle_probe.py` and `dtcg_hex_variants.py`:

| Variant | Contents | `ui ds import` | `ui ds a11y` |
|---|---|---|---|
| One file (today's Dark mode file) | aliases into `_Primitives` | 295 tokens, 82 roles | **0 pairs**, exit 0 (false green) |
| Merged | Dark mode + `_Primitives` in one file, aliases intact | 295 tokens, 82 roles | **0 pairs**: `ui` does not resolve aliases even within one file |
| Resolved | aliases replaced by literals, colour **objects** | 546 tokens, **0 roles**, 1,635 skipped (objects split into `…value.hex`/`…value.alpha`) | pairs found only through mangled names |
| **Resolved + hex** | aliases replaced by literals, colour `$value` a hex string | **273 colour tokens, 84 roles, 7 gaps** | **1,852 pairs checked, 975 below AA**, exit 1: a real measurement |
| Resolved + hex, no `$extensions` | as above | identical | identical |

Resolved + hex run, from `$SCRATCH/agents/w5/p/a11y-resolved-hex.txt`:

```text
$ ui ds a11y --dir ds-resolved-hex
ds a11y: 1852 text×surface pair(s) checked (roles inferred from token names — cartesian; prefer -foreground pairing or --pairs), 975 below AA (4.5:1).
  ✗ 1-color-modes-colors-background.bg-brand-secondary on 1-color-modes-colors-background.bg-brand-solid — 1:1 (fail); fails normal-text AA
exit=1
```

Sanity check on the resolved + hex file: `bg-primary` = `#0A0A0A` and `text-primary` = `#FAFAFA`, matching plan.md "Dark mode, resolved".

**INFERENCE:** the plan's resolved values came from the collections that survived the overwrite, the local copies. Whether the overwritten library copies held different values is UNKNOWN. Closing check: test case 3 exports both after the fix and diffs them.

**Live Figma cross-check (`evidence/figma-live-variable-check.md`, 2026-10-25 21:25).** Read-only `figma-agent exec-js` scripts were run against the open kit file in Figma. Their output was compared with Studio's `.fig` import, read through `--export-variables` and `resolve-untitled-tokens.py`:

| Check | Figma | Studio | Result |
|---|---|---|---|
| Colour variables, Dark mode and Light mode (`1. Color modes`) | 273 each | 273 each | 273/273 identical, hex and alpha |
| Spacing, radius, widths, containers (numbers) | 76 | 76 | 76/76 identical once units are normalised |
| `6. Typography` variables | 32 | 1 | **Studio's DTCG export lacks 31 of 32** |

So the colour and number values that survive the overwrite equal Figma's.

**The Typography loss is the same overwrite bug, not a separate one.** Checked in scratch (`$SCRATCH/agents/w5/p/native/`):
- The import holds all 32 variables. `designos untitled-ui.fig --export-variables native --format figma-native` exports them, because that exporter makes names unique (`variables_dtcg.rs:816-833`). It writes `6. Typography.zip` with **32 tokens (10 string, 22 number)**, in collection `VariableCollectionId:7403:16248`, local with no `source_library_key`. It also writes `6. Typography (2).zip` with **1 token**, in `VariableCollectionId:7856:138521`, the library copy.
- In the DTCG manifest, both collections map to the same file, `6_ Typography.Value.tokens.json` (group `6_ Typography`, mode `Value`). `export_dtcg` writes collections in id order, so the library copy (`7856:…`) is written after the local one (`7403:…`), and `main.rs` overwrites the file (`:442-450`). The file on disk holds that 1 token.
- Rule 1 fixes it. The local collection keeps `6_ Typography.Value.tokens.json` with its 32 tokens, and the library copy moves to `6_ Typography (2).Value.tokens.json`. Test 1's fixture already covers this shape. Dogfood step 5 checks the kit.

## 3. Behaviour rules

1. **Unique names, never an overwrite.**
   - Within one DTCG export, every collection gets a unique group name.
   - When several collections sanitise to the same group, the one without `source_library_key` (the file's local collection) keeps the plain name. The others take ` (2)`, ` (3)` and so on, in export order (collection id order, as today).
   - File names derive from the unique group: `_Primitives (2).Style.tokens.json`.
   - Cross-collection alias references use the target collection's unique group: `{_Primitives (2).Colors.Brand.500}`.
   - The manifest's `group` holds the unique name, and its `name` keeps the original.
   - `main.rs` refuses, rather than overwrites, a bundle that still contains two files with one name. The error names them and the exit code is 2. This is a guard; with rule 1 it cannot trigger.
2. **Zip works.** With unique names, `--export-variables x.zip --format dtcg` writes the zip. The zip error message no longer says "not a fig file": map it to `FigError` text that names the zip step.
3. **A round trip keeps every collection.** `--import-variables <dir>/tokens.manifest.json` of a fixed export restores every collection, each with its own tokens. The importer already reads each collection's file under its manifest `group` (`:1697-1750`). INFERENCE: no importer change is needed. I have not read whether it resolves alias references by `group` or by `name`. Test case 3 is the check that settles it.
4. **Single-file export per mode.** With `--format dtcg --mode "<mode name>"`, the output path must end in `.json`. Studio writes **one** file holding:
   - every collection that has a mode with exactly that name, at that mode
   - every collection with exactly one mode, at that mode (`_Primitives` "Style", `2. Radius` "Mode 1" and so on)
   - nothing else: collections with several modes and none of that name are left out and listed in the report under `skipped_collections`

   `--collection "<name>"` (repeatable) narrows this to the named collections. When a name matches more than one collection, the local one wins (rule 1), and the report names the others. `--collection id:<collection id>` picks one exactly.
5. **Self-contained by default.** Without `--resolve-aliases`, the single file must resolve on its own. Every collection an alias in the file points to, transitively, is added to the file at the matching mode (the same mode-matching rule as `resolve_literal_in_mode`). It is listed in the report under `inlined_for_aliases`. A test asserts that every `{…}` in the file names a token in the same file.
6. **`--resolve-aliases`.**
   - Every alias is replaced by its literal value in the requested mode, through `resolve_literal_in_mode`.
   - The token keeps its provenance: `$extensions.org.designos.alias_of = "{<group>.<path>}"` and `alias_of_id = "<variable id>"`.
   - Nothing is inlined for aliases in this mode.
   - An alias that cannot be resolved (missing target, more than 16 hops) **stops the export**: no file is written, exit 2, and the message lists up to 20 token paths with the reason. A half-resolved file would give the same false green as today.
7. **`--color-format hex|object`.**
   - `object` (the default) is today's DTCG 2025.10 object.
   - `hex` writes `"$value": "#RRGGBB"`, or `"#RRGGBBAA"` when alpha is below 1. Alpha is rounded to 8 bits; hex is uppercase, as `color_to_dtcg_obj` writes it today.
   - Valid with every DTCG export, not only `--mode`.
8. **The onboarding command** that spec A runs:

   ```bash
   designos <kit>.fig --export-variables dark.tokens.json --format dtcg --mode "Dark mode" --collection "1. Color modes" --resolve-aliases --color-format hex
   ```

9. **Report.** The CLI prints `{"format":"dtcg","path","mode","collections":[{"id","name","group","mode"}],"skipped_collections":[{"id","name","modes"}],"inlined_for_aliases":[…],"tokens":n,"aliases_resolved":n,"omitted_extensions":[…]}`. In directory and zip mode, `files` lists every file once, and the count equals the files on disk.
10. **Wrong combinations are errors**, exit 2:
    - `--mode` without `--format dtcg`
    - `--mode` with a directory or `.zip` path
    - an unknown mode name; the message lists the mode names in the file
    - `--resolve-aliases` or `--color-format` without `--export-variables`
    - `--format dtcg` to a `.json` path without `--mode`. Today this makes a directory called `x.json`; the message now says to pass a directory, a `.zip`, or `--mode`.

## 4. Data model / schema changes

None to documents. DTCG output changes:
- Unique group and file names when collection names collide; outputs without collisions are byte-identical to today.
- The new `org.designos.alias_of` and `alias_of_id` extensions, written only with `--resolve-aliases`.
- An optional hex string colour value.

`ExportBundle` gains nothing. A new `pub fn export_dtcg_mode(doc: &Document, opts: &DtcgModeOptions) -> Result<(Vec<u8>, ModeExportReport), FigError>` sits beside `export_variables`.

## 5. Contracts

- `pub struct DtcgModeOptions { pub mode: String, pub collections: Vec<String>, pub resolve_aliases: bool, pub color_format: ColorFormat }`, with `pub enum ColorFormat { Object, Hex }`. `export_dtcg` also takes a `ColorFormat`; the default `Object` keeps today's output.
- `Options` gains `mode: Option<String>`, `collections: Vec<String>`, `resolve_aliases: bool`, `color_format: Option<String>`.
- `USAGE` gains `[--export-variables PATH [--format …] [--mode NAME [--collection NAME|id:ID]… [--resolve-aliases]] [--color-format hex|object]]`.
- A token after `--resolve-aliases --color-format hex`:

  ```json
  {"$type":"color","$value":"#0A0A0A","$extensions":{"org.designos":{"id":"VariableID:…","collection_id":"VariableCollectionId:5256:372339","alias_of":"{_Primitives.Colors.Gray (dark mode).950}","alias_of_id":"VariableID:…"}}}
  ```

  The alias path shown is illustrative; the real one comes from the kit.
- A manifest entry after rule 1: `{"id":"VariableCollectionId:13039:909","name":"_Primitives","group":"_Primitives (2)","modes":[{"id":"5248:3","name":"Style","file":"_Primitives (2).Style.tokens.json"}],…}`.

## 6. File-format mapping

| Studio | DTCG file |
|---|---|
| collection name, unique per export | top-level group key (manifest `group`), the original in manifest `name` |
| alias in the same collection | `{Group.path}` (unchanged) |
| alias to another collection | `{TargetGroup.path}` using the unique group; with `--mode`, the target collection is inlined in the same file |
| alias with `--resolve-aliases` | the literal value, plus `$extensions.org.designos.alias_of` and `alias_of_id` |
| colour | `{alpha,colorSpace,components,hex}` (default) or `"#RRGGBB[AA]"` (`--color-format hex`) |

Importing a resolved file back into Studio creates literal variables, not aliases. That is expected and documented; it is not a round-trip format.

## 7. UI

None. This is CLI only, and `es-designer` is not needed.

## 8. Edge cases

- Three collections with the same name: `(2)` and `(3)` in id order. A local one keeps the plain name.
- Two collections with the same name and neither local: the first in id order keeps the plain name.
- The requested mode exists in two collections with the same name (the kit's two "1. Color modes"): both go in the file under unique groups. `--collection "1. Color modes"` picks the local one, and the report lists the other id.
- Mode names that differ only in case ("dark mode"): no match. An exact name is required, and the error lists the names.
- An alias chain whose middle collection has no mode of the requested name: `resolve_literal_in_mode` falls back to that collection's default mode, as it does today for figma-native. The report counts these as `aliases_resolved_via_default_mode`.
- A colour with alpha 0: `#RRGGBB00`.
- A number or string alias with `--resolve-aliases`: the literal number or string. Booleans keep today's form.
- A document with no variables: `--mode` gives an error ("no variable collections"), exit 2.

## 9. Non-goals

- Any change to the `ui` CLI (ease-design repository). See "Out of scope: the `ui` kernel" below.
- Changing figma-native or figma-rest exports, apart from sharing `ColorFormat` if useful.
- A DTCG "resolver" or theme file format (multi-mode in one file).
- Committing any kit export into this repository (paid kit, public repo; plan.md).

## 10. Test plan (red first)

Fixtures are built in code: two collections both named `_Primitives`, one with `source_library_key`, plus a `Color modes` collection with `Light`/`Dark` modes aliasing into the second `_Primitives`, and one collection with a single `Mode 1`. Tests live in `variables_dtcg.rs` (`mod tests`, `:2373+`) and in the `main.rs` tests.

| # | Case | Expected | Fails before |
|---|---|---|---|
| 1 | `dtcg_export_gives_same_named_collections_unique_groups_and_files` | `_Primitives.Style.tokens.json` (local) and `_Primitives (2).Style.tokens.json`. Manifest groups are unique, and aliases into the library copy say `{_Primitives (2).…}`. | yes |
| 2 | `dtcg_zip_export_with_same_named_collections_succeeds` | the zip holds 1 + 5 entries | yes (duplicate filename) |
| 3 | `dtcg_round_trip_keeps_every_collection` | export to a directory, then import the manifest into an empty document. Same collection ids. Collection names come back from the manifest `name`, not `group` (no `_Primitives (2)` after import). Same variable count per collection. A `{_Primitives (2).…}` alias resolves to the library copy's variable after import. Same resolved Dark `bg-primary`. | yes |
| 4 | `cli_directory_export_never_overwrites_within_one_run` | `files` count = files on disk | yes (13 vs 9 on the kit) |
| 5 | `mode_export_is_one_self_contained_file` | every `{…}` in the file resolves inside the file; `inlined_for_aliases` lists `_Primitives (2)` | yes (no flag) |
| 6 | `resolve_aliases_writes_literals_and_keeps_provenance` | Dark `bg-primary` is a literal; `alias_of` equals the old reference string | yes |
| 7 | `color_format_hex_writes_strings_with_alpha` | `#0A0A0A` and `#FFFFFF80` | yes |
| 8 | `unknown_mode_is_an_error_listing_modes` | message lists `Light`, `Dark`, `Style`, `Mode 1` | yes |
| 9 | `unresolvable_alias_stops_the_export_and_writes_nothing` | alias to a deleted variable: exit 2 / `Err`, no file | yes |
| 10 | `dtcg_to_a_json_path_without_mode_is_an_error` | `Err`, and no directory named `x.json` is created | yes (creates a directory) |
| 11 | `collection_filter_prefers_the_local_copy` | `--collection _Primitives` picks the local one and reports the library id | yes |

Run:
- `cargo test -p designos-fig variables_dtcg`
- `cargo test -p designos-app --bin designos variables`
- `cargo test --workspace --locked`
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`

**Dogfood acceptance (PR evidence, terminal capture on the PR build with a copy of the kit, outside the repo):**
1. `--export-variables dtcg --format dtcg`: `files` lists 13 unique names, and `ls | wc -l` = 13.
2. `--export-variables dtcg.zip --format dtcg`: exit 0.
3. The onboarding command of rule 8, then `ui ds import dark.tokens.json --dir <scratch>` and `ui ds a11y --dir <scratch>`:
   - import reports at least 80 roles recognized
   - a11y reports more than 0 pairs checked
   - `bg-primary` is `#0A0A0A`
4. Diff the Dark values of the local and the library "1. Color modes" (from step 1) and paste the count of differing tokens.
5. `6_ Typography.Value.tokens.json` from step 1 holds 32 tokens (10 string, 22 number), matching Figma's 32 (`evidence/figma-live-variable-check.md`). `6_ Typography (2).Value.tokens.json` holds the library copy's 1 token.

## 11. Performance budgets

- The export runs after the document is loaded, so the kit load (about 15–60 s, measured in this session) dominates. The export itself: under 1 s for the kit's 10 collections and about 950 variables (INFERENCE; closing check: `time` in the PR).
- `resolve_literal_in_mode` is at most 16 hops per alias, which is linear in the number of tokens.
- No UI or idle cost; CLI only.

## 12. Dependencies and build order

Independent. **It must merge before spec A runs its onboarding**, or A has to onboard from hand-merged files.

Order:
1. Unique groups and file names, with tests 1–4.
2. Wrong-combination errors (test 10).
3. `ColorFormat` (test 7).
4. `export_dtcg_mode` (tests 5, 6, 8, 9, 11).
5. CLI flags and report.
6. Dogfood evidence.
7. PR (squash).

## 13. Owner rulings that apply

- R3 (2026-10-25): Studio improves itself; no Figma. Onboarding goes through Studio's CLI and the design:os CLI.
- plan.md: the raw kit export is not committed (paid kit, public repo).
- `docs/releasing.md` law "Nothing vanishes silently", as carried in the delivery workflow. The overwrite in rule 1 is exactly that kind of silent loss.
- 2026-09-22: visual evidence in every PR. Standing: `main` is protected.

## Decisions on the open questions (writer, 2026-10-25)

1. **Bundle with primitives inlined, or `--resolve-aliases`?** Both, because the scratch probe settled it:
   - `ui` 0.6.0 does not resolve aliases even inside one file (merged variant: 0 pairs), so onboarding needs `--resolve-aliases`.
   - The self-contained default (rule 5) is still the right DTCG output for other tools, and costs little.
2. **Hex strings?** Yes, as an option. `ui` splits DTCG 2025.10 colour objects into `…value.hex` tokens and recognises 0 roles. The hex variant recognises 84 roles and measures 1,852 pairs. The default stays the 2025.10 object.
3. **Which duplicate keeps the plain name?** The local collection, because aliases inside the file point at local variables first and the user edits those. The library copy is a mirror.
4. **Refuse or half-write on an unresolvable alias?** Refuse. A file with a dangling alias is what produced today's false green.

## Out of scope: the `ui` kernel (ease-design repository, follow-up issue there)

Exact gaps, each reproduced above:
1. `ui ds import` reads one file and does not resolve `{alias}` references, not even to tokens in the same file (merged variant: 82 roles, 0 pairs).
2. `ui ds import` does not read DTCG 2025.10 colour objects `{colorSpace, components, alpha, hex}`. It flattens them into `…value.hex`/`…value.alpha` tokens (resolved-object variant: 546 tokens, 0 roles).
3. `ui ds import` walks into `$extensions` and reports its fields as "un-typeable tokens" (132 and 1,635 skipped). Keys starting with `$` should be ignored.
4. `ui ds a11y` with 0 measurable pairs prints a warning but **exits 0**. A check that measured nothing should fail.
5. `ui ds a11y` infers pairs as a cartesian product of names (1,852 pairs, including `bg-*` on `bg-*` at 1:1). It needs Untitled's `text-*`/`fg-*` on `bg-*` role pairing, or the `--pairs` option documented for spec A.

## Open questions for the owner

None. These are CLI flags and a bug fix, with no visible product change.

**Note to the controller:** plan.md says the DTCG export "writes 9 files". That count is the overwrite bug: 13 were written, and 4 were overwritten. The plan's resolved Dark values come from the surviving local collections. Spec A should re-derive them after this fix (test-plan step 4).

**Changelog:** 2026-10-25 (round 2): added the live Figma cross-check (colours 273/273 and numbers 76/76 match; Typography 32 vs 1) and the finding that the Typography loss is the same overwrite bug (local `7403:16248` with 32 tokens overwritten by library `7856:138521` with 1 token, figma-native export as proof); dogfood step 5 added. Relabelled valid at `205c72ef` earlier the same day.
