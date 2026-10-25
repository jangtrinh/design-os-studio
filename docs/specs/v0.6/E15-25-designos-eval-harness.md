# E15-25 Build the designos-eval harness: runner, tasks, 3 runs, confidence intervals, held-out set, JSONL

- Issue: #613 (task E15-25, epic #585, L / P0)
- Spec base: `main` at `767acf9`.
- Files: `crates/designos-eval/` (new crate: `Cargo.toml`, `src/{main.rs,lib.rs,cli.rs,task.rs,split.rs,runner.rs,agent.rs,sandbox.rs,transcript.rs,stats.rs,rng.rs,hash.rs,jsonl.rs,config.rs}`, `tests/`), workspace `Cargo.toml` (member), `crates/designos-mcp/src/server.rs` (transcript logging switch).

## 1. Summary and user story

As the team, we want to measure how well agents design with design:os, offline and reproducibly, on tasks we own, through the user's own agent CLI, so changes to tools, skills and prompts are judged by numbers instead of impressions.

## 2. Current state

- FACT: workspace members are the 11 crates listed in `Cargo.toml:3-15`; there is no eval crate.
- FACT: `designos-mcp` is a stdio JSON-RPC MCP server over `rmcp =3.5.1`; its dev-dependencies already include the `rmcp` client with `transport-child-process` (`crates/designos-mcp/Cargo.toml:19,23`).
- FACT: the server confines file access to a workspace root (`server.rs:319-328`) and supports an isolated session directory (`server.rs:80-94`, `for_session_dir`).
- FACT: the CPU renderer is the bit-exact reference (`designos-render`, CLAUDE.md §2 and §4.3); `render_node(doc, node, scale)` exists (`crates/designos-render/src/lib.rs:162`).
- FACT: the Studio agent terminal knows these CLIs: Claude Code, Codex CLI, Antigravity, Grok Build, shell (`crates/designos-app/src/terminal.rs:62-101`).
- FACT: no `rand` or SHA crate is in `Cargo.lock` (checked on `767acf9`).

## 3. Behaviour rules

1. **Task.** One TOML table per task in `suites/<suite>/tasks.toml`:
   - `id` (`s2-0007-shadcn`), `suite` (`s1`…`s5`), `source` (register id, E15-23), `surface` (E15-21 id or `none`);
   - `brief` (text) and/or `inputs` (files relative to the suite folder: reference PNG, start document);
   - `start` (`blank` | path to a start document | `gen:<profile>:<seed>` for E15-24 documents);
   - `design_system` (`none` | library document path);
   - `checklist` (list of yes/no items for the judge, E15-31);
   - `scope` (for edit tasks: node ids or frame names the agent may change; used by E15-29);
   - `budget` (`max_tool_calls` default 400, `timeout_s` default 900).
2. **Held-out split.** For each suite, `split.lock` lists every task id with its split and the SHA-1 of the task's canonical TOML (keys sorted) plus the SHA-1 of each input file.
   - Assignment: `held_out = (u32::from_be_bytes(sha1(salt ‖ id)[0..4]) as f64 / 2^32) < 0.30`; `salt` is a fixed string per suite stored in `split.lock`. The share is checked: every suite's held-out fraction must be within 0.30 ± 0.05, otherwise the lock generation fails and asks for a new salt.
   - `designos-eval run --mode tuning` refuses held-out tasks with `held_out_refused` (AC2). `--mode release` runs everything.
   - Any held-out task whose hash differs from `split.lock` makes every run fail with `split_lock_mismatch`; regenerating the lock is a reviewed commit (`designos-eval lock --suite s2 --write`).
3. **Runs.** `--runs 3` by default (minimum 1, maximum 10). Each run gets a fresh sandbox:
   - a new temp workspace with copies of the start document and inputs;
   - a `designos-mcp` process started with `--workspace <dir>` (`crates/designos-mcp/src/main.rs:7`) and env `DESIGNOS_MCP_TRANSCRIPT=<dir>/transcript.jsonl`. The harness removes `DESIGNOS_SESSION_DIR`, `DESIGNOS_LIVE_*` and `DESIGNOS_CONTEXT_JSON` from the child environment (`server.rs:36-50`), so a run can never write to an open Studio's live files;
   - the agent CLI command from the selected profile (§3.5). The prompt is a fixed harness preamble followed by the brief. The preamble says: open `start.designos` with `designos_open_file` (when the task has a start document), build only inside the requested output frame(s), and save the result to `output.designos` with `designos_save_file`. The preamble text is versioned in `src/preamble.md` and its SHA-1 is written to the `meta` line.
4. **Transcript.** `designos-mcp`, when `DESIGNOS_MCP_TRANSCRIPT` is set, appends one line per `tools/call`: `{ "seq", "t_ms", "tool", "args", "result" | "error", "is_error", "undo_depth" }`. Logging failure fails the call (nothing vanishes silently). Without the variable, nothing is logged.
5. **Agent profiles** (`designos-eval.toml`, user config at the platform config dir, overridable with `--config`):
   ```toml
   [agent.claude]
   command = ["claude", "-p", "{prompt}", "--mcp-config", "{mcp_config}", "--output-format", "stream-json", "--model", "{model}"]
   model = "claude-sonnet-5"
   usage = "claude-stream-json"
   [agent.codex]
   command = ["codex", "exec", "--json", "--model", "{model}", "-c", "mcp_servers.designos.command=\"{mcp_bin}\"", "-c", "mcp_servers.designos.args=[\"--workspace\",\"{workspace}\"]", "{prompt}"]
   model = "gpt-5.6-terra"
   usage = "codex-json"
   [agent.custom]
   command = ["my-agent", "--prompt-file", "{prompt_file}", "--mcp", "{mcp_config}"]
   model = "local-name"
   usage = "none"
   ```
   Placeholders: `{prompt}`, `{prompt_file}`, `{mcp_config}` (a JSON file the harness writes), `{mcp_bin}`, `{workspace}`, `{model}`, `{output_dir}`. Unknown placeholders are an error at load. `usage` selects a parser that extracts token counts from the CLI's output; `none` records tokens as `null`.
6. **No bundled model (R4.13).** The harness runs only the configured command. With no profile it fails with `agent_not_configured`. The stub agent (§3.10) is a scripted MCP client used by tests; it calls no model.
7. **Per-run outputs.** After the agent exits (or on timeout, killed with SIGTERM then SIGKILL after 5 s):
   - the final document is loaded from the session (the agent is told to save to `output.designos`; if missing, the harness saves the live session through the same MCP server before stopping it);
   - output frames: frames created or changed during the run (from the transcript) or the task's declared `output` frame names;
   - each output frame is rendered on the CPU path at scale 1 (PNG saved to `{output_dir}`);
   - deterministic metrics (E15-26) and suite metrics run on the document and renders.
8. **Cost metrics** per run: `tool_calls`, `tool_errors`, `error_rate = tool_errors / tool_calls`, `retries` (a call within the next 3 calls after an error to the same tool whose args differ from the failed call in at most 2 top-level keys), `retry_rate = retries / tool_calls`, `undo_groups` (final undo depth − initial), `wall_ms`, `tokens_in`, `tokens_out` (nullable), `exit` (`ok | timeout | crashed | budget`).
9. **Statistics.** For each metric `m` and suite:
   - per task: `x̄_t` = mean over its runs;
   - suite mean: `M = mean_t(x̄_t)`;
   - 95% CI: two-stage nonparametric bootstrap with `B = 10_000` resamples. Each resample draws tasks with replacement, then, within each drawn task, draws its runs with replacement and averages them; the statistic is the mean over drawn tasks. CI = the 2.5th and 97.5th percentiles (type 7 interpolation);
   - RNG: xoshiro256** seeded by SplitMix64 from `sha1(release ‖ suite ‖ metric)` first 8 bytes, so CIs are reproducible bit for bit;
   - with 1 run per task the second stage is skipped and the record says `ci_method: "bootstrap-tasks"`.
10. **Stub agent.** `designos-eval stub-agent --script <file.jsonl>` connects to the MCP server given in `{mcp_config}` and replays `{tool, args}` lines, writing nothing else. Tests use it as the agent command.
11. **Replay.** `designos-eval replay --transcript <file>` re-executes the recorded tool calls in order against a fresh sandbox with the current engine and recomputes deterministic metrics. A recorded result that referenced ids which now differ is followed through the E15-4 name-to-id map when present; when a later call fails because an id no longer exists, the replay stops with `replay_diverged` at that `seq` (AC3 requires identical metrics when the engine is unchanged).
12. **Source gate.** Before a run, the task's `source` must be `approved` in the E15-23 register; otherwise `source_not_approved`.

## 4. Data model / schema

JSONL (`results/<release>/<suite>.jsonl`), schema version 1, one object per line, `type` discriminates:

```json
{"type":"meta","schema":1,"release":"v0.6.0","git":"<sha>","preamble_sha1":"<sha1>","started":"2026-11-02T10:00:00Z","agent":"claude","model":"claude-sonnet-5","judge_model":null,"mode":"release","runs":3,"host":{"os":"macos","arch":"aarch64"}}
{"type":"run","task":"s2-0007-shadcn","split":"dev","run":1,"exit":"ok","wall_ms":184233,"tool_calls":37,"tool_errors":2,"error_rate":0.054,"retries":1,"retry_rate":0.027,"undo_groups":12,"tokens_in":120443,"tokens_out":9120,"metrics":{"render_success":1.0,"token_coverage":0.82,"instance_ratio":0.41},"artefacts":{"doc":"s2-0007-shadcn/1/output.designos","renders":["s2-0007-shadcn/1/frame-1.png"],"transcript":"s2-0007-shadcn/1/transcript.jsonl"}}
{"type":"task","task":"s2-0007-shadcn","split":"dev","runs":3,"mean":{"token_coverage":0.80},"sd":{"token_coverage":0.03}}
{"type":"suite","suite":"s2","split":"all","n_tasks":100,"metric":"token_coverage","mean":0.78,"ci95":[0.74,0.81],"ci_method":"bootstrap-two-stage","B":10000}
```

Rules: numbers are finite (NaN and infinity are written as `null` with a `notes` entry); metric keys are snake_case; unknown keys are allowed for forward compatibility; a JSON Schema file `crates/designos-eval/schema/results-v1.json` is the contract and is validated in tests.

Rust: `Task`, `SplitLock`, `AgentProfile`, `RunRecord`, `TaskSummary`, `SuiteSummary`, `Meta` with `serde`; `stats::bootstrap_ci(per_task_runs: &[Vec<f64>], b: u32, seed: u64) -> (f64, f64, f64)`.

## 5. Contracts

CLI `designos-eval`:

| Command | Purpose |
|---|---|
| `run --suite s1,s2 --agent claude [--model m] [--runs 3] [--mode tuning|release] [--tasks id,…] [--out results/] [--jobs 2]` | live runs through the agent CLI |
| `replay --suite s3 [--transcripts dir] [--out dir]` | re-run recorded transcripts on the current engine |
| `stats --in results/v0.6.0/` | recompute task and suite summaries |
| `lock --suite s2 [--write]` | check or regenerate `split.lock` |
| `stub-agent --script f.jsonl` | scripted MCP client (tests) |
| `gen …`, `judge …`, `gate …`, `vote-sheet …`, `vote-import …` | E15-24, E15-31, E15-33, E15-32 |

Exit codes: 0 success, 1 task failures recorded (results still written), 2 usage or config error, 3 refused (held-out in tuning, unapproved source, split-lock mismatch). Errors print `<name>: <what to do>`, for example `held_out_refused: task s2-0011-carbon is held out; run with --mode release or remove it from --tasks`.

`--jobs` runs tasks in parallel (default 1), each with its own sandbox and MCP process.

## 6. File-format mapping

Reads and writes `.designos` through `designos-doc`. No format change.

## 7. UI

None (CLI only).

## 8. Edge cases

1. Agent CLI not on PATH: `agent_not_found` naming the binary, exit 2.
2. Agent exits 0 without touching the document: run recorded with `render_success = 0` for missing outputs, not skipped.
3. The agent writes outside the workspace: impossible through MCP (`server.rs:319-328`); direct file writes by the agent CLI are not prevented by the harness and are out of scope.
4. Timeout mid-write: the session state at kill time is evaluated; `exit = "timeout"`.
5. Non-deterministic agent output across runs: expected; CIs capture it. Determinism is required only for metrics on a fixed document (AC3).
6. Disk full while saving artefacts: the run is recorded as `crashed` with the I/O error; nothing else silently drops.
7. Task with zero applicable checklist items: judge metrics are `null` for that task and excluded from the judge mean (counted in `notes`).

## 9. Non-goals

- Metric definitions (E15-26, E15-27, E15-29, E15-30), judge (E15-31), CI gate (E15-33), suites' content.
- Sandboxing the agent CLI's own network or file access.

## 10. Test plan

Seam: `cargo test -p designos-eval` with the stub agent as the configured command.

Fail-before: the crate does not exist; the workspace test count rises by the new tests.

| Test | Input | Expected |
|---|---|---|
| `stub_run_writes_valid_jsonl` (AC1) | 3 toy tasks, stub scripts that create a frame each, `--runs 2` | one meta, 6 run, 3 task, ≥ 1 suite line; every line validates against `results-v1.json` |
| `held_out_refused_in_tuning` (AC2) | `--mode tuning --tasks <held-out id>` | exit 3, `held_out_refused` |
| `replay_reproduces_metrics` (AC3) | record a stub run, replay it twice | identical deterministic metric values (bit-equal f64) |
| `split_fraction_within_bounds` | lock for 100 synthetic ids | held-out count in 25–35 |
| `split_lock_mismatch_refused` | edit a held-out task's brief | exit 3, `split_lock_mismatch` |
| `bootstrap_is_reproducible` | fixed per-task runs, seed | identical CI bounds across two calls |
| `bootstrap_known_case` | 4 tasks, each runs `[1,1,1]` | CI `[1.0, 1.0]` |
| `transcript_logging_failure_fails_call` | transcript path in a read-only dir | tool call returns an error |
| `unapproved_source_refused` | task with source in `pending` | exit 3, `source_not_approved` |

Visual evidence: terminal capture of a stub run, the JSONL head, and a replay match.

## 11. Performance budgets

- Harness overhead per run (sandbox, MCP start, final render and metrics, excluding agent time) ≤ 3 s for documents under 10k nodes.
- Bootstrap with B = 10,000 over 100 tasks × 3 runs ≤ 200 ms.
- The harness never touches Studio; CLAUDE.md §4 invariants are unaffected. Renders use the CPU path only.

## 12. Dependencies and build order

- Needs E15-23 (register).
- Unblocks E15-24, E15-26 … E15-33. Uses E15-4 (#589) name-to-id maps for replay when present.

## 13. Owner rulings that apply

- R4.13: every AI call goes through the user's agent CLI; nothing is bundled.
- The eval never uses client files (source gate plus the E15-23 ban check).
- Full scope: 3 runs, CIs, held-out set and JSONL from day one.
