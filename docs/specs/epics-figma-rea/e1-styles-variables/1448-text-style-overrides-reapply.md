# #1448 Text styles: keep the style linked when a property is overridden or bound, with Reapply text style

| Field | Value |
|---|---|
| Issue | #1448 (milestone v0.8). Owner ruling 2026-11-01 (AskUserQuestion): "follow Figma", after #1452 merges. #1452 merged as `e4efa94e2` |
| Still real? | Yes. Main detaches on every style-owned edit (#1424 `ccdac9464`) and on K scale and paste (#1616) |
| Size | L: about 12–16 builder hours, plus about 1 h of live Figma experiments first |
| Tier | Claude Sonnet: judgment-heavy, touching text sessions, style propagation, `.fig` and undo |
| Depends on | #1677 (#1030) merged; #1432's comparison helper merged; the live Figma experiments (step 0); for the `.fig` leg, #1664 merged |
| Ready after owner answers? | Yes. "Follow Figma" is ruled, so the remaining unknowns are parity facts for step 0, not owner questions |

Code baseline: `origin/main@d22dad970`.

## 1. Outcome

- A designer changes the size of a text layer that uses "Body/M", or binds its line height to a variable. The layer keeps "Body/M" and shows that it has overrides.
- "Reapply text style" in the Typography section removes the overrides and their bindings in one undo step.
- Editing the style later updates every linked layer but keeps each layer's overridden fields.
- "Detach style" is the only way to drop the link.

## 2. Re-anchor (origin/main)

- FACT: the detach-on-edit rule is applied in three places:
  - `edit_node_text_detaching` and the in-session `LinkEdit::DetachIfStyleOwnedChanged` (`crates/designos-app/src/canvas/textsession.rs:353-363`, `:374`, `:415-418`);
  - K scale (`canvas/interact.rs:1583-1585`);
  - paste properties (`ops.rs:1114`).
- FACT: bind on a style-linked text layer goes through the bind picker. Its text path calls `requesting_new_faces` (`crates/designos-app/src/panels/var_bind.rs:724-725`). The detach-on-bind rule ruled in W2a / #1452 lives on that path; the builder cites the exact line.
- FACT: style edits push values to every linked node. `update_style_values` → `update_style` in `designos-doc`, then re-fit (`crates/designos-app/src/styles.rs:205-235`). Nothing knows which fields a node overrode.
- FACT: `.fig` already imports layers that are linked *and* have values different from their style, because Figma writes them that way. Import keeps the link and the values, so main already shows an "override" state it cannot name. INFERENCE from `fig.rs` text import plus #1327's fix; step 0 confirms.
- FACT (Figma RE, `plans/reports/re-figma-261031-color-modes-typography-binds.md:60-64`):
  - the selection property `numTextStyleOverrides`;
  - the button `clearTextStyleOverridesButton`, labelled "Reapply text style";
  - the engine call `clearOverridesOverTextStyleInSelection`;
  - no bind path detaches.

  UNKNOWN: whether Reapply removes bindings; whether the panel shows per-field controls while a style is linked.

## 3. Step 0: live Figma experiments (about 1 h, on the owner-allowed scratch file, through `figma-agent`)

- E1: Change the size of a linked text, then read the result. Expected: `textStyleId` is kept and `numTextStyleOverrides` = 1.
- E2: Bind `lineHeight` on a linked text, then read. Is the style kept? Does the override count change?
- E3: Run "Reapply text style" after E2. Is the binding removed?
- E4: Change the style's size while a layer overrides its size. Is the layer's size kept?
- E5: Override size on one run (mixed). Does the count include run-level overrides?
- E6: Where the override indicator is drawn and what it says. Take a screenshot for the Studio design.

Record the results in `plans/reports/re-figma-<date>-text-style-overrides.md`. Rules 1–6 below assume the expected answers. A different answer replaces the rule it contradicts, and the builder notes the change in the PR.

## 4. Scope and rules

1. **Model.** Controller decision on the invisible choice, recorded:
   - store an explicit per-node override set `Node.text_style_overrides: OwnedFieldSet`, a bitset over the owned fields from #1432's helper, with a serde default of empty;
   - do not derive overrides from "value ≠ style". Derivation breaks when the style later changes to equal an override, or when an override equals the old style value.
2. **Edit.** Rule 1 replaces the detach-on-edit rule at all three sites:
   - an edit that changes owned fields (per #1432's helper bitset) keeps the link and ORs those fields into `text_style_overrides`;
   - an edit that changes nothing owned changes nothing.
3. **Bind.** Binding a typography variable on a linked layer keeps the link and adds that field to the override set (E2). The detach-on-bind rule is removed.
4. **Style edit.** `update_style` writes the new style values only to the fields that are *not* in a linked node's override set (E4). Re-fit as today.
5. **Reapply text style.**
   - The command writes the style's values into every overridden field, removes the node's bindings on owned keys (E3), and clears the set.
   - It is one undo step for the whole selection.
   - It shows only when the selection has overrides.
6. **Detach style.** Unchanged: the only way to remove the link. It clears the override set.
7. **UI.** Per E6 and `design/principles.md` P5/P11:
   - the text-style row shows an override mark and count (count pill, P5);
   - "Reapply style" sits in the style row's menu, verb + object, ≤ 3 words;
   - Load `es-designer` before UI code.
8. **`.fig`.**
   - Import: a linked text whose owned values differ from its style gets the set computed once at import. Since the file has no explicit set, this is the only derivation, and it is documented.
   - Export: values are written as they are, which is how Figma stores them.
   - A `.designos` round trip keeps the set.
9. **MCP.** `designos_get_node` reports `text_style_overrides`. `apply_batch` update follows the same edit rule. Shared through `designos-doc` (#1028 pattern).
10. **Old files.** No field, so an empty set. Behaviour for already-linked layers whose values differ (from import): the set is derived on load, once, by the same import rule (rule 8). The builder confirms this is cheap on a 112k-node file: one pass, no allocation per node without a text link.

## 5. Files likely touched

`crates/designos-doc/src/lib.rs` or `node.rs` (field), `text_style_ops.rs`, `styles_ops.rs` (`update_style`); `crates/designos-app/src/canvas/textsession.rs`, `canvas/interact.rs`, `ops.rs`, `styles.rs`, `panels/typography.rs`, `panels/var_bind.rs`; `crates/designos-fig/src/fig.rs` (import derivation); `crates/designos-mcp/src/tools/` (node read and update).

## 6. Acceptance criteria (red on main unless noted)

- **AC-1** A linked text gets its size changed in the Typography section. The link is kept, `text_style_overrides = {font_size}`, the row shows the override mark, and it is one undo step.
- **AC-2** Bind `lineHeight` on a linked text. The link is kept and `{line_height}` is added.
- **AC-3** Reapply text style: the values equal the style, owned bindings are removed (per E3), the set is empty, and one undo restores all three.
- **AC-4** Edit the style's size from 16 to 18 with two linked layers, one with a size override of 24. The plain layer goes to 18, and the overriding layer stays 24.
- **AC-5** Detach style removes the link and the set, and the values are unchanged (existing behaviour kept).
- **AC-6** K scale and paste properties on a linked text follow rule 2, or the step 0 result if it differs.
- **AC-7** `.fig` import of a Figma fixture with an overridden linked text gives a non-empty set and the link. Export, then reimport, gives the same set.
- **AC-8** Fill-only and alignment-only edits leave the set empty (#1432 helper).
- **AC-9** The 1440 capture shows the override mark and the Reapply item. The Zero Fake UI gate passes.
- **AC-10** Performance: loading the VSF-sized benchmark adds no measurable time. Report a measured number with its citation, or SKIPPED with the reason.

## 7. Test steps

`cargo test -p designos-doc`, `cargo test -p designos-app --bin designos canvas::textsession`, `panels::typography`, `styles::`, `cargo test -p designos-fig`, `cargo test -p designos-mcp`, then the workspace gates, revert checks for AC-1, AC-3 and AC-4, and the 1440 capture.

## 8. Edge cases

- A run-level override (one word bigger): set per E5. Expected: the node-level set includes `font_size`.
- An instance text whose main's text is linked: overrides on the instance stay instance overrides (#1327/#1347 paths). The builder must not double-count.
- A library text style updated through a library update: same as AC-4.
- A mixed selection of linked and unlinked layers: Reapply acts on linked ones only.
- Undo across a style edit that skipped overridden fields: restores exactly.

## 9. Non-goals

- Fill, effect or grid style overrides (the fill/stroke model is #1464 / PR #1484).
- Per-run override display.

## 10. Builder rules

One PR, or two if the `.fig` leg waits for the hot-file queue: doc, app and MCP first, then `.fig`. `Closes #1448` on the last one. This supersedes the detach rule of #1026; say so in the PR body.

> Controller rulings 2026-11-07 (after the rebuild):
> - Step 0 (live Figma experiments) SKIPPED: no owner scratch file is named. E3/E4/E6 stay recorded assumptions; revisit when the owner names a scratch file.
> - AC-7 scoped: a kiwi `.fig` import is style-authoritative (fig.rs), so base-value overrides cannot arrive from kiwi. AC-7 is met by overrides that arrive through character ranges, bound variables and REST JSON, plus a save → reimport round trip of a set made in Studio.

> OWNER RULING 2026-11-07 (AskUserQuestion, "Match Figma (Recommended)"), supersedes rule 1's field scope and rule 2:
> Counsel evidence (plans/reports/kongming-261107-1448-fig-override-roundtrip.md): Figma's override-over-text-style covers only fontWeight/italic (font style), text decoration and hyperlink (Plugin API TextStyleOverrideType; kiwi styleOverrideTable `isOverrideOverTextStyle`, schema field 376; 114 such entries in the Untitled UI kit, 0 for size/line height). Node typography values under a link are stale in Figma files (51% of 12,400 kit texts differ; Figma renders the style), so the style-authoritative kiwi import is correct.
> - The stored override set covers ONLY: font weight / font style (italic), text decoration, hyperlink. Edits to those keep the link and record an override.
> - Edits to size, line height, letter spacing, paragraph spacing/indent, font family, text case and any other style-owned field DETACH the style, exactly as on main. Binding a variable to such a field also detaches.
> - .fig export/import maps the set to native `styleOverrideTable` entries with `isOverrideOverTextStyle: true` (lossless round trip; no Studio-only pluginData).
> - Reapply text style, the count pill and instance sync (copy the set; B2) apply to the 4 overridable types.

## Controller rulings, round 3 (2026-11-07 13:00)
- **Reapply clears the hyperlink.** A hyperlink is one of Figma's four override types, so "Reapply style" clears it together with the others. Confidence is low: the Figma scratch check (Step 0) is still in the owner batch. If Figma keeps the link, this one line flips.
- **Hyperlink `.fig` export stays layer-level for now.** Studio has no per-range hyperlink. Import already hoists a flagged hyperlink entry onto the layer. Writing it as a flagged `styleOverrideTable` entry is a follow-up, not a merge blocker.
- **Size writers outside the rule** (lint-fix, cmdk auto-fix, whiteboard `set_text_size`) behave as they did on main, where they did not detach either. Follow-up issue filed; not part of #1448.
- **#1432 AC 5** (Text and Sticky selection) stays with #1643. The #1448 PR says `Refs #1432`.
