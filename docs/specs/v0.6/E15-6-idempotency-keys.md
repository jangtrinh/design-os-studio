# E15-6 Deduplicate replayed agent writes with idempotency keys

| Field | Value |
|---|---|
| Epic / pack | E15 / v0.6 AI design |
| Size / priority | S / P0 |
| Depends on | E15-4, E15-5 |
| Unblocks | RD-3 (spawned agents retry safely), E15-13 (its undo test needs keys) |
| Touches | `crates/designos-mcp/src/tools/batch.rs`, new `crates/designos-mcp/src/tools/ledger.rs`, `crates/designos-mcp/src/server.rs` (dispatch wrapper) |

## 1. Summary and user story

As an agent CLI that retries a call after a timeout, I attach an `idempotency_key`. If the first call
already landed, the retry returns the stored reply and creates nothing. If I mistakenly reuse a key
for a different payload, I get a named error and nothing changes.

## 2. Current state (FACT)

- No mutating tool accepts an idempotency key; argument structs use `deny_unknown_fields`
  (`crates/designos-mcp/src/engine.rs:577-702`), so a key is rejected today.
- Each MCP connection is a separate stdio process (`crates/designos-mcp/src/main.rs:5-17`), so any
  in-memory state is lost when an agent CLI reconnects.
- Studio sessions have a per-file session directory (`LiveSessionBridge.session_dir`,
  `crates/designos-mcp/src/server.rs:19-32`, `for_session_dir` `server.rs:80-93`) and a document epoch
  counter announced on every document switch (`server.rs:28-29`, `publish_live_doc_sync`
  `server.rs:293-317`). That counter does not move on `designos_undo`/`designos_redo`
  (`engine.rs:379-388`), so it cannot tell a retry that the write was undone.
- serde_json is used without `preserve_order` (`Cargo.toml` workspace `serde_json = "1"`), so
  `serde_json::Map` is ordered by key and serializes canonically.

## 3. Behaviour rules

1. Every mutating tool (the `mutating` list at `server.rs:508-520` plus `designos_apply_batch`,
   `designos_set_in_progress` (E15-7) and later write tools) accepts optional
   `idempotency_key: string` (1..=128 chars, `[A-Za-z0-9._:-]`).
2. Payload hash = 64-bit FNV-1a over the canonical JSON of the call arguments with
   `idempotency_key` removed, prefixed by the tool name. Canonical = `serde_json::to_vec` of the
   key-ordered `Value`.
3. Lookup happens before any validation or dispatch:
   - key absent from the ledger → run the call; on success store `{key, tool, hash, undo_depth, live,
     gone, reply, created_ms}` (`undo_depth` per item 6, `live` and `gone` per item 7); errors are not
     stored (a failed call is retried for real);
   - key present, same tool and hash, write still applied (item 6) and the document still holds its
     result (item 7) → return the stored reply unchanged, run nothing, publish nothing; the result's
     `_meta` carries `{"designos/idempotent_replay": true}` (outside `structuredContent`, so the reply
     body is identical);
   - key present, same tool and hash, entry undone or dead (item 6), or the document no longer holds
     its result (item 7) → `IDEMPOTENCY_KEY_STALE`, nothing runs, `safe_to_retry: true`, `next_step`
     "Read the target nodes; this write was undone or is not in the open document. Send it again with
     a new idempotency_key only if you still want it." The stored reply is never returned, because it
     may name nodes that no longer exist;
   - key present with a different tool or hash → `IDEMPOTENCY_KEY_REUSED`, nothing runs,
     `safe_to_retry: true`.
4. Lifetime: the ledger belongs to the document session.
   - With a session directory, it is persisted to `<session_dir>/idempotency-ledger.jsonl` (append
     one line per stored entry, fsync per write) and loaded on server start, after the initial canvas
     load of `Session::with_session_dir` (that first load is not a clearing reload), so it survives an
     MCP reconnect. A loaded entry is a claim about a document the new process has to check (item 7).
   - Without a session directory (standalone `--workspace` use), it is in memory for the process.
   - It is cleared when the document closes or switches: `designos_open_file`,
     `designos_new_document`, and a canvas reload that replaces the document
     (`sync_from_live_canvas_if_newer`, `server.rs:194-219`). Clearing truncates the file.
5. Capacity: at most 5,000 entries or 16 MiB of stored replies; beyond that the oldest entries are
   dropped. A dropped key behaves as unknown. The cap and the drop count are reported by
   `designos_get_app_state` (E15-16).
6. Undo and redo are tracked per entry through the undo stack, not by clearing the ledger:
   - Each entry stores `undo_depth` = `History::undo_depth()` right after the call's own commit and
     before any E15-13 auto-fix entry is pushed on top of it (so undoing only the fixes leaves the key
     valid).
   - On lookup the write counts as undone when `history.undo_depth() < entry.undo_depth`. A key whose
     write is still applied replays. A key whose write was undone gets `IDEMPOTENCY_KEY_STALE` and is
     not re-applied (the undo was deliberate; re-applying would bring back what the user removed).
     Undoing the second of two writes leaves the first key valid.
   - Redo back to the stored depth makes the key valid again (the write is back, the reply is true).
   - A new undoable write while the redo stack is non-empty discards that redo branch: every entry
     whose `undo_depth` is above the depth at that moment is marked dead for good (stale even if the
     depth later grows again).
   - Each such transition is appended to the ledger file as `{"key":K,"state":"stale"|"valid"}`; on
     load the earlier process's history is gone, so a loaded entry has `undo_depth = None` ("reloaded":
     the depth rule does not apply, item 7 alone decides) and a loaded `stale` entry stays stale. A
     new entry stored after the reload gets a real depth and follows the depth rule.
   - Studio-side undo: it changes the canvas snapshot, which the session picks up through the canvas
     reload (`sync_from_live_canvas_if_newer`). That reload clears the whole ledger (item 4), so a
     retry after a Studio undo runs as a new write. This differs from an MCP-side `designos_undo` (the
     retry is refused) and is recorded as an open owner question in the PR.
   Documented in the tool description ("a key names one logical write").
7. Presence check on every replay. A key counts as applied only while the open document holds what the
   write produced. At store time the call records `live` = `[{id, kind}]` for every node the reply says
   exists after the call (ids under `created` and `name_to_id` values that exist in the committed
   document, plus `node_id` of a non-delete tool; a node the same batch created and then deleted is
   not listed) and `gone` = the ids the call deleted (`designos_delete_node`, batch `delete` ops;
   nodes created and deleted inside one batch are not listed). On lookup, before the stored reply is
   returned: every `live` id must exist with the same `kind`, and no `gone` id may exist. Any miss →
   `IDEMPOTENCY_KEY_STALE`, nothing runs. The check also covers a reload: the new process sees only
   what `canvas.designos` holds (`server.rs:194-214`; a normal edit appends `doc-patch.jsonl`,
   `server.rs:249-266`, and does not rewrite the canvas, and history is in memory only), so a key whose
   nodes are not in the loaded document is stale, never "applied forever". An entry with empty `live`
   and `gone` (variables-only batch, a comment on no node) has nothing to check: it replays inside the
   process by the depth rule and is `IDEMPOTENCY_KEY_STALE` once reloaded. The check compares ids and
   kinds only; an editor that reused an id for a node of the same kind would pass it (residual risk,
   named in 8.7).

## 4. Data model / schema changes

- `struct LedgerNode { id: u64, kind: String }`.
- `struct LedgerEntry { key: String, tool: String, hash: u64, undo_depth: Option<usize>, dead: bool,
  live: Vec<LedgerNode>, gone: Vec<u64>, reply: Value, created_ms: u64 }` (serde, one JSON line each;
  `undo_depth` is `None` after a reload; state transitions are extra `{"key","state"}` lines).
- `Session.ledger: Ledger` with `lookup(key, current_undo_depth, &Document)`, `store`, `mark_dead_above(depth)`,
  `clear`, backed by an optional file path.
- Input schemas of mutating tools add the `idempotency_key` property (declared once for the batch in
  E15-4).

## 5. Contracts

Example retry:

```json
// call 1 and call 2 (identical)
{"name":"designos_apply_batch","arguments":{"idempotency_key":"run7-step3","ops":[{"op":"insert","parent":1,"kind":"rectangle","name":"Tag"}]}}
// both replies: identical structuredContent; call 2 has _meta {"designos/idempotent_replay": true}
```

Mismatch:

```json
{"error":{"code":"IDEMPOTENCY_KEY_REUSED","path":"/idempotency_key",
  "message":"Key run7-step3 was used for a different designos_apply_batch payload.",
  "safe_to_retry":true,"next_step":"Send this payload with a new idempotency_key."}}
```

`corrected_example`: the same arguments with the key suffixed `-2`.

## 6. File-format mapping

None. The ledger file lives in the session directory, never in the document.

## 7. UI

None.

## 8. Edge cases

1. Retry arrives while the first call is still running: impossible inside one process (the session is
   behind a `Mutex`, `server.rs:14-16`); across a reconnect the first process has finished or died. If
   it died mid-call, no entry was stored and the retry runs for real; the E15-5 tuple shows whether
   the first call changed state.
2. Key reused after a document switch: the ledger was cleared, so it runs as new.
3. Corrupt ledger line on load (partial write): skipped and counted; never fails startup.
4. Same key used by two different agents (RD-3): each Studio agent has its own session directory
   (RD-3 §2), so ledgers are separate and keys never collide across agents. A caller that shares one
   session directory between agents must namespace its own keys; the hash includes only the tool name
   and payload.
5. Dry-run call with a key: dry runs are not stored (no state change to protect); the key is checked
   for mismatch only.
6. Very large reply (diff images, E15-8): image content is not stored; the stored reply keeps the image
   metadata, and a replay re-renders the images from the current document, flagged in `_meta`
   `designos/images_rerendered: true`.
7. Reload with an id collision: the editor may give a colliding id to another layer on replay
   (`replay.rs:16-17`). A same-kind node on the stored id passes the presence check although it is a
   different layer; the agent's next update then edits the wrong layer. Not closed here (a node
   fingerprint would need a stable per-node token the model does not have); the tool description tells
   agents to `designos_get_node` after a reconnect before trusting a replayed id.
8. Reconnect with a stored reply naming ids the new document lacks (no canvas written yet, or a later
   delete removed them): `IDEMPOTENCY_KEY_STALE`, the document is untouched, and the agent re-sends
   with a new key.

## 9. Non-goals

- Cross-document or cross-session dedupe.
- Idempotency for read tools.

## 10. Test plan

Seam: `cargo test -p designos-mcp`.

| Case | Steps | Expected |
|---|---|---|
| `same_key_same_payload_applies_once` | send batch twice with key `k1` | node count +1 once; replies byte-identical structuredContent; second has replay `_meta` |
| `key_reuse_with_changed_payload_refused` | `k1` with a different name | `IDEMPOTENCY_KEY_REUSED`; revision and undo depth unchanged |
| `ledger_survives_reconnect` | session dir in temp; batch with `k1` creates node N; write the document to the session's `canvas.designos` (what Studio's snapshot does); drop `Session`; new `Session::with_session_dir`; resend `k1` | N exists in the new session; no new nodes; stored reply returned |
| `reconnect_without_document_state_is_stale` | same, but the canvas file is not written (the new session's document lacks N) | `IDEMPOTENCY_KEY_STALE`; no node created; `designos_get_node` N is `NODE_NOT_FOUND`; undo depth unchanged |
| `reconnect_after_node_deleted_is_stale` | as `ledger_survives_reconnect`, then `designos_delete_node` N in the new session; resend `k1` | `IDEMPOTENCY_KEY_STALE`; no new node |
| `replay_after_created_node_deleted_is_stale` | one process: `k1` creates N; `designos_delete_node` N; resend `k1` | `IDEMPOTENCY_KEY_STALE`; N stays absent |
| `delete_key_replays_only_while_gone` | `k1` deletes node M; resend `k1` (replays); then `designos_undo` brings M back; resend `k1` | replay, then `IDEMPOTENCY_KEY_STALE` |
| `reloaded_entry_without_checkable_ids_is_stale` | `k1` is a variables-only batch (empty `live` and `gone`); replays in-process; reconnect; resend `k1` | in-process replay; `IDEMPOTENCY_KEY_STALE` after reconnect |
| `reloaded_key_follows_presence_not_depth` | reconnect with N present; `designos_update_node` on N (new write, history starts empty); resend `k1` | replay (N present); never "applied forever" by a forced depth, only by the presence check |
| `ledger_cleared_on_document_switch` | call, `designos_new_document`, resend | runs again |
| `errors_not_stored` | first call invalid, fix payload, same key | second call runs and is stored |
| `undo_then_same_key_is_refused_not_replayed` | batch with key `k1` creates a node; `designos_undo`; resend `k1` | `IDEMPOTENCY_KEY_STALE`; the node stays absent; revision and undo depth unchanged |
| `undo_second_write_keeps_first_key_valid` | writes `k1` then `k2`; `designos_undo`; resend `k1`, then `k2` | `k1` replays (node count unchanged, no duplicate); `k2` is `IDEMPOTENCY_KEY_STALE` |
| `redo_makes_key_valid_again` | write `k1`; undo; redo; resend `k1` | replay |
| `new_write_after_undo_kills_undone_key` | write `k1`; undo; write `k2` (discards the redo branch); resend `k1` | `IDEMPOTENCY_KEY_STALE` even if the depth later grows past `k1`'s |
| `stale_mark_survives_reconnect` | session dir; call `k1`, undo, drop `Session`, new `Session::with_session_dir`, resend `k1` | `IDEMPOTENCY_KEY_STALE` |
| `ledger_cleared_on_canvas_reload` | session with a live canvas file; call `k1`; replace the canvas file with a newer snapshot; resend `k1` | runs again as a new write |

The test `undo_of_auto_fix_keeps_write_key_valid` (write `k1` that triggers an E15-13 fix; one
`designos_undo`; resend `k1`; replay, the write is present, only the fix is gone) needs E15-13, which
lands in wave 4 after this spec's wave 3. It lives in the E15-13 test plan and is that PR's
acceptance for the `undo_depth` bookkeeping in item 6. This spec's own tests use no auto-fix.

Fail-before: all fail today (`idempotency_key` is an unknown field).

## 11. Performance budgets

Lookup O(1) (`HashMap<String, LedgerEntry>`); hashing O(payload). Appending one line plus fsync per
stored write: under 2 ms on SSD; recorded. Loading 5,000 entries at start: under 50 ms.

## 12. Dependencies and build order

After E15-4 and E15-5. Independent of E15-7/8.

## 13. Owner rulings

- All AI through the user's agent CLI: the retry behaviour targets the agent CLIs that run in Studio's
  terminal.
- Parallel agents hard-locked to frames (RD-3): ledgers are per session directory, one per agent (8.4).
