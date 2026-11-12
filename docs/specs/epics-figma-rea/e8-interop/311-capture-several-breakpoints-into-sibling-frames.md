# #311 Capture one page at several widths; each width becomes a sibling frame

| Field | Value |
|---|---|
| Issue | #311 (task T8.28, M / P2; epic #21; milestone v0.30 Web capture) |
| Code baseline | `origin/main@d22dad970` |
| Verdict | **Still real.** One capture = one viewport; the importer places one frame. |
| Size | M: 8–10 h builder (importer batch 3 h, extension resize loop 4 h, tests 2 h) |
| Tier | Claude Sonnet (shares the #310 surface; browser automation judgment). The importer half alone is Grok build-fast material if split. |
| Touches | `crates/designos-webcapture/src/import.rs` (new `import_captures_side_by_side(&[Capture], …)` beside `import_capture` `:168`), `crates/designos-app/src/capture_inbox.rs` (from #310: accept a batch), `extensions/chrome/` (from #310: width loop), `crates/designos-app/src/main.rs` / `cli.rs` only if CLI accepts several `--import-capture` paths (decided: yes, repeatable flag, cheap and testable). |
| Depends on | #310 (transport). Owner answer Q-311 (README Q7: resize method). |

## 1. Still real? (FACT)

- Capture envelope records the viewport it was taken at: `viewport: {width, height, dpr}` (`crates/designos-webcapture/js/capture.js:1548-1552`). Width is whatever the browser window is; the script does not resize.
- Importer takes one `Capture` and one optional `origin` (`ImportOptions.origin: Option<(f32, f32)>`, `import.rs:22-29`; `import_capture` `:168`). No batch API, no sibling layout.
- "Web capture has no bundled browser" (`full-scope-e7-e8-e10.md:325`), so widths must come from the user's browser via the extension (#310), or from captures the user already has (CLI / MCP / file open).
- Chrome can render a page at a given CSS width without resizing the window through the DevTools protocol (`Emulation.setDeviceMetricsOverride`) via `chrome.debugger` — INFERENCE from public Chrome extension API knowledge; builder confirms on Chrome stable. While attached, Chrome shows its "is debugging this browser" bar (user-visible; INFERENCE, confirm).
- FACT: `debugger` cannot be an optional permission (developer.chrome.com/docs/extensions/reference/api/permissions), so using it means every install shows the stronger warning, even for users who never capture several widths. Alternative without it: open a temporary window sized so its inner width equals the target (`chrome.windows.create` + measure `innerWidth`, adjust, capture) — less exact at widths below the OS minimum window size. → owner Q7 (README) decides.

## 2. User-visible outcome

From the extension the designer picks "Send at several widths". Studio receives one frame per width, side by side on the same page, named by width (e.g. "Pricing — 375", "Pricing — 768", "Pricing — 1440"), top-aligned, narrowest on the left, all selected, one undo step.

## 3. Behaviour rules (decided; technical)

- R1 Importer: `import_captures_side_by_side(doc, page, captures, opts)` builds each frame with the existing importer, sorts by `viewport.width` ascending, places them left to right with an 80 px gap (Studio's 4 px rhythm), top edges aligned at the placement origin, one `Command` (one undo step).
- R2 Names: `<page title> — <width>`; empty title → host name.
- R3 Captures in one batch must share the same URL (ignoring hash); otherwise refuse with "Captures are from different pages".
- R4 Extension loop (with #310; resize method per Q7 — shown here for the `debugger` option; the window option swaps step 1 for a sized temporary window): for each width — set metrics override, wait for layout to settle (`document.fonts.ready` + 2 animation frames + the script's own image wait), run `__designosCapture`, next width; always clear the override and detach at the end, also on error. Send one batch POST.
- R5 Media-query-driven content (e.g. a hamburger menu) is whatever the page renders at that width; no attempt to normalise.
- R6 CLI: `--import-capture a.json --import-capture b.json` imports side by side via R1 (testable without a browser).

## 4. Acceptance criteria (red then green)

- **AC-1** Three fixture captures of `tests/playwright/fixtures/card.html` taken by Playwright at 375 / 768 / 1440 import as three sibling frames, widths equal to each capture's `viewport.width`, x positions strictly increasing with 80 px gaps, same y. Red: no batch API.
- **AC-2** One Cmd+Z removes all three.
- **AC-3** Batch with two different URLs → error, document unchanged.
- **AC-4** Extension run at 3 widths on the fixture page (Playwright with unpacked extension) produces the same three frames as AC-1 (frame sizes ± 1 px).
- **AC-5** After the run, the browser tab is back to its own width (no override left; debugger detached).
- **AC-6** Visual: the three frames in Studio (1440 capture).

## 5. Test steps

`cargo test -p designos-webcapture side_by_side`; CLI repeated flag test in `cli.rs`; Playwright extension run; screenshot.

## 6. Edge cases

- A width larger than the screen: emulation still lays out at that width (INFERENCE; confirm).
- Pages that redirect by user agent (m.example.com): not handled; the batch rule R3 refuses mixed URLs.
- Lazy content that loads on scroll differs per width: captured as rendered.

## 7. Non-goals

Device frames (iPhone bezel) around captures; automatic responsive auto-layout merging of the frames into one adaptive component.

## 8. User-visible details not settled by the issue

- **Q-311 (README Q7)**: resize method (`debugger` permission vs sized window vs leave out).
- Default widths decided: 375 / 768 / 1440, editable in the popup (owner's standard check widths).
- If `debugger` is chosen, the extension adds a one-line hint "Chrome shows a debugging bar while it resizes the page".

## 9. Rollback

Revert the batch path; single captures keep working.
