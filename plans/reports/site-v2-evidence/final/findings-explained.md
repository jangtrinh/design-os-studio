# v2 page: every open gate finding, located and judged (2026-10-18, head 1d54fd1)

Lines are `docs/index.html` unless a file is named. `ui gate` prints its own line numbers about 22-28 lines off from the file, so each finding below is located by selector and given the real line. The #128 baseline is `plans/reports/pages-dark-landing-261017.md` on main.

## ui gate: 1 error (same as #128)
`autofix-not-clean: img-onerror`. `ui autofix` would add an `onerror` handler to all 9 `<img>`: lines 370, 383 and 396 (proof card backs), 419 (anatomy render), 451, 461 and 469 (workflow posters), 491 (big-files poster) and 546 (New in 0.3.0 render). It would also inject a fallback script before `</body>`.

On a failed load, that handler swaps the image for a generated SVG placeholder. If the script is missing, it falls back to `https://picsum.photos/seed/fallback/800/600`, a stock photo from a third party.

**Verdict: rejected on purpose, same as #128.** Every image is a same-origin file in the repo, and a failed load leaves its alt text visible. Applying the fix would put a placeholder or a stock photo where the page promises a real Studio render (the proposal's rule: every UI pixel is a real Studio render or film frame). It would also add a third-party request.

## ui gate: warnings (taste 5; #128 had 9)
| Finding | Real location | New vs #128 | Verdict |
|---|---|---|---|
| container-nesting-depth x4 | `#install` `.install > div > .panel > .code` (around 555-600) | no | Grid wrappers that each carry layout. False positive, as on #128. |
| container-nesting-depth | `#get` `.wrap.get__row > .get__actions > .cta`, lines 651-652 | **yes** | `.get__actions` stacks the button row over the downloads link inside the 2-column CTA grid. Layout wrapper, not decoration. |
| overused-font Inter | `:root` font | no | Owner mandate (Inter only). |
| image-hover-transform "9 images" | `.qa summary .ico` transition, line 219 | no | The only transform transition is the FAQ chevron. False positive, as on #128. |
| kicker-above-heading | `#install` | no | The page has no all-caps text. False positive, as on #128. |
| line-length "400-character run" | `<text id="closer-row">`, line 671 | **yes** | The wordmark's repeated feature names. SVG text clipped inside the glyphs, decorative (`role="img"` carries the name), never read as a line. |

## slop-detect, file scan: 6 findings (#128: 4)
| Finding | Real location | New vs #128 | Verdict |
|---|---|---|---|
| cramped-padding `<section> "sec"` (bg) | `#install.sec.band`, line 557 | no | Inset comes from `.wrap`'s centred width. False positive, as on #128. |
| cramped-padding `<div> "qa"` | `#faq .qa` | no | As on #128. |
| cramped-padding `<section> "scale"` | `#scale`, line 482 | replaces #128's old `#scale.sec.band` | Inset comes from `.wrap` plus `.scale__scene` padding (`docs/assets/v2-big-files.css:13`). The file scan has no layout; the rendered scan at 1440 and 375 does not flag it. |
| cramped-padding `<section> "sec"` (border-top+bg) | `#get.sec.get`, line 645 | **yes** | `padding-block: 48px` (`docs/assets/v2-closer.css:7`) plus `.wrap` inset. Not flagged by the rendered scan. |
| tight-leading 1.00x | `.proof-num` (365, 378, 391), `.scale__num` (486), wordmark SVG `<text>` (667-671), `.btn` labels | **yes** | All are single-line: display numerals, SVG text, and pill labels centred in 44-48 px pills. Line height only affects line-to-line spacing, so a single line is unaffected. |
| overused-font | | no | Owner mandate. |

## slop-detect, rendered: film-less copy at 1440x900 (18) and 375x812 (14). #128: 4 and 3.
The scan runs on a copy of `docs/` with only the film's two `<source>` tags removed. With the real 14.6 MB film it exceeds slop-detect's 30 s navigation timeout (two time-outs today; #128 used the same method). Logs: `logs/slop-rendered-nofilm-*.txt`.

| Finding | Real location | New vs #128 | Verdict |
|---|---|---|---|
| cramped-padding 16.56 px for 92 px text (375: 7.2 px for 40 px) | hero `#sel-box`, line 318 | no | The narrator's selection box hugging the headline, as Studio's selection does. #128 measured the ink gap at about 27 px. |
| line-length ~144 and ~126 chars | `.new-also p` and `.foot .legal p` | **fixed** | Both now `max-width: var(--measure)` (1d54fd1); the finding is gone. |
| clipped-overflow-container `div.scrub__stage` | `docs/assets/v2-story.css:33` (stage built by `tenant/scrub-section.js:74`) | **yes** | The sticky stage itself clips the scene's own content while pinned. That is the Tenant pattern. No ancestor of a stage clips it (tenant-lint PASS). |
| buried-raster `<img>` at opacity 0 | `.anatomy__render`, line 419 | **yes** | In motion mode the real render is held at 0 and fades in at progress 0.86-1 (`v2-anatomy.js`). It is the scene's final state, and the static layout shows it at full opacity. |
| nested-cards x3 (1440 only) | `.proof-card` (`v2-story.css:75`) holding `.proof-face` (`v2-story.css:78`) | **yes** | In motion mode `.proof-card` has no background and no shadow. It is an invisible 3D container for the two faces, so the reader sees one card. Not flagged at 375. |
| low-contrast x8-9 "via analytic-gradient+alpha" | proof card fronts, lines 365-396 | **yes** | Measured on real pixels: in motion mode at 1440, numbers and labels are 14.1-14.4:1 and notes 5.5-6.3:1 (`logs/proof-front-contrast.txt`). In the static row at 375 they are 14.2-14.7:1 and 6.5:1 (`logs/proof-front-contrast-375.txt`). The scanner composites the 7% tint gradient against an unresolved backdrop. |
| low-contrast "208,610" pixel 1.0:1, median 17.0:1 | `.proof-num`, line 365 | **yes** | Median 17:1. The 1.0:1 minimum is antialiased glyph edges. |
| body-text-viewport-edge x2 (375) | `.proof-note` of cards 2 and 3 | **yes** | Those cards sit off-screen in the proof cards' horizontal swipe row (`overflow-x: auto`, scroll-snap), the mobile pattern the proposal specifies. Page `scrollWidth` is 375: no page overflow. |
| script-error "reading 'getAttribute'" | film script, line ~765 | copy only | Caused by the removed `<source>` in the scan copy. The real page logs 0 errors across a full motion scroll (`logs/motion-scroll-errors.json`). |
| overused-font | | no | Owner mandate. |
| cramped-padding on pills (#128) | | gone | |
