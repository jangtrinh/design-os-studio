# #688 Agents panel empty state: sample prompts that send a real prompt, and attach hints

| Field | Value |
|---|---|
| Issue | #688 (parent #585) |
| Size | S |
| Depends on | #681 (the template actions are gone); soft: #652 (install guidance), #654 (selection chip), #655 (@-mentions) |
| Unblocks | none |
| Touches | new `crates/designos-app/src/agent_sample_prompts.rs` (loader and context choice), new data file `crates/designos-app/src/agent_sample_prompts.toml`, `crates/designos-app/src/terminal.rs` (hot spot: the empty state `:2790-2940`), `crates/designos-app/Cargo.toml` (`toml` as a direct dependency), tests |

Code baseline: `origin/main@9f3f7a5`. The issue cites `terminal.rs:2604-2630`; on this commit those lines are the file and page strip. The "What's next?" empty state is at `terminal.rs:2790-2940`.

## 1. Summary and user story

As a designer opening the Agents panel on a file, I want a few prompts that fit what I am looking at, one click to put one in the composer, and a line that tells me how to give the agent my selection. Nothing is sent until I press Send.

## 2. Current state (FACT)

- The empty state shows when `chat_turns` is empty in chat mode: a plus badge, the heading "What's next?" and four category cards ("Tidy and polish", "Generate designs", "Create plugins and shaders", "PRD & Spec Driven v2") that expand into fixed action rows (`terminal.rs:2790-2940`). Chat mode is `side_panel_pty_open == false`; the default panel is the terminal view (`:743`).
- Clicking a row calls `submit_chat_prompt` and `dispatch_figma_ai_prompt` at once, so a click sends (`:2911-2940`). One of them also applied a template (#681 removes it). Four rows name planning paths (`:2884-2896`).
- The cards, rows and text use colour and size literals, not theme tokens (`:2800-2830`).
- The composer is a single-line `TextEdit` with id `agents-ask-for-changes-input` and `state.input_line` (`:2959-2975`, `:646`).
- Installed agent CLIs: `AgentCliPreset::is_installed()` (`:193-198`). #652 (not on `main`) specifies the install guidance and the picker.
- `designos-app` has no `toml` dependency (no `toml::` use under `src`); `toml` is in `Cargo.lock` as a transitive crate.
- The app knows the selection (`app.selection`), the current page (`app.page`) and node kinds (`doc.get(id).kind`). The panel painters receive only `TerminalState` (`show_left(state, ui, rect, file_path)`, `:2471`); #654 passes a summary struct the same way and this task reuses that path.
- #654 (selection chip) and #655 (@-mention) are not on `main`, so the hint line cannot name them yet.

## 3. Behaviour rules

1. **Contexts**, chosen in this order: `empty_file` (no page of the document has a layer), `frame` (the selection holds at least one Frame, Component, Component set, Instance or Section), `no_selection` (everything else, including a selection of other kinds).
2. **Up to six prompts** for the chosen context, in file order. Each has a short label and a full text.
3. **Click** puts the prompt text in the composer and focuses it. It never sends, never touches the canvas, and writes nothing to a PTY. If the composer already holds text, the prompt is appended after a space; the user's text is never replaced.
4. **Data.** Prompts are a TOML list in `agent_sample_prompts.toml`, compiled in with `include_str!`. The loader rejects: an unknown context, more than six per context, an empty label or text, text over 300 characters, and any text containing `plans/`, `.md` or a path separator-led word. A test loads the shipped file.
5. **Hint line**, one line under the prompts, built only from features that exist: "Click the selection chip to send what is selected." (when #654 is present) and "Type @ to mention layers." (when #655 is present). With neither, no hint line.
6. **No agent CLI installed.** When none of Claude, Codex, Grok and Antigravity is installed, the empty state shows #652's install guidance instead of prompts and hint. Until #652 lands it shows one line "No agent CLI found" with the install page of each CLI as links (`agent_start_failure::install_url`).
7. The four category cards and every fixed action row are removed; their real actions (Open PTY, Send selection, Apply spec, Sync canvas) remain reachable from the panel controls they already have (header and dock bar).
8. The wording of the prompts is the owner's; the starter set below is a proposal.

Starter set (agent-neutral, each uses a real Studio capability):

- `no_selection`: "Describe this page" (Describe the screens on this page and how they relate.), "Find design problems" (Check this page against the file's variables and styles and list what breaks them.), "Build a new screen" (Design a new screen for this file that follows its existing styles.), "Name the layers" (Rename the layers on this page so each name says what the layer is.).
- `frame`: "Review this frame" (Review the selected frame for spacing, alignment and contrast problems.), "Make it resize well" (Make the selected frame resize well with auto layout.), "Bind to variables" (Bind the selected frame's colours and spacing to this file's variables where they match.), "Shorten the text" (Rewrite the text in the selected frame to be shorter and clearer.), "Try another layout" (Make a second version of the selected frame with a different layout.).
- `empty_file`: "Start a landing page", "Start a dashboard", "Start a settings screen", "Start a mobile sign-in" (each: Design a first draft of a <thing> for this file.).

## 4. Data model / schema changes

None in the file format. `SamplePrompt { context, label, text }` in memory.

## 5. Contracts

`fn sample_prompts(ctx: PromptContext) -> &'static [SamplePrompt]` and `fn prompt_context(doc, page, selection) -> PromptContext` (pure, tested). The caller passes a small `PanelContext { page_empty, selection_kind }` to the painter, like #654's `SelectionSummary`.

## 6. File-format mapping

None.

## 7. UI

Described in words; checked against `design/principles.md` P1 to P10 and `design/soul.md`.

- The heading "What's next?" stays (large strong type). Under it, up to six rows, each `CONTROL_ROW` (40 px) high, full width minus `INSET_PANEL`, `RADIUS_CONTROL`, fill `BG_SECONDARY`, `BORDER` hairline; hover `BG_HOVER`. The label is body-medium in `TEXT`; the full text is one caption line in `TEXT_SECONDARY`, elided. Each row is one pointer target of at least `TARGET_MIN`. Rows are `SPACE_ROW` apart. All colours and sizes are `theme.rs` constants, which retires the literals in this block.
- The hint line is caption type, `TEXT_SECONDARY`, `SPACE_GROUP` below the last row.
- The no-CLI state is the picker's guidance (#652) or one text line with link buttons of `CONTROL_STANDARD` height.
- No animation; no timer; the empty state repaints on events only.
- Desktop capture at 1440 for the three contexts and the no-CLI state. Responsive: SKIPPED (owner default). Load es-designer before UI code; update the Figma frames from the captures first.

## 8. Edge cases

1. Selection changes while the empty state shows: the prompts change in the next frame; the composer text is untouched.
2. A selected layer implies the file is not empty, so `empty_file` and `frame` never both hold; the order in rule 1 only breaks ties between `frame` and `no_selection`.
3. Hidden or locked frames count as frames.
4. The composer text is non-empty: the prompt is appended with a space (rule 3).
5. A prompt chip is clicked twice: the text is appended twice; the user can edit.
6. The data file is edited to hold a bad entry: the loader test fails the build.
7. Narrow panel: labels elide; rows keep their height.

## 9. Non-goals

- Sending on click; localised prompts; a prompt editor UI; remembering clicks.

## 10. Test plan (red first)

Seams: pure tests for the loader and the context function; the `Sim` harness for the panel.

| Case | Expected | Fails before |
|---|---|---|
| `shipped_prompts_load_within_limits` | at most 6 per context, no planning path, text under 300 | yes (no file) |
| `loader_rejects_bad_entries` | unknown context, 7 entries, empty text each rejected | yes |
| `context_is_chosen_in_order` | empty file, frame, no selection, text selection | yes |
| `the_three_contexts_show_their_prompts` | recorded rows match the file | yes |
| `a_click_fills_the_composer_and_changes_nothing_else` | `input_line` equals the text; doc hash equal; stub PTY got 0 bytes | yes (click sends today) |
| `a_click_appends_to_an_existing_draft` | draft kept, prompt after it | yes |
| `no_cli_installed_shows_install_guidance` | no prompt rows; guidance rows | yes |
| `hint_names_only_existing_features` | no hint without #654 and #655 | yes |
| `old_category_cards_are_gone` | no `agents-cat-*` ids | yes |

Run: `cargo test -p designos-app terminal agent_sample_prompts`; `python3 scripts/zero_fake_ui_gate.py`.

## 11. Performance budgets

One context computation per frame over the selection's first node and the current page's child count (O(1), no tree walk); the prompt list is a static slice. No timer.

## 12. Dependencies and build order

After #681 (it removes the template path this empty state used) and preferably after #652, #654 and #655 so the hint and the no-CLI state are real. `terminal.rs` gets call sites only. Merge order is in README-part4.

## 13. Owner rulings

- Owner ruling 2026-10-19 (#689, work like Orca ADE): the terminal is the conversation. Sample prompts show only when an agent CLI is detected and the tab has no conversation yet; a click fills the composer and never sends. With no CLI detected the empty state reads "No agent CLI detected. Install one or pick a default agent in Settings" with the #652 install guidance.

- Up to six sample prompts per context; a click fills the composer, focused, and never sends; prompts are data (a TOML list); one hint line about #654 and #655; install guidance when no CLI is installed. (Issue text.)

## Open questions for the owner (recommended first)

1. Where the empty state and composer live. They appear only in chat mode today; the default panel is the terminal view. Recommended: keep them in chat mode for this task and decide the panel mode in #689.
2. The wording of the sample prompts. Recommended: the starter set above, edited by the owner in the TOML file.
