# E15-5 Return named MCP errors with a corrected example, a `safe_to_retry` flag and structured output

| Field | Value |
|---|---|
| Epic / pack | E15 / v0.6 AI design |
| Size / priority | M / P0 |
| Depends on | E15-4 |
| Unblocks | E15-6, E15-15 (rules pack quotes the catalogue), every later MCP tool |
| Touches | `crates/designos-mcp/src/server.rs`, new `crates/designos-mcp/src/tools/errors.rs`, `crates/designos-mcp/src/engine.rs` (`tools()` schemas, error returns), `crates/designos-doc/src/spec.rs` (`deny_unknown_fields`) |

## 1. Summary and user story

As an agent, when a call fails I get a stable error code, the JSON path of the bad value, the valid
options, a corrected payload I can send as is, and a flag that tells me whether resending is safe or
whether I must re-read the canvas first. Every successful reply is structured and declared by an
`outputSchema`, so I never parse prose.

## 2. Current state (FACT)

- Engine and session return `Result<Value, String>` (`crates/designos-mcp/src/engine.rs:37`,
  `crates/designos-mcp/src/server.rs:386`). Messages are free text, e.g. `"limit must be between 1 and
  500"` (`engine.rs:60`), `"invalid arguments: {serde error}"` (`engine.rs:548-550`).
- `call_tool` already maps success to `CallToolResult::structured(value)` and failure to
  `CallToolResult::structured_error(json!({"error": message}))` (`server.rs:654-656`), i.e.
  `isError: true` with `structuredContent: {"error": "<string>"}`.
- Tool descriptors carry `inputSchema` and annotations but no `outputSchema`
  (`engine.rs:712-716`, `server.rs:628`). The server enables only the tools capability
  (`server.rs:635`).
- `NodeSpec` silently ignores unknown keys (`crates/designos-doc/src/spec.rs:45`), so a typo such as
  `"corner_radus"` is lost without an error.
- Some failures change state before failing: `designos_open_file` replaces the session document, then
  can fail to tell the editor (`server.rs:412-418`); this must not be reported as retry-safe.
- The stdio test asserts 22 tools and `is_error` flags (`crates/designos-mcp/tests/stdio.rs:12-21`).

## 3. Behaviour rules

### 3.1 Error envelope

Every failed tool call returns `isError: true`, a text content block with the compact JSON (for
clients without structured content support) and:

```json
{"error": {
  "code": "UNKNOWN_VARIANT_VALUE",
  "message": "Variant property Size has no value \"xl\".",
  "path": "/ops/0/spec/variant/Size",
  "received": "xl",
  "expected": "one of: sm, md, lg",
  "valid_options": ["sm", "md", "lg"],
  "corrected_example": {"op": "apply_spec", "parent": 1, "spec": {"kind": "instance", "component": "Button", "variant": {"Size": "sm"}}},
  "safe_to_retry": true,
  "next_step": "Resend with one of valid_options."
}}
```

Field rules:
- `code`: from the catalogue (3.3), `UPPER_SNAKE`, stable across releases; codes are never reused for
  another meaning.
- `path`: JSON Pointer (RFC 6901) into the call's `arguments`; empty string for whole-call errors.
- `valid_options`: at most 20 entries, ranked as the producing spec defines; omitted when not
  applicable.
- `corrected_example`: the smallest complete argument object (or batch op) that passes validation,
  built by substituting the first valid option into the offending value. Omitted only when no
  mechanical correction exists (e.g. `NODE_NOT_FOUND`).
- `safe_to_retry`: `true` only when the server guarantees that this call changed nothing: document,
  history, session ledger (E15-6), edit log, files. `false` whenever any of those may have changed.
- `next_step`: one sentence naming the concrete next action.

### 3.2 `safe_to_retry` determination

Computed, not hand-set per message:
1. Before dispatch, the session records `(doc revision counter, history undo depth, edit-log length,
   ledger length)`; file-writing tools also record whether the output file existed.
2. After the error, the same tuple is compared. Equal and no file created → `true`; otherwise
   `false`.
3. The document revision counter is a new `Engine.revision: u64` incremented by every successful
   `exec`, rollback and canvas reload; it replaces any need to hash the document.

Examples: an invalid batch (nothing ran) → `true`; editor publish failure whose rollback succeeded →
`true` (tuple restored); `designos_open_file` that swapped the document and then failed to announce
it (`server.rs:417-418`) → `false`, code `EDITOR_NOT_NOTIFIED`, `next_step` "Call
designos_get_document before editing."

### 3.3 Catalogue

`errors.rs` defines `enum ErrorCode` with a `const ALL: &[ErrorCode]` and, per code, its message
template and default `next_step`. Initial catalogue (codes added by later specs append here):

| Group | Codes |
|---|---|
| Arguments | `INVALID_ARGUMENTS`, `UNKNOWN_FIELD`, `MISSING_FIELD`, `VALUE_OUT_OF_RANGE`, `TEXT_TOO_LONG`, `INVALID_COLOR` |
| Nodes | `NODE_NOT_FOUND`, `NOT_A_CONTAINER`, `PAGE_ROOT_PROTECTED`, `MOVE_CYCLE`, `INSTANCE_CYCLE`, `OVERRIDE_FIELD_NOT_APPLICABLE` |
| Grounding (E15-1..3) | `UNKNOWN_COMPONENT`, `AMBIGUOUS_COMPONENT`, `UNKNOWN_VARIANT_PROPERTY`, `UNKNOWN_VARIANT_VALUE`, `NO_SUCH_VARIANT`, `UNKNOWN_OVERRIDE_PATH`, `AMBIGUOUS_OVERRIDE_PATH`, `UNKNOWN_COMPONENT_PROPERTY`, `PROPERTY_TYPE_MISMATCH`, `FIELD_NOT_ALLOWED_ON_INSTANCE`, `UNKNOWN_SLOT`, `SLOT_FILL_NOT_ALLOWED`, `SLOT_CHILD_COUNT`, `INVALID_SLOT_VALUE`, `SLOT_CONTAINER_NOT_FOUND`, `UNKNOWN_VARIABLE`, `AMBIGUOUS_VARIABLE`, `VARIABLE_TYPE_MISMATCH`, `VARIABLE_SCOPE_MISMATCH`, `UNKNOWN_STYLE`, `STYLE_TYPE_MISMATCH`, `UNKNOWN_VARIABLE_MODE` |
| Batch (E15-4) | `BATCH_OP_FAILED`, `UNKNOWN_REF`, `DUPLICATE_AS`, `BATCH_TOO_LARGE` |
| Session | `UNKNOWN_TOOL`, `TOOL_DISABLED` (E15-15), `UNSAVED_CHANGES`, `PATH_OUTSIDE_WORKSPACE`, `FILE_EXISTS`, `FILE_TOO_LARGE`, `DOCUMENT_INVALID`, `EDITOR_NOT_NOTIFIED`, `EDITOR_PUBLISH_FAILED`, `PATCH_TOO_LARGE`, `HISTORY_DROPPED`, `INTERNAL_ERROR` |
| Added by other specs of this pack (each spec owns its fixture) | E15-6 `IDEMPOTENCY_KEY_REUSED`, `IDEMPOTENCY_KEY_STALE` (its `next_step`: read the targets, the write was undone or is not in the open document); E15-7 `IN_PROGRESS_TARGET_NOT_ROOT`, `NODE_IN_PROGRESS_BY_OTHER_RUN`; E15-9 `UNKNOWN_RULE`, `UNKNOWN_PRESET`, `EXACTLY_ONE_SCOPE`; E15-17 `INVALID_SELECTOR`, `CURSOR_STALE`, `RULE_UNAVAILABLE`; T7.1 `IMAGE_TOO_LARGE`, `NODES_ON_DIFFERENT_PAGES`, `INVALID_REGION`, `EXACTLY_ONE_TARGET`; T7.4 `UNKNOWN_LANGUAGE`; T7.9 `UNKNOWN_PROPERTY_KEY`; T7.40 `UNKNOWN_TOPIC`, `TOPIC_NOT_AVAILABLE`; RD-3 `OUTSIDE_OWNED_FRAMES`, `FRAME_LEASED`, `NOT_LEAD`, `SPAWN_REQUIRES_STUDIO`, `RUNTIME_NOT_INSTALLED`, `TOO_MANY_AGENTS`; RD-4 `UNKNOWN_SKILL` |

`BATCH_OP_FAILED` nests the op's own envelope under `error.cause` and sets `path` to
`/ops/<i>/...`.

### 3.4 Unknown keys

`NodeSpec`, `LayoutSpec` and `OverrideSpec` gain `#[serde(deny_unknown_fields)]`. A serde
unknown-field error maps to `UNKNOWN_FIELD` with `valid_options` = the 5 closest field names of that
struct (Levenshtein). The live-spec drop path in Studio reports the same message in its toast instead
of applying a partial spec.

### 3.5 Structured output

- Every tool declares `outputSchema` (JSON Schema, `type: object`). Its shape is
  `{"anyOf": [<success schema>, <error envelope schema>]}` so error results with `structuredContent`
  conform.
- Success replies keep their current field names (no rename); schemas are written for them.
- Complex tools carry 1-5 worked examples in their description, each a complete call plus the key
  reply fields: `designos_apply_spec` (3), `designos_apply_batch` (4), `designos_query_xpath` (3);
  `designos_apply_style` (part 2) gets its examples when it lands.

## 4. Data model / schema changes

- `ToolError { code, message, path, received, expected, valid_options, corrected_example, cause,
  next_step }` with `impl From<ToolError> for Value`. `safe_to_retry` is filled by the session (3.2),
  never by the producer.
- Engine and tool functions return `Result<Value, ToolError>`. `Engine.revision: u64` (3.2).
- Back-compat note: `structuredContent.error` changes from a string to an object. The string is kept as
  `error.message`. This is an intentional contract change listed in `CHANGELOG.md`.

## 5. Contracts

- Applies to all tools in `tools()` (`engine.rs:705`, `server.rs:620`).
- Server instructions (`server.rs:637`) gain one sentence: "Errors carry code, path, valid_options,
  corrected_example and safe_to_retry; when safe_to_retry is false, re-read with
  designos_get_document before retrying."
- Idempotency: unchanged here (E15-6). Dry-run: errors from a dry run are always
  `safe_to_retry: true`. Undo: an error never adds an undo entry.

## 6. File-format mapping

None.

## 7. UI

None in Studio beyond the live-spec toast text (3.4). Zero fake UI: no new controls.

## 8. Edge cases

1. Serde error without a field name (wrong JSON type at root): `INVALID_ARGUMENTS`, `path` "".
2. Nested batch error inside an override path: `path` is the full pointer
   `/ops/2/spec/overrides/0/path`.
3. Error raised after a partial file write (`designos_save_file` write failure removes the new file,
   `server.rs:460-463`): file existed before? no; exists after? no → `safe_to_retry: true`.
4. Panic inside a tool: caught at the session boundary (`std::panic::catch_unwind` around dispatch),
   reported as `INTERNAL_ERROR` with `safe_to_retry: false`; no automatic state repair is attempted,
   and `next_step` asks the agent to re-read with `designos_get_document`. `INTERNAL_ERROR` is in the
   catalogue (Session group).
5. Option list larger than 20 (component with 300 variants): first 20 by the producer's ranking, plus
   `valid_options_total`.
6. Corrected example would itself be invalid for another reason (two errors in one payload): the
   example fixes only the reported error; the next call reports the next one. Documented.

## 9. Non-goals

- Localised messages.
- Changing success field names.

## 10. Test plan

Seam: MCP protocol tests over stdio (`crates/designos-mcp/tests/stdio.rs`) plus engine unit tests.

1. `every_code_has_a_fixture`: table-driven test iterating `ErrorCode::ALL`; each entry has a
   trigger payload; the test asserts the code, a non-empty `next_step`, and, when
   `corrected_example` is present, that sending it succeeds (or fails with a different code for
   chained cases explicitly listed).
2. `safe_to_retry_false_only_when_state_changed`: for each fixture, compare revision, undo depth,
   edit-log length and ledger length before/after; assert `safe_to_retry == unchanged`.
3. `open_file_announce_failure_is_not_retry_safe`: point the edit log at a read-only directory; the
   call fails with `EDITOR_NOT_NOTIFIED` and `safe_to_retry: false`.
4. `tools_list_validates_against_mcp_2025_11_25`: vendored schema file
   `crates/designos-mcp/tests/fixtures/mcp-schema-2025-11-25.json` (from the MCP specification
   repository, MIT licence, attribution header); validate the `tools/list` result with the
   `jsonschema` dev-dependency.
5. `unknown_spec_field_is_named`: `{"kind":"rectangle","corner_radus":4}` → `UNKNOWN_FIELD`,
   `valid_options[0] == "corner_radius"`.

Fail-before: test 5 fails today (field ignored); test 4 fails (no `outputSchema`).

## 11. Performance budgets

Error construction is off the hot path. Levenshtein ranking caps candidates at 10,000 names and stops
at distance 4; under 2 ms per error. Revision tuple capture is O(1).

## 12. Dependencies and build order

After E15-4 (batch errors nest). Before E15-6 (needs the envelope and ledger length in the tuple) and
E15-15 (rules pack quotes codes).

## 13. Owner rulings

- Full scope: every tool gets `outputSchema` and catalogued errors in this task, not a subset.
- All AI through the user's agent CLI: errors are written for the agent CLIs (`claude`, `codex`,
  `agy`, `grok`) that consume MCP results.
