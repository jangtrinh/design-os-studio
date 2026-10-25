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

Status: DONE
