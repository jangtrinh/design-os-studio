# UI polish rulings — round 1 (controller, 2026-11-02)

Input: `plans/reports/critique-261102-project-docs-mockups-r1.md` (independent Opus critique, 102 findings: 22 HIGH / 50 MED / 30 LOW). Owner bar: "flawless in every detail"; owner said not to be asked about UI details ("bế từ design studio sang cho khỏi phải quan tâm nhiều").

## R0 — Authority order for chrome
Studio's shipped widget kit on `main` (`crates/designos-app/src/widgets/kit/*`, `theme.rs`) beats the part-2 spec wherever they disagree. Part 1 already says chrome uses Studio's existing constants; the spec values that contradicted the kit were the controller's error. Spec text is corrected to the kit in the same change. Tokens verified on main e4efa94: `BORDER_STRONG` #404040, `BG_ERROR_SOLID` #DC2626, `TEXT_WHITE`, `BG_OVERLAY` #262626 b3, `BG_PRIMARY_SOLID` #262626, `BG_PRIMARY_ALT` #171717, `TEXT_PLACEHOLDER` #737373, `TEXT_ERROR` #F87171, `TEXT_SECONDARY_HOVER` #E5E5E5, `TEXT_ON_BRAND` #0A0A0A, `AGENT_1` #FFA629, `widgets/kit/surface.rs` `modal_surface`.

## Global findings G01–G10: accepted as the critique states
Secondary button, destructive button, modal scrim/surface, tooltip, toast, fields/search/select, danger text: copy the kit exactly (read the kit source for every value, do not infer from screenshots).

## Q1 — Checkbox
Use the kit checkbox as is (radius 4, `BG_BRAND` fill when checked). Reason: one checkbox in the product; a checked box is a state, which P2 allows the brand colour for. Multi-select tiles in the picker use the same checked look on their check circle.

## Q2 — Selected tree row
Use the kit's selected-row treatment for Layers/Pages rows (text colour and weight as the kit draws them). Spec part 1 is corrected.

## Q3 — Project tile colour
Reuse `TILE_SPEC_BG` / `TILE_SPEC_GLYPH`: the Project tab replaces the Spec tab, and no new token is added.

## Q4 — Agent colour in docs
Keep the agent session palette (`AGENT_1` …) exactly as on canvas (owner decision P12: "like the canvas"). To stop the write flash reading as a focus ring, the flash outline is 2 px with the agent colour at 60 % and the agent name tag is always shown with it; focus rings stay 1 px `FOCUS_RING`.

## Q5 — Count in the primary button
No pill inside the orange button. The footer shows a count pill with "selected" at the left ("3" pill + "selected"), the button reads "Pin frames". Spec §5 corrected.

## Q6 — KPI sample numbers
Make sample data consistent: the linked spec in the sample has 3 screens, so the Screens tile shows `3 · 24`. The metric definition (D3) is unchanged.

## Q7 — Principles text contrast note
Not in scope of this work. Recorded for the design-system owner.

## Sample-data and copy rules for round 2
- One sample world across all screens: project "Checkout", files Checkout-web (6 frames: Cart, Payment, Order confirmed, Card declined, Processing, Empty cart) and Checkout-mobile (3 frames: Cart, Payment, Order confirmed). Pinned: Screens = Cart, Payment, Order confirmed (web); States under Payment = Card declined, Processing.
- Page titles identical everywhere they appear (tree, tab, H1, picker).
- ux-writing rule 1 and 13 fixes as the critique lists: "Copy link to block" → "Copy block link", "Copy link to page" → "Copy page link", "Open in canvas" keeps its label (verb + object, 3 words) but uses the canvas/frame glyph, never the external-link glyph; the external-link glyph is reserved for links that leave the app.

## Round 2 addendum (controller, 2026-11-02): the fixer's four "Needs owner" items
All four are settled by R0 (the shipped kit wins) and need no owner input:
1. Placeholder and quaternary icon colour `#737373` (4.17:1 on `#0A0A0A`): keep the kit token. The mockups mirror the product; changing a product-wide token is design-system work, recorded next to Q7 as a follow-up for the design-system owner.
2. Home card gap 24 and header-to-grid 32 (product) vs P3/P9 text: the mockups copy the product. Principles text vs product drift is the same design-system follow-up.
3. Icons: Phosphor. FACT: `crates/designos-app/src/icons.rs:1` vendors `@phosphor-icons/core`; the round-2 brief's "Lucide" was the controller's error.
4. Focus ring: the kit's `widgets/kit/focus.rs` `focus_ring` band, not the 1 px in Q4. Q4's "1 px" is corrected; the agent write flash stays 2 px at 60 % with the name tag, so the two still differ by colour and tag.
