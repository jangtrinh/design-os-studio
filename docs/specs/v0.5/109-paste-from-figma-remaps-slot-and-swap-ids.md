# #109 Paste from Figma: remap slot preferred, InstanceSwap preferred and PropValue::Slot ids

- Issue: https://github.com/jangtrinh/design-os-studio/issues/109 (audit E3-3, epic #16, S / P1)
- Spec base: `main` at `767acf9`.
- Open PR: #494 (`task/109-clipboard-remap-slot-and-swap-ids`, +182 −4, one file). No release review yet.
- Files: `crates/designos-app/src/clipboard.rs` only.

## 1. Summary and user story

As a designer pasting a Figma selection that contains components with slot or instance-swap properties, the pasted properties must refer to the pasted components. They must not refer to unrelated layers of my document that happen to have the same numeric ids.

## 2. Current state

- FACT: `place_figma` (`clipboard.rs:107`) imports the clipboard payload into a scratch document, then copies it into the open document through `instantiate_mapped` (`clipboard.rs:261-290`). That function returns the scratch-id → document-id `map`.
- FACT: `detach_foreign_refs` (`clipboard.rs:177-186`) remaps through `map` only:
  - `NodeKind::Instance::component`;
  - `component.source`;
  - `prototype.fixed_children`.

  It clears `style_refs`, `variables` and `prototype.reactions` (assumption A11 in the brainstorm: styles and variables are not merged).
- FACT, not remapped, so the scratch ids survive verbatim into the document and alias unrelated nodes:
  - `component.slot_configs[*].preferred: Vec<NodeId>` (`node.rs:245-246`);
  - `component.props[*].kind = PropKind::InstanceSwap { preferred }` (`node.rs:209`);
  - `component.props[*].default` when it is `PropValue::Instance(id)` or `PropValue::Slot(ids)`;
  - `component.values[*]` when it is `PropValue::Instance(id)` or `PropValue::Slot(ids)` (`node.rs:197-201`).

## 3. Behaviour rules

1. For every pasted node, every id in the five places listed in §2 is replaced by `map[id]` when present.
2. An id that is not in `map` (its target was not pasted, for example a component kept only on the scratch document's internal canvas) is **dropped**:
   - from a `Vec`, the entry is removed and order is preserved;
   - for a `PropValue::Instance(id)` with no mapping, the value is reset to the property's default, if that default maps;
   - otherwise the entry is removed from `values`. For a def default, it becomes `PropValue::Instance(NodeId(0))`, the importer's existing "unset" convention (`rest_import.rs:1194`).
3. The number of dropped references is returned from `detach_foreign_refs` and summed over the paste.
4. When the sum is > 0, `place_figma` shows one informational toast: `"Pasted. {n} component reference(s) pointed outside the selection and were removed."` (`show_toast(text, false)`, `app.rs:273`).
5. The paste is still one undo step.
6. Native Studio-to-Studio copy and paste (`instantiate`) is not changed by this task. Those ids refer to the same document, where they stay valid (`clipboard.rs:280-283` keeps connector ends the same way).

## 4. Data model / schema changes

None.

## 5. Contracts

- `fn detach_foreign_refs(n: &mut Node, map: &HashMap<NodeId, NodeId>) -> usize`. It returns the count of dropped refs.
- No MCP or CLI change.

## 6. File-format mapping

None. Paste uses the clipboard payload: HTML with `(figmeta)` / `(figma)` markers wrapping a fig-kiwi archive (`rea-findings.md` §Clipboard). Ids are scratch-document `NodeId`s by the time this code runs. RESOLVED.

## 7. UI

One info toast, using the existing toast surface. No new control.

## 8. Edge cases

1. Paste a component and an instance of it, where the instance's swap value is the component: the value remaps to the pasted component.
2. Paste only an instance whose slot prefers components that are not pasted: `preferred` becomes `[]`, and the toast reports n.
3. Paste where the scratch id `2` exists in the document as an unrelated node: after paste, no slot or swap list contains the document's `2` (the alias bug).
4. Paste with zero component props: n = 0 and no toast.
5. A duplicate id inside `preferred` that maps: both entries are kept, in order.
6. Undo after paste: the document is identical to the state before the paste.

## 9. Non-goals

- Merging styles, variables or library references from the payload (A11).
- Remapping publish keys (`unresolved_preferred`, #107): these are keys, not ids.

## 10. Test plan

Seam: `crates/designos-app/src/clipboard.rs` `#[cfg(test)]`.

- `figma_paste_remaps_slot_and_swap_ids_and_drops_and_reports_unknown_ids` (from PR #494). Build a scratch document with:
  - component `C` (scratch id 2);
  - component `D` (scratch id 7), not in the pasted roots;
  - an instance with `slot_configs["Body"].preferred = [2, 7]`, `InstanceSwap { preferred: [2, 7] }`, `values["Body"] = Slot([2])` and `values["Icon"] = Instance(7)`.

  Paste roots `C` and the instance into a document that already holds node id 2. Assert:
  - `preferred == [map[2]]` in both places;
  - `Slot([map[2]])`;
  - `Icon` reset or removed per rule 2;
  - the returned dropped count is 3;
  - one undo restores the document.
  - Fails before: yes (observed in PR #494: `left: [NodeId(2), NodeId(7)] right: [NodeId(18)]`).
- `figma_paste_without_component_props_shows_no_toast`.
- Gate: `cargo clippy --workspace --all-targets --locked -- -D warnings`. PR #494 ran with `-A dead_code`, which the executor contract forbids. Re-run without it.

## 11. Performance budgets

No `CLAUDE.md` §4 invariant applies. Each id lookup is O(1).

## 12. Dependencies and build order

- Depends on #31 (merged).
- Runs after #112, which edits `instantiate_mapped` in the same file.
- Independent of the fig queue.

## 13. Owner rulings that apply

- Brainstorm A11 (paste: styles, variables and library refs are not merged; resolved values stay) and owner P0 "Figma clipboard paste interop" (brainstorm, owner decisions 2026-10-17).
- The executor contract: no `#[allow]` and no `-A` flags in gates.
