# #242 Target the editor selection by default, configurable per user and session

| Field | Value |
|---|---|
| Issue | #242 (task T7.5, epic #20; milestone v0.7 Effects & Export; owner ruling R1.4) |
| Size | M |
| Depends on | issue: #238 (T7.1), #75, #76, #77, #81 (#78 and #80 are closed). This spec's reading: hard #77 (protocol version in `context.json`); #238 soft (the resolver is schema-driven, so the screenshot tool joins when it lands); #75, #76, #81 touch the edit log, not `context.json` (INFERENCE, open question 4). #75, #76, #77 and #81 have no milestone; their 2026-10-17 amendments say "Blocks T7.5 ... Proposed: move to milestone v0.7". |
| Unblocks | none |
| Touches | new `crates/designos-mcp/src/tools/target.rs` (resolver), `crates/designos-mcp/src/server.rs` (`LiveSessionBridge` `:19-35`, `sync_from_live_canvas_if_newer` `:194-240`), `crates/designos-mcp/src/engine.rs` (argument structs `NodeArgs` `:584`, `CodeArgs` `:604`, and the seven required-`node_id` tools), `crates/designos-app/src/terminal.rs` (hot spot: `sync_live_file_context` `:800-860`, per-tab override state, one header control), `crates/designos-app/src/panels/menus.rs` (Preferences submenu `:513-519`, `Action` enum `:108-118`), `crates/designos-app/src/comments/settings.rs` (one string choice), `crates/designos-app/src/live_sync_tests.rs`, new MCP tests |

Code baseline: `origin/main@0e8a9a5` (`docs(specs): v0.6 task specs, part 4 (agent terminal UX and constraint solver) (#697)`). v0.7 opens after v0.6 is tagged. The worker re-anchors every cited line against HEAD before writing code and reports drift first.

## 1. Summary and user story

As a designer working with an agent in Studio, I select a layer and say "make this darker". The agent calls a tool without naming a layer and the tool acts on my selection, the way Figma's desktop MCP server does. As a power user I can change that default for all my sessions or for one agent tab: use the selection, require an explicit layer id, or use the current page. A stale selection, an empty one or a detached session gives a clear, named error, never a guess.

## 2. Current state (FACT)

1. **Figma.** The public tools page (read 2026-10-19) documents no `nodeId` fallback and no selection default for any tool; it says only that prompting with the current selection works on the desktop server and not on the remote one. Ruling R1.4 in the issue ("like Figma MCP, but more open") therefore describes behaviour the page does not spell out. The owner's decision 4 (a) says yes, when attached to a Studio window (`plans/261017-epics-full-scope/owner-decisions-and-cross-check.md:51`).
2. **Which tools need a node today** (`designos-mcp`): `node_id` is required by `designos_get_node`, `designos_generate_code`, `designos_update_node`, `designos_delete_node`, `designos_export_spec`, `designos_set_dev_status` (`engine.rs:735-816`, the `add(...)` table) and `designos_render_png` (`server.rs:625`). It is optional on `designos_get_variables`, `designos_add_comment` and `designos_list_comments`, where leaving it out already means "whole document" or "by page". `parent_id` is optional on `designos_list_nodes`, `designos_create_node` and `designos_apply_spec` and defaults to the active page (`Engine::resolve_default_page`, `engine.rs:30-35`).
3. **The page is already a default; the selection is not.** The server reads `current_page_id` from `context.json` on every call (`server.rs:222-229`; env fallback `DESIGNOS_TARGET_PAGE_ID`, `:230-234`). Nothing in `designos-mcp/src` reads `selected_node_ids`.
4. **Studio writes the selection.** `sync_live_file_context` puts `selected_node_ids`, `current_page_id` and the page name in `context.json` whenever `(mutation epoch, page, selection hash)` changes (`terminal.rs:822-835`), then writes `canvas.designos` after it (`:840-848`). The two files are two atomic writes, not one: a reader can see the new selection with the previous canvas. The write is skipped while an edit is in progress, agent edits are held or layout is draining (`:808-813`).
5. **The canvas can be scoped.** Above 25,000 nodes the copy is `scoped_canvas`, which ships the selected frames whole and others as shells (`terminal.rs:843-845`; `crates/designos-app/src/live_canvas_scope.rs:17`, `:54`).
6. **The #654 ruling.** The accepted spec of the selection chip (`docs/specs/v0.6/654-live-selection-chip-click-to-send.md`, rules 2 and 13) says: the selection is never auto-injected into a prompt; nothing is sent until the user clicks the chip; and it names this task's ruling as a related, unchanged decision: "the chip controls what is pasted into the prompt; it does not replace the tool default". The test `terminal.rs:5607` asserts that the prompt hook's context reply never contains `selected_node_ids` while the file does (`:5610`). The agent's system prompt already tells it to read `$DESIGNOS_CONTEXT_JSON` (`terminal.rs:305`), so it can already see the selection by reading that file.
7. **Is there a conflict with #654?** No conflict with its rule: the selection is read inside the tool call, on the server side; nothing is added to a prompt and no tokens are spent until the agent calls a tool. There is one tension with its intent ("click to send"): with a selection default, a tool can act on a selection the user never sent. The ruling that settles it is R1.4 and decision 4 (a), so this spec follows them and asks the owner about writes (open question 1).
8. **Settings.** App settings are `settings.json` in the library root (`comments/settings.rs:8`); Preferences is a menu with "Your name..." and "Use GPU rendering" (`panels/menus.rs:513-519`). There is no per-agent-tab setting store on `main`. v0.6's #656 and #682 add a Preferences entry for agent settings and a `load_choice` / `save_choice` pair beside `load_flag` (their specs, not yet on `main`).
9. **Errors.** Failures are free text today (`engine.rs:37`, `Result<Value, String>`). v0.6's E15-5 adds named codes with `safe_to_retry`; it appends new codes to its catalogue (`docs/specs/v0.6/E15-5-named-errors-structured-output.md`, section 3.3).
10. `crates/designos-app/src/live_sync_tests.rs` exists and is the issue's test seam.

## 3. Behaviour rules

1. **Scope: only a call that fails today changes.** The fallback applies to tools whose `node_id` is required (fact 2, and any tool a later task adds with a required `node_id`: `designos_get_screenshot` (#238), the metadata tool of T7.2 and `designos_get_design_context` (T7.4)). A call that names its `node_id` behaves as before. A tool where `node_id` or `parent_id` is optional keeps its meaning: no previously valid call changes result, in any mode.
2. **Modes.** `selection`, `node_id`, `page`. They only decide what a missing `node_id` means; the tool list, schemas and options are identical in every mode (a test compares `tools()` across modes).
   - `node_id` (require): a missing `node_id` returns `NODE_ID_REQUIRED` naming the tool and the other two ways to proceed.
   - `page`: a missing `node_id` means the active page's root node (`resolve_default_page`). Existing guards still apply (for example a page root cannot be deleted).
   - `selection` (the factory default when attached): see rule 3.
3. **Selection mode.**
   - The session is attached when `context.json` is readable in the session directory. A detached session (standalone `designos-mcp`, no session directory) behaves as mode `node_id`, with `NODE_ID_REQUIRED` saying the session is detached.
   - Read `selected_node_ids` once at the start of the call.
   - None selected: `NO_SELECTION`.
   - More than one: `AMBIGUOUS_SELECTION`, with the ids and names in `valid_options` (up to 20). The tool never picks "the first".
   - Exactly one, but it is not in the session document: `STALE_SELECTION`, listing the missing id.
   - Exactly one and present: the tool runs on it.
4. **Echo.** A reply from a call that used the fallback carries `"target": { "source": "selection" | "page", "node_ids": [..] }`, so the agent and the transcript always show what was acted on.
5. **Errors** (E15-5 envelope once it exists; plain messages before): `NODE_ID_REQUIRED`, `NO_SELECTION`, `AMBIGUOUS_SELECTION`, `STALE_SELECTION`. All four are `safe_to_retry: true` (nothing changed), carry a `next_step` sentence, and `STALE_SELECTION` says to re-read with `designos_get_document` or ask the user to select again. They are appended to E15-5's catalogue.
6. **Settings.** Studio resolves the effective mode and writes it into `context.json` as `"target_default": "selection" | "node_id" | "page"`; the server reads only that field (one resolver, live updates, no MCP read of `settings.json`).
   - Per user: `agent_target_default` in `settings.json`, set from Preferences.
   - Per session: an override held by the agent tab for the life of the tab; "use my default" clears it. It is not saved across restarts (non-goal).
   - Order: session override, then user default, then the factory default `selection`.
   - The effective mode is part of the `last_synced_key` (`terminal.rs:822`), so changing it rewrites `context.json` even when the document and selection did not change.
7. **Protocol version.** The new field rides on the protocol version of #77. A designos-mcp that does not know `target_default` ignores it and requires `node_id` as today; #77's mismatch error is the user-visible signal.
8. `designos_get_document` reports `target_default` and `attached` (not the selected ids).
9. **Inside a batch** (E15-4, if its ops may omit `node_id`): the selection is read once when the batch starts, so later ops cannot see a changed selection.
10. Nothing here injects the selection into a prompt, the system prompt or a hook reply; the test of `terminal.rs:5607` stays green.

## 4. Data model / schema changes

`context.json` gains `"target_default"`. `settings.json` gains `agent_target_default` (a string). In memory: the tab's override. No document change.

## 5. Contracts

- `context.json` field `target_default` (string, one of three). Missing or unknown value means "require" for safety.
- Error codes of rule 5; the reply field `target` of rule 4.
- `fn resolve_node_target(ctx: &ContextSnapshot, doc: &Document, page_root: NodeId) -> Result<NodeId, TargetError>` (pure; the server passes what it already read).

## 6. File-format mapping

None. Figma has no equivalent setting; the Figma MCP page does not document a fallback, so there is nothing to map.

## 7. UI

Described in words. Checked against `design/principles.md` (P1 to P10) and `design/soul.md`; every colour and size is a `theme.rs` constant; `ui ds context --dir .` is run before the PR; the controls are menu rows and a pill that follow the existing patterns.

- **Per user.** Preferences gets a submenu "Agent target" with three radio rows in the style of "Use GPU rendering" (`panels/menus.rs:517`): "Use the selected layer", "Require a layer id", "Use the current page". The active row is the checked one. No counts, no icons beyond the radio mark.
- **Per session.** A `CONTROL_DENSE` (24 px) pill menu in the agent panel's header strip, beside the selection chip of #654 (it uses the same strip; it never moves the chip). It shows the effective mode word ("Selection", "Layer id", "Page"); a click opens the three rows plus "Use my default". Neutral fill, caption type, `TEXT_SECONDARY` when it follows the default and `TEXT` when overridden. A tooltip says what the mode does in one sentence.
- No animation and no timer. Zero fake UI: the pill shows the mode that is in `context.json`, nothing else.
- Evidence: desktop captures at 1440 of the Preferences submenu, the header pill closed and open, and an agent tab after an override. Responsive: SKIPPED (owner default). Load `es-designer` before UI code. Update the Figma frames from the verified captures first (owner rule). The exact placement is a proposal for the owner (open question 3).

## 8. Edge cases

1. The user clears the selection while the agent works: the next unnamed call returns `NO_SELECTION`; a call already running used its snapshot.
2. The selection changes between two unnamed calls: each call reads it fresh; the echo shows which layer it used.
3. `context.json` is newer than `canvas.designos` (fact 4): the selected id may not be in the server's copy yet, so the call returns `STALE_SELECTION` (retry-safe); the agent calls `designos_get_document` and retries.
4. The agent deleted the selected layer: the id is gone in the server's copy at once, `STALE_SELECTION` until Studio reports the new selection.
5. A scoped canvas (above 25,000 nodes): a selected frame is whole up to a budget and a shell past it (`live_canvas_scope.rs`, test `a_selected_frame_past_the_cap_is_still_shared`); a read of a shell returns what the copy holds. This task does not change that.
6. Two Studio windows, two agents: each session directory has its own `context.json`, so each agent sees its own selection.
7. An old Studio with a new designos-mcp: no `target_default` field, so the mode is "require"; #77's version check reports the mismatch.
8. A selected page node or hidden layer: valid targets like any layer.
9. A tool with a required `node_id` added later without being covered: a test that reads every tool's schema fails until the tool is covered or listed as an exception.
10. Multi-selection of 10,000 layers: the error lists 20 names and says how many more.

## 9. Non-goals

- Following the selection for `parent_id` (creating inside the selected frame): it stays the active page (open question 2).
- Pushing the selection into prompts or hook replies (#654's ruling).
- Saving the per-session override across restarts, and per-folder or per-file defaults.
- Making the agent see the selection in screenshots (T7.1) or in design context (T7.4).
- Changing the 25,000-node scoping threshold.

## 10. Test plan (red first)

Seams: pure-function tests for the resolver; `designos-mcp` session tests with a temporary session directory holding `context.json` and `canvas.designos`; `live_sync_tests.rs` and `shell` tests for the setting and the pill; the existing `terminal.rs` context test.

| Case | Expected | Fails before |
|---|---|---|
| `omitted_node_id_uses_the_single_selection` | `context.json` selects node 7; `designos_get_node {}` returns node 7 with `target.source == "selection"` | yes (error today) |
| `explicit_node_id_wins_in_every_mode` | the same call with `node_id: 9` returns node 9 in all three modes | guard |
| `require_mode_errors` | `NODE_ID_REQUIRED` with `safe_to_retry: true` | yes |
| `page_mode_targets_the_page_root` | omitted id returns the active page's root; `delete_node {}` is refused by the existing page-root guard | yes |
| `no_selection_is_a_named_error` | empty `selected_node_ids`: `NO_SELECTION` | yes |
| `several_selected_is_ambiguous_and_lists_them` | 3 ids: `AMBIGUOUS_SELECTION` with 3 `valid_options`; nothing changed (document hash equal) | yes |
| `a_selected_id_missing_from_the_document_is_stale` | id 99 absent: `STALE_SELECTION` naming 99 | yes |
| `detached_session_requires_node_id` | no session directory: `NODE_ID_REQUIRED` mentioning detached | yes |
| `optional_node_id_tools_keep_their_meaning` | `designos_get_variables {}`, `designos_list_nodes {}`, `designos_add_comment` without node: same results in all modes | guard |
| `tool_list_is_identical_in_every_mode` | `tools()` equal across modes | guard |
| `every_required_node_id_tool_is_covered` | reflection over `tools()`: each tool with a required `node_id` is in the resolver's set | yes |
| `setting_precedence` | session override beats user default beats factory `selection`; "use my default" clears the override | yes |
| `mode_change_rewrites_context_json` | change the mode with an unchanged document and selection: `context.json` is rewritten with the new `target_default` (the `last_synced_key` includes it) | yes |
| `context_reply_still_excludes_the_selection` | the assertion at `terminal.rs:5607` stays green | guard |
| `old_context_without_the_field_means_require` | no `target_default`: `NODE_ID_REQUIRED` | yes |
| `preferences_submenu_and_header_pill` | `Sim`: the three rows exist, the checked one follows the setting; the pill shows the effective mode and opens its menu | yes |
| `zero_fake_ui_gate_passes` | `python3 scripts/zero_fake_ui_gate.py` | guard |

Run: `cargo test -p designos-mcp`, `cargo test -p designos-app live_sync terminal menus`, `python3 scripts/zero_fake_ui_gate.py`.

## 11. Performance budgets

- Server: the resolver reads fields from the `context.json` the call already parses (`server.rs:222-229`); under 0.1 ms per call. Id lookups are map lookups (`doc.nodes` is a `BTreeMap`).
- Studio: one more field in the key and one more string in the JSON it already writes; no extra serialisation of the document, no UI-thread `to_json` (CLAUDE.md section 4), no timer. The header pill paints from an in-memory value.

## 12. Dependencies and build order

After #77 at least (rule 7). `crates/designos-mcp/src/engine.rs` is a hot spot: this task touches the argument parsing of seven tools, so it lands last among the MCP tasks of the pack (after #244, #246 and the codegen targets) and rebases onto them. `terminal.rs` is shared with other agent-panel work; the pill and the key change are call sites into a new small module (`agent_target.rs`), per the pack's rule. Merge order is in the README.

## 13. Owner rulings

- Owner ruling 2026-10-19: when an agent calls a read, edit or delete tool without `node_id`, the tool defaults to the editor selection and its reply names the layers it acted on. This applies only to calls that fail today; the selection chip stays click-to-send.

- R1.4: like Figma MCP (selection default when attached) but more open: configurable, expose everything. (Issue text.)
- Decision 4 (a): MCP tools default to the editor selection when attached to a Studio window; otherwise `node_id` is required (`owner-decisions-and-cross-check.md:51`).
- The selection is never auto-injected into a prompt; the chip is click-to-send (#654, accepted spec; this task does not touch it).
- Calls without `node_id` use the configured default; the default is settable per user and per session; every tool and option stays exposed; a stale selection gives a clear error; detached sessions require `node_id`. (Issue acceptance criteria.)

## Open questions for the owner (recommended first)

1. **Do writes follow the selection default?** This is the one place the task meets the "click to send" intent of #654.
   - A (recommended): yes, for every required-`node_id` tool, including `designos_update_node` and `designos_delete_node`, as R1.4 says "expose everything". The reply always names the layer it acted on, and every agent call is one undo step.
   - B: reads and `designos_render_png` follow the selection; edits and deletes still need an explicit `node_id` in every mode.
2. **`parent_id` tools** (`designos_create_node`, `designos_apply_spec`). Recommended: unchanged (active page). Creating inside a selected frame would change today's behaviour for flows the system prompt describes ("deliver all designs onto that file and page"). Alternative: in selection mode, one selected container becomes the default parent.
3. **Where the per-session control lives.** Recommended: a small pill in the agent panel's header strip next to the selection chip, with a "Use my default" row. Alternative: a row in the tab's own menu once the agent panel has one, or the Preferences submenu only (user level only).
4. **Dependencies.** The issue lists #75, #76, #77 and #81. Recommended: schedule #77 (needed) in v0.7 first, and let #75, #76 and #81 proceed independently, since they change the edit log, not `context.json` (INFERENCE; check by reading their diffs when they land). Alternative: keep all four as hard dependencies.
