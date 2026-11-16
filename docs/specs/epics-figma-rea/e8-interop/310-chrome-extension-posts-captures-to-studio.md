# #310 Chrome extension: capture the current page and send it to the open Studio file

| Field | Value |
|---|---|
| Issue | #310 (task T8.26, M / P2; epic #21; milestone v0.30 Web capture). Web capture is owner-kept (roadmap trim 2026-10-30; epic #21 ruling R6.22; ruling 22 in `plans/261017-epics-full-scope/owner-decisions-and-cross-check.md:221`: the "never html-to-figma" rule covers only the Figma write workflow, Studio's own web-capture importer stays). |
| Code baseline | `origin/main@d22dad970` |
| Verdict | **Still real.** Capture script and importer shipped (#276, #277 closed 2026-10-22); no extension, no endpoint in Studio that accepts a capture. |
| Size | M–L: 12–16 h builder (extension 5 h, Studio inbox 5 h, tests 3 h) + store listing if Q-310b picks it |
| Tier | Claude Sonnet (new network-facing surface on the user's machine; security-sensitive) |
| Touches | new `extensions/chrome/` (MV3: `manifest.json`, `background.js`, `popup.html/js`, copies `crates/designos-webcapture/js/capture.js` at build time — one source of truth, a script copies it), new `crates/designos-app/src/capture_inbox.rs` (listener + queue), `crates/designos-app/src/shell.rs` (drain the queue each frame like other inbound work), `crates/designos-app/src/ops.rs` (reuse the web-capture import used by the menu, `:2464`), `.project-agent.md` + `~/.config/project-localhost-ports.json` (port record, per Q-310a). |
| Depends on | Owner answers Q-310a (transport/port), Q-310b (distribution), Q-310c (pairing). Unblocks #311. |

## 1. Still real? (FACT)

- Capture script: `crates/designos-webcapture/js/capture.js` (1,578 lines) exposes `window.__designosCapture(options) -> Promise<Capture>` (`:4`); envelope `{schema: "designos-webcapture", version: 1, url, title, viewport {width, height, dpr}, scroll, …}` (`:1543-1560`). No bundled browser by design ("Web capture has no bundled browser", `full-scope-e7-e8-e10.md:325`).
- Import entry points today: File menu web-capture import (`ops.rs:2464`, picker `io.rs:495`), CLI `--import-capture` (`main.rs:431-434`), MCP `designos_import_web_capture` (`designos-mcp/src/engine.rs:1241`), library import of `.webcapture` / capture JSON (`designos-library/src/store.rs:166-169`). All take a file; none receives a capture from a browser.
- A loopback + token pattern already exists in Studio: plugin panels bind `127.0.0.1:0` and put a `uuid` token in the URL (`crates/designos-app/src/plugins/runtime.rs:41-46`), body cap `MAX_BODY = 2 MiB` (`:17`).
- Plan wording: "Chrome extension that posts a capture to the Studio loopback with a token; AC: a token is required; captures land on the open file" (`full-scope-e7-e8-e10.md:432-433`).
- Port contract: `.project-agent.md` has no `localhost_port`; the machine registry gives `design-os-studio` port **4103**, used by the promo-film HyperFrames preview server (`~/.config/project-localhost-ports.json`). A Studio inbox on 4103 would collide with that server on the owner's machine. → Q-310a.
- Alternative channel FACT: `designos-mcp` already changes the open document of a live Studio session through an append-only edit log Studio polls each frame (`crates/designos-app/src/live_patch_log.rs:1-4`, `doc-patch.jsonl`). A Chrome native-messaging host could reuse that path without a port (option in Q-310a).

## 2. User-visible outcome

A designer on any web page clicks the DESIGN:OS Studio extension, then "Send to Studio". Within seconds the page appears in the file open in Studio as an editable frame (auto layout, real text), selected and centred, one undo step. If Studio is not running or not paired, the extension says so and how to fix it.

## 3. Behaviour rules

Decided (technical):
- R1 Studio side: an inbox that accepts one POST of a capture JSON (schema `designos-webcapture` v1) from the extension only: reject requests with an `Origin` other than `chrome-extension://<our id>`, reject without the token (401), bind `127.0.0.1` only, body cap 64 MiB (captures carry data-URL images; T8.25 has its own image byte budget), JSON validated by the existing `designos_webcapture` parser before anything touches the document.
- R2 Landing: the active document tab, its current page, centred in the viewport, selected, one undo step (same placement as Paste from Figma, `app/src/clipboard.rs:186-232`). On Home with no file open: create a new design file named after the page title and open it.
- R3 Feedback: Studio toast "Captured <page title>" with the import summary (`ImportReport::summary_toast`, `designos-webcapture/src/import.rs:107`); extension popup shows Sent / Studio not running / Not paired / Page blocked (chrome://, Web Store pages cannot be scripted).
- R4 The inbox starts with Studio and stops on quit (no orphan listener; quit cleanup path). If its port is taken, Studio does not pick another one: the extension shows "Studio not reachable" and Studio logs the port owner (per the global port rule).
- R5 Extension permissions for #310: `activeTab`, `scripting`, `storage`; host permission only for `http://127.0.0.1:<port>/*`. No `<all_urls>`. #311's multi-width capture adds `debugger`, which Chrome does not allow as an optional permission (FACT, developer.chrome.com/docs/extensions/reference/api/permissions: not-optional list includes "debugger"), so it would show at install for every user → owner Q7 in README.
- R7 Fixed extension ID: the `Origin` check in R1 needs a stable `chrome-extension://<id>`. Pin it with the manifest `key` field ("maintains the unique ID of an extension … when it is loaded during development", developer.chrome.com/docs/extensions/reference/manifest/key), whatever Q-310b picks. Applies to both Q-310a options.
- R6 `capture.js` is copied into the extension by a script, never edited there; a CI check fails if the two differ.

Needs owner: transport/port (Q-310a), distribution (Q-310b), pairing (Q-310c).

## 4. Acceptance criteria (red then green)

- **AC-1** POST without token → 401; wrong `Origin` → 403; valid → 200 and the frame lands (integration test against the inbox on a test port with an injected `App`).
- **AC-2** Capture of `crates/designos-webcapture/tests/playwright/fixtures/card.html` sent through the real extension (Playwright with the unpacked extension loaded) lands in the open file; its frame name and child count equal a `--import-capture` of the same capture.
- **AC-3** Studio not running → popup shows "Studio not reachable" within 2 s.
- **AC-4** Malformed or oversize body → 400/413, document unchanged, no panic.
- **AC-5** Quit Studio → port released (`lsof -i :<port>` empty).
- **AC-6** Visual: extension popup + Studio with the landed frame (1440).

## 5. Test steps

1. `cargo test -p designos-app capture_inbox`.
2. Playwright (`tests/playwright/run_capture_tests.mjs` pattern) with `--load-extension=extensions/chrome` against a built Studio (owner machine, display awake).
3. Screenshots.

## 6. Edge cases

- Two Studio windows/processes: only one can own the port; the other shows "Capture inbox in use by another Studio window".
- Board or Deck document active: lands on the current page the same way.
- Very long pages: capture script's own limits (`T8.25` spec §8) apply; the toast reports truncation.
- User switches tab in Studio while a capture is in flight: lands in the tab active at arrival.

## 7. Non-goals

Firefox/Safari extensions; multi-breakpoint capture (#311); editing captures in the browser; any cloud relay.

## 8. User-visible details not settled by the issue

- **Q-310a** Transport: fixed loopback port (which one) vs Chrome native messaging.
- **Q-310b** Distribution: Chrome Web Store listing vs "Load unpacked" from the Studio app bundle.
- **Q-310c** Pairing: one-time code typed into the extension vs an "Allow this extension?" prompt in Studio.

## 9. Rollback

Revert removes the listener (`capture_inbox.rs` + its start call) and the extension folder; no file-format change. A published store listing (if Q-310b picks it) is unpublished separately.
