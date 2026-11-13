# Review: E1, E2, E19 and E20 "True Figma Implementation Specs" (read-only)

Date 2026-11-12. Code checked against `origin/main` = `b731e4359`. The specs' base `e0e44f3c2` is real and is an ancestor of main.
Issue state from `gh issue view`. The Kiwi schema was checked in `crates/designos-fig/tests/data/figma-real-schema.txt` on main
(4,804 lines). Engine strings were checked only in the extracts of the nested copy spec's evidence folder
(see [spec-nested-copy-identity.md](../finalize-e1-e2-e3/spec-nested-copy-identity.md)); that folder is not in the repository.
Read-only review: no repository writes, no cargo, no GitHub writes.

"The reference spec" below means the evidence-backed spec of the same area:
[spec-nested-copy-identity.md](../finalize-e1-e2-e3/spec-nested-copy-identity.md) (slices S1 to S8) for E19 and
[spec-layout-drift-tail.md](../finalize-e1-e2-e3/spec-layout-drift-tail.md) for E2. "The dossier" means the matching file in
[evidence](../evidence/). Pull requests 1946 (first auto-layout slice), 1947 (nine MCP fixes) and 1950 (per-property variant picks)
were merged after this review was written; sentences they change are marked.

## Verdicts

| Spec | Verdict | Reason |
|---|---|---|
| E1/E2 REA README, master plan (`e1-e2-rea-resolutions/README.md`) | SUPERSEDED | 9 of its 13 slices target issues already closed and merged (#135 #136 #137 #143 #1448). The #1885 and #1895 slices need evidence first. The grid slice has no issue. Its hot-file queue slots 27–32 are not real (merges run through the project's own merge queue). |
| E1 step-0 (`e1-step0-…-1856.md`) | SUPERSEDED, except the component property default part (#1856), which is READY AFTER FIXES | Extended collections (#1813), Display P3 (#1812), OKLCH CSS (#1827, #1451), timing and easing variables (#1872) and text style reapply (#1749) are all merged, and all were merged before the spec's own base. Following the spec would build a second model beside the ones on main. |
| E2 layout and grid (`e2-layout-grid-…-unbounded-tracks.md`) | NEEDS EVIDENCE (#1885, #1895). Grid part: no issue, PARTLY DONE | It answers a different question than #1885 asks. It contradicts main and the reference spec on the Auto gap enum. The tilted-child rule has no source; the spec's own dossier calls it INFERENCE. Runtime checks issue #1942, items 7 and 8, covers both. |
| E19 slots, clip, hydration (`e19-e3-…-repair-spec.md`) | SUPERSEDED for #473, #1583, #1888 and #1887. Ring arc repair (#1907): READY AFTER FIXES | The preferred-only slot and selection outline issues (#473, #1583) are closed and fixed (#1750, #1760). The stale derived sizes and unopened page instance issues (#1888, #1887) are slices S2 and S8 of the reference nested-copy spec. Their "Figma facts" rest on strings that are not in the evidence and on a schema field that does not exist. The #1907 slice names the wrong override field and the wrong arc fields. |
| E20 save, load, quit (`e20-save-load-quit-lazy-hydration-spec.md`) | Split. #1717, #1718, #1719 and the cleanup part of #1648: SUPERSEDED by contributor drafts #1786, #1785, #1783 and #1684. #1686 and #1712: NEEDS DESIGN (not ready to implement). #1715 and #1716: READY AFTER FIXES. Rest of #1648: READY AFTER FIXES | Four slices duplicate open drafts that the owner has decided to carry. Two code anchors point at a line that is already guarded. The #1686 design would lose pages when an older build opens the file. The #1712 reader cannot plug into the existing loader. The #1715 fix would block in a place main deliberately keeps non-blocking. |

## What the review could not verify

1. **Figma app artefacts.** The evidence names `plugin-api.d.ts`, `editor_wasm.wasm`, `wasm-strings.txt`, the JS chunks `7a72fc59…`, `28827…`, `17185…` and `39902-…`, and the REA context IDs. None of these is in the repository or was available to the reviewer. Any Figma claim whose only source is one of them is graded ASSERTED.
2. **Base commit.** Every spec says "Base `origin/main@e0e44f3c2`". Yet every E1 fix and three E19 fixes are ancestors of that commit (checked with `git merge-base --is-ancestor`: #1813, #1812, #1827, #1451, #1872, #1749, #1750, #1760, #1911). The writers named a real base but did not read what it contains.
6. **Old-build data loss for #1686 is inferred.** The claim that a 0.7.13 build silently drops `deferred_pages` rests on `Document` lacking `deny_unknown_fields`. The reviewer did not test it by opening a file in 0.7.13.
7. **File count.** 29 spec files plus the index were created on 2026-11-12 (file birth times; all were created, none only edited). The count of 29 may include the index as one of them.
3. **Performance numbers.** These were not re-measured: the E20 figures (1.8 GB, under 4.5 s, under 18 s, 6.6 GB) and the claim "reopen drops back to 16–18 s". The issue figures they quote are sourced from #1686 and #1712. The spec's "Result" lines are predictions written as results.
4. **Test-vector tables.** All four specs grade test vectors `[FACT - seen]` or "Verified", but no Figma-authored file stands behind any of them. They are arithmetic or intent, so the reviewer grades them all ASSERTED. The only vectors in these specs that come from Figma are the ones copied from the reference spec.
5. **The dossier for E19 uses Studio's own bug reports as Figma evidence.** For example, the #1583 "Figma behaviour" is sourced "[FACT - seen: issue `#1583`, … `wasm-strings.txt:44258`]". It also reuses the reference spec's kit counts as if they were its own.

---

<a id="e1-e2-readme"></a>

## 1. Master plan in E1 and E2 ([`e1-e2-rea-resolutions/README.md`](../e1-e2-rea-resolutions/README.md), 116 lines)

**a. What it says to build.** It is a master plan of 13 slices:
- Extended collections (#135): model, Kiwi codec and Variables panel.
- Display P3 (#136): model, codec and colour matrix.
- Authored colour (#137): model plus CSS, then codec.
- Text style overrides plus component property defaults (#1448 and #1856, one slice).
- Transition variables (#143).
- The two layout rules (#1885, #1895).
- Grid tracks beyond 64.

The plan puts these in hot-file "slots" 27–32 and routes each slice to named model tiers. It also lists four "could not verify" items: multi-tier override delete, ICC v2, a rotated child with a negative margin, and the REST read-only status.

**b. Code references.** 10 file paths cited. 8 found exactly. 0 renamed. 2 not found:
- `designos-render/src/color.rs`: colour profile code is `crates/designos-doc/src/color_profile.rs` and `crates/designos-export/src/png_profile.rs`.
- `designos-export/src/png.rs`: the PNG profile chunk is in `png_profile.rs`.

**c. Figma facts.** These are the same as in the two child specs (graded below). Its own claim "resolves every Step-0 blocker … with bit-exact Kiwi IDs and mathematical proofs" is ASSERTED. The Kiwi IDs are real schema fields, but a field's existence does not prove a behaviour.

**d. Overlap with work in flight.**
- Slot 31 for #1885/#1895 codecs touches `fig.rs` and `fig_export.rs`. Reference layout spec slice 2 (#1885) touches `fig.rs:3324`, `rest_import.rs:4445` and `rest_import.rs:3477`. Slot 31 is held until observations exist for #1885 and #1895 (issue #1942).
- The "Hot-File Queue Positions" invent a queue; merges run through the project's own merge queue.
- The routing to named model tiers conflicts with the owner's working agreements (for example, UI work never goes to Grok). It is not a project ruling.

**e. Issue state.**

| Issue | State |
|---|---|
| #135 | closed (merged #1813) |
| #136 | closed (#1812) |
| #137 | closed (#1451, #1827) |
| #143 | closed (#1872) |
| #1448 | closed (#1749) |
| #1856 | open |
| #1885 | open |
| #1895 | open |
| Grid tracks beyond 64 | no issue (#24 "more than 8 tracks" is closed) |

**f. Verdict: SUPERSEDED.** Keep nothing from it except as an index into the two child specs.

---

<a id="e1-step-0"></a>

## 2. Step 0 in E1 ([`e1-step0-rea-resolutions-135-136-137-143-1448-1856.md`](../e1-e2-rea-resolutions/e1-step0-rea-resolutions-135-136-137-143-1448-1856.md), 372 lines)

**a. What it says to build.**
1. Extended collections (#135). Add `parent_collection_id`, `root_collection_key`, `is_extendable` and `overrides` to `VariableCollection`, and `parent_mode_id` to `VariableMode`. Resolution is recursive. Reset deletes the override.
2. Document colour profile (#136). Add a `DocumentColorProfile` enum to `paint.rs`. Convert uses an sRGB to P3 matrix; Assign keeps the floats. The P3 PNG gets `cHRM` and `iCCP` chunks.
3. Authored colour (#137). Add `AuthoredColor` on `Paint`. Dev Mode CSS emits `oklch()`, `oklab()` and `color(display-p3 …)` through a new `format_paint_color`.
4. Text style overrides (#1448). Add a run flag `is_override_over_text_style`. Weight, italic, decoration and link keep the style linked. Size, family and spacing detach it. Reapply drops the flagged runs.
5. Component property defaults bound to a variable (#1856). Add `bound_variable` on "PropDef" and "PropAssignment", read from `varValue` (9 on the definition, 3 on the assignment).
6. Transition timing and easing variables (#143). Add `duration_var` and `easing_var` on `Action::Navigate`.

**b. Code references.** 25 cited as existing. 15 found exactly, 5 found elsewhere or renamed, 5 cited as existing but absent (the invention signal). Proposed new by the spec and not counted: `AuthoredColor`, `AuthoredColorSpace`, `format_paint_color`.
- Found exactly: `variables.rs`, `variables_apply.rs`, `var_bind.rs`, `paint.rs`, `node.rs`, `css.rs`, `styles_ops.rs`, `prototype.rs`, `fig.rs`, `fig_export.rs`, `kiwi.rs`, the `VariableCollection` and `VariableMode` fields, `Paint { kind, opacity, visible, blend }`, `TextRun { start, end, style }`.
- Found elsewhere or renamed:

| Spec says | Main has |
|---|---|
| `reapply_text_style` in `styles_ops.rs` | `crates/designos-doc/src/text_style_ops.rs:122` |
| "PropDef" | `ComponentPropDef` (`node.rs:258`) |
| `DocumentColorProfile` | `ColorProfile` (`color_profile.rs:23`) |
| `render/src/color.rs` | `color_profile.rs` |
| `export/src/png.rs` | `png_profile.rs` |

- Cited as existing but absent: `text_props_mut`, `format_standard_color` (called as the fallback), "PropAssignment" (assignments are `ComponentLink.values: BTreeMap<String, PropValue>`), `PropDef.bound_variable` (said to exist "in Studio"), and `Action::Navigate { duration_var, easing_var }` (main binds through node binding keys, `is_easing_binding_key` / `is_duration_binding_key`).

**c. Figma facts.**

| Claim | Grade | Evidence and check |
|---|---|---|
| `VARIABLE_SET = 31`, `VARIABLE_OVERRIDE = 35`; `backingVariableSetId = 377`, `isCollectionExtendable = 385`, `rootVariableKey = 386`, `inheritedVariableIds = 517`, `overriddenVariableId = 464`, `variableDataValues = 315` | SOURCED | schema lines 71, 75, 2196–2201, 2178 (the spec's line numbers match) |
| `VariableSetMode.parentVariableSetId = 4`, `parentModeId = 5` | SOURCED | schema 3919–3920 (the spec cites 2748–2753, the wrong line) |
| Plugin API `ExtendedVariableCollection` (`parentVariableCollectionId`, `variableOverrides`, `removeOverridesForVariable`) | ASSERTED | `plugin-api.d.ts` is not in the repository or available to the reviewer. Moot: main ships #1813 |
| Resolution falls through to the parent mode; reset restores the parent | ASSERTED (no runtime) | matches main's `variables_extend.rs` model |
| `DocumentColorProfile { LEGACY 0, SRGB 1, DISPLAY_P3 2 }` = 349; `colorProfile = 341` | SOURCED | schema 4315–4318, 2208–2212 |
| sRGB to P3 matrix values; "Convert multiplies, Assign keeps floats" | ASSERTED | no source given. Moot (#1812) |
| `ColorSpace { SRGB 0, DISPLAY_P3 1, HEX 2, … }` | **CONTRADICTED** | schema 489–497: `DEFAULT = 0, SRGB = 1, DISPLAY_P3 = 2, HSL = 3, HSB = 4, OKLAB = 5, OKLCH = 6, CMYK = 7`. There is no `HEX`. Any codec written from the spec would mis-tag every space. |
| `Paint.authoredColor = 44`, `authoredStops = 45`, `Effect.authoredColor = 56` | SOURCED | schema 921–922, 1258 |
| `styleIdForText = 334`, `isOverrideOverTextStyle = 376` | SOURCED | schema 1780, 1759 (both on NodeChange) |
| "`NodeChange::styleOverrideTable = 3`" | CONTRADICTED (wrong message) | tag 3 is on `TextData` (schema 959) and `VectorData` (1037) |
| Only weight, italic, decoration and link keep a text style linked; size and family detach it | ASSERTED here | the same rule is already on main: `text_style_overrides.rs:1-6` |
| `ComponentPropDef.varValue = 9`, `ComponentPropAssignment.varValue = 3` | SOURCED | schema 2842, 2831 |
| `PrototypeAction.transitionDurationVar = 46`, `easingVar = 47`; `BehaviorTransition` 5 (duration var) and 6 (delay var) | SOURCED | schema 3207–3208, 4443–4444 |

**d. Overlap with work in flight.**
- Collides with merged code. Extended collections live in `variables_extend.rs`: `CollectionExtension { parent, mode_map, overrides }` under the extension key `org.designos.extendedCollection`, with no new field on purpose. The spec's new fields would fork the model.
- Contributor draft #1862 (extended collection links in branch merge) also touches that model, but it was skipped when contributor drafts were carried, because #1867 on main already covers it. No conflict.
- The #1856 part does not collide with any work in flight.

**e. Issue state.** Extended collections, Display P3, OKLCH, timing variables and text style reapply (#135, #136, #137, #143, #1448) are closed, and their fixes are ancestors of the spec's base. Component property defaults bound to a variable (#1856) is open (v0.7.14).

**f. Verdict: SUPERSEDED, except #1856, which is READY AFTER FIXES.** For #1856 the fixes are:
1. Use `ComponentPropDef` and add a field such as `default_variable: Option<String>`, serde default, skipped when empty.
2. Read `varValue` (9) in both `.fig` readers. The assignment side is already read at `fig.rs:2495-2496`, so do not redo it.
3. Write it in `fig_export.rs` beside the existing `varValue` writes (`fig_export.rs:1881`, `:1913`).
4. Add the variable id to `components[].boundVariables` in `crates/designos-fig/src/design_system_scan.rs`. That scan is what the issue's done-condition names, and the spec never mentions it.
5. Write a round-trip test through a real `.fig`.

Drop the `AuthoredColor` codec: no open issue asks for it, and the `ColorSpace` table is wrong.

---

<a id="e2-layout-grid"></a>

## 3. Layout and grid in E2 ([`e2-layout-grid-1885-1895-unbounded-tracks.md`](../e1-e2-rea-resolutions/e2-layout-grid-1885-1895-unbounded-tracks.md), 273 lines)

**a. What it says to build.**
1. Auto gap backing store (#1885). Auto gap sets `stackPrimaryAlignItems = SPACE_BETWEEN (4)`, and the numeric spacing stays stored. An instance override of the spacing alone keeps space-between. The wrap line gap is governed "independently" by `stackCounterSpacing`.
2. Tilted child (#1895). Clamp the child's width and height by min and max before rotation. Reserve the rotated child's bounding box. Turn Fill into Fixed for a non-quarter turn. A quarter turn swaps the axes and keeps Fill. The code goes into `flow_child_input`.
3. Grid tracks beyond 64. Make `GridTracks` a `Vec` and placement occupancy dynamic.

**b. Code references.** 22 cited. 17 found exactly, 4 renamed, 1 already done, 0 missing.
- Found exactly: `layout_solver.rs`, `layout_apply.rs`, `layout_solver_types.rs`, `layout_solver_grid_placement.rs`, `node.rs`, `fig.rs`, `fig_export.rs`, `kiwi.rs`, `flow_child_input` (`layout_apply.rs:87`), `content_sized_axes`, `natural_size`, `ChildIn::from_layout_child`, `SizeLimits` and `From<&LayoutChild>`, `MAX_GRID_TRACKS = 64` (`node.rs:809`), the `[bool; MAX_GRID_TRACKS]` occupancy (`layout_solver_grid_placement.rs:30`), `resolve_node`, `variables.bindings`.
- Renamed:

| Spec says | Main has |
|---|---|
| `clamp_width` / `clamp_height` | `clamp_w` / `clamp_h` |
| `fill_main` / `fill_cross` | `main_fill` / `cross_fill` |
| `transform.rotation()` | `Affine::rotation_rad()` (`geom.rs:97`) |
| unchanged `flow_child_input` | already calls the tilted-child logic in `layout_rotated.rs` (`fix_tilted_input`, `:113`) |

- Already done: `GridTracks(pub Vec<GridTrack>)`, documented "growable beyond that" (`node.rs:840-843`). What is still capped: `layout_solver_grid.rs:96` (`cols.clamp(1, MAX_GRID_TRACKS)`), placement occupancy, and `rest_import.rs:4313` / `:4397`.

**c. Figma facts.**

| Claim | Grade | Evidence and check |
|---|---|---|
| `StackJustify` values; `stackSpacing = 107`, `stackPrimaryAlignItems = 230`, `stackWrap = 323`, `stackCounterSpacing = 324`, `stackCounterAlignContent = 343` | SOURCED | schema 339–346, 1960–1984 |
| "Setting gap to Auto sets `SPACE_BETWEEN = 4`" `[FACT - seen]` | **CONTRADICTED** | Main, `fig.rs:2619-2621`: "The legacy `SPACE_EVENLY` (3) is Figma's space-between (real files carry it and never `SPACE_BETWEEN`; Figma lints it when `stackLayoutVersion < 1`)". The reference RE notes say the same: the WASM gates "justify == 3, which is the legacy `SPACE_EVENLY` value". The spec's only source is schema lines 339 and 1960, which show the enum, not which value Auto writes. |
| "Spacing stays stored as an inactive backing store" | INFERRED at best | consistent with reference S2 (JS ~3,523,560: the spacing field reads null in auto modes). The dossier gives no source. |
| "An instance override of spacing alone keeps space-between" `[FACT - seen]` | ASSERTED | the spec's own dossier (`rea-e1-e2-e12-vars-layout-text.md:284`) grades this "[INFERENCE from solver priority]". The spec upgraded it to FACT. |
| "The wrap line gap is governed independently by `stackCounterSpacing`" `[FACT - seen]` | **CONTRADICTED / off-target** | #1885 asks about Auto (non-finite or absent) counter spacing. Reference RE R4: 477 of 960 kit wrap stacks store it non-finite, meaning "follow the item spacing" (INFERRED). The spec never addresses the Auto case, which is the whole issue: "an instance override that sets only `itemSpacing` keeps the base's resolved counter gap". |
| Tilted child: min/max clamp before rotation, bounding box reserved, Fill becomes Fixed off quarter turns, Fill kept on quarter turns | ASSERTED (dossier line 299 and the summary table at 558 both say INFERENCE; the spec says "FACT - seen / INFERENCE") | Reference spec §5 and RE R5: UNKNOWN, with the only string being the Sites lint "A child layer set to {fill} can't be rotated between 45 and 135 degrees". Main's `layout_rotated.rs:16-19` already treats Fill, grow, stretch **and min/max** as Fixed for a tilted child, and says "Figma's behaviour for Fill on a tilted layer is unverified". The spec's "clamp by min/max first" **contradicts** main. Its Fill rule at 90 degrees is untested against the Sites lint's 45–135 band. |
| `GridTrackSizingType { FLEX 0, FIXED 1, HUG 2 }`, `gridRows = 435`… `gridRowsSizing = 475`, `GUIDPositionMap` | SOURCED | schema 1991–2000, 2755–2774. The INDEX's "`FIXED=0`/`FLEX=1`/`HUG=2`" contradicts both the schema and this spec. |
| "A grid with 100 columns places without truncation" `[FACT - seen]` | ASSERTED | no Figma file is cited |

**d. Overlap with work in flight.**
- The Auto gap slice (E2-REA-1, `layout_solver.rs` and `layout_apply.rs`) collides with layout slice 1 (#1920 legacy auto spacing). That slice was branch `fix/1920-legacy-auto-spacing`; it has since landed (pull request 1946) and changed `layout_solver.rs`, so the spec's rewrite would now collide with code on main.
- The spec's "Auto = SPACE_BETWEEN" premise contradicts that slice's version 0 / version 1 rules, which are now on main (pull request 1946).
- The tilted-child slice (E2-REA-2) rewrites what #1897 shipped (`ae37ad1d7`).
- Both slices contradict an owner ruling: #1885 and #1895 "stay open until those issues have observations" (#1942).
- The grid slice touches `node.rs` and `fig.rs`/`rest_import.rs`. It collides with nothing in flight, but no issue backs it.

**e. Issue state.** The wrap line gap override and the tilted Fill child (#1885, #1895) are both open. Grid: no issue.

**f. Verdict: NEEDS EVIDENCE.** The runtime checks issue already covers exactly these: item 7 (an Auto counter gap with an item-spacing override; read the line pitch) and item 8 (Fill at 30 degrees with min/max) of #1942. Neither this spec nor the reference spec's slice 2 (inferred R4) can be implemented until item 7 has an observation (owner ruling on #1885 and #1895). When it does, the reference's R4 framing is the one that asks the issue's question. The grid part should get an issue first. Its remaining work is the solver clamp at `layout_solver_grid.rs:96`, placement occupancy and the importer caps, not `GridTracks`.

---

<a id="e19-slots-clip-hydration"></a>

## 4. Slots, clip, hydration, repair in E19 ([`e19-e3-slots-clip-hydration-repair-spec.md`](../e19-e3-rea-followups/e19-e3-slots-clip-hydration-repair-spec.md), 229 lines)

**a. What it says to build.**
1. Preferred-only slots (#473). Do not emit an empty `slotPropConfig`. Add preferred components to the export roots.
2. Selection outline (#1583). Clip the selection box to clipping ancestors in `canvas/selection.rs`.
3. New instance from an unopened page (#1887). Hydrate the referenced pages before `copy_tree` in `create_instance`.
4. Stale derived sizes on page load (#1888). Add `symbol_fingerprint` to `DerivedSymbolData`. Compare it on `App::set_active_page` and re-sync.
5. Ring arc on pre-0.7.13 files (#1907). Add `migrate_legacy_document` in `load.rs`. When the file version is below 0.7.13 and an instance ellipse arc differs from its main, insert `OverrideField::CornerRadii`.

**b. Code references.** 30 cited. 10 found exactly, 8 found elsewhere or at another line, 10 cited as existing but absent, 1 proposed new, and 1 exists but is the wrong field.
- Found exactly: `components.rs`, `interact.rs`, `component.slot_configs`, `copy_tree` (`components.rs:100`), `reference_of`, `sync_one`, `session.rs`, `sync_pair`, `component.source`, `NodeKind::Ellipse(ArcData)`.
- Found elsewhere or at another line:

| Spec says | Main has |
|---|---|
| `slot_config_of` 295–315 | `:321` |
| `fig_export.rs:720-745` | that range is GUID allocation; the slot guard is `fig_export.rs:1872` (`cfg.has_settings()`) |
| `prune_unreachable` in the exporter | `lib.rs:1530`, which already pushes `cfg.preferred` at `:1562` |
| `carrier.rs` | `fig_export/carrier.rs` |
| `preferred_values` | `SlotPropConfig.preferred` |
| `create_instance` 577–620 | `:913` / `create_instance_reserving :919` |
| `hydrate_page` | `Document::ensure_page_hydrated` (`lib.rs:800`) |
| ArcData `starting_angle`, `ending_angle` | `start`, `sweep`, `inner_radius` |

- Cited as existing but absent (the invention signal): `is_empty_or_preferred_only`, `reachable_roots`, `canvas/selection.rs`, `selection_rect`, `document.rs`, `DerivedSymbolData`, `App::set_active_page`, `load.rs`, `migrate_legacy_document`, a native file version marker.
- Proposed new: `referenced_unhydrated_pages`.
- Wrong field: the spec uses `OverrideField::CornerRadii` for the arc. `OverrideField::Arc` (`node.rs:232`) was added by #1911 and is the right one.

**c. Figma facts.**

| Claim | Grade | Evidence and check |
|---|---|---|
| A preferred-only slot omits `slotPropConfig`; preferred components stay reachable | ASSERTED here | source is "`39902-….min.js:3773186`" (not in the repository) and `spec-nested-copy-identity.md:49-57`, which says nothing about slots. Moot: fixed on main (`SlotPropConfig::has_settings`, `empty_config_written`; #1750). |
| Selection outline clipped by `clipsContent`; string `selection_bounds_clip_ancestor` at `wasm-strings.txt:44258` | ASSERTED (likely invented) | the string is absent from both engine-string extracts that were available to the reviewer. The dossier (line 315) cites "issue `#1583`" itself as the Figma evidence. Moot: #1583 closed by #1760. |
| `instances_with_child_nodes_in_file = 0`; Figma materializes copies on demand | SOURCED, borrowed from the reference spec (A1) | `03-figma-file-identity-counts.txt` |
| `InstanceRematerializer` at `editor_wasm.wasm:27232618` | ASSERTED (not found) | absent from `01-…` and `02-engine-strings-cited.txt` |
| `DerivedDataUpdater` (49926075), the repairer path (49957641) | SOURCED as strings | in `02-engine-strings-cited.txt:9,12`. The spec mislabels 49957641 as `symbol_fingerprint_mismatch`; it is `FGMixedDirtyOriginDerivedSubtreeRepairer.cpp`. The reference grades the meaning INFERRED; this spec says FACT. |
| "Figma stores `derivedSymbolData` paired with a `symbolFingerprint` on each instance" | **CONTRADICTED** | the word "fingerprint" appears nowhere in the 4,804-line schema. `derivedSymbolData = 125` exists (schema 1809) with no fingerprint field. The fingerprint is Studio's own (`instance_fingerprints`, `rest_import.rs:2050`; reference spec §4). |
| Arc overrides kept in `symbolOverrides` under "`arcDataOverride`" | ASSERTED | the schema has `NodeChange.arcData = 195` and `symbolOverrides = 2`, but no `arcDataOverride`. Not needed: #1907 is a Studio file-migration issue with no Figma question. |

**d. Overlap with work in flight.**
- **Stale derived sizes on page load (#1888).** This is reference slice S2 (#1919 + #1888). That slice waits for S1 and S7, as the reference spec orders its slices. The reference puts the check in `rest_import.rs` `apply_overrides` during hydration, using the existing fingerprints. This spec puts a new field on a type that does not exist and a check in the app's page switch, so the two designs conflict.
- **New instance from an unopened page (#1887).** This is reference slice S8, which is ordered last. An owner ruling also keeps the `components.rs` copy and sync code for the reference spec's slices until they land. Collision on `components.rs`.
- **Ring arc repair (#1907).**
  - It needs a "file older than 0.7.13" marker, and main has none.
  - The nested copy link fix, slice S1 (#1925, still open), owns `finish_load` (`lib.rs:1770`), `load_repair` and "the document marker". The ring arc repair (#1907) should reuse that marker and land after S1.
  - A standing rule applies to the marker: serde default, skipped when empty, and a test that the previous release still opens the file.
- **Selection outline (#1583).** Open contributor draft #1625 ("clip instance chrome to ancestor clipping shapes") targets an issue that is already closed. It should be declined when contributor drafts are carried.

**e. Issue state.**

| Issue | State |
|---|---|
| #473 | closed (#1750) |
| #1583 | closed (#1760) |
| #1887 | open |
| #1888 | open |
| #1907 | open |

The INDEX says this spec "resolves #1881 instance clip flag, #1886 lazy hydration, #1891 symbolData repair". All three labels are wrong:
- The label points at a closed wrap counter gap import issue (#1881).
- The label points at a merged layout pull request (#1886).
- The label points at an open image-cache bound issue (#1891).

**f. Verdict.** Preferred-only slots and the selection outline (#473, #1583): SUPERSEDED (fixed on main). Stale derived sizes and the unopened page instance (#1888, #1887): SUPERSEDED by reference slices S2 and S8.

Ring arc repair for pre-0.7.13 files (#1907): READY AFTER FIXES:
1. Use `OverrideField::Arc`, not `CornerRadii`.
2. Use the field names `start`, `sweep`, `inner_radius`.
3. Repair in `finish_load` / `load_repair`, not in a new `load.rs`.
4. Gate on the document marker that S1 adds, and land after S1.
5. Handle the issue's own warning: an arc may differ only because the main changed since the last sync. Flag only when the instance arc also differs from what a sync would have written, or report instead of flagging.
6. Write a red test on a real pre-0.7.13 `.designos` fixture (the spec's fixture is said to be "pre-0.7.12").

---

<a id="e20-save-load-quit"></a>

## 5. Save, load, quit in E20 ([`e20-save-load-quit-lazy-hydration-spec.md`](../e20-save-load-quit/e20-save-load-quit-lazy-hydration-spec.md), 279 lines)

**a. What it says to build.**
1. Compact unopened pages (#1686). Add `deferred_pages: BTreeMap<NodeId, DeferredPagePayload>` with compressed bytes. Pages become shells and hydrate on first visit.
2. Reopen CPU (#1712). Read through an 8 MB `BufReader` over an `EvictingMmapReader` that drops consumed 16 MB windows with `madvise`.
3. Clone and lock (#1715). `Clone for Document` takes the hydrator lock blocking. Add `SaveError::UnhydratedPagesRefused`.
4. Dropped copy (#1716). Add a client id with release on `Drop`.
5. Quit drops queued work (#1717). Add a `handle_quit_requested` that drains `queued_ops`.
6. Library thumbnail guard (#1718). Add the 25,000-node guard at `session.rs:958`.
7. Queued save tab (#1719). Add `QueuedOp::Save { tab_id }`.
8. Quit hardening (#1648). A `#[no_mangle] applicationShouldTerminate` with `catch_unwind`, unhide, library flush, and one cleanup in `App::drop`.

**b. Code references.** 35 cited. 12 found exactly, 6 found at another line or wrong, 11 cited as existing but absent (the invention signal), 6 proposed new.
- Found exactly: `pending_pages.rs:102` (`try_lock`), `page_hydrator`, `DeferredFigHydrator` (`rest_import.rs:3764`), `hydrate_all`, `is_hydrated`, `thumb::refresh_in_background`, `export_doc`, `tabs.rs`, `tabs.entries`, `shell.rs`, `macos_terminate.rs`, `persist_document_to`.
- Found at another line or wrong:

| Spec says | Main has |
|---|---|
| `lib.rs:235` (Clone) | `DirtyLog` sits there; `impl Clone for Document` is `lib.rs:748` |
| **`session.rs:958` is the unguarded call** | `:955-956` **already has the guard**. The unguarded call is `session.rs:1337` (the issue says 1332). The spec's fix lands on the wrong line and fixes nothing. |
| guards at 579, 616, 739 | they are at 772, 813, 955 |
| `ops.rs:2635` | `:2678` / `:2682` |
| `applicationShouldTerminate` as `#[no_mangle] extern "C"` | main registers `extern "C-unwind" fn application_should_terminate` on the delegate class (`macos_terminate.rs:96`) |
| `NSTerminateLater // 1`, `NSTerminateNow // 2` | wrong values. objc2-app-kit `NSApplication.rs:1059-1066`: TerminateCancel 0, TerminateNow 1, TerminateLater 2. Main uses `TerminateCancel`, not Later. |

- Cited as existing but absent (the invention signal): `crate::hex_bytes` (main has `serde_images`), `clone_hydrator`, `clone_fields_without_hydrator`, `has_pending_instances` (main: `has_pending_pages`), `queued_ops`, `execute_op` (main: `finish_queued_work_before_quit`, `deferred_work.rs:35`), `TabId`, `get_app_instance`, `cleanup_runtime_resources`, `save_tab_at_index`, and the claim that `memmap2` is available (it is only a transitive entry in `Cargo.lock`, not a dependency).
- Proposed new: `DeferredPagePayload`, `EvictingMmapReader`, `SaveError::UnhydratedPagesRefused` (main already has `PendingPagesError { NotLoaded, LoadedElsewhere, … }`), `QueuedOp`, `handle_quit_requested`, `flush_all_library_edits`.

**c. Figma facts.** The spec makes no Figma behaviour claim. The dossier name is decoration: everything here is Studio-native. Its Studio claims, graded:

| Claim | Grade | Check |
|---|---|---|
| The #1686 size and time table (1.67 GB to 4.95 GB, 3.5 s to 17 s) | SOURCED | issue #1686 body |
| "An older Studio build that opens a file with `deferred_pages` displays a notice" | **CONTRADICTED** | `Document` has no `deny_unknown_fields` (`lib.rs:259`; only `spec.rs` uses it). An older build silently drops the field, opens shell pages, and its next save writes them empty. That breaks the standing rule that "a test proves the previous release still opens the file", and the data-loss rule #1686 itself names as not in scope (#1670). |
| "Unbuffered or small-buffer per-byte reads cause the CPU" | CONTRADICTED in part | main already reads through `BufReader::with_capacity(1 << 20, …)` (`designos-library/src/store.rs:802`) into `finite_json::from_reader_repairing`. That function needs `Read + Seek` (`finite_json/mod.rs:129`) for the non-finite-number repair pass. `EvictingMmapReader` implements `Read` only, and calling `serde_json::from_reader` directly would drop the repair. |
| "Reopen drops to 16–18 s, resident pages at most 16 MB" | ASSERTED | not measured |
| #1715: take the lock blocking in `Clone` | **CONTRADICTED by a code invariant** | `pending_pages.rs:99-101`: "A clone never waits for the loader: if it is busy loading a page (or a loader calls back into a clone), the list is taken the first time it is asked". A blocking lock can deadlock on a re-entrant loader. The issue's expected fix: build the list before the loader is shared, and refuse the save if the list is unknown. |
| #1648: "unhide the app if hidden" | ASSERTED fix, still in scope | Main already tries: `macos_terminate.rs:82-84` ("bringing a hidden window back first"), added by #1645 (`7d53ebf50`, 11-05). But #1648 was filed as a follow-up to that same fix and says the question "may not appear" when hidden (inferred from winit, not reproduced). Needs a repro first. |

**d. Overlap with work in flight.**
- **Duplicates of open contributor drafts** (all by NamAIEngineer):

| Issue | Draft |
|---|---|
| #1717 | #1786 "drain queued work before shutdown" |
| #1718 | #1785 "guard large library thumbnail copies" |
| #1719 | #1783 "keep queued saves bound to their tab" |
| #1648 cleanup | #1684 "make quit cleanup idempotent" |

- All four belong to the app shell and lifecycle group of contributor drafts (#1081 #1175 #1684 #1786 #1783 #1785 #1774). The owner decided to carry bug-fix drafts with credit, and that group is still on hold. Implementing the spec's versions would override the owner's carry decision.
- **Files.** The compact pages, lazy reader and clone lock slices (#1686, #1712, #1715) touch `lib.rs`, `pending_pages.rs`, `rest_import.rs` and `designos-library/src/store.rs`. `lib.rs` is shared with S1 (#1925, `finish_load` and the marker). The compact pages slice (#1686) also overlaps S2's native-format addition (the "derived record").
- **Later merge.** Pull request 1947 made the MCP read tools fill in a deferred page before they read it. This bears on the "what every reader does" question for #1686 (section 5f); it does not change any verdict here.

**e. Issue state.** All eight are open, in milestone "Backlog: AI design 2 roadmap & overflow", not v0.7.14. The epic is #1788. Per their own issue text, #1715, #1716, #1717, #1718 and #1719 are "not urgent".

**f. Verdict, by slice.**

| Slice | Verdict | What to do |
|---|---|---|
| Quit drops queued work (#1717), library thumbnail guard (#1718), queued save tab (#1719), cleanup runs twice (#1648) | SUPERSEDED | carry drafts #1786, #1785, #1783 and #1684 |
| Rest of quit hardening (#1648: panic guard, library flush on logout, hidden app) | READY AFTER FIXES | write it against `application_should_terminate` / `should_terminate`, keep `TerminateCancel` plus `request_quit`, use `catch_unwind` with a `TerminateNow` fallback. Hidden app: reproduce first, because main already tries to unhide (#1645) and the issue says that may not be enough. |
| Compact unopened pages (#1686) | NEEDS DESIGN | answer the issue's own three open questions: the format and how older builds react (they will silently drop the field, so a refusal mechanism must come first); what every reader does (MCP, export, Dev mode, search, lint); main edits while pages are compact. Then measure. |
| Reopen CPU (#1712) | NEEDS DESIGN | measure first. The reader must keep `Seek` and the repair pass. |
| Clone and lock (#1715) | READY AFTER FIXES | fix at `remember_deferred_pages` (`pending_pages.rs:97-104`): build the list before the loader is shared, without blocking. If the list is unknown at save time, refuse with `PendingPagesError`. |
| Dropped copy (#1716) | READY AFTER FIXES | either release-on-drop or a recoverable message ("Pages were loaded in a closed copy. Reopen the file"), as the issue allows. Name `PendingPagesError::LoadedElsewhere`, which exists. |

---

## Conflicts with work in flight

- **Layout slice 1 (#1920)**, branch `fix/1920-legacy-auto-spacing`, has since landed (pull request 1946). The E2 slice for #1885 targets the same `layout_solver.rs`. Its premise "Auto gap = `SPACE_BETWEEN` (4)" contradicts main (`fig.rs:2619-2621`) and the reference rules for version 0 and version 1 that the slice implemented.
- **Owner ruling on #1885 and #1895.** The wrap line gap override and the tilted Fill child (#1885, #1895) stay open until #1942 has observations. The two layout slices of E2 and README slots 31 and 32 must not be implemented.
- **Tilted children.** The E2 #1895 code would overwrite `layout_rotated.rs`, which #1897 merged (`ae37ad1d7`).
- **Nested-copy spec slice S1 (#1925, still open)** owns `finish_load`, `load_repair` and the document marker. The E19 migration (#1907) and the E20 changes (#1686, #1715) touch the same load path. The ring arc repair (#1907) must reuse the marker of S1 and land after it.
- **Nested-copy spec slices S2 (#1888) and S8 (#1887)** are ordered by the reference spec: S2 after S1 and S7, S8 last. An owner ruling keeps other work off the `components.rs` copy and sync code until its slices land. The E19 slices for #1887 and #1888 would break both.
- **App shell and lifecycle drafts** (on hold). The E20 slices for #1717, #1718, #1719 and the #1648 cleanup duplicate drafts #1786, #1785, #1783 and #1684 (the owner decided to carry contributor drafts).
- **Extended collections.** The E1 model would fork the one merged in #1813 (`variables_extend.rs`).
- **The index's process rules.** The routing to named model tiers and the "hot-file slots 27–32" are not the project's process. The index also drops the owner's 2026-10-16 exception to the "< 64 programs" invariant (the uncapped refine when the view is idle).

## Index errors (EPIC-FIGMA-SPECS-INDEX.md)

- In the E19 row: "`isSlot=476`/`isSlotContent=483`". The schema has `isSlot = 463` and `isSlotContent = 495`; 476 is `gridChildVerticalAlign` and 483 is `variantPropSpecs`.
- In the E1 row: "`#135` `isExtension=904`". There is no Kiwi tag 904; `isExtension` is a Plugin API property. "`#143` timing rejection" does not match the spec (timing variables). "`#1856` `StyleType`" does not match the issue (component property defaults bound to a variable).
- In the E2 row: "`GridTrackSizeType` `FIXED=0`/`FLEX=1`/`HUG=2`" contradicts the schema and the E2 spec (FLEX 0, FIXED 1, HUG 2). It also files the grid solver under #1895, which is the tilted-child issue.
- In the E12 row: "`styleOverrideTable=132`". The schema has tag 3 on `TextData` and on `VectorData`.
- In the E19 row: "Resolves #1881 instance clip flag, #1886 lazy hydration override preservation, #1891 orphan/cycle symbolData repair". None matches: #1881 is a closed wrap counter gap issue, #1886 is a merged layout PR, #1891 is an image cache bound. The spec body covers #473, #1583, #1887, #1888 and #1907 instead.
- Header: "Status: Canonical & Binding" and "100% backed by clean-room reverse engineering". Graded ASSERTED. None of the four specs reviewed here is backed for its behaviour claims.

## The 29 new specs (from the index; line counts measured, status as the index or file claims)

| Epic (index) | Spec | Issues named (index or file header) | Lines | Claimed status |
|---|---|---|---|---|
| E1 #14, E2 #15 | e1-e2-rea-resolutions/README.md | #135 #136 #137 #143 #1448 #1856 #1885 #1895 | 116 | Canonical & Binding (index) |
| E1 #14 | e1-e2-rea-resolutions/e1-step0-…-1856.md | #135 #136 #137 #143 #1448 #1856 | 372 | all facts "[FACT - seen]" |
| E2 #15, E18 #1486 | e1-e2-rea-resolutions/e2-layout-grid-…-unbounded-tracks.md | #1885 #1895, grid | 273 | "[FACT - seen]" |
| E3 #16, E19 #1487 | e19-e3-rea-followups/e19-e3-…-repair-spec.md | #473 #1583 #1887 #1888 #1907 (index: #1881 #1886 #1891) | 229 | vectors "Verified" |
| E20 #1788 | e20-save-load-quit/e20-…-lazy-hydration-spec.md | #1648 #1686 #1712 #1715–#1719 | 279 | Canonical (index) |
| E4 #17 | e4-draw-vector/README.md | #179 #193 #194 #206 #499 #1506 | 85 | status table |
| E4 #17 | e4-draw-vector/e4-basic-shapes-strokes-vector-tools-spec.md | #193 #194 #196 #197 #499 #952 #953 #1506 | 355 | Build-Ready |
| E4 #17 | e4-draw-vector/e4-figma-draw-variable-width-…-text-path.md | E4 | 336 | Build-Ready |
| E5 #18 | e5-effects-shaders/README.md | #37 #38 #168 #176 #521 #571 #1328 | 84 | status table |
| E5 #18 | e5-effects-shaders/e5-image-filters-progressive-blur-glass-noise-spec.md | #37 #38 #521 #1328 | 364 | Build-Ready |
| E5 #18 | e5-effects-shaders/e5-video-fills-custom-wgsl-shaders-3d-transforms.md | #39 #168–#176 #571 | 248 | Build-Ready |
| E6 #19 | e6-motion-export/README.md | #213 #457 #459 #528 | 116 | Canonical (index) |
| E6 #19 | e6-motion-export/e6-export-bounds-settings-easing-springs-spec.md | #213 #457 #459 #528 | 331 | Canonical (index) |
| E6 #19 | e6-motion-export/e6-motion-timeline-lottie-video-export-spec.md | #140 #142 #164 #167 #209 #237 | 270 | Canonical (index) |
| E7 #20 | e7-devmode-mcp/e7-annotations-measurements-code-connect-mcp-spec.md | E7 | 265 | Canonical (index) |
| E8 #21 | e8-interop-pptx/e8-pptx-import-export-spec.md | #290 #294 #307 #308 | 330 | Canonical (index) |
| E9 #22, #1215 | e9-slides-whiteboard/README.md | #33–#36 #1215 | 151 | Canonical (index) |
| E9 #22 | e9-slides-whiteboard/e9-slides-transitions-builds-deck-roundtrip-spec.md | E9 | 410 | Canonical (index) |
| E9 #22, #1215 | e9-slides-whiteboard/e9-1215-connectors-mindmap-mermaid-freehand-spec.md | #333 #1215 | 394 | Canonical (index) |
| E10 #23 | e10-versions-branching/e10-three-way-diff-branch-merge-git-driver-spec.md | E10 (new area) | 214 | Canonical (index) |
| E11 #123 | e11-prototyping/README.md | E11 | 121 | Canonical (index) |
| E11 #123 | e11-prototyping/e11-proto-crate-triggers-sticky-smart-animate-spec.md | E11 | 279 | Canonical (index) |
| E11 #123 | e11-prototyping/e11-video-audio-gif-present-ux-mcp-spec.md | E11 | 196 | Canonical (index) |
| E12 #124, #1107 | e12-text-fonts-a11y/README.md | #828 #869 #904 #915 #935 #944 #951 #969 #1019 | 103 | Canonical (index) |
| E12 #124 | e12-text-fonts-a11y/e12-variable-fonts-rich-text-decorations-smart-symbols.md | E12 | 306 | Canonical (index) |
| E12 #124 | e12-text-fonts-a11y/e12-canvas-a11y-contrast-eyedropper-video.md | E12 | 304 | Canonical (index) |
| E13 #125 | e13-sites-code-layers/README.md | E13 | 76 | Canonical (index) |
| E13 #125 | e13-sites-code-layers/e13-sites-breakpoints-scroll-cms-compiler-spec.md | E13 | 350 | Canonical (index) |
| E13 #125 | e13-sites-code-layers/e13-code-layers-runtime-sandbox-mcp-spec.md | E13 | 232 | Canonical (index) |

Plus the index itself (238 lines, "Canonical & Binding"). "E4" and similar in the issues column mean the spec header names only the epic.

## Open questions

1. Component property defaults (#1856): implement now on the corrected scope (section 2f), or batch it with the scan-file work?
2. Save, load and quit (E20): carry drafts #1786, #1785, #1783 and #1684 first (the app shell group on hold), then write a fresh, sourced spec for #1686, #1712, #1715 and #1716?
3. Grid beyond 64 columns: file an issue (solver clamp, placement occupancy, importer caps), or drop it?
