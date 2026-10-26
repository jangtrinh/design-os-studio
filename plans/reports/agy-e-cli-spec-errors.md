# CLI & MCP Spec Errors + Ask Scope Note (`#1003` RT-04/RT-07, `#1101` docs)

## 1) What could NOT be verified

- **Ambiguous untagged enum field attribution when multiple fields fail simultaneously or when a raw string fails without a `Value` payload**: When `serde_json` rejects an `#[serde(untagged)]` enum (`NumberOrVar`, `BoolOrVar`, or `InProgressSpec`), `serde_json::Error` emits `data did not match any variant of untagged enum <EnumName> at line L column C` without naming the struct field in the error message. When the raw JSON `serde_json::Value` is available (both CLI `--apply-spec` via `cli::parse_spec_json` and MCP `designos_apply_spec` via `engine::parse_spec` / `from_serde_error_with_args`), `describe_spec_value_error` walks the `NodeSpec` JSON tree in field order and names the exact field and what it must be. The cases that cannot be disambiguated without a custom `Deserialize` implementation or a path-tracking deserializer crate (`serde_path_to_error`, excluded by the no-new-crate constraint) are:
  1. Calling `from_serde_error(&err, path, true)` directly without passing `Some(&raw_args)` when an untagged enum fails (the fallback message still replaces the Rust enum name `NumberOrVar` / `BoolOrVar` / `InProgressSpec` with plain words, e.g. `A numeric spec field must be a number or a "var:..." string.`, but cannot name which numeric field failed without the `Value`).
  2. When a spec contains multiple invalid fields at once, `describe_spec_value_error` reports the first invalid field encountered in deterministic tree order rather than all invalid fields simultaneously.
  3. Deeply nested fields inside free-form sub-objects (`fill`, `stroke`, `strokes`, `effects`, `layout_grids`, `VectorPathSpec`, `arc_data`, `blend_mode`, `mask_type`, `boolean_op`, `overflow`, `ExportSetting`) where `serde`'s error message does not name a Rust primitive/struct type (`u64`, `u32`, `u16`, `f32`, `NodeSpec`, `LayoutSpec`, `OverrideSpec`, `InProgressSpec`, `NumberOrVar`, `BoolOrVar`, `tuple of size 4`): those retain `format_spec_serde_error` / `clean_serde_message` output, which translates enum variant lists and unknown-field suggestions into plain words without naming the parent field path (#920 is scoped to 0.6.3).
- **Windows / Linux native binary execution of `--screenshot`**: Verified on macOS (`aarch64-apple-darwin`); Ubuntu and Windows CI runners were not executed locally.

---

## 2) Acceptance table (1–6)

| # | Acceptance Check | Status | Command & Pasted Output |
|---|---|---|---|
| 1 | **Red first:** write the failing tests (`cli_screenshot_refuses_broken_files_and_captures_good_file`, `cli_apply_spec_reports_field_and_expected_type_in_plain_words`, `apply_spec_reports_field_and_expected_type_in_plain_words`), run them on `f5644046`, and save the failure output to `plans/reports/agy-e-cli-spec-errors-evidence/red.log`. | PASS (failed as expected on `f5644046`) | Saved at `plans/reports/agy-e-cli-spec-errors-evidence/red.log` (see excerpt below). |
| 2 | **Green:** make the fix, re-run the same tests, and save the pass output to `plans/reports/agy-e-cli-spec-errors-evidence/green.log`. | PASS | Saved at `plans/reports/agy-e-cli-spec-errors-evidence/green.log` (see excerpt below). |
| 3 | `cargo run -p designos-mcp --bin gen-tool-docs` (or the repo's generator) leaves no diff after regenerating `generated/rules.md`, and the rules drift test passes. | PASS | `cargo run -p designos-mcp --bin gen-tool-docs --locked` + `rules::tests::drift_check_fails_on_schema_change ... ok` and `rules::tests::rules_under_token_budget ... ok` (see output below). |
| 4 | `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --locked -- -D warnings` exit 0. | PASS | Both exited `0` with zero warnings (see output below). |
| 5 | `cargo test -p designos-mcp --locked`, `cargo test -p designos-eval --locked`, and `cargo test -p designos-app --bin designos --locked cli` pass. | PASS | All three test suites exited `0` (see output below). |
| 6 | Remove `out/` before finishing. `git status -sb` shows only the files you mean to commit, your report, and the evidence folder. | PASS | `rm -rf out` run; `git status -sb` verified clean (see output below). |

### Pasted Output — Acceptance 1 (`plans/reports/agy-e-cli-spec-errors-evidence/red.log`)

```text
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test spec_errors --locked -- --nocapture
     Running tests/spec_errors.rs (out/cargo/debug/deps/spec_errors-9f07075765b87406)

running 1 test

thread 'apply_spec_reports_field_and_expected_type_in_plain_words' (49547910) panicked at crates/designos-mcp/tests/spec_errors.rs:47:9:
for {"name":"Card","children":[[{"kind":"rectangle","name":"Box"}]]}, expected "children must be an array of NodeSpec objects" in "Invalid arguments: invalid type: map, expected u64"
test apply_spec_reports_field_and_expected_type_in_plain_words ... FAILED

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked cli_screenshot_refuses_broken_files_and_captures_good_file -- --nocapture
running 1 test
thread 'icon_tests::cli_screenshot_refuses_broken_files_and_captures_good_file' (49553129) panicked at crates/designos-app/src/main.rs:705:17:
designos trunc.fig --screenshot must exit non-zero, got ExitStatus(unix_wait_status(0))
stderr:
test icon_tests::cli_screenshot_refuses_broken_files_and_captures_good_file ... FAILED

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3279 filtered out; finished in 36.22s

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked cli_apply_spec_reports_field_and_expected_type_in_plain_words -- --nocapture
running 1 test
thread 'icon_tests::cli_apply_spec_reports_field_and_expected_type_in_plain_words' (49555511) panicked at crates/designos-app/src/main.rs:782:13:
for {"name":"Card","children":[[{"kind":"rectangle","name":"Box"}]]}, expected "children must be an array of NodeSpec objects" in "Invalid NodeSpec JSON in /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.R2rC09/designos_rt07_cli_43589/aa.json: invalid type: map, expected u64 at line 1 column 40"
test icon_tests::cli_apply_spec_reports_field_and_expected_type_in_plain_words ... FAILED

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3279 filtered out; finished in 0.00s
```

### Pasted Output — Acceptance 2 (`plans/reports/agy-e-cli-spec-errors-evidence/green.log`)

```text
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test spec_errors --locked -- --nocapture
     Running tests/spec_errors.rs (out/cargo/debug/deps/spec_errors-9f07075765b87406)

running 1 test
test apply_spec_reports_field_and_expected_type_in_plain_words ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked cli -- --nocapture
     Running unittests src/main.rs (out/cargo/debug/deps/designos-d02548a0edc4e955)

running 291 tests
...
test cli::tests::parse_spec_json_names_the_field_and_expected_type ... ok
...
test icon_tests::cli_apply_spec_reports_field_and_expected_type_in_plain_words ... ok
test icon_tests::cli_screenshot_refuses_broken_files_and_captures_good_file ... ok

test result: ok. 291 passed; 0 failed; 0 ignored; 0 measured; 2990 filtered out; finished in 55.73s
```

### Pasted Output — Acceptance 3, 4, 5, 6

```text
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo run -p designos-mcp --bin gen-tool-docs --locked
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 02s
     Running `out/cargo/debug/gen-tool-docs`

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo fmt --all -- --check
(exited with code 0)

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 40.98s
(exited with code 0)

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --locked
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_mcp-96fd55246d4a3a66)
running 245 tests
...
test rules::tests::drift_check_fails_on_schema_change ... ok
test rules::tests::rules_under_token_budget ... ok
...
test result: ok. 245 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.18s
     Running tests/spec_errors.rs (out/cargo/debug/deps/spec_errors-9f07075765b87406)
test apply_spec_reports_field_and_expected_type_in_plain_words ... ok
     Running tests/spec_text_colour.rs (out/cargo/debug/deps/spec_text_colour-60d15eb63d61beaf)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/tools_snapshot.rs (out/cargo/debug/deps/tools_snapshot-5bd50496857690f7)
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-eval --locked
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_eval-8a3f9268eb48d94d)
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

$ rm -rf out && git status -sb
## jangtrinh/agy-e-cli-spec-errors
 M README.md
 M crates/designos-app/src/cli.rs
 M crates/designos-app/src/main.rs
 M crates/designos-mcp/generated/rules.md
 M crates/designos-mcp/rules/workflow.md
 M crates/designos-mcp/src/engine.rs
 M crates/designos-mcp/src/tools/errors.rs
 M skills/design-os-studio/rules.md
?? crates/designos-mcp/tests/spec_errors.rs
?? plans/reports/agy-e-cli-spec-errors-evidence/
```

---

## 3) Changes per file

### Skills Loaded & Skill-Driven Audit (`msg_9ba75475b61f`)
- **Skills loaded**:
  - `es:dev` (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`)
  - `es:feature-dev` (`/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`)
  - `es:rust` (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`)
  - `es:rust-gpu-canvas` (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`)
  - `es:astra-work` (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`)
  - `es:fable-thinking` (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`)
  - `es:orca-native-orchestration` (`/Users/jangtrinh/.gemini/config/skills/es-orca-native-orchestration/SKILL.md`)
- **Changes made after checking code against `/es:rust` and `/es:rust-gpu-canvas`**:
  - Eliminated `args.clone()` in `crates/designos-mcp/src/engine.rs` (`parse` and `parse_spec`) by deserializing directly from `&args` via `T::deserialize(&args)` (`serde::Deserialize` on `&serde_json::Value`), avoiding a full `serde_json::Value` tree clone on every MCP tool call and `designos_apply_spec` invocation.
  - Eliminated double JSON parsing in `crates/designos-app/src/cli.rs` (`parse_spec_json`) by parsing `raw: &str` once into `serde_json::Value` and deserializing `NodeSpec` from `&val` (`<designos_doc::NodeSpec as serde::Deserialize>::deserialize(&val)`) instead of calling `serde_json::from_str` twice.
  - Verified zero per-frame allocations or document walks, preserved `egui` lock discipline (`ctx.load_texture` never called inside `ctx.data_mut`), and left `designos-gpu` `interactive()` guards untouched.

### Per-File Summary
- **`crates/designos-app/src/main.rs`**:
  - In `main()`, when `opts.screenshot.is_some()` and `opts.file` is `Some(path)`, calls `screenshot_file_error(path)` before `spawn_startup_sweep()` and `eframe::run_native(...)`. If the file cannot be loaded (and is not a comments or variables JSON payload), prints `{file} could not be opened: {reason}. Check the file, then try again.` to stderr and exits `1` without creating a window or writing a PNG.
  - In `run_agent_cli`, routed `--apply-spec` parsing through `cli::parse_spec_json(&raw)`.
  - Added integration tests `icon_tests::cli_screenshot_refuses_broken_files_and_captures_good_file` (testing both `designos` and `designos-studio` on `trunc.fig`, `corrupt.designos`, `garbage.fig`, plus a `good.designos` control) and `icon_tests::cli_apply_spec_reports_field_and_expected_type_in_plain_words`.
- **`crates/designos-app/src/cli.rs`**:
  - Added `parse_spec_json`, `describe_spec_value_error` (plus recursive helpers for `NodeSpec`, `LayoutSpec`, `OverrideSpec`, `in_progress`, `slots`, and `comments`), and `humanize_spec_serde_error` so `--apply-spec` errors name the offending spec field and what it must be in plain words instead of serde's Rust type names (`u64`, `u32`, `u16`, `f32`, `NumberOrVar`, `BoolOrVar`, `InProgressSpec`, `struct NodeSpec`, `struct LayoutSpec`, `struct OverrideSpec`, `tuple of size 4`).
  - Added unit test `cli::tests::parse_spec_json_names_the_field_and_expected_type`.
- **`crates/designos-mcp/src/engine.rs`**:
  - Updated `parse` and `parse_spec` to deserialize from `&args` without cloning `args`, pass `Some(&args)` to `from_serde_error_with_args`, and check `describe_spec_args_error(&args)` in `parse_spec` so structural spec errors (such as `children: [[]]`, `layout: []`, `overrides: [[]]`, `comments: [[]]`, which `serde`'s generated `visit_seq` would otherwise silently accept as default structs) are rejected with plain-language field messages while keeping `path` unchanged (`""`).
- **`crates/designos-mcp/src/tools/errors.rs`**:
  - Added `describe_spec_args_error`, `describe_spec_value_error` (and helpers), and `humanize_spec_serde_message`, wired into `from_serde_error_with_args` when `is_spec` is `true` without changing `ToolError.path`.
- **`crates/designos-mcp/tests/spec_errors.rs`**:
  - Added `apply_spec_reports_field_and_expected_type_in_plain_words` testing `designos_apply_spec` across 27 invalid spec payloads and asserting that no Rust type names leak into the error message.
- **`crates/designos-mcp/rules/workflow.md`, `crates/designos-mcp/generated/rules.md`, `skills/design-os-studio/rules.md`**:
  - Added the two-sentence Ask scope note under `## Permissions` in `workflow.md`, tightened wording in steps 1–6 by ~180 characters to stay within `MAX_RULES_CHARS = 24_000` (`rules_under_token_budget`), and regenerated `generated/rules.md` and `skills/design-os-studio/rules.md` via `cargo run -p designos-mcp --bin gen-tool-docs --locked`.
- **`README.md`**:
  - Updated the Agents paragraph to state that Ask covers agent tabs started from Studio's Agents panel, while the `designos` CLI you run yourself and a `designos-mcp` started by another host are not gated by Studio (that host approves its own MCP calls).

---

## 4) Every user-facing string added, verbatim

### CLI `--screenshot` broken-file stderr (`crates/designos-app/src/main.rs`)
- `"{} could not be opened: {reason}. Check the file, then try again."`

### Spec parse & field validation strings (`crates/designos-app/src/cli.rs` and `crates/designos-mcp/src/tools/errors.rs`)
- `"{e}. Check the JSON syntax, then try again."`
- `"spec must be a NodeSpec object. Pass a JSON object with fields such as kind, name, and children."`
- `"children must be an array of NodeSpec objects. Pass a JSON array of node objects."`
- `"children must be an array of NodeSpec objects. Pass each child as a JSON object."`
- `"layout must be a LayoutSpec object. Pass an object such as {\"mode\": \"row\", \"gap\": 8}."`
- `"overrides must be an array of OverrideSpec objects. Pass a JSON array of override objects."`
- `"overrides must be an array of OverrideSpec objects. Pass each override as a JSON object."`
- `"in_progress must be a boolean or an in_progress object. Pass true, false, or an object with status, label, note, or agent."`
- `"{k} must be a string. Pass a string value."`
- `"{k} must be a number or a \"var:...\" string. Pass a numeric value or a variable reference."`
- `"opacity must be a number between 0 and 1 or a \"var:...\" string. Pass a numeric opacity such as 0.8 or a variable reference."`
- `"{k} must be an array of 4 numbers or \"var:...\" strings. Pass [top_left, top_right, bottom_right, bottom_left]."`
- `"padding must be an array of 4 numbers or \"var:...\" strings. Pass [top, right, bottom, left]."`
- `"{k} must be a boolean or a \"var:...\" string. Pass true, false, or a variable reference."`
- `"id must be a non-negative integer. Pass a numeric node ID."`
- `"font_weight must be a number between 100 and 900. Pass a numeric weight such as 400 or 600."`
- `"{k} must be a positive integer. Pass a whole number such as 1 or 2."`
- `"grid_columns must be a positive integer. Pass a column count such as 2 or 3."`
- `"{k} must be a number. Pass a numeric value."`
- `"{k} must be a boolean. Pass true or false."`
- `"variant must be an object mapping variant property names to string values. Pass an object such as {\"Size\": \"md\"}."`
- `"mode must be an object mapping variable collection names or IDs to mode names or IDs. Pass an object such as {\"Theme\": \"Dark\"}."`
- `"props must be an object mapping component property names to values. Pass a JSON object."`
- `"annotations must be an array of strings. Pass a JSON array of string notes."`
- `"comments must be an array of comment thread objects. Pass a JSON array of objects with messages."`
- `"comments must be an array of comment thread objects. Pass each thread as a JSON object."`
- `"slots must be an object mapping slot names to arrays of NodeSpec objects. Pass an object such as {\"content\": [...] }."`
- `"slots.{slot_name} must be an array of NodeSpec objects. Pass a JSON array of node objects."`
- `"slots.{slot_name} must be an array of NodeSpec objects. Pass each child as a JSON object."`
- `"in_progress.{k} must be a string. Pass a string value."`
- `"visible must be a boolean. Pass true or false."`
- `"opacity must be a number between 0 and 1. Pass a numeric opacity such as 0.8."`
- `"A numeric spec field must be a number or a \"var:...\" string. Pass a numeric value or a variable reference."`
- `"A boolean spec field must be a boolean or a \"var:...\" string. Pass true, false, or a variable reference."`
- `"4 numbers or \"var:...\" strings"` (replacing `"tuple of size 4"` in fallback serde messages)
- `"a LayoutSpec object"` (replacing `"struct LayoutSpec"` in fallback serde messages)
- `"a NodeSpec object"` (replacing `"struct NodeSpec"` in fallback serde messages)
- `"an OverrideSpec object"` (replacing `"struct OverrideSpec"` in fallback serde messages)
- `"a non-negative integer"` (replacing `"u64"` in fallback serde messages)
- `"a positive integer"` (replacing `"u32"` in fallback serde messages)
- `"a number between 100 and 900"` (replacing `"u16"` in fallback serde messages)
- `"a number"` (replacing `"f32"` in fallback serde messages)

### Documentation strings (`crates/designos-mcp/rules/workflow.md`, `crates/designos-mcp/generated/rules.md`, `skills/design-os-studio/rules.md`, `README.md`)
- In `crates/designos-mcp/rules/workflow.md` (and regenerated into `crates/designos-mcp/generated/rules.md` and `skills/design-os-studio/rules.md`):
  `"Ask covers agent tabs started from Studio's Agents panel. The \`designos\` CLI you run yourself and a \`designos-mcp\` started by another host are not gated by Studio; that host approves its own MCP calls."`
- In `README.md`:
  `"Ask covers agent tabs started from Studio's Agents panel; the \`designos\` CLI you run yourself, and a \`designos-mcp\` started by another host, are not gated by Studio — that host approves its own MCP calls."`

---

## 5) Findings outside scope

- **Serde `#[derive(Deserialize)]` `visit_seq` on all-default structs (`NodeSpec`, `LayoutSpec`, `OverrideSpec`, `SpecCommentThread`, `InProgressWire`)**: Because serde's derived struct `Visitor` implements `visit_seq` in addition to `visit_map`, passing an empty JSON array `[]` where a struct object is expected (e.g. `"children": [[]]`, `"layout": []`, `"overrides": [[]]`) silently succeeds in raw `serde_json::from_str::<NodeSpec>` by constructing a default struct with all `None`/empty fields. Our `describe_spec_value_error` / `describe_spec_args_error` validator catches and refuses these in both `--apply-spec` and `designos_apply_spec`.
- **Single-attempt `ViewportCommand::Screenshot` under heavy parallel CPU load on macOS (`crates/designos-app/src/shell.rs`)**: In `Shell::request_screenshot`, `shot.requested` is set to `true` once on frame 12 (CPU mode) and never re-sent if the macOS window server drops that single off-focus (`with_active(false)`) readback while 290+ unit tests are running in parallel, causing the window to close at frame 241 with `designos: screenshot was not captured`. Left `shell.rs` untouched as it is outside the allowed files list.

---

## 6) Assumptions

- For `designos FILE --screenshot OUT.png`, a `.json` file whose contents match `comments::import::is_comments_payload` or `variables::is_variables_payload` is intentionally allowed through `screenshot_file_error` because `Shell::build` handles comments and variables JSON payloads on the default demo document (`crates/designos-app/src/shell.rs:159-169`). All other unreadable `.fig`, `.designos`, `.svg`, or `.json` files are rejected before starting the window.

---

## Round 2

### 1) What could NOT be verified
- **Non-macOS native window close timing under `--screenshot` (`Linux` / `Windows`)**: Verified on macOS (`aarch64-apple-darwin`) both in-process (`cli_screenshot_loads_good_file_once`) and via child processes (`tests/cli_screenshot.rs`); Ubuntu and Windows CI runners were not executed locally.

### Conclusion & Epistemic Ledger
- **Conclusion (FACT)**: Removed the duplicate `screenshot_file_error` pre-load in `crates/designos-app/src/main.rs` so `--screenshot` loads `opts.file` once inside `Shell::build` (`crates/designos-app/src/shell.rs`). When that initial load fails in screenshot mode, `Shell::build` records the formatted message in `shell::SHOT_OPEN_ERROR: OnceLock<String>`, marks `Shot.open_failed = true`, and sends `ViewportCommand::Close`; `Shell::draw` returns immediately on `open_failed` without rendering a blank `Untitled` document or requesting a screenshot, and `main` prints the single error line to stderr and exits `1`. Moved the RT-04 CLI test into `crates/designos-app/tests/cli_screenshot.rs` using `env!("CARGO_BIN_EXE_designos")` and `env!("CARGO_BIN_EXE_designos-studio")` with no nested `cargo build`.
- **Epistemic Ledger**:
  - **FACT**: On `2870e72a`, `cli_screenshot_loads_good_file_once` failed with `left: 2, right: 1` (`plans/reports/agy-e-cli-spec-errors-evidence/r2-red.log`) because `main.rs` called `io::load_path_with_warning` in `screenshot_file_error` before `Shell::build` called `app.open_path`.
  - **FACT**: After removing the pre-load in `main.rs` and handling initial-load failures in `Shell::build` / `Shell::draw`, `cli_screenshot_loads_good_file_once` passes with `load_call_count == 1` (`plans/reports/agy-e-cli-spec-errors-evidence/r2-green.log`), and `crates/designos-app/tests/cli_screenshot.rs` passes for both `designos` and `designos-studio`.
  - **FACT**: In `app.open_path` (`crates/designos-app/src/ops.rs:2143-2182`), every successful document open calls `self.replace_document` -> `self.changed()` (setting `self.canvas.dirty = true`), and `.json` comments/variables payloads set a non-error toast (`toast.error == false`), whereas every failed open leaves `self.canvas.dirty` untouched (`false`) and sets `toast.error = true`.
  - **INFERENCE**: Tracking `open_failed: bool` per `Shot` instance in addition to storing the message in `SHOT_OPEN_ERROR: OnceLock<String>` prevents cross-test interference when multiple `Shell` instances run inside the same unit-test process.
  - **ASSUMPTION**: None added beyond the `.json` comments/variables payload exception preserved from `app.open_path`.
- **Weakest link**: In-process detection of `app.open_path` failure in `Shell::build` relies on `app.open_path` leaving `app.canvas.dirty == false` and setting `app.toast.error == true` on load failure (since `ops.rs` is outside the Round 2 allowed files list).
- **Unresolved questions**: None.

### 2) Acceptance Table (Round 2)

| # | Acceptance Check | Status | Command & Evidence |
|---|---|---|---|
| 1 | `r2-red.log`: the load-once proof fails on `2870e72a`. `r2-green.log`: it passes on your head, and so does the moved RT-04 test. | PASS | Saved at `plans/reports/agy-e-cli-spec-errors-evidence/r2-red.log` (`left: 2, right: 1`) and `plans/reports/agy-e-cli-spec-errors-evidence/r2-green.log` (both tests `ok`). |
| 2 | `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --locked -- -D warnings` exit 0. | PASS | Both exited `0` under the test harness (see output below). |
| 3 | `cargo test -p designos-app --test cli_screenshot --locked` and `cargo test -p designos-app --bin designos --locked cli` pass under the harness. | PASS | `cli_screenshot` (1 passed) and `--bin designos cli` (291 passed, 0 failed) exited `0` (see output below). |
| 4 | Commit as `fix(cli): …` (no AI or tool names), push, and add a "Round 2" line to the PR body. Keep the PR a draft, never merge, and delete `out/`. | PASS | Committed, pushed, PR #1117 body updated with Round 2 line, `out/` deleted. |

### 3) Changes per file (Round 2)

#### Skills Loaded & Changes Because of Them
- **Skills loaded**:
  - `/es:dev` (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`)
  - `/es:feature-dev` (`/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`)
  - `/es:rust` (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`)
  - `/es:rust-gpu-canvas` (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`)
  - `/es:fable-thinking` (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`)
  - `/es:astra-work` (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`)
- **What changed because of the loaded skills**:
  - Per `/es:rust` (eliminating duplicate full-document deserialization and 2x peak memory on 1M-record `.fig` files), removed `screenshot_file_error` from `main.rs` so `--screenshot` loads `opts.file` once through `Shell::build`.
  - Per `/es:rust-gpu-canvas` & `/es:fable-thinking` adversarial verification, added `open_failed: bool` on `Shot` so `Shell::draw` returns before `draw_window` and `request_screenshot` (preventing any blank `Untitled` frame compilation/rendering on broken files and avoiding cross-test `OnceLock` state leakage in unit tests).
  - Per `/es:rust` testing discipline, counted calls to `io::load_path_with_warning` per file path in `#[cfg(test)]` via `mod io` in `main.rs` (zero runtime cost in release builds, zero edits to `io.rs`) and moved the CLI binary test to `crates/designos-app/tests/cli_screenshot.rs` using `env!("CARGO_BIN_EXE_designos")` and `env!("CARGO_BIN_EXE_designos-studio")`.

#### Per-File Summary
- **`crates/designos-app/src/main.rs`**:
  - Removed `screenshot_file_error` and the pre-window load block before `agent_session_files::spawn_startup_sweep()`.
  - After `eframe::run_native`, checks `shell::SHOT_OPEN_ERROR.get()` first when `screenshot_mode` is true, printing `{err}` to stderr and exiting `1` before the `SHOT_OK` fallback check.
  - Wrapped `mod io` with a `#[cfg(test)]` per-path call counter on `io::load_path_with_warning` (`io::load_call_count`) — the cheapest honest proof that counts real calls to the loader in a unit test without touching `io.rs` or `ops.rs`.
  - Deleted `build_cli_binaries`, `run_cli_bin`, and `cli_screenshot_refuses_broken_files_and_captures_good_file` from `mod icon_tests`, and added `cli_screenshot_loads_good_file_once`.
- **`crates/designos-app/src/shell.rs`**:
  - Added `pub static SHOT_OPEN_ERROR: std::sync::OnceLock<String> = std::sync::OnceLock::new();` next to `SHOT_OK`.
  - In `Shell::build`, when `opts.file` is `Some(f)` and `opts.screenshot.is_some()`, detects initial-load failure after `app.open_path(f.clone())` (preserving the `.json` comments/variables payload exception in `app.open_path`), records `"{} could not be opened: {reason}. Check the file, then try again."` in `SHOT_OPEN_ERROR`, marks `Shot.open_failed = true`, and sends `egui::ViewportCommand::Close`.
  - In `Shell::draw`, when `self.shot.as_ref().is_some_and(|s| s.open_failed)` is true, sends `egui::ViewportCommand::Close` and returns immediately before `draw_window` and `request_screenshot`.
- **`crates/designos-app/tests/cli_screenshot.rs`**:
  - Added integration test `cli_screenshot_refuses_broken_files_and_captures_good_file` using `env!("CARGO_BIN_EXE_designos")` and `env!("CARGO_BIN_EXE_designos-studio")`, 30 s timeout in `run_cli_bin`, the three broken files (`trunc.fig`, `corrupt.designos`, `garbage.fig`), the single-line stderr check, and the `good.designos` control.

### Exact Commands & Pasted Output (Round 2)

#### `plans/reports/agy-e-cli-spec-errors-evidence/r2-red.log`
```text
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked cli_screenshot_loads_good_file_once -- --nocapture
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-mcp)
   Compiling designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 53.34s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-d02548a0edc4e955)

running 1 test

thread 'icon_tests::cli_screenshot_loads_good_file_once' (50393459) panicked at crates/designos-app/src/main.rs:892:9:
assertion `left == right` failed: --screenshot must load /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.jIvcQN/designos_rt04_load_once_99620/good.designos exactly once
  left: 2
 right: 1
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
test icon_tests::cli_screenshot_loads_good_file_once ... FAILED

failures:

failures:
    icon_tests::cli_screenshot_loads_good_file_once

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3281 filtered out; finished in 0.06s

error: test failed, to rerun pass `-p designos-app --bin designos`
```

#### `plans/reports/agy-e-cli-spec-errors-evidence/r2-green.log`
```text
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked cli_screenshot_loads_good_file_once -- --nocapture
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-mcp)
   Compiling designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 23.16s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-d02548a0edc4e955)

running 1 test
test icon_tests::cli_screenshot_loads_good_file_once ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3280 filtered out; finished in 0.03s

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --test cli_screenshot --locked -- --nocapture
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-mcp)
   Compiling designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 18.19s
     Running tests/cli_screenshot.rs (out/cargo/debug/deps/cli_screenshot-71d1269d723c30fb)

running 1 test
test cli_screenshot_refuses_broken_files_and_captures_good_file ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.54s
```

#### Acceptance 2 & 3 Commands & Output
```text
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo fmt --all -- --check
(exited with code 0)

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 42.11s
(exited with code 0)

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --test cli_screenshot --locked
     Running tests/cli_screenshot.rs (out/cargo/debug/deps/cli_screenshot-71d1269d723c30fb)

running 1 test
test cli_screenshot_refuses_broken_files_and_captures_good_file ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.54s

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked cli
     Running unittests src/main.rs (out/cargo/debug/deps/designos-d02548a0edc4e955)

running 291 tests
...
test cli::tests::parse_spec_json_names_the_field_and_expected_type ... ok
test icon_tests::agent_cli_apply_comment_and_export_roundtrip ... ok
test icon_tests::cli_apply_spec_reports_field_and_expected_type_in_plain_words ... ok
test icon_tests::cli_screenshot_loads_good_file_once ... ok
test icon_tests::web_capture_cli_import_and_out_roundtrip ... ok
...
test result: ok. 291 passed; 0 failed; 0 ignored; 0 measured; 2990 filtered out; finished in 22.60s
```

---

## Round 3

### 1) What could NOT be verified
- **Non-macOS window server behavior under `--screenshot` (`Ubuntu` / `Windows`)**: All headless MCP, CLI, and screenshot tests were executed on macOS (`aarch64-apple-darwin`).

### Conclusion & Epistemic Ledger
- **Conclusion (FACT)**: Addressed all Round 3 rulings (`F1`–`F7`, real open result `App::try_open_path`, and `io::load_call_count` test seam):
  1. **F1 (`set_slot` errors name real cause)**: `describe_spec_args_error` in `crates/designos-mcp/src/tools/errors.rs` no longer falls back to `describe_spec_value_error(raw_args)`. When `op == "set_slot"`, it validates `node`/`node_id`/`instance`, `slot`, `as`, and walks `fill`/`value` via `describe_slot_fill_error`, reporting `"slot must be a string. Pass a slot name string."` on `slot: 5`, `"fill/0 must be a NodeSpec object. Pass each slot child as a JSON object."` on `fill: [[...]]`, and `"fill/0/width must be a number or a \"var:...\" string. Pass a number in pixels or a variable reference."` on a bad child field inside `fill`.
  2. **F2 (say where)**: Threaded `prefix: &str` (`join_spec_path`) through `describe_spec_value_error_at`, `describe_layout_spec_error`, `describe_slot_fill_error`, `describe_slots_error`, `describe_override_spec_error`, `describe_in_progress_error`, and `describe_comments_error` so nested errors carry the full path from the spec root (e.g. `children/0/layout/padding must be ...`).
  3. **F3 (one parser)**: Deleted the duplicated spec-error walk (~378 lines) from `crates/designos-app/src/cli.rs` and delegated `cli::parse_spec_json` to `designos_mcp::tools::errors::parse_spec_json`.
  4. **F4 (never refuse what serde accepts, with two stated exceptions)**:
     - Exception (a) (positional array where an object is expected: `children: [[...]]`, `layout: []`, `overrides: [[]]`, `in_progress: []`, `comments: [[]]`) and Exception (b) (number that becomes non-finite as `f32`, e.g. `x: 1e40` or `width: 1e40`, checked via `is_non_finite_f32_number`) are both refused with clear messages (`"{field_path} must be a finite number. Pass a smaller number."` / `"{field_path} must contain finite numbers. Pass smaller numbers."`).
     - Removed the false rejects on structured `Comment` `author` (`RawAuthor::Structured` in `crates/designos-doc/src/comments.rs:137`) and the invented `created_at` check; added full path-prefixed validation for `Comment` message sub-fields (`id`, `author`, `author_id`, `author_color`, `text`, `created`, `edited` in `RawComment`, `crates/designos-doc/src/comments.rs:213`) and `Color` (`crates/designos-doc/src/paint.rs:7`) without leaking Rust type names.
  5. **F5**: Batch `apply_spec` and `set_slot` in `crates/designos-mcp/src/tools/batch.rs` run `describe_spec_args_error(&raw_op)` both on `serde` error and after `serde_json::from_value` succeeds.
  6. **F6**: `designos_mcp::tools::errors::parse_spec_json` deserializes `designos_doc::NodeSpec` directly from `raw: &str` via `serde_json::from_str::<designos_doc::NodeSpec>(raw)` so duplicate JSON keys are refused on the CLI just as on base, and strips internal error-code prefixes (`UNKNOWN_FIELD: `, `INVALID_SLOT_VALUE: `) from user-facing messages while keeping `ErrorCode::UnknownField` and `ErrorCode::InvalidSlotValue` on `ToolError.code`.
  7. **Real open result (`App::try_open_path`)**: Added `App::try_open_path(&mut self, p: PathBuf) -> Result<(), String>` next to `App::open_path` in `crates/designos-app/src/ops.rs`. `open_path` calls `let _ = self.try_open_path(p);`, and `Shell::build` (`crates/designos-app/src/shell.rs`) calls `app.try_open_path(f.clone())` directly instead of inferring failure from `app.canvas.dirty` and `app.toast`.
  8. **Simple test seam**: Deleted the `#[path = "."] mod io_raw` shim from `crates/designos-app/src/main.rs` and placed the `#[cfg(test)]` `load_call_count` counter directly inside `io::load_path_with_warning` in `crates/designos-app/src/io.rs`.
- **Epistemic Ledger**:
  - **FACT**: All 6 new `designos-mcp` tests and all 4 new `designos-app` CLI tests failed on `994b1f05` (`plans/reports/agy-e-cli-spec-errors-evidence/r3-red.log`) and pass on the new head (`plans/reports/agy-e-cli-spec-errors-evidence/r3-green.log`).
  - **FACT**: Across the 455-combination differential matrix (`35` `NodeSpec` fields × `13` sample values in `round3_f32_overflow_and_comments_parity_with_serde`), the only inputs accepted by `serde_json::from_str::<designos_doc::NodeSpec>` that `parse_spec_json` rejects are Exception (a) (positional arrays where an object is expected) and Exception (b) (`1e40` on `#[serde(untagged)]` `NumberOrVar` fields where `serde`'s internal `ContentDeserializer` casts `1e40_f64 as f32` to `f32::INFINITY`), and zero error messages leak Rust type names or internal error-code prefixes.
  - **FACT**: `git diff --stat 994b1f05..HEAD -- crates/designos-app/src/cli.rs` shows `1 insertion(+), 378 deletions(-)`.
  - **INFERENCE**: None required; all acceptance checks were executed directly.
  - **ASSUMPTION**: None.
- **Unresolved questions**: None.

### 2) Acceptance Table (Round 3)

| # | Acceptance Check | Status | Command & Evidence |
|---|---|---|---|
| 1 | `r3-red.log`: every new test fails on `994b1f05`. `r3-green.log`: they pass on your head. | PASS | `plans/reports/agy-e-cli-spec-errors-evidence/r3-red.log` (6 failed in `designos-mcp`, 4 failed in `designos-app`) and `plans/reports/agy-e-cli-spec-errors-evidence/r3-green.log` (all passed). |
| 2 | `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --locked -- -D warnings` exit 0. | PASS | Both exited `0` (`plans/reports/agy-e-cli-spec-errors-evidence/r3-lint.log`). |
| 3 | `cargo test -p designos-mcp --locked`, `cargo test -p designos-eval --locked`, `cargo test -p designos-app --bin designos --locked cli`, and `cargo test -p designos-app --test cli_screenshot --locked` pass under the harness. | PASS | All 4 test commands exited `0` (`plans/reports/agy-e-cli-spec-errors-evidence/r3-tests.log`). |
| 4 | Show `git diff --stat 994b1f05..HEAD -- crates/designos-app/src/cli.rs`: the duplicate is gone. | PASS | `crates/designos-app/src/cli.rs \| 379 +----------------------------------------` (`1 insertion(+), 378 deletions(-)`). |
| 5 | Commit as `fix(mcp): …` / `fix(cli): …` (no AI or tool names), push, and add a "Round 3" line to the PR body. Keep the PR a draft, never merge, and delete `out/`. | PASS | Committed, pushed, PR #1117 body updated with Round 3 line, `out/` deleted. |

### 3) Changes per file (Round 3)

#### Skills Loaded & Changes Because of Them
- **Skills loaded**:
  - `es:dev` (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`)
  - `es:feature-dev` (`/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`)
  - `es:rust` (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`)
  - `es:rust-gpu-canvas` (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`)
  - `es:fable-thinking` (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`)
  - `es:astra-work` (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`)
- **What changed because of the loaded skills**:
  - Per `es:fable-thinking` falsification-first discipline, reproduced all reviewer probes (`set_slot` error misattribution, nested path loss, `f32` overflow vs non-numeric type error, structured comment `author` false reject, batch `apply_spec` bypass, duplicate JSON key acceptance on CLI, and `UNKNOWN_FIELD:` prefix leak) in `r3-red.log` before editing production code, and added a 455-case differential matrix test in `crates/designos-mcp/tests/spec_errors.rs` that caught `#[serde(untagged)]` `NumberOrVar` accepting `1e40` as `f32::INFINITY`.
  - Per `es:rust` single-source-of-truth and API design rules, consolidated spec parsing into `designos_mcp::tools::errors::parse_spec_json`, added `App::try_open_path(&mut self, p: PathBuf) -> Result<(), String>` in `crates/designos-app/src/ops.rs`, and moved the `#[cfg(test)]` load counter into `crates/designos-app/src/io.rs`.

#### Per-File Summary
- **`crates/designos-mcp/src/tools/errors.rs`**:
  - Stripped `"INVALID_SLOT_VALUE: "` from `ToolError.message` when constructing `ErrorCode::InvalidSlotValue` in `from_serde_error_with_args`.
  - Added `join_spec_path`, `is_non_finite_f32_number`, `is_valid_color`, `is_valid_comment_author`, `describe_comments_error`, `describe_slot_fill_error`, `describe_spec_value_error_at`, and `pub fn parse_spec_json`.
  - Updated `describe_layout_spec_error`, `describe_slots_error`, `describe_override_spec_error`, `describe_in_progress_error`, and `describe_spec_value_error_at` to carry the full slash-separated path from the spec root and distinguish non-finite `f32` numbers (`1e40`) from non-numeric values.
  - Updated `describe_spec_args_error` to validate `set_slot` (`node`/`node_id`/`instance`, `slot`, `as`, `fill`/`value`) and `apply_spec` (`parent_id`, `parent`, `index`, `as`, `diff_image`, `idempotency_key`, `spec`), returning `None` at the end instead of treating arbitrary batch ops as a `NodeSpec`.
  - Added `.replace("struct Color", "an RGBA color object")` in `humanize_spec_serde_message`.
- **`crates/designos-mcp/src/tools/batch.rs`**:
  - Added `describe_spec_args_error(&raw_op)` check after `serde_json::from_value` in both `"apply_spec"` and `"set_slot"`.
- **`crates/designos-mcp/tests/spec_errors.rs`**:
  - Added `round3_set_slot_bad_slot_reports_slot_field`, `round3_set_slot_bad_child_item_in_fill_reports_child_path`, `round3_set_slot_bad_child_field_in_fill_reports_nested_path`, `round3_nested_spec_errors_carry_path_from_root`, `round3_f32_overflow_and_comments_parity_with_serde` (including the 455-combination differential matrix), and `round3_batch_apply_spec_runs_same_walk_as_direct_apply_spec`.
- **`crates/designos-app/src/cli.rs`**:
  - Deleted the duplicate `describe_spec_value_error` walk and helpers (`-378` lines) and delegated `cli::parse_spec_json` to `designos_mcp::tools::errors::parse_spec_json`.
- **`crates/designos-app/src/ops.rs`**:
  - Added `pub fn try_open_path(&mut self, p: PathBuf) -> Result<(), String>` next to `pub fn open_path(&mut self, p: PathBuf)` so every open failure path returns `Err(String)` while `open_path` calls `let _ = self.try_open_path(p);`.
- **`crates/designos-app/src/shell.rs`**:
  - Updated `Shell::build` to call `if let Err(err) = app.try_open_path(f.clone())` in screenshot mode instead of inferring open failure from `app.canvas.dirty` and `app.toast`.
- **`crates/designos-app/src/io.rs` & `crates/designos-app/src/main.rs`**:
  - Moved `#[cfg(test)] fn load_counts` and `#[cfg(test)] pub fn load_call_count` into `crates/designos-app/src/io.rs` inside `load_path_with_warning`, deleted `#[path = "."] mod io_raw` from `crates/designos-app/src/main.rs`, and added `round3_cli_apply_spec_reports_nested_path`, `round3_cli_apply_spec_refuses_duplicate_keys`, `round3_cli_apply_spec_unknown_field_omits_code_prefix`, `round3_cli_apply_spec_rejects_f32_overflow_with_clear_message`, plus direct `App::try_open_path` assertions in `cli_screenshot_loads_good_file_once`.

### 4) Every user-facing string in `crates/designos-mcp/src/tools/errors.rs` (and `crates/designos-app/src/shell.rs`), verbatim

#### Spec & `set_slot` validation strings (`crates/designos-mcp/src/tools/errors.rs`)
- `"{path} must be an array of comment thread objects. Omit comments when applying a spec, or pass valid comment thread objects."`
- `"{msg_path}/id must be a string. Pass a string value."`
- `"{msg_path}/author must be a string or an author object. Pass a display name string or an object such as {{\"id\": \"...\", \"name\": \"...\"}}."`
- `"{msg_path}/author_id must be a string. Pass a string value."`
- `"{msg_path}/author_color must be an RGBA color object with numeric r, g, b, and a fields between 0 and 1."`
- `"{msg_path}/text must be a string. Pass a string value."`
- `"{msg_path}/created must be a string. Pass an RFC 3339 timestamp string."`
- `"{msg_path}/edited must be a string. Pass an RFC 3339 timestamp string or null."`
- `"{prefix} must be a LayoutSpec object. Pass a JSON object such as {{\"direction\": \"row\", \"gap\": 8}}."`
- `"{field_path} must be a string. Pass a string value."`
- `"{field_path} must be a boolean. Pass true or false."`
- `"{field_path} must be a finite number. Pass a smaller number."`
- `"{field_path} must be a number or a \"var:...\" string. Pass a number in pixels or a variable reference."`
- `"{field_path} must contain finite numbers. Pass smaller numbers."`
- `"{field_path} must be an array of 4 numbers or \"var:...\" strings ([top, right, bottom, left]). Pass 4 values such as [16, 16, 16, 16]."`
- `"{field_path} must be a positive integer. Pass a whole number of grid tracks."`
- `"{item_path} must be a NodeSpec object. Pass each slot child as a JSON object."`
- `"{slot_path} must be false or an array of NodeSpec objects. Pass false or [NodeSpec] for the slot."`
- `"{path} must be an object mapping slot names to false or an array of NodeSpec objects. Pass false or [NodeSpec] for each slot."`
- `"{prefix} must be an OverrideSpec object. Pass each override as a JSON object."`
- `"{field_path} must be a non-negative integer. Pass a numeric node ID."`
- `"{field_path} must be a number between 0 and 1. Pass a numeric opacity such as 0.8."`
- `"{field_path} must be an object mapping variant property names to string values. Pass an object such as {{\"Size\": \"md\"}}."`
- `"{field_path} must be an object mapping component property names to values. Pass a JSON object."`
- `"{prefix} must be a boolean or an in_progress object. Pass true, false, or an object such as {{\"agent\": \"...\"}}."`
- `"{field_path} must be a non-negative integer. Pass a whole number."`
- `"{label} must be a NodeSpec object. Pass a JSON object with node fields such as kind, name, and children."`
- `"{field_path} must be an object mapping variable collection names to mode names. Pass an object such as {{\"Theme\": \"Dark\"}}."`
- `"{field_path} must be an array of OverrideSpec objects. Pass each override as a JSON object."`
- `"{field_path} must be a number. Pass a numeric coordinate in pixels."`
- `"{field_path} must be a number. Pass a numeric size in pixels."`
- `"{field_path} must be a number between 0 and 1 or a \"var:...\" string. Pass a number such as 0.8 or a variable reference."`
- `"{field_path} must be an array of 4 numbers or \"var:...\" strings ([tl, tr, br, bl]). Pass 4 values such as [8, 8, 8, 8]."`
- `"{field_path} must be a number between 100 and 900 (for example 400, 500, 600, or 700). Pass an integer weight."`
- `"{field_path} must be a boolean or a \"var:...\" string. Pass true, false, or a boolean variable reference."`
- `"{field_path} must be an array of strings. Pass an array such as [\"Note\"]."`
- `"{field_path} must be an array of NodeSpec objects. Pass each child as a JSON object."`
- `"node must be a node ID or $ref string. Pass a numeric node ID or a \"$name\" reference."`
- `"slot must be a string. Pass a slot name string."`
- `"as must be a string. Pass an alias name string."`
- `"parent_id must be a non-negative integer. Pass a numeric node ID."`
- `"parent must be a node ID or $ref string. Pass a numeric node ID or a \"$name\" reference."`
- `"index must be a non-negative integer. Pass a zero-based child index."`
- `"diff_image must be a boolean. Pass true or false."`
- `"idempotency_key must be a string. Pass a string key."`
- `"Unknown field \`{bad}\`."`
- `"Unknown field \`{bad}\` (valid options: {})"`
- `"value must be a number or a \"var:...\" string. Pass a number in pixels or a variable reference."`
- `"visible must be a boolean or a \"var:...\" string. Pass true, false, or a boolean variable reference."`
- `"in_progress must be a boolean or an in_progress object. Pass true, false, or an object such as {\"agent\": \"...\"}."`

#### Screenshot open-failure string (`crates/designos-app/src/shell.rs`)
- `"{} could not be opened: {reason}. Check the file, then try again."`

### 5) Coverage & fields not covered by custom per-field messages
- **All 35 `NodeSpec` fields** (`id`, `kind`, `name`, `component`, `variant`, `props`, `overrides`, `slots`, `preset`, `x`, `y`, `width`, `height`, `width_sizing`, `height_sizing`, `min_width`, `max_width`, `min_height`, `max_height`, `col_span`, `row_span`, `fill`, `stroke`, `stroke_weight`/`stroke_width`, `corner_radius`/`radius`, `corner_radii`, `opacity`, `clips_content`, `visible`, `text`, `font_size`, `line_height`, `font_weight`, `text_color`/`color`, `text_style`, `effect_style`, `grid_style`, `mode`, `layout`, `dev_status`, `annotation`, `annotations`, `in_progress`, `comments`, `children`), **all 9 `LayoutSpec` fields** (`direction`/`dir`, `wrap`, `gap`, `counter_gap`, `padding`, `primary_align`/`justify`, `counter_align`/`align`, `grid_columns`, `grid_rows`), **all 11 `OverrideSpec` fields** (`path`, `id`, `text`, `fill`, `stroke`, `visible`, `opacity`, `swap`, `variant`, `props`, `slots`), **all 6 `InProgressWire` fields** (`run_id`, `agent`/`agent_name`, `status`, `started_at_ms`, `since_ms`, `stale_after_s`), and **all 7 `RawComment` message fields** (`id`, `author`, `author_id`, `author_color`, `text`, `created`, `edited`) are covered.
- **Fields without dedicated per-field messages**:
  - Top-level `ResolvedCommentThread` metadata fields (`id`, `resolved`, `page_id`, `page_name`, `world_x`, `world_y`, `target_node_id`, `target_node_name`, `target_node_kind`, `target_frame_id`, `target_frame_name`, `messages` in `crates/designos-doc/src/spec.rs:406-418`) are validated together and report the thread-level message `"{path} must be an array of comment thread objects. Omit comments when applying a spec, or pass valid comment thread objects."` rather than individual per-field messages.
  - Arbitrary JSON values inside `props: BTreeMap<String, serde_json::Value>` (`NodeSpec.props` and `OverrideSpec.props`) have no inner schema restrictions in `NodeSpec` / `OverrideSpec` (any JSON value is accepted by `serde` once `props` is a JSON object).

### 6) Exact Commands & Pasted Output (Round 3)

#### `plans/reports/agy-e-cli-spec-errors-evidence/r3-red.log`
```text
$ cargo test -p designos-mcp --test spec_errors --locked round3_
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-mcp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.96s
     Running tests/spec_errors.rs (out/cargo/debug/deps/spec_errors-9f07075765b87406)

running 6 tests
test round3_nested_spec_errors_carry_path_from_root ... FAILED
test round3_f32_overflow_and_comments_parity_with_serde ... FAILED
test round3_batch_apply_spec_runs_same_walk_as_direct_apply_spec ... FAILED
test round3_set_slot_bad_child_field_in_fill_reports_nested_path ... FAILED
test round3_set_slot_bad_slot_reports_slot_field ... FAILED
test round3_set_slot_bad_child_item_in_fill_reports_child_path ... FAILED

failures:

---- round3_nested_spec_errors_carry_path_from_root stdout ----

thread 'round3_nested_spec_errors_carry_path_from_root' (52381226) panicked at crates/designos-mcp/tests/spec_errors.rs:160:5:
expected nested path in "Invalid arguments: padding must be an array of 4 numbers or \"var:...\" strings ([top, right, bottom, left]). Pass 4 values such as [16, 16, 16, 16]."
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- round3_f32_overflow_and_comments_parity_with_serde stdout ----

thread 'round3_f32_overflow_and_comments_parity_with_serde' (52381225) panicked at crates/designos-mcp/tests/spec_errors.rs:175:5:
expected 'x must be a finite number' in "Invalid arguments: x must be a number. Pass a numeric coordinate in pixels."

---- round3_batch_apply_spec_runs_same_walk_as_direct_apply_spec stdout ----

thread 'round3_batch_apply_spec_runs_same_walk_as_direct_apply_spec' (52381224) panicked at crates/designos-mcp/tests/spec_errors.rs:265:10:
batch apply_spec with layout: [] must fail: Object {"changed": Object {"deleted": Number(0), "inserted": Number(1), "moved": Number(0), "styles": Number(0), "updated": Number(0), "variables": Number(0)}, "created": Array [Number(2)], "created_count": Number(1), "dry_run": Bool(false), "issues": Array [], "issues_total": Number(0), "issues_truncated": Bool(false), "name_to_id": Object {"Hero": Number(2)}, "touched_bounds": Object {"height": Number(100.0), "width": Number(200.0), "x": Number(0.0), "y": Number(0.0)}, "touched_roots": Array [Object {"bounds": Object {"height": Number(100.0), "width": Number(200.0), "x": Number(0.0), "y": Number(0.0)}, "node_id": Number(2), "op": Number(0)}], "undo_label": String("Agent: designos-mcp")}

---- round3_set_slot_bad_child_field_in_fill_reports_nested_path stdout ----

thread 'round3_set_slot_bad_child_field_in_fill_reports_nested_path' (52381227) panicked at crates/designos-mcp/tests/spec_errors.rs:135:5:
expected 'fill/0/width must be a number or a "var:..." string' in "op #0 (set_slot) failed: Invalid arguments: fill must be a string. Pass a string value."

---- round3_set_slot_bad_slot_reports_slot_field stdout ----

thread 'round3_set_slot_bad_slot_reports_slot_field' (52381229) panicked at crates/designos-mcp/tests/spec_errors.rs:83:9:
for {"fill":[{"kind":"text","text":"Body"}],"node":2,"op":"set_slot","slot":5}, expected 'slot must be a string' in "op #0 (set_slot) failed: Invalid arguments: fill must be a string. Pass a string value."

---- round3_set_slot_bad_child_item_in_fill_reports_child_path stdout ----

thread 'round3_set_slot_bad_child_item_in_fill_reports_child_path' (52381228) panicked at crates/designos-mcp/tests/spec_errors.rs:112:5:
expected 'fill/0 must be a NodeSpec object' in "op #0 (set_slot) failed: Invalid arguments: fill must be a string. Pass a string value."


failures:
    round3_batch_apply_spec_runs_same_walk_as_direct_apply_spec
    round3_f32_overflow_and_comments_parity_with_serde
    round3_nested_spec_errors_carry_path_from_root
    round3_set_slot_bad_child_field_in_fill_reports_nested_path
    round3_set_slot_bad_child_item_in_fill_reports_child_path
    round3_set_slot_bad_slot_reports_slot_field

test result: FAILED. 0 passed; 6 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.01s

error: test failed, to rerun pass `-p designos-mcp --test spec_errors`

$ cargo test -p designos-app --bin designos --locked round3_
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-mcp)
   Compiling designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 18.70s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-d02548a0edc4e955)

running 4 tests
test icon_tests::round3_cli_apply_spec_unknown_field_omits_code_prefix ... FAILED
test icon_tests::round3_cli_apply_spec_reports_nested_path ... FAILED
test icon_tests::round3_cli_apply_spec_rejects_f32_overflow_with_clear_message ... FAILED
test icon_tests::round3_cli_apply_spec_refuses_duplicate_keys ... FAILED

failures:

---- icon_tests::round3_cli_apply_spec_unknown_field_omits_code_prefix stdout ----

thread 'icon_tests::round3_cli_apply_spec_unknown_field_omits_code_prefix' (52386431) panicked at crates/designos-app/src/main.rs:783:9:
user-facing CLI error must not include internal code prefix: "Invalid NodeSpec JSON in /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.OI1g15/designos_rt07_r3_unk_12100/spec.json: UNKNOWN_FIELD: unknown field `colour` (valid_options: slots, layout, props, col_span, fill)"
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- icon_tests::round3_cli_apply_spec_reports_nested_path stdout ----

thread 'icon_tests::round3_cli_apply_spec_reports_nested_path' (52386430) panicked at crates/designos-app/src/main.rs:748:9:
expected nested path in "Invalid NodeSpec JSON in /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.OI1g15/designos_rt07_r3_nested_12100/spec.json: padding must be an array of 4 numbers or \"var:...\" strings ([top, right, bottom, left]). Pass 4 values such as [16, 16, 16, 16]."

---- icon_tests::round3_cli_apply_spec_rejects_f32_overflow_with_clear_message stdout ----

thread 'icon_tests::round3_cli_apply_spec_rejects_f32_overflow_with_clear_message' (52386429) panicked at crates/designos-app/src/main.rs:800:9:
expected finite-number message in "Invalid NodeSpec JSON in /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.OI1g15/designos_rt07_r3_ovf_12100/spec.json: x must be a number. Pass a numeric coordinate in pixels."

---- icon_tests::round3_cli_apply_spec_refuses_duplicate_keys stdout ----
{
  "root_node_id": 2,
  "saved": true
}

thread 'icon_tests::round3_cli_apply_spec_refuses_duplicate_keys' (52386428) panicked at crates/designos-app/src/main.rs:766:51:
duplicate JSON keys must be refused: ()


failures:
    icon_tests::round3_cli_apply_spec_refuses_duplicate_keys
    icon_tests::round3_cli_apply_spec_rejects_f32_overflow_with_clear_message
    icon_tests::round3_cli_apply_spec_reports_nested_path
    icon_tests::round3_cli_apply_spec_unknown_field_omits_code_prefix

test result: FAILED. 0 passed; 4 failed; 0 ignored; 0 measured; 3281 filtered out; finished in 0.09s

error: test failed, to rerun pass `-p designos-app --bin designos`
```

#### `plans/reports/agy-e-cli-spec-errors-evidence/r3-green.log`
```text
$ cargo test -p designos-mcp --test spec_errors --locked round3_
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-mcp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 15.32s
     Running tests/spec_errors.rs (out/cargo/debug/deps/spec_errors-9f07075765b87406)

running 6 tests
test round3_nested_spec_errors_carry_path_from_root ... ok
test round3_batch_apply_spec_runs_same_walk_as_direct_apply_spec ... ok
test round3_set_slot_bad_child_field_in_fill_reports_nested_path ... ok
test round3_set_slot_bad_child_item_in_fill_reports_child_path ... ok
test round3_set_slot_bad_slot_reports_slot_field ... ok
test round3_f32_overflow_and_comments_parity_with_serde ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.01s


$ cargo test -p designos-app --bin designos --locked round3_
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-mcp)
   Compiling designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 30.73s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-d02548a0edc4e955)

running 4 tests
test icon_tests::round3_cli_apply_spec_rejects_f32_overflow_with_clear_message ... ok
test icon_tests::round3_cli_apply_spec_refuses_duplicate_keys ... ok
test icon_tests::round3_cli_apply_spec_unknown_field_omits_code_prefix ... ok
test icon_tests::round3_cli_apply_spec_reports_nested_path ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 3281 filtered out; finished in 0.01s


$ cargo test -p designos-app --bin designos --locked cli_screenshot_loads_good_file_once
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-mcp)
   Compiling designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 36.55s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-d02548a0edc4e955)

running 1 test
test icon_tests::cli_screenshot_loads_good_file_once ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3284 filtered out; finished in 0.15s
```

#### Acceptance 2, 3, & 4 Commands & Output
```text
$ cargo fmt --all -- --check
EXIT=0

$ cargo clippy --workspace --all-targets --locked -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 17.28s
EXIT=0

$ cargo test -p designos-mcp --locked
test result: ok. 245 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.22s
     Running tests/spec_errors.rs
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

$ cargo test -p designos-eval --locked
test result: ok. 46 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s

$ cargo test -p designos-app --bin designos --locked cli
test result: ok. 295 passed; 0 failed; 0 ignored; 0 measured; 2990 filtered out; finished in 73.91s

$ cargo test -p designos-app --test cli_screenshot --locked
running 1 test
test cli_screenshot_refuses_broken_files_and_captures_good_file ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.63s

$ git diff --stat 994b1f05..HEAD -- crates/designos-app/src/cli.rs
 crates/designos-app/src/cli.rs | 379 +----------------------------------------
 1 file changed, 1 insertion(+), 378 deletions(-)
```

---

## Round 4

### 1) What could NOT be verified
- **Non-macOS CI runners (`Linux` / `Windows`)**: All `designos-mcp` and `designos-eval` unit/integration tests, `cargo fmt`, and workspace `cargo clippy` were executed locally on macOS (`aarch64-apple-darwin`) under the isolated sandbox harness; Ubuntu and Windows CI runners were not executed locally.

### Conclusion & Epistemic Ledger
- **Conclusion (FACT)**: Fixed `describe_spec_args_error` in `crates/designos-mcp/src/tools/errors.rs` so that when `op == Some("apply_spec")` (a batch `apply_spec` operation where `ApplySpecOp.parent: Option<NodeRef>` has `#[serde(alias = "parent_id")]`), `"parent_id"` accepts `null`, `u64`, and `String` (`NodeRef::Id(u64)` and `NodeRef::Ref(String)`), identical to `"parent"`. When `op` is absent (direct `designos_apply_spec`, where `ApplySpecArgs.parent_id` is `Option<u64>`), `"parent_id"` retains its integer-only check (`"parent_id must be a non-negative integer. Pass a numeric node ID."`). Also updated `set_slot` in `describe_spec_args_error` to check `"node"`, `"node_id"`, and `"instance"` independently in a loop so every `NodeRef` alias on `SetSlotOp.node` is validated consistently and names the offending alias key on invalid non-`NodeRef` types.
- **Epistemic Ledger**:
  - **FACT**: On `b17d1ba7`, `round4_batch_apply_spec_accepts_noderef_parent_id_while_direct_refuses_string` failed (`plans/reports/agy-e-cli-spec-errors-evidence/r4-red.log`) because `serde_json::from_value::<ApplySpecOp>` accepted `"parent_id": "@card"` via `#[serde(alias = "parent_id")]`, and then the post-serde call to `describe_spec_args_error(&raw_op)` at `crates/designos-mcp/src/tools/batch.rs:625` rejected `"parent_id": "@card"` at `crates/designos-mcp/src/tools/errors.rs:1384` with `"parent_id must be a non-negative integer. Pass a numeric node ID."`.
  - **FACT**: Direct `designos_apply_spec` uses `ApplySpecArgs` (`crates/designos-mcp/src/engine.rs:1471-1479`), which has `#[serde(deny_unknown_fields)]`, no `op` field, and `parent_id: Option<u64>` (no `parent` alias).
  - **FACT**: After checking `op == Some("apply_spec")` in `describe_spec_args_error`, `round4_batch_apply_spec_accepts_noderef_parent_id_while_direct_refuses_string` and all 8 tests in `crates/designos-mcp/tests/spec_errors.rs` pass (`plans/reports/agy-e-cli-spec-errors-evidence/r4-green.log`), and all 245 unit tests + integration test suites in `designos-mcp` and `designos-eval` pass under the harness.
  - **INFERENCE**: Because `ApplySpecArgs` has `#[serde(deny_unknown_fields)]` and no `op` field, checking `op == Some("apply_spec")` in `describe_spec_args_error` cleanly separates batch `apply_spec` from direct `designos_apply_spec` both on the post-serde walk and on the `from_serde_error_with_args` fallback path.
  - **ASSUMPTION**: None.
- **Weakest link**: Distinguishing batch `apply_spec` from direct `designos_apply_spec` inside `describe_spec_args_error(&Value)` relies on `obj.get("op").and_then(Value::as_str) == Some("apply_spec")`, which holds because batch ops require `"op": "apply_spec"` and `ApplySpecArgs` forbids `"op"` via `#[serde(deny_unknown_fields)]`.
- **Unresolved questions**: None.

### 2) Acceptance Table (Round 4)

| # | Acceptance Check | Status | Command & Evidence |
|---|---|---|---|
| 1 | `r4-red.log` (fails on `b17d1ba7`) and `r4-green.log` (passes on your head). | PASS | Saved at `plans/reports/agy-e-cli-spec-errors-evidence/r4-red.log` (1 failed on `b17d1ba7`) and `plans/reports/agy-e-cli-spec-errors-evidence/r4-green.log` (8 passed, 0 failed on head). |
| 2 | `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --locked -- -D warnings` exit 0. | PASS | Both exited `0` under the harness (`FMT_EXIT=0`, `CLIPPY_EXIT=0`). |
| 3 | `cargo test -p designos-mcp --locked` and `cargo test -p designos-eval --locked` pass under the harness. | PASS | `designos-mcp` and `designos-eval` passed with 0 failures (`MCP_TEST_EXIT=0`, `EVAL_TEST_EXIT=0`). |
| 4 | Commit as `fix(mcp): …` (no AI or tool names), push, and add a "Round 4" line to the PR body. Keep the PR a draft. Delete `out/`. | PASS | Committed, pushed, PR #1117 body updated with Round 4 line, `out/` deleted. |

### 3) Audited `NodeRef` & Alias Fields Across Batch Ops and `describe_spec_args_error`

Every struct and field that passes through `describe_spec_args_error` (`crates/designos-mcp/src/tools/errors.rs:1359`), as well as every other batch op in `crates/designos-mcp/src/tools/batch.rs`, was audited against its serde definition:

| Struct / Op | Goes through `describe_spec_args_error`? | Field & Serde Attributes | Serde Accepts | Walk (`describe_spec_args_error`) Accepts | Result |
|---|---|---|---|---|---|
| `ApplySpecOp` (`op = "apply_spec"`, `batch.rs:212-221`) | Yes (`batch.rs:622, 625`) | `op: String` | `"apply_spec"` | `"apply_spec"` | Match |
| `ApplySpecOp` (`op = "apply_spec"`) | Yes | `#[serde(rename = "as")] alias: Option<String>` | omitted, `null`, `String` | omitted, `null`, `String` (`!alias.is_null() && !alias.is_string()`) | Match |
| `ApplySpecOp` (`op = "apply_spec"`) | Yes | `#[serde(alias = "parent_id")] parent: Option<NodeRef>` | omitted, `null`, `u64`, `String` on `"parent"` or `"parent_id"` | **Fixed in Round 4**: omitted, `null`, `u64`, `String` on both `"parent"` and `"parent_id"` when `op == Some("apply_spec")` | Fixed (was rejecting `String` on `"parent_id"`) |
| `ApplySpecOp` (`op = "apply_spec"`) | Yes | `index: Option<usize>` | omitted, `null`, `usize` (`u64`) | omitted, `null`, `u64` (`!idx.is_null() && idx.as_u64().is_none()`) | Match |
| `ApplySpecOp` (`op = "apply_spec"`) | Yes | `spec: NodeSpec` (incl. aliases `dir`, `justify`, `align`, `stroke_width`, `radius`, `color`, `agent_name`, `started_at_ms`) | `NodeSpec` fields + aliases | `describe_spec_value_error(spec)` checks both canonical names and serde aliases | Match |
| `SetSlotOp` (`op = "set_slot"`, `batch.rs:384-394`) | Yes (`batch.rs:716, 719`) | `op: String` | `"set_slot"` | `"set_slot"` | Match |
| `SetSlotOp` (`op = "set_slot"`) | Yes | `#[serde(rename = "as")] alias: Option<String>` | omitted, `null`, `String` | omitted, `null`, `String` (`!alias.is_null() && !alias.is_string()`) | Match |
| `SetSlotOp` (`op = "set_slot"`) | Yes | `#[serde(alias = "node_id", alias = "instance")] node: NodeRef` | `u64`, `String` on `"node"`, `"node_id"`, or `"instance"` | `u64`, `String` on `"node"`, `"node_id"`, and `"instance"` (`node.as_u64().is_none() && !node.is_string()`) | Match (tightened loop to check each key independently) |
| `SetSlotOp` (`op = "set_slot"`) | Yes | `slot: String` | `String` | `String` (`!slot.is_string()`) | Match |
| `SetSlotOp` (`op = "set_slot"`) | Yes | `#[serde(alias = "fill")] value: SlotFillSpec` | `false` or `[NodeSpec, ...]` on `"value"` or `"fill"` | `describe_slot_fill_error` on `"fill"` and `"value"` accepts `false` or array of `NodeSpec` objects | Match |
| `ApplySpecArgs` (direct `designos_apply_spec`, `engine.rs:1471-1479`, no `op`) | Yes (`engine.rs:1321-1322`) | `parent_id: Option<u64>` (no alias) | omitted, `null`, `u64` | omitted, `null`, `u64` when `op != Some("apply_spec")`; refuses strings like `"root"` with `"parent_id must be a non-negative integer. Pass a numeric node ID."` | Match |
| `ApplySpecArgs` (direct `designos_apply_spec`) | Yes | `diff_image: bool`, `idempotency_key: Option<String>`, `lint: Option<PerCallLintOverride>`, `spec: NodeSpec` | `bool`, `null \| String`, `PerCallLintOverride`, `NodeSpec` | `bool`, `null \| String`, `describe_spec_value_error(spec)` | Match |
| `InsertOp` (`"insert"` / `"create"`, `batch.rs:225-248`) | No (`is_spec = false`, no post-serde walk) | `#[serde(rename = "as")] alias: Option<String>`, `#[serde(alias = "parent_id")] parent: Option<NodeRef>` | `Option<String>`, `Option<NodeRef>` | Not walked by `describe_spec_args_error`; validated by `validate_ref_pre_run` | Match |
| `UpdateOp` (`"update"`, `batch.rs:252-271`) | No (`is_spec = false`, no post-serde walk) | `#[serde(rename = "as")] alias: Option<String>`, `#[serde(alias = "node_id")] node: NodeRef` | `Option<String>`, `NodeRef` | Not walked by `describe_spec_args_error`; validated by `validate_ref_pre_run` | Match |
| `MoveOp` (`"move"`, `batch.rs:275-285`) | No (`is_spec = false`, no post-serde walk) | `#[serde(rename = "as")] alias: Option<String>`, `#[serde(alias = "node_id")] node: NodeRef`, `#[serde(alias = "parent_id")] parent: NodeRef` | `Option<String>`, `NodeRef`, `NodeRef` | Not walked by `describe_spec_args_error`; validated by `validate_ref_pre_run` | Match |
| `DeleteOp` (`"delete"`, `batch.rs:289-296`) | No (`is_spec = false`, no post-serde walk) | `#[serde(rename = "as")] alias: Option<String>`, `#[serde(alias = "node_id")] node: NodeRef` | `Option<String>`, `NodeRef` | Not walked by `describe_spec_args_error`; validated by `validate_ref_pre_run` | Match |
| `SetVariablesOp` (`"set_variables"`, `batch.rs:329-340`) | No (`is_spec = false`, no post-serde walk) | `#[serde(rename = "as")] alias: Option<String>` | `Option<String>` | Not walked by `describe_spec_args_error` | Match |
| `SetStyleOp` (`"set_style"`, `batch.rs:369-380`) | No (`is_spec = false`, no post-serde walk) | `#[serde(rename = "as")] alias: Option<String>`, `apply[].node: NodeRef` (`#[serde(alias = "node_id")]`) | `Option<String>`, `NodeRef` | Not walked by `describe_spec_args_error`; validated by `validate_ref_pre_run` | Match |
| `SetInProgressOp` (`"set_in_progress"`, `batch.rs:398-421`) | No (`is_spec = false`, no post-serde walk) | `#[serde(rename = "as")] alias: Option<String>`, `#[serde(alias = "node_ids")] nodes: Option<Vec<NodeRef>>`, `#[serde(alias = "node_id")] node: Option<NodeRef>`, `#[serde(alias = "agent_name")] agent: Option<String>` | `Option<String>`, `Option<Vec<NodeRef>>`, `Option<NodeRef>` | Not walked by `describe_spec_args_error`; validated by `validate_ref_pre_run` | Match |

### 4) Skills Loaded & Changes Per File (Round 4)

#### Skills Loaded & Changes Because of Them
- **Skills loaded**:
  - `/es:dev` (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`)
  - `/es:feature-dev` (`/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`)
  - `/es:rust` (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`)
  - `/es:rust-gpu-canvas` (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`)
  - `/es:fable-thinking` (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`)
  - `/es:astra-work` (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`)
- **What changed because of the loaded skills**:
  - Per `/es:fable-thinking` (falsification-first & systematic scope audit), wrote the red test on `b17d1ba7` first (`r4-red.log`) before editing `errors.rs`, and audited every struct and alias that passes through `describe_spec_args_error` (`ApplySpecOp`, `SetSlotOp`, `ApplySpecArgs`, plus all other batch ops).
  - Per `/es:rust` (zero-clone data flow), kept `describe_spec_args_error(&Value)` operating on borrowed `&Value` references without cloning JSON subtrees.

#### Per-File Summary
- **`crates/designos-mcp/src/tools/errors.rs`**:
  - In `describe_spec_args_error`, extracted `let op = obj.get("op").and_then(Value::as_str);`.
  - When `op == Some("apply_spec")`, allowed `"parent_id"` to be `null`, `u64`, or `String` (matching `ApplySpecOp.parent: Option<NodeRef>` with `#[serde(alias = "parent_id")]`), returning `"parent_id must be a node ID or $ref string. Pass a numeric node ID or a \"$name\" reference."` only when it is none of those.
  - When `op != Some("apply_spec")` (direct `designos_apply_spec`), preserved the integer-only `"parent_id must be a non-negative integer. Pass a numeric node ID."` check.
  - In `op == Some("set_slot")`, checked each of `["node", "node_id", "instance"]` in a loop so every `NodeRef` alias on `SetSlotOp` is checked and names the exact key on non-`NodeRef` values.
- **`crates/designos-mcp/tests/spec_errors.rs`**:
  - Added `round4_batch_apply_spec_accepts_noderef_parent_id_while_direct_refuses_string` verifying:
    1. Direct `designos_apply_spec` with `"parent_id": "root"` is still refused with `"parent_id must be a non-negative integer"`.
    2. Batch `apply_spec` with `"parent_id": "@card"` succeeds and `Title` lands as a child under `Card`.
    3. Batch `apply_spec` with `"parent_id": 7`, `"parent": "@sub"`, and `"parent_id": "$prev"` all succeed and place children under their respective parents.
    4. Batch `apply_spec` with a non-`NodeRef` `"parent_id": true` is refused with `"parent_id must be a node ID or $ref string"`.
    5. Batch `set_slot` accepts `"@card"` on `"node"`, `"node_id"`, and `"instance"` aliases without a `NodeRef` false reject.

### 5) Exact Commands & Pasted Output (Round 4)

#### `plans/reports/agy-e-cli-spec-errors-evidence/r4-red.log`
```text
$ cargo test -p designos-mcp --test spec_errors --locked round4_
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-mcp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.66s
     Running tests/spec_errors.rs (out/cargo/debug/deps/spec_errors-9f07075765b87406)

running 1 test
test round4_batch_apply_spec_accepts_noderef_parent_id_while_direct_refuses_string ... FAILED

failures:

---- round4_batch_apply_spec_accepts_noderef_parent_id_while_direct_refuses_string stdout ----

thread 'round4_batch_apply_spec_accepts_noderef_parent_id_while_direct_refuses_string' (53539057) panicked at crates/designos-mcp/tests/spec_errors.rs:373:10:
batch apply_spec with parent_id: "@card" must succeed: ToolError { code: BatchOpFailed, message: "op #1 (apply_spec) failed: Invalid arguments: parent_id must be a non-negative integer. Pass a numeric node ID.", path: "/ops/1", received: None, expected: None, valid_options: [], valid_options_total: None, corrected_example: None, cause: Some(ToolError { code: InvalidArguments, message: "Invalid arguments: parent_id must be a non-negative integer. Pass a numeric node ID.", path: "/ops/1", received: None, expected: None, valid_options: [], valid_options_total: None, corrected_example: None, cause: None, failed_op: None, op: None, safe_to_retry: true, next_step: "Adjust the value at `path` using `valid_options` or `corrected_example` and retry." }), failed_op: Some(1), op: Some("apply_spec"), safe_to_retry: true, next_step: "Adjust the value at `path` using `valid_options` or `corrected_example` and retry." }
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    round4_batch_apply_spec_accepts_noderef_parent_id_while_direct_refuses_string

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p designos-mcp --test spec_errors`
```

#### `plans/reports/agy-e-cli-spec-errors-evidence/r4-green.log`
```text
$ cargo test -p designos-mcp --test spec_errors --locked
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-mcp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 16.43s
     Running tests/spec_errors.rs (out/cargo/debug/deps/spec_errors-9f07075765b87406)

running 8 tests
test round3_nested_spec_errors_carry_path_from_root ... ok
test round3_batch_apply_spec_runs_same_walk_as_direct_apply_spec ... ok
test apply_spec_reports_field_and_expected_type_in_plain_words ... ok
test round3_set_slot_bad_child_item_in_fill_reports_child_path ... ok
test round3_set_slot_bad_child_field_in_fill_reports_nested_path ... ok
test round3_set_slot_bad_slot_reports_slot_field ... ok
test round3_f32_overflow_and_comments_parity_with_serde ... ok
test round4_batch_apply_spec_accepts_noderef_parent_id_while_direct_refuses_string ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
```

#### Acceptance 2 & 3 Commands & Output
```text
$ cargo fmt --all -- --check
FMT_EXIT=0

$ cargo clippy --workspace --all-targets --locked -- -D warnings
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-mcp)
    Checking designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-e-cli-spec-errors/crates/designos-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 17.22s
CLIPPY_EXIT=0

$ cargo test -p designos-mcp --locked
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_mcp-96fd55246d4a3a66)
test result: ok. 245 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.21s
     Running tests/spec_errors.rs (out/cargo/debug/deps/spec_errors-9f07075765b87406)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running tests/stdio.rs (out/cargo/debug/deps/stdio-9963b9496c82d7e4)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
     Running tests/text_fill.rs (out/cargo/debug/deps/text_fill-b3d88a68a195ca3e)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running tests/tools_snapshot.rs (out/cargo/debug/deps/tools_snapshot-e6488781337fbb09)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
     Running tests/version.rs (out/cargo/debug/deps/version-7878cb959d478560)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
   Doc-tests designos_mcp
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
MCP_TEST_EXIT=0

$ cargo test -p designos-eval --locked
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_eval-9a9f1f166d66684f)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs (out/cargo/debug/deps/designos_eval-30aca93b7d8d8c4a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/harness.rs (out/cargo/debug/deps/harness-28beba16668beb59)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.49s
     Running tests/real_engine.rs (out/cargo/debug/deps/real_engine-4a6cab08ce582fe8)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.71s
   Doc-tests designos_eval
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EVAL_TEST_EXIT=0
```

Status: DONE
