# Scout 261102 — Studio agent surface inventory (Haiku, read-only)

Persisted by controller from the scout's hand-back. STALE: built on a local checkout 85 commits behind origin/main; several claims are wrong on main — see review-261102-design-studio-adoption-ruling.md §1.

## 1. MCP tools
- 22 advertised = 18 engine (`crates/designos-mcp/src/engine.rs` `tools()` 705-829) + 4 server (`server.rs:623-626`: open_file, save_file, render_png, new_document).
- Dispatch-only, not advertised: `designos_query_xpath` (engine.rs:307), `designos_lint` (engine.rs:350).
- Drift: CLAUDE.md §5 says 29 tools; code/plugin.json/docs say 22.

## 2. Selection/page/viewport exposure
- `context.json` written by `TerminalState::sync_live_file_context` (terminal.rs:769, fields 793-800): session_id, doc_name, file_path, current_page_id, current_page_name, selected_node_ids. No viewport.
- MCP reads only `current_page_id` (server.rs:226); `selected_node_ids` read nowhere outside a test (server.rs:721).
- Also writes global `/tmp/designos-live-context.json` (terminal.rs:803-805).

## 3. Agent-driven select/scroll/zoom
- None in designos-mcp. App-only: `zoom_to_selection`/`zoom_to` (ops.rs:1582,1594). Applied patches select inserted roots (live_patch_apply.rs ~92-95).

## 4. Live edit-log / undo / conflicts
- MCP appends one line per tool call to `doc-patch.jsonl` (server.rs:107, 252-262). App `LivePatchLog` (live_patch_log.rs) + `apply_pending_live_patches` (live_patch_apply.rs:50), held while user mid-edit (:17-24), one `Tx` per patch = one undo step (:112-126).
- No content version check; remaps reset when `patch.base` differs (designos-doc live_patch/replay.rs:91-101); ops on missing nodes skipped + reported (replay.rs:141).

## 5. CLI vs MCP
- CLI flags in cli.rs (256-266), dispatch main.rs:92-93 → run_agent_cli :176. Shared core only `Document::apply_spec`/`export_spec` (designos-doc spec.rs:360, 614). Parsing, persistence, comments are separate paths.

## 6. Self-describing help
- No help/example tool. Only `description` strings (engine.rs:705-829) + server instructions (server.rs:637).

## 7. Skills / host bundles
- `skills/design-os-studio/SKILL.md`; `.agent/skills/` 22 design-os-* skills; `.agents/agents/` 4 project agents; plugin manifests: `.claude-plugin/`, `.codex-plugin/`, `.factory-plugin/`, `.agents/plugins/marketplace.json`. No Gemini manifest. No in-repo per-host generator (external `ui` binary from ease-design).

## 8. AGENTS.md / CLAUDE.md
- AGENTS.md hand-written + generated ease-design block (95-157). CLAUDE.md hand-written.

## 9. Node identity
- `NodeId(pub u64)` counter (designos-doc lib.rs:88-92, alloc 341-352). Stable across save/load; copy/paste allocates fresh ids (app clipboard.rs:262-266). MCP and editor have separate counters.

## 10. Agent code in app
- terminal.rs (PTY agent terminal + context sync), live_patch_apply.rs, live_patch_log.rs, live_canvas_scope.rs, cmdk.rs, specs.rs, shell.rs wiring, RailItem::Agents.

## 11. design/ + gates
- design/ 22 entries (principles, soul, tokens, memory.*). zero_fake_ui_gate.py (957 lines, Z01-Z14), design_os_self_evolve.py (615 lines).

## Risks flagged
- Tool count drift 22 vs 29; unadvertised tools; global /tmp context files collide across windows; no content version check; `.agent/` and `.agents/` both exist.

Status: DONE_WITH_CONCERNS (report persisted by controller)
