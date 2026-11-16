# #1432 Text-style link: K scale, paste properties and a redundant-run re-pick ignore the detach rule

| Field | Value |
|---|---|
| Issue | #1432 (bug, P2, milestone v0.8). Five cases: three in the body, two in the 2026-10-31 comment |
| Still real? | Partly. Case 1 (K scale) and case 2 (paste properties) were changed on main by #1616 (`d6e6b67ee`); both now detach. Case 2's Figma-parity check was never done. Cases 3 and 4 are still real. Case 5 has contributor **draft PR #1643** |
| Size | Cases 3 and 4: S/M, about 4 h. Case 5 carry: S, about 1 h. Case 2 parity check: about 30 min of the owner-allowed scratch Figma file |
| Tier | Cases 3 and 4: Argon. Argon built #1424; sim tests can check the work. Case 5: carry #1643 with credit, reviewed by Grok read-only |
| Depends on | #1677 (#1030) merged first, because both touch the text-style paths. **#1448 depends on this spec's comparison helper** |
| Ready after owner answers? | Yes. The case 2 parity fact comes from a live check, not an owner question |

Code baseline: `origin/main@d22dad970`.

## 1. Outcome

- Re-picking the same font family, or changing only the text colour of a linked text layer, keeps its text style link. Nothing the style owns changed.
- Text colour from the Typography section reaches every text-bearing layer in a mixed selection, such as Text and Sticky together, and keeps the Text's style link.
- Paste properties and K scale behave as Figma does. Paste is checked live.

## 2. Re-anchor (origin/main)

- **Case 1 (K scale): changed.** `scale_node_properties_proportionally` now removes the text link when the text changed (`crates/designos-app/src/canvas/interact.rs:1567-1585`, via #1616 / contributor PR #1507). It compares the whole `TextProps` (`:1583`), not the owned fields. That is harmless for K scale, which always changes `font_size`. Figma parity for K scale is still unverified (the issue asked for it).
- **Case 2 (paste properties): changed, parity unverified.** `paste_properties` writes the source's text style and removes the target's `"text"` link (`crates/designos-app/src/ops.rs:1111-1114`). The issue says "check what Figma does (it may carry the source's style link)". #1616 did not check.
- **Cases 3 and 4: still real.** `style_owned_changed` (`crates/designos-app/src/canvas/textsession.rs:339-345`) treats any change in run *count* or run *ranges* (`:343-344`) as an owned change. So:
  - a fill-only run split or merge detaches (case 4);
  - re-picking the same family on a text with a redundant run detaches (case 3).

  Both the in-session path (`edit_props_with_link`, `:415`) and the out-of-session path (`edit_node_text_detaching`, `:353-363`) use it.
- **Case 5: still real.** `set_style_preserving_link` matches `NodeKind::Text` only (`crates/designos-app/src/panels/typography.rs:83-96`, the match at `:90`). PR #1643 (NamAIEngineer, head `495cc74c9`, 2 files) routes it through `any_text_mut()`. It **also** changes the Typography/Fill gating, so a Sticky first in the selection shows the controls, and the `nodes_summary` filter. That goes beyond case 5 (see §3.4).

## 3. Scope and rules

1. **Shared comparison helper (cases 3 and 4).**
   - Replace the run-structure comparison with a per-segment comparison of the *owned* style. Split the text at the union of the before and after run boundaries. For each segment, compare the effective `TextStyle` with fills and alignment ignored (`same_text_style` already ignores them; check `designos_doc::lint::token_match::same_text_style`). Then compare the paragraph triple (indent, list spacing, leading trim).
   - Name: `style_owned_fields_changed(before: &TextProps, after: &TextProps) -> OwnedChange`. It returns which owned fields changed (a small bitset), not only a bool. **#1448 reuses the bitset** to mark overrides.
   - Place it in `designos-doc/src/text_style_ops.rs`, next to the create/detach operations (#1028), so MCP can share it.
   - Both `textsession.rs` call sites and the K-scale site (`interact.rs:1583`) use it. That is one rule (DRY).
2. **Case 2 (paste properties).** Run the live check first: a linked text A and an unlinked text B. Copy A's properties and paste them on B, then the reverse with B linked to another style. Record the result in the PR body.
   - If Figma carries the source's link: paste sets the target's `"text"` link to the source's style key, when the source had one.
   - If Figma does not carry it: keep main's behaviour and add a test that pins it.
3. **Case 1 (K scale).** The same live check: K-scale a linked text. Keep main's behaviour (detach) unless Figma differs. Pin it with a test either way.
4. **Case 5 (carry #1643).**
   - Keep the `any_text_mut()` routing and the `fit_text` restriction to `NodeKind::Text`.
   - Selection-order gating (Sticky first shows the Typography controls) is extra scope, but it follows the owner's canvas-merge ruling (Sticky, Shape and Connector are text-bearing, from the #1424 round 3 ruling). Controller decision: accept it in the same PR with its own test, and note it in the PR body.

## 4. Files likely touched

`crates/designos-doc/src/text_style_ops.rs` (helper); `crates/designos-app/src/canvas/textsession.rs`, `canvas/interact.rs`, `ops.rs` (case 2, only if parity differs); `panels/typography.rs`, `panels/properties.rs` (PR #1643).

## 5. Acceptance criteria (each red on main unless noted)

- **AC-1** (case 4) A linked text with one word in its own fill-only run, coloured blue. Apply `set_style(fills = blue)` to the whole text. The runs merge and the link is **kept**, both inside and outside a session.
- **AC-2** (case 4) A partial fill on one word inside a session creates a fill-only run, and the link is kept.
- **AC-3** (case 3) A linked text with a redundant run (same owned style as the base). Re-pick the same family: the link is kept and nothing changes in the undo history. Builder checks this against the existing no-op rule.
- **AC-4** A size change on one word still detaches (the existing #1424 behaviour is unchanged).
- **AC-5** (case 5) A [Text, Sticky] selection gets a text colour from Typography. Both layers change, and the Text keeps its style link. The [Sticky, Text] order shows the controls.
- **AC-6** (cases 1 and 2) Tests pin the behaviour the live check found. The PR body quotes the check: date, file, and what was observed.
- **AC-7** The helper's bitset names the changed owned fields: a test where only `letter_spacing` changes returns exactly that field.

## 6. Test steps

`cargo test -p designos-doc text_style_ops`, `cargo test -p designos-app --bin designos canvas::textsession`, `cargo test -p designos-app --bin designos panels::typography`, revert checks for AC-1 and AC-3, then the workspace gates. Evidence: a terminal capture and a 1440 capture of the case 5 selection.

## 7. Edge cases

- Run boundaries at UTF-8 grapheme edges: the segment split uses byte ranges as `TextRun` does, with no new boundary logic.
- An empty text: no segments, no change.
- Alignment-only change: kept, as alignment is not owned (`textsession.rs:338` doc comment).
- A text on an instance whose style comes from the main: unchanged by this spec.

## 8. Non-goals

- Keeping the link on *owned* edits (that is #1448).
- Sticky/Shape create-style.

## 9. Builder rules

Two PRs:
- the helper plus cases 1–4 (Argon), `Refs #1432`;
- the #1643 carry, `Closes #1432`, merged last.

The helper PR must merge before #1448 is dispatched.
