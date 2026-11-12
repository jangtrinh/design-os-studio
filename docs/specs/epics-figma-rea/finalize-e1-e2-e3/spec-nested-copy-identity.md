# Spec: identity of a layer inside a nested component copy (v0.7.14)

Base `origin/main` at e0e44f3c2, 2026-11-12. Research, diagnosis and spec only; no product code committed. All raw
outputs are under `evidence/` next to this file (paths below are relative to the worktree root).

Outcome in one paragraph. Figma identifies a layer inside a (nested) instance by a path of authored-layer ids, one per
instance level, each resolved inside that level's own main; it never stores the copy's layers. Studio stores the copy
and links each copy layer to the layer one level up. That model is sound; #1925 comes from one importer line that
resolves a Studio-written override key against the whole file instead of against the level it belongs to (proven by
instrumentation). A Figma-authored file is not affected (0 of 759,796 copy layers mislinked in the kit), but the kit
saved by Studio as `.fig` and reopened has **321,148 of 759,796** copy layers mislinked. The fix (slice S1) was
prototyped: mislinked 321,148 -> 0 on that round trip, the #1925 table goes from lost to kept in every cell, older
native files are repaired on load, the Figma-authored kit is identical per instance, and on the round-tripped kit no
instance gets worse while 13 no-change syncs and 3 switch round trips that changed something now return exactly. #1919 is a different cause (Studio overwrites the layout sizes Figma
stored for copy layers); its design follows from Figma's "derived override" model but was not prototyped. #1909 hit
the stop rule: two prototypes of "relink instead of rebuild on a switch back" both broke kit instances; the fork is in
section 6.

## 1. What could NOT be verified (read first)

1. **No runtime observation of Figma.** Nothing ran in the Figma app (no account, no scratch file). Engine behaviour is
   read from the engine's own strings, mapped to functions by a string cross-reference; file-format behaviour is read
   from a Figma-authored file (the Untitled UI kit). Every "inferred" row in section 2 needs a runtime probe to become
   "seen".
2. **REA could not open the engine.** `rea inspect-wasm-artifact` refuses modules over 8 MiB (the engine is 53.9 MB;
   `evidence/re/00-sources.txt`, `evidence/re/rea-inspect-wasm-refusal.json`). I used my own clean-room string
   cross-reference instead (`evidence/re/scripts/wasm_string_xref.py`). No function body was disassembled or read.
3. **Unknown in Figma, settled by nothing static:** (a) whether an override on a layer the other variant lacks comes
   back after a switch there and back (#1915 part 2); (b) the exact text box size of an overridden auto-resize text on
   the *intermediate* variant (#1918 mid-state); (c) the per-axis size rule for a resized instance on a swap to a main
   of another size. Each is marked UNKNOWN below with the probe that would settle it.
4. **#1919 design not prototyped.** It needs a document-model field and changes in three sync passes; a partial hack
   would pass the size-only harness while transforms drift (the harness digest has no transform). Its kit target is
   therefore a requirement, not a measured result.
5. **Per-cause counts for #1915–#1918 are the triage's** (`triage-kit-variant-switch-there-and-back.md`, measured at
   3d3892d3f). At e0e44f3c2 I re-measured only the totals (59 and 36, identical); line numbers were re-read at
   e0e44f3c2.
6. **#1887 and #1888** were not reproduced; their causes are taken from the issue text plus a code read (marked).
7. **Timings are noisy.** Other agents were compiling on this Mac. Import and native-load times are the minimum of three
   runs (`evidence/out/kit_import_timing-*.txt`); hydration varied 22–30 s between identical builds.

## 2. Figma facts

Tags: SEEN = observed in Figma's file data or engine strings (evidence pasted or pointed to); INFERRED = reasoned from
seen facts; UNKNOWN = not settled.

### 2(a) How a layer inside a nested instance is identified

| # | Tag | Fact | Evidence |
|---|---|---|---|
| A1 | SEEN | A Figma file stores **no layers for an instance's copy**: 0 of 42,216 INSTANCE nodes in the kit have a child node. | `evidence/re/03-figma-file-identity-counts.txt`: `instances_with_child_nodes_in_file=0` |
| A2 | SEEN | An override and a derived value are stored on the top instance as entries keyed by `guidPath` (array of GUIDs). Kit: 123,167 override entries (100,266 of length 1, 17,879 length 2, 4,912 length 3, 110 length 4) and 565,783 derived entries (lengths 1–6). | schema `GUIDPath { guids }`, `SymbolData.symbolOverrides`, `NodeChange.derivedSymbolData` (`crates/designos-fig/src/figma_schema.txt:1007-1012,1400,1404`); counts in 03 |
| A3 | SEEN | **One path step per instance level, each resolved inside that level's own main** (the nested instance's main, or the main a property assignment or swap picked for it). Resolving that way, 119,729 of 123,167 override paths (97.2 %) and 563,996 of 565,783 derived paths (99.7 %) resolve completely. A step is never resolved against a copy (there are none, A1). | 03: `override fully_resolved=true = 119729`, `step1 resolved_in_sibling_variant (variant picked by property) = 563`, `step1 resolved_in_other_symbol (instance swap) = 1302`, `next_level_via_swap = 6440` |
| A4 | SEEN | The instance's own overrides (its size, padding, style links) are the entry whose path is just the main's GUID: 36,403 entries. | 03: `own_entry_of_instance_root (path = its main) = 36403`; fields mostly `size`, `stackPrimarySizing` |
| A5 | SEEN | In this Figma-authored kit every path step is a GUID; no node carries an `overrideKey` (0 of 119,748). The engine has the field and a repair routine for it. | 03: `nodes_with_overrideKey=0`; schema `overrideKey = 213 : GUID`; strings `Repair missing override keys` (49805909), `repairStateGroupsWithMissingOverrideKeys` (49806250), `setOverrideKey should no longer be called on IF sublayers` (49822482) in `evidence/re/02-engine-strings-cited.txt` |
| A6 | SEEN (strings) / INFERRED (meaning) | In memory an instance sublayer is a materialized node that knows its backing layer and is looked up by override key/path in the backing node: `Can't find child with override key in backing node`, `Can't find child with override path in backing node`, `getBackingNodeSublayerForInstanceSublayer`, `Source/derived instance sublayer backingLayerID mismatch`, unit `derived-subtree/FGDerivedSubtreeMaterializer.cpp`, `spec-impl/FGInstanceSpec.cpp`. INFERRED: the in-memory identity is the same per-level path as in the file. | `evidence/re/02-engine-strings-cited.txt` |
| A7 | SEEN | A Studio-written `.fig` differs from Figma's on exactly this point: it writes the copy's layers as nodes (6 of 6 instances have children) and gives 6 of them an `overrideKey` that differs from their GUID (the authored layer in the innermost main). | `evidence/re/04-studio-written-file-counts.txt`; writer `crates/designos-fig/src/fig_export.rs:1507-1511` (`overrideKey = authored_source_guid`) |

### 2(b) What happens to overrides when a main is edited

| # | Tag | Fact | Evidence |
|---|---|---|---|
| B1 | INFERRED from A1–A3 | The copy is re-materialized from the backing main and the path-keyed overrides are applied again. An override is lost only when its path no longer resolves; nothing is "rebuilt without overrides", because overrides never lived on copy nodes. | A1, A2, A3 |
| B2 | SEEN | Overrides whose path no longer resolves stay in the file: 3,438 override entries (2,965 at the first step) in the kit point at no current layer. The engine names them: `Component customizations: ignored stale override paths` (func 53698), and an instance property `unappliedSublayerOverridePaths`. | 03 `override fully_resolved=false = 3438`; `evidence/re/01-engine-strings-by-function.txt` (func 53698, func 15011) |
| B3 | SEEN (strings) / INFERRED | Derived data is recomputed by an updater for dirty inputs: `DerivedDataUpdater` (49926075), `fullscreen/lib/scenegraph/derived-subtree/FGMixedDirtyOriginDerivedSubtreeRepairer.cpp` (49957641), analytics `is_move_update`. INFERRED: an instance whose inputs did not change keeps its derived values. | `evidence/re/02-engine-strings-cited.txt`, func 30337 in 01 |

### 2(c) Variant switch and swap

| # | Tag | Fact | Evidence |
|---|---|---|---|
| C1 | SEEN | Transfer maps an **old override path to a new path**, field by field, **separately per inheritance level**, for overrides and for derived values: `oldPath`, `newPath`, `Override path successfully remapped`, `remapOverridesForInheritanceLevel`, `remapDerivedOverridesForInheritanceLevel`, `Derived override transferred` / `not transferred`, `Override transferred` / `not transferred` (func 12511, 10819). Analytics row: `override_level`, `old_path`, `path_transferred`, `path_remapped`, `field_transferred`, `field_modified`, `used_subtree_matcher` (func 30337). Schema: `NodeChange.overrideLevel = 321`. | 01 |
| C2 | SEEN | Variants of one set **never share layer ids**: of 23,909 same-name-path layer pairs across variants of 573 sets, 0 share a GUID or override key. So a switch cannot match by id; it matches by name and hierarchy: `computeInstanceNameHierarchyMatchingHelper` and `getNameIgnoringOverrides` in `FGSubtreeMatcher.cpp` (funcs 14759, 28557, 28564, 28565). | 03 last line; 01 |
| C3 | SEEN (help) + SEEN (strings) | "Originally matched" gate: a changed field carries to the new variant only if the old and new variants' own values were equal. The engine's gate (`checkInitialValue`, func 12512) has messages for `overriddenSymbolID`, `autolayout field`, `paint field` and a generic `Initial value check failed`, so which other fields it gates is not settled by strings. Help: text changes are kept when the text layer's name is the same ("the criteria is less strict"); D-9 rests on that sentence. | 01 func 12512; help quotes in `re-1824-nivb-evidence/03-public-sources-quotes.txt` §4 and `researcher-261031-1130-figma-instance-swap-rules.md` Q1, Q5 |
| C4 | SEEN | Single text rule: an untransferred text override moves to the only text layer when the instance had exactly one text node and the new main has exactly one; it does not apply when the override was hoisted. Strings: `Single text rule applied` and three `did not apply` reasons (func 12511). | 01 func 12511 |
| C5 | SEEN | Inner swaps are remapped or hoisted: `remapOverridesThroughTransferredInnerSwaps`, `...UntransferredInnerSwaps`, `Hoisting overrides on inner swap` (func 12511). | 01 |
| C6 | SEEN (strings) | A dropped override is a recorded outcome, not silence: `Override not transferred`, `dropped_override_count` (49764890), and for the variable-driven path `(W01) NIVB: Overrides are being dropped`. | 01, 02 |
| C7 | UNKNOWN | Whether an override on a layer the other variant lacks comes back after switching back. B2 shows Figma keeps unresolved paths on an instance after a main edit; whether a switch keeps untransferred entries was not settled. Probe: in Figma, override a layer only variant A has, switch to B and back, inspect the layer. | — |
| C8 | INFERRED | Size of a resized instance's children after a switch: children follow their constraints for the kept size (Plugin API `resize`: "If the node contains children with constraints, it applies those constraints during resizing"), and derived values are re-derived (B3). | researcher-261031 Q4 |
| C10 | SEEN (file) / INFERRED (switch) | The instance's own size: a resized instance stores it as its own override entry (path = its main, field `size`; A4, 36,403 own entries, `size` the most common field), and the transferrer remaps overrides per path and level (C1). INFERRED: on a variant switch the own `size` override carries like any other field, subject to the gate of C3; whether the size gate compares the two variants' sizes is UNKNOWN (generic gate string only). | 03, 01 |
| C9 | UNKNOWN | The text box size of an overridden auto-resize text on the intermediate variant (keep the old box, or measure the text again). The round trip back to the original variant ends at the original size under either rule. | — |

### 2(d) Can an update with no change to the main change sizes?

| # | Tag | Fact | Evidence |
|---|---|---|---|
| D1 | SEEN | Figma stores the layout result of instance sublayers per instance: 376,911 derived `size` entries, 125,454 `transform`, 249,332 `fillGeometry`, 123,476 `strokeGeometry`, 84,425 `derivedTextData`, plus padding, spacing, corner and border values. | 03 `derived_field ...` |
| D2 | SEEN | Figma's transferrer treats these as a category of override ("derived override") with its own remap per level (C1). | 01 func 12511 |
| D3 | INFERRED | With no change to the main, no derived input is dirty (B3), so no size changes. Not observed at runtime. | B3 |
| D4 | SEEN (Studio side) | On the kit, every top-level-pick sublayer whose size a no-change sync altered had been imported exactly at Figma's derived size (43 of 43 with a derived entry); after the sync 36 of them sit at the reference (main) size, 7 at a re-solved size. | `evidence/out/attribution-noop-base.txt` |

## 3. Test vectors from Figma (for the build workers' tests)

Each vector says what Figma does and on what evidence; write the Studio test so that it asserts this outcome.

| TV | Case | Expected | From (tier) |
|---|---|---|---|
| TV1 | Main A holds a Shape; main I holds a copy of A; main O holds a copy of I; a page instance T of O. Override the fill (and opacity, ring arc) of T's deep Shape. Save as `.fig` (Studio), reopen, edit A (corner radius, or the same field), sync A, I, O. | The override stays on T's deep Shape, same node id, flag kept, at depth 1 and depth 2. A follower instance takes the main edit. | A2, A3 seen; B1 inferred |
| TV2 | Same as TV1, but the file was imported by the old build and saved as native JSON (older native file), then opened by the new build. | Same as TV1; the load reports how many links it repaired. | A3 seen; B1 inferred |
| TV3 | A wrapper main holds two copies of the same inner main (`lead`, `trail`); `trail` is swapped on the page copy. Studio-written `.fig` round trip, then a wrapper sync. | Still exactly two copies, `trail` still swapped; no layer duplicated. (Two copies of one main are two authored layers, so two path steps.) | A3 seen |
| TV4 | Two variants with a same-named text layer and different default texts; override the text, switch to the other variant and back. | Text override kept on both legs. | C2 seen; C3 help text (seen), engine side inferred |
| TV5 | Two variants whose same-named rectangle has different fills in the mains; override the fill, switch. | The fill override is not carried (originally-matched gate). | C3 seen (help example, paint gate string) |
| TV6 | Old variant has exactly one text node, new variant has exactly one text node with another name; override the text, switch. | The text moves to the only text node. With two text nodes in the new variant it does not move. | C4 seen (strings) |
| TV7 | Override a layer only variant A has; switch to B. | The override is reported as not transferred (a count, never silent). Whether it returns on switching back: UNKNOWN (C7); do not assert it. | C6 seen (strings); C7 unknown |
| TV8 | An instance resized to half its main's size, child with Scale constraints; switch variant. | Child is half the new variant's child size (fitted to the kept instance size). | C8 inferred |
| TV9 | Import a `.fig` whose instances carry derived sizes; sync their mains with no change. | Nothing changes: sizes, transforms, visibility, fills, flags, layer count. | D1 seen; D3 inferred |
| TV10 | Same as TV9, then change one unrelated field (a fill) of a main. | Only the fill changes on its instances; derived sizes and transforms of untouched layers stay. | B3, D3 inferred |

## 4. Studio's proven causes (file:line at e0e44f3c2)

### #1925 (P1) — PROVEN by instrumentation

- Writer: `crates/designos-fig/src/rest_import.rs:2205-2214`, `link_sources` **first loop, override-key branch**. For a
  Studio-written `.fig`, each copy layer exists as a node with `overrideKey` = its authored layer in the innermost main
  (`fig_export.rs:1507-1511`). The loop resolves that key through `self.ids` against the whole file and links the copy
  layer to that main layer. The second loop (`:2218-2245`) never overwrites it (`source.is_none()` guard).
  Instrumented output (`evidence/out/studio_nested_links.txt`, patch `evidence/patches/instrument-source-writers.patch`):
  ```
  LINKS fig-reopen depth=2: NodeId(12) 'I' depth=1 source=NodeId(8)('I' in 'O') writer=link_sources.loop1.override_key link=Some(OneLevelUp)
  LINKS fig-reopen depth=2: NodeId(13) 'A' depth=2 source=NodeId(5)('A' in 'I') writer=link_sources.loop1.override_key link=Some(MainLayerOtherLevel)
  LINKS fig-reopen depth=2: NodeId(14) 'Shape' depth=3 source=NodeId(3)('Shape' in 'A') writer=link_sources.loop1.override_key link=Some(MainLayerOtherLevel)
  ```
  This refutes the earlier inference that the second loop writes these links.
- Effect: `crates/designos-doc/src/components.rs:2390-2401` (`sync_pair`) treats a child whose `source` is not one of the
  reference's children as stale and removes it; `:2406-2416` rebuilds it with `copy_tree` and no overrides.
- Kit (Figma-authored): **0 of 759,796** copy layers mislinked; all 759,796 were written by `expand_instance` cloning
  (`rest_import.rs:2846`) one level up (`evidence/out/studio_nested_links-kit.txt`). #1925 is specific to Studio-written
  `.fig` files and to native files saved after opening one.
- Older native files: FACT, they lose the override too (`evidence/out/older_native_file-base.txt`: `depth=1 KEPT=false`,
  `depth=2 KEPT=false`), because the native save keeps the wrong links.

### #1919 (P1) — REPRODUCED 36 of 200 at e0e44f3c2, cause attributed

- Count: `evidence/out/harness-base.tsv` `SUMMARY noop_differ=36` (31 with a layer size change, 5 without; 12 instance
  roots snapped to their main's size, 19 where only sublayers changed).
- Cause 1 (sublayers): the importer writes Figma's derived sizes into `node.size` without recording that they are layout
  results (`rest_import.rs:2977` `apply_fast_derived`, `:2905` `resize_override_node`; the `explicitly_sized` set is local
  to one call). `sync_pair` then calls `apply_reference` (`components.rs:2296-2301`), whose loop copies every non-overridden
  field from the reference (`components.rs:1525` `copy_field(f, r, n)`), Size included, so the Figma value is replaced
  by the reference's size; the constraint pass at the end (`components.rs:2430-2433`) runs only when the instance's own
  size differs from its reference and re-derives with Studio's solver. Attribution: 43 of 43 top-level-pick layers with a
  derived entry were at Figma's value before the sync; 36 end at the reference size, 7 at another re-solved size.
- Cause 2 (roots): 12 roots (11 nested picks, 1 top-level) without a Size flag take their reference's size in the same
  `apply_reference` loop.
- 5 instances change without a size change (visibility or fills). Not attributed (UNKNOWN); slice S2 must classify them.

### #1909 (P1) — mechanism confirmed, fix not found (fork, section 6)

`swap_main` rebuilds the copy from the variant main's own layers (`components.rs:1762-1768`, `copy_tree(.., link=true)` at `:1764`), so
their links point into the variant main. When the switch lands on the variant the reference itself shows,
`sync_pair` keeps the reference (`:2250`), matches none of its children (`:2392`), removes the copy's children (`:2401`) and
rebuilds them (`:2410`). Swap_main also folds the outer main's customizations into the copy's own flags
(`effective_child_overrides`, `components.rs:1627-1686`: `chain_fields` and differences from the variant's own layer), so
after a round trip the copy cannot tell its own overrides from the outer level's.

### #1915 (P1)

`swap_main` carried-override loop, `components.rs:1890-1895`: when no layer of the new copy has the old name path (and
the single text rule does not apply) the override is skipped with `continue` and nothing counts it; `:1896-1898` skips a
second override for an already used target the same way. Inner swaps are counted (`:1872`), overrides are not.

### #1916 (P1)

`components.rs:1918-1921`: inside one component set, `transfer_gate_diff` (`:1966`) removes `Characters` and `TextStyle`
whenever the two variants' own text layers differ; the exemption covers only a swap to another family
(`!same_family && ...`). Figma's help keeps text changes when the layer name matches, whatever the default texts (C3).

### #1917 (P2)

Two skips: `swap_main` `:1805` (`!(top_swap && same_family)`: no constraint pass for a top-level variant switch) and
`sync_pair` `:2430` (`!(top && swapped_here)`). The triage trial lifted both when the root carries a Size override.

### #1918 (P2)

`copy_overridden` (`components.rs:1257`, called at `:1928`) carries the characters but not the text box size, and
`keeps_own_text_size` (`:1415`) then keeps the new variant's box for overridden auto-resize text.

### #1887, #1888 (not reproduced; issue text + code read)

- #1888: `apply_overrides` (`rest_import.rs:2948-2975`) applies stored derived data when a deferred page is hydrated,
  without the main-fingerprint check that the save path uses (`instance_fingerprints`, consulted only at `:3971`).
  FACT by code read.
- #1887: `create_instance` copies a main whose nested instance sits on an unopened page and is still childless; the
  pending record is keyed by the original instance, not by the new copy. INFERENCE from the issue text; not reproduced.

## 5. Design

Each decision cites a Figma fact (section 2) or is marked Studio-only with why it is safe.

| # | Decision | Basis |
|---|---|---|
| D-1 | Keep Studio's representation: copy layers are nodes, each linked to the layer one level up (`component.source`). The chain of links is Studio's encoding of Figma's per-level path: following a copy layer's links to the first layer of a main gives the path step of that level. | A3 (one step per level, resolved in that level's main). Studio-only: storing copy nodes; safe because the export already writes paths this way (`authored_source_guid`). |
| D-2 | A `.fig` override key (or any path step) is resolved **inside the level it belongs to**: for a copy layer whose parent compares against reference R, the link is the child of R that stands for the same authored layer; never a node found by a file-wide id lookup. | A3, A6 |
| D-3 | "Stands for the same authored layer" = follow `source` until the next step would reach a main component root (`authored_layer`). Twins (two copies of one main) are two authored layers and never merge. Ambiguous matches are left alone. | A3; the `source_root` regression in `opus-review-1824-r2.md` (Blocker 1) is exactly a merge of two steps. |
| D-4 | The importer applies D-2 to every instance after linking, inner instances first, until no link changes (bounded). On the Figma-authored kit this changes nothing (all links are already one level up). | A1, A7 |
| D-5 | Native files saved by an older build are repaired once on load by the same pass, counted in the load repair report (never silent). Studio-only (file migration): safe because it only relinks a layer to a reference child that stands for the same authored layer and leaves values and flags alone; a persisted marker (`copy_links_checked`) keeps the cost off files the fixed build saved. |
| D-6 | Layout values Figma stored for copy layers (sizes, transforms, outlines, text layout) are a separate kind of value, kept while the inputs they were derived from are unchanged, and re-derived when an input changes. A no-change sync therefore changes nothing; an edit of an unrelated field changes only that field. | D1, D2, B3. The input test is Studio-only: per layer, record the reference size and transform it was derived against and the parent's size; equal = unchanged. Safe: it only skips a re-derivation whose inputs are identical. |
| D-7 | Hydrating a deferred page applies stored derived data only for instances whose mains still fingerprint as at load (the same check the save path makes); otherwise it re-derives. | D1, B3 (derived data is only valid for its inputs) |
| D-8 | A swap or switch reports every override it cannot carry in the existing dropped count (like inner swaps). | C6 |
| D-9 | Text overrides (`Characters`, `TextStyle`) are not subject to the originally-matched gate inside a set; name-path matching and the single text rule still apply. | C3, C4 |
| D-10 | After a switch that keeps a resized instance's size, its children are fitted to that size by their constraints from the new main's geometry. | C8 |
| D-11 | An overridden auto-resize text that maps to a layer of the new variant brings its derived size along when the text style is the same (derived override transfer); with another style it is measured again. Only the round trip is asserted. | C1, D2; the mid-state is C9 UNKNOWN |

Rejected options and why: relinking at the end of `swap_main` (the #1824 R4 attempt) or relinking every stale child in
`sync_pair`, with or without an equal-value flag prune (measured to break kit instances, section 6); "match by `source_root`" (merges twins; contradicts A3: two copies are two steps);
"importer-only fix" (older native files stay broken: `older_native_file-base.txt`); "skip a sync when the main's
fingerprint is unchanged" (a sync also serves mode and variable changes; it would hide the drift of TV10 rather than fix
it, which contradicts B3's per-input recompute).

### Prototype (one, measured, reverted, kept as a patch)

Patch: `evidence/patches/prototype-identity-v3-importer-and-load.patch` (D-2 … D-5: `authored_layer`,
`relink_copy_layers_to_reference`, the importer pass in `link_sources`, and the load pass in `finish_load`; the marker
of D-5 was not prototyped, so the load pass ran on every load).

| Check | Base e0e44f3c2 | Prototype v3 | Raw |
|---|---|---|---|
| #1925 table, `KEPT=true/false` per depth (Session, Native, Fig, Fig-then-native) | d1: 12/0, 12/0, **0/12**, **0/12**; d2: 18/0, 18/0, **0/18**, **0/18** | d1 and d2: all kept (12/0, 18/0 in every medium); d0 6/0 both | `evidence/out/probe_deep_override_reopen-{base,v3}.txt` |
| Older native file (written by the base build), main edit + sync | depth 1 lost, depth 2 lost | depth 1 kept (1 link repaired), depth 2 kept (3 links repaired) | `evidence/out/older_native_file-{base,v3}.txt` |
| Kit: no-change sync, instances altered of 200 | 36 | 36, **identical per instance** (0 of 200 after-states differ from base) | `evidence/out/harness-{base,v3}.tsv`, `compare-base-vs-v3.txt` |
| Kit: switch there and back, instances not returning of 200 | 59 | 59, **identical per instance** | same |
| Kit saved by Studio as `.fig` and reopened: copy layers not linked one level up | **321,148 of 759,796** | **0** | `harness-{base-rt,v3-rt}.log` |
| Same round trip, no-change sync altered of 200 | 47 | 34: **13 now unchanged, 0 newly changed**, 9 with fewer differing lines | `compare-base-rt-vs-v3-rt.txt` |
| Same round trip, switch there and back not returning of 200 | 58 | 55: **3 fixed, 0 broken**, 5 with fewer differing lines | same |
| Kit: total layers | 877,261 | 877,261 | `harness-*.log` |
| Kit: `.fig` import, min of 3 | 9.75 s and 10.70 s (two runs of the same base build: noise about 10 %) | 10.80 s (inside that noise) | `kit_import_timing-{base,base2,v3}.txt` |
| Kit: native load (5.0 GB JSON), min of 3 | 11.87 s and 11.14 s | 15.34 s (**+29 % to +38 %**, the unconditional load pass; it relinked 0 layers on this Figma-derived file) | same |
| Kit: no-change sync time, 200 picks | 6.10 s | 6.41 s | `harness-*.tsv` SUMMARY |

So: the identity design is verified (0 regressions on 200 kit instances in both the Figma file and its Studio round
trip, every #1925 cell fixed, 321,148 mislinked layers repaired), but the
no-change-sync target (0 of 200) is **not** reached by it: #1919 is a different cause (section 4) and needs slice S2.
The load-pass cost must come down (D-5 marker; slice S1 acceptance 6).

## 6. The #1909 fork (stop rule hit)

Two prototypes of one idea, "a copy layer that stands for the same authored layer as a reference child is relinked
instead of rebuilt when `sync_pair` would remove it":

| Prototype | Patch | Kit there-and-back vs base, per instance |
|---|---|---|
| v1: relink only | `evidence/patches/prototype-identity-slices-a-b.patch` | 1 fixed, **33 broken** (all nested; extra flags such as `Stroke`, `Fills`, `Characters` with unchanged values; one text 98 → 32 wide) |
| v2: relink and drop flags whose value equals the reference layer's | `evidence/patches/prototype-identity-slices-a-b-v2.patch` | 1 fixed, **3 broken** (`Stroke` flag and value from the variant main instead of the outer main), 4 changed (text sizes; one improved from 65 to 4 differing lines and no lost layers) |

Raw: `evidence/out/compare-base-vs-protoAB*.txt`, `harness-protoAB*-dump.log.redacted` (names and texts redacted).

Why it fails (FACT from the dumps plus code read): Studio keeps one flag set per copy layer. `effective_child_overrides`
folds the outer main's customizations into it on the first leg, so on the way back the copy cannot tell its own overrides
from the outer main's. Figma keeps them apart (C1: transfer per inheritance level; `overrideLevel` in the schema).

Options for the owner (or a kongming ruling; I did not escalate, per the brief):
1. **Per-level flags (recommended, Figma's model).** A copy layer records which of its carried values came from an outer
   level (for example `component.inherited_overrides`), set by `swap_main` from `chain_fields` and from differences with
   the variant's own layer; a relink drops only those. Cost: a document field, `swap_main`, the relink, export of the
   carried values. Needs a new prototype on the kit before a build.
2. **Accept v2.** Fixes the six named instances only partly (1 of 59 returns), breaks 3 with a visible stroke change.
   Not recommended.
3. **Leave #1909 open** and ship S1, S2, S4–S7 first.

## 7. Build slices (one pull request each)

Ownership: S1 owns a new file `crates/designos-doc/src/copy_layer_links.rs` plus `rest_import.rs::link_sources`,
`lib.rs` (`finish_load`, `mod`/`pub use` lines) and `load_repair.rs`. S4→S7 run in this order on `components.rs` (one
worker, four PRs, or four workers strictly in sequence). S2 starts after S1 and S7 are merged (it touches
`components.rs` and `rest_import.rs`). S8 last. S1 and S4 can run in parallel.

Every slice: tests written red first and shown failing; gates per the common rules; the kit harness
(`evidence/probes/kit_nested_identity_harness.rs`, runner `evidence/scripts/run-kit-harness.sh`) compared **per
instance** against base with `evidence/scripts/compare-harness-runs.py` (a total alone is not accepted: the #1824 R4
regression hid inside an unchanged total).

### S1 — #1925: resolve copy-layer links level by level (closes #1925)

Files: new `crates/designos-doc/src/copy_layer_links.rs` (`authored_layer`, `relink_copy_layers_to_reference`, and its
own copy of the rule for which node a sync compares a parent's children against; S1 does not touch `components.rs`, so
it can run beside S4; S2 may later make `sync_pair` call the shared rule), `crates/designos-doc/src/lib.rs`,
`crates/designos-doc/src/load_repair.rs` (a non-structural `copy_links_relinked` count), document marker
`copy_links_checked` (serde default false), `crates/designos-fig/src/rest_import.rs` (`link_sources` end only), tests in
`crates/designos-fig/tests/` and `crates/designos-doc/tests/`. Start from the v3 patch.

Acceptance tests (user terms, red first):
1. A fill, an opacity and a ring-arc change on a layer one and two component levels deep survive a `.fig` save, reopen
   and an edit of the innermost main (any field, and the same field) followed by syncs of every level (TV1).
2. The same for a file opened by the previous build and saved natively (use
   `evidence/fixtures/older-native-after-fig-import-depth{1,2}.designos.json`); the load reports how many links it
   repaired (TV2).
3. Two copies of one inner main in a wrapper, one swapped on the page copy: after a `.fig` round trip and a wrapper sync
   the page copy still has exactly two copies and the swap stays on the same one (TV3).
4. Reset of an override on a deep layer of a reopened `.fig` returns the outer main's value, not the innermost main's.
   Fixture: main I itself overrides the shape's fill (so the two values differ); otherwise the test cannot fail. Derived
   from D-2, not measured.
5. A file saved by the fixed build loads without running the relink pass (marker set).
6. Kit (Figma file): per-instance identical to base on both metrics (0 of 200 after-states differ), 877,261 layers.
7. Kit saved by Studio as `.fig` and reopened (harness `ROUNDTRIP=1`): 0 copy layers not linked one level up (base
   321,148); per instance no pick worse than base on either metric (prototype: 0 worse, 16 better).
8. `.fig` import and native load within +5 % of base, min of 3 runs, base measured twice in the same session.

Mutants to kill: resolve the key file-wide (today's line); match by layer name instead of authored layer (test 3 has two
same-name copies); stop `authored_layer` one step later, at the main (merges twins: test 3); drop the uniqueness check;
skip the load pass (test 2); run the pass without the marker (tests 5, 8).
Must not change: anything on Figma-authored files (kit per-instance identical); `fig_export.rs` output.

### S2 — #1919, #1888: keep Figma's derived layout values while their inputs are unchanged (closes #1919, #1888)

Files: `crates/designos-doc/src/node.rs` (a `derived` record on the component link: size and transform as stored, plus
the reference size/transform and parent size they were derived against; serde default), `crates/designos-doc/src/components.rs`
(`apply_reference` size and transform, `constrain_direct_children`, `solve_instance_auto_layout`),
`crates/designos-fig/src/rest_import.rs` (`apply_fast_derived`, `resize_override_node`, `apply_overrides` during
hydration: fingerprint check of D-7), `crates/designos-fig/src/fig_export/kept_derived.rs` only if the record changes what
is written. Order: after S1 and S7.

Acceptance tests:
1. Import a `.fig` whose resized instances carry derived sizes (a small fixture made with the existing testkit), sync
   every main with no change: no size, transform, visibility, fill, flag or layer count changes (TV9).
2. Change one main's fill only: the instances' fills change; no derived size or transform changes (TV10).
3. Change the main's child size: that layer and the layers whose layout depends on it are re-derived (the record is
   dropped), as today.
4. Open a deferred page after a main edit: the edited main's instances are re-derived, untouched mains' instances keep
   the stored values (#1888).
5. Kit: extend the harness digest with each layer's transform; no-change sync **0 of 200**; there-and-back per instance
   not worse than base on any pick; the 5 non-size instances classified and fixed or reported.

Mutants: copy the reference size when the record is valid; skip the constraint pass for a layer whose record is
invalid; ignore the parent size in the validity test; skip the fingerprint check on hydration.
Must not change: what a sync does when a main's geometry really changed (existing component tests).

### S4 — #1915: count every override a switch cannot carry (closes #1915 part 1)

Files: `crates/designos-doc/src/components.rs` (`swap_main` carried loop `:1890-1898`, `Ctx`, the counts returned by
`swap_instance_inner` / `set_instance_prop_inner`), MCP/app reporting only where the inner-swap count is already reported.
Tests: (1) an override on a layer only variant A has is reported once as dropped on the switch to B (TV7);
(2) a carried override is not counted; (3) undo restores it. Kit: the count is > 0 on the 7 instances the triage lists
and 0 on the others; values per instance identical to base. Mutant: count before the single-text fallback.
Stays open: whether the override returns on the switch back (C7 UNKNOWN; needs a Figma runtime probe).

### S5 — #1916: text overrides cross a switch by name (closes #1916)

Files: `components.rs:1918-1921` only. Tests: TV4 both legs; TV5 still drops the fill; the equal-default-text control still
passes. Kit: per instance no pick breaks; the triage expects 4 primary picks to return. Mutant: keep the gate for
`TextStyle` only.

### S6 — #1917: fit the children of a resized instance after a switch (closes #1917)

Files: `components.rs:1805` and `:2430`. Tests: TV8; a not-resized instance is unchanged; an auto-layout instance keeps its
layout result. Kit: per instance no pick breaks; the triage trial had 24 losing their size differences and 11 returning
exactly. Mutant: lift only one of the two skips (the triage showed that alone changes nothing).

### S7 — #1918: the text box of an overridden text after a switch there and back (closes #1918)

Files: `components.rs` (`copy_overridden` `:1257` and its call `:1928`, `keeps_own_text_size` `:1415`). Test: an
overridden auto-resize text returns to the width it had after a switch there and back (round trip only; do not assert the
intermediate width, C9). Kit: per instance no pick breaks (the triage trial made Banner worse: it must not); the trial
returned 4 exactly. Mutant: carry the size when the text style differs.

### S8 — #1887: a new instance of a main whose nested instance is on an unopened page

Files: `components.rs` (`create_instance_reserving`) or the app call site, whichever the worker finds owns hydration;
Studio-only (deferred loading has no Figma counterpart; Figma materializes lazily too: `Invisible descendants should be
materialized prior to applying nested overrides`, 49875705). Test: the copy has the nested instance's layers at once.
Reproduce first; this spec did not.

### Issues that stay open after all slices

- **#1909**: needs the section 6 ruling and a new prototype; no slice closes it.
- **#1915**, part 2 (the override coming back after a switch there and back): UNKNOWN in Figma (C7). S4 only makes the
  drop visible.
- **#1918** intermediate state (C9): only the round trip is fixed.

## 8. Epistemic ledger (load-bearing claims)

| Claim | Tier | Evidence |
|---|---|---|
| Figma stores no copy layers; overrides and derived values are path-keyed, one step per level, resolved in that level's main | FACT | `evidence/re/03-figma-file-identity-counts.txt` |
| Variants never share layer ids; switching matches by name hierarchy | FACT (ids) / FACT strings (matcher) | 03, 01 |
| Figma remaps overrides and derived values per inheritance level | FACT strings; behaviour INFERRED | 01 func 12511, 30337 |
| #1925 is written by `link_sources` loop 1, override-key branch | FACT | `evidence/out/studio_nested_links.txt` |
| The kit has 0 mislinked copy layers; its Studio `.fig` round trip has 321,148, and 0 with the fix | FACT | `studio_nested_links-kit.txt`, `harness-{base-rt,v3-rt}.log` |
| Older native files lose the override today and are repaired by the load pass | FACT | `older_native_file-{base,v3}.txt` |
| The identity fix changes no instance of the Figma file and makes none of the round-tripped file worse | FACT (200 picks, two metrics, each file) | `compare-base-vs-v3.txt`, `compare-base-rt-vs-v3-rt.txt` |
| #1919 is the sync overwriting Figma's derived sizes | FACT for the 43 top-level layers; the 48 nested-pick layers and 5 non-size instances not attributed to Figma data | `attribution-noop-base.txt` |
| D-6 reaches 0 of 200 | ASSUMPTION (not prototyped) | — |
| Per-level flags fix #1909 without kit regressions | ASSUMPTION | section 6 |

Assumptions I made instead of asking: (1) an equal-value flag on a copy layer that a user set on purpose is rare
(confidence medium; flipped by a kit count of user flags equal to the reference); (2) the +10.8 % import time is noise
(medium: two identical base runs differed by 10 %; flipped by a quiet-machine rerun showing > 5 %).

## Unresolved questions

1. #1909: per-level flags (option 1), accept v2 (option 2), or leave open (option 3)?
2. May a Figma scratch-file probe settle C7 (override return after a switch back) and C9 (intermediate text size)? The
   owner's rule needs AskUserQuestion before creating a Figma file.
3. S2 changes the document model (a derived record on copy layers). Is a native-format addition acceptable for 0.7.14?
