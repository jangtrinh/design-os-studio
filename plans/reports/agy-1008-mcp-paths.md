# Report — `designos-mcp` File Paths: Export `.fig`, File Names, Echoed Paths (`#1008`, `#1003` RT-05/RT-06, `#1087`)

## 1. What could NOT be verified

- **Nothing in the assigned scope was left unverified.** Per the task constraints ("do not build `designos-app`'s test binaries; the controller runs those"), `designos-app` test binaries and live GUI terminal workspace binding (`#1074`, owned by another worker in `crates/designos-app/src/terminal.rs`) were not built or executed here.
- **Conclusion (`FACT`):**
  - `FACT`: `designos_export_fig_file` (`Session` in `crates/designos-mcp/src/server.rs` and `Engine::call` in `crates/designos-mcp/src/engine.rs`) now refuses symbolic links at the final path component (pointing inside the workspace, pointing outside the workspace, or dangling), refuses folders and non-plain files (FIFOs/sockets/hard-linked files), refuses existing files unless `overwrite: true` is passed (`ErrorCode::FileExists`), compares `.fig` extensions case-insensitively (`UPPER.FIG`, `Mixed.Fig`), and writes exclusively through `crate::engine::write_checked_output_file` (which delegates to `session_file_io::truncate(resolved, false)` + `session_file_io::append(resolved, bytes, true)` using `O_NOFOLLOW | O_NONBLOCK` + `fstat` without `O_TRUNC`).
  - `FACT`: Every file-creating MCP tool (`designos_save_file`, `designos_render_png`, `designos_export_fig_file`) refuses paths containing control characters (`\n`, `\r`, C0/DEL/C1 controls) or Unicode bidi override/isolate characters (`U+202A..=U+202E`, `U+2066..=U+2069`, `U+200E`, `U+200F`, `U+061C`) with `ErrorCode::InvalidArguments`.
  - `FACT`: Every file-creating MCP tool checks path component byte length (`> 255` bytes) before opening or creating the file and returns `ErrorCode::InvalidArguments` (`"<p> has a name longer than 255 bytes. Use a shorter name."`) rather than `ErrorCode::FileExists`.
  - `FACT`: `ErrorCode::PathOutsideWorkspace` never stores or serializes `received` in `ToolError` (`crates/designos-mcp/src/tools/errors.rs`), and `input_path`, `output_path_allow_overwrite`, `png_output_path`, and `fig_output_path` in `crates/designos-mcp/src/server.rs` check workspace containment before any error branch that echoes the caller's path (`designos_open_file`, `designos_save_file`, `designos_render_png`, `designos_export_fig_file`, `designos_import_web_capture`).
  - `FACT`: Outside-workspace canary files remain byte-for-byte identical (verified via FIPS 180-4 SHA-256 digests before and after every refused call) across all 12 tests in `crates/designos-mcp/tests/mcp_paths.rs`.
- **Weakest link:** Bare `Engine::call("designos_export_fig_file", ...)` has no workspace root (unlike `Session` in `server.rs`), so it enforces name, extension, target-type, `overwrite`, and `O_NOFOLLOW` write checks on the path directly while workspace boundary enforcement lives in `Session`.

---

## 2. Acceptance Table (1–6)

| # | Gate | Result | Evidence |
|---|---|---|---|
| 1 | Red first: tests for every case in objectives 1–4 fail on `dc23fdd4` for the stated reason and pass on head (`red.log` and `green.log` saved) | **PASS** | `plans/reports/agy-1008-mcp-paths-evidence/red.log` (`0 passed; 12 failed`) and `plans/reports/agy-1008-mcp-paths-evidence/green.log` (`12 passed; 0 failed`) |
| 2 | Canary file outside the workspace is byte-identical after every refused call, shown by a test that hashes it before and after | **PASS** | `export_fig_refuses_symlink_to_outside_canary_and_leaves_canary_untouched`, `outside_workspace_refusal_never_echoes_path_and_preserves_canary`, `save_file_and_render_png_refuse_dangling_and_live_symlinks_and_preserve_canary`, `bare_engine_export_fig_enforces_all_target_checks` in `crates/designos-mcp/tests/mcp_paths.rs` |
| 3 | `cargo fmt --all -- --check` exits 0 | **PASS** | Exited `0` |
| 4 | `cargo clippy --workspace --all-targets --locked -- -D warnings` exits 0 | **PASS** | Exited `0` |
| 5 | `cargo test -p designos-mcp --locked` and `cargo test -p designos-eval --locked` pass (mandatory harness) | **PASS** | `designos-mcp`: 377 passed, 0 failed; `designos-eval`: 25 passed, 0 failed |
| 6 | Delete `out/` at the end | **PASS** | `rm -rf out && test ! -e out` exited `0` |

### Acceptance 1 & 2 — Red (`dc23fdd4d`) and Green (`HEAD`) Commands & Output

#### Red Run on `dc23fdd4d` (`plans/reports/agy-1008-mcp-paths-evidence/red.log`)

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test mcp_paths --locked > plans/reports/agy-1008-mcp-paths-evidence/red.log 2>&1
```

Output (`plans/reports/agy-1008-mcp-paths-evidence/red.log`):
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-mcp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 3.16s
     Running tests/mcp_paths.rs (out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c)

running 12 tests
test save_file_and_render_png_refuse_dangling_and_live_symlinks_and_preserve_canary ... FAILED
test export_fig_validates_extension_case_insensitively ... FAILED
test file_creation_tools_report_name_too_long_instead_of_file_exists ... FAILED
test file_creation_tools_refuse_bidi_override_characters_in_names ... FAILED
test outside_workspace_refusal_never_echoes_path_and_preserves_canary ... FAILED
test file_creation_tools_refuse_control_characters_in_names ... FAILED
test export_fig_refuses_folder ... FAILED
test export_fig_refuses_existing_file_unless_overwrite_is_true ... FAILED
test export_fig_refuses_symlink_to_outside_canary_and_leaves_canary_untouched ... FAILED
test export_fig_refuses_dangling_symlink_inside_and_outside_workspace ... FAILED
test export_fig_refuses_symlink_inside_workspace ... FAILED
test bare_engine_export_fig_enforces_all_target_checks ... FAILED

failures:

---- save_file_and_render_png_refuse_dangling_and_live_symlinks_and_preserve_canary stdout ----

thread 'save_file_and_render_png_refuse_dangling_and_live_symlinks_and_preserve_canary' (45442105) panicked at crates/designos-mcp/tests/mcp_paths.rs:639:13:
assertion `left == right` failed: live.designos overwrite=false: {"code":"FILE_EXISTS","corrected_example":{"overwrite":true,"path":"live.designos"},"error":{"code":"FILE_EXISTS","corrected_example":{"overwrite":true,"path":"live.designos"},"message":"Cannot create file (choose an unused .designos or .designos filename, or set overwrite=true): File exists (os error 17)","next_step":"Pass overwrite=true or choose a new path.","path":"/path","received":"live.designos","safe_to_retry":true},"message":"Cannot create file (choose an unused .designos or .designos filename, or set overwrite=true): File exists (os error 17)","next_step":"Pass overwrite=true or choose a new path.","path":"/path","received":"live.designos","safe_to_retry":true}
  left: FileExists
 right: InvalidArguments
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- export_fig_validates_extension_case_insensitively stdout ----

thread 'export_fig_validates_extension_case_insensitively' (45442100) panicked at crates/designos-mcp/tests/mcp_paths.rs:268:33:
UPPER.FIG should be accepted: {"code":"INVALID_ARGUMENTS","error":{"code":"INVALID_ARGUMENTS","expected":"*.fig","message":"Export path must end in .fig","next_step":"Adjust the value at `path` using `valid_options` or `corrected_example` and retry.","path":"/output_path","received":"UPPER.FIG","safe_to_retry":true},"expected":"*.fig","message":"Export path must end in .fig","next_step":"Adjust the value at `path` using `valid_options` or `corrected_example` and retry.","path":"/output_path","received":"UPPER.FIG","safe_to_retry":true}

---- file_creation_tools_report_name_too_long_instead_of_file_exists stdout ----

thread 'file_creation_tools_report_name_too_long_instead_of_file_exists' (45442103) panicked at crates/designos-mcp/tests/mcp_paths.rs:496:5:
assertion `left != right` failed: must not report FILE_EXISTS
  left: FileExists
 right: FileExists

---- file_creation_tools_refuse_bidi_override_characters_in_names stdout ----

thread 'file_creation_tools_refuse_bidi_override_characters_in_names' (45442101) panicked at crates/designos-mcp/tests/mcp_paths.rs:439:14:
save_file must refuse bidi char: Object {"path": String("/private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.IODkB1/designos-mcp-paths-ws-rt05-bidi-19153-1792896390673243000-7/rlo\u{202e}evil.designos"), "saved": Bool(true)}

---- outside_workspace_refusal_never_echoes_path_and_preserves_canary stdout ----

thread 'outside_workspace_refusal_never_echoes_path_and_preserves_canary' (45442104) panicked at crates/designos-mcp/tests/mcp_paths.rs:587:9:
designos_open_file {"path":"/etc/passwd"} must not set received on PATH_OUTSIDE_WORKSPACE: {"code":"PATH_OUTSIDE_WORKSPACE","error":{"code":"PATH_OUTSIDE_WORKSPACE","message":"Path must stay inside the configured workspace","next_step":"Pass a relative path inside the active workspace.","path":"/path","received":"/etc/passwd","safe_to_retry":true},"message":"Path must stay inside the configured workspace","next_step":"Pass a relative path inside the active workspace.","path":"/path","received":"/etc/passwd","safe_to_retry":true}

---- file_creation_tools_refuse_control_characters_in_names stdout ----

thread 'file_creation_tools_refuse_control_characters_in_names' (45442102) panicked at crates/designos-mcp/tests/mcp_paths.rs:374:14:
save_file must refuse control char: Object {"path": String("/private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.IODkB1/designos-mcp-paths-ws-rt05-ctrl-19153-1792896390673932000-10/line\nbreak.designos"), "saved": Bool(true)}

---- export_fig_refuses_folder stdout ----

thread 'export_fig_refuses_folder' (45442097) panicked at crates/designos-mcp/tests/mcp_paths.rs:245:9:
assertion `left == right` failed: {"output_path":"dir.fig"}: {"code":"INTERNAL_ERROR","error":{"code":"INTERNAL_ERROR","message":"Cannot write .fig file: Is a directory (os error 21)","next_step":"Call designos_get_document to inspect current state before retrying.","path":"/output_path","safe_to_retry":true},"message":"Cannot write .fig file: Is a directory (os error 21)","next_step":"Call designos_get_document to inspect current state before retrying.","path":"/output_path","safe_to_retry":true}
  left: InternalError
 right: InvalidArguments

---- export_fig_refuses_existing_file_unless_overwrite_is_true stdout ----

thread 'export_fig_refuses_existing_file_unless_overwrite_is_true' (45442096) panicked at crates/designos-mcp/tests/mcp_paths.rs:209:10:
existing .fig without overwrite must fail: Object {"bytes": Number(22649), "omitted_comment_threads": Number(0), "omitted_extensions": Array [], "output_path": String("/private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.IODkB1/designos-mcp-paths-ws-overwrite-19153-1792896390673102000-3/existing.fig"), "path": String("/private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.IODkB1/designos-mcp-paths-ws-overwrite-19153-1792896390673102000-3/existing.fig"), "saved": Bool(true)}

---- export_fig_refuses_symlink_to_outside_canary_and_leaves_canary_untouched stdout ----

thread 'export_fig_refuses_symlink_to_outside_canary_and_leaves_canary_untouched' (45442099) panicked at crates/designos-mcp/tests/mcp_paths.rs:130:14:
symlink at last path component must be refused: Object {"bytes": Number(22649), "omitted_comment_threads": Number(0), "omitted_extensions": Array [], "output_path": String("/private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.IODkB1/designos-mcp-paths-ws-sym-out-19153-1792896390673211000-5/evil.fig"), "path": String("/private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.IODkB1/designos-mcp-paths-ws-sym-out-19153-1792896390673211000-5/evil.fig"), "saved": Bool(true)}

---- export_fig_refuses_dangling_symlink_inside_and_outside_workspace stdout ----

thread 'export_fig_refuses_dangling_symlink_inside_and_outside_workspace' (45442095) panicked at crates/designos-mcp/tests/mcp_paths.rs:186:18:
dangling symlink must be refused: Object {"bytes": Number(22649), "omitted_comment_threads": Number(0), "omitted_extensions": Array [], "output_path": String("/private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.IODkB1/designos-mcp-paths-ws-sym-dangling-19153-1792896390673008000-1/dangling_out.fig"), "path": String("/private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.IODkB1/designos-mcp-paths-ws-sym-dangling-19153-1792896390673008000-1/dangling_out.fig"), "saved": Bool(true)}

---- export_fig_refuses_symlink_inside_workspace stdout ----

thread 'export_fig_refuses_symlink_inside_workspace' (45442098) panicked at crates/designos-mcp/tests/mcp_paths.rs:158:14:
in-workspace symlink must be refused: Object {"bytes": Number(22649), "omitted_comment_threads": Number(0), "omitted_extensions": Array [], "output_path": String("/private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.IODkB1/designos-mcp-paths-ws-sym-in-19153-1792896390673134000-4/link_in.fig"), "path": String("/private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.IODkB1/designos-mcp-paths-ws-sym-in-19153-1792896390673134000-4/link_in.fig"), "saved": Bool(true)}

---- bare_engine_export_fig_enforces_all_target_checks stdout ----

thread 'bare_engine_export_fig_enforces_all_target_checks' (45442094) panicked at crates/designos-mcp/tests/mcp_paths.rs:297:10:
bare Engine must refuse symlink: Object {"bytes": Number(22649), "omitted_comment_threads": Number(0), "omitted_extensions": Array [], "output_path": String("/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.IODkB1/designos-mcp-paths-engine-direct-19153-1792896390672957000-0/sym.fig"), "path": String("/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.IODkB1/designos-mcp-paths-engine-direct-19153-1792896390672957000-0/sym.fig"), "saved": Bool(true)}


failures:
    bare_engine_export_fig_enforces_all_target_checks
    export_fig_refuses_dangling_symlink_inside_and_outside_workspace
    export_fig_refuses_existing_file_unless_overwrite_is_true
    export_fig_refuses_folder
    export_fig_refuses_symlink_inside_workspace
    export_fig_refuses_symlink_to_outside_canary_and_leaves_canary_untouched
    export_fig_validates_extension_case_insensitively
    file_creation_tools_refuse_bidi_override_characters_in_names
    file_creation_tools_refuse_control_characters_in_names
    file_creation_tools_report_name_too_long_instead_of_file_exists
    outside_workspace_refusal_never_echoes_path_and_preserves_canary
    save_file_and_render_png_refuse_dangling_and_live_symlinks_and_preserve_canary

test result: FAILED. 0 passed; 12 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

error: test failed, to rerun pass `-p designos-mcp --test mcp_paths`
```

#### Green Run on `HEAD` (`plans/reports/agy-1008-mcp-paths-evidence/green.log`)

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test mcp_paths --locked > plans/reports/agy-1008-mcp-paths-evidence/green.log 2>&1
```

Output (`plans/reports/agy-1008-mcp-paths-evidence/green.log`):
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-mcp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.64s
     Running tests/mcp_paths.rs (out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c)

running 12 tests
test export_fig_refuses_symlink_inside_workspace ... ok
test export_fig_refuses_dangling_symlink_inside_and_outside_workspace ... ok
test export_fig_refuses_symlink_to_outside_canary_and_leaves_canary_untouched ... ok
test export_fig_refuses_folder ... ok
test save_file_and_render_png_refuse_dangling_and_live_symlinks_and_preserve_canary ... ok
test file_creation_tools_refuse_control_characters_in_names ... ok
test file_creation_tools_report_name_too_long_instead_of_file_exists ... ok
test file_creation_tools_refuse_bidi_override_characters_in_names ... ok
test outside_workspace_refusal_never_echoes_path_and_preserves_canary ... ok
test export_fig_refuses_existing_file_unless_overwrite_is_true ... ok
test export_fig_validates_extension_case_insensitively ... ok
test bare_engine_export_fig_enforces_all_target_checks ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
```

### Acceptance 3 — `cargo fmt --all -- --check`

Command:
```sh
cargo fmt --all -- --check && echo "FMT_CHECK=0"
```

Output:
```text
FMT_CHECK=0
```

### Acceptance 4 — `cargo clippy --workspace --all-targets --locked -- -D warnings`

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_DIR="$PWD/out/cargo"
cargo clippy --workspace --all-targets --locked -- -D warnings
echo "CLIPPY_EXIT=$?"
```

Output:
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
    Checking designos-doc v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-doc)
    Checking designos-fonts v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-fonts)
    Checking designos-render v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-render)
    Checking designos-diff v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-diff)
    Checking designos-svg v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-svg)
    Checking designos-export v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-export)
    Checking designos-webcapture v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-webcapture)
    Checking designos-fig v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-fig)
    Checking designos-library v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-library)
    Checking designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-mcp)
    Checking designos-eval v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-eval)
    Checking designos-gpu v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-gpu)
   Compiling designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 42.38s
CLIPPY_EXIT=0
```

### Acceptance 5 — `cargo test -p designos-mcp --locked` and `cargo test -p designos-eval --locked`

Command (`designos-mcp`):
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --locked
```

Output (`designos-mcp`):
```text
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_mcp-96fd55246d4a3a66)
test result: ok. 245 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.44s
     Running unittests src/main.rs (out/cargo/debug/deps/designos_mcp-616b53869353a607)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/bin/gen-tool-docs.rs (out/cargo/debug/deps/gen_tool_docs-f413a393b4d55d8f)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/colour_detach.rs (out/cargo/debug/deps/colour_detach-1aa6ab2233591601)
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
     Running tests/colour_var_library.rs (out/cargo/debug/deps/colour_var_library-01318f1c86eb2572)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
     Running tests/colour_var_refs.rs (out/cargo/debug/deps/colour_var_refs-f067c8c79555baaa)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/list_lint_rules.rs (out/cargo/debug/deps/list_lint_rules-378a191ea16cbc7f)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.54s
     Running tests/live_doc_epoch.rs (out/cargo/debug/deps/live_doc_epoch-23ae83d21f949530)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
     Running tests/live_session_guards.rs (out/cargo/debug/deps/live_session_guards-ab113f1c25ffacf0)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
     Running tests/mcp_paths.rs (out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s
     Running tests/session.rs (out/cargo/debug/deps/session-70f6c9de7d7f9973)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s
     Running tests/session_file_sites/main.rs (out/cargo/debug/deps/session_file_sites-1a885e87e98ff7af)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
     Running tests/session_files/main.rs (out/cargo/debug/deps/session_files-657090e65be613f1)
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
     Running tests/spec_text_colour.rs (out/cargo/debug/deps/spec_text_colour-75b28c66cba35b01)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
     Running tests/stdio.rs (out/cargo/debug/deps/stdio-9963b9496c82d7e4)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
     Running tests/text_fill.rs (out/cargo/debug/deps/text_fill-b3d88a68a195ca3e)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/tools_snapshot.rs (out/cargo/debug/deps/tools_snapshot-e6488781337fbb09)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
     Running tests/version.rs (out/cargo/debug/deps/version-7878cb959d478560)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
   Doc-tests designos_mcp
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Command (`designos-eval`):
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-eval --locked
```

Output (`designos-eval`):
```text
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_eval-9a9f1f166d66684f)

running 1 test
test hash::tests::fips_180_4_vectors ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/main.rs (out/cargo/debug/deps/designos_eval-30aca93b7d8d8c4a)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/harness.rs (out/cargo/debug/deps/harness-28beba16668beb59)

running 16 tests
test bootstrap_known_case ... ok
test split_fraction_within_bounds ... ok
test missing_register_refuses_the_run ... ok
test held_out_refused_in_tuning ... ok
test split_lock_mismatch_refused ... ok
test unapproved_source_refused ... ok
test bootstrap_is_reproducible ... ok
test chatty_agent_is_not_recorded_as_a_timeout ... ok
test unsaved_session_is_rebuilt_from_the_transcript ... ok
test noop_agent_is_not_a_render_success ... ok
test unimplemented_subcommands_fail_instead_of_pretending ... ok
test replay_cli_uses_the_current_engine_for_every_tool ... ok
test live_runs_wire_the_agent_to_designos_mcp_not_to_the_eval_binary ... ok
test stub_run_writes_valid_jsonl ... ok
test timed_out_agent_gets_sigterm_before_sigkill ... ok
test jobs_run_tasks_side_by_side_and_keep_record_order ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.65s

     Running tests/real_engine.rs (out/cargo/debug/deps/real_engine-4a6cab08ce582fe8)

running 8 tests
test started_timestamp_is_rfc3339_utc ... ok
test only_a_growing_undo_stack_counts_as_an_edit ... ok
test mcp_client_reports_a_failing_tool_as_a_result_not_a_transport_error ... ok
test transcript_logging_failure_fails_the_call ... ok
test replay_stops_with_replay_diverged_when_a_recorded_success_now_fails ... ok
test replay_runs_in_a_sandbox_prepared_like_the_run ... ok
test replay_follows_ids_and_creates_after_apply_spec_do_not_collide ... ok
test replay_reproduces_metrics_of_the_original_run ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.83s

   Doc-tests designos_eval

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### Acceptance 6 — Delete `out/` at the end

Command:
```sh
rm -rf out && test ! -e out && echo "OUT_REMOVED=0"
```

Output:
```text
OUT_REMOVED=0
```

---

## 3. Changes per File

- `crates/designos-mcp/src/tools/errors.rs`:
  - Updated `ToolError::with_received` and `ToolError::to_json_value` so `ErrorCode::PathOutsideWorkspace` never records or emits `received`.
- `crates/designos-mcp/src/engine.rs`:
  - Added `#[serde(default)] pub overwrite: bool` to `ExportFigArgs` (made `pub(crate)`) and added `"overwrite": {"type": "boolean", "default": false}` to `designos_export_fig_file`'s input schema in `engine::tools()`.
  - Added `has_control_or_bidi_char`, `validate_output_name`, `validate_existing_output_target`, and `write_checked_output_file` (which delegates to `crate::session_file_io::truncate(resolved, false)` and `crate::session_file_io::append(resolved, bytes, true)` and maps `session_file_io::refusal_of`).
  - Updated `"designos_export_fig_file" | "export_fig_file"` in `Engine::call` to validate output names, symlinks/folders/non-plain files, case-insensitive `.fig` extension, `overwrite`, and write via `write_checked_output_file`.
- `crates/designos-mcp/src/server.rs`:
  - Updated `input_path`, `output_path_allow_overwrite`, `png_output_path`, and `fig_output_path` to check workspace containment first (before any error branch that echoes the caller's path), validate output names (`validate_output_name`), inspect `symlink_metadata` on both the uncanonicalized requested path and resolved target (`validate_existing_output_target`), and compare `.fig` extensions case-insensitively.
  - Updated `"designos_save_file"`, `"designos_render_png"`, and `"designos_export_fig_file" | "export_fig_file"` in `Session::call_inner` to write exclusively through `crate::engine::write_checked_output_file`.
- `crates/designos-mcp/src/rules.rs`:
  - Updated `tool_error_codes("designos_export_fig_file")` to `&["\`INVALID_ARGUMENTS\`", "\`PATH_OUTSIDE_WORKSPACE\`", "\`FILE_EXISTS\`", "\`TOOL_DISABLED\`"]` (matching `designos_save_file` and staying within the 24,000-char rules pack budget).
- `crates/designos-mcp/tests/snapshots/tools.json`, `crates/designos-mcp/generated/rules.md`, `skills/design-os-studio/rules.md`:
  - Updated via `cargo run -p designos-mcp --bin gen-tool-docs --locked` solely for `designos_export_fig_file`'s new `overwrite` property and `FILE_EXISTS` error code (required by `tests/tools_snapshot.rs::docs_blocks_are_current` and `rules::tests::prompt_resource_tool_identical`).
- `crates/designos-mcp/tests/mcp_paths.rs`:
  - Added 12 red-first integration tests covering all cases in Objectives 1–4 and Acceptance #2 (outside-workspace canary SHA-256 verification).
- `plans/reports/agy-1008-mcp-paths-evidence/red.log` & `plans/reports/agy-1008-mcp-paths-evidence/green.log`:
  - Saved verbatim red (`dc23fdd4d`) and green (`HEAD`) test outputs.

---

## 4. Every Error String Added, Verbatim

1. `"{display_path} has a name with a line break or a direction-control character. Rename it, then try again."` (`ErrorCode::InvalidArguments`)
2. `"{display_path} has a name longer than 255 bytes. Use a shorter name."` (`ErrorCode::InvalidArguments`)
3. `"{display_path} is a link, not a file. Delete the link, then try again."` (`ErrorCode::InvalidArguments`)
4. `"{display_path} is a folder, not a file. Choose a file path, then try again."` (`ErrorCode::InvalidArguments`)
5. `"{display_path} is not a plain file. Delete it, then try again."` (`ErrorCode::InvalidArguments`)
6. `"{display_path} already exists. Pass overwrite: true to replace it."` (`ErrorCode::FileExists`)
7. `"{display_path} already exists. Choose a new path, then try again."` (`ErrorCode::FileExists`, for tools without an `overwrite` parameter such as `designos_render_png`)
8. `"{display_path} is read-only. Make it writable first, or choose a new path."` (`ErrorCode::InvalidArguments`)
9. `"{display_path} has a second name (a hard link). Delete it, then try again."` (`ErrorCode::InvalidArguments`)
10. `"{display_path} can't be written: {e}. Check its permissions and free space, then try again."` (`ErrorCode::InternalError`)
11. `"{path} does not end with .fig. Choose a path ending with .fig, then try again."` / `"{} does not end with .fig. Choose a path ending with .fig, then try again."` (`ErrorCode::InvalidArguments`)

---

## 5. Findings Outside Scope

- **#1074 (`crates/designos-app/src/terminal.rs`):** When a document has not been saved to disk yet, Studio launches the agent terminal with `/` as the workspace root. Per task instructions, `#1074` is context-only and owned by another worker in `crates/designos-app/`.
- **`designos_save_file` and `designos_render_png` extension case-sensitivity:** Objective 1 specifically requested case-insensitive `.fig` extension comparison on `designos_export_fig_file`. `output_path_allow_overwrite` (`.designos`) and `png_output_path` (`.png`) still use exact lowercase extension checks, left unchanged to avoid unrequested behavior changes outside scope.

---

## 6. Assumptions

- **`ASSUMPTION` (verified against `rules.rs` and `tools_snapshot.rs`):** In `crates/designos-mcp/src/rules.rs`, `tool_error_codes("designos_export_fig_file")` replaced `` "`INTERNAL_ERROR`" `` with `` "`FILE_EXISTS`" `` (matching `tool_error_codes("designos_save_file")`, which lists `INVALID_ARGUMENTS`, `PATH_OUTSIDE_WORKSPACE`, `FILE_EXISTS`, `TOOL_DISABLED`) because adding both `(optional: \`overwrite\`)` (+23 chars) and `, \`FILE_EXISTS\`` (+15 chars) while retaining `\`INTERNAL_ERROR\`` would push `generated/rules.md` to `24,010` chars (`> MAX_RULES_CHARS = 24,000`). With this alignment, `generated/rules.md` is `23,992` chars (`<= 24,000`).
- **`ASSUMPTION` (verified against `tools_snapshot.rs`):** Running `cargo run -p designos-mcp --bin gen-tool-docs` updates `crates/designos-mcp/tests/snapshots/tools.json` together with `crates/designos-mcp/generated/rules.md` and `skills/design-os-studio/rules.md`, which `tests/tools_snapshot.rs::docs_blocks_are_current` and `src/rules.rs::tests::prompt_resource_tool_identical` assert are byte-for-byte identical to `generate_default_rules_pack()`.

---

## Round 2

### 1. What Could NOT Be Verified

- **Windows cross-target check (`x86_64-pc-windows-gnu`):** Running `cargo check -p designos-mcp --tests --target x86_64-pc-windows-gnu --locked` on this macOS worker stops at `cfg-if`/`core` with `error[E0463]: can't find crate for core` (`the x86_64-pc-windows-gnu target may not be installed`). Adding `#![cfg(unix)]` at line 1 of `crates/designos-mcp/tests/mcp_paths.rs` gates the entire unix-specific integration test crate off on Windows so Windows CI does not compile `std::os::unix` imports or `libc::mkfifo`/`RLIMIT_FSIZE`.

---

### 2. Acceptance Table & Command Outputs

| # | Gate | Command | Status |
|---|---|---|---|
| 1 | Red / Green (`r2-red.log` -> `r2-green.log`) | `cargo test -p designos-mcp --test mcp_paths --locked` on `f2b533c1` (8 Round 2 tests fail) and on `HEAD` (20/20 pass) | PASS |
| 2 | Rustfmt | `cargo fmt --all -- --check` | PASS (`exit 0`) |
| 3 | Clippy | `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS (`exit 0`) |
| 4 | Test suites (`designos-mcp` & `designos-eval`) | `cargo test -p designos-mcp --locked` and `cargo test -p designos-eval --locked` under short-`$TMPDIR` harness | PASS (`exit 0`) |
| 5 | Windows target check (`x86_64-pc-windows-gnu`) | `cargo check -p designos-mcp --tests --target x86_64-pc-windows-gnu --locked` | Target stdlib not installed locally (`#![cfg(unix)]` added at top of `tests/mcp_paths.rs`) |
| 6 | Clean workspace (`out/` removed) | `rm -rf out && test ! -e out` | PASS (`exit 0`) |

#### Acceptance 1 — Red / Green (`r2-red.log` and `r2-green.log`)

- Full logs:
  - `plans/reports/agy-1008-mcp-paths-evidence/r2-red.log`
  - `plans/reports/agy-1008-mcp-paths-evidence/r2-green.log`

Command (`r2-red.log` against `f2b533c1` `src/`):
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test mcp_paths --locked > plans/reports/agy-1008-mcp-paths-evidence/r2-red.log 2>&1 || true
rm -rf "$S"
```

Output (`r2-red.log` summary):
```text
thread 'export_fig_rules_list_both_file_exists_and_internal_error' (47039331) panicked at crates/designos-mcp/tests/mcp_paths.rs:677:5:
designos_export_fig_file rules must list INTERNAL_ERROR: ["`INVALID_ARGUMENTS`", "`PATH_OUTSIDE_WORKSPACE`", "`FILE_EXISTS`", "`TOOL_DISABLED`"]

thread 'argument_errors_never_echo_outside_paths_in_corrected_example_or_received' (47039323) panicked at crates/designos-mcp/tests/mcp_paths.rs:695:9:
designos_render_png {"bogus":1,"node_id":1,"path":"/etc/SECRETNAME.png"} leaked outside path in argument error reply: ...

thread 'in_workspace_absolute_paths_via_symlink_and_var_alias_are_accepted' (47039337) panicked at crates/designos-mcp/tests/mcp_paths.rs:662:33:
save_file via alias (/var/folders/.../alias/alias.designos) must succeed: {"code":"PATH_OUTSIDE_WORKSPACE",...}

thread 'new_files_respect_umask_0644_and_255_byte_names_work_in_both_modes' (47039338) panicked at crates/designos-mcp/tests/mcp_paths.rs:608:9:
assertion `left == right` failed: a.designos mode was 600, expected 644 (not 0600)
  left: 384
 right: 420

thread 'concurrent_no_overwrite_saves_never_both_succeed_or_corrupt_json' (47039325) panicked at crates/designos-mcp/tests/mcp_paths.rs:497:5:
assertion `left == right` failed: two concurrent no-overwrite saves must never both succeed (both_ok=298/300)
  left: 298
 right: 0

thread 'file_planted_during_no_overwrite_export_is_never_overwritten' (47039336) panicked at crates/designos-mcp/tests/mcp_paths.rs:535:5:
assertion `left == right` failed: a file planted between check and write must never be overwritten without overwrite: true (lost=276/300)
  left: 276
 right: 0

thread 'partial_write_failure_preserves_original_leaves_no_partial_file_and_reports_honest_retry' (47039351) panicked at crates/designos-mcp/tests/mcp_paths.rs:577:5:
failed write must not leave a partial new.designos on disk

thread 'trailing_slash_is_refused_as_folder_and_extension_messages_put_cause_first' (47039354) panicked at crates/designos-mcp/tests/mcp_paths.rs:716:41:
trailing slash must be refused as folder: Object {"bytes": Number(22756), ..., "saved": Bool(true)}

failures:
    argument_errors_never_echo_outside_paths_in_corrected_example_or_received
    concurrent_no_overwrite_saves_never_both_succeed_or_corrupt_json
    export_fig_rules_list_both_file_exists_and_internal_error
    file_planted_during_no_overwrite_export_is_never_overwritten
    in_workspace_absolute_paths_via_symlink_and_var_alias_are_accepted
    new_files_respect_umask_0644_and_255_byte_names_work_in_both_modes
    partial_write_failure_preserves_original_leaves_no_partial_file_and_reports_honest_retry
    trailing_slash_is_refused_as_folder_and_extension_messages_put_cause_first

test result: FAILED. 12 passed; 8 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.65s
```

Command (`r2-green.log` on `HEAD`):
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test mcp_paths --locked > plans/reports/agy-1008-mcp-paths-evidence/r2-green.log 2>&1
rm -rf "$S"
```

Output (`r2-green.log`):
```text
running 20 tests
test export_fig_rules_list_both_file_exists_and_internal_error ... ok
test export_fig_refuses_folder ... ok
test export_fig_refuses_symlink_inside_workspace ... ok
test export_fig_refuses_dangling_symlink_inside_and_outside_workspace ... ok
test export_fig_refuses_symlink_to_outside_canary_and_leaves_canary_untouched ... ok
test argument_errors_never_echo_outside_paths_in_corrected_example_or_received ... ok
test file_creation_tools_report_name_too_long_instead_of_file_exists ... ok
test file_creation_tools_refuse_control_characters_in_names ... ok
test file_creation_tools_refuse_bidi_override_characters_in_names ... ok
test export_fig_refuses_existing_file_unless_overwrite_is_true ... ok
test outside_workspace_refusal_never_echoes_path_and_preserves_canary ... ok
test export_fig_validates_extension_case_insensitively ... ok
test bare_engine_export_fig_enforces_all_target_checks ... ok
test in_workspace_absolute_paths_via_symlink_and_var_alias_are_accepted ... ok
test new_files_respect_umask_0644_and_255_byte_names_work_in_both_modes ... ok
test concurrent_no_overwrite_saves_never_both_succeed_or_corrupt_json ... ok
test file_planted_during_no_overwrite_export_is_never_overwritten ... ok
test partial_write_failure_preserves_original_leaves_no_partial_file_and_reports_honest_retry ... ok
test trailing_slash_is_refused_as_folder_and_extension_messages_put_cause_first ... ok
test save_file_and_render_png_refuse_dangling_and_live_symlinks_and_preserve_canary ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.96s
```

#### Acceptance 2 — `cargo fmt --all -- --check`

Command:
```sh
cargo fmt --all -- --check && echo "FMT_CHECK=0"
```

Output:
```text
FMT_CHECK=0
```

#### Acceptance 3 — `cargo clippy --workspace --all-targets --locked -- -D warnings`

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings && echo "CLIPPY_EXIT=0"
rm -rf "$S"
```

Output:
```text
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-mcp)
    Checking designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.90s
CLIPPY_EXIT=0
```

#### Acceptance 4 — `cargo test -p designos-mcp --locked` and `cargo test -p designos-eval --locked`

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --locked
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-eval --locked
rm -rf "$S"
```

Output (`designos-mcp` & `designos-eval`):
```text
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_mcp-96fd55246d4a3a66)
test result: ok. 245 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.67s
     Running tests/mcp_paths.rs (out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c)
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.71s
     Running tests/session_file_safety.rs (out/cargo/debug/deps/session_file_safety-...)
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
     Running tests/session_file_sites.rs (out/cargo/debug/deps/session_file_sites-...)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/tools_snapshot.rs (out/cargo/debug/deps/tools_snapshot-e6488781337fbb09)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
     Running tests/version.rs (out/cargo/debug/deps/version-7878cb959d478560)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

     Running unittests src/lib.rs (out/cargo/debug/deps/designos_eval-9a9f1f166d66684f)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/harness.rs (out/cargo/debug/deps/harness-28beba16668beb59)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.15s
     Running tests/real_engine.rs (out/cargo/debug/deps/real_engine-4a6cab08ce582fe8)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
```

#### Acceptance 5 — `cargo check -p designos-mcp --tests --target x86_64-pc-windows-gnu`

Command:
```sh
cargo check -p designos-mcp --tests --target x86_64-pc-windows-gnu --locked
```

Output:
```text
error[E0463]: can't find crate for `core`
  |
  = note: the `x86_64-pc-windows-gnu` target may not be installed
  = help: consider downloading the target with `rustup target add x86_64-pc-windows-gnu`
```

---

### 3. Changes per File (Round 2)

- `crates/designos-mcp/src/engine.rs`:
  - Consolidated output target validation into `validate_existing_output_target(target_meta, display_path, ptr, ext, overwrite, supports_overwrite)` called by both `Engine::call` and `Session` (`output_path_allow_overwrite`, `png_output_path`, `fig_output_path`). It validates `validate_output_name`, refuses symlinks, refuses folders and paths ending in `/` (or `\.` / `.`), checks the required extension (`.designos`, `.png`, or case-insensitive `.fig`) with cause-first messages, and refuses existing files (when `!overwrite`), hard links (`nlink > 1`), non-plain files (FIFOs/sockets/devices), and read-only files.
  - Updated `write_checked_output_file(resolved, display_path, ptr, bytes, overwrite)`:
    - When `overwrite == false`: opens `resolved` with `OpenOptions::new().write(true).create_new(true)` (`O_CREAT | O_EXCL`) and `O_NOFOLLOW` on unix (using the process's normal umask `0644`, not `0600`), maps `ErrorKind::AlreadyExists` to `ErrorCode::FileExists`, and if `write_all`/`sync_all` fails, drops the handle, removes the newly created file, and sets `safe_to_retry` to whether the partial file was removed.
    - When `overwrite == true`: checks `symlink_metadata(resolved)` and refuses symlinks, folders, non-plain files, hard links (`nlink > 1`), and read-only files; opens a short-named sibling temp file `.designos-out-{pid}-{seq}.tmp` in the same directory with `create_new(true)` + `O_NOFOLLOW` (so 255-byte target filenames never exceed `NAME_MAX`), copies `old.permissions()` when replacing an existing file (or respects default umask `0644` when creating a new file), calls `write_all` + `sync_all`, and atomically renames `&tmp` over `resolved`, unlinking `&tmp` on any failure so the original file is never destroyed and no partial file remains.
- `crates/designos-mcp/src/server.rs`:
  - Deleted the three pre-check blocks in `output_path_allow_overwrite`, `png_output_path`, and `fig_output_path` that called `Path::new(path).canonicalize()` on a not-yet-existing file (which rejected valid in-workspace absolute paths through symlink aliases or macOS `/var` vs `/private/var`).
  - Delegated all output target checks in `output_path_allow_overwrite`, `png_output_path`, and `fig_output_path` to `crate::engine::validate_existing_output_target`.
  - Updated `call_with_safety` so `safe = err.safe_to_retry && (after == before) && (files_created == 0)`.
  - Passed `a.overwrite`, `false`, and `a.overwrite` into `write_checked_output_file` for `designos_save_file`, `designos_render_png`, and `designos_export_fig_file`.
- `crates/designos-mcp/src/tools/errors.rs`:
  - Updated `build_unknown_field_corrected_example` and `contextualize_corrected_example` to strip `"path"` and `"output_path"` from `corrected_example` and `received` on argument errors (`err.code != ErrorCode::FileExists && err.code != ErrorCode::UnsavedChanges`), and prevented `contextualize_corrected_example` from re-inserting unknown fields or path arguments on `ErrorCode::UnknownField`.
- `crates/designos-mcp/src/rules.rs`, `crates/designos-mcp/generated/rules.md`, `skills/design-os-studio/rules.md`:
  - Restored `` "`INTERNAL_ERROR`" `` alongside `` "`FILE_EXISTS`" `` in `tool_error_codes("designos_export_fig_file")`, adjusted `MAX_PURPOSE_CHARS` from `106` to `104` to keep `generated/rules.md` within the 24,000-character budget (`23,972` chars), and regenerated the rules files via `gen-tool-docs`.
- `crates/designos-mcp/tests/session_file_sites/allowlist.rs`:
  - Added the single allowlist entry `Allowed { file: "src/engine.rs", func: Some("write_checked_output_file"), why: WORKSPACE_PATH },`.
- `crates/designos-mcp/tests/mcp_paths.rs`:
  - Added `#![cfg(unix)]` at the top of the test file and added 8 Round 2 tests (`20` tests total).

---

### 4. User-Facing Strings Added or Changed in Round 2 (Verbatim) & Full Refused Control / Bidi Set

#### User-Facing Strings Added or Changed
1. `"{display_path} does not end with .designos. Choose a path ending with .designos, then try again."` (`ErrorCode::InvalidArguments`, replaces `"Save path must end in .designos"`)
2. `"{display_path} does not end with .png. Choose a path ending with .png, then try again."` (`ErrorCode::InvalidArguments`, replaces `"Render path must end in .png"`)
3. `"{display_path} does not end with .fig. Choose a path ending with .fig, then try again."` (`ErrorCode::InvalidArguments`, unified across `Engine` and `Session` via `"{display_path} does not end with .{ext}. Choose a path ending with .{ext}, then try again."`)
4. `"{display_path} is a folder, not a file. Choose a file path, then try again."` (`ErrorCode::InvalidArguments`, now also returned for paths ending with `/`, `/.`, or `.`)

#### Full Refused Control & Bidirectional Character Set (`has_control_or_bidi_char`)
- **Unicode Cc control characters (`char::is_control()`):**
  - `U+0000..=U+001F` (C0 controls, including `\0`, `\t`, `\n`, `\r`, `\x1B`)
  - `U+007F` (`DELETE`)
  - `U+0080..=U+009F` (C1 controls)
- **Bidirectional embedding & override controls:**
  - `U+202A` (`LEFT-TO-RIGHT EMBEDDING`, `LRE`)
  - `U+202B` (`RIGHT-TO-LEFT EMBEDDING`, `RLE`)
  - `U+202C` (`POP DIRECTIONAL FORMATTING`, `PDF`)
  - `U+202D` (`LEFT-TO-RIGHT OVERRIDE`, `LRO`)
  - `U+202E` (`RIGHT-TO-LEFT OVERRIDE`, `RLO`)
- **Bidirectional isolate controls:**
  - `U+2066` (`LEFT-TO-RIGHT ISOLATE`, `LRI`)
  - `U+2067` (`RIGHT-TO-LEFT ISOLATE`, `RLI`)
  - `U+2068` (`FIRST STRONG ISOLATE`, `FSI`)
  - `U+2069` (`POP DIRECTIONAL ISOLATE`, `PDI`)
- **Implicit directional marks (retained per Ruling 8):**
  - `U+200E` (`LEFT-TO-RIGHT MARK`, `LRM`)
  - `U+200F` (`RIGHT-TO-LEFT MARK`, `RLM`)
  - `U+061C` (`ARABIC LETTER MARK`, `ALM`)

---

## Round 3

### 1. What could NOT be verified

- **Nothing in the assigned Round 3 scope was left unverified locally.** Cross-OS runtime execution on Windows and Ubuntu is handled by GitHub Actions CI (`#![cfg(unix)]` is at line 1 of both `crates/designos-mcp/tests/mcp_paths.rs` and `crates/designos-mcp/tests/mcp_paths_full_disk.rs`).
- **Conclusion (`FACT`):**
  - `FACT`: Reproduced finding **N-10** on `d044dca4` before editing: building `mcp_paths` (`out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c`) and running it at the default thread count hung on run 2 (`rc=142`, `SIGALRM`) because six tests in `mcp_paths.rs` called `TempDir::new` twice in one function while `partial_write_failure_preserves_original_leaves_no_partial_file_and_reports_honest_retry` queued `RLIMIT_LOCK.write()` in between.
  - `FACT`: Moved `partial_write_failure_preserves_original_leaves_no_partial_file_and_reports_honest_retry` into its own `#![cfg(unix)]` integration test binary `crates/designos-mcp/tests/mcp_paths_full_disk.rs` (running in its own process) and deleted `RLIMIT_LOCK` and its guard from `TempDir` in `crates/designos-mcp/tests/mcp_paths.rs`.
  - `FACT`: In `crates/designos-mcp/src/engine.rs`, updated `write_checked_output_file` and `validate_existing_output_target` for **N-1** so that when `designos_render_png` hits an existing `.png` file or loses a race at `create_new`, it returns the base (`dc23fdd4`) message `format!("Cannot create PNG file (choose an unused .png filename): {e}")` without mentioning `overwrite` or setting `corrected_example`, while tools that support `overwrite` (`designos_save_file`, `designos_export_fig_file`) keep `format!("{display_path} already exists. Pass overwrite: true to replace it.")` with `corrected_example: Some(json!({ field_name: display_path, "overwrite": true }))`.
  - `FACT`: Added `render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint` in `crates/designos-mcp/tests/mcp_paths.rs`, verified it fails on `d044dca4` (`plans/reports/agy-1008-mcp-paths-evidence/r3-red.log`) and passes on `HEAD` (`plans/reports/agy-1008-mcp-paths-evidence/r3-green.log`), and ran 40 back-to-back runs of `mcp_paths` (`ok=40 hang=0 fail=0`) plus 20 back-to-back runs of `mcp_paths_full_disk` (`ok=20 hang=0 fail=0`) at the default thread count under a 60 s timeout (`plans/reports/agy-1008-mcp-paths-evidence/r3-loop.log`).
- **Weakest link:** `write_checked_output_file` infers `supports_overwrite` from whether `display_path`'s extension is `.png` because `server.rs` is outside the Round 3 allowed-files list and every caller validates the required extension (`.designos`, `.png`, `.fig`) before calling `write_checked_output_file`.
- **Assumptions (`ASSUMPTION`):**
  - `ASSUMPTION` (verified against `server.rs` and `engine.rs`): All three callers of `write_checked_output_file` (`designos_save_file`, `designos_render_png`, `designos_export_fig_file`) run `validate_existing_output_target` first, which guarantees `display_path` has extension `.designos`, `.png`, or `.fig` before `write_checked_output_file` executes.
- **Unresolved questions:** None.

---

### 2. Acceptance Table & Command Outputs

| # | Gate | Command | Result |
|---|---|---|---|
| 1 | 40x `mcp_paths` loop + 20x `mcp_paths_full_disk` loop at default thread count under 60 s timeout (`r3-loop.log`) | `cargo test -p designos-mcp --test mcp_paths --no-run --locked` + 40 runs; `cargo test -p designos-mcp --test mcp_paths_full_disk --no-run --locked` + 20 runs | **PASS** (`mcp_paths`: `runs=40 ok=40 hang=0 fail=0`; `mcp_paths_full_disk`: `runs=20 ok=20 hang=0 fail=0`) |
| 2 | N-1 test fails on `d044dca4` (`r3-red.log`) and passes on head (`r3-green.log`) | `cargo test -p designos-mcp --test mcp_paths --locked` | **PASS** (`r3-red.log`: `19 passed; 1 failed`; `r3-green.log`: `20 passed; 0 failed`) |
| 3 | `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --locked -- -D warnings` | `cargo fmt --all -- --check` && `cargo clippy --workspace --all-targets --locked -- -D warnings` | **PASS** (`FMT_EXIT=0`, `CLIPPY_EXIT=0`) |
| 4 | `cargo test -p designos-mcp --locked` and `cargo test -p designos-eval --locked` | Both run inside mandatory short-`$TMPDIR` harness | **PASS** (`MCP_TEST_EXIT=0`, `EVAL_TEST_EXIT=0`) |
| 5 | Commit, push, update PR #1103 body with Round 3 line, keep draft, delete `out/` | `rm -rf out && test ! -e out` | **PASS** (`OUT_REMOVED=0`) |

#### N-10 Reproduction on `d044dca4` (Before Fix)

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
BIN=$(env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test mcp_paths --no-run --locked 2>&1 | sed -n 's/.*Executable tests\/mcp_paths.rs (\(.*\))/\1/p')
echo "BIN=$BIN"
for i in $(seq 1 20); do
  if perl -e 'alarm 15; exec @ARGV' env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" "$BIN" >/dev/null 2>&1; then
    echo "run $i: ok"
  else
    rc=$?
    echo "run $i: HUNG/FAILED rc=$rc"
    break
  fi
done
rm -rf "$S"
```

Output:
```text
BIN=out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c
run 1: ok
run 2: HUNG/FAILED rc=142
```

#### Acceptance 1 — 40x `mcp_paths` & 20x `mcp_paths_full_disk` Loops (`plans/reports/agy-1008-mcp-paths-evidence/r3-loop.log`)

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
LOG="plans/reports/agy-1008-mcp-paths-evidence/r3-loop.log"
: > "$LOG"
BUILD_PATHS=$(env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test mcp_paths --no-run --locked 2>&1)
echo "$BUILD_PATHS" >> "$LOG"
BIN_PATHS=$(printf '%s\n' "$BUILD_PATHS" | sed -n 's/.*Executable tests\/mcp_paths.rs (\(.*\))/\1/p')
echo "BIN_PATHS=$BIN_PATHS" >> "$LOG"

ok=0; hang=0; fail=0
for i in $(seq 1 40); do
  OUT=$(perl -e 'alarm 60; exec @ARGV' env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" "$BIN_PATHS" 2>&1)
  rc=$?
  RES=$(printf '%s\n' "$OUT" | grep "test result:" || true)
  if [ "$rc" -eq 0 ]; then
    ok=$((ok + 1))
    echo "mcp_paths run $i/40: rc=0 $RES" >> "$LOG"
  elif [ "$rc" -eq 142 ]; then
    hang=$((hang + 1))
    echo "mcp_paths run $i/40: HUNG rc=$rc" >> "$LOG"
  else
    fail=$((fail + 1))
    echo "mcp_paths run $i/40: FAIL rc=$rc $RES" >> "$LOG"
  fi
done
echo "mcp_paths summary: runs=40 ok=$ok hang=$hang fail=$fail" >> "$LOG"

BUILD_FD=$(env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test mcp_paths_full_disk --no-run --locked 2>&1)
echo "$BUILD_FD" >> "$LOG"
BIN_FD=$(printf '%s\n' "$BUILD_FD" | sed -n 's/.*Executable tests\/mcp_paths_full_disk.rs (\(.*\))/\1/p')
echo "BIN_FD=$BIN_FD" >> "$LOG"

fd_ok=0; fd_hang=0; fd_fail=0
for i in $(seq 1 20); do
  OUT=$(perl -e 'alarm 60; exec @ARGV' env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" "$BIN_FD" 2>&1)
  rc=$?
  RES=$(printf '%s\n' "$OUT" | grep "test result:" || true)
  if [ "$rc" -eq 0 ]; then
    fd_ok=$((fd_ok + 1))
    echo "mcp_paths_full_disk run $i/20: rc=0 $RES" >> "$LOG"
  elif [ "$rc" -eq 142 ]; then
    fd_hang=$((fd_hang + 1))
    echo "mcp_paths_full_disk run $i/20: HUNG rc=$rc" >> "$LOG"
  else
    fd_fail=$((fd_fail + 1))
    echo "mcp_paths_full_disk run $i/20: FAIL rc=$rc $RES" >> "$LOG"
  fi
done
echo "mcp_paths_full_disk summary: runs=20 ok=$fd_ok hang=$fd_hang fail=$fd_fail" >> "$LOG"
rm -rf "$S"
```

Output (`plans/reports/agy-1008-mcp-paths-evidence/r3-loop.log`):
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-mcp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.22s
  Executable tests/mcp_paths.rs (out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c)
BIN_PATHS=out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c
mcp_paths run 1/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.08s
mcp_paths run 2/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.08s
mcp_paths run 3/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.03s
mcp_paths run 4/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.05s
mcp_paths run 5/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.09s
mcp_paths run 6/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.95s
mcp_paths run 7/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.74s
mcp_paths run 8/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.00s
mcp_paths run 9/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.06s
mcp_paths run 10/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.02s
mcp_paths run 11/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.04s
mcp_paths run 12/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.99s
mcp_paths run 13/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.01s
mcp_paths run 14/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.99s
mcp_paths run 15/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.98s
mcp_paths run 16/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.74s
mcp_paths run 17/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.02s
mcp_paths run 18/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.80s
mcp_paths run 19/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.84s
mcp_paths run 20/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.70s
mcp_paths run 21/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.85s
mcp_paths run 22/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.63s
mcp_paths run 23/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.75s
mcp_paths run 24/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.84s
mcp_paths run 25/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.93s
mcp_paths run 26/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.97s
mcp_paths run 27/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.77s
mcp_paths run 28/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.84s
mcp_paths run 29/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.22s
mcp_paths run 30/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.02s
mcp_paths run 31/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.13s
mcp_paths run 32/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.97s
mcp_paths run 33/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.90s
mcp_paths run 34/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.00s
mcp_paths run 35/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.12s
mcp_paths run 36/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.82s
mcp_paths run 37/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.06s
mcp_paths run 38/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.92s
mcp_paths run 39/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.94s
mcp_paths run 40/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.00s
mcp_paths summary: runs=40 ok=40 hang=0 fail=0
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-mcp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 3.53s
  Executable tests/mcp_paths_full_disk.rs (out/cargo/debug/deps/mcp_paths_full_disk-a818e059bf9cad4b)
BIN_FD=out/cargo/debug/deps/mcp_paths_full_disk-a818e059bf9cad4b
mcp_paths_full_disk run 1/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
mcp_paths_full_disk run 2/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 3/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 4/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 5/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 6/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 7/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 8/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 9/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 10/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 11/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 12/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 13/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 14/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 15/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 16/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 17/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 18/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 19/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 20/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk summary: runs=20 ok=20 hang=0 fail=0
```

#### Acceptance 2 — N-1 Red (`d044dca4`) and Green (`HEAD`) (`r3-red.log` and `r3-green.log`)

Command (`r3-red.log` against `d044dca4` `src/`):
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test mcp_paths --locked > plans/reports/agy-1008-mcp-paths-evidence/r3-red.log 2>&1 || true
rm -rf "$S"
```

Output (`plans/reports/agy-1008-mcp-paths-evidence/r3-red.log`):
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-mcp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 4.04s
     Running tests/mcp_paths.rs (out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c)

running 20 tests
test export_fig_rules_list_both_file_exists_and_internal_error ... ok
test export_fig_refuses_folder ... ok
test argument_errors_never_echo_outside_paths_in_corrected_example_or_received ... ok
test export_fig_refuses_symlink_to_outside_canary_and_leaves_canary_untouched ... ok
test export_fig_refuses_dangling_symlink_inside_and_outside_workspace ... ok
test export_fig_refuses_symlink_inside_workspace ... ok
test file_creation_tools_report_name_too_long_instead_of_file_exists ... ok
test file_creation_tools_refuse_control_characters_in_names ... ok
test file_creation_tools_refuse_bidi_override_characters_in_names ... ok
test trailing_slash_is_refused_as_folder_and_extension_messages_put_cause_first ... ok
test render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint ... FAILED
test export_fig_refuses_existing_file_unless_overwrite_is_true ... ok
test export_fig_validates_extension_case_insensitively ... ok
test outside_workspace_refusal_never_echoes_path_and_preserves_canary ... ok
test save_file_and_render_png_refuse_dangling_and_live_symlinks_and_preserve_canary ... ok
test bare_engine_export_fig_enforces_all_target_checks ... ok
test in_workspace_absolute_paths_via_symlink_and_var_alias_are_accepted ... ok
test new_files_respect_umask_0644_and_255_byte_names_work_in_both_modes ... ok
test concurrent_no_overwrite_saves_never_both_succeed_or_corrupt_json ... ok
test file_planted_during_no_overwrite_export_is_never_overwritten ... ok

failures:

---- render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint stdout ----

thread 'render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint' (48115139) panicked at crates/designos-mcp/tests/mcp_paths.rs:571:13:
render_png has no overwrite option and must never tell the caller to pass overwrite: png-race-0.png already exists. Pass overwrite: true to replace it.
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint

test result: FAILED. 19 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.91s

error: test failed, to rerun pass `-p designos-mcp --test mcp_paths`
```

Command (`r3-green.log` on `HEAD`):
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test mcp_paths --locked > plans/reports/agy-1008-mcp-paths-evidence/r3-green.log 2>&1
rm -rf "$S"
```

Output (`plans/reports/agy-1008-mcp-paths-evidence/r3-green.log`):
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-mcp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 4.72s
     Running tests/mcp_paths.rs (out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c)

running 20 tests
test export_fig_rules_list_both_file_exists_and_internal_error ... ok
test export_fig_refuses_symlink_to_outside_canary_and_leaves_canary_untouched ... ok
test export_fig_refuses_dangling_symlink_inside_and_outside_workspace ... ok
test export_fig_refuses_folder ... ok
test export_fig_refuses_symlink_inside_workspace ... ok
test argument_errors_never_echo_outside_paths_in_corrected_example_or_received ... ok
test file_creation_tools_report_name_too_long_instead_of_file_exists ... ok
test file_creation_tools_refuse_control_characters_in_names ... ok
test trailing_slash_is_refused_as_folder_and_extension_messages_put_cause_first ... ok
test file_creation_tools_refuse_bidi_override_characters_in_names ... ok
test save_file_and_render_png_refuse_dangling_and_live_symlinks_and_preserve_canary ... ok
test outside_workspace_refusal_never_echoes_path_and_preserves_canary ... ok
test export_fig_refuses_existing_file_unless_overwrite_is_true ... ok
test export_fig_validates_extension_case_insensitively ... ok
test bare_engine_export_fig_enforces_all_target_checks ... ok
test in_workspace_absolute_paths_via_symlink_and_var_alias_are_accepted ... ok
test new_files_respect_umask_0644_and_255_byte_names_work_in_both_modes ... ok
test render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint ... ok
test concurrent_no_overwrite_saves_never_both_succeed_or_corrupt_json ... ok
test file_planted_during_no_overwrite_export_is_never_overwritten ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.75s
```

#### Acceptance 3 — `cargo fmt --all -- --check` & `cargo clippy --workspace --all-targets --locked -- -D warnings`

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
cargo fmt --all -- --check && echo "FMT_EXIT=$?"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings && echo "CLIPPY_EXIT=$?"
rm -rf "$S"
```

Output:
```text
FMT_EXIT=0
    Checking designos-eval v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-eval)
    Checking designos-gpu v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-gpu)
    Checking egui-wgpu v0.35.0
    Checking eframe v0.35.0
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 35.88s
CLIPPY_EXIT=0
```

#### Acceptance 4 — `cargo test -p designos-mcp --locked` & `cargo test -p designos-eval --locked`

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --locked && echo "MCP_TEST_EXIT=$?"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-eval --locked && echo "EVAL_TEST_EXIT=$?"
rm -rf "$S"
```

Output:
```text
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_mcp-96fd55246d4a3a66)
test result: ok. 245 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.63s
     Running unittests src/main.rs (out/cargo/debug/deps/designos_mcp-616b53869353a607)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/bin/gen-tool-docs.rs (out/cargo/debug/deps/gen_tool_docs-f413a393b4d55d8f)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/colour_detach.rs (out/cargo/debug/deps/colour_detach-1aa6ab2233591601)
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/colour_var_library.rs (out/cargo/debug/deps/colour_var_library-01318f1c86eb2572)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running tests/colour_var_refs.rs (out/cargo/debug/deps/colour_var_refs-f067c8c79555baaa)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/list_lint_rules.rs (out/cargo/debug/deps/list_lint_rules-378a191ea16cbc7f)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.18s
     Running tests/live_doc_epoch.rs (out/cargo/debug/deps/live_doc_epoch-23ae83d21f949530)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/live_session_guards.rs (out/cargo/debug/deps/live_session_guards-ab113f1c25ffacf0)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/mcp_paths.rs (out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c)
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.90s
     Running tests/mcp_paths_full_disk.rs (out/cargo/debug/deps/mcp_paths_full_disk-a818e059bf9cad4b)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/session.rs (out/cargo/debug/deps/session-70f6c9de7d7f9973)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
     Running tests/session_file_sites/main.rs (out/cargo/debug/deps/session_file_sites-1a885e87e98ff7af)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
     Running tests/session_files/main.rs (out/cargo/debug/deps/session_files-657090e65be613f1)
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
     Running tests/spec_text_colour.rs (out/cargo/debug/deps/spec_text_colour-75b28c66cba35b01)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
     Running tests/stdio.rs (out/cargo/debug/deps/stdio-9963b9496c82d7e4)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/text_fill.rs (out/cargo/debug/deps/text_fill-b3d88a68a195ca3e)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/tools_snapshot.rs (out/cargo/debug/deps/tools_snapshot-e6488781337fbb09)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
     Running tests/version.rs (out/cargo/debug/deps/version-7878cb959d478560)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
   Doc-tests designos_mcp
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
MCP_TEST_EXIT=0
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_eval-9a9f1f166d66684f)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs (out/cargo/debug/deps/designos_eval-30aca93b7d8d8c4a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/harness.rs (out/cargo/debug/deps/harness-28beba16668beb59)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.11s
     Running tests/real_engine.rs (out/cargo/debug/deps/real_engine-4a6cab08ce582fe8)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s
   Doc-tests designos_eval
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EVAL_TEST_EXIT=0
```

#### Acceptance 5 — Delete `out/`

Command:
```sh
rm -rf out && test ! -e out && echo "OUT_REMOVED=0"
```

Output:
```text
OUT_REMOVED=0
```

---

### 3. Changes per File (Round 3)

- `crates/designos-mcp/tests/mcp_paths_full_disk.rs`:
  - Created a dedicated `#![cfg(unix)]` integration test file containing `partial_write_failure_preserves_original_leaves_no_partial_file_and_reports_honest_retry` so `RLIMIT_FSIZE` manipulation runs in its own isolated test process.
- `crates/designos-mcp/tests/mcp_paths.rs`:
  - Removed `static RLIMIT_LOCK: RwLock<()>` and the `RwLockReadGuard` field from `TempDir`, eliminating the reader/writer lock acquisition deadlock across multi-`TempDir` tests.
  - Moved `partial_write_failure_preserves_original_leaves_no_partial_file_and_reports_honest_retry` out to `mcp_paths_full_disk.rs`.
  - Added `render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint` testing 100 concurrent `designos_render_png` races and verifying that `render_png` `FILE_EXISTS` errors use the base wording (`"Cannot create PNG file (choose an unused .png filename): ..."`) without mentioning `overwrite` or setting `corrected_example`, while `designos_save_file` and `designos_export_fig_file` retain `"Pass overwrite: true to replace it."` and `corrected_example`.
- `crates/designos-mcp/src/engine.rs`:
  - Updated `write_checked_output_file` and `validate_existing_output_target` so `AlreadyExists` on `.png` outputs (`!supports_overwrite`) returns `ToolError::new(ErrorCode::FileExists, ptr, format!("Cannot create PNG file (choose an unused .png filename): {e}")).with_received(json!(display_path))`, and on tools supporting `overwrite` returns `format!("{display_path} already exists. Pass overwrite: true to replace it.")` with `corrected_example: Some(json!({ field_name: display_path, "overwrite": true }))`.
- `plans/reports/agy-1008-mcp-paths-evidence/r3-red.log`, `plans/reports/agy-1008-mcp-paths-evidence/r3-green.log`, `plans/reports/agy-1008-mcp-paths-evidence/r3-loop.log`:
  - Saved verbatim Red, Green, and 40x/20x loop outputs.

---

## Round 4

### 1. What Could NOT Be Verified

- **Windows cross-target check (`x86_64-pc-windows-gnu`):** Running `cargo check -p designos-mcp --tests --target x86_64-pc-windows-gnu --locked` on this macOS worker cannot compile `core` because the `x86_64-pc-windows-gnu` target standard library is not installed locally (`#![cfg(unix)]` remains at line 1 of `crates/designos-mcp/tests/mcp_paths.rs` and `crates/designos-mcp/tests/mcp_paths_full_disk.rs`).

---

### 2. Acceptance Table & Command Outputs (Round 4)

- **Epistemic Ledger:**
  - **FACT:** On `20913aef`, `validate_existing_output_target` (`crates/designos-mcp/src/engine.rs:1605`) returned `Ok(())` when `target_meta` existed and `!overwrite && !supports_overwrite`, causing `designos_render_png` to render the full PNG before failing at `write_checked_output_file`, and `write_checked_output_file` (`crates/designos-mcp/src/engine.rs:1657-1666`) sniffed the `.png` extension and returned `Cannot create PNG file (choose an unused .png filename): {e}` on `AlreadyExists`.
  - **FACT:** Restoring the early refusal in `validate_existing_output_target` for `!supports_overwrite` and passing `supports_overwrite: bool` explicitly to `write_checked_output_file` from its four call sites (`engine.rs:911`, `server.rs:1037`, `server.rs:1069`, `server.rs:1079`) makes both early validation and race-loser `AlreadyExists` return the exact cause-first wording `"{display_path} already exists. Choose a new path, then try again."` with `received: Some(json!(display_path))` and `corrected_example: None`, while tools with `supports_overwrite = true` keep `"{display_path} already exists. Pass overwrite: true to replace it."` and `corrected_example: Some(json!({ field_name: display_path, "overwrite": true }))`.
  - **FACT:** Both the updated race test (`render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint`) and the new non-race existing-PNG test (`render_png_refuses_existing_file_early_and_preserves_bytes`) fail on `20913aef` (`r4-red.log`) and pass on `HEAD` (`r4-green.log`), and 20 back-to-back runs of the `mcp_paths` test binary at default thread count pass with 0 hangs (`r4-loop.log`).
  - **Weakest link:** Windows target stdlib is not installed on this macOS worker, so Windows compilation of `designos-mcp` relies on CI (`#![cfg(unix)]` gates unix-only integration tests).

| # | Gate | Command | Status |
|---|---|---|---|
| 1 | Red / Green (`r4-red.log` -> `r4-green.log`) | `cargo test -p designos-mcp --test mcp_paths --locked` against `20913aef` `src/` (`19 passed; 2 failed`) and on `HEAD` (`21 passed; 0 failed`) | PASS |
| 2 | Rustfmt & Clippy | `cargo fmt --all -- --check` (`exit 0`) & `cargo clippy --workspace --all-targets --locked -- -D warnings` (`exit 0`) | PASS |
| 3 | Test suites (`designos-mcp` & `designos-eval`) | `cargo test -p designos-mcp --locked` & `cargo test -p designos-eval --locked` under short-`$TMPDIR` harness | PASS |
| 4 | 20x back-to-back `mcp_paths` binary loop (`r4-loop.log`) | 20 runs of built `mcp_paths` binary at default thread count, 60 s timeout each (`runs=20 ok=20 hang=0 fail=0`) | PASS |
| 5 | Clean workspace (`out/` removed) | `rm -rf out && test ! -e out` | PASS |

#### Acceptance 1 — Red (`20913aef`) and Green (`HEAD`) (`r4-red.log` and `r4-green.log`)

Command (`r4-red.log` against `20913aef` `src/`):
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test mcp_paths --locked > plans/reports/agy-1008-mcp-paths-evidence/r4-red.log 2>&1 || true
rm -rf "$S"
```

Output (`plans/reports/agy-1008-mcp-paths-evidence/r4-red.log`):
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-mcp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 3.28s
     Running tests/mcp_paths.rs (out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c)

running 21 tests
test export_fig_rules_list_both_file_exists_and_internal_error ... ok
test export_fig_refuses_folder ... ok
test export_fig_refuses_dangling_symlink_inside_and_outside_workspace ... ok
test export_fig_refuses_symlink_to_outside_canary_and_leaves_canary_untouched ... ok
test export_fig_refuses_symlink_inside_workspace ... ok
test argument_errors_never_echo_outside_paths_in_corrected_example_or_received ... ok
test file_creation_tools_report_name_too_long_instead_of_file_exists ... ok
test file_creation_tools_refuse_control_characters_in_names ... ok
test render_png_refuses_existing_file_early_and_preserves_bytes ... FAILED
test trailing_slash_is_refused_as_folder_and_extension_messages_put_cause_first ... ok
test file_creation_tools_refuse_bidi_override_characters_in_names ... ok
test render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint ... FAILED
test save_file_and_render_png_refuse_dangling_and_live_symlinks_and_preserve_canary ... ok
test export_fig_refuses_existing_file_unless_overwrite_is_true ... ok
test outside_workspace_refusal_never_echoes_path_and_preserves_canary ... ok
test bare_engine_export_fig_enforces_all_target_checks ... ok
test export_fig_validates_extension_case_insensitively ... ok
test in_workspace_absolute_paths_via_symlink_and_var_alias_are_accepted ... ok
test new_files_respect_umask_0644_and_255_byte_names_work_in_both_modes ... ok
test concurrent_no_overwrite_saves_never_both_succeed_or_corrupt_json ... ok
test file_planted_during_no_overwrite_export_is_never_overwritten ... ok

failures:

---- render_png_refuses_existing_file_early_and_preserves_bytes stdout ----

thread 'render_png_refuses_existing_file_early_and_preserves_bytes' (48471518) panicked at crates/designos-mcp/tests/mcp_paths.rs:621:5:
assertion `left == right` failed
  left: "Cannot create PNG file (choose an unused .png filename): File exists (os error 17)"
 right: "existing.png already exists. Choose a new path, then try again."
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint stdout ----

thread 'render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint' (48471517) panicked at crates/designos-mcp/tests/mcp_paths.rs:571:13:
assertion `left == right` failed: render_png FILE_EXISTS message must use ruling-1 wording, got: Cannot create PNG file (choose an unused .png filename): File exists (os error 17)
  left: "Cannot create PNG file (choose an unused .png filename): File exists (os error 17)"
 right: "png-race-0.png already exists. Choose a new path, then try again."


failures:
    render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint
    render_png_refuses_existing_file_early_and_preserves_bytes

test result: FAILED. 19 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.93s

error: test failed, to rerun pass `-p designos-mcp --test mcp_paths`
```

Command (`r4-green.log` on `HEAD`):
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test mcp_paths --locked > plans/reports/agy-1008-mcp-paths-evidence/r4-green.log 2>&1
rm -rf "$S"
```

Output (`plans/reports/agy-1008-mcp-paths-evidence/r4-green.log`):
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-mcp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.43s
     Running tests/mcp_paths.rs (out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c)

running 21 tests
test export_fig_rules_list_both_file_exists_and_internal_error ... ok
test export_fig_refuses_folder ... ok
test export_fig_refuses_dangling_symlink_inside_and_outside_workspace ... ok
test export_fig_refuses_symlink_inside_workspace ... ok
test export_fig_refuses_symlink_to_outside_canary_and_leaves_canary_untouched ... ok
test argument_errors_never_echo_outside_paths_in_corrected_example_or_received ... ok
test file_creation_tools_report_name_too_long_instead_of_file_exists ... ok
test trailing_slash_is_refused_as_folder_and_extension_messages_put_cause_first ... ok
test file_creation_tools_refuse_control_characters_in_names ... ok
test render_png_refuses_existing_file_early_and_preserves_bytes ... ok
test file_creation_tools_refuse_bidi_override_characters_in_names ... ok
test save_file_and_render_png_refuse_dangling_and_live_symlinks_and_preserve_canary ... ok
test outside_workspace_refusal_never_echoes_path_and_preserves_canary ... ok
test export_fig_refuses_existing_file_unless_overwrite_is_true ... ok
test export_fig_validates_extension_case_insensitively ... ok
test bare_engine_export_fig_enforces_all_target_checks ... ok
test in_workspace_absolute_paths_via_symlink_and_var_alias_are_accepted ... ok
test new_files_respect_umask_0644_and_255_byte_names_work_in_both_modes ... ok
test render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint ... ok
test concurrent_no_overwrite_saves_never_both_succeed_or_corrupt_json ... ok
test file_planted_during_no_overwrite_export_is_never_overwritten ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.05s
```

#### Acceptance 2 — `cargo fmt --all -- --check` & `cargo clippy --workspace --all-targets --locked -- -D warnings`

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
cargo fmt --all -- --check && echo "FMT_EXIT=$?"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings && echo "CLIPPY_EXIT=$?"
rm -rf "$S"
```

Output:
```text
FMT_EXIT=0
    Checking designos-eval v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-eval)
    Checking designos-gpu v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-gpu)
    Checking egui-wgpu v0.35.0
    Checking eframe v0.35.0
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 33.95s
CLIPPY_EXIT=0
```

#### Acceptance 3 — `cargo test -p designos-mcp --locked` & `cargo test -p designos-eval --locked`

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --locked && echo "MCP_TEST_EXIT=$?"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-eval --locked && echo "EVAL_TEST_EXIT=$?"
rm -rf "$S"
```

Output:
```text
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_mcp-96fd55246d4a3a66)
test result: ok. 245 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.16s
     Running unittests src/main.rs (out/cargo/debug/deps/designos_mcp-616b53869353a607)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/bin/gen-tool-docs.rs (out/cargo/debug/deps/gen_tool_docs-f413a393b4d55d8f)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/colour_detach.rs (out/cargo/debug/deps/colour_detach-1aa6ab2233591601)
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
     Running tests/colour_var_library.rs (out/cargo/debug/deps/colour_var_library-01318f1c86eb2572)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/colour_var_refs.rs (out/cargo/debug/deps/colour_var_refs-f067c8c79555baaa)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/list_lint_rules.rs (out/cargo/debug/deps/list_lint_rules-378a191ea16cbc7f)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.22s
     Running tests/live_doc_epoch.rs (out/cargo/debug/deps/live_doc_epoch-23ae83d21f949530)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/live_session_guards.rs (out/cargo/debug/deps/live_session_guards-ab113f1c25ffacf0)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/mcp_paths.rs (out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c)
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.14s
     Running tests/mcp_paths_full_disk.rs (out/cargo/debug/deps/mcp_paths_full_disk-a818e059bf9cad4b)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/session.rs (out/cargo/debug/deps/session-70f6c9de7d7f9973)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
     Running tests/session_file_sites/main.rs (out/cargo/debug/deps/session_file_sites-1a885e87e98ff7af)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s
     Running tests/session_files/main.rs (out/cargo/debug/deps/session_files-657090e65be613f1)
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
     Running tests/spec_text_colour.rs (out/cargo/debug/deps/spec_text_colour-75b28c66cba35b01)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/stdio.rs (out/cargo/debug/deps/stdio-9963b9496c82d7e4)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/text_fill.rs (out/cargo/debug/deps/text_fill-b3d88a68a195ca3e)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/tools_snapshot.rs (out/cargo/debug/deps/tools_snapshot-e6488781337fbb09)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
     Running tests/version.rs (out/cargo/debug/deps/version-7878cb959d478560)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
   Doc-tests designos_mcp
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
MCP_TEST_EXIT=0
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_eval-9a9f1f166d66684f)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs (out/cargo/debug/deps/designos_eval-30aca93b7d8d8c4a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/harness.rs (out/cargo/debug/deps/harness-28beba16668beb59)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.13s
     Running tests/real_engine.rs (out/cargo/debug/deps/real_engine-4a6cab08ce582fe8)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
   Doc-tests designos_eval
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EVAL_TEST_EXIT=0
```

#### Acceptance 4 — 20 back-to-back runs of `mcp_paths` test binary (`r4-loop.log`)

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
{
  BIN="$(env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test mcp_paths --no-run --locked 2>&1 | tee /dev/stderr | sed -n 's/.*Executable tests\/mcp_paths\.rs (\(.*\))/\1/p' | tail -n 1)"
  echo "BIN=$BIN"
  ok=0; hang=0; fail=0
  for i in $(seq 1 20); do
    out="$(python3 -c '
import os, subprocess, sys
bin_path, idx = sys.argv[1], sys.argv[2]
try:
    p = subprocess.run([bin_path], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=60)
    last = [l for l in p.stdout.splitlines() if "test result:" in l]
    summary = last[-1] if last else p.stdout[-200:]
    print(f"mcp_paths run {idx}/20: rc={p.returncode} {summary}")
    sys.exit(p.returncode)
except subprocess.TimeoutExpired:
    print(f"mcp_paths run {idx}/20: TIMEOUT (>60s)")
    sys.exit(124)
' "$BIN" "$i")"
    rc=$?
    echo "$out"
    if [ "$rc" -eq 0 ]; then ok=$((ok+1)); elif [ "$rc" -eq 124 ]; then hang=$((hang+1)); else fail=$((fail+1)); fi
  done
  echo "mcp_paths summary: runs=20 ok=$ok hang=$hang fail=$fail"
  test "$ok" -eq 20 -a "$hang" -eq 0 -a "$fail" -eq 0
} > plans/reports/agy-1008-mcp-paths-evidence/r4-loop.log 2>&1
rm -rf "$S"
```

Output (`plans/reports/agy-1008-mcp-paths-evidence/r4-loop.log`):
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-mcp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.55s
  Executable tests/mcp_paths.rs (out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c)
BIN=out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c
mcp_paths run 1/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.10s
mcp_paths run 2/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.19s
mcp_paths run 3/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.09s
mcp_paths run 4/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.98s
mcp_paths run 5/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.92s
mcp_paths run 6/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.99s
mcp_paths run 7/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.94s
mcp_paths run 8/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.05s
mcp_paths run 9/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.97s
mcp_paths run 10/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.09s
mcp_paths run 11/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.05s
mcp_paths run 12/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.09s
mcp_paths run 13/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.13s
mcp_paths run 14/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.08s
mcp_paths run 15/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.08s
mcp_paths run 16/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.12s
mcp_paths run 17/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.10s
mcp_paths run 18/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.06s
mcp_paths run 19/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.08s
mcp_paths run 20/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.13s
mcp_paths summary: runs=20 ok=20 hang=0 fail=0
```

#### Acceptance 5 — Delete `out/`

Command:
```sh
rm -rf out && test ! -e out && echo "OUT_REMOVED=0"
```

Output:
```text
OUT_REMOVED=0
```

---

### 3. Changes per File (Round 4)

- `crates/designos-mcp/src/engine.rs`:
  - Restored the early refusal in `validate_existing_output_target` when `!overwrite && !supports_overwrite`, returning `ToolError::new(ErrorCode::FileExists, ptr, format!("{display_path} already exists. Choose a new path, then try again.")).with_received(json!(display_path))` (with no `corrected_example`).
  - Added `supports_overwrite: bool` parameter to `write_checked_output_file`, removed `.png` extension sniffing, and updated the `AlreadyExists` branch for `!supports_overwrite` to return `ToolError::new(ErrorCode::FileExists, ptr, format!("{display_path} already exists. Choose a new path, then try again.")).with_received(json!(display_path))` (no OS error text, no `corrected_example`), while `supports_overwrite = true` keeps `format!("{display_path} already exists. Pass overwrite: true to replace it.")` and `with_corrected_example(json!({ field_name: display_path, "overwrite": true }))`.
  - Updated the `write_checked_output_file` call site in `Engine::call` (`"designos_export_fig_file" | "export_fig_file"`) to pass `supports_overwrite = true`.
- `crates/designos-mcp/src/server.rs`:
  - Updated the three `write_checked_output_file` call sites in `Session::call_inner`:
    - `"designos_save_file"` (`:1037`): `supports_overwrite = true`
    - `"designos_render_png"` (`:1069`): `supports_overwrite = false`
    - `"designos_export_fig_file" | "export_fig_file"` (`:1079`): `supports_overwrite = true`
- `crates/designos-mcp/tests/mcp_paths.rs`:
  - Updated `render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint` to assert the exact ruling-1 message `format!("{name} already exists. Choose a new path, then try again.")`, `err.received == Some(json!(name))`, and `err.corrected_example.is_none()`, while keeping the `designos_save_file` and `designos_export_fig_file` assertions.
  - Added `render_png_refuses_existing_file_early_and_preserves_bytes` testing `designos_render_png` on an already-existing PNG outside any race (both with a valid `node_id` and with a nonexistent `node_id` to verify early refusal in `validate_existing_output_target` before node lookup/rendering) and verifying `FILE_EXISTS`, exact ruling-1 message, `received`, `corrected_example == None`, and unchanged file bytes/SHA-256.
- `plans/reports/agy-1008-mcp-paths-evidence/r4-red.log`, `plans/reports/agy-1008-mcp-paths-evidence/r4-green.log`, `plans/reports/agy-1008-mcp-paths-evidence/r4-loop.log`:
  - Saved verbatim Red (`20913aef`), Green (`HEAD`), and 20x loop outputs.

---

### 4. Every User-Facing String, Verbatim (Round 4)

1. `"{display_path} already exists. Choose a new path, then try again."` (`ErrorCode::FileExists`, `received: Some(json!(display_path))`, `corrected_example: None`, returned by both `validate_existing_output_target` and `write_checked_output_file` when `!overwrite && !supports_overwrite`)
2. `"{display_path} already exists. Pass overwrite: true to replace it."` (`ErrorCode::FileExists`, `received: Some(json!(display_path))`, `corrected_example: Some(json!({ field_name: display_path, "overwrite": true }))`, returned by both `validate_existing_output_target` and `write_checked_output_file` when `!overwrite && supports_overwrite`)

Status: DONE
