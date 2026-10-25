# #32 Slots: authoring UI (create slot, slot settings)

- Issue: https://github.com/jangtrinh/design-os-studio/issues/32 (epic #16)
- Spec base: `main` at `767acf9`.
- Open PR: #502 (`task/32-slots-authoring-ui`). Release reviews:
  - Round 1: two MAJORs. The first (fixed toggle ids) is fixed in `2ded1bb`. The second (`displayByDefault` inverted) was rejected with evidence: Figma Plugin API `SlotSettings.displayEmptyByDefault` maps one to one onto kiwi `displayByDefault`.
  - Round 2, head `66a67d8`: one MAJOR. The preferred-instance multi-select shows only the first item and toggles by label.
  - The owner ruled on 2026-10-18 that the release session fixes this PR itself.
- Files: `crates/designos-doc/src/components.rs`, `node.rs`, `slot_model_tests.rs`; `crates/designos-app/src/comp_ops.rs`, `panels/components.rs`, `panels/context.rs`, `panels/menus.rs`, `widgets/menu.rs`, `shell/tests/components.rs`.

## 1. Summary and user story

As a design-system author, I select a frame inside a main component and choose **Convert to slot**. I then set the slot's limits and preferred instances in a **Slot settings** block, so consumers of the component can fill that region with the right content.

## 2. Current state

- FACT: the slot model exists: `PropKind::Slot` (`node.rs:210`), `SlotPropConfig` (`node.rs:225-247`) and `ComponentLink.is_slot / slot_configs` (`node.rs:261-266`). No command authors them. The only writers are the `.fig` and REST importers (`rest_import.rs:1019`, `:1197-1210`).
- FACT: `add_prop` (`components.rs:995`), `rename_prop` (`:1011`) and `delete_prop` (`:1050`) exist. `rename_prop` and `delete_prop` move or drop `slot_configs` since PR #49 (H1 fix).
- FACT: `is_slot_container` (`components.rs:208-232`) treats a node as a slot when it has an `OverrideField::SlotContent` override or binding, or a binding to a `PropKind::Slot` def. It does not read `is_slot` (stated in PR #49 "Not changed on purpose").
- FACT: the canvas draws slot chrome via `fast_is_slot_container` (`canvas/draw.rs:30-37`).

## 3. Behaviour rules

1. **Availability of Convert to slot.** Enabled when exactly one node is selected, and that node:
   - is a `Frame` (not a Component, Instance, Group or Section);
   - has an enclosing `Component` (or a variant inside a `ComponentSet`) as an ancestor, without passing through an `Instance`;
   - is not already a slot.

   Otherwise the menu item is disabled, with the tooltip `"Select a frame inside a main component"`. No fake enabled state.
2. **Convert to slot is one undo step** (`Tx::commit` with one `Command::Batch`). It does all of the following:
   - (a) sets `is_slot = true` on the frame;
   - (b) adds to the prop owner (`prop_owner`, `comp_ops.rs:411`: the set for a variant, else the component) a def `ComponentPropDef { name, kind: PropKind::Slot, default: PropValue::Slot(vec![]) }`. `name` = the frame's name, made unique among the owner's props by appending ` 2`, ` 3`, …;
   - (c) binds `frame.component.bindings[OverrideField::SlotContent] = name`;
   - (d) inserts `owner.component.slot_configs[name] = SlotPropConfig::default()` (every field `None`);
   - (e) runs `sync_instances` so instance mirrors get `is_slot` (`apply_reference`, `components.rs:753`).

   The frame's existing children stay, and become the slot's default content.
3. **Slot settings block** (shown when the selection is a slot frame in a main component, or the owner with slot props; one block per slot prop). Each control writes `SlotPropConfig` through `set_slot_config(doc, owner, name, cfg) -> Command`, one undo step per change.

   | Label | Field | Control |
   |---|---|---|
   | `Minimum layers` | `min_children` | numeric field |
   | `Maximum layers` | `max_children` | numeric field |
   | `Only allow preferred instances` | `allow_preferred_values_only` | checkbox |
   | `Preferred instances` | `preferred` | multi-select of components |
   | `By default, fill items on slot's counter axis` | `stretch_child_on_insert` | checkbox |
   | `By default, display empty slot` | `display_by_default` | checkbox |

   - For the numeric fields, an empty value means `None` (Figma's placeholder text). Values are clamped to `0..=999`. If min > max, the edited field wins and the other is set equal.
   - The labels are Figma UI3's exact English strings (RESOLVED, §6 Q32-1).
4. **Toggle identity.** Every control's egui id and recorded name include the slot property name (`slot-preferred-only:<name>` and so on). Two slot props never share a widget state (round 1 MAJOR 1).
5. **Preferred instances multi-select** (round 2 MAJOR):
   - each row's check mark is `preferred.contains(&component_id)`, keyed by `NodeId`, never by label;
   - clicking a checked row removes that id, and clicking an unchecked row adds it;
   - the closed field shows `"None"`, the single component name, or `"{n} components"`;
   - two components with the same label are two rows that toggle independently.
6. **Persistence.** Settings live in `slot_configs`, which is already serialized (`node.rs:261-263`) and exported to `.fig` (`fig_export.rs:1018-1029`).
7. **`is_slot_container` reads `is_slot`.** A node with `component.is_slot == true`, or whose `source` has it, is a slot container. Without this, an authored slot whose binding is missing would not draw chrome.

## 4. Data model / schema changes

None. Every field exists. Doc comment fix: `SlotPropConfig.display_by_default` (`node.rs:229-231`) should read: "`displayByDefault` (Plugin API `displayEmptyByDefault`): the slot's empty highlight shows on empty instances."

## 5. Contracts

- `pub fn convert_to_slot(doc: &mut Document, frame: NodeId) -> Result<Command, SlotAuthorError>`
- `pub fn set_slot_config(doc: &mut Document, owner: NodeId, name: &str, cfg: SlotPropConfig) -> Command`
- `pub enum SlotAuthorError { NotAFrame, NotInMainComponent, InsideInstance, AlreadySlot, Missing }`. The app shows `"Can't convert: <reason>"` as an error toast if it is ever reached; the menu gating should prevent it.
- `App::convert_selection_to_slot(&mut self) -> bool` in `comp_ops.rs`.
- MCP: none in this task. A follow-up can expose `convert_to_slot`.

## 6. File-format mapping (RESOLVED)

- `isSlot` = 463 on the frame `NodeChange`.
- The def is `ComponentPropDef.type = SLOT (7)`, with `slotPropConfig` (12) = `{stretchChildOnInsert=1, displayByDefault=2, minChildren=3, maxChildren=4, allowPreferredValuesOnly=5}`.
- The binding is `ComponentPropRef.componentPropNodeField = SLOT_CONTENT_ID (4)`.
- Figma Plugin API `SlotSettings` (developers.figma.com, read 2026-10-18):
  - `stretchChildOnInsert`: "when a layer is inserted into the slot, apply counter-axis fill";
  - `displayEmptyByDefault`: "whether the slot highlight is always shown on empty instances of this slot";
  - `minChildren`, `maxChildren` (`null` = not set; min ≤ max);
  - `allowPreferredValuesOnly`: "restrict the slot to its `preferredValues` list".
- Editor command ids from the Figma 126.9.13 bundle: `CONVERT_TO_SLOT`, `GROUP_SELECTION_INTO_SLOT`, `RESET_SLOT_ASSIGNMENT`; predicates `canConvertSelectedNodesToSlot`, `canGroupSelectedNodesIntoSlot`, `canConvertSelectedSlotsToFrame`.

RESOLVED, Q32-1 (REA 2026-10-18). The exact Figma UI3 English labels are in the Facts section below. The table in rule 3 uses them.

### Facts (REA 2026-10-18)

Method. Read-only. The English string bundle is in the Figma desktop HTTP cache, `~/Library/Application Support/Figma/DesktopProfile/v43/Cache/Cache_Data`, as `webpack-artifacts/assets/figma_app-<hash>.min.en.json.br` (brotli). The earlier search had looked at the V8 Code Cache, not the HTTP cache. Six cached builds were checked (`f480317378e0928d`, `2418ff423c6da2e0`, `f723190d9cd1a994`, `a8572f133f7bf411`, `cdc5177f8cc111b9`, `d868b4732e38dc67`), and all carry the same strings. `rea` 6.7.0 (`npx -y rea-agents analyze-javascript-application`, evidence `ev_2624766…`) indexed the settings chunk `9905-03972f70349cc010.min.js.br`. Field wiring was read from that chunk.

| Figma key (prefix `design_systems.component_properties.`) | English string | Bound to |
|---|---|---|
| `slot_restrictions.popover_title` | `Slot limits` | popover title |
| `slot_restrictions.content_section` | `Settings` | section heading |
| `slot_restrictions.min_layers` | `Minimum layers` | `minChildren` |
| `slot_restrictions.max_layers` | `Maximum layers` | `maxChildren` |
| `slot_restrictions.no_limit` | `None` | empty numeric field |
| `slot_restrictions.min_exceeds_max_error` | `Minimum can’t exceed maximum.` | error tooltip |
| `slot_restrictions.preferred_only` | `Only allow preferred instances` | `allowPreferredValuesOnly` (recording key `preferredOnly`) |
| `slot_restrictions.display_by_default` | `By default, display empty slot` | `displayByDefault` |
| `slot_fill_config.counter_axis_label` | `By default, fill items on slot's counter axis` | `stretchChildOnInsert` |
| `slot_fill_config.disabled_tooltip` | `Slot must have auto layout` | fill checkbox disabled |
| `slot_fill_config.info_tooltip` | `If a slot follows a horizontal flow (x-axis), items will stretch along its height (y-axis).` | info icon |
| `preferred_instances` | `Preferred instances` | preferred list |
| `select_preferred_values` | `Select preferred values` | picker |
| `slot_restrictions.pill_label` | `{count} limit(s)` (ICU plural) | summary pill |
| `slot_restrictions.row_min` / `row_max` / `row_preferred_only` | `{n} layer minimum` / `{n} layer maximum` / `Preferred instances only` | summary rows |

Instance banners: `This slot has a {minChildren}-layer minimum.`, `This slot has a {maxChildren}-layer maximum.`, and `Only preferred instances can be added. Click the slot to view options.`

Action labels (`fullscreen_actions.*`):
- `convert-to-slot` → `Convert to slot`
- `group-selection-into-slot` → `Wrap in new slot`
- `convert-slot-to-frame` → `Convert slot to frame`
- `reset-slot-assignment` → `Reset slot`

Panel labels:
- `fullscreen.properties_panel.section_slot.edit_slot_property` → `Edit slot property`
- `fullscreen.properties_panel.section_slot.disabled_convert_to_slot` → `Slot cannot be applied to frame with grid layout`

Evidence snippet (settings chunk, restated): the `slot_restrictions.display_by_default` checkbox is rendered by a component whose prop is `displayByDefault`. The `slot_fill_config.counter_axis_label` checkbox is disabled when the slot has no auto layout.

Kiwi cross-check: the cached fullscreen wasm schema (`c36343e1…`) has `SlotPropConfig { stretchChildOnInsert = 1; displayByDefault = 2; minChildren = 3; maxChildren = 4; allowPreferredValuesOnly = 5; }`, identical to the mapping above.

Parity notes for the spec owner (rules unchanged here):
- Figma shows `Minimum can’t exceed maximum.` as an error instead of auto-equalising the other field (rule 3).
- Figma's fill checkbox is disabled when the slot has no auto layout.
- The `Wrap in new slot` command exists and is out of this task's scope.

## 7. UI

- **Entry points.** Right-click menu on a frame inside a main component → `Convert to slot`. The same action appears in the Design panel's `Component properties` section header menu. It is disabled with a tooltip when rule 1 fails.
- **Slot settings block.** It sits in the right panel under `Component properties`, one titled group per slot property (`Slot · <name>`). Rows are 28 px high with ≥24 px hit targets (P3).
  - Numeric fields use `theme::font_mono()` (P5).
  - Checkboxes use the existing checkbox widget.
  - The preferred multi-select reuses the instance-swap dropdown widget with check marks.
- **Tokens and copy.** Colours come from `theme.rs` only (P1). The component accent is cyan `#06B6D4` for the slot group icon (P2). No purple.
- **Zero fake UI (P6).** Every row writes a command. A disabled state is real and has a reason.
- **Evidence.** Desktop capture at 1440 of the Slot settings block, with two slot props. Responsive captures are skipped (owner default).

## 8. Edge cases

1. Converting a frame named `Body` when the owner already has a prop `Body`: the new prop is `Body 2`.
2. Converting inside a variant: the def goes on the `ComponentSet`. The other variants get no binding, and the slot exists only in that variant (Figma behaves the same way: slot props are per set, regions per variant).
3. Two slot props on one component: the toggles are independent (rule 4).
4. Min 3, then Max 2: max wins and min becomes 2.
5. Undo after Convert: `is_slot`, the def, the binding and the config are all gone in one step.
6. Deleting the slot prop afterwards: `delete_prop` drops the config (PR #49 H1). The frame keeps `is_slot = true`, and its binding points at a missing prop. Rule: `delete_prop` also clears `is_slot` and the `SlotContent` binding on frames bound to that name, in the same command.
7. Preferred list containing a deleted component: the row is not shown, and the id stays until garbage collection (`prune_unreachable` keeps preferred components alive, per PR #49).

## 9. Non-goals

- Editing slot content in instances (#34).
- The empty placeholder, the `+` picker and stretch behaviour (#150).
- Group selection into slot (`GROUP_SELECTION_INTO_SLOT`). File it as a follow-up if wanted.
- Convert slot back to frame.

## 10. Test plan

Seams: `crates/designos-doc/src/slot_model_tests.rs` and `crates/designos-app/src/shell/tests/components.rs`.

| Test | Expected | Fails before |
|---|---|---|
| `convert_to_slot_sets_flag_def_binding_config_in_one_undo_step` | after commit: `is_slot`, def `Body: Slot`, binding, config default; after one undo: all absent | yes |
| `convert_to_slot_is_disabled_outside_a_main_component` | `Err(NotInMainComponent)`; menu item disabled | yes |
| `slot_settings_persist_through_native_json_and_fig` | set min 1, max 3, preferred-only, stretch, show-empty; `to_json`/`from_json` and `export_fig`/`import_fig` equal | yes |
| `slot_toggles_of_a_second_slot_change_only_that_slot` | from PR #502 fix | yes |
| `preferred_multi_select_checks_by_id_and_toggles_each_of_two_preferred` | add Icon then Avatar: field `2 components`; reopen: both checked; click Avatar: only Icon remains | yes on PR head `66a67d8` |
| `same_label_components_toggle_independently` | two components named `Icon` | yes |
| `deleting_a_slot_prop_clears_the_frame_slot_flag_and_binding` | rule from edge case 6 | yes |

Visual: 1440 capture.

## 11. Performance budgets

`CLAUDE.md` §4.1 applies: the settings panel must not call `doc.to_json` or `doc_hash` per frame. Panel rendering reads the config. Edits go through `Tx` and bump `TileView::rev` once per commit, never on hover.

## 12. Dependencies and build order

- Depends on #31 (merged) and #112 (copy semantics).
- Blocks #34, #150 and #151.
- Shares `panels/components.rs` and `comp_ops.rs` with #34, #150 and #151. Merge #32 first, then rebase the others.

## 13. Owner rulings that apply

- Brainstorm "Assumptions … chosen conservatively = Figma UI3 1:1": labels follow Figma.
- Owner default: responsive checks skipped. Desktop 1440 capture only.
- Owner ruling of 2026-10-18: the release session fixes PR #502.
- Design principles P1–P6 (`design/principles.md`).
