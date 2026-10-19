# #253 Propose the 8-plugin compatibility corpus and get owner approval (T8.32)

| Field | Value |
|---|---|
| Issue | #253 (task T8.32; epic E8 #21; Size S / P1; owner ruling R1.1) |
| Size | S (the proposal and the record are about a day; the approval is the owner's, not an agent's) |
| Depends on | none |
| Unblocks | #254 (the spike runs one corpus plugin; T8.3), #298 (T8.11, the corpus harness and the gate for every later plugin pack) |
| Touches | a new corpus record `plans/261017-epics-full-scope/plugin-compatibility-corpus.md`, small fixtures `crates/designos-app/tests/fixtures/plugin-corpus/<plugin>/manifest.json` with a notice file, one test in `crates/designos-app/src/plugins/store.rs` tests (`parse_figma_manifest` `:1512`), one script `scripts/check_plugin_corpus_record.py`. No source code of the product changes. |

Code baseline: `origin/main@0e8a9a5`. Evidence: GitHub API reads on 2026-10-19 (licences, default-branch commits, directory listings, manifests at the pinned commits); REA 2026-10-19 (plugin typings 1.142.0 and the API areas per plugin).

## 1. Summary and user story

As the owner of the plugin runtime roadmap, I want a fixed list of 8 real open-source Figma plugins, each pinned to a commit and licence-checked, so that "Studio runs Figma plugins" has a concrete, repeatable target and I approve it once.

## 2. Current state (FACT)

- Owner ruling R1.1 (2026-10-17, `plans/261017-epics-full-scope/owner-decisions-and-cross-check.md`, Round 1 item 1): the coordinator proposes 8 open-source Figma plugins and the owner approves the list. The same round fixed how plugins get in (item 2: "Import plugin from manifest..." from a local folder or repo) and where the UI shows (item 3: an embedded webview with a system-browser fallback).
- The repository can already parse and import a Figma manifest: `parse_figma_manifest` (`crates/designos-app/src/plugins/store.rs:1512`) and `import_figma_manifest` (`:1681`); an imported Figma bundle is never runnable today (`list` forces `runtime_supported = false`, `:441-452`; `load_bundle` accepts only the native schema 2, `:273-283`).
- `import_figma_manifest` needs the manifest's `id` and every asset path it lists (`main`, `ui`) to exist in the folder (`store.rs:1698-1726`).
- The v0.7 milestone description lists "plugin corpus approval" and "plugin runtime spike T8.3" as infra of this pack; #298 (T8.11) reads this list.
- FACT (GitHub API, 2026-10-19): the eight proposed plugins, their licences and the commit that was the default-branch head on that date (the pin) are in section 3.
- FACT (GitHub API, directory listings at the pinned commits): only `lukasoppermann/design-tokens` commits a built output (`dist/plugin.js`, `dist/ui.html`, `dist/ui.js`; 26,133, 354,382 and 354,340 bytes). The other plugins commit sources only: `figma/plugin-samples` (three folders: `code.ts`, `manifest.json`, `tsconfig.json`, and `ui.html` for `document-change`), `destefanis/design-lint` (no `dist/`), `thomas-lowry/themer`, `alexwidua/figma-easing-gradients` (no `build/`), `SaraVieira/svg-to-jsx` (no `dist/`). Their manifests name built files (`code.js`, `dist/code.js`, `build/main.js`, ...), so each needs a build before it can be imported or run.
- FACT (same listings): `thomas-lowry/themer` keeps its manifest at `public/manifest.json`, not at the repository root; the other seven are at the plugin folder root.
- FACT (manifest reads at the pinned commits): all eight manifests have an `id`; `figma/plugin-samples/svg-inserter` has `parameters` (three, one optional, one free-form), `networkAccess.allowedDomains = ["none"]`, `documentAccess = "dynamic-page"`, `editorType` figma and figjam; `capital` allows one https origin; `document-change` has a `ui` file; `design-tokens` has a `menu` and `allowedDomains = ["*"]` with a `reasoning` string; `easing-gradients` has `relaunchButtons`; `svg-to-jsx` allows six https origins.
- INFERENCE (reading `parse_figma_manifest`, `store.rs:1512-1662`): all eight manifests pass the parser (known keys only, `["*"]` and `["none"]` accepted by `validate_domain_list` `:1474-1488`). Not executed. Closing check: the manifest test in section 10.
- FACT (REA 2026-10-19, plugin typings 1.142.0): the plugin API has 133 `PluginAPI` members and 257 interfaces; the corpus needs about 29 of the 133 members.
- UNKNOWN: whether the owner wants plugins from one repository counted once or three times (three of the eight come from one repository). Closing check: the approval round (open question 2).
- UNKNOWN: the API areas of plugins 4 to 8 beyond the REA summary were not re-read in this session (only plugins 1 to 3 were checked against the pinned code). Closing check: the first harness run of #298 prints the unsupported calls per plugin.

## 3. Behaviour rules

1. **The proposal** is the table below. It is a proposal until the owner approves; the record says "Proposed" until the approval line exists.
2. **Pins.** Every URL in the record is pinned to a 40-character commit, never a branch.
3. **Licence.** Every plugin is MIT (verified through the repository licence field on 2026-10-19). A plugin whose licence is missing is skipped, never listed.
4. **Approval is a flagged owner step.** The owner approves through `AskUserQuestion`; an agent never writes the approval line before that answer exists, and records the owner's literal answer and the date when it does.
5. **Record shape.** The record lists, per plugin: repo URL at the pin, subfolder, licence, manifest path, whether a built output is committed, editor types, document access, network access, API areas, and ageing. It states the build recipe for each plugin as UNKNOWN until #298 records it.
6. **#298 reads the record.** The harness takes the plugin list from the record (the script of section 10 is the contract of its shape).

| # | Plugin | Repository URL at the pin (commit-pinned) | Licence | Manifest | Built output committed | API areas (REA 2026-10-19; plugins 1 to 3 re-read in this session) | Ageing (head commit, last push) |
|---|---|---|---|---|---|---|---|
| 1 | `svg-inserter` | https://github.com/figma/plugin-samples/tree/03131bef561eb25ee2b704e3b39e40acc70330e0/svg-inserter | MIT | folder root | no | parameters (`on('input')`, suggestions), `on('run')`, `createNodeFromSvg`, `closePlugin` | 2026-03-23 / 2026-03-23 |
| 2 | `capital` | https://github.com/figma/plugin-samples/tree/03131bef561eb25ee2b704e3b39e40acc70330e0/capital | MIT | folder root | no | `fetch`, `networkAccess.allowedDomains`, a UI used as a network helper (`showUI`, `ui.onmessage`, `postMessage`; hidden UI per REA) | 2026-03-23 / 2026-03-23 |
| 3 | `document-change` | https://github.com/figma/plugin-samples/tree/03131bef561eb25ee2b704e3b39e40acc70330e0/document-change | MIT | folder root | no | `documentchange` event, `loadAllPagesAsync`, `showUI`, `ui.postMessage` | 2026-03-23 / 2026-03-23 |
| 4 | `design-lint` | https://github.com/destefanis/design-lint/tree/4c3c40cdd47e93db8e01bb110d992c7235b40efd | MIT | root | no | read traversal, styles, `clientStorage`, viewport | 2023-11-16 / 2024-01-12 |
| 5 | `design-tokens` | https://github.com/lukasoppermann/design-tokens/tree/0acc078fb07a101910e0ab7a2870e1cf9814ffb9 | MIT | root | yes (`dist/`) | 9 menu commands, `figma.command`, dynamic-page access, styles and variables | 2026-02-16 / 2026-02-16 |
| 6 | `themer` | https://github.com/thomas-lowry/themer/tree/25c65a02cd87545ef3aee5ca6b5e617095720b58 | MIT | `public/manifest.json` | no | style writes, `importStyleByKeyAsync` (stub), fonts | 2022-09-08 / 2023-01-09 |
| 7 | `figma-easing-gradients` | https://github.com/alexwidua/figma-easing-gradients/tree/2e35c821935b9da22c44a51185fd1b38bbb7d1c5 | MIT | root | no | gradients, `pluginData`, relaunch buttons | 2022-03-24 / 2023-01-23 |
| 8 | `svg-to-jsx` | https://github.com/SaraVieira/svg-to-jsx/tree/b265369410387f99822220c8d53d2d8f3e326050 | MIT | root | no | `exportAsync` (SVG), UI fetch (stub) | 2024-03-05 / 2024-03-05 |

Reserves (replace a plugin the owner rejects; same checks apply): `brianlovin/figma-dominant-color-toolkit` (MIT, last push 2022-12-09), `tokens-studio/figma-plugin` (MIT, last push 2026-10-15, large), and the `resizer` folder of `figma/plugin-samples`. Skipped because the repository has no licence (REA 2026-10-19, not re-checked here): `iconify-figma` and `Lazy-Export`.

## 4. Data model / schema changes

None.

## 5. Contracts

- The record is a Markdown table with the columns above; the check script reads rows 1 to 8, the 40-hex pin and the licence column.
- Fixtures: one `manifest.json` per plugin, byte-for-byte from the pinned commit, with a `NOTICE.txt` in the fixture folder holding the plugin's copyright notice and the pin (MIT requires the notice to travel). No source or built code is vendored by this task.
- No change to `plugins/store.rs` behaviour; one test is added to its test module.

## 6. File-format mapping

None. (The manifests are Figma's own plugin manifest format, parsed by the existing `FigmaManifest`.)

## 7. UI

None. This task ships no UI, no screen and no label. Visual evidence is a terminal capture: the check script output (8 rows, pins, licences) and the manifest test result.

## 8. Edge cases

- A pinned commit disappears from GitHub (force push): the check script cannot detect it offline; the record carries the pin and the harness (#298) fails loudly when it cannot fetch it.
- A repository changes licence after the pin: the pin keeps the licence of that commit; the record states the licence per pin.
- Three plugins in one repository: counted as three plugins, one repository (open question 2).
- A plugin that needs a build: the record says so; the build recipe and where the built files live are #298's decision (open question 3).
- `design-tokens` asks for network access to any host: it is in the corpus to exercise the `["*"]` case; the runtime still obeys the one-time consent rule of ruling 17 (#302).
- A plugin using a feature Studio will never support (hosted-only APIs): it stays in the corpus; its unsupported calls are reported, not hidden (#298).

## 9. Non-goals

- Running any plugin (#254 and later).
- Vendoring source or built code of any plugin.
- Choosing a build toolchain or where built artifacts live.
- Approving the list on the owner's behalf.
- Marketplace, discovery or an in-app list of plugins.

## 10. Test plan (red first)

Seams: one `cargo test` over the vendored manifests (no network, per the executor contract), one script check over the record.

| Case | Expected | Fails before |
|---|---|---|
| `corpus_manifests_all_parse_with_the_studio_parser` | the 8 vendored manifests parse with `parse_figma_manifest`; each has an `id`; `design-tokens` keeps `["*"]` and its reasoning; `svg-inserter` has 3 parameters with `optional` and `allowFreeform` as in the source | yes (no fixtures) |
| `corpus_fixture_folders_carry_a_notice_with_the_pin_and_licence` | each fixture folder has a `NOTICE.txt` naming the repository, the 40-hex pin and the licence | yes |
| `check_script_accepts_the_record` | `python3 scripts/check_plugin_corpus_record.py` prints 8 rows, each with a 40-hex pin and licence `MIT`, exit 0 | yes (no record, no script) |
| `check_script_rejects_a_branch_url_and_a_missing_licence` | a broken copy of the record exits non-zero with the row number | yes |
| `check_script_refuses_an_approval_line_without_a_date_and_an_answer` | exit non-zero | yes |

Run: `cargo test -p designos-app plugins`, `python3 scripts/check_plugin_corpus_record.py`. Evidence: red/green capture of the five cases and the script's output.

Owner-manual checks (flagged, never claimed by an agent):

- **Approval.** The owner answers `AskUserQuestion` with the 8 plugins (approve, swap one for a reserve, or name others). The agent then records the literal answer and the date in the record's approval line.
- **Live Figma read (optional, closes the API-area UNKNOWN early).** Opening each plugin once in Figma and listing the API calls it makes is #298's first output; no Figma read is needed to approve the list.

## 11. Performance budgets

None for the product (no runtime change). The check script runs in under 1 second offline; the manifest test adds under 50 ms to `cargo test -p designos-app plugins`.

## 12. Dependencies and build order

No task dependency. Land the proposal record and the check first; fixtures and the manifest test in the same PR; the approval line in a follow-up commit after the owner answers. #254 and #298 wait for the approval line (not for the PR merge). No hot file (README-part2).

## 13. Owner rulings

- Owner ruling 2026-10-19: the 8-plugin corpus is approved as proposed. The three figma/plugin-samples folders count as three plugins, and built output is vendored once at the pinned commits.

- R1.1 (2026-10-17): the coordinator proposes 8 open-source Figma plugins and the owner approves the list.
- R1.2 and R1.3 (same round): plugins enter by "Import plugin from manifest..." and show their UI in an embedded webview; R5.17: network limited to the manifest's `allowedDomains` with a one-time consent (#302).
- Acceptance (issue): 8 open-source Figma plugins listed with repo, licence and API surface covered; the owner approves via `AskUserQuestion` and the list is recorded; #298 (T8.11) reads this list.

## Open questions for the owner (recommended first)

1. Approve the 8 plugins above. Recommended: yes as proposed; swap a plugin only for a reserve of the same API area.
2. Three of the eight come from one repository (`figma/plugin-samples`). Recommended: count them as three plugins; they cover parameters, network and events, which no other repository covers.
3. Six of the eight need a build before they can run. Recommended: #298 builds each once at the pin and vendors the built `main` and `ui` files with the licence notice under the test fixtures; CI never builds or fetches.
4. Two plugins are old (themer 2022, easing-gradients 2022). Recommended: keep them; old plugins exercise old API habits, which is the point of a corpus.
