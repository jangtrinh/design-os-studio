# #689 Agent composer: send into the tab's live CLI session, with prompt history and a draft per tab

| Field | Value |
|---|---|
| Issue | #689 (parent #585) |
| Size | L |
| Depends on | #651 (open flow: start or resume the file's session), #653 (stable sessions), #681 (no template, no canned transcript); soft: #655 (reference chips) |
| Unblocks | #688 (sample prompts fill this composer), #683 and #677 (they hang off the one live session) |
| Touches | new `crates/designos-app/src/agent_composer.rs` (per-tab composer state, history, send queue), `crates/designos-app/src/terminal.rs` (hot spot: `submit_chat_prompt` `:1029-1160`, `build_real_cli_command` `:286-326`, composer `:2949-3100`, chat thread `:3206-3322`, `sync_chat_from_pty` `:1160`), `scripts/zero_fake_ui_gate.py` (`Z01`, `Z02`) and its self-test, tests |

Code baseline: `origin/main@9f3f7a5`.

## 1. Summary and user story

As a designer, when I type to the agent in the composer, my message goes into the same conversation I see in the terminal. The agent remembers what I said before, my history is one arrow key away, and my half-typed message waits for me in each tab.

## 2. Current state (FACT)

- A composer message runs a **one-shot** CLI call typed into a **shell** tab: `claude --dangerously-skip-permissions -p '<prompt>'`, `codex ... exec '<prompt>'`, `agy ... -p`, `grok --always-approve -p` (`build_real_cli_command`, `terminal.rs:286-326`; `submit_chat_prompt` wraps it in `printf` markers and `send_line_clearing_pending`, `:1097`, `:594-600`). There is no `--continue`. The agent forgets earlier messages; the session the user resumes never sees them.
- The composer is a single-line `TextEdit` (`TextEdit::singleline`, id `agents-ask-for-changes-input`, `:2959-2975`) over `state.input_line`, one `String` per document tab (`:646`). It exists only in chat mode (`side_panel_pty_open == false`); the default panel is the terminal view with no composer (`:743`). Enter submits (`:2971`).
- The composer box is a fixed 96 px (`:2777`). The chat thread shows user and assistant bubbles; the assistant text is cut out of the PTY log between `DESIGNOS_CHAT_BEGIN_<id>` and `END_<id>` markers (`:1173-1174`).
- Pasting into a live agent tab already exists: `paste_into_agent_tab` writes `bracketed_paste_bytes(text)` only to a live agent tab (`:1444-1452`, `:1545-1552`). `format_paste_bytes(text, bracketed)` wraps in bracketed paste when the CLI enabled it or the text has newlines, and replaces ESC with `U+241B` (`:1558-1570`). The terminal reports the CLI's bracketed-paste mode through `vt100::Screen::bracketed_paste` (`:3701`).
- Shift+Enter in the terminal is sent as `ESC CR` (`:3768`).
- Copying layers on the canvas puts clipboard JSON on the system clipboard (`App::copy_selection`, `shell.rs` copy handler near `:519`); `clipboard::from_json` reads it (`clipboard.rs:327`).
- `ensure_default_tab` spawns a Shell tab when none exists (`:1004-1008`); #651 replaces that with `open_for_agent_use`.
- Gate checks that bind to the one-shot path: `Z01` requires `claude -p`, `codex exec`, `agy -p` and `grok -p` to appear in `submit_chat_prompt` or `build_real_cli_command` (`scripts/zero_fake_ui_gate.py:473-496`); `Z02` requires `DESIGNOS_AGENT_RUNTIME` in `build_real_cli_command` (`:499-515`).
- UNKNOWN: whether each CLI turns on bracketed paste, and whether a paste followed at once by Enter is taken as one submit. Check: for each CLI, read `screen().bracketed_paste()` after start and send a two-line paste plus `\r` in a captured run.
- UNKNOWN: how long after spawn a CLI is ready for input. Check: measure the first quiet period per CLI.

## 3. Behaviour rules

1. **Send writes to the live tab.** The text goes into the active agent tab's PTY as a bracketed paste (`ESC[200~ … ESC[201~`; multiline text is always bracketed), then `\r` as a separate write after a per-CLI delay: 50 ms for Claude; for the other CLIs 500 ms plus 1 ms per 4,096 bytes of the paste, unless a CLI is measured to need Enter in the same write. The delay is a deadline checked each frame (one `request_repaint_after`), never a UI-thread sleep. The conversation is the tab's own session. No `-p`, `exec` or shell line is built on this path; `build_real_cli_command` and its use in `submit_chat_prompt` are removed.
2. **Not running.** If the tab's CLI is not running, Send first runs #651's `open_for_agent_use` (resume the file's last session, else the picker). Where the CLI accepts a first prompt on launch, the prompt goes with the launch: argv for Claude, Codex and Grok; the interactive-prompt flag for Gemini and Antigravity. For the other CLIs the message waits in a queue and is written when the tab is ready (alive, past the start window, PTY quiet for the measured period). If the start fails (#650's surface), the queue is dropped and the text returns to the composer draft. Nothing is lost.
3. **Keys.** Enter sends; Shift+Enter inserts a newline. The field grows from 1 to 8 lines and then scrolls. An empty field cannot send.
4. **History.** Up arrow on an empty field, or while browsing, walks this tab's sent prompts newest first, up to 100. Down walks back and ends on the half-typed draft. History is per tab, in memory, and dies with the tab.
5. **Drafts.** Each tab keeps its own draft. Switching tabs or sessions swaps drafts; nothing is cleared.
6. **References.** Pasting layers copied from the canvas (clipboard JSON that `clipboard::from_json` reads) inserts a reference chip in #655's format, not raw JSON. Without #655 the paste inserts the layer names in plain text and nothing more.
7. **While running, with a gate.** Send stays enabled while a turn runs; the text goes to the CLI as keystrokes would, and the CLI decides whether to queue it. Send is refused, with the reason shown under the composer, when: no agent CLI runs in the tab (`no-agent`), the CLI waits at a permission or selection prompt (`permission`; answer it in the terminal or the #656 card), the tab is still starting (`not-ready`), or the PTY cannot be written (`not-writable`). The first version detects the prompt state from the session hooks where #683 provides them, else from the PTY being quiet with bracketed-paste mode on. Selection prompts are answered with keystrokes in the terminal, never by pasting a label.
7a. **Receipts, never an automatic resend.** Each send ends as `Written`, `Queued`, `Refused(reason)` or `Unconfirmed`. When the turn-start hook has not arrived 10 s after a write, the composer shows "Sent, not confirmed: check the terminal". Studio never resends on silence. A failed start puts the text back in the draft with "Not delivered: check the terminal".
8. **Entry points share one path.** The composer, #688's sample prompts, "Make design" (#681) and `dispatch_figma_ai_prompt` all call one `TerminalState::send_to_agent(text)`.
9. **The terminal is the conversation** (owner ruling 2026-10-19: work like Orca ADE). The chat bubble thread is retired: `submit_chat_prompt`'s chat turns, `sync_chat_from_pty` and the BEGIN/END markers are removed. The composer is an optional front door over the same PTY at the bottom of the terminal view; the user can always type in the terminal directly. A structured chat, if ever added, is a separate opt-in session type, never a mirror of the terminal.
10. Limits: a message over 100,000 characters is refused with a status line; ESC bytes in the text are replaced as `format_paste_bytes` does today.
11. Gate: `Z01` becomes "the composer path writes to the live tab and builds no `-p` or `exec` line"; `Z02` keeps the `configure_pty_env` check only. The self-test mutates both.

## 4. Data model / schema changes

- `ComposerState { draft, history: VecDeque<String> (max 100), cursor: Option<usize>, stash: String }` per tab; `pending_send: Option<String>` per tab. In memory.
- Remove `TerminalState.input_line` as the shared field.

## 5. Contracts

- `send_to_agent(&mut self, text: &str, file_path) -> SendOutcome { Written, Queued, Refused(reason) }`. Used by every entry point of rule 8.

## 6. File-format mapping

None.

## 7. UI

Described in words; checked against `design/principles.md` P1 to P10 and `design/soul.md`.

- The composer box grows with its text from one line to eight (`font_body` line height), then scrolls; its minimum height is `CONTROL_ROW` (40 px) plus the control row. It uses `RADIUS_FRAME`, fill `BG_SECONDARY`, a `BORDER` hairline, and the focus ring in the brand orange. The fixed 96 px box and the `#0D99FF` outline (`:2951-2953`) become tokens.
- The Send button is a circle of `CONTROL_STANDARD` (28 px), the view's one primary action, in `BG_BRAND` with a dark arrow. It is disabled and `TEXT_SECONDARY` when the field is empty.
- A reference chip is a pill of `CONTROL_DENSE` (24 px), `radius_full`, `BG_TERTIARY`, caption type, with the layer name elided; a component layer uses the `component` cyan, as everywhere.
- A queued message shows the caption "Starting <agent>… your message will be sent" under the field; it has no spinner of its own.
- The attach `+`, book and PRD-tag controls keep their jobs until #655 and the owner decide otherwise.
- Desktop capture at 1440: a three-line draft, history browsing, a chip, the queued note, two tabs with two drafts. Responsive: SKIPPED (owner default). Load es-designer before UI code; update the Figma frames from the captures first.

## 8. Edge cases

1. The CLI is not in bracketed-paste mode and the text has newlines: send it as one line with spaces for newlines, and say so once in the status line (UNKNOWN which CLIs need this).
2. The user presses Up with text in the field: it moves the text cursor; history starts only on an empty field.
3. The history already holds the same text as the last entry: not added twice.
4. The agent exits while a message is queued: the surface shows; the text returns to the draft.
5. Send during a start-failure panel: refused with the panel's own message.
6. Two tabs send in the same frame: each writes to its own PTY.
7. Switching documents: composer state belongs to the tab, so it stays with its document tab.
8. A very fast double Enter: the second sees an empty field and does nothing.
9. Shell tabs: the composer is disabled with "Select an agent tab"; no shell line is ever built.

## 9. Non-goals

- Persisting history across restarts (open question 2). A queue for a running CLI. Voice input. Streaming the answer into bubbles.

## 10. Test plan (red first)

Seams: a stub CLI script that appends every pasted block to a history file and answers with the number of blocks it has seen; unit tests for the history ring and the draft swap; the `Sim` harness.

| Case | Expected | Fails before |
|---|---|---|
| `two_messages_in_a_row_reach_one_session` | the stub's second answer says it has seen two blocks | yes (one-shot calls) |
| `no_one_shot_call_remains_on_the_composer_path` | shipped `submit_chat_prompt` and `send_to_agent` hold no `-p ` or ` exec ` command line; the stub receives pastes only | yes |
| `enter_sends_shift_enter_adds_a_newline` | PTY bytes and field text | yes |
| `field_grows_to_eight_lines_then_scrolls` | recorded height | yes |
| `up_and_down_walk_history_and_restore_the_draft` | 100 entries max, order, draft back | yes |
| `each_tab_keeps_its_draft` | draft A and B after switching | yes |
| `not_running_starts_then_sends_when_ready` | the message arrives after the start; once | yes |
| `failed_start_returns_the_text_to_the_draft` | draft equals the message | yes |
| `pasting_copied_layers_makes_a_chip` | chip in #655 format (with #655), names only without | yes |
| `all_entry_points_use_one_path` | sample prompt, Make design, palette action reach the same fn | yes |
| gate `Z01` and `Z02` self-test | pass on the new shape; fail if `-p` returns | yes |

Run: `cargo test -p designos-app terminal agent_composer`; `python3 scripts/zero_fake_ui_gate.py`; `python3 scripts/zero_fake_ui_gate_selftest.py`. Owner-manual: one real conversation per CLI where the second message depends on the first.

## 11. Performance budgets

- Composer layout per frame: one text layout of at most the visible 8 lines; under 0.2 ms. History and drafts are O(1) per key.
- The write is a single `write_all`; the `\r` is written on the next frame, never after a sleep on the UI thread.
- No `to_json`; no timer. The queue is checked on frames that already run.

## 12. Dependencies and build order

After #651, #653, #681 and, for chips, #655. `terminal.rs` is the hot spot: `agent_composer.rs` holds the logic; `terminal.rs` loses the one-shot code and gets call sites. Merge order is in README-part4. Part 3's #651 spec says the composer starts a one-shot shell on first submit; this task removes that behaviour, so #651's implementer should leave that line out.

## 13. Owner rulings

- Send writes the prompt into the active tab's running CLI with bracketed paste then Enter; the conversation is the tab's session; start or resume first when the CLI is not running; Enter sends and Shift+Enter inserts a newline; 1 to 8 lines; Up arrow history of 100 per tab; a draft per tab; a pasted layer reference becomes a chip; Send stays available while a turn runs. (Issue text.)
- Skip-permissions stays the default; `GROK_FOLDER_TRUST=0` stays.
- Owner ruling 2026-10-19: work like Orca ADE. The agent CLI's own TUI in the tab's PTY is the only conversation; the chat bubble thread is retired; the composer writes into the live session by bracketed paste and a delayed Enter, gated on the agent's state, with receipts and no automatic resend.

## Open questions for the owner (recommended first)

1. (Answered 2026-10-19, see §13: work like Orca ADE.) Where the conversation shows. Recommended: the terminal is the conversation; the composer sits at the bottom of the terminal view in both modes; the chat bubble thread is retired because assistant text cannot be cut from a live TUI.
2. History across restarts. Recommended: not persisted in v0.6; a stable session (#653) already keeps the CLI's own history.
