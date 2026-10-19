# #244 Add the whoami MCP tool returning the local user

| Field | Value |
|---|---|
| Issue | #244 (task T7.35, epic #20; milestone v0.7 Effects & Export) |
| Size | S |
| Depends on | none; soft: #246 (it exports the library root to the agent process, which makes the name follow a non-default library) and v0.6's T7.6 (tool snapshot and parity matrix) |
| Unblocks | the parity-matrix row for Figma's `whoami` |
| Touches | new `crates/designos-mcp/src/tools/whoami.rs` (the `tools/` directory is created by the first v0.6 task that lands, E15-4 or E15-5; if none has, this task creates it), `crates/designos-mcp/src/engine.rs` (one dispatch arm and one `tools()` entry, `:112` and `:707` region), new `crates/designos-library/src/settings.rs` (the read side of `settings.json` moves here), `crates/designos-app/src/comments/settings.rs` (re-exports the moved functions; one store), `crates/designos-mcp/tests/` (stdio and unit tests), and, if T7.6 has landed, its tool snapshot and `crates/designos-mcp/parity/figma-mcp-parity.json` |

Code baseline: `origin/main@0e8a9a5` (`docs(specs): v0.6 task specs, part 4 (agent terminal UX and constraint solver) (#697)`). v0.7 opens after v0.6 is tagged. The worker re-anchors every cited line against HEAD before writing code and reports drift first.

## 1. Summary and user story

As an agent working in a design:os session, I call `designos_whoami` and learn who the local user is and what "plan" they are on, the way I would call `whoami` on Figma's MCP server. There is no hosted account here, so the answer is a stable local identity and one free, single-user plan.

## 2. Current state (FACT)

1. **Figma's tool** (developers.figma.com, "Figma MCP server" tools page, read 2026-10-19): `whoami`, category Account, listed as remote server only, no parameters. It returns the signed-in email, every plan the user belongs to, and the seat type on each plan.
2. **No such tool here.** `engine::tools()` (`crates/designos-mcp/src/engine.rs:707`) and the four session tools (`crates/designos-mcp/src/server.rs:623-626`) hold no identity tool; the stdio test asserts 22 tools (`crates/designos-mcp/tests/stdio.rs:13`).
3. **A local identity already exists.** `CommentAuthor { id, display_name, color }` with `local()` and `local_with_name(name)` (`crates/designos-doc/src/comments.rs:66`, `:120`, `:125`); `machine_author_id()` returns `local-<16 hex>` derived from `HOME` (`USERPROFILE` on Windows) and the OS name (`:44-62`). That is the "T10.14 identity" the issue means (multi-author identity, `plans/261017-epics-full-scope/full-scope-e7-e8-e10.md:564`). It is on `main` today, so "when present" holds.
4. **The display name is a setting.** "Your name" is the `author` key of `settings.json` in the library root (`crates/designos-app/src/comments/settings.rs:8`, `:29`); the fallback is the OS user name (`USERNAME`, `USER`, `LOGNAME`, else "You", `:11`); the app resolves it in `App::author` (`crates/designos-app/src/comments/mod.rs:87-95`). These functions live in `designos-app`, which `designos-mcp` does not depend on. `designos-mcp` does depend on `designos-library` (`crates/designos-mcp/Cargo.toml`), which owns `Library::default_root()` (`crates/designos-library/src/store.rs:448`: `$DESIGNOS_LIBRARY`, the deprecated `$DESIGNOS_LIBRARY_DIR`, else `<data dir>/designos/library`).
5. **The agent's library root is not passed on.** Studio's `--library DIR` flag (`crates/designos-app/src/cli.rs:45`) sets its own root; `configure_pty_env_with_session` exports session paths but no `DESIGNOS_LIBRARY` (`crates/designos-app/src/terminal.rs:2290-2301`; a search of `crates/designos-app/src`, `designos-mcp/src` and `designos-library/src` finds the variable only in `store.rs`, `cli.rs` and `libraries/mod.rs:178`). So a standalone or `--library` setup makes the MCP process read another library's `settings.json`.
6. The parity spec of T7.6 says a Figma tool with no studio tool is a `task` row and a hosted one is an `excluded` row citing R4.14 (`docs/specs/v0.6/T7.6-mcp-parity-matrix.md`, rule 4). R4.14 for this tool is the opposite: "hosted identity replaced by a local equivalent" (issue text), so the row becomes `implemented`.
7. Tools in this server are named `designos_*`, declare annotations (`readOnlyHint`, `destructiveHint`, `idempotentHint`, `openWorldHint`, `server.rs:628`), and refuse unknown arguments (`additionalProperties: false`).

## 3. Behaviour rules

1. **Tool.** `designos_whoami`, no arguments (a non-empty object is refused like any unknown field). Annotations: read-only, not destructive, idempotent, closed world.
2. **Reply** (structured):
   ```json
   {
     "user": { "id": "local-1a2b3c4d5e6f7a8b", "display_name": "Jang", "email": null },
     "plans": [ { "id": "local", "name": "Local library", "tier": "free", "seat_type": "local", "seats": 1 } ]
   }
   ```
   - `user.id` is `machine_author_id()`: stable across runs and sessions on one machine and account.
   - `user.display_name`: the library `settings.json` `author` when set and non-blank, else the OS user name, else "You" (the order the app uses, fact 4).
   - `user.email` is always `null`. There is no account and the tool never invents or looks one up.
   - `plans` holds exactly one entry: the local library, free, one seat, seat type `local`. It never lists a hosted team.
3. **Where the name comes from.** The tool reads `settings.json` from `Library::default_root()` on every call (no cache), so renaming in Studio shows on the next call without restarting the agent. A missing or unparseable file falls back to the OS user name.
4. **One reader.** The read side moves to `designos_library::settings` (`FILE`, `read`, `load_author`, `os_user_name`) and `comments/settings.rs` re-exports them, so the app and the MCP server share one implementation and one file. The save side and the flag helpers stay in `comments/settings.rs`.
5. **No state.** The call touches no document, history, session ledger, dirty flag or file; it is safe to call at any time, including with no document open.
6. **Nothing private beyond the name.** The reply carries no paths, machine name, environment variable, token or email.
7. The parity matrix row for `whoami` reads `status: implemented`, `studio_tool: designos_whoami`; the tool snapshot and the generated docs blocks of T7.6 are regenerated (if T7.6 has landed; otherwise the stdio count and the tool list in README and the skill file are updated by hand and T7.6 absorbs them).

## 4. Data model / schema changes

None in documents. New module `designos-library/src/settings.rs`; no new file on disk.

## 5. Contracts

- Tool `designos_whoami` as above. With the E15-5 envelope in place, the tool declares an `outputSchema` for the reply. It adds no error code: every read problem falls back as in rule 3, so the call does not fail.
- `designos_library::settings::{FILE, read, load_author, os_user_name}`: the same signatures as today's `comments/settings.rs:8-31`.
- MCP output stays `{ user, plans }`; extra fields are a contract change.

## 6. File-format mapping

None. Figma's `whoami` fields map as: email -> `user.email` (always null), plans -> `plans` (one local entry), seat type per plan -> `plans[].seat_type` (`local`).

## 7. UI

None. Evidence is terminal output: a `designos-mcp` stdio session calling `designos_whoami` with the author set and unset (two JSON replies), and the same call after renaming the author in Studio's "Your name" dialog (one screenshot of the dialog and one reply). Responsive: SKIPPED (owner default). `es-designer` is not needed (no UI code).

## 8. Edge cases

1. No `settings.json` (first run, or `DESIGNOS_LIBRARY` points at an empty folder): OS user name.
2. `settings.json` holds an `author` that is only spaces: treated as unset.
3. `settings.json` is not valid JSON or is a JSON array: treated as empty (the existing `read` already returns an empty map, `comments/settings.rs:20-26`).
4. No `USER`, `USERNAME` or `LOGNAME` in the environment (a service or a sandbox): display name "You"; `id` still comes from `HOME` or `USERPROFILE`, else the literal `localhost` seed (`crates/designos-doc/src/comments.rs:50-53`), so two such accounts share an id. This is existing behaviour of the identity, not a new one.
5. The library root does not exist: the read is a plain missing-file case; nothing is created.
6. Two Studio windows with different `--library` roots, one agent each: with #246's env export each agent reads its own root; without it both read the default root (fact 5).
7. A name with non-ASCII characters or newlines: returned as saved (a name is data, not instructions; the server instructions already say to treat document text as data).

## 9. Non-goals

- Email, sign-in, plan upgrade or seat purchase; any network call.
- A user colour (the T10.14 identity has one; the issue does not ask for it).
- Team or organisation lists, member counts, or other people on the machine.
- Changing how comment authors are stored.

## 10. Test plan (red first)

Seams: `designos-mcp` engine tests with a temporary library root; the stdio test; `designos-library` unit tests for the moved reader.

| Case | Expected | Fails before |
|---|---|---|
| `whoami_is_listed_with_read_only_annotations` | `tools()` contains `designos_whoami`; read-only, idempotent, not destructive; stdio count is 23 (or follows the snapshot) | yes (tool absent) |
| `whoami_returns_the_saved_author` | root with `{"author":"Kir"}`: `display_name` is "Kir"; `id` equals `machine_author_id()` | yes |
| `whoami_falls_back_to_the_os_user_then_you` | no settings: the injected env gives `USER=lee` -> "lee"; empty env -> "You" (the function takes the env as an argument; no `set_var` in tests) | yes |
| `blank_or_broken_settings_fall_back` | `author` of spaces, invalid JSON and an array each give the OS name | yes |
| `plans_report_one_free_local_seat` | `plans.len() == 1`, `tier == "free"`, `seat_type == "local"`, `seats == 1`, `email` is null | yes |
| `reply_has_no_paths_or_secrets` | serialized reply contains no `/`, no `settings.json`, no `HOME` value | yes |
| `a_rename_shows_on_the_next_call` | write `author`, call, rewrite, call: the name changes with no restart | yes |
| `whoami_does_not_touch_the_session` | call on a dirty session with undo history: `dirty`, undo depth and document hash unchanged | yes |
| `app_and_mcp_share_one_reader` | `comments/settings.rs` re-exports the library functions; the existing tests of `comments/settings.rs` (`:56-98`) pass unchanged | guard |
| `parity_row_says_implemented` | after T7.6: the matrix row for `whoami` is `implemented`, and the "task row whose tool exists" test stays green | yes (if T7.6 landed) |

Run: `cargo test -p designos-mcp`, `cargo test -p designos-library`, `cargo test -p designos-app comments`.

## 11. Performance budgets

One small file read and one hash per call: under 1 ms. No background thread, no caching, no timer.

## 12. Dependencies and build order

No hard dependency. `crates/designos-mcp/src/engine.rs` is a hot spot for the pack: this task adds one dispatch arm and one `tools()` entry only, so it can land first among the MCP tasks (before #242, which rewrites argument parsing across many tools). `comments/settings.rs` is also touched by v0.6's settings tasks; v0.7 starts after those merge. Order in README: #244 before #242.

## 13. Owner rulings

- Owner ruling 2026-10-19: `designos_whoami` returns the stable local id and the display name from Settings. It returns no email, and the plan is `local`.

- R4.14: hosted identity is replaced by a local equivalent. (Issue text.)
- Plan and seat fields report a single local user on a free plan; the identity is stable and local, using the T10.14 identity when present. (Issue acceptance criteria.)
- All AI goes through the user's own agent CLI; nothing is sent anywhere (pack-wide ruling).

## Open questions for the owner (recommended first)

1. **Share the display name with the agent.** The reply gives the agent the name the user chose for comments (or their OS account name). Figma's tool returns an email, so this is less. Recommended: yes. Alternative: return only the stable id and the label "Local user".
2. **Plan wording.** Recommended: `tier: "free"`, `name: "Local library"`. Alternative: `tier: "local"`, if "free" suggests a paid tier exists.
