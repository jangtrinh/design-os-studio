# E1 Styles & Variables: build-ready specs for the 14 open sub-issues (2026-11-06)

Epic #14. Specs re-anchored on `origin/main@d22dad970` (fetched 2026-11-06 22:5x). Line numbers are cited with symbol names because they move. Every load-bearing claim in a spec is tagged FACT, INFERENCE, ASSUMPTION or UNKNOWN. Nothing here edits GitHub.

## Status table

| Issue | Still real? | Size (builder h) | Tier | Depends on | Ready after owner answers? |
|---|---|---|---|---|---|
| [#132](132-bind-variables-gradient-stops-effects-glass.md) stops/effects/glass binds | Partly. Instance-override effect export drops radius/spread/offset bindings (`fig_export.rs:2246-2257`); one stop test is missing; glass waits for #38 | S, 3 (glass sized in #38) | Argon | #1664; glass: #38 | Yes (no question) |
| [#133](133-bind-variables-min-max-font-family-paragraph.md) min/max, font family, paragraph | Partly. min/max keys missing from `property_kind`; no font request on a mode switch; no `.fig` tests; **no min/max inspector fields exist** | A: S 4 · B: S 3 · C: M 6 | A Argon · B Sonnet · C Sonnet + es-designer | A: #1664 | A, B yes · C after **Q1** |
| [#134](134-bind-variables-component-properties.md) component property binds | Yes. `PropValue` has no binding; an alias in `varValue` is dropped on import (`fig.rs:2084-2086`) | L, 14 | Sonnet | REA step 0; #1664; hot-file order | Yes (variant leg waits on REA, not the owner) |
| [#135](135-extended-variable-collections-overrides.md) extended collections | Yes. No parent link; `VARIABLE_OVERRIDE` nodes dropped (`fig.rs:1436`, `:1532`) | XL, 24 | Sonnet | REA step 0; #1664; after #134 | Slices 1–2 yes · slice 3 after **Q3** |
| [#136](136-display-p3-colour-per-document.md) Display P3 per document | Yes. No profile in model, render, GPU (`Rgba8Unorm`) or PNG | XL, 28–32 | Sonnet | REA step 0; #1664 (slice i) | i–iii yes · iv after **Q2** |
| [#137](137-oklch-oklab-css-output.md) oklch()/display-p3 CSS | Partly. Picker shipped (#1451, #1554). CSS prints hex/rgba only (`util.rs:70`); `Paint` has no authored colour | a: M 8 · b: S 2 | Argon | a and b: #136 slice i (2026-10-31 ruling); a also needs #1664 and **Q4** | a after **Q4** and #136 i · b after #136 i |
| [#141](141-animation-styles-model-picker-fig.md) animation styles | Yes, **blocked**: no Motion model; #140 open | M, 8 | Argon or Sonnet (at dispatch) | #140 | No: blocked on #140 |
| [#143](143-easing-timing-variables.md) easing/timing variables | Yes, **blocked** for tracks: #140, #142 open | M, 10 | Sonnet | #140, #142; REA step 0 | No: blocked (a transition-only slice may start if REA allows) |
| [#1030](1030-text-styles-typography-variable-bindings.md) text styles vs typography bindings | Yes. Contributor **draft PR #1677** in Grok wave A | S, 2 (review + carry) | Grok review → controller; fix-up Argon | none; merges before #1432 and #1448 | Yes |
| [#1341](1341-default-mode-switch-hug-text-agent-batch.md) default-mode switch, hug text (MCP) | Yes. Contributor **draft PR #1357** (stale base, needs rebase) | S, 1–2 | Grok build-fast (rebase + test); review by Claude/Codex | none | Yes |
| [#1432](1432-text-style-link-detach-rule-gaps.md) detach-rule gaps | Partly. Cases 1–2 changed by #1616 (Figma parity unchecked); cases 3–4 real (`textsession.rs:339-345`); case 5 = contributor **draft PR #1643** | S/M, 5 | Argon (helper) + carry #1643 | after #1677; **blocks #1448** | Yes (cases 1–2 need a live Figma check, not the owner) |
| [#1443](1443-picker-binding-context-per-frame.md) picker per-frame context | Perf half yes; mixed-mode pill fixed (#1612). Contributor **draft PR #1674** | S, 2 | Grok review; Grok build-fast fix-up | after #1630 (same file) | Yes |
| [#1446](1446-picker-style-bound-gradient-stops-lock.md) style-bound stops not locked | Yes. Reachable via `.fig` import / MCP, not via the UI | S, 3 | Argon | after #1674 (same file; #1630 is a file-order constraint only) | Yes |
| [#1448](1448-text-style-overrides-reapply.md) style overrides + Reapply | Yes ("follow Figma" ruled 2026-11-01) | L, 15 + 1 live check | Sonnet | #1677, #1432 helper, Figma step 0; `.fig` leg: #1664 | Yes |

Total about 140 builder hours, about 28 of them blocked (#141, #143). Close candidates: none fully fixed. #1443's cosmetic half and #1432 cases 1–2 are fixed on main; record that in the closing PR bodies.

## Order and file ownership

**Hot-file queue** (`fig.rs`, `fig_export.rs`, `kiwi.rs`, `rest_import.rs`; one worker at a time, binding since 2026-10-17). In-flight #1664 touches all four, so everything below waits for it:

This keeps the order the issues state (positions 23, 24, 25, 27, 28, 35):

1. #132 export arm (pos 23)
2. #133-A (24)
3. #134 (25)
4. #135 slice 2 (27)
5. #136 slice i (28)
6. #137-a (after #136 i, per the 2026-10-31 ruling)
7. #1448 `.fig` leg
8. #141 (35)
9. #143

#137-b is not on the queue: it changes only `designos-export` and runs once #136 slice i has merged.

If the controller wants #136-i earlier (it is S and it unblocks #137), that is a deviation from the 2026-10-17 queue and must be stated as one.

**Same-file chains outside the queue** (never in parallel):

| File | Order |
|---|---|
| `panels/var_bind.rs` | #1630 → #1674 (#1443) → #1446 → #1448 bind path → #134 UI |
| `styles.rs` / `text_style_ops.rs` / `textsession.rs` / `typography.rs` | #1677 (#1030) → #1643 → #1432 helper → #1448 |
| `variables_apply.rs` | #1677 → #133-A → #134 → #143 |

**Can start now** (no hot file, no file overlap with each other):
- carry #1357 (#1341);
- review #1677 (already queued) and #1674;
- #133-B (font requests; `variables/mod.rs` + `fonts.rs`);
- #135 slice 1 (doc-only, after its step 0);
- all read-only REA / Figma step-0 spikes for #134, #135, #136, #137 a0 and #1448 E1–E6, plus the #1432 cases 1–2 check.

GUI-based Figma experiments run one at a time, when the owner is idle.

## Rollback

Every slice is one squash PR. Revert = `git revert <squash>`. Additive serde fields (#135, #136, #137, #1448) use defaults that are skipped when empty, so files written before a revert still open. A file written *with* a new field and opened by an older build loses only that field, the same as any optional field.

## Owner questions

**Q1. #133: min/max size fields.** Studio has no min/max width/height fields today. Binding them needs a place to click. Unblocks #133 leg C.
- **A (recommended):** add Figma-style min/max fields (W/H dropdown "Add min width…"), each with the bind icon, in #133. Without them the binding exists only through `.fig` and agents.
- B: bind through `.fig`/MCP only now; file the fields as a separate E2 layout issue.
- C: no min/max fields; bindings stay data-only.

**Q2. #136: file colour-profile setting (Assign / Convert to Display P3).** Unblocks #136 slice iv.
- **A (recommended):** include it as the last slice of #136. Without it a P3 document can only come from a Figma import.
- B: separate issue after #136.
- C: never; P3 is import-only.

**Q3. #135: how much extended-collection UI.** Unblocks #135 slice 3.
- **A (recommended):** the Variables panel shows inherited versus overridden values; editing a cell creates an override; "Reset" removes it. Creating a new extension comes later. This covers all three acceptance criteria with the smallest UI.
- B: also "Extend collection" to create extensions in Studio.
- C: no panel UI; extensions only open, render and save correctly.

**Q4. #137: which picked colours change CSS output.** Figma prints `hsl()` for colours edited in HSL, and `oklch()`/`oklab()` for colours edited in those modes. Unblocks #137 slice a.
- **A (recommended):** OKLCH and OKLAB only. Colours edited in HEX/RGB/HSL/HSB keep today's hex/rgba output; that matches the issue and keeps existing exports stable.
- B: follow Figma fully, so HSL-edited colours print `hsl()` too.

**Q5. Milestones.** #132–#136, #1030 and #1341 sit in "Backlog: AI design 2", while their bodies and the epic amendment (2026-10-17) say v0.8 Variables everywhere. Unblocks release planning only.
- **A (recommended):** move the open parts of #132–#137, #1030 and #1341 to v0.8, and keep #141/#143 in v0.17. This matches the epic amendment.
- B: keep them in Backlog; v0.8 ships only the bug fixes (#1030, #1341, #1432, #1443, #1446, #1448).

## Decided without asking (invisible or already ruled)

Each assumption gives its confidence and the evidence that would flip it.

| Decision | Confidence | Would flip if |
|---|---|---|
| #132 glass waits for #38 (wave-1 ruling 2026-10-30) | high (ruled) | owner reverses the ruling |
| #137: colour space is authored per paint, in Figma notation, with no OKLCH output-format toggle (re-ruling 2026-10-31) | high (ruled) | owner reverses the ruling |
| #1448 follows Figma (owner 2026-11-01). The override set is stored explicitly per node, not derived from value ≠ style | medium | step 0 E4 shows Figma itself re-derives overrides on a style edit (an override equal to the new style value disappears) |
| #1030: Create carries bindings, so the "drawn values + toast" fallback is moot | high | `style_property_kind` Text keys (`styles_ops.rs:41-52`) turn out not to round-trip through `.fig` style export |
| #1030: Create from layers with differing bindings is refused, the same as for differing values | medium | owner prefers carrying the common subset; it is user-visible but follows the existing refusal rule |
| #1446: a pill shown from a style does not unbind the style's variable; it acts like any edit of a style-linked paint | medium | Figma's picker unbinds the style variable from a linked layer (live check) |
| #1341: the MCP twin of `changed_variables` stays in MCP, with a doc comment pointing to the app copy | high (invisible) | reviewer requires a shared helper in the PR |
| #1432 case 5: PR #1643's gating for any selection order is accepted, per the canvas-merge ruling | high | owner says Sticky-first should not show Typography |
| #136: a P3 document stores P3 component numbers (ASSUMPTION) | medium | step 0 (a) shows Figma stores sRGB numbers plus the profile |
| #135 DTCG export writes resolved values per extension mode | medium | DTCG 2025 spec gains an inheritance construct that Figma's DTCG export uses |

## Unresolved (not owner questions; parity facts for the step-0 spikes)

- #134: whether Figma lets a VARIANT property bind to a variable, and how it stores the alias.
- #135: the `VARIABLE_OVERRIDE` payload, and what Reset writes.
- #136: whether a P3 document stores P3 numbers (assumed yes), and how images are handled.
- #137: the `AuthoredColor` `c0..c4` mapping per space.
- #1448: whether Reapply removes bindings (E3), and how the indicator looks (E6).
- #1432: Figma's paste-properties link behaviour and its K-scale link behaviour.
- #143: whether prototype transitions consume EASING/TIMING variables.
