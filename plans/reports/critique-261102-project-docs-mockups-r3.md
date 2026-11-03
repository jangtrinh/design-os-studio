# Critique r3: Project docs workspace mockups (22 screens)

Reviewer: independent (did not build; report only). Worktree `plan/project-docs-workspace` @ a3fb8041 (round-3 fix = c221d9d5; diff reviewed `b91395f3..HEAD -- plans/261102-project-docs-workspace`). Date 2026-11-02.
Method: headless Chrome 1440×900 @2x of all 22 mockups + index (`--headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=2`), M09-full also at 1440×2630; computed boxes/styles via Playwright at 1x (`q.cjs`); 2x crops; kit source read for every chrome value cited. Evidence: `scratchpad/review-r3/` (`cap/` 2x, `v/` 1x views, `x1/` 1x renders, `z/` crops and montages, `gate/*.json`, `slop.txt`, `q.cjs`).

## Not verified (read first)
- Motion, drag-ghost offset, screen-reader names, egui text metrics: static HTML, nothing to measure.
- Responsive captures: skipped by owner default (none exist).
- Kit horizontal-scrollbar thumb formula: no kit source found for the table scrollbar, so N4 only compares spec against mockup, not against the kit.
- Tree-row keyboard focus beyond the one specimen (M13 Processing): CSS read only.

## Verdict
**NEEDS CHANGES**: 0 HIGH, 3 MED, 6 LOW (new). All 26 round-2 findings are VERIFIED; none is NOT FIXED and none REGRESSED.
Weakest link: the new M06b screen shows the only truncated cell in the set ("3-D Secure when the i…"), and the spec has no rule for column width or cell overflow, so an engineer has to guess how wide a column is.

Deterministic lenses: `ui gate` exits 1 on all 22 files, as in round 2. The only errors are the settled `#737373` low-contrast pair (4.18:1 and 3.19:1; addendum item 1). Per-file finding classes are unchanged from round 2 except `off-system-token` warnings, which rose by 8 on every file (544 → 552). Those 8 are new `var(--fs-sm/--lh-sm/--fs-xs/--lh-xs/--lh-xl/--control-xs/--w-strong/--text-secondary)` references in shell.css, all of the same class. M06b has the same classes as the other shell screens. `slop-detect`: 207 findings in the same four classes (cramped-padding 180, clipped-overflow-container 19, overused-font 7, gray-on-color 1), all false positives as explained in r2. Committed PNGs match fresh 1x renders exactly (ImageMagick AE = 0 at 2 % fuzz on all 22, m09-full at 1440×2630). `design/memory.events.*` were restored with `git checkout` after the gate run.

## 1. Round-2 verification (26)

| ID | Measured now (1x CSS px) | Result |
|---|---|---|
| G1 | Spec part 1 l.40 now gives the kit `count_badge` neutral triple, 22 tall, min width 22, 6 px padding, full radius. M03 `.g .pill`: 22×22, `#171717`, 1 px `#404040`, `#D4D4D4` Inter 12/18/500, r11, padding 6/6 (badge.rs:31-33, 75-79; `BADGE_SM` 22, theme.rs:495). | VERIFIED (old name remains on l.36, see N6) |
| G2 | M07: file icon x328, label x348 (row x304: +24/+44); page icon 344, label 364 (+40/+60). M08: "All files" 328/348, file rows 328/348, so all labels share one column. Spec §5 states +44/+60. | VERIFIED |
| G3 | M04, M04b, M10, M10b, M10c: H2 = Accepted methods / Fees and limits / Before release; 1 table, 1 checklist; "On this page" lists the same 3; column bottoms 805/805/935/853/935. | VERIFIED (callout missing on M04/M04b, see N7) |
| M03-1 | M03 pair 339–387; selected fill `.selblk` starts at x391, a 4 px gap; column 399 − 12 = 387. M04/M04b pair 339–387. Spec §3.1 "60 to 12 px left". | VERIFIED |
| M04b-1 | Ruling S1: `.menu.scr` 344, clip 402–730 (328); "Code" shows 16 of 32 px; thumb 4×206.9 at x713 (= panel right 719 − 6, menu.rs:576-580), `#A3A3A3`. Scrolled-to-end menu: thumb 523.1–730 (bottom of the band), list offset 192. | VERIFIED |
| M05-1 | Link popover rows "Cart rules / Error handling / Payment flow": 304×32, 12px/18px/500. | VERIFIED |
| M06-1 | M06 Row handle [367,283] and Add row [399,377]: transparent fill, `#737373`. Column handle has the `on` look (menu open). No `.hov` control. | VERIFIED |
| M06-2 | Ruling S4: M06 `.tbl` 922 at x399, frame 1 px `#262626` r12 on the table itself; Limit 980–1130 shows 109 px up to the box edge at 1089; Notes 1130–1320 is out of view; Add column at x1329 is clipped. M06b: table at x135 (offset 264 = 954 − 690), last column ends at 1057, "+" at 1065–1089 (= 1057 + 8, flush with the box edge), centred y223 on the 36 px header; thumb 499 at 590–1089. | VERIFIED |
| M08-1 | Ruling S3: M07 Checkout-web pill 6, M08 pill 1, All files 4 (= 1 + 3 = 4 tiles). Spec §5 reworded. | VERIFIED (new wording is self-contradictory, see N3) |
| M09-1 | Cards read "Promo banner · Checkout-web" (Frame not found) and "Cart · Checkout-tablet" (File not found), in both M09 and M09-full. Neither name appears in the tree, the picker inventory or "2 files". | VERIFIED |
| M10-1 | M10 and M10c: icon box 416,99 16×20, svg 101–117, centre 109 = text centre 109 = button centre 109. | VERIFIED |
| M11-1 | `.agent-tag` "Claude" 56.3×24, 12px/18px/500, `#0A0A0A` on the session colour `#FFA629`. | VERIFIED |
| L1 (M13b-1) | Modal 500,336 440×180, centre 426 = 450 − 24; body lines y396 and y424 (20 tall, 8 apart); buttons top 452 (8 below). Matches home/dialog.rs. | VERIFIED |
| L2 (M04-1) | "No blocks match": `rgb(212,212,212)` at opacity .5, 12/500. | VERIFIED |
| L3 (M02-1) | "Clear search" 24 tall, 14px/20px/600. | VERIFIED |
| L4 (M05-2) | "Turn into" weight 600, 14 px. | VERIFIED (line-height 24 vs spec 20, see N5) |
| L5 (M12-1) | Four value rows 30 tall (194–224); all four labels at y224. | VERIFIED |
| L6 (G4) | Home y4, project tabs y3, + tab y4, divider y8 (checked on M01, M03, M13). Integer y throughout. | VERIFIED |
| L7 (M01-1) | "+ Folder" 66×24 at x482 = card right 556 − 8 − 66. | VERIFIED |
| L8 (M01-2) | Folders heading y583 = grid bottom 555 + 28. | VERIFIED |
| L9 (M01-3) | "Open project" centre y319 = name centre 319; right edge 548 = card right − 8. | VERIFIED |
| L10 (M13b-2) | M13b reads "“Cart rules” moves to the trash."; spec l.141 and the copy deck are lowercase; phase-02 l.13 reads "(moves to Studio's trash)". | VERIFIED |
| L11 (M02-2) | Placeholder "Type / for blocks" text x399.0; caret x399; column x399. | VERIFIED |
| L12 (M13-1) | Caption "Left tree is a composite specimen: …" at 304,52 (192×90). | VERIFIED |
| L13 (G5) | M13 "Processing" row: outline 2 px `#F97316`, offset −2 (band inside the row). | VERIFIED |
| L14 (M07-1) | M07 footer: dropdown 32 = segmented 32 (buttons stay 40). | VERIFIED |

Regression sweep: all 22 screens plus the index were rendered and inspected at 1x (2x crops where values changed). The shell.css changes (tab strip, `.menu` font, `.mi.dis`, `.tbtn`, `.tbl` frame, `.pk-*`, `.kpi`, `.pcard`, `.banner .ico`, `.agent-tag`, `.r.foc`) render correctly on every screen that uses them. All menus measure 12/18/500 on 32 px rows (M01, M03, M06, M13). The table renders identically on M03, M04, M10–M11c (frame cut at the box edge, thumb 499). No regression found.

## 2. New findings

| ID | Sev | Screen | Measured evidence | Citation | Exact fix |
|---|---|---|---|---|---|
| N1 | MED | M06b (also M06, hidden) | The Notes cell "3-D Secure when the issuer asks" renders "3-D Secure when the i…": `scrollWidth` 243 > `clientWidth` 190 (cell 866,241). It is the only truncated cell in the 22 screens, and a designer sees it at 100 %. Column widths 160/150/130/140/150/190 come from nowhere in the spec. Markdown tables carry no widths, and spec part 1 l.55 and part 2 §4 give no width or overflow rule. An engineer must invent column sizing (fit to content would give no ellipsis and a different table width, thumb and cut column). | Spec part 1 l.55 (table row), part 2 §4 l.77-78 ("Markdown cells are single-line"). | Add one rule to §4 (owner call). Recommended: "A column is as wide as its widest cell plus the 12 px insets, between 120 and 320. Text longer than 320 ellipsizes at the cell edge and shows in full while the cell is edited." Then either keep the Notes cell as the specimen of the 320 cap (widen Notes to 320 and recompute content width, thumb and M06 cut) or shorten the sample to "3-D Secure on request" so nothing truncates. |
| N2 | MED | M04b | The two side-by-side menu specimens leave a 20 px slot (x719–739) over the table. It shows chopped page text "Set", "2 c", "2 d", "5 c" and a scrollbar sliver, and the table header pokes out 30 px right of menu 2 (1059–1089). All of this is visible at 100 % (crop `z/m04b-gap.png`). This is a capture artifact: the product never shows two menus. The fixer surfaced it as README "Needs owner 3", but no ruling exists. | Composite-specimen logic as in r2 M13-1 ("one state per capture"); spec §3.2 describes a single menu. | Split it into M04b "Top" and a new M04c "Scrolled to end", one menu each under the caret at x399, the way M06/M06b were split. Alternatively, keep one capture and caption it "composite specimen", as M13 does. |
| N3 | MED | Spec part 2 §5 vs M07 | The new wording "+ count of the rows listed under the file (…all frames in Pin and Embed modes…)" is self-contradictory. In M07 the rows listed under Checkout-web are 4 page rows (Cart flow, Payment flow, Payment states, Confirmation), while the pill shows 6 (frames). The head clause gives 4 and the parenthesis gives 6, so an engineer can build either. | Ruling S3 (the intent is the frames the grid lists). | §5: "+ a count pill = the number of frames the grid lists for that file (all frames in Pin and Embed modes, unpinned frames in Suggest mode); it is not the number of page rows." Mockups stay as they are. |
| N4 | LOW | Spec part 2 §4 vs M06/M06b | The spec gives thumb length = 690 × 688 ÷ content width = 690 × 688 ÷ 954 = 497.6. The mockup thumb is 499 (= 690 × 690 ÷ 954). The same 1.5 px gap existed for 960 in round 2. | §4 l.77. | Pick one formula and use it in both places: either the spec says "690 × 690 ÷ content width" or `.hscroll i` width becomes 497.6. |
| N5 | LOW | Spec part 2 §3.3 vs M05 | The spec says "Turn into" is 14/20 600. The mockup is 14px/24px/600 because `.ftb .fb.txt` inherits the 24 px paragraph line-height. Nothing visible changes (the button is a 32 px flex box). | §3.3 l.55; kit button label text-sm 14/20 (button.rs:181-188). | `.ftb .fb.txt { line-height: var(--lh-sm) }`. |
| N6 | LOW | Spec part 1 l.36 | The group-row line still says "count pill at right (`Badge/StatusCount`, …)", the design-studio component name. l.40 now defines the kit `count_badge`, so one section points to two different components. | G1 fix intent (R0: spec corrected to the kit). | l.36: "count pill at right (kit `count_badge`, see Count pill; never "(3)")". |
| N7 | LOW | M04, M04b | The "Payment methods" page has no callout ("Card payments use 3-D Secure…"), which M03, M05, M06, M10, M10b, M10c, M11, M11b and M11c all render after the list. This predates round 3, but the G3 fix now shows the rest of the page, so the gap is exposed (DOM: list → "/" line → H2 "Fees and limits"). | Ruling r2 "Everything else … one consistent world". | Insert the shared callout block after the "/" line (before "Fees and limits") in M04 and M04b. It sits mostly under the menus; its right part shows at x719–1089. |
| N8 | LOW | Spec part 2 §3.2 | "the 4 px thumb (… 6 px from the right edge)" can be read as a 6 px gap. Kit `scroll_thumb` puts the thumb's left edge at `band.right() − 6` with width 4, leaving a 2 px gap: measured 713–717 in a 399–719 panel. The README's M04b-02 row says "1 px from the right edge" (measured from the inner border edge), a third way of saying it. | menu.rs:576-580. | §3.2: "thumb 4 wide, its left edge 6 px inside the panel's right edge (kit `scroll_thumb`)". README M04b-02 row: same words. |
| N9 | LOW | README "Needs owner 2" | It says phase-02 "still says Delete to Trash … not edited". Commit c221d9d5 edited it to "Delete page (moves to Studio's trash)". | `git log` on phase-02. | Delete "Needs owner" item 2. |

### M06b full critique (new screen, Operate mode)
- Job: show where the add-column "+" lives once the table is scrolled to its end (ruling S4). Focal point: the "+" next to the closed right corners. It reads correctly at 100 %.
- Geometry measured: offset 264, table 135–1057, "+" 1065–1089 at y211–235 centred on the 36 px header, thumb 590–1089 flush right. The add-row "+" stays at x399 without scrolling, and "On this page" highlights "Fees and limits". Exactly one control (Add column) has the hover look; Add row has the rest look. Both follow §4.
- The left sliver (x399–446) is the tail of the Fee column. Its header and values are scrolled out, so it is blank. That is true scroll behaviour, and it reads as scrollable together with the thumb.
- Issue: N1 (the ellipsized Notes cell, with no rule behind it).
- Nielsen, applicable 8 of 10 (9 error recovery and 10 help are n/a for a static scroll state): status 3 (thumb at the end, TOC active) · real world 3 · control 3 (thumb draggable; no left-edge fade, which the spec accepts) · consistency 3 (same frame and icon-button looks as M06; the ellipsis appears nowhere else) · error prevention 3 · recognition 3 (the sliver column has no visible header, which is inherent to scrolling) · efficiency 3 · minimalism 3. Total **24/32**.
- Cognitive-load pass, 8 items: 0 failures (one focus, ≤ 4 visible controls, no decision overload).

## 3. Spec ↔ mockup on every value the round-3 diff changed

| # | Spec (changed text) | Mockup measured | Match |
|---|---|---|---|
| 1 | P1 tab strip: Home 72×24 at y4, project tabs 26 at y3, + 24 at y4 | 8,4 72×24 · 92,3 240×26 · 580,4 24×24 · divider y8 | yes |
| 2 | P1 count pill: kit `count_badge` neutral, 22, min 22, pad 6, full radius, Inter 12/18 500 | 22×22, pad 6, r11, `#171717`/`#404040`/`#D4D4D4`, Inter 12/18/500 | yes (l.36 old name: N6) |
| 3 | §1 + Folder 66×24, 8 inside | 66×24 at card right − 8 | yes |
| 4 | §1 Folders heading 28 below last row | 555 → 583 | yes |
| 5 | §1 Open project centred on name line, 8 inside | cy 319 = 319; right 548 | yes |
| 6 | §2 placeholder at x399, caret over first letter | text 399, caret 399 | yes |
| 7 | §3.1 pair spans 60–12 left of column; fill 8 left; 4 px clear | 339–387; fill 391; gap 4 | yes |
| 8 | §3.2 cap 344 = 8 + 328 + 8; cuts Code at half; thumb 206.9 "6 px from right edge"; end offset 192 | 344; clip 328; Code 16/32; thumb 206.9 at 719 − 6; end thumb bottom = band bottom | yes (wording: N8) |
| 9 | §3.3 Turn into 14/20 600 | 14/24/600 | **partial** (N5, invisible) |
| 10 | §3.3 link result rows 32, 12/18 500 | 32, 12/18/500 | yes |
| 11 | §4 revealed "+" rest look; one hover at most | M06 rest ×2; M06b one hover (Add column) | yes |
| 12 | §4 add column 8 px right of last column, scrolls with table, out of view at start | M06 x1329 clipped; M06b 1057 + 8 = 1065, flush 1089 | yes |
| 13 | §4 track 690×11, thumb 7, length 690 × 688 ÷ W | track 690×11, thumb 7, length 499 (= 690²/954) | **NO** (N4, 1.5 px) |
| 14 | §4 frame 1 px `BORDER` r12 scrolls with cells; Limit 109 of 150 | `.tbl` 1 px `#262626` r12; Limit 980–1089 = 109 | yes |
| 15 | §5 file row label +44, pages icon +40 / label +60, All files glyph +24 / label +44 | 348/364/344/328/348 with row x304 | yes |
| 16 | §5 pill = "count of the rows listed under the file (all frames Pin/Embed; unpinned Suggest)" | M07 6 (4 page rows listed), M08 1 / 3 / 4 | **ambiguous** (N3) |
| 17 | §10 KPI value row exactly 30 | 4 × 30, labels y224 | yes |
| 18 | §12 "“<title>” moves to the trash.", 20-tall lines, 8 between, 8 before buttons, 440×180 | same string; 396/424; 452; 440×180 | yes |
| 19 | §15 copy deck "trash." / phase-02 "moves to Studio's trash" | M13b string matches; Home "Move to trash", sidebar label "Trash" | yes |

## 4. Already excellent
- All 26 fixes hold to the pixel. The fixer's README before/after table matched every value I re-measured.
- The S4 table model is coherent: the frame lives on the table and scrolls with it, the cut column is cut mid-column, and the "+" belongs to the last column. M06 and M06b together show both ends unambiguously.
- The S1 cap now cuts exactly half a row, with kit-exact thumb placement (menu.rs:576-580).
- The committed PNGs are byte-for-byte what the HTML renders, so engineers working from PNGs or HTML see the same thing.

## Unresolved questions
1. N1: how wide is a table column, and what happens to text longer than that (fit to content with a cap plus ellipsis, fixed width plus ellipsis, or no cap)? This needs an owner or controller ruling before an engineer builds tables.
2. N2: split M04b into two captures, or keep the composite and caption it (README "Needs owner 3")?

Status: DONE_WITH_CONCERNS
Summary: All 22 screens were rendered at 1440×900 @2x and measured. All 26 round-2 findings are VERIFIED, and no regression was found. There are 3 new MED (M06b column-width/overflow rule missing, M04b composite capture collision, self-contradictory S3 wording in spec §5) and 6 LOW, so the verdict is NEEDS CHANGES.
Concerns/Blockers: N1 needs a ruling on column width; motion and responsive behaviour remain unverifiable from static captures.
