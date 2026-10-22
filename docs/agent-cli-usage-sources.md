# Agent CLI Token & Cost Usage Sources (`#678`)

This document records the authoritative token and cost usage sources checked for each embedded agent CLI runtime in DESIGN:OS Studio, the exact fields read by [`crates/designos-app/src/agent_usage.rs`](../crates/designos-app/src/agent_usage.rs), the CLI version verified, and the date of verification.

Studio enforces a strict **show-only-what-a-source-proves** policy:
- No token or cost number is ever estimated from text length, prompt size, or elapsed time.
- A CLI with `"no source"` or `"unverified"` status renders nothing in the header strip (not `0`, not `-`).
- When a CLI has a usable source (`ClaudeTranscript`, `ClaudeStatusLine`, or `GrokUsage`) but its transcript or subprocess output is missing or unreadable, the header field is hidden and the tab tooltip says `"Usage not available"`.

## Per-CLI Source Matrix

| CLI | Version Checked | Date Checked | `UsageSource` Variant | Source Mechanism | Exact Fields Read | Cost Label & Limits | Status |
|---|---|---|---|---|---|---|---|
| **Claude Code** (`claude`) | `2.1.302` | `2026-10-19` | `ClaudeTranscript` | Hook stdin JSON (`session_id`, `transcript_path`) captured once per session to `<session_dir>/claude-transcript.json`; Studio tails `transcript_path` incrementally by byte offset off the UI thread | Hook stdin: `session_id`, `transcript_path`. Transcript JSONL: `message.id` (deduplication key, keeping last entry per `message.id`), `message.usage.input_tokens`, `message.usage.cache_creation_input_tokens`, `message.usage.cache_read_input_tokens`, `message.usage.output_tokens` | Tokens only (`in = sum(input_tokens + cache_creation_input_tokens + cache_read_input_tokens)`, `out = sum(output_tokens)`). No cost in transcript. | **usable** (default for Claude tabs) |
| **Claude Code** (`claude`) | `2.1.302` | `2026-10-19` | `ClaudeStatusLine` | Documented `statusLine` command JSON on stdin (`<session_dir>/claude-statusline.json` when configured) | `cost.total_cost_usd`, `context_window.total_input_tokens`, `context_window.total_output_tokens` | `CostLabel::Estimate` (`$X.XX est.`, tooltip: `"estimated session cost in USD, computed client-side at list price"`). Not installed by default so Studio never overwrites the user's own `statusLine`. | **documented** (optional; `ClaudeTranscript` used by default so user's `statusLine` is untouched) |
| **Grok Build** (`grok`) | `1.0.62` | `2026-10-19` | `GrokUsage` | Subprocess `grok usage <session-id>` run off the UI thread with a 5 s timeout on turn end, tab focus, and panel open | `session.inputTokens`, `session.outputTokens`, `session.cachedReadTokens`, `session.cacheCreationTokens`, `session.reasoningTokens`, `session.totalTokens`, `session.costUsdTicks` | `CostLabel::Reference` (`costUsdTicks / 1e10` USD, rendered as `$X.XX ref.`, tooltip: `"list-price reference, not a bill"`). When `costUsdTicks` is absent/null, only token counts are shown. | **usable** (default for Grok tabs) |
| **Codex CLI** (`codex`) | `0.163.0` | `2026-10-19` | none (`no source`) | Local rollout files `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl` (`token_count` events, `payload.info.total_token_usage`) | none read | No official documentation; `token_count` events were present in two local `0.162.1` rollouts but absent from four short `0.163.0` rollouts. Studio shows nothing until verified on the installed version. | **unverified / no source** |
| **Antigravity CLI** (`agy`) | `1.4.2` | `2026-10-19` | none (`no source`) | `agy --help` lists no usage subcommand, flag, or hook usage payload | none read | No source exposed by `agy` `1.4.2`. Studio shows nothing. | **no source** |
| **Shell** (`$SHELL`) | N/A | `2026-10-19` | none (`no source`) | Plain interactive login shell | none read | Not an agent CLI; Studio shows nothing. | **no source** |

## Details by Source

### 1. `ClaudeTranscript` (`Claude Code 2.1.302`)
- **Hook capture**: Every Claude Code hook event receives a JSON object on stdin containing `session_id` and `transcript_path`. Studio's session hook writes `{"session_id":"...","transcript_path":"..."}` to `<session_dir>/claude-transcript.json` once per `session_id` (and updates it if `/clear` rotates `session_id`).
- **Deduplication**: A single assistant message (`message.id`) can appear on multiple JSONL lines as tool use and streaming progress are flushed. `ClaudeTranscriptReader` deduplicates by `message.id`, keeping the **last** `usage` block seen for each `message.id`.
- **Incremental tailing**: `ClaudeTranscriptReader` tracks the file identity (inode/creation time), leading bytes, and byte offset so each poll reads only newly appended lines, resetting from offset `0` if the transcript file is truncated or rotated.

### 2. `ClaudeStatusLine` (`Claude Code 2.1.302`)
- Documented at `code.claude.com/docs/en/statusline`.
- Provides `cost.total_cost_usd` (client-side list-price estimate, labelled `est.`) and `context_window.total_input_tokens` / `total_output_tokens`.
- Because Claude Code allows only one `statusLine` per settings scope, Studio does not inject a `statusLine` override into `hooks.json` by default, preserving the user's own status line.

### 3. `GrokUsage` (`Grok 1.0.62`)
- Documented in `~/.grok/docs/user-guide/17-sessions.md` (`## The grok usage Subcommand`).
- Requires the session UUID passed via `grok --session-id <uuid>` (or `-r <uuid>` on resume).
- `session.costUsdTicks` uses `10^10` ticks per USD (`costUsdTicks as f64 / 1e10`). Because Grok Build runs on subscription compute, this figure is always labelled `ref.` (`"list-price reference, not a bill"`).
- If `costUsdTicks` is omitted or `null` (for models without a list price), Studio shows the token counts and omits the cost segment.
