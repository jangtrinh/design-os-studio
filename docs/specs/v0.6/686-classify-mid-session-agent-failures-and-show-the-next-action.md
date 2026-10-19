# #686 Agent tabs: classify mid-session failures (rate limit, login expired, billing, server error, context full) and show the next action

| Field | Value |
|---|---|
| Issue | #686 (parent #585) |
| Size | M |
| Depends on | #650 (start-failure surface, closed, on `main`), #673 (start-failure classes from output); soft: #683 (event stream, error status) |
| Unblocks | #687 (announces "failed: <class>") |
| Touches | new `crates/designos-app/src/agent_session_failure.rs` (classify, banner state, details), new `crates/designos-app/src/agent_failure_redact.rs` (redaction), new data file `crates/designos-app/src/agent_failure_patterns.toml`, `crates/designos-app/src/agent_start_failure.rs` (reuse of `FixAction`, `output_excerpt`), `crates/designos-app/src/terminal.rs` (hot spot: viewport `:3614`, polling `:1340-1352`), tests with captured-output fixtures |

Code baseline: `origin/main@9f3f7a5`.

## 1. Summary and user story

As a designer, when my agent stops in the middle of a session because of a limit, an expired login, billing, a provider outage, a full context or no network, I want one plain sentence and one button to fix it, not terminal text to decode.

## 2. Current state (FACT)

- #650 shipped: a CLI that exits within 5 s of spawn shows a panel with cause, last output and one fix (`agent_start_failure.rs`: `START_FAILURE_WINDOW` `:13`, `StartFailureCause` `:19-24`, `FixAction` `:28-35`, `diagnose` `:116-150`, `paint` `:195`). It is shown over the terminal viewport (`terminal.rs:3615-3618`).
- `diagnose` already keeps its needle lists as in-code constants (`NOT_INSTALLED`, `UNKNOWN_FLAG`, `NOT_LOGGED_IN`, `agent_start_failure.rs:117-130`) and has `login_command` and `install_url` per CLI. The start window ends the check: later exits are only noted as "Agent exited" (`terminal.rs:1340-1352`).
- A tab keeps its output in `raw_log: Arc<Mutex<String>>` and `strip_ansi_codes` removes escapes (`terminal.rs:381-382`, `:1480`).
- Hooks: Grok's `StopFailure` event carries `error` (`rate_limit`, `authentication_failed`, `invalid_request`, `server_error`, `max_output_tokens`, `unknown`; capacity errors are `rate_limit`), `errorDetails` (clipped at 1,000 characters) and `lastAssistantMessage` (Grok guide, `10-hooks.md` lines 180, 480). #647 loads Claude and Grok hooks; Studio's current hook writes only a state word (`terminal.rs:2194-2196`). Codex loads no hooks.
- `designos-app` already depends on `ureq` 2 for HTTP (`Cargo.toml:45`). It has no `toml` dependency (`toml` is only in `Cargo.lock` transitively).
- UNKNOWN: Claude Code's `StopFailure` error classes. Check: capture one from a real tab (see #683 "Could not ground").
- UNKNOWN: the real output text of each CLI for each class (rate limit, login expired, billing, outage, context full, offline). No capture exists on `main`. The patterns in this task come only from captured output; none is written from memory.
- UNKNOWN: each provider's public status page address and whether it serves a machine-readable incident list. Check: fetch each once while writing the data file and record the address and format.

## 3. Behaviour rules

1. **When a failure is classified.** (a) A `StopFailure` record (Claude, Grok; needs #683's stream or an equivalent hook line). (b) A non-zero exit after the start window. (c) For Codex, which has no hooks, a pattern match in output that arrived after the last turn start while the tab is not done. A zero exit is never a failure.
2. **Classes**: `RateLimited`, `LoginExpired`, `BillingOrQuota`, `ProviderServerError`, `ContextFull`, `NetworkOffline`. Anything else is `Unknown`.
3. **Inputs, in order.** The hook's `error` class when present; then the CLI's output patterns; then `Unknown`. A hook class `unknown` falls through to the patterns. Patterns live in `agent_failure_patterns.toml`, one table per CLI, each pattern with its class, an optional named capture for a reset time, and the fixture file that proves it. A pattern without a fixture is not accepted.
4. **Banner** above the terminal, one line in plain words and one action:
   - `RateLimited`: "Rate limited." Action "Wait and retry". With a reset time: "Rate limited until 14:05." The action sends the retry text only after the reset; before it the button is disabled with the time.
   - `LoginExpired`: "Login expired." Action "Run login" (the existing `FixAction::RunLogin`, `login_command(preset)`).
   - `BillingOrQuota`: "Billing or quota problem." Action "Open billing" (the provider's billing page address, data per CLI).
   - `ProviderServerError`: "The provider has a problem." Action "Retry". After classification Studio fetches that provider's public status page once with `ureq` off the UI thread and, if an incident is open, appends its title. It never polls.
   - `ContextFull`: "This conversation is full." Action "Start a new session".
   - `NetworkOffline`: "No network." Action "Retry when online": one connection attempt per click, never a poll.
5. **Unknown failure.** The banner shows the last output lines (as #650's panel does) and "Copy details". It names no class and no cause.
6. **Copy details** puts on the clipboard: CLI name and version, the class (or "unknown"), the last 40 lines of output (escapes stripped), and the session id. Every line passes the redactor first. The redactor removes values that match key shapes (provider key prefixes, `Authorization: Bearer ...`, `NAME=value` where the name contains KEY, TOKEN, SECRET or PASSWORD, long base64 or hex runs after such a name). It replaces them with `[redacted]`. Studio never adds a token or key to the text.
7. **Lifetime.** The banner disappears when the next turn starts, when its action completes, or when the tab closes. It does not persist across a restart.
8. **Status.** A classified failure sets the tab's status to `Error(class)` (#683) and emits one status change for #687. Without #683 the banner works alone.
9. Shell tabs never show a banner.

## 4. Data model / schema changes

- `SessionFailure { class, detail_line, reset_at, fix }` per tab in memory. `agent_failure_patterns.toml` (data). No file-format change.

## 5. Contracts

- Pattern file schema: `[[cli.claude.pattern]] class = "rate_limited"; regex = "..."; fixture = "tests/fixtures/agent-failures/claude-rate-limited.txt"`. The same file holds the start-failure needles when #673 moves them; one place for all CLI text patterns.
- `FixAction` gains `OpenBilling(&'static str)`, `RetryText`, `NewSession` and `RetryWhenOnline`. #673 and part 3's #653 also add actions; each adds its own variants only.

## 6. File-format mapping

None.

## 7. UI

Described in words; checked against `design/principles.md` P1 to P10 and `design/soul.md`.

- The banner is a full-width strip at the top of the terminal viewport, `BG_SECONDARY` with a `BORDER` hairline below, inset `INSET_FRAME`. A 8 px state dot at the left (`DANGER` for failures, `WARNING` for rate limits and offline). The sentence is body type, `TEXT`, elided to the width.
- The action is a `CONTROL_STANDARD` (28 px) button at the right, `RADIUS_CONTROL`, neutral fill. It is secondary because the view's one primary action is Send. "Copy details" is a text button of `CONTROL_DENSE` (24 px) height left of it.
- The reset time and any counts use `font_mono`.
- No animation, no timer.
- Desktop capture at 1440 for each of the six classes and for an unknown failure, from stub CLIs that print captured text. Responsive: SKIPPED (owner default). Load es-designer before UI code; update the Figma frames from the captures first.

## 8. Edge cases

1. A rate limit and a server error in one output: the first match in pattern order wins, and the order is the table order in the file.
2. The reset time is in the past by the time the user looks: the action is enabled.
3. The status page is unreachable: no incident title; no error text beyond the banner.
4. Two tabs fail at once: each has its own banner.
5. A key shape inside the output that is part of an error message (a masked key): redacted anyway.
6. A CLI prints the word "login" in normal output: patterns need a context phrase plus the hook or exit trigger of rule 1, so normal output does not raise a banner.
7. A very long line: elided in the banner; kept whole (redacted) in the copied details.
8. The tab is dead: "Start a new session" still works; retry actions are disabled.

## 9. Non-goals

- Retrying the user's prompt automatically. Showing token usage (#678). A global error log. Provider health dashboards.

## 10. Test plan (red first)

Seams: pure classification tests over fixture files; redaction unit tests; a stub CLI that prints a fixture and exits; the `Sim` harness for the banner.

| Case | Expected | Fails before |
|---|---|---|
| `each_class_fixture_shows_its_banner_and_action` (one row per class and per CLI that has a fixture) | class and action match | yes |
| `hook_class_wins_over_output_patterns` | `StopFailure` `rate_limit` with login-like text: `RateLimited` | yes |
| `unknown_error_shows_no_class` | banner has no class, last lines and Copy details only | yes |
| `normal_output_with_the_word_login_raises_no_banner` | none | guard |
| `copied_details_pass_a_redaction_test_with_planted_fake_keys` | planted keys of five shapes absent; `[redacted]` present | yes |
| `copied_details_have_the_four_parts` | name and version, class, 40 lines, session id | yes |
| `status_page_is_fetched_once_and_never_polled` | one request for repeated paints | yes |
| `retry_when_online_makes_one_attempt_per_click` | counter | yes |
| `banner_clears_on_the_next_turn` | none after `UserPromptSubmit` | yes |
| `pattern_without_a_fixture_is_rejected` | loader error | yes |

Run: `cargo test -p designos-app agent_session_failure agent_failure_redact`. Evidence: red/green capture and the 1440 captures. Owner-manual: one real rate-limit or login-expiry per CLI where it can be provoked.

## 11. Performance budgets

- Classification runs when a trigger fires, over the last 200 lines only; under 1 ms. No scan of the whole `raw_log` per frame.
- The status page request and the connectivity attempt run on a worker thread; the UI thread never blocks.
- No polling and no timer; the banner costs nothing when absent.

## 12. Dependencies and build order

After FU-673 (shared patterns and `FixAction`); before or with #683 (the hook stream makes trigger (a) exact). `terminal.rs` gets call sites only. Merge order is in README-part4.

## 13. Owner rulings

- Classify from hook payloads and the CLI's own output, patterns kept as data per CLI; one plain line and one action; one status-page check after a server error and never while idle; Copy details never includes tokens or keys; an unknown failure never gets a made-up cause. (Issue text.)

## Open questions for the owner (recommended first)

1. Which providers' status pages to read. Recommended: only those whose public page serves a machine-readable incident list, verified once; the others show no incident title.
2. "Wait and retry" sends the retry text itself. Recommended: it sends "continue" into the tab's CLI after the reset time, because the CLI's own retry state is unknown; the user can change the text in the composer first.
