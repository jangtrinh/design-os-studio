# #655 @-mention picker in the agent composer

| Field | Value |
|---|---|
| Issue | #655 (agent terminal UX list; audit "On-demand context via @-mention") |
| Size / priority | L / P2 |
| Depends on | #654 (shares the layer reference text); T7.9 / T7.38 only for library components (see rule 6) |
| Unblocks | none |
| Touches | new `crates/designos-app/src/agent_mention.rs` (search and reference format), new `crates/designos-app/src/agent_mention_popup.rs` (UI), `crates/designos-app/src/terminal.rs` (hot spot: composer `:2940-3101`, `submit_chat_prompt` `:1029-1155`, `provision_session_agent_files` skill text `:917-945`), tests |

Code baseline: `origin/main@01d48f2` (0.5.0).

## 1. Summary and user story

As a designer writing to the agent, I type `@`, search my file for a layer, page, component, variable or style, pick
it, and the agent can read exactly that thing through designos-mcp, so I do not have to describe it or select it
first.

## 2. Current state (FACT)

- There are two input surfaces in the agent panel. The default one is the live PTY viewport
  (`side_panel_pty_open = true` at startup, `terminal.rs:743`), where the user types into the agent's own TUI and
  Studio has no input box. The other is the chat-mode composer, a single-line `TextEdit` with the hint "Ask for
  changes" (`agents-ask-for-changes-input`, `terminal.rs:2960-2973`), reachable through the header "AI" toggle
  (`:2670-2685`). Its prompt runs as a one-shot CLI command typed into a shell tab (`submit_chat_prompt`, `:1029-1155`).
- The composer has context tags, not a picker: the `+` button prepends `@selection @prd` (`:2979-2996`), the "PRD" chip
  cycles `@prd`, `@selection`, `@tokens` (`:3010-3051`), and the mic button inserts a PRD template (`:3053-3067`). The
  tags are plain text; `@prd` points at an internal spec path (`submit_chat_prompt` `:1086-1094`).
- Layer search helpers: `find::filter::matcher` and `rows(doc, page, matcher)` search layer names on one page
  (`crates/designos-app/src/find/filter.rs:15-50`).
- What the agent can read through MCP today (`designos-mcp/src/engine.rs`): `designos_get_node(node_id)` (`:80`),
  `designos_export_spec(node_id)` (`:271`), `designos_list_nodes(query, parent_id, limit <= 500)` (`:57`),
  `designos_get_variables` (all variables and collections, `:84`), and `designos_get_document`, which lists styles with
  `key`, `name`, `property` and values (`:42-56`). There is no MCP tool for library components that are not nodes of the open
  file (T7.38 `designos_get_libraries` and T7.9 `designos_search_design_system` are in this pack, not yet on main).
- The session skill text tells the agent where the context files are (`terminal.rs:917-945`); it knows nothing about mentions.
- Node ids are stable inside a session except where a live patch remaps them (`designos-doc/src/live_patch/mod.rs`,
  `LivePatch.base`); references therefore carry a name and kind next to the id.

## 3. Behaviour rules

1. **Trigger.** Typing `@` at the start of the text or after a space opens the mention popup anchored above the input.
   Typing filters it; Esc closes it without changing the text. (Which surfaces get it: open question 1.)
2. **Sources and kinds.** Results come from the open file only, in five kinds:
   - layers: every layer of every page by name (page name shown);
   - pages: page nodes;
   - components: main components and component sets of this file (`NodeKind::Component`, `ComponentSet`);
   - variables: `doc.variables` by name (collection shown);
   - styles: `doc.styles` by name (property shown).
3. **Search.** Case-insensitive; ranking: exact name, then word-prefix, then substring; layers on the current page rank
   above other pages. At most 8 results per kind and 30 in total. The scan runs with a per-frame time budget of 4 ms and
   continues next frame (with one repaint request) when it is not finished, so a 1,000,000-layer file never blocks the
   frame; results refine as it proceeds. Hidden layers are listed, greyed.
4. **Picking** inserts the label `@<name>` at the caret and records a mention `{ label, kind, id, name, page }` in the
   input's side list. The visible text stays short.
5. **Reference format.** When the prompt is sent (composer submit, or the paste of rule 7), each `@label` that is in the side
   list is followed in a footer block by its reference:
   `References: @Hero/Title = designos:node/4521 (text layer "Hero/Title", page "Landing")`. The grammar is
   `designos:<kind>/<id>` with kind `node` (layers, pages, components: all are nodes), `variable` (the variable id) or `style` (the
   style key). The name, kind and page next to the id are for the agent to check: if `designos_get_node` fails or returns another
   name, the agent searches by name before giving up.
6. **How the agent resolves it** (the contract the session skill teaches, one paragraph added to `SKILL.md` in
   `provision_session_agent_files`):
   - `designos:node/<id>`: call `designos_get_node` with `node_id`, and `designos_export_spec` with `node_id` for the layer's
     NodeSpec (the "scoped spec": only that layer and its children);
   - `designos:variable/<id>`: call `designos_get_variables` and read that id;
   - `designos:style/<key>`: call `designos_get_document` and read that key from `styles`.
   Library components that are not nodes of the file are not offered in this task; they follow T7.9 / T7.38 (open question 2).
7. **No inline spec.** The mention never inlines a layer's spec or screenshot into the prompt: the agent pays tokens only for
   what it chooses to read through MCP.
8. Editing or deleting an `@label` in the text drops its entry from the side list; a label that no longer appears in the text is
   not sent.
9. A prompt with no mentions is sent exactly as today.
10. Applies to all CLIs: the footer is plain text in the prompt.

## 4. Data model / schema changes

- `Mention { label, kind: MentionKind, id: String, name, page: Option<String> }` and the composer's `mentions: Vec<Mention>` (in
  memory, per `TerminalState`).
- `MentionKind::{Node, Variable, Style}` for the reference grammar; no change to `designos-doc`.

## 5. Contracts

- Reference grammar `designos:<node|variable|style>/<id>` (rule 5), documented in `SKILL.md` and in the module header.
- MCP: unchanged. The resolution path uses existing tools only (rule 6). A test pins each tool call that resolves a mention.

## 6. File-format mapping

None.

## 7. UI

Described in words. Checked against `design/principles.md` (P1 to P10) and `design/soul.md`; the rules used, with the `theme.rs` constants, are recapped in README-part3 under "UI rules checked for part 3". In short: every colour, gap, inset, control height and radius below is a `crates/designos-app/src/theme.rs` constant (the panel's own `TERM_*` / `STATUS_*` constants at `terminal.rs:29-55` stay for the terminal viewport and the existing tab status dots and are not extended); selected and active rows use the neutral fill `BG_SELECTED`; the brand orange is the view's one primary action; counts sit in their own pill, not in a label.

The issue fixes: typing `@` searches layers, pages, components, variables and styles, and the picked item becomes a resolvable reference. The look is a proposal for the owner's confirmation:

- A popup above the input, width of the input, `RADIUS_FRAME`, `BG_MENU` with a `BORDER_MENU` hairline, inset `INSET_POPOVER`. Kind headings ("Layers", "Pages", "Components", "Variables", "Styles") in caption `TEXT_SECONDARY`, `SPACE_GROUP` above and `SPACE_FACT` below (P9). Rows are `CONTROL_ROW` high, two lines: a vector icon of the kind and the name (matched text in body-medium), and a caption line with the page or collection; the active row uses the neutral `BG_SELECTED` fill; hover `BG_MENU_HOVER`. At most 8 rows are visible, then it scrolls.
- Keyboard: Up/Down move, Enter or Tab picks, Esc closes; every row meets `TARGET_MIN`.
- An empty result shows one line "Nothing in this file matches" (caption, `TEXT_SECONDARY`).
- No animation; the popup exists only while open (no idle cost).
- Desktop captures at 1440: popup with mixed results, filtered, no results; responsive SKIPPED (owner default). Load es-designer before UI code; update the Figma frames from the captures first.

## 8. Edge cases

1. Two layers with the same name: both listed, with their page and a short path hint; each pick has its own id.
2. A layer deleted after the pick: the reference stays; the agent's `designos_get_node` fails and it searches by name (rule 5).
3. `@` inside an email address (`a@b.com`): no popup (the trigger needs start or a space before it).
4. A name containing `@` or spaces: the label is the name with spaces kept; the side list maps it, the footer is unambiguous.
5. A 1,000,000-layer file: first results within 50 ms, refined over frames (rule 3).
6. Pasting text that contains `@Hero` by hand: no side-list entry, sent as plain text.
7. Variable with the same name in two collections: both listed with their collection.
8. Locked or hidden layers: listed (greyed when hidden); picking is allowed.

## 9. Non-goals

- Inlining content, screenshots or specs into the prompt.
- Mentions inside the CLI's own TUI input when Studio does not own the input (see the open question).
- Library components not present in the file (T7.9 / T7.38).
- Mentions in canvas comments (#358).

## 10. Test plan (red first)

Seams: pure functions in `agent_mention.rs` (search, footer building, grammar parser), a `Sim` test for the popup, and one
engine test that resolves a mention through designos-mcp.

| Case | Expected (fails before) |
|---|---|
| `search_finds_all_five_kinds` | a synthetic document with a layer, a page, a component, a variable and a style named alike: one result per kind, ranked exact first |
| `search_ranks_current_page_first` | same name on two pages |
| `search_is_budgeted_on_a_huge_document` | the synthetic 1,000,000-layer generator (large-fixture pattern): the first call returns within the 4 ms budget with a continuation |
| `footer_lists_each_mention_once` | two uses of one label: one footer line |
| `deleted_label_is_not_sent` | remove the text, submit: no footer |
| `grammar_round_trip` | every kind builds and parses back |
| `mentioned_layer_is_resolved_through_designos_mcp` | build the prompt, parse the `designos:node/<id>`, call `Engine::call("designos_get_node", {node_id})` and `designos_export_spec` on the same document: the returned name equals the mention's (the issue's acceptance) |
| `style_and_variable_resolution_use_existing_tools` | `designos_get_document` returns the style key; `designos_get_variables` returns the variable id |
| `popup_keyboard` | `Sim`: type `@he`, Down, Enter: label inserted, side list has one entry |
| `no_popup_inside_an_email` | typing `a@b` opens nothing |
| `skill_text_documents_the_grammar` | `SKILL.md` content contains the three resolution lines |

Run: `cargo test -p designos-app` and `cargo test -p designos-mcp`. Evidence: red/green capture, the three 1440 captures, and one
owner-manual run with a real agent (Claude): the agent answers about a mentioned layer after calling `designos_get_node` (screenshot
of the tool call in the terminal).

## 11. Performance budgets

- Per frame while the popup is open: at most 4 ms of scanning, then a continuation; zero cost when closed.
- Memory: no index kept for the whole file; results are at most 30 rows.
- Footer building O(mentions).

## 12. Dependencies and build order

After #654 for the shared layer-reference wording; library components wait for T7.9 / T7.38. The composer lives in
`terminal.rs`, a hot spot: the search, grammar and popup are new files; `terminal.rs` gets the input hook and the submit path.

## 13. Owner rulings

- Typing `@` searches layers, pages, components, variables and styles; the picked item is inserted as a resolvable reference
  (`node_id` plus a scoped spec) that the agent reads through MCP. (Issue text.)
- The selection is never auto-injected; mentions are explicit. Related decided ruling, unchanged: MCP tools default to the editor selection when attached, configurable per user and session
  (owner decision 4 in `plans/261017-epics-full-scope/owner-decisions-and-cross-check.md`); a mention names a thing explicitly and works whatever that default is.
- Skip-permissions stays the default.

## Open questions for the owner

1. **Which input gets the picker.** The default panel is the live terminal, where the user types into the agent's own screen;
   the composer exists only in chat mode.
   - A (recommended): both. The chat-mode composer gets the `@` popup, and the terminal view gets a "Mention" button in the panel header
     that opens the same popup and pastes the reference text into the agent tab (no Enter). This covers the default surface without
     reading keystrokes inside another program.
   - B: chat-mode composer only. Smallest, but the default surface has no mentions.
   - C: intercept `@` typed in the terminal view. It reads the user's keystrokes to guess when a prompt starts and breaks when a CLI changes
     its prompt; not recommended.
2. **Library components.** Components from enabled libraries are not nodes of the file.
   - A (recommended): leave them out until T7.9 / T7.38 land in this pack, then add them as a sixth kind.
   - B: offer them now by name only, with no way for the agent to resolve them.
3. **What the inserted text looks like in the prompt.**
   - A (recommended): the short `@Hero/Title` label in the text, with the reference in a footer block, so the sentence reads normally.
   - B: the full reference inline, `designos:node/4521`, which is exact but noisy to read and edit.
4. **Overlap with the CLIs' own `@file` mentions.** Claude Code and others treat `@` as a file mention in their own input.
   - A (recommended): keep `@` for Studio's picker only in the places Studio owns (composer, Mention button) and paste plain text into the
     CLI, so the CLI's own completion is untouched.
   - B: use a different trigger character for Studio's picker (for example `#`) to avoid any confusion.
