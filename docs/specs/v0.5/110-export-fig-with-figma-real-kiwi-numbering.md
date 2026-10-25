# #110 Export .fig with Figma's real kiwi numbering (ComponentPropType, NodeChange tags)

- Issue: https://github.com/jangtrinh/design-os-studio/issues/110 (audit E3-4, epic #16, M / P1)
- Spec base: `main` at `767acf9`.
- Open PR: #514 (`task/110-fig-real-kiwi-numbering`, +737 −71). Release review, head `a79dd56`, found one MAJOR: exported records still carry invented tags 701+ (`jsonSerialized`, `stackPaddingTop/Left`, `gridColumn/RowAnchorIndex`, `connectorStart/EndStrokeCap`), and the test only samples real tags. The owner ruled on 2026-10-18 that the release session fixes it.
- Hot files: `fig_export.rs`, `kiwi.rs`, `testkit.rs` (serial queue).

## 1. Summary and user story

As a designer who exports a `.fig` from Studio, I need Figma.app to open the file with components, properties and slots intact. Figma decodes with its own field table. Every field id and enum value we write must therefore be Figma's, and we must write no field Figma does not know.

## 2. Current state

- FACT: `testkit::figma_like_schema()` (`testkit.rs:17`) builds the embedded export schema:
  - `ComponentPropType` = `[BOOL, TEXT, INSTANCE_SWAP, VARIANT, SLOT]` → 0..4 (`testkit.rs:162`), so SLOT is 4 where Figma has 7 and INSTANCE_SWAP is 2 where Figma has 3;
  - `ComponentPropNodeField` = `[VISIBLE, TEXT_DATA, OVERRIDDEN_SYMBOL_ID, SLOT_CONTENT_ID]` → 0..3 (`testkit.rs:163`), where Figma has `INHERIT_FILL_STYLE_ID=3` and `SLOT_CONTENT_ID=4`;
  - `NodeChange` fields declared through a helper `f(...)` at `testkit.rs:495`, mostly with sequential ids. Some use real ids explicitly (`codeSyntax=358` at `testkit.rs:665`, `stackChildMargin*=606..609` at `testkit.rs:788-791`). `isSlot` and `isSlotContent` (`testkit.rs:793-794`) are sequential.
- FACT: `Schema::encode_message` writes fields in insertion order (per review, `kiwi.rs:633`). A decoder that meets an unknown field id stops (`kiwi.rs:471`, `unknown field id`).
- FACT: the exporter writes `jsonSerialized` for nodes with `slot_content_id` (`fig_export.rs:647-649`). The real schema has no such field.
- FACT: the import of a real `.fig` uses the schema embedded in that file (`fig.rs`), so changing the export schema does not change how existing `.fig` files import.

## 3. Behaviour rules

1. **Real ids only.** Every `NodeChange` field the exporter can write has its Figma id from the 126.9.13 schema. The schema builder fails (panics in tests; `debug_assert` plus `Err` in release builds) when a `NodeChange` field name is not in the real-id table. No fallback numbering (701+).
2. **Real enum values.**
   - `ComponentPropType`: `BOOL=0, TEXT=1, COLOR=2, INSTANCE_SWAP=3, VARIANT=4, NUMBER=5, IMAGE=6, SLOT=7` (later values may be declared too).
   - `ComponentPropNodeField`: `VISIBLE=0, TEXT_DATA=1, OVERRIDDEN_SYMBOL_ID=2, INHERIT_FILL_STYLE_ID=3, SLOT_CONTENT_ID=4`.
   - Every other enum the exporter writes uses Figma's values. The builder checks each name against the table.
3. **Real sub-message ids.**
   - `ComponentPropDef{id=1, name=2, initialValue=3, sortPosition=4, parentPropDefId=5, type=6, isDeleted=7, preferredValues=8, varValue=9, parameterConfig=10, description=11, slotPropConfig=12}`
   - `ComponentPropAssignment{defID=1, value=2, varValue=3}`
   - `ComponentPropRef{nodeField=1, defID=2, zombieFallbackName=3, componentPropNodeField=4, isDeleted=5}`
   - `ComponentPropValue{boolValue=1, textValue=2, guidValue=3, floatValue=4, …}`
   - `SymbolData{symbolID=1, symbolOverrides=2, uniformScaleFactor=3}`
   - `SlotPropConfig{1..5}`, and the messages listed in specs 107 and 108.
4. **Drop non-Figma fields from `.fig` output.**
   - `jsonSerialized`: not written to `.fig`. The slot content id travels per property in `varValue` (#108). The node-level legacy id is kept in `.designos` only. It is lost on `.fig` export, and that loss is counted in an `ExportNotes.dropped_legacy_slot_content_ids` counter.
   - `stackPaddingTop` and `stackPaddingLeft`: removed. Top padding is written on `stackVerticalPadding` (210), left on `stackHorizontalPadding` (209), right on `stackPaddingRight` (233) and bottom on `stackPaddingBottom` (234).
   - `gridColumnAnchorIndex` and `gridRowAnchorIndex`: removed. The real anchors are `gridColumnAnchor` (440) and `gridRowAnchor` (439), both `GUID`.
   - `connectorStartStrokeCap` and `connectorEndStrokeCap`: written as `connectorStartCap` (245) and `connectorEndCap` (246).
5. **Import stays tolerant.** Our importer decodes with the file's embedded schema, so files exported before this change still import.
6. **Owner check.** Exporting the synthetic slot fixture (spec 160 rule 6) and an auto-layout component, then opening them in Figma.app 126.9.13, shows components, properties and slots. A screenshot goes in the PR. The owner's file is used only as the reference for the schema, never committed (R2.6).

## 4. Data model / schema changes

- No document model change.
- `testkit.rs`: a `REAL_NODE_CHANGE_TAGS: &[(&str, u32)]` table, plus the real-id sub-messages and enums from rules 2 and 3.
- New counter `ExportNotes { dropped_legacy_slot_content_ids: usize }`, returned by a new `export_fig_with_notes(doc) -> Result<(Vec<u8>, ExportNotes), FigError>`. `export_fig` (`lib.rs:39`) wraps it.

## 5. Contracts

- `pub fn export_fig_with_notes(doc: &Document) -> Result<(Vec<u8>, ExportNotes), FigError>`. It mirrors `pub fn export_fig(doc: &Document) -> Result<Vec<u8>, FigError>` (`fig_export.rs:88`), which becomes a wrapper.
- Schema-builder error: `FigError::Unsupported(format!("unmapped NodeChange field: {name}"))`. `FigError` (`lib.rs:46-53`) has `Json`, `Format` and `Unsupported`, and no new variant is added.

## 6. File-format mapping

RESOLVED from the Figma 126.9.13 editor bundle's embedded schema:
- `NodeChange` slot and component tags: `componentKey=216`, `componentPropDefs=266`, `componentPropRefs=267`, `componentPropAssignments=268`, `isSlot=463`, `isSlotContent=495`, `symbolData=113`, `derivedSymbolData=125`, `isStateGroup=225`, `key=319`, `overrideKey=213`, `variantPropSpecs=483`.
- Auto layout:
  - `stackSpacing=107`, `stackPadding=109` (legacy), `stackHorizontalPadding=209` (left), `stackVerticalPadding=210` (top), `stackPaddingRight=233`, `stackPaddingBottom=234`, `stackCounterSpacing=324`.
  - `stackPaddingTop` and `stackPaddingLeft` do NOT exist in the real schema.
- Connector: `connectorStart=242`, `connectorEnd=243`, `connectorStartCap=245`, `connectorEndCap=246` (type `StrokeCap`).
- Grid: `gridRowAnchor=439` and `gridColumnAnchor=440`, type `GUID`. The names `gridRowAnchorIndex` and `gridColumnAnchorIndex` do NOT exist in the real schema.
- `codeSyntax=358`; `stackChildMargin*` = 606..609.

Rule: every other field the exporter writes must be copied from the embedded schema. The implementer dumps the full `NodeChange` table with the existing REA dump (`figma-126-kiwi-schema.kiwi`, kept outside the repo per `rea-findings.md` "Method"), or decodes the schema of any `.fig` locally. PR #514 already mapped 256 `NodeChange` fields that way.

## 7. UI

None. The evidence is a Figma.app screenshot of the opened export.

## 8. Edge cases

1. A slot instance with a legacy `slot_content_id`: no `jsonSerialized` in the bytes, `componentPropAssignments` is decodable, and the counter is 1.
2. An auto-layout component with padding 8/12/16/20: only the real padding fields carry the values. A strict decoder (one limited to the real tags) reads `componentPropDefs` after them.
3. A connector with caps: both caps are present on the real fields.
4. A document with no components: the output decodes with the strict real-tag decoder.
5. A future exporter change adds a field missing from the table: the schema build fails in tests and blocks the merge.
6. Re-import of a file exported before this change: still green, because it uses the embedded schema.

## 9. Non-goals

- Exporting fields we do not model.
- Making Figma show our legacy node-level slot id.

## 10. Test plan

Seam: `crates/designos-fig/tests/fig_export_real_tags.rs` (from PR #514), extended.

- `exported_fig_uses_figmas_real_kiwi_numbering_for_component_prop_type_and_node_change_tags`: assert `SLOT == 7`, `INSTANCE_SWAP == 3` and `SLOT_CONTENT_ID == 4`, and every `NodeChange` field id in the embedded export schema is in `REAL_NODE_CHANGE_TAGS` with the same id.
  - Fails before: yes on `main`.
- `strict_real_tag_decoder_reads_an_exported_slot_instance`:
  1. export the synthetic slot fixture;
  2. decode with a schema whose `NodeChange` contains only `REAL_NODE_CHANGE_TAGS`;
  3. assert the decode succeeds and `componentPropAssignments` is read.
  - Fails before: yes (`jsonSerialized` unknown id).
- `strict_real_tag_decoder_reads_an_auto_layout_component`: same, for a component with padding.
  - Fails before: yes (`stackPaddingTop`).
- `connector_caps_are_written_on_real_fields`.
- `schema_build_rejects_an_unmapped_node_change_field`.
- Manual: Figma.app open, with a screenshot in the PR.

## 11. Performance budgets

No `CLAUDE.md` §4 invariant applies. Export time is unchanged, because the table lookup happens at schema build, once per export.

## 12. Dependencies and build order

- Depends on #31 (merged), #160 (fixture), #107 and #108 (the messages they add must already use real ids).
- Last in the fig queue, after #108.

## 13. Owner rulings that apply

- R2.6 (round 2, item 6):
  - the Figma.app open check uses the owner's file as the slot reference after #160;
  - #160 found 0 slots and reported to the owner;
  - the open check therefore uses the synthetic fixture, and the owner file is never committed.
- Brainstorm no-gos: no loosened assertions.
- Owner ruling of 2026-10-18: the release session fixes PR #514.
