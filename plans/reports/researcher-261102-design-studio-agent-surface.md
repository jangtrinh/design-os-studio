# itspatmorgan/design-studio — agent-collaboration surface: what is worth learning

Repo: MIT, React/Vite/Excalidraw 0.18.1, clone HEAD `531e181`. Read-only analysis; nothing run/installed.
Paths below are relative to `src/platform/` unless noted. `canvas/` = `modules/canvas/`.
Labels: FACT = read in source, cited. INFERENCE = my reasoning, not observed. UNKNOWN = not checked.

## 0. NOT VERIFIED (read first)

- No `node_modules` in clone (untrusted, not installed) -> all claims about Excalidraw internals (`reconcileElements` tie-breaks, `CaptureUpdateAction.NEVER` history semantics, `restoreElements`) are INFERENCE from upstream knowledge, not read here.
- Nothing executed: no test run, no live browser. Behaviours (races, perf of 30 iframes) are read from code, never observed.
- `mergeRemote.ts` and `useCanvasFile.ts` have NO tests (grep for `mergeRemote|useCanvasFile` over src hits only canvas files + README, no `*.test.*`). I cannot cite a test proving the merge rules.
- Skill-load step: `es-fable-thinking` Skill tool not in my toolset; ledger discipline applied manually.
- Docs (README/context md) are partly marketing-dense; where docs and code disagreed I trusted code. No disagreement found on the points below.

Weakest link of the whole report: topic 3/4 (undo + merge) conclusions about *concurrent same-element* edits rest on Excalidraw's `version/versionNonce` rule, which I did not read.

## Ledger (load-bearing)

| # | Claim | Class |
|---|---|---|
| 1 | One `run(scene, tool, args, ctx)` serves both browser and CLI | FACT tools.ts:282, agent.ts:110, cli.ts:164 |
| 2 | Parity is by shared function, not by type/contract test | FACT (no test imports agent.ts/cli.ts; tools.test.ts imports only `run`) |
| 3 | CLI silently ignores `describe.scope` | FACT: `scope` handled only agent.ts:102-107; describe() reads only `args.ids` tools.ts:1079 |
| 4 | Lifecycle `activity` has no non-test caller | FACT (grep `.activity(` -> only state.ts + tests) |
| 5 | Canvas embeds show no visible stale banner | FACT PreviewHost.tsx:220,228 gated `surface === 'page'` |
| 6 | Same-element concurrent edit winner is arbitrary | INFERENCE |

---

## 1. One tool table, two transports (live `window.__studioCanvas` + file `pnpm canvas`)

**Mechanism (FACT)**
- `tools.ts` header: tools "defined once", "Pure: elements in, elements out, nothing imported from Excalidraw" (tools.ts:1-4). `TOOLS: ToolDef[]` (80-183) is the single registry; each entry = name, summary, arg docs, runnable `example`, optional `live: true` (83, 163, 171) for tools needing a browser (context, point, screenshot).
- `run()` (282-306) is a pure `switch` over the non-live tools; live tool names fall to `default` and throw a ToolError that lists only the non-live tools (299-304). `help()` (259-272) marks live tools `needs: 'the open canvas in a browser'` (265), so the same help text is truthful on both transports.
- Transport 1, agent.ts: `call()` (94-141) -> `run(before, ...)` (110) -> re-measure touched elements with Excalidraw `restoreElements({refreshDimensions:true})` (113-122) -> ONE `api.updateScene(... IMMEDIATELY)` (123-126).
- Transport 2, cli.ts: parse file (67-78) -> build a `Ctx` from the built manifest (82-161) -> `run(...)` (164) -> permission gate (`canChange`, 173) -> compare-and-swap (`readFileSync !== text` -> fail, 179-180) -> tmp file + `renameSync` (181-184). Output = JSON on stdout (186). `Ctx` is a tiny dependency-injection port: `artifact()`, `artifacts()`, `linkPath()` (tools.ts:44-53), implemented twice (agent.ts:40-65 via `inspectArtifact`, cli.ts:117-161 via manifest JSON + FILE_TYPES).
- Shared storage shape too: `slim.ts` "imports nothing", used by app `format.ts` and CLI (slim.ts:1-3; cli.ts:181).

**Why good**
- Behaviour parity falls out of one function, not of discipline. Tests (`tools.test.ts`, 22 tests) drive `run` through a fake `Ctx` (tools.test.ts:46-57) and therefore cover both transports' logic with node:test and zero browser.
- Port/adapter split (`Ctx`) is minimal: 3 methods. The pure core never learns what an "artifact" physically is.
- Atomic batch semantics: `create` works on a local copy (`let scene = input.slice()`, 463); an error at element i throws before any `updateScene`, so a 200-element call is all-or-nothing, and errors are prefixed `elements[i] (type): ...` (755-757). INFERENCE-free: the throw happens inside `run`, `updateScene` is only reached after (agent.ts:110-126).
- Refs: `@name` lets one `create` call build a connected diagram (artifact -> text below -> rectangle rightOf -> arrow from/to) without the agent knowing ids yet (tools.ts:117-124, 464-467). `Object.create(null)` guards `__proto__` ref names (464, also 341).
- Output hygiene: `create` returns `warnings` for overlaps (761-772) and grows notes/sections to fit content with a notice (508, 531-534, 774-783). Errors state the next action ("Use an id from describe...", 148-156).
- `bump()` raises `version` + new `versionNonce` on every change (elements.ts:259-261, comment: "so an open canvas takes the change instead of keeping its own"). This ties tools to the merge layer (topic 4) AND to render memoisation (`CanvasItem` memo compares `element.version`, embeds.tsx:181).
- Pure core is one of the few places the repo is genuinely small: describe/read output is a compact agent-oriented projection (kind, x/y/w/h, text, colour names, `artifact:{title,type}`), not Excalidraw JSON (tools.ts:1076-1131).

**Weak spots**
- Parity gap proven: `describe({scope:'selection'|'view'})` works only in the browser. CLI hits the shared `describe()` which ignores `scope` and returns everything, silently (FACT: tools.ts:1079 reads only `ids`; scope translated at agent.ts:102-107 only). help text does say "The last two need the open canvas" (tools.ts:95) but the CLI doesn't enforce it. Wrong-but-plausible output = the worst failure class for an agent.
- `help` examples (`ToolDef.example`) are never executed by a test (grep `example|TOOLS` in tools.test.ts -> no hits). Docs can drift from code.
- Two `Ctx` implementations diverge by construction: CLI reads `public/prototypes/manifest.json` (cli.ts:83-84), which can be stale unless rebuilt; browser reads the live manifest (agent.ts:28). Text size is estimated in the CLI path (no Excalidraw to measure); the browser path re-measures (agent.ts:112-122; tools.ts comment "the tools estimate it").
- Tool scope check is inconsistent: `call()` gates on `editable()` only for non-read tools (agent.ts:95); `point`, `screenshot`, `context` bypass `call` (143-212). Harmless today (they don't write), but `point` mutates `selectedElementIds`.
- Dev-only, no auth: `window.__studioCanvas` is installed only if `import.meta.env.DEV` (Canvas.tsx:204). Single global; second canvas tab replaces/removes by identity check (219-222).

**Portability (native Rust/egui/wgpu + MCP)**: Survives intact — an MCP server already IS "one tool table, two transports" (in-process document vs headless file); the pure `run(scene,tool,args,ctx)` + injected `Ctx` + `touched` ids maps 1:1 to a Rust trait over `Document`.

---

## 2. help()/examples, context(), describe(), point(), screenshot(): the look-at-this loop

**Mechanism (FACT)**
- Discovery: `help()` lists tools; `help('create')` adds per-element-type docs (`ELEMENT_TYPES`, 185-257; attached at 271) each with a copy-pasteable `example`. SKILL.md says "Read current tool help instead of assuming argument names or defaults" (use-canvas/SKILL.md:20-25) — the skill carries NO tool schema, only the rule to ask the tool.
- `context()` (agent.ts:146-158): `{canvas file path, editable, selection ids, viewport (canvas coords + zoom), element count, inView ids}`. Purpose stated in tools.ts:85: make sense of "this" and "here".
- `describe` (tools.ts:1076-1131): reads scene as data; `scope:'selection'|'view'` converted to `ids` in agent.ts before calling the pure fn (102-108); `view` excludes label children (`!el.containerId`, 106). Artifacts are named via `ctx.artifact()` (1115-1119) — an agent sees "Main (View)", not a URL.
- `point({ids})` (agent.ts:167-187): sets `selectedElementIds` with `CaptureUpdateAction.NEVER` (177) then `scrollToContent` with `maxZoom = current zoom` — "Never zooms in: a small thing is centered, a big one fits" (179-185). Resolves container labels (172).
- `screenshot({ids, scale})` (189-212): `exportToBlob`, scale clamped 0.1-4 ("a huge scale crashes the tab", 190), filters by id + frame + container (193-198), returns a data URL.
- New-content reveal: after `create`, if nothing new is in view it scrolls (agent.ts:127-138). The human sees what the agent did.

**Why good**
- Closes the deixis gap: human selects / looks; agent reads `context().selection / inView` (ids, not pixels) and answers "this one" with `point(ids)`. Bidirectional pointing with ids as the shared coordinate system — cheap in tokens, exact.
- Selection scoping inside `describe` keeps agent reads small on a big canvas.
- `point` uses NEVER capture -> pointing doesn't pollute undo. Never-zoom-in rule avoids disorienting the human's camera.
- Skill = a rule ("inspect before, describe after, prefer tools over JSON edits", SKILL.md:14, 31-33) + pointers; no duplicated spec. Prevents skill/tool drift.

**Weak spots**
- `screenshot` explicitly cannot show embedded views — "Views show as plain boxes; use describe" (tools.ts:173-174), since Excalidraw export doesn't render iframes. So the "verify visually" step for the main content type is NOT covered by the tool; SKILL.md:41 pushes the agent to "inspect the rendered canvas and embedded artifacts" with other (browser) means. Live screenshots miss the thing that matters most.
- `context()` returns ids only; no names. The agent must follow with `describe({scope:'selection'})` (extra round trip). No "what changed since last call" diff.
- `point` replaces selection (human's selection lost) with no restore.
- `view` scope uses bounding-box intersection (agent.ts:87-91) with no cap on element count; huge zoomed-out views return everything.

**Portability**: Survives — `context/describe(scope)/point/screenshot` need only selection + camera + id addressing; in egui/wgpu `screenshot` is *better* (you can render live embeds, unlike Excalidraw export).

---

## 3. Undo: "one tool call = one undo step"; live vs external edits

**Mechanism (FACT)**
- Live: agent.ts:5 "one tool call is one undo step". Implemented by building the whole result scene and committing it with a single `api.updateScene({... captureUpdate: IMMEDIATELY})` (agent.ts:123-126) — one call = one history entry, regardless of 1 or 200 elements. `point` -> `NEVER` (177).
- External (CLI or other tab): `takeFile` applies with `captureUpdate: NEVER` (useCanvasFile.ts:99-104) — taken into the scene, not undoable. SKILL.md states it plainly: "Live tool operations support undo. External file edits may not be separate undo steps." (use-canvas/SKILL.md:16).
- Persistence is decoupled from undo: live edits flow through normal `onChange` -> dirty -> 1.2 s debounce save (useCanvasFile.ts:24, 174-185). The file never stores deleted elements (slim.ts:5, 91), so "undo of delete" is in-memory history only; a saved delete is final in the file.

**Why good**
- Granularity chosen at the right level: a tool call is the unit of intent, and a batched `create` of a whole diagram is one ⌘Z. Human-friendly rollback of agent work.
- Honest contract: the skill doesn't pretend the CLI is undoable.

**Weak spots**
- CLI edits are un-undoable by the human in the app, and CLI has no inverse tool; only git. After NEVER-replace, human ⌘Z history still holds pre-edit deltas — INFERENCE: undoing may collide with the externally changed scene (depends on Excalidraw store delta rebasing, not read).
- One-step undo of an agent batch undoes ONLY the batch if no human edit came in between; INFERENCE: interleaved human edits make ⌘Z order mixed — no "undo agent's last operation" tool, and no operation log.
- No test covers undo (grep `undo` in tools.test.ts -> none; agent.ts is untested).
- Debounced save means undo after a save and before a reload works, but the file has already been rewritten; no history in the file.

**Portability**: Survives and is easy in a native app — command-pattern history with "MCP tool call = one transaction" is exactly the same boundary; note the repo's choice (external = not undoable) is a limitation of file-watching, not a design goal.

---

## 4. mergeRemote with lastIds — concurrent agent + human edits

**Mechanism (FACT)**
- Problem: file stores only live elements (slim.ts:5, 91) so no tombstones exist. An element missing from the incoming file is ambiguous: "deleted there" vs "created here, not yet saved".
- Solution (mergeRemote.ts:7-25): `lastIds` = ids present in the file as the canvas last knew it (`disk.current.ids`, set on load/take/save: useCanvasFile.ts:51, 98, 140). `deletedThere = lastIds − remoteIds` (18) -> mark matching local elements `isDeleted` via `newElementWith` (19-21); then Excalidraw `reconcileElements(marked, remote, appState)` (22): per-id, higher `version` wins (comment at 7-8: "Excalidraw's own rules"). `needsWriteBack` = any live element we hold that the file lacks (24) -> schedule a save of the union (useCanvasFile.ts:109-115).
- Merge only when dirty or saving: `if (dirty.current || saving.current) mergeRemote(...)` else the file simply replaces the scene (91-97).
- Save protocol: content-hash version (`versionOf` = sha1[:16], node/build/files/paths.js:135); server CAS: `request.base !== versionOf(content)` -> 409 (node/build/files/source.js:39-40); client turns 409 into `SourceChanged` (app/data/files.ts:235) -> re-read, `takeFile` (merge), retry after 300-600 ms jitter, max 3 conflicts then pause + toast (useCanvasFile.ts:146-157, 26). Other failures: exponential backoff 2 s -> 30 s (159-161). `inflight` text lets the app recognise its own save echoing back via HMR (230, 56).
- Flush on tab hidden / unmount / route leave (237-251, 204-208). `persist()` (190-201) lets the agent force a save and learn whether it succeeded (agent.ts:215-220).
- Half-written file tolerated: parse failure returns false "wait for the next change" (useCanvasFile.ts:80-85).

**Why good**
- `lastIds` is a tiny, correct substitute for tombstones: no CRDT, no op-log, one Set<string>, three call sites. Keeps files clean in git while still distinguishing delete from not-yet-saved create.
- Optimistic concurrency with a content-hash base (no counters to corrupt) + merge-then-retry loop with jitter and a cap = bounded behaviour under a chatty agent.
- Agent edits bump `version` (elements.ts:259) so they actually win reconcile; human unsaved edits survive agent writes (union write-back).
- Recognising own echo (`inflight`) avoids save loops.

**Weak spots**
- No tests for mergeRemote/useCanvasFile (see §0). Rules exist only in comments.
- `version` is an edit counter, not time (elements.ts:260: `version + 1`). INFERENCE: if human edited element E five times and the agent bumps it once from the same base, human wins even if the agent acted later; on a tie the winner is the random `versionNonce` -> arbitrary. Last-writer-wins at element granularity, no conflict surfaced to anyone.
- Server write is `writeFileSync` (not tmp+rename; source.js:41) while CLI uses tmp+rename (cli.ts:181-184); CLI CAS check (179) is a read-then-write with no lock, so a browser save can land between check and rename (window = microseconds, but real; whoever renames last wins silently). Server side is atomic only versus other Node-process requests (sync read->check->write in one tick, source.js:35-42).
- `deletedThere` lets a remote delete beat a local *unsaved edit* of that element (marked deleted regardless of local version, mergeRemote.ts:19-21). Intentional but lossy.
- After 3 conflicts it just pauses and toasts (useCanvasFile.ts:150-156): data stays only in memory.
- Scope: whole-element granularity only; two parties editing different properties of one element lose one set.

**Portability**: Idea survives; the specific trick doesn't need translating if the native doc keeps tombstones/ids in memory — but if you persist a tombstone-free file AND let external writers edit it, `lastIds` (set of ids at last sync) is the cheapest correct delete-vs-new disambiguator.

---

## 5. liveViews: MAX_LIVE, mount gate, eviction — bounding cost of many live embeds

**Mechanism (FACT)** — liveViews.ts is 45 lines, pure, tested (liveViews.test.ts, 3 tests).
- Constants: `MAX_LIVE=30`, `EVICT_GRACE_MS=5000`, `SWEEP_INTERVAL_MS=3000`, `MOUNT_INTERVAL_MS=150` (liveViews.ts:5-11).
- Mount gate: `createMountGate()` closure returns true for at most one NEW mount per 150 ms (14-21). Comment: "several heavy views mounting in one frame freeze panning" (10).
- Admission in Canvas.tsx:260-286: `renderEmbeddable` runs per Excalidraw render; if the embed is near viewport (not `offscreen`, not zoomed-out `overview`; embeds.tsx:102-112, margin `CULL_MARGIN_PX`=400, OVERVIEW_ZOOM=0.12, embeds.tsx:96-98) and either already admitted or `gate(now)` passes, record `lastSeen`; else arm ONE timer (`admitTimer`) that bumps `epoch` after 150 ms to re-render and retry (266-271). Not-yet-admitted -> sized placeholder `<div bg-muted/50>` (ArtifactHost.tsx:127-128). Frame never moves.
- Eviction: interval 3 s re-reads what is visible (still canvas does not re-render, comment 288), refreshes `lastSeen` for visible admitted (Canvas.tsx:297-302), calls `pickEvictions` (liveViews.ts:25-45): evict (a) ids removed from canvas always, (b) if admitted count minus (a) exceeds 30: oldest-seen among non-visible with `now - seen >= 5000`, only as many as the excess (37-44). Evicted -> `seen.delete`, `epoch++` -> placeholder again (305-307).
- Offscreen but admitted: `visibility:hidden; content-visibility:hidden` — mounted, still receiving HMR (ArtifactHost.tsx:124; README:77 "does not pause them").
- Memoisation so panning doesn't re-render every embed: `CanvasItem` memo on id/version/offscreen/overview/mounted (embeds.tsx:171-184).
- Each live view = its own same-origin iframe with its own React runtime, 1440 px layout viewport scaled/cropped, inert (modules/view/README.md:23-31).

**Why good**
- Cost bounded by *count*, with hysteresis: grace window prevents mount/unmount thrash when panning back and forth (5 s), soft budget means visible embeds are never evicted even if > 30 (tested: liveViews.test.ts:17-22, visible + recent protected, oldest-eligible first).
- Time-sliced admission (one per 150 ms) trades load latency for steady frame time — the right trade for a pan/zoom surface.
- Pure functions (`createMountGate`, `pickEvictions`) isolate policy from React; unit-testable without DOM.
- Overview mode (zoom < threshold) shows labelled cards instead of rendering (embeds.tsx:161-165) — LOD.

**Weak spots**
- Bounds mount COUNT, not CPU: hidden-but-mounted iframes keep running + HMR (README:77, ArtifactHost.tsx:124). 30 full React runtimes is the ceiling; doc admits "target budget of 30 retained previews ... can temporarily exceed" (canvas/README.md:79).
- `renderEmbeddable` has side effects in render (ref write + `setTimeout` + later `setState`, Canvas.tsx:260-286) — fragile under React strict/concurrent rendering; they contained it with a single `admitTimer`, but it is a render-phase mutation.
- 3 s polling interval with `Date.now()`; `lastSeen` refresh only for visible; no device-aware budget (30 is a constant, not measured).
- Eviction loses interaction state of that view (README source.md state table: "Returning creates a fresh preview").
- Test coverage covers the policy functions only; the admission/timer wiring (Canvas.tsx) is untested.

**Portability**: Survives fully and is trivial in native — gate + LRU-with-grace + visible-pinned + placeholder-keeps-frame is a generic "texture/view budget" policy; for wgpu, count maps to GPU memory, and "hidden but running" should be paused (a luxury egui/wgpu has and DOM doesn't).

---

## 6. Artifact lifecycle: source/preparing/displayed, freshness, activity — how the UI knows a render is stale

**Mechanism (FACT)**
- State machine `core/artifact-lifecycle/state.ts`: `{generation, attempt, source, preparing, displayed:{revision,commit}, phase, detail, activity}` (4-14). Revision = `{source: sha256(file), inputs: sha256(all dependency hashes)}` (2).
- `freshness()` (18-22): `empty` (nothing displayed) | `unknown` (no source evidence) | `current` (source===displayed) | `stale` (differ). Phase and freshness are deliberately orthogonal (phase: unknown/preparing/ready/error/paused/disposed, 11).
- Ticket discipline: `prepare()` returns `{generation, attempt, revision}`; `commit/fail` succeed only if `accepts(ticket)`: phase preparing AND same generation AND same attempt AND revision equals current source (43-47). Any `observe` of new source bumps `generation` and thereby kills older tickets (52-62). So a late async completion for old source can never acknowledge newer source.
- `fail(ticket, detail, retained)` / `error(detail, retained)` let the renderer choose: keep last good output (stale) or clear it (86-101). `detail` capped at 4096 chars (88, 100, 111).
- `validLifecycle()` (132-191): transport-level validator that rejects fabricated readiness (e.g. phase `ready` requires source===displayed, 170-178; sha256 regex 135-140).
- Evidence of "what is actually running" (compiled.ts + Vite plugin):
  - Server `/__studio/revision` hashes (sha256) the artifact + its whole local Vite module-graph + shared stylesheet + theme files + CSS `@import`s (node/build/vite-artifact-revisions-plugin.js:44-64, 107-128); caps 512 files / 4096 modules / 2 MB each; symlink/containment checks (23-29).
  - Plugin pair: a `pre` transform records the hash of each module's source *as Vite sees it* (69-87); a `post` transform appends a snippet to the module that, on **evaluation in the browser**, writes `path -> hash` into `globalThis[Symbol.for('studio.compiled-inputs')]` (88-105).
  - `applied(snapshot)` = every disk hash equals the browser-evaluated hash (inputs.ts:12-15). `ready()` from the renderer's React commit + `applied` => `commit` -> current (compiled.ts:34-52). Comment: "A compiled-module notification does not prove a React commit. Only renderer ready() does" (compiled.ts:126-127); "A refresh counter alone is never evidence of applied source" (compiled.ts:7-8).
  - Compile error (`vite:error`) latches `failure` until matching repaired inputs commit (compiled.ts:35-40, 118-125); unconfirmed after 15 s -> `unknown` with actionable text (43-48).
  - `readRevision` re-validates the response shape and recomputes `digestInputs` (inputs.ts:16-45).
- Cross-iframe transport: lifecycle computed in the child iframe, sent to host with a monotonic `sequence`; host drops `sequence <= last` and requires sender window, exact origin, session, runtime, identity to match (PreviewHost.tsx:104-119 (acceptsPreview + sequence check); protocol.ts:104-118; view/README.md:41,46).
- Surface: `lifecycleAttributes()` -> `data-artifact-phase/freshness/source-revision/input-revision/displayed-revision/commit` (state.ts:195-203) on the DOM, so an automated browser agent can read freshness without app APIs; a bubbling `studio:artifact-lifecycle` event too (PreviewHost.tsx:120-130). Human banner: "This preview is out of date." / "The latest changes could not render. Give the error to your agent." (PreviewHost.tsx:220-227; MarkdownPage.tsx:96-103).
- `activity`: `{provider, state: idle|working}|null`, explicit and independent of source changes (state.ts:121-126; test state.test.ts:77-87). Header: "this does not infer agent activity" (state.ts:1).
- 12 tests incl. "a React commit with older compiled inputs cannot acknowledge new source" (compiled.test.ts:23), "late compilation error cannot mark repaired inputs failed" (94), "bounded unknown outcome" (79).

**Why good**
- Answers the real question "is what I see the code that is on disk?" with *evidence from the running module*, not with a timestamp or HMR event. Dependency-only changes make output stale even if the entry hash is unchanged (source.md "Dependency-only changes can therefore make output stale").
- Honesty by construction: unknown stays unknown (`unknown` freshness, no fabricated ready). Failure may retain the previous render but is *labelled* stale.
- Ticket pattern (generation + attempt + revision) is a clean answer to async races; it is small (200 lines) and unit-tested.
- DOM attributes = agent-readable truth for a browser-driving agent; same vocabulary across Mermaid, Markdown, View renderers.

**Weak spots**
- `activity` is dead code: the field, setter, validator and tests exist, but no non-test caller sets it (FACT, grep). The "agent is working" UI signal the data model anticipates is not wired. Docs say "optional activity" (view/README.md:92) — consistent, just unused.
- Stale banner is visible only for `surface === 'page'` (PreviewHost.tsx:220); canvas embeds (the multi-view case) expose freshness only as data attributes. A human looking at a canvas gets no visible staleness cue for any of the 30 embeds. FACT.
- Coverage limits admitted by the docs: runtime-fetched data, remote assets, localStorage, layout dims, imported assets "remain unverified" (source.md). Dev-only: published bundles report `unknown` (compiled.ts:101-111).
- Complexity vs payoff: ~600 lines of state/validator/tracker/plugin/bridge for a banner and DOM attributes; the 15-second `unknown` watchdog and latch logic (compiled.ts:26-61) is subtle.
- Plugin hashes with `crypto.createHash` per transform and reads module graph from `server.environments.client.moduleGraph` (plugin.js:117): couples to Vite 7+ environment API (UNKNOWN: version compatibility beyond pinned).
- Hash over `JSON.stringify(sources)` (plugin.js:13): stable only if sorted — it is (`[...files].sort()`, line 63).

**Portability**: Core survives and is cheap natively — {source rev, inputs rev, displayed rev, generation/attempt ticket, freshness derived} is renderer-agnostic; in a native canvas the "browser-evaluated hash" analogue is "hash of inputs the last committed frame/texture was built from" — the same three-fact model works; the Vite-specific plugin does not translate.

---

## 7. Prototype structure: app/ states/ discovery/ + Markdown context next to code; states as first-class files

**Mechanism (FACT)**
- Convention, not enforcement: the prototype contract mandates only `meta.json` and `.tsx` views with default export (prototypes/README.md:12-14); folders at any depth, `_`-prefixed = helper, not navigable (16). `app/ states/ discovery/` exists in the sample (`src/prototypes/parker/feedback-inbox/`) and is expressed via `meta.json.order` (explicit ordering list, README:37) — the platform does not require these folder names; `build-prototype/SKILL.md` does not mention them (grep: absent).
- A state = a 7-line view file that renders the *real* screen with injected start props or data: `states/inbox/empty.tsx` wraps `<FeedbackInboxScreen/>` in `<StaticItems value={[]}>` (empty.tsx:3-8); `validation-error.tsx` renders `<FeedbackInboxScreen adding tried />` (validation-error.tsx:3-7). The screen exports a props type with starting state "so the files in states/ can show it filtered, empty, with the panel open" (app/feedback-inbox.tsx:6-7, 23-31). `StaticItems = Override.Provider` swaps the store without touching live data (app/_components/store.ts:68-76).
- Each state is a first-class artifact: own permanent id (`/** @studio-id ... */` line 1), navigable, embeddable in Markdown (`![Validation error](states/inbox/validation-error.tsx)`, about-this-prototype.md) and on canvases. The sample `eng-handoff.excalidraw` has 5 frames, 10 `embeddable` elements linking by permanent artifact ids, 15 texts, 3 arrows (parsed JSON, my count) — i.e., a spec board made of live screens + states.
- Context lives beside code: `discovery/project-context.md` (problem, audience, assumptions, open questions, embedded live diagram/lofi/canvas) and `about-this-prototype.md` point the agent: "point your agent to Project context so it understands the idea behind the request" (about-this-prototype.md). Also `discovery/feedback-flow.mermaid`, `discovery/lofi-inbox.tsx`, `discovery/breadboard.excalidraw` (same dir listing).
- Permanent ids make cross-file references rename-safe: links are `/prototypes/<protoId>/artifacts/<artifactId>` (prototypes/README.md:64; embed links in eng-handoff). Identity is a build gate: missing ID fails the build; `pnpm studio identify` assigns (README:22).
- Agent entry point for an existing prototype: `pnpm studio context src/prototypes/<key>/<id> --json` returns assignment, pending rebuild, scope, enabled modules, system entries (README:26; build-prototype/SKILL.md:8).
- AGENTS.md "studio:modules" block is *generated* from installed modules: each module contributes its own skill line (AGENTS.md "<!-- studio:modules -->"), so agent routing = installed capabilities (header: "Generated by `pnpm studio sync`").
- Boundary guard for agents: prototype code may import only React + its system + `@module/*`; direct `lucide-react`/router import is a build/dev error (prototypes/README.md:100-104); `canvas` shows only same-prototype artifacts (tools.ts:176-180, test tools.test.ts:269,279).
- Lo-fi switch: `/** @lofi */` comment renders the same component grayscale/handwritten (README:94) — same artifact, different fidelity.

**Why good**
- "States as files that import the real screen" removes drift: a state cannot go out of date relative to the component, appears live on canvases, and gives the agent/human a named, linkable target ("the empty inbox") instead of a navigation script.
- Intent artifacts (problem, assumptions, open questions) live in the same tree/versioning as code and are directly embeddable/linkable, so agents read them as ordinary files — no separate PM tool.
- Permanent ids + manifest resolution decouple links from paths; `meta.json.order` decouples display order from file names.
- Guardrails (import boundary, same-prototype embeds, identity gate) are enforced by build/check, not by prompt text.

**Weak spots**
- The structure is a sample convention; nothing validates "each screen has states" or "has discovery", and the skill doesn't teach it (FACT: absent from build-prototype/SKILL.md). Adoption depends on the agent reading the sample.
- Pattern requires authors to expose screen props for every state (`FeedbackInboxStart`, feedback-inbox.tsx:23-29) — screen components get test-hook props in production code; 5 of 8 states need them.
- `meta.json.order` in the sample lists 23 paths, with duplicates of folder vs file entries (order list contains both `states/inbox` and individual files) — manual upkeep ("Moving, renaming, or deleting an artifact must update its `order` entries", README:44).
- `StaticItems` wraps only the store; states with other side effects (network, router) are on the author.
- States are interactive full screens, not frozen snapshots, so they depend on injected data staying valid as the schema evolves (no type-level link between state files and screen props beyond TS).

**Portability**: States-as-addressable-files survives only partly — in a native design canvas the analogue is "named variants/states of a component with their own ids", already native to Figma-style component sets; Markdown-context-beside-artifacts and `context --json` survive as MCP resources/tools directly.

---

## 8. Other genuinely clever bits

1. **Skill carries a rule, tool carries the schema** (use-canvas/SKILL.md:20-25 vs tools.ts `help`): schema can't drift because the skill points at it. Direct lesson for MCP: self-describing tools + short rule skill.
2. **Inspection before mount** — `inspectArtifact()` (app/artifacts/resolveArtifact.ts:76-113) resolves "preview or card? why not?" with metadata only, without importing renderers; the SAME function powers UI (ArtifactHost.tsx:46) and the agent's `artifacts`/`describe` (agent.ts:43). One truth for "what would this render as".
3. **Per-person camera outside the file** (slim.ts:78-79 "The camera is per person"; camera.ts localStorage key 11-13): diffs stay clean, and `context().viewport` still reads the live camera.
4. **Slim, stable file format** for diffs + agent reading: no deleted elements, no images (hard `check` error, type.ts), defaults dropped, numbers rounded to 2 dp, keys sorted, volatile `updated` dropped (slim.ts:5-10, 26-76); `studioVersion` future-proofing refuses to open/save newer files (cli.ts:79-80, type.ts).
5. **Ownership/permission at the tool boundary**: CLI refuses unless owner/Admin and prototype active (cli.ts:173-177); browser `editable()` (agent.ts:95-99). Plus path containment: canvas file must be inside a prototype (cli.ts:33-57).
6. **Generated AGENTS.md routes by installed module** (AGENTS.md studio:modules): agent capabilities are a function of enabled modules; a disabled module's skill line disappears.
7. **Untrusted-ref safety**: `Object.create(null)` for caller-chosen ref names (tools.ts:341, 464); 4000-char / 20-4000 size limits (125-139); MAX_CREATE=200 (63, 461).
8. **Lifecycle bridge hardening**: sender window + exact origin + session + runtime + identity + monotonic sequence on every message (protocol.ts:104-118; PreviewHost.tsx:104-119) — and the docs openly say the iframe is not a security sandbox (view/README.md:56).

## Ranked: what is most worth a native-canvas team's attention (evidence only, no build advice)

1. Pure shared tool core + injected context (§1) — highest ratio of value/complexity, proven by 22 node tests.
2. Source/inputs/displayed revision + ticket model (§6, state.ts 1-130) — small, race-correct; the compiled-input plumbing is Vite-specific.
3. Selection/viewport/point loop with id addressing (§2).
4. `lastIds` delete-vs-new trick (§4) — only if files are tombstone-free and externally edited.
5. Count-based live-embed budget with grace + gate (§5) — generic LRU policy.
6. States-as-files convention (§7) — pattern, not mechanism.

## Unresolved questions

- Excalidraw 0.18.1 `reconcileElements` / `CaptureUpdateAction.NEVER` real behaviour on concurrent same-element edits and on ⌘Z after external replace (needs node_modules or upstream source read).
- Does anything outside this repo (e.g. the website or a harness) call `lifecycle.activity()`? (none in this repo)
- Does `normalizeEmbeds` (Canvas.tsx:238) mutate agent-created embeds after the fact (sizes/links), affecting "one tool call = one undo step"? Not read.
- CLI `describe` scope gap: is it a known issue upstream? (CHANGELOG not searched.)

Status: DONE_WITH_CONCERNS
Summary: All 7 topics + extras covered with cited FACT/INFERENCE; report written at the path above.
Concerns/Blockers: Excalidraw internals not read (no node_modules); mergeRemote/undo/agent.ts/cli.ts wiring have no tests in repo so behaviour claims there are code-reading only; nothing executed.
