# #678 Agent tabs: show each CLI tab's token and cost usage

| Field | Value |
|---|---|
| Issue | #678 (owner request 2026-10-18, ruled for v0.6 on 2026-10-19) |
| Size / priority | M / P2 |
| Depends on | #653 (session ids and the registry; Grok usage is read by session id) |
| Unblocks | none |
| Touches | new `crates/designos-app/src/agent_usage.rs` (sources, readers, dedupe), new `docs/agent-cli-usage-sources.md` (the per-CLI source document the issue requires), `crates/designos-app/src/terminal.rs` (hot spot: tab header and tooltip, hook files `:2194-2233`, `poll_live_agent_bridge`), tests |

Code baseline: `origin/main@01d48f2` (0.5.0). Sources: Claude from its statusline and hooks pages (code.claude.com/docs/en/statusline and `/hooks`,
2026-10-19) and a local transcript; Grok from its bundled user guide (`~/.grok/docs/user-guide/17-sessions.md`, `25-status-line.md`, grok 1.0.62) and
`grok usage`; Codex from a web search (no official page found) and local rollout files; Antigravity from `agy --help`.

## 1. Summary and user story

As a designer running agents, I see how many tokens a tab's session has used and, where the CLI reports one, what it cost, taken only from numbers the
CLI really exposes. A CLI that exposes nothing shows nothing, not a zero.

## 2. Current state (FACT)

- Studio shows no usage anywhere in the agent panel. The tab header shows the title, a status dot and the provider (`paint_orca_tab_status_and_provider`,
  `terminal.rs:1983-2058`).
- Studio's hooks write only `{"state": ...}` to `agent-status.json` and ignore the hook's stdin (`status_hook_command`, `terminal.rs:2194-2196`).
- Per-CLI sources, as checked on 2026-10-19 on the dev machine (versions in `plans/reports/release-060-evidence/cli-help-261019/versions.txt`):

| CLI | Source | What it gives | Limits | Status |
|---|---|---|---|---|
| Claude Code 2.1.302 | **Transcript** `transcript_path` (a common field of every hook's stdin, documented; "written asynchronously and may lag") | per assistant message `usage`: `input_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens`, `output_tokens` (observed in a local transcript) | the same `message.id` appears several times: in one local transcript 1,960 usage lines carried 982 distinct ids, up to 6 repeats, so a plain sum would roughly double the count | tokens: observed, not documented as a stable format; dedupe required |
| Claude Code | **statusLine** command (documented setting; JSON on stdin) | `cost.total_cost_usd` ("estimated session cost in USD, computed client-side at list price ... may differ from your actual bill"), `context_window.total_input_tokens` / `total_output_tokens` (what is **in the window now**, from the latest response, not a session total), `rate_limits.five_hour` / `seven_day` (subscribers only, may be absent) | one `statusLine` per settings scope: adding Studio's would replace the user's own in that tab (precedence between `--settings` and user settings not read) | documented; cost is an estimate |
| Grok 1.0.62 | `grok usage <session-id> [turn]` (documented, "persisted token and cost usage") | JSON: `session` with `inputTokens`, `outputTokens`, `cachedReadTokens`, `cacheCreationTokens`, `reasoningTokens`, `totalTokens`, `modelCalls`, `costUsdTicks`, `turnCount`, `primaryModelId`, and per-turn `turns` | `costUsdTicks` is 10^10 ticks per USD (user guide); the cost is a list-price reference, not a bill (owner, 2026-10-12: Grok runs on subscription compute); needs the session id (#653 assigns it with `--session-id`) | documented; ran on a real session, output matched |
| Codex 0.163.0 | rollout file `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`, `token_count` events (`payload.info.total_token_usage`) | cumulative tokens | no official documentation found; community sources disagree on the field nesting and say it can change between versions; present in two local 0.162.1 rollouts, **absent from four short 0.163.0 rollouts** | **unverified** for the installed version |
| Antigravity 1.4.2 | none found: `agy --help` lists no usage command or flag | nothing | `~/.gemini/antigravity-cli` holds conversation databases whose format was not inspected | **no source** |

- Tab session ids come from #653 (Studio-chosen UUIDs for Claude and Grok).

## 3. Behaviour rules

1. **Show only what a source proves.** A tab shows usage only from a source row above that is marked usable (rule 5). No estimate is computed from
   text length, prompt size or time. A CLI with no usable source shows no usage field at all: not `0`, not `-`.
2. **Tokens.** The readout is `in` (input incl. cached reads and cache writes where the source reports them) and `out` (output, including reasoning
   tokens where the source adds them separately), cumulative for the session, in compact units (`12.4k`). The label says what the source calls them.
3. **Cost.** Shown only where the source reports a cost, and always labelled by what it is: `est.` for a client-side list-price estimate (Claude
   statusLine) and `ref.` for a list-price reference (Grok), never a bare `$`. The labelling form is open question 2.
4. **Claude tokens from the transcript.** The Claude readout sums the transcript's `usage` after deduplicating by `message.id`, keeping the last
   usage seen for each id. The hook command writes `session_id` and `transcript_path` (from its stdin JSON) once per session into
   `<session folder>/claude-transcript.json`; Studio then tails the file incrementally by offset (like `LivePatchLog`), off the UI thread. If the
   transcript is missing or unreadable the field is hidden.
5. **Which sources are usable in this task:** Claude transcript tokens (rule 4); Claude cost only if the owner chooses the statusLine option;
   Grok `grok usage` (run once when the tab's turn ends, when the tab is focused, and when the panel opens; each run is one short subprocess off the
   UI thread, with a 5 s timeout); Codex and Antigravity: no field until their rows are verified (Codex needs a live check on the installed version;
   agy has no source). The per-CLI document records that decision.
6. **Document.** `docs/agent-cli-usage-sources.md` records, per CLI, the source, the exact fields read, the version checked, the date, and "no source" where
   that is the truth. It is updated whenever a source row changes.
7. **Failure is silent but visible in the document, not in the UI**: a failed Grok call or an unreadable transcript hides the field; the tooltip says
   "Usage not available".
8. Usage never goes into the agent's prompt, the document, or any log line that leaves the machine.

## 4. Data model / schema changes

- `UsageReading { input: u64, output: u64, cost: Option<CostLabel>, source: UsageSource, as_of_ms }`, `CostLabel::{Estimate(f64), Reference(f64)}`,
  `UsageSource::{ClaudeTranscript, ClaudeStatusLine, GrokUsage}` (in memory, per tab).
- A small cache file `usage.json` in the session folder (#653) may keep the last reading so a resumed tab shows the previous number at once; optional.

## 5. Contracts

- The Claude hook entries gain one capture command (reads stdin once per session). No change to the status writes.
- `grok usage` is read-only and local.
- No MCP change.

## 6. File-format mapping

None.

## 7. UI

Described in words. Checked against `design/principles.md` (P1 to P10) and `design/soul.md`; the rules used, with the `theme.rs` constants, are recapped in README-part3 under "UI rules checked for part 3". In short: every colour, gap, inset, control height and radius below is a `crates/designos-app/src/theme.rs` constant (the panel's own `TERM_*` / `STATUS_*` constants at `terminal.rs:29-55` stay for the terminal viewport and the existing tab status dots and are not extended); selected and active rows use the neutral fill `BG_SELECTED`; the brand orange is the view's one primary action; counts sit in their own pill, not in a label.

The issue fixes: usage per tab, from real sources, nothing when a CLI exposes nothing. Placement and form are proposals for the owner's confirmation:

- A caption in the tab's tooltip and at the right end of the panel's header strip: the token counts and, where reported, the cost label, all numbers in `font_mono` (counters, P5), `TEXT_SECONDARY`. Nothing is drawn for a tab without a reading.
- The tooltip lists the source ("From Claude's transcript", "From grok usage"), the time of the reading, and for a cost the words "list-price reference, not a bill".
- No animation, no timers (idle CPU invariant); the reading updates on events (turn end, focus), not per frame.
- Desktop captures at 1440: Claude tab with tokens, Grok tab with tokens and a `ref.` cost, Codex or agy tab with nothing. Responsive SKIPPED (owner default). Load es-designer before UI code; update the Figma frames from the captures first.

## 8. Edge cases

1. A resumed Claude session: the transcript already holds earlier turns; the readout covers the whole session, as the user would expect for the session.
2. `/clear` in Claude starts a new session id (the statusLine page says the cost resets); the hook capture runs again and the readout restarts.
3. Grok `costUsdTicks` absent for a model without a price: no cost shown, tokens still shown.
4. Two tabs on the same session: each reads the same source; numbers equal.
5. The transcript file rotates or is rewritten: the reader resets its offset like `LivePatchLog` does for a rewritten log.
6. The deduplication keeps the last usage per `message.id`; a message whose usage grows across its repeats counts once at its final value.
7. A CLI update changes a format: the parser rejects unknown shapes and hides the field; the document row is re-verified.
8. Very large transcripts (tens of MB): incremental tail, never a full re-read per poll.

## 9. Non-goals

- Estimating usage for CLIs without a source.
- Billing, quotas or rate-limit warnings (Claude's `rate_limits` are not shown here).
- Aggregating usage across tabs or files.
- Reading conversation content.

## 10. Test plan (red first)

Seams: parser unit tests on synthetic fixtures written from each documented shape (no real transcripts are committed); a stub `grok` script that prints the
documented JSON; a `Sim` test for the readout.

| Case | Expected (fails before) |
|---|---|
| `claude_transcript_tokens_are_deduplicated_by_message_id` | a synthetic transcript with the same id three times and growing usage: counted once, at the last value |
| `claude_transcript_missing_hides_the_field` | no file: no reading |
| `grok_usage_json_parses_tokens_and_cost` | the documented shape: tokens summed as the guide defines; `costUsdTicks / 1e10` dollars, labelled `ref.` |
| `grok_without_a_cost_shows_tokens_only` | `costUsdTicks` absent |
| `codex_and_agy_show_nothing` | the readout is `None` and no text is drawn (not `0`) |
| `hook_capture_writes_session_and_transcript_path_once` | run the hook command with a documented stdin JSON: file written once |
| `reader_tails_incrementally` | append lines: only new bytes read |
| `grok_call_runs_off_the_ui_thread_with_a_timeout` | a stub that sleeps past 5 s: the readout hides, the first frame is not blocked |
| `no_estimates` | a test that fails if any reading's `source` is not one of the three documented sources |
| `sources_document_matches_code` | every `UsageSource` variant has a row in `docs/agent-cli-usage-sources.md` (string check) |
| `claude_live_check` (owner-manual) | in a real Claude tab, the readout's `in` + `out` after a prompt equals the sum of that turn's usage in the transcript (screenshot of both) |
| `grok_live_check` (owner-manual) | the readout equals `grok usage <id>` run by hand |

Run: `cargo test -p designos-app agent_usage`. Evidence: red/green capture, the three 1440 captures, the two live checks, and the per-CLI document in the PR.

## 11. Performance budgets

- Transcript tail: reads only new bytes; under 2 ms per poll for 1 MB of new data; polled on turn events and every 5 s while the tab is working.
- `grok usage`: one subprocess (tens of ms) per trigger, never on the UI thread.
- Idle: no timers when no tab is working; no per-frame reads.

## 12. Dependencies and build order

After #653. `terminal.rs` is a hot spot: readers and parsers live in the new file; `terminal.rs` gets the header caption call and the hook capture entry.
Coordinate with #677 (it also adds hook entries to the same files; the two edits are separate lines).

## 13. Owner rulings

- Each agent tab shows the session's usage (tokens, and cost where the CLI reports it) from a source the CLI actually exposes (hook payloads, status lines
  or its own usage files). No estimates presented as real numbers. A CLI that exposes nothing shows nothing, not a zero. The source per CLI is documented and
  checked against each CLI's current docs. (Issue text.)
- Codex keeps the heuristic status; skip-permissions stays the default.

## Open questions for the owner

1. **Claude's cost.** Claude reports a client-side list-price estimate only through its statusLine setting, which replaces the user's own status line in that tab.
   - A (recommended): show Claude tokens from the transcript only, no cost. It touches nothing of the user's setup and every number shown is a real count.
   - B: also install a statusLine for the cost, labelled `est.`, and accept that the user's own status line does not show in Studio tabs (or find a way to chain it).
2. **How a list-price cost is labelled.** The issue forbids estimates presented as real numbers.
   - A (recommended): `$0.42 ref.` with a tooltip "list-price reference, not a bill".
   - B: do not show any cost, tokens only.
   - C: `$0.42 (list price)` written out in the readout.
3. **Placement.**
   - A (recommended): in the tab tooltip and at the right end of the header strip.
   - B: tooltip only (quieter, but hidden until hover).
   - C: in the tab itself next to the status dot (always visible, uses tab width).
4. **Codex until its source is verified.**
   - A (recommended): show nothing for Codex now and add it when a live check on the installed version proves the field, recorded in the source document.
   - B: read the rollout `token_count` today for versions where it is present, and show nothing when it is absent (it is undocumented and may change).
