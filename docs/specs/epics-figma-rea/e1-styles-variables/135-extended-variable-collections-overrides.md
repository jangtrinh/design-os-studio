# #135 Support extended variable collections with per-variable overrides (E1-6)

| Field | Value |
|---|---|
| Issue | #135 (E1-6, P2). Milestone on GitHub: Backlog; body says v0.8 |
| Still real? | Yes. Collections have no parent link; `VARIABLE_OVERRIDE` nodes are dropped on import |
| Size | XL: about 20–28 builder hours across 3 slices |
| Tier | Claude Sonnet (variable resolution is a shared hot path; the work sits on the hot-file queue) |
| Depends on | REA spike (step 0); #1664 merged; #134 merged first (hot-file order) |
| Ready after owner answers? | After Q3 (authoring scope). Slices 1–2 can start after step 0 whatever the Q3 answer is |

Code baseline: `origin/main@d22dad970`.

## 1. Outcome

An extended collection inherits all variables and modes of its parent, which can be a library collection. It can override single values. Changing a parent value flows to every value the extension did not override. "Reset" on an overridden value brings back the parent's value. A `.fig` file with extended collections opens, renders with the right values and saves back intact.

## 2. Re-anchor (origin/main)

- FACT: `VariableCollection { id, name, modes, default_mode, variables, key, source_library_key, extensions }` has no parent field (`crates/designos-doc/src/variables.rs:31-49`). The doc comment at `:45-46` names `com.figma.parentVariableSetId` as a kept extension key: it is stored, but nothing resolves through it.
- FACT (schema, `crates/designos-fig/src/figma_schema.txt`):
  - `VariableSetMode { id=1, name=2, sortPosition=3, parentVariableSetId=4, parentModeId=5 }` (`:2748-2753`);
  - `NodeChange.isCollectionExtendable = 385` (`:1592`);
  - `NodeType VARIABLE_OVERRIDE = 35` (`:82`);
  - `VariableModeBySetMapEntry.variableSetExtensionID = 3` (`:1763-1765`).
- FACT: import drops `VARIABLE_OVERRIDE` nodes with `DOCUMENT`/`VARIABLE`/`VARIABLE_SET` (`crates/designos-fig/src/fig.rs:1436`, `:1532`). `isCollectionExtendable` is kept as node metadata text only (`fig.rs:1213`).
- FACT: the DTCG importer never treats `com.figma.parentVariableSetId` as a link (`crates/designos-fig/src/variables_dtcg.rs:2170`).
- INFERENCE: a `VARIABLE_OVERRIDE` node holds the extension's overridden values for one parent variable. The extension's modes point at parent modes through `parentModeId`, and a layer's explicit mode for an extension is keyed with `variableSetExtensionID`. Closing check: step 0.

## 3. Scope and rules

0. **REA spike (about 3 h, `rea`)**, on a scratch Figma file plus the editor bundle:
   - (a) the fields of a `VARIABLE_OVERRIDE` node change: which variable, which extension, and per-mode values;
   - (b) whether an extension can add its own variables or modes;
   - (c) how a library parent is referenced (key versus guid);
   - (d) what "Reset" writes (the override is removed, or set equal);
   - (e) whether a layer's explicit mode targets the extension or the parent.

   Report: `plans/reports/re-figma-<date>-extended-collections.md`.
1. **Model (slice 1).**
   - `VariableCollection.parent: Option<String>` (parent collection id, serde default, skipped when `None`).
   - `VariableMode.parent_mode: Option<String>`.
   - Per-variable overrides `VariableCollection.overrides: BTreeMap<String /*parent var id*/, BTreeMap<String /*ext mode id*/, VariableValue>>`.
   - Old files are unchanged byte for byte.
2. **Resolution (slice 1).** Resolving a variable under an extension mode works like this:
   - an override value wins;
   - otherwise use the parent variable's value in `parent_mode`;
   - aliases resolve in the same mode context.

   The walk is bounded: depth ≤ 8, and a cycle is a lint error, never a hang. No per-frame allocation; the existing resolve fast path is kept.
3. **`.fig` round trip (slice 2).**
   - Import `parentVariableSetId`/`parentModeId`, `isCollectionExtendable` and `VARIABLE_OVERRIDE` nodes.
   - Export them back.
   - A library parent is kept by key, so the file still points at the library after a save.
4. **Panel (slice 3, scope per Q3).**
   - Recommended scope: the Variables panel shows an extension with inherited values (muted), overridden values (normal weight, with an override mark), and "Reset" on an overridden cell. Editing a cell in an extension creates an override.
   - Optional per Q3: "Extend collection" to create a new extension. The Untitled-UI-dark surface rules (#1155) apply.

## 4. Files likely touched

`crates/designos-doc/src/variables.rs`, `variables_edit.rs`; `crates/designos-fig/src/variables.rs`, `fig.rs`, `fig_export.rs`; `crates/designos-app/src/variables/view.rs` (slice 3).

## 5. Acceptance criteria

- **AC-1** (slice 1) Parent P has variable `brand` with Light = red. Extension E has mode E-Light (parent mode Light) and no override. A node set to E-Light resolves `brand` = red. Change P's Light value to blue, and the node resolves blue.
- **AC-2** (slice 1) E overrides `brand` in E-Light as green. The node resolves green, and the parent change in AC-1 does not affect it.
- **AC-3** (slice 1 + 3) "Reset" on the override removes it, the node resolves the parent's value, and undo restores the override. The slice 1 doc-level reset is tested in slice 1; the panel button comes in slice 3.
- **AC-4** (slice 2) A Figma-written fixture with an extension of a library collection, `isCollectionExtendable = true` and two overrides round-trips. After `.fig` save and reopen, `parentVariableSetId=4`, `parentModeId=5`, `isCollectionExtendable=385` and both overrides are present, and the parent library key is unchanged. Red on main, where the overrides are lost.
- **AC-5** A document without extensions serializes byte-identical (`.designos` and `.fig`).
- **AC-6** A cyclic parent chain, possible only in a crafted file, yields a lint issue and the stored fallback value. No hang; a test runs it with a timeout.
- **AC-7** (slice 3) A desktop capture at 1440 shows inherited versus overridden cells and Reset. Zero Fake UI gate passes.

## 6. Test steps

`cargo test -p designos-doc variables`, `cargo test -p designos-fig` (new `fig_extended_collection_overrides_round_trip.rs`), `cargo test -p designos-mcp` (MCP `get_variables` shows the resolved values), then the workspace gates. Plus the 1440 capture for slice 3.

## 7. Edge cases

- The parent collection is missing (an unsubscribed library): values fall back to the override, then to the stored literal, and a lint issue names the missing parent.
- The parent gains a mode after the extension was made: the extension has no mapped mode, so its default mode is used (confirm in step 0).
- The parent deletes a variable that has an override: the override is kept but unused, and the lint flags it. Nothing vanishes silently.
- DTCG export of an extension: write resolved values per extension mode. Builder decision, recorded: DTCG has no inheritance concept.

## 8. Non-goals

- Publishing extensions as a library (E6/E-library work).
- Extensions of extensions beyond what Figma allows (step 0 b).

## 9. Figma parity

Checked: schema fields. Needs a live check: everything in step 0.

## 10. Builder rules

Three PRs (slices). `Refs #135` until slice 3, which uses `Closes #135`. Hot-file queue after #134.
