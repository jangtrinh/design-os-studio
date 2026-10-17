# E7 Dev Mode & MCP, E8 Interop & Plugins, E10 Versions & Branching

Draft from the brainstormer agent, 2026-10-17. Status: DRAFT; owner rulings pending. Owner ruling after drafting: FULL SCOPE, no MVP, so every 'MVP', 'later' and 'non-goal' line below must become a task or an owner decision.

**Next action:** the owner rules on the 12 decisions below (D7.x, D8.x, D10.x). Each lists the recommended option first. All task drafts that don't depend on a decision can be filed as sub-issues right away.

### Epistemic Ledger
- **FACT:** I read the gap list, the brainstorm (A1–A13), REA §R4 and §Versions, the catalog, `CONTEXT.md` and issues #20, #21 and #23 (1 REST call). Every code line cited below was read in this session.
- **INFERENCE:** sizes, pack order, risks.
- **ASSUMPTION:** Figma Community plugins have no public download path. `@figma/plugin-typings` (public npm) defines the API surface. Size scale (worker-days): S ≤1d, M 2–4d, L 1–2 wk, XL >2 wk.
- **UNKNOWN:** the plugin sandbox internals (R3 was blocked), and whether the `.fig` import keeps Figma GUIDs. None was found in `node.rs`; only `"guid:s:l"` strings for links (`node.rs:141,147`).

Tags: **[F]** FACT, **[I]** INFERENCE, **[A]** ASSUMPTION.

**Not verified:** nothing was compiled or run, per the brief. Line numbers are from today's tree.

---

# E7 Dev Mode & MCP (#20)

## 1. What exists today
- **[F] MCP tools.** About 23 tools: 19 engine dispatch arms (`designos-mcp/src/engine.rs:39-369`) plus 4 server tools (`server.rs:614-628`: open, save, render_png, new_document).
  - Transport is stdio only (`main.rs:2,15`).
  - Docs disagree: README says 22, CLAUDE.md says 29.
- **[F] Screenshots.** `designos_render_png` writes a new file only; it returns no image content (`server.rs:465-488`).
- **[F] Code syntax already reaches MCP.** `designos_get_variables` already returns `code_syntax` (`engine.rs:94`). The gap row "codeSyntax in get_variables" is closed.
- **[F] Codegen languages.** `designos_generate_code` supports css, tailwind, react, swiftui, uikit, compose, android-xml and svg. Vue is rejected (`engine.rs:109-124`, test `engine.rs:957`).
  - The Dev Mode `Language::ALL` has 7 entries (`dev/mod.rs:31`).
- **[F] Code Connect is still a stub.** It moved to `panels/inspect.rs:501-552`.
  - "Connected" means an annotation contains `"Code Connect:"` (`:508`).
  - The snippet imports a non-existent `@designos/code-connect` package (`:530`).
- **[F] Dev model** (`designos-doc/src/dev.rs`):
  - `DevStatus` has 3 values (`:17`);
  - annotations carry 22 property labels (`:52`), versus Figma's 33;
  - `category` is a free string (`:42`), with no document-level categories and no measurements;
  - there is a ready-for-dev snapshot and fingerprint (`:87-105`), and `diff_node`/`diff_subtree` (`:333,395`).
- **[F] Dev Mode UI exists:** list, sections, annot, compare (Compare changes) and highlight (`app/src/dev/*.rs`).
  - Dev data is imported from REST only (`designos-fig/src/dev.rs:1-2`).
  - `annotationCategories` appears in a `fig.rs:737` skip list. **[I]** It is ignored in the `.fig` import.
- **[F] Live sync shares selection only indirectly.** The agent gets the frames that hold the selection (`live_canvas_scope.rs:17-22`), but no tool exposes "current selection".

## 2. Proposed scope
- **MVP pack E7.1, headline "Agent design context" (P2):**
  - `get_screenshot` returning image content;
  - `get_design_context` (code + hints + screenshot, with an XML fallback above 40k chars);
  - `get_metadata` as sparse XML;
  - selection as the default target;
  - the tool list becomes the single source for the docs.
  - **[I]** This is the highest leverage for the Studio Agent Runtime, and it touches no hot `.fig` files.
- **E7.2, headline "Code Connect for real" (P2):** a mapping store, the Inspect UI and 4 MCP tools.
- **E7.3, headline "Annotate & focus" (P2):** 33-property annotations, categories and measurements with `.fig`/REST round-trip; focus view; a dev status activity log.
- **Non-goals:**
  - native Vue/HTML/Web Components/Flutter codegen (R4: Figma has these only as Code Connect labels);
  - a 1:1 clone of all 41 remote MCP tools;
  - Streamable HTTP transport (stdio serves every target client, **[A]**);
  - VS Code extension (P3);
  - codegen plugins (these belong to E8).

## 3. Open decisions (owner)
- **D7.1 First headline?**
  - **(a) Agent design context (recommended):** M–L size, no `.fig` hot files, and it immediately improves every PTY agent.
  - (b) Code Connect first: more visible to developers, but it needs a storage decision (D7.2).
  - Trade-off: agent value now vs developer-handoff value now.
- **D7.2 Where do Code Connect mappings live?**
  - **(a) In the design document (recommended):** it works offline, travels with the file, and MCP reads it directly.
  - (b) In the code repo as `figma.connect`-style files, with Studio reading a configured repo path: devs own them and they are versioned with the code, but a repo must be configured.
  - (c) Both, (a) now and import of (b) later.
  - Trade-off: designer-owned vs dev-owned source of truth. Importing (b) requires Figma node GUIDs, which is UNKNOWN.
- **D7.3 Should MCP tools default to the editor selection when `node_id` is omitted?**
  - **(a) Yes when attached to a Studio window, otherwise `node_id` is required (recommended):** this is how Figma desktop MCP is used ("implement my selection").
  - (b) Always require `node_id`.
  - Trade-off: (a) couples to live sync (#75–#81 are open). (b) makes the agent search first.
- **D7.4 Close the gap row "Vue/HTML/Web Components/Flutter codegen"?**
  - **(a) Close it (recommended):** R4 shows Figma has no native codegen for these; agents and Code Connect cover them.
  - (b) Add Vue and HTML only (S each).
  - (c) Add all four.
  - Trade-off: parity purity vs marketing breadth. Each target is maintained forever.

## 4. Task breakdown

### Pack E7.1
- **T7.1 [M·P2] `designos_get_screenshot`**
  - Touches: `designos-mcp/src/server.rs` (next to render_png), `engine.rs` `tools()`.
  - Seam: `cargo test -p designos-mcp`, which calls the tool on a fixture.
  - AC:
    1. Returns MCP image content (`image/png`, base64) for `node_id` and scale 0.25–4.
    2. The decoded size equals node bounds × scale, ±1 px.
    3. Above a pixel cap the call returns an error naming the cap; nothing is truncated silently.
  - Deps: none.
- **T7.2 [S·P2] `designos_get_metadata`** (sparse XML: id, name, type, x, y, w, h, children; depth limit)
  - Touches: `engine.rs`.
  - Seam: unit test.
  - AC:
    1. The XML parses.
    2. Node count equals the subtree up to the depth limit.
    3. Hidden nodes are flagged, not dropped.
  - Deps: none.
- **T7.3 [L·P2] `designos_get_design_context`**
  - Content:
    - code in the requested or default language;
    - hints: variables used with `code_syntax`, styles, component props, annotations, dev status, and Code Connect when present;
    - a screenshot;
    - when the output is above 40k chars, sparse XML.
  - Touches: `engine.rs`; reuses `designos-app/src/dev/data.rs` logic, which must first move into `designos-export` or `designos-doc` so the app and MCP share it (DRY).
  - Seam: `cargo test -p designos-mcp`.
  - AC:
    1. On a fixture card, the output contains the CSS, the `var(--Color-Primary, …)` hint and one image.
    2. Above 40k chars it returns XML with `truncated:true`.
    3. The Inspect panel output is unchanged: the existing `dev/mod.rs` tests stay green.
  - Deps: T7.1, T7.2.
- **T7.4 [M·P2] Selection channel**
  - The editor publishes the selection IDs in the live scope; tools default to them (per D7.3).
  - Touches: `live_canvas_scope.rs`, `server.rs:194` (`sync_from_live_canvas_if_newer`).
  - Seam: `live_sync_tests.rs`.
  - AC:
    1. A tool called without `node_id` targets the current selection.
    2. A stale selection (node deleted) returns a clear error.
    3. A detached session still requires `node_id`.
  - Deps: T7.1; run after #75–#81 settle (same files).
- **T7.5 [S·P2] Tool-count truth**
  - A snapshot test of `tools()` names; README and CLAUDE.md quote the generated list.
  - Seam: unit test.
  - AC:
    1. The test fails when a tool is added without updating the snapshot.
    2. README and CLAUDE.md state the same number.
  - Deps: T7.1–T7.3.

### Pack E7.2
- **T7.6 [M·P2] Code Connect model in `Document`**
  - Per component: mappings `{label, component_name, source, snippet, imports, prop_map}`. Native format only, not `.fig`: R4 says Figma stores this server-side.
  - Touches: a new `designos-doc/src/code_connect.rs`; `Document` serde with `skip_serializing_if`.
  - Seam: doc unit tests.
  - AC:
    1. Native save → load round-trip is lossless.
    2. A legacy file without the field loads.
    3. Deleting the component main archives its mappings and the deletion is reported; nothing vanishes.
  - Deps: D7.2.
- **T7.7 [M·P2] Replace the Inspect stub** (`panels/inspect.rs:501-552`)
  - Connected state comes from the store. The snippet is shown with instance props substituted, plus a label tab per mapping. Remove the fake package and the annotation heuristic.
  - Seam: the existing `inspect_code_connect_*` test at `:997`, rewritten.
  - AC:
    1. No string `@designos/code-connect` remains.
    2. Instance props appear in the snippet.
    3. An unmapped component shows the empty state.
  - Deps: T7.6. Load es-designer.
- **T7.8 [M·P2] MCP Code Connect tools**
  - `get_code_connect_map`, `add_code_connect_map`, `get_code_connect_suggestions` (unmapped components plus name-matched candidates), `send_code_connect_mappings`.
  - Seam: `cargo test -p designos-mcp`.
  - AC:
    1. Add → get round-trips.
    2. Suggestions exclude components that are already mapped.
    3. Edits go through History (1-step undo).
  - Deps: T7.6.

### Pack E7.3
- **T7.9 [L·P2] Annotation parity**
  - 33 `AnnotationPropertyType` values, document `annotationCategories` (5 presets + 8 colors + custom), and `measurements=384`.
  - Touches: `designos-doc/src/dev.rs`, `fig.rs` (hot), `fig_export.rs`, `kiwi.rs` (hot), `designos-fig/src/dev.rs`, `rest_import.rs` (hot).
  - Seam: `.fig` round-trip tests in `testkit.rs`.
  - AC:
    1. A Figma-authored fixture with categories and measurements imports and exports with equal fields.
    2. Unknown property types pass through.
    3. Categories are no longer in the `fig.rs:737` skip list.
  - Deps: an REA read of the enum names. Serialize with E8 T8.4 (same hot files).
- **T7.10 [M·P2] Focus view**
  - A Ready-for-dev node opens isolated with status, Compare changes (reusing `dev/compare.rs`) and the activity log.
  - Seam: app shell test plus a screenshot.
  - AC:
    1. Opens from the Dev list.
    2. Only the focused subtree is rendered.
    3. Esc returns to the previous view.
  - Deps: T7.11, REA semantics. Load es-designer.
- **T7.11 [S·P2] Dev status activity log** (`prevStatus`, timestamps, description; fields of `sectionStatusInfo=355`)
  - Touches: `designos-doc/src/dev.rs`, `fig.rs`.
  - Seam: doc tests.
  - AC:
    1. Each status change appends an entry.
    2. The log round-trips through `.fig` import/export.
  - Deps: none.

## 5. Risks and unknowns
- **[I]** T7.4 collides with live-sync issues #75–#81 (`server.rs`, `live_*`). Sequence it after them.
- **[I]** In `get_design_context` on a 723k-node file, codegen and hints must stay bounded and off the UI thread.
- **REA still needed:**
  - the exact arguments of the desktop Code Connect tool schemas (in editor chunks);
  - the 33 `AnnotationPropertyType` names;
  - the shape of `measurements`;
  - focus view semantics and strings (en.json).
- **[A]** The remote MCP's 41-tool schemas are server-side, not in Figma.app. Use the public MCP docs instead of REA.

---

# E8 Interop & Plugins (#21)

## 1. What exists today
- **[F] A native plugin system already exists.** It is not Figma-compatible.
  - Bundles are installed only from `jangtrinh/design-os-plugins`. Figma Community URLs are explicitly rejected (`plugins/store.rs:72`, test `:466`).
  - "Installation never evaluates downloaded code" (`store.rs:1`).
  - Plugins run as a loopback HTTP panel (`plugins/runtime.rs:34-75`) under a strict CSP plus network origins from the manifest (`:49-58`), opened in the system browser (`plugins/mod.rs:114`).
  - The only host API is insert-SVG (`runtime.rs:20-25`).
  - There is no JS engine, and no webview crate in any `Cargo.toml` (grep, 0 hits).
- **[F] Clipboard paste of a Figma payload shipped in Wave A** (`designos-fig/src/clipboard.rs:52`, `app/src/clipboard.rs:28-92`).
  - Per A11, styles, variables and prototype links in the payload are not merged.
- **[F] Sketch and PPTX:** no code; grep finds 0 matches.
  - `zip` (deflate) is already a dependency of `designos-fig` (`Cargo.toml:21`).
  - `roxmltree` is in the app (`Cargo.toml:48`).
- **[F] Decks** are native slides built as frame trees (`decks/mod.rs:1`), with PDF export (`decks/export.rs`).
- **[F] Web capture:** none.

## 2. Proposed scope
- **MVP pack E8.1, headline "PPTX in and out" (P1, L):**
  - PPTX import and export for decks;
  - follow-ups: clipboard merges styles and variables;
  - a plugin runtime design doc and spike, not shipped and behind a flag.
  - **[I]** The plugin runtime cannot ship as a headline before the spike de-risks the sandbox.
- **E8.2, headline "Run Figma plugins" (P1, XL split into L-sized tasks):**
  - a QuickJS host;
  - the core of the `figma` API;
  - the showUI/postMessage bridge;
  - a Figma manifest loader;
  - a compatibility harness.
- **E8.3, headline "Open Sketch files" (P2, L):** Sketch import, plus plugin API breadth (codegen plugins, parameters).
- **E8.4, headline "Capture a web page" (P2, L):** a capture script plus an importer and an MCP tool.
- **Non-goals:**
  - installing from Figma Community (**[A]** no public download, and ToS);
  - widgets (P3);
  - Figma REST;
  - a bundled Chromium;
  - `.sketch` export;
  - Keynote;
  - `clientStorage` sync.

## 3. Open decisions (owner)
- **D8.1 What defines "plugin runtime MVP done"?**
  - **(a) A named list of 5–10 open-source Figma plugins you use, run unmodified (recommended):** it is measurable and it fixes the API surface to build.
  - (b) An API coverage % of `@figma/plugin-typings`: hard to verify, and it invites breadth over depth.
  - (c) Codegen / Dev Mode plugins first.
  - Trade-off: the size of the XL is set by this answer.
- **D8.2 How do Figma plugins get into the studio?**
  - **(a) "Import plugin from manifest…" from a local folder or cloned repo (recommended):** this is Figma's own development-plugin path, with no ToS exposure.
  - (b) Repackage plugins into the native registry: curated, but needs a licence review per plugin.
  - (c) Both.
  - Trade-off: users must obtain the plugin source themselves under (a).
- **D8.3 Where does a plugin's UI show?**
  - **(a) System browser for the MVP, embedded webview decided after the spike (recommended):** it reuses `runtime.rs` with zero new platform dependencies.
  - (b) Embedded webview now (wry/WKWebView; Linux needs WebKitGTK): Figma-like floating panel, heavy dependency.
  - Trade-off: shipping speed vs a window-switching UX.
- **D8.4 Plugin network access?**
  - **(a) Allow only the manifest's `networkAccess.allowedDomains`, after a one-time consent prompt per plugin (recommended):** mirrors Figma and keeps offline as the default.
  - (b) Block all network: breaks Unsplash/Iconify-type plugins.
  - (c) Allow all.
  - Trade-off: compatibility vs privacy.

## Technical decisions taken
- **JS engine:** QuickJS via `rquickjs` on a worker thread with memory and time limits. Mutations are marshalled to the UI thread with the existing `InsertRequest` pattern (`runtime.rs:20`). One plugin run is one History step.
- **API surface:** derived from `@figma/plugin-typings`. Unsupported calls throw a named error and are counted per plugin (Law 1: nothing vanishes silently).
- **Crate:** a new `designos-interop` crate with `pptx/`, `sketch/` and `webcapture/` modules. It reuses `zip` and `roxmltree`; no new archive or XML dependencies.
- **PPTX export:** editable text and shapes. Effects or features PPTX cannot express fall back to an embedded PNG, and each fallback is listed in the export report.
- **Web capture:**
  - the user's own Chrome or Playwright runs a capture script that produces a DOM snapshot as JSON;
  - an importer maps flexbox to auto layout and turns text and images into nodes;
  - no bundled browser; agents reach it through an MCP tool.
- **File probing:** the importer probes zip entries (`document.json`/`meta.json` vs `canvas.fig`) because both `.sketch` and zipped `.fig` start with the zip magic.

## 4. Task breakdown

### Pack E8.1
- **T8.1 [L·P1] PPTX import → native deck**
  - Content: slide size, sp geometry (preset + custom), text runs, pictures, groups, theme colors and fonts, speaker notes.
  - Touches: `designos-interop/pptx`, `decks/mod.rs`, the probe in `designos-library/src/store.rs`.
  - Seam: crate tests on fixture decks exported from PowerPoint, Keynote and Google Slides.
  - AC:
    1. Slide count and text content match the fixture.
    2. Unsupported elements become placeholders and the import report gives their count.
    3. A 100-slide deck imports in under 2 s on M-series (**[A]** budget).
  - Deps: fixture files (owner or LibreOffice).
- **T8.2 [L·P1] PPTX export from deck**
  - Touches: `designos-interop/pptx`, `decks/export.rs`.
  - Seam: export → re-import with T8.1, plus a PowerPoint/Keynote open check (screenshot).
  - AC:
    1. The file opens in Keynote/PowerPoint with no repair prompt.
    2. Text stays editable.
    3. Fallback PNGs are listed in the report.
  - Deps: T8.1.
- **T8.3 [M·P1] Plugin runtime design doc and spike** (not shipped)
  - Content: QuickJS host, a minimal `figma` global, one real open-source plugin end to end.
  - Touches: a new `app/src/plugins/figma_host/` behind a flag.
  - Seam: a spike test that runs the plugin headless.
  - AC:
    1. The chosen plugin creates nodes and the History has one step.
    2. The doc records sandbox semantics, limits and the security model.
    3. It ends with a go/no-go on D8.3.
  - Deps: D8.1, D8.2; the R3 REA retry.
- **T8.4 [M·P2] Clipboard follow-ups**
  - Content: merge styles and variables from the paste payload (A11 deferred this). Later: copy Studio → Figma by writing an HTML fig-kiwi payload.
  - Touches: `designos-fig/src/clipboard.rs`, `fig.rs` (hot), `fig_export.rs`, `app/src/clipboard.rs`.
  - Seam: fig tests with captured payload fixtures.
  - AC:
    1. A pasted node keeps its style link when a same-key style exists, and creates a local style otherwise.
    2. No duplicate styles appear on repeated pastes.
  - Deps: serialize with T7.9.

### Pack E8.2
- **T8.5 [L·P1] QuickJS host**
  - Content: worker thread, memory and time limits, message queue, cancel on close.
  - Seam: unit tests.
  - AC:
    1. An infinite loop is killed at the limit with a toast.
    2. Memory overrun is contained.
    3. Close cancels pending mutations.
  - Deps: T8.3.
- **T8.6a [L·P1] `figma` nodes API** (create, read and write props for FRAME, RECT, ELLIPSE, TEXT, VECTOR, GROUP; selection; currentPage; viewport; notify; closePlugin)
  - Seam: a JS test script suite.
  - AC: each D8.1 plugin's node calls succeed, and unsupported calls are counted.
  - Deps: T8.5.
- **T8.6b [L·P1] Styles, variables and components API** (COMPONENT, INSTANCE, variables, styles)
  - Deps: T8.6a.
- **T8.6c [M·P1] Fonts, images and export API** (`loadFontAsync`, `createImage`, `exportAsync`)
  - Deps: T8.6a.
- **T8.7 [M·P1] showUI / postMessage bridge**
  - Content: `parent.postMessage({pluginMessage})` both ways over loopback (`runtime.rs`).
  - Seam: a headless browser test.
  - AC: messages round-trip within 50 ms, and the CSP is kept.
  - Deps: T8.5, D8.3.
- **T8.8 [M·P1] Figma manifest loader** (`main`, `ui`, `editorType`, `networkAccess`, `menu`, `parameters`)
  - Touches: `plugins/store.rs` (a new source kind; the native registry is untouched).
  - AC:
    1. An invalid manifest is rejected with the field name.
    2. Menu entries show up under the Plugins menu.
    3. The network policy follows D8.4.
  - Deps: D8.2, D8.4.
- **T8.9 [M·P1] Compatibility harness**
  - Runs the D8.1 list headless and reports unsupported-API counts per plugin.
  - AC: a CI-runnable report, with the list pass rate as the pack gate.
  - Deps: T8.6a–T8.8.

### Pack E8.3
- **T8.10 [L·P2] Sketch import**
  - Content: `document.json`, `pages/*.json`, symbols → components, overrides, shared styles, `attributedString` text, booleans, images.
  - Touches: `designos-interop/sketch`, the probe in `store.rs`.
  - Seam: fixture `.sketch` files.
  - AC:
    1. Layer count and names match.
    2. Symbols become components and instances.
    3. Unsupported layers are counted in the import report.
- **T8.11 [M·P2] Plugin API breadth** (`figma.codegen`, `parameters`/quick actions, `relaunchData`)
  - Deps: T8.9.

### Pack E8.4
- **T8.12 [M·P2] Capture script** (JS: computed styles, boxes, text runs, images as data URLs)
  - Seam: Playwright on local HTML fixtures.
- **T8.13 [L·P2] Web-capture importer and the MCP tool `designos_import_web_capture`**
  - AC:
    1. A flex row becomes horizontal auto layout.
    2. Text stays editable.
    3. The screenshot diff against the source page is within a set threshold.
  - Deps: T8.12.

## 5. Risks and unknowns
- **[I]** API surface creep is the main XL risk. Without the D8.1 list, the runtime never finishes.
- **[F]** Issue #21: the R3 REA of the sandbox (`jsvm-cpp.wasm`) was blocked by the classifier.
  - Still needed: globals, ES level, timer availability, how the `figma` object is injected, and manifest validation rules.
  - Retry with a narrower read-only `rea` scope.
- **[A]** Figma's PPTX conversion and the web-capture extension are likely server-side or in the Chrome extension, not in Figma.app, so REA may find nothing locally.
- **[I]** The owner's "never html-to-figma" rule covers the Figma *workflow*, not this importer. Confirm it (see the questions at the end).
- **[I]** The probe collision between `.sketch` and zipped `.fig` could misroute imports. Test both.

---

# E10 Versions & Branching (#23)

## 1. What exists today
- **[F] Versions are full snapshots.** `versions/<file-id>/<n>.designos` plus an `index.json` with named and auto versions, and retention thinning (`designos-library/src/versions.rs:1-12,74`).
  - API: save, list, load, rename, delete (`:138-221`).
- **[F] App:** a history panel, view read-only, restore, banner and modal (`app/src/history/*`, `mod.rs:125-235`).
  - Restoring replaces the document wholesale (`history/mod.rs:4`).
- **[F] Diff building blocks:**
  - `dev::diff_node` / `diff_subtree` (`designos-doc/src/dev.rs:333,395`);
  - library `diff_snapshots` (`library_ops.rs:382`);
  - changelog `ChangeEntry` (`publish.rs:19`).
- **[F] `duplicate_file` is a byte copy** (`store.rs:528-544`), so NodeIds are preserved. Usable as a branch fork.
- **[F] Figma side (R5):** kiwi has no branch fields. Branching is server REST. The schema has `DiffPayload` (`diffType BRANCHING | NODE_CHANGES_ONLY`) and `Message.originFileKey=47`.
- **[F] Comments already have `author`** (`designos-doc/src/comments.rs:19`). How far multi-author comments go is UNKNOWN.

## 2. Proposed scope
- **MVP pack E10.1, headline "Branch and merge a file" (P2):**
  - branch metadata and storage;
  - a branch switcher;
  - a three-way diff;
  - a review screen;
  - merge with conflict resolution;
  - automatic versions before and after the merge as rollback.
- **E10.2:** update a branch from main; compare any two versions (the same engine).
- **E10.3 (depends on D10.1):** a git merge driver and diff CLI for `.designos` files.
- **Non-goals:**
  - hosted review requests, notifications and permissions;
  - real-time multiplayer (P3);
  - branches in `.fig` export (kiwi has no field; only the current branch's content exports);
  - nested folders (P3).

## 3. Open decisions (owner)
- **D10.1 Who are branches for?**
  - **(a) In-app branches now, a git merge driver as the next pack (recommended):** solo exploration ships first. For an offline-first product, git is the only realistic channel for async team work, and the same engine powers both.
  - (b) In-app only.
  - (c) Git driver only, no branch UI.
  - Challenge: for a solo designer, "Duplicate + Compare" already covers much of the exploration need. Without a team channel, merge value is limited.
- **D10.2 Conflict granularity?**
  - **(a) Per layer (recommended):** different properties of the same layer auto-merge; there is a conflict only when both sides changed the same property, and the user picks main or branch per conflicted layer. Fewest false conflicts; **[A]** close to Figma.
  - (b) Whole top-level frame pick: simplest, loses more work.
  - (c) Per-property pick UI: most control, heavy UI.
- **D10.3 Where do branches appear?**
  - **(a) Inside the file: a title-bar switcher, and Home shows the file once with a branch count (recommended).** Figma parity.
  - (b) As separate Home files with a "branch of" badge: simpler, but clutters Home.
- **D10.4 What happens to a merged branch?**
  - **(a) Archived, read-only and restorable (recommended):** Law 1, nothing vanishes silently.
  - (b) Deleted.
  - (c) Stays editable.

## Technical decisions taken
- A branch is a library file plus metadata `{branch_of, base_version, name, status}`.
- At fork time, a named version "Branch point" is saved on main and used as the merge base.
- **NodeId clash on merge.** Both sides allocate IDs from the same counter after a fork, so nodes added on the branch are remapped on merge. References are fixed up: instances, prototype links, style refs, variable bindings, comment anchors.
- The diff and merge are pure functions in `designos-doc`, run off the UI thread. Merge is one History step, with automatic versions on main before and after.

## 4. Task breakdown

### Pack E10.1
- **T10.1 [M·P2] Branch metadata and storage**
  - Touches: `designos-library/src/store.rs` (`FileEntry`), `versions.rs`.
  - Seam: library unit tests.
  - AC:
    1. Creating a branch saves the base version and a copy with the same NodeIds.
    2. Branches list and rename.
    3. Legacy indexes without the field load.
  - Deps: D10.3.
- **T10.2 [L·P2] Three-way diff engine** (`DiffPayload`-like serializable change set; covers nodes plus doc-level styles, variables and components)
  - Touches: a new `designos-doc/src/branch_diff.rs`; reuses `dev.rs:333` and `library_ops.rs:382`.
  - Seam: pure unit tests with base/main/branch fixtures.
  - AC:
    1. Add, remove, move and property changes are classified correctly.
    2. A conflict is reported only on a same-property double change (per D10.2).
    3. A 723k-node diff stays within a set time budget off the UI thread (**[A]**).
  - Deps: none.
- **T10.3 [L·P2] Merge with ID remap and conflict resolution**
  - Touches: `designos-doc`, `app/src/history/session.rs`.
  - Seam: doc tests plus an app shell test.
  - AC:
    1. Branch-added nodes get fresh IDs and every reference resolves.
    2. Resolved conflicts apply the chosen side.
    3. Automatic versions before and after the merge exist, and one undo reverts the merge.
  - Deps: T10.1, T10.2, D10.2.
- **T10.4 [M·P2] Branch UI** (title-bar switcher, create/rename/archive, Home branch count)
  - Touches: `app/src/history/`, title-bar shell, `home/main_view.rs`.
  - Seam: shell tests plus screenshots.
  - AC:
    1. Switching branches swaps the document without dirtying it.
    2. Merged branches show as archived (D10.4).
  - Deps: T10.1. Load es-designer.
- **T10.5 [M·P2] Review-before-merge screen** (changed frames with before/after thumbnails from the CPU renderer, conflict list)
  - Seam: screenshot plus a shell test.
  - AC:
    1. Every change in the change set is listed.
    2. Merge stays disabled until all conflicts are resolved.
  - Deps: T10.2, T10.4.

### Pack E10.2
- **T10.6 [M·P2] Update branch from main** (reverse merge, using the same engine)
  - Deps: T10.3.
- **T10.7 [S·P2] Compare any two versions** in the history panel
  - Deps: T10.2.

### Pack E10.3 (if D10.1 = a or c)
- **T10.8 [M·P2] `designos merge-driver %O %A %B` and `designos diff`**
  - Touches: `app/src/cli.rs`.
  - Seam: CLI tests in a temp git repo.
  - AC:
    1. A clean three-way merge exits 0.
    2. A conflict exits non-zero and writes a conflict report; the file is never left corrupt.
  - Deps: T10.2, T10.3.

## 5. Risks and unknowns
- **[I]** The NodeId clash is the main correctness risk. The remap must cover every cross-reference; a missed one silently breaks a link.
- **[I]** Branches are full byte copies. A 723k-node file doubles its disk use per branch: "copy-on-write" here is copy-on-create. Acceptable for the MVP; measure it.
- **[F]** History exists only for library files (`has_history`, `history/mod.rs:125`). Files opened from disk outside the library cannot branch until they are imported.
- **REA still needed:**
  - the `DiffPayload` schema fields;
  - branch and merge UI strings and conflict-flow semantics (en.json and editor chunks).
- **[A]** The merge algorithm itself is server-side and not readable locally.

---

## Cross-epic notes
- **Hot-file collisions [I]:**
  - T7.9 and T8.4 both touch `fig.rs`, `fig_export.rs` and `kiwi.rs`. Serialize them.
  - T7.4 overlaps live-sync issues #75–#81.
- **Suggested pack order [I]:**
  1. E7.1, alongside the non-shipping T8.3 spike.
  2. E8.1 (PPTX).
  3. E8.2 (plugin runtime).
  4. E10.1.
  5. Then E7.2, E8.3, E7.3, E10.2, E8.4.
- **Assumptions to confirm:**
  - `@figma/plugin-typings` is enough to define the API surface (high). Flips if the sandbox exposes undocumented globals that plugins rely on.
  - Community plugins cannot be downloaded (medium). Flips if a public bundle URL exists.
  - The 723k-node diff budget is reachable off-thread (medium). Flips on the first benchmark.

## Unresolved questions
- Does "web capture" conflict with the owner's no-html-to-figma rule, or does that rule only cover the Figma workflow?
- PPTX appears in both E8 and E9 in the gap list. Confirm E8 owns it.
- How far do multi-author comments go (listed under E10 in the gap list, absent from #23)? In scope or not?
- Which open-source Figma plugins make up the D8.1 list?

Status: DONE_WITH_CONCERNS
Summary: Drafted the brainstorm, decisions and pack-ordered task breakdown for E7, E8 and E10, grounded in code. The gap row "codeSyntax in get_variables" is already closed (`engine.rs:94`). A native plugin system with a browser-panel UI and insert-SVG only already exists.
Concerns/Blockers: Nothing was compiled or run (per the brief). The plugin sandbox (R3) is still unread. The merge NodeId clash and the plugin API scope (D8.1) are the main risks.