# Critique r1: project docs workspace mockups (M01–M13b + index)

Independent critic, report-only. Generator ≠ grader: no mockup or spec was edited.
Date 2026-11-02. Inputs: `plans/261102-project-docs-workspace/ui-mockups/` (16 PNG + m09-full + index, HTML, tokens.css, shell.css, README), spec part 1 + part 2, `origin/main` `design/principles.md`, `design/ux-writing.md`, `theme.rs`, widget kit `crates/designos-app/src/widgets/kit/*.rs`, `canvas/agent_changes_chip.rs`, `shell.rs` toast, design-studio refs `ds-*.jpg`, Studio captures `docs/images/designos-home.png` (v0.7.5) and `designos-agent-spec.png`.

## Not verified (read first)
- egui rendering of any of this; only HTML at 1x/2x.
- Hover/focus/motion states that are not drawn; 375/768 widths (owner default SKIPPED, Operate criterion 7 = n/a on every screen).
- Studio's current look is taken from (a) the widget kit source on `origin/main` (exact tokens) and (b) two 2x screenshots (sizes estimated ±2 px). Studio's current Home card gap cannot be measured (the capture has one card).
- Icon glyph fidelity against `icons.rs` was not compared glyph by glyph.
- ui gate / slop-detect re-run on M03 and M07 only (results identical in kind to README's 18-file run); other files rely on README's run.

**Weakest link:** where the approved spec and the `origin/main` widget kit disagree (secondary/destructive button, modal scrim, field, checkbox, menu text), I call the kit "Studio's existing look" and flag the spec (SPEC). That needs an owner/controller ruling, see questions.

## Method
- Measured with Playwright (`getBoundingClientRect` + computed style) on all 18 files at DSF 2: `<scratch>/critique-r1/measure.json`, 2x captures `2x-*.png`, zoom crops `z-*.png` (scratch = `/private/tmp/claude-501/-Users-jang-Products-design-os-studio/f1428fa3-d2ec-4abe-b46d-db13f0015fd5/scratchpad/critique-r1/`).
- Contrast computed (WCAG relative luminance) for every doubtful pair; all text pairs pass AA, lowest = `DANGER` on `BG` 5.26:1.
- Lens B (deterministic): `ui gate` M03/M07 exit 1, **0 errors**, token-coverage 20 % FAIL (justified in README: mockup token names not in `design/`); slop-detect exit 2: `cramped-padding` (false positive, fixed-height flex rows), `clipped-overflow-container` (artboard crop, false positive), `overused-font` Inter (false positive, P5), `repeated-container-text` "Payment" ×5 in M07 (**real**, see M07-09).
- Operate rubric (taste-duel): criteria 1 and 3 n/a except M12 (overview); 7 n/a (owner skip). Applicable max 10 (M12: 14). Pass = ≥ 75 % and no 0 on 4 or 8.

---

## Cross-screen (fix once in shell.css / tokens.css)

| ID | Sev | What is wrong (measured) | Fix (token terms) | SPEC |
|---|---|---|---|---|
| G01 | HIGH | Secondary button `.btn.sec` = `BG_TERTIARY` fill, no border, `TEXT` label. Studio kit `button_look(Secondary)`: fill `BG`, 1 px `BORDER_STRONG`, label `TEXT_SECONDARY`, icon `ICON_QUATERNARY`; hover `BG_HOVER` + `TEXT_SECONDARY_HOVER`; pressed `BG_PRESSED`. P2 names `BORDER_STRONG` as the secondary-button boundary. Seen on M01 "Open project", M07/M08 Cancel, M10 "Load their version", M13b Cancel, index Prev/Next. | `.btn.sec { background: var(--bg); border: 1px solid var(--border-strong); color: var(--text-secondary); }` add `--icon-quaternary #737373`, `--text-secondary-hover #E5E5E5`. | Yes: part 2 "secondary = `BG_TERTIARY`" → kit secondary |
| G02 | HIGH | Destructive button = `DANGER` #EF4444 + `TEXT_ON_BRAND`. Kit `button_look(Destructive)` = `BG_ERROR_SOLID` #DC2626 + `TEXT_WHITE` (4.83:1), hover `BG_ERROR_SOLID_HOVER`, pressed `BG_ERROR_SOLID_PRESSED`; P2: "White text sits only on `BG_ERROR_SOLID`". README ambiguity 4 picked the wrong pair. M13b. | `.btn.dng { background: var(--bg-error-solid); color: var(--text-white); }` add both tokens. | Yes: part 2 §12 "`DANGER` bg" |
| G03 | HIGH | Modal = 1 px `BORDER_MENU` + scrim black 60 %. Kit `modal_surface`: scrim `BG_OVERLAY` (#262626 @ 0xB3), frame `BG`, **no border**, radius 16. Measured: mockup modal fill vs black-60 scrim 1.14:1, so the panel edge exists only through the border; Studio's grey scrim makes the `BG` panel read as the darker plane. M07, M08, M13b. | `--scrim: rgba(38,38,38,.70)` (`BG_OVERLAY`); `.modal { border: 0; background: var(--bg); }`. | Yes: part 2 "scrim black 60 %" |
| G04 | HIGH | Tooltip = `BG_MENU` + 1 px `BORDER_MENU` (M12 `.tip` bg rgb(10,10,10), border #262626). P2 + kit `tooltip_look`: `TEXT` on `BG_PRIMARY_SOLID` #262626, no border. | `.tip { background: var(--bg-primary-solid); border: 0; }` (radius 8 stays). | README ambiguity 15 reversed |
| G05 | HIGH | Toast ≠ "Studio's existing toast". Kit `toast_surface` + `shell.rs`: fill `BG_PRIMARY_ALT` #171717, 1 px `BORDER_MENU`, radius 12, 8 px status dot `ICON_BRAND_PRIMARY` at `DOT_INSET` 16, text x+28 (`PAD_LEFT`), right 16, 14 **Medium** `TEXT`, min width 240. Mockup: `BG_TERTIARY`, no border, no dot, padding 16/16, 14 Regular; hint split into a second span (gap 8 + leading space → "AI  · paste"). M12. | Rebuild `.toast` from those values; one text run "Copied for AI · paste into the agent" (hint in `TEXT_TERTIARY` allowed, but no flex gap). | Name `kit::toast_surface` in part 2 |
| G06 | HIGH | Text fields / search / select drawn `BG_SECONDARY` + 1 px `BORDER` (#262626) at rest, focus = 1 px `FOCUS_RING` border, placeholder `TEXT_TERTIARY` #A3A3A3. Kit `field_look`: rest `BG` + 1 px `BORDER_STRONG`; focused `BORDER_SELECTED` #FB923C **2 px**; P2: placeholder = `TEXT_PLACEHOLDER` #737373 (4.18:1 on `BG`). `dropdown_look`: `BG`, no border at rest, `BORDER` on hover. Seen: M05 link field, M07/M08 "Search frames", M07 "Under screen" select, M13 search + rename row, M01 Home search. | `.field,.sfield,.hs-search { background: var(--bg); border-color: var(--border-strong); }` `.field.foc,.sfield { border: 2px solid var(--border-selected); }` `.field .ph { color: var(--text-placeholder); }` `.dd { background: var(--bg); border-color: transparent; }` | Yes: part 2 focus-ring line vs fields |
| G07 | HIGH | Checkbox is not "Studio's checkbox widget" (part 1 checklist row). Kit `checkbox`: 16 box, radius 4 (`SPACE_FACT`), unchecked transparent + 1 px `BORDER_STRONG`, checked `BG_BRAND` fill + `TEXT_ON_BRAND` check. Mockup radius 2 (`RADIUS_MARK`), checked `TEXT` #FAFAFA fill. M03, M11, M11b; M08 multi-select check circles copy the same white fill. | `.chk .box { border-radius: 4px; } .chk .box.on { background: var(--bg-brand); border-color: var(--bg-brand); color: var(--text-on-brand); }` same for `.pk-tile .cc.on`. | Needs owner ruling: orange checks add an accent beyond "one accent per view" (Q1) |
| G08 | MED | Menu rows: label `TEXT` #FAFAFA, icons `ICON_SECONDARY`. Kit `menu_panel_look`/`menu_item_look`: label `TEXT_SECONDARY`, icon `ICON_QUATERNARY`; hover `BG_HOVER` + `TEXT_SECONDARY_HOVER` + `ICON_QUATERNARY_HOVER`. All menus (M01, M03, M04, M04b, M05 results, M06, M13). | `.mi { color: var(--text-secondary); } .mi .i { color: var(--icon-quaternary); } .mi.hl { color: var(--text-secondary-hover); }` | Part 2 menu line should name kit looks |
| G09 | MED | Rows, tabs, segmented differ from kit. Tree/list: kit `row_look` rest `TEXT_SECONDARY`/`ICON_QUATERNARY`, selected `BG_SELECTED` + `TEXT_SECONDARY_HOVER`; mockup selected `TEXT` 500, icons `ICON_TERTIARY`→`ICON`. Tabs: kit `tab_look` rest `TEXT_QUATERNARY`, selected `TEXT_SECONDARY`; mockup selected `TEXT`. Segmented: kit selected tile `BG_SELECTED` + 1 px `BORDER_STRONG` + `TEXT_SECONDARY`; mockup no border, `TEXT` (M07/M08). | Map `.r`, `.tab`, `.seg span.on` to the kit triples. | Part 1 states selected row `TEXT` 500 → decide (Q2) |
| G10 | HIGH | Danger **text** uses `DANGER` #EF4444 ("Delete", "Delete column", Home "Delete"). P2: `DANGER` = icons, `TEXT_ERROR` #F87171 = text (7.16:1 vs 5.26:1). M01, M03, M06, M13. | `.mi.dng { color: var(--text-error); } .mi.dng .i { color: var(--danger); }` | Yes: part 2 "Delete in `DANGER` text" |
| G11 | MED | Tree title-row icon buttons gap 4 (x199/227/255, 24 wide). Studio Pages header measures ≈12 (search centre x229, "+" centre x265 in designos-agent-spec.png); P3 peers in a bar = `SPACE_INLINE` 8. Header "…" right edge 275 vs title-row "+" right edge 279 (4 px off one vertical line). All tree screens. | `.tree-title .acts { gap: 8px }`; `.tree-head .more { right: 8px }` (`PANEL_ACTION_RIGHT_PAD`). | No |
| G12 | MED | Group count pills sit flush on the row's right edge (pill x257–279, row x56–279): on group hover/selected fill the pill kisses the rounded edge; vertical inset is 5. | `.g { padding-right: 4px }` (`SPACE_FACT`). | No |
| G13 | MED | Overlay offsets inconsistent: block menu 4 below handle (254→258), submenu 4 right, slash menu 4 below line box (390→394), inline toolbar 8 above line (222→230), link popover 8 below (254→262), M13 context menu not anchored at all. | One rule: popovers `SPACE_FACT` 4 from their anchor, the selection toolbar 8 (spec). | Yes: name the offset once in part 2 shared values |
| G14 | MED | Same control, different looks. "+" add: ghost `ICON_TERTIARY` (block handle, tree placeholder) vs filled `BG_TERTIARY` + `ICON` (table add row/col, column handle; M06 `.plus24` bg rgb(38,38,38)). Pressed icon button: `.handles .ibtn.on` = `BG_HOVER` #171717 (M03) vs `.ibtn.on` = `BG_SELECTED` #262626 (M09 Edit diagram, M13 search). Row handle transparent vs column handle filled (M06). | All hover-revealed icon buttons = kit `Icon` hierarchy (ghost, `ICON_QUATERNARY`, hover `BG_HOVER`); pressed = `BG_SELECTED` everywhere. | No |
| G15 | MED | ux-writing rule 13: the arrow-out-of-box glyph is reserved for links that leave the app. Used for in-app "Open in canvas" (embed header, M03/M09) and "Open" (tree menu, M13). Rule 1 (≤ 3 words, no prepositional tail): "Copy link to block", "Copy link to page" (4 words), "Open in canvas". | Glyph: an in-app open icon (e.g. maximize / arrow-up-right without box). Labels: "Copy block link", "Copy page link", visible "Open frame" + accessible name "Open frame in canvas". | Yes: §15 copy deck |
| G16 | MED | " · " separators built from flex gap + a leading space: embed header "Payment" ends x486.2, file span starts x494.2 (8 gap) then "· " → ~8 px before the dot, ~3.5 px after. Same in toast. | Render title + " · " + file name as adjacent spans with gap 0 (spaces both sides of the dot). | No |
| G17 | MED | Tab strip fidelity vs `origin/main`: Studio file tabs show a coloured kind tile (blue design tile in designos-agent-spec.png) and a 1 px divider after Home; mockup uses bare outline glyphs (book, #) and no divider. Part 1 asks for "the Project tile icon". | Tab icon = 16 px kind tile; divider after Home. | Yes: no `TILE_PROJECT_*` colour exists in theme.rs (M01 borrows `TILE_SPEC_*`) (Q3) |
| G18 | MED | App rail items are placeholders (menu, book, layers, sparkles, plus; README 18). Engineers rebuilding 1:1 will copy a fake rail (P6). Current rail (agent-spec.png): menu, pages, sparkles, plus, briefcase, separator, hexagon, components. | Copy the real rail, add only the Project tile. | No |
| G19 | MED | `AGENT_1` #FFA629 (caret, 1 px write flash, tree dot, tag) and `FOCUS_RING` #F97316 are both warm-orange 1 px outlines; M11's flash around the list reads as a keyboard focus ring. | Systemic (canvas palette, P12); options: thicker flash (2 px) or `AGENT_2..4` default for docs. | Yes (Q4) |

---

## Per screen

### M01 Home, open project — Operate 6/10 FAIL (2:1 · 4:1 · 5:2 · 6:1 · 8:1)
| ID | Sev | Observation | Fix | SPEC |
|---|---|---|---|---|
| M01-01 | HIGH | Home chrome copied from the stale v0.6.3 capture (`studio-home-main.png` shows "v0.6.3"), not `origin/main` (`designos-home.png`, v0.7.5). Measured: Import 64×32, radius 16, 12 px; "+ Design file" 32 tall pill; filter chips radius 16 12 px. Current Home: buttons ≈36 tall, radius 8, 14/600; sidebar rows ≈36 with 14 px labels; version badge + eye; "+ FigJam board". P4: "buttons, including Home and dialog buttons, use `RADIUS_CONTROL`"; P5: button labels 14 Semibold. | Rebuild M01 from designos-home.png; `.btn.pill` → `.btn` 36 radius 8 14/600; README ambiguity 17 source replaced. | No |
| M01-02 | HIGH | Card grid gap 24 (cards x296, 544, 792, 1040; w224). P3 `SPACE_BLOCK` 16 "cards in a grid, Home tiles"; P9 "SPACE_BLOCK between cards". | `--home-card-gap: 16px`. | No |
| M01-03 | MED | Hover "Open project" (x399.6 y175, 111×36) covers the top-right corner of the doc sketch (sketch x360–456, y176–296). | Keep the thumbnail art out of the top-right 36 + 8 zone (sketch 80×100 centred lower) or anchor the button bottom-right. | No |
| M01-04 | MED | Context menu floats at (380, 406), 37 px below the Checkout tile, tied to no tile or pointer. | Open at the pointer inside the tile (top-left = click point). | No |
| M01-05 | MED | Spec §1 third entry point (project file grid header "Open project" left of "+ Design file") not drawn. | Add a second specimen or note. | No |
| M01-06 | MED | Sidebar lists 2 projects (Checkout, Onboarding), grid shows 4 (Design system, Marketing site missing). | One project list. | No |
| M01-07 | MED | Project icons use `TILE_SPEC_BG`/#FB923C (spec-file colours): five orange glyphs plus the orange primary in one view. | Define a project tile colour. | Yes (Q3) |
| M01-08 | LOW | All four thumbnails are the same generic doc sketch; card title 12/600 vs current 14/600. | Vary (page count, first heading); 14/600 title. | No |
| M01-09 | LOW | Filter bar bottom y134 → cards y166 = 32; P3 says Home header to grid = `SPACE_REGION` 48. The current product also measures ≈32, so principle and product disagree. | Leave; flag to principles owner. | Yes |

### M02 First open, empty states — 8/10 pass (2:2 · 4:2 · 5:2 · 6:1 · 8:1)
| ID | Sev | Observation | Fix | SPEC |
|---|---|---|---|---|
| M02-01 | MED | Placeholder rows ("No spec pages" …) start at x80 (icon column) while item labels start at x100: the empty row reads as a misaligned item. | `padding-left: 44px` (24 + 16 icon + 4 `SPACE_FACT`) so text sits on the label column. | Yes: state the x |
| M02-02 | MED | Spec §2 search-no-result state ("No pages match" + **Clear search**) is drawn nowhere (`.nomatch` CSS exists, unused); M13 shows only matches. | Add a specimen (M02 or M13). | No |
| M02-03 | LOW | "No spec pages" row shows a `BG_HOVER` fill although spec says the row is not selectable; the fill invites a click that does nothing. | Hover reveals only the "+". | No |
| M02-04 | LOW | Header "2 files" counts design files while the tree holds pages: ambiguous next to a 1-page tree. | e.g. "2 design files" or keep. | Yes |

### M03 Reading page, block hover, block menu, Turn into — 9/10 pass (2:2 · 4:2 · 5:2 · 6:2 · 8:1)
| ID | Sev | Observation | Fix | SPEC |
|---|---|---|---|---|
| M03-01 | HIGH | Checklist boxes not Studio's widget (G07): radius 2, checked `TEXT` fill. | G07. | Q1 |
| M03-02 | MED | Selected block `.selblk` 706×24: the `BG_SELECTED_AREA` fill hugs the 24 px line with no vertical inset, so it reads as a text highlight. | `padding: 4px 8px; margin: 12px -8px -4px` (`SPACE_FACT` vertical). | No |
| M03-03 | MED | Pressed drag handle fill `BG_HOVER` vs `BG_SELECTED` elsewhere (G14). | G14. | No |
| M03-04 | MED | Turn into submenu does not mark the current type (Text). | Check icon right-aligned on the current item. | Yes |
| M03-05 | MED | Embed header kind icon at x412 (inset 12), body render at x416 (inset 16): left edges 4 px apart (P10 one content inset). | Header inset 0 16. | Yes: part 1 "inset 0 12" |
| M03-06 | MED | Dot spacing in embed header (G16); "Delete" text colour (G10); "Copy link to block" (G15); external-link glyph (G15). | See G. | G15 |
| M03-07 | LOW | The visible prose under the menus is one sentence; compared with ds-document-page.jpg (multi-paragraph, bold terms, links) the reading rhythm cannot be judged here. | Fine for a menu shot; M12 carries the reading page. | No |

### M04 Slash menu filtered "ta" — 8/10 pass (2:2 · 4:2 · 5:2 · 6:1 · 8:1)
| ID | Sev | Observation | Fix | SPEC |
|---|---|---|---|---|
| M04-01 | LOW | No-match state ("No blocks match", disabled row) not drawn (spec §3.2). | Add as third specimen. | No |
| M04-02 | LOW | Menu 4 px below the line box vs 8 px for the link popover (G13). | G13. | G13 |

### M04b Slash menu unfiltered — 8/10 pass (2:2 · 4:2 · 5:2 · 6:1 · 8:1)
| ID | Sev | Observation | Fix | SPEC |
|---|---|---|---|---|
| M04b-01 | MED | Hints set in Inter 12 `TEXT_TERTIARY`: "```" renders as three dots indistinguishable from an ellipsis; "[]", "---" read as glyph noise. They are Markdown syntax (P5 mono for code). | Hints in `Roboto Mono` 12/18 `TEXT_TERTIARY`. | Yes |
| M04b-02 | MED | No scroll affordance: the 358-tall menu ends exactly on "Code"; nothing tells the user Design and Data exist below. | Draw the `VAR_SCROLL_THUMB` thumb in the right inset; let the max height cut a row mid-way. | Yes |
| M04b-03 | LOW | Right specimen clips content at the outer 1 px border: "Bulleted list" is half under the radius-16 corner, not at the 8 px inner inset. | Clip the scroll area to the inset. | No |
| M04b-04 | LOW | Two menus hang off one caret with no caption. | Captions "Top" / "Scrolled to end" as M13 does. | No |

### M05 Inline toolbar + link popover — 7/10 FAIL (2:2 · 4:1 · 5:2 · 6:1 · 8:1)
| ID | Sev | Observation | Fix | SPEC |
|---|---|---|---|---|
| M05-01 | HIGH | Link field tokens (G06): fill `BG_SECONDARY`, 1 px `FOCUS_RING`, placeholder `TEXT_TERTIARY`. | G06. | G06 |
| M05-02 | MED | Existing-link state (URL + **Open** + **Remove link**, tooltip "Opens in browser") from §3.3 not drawn. | Add specimen. | No |
| M05-03 | LOW | First and highlighted suggestion "Payment methods" is the page being edited (self-link). | Exclude the current page or rank it last. | Yes |
| M05-04 | LOW | Toolbar "Turn into" uses chevron-down; block menu and spec use ▸. | Pick one. | Yes |

### M06 Table editing — 8/10 pass (2:2 · 4:1 · 5:2 · 6:2 · 8:1)
| ID | Sev | Observation | Fix | SPEC |
|---|---|---|---|---|
| M06-01 | MED | Same page "Payment methods" has H2s "Fees and limits / Settlement" here but "Accepted methods / Before release" in M03, M04, M10, M11; the two "On this page" lists contradict. | One page model: add "Fees and limits" as a third H2 of that page; OTP lists 3. | No |
| M06-02 | MED | Column handle (x623 y206, 24×24, filled) overlaps the H2 line box (y186–214) by 8 px. | Show the handle inside the header row (top centre), never above the table border. | Yes: §4 "handle above its header" |
| M06-03 | MED | Handle/add looks mixed (G14): row handle ghost, column handle and both "+" filled `BG_TERTIARY`. | G14. | No |
| M06-04 | LOW | "Insert left" / "Insert right" share one "+" icon while Move left/right have arrows. | Directional insert icons. | No |

### M07 Pin frame modal — 7/10 FAIL (2:2 · 4:1 · 5:2 · 6:1 · 8:1)
| ID | Sev | Observation | Fix | SPEC |
|---|---|---|---|---|
| M07-01 | HIGH | "Card declined" and "Processing" are pinned (tree: States › Payment › Card declined, Processing) but their tiles carry no "Pinned" badge, and the modal is pinning "Card declined" again under Payment. | Badge them; demo an unpinned state frame (e.g. add "Card expired"). | No |
| M07-02 | MED | Left list structure: chevrons on page rows ("Payment flow" expanded with no children, "Cart flow" collapsed), none on the file row; grid shows all 6 frames although "Payment flow" is the active page. Spec: "a file's pages expand under it (chevron)". | Chevron (`LAYER_CHEVRON_BOX` 24) on file rows; pages as plain depth-1 rows (indent 16); selected page `BG_SELECTED`; grid = that page's frames. | No |
| M07-03 | MED | List rows separated by 8 px (y276, 316, 356, 396 for 32-tall rows); tree rows use gap 0 (P9: filled rows separate themselves; `SPACE_ROW` 4 max). | `.pk-left { gap: 0 }`, field → list `SPACE_GROUP` 12. | No |
| M07-04 | MED | Column tops misaligned: left content starts y232 (padding 16), grid y224 (padding 8). | `.pk-right { padding-top: 16px }`. | Yes (README 12) |
| M07-05 | MED | Tile name 2 px below the thumbnail (thumb bottom 384, name 386). | `margin-top: 8px` (`SPACE_INLINE`). | Yes (README 12) |
| M07-06 | LOW | "Pinned" badge (y232–254) overlaps the render's top edge (~y251) by 3 px. | Fit the render inside the thumbnail below the badge band (top inset 38) or bottom-align the render. | No |
| M07-07 | LOW | Badge is a pill; kit `status_tag` (same tinted triple) uses radius 8. | Use `status_tag`. | Yes: §5 "pill" |
| M07-08 | LOW | No close control in the modal header (Esc only). | Add the kit Close button or state Esc-only in spec. | Yes |
| M07-09 | LOW | slop-detect `repeated-container-text`: "Payment" ×5 in one modal (tile, three "Payment flow", select value). | Vary sample names. | No |
| — | — | Plus G01 Cancel, G03 scrim/border, G06 search/select, G09 segmented. | | |

### M08 Suggest from canvas — 6/10 FAIL (2:2 · 4:0 · 5:2 · 6:1 · 8:1)
| ID | Sev | Observation | Fix | SPEC |
|---|---|---|---|---|
| M08-01 | HIGH | Frame inventory contradicts M07: Checkout-web had 6 frames (Payment, Card declined, Processing, Order confirmed, Cart, Empty cart); here its 3 unpinned frames are "Empty cart, Address, Promo applied" — Address and Promo applied do not exist in M07. | One frame inventory per file across M07/M08/M09. | No |
| M08-02 | HIGH | Mobile sample renders look broken at 2x (z-m08-mobile.png): "Linen shirt" and the card number wrap to two lines, "08 / 28" wraps, and "Order 10482 · Receipt sent to …@example.com" runs off the sheet's left edge. In a product whose job is rendering frames this reads as a renderer bug. | Author mobile frames at 375 design width, scale down. | No |
| M08-03 | MED | "Empty cart" thumbnail shows a cart with 3 items; "Promo applied" is identical to it; "Address" is the Payment form. | Distinct, truthful renders. | No |
| M08-04 | MED | Same column-top (header y224 vs field y232) and 8 px row-gap issues as M07-03/04. | As M07. | No |
| M08-05 | MED | Count pill inside the primary = `TEXT_ON_BRAND` fill + `TEXT` label: a black hole in the orange button; the kit has "no brand tone" for count badges (badge.rs). | Define the on-brand count pill (e.g. `BG_BRAND_PRESSED` fill + `TEXT_ON_BRAND`). | Yes (Q5) |
| M08-06 | LOW | "All files" row (layers icon) is not in spec §5. | Add to spec or drop. | Yes |
| — | — | Check circles G07; G01/G03/G06/G09. | | |

### M09 Embed + diagram states (900 and full) — 8/10 pass (2:2 · 4:1 · 5:2 · 6:2 · 8:1)
| ID | Sev | Observation | Fix | SPEC |
|---|---|---|---|---|
| M09-01 | HIGH | Tree selects "Payment methods" while the page shown is "Payment flow", which exists nowhere in the tree. | Add "Payment flow" (Spec) and select it. | No |
| M09-02 | HIGH | State-panel buttons ("Relink frame", "Try again") 28 tall (`CONTROL_STANDARD`, legacy) with 12/18 600 labels. P3 button heights {32, 36, 40, 44, 48}; P5 button labels 14 Semibold. README ambiguity 9 ("36 does not fit 120") is false: 20 + 4 + 18 + 12 + 36 = 90 ≤ 120 (15 px top and bottom); measured stack with 28 = 82. | Kit secondary 32 (`CONTROL_XS`) 14/20 600 (or 36 `CONTROL_SM`). | Yes: give the size in §6 |
| M09-03 | MED | "File not found" state (§6) and Ready + hover state (open button → `ICON`) not drawn. | Add. | No |
| M09-04 | MED | Embed header inset / dot spacing / external glyph (M03-05, G15, G16). | As referenced. | Yes |
| M09-05 | LOW | "Frame not found" / "Can't render this frame" use the frame glyph; no error cue. | Alert glyph `ICON_TERTIARY`. | Yes |
| M09-06 | LOW | Diagram preview centred at x733 (SVG 471–995) vs card centre x744. | Centre the SVG. | No |
| M09-07 | LOW | H2 "Frame embeds" is mockup meta, not document content. | Real heading. | No |
| M09-08 | LOW | Straight apostrophe in "Can't render this frame" next to curly quotes in M13b. | Use ’ consistently. | Yes: §15 |

### M10 Conflict banner + "Saving…" — 7/10 FAIL (2:2 · 4:1 · 5:2 · 6:1 · 8:1)
| ID | Sev | Observation | Fix | SPEC |
|---|---|---|---|---|
| M10-01 | MED | "Saving…" shows while the conflict is unresolved: it says the user's version is being written before they chose. | Hold saves while the banner is open; header keeps "N files" (or a held state). | Yes |
| M10-02 | MED | "Save failed" banner (`DANGER` border, "Can't save this page." + error + **Try again**) not drawn. | Add specimen. | No |
| M10-03 | LOW | Banner → H1 gap 24 (banner bottom 142, H1 top 166) is unspecified. | Name it `SPACE_SECTION` 24. | Yes |
| M10-04 | LOW | "3-D Secure" set as inline code (a product term, craft-floor §2 "mono as costume"); the chip `BG_TERTIARY` on callout `BG_SECONDARY` is 1.18:1, so it is invisible. Also M11/M11b. | Plain text. | No |

### M11 Agent co-editing — 7/10 FAIL (2:2 · 4:1 · 5:1 · 6:2 · 8:1)
| ID | Sev | Observation | Fix | SPEC |
|---|---|---|---|---|
| M11-01 | HIGH | Agent caret sits at the end of the paragraph ("…order below.") while the write flash marks the list the agent changed ("Pay later (planned)"). Spec: caret "at the block it writes". | Caret at the end of "Pay later (planned)", tag above it. | No |
| M11-02 | MED | Flash outline reads as a focus ring (G19). | G19. | Q4 |
| M11-03 | LOW | Chip says writes are queued while the list already shows the write (README 14 accepts it). | One state per capture. | No |

### M11b Off-screen "3 changes · Show" chip — 7/10 FAIL (2:2 · 4:2 · 5:1 · 6:1 · 8:1)
| ID | Sev | Observation | Fix | SPEC |
|---|---|---|---|---|
| M11b-01 | HIGH | Spec §9 says reuse the canvas chip (#695). `canvas/agent_changes_chip.rs`: 32 tall (`CONTROL_XS`) pill filled with the session colour, count in a separate mono pill (`BG_TERTIARY`, 16 tall), noun + "Show" in `AGENT_LABEL_TEXT`, direction chevron. Mockup: 28 tall `BG_TERTIARY` chip, dot, count inline in Inter ("3 changes"), "Show" underlined. Not a reuse; count not in a pill (P5). | Port the canvas anatomy as-is. | No |
| M11b-02 | LOW | "Show" is a 32×18 underlined word; if only the word is clickable it is under `TARGET_MIN` 24. | Whole chip is the button (as the kit does). | No |

### M12 Overview, KPI strip, Copy cho AI, toast — 10/14 = 71 % FAIL (1:1 · 2:2 · 3:2 · 4:1 · 5:2 · 6:1 · 8:1)
| ID | Sev | Observation | Fix | SPEC |
|---|---|---|---|---|
| M12-01 | HIGH | Tooltip (G04) and toast (G05) do not match Studio's surfaces. | G04, G05. | G05 |
| M12-02 | MED | KPI "13 · 96 Screens · rows", "24 · 11 REQs · US" while the tree shows Screens 3, Spec 3; a tile "click selects the matching group", so 13 lands on a group of 3. | Label the scope (spec model vs pinned) or change the Screens tile's source. | Yes (Q6) |
| M12-03 | MED | Values in Roboto Mono with spaced middle dot ("24 · 11", 128 px): mono spaces put a full character cell each side, so it reads as two drifting numbers. | Separator in Inter `TEXT_TERTIARY`, no extra spaces. | No |
| M12-04 | MED | Criterion 1: "Gates approved 4/5" does not say which gate is missing; the overview's one attention item has no action. | Tile sublabel names the open gate. | Yes |
| M12-05 | MED | Linked package "designos-studio-v2" in a Checkout project. | e.g. "checkout-spec · spec v2". | No |
| M12-06 | LOW | README ambiguity 8 is stale (label wraps, 98 tall); capture shows "Screens · rows" on one line, tiles 80. | Update README. | No |

### M13 Tree interactions — 7/10 FAIL (2:1 · 4:2 · 5:2 · 6:1 · 8:1)
| ID | Sev | Observation | Fix | SPEC |
|---|---|---|---|---|
| M13-01 | MED | Drag ghost (x70 y269, 224×32, opacity 0.8) lies over "Cart rules"; the underlying label shows through ("Cart rules" under "Error handling", z-m13-ghost.png). | Opaque `BG_SELECTED` ghost with the opacity on its contents only, or offset the ghost to the pointer. | No |
| M13-02 | MED | Context menu top y557 while its target "Checkout research" is y617–649; room exists below (617 + 251 = 868 < 900). | Top-left at the pointer inside the row. | No |
| M13-03 | MED | "Open" uses the external-link glyph (G15). | G15. | G15 |
| M13-04 | LOW | Rename row: 1 px `FOCUS_RING` on `BG_SECONDARY` vs kit field focus 2 px `BORDER_SELECTED` on `BG` (G06). | G06. | Yes |
| M13-05 | LOW | In the search specimen "Cart rules" carries `BG_SELECTED` with no reason given (selection vs keyboard highlight). | Caption it or drop the fill. | No |

### M13b Delete page modal — 8/10 pass (2:2 · 4:2 · 5:2 · 6:1 · 8:1)
| ID | Sev | Observation | Fix | SPEC |
|---|---|---|---|---|
| M13b-01 | HIGH | Destructive button tokens (G02). | G02. | Yes |
| M13b-02 | HIGH | Initial focus is drawn on "Delete page" (1 px `FOCUS_RING_ERROR`, offset 2): Enter deletes. A destructive confirm puts initial focus on Cancel. | Focus on Cancel; ring on Delete only when tabbed. | Yes: §12 names only the ring colour |
| M13b-03 | MED | Body wraps to a one-word widow "break." (modal 360, text box 310). | Two lines, one sentence each (break after "Trash."), or width 400. | No |
| M13b-04 | LOW | Modal centre y441 vs window 450 (M07 picker is exactly centred). | Centre. | No |
| — | — | G01 Cancel, G03 scrim/border. | | |

### index.html viewer (not scored: a review tool, not product UI)
| ID | Sev | Observation | Fix | SPEC |
|---|---|---|---|---|
| IDX-01 | MED | M09 entry loads the 900 version; §7 diagram states (below the fold) are unreachable from the index. | Add an "M09 full" entry. | No |
| IDX-02 | LOW | Prev / Next / Open full size use the G01 secondary look. | Follows G01. | No |

---

## README ambiguities, judged
| # | Verdict |
|---|---|
| 1, 2, 5, 6, 7, 10, 11, 13, 16, 19 | Accept. |
| 3 | Accept (`AGENT_LABEL_TEXT`, 10.12:1); spec §9 "white text" must change. |
| 4 | Reject: kit destructive = `BG_ERROR_SOLID` + `TEXT_WHITE` (G02). |
| 8 | Stale (M12-06). |
| 9 | Reject: the 36-does-not-fit claim is false; 28 breaks P3/P5 (M09-02). |
| 12 | Partly reject: 8 px row gaps, 8 vs 16 top padding, 2 px caption (M07-03/04/05). |
| 14 | Accept as composite, but the caret is on the wrong block (M11-01). |
| 15 | Reject: tooltip and toast have kit looks (G04, G05). |
| 17 | Reject: source capture is v0.6.3; current Home differs (M01-01). |
| 18 | Reject for a 1:1 rebuild: use the real rail (G18). |
| 20 | Sample content is inconsistent across screens (M06-01, M07-01, M08-01, M09-01). |

## SPEC changes collected
1. Part 2 shared values: secondary button, destructive button, modal scrim/border, toast, tooltip, field focus, menu text, popover offset → cite the kit looks (G01–G06, G08, G13).
2. "Delete in `DANGER` text" → `TEXT_ERROR` text, `DANGER` icon (G10).
3. §15: "Copy link to block/page", "Open in canvas" → rule-1 labels; no external glyph for in-app opens (G15).
4. Part 1 embed header inset 12 → 16 (M03-05); §6 state-panel button size (M09-02); state-panel glyph.
5. §9 tag text white → `AGENT_LABEL_TEXT`; agent colour vs focus ring (G19).
6. §8 saves held during a conflict (M10-01); banner→title gap.
7. §10 KPI scope vs tree counts; "Gates approved" names the open gate.
8. §12 initial focus on Cancel in the delete modal.
9. §3.1 current type marked in Turn into; §3.2 hints in mono + scroll affordance.
10. Tile colour for `TileKind::Project` (G17, M01-07); "Pinned" chip shape; on-brand count pill.
11. `design/principles.md` P2 says white on `BG_ERROR_SOLID` is 6.47:1; measured 4.83:1 (still AA). Principles text error, not a mockup defect.
12. P3 "Home header to grid = `SPACE_REGION` 48" vs product ≈32 (M01-09).

## Already excellent (do not break)
- Layout geometry matches spec to the pixel: rail x0 w48, tree x48 w240, column x399 w690, "On this page" x1137 w192, column top y80 (32 + 48), picker 880×600 at (280, 150) exactly centred.
- Tree rhythm identical on 15 screens: 32 rows, inset 8, indent 16, 24 chevron box, pills 22 centred (5/5).
- Concentric radii throughout: menu 16 / item 8 at inset 8; toolbar 12 / button 8 at inset 4; segmented 8 / tile 6 at inset 2; card 12 / thumb top 11 inside 1 px border.
- No shadow anywhere; hairlines only; every colour from tokens.css; no purple.
- Reading type scale exact (30/36 · 16/26 · 20/28 700 · 16/24 · 14/24); H2 32 above / 16 below; KPI 24 below the description; list indent 24, 4 between items.
- "On this page": 26-px items (≥ `TARGET_MIN`), active rule + `TEXT`, H3 indent 12.
- Diagram states are strong: line-number gutter with the error line in `DANGER`, 32-px error strip, last good render at 50 %, unsupported message copy exact.
- Copy deck followed almost everywhere; sentence case; ellipsis only on ongoing operations; curly quotes in the delete body.
- Every text pair measured ≥ 4.5:1 (lowest `DANGER` on `BG` 5.26:1; agent tag 10.12:1).

## Totals
| Severity | Count |
|---|---|
| HIGH | 22 |
| MED | 50 |
| LOW | 30 |
| **All** | **102** |

Operate: 7 pass (M02, M03, M04, M04b, M06, M09, M13b), 9 fail (M01, M05, M07, M08, M10, M11, M11b, M12, M13). M08 also has a 0 on criterion 4 (data truth). Passing screens still carry HIGH findings (M03-01, M09-01/02, M13b-01/02): a pass is not a release.

## Unresolved questions
- Q1. Checklist and multi-select checks: Studio's orange checkbox (kit) or a neutral check to keep one accent per view?
- Q2. Tree selected row: spec `TEXT` 500, or kit `row_look` (`TEXT_SECONDARY_HOVER`, 400)?
- Q3. Colour for the Project tile (tab icon, Home cards): new `TILE_PROJECT_*` token, or reuse an existing kind?
- Q4. Docs agent presence: keep `AGENT_1` amber (same as canvas) despite the focus-ring look-alike, or thicken the flash to 2 px?
- Q5. Count pill inside the orange primary: which token pair?
- Q6. KPI Screens tile: count spec screens (13) or pinned screens (3)? The click target must agree with the number.
