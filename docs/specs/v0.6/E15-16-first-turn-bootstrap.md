# E15-16 Prefetch design context on an agent's first turn

| Field | Value |
|---|---|
| Epic / pack | E15 / v0.6 AI design |
| Size / priority | M / P0 |
| Depends on | T7.2 (#239), T7.4 (#241), T7.38 (#247), T7.9 (#272), E15-15 |
| Unblocks | RD-3 (spawned agents start with pre-fetched context), part-2 E15-36 |
| Touches | `crates/designos-app/src/terminal.rs` (agent runtime bridge), new `crates/designos-export/src/agent_context.rs` (`bootstrap` builder, next to T7.2's `metadata_xml`), new `crates/designos-mcp/src/tools/bootstrap.rs` (`designos_get_app_state`) |

## 1. Summary and user story

As an agent started from Studio, my first turn already contains the open file, its pages, a depth-3
outline of the frames I am working on, a variables summary, the components and
libraries I can use, the active style, and the rules pack, marked as current, so my first tool call is
a write, not five discovery reads. Agents outside Studio get the same block from
`designos_get_app_state`. The canvas selection is never injected into any agent's prompt (owner
ruling): an agent gets it only by calling `designos_get_app_state` with `include_selection: true`.

## 2. Current state (FACT)

- Studio writes `context.json` with session id, document name, file path, current page and selected ids
  only (`crates/designos-app/src/terminal.rs:769-802`; `selected_node_ids` at `793-800`).
- The Claude Code `UserPromptSubmit` hook already prints `context.json` into the prompt context
  (`terminal.rs:869-883`), selected ids included; other CLIs get only the launch command text
  (`terminal.rs:140-151`). Printing the selected ids contradicts the owner ruling that the selection
  is never auto-injected, so this task removes them from that print (3.2).
- First prompts go through `submit_chat_prompt` (`terminal.rs:1006`) into the PTY.
- For large files the agent's canvas is a scoped copy: current page, at most 128 top-level frames as
  shells, selected frames in full up to 5,000 layers (`crates/designos-app/src/live_canvas_scope.rs:1-14`).
- designos-app depends on designos-export and designos-library at runtime and on designos-mcp only as
  a dev-dependency (`crates/designos-app/Cargo.toml:29-36`, `57-59`).

## 3. Behaviour rules

### 3.1 Content (sections in priority order, each with a token cap)

| # | Section | Source | Cap (tokens) |
|---|---|---|---|
| 1 | Freshness header: generated time (RFC 3339), document revision, "current as of now; do not re-read; your tool replies report changes" | Studio | 60 |
| 2 | Document: name, file path, kind, page list (id, name, top-level count), current page | doc | 300 |
| 3 | Target frames outline: depth 3 (`metadata_xml`, T7.2) of each explicit target top-level frame (RD-3 lease frames, or `target_node_ids`), else of the current page's top-level frames. Never derived from the canvas selection | T7.2 builder | 2,500 |
| 4 | Variables summary: per collection name, modes, count, and the first 20 variable names with values in the default mode | doc | 1,200 |
| 5 | Components and libraries: local component sets/components (first 50 names with variant property names) and the T7.38 library list | T7.38, doc | 1,200 |
| 6 | Active style: the applied archetype id and parameters (part-2 E15-19 records it), else `none` | doc | 200 |
| 7 | Rules pack (E15-15) for the session's enabled tools | E15-15 | 6,000 |

Total budget 12,000 tokens (chars / 4). Each section that hits its cap ends with an explicit marker,
e.g. `[truncated: 1,240 more nodes in outline; call designos_get_metadata {"node_id":812,"depth":3}]`.
Never a silent cut. Sections 1-2 are never truncated below their content.

### 3.2 Delivery

- Studio builds the block off the UI thread on agent run start (new run id, E15-7) from its full
  document snapshot (not the scoped canvas) and writes `<session_dir>/bootstrap.md` plus
  `bootstrap.json` (same content, structured).
- Claude Code: the `UserPromptSubmit` hook prints `bootstrap.md` once per run (a marker file
  `bootstrap.delivered.<run>` prevents repeats); later prompts get only `context.json` as today,
  printed without `selected_node_ids` (the file keeps the field for Studio's own use; the hook strips
  it from the printed text).
- Other CLIs (Codex, Antigravity, Grok): `submit_chat_prompt` prepends the block to the first prompt
  of the run, inside a fenced `designos-context` block.
- RD-3 spawned agents: the lead's `spawn_agents` passes each agent's frames as its targets (section 3
  uses them).

### 3.3 `designos_get_app_state`

Returns the same structure built by the MCP from its session document (scoped canvas when attached to
Studio; the response says `scoped: true` then), plus session facts: enabled tools, ledger size
(E15-6), session options (E15-12/13). Arguments: `target_node_ids` (optional), `max_tokens` (default
12,000, 2,000..=25,000), `sections` (optional subset), `include_selection` (default `false`; when
`true` the reply adds a `selection` section with ids, names, kinds and page-space bounds, 300 tokens).
This explicit argument is the only way the selection reaches an agent.

## 4. Data model / schema changes

- `designos_export::agent_context::{Bootstrap, BootstrapSection, bootstrap(doc, &BootstrapOptions,
  libraries, rules) -> Bootstrap}`, rendering to markdown and JSON.
- Session files `bootstrap.md`, `bootstrap.json`, marker files; removed when the session directory is
  cleaned.
- No document change.

## 5. Contracts

Tool `designos_get_app_state` (`readOnlyHint:true, idempotentHint:true`):

```json
{"type":"object","additionalProperties":false,"properties":{
  "target_node_ids":{"type":"array","items":{"type":"integer","minimum":1},"maxItems":50},
  "max_tokens":{"type":"integer","minimum":2000,"maximum":25000,"default":12000},
  "sections":{"type":"array","items":{"enum":["document","outline","variables","components","style","rules"]}},
  "include_selection":{"type":"boolean","default":false}}}
```

Output:

```json
{"generated_at":"2026-10-18T21:44:00Z","doc_revision":4182,"scoped":false,
 "sections":[{"id":"outline","tokens":2480,"truncated":true,
   "truncation_note":"1,240 more nodes; call designos_get_metadata {\"node_id\":812,\"depth\":3}",
   "content":"<frame id=\"812\" ...>"}],
 "markdown":"# design:os context ...","total_tokens":11820,"session":{"enabled_tools":[...],"ledger_entries":3}}
```

Errors: `NODE_NOT_FOUND` (target id), `VALUE_OUT_OF_RANGE`.

## 6. File-format mapping

None.

## 7. UI

No new controls. The Agents chat shows the first user turn as typed by the user; the attached context
is indicated by a small `Context attached` caption (11 px, secondary text) under that turn, which opens
`bootstrap.md` in the raw terminal view when clicked (real action; Zero fake UI).

## 8. Edge cases

1. No explicit targets and a page with 5,000 top-level frames: outline covers the first frames in paint order
   up to the cap, with a marker naming the remainder.
2. File with no variables or components: sections say so explicitly.
3. Agent restarted within the same session: new run id → new bootstrap.
4. User changes selection after the run started: nothing is sent; an agent that needs the selection
   calls `designos_get_app_state` with `include_selection: true`.
5. Bootstrap build slower than the user typing the first prompt: the prompt waits at most 500 ms, then
   sends with a compact block (sections 1-2 and the rules) and a note to call
   `designos_get_app_state` for the rest.
6. Rules pack disabled tools: section 7 (rules pack) reflects the enable list.

## 9. Non-goals

- Re-sending context every turn.
- Summarising document content with a model (R4.13: none bundled).

## 10. Test plan

Seam: recorded-fixture test with a stub agent CLI (a shell script that logs its stdin and the tool
calls it would make, used as the agent command).

| Case | Expected |
|---|---|
| `first_turn_contains_every_section` | stub run on the tokens fixture → recorded first prompt (or hook output) contains all 7 section headers |
| `selection_never_injected` | stub run with 3 nodes selected → first prompt, hook output and every later prompt contain none of the selected ids or names as a selection, `context.json` printed by the hook has no `selected_node_ids`, and `bootstrap.json` has no `selection` section; outline targets are the page's top-level frames |
| `selection_only_by_explicit_argument` | same selection; `designos_get_app_state` without the argument vs `include_selection: true` | no `selection` section vs a `selection` section listing the 3 nodes |
| `large_fixture_within_budget_with_markers` | synthetic large fixture (README) → `total_tokens ≤ 12000`; outline section has a truncation note naming a follow-up call |
| `stub_agent_zero_discovery_calls` | stub agent that writes immediately using ids from the bootstrap → MCP log shows the first call is a write (no get_document/list_nodes/get_metadata before it) |
| `app_state_tool_matches_studio_block` | same document → `designos_get_app_state` sections equal Studio's (except `scoped`) |
| `delivered_once_per_run` | two prompts in one run → block only in the first |

## 11. Performance budgets

Building the block on the large fixture: ≤ 300 ms off the UI thread (outline and summaries are capped
walks, never full-document serialization; CLAUDE.md §4.1). The UI thread only spawns the job and reads
the result.

## 12. Dependencies and build order

After T7.2, T7.4 (context format conventions), T7.38, T7.9 and E15-15. Before RD-3's prefetch step and
part-2 variant exploration.

## 13. Owner rulings

- All AI through the user's agent CLI (R4.13): context goes only to the user's own CLI.
- Parallel agents hard-locked to frames (RD-3): spawned agents get their own frames as targets.
- Full scope: every listed section, with explicit truncation, ships together.
