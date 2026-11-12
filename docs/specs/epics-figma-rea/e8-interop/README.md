# E8 Interop (epic #21): build-ready specs, 2026-11-06

Code baseline: `origin/main@d22dad970` (fetched 2026-11-06 ~22:50). Every spec re-anchors against HEAD before coding.

## Finding first: milestone "v0.7.8 Interop" holds no E8 issue

Milestone 66 "v0.7.8 Interop" has 3 open issues: #1225 (WB-10 Clipboard Board ↔ Design), #1226 (WB-11 export toggle), #1227 (WB-12 MCP ops for whiteboard kinds). All three are sub-issues of #1215 (Whiteboard epic), **not** of #21. None of E8's 17 open sub-issues is in v0.7.8, so none is specced as a v0.7.8 item here. The whiteboard trio belongs with the #1215 specs (WB order already ruled in `controller-ledger.md`).

## Scope of this pass

10 issues specced in full; 2 bugs with open PRs get landing notes; 5 are ranked with one line each (not specced).

| Issue | Still real? | Size | Tier | Depends on | Ready after owner answers? |
|---|---|---|---|---|---|
| [#834](834-fig-save-roundtrip-pixel-gate.md) .fig save drops lists / icon strokes | Partly. Top-level fixed by #1647 (INFERENCE from its body); nested overrides still dropped (#1649, PR #1664); smoke 4a pixel gate missing (FACT) | M 5–7 h | Grok build-fast | PR #1664 | Yes, no question |
| [#891](891-font-weight-writers-close-out.md) font_weight writers sync `wght` | Writers fixed by #1616 (#1414); 2 leftovers moved to #941 | XS 1 h | Grok build-fast | none | Yes; then close |
| [#941](941-fig-font-weight-roundtrip-style-names-axis.md) .fig weight round trip | Yes, all 3 items; kit shows Inter "Semi Bold" ×2,770 and `axisName: "Weight"` | M 6–8 h | Claude Sonnet | PRs #1664 / #1671 / #1672 (designos-fig) | Yes, no question |
| [#289](289-figma-paste-merges-styles-variables-links.md) Figma paste keeps styles / variables / links | Yes (`clipboard.rs:291-293` clears them) | M–L 10–14 h + 1 h step 0 | Claude Sonnet | step 0 capture; Q1 | After Q1 |
| [#307](307-zip-entry-probe-routes-sketch-and-zipped-fig.md) zip-entry probe | Yes (extension-only routing, twice) | S 4–5 h | Grok build-fast | none | Yes, no question |
| [#308](308-import-sketch-files-core.md) Sketch import core | Yes (no Sketch code) | L 20–28 h | Claude Sonnet | #307; Q2 | After Q2 |
| [#309](309-sketch-symbols-components-shared-styles-libraries.md) Sketch symbols / styles / libraries | Yes | L 16–22 h | Claude Sonnet | #308; Q3 | After Q3 |
| [#316](316-copy-from-studio-to-figma-clipboard.md) Copy Studio → Figma | Yes (copy writes Studio JSON only) | M 10–14 h + step 0 | Claude Sonnet | step 0; PRs #1664 / #1671 / #1672; Q8 | After Q8 |
| [#310](310-chrome-extension-posts-captures-to-studio.md) Chrome extension → Studio | Yes (no extension, no inbox) | M–L 12–16 h | Claude Sonnet | Q4, Q5, Q6 | After Q4–Q6 |
| [#311](311-capture-several-breakpoints-into-sibling-frames.md) Several widths → sibling frames | Yes (one capture = one frame) | M 8–10 h | Claude Sonnet | #310; Q7 | After Q7 |
| [#1243](1243-1346-in-flight-landing-notes.md) auto-layout wrap vs Figma | Yes on main; PR #1680 targets it | — (PR exists) | follow-up: Grok build-fast | controller Wave B (after #1667, #1672) | Yes; ask PR for the not-fit case (AC-2/AC-3) |
| [#1346](1243-1346-in-flight-landing-notes.md) scaled-instance text size | Yes on main; PR #1411 targets it (BEHIND main) | — (PR exists) | follow-up: Claude Sonnet | owner left #1411 out of this release (11-04); owner-manual Figma read of 0.875 / 0.833 classes | Release-scope call already made by the owner |
| #290 Import PPTX into a deck | Yes (no PPTX code; `DocumentKind::Deck` exists, `designos-doc/src/board.rs:13-19`) | L | Claude Sonnet | none | Not specced this pass (rank 1 below) |
| #294 Export decks to PPTX | Yes | L | Claude Sonnet | #290; #292 (open, E9 transitions) | Not specced (rank 2) |
| #321 Paste from Sketch / Illustrator clipboards | Yes (only the Figma flavour is read, `app/src/clipboard.rs:28-31`) | M | Claude Sonnet | #308; REA of both flavours (Illustrator 2025/2026 installed locally, Sketch not) | Not specced (rank 3) |
| #320 Import .jam / .deck | Yes; `fig-jam.` magic already opens (`designos-fig/src/container.rs:76`, `:113`) | M | Claude Sonnet | #291, #317, #319 still open (E9 node types); #174 closed | Blocked by E9 (rank 4) |
| #849 Bundle an iOS UI kit as a library | Yes (no bundled kit) | ? | n/a | #418 licence gate (open) | Owner scope decision, not a build spec (rank 5) |

### Not specced: ranked

1. **#290 PPTX import** — L. Largest user value left (decks in from PowerPoint/Keynote/Google Slides). Needs real fixture decks and the `designos-interop/pptx` module (crate decided in `full-scope-e7-e8-e10.md:322`).
2. **#294 PPTX export** — L. Needs #290 for the re-import seam and #292 (open) for transitions.
3. **#321 Sketch / Illustrator paste** — M. Needs #308's mapper and a REA pass on each app's pasteboard flavours (global rule: use `rea`).
4. **#320 .jam / .deck import** — M. Blocked by E9 node types (#291, #317, #319).
5. **#849 iOS UI kit** — owner product/licence call (#418); not a build task until a licensed kit is chosen.

## Build order (file ownership: no two parallel legs share a file)

- Wave 1 (parallel, disjoint): #307 (designos-library + io pickers) · #891 guard test (designos-doc test file) · #834 CLI flag + smoke (cli.rs, main.rs, smoke script).
- Wave 2 (after PRs #1664 / #1671 / #1672 merge — they own designos-fig): #941 (designos-fig + paint.rs). Then #289, then #316 (hot-file queue: #289 is position 29, #316 position 44; both touch `designos-fig/src/clipboard.rs` → serial, never parallel).
- Wave 3: #308 → #309 (new crate; serial). #310 → #311 (shared extension + inbox; serial). Wave 3 lanes can run beside each other.

## Owner questions

Q1 (Q-289). **Paste from Figma: a pasted style has the same name as a different style already in the file (#289).** What happens?
- (a) **Recommended:** reuse the file's style when name and values both match; otherwise add the pasted one as "Brand/Primary 2". Reason: nothing changes colour by surprise, and no exact duplicates pile up.
- (b) Always link to the file's same-name style, so the pasted layers take the file's look.
- (c) Always add the pasted style under the same name, which leaves two styles with one name.

Q2 (Q-308). **Sketch import: which files define "done" (#308)?** Sketch is not installed on this Mac.
- (a) **Recommended:** you supply 2–3 real `.sketch` files you are allowed to use, and the builder adds small synthetic ones for edge cases. Reason: real files catch what our reading of the schema misses.
- (b) Synthetic files only, built from Sketch's published schema. This is quicker, but the tests can only check our own reading of the format.
- (c) Public `.sketch` files from the web, after a licence check per file.

Q3 (Q-309). **Sketch library symbols embedded in a file (#309).** How do they appear?
- (a) **Recommended:** as local components on a page named "Sketch libraries", one section per library. Reason: everything stays editable and nothing is missing.
- (b) As Studio library components. Studio has no library file to link them to, so each would be a dead link until one is made.
- (c) Detached into plain layers, so no components are created.

Q4 (Q-310a). **Chrome extension → Studio: how does the extension reach Studio (#310)?** The plan says "loopback with a token". The port registry gives this project 4103, but the promo-film preview server already uses it.
- (a) **Recommended:** loopback on a new fixed port, registered as Studio's app port (4103 stays the preview), with a token. Reason: it matches the plan and works in Chrome, Edge, Brave and Arc. The cost: it needs an explicit exception to the HARD RULE that each project owns exactly one localhost port.
- (b) Chrome native messaging. It needs no port and no token, and Chrome only lets our extension call it. The cost: Studio installs a small host file for each browser.
- Both options pin the extension ID with the manifest `key` field.

Q5 (Q-310b). **How do users install the extension (#310)?**
- (a) **Recommended:** an unlisted Chrome Web Store listing. Reason: one-click install and automatic updates. Google review takes time, and a developer account is needed.
- (b) "Load unpacked" from a folder inside the Studio app. There is no store, but users must turn on developer mode and repeat the steps after each update.

Q6 (Q-310c). **Pairing the extension with Studio (#310).**
- (a) **Recommended:** the first time, Studio asks "Allow the Chrome extension to add captures?", and the token is saved automatically. Reason: the user types nothing.
- (b) Studio shows a 6-digit code that the user types into the extension popup once.

Q7 (Q-311). **How does the extension resize the page for "Send at several widths" (#311)?** Default widths are decided as 375 / 768 / 1440 (your standard check widths), and the user can edit them.
- (a) **Recommended:** Chrome's debugger API. Reason: widths are exact, any width works, and the user's window stays as it is. The cost: Chrome cannot make this permission optional, so every user sees the stronger install warning, and Chrome shows a "debugging this browser" bar during the capture.
- (b) A temporary browser window sized to each width. The install warning stays mild, but widths below the OS minimum window size can be off by a few pixels, and the window flashes on screen.
- (c) Leave multi-width capture out of the extension. Users capture each width by hand, and Studio still lays out several captures side by side.

Q8 (Q-316). **Copy to Figma: when does Studio put the Figma payload on the clipboard (#316)?**
- (a) **Recommended:** on every Cmd+C, next to Studio's own data, with large selections encoded in the background. Reason: it pastes into Figma the same way Figma pastes into Studio, with no extra command to learn.
- (b) Only through a separate "Copy for Figma" command in the Edit and context menus. Normal copy costs nothing extra, but users must find the command.

## Facts gathered in this pass that other epics may need

- The kit's text uses Inter "Semi Bold" (×2,770), "Extra Bold" and "Extra Light", with spaces. Studio's `.fig` writer emits "SemiBold", "ExtraBold" and "ExtraLight" (`fig_export.rs:2897-2918`). Details are in the #941 spec.
- Ruleset "Protect main" requires `Test (macos-15), Clippy, Rustfmt, Check`. CLAUDE.md §Git Workflow still says `Test (macos-14)`, which is a stale line.
- `.project-agent.md` has no `localhost_port`, while the registry has 4103 for this project (promo-film preview). Per the port rule the two records should agree. That needs a separate fix and is out of E8 scope.

## User-visible choices made without asking (controller may fold into AskUserQuestion rounds)

- #834: new CLI flag `--export-png` (mirrors `--export-fig`). [spec](834-fig-save-roundtrip-pixel-gate.md)
- #941: Inter only uses Figma's spaced style names ("Semi Bold") when no source name exists; other families keep "SemiBold". [spec](941-fig-font-weight-roundtrip-style-names-axis.md)
- #289: paste toast wording ("Pasted from Figma (3 styles not linked, 1 link dropped)"); a payload variable alias to a missing variable becomes a literal value. [spec](289-figma-paste-merges-styles-variables-links.md)
- #307: error texts for a zip holding several `.fig` files, Sketch before #308 ("Sketch import is coming in a later release"), and Sketch ≤ 42 files. [spec](307-zip-entry-probe-routes-sketch-and-zipped-fig.md)
- #308: the import report goes into the existing import-warning toast; no new panel. Unsupported Sketch symbols stay detached until #309 ships. [spec](308-import-sketch-files-core.md)
- #309: Sketch swatches become one colour collection named "Sketch swatches". [spec](309-sketch-symbols-components-shared-styles-libraries.md)
- #310: a capture lands on the active tab's current page, centred, selected, one undo step. With no file open, Studio creates a new design file named after the page title. Wording for the extension popup states. [spec](310-chrome-extension-posts-captures-to-studio.md)
- #311: default widths are 375 / 768 / 1440. Frames are named "<title> — <width>", narrowest on the left, with an 80 px gap, top-aligned. Batches that mix URLs are refused. [spec](311-capture-several-breakpoints-into-sibling-frames.md)
- #316: "Copied for Studio only: <reason>" toast when the Figma encode fails; Figma text flavour = text of the selected text layers. [spec](316-copy-from-studio-to-figma-clipboard.md)

## Assumptions (confidence; what flips it)

- #834 AC-1/AC-2 status comes from the body of #1647 (0 pixels on "Welcome", 0 added override flags) and was not re-measured. Medium confidence; the first run of the new 4a pixel step either confirms or flips it.
- #941 R5 stale-axis rule: no file-version field was found, so `font_weight` wins only when the axis is a whole CSS step that disagrees with the weight. Medium confidence; a real v0.6.0 file whose axis is a deliberate whole-step value different from its weight would flip it.
- #310/#311: `debugger` cannot be optional and `key` pins an unpacked ID. Both are FACT from Chrome docs fetched 2026-11-06. That Emulation width override works without visible side effects beyond the debugging bar is an INFERENCE with medium confidence; the AC-4 Playwright run confirms or flips it.
- #289/#316: the clipboard buffer carries style and VARIABLE nodes, and Figma accepts a ZIP-wrapped buffer. Both are UNKNOWN, with low confidence either way; step 0 settles them.

## Unresolved (non-owner)

- Step 0 captures for #289 and #316 need a real Cmd+C in Figma.app on this Mac: a computer-use Cmd+C, or the owner does it by hand. The figma-agent plugin route is [UNVERIFIED], because the Plugin API probably cannot write a node copy to the system clipboard (INFERENCE).
- Two owner-manual Figma checks stay flagged and are never claimed by a builder: #941 AC-6 and #1346's 0.875 / 0.833 scale classes.
