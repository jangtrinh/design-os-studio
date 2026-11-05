# Ruling 261102: what to adopt from itspatmorgan/design-studio into DESIGN:OS Studio

Grader: independent, read-only. External clone = `ext/` (`src/platform/` prefix dropped). Ours = `jangtrinh/design-os-studio` **remote `main` 8f267b3 (2026-11-02)**, read through `gh api` because the local checkout is stale (see D0).

## 0. Not verified (read first)
- **The local checkout is 15 days behind.** Local HEAD is 767acf9 (2026-10-18). The scout inventory and the controller's spot-check ran on that tree. I re-checked every claim about our code against remote `main`. Line numbers marked "rm" are in remote files.
- Nothing was run, in either repo. Host install behaviour (Claude/Codex plugin, MCP registration) is not verified.
- Whether a per-host bundle generator lives outside this repo (the ease-design `ui` binary): UNKNOWN.
- That the G6 chip ("N changes → Show") exists is INFERENCE from closed issue #695. I did not open its code.

Weakest link: gap #3 (`designos_point`). I checked that no such tool exists, but the shape of its live bridge is my proposal and has not been checked against the live-patch protocol.

## 1. Verification table

| Report claim | Verdict | Checked at |
|---|---|---|
| One `run()` serves browser and CLI | CONFIRMED | ext tools.ts:1-4, 282-306; agent.ts:110; cli.ts:164 |
| The CLI silently ignores `describe.scope` | CONFIRMED | tools.ts:1079 reads only `ids`; scope is translated only at agent.ts:102-107 |
| One tool call = one undo step (`IMMEDIATELY`); `point` uses `NEVER` | CONFIRMED | agent.ts:123-126, 177 |
| `point` never zooms in and replaces the selection | CONFIRMED | agent.ts:176-185 |
| The CLI does a compare-and-swap, then tmp+rename | CONFIRMED | cli.ts:179-184 |
| `lastIds` tells a delete from a not-yet-saved create | CONFIRMED | mergeRemote.ts:7-25 |
| Live views: MAX_LIVE 30, 150 ms gate, 5 s grace | CONFIRMED | liveViews.ts:5-45 |
| Freshness states and ticket discipline | CONFIRMED | state.ts:18-22, 43-62 |
| `activity` has no non-test caller | CONFIRMED | grep finds no `.activity(` call outside tests |
| The stale banner shows only on the page surface | CONFIRMED | PreviewHost.tsx:220, 228 |
| Skill sync with a hash receipt; CI runs `sync --check` | CONFIRMED | agent-skills.js:94-166; checks.yml:77; .agents/studio-skills.json |
| Generated routing block | CONFIRMED | declarations.ts:5-22 |
| `help` examples are never tested | CONFIRMED | tools.test.ts: 22 tests, 0 "example" hits |
| **Scout: 22 tools (18+4), xpath/lint not advertised** | **WRONG on main** | the snapshot lists 37 tools; xpath and lint are advertised; `every_dispatched_tool_is_declared` (rm tests/tools_snapshot.rs:53) |
| **Scout: CLAUDE.md says 29 tools** | **STALE** | rm CLAUDE.md:41 has a generated block; gen-tool-docs.rs:11 lists DOC_TARGETS |
| **Scout: `selected_node_ids` is read by nothing** | **WRONG on main** | rm server.rs:489-490; it is exposed only through `include_selection` (rm bootstrap.rs:4-5, 137; owner ruling #698) |
| **Scout: global `/tmp` context collides across windows** | **FIXED on main** (#690) | rm terminal.rs: `/tmp/designos` appears only in tests; a scanner guard is at rm agent_session_files.rs:1322 |
| Scout: no viewport field in context | CONFIRMED on main | rm terminal.rs:2918-2928; `viewport` has 0 hits in rm engine.rs and server.rs |
| Scout: no point/focus tool | CONFIRMED on main | none of the 37 snapshot names is point/highlight/reveal |
| Governance report: "adopt a `context --json` preflight tool" | ALREADY SHIPPED | `designos_get_app_state` (freshness = doc_revision, rules pack per enabled tool; rm bootstrap.rs:119-140) |

## 2. The genuinely good ideas ("hay thật"), at most 8

1. **One tool table, two transports** (tools.ts:1-4, 282; cli.ts:164). The problem: browser and CLI agents drift apart. The fix: one pure `run(scene, tool, args, ctx)` with a three-method `Ctx` port behind both, so parity comes from the code structure, not from discipline. Weak spot: a live-only argument such as `scope` is silently ignored on the CLI.
2. **Bidirectional pointing by id** (agent.ts:146-187). `context()` gives the agent the selection, the viewport and the ids in view, and `point(ids)` shows the human "this one". Ids are cheap and exact. Weak spot: `point` takes over the human's selection and camera.
3. **One call = one undo; pointing is not an undo step** (agent.ts:123-126, 177). The unit of intent is the tool call, and a 200-element batch is one ⌘Z. Weak spot: edits made through the CLI cannot be undone.
4. **Atomic batch with `@ref` names and fix-it errors** (tools.ts:458-468, 752-772). An agent can build a connected diagram in one call. If one element fails, nothing lands, and the error says `elements[i] (type): …`. Overlap warnings come back too.
5. **CAS + tmp/rename on external writes; `lastIds` in place of tombstones** (cli.ts:179-184; mergeRemote.ts:7-25). This is bounded, honest concurrency without a CRDT. Weak spot: the winner of a same-element conflict is arbitrary, and no test covers it.
6. **Count budget for live embeds, with a mount gate and an eviction grace** (liveViews.ts:5-45). Cost stays bounded, and panning does not cause thrash. Weak spot: it bounds count, not CPU.
7. **Freshness from evidence, with tickets** (state.ts:18-62). "Unknown" stays unknown, and a late completion cannot acknowledge newer source. Weak spot: about 600 lines for a banner.
8. **The skill carries the rule, the tool carries the schema; adapters are generated and checked in CI** (use-canvas/SKILL.md:20-25; agent-skills.js:94-166; checks.yml:77). The schema cannot drift into the prose, and a stale adapter fails CI. Weak spot: each stub costs one extra read hop.

## 3. Mapping to DESIGN:OS Studio (remote main)

| # | Status | Evidence |
|---|---|---|
| 1 | **GAP** (CLI transport) | rm designos-mcp main.rs flags: only `--help`, `--list-lint-rules`, `--version`, `--workspace`. rm app cli.rs has hand-written `--add-comment`, `--reply-comment`, `--apply-spec` and `--export-spec` beside the MCP tools |
| 2 | Selection: ALREADY HAVE (opt-in, #698). Viewport: **GAP**. Point: **GAP** | rm bootstrap.rs:137; no viewport field; no point tool |
| 3 | ALREADY HAVE | owner context (shipped); `apply_batch` is one undo group; `set_in_progress` is "not an undo step" (snapshot) |
| 4 | ALREADY HAVE (stronger) | `designos_apply_batch`: 1-200 ops, atomic, `@<as>` refs, `dry_run`; the lint issues block covers overlap warnings |
| 5 | ALREADY HAVE (different model) | op log with base remap (rm replay.rs:128-144) and the busy-frame banner (#693, closed) |
| 6 | NOT APPLICABLE / ALREADY HAVE | there are no live iframes; the GPU `Program` cap and VRAM budget are the analogue (CLAUDE.md §4) |
| 7 | Agent side: ALREADY HAVE. Human side: NOT APPLICABLE | `get_app_state` freshness/doc_revision. The Vite module-hash machinery is web-only |
| 8 | Tool docs: ALREADY HAVE. Host bundles: **GAP** | gen-tool-docs + tools_snapshot cover README, CLAUDE.md and SKILL.md only (gen-tool-docs.rs:11). The manifests are hand-written and have drifted (D1) |

All 11 tasks in agent-terminal-ux-tasks.json (#682-#695) are CLOSED, so nothing below repeats them.

## 4. Ranked adoption list (GAPs only)

**1. Generate the host bundles and check them (from idea 8).**
- Shape: extend `crates/designos-mcp/src/bin/gen-tool-docs.rs` to write the tool count, description and version (from workspace `Cargo.toml`) into `.claude-plugin/`, `.codex-plugin/` and `.factory-plugin/` (plugin + marketplace), and into root `llms.txt`. Add those paths to the prose-count check in `tests/tools_snapshot.rs`. Add the missing hosts that owner policy requires (a Gemini extension and others) as outputs of the same generator.
- Value: the public listings stop lying (they say 36 tools when there are 37, and version 0.2.0 when the workspace is at 0.7.6).
- Size: 4-6 h for the manifests and llms.txt. Each new host is 1-2 days, including the smoke test the owner rule requires.
- Risk: low. Skip the receipt+hash: nobody hand-edits files inside our bundle.
- Verify: `cargo test -p designos-mcp --test tools_snapshot` fails after hand-editing "36" into a manifest.
- Absence (FACT): grep finds no `gemini-extension.json` in the tree; `self_check.py` checks only that the JSON files exist.

**2. Headless `--call` over the same Engine (from idea 1 + CAS from idea 5).**
- Shape: `designos-mcp --call <tool> '<json>' --file x.designos [--write]`. It runs `Engine::call`, which tests already drive directly (`e.call(...)`), refuses the write if the file changed since it was read, and writes through tmp+rename. A live-only argument (`include_selection`) is an error, never silently ignored.
- Value: shell-only agents, CI, eval and bug reproductions get the exact MCP behaviour. The bespoke CLI flags can then go.
- Size: about 1 day.
- Risk: medium; it adds another write path. Reuse `session_file_io`.
- Verify: an integration test runs `--call create_node`, then `--call get_node`, and a CAS-conflict test.
- Absence (FACT): the main.rs flag list above.

**3. `designos_point {node_ids}`: the agent shows the human which layer it means, without moving the view (idea 2, adapted).**
- Shape: an MCP tool that emits a presence event. `agent_presence.rs` (#682) moves the cursor and flashes the nodes; if they are off-screen, the G6 chip "Agent points at N → Show" appears. It does not change the selection, does not move the view (owner ruling, #695), adds no undo step, and makes no `TileView::rev` bump.
- Value: closes the "which one?" loop on the canvas.
- Size: 1-2 days.
- Risk: agent spam; rate-limit it.
- Verify: a stub call, then a 1440 screenshot showing the cursor, flash and chip; a test that the view and the selection are unchanged.
- Absence (FACT): the 37-tool snapshot.

**4. `get_app_state include_view=true` (from idea 2).**
- Shape: the page-space visible rect, the zoom, and the top-level ids in view. Write them into context.json (rm terminal.rs:2918) and expose them only on request, like the selection under the #698 ruling.
- Value: answers "here" and "what I'm looking at".
- Size: about 4 h.
- Risk: low. It is viewport-only state.
- Verify: a unit test that copies `selection_only_by_explicit_argument` (rm bootstrap.rs:199).
- Absence (FACT): `viewport` has 0 hits.

**5. Execute the examples in tool descriptions (the fix for idea 8's weak spot).**
- Shape: a test in `tools_snapshot.rs` that parses every JSON example in the descriptions of `query_xpath`, `apply_batch`, `apply_spec` and `search_design_system`, validates it against `inputSchema`, and runs it on a fixture.
- Size: about 3 h.
- Verify: a deliberately broken example makes the test fail.
- Absence: INFERENCE. "example" has 0 hits in tools_snapshot.rs; I did not grep the other test files.

## 5. Do NOT copy
- Auto-scrolling to new content, and a `point` that moves the camera or the selection: this contradicts the owner ruling of 2026-10-19 (#695).
- A `help` tool: MCP `tools/list` plus `get_guidelines` already carry the schema.
- The Vite module-hash revision plugin and the live-iframe budget: both are web-only. We have the GPU caps.
- `lastIds` merge: we have an op log, not whole-file replace.
- States-as-files: component variants are native.
- Comment/frontmatter ID adapters and opaque random IDs for every resource: NodeId in the document is enough.
- The module system, literal-only declaration reader, dual-root ownership, before-side authority and manual Git upgrade: these serve team forks. Ours is a single-owner, PR-protected product.
- The `activity` field: it is dead code there, and hooks-based turn status (#683) already exists here.
- The prose volume (21k words of context docs).
- An unauthenticated global dev API like `window.__studioCanvas`.

## 6. Our own defects surfaced
| ID | Defect | Severity | Fix |
|---|---|---|---|
| D0 | The local checkout is at 767acf9, 15 days behind remote 8f267b3. The scout and the controller verified a stale tree, and this session's CLAUDE.md still says "29 tools" | High (decision quality) | Fast-forward main (`git pull --ff-only`) before any scout; scouts read `origin/main` |
| D1 | The plugin manifests say "36-tool" while the snapshot has 37, and version "0.2.0" while the workspace is 0.7.6 | Medium (public) | Gap #1 |
| D2 | Root `llms.txt` says "22-tool" (#1014 is open) | Medium | Add it to the gen-tool-docs targets |
| D3 | `.claude-plugin` and `.codex-plugin` declare no `mcpServers`. INFERENCE: installing the plugin does not register designos-mcp | Unknown; needs a host check | Check against the host docs; add the entry or document the manual step |
| — | Scout risks (29 vs 22, unadvertised tools, `/tmp` collision, unread selection) | Already resolved on main | None |

## Unresolved questions
1. Is D3 intentional, because the binary is installed separately?
2. Does the external `ui` generator already produce Gemini, Grok and ChatGPT bundles?
3. Gap #3 sends a pointing event the canvas must show. Should it go through `doc-patch.jsonl` (a non-document line kind) or a separate presence channel?

Status: DONE_WITH_CONCERNS
Summary: Verified both repos. Most of the design-studio ideas already ship on remote main. There are 5 GAPs, led by generated host-bundle manifests and a headless `--call` transport.
Concerns/Blockers: the local checkout is stale (D0). Nothing was executed. Gap #5's absence was grepped in one file only.
