# #254 Spike the plugin runtime and write its design doc (behind a flag) (T8.3)

| Field | Value |
|---|---|
| Issue | #254 (task T8.3; epic E8 #21; Size M / P1; REA need R3, retried with `rea`) |
| Size | M (the engine spike is one to two days; the design doc and the fixtures are the rest) |
| Depends on | #253 (T8.32: the approved corpus; the spike runs one corpus plugin), #166 (X3: `cargo-deny`; its text says the gate lands before new crates such as rquickjs and wry) |
| Unblocks | #255 (T8.5: the production host with limits), #256 (T8.8: core nodes API), #257 (T8.9: events and undo grouping), #296 (T8.7: plugin UI webview). They are referenced here, not redefined. |
| Touches | new `crates/designos-app/src/plugins/host/` (spike modules, compiled only with a Cargo feature), `crates/designos-app/Cargo.toml` (feature `plugin-host-spike`, optional `rquickjs`), `crates/designos-app/src/plugins/store.rs` (one pure gate function and tests), new `docs/plugin-runtime-design.md`, fixtures `crates/designos-app/tests/fixtures/plugin-corpus/svg-inserter/`. No hot file. |

Code baseline: `origin/main@0e8a9a5`. Evidence: REA 2026-10-19 (Figma plugin runtime bundle and plugin typings 1.142.0, `@figma/plugin-typings` MIT; an rquickjs 0.14 probe, measured); GitHub API reads on 2026-10-19 for the fixture plugin. No Figma code is copied.

## 1. Summary and user story

As the maintainer of the plugin roadmap, I want a flagged proof that a real Figma plugin (`svg-inserter`) runs end to end headless on a sandboxed JavaScript engine against a Studio document, and a design doc that fixes the sandbox limits and the security model, so that the production host (#255 and later) starts from a measured design instead of a guess.

## 2. Current state (FACT)

- There is no JavaScript engine in the repository: `rquickjs` is not in `Cargo.lock` and not in any `Cargo.toml`; there is no `deny.toml` (#166 adds it).
- `plugins/runtime.rs` is a loopback panel server: `Runtime::start` binds `127.0.0.1:0` (`runtime.rs:34-41`), serves the bundle's panel and answers one endpoint, `insert-svg`, behind a token, an origin check and the `document:insert-svg` permission (`:187-193`). The plugin's `main` file is never executed. #296 later replaces the panel with a webview.
- `plugins/store.rs` is the permission gate today. `list` forces `runtime_supported = false`, `permissions` and `network` empty for every installed bundle, because `installed.json` is unverified (`store.rs:441-452`). `load_bundle` is the only path that sets `runtime_supported = true`, and only for a native schema-2 manifest (`:273-307`; "Schema 1 bundles do not support execution" `:282`). An imported Figma manifest is "never runnable" (`:450`). Native permissions are the two strings `document:insert-svg` and `storage:local` (`:185`). Tests pin that installed metadata cannot grant capabilities (`:865-935`).
- The repository already parses and imports Figma manifests: `parse_figma_manifest` (`store.rs:1512`), `import_figma_manifest` (`:1681`), `FigmaManifest` (`:1205`), `KNOWN_FIGMA_MANIFEST_FIELDS` including `documentAccess`, `capabilities` and `relaunchButtons` (`:1347-1369`).
- Document edits by an external actor already travel as a log: `LiveOp` (`designos-doc/src/live_patch/mod.rs:130-172`), `live_ops_for_command(doc, cmd)` (`live_patch/record.rs:12`) and `replay_live_patch` (`live_patch/replay.rs:78`). The editor applies a batch as commands; a transaction is `Tx` (`designos-app/src/edit.rs:71`), committed as one undo step. The agent bridge hands an external actor a scoped copy of a large document (`live_canvas_scope.rs:17`, budgets `:12-15`).
- SVG insertion exists as a library: `designos_svg::svg_to_nodes` (`designos-svg/src/lib.rs:97`) and `import_svg` (`:110`); the panel path is `insert_plugin_svg` (`plugins/mod.rs`).
- FACT (GitHub API, pinned commit `03131bef561eb25ee2b704e3b39e40acc70330e0` of `figma/plugin-samples`, MIT, read 2026-10-19): `svg-inserter` has `manifest.json` (three parameters, `networkAccess` none, `documentAccess` `dynamic-page`) and a 67-line `code.ts`; it uses `figma.parameters.on('input')` with suggestions, `figma.on('run')` with the parameter values, `figma.createNodeFromSvg` and `figma.closePlugin`, and no UI. The repository ships `code.ts` only, no built `code.js`.
- FACT (REA 2026-10-19, Figma plugin runtime bundle), the Figma runtime:
  - The engine is QuickJS compiled to wasm (about 750 KB) on the main thread, ES2023 and later; it exposes a handle API and three limit calls (memory limit, maximum stack size, execution time limit). Only one VM exists at a time; it is reset after each run; dynamic `import()` is rejected.
  - Plugin code runs wrapped in a `with(scope)` and a strict `eval`; `window`, `document` and `indexedDB` are undefined; the host injects timers (although the public docs say otherwise), `console`, `atob` and `btoa`, and an allow-listed `fetch`.
  - The plugin UI is an iframe with a null origin; messages are `{pluginMessage, pluginId}`, deep-copied, delivered as a `message` event with `{origin}`.
  - Manifest validation: `editorType`, permissions, capabilities (some need the dev editor type), unique parameters (optional ones last) and `relaunchButtons`; `allowedDomains` accepts `["none"]`, `["*"]` (with a reasoning string) or patterns.
  - One run is one undo step; `commitUndo()` inserts a boundary. `pluginData` and `sharedPluginData` entries are limited to 100 kB each; a shared namespace needs at least 3 alphanumeric characters; a relaunch description is at most 1,000 characters.
  - The API surface is 133 `PluginAPI` members and 257 interfaces (plugin typings 1.142.0). The corpus needs about 29 of the 133.
- FACT (REA 2026-10-19, rquickjs 0.14 probe, measured): ES2023 features, WeakRef and iterator helpers are present; `structuredClone`, `console`, timers, `fetch`, `TextEncoder` and `Intl` are not (the host must provide what Figma provides); a 16 MiB memory limit makes an allocation loop fail with an out-of-memory error that is caught; a 256 KiB stack limit makes deep recursion a `RangeError`; an interrupt handler killed `while(true)` in 200 ms and the context stayed reusable; start-up is about 56 microseconds.
- UNKNOWN: Figma's plugin timeouts and memory ceilings (set server-side). Closing check: owner-manual live Figma read (below).
- UNKNOWN: which QuickJS version rquickjs-sys vendors. Closing check: `cargo tree` and the vendored source at the first compile (recorded in the design doc).
- UNKNOWN: whether Figma exposes `TextEncoder`, `Intl` or `structuredClone` to plugins. Closing check: the same live read (`typeof` probes in a plugin run).

## 3. Behaviour rules

1. **Engine.** `rquickjs` on a worker thread. One run at a time per host (as Figma: one VM, reset after each run). A run creates a fresh context; closing drops it. Alternatives recorded in the doc: Boa (pure Rust, slower), and QuickJS-in-wasm through a wasm runtime for hard isolation (documented, not built).
2. **Limits.** A `RunLimits { memory_bytes, stack_bytes, slice, total }` struct; spike defaults 64 MiB, 1 MiB, 5 s of uninterrupted execution, 60 s per run (ASSUMPTION: Figma's values are UNKNOWN; #255 sets the production values and the toast). The mechanisms are the probe's: a memory limit, a stack limit, and an interrupt handler checking a deadline and a stop flag. Tests use the probe's values (16 MiB, 256 KiB, 200 ms).
3. **Globals.** Only the Figma-confirmed set is injected: `figma` (the spike API), `console`, timers (`setTimeout`, `clearTimeout`, `setInterval`, `clearInterval`) driven by the worker's own loop, `atob`, `btoa`, `fetch` (rejects for a manifest with `networkAccess` none). `window`, `document`, `indexedDB` are undefined; dynamic `import()` is rejected. Nothing else is added until the live read (a global Figma does not expose must not exist here).
4. **Document access (scoped working copy and LivePatch bridge).** The worker holds a working copy of the document (the spike clones it; the design doc records the scoped copy of `live_canvas_scope.rs` for large files, with the same budgets). The plugin reads and writes the copy synchronously, so a read after a write sees the write (read-your-writes). Every write is a `Command` run on the copy and logged as `LiveOp`s through `live_ops_for_command`. On commit, on a yield (an awaited host call or a timer turn) and on a plugin-initiated `closePlugin`, the logged batch is sent to the UI thread, which replays it with `replay_live_patch` inside one `Tx`.
5. **One run is one `Tx`.** All batches of a run join one `Tx`; `figma.commitUndo()` commits the `Tx` (one undo step) and starts a new one. A host-initiated stop (user Stop, limit kill, window close) drops the unsent batch and rolls the `Tx` back; a plugin's own `closePlugin()` commits first. (This is the rule #255's third criterion and #257's second one build on; the spike proves it on the headless path only.)
6. **Permission gate.** A pure function in `store.rs`, `plugin_can_run_in_host(&FigmaManifest) -> Result<(), String>`, decides whether a manifest may run in the spike host. It requires: the `figma` editor type, a `main`, `networkAccess.allowedDomains == ["none"]` (the spike has no network), no `enableProposedApi` or `enablePrivatePluginApi`, and no `capabilities` or `permissions` entry it does not implement. It never reads `installed.json`; `list` keeps forcing `runtime_supported = false` for imported bundles (`store.rs:443-444`); the existing tests at `:865-935` stay green. Native bundles are untouched.
7. **Flag.** The host compiles only with the Cargo feature `plugin-host-spike` (off by default, so the default build, `cargo tree` and CI carry no `rquickjs`). With the feature on, there is still no menu entry, no button and no change to `runtime_supported`: the only entry point is a library function used by the headless test. Ships nothing user-visible.
8. **Spike API subset.** `figma.on('run', ...)` (with the parameter values), `figma.parameters.on('input', ...)` (the suggestions object is accepted and ignored), `figma.createNodeFromSvg(svg)`, `figma.closePlugin(message?)`, `figma.notify(text)` (collected, not shown), `figma.commitUndo()`, `console`, timers. Anything else on `figma` throws a `TypeError` naming the missing member (so #298 can count unsupported calls). `createNodeFromSvg` uses `designos_svg::svg_to_nodes` and inserts the tree under the working copy's current page.
9. **Fixture.** `svg-inserter` at its pin, with its manifest, its `LICENSE` notice, a `SOURCE.txt` (repository, pin, the one-time build command and the toolchain version used) and the built `code.js` (the repository ships only `code.ts`; the build is run once by the author of the spike, CI never builds).
10. **Design doc.** `docs/plugin-runtime-design.md` records: the Figma runtime facts above with their labels; the engine choice and alternatives; the limits and why; the security model (trust boundaries: plugin code is untrusted, the host API is the only door, the manifest declares capabilities, `installed.json` never grants, network only through the allow-list, one-time consent later by #302); the document bridge and its undo rule; the risk list; the API roadmap; the open numbers.

## 4. Data model / schema changes

None to the document or the file formats. New types live in `plugins/host/` (`RunLimits`, `RunEvent`, `RunReport`, `HostError`), compiled only with the feature.

## 5. Contracts

- Library entry point (feature on): `run_plugin(manifest: &FigmaManifest, code: &str, event: RunEvent, doc: &mut Document, limits: RunLimits) -> RunReport`, where `RunReport { closed: bool, notifications: Vec<String>, console: Vec<String>, patches: Vec<LivePatch>, error: Option<HostError> }`. It applies the batches to `doc` through `History::exec` inside one transaction so a test can undo the run.
- The gate function of rule 6 is public in `plugins::store` and has no dependency on the feature.
- The design doc is the contract for #255, #256, #257 and #296; they cite its section names. Changes to a documented limit go through the doc first.
- cargo-deny (#166) must pass with the feature on: `rquickjs` and its sys crate carry MIT sources (INFERENCE, to be confirmed by the gate; the spike does not merge before #166 does).

## 6. File-format mapping

None. (The plugin manifest format is Figma's own, parsed by the existing `parse_figma_manifest`.)

## 7. UI

None. Ships nothing user-visible, per the issue's acceptance criteria 3: no menu, no button, no label, no screen. Visual evidence in the PR is a terminal capture of the headless test (the run, the inserted node tree, the closed flag) and of `cargo tree -p designos-app` without the feature (no `rquickjs`). es-designer is not needed (no UI file changes).

## 8. Edge cases

- A plugin that never calls `closePlugin()`: the run ends when the event loop is idle (no timers, no pending host calls) and is reported as `closed = false`; the host does not wait forever.
- A plugin that throws: the error is reported with the message; the `Tx` rolls back; nothing reaches the document.
- `createNodeFromSvg` with invalid SVG: throws a JavaScript `Error` inside the plugin (as Figma does), not a host panic.
- A timer that outlives `closePlugin()`: cancelled with the context.
- A plugin that allocates in a loop: out-of-memory is caught and reported; the worker thread survives and is reusable.
- Deep recursion: a `RangeError` inside the plugin.
- A plugin that blocks the event loop: the interrupt handler stops it at the slice limit.
- A manifest asking for `["*"]` network or a UI: refused by the gate in the spike (documented as "not yet", not as "never").
- An engine crash (a bug in native code) would take down Studio: recorded as the top risk in the doc, with the child-process and wasm-engine mitigations; the spike does not build them.
- Large documents: the spike clones; the doc states the scoped-copy path and its budgets; a document over the agent bridge's threshold is out of the spike's tests.
- Linux and Windows: `rquickjs` builds a C library; the doc lists the toolchain need; CI builds the feature only after #166.

## 9. Non-goals

- Production limits, the stop toast, cancel semantics in the UI (#255).
- The core nodes API (#256), events and undo grouping beyond the one-run-one-`Tx` proof (#257), the UI webview and `postMessage` bridge (#296), fonts, images and export (#297), styles and variables (#299), network consent (#302), the rest of the API (#305), the corpus harness (#298).
- Network access, any UI, `clientStorage`, `pluginData`.
- iPad and JavaScriptCore (#444); the host sits behind a small engine trait so another engine can replace `rquickjs` there (INFERENCE from #444's text).
- A user-visible feature flag.

## 10. Test plan (red first)

Seams: headless host tests in `plugins/host/` (feature on) and gate tests in `store.rs` (feature independent). All "fails before" are yes: no host exists.

Fixture run: the `svg-inserter` code with parameters `icon` (an SVG template string with its size and colour placeholders, which is what the plugin's own suggestions hand back), `size` `24`, `color` `red`.

| Case | Expected | Fails before |
|---|---|---|
| `svg_inserter_runs_headless_and_inserts_the_icon` (the acceptance criterion) | after `run_plugin` the document has one new frame subtree named from the SVG under the page; `closed` is true; no error | yes |
| `a_run_is_one_undo_step` | one `History` undo removes the whole inserted tree | yes |
| `commit_undo_splits_a_run_into_two_steps` | a script with two inserts and a `commitUndo()` between them: two undo steps | yes |
| `host_stop_drops_the_unsent_batch` | stop mid-run: the document equals the original | yes |
| `plugin_close_commits_the_batch` | `closePlugin()` after an insert: the insert is in the document | yes |
| `read_your_writes_in_the_working_copy` | a script reads the node it just created and gets its name | yes |
| `infinite_loop_is_interrupted_and_the_host_is_reusable` | `while(true)` returns an error within 200 ms (test limits) and a second run works | yes |
| `memory_limit_is_enforced` | an allocation loop fails with out-of-memory at 16 MiB (test limits); the worker survives | yes |
| `stack_overflow_is_a_range_error` | deep recursion at 256 KiB reports `RangeError` | yes |
| `globals_are_the_figma_confirmed_set` | `window`, `document`, `indexedDB` are undefined; `console`, timers, `atob`, `btoa`, `fetch` are functions; `TextEncoder`, `Intl`, `structuredClone` are undefined | yes |
| `dynamic_import_is_rejected` | `import('x')` rejects | yes |
| `timers_run_on_the_worker_loop_and_are_cancelled_on_close` | an interval stops after `closePlugin()` | yes |
| `unsupported_figma_member_throws_a_named_type_error` | `figma.createRectangle()` throws naming `createRectangle` | yes |
| `invalid_svg_throws_a_plugin_error_not_a_host_panic` | caught in plugin; no panic | yes |
| `permission_gate_table` | refuses: no `figma` editor type, network not none, proposed API, unknown capability, missing main; accepts the `svg-inserter` manifest | yes |
| `installed_metadata_cannot_grant_runtime_capabilities` | the existing tests at `store.rs:865-935` stay green | guard |
| `default_build_has_no_js_engine` | `cargo tree -e normal -p designos-app` without the feature lists no `rquickjs` | yes |
| `feature_on_changes_no_user_visible_state` | `runtime_supported` for an imported bundle stays false; no new menu entry | yes |
| `cargo_deny_passes_with_the_feature` | `cargo deny check licenses` green with the feature on | yes (after #166) |

Run: `cargo test -p designos-app plugins`, `cargo test -p designos-app --features plugin-host-spike plugins::host`, `cargo tree -e normal -p designos-app`, `cargo deny check licenses --features plugin-host-spike`. Evidence: red/green terminal capture of the headless test, the run output, the `cargo tree` output.

Owner-manual check (flagged, never claimed by an agent): **live Figma read.** In a scratch Figma file run three throwaway plugins and record: (1) a `while(true)` plugin: time until Figma stops it; (2) an allocation loop: size when it fails; (3) a plugin that prints `typeof` for `TextEncoder`, `Intl`, `structuredClone`, `setTimeout`, `fetch`. This closes the timeout, memory and global-set UNKNOWNs; the design doc then replaces its ASSUMPTION defaults with the measured values.

## 11. Performance budgets

- Context start-up: about 56 microseconds (probe, measured, FACT above); a run's start-to-first-instruction under 5 ms including the globals, measured and recorded.
- A batch from the worker to the UI thread: at most one per frame is applied (`poll_plugin_runtime` already applies one operation per frame, `plugins/mod.rs`), so a busy plugin cannot monopolise the editor.
- Memory: bounded by `RunLimits.memory_bytes` plus the working copy; the context and the copy are freed at close.
- Idle: no thread, no timer and no repaint request after the run ends (worker joined within 100 ms of close); 0.00% CPU when no plugin runs. The default build is unchanged in size and time.

## 12. Dependencies and build order

#166 (X3, the licence gate) first, then #253's approval line, then this spike. Order inside: the gate function and its tests (feature independent), the fixture, the host (limits, globals, the bridge), the design doc, the CI feature job. #255, #256, #257 and #296 start after this merges (README-part2). No hot file.

## 13. Owner rulings

- Owner ruling 2026-10-19: in-process engine, with rquickjs on a dedicated worker thread, for the spike and v0.12. A child process or QuickJS-in-wasm stays a later hardening step.

- R1.1 (2026-10-17): the corpus is approved by the owner (#253); R1.2: plugins enter by "Import plugin from manifest..."; R1.3: the plugin UI is an embedded webview with a system-browser fallback (#296); R5.17: network only through the manifest's `allowedDomains` after a one-time consent (#302). Recorded in `plans/261017-epics-full-scope/owner-decisions-and-cross-check.md`.
- Acceptance (issue): one corpus plugin runs end to end headless; the doc fixes sandbox limits and the security model; ships nothing user-visible.
- Delivery law 6: "Close cancels pending mutations" (#255) is read as a host-initiated close; a plugin's own `closePlugin()` commits (rule 5).

## Open questions for the owner (recommended first)

1. Spike limit defaults: 64 MiB memory, 1 MiB stack, 5 s slice, 60 s per run. Recommended: yes for the spike; #255 sets production values after the live read.
2. Isolation level. Recommended: in-process `rquickjs` now; revisit a child process or a wasm engine if #255's crash containment cannot meet its criteria. The design doc records the trade-off.
3. The built `code.js` of `svg-inserter` is vendored (the repository has only `code.ts`). Recommended: yes, once, with the licence notice and a recorded build command; CI never builds.
4. Run the live Figma read (three throwaway plugins, about 20 minutes). Recommended: yes before #255 starts.
