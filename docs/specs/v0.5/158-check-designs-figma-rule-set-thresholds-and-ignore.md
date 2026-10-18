# #158 Check designs: match Figma's rule set, thresholds and dismiss/ignore

- Issue: https://github.com/jangtrinh/design-os-studio/issues/158 (task E3-13, epic #16, M / P2)
- Spec base: `main` at `767acf9`.
- Open PR: none.
- Files: `crates/designos-app/src/check_designs/*` (new rule modules `text_styles.rs` and `libraries.rs`, plus `mod.rs`, `state.rs`, `contrast.rs`), `panels/check_designs.rs`; `crates/designos-doc/src/lib.rs` and a new `check_designs_settings.rs` (document field).

## 1. Summary and user story

As a designer used to Figma's Check designs, I want:
- the same rules, each one switchable;
- contrast at AA or AAA, with the right large-text threshold;
- to ignore a finding I consider intentional, and have it stay ignored, saved in the file, after rescans and reopen.

## 2. Current state

- FACT: Studio rules are `HardCodedValue`, `Contrast` and `DetachedInstance` (`check_designs/mod.rs:34-38`). Every rule is always on.
- FACT: contrast uses AA only: 4.5:1, or 3:1 for large text, where large means ≥ 24 px or ≥ 18.66 px at weight ≥ 700 (`contrast.rs:55-62`).
- FACT: there is no ignore, dismiss or rule settings, and nothing is stored in `Document` (`designos-doc/src/lib.rs:142-195`).
- REA (E3-d), FACT from the Figma 126.9.13 editor bundle (V8 code cache strings). The Check designs linter (`DESIGN_LINTER`, `LinterBindings`, `LinterCppBindings`) registers these rules, each with `enabled`, `detect`, `getFix` and `applyFix`:

  | Figma rule id | Description (verbatim) |
  |---|---|
  | `REQUIRE_NO_DETACHED_COMPONENTS` | "Detects nodes that have been detached from a component." |
  | `REQUIRE_ASSETS_FROM_SELECTED_LIBRARIES` | "Detects assets that are not from the selected libraries." |
  | `REQUIRE_TEXT_STYLES` | "Detects missing text styles for text nodes." |
  | `RECOMMEND_VARIABLES` (`COLOR_PROPERTY`) | "Unified color rule: detects hardcoded colors, wrong-library variables, and contrast issues for fill and stroke properties." |
  | `REQUIRE_CORNER_RADIUS_VARIABLES` | "Detects missing number variables for corner radius fields, and applies a fix to use the most appropriate variable." |
  | `REQUIRE_PADDING_VARIABLES` | same wording, "stack padding fields" |
  | `REQUIRE_HORIZONTAL_SPACING_VARIABLES` | same wording, "horizontal spacing fields" |
  | `REQUIRE_VERTICAL_SPACING_VARIABLES` | same wording, "vertical spacing fields" |
  | `EXTENSIBILITY_RULE` | "This rule ONLY exists for the Rule ID. All detection and fix logic are handled in design_linter_custom_rules.ts" (custom rules; not ported) |

  Other strings found:
  - `counts_by_rule`, `check_designs_show_violation_count`, `seen_linter_onboarding_overlay`;
  - `MAX_VARIABLE_SET_THUMBNAILS_IF_CHECK_DESIGNS_ENABLED`;
  - an MCP tool name pattern `^mcp__.+__check_designs$`.

  The shared contrast module has the categories `largeText`, `normalText` and `graphics`, AA and AAA levels (`contrastLevelForCategory`, `AAA`), and the unavailable reasons `TextInBackground`, `VideoInBackground`, `ImageInBackground`, `GradientInBackground`, `BlendModeInBackground`, `BlendModeInForeground`, `MixedBackgrounds`, `MixedStandards`, `Unavailable`.
- REA negative result: the embedded kiwi schema has no linter, violation or ignore field (0 hits for `lint`, `violation` or an ignore list in `NodeChange` or the document messages). Figma does not persist ignores inside the `.fig` scene.

## 3. Behaviour rules

1. **Rule set.** Studio exposes these rules, keyed by Figma's ids:

   | Studio rule (panel group) | Figma id | Status |
   |---|---|---|
   | `Detached instances` | `REQUIRE_NO_DETACHED_COMPONENTS` | exists |
   | `Library assets` | `REQUIRE_ASSETS_FROM_SELECTED_LIBRARIES` | new (rule 4) |
   | `Text styles` | `REQUIRE_TEXT_STYLES` | new (rule 5) |
   | `Colours` | `RECOMMEND_VARIABLES` | exists: hard-coded colours, plus `Contrast`, which keeps its own group |
   | `Corner radius` | `REQUIRE_CORNER_RADIUS_VARIABLES` | from #154 |
   | `Padding` | `REQUIRE_PADDING_VARIABLES` | from #154 |
   | `Horizontal spacing` | `REQUIRE_HORIZONTAL_SPACING_VARIABLES` | from #154 (`itemSpacing` of horizontal stacks) |
   | `Vertical spacing` | `REQUIRE_VERTICAL_SPACING_VARIABLES` | from #154 (`itemSpacing` of vertical stacks and `counterAxisSpacing`) |

   Studio's extra number properties from #154 (stroke weight, font size, line height) live under a Studio rule `REQUIRE_TYPOGRAPHY_AND_STROKE_VARIABLES`, labelled `Typography and stroke`.
2. **Rule toggles.**
   - Each rule can be enabled or disabled per document. The default is all enabled.
   - A disabled rule is not evaluated.
   - Toggles live in a `Rules` popover in the panel header.
3. **Contrast thresholds.** Document setting `contrast_level`: `AA` (default) or `AAA`.

   | Level | Normal text | Large text |
   |---|---|---|
   | AA | 4.5:1 | 3:1 |
   | AAA | 7:1 | 4.5:1 |

   Large text means font size ≥ 24 px, or ≥ 18.66 px with weight ≥ 700 (unchanged from `contrast.rs:56-61`). The fix suggests the mildest recolour that reaches the active level.
4. **Library assets rule.** Flag a node using any of the following when its asset's `library_id` is not in `doc.library.enabled`:
   - a component instance whose main is a library copy (`doc.library.external` entry of kind Component or ComponentSet);
   - a style link (`style_refs` value whose key is an external style);
   - a variable binding to an external variable.

   Message: `Uses "{asset}" from library "{name}", which isn't enabled in this file`. No fix: the user decides whether to enable the library or swap.
5. **Text styles rule.** Flag a visible text node that has no `style_refs["text"]`.
   - If exactly one local or enabled-library text style equals its text properties (the existing exact match in `hardcoded.rs`), the fix is `Apply style`.
   - Otherwise no fix. Message: `No text style`.
   - Findings for the same node from the hard-coded rule's text-style match are merged into this rule, so there is no duplicate row.
6. **Ignore.**
   - Every row has `Ignore`. Ignoring adds an `IgnoreKey { node, rule, property }` to the document, as one undo step. `property` is the binding key (`fills/0`, `itemSpacing`), or empty for whole-node rules.
   - An ignored finding never appears again while its node, rule and property match, even after rescans and reopen (AC3).
   - The panel footer shows `{n} ignored` with a `Show` toggle that lists ignored findings, each with `Restore`.
   - Deleting the node removes its keys on save (garbage collection).
7. **Dismiss** is a session-only hide (`Dismiss` row action, not saved). It reappears after reopen. The persistent form is Ignore (rule 6), stored in the document because AC2 requires it. UNRESOLVED (Q158-2): whether Figma persists ignored findings, and where. The `.fig` scene schema has no such field.

## 4. Data model / schema changes

```rust
// designos-doc
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CheckDesignsSettings {
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub disabled_rules: BTreeSet<String>,          // Figma rule ids
    #[serde(default, skip_serializing_if = "is_aa")]
    pub contrast_level: ContrastLevel,             // "AA" | "AAA", default AA
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub ignored: BTreeSet<IgnoreKey>,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct IgnoreKey { pub node: NodeId, pub rule: String, #[serde(default, skip_serializing_if = "String::is_empty")] pub property: String }
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContrastLevel { #[default] AA, AAA }
// Document
#[serde(default, skip_serializing_if = "CheckDesignsSettings::is_empty")]
pub check_designs: CheckDesignsSettings,
```

- Back-compat: absent means default, so old files load unchanged and new files without settings are byte-identical.
- `renumber_ids` remaps `IgnoreKey.node` the same way other `NodeId` fields are remapped (`renumber_ids.rs`).
- A new `Command::SetCheckDesignsSettings(CheckDesignsSettings)` with an inverse, so ignore, toggles and level changes are undoable.
- `.fig`: not exported. There is no Figma field (REA negative result). The settings are lost on `.fig` export, which is acceptable and stated in the export doc comment.

## 5. Contracts

- `pub enum Rule` gains `LibraryAssets`, `TextStyles`, `CornerRadius`, `Padding`, `HorizontalSpacing`, `VerticalSpacing`, `TypographyAndStroke`.
- `fn figma_id(self) -> &'static str`.
- `Finding` gains `pub property: String` for `IgnoreKey`.
- `App::check_designs_ignore(&mut self, f: &Finding) -> bool`, `App::check_designs_restore(&mut self, key: &IgnoreKey) -> bool`, `App::set_check_designs_rule(&mut self, rule: Rule, on: bool)`, `App::set_contrast_level(&mut self, l: ContrastLevel)`.
- MCP: none in this task. Figma's MCP has a `check_designs` tool (REA). A Studio MCP tool is a follow-up.

## 6. File-format mapping

None for `.fig`, because there is no Figma field. Native `.designos` only (§4). RESOLVED, with the REA negative result recorded.

## 7. UI

- **Panel header.** A `Rules` button (24×24, Phosphor `Sliders`). `Icon` has no `Sliders` today (`icons.rs:15-140`), so vendor it per the CONTEXT.md icon rule. It opens a popover with:
  - one checkbox row per rule (28 px, Figma rule label);
  - a segmented control `AA | AAA` for contrast.
- **Rows.** `Ignore` and `Dismiss` are in the row overflow menu, which keeps the row compact. The `Fix` button is unchanged.
- **Footer.** `{n} ignored` as a count badge (P5) with a `Show` toggle. Ignored rows render muted, with `Restore`.
- **Rules.** Zero fake UI: every toggle changes the audit. Tokens only.
- **Evidence.** A 1440 capture of the popover, the panel with ignored findings shown, and AAA mode.

## 8. Edge cases

1. `#767676` 14 px on white = 4.54:1: passes AA, fails AAA (finding in AAA mode only).
2. `#595959` 14 px on white = 7.00:1: passes AAA.
3. 24 px `#767676` on white: AAA large needs 4.5, ratio 4.54, so it passes.
4. Ignore the gap finding on frame F, rescan: absent. Save, reopen: still absent. Change F's gap to another value that matches a different variable: the key `(F, REQUIRE_HORIZONTAL_SPACING_VARIABLES, itemSpacing)` still matches, so it stays ignored. Documented, matching per-property ignore.
5. Ignore, then undo: the finding is back.
6. Disable `Text styles`: those findings vanish. Re-enable: they return.
7. An instance from a library that is later disabled in Libraries: a `Library assets` finding appears with no fix.
8. Duplicate an ignored node: the copy is not ignored, because it has a new id.

## 9. Non-goals

- Custom rules (`EXTENSIBILITY_RULE`, `design_linter_custom_rules.ts`).
- An org-wide policy (`FileCanUseDesignLinterWithReasons`). Studio is local-first, with no gating (full-scope audit technical decision: "Check designs ships to all users").
- Graphics (non-text) contrast. Figma's `graphics` category is noted; a follow-up can add it.

## 10. Test plan

Seams: `crates/designos-app/src/check_designs/tests/rules.rs` (new) and `crates/designos-doc` settings round-trip tests.

| Test | Fails before |
|---|---|
| `every_figma_rule_id_has_a_studio_rule` (the table in §3.1; `Rule::ALL.map(figma_id)` contains all 8 ids) | yes |
| `aaa_level_flags_767676_which_aa_passes` | yes |
| `aaa_large_text_threshold_is_4_5` | yes |
| `ignored_finding_never_reappears_after_rescan_and_reload` (ignore, `to_json`, `from_json`, audit) | yes |
| `ignore_is_one_undo_step` | yes |
| `disabled_rule_is_not_evaluated` | yes |
| `text_without_text_style_is_flagged_with_apply_style_when_exact_match` | yes |
| `asset_from_disabled_library_is_flagged_without_fix` | yes |
| `settings_round_trip_and_old_files_load` (doc test; empty settings write no key) | yes |
| `renumber_ids_remaps_ignore_keys` | yes |

## 11. Performance budgets

- `CLAUDE.md` §4.1: settings are serialized through serde, and `doc_hash` streams them. No `to_json` on the UI thread.
- Ignore lookup is `BTreeSet::contains`, O(log n) per finding.

## 12. Dependencies and build order

- Depends on #35 (merged), #154 (number rules) and #155 (library sources, used by `Library assets` and `Text styles`).
- Before or with #157. If #157 lands first, the new rules must run inside its sliced walk.

## 13. Owner rulings that apply

- **R2.7** (round 2, item 7): exact matches only, for every Bind and Apply-style fix here.
- Full-scope audit technical decision: Check designs ships to all users, with no Org/Enterprise gating.
- Owner default: 1440 capture only.
- UNRESOLVED (owner-visible), Q158-1: the default contrast level. AA is chosen, matching the current code. Figma's default is not confirmed.
