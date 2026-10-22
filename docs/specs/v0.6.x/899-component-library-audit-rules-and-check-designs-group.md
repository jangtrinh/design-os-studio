# #899 Audit the component library for unused, duplicate and dead components (lint + Check designs)

| Field | Value |
|---|---|
| Issue | #899 (enhancement). Owner ruling 2026-10-22: "Cái gì hữu ích thì plan đi spec driven rồi tạo task". Controller ruling: lint rules through `designos_lint` and a new group in Check designs. |
| Size | M (upper end, about three days: one shared index, five rules, lint plumbing, one panel group, fixtures, generated docs) |
| Depends on | Nothing open. The incremental Check designs scan (`ScanCursor` / `audit_step`) and the ignore list (`CheckDesignsSettings.ignored`) are already on `main`. |
| Unblocks | The follow-ups in section 9 (cross-file usage count, unnamed variant axes). |
| Touches | new `crates/designos-doc/src/lint/rules/library/` (`mod.rs` index and verdicts, `segment.rs` ds/icon/screen, `signature.rs` unit hash, `tests.rs`); `lint/registry.rs` (5 rules, preset `library`), `lint/rules/mod.rs` (`ActiveRules`, `NodeEvalCtx`, `evaluate_node`), `lint/traverse.rs` (index built next to `Components`), `lint/finding.rs` (`LintReport.library`); `designos-mcp/src/engine.rs` (tool text, `mcp_lint_returns_full_rule_set`), generated `generated/rules.md` and `tests/snapshots/tools.json`; `scripts/check_vendor_skills.py`; `designos-render/tests/lint_rules.rs`; `designos-app/src/check_designs/{mod.rs,state.rs,library.rs (new),fixture.rs}`, `panels/check_designs.rs`. Hot files: `lint/traverse.rs`, `check_designs/mod.rs`. |

Code baseline: `origin/main@42a0d027`. Re-anchor every cited line against HEAD and report drift before writing code. "P" below is `jangtrinh/design-os-figma-plugin@7c1ef8bb` (TypeScript on the Figma Plugin API: logic is ported, no Figma call is).

## 1. Summary and user story

As the owner of a large kit (Untitled UI, VSF PCP), I want Studio to tell me which components nobody uses, which share a name, which sets are empty, which variants are dead and which masters repeat another one's layers, without 1,469 icons and a dozen screens drowning the list, so that I can clean the library with evidence. The same verdicts are available to agents through `designos_lint` and to designers as one group in Check designs.

## 2. Current state (FACT unless labelled)

- Lint is per layer. 27 rules live in `RULES` (`lint/registry.rs:149-417`); presets are `LintPreset { Agent, CheckDesigns, Recommended, Strict }` (`:99-108`); category `Components` exists and holds only `detached-instance` (`:250-258`, presets CheckDesigns/Recommended/Strict). One document-level rule exists, `broken-style-binding` (`:409`); it has no owning node and is folded in by hand (`style_bindings.rs:1-4`, `engine.rs:1045`). Library findings do have an owning node (the master), so they use the per-node path.
- Per-node rules get document-wide state through `NodeEvalCtx` (`rules/mod.rs:180-198`), built next to the `detached::Components` index: per call in `lint_subtree_with_components` (`traverse.rs:487-496`), once per run in `lint_in_slices_with_budget` (`:643-645`), lazily in `SingleNodeLinter` (`:346-376`). Ignores are applied after the scan by `apply_check_designs_ignores` (`:445`) through `doc.is_lint_ignored_property` (`lib.rs:817`); the key is `IgnoreKey { node, rule, property }` (`settings.rs:24-29`).
- `designos_lint` roots are `page_id`, `node_id`, `node_ids`, else the default page (`engine.rs:911-946`; all pages only when no default page resolves). `LintReport` is built at `traverse.rs:550` and `:746`; `by_category` is a fixed 8-array (`:569`), so no new category is added.
- Tests that pin the registry: `mcp_lint_returns_full_rule_set` expects every `all_rule_ids()` rule to fire under `strict` (`engine.rs:2816-3000`); `same_findings_three_callers` expects exactly one finding for the `check-designs` preset on a document holding an unused `Component` (`engine.rs:3008-3070`); `strict` must enable every rule (`lint/mod.rs:260-263`); `CANONICAL_LINT_RULE_IDS` in `scripts/check_vendor_skills.py:37` must equal the registry (`designos-mcp/tests/list_lint_rules.rs`).
- Check designs runs its own scan with its own `Rule` enum of 10 groups (`check_designs/mod.rs:39-64`); it uses `designos_doc::lint` only for the shared detached rule (`check_designs/detached.rs:9-31`), so the lint registry and the panel do not share a rule list. The scan is time-sliced: `SLICE_BUDGET` = 2 ms (`mod.rs:35`); the Prepare phase already walks every node of the document once, whatever the scope, to fill `Components` (`:321-345`), counts the scope's layers (`:347-358`), then builds the token index in one shot (`:360`); the Walk phase visits only the scope's layers (`:367-434`). Scope is fixed when the panel opens: the selection, else the current page (`state.rs:98`, `open_check_designs`). Rows show an Ignore and a Dismiss button, a Fix button only with a fix, and a click selects the layer and fits it (`panels/check_designs.rs:395-430`, `Act::Focus` `:263`). A row whose message contains ` from library "` gets a "Library" badge (`mod.rs:133-135`).
- Model: a master is a `ComponentSet`, or a `Component` that is not a child of a set; masters on pages are listed by `local_component_roots` (`library_ops.rs:495-506`), which `apply_spec` uses as its name index, keyed by the exact stored name (`spec_instance.rs:222-226`), and which answers `AMBIGUOUS_COMPONENT` when a name matches twice (`:478-493`). Components copied from enabled libraries live on a hidden page not in `doc.pages` (`library.rs:1-6`, `library_ops.rs:442`). An instance is `NodeKind::Instance { component: Option<NodeId>, .. }` and points at the chosen variant, not the set (`components.rs:577-600`, swap `:1316-1333`); `instances_of` is the usage predicate (`:138`). Variant props, instance-swap defaults/preferred values and slot `preferred` lists are `ComponentPropDef`, `PropKind::InstanceSwap` (`node.rs:205-219`) and `SlotPropConfig` (`node.rs:225-245`).
- What Studio can know about library usage: the instances in this file; whether this file is published and at which version (`doc.library.published: Option<PublishInfo>`, `library.rs:55-56`); which libraries it consumes (`library.enabled`, `library.external`). A search of `designos-library`, `designos-app` and `designos-doc` for consumers or "used by" finds no code that knows which other files use a published library (INFERENCE: none exists). So instances in other files cannot count as usage.
- Ignores shipped as `CheckDesignsSettings.ignored`, a set of `IgnoreKey { node, rule, property }` (`settings.rs:24-46`), not as the fingerprinted `check_ignored` list that spec #158 sketches (§3 rules 3-4): no `check_ignored` exists in `crates/*/src`. Spec #158's rule table has no thresholds for library rules.
- Claim check, "junk-name and token-violation are already covered" (study report):
  - junk-name: TRUE for names. `default-layer-name` runs on every visited node including `Component` and `ComponentSet`, with no kind filter (`rules/mod.rs:641-643`); `is_default_layer_name` matches empty, and `Frame|Group|Rectangle|Ellipse|Line|Polygon|Star|Vector|Text|Section|Component|Instance|Layer` plus optional digits (`:32-52`), a superset of P's `component|frame|group` (`audit-ds-detect.ts:93`). But the rule is in `Recommended` and `Strict` only (`registry.rs:261-267`): not `agent`, not `check-designs`, and Check designs never shows it. FALSE for P's "every variant axis is `Property N`" (`:94`, `:155`): nothing checks `ComponentPropDef` names, and INFERENCE: Studio does not create such a name (no `"Property 1"` literal in `crates/*/src`), so it appears only after a `.fig` import.
  - token-violation: TRUE only for the narrow case. `hardcoded-value` reports a value that equals an existing variable or style (`rules/scale.rs:831-880`); P flags any unbound SOLID paint on the master and its direct children (`audit-ds-detect.ts:195`). The wider check is `hardcoded-color`, `Strict` only (`registry.rs:390`, `rules/mod.rs:686-742`). Neither is ported.
- P facts ported (pure functions over raw facts): segmentation `classifyAll` (`audit-ds-structure.ts:87-101`; constants `:17,:64`), `unitHash` / `comparable` (`:104-122`), `detectStructure` incl. dead variants (`:138-212`), the detector bodies (`audit-ds-detect.ts:142-200`), tombstone normalisation (`audit-ds-families.ts:18-20`). P's facts gatherer caps a unit at 300 nodes, depth 10, 2,000 text characters and does not descend into instances (`executor-audit-units.ts:13-16,105-107`). P's other detectors (deprecated, misfiled, redundant-family) are not in the issue and are listed in section 9.
- UNKNOWN: how many masters the 112,474-node "VSF - PCP" benchmark holds (P saw about 1,590 on its VSF file). Closing check: a throwaway `#[ignore]` test that counts `Component|ComponentSet` nodes on the opened benchmark.
- UNKNOWN: whether an imported `Instance { component: None }` can stand for a local master (the importer resolves by guid, so INFERENCE: no). Closing check: import the benchmark `.fig`, count top instances with `component: None`, compare their symbol ids with local masters' `source_id`.

## 3. Behaviour rules

1. **Master.** The unit of audit is what `local_component_roots` returns: every `ComponentSet`, and every `Component` whose parent is not a `ComponentSet`, under `doc.pages`. Not masters: variant children (judged through their set), anything on the hidden library page, anything with `component.source.is_some()` (a mirror copy inside an instance).
2. **Segmentation** (P `classifyAll`). Per master, in this order: *screen* when its own `size` is at least 1000 x 700; *icon* when it is vector-only and at least 20 vector-only masters share its name prefix (text before the first `/`, trimmed; `""` when none); else *design system*. Vector-only: every unit has no text and every node of its capped walk is Frame, Group, Rectangle, Ellipse, Line, Polygon, Star, Vector, BooleanOp or Component (units are a set's variants, or the master itself; a unit that hit a cap is not vector-only). Constants `SCREEN_MIN_W/H`, `MIN_ICON_FAMILY`, `MAX_UNIT_NODES` 300, `MAX_UNIT_DEPTH` 10, `MAX_UNIT_TEXT` 2000 are named consts, not settings. Icons and screens get no finding and are counted.
3. **Usage.** `uses(c)` = number of `Instance` nodes with `component == Some(c)` anywhere in `doc.nodes`, hidden or not, mirror copies included (the `instances_of` predicate; counting mirrors only errs toward "used", and it keeps an instance swapped inside another instance counted). A set's uses are its variants' plus any pointing at the set id. A master also counts as used when it is the default value or a preferred value of an instance-swap property, or in a slot's `preferred` list (a preferred set covers its variants). Instances with `component: None` count for nothing.
4. **Scope: facts are file-wide, findings follow the scope.** The index is always built over every page, so a component used only on another page is never reported unused. A finding is emitted only when the master is visited by the traversal, i.e. lies inside the lint roots (`page_id`, `node_id`, `node_ids`, default page) or the Check designs scope. Whole file through MCP: `node_ids` = every page id (a `Page` root is accepted, `traverse.rs` `push_root_items`). Hidden masters are skipped, like every rule.
5. **The five rules** (design-system masters only). Ids are canonical; aliases are P's names.

| id (aliases) | Fires on | Message | Severity | Presets |
|---|---|---|---|---|
| `unused-component` | uses of the master (set: all variants) is 0 and nothing references it (rule 3) | `No instances in this file`; on a published file append ` · other files may use it` | info | library, strict |
| `duplicate-component-name` (`duplicate-name`) | at least 2 design-system masters with the same stored name (exact and case-sensitive, the key `apply_spec` resolves by; blank names skipped; so `X` and `❌ X` differ, as in P) | `Same name as {n} other components` (singular for 1) | warning | library, strict |
| `empty-component-set` (`empty-set`) | a `ComponentSet` with at most 1 `Component` child | `Component set with no variants` / `Component set with 1 variant` | warning | library, strict |
| `dead-variant` (`dead-variants`) | a set with uses above 0 and no instance pointing at the set id itself, with at least 1 variant at 0 uses; one finding per set | `{k} of {n} variants have no instances: {a}, {b}, {c} +{m} more` (3 names) | info | library, strict |
| `duplicate-component-structure` (`duplicate-structure`) | comparable units with equal signatures: inside one set, or across masters (rule 6); not on a master already flagged `duplicate-component-name` | within a set `{n} identical variants: {a}, {b}`; across `Same layers as "{other}" +{m} more` | info | library, strict |

   Category is `Components` for all five; none is fixable. Not in `agent` (an agent that is building components creates unused ones, and the per-call issues block after writes defaults to this preset, `tools/session_options.rs:19-29`); not in `check-designs` (the Figma-parity set; `same_findings_three_callers` would see a second finding on its unused `Component`); not in `recommended`. Severity reasons: unused and dead are info because usage in other files is unknowable (section 2); a name clash is a warning because it makes `apply_spec` answer `AMBIGUOUS_COMPONENT`; an empty set is structurally wrong.
6. **Signature** (P `unitHash`). Per unit, a pre-order walk (not into `Instance` children) streams into a hasher, with no `format!` and no JSON: per node depth, kind, name (nameless at the root, so top names do not matter), rounded width and height; text content; for fills and strokes the paint kind, the solid colour with its opacity or the bound variable id from `variables.bindings["fills/i"|"strokes/i"]`. A unit is *comparable* when it has text, an instance, or any non-root name that is not a default name (`is_default_layer_name` plus P's boolean-op names `boolean*|union|subtract|intersect|exclude`). Deviation from P: a unit that hit a cap is never comparable (P hashes the cap marker, so two long units that differ only after the cap compare equal). A cross-master group whose members share one name after stripping a leading `❌` is skipped (P's tombstone rule). One finding per involved master names another member.
7. **Registry.** Add the 5 `RuleSpec`s (`needs: Geometry`, `fixable: false`) and `LintPreset::Library` (`"library"`) containing exactly them; `Strict` contains them (the existing `strict` test requires it). `all_preset_names()` gains `library`.
8. **Traversal.** `ActiveRules` gets `library` (any of the five on). The `LibraryIndex` is built only then, in the three places `Components` is built; `evaluate_node` calls `index.verdicts(doc, n)` for a visible master and emits through the existing `emit` (so severity overrides, ignores, `include_ignored` and pagination work unchanged). `has-problem('unused-component')` works through `SingleNodeLinter`; `@problems` (agent preset) does not include them.
9. **Report.** When a library rule was enabled, `LintReport` carries `library: { design_system, icons, screens }`, counts of the whole file's local masters, so nothing is skipped silently; absent otherwise.
10. **Check designs.** One new group `Rule::ComponentLibrary`, title "Component library", Studio-defined id `COMPONENT_LIBRARY` for the Rules popover switch (on by default; stored like the others in `disabled_rules`). Rows carry the lint rule id in `Finding.property`; `ignore_key()` for this rule is `IgnoreKey::new(property, node)`, so Ignore in the panel and `include_ignored` in `designos_lint` agree. No Fix; the row click already selects and fits the master. The Prepare phase feeds each node to `LibraryIndex::insert_node` in the existing `Components` loop, then runs `prepare(doc, until)` (units, segmentation, groups) inside the same 2 ms slices; the Walk phase asks `verdicts` at each visible master and counts each icon or screen it visits into `Report.skipped_icons` / `skipped_screens` (scope-limited, unlike the file-wide `LintReport.library`). Messages must not contain ` from library "`.

## 4. Data model / schema changes

None to the document, `.designos` or `.fig`. In memory: `LibraryIndex`, `MasterKind`, `LibraryVerdict`, `LibrarySummary` (new module); `LintPreset::Library`; `ActiveRules.library`; `NodeEvalCtx.library: &LibraryIndex`; `LintReport.library: Option<LibrarySummary>` (`skip_serializing_if` none; copied in `paginate_with_prefix`, `finding.rs:215-227`); `Rule::ComponentLibrary` (`Rule::ALL` 10 to 11); `ScanCursor.library` and `library_done`; `Report.skipped_icons`, `Report.skipped_screens`.

## 5. Contracts

```rust
// designos-doc/src/lint/rules/library/mod.rs
pub enum MasterKind { DesignSystem, Icon, Screen }
pub struct LibrarySummary { pub design_system: usize, pub icons: usize, pub screens: usize }
pub struct LibraryVerdict { pub rule: &'static str, pub message: String }
#[derive(Default)] pub struct LibraryIndex { /* uses, refs, masters, kinds, name groups, signature groups */ }
impl LibraryIndex {
    pub fn insert_node(&mut self, n: &Node);                         // O(1): tally uses, refs, master candidates
    pub fn prepare(&mut self, doc: &Document, until: Instant) -> bool; // resumable; true when finished
    pub fn build(doc: &Document) -> Self;                            // one shot (prepare with no deadline)
    pub fn kind_of(&self, id: NodeId) -> Option<MasterKind>;
    pub fn summary(&self) -> LibrarySummary;
    pub fn verdicts(&self, doc: &Document, master: &Node) -> Vec<LibraryVerdict>; // fixed rule order; empty before `prepare` finished
}
```
MCP `designos_lint`: no new parameter; `preset` accepts `library`; `rules` accepts the 5 ids and aliases; `UNKNOWN_PRESET.valid_options` gains `library` (from `all_preset_names()`); no new error code. The tool text (`engine.rs:1880`) gains one sentence: "preset `library` audits unused, duplicate and dead components; pass every page id in `node_ids` for the whole file". Whole-file call and reply:
```json
{"preset":"library","node_ids":[2,40],"limit":100}
{"findings":[{"rule":"unused-component","category":"components","severity":"info","node_id":4821,"node_name":"Button / Old","message":"No instances in this file","fixable":false}],
 "by_rule":{"unused-component":1},"library":{"design_system":118,"icons":1469,"screens":12},"complete":true}
```
Regenerate: `cargo run -p designos-mcp --bin gen-tool-docs`; `tools_snapshot.rs` and the `rules.rs` drift check must pass. Update `CANONICAL_LINT_RULE_IDS` (`scripts/check_vendor_skills.py:37`).

## 6. File-format mapping

None stored. Ignores persist as today in `check_designs.ignored` (`settings.rs:39-46`), keyed by master node id and lint rule id. `.fig` import supplies `Instance.component` links and set children; nothing new is read or written.

## 7. UI (words; the implementer loads `es-designer` and the Studio design read order of CLAUDE.md section 5 before UI code)

- **Group.** In the Check designs panel a new group "Component library" with the existing count pill (P5), listed last (after "Typography and stroke"). Its rows reuse the panel row: the master's name in body-medium, the message under it (two lines at most, elided), icon `Icon::Component` in `STATUS_INFO_TEXT` (the pair the other component groups use; no new colour), Ignore and Dismiss buttons, no Fix button. Clicking a row selects the master and fits it. Rows keep document order. Copy follows `design/ux-writing.md`: messages are the strings in rule 5, one idea each; group title and switch label are headings, not action labels.
- **Rules popover.** One more checkbox row "Component library" (the popover grows by one row); it switches the whole group.
- **Skipped note.** When the scope holds icons or screens that were not judged, a note row (the `unreadable_library_note` pattern, 28 px, caption, `TEXT_SECONDARY`) at the top of the list reads `Skipped 1,469 icons and 12 screens`, numbers in `Roboto Mono` (as the progress row does), plural from `widgets::plural`, with the tooltip "Component library checks skip icon sets and screen-sized components". It shows whenever the count is above 0 and the group is on, even when the group has no rows (nothing vanishes silently).
- **Empty state.** No masters, or no findings: no group and no header, like every other group; the panel's existing "No issues found" and layer count stay true. A scan in flight shows the existing progress row; the group fills in when the scan ends.
- Desktop captures at 1440 (`target/release/designos --fixture check-designs --panel check-designs --size 1440x900 --screenshot <png>`): the group with an unused master, a dead variant and a duplicate name; the popover with the new row; the skipped note. The fixture (`fixture.rs`) gains the masters for this. Responsive: SKIPPED (owner default). Update the Figma frames from the captures first (owner 2026-10-13).

## 8. Edge cases

1. A variant child is never reported; its set is. A set with one used variant and one unused is `dead-variant`, not `unused-component`.
2. A master used only by an unused master counts as used (no transitive analysis).
3. A master referenced only by an instance-swap default or a preferred list counts as used.
4. A published file (`doc.library.published` set): usage in other files is unknowable; the unused message says so.
5. Components copied from an enabled library are never audited here (the "Library assets" group owns them).
6. A legitimate large layout component of 1000 x 700 or more is classed as a screen and skipped; the note shows the count so it is not hidden.
7. 20 or more vector-only masters with no `/` in their names all become icons (P's accepted trade-off for flat-named libraries).
8. Two masters named `Button` on different pages: both are flagged when both pages are in scope; in a one-page scope only that page's row appears, the fact is still file-wide.
9. Ignoring one member of a duplicate pair hides that row only. Ignoring a `dead-variant` row hides later dead variants of the same set too (no value fingerprint).
10. Rename or delete: the next scan recomputes everything; nothing is cached across scans (the panel drops the index with the cursor; MCP drops it after the call).
11. A document with no masters pays only one branch per node in the existing Prepare pass.

## 9. Non-goals

- P's other detectors: junk-name and token-violation (covered, section 2), deprecated (needs P's `idp/status` plugin data or a name tag Studio does not define), misfiled (needs a configured section taxonomy), redundant-family (heuristic clustering, not in the issue).
- An auto-fix, "delete unused", "merge duplicates" or any destructive action. Findings are read-only; Select is the existing row click.
- Cross-file usage. A "used by N files" count would need a read of every saved project's `library.external`; not built, see open question 5.
- A check of unnamed variant axes (`Property 1`); thresholds as user settings; per-variant ignore; an unused-icons list.
- Changing `default-layer-name`, `hardcoded-value`, `hardcoded-color` or their presets.

## 10. Test plan (red first)

All "fails before": yes, the rules, preset and group do not exist; "guard" marks a test that must stay green. Put the `designos-doc` tests in `lint/rules/library/tests.rs`; hand-built fixtures with the icon and screen cases included.

| Case | Expected | Fails before |
|---|---|---|
| `unused_master_flagged_and_used_set_is_not` | no instance: `unused-component`; one instance of any variant: none | yes |
| `a_swapped_nested_instance_still_counts_as_use` | instance inside an instance copy pointing at B (mirror, `source` differs): B used | yes |
| `swap_default_and_preferred_references_count_as_use` | default `PropValue::Instance`, `InstanceSwap.preferred`, slot `preferred` (set covers variants): used | yes |
| `duplicate_name_is_exact_case_sensitive_and_skips_blank_names_and_variants` | two `Button` flagged; `button` and `Button ` not; blank names and variant names never | yes |
| `empty_set_flags_zero_and_one_variant_only` | 0 and 1 flagged, 2 not | yes |
| `dead_variant_needs_a_used_set_and_caps_the_name_list` | unused set: no `dead-variant`; 5 dead of 8 lists 3 names `+2 more` | yes |
| `duplicate_structure_within_set_and_across_masters` | identical variants, identical masters flagged; root name ignored | yes |
| `duplicate_structure_skips_bare_vector_skeletons_capped_units_tombstone_pairs_and_name_duplicates` | each case yields no structure finding | yes |
| `screen_and_icon_edges` | 999x700 and 1000x699 are ds, 1000x700 is screen; 19 vs 20 same-prefix vector-only masters; a capped unit is not vector-only | yes |
| `icons_and_screens_get_no_finding_and_are_counted` | `summary()` equals the fixture counts, zero findings on them | yes |
| `mirrors_library_page_nodes_and_variant_children_are_not_masters` | none audited | yes |
| `published_file_unused_message_adds_other_files_may_use_it` | suffix present only when `library.published` is set | yes |
| `library_rules_are_registered_in_strict_and_library_only` | `strict` enables all; preset `library` is exactly the 5; `agent`, `check-designs`, `recommended` enable none; aliases resolve both ways | yes |
| `fail_and_clean_<rule>` x5 in `designos-render/tests/lint_rules.rs` | `lint_single_rule` pairs | yes |
| `scope_limits_findings_not_facts` | master on page A, only instance on page B: lint of page A reports no `unused-component`; lint of page B reports nothing for it | yes |
| `ignore_hides_a_library_finding_and_include_ignored_marks_it` | `IgnoreKey::new("unused-component", id)` hides it; `include_ignored` sets `ignored: true` | yes |
| `lint_report_has_library_counts_only_when_a_library_rule_ran` | `library` key present or absent | yes |
| `has_problem_selects_the_master` | `//COMPONENT[has-problem('unused-component')]` returns it | yes |
| `mcp_lint_returns_full_rule_set` extended | five new fixtures; each rule in `by_rule` under `strict` | yes |
| `same_findings_three_callers`, `list_lint_rules_*`, `vendored_skills_checker_fallback_ids_are_the_registry_ids`, `tools_snapshot` | green after the doc/list regeneration | guard |
| `designos_lint_fix_ignores_library_findings` | `fixed_count` unchanged on a library fixture | guard |
| panel: `component_library_group_follows_scope_and_document_order` | page scope lists that page's masters; selection scope lists selected ones | yes |
| panel: `library_row_ignore_key_is_the_lint_rule_id_and_message_has_no_library_badge_marker` | key equals `IgnoreKey::new(id, node)`; `is_from_library()` false for all five messages | yes |
| panel: `switch_off_builds_no_index_and_lists_nothing`, `skipped_counts_cover_the_scope_only` | disabled rule skips `prepare`; counts per scope | yes |
| panel: fixture test `the_fixture_exercises_every_rule_family` | updated count (8 plus the library rows) and unfixable list | yes |
| perf (`#[ignore]`, release): `library_prepare_stays_within_slice_budget` | synthetic 112,474 nodes with 1,600 masters: no `audit_step(2 ms)` over 8 ms; `LibraryIndex::build` time recorded | yes |

Run (with `export PATH="$HOME/.cargo/bin:$PATH"`): `cargo test -p designos-doc library`, `cargo test -p designos-doc lint`, `cargo test -p designos-render --test lint_rules`, `cargo test -p designos-mcp`, `cargo run -p designos-mcp --bin gen-tool-docs`, `python3 scripts/check_vendor_skills.py`, `cargo test -p designos-app check_designs`, `cargo test -p designos-app check_designs --release -- --ignored`, `python3 scripts/zero_fake_ui_gate.py`, `python3 scripts/design_os_self_evolve.py`. Evidence: red/green captures, a terminal capture of the `library` preset reply, the 1440 captures, the recorded timings.

## 11. Performance budgets

- Check designs: the extra Prepare work is one branch per node inside the existing `Components` loop plus `prepare`, sliced by `SLICE_BUDGET` (2 ms); no `audit_step` call may pass 8 ms on the synthetic 112,474-node document with 1,600 masters (same rule as spec #157). Signature work is bounded by 300 nodes per unit; INFERENCE: tens of milliseconds in total across frames; measured and recorded.
- Headless (`designos_lint`): `LibraryIndex::build` runs off the UI thread, like `Components::new` and the token index; ASSUMPTION: at most 60 ms release on the benchmark, to be measured. Under the `strict` preset used for per-call `issues` (opt-in), every mutating call pays the build; recorded in the PR.
- Memory: per unit one `u64` hash, a master index and the variant name; no structure arrays kept.
- Hashing is streamed; no `doc.to_json()`, no per-node `format!` (CLAUDE.md section 4). The panel adds no timer and no repaint request when idle (0.00% CPU after `Done`); the index lives in the cursor and is dropped with it.

## 12. Dependencies and build order

No dependency. Order: (1) the library module and its tests; (2) registry, preset, traversal plumbing, render pairs; (3) MCP tool text, generated docs, the vendor-skills list, the extended `mcp_lint_returns_full_rule_set`; (4) the panel group, fixture, then the captures and the Figma update before the gates (owner 2026-10-13). `traverse.rs` and `check_designs/mod.rs` are hot: rebase before each push. v0.6.1 fixes may move the cited lines.

## 13. Owner rulings that apply

- 2026-10-22: "Cái gì hữu ích thì plan đi spec driven rồi tạo task" (whatever is useful: plan it spec-driven, then create tasks); lint rules through `designos_lint` and a new group in Check designs (controller ruling the same day).
- 2026-10-21: "DESIGN:OS Studio adopts VSF PCP's UX writing standard as-is, enforced by a gate + review." Messages, group title and note follow `design/ux-writing.md`; a reviewer PASS is saved under `plans/reports/`.
- Owner default: responsive checks only on request. 2026-10-13: update Figma from the first verified capture, gates after.
- Releases are verified on macOS. The performance invariants of CLAUDE.md section 4 are non-negotiable. `main` is protected: PR and squash.

## Decisions on the open questions (controller, 2026-10-22)

The owner delegated these calls ("Orchestrate it yourself and answer it yourself", relayed by the release controller on 2026-10-22). Each question below was settled with its recommended answer, so they are binding for the implementer. The reasons are kept so the owner can reverse any of them.


1. **Group title.** Recommended "Component library", because "Library assets" already exists and two groups named "Library" would read as one. Alternative: "Library" (the issue's word).
2. **Panel switch default.** Recommended on, so the group shows for everyone with components; alternative off until a release has used it. Either way the lint rules are opt-in (`library` preset), never in `agent`.
3. **Scope in the panel.** Recommended: the group follows the panel's scope (page or selection), facts stay file-wide, and the MCP call takes every page id for a whole-file audit. Alternative: always whole file, which breaks the scope line the panel shows.
4. **Severities and presets.** Recommended: info for unused, dead variant and duplicate structure; warning for duplicate name and empty set; presets `library` and `strict` only. Alternative: add the two warnings to `recommended`.
5. **Cross-file usage.** Recommended: not now; the message says other files may use a published file's components. A "used by N files" count needs a scan of saved projects; file it as a follow-up if wanted.
6. **Unnamed variant axes (`Property 1`).** P flags them as junk-name; Studio does not check them, and only imported files have them. Recommended: out of #899, one small follow-up rule. Alternative: add a sixth rule here.
