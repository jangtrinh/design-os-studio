# #1030 Text styles ignore the layer's typography variable bindings

| Field | Value |
|---|---|
| Issue | #1030 (bug, P2). Controller ruling v0.6.2 #9 is in the issue body |
| Still real? | Yes, on main (evidence below). Contributor **draft PR #1677** (NamAIEngineer, head `f1df59179`, created 2026-11-06) targets it. It is in this epic-wave's "P0 verify wave A" Grok review (`plans/261106-2245-epic-waves/controller-ledger.md:18`) |
| Size | S: about 2 h to review and carry; about 1 h fix-up if the review finds a gap |
| Tier | Review: Grok 4.7 read-only, already queued, then a controller check. Fix-up, if needed: Argon, or the contributor via a PR comment. Carry with credit: the contributor stays the commit author (#1424/#1515/#1616 precedent) |
| Depends on | None. It must merge **before** #1448 (same files: `styles.rs`, `text_style_ops.rs`) |
| Ready after owner answers? | Yes. No owner question |

Code baseline: `origin/main@d22dad970`.

## 1. Outcome

- Applying a text style to a layer whose font size is bound to a variable removes that binding, so the canvas shows the style's size. The style stays checked, and there is one undo step.
- Creating a text style from a layer that shows a bound size of 32 (stored 18) makes a style whose size is bound to the same variable. The style also resolves to 32 in that mode.
- Binding a variable on a style while one of its linked layers is being typed into gives two clean undo steps.

## 2. Re-anchor (origin/main)

- FACT: create copies stored values and empty bindings: `create_text_style_from_nodes` builds the style from `shared_text_style` (raw `TextProps`, `crates/designos-doc/src/text_style_ops.rs:35-45`), and `bindings: Default::default()` (`:77`).
- FACT: direct node bindings win over style bindings in `resolve_node` (`crates/designos-doc/src/variables_apply.rs:577-579`), so a kept node binding hides the applied style.
- FACT: the style model *can* hold typography bindings: `style_property_kind` allows `fontSize|lineHeight|letterSpacing|paragraphSpacing|paragraphIndent|fontWeight|fontFamily|fontStyle` on `Style::Text` (`crates/designos-doc/src/styles_ops.rs:41-52`). The ruling's fallback ("drawn values + toast when the model cannot hold them") is therefore **moot**. Create carries the bindings.
- FACT: `bind_style_variable` (`crates/designos-app/src/styles.rs:339-356`) calls `scrub_end` but not `end_text_edit_of`. `update_style_values` does call it (`styles.rs:217`). So the undo hazard named in the 2026-10-23 comment is still on main.

## 3. Scope and rules (the review checklist for #1677)

1. **Apply.** `apply_style` / `link` for a text style removes the node's bindings on the keys the style owns, and only those keys: the eight keys of `style_property_kind` Text, plus `listSpacing` if it is owned. It does this in the same command. Fill, opacity, size and other bindings stay.
2. **Create.**
   - The style takes each layer's *resolved* typography values, read through `resolve_node`, so the drawn value is what gets saved.
   - When every text layer binds the same variable on a key, the style gets that binding (`StyleMeta.bindings`), and the layers' own bindings on that key are removed (the style now carries them).
   - Layers whose bindings differ on an owned key: refuse with the same error as differing values (`StyleOpError::NothingToStyle`). Controller decision, recorded: this is consistent with `shared_text_style` refusing differing values (`text_style_ops.rs:44`). The PR does this ("reject incompatible maps").
3. **Undo.** `bind_style_variable` and the library rebind path (`crates/designos-app/src/libraries/mod.rs:740`) end the open text session of every linked layer first, as `update_style_values` does.
4. **Shared layer.** The fix belongs in `designos-doc::text_style_ops::create_text_style_from_nodes` (#1028), not in the app. FACT: today only the app calls it (`crates/designos-app/src/styles.rs:258`); `designos-mcp` has no caller (grep on origin/main). A future MCP create path will pick up the same behaviour.

## 4. Files (from PR #1677)

`crates/designos-doc/src/text_style_ops.rs`, `variables_apply.rs`; `crates/designos-app/src/styles.rs`, `styles/variable_binding_tests.rs`, `libraries/mod.rs`, `check_designs/fix.rs`.

## 5. Acceptance criteria (each must fail with the fix reverted)

- **AC-1** Layer with `fontSize` stored 18, bound to `size` (Dark = 32). Apply a style with size 60: the node has no `fontSize` binding, it renders at 60, the style is checked, and one undo restores both the binding and the 32 render.
- **AC-2** Apply leaves a `fills/0` binding and an `opacity` binding on the same node untouched.
- **AC-3** Create from that layer in Dark mode: the style's `fontSize` binding is `size`, and its resolved size is 32. The layer stays linked and still renders 32.
- **AC-4** Create from two layers that bind `fontSize` to different variables is refused, and the document is unchanged.
- **AC-5** Doc level: calling `designos_doc::text_style_ops::create_text_style_from_nodes` directly on a bound layer carries the binding and the resolved value, with no app involved (`cargo test -p designos-doc text_style_ops`).
- **AC-6** Two-step undo: type in a linked layer, then bind a style variable from the Variables panel. The first undo removes the binding, and the second undo removes the typed text.

## 6. Test steps

1. `gh pr checkout 1677`.
2. `cargo test -p designos-app --bin designos styles::variable_binding_tests`, `cargo test -p designos-doc text_style_ops`.
3. Revert the fix hunks only, rerun, and confirm red (mutation check).
4. Workspace gates. Evidence: terminal capture of red and green.

## 7. Edge cases

- Text with mixed runs: the existing `has_single_text_style` refusal is unchanged.
- A Sticky or Shape-with-text: create stays `NodeKind::Text`-only (`text_layers`, `text_style_ops.rs:21-30`), unchanged.
- A library text style: apply removes owned node bindings the same way. Create is local only.
- `#1448` will later keep a binding added *after* apply as an override. This PR's apply rule (clear on apply) stays correct under #1448, because apply = reapply.

## 8. Non-goals

- The override model (#1448).
- Colour-style binding behaviour (already correct).

## 9. Builder rules

Carry #1677 with credit (`Co-authored-by` per the #1424 precedent). The squash title must not carry issue or audit codes beyond the `(#1030)` suffix. `Closes #1030`.
