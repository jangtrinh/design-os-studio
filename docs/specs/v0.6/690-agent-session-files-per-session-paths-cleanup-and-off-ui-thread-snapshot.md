# #690 Agent session files: per-window and per-session paths, cleanup on close, and no full-document JSON on the UI thread

| Field | Value |
|---|---|
| Issue | #690 (parent #585); related: #43 (drop the `/tmp` live-sync fallback for open and new document) |
| Size | M |
| Depends on | soft: #653 (stable session folders: its rules decide which folders are "kept") |
| Unblocks | none |
| Touches | new `crates/designos-app/src/agent_session_files.rs` (snapshot worker, owner marker, stale sweep), `crates/designos-app/src/terminal.rs` (hot spot: `for_tab` `:704-744`, `bind_session` `:758-775`, `sync_live_file_context` `:800-855`, `poll_live_agent_bridge` channel 1b `:1298-1313`, `configure_pty_env_with_session` `:2301-2304`, `shutdown` `:991-995`), `crates/designos-mcp/src/server.rs` (`:45-70`), `scripts/zero_fake_ui_gate.py` (`Z16`) and its self-test, tests |

Code baseline: `origin/main@9f3f7a5`. Line numbers differ from the issue's (`:~804, 816`, `:676-680`); the exact ones are below.

## 1. Summary and user story

As a designer with two Studio windows open and agents attached, I want each agent to see its own file and no other. I want closed tabs to leave no files behind, and I want typing and dragging to stay smooth on a large file while the agent's copy updates.

## 2. Current state (FACT)

1. **Shared `/tmp` files.** In non-test builds `sync_live_file_context` also writes `/tmp/designos-live-context.json` (`terminal.rs:836`) and `/tmp/designos-live-canvas.designos` (`:848`) on every change. `for_tab` reads the mtime of `/tmp/designos-live-agent-out.spec.json` (`:715`) and `poll_live_agent_bridge` watches and applies that file as a second channel (`:1298-1313`). `configure_pty_env_with_session` exports `DESIGNOS_LIVE_SPEC=/tmp/designos-live-agent-out.spec.json` and `DESIGNOS_SELECTION_JSON=/tmp/designos-live-selection.spec.json` when no session is given (`:2301-2304`). `designos-mcp` falls back to `/tmp/designos-live-canvas.designos`, `/tmp/designos-live-doc-sync.designos`, `/tmp/designos-live-agent-out.spec.json` and `/tmp/designos-live-context.json` when an env var is missing (`designos-mcp/src/server.rs:53-61`); with no env at all it keeps no live bridge (`:46-49`). Nothing under `docs`, `scripts`, `design` or the README names these paths (search run on this commit).
2. **Folders never deleted.** The session folder is `temp_dir()/designos-sessions/<session id>` (`terminal.rs:265`, `:708`, `:764`). A window's id is `p<pid>-w<n>-tab-<tab>` (`tabs.rs:161-171`, `terminal.rs:700-702`); a bare terminal uses `p<pid>-tab-<tab>`. `shutdown` kills the children but removes nothing (`:991-995`); `remove_dir_all` appears only in tests (`:4620-4621`).
3. **Full-document JSON on the UI thread.** `sync_live_file_context` calls `doc.to_json()` for documents up to 25,000 nodes, and `scoped_canvas(...).to_json()` above that, on the UI thread whenever the mutation epoch, page or selection changes (`:841-844`). It also writes `context.json`, the MCP config, hooks and skill files there (`:827-851`, `provision_session_agent_files` `:857-`). CLAUDE.md section 4 forbids UI-thread serialization of the document.
4. **The issue's "same pattern the autosave uses" does not match the code.** The library autosave writes the document synchronously (`lib.save_file_counted(&lf.id, &self.doc)`, `home/session.rs:613`). Only the thumbnail refresh runs in a thread, from a full `Document` clone (`thumb::refresh_in_background(lib, id, self.export_doc().into_owned())`, `:629`; `home/thumb.rs:97-101`).
5. **There is no cheap clone.** `Document.nodes` is a `BTreeMap<NodeId, Node>` (`designos-doc/src/lib.rs:147`); a clone copies every node. UNKNOWN: its cost at 25,000 nodes. Check: time `doc.clone()` on a 24,000-node fixture with `test_timing::fastest_within`.
6. The publish is skipped while an edit is in progress, agent edits are held, or auto-layout subtrees are still draining (`:812-815`); `last_synced_key` then stays unset. Keep this guard.
7. Two Studio processes differ by pid; two windows in one process differ by the `w<n>` counter. A pid can be reused after a crash.
8. `test_timing::fastest_within` is the existing budget helper for UI-thread cost tests (`crates/designos-app/src/test_timing.rs`).

## 3. Behaviour rules

1. **No global path.** Non-test and test builds read and write only files inside the session folder. Delete the writes at `:836`, `:848`, the read at `:715`, the channel at `:1298-1313`, the env defaults at `:2301-2304` (a session is always supplied) and the `/tmp` fallbacks in `designos-mcp/src/server.rs:53-61` (a missing path stays `None`). Gate `Z16` fails when shipped code of `terminal.rs` or `server.rs` names any `/tmp/designos-live-` path.
2. **Per window and session.** Folder names keep the window id (`p<pid>-w<n>`) and, with #653, the stable per-file key. Two windows never share a folder: a second window that finds a folder owned by a live process uses its own (#653 rule 3).
3. **Owner marker.** On creating a session folder write `owner.json { pid, started_ms }`.
4. **Cleanup on close.** Closing a tab removes its session folder after its children are killed, unless the session is kept for resume (#653: a folder under the library's `agent-sessions/`). `shutdown` of a whole window does the same for every tab.
5. **Cleanup on launch.** At start-up, off the UI thread, remove folders under `designos-sessions` that are older than 7 days (folder mtime) and whose owner is gone: the marker's pid is not running, or its start time differs from the running process with that pid. A folder without a marker uses the pid in its name. Never remove a folder whose owner is alive, never remove anything outside `designos-sessions`, and never touch `agent-sessions/` (stable folders).
6. **Off the UI thread.** The UI thread decides whether to publish (the key check and the guard of fact 6) and takes a snapshot; a worker thread serializes and writes `canvas.designos`, `context.json` and the provisioned agent files with `atomic_write_session_file`. The worker keeps only the newest request (latest wins), so a burst writes at most one file per finished job.
   - Snapshot = a clone of the document (at most 25,000 nodes) or the scoped canvas (above that), built on the UI thread. If the clone alone costs more than the budget of section 11, stop: report the number and ask (open question 1). Do not hide the cost by lowering the 25,000 threshold.
   - `last_synced_key` is set when the worker has written that key, so the guard and the agent's reads agree.
7. The agent never reads a partial file: writes stay atomic (temp file then rename).

## 4. Data model / schema changes

None in the document. New session-folder file `owner.json`. In memory: the worker handle and a `latest: Option<Snapshot>` slot.

## 5. Contracts

- `designos-mcp` environment: the variables Studio already sets (`DESIGNOS_SESSION_DIR` and the specific paths) keep their names; the fallback defaults disappear. An external agent that relied on the global files must set `DESIGNOS_SESSION_DIR`; no such use is documented in this repository.
- `fn sweep_stale_sessions(root: &Path, now: SystemTime, is_alive: impl Fn(u32, Option<u64>) -> bool) -> usize` (pure with an injected liveness check; tested).
- Process liveness needs a way to ask the OS. UNKNOWN: whether `libc` or `sysinfo` is already a direct dependency of `designos-app`. Check: `cargo tree -p designos-app -i libc`; add the smallest one if absent.

## 6. File-format mapping

None. `canvas.designos` content is unchanged (same serializer, same scoped rules).

## 7. UI

None. Evidence: terminal captures. The PR shows (a) `ls /tmp/designos-live-*` before the change (files, mtimes) and after (none created), (b) `ls` of `designos-sessions` after closing a tab, (c) the timing test output on the 24,000-node fixture before and after. A screenshot of the Agents panel working on that file at 1440 proves the feature still works. Responsive: SKIPPED (owner default). es-designer is not needed (no UI code).

## 8. Edge cases

1. The agent reads `canvas.designos` while the worker writes: it sees the old or the new whole file.
2. The user edits rapidly: many snapshots requested, one or two written; the last always lands.
3. Studio crashes: folders stay; the next launch sweeps those older than 7 days with a dead owner.
4. A pid is reused by another program: the start time differs, so the folder counts as abandoned.
5. The worker thread fails to write (disk full): the status line says so once; the key stays unsynced and retries on the next change.
6. A folder is open in the user's own terminal when cleanup runs: only folders whose owner is gone and that are older than 7 days are removed.
7. Two windows, same file, before #653: separate folders by window id; no shared files exist any more.
8. Closing a tab while a snapshot is in flight: the worker finishes into a folder that is then removed, or the removal waits for the worker's ack; the file is not recreated afterwards.
9. A test build: same code path as release, so the tests exercise the real worker.

## 9. Non-goals

- The scope threshold and node copying of the scoped canvas (#696 owns variables and styles). #43 (open and new document). The structure-sharing document clone (open question 1). The `agent-sessions/` layout (#653).

## 10. Test plan (red first)

Seams: `Sim`/terminal unit tests with two `TerminalState`s, a fake clock and liveness for the sweep, and `fastest_within` for UI-thread cost.

| Case | Expected | Fails before |
|---|---|---|
| `no_global_tmp_path_in_shipped_code` (gate `Z16`) | no `/tmp/designos-live-` in shipped `terminal.rs` or `server.rs` | yes |
| `a_sync_creates_no_global_file` | mtimes of the four global names unchanged or absent after two syncs | yes (non-test builds write them; the test runs the shared code path) |
| `two_windows_keep_separate_contexts` | two states with two docs: each `context.json` names its own doc and page | guard |
| `mcp_without_a_session_dir_has_no_live_paths` | `LiveSessionBridge` paths are `None` for a partial env | yes |
| `closing_a_tab_removes_its_session_folder` | folder gone; a kept (#653) folder stays | yes |
| `sweep_removes_only_old_folders_with_a_dead_owner` | table: old dead, old alive, young dead, no marker with pid in name, foreign folder | yes |
| `sweep_never_touches_stable_session_folders` | `agent-sessions/` untouched | guard |
| `snapshot_is_written_off_the_ui_thread` | the UI thread never calls `to_json` (probe on the thread id) | yes |
| `ui_thread_cost_on_a_24k_node_document_stays_under_budget` | `fastest_within(4 ms, ...)` for the clone and enqueue | yes if the claim in fact 3 holds (measure first; if the current code already passes, say so in the PR) |
| `burst_of_changes_writes_the_latest_snapshot` | final file equals the last state | yes |
| `agent_never_reads_a_partial_file` | repeated reads during writes all parse | guard |

Run: `cargo test -p designos-app terminal agent_session_files`, `cargo test -p designos-mcp`, `python3 scripts/zero_fake_ui_gate.py`.

## 11. Performance budgets

- UI thread per publish: the key check, the guard, and the clone, at most 4 ms at 24,000 nodes (initial value; measured and recorded), so no frame exceeds 16 ms because of the sync.
- Worker: one serialization per finished job; memory is one snapshot in flight and one pending.
- Launch sweep: off the UI thread; under 50 ms of work for 1,000 folders.
- No periodic timer; the worker blocks on a channel.

## 12. Dependencies and build order

After part 3's #653 for the folder rules (it rewrites `for_tab`, `bind_session` and the `SessionEnv` paths); if #653 slips, rules 1 and 3 (removal of global paths, owner marker) can go first because they touch about ten lines. `terminal.rs` hot spot: logic in `agent_session_files.rs`. `designos-mcp/src/server.rs` is shared with #43 and E15-5: rebase. Merge order is in README-part4.

## 13. Owner rulings

- Context and canvas files live only in the session folder keyed by window and session; closing a tab removes its folder unless kept for resume; launch removes folders older than 7 days with no owner process; the snapshot is written off the UI thread; two windows keep separate contexts. (Issue text.)
- CLAUDE.md section 4: no UI-thread document serialization.

## Open questions for the owner (recommended first)

1. If cloning a 24,000-node document on the UI thread is itself over budget. Recommended: report the number, and make the snapshot incremental (send only changed nodes to the worker) as its own task; keep the 25,000 threshold unchanged.
2. The marker file `owner.json`. Recommended: add it; the alternative is to trust only the pid in the folder name, which a reused pid can fool.
