# #653 Stable per-file agent sessions that survive a restart

| Field | Value |
|---|---|
| Issue | #653 (agent terminal UX list, ruled for v0.6 on 2026-10-19; audit `plans/reports/agent-terminal-ux-roadmap-audit-261019.md` experience 1) |
| Size / priority | L / P1 (foundation: #651, #652, #677, #678 build on its registry) |
| Depends on | none (builds on 0.5.0 launch commands, #642 and #646-#650) |
| Unblocks | #651, #652, #677, #678, #679 |
| Touches | new `crates/designos-app/src/agent_sessions.rs` (registry, path rules, resume arguments), `crates/designos-app/src/terminal.rs` (hot spot: `TerminalState::for_tab`, `bind_session`, `spawn_tab_running`, `AgentCliPreset::cli_launch_command`; keep the edits thin), `crates/designos-app/src/tabs.rs` (`session_id_for_tab`), `crates/designos-library/src/store.rs` (new `Library::agent_session_dir`, cleanup on delete), tests |

Code baseline: `origin/main@01d48f2` (0.5.0). The issue cites `terminal.rs:668`; the pid-scoped id is now
`TerminalState::for_tab` (`terminal.rs:706-708`) and `TabsState::ensure_instance_id` (`tabs.rs:161-166`).

## 1. Summary and user story

As a designer who closes Studio and reopens a file, I click Agent and the previous conversation for that
file continues, because the file's agent session folder and the CLI's own session records are the same
ones as before the restart.

## 2. Current state (FACT)

- A session id is `p<pid>-w<n>-tab-<tab id>` (`tab_session_id`, `terminal.rs:698-700`; instance id
  `p{pid}-w{n}` from a process counter, `tabs.rs:161-166`). The session folder is
  `std::env::temp_dir()/designos-sessions/<session id>` (`terminal.rs:706-708`, `bind_session` `:759-775`).
  Nothing in it survives a new process, and the OS can clear the temp folder.
- The folder holds `context.json`, `canvas.designos`, `mcp-config.json`, `hooks.json`, `SKILL.md`,
  `AGENTS.md`, `doc-patch.jsonl`, `agent-status.json`, `selection.spec.json`, `.claude/skills/...`,
  `.agents/...`, `.grok/config.toml` and `.grok/hooks/`, all provisioned by
  `provision_session_agent_files` (`terminal.rs:857-948`) and rewritten from the live document
  (`sync_live_file_context`, `:800-855`).
- Every agent CLI tab starts with its cwd set to this folder (`resolve_pty_cwd`, `terminal.rs:2254-2265`);
  the Grok launch also passes `--cwd "$DESIGNOS_SESSION_DIR"` (`:173`).
- The four launch commands never resume (`cli_launch_command`, `terminal.rs:155-180`): they always start a
  new conversation.
- A tab belongs to a document tab (`ProjectTab.terminal`, `tabs.rs:99`) with `lib_file: Option<LibFile>`
  (`home/session.rs:18-23`: `id`, `project_id`, `project`, `name`) and `file_path: Option<PathBuf>`.
  A file opened from disk has `lib_file == None` (`io.rs:833`, `home/mod.rs:732`, `:806`).
- Library root: `$DESIGNOS_LIBRARY` or the platform data dir (`designos-library/src/store.rs:454`);
  per-file data already lives under the root: `projects/<project>/files/<file>.designos`, thumbnails,
  a trash folder (`store.rs:461-464`, `:900-994`).
- What the CLIs offer for resuming (sources: each CLI's `--help` and `--version` captured 2026-10-19 in
  `plans/reports/release-060-evidence/cli-help-261019/`; Grok also from its bundled user guide
  `~/.grok/docs/user-guide/17-sessions.md`, grok 1.0.62; Claude from `claude --help`, 2.1.302):

| CLI (version) | Resume the most recent | Resume by id | Pre-assign an id for a new session | Scoped to the working directory |
|---|---|---|---|---|
| Claude Code 2.1.302 | `-c, --continue` ("most recent conversation in the current directory") | `-r, --resume [id]` | `--session-id <uuid>` | yes (stated for `--continue`) |
| Codex 0.163.0 | `codex resume --last` | `codex resume [SESSION_ID or name]` | none found in `--help` | the picker and `--last` filter by cwd unless `--all` is given (help text of `--all`) |
| Grok 1.0.62 | `-c, --continue`; `-r` with no value | `-r, --resume [id or title]` | `-s, --session-id <uuid>` for a new session | yes: sessions are stored per working directory (`~/.grok/sessions/<encoded-cwd>/<id>/`, user guide) |
| Antigravity (agy) 1.4.2 | `-c, --continue` ("most recent conversation") | `--conversation <id>` | none found in `--help` | not stated in `--help`: unverified |

## 3. Behaviour rules

1. **Stable identity.** A file's agent session folder is a fixed path derived from a key, shown here in the
   recommended location (open question 2): `<library root>/agent-sessions/<key>/`, where `<key>` is the library file id for a library file
   (`LibFile.id`). The folder is created on first use and reused on every later open of that file, in every
   process. Its contents are the same files today's session folder holds. The `p<pid>` scheme and the temp
   folder are no longer used for files that have a key.
2. **Files without a library id.** The key rule for a file opened from disk is an owner decision (see Open
   questions). Until it is answered, such files keep today's per-process temp folder and get no resume.
3. **Concurrent windows.** Two windows with the same file would share one folder; the second window to open
   the same key gets a lock file `session.lock` (pid + start time) and falls back to a per-process folder with
   a one-line status "Another window owns this file's agent session" (no resume there). A stale lock
   (pid not running) is taken over.
4. **Registry.** `agent-sessions/<key>/sessions.json` lists the **agent CLI** sessions Studio started for the file. Only CLIs with a resume command are registered (Claude, Codex, Grok, Antigravity); a Shell tab and a custom
   "Other CLI..." command are never registered, so the newest entry is always one that can be resumed (#651 rule 2.2). Each entry:
   `{ id, cli, label, created_ms, last_active_ms, last_status }`, newest first, at most 50 entries per file
   (older ones are dropped from the registry; the CLI's own store is never touched). The registry is
   written atomically (`atomic_write_session_file`, `terminal.rs:2184-2191`) whenever a tab starts, changes
   status to done/idle, or is renamed.
5. **Session ids.** Claude and Grok sessions get an id chosen by Studio (a UUID passed with `--session-id`
   / `-s` on a new session) so the registry knows the id without reading the CLI's files. Codex and
   Antigravity have no flag to pre-assign an id in `--help`, and Studio does **not** look an id up in their private stores: their entries carry no id and resume by "most recent in this folder"
   (`resume --last`, `--continue`; rule 6). A rollout-file lookup for Codex was considered and rejected: a newer conversation started outside Studio in the same folder would be picked up and resume the wrong one.
6. **Resume commands** (all run through `build_agent_shell_cmd`, `terminal.rs:2112-2127`, with the same
   flags as today plus):
   - Claude: `--resume <id>` (known id) or `--continue`;
   - Codex: `resume --last` (no id is known), followed by today's `-c mcp_servers.designos...` overrides
     (the `codex_mcp_overrides!` macro, `terminal.rs:63-73`) and the bypass flag;
   - Grok: `--resume <id>` or `--continue`, keeping `--cwd "$DESIGNOS_SESSION_DIR"` and `--always-approve`;
   - Antigravity: `--continue` only when its cwd scoping has been shown by a live check (below); `--conversation <id>` is used only with an id Studio can obtain without reading agy's private files, which none of agy's `--help` offers. Until the live check passes, an agy entry is not resumable and is not registered, the new-session picker shows instead (#651).
   Because the cwd is the stable folder, "most recent" means "most recent for this file" for Claude and Grok (both state cwd scoping in their own documentation or `--help`). For Codex, the `--all` option's help text
   ("disables cwd filtering") implies the picker is cwd-filtered by default, but `--last` itself says only "continue the most recent session": **whether `--last` is cwd-scoped needs a live check before the Codex part of this task is
   Ready**; if it is not scoped, Codex entries are not registered and Codex always starts new. The same live check applies to agy's `--continue`.
7. **Every resume refreshes the provisioned files** (context, canvas, MCP config, hooks, skill) before the CLI
   starts, exactly as a new tab does (`spawn_tab_running` calls `provision_session_agent_files`, `:1225`).
8. **Failure to resume** (the CLI exits within the five-second window, for example because the session was
   deleted in the CLI): the existing start-failure panel appears (`agent_start_failure`) with the CLI's output;
   the registry entry is marked `missing` and the panel's fix is "Start a new session" (new fix action).
9. **Cleanup.** Emptying the library trash for a file deletes its `agent-sessions/<key>/` folder; deleting a
   file's agent sessions is also offered from the sessions dropdown of #651. The CLIs' own conversation
   stores are never deleted by Studio.
10. **Environment.** `DESIGNOS_SESSION_DIR` and every other `DESIGNOS_*` variable point at the stable folder
    (`configure_pty_env_with_session`, `terminal.rs:2278-2335`), unchanged in name.
11. Skip-permissions stays the default; resume uses the same permission flags as a new start. `GROK_FOLDER_TRUST=0`
    stays (`terminal.rs:2282-2287`).

## 4. Data model / schema changes

- `Library::agent_session_dir(file_id: &str) -> PathBuf` and `Library::remove_agent_sessions(file_id)` in
  `designos-library`.
- `agent_sessions::SessionRegistry { entries: Vec<SessionEntry> }` with `SessionEntry` as in rule 4, plus
  `version: u32 = 1`; unknown fields are kept on rewrite (`serde(flatten)` map) so a newer Studio's data
  survives an older one.
- `TerminalState` gains `key: Option<String>` and an `Option<SessionRegistry>`; `for_tab` keeps working for
  files without a key.

## 5. Contracts

- `designos-mcp` is unchanged: it still reads `DESIGNOS_SESSION_DIR` and friends (`designos-mcp/src/server.rs:35-44`).
- `sessions.json` is a Studio-internal file; its shape is versioned (rule 4) and documented in the module
  header. Tools and agents must not write it.

## 6. File-format mapping

None in `.designos` / `.fig` (session data never enters the document). Library: the new folder sits next to
`projects/` and is ignored by library export/import.

## 7. UI

No UI of its own. The visible effects belong to #651 (open flow) and #652 (picker). One status line is added
to the panel's existing status text (`status_msg`) for rule 3.

## 8. Edge cases

1. Library file renamed or moved to another project: the id is unchanged; sessions follow.
2. Library file duplicated: a new id, a new empty session folder (conversations are not copied).
3. File restored from trash: its folder was deleted when the trash was emptied; if only trashed, the folder
   remains and is reused.
4. Library root changed (`$DESIGNOS_LIBRARY` differs between runs): the new root has no folder, so no resume;
   not an error.
5. Session folder missing while the registry says there are sessions (user deleted it): the registry is
   recreated empty; the CLI's own store may still hold conversations, which the picker's "recent for this
   folder" resume (rule 6) can reach.
6. Registry corrupted: renamed to `sessions.json.bad`, a fresh one is started, a status line says so.
7. Grok and Claude ids supplied by Studio already exist in the CLI's store (a reused UUID): not possible with a
   fresh UUIDv4 per new session; on collision the CLI's own error shows in the failure panel.
8. Windows: the library root and folder rules are unchanged; resume flags are the CLIs' own (releases are
   verified on macOS).

## 9. Non-goals

- Copying or exporting conversations; reading conversation content.
- Restoring a closed tab's scrollback (the CLI redraws its own history).
- Cross-machine sync.
- Listing sessions the user started outside Studio (the CLI's own `/resume` picker does that).

## 10. Test plan (red first)

Seams: unit tests with a temp library root (`DESIGNOS_LIBRARY`), stub CLIs written as shell scripts that
record their argv (the pattern of `spawn_exiting_stub_agent`, `terminal.rs:544-548`).

| Case | Expected (fails before) |
|---|---|
| `session_dir_is_keyed_by_library_file_id` | two `TerminalState`s for the same `LibFile.id` in different "processes" (different pid inputs) have the same folder under the library root |
| `registry_round_trip_keeps_unknown_fields` | write, add an unknown field, rewrite: field kept |
| `claude_new_start_passes_a_session_id_and_registers_it` | stub `claude` receives `--session-id <uuid>`; the registry holds that uuid |
| `resume_commands_per_cli` | table-driven: argv of the stub for Claude (`--resume id`, `--continue`), Codex (`resume --last` plus the MCP overrides; never an id), Grok (`--resume id`, `--continue`, `--cwd`), agy (`--continue` only when its live check passed) |
| `shell_and_custom_cli_tabs_are_never_registered` | start a Shell tab and a custom command: `sessions.json` unchanged |
| `codex_has_no_id_lookup` | a newer rollout file for the same folder created by another process does not change the Codex resume argv |
| `restart_resumes_the_previous_session` | start stub, record entry, drop the state, build a new state for the same key: the resume argv carries the recorded id |
| `second_window_does_not_share_the_folder` | lock held: the second state gets a different folder and the status line |
| `stale_lock_is_taken_over` | dead pid in the lock: taken over |
| `resume_failure_marks_the_entry_missing` | stub exits at once with an error: start-failure panel and `missing` entry |
| `trash_empty_removes_the_session_folder` | `empty_trash` removes `agent-sessions/<id>` |
| `launch_commands_use_only_flags_each_cli_lists` | each launch/resume string checked against the captured `--help` flag sets in `cli-help-261019/` (extends the #642/#646 pin test) |

Evidence: terminal capture of the red-then-green run, plus one end-to-end recording on a real CLI (owner
machine, Claude and Grok): quit Studio, reopen the file, click Agent, the previous conversation continues
(screenshots of the conversation before and after the restart). Owner-manual for agy until the cwd scoping is
verified.

## 11. Performance budgets

- Registry read/write: under 5 ms (small JSON), done off the UI thread on start and on status changes.
- No new work per frame; `sync_live_file_context` is unchanged.
- Folder size: dominated by `canvas.designos` (a copy of the live document, `doc.nodes.len() <= 25_000`
  else a scoped copy, `terminal.rs:840-844`); budget: no more than one document-sized file per file entry
  (already true), reported in the PR for the large synthetic fixture.

## 12. Dependencies and build order

First of the agent-panel items. #651 and #652 build on its registry and path rules; #677 and #678 read its
session ids. `terminal.rs` is the hot spot: put new logic in `agent_sessions.rs` and keep `terminal.rs` edits to
the call sites (see README-part3).

## 13. Owner rulings

- Clicking Agent resumes the last session (#651).
- Skip-permissions stays the default; `GROK_FOLDER_TRUST=0` stays.
- Codex tabs keep the heuristic status for now.

## Open questions for the owner

1. **Key for files opened from disk (no library id).** A `.fig` or `.designos` opened from a path has no library
   file id, so the issue's "key it by the library file id" does not cover it.
   - A (recommended): key by a hash of the file's canonical path. Sessions follow the file in place and
     survive restarts; renaming or moving the file starts a fresh history.
   - B: import such files into the library first, so every file has an id. Resume then always works, but opening
     a disk file silently adds a copy to the library, which the owner may not want.
   - C: no resume for disk files (today's temporary folder). Simplest, and a file opened from disk never gets
     its conversation back.
2. **Where session folders live.** Sessions hold a copy of the live document and agent files, so location
   decides disk use and clean-up.
   - A (recommended): in the library root next to `projects/` (`agent-sessions/<file-id>/`). The library already owns
     per-file data and trash clean-up, and the folder is deleted with the file. Precedent: the owner answer of 2026-10-18 for branches (T10.1, `docs/specs/v0.6/README.md` of #615) put
     per-file branch data in the library next to the file (`branches/<file-id>/`).
   - B: in the OS cache folder. The OS may clear it, which breaks resume silently.
   - C: next to the file on disk (`<file>.agent/`). Visible and portable, but it litters the user's folders
     and fails on read-only locations.

## Could not ground

- Antigravity: whether `--continue` is scoped to the working directory, how to learn a new conversation's id
  (`~/.gemini/antigravity-cli/conversations/<uuid>.db` files exist, observed 2026-10-19, but nothing ties a file
  to a folder), and any flag to pre-assign an id. Until a live check shows that `--continue` is cwd-scoped, agy is not resumable and not registered (rule 6). The live check is
  a gate before the agy part of this task is Ready.
- Codex: whether `resume --last` is limited to the working directory (the help text of `--all` implies a cwd filter for the picker; `--last` is not stated). Live check before Ready: start Codex in two folders, resume `--last` in each.
