# #52 Check designs: audit text and overrides inside instances

- Issue: https://github.com/jangtrinh/design-os-studio/issues/52 (follow-up M4 of PR #45, epic #16)
- Spec base: `main` at `767acf9`. PR #45 (Check designs) is merged.
- Open PR: #577 (`task/52-audit-text-and-overrides-inside-inst`, +246 −12). No release review yet at spec time.
- Files: `crates/designos-app/src/check_designs/mod.rs`, `hardcoded.rs`, `contrast.rs`, `contrast/backdrop.rs`, `fix.rs`, `tests/instances.rs` (new); `crates/designos-doc/src/components.rs`.

## 1. Summary and user story

As a designer running Check designs on a screen built from components, I want to see problems that an instance introduces:
- overridden text colour that fails contrast;
- a hard-coded override on a property the main component bound to a variable.

Fixing them must write an override on that instance only, never on the main component.

## 2. Current state

- FACT: `audit` (`check_designs/mod.rs:112-138`) skips every node where `n.component.source.is_some()` (`mod.rs:124`). Every layer inside an instance is therefore unaudited.
- FACT: `designos_doc::resolved_node(doc, id)` (`variables_apply.rs:300`) returns the node with bound variable values applied.
- FACT: `record_override(before, after)` (`components.rs:297-345`) derives `OverrideField`s from node diffs:
  - fills, stroke, effects, text and style refs are included;
  - `variables.bindings` changes are not detected.

  So binding a variable on an instance node, which is the Bind fix, would not survive the next sync.
- FACT: `record_overrides` in the app (`comp_ops.rs:59`) applies `record_override` to commands on instance nodes.
- FACT: contrast thresholds are AA 4.5:1, or 3:1 for large text, where large means ≥ 24 px, or ≥ 18.66 px with weight ≥ 700 (`contrast.rs:55-62`).

## 3. Behaviour rules

1. **Descend into instances.** `audit` visits nodes inside instances: those with `component.source` set and whose instance root is visible. Hidden layers are still skipped.
2. **Report only divergence.** For a node `i` inside an instance, with main counterpart `m = i.component.source`:
   - (a) **Hard-coded value.** Report only if the paint, text style or effect list of `resolved_node(i)` differs from `resolved_node(m)`, or if `m` binds the property (`m.variables.bindings` has the key) and `i` does not. A value equal to the main component's is reported once, on the main component (when the main is in scope), and never on instances.
   - (b) **Contrast.** Report only if the text colour of `i`, or its computed backdrop, differs from the main's, and the ratio fails. If both text and backdrop equal the main's, nothing is reported for `i`.
3. **Backdrop inside an instance.** The ancestor walk goes from `i` through its instance ancestors, then the instance root's own ancestors on the page, using the same `contrast/backdrop.rs` rules, owner ruling 1A included.
4. **Fixes write overrides.**
   - Every fix on a node inside an instance mutates that node, and records the override field through `record_override`:
     - `SetTextColor` → `TextStyle`;
     - `BindVariable` on `fills/N` → `Fills`;
     - on `strokes/N` → `Stroke`;
     - `ApplyStyle` → per style kind.
   - `record_override` additionally reports the field when `variables.bindings` differs for that field's keys:
     - `fills/*` → `Fills`;
     - `strokes/*` → `Stroke`;
     - text keys (`fontSize`, `lineHeight`, `letterSpacing`, `fontWeight`) → `TextStyle`;
     - `characters` → `Characters`;
     - `topLeftRadius` and the other corners → `CornerRadii`;
     - `opacity` → `Opacity`;
     - `width` and `height` → `Size`;
     - padding and spacing keys → `Layout`;
     - `strokeWeight*` → `Stroke`;
     - `visible` → `Visible`.
   - The main component and other instances are never touched.
   - One undo step per fix, and one for Fix all.
5. **No safe override path → no fix.**
   - `Reconnect` is never offered inside an instance.
   - A text node inside a nested instance whose main text has a `SlotContent` binding (slot content) is treated as an ordinary node.
   - Findings with no fix are listed with no `Fix` button.
6. **Row label.** Findings inside instances show the layer name plus `· in <instance name>`, so the user can locate them.

## 4. Data model / schema changes

None. `record_override` gains binding detection (a behaviour change in `designos-doc`).

## 5. Contracts

- `pub fn record_override(before: &Node, after: &Node) -> BTreeSet<OverrideField>` keeps its signature and detects more (rule 4).
- `check_designs::Finding` gains `pub in_instance: Option<NodeId>`, the instance root, used for the row label.

## 6. File-format mapping

None. RESOLVED.

## 7. UI

- **Panel.** The Check designs panel (`panels/check_designs.rs`) rows look the same, plus the muted ` · in <instance>` suffix (theme muted text).
- **Evidence.** A 1440 capture of the panel before and after, on a fixture with an overridden instance.

## 8. Edge cases

1. Main Button: label `#FFFFFF` on fill `#0D99FF`. It fails AA (ratio 2.99:1 < 4.5), so it is reported on the main only. Its instance with the same colours: no finding (rule 2b).
2. An instance overrides the label colour to `#BBBBBB` on a white card backdrop. One finding, on the instance's label. The fix recolours only that instance, with `OverrideField::TextStyle`, and other instances are unchanged.
3. Main fill bound to `color/primary`; an instance overrides the fill to raw `#0D99FF`, equal to that variable's value. Hard-coded finding with Bind, applied as an override (`OverrideField::Fills`). A sync afterwards keeps the binding.
4. An instance overrides the fill to a value matching no variable: no finding.
5. A hidden instance, or a hidden layer inside an instance: skipped.
6. Instances nested 3 deep: the backdrop walk crosses all instance roots.
7. Fix all with one main finding and two instance findings: one undo step reverts all three.

## 9. Non-goals

- Number variables (#154).
- Library suggestions (#155).
- Whole-page performance (#157). The descent in rule 1 increases scanned layers; #157 removes the cap.

## 10. Test plan

Seam: `crates/designos-app/src/check_designs/tests/instances.rs`.

| Test | Fixture literal | Expected | Fails before |
|---|---|---|---|
| `text_override_failing_aa_inside_instance_is_listed_and_fix_recolours_only_that_instance` | main label `#111111` on white; instance A overrides the label to `#BBBBBB`; instance B untouched | 1 finding on A's label; after Fix: A's label passes AA, B's label is `#111111`, the main is `#111111`; one undo restores | yes |
| `hard_coded_override_on_bound_property_lists_bind_fix_applied_as_override` | main fill bound to `color/primary` (`#0D99FF`); instance fill raw `#0D99FF` | Bind finding; after Fix: the instance has the binding and `Fills` in overrides; `sync_instances` keeps it | yes |
| `layers_matching_main_component_produce_no_finding` | instance identical to the main | 0 findings from the instance subtree | yes (guard against duplicates) |
| `record_override_detects_variable_binding_changes` (designos-doc) | before: no binding; after: `fills/0` bound | contains `Fills` | yes |

## 11. Performance budgets

- `CLAUDE.md` §4.1: the audit runs on `rev` change, as today (`state.rs` `refresh_check_designs`).
- Descending into instances raises the scan count. The `MAX_SCANNED_LAYERS = 20_000` cap (`mod.rs:30`) still applies until #157 lands, and the panel's truncated note stays honest.

## 12. Dependencies and build order

- Depends on #35 (merged as PR #45).
- Run before #157, which changes the scan loop.
- Shares `contrast/*` with #65: land #65 first, or rebase.

## 13. Owner rulings that apply

- Owner ruling for PR #45 (2026-10-18), **1A**: the contrast backdrop is skip-only. Text is skipped when a painted descendant of an earlier sibling, or an ancestor's stroke band, lies under it. A covering child is never composited.
- Owner ruling **2B** (2026-10-18) governs text-fill binding inside fixes; see spec 68.
- Owner default: 1440 capture only.
