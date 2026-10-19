# #681 Agents panel shows agent output that no agent produced (template on send, canned transcript, fake mic)

| Field | Value |
|---|---|
| Issue | #681 (bug; Zero Fake UI law, `design/principles.md` P6) |
| Size | M |
| Depends on | #650 (the "agent failed to start" surface, closed and on `main`) |
| Unblocks | #688 (sample prompts), #689 (composer sends into the live CLI) |
| Touches | `crates/designos-app/src/terminal.rs` (hot spot: `submit_chat_prompt` `:1029-1160`, the empty state `:2790-2940`, the composer `:2949-3100`), `scripts/zero_fake_ui_gate.py` and `scripts/zero_fake_ui_gate_selftest.py`, tests in `terminal.rs` |

Code baseline: `origin/main@9f3f7a5`. Every line number below was read on that commit.

## 1. Summary and user story

As a designer, I want the Agents panel to show only what my agent did. When I press Send, the canvas changes only if the agent writes to it through its tools.
When the agent CLI is missing, I want to be told that, with the fix, and not to read a transcript that says something ran.

## 2. Current state (FACT)

- Pressing Send runs `submit_chat_prompt` and then the composer applies a template: for any prompt not starting with `$` or `!`, `cmdk::synthesize_spec_from_prompt(&prompt)` is turned
  into `TerminalAction::ApplySpecJson` (`terminal.rs:3089-3100`, the call at `:3093`). This runs even when the real CLI ran.
- `synthesize_spec_from_prompt` (`cmdk.rs:632-651`) picks a template by keyword (`pricing`, `login`, `table`, `console`, `sidebar`, `hero`) and falls back to a generic card for every other prompt.
- `submit_chat_prompt` also builds a template (`synth_spec`, `terminal.rs:1106-1112`) when the prompt looks like design creation (`is_design_creation_prompt`, `:329-334`, which matches words such as `make`, `card`, `screen`) or when the real CLI did not run. It stores it in the assistant turn as `extracted_spec_json`.
- `run_real_cli` is false when the binary is missing, when `DESIGNOS_FORCE_LOCAL_SYNTH` is set (`:1075`), and always under `cfg!(test)` (`:1074`). In that case the shell prints
  `Status: Executed via <cli> CLI shell & streamed to Canvas` (`:1099-1103`). Nothing ran.
- The microphone button (`agents-composer-mic`) inserts the fixed text `@prd Audit the selected frame against the PRD acceptance criteria` (`:3058-3068`, template at `:3061`).
  Studio has no speech-to-text code: a search of `crates/designos-app/src` and `crates/designos-app/Cargo.toml` for "speech" and "dictat" finds nothing (INFERENCE from the search; the check is `cargo tree -p designos-app | grep -i speech`).
- The dock action "Make design (First Draft)" applies `synthesize_spec_from_prompt("pricing card")` after submitting its prompt (`:2930-2938`, call at `:2933`).
- Prompts that contain `prd`, `req-` or `us-` get `PRD Context: plans/specs/designos-studio-v2/prd/PRD.md ...` echoed into the transcript (`:1084-1094`).
  The empty state's "PRD & Spec Driven v2" category sends prompts that name `plans/specs/designos-studio-v2/...` (`:2884-2896`). That is an internal planning folder, not the user's file.
- The command palette has two template paths: the row `cmdk-synth-spec` titled "Make an image" (`cmdk.rs:146-152`, handler `:470`) and the image-search button (`cmdk.rs:363`, tooltip "Search with image or generate visual draft"). Both apply the same template. Their labels do not say "template".
- The gate checks the wrong thing for this bug: `Z01` requires that `submit_chat_prompt` mentions `runtime_binary_available` and spells `claude -p`, `codex exec`, `agy -p`, `grok -p` (`scripts/zero_fake_ui_gate.py:473-496`); `Z04` requires the id `agents-composer-mic` to have a click handler (`:530-548`).
- Two existing tests assert the fake text: the PRD echo (`terminal.rs:4425-4440`) and the mic template (`:4564`).
- The default Agents panel is the terminal view; the composer, the empty state and the chat thread appear only in chat mode (`side_panel_pty_open == false`; default `true` at `:743`; chat mode toggles at `:2684`, `:3007`).

## 3. Behaviour rules

1. A prompt sent from the composer never applies a template. Delete the `synthesize_spec_from_prompt` block in the composer (`:3089-3100`) and the `synth_spec` value in `submit_chat_prompt` (`:1107-1112`). The canvas changes only through the agent's own MCP writes, replayed from `doc-patch.jsonl`.
2. Delete the canned transcript branch (`:1099-1103`), `DESIGNOS_FORCE_LOCAL_SYNTH`, `is_prd_query` and `prd_context_line` (`:1084-1094`). No code path prints a status that claims a CLI ran.
3. When the active CLI is not installed, Send does not type anything into a shell. It switches the panel to the terminal view, opens an agent tab for that CLI, and lets #650's surface show the cause and the fix.
   The tab exits within the start window, so `agent_start_failure::diagnose(preset, false, ...)` yields `NotInstalled` (INFERENCE: `build_agent_shell_cmd` runs the CLI through a shell, which exits 127; the first test below proves it).
4. The microphone button is removed (owner ruling allows "removed until it can"). `agents-composer-mic` leaves the layout, the hit test and the gate list. Speech-to-text is a separate task.
5. "Make design" sends a prompt to the agent through the same send path as the composer. It applies no template. The prompt text is data in one constant (open question 1 holds the recommended wording).
6. No user-facing text names a planning folder. Remove the "PRD & Spec Driven v2" category (`:2884-2896`) and the `@prd` text inserted by the mic. #688 later replaces the empty state's categories with sample prompts.
7. The command palette keeps `cmdk-synth-spec` and the image-search button, because the owner kept explicitly labelled template commands. Their titles must say what they do (open question 2). No other module may call `synthesize_spec_from_prompt`.
8. Gate `Z15` fails when shipped code of `terminal.rs` calls `synthesize_spec_from_prompt` or contains the literal `Executed via`. `Z01` and `Z04` are updated for the new shape (rule 4; `Z01` keeps its `runtime_binary_available` check). The self-test re-introduces both regressions in memory and asserts `Z15` fails.

## 4. Data model / schema changes

None. `AgentChatTurn.extracted_spec_json` stays for specs the user copies from a real CLI output; this task removes only the value Studio invented.

## 5. Contracts

- No MCP or log format change.
- A test seam: `TerminalState` gets a `cfg(test)`-only `stub_cli: Option<PathBuf>`. When set, `submit_chat_prompt` runs that script as the CLI instead of the real binary. This replaces the blanket `!cfg!(test)` guard at `:1074`, which today makes every test take the fake branch.

## 6. File-format mapping

None.

## 7. UI

Described in words; checked against `design/principles.md` P1 to P10 and `design/soul.md`.

- The composer loses the microphone circle. The remaining controls keep their places (attach `+`, book, Send). Pointer targets stay at least `TARGET_MIN` 24 px.
- The empty state loses the fourth category card. The three others stay as they are until #688.
- A missing CLI shows #650's existing panel over the terminal view: title "<CLI> failed to start", the cause, the last output lines, one fix button. No new surface.
- No new colour, spacing or animation. Idle repaint behaviour is unchanged.
- Evidence: a desktop capture at 1440 before and after, with the same prompt ("pricing") and a stub CLI that writes nothing: before, a pricing card appears; after, the canvas is unchanged. Also a capture of the missing-CLI surface. Responsive: SKIPPED (owner default). Load es-designer before UI code; run `python3 scripts/zero_fake_ui_gate.py`.

## 8. Edge cases

1. A prompt that starts with `$` or `!` is a raw shell line today. It keeps working in a shell tab; nothing here changes it.
2. A stub CLI that does write one MCP edit: the edit is replayed as one agent step; no second change appears.
3. A prompt sent while the agent tab is dead (`accepts_agent_input` false): the existing message "Agent exited" shows (`:1140-1143`).
4. The user has two CLIs installed and the active one is missing: Send acts on the active runtime only; the surface names it.
5. `extracted_spec_json` from a real output block can still be applied by an explicit click on its bubble (`:3314-3322`). It is the agent's text, so it stays.

## 9. Non-goals

- Speech-to-text.
- The sample prompts (#688) and the live-CLI send path (#689).
- Renaming or removing palette commands; the PRD tag chip (open question 3).

## 10. Test plan (red first)

Seams: `terminal.rs` unit tests with the `stub_cli` seam, the `Sim` harness for the panel, the gate self-test.

| Case | Expected | Fails before |
|---|---|---|
| `prompt_with_a_stub_cli_that_writes_nothing_leaves_the_document_hash_unchanged` | send "pricing"; `doc_hash` equal before and after | yes: template applied |
| `design_words_do_not_apply_a_template` | send "make a card"; hash unchanged | yes |
| `missing_cli_shows_the_start_failure_surface_and_no_executed_text` | runtime Claude with an empty `PATH`: tab has `start_failure` `NotInstalled`; no turn text contains `Executed` | yes |
| `no_planning_path_reaches_the_transcript` | send "audit prd"; no turn text contains `plans/specs` | yes |
| `make_design_sends_a_prompt_and_applies_nothing` | click the action; the stub receives the prompt; hash unchanged | yes |
| `composer_has_no_microphone_control` | no `agents-composer-mic` rect | yes |
| gate `Z15` and its self-test mutations | pass on the fixed tree; fail when either regression returns | yes (no `Z15`) |
| existing tests `:4425-4440` and `:4564` | rewritten to assert the absence | n/a |

Run: `cargo test -p designos-app terminal`, `python3 scripts/zero_fake_ui_gate.py`, `python3 scripts/zero_fake_ui_gate_selftest.py`.

## 11. Performance budgets

Removal only. No new work per frame; no timer; `to_json` is not called on the UI thread.

## 12. Dependencies and build order

First in the part-4 queue and before part 3's FU-673 on `terminal.rs` (the regions barely overlap, but #681 rewrites `submit_chat_prompt`, which #651 and #689 also touch). #688 and #689 rebase on it.

## 13. Owner rulings

- A prompt changes the canvas only through the agent's own MCP writes; the palette keeps explicitly labelled template commands.
- The mic records speech to text or is removed; "Make Design" sends a prompt; no planning paths in user text; the gate gains the check. (Issue text, ruled for v0.6.)
- Zero Fake UI (P6) and Terminal-First (P7) apply.

## Open questions for the owner (recommended first)

1. Wording of the Make design prompt. Recommended: "Design a first draft for the selected frame, or for this page if nothing is selected, using this file's variables and styles."
2. Palette labels. Recommended: rename "Make an image" to "Insert example card (template)" and fix the image-search tooltip, in this PR, because both claim more than they do.
3. The `PRD` tag chip in the composer. Recommended: leave it; it adds text only. Remove it when #655 adds real @-mentions.
