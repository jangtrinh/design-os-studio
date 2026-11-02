# UI polish rulings — round 2 (controller, 2026-11-02)

Input: `plans/reports/critique-261102-project-docs-mockups-r2.md` (independent Opus re-review: 1 HIGH / 11 MED / 14 LOW, verdict NEEDS CHANGES). R0 from round 1 still holds: the shipped kit wins for chrome, and the spec text is corrected to the kit in the same change.

## S1 — Slash menu cap (critique Q1, M04b-1)
Keep "the cap cuts a row": it is how the user sees the list scrolls. Change the value, not the rule: max height = menu top inset + 10.5 × kit menu row height + bottom inset, computed from `widgets/kit/rows.rs`, so half of row 11 shows under the cap. Spec §3.2 gets the computed number.

## S2 — Trash wording (critique Q2)
Pages go to Studio's own trash (FACT: `designos-library/src/store.rs:484` `trash_dir`; Home `main_view.rs:184,512` write "trash" lowercase in sentences, `sidebar.rs:141` uses "Trash" only as the section label). In sentences write "trash" lowercase; capital only when naming the Home section as a label.

## S3 — Per-file count pill in the picker (critique Q3, M08-1)
The pill counts the rows that list shows under that file: all frames in Pin mode, unpinned frames in Suggest mode. Same meaning ("rows below"), so both screens are right; spec §5 wording changes from "frame count pill" to "count of the rows listed under the file".

## S4 — Wide table (critique Q4, M06-2)
A table wider than the reading column scrolls horizontally inside its own box (no page scroll), with the kit horizontal scrollbar visible. The add-column "+" belongs to the last column (8 px right of it) and scrolls with the table; at scroll start it is out of view. M06 shows the table at scroll start with the scrollbar, the cut column cut cleanly at the box edge, and a second small capture state (m06 already has room or a new m06b) scrolled to the end showing the "+" next to the last column. Spec §4 says this.

## Everything else
Every other HIGH/MED/LOW finding is accepted as the critique states. Sample data must stay one consistent world: a "Frame not found" card must name a frame that does not exist in that world (e.g. a deleted "Promo banner"), and a "File not found" card must name a file that is not in the project (e.g. "Checkout-tablet").

## S1 confirmation (after round 3)
The cap is 344: the 24 px "Basic" section label counts as the first entry, so half of the eleventh entry shows under the cap, which is what S1 asks for. The literal "8 + 10.5 × 32 + 8 = 352" ignored the label and is not used.
