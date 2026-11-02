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

## Round 3 rulings (after `critique-261102-project-docs-mockups-r3.md`)
- **T1 — Table column width (N1).** Never ellipsis in a cell; hiding cell text hides data. Column width = widest cell content + 12 px insets each side, clamped to 120–320. Text that is still longer wraps inside the cell and the row grows. Tables wider than the column scroll horizontally (S4). Spec §4 states it; M06b's Notes cell wraps.
- **T2 — Two-menu capture (N2).** Split like M06/M06b: M04b = unfiltered menu at the top of its list, new M04c = the same menu scrolled to the end. One menu per capture, no composite.
- **T3 — Picker pill wording (N3).** "The number of frames listed for that file: all frames in Pin mode, unpinned frames in Suggest mode. Not the number of page rows." Replaces S3's wording; the meaning is unchanged.
- **T4 — LOWs N4–N9 accepted.** Scrollbar thumb formula in the spec matches the mockup (thumb = track × viewport ÷ content). Slash-menu thumb inset is the kit's 2 px (`menu.rs`). "Turn into" line height matches the spec 14/20. Spec part 1 drops the old `Badge/StatusCount` name. M04/M04b/M04c show the callout block like every other "Payment methods" screen. README "Needs owner" drops the settled items.
