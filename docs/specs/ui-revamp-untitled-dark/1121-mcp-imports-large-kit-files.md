# N designos-mcp imports large kit files: one import limit, typed size errors

| Field | Value |
|---|---|
| Issue | #1121 (epic #1118, spec key N) |
| Size | S. About half a day: one shared constant, one typed error, a usage-text fix, five tests. |
| Depends on | none |
| Unblocks | The R3 dogfood loop (kit work through `designos-mcp`, specs A, B and C). The usage error points agents at `--version`, so a stale `designos-mcp` is easy to spot. |
| Touches | `crates/designos-mcp/src/server.rs`:<br>- `designos_open_file` size gate `:844-849`<br>- web-capture gate `:1052-1060`<br>- fixture 44 `:2435-2441`<br>`crates/designos-mcp/src/tools/errors.rs` (`default_next_step` `:295-322`)<br>`crates/designos-mcp/src/main.rs:29` (usage text only)<br>`crates/designos-mcp/tests/session.rs`<br>`crates/designos-mcp/tests/version.rs` (one more case) |

Code baseline: `origin/main@35a65796`, also valid at `205c72ef` (newest `origin/main` on 2026-10-25): between the two, `crates/designos-mcp`, `crates/designos-fig/src` and `designos-app` `main.rs`/`cli.rs` did not change (only a new test file `designos-fig/tests/fig_effect_grid_styles.rs` was added). The first draft cited local `main@ec47da26`, which is 4 commits behind it. Every line cited below was re-anchored to `35a65796` with `git show 35a65796:<path>`, except the scratch runs in section 2, which ran on an `ec47da26` build. Re-anchor against HEAD again and report drift before writing code. Scratch evidence for this spec, outside the repo: `$SCRATCH/agents/w5/n/`. `$SCRATCH` is the controller's scratchpad, `/private/tmp/claude-501/-Users-jangtrinh--claude-profiles-work-spares-spare-AW68Nz/a6445c24-c40e-4254-a3e2-5e0bafafd0f4/scratchpad`.

## 1. Summary and user story

As an agent driving Studio through `designos-mcp`, I open the 336 MB Untitled UI kit with `designos_open_file` and work on it. I get the same typed `FILE_TOO_LARGE` error, with the same limit, whichever import tool I call. If I start the binary wrongly, the usage error tells me about `--version`, so I can check which release I am talking to.

**Diagnosis first.** The dogfood report ("designos-mcp refuses the kit: Input file exceeds the 64 MiB import limit", plan.md "Dogfood gaps") did not come from HEAD:
- The probe client used the stale `/Applications/designos.app` build, version 0.4.0.
- 0.6.0 and later open the kit. The run in section 2 used an `ec47da26` build, which reports 0.6.0.
- The 64 MiB limit on `designos_open_file` was raised to 512 MiB in #602 (CHANGELOG.md:84), by owner ruling Q2 of 2026-10-18 (`docs/specs/v0.6/README.md:180`).

What remains at `35a65796`:
- `designos_import_web_capture` still has the old 64 MiB limit.
- That limit returns an untyped error that arrives as `INVALID_ARGUMENTS`.
- `designos-mcp --version` / `-V` already exists (#845, shipped in 0.6.1; test `crates/designos-mcp/tests/version.rs`). But the usage error that a wrong invocation prints does not mention it (`main.rs:29`), and the stale 0.4.0 build has no such flag.

This spec fixes the size limit and its error, and points the usage error at `--version`. It keeps the owner's 512 MiB limit.

## 2. Current state (FACT unless labelled)

**The limits.**
- `designos_open_file`: `len > 512 * 1024 * 1024` returns `FILE_TOO_LARGE` at `/path`, with `received` (bytes) and `expected` `"<= 536870912 bytes"` (`server.rs:844-849`). After loading, `validate_document` refuses more than 1,200,000 layers (`server.rs:1249-1252`), and that error maps to `FILE_TOO_LARGE` (`server.rs:866-868`).
- `designos_import_web_capture` with a `path`: `len > 64 * 1024 * 1024` returns the plain string `"Input file exceeds the 64 MiB import limit"` (`server.rs:1056-1057`). `ToolError::from_plain_message` turns a message with no code prefix into `INVALID_ARGUMENTS` with path `""` and `safe_to_retry:true` (`errors.rs:591-607`).
- The rules pack already promises `FILE_TOO_LARGE` for this tool (`rules.rs:561`), so the reply contradicts the documented contract.
- Studio itself has no file-size limit. `designos_library::load_external` reads the whole file (`designos-library/src/store.rs:134-139`), and the `.fig` container bounds only inflation: 2 GiB per entry, 4 GiB total, ratio 100 (`designos-fig/src/container.rs:9-17`). Studio's `--import-capture` reads the capture file with no limit (`designos-app/src/main.rs:238`).

**Why the 512 MiB limit exists (FACT, from the record).**
- E15-17 §3.5 (`docs/specs/v0.6/E15-17-streaming-large-document-query.md:78-84`) raised it from 64 MiB "for the same reason" as the layer limit: reads became bounded.
- The owner ruled the numbers on 2026-10-18: "raise them to 1,200,000 layers (`validate_document`) and 512 MiB (imports). The other safety limits, such as deadlines and memory, stay."
- The limit is therefore a memory guard for a headless server, sized by owner ruling. It is not a DoS rule.
- Measured in this session: opening the 336,639,376-byte kit peaked at 9.6–9.9 GB RSS, against 6.5 GB for Studio's `--inspect-json` (plan.md). That is about 29× the file size.
- INFERENCE (linear extrapolation): a `.fig` at the 512 MiB limit would need about 15 GB.

**Version.**
- `serverInfo` carries `CARGO_PKG_VERSION` (`server.rs:1460`).
- At `35a65796`, `main.rs:19-21` prints `designos-mcp <version>` for `--version` or `-V`, and `--help` lists the flag (`:25`). `tests/version.rs` covers both.
- Any other invocation exits 1 with `Usage: designos-mcp --workspace DIRECTORY` (`main.rs:29`), which does not mention `--version`.
- The `--version` run in section 2 failed because it used the older `ec47da26` build; it is not a gap on `origin/main`.

**Error polish.**
- `default_next_step` has no arm for `FileTooLarge`, so the reply tells the agent to use `valid_options` or `corrected_example`, which this error never carries (`errors.rs:320`).
- `safe_to_retry` defaults to `true` (`errors.rs:443`), although retrying the same file fails the same way. Fixture 44 records the code (`server.rs:2435-2441`).

**Reproduction (scratch, 2026-10-25).** Workspace `$SCRATCH/uui-ws` holds the kit copy. The client is `$SCRATCH/tools/mcp_client.py`, which by default runs `/Applications/designos.app/Contents/MacOS/designos-mcp`.

```text
# stale build the dogfood session used
$ python3 tools/mcp_server_info.py /Applications/designos.app/Contents/MacOS/designos-mcp uui-ws
{"name": "designos-mcp", "version": "0.4.0"}
$ python3 tools/mcp_client.py uui-ws calls-open.json out-stale-0.4      # open_file untitled-ui.fig
== 01-open: 54 chars; head: '{"error":"Input file exceeds the 64 MiB import limit"}'

# local main ec47da26 (0.6.0) through tools/designos-mcp-current.sh
$ python3 tools/mcp_server_info.py tools/designos-mcp-current.sh uui-ws
{"name": "designos-mcp", "version": "0.6.0"}
$ DESIGNOS_MCP_BIN=tools/designos-mcp-current.sh /usr/bin/time -l python3 mcp_client_stderr.py uui-ws calls-open.json out-head-run2
== 01-open: 416272 chars; head: '{"active_page_id":1,...,"name":"❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy)","node_count":207601,...
       65.48 real         7.03 user         0.82 sys
          9610690560  maximum resident set size
$ tools/designos-mcp-current.sh --version
Error: "Usage: designos-mcp --workspace DIRECTORY"          # exit 1 on ec47da26; origin/main has --version since #845

# ec47da26: the web-capture limit (sparse files); the same check is at server.rs:1056-1057 on 35a65796
$ python3 mcp_client_stderr.py ws-capture calls-capture.json out-capture
01-capture-65mib  {"code":"INVALID_ARGUMENTS","error":{"code":"INVALID_ARGUMENTS","message":"Input file exceeds the 64 MiB import limit","next_step":"Adjust the value at `path` using `valid_options` or `corrected_example` and retry.","path":"","safe_to_retry":true},...}
02-capture-63mib  {"code":"INVALID_ARGUMENTS",..."message":"Failed to import web capture: invalid capture JSON: expected value at line 1 column 1",...}
03-open-513mib    {"code":"FILE_TOO_LARGE","error":{"code":"FILE_TOO_LARGE","expected":"<= 536870912 bytes","message":"Input file exceeds the 512 MiB import limit","next_step":"Adjust the value at `path` using `valid_options` or `corrected_example` and retry.","path":"/path","received":537919488,"safe_to_retry":true},...}
```

Further notes on the evidence:
- A third `ec47da26` run (`out-head`) also succeeded, in 150 s with a 9.87 GB peak.
- Two earlier `ec47da26` attempts ended with the server closing stdout before it replied, with stderr discarded. UNKNOWN cause. INFERENCE: the wrapper's `cargo run` was rebuilding under concurrent cargo builds in the shared build directory (other agents' `rustc` processes were running, and a parallel build failed with "failed to map object file"). The two later runs had stderr captured and succeeded. This does not change the spec. The worker re-runs the kit open once on an idle machine and records the time and peak RSS.

## 3. Behaviour rules

1. **One limit for file imports.** A single constant, `MAX_IMPORT_FILE_BYTES: u64 = 512 * 1024 * 1024`, used by `designos_open_file` and by `designos_import_web_capture` when it receives `path`. Its doc comment cites the owner ruling (2026-10-18, Q2). The value does not change; only the owner can change it.
2. **Typed refusal everywhere.** Both tools return `FILE_TOO_LARGE`:
   - at `/path`
   - `received` = the byte length
   - `expected` = `"<= 536870912 bytes"`
   - message `"Input file exceeds the 512 MiB import limit"`

   No size refusal may go through `from_plain_message`.
3. **Comparison.** `len > MAX_IMPORT_FILE_BYTES` refuses, so exactly 512 MiB is accepted. The check runs on `fs::metadata` (symlinks followed) before any read, as today.
4. **Retry advice.** `FILE_TOO_LARGE` is `safe_to_retry:false`, because the same file fails the same way. `default_next_step` gains an arm: "This file is over the limit in `expected`. Open it in DESIGN:OS Studio, or use a smaller file." The same arm covers the 1,200,000-layer refusal, which shares the code.
5. **Usage text only.** `--version`/`-V` and `--help` stay as they are (#845). The usage error at `main.rs:29` becomes `Usage: designos-mcp --workspace DIRECTORY | --version | --list-lint-rules | --help`. Nothing else in `main.rs` changes.
6. **Inline captures are unchanged.** `designos_import_web_capture` with `capture` (inline JSON) has no file, so the file limit does not apply. This spec adds no limit to the inline argument.

## 4. Data model / schema changes

None. No tool schema, description or rules-pack text changes. `rules.rs:561` already lists `FILE_TOO_LARGE` for `designos_import_web_capture`, and `gen-tool-docs` output is unchanged; the worker confirms with `git diff --stat` after running it.

## 5. Contracts

- Rust: `pub(crate) const MAX_IMPORT_FILE_BYTES: u64 = 512 * 1024 * 1024;` next to `input_path` in `server.rs`, plus a helper `fn import_size_gate(len: u64) -> Result<(), ToolError>` that both arms call.
- Web capture over the limit:

  ```json
  {"error":{"code":"FILE_TOO_LARGE","path":"/path","received":537919488,"expected":"<= 536870912 bytes","message":"Input file exceeds the 512 MiB import limit","safe_to_retry":false,"next_step":"This file is over the limit in `expected`. Open it in DESIGN:OS Studio, or use a smaller file."}}
  ```

- `designos_open_file`: the same envelope, which differs from today only in `safe_to_retry` and `next_step`.
- CLI: a wrong invocation prints `Usage: designos-mcp --workspace DIRECTORY | --version | --list-lint-rules | --help` and exits 1, as today. `--version` itself is unchanged (#845).

## 6. File-format mapping

None.

## 7. UI

None. This is headless; no Studio surface changes, and `es-designer` is not needed.

## 8. Edge cases

- A capture file between 64 MiB and 512 MiB: now passes the size gate and is parsed. Invalid JSON keeps its current error (`INVALID_ARGUMENTS`, "invalid capture JSON").
- A capture of exactly 536,870,912 bytes: accepted by the gate.
- A file that grows between the size check and the read: still read whole, as today. This is a local workspace file; no new guard is added.
- A `.designos` over the limit that is otherwise valid: `FILE_TOO_LARGE`. No partial open.
- Both limits in one session: the layer limit refuses after load with the same code, and `received` stays the file path there, as today. Only the `next_step` text changes.
- `--version` together with other arguments: the usage error (now naming `--version`), exit 1, as today.

## 9. Non-goals

- Raising or removing the 512 MiB limit or the 1,200,000-layer limit (owner ruling Q2). Streaming import.
- Making `designos-mcp` use less memory than about 29× the `.fig` size.
- Updating the stale `/Applications/designos.app` (environment: install the 0.6.0 `.dmg`). Changing the scratch client's default binary.
- Any change to `--version`/`-V` or `--help` (already shipped in #845).
- A `--version` flag for the Studio CLI (`designos`). Possible follow-up; the About dialog already shows the version.
- A size limit for the inline `capture` argument.

## 10. Test plan (red first)

Sparse files come from `File::set_len`, which fixture 44 already uses (`server.rs:2437-2439`), so no large fixture is committed.

| Case | Where | Expected | Fails before |
|---|---|---|---|
| `web_capture_between_64_and_512_mib_passes_the_size_gate` | `tests/session.rs` | 65 MiB sparse capture: the error is about JSON, not `import limit` | yes (64 MiB message) |
| `web_capture_over_512_mib_is_file_too_large` | `tests/session.rs` | 513 MiB sparse capture: `FILE_TOO_LARGE`, path `/path`, `received` = byte length, `expected` set | yes (`INVALID_ARGUMENTS`, path `""`) |
| `open_file_between_64_and_512_mib_passes_the_size_gate` | `tests/session.rs` | 65 MiB sparse `.designos`: `DOCUMENT_INVALID`, never `FILE_TOO_LARGE` | guard (green at HEAD) |
| `file_too_large_is_not_safe_to_retry_and_names_the_next_step` | `server.rs` fixture 44 | `safe_to_retry:false`, `next_step` mentions Studio | yes |
| `every_code_has_a_fixture` | `server.rs` | still green with the updated fixture 44 | guard |
| `usage_error_names_the_version_flag` | `tests/version.rs` (its `run` helper) | `designos-mcp --bogus` exits 1 and stderr contains `--version` | yes (usage names only `--workspace`) |
| `version_flags_print_the_cargo_version` | `tests/version.rs` (exists) | unchanged, green | guard |

Run:
- `cargo test -p designos-mcp --test session`
- `cargo test -p designos-mcp --test version`
- `cargo test -p designos-mcp every_code_has_a_fixture`
- `cargo run -p designos-mcp --bin gen-tool-docs`, then `git diff --stat` shows no generated change
- `cargo fmt --all --check`
- `cargo clippy -p designos-mcp --all-targets -- -D warnings`

Evidence for the PR: a terminal capture of three things on the PR build:
- `--version` plus one wrong invocation (the usage text)
- the 513 MiB web-capture refusal (typed envelope)
- the kit open: `node_count` 207,601, with `/usr/bin/time -l` peak RSS

## 11. Performance budgets

- The size gate is one `stat` call, unchanged.
- Kit open through the MCP, recorded on an idle M-series Mac: at most 120 s wall time and at most 12 GB peak RSS for the 336 MB kit. Measured in this session: 65 s and 9.6 GB, then 150 s and 9.9 GB under heavy concurrent builds. A worse result is reported in the PR, not hidden. Memory reduction is a non-goal.
- No document mutation, and no `doc.to_json()`.

## 12. Dependencies and build order

Independent of every other revamp spec. Order:
1. The constant and helper. Both arms use them.
2. The `default_next_step` arm and `safe_to_retry`.
3. The fixture 44 update.
4. The usage text and its test in `tests/version.rs`.
5. Gates.
6. The evidence capture.
7. PR (squash).

## 13. Owner rulings that apply

- 2026-10-18, Q2 (`docs/specs/v0.6/README.md:180`): "raise them to 1,200,000 layers (`validate_document`) and 512 MiB (imports). The other safety limits, such as deadlines and memory, stay."
- 2026-10-25, R3 (plan.md): Studio improves itself; no Figma. The kit must be workable through Studio's own CLI and MCP.
- 2026-09-22: visual evidence in every PR. Standing: `main` is protected (PR, green macOS test, clippy, rustfmt, squash).

## Decisions on the open questions (writer, 2026-10-25)

1. **Raise the limit to what Studio handles, stream, or add an opt-in, as the brief suggested?** None of them. The kit (336 MB) is under the owner's 512 MiB limit, and 0.6.0 and later open it. The refusal came from a stale 0.4.0 binary. Raising the limit would reverse an owner ruling without a file that needs it. Reason kept: the measured 9.6–9.9 GB peak shows the limit still guards memory.
2. **Does the web-capture limit follow the 512 MiB ruling?** Yes. Q2 says "512 MiB (imports)", and both tools import a workspace file. The 64 MiB check at `server.rs:1056` is the one #602 missed: E15-17 cites only the open-file check.
3. **`safe_to_retry:false` for `FILE_TOO_LARGE`?** Yes. The same call fails the same way. This matches `CHECK_FAILED` in #896.
4. **A `--version` flag?** It already exists on `origin/main` (#845, `tests/version.rs`); an independent review caught that the first draft of this spec re-specified it from the older local `ec47da26`. Only the usage error changes, so a wrong invocation points at it. The dogfood failure was a stale binary that `designos-mcp --version` would have exposed in one command, and 0.4.0 has no such flag.

## Open questions for the owner

None. All the calls here are technical, and the size value stays as the owner ruled.

**Note to the controller:** plan.md "Facts the specs rest on" → "Dogfood gaps hit" says designos-mcp refuses the kit. That holds only for the stale 0.4.0 build. On 0.6.0 and later the kit opens (207,601 nodes). The plan's facts list should say so; this spec does not edit plan.md.
