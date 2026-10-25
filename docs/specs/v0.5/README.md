# v0.5 Components: task specs

Pack goal: **components that behave like Figma's.**
- Slots can be authored, filled, migrated, exported and generated in code.
- Check designs covers Figma's rule set at any page size.
- Developers get a read-only component playground.
- Imported kits keep their instance overrides, variants and resized children.

Milestone: [v0.5 Components](https://github.com/jangtrinh/design-os-studio/milestone/3). Epic: #16 (E3 Slots & Check designs).

These specs follow the spec-ready gate in `docs/releasing.md` (owner decision of 2026-10-18; PR #579). They were written against `main` at `767acf9` on 2026-10-18.
- Already merged, and the base for these specs: PR #45 (Check designs), PR #49 (slot model) and PR #53 (zero-fake-UI gate).
- An open PR listed below is the starting branch. Each spec states the remaining work and every release-review finding.
- PR heads move. Before coding, re-anchor the spec against the PR head and report any drift.

## Index

| Issue | Title | Spec | Status (2026-10-18) |
|---|---|---|---|
| #160 | Verify VSF - PCP.fig slot data, synthetic fixture | [160](160-verify-vsf-pcp-slot-data-and-synthetic-fixture.md) | PR #495, changes requested (1 MAJOR); release session fixing |
| #111 | Reject raw-number slot-list fallback | [111](111-reject-raw-number-slot-list-fallback.md) | PR #492, not yet reviewed |
| #112 | Slot doc-op semantics, unknown SlotPropConfig keys | [112](112-slot-doc-op-semantics-and-unknown-slot-config-keys.md) | PR #493, 1 MAJOR (Duplicate/Paste keep the guid); release session fixing |
| #107 | Real preferredValues object (SLOT, INSTANCE_SWAP) | [107](107-real-preferred-values-object-for-slot-and-instance-swap.md) | PR #510, 2 open MAJORs (round 2); release session fixing |
| #108 | Slot content id per property | [108](108-slot-content-id-per-property.md) | PR #512, fix round 2 at `4bc2bb8`; legacy migration dropped (needs owner confirmation, Q108-1) |
| #110 | Export .fig with Figma's real kiwi numbering | [110](110-export-fig-with-figma-real-kiwi-numbering.md) | PR #514, 1 MAJOR (invented tags 701+); release session fixing |
| #109 | Paste from Figma remaps slot and swap ids | [109](109-paste-from-figma-remaps-slot-and-swap-ids.md) | PR #494, not yet reviewed; gate ran with `-A dead_code` (re-run without) |
| #32 | Slot authoring UI | [32](32-slot-authoring-ui-create-slot-and-settings.md) | PR #502, 1 open MAJOR (preferred multi-select); release session fixing |
| #34 | Edit slot content in instances | [34](34-edit-slot-content-in-instances-without-detaching.md) | PR #503, 3 MAJORs + 1 MINOR; release session fixing |
| #150 | Empty-slot placeholder, preferred picker, stretch | [150](150-empty-slot-placeholder-preferred-picker-stretch-on-insert.md) | PR #508, 4 MAJORs, plus the stretch axis error found while writing this spec; release session fixing |
| #151 | Migrate swap or boolean regions to slots | [151](151-migrate-instance-swap-or-boolean-regions-to-slots.md) | PR #533, 2 MAJORs; release session fixing |
| #152 | Slots in codegen and Dev Mode inspect | [152](152-emit-slots-in-codegen-and-dev-mode-inspect.md) | PR #496, 3 MAJORs + 1 MINOR; release session fixing |
| #159 | Read-only component playground | [159](159-read-only-component-playground-in-dev-mode.md) | PR #501, 3 MAJORs; release session fixing |
| #52 | Check designs inside instances | [52](52-check-designs-audit-text-and-overrides-inside-instances.md) | PR #577, not yet reviewed |
| #65 | Contrast over opaque parents and solid siblings | [65](65-check-designs-recover-contrast-over-opaque-parents-and-siblings.md) | PR #576, fix round at `2707f3c`, re-review pending; 1A reading needs owner confirmation (Q65-1) |
| #68 | Recolour text runs on colour style apply | [68](68-recolour-text-runs-when-applying-colour-style.md) | PR #575 (test only); the review MAJOR contradicts owner ruling 2B, so it is rejected |
| #154 | Hard-coded numbers to number variables | [154](154-check-designs-hard-coded-numbers-matching-number-variables.md) | Claimed, no PR |
| #155 | Suggestions from enabled libraries | [155](155-check-designs-suggest-variables-and-styles-from-enabled-libraries.md) | Not started |
| #157 | Whole-page incremental scan | [157](157-check-designs-scan-whole-pages-incrementally.md) | Not started |
| #158 | Figma rule set, thresholds, ignore | [158](158-check-designs-figma-rule-set-thresholds-and-ignore.md) | Not started |
| #518 | Instance multi-paragraph and mixed-style text overrides | [518](518-instance-multi-paragraph-and-mixed-style-text-overrides.md) | PR #537, not yet reviewed |
| #522 | Resized instances: Stretch and Scale children | [522](522-resized-instances-apply-stretch-and-scale-constraints.md) | PR #538, not yet reviewed |
| #525 | Nested swap keeps the primary variant shell | [525](525-nested-instance-swap-applies-variant-shell.md) | PR #540, 1 MAJOR (kit detector coverage); release session fixing |
| #5 | Import: global clone budget, review backlog | [5](5-import-global-clone-budget-and-review-backlog.md) | Not started; item 4 (NaN as null) is already fixed on `main` |

The epic #16 has no spec of its own.

## Build order

Hot files (`fig.rs`, `kiwi.rs`, `rest_import.rs`, `fig_export.rs`) run one at a time (`docs/releasing.md`).

1. **Fig queue, serial:** #160 → #111 → #112 → #107 → #108 → #110.
   - #111 introduces `ImportNotes`, which #107 and #5 extend.
   - #160 supplies the real-numbered synthetic fixture for #107 and #110.
2. **Import bug queue, serial, interleaved with the fig queue on the same hot files:** #518 → #522 → #525 → #5.
   - #518 and #522 share the null-GUID sentinel rule; whichever lands first implements it.
3. **App, slots:** #112 (`clipboard.rs`) → #109.
4. **Slot UX:** #32 → #34 → #150. #151 comes after #34.
   - #34 and #150 share the `sync_pair` Reset fix and the insert-target fix. Fix them once, in #34.
5. **Codegen and Dev Mode:** #152 and #159 are independent of 1–4. They share `dev/`, so merge #152 first.
6. **Check designs:** #65 → #52, plus #68 at any time. Then #154 → #155 → #158 → #157.
   - #157 goes last, so every rule runs in the sliced walk.

## Facts found by REA for these specs

Method: read-only string extraction (`rg -a`, `strings`) from the Figma 126.9.13 desktop editor's V8 code cache (`~/Library/Application Support/Figma/DesktopProfile/v43/Code Cache/js`), which includes the embedded kiwi schema text. `rea-agents` 6.7.0 was available through `npx`. The facts below came from the string scan. Nothing was copied into the repo.

- Kiwi schema:
  - `ComponentPropDef{…preferredValues=8, varValue=9, slotPropConfig=12…}`, `ComponentPropAssignment{defID=1, value=2, varValue=3}`, `ComponentPropRef{nodeField=1, defID=2, zombieFallbackName=3, componentPropNodeField=4, isDeleted=5}`;
  - `ComponentPropPreferredValues{stringValues=1 (deprecated), instanceSwapValues=2, parameterValues=3}`, `InstanceSwapPreferredValue{type=1, key=2}`;
  - `VariableAnyValue.slotContentIdValue=18`, `VariableDataType.SLOT_CONTENT_ID=18`, `VariableResolvedDataType.SLOT_CONTENT_ID=12`;
  - `NodeChange`: `componentKey=216`, `isStateGroup=225`, `key=319`, `overrideKey=213`, `stackPaddingRight=233`, `stackPaddingBottom=234`, `stackHorizontalPadding=209`, `stackVerticalPadding=210`, `gridRowAnchor=439`, `gridColumnAnchor=440` (GUID), `connectorStartCap=245`, `connectorEndCap=246`;
  - `NodeChange` has no `jsonSerialized`, `stackPaddingTop`, `stackPaddingLeft` or `grid*AnchorIndex`;
  - `NodeType` has no SLOT value; a slot is a FRAME with `isSlot=463`;
  - no linter, violation or ignore field anywhere in the schema.
- Check designs linter rule ids, with verbatim descriptions in spec 158:
  - `REQUIRE_NO_DETACHED_COMPONENTS`, `REQUIRE_ASSETS_FROM_SELECTED_LIBRARIES`, `REQUIRE_TEXT_STYLES`, `RECOMMEND_VARIABLES`;
  - `REQUIRE_CORNER_RADIUS_VARIABLES`, `REQUIRE_PADDING_VARIABLES`, `REQUIRE_HORIZONTAL_SPACING_VARIABLES`, `REQUIRE_VERTICAL_SPACING_VARIABLES`;
  - `EXTENSIBILITY_RULE`.
- Contrast module:
  - categories `largeText`, `normalText`, `graphics`; levels AA and AAA;
  - unavailable reasons `TextInBackground`, `VideoInBackground`, `ImageInBackground`, `GradientInBackground`, `BlendModeInBackground`, `BlendModeInForeground`, `MixedBackgrounds`, `MixedStandards`, `Unavailable`.
- Slot editor commands and bindings:
  - `CONVERT_TO_SLOT`, `GROUP_SELECTION_INTO_SLOT`, `RESET_SLOT_ASSIGNMENT`, `SLOT_PREFERRED_INSTANCE_INSERT`, `EMPTY_SLOT_CONTENTS_ID`;
  - `canConvertSelectedSlotsToFrame`, `SlotLimitsBindings_computeLimitViolations`, `selectNonPreferredChildren`;
  - no "migrate to slots" command exists (0 hits).
- Figma Plugin API docs (developers.figma.com, read 2026-10-18):
  - `SlotSettings.stretchChildOnInsert` = "apply counter-axis fill";
  - `displayEmptyByDefault` = "slot highlight is always shown on empty instances";
  - `SlotNode.limitViolations` ∈ {`BELOW_MIN`, `ABOVE_MAX`, `HAS_NON_PREFERRED`}; edits that break limits do not throw.
- The owner's `VSF - PCP.fig` (124,453,334 bytes): measured on issue #160 with 0 slot records and 15 INSTANCE_SWAP `preferredValues` objects. Not re-decoded for these specs. The file was not copied, and no content from it is quoted.

## Unresolved questions

Owner-visible:
- **Q108-1.** Accept the release session's reading of #108 AC2 ("old files migrate" → "old files keep working unchanged, no guessed migration")?
- **Q65-1.** Owner ruling 1A says "skip only, no compositing of a covering child". Does adopting a single opaque, solid, fully covering sibling as the backdrop (no compositing; #65 probe B) fit 1A?
- **Q68-1.** Amend the #68 issue AC ("recolours every run") to cite ruling 2B (matching runs follow, highlighted runs keep their colour)?
- **Q160-1.** #160 found no slots in the owner's file. Will the owner provide a Figma file that contains authored slots for the Figma.app check in #110, or should the synthetic fixture suffice?
- **Q158-1.** Default contrast level for Check designs: AA (current code) or AAA?

Technical, unresolved by REA:
- **Q32-1.** The exact Figma UI3 English labels of the slot settings fields. The i18n strings are not in the local cache.
- **Q107-1.** Does REST `preferredValues` write `"COMPONENT_SET"` or `"STATE_GROUP"` for set entries? The import accepts both.
- **Q158-2.** Does Figma persist ignored Check designs findings, and where? It is not in the `.fig` scene schema.
- **Q522-1.** How does Figma resize a rotated child under non-uniform Stretch?
- **Q5-1.** Where is the PR #1 review report (~30 MEDIUM, 13 LOW)?

Technical decisions recorded rather than asked:
- **#157** uses time-sliced scanning on the UI thread instead of a background thread. A thread would need a whole-document clone.
- **#151** migrate semantics are Studio's own, because Figma 126.9.13 has no equivalent command.
- **#34** blocks limit-breaking edits with a toast, per the AC, where Figma only flags violations.

## Owner answers (2026-10-18)
- **Q108-1:** old files open unchanged, with no migration step. Readers accept both the old per-node form and the new per-property form, and saving writes the new form.
- **Q158-1:** Check designs contrast defaults to **WCAG AA**, with AAA as an option.
- **Q65-1:** allowed. A single opaque sibling that fully covers the text may be used as its background. This fits ruling 1A: it is one known backdrop, not compositing.
- **Q160-1:** the self-made fixture is enough for #110. No check with a real Figma file is required.
- **Q68-1:** the coordinator amends the issue text of #68 to cite ruling 2B: only runs that match the old colour follow the new colour.
