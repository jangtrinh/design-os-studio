# Project docs workspace: UI mockups (M01 to M13)

Open **[index.html](index.html)** first: left list of every mockup with its spec section, the selected mockup in an iframe scaled to fit (CSS transform), Prev / Next buttons, Left / Right arrow keys, `#m07`-style deep links, and an "Open full size" link. Works from `file://`, no server, no network. Capture: [png/index.png](png/index.png).

Static HTML/CSS reference for the approved UI spec ([part 1](../ui-spec-ported-from-design-studio.md), [part 2](../ui-spec-part-2-editor-dialogs-and-states.md)). Chrome values come from `crates/designos-app/src/theme.rs` (origin/main) and the two spec files; nothing was redesigned. Captures are 1440 x 900 headless Chrome.

Shared files: [tokens.css](tokens.css) (every colour literal lives here only), [shell.css](shell.css) (layout and components). Icons are inline SVG, 16 px (rail 20 px), Lucide-style paths, `stroke-width="1.5"` in a 24 viewBox (renders about 1 px at 16 px; see Ambiguities).

## Mockups

| ID | HTML | PNG | Spec section |
|---|---|---|---|
| M01 Home, open project | [m01-home-open-project.html](m01-home-open-project.html) | [png](png/m01-home-open-project.png) | part 2 §1 |
| M02 First open, empty states | [m02-first-open-empty-states.html](m02-first-open-empty-states.html) | [png](png/m02-first-open-empty-states.png) | part 2 §2 |
| M03 Reading page, block hover, block menu, Turn into | [m03-reading-block-menu.html](m03-reading-block-menu.html) | [png](png/m03-reading-block-menu.png) | part 1 reading page, embed card, On this page; part 2 §3.1 |
| M04 Slash menu filtered "ta" | [m04-slash-menu-filtered.html](m04-slash-menu-filtered.html) | [png](png/m04-slash-menu-filtered.png) | part 2 §3.2 |
| M04b Slash menu unfiltered | [m04b-slash-menu-unfiltered.html](m04b-slash-menu-unfiltered.html) | [png](png/m04b-slash-menu-unfiltered.png) | part 2 §3.2 |
| M05 Inline toolbar + link popover | [m05-inline-toolbar-link-popover.html](m05-inline-toolbar-link-popover.html) | [png](png/m05-inline-toolbar-link-popover.png) | part 2 §3.3 |
| M06 Table editing | [m06-table-editing.html](m06-table-editing.html) | [png](png/m06-table-editing.png) | part 2 §4 |
| M07 Pin frame modal | [m07-pin-frame-modal.html](m07-pin-frame-modal.html) | [png](png/m07-pin-frame-modal.png) | part 2 §5 |
| M08 Suggest from canvas | [m08-suggest-from-canvas.html](m08-suggest-from-canvas.html) | [png](png/m08-suggest-from-canvas.png) | part 2 §5 |
| M09 Embed + diagram states | [m09-embed-and-diagram-states.html](m09-embed-and-diagram-states.html) (900) and [m09-embed-and-diagram-states-full.html](m09-embed-and-diagram-states-full.html) (1600) | [png](png/m09-embed-and-diagram-states.png), [full](png/m09-full.png) | part 2 §6, §7 |
| M10 Conflict banner + "Saving…" | [m10-conflict-banner-saving.html](m10-conflict-banner-saving.html) | [png](png/m10-conflict-banner-saving.png) | part 2 §8 |
| M11 Agent co-editing | [m11-agent-co-editing.html](m11-agent-co-editing.html) | [png](png/m11-agent-co-editing.png) | part 2 §9 |
| M11b Off-screen "3 changes, Show" chip | [m11b-agent-offscreen-chip.html](m11b-agent-offscreen-chip.html) | [png](png/m11b-agent-offscreen-chip.png) | part 2 §9 |
| M12 Overview, KPI strip, linked spec, Copy cho AI, toast | [m12-overview-kpi-copy-for-ai.html](m12-overview-kpi-copy-for-ai.html) | [png](png/m12-overview-kpi-copy-for-ai.png) | part 2 §10 |
| M13 Tree interactions | [m13-tree-interactions.html](m13-tree-interactions.html) | [png](png/m13-tree-interactions.png) | part 2 §12 |
| M13b Delete page modal | [m13b-delete-page-modal.html](m13b-delete-page-modal.html) | [png](png/m13b-delete-page-modal.png) | part 2 §12 |

All 16 PNGs are 1440 x 900 (checked with `sips`); `m09-full.png` is 1440 x 1600; `index.png` is 1440 x 900. Responsive captures (375/768) are SKIPPED by owner default.

## Layout measurement (acceptance 1)

Headless Chrome, `getBoundingClientRect()` after fonts loaded, on `m03-reading-block-menu.html` (identical on every page that has the region (rail and tree on 16 of 17 pages, reading column on 15, "On this page" on 10; M01 is Home and has none, M13 has no reading column; counts from the measure run)):

| Region | x | y | w | h | Expected |
|---|---|---|---|---|---|
| `.rail` | 0 | 32 | 48 | 868 | x0 w48 |
| `.tree` | 48 | 32 | 240 | 868 | x48 w240 |
| `.col` (reading column) | 399 | 80 | 690 | content | x399 w690 |
| `.otp` ("On this page") | 1137 | 80 | 192 | 78 | x1137 w192 |
| `.tabstrip` | 0 | 0 | 1440 | 32 | 32 |
| `.tree-head` | 48 | 32 | 239 + 1 border = 240 | 65 | 65 |
| `.tree-title` ("Documents" row) | 48 | 97 | 240 | 40 | 40 |
| group row `.g` / item row `.r` | 56 | 137 / 169 | 224 | 32 / 32 | 32, inset 8 |
| `.embed-h` (card header) | 400 | 595 | 688 (+2 border) | 36 | 36 |
| `.otp a` (item) | 1137 | 106, 132 | 192 | 26 | 26 |

Column top is y80 = 32 tab strip + 48 `SPACE_REGION`. Main region x288 w1152; 399 = 288 + (1152 - 930) / 2.

## Colour literal proof (acceptance 3)

```
grep -nE '#[0-9a-fA-F]{3,8}\b|rgba?\(|hsla?\(|oklch\(' shell.css index.html m*.html   -> 0 matches
tokens.css: 22 distinct #RRGGBB values, every one present in theme.rs (script check: missing = [])
box-shadow / drop-shadow / text-shadow in shell.css, index.html, m*.html            -> 0 matches
purple / violet / indigo in tokens.css, shell.css                                    -> 0 matches
emoji probe: rg '[\x{1F300}-\x{1FAFF}\x{2600}-\x{27BF}]' (red-probe on "✓" exit 0 = can go red)  -> exit 1 = none
```

Three alpha tokens are derived values the spec names: `--border-translucent` (BORDER_TRANSLUCENT), `--bg-selected-area` (BG_SELECTED_AREA), `--bg-tertiary-60` (spec: embed header BG_TERTIARY at 60 %), `--scrim` (spec: black 60 %). Sample frame renders use `READER_LIGHT_*` values from theme.rs.

## Gates (acceptance 5, es-designer checklist §1)

Commands run per file (18 files: 16 mockups, `m09-...-full`, and `index.html`):

| Gate | Command | Result |
|---|---|---|
| ui gate | `ui gate <file>` | **0 errors on all 18 files.** Exit code 1 on all, caused only by `token-coverage: 20% (floor 80%) — FAIL` (18 % on index.html). Cause: the gate checks `var(--x)` names against the project design:os DS (`design/`), and the mockups use their own `tokens.css` names (`--text`, `--bg-secondary`, `--w-strong`, `--sample-*`, ...), reported as `off-system-token` warnings (about 440 per file, same names repeated). Every value behind them is a theme.rs value (script check above). Not a pass: justified by "mockup token file mirrors theme.rs but is not registered in `design/design.tokens.json`". Layout clean, a11y clean, content clean. |
| ui gate fixes applied | | `var(--d)` undeclared (now in `:root`), `font-display: swap`, viewport meta, `overflow: clip` instead of `hidden` on html/body, `.fr .nm` colour leaking from the picker tile rule (real 1.04:1 contrast bug in M07/M08 thumbnails, fixed by renaming classes), hover media guard in index.html. |
| ui gate remaining warnings | | `container-nesting-depth` (sample frame renders inside the embed card are nested 7 to 8 deep; menus `div.app > div.menu > div.mi`), `font-scale-sprawl` / `flat-type-hierarchy` (document type scale 14/16/18/20/30 from the spec plus sample-render sizes), `radius-sprawl` (2, 4, 5, 10, 11, 12, 14, 16, 60: spec roles 2/6/8/12/16 plus pill radii = height / 2 and the scaled frame renders), `z-index-off-ladder` (15, 25, 31), `shape-assembled-illustration` (rail tiles), `nested-cards` (sample frame fields inside the embed card). All warnings, all justified by the spec (P4 pills, document type scale) or by the sample content. |
| slop-detect | `~/.claude/skills/es-designer/scripts/vendor/slop-detect/slop-detect detect <file>` | exit 2 on all 18 files, findings only: `cramped-padding` x150 (height-centred rows and pills: fixed `height` with flex centring, e.g. `.mi`, `.r`, `.tab`; false positive, the padding is the 8 px inset in the CSS), `clipped-overflow-container` x16 (`html/body/.app overflow: clip` is the 1440 x 900 artboard crop; menus are inside it), `overused-font` x3 (Inter is Studio's chrome font, P5), `repeated-container-text` x2 (repeated tree rows), `gray-on-color` x1 (M03 selected-block tint `BG_SELECTED_AREA` over `BG`; D4D4D4 on the blended fill is about 11:1, the detector misreads the blue tint as a coloured surface). No Advisory section was printed. |
| a11y-audit (axe, rendered) | `a11y-audit file://<abs>/<file> --tags wcag2a,wcag2aa,wcag21aa` | exit 0 on all 18 files after fixing `aria-label` on role-less spans (now `role="img"`) and the dragged-row opacity in M13 (0.4 to 0.6). |
| DS drift | manual vs `theme.rs` | 22 hex values mapped, 0 unmapped. Non-token sizes listed under Ambiguities. |
| probe-render.mjs, build-evidence lint, taste-duel | | NOT RUN: fixed 1440 x 900 artboards, responsive checks skipped by owner default, no builder loop (reference mockups, not product code). |

## Ambiguities resolved (choice made)

Spec contradictions or missing values, each resolved with the closest named token:

1. **Font paths.** `tokens.css` loads Inter and Roboto Mono from `crates/designos-fonts/assets/` with paths relative to this folder, so the mockups need a checkout at or after `main` e4efa94 (Roboto Mono is not in older checkouts).
2. **Icon stroke.** "16 px, 1.5 stroke": used `stroke-width="1.5"` on the 24 viewBox (Lucide's `strokeWidth={1.5}` at `size={16}`), which renders about 1 px. If a rendered 1.5 px is meant, set `stroke-width` 2.25.
3. **Agent name tag text colour.** Part 2 §9 says white text; theme.rs `AGENT_LABEL_TEXT` is `#0A0A0A` and the session colour `AGENT_1` is `#FFA629` (white on it is under 2:1). Used `AGENT_LABEL_TEXT`.
4. **Destructive button label.** Spec: `DANGER` background. White on `#EF4444` is 3.8:1, so the label uses `TEXT_ON_BRAND` (`#0A0A0A`, about 5.3:1). Primary buttons also use `TEXT_ON_BRAND`.
5. **Block handle position.** "32 px left of the column": the drag handle spans x column-32 to column-8, the "+" sits 24 px further left (column-56 to column-32).
6. **Space after the description before a non-heading first block** (M02 empty paragraph): 32 px (same as H2).
7. **Empty group count pill.** Empty groups show no pill (the placeholder row says it).
8. **KPI tile height.** Four tiles at gap 16 are 160.5 px wide; the label "Screens · contract rows" wraps to two lines, so all tiles stretch to 98 px (80 px if the label fitted). Value font Roboto Mono 20/30 600.
9. **Embed state panels.** Rendering / Frame not found / Render failed panels use `BG_SECONDARY`, radius 0 (they sit 16 px inside a 12 px card, so no concentric radius), and a 28 px (`CONTROL_STANDARD`) 12/18 600 secondary button, because a 36 px button does not fit the 120 px panel. "Rendering…" block uses the aspect ratio 1280:360 of the last render.
10. **Slash menu filter "ta".** Substring match on labels gives State (Design) and Table (Data); Table highlighted as the brief asks. M04b shows the unfiltered menu twice (scrolled to top, scrolled to the end), because max height 360 cannot show all 14 rows and 3 section labels at once.
11. **Table.** Hide-scrollbars capture flag removes native bars, so the horizontal scrollbar is drawn (track 11 px = `VAR_SCROLL_H`, thumb `VAR_SCROLL_THUMB`, thumb width = 690 / 960). Table content is 960 px wide. The add-row "+" sits under the scrollbar at the left edge; add-column "+" sits 8 px right of the column.
12. **Picker internals not in the spec:** header 64 px, footer 76 px (36 button + 2 x 20), left column padding 16/24, right column padding 8 24, tile name margin 2, selected tile 2 px `BORDER_SELECTED` outline inside the thumbnail, multi-select check circle 20 px (selected: `TEXT` fill, `TEXT_ON_BRAND` check). "Pinned" badge `BADGE_SM` 22.
13. **M03 selection.** The block with the open menu shows `BG_SELECTED_AREA` behind it (spec: selected = handle click) and its handle button is pressed.
14. **M11.** Agent caret, write flash and the "Claude is waiting" chip appear together as the brief requires (in the product the chip implies the caret is held). The M11b variant removes caret and flash and shows "3 changes" + "Show".
15. **M12.** The "Copy cho AI" icon sits 8 px right of the heading text, with a tooltip above it; tooltip style = menu style (radius 8, `BG_MENU`, 1 px `BORDER_MENU`). Toast: `BG_TERTIARY`, radius 12, 40 tall, bottom 72, centred on the window.
16. **M13.** The search state cannot coexist with rename / drag / context menu in one list (search filters rows), so the live tree shows rename ("Competitor flows"), drag (dragged row "Error handling" dimmed, 80 % ghost, 2 px `TEXT` insertion line after "Payment methods") and the context menu (on "Checkout research"), and a second specimen panel (x520) shows the same tree in search state for "ca" with `TEXT` 600 matches. The context menu lists the page-item entries from §12.
17. **M01 Home.** Layout copied from the existing Studio capture (`studio-home-main.png`): sidebar 264, header 64, filter bar 38 (divider at y134), cards 224 x 140 thumb, gap 24, pill buttons 32 tall; no app rail on Home. "+ Design file" uses `BG_BRAND` (the old capture shows blue; spec says orange is the one accent). The context menu is drawn under the tile at 380, 406 (pointer position is arbitrary). "Open project" is a 36 px secondary button at the tile's top right; project thumbs use `BG_SECONDARY` so the secondary button stays visible.
18. **App rail items** (menu, book, layers, sparkles, plus) are placeholders for Studio's real rail icons; only the book (Project) tile is meaningful.
19. **Values outside spec and theme.rs** (derived or sample only): 5 / 10 px link-mark on linked-spec icons; 7 px scrollbar thumb inside the 11 px track; 20 px check circle and caret height; 38 px Home filter bar (measured from Studio's capture); 64 / 76 picker header and footer; 560 px specimen height; 960 px table width; and every size inside the sample checkout frame renders (22/26/32/36 px etc., scaled by CSS transform to the embed width).
20. **Sample content.** Strings not in the §15 copy deck are sample content: page titles, list items, table cells, file and frame names, "Frames" tile names, error line "Out of memory while drawing 1 of 41 layers.", "Order 10482 ...", menu extras on Home ("Add to starred", "Reveal in Finder", "Delete"), "Pages" rows in the link popover, rail tooltips. All chrome copy follows §15 ("Copy cho AI" kept). Straight apostrophe in "Can't render this frame" (deck spelling).

## NOT verified

- Rendering in egui. These mockups define pixel intent only; egui text metrics (Inter at opsz 14 via the bundled variable font) can differ by a pixel.
- `ui gate` is not a pass (token-coverage, see above); `slop-detect` exits 2 on design-intent findings.
- Hover, focus and motion states beyond the ones drawn (spec §14 durations are not animated here).

## Rebuild

The HTML was generated by a throw-away script in the scratch folder (not part of the deliverable). Edit the HTML/CSS here directly, or capture again with:

```
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new --disable-gpu --hide-scrollbars --window-size=1440,900 --screenshot=png/<name>.png file://$PWD/<name>.html
```
