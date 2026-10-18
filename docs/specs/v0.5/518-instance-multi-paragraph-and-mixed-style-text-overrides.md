# #518 Instance text overrides with several paragraphs or mixed styles are lost (the component's Lorem ipsum shows)

- Issue: https://github.com/jangtrinh/design-os-studio/issues/518 (bug, Untitled UI kit render audit 2026-10-18)
- Spec base: `main` at `767acf9`.
- Open PR: #537 (`task/518-instance-text-overrides`, +293 −26). Root cause named in the PR: Figma's null-GUID sentinel on `overrideKey`, plus `styleIdForText` ignored on override-table entries. No release review yet at spec time.
- Hot files: `crates/designos-fig/src/fig.rs`, `crates/designos-fig/src/rest_import.rs` (serial queue).

## 1. Summary and user story

As a designer importing a real Figma kit, text I overrode inside an instance must appear as I wrote it. This includes multi-paragraph text and text with bold, underlined or coloured words. The component's placeholder must not show. 125 placeholder texts in 66 frames on 26 pages of the Untitled UI kit show today.

## 2. Current state

- FACT: `guid_str` (`fig.rs:243-245`) and `kiwi_guid_str` (`fig.rs:72-81`) format any `{sessionID, localID}`, including `{4294967295, 4294967295}`.
- FACT: `overrideKey` is copied into `_overrideKey` (`fig.rs:1175-1177`). The importer registers it in `override_keys` (`rest_import.rs:759`, `:857`).
- FACT: in fast-clone mode (`rest_overrides.is_empty() && doc.nodes.len() >= 10_000`, `rest_import.rs:1847`), `expand_instance` takes `source_id` and inserts `override_keys.entry(src).or_insert_with(...)` (`rest_import.rs:1864-1867`). If `src` already holds the sentinel key, the real guid is never registered.
- FACT: overrides resolve through `resolve_path_cached` (`rest_import.rs:2077`), using `node_matches` (`:2017`) and `build_subtree_lookup` (`:2029`).
- FACT: text override content comes from `convert_override` → `textData` (`fig.rs:2105`). Runs come from `text_overrides` (`fig.rs:2444-2455`), which reads `characterStyleIDs` and `styleOverrideTable`. A table entry's `styleIdForText` is not resolved. The node-level `styleIdForText` is resolved at `fig.rs:2411`.
- FACT from the issue: "Design note" instance `11185:8546` carries inner overrides `characters` and `styledTextSegments`. Exposure: 125 nodes, 66 frames, 26 pages.

## 3. Behaviour rules

1. **Null GUID.** `{sessionID: 0xFFFFFFFF, localID: 0xFFFFFFFF}` is Figma's "unset" GUID. `guid_str` and `kiwi_guid_str` return `None` when either part is ≥ `u32::MAX`. As a result:
   - `_overrideKey` is not written for it;
   - `styleIdFor*` references with it resolve to nothing, which is the same as unset.
2. **Override key registration.** In fast-clone mode, a component child with no valid `overrideKey` registers its own guid tail as its key. A cloned node stays addressable by both its `_overrideKey` (when valid) and its component guid. `node_matches` and `build_subtree_lookup` follow `component.source` to the source's key.
3. **Text content.** A matched text override sets `characters` exactly, newlines included. A paragraph break `\n` and a soft break U+2028 are kept verbatim.
4. **Mixed styles.** Override runs come from `characterStyleIDs` and `styleOverrideTable`. For each table entry, resolve `styleIdForText` to the shared text style node first. Then apply the entry's inline fields on top: `fontSize`, `fontName`, `textDecoration`, `fillPaints` and the rest. Missing fields inherit from the node's base style.
5. **Order.** Raw overrides apply, then `derivedSymbolData` (`rest_import.rs:1904-1925`). Derived text data never replaces override `characters` with the component's characters. Derived data carries layout and geometry, not content.

## 4. Data model / schema changes

None.

## 5. Contracts

None public. Internal: `fn guid_str(g: &Value) -> Option<String>` and `fn kiwi_guid_str(g: &KiwiValue) -> Option<String>` return `None` for the sentinel.

## 6. File-format mapping (RESOLVED)

- `NodeChange.overrideKey = 213` (GUID).
- The null-GUID sentinel `4294967295:4294967295` is observed in the kit (PR #537 root cause).
- `textData`, with `characters`, `characterStyleIDs` and `styleOverrideTable`.
- `styleIdForText = 334` (`StyleId{guid=1, assetRef=2}`), from `rea-findings.md` §Shared styles.
- `symbolData.symbolOverrides = 113.2`, with `guidPath` on each override entry.

## 7. UI

None. Evidence: Studio versus Figma renders of the "Design note" frames `3936:417924` and `3936:417929` through the existing kit render audit, captured as PNG.

## 8. Edge cases

1. An override with two paragraphs, `"Line one\nLine two"`, and run 2 bold: both paragraphs present, run 2 bold.
2. An override entry with `styleIdForText` pointing at "Text lg/Semibold" (weight 600) plus `textDecoration: UNDERLINE`: the run is semibold and underlined.
3. A component child whose `overrideKey` is the sentinel, inside an instance in a document with 10k nodes or more (fast clone): the override applies.
4. The same document under 10k nodes (slow path): the override applies, so both paths agree.
5. `styleIdForFill` with the sentinel on a run: no fill style link, and the inline `fillPaints` are used.
6. A title override in the same instance: still applies (a regression guard from the issue).

## 9. Non-goals

- Layout fixes for resized instances (#522).
- Variant and swap shells (#525).

## 10. Test plan

Seam: `crates/designos-fig/tests/fig_roundtrip.rs` (synthetic) and `tests/real_files.rs` (kit, `#[ignore]`, run with `DESIGNOS_REFERENCE_DIR`).

- `instance_text_override_with_two_paragraphs_and_mixed_styles_imports` (the issue AC). Component `Note` with child text `Supporting text`, `"Lorem ipsum"`, `overrideKey` = sentinel. An instance overrides `guidPath [child]` with:
  - `characters: "Hello\nWorld"`;
  - `characterStyleIDs: [0,0,0,0,0,0,1,1,1,1,1]`;
  - `styleOverrideTable: [{styleID: 1, styleIdForText: <Semibold style guid>, textDecoration: UNDERLINE}]`.

  Pad the document to 10,001 nodes to force fast-clone. Expect the content `"Hello\nWorld"`, and run `6..11` with weight 600 and underline.
  - Fails before: yes.
- The same without padding (slow path): same expectation.
- `sentinel_guid_is_treated_as_unset`: `guid_str` of the sentinel is `None`.
- Kit: `untitled_ui_instance_text_overrides_resolve` (ignored). Assert that the frames `3936:417924` and `3936:417929` contain no text node whose characters start with `"Lorem ipsum"` inside an instance with a text override, and count the remaining placeholder nodes against the issue's 125. Run it on the owner machine and paste the counts in the PR.

## 11. Performance budgets

- No `CLAUDE.md` §4 invariant applies to the import path.
- Fast-clone stays O(n). The extra key registration is one map insert per cloned node.

## 12. Dependencies and build order

- No issue dependency.
- `fig.rs` and `rest_import.rs` are hot files: serialise with the slot queue (#111, #112, #107, #108, #110) and with #522, #525 and #5.
- Suggested order among the import bugs: #518, then #522, then #525.

## 13. Owner rulings that apply

- Hot-file rule (`docs/releasing.md`): tasks touching `fig.rs`, `kiwi.rs` or `rest_import.rs` run one after another.
- The kit-render audit is the evidence source. The owner rule on marketing videos names the Untitled UI kit as the demo kit; the kit file stays local.
