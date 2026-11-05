# Critique r2 — Project docs workspace mockups (21 screens)

Reviewer: independent (did not build; report only). Worktree `plan/project-docs-workspace` @ b91395f3. Date 2026-11-02.
Method: headless Chrome 1440×900 @2x captures of all 21 mockups (+ index), M09-full at 1440×2630; computed boxes via Playwright (`measure.cjs`, 1x CSS px); zoom crops; kit source read for every chrome value cited. Captures: `scratchpad/review-r2/cap/*.png` (2x), `v/*.png` (1x), crops `z/*.png`. Gate JSON: `gate-*.json`; slop-detect: `slop.txt`.

## Not verified (read first)
- Motion (§14 timings, flash envelope, crossfade): static mockups, nothing to measure.
- Drag ghost offset (+12, +8): no pointer drawn; only the internal consistency (ghost centre vs drop line) checked.
- Responsive (§11) captures: skipped by owner default (none exist to check).
- Hover/focus states that no mockup draws (tree-row keyboard focus, menu-row keyboard focus) — only CSS read.
- Accessible names: sampled via `aria-label` in markup, not run through a screen reader.
- README "Token citations" line numbers: spot-checked ~10, not all.

## Verdict
**NEEDS CHANGES** — 1 HIGH, 11 MED, 14 LOW.
Weakest link: the spec text (part 1 count pill) still contradicts the kit-faithful mockups; the R0 "spec corrected in the same change" promise missed it.

Deterministic lenses: `ui gate` exit 1 on all 21 — the only errors are `#737373` contrast (4.18:1 on `BG`, 3.19:1 for the selected-tab close glyph on `BG_SELECTED`, which is exactly kit `close_look_over(Selected)` button.rs:118-120). Both are the settled addendum item 1 (kit token) → not raised. `slop-detect`: 210 hits = cramped-padding 181 (kit row geometry, false positive), clipped-overflow 19 (1440×900 artboard clip), overused-font 9 (Inter/Roboto Mono are the DS fonts), gray-on-color 1 (`TEXT_SECONDARY` on the `BG_SELECTED_AREA` block tint ≈ 11:1, false positive). No layout/taste/content errors.

## Findings

| ID | Sev | Screen | Measured evidence | Kit / spec citation | Exact fix |
|---|---|---|---|---|---|
| G1 | HIGH | spec part 1 vs M02–M13 tree, M07/M08 picker, M08 header | Every count pill renders 22×22, fill `#171717`, 1 px `#404040`, Inter 12/500, radius 11 (M03 `.g .pill` [253,142 22×22]). Spec part 1 l.40 says "`BG_TERTIARY` fill, `TEXT_SECONDARY` mono 12, 16 tall". An engineer working from the spec builds a visibly different pill on every group row. | Mockup = kit `count_badge` / `count_badge_look(Neutral)` badge.rs:31-33, 75-79 (`BADGE_NEUTRAL_*`, text-xs Medium, full radius). R0: spec must be corrected to the kit. | Spec part 1 l.40 → "Count pill: kit `count_badge` neutral (`BADGE_NEUTRAL_BG` fill, 1 px `BADGE_NEUTRAL_BORDER`, `BADGE_NEUTRAL_TEXT` 12/18 500 Inter), `BADGE_SM` 22 tall, full radius." Leave mockups as is. (The agent-chip count pill, §9, is a different component and correctly stays 16 / `BG_TERTIARY` / mono.) |
| G2 | MED | M07, M08 picker left list | M07: file row icon x332, label x356 (icon→label gap 8); page rows icon x344, label x364 → depth indent 12 (icon) / 8 (label), not 16. M08: "All files" (no chevron) icon x328, label x352 vs file rows label x356 → labels 4 px out of column (crop `z/m08-picker-list.png`). Doc tree uses chevron 24 + icon 16 + 4 + label and +16 per depth (M03: Payment icon 80 / Card declined icon 96). | Spec part 2 §5 "kit row look… pages expand… as plain depth-1 rows (indent 16)"; spec part 1 item row "16 px icon + `SPACE_FACT` 4 + label"; `LAYER_INDENT` 16 theme.rs:545. | `shell.css` l.288-297: `.pk-file { gap: 4px }`, delete `.pk-file .ch + .i { margin-left:-4px }`, `.pk-file.nochev { padding-left: 24px }` (keep) → all labels at row+44; `.pk-page { margin-left:16px; padding-left:24px; gap:4px }` (keep) → icon +16, label +16. |
| M05-1 | MED | M05 link popover | Result rows "Cart rules / Error handling / Payment flow": font 14px/24px 500 (`.menu .mi` [596.8,310 304×32]); every other menu row in the set is 12/18 500. The popover inherits `.col` paragraph type. | Spec part 2 "Menu / popover … label 12/18 Medium (`font_menu`)"; kit `menu_item_look` rows.rs:47-53. | `.col .menu { font-size: var(--fs-xs); line-height: var(--lh-xs); }` (or move the popover out of `.col`). |
| M11-1 | MED | M11 agent name tag | Tag "Claude" [554.8,354 63×24] font 14px/24 500 — same size as the list text it labels; spec and canvas use 12. | Spec part 2 §9 l.120 "12/18 500 `AGENT_LABEL_TEXT`"; canvas `agent_overlay.rs:146` `font_caption_medium()` = 12 (theme.rs:266, 321). | `.atag { font-size: var(--fs-xs); line-height: var(--lh-xs); }` (height stays `BADGE_MD` 24). |
| M10-1 | MED | M10, M10c banners | Warning/danger icon box y98.5–114.5 (centre 106.5); text line 99–119 and buttons 93–125 centre on 109 → icon sits 2.5 px high (visible at 2x, crop `z/m10-banner-icon.png`). Icon wrapper is 16×21 (inline SVG baseline box). Callout, state panels and embed headers are correctly centred. | P10 / optical centring; spec §8 "WARNING icon + text". | `.banner > .ico { display:grid; place-items:center; height:20px; align-self:center }` → icon top 101, centre 109. |
| G3 | MED | M04, M04b, M10, M10b, M10c ("Payment methods") | Page renders 1 H2 ("Accepted methods") then blank to y900 (M10b: 463 px empty), while "On this page" lists 3 headings; the same page in M03/M06/M11/M11b/M11c renders all 3 H2 + table + checklist. | Ruling sample-data rule "one sample world"; part 1 "On this page" lists the page's headings. | Append the shared "Fees and limits" table section and "Before release" checklist (as in M11b) after the callout in these 5 files. |
| M04b-1 | MED | M04b slash menu "Top" | Clip 402–746; last row "Code" ends exactly at 746, "Design" label starts at 746 → nothing partially visible; only the thumb signals more. R1 M04b-02 asked for a cut row. | Spec part 2 §3.2 "max height 360 with scroll (the cap cuts a row, the scroll thumb shows more below)". | With fixed 360 / 8 padding / 24 + 10×32 rows the cap always lands on a boundary, so the mockup cannot cut a row without breaking another value. Fix the spec instead: §3.2 "the cap ends on a row boundary; the scroll thumb shows more below" (see Q1). If a cut row is wanted, the cap value itself must change (owner call). |
| M06-1 | MED | M06 vs M03 | Revealed controls drawn in **hover** look in M06: row handle [367,283], add-row "+" [399,377], add-column "+" [1097,211] all `BG_HOVER` #171717 + `#A3A3A3`, three at once. M03 revealed "+" handle is **rest** look (no fill, `#737373`). Only one control can be hovered. R1 G14 not fully fixed. | Spec part 2 "Icon buttons (`Hierarchy::Icon`): rest `ICON_QUATERNARY`, hover `BG_HOVER` + `ICON_QUATERNARY_HOVER`"; §4 "Hovering the table shows…". | M06: rest look on the three revealed controls (`.ibtn` without `.hov`); keep at most one `.hov` where the pointer is. |
| M06-2 | MED | M06 (+ M03, M11, M11b, M11c table) | `.tbl` is 960 wide in a 690 viewport: "Limit" column 980–1100 clipped by 11 px at 1089, a sixth column "Notes" (1100–1360) never visible; the add-column "+" sits at x1097 = 8 px right of the **viewport**, not of the last column. The rounded right edge makes the clipped table read as complete. | Spec part 2 §4 "add column … 8 px right of the last column"; "Wide tables scroll horizontally inside the 690 column". | Either (a) state in spec §4 "the add-column + pins 8 px right of the scroll viewport while the last column is scrolled out", or (b) draw M06 scrolled to the end (Notes visible, thumb at right) so "+" is truly 8 px right of the last column. Also clip at a column boundary in the scrolled-to-start state or keep it — but not 11 px into "Limit". |
| M09-1 | MED | M09 / M09-full | "Order confirmed · Checkout-web" shows **Frame not found**, but M07 shows that frame exists and is Pinned, and the tree lists it under Screens. "Cart · Checkout-mobile" = **File not found** while the next card "Payment · Checkout-mobile" = **Can't render** (file opened) on the same page; tree header still "2 files". | Ruling "One sample world across all screens". | Frame-not-found card → a frame outside the inventory (e.g. "Address · Checkout-web", deleted). File-not-found card → a file outside the project (e.g. "Cart · Checkout-legacy"). |
| M08-1 | MED | M07 vs M08 picker | Same row "Checkout-web" shows pill **6** in M07 (all frames) and **1** in M08 (unpinned frames). Spec defines only the "All files" count as "frames not pinned yet"; file rows are "frame count pill". | Spec part 2 §5 "A file row = … name + frame count pill"; "Suggest mode adds All files … count = frames not pinned yet". | Pick one and write it in §5: recommended "in Suggest mode every count = frames not pinned yet" (mockup unchanged), or show 6 / 3 in M08. |
| M03-1 | MED | M03 (block selected) | Pressed drag handle [367,234 24×24] ends at x391; selected-block fill `.blk` starts at x391 → 0 px gap, the grey handle and blue fill read as one shape (crop `z/m03-handle-fill.png`). | Spec part 2 §3.1 handle "32 px left of the column" (ambiguous) + fill "8 px horizontal inset". | Move the handle pair left 4 (`SPACE_FACT`): "+" x339, ⋮⋮ x363–387; write "handle pair ends 12 px left of the column" in §3.1. |
| M13b-1 | LOW | M13b | Modal 440×192, top 336 → centre 18 px above window centre; product `home/dialog.rs` would size it 180 (24+24+12+2×28+40+24) and place buttons at y452, not 464 (body→buttons gap 20 vs 8). | `home/dialog.rs:254-265` (`h`, `from_center_size(screen.center() − 24)`); spec "centred 24 px above". | Reduce gap after last body line to 8 (buttons top 452, modal height 180, top 336). |
| M04-1 | LOW | M04 "No blocks match" | Disabled row text `#A3A3A3`. | Kit disabled menu row = `TEXT_SECONDARY` at half alpha (rows.rs:50 + `disabled()` mod.rs:153). | `color: var(--text-secondary); opacity:.5` on the label (≈ `TEXT_DISABLED`). |
| M02-1 | LOW | M02 "Clear search" | Tertiary text button 24 tall, label 12px/600. | Kit `button`: "Every size uses text-sm semibold" button.rs:181-188 (14/20 600); spec "label 14/20 600 at every size". | `.tbtn { font-size: var(--fs-sm); line-height: var(--lh-sm) }`. |
| M05-2 | LOW | M05 toolbar | "Turn into" label 14px/500. | Kit button labels 600 (button.rs:188). | `font-weight: var(--w-strong)`. |
| M12-1 | LOW | M12 KPI | Labels at y225 in tiles 1–3, y224 in tile 4 ("4/5" row is 30 tall, "24·11" rows 31 tall because the Inter separator span is taller). | Spec §10 value 20/30. | `.v { height:30px; display:flex; align-items:baseline }` or `.sep { line-height:30px; vertical-align:top }`. |
| G4 | LOW | all shell screens | Home tab y3.5, project tabs y2.5 (half-pixel; blurry at 1x). | `tabs.rs:983` (top+4), `tabs.rs:1027` (top+3). | `.tabstrip { align-items:flex-start }`, `.tab.home { margin-top:4px }`, `.tab.proj { margin-top:3px }`, `.tab.plus` top 4. |
| M01-1 | LOW | M01 | "+ Folder" 73.5×24 at x474.5. | `home/mod.rs:1834` `plus_r` = 66×24 at right−74. | `width:66px; padding:0` centred label. |
| M01-2 | LOW | M01 | "Folders" heading y567 (12 below last project card). Product adds the row gap too: card bottom 555 + 16 + 12 → 583. | `home/mod.rs:1787` `proj_rows_h` includes `pgap_y`; `:1929` `+12`. | Folders section top +16 (583). |
| M01-3 | LOW | M01 | Hover "Open project" centred y327 (on name+meta block); name row centre is 319. | Spec part 2 §1 "at the right end of the name row". | Either centre on 319 or change spec wording to "right of the name and meta lines". |
| M13b-2 | LOW | M13b, spec §12/§15 | "moves to the Trash." (capital T). Product copy: "Move to trash", "Empty the trash?", "until you empty the trash" (home/mod.rs:561, :840; main_view.rs:184). | ux-writing rule 9 (product vocabulary), rule 11. | "moves to the trash." — unless the page goes to the macOS Trash (then keep; see Q2). |
| M02-2 | LOW | M02 | Placeholder "Type / for blocks" starts x402, text column x399 (caret drawn at 399). | Part 1 reading column x399. | Placeholder at x399, caret overlaid. |
| M13-1 | LOW | M13 | One capture shows three mutually exclusive states: drag in progress + context menu open + rename field. | Spec §9 "One state per capture" (agent); same logic. | Note "composite specimen" in the caption, or split. |
| G5 | LOW | shell.css l.92 | `.r.foc` draws the ring outside (offset 2); no mockup shows tree keyboard focus. | Spec part 1 "keyboard focus: the kit 2 px `FOCUS_RING` band drawn inside the row". | `outline-offset:-2px`; add one focused row specimen (e.g. M13). |
| M07-1 | LOW | M07/M08 footer | One row mixes 32 (segmented), 36 (dropdown), 40 (buttons). | Kit sizes each control; no rule broken. | Optional: dropdown 32 (`CONTROL_XS`) to match the segmented control. Pedantic. |

## Round-1 fix verification (15)

| R1 ID | Check (measured now) | Result |
|---|---|---|
| G01 secondary button | M10 "Load their version": fill `#0A0A0A`, 1 px `#404040`, `#D4D4D4` 14/600, r8, 32 tall | VERIFIED |
| G02 destructive | M13b "Delete page": `#DC2626` + `#FFFFFF` | VERIFIED |
| G03 modal | M13b scrim `rgba(38,38,38,.70)`; frame `BG`, no border, r16, inset 24 | VERIFIED |
| G04 tooltip | M05/M12 `.tip`: `#262626`, no border, r8, padding 8/12, `TEXT` 12/18 | VERIFIED |
| G05 toast | M12: `#171717`, 1 px `#262626`, r12, 40 tall, bottom 72, dot centre x+16, text x+28, right 16, 14/500 | VERIFIED |
| G06 fields | M05 link field focused 2 px `#FB923C`, placeholder `#737373`; existing-link field 1 px `#404040`; M07 dropdown no border at rest | VERIFIED |
| G07 checkbox | M06 16×16 r4; checked `#E36323` fill + `#0A0A0A` check; unchecked 1 px `#404040` | VERIFIED |
| G10 danger text | M03 "Delete": label `#F87171`, icon `#EF4444` | VERIFIED |
| G14 same control, same look | M03 revealed "+" rest look vs M06 revealed handles/"+" hover look | NOT FIXED (partial) → M06-1 |
| M04b-02 scroll affordance | Thumb present (4 px `#A3A3A3`, 6 from panel edge = menu.rs:580); no row cut | NOT FIXED (partial) → M04b-1 |
| M07-01 pinned badges | Cart, Payment, Card declined, Processing, Order confirmed carry "Pinned" (status_tag 22, r8, neutral triple) | VERIFIED |
| M09-02 state buttons | Relink frame / Try again 32 tall, 14/600, stack 17/17 in 120 panel | VERIFIED |
| M11b-01 canvas chip | Pill 32, inset 8, count 16 `#262626` Roboto Mono, gaps 4, chevron 12; width 137.1 = formula in agent_changes_chip.rs:415-431 | VERIFIED (pixel-exact) |
| M13b-02 initial focus | Ring on Cancel: 2 px `#F97316`, offset 2 (= focus.rs:24-40 band 2–4 px outside); Delete has none | VERIFIED |
| M10-01 saves held | M10 header "2 files" while banner open; "Saving…" only in M10b | VERIFIED |

Also re-checked and holding: M07-03 (rows contiguous 32), M07-04 (column tops both y227), M07-05 (name 8 below thumb), M06-02 (column handle inside header, right inset 8; menu 4 below), M12-03 (Inter separator, 4 px each side), M02-01 (placeholder label x100), M03-05 (embed header icon x416 = body inset), G16 (" · " one text run), G13 offsets (block menu +4, slash +4, link popover +4, toolbar −8 — all measured).

## Spec ↔ mockup (concrete values)

| # | Spec value | Mockup measured | Match |
|---|---|---|---|
| 1 | Part 1 count pill 16 tall, `BG_TERTIARY`, mono | 22, `#171717` + `#404040` border, Inter 500 | **NO** (G1; mockup = kit) |
| 2 | §9 agent tag 12/18 500 | 14/24 500 | **NO** (M11-1) |
| 3 | Menu/popover label 12/18 | M05 results 14/24 | **NO** (M05-1) |
| 4 | §3.2 slash cap cuts a row | ends on row boundary | **NO** (M04b-1) |
| 5 | §4 add column 8 px right of last column | 8 px right of viewport; last column hidden | **NO** (M06-2) |
| 6 | §5 file row "frame count pill" | M08 shows unpinned count (1) | **NO / ambiguous** (M08-1) |
| 7 | Modal confirm "centred 24 px above window centre", 440 | 440 ✓, centre 18 above (height 192 vs product 180) | partial (M13b-1) |
| 8 | Tree header 65; name y+16, sub y+40, "…" y+12 | 65; 48/72/44 (tree top 32) | yes |
| 9 | Panel title row 40, icon buttons 24 gap 8, right pad 8 | 40; x191/223/255; 279→287 | yes |
| 10 | Item row 32, inset 8, r8, icon 16 + 4 + label 12/18 | 223×32 at x56, icon 80, label 100 | yes |
| 11 | Picker 880×600, head 60, footer 76, left 240 pad 16/24 | 880×600 at 280,150; 60; 76; 240, pad 16 16 16 24 | yes |
| 12 | Picker tile thumb 144, name 8 below, Pinned tag 22 r8 | 144; +8; 57.6×22 r8 | yes |
| 13 | Embed header 36, inset 0 8 0 16, `BG_TERTIARY` 60 % | 36, padding 0 8 0 16, rgba(38,38,38,.6) | yes |
| 14 | Selection toolbar 8 above, r12, inset 4, buttons 32 r8 | 222→230 = 8; r12; 3+1; 32 r8 | yes |
| 15 | Slash menu 320 wide, max 360, 4 under caret line, hints mono `TEXT_QUATERNARY` | 320; 360; 390→394; Roboto Mono `#A3A3A3` | yes |
| 16 | Banner 24 above title, inset 12 16, 32 buttons | 138→162; 12/16; 32 | yes |
| 17 | KPI 24 below description, 4 tiles gap 16, value 20/30 600 | 154→178; 160.5 + 16; 20/30 600 | yes |
| 18 | Toast bottom 72, 40 tall, min 240 | 828 → 72; 40; 276 | yes |
| 19 | Rename field 24 tall, 4 inset, right edge 8 inside row | 685–709; 279−271 = 8 | yes |
| 20 | Off-screen chip 32, count 16 `BG_TERTIARY` mono | 32; 16 `#262626` Roboto Mono | yes |
| 21 | Copy-deck strings (block menu, tree menu, picker, embed states, save, delete) | all strings seen match §15 verbatim (checked in renders) | yes |

## Already excellent
- Kit fidelity of the shared chrome is now real, not claimed: secondary/primary/destructive buttons, modal surface, tooltip, toast, fields, checkbox, segmented, status tag, menu rows (12/18 500, x16 icon / x40 label; check x12 / icon x32 / label x60), submenu `SUBMENU_OVERLAP` 0 — all measured equal to kit source.
- Agent off-screen chip is pixel-exact to `agent_changes_chip.rs` (width formula reproduces 137.1 px).
- App rail geometry equals `panels/rail.rs` (tile left 7.5 on the 47 px inner width, tops 56 + 56·i + 12).
- Home project card equals `home/mod.rs:1784-1890` (260×168, gap 16, preview 92 inset 8, 22 px tile, name/meta y and colours, section heading subtitle x+10).
- Overlay offsets are consistent everywhere (4 below anchors, toolbar 8 above).
- Focus ring band (2 px, 2–4 px outside) and in-cell inset band match `focus.rs`; initial focus on Cancel.
- Reading-page type scale and vertical rhythm (H1 → desc 12, desc → first block 32, H2 32 above, banner 24 above title) all on the ladder.
- One sample world mostly holds (picker inventory, KPI 3 · 24, pinned states, file counts).
- Token discipline: no colour literal outside `tokens.css`; Phosphor glyphs; frame glyph (not external-link) on in-app opens; external-link only on "Open" in the link popover.

## Unresolved questions
1. M04b-1: should spec §3.2 drop "the cap cuts a row" (current mockup ends on a row boundary), or must the mockup show a partial row? (Owner/spec call.)
2. M13b-2: does "Delete page" move the Markdown file to the macOS Trash (capital T is correct) or to Studio's own trash (lowercase, matching Home)?
3. M08-1: in Suggest mode, should per-file pills show unpinned counts (current mockup) or total frame counts (spec text)?
4. M06-2: when a wide table is scrolled to the start, does the add-column "+" pin to the viewport edge or stay with the last column (off-screen)?

Status: DONE_WITH_CONCERNS
Summary: All 21 mockups rendered and measured; shared chrome now matches the kit, 13 of 15 round-1 fixes verified; 1 HIGH (stale spec count pill), 11 MED, 14 LOW remain → NEEDS CHANGES.
Concerns/Blockers: Motion, drag-ghost offset and responsive behaviour cannot be verified from static captures; four spec ambiguities above need a controller/owner ruling.
