# #316 Copy from Studio, paste into Figma (fig-kiwi HTML clipboard payload)

| Field | Value |
|---|---|
| Issue | #316 (task T8.29, M / P2; epic #21; milestone v0.35 Bring boards in) |
| Code baseline | `origin/main@d22dad970`. Re-anchor every line against HEAD and report drift before coding. |
| Verdict | **Still real.** Studio writes only its own JSON as plain text on copy; nothing writes a Figma `text/html` payload. |
| Size | M: 10–14 h builder + 1 h step 0 + owner-manual Figma.app check |
| Tier | Claude Sonnet (format interop with an external app, clipboard on the UI thread, HOT FILE `fig_export.rs`) |
| Touches | `crates/designos-fig/src/clipboard.rs` (new `encode_figma_clipboard_html(doc_subset) -> String`, next to the parser `:57`), `crates/designos-fig/src/fig_export.rs` (only a `pub(crate)` entry that builds the Message for a subset with paste fields; HOT FILE queue position 44), `crates/designos-app/src/shell.rs` (`route_system_copy` `:1293-1299`), `crates/designos-app/src/ops.rs` (`copy_selection` `:851-858`), `crates/designos-app/src/clipboard.rs` (`capture` reuse). |
| Depends on | Step 0. PRs #1664, #1671, #1672 (designos-fig) land first. Owner answer **Q-316** (when the Figma flavour is written). |

## 1. Still real? (FACT unless tagged)

- Copy path: `route_system_copy` (`shell.rs:1293`) → `App::copy_selection` (`ops.rs:851`) → `clipboard::capture` + `to_json` → `ctx.copy_text(s)` (`shell.rs:1297`): one `text/plain` flavour holding Studio JSON.
- Paste path relies on that: "Studio's own markup is the whole clipboard: it replaced every other flavour when it was copied" (`ops.rs:870-872`); text is read first, HTML only when the text is not a Studio clip (`:873-878`).
- `arboard = "3"` is already a dependency (`crates/designos-app/Cargo.toml:42`) and is used to read HTML (`app/src/clipboard.rs:31`).
- Writer pieces exist: `export_fig(doc)` builds a full `.fig` (ZIP with `canvas.fig`, `meta.json`, `images/`) from a whole `Document` (`fig_export.rs:206-221`). No subset entry, no paste fields.
- Format facts (REA of Figma desktop 126.9.13, `plans/261017-figma-parity-wave-ab/rea-findings.md:9-26`):
  - FACT: Figma writes `text/plain` (selection text) and `text/html` = `<meta charset="utf-8"><div><span data-metadata="<!--(figmeta)` + base64(JSON) + `(/figmeta)-->"></span><span data-buffer="<!--(figma)` + base64(buffer) + `(/figma)-->"></span></div><span style="white-space:pre-wrap;">TEXT</span>`.
  - FACT: buffer = wasm `encodeClipboardData(...)`; reader = `decodeMessage(decodeClipboardData(bytes))`. Message paste fields: `nodeChanges`, `blobs`, `pasteID`, `pasteFileKey`, `pastePageId`, `pasteIsPartiallyOutsideEnclosingFrame`, `pasteBranchSourceFileKey`.
  - INFERENCE: buffer is a fig-kiwi archive; figmeta = `{ fileKey, pasteID, dataType: "scene" }`.
  - UNKNOWN: whether Figma's reader accepts a ZIP-wrapped archive, a bare `fig-kiwi` container, or only one of them; whether an unknown `fileKey` / missing `pasteID` is accepted; whether Figma needs `CANVAS`/`DOCUMENT` node changes in the payload; how fonts it lacks are handled.

## 2. User-visible outcome

A designer selects layers in Studio, copies, switches to Figma and pastes: the same layers appear as editable Figma layers (frames, auto layout, text, vectors, images), not a picture.

## 3. Step 0 (1 h, before code)

Copy a frame with text, an image and a vector in Figma.app (`/Applications/Figma.app`, scratch file), save the `text/html` to a fixture, decode with `parse_figma_clipboard_html`, and record: buffer magic (`PK` vs `fig-kiwi`), compression, schema chunk present or not, the Message's top-level fields and values for `pasteID` / `pasteFileKey` / `pastePageId`, and the figmeta JSON. This fixes every UNKNOWN above that can be read; acceptance by Figma remains the AC-1 owner-manual check.

## 4. Behaviour rules (decided; technical)

- R1 Build a subset `Document` from the selection with the same `capture` used by copy (`app/src/clipboard.rs`), including the styles, variables and images the subset uses (so Figma gets values, not dangling refs).
- R2 Encode with the existing writer + paste fields per step 0; wrap in the exact HTML template above; `TEXT` = concatenated text of selected text layers (or the first layer name).
- R3 Write both flavours in one clipboard set via `arboard` (`set().html(html, Some(studio_json))`), so `text/plain` still carries Studio JSON and Studio's own paste (`ops.rs:873`) is unchanged.
- R4 Studio's paste must prefer its own JSON over the Figma HTML it just wrote (today's order already does: text first). Add a test.
- R5 Encoding runs off the UI thread when the subset exceeds 2,000 nodes (decided threshold; measure in the PR): plain text is written immediately, HTML is added when ready unless the clipboard changed meanwhile (compare change count where the platform exposes it; else skip the late write).
- R6 Never write HTML for an empty selection; never throw away an export error silently — toast "Copied for Studio only: <reason>".

## 5. Acceptance criteria (red then green)

- **AC-1** Owner-manual (flagged, never claimed by the builder): paste into Figma.app shows the frame with layer names, text editable, image present (screenshot of Figma's layers panel + canvas).
- **AC-2** Round trip, automated: Studio copy → `parse_figma_clipboard_html` + `import_figma_clipboard` (`designos-fig/src/clipboard.rs:57`, `:113`) → same layer names, kinds and sizes as the selection.
- **AC-3** After copy, Studio paste in the same or another Studio window pastes the Studio clip (component links intact), not the Figma import.
- **AC-4** The HTML matches the template byte-for-byte except the base64 bodies and TEXT (string test).
- **AC-5** Copy of 10,000 nodes keeps the UI frame time under 16 ms during copy (R5), measured in a test or a logged run.

## 6. Test steps

1. `cargo test -p designos-fig clipboard` (encode/parse round trip).
2. App tests for AC-3 with an injected clipboard.
3. Owner-manual AC-1 in Figma.app; then put the screenshot in the PR and update Figma per the owner's "Figma first" rule only if a UI changed (none expected).

## 7. Edge cases

- Selection includes instances whose main is outside the selection → include the main in the payload (Figma needs it) or detach; decided: include the main as a hidden-from-canvas component node only if step 0 shows Figma supports it, else detach and toast the count.
- Studio-only node kinds (sticky, connector, stamp, section on design pages) → their `.fig` export path (#1614 connectors as vectors with a carrier) applies.
- Very large images → the payload grows; no cap beyond R5 (INFERENCE: Figma handles MB-size payloads; its own reader caps unknown).

## 8. Non-goals

Paste from Figma (shipped). Copy to Sketch/Illustrator. Copy as PNG/SVG/CSS (exists, `panels/export.rs:35`).

## 9. User-visible details not settled by the issue

- **Q-316**: does plain Cmd+C always add the Figma flavour, or is it a separate "Copy for Figma" command?

## 10. Rollback

Revert the copy-route change; Cmd+C returns to plain-text Studio JSON only.
