# E15-6 Deduplicate replayed agent writes with idempotency keys

| Field | Value |
|---|---|
| Epic / pack | E15 / v0.6 AI design |
| Size / priority | S / P0 |
| Depends on | E15-4, E15-5 |
| Unblocks | RD-3 (spawned agents retry safely) |
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
  `server.rs:293-317`).
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
   - key absent from the ledger → run the call; on success store `{key, tool, hash, doc_epoch,
     reply, created_ms}`; errors are not stored (a failed call is retried for real);
   - key present, same tool and hash, same `doc_epoch` → return the stored reply unchanged, run
     nothing, publish nothing; the result's `_meta` carries `{"designos/idempotent_replay": true}`
     (outside `structuredContent`, so the reply body is identical);
   - key present with a different tool or hash → `IDEMPOTENCY_KEY_REUSED`, nothing runs,
     `safe_to_retry: true`.
4. Lifetime: the ledger belongs to the document session.
   - With a session directory, it is persisted to `<session_dir>/idempotency-ledger.jsonl` (append
     one line per stored entry, fsync per write) and loaded on server start, so it survives an MCP
     reconnect.
   - Without a session directory (standalone `--workspace` use), it is in memory for the process.
   - It is cleared when the document closes or switches: `designos_open_file`,
     `designos_new_document`, and a canvas reload that replaces the document
     (`sync_from_live_canvas_if_newer`, `server.rs:194-219`). Clearing truncates the file.
5. Capacity: at most 5,000 entries or 16 MiB of stored replies; beyond that the oldest entries are
   dropped. A dropped key behaves as unknown. The cap and the drop count are reported by
   `designos_get_app_state` (E15-16).
6. Undo does not clear entries: a retry with the same key after the user undid the batch returns the
   stored reply and does not re-apply. Documented in the tool description ("a key names one logical
   write").

## 4. Data model / schema changes

- `struct LedgerEntry { key: String, tool: String, hash: u64, doc_epoch: u64, reply: Value,
  created_ms: u64 }` (serde, one JSON line each).
- `Session.ledger: Ledger` with `lookup`, `store`, `clear`, backed by an optional file path.
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
4. Same key used by two different agents (RD-3): the hash includes the tool name and payload only, so
   identical payloads from two agents dedupe; RD-3 prefixes keys with the agent run id to avoid this.
5. Dry-run call with a key: dry runs are not stored (no state change to protect); the key is checked
   for mismatch only.
6. Very large reply (diff images, E15-8): image content is not stored; the stored reply keeps the image
   metadata, and a replay re-renders the images from the current document, flagged in `_meta`
   `designos/images_rerendered: true`.

## 9. Non-goals

- Cross-document or cross-session dedupe.
- Idempotency for read tools.

## 10. Test plan

Seam: `cargo test -p designos-mcp`.

| Case | Steps | Expected |
|---|---|---|
| `same_key_same_payload_applies_once` | send batch twice with key `k1` | node count +1 once; replies byte-identical structuredContent; second has replay `_meta` |
| `key_reuse_with_changed_payload_refused` | `k1` with a different name | `IDEMPOTENCY_KEY_REUSED`; revision and undo depth unchanged |
| `ledger_survives_reconnect` | session dir in temp; call, drop `Session`, new `Session::with_session_dir`, resend | no new nodes; stored reply returned |
| `ledger_cleared_on_document_switch` | call, `designos_new_document`, resend | runs again |
| `errors_not_stored` | first call invalid, fix payload, same key | second call runs and is stored |

Fail-before: all fail today (`idempotency_key` is an unknown field).

## 11. Performance budgets

Lookup O(1) (`HashMap<String, LedgerEntry>`); hashing O(payload). Appending one line plus fsync per
stored write: under 2 ms on SSD; recorded. Loading 5,000 entries at start: under 50 ms.

## 12. Dependencies and build order

After E15-4 and E15-5. Independent of E15-7/8.

## 13. Owner rulings

- All AI through the user's agent CLI: the retry behaviour targets the agent CLIs that run in Studio's
  terminal.
- Parallel agents hard-locked to frames (RD-3): keys are namespaced per agent run (8.4).
