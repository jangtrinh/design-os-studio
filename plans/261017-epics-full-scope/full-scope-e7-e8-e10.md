# E7 Dev Mode & MCP, E8 Interop & Plugins, E10 Versions & Branching: FULL SCOPE

Brainstormer revision, 2026-10-17, under the owner ruling: full scope, no MVP. Status: owner decisions pending. Supersedes draft-e7-e8-e10.md.

**Next action:** the owner rules on the 12 decisions below (D7.x, D8.x, D10.x). Each lists the recommended option first. Tasks that don't depend on a decision can be filed now.

This revision applies the owner ruling of 2026-10-17. It plans the full scope of each epic; packs only set the delivery order. The only exclusions are items that are out of the product by nature, and each one is listed as an owner decision (D7.1, D8.1, D10.1). Nothing is cut for effort.

### Epistemic Ledger
- **FACT:** I read the gap list, the catalog (§10–14), REA §R4 and §Versions, the brainstorm, `CONTEXT.md` and issues #20, #21 and #23. Every code line cited below was read in this session.
- **INFERENCE:** sizes, pack order, risks.
- **ASSUMPTION:**
  - Figma Community plugins have no public download.
  - `@figma/plugin-typings` (public npm) defines the API surface.
  - `rmcp` supports Streamable HTTP.
  - Size scale (worker-days): S ≤1d, M 2–4d, L 1–2 wk, XL >2 wk.
- **UNKNOWN:**
  - the plugin sandbox internals (R3 was blocked);
  - whether the `.fig` import keeps Figma GUIDs (none found in `node.rs`);
  - whether `.jam`/`.deck` files are probed (not found in `store.rs` or `fig/lib.rs`);
  - the exact list of Figma's 41 MCP tools.

Tags: **[F]** FACT, **[I]** INFERENCE, **[A]** ASSUMPTION.

**Not verified:** nothing was compiled or run, per the brief.

Task format: `ID [size·priority] title`, then Touches, Seam (the test seam), AC, Deps. Hot files are marked 🔥.

---

# E7 Dev Mode & MCP (#20)

## 1. What exists today
- **[F] MCP tools:** about 23. There are 19 engine dispatch arms (`designos-mcp/src/engine.rs:39-369`) plus 4 server tools (`server.rs:614-628`), over stdio only (`main.rs:2,15`).
  - Docs disagree: README says 22, CLAUDE.md says 29.
- **[F] Screenshots:** `render_png` writes a file only; it returns no image content (`server.rs:465-488`).
- **[F] Code syntax is already exposed in MCP** (`engine.rs:94`) and editable in the variables UI (`variables/view.rs`).
- **[F] Codegen:** css, tailwind, react, swiftui, uikit, compose, android-xml and svg (`engine.rs:109-124`); 7 languages in the Inspect panel (`dev/mod.rs:31`).
- **[F] Code Connect is a stub** at `panels/inspect.rs:501-552`.
  - "Connected" means an annotation contains `"Code Connect:"` (`:508`).
  - The snippet imports a fake `@designos/code-connect` package (`:530`).
- **[F] Dev model** (`designos-doc/src/dev.rs`):
  - `DevStatus` has 3 values (`:17`);
  - `DevLink` covers dev resources (`:35`);
  - annotations carry 22 property labels (`:52`); `category` is a free string;
  - there is a ready-for-dev snapshot and fingerprint (`:87-105`), and `diff_node`/`diff_subtree` (`:333,395`).
- **[F] Dev Mode UI:**
  - list, sections, annot, Compare changes and highlight (`app/src/dev/*`);
  - a "Dev Mode Plugins & Codegen" panel header (`panels/right.rs:186`);
  - a PRODUCT:OS spec reader (`app/src/specs.rs:1`).
- **[F] Dev data import:** from REST only (`designos-fig/src/dev.rs:1-2`). `annotationCategories` is in a `fig.rs:737` skip list.
- **[F] Live sync** shares the frames that hold the selection (`live_canvas_scope.rs:17-22`), but no tool exposes "current selection".

## 2. Full scope
The epic covers everything in gap-list E7 (Code Connect, Vue/HTML/Web Components/Flutter codegen, focus view, MCP parity, code syntax) and catalog §10:
- inspect and measurements;
- annotations;
- statuses and notifications;
- Compare changes (exists);
- focus view and interactive inspection;
- dev resources (exists, plus `.fig` round-trip);
- codegen plugins (delivered by E8 T8.17, hosted in this panel);
- VS Code, Jira and Google Chat integrations;
- Code Connect: CLI, UI, every label, multiple mappings;
- MCP with full tool parity, canvas writes, code-to-canvas and a desktop (HTTP) server;
- Specs.

Out-of-product-by-nature candidates go to the owner in D7.1.

## 3. Open decisions (owner)
- **D7.1 Exclusions by nature.** Candidates: Figma-hosted Dev Mode webhooks, and a hosted remote MCP endpoint.
  - **(a) Exclude both; build Jira, Google Chat and the VS Code extension as opt-in local or network add-ons, plus a local outgoing status hook (recommended):** none of these is Figma-hosted, and offline stays the default.
  - (b) Exclude both, plus the Jira and Google Chat integrations.
  - (c) Exclude nothing: a self-hosted webhook relay. This conflicts with offline-first.
- **D7.2 Code Connect source of truth when a code repo is linked.**
  - **(a) Repo files win; the document keeps a synced copy, and UI edits of repo-backed mappings are disabled with "Edit in code" (recommended):** Figma parity, and the same pattern as ruling A13.
  - (b) The document wins and the CLI only seeds it.
  - Trade-off: developer-owned vs designer-owned; (b) risks silent drift from the code.
- **D7.3 Should MCP tools default to the editor selection when `node_id` is omitted?**
  - **(a) Yes when attached to a Studio window (recommended):** the Figma desktop MCP workflow.
  - (b) Always require `node_id`.
  - Trade-off: (a) couples to live sync (#75–#81).
- **D7.4 First pack headline?**
  - **(a) Agent design context (recommended):** M–L size, no hot `.fig` files, and every PTY agent gains from it immediately.
  - (b) Code Connect first.

## 4. Task breakdown

### Pack E7.1: Agent design context
- **T7.1 [M·P2] `designos_get_screenshot`**
  - Touches: `server.rs`, `engine.rs` `tools()`.
  - Seam: `cargo test -p designos-mcp`.
  - AC:
    1. Returns image content (`image/png`, base64) for a node at scale 0.25–4.
    2. Size equals bounds × scale, ±1 px.
    3. Above the pixel cap, a named error; nothing is truncated silently.
- **T7.2 [S·P2] `designos_get_metadata`** (sparse XML)
  - Seam: unit test.
  - AC:
    1. The XML parses.
    2. Node count equals the subtree up to the depth limit.
    3. Hidden nodes are flagged.
- **T7.3 [M·P2] Move the design-context data into a shared crate**
  - Move `app/src/dev/data.rs` logic into `designos-export` so the app and MCP share it.
  - Seam: the existing `dev/mod.rs` tests stay green.
  - AC: no logic is duplicated, and the Inspect panel output is unchanged.
- **T7.4 [L·P2] `designos_get_design_context`**
  - Content: code, hints (variables with `code_syntax`, styles, props, annotations, status, Code Connect) and a screenshot. Above 40k chars, an XML fallback.
  - AC:
    1. A fixture card yields CSS, a `var(--Color-Primary, …)` hint and one image.
    2. Above 40k chars: XML with `truncated:true`.
  - Deps: T7.1–T7.3.
- **T7.5 [M·P2] Selection channel**
  - Touches: `live_canvas_scope.rs`, `server.rs:194`.
  - Seam: `live_sync_tests.rs`.
  - AC:
    1. Calls without `node_id` target the selection (per D7.3).
    2. A stale selection gives a clear error.
    3. Detached sessions require `node_id`.
  - Deps: T7.1, and #75–#81 settled.
- **T7.6 [S·P2] MCP parity matrix and tool-list truth**
  - Build the matrix from the public Figma MCP docs. Add a snapshot test of `tools()`; README and CLAUDE.md quote the generated list.
  - AC:
    1. Every Figma tool row maps to a studio tool or a task ID.
    2. The snapshot fails when the tool list drifts.

### Pack E7.2: Desktop MCP and remaining tools
- **T7.7 [M·P2] Streamable HTTP transport**
  - Bind 127.0.0.1 on the port assigned in `.project-agent.md` or the registry (port contract), behind a session token.
  - AC:
    1. Binds only on loopback and the assigned port.
    2. A request without the token gets 401.
    3. stdio still works.
  - Deps: a port allocation.
- **T7.8 [M·P2] `download_assets`** (images and vectors of a subtree into a workspace dir)
  - AC:
    1. Every image fill and every exportable vector is written once (deduplicated by hash).
    2. Paths stay confined to the workspace.
- **T7.9 [M·P2] `search_design_system`** (components, styles and variables across local and enabled libraries)
  - AC:
    1. Ranked results include library origin.
    2. Disabled libraries are excluded.
- **T7.10 [S·P2] `set_codegen_config`** (per-file language and units, persisted)
  - AC: the setting persists in native save, and `generate_code` and `get_design_context` honor it.
- **T7.11 [M·P2] `get_figjam`** (board stickies, connectors, sections, shapes as structured content)
  - AC: a board fixture returns every node type, and connector endpoints resolve.
- **T7.12 [M·P2] `get_motion_context`**
  - Deps: the E6 motion model.
- **T7.13 [M·P2] `use_figma` parity: `designos_run_plugin_script`**
  - Runs Figma-Plugin-API JS against the document; one call is one History step.
  - Deps: E8 T8.8 and T8.9.
- **T7.14 [S·P2] Code-to-canvas: `designos_import_web_capture` / `generate_design`**
  - Deps: E8 T8.27.

### Pack E7.3: Code Connect
- **T7.15 [M·P2] Code Connect model in `Document`**
  - Multiple `{label, component_name, source, snippet, imports, prop_map}` mappings per component.
  - Touches: a new `designos-doc/src/code_connect.rs`.
  - AC:
    1. Lossless native round-trip.
    2. Legacy files load.
    3. Deleting a component archives its mappings and reports them.
- **T7.16 [M·P2] Replace the Inspect stub** (`panels/inspect.rs:501-552`)
  - Label tabs, snippet with instance props substituted.
  - Seam: the rewritten `:997` test plus a screenshot.
  - AC:
    1. No `@designos/code-connect` string remains.
    2. Instance props appear in the snippet.
    3. Unmapped components show the empty state.
  - Deps: T7.15. Load es-designer.
- **T7.17 [M·P2] Prop-mapping helpers** (the equivalents of enum, boolean, string, instance, children and textContent)
  - AC: each helper is covered by a fixture; variant changes update the snippet.
- **T7.18 [M·P2] MCP Code Connect tools** (get map, add map, suggestions, send mappings)
  - AC:
    1. Add → get round-trips.
    2. Suggestions skip components that are already mapped.
    3. One undo step per add.
  - Deps: T7.15.
- **T7.19 [M·P2] Keep Figma node GUIDs on import**
  - Touches: 🔥 `fig.rs`, 🔥 `kiwi.rs`, 🔥 `rest_import.rs`, `fig_export.rs`.
  - AC:
    1. The GUID survives `.fig` and REST import and re-export.
    2. A Figma node URL resolves to a studio node.
- **T7.20 [L·P2] `designos code-connect` CLI**
  - Parses `*.figma.tsx`, `.swift`, `.kt` and html files in a repo; publishes and unpublishes into the document (per D7.2).
  - AC:
    1. Figma's sample repo publishes every mapping.
    2. Re-running is idempotent.
    3. Parse errors name the file and line.
  - Deps: T7.15, T7.19.
- **T7.21 [M·P2] Connect-component UI** (pick a source file for an unmapped component, name-matched suggestions)
  - Deps: T7.16, T7.18. Load es-designer.

### Pack E7.4: Annotate, focus, notify
- **T7.22 [L·P2] Annotation model and UI**
  - 33 property types, document categories (5 presets + 8 colors + custom), measurements.
  - Touches: `designos-doc/src/dev.rs`, `app/src/dev/annot.rs`, `sections.rs`.
  - AC:
    1. All 33 types are selectable.
    2. Category colors render.
    3. Measurements draw and update when nodes move.
  - Deps: REA enum names. Load es-designer.
- **T7.23 [L·P2] Dev data round-trip** (annotations, categories, measurements, `sectionStatusInfo=355`, dev resources)
  - Touches: 🔥 `fig.rs`, 🔥 `kiwi.rs`, `fig_export.rs`, `designos-fig/src/dev.rs`, 🔥 `rest_import.rs`.
  - Seam: `testkit.rs` round-trip.
  - AC:
    1. A Figma-authored fixture round-trips with equal fields.
    2. Categories leave the `fig.rs:737` skip list.
  - Deps: T7.22.
- **T7.24 [S·P2] Dev status activity log** (prevStatus, timestamp, description)
  - AC: each change appends an entry, and the log round-trips via T7.23.
- **T7.25 [L·P2] Focus view and interactive inspection**
  - Isolated subtree, Compare changes, log, and playable variants and prototype interactions.
  - AC:
    1. Opens from the Dev list.
    2. Only the subtree renders.
    3. Variant switches play.
    4. Esc returns.
  - Deps: T7.24. Load es-designer.
- **T7.26 [M·P2] Local status notifications** (in-app inbox of status changes across library files; optional OS notification)
  - AC:
    1. A status change in file A shows in the inbox while file B is open.
    2. Mark-read persists.
- **T7.27 [M·P2] Specs authoring**
  - Link written intent to frames, edit in-app, and add an MCP `get_specs` tool, building on `specs.rs`.
  - AC:
    1. A spec links to a frame and the link survives save.
    2. MCP returns the specs for a node.

### Pack E7.5: Extra codegen targets
- **T7.28 [S·P1] Vue target**
- **T7.29 [S·P1] HTML target**
- **T7.30 [S·P1] Web Components target**
- **T7.31 [M·P1] Flutter target**
- For all four:
  - Touches: `designos-export/src/<lang>.rs`, `Language::ALL`, the MCP language list.
  - Seam: `designos-export` snapshot tests.
  - AC:
    1. The fixture card output compiles or validates (vue-tsc, html-validate, tsc, `flutter analyze`, **[A]** these tools are available).
    2. Bound variables print `code_syntax` per A5 and A7.

### Pack E7.6: Integrations (per D7.1)
- **T7.32 [L·P3] VS Code extension**
  - Connects to `designos-mcp` (stdio or HTTP); inspect, Code Connect and selection.
  - AC: installs from a `.vsix`; shows the design context of the selected node.
  - Deps: T7.4, T7.7.
- **T7.33 [M·P3] Jira link previews and Google Chat status posts** (opt-in network, per-integration consent)
  - AC: off by default; no request is made before consent.
- **T7.34 [S·P3] Local outgoing status hook** (a user-configured POST on status change)
  - AC: off by default; failures are logged and visible.
  - Deps: D7.1.

## 5. Risks and unknowns
- **[I]** T7.5 and T7.7 overlap live-sync issues #75–#81.
- **[I]** T7.19 and T7.23 hit the same hot files as E8 T8.4 and T8.29. Serialize them.
- **[I]** `get_design_context` on a 723k-node file must stay bounded and off the UI thread.
- **[A]** T7.7 assumes `rmcp` supports Streamable HTTP.
- **REA still needed:**
  - the 33 `AnnotationPropertyType` names;
  - the shape of `measurements`;
  - the desktop Code Connect tool argument schemas;
  - focus view semantics and strings.
- The 41-tool list lives in the public docs, not in Figma.app.

---

# E8 Interop & Plugins (#21)

## 1. What exists today
- **[F] A native plugin system exists:**
  - installs only from the native registry; Figma URLs are rejected (`plugins/store.rs:72`, test `:466`);
  - no code is evaluated at install (`:1`);
  - loopback panel under CSP plus manifest network origins (`plugins/runtime.rs:34-75`), opened in the system browser (`plugins/mod.rs:114`);
  - the only host API is insert-SVG (`runtime.rs:20-25`);
  - no JS engine and no webview crate.
- **[F] Figma clipboard paste shipped** (`designos-fig/src/clipboard.rs:52`, `app/src/clipboard.rs:28-92`). Per A11, styles, variables and links are not merged.
- **[F] No Sketch or PPTX code.** `zip` is already a dependency (`designos-fig/Cargo.toml:21`), and `roxmltree` is in the app (`Cargo.toml:48`).
- **[F] Decks** are frame trees (`decks/mod.rs:1`).
- **[F] Widget kiwi fields** are in the `fig.rs:735-740` skip list.
- **[F] SVG paste and import exist** (inventory).

## 2. Full scope
The epic covers everything in gap-list E8 (Plugin API runtime, Sketch, PPTX, web capture, Figma clipboard) and catalog §12 and §14:
- the full Plugin API (update 141): nodes, styles, variables, components, team library, fonts, images, export, events, clientStorage, parameters, relaunch data, menus, network, FigJam and Slides editor types, Dev Mode and codegen plugins, text review, util, timer;
- widgets;
- generative plugins built by the agent;
- Sketch import;
- PPTX import and export;
- web capture: script, Chrome extension, responsive capture, MCP;
- the clipboard both ways;
- `.jam`/`.deck` import;
- paste from other tools.

## 3. Open decisions (owner)
- **D8.1 Exclusions by nature.** Candidates:
  - Community publishing and installing from Figma Community;
  - the plugin payments API;
  - Figma REST, webhooks and Plan Access Tokens;
  - importing Figma Make output;
  - multiplayer plugin APIs (`activeUsers`, `currentUser` server identity).

  Options:
  - **(a) Exclude the Figma-hosted items; stub the multiplayer and payments APIs to a single local user and a "free/unpaid" status (recommended):** plugins that call them still run.
  - (b) Exclude them, and calls throw a named "unsupported" error.
  - (c) Additionally offer the native registry as a local "community" channel (it exists today).
- **D8.2 How do Figma plugins get in?**
  - **(a) "Import plugin from manifest…" from a local folder or cloned repo (recommended):** Figma's own development path, with no ToS exposure.
  - (b) Repackage into the native registry: needs a licence review per plugin.
  - (c) Both.
- **D8.3 Plugin UI host.**
  - **(a) Embedded webview window (wry/WKWebView), with the system browser as fallback where no webview exists (recommended):** a floating panel as in Figma; the fallback is the existing `runtime.rs`.
  - (b) System browser only: no new dependencies, but window-switching UX.
  - Trade-off: on Linux, (a) needs WebKitGTK.
- **D8.4 Plugin network access.**
  - **(a) Only the manifest's `allowedDomains`, after a one-time consent prompt per plugin (recommended):** mirrors Figma, offline stays the default.
  - (b) Block all.
  - (c) Allow all.

## Technical decisions taken
- QuickJS (`rquickjs`) on a worker thread with memory and time limits. Mutations are marshalled to the UI thread (the `InsertRequest` pattern). One plugin run is one History step.
- The API surface comes from `@figma/plugin-typings`. Unsupported calls throw a named error and are counted.
- A new `designos-interop` crate (`pptx/`, `sketch/`, `webcapture/`), reusing `zip` and `roxmltree`.
- The file probe inspects zip entries, because `.sketch` and zipped `.fig` share the zip magic.
- PPTX export: editable text and shapes; anything else falls back to an embedded PNG, listed in the report.
- Web capture has no bundled browser.

## 4. Task breakdown

### Pack E8.1: PPTX in and out
- **T8.1 [L·P1] PPTX import → deck**
  - Content: slide size, preset and custom geometry, text runs, pictures, groups, theme, notes.
  - Touches: `designos-interop/pptx`, `decks/mod.rs`, the probe in `designos-library/src/store.rs`.
  - Seam: fixture decks from PowerPoint, Keynote and Google Slides.
  - AC:
    1. Slide count and text match.
    2. Unsupported elements are counted in the import report.
    3. 100 slides import in under 2 s (**[A]** budget).
- **T8.2 [L·P1] PPTX export**
  - Seam: export → re-import with T8.1, plus opening in Keynote or PowerPoint (screenshot).
  - AC:
    1. Opens with no repair prompt.
    2. Text stays editable.
    3. Fallbacks are listed.
  - Deps: T8.1.
- **T8.3 [M·P1] Plugin runtime design doc and spike** (not shipped, behind a flag)
  - AC:
    1. One real open-source plugin runs end to end headless.
    2. The doc fixes sandbox limits and the security model.
  - Deps: the R3 REA retry.
- **T8.4 [M·P1] Paste follow-ups:** merge styles, variables, prototype links and library refs from the Figma payload
  - Touches: `designos-fig/src/clipboard.rs`, 🔥 `fig.rs`, `app/src/clipboard.rs`.
  - AC:
    1. A same-key style is linked, otherwise a local style is created.
    2. Repeated pastes create no duplicates.

### Pack E8.2: Run Figma plugins
- **T8.5 [L·P1] QuickJS host**
  - AC:
    1. An infinite loop is killed at the limit, with a toast.
    2. Memory overrun is contained.
    3. Close cancels pending mutations.
  - Deps: T8.3.
- **T8.6 [M·P1] Figma manifest loader** (`main`, `ui`, `editorType`, `menu`, `parameters`, `networkAccess`) from a local folder or repo
  - Touches: `plugins/store.rs` (a new source kind).
  - AC:
    1. An invalid field is named in the error.
    2. Menu entries appear.
  - Deps: D8.2.
- **T8.7 [L·P1] UI host and `postMessage` bridge**
  - AC:
    1. Messages round-trip in under 50 ms.
    2. CSP is kept.
    3. The fallback path works.
  - Deps: D8.3, T8.5.
- **T8.8 [L·P1] Core nodes API** (create, read, write for FRAME, RECT, ELLIPSE, LINE, POLYGON, STAR, VECTOR, TEXT, GROUP, SECTION, BOOLEAN; selection; currentPage; viewport; notify; closePlugin)
  - Seam: a JS test suite.
  - AC: every call is covered by a script test.
  - Deps: T8.5.
- **T8.9 [M·P1] Events and undo grouping** (selectionchange, documentchange, currentpagechange, run, close)
  - Deps: T8.8.
- **T8.10 [M·P1] Fonts, images and export** (`loadFontAsync`, `createImage`, `exportAsync`)
  - Deps: T8.8.
- **T8.11 [M·P1] Compatibility harness**
  - Runs a corpus of open-source plugins headless and reports typings coverage and unsupported calls.
  - AC: a CI report; it is the gate for every later E8 pack.
  - Deps: T8.6–T8.10.

### Pack E8.3: Full Plugin API
- **T8.12 [L·P1] Styles, variables, components, instances, component props API**
  - Deps: T8.8.
- **T8.13 [M·P1] `teamLibrary` API → local published libraries**
  - Deps: T8.12.
- **T8.14 [M·P1] `clientStorage` (local), parameters/quick actions, relaunch data**
  - Deps: T8.6.
- **T8.15 [S·P1] Network policy and consent UI**
  - Deps: D8.4, T8.7.
- **T8.16 [L·P1] FigJam and Slides editor types** (stickies, connectors, shapes-with-text, slide APIs)
  - Deps: T8.8.
- **T8.17 [L·P1] Dev Mode plugins:** `figma.codegen` and inspect-panel plugins in the "Plugins & Codegen" panel (`panels/right.rs:186`)
  - AC: a codegen plugin's output appears as a language tab.
  - Deps: T8.8.
- **T8.18 [L·P1] Remaining surface:** text review, `util`, `timer`, misc
  - AC: the T8.11 coverage report shows 100% of typings, minus the D8.1 exclusions.
  - Deps: T8.12–T8.17.
- **T8.19 [S·P1] Stubs or errors for the excluded APIs, per D8.1**

### Pack E8.4: Widgets and agent-built plugins
- **T8.20a [L·P3] Widget runtime and render** (JSX widget API on the QuickJS host)
  - Deps: T8.8.
- **T8.20b [M·P3] Widget synced state, property menu and interaction** (local, single user)
  - Deps: T8.20a.
- **T8.20c [M·P3] Widget node `.fig` import and export** (`widgetSyncedState`, `widgetMetadata`)
  - Touches: 🔥 `fig.rs`, 🔥 `kiwi.rs`, `fig_export.rs`.
  - Deps: T8.20b.
- **T8.21 [M·P3] Generative plugins**
  - The agent scaffolds a plugin in a local folder and runs it via MCP `designos_run_plugin`.
  - AC: the agent creates and runs a plugin from a prompt with no manual steps.
  - Deps: T8.11, E7 T7.13.

### Pack E8.5: Sketch
- **T8.22 [L·P2] Sketch import core** (pages, layers, groups, shapes, booleans, `attributedString` text, images)
  - Seam: fixture `.sketch` files.
  - AC: layer count and names match; unsupported layers are counted.
- **T8.23 [L·P2] Symbols → components, overrides, shared styles, Sketch libraries**
  - Deps: T8.22.
- **T8.24 [S·P2] Zip-entry file probe and Home import flow** (`store.rs`)
  - AC: `.sketch` and zipped `.fig` both route correctly.

### Pack E8.6: Web capture
- **T8.25 [M·P2] Capture script** (computed styles, boxes, text runs, data-URL images)
  - Seam: Playwright on local fixtures.
- **T8.26 [M·P2] Chrome extension** that posts a capture to the Studio loopback with a token
  - AC: a token is required; captures land on the open file.
  - Deps: T8.25.
- **T8.27 [L·P2] Importer:** flex → auto layout, editable text
  - AC: the screenshot diff against the source page is within threshold.
  - Deps: T8.25.
- **T8.28 [M·P2] Responsive capture:** several breakpoints become sibling frames
  - Deps: T8.27.

### Pack E8.7: Clipboard both ways and other formats
- **T8.29 [M·P2] Copy Studio → Figma** (write the HTML fig-kiwi payload)
  - Touches: 🔥 `fig_export.rs`, `clipboard.rs`.
  - AC: the payload pastes into Figma.app (screenshot).
- **T8.30 [M·P2] `.jam` / `.deck` import** (verify the probe, then map board and slide nodes)
  - Deps: E9 node types.
- **T8.31 [M·P3] Paste from other tools** (Sketch and Illustrator clipboard flavors)
  - Deps: T8.22.

## 5. Risks and unknowns
- **[I]** The XL surface is gated by the T8.11 coverage report. Without that report, "done" cannot be measured.
- **[F]** The R3 REA of `jsvm-cpp.wasm` was blocked (#21). Still needed: globals, ES level, timers, how `figma` is injected, manifest validation, widget runtime semantics.
- **[A]** Figma's PPTX conversion and the capture extension may be server-side or in the extension, so REA may find nothing locally.
- **[I]** Hot-file collisions: T8.4, T8.20c and T8.29 vs E7 T7.19 and T7.23.

---

# E10 Versions & Branching (#23)

## 1. What exists today
- **[F] Versions are full snapshots** with named and auto entries and retention (`designos-library/src/versions.rs:1-12,74,138-221`).
- **[F] App:** history panel, view, restore, banner (`app/src/history/*`). Restore replaces the whole document (`mod.rs:4`). History works only for library files (`mod.rs:125`).
- **[F] Diff building blocks:** `dev.rs:333,395`, `library_ops.rs:382`, `publish.rs:19`.
- **[F] `duplicate_file` is a byte copy** (`store.rs:528-544`), so NodeIds are preserved.
- **[F] Figma side (R5):** kiwi has no branch fields. Server REST handles branches; the schema has `DiffPayload` (`BRANCHING | NODE_CHANGES_ONLY`) and `originFileKey=47`.
- **[F] Comments** have a free-text `author` display name (`designos-doc/src/comments.rs:16-24`).
- **[F] Folders** have a `parent` field, so nesting exists in the model (`store.rs:26-30`). UI depth limit: UNKNOWN.

## 2. Full scope
The epic covers everything in gap-list E10 (branching and merging, nested folders, multi-author comments; tab groups added from the P3 row it shares with nested folders, **[I]**) and catalog §13:
- branch, review request, pull from main, merge;
- version history with auto, named and branch checkpoints;
- nested folders 10 deep, folder preview;
- workspace sharing, delivered as a local equivalent.

Real-time multiplayer and hosted collaboration go to the owner in D10.1.

## 3. Open decisions (owner)
- **D10.1 Exclusions by nature.** Candidates:
  - real-time multiplayer, cursors, cursor chat, spotlight, observation;
  - hosted review notifications;
  - Figma-hosted workspace sharing;
  - viewer history.

  Options:
  - **(a) Exclude them all and ship local equivalents: a review state on branches, a git merge driver, and a shared library root on a synced folder (recommended).** These are the only async-team channels compatible with offline-first.
  - (b) Exclude them with no local equivalents.
  - (c) An optional self-hosted sync server: contradicts offline-first.
- **D10.2 Conflict granularity.**
  - **(a) Per layer (recommended):** different properties auto-merge; a conflict only when both sides changed the same property; pick main or branch per layer. Fewest false conflicts.
  - (b) Whole-frame pick.
  - (c) Per-property pick UI.
- **D10.3 Where do branches appear?**
  - **(a) Inside the file, with a title-bar switcher and a Home branch count (recommended):** Figma parity.
  - (b) Separate Home files with a "branch of" badge.
- **D10.4 What happens to a merged branch?**
  - **(a) Archived, read-only and restorable (recommended):** Law 1, nothing vanishes silently.
  - (b) Deleted.
  - (c) Stays editable.

## Technical decisions taken
- A branch is a library file plus `{branch_of, base_version, name, status}`. At fork time, a "Branch point" named version on main becomes the merge base.
- On merge, nodes added on the branch get fresh IDs, and every reference is fixed up: instances, prototype links, style refs, variable bindings, comment anchors.
- Diff and merge are pure functions in `designos-doc`, run off the UI thread.
- Merge is one History step, with automatic versions before and after.
- Author identity becomes a stable ID plus a display name, per machine.

## 4. Task breakdown

### Pack E10.1: Branch and merge
- **T10.1 [M·P2] Branch metadata and storage**
  - Touches: `store.rs` (`FileEntry`), `versions.rs`.
  - AC:
    1. Creating a branch saves the base version plus a copy with the same NodeIds.
    2. Legacy indexes load.
  - Deps: D10.3.
- **T10.2 [L·P2] Three-way diff engine** (nodes plus styles, variables and components)
  - Touches: a new `designos-doc/src/branch_diff.rs`.
  - Seam: pure fixtures.
  - AC:
    1. Add, remove, move and property changes are classified correctly.
    2. A conflict is reported only per D10.2.
    3. A 723k-node diff stays within budget off-thread (**[A]**).
- **T10.3 [L·P2] Merge with ID remap and conflict resolution**
  - AC:
    1. Every reference resolves after remapping.
    2. The chosen side applies.
    3. Versions exist before and after; one undo reverts.
  - Deps: T10.1, T10.2.
- **T10.4 [M·P2] Branch UI** (switcher, create, rename, archive, Home count)
  - AC: switching never dirties the document; archived branches are shown (D10.4).
  - Deps: T10.1. Load es-designer.
- **T10.5 [M·P2] Review-before-merge screen** (before/after thumbnails, conflict list)
  - AC: merge stays disabled until every conflict is resolved.
  - Deps: T10.2, T10.4.
- **T10.6 [S·P2] Branch checkpoints in version history** (created, merged, updated entries)
  - Deps: T10.1.

### Pack E10.2: Stay in sync and review
- **T10.7 [M·P2] Update branch from main**
  - Deps: T10.3.
- **T10.8 [S·P2] Compare any two versions**
  - Deps: T10.2.
- **T10.9 [M·P2] Local review request** (In review / Approved / Changes requested; reviewer name; comments anchored to changes)
  - AC: the state persists and the merge screen shows it.
  - Deps: T10.5, T10.14.
- **T10.10 [S·P2] Merge comment threads across branches**
  - Deps: T10.3.

### Pack E10.3: Team async without a server (per D10.1)
- **T10.11 [M·P2] `designos merge-driver %O %A %B` and `designos diff` CLI**
  - Touches: `app/src/cli.rs`.
  - Seam: a temp git repo.
  - AC:
    1. A clean merge exits 0.
    2. A conflict exits non-zero with a report; the file is never corrupted.
  - Deps: T10.2, T10.3.
- **T10.12 [S·P2] `.gitattributes` installer and docs**
  - Deps: T10.11.
- **T10.13 [M·P2] Shared library root on a synced folder** (iCloud, Dropbox, NAS): detect conflict copies and use a lock file
  - AC: conflict copies are surfaced, never silently overwritten.

### Pack E10.4: Comments and workspace
- **T10.14 [S·P2] Multi-author identity** (stable ID, display name, color)
  - Touches: `designos-doc/src/comments.rs`.
  - AC: legacy author strings migrate.
- **T10.15 [M·P2] @mentions, reactions, filter by author**
  - Deps: T10.14.
- **T10.16 [M·P3] Nested folders 10 deep and folder preview**
  - AC: depth 10 is enforced with a clear error at 11; previews render.
- **T10.17 [M·P3] Tab groups** (`tabs.rs`)

## 5. Risks and unknowns
- **[I]** The NodeId remap is the main correctness risk: one missed reference type silently breaks a link.
- **[I]** Branches are full copies, so a 723k-node file doubles its disk use per branch. Measure it.
- **[F]** Files outside the library cannot branch until they are imported.
- **REA still needed:** the `DiffPayload` fields, branch and merge strings, and the conflict flow. **[A]** The merge algorithm is server-side.

---

## Cross-epic
- **Suggested pack order [I]:**
  1. E7.1, alongside the non-shipping T8.3 spike.
  2. E8.1.
  3. E8.2.
  4. E10.1.
  5. E7.3.
  6. E8.3.
  7. E7.2: its T7.13 and T7.14 depend on E8.
  8. E10.2.
  9. E8.5.
  10. E7.4.
  11. E8.6.
  12. E10.3.
  13. E7.5.
  14. E8.7.
  15. E10.4.
  16. E8.4.
  17. E7.6.
- **Cross-epic dependencies:**
  - T7.12 needs E6 (motion model).
  - T8.30 needs E9 (board and slide node types).
  - T7.13 and T7.14 need E8.
- **Hot-file queue:** T7.19 → T7.23 → T8.4 → T8.20c → T8.29.
- **Assumptions to confirm:**
  - `@figma/plugin-typings` covers the API surface (high).
  - Community plugins cannot be downloaded (medium).
  - `rmcp` supports HTTP (medium).
  - The 723k-node diff budget is achievable (medium).

## Unresolved questions
- Is web capture compatible with your no-html-to-figma rule, or does that rule cover only the Figma workflow?
- PPTX is listed under both E8 and E9. Can E8 own it?
- Which open-source Figma plugins should seed the T8.11 corpus?
- Which fixed localhost port should the MCP HTTP transport (T7.7) use?

Status: DONE_WITH_CONCERNS
Summary: Replanned E7, E8 and E10 to full scope under the owner ruling: 85 tasks in 17 packs, in dependency order. Every gap-list and catalog item maps to a task. Nature-based exclusions are flagged as owner decisions D7.1, D8.1 and D10.1.
Concerns/Blockers: Nothing was compiled or run (per the brief). The plugin sandbox (R3) is still unread. The 41 MCP tools and the GUID retention are unknown. Some tasks depend on E6 and E9.