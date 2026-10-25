# #159 Add a read-only component playground in Dev Mode

- Issue: https://github.com/jangtrinh/design-os-studio/issues/159 (task E3-14, epic #16, M / P2)
- Spec base: `main` at `767acf9`.
- Open PR: #501 (`task/159-dev-mode-component-playground`, +755 −22). The release review, head `ba711d1`, found three MAJORs:
  1. a whole-document clone and an export on the UI thread, on every selection and every keystroke;
  2. a nested instance targets the outer instance;
  3. instance-swap codegen comes from the untouched main component.

  The owner ruled on 2026-10-18 that the release session fixes this PR itself.
- Files: `crates/designos-app/src/dev/` (new `playground.rs`, plus `mod.rs` and `sections.rs`), `panels/inspect.rs`, `shell/tests/dev.rs`.

## 1. Summary and user story

As a developer in Dev Mode, I select a component or an instance and try its properties (variant, boolean, text, instance swap) on a scratch copy. I see the preview and the generated code update live, while the design file stays exactly as it was.

## 2. Current state

- FACT: Dev Mode Inspect lists component props read-only through `component_rows` (`dev/data.rs:161-177`). There is no playground.
- FACT: codegen entry is `Language::generate(self, doc, node, react_tailwind)` (`dev/mod.rs:47`).
- FACT: `dev_snapshot(&self, id) -> Option<(Document, Vec<Node>)>` exists (`dev/mod.rs:263`).
- FACT: the subtree renderer `render_subtree_to_rgba_scaled` is used by the PR for the preview.

## 3. Behaviour rules

1. **Target.** The nearest `NodeKind::Instance` found by walking up from the primary selection, the selection included. If there is none, and the selection is a `Component` or `ComponentSet` (or is inside one), the target is that component; for a set, its default variant. Never the outermost instance (MAJOR 2).
2. **Scratch document.** It is built once per target and rebuilt only when the target or the document `rev` changes. It contains only:
   - the target's main component;
   - its `ComponentSet`, when it is a variant, with all variants;
   - every component referenced by its instance-swap props: `preferred` lists, or the current values;
   - nested instance components reachable from those, transitively;
   - images used by those subtrees;
   - one scratch instance of the target component (`create_instance` on the scratch document).

   Never `app.doc.clone()` (MAJOR 1).
3. **Read-only.** Prop edits call `set_instance_prop` on the scratch instance. `app.doc` is never touched: `doc_hash(&app.doc)` and `app.doc.mutation_epoch()` are unchanged, and nothing is pushed to History.
4. **Controls per prop kind.**
   - Variant: dropdown of options.
   - Boolean: switch.
   - Text: single-line input.
   - InstanceSwap: dropdown of the `preferred` components, or, when `preferred` is empty, the components in the scratch document. The full document is never scanned (MAJOR 1).
   - Slot props are not shown, which is honest: the playground cannot author slot content. The section says `Slots aren't editable here` in muted text when the component has slots.
5. **Preview.** Render the scratch instance at scale `s = 168 / max(w, h)`, with no floor and capped at 2.0 for HiDPI, into a 168 px square stage. Re-render only after a prop change or a target change, never per frame. Upload it as a texture outside `ctx.data_mut` (§4.2).
6. **Codegen.** Always generate from the scratch instance (`inst_id`), in the language selected in Dev Mode. This also covers a selected main component (MAJOR 3). Copy-code copies that output.
7. **Reset** restores the scratch instance's values to the target's current values (instance) or defaults (component).
8. **Persistence.** None. Scratch state lives in `DevPlaygroundState` keyed by target `NodeId`, and is dropped when the target changes or the document closes.

## 4. Data model / schema changes

None in the document.

App state:

```rust
pub struct DevPlaygroundState { target: Option<NodeId>, built_rev: u64, scratch: Option<Document>, inst: NodeId, values: BTreeMap<String, PropValue>, preview: Option<egui::TextureHandle>, preview_dirty: bool }
```

## 5. Contracts

```rust
pub fn playground_target(doc: &Document, sel: NodeId) -> Option<NodeId>;
pub fn build_playground_doc(doc: &Document, target: NodeId) -> Option<(Document, NodeId /* scratch instance */)>;
```

No MCP or CLI change.

## 6. File-format mapping

None. RESOLVED.

## 7. UI

- **Placement.** A `Playground` section in the Dev Mode Inspector, under `Component properties`, shown when rule 1 finds a target.
- **Read-only badge.** At the top-left of the stage, a small badge `Read-only` with the Dev Mode emerald `#14AE5C` outline (P2).
- **Stage.** 168 px square on the card surface `#1E1E22`, with 8 px corner radius (P4 concentric radius).
- **Controls.** Rows 28 px high with ≥24 px targets (P3), labels 11 px Inter, values `font_mono` where numeric.
- **Actions.** `Reset` is a secondary button. The code block below updates live.
- **Zero fake UI.** Every control writes the scratch instance. `Reset` is disabled when nothing has changed.
- **Evidence.** A 1440 capture: a Button instance with `showIcon` toggled off, and the code showing `showIcon={false}`.

## 8. Edge cases

1. A Button instance nested in a Card: the target is the Button, the props are the Button's, and the code is the Button's (MAJOR 2).
2. A main Button component: swap Icon and the code shows the new icon (MAJOR 3).
3. A large `.fig` (VSF scale): selecting a component allocates only the scratch subset. No whole-document clone happens (verified by a test on node counts).
4. A 2000×400 component: preview scale 0.084, fully visible, no 0.5 floor.
5. Instance-swap with an empty `preferred` on a big document: the dropdown lists only components already in the scratch document.
6. The document changes underneath (another edit): scratch rebuilt on the next frame, and values reset to the target's.
7. A component with only slot props: the section shows the muted note and no controls.

## 9. Non-goals

- Applying playground values to the document, such as "Apply to instance". That would break the read-only AC.
- Editing slot content.
- Saving presets.

## 10. Test plan

Seam: `crates/designos-app/src/shell/tests/dev.rs`.

| Test | Expected | Fails before |
|---|---|---|
| `dev_mode_component_playground_toggles_props_without_mutating_doc_and_updates_codegen_live` (PR) | `doc_hash` and `mutation_epoch` equal before and after; code contains `showIcon={false}` and the edited label | yes |
| `playground_targets_nearest_nested_instance` | Card > Button; toggling `showIcon` changes the Button code | yes on PR head |
| `playground_swap_on_main_component_updates_code` | icon name in code changes | yes on PR head |
| `playground_scratch_doc_contains_only_the_component_closure` | 10k-node doc; `scratch.nodes.len()` ≤ component closure + 1 instance subtree | yes on PR head |
| `playground_preview_scale_has_no_half_floor` | 2000×400 gives 0.084 | yes on PR head |
| `reset_restores_values_and_disables_itself` | | yes |

## 11. Performance budgets

- `CLAUDE.md` §4.1: never `to_json` or `doc_hash` per frame. The test reads `doc_hash` only in the assertion.
- §4.2: `ctx.load_texture` is never called inside `ctx.data_mut`.
- No full-document clone or scan on the UI thread. The scratch build is O(component closure).
- The preview renders only on change, so 0% idle CPU holds (§4.4 idle rule).

## 12. Dependencies and build order

- No dependency.
- It can land first. It shares `dev/` with #152. Merge #152 before #159 or rebase, whichever opens first.

## 13. Owner rulings that apply

- D-E3-2 (`owner-decisions-and-cross-check.md` round 7, item 26): the component playground is built (E3-14).
- Owner default: 1440 capture only.
- Owner ruling of 2026-10-18: the release session fixes PR #501.
