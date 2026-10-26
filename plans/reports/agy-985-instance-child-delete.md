# Report — DEV · `designos-mcp` · Refuse Deleting a Layer Inside an Instance (`#985`, `v0.7.1`)

## Epistemic Ledger (`FACT` / `INFERENCE` / `ASSUMPTION`) & Skills Loaded

### Skills Loaded & Applied
- **`es:dev` -> `es:feature-dev`** (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`): Executed the DEV phase directly against the accepted specification, starting with Red-first failing tests on unmodified `f5644046` (`red.log`) before writing the implementation and capturing the Green pass (`green.log`).
- **`es:rust`** (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`): Kept `enclosing_instance_for_delete` zero-allocation on the walk path (borrowing `&Document` and walking parent `NodeId`s in `O(depth)`), and simplified the slot-container early return in `crates/designos-mcp/src/tools/errors.rs` (`if designos_doc::is_slot_container(doc, anc_id) { return nearest_instance; }`) to satisfy `clippy::question_mark` under `-D warnings`.
- **`es:rust-gpu-canvas`** (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`): Loaded per repository contract; no GPU/canvas files were touched because this task is scoped strictly to `designos-mcp`.
- **`es:astra-work`** & **`es:fable-thinking`** (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`): Applied falsification-first analysis of slot-content marking (`is_slot_container`, `OverrideField::SlotContent`, `ComponentLink::is_slot_content`, `mark_slot_content_subtree`) and `ErrorCode::ALL` fixture coverage before mutating code.

### Epistemic Ledger
- **`FACT` (`crates/designos-app/src/ops.rs:565-568`):** Studio's `delete_selection` refuses deleting any node inside a component instance where `!in_slot && is_linked(n) && !is_top_instance(n)` (with `in_slot = parent.is_some_and(|p| is_slot_container(&self.doc, p))`).
- **`FACT` (`crates/designos-doc/src/components.rs:186-254`, `crates/designos-doc/src/spec_instance.rs:1744-1752`, `crates/designos-doc/src/writer_command.rs:43-50`):**
  - Instance slot containers satisfy `designos_doc::is_slot_container(doc, slot_id)` and record `OverrideField::SlotContent` in `slot.component.overrides` when edited via `designos_apply_spec`, `designos_create_node`, `designos_delete_node`, or `insert_into_slot`.
  - Root nodes inserted into a slot via `slots: { ... }` (`apply_instance_slots_with_ctx`), `insert_into_slot`, or `convert_to_slot` also carry `node.component.is_slot_content = true`.
  - Non-instance subtrees added inside a slot container have `component.source == None` (`!is_linked(n)`) and reach the slot container (`is_slot_container(doc, anc_id)`) before any `NodeKind::Instance` ancestor.
  - Conversely, a child *inside* a component `Instance` that was placed into a slot has that slot-added `Instance` (`is_top_instance(anc) == true`) as its nearest enclosing `Instance` ancestor before the slot container, so the slot-added instance root itself is deletable while internal layers of that slot-added instance remain protected.
- **`FACT` (`crates/designos-mcp/src/rules.rs:500-513`, `crates/designos-mcp/src/server.rs:2635-2638`):** `ErrorCode::InvalidArguments` (`"INVALID_ARGUMENTS"`) is already documented in `rules.rs` for both `designos_delete_node` and `designos_apply_batch` (where batch operation failures wrap the inner cause in `ErrorCode::BatchOpFailed`), and `server.rs::tests::every_code_has_a_fixture` asserts exhaustive coverage of `ErrorCode::ALL` inside `server.rs`.
- **`ASSUMPTION`:** Reused the documented `ErrorCode::InvalidArguments` (`"INVALID_ARGUMENTS"`) code (wrapped in `ErrorCode::BatchOpFailed` for `designos_apply_batch`) and resolved `{instance}` in `{name} is inside the instance {instance}. Hide it or set an override instead.` to the owning top-level instance root (`is_top_instance`, matching `designos_doc::instance_root_of` and stopping at slot boundaries so a child inside a slot-added instance names the slot-added instance).

---

## 1. What Could NOT Be Verified

- Live interactive GUI deletion inside the running macOS desktop app window was not manually clicked in this headless worker session (`designos-app` was out of scope and untouched; MCP engine and batch execution were verified directly through `cargo test -p designos-mcp --locked` and `cargo test -p designos-eval --locked`).

---

## 2. Acceptance Table

| # | Check | Command | Result |
|---|---|---|---|
| 1a | Red first on `f5644046` (`red.log`): direct child, grandchild in nested instance, batch atomicity, slot-added content, instance root | `cargo test -p designos-mcp --test instance_child_delete --locked` (on `f5644046` with only the test file added) | **Failed as expected** (`1 passed; 4 failed` — recorded in `plans/reports/agy-985-instance-child-delete-evidence/red.log`) |
| 1b | Green on head (`green.log`): all 5 test cases pass | `cargo test -p designos-mcp --test instance_child_delete --locked` | **Passed** (`5 passed; 0 failed` — recorded in `plans/reports/agy-985-instance-child-delete-evidence/green.log`) |
| 2a | Formatting check | `cargo fmt --all -- --check` | **Exit 0** |
| 2b | Workspace Clippy gate | `cargo clippy --workspace --all-targets --locked -- -D warnings` | **Exit 0** |
| 3a | Full `designos-mcp` test suite | `cargo test -p designos-mcp --locked` | **Exit 0** (`245` unit tests + all integration test suites passed) |
| 3b | Full `designos-eval` test suite | `cargo test -p designos-eval --locked` | **Exit 0** (`1` unit test + `16` harness tests + `8` real_engine tests passed) |
| 4a | Clean `out/` directory | `rm -rf out && test ! -e out` | **Exit 0** |
| 4b | Open draft PR into `release/v0.6.2` with `Fixes #985` | `gh pr create --draft --base release/v0.6.2 --head jangtrinh/agy-985-instance-child-delete ...` | **Created draft PR [#1130](https://github.com/jangtrinh/design-os-studio/pull/1130)** |

---

## 3. Changes Per File

- [`crates/designos-mcp/src/tools/errors.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-985-instance-child-delete/crates/designos-mcp/src/tools/errors.rs#L756-L816):
  - Added `enclosing_instance_for_delete(doc: &Document, id: NodeId) -> Option<NodeId>` to detect when `id` sits inside a component instance at any depth (including nested instances and nested instances inside slots) while allowing top-level instance roots, main-component children, and slot content (`is_slot_container(doc, parent)`, unlinked `is_slot_content` nodes, and non-instance subtrees inside a slot container).
  - Added `check_not_instance_child(doc: &Document, id: NodeId, path: &str) -> Option<ToolError>` returning `ErrorCode::InvalidArguments` with cause-first message `"{node_name} is inside the instance {inst_name}. Hide it or set an override instead."`, `next_step: "Hide it or set an override instead."`, and `received: id.0`.
- [`crates/designos-mcp/src/engine.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-985-instance-child-delete/crates/designos-mcp/src/engine.rs#L528-L530):
  - In `"designos_delete_node"`, called `crate::tools::errors::check_not_instance_child(&self.doc, id, "/node_id")` before executing `Command::Remove { id }`.
- [`crates/designos-mcp/src/tools/batch.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-985-instance-child-delete/crates/designos-mcp/src/tools/batch.rs#L1715-L1717):
  - In `ParsedOp::Delete`, called `crate::tools::errors::check_not_instance_child(&trial, id, &op_path)` and wrapped any refusal in `ToolError::batch_op_failed(op_idx, op_name, cause)` so the entire batch aborts atomically without modifying `engine.doc`.
- [`crates/designos-mcp/tests/instance_child_delete.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-985-instance-child-delete/crates/designos-mcp/tests/instance_child_delete.rs#L1-L459):
  - Added 5 integration tests covering direct instance children, nested instances and grandchildren in nested instances, atomic batch failure and rollback, slot-added content (via `slots` in `designos_apply_spec`, nested frames/text in slots, slot-added component instances, and `designos_create_node` in slots) across both `designos_delete_node` and `designos_apply_batch`, and deleting instance roots and main-component children.
- `plans/reports/agy-985-instance-child-delete-evidence/red.log` & `plans/reports/agy-985-instance-child-delete-evidence/green.log`:
  - Recorded the exact Red and Green test outputs.

---

## 4. Every User-Facing String, Verbatim

- Error message (`ToolError::message`):
  - `"{node_name} is inside the instance {inst_name}. Hide it or set an override instead."`
  - Fallback when `node.name` is empty: `"node {id}"`
  - Fallback when `inst.name` is empty: `"node {inst_id}"`
- Error next step (`ToolError::next_step`):
  - `"Hide it or set an override instead."`
- Batch wrapper message (`ToolError::batch_op_failed` existing format with the new cause message):
  - `"op #{failed_op} (delete) failed: {node_name} is inside the instance {inst_name}. Hide it or set an override instead."`

---

## 5. Exact Commands and Pasted Output

### 5.1 Red Test Pass on `f5644046` (`plans/reports/agy-985-instance-child-delete-evidence/red.log`)

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test instance_child_delete --locked 2>&1 | tee plans/reports/agy-985-instance-child-delete-evidence/red.log
```

Output:
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-985-instance-child-delete/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-985-instance-child-delete/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-985-instance-child-delete/crates/designos-mcp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 5.33s
     Running tests/instance_child_delete.rs (out/cargo/debug/deps/instance_child_delete-9d44b443ae810025)

running 5 tests
test delete_node_refuses_direct_child_of_instance ... FAILED
test delete_node_refuses_nested_instance_and_grandchild_in_nested_instance ... FAILED
test batch_delete_refuses_instance_child_and_nested_grandchild_atomically ... FAILED
test slot_added_content_stays_deletable_in_delete_node_and_batch ... FAILED
test instance_root_and_main_component_children_stay_deletable ... ok

failures:

---- delete_node_refuses_direct_child_of_instance stdout ----

thread 'delete_node_refuses_direct_child_of_instance' (50351952) panicked at crates/designos-mcp/tests/instance_child_delete.rs:107:10:
deleting a direct child of an instance must be refused: Object {"deleted": Bool(true), "issues": Array [], "issues_total": Number(0), "issues_truncated": Bool(false), "node_id": Number(11)}
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- delete_node_refuses_nested_instance_and_grandchild_in_nested_instance stdout ----

thread 'delete_node_refuses_nested_instance_and_grandchild_in_nested_instance' (50351953) panicked at crates/designos-mcp/tests/instance_child_delete.rs:139:10:
deleting a grandchild inside a nested instance must be refused: Object {"deleted": Bool(true), "issues": Array [], "issues_total": Number(0), "issues_truncated": Bool(false), "node_id": Number(10)}

---- batch_delete_refuses_instance_child_and_nested_grandchild_atomically stdout ----

thread 'batch_delete_refuses_instance_child_and_nested_grandchild_atomically' (50351951) panicked at crates/designos-mcp/tests/instance_child_delete.rs:189:10:
batch containing an instance-child delete must fail as a whole: Object {"changed": Object {"deleted": Number(1), "inserted": Number(1), "moved": Number(0), "styles": Number(0), "updated": Number(1), "variables": Number(0)}, "created": Array [Number(12)], "created_count": Number(1), "dry_run": Bool(false), "issues": Array [], "issues_total": Number(0), "issues_truncated": Bool(false), "name_to_id": Object {"extra": Number(12)}, "touched_bounds": Object {"height": Number(116.0), "width": Number(400.0), "x": Number(0.0), "y": Number(0.0)}, "touched_roots": Array [Object {"bounds": Object {"height": Number(100.0), "width": Number(100.0), "x": Number(0.0), "y": Number(0.0)}, "node_id": Number(12), "op": Number(0)}, Object {"bounds": Object {"height": Number(0.0), "width": Number(0.0), "x": Number(320.0), "y": Number(100.0)}, "node_id": Number(8), "op": Number(1)}, Object {"bounds": Object {"height": Number(16.0), "width": Number(60.0), "x": Number(340.0), "y": Number(100.0)}, "node_id": Number(8), "op": Number(2)}], "undo_label": String("Agent: designos-mcp")}

---- slot_added_content_stays_deletable_in_delete_node_and_batch stdout ----

thread 'slot_added_content_stays_deletable_in_delete_node_and_batch' (50351955) panicked at crates/designos-mcp/tests/instance_child_delete.rs:320:10:
slot container itself is an instance child and cannot be deleted: Object {"deleted": Bool(true), "issues": Array [], "issues_total": Number(0), "issues_truncated": Bool(false), "node_id": Number(18)}


failures:
    batch_delete_refuses_instance_child_and_nested_grandchild_atomically
    delete_node_refuses_direct_child_of_instance
    delete_node_refuses_nested_instance_and_grandchild_in_nested_instance
    slot_added_content_stays_deletable_in_delete_node_and_batch

test result: FAILED. 1 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

error: test failed, to rerun pass `-p designos-mcp --test instance_child_delete`
```

### 5.2 Green Test Pass on Head (`plans/reports/agy-985-instance-child-delete-evidence/green.log`)

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test instance_child_delete --locked 2>&1 | tee plans/reports/agy-985-instance-child-delete-evidence/green.log
```

Output:
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-985-instance-child-delete/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-985-instance-child-delete/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-985-instance-child-delete/crates/designos-mcp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.91s
     Running tests/instance_child_delete.rs (out/cargo/debug/deps/instance_child_delete-9d44b443ae810025)

running 5 tests
test delete_node_refuses_nested_instance_and_grandchild_in_nested_instance ... ok
test delete_node_refuses_direct_child_of_instance ... ok
test batch_delete_refuses_instance_child_and_nested_grandchild_atomically ... ok
test instance_root_and_main_component_children_stay_deletable ... ok
test slot_added_content_stays_deletable_in_delete_node_and_batch ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

### 5.3 `cargo fmt --all -- --check` & `cargo clippy --workspace --all-targets --locked -- -D warnings`

Command:
```sh
cargo fmt --all -- --check
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings
```

Output:
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-985-instance-child-delete/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-985-instance-child-delete/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-985-instance-child-delete/crates/designos-mcp)
    Checking designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-985-instance-child-delete/crates/designos-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 25.85s
FMT=0 CLIPPY=0
```

### 5.4 `cargo test -p designos-mcp --locked` & `cargo test -p designos-eval --locked`

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --locked
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-eval --locked
```

Output:
```text
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_mcp-96fd55246d4a3a66)
test result: ok. 245 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.90s

     Running tests/colour_detach.rs (out/cargo/debug/deps/colour_detach-95308d471d611a08)
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

     Running tests/colour_var_library.rs (out/cargo/debug/deps/colour_var_library-777bb8c5ef476570)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

     Running tests/colour_var_refs.rs (out/cargo/debug/deps/colour_var_refs-f5ff84c7c93e3c20)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

     Running tests/evaluation.xml (out/cargo/debug/deps/evaluation-...)
     Running tests/instance_child_delete.rs (out/cargo/debug/deps/instance_child_delete-9d44b443ae810025)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/list_lint_rules.rs (out/cargo/debug/deps/list_lint_rules-...)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/live_doc_epoch.rs (out/cargo/debug/deps/live_doc_epoch-...)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running tests/live_session_guards.rs (out/cargo/debug/deps/live_session_guards-...)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running tests/mcp_paths.rs (out/cargo/debug/deps/mcp_paths-...)
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s

     Running tests/mcp_paths_full_disk.rs (out/cargo/debug/deps/mcp_paths_full_disk-...)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/session.rs (out/cargo/debug/deps/session-...)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s

     Running tests/session_file_sites.rs (out/cargo/debug/deps/session_file_sites-...)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running tests/session_files.rs (out/cargo/debug/deps/session_files-...)
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s

     Running tests/spec_text_colour.rs (out/cargo/debug/deps/spec_text_colour-75b28c66cba35b01)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

     Running tests/stdio.rs (out/cargo/debug/deps/stdio-9963b9496c82d7e4)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

     Running tests/text_fill.rs (out/cargo/debug/deps/text_fill-b3d88a68a195ca3e)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

     Running tests/tools_snapshot.rs (out/cargo/debug/deps/tools_snapshot-e6488781337fbb09)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s

     Running tests/version.rs (out/cargo/debug/deps/version-7878cb959d478560)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running unittests src/lib.rs (out/cargo/debug/deps/designos_eval-9a9f1f166d66684f)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/harness.rs (out/cargo/debug/deps/harness-28beba16668beb59)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.13s

     Running tests/real_engine.rs (out/cargo/debug/deps/real_engine-4a6cab08ce582fe8)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
```

Status: DONE
